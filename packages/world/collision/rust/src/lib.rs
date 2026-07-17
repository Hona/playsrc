use playsrc_bsp::{
    Brush as BspBrush, BrushSide as BspSide, Bsp, Leaf, LumpData, Model, Node, Plane as BspPlane,
};
use std::{fmt, ops::Range};

mod snapshot;

pub use snapshot::{
    Candidate, ConvexInput, ObjectInput, ObjectOverlapRequest, ObjectRole, ObjectTraceRequest,
    PhysicsShape, SNAPSHOT_VERSION, Snapshot, SnapshotLimits, SnapshotRayRequest, SnapshotShape,
    SnapshotTraceRequest, TraceScope, Transform,
};

pub const SURF_SKY: u16 = 0x0004;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub distance: f32,
    pub kind: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Brush {
    pub first_side: usize,
    pub side_count: usize,
    pub contents: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Side {
    pub plane: usize,
    pub texture_info: i16,
    pub displacement: i16,
    pub bevel: i16,
}
#[derive(Clone, Debug)]
pub struct World {
    pub planes: Vec<Plane>,
    pub sides: Vec<Side>,
    pub brushes: Vec<Brush>,
    pub leaves: Vec<Leaf>,
    pub leaf_brushes: Vec<u16>,
    pub nodes: Vec<Node>,
    pub models: Vec<Model>,
    pub world_brushes: Vec<usize>,
    pub model_brushes: Vec<Vec<usize>>,
    pub texture_flags: Vec<u16>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub fraction: f32,
    pub fraction_left_solid: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub brush: Option<usize>,
    pub contents: u32,
    pub plane: Option<Plane>,
    pub surface_flags: u16,
    pub hit: Option<Hit>,
    pub snapshot: Option<u64>,
    pub end: [f32; 3],
}
impl Trace {
    pub fn did_hit(self) -> bool {
        self.fraction < 1.0 || self.start_solid || self.all_solid
    }

    pub fn is_sky(self) -> bool {
        self.surface_flags & SURF_SKY != 0
    }

    pub fn entity_identity(self) -> Option<u64> {
        match self.hit {
            Some(Hit::Object {
                identity,
                role: ObjectRole::Entity,
                ..
            }) => Some(identity),
            _ => None,
        }
    }

    pub fn hit_world(self) -> bool {
        matches!(
            self.hit,
            Some(Hit::WorldBrush { .. })
                | Some(Hit::Object {
                    role: ObjectRole::StaticProp,
                    ..
                })
        )
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hit {
    WorldBrush {
        brush: usize,
    },
    Object {
        identity: u64,
        role: ObjectRole,
        feature: Feature,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Feature {
    Brush {
        model: usize,
        brush: usize,
    },
    Box,
    Convex {
        solid: usize,
        convex: usize,
        triangle: Option<usize>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidRange,
    InvalidReference,
    NonFinite,
    InvalidHull,
    InvalidSnapshot,
    DuplicateIdentity,
    Limit,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub item: Option<usize>,
    pub range: Option<Range<usize>>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for Error {}

pub fn compile(bsp: &Bsp) -> Result<World, Error> {
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let sides = match &bsp.lumps[19].records {
        LumpData::BrushSides(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let texture_info = match &bsp.lumps[6].records {
        LumpData::TextureInfo(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let brushes = match &bsp.lumps[18].records {
        LumpData::Brushes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaves = match &bsp.lumps[10].records {
        LumpData::Leaves(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaf_brushes = match &bsp.lumps[17].records {
        LumpData::LeafBrushes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let nodes = match &bsp.lumps[5].records {
        LumpData::Nodes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let models = match &bsp.lumps[14].records {
        LumpData::Models(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let planes = planes
        .iter()
        .enumerate()
        .map(|(i, p)| plane(p, i))
        .collect::<Result<Vec<_>, _>>()?;
    let texture_flags = texture_info
        .iter()
        .map(|value| value.flags as u16)
        .collect::<Vec<_>>();
    let sides = sides.iter().map(side).collect::<Vec<_>>();
    let mut output = Vec::with_capacity(brushes.len());
    for (i, b) in brushes.iter().enumerate() {
        let brush = brush(b, i, sides.len())?;
        for side in &sides[brush.first_side..brush.first_side + brush.side_count] {
            if side.plane >= planes.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
            if side.texture_info < -1
                || side.texture_info >= 0 && side.texture_info as usize >= texture_flags.len()
            {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
        }
        output.push(brush);
    }
    for (i, leaf) in leaves.iter().enumerate() {
        let start = leaf.first_leaf_brush as usize;
        let end = start
            .checked_add(leaf.leaf_brush_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(i)))?;
        if end > leaf_brushes.len()
            || leaf_brushes[start..end]
                .iter()
                .any(|v| *v as usize >= output.len())
        {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
    }
    for (i, node) in nodes.iter().enumerate() {
        if node.plane_index < 0 || node.plane_index as usize >= planes.len() {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
    }
    if models.is_empty() {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    let model_brushes = models
        .iter()
        .map(|model| {
            brushes_for_model(
                model.head_node,
                &nodes,
                &leaves,
                &leaf_brushes,
                output.len(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let world_brushes = model_brushes[0].clone();
    Ok(World {
        planes,
        sides,
        brushes: output,
        leaves,
        leaf_brushes,
        nodes,
        models,
        world_brushes,
        model_brushes,
        texture_flags,
    })
}
fn plane(p: &BspPlane, i: usize) -> Result<Plane, Error> {
    let normal = [p.normal.x.value(), p.normal.y.value(), p.normal.z.value()];
    let distance = p.distance.value();
    if normal.iter().any(|v| !v.is_finite()) || !distance.is_finite() {
        return Err(error(ErrorCode::NonFinite, Some(i)));
    }
    Ok(Plane {
        normal,
        distance,
        kind: p.kind,
    })
}
fn side(v: &BspSide) -> Side {
    Side {
        plane: v.plane_index as usize,
        texture_info: v.texture_info_index,
        displacement: v.displacement_info_index,
        bevel: v.bevel,
    }
}
fn brush(v: &BspBrush, i: usize, total: usize) -> Result<Brush, Error> {
    let first =
        usize::try_from(v.first_side).map_err(|_| error(ErrorCode::InvalidRange, Some(i)))?;
    let count =
        usize::try_from(v.side_count).map_err(|_| error(ErrorCode::InvalidRange, Some(i)))?;
    if first.checked_add(count).is_none_or(|x| x > total) {
        return Err(error(ErrorCode::InvalidRange, Some(i)));
    }
    Ok(Brush {
        first_side: first,
        side_count: count,
        contents: v.contents as u32,
    })
}

impl World {
    pub fn empty() -> Self {
        Self {
            planes: Vec::new(),
            sides: Vec::new(),
            brushes: Vec::new(),
            leaves: Vec::new(),
            leaf_brushes: Vec::new(),
            nodes: Vec::new(),
            models: Vec::new(),
            world_brushes: Vec::new(),
            model_brushes: Vec::new(),
            texture_flags: Vec::new(),
        }
    }

    pub fn trace_hull(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, Error> {
        let Some(model) = self.models.first() else {
            return self.trace_brushes(start, end, hull, mask, &[], BrushOwner::World);
        };
        self.trace_headnode(model.head_node, start, end, hull, mask, BrushOwner::World)
    }

    pub fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, Error> {
        let model_record = self
            .models
            .get(model)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(model)))?;
        if origin.iter().any(|value| !value.is_finite()) {
            return Err(error(ErrorCode::InvalidHull, Some(model)));
        }
        let local = [
            position[0] - origin[0],
            position[1] - origin[1],
            position[2] - origin[2],
        ];
        self.trace_headnode(
            model_record.head_node,
            local,
            local,
            hull,
            u32::MAX,
            BrushOwner::Model {
                identity: 0,
                role: ObjectRole::Entity,
                model,
            },
        )
        .map(|trace| trace.start_solid)
    }

    pub(crate) fn trace_model_hull(
        &self,
        model: usize,
        request: ObjectTraceRequest,
        identity: u64,
        role: ObjectRole,
    ) -> Result<Trace, Error> {
        let model_record = self
            .models
            .get(model)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(model)))?;
        let basis = request.transform.basis()?;
        let local_start = basis.inverse_point(request.start);
        let local_end = add(
            local_start,
            basis.inverse_vector(sub(request.end, request.start)),
        );
        let mut trace = self.trace_headnode(
            model_record.head_node,
            local_start,
            local_end,
            request.hull,
            request.mask,
            BrushOwner::Model {
                identity,
                role,
                model,
            },
        )?;
        if let Some(plane) = trace.plane.as_mut() {
            plane.normal = basis.vector(plane.normal);
        }
        trace.end = interpolate(request.start, request.end, trace.fraction);
        Ok(trace)
    }

    fn trace_headnode(
        &self,
        head_node: i32,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
        owner: BrushOwner,
    ) -> Result<Trace, Error> {
        let brushes = self.ordered_brushes(head_node, start, end, hull)?;
        self.trace_brushes(start, end, hull, mask, &brushes, owner)
    }

    fn ordered_brushes(
        &self,
        head_node: i32,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
    ) -> Result<Vec<usize>, Error> {
        if start
            .into_iter()
            .chain(end)
            .chain(hull.mins)
            .chain(hull.maxs)
            .any(|value| !value.is_finite())
            || hull
                .mins
                .into_iter()
                .zip(hull.maxs)
                .any(|(minimum, maximum)| minimum > maximum)
        {
            return Err(error(ErrorCode::InvalidHull, None));
        }
        let center = scale(add(hull.mins, hull.maxs), 0.5);
        let extents = scale(sub(hull.maxs, hull.mins), 0.5);
        let point = dot(extents, extents) < 1.0e-6;
        let mut pending = vec![(head_node, add(start, center), add(end, center), 0_usize)];
        let mut seen = std::collections::BTreeSet::new();
        let mut ordered = Vec::new();
        while let Some((mut child, first, mut second, depth)) = pending.pop() {
            let mut traversal_depth = depth;
            while child >= 0 {
                traversal_depth += 1;
                if traversal_depth > self.nodes.len() {
                    return Err(error(ErrorCode::InvalidReference, None));
                }
                let node = self
                    .nodes
                    .get(child as usize)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
                let plane = self
                    .planes
                    .get(node.plane_index as usize)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
                let first_distance = dot(first, plane.normal) - plane.distance;
                let second_distance = dot(second, plane.normal) - plane.distance;
                let offset = if point {
                    0.0
                } else if (0..3).contains(&plane.kind) {
                    extents[plane.kind as usize]
                } else {
                    dot_abs(extents, plane.normal)
                };
                if first_distance > offset && second_distance > offset {
                    child = node.children[0];
                    continue;
                }
                if first_distance < -offset && second_distance < -offset {
                    child = node.children[1];
                    continue;
                }
                let (side, near_fraction, far_fraction) = if first_distance < second_distance {
                    let reciprocal = 1.0 / (first_distance - second_distance);
                    (
                        1,
                        ((first_distance - offset - 0.03125) * reciprocal).clamp(0.0, 1.0),
                        ((first_distance + offset + 0.03125) * reciprocal).clamp(0.0, 1.0),
                    )
                } else if first_distance > second_distance {
                    let reciprocal = 1.0 / (first_distance - second_distance);
                    (
                        0,
                        ((first_distance + offset + 0.03125) * reciprocal).clamp(0.0, 1.0),
                        ((first_distance - offset - 0.03125) * reciprocal).clamp(0.0, 1.0),
                    )
                } else {
                    (0, 1.0, 0.0)
                };
                let near_end = interpolate(first, second, near_fraction);
                let far_start = interpolate(first, second, far_fraction);
                pending.push((node.children[side ^ 1], far_start, second, traversal_depth));
                child = node.children[side];
                second = near_end;
            }
            let leaf_index = (-1_i64 - i64::from(child)) as usize;
            let leaf = self
                .leaves
                .get(leaf_index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf_index)))?;
            let begin = leaf.first_leaf_brush as usize;
            let finish = begin + leaf.leaf_brush_count as usize;
            for brush in self
                .leaf_brushes
                .get(begin..finish)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf_index)))?
            {
                let brush = *brush as usize;
                if seen.insert(brush) {
                    ordered.push(brush);
                }
            }
        }
        Ok(ordered)
    }

    fn trace_brushes(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
        brushes: &[usize],
        owner: BrushOwner,
    ) -> Result<Trace, Error> {
        if start
            .iter()
            .chain(end.iter())
            .chain(hull.mins.iter())
            .chain(hull.maxs.iter())
            .any(|v| !v.is_finite())
            || hull.mins.iter().zip(hull.maxs).any(|(a, b)| *a > b)
        {
            return Err(error(ErrorCode::InvalidHull, None));
        }
        let center = [
            (hull.mins[0] + hull.maxs[0]) * 0.5,
            (hull.mins[1] + hull.maxs[1]) * 0.5,
            (hull.mins[2] + hull.maxs[2]) * 0.5,
        ];
        let extents = [
            (hull.maxs[0] - hull.mins[0]) * 0.5,
            (hull.maxs[1] - hull.mins[1]) * 0.5,
            (hull.maxs[2] - hull.mins[2]) * 0.5,
        ];
        let trace_start = [
            start[0] + center[0],
            start[1] + center[1],
            start[2] + center[2],
        ];
        let trace_end = [end[0] + center[0], end[1] + center[1], end[2] + center[2]];
        let point = dot(extents, extents) < 1.0e-6;
        let mut result = Trace {
            fraction: 1.0,
            fraction_left_solid: 0.0,
            start_solid: false,
            all_solid: false,
            brush: None,
            contents: 0,
            plane: None,
            surface_flags: 0,
            hit: None,
            snapshot: None,
            end,
        };
        for &index in brushes {
            let brush = &self.brushes[index];
            if brush.contents & mask == 0 {
                continue;
            }
            let mut enter = -1.0_f32;
            let mut leave = 1.0_f32;
            let mut starts_out = false;
            let mut gets_out = false;
            let mut clip = None;
            let mut rejected = false;
            for side in &self.sides[brush.first_side..brush.first_side + brush.side_count] {
                if point && side.bevel != 0 {
                    continue;
                }
                let p = self.planes[side.plane];
                let offset = if point {
                    0.0
                } else {
                    extents[0] * p.normal[0].abs()
                        + extents[1] * p.normal[1].abs()
                        + extents[2] * p.normal[2].abs()
                };
                let d1 = dot(trace_start, p.normal) - p.distance - offset;
                let d2 = dot(trace_end, p.normal) - p.distance - offset;
                if d1 > 0.0 {
                    starts_out = true;
                    if d2 > 0.0 {
                        rejected = true;
                        break;
                    }
                } else {
                    if d2 <= 0.0 {
                        continue;
                    }
                    gets_out = true;
                }
                if d1 > d2 {
                    let f = (d1 - 0.03125).max(0.0) / (d1 - d2);
                    if f > enter {
                        enter = f;
                        clip = Some((p, *side));
                    }
                } else {
                    leave = leave.min((d1 + 0.03125) / (d1 - d2));
                }
            }
            if rejected {
                continue;
            }
            if point && starts_out && result.fraction_left_solid - enter > 0.0 {
                starts_out = false;
            }
            if !starts_out {
                result.start_solid = true;
                result.contents = brush.contents;
                result.brush = Some(index);
                result.hit = Some(owner.hit(index));
                if !gets_out {
                    result.all_solid = true;
                    result.fraction = 0.0;
                    result.fraction_left_solid = 1.0;
                    result.plane = None;
                    result.surface_flags = 0;
                    break;
                } else if point && leave != 1.0 && leave > result.fraction_left_solid {
                    result.fraction_left_solid = leave;
                    if result.fraction <= leave {
                        result.fraction = 1.0;
                        result.plane = None;
                        result.surface_flags = 0;
                    }
                }
                continue;
            }
            if enter < leave && enter > -1.0 && enter < result.fraction {
                result.fraction = enter.max(0.0);
                result.brush = Some(index);
                result.contents = brush.contents;
                let (plane, side) = clip.expect("an entering brush plane is selected");
                result.plane = Some(plane);
                result.surface_flags = self.surface_flags(side);
                result.hit = Some(owner.hit(index));
            }
        }
        result.end = interpolate(start, end, result.fraction);
        Ok(result)
    }

    fn surface_flags(&self, side: Side) -> u16 {
        usize::try_from(side.texture_info)
            .ok()
            .and_then(|index| self.texture_flags.get(index))
            .copied()
            .unwrap_or(0)
    }
}
pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn scale(a: [f32; 3], value: f32) -> [f32; 3] {
    [a[0] * value, a[1] * value, a[2] * value]
}
pub(crate) fn interpolate(start: [f32; 3], end: [f32; 3], fraction: f32) -> [f32; 3] {
    add(start, scale(sub(end, start), fraction))
}
fn dot_abs(extents: [f32; 3], normal: [f32; 3]) -> f32 {
    extents[0] * normal[0].abs() + extents[1] * normal[1].abs() + extents[2] * normal[2].abs()
}
fn error(code: ErrorCode, item: Option<usize>) -> Error {
    Error {
        code,
        item,
        range: None,
    }
}
fn brushes_for_model(
    head: i32,
    nodes: &[Node],
    leaves: &[Leaf],
    leaf_brushes: &[u16],
    brush_count: usize,
) -> Result<Vec<usize>, Error> {
    let mut pending = vec![head];
    let mut seen_nodes = std::collections::BTreeSet::new();
    let mut seen_leaves = std::collections::BTreeSet::new();
    let mut brushes = std::collections::BTreeSet::new();
    while let Some(child) = pending.pop() {
        if child >= 0 {
            let index = child as usize;
            let node = nodes
                .get(index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            if seen_nodes.insert(index) {
                pending.extend(node.children);
            }
        } else {
            let index = (-1_i64 - child as i64) as usize;
            if !seen_leaves.insert(index) {
                continue;
            }
            let leaf = leaves
                .get(index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            let start = leaf.first_leaf_brush as usize;
            let end = start + leaf.leaf_brush_count as usize;
            for &brush in leaf_brushes
                .get(start..end)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?
            {
                if brush as usize >= brush_count {
                    return Err(error(ErrorCode::InvalidReference, Some(index)));
                }
                brushes.insert(brush as usize);
            }
        }
    }
    Ok(brushes.into_iter().collect())
}

#[derive(Clone, Copy)]
enum BrushOwner {
    World,
    Model {
        identity: u64,
        role: ObjectRole,
        model: usize,
    },
}
impl BrushOwner {
    fn hit(self, brush: usize) -> Hit {
        match self {
            Self::World => Hit::WorldBrush { brush },
            Self::Model {
                identity,
                role,
                model,
            } => Hit::Object {
                identity,
                role,
                feature: Feature::Brush { model, brush },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_bsp::{Limits, Profile, parse};
    fn set(b: &mut [u8], i: usize, o: usize, l: usize, v: i32) {
        let h = 8 + i * 16;
        b[h..h + 4].copy_from_slice(&(o as i32).to_le_bytes());
        b[h + 4..h + 8].copy_from_slice(&(l as i32).to_le_bytes());
        b[h + 8..h + 12].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture() -> Bsp {
        let mut b = vec![0; 1036];
        b[..4].copy_from_slice(b"VBSP");
        b[4..8].copy_from_slice(&20_i32.to_le_bytes());
        let mut add = |i: usize, v: i32, x: &[u8]| {
            let o = b.len();
            b.extend_from_slice(x);
            set(&mut b, i, o, x.len(), v);
        };
        let mut p = Vec::new();
        for (n, d) in [
            ([1_f32, 0., 0.], 16_f32),
            ([-1., 0., 0.], 16.),
            ([0., 1., 0.], 16.),
            ([0., -1., 0.], 16.),
            ([0., 0., 1.], 16.),
            ([0., 0., -1.], 16.),
        ] {
            for x in n {
                p.extend_from_slice(&x.to_le_bytes())
            }
            p.extend_from_slice(&d.to_le_bytes());
            p.extend_from_slice(&0_i32.to_le_bytes());
        }
        add(1, 0, &p);
        let mut texture_info = [0_u8; 72];
        texture_info[64..68].copy_from_slice(&(SURF_SKY as i32).to_le_bytes());
        add(6, 0, &texture_info);
        let mut sides = Vec::new();
        for i in 0..6_u16 {
            sides.extend_from_slice(&i.to_le_bytes());
            sides.extend_from_slice(&0_i16.to_le_bytes());
            sides.extend_from_slice(&(-1_i16).to_le_bytes());
            sides.extend_from_slice(&0_i16.to_le_bytes());
        }
        add(19, 0, &sides);
        let mut brush = 0_i32.to_le_bytes().to_vec();
        brush.extend_from_slice(&6_i32.to_le_bytes());
        brush.extend_from_slice(&1_i32.to_le_bytes());
        brush.extend_from_slice(&0_i32.to_le_bytes());
        brush.extend_from_slice(&6_i32.to_le_bytes());
        brush.extend_from_slice(&1_i32.to_le_bytes());
        add(18, 0, &brush);
        let mut leaf = vec![0; 32];
        leaf[24..26].copy_from_slice(&0_u16.to_le_bytes());
        leaf[26..28].copy_from_slice(&1_u16.to_le_bytes());
        add(10, 1, &leaf);
        add(17, 0, &0_u16.to_le_bytes());
        add(5, 0, &[0; 32]);
        let mut model = [0; 48];
        model[36..40].copy_from_slice(&(-1_i32).to_le_bytes());
        add(14, 0, &model);
        parse(&b, Profile::Source2013V20, Limits::default()).unwrap()
    }
    #[test]
    fn traces_point_and_hull_against_brush() {
        let w = compile(&fixture()).unwrap();
        assert_eq!(w.brushes.len(), 2);
        assert_eq!(w.world_brushes, [0]);
        assert_eq!(w.model_brushes, [vec![0]]);
        let point = Hull {
            mins: [0.; 3],
            maxs: [0.; 3],
        };
        let t = w
            .trace_hull([-32., 0., 0.], [32., 0., 0.], point, 1)
            .unwrap();
        assert_eq!(t.brush, Some(0));
        assert!(t.fraction > 0.24 && t.fraction < 0.26);
        assert_eq!(t.plane.unwrap().normal, [-1., 0., 0.]);
        assert!(t.is_sky());
        assert_eq!(t.hit, Some(Hit::WorldBrush { brush: 0 }));
        let hull = Hull {
            mins: [-2.; 3],
            maxs: [2.; 3],
        };
        let h = w
            .trace_hull([-32., 0., 0.], [32., 0., 0.], hull, 1)
            .unwrap();
        assert!(h.fraction < t.fraction);
        let inside = w.trace_hull([0.; 3], [1., 0., 0.], point, 1).unwrap();
        assert!(inside.start_solid && inside.all_solid);
        assert!(
            w.overlaps_model_hull(0, [100., 0., 0.], [100., 0., 0.], point)
                .unwrap()
        );
        assert!(
            !w.overlaps_model_hull(0, [100., 0., 0.], [0., 0., 0.], point)
                .unwrap()
        );
    }

    fn box_object(identity: u64, minimum: [f32; 3], maximum: [f32; 3]) -> ObjectInput {
        ObjectInput {
            identity,
            role: ObjectRole::Entity,
            enabled: true,
            transform: Transform::IDENTITY,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 1,
            surface_flags: 0,
            shape: SnapshotShape::BoundingBox {
                bounds: Hull {
                    mins: minimum,
                    maxs: maximum,
                },
            },
        }
    }

    #[test]
    fn bounded_snapshot_traces_world_models_and_entities_in_source_order() {
        let world = compile(&fixture()).unwrap();
        let translated_model = ObjectInput {
            identity: 40,
            role: ObjectRole::Entity,
            enabled: true,
            transform: Transform {
                origin: [100.0, 0.0, 0.0],
                angles: [0.0, 90.0, 0.0],
            },
            linear_velocity: [10.0, 0.0, 0.0],
            angular_velocity: [0.0, 90.0, 0.0],
            collision_group: 3,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel { model: 0 },
        };
        let snapshot =
            Snapshot::compile(&world, 9, vec![translated_model], SnapshotLimits::default())
                .unwrap();
        let trace = world
            .trace_snapshot_ray(
                &snapshot,
                SnapshotRayRequest {
                    start: [100.0, -32.0, 0.0],
                    end: [100.0, 32.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(trace.snapshot, Some(9));
        assert_eq!(trace.end[0], 100.0);
        assert!(trace.end[1] < -15.9);
        let normal = trace.plane.unwrap().normal;
        assert!(normal[0].abs() < 0.000001 && (normal[1] + 1.0).abs() < 0.000001);
        assert_eq!(
            trace.hit,
            Some(Hit::Object {
                identity: 40,
                role: ObjectRole::Entity,
                feature: Feature::Brush { model: 0, brush: 0 },
            })
        );

        let empty = World::empty();
        let ordered = Snapshot::compile(
            &empty,
            10,
            vec![
                box_object(8, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
                box_object(7, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let direct = empty
            .trace_snapshot_ray(
                &ordered,
                SnapshotRayRequest {
                    start: [-10.0, 0.0, 0.0],
                    end: [10.0, 0.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(
            direct.hit,
            Some(Hit::Object {
                identity: 8,
                role: ObjectRole::Entity,
                feature: Feature::Box,
            })
        );
        assert_eq!(direct.plane.unwrap().normal, [-1.0, 0.0, 0.0]);
        assert_eq!(&ordered.snapshot_bytes().unwrap()[..8], b"CSNP\x01\0\0\0");
    }

    #[test]
    fn physics_convexes_preserve_feature_contents_and_transform() {
        let vertices = vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let shape = PhysicsShape::compile(
            55,
            vec![ConvexInput {
                solid: 2,
                convex: 3,
                contents: 0x4000_0001,
                vertices,
                triangles,
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let world = World::empty();
        let snapshot = Snapshot::compile(
            &world,
            12,
            vec![ObjectInput {
                identity: 99,
                role: ObjectRole::StaticProp,
                enabled: true,
                transform: Transform {
                    origin: [5.0, 0.0, 0.0],
                    angles: [0.0, 45.0, 0.0],
                },
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                collision_group: 0,
                contents: 0,
                surface_flags: 0,
                shape: SnapshotShape::Physics(shape),
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let filter_calls = std::cell::Cell::new(0);
        let trace = world
            .trace_snapshot_ray(
                &snapshot,
                SnapshotRayRequest {
                    start: [0.0, 0.0, 0.0],
                    end: [10.0, 0.0, 0.0],
                    mask: u32::MAX,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| {
                    filter_calls.set(filter_calls.get() + 1);
                    false
                },
            )
            .unwrap();
        assert_eq!(
            filter_calls.get(),
            0,
            "ordinary static props bypass entity filters"
        );
        assert!(trace.fraction < 0.5);
        assert_eq!(trace.contents, 0x4000_0001);
        assert!(matches!(
            trace.hit,
            Some(Hit::Object {
                identity: 99,
                role: ObjectRole::StaticProp,
                feature: Feature::Convex {
                    solid: 2,
                    convex: 3,
                    triangle: Some(_),
                },
            })
        ));
    }

    #[test]
    fn snapshot_limits_and_world_equal_hit_precedence_are_categorical() {
        let world = compile(&fixture()).unwrap();
        let coincident = Snapshot::compile(
            &world,
            13,
            vec![box_object(1, [-16.0; 3], [16.0; 3])],
            SnapshotLimits::default(),
        )
        .unwrap();
        let trace = world
            .trace_snapshot_ray(
                &coincident,
                SnapshotRayRequest {
                    start: [-32.0, 0.0, 0.0],
                    end: [32.0, 0.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(trace.hit, Some(Hit::WorldBrush { brush: 0 }));

        assert_eq!(
            Snapshot::compile(
                &World::empty(),
                1,
                vec![box_object(1, [0.0; 3], [1.0; 3])],
                SnapshotLimits {
                    max_objects: 0,
                    ..SnapshotLimits::default()
                },
            )
            .unwrap_err()
            .code,
            ErrorCode::Limit
        );
        let tiny = Snapshot::compile(
            &World::empty(),
            1,
            vec![box_object(1, [0.0; 3], [1.0; 3])],
            SnapshotLimits {
                max_snapshot_bytes: 8,
                ..SnapshotLimits::default()
            },
        )
        .unwrap();
        assert_eq!(tiny.snapshot_bytes().unwrap_err().code, ErrorCode::Limit);
    }

    #[test]
    fn equal_world_contacts_retain_near_leaf_brush_order() {
        let mut world = compile(&fixture()).unwrap();
        world.leaf_brushes = vec![1, 0];
        world.leaves[0].first_leaf_brush = 0;
        world.leaves[0].leaf_brush_count = 2;
        let point = Hull {
            mins: [0.0; 3],
            maxs: [0.0; 3],
        };
        let first = world
            .trace_hull([-32.0, 0.0, 0.0], [32.0, 0.0, 0.0], point, 1)
            .unwrap();
        assert_eq!(first.hit, Some(Hit::WorldBrush { brush: 1 }));
        world.leaf_brushes = vec![0, 1];
        let reversed = world
            .trace_hull([-32.0, 0.0, 0.0], [32.0, 0.0, 0.0], point, 1)
            .unwrap();
        assert_eq!(reversed.hit, Some(Hit::WorldBrush { brush: 0 }));
    }
}
