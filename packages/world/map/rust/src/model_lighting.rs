use crate::{AmbientIndex, AmbientSample, LightingData, WorldLight, SOURCE_AMBIENT_RAY_LENGTH};
use playsrc_collision::{MASK_OPAQUE, Snapshot, SnapshotRayRequest, TraceScope, World as Collision};
use playsrc_visibility::World as Visibility;
use std::collections::BTreeMap;

const MAX_LOCAL_LIGHTS: usize = 4;
const MAX_CACHE_ENTRIES: usize = 200;
const MIN_WORLD_LIGHT: f32 = 0.0002;
const LIGHT_IN_AMBIENT_CUBE: i32 = 0x0001;
const SURFACE_SKY: u16 = 0x0004;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectedWorldLight {
    pub source: usize,
    pub light: WorldLight,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelWorldLighting {
    pub origin: [f32; 3],
    pub leaf: usize,
    pub ambient_cube: [[f32; 3]; 6],
    pub local_lights: Vec<SelectedWorldLight>,
}

#[derive(Debug)]
pub struct ModelLightingWorld {
    indexes: Vec<AmbientIndex>,
    samples: Vec<AmbientSample>,
    lights: Vec<WorldLight>,
    cache: BTreeMap<(i32, i32, i32, usize), ModelWorldLighting>,
}

impl ModelLightingWorld {
    pub fn new(lighting: LightingData) -> Self {
        Self {
            indexes: lighting.ambient_indexes,
            samples: lighting.ambient_samples,
            lights: lighting.world_lights,
            cache: BTreeMap::new(),
        }
    }

    pub fn sample(
        &mut self,
        origin: [f32; 3],
        visibility: &Visibility,
        collision: &Collision,
        snapshot: &Snapshot,
    ) -> Result<ModelWorldLighting, ()> {
        if origin.iter().any(|value| !value.is_finite()) {
            return Err(());
        }
        let leaf = visibility.locate_leaf(origin).map_err(|_| ())?;
        let key = (
            (origin[0] as i32 + 32_768) >> 5,
            (origin[1] as i32 + 32_768) >> 5,
            (origin[2] as i32 + 32_768) >> 7,
            leaf,
        );
        if let Some(value) = self.cache.get(&key) {
            return Ok(value.clone());
        }
        let origin = cache_lighting_origin(origin, leaf, visibility, collision, snapshot)?;
        let mut ambient_cube = self.sample_leaf(leaf, origin, visibility)?;
        let cluster = visibility.leaves.get(leaf).ok_or(())?.cluster;
        let mut selected = Vec::<(f32, SelectedWorldLight, [f32; 3], f32)>::new();
        let mut skylight = false;

        for (source, light) in self.lights.iter().copied().enumerate() {
            if light.kind == 5
                || light.flags & LIGHT_IN_AMBIENT_CUBE != 0
                || (light.kind == 3 && skylight)
                || (light.kind != 3
                    && (cluster < 0
                        || light.cluster < 0
                        || !visibility.visible(cluster as usize, light.cluster as usize)))
            {
                continue;
            }
            let (end, direction, ratio) = source_world_light_ray(origin, &light)?;
            if ratio <= 0.0 {
                continue;
            }
            let maximum = light.intensity.into_iter().fold(0.0_f32, f32::max);
            if light.kind != 0 && maximum * ratio < MIN_WORLD_LIGHT {
                continue;
            }
            let trace = collision
                .trace_snapshot_ray(
                    snapshot,
                    SnapshotRayRequest {
                        start: origin,
                        end,
                        mask: MASK_OPAQUE,
                        scope: TraceScope::WorldOnly,
                        ignored: &[],
                    },
                    |_| false,
                )
                .map_err(|_| ())?;
            let distance = length(subtract(end, origin));
            let admitted = if light.kind == 3 {
                trace.surface_flags & SURFACE_SKY != 0
            } else {
                (1.0 - trace.fraction) * distance <= 8.0
            };
            if !admitted {
                continue;
            }
            if light.kind == 3 {
                skylight = true;
            }
            let illumination = ratio * dot(light.intensity, [0.299, 0.587, 0.114]);
            let angular = source_world_light_angle(&light, direction)?;
            let record = SelectedWorldLight { source, light };
            if light.kind != 0 && illumination < MIN_WORLD_LIGHT {
                add_world_light_to_cube(&mut ambient_cube, direction, light.intensity, ratio * angular)?;
            } else if selected.len() < MAX_LOCAL_LIGHTS {
                selected.push((illumination, record, direction, ratio));
            } else if let Some((minimum, _)) = selected
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| left.0.total_cmp(&right.0))
                && illumination > selected[minimum].0
            {
                let (_, demoted, old_direction, old_ratio) =
                    std::mem::replace(&mut selected[minimum], (illumination, record, direction, ratio));
                add_world_light_to_cube(
                    &mut ambient_cube,
                    old_direction,
                    demoted.light.intensity,
                    old_ratio * source_world_light_angle(&demoted.light, old_direction)?,
                )?;
            } else {
                add_world_light_to_cube(&mut ambient_cube, direction, light.intensity, ratio * angular)?;
            }
        }

        let value = ModelWorldLighting {
            origin,
            leaf,
            ambient_cube,
            local_lights: selected.into_iter().map(|(_, light, _, _)| light).collect(),
        };
        if self.cache.len() == MAX_CACHE_ENTRIES {
            self.cache.pop_first();
        }
        self.cache.insert(key, value.clone());
        Ok(value)
    }

    fn sample_leaf(
        &self,
        mut leaf: usize,
        origin: [f32; 3],
        visibility: &Visibility,
    ) -> Result<[[f32; 3]; 6], ()> {
        let mut index = *self.indexes.get(leaf).ok_or(())?;
        if index.sample_count == 0 && index.first_sample != 0 {
            leaf = usize::from(index.first_sample);
            index = *self.indexes.get(leaf).ok_or(())?;
        }
        if index.sample_count == 0 {
            return Ok([[0.0; 3]; 6]);
        }
        let bounds = visibility.leaves.get(leaf).ok_or(())?;
        let start = usize::from(index.first_sample);
        let end = start.checked_add(usize::from(index.sample_count)).ok_or(())?;
        let samples = self.samples.get(start..end).ok_or(())?;
        let mut cube = [[0.0; 3]; 6];
        let mut total = 0.0;
        for sample in samples {
            let position = std::array::from_fn(|axis| {
                let minimum = f32::from(bounds.mins[axis]);
                let maximum = f32::from(bounds.maxs[axis]);
                minimum + (maximum - minimum) * (f32::from(sample.position[axis]) / 255.0)
            });
            let delta = subtract(position, origin);
            let factor = 1.0 / (dot(delta, delta) + 1.0);
            total += factor;
            for (side, source) in cube.iter_mut().zip(sample.cube) {
                for (channel, value) in side.iter_mut().zip(source) {
                    *channel += value * factor;
                }
            }
        }
        if !total.is_finite() || total <= 0.0 {
            return Err(());
        }
        for side in &mut cube {
            for channel in side {
                *channel /= total;
                if !channel.is_finite() {
                    return Err(());
                }
            }
        }
        Ok(cube)
    }
}

fn cache_lighting_origin(
    origin: [f32; 3],
    origin_leaf: usize,
    visibility: &Visibility,
    collision: &Collision,
    snapshot: &Snapshot,
) -> Result<[f32; 3], ()> {
    let (minimum, maximum) = source_lightcache_bounds(origin);
    let mut center = std::array::from_fn(|axis| (minimum[axis] + maximum[axis]) * 0.5);
    let center_leaf = visibility.locate_leaf(center).map_err(|_| ())?;
    let mut trace_center = true;
    if center_leaf != origin_leaf {
        let contents = visibility.leaves.get(center_leaf).ok_or(())?.contents as u32;
        if contents & MASK_OPAQUE != 0 {
            trace_center = false;
        } else {
            snap_to_reference_leaf(visibility, origin, &mut center)?;
        }
    }
    let trace = |end| {
        collision
            .trace_snapshot_ray(
                snapshot,
                SnapshotRayRequest {
                    start: origin,
                    end,
                    mask: MASK_OPAQUE,
                    scope: TraceScope::WorldOnly,
                    ignored: &[],
                },
                |_| false,
            )
            .map_err(|_| ())
    };
    if trace_center {
        let hit = trace(center)?;
        if hit.start_solid {
            return Ok(origin);
        }
        if hit.fraction >= 1.0 {
            return Ok(center);
        }
    }
    center[0] = (minimum[0] + maximum[0]) * 0.5;
    center[1] = (minimum[1] + maximum[1]) * 0.5;
    center[2] = origin[2];
    snap_to_reference_leaf(visibility, origin, &mut center)?;
    Ok(if trace(center)?.fraction < 1.0 { origin } else { center })
}

fn snap_to_reference_leaf(
    visibility: &Visibility,
    reference: [f32; 3],
    point: &mut [f32; 3],
) -> Result<(), ()> {
    let mut child = visibility.models.first().ok_or(())?.head_node;
    let mut depth = 0;
    while child >= 0 {
        if depth > visibility.nodes.len() {
            return Err(());
        }
        let node = visibility.nodes.get(child as usize).ok_or(())?;
        let plane = visibility.planes.get(node.plane_index as usize).ok_or(())?;
        let side = dot(plane.normal, reference) - plane.distance;
        let distance = dot(plane.normal, *point) - plane.distance;
        if side < 0.0 {
            child = node.children[1];
            if distance > 0.0 {
                *point = subtract(*point, scale(plane.normal, distance + 0.5));
            }
        } else {
            child = node.children[0];
            if distance < 0.0 {
                *point = add(*point, scale(plane.normal, -distance + 0.5));
            }
        }
        depth += 1;
    }
    Ok(())
}

pub fn source_lightcache_bounds(origin: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let sizes = [32.0, 32.0, 128.0];
    let minimum = std::array::from_fn(|axis| {
        let cell = (origin[axis].abs() as i32) / sizes[axis] as i32;
        if origin[axis] >= 0.0 {
            cell as f32 * sizes[axis]
        } else {
            -((cell + 1) as f32) * sizes[axis]
        }
    });
    let maximum = std::array::from_fn(|axis| minimum[axis] + sizes[axis]);
    (minimum, maximum)
}

pub fn source_world_light_ray(
    origin: [f32; 3],
    light: &WorldLight,
) -> Result<([f32; 3], [f32; 3], f32), ()> {
    if !(0..=5).contains(&light.kind) {
        return Err(());
    }
    if light.kind == 3 {
        let direction = light.normal.map(|value| -value);
        return Ok((add(origin, scale(direction, SOURCE_AMBIENT_RAY_LENGTH)), direction, 1.0));
    }
    let delta = subtract(light.origin, origin);
    let squared = dot(delta, delta);
    let distance = squared.sqrt();
    if !distance.is_finite() || distance == 0.0 {
        return Err(());
    }
    let direction = scale(delta, distance.recip());
    let (minimum, maximum) = source_lightcache_bounds(origin);
    let radius = length(subtract(maximum, origin));
    if matches!(light.kind, 1 | 2) {
        let closest = std::array::from_fn(|axis| light.origin[axis].clamp(minimum[axis], maximum[axis]));
        let delta = subtract(closest, light.origin);
        if dot(delta, delta) > light.radius * light.radius {
            return Ok((light.origin, direction, 0.0));
        }
    }
    if light.kind == 2 {
        let sine = (1.0 - light.stop_dot2 * light.stop_dot2).max(0.0).sqrt();
        if !sphere_intersects_cone(origin, radius, light.origin, light.normal, sine, light.stop_dot2)? {
            return Ok((light.origin, direction, 0.0));
        }
    } else if light.kind == 0
        && (distance > radius + light.radius
            || !sphere_intersects_cone(origin, radius, light.origin, light.normal, 1.0, 0.0)?)
    {
        return Ok((light.origin, direction, 0.0));
    }
    let ratio = match light.kind {
        0 => {
            if light.radius != 0.0 && distance > light.radius {
                0.0
            } else {
                squared.max(1.0).recip()
            }
        }
        1 | 2 => (light.constant_attenuation
            + light.linear_attenuation * distance
            + light.quadratic_attenuation * squared)
            .recip(),
        4 => (light.linear_attenuation - distance).max(0.0),
        _ => 0.0,
    };
    ratio.is_finite().then_some((light.origin, direction, ratio)).ok_or(())
}

fn sphere_intersects_cone(
    center: [f32; 3],
    radius: f32,
    origin: [f32; 3],
    normal: [f32; 3],
    sine: f32,
    cosine: f32,
) -> Result<bool, ()> {
    if !sine.is_finite() || sine <= 0.0 {
        return Err(());
    }
    let back = subtract(origin, scale(normal, radius / sine));
    let delta = subtract(center, back);
    if dot(normal, delta) >= length(delta) * cosine {
        let delta = subtract(center, origin);
        let distance = length(delta);
        if -dot(normal, delta) >= distance * sine {
            return Ok(distance <= radius);
        }
        return Ok(true);
    }
    Ok(false)
}

pub fn source_world_light_angle(light: &WorldLight, direction: [f32; 3]) -> Result<f32, ()> {
    let value = match light.kind {
        0 => (-dot(direction, light.normal)).max(0.0),
        1 | 4 => 1.0,
        2 => {
            let cone = -dot(direction, light.normal);
            if cone <= light.stop_dot2 {
                0.0
            } else if cone >= light.stop_dot {
                1.0
            } else {
                let factor = (cone - light.stop_dot2) / (light.stop_dot - light.stop_dot2);
                if light.exponent == 0.0 || light.exponent == 1.0 {
                    factor
                } else {
                    factor.powf(light.exponent)
                }
            }
        }
        3 => 1.0,
        _ => 0.0,
    };
    value.is_finite().then_some(value).ok_or(())
}

pub fn add_world_light_to_cube(
    cube: &mut [[f32; 3]; 6],
    direction: [f32; 3],
    intensity: [f32; 3],
    ratio: f32,
) -> Result<(), ()> {
    for (face, axis) in cube.iter_mut().zip([
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ]) {
        let weight = dot(axis, direction);
        if weight > 0.0 {
            for channel in 0..3 {
                face[channel] += ratio * weight * intensity[channel];
                if !face[channel].is_finite() {
                    return Err(());
                }
            }
        }
    }
    Ok(())
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}
fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}
fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}
fn scale(value: [f32; 3], amount: f32) -> [f32; 3] {
    [value[0] * amount, value[1] * amount, value[2] * amount]
}
fn length(value: [f32; 3]) -> f32 {
    dot(value, value).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visibility(leaves: Vec<playsrc_bsp::Leaf>) -> Visibility {
        Visibility {
            identity: [0; 32],
            visibility_mode: playsrc_visibility::VisibilityMode::NoVis,
            cluster_count: 0,
            words_per_row: 0,
            pvs: Vec::new(),
            pas: Vec::new(),
            planes: Vec::new(),
            nodes: Vec::new(),
            leaves,
            leaf_faces: Vec::new(),
            models: Vec::new(),
            areas: Vec::new(),
            portals: Vec::new(),
            portal_vertices: Vec::new(),
            leaf_displacements: Vec::new(),
        }
    }

    fn leaf() -> playsrc_bsp::Leaf {
        playsrc_bsp::Leaf {
            contents: 0,
            cluster: 0,
            area_and_flags: 0,
            mins: [0; 3],
            maxs: [255; 3],
            first_leaf_face: 0,
            leaf_face_count: 0,
            first_leaf_brush: 0,
            leaf_brush_count: 0,
            leaf_water_data_id: -1,
            padding: 0,
            ambient_cube: None,
        }
    }

    fn light() -> WorldLight {
        WorldLight {
            origin: [64.0, 0.0, 0.0],
            intensity: [1.0; 3],
            normal: [-1.0, 0.0, 0.0],
            cluster: 0,
            kind: 1,
            style: 0,
            stop_dot: 0.9,
            stop_dot2: 0.8,
            exponent: 1.0,
            radius: 128.0,
            constant_attenuation: 1.0,
            linear_attenuation: 0.0,
            quadratic_attenuation: 1.0,
            flags: 0,
            texture_info: -1,
            owner: -1,
        }
    }

    #[test]
    fn world_lights_keep_authored_attenuation_cones_and_signed_cache_cells() {
        let mut light = light();
        let (_, direction, ratio) = source_world_light_ray([0.0; 3], &light).unwrap();
        assert_eq!(direction, [1.0, 0.0, 0.0]);
        assert_eq!(ratio, 1.0 / 4097.0);
        light.kind = 2;
        assert_eq!(source_world_light_angle(&light, direction).unwrap(), 1.0);
        light.normal = [1.0, 0.0, 0.0];
        assert_eq!(source_world_light_angle(&light, direction).unwrap(), 0.0);
        light.kind = 1;
        light.radius = 1.0;
        assert_eq!(source_world_light_ray([0.0; 3], &light).unwrap().2, 0.0);
        assert_eq!(
            source_lightcache_bounds([-0.5, 32.0, -128.5]),
            ([-32.0, 32.0, -256.0], [0.0, 64.0, -128.0]),
        );
    }

    #[test]
    fn demoted_world_lights_accumulate_only_on_facing_ambient_sides() {
        let mut cube = [[0.0; 3]; 6];
        add_world_light_to_cube(&mut cube, [1.0, 0.0, 0.0], [2.0, 3.0, 4.0], 0.5)
            .unwrap();
        assert_eq!(cube[0], [1.0, 1.5, 2.0]);
        assert!(cube[1..].iter().all(|side| *side == [0.0; 3]));
    }

    #[test]
    fn ambient_samples_use_inverse_squared_distance_and_borrow_solid_leaf_neighbors() {
        let world = ModelLightingWorld {
            indexes: vec![
                AmbientIndex { sample_count: 0, first_sample: 1 },
                AmbientIndex { sample_count: 2, first_sample: 0 },
            ],
            samples: vec![
                AmbientSample { cube: [[1.0, 2.0, 3.0]; 6], position: [0; 3] },
                AmbientSample { cube: [[3.0, 6.0, 9.0]; 6], position: [255, 0, 0] },
            ],
            lights: Vec::new(),
            cache: BTreeMap::new(),
        };
        let visibility = visibility(vec![leaf(), leaf()]);
        let cube = world.sample_leaf(0, [0.0; 3], &visibility).unwrap();
        let far = 1.0 / (255.0_f32 * 255.0 + 1.0);
        let expected = (1.0 + 3.0 * far) / (1.0 + far);
        assert_eq!(cube[0][0].to_bits(), expected.to_bits());
        assert_eq!(cube[5][1].to_bits(), (expected * 2.0).to_bits());
    }
}
