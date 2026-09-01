use crate::units::{
    DEGREES_PER_RADIAN, INCHES_PER_METER, METERS_PER_INCH, RADIANS_PER_DEGREE, internal_direction,
    internal_position, source_direction, source_position,
};
use crate::{
    AngularVelocityLimit, CacheActivity, CachedTransform, ClockError, CollisionBody,
    CollisionCorrection, CollisionMotion, CollisionPush, CollisionRequest, CollisionResponse,
    ContactFeatureBinding, ContactSurface, ContactTolerances, ContinuousError, ContinuousEvent,
    ContinuousEventDelay, ContinuousEventQueue, ContinuousEventTime, ConvexEndpoint,
    ConvexPairEvent, ConvexPairQuery, CoreOrientation, CoreTransformState, EventTimingHint,
    EventTimingKind, FeatureEventError, FeatureMotion, FeaturePlacement, FeatureTopology,
    FeatureWalkError, FixedStepClock, ImpactContactPoint, MotionError, MotionProfile, ObjectFrame,
    OrientationError, PhysicalShape, ProjectionKnot, QueuedVelocity, ResponseError, ShapeError,
    SourceAngleBasis, SurfaceFeaturePair, SurfacePair, TopologyError, TransformCache,
    TransformCacheError, VelocityCommandLimits, VelocityState, walk_compact_features,
};
use playsrc_collision::{AuthoredHullRef, PhysicsShape};
use playsrc_material::SurfacePropertyRegistry;
use std::{fmt, sync::Arc};
mod archive;
mod contacts;
pub use archive::BodyArchive;
pub use fluid::FluidInput;
pub use shadow::ShadowObservation;
mod events;
mod fluid;
#[cfg(test)]
#[path = "world/policy_tests.rs"]
mod policy_tests;
mod pose;
mod shadow;
mod storage;
mod time;
pub use contacts::{ContactBank, ContactOwnerState, FrictionContact, FrictionEvent};
pub use events::{PhysicsCallback, PhysicsCallbackKind, PhysicsCollisionData, PhysicsContactData};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhysicsStatistics {
    pub retired_impact_pairs: u32,
    pub oversized_contact_groups: u32,
    pub contact_freezes: u32,
    pub recursive_refinements: u32,
    pub recursive_scans: u32,
    pub tangent_solves: u32,
    pub feature_updates: u32,
    pub propagated_impacts: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetiredImpactPair {
    pub identity: u64,
    pub bodies: [u64; 2],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentObservation {
    pub contact: u64,
    pub response_coefficient: f32,
    pub time: f64,
    pub before: crate::ManifoldContact,
    pub bodies: [Option<crate::TangentBody>; 2],
    pub result: crate::ManifoldTangentResult,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionObservation {
    pub repetitions: u16,
    pub endpoints: [Option<CollisionBody>; 2],
    pub queued_before: [QueuedVelocity; 2],
    pub request_input: Option<CollisionRequest>,
    pub collision: EnvironmentCollision,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NormalObservation {
    pub time: f64,
    pub bodies: Vec<(u64, crate::NormalBody)>,
    pub contacts: Vec<(u64, [f64; 3], crate::NormalContactRow)>,
    pub system: crate::AssembledNormalSystem,
    pub solution: Option<crate::CoupledNormalSolution>,
    pub after: Vec<(u64, VelocityState, QueuedVelocity)>,
}
mod pairs;
pub use pairs::{ConvexPairObservation, ConvexPairResidence, ConvexPairState, RecursivePairState};

const INITIAL_TIMESTEP: f32 = 1.0 / 66.0;
const DEFAULT_CONTROLLER: u64 = 1;
const AIR_CONTROLLER: u64 = 2;

pub trait CollisionSolverState {
    fn clone_solver(&self) -> Box<dyn CollisionSolver>;
    fn same_state(&self, other: &dyn CollisionSolver) -> bool;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
impl<T: CollisionSolver + Clone + PartialEq + 'static> CollisionSolverState for T {
    fn clone_solver(&self) -> Box<dyn CollisionSolver> {
        Box::new(self.clone())
    }
    fn same_state(&self, other: &dyn CollisionSolver) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
pub trait CollisionSolver: fmt::Debug + CollisionSolverState + Send + Sync {
    fn should_collide(&self, first: u64, second: u64) -> bool;
    fn should_solve_penetration(&mut self, first: u64, second: u64, timestep: f32) -> bool;
    fn should_freeze_object(&mut self, body: u64) -> bool;
    fn additional_collision_checks_this_tick(&mut self, current: i32) -> i32;
    fn should_freeze_contacts(&mut self, bodies: &[u64]) -> bool;
}
#[derive(Debug, Default)]
struct CollisionSolverHandle(Option<Box<dyn CollisionSolver>>);
impl Clone for CollisionSolverHandle {
    fn clone(&self) -> Self {
        Self(self.0.as_ref().map(|solver| solver.clone_solver()))
    }
}
fn game_collision_allowed(
    solver: &CollisionSolverHandle,
    first: &RigidBody,
    second: &RigidBody,
) -> bool {
    solver.0.as_ref().is_none_or(|solver| {
        if (first.callback_flags & 0x0800 != 0 && second.callback_flags & 0x0400 != 0)
            || (second.callback_flags & 0x0800 != 0 && first.callback_flags & 0x0400 != 0)
        {
            return false;
        }
        solver.should_collide(first.identity, second.identity)
    })
}
impl PartialEq for CollisionSolverHandle {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same_state(b.as_ref()),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerformanceSettings {
    pub max_collisions_per_body: usize,
    pub max_collision_checks: usize,
    pub maximum_linear_speed: f32,
    pub maximum_angular_speed: f32,
    pub lookahead_world: f32,
    pub lookahead_bodies: f32,
    pub minimum_friction_mass: f32,
    pub maximum_friction_mass: f32,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_collisions_per_body: 6,
            max_collision_checks: 250,
            maximum_linear_speed: 2000.0,
            maximum_angular_speed: 3600.0,
            lookahead_world: 1.0,
            lookahead_bodies: 0.5,
            minimum_friction_mass: 10.0,
            maximum_friction_mass: 2500.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvironmentConfig {
    pub random_seed: u32,
    pub gravity: [f32; 3],
    pub air_density: f32,
    pub timestep: f32,
    pub max_bodies: usize,
    pub max_events: usize,
    pub performance: PerformanceSettings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyKind {
    Static,
    Dynamic,
}

#[derive(Clone, Copy)]
enum VelocityWrite {
    Queued,
    Add,
    Immediate,
}

#[derive(Clone, Debug)]
pub struct BodyInput {
    pub volume: f32,
    pub inertia_factor: f32,
    pub rotational_inertia_limit: f32,
    pub identity: u64,
    pub shape: Arc<PhysicsShape>,
    pub material: u32,
    pub kind: BodyKind,
    pub mass: f32,
    pub position: [f32; 3],
    pub angles: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub drag: f32,
    pub collisions_enabled: bool,
    pub gravity_enabled: bool,
    pub drag_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PublishedBody {
    pub angles: [f32; 3],
    pub identity: u64,
    pub position: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub orientation: [f32; 9],
    pub asleep: bool,
    pub motion_enabled: bool,
    pub is_static: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyCoreState {
    pub position: [f64; 3],
    pub orientation: CoreOrientation,
    pub collision_orientation: Option<[f64; 9]>,
    pub velocity: VelocityState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyCollision {
    pub body: u64,
    pub opposing: u64,
    pub body_offset: [f32; 3],
    pub opposing_offset: [f32; 3],
    pub normal: [f32; 3],
    pub request_speed: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyMotionPhase {
    pub position: [f64; 3],
    pub prior_orientation: CoreOrientation,
    pub next_orientation: CoreOrientation,
    pub projection_velocity: [f32; 3],
    pub start: f64,
    pub end: f64,
    pub inverse_step: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyConvex {
    pub body: u64,
    pub convex: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvexContactPair {
    pub endpoints: [BodyConvex; 2],
    pub seed: SurfaceFeaturePair,
    pub start: f64,
    pub end: f64,
    pub maximum_feature_transitions: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RetainedMotionPhase {
    phase: BodyMotionPhase,
    motion: CollisionMotion,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum QueuedCollisionInput {
    Direct(BodyCollision),
    Pair {
        pair: ConvexContactPair,
        predicted: ConvexPairEvent,
    },
}

impl QueuedCollisionInput {
    fn bodies(self) -> [u64; 2] {
        match self {
            Self::Direct(collision) => [collision.body, collision.opposing],
            Self::Pair { pair, .. } => pair.endpoints.map(|endpoint| endpoint.body),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct QueuedCollision {
    identity: u64,
    input: QueuedCollisionInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvironmentEvent {
    Boundary,
    Collision(u64),
    Convex(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactGeometry {
    pub endpoints: [BodyConvex; 2],
    pub shape_ids: [u64; 2],
    pub binding: ContactFeatureBinding,
    pub surface: ContactSurface,
    pub synchronized_offsets: [[f32; 3]; 2],
    pub materials: [u32; 2],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionDispatch {
    pub bodies: [u64; 2],
    pub time_code: u32,
    pub transforms: [CachedTransform; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentCollision {
    pub event_identity: Option<u64>,
    pub event_time: f64,
    pub body: u64,
    pub opposing: u64,
    pub normal: [f32; 3],
    pub before: [VelocityState; 2],
    pub after: [VelocityState; 2],
    pub queued_after: [QueuedVelocity; 2],
    pub applied: [bool; 2],
    pub surface_pair: SurfacePair,
    pub pushes: Vec<CollisionPush>,
    pub correction_impulse: f64,
    pub effective_masses: [f64; 2],
    pub request_speed: Option<f32>,
    pub geometry: Option<ContactGeometry>,
    pub dispatch: Option<CollisionDispatch>,
}

impl EnvironmentCollision {
    /// Authored contact point in Source XYZ inches; absent for unassociated explicit contacts.
    pub fn contact_point(&self) -> Option<[f32; 3]> {
        self.geometry
            .map(|geometry| source_position(geometry.surface.point))
    }

    /// Source XYZ surface normal directed from `body` toward `opposing`.
    pub fn surface_normal(&self) -> [f32; 3] {
        source_direction(self.normal.map(|component| -component), 1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RigidBody {
    material_token: u16,
    volume: f32,
    buoyancy_ratio: f32,
    storage_identity: u64,
    crowded_contact_ordering: bool,
    temporarily_frozen: bool,
    callback_flags: u16,
    quiet: crate::sleep::QuietMotion,
    identity: u64,
    core_identity: u64,
    shape: Arc<PhysicsShape>,
    physical: PhysicalShape,
    frame: ObjectFrame,
    topology: Arc<[FeatureTopology]>,
    material: u32,
    kind: BodyKind,
    core_position: [f64; 3],
    previous_core_position: [f64; 3],
    orientation: CoreOrientation,
    collision_orientation: Option<[f64; 9]>,
    previous_orientation: CoreOrientation,
    core_time: f64,
    core_inverse_step: f32,
    velocity: VelocityState,
    queued_velocity: QueuedVelocity,
    collision_count: u16,
    linear_damping: f32,
    angular_damping: f32,
    drag: f32,
    collisions_enabled: bool,
    motion_enabled: bool,
    asleep: bool,
    movement_range: crate::MovementRange,
    motion_phase: Option<RetainedMotionPhase>,
    publication_phase: Option<(BodyMotionPhase, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentSnapshot {
    shadows: shadow::ShadowSpace,
    fluids: fluid::FluidSpace,
    next_controller: u64,
    core_storage: storage::CoreStorage,
    retired_impact_pairs: Vec<RetiredImpactPair>,
    callbacks: events::CallbackState,
    normal_observations: Option<Vec<NormalObservation>>,
    collision_observations: Option<Vec<CollisionObservation>>,
    recovered_pairs: Vec<[u64; 2]>,
    world_materials: [u16; 128],
    tangent_observations: Option<Vec<TangentObservation>>,
    statistics: PhysicsStatistics,
    event_reporting: bool,
    active_objects: Vec<u64>,
    last_friction_time: f32,
    friction_events: Vec<contacts::FrictionEvent>,
    sleep_scheduler: crate::SleepScheduler,
    contacts: contacts::ContactGroups,
    collision_solver: CollisionSolverHandle,
    pairs: pairs::PairSpace,
    islands: crate::SimulationIslands,
    next_core: u64,
    pending_wake: Vec<u64>,
    delete_queue_enabled: bool,
    pending_deletes: Vec<u64>,
    transforms: TransformCache,
    config: EnvironmentConfig,
    surface_identity: [u8; 32],
    bodies: Vec<RigidBody>,
    queue: ContinuousEventQueue<EnvironmentEvent>,
    collisions: Vec<EnvironmentCollision>,
    queued_collisions: Vec<QueuedCollision>,
    clock: FixedStepClock,
    ticks: u64,
    paused: bool,
}

#[derive(Clone, Debug)]
pub struct PhysicsEnvironment {
    shadows: shadow::ShadowSpace,
    fluids: fluid::FluidSpace,
    next_controller: u64,
    core_storage: storage::CoreStorage,
    retired_impact_pairs: Vec<RetiredImpactPair>,
    callbacks: events::CallbackState,
    normal_observations: Option<Vec<NormalObservation>>,
    collision_observations: Option<Vec<CollisionObservation>>,
    recovered_pairs: Vec<[u64; 2]>,
    world_materials: [u16; 128],
    tangent_observations: Option<Vec<TangentObservation>>,
    statistics: PhysicsStatistics,
    event_reporting: bool,
    active_objects: Vec<u64>,
    last_friction_time: f32,
    friction_events: Vec<contacts::FrictionEvent>,
    sleep_scheduler: crate::SleepScheduler,
    contacts: contacts::ContactGroups,
    pairs: pairs::PairSpace,
    collision_solver: CollisionSolverHandle,
    islands: crate::SimulationIslands,
    next_core: u64,
    pending_wake: Vec<u64>,
    delete_queue_enabled: bool,
    pending_deletes: Vec<u64>,
    tolerances: ContactTolerances,
    search_ranges: crate::CollisionSearchRanges,
    transforms: TransformCache,
    config: EnvironmentConfig,
    surfaces: Arc<SurfacePropertyRegistry>,
    bodies: Vec<RigidBody>,
    queue: ContinuousEventQueue<EnvironmentEvent>,
    collisions: Vec<EnvironmentCollision>,
    queued_collisions: Vec<QueuedCollision>,
    clock: FixedStepClock,
    ticks: u64,
    paused: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentError {
    MissingShadow,
    StepLimit,
    CollisionSolverRequired,
    CallbackLimit,
    ObservationLimit,
    SourceVector(crate::SourceVectorError),
    Sleep(crate::SleepError),
    Contacts(contacts::ContactError),
    Pairs(pairs::PairError),
    Island(crate::IslandError),
    Cache(TransformCacheError),
    NonFinite,
    InvalidTimestep,
    InvalidAirDensity,
    InvalidBodyLimit,
    InvalidPerformance,
    DuplicateBody,
    MissingBody,
    BodyLimit,
    StaticBody,
    DisabledMotion,
    DisabledCollision,
    IdenticalBodies,
    CollisionLimit,
    MissingMotionPhase,
    InvalidMotionPhase,
    DerivedEventTime,
    DependencyMismatch,
    SnapshotMismatch,
    ClockOverflow,
    Clock(ClockError),
    Shape(ShapeError),
    Topology(TopologyError),
    Motion(MotionError),
    Orientation(OrientationError),
    Response(ResponseError),
    Event(ContinuousError),
    Feature(FeatureEventError),
    FeatureWalk(FeatureWalkError),
    MovementRange(crate::MovementRangeError),
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingShadow => formatter.write_str("body has no shadow controller"),
            Self::StepLimit => {
                formatter.write_str("physical submission exceeds its integration-step capacity")
            }
            Self::CollisionSolverRequired => {
                formatter.write_str("oversized contact groups require a collision solver policy")
            }
            Self::CallbackLimit => {
                formatter.write_str("physical callback output exceeds its configured capacity")
            }
            Self::ObservationLimit => {
                formatter.write_str("physical observation buffer exceeds its configured capacity")
            }
            Self::SourceVector(error) => error.fmt(formatter),
            Self::Sleep(error) => error.fmt(formatter),
            Self::Contacts(error) => error.fmt(formatter),
            Self::Pairs(error) => error.fmt(formatter),
            Self::Island(error) => error.fmt(formatter),
            Self::Cache(error) => error.fmt(formatter),
            Self::NonFinite => {
                formatter.write_str("physical environment contains a non-finite value")
            }
            Self::InvalidTimestep => {
                formatter.write_str("physical environment timestep must be positive")
            }
            Self::InvalidAirDensity => {
                formatter.write_str("physical environment air density cannot be negative")
            }
            Self::InvalidBodyLimit => {
                formatter.write_str("physical environment body limit must be positive")
            }
            Self::InvalidPerformance => {
                formatter.write_str("physical environment performance settings are invalid")
            }
            Self::DuplicateBody => formatter.write_str("physical body identity already exists"),
            Self::MissingBody => formatter.write_str("physical body identity does not exist"),
            Self::BodyLimit => formatter.write_str("physical environment body limit exceeded"),
            Self::StaticBody => formatter.write_str("static physical body cannot enable motion"),
            Self::DisabledMotion => {
                formatter.write_str("disabled physical body cannot accept velocity")
            }
            Self::DisabledCollision => {
                formatter.write_str("disabled physical body cannot accept collision response")
            }
            Self::IdenticalBodies => {
                formatter.write_str("physical collision endpoints must be distinct bodies")
            }
            Self::CollisionLimit => {
                formatter.write_str("physical environment collision event limit exceeded")
            }
            Self::MissingMotionPhase => {
                formatter.write_str("queued collision endpoint has no retained motion phase")
            }
            Self::InvalidMotionPhase => {
                formatter.write_str("queued collision lies outside its retained motion phase")
            }
            Self::DerivedEventTime => formatter
                .write_str("authored feature-event time must be regenerated from its motion phase"),
            Self::DependencyMismatch => {
                formatter.write_str("physical environment dependency identity does not match")
            }
            Self::SnapshotMismatch => {
                formatter.write_str("physical environment snapshot contains inconsistent state")
            }
            Self::ClockOverflow => formatter.write_str("physical environment clock cannot advance"),
            Self::Clock(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
            Self::Topology(error) => error.fmt(formatter),
            Self::Motion(error) => error.fmt(formatter),
            Self::Orientation(error) => error.fmt(formatter),
            Self::Response(error) => error.fmt(formatter),
            Self::Event(error) => error.fmt(formatter),
            Self::Feature(error) => error.fmt(formatter),
            Self::FeatureWalk(error) => error.fmt(formatter),
            Self::MovementRange(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for EnvironmentError {}
impl From<crate::SourceVectorError> for EnvironmentError {
    fn from(error: crate::SourceVectorError) -> Self {
        Self::SourceVector(error)
    }
}
impl From<crate::EnergyError> for EnvironmentError {
    fn from(error: crate::EnergyError) -> Self {
        Self::Response(error.into())
    }
}
impl From<crate::SleepError> for EnvironmentError {
    fn from(error: crate::SleepError) -> Self {
        Self::Sleep(error)
    }
}
impl From<pairs::PairError> for EnvironmentError {
    fn from(error: pairs::PairError) -> Self {
        Self::Pairs(error)
    }
}

impl From<crate::IslandError> for EnvironmentError {
    fn from(error: crate::IslandError) -> Self {
        Self::Island(error)
    }
}

impl From<crate::MovementRangeError> for EnvironmentError {
    fn from(value: crate::MovementRangeError) -> Self {
        Self::MovementRange(value)
    }
}

impl From<TransformCacheError> for EnvironmentError {
    fn from(value: TransformCacheError) -> Self {
        Self::Cache(value)
    }
}

impl From<ClockError> for EnvironmentError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

impl From<ShapeError> for EnvironmentError {
    fn from(error: ShapeError) -> Self {
        Self::Shape(error)
    }
}
impl From<TopologyError> for EnvironmentError {
    fn from(error: TopologyError) -> Self {
        Self::Topology(error)
    }
}
impl From<MotionError> for EnvironmentError {
    fn from(error: MotionError) -> Self {
        Self::Motion(error)
    }
}
impl From<OrientationError> for EnvironmentError {
    fn from(error: OrientationError) -> Self {
        Self::Orientation(error)
    }
}
impl From<ContinuousError> for EnvironmentError {
    fn from(error: ContinuousError) -> Self {
        Self::Event(error)
    }
}

impl From<ResponseError> for EnvironmentError {
    fn from(error: ResponseError) -> Self {
        Self::Response(error)
    }
}
impl From<FeatureEventError> for EnvironmentError {
    fn from(error: FeatureEventError) -> Self {
        Self::Feature(error)
    }
}
impl From<FeatureWalkError> for EnvironmentError {
    fn from(error: FeatureWalkError) -> Self {
        Self::FeatureWalk(error)
    }
}

impl EnvironmentConfig {
    fn validate(self) -> Result<Self, EnvironmentError> {
        let performance = self.performance;
        if self.gravity.iter().any(|component| !component.is_finite())
            || !self.air_density.is_finite()
            || !self.timestep.is_finite()
            || [
                performance.maximum_linear_speed,
                performance.maximum_angular_speed,
                performance.lookahead_world,
                performance.lookahead_bodies,
                performance.minimum_friction_mass,
                performance.maximum_friction_mass,
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(EnvironmentError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(EnvironmentError::InvalidTimestep);
        }
        if self.air_density < 0.0 {
            return Err(EnvironmentError::InvalidAirDensity);
        }
        if self.max_bodies == 0 {
            return Err(EnvironmentError::InvalidBodyLimit);
        }
        if self.max_events == 0 || self.max_events >= crate::continuous::SORTED_EVENT_CAPACITY {
            return Err(ContinuousError::InvalidEventCapacity.into());
        }
        if performance.max_collisions_per_body == 0
            || performance.max_collision_checks == 0
            || performance.maximum_linear_speed <= 0.0
            || performance.maximum_angular_speed <= 0.0
            || performance.lookahead_world < 0.0
            || performance.lookahead_bodies < 0.0
            || performance.minimum_friction_mass <= 0.0
            || performance.maximum_friction_mass < performance.minimum_friction_mass
        {
            return Err(EnvironmentError::InvalidPerformance);
        }
        Ok(self)
    }
}

impl RigidBody {
    fn integration_frame(&self) -> crate::impulses::ImpulseFrame {
        let (position, orientation) = if let Some(basis) = self.collision_orientation {
            (self.core_position, basis)
        } else if let Some(phase) = self.motion_phase() {
            (phase.position, phase.prior_orientation.matrix())
        } else {
            (self.core_position, self.orientation.matrix())
        };
        crate::impulses::ImpulseFrame {
            position,
            orientation,
            inverse_mass: if self.motion_enabled {
                1.0 / self.physical.mass
            } else {
                0.0
            },
            inverse_inertia: self.physical.inertia.map(|value| {
                if self.motion_enabled {
                    1.0 / value
                } else {
                    0.0
                }
            }),
        }
    }
    pub fn material_index(&self) -> u32 {
        u32::from(self.material_token)
    }
    pub fn volume(&self) -> f32 {
        self.volume
    }
    pub fn buoyancy_ratio(&self) -> f32 {
        self.buoyancy_ratio
    }
    pub fn core_storage_identity(&self) -> u64 {
        self.storage_identity
    }
    fn integrate_rotation(
        &mut self,
        timestep: f32,
        apply_inertia: bool,
    ) -> Result<(CoreOrientation, CollisionMotion), EnvironmentError> {
        let basis = self
            .collision_orientation
            .unwrap_or_else(|| self.orientation.matrix());
        if self.physical.uses_simple_rotation() || !apply_inertia {
            Ok((
                self.orientation.advance(self.velocity.angular, timestep)?,
                CollisionMotion::from_velocity(
                    self.velocity,
                    timestep,
                    basis,
                    self.physical.surface_deviation_radius,
                )?,
            ))
        } else {
            let step = crate::AngularStep::integrate(
                self.velocity.angular,
                self.physical.inertia,
                timestep,
            )?;
            let next = self.orientation.apply_local_delta(step.delta)?;
            let motion = CollisionMotion::from_delta(
                self.velocity.linear,
                step.delta,
                timestep,
                basis,
                self.physical.surface_deviation_radius,
            )?;
            self.velocity.angular = step.angular_velocity;
            Ok((next, motion))
        }
    }
    fn synchronize_collision_pose(&mut self, time: f64) -> Result<(), EnvironmentError> {
        let phase = self
            .motion_phase()
            .ok_or(EnvironmentError::MissingMotionPhase)?;
        let (orientation, transform) = phase.sample(time)?;
        self.core_position = transform.position;
        self.orientation = orientation;
        if time != phase.start || self.collision_orientation.is_none() {
            self.collision_orientation = Some(transform.orientation);
        }
        if !self.physical.uses_simple_rotation() {
            self.velocity.angular = phase
                .prior_orientation
                .phase_angular_velocity(phase.next_orientation, phase.inverse_step)?;
        }
        Ok(())
    }
    pub fn crowded_contact_ordering(&self) -> bool {
        self.crowded_contact_ordering
    }
    pub fn callback_flags(&self) -> u16 {
        self.callback_flags
    }
    pub fn core_identity(&self) -> u64 {
        self.core_identity
    }
    pub fn identity(&self) -> u64 {
        self.identity
    }
    pub fn kind(&self) -> BodyKind {
        self.kind
    }
    pub fn material(&self) -> u32 {
        self.material
    }
    pub fn physical_shape(&self) -> PhysicalShape {
        self.physical
    }
    pub fn object_frame(&self) -> ObjectFrame {
        self.frame
    }
    pub fn topology(&self, convex: usize) -> Option<&FeatureTopology> {
        self.topology.get(convex)
    }
    pub fn authored_hull(&self, convex: usize) -> Option<AuthoredHullRef> {
        let hull = if convex < self.shape.convex_count() {
            AuthoredHullRef::Piece(convex)
        } else {
            AuthoredHullRef::Enclosure(convex - self.shape.convex_count())
        };
        self.shape.authored_hull(hull).map(|_| hull)
    }
    fn hull_index(&self, hull: AuthoredHullRef) -> Result<usize, EnvironmentError> {
        let index = match hull {
            AuthoredHullRef::Piece(index) => index,
            AuthoredHullRef::Enclosure(index) => self
                .shape
                .convex_count()
                .checked_add(index)
                .ok_or(TopologyError::MissingAuthoredEnclosure { enclosure: index })?,
        };
        if self.authored_hull(index) != Some(hull) {
            return Err(crate::HierarchyError::InvalidHull.into());
        }
        Ok(index)
    }
    pub fn core_position(&self) -> [f64; 3] {
        self.core_position
    }
    pub fn previous_core_position(&self) -> [f64; 3] {
        self.previous_core_position
    }
    pub fn core_orientation(&self) -> CoreOrientation {
        self.orientation
    }
    pub fn previous_core_orientation(&self) -> CoreOrientation {
        self.previous_orientation
    }
    pub fn internal_velocity(&self) -> VelocityState {
        self.velocity
    }
    pub fn queued_velocity(&self) -> QueuedVelocity {
        self.queued_velocity
    }

    fn reported_velocity(&self) -> VelocityState {
        VelocityState {
            linear: std::array::from_fn(|axis| {
                self.velocity.linear[axis] + self.queued_velocity.linear[axis]
            }),
            angular: std::array::from_fn(|axis| {
                self.velocity.angular[axis] + self.queued_velocity.angular[axis]
            }),
        }
    }
    pub fn is_asleep(&self) -> bool {
        self.asleep
    }
    pub fn movement_range(&self) -> &crate::MovementRange {
        &self.movement_range
    }
    pub fn collision_count(&self) -> u16 {
        self.collision_count
    }
    pub fn is_motion_enabled(&self) -> bool {
        self.motion_enabled
    }
    pub fn collisions_enabled(&self) -> bool {
        self.collisions_enabled
    }
    pub fn is_moveable(&self) -> bool {
        self.kind == BodyKind::Dynamic && self.motion_enabled
    }
    pub fn motion_phase(&self) -> Option<BodyMotionPhase> {
        self.motion_phase.map(|retained| retained.phase)
    }
    pub fn collision_motion(&self) -> Option<CollisionMotion> {
        self.motion_phase.map(|retained| CollisionMotion {
            linear_velocity: self.velocity.linear,
            ..retained.motion
        })
    }

    pub fn published(&self) -> Result<PublishedBody, EnvironmentError> {
        let (orientation, position) = if let Some((phase, terminal)) = self.publication_phase {
            let (orientation, transform) = phase.publication_sample(terminal)?;
            (orientation, transform.position)
        } else {
            (
                self.previous_orientation
                    .interpolate(self.orientation, 0.0)?,
                self.core_position,
            )
        };
        let pose = self.frame.object_pose(ProjectionKnot {
            position,
            orientation: orientation.matrix(),
        })?;
        let velocity = self.reported_velocity();
        Ok(PublishedBody {
            angles: orientation.source_angles()?,
            identity: self.identity,
            position: source_position(pose.position),
            linear_velocity: source_direction(velocity.linear, INCHES_PER_METER),
            angular_velocity: source_direction(velocity.angular, DEGREES_PER_RADIAN),
            orientation: orientation.source_matrix(),
            asleep: self.asleep,
            motion_enabled: self.motion_enabled,
            is_static: self.kind == BodyKind::Static,
        })
    }

    fn motion_profile(&self, config: EnvironmentConfig) -> MotionProfile {
        MotionProfile {
            timestep: config.timestep,
            gravity: internal_direction(config.gravity, METERS_PER_INCH),
            linear_damping: self.linear_damping,
            angular_damping: self.angular_damping,
            settling: false,
            drag: self.drag,
            air_density: config.air_density,
            linear_drag_basis: self.physical.linear_drag_basis,
            angular_drag_basis: self.physical.angular_drag_basis,
            maximum_linear_speed: config.performance.maximum_linear_speed * METERS_PER_INCH,
            maximum_angular_per_step: config.performance.maximum_angular_speed
                * RADIANS_PER_DEGREE
                * INITIAL_TIMESTEP,
        }
    }

    fn prepare_boundary(&mut self, boundary: f64) -> Result<bool, EnvironmentError> {
        if self.kind == BodyKind::Static || self.asleep {
            return Ok(false);
        }
        if let Some(phase) = self.motion_phase()
            && phase.start != boundary
        {
            if phase.end != boundary {
                return Err(EnvironmentError::InvalidMotionPhase);
            }
            self.core_position = std::array::from_fn(|axis| {
                phase.position[axis]
                    + f64::from(phase.projection_velocity[axis])
                        * f64::from((boundary - phase.start) as f32)
            });
            self.orientation = phase.next_orientation;
        }
        self.velocity = self.reported_velocity();
        self.queued_velocity = QueuedVelocity::default();
        self.collision_count = 0;
        self.temporarily_frozen = false;
        self.collision_orientation = Some(self.orientation.matrix());
        Ok(true)
    }

    fn commit_boundary(
        &mut self,
        config: EnvironmentConfig,
        boundary: f64,
    ) -> Result<(), EnvironmentError> {
        self.velocity = self
            .motion_profile(config)
            .constrain_velocity(self.velocity)?;
        let (next_orientation, motion) = self.integrate_rotation(config.timestep, true)?;
        let phase = BodyMotionPhase {
            position: self.core_position,
            prior_orientation: self.orientation,
            next_orientation,
            projection_velocity: self.velocity.linear,
            start: boundary,
            end: boundary + f64::from(config.timestep),
            inverse_step: (1.0 / f64::from(config.timestep)) as f32,
        };
        self.collision_orientation = None;
        self.retain_integrated_phase(phase, motion)?;
        self.publication_phase = None;
        if self.asleep {
            self.motion_phase = None;
            self.previous_core_position = self.core_position;
            self.previous_orientation = self.orientation;
        }
        Ok(())
    }

    fn publish_phase(&mut self, terminal: f64) -> Result<(), EnvironmentError> {
        let Some(phase) = self.motion_phase() else {
            return Ok(());
        };
        phase.publication_sample(terminal)?;
        self.previous_core_position = phase.position;
        self.core_position = std::array::from_fn(|axis| {
            phase.position[axis]
                + f64::from(phase.projection_velocity[axis]) * (phase.end - phase.start)
        });
        self.previous_orientation = phase.prior_orientation;
        self.orientation = phase.next_orientation;
        self.collision_orientation = None;
        self.publication_phase = Some((phase, terminal));
        Ok(())
    }

    fn retain_motion_phase(&mut self, phase: BodyMotionPhase) -> Result<(), EnvironmentError> {
        let basis = self
            .collision_orientation
            .unwrap_or_else(|| phase.prior_orientation.matrix());
        let motion = CollisionMotion::from_velocity(
            self.velocity,
            (phase.end - phase.start) as f32,
            basis,
            self.physical.surface_deviation_radius,
        )?;
        self.retain_integrated_phase(phase, motion)
    }
    fn retain_integrated_phase(
        &mut self,
        phase: BodyMotionPhase,
        motion: CollisionMotion,
    ) -> Result<(), EnvironmentError> {
        self.movement_range
            .advance(phase.start, (phase.end - phase.start) as f32, motion)?;
        self.core_time = phase.start;
        self.core_inverse_step = phase.inverse_step;
        self.motion_phase = Some(RetainedMotionPhase { phase, motion });
        Ok(())
    }
}

impl BodyMotionPhase {
    fn publication_sample(
        self,
        time: f64,
    ) -> Result<(CoreOrientation, ProjectionKnot), EnvironmentError> {
        if !time.is_finite()
            || !self.start.is_finite()
            || !self.end.is_finite()
            || !self.inverse_step.is_finite()
        {
            return Err(EnvironmentError::NonFinite);
        }
        if self.start >= self.end || self.inverse_step <= 0.0 {
            return Err(EnvironmentError::InvalidMotionPhase);
        }
        self.interpolate_at(time, true)
    }
    pub fn sample(self, time: f64) -> Result<(CoreOrientation, ProjectionKnot), EnvironmentError> {
        self.sample_bounded(time, self.end, true)
    }

    pub(crate) fn search_sample(
        self,
        time: f64,
    ) -> Result<(CoreOrientation, ProjectionKnot), EnvironmentError> {
        self.sample_bounded(
            time,
            self.end + f64::from(crate::continuous::SEARCH_CELL_SECONDS),
            false,
        )
    }

    fn sample_bounded(
        self,
        time: f64,
        terminal: f64,
        preserve_zero: bool,
    ) -> Result<(CoreOrientation, ProjectionKnot), EnvironmentError> {
        if !time.is_finite()
            || !terminal.is_finite()
            || !self.start.is_finite()
            || !self.end.is_finite()
            || !self.inverse_step.is_finite()
        {
            return Err(EnvironmentError::NonFinite);
        }
        if self.start >= self.end
            || time < self.start
            || time > terminal
            || self.inverse_step <= 0.0
        {
            return Err(EnvironmentError::InvalidMotionPhase);
        }
        self.interpolate_at(time, preserve_zero)
    }
    fn interpolate_at(
        self,
        time: f64,
        preserve_zero: bool,
    ) -> Result<(CoreOrientation, ProjectionKnot), EnvironmentError> {
        let elapsed = (time - self.start) as f32;
        let orientation = if elapsed == 0.0 && preserve_zero {
            self.prior_orientation
        } else {
            self.prior_orientation.interpolate(
                self.next_orientation,
                f64::from(elapsed * self.inverse_step),
            )?
        };
        let mut transform = ProjectionKnot::from_cache(
            self.position,
            orientation,
            self.projection_velocity,
            self.start,
            time,
        )?;
        if elapsed == 0.0 && !preserve_zero {
            transform.position = std::array::from_fn(|axis| {
                self.position[axis] + f64::from(self.projection_velocity[axis]) * f64::from(elapsed)
            });
        }
        Ok((orientation, transform))
    }
}

impl PhysicsEnvironment {
    pub fn contact_tolerances(&self) -> ContactTolerances {
        self.tolerances
    }
    pub fn collision_search_ranges(&self) -> crate::CollisionSearchRanges {
        self.search_ranges
    }
    pub fn transform_cache(&self) -> &TransformCache {
        &self.transforms
    }

    pub fn cached_transform(&mut self, identity: u64) -> Result<CachedTransform, EnvironmentError> {
        let (activity, state) = self.transform_input(identity)?;
        Ok(self
            .transforms
            .resolve(identity, activity, self.clock.time_code(), state)?)
    }

    fn transform_input(
        &self,
        identity: u64,
    ) -> Result<(CacheActivity, CoreTransformState), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        let time = self.clock.current_time();
        let (activity, state) = if let Some(phase) = body.motion_phase() {
            (
                CacheActivity::Simulated,
                CoreTransformState {
                    object_frame: body.frame,
                    position: phase.position,
                    prior_orientation: phase.prior_orientation,
                    next_orientation: phase.next_orientation,
                    projection_velocity: phase.projection_velocity,
                    core_time: phase.start,
                    environment_time: time,
                    inverse_step: phase.inverse_step,
                },
            )
        } else {
            (
                CacheActivity::Inactive,
                CoreTransformState {
                    object_frame: body.frame,
                    position: body.core_position,
                    prior_orientation: body.previous_orientation,
                    next_orientation: body.orientation,
                    projection_velocity: [0.0; 3],
                    core_time: body.core_time,
                    environment_time: time,
                    inverse_step: body.core_inverse_step,
                },
            )
        };
        Ok((activity, state))
    }
    pub fn new(
        config: EnvironmentConfig,
        surfaces: Arc<SurfacePropertyRegistry>,
    ) -> Result<Self, EnvironmentError> {
        let config = config.validate()?;
        if surfaces.resolve(Some(b"default")).is_none() {
            return Err(EnvironmentError::DependencyMismatch);
        }
        let mut queue = ContinuousEventQueue::new(
            0.0,
            config
                .max_events
                .checked_add(1)
                .ok_or(ContinuousError::InvalidEventCapacity)?,
        )?;
        queue.insert(EnvironmentEvent::Boundary, 0.0)?;
        let mut islands = crate::SimulationIslands::new(
            config
                .max_bodies
                .checked_add(1)
                .ok_or(EnvironmentError::InvalidBodyLimit)?,
            config
                .max_events
                .checked_mul(3)
                .and_then(|n| n.checked_add(2))
                .ok_or(EnvironmentError::InvalidPerformance)?,
        )?;
        islands.register_core(0, true)?;
        islands.register_controller(crate::IslandController {
            identity: DEFAULT_CONTROLLER,
            priority: 1000,
            associated: vec![],
        })?;
        islands.register_controller(crate::IslandController {
            identity: AIR_CONTROLLER,
            priority: 500,
            associated: vec![],
        })?;
        Ok(Self {
            shadows: shadow::ShadowSpace::default(),
            fluids: fluid::FluidSpace::default(),
            next_controller: 3,
            core_storage: storage::CoreStorage::default(),
            retired_impact_pairs: Vec::new(),
            callbacks: events::CallbackState::default(),
            normal_observations: None,
            collision_observations: None,
            recovered_pairs: Vec::new(),
            world_materials: std::array::from_fn(|i| i as u16),
            tangent_observations: None,
            statistics: PhysicsStatistics::default(),
            event_reporting: false,
            active_objects: Vec::new(),
            last_friction_time: 0.0,
            friction_events: Vec::new(),
            sleep_scheduler: crate::SleepScheduler::new(10, config.random_seed),
            contacts: contacts::ContactGroups::new(config.max_events),
            pairs: pairs::PairSpace::new(config.max_bodies, config.max_events)?,
            collision_solver: CollisionSolverHandle::default(),
            islands,
            next_core: 1,
            pending_wake: Vec::new(),
            delete_queue_enabled: false,
            pending_deletes: Vec::new(),
            tolerances: ContactTolerances::from_gravity(config.gravity)?,
            search_ranges: crate::CollisionSearchRanges::new(
                config.timestep,
                config.performance.lookahead_world,
                config.performance.lookahead_bodies,
            )?,
            transforms: TransformCache::default(),
            config,
            surfaces,
            bodies: Vec::new(),
            queue,
            collisions: Vec::new(),
            queued_collisions: Vec::new(),
            clock: FixedStepClock::new(config.timestep)?,
            ticks: 0,
            paused: false,
        })
    }

    pub fn config(&self) -> EnvironmentConfig {
        self.config
    }
    pub fn random_state(&self) -> u32 {
        self.sleep_scheduler.random_state()
    }
    pub fn time(&self) -> f64 {
        self.clock.current_time()
    }
    pub fn clock(&self) -> FixedStepClock {
        self.clock
    }
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn bodies(&self) -> &[RigidBody] {
        &self.bodies[..self.bodies.len() - self.pending_deletes.len()]
    }
    pub fn set_event_reporting(&mut self, enabled: bool) {
        self.event_reporting = enabled;
    }
    /// Installs the Source triangle-material translation; unspecified slots retain their values.
    pub fn set_world_material_index_table(&mut self, indices: &[i32]) {
        for (target, index) in self.world_materials.iter_mut().zip(indices) {
            *target = *index as u16;
        }
    }
    pub fn world_material_index_table(&self) -> &[u16; 128] {
        &self.world_materials
    }
    fn contact_materials(
        &self,
        endpoints: [BodyConvex; 2],
        features: crate::SurfaceFeaturePair,
    ) -> Result<[u32; 2], EnvironmentError> {
        let mut materials = [0; 2];
        for (side, feature) in [features.first, features.second].into_iter().enumerate() {
            let body = self
                .body(endpoints[side].body)
                .ok_or(EnvironmentError::MissingBody)?;
            let encoded = (body
                .topology(endpoints[side].convex)
                .ok_or(EnvironmentError::DependencyMismatch)?
                .face_metadata(feature.edge)?
                >> 24)
                & 0x7f;
            materials[side] = if encoded == 0 {
                body.material
            } else {
                u32::from(self.world_materials[encoded as usize])
            };
        }
        Ok(SurfacePair::from_registry(&self.surfaces, materials)
            .map_err(|_| EnvironmentError::DependencyMismatch)?
            .identities)
    }
    pub fn statistics(&self) -> PhysicsStatistics {
        self.statistics
    }
    pub fn record_tangent_observations(&mut self, enabled: bool) {
        self.tangent_observations = enabled.then(Vec::new);
    }
    pub fn tangent_observations(&self) -> Option<&[TangentObservation]> {
        self.tangent_observations.as_deref()
    }
    pub fn record_collision_observations(&mut self, enabled: bool) {
        self.collision_observations = enabled.then(Vec::new);
    }
    pub fn collision_observations(&self) -> Option<&[CollisionObservation]> {
        self.collision_observations.as_deref()
    }
    pub fn record_normal_observations(&mut self, enabled: bool) {
        self.normal_observations = enabled.then(Vec::new);
    }
    pub fn normal_observations(&self) -> Option<&[NormalObservation]> {
        self.normal_observations.as_deref()
    }
    pub fn set_callback_flags(&mut self, body: u64, flags: u16) -> Result<(), EnvironmentError> {
        self.body_mut(body)?.callback_flags = flags;
        Ok(())
    }
    pub fn set_collision_solver(&mut self, solver: Option<Box<dyn CollisionSolver>>) {
        self.collision_solver = CollisionSolverHandle(solver);
    }
    pub fn collision_solver(&self) -> Option<&dyn CollisionSolver> {
        self.collision_solver.0.as_deref()
    }
    pub fn collision_solver_mut(&mut self) -> Option<&mut (dyn CollisionSolver + 'static)> {
        self.collision_solver.0.as_deref_mut()
    }
    pub fn set_collisions_enabled(
        &mut self,
        identity: u64,
        enabled: bool,
    ) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        if enabled {
            candidate.body_mut(identity)?.callback_flags |= 0x0800;
            candidate.change_collision_state(identity, enabled)?;
            candidate.body_mut(identity)?.callback_flags &= !0x0800;
        } else {
            candidate.change_collision_state(identity, enabled)?;
        }
        *self = candidate;
        Ok(())
    }
    fn change_collision_state(
        &mut self,
        identity: u64,
        enabled: bool,
    ) -> Result<(), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        if body.collisions_enabled == enabled {
            return Ok(());
        }
        let core = body.core_identity;
        if enabled {
            self.enable_spatial(core)?;
            self.body_mut(identity)?.collisions_enabled = true;
        } else {
            self.disable_body_contacts(core)?;
            self.remove_spatial_body(core)?;
            self.pairs.register_body(core)?;
            self.body_mut(identity)?.collisions_enabled = false;
            self.remove_body_contacts(core)?;
        }
        Ok(())
    }
    pub fn gravity_enabled(&self, body: u64) -> Result<bool, EnvironmentError> {
        self.controller_enabled(body, DEFAULT_CONTROLLER)
    }
    pub fn drag_enabled(&self, body: u64) -> Result<bool, EnvironmentError> {
        self.controller_enabled(body, AIR_CONTROLLER)
    }
    fn controller_enabled(&self, body: u64, controller: u64) -> Result<bool, EnvironmentError> {
        let core = self
            .body(body)
            .ok_or(EnvironmentError::MissingBody)?
            .core_identity;
        Ok(self
            .islands
            .core_controllers(core)
            .ok_or(crate::IslandError::MissingCore)?
            .contains(&controller))
    }
    fn set_controller_enabled(
        &mut self,
        body: u64,
        controller: u64,
        enabled: bool,
    ) -> Result<(), EnvironmentError> {
        let value = self.body(body).ok_or(EnvironmentError::MissingBody)?;
        if value.kind == BodyKind::Static || self.controller_enabled(body, controller)? == enabled {
            return Ok(());
        }
        let core = value.core_identity;
        if enabled {
            self.islands.attach(core, controller)?;
        } else {
            self.islands.detach(core, controller)?;
        }
        Ok(())
    }
    pub fn set_gravity_enabled(
        &mut self,
        body: u64,
        enabled: bool,
    ) -> Result<(), EnvironmentError> {
        self.set_controller_enabled(body, DEFAULT_CONTROLLER, enabled)
    }
    pub fn set_drag_enabled(&mut self, body: u64, enabled: bool) -> Result<(), EnvironmentError> {
        self.set_controller_enabled(body, AIR_CONTROLLER, enabled)
    }
    pub fn islands(&self) -> &crate::SimulationIslands {
        &self.islands
    }
    pub fn pending_wake_cores(&self) -> &[u64] {
        &self.pending_wake
    }
    pub fn active_bodies(&self) -> impl Iterator<Item = &RigidBody> {
        // Newly sleeping objects remain in this submission's publication roster.
        self.active_objects.iter().map(|core| {
            self.bodies.iter().find(|body|body.core_identity==*core).expect("active physical object")
        })
    }
    pub fn body(&self, identity: u64) -> Option<&RigidBody> {
        self.bodies.iter().find(|body| body.identity == identity)
    }
    pub fn pending_collisions(&self) -> impl Iterator<Item = ContinuousEvent> + '_ {
        self.queue
            .entries()
            .iter()
            .filter_map(|event| match event.identity {
                EnvironmentEvent::Collision(identity) => Some(ContinuousEvent {
                    identity,
                    time: event.time,
                }),
                EnvironmentEvent::Boundary | EnvironmentEvent::Convex(_) => None,
            })
    }
    pub fn collisions(&self) -> &[EnvironmentCollision] {
        &self.collisions
    }
    pub fn retired_impact_pairs(&self) -> &[RetiredImpactPair] {
        &self.retired_impact_pairs
    }

    pub fn create_body(&mut self, input: BodyInput) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        candidate.create_body_inner(input)?;
        *self = candidate;
        Ok(())
    }
    fn create_body_inner(&mut self, input: BodyInput) -> Result<(), EnvironmentError> {
        if self
            .bodies
            .iter()
            .any(|body| body.identity == input.identity)
        {
            return Err(EnvironmentError::DuplicateBody);
        }
        if self.bodies.len() == self.config.max_bodies {
            return Err(EnvironmentError::BodyLimit);
        }
        if input
            .position
            .iter()
            .chain(input.angles.iter())
            .chain(input.linear_velocity.iter())
            .chain(input.angular_velocity.iter())
            .chain(
                [
                    input.linear_damping,
                    input.angular_damping,
                    input.drag,
                    input.volume,
                ]
                .iter(),
            )
            .any(|component| !component.is_finite())
        {
            return Err(EnvironmentError::NonFinite);
        }
        if input.linear_damping < 0.0 || input.angular_damping < 0.0 || input.drag < 0.0 {
            return Err(EnvironmentError::Motion(MotionError::NegativeCoefficient));
        }
        let material_token = input.material as u16;
        let material = self
            .surfaces
            .surface_data(input.material as i32)
            .ok_or(EnvironmentError::DependencyMismatch)?
            .index;
        let mut physical = PhysicalShape::from_collision(
            &input.shape,
            input.mass,
            input.inertia_factor,
            input.rotational_inertia_limit,
        )?;
        if input.kind == BodyKind::Static {
            physical = physical.static_drag_bases();
        }
        let topology = if let Some(owner) = self
            .bodies
            .iter()
            .find(|body| Arc::ptr_eq(&body.shape, &input.shape))
        {
            Arc::clone(&owner.topology)
        } else {
            (0..input.shape.convex_count())
                .map(AuthoredHullRef::Piece)
                .chain(
                    (0..input
                        .shape
                        .authored_hierarchy()
                        .map_or(0, |h| h.enclosures.len()))
                        .map(AuthoredHullRef::Enclosure),
                )
                .map(|hull| FeatureTopology::from_collision_hull(&input.shape, hull))
                .collect::<Result<Vec<_>, _>>()?
                .into()
        };
        let (placement_orientation, orientation) =
            SourceAngleBasis::from_degrees(input.angles)?.body_orientations()?;
        let position = internal_position(input.position);
        let core_position =
            ObjectFrame::place_core(position, placement_orientation.matrix(), physical.center)?;
        let mut velocity = VelocityState {
            linear: [0.0; 3],
            angular: [0.0; 3],
        };
        let mut queued_velocity = if input.kind == BodyKind::Dynamic {
            QueuedVelocity {
                linear: internal_direction(input.linear_velocity, METERS_PER_INCH)
                    .map(|value| 0.0 + value),
                angular: internal_direction(input.angular_velocity, RADIANS_PER_DEGREE)
                    .map(|value| 0.0 + value),
            }
        } else {
            QueuedVelocity::default()
        };
        self.velocity_command_limits()
            .apply(&mut velocity, &mut queued_velocity)?;
        let creation_end = if self.clock.next_boundary() == 0.0 {
            f64::from(INITIAL_TIMESTEP)
        } else {
            self.clock.next_boundary()
        };
        let core_inverse_step = (1.0 / (creation_end - self.clock.current_time())) as f32;
        if !core_inverse_step.is_finite() || core_inverse_step <= 0.0 {
            return Err(EnvironmentError::InvalidMotionPhase);
        }
        let core_identity = self.next_core;
        let next_core = core_identity
            .checked_add(1)
            .ok_or(crate::IslandError::Identity)?;
        let mut islands = self.islands.clone();
        islands.register_core(core_identity, input.kind == BodyKind::Static)?;
        if input.kind == BodyKind::Dynamic {
            if input.gravity_enabled {
                islands.attach(core_identity, DEFAULT_CONTROLLER)?;
            }
            if input.drag_enabled {
                islands.attach(core_identity, AIR_CONTROLLER)?;
            }
        }
        self.bodies.insert(
            self.bodies.len() - self.pending_deletes.len(),
            RigidBody {
                material_token,
                volume: input.volume,
                buoyancy_ratio: fluid::volume_ratio(
                    physical.mass,
                    input.volume,
                    self.surfaces.records[material as usize].physics.density,
                )?,
                storage_identity: self.core_storage.acquire()?,
                crowded_contact_ordering: false,
                temporarily_frozen: false,
                callback_flags: 0x1127,
                quiet: crate::sleep::QuietMotion::new(),
                identity: input.identity,
                core_identity,
                shape: input.shape,
                physical,
                frame: ObjectFrame::from_center(physical.center)?,
                topology,
                material,
                kind: input.kind,
                core_position,
                previous_core_position: core_position,
                orientation,
                collision_orientation: None,
                previous_orientation: orientation,
                core_time: 0.0,
                core_inverse_step,
                velocity,
                queued_velocity,
                collision_count: 0,
                linear_damping: input.linear_damping,
                angular_damping: input.angular_damping,
                drag: if input.kind == BodyKind::Static {
                    0.0
                } else {
                    input.drag
                },
                collisions_enabled: input.collisions_enabled,
                motion_enabled: true,
                asleep: true,
                movement_range: crate::MovementRange::new(
                    crate::continuous::SORTED_EVENT_CAPACITY,
                )?,
                motion_phase: None,
                publication_phase: None,
            },
        );
        self.islands = islands;
        self.next_core = next_core;
        self.pairs.register_body(core_identity)?;
        if input.collisions_enabled {
            self.recheck_spatial(core_identity)?;
        }
        Ok(())
    }

    pub fn destroy_body(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        let index = candidate
            .bodies()
            .iter()
            .position(|body| body.identity == identity)
            .ok_or(EnvironmentError::MissingBody)?;
        let last = candidate.bodies().len() - 1;
        candidate.bodies.swap(index, last);
        if candidate.delete_queue_enabled {
            candidate.bodies[last].callback_flags |= 0x0400;
            candidate.pending_deletes.push(identity);
        } else {
            candidate.destroy_body_inner(identity)?;
        }
        *self = candidate;
        Ok(())
    }
    pub fn enable_delete_queue(&mut self, enabled: bool) {
        self.delete_queue_enabled = enabled;
    }
    pub fn set_buoyancy_ratio(
        &mut self,
        identity: u64,
        ratio: f32,
    ) -> Result<(), EnvironmentError> {
        if !ratio.is_finite() {
            return Err(EnvironmentError::NonFinite);
        }
        self.body_mut(identity)?.buoyancy_ratio = ratio;
        Ok(())
    }
    pub fn cleanup_delete_list(&mut self) -> Result<(), EnvironmentError> {
        if self.pending_deletes.is_empty() {return Ok(());}
        let mut candidate = self.clone();
        candidate.cleanup_deleted_bodies()?;
        *self = candidate;
        Ok(())
    }
    fn cleanup_deleted_bodies(&mut self) -> Result<(), EnvironmentError> {
        for identity in std::mem::take(&mut self.pending_deletes) {
            self.destroy_body_inner(identity)?;
        }
        Ok(())
    }
    fn destroy_body_inner(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let index = self
            .bodies
            .iter()
            .position(|body| body.identity == identity)
            .ok_or(EnvironmentError::MissingBody)?;
        let removed = self
            .queued_collisions
            .iter()
            .filter(|event| event.input.bodies().contains(&identity))
            .map(|event| event.identity)
            .collect::<Vec<_>>();
        for event in removed {
            self.cancel_collision(event)?;
        }
        self.callbacks.detached_body = Some(identity);
        self.destroy_body_shadow(identity)?;
        self.remove_body_contacts(self.bodies[index].core_identity)?;
        self.remove_spatial_body(self.bodies[index].core_identity)?;
        self.fluids.remove_body(identity)?;
        self.forget_fluid_core(self.bodies[index].core_identity);
        self.transforms.invalidate(identity)?;
        let core = self.bodies[index].core_identity;
        if let Some(index) = self.active_objects.iter().position(|value| *value == core) {
            self.active_objects.swap_remove(index);
        }
        self.islands.remove_core(core)?;
        self.pending_wake.retain(|value| *value != core);
        let removed = self.bodies.swap_remove(index);
        self.core_storage.release(removed.storage_identity)?;
        self.callbacks.detached_body = None;
        Ok(())
    }

    pub fn wake(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let time = self.time();
        let body = self.body_mut(identity)?;
        if body.kind == BodyKind::Static {
            return Ok(());
        }
        body.asleep = false;
        let core = body.core_identity;
        if self.islands.movement(core) != Some(crate::CoreMovement::Dormant) {
            self.body_mut(identity)?.quiet.refresh_time(time);
            return Ok(());
        }
        if self.islands.movement(core) == Some(crate::CoreMovement::Dormant)
            && !self.pending_wake.contains(&core)
        {
            self.pending_wake.push(core);
        }
        Ok(())
    }

    /// Source world-space force impulse, in kg inches/second.
    pub fn apply_force_center(
        &mut self,
        identity: u64,
        force: [f32; 3],
    ) -> Result<(), EnvironmentError> {
        self.apply_impulse_command(identity, crate::impulses::ImpulseCommand::Center(force))
    }

    pub fn apply_force_offset(
        &mut self,
        identity: u64,
        force: [f32; 3],
        point: [f32; 3],
    ) -> Result<(), EnvironmentError> {
        self.apply_impulse_command(
            identity,
            crate::impulses::ImpulseCommand::Offset { force, point },
        )
    }

    /// Source world-axis torque impulse, in kg degrees/second.
    pub fn apply_torque_center(
        &mut self,
        identity: u64,
        torque: [f32; 3],
    ) -> Result<(), EnvironmentError> {
        self.apply_impulse_command(identity, crate::impulses::ImpulseCommand::Torque(torque))
    }

    /// Returns center force and body-local torque in Source units.
    pub fn calculate_force_offset(
        &self,
        identity: u64,
        force: [f32; 3],
        point: [f32; 3],
    ) -> Result<([f32; 3], [f32; 3]), EnvironmentError> {
        Ok(self
            .impulse_frame(identity)?
            .calculate_force(force, point)?)
    }

    /// Returns world linear and body-local angular velocity in Source units.
    pub fn calculate_velocity_offset(
        &self,
        identity: u64,
        force: [f32; 3],
        point: [f32; 3],
    ) -> Result<([f32; 3], [f32; 3]), EnvironmentError> {
        Ok(self
            .impulse_frame(identity)?
            .calculate_velocity(force, point)?)
    }
    pub fn velocity_at_point(
        &self,
        identity: u64,
        point: [f32; 3],
    ) -> Result<[f32; 3], EnvironmentError> {
        if point.iter().any(|v| !v.is_finite()) {
            return Err(EnvironmentError::NonFinite);
        }
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        let frame = self.impulse_frame(identity)?;
        let angular = body.reported_velocity().angular.map(f64::from);
        let world_angular: [f32; 3] = std::array::from_fn(|row| {
            let x = frame.orientation[row * 3] * angular[0];
            let y = frame.orientation[row * 3 + 1] * angular[1];
            ((x + y) + frame.orientation[row * 3 + 2] * angular[2]) as f32
        });
        let point = internal_position(point);
        let offset =
            std::array::from_fn::<_, 3, _>(|axis| (point[axis] - frame.position[axis]) as f32);
        let rotation = [
            world_angular[1] * offset[2] - world_angular[2] * offset[1],
            world_angular[2] * offset[0] - world_angular[0] * offset[2],
            world_angular[0] * offset[1] - world_angular[1] * offset[0],
        ];
        let result = source_direction(
            std::array::from_fn(|axis| {
                (rotation[axis] + body.velocity.linear[axis]) + body.queued_velocity.linear[axis]
            }),
            INCHES_PER_METER,
        );
        if result.iter().any(|v| !v.is_finite()) {
            return Err(EnvironmentError::NonFinite);
        }
        Ok(result)
    }

    fn impulse_frame(
        &self,
        identity: u64,
    ) -> Result<crate::impulses::ImpulseFrame, EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        Ok(body.integration_frame())
    }
    fn sampled_impulse_frame(
        &self,
        identity: u64,
    ) -> Result<crate::impulses::ImpulseFrame, EnvironmentError> {
        let mut frame = self.impulse_frame(identity)?;
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        let elapsed = f64::from((self.time() - body.core_time) as f32);
        if !body.asleep
            && body.kind != BodyKind::Static
            && elapsed != 0.0
            && let Some(phase) = body.motion_phase()
        {
            let next = if body.collision_orientation.is_some() {
                body.orientation
            } else {
                phase.next_orientation
            };
            frame.orientation = phase
                .prior_orientation
                .interpolate(next, elapsed * f64::from(body.core_inverse_step))?
                .matrix();
            frame.position = std::array::from_fn(|axis| {
                phase.position[axis] + f64::from(phase.projection_velocity[axis]) * elapsed
            });
        }
        Ok(frame)
    }

    fn apply_impulse_command(
        &mut self,
        identity: u64,
        command: crate::impulses::ImpulseCommand,
    ) -> Result<(), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        if !body.is_moveable() {
            return Ok(());
        }
        let mut active = body.velocity;
        let mut queued = body.queued_velocity;
        self.impulse_frame(identity)?.queue(command, &mut queued)?;
        self.velocity_command_limits()
            .apply(&mut active, &mut queued)?;
        let body = self.body_mut(identity)?;
        body.velocity = active;
        body.queued_velocity = queued;
        self.wake(identity)
    }

    pub fn sleep(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        candidate.sleep_inner(identity)?;
        *self = candidate;
        Ok(())
    }
    fn sleep_inner(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        if body.kind == BodyKind::Static {
            return Ok(());
        }
        let core = body.core_identity;
        self.pending_wake.retain(|value| *value != core);
        if !self
            .islands
            .movement(core)
            .is_some_and(crate::CoreMovement::is_simulated)
        {
            self.body_mut(identity)?.asleep = true;
            return Ok(());
        }
        self.remove_body_contacts(core)?;
        let now = self.time();
        let body = self.body_mut(identity)?;
        let phase = body.motion_phase();
        body.quiet.request_sleep(
            phase.map_or(body.core_position, |phase| phase.position),
            phase.map_or(body.orientation.quaternion, |phase| {
                phase.next_orientation.quaternion
            }),
            now,
        )?;
        let island = self
            .islands
            .island_of(core)
            .ok_or(crate::IslandError::MissingCore)?;
        self.islands.rebuild(island)?;
        self.islands.resolve_connectivity(island)?;
        let island = self
            .islands
            .island_of(core)
            .ok_or(crate::IslandError::MissingCore)?;
        self.check_island_movement(island, now)?;
        for core in self
            .islands
            .island(island)
            .ok_or(crate::IslandError::MissingIsland)?
            .cores
            .clone()
        {
            let index = self.core_body_index(core)?;
            let body = &mut self.bodies[index];
            if body.asleep {
                body.motion_phase = None;
                body.publication_phase = None;
                body.previous_core_position = body.core_position;
                body.previous_orientation = body.orientation;
            }
        }
        Ok(())
    }

    pub fn set_motion_enabled(
        &mut self,
        identity: u64,
        enabled: bool,
    ) -> Result<(), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        if body.kind == BodyKind::Static || body.motion_enabled == enabled {
            return Ok(());
        }
        let core = body.core_identity;
        let collisions = body.collisions_enabled;
        let mut candidate = self.clone();
        let body = candidate.body_mut(identity)?;
        body.velocity = VelocityState {
            linear: [0.0; 3],
            angular: [0.0; 3],
        };
        body.motion_enabled = enabled;
        if enabled {
            candidate.recheck_spatial(core)?;
        } else {
            candidate.change_collision_state(identity, false)?;
            candidate.body_mut(identity)?.queued_velocity = QueuedVelocity::default();
            candidate.change_collision_state(identity, collisions)?;
        }
        candidate.recheck_collision_filter_inner(identity)?;
        candidate.recheck_contact_points_inner(identity)?;
        *self = candidate;
        Ok(())
    }
    pub fn recheck_collision_filter(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        candidate.recheck_collision_filter_inner(identity)?;
        *self = candidate;
        Ok(())
    }
    fn recheck_collision_filter_inner(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        if body.callback_flags & 0x0400 != 0 {
            return Ok(());
        }
        let core = body.core_identity;
        self.body_mut(identity)?.callback_flags |= 0x0800;
        self.recheck_spatial(core)?;
        self.body_mut(identity)?.callback_flags &= !0x0800;
        Ok(())
    }
    pub fn recheck_contact_points(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        candidate.recheck_contact_points_inner(identity)?;
        *self = candidate;
        Ok(())
    }

    pub fn set_motion_phase(
        &mut self,
        identity: u64,
        phase: BodyMotionPhase,
    ) -> Result<(), EnvironmentError> {
        if !phase.start.is_finite() || !phase.end.is_finite() || phase.start >= phase.end {
            return Err(EnvironmentError::InvalidMotionPhase);
        }
        phase.sample(phase.start)?;
        phase.sample(phase.end)?;
        let body = self.body_mut(identity)?;
        if body.kind == BodyKind::Static || !body.motion_enabled {
            return Err(EnvironmentError::DisabledMotion);
        }
        body.retain_motion_phase(phase)?;
        let core = body.core_identity;
        if !body.asleep {
            self.pending_wake.retain(|value| *value != core);
            self.islands
                .set_movement(core, crate::CoreMovement::Moving)?;
            self.islands.activate(
                self.islands
                    .island_of(core)
                    .ok_or(crate::IslandError::MissingCore)?,
            )?;
        }
        Ok(())
    }

    pub fn schedule_collision(
        &mut self,
        identity: u64,
        candidate_time: f64,
        collision: BodyCollision,
    ) -> Result<ContinuousEventTime, EnvironmentError> {
        self.validate_collision(collision)?;
        for body in [collision.body, collision.opposing] {
            let body = self.body(body).ok_or(EnvironmentError::MissingBody)?;
            if body.kind == BodyKind::Dynamic && body.motion_enabled {
                let phase = body
                    .motion_phase()
                    .ok_or(EnvironmentError::MissingMotionPhase)?;
                if candidate_time < phase.start || candidate_time > phase.end {
                    return Err(EnvironmentError::InvalidMotionPhase);
                }
            }
        }
        let scheduled = self
            .queue
            .insert(EnvironmentEvent::Collision(identity), candidate_time)?;
        self.queued_collisions.push(QueuedCollision {
            identity,
            input: QueuedCollisionInput::Direct(collision),
        });
        Ok(scheduled.time)
    }

    pub fn schedule_convex_pair(
        &mut self,
        identity: u64,
        input: ConvexContactPair,
    ) -> Result<Option<ConvexPairEvent>, EnvironmentError> {
        if self
            .queue
            .entries()
            .iter()
            .any(|event| event.identity == EnvironmentEvent::Collision(identity))
        {
            return Err(ContinuousError::DuplicateEvent.into());
        }
        let mut transforms = self.transforms.clone();
        let predicted = self.query_convex_pair(input, &mut transforms)?;
        for endpoint in input.endpoints {
            transforms.release(endpoint.body)?;
        }
        let Some((pair, predicted)) = predicted else {
            self.transforms = transforms;
            return Ok(None);
        };
        self.queue
            .insert(EnvironmentEvent::Collision(identity), predicted.time)?;
        self.queued_collisions.push(QueuedCollision {
            identity,
            input: QueuedCollisionInput::Pair { pair, predicted },
        });
        self.transforms = transforms;
        Ok(Some(predicted))
    }

    fn query_convex_pair(
        &self,
        mut input: ConvexContactPair,
        transforms: &mut TransformCache,
    ) -> Result<Option<(ConvexContactPair, ConvexPairEvent)>, EnvironmentError> {
        if input.endpoints[0].body == input.endpoints[1].body {
            return Err(EnvironmentError::IdenticalBodies);
        }
        if input.start != self.clock.current_time() {
            return Err(EnvironmentError::InvalidMotionPhase);
        }
        let bodies = [
            self.body(input.endpoints[0].body)
                .ok_or(EnvironmentError::MissingBody)?,
            self.body(input.endpoints[1].body)
                .ok_or(EnvironmentError::MissingBody)?,
        ];
        if !bodies
            .iter()
            .any(|body| body.kind == BodyKind::Dynamic && body.motion_enabled)
        {
            return Err(EnvironmentError::DisabledMotion);
        }
        if bodies.iter().any(|body| !body.collisions_enabled) {
            return Err(EnvironmentError::DisabledCollision);
        }
        let mut topologies = [None; 2];
        for side in 0..2 {
            topologies[side] = Some(bodies[side].topology(input.endpoints[side].convex).ok_or(
                TopologyError::MissingAuthoredConvex {
                    convex: input.endpoints[side].convex,
                },
            )?);
        }
        let topologies = topologies.map(Option::unwrap);
        let mut poses = [ProjectionKnot {
            position: [0.0; 3],
            orientation: [0.0; 9],
        }; 2];
        for (index, body) in input
            .endpoints
            .map(|endpoint| endpoint.body)
            .into_iter()
            .enumerate()
        {
            let (activity, state) = self.transform_input(body)?;
            poses[index] = transforms
                .resolve(body, activity, self.clock.time_code(), state)?
                .object;
            transforms.pin(body)?;
        }
        let mut motions = poses.map(FeatureMotion::Stationary);
        let mut bounds = [CollisionMotion::stationary(); 2];
        for side in 0..2 {
            if bodies[side].kind == BodyKind::Dynamic && bodies[side].motion_enabled {
                let phase = bodies[side]
                    .motion_phase()
                    .ok_or(EnvironmentError::MissingMotionPhase)?;
                motions[side] = FeatureMotion::Moving {
                    phase,
                    frame: bodies[side].frame,
                    cache_time: input.start,
                    cached: poses[side],
                };
                bounds[side] = bodies[side]
                    .collision_motion()
                    .ok_or(EnvironmentError::MissingMotionPhase)?;
            }
        }
        let query = ConvexPairQuery {
            endpoints: std::array::from_fn(|side| ConvexEndpoint {
                topology: topologies[side],
                physical: &bodies[side].physical,
                motion: motions[side],
                bounds: bounds[side],
            }),
            seed: input.seed,
            start: input.start,
            end: input.end,
            extra_radius: 0.0,
            tolerances: self.tolerances,
            maximum_feature_transitions: input.maximum_feature_transitions,
        }
        .next()?;
        let Some(mut predicted) = query.event else {
            return Ok(None);
        };
        let Some(adjusted) = (ContinuousEventDelay {
            separation: query.separation,
            collision_distance: self.tolerances.real_surface,
            speed: query.bounds.worst_case_speed,
            timestep: self.config.timestep,
            scale: 1.0,
            current_time: input.start,
            proposed_time: predicted.time,
            phase_end: input.end,
            hint: EventTimingHint::ShortDelay,
            kind: predicted.kind,
        })
        .candidate()?
        else {
            return Ok(None);
        };
        predicted.time = adjusted;
        input.endpoints = query.selection.order().map(|side| input.endpoints[side]);
        input.seed = query.selection.ordered_pair();
        Ok(Some((input, predicted)))
    }

    pub fn cancel_collision(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let position = self
            .queued_collisions
            .iter()
            .position(|event| event.identity == identity)
            .ok_or(ContinuousError::MissingEvent)?;
        self.queue.remove(EnvironmentEvent::Collision(identity))?;
        self.queued_collisions.remove(position);
        Ok(())
    }

    pub fn rekey_collision(
        &mut self,
        identity: u64,
        candidate_time: f64,
    ) -> Result<ContinuousEventTime, EnvironmentError> {
        let queued = self
            .queued_collisions
            .iter()
            .find(|event| event.identity == identity)
            .ok_or(ContinuousError::MissingEvent)?;
        let QueuedCollisionInput::Direct(collision) = queued.input else {
            return Err(EnvironmentError::DerivedEventTime);
        };
        self.validate_collision(collision)?;
        for identity in [collision.body, collision.opposing] {
            let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
            if body.kind == BodyKind::Dynamic && body.motion_enabled {
                let phase = body
                    .motion_phase()
                    .ok_or(EnvironmentError::MissingMotionPhase)?;
                if candidate_time < phase.start || candidate_time > phase.end {
                    return Err(EnvironmentError::InvalidMotionPhase);
                }
            }
        }
        Ok(self
            .queue
            .update(EnvironmentEvent::Collision(identity), candidate_time)?
            .time)
    }

    /// Dispatches events strictly before `terminal`, crossing at most two integration boundaries.
    /// Returns collision/feature-transition dispatches; fixed boundaries are excluded from the count.
    pub fn advance_events_before(&mut self, terminal: f64) -> Result<usize, EnvironmentError> {
        if !terminal.is_finite() {
            return Err(EnvironmentError::NonFinite);
        }
        self.advance_events(Some(terminal))
    }

    /// Dispatches through the next integration boundary and prepares the following motion phase.
    /// Earlier/equal collision keys retain queue order; the boundary is excluded from the returned count.
    pub fn advance_boundary(&mut self) -> Result<usize, EnvironmentError> {
        self.advance_events(None)
    }

    fn advance_events(&mut self, terminal: Option<f64>) -> Result<usize, EnvironmentError> {
        if self.paused {
            return Ok(0);
        }
        let mut candidate = self.clone();
        let dispatched = candidate.advance_events_inner(terminal)?;
        *self = candidate;
        Ok(dispatched)
    }

    fn core_body_index(&self, core: u64) -> Result<usize, EnvironmentError> {
        self.bodies
            .iter()
            .position(|body| body.core_identity == core)
            .ok_or(EnvironmentError::MissingBody)
    }

    fn revive_island(&mut self, island: u64) -> Result<(), EnvironmentError> {
        loop {
            let members = self
                .islands
                .island(island)
                .ok_or(crate::IslandError::MissingIsland)?
                .cores
                .clone();
            let mut restart = false;
            for core in members.into_iter().rev() {
                if self
                    .islands
                    .movement(core)
                    .is_some_and(crate::CoreMovement::is_simulated)
                {
                    continue;
                }
                let index = self.core_body_index(core)?;
                if self.bodies[index].kind == BodyKind::Static {
                    continue;
                }
                self.islands
                    .set_movement(core, crate::CoreMovement::Discovering)?;
                let now = self.time();
                let end = self.clock.next_boundary();
                let body = &mut self.bodies[index];
                body.quiet.refresh_time(now);
                if body.core_time != 0.0 {
                    body.velocity.linear = [0.0; 3];
                }
                body.core_time = now;
                let remaining = (end - now) as f32;
                body.core_inverse_step = if f64::from(remaining) > 1.0e-10 {
                    (1.0 / f64::from(remaining)) as f32
                } else {
                    f32::from_bits(0x5015_02f9)
                };
                body.orientation = body.previous_orientation;
                body.motion_phase = (end > now).then_some(RetainedMotionPhase {
                    phase: BodyMotionPhase {
                        position: body.core_position,
                        prior_orientation: body.orientation,
                        next_orientation: body.orientation,
                        projection_velocity: [0.0; 3],
                        start: now,
                        end,
                        inverse_step: body.core_inverse_step,
                    },
                    motion: CollisionMotion::stationary(),
                });
                body.publication_phase = None;
                body.asleep = false;
                self.recheck_spatial(core)?;
                self.finish_fluid_revival(core)?;
                self.islands
                    .set_movement(core, crate::CoreMovement::Moving)?;
                let grew = self.grow_revival_contacts(core)?;
                self.bodies[index].asleep = false;
                if !self.active_objects.contains(&core) {
                    self.active_objects.push(core);
                }
                self.emit_object_callback(self.bodies[index].identity, true)?;
                if grew {
                    restart = true;
                    break;
                }
            }
            if !restart {
                break;
            }
        }
        self.islands.activate(island)?;
        Ok(())
    }

    fn advance_islands(&mut self, boundary: f64) -> Result<(), EnvironmentError> {
        for index in (0..self.pending_wake.len()).rev() {
            let core = self.pending_wake[index];
            let island = self
                .islands
                .island_of(core)
                .ok_or(crate::IslandError::MissingCore)?;
            self.revive_island(island)?;
        }
        self.pending_wake.clear();
        let active = self.islands.phase_order().collect::<Vec<_>>();
        let mut touched = Vec::new();
        for island in active {
            let unit = self
                .islands
                .island(island)
                .ok_or(crate::IslandError::MissingIsland)?;
            let members = unit.cores.clone();
            let controllers = unit.controllers.clone();
            let mut prepared = Vec::new();
            let mut velocities = Vec::with_capacity(members.len());
            for core in members.iter().rev() {
                let index = self.core_body_index(*core)?;
                if self.bodies[index].prepare_boundary(boundary)? {
                    prepared.push(*core);
                }
                velocities.push(self.bodies[index].velocity.linear);
            }
            if self.islands.activity_mut(island)?.advance(&velocities)? {
                for core in &members {
                    let index = self.core_body_index(*core)?;
                    self.bodies[index].quiet.refresh_time(boundary);
                }
            }
            let check_movement = self.sleep_scheduler.advance();
            for controller in controllers.iter().rev() {
                if self.run_shadow_controller(controller.controller)? {
                    continue;
                }
                if self.run_fluid_controller(controller.controller)? {
                    continue;
                }
                if self.contact_controller(controller.controller)? {
                    continue;
                }
                for core in controller.cores.iter().rev() {
                    if !prepared.contains(core) {
                        continue;
                    }
                    let index = self.core_body_index(*core)?;
                    let body = &mut self.bodies[index];
                    let mut profile = body.motion_profile(self.config);
                    profile.settling = matches!(
                        self.islands.movement(*core),
                        Some(crate::CoreMovement::Slow | crate::CoreMovement::Calm)
                    );
                    match controller.controller {
                        DEFAULT_CONTROLLER => {
                            body.velocity = profile.accelerate(
                                body.velocity,
                                &mut body.queued_velocity,
                                !body.motion_enabled,
                            )?;
                        }
                        AIR_CONTROLLER => {
                            let factors =
                                profile.aerodynamic_factors(body.velocity, body.orientation)?;
                            body.velocity = profile.apply_aerodynamics(body.velocity, factors)?;
                        }
                        _ => return Err(crate::IslandError::MissingController.into()),
                    }
                }
            }
            touched.extend(prepared);
            if check_movement {
                self.check_island_movement(island, boundary)?;
            }
            for core in members.iter().rev() {
                self.recheck_invalid_body(*core)?;
            }
            if self
                .islands
                .island(island)
                .ok_or(crate::IslandError::MissingIsland)?
                .connectivity_dirty
            {
                self.islands.rebuild(island)?;
                self.islands.resolve_connectivity(island)?;
            }
            if self.islands.island(island).unwrap().connectivity_dirty {
                self.islands.resolve_connectivity(island)?;
            }
        }
        let mut active_ranges = Vec::new();
        for core in touched.into_iter().rev() {
            let index = self.core_body_index(core)?;
            self.bodies[index].commit_boundary(self.config, boundary)?;
            if self.bodies[index].movement_range.is_due() {
                active_ranges.push(core);
            }
        }
        self.dispatch_ranges(&active_ranges)?;
        self.refresh_exact_pairs()?;
        Ok(())
    }

    fn check_island_movement(
        &mut self,
        island: u64,
        boundary: f64,
    ) -> Result<(), EnvironmentError> {
        let members = self
            .islands
            .island(island)
            .ok_or(crate::IslandError::MissingIsland)?
            .cores
            .clone();
        let mut calm = true;
        for core in members.iter().rev() {
            let index = self.core_body_index(*core)?;
            let body = &mut self.bodies[index];
            let phase = body.motion_phase();
            let position = phase.map_or(body.core_position, |p| p.position);
            let orientation = phase.map_or(body.orientation, |p| p.next_orientation);
            let recent = phase.map_or(body.orientation, |p| p.prior_orientation);
            let state = body.quiet.advance(
                crate::SleepSample {
                    reference: body.quiet.reference(),
                    position,
                    orientation: orientation.quaternion,
                    recent_orientation: recent.quaternion.map(|v| v as f32),
                    angular_velocity: body.velocity.angular,
                    radius: body.physical.radius,
                    quiet_interval: 0.3,
                    now: boundary,
                },
                recent.quaternion,
            )?;
            calm &= state == crate::SleepState::Sleeping;
            self.islands.set_movement(
                *core,
                match state {
                    crate::SleepState::Moving => crate::CoreMovement::Moving,
                    crate::SleepState::QuietPending => crate::CoreMovement::Slow,
                    crate::SleepState::Sleeping => crate::CoreMovement::Calm,
                },
            )?;
        }
        if calm {
            for core in members.iter().rev() {
                let index = self.core_body_index(*core)?;
                let body = &mut self.bodies[index];
                body.velocity = VelocityState {
                    linear: [0.0; 3],
                    angular: [0.0; 3],
                };
                body.queued_velocity = QueuedVelocity::default();
                if let Some(mut phase) = body.motion_phase() {
                    body.core_position = phase.position;
                    body.orientation = phase.prior_orientation;
                    phase.next_orientation = phase.prior_orientation;
                    phase.projection_velocity = [0.0; 3];
                    body.motion_phase = Some(RetainedMotionPhase {
                        phase,
                        motion: CollisionMotion::stationary(),
                    });
                }
                body.asleep = true;
                self.islands
                    .set_movement(*core, crate::CoreMovement::Dormant)?;
                self.recheck_spatial(*core)?;
                self.bodies[index].movement_range.stop_gradients(boundary)?;
                self.reset_body_ranges(*core)?;
                self.transforms.invalidate(self.bodies[index].identity)?;
                self.emit_object_callback(self.bodies[index].identity, false)?;
            }
            self.islands.freeze(island)?;
        }
        Ok(())
    }

    fn advance_events_inner(&mut self, terminal: Option<f64>) -> Result<usize, EnvironmentError> {
        let candidate = self;
        let mut dispatched = 0;
        let mut boundaries = 0;
        loop {
            let event = match terminal {
                Some(terminal) => candidate.queue.pop_before(terminal)?,
                None => candidate.queue.pop_next(),
            };
            let Some(event) = event else {
                if terminal.is_none() {
                    return Err(ContinuousError::MissingEvent.into());
                }
                break;
            };
            if event.identity == EnvironmentEvent::Boundary {
                if boundaries == crate::continuous::SORTED_EVENT_CAPACITY {
                    return Err(EnvironmentError::StepLimit);
                }
                candidate.clock.cross_boundary(event.time.absolute)?;
                candidate.queue.reset(event.time.absolute)?;
                candidate.advance_islands(event.time.absolute)?;
                candidate
                    .queue
                    .insert(EnvironmentEvent::Boundary, candidate.clock.next_boundary())?;
                boundaries += 1;
                if terminal.is_none() {
                    break;
                }
                continue;
            }
            if dispatched >= candidate.config.max_events {
                return Err(EnvironmentError::CollisionLimit);
            }
            candidate.clock.visit(event.time.absolute)?;
            let (identity, automatic, queued) = match event.identity {
                EnvironmentEvent::Collision(identity) => {
                    let position = candidate
                        .queued_collisions
                        .iter()
                        .position(|queued| queued.identity == identity)
                        .ok_or(ContinuousError::MissingEvent)?;
                    (
                        identity,
                        false,
                        candidate.queued_collisions.remove(position),
                    )
                }
                EnvironmentEvent::Convex(identity) => {
                    let Some(queued) = candidate.convex_event(identity)? else {
                        dispatched += 1;
                        continue;
                    };
                    (identity, true, queued)
                }
                EnvironmentEvent::Boundary => unreachable!(),
            };
            if let QueuedCollisionInput::Pair {
                mut pair,
                predicted,
            } = queued.input
                && predicted.kind == EventTimingKind::FeatureTransition
            {
                pair.start = event.time.absolute;
                if pair.start < pair.end {
                    if automatic {
                        candidate.after_convex_event(identity)?;
                    } else {
                        candidate.schedule_convex_pair(identity, pair)?;
                    }
                }
                dispatched += 1;
                continue;
            }
            let ids = queued.input.bodies();
            if let QueuedCollisionInput::Direct(collision) = queued.input {
                candidate.validate_collision(collision)?;
            }
            for id in ids {
                let body = candidate.body(id).ok_or(EnvironmentError::MissingBody)?;
                if body.kind != BodyKind::Static
                    && candidate.islands.movement(body.core_identity)
                        == Some(crate::CoreMovement::Dormant)
                {
                    let island = candidate
                        .islands
                        .island_of(body.core_identity)
                        .ok_or(crate::IslandError::MissingCore)?;
                    candidate.revive_island(island)?;
                }
            }
            let cache_poses = if matches!(queued.input, QueuedCollisionInput::Pair { .. }) {
                let body = candidate.cached_transform(ids[0])?.object;
                candidate.transforms.pin(ids[0])?;
                let fixed = candidate.cached_transform(ids[1])?.object;
                candidate.transforms.pin(ids[1])?;
                Some([body, fixed])
            } else {
                None
            };
            let dispatch = if cache_poses.is_some() {
                Some(CollisionDispatch {
                    bodies: ids,
                    time_code: candidate.clock.time_code(),
                    transforms: [
                        candidate
                            .transforms
                            .current(ids[0])
                            .ok_or(EnvironmentError::SnapshotMismatch)?
                            .1,
                        candidate
                            .transforms
                            .current(ids[1])
                            .ok_or(EnvironmentError::SnapshotMismatch)?
                            .1,
                    ],
                })
            } else {
                None
            };
            for identity in ids {
                let body = candidate.body_mut(identity)?;
                if body.kind == BodyKind::Static || !body.motion_enabled {
                    continue;
                }
                let phase = body
                    .motion_phase()
                    .ok_or(EnvironmentError::MissingMotionPhase)?;
                body.previous_core_position = phase.position;
                body.previous_orientation = phase.prior_orientation;
                body.synchronize_collision_pose(event.time.absolute)?;
            }
            let (collision, geometry) = match queued.input {
                QueuedCollisionInput::Direct(collision) => (collision, None),
                QueuedCollisionInput::Pair { pair, .. } => {
                    let (collision, geometry) = candidate.project_pair_contact(
                        pair,
                        cache_poses.expect("feature dispatch acquired both endpoint poses"),
                    )?;
                    (collision, Some(geometry))
                }
            };
            if cache_poses.is_some() {
                for identity in ids {
                    candidate.transforms.release(identity)?;
                }
            }
            let admitted = if automatic {
                let source = candidate.collision_source_cores(identity)?;
                let preferred = if candidate.islands.movement(source[0])
                    == Some(crate::CoreMovement::Dormant)
                {
                    source[0]
                } else {
                    source[1]
                };
                Some(candidate.admit_contact(
                    geometry.ok_or(EnvironmentError::SnapshotMismatch)?,
                    preferred,
                )?)
            } else {
                None
            };
            let materials = if let Some(geometry) = geometry {
                let side = usize::from(geometry.endpoints[0].body != collision.body);
                [geometry.materials[side], geometry.materials[1 - side]]
            } else {
                [
                    candidate.body(collision.body).unwrap().material,
                    candidate.body(collision.opposing).unwrap().material,
                ]
            };
            let callback = if let (Some(contact), Some(geometry)) = (admitted, geometry) {
                let elapsed = candidate.contact_impact_elapsed(contact)?;
                let callback = candidate.collision_callback_data(geometry, elapsed)?;
                if let Some(data) = callback {
                    candidate.emit_collision_callback(geometry, data, None)?;
                }
                callback
            } else {
                None
            };
            let contact_velocity = if callback.is_some() {
                Some(candidate.collision_callback_velocity(
                    geometry.unwrap(),
                    candidate.collision_source_cores(identity)?,
                )?)
            } else {
                None
            };
            candidate.resolve_material_collision(collision, Some(materials))?;
            let journal = candidate.collisions.last_mut().expect("collision journal");
            journal.event_identity = Some(identity);
            journal.event_time = event.time.absolute;
            journal.geometry = geometry;
            journal.dispatch = dispatch;
            let applied = journal.applied;
            if let Some(contact) = admitted {
                candidate.finish_contact_impact(contact, identity)?;
                if let Some(data) = callback {
                    candidate.emit_collision_callback(geometry.unwrap(), data, contact_velocity)?;
                }
                dispatched += 1;
                continue;
            }
            let mut changed_ranges = Vec::new();
            for (side, identity) in [collision.body, collision.opposing].into_iter().enumerate() {
                if !applied[side] {
                    continue;
                }
                let config = candidate.config;
                let body = candidate.body_mut(identity)?;
                if body.kind == BodyKind::Static || !body.motion_enabled {
                    continue;
                }
                body.velocity = body
                    .motion_profile(config)
                    .constrain_velocity(body.velocity)?;
                let previous = body
                    .motion_phase()
                    .ok_or(EnvironmentError::MissingMotionPhase)?;
                let remaining = (previous.end - event.time.absolute) as f32;
                if remaining <= 0.0 {
                    body.motion_phase = None;
                    continue;
                }
                let (next_orientation, motion) = body.integrate_rotation(remaining, false)?;
                body.retain_integrated_phase(
                    BodyMotionPhase {
                        position: body.core_position,
                        prior_orientation: body.orientation,
                        next_orientation,
                        projection_velocity: body.velocity.linear,
                        start: event.time.absolute,
                        end: previous.end,
                        inverse_step: (1.0 / f64::from(remaining)) as f32,
                    },
                    motion,
                )?;
                if body.movement_range.is_due() {
                    changed_ranges.push(body.core_identity);
                }
            }
            if automatic {
                candidate.dispatch_ranges(&changed_ranges)?;
                candidate.after_convex_event(identity)?;
            }
            dispatched += 1;
        }
        if let Some(terminal) = terminal {
            candidate.clock.finish_submission(terminal)?;
        }
        Ok(dispatched)
    }

    fn project_pair_contact(
        &self,
        pair: ConvexContactPair,
        poses: [ProjectionKnot; 2],
    ) -> Result<(BodyCollision, ContactGeometry), EnvironmentError> {
        let bodies = [
            self.body(pair.endpoints[0].body)
                .ok_or(EnvironmentError::MissingBody)?,
            self.body(pair.endpoints[1].body)
                .ok_or(EnvironmentError::MissingBody)?,
        ];
        let mut topologies = [None; 2];
        for side in 0..2 {
            topologies[side] = Some(bodies[side].topology(pair.endpoints[side].convex).ok_or(
                TopologyError::MissingAuthoredConvex {
                    convex: pair.endpoints[side].convex,
                },
            )?);
        }
        let topologies = topologies.map(Option::unwrap);
        let placements = std::array::from_fn::<_, 2, _>(|side| FeaturePlacement {
            topology: topologies[side],
            position: poses[side].position,
            orientation: poses[side].orientation,
        });
        let selection = walk_compact_features(
            placements[0],
            placements[1],
            pair.seed,
            pair.maximum_feature_transitions,
            |_| {},
        )?;
        let order = selection.order();
        let bodies = order.map(|side| bodies[side]);
        let topologies = order.map(|side| topologies[side]);
        let endpoints = order.map(|side| pair.endpoints[side]);
        let materials = self.contact_materials(endpoints, selection.ordered_pair())?;
        let mut binding = ContactFeatureBinding::new(topologies, selection.ordered_pair(), true)?;
        let mut surface =
            binding.project(topologies, order.map(|side| poses[side]), self.tolerances)?;
        surface.clamp_penetration();
        let mut offsets = [[0.0; 3]; 2];
        let mut dynamic = [false; 2];
        for side in 0..2 {
            dynamic[side] = bodies[side].kind == BodyKind::Dynamic && bodies[side].motion_enabled;
            if bodies[side].kind == BodyKind::Dynamic {
                let basis = bodies[side]
                    .collision_orientation
                    .unwrap_or_else(|| bodies[side].orientation.matrix());
                offsets[side] = ImpactContactPoint::from_world(
                    bodies[side].core_position,
                    basis,
                    surface.point,
                )?;
            }
        }
        if !dynamic.iter().any(|value| *value) {
            return Err(EnvironmentError::DisabledMotion);
        }
        let body = usize::from(!dynamic[0]);
        let opposing = 1 - body;
        let request_speed = CollisionRequest {
            contact_distance: surface.distance,
            contact_threshold: self.tolerances.collision_distance,
            rotations: std::array::from_fn(|side| {
                dynamic[side].then_some(crate::CollisionRotation {
                    angular_velocity: bodies[side].velocity.angular,
                    contact_radius: bodies[side].physical.radius,
                })
            }),
            inverse_timestep: (1.0 / f64::from(self.config.timestep)) as f32,
        }
        .speed()?;
        let collision = BodyCollision {
            body: bodies[body].identity,
            opposing: bodies[opposing].identity,
            body_offset: offsets[body],
            opposing_offset: offsets[opposing],
            normal: if body == 0 {
                surface.normal.map(|value| -value)
            } else {
                surface.normal
            },
            request_speed: Some(request_speed),
        };
        Ok((
            collision,
            ContactGeometry {
                endpoints,
                shape_ids: bodies.map(|body| body.shape.identity),
                binding,
                surface,
                synchronized_offsets: offsets,
                materials,
            },
        ))
    }

    fn validate_collision(&self, input: BodyCollision) -> Result<(), EnvironmentError> {
        if input.body == input.opposing {
            return Err(EnvironmentError::IdenticalBodies);
        }
        if input
            .body_offset
            .iter()
            .chain(input.opposing_offset.iter())
            .chain(input.normal.iter())
            .any(|value| !value.is_finite())
            || input.request_speed.is_some_and(|value| !value.is_finite())
        {
            return Err(EnvironmentError::NonFinite);
        }
        if input.normal == [0.0; 3] {
            return Err(ResponseError::ZeroDirection.into());
        }
        let body = self.body(input.body).ok_or(EnvironmentError::MissingBody)?;
        let other = self
            .body(input.opposing)
            .ok_or(EnvironmentError::MissingBody)?;
        if !body.collisions_enabled || !other.collisions_enabled {
            return Err(EnvironmentError::DisabledCollision);
        }
        if body.kind == BodyKind::Static {
            return Err(EnvironmentError::StaticBody);
        }
        if !body.motion_enabled {
            return Err(EnvironmentError::DisabledMotion);
        }
        Ok(())
    }

    pub fn set_velocity(
        &mut self,
        identity: u64,
        linear: Option<[f32; 3]>,
        angular: Option<[f32; 3]>,
    ) -> Result<(), EnvironmentError> {
        self.write_velocity(identity, linear, angular, VelocityWrite::Queued)
    }

    pub fn add_velocity(
        &mut self,
        identity: u64,
        linear: Option<[f32; 3]>,
        angular: Option<[f32; 3]>,
    ) -> Result<(), EnvironmentError> {
        self.write_velocity(identity, linear, angular, VelocityWrite::Add)
    }

    pub fn set_velocity_instantaneous(
        &mut self,
        identity: u64,
        linear: Option<[f32; 3]>,
        angular: Option<[f32; 3]>,
    ) -> Result<(), EnvironmentError> {
        self.write_velocity(identity, linear, angular, VelocityWrite::Immediate)
    }

    fn write_velocity(
        &mut self,
        identity: u64,
        linear: Option<[f32; 3]>,
        angular: Option<[f32; 3]>,
        operation: VelocityWrite,
    ) -> Result<(), EnvironmentError> {
        if linear
            .into_iter()
            .chain(angular)
            .flatten()
            .any(|component| !component.is_finite())
        {
            return Err(EnvironmentError::NonFinite);
        }
        let limits = self.velocity_command_limits();
        let body = self.body_mut(identity)?;
        if body.kind == BodyKind::Static {
            return Err(EnvironmentError::StaticBody);
        }
        if !body.motion_enabled {
            return Err(EnvironmentError::DisabledMotion);
        }
        let mut active = body.velocity;
        let mut queued = body.queued_velocity;
        for (active, pending, input) in [
            (
                &mut active.linear,
                &mut queued.linear,
                linear.map(|value| internal_direction(value, METERS_PER_INCH)),
            ),
            (
                &mut active.angular,
                &mut queued.angular,
                angular.map(|value| internal_direction(value, RADIANS_PER_DEGREE)),
            ),
        ] {
            if let Some(value) = input {
                match operation {
                    VelocityWrite::Queued => {
                        *active = [0.0; 3];
                        *pending = value;
                    }
                    VelocityWrite::Immediate => {
                        *active = value;
                        *pending = [0.0; 3];
                    }
                    VelocityWrite::Add => {
                        for axis in 0..3 {
                            pending[axis] += value[axis];
                        }
                    }
                }
            }
        }
        limits.apply(&mut active, &mut queued)?;
        body.velocity = active;
        body.queued_velocity = queued;
        self.wake(identity)
    }

    fn velocity_command_limits(&self) -> VelocityCommandLimits {
        VelocityCommandLimits {
            linear_speed: self.config.performance.maximum_linear_speed * METERS_PER_INCH,
            angular_per_step: self.config.performance.maximum_angular_speed
                * RADIANS_PER_DEGREE
                * INITIAL_TIMESTEP,
            inverse_step: (1.0 / f64::from(self.config.timestep)) as f32,
        }
    }

    pub fn set_core_state(
        &mut self,
        identity: u64,
        state: BodyCoreState,
    ) -> Result<(), EnvironmentError> {
        if state
            .position
            .iter()
            .chain(state.orientation.quaternion.iter())
            .any(|component| !component.is_finite())
            || state
                .velocity
                .linear
                .iter()
                .chain(state.velocity.angular.iter())
                .any(|component| !component.is_finite())
            || state
                .collision_orientation
                .is_some_and(|basis| basis.iter().any(|component| !component.is_finite()))
        {
            return Err(EnvironmentError::NonFinite);
        }
        let body = self.body_mut(identity)?;
        if body.kind == BodyKind::Static {
            return Err(EnvironmentError::StaticBody);
        }
        if !body.motion_enabled {
            return Err(EnvironmentError::DisabledMotion);
        }
        self.transforms.invalidate(identity)?;
        let time = self.clock.current_time();
        let body = self.body_mut(identity)?;
        body.core_position = state.position;
        body.previous_core_position = state.position;
        body.orientation = state.orientation;
        body.collision_orientation = state.collision_orientation;
        body.previous_orientation = state.orientation;
        body.core_time = time;
        body.velocity = state.velocity;
        body.queued_velocity = QueuedVelocity::default();
        body.motion_phase = None;
        body.publication_phase = None;
        Ok(())
    }

    pub fn resolve_collision(
        &mut self,
        input: BodyCollision,
    ) -> Result<&EnvironmentCollision, EnvironmentError> {
        self.resolve_material_collision(input, None)
    }
    fn resolve_material_collision(
        &mut self,
        input: BodyCollision,
        materials: Option<[u32; 2]>,
    ) -> Result<&EnvironmentCollision, EnvironmentError> {
        if self.collisions.len() >= self.config.max_events {
            return Err(EnvironmentError::CollisionLimit);
        }
        let transition = self.apply_material_collision(input, materials, 0)?;
        self.collisions.push(transition);
        Ok(self.collisions.last().unwrap())
    }
    fn apply_material_collision(
        &mut self,
        input: BodyCollision,
        materials: Option<[u32; 2]>,
        repetitions: u16,
    ) -> Result<EnvironmentCollision, EnvironmentError> {
        if self
            .collision_observations
            .as_ref()
            .is_some_and(|values| values.len() == self.config.max_events)
        {
            return Err(EnvironmentError::ObservationLimit);
        }
        self.validate_collision(input)?;
        let first = self
            .bodies
            .iter()
            .position(|body| body.identity == input.body)
            .ok_or(EnvironmentError::MissingBody)?;
        let second = self
            .bodies
            .iter()
            .position(|body| body.identity == input.opposing)
            .ok_or(EnvironmentError::MissingBody)?;
        let body = &self.bodies[first];
        let opposing = &self.bodies[second];
        let pair = SurfacePair::from_registry(
            &self.surfaces,
            materials.unwrap_or([body.material, opposing.material]),
        )
        .map_err(|_| EnvironmentError::DependencyMismatch)?;
        let before = [body.velocity, opposing.velocity];
        let queued_before = [body.queued_velocity, opposing.queued_velocity];
        let endpoint = |value: &RigidBody, offset: [f32; 3]| CollisionBody {
            orientation: value
                .collision_orientation
                .unwrap_or_else(|| value.orientation.matrix()),
            local_offset: offset,
            inverse_mass: if value.motion_enabled {
                1.0 / value.physical.mass
            } else {
                0.0
            },
            inverse_inertia: value.physical.inertia.map(|component| {
                if value.motion_enabled {
                    1.0 / component
                } else {
                    0.0
                }
            }),
            linear_velocity: value.reported_velocity().linear,
            angular_velocity: value.reported_velocity().angular,
        };
        let dynamic_opposing = opposing.kind == BodyKind::Dynamic;
        let endpoints = [
            Some(endpoint(body, input.body_offset)),
            dynamic_opposing.then(|| endpoint(opposing, input.opposing_offset)),
        ];
        let solved = CollisionResponse {
            repetitions,
            body: endpoints[0].unwrap(),
            opposing: endpoints[1],
            normal: input.normal,
            friction: pair.friction,
            elasticity: pair.elasticity,
        }
        .solve()?;
        let (resolved, correction_impulse, required_speed) =
            if let Some(request_speed) = input.request_speed {
                let mut correction = CollisionCorrection::from_request(
                    [Some(solved.body), solved.opposing],
                    input.normal,
                    request_speed,
                )?;
                correction.velocity = solved.correction_velocity;
                let correction = correction.apply()?;
                (
                    correction.endpoints,
                    correction.impulse,
                    correction.required_speed,
                )
            } else {
                ([Some(solved.body), solved.opposing], 0.0, 0.0)
            };
        let limits = crate::CollisionVelocityLimits {
            maximum_linear: self.config.performance.maximum_linear_speed * METERS_PER_INCH,
            angular: AngularVelocityLimit {
                maximum_per_step: self.config.performance.maximum_angular_speed
                    * RADIANS_PER_DEGREE
                    * INITIAL_TIMESTEP,
                timestep: self.config.timestep,
            },
        };
        let mut resolved = resolved;
        for body in resolved.iter_mut().flatten() {
            let velocity = limits.apply(VelocityState {
                linear: body.linear_velocity,
                angular: body.angular_velocity,
            })?;
            body.linear_velocity = velocity.linear;
            body.angular_velocity = velocity.angular;
        }
        let mut committed = crate::CollisionMotionCommit {
            endpoints: resolved,
            current: before,
            effective_masses: solved.effective_masses,
            normal: input.normal.map(|value| -value),
            required_speed,
            allow_delay: input.request_speed.is_some()
                && !(repetitions > 10
                    && solved.correction_velocity == crate::CorrectionVelocity::ClosingOnly),
            collision_counts: [body.collision_count, opposing.collision_count],
        }
        .apply()?;
        let mut frozen = [body.temporarily_frozen, opposing.temporarily_frozen];
        for (side, index) in [first, second].into_iter().enumerate() {
            if committed.applied[side]
                && i32::from(committed.collision_counts[side] as i16)
                    > self.config.performance.max_collisions_per_body as i32
            {
                frozen[side] =
                    self.collision_solver.0.as_mut().is_none_or(|solver| {
                        solver.should_freeze_object(self.bodies[index].identity)
                    });
            }
        }
        if frozen.iter().any(|value| *value) {
            committed.applied = [true; 2];
            for (side, index) in [first, second].into_iter().enumerate() {
                if self.bodies[index].kind == BodyKind::Static {
                    continue;
                }
                frozen[side] = true;
                committed.queued[side] = QueuedVelocity {
                    linear: std::array::from_fn(|i| {
                        committed.active[side].linear[i] + committed.queued[side].linear[i]
                    }),
                    angular: std::array::from_fn(|i| {
                        committed.active[side].angular[i] + committed.queued[side].angular[i]
                    }),
                };
                committed.active[side] = VelocityState {
                    linear: [0.0; 3],
                    angular: [0.0; 3],
                };
            }
        }
        let transition = EnvironmentCollision {
            event_identity: None,
            event_time: self.time(),
            body: input.body,
            opposing: input.opposing,
            normal: input.normal,
            before,
            after: committed.active,
            queued_after: committed.queued,
            applied: committed.applied,
            surface_pair: pair,
            pushes: solved.pushes,
            correction_impulse,
            effective_masses: solved.effective_masses,
            request_speed: input.request_speed,
            geometry: None,
            dispatch: None,
        };
        for (side, index) in [first, second].into_iter().enumerate() {
            self.bodies[index].velocity = committed.active[side];
            self.bodies[index].queued_velocity = committed.queued[side];
            self.bodies[index].collision_count = committed.collision_counts[side];
            self.bodies[index].temporarily_frozen = frozen[side];
        }
        if let Some(observations) = &mut self.collision_observations {
            observations.push(CollisionObservation {
                repetitions,
                endpoints,
                queued_before,
                request_input: None,
                collision: transition.clone(),
            });
        }
        Ok(transition)
    }

    pub fn surface_pair(&self, first: u64, second: u64) -> Result<SurfacePair, EnvironmentError> {
        let first = self.body(first).ok_or(EnvironmentError::MissingBody)?;
        let second = self.body(second).ok_or(EnvironmentError::MissingBody)?;
        SurfacePair::from_registry(&self.surfaces, [first.material, second.material])
            .map_err(|_| EnvironmentError::DependencyMismatch)
    }

    pub fn simulate(&mut self, duration: f32) -> Result<(), EnvironmentError> {
        if self.paused {
            return Ok(());
        }
        let mut candidate = self.clone();
        candidate.cleanup_deleted_bodies()?;
        let terminal = candidate.clock.submission_terminal(duration)?;
        candidate.collisions.clear();
        candidate.callbacks.begin();
        if terminal.is_some() && candidate.event_reporting {
            candidate.emit_callback(PhysicsCallbackKind::PostSimulation, [None, None], None)?;
        }
        if terminal.is_some() {
            candidate.callbacks.expire_pair_clocks(candidate.time());
        }
        candidate.recovered_pairs.clear();
        candidate.statistics = PhysicsStatistics::default();
        candidate.shadows.begin();
        candidate.retired_impact_pairs.clear();
        if let Some(observations) = &mut candidate.normal_observations {
            observations.clear();
        }
        candidate.clear_convex_observations();
        if let Some(observations) = &mut candidate.collision_observations {
            observations.clear();
        }
        if let Some(observations) = &mut candidate.tangent_observations {
            observations.clear();
        }
        candidate.friction_events.clear();
        let sleeping = candidate
            .active_objects
            .iter()
            .filter(|core| candidate.islands.movement(**core) == Some(crate::CoreMovement::Dormant))
            .copied()
            .collect::<Vec<_>>();
        for core in sleeping.into_iter().rev() {
            let index = candidate
                .active_objects
                .iter()
                .position(|value| *value == core)
                .ok_or(EnvironmentError::SnapshotMismatch)?;
            candidate.active_objects.swap_remove(index);
        }
        if let Some(terminal) = terminal {
            candidate.advance_events_inner(Some(terminal))?;
            for body in &mut candidate.bodies {
                body.publish_phase(terminal)?;
            }
        }
        candidate.report_friction()?;
        candidate.callbacks.finish();
        candidate.ticks = self
            .ticks
            .checked_add(1)
            .ok_or(EnvironmentError::ClockOverflow)?;
        *self = candidate;
        Ok(())
    }

    pub fn snapshot(&self) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            shadows: self.shadows.clone(),
            fluids: self.fluids.clone(),
            next_controller: self.next_controller,
            core_storage: self.core_storage.clone(),
            retired_impact_pairs: self.retired_impact_pairs.clone(),
            callbacks: self.callbacks.clone(),
            normal_observations: self.normal_observations.clone(),
            collision_observations: self.collision_observations.clone(),
            recovered_pairs: self.recovered_pairs.clone(),
            world_materials: self.world_materials,
            tangent_observations: self.tangent_observations.clone(),
            statistics: self.statistics,
            event_reporting: self.event_reporting,
            active_objects: self.active_objects.clone(),
            last_friction_time: self.last_friction_time,
            friction_events: self.friction_events.clone(),
            sleep_scheduler: self.sleep_scheduler,
            contacts: self.contacts.clone(),
            collision_solver: self.collision_solver.clone(),
            pairs: self.pairs.clone(),
            islands: self.islands.clone(),
            next_core: self.next_core,
            pending_wake: self.pending_wake.clone(),
            delete_queue_enabled: self.delete_queue_enabled,
            pending_deletes: self.pending_deletes.clone(),
            transforms: self.transforms.clone(),
            config: self.config,
            surface_identity: self.surfaces.identity,
            bodies: self.bodies.clone(),
            queue: self.queue.clone(),
            collisions: self.collisions.clone(),
            queued_collisions: self.queued_collisions.clone(),
            clock: self.clock,
            ticks: self.ticks,
            paused: self.paused,
        }
    }

    pub fn restore(&mut self, snapshot: EnvironmentSnapshot) -> Result<(), EnvironmentError> {
        if !snapshot.shadows.valid(&snapshot) {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if !snapshot.fluids.validate(&snapshot) {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot.next_controller < 3 {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if !snapshot
            .core_storage
            .valid(&snapshot.bodies, snapshot.config.max_bodies)
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot.retired_impact_pairs.len() > snapshot.config.max_events
            || snapshot.retired_impact_pairs.len()
                != snapshot.statistics.retired_impact_pairs as usize
            || snapshot
                .retired_impact_pairs
                .iter()
                .enumerate()
                .any(|(index, pair)| {
                    pair.identity == 0
                        || pair.bodies[0] == pair.bodies[1]
                        || snapshot.retired_impact_pairs[..index]
                            .iter()
                            .any(|old| old.identity == pair.identity)
                })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot.pending_deletes.len() > snapshot.bodies.len()
            || snapshot
                .pending_deletes
                .iter()
                .enumerate()
                .any(|(index, id)| {
                    snapshot.pending_deletes[..index].contains(id)
                        || !snapshot.bodies
                            [snapshot.bodies.len() - snapshot.pending_deletes.len()..]
                            .iter()
                            .any(|body| body.identity == *id && body.callback_flags & 0x0400 != 0)
                })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot.surface_identity != self.surfaces.identity {
            return Err(EnvironmentError::DependencyMismatch);
        }
        snapshot.config.validate()?;
        if !snapshot
            .callbacks
            .validate(snapshot.config.max_events, snapshot.core_storage.next)
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot.normal_observations.as_ref().is_some_and(|values| {
            values.len() > snapshot.config.max_events
                || values
                    .iter()
                    .any(|v| !v.time.is_finite() || v.time > snapshot.clock.current_time())
        }) {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot
            .collision_observations
            .as_ref()
            .is_some_and(|values| values.len() > snapshot.config.max_events)
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if snapshot
            .tangent_observations
            .as_ref()
            .is_some_and(|observations| {
                observations.len() > snapshot.config.max_events
                    || observations.iter().any(|o| {
                        !o.response_coefficient.is_finite()
                            || o.response_coefficient < 0.0
                            || !o.time.is_finite()
                            || o.time > snapshot.clock.current_time()
                    })
            })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        snapshot.islands.validate()?;
        snapshot.pairs.validate(&snapshot)?;
        snapshot.contacts.validate(&snapshot)?;
        if snapshot.recovered_pairs.len() > snapshot.config.max_events
            || snapshot
                .recovered_pairs
                .iter()
                .enumerate()
                .any(|(i, pair)| {
                    pair[0] >= pair[1]
                        || snapshot.recovered_pairs[..i].contains(pair)
                        || pair.iter().any(|id| *id == 0 || *id >= snapshot.next_core)
                })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if !snapshot.last_friction_time.is_finite()
            || snapshot.friction_events.len() > snapshot.config.max_events
            || snapshot.active_objects.len() > snapshot.bodies.len()
            || snapshot.active_objects.iter().enumerate().any(|(i, core)| {
                snapshot.active_objects[..i].contains(core)
                    || !snapshot
                        .bodies
                        .iter()
                        .any(|body| body.core_identity == *core && body.kind == BodyKind::Dynamic)
            })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        let ids = snapshot.islands.core_ids().collect::<Vec<_>>();
        if ids.len() != snapshot.bodies.len() + 1
            || !ids.contains(&0)
            || snapshot.next_core == 0
            || snapshot.bodies.iter().any(|body| {
                body.core_identity == 0
                    || body.core_identity >= snapshot.next_core
                    || !ids.contains(&body.core_identity)
                    || snapshot.islands.is_immovable(body.core_identity)
                        != Some(body.kind == BodyKind::Static)
            })
            || snapshot.pending_wake.iter().enumerate().any(|(i, core)| {
                snapshot.pending_wake[..i].contains(core)
                    || !snapshot.bodies.iter().any(|body| {
                        body.core_identity == *core
                            && body.kind == BodyKind::Dynamic
                            && !body.asleep
                    })
            })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        let tolerances = ContactTolerances::from_gravity(snapshot.config.gravity)?;
        let search_ranges = crate::CollisionSearchRanges::new(
            snapshot.config.timestep,
            snapshot.config.performance.lookahead_world,
            snapshot.config.performance.lookahead_bodies,
        )?;
        if snapshot
            .transforms
            .owners()
            .any(|identity| !snapshot.bodies.iter().any(|body| body.identity == identity))
            || !snapshot.clock.current_time().is_finite()
            || snapshot.clock.timestep().to_bits() != snapshot.config.timestep.to_bits()
            || snapshot.queue.base().to_bits() != snapshot.clock.last_boundary().to_bits()
            || snapshot.bodies.len() > snapshot.config.max_bodies
            || snapshot.queue.len() > snapshot.config.max_events.saturating_add(1)
            || snapshot.collisions.len() > snapshot.config.max_events
            || snapshot
                .queued_collisions
                .len()
                .checked_add(snapshot.pairs.event_count())
                .and_then(|v| v.checked_add(1))
                != Some(snapshot.queue.len())
            || !snapshot.queue.entries().iter().any(|event| {
                event.identity == EnvironmentEvent::Boundary
                    && event.time.absolute == snapshot.clock.next_boundary()
            })
        {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        for (index, body) in snapshot.bodies.iter().enumerate() {
            if !body.core_time.is_finite()
                || self.surfaces.records.get(body.material as usize).is_none()
                || !body.core_inverse_step.is_finite()
                || !body.volume.is_finite()
                || !body.buoyancy_ratio.is_finite()
                || body.core_inverse_step <= 0.0
                || snapshot.bodies[..index].iter().any(|prior| {
                    prior.identity == body.identity || prior.core_identity == body.core_identity
                })
                || !body
                    .physical
                    .validate(&body.shape, body.kind == BodyKind::Static)?
                || ObjectFrame::from_center(body.physical.center)? != body.frame
                || body
                    .core_position
                    .iter()
                    .chain(body.previous_core_position.iter())
                    .chain(body.orientation.quaternion.iter())
                    .chain(body.previous_orientation.quaternion.iter())
                    .any(|value| !value.is_finite())
                || body
                    .velocity
                    .linear
                    .iter()
                    .chain(body.velocity.angular.iter())
                    .chain(body.queued_velocity.linear.iter())
                    .chain(body.queued_velocity.angular.iter())
                    .any(|value| !value.is_finite())
                || body
                    .collision_orientation
                    .is_some_and(|basis| basis.iter().any(|value| !value.is_finite()))
                || body.motion_phase.is_some_and(|retained| {
                    let phase = retained.phase;
                    phase.sample(phase.start).is_err()
                        || phase.sample(phase.end).is_err()
                        || retained.motion.validate().is_err()
                })
                || body.publication_phase.is_some_and(|(phase, terminal)| {
                    phase.sample(phase.start).is_err()
                        || phase.publication_sample(terminal).is_err()
                })
            {
                return Err(EnvironmentError::SnapshotMismatch);
            }
        }
        for collision in &snapshot.collisions {
            if collision.body == collision.opposing
                || collision.pushes.len() > 102
                || !collision.correction_impulse.is_finite()
                || collision.correction_impulse < 0.0
                || !collision.event_time.is_finite()
                || collision
                    .normal
                    .iter()
                    .any(|component| !component.is_finite())
                || collision.queued_after.iter().any(|value| {
                    value
                        .linear
                        .iter()
                        .chain(value.angular.iter())
                        .any(|value| !value.is_finite())
                })
                || collision.geometry.is_some_and(|geometry| {
                    geometry
                        .surface
                        .point
                        .iter()
                        .any(|value| !value.is_finite())
                        || !geometry.surface.distance.is_finite()
                        || geometry.surface.frame().is_err()
                        || geometry
                            .synchronized_offsets
                            .iter()
                            .flatten()
                            .any(|value| !value.is_finite())
                })
            {
                return Err(EnvironmentError::SnapshotMismatch);
            }
        }
        for (index, event) in snapshot.queued_collisions.iter().enumerate() {
            let ids = event.input.bodies();
            if snapshot.queued_collisions[..index]
                .iter()
                .any(|prior| prior.identity == event.identity)
                || !snapshot
                    .queue
                    .entries()
                    .iter()
                    .any(|entry| entry.identity == EnvironmentEvent::Collision(event.identity))
                || ids
                    .iter()
                    .any(|id| !snapshot.bodies.iter().any(|body| body.identity == *id))
                || ids[0] == ids[1]
            {
                return Err(EnvironmentError::SnapshotMismatch);
            }
            match event.input {
                QueuedCollisionInput::Direct(collision) => {
                    if collision
                        .body_offset
                        .iter()
                        .chain(collision.opposing_offset.iter())
                        .chain(collision.normal.iter())
                        .any(|value| !value.is_finite())
                        || collision
                            .request_speed
                            .is_some_and(|value| !value.is_finite() || value < 0.0)
                    {
                        return Err(EnvironmentError::SnapshotMismatch);
                    }
                }
                QueuedCollisionInput::Pair {
                    pair: input,
                    predicted,
                } => {
                    if [
                        input.start,
                        input.end,
                        predicted.time,
                        predicted.root.time,
                        predicted.root.value,
                    ]
                    .iter()
                    .any(|value| !value.is_finite())
                        || input.start >= input.end
                        || input.maximum_feature_transitions == 0
                        || predicted.time < input.start
                        || predicted.time > input.end
                    {
                        return Err(EnvironmentError::SnapshotMismatch);
                    }
                    for (endpoint, feature) in input
                        .endpoints
                        .into_iter()
                        .zip([input.seed.first, input.seed.second])
                    {
                        let topology = snapshot
                            .bodies
                            .iter()
                            .find(|body| body.identity == endpoint.body)
                            .and_then(|body| body.topology(endpoint.convex))
                            .ok_or(EnvironmentError::SnapshotMismatch)?;
                        if topology.edge(feature.edge).is_err() {
                            return Err(EnvironmentError::SnapshotMismatch);
                        }
                    }
                }
            }
        }
        self.config = snapshot.config;
        self.callbacks = snapshot.callbacks;
        self.normal_observations = snapshot.normal_observations;
        self.collision_observations = snapshot.collision_observations;
        self.recovered_pairs = snapshot.recovered_pairs;
        self.world_materials = snapshot.world_materials;
        self.statistics = snapshot.statistics;
        self.tangent_observations = snapshot.tangent_observations;
        self.event_reporting = snapshot.event_reporting;
        self.active_objects = snapshot.active_objects;
        self.last_friction_time = snapshot.last_friction_time;
        self.friction_events = snapshot.friction_events;
        self.sleep_scheduler = snapshot.sleep_scheduler;
        self.contacts = snapshot.contacts;
        self.collision_solver = snapshot.collision_solver;
        self.pairs = snapshot.pairs;
        self.islands = snapshot.islands;
        self.next_core = snapshot.next_core;
        self.pending_wake = snapshot.pending_wake;
        self.delete_queue_enabled = snapshot.delete_queue_enabled;
        self.pending_deletes = snapshot.pending_deletes;
        self.shadows = snapshot.shadows;
        self.fluids = snapshot.fluids;
        self.next_controller = snapshot.next_controller;
        self.core_storage = snapshot.core_storage;
        self.retired_impact_pairs = snapshot.retired_impact_pairs;
        self.tolerances = tolerances;
        self.search_ranges = search_ranges;
        self.bodies = snapshot.bodies;
        self.transforms = snapshot.transforms;
        self.queue = snapshot.queue;
        self.collisions = snapshot.collisions;
        self.queued_collisions = snapshot.queued_collisions;
        self.clock = snapshot.clock;
        self.ticks = snapshot.ticks;
        self.paused = snapshot.paused;
        Ok(())
    }

    fn body_mut(&mut self, identity: u64) -> Result<&mut RigidBody, EnvironmentError> {
        self.bodies
            .iter_mut()
            .find(|body| body.identity == identity)
            .ok_or(EnvironmentError::MissingBody)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        BodyCollision, BodyConvex, BodyCoreState, BodyInput, BodyKind, BodyMotionPhase,
        ConvexContactPair, EnvironmentConfig, EnvironmentError, PerformanceSettings,
        PhysicsEnvironment,
    };
    use crate::{
        CoreOrientation, EventTimingKind, SurfaceFeature, SurfaceFeatureKind, SurfaceFeaturePair,
        VelocityState,
    };
    use playsrc_collision::{
        AuthoredConvex, AuthoredShapeProperties, AuthoredTriangle, ConvexInput, PhysicsShape,
        SnapshotLimits,
    };
    use playsrc_material::{SurfacePropertyFile, SurfacePropertyRegistry};
    use std::sync::Arc;

    #[test]
    fn world_material_tables_preserve_partial_updates_unsigned_storage_and_snapshots() {
        let mut world = PhysicsEnvironment::new(config(), surfaces(b".8")).unwrap();
        assert_eq!(
            *world.world_material_index_table(),
            std::array::from_fn(|i| i as u16)
        );
        world.set_world_material_index_table(&[7, -1, 65536, 65537]);
        assert_eq!(
            &world.world_material_index_table()[..6],
            &[7, 65535, 0, 1, 4, 5]
        );
        let saved = world.snapshot();
        world.set_world_material_index_table(&[19; 130]);
        assert_eq!(*world.world_material_index_table(), [19; 128]);
        world.restore(saved.clone()).unwrap();
        assert_eq!(world.snapshot(), saved);
        world.set_world_material_index_table(&[]);
        assert_eq!(world.snapshot(), saved);
    }

    fn config() -> EnvironmentConfig {
        EnvironmentConfig {
            random_seed: 1,
            gravity: [0.0, 0.0, -800.0],
            air_density: 2.0,
            timestep: 0.015,
            max_bodies: 4,
            max_events: 8,
            performance: PerformanceSettings::default(),
        }
    }
    fn surfaces(friction: &[u8]) -> Arc<SurfacePropertyRegistry> {
        let mut bytes = b"default { friction ".to_vec();
        bytes.extend_from_slice(friction);
        bytes.extend_from_slice(b" elasticity .25 density 2000 }");
        Arc::new(
            SurfacePropertyRegistry::compile(&[SurfacePropertyFile {
                logical_path: "scripts/surfaceproperties.txt",
                bytes: &bytes,
            }])
            .unwrap(),
        )
    }

    pub(crate) fn tetrahedron(scale: f32) -> Arc<PhysicsShape> {
        let points = vec![
            [0.0_f32, 0.0, 0.0],
            [0.1 * scale, 0.0, 0.0],
            [0.0, 0.1 * scale, 0.0],
            [0.0, 0.0, 0.1 * scale],
        ];
        let triangles = vec![[1_u32, 2, 0], [3, 1, 0], [2, 3, 0], [3, 2, 1]];
        let directed = triangles
            .iter()
            .enumerate()
            .flat_map(|(face, vertices)| {
                (0..3).map(move |edge| {
                    (
                        face * 4 + edge + 1,
                        vertices[2 - edge],
                        vertices[2 - (edge + 1) % 3],
                    )
                })
            })
            .collect::<Vec<_>>();
        let authored = triangles
            .iter()
            .enumerate()
            .map(|(face, vertices)| {
                let mut raw = [0_u8; 16];
                for edge in 0..3 {
                    let (slot, start, end) = directed[face * 3 + edge];
                    let opposite = directed
                        .iter()
                        .find(|(_, a, b)| *a == end && *b == start)
                        .unwrap()
                        .0;
                    let offset = opposite as i32 - slot as i32;
                    let word = start | (((offset as u32) & 0x7fff) << 16);
                    raw[4 + edge * 4..8 + edge * 4].copy_from_slice(&word.to_le_bytes());
                }
                AuthoredTriangle {
                    vertices: *vertices,
                    raw,
                }
            })
            .collect();
        let mut node = [0_u8; 28];
        node[4..8].copy_from_slice(&32_i32.to_le_bytes());
        node[20..24].copy_from_slice(&(0.1_f32 * scale).to_le_bytes());
        node[24..27].copy_from_slice(&[250; 3]);
        Arc::new(
            PhysicsShape::compile_authored(
                (u64::from(scale.to_bits()) << 32) | 7,
                vec![ConvexInput {
                    solid: 0,
                    convex: 0,
                    contents: 1,
                    vertices: points
                        .iter()
                        .map(|point| {
                            [
                                point[0] * 39.37008,
                                point[2] * 39.37008,
                                -point[1] * 39.37008,
                            ]
                        })
                        .collect(),
                    triangles,
                    authored: Some(AuthoredConvex {
                        raw_header: [80, 0, 0, 0, 0, 0, 0, 0, 4, 9, 0, 0, 4, 0, 0, 0],
                        points,
                        triangles: authored,
                    }),
                }],
                AuthoredShapeProperties {
                    center: [0.0; 3],
                    inertia: [0.01 * scale * scale; 3],
                    radius: 0.1 * scale,
                    max_surface_deviation: 250,
                    drag_axes: Some([1.0; 3]),
                },
                playsrc_collision::AuthoredHierarchy {
                    nodes: vec![playsrc_collision::AuthoredHierarchyNode {
                        raw: node,
                        children: None,
                        hull: Some(playsrc_collision::AuthoredHullRef::Piece(0)),
                    }],
                    enclosures: Vec::new(),
                },
                SnapshotLimits::default(),
            )
            .unwrap(),
        )
    }

    fn collision_world() -> PhysicsEnvironment {
        collision_world_scale(1.0)
    }

    #[test]
    fn shared_cache_snapshot_owns_slots_and_closes_destroyed_body_links() {
        let mut world = collision_world();
        let moving = world.cached_transform(1).unwrap();
        let fixed = world.cached_transform(2).unwrap();
        assert_eq!(
            (world.transforms.slot(1), world.transforms.slot(2)),
            (Some(0), Some(1))
        );
        let before = world.snapshot();
        assert_eq!(
            world.cached_transform(99),
            Err(EnvironmentError::MissingBody)
        );
        assert_eq!(world.snapshot(), before);
        world.destroy_body(1).unwrap();
        assert_eq!(world.transforms.slot(1), None);
        assert_eq!(world.transforms.cursor(), 2);
        assert_eq!(world.cached_transform(2).unwrap(), fixed);
        world.restore(before.clone()).unwrap();
        assert_eq!(world.cached_transform(1).unwrap(), moving);
        assert_eq!(world.snapshot(), before);
        let mut invalid = before;
        invalid.bodies.remove(0);
        let unchanged = world.snapshot();
        assert_eq!(
            world.restore(invalid),
            Err(EnvironmentError::SnapshotMismatch)
        );
        assert_eq!(world.snapshot(), unchanged);
    }

    fn collision_world_scale(fixed_scale: f32) -> PhysicsEnvironment {
        let mut world = PhysicsEnvironment::new(config(), surfaces(b"0")).unwrap();
        for (identity, kind) in [(1, BodyKind::Dynamic), (2, BodyKind::Static)] {
            world
                .create_body(BodyInput {
                    volume: 0.0,
                    inertia_factor: 1.0,
                    rotational_inertia_limit: 0.05,
                    identity,
                    shape: tetrahedron(if kind == BodyKind::Static {
                        fixed_scale
                    } else {
                        1.0
                    }),
                    material: 0,
                    kind,
                    mass: 5.0,
                    position: [0.0; 3],
                    angles: [0.0; 3],
                    linear_velocity: [0.0; 3],
                    angular_velocity: [0.0; 3],
                    linear_damping: 0.0,
                    angular_damping: 0.0,
                    drag: 0.0,
                    collisions_enabled: true,
                    gravity_enabled: false,
                    drag_enabled: false,
                })
                .unwrap();
        }
        world
            .set_core_state(
                1,
                BodyCoreState {
                    position: [0.0, -0.02, 0.0],
                    orientation: CoreOrientation {
                        quaternion: [0.0, 0.0, 0.0, 1.0],
                    },
                    collision_orientation: None,
                    velocity: VelocityState {
                        linear: [0.0, 1.0, 0.0],
                        angular: [0.0; 3],
                    },
                },
            )
            .unwrap();
        world.wake(1).unwrap();
        world
            .set_motion_phase(
                1,
                BodyMotionPhase {
                    position: [0.0, -0.02, 0.0],
                    prior_orientation: CoreOrientation {
                        quaternion: [0.0, 0.0, 0.0, 1.0],
                    },
                    next_orientation: CoreOrientation {
                        quaternion: [0.0, 0.0, 0.0, 1.0],
                    },
                    projection_velocity: [0.0, 1.0, 0.0],
                    start: 0.0,
                    end: f64::from(config().timestep),
                    inverse_step: (1.0 / f64::from(config().timestep)) as f32,
                },
            )
            .unwrap();
        world
    }
    #[test]
    fn creation_mass_and_inertia_parameters_are_normalized_before_shape_derivation() {
        use crate::PhysicalShape;
        let shape = tetrahedron(1.0);
        let minimum = PhysicalShape::from_collision(&shape, -5.0, 0.0, 0.05).unwrap();
        let explicit = PhysicalShape::from_collision(&shape, 0.1, 1.0, 0.05).unwrap();
        assert_eq!(minimum, explicit);
        let maximum = PhysicalShape::from_collision(&shape, 100_000.0, 2.0e18, 0.0).unwrap();
        assert_eq!(maximum.mass, 50_000.0);
        assert_eq!(maximum.inertia_factor, 1.0e18_f32);
        let scaled = PhysicalShape::from_collision(&shape, 5.0, 0.25, 0.0).unwrap();
        let source = shape.authored_properties().unwrap();
        assert_eq!(
            scaled.inertia,
            source.inertia.map(|v| (f64::from(v * 0.25) * 5.0) as f32)
        );
        let limited = PhysicalShape::from_collision(&shape, 5.0, 0.25, 1.0).unwrap();
        let squared = scaled.inertia.map(|v| f64::from(v) * f64::from(v));
        let length = ((squared[0] + squared[1]) + squared[2]).sqrt() as f32;
        assert_eq!(limited.inertia, [length; 3]);
        assert_eq!(
            PhysicalShape::from_collision(&shape, 5.0, f32::NAN, 0.05),
            Err(crate::ShapeError::NonFinite)
        );
    }

    fn contact() -> BodyCollision {
        BodyCollision {
            body: 1,
            opposing: 2,
            body_offset: [0.0; 3],
            opposing_offset: [0.0; 3],
            normal: [0.0, -1.0, 0.0],
            request_speed: None,
        }
    }

    #[test]
    fn independent_environments_warm_pause_restore_and_reject_dependency_changes() {
        let mut first = PhysicsEnvironment::new(config(), surfaces(b".8")).unwrap();
        let second = PhysicsEnvironment::new(config(), surfaces(b".8")).unwrap();
        first.simulate(first.clock().timestep()).unwrap();
        assert_eq!(first.ticks(), 1);
        assert_eq!(second.ticks(), 0);
        let snapshot = first.snapshot();
        first.set_paused(true);
        first.simulate(first.clock().timestep()).unwrap();
        assert_eq!(first.ticks(), 1);
        first.restore(snapshot.clone()).unwrap();
        assert!(!first.is_paused());
        first.simulate(first.clock().timestep()).unwrap();
        assert_eq!(first.ticks(), 2);
        let mut wrong = PhysicsEnvironment::new(config(), surfaces(b".5")).unwrap();
        let before = wrong.snapshot();
        assert_eq!(
            wrong.restore(snapshot),
            Err(EnvironmentError::DependencyMismatch)
        );
        assert_eq!(wrong.snapshot(), before);
    }

    #[test]
    fn environment_rejects_invalid_settings_and_unbound_collisions_atomically() {
        assert!(matches!(
            PhysicsEnvironment::new(
                EnvironmentConfig {
                    timestep: 0.0,
                    ..config()
                },
                surfaces(b".8")
            ),
            Err(EnvironmentError::InvalidTimestep)
        ));
        let mut world = PhysicsEnvironment::new(config(), surfaces(b".8")).unwrap();
        world
            .queue
            .insert(super::EnvironmentEvent::Collision(1), 0.001)
            .unwrap();
        let before = world.snapshot();
        assert_eq!(
            world.simulate(world.clock().timestep()),
            Err(EnvironmentError::Event(
                crate::ContinuousError::MissingEvent
            ))
        );
        assert_eq!(world.snapshot(), before);
    }

    #[test]
    fn collision_endpoint_validation_preserves_the_entire_prior_environment() {
        let mut environment = PhysicsEnvironment::new(config(), surfaces(b".8")).unwrap();
        let before = environment.snapshot();
        let collision = BodyCollision {
            body: 1,
            opposing: 1,
            body_offset: [0.0; 3],
            opposing_offset: [0.0; 3],
            normal: [0.0, -1.0, 0.0],
            request_speed: None,
        };
        assert_eq!(
            environment.resolve_collision(collision),
            Err(EnvironmentError::IdenticalBodies)
        );
        assert_eq!(environment.snapshot(), before);
        assert_eq!(
            environment.resolve_collision(BodyCollision {
                opposing: 2,
                ..collision
            }),
            Err(EnvironmentError::MissingBody)
        );
        assert_eq!(environment.snapshot(), before);
        assert_eq!(
            environment.set_core_state(
                1,
                BodyCoreState {
                    position: [f64::NAN, 0.0, 0.0],
                    orientation: CoreOrientation {
                        quaternion: [0.0, 0.0, 0.0, 1.0],
                    },
                    collision_orientation: None,
                    velocity: VelocityState {
                        linear: [0.0; 3],
                        angular: [0.0; 3],
                    },
                },
            ),
            Err(EnvironmentError::NonFinite)
        );
        assert_eq!(environment.snapshot(), before);
    }

    #[test]
    fn simulation_consumes_nonzero_timed_contact_and_preserves_remaining_motion() {
        let mut world = collision_world();
        let scheduled = world.schedule_collision(17, 0.005, contact()).unwrap();
        let queued = world.snapshot();
        let mut replay = PhysicsEnvironment::new(config(), surfaces(b"0")).unwrap();
        replay.restore(queued).unwrap();
        world.simulate(world.clock().timestep()).unwrap();
        replay.simulate(replay.clock().timestep()).unwrap();
        assert_eq!(world.snapshot(), replay.snapshot());
        assert_eq!(world.collisions().len(), 1);
        let collision = &world.collisions()[0];
        assert_eq!(collision.event_identity, Some(17));
        assert_eq!(collision.event_time.to_bits(), scheduled.absolute.to_bits());
        assert_eq!(collision.before[0].linear, [0.0, 1.0, 0.0]);
        assert!((collision.after[0].linear[1] + 0.25).abs() < 1.0e-6);
        let boundary = f64::from(config().timestep);
        let remaining = boundary - scheduled.absolute;
        let at_boundary =
            -0.02 + scheduled.absolute + f64::from(collision.after[0].linear[1]) * remaining;
        let expected = at_boundary + f64::from(collision.after[0].linear[1]) * boundary;
        assert_eq!(
            world.body(1).unwrap().core_position()[1].to_bits(),
            expected.to_bits()
        );
        let publication_time = world.time();
        let published_elapsed = (publication_time - boundary) as f32;
        let published_internal =
            at_boundary + f64::from(collision.after[0].linear[1]) * f64::from(published_elapsed);
        let published = world.body(1).unwrap().published().unwrap();
        assert_eq!(
            published.position[2].to_bits(),
            ((-published_internal * f64::from(super::INCHES_PER_METER)) as f32).to_bits()
        );
        assert_eq!(world.pending_collisions().count(), 0);
    }

    #[test]
    fn typed_queue_rekeys_cancels_closes_destroyed_endpoints_and_rolls_back_failure() {
        let mut world = collision_world();
        world.schedule_collision(1, 0.005, contact()).unwrap();
        world.schedule_collision(2, 0.005, contact()).unwrap();
        assert_eq!(world.pending_collisions().next().unwrap().identity, 2);
        world.rekey_collision(1, 0.005).unwrap();
        assert_eq!(world.pending_collisions().next().unwrap().identity, 1);
        world.cancel_collision(2).unwrap();
        let mut failed = world.clone();
        failed.set_motion_enabled(1, false).unwrap();
        let before = failed.snapshot();
        assert_eq!(
            failed.simulate(failed.clock().timestep()),
            Err(EnvironmentError::DisabledMotion)
        );
        assert_eq!(failed.snapshot(), before);
        world.destroy_body(2).unwrap();
        assert_eq!(world.pending_collisions().count(), 0);
        let snapshot = world.snapshot();
        world.restore(snapshot.clone()).unwrap();
        assert_eq!(world.snapshot(), snapshot);
    }

    #[test]
    fn simultaneous_contacts_dispatch_newest_first_and_boundary_contacts_wait_for_next_phase() {
        let mut simultaneous = collision_world();
        simultaneous
            .schedule_collision(1, 0.005, contact())
            .unwrap();
        simultaneous
            .schedule_collision(2, 0.005, contact())
            .unwrap();
        simultaneous
            .simulate(simultaneous.clock().timestep())
            .unwrap();
        assert_eq!(
            simultaneous
                .collisions()
                .iter()
                .map(|event| event.event_identity)
                .collect::<Vec<_>>(),
            [Some(2), Some(1)]
        );
        assert!(!simultaneous.collisions()[0].pushes.is_empty());
        assert!(simultaneous.collisions()[1].pushes.is_empty());

        let mut boundary = collision_world();
        let terminal = f64::from(config().timestep);
        boundary.schedule_collision(9, terminal, contact()).unwrap();
        boundary.advance_events_before(terminal).unwrap();
        assert!(boundary.collisions().is_empty());
        assert_eq!(boundary.pending_collisions().count(), 1);
        boundary.simulate(boundary.clock().timestep()).unwrap();
        assert_eq!(boundary.collisions().len(), 1);
        assert_eq!(boundary.collisions()[0].event_identity, Some(9));
        assert_eq!(
            boundary.collisions()[0].event_time.to_bits(),
            terminal.to_bits()
        );
        assert_eq!(boundary.pending_collisions().count(), 0);
    }

    #[test]
    fn boundary_and_collision_equal_keys_preserve_insertion_order_and_controller_phase() {
        let terminal = f64::from(config().timestep);
        let mut before = collision_world();
        before.set_gravity_enabled(1, true).unwrap();
        before.schedule_collision(1, terminal, contact()).unwrap();
        before.simulate(before.clock().timestep()).unwrap();
        let mut after = collision_world();
        after.set_gravity_enabled(1, true).unwrap();
        after.advance_boundary().unwrap();
        after.schedule_collision(1, terminal, contact()).unwrap();
        after.simulate(after.clock().timestep()).unwrap();
        let accelerated = (1.0
            + f64::from(800.0_f32 * super::METERS_PER_INCH) * f64::from(config().timestep))
            as f32;
        assert_eq!(
            after.collisions()[0].before[0].linear[1].to_bits(),
            accelerated.to_bits()
        );
        let twice = (f64::from(accelerated)
            + f64::from(800.0_f32 * super::METERS_PER_INCH) * f64::from(config().timestep))
            as f32;
        assert_eq!(
            before.collisions()[0].before[0].linear[1].to_bits(),
            twice.to_bits()
        );
        assert_eq!(before.clock(), after.clock());
        assert_eq!(before.clock().last_boundary(), terminal);
        assert_eq!(before.queue.base(), terminal);
    }

    #[test]
    fn boundary_limit_pause_and_time_reversal_preserve_complete_phase_state() {
        let mut world = collision_world();
        let original = world.snapshot();
        assert_eq!(
            world.advance_events_before(2000.0),
            Err(EnvironmentError::StepLimit)
        );
        assert_eq!(world.snapshot(), original);
        world.simulate(world.clock().timestep()).unwrap();
        let advanced = world.snapshot();
        assert_eq!(
            world.advance_events_before(0.0),
            Err(EnvironmentError::Event(
                crate::ContinuousError::EventBeforeBase
            ))
        );
        assert_eq!(world.snapshot(), advanced);
        world.set_paused(true);
        let paused = world.snapshot();
        world.advance_boundary().unwrap();
        world.simulate(world.clock().timestep()).unwrap();
        assert_eq!(world.snapshot(), paused);
    }

    #[test]
    fn explicit_sleep_freezes_active_phase_and_preserves_pending_activation_velocity() {
        let mut active = collision_world();
        active.simulate(active.clock().timestep()).unwrap();
        let phase = active.body(1).unwrap().motion_phase().unwrap();
        active.sleep(1).unwrap();
        let body = active.body(1).unwrap();
        assert_eq!(body.core_position(), phase.position);
        assert_eq!(body.core_orientation(), phase.prior_orientation);
        assert_eq!(
            body.internal_velocity(),
            VelocityState {
                linear: [0.0; 3],
                angular: [0.0; 3]
            }
        );
        assert!(body.motion_phase().is_none());
        let frozen = body.published().unwrap();
        active.simulate(active.clock().timestep()).unwrap();
        assert_eq!(active.body(1).unwrap().published().unwrap(), frozen);
        active.wake(1).unwrap();
        active.simulate(active.clock().timestep()).unwrap();
        assert_eq!(active.body(1).unwrap().core_position(), phase.position);

        let mut pending = PhysicsEnvironment::new(config(), surfaces(b"0")).unwrap();
        pending
            .create_body(BodyInput {
                volume: 0.0,
                inertia_factor: 1.0,
                rotational_inertia_limit: 0.05,
                identity: 1,
                shape: tetrahedron(1.0),
                material: 0,
                kind: BodyKind::Dynamic,
                mass: 5.0,
                position: [0.0, 0.0, 50.0],
                angles: [0.0; 3],
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                linear_damping: 0.0,
                angular_damping: 0.0,
                drag: 0.0,
                collisions_enabled: false,
                gravity_enabled: false,
                drag_enabled: false,
            })
            .unwrap();
        pending
            .add_velocity(1, Some([10.0, 0.0, 0.0]), None)
            .unwrap();
        let velocity = pending.body(1).unwrap().internal_velocity();
        let queued = pending.body(1).unwrap().queued_velocity();
        let position = pending.body(1).unwrap().core_position();
        pending.sleep(1).unwrap();
        pending.simulate(pending.clock().timestep()).unwrap();
        assert_eq!(pending.body(1).unwrap().core_position(), position);
        assert_eq!(pending.body(1).unwrap().internal_velocity(), velocity);
        assert_eq!(pending.body(1).unwrap().queued_velocity(), queued);
        pending.wake(1).unwrap();
        pending.simulate(pending.clock().timestep()).unwrap();
        assert_ne!(pending.body(1).unwrap().core_position(), position);
    }

    #[test]
    fn velocity_writes_keep_pending_changes_separate_and_preserve_atomic_failures() {
        let mut world = collision_world();
        let phase = world.body(1).unwrap().motion_phase().unwrap();
        world.set_velocity(1, Some([10.0, 0.0, 0.0]), None).unwrap();
        assert_eq!(world.body(1).unwrap().internal_velocity().linear, [0.0; 3]);
        assert_eq!(
            world.body(1).unwrap().queued_velocity().linear,
            super::internal_direction([10.0, 0.0, 0.0], super::METERS_PER_INCH)
        );
        world.add_velocity(1, Some([5.0, 0.0, 0.0]), None).unwrap();
        assert_eq!(world.body(1).unwrap().motion_phase(), Some(phase));
        world
            .set_velocity_instantaneous(1, Some([2.0, 3.0, 4.0]), None)
            .unwrap();
        assert_eq!(world.body(1).unwrap().queued_velocity().linear, [0.0; 3]);
        assert_eq!(
            world.body(1).unwrap().internal_velocity().linear,
            super::internal_direction([2.0, 3.0, 4.0], super::METERS_PER_INCH)
        );
        let before = world.snapshot();
        assert_eq!(
            world.add_velocity(1, None, Some([f32::NAN, 0.0, 0.0])),
            Err(EnvironmentError::NonFinite)
        );
        assert_eq!(world.snapshot(), before);
    }

    pub(super) fn automatic_pair_world(rotating: bool) -> (PhysicsEnvironment, ConvexContactPair) {
        let mut world = collision_world_scale(10.0);
        let identity = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        let angular = if rotating { [0.0, 0.0, 20.0] } else { [0.0; 3] };
        let orientation = if rotating {
            identity.advance(angular, 0.035).unwrap()
        } else {
            identity
        };
        let velocity = if rotating { [0.0; 3] } else { [0.0, 1.0, 0.0] };
        let position = [0.25, if rotating { -0.11 } else { -0.12 }, 0.25];
        world
            .set_core_state(
                1,
                BodyCoreState {
                    position,
                    orientation,
                    collision_orientation: None,
                    velocity: VelocityState {
                        linear: velocity,
                        angular,
                    },
                },
            )
            .unwrap();
        world
            .set_motion_phase(
                1,
                BodyMotionPhase {
                    position,
                    prior_orientation: orientation,
                    next_orientation: orientation.advance(angular, config().timestep).unwrap(),
                    projection_velocity: velocity,
                    start: 0.0,
                    end: f64::from(config().timestep),
                    inverse_step: (1.0 / f64::from(config().timestep)) as f32,
                },
            )
            .unwrap();
        let moving = world.body(1).unwrap().topology(0).unwrap();
        let selected = moving
            .edges()
            .iter()
            .position(|edge| edge.start == 2)
            .unwrap();
        let vertex = moving.edge_id(selected).unwrap();
        let face = world
            .body(2)
            .unwrap()
            .topology(0)
            .unwrap()
            .edge_id(3)
            .unwrap();
        let pair = ConvexContactPair {
            endpoints: [
                BodyConvex { body: 1, convex: 0 },
                BodyConvex { body: 2, convex: 0 },
            ],
            seed: SurfaceFeaturePair {
                first: SurfaceFeature {
                    edge: vertex,
                    kind: SurfaceFeatureKind::Vertex,
                },
                second: SurfaceFeature {
                    edge: face,
                    kind: SurfaceFeatureKind::Face,
                },
            },
            start: 0.0,
            end: f64::from(config().timestep),
            maximum_feature_transitions: 1024,
        };
        (world, pair)
    }

    #[test]
    fn authored_pair_generates_contact_time_offset_and_response_without_an_injected_event() {
        let (mut world, pair) = automatic_pair_world(false);
        let predicted = world.schedule_convex_pair(21, pair).unwrap().unwrap();
        assert_eq!(predicted.kind, EventTimingKind::Collision);
        let expected_time =
            0.12 - f64::from(0.1_f32) - f64::from(world.contact_tolerances().collision_distance);
        assert!((predicted.time - expected_time).abs() < 1.0e-8);
        let saved = world.snapshot();
        assert_eq!(
            world.rekey_collision(21, 0.001),
            Err(EnvironmentError::DerivedEventTime)
        );
        assert_eq!(
            world.schedule_convex_pair(21, pair),
            Err(EnvironmentError::Event(
                crate::ContinuousError::DuplicateEvent
            ))
        );
        assert_eq!(world.snapshot(), saved);
        let mut replay = PhysicsEnvironment::new(config(), surfaces(b"0")).unwrap();
        replay.restore(saved).unwrap();
        world.simulate(world.clock().timestep()).unwrap();
        replay.simulate(replay.clock().timestep()).unwrap();
        assert_eq!(world.snapshot(), replay.snapshot());
        assert_eq!(world.collisions().len(), 1);
        assert_eq!(world.collisions()[0].event_identity, Some(21));
        let geometry = world.collisions()[0].geometry.unwrap();
        assert_ne!(geometry.shape_ids[0], geometry.shape_ids[1]);
        let topology = world
            .body(geometry.endpoints[0].body)
            .unwrap()
            .topology(geometry.endpoints[0].convex)
            .unwrap();
        let vertex = topology
            .edge(geometry.binding.features().first.edge)
            .unwrap();
        assert_eq!(
            geometry.synchronized_offsets[0],
            topology.points()[vertex.start as usize]
        );
        assert!(world.collisions()[0].contact_point().is_some());
        assert_eq!(world.collisions()[0].surface_normal()[2], -1.0);
        assert!((world.collisions()[0].after[0].linear[1] + 0.25).abs() < 1.0e-6);
        assert_eq!(world.pending_collisions().count(), 0);
    }

    #[test]
    fn pair_endpoint_order_and_derived_snapshot_validation_remain_owned() {
        let (mut forward, pair) = automatic_pair_world(false);
        let mut reverse = forward.clone();
        let reversed = ConvexContactPair {
            endpoints: [pair.endpoints[1], pair.endpoints[0]],
            seed: SurfaceFeaturePair {
                first: pair.seed.second,
                second: pair.seed.first,
            },
            ..pair
        };
        assert_eq!(
            forward.schedule_convex_pair(71, pair).unwrap(),
            reverse.schedule_convex_pair(71, reversed).unwrap()
        );
        forward.advance_events_before(pair.end).unwrap();
        reverse.advance_events_before(pair.end).unwrap();
        assert_eq!(forward.collisions(), reverse.collisions());
        assert_eq!(forward.body(1).unwrap(), reverse.body(1).unwrap());
        assert_eq!(
            forward.collisions()[0].geometry.unwrap().endpoints,
            pair.endpoints
        );
        let (mut world, pair) = automatic_pair_world(false);
        world.schedule_convex_pair(72, pair).unwrap();
        let before = world.snapshot();
        let mut invalid = before.clone();
        let super::QueuedCollisionInput::Pair { predicted, .. } =
            &mut invalid.queued_collisions[0].input
        else {
            unreachable!()
        };
        predicted.root.value = f64::NAN;
        assert_eq!(
            world.restore(invalid),
            Err(EnvironmentError::SnapshotMismatch)
        );
        assert_eq!(world.snapshot(), before);
        world.destroy_body(2).unwrap();
        assert_eq!(world.pending_collisions().count(), 0);
    }

    #[test]
    fn restored_search_policy_uses_the_snapshot_configuration() {
        let source = collision_world_scale(1.0);
        let mut config = source.config();
        config.timestep = 0.03;
        config.performance.lookahead_world = 2.0;
        config.performance.lookahead_bodies = 0.25;
        let mut destination =
            PhysicsEnvironment::new(config, Arc::clone(&source.surfaces)).unwrap();
        assert_ne!(
            destination.collision_search_ranges(),
            source.collision_search_ranges()
        );
        destination.restore(source.snapshot()).unwrap();
        assert_eq!(
            destination.collision_search_ranges(),
            source.collision_search_ranges()
        );
        assert_eq!(
            destination.collision_search_ranges(),
            crate::CollisionSearchRanges::new(
                source.config().timestep,
                source.config().performance.lookahead_world,
                source.config().performance.lookahead_bodies
            )
            .unwrap()
        );
    }

    #[test]
    fn bodies_sharing_one_collision_resource_retain_one_topology_identity() {
        let mut world = collision_world_scale(1.0);
        let shape = Arc::clone(&world.body(1).unwrap().shape);
        world
            .create_body(BodyInput {
                volume: 0.0,
                inertia_factor: 1.0,
                rotational_inertia_limit: 0.05,
                identity: 3,
                shape,
                material: 0,
                kind: BodyKind::Dynamic,
                mass: 7.0,
                position: [10.0, 0.0, 0.0],
                angles: [0.0; 3],
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                linear_damping: 0.0,
                angular_damping: 0.0,
                drag: 0.0,
                collisions_enabled: false,
                gravity_enabled: false,
                drag_enabled: false,
            })
            .unwrap();
        assert!(std::ptr::eq(
            world.body(1).unwrap().topology(0).unwrap(),
            world.body(3).unwrap().topology(0).unwrap()
        ));
        let mut replay =
            PhysicsEnvironment::new(world.config(), Arc::clone(&world.surfaces)).unwrap();
        replay.restore(world.snapshot()).unwrap();
        assert!(std::ptr::eq(
            replay.body(1).unwrap().topology(0).unwrap(),
            replay.body(3).unwrap().topology(0).unwrap()
        ));
    }

    #[test]
    fn waking_a_static_body_preserves_the_entire_environment() {
        let mut world = collision_world_scale(1.0);
        let before = world.snapshot();
        world.wake(2).unwrap();
        assert_eq!(world.snapshot(), before);
        assert!(world.body(2).unwrap().asleep);
        assert_eq!(world.wake(900), Err(EnvironmentError::MissingBody));
        assert_eq!(world.snapshot(), before);
    }

    #[test]
    fn environment_collision_capacity_reserves_the_boundary_from_the_actual_queue_limit() {
        let world = collision_world_scale(1.0);
        let config = EnvironmentConfig {
            max_events: 65_531,
            ..world.config()
        };
        assert!(config.validate().is_ok());
        assert!(
            EnvironmentConfig {
                max_events: 65_532,
                ..config
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn impulse_commands_preserve_static_pinned_and_invalid_input_semantics() {
        let mut world = collision_world_scale(1.0);
        assert!(world.body(2).unwrap().is_motion_enabled());
        assert!(!world.body(2).unwrap().is_moveable());
        let before = world.snapshot();
        world.apply_force_center(2, [f32::NAN; 3]).unwrap();
        world
            .apply_force_offset(2, [f32::NAN; 3], [f32::NAN; 3])
            .unwrap();
        world.apply_torque_center(2, [f32::NAN; 3]).unwrap();
        world.set_motion_enabled(2, false).unwrap();
        assert_eq!(world.snapshot(), before);
        world.sleep(1).unwrap();
        let before = world.snapshot();
        assert_eq!(
            world.apply_force_center(1, [f32::NAN; 3]),
            Err(EnvironmentError::Motion(crate::MotionError::NonFinite))
        );
        assert_eq!(world.snapshot(), before);
        world.apply_force_center(1, [0.0; 3]).unwrap();
        assert!(!world.body(1).unwrap().is_asleep());
        world.set_motion_enabled(1, false).unwrap();
        let before = world.snapshot();
        world.apply_force_offset(1, [100.0; 3], [1.0; 3]).unwrap();
        world.apply_torque_center(1, [100.0; 3]).unwrap();
        assert_eq!(world.snapshot(), before);
        let (linear, angular) = world
            .calculate_velocity_offset(1, [100.0; 3], [1.0; 3])
            .unwrap();
        assert_eq!(linear, [0.0; 3]);
        assert_eq!(angular, [0.0; 3]);
    }

    #[test]
    fn owned_phase_bounds_exclude_separating_stationary_and_unreachable_pairs_without_mutation() {
        for (linear, position_y) in [(-1.0, -0.12), (0.0, -0.12), (1.0, -1.0)] {
            let (mut world, pair) = automatic_pair_world(false);
            let prior = world.body(1).unwrap().motion_phase().unwrap();
            let phase = BodyMotionPhase {
                position: [0.25, position_y, 0.25],
                projection_velocity: [0.0, linear, 0.0],
                ..prior
            };
            world
                .set_core_state(
                    1,
                    BodyCoreState {
                        position: phase.position,
                        orientation: phase.prior_orientation,
                        collision_orientation: None,
                        velocity: VelocityState {
                            linear: phase.projection_velocity,
                            angular: [0.0; 3],
                        },
                    },
                )
                .unwrap();
            world.set_motion_phase(1, phase).unwrap();
            world.cached_transform(1).unwrap();
            world.cached_transform(2).unwrap();
            let before = world.snapshot();
            assert_eq!(world.schedule_convex_pair(30, pair).unwrap(), None);
            assert_eq!(world.snapshot(), before);
        }
    }

    #[test]
    fn pair_queries_commit_only_complete_unpinned_cache_and_queue_updates() {
        let (mut world, pair) = automatic_pair_world(false);
        let before = world.snapshot();
        assert!(
            world
                .schedule_convex_pair(
                    37,
                    ConvexContactPair {
                        maximum_feature_transitions: 0,
                        ..pair
                    }
                )
                .is_err()
        );
        assert_eq!(world.snapshot(), before);
        let cached = world.cached_transform(1).unwrap();
        world.schedule_convex_pair(37, pair).unwrap().unwrap();
        assert_eq!(world.transform_cache().current(1).unwrap().1, cached);
        // Both endpoints must have been released before the public operation returns.
        world.transforms.pin(1).unwrap();
        world.transforms.release(1).unwrap();
        world.transforms.pin(2).unwrap();
        world.transforms.release(2).unwrap();
        assert_eq!(
            world.transform_cache().owners().collect::<Vec<_>>(),
            vec![1, 2]
        );
        let before = world.snapshot();
        assert_eq!(
            world.schedule_convex_pair(
                38,
                ConvexContactPair {
                    start: 0.001,
                    ..pair
                }
            ),
            Err(EnvironmentError::InvalidMotionPhase)
        );
        assert_eq!(world.snapshot(), before);
    }

    #[test]
    fn close_rotating_contact_requires_reapproach_before_scheduling_a_collision() {
        let (mut world, pair) = automatic_pair_world(false);
        let prior = world.body(1).unwrap().motion_phase().unwrap();
        let angular = [0.0, 0.0, -20.0];
        let phase = BodyMotionPhase {
            position: [0.25, -0.105, 0.25],
            projection_velocity: [0.0; 3],
            next_orientation: prior
                .prior_orientation
                .advance(angular, config().timestep)
                .unwrap(),
            ..prior
        };
        world
            .set_core_state(
                1,
                BodyCoreState {
                    position: phase.position,
                    orientation: phase.prior_orientation,
                    collision_orientation: None,
                    velocity: VelocityState {
                        linear: [0.0; 3],
                        angular,
                    },
                },
            )
            .unwrap();
        world.set_motion_phase(1, phase).unwrap();
        world.cached_transform(1).unwrap();
        world.cached_transform(2).unwrap();
        let before = world.snapshot();
        assert_eq!(world.schedule_convex_pair(40, pair).unwrap(), None);
        assert_eq!(world.snapshot(), before);
    }

    #[test]
    fn close_contact_search_can_sample_one_cell_past_a_short_remaining_phase() {
        let (mut world, original_pair) = automatic_pair_world(false);
        let prior = world.body(1).unwrap().motion_phase().unwrap();
        let angular = [0.0, 0.0, -20.0];
        let duration = 0.001_f32;
        let phase = BodyMotionPhase {
            position: [0.25, -0.105, 0.25],
            projection_velocity: [0.0; 3],
            end: f64::from(duration),
            inverse_step: 1.0 / duration,
            next_orientation: prior.prior_orientation.advance(angular, duration).unwrap(),
            ..prior
        };
        world
            .set_core_state(
                1,
                BodyCoreState {
                    position: phase.position,
                    orientation: phase.prior_orientation,
                    collision_orientation: None,
                    velocity: VelocityState {
                        linear: [0.0; 3],
                        angular,
                    },
                },
            )
            .unwrap();
        world.set_motion_phase(1, phase).unwrap();
        let pair = ConvexContactPair {
            end: phase.end,
            ..original_pair
        };
        world.cached_transform(1).unwrap();
        world.cached_transform(2).unwrap();
        let before = world.snapshot();
        assert_eq!(
            phase.sample(f64::from(0.005_f32)),
            Err(EnvironmentError::InvalidMotionPhase)
        );
        assert!(phase.search_sample(f64::from(0.005_f32)).is_ok());
        assert_eq!(
            phase.search_sample(0.01),
            Err(EnvironmentError::InvalidMotionPhase)
        );
        assert_eq!(world.schedule_convex_pair(42, pair).unwrap(), None);
        assert_eq!(world.snapshot(), before);
    }

    #[test]
    fn close_contact_can_transition_its_feature_before_the_later_collision() {
        let (mut world, pair) = automatic_pair_world(true);
        let prior = world.body(1).unwrap().motion_phase().unwrap();
        let velocity = world.body(1).unwrap().internal_velocity();
        let phase = BodyMotionPhase {
            position: [0.25, -0.08, 0.25],
            ..prior
        };
        world
            .set_core_state(
                1,
                BodyCoreState {
                    position: phase.position,
                    orientation: phase.prior_orientation,
                    collision_orientation: None,
                    velocity,
                },
            )
            .unwrap();
        world.set_motion_phase(1, phase).unwrap();
        let first = world.schedule_convex_pair(41, pair).unwrap().unwrap();
        assert!(matches!(first.kind, EventTimingKind::FeatureTransition));
        assert!(first.time > 0.003 && first.time < 0.006);
        world.advance_events_before(pair.end).unwrap();
        assert_eq!(world.collisions().len(), 1);
        assert!(world.collisions()[0].event_time > first.time);
    }

    #[test]
    fn authored_sector_event_reminimizes_the_pair_and_preserves_collision_free_motion() {
        let (mut world, pair) = automatic_pair_world(true);
        let predicted = world.schedule_convex_pair(22, pair).unwrap().unwrap();
        assert!(matches!(predicted.kind, EventTimingKind::FeatureTransition));
        assert!((0.003..0.006).contains(&predicted.time));
        let before = world.body(1).unwrap().internal_velocity();
        let mut replay = PhysicsEnvironment::new(config(), surfaces(b"0")).unwrap();
        replay.restore(world.snapshot()).unwrap();
        assert_eq!(world.advance_events_before(pair.end).unwrap(), 1);
        assert_eq!(replay.advance_events_before(pair.end).unwrap(), 1);
        assert_eq!(world.snapshot(), replay.snapshot());
        assert_eq!(world.pending_collisions().count(), 0);
        assert!(world.collisions().is_empty());
        assert_eq!(world.body(1).unwrap().internal_velocity(), before);
        world.simulate(world.clock().timestep()).unwrap();
    }
}
