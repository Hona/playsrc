use crate::{
    Error, ErrorCode, Feature, Hit, Hull, Plane, Trace, World, add, dot, error, interpolate, scale,
    sub,
};
use playsrc_phy::{Asset as PhyAsset, Classification as PhyClassification};
use std::collections::BTreeSet;

pub const SNAPSHOT_VERSION: u32 = 2;
const DIST_EPSILON: f32 = 1.0 / 32.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectRole {
    Entity,
    StaticProp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
}
impl Transform {
    pub const IDENTITY: Self = Self {
        origin: [0.0; 3],
        angles: [0.0; 3],
    };

    pub(crate) fn basis(self) -> Result<Basis, Error> {
        if self
            .origin
            .into_iter()
            .chain(self.angles)
            .any(|value| !value.is_finite())
        {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let (sp, cp) = self.angles[0].to_radians().sin_cos();
        let (sy, cy) = self.angles[1].to_radians().sin_cos();
        let (sr, cr) = self.angles[2].to_radians().sin_cos();
        let crcy = cr * cy;
        let crsy = cr * sy;
        let srcy = sr * cy;
        let srsy = sr * sy;
        Ok(Basis {
            columns: [
                [cp * cy, cp * sy, -sp],
                [sp * srcy - crsy, sp * srsy + crcy, sr * cp],
                [sp * crcy + srsy, sp * crsy - srcy, cr * cp],
            ],
            origin: self.origin,
        })
    }

    pub fn transform_point(self, point: [f32; 3]) -> Result<[f32; 3], Error> {
        self.basis().map(|basis| basis.point(point))
    }

    pub fn inverse_transform_point(self, point: [f32; 3]) -> Result<[f32; 3], Error> {
        self.basis().map(|basis| basis.inverse_point(point))
    }

    pub fn compose(self, local: Self) -> Result<Self, Error> {
        self.basis()?;
        local.basis()?;
        let rotation = quaternion_from_angles(self.angles);
        Ok(Self {
            origin: add(self.origin, quaternion_rotate(rotation, local.origin)),
            angles: angles_from_quaternion(quaternion_multiply(
                rotation,
                quaternion_from_angles(local.angles),
            )),
        })
    }

    pub fn relative_to(self, parent: Self) -> Result<Self, Error> {
        self.basis()?;
        parent.basis()?;
        let inverse = quaternion_inverse(quaternion_from_angles(parent.angles));
        Ok(Self {
            origin: quaternion_rotate(inverse, sub(self.origin, parent.origin)),
            angles: angles_from_quaternion(quaternion_multiply(
                inverse,
                quaternion_from_angles(self.angles),
            )),
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Basis {
    columns: [[f32; 3]; 3],
    origin: [f32; 3],
}
impl Basis {
    pub(crate) fn point(self, point: [f32; 3]) -> [f32; 3] {
        add(self.origin, self.vector(point))
    }

    pub(crate) fn vector(self, vector: [f32; 3]) -> [f32; 3] {
        add(
            add(
                scale(self.columns[0], vector[0]),
                scale(self.columns[1], vector[1]),
            ),
            scale(self.columns[2], vector[2]),
        )
    }

    pub(crate) fn inverse_point(self, point: [f32; 3]) -> [f32; 3] {
        self.inverse_vector(sub(point, self.origin))
    }

    pub(crate) fn inverse_vector(self, vector: [f32; 3]) -> [f32; 3] {
        [
            dot(vector, self.columns[0]),
            dot(vector, self.columns[1]),
            dot(vector, self.columns[2]),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    pub max_objects: usize,
    pub max_convexes: usize,
    pub max_vertices: usize,
    pub max_triangles: usize,
    pub max_axes_per_convex: usize,
    pub max_candidate_visits: usize,
    pub max_ignored_identities: usize,
    pub max_snapshot_bytes: usize,
}
impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_objects: 4_096,
            max_convexes: 65_536,
            max_vertices: 3_000_000,
            max_triangles: 1_000_000,
            max_axes_per_convex: 1_000_000,
            max_candidate_visits: 4_096,
            max_ignored_identities: 4_096,
            max_snapshot_bytes: 16 * 1024 * 1024,
        }
    }
}
impl SnapshotLimits {
    fn validate(self) -> Result<Self, Error> {
        if [
            self.max_objects,
            self.max_convexes,
            self.max_vertices,
            self.max_triangles,
            self.max_axes_per_convex,
            self.max_candidate_visits,
            self.max_ignored_identities,
            self.max_snapshot_bytes,
        ]
        .contains(&0)
        {
            Err(error(ErrorCode::Limit, None))
        } else {
            Ok(self)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConvexInput {
    pub solid: usize,
    pub convex: usize,
    pub contents: u32,
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsShape {
    pub identity: u64,
    convexes: Vec<Convex>,
}
impl PhysicsShape {
    pub fn compile(
        identity: u64,
        inputs: Vec<ConvexInput>,
        limits: SnapshotLimits,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        if inputs.is_empty() || inputs.len() > limits.max_convexes {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut vertex_count = 0_usize;
        let mut triangle_count = 0_usize;
        let mut convexes = Vec::with_capacity(inputs.len());
        for (item, input) in inputs.into_iter().enumerate() {
            vertex_count = vertex_count
                .checked_add(input.vertices.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            triangle_count = triangle_count
                .checked_add(input.triangles.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            if vertex_count > limits.max_vertices
                || triangle_count > limits.max_triangles
                || input.vertices.len() < 4
                || input.triangles.len() < 4
                || input
                    .vertices
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
            {
                return Err(error(ErrorCode::InvalidSnapshot, Some(item)));
            }
            let center = scale(
                input.vertices.iter().copied().fold([0.0; 3], add),
                1.0 / input.vertices.len() as f32,
            );
            let mut faces = Vec::with_capacity(input.triangles.len());
            let edge_capacity = input
                .triangles
                .len()
                .checked_mul(3)
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            let mut edges = Vec::with_capacity(edge_capacity);
            for (triangle, indexes) in input.triangles.iter().copied().enumerate() {
                let [a, b, c] = indexes.map(|index| {
                    input
                        .vertices
                        .get(index as usize)
                        .copied()
                        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(item)))
                });
                let (a, b, c) = (a?, b?, c?);
                let mut normal = cross(sub(b, a), sub(c, a));
                let length = length(normal);
                if !length.is_finite() || length <= f32::EPSILON {
                    return Err(error(ErrorCode::Unsupported, Some(item)));
                }
                normal = scale(normal, 1.0 / length);
                if dot(normal, sub(center, a)) > 0.0 {
                    normal = scale(normal, -1.0);
                }
                faces.push(Face { normal, triangle });
                edges.extend([sub(b, a), sub(c, b), sub(a, c)]);
            }
            let axis_count = faces
                .len()
                .checked_add(3)
                .and_then(|value| {
                    edges
                        .len()
                        .checked_mul(3)
                        .and_then(|edges| value.checked_add(edges))
                })
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            if axis_count > limits.max_axes_per_convex {
                return Err(error(ErrorCode::Limit, Some(item)));
            }
            convexes.push(Convex {
                solid: input.solid,
                convex: input.convex,
                contents: input.contents,
                vertices: input.vertices,
                faces,
                edges,
            });
        }
        Ok(Self { identity, convexes })
    }

    pub fn from_phy(
        identity: u64,
        asset: &PhyAsset,
        solid: usize,
        limits: SnapshotLimits,
        mut contents_for_game_data: impl FnMut(i32) -> u32,
    ) -> Result<Self, Error> {
        let source = asset
            .solids
            .get(solid)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(solid)))?;
        if source.classification != PhyClassification::Handled {
            return Err(error(ErrorCode::Unsupported, Some(solid)));
        }
        let inputs = source
            .convexes
            .iter()
            .enumerate()
            .map(|(convex, value)| ConvexInput {
                solid,
                convex,
                contents: contents_for_game_data(value.client_data),
                vertices: value
                    .points
                    .iter()
                    .map(|point| point.source_inches.map(|axis| f32::from_bits(axis.0)))
                    .collect(),
                triangles: value
                    .triangles
                    .iter()
                    .map(|triangle| triangle.point_indices)
                    .collect(),
            })
            .collect();
        Self::compile(identity, inputs, limits)
    }

    fn contents(&self) -> u32 {
        self.convexes
            .iter()
            .fold(0, |contents, convex| contents | convex.contents)
    }

    fn counts(&self) -> (usize, usize, usize) {
        self.convexes.iter().fold(
            (0_usize, 0_usize, 0_usize),
            |(convexes, vertices, triangles), convex| {
                (
                    convexes + 1,
                    vertices + convex.vertices.len(),
                    triangles + convex.faces.len(),
                )
            },
        )
    }

    pub fn convex_count(&self) -> usize {
        self.convexes.len()
    }

    pub fn local_bounds(&self) -> Hull {
        let mut mins = [f32::INFINITY; 3];
        let mut maxs = [f32::NEG_INFINITY; 3];
        for vertex in self
            .convexes
            .iter()
            .flat_map(|convex| convex.vertices.iter())
        {
            for axis in 0..3 {
                mins[axis] = mins[axis].min(vertex[axis]);
                maxs[axis] = maxs[axis].max(vertex[axis]);
            }
        }
        Hull { mins, maxs }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Face {
    normal: [f32; 3],
    triangle: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct Convex {
    solid: usize,
    convex: usize,
    contents: u32,
    vertices: Vec<[f32; 3]>,
    faces: Vec<Face>,
    edges: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SnapshotShape {
    BrushModel { model: usize },
    BoundingBox { bounds: Hull },
    OrientedBox { bounds: Hull },
    Physics(PhysicsShape),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectInput {
    pub identity: u64,
    pub role: ObjectRole,
    pub enabled: bool,
    pub transform: Transform,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub collision_group: i32,
    pub contents: u32,
    pub surface_flags: u16,
    pub shape: SnapshotShape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotRecord {
    pub identity: u64,
    pub role: ObjectRole,
    pub enabled: bool,
    pub transform: Transform,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub collision_group: i32,
    pub contents: u32,
    pub surface_flags: u16,
    pub shape: SnapshotShape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    world: [u8; 32],
    identity: u64,
    objects: Vec<SnapshotRecord>,
    limits: SnapshotLimits,
}
impl Snapshot {
    pub fn compile(
        world: &World,
        identity: u64,
        inputs: Vec<ObjectInput>,
        limits: SnapshotLimits,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        if inputs.len() > limits.max_objects {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut identities = BTreeSet::new();
        let mut objects = Vec::with_capacity(inputs.len());
        let mut convex_count = 0_usize;
        let mut vertex_count = 0_usize;
        let mut triangle_count = 0_usize;
        for (item, input) in inputs.into_iter().enumerate() {
            if !identities.insert(input.identity) {
                return Err(error(ErrorCode::DuplicateIdentity, Some(item)));
            }
            input.transform.basis()?;
            if input
                .linear_velocity
                .into_iter()
                .chain(input.angular_velocity)
                .any(|value| !value.is_finite())
            {
                return Err(error(ErrorCode::InvalidSnapshot, Some(item)));
            }
            let contents = match &input.shape {
                SnapshotShape::BrushModel { model } => {
                    let brushes = world
                        .model_brushes
                        .get(*model)
                        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(item)))?;
                    brushes
                        .iter()
                        .fold(0, |value, brush| value | world.brushes[*brush].contents)
                }
                SnapshotShape::BoundingBox { bounds } => {
                    validate_hull(*bounds, item)?;
                    if input.transform.angles != [0.0; 3] {
                        return Err(error(ErrorCode::InvalidSnapshot, Some(item)));
                    }
                    input.contents
                }
                SnapshotShape::OrientedBox { bounds } => {
                    validate_hull(*bounds, item)?;
                    input.contents
                }
                SnapshotShape::Physics(shape) => {
                    let counts = shape.counts();
                    convex_count = convex_count
                        .checked_add(counts.0)
                        .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
                    vertex_count = vertex_count
                        .checked_add(counts.1)
                        .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
                    triangle_count = triangle_count
                        .checked_add(counts.2)
                        .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
                    if convex_count > limits.max_convexes
                        || vertex_count > limits.max_vertices
                        || triangle_count > limits.max_triangles
                    {
                        return Err(error(ErrorCode::Limit, Some(item)));
                    }
                    shape.contents()
                }
            };
            objects.push(SnapshotRecord {
                identity: input.identity,
                role: input.role,
                enabled: input.enabled,
                transform: input.transform,
                linear_velocity: input.linear_velocity,
                angular_velocity: input.angular_velocity,
                collision_group: input.collision_group,
                contents,
                surface_flags: input.surface_flags,
                shape: input.shape,
            });
        }
        Ok(Self {
            world: world.identity,
            identity,
            objects,
            limits,
        })
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn world_identity(&self) -> [u8; 32] {
        self.world
    }

    pub fn records(&self) -> &[SnapshotRecord] {
        &self.objects
    }

    pub fn object_transform(&self, identity: u64) -> Option<Transform> {
        self.object(identity).map(|object| object.transform)
    }

    pub fn object_velocity(&self, identity: u64) -> Option<([f32; 3], [f32; 3])> {
        self.object(identity)
            .map(|object| (object.linear_velocity, object.angular_velocity))
    }

    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut output = BoundedBytes::new(self.limits.max_snapshot_bytes);
        output.bytes(b"CSNP")?;
        output.u32(SNAPSHOT_VERSION)?;
        output.bytes(&self.world)?;
        output.u64(self.identity)?;
        output
            .u32(u32::try_from(self.objects.len()).map_err(|_| error(ErrorCode::Limit, None))?)?;
        for object in &self.objects {
            output.u64(object.identity)?;
            output.u8(match object.role {
                ObjectRole::Entity => 0,
                ObjectRole::StaticProp => 1,
            })?;
            output.u8(u8::from(object.enabled))?;
            output.u16(object.surface_flags)?;
            output.i32(object.collision_group)?;
            output.u32(object.contents)?;
            for value in object
                .transform
                .origin
                .into_iter()
                .chain(object.transform.angles)
                .chain(object.linear_velocity)
                .chain(object.angular_velocity)
            {
                output.f32(value)?;
            }
            match &object.shape {
                SnapshotShape::BrushModel { model } => {
                    output.u8(0)?;
                    output.u64(*model as u64)?;
                }
                SnapshotShape::BoundingBox { bounds } => {
                    output.u8(1)?;
                    output.hull(*bounds)?;
                }
                SnapshotShape::OrientedBox { bounds } => {
                    output.u8(2)?;
                    output.hull(*bounds)?;
                }
                SnapshotShape::Physics(shape) => {
                    output.u8(3)?;
                    output.u64(shape.identity)?;
                    output.u32(
                        u32::try_from(shape.convexes.len())
                            .map_err(|_| error(ErrorCode::Limit, None))?,
                    )?;
                }
            }
        }
        Ok(output.finish())
    }

    fn object(&self, identity: u64) -> Option<&SnapshotRecord> {
        self.objects
            .iter()
            .find(|object| object.identity == identity)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceScope {
    Everything,
    WorldOnly,
    EntitiesOnly,
    EverythingFilterProps,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotTraceRequest<'a> {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
    pub scope: TraceScope,
    pub ignored: &'a [u64],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotRayRequest<'a> {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub mask: u32,
    pub scope: TraceScope,
    pub ignored: &'a [u64],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectTraceRequest {
    pub identity: u64,
    pub transform: Transform,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectOverlapRequest {
    pub identity: u64,
    pub transform: Transform,
    pub position: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub identity: u64,
    pub role: ObjectRole,
    pub collision_group: i32,
    pub contents: u32,
}

impl World {
    pub fn trace_snapshot_hull(
        &self,
        snapshot: &Snapshot,
        request: SnapshotTraceRequest<'_>,
        should_hit: impl Fn(Candidate) -> bool,
    ) -> Result<Trace, Error> {
        if snapshot.world != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        if request.ignored.len() > snapshot.limits.max_ignored_identities {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut world_trace = if request.scope == TraceScope::EntitiesOnly {
            miss(request.start, request.end)
        } else {
            self.trace_hull(request.start, request.end, request.hull, request.mask)?
        };
        world_trace.world = self.identity;
        world_trace.snapshot = Some(snapshot.identity);
        if world_trace.start_solid || request.scope == TraceScope::WorldOnly {
            return Ok(world_trace);
        }
        let world_fraction = world_trace.fraction;
        let dynamic_end = world_trace.end;
        let mut object_trace = miss(request.start, dynamic_end);
        object_trace.world = self.identity;
        let mut visits = 0_usize;
        for object in &snapshot.objects {
            if !object.enabled
                || request.scope == TraceScope::EntitiesOnly
                    && object.role == ObjectRole::StaticProp
                || request.ignored.contains(&object.identity)
                || object.contents & request.mask == 0
            {
                continue;
            }
            let candidate = Candidate {
                identity: object.identity,
                role: object.role,
                collision_group: object.collision_group,
                contents: object.contents,
            };
            if (object.role != ObjectRole::StaticProp
                || request.scope == TraceScope::EverythingFilterProps)
                && !should_hit(candidate)
            {
                continue;
            }
            visits += 1;
            if visits > snapshot.limits.max_candidate_visits {
                return Err(error(ErrorCode::Limit, Some(visits)));
            }
            let candidate = self.trace_object(
                object,
                ObjectTraceRequest {
                    identity: object.identity,
                    transform: object.transform,
                    start: request.start,
                    end: dynamic_end,
                    hull: request.hull,
                    mask: request.mask,
                },
                snapshot.limits,
            )?;
            merge(&mut object_trace, candidate);
            if object_trace.all_solid {
                break;
            }
        }
        if object_trace.hit.is_none() && !object_trace.start_solid && !object_trace.all_solid {
            return Ok(world_trace);
        }
        object_trace.fraction *= world_fraction;
        object_trace.fraction_left_solid *= world_fraction;
        object_trace.end = interpolate(request.start, request.end, object_trace.fraction);
        object_trace.snapshot = Some(snapshot.identity);
        Ok(object_trace)
    }

    pub fn trace_snapshot_ray(
        &self,
        snapshot: &Snapshot,
        request: SnapshotRayRequest<'_>,
        should_hit: impl Fn(Candidate) -> bool,
    ) -> Result<Trace, Error> {
        self.trace_snapshot_hull(
            snapshot,
            SnapshotTraceRequest {
                start: request.start,
                end: request.end,
                hull: Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                mask: request.mask,
                scope: request.scope,
                ignored: request.ignored,
            },
            should_hit,
        )
    }

    pub fn trace_object_hull_at(
        &self,
        snapshot: &Snapshot,
        request: ObjectTraceRequest,
    ) -> Result<Trace, Error> {
        if snapshot.world != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let object = snapshot
            .object(request.identity)
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
        let mut trace = self.trace_object(object, request, snapshot.limits)?;
        trace.world = self.identity;
        trace.snapshot = Some(snapshot.identity);
        Ok(trace)
    }

    pub fn overlaps_object_hull_at(
        &self,
        snapshot: &Snapshot,
        request: ObjectOverlapRequest,
    ) -> Result<bool, Error> {
        self.trace_object_hull_at(
            snapshot,
            ObjectTraceRequest {
                identity: request.identity,
                transform: request.transform,
                start: request.position,
                end: request.position,
                hull: request.hull,
                mask: request.mask,
            },
        )
        .map(|trace| trace.start_solid)
    }

    fn trace_object(
        &self,
        object: &SnapshotRecord,
        request: ObjectTraceRequest,
        limits: SnapshotLimits,
    ) -> Result<Trace, Error> {
        let mut trace = match &object.shape {
            SnapshotShape::BrushModel { model } => {
                self.trace_model_hull(*model, request, object.identity, object.role)?
            }
            SnapshotShape::BoundingBox { bounds } => {
                let mut trace = trace_box(
                    request.start,
                    request.end,
                    request.hull,
                    add(request.transform.origin, bounds.mins),
                    add(request.transform.origin, bounds.maxs),
                    object.contents,
                )?;
                set_box_feature(&mut trace);
                trace
            }
            SnapshotShape::OrientedBox { bounds } => {
                let basis = request.transform.basis()?;
                let point = point_hull(request.hull);
                if point {
                    let local_start = basis.inverse_point(request.start);
                    let local_end = add(
                        local_start,
                        basis.inverse_vector(sub(request.end, request.start)),
                    );
                    let mut trace = trace_box(
                        local_start,
                        local_end,
                        request.hull,
                        bounds.mins,
                        bounds.maxs,
                        object.contents,
                    )?;
                    if let Some(plane) = trace.plane.as_mut() {
                        plane.normal = basis.vector(plane.normal);
                    }
                    trace.end = interpolate(request.start, request.end, trace.fraction);
                    set_box_feature(&mut trace);
                    trace
                } else {
                    let vertices = box_vertices(*bounds);
                    let faces = box_faces();
                    let edges = box_edges();
                    let mut trace = trace_convex(
                        request.start,
                        request.end,
                        request.hull,
                        &vertices,
                        &faces,
                        &edges,
                        basis,
                        object.contents,
                        limits,
                    )?;
                    set_box_feature(&mut trace);
                    trace
                }
            }
            SnapshotShape::Physics(shape) => {
                let basis = request.transform.basis()?;
                let mut output = miss(request.start, request.end);
                for convex in &shape.convexes {
                    if convex.contents & request.mask == 0 {
                        continue;
                    }
                    let mut candidate = trace_convex(
                        request.start,
                        request.end,
                        request.hull,
                        &convex.vertices,
                        &convex.faces,
                        &convex.edges,
                        basis,
                        convex.contents,
                        limits,
                    )?;
                    if candidate.hit.is_some() || candidate.start_solid || candidate.all_solid {
                        let triangle = candidate.hit.and_then(|hit| match hit {
                            Hit::Object {
                                feature: Feature::Convex { triangle, .. },
                                ..
                            } => triangle,
                            _ => None,
                        });
                        candidate.hit = Some(Hit::Object {
                            identity: object.identity,
                            role: object.role,
                            feature: Feature::Convex {
                                solid: convex.solid,
                                convex: convex.convex,
                                triangle,
                            },
                        });
                    }
                    merge(&mut output, candidate);
                }
                output
            }
        };
        if (trace.hit.is_some() || trace.start_solid || trace.all_solid)
            && !matches!(&object.shape, SnapshotShape::BrushModel { .. })
        {
            trace.surface_flags = object.surface_flags;
            let feature = match trace.hit {
                Some(Hit::Object { feature, .. }) => feature,
                _ => Feature::Box,
            };
            trace.hit = Some(Hit::Object {
                identity: object.identity,
                role: object.role,
                feature,
            });
        }
        trace.world = self.identity;
        Ok(trace)
    }
}

fn trace_box(
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    mins: [f32; 3],
    maxs: [f32; 3],
    contents: u32,
) -> Result<Trace, Error> {
    validate_hull(hull, 0)?;
    if start
        .into_iter()
        .chain(end)
        .chain(mins)
        .chain(maxs)
        .any(|value| !value.is_finite())
        || mins.into_iter().zip(maxs).any(|(min, max)| min > max)
    {
        return Err(error(ErrorCode::InvalidSnapshot, None));
    }
    let center = scale(add(hull.mins, hull.maxs), 0.5);
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    let ray_start = add(start, center);
    let ray_end = add(end, center);
    let expanded_mins = sub(mins, extents);
    let expanded_maxs = add(maxs, extents);
    let axes = [
        IntervalAxis::new([-1.0, 0.0, 0.0], -expanded_mins[0], None),
        IntervalAxis::new([0.0, -1.0, 0.0], -expanded_mins[1], None),
        IntervalAxis::new([0.0, 0.0, -1.0], -expanded_mins[2], None),
        IntervalAxis::new([1.0, 0.0, 0.0], expanded_maxs[0], None),
        IntervalAxis::new([0.0, 1.0, 0.0], expanded_maxs[1], None),
        IntervalAxis::new([0.0, 0.0, 1.0], expanded_maxs[2], None),
    ];
    clip_axes(
        start,
        end,
        ray_start,
        ray_end,
        &axes,
        contents,
        point_hull(hull),
    )
}

#[allow(clippy::too_many_arguments)]
fn trace_convex(
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    local_vertices: &[[f32; 3]],
    faces: &[Face],
    edges: &[[f32; 3]],
    basis: Basis,
    contents: u32,
    limits: SnapshotLimits,
) -> Result<Trace, Error> {
    validate_hull(hull, 0)?;
    let center = scale(add(hull.mins, hull.maxs), 0.5);
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    let ray_start = add(start, center);
    let ray_end = add(end, center);
    let vertices = local_vertices
        .iter()
        .copied()
        .map(|vertex| basis.point(vertex))
        .collect::<Vec<_>>();
    let mut directions = faces
        .iter()
        .map(|face| (basis.vector(face.normal), Some(face.triangle)))
        .collect::<Vec<_>>();
    if !point_hull(hull) {
        directions.extend([
            ([1.0, 0.0, 0.0], None),
            ([0.0, 1.0, 0.0], None),
            ([0.0, 0.0, 1.0], None),
        ]);
        for edge in edges.iter().copied().map(|edge| basis.vector(edge)) {
            directions.extend([
                (cross(edge, [1.0, 0.0, 0.0]), None),
                (cross(edge, [0.0, 1.0, 0.0]), None),
                (cross(edge, [0.0, 0.0, 1.0]), None),
            ]);
        }
    }
    if directions.len() > limits.max_axes_per_convex {
        return Err(error(ErrorCode::Limit, None));
    }
    let mut axes = Vec::with_capacity(directions.len() * 2);
    for (direction, triangle) in directions {
        let length = length(direction);
        if length <= f32::EPSILON {
            continue;
        }
        let normal = scale(direction, 1.0 / length);
        let radius = dot_abs(normal, extents);
        let (minimum, maximum) = vertices.iter().copied().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(minimum, maximum), vertex| {
                let projection = dot(vertex, normal);
                (minimum.min(projection), maximum.max(projection))
            },
        );
        axes.push(IntervalAxis::new(
            scale(normal, -1.0),
            -(minimum - radius),
            triangle,
        ));
        axes.push(IntervalAxis::new(normal, maximum + radius, triangle));
    }
    let mut trace = clip_axes(
        start,
        end,
        ray_start,
        ray_end,
        &axes,
        contents,
        point_hull(hull),
    )?;
    if trace.hit.is_some() || trace.start_solid || trace.all_solid {
        let triangle = trace.hit.and_then(|hit| match hit {
            Hit::Object {
                feature: Feature::Convex { triangle, .. },
                ..
            } => triangle,
            _ => None,
        });
        trace.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Convex {
                solid: 0,
                convex: 0,
                triangle,
            },
        });
    }
    Ok(trace)
}

#[derive(Clone, Copy)]
struct IntervalAxis {
    normal: [f32; 3],
    distance: f32,
    triangle: Option<usize>,
}
impl IntervalAxis {
    const fn new(normal: [f32; 3], distance: f32, triangle: Option<usize>) -> Self {
        Self {
            normal,
            distance,
            triangle,
        }
    }
}

fn clip_axes(
    requested_start: [f32; 3],
    requested_end: [f32; 3],
    start: [f32; 3],
    end: [f32; 3],
    axes: &[IntervalAxis],
    contents: u32,
    point: bool,
) -> Result<Trace, Error> {
    let mut output = miss(requested_start, requested_end);
    let mut enter = -1.0_f32;
    let mut leave = 1.0_f32;
    let mut starts_out = false;
    let mut gets_out = false;
    let mut selected = None;
    for axis in axes {
        let d1 = dot(start, axis.normal) - axis.distance;
        let d2 = dot(end, axis.normal) - axis.distance;
        if d1 > 0.0 {
            starts_out = true;
            if d2 > 0.0 {
                return Ok(output);
            }
        } else {
            if d2 <= 0.0 {
                continue;
            }
            gets_out = true;
        }
        if d1 > d2 {
            let fraction = (d1 - DIST_EPSILON).max(0.0) / (d1 - d2);
            if fraction > enter {
                enter = fraction;
                selected = Some(*axis);
            }
        } else {
            leave = leave.min((d1 + DIST_EPSILON) / (d1 - d2));
        }
    }
    if !starts_out {
        output.start_solid = true;
        output.contents = contents;
        output.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Box,
        });
        if !gets_out {
            output.all_solid = true;
            output.fraction = 0.0;
            output.fraction_left_solid = 1.0;
            output.end = requested_start;
        } else if point && leave != 1.0 {
            output.fraction_left_solid = leave;
        }
        return Ok(output);
    }
    if enter < leave && enter > -1.0 && enter < 1.0 {
        let axis = selected.ok_or_else(|| error(ErrorCode::InvalidSnapshot, None))?;
        output.fraction = enter.max(0.0);
        output.end = interpolate(requested_start, requested_end, output.fraction);
        output.contents = contents;
        output.plane = Some(Plane {
            normal: axis.normal,
            distance: dot(output.end, axis.normal),
            kind: 3,
        });
        output.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Convex {
                solid: 0,
                convex: 0,
                triangle: axis.triangle,
            },
        });
    }
    Ok(output)
}

fn merge(output: &mut Trace, candidate: Trace) {
    if candidate.all_solid || candidate.start_solid || candidate.fraction < output.fraction {
        if output.start_solid {
            let left = output.fraction_left_solid;
            let snapshot = output.snapshot;
            *output = candidate;
            output.start_solid = true;
            output.fraction_left_solid = left.max(output.fraction_left_solid);
            output.snapshot = snapshot.or(output.snapshot);
        } else {
            let snapshot = output.snapshot;
            *output = candidate;
            output.snapshot = snapshot.or(output.snapshot);
        }
    }
}

fn set_box_feature(trace: &mut Trace) {
    if trace.hit.is_some() || trace.start_solid || trace.all_solid {
        trace.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Box,
        });
    }
}

fn miss(_start: [f32; 3], end: [f32; 3]) -> Trace {
    Trace {
        world: [0; 32],
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
    }
}

fn validate_hull(hull: Hull, item: usize) -> Result<(), Error> {
    if hull
        .mins
        .into_iter()
        .chain(hull.maxs)
        .any(|value| !value.is_finite())
        || hull
            .mins
            .into_iter()
            .zip(hull.maxs)
            .any(|(min, max)| min > max)
    {
        Err(error(ErrorCode::InvalidHull, Some(item)))
    } else {
        Ok(())
    }
}

fn point_hull(hull: Hull) -> bool {
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    dot(extents, extents) < 1.0e-6
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn quaternion_from_angles(angles: [f32; 3]) -> [f64; 4] {
    let (sp, cp) = (f64::from(angles[0]).to_radians() * 0.5).sin_cos();
    let (sy, cy) = (f64::from(angles[1]).to_radians() * 0.5).sin_cos();
    let (sr, cr) = (f64::from(angles[2]).to_radians() * 0.5).sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

fn quaternion_multiply(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn quaternion_inverse(value: [f64; 4]) -> [f64; 4] {
    [-value[0], -value[1], -value[2], value[3]]
}

fn quaternion_rotate(rotation: [f64; 4], value: [f32; 3]) -> [f32; 3] {
    let vector = [
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
        0.0,
    ];
    let result = quaternion_multiply(
        quaternion_multiply(rotation, vector),
        quaternion_inverse(rotation),
    );
    [result[0] as f32, result[1] as f32, result[2] as f32]
}

fn angles_from_quaternion(value: [f64; 4]) -> [f32; 3] {
    let roll = (2.0 * (value[3] * value[0] + value[1] * value[2]))
        .atan2(1.0 - 2.0 * (value[0] * value[0] + value[1] * value[1]));
    let sine_pitch = 2.0 * (value[3] * value[1] - value[2] * value[0]);
    let pitch = if sine_pitch.abs() >= 1.0 {
        std::f64::consts::FRAC_PI_2.copysign(sine_pitch)
    } else {
        sine_pitch.asin()
    };
    let yaw = (2.0 * (value[3] * value[2] + value[0] * value[1]))
        .atan2(1.0 - 2.0 * (value[1] * value[1] + value[2] * value[2]));
    [
        pitch.to_degrees() as f32,
        yaw.to_degrees() as f32,
        roll.to_degrees() as f32,
    ]
}

fn length(value: [f32; 3]) -> f32 {
    dot(value, value).sqrt()
}

fn dot_abs(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].abs() * b[0] + a[1].abs() * b[1] + a[2].abs() * b[2]
}

fn box_vertices(bounds: Hull) -> Vec<[f32; 3]> {
    let mut vertices = Vec::with_capacity(8);
    for z in [bounds.mins[2], bounds.maxs[2]] {
        for y in [bounds.mins[1], bounds.maxs[1]] {
            for x in [bounds.mins[0], bounds.maxs[0]] {
                vertices.push([x, y, z]);
            }
        }
    }
    vertices
}

fn box_faces() -> Vec<Face> {
    [
        ([-1.0, 0.0, 0.0], 0),
        ([1.0, 0.0, 0.0], 1),
        ([0.0, -1.0, 0.0], 2),
        ([0.0, 1.0, 0.0], 3),
        ([0.0, 0.0, -1.0], 4),
        ([0.0, 0.0, 1.0], 5),
    ]
    .into_iter()
    .map(|(normal, triangle)| Face { normal, triangle })
    .collect()
}

fn box_edges() -> Vec<[f32; 3]> {
    vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

struct BoundedBytes {
    bytes: Vec<u8>,
    maximum: usize,
}
impl BoundedBytes {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), Error> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(error(ErrorCode::Limit, None));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), Error> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn f32(&mut self, value: f32) -> Result<(), Error> {
        self.u32(value.to_bits())
    }

    fn hull(&mut self, hull: Hull) -> Result<(), Error> {
        for value in hull.mins.into_iter().chain(hull.maxs) {
            self.f32(value)?;
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
