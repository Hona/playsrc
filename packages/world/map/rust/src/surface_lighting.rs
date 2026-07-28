use crate::{CanonicalMap, LightingSamples, SurfaceLightingKind};
use playsrc_visibility::World as VisibilityWorld;
use std::collections::BTreeSet;

const SURF_SKY_2D: i32 = 0x0002;
const SURF_SKY: i32 = 0x0004;
const SURF_NODRAW: i32 = 0x0080;
pub const SOURCE_AMBIENT_RAY_LENGTH: f32 = 32_768.0 * 1.74;
pub const SOURCE_AMBIENT_RAY_COUNT: usize = 162;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceLightError {
    InvalidInput,
    Cancelled,
}
impl From<()> for SurfaceLightError {
    fn from(_: ()) -> Self {
        Self::InvalidInput
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceLightHit {
    pub face: usize,
    pub displacement: Option<usize>,
    pub fraction: f32,
    pub lightmap: [i32; 2],
    pub radiance: [f32; 3],
    pub sky: bool,
}

#[derive(Clone, Debug)]
struct FaceLight {
    average: Option<[f32; 3]>,
    sample_start: usize,
}

#[derive(Clone, Debug)]
pub struct SurfaceLightingWorld {
    map: CanonicalMap,
    visibility: VisibilityWorld,
    detail_faces: Vec<Vec<usize>>,
    face_light: Vec<FaceLight>,
    samples: Vec<[f32; 3]>,
    water_materials: BTreeSet<usize>,
    sky_ambient: Option<[f32; 3]>,
}

impl SurfaceLightingWorld {
    pub fn compile(
        map: &CanonicalMap,
        visibility: &VisibilityWorld,
        water_materials: BTreeSet<usize>,
    ) -> Result<Self, SurfaceLightError> {
        if map.surfaces.len() != map.lighting.surfaces.len()
            || visibility.leaves.len() != visibility.leaf_displacements.len()
        {
            return Err(SurfaceLightError::InvalidInput);
        }
        let LightingSamples::LinearRgb32(samples) = &map.lighting.samples else {
            return Err(SurfaceLightError::InvalidInput);
        };
        let mut node_faces = BTreeSet::new();
        for node in &visibility.nodes {
            let start = usize::from(node.first_face);
            let end = start.checked_add(usize::from(node.face_count)).ok_or(())?;
            if end > map.surfaces.len() {
                return Err(SurfaceLightError::InvalidInput);
            }
            node_faces.extend(start..end);
        }
        let mut detail_faces = Vec::with_capacity(visibility.leaves.len());
        for leaf in &visibility.leaves {
            let start = usize::from(leaf.first_leaf_face);
            let end = start
                .checked_add(usize::from(leaf.leaf_face_count))
                .ok_or(())?;
            let faces = visibility.leaf_faces.get(start..end).ok_or(())?;
            detail_faces.push(
                faces
                    .iter()
                    .map(|face| usize::from(*face))
                    .filter(|face| !node_faces.contains(face))
                    .collect(),
            );
        }
        let mut face_light = Vec::with_capacity(map.lighting.surfaces.len());
        for (face, lighting) in map.lighting.surfaces.iter().enumerate() {
            if lighting.face as usize != face {
                return Err(SurfaceLightError::InvalidInput);
            }
            let sample_start = lighting.sample_start as usize;
            let average = if lighting.kind == SurfaceLightingKind::Unlit || sample_start == 0 {
                None
            } else {
                Some(*samples.get(sample_start - 1).ok_or(())?)
            };
            face_light.push(FaceLight {
                average,
                sample_start,
            });
        }
        let sky_ambient = map
            .lighting
            .world_lights
            .iter()
            .find(|light| light.kind == 5)
            .map(|light| light.intensity);
        Ok(Self {
            map: map.clone(),
            visibility: visibility.clone(),
            detail_faces,
            face_light,
            samples: samples.clone(),
            water_materials,
            sky_ambient,
        })
    }

    pub fn trace(
        &self,
        start: [f32; 3],
        end: [f32; 3],
    ) -> Result<Option<SurfaceLightHit>, SurfaceLightError> {
        if start.into_iter().chain(end).any(|value| !value.is_finite()) {
            return Err(SurfaceLightError::InvalidInput);
        }
        let delta = sub(end, start);
        let mut state = TraceState {
            start,
            delta,
            closest: 1.0,
            hit: None,
            sky: None,
            touched_displacements: BTreeSet::new(),
        };
        let head = self.visibility.models.first().ok_or(())?.head_node;
        self.trace_child(head, 0.0, 1.0, &mut state, 0)?;
        for face in state.touched_displacements.clone() {
            self.test_displacement(face, &mut state)?;
        }
        Ok(state.hit.or(state.sky))
    }

    fn trace_child(
        &self,
        child: i32,
        start: f32,
        end: f32,
        state: &mut TraceState,
        depth: usize,
    ) -> Result<(), SurfaceLightError> {
        if depth > self.visibility.nodes.len() {
            return Err(SurfaceLightError::InvalidInput);
        }
        if child < 0 {
            let leaf = (-1i64 - i64::from(child)) as usize;
            for face in self.detail_faces.get(leaf).ok_or(())? {
                if self
                    .map
                    .surfaces
                    .get(*face)
                    .and_then(|surface| surface.displacement.as_ref())
                    .is_some()
                {
                    continue;
                }
                let surface = self.map.surfaces.get(*face).ok_or(())?;
                if dot(surface.plane[..3].try_into().unwrap(), state.delta) > 0.0 {
                    continue;
                }
                let denom = dot(surface.plane[..3].try_into().unwrap(), state.delta);
                if denom == 0.0 {
                    continue;
                }
                let fraction = (surface.plane[3]
                    - dot(surface.plane[..3].try_into().unwrap(), state.start))
                    / denom;
                if fraction >= start && fraction <= end && fraction < state.closest {
                    self.test_face(*face, fraction, state)?;
                }
            }
            for face in self.visibility.leaf_displacements.get(leaf).ok_or(())? {
                state.touched_displacements.insert(usize::from(*face));
            }
            return Ok(());
        }
        let node = self.visibility.nodes.get(child as usize).ok_or(())?;
        let plane = self
            .visibility
            .planes
            .get(node.plane_index as usize)
            .ok_or(())?;
        let front = dot(plane.normal, state.start) + start * dot(plane.normal, state.delta)
            - plane.distance;
        let back =
            dot(plane.normal, state.start) + end * dot(plane.normal, state.delta) - plane.distance;
        let side = usize::from(front < 0.0);
        if (back < 0.0) == (side == 1) {
            return self.trace_child(node.children[side], start, end, state, depth + 1);
        }
        let fraction = front / (front - back);
        let mid = start * (1.0 - fraction) + end * fraction;
        self.trace_child(node.children[side], start, mid, state, depth + 1)?;
        if state.hit.is_some() {
            return Ok(());
        }
        let first = usize::from(node.first_face);
        for face in first..first + usize::from(node.face_count) {
            if self.map.surfaces[face].flags & (SURF_SKY | SURF_SKY_2D) != 0 {
                self.remember_sky(face, mid, state);
                continue;
            }
            if self.test_face(face, mid, state)? {
                return Ok(());
            }
        }
        self.trace_child(node.children[1 - side], mid, end, state, depth + 1)
    }

    fn test_face(
        &self,
        face: usize,
        fraction: f32,
        state: &mut TraceState,
    ) -> Result<bool, SurfaceLightError> {
        let surface = self.map.surfaces.get(face).ok_or(())?;
        if fraction >= state.closest
            || surface.flags & SURF_NODRAW != 0
            || self.water_materials.contains(&surface.material)
            || self
                .face_light
                .get(face)
                .and_then(|light| light.average)
                .is_none()
        {
            return Ok(false);
        }
        if surface.flags & (SURF_SKY | SURF_SKY_2D) != 0 {
            self.remember_sky(face, fraction, state);
            return Ok(false);
        }
        let point = add(state.start, scale(state.delta, fraction));
        let (ds, dt) = lightmap_coordinates(surface, point)?;
        if !lightmap_admitted(surface, ds, dt) {
            return Ok(false);
        }
        let average = self.face_light[face].average.ok_or(())?;
        let reflectivity = self
            .map
            .materials
            .get(surface.material)
            .ok_or(())?
            .reflectivity;
        state.closest = fraction;
        state.hit = Some(SurfaceLightHit {
            face,
            displacement: None,
            fraction,
            lightmap: [ds as i32, dt as i32],
            radiance: mul(average, reflectivity),
            sky: false,
        });
        Ok(true)
    }

    fn remember_sky(&self, face: usize, fraction: f32, state: &mut TraceState) {
        if state.sky.is_none() {
            state.sky = Some(SurfaceLightHit {
                face,
                displacement: None,
                fraction,
                lightmap: [0; 2],
                radiance: self.sky_ambient.unwrap_or([0.0; 3]),
                sky: true,
            });
        }
    }

    fn test_displacement(
        &self,
        face: usize,
        state: &mut TraceState,
    ) -> Result<(), SurfaceLightError> {
        let surface = self.map.surfaces.get(face).ok_or(())?;
        let displacement = surface.displacement.as_ref().ok_or(())?;
        for (triangle_index, triangle) in surface.triangles.iter().enumerate() {
            let points = triangle.map(|index| surface.positions[index as usize]);
            if let Some((fraction, barycentric)) = ray_triangle(state.start, state.delta, points)
                && fraction < state.closest
            {
                let uv = interpolate2(
                    surface.lightmap_uv[triangle[0] as usize],
                    surface.lightmap_uv[triangle[1] as usize],
                    surface.lightmap_uv[triangle[2] as usize],
                    barycentric,
                );
                let ds = (uv[0] * surface.lightmap_size[0] as f32) as i32;
                let dt = (uv[1] * surface.lightmap_size[1] as f32) as i32;
                let light = &self.face_light[face];
                let smax = surface.lightmap_size[0] as usize + 1;
                let index = light
                    .sample_start
                    .checked_add(dt.max(0) as usize * smax + ds.max(0) as usize)
                    .ok_or(())?;
                let radiance = *self.samples.get(index).ok_or(())?;
                let reflectivity = self.map.materials[surface.material].reflectivity;
                state.closest = fraction;
                state.hit = Some(SurfaceLightHit {
                    face,
                    displacement: Some(displacement.source),
                    fraction,
                    lightmap: [ds, dt],
                    radiance: mul(radiance, reflectivity),
                    sky: false,
                });
                let _ = triangle_index;
            }
        }
        Ok(())
    }

    pub fn ambient_cube(
        &self,
        origin: [f32; 3],
        directions: &[[f32; 3]],
        mut cancelled: impl FnMut(usize) -> bool,
    ) -> Result<[[f32; 3]; 6], SurfaceLightError> {
        if directions.len() != SOURCE_AMBIENT_RAY_COUNT {
            return Err(SurfaceLightError::InvalidInput);
        }
        let mut samples = Vec::with_capacity(directions.len());
        for (index, direction) in directions.iter().copied().enumerate() {
            if cancelled(index) || direction.iter().any(|value| !value.is_finite()) {
                return Err(SurfaceLightError::Cancelled);
            }
            let end = add(origin, scale(direction, SOURCE_AMBIENT_RAY_LENGTH));
            samples.push(
                self.trace(origin, end)?
                    .map_or([0.0; 3], |hit| hit.radiance),
            );
        }
        project_ambient_cube(directions, &samples)
    }
}

struct TraceState {
    start: [f32; 3],
    delta: [f32; 3],
    closest: f32,
    hit: Option<SurfaceLightHit>,
    sky: Option<SurfaceLightHit>,
    touched_displacements: BTreeSet<usize>,
}

pub fn project_ambient_cube(
    directions: &[[f32; 3]],
    samples: &[[f32; 3]],
) -> Result<[[f32; 3]; 6], SurfaceLightError> {
    if directions.len() != SOURCE_AMBIENT_RAY_COUNT || directions.len() != samples.len() {
        return Err(SurfaceLightError::InvalidInput);
    }
    let axes = [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    let mut output = [[0.0; 3]; 6];
    for (side, axis) in axes.into_iter().enumerate() {
        let mut weight = 0.0;
        for (direction, sample) in directions.iter().zip(samples) {
            let value = dot(*direction, axis);
            if value > 0.0 {
                weight += value;
                output[side] = add(output[side], scale(*sample, value));
            }
        }
        if weight <= 0.0 {
            return Err(SurfaceLightError::InvalidInput);
        }
        output[side] = scale(output[side], 1.0 / weight);
    }
    Ok(output)
}

fn lightmap_coordinates(
    surface: &crate::Surface,
    point: [f32; 3],
) -> Result<(f32, f32), SurfaceLightError> {
    let project = |axis: usize| {
        dot(
            point,
            surface.lightmap_vectors[axis][..3].try_into().unwrap(),
        ) + surface.lightmap_vectors[axis][3]
            - surface.lightmap_mins[axis] as f32
    };
    Ok((project(0), project(1)))
}

fn lightmap_admitted(surface: &crate::Surface, ds: f32, dt: f32) -> bool {
    ds >= 0.0
        && dt >= 0.0
        && ds <= surface.lightmap_size[0] as f32
        && dt <= surface.lightmap_size[1] as f32
}

fn ray_triangle(
    start: [f32; 3],
    delta: [f32; 3],
    points: [[f32; 3]; 3],
) -> Option<(f32, [f32; 3])> {
    let edge1 = sub(points[1], points[0]);
    let edge2 = sub(points[2], points[0]);
    let p = cross(delta, edge2);
    let determinant = dot(edge1, p);
    if determinant >= 0.0 || determinant.abs() < 1e-8 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let t = sub(start, points[0]);
    let u = dot(t, p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(t, edge1);
    let v = dot(delta, q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let fraction = dot(edge2, q) * inverse;
    (0.0..=1.0)
        .contains(&fraction)
        .then_some((fraction, [1.0 - u - v, u, v]))
}

fn interpolate2(a: [f32; 2], b: [f32; 2], c: [f32; 2], weights: [f32; 3]) -> [f32; 2] {
    [
        a[0] * weights[0] + b[0] * weights[1] + c[0] * weights[2],
        a[1] * weights[0] + b[1] * weights[1] + c[1] * weights[2],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn mul(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_projection_preserves_axis_weighting_and_order() {
        let mut directions = vec![[1.0, 0.0, 0.0]; SOURCE_AMBIENT_RAY_COUNT];
        for (index, direction) in directions.iter_mut().enumerate() {
            *direction = match index % 6 {
                0 => [1.0, 0.0, 0.0],
                1 => [-1.0, 0.0, 0.0],
                2 => [0.0, 1.0, 0.0],
                3 => [0.0, -1.0, 0.0],
                4 => [0.0, 0.0, 1.0],
                _ => [0.0, 0.0, -1.0],
            };
        }
        let samples = directions
            .iter()
            .map(|direction| direction.map(f32::abs))
            .collect::<Vec<_>>();
        assert_eq!(
            project_ambient_cube(&directions, &samples).unwrap(),
            [
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0]
            ]
        );
    }
}
