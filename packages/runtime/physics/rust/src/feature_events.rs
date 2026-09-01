use crate::{
    BodyMotionPhase, ContinuousError, ContinuousRoot, ContinuousTraversal, EdgeId, FeatureTopology,
    ObjectFrame, ProjectionKnot, TopologyError,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FeatureMotion {
    Stationary(ProjectionKnot),
    Moving {
        phase: BodyMotionPhase,
        frame: ObjectFrame,
        cache_time: f64,
        cached: ProjectionKnot,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureEventKind {
    Collision,
    SectorTransition { adjacent_edge: EdgeId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureEvent {
    pub time: f64,
    pub kind: FeatureEventKind,
    pub vertex_edge: EdgeId,
    pub face_edge: EdgeId,
    pub root: ContinuousRoot,
}

#[derive(Clone, Copy, Debug)]
pub struct VertexFaceSectorQuery<'a> {
    pub moving: &'a FeatureTopology,
    pub vertex: EdgeId,
    pub face: EdgeId,
    pub plane: EventPlane,
    pub motions: [FeatureMotion; 2],
    pub start: f64,
    pub end: f64,
    pub threshold: f64,
    pub maximum_deviation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventPlane {
    pub origin: [f64; 3],
    pub normal: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexEdgeGeometry {
    pub vertex: [f64; 3],
    pub edge: EventEdge,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexPairGeometry {
    pub points: [[f64; 3]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexPairCollision {
    pub geometry: VertexPairGeometry,
    pub initial_direction: [f64; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VertexPairEventKind {
    Collision,
    EdgeTransition { side: usize, adjacent_edge: EdgeId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexPairEvent {
    pub time: f64,
    pub kind: VertexPairEventKind,
    pub vertices: [EdgeId; 2],
    pub root: ContinuousRoot,
}

#[derive(Clone, Copy, Debug)]
pub struct VertexPairEventQuery<'a> {
    pub topologies: [&'a FeatureTopology; 2],
    pub vertices: [EdgeId; 2],
    pub motions: [FeatureMotion; 2],
    pub motion_bounds: [crate::CollisionMotion; 2],
    pub radii: [f32; 2],
    pub separating_normal: [f32; 3],
    pub start: f64,
    pub end: f64,
    pub initial_distance: f32,
    pub extra_radius: f32,
    pub tolerances: crate::ContactTolerances,
    pub maximum_collision_deviation: f64,
    pub worst_case_speed: f64,
}

impl VertexPairEventQuery<'_> {
    pub fn next(self) -> Result<Option<VertexPairEvent>, FeatureEventError> {
        if [
            self.start,
            self.end,
            self.maximum_collision_deviation,
            self.worst_case_speed,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || [
                self.initial_distance,
                self.extra_radius,
                self.radii[0],
                self.radii[1],
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(FeatureEventError::NonFinite);
        }
        if self.start >= self.end {
            return Err(FeatureEventError::InvalidInterval);
        }
        if self.maximum_collision_deviation <= 0.0
            || self.worst_case_speed <= 0.0
            || self.radii.iter().any(|radius| *radius <= 0.0)
        {
            return Err(FeatureEventError::InvalidDeviation);
        }
        for motion in self.motion_bounds {
            motion.validate()?;
        }
        for motion in self.motions {
            motion.validate_interval(self.start, self.end)?;
        }
        let geometry = VertexPairGeometry::from_features(self.topologies, self.vertices)?;
        let collision = geometry.collision(self.separating_normal)?;
        let mut matrices = MotionSampleCache::new(self.motions, self.start)?;
        let mut sample = |request| matrices.sample(request);
        let root = find_event_root(
            self.start,
            self.end,
            f64::from(self.tolerances.collision_distance) + f64::from(self.extra_radius),
            f64::from(self.tolerances.real_surface + self.extra_radius * 0.5_f32),
            self.maximum_collision_deviation,
            false,
            |time| collision.distance(sample(time)?),
        )?;
        let event = |root: ContinuousRoot, kind| VertexPairEvent {
            time: root.time,
            kind,
            vertices: self.vertices,
            root,
        };
        let mut best = root.map(|root| event(root, VertexPairEventKind::Collision));
        let horizon = best.map_or(self.end, |event| event.time);
        let maximum_distance = self.worst_case_speed * f64::from((horizon - self.start) as f32)
            + f64::from(self.initial_distance);
        let speeds = std::array::from_fn::<_, 2, _>(|side| {
            let point = geometry.points[side].map(|value| value as f32);
            let squared = (point[0] * point[0] + point[1] * point[1]) + point[2] * point[2];
            f64::from(self.motion_bounds[side].linear_speed)
                + f64::from(squared).sqrt()
                    * f64::from(self.motion_bounds[side].rotation.angular_speed)
        });
        let speed_sum = speeds[1] + speeds[0];
        let collision = f64::from(self.tolerances.collision_distance);
        let safe_sector = (collision * collision).min(maximum_distance * maximum_distance) * -0.5
            / f64::from(self.radii[1].max(self.radii[0]));
        for point_side in 0..2 {
            let edge_side = 1 - point_side;
            let maximum_deviation = maximum_distance
                * f64::from(self.motion_bounds[edge_side].rotation.angular_speed)
                + speed_sum;
            let selected = VertexPairGeometry {
                points: [geometry.points[point_side], geometry.points[edge_side]],
            };
            for adjacent in
                SectorDirection::outgoing(self.topologies[edge_side], self.vertices[edge_side])?
            {
                let squared = dot(adjacent.difference, adjacent.difference);
                let threshold = (squared * safe_sector) * adjacent.inverse_length;
                let end = best.map_or(self.end, |event| event.time);
                if let Some(root) = find_event_root(
                    self.start,
                    end,
                    threshold,
                    threshold,
                    maximum_deviation,
                    true,
                    |time| {
                        let poses = sample(time)?;
                        selected.sector(adjacent.direction, [poses[point_side], poses[edge_side]])
                    },
                )? {
                    best = Some(event(
                        root,
                        VertexPairEventKind::EdgeTransition {
                            side: edge_side,
                            adjacent_edge: adjacent.edge,
                        },
                    ));
                }
            }
        }
        Ok(best)
    }
}

impl VertexPairGeometry {
    pub fn from_features(
        topologies: [&FeatureTopology; 2],
        vertices: [EdgeId; 2],
    ) -> Result<Self, FeatureEventError> {
        let mut points = [[0.0; 3]; 2];
        for side in 0..2 {
            points[side] = topologies[side].points()
                [topologies[side].edge(vertices[side])?.start as usize]
                .map(f64::from);
        }
        Ok(Self { points })
    }

    pub fn collision(
        self,
        separating_normal: [f32; 3],
    ) -> Result<VertexPairCollision, FeatureEventError> {
        if separating_normal.iter().any(|value| !value.is_finite()) {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(VertexPairCollision {
            geometry: self,
            initial_direction: separating_normal.map(|value| f64::from(-value)),
        })
    }

    pub fn sector(
        self,
        edge_direction: [f64; 3],
        poses: [ProjectionKnot; 2],
    ) -> Result<f64, FeatureEventError> {
        let delta = sub(
            world_point(poses[1], self.points[1]),
            world_point(poses[0], self.points[0]),
        );
        let direction = world_vector(poses[1].orientation, edge_direction);
        let value = dot(delta, direction);
        if !value.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(value)
    }
}

impl VertexPairCollision {
    pub fn distance(self, poses: [ProjectionKnot; 2]) -> Result<f64, FeatureEventError> {
        let delta = sub(
            world_point(poses[1], self.geometry.points[1]),
            world_point(poses[0], self.geometry.points[0]),
        );
        let squared = dot(delta, delta);
        let projected = dot(delta, self.initial_direction) * f64::from(1.2_f32);
        if !squared.is_finite() || !projected.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(if projected.abs() * projected >= squared {
            squared.sqrt()
        } else {
            projected
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexEdgeCollision {
    pub geometry: VertexEdgeGeometry,
    pub virtual_normal: [f64; 3],
    pub plane_offset: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VertexEdgeEventKind {
    Collision,
    FaceTransition { face: EdgeId },
    EdgeTransition { adjacent_edge: EdgeId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexEdgeEvent {
    pub time: f64,
    pub kind: VertexEdgeEventKind,
    pub vertex: EdgeId,
    pub edge: EdgeId,
    pub root: ContinuousRoot,
}

#[derive(Clone, Copy, Debug)]
pub struct VertexEdgeEventQuery<'a> {
    pub topologies: [&'a FeatureTopology; 2],
    pub vertex: EdgeId,
    pub edge: EdgeId,
    pub motions: [FeatureMotion; 2],
    pub motion_bounds: [crate::CollisionMotion; 2],
    pub start: f64,
    pub end: f64,
    pub initial_distance: f32,
    pub extra_radius: f32,
    pub opposing_inverse_diameter: f32,
    pub tolerances: crate::ContactTolerances,
    pub maximum_collision_deviation: f64,
    pub worst_case_speed: f64,
}

impl VertexEdgeEventQuery<'_> {
    pub fn next(self) -> Result<Option<VertexEdgeEvent>, FeatureEventError> {
        if [
            self.start,
            self.end,
            self.maximum_collision_deviation,
            self.worst_case_speed,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || [
                self.initial_distance,
                self.extra_radius,
                self.opposing_inverse_diameter,
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(FeatureEventError::NonFinite);
        }
        if self.start >= self.end {
            return Err(FeatureEventError::InvalidInterval);
        }
        if self.maximum_collision_deviation <= 0.0
            || self.worst_case_speed <= 0.0
            || self.opposing_inverse_diameter <= 0.0
        {
            return Err(FeatureEventError::InvalidDeviation);
        }
        for motion in self.motion_bounds {
            motion.validate()?;
        }
        for motion in self.motions {
            motion.validate_interval(self.start, self.end)?;
        }
        let mut matrices = MotionSampleCache::new(self.motions, self.start)?;
        let mut sample = |request| matrices.sample(request);
        let geometry =
            VertexEdgeGeometry::from_features(self.topologies, [self.vertex, self.edge])?;
        let collision = geometry.collision(
            sample(crate::ContinuousSample::Raster {
                time: self.start,
                index: 0,
            })?,
            self.tolerances.collision_distance,
            self.extra_radius,
        )?;
        let root = find_event_root(
            self.start,
            self.end,
            f64::from(self.tolerances.collision_distance) + f64::from(self.extra_radius),
            f64::from(self.tolerances.real_surface + self.extra_radius * 0.9_f32),
            self.maximum_collision_deviation,
            false,
            |time| collision.distance(sample(time)?),
        )?;
        let event = |root: ContinuousRoot, kind| VertexEdgeEvent {
            time: root.time,
            kind,
            vertex: self.vertex,
            edge: self.edge,
            root,
        };
        let mut best = root.map(|root| event(root, VertexEdgeEventKind::Collision));
        for face in [self.edge, self.topologies[1].opposite(self.edge)?] {
            let topology = self.topologies[1];
            let origin = topology.points()[topology.edge(face)?.start as usize];
            let next = topology.points()[topology.edge(topology.next(face)?)?.start as usize];
            let previous =
                topology.points()[topology.edge(topology.previous(face)?)?.start as usize];
            let along = std::array::from_fn(|axis| f64::from(next[axis] - origin[axis]));
            let face_normal = cross(
                sub(next.map(f64::from), origin.map(f64::from)),
                sub(previous.map(f64::from), origin.map(f64::from)),
            );
            let plane = EventPlane {
                origin: origin.map(f64::from),
                normal: normalize_event_perpendicular(cross(along, face_normal))?,
            };
            let end = best.map_or(self.end, |event| event.time);
            let target = -f64::from(self.tolerances.feature_change_distance);
            if let Some(root) = find_event_root(
                self.start,
                end,
                target,
                target,
                self.worst_case_speed,
                true,
                |time| {
                    let [point, edge] = sample(time)?;
                    plane.distance(geometry.vertex, point, edge)
                },
            )? {
                best = Some(event(root, VertexEdgeEventKind::FaceTransition { face }));
            }
        }
        let [point_motion, edge_motion] = self.motion_bounds;
        let point = geometry.vertex.map(|value| value as f32);
        let squared = (point[0] * point[0] + point[1] * point[1]) + point[2] * point[2];
        let point_speed = f64::from(point_motion.linear_speed)
            + f64::from(squared).sqrt() * f64::from(point_motion.rotation.angular_speed);
        let edge_speed = f64::from(
            edge_motion.rotation.surface_speed * edge_motion.rotation.angular_speed
                + edge_motion.linear_speed,
        );
        let horizon = best.map_or(self.end, |event| event.time);
        let distance = (f64::from(self.initial_distance)
            - self.worst_case_speed * f64::from((horizon - self.start) as f32))
        .max(1.0e-8);
        let maximum_deviation = (edge_speed + point_speed) / distance
            + f64::from(point_motion.rotation.angular_speed + edge_motion.rotation.angular_speed);
        let threshold = (f64::from(self.tolerances.collision_distance)
            .min(f64::from(self.initial_distance))
            * f64::from(self.opposing_inverse_diameter + self.opposing_inverse_diameter))
            * f64::from(-0.3_f32);
        let topology = self.topologies[0];
        let mut adjacent = topology.opposite(topology.previous(self.vertex)?)?;
        for _ in 0..topology.edges().len() {
            let direction = EventEdge::from_edge(topology, adjacent)?.direction;
            let end = best.map_or(self.end, |event| event.time);
            if let Some(root) = find_event_root(
                self.start,
                end,
                threshold,
                threshold,
                maximum_deviation,
                true,
                |time| geometry.sector(direction, sample(time)?),
            )? {
                best = Some(event(
                    root,
                    VertexEdgeEventKind::EdgeTransition {
                        adjacent_edge: adjacent,
                    },
                ));
            }
            if adjacent == self.vertex {
                return Ok(best);
            }
            adjacent = topology.opposite(topology.previous(adjacent)?)?;
        }
        Err(FeatureEventError::Topology(TopologyError::OpenFan))
    }
}

impl VertexEdgeGeometry {
    pub fn from_features(
        topologies: [&FeatureTopology; 2],
        features: [EdgeId; 2],
    ) -> Result<Self, FeatureEventError> {
        let vertex =
            topologies[0].points()[topologies[0].edge(features[0])?.start as usize].map(f64::from);
        Ok(Self {
            vertex,
            edge: EventEdge::from_edge(topologies[1], features[1])?,
        })
    }

    pub fn collision(
        self,
        poses: [ProjectionKnot; 2],
        collision_distance: f32,
        extra_radius: f32,
    ) -> Result<VertexEdgeCollision, FeatureEventError> {
        let offset = (f64::from(collision_distance) + f64::from(extra_radius)) * 0.5;
        let delta = sub(
            world_point(poses[1], self.edge.origin),
            world_point(poses[0], self.vertex),
        );
        let perpendicular = cross(
            delta,
            world_vector(poses[1].orientation, self.edge.direction),
        );
        let virtual_normal =
            normalize_event_perpendicular(local_vector(poses[1].orientation, perpendicular))?;
        if !offset.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(VertexEdgeCollision {
            geometry: self,
            virtual_normal,
            plane_offset: offset,
        })
    }

    pub fn sector(
        self,
        adjacent_direction: [f64; 3],
        poses: [ProjectionKnot; 2],
    ) -> Result<f64, FeatureEventError> {
        let point = world_point(poses[0], self.vertex);
        let origin = world_point(poses[1], self.edge.origin);
        let direction = world_vector(poses[1].orientation, self.edge.direction);
        let crosswise = cross(direction, sub(point, origin));
        let perpendicular = cross(crosswise, direction).map(|value| f64::from(value as f32));
        let normal = normalize_event_perpendicular(perpendicular)?;
        let result = dot(
            world_vector(poses[0].orientation, adjacent_direction),
            normal,
        );
        if !result.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(result)
    }
}

impl VertexEdgeCollision {
    pub fn distance(self, poses: [ProjectionKnot; 2]) -> Result<f64, FeatureEventError> {
        let vertex = world_point(poses[0], self.geometry.vertex);
        let origin = world_point(poses[1], self.geometry.edge.origin);
        let direction = world_vector(poses[1].orientation, self.geometry.edge.direction);
        let perpendicular = cross(sub(origin, vertex), direction);
        let length = ((perpendicular[0] * perpendicular[0] + perpendicular[1] * perpendicular[1])
            + perpendicular[2] * perpendicular[2])
            .sqrt();
        let normal = world_vector(poses[1].orientation, self.virtual_normal);
        let virtual_distance = ((perpendicular[0] * normal[0] + perpendicular[1] * normal[1])
            + perpendicular[2] * normal[2])
            + self.plane_offset;
        if !length.is_finite() || !virtual_distance.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(if virtual_distance < length {
            virtual_distance
        } else {
            length
        })
    }
}

fn normalize_event_perpendicular(value: [f64; 3]) -> Result<[f64; 3], FeatureEventError> {
    let squared = (value[0] * value[0] + value[1] * value[1]) + value[2] * value[2];
    if !squared.is_finite() {
        return Err(FeatureEventError::NonFinite);
    }
    if squared < 1.0e-19 {
        return Ok(value);
    }
    let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
    Ok(value.map(|value| value * inverse))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectorDirection {
    pub edge: EdgeId,
    pub difference: [f64; 3],
    pub direction: [f64; 3],
    pub inverse_length: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectorThreshold {
    pub separation: f32,
    pub collision_distance: f32,
    pub extra_radius: f32,
    pub change_distance: f32,
    pub inverse_diameter: f32,
}

impl SectorThreshold {
    pub fn value(self) -> Result<f64, FeatureEventError> {
        if [
            self.separation,
            self.collision_distance,
            self.extra_radius,
            self.change_distance,
            self.inverse_diameter,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(FeatureEventError::NonFinite);
        }
        if self.collision_distance <= 0.0
            || self.extra_radius < 0.0
            || self.change_distance < 0.0
            || self.inverse_diameter < 0.0
        {
            return Err(ContinuousError::InvalidBracket.into());
        }
        let distance = f64::from(self.separation.min(self.collision_distance))
            + f64::from(self.extra_radius * 0.1_f32);
        let scale = -(self.change_distance * self.inverse_diameter);
        let threshold = (distance * f64::from(scale)) / f64::from(self.collision_distance);
        if !threshold.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(threshold)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventEdge {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePairGeometry {
    pub first: EventEdge,
    pub second: EventEdge,
    pub sign: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgePairEventKind {
    Collision,
    Parallel,
    FaceTransition { side: usize, face: EdgeId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePairEvent {
    pub time: f64,
    pub kind: EdgePairEventKind,
    pub edges: [EdgeId; 2],
    pub root: ContinuousRoot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePairEventQuery<'a> {
    pub topologies: [&'a FeatureTopology; 2],
    pub edges: [EdgeId; 2],
    pub motions: [FeatureMotion; 2],
    pub start: f64,
    pub end: f64,
    pub contact_normal: [f32; 3],
    pub collision_distance: f64,
    pub real_distance: f64,
    pub sector_thresholds: [f64; 2],
    pub maximum_collision_deviation: f64,
    pub angular_deviation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexFaceEventQuery<'a> {
    pub moving: &'a FeatureTopology,
    pub fixed: &'a FeatureTopology,
    pub vertex: EdgeId,
    pub face: EdgeId,
    pub moving_motion: FeatureMotion,
    pub fixed_motion: FeatureMotion,
    pub start: f64,
    pub end: f64,
    pub initial_distance: f64,
    pub collision_distance: f64,
    pub real_distance: f64,
    pub sector_threshold: f64,
    pub maximum_collision_deviation: f64,
    pub maximum_angular_deviation: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureEventError {
    NonFinite,
    InvalidInterval,
    InvalidDeviation,
    InvalidMotionPhase,
    OpenVertexFan,
    DegenerateEdge,
    ParallelEdges,
    Topology(TopologyError),
    Continuous(ContinuousError),
    Minimization(crate::FeatureWalkError),
}

impl fmt::Display for FeatureEventError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => output.write_str("feature-event input contains a non-finite value"),
            Self::InvalidInterval => output.write_str("feature-event interval must increase"),
            Self::InvalidDeviation => output.write_str("feature-event deviation must be positive"),
            Self::InvalidMotionPhase => {
                output.write_str("feature-event motion phase cannot sample the requested time")
            }
            Self::OpenVertexFan => {
                output.write_str("feature-event authored vertex fan does not close")
            }
            Self::DegenerateEdge => output.write_str("feature-event authored edge has zero length"),
            Self::ParallelEdges => {
                output.write_str("edge-pair event input requires nonparallel initial edges")
            }
            Self::Topology(error) => error.fmt(output),
            Self::Continuous(error) => error.fmt(output),
            Self::Minimization(error) => error.fmt(output),
        }
    }
}

impl std::error::Error for FeatureEventError {}

impl From<TopologyError> for FeatureEventError {
    fn from(error: TopologyError) -> Self {
        Self::Topology(error)
    }
}
impl From<ContinuousError> for FeatureEventError {
    fn from(error: ContinuousError) -> Self {
        Self::Continuous(error)
    }
}

impl From<crate::FeatureWalkError> for FeatureEventError {
    fn from(error: crate::FeatureWalkError) -> Self {
        Self::Minimization(error)
    }
}

impl FeatureMotion {
    pub(crate) fn validate_interval(self, start: f64, end: f64) -> Result<(), FeatureEventError> {
        match self {
            Self::Stationary(_) => {
                self.sample(start)?;
                self.sample(end)?;
            }
            Self::Moving {
                phase, cache_time, ..
            } => {
                if cache_time != start {
                    return Err(FeatureEventError::InvalidMotionPhase);
                }
                self.sample(start)?;
                phase
                    .sample(start)
                    .map_err(|_| FeatureEventError::InvalidMotionPhase)?;
                phase
                    .sample(end)
                    .map_err(|_| FeatureEventError::InvalidMotionPhase)?;
            }
        }
        Ok(())
    }
    pub fn sample(self, time: f64) -> Result<ProjectionKnot, FeatureEventError> {
        match self {
            Self::Stationary(transform) => {
                if !time.is_finite()
                    || transform
                        .position
                        .iter()
                        .chain(transform.orientation.iter())
                        .any(|value| !value.is_finite())
                {
                    return Err(FeatureEventError::NonFinite);
                }
                Ok(transform)
            }
            Self::Moving {
                phase,
                frame,
                cache_time,
                cached,
            } => {
                if time == cache_time {
                    return Self::Stationary(cached).sample(time);
                }
                let core = phase
                    .search_sample(time)
                    .map_err(|_| FeatureEventError::InvalidMotionPhase)?
                    .1;
                Ok(frame.object_pose(core)?)
            }
        }
    }

    fn projected(self, time: f64) -> Result<ProjectionKnot, FeatureEventError> {
        match self {
            Self::Stationary(_) => self.sample(time),
            Self::Moving { phase, frame, .. } => Ok(frame.object_pose(
                phase
                    .search_sample(time)
                    .map_err(|_| FeatureEventError::InvalidMotionPhase)?
                    .1,
            )?),
        }
    }
}

struct MotionSampleCache {
    motions: [FeatureMotion; 2],
    cells: [[Option<ProjectionKnot>; 21]; 2],
}

impl MotionSampleCache {
    fn new(motions: [FeatureMotion; 2], start: f64) -> Result<Self, FeatureEventError> {
        let mut cells = [[None; 21]; 2];
        for side in 0..2 {
            let initial = motions[side].sample(start)?;
            if matches!(motions[side], FeatureMotion::Stationary(_)) {
                cells[side].fill(Some(initial));
            } else {
                cells[side][0] = Some(initial);
            }
        }
        Ok(Self { motions, cells })
    }

    fn sample(
        &mut self,
        sample: crate::ContinuousSample,
    ) -> Result<[ProjectionKnot; 2], FeatureEventError> {
        let mut poses = [ProjectionKnot {
            position: [0.0; 3],
            orientation: [0.0; 9],
        }; 2];
        for (side, pose) in poses.iter_mut().enumerate() {
            *pose = match sample {
                crate::ContinuousSample::Raster { time, index } => {
                    let slot = self.cells[side]
                        .get_mut(usize::from(index))
                        .ok_or(FeatureEventError::InvalidMotionPhase)?;
                    if let Some(pose) = *slot {
                        pose
                    } else {
                        let pose = self.motions[side].projected(time)?;
                        *slot = Some(pose);
                        pose
                    }
                }
                crate::ContinuousSample::Refinement { time } => {
                    self.motions[side].projected(time)?
                }
            };
        }
        Ok(poses)
    }
}

impl VertexFaceEventQuery<'_> {
    pub fn next(self) -> Result<Option<FeatureEvent>, FeatureEventError> {
        if [
            self.start,
            self.end,
            self.collision_distance,
            self.real_distance,
            self.sector_threshold,
            self.maximum_collision_deviation,
            self.maximum_angular_deviation,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || !self.initial_distance.is_finite()
        {
            return Err(FeatureEventError::NonFinite);
        }
        if self.start >= self.end {
            return Err(FeatureEventError::InvalidInterval);
        }
        if self.maximum_collision_deviation <= 0.0 || self.maximum_angular_deviation <= 0.0 {
            return Err(FeatureEventError::InvalidDeviation);
        }
        let selected = self.moving.edge(self.vertex)?;
        let vertex = self.moving.points()[selected.start as usize].map(f64::from);
        let plane = EventPlane::from_edge(self.fixed, self.face)?;
        self.moving_motion.validate_interval(self.start, self.end)?;
        self.fixed_motion.validate_interval(self.start, self.end)?;

        let mut matrices =
            MotionSampleCache::new([self.moving_motion, self.fixed_motion], self.start)?;
        let mut distance_at = |request| -> Result<f64, FeatureEventError> {
            let [moving, fixed] = matrices.sample(request)?;
            plane.distance(vertex, moving, fixed)
        };
        let mut evaluation_error = None;
        let collision = ContinuousTraversal {
            lower: self.start,
            upper: self.end,
            target: self.collision_distance,
            real_surface: self.real_distance,
            maximum_deviation: self.maximum_collision_deviation,
            initial_value: Some(self.initial_distance),
            maximum_cells: 20,
        }
        .solve(|time| match distance_at(time) {
            Ok(value) => value,
            Err(error) => {
                evaluation_error = Some(error);
                f64::NAN
            }
        });
        if let Some(error) = evaluation_error {
            return Err(error);
        }
        let best = collision?.map(|crossing| FeatureEvent {
            time: crossing.root.time,
            kind: FeatureEventKind::Collision,
            vertex_edge: self.vertex,
            face_edge: self.face,
            root: crossing.root,
        });

        let sector = VertexFaceSectorQuery {
            moving: self.moving,
            vertex: self.vertex,
            face: self.face,
            plane,
            motions: [self.moving_motion, self.fixed_motion],
            start: self.start,
            end: best.map_or(self.end, |event| event.time),
            threshold: self.sector_threshold,
            maximum_deviation: self.maximum_angular_deviation,
        }
        .visit_cached(|_| {}, &mut matrices)?;
        Ok(sector.or(best))
    }
}

impl VertexFaceSectorQuery<'_> {
    pub fn visit(
        self,
        accepted: impl FnMut(FeatureEvent),
    ) -> Result<Option<FeatureEvent>, FeatureEventError> {
        let mut matrices = MotionSampleCache::new(self.motions, self.start)?;
        self.visit_cached(accepted, &mut matrices)
    }

    fn visit_cached(
        self,
        mut accepted: impl FnMut(FeatureEvent),
        matrices: &mut MotionSampleCache,
    ) -> Result<Option<FeatureEvent>, FeatureEventError> {
        if [self.start, self.end, self.threshold, self.maximum_deviation]
            .iter()
            .any(|value| !value.is_finite())
            || self.plane.normal.iter().any(|value| !value.is_finite())
        {
            return Err(FeatureEventError::NonFinite);
        }
        if self.start > self.end {
            return Err(FeatureEventError::InvalidInterval);
        }
        if self.maximum_deviation <= 0.0 {
            return Err(FeatureEventError::InvalidDeviation);
        }
        for motion in self.motions {
            motion.validate_interval(self.start, self.end)?;
        }
        let [initial_moving, initial_fixed] = matrices.sample(crate::ContinuousSample::Raster {
            time: self.start,
            index: 0,
        })?;
        let local_normal = local_vector(
            initial_moving.orientation,
            world_vector(initial_fixed.orientation, self.plane.normal),
        );
        let maximum_change = self.maximum_deviation * f64::from((self.end - self.start) as f32);
        let mut best = None::<FeatureEvent>;
        for adjacent in SectorDirection::outgoing(self.moving, self.vertex)? {
            let direction = adjacent.direction;
            let initial_gradient = dot(local_normal, adjacent.difference) * adjacent.inverse_length;
            let end = best.map_or(self.end, |event| event.time);
            if initial_gradient < maximum_change {
                let root = if initial_gradient <= self.threshold {
                    Some(ContinuousRoot {
                        time: self.start,
                        value: initial_gradient,
                        iterations: 0,
                        exhausted: false,
                    })
                } else if end > self.start {
                    let mut evaluation_error = None;
                    let crossing = ContinuousTraversal {
                        lower: self.start,
                        upper: end,
                        target: self.threshold,
                        real_surface: self.threshold,
                        maximum_deviation: self.maximum_deviation,
                        initial_value: Some(initial_gradient),
                        maximum_cells: 20,
                    }
                    .solve(|request| {
                        let sampled = matrices.sample(request).and_then(|[moving, fixed]| {
                            self.plane.gradient(direction, moving, fixed)
                        });
                        match sampled {
                            Ok(value) => value,
                            Err(error) => {
                                evaluation_error = Some(error);
                                f64::NAN
                            }
                        }
                    });
                    if let Some(error) = evaluation_error {
                        return Err(error);
                    }
                    crossing?.map(|crossing| crossing.root)
                } else {
                    None
                };
                if let Some(root) = root {
                    best = Some(FeatureEvent {
                        time: root.time,
                        kind: FeatureEventKind::SectorTransition {
                            adjacent_edge: adjacent.edge,
                        },
                        vertex_edge: self.vertex,
                        face_edge: self.face,
                        root,
                    });
                    accepted(best.expect("accepted sector event was retained"));
                }
            }
        }
        Ok(best)
    }
}

impl EventPlane {
    pub fn from_edge(topology: &FeatureTopology, edge: EdgeId) -> Result<Self, FeatureEventError> {
        let face = topology.edge(edge)?;
        let origin = topology.points()[face.start as usize].map(f64::from);
        let along = sub(
            topology.points()[topology.edge(topology.next(edge)?)?.start as usize].map(f64::from),
            origin,
        );
        let across = sub(
            topology.points()[topology.edge(topology.previous(edge)?)?.start as usize]
                .map(f64::from),
            origin,
        );
        let raw = [
            along[1] * across[2] - along[2] * across[1],
            along[2] * across[0] - along[0] * across[2],
            along[0] * across[1] - along[1] * across[0],
        ];
        let squared = dot(raw, raw);
        if squared < 1.0e-19 {
            return Err(FeatureEventError::DegenerateEdge);
        }
        let inverse = event_reciprocal_square_root(squared);
        Ok(Self {
            origin,
            normal: raw.map(|value| value * inverse),
        })
    }

    pub fn distance(
        self,
        point: [f64; 3],
        moving: ProjectionKnot,
        fixed: ProjectionKnot,
    ) -> Result<f64, FeatureEventError> {
        let point = world_point(moving, point);
        let plane_point = world_point(fixed, self.origin);
        let normal = world_vector(fixed.orientation, self.normal);
        let distance = dot(sub(point, plane_point), normal);
        if !distance.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(distance)
    }

    pub fn gradient(
        self,
        direction: [f64; 3],
        moving: ProjectionKnot,
        fixed: ProjectionKnot,
    ) -> Result<f64, FeatureEventError> {
        let normal = local_vector(
            moving.orientation,
            world_vector(fixed.orientation, self.normal),
        );
        let gradient = dot(normal, direction);
        if !gradient.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(gradient)
    }
}

impl SectorDirection {
    pub fn from_edge(topology: &FeatureTopology, edge: EdgeId) -> Result<Self, FeatureEventError> {
        let authored = topology.edge(edge)?;
        let start = topology.points()[authored.start as usize];
        let end = topology.points()[authored.end as usize];
        let difference = std::array::from_fn(|axis| f64::from(end[axis] - start[axis]));
        let squared = dot(difference, difference);
        if squared == 0.0 {
            return Err(FeatureEventError::DegenerateEdge);
        }
        let inverse_length =
            f64::from(event_reciprocal_square_root(f64::from(squared as f32)) as f32);
        Ok(Self {
            edge,
            difference,
            direction: difference.map(|value| value * inverse_length),
            inverse_length,
        })
    }

    pub fn outgoing(
        topology: &FeatureTopology,
        first: EdgeId,
    ) -> Result<Vec<Self>, FeatureEventError> {
        let start = topology.edge(first)?.start;
        let mut current = topology.opposite(topology.previous(first)?)?;
        let mut directions = Vec::new();
        for _ in 0..topology.edges().len() {
            if topology.edge(current)?.start != start {
                return Err(FeatureEventError::OpenVertexFan);
            }
            directions.push(Self::from_edge(topology, current)?);
            if current == first {
                return Ok(directions);
            }
            current = topology.opposite(topology.previous(current)?)?;
        }
        Err(FeatureEventError::OpenVertexFan)
    }
}

impl EventEdge {
    pub fn from_edge(
        topology: &FeatureTopology,
        identity: EdgeId,
    ) -> Result<Self, FeatureEventError> {
        let edge = topology.edge(identity)?;
        let first = topology.points()[edge.start as usize];
        let second = topology.points()[edge.end as usize];
        let difference = std::array::from_fn(|axis| f64::from(second[axis] - first[axis]));
        let squared = (difference[0] * difference[0] + difference[1] * difference[1])
            + difference[2] * difference[2];
        if squared < 1.0e-19 {
            return Err(FeatureEventError::DegenerateEdge);
        }
        let inverse = event_reciprocal_square_root(squared);
        Ok(Self {
            origin: first.map(f64::from),
            direction: difference.map(|value| value * inverse),
        })
    }
}

impl EdgePairGeometry {
    pub fn distance(
        self,
        first: ProjectionKnot,
        second: ProjectionKnot,
    ) -> Result<f64, FeatureEventError> {
        let first_direction = world_vector(first.orientation, self.first.direction);
        let second_direction = world_vector(second.orientation, self.second.direction);
        let normal = cross(first_direction, second_direction);
        let first_point = world_point(first, self.first.origin);
        let second_point = world_point(second, self.second.origin);
        let project =
            |point: [f64; 3]| (normal[0] * point[0] + normal[1] * point[1]) + normal[2] * point[2];
        let numerator = project(first_point) - project(second_point);
        let squared =
            ((normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2]) as f32;
        if !squared.is_finite() || !self.sign.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        let inverse = event_reciprocal_square_root(f64::from(squared)) as f32;
        let distance = (numerator * f64::from(inverse)) * self.sign;
        if !distance.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(distance)
    }

    pub fn parallelism(
        self,
        first: ProjectionKnot,
        second: ProjectionKnot,
    ) -> Result<f64, FeatureEventError> {
        let normal = cross(
            world_vector(first.orientation, self.first.direction),
            world_vector(second.orientation, self.second.direction),
        );
        let squared = (normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2];
        if !squared.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        Ok(squared)
    }
}

impl EdgePairEventQuery<'_> {
    pub fn next(self) -> Result<Option<EdgePairEvent>, FeatureEventError> {
        if [
            self.start,
            self.end,
            self.collision_distance,
            self.real_distance,
            self.sector_thresholds[0],
            self.sector_thresholds[1],
            self.maximum_collision_deviation,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || !self.angular_deviation.is_finite()
            || self.contact_normal.iter().any(|value| !value.is_finite())
        {
            return Err(FeatureEventError::NonFinite);
        }
        if self.start >= self.end {
            return Err(FeatureEventError::InvalidInterval);
        }
        if self.maximum_collision_deviation <= 0.0 || self.angular_deviation < 0.0 {
            return Err(FeatureEventError::InvalidDeviation);
        }
        for motion in self.motions {
            motion.validate_interval(self.start, self.end)?;
        }
        let initial = [
            self.motions[0].sample(self.start)?,
            self.motions[1].sample(self.start)?,
        ];
        let first = EventEdge::from_edge(self.topologies[0], self.edges[0])?;
        let second = EventEdge::from_edge(self.topologies[1], self.edges[1])?;
        let normal = cross(
            world_vector(initial[0].orientation, first.direction),
            world_vector(initial[1].orientation, second.direction),
        );
        if normal == [0.0; 3] {
            return Err(FeatureEventError::ParallelEdges);
        }
        let side = dot(normal, self.contact_normal.map(f64::from));
        let geometry = EdgePairGeometry {
            first,
            second,
            sign: if side < 0.0 { -1.0 } else { 1.0 },
        };
        let mut matrices = MotionSampleCache::new(self.motions, self.start)?;
        let mut sample = |request| matrices.sample(request);
        let collision = find_event_root(
            self.start,
            self.end,
            self.collision_distance,
            self.real_distance,
            self.maximum_collision_deviation,
            false,
            |time| {
                let transforms = sample(time)?;
                geometry.distance(transforms[0], transforms[1])
            },
        )?;
        let mut best = collision.map(|root| EdgePairEvent {
            time: root.time,
            kind: EdgePairEventKind::Collision,
            edges: self.edges,
            root,
        });

        let angular = f64::from(self.angular_deviation);
        let end = best.map_or(self.end, |event| event.time);
        let parallel = find_event_root(
            self.start,
            end,
            1.0e-19,
            1.0e-19,
            (angular + angular) + 1.0e-19,
            true,
            |time| {
                let transforms = sample(time)?;
                geometry.parallelism(transforms[0], transforms[1])
            },
        )?;
        if let Some(root) = parallel {
            best = Some(EdgePairEvent {
                time: root.time,
                kind: EdgePairEventKind::Parallel,
                edges: self.edges,
                root,
            });
        }
        for side in 0..2 {
            let opposite = self.topologies[side].opposite(self.edges[side])?;
            for (face, sign) in [
                (opposite, geometry.sign),
                (self.edges[side], -geometry.sign),
            ] {
                let plane = EventPlane::from_edge(self.topologies[side], face)?;
                let direction = if side == 0 {
                    second.direction
                } else {
                    first.direction
                }
                .map(|value| value * sign);
                let end = best.map_or(self.end, |event| event.time);
                let root = find_event_root(
                    self.start,
                    end,
                    self.sector_thresholds[1 - side],
                    self.sector_thresholds[1 - side],
                    angular + 1.0e-19,
                    true,
                    |time| {
                        let transforms = sample(time)?;
                        let normal = world_vector(transforms[side].orientation, plane.normal);
                        let direction = world_vector(transforms[1 - side].orientation, direction);
                        Ok((direction[0] * normal[0] + direction[1] * normal[1])
                            + direction[2] * normal[2])
                    },
                )?;
                if let Some(root) = root {
                    best = Some(EdgePairEvent {
                        time: root.time,
                        kind: EdgePairEventKind::FaceTransition { side, face },
                        edges: self.edges,
                        root,
                    });
                }
            }
        }
        Ok(best)
    }
}

fn find_event_root(
    start: f64,
    end: f64,
    target: f64,
    real: f64,
    deviation: f64,
    immediate: bool,
    mut evaluate: impl FnMut(crate::ContinuousSample) -> Result<f64, FeatureEventError>,
) -> Result<Option<ContinuousRoot>, FeatureEventError> {
    let initial = evaluate(crate::ContinuousSample::Raster {
        time: start,
        index: 0,
    })?;
    if immediate && initial <= target {
        return Ok(Some(ContinuousRoot {
            time: start,
            value: initial,
            iterations: 0,
            exhausted: false,
        }));
    }
    if end <= start {
        return Ok(None);
    }
    let mut failure = None;
    let result = ContinuousTraversal {
        lower: start,
        upper: end,
        target,
        real_surface: real,
        maximum_deviation: deviation,
        initial_value: Some(initial),
        maximum_cells: 20,
    }
    .solve(|time| match evaluate(time) {
        Ok(value) => value,
        Err(error) => {
            failure = Some(error);
            f64::NAN
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(result?.map(|crossing| crossing.root))
}

fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    (first[1] * second[1] + first[0] * second[0]) + first[2] * second[2]
}
fn event_reciprocal_square_root(value: f64) -> f64 {
    crate::arithmetic::refined_inverse_root::<4>(value)
}
fn sub(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] - second[axis])
}
fn world_vector(matrix: [f64; 9], point: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| {
        (matrix[axis * 3 + 1] * point[1] + matrix[axis * 3] * point[0])
            + matrix[axis * 3 + 2] * point[2]
    })
}
fn local_vector(matrix: [f64; 9], point: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| {
        (matrix[axis + 3] * point[1] + matrix[axis] * point[0]) + matrix[axis + 6] * point[2]
    })
}
fn world_point(transform: ProjectionKnot, point: [f64; 3]) -> [f64; 3] {
    let rotated = world_vector(transform.orientation, point);
    std::array::from_fn(|axis| rotated[axis] + transform.position[axis])
}

#[cfg(test)]
mod tests {
    use crate::ObjectFrame;
    #[test]
    fn sector_threshold_scales_actual_separation_by_the_opposing_body_diameter() {
        let input = super::SectorThreshold {
            separation: f32::from_bits(0x3bc9_bc96),
            collision_distance: f32::from_bits(0x3bcf_fe5a),
            extra_radius: 0.0,
            change_distance: f32::from_bits(0x3a26_6515),
            inverse_diameter: f32::from_bits(0x3e0e_7227),
        };
        assert_eq!(input.value().unwrap().to_bits(), 0xbf16_7350_01f2_4669);
        let far = super::SectorThreshold {
            separation: 1.0,
            ..input
        }
        .value()
        .unwrap();
        assert!(input.value().unwrap() > far);
        assert_eq!(
            super::SectorThreshold {
                collision_distance: 0.0,
                ..input
            }
            .value(),
            Err(super::FeatureEventError::Continuous(
                crate::ContinuousError::InvalidBracket
            ))
        );
    }
    use super::*;
    use crate::{AuthoredFace, CoreOrientation};

    #[test]
    fn raster_slots_retain_first_visit_poses_and_refinement_samples_remain_independent() {
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        let pose = ProjectionKnot {
            position: [-0.0, 0.0, 0.0],
            orientation: orientation.matrix(),
        };
        let phase = BodyMotionPhase {
            position: pose.position,
            prior_orientation: orientation,
            next_orientation: orientation,
            projection_velocity: [1.0, 0.0, 0.0],
            start: 0.0,
            end: 0.03,
            inverse_step: 1.0 / 0.03,
        };
        let motions = [
            FeatureMotion::Moving {
                phase,
                frame: ObjectFrame::identity(),
                cache_time: 0.0,
                cached: pose,
            },
            FeatureMotion::Stationary(pose),
        ];
        let mut cache = MotionSampleCache::new(motions, 0.0).unwrap();
        let initial = cache
            .sample(crate::ContinuousSample::Raster {
                time: 0.0,
                index: 0,
            })
            .unwrap();
        let zero = cache
            .sample(crate::ContinuousSample::Refinement { time: 0.0 })
            .unwrap();
        assert_eq!(initial[0].position[0].to_bits(), (-0.0_f64).to_bits());
        assert_eq!(zero[0].position[0].to_bits(), 0.0_f64.to_bits());
        let first = cache
            .sample(crate::ContinuousSample::Raster {
                time: 0.005,
                index: 1,
            })
            .unwrap();
        let retained = cache
            .sample(crate::ContinuousSample::Raster {
                time: 0.006,
                index: 1,
            })
            .unwrap();
        assert_eq!(first, retained);
        let projected = cache
            .sample(crate::ContinuousSample::Refinement { time: 0.006 })
            .unwrap();
        assert_ne!(retained[0], projected[0]);
        assert_eq!(projected[1], pose);
    }

    #[test]
    fn vertex_edge_virtual_plane_and_rounded_sector_match_fixed_inputs() {
        let geometry = VertexEdgeGeometry {
            vertex: [
                13813741586594398208,
                4590369548129009664,
                13703814241021067264,
            ]
            .map(f64::from_bits),
            edge: EventEdge {
                origin: [
                    4612901990240878592,
                    9223372036854775808,
                    13836274027095654400,
                ]
                .map(f64::from_bits),
                direction: [0.0, 0.0, f64::from_bits(4607182418800017404)],
            },
        };
        let fixed = ProjectionKnot {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let poses = [
            ProjectionKnot {
                position: [
                    4613159370989762568,
                    13816272639929047616,
                    4472762171280850944,
                ]
                .map(f64::from_bits),
                ..fixed
            },
            fixed,
        ];
        let collision = geometry
            .collision(poses, f32::from_bits(0x3bcffe5a), 0.0)
            .unwrap();
        assert_eq!(
            collision.virtual_normal.map(f64::to_bits),
            [4604544271290062538, 4604544271145541840, 0]
        );
        assert_eq!(collision.plane_offset.to_bits(), 4569464545361264640);
        let extra = f32::from_bits(0x3bb4_c901);
        assert_eq!(
            geometry
                .collision(poses, f32::from_bits(0x3bcffe5a), extra)
                .unwrap()
                .plane_offset
                .to_bits(),
            ((f64::from(f32::from_bits(0x3bcffe5a)) + f64::from(extra)) * 0.5).to_bits()
        );
        assert_eq!(
            collision.distance(poses).unwrap().to_bits(),
            4587319725415975205
        );
        assert_eq!(
            geometry
                .sector(
                    [
                        4605321113040850789,
                        4603658452584689791,
                        4495329308725226322
                    ]
                    .map(f64::from_bits),
                    poses
                )
                .unwrap()
                .to_bits(),
            4593870734223984062
        );
        assert_eq!(
            geometry.sector([f64::NAN, 0.0, 0.0], poses),
            Err(FeatureEventError::NonFinite)
        );
    }

    #[test]
    fn vertex_on_edge_keeps_the_selected_zero_distance_normal() {
        let topology = topology(vec![[0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]);
        let placement = crate::FeaturePlacement {
            topology: &topology,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let result = crate::VertexEdgeSeparation::evaluate(
            [placement, placement],
            [topology.edge_id(0).unwrap(); 2],
            0.0,
        )
        .unwrap();
        assert_eq!(result.distance.to_bits(), (-0.0_f32).to_bits());
        assert_eq!(result.normal, [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn vertex_pair_collision_preserves_signed_projection_and_zero_ties() {
        let geometry = VertexPairGeometry {
            points: [[0.0; 3]; 2],
        };
        let identity = ProjectionKnot {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let poses = [
            identity,
            ProjectionKnot {
                position: [3.0, 4.0, 0.0],
                ..identity
            },
        ];
        let collision = geometry.collision([0.0, -1.0, 0.0]).unwrap();
        assert_eq!(collision.distance(poses).unwrap(), 4.0 * f64::from(1.2_f32));
        assert_eq!(
            geometry
                .collision([0.0, 1.0, 0.0])
                .unwrap()
                .distance(poses)
                .unwrap(),
            -4.0 * f64::from(1.2_f32)
        );
        assert_eq!(geometry.sector([0.0, 1.0, 0.0], poses).unwrap(), 4.0);
        assert_eq!(
            collision.distance([identity; 2]).unwrap().to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            geometry.collision([f32::NAN, 0.0, 0.0]),
            Err(FeatureEventError::NonFinite)
        );
    }

    #[test]
    fn coincident_vertex_pairs_report_intrusion_without_inventing_a_normal() {
        let topology = topology(vec![[0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]);
        let placement = crate::FeaturePlacement {
            topology: &topology,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let vertex = topology.edge_id(0).unwrap();
        assert_eq!(
            crate::VertexPairSeparation::evaluate([placement; 2], [vertex; 2], 0.0),
            Err(crate::FeatureWalkError::CoincidentVertices)
        );
        let point = crate::SurfaceFeature {
            edge: vertex,
            kind: crate::SurfaceFeatureKind::Vertex,
        };
        assert_eq!(
            crate::walk_compact_features(
                placement,
                placement,
                crate::SurfaceFeaturePair {
                    first: point,
                    second: point
                },
                16,
                |_| {}
            ),
            Err(crate::FeatureWalkError::Intruded)
        );
    }

    fn topology(points: Vec<[f32; 3]>) -> FeatureTopology {
        let word = |point: u32, offset: i32| point | (((offset as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            points,
            &[
                AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    metadata: 1,
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap()
    }

    fn query<'a>(
        moving: &'a FeatureTopology,
        fixed: &'a FeatureTopology,
    ) -> VertexFaceEventQuery<'a> {
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        VertexFaceEventQuery {
            moving,
            fixed,
            vertex: moving.edge_id(0).unwrap(),
            face: fixed.edge_id(0).unwrap(),
            moving_motion: FeatureMotion::Moving {
                phase: BodyMotionPhase {
                    position: [0.0, 0.0, 0.02],
                    prior_orientation: orientation,
                    next_orientation: orientation,
                    projection_velocity: [0.0, 0.0, -1.0],
                    start: 0.0,
                    end: 0.015,
                    inverse_step: (1.0 / f64::from(0.015_f32)) as f32,
                },
                frame: ObjectFrame::identity(),
                cache_time: 0.0,
                cached: ProjectionKnot {
                    position: [0.0, 0.0, 0.02],
                    orientation: orientation.matrix(),
                },
            },
            fixed_motion: FeatureMotion::Stationary(ProjectionKnot {
                position: [0.0; 3],
                orientation: orientation.matrix(),
            }),
            start: 0.0,
            end: 0.015,
            initial_distance: 0.02,
            collision_distance: 0.01,
            real_distance: 0.0,
            sector_threshold: -0.0001,
            maximum_collision_deviation: 1.0,
            maximum_angular_deviation: 1.0e-12,
        }
    }

    #[test]
    fn selected_vertex_produces_its_collision_without_all_vertex_search() {
        let topology = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let event = query(&topology, &topology).next().unwrap().unwrap();
        assert_eq!(event.kind, FeatureEventKind::Collision);
        assert!((event.time - 0.01).abs() < 1.0e-8);
    }

    #[test]
    fn selected_feature_uses_cached_start_pose_and_projects_only_later_times() {
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        let phase = BodyMotionPhase {
            position: [4.0, 5.0, 6.0],
            prior_orientation: orientation,
            next_orientation: orientation,
            projection_velocity: [2.0, 0.0, 0.0],
            start: 0.0,
            end: 0.015,
            inverse_step: 1.0 / 0.015,
        };
        let cached = ProjectionKnot {
            position: [1.0, 2.0, 3.0],
            orientation: orientation.matrix(),
        };
        let motion = FeatureMotion::Moving {
            phase,
            frame: ObjectFrame::identity(),
            cache_time: 0.0,
            cached,
        };
        assert_eq!(motion.sample(0.0).unwrap(), cached);
        assert_eq!(
            motion.sample(0.005).unwrap(),
            phase.search_sample(0.005).unwrap().1
        );
        assert!(motion.validate_interval(0.001, 0.015).is_err());
    }

    #[test]
    fn authored_adjacent_sector_preempts_the_later_collision() {
        let moving = topology(vec![[0.0; 3], [1.0, 0.0, 0.01], [0.0, 1.0, 0.0]]);
        let fixed = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let mut query = query(&moving, &fixed);
        let FeatureMotion::Moving {
            phase,
            frame,
            cache_time,
            cached,
        } = query.moving_motion
        else {
            unreachable!()
        };
        query.moving_motion = FeatureMotion::Moving {
            phase: BodyMotionPhase {
                position: [0.0, 0.0, 1.0],
                projection_velocity: [0.0; 3],
                next_orientation: phase
                    .prior_orientation
                    .advance([0.0, 2.0, 0.0], 0.015)
                    .unwrap(),
                ..phase
            },
            frame,
            cache_time,
            cached: ProjectionKnot {
                position: [0.0, 0.0, 1.0],
                ..cached
            },
        };
        query.initial_distance = 1.0;
        query.maximum_angular_deviation = 2.0;
        let event = query.next().unwrap().unwrap();
        let FeatureEventKind::SectorTransition { adjacent_edge } = event.kind else {
            panic!("sector event expected")
        };
        assert_eq!(moving.edge(adjacent_edge).unwrap().end, 1);
        assert!((0.004..0.006).contains(&event.time));
    }

    #[test]
    fn authored_sector_order_shortens_each_later_search_horizon() {
        let moving = topology(vec![[0.0; 3], [1.0, 0.0, 0.014], [1.0, 0.0, 0.018]]);
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        let pose = ProjectionKnot {
            position: [0.0; 3],
            orientation: orientation.matrix(),
        };
        let phase = BodyMotionPhase {
            position: [0.0; 3],
            prior_orientation: orientation,
            next_orientation: orientation.advance([0.0, 2.0, 0.0], 0.015).unwrap(),
            projection_velocity: [0.0; 3],
            start: 0.0,
            end: 0.015,
            inverse_step: (1.0 / f64::from(0.015_f32)) as f32,
        };
        let query = VertexFaceSectorQuery {
            moving: &moving,
            vertex: moving.edge_id(0).unwrap(),
            face: moving.edge_id(0).unwrap(),
            plane: EventPlane {
                origin: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
            },
            motions: [
                FeatureMotion::Moving {
                    phase,
                    frame: ObjectFrame::identity(),
                    cache_time: 0.0,
                    cached: pose,
                },
                FeatureMotion::Stationary(pose),
            ],
            start: 0.0,
            end: 0.015,
            threshold: -0.0001,
            maximum_deviation: 2.0,
        };
        let mut accepted = Vec::new();
        let selected = query.visit(|event| accepted.push(event)).unwrap().unwrap();
        assert_eq!(accepted.len(), 2);
        let indices = accepted
            .iter()
            .map(|event| {
                let FeatureEventKind::SectorTransition { adjacent_edge } = event.kind else {
                    unreachable!()
                };
                moving.edge(adjacent_edge).unwrap().end
            })
            .collect::<Vec<_>>();
        assert_eq!(indices, [2, 1]);
        assert!(accepted[1].time < accepted[0].time);
        assert_eq!(selected, accepted[1]);
        let opposite_order = VertexFaceSectorQuery {
            vertex: moving.edge_id(3).unwrap(),
            ..query
        }
        .visit(|_| {})
        .unwrap()
        .unwrap();
        assert_ne!(selected.time.to_bits(), opposite_order.time.to_bits());
    }

    #[test]
    fn event_planes_and_sector_axes_keep_their_distinct_target_arithmetic() {
        let floor = topology(vec![
            [2.54, -0.0, 2.54],
            [-2.54, -0.0, 2.54],
            [-2.54, -0.0, -2.54],
        ]);
        let plane = EventPlane::from_edge(&floor, floor.edge_id(0).unwrap()).unwrap();
        assert_eq!(plane.normal[1].to_bits(), 0xbfef_ffff_ffff_ffbc);
        assert_eq!(plane.origin[1].to_bits(), (-0.0_f64).to_bits());
        let axis = topology(vec![[0.0; 3], [0.0, 0.0, -0.2032], [1.0, 0.0, 0.0]]);
        let direction = SectorDirection::from_edge(&axis, axis.edge_id(0).unwrap()).unwrap();
        assert_eq!(direction.direction[2].to_bits(), 0xbfef_ffff_fd9b_9980);
        let outgoing = SectorDirection::outgoing(&axis, axis.edge_id(0).unwrap()).unwrap();
        assert_eq!(outgoing.len(), 2);
        assert_eq!(outgoing.last().unwrap().edge, axis.edge_id(0).unwrap());
    }

    #[test]
    fn malformed_query_inputs_fail_before_any_feature_event_is_published() {
        let topology = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let input = query(&topology, &topology);
        assert_eq!(
            VertexFaceEventQuery {
                start: f64::NAN,
                ..input
            }
            .next(),
            Err(FeatureEventError::NonFinite)
        );
        assert_eq!(
            VertexFaceEventQuery {
                end: input.start,
                ..input
            }
            .next(),
            Err(FeatureEventError::InvalidInterval)
        );
        assert_eq!(
            VertexFaceEventQuery {
                maximum_angular_deviation: 0.0,
                ..input
            }
            .next(),
            Err(FeatureEventError::InvalidDeviation)
        );
    }

    #[test]
    fn skew_edges_preserve_signed_distance_and_parallel_measurement() {
        let identity = ProjectionKnot {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let geometry = EdgePairGeometry {
            first: EventEdge {
                origin: [0.0, 0.0, 1.0],
                direction: [1.0, 0.0, 0.0],
            },
            second: EventEdge {
                origin: [0.0; 3],
                direction: [0.0, 1.0, 0.0],
            },
            sign: 1.0,
        };
        assert_eq!(geometry.distance(identity, identity).unwrap(), 1.0);
        assert_eq!(
            EdgePairGeometry {
                sign: -1.0,
                ..geometry
            }
            .distance(identity, identity)
            .unwrap(),
            -1.0
        );
        assert_eq!(geometry.parallelism(identity, identity).unwrap(), 1.0);
        assert_eq!(
            EdgePairGeometry {
                second: geometry.first,
                ..geometry
            }
            .parallelism(identity, identity)
            .unwrap(),
            0.0
        );
    }

    #[test]
    fn authored_edge_pair_collision_precedes_parallel_and_four_face_transitions() {
        let first = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        let second = topology(vec![[0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]);
        let motion = query(&first, &second);
        let input = EdgePairEventQuery {
            topologies: [&first, &second],
            edges: [first.edge_id(0).unwrap(), second.edge_id(0).unwrap()],
            motions: [motion.moving_motion, motion.fixed_motion],
            start: 0.0,
            end: 0.015,
            contact_normal: [0.0, 0.0, 1.0],
            collision_distance: 0.01,
            real_distance: 0.0,
            sector_thresholds: [-0.0001; 2],
            maximum_collision_deviation: 1.0,
            angular_deviation: 0.0,
        };
        let event = input.next().unwrap().unwrap();
        assert_eq!(event.kind, EdgePairEventKind::Collision);
        assert!((event.time - 0.01).abs() < 1.0e-8);
        let parallel = EdgePairEventQuery {
            topologies: [&first, &first],
            edges: [first.edge_id(0).unwrap(); 2],
            ..input
        }
        .next();
        assert_eq!(parallel, Err(FeatureEventError::ParallelEdges));
        assert_eq!(
            EdgePairEventQuery {
                angular_deviation: -1.0,
                ..input
            }
            .next(),
            Err(FeatureEventError::InvalidDeviation)
        );
        assert_eq!(
            EdgePairEventQuery {
                start: f64::NAN,
                ..input
            }
            .next(),
            Err(FeatureEventError::NonFinite)
        );
    }
}
