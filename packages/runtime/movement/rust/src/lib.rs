mod movement;
mod mover;

pub use mover::{
    BlockContact, BlockContactKind, LinearPusherRequest, PUSHER_SNAPSHOT_VERSION, PushSubject,
    Pushability, PusherFrame, PusherHierarchyMemberRequest, PusherHierarchyTransform, PusherLimits,
    PusherResult, PusherSnapshot, PusherStatus, SubjectMove, TransformPusherRequest,
    advance_linear_pushers, advance_transform_pushers,
};

use playsrc_collision::{Hull, World as CollisionWorld};
use std::fmt;

pub const SNAPSHOT_VERSION: u32 = 1;
pub const MAX_COORDINATE: f32 = 16_384.0;
pub const MAX_COMMAND_MAGNITUDE: f32 = 3_500.0;

pub const CONTENTS_SLIME: u32 = 0x10;
pub const CONTENTS_WATER: u32 = 0x20;
pub const CONTENTS_MOVEABLE: u32 = 0x4000;
pub const CONTENTS_CURRENT_0: u32 = 0x40000;
pub const CONTENTS_CURRENT_90: u32 = 0x80000;
pub const CONTENTS_CURRENT_180: u32 = 0x100000;
pub const CONTENTS_CURRENT_270: u32 = 0x200000;
pub const CONTENTS_CURRENT_UP: u32 = 0x400000;
pub const CONTENTS_CURRENT_DOWN: u32 = 0x800000;
pub const CONTENTS_LADDER: u32 = 0x20000000;
pub const MASK_WATER: u32 = CONTENTS_WATER | CONTENTS_MOVEABLE | CONTENTS_SLIME;
pub const MASK_CURRENT: u32 = CONTENTS_CURRENT_0
    | CONTENTS_CURRENT_90
    | CONTENTS_CURRENT_180
    | CONTENTS_CURRENT_270
    | CONTENTS_CURRENT_UP
    | CONTENTS_CURRENT_DOWN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StuckRecoveryMode {
    ClientWorld,
    Incremental,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementConstraint {
    pub center: [f32; 3],
    pub radius: f32,
    pub width: f32,
    pub outward_speed_factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    pub tick_interval: f32,
    pub acceleration: f32,
    pub air_acceleration: f32,
    pub friction: f32,
    pub stop_speed: f32,
    pub gravity: f32,
    pub gravity_scale: f32,
    pub lagged_movement_scale: f32,
    pub step_height: f32,
    pub step_epsilon: f32,
    pub ground_probe: f32,
    pub stay_ground_rise: f32,
    pub ground_network_snap: f32,
    pub standable_normal: f32,
    pub maximum_velocity: f32,
    pub server_max_speed: f32,
    pub client_max_speed: f32,
    pub surface_max_speed_factor: f32,
    pub movement_constraint: Option<MovementConstraint>,
    pub noclip_speed: f32,
    pub noclip_acceleration: f32,
    pub spectator_speed: f32,
    pub spectator_acceleration: f32,
    pub spectator_noclip: bool,
    pub observer_hull: Option<Hull>,
    pub bounce: f32,
    pub allow_auto_movement: bool,
    pub ladders_enabled: bool,
    pub ladder_distance: f32,
    pub ladder_speed: f32,
    pub ladder_lateral_factor: f32,
    pub ladder_jump_speed: f32,
    pub water_idle_sink_speed: f32,
    pub water_wish_speed_factor: f32,
    pub water_swim_speed: f32,
    pub slime_swim_speed: f32,
    pub water_exit_forward: f32,
    pub water_exit_height: f32,
    pub water_exit_down: f32,
    pub water_exit_up_speed: f32,
    pub water_exit_push_speed: f32,
    pub water_exit_duration_ms: f32,
    pub water_exit_maximum_ms: f32,
    pub current_speed_per_level: f32,
    pub roll_speed: f32,
    pub roll_angle: f32,
    pub fall_punch_threshold: f32,
    pub floating_fall_reduction: f32,
    pub optimized_movement: bool,
    pub single_player: bool,
    pub stuck_recovery: StuckRecoveryMode,
    pub stuck_retry_seconds: f32,
    pub stuck_multiplayer_interval: f32,
    pub stuck_singleplayer_interval: f32,
    pub solid_mask: u32,
    pub water_mask: u32,
    pub ladder_mask: u32,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            tick_interval: 0.015,
            acceleration: 10.0,
            air_acceleration: 10.0,
            friction: 4.0,
            stop_speed: 100.0,
            gravity: 800.0,
            gravity_scale: 1.0,
            lagged_movement_scale: 1.0,
            step_height: 18.0,
            step_epsilon: 0.03125,
            ground_probe: 2.0,
            stay_ground_rise: 2.0,
            ground_network_snap: 0.015625,
            standable_normal: 0.7,
            maximum_velocity: 3_500.0,
            server_max_speed: 320.0,
            client_max_speed: 0.0,
            surface_max_speed_factor: 1.0,
            movement_constraint: None,
            noclip_speed: 5.0,
            noclip_acceleration: 5.0,
            spectator_speed: 3.0,
            spectator_acceleration: 5.0,
            spectator_noclip: true,
            observer_hull: None,
            bounce: 0.0,
            allow_auto_movement: true,
            ladders_enabled: true,
            ladder_distance: 2.0,
            ladder_speed: 200.0,
            ladder_lateral_factor: 1.0,
            ladder_jump_speed: 270.0,
            water_idle_sink_speed: 60.0,
            water_wish_speed_factor: 0.8,
            water_swim_speed: 100.0,
            slime_swim_speed: 80.0,
            water_exit_forward: 24.0,
            water_exit_height: 8.0,
            water_exit_down: 1_024.0,
            water_exit_up_speed: 256.0,
            water_exit_push_speed: 50.0,
            water_exit_duration_ms: 2_000.0,
            water_exit_maximum_ms: 10_000.0,
            current_speed_per_level: 50.0,
            roll_speed: 200.0,
            roll_angle: 0.0,
            fall_punch_threshold: 303.0,
            floating_fall_reduction: 173.0,
            optimized_movement: true,
            single_player: false,
            stuck_recovery: StuckRecoveryMode::ClientWorld,
            stuck_retry_seconds: 0.05,
            stuck_multiplayer_interval: 1.0,
            stuck_singleplayer_interval: 0.2,
            solid_mask: 0x0204_000b,
            water_mask: MASK_WATER,
            ladder_mask: 0x0204_000b | CONTENTS_LADDER,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub forward: f32,
    pub side: f32,
    pub yaw_degrees: f32,
    pub jump: bool,
    pub crouch: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepInput {
    pub command_number: u32,
    pub command: Command,
    pub pitch_degrees: f32,
    pub up: f32,
    pub speed_button: bool,
    pub mode_request: Option<ModeRequest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Mode {
    Walk = 0,
    Noclip = 1,
    None = 2,
    Isometric = 3,
    Fly = 4,
    FlyGravity = 5,
    Ladder = 6,
    Observer = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MoveCollision {
    Default = 0,
    Bounce = 1,
    Custom = 2,
    Slide = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ObserverMode {
    None = 0,
    Deathcam = 1,
    Freezecam = 2,
    Fixed = 3,
    InEye = 4,
    Chase = 5,
    PointOfInterest = 6,
    Roaming = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateDisposition {
    Preserve,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionDisposition {
    pub velocity: StateDisposition,
    pub ground: StateDisposition,
    pub water: StateDisposition,
}

impl TransitionDisposition {
    pub const PRESERVE: Self = Self {
        velocity: StateDisposition::Preserve,
        ground: StateDisposition::Preserve,
        water: StateDisposition::Preserve,
    };

    pub const RESET_ENVIRONMENT: Self = Self {
        velocity: StateDisposition::Preserve,
        ground: StateDisposition::Reset,
        water: StateDisposition::Reset,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModeRequest {
    pub mode: Mode,
    pub disposition: TransitionDisposition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepStrategy {
    FurthestHorizontal,
    HighFirst,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterSampling {
    WaistThenEyes,
    EyesThenWaist,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPolicy {
    pub sampling: WaterSampling,
    pub waist_height_offset: f32,
    pub refresh_before_walk: bool,
    pub apply_currents: bool,
    pub jump_wish_at_waist: bool,
    pub amplify_forward_pitch: bool,
    pub ledge_uses_command_direction: bool,
    pub ledge_jump_overrides_backward: bool,
    pub suppress_airborne_duck: bool,
    pub suppress_submerged_duck: bool,
}

impl Default for WaterPolicy {
    fn default() -> Self {
        Self {
            sampling: WaterSampling::WaistThenEyes,
            waist_height_offset: 0.0,
            refresh_before_walk: true,
            apply_currents: true,
            jump_wish_at_waist: true,
            amplify_forward_pitch: true,
            ledge_uses_command_direction: false,
            ledge_jump_overrides_backward: false,
            suppress_airborne_duck: false,
            suppress_submerged_duck: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    pub maximum_speed: f32,
    pub air_speed_cap: f32,
    pub bunnyhop_speed_cap: Option<f32>,
    pub backward_speed_factor: f32,
    pub backward_speed_minimum: f32,
    pub ground_detach_speed: f32,
    pub jump_impulse: f32,
    pub air_dash_impulse: Option<f32>,
    pub surface_friction: f32,
    pub surface_jump_factor: f32,
    pub standing_hull: Hull,
    pub crouched_hull: Hull,
    pub standing_view: [f32; 3],
    pub crouched_view: [f32; 3],
    pub duck_duration: f32,
    pub unduck_duration: f32,
    pub crouched_command_factor: f32,
    pub allow_jump: bool,
    pub allow_duck: bool,
    pub allow_noclip: bool,
    pub allow_crouched_jump: bool,
    pub replace_vertical_while_ducking: bool,
    pub step_strategy: StepStrategy,
    pub water: WaterPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            maximum_speed: 320.0,
            air_speed_cap: 30.0,
            bunnyhop_speed_cap: None,
            backward_speed_factor: 1.0,
            backward_speed_minimum: 0.0,
            ground_detach_speed: 140.0,
            jump_impulse: 268.328_16,
            air_dash_impulse: None,
            surface_friction: 1.0,
            surface_jump_factor: 1.0,
            standing_hull: Hull {
                mins: [-16.0, -16.0, 0.0],
                maxs: [16.0, 16.0, 72.0],
            },
            crouched_hull: Hull {
                mins: [-16.0, -16.0, 0.0],
                maxs: [16.0, 16.0, 36.0],
            },
            standing_view: [0.0, 0.0, 64.0],
            crouched_view: [0.0, 0.0, 28.0],
            duck_duration: 0.4,
            unduck_duration: 0.2,
            crouched_command_factor: 0.333_333_34,
            allow_jump: true,
            allow_duck: true,
            allow_noclip: false,
            allow_crouched_jump: true,
            replace_vertical_while_ducking: true,
            step_strategy: StepStrategy::FurthestHorizontal,
            water: WaterPolicy::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub grounded: bool,
    pub crouched: bool,
    pub jump_latched: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundState {
    pub support: Option<u64>,
    pub normal: [f32; 3],
    pub surface_friction: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CrouchPhase {
    Standing = 0,
    Ducking = 1,
    Crouched = 2,
    Unducking = 3,
    Blocked = 4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrouchState {
    pub phase: CrouchPhase,
    pub fraction: f32,
    pub linear_fraction: f32,
    pub start_fraction: f32,
    pub elapsed: f32,
    pub duration: f32,
}

impl CrouchState {
    pub const STANDING: Self = Self {
        phase: CrouchPhase::Standing,
        fraction: 0.0,
        linear_fraction: 0.0,
        start_fraction: 0.0,
        elapsed: 0.0,
        duration: 0.0,
    };

    pub const CROUCHED: Self = Self {
        phase: CrouchPhase::Crouched,
        fraction: 1.0,
        linear_fraction: 1.0,
        start_fraction: 1.0,
        elapsed: 0.0,
        duration: 0.0,
    };

    pub fn uses_crouched_hull(self) -> bool {
        matches!(
            self.phase,
            CrouchPhase::Crouched | CrouchPhase::Unducking | CrouchPhase::Blocked
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub player_identity: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub base_velocity: [f32; 3],
    pub base_velocity_applied: bool,
    pub local_angles: [f32; 3],
    pub absolute_view_angles: [f32; 3],
    pub mode: Mode,
    pub move_collision: MoveCollision,
    pub observer_mode: ObserverMode,
    pub observer_target: Option<u64>,
    pub ground: Option<GroundState>,
    pub surface_friction: f32,
    pub ladder_normal: [f32; 3],
    pub crouch: CrouchState,
    pub view_offset: [f32; 3],
    pub jump_latched: bool,
    pub fall_speed: f32,
    pub water_level: u8,
    pub water_type: u32,
    pub water_jump_time_ms: f32,
    pub water_jump_velocity: [f32; 3],
    pub previous_jump: bool,
    pub previous_crouch: bool,
    pub previous_forward: f32,
    pub stuck_offset: u8,
    pub stuck_next_check_time: f32,
}

impl State {
    pub fn from_player(player: Player, policy: Policy) -> Self {
        let crouch = if player.crouched {
            CrouchState::CROUCHED
        } else {
            CrouchState::STANDING
        };
        Self {
            player_identity: 0,
            position: player.position,
            velocity: player.velocity,
            base_velocity: [0.0; 3],
            base_velocity_applied: false,
            local_angles: [0.0; 3],
            absolute_view_angles: [0.0; 3],
            mode: Mode::Walk,
            move_collision: MoveCollision::Default,
            observer_mode: ObserverMode::None,
            observer_target: None,
            ground: player.grounded.then_some(GroundState {
                support: None,
                normal: [0.0, 0.0, 1.0],
                surface_friction: policy.surface_friction,
            }),
            surface_friction: policy.surface_friction,
            ladder_normal: [0.0; 3],
            crouch,
            view_offset: if player.crouched {
                policy.crouched_view
            } else {
                policy.standing_view
            },
            jump_latched: player.jump_latched,
            fall_speed: 0.0,
            water_level: 0,
            water_type: 0,
            water_jump_time_ms: 0.0,
            water_jump_velocity: [0.0; 3],
            previous_jump: player.jump_latched,
            previous_crouch: player.crouched,
            previous_forward: 0.0,
            stuck_offset: 0,
            stuck_next_check_time: 0.0,
        }
    }

    pub fn player(self) -> Player {
        Player {
            position: self.position,
            velocity: self.velocity,
            grounded: self.ground.is_some(),
            crouched: self.crouch.uses_crouched_hull(),
            jump_latched: self.jump_latched,
        }
    }

    pub fn active_hull(self, policy: Policy) -> Hull {
        if self.crouch.uses_crouched_hull() {
            policy.crouched_hull
        } else {
            policy.standing_hull
        }
    }

    pub fn snapshot_bytes(self) -> Vec<u8> {
        let mut bytes = b"PMOV".to_vec();
        bytes.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
        bytes.push(self.mode as u8);
        bytes.push(self.crouch.phase as u8);
        bytes.push(self.water_level);
        bytes.push(u8::from(self.jump_latched));
        bytes.push(u8::from(self.previous_jump));
        bytes.push(u8::from(self.previous_crouch));
        bytes.push(self.stuck_offset);
        bytes.push(u8::from(self.ground.is_some()));
        bytes.extend_from_slice(
            &self
                .ground
                .and_then(|ground| ground.support)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        for value in self
            .position
            .into_iter()
            .chain(self.velocity)
            .chain(self.view_offset)
            .chain([
                self.crouch.fraction,
                self.crouch.start_fraction,
                self.crouch.elapsed,
                self.crouch.duration,
                self.fall_speed,
            ])
            .chain(self.ground.map(|ground| ground.normal).unwrap_or([0.0; 3]))
            .chain([self
                .ground
                .map(|ground| ground.surface_friction)
                .unwrap_or(0.0)])
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub fraction: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub end: [f32; 3],
    pub normal: Option<[f32; 3]>,
    pub hit: Option<u64>,
    pub contents: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverTarget {
    pub position: [f32; 3],
    pub angles: [f32; 3],
    pub velocity: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoverMotion {
    pub identity: u64,
    pub displacement: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub swept_contact: bool,
    pub unblockable: bool,
}

pub trait Tracer {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<Trace, Error>;

    fn point_contents(&self, point: [f32; 3]) -> Result<u32, Error> {
        self.trace(
            point,
            point,
            Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            },
            u32::MAX,
        )
        .map(|trace| trace.contents)
    }

    fn support_velocity(&self, _support: u64) -> Result<[f32; 3], Error> {
        Ok([0.0; 3])
    }

    fn support_is_floating(&self, _support: u64) -> Result<bool, Error> {
        Ok(false)
    }

    fn conveyor_velocity(&self, _support: u64) -> Result<Option<[f32; 3]>, Error> {
        Ok(None)
    }

    fn is_world(&self, _hit: u64) -> bool {
        true
    }

    fn surface_climbable(&self, _hit: Option<u64>) -> bool {
        false
    }

    fn mover_motion(
        &self,
        _position: [f32; 3],
        _hull: Hull,
        _support: Option<u64>,
    ) -> Result<Option<MoverMotion>, Error> {
        Ok(None)
    }

    fn trace_without(
        &self,
        _ignored: u64,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, Error> {
        self.trace(start, end, hull, mask)
    }

    fn overlaps_mover(&self, _mover: u64, _position: [f32; 3], _hull: Hull) -> Result<bool, Error> {
        Ok(false)
    }

    fn observer_target(&self, _target: u64) -> Result<Option<ObserverTarget>, Error> {
        Ok(None)
    }

    fn movement_time(&self) -> Option<f32> {
        None
    }
}

impl Tracer for CollisionWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<Trace, Error> {
        let trace = self
            .trace_hull(start, end, hull, mask)
            .map_err(|_| Error::new(Operation::Trace, FailureKind::Malformed, "collision"))?;
        Ok(Trace {
            fraction: trace.fraction,
            start_solid: trace.start_solid,
            all_solid: trace.all_solid,
            end: trace.end,
            normal: trace.plane.map(|plane| plane.normal),
            hit: trace.brush.map(|brush| brush as u64),
            contents: trace.contents,
        })
    }

    fn point_contents(&self, point: [f32; 3]) -> Result<u32, Error> {
        self.trace_hull(
            point,
            point,
            Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            },
            u32::MAX,
        )
        .map(|trace| trace.contents)
        .map_err(|_| {
            Error::new(
                Operation::PointContents,
                FailureKind::Malformed,
                "collision",
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    Malformed,
    Unsupported,
    Unknown,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Validate,
    Trace,
    PointContents,
    Mover,
    Move,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub command_number: u32,
    pub operation: Operation,
    pub kind: FailureKind,
    pub field: &'static str,
    pub offending_bits: Option<u32>,
    pub limit_bits: Option<u32>,
}

impl Error {
    pub const fn new(operation: Operation, kind: FailureKind, field: &'static str) -> Self {
        Self {
            command_number: 0,
            operation,
            kind,
            field,
            offending_bits: None,
            limit_bits: None,
        }
    }

    pub const fn with_command(mut self, command_number: u32) -> Self {
        self.command_number = command_number;
        self
    }

    pub fn with_value(mut self, offending: f32, limit: Option<f32>) -> Self {
        self.offending_bits = Some(offending.to_bits());
        self.limit_bits = limit.map(f32::to_bits);
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "command {} {:?} {:?}: {}",
            self.command_number, self.operation, self.kind, self.field
        )
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryPurpose {
    InitialPosition,
    StuckRecovery,
    GroundFull,
    GroundQuadrant,
    Unduck,
    Displacement,
    Endpoint,
    StayGroundUp,
    StayGroundDown,
    StepUp,
    StepDown,
    WaterWaist,
    WaterEye,
    WaterLanding,
    Ladder,
    MoverDisplacement,
    TossDisplacement,
    ObserverDisplacement,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryRecord {
    pub purpose: QueryPurpose,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
    pub result: Trace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointQueryPurpose {
    WaterFeet,
    WaterWaist,
    WaterEyes,
    LadderFloor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointQueryRecord {
    pub purpose: PointQueryPurpose,
    pub point: [f32; 3],
    pub contents: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub hit: Option<u64>,
    pub normal: [f32; 3],
    pub impact_velocity: [f32; 3],
    pub order: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    ModeChanged {
        from: Mode,
        to: Mode,
        disposition: TransitionDisposition,
    },
    ModeDenied {
        requested: Mode,
    },
    GroundChanged {
        from: Option<u64>,
        to: Option<u64>,
    },
    Jumped,
    Landed {
        fall_speed: f32,
    },
    Stepped {
        height: f32,
    },
    CrouchChanged {
        from: CrouchPhase,
        to: CrouchPhase,
    },
    WaterEntered {
        level: u8,
        contents: u32,
    },
    WaterExited {
        previous_level: u8,
        contents: u32,
    },
    LadderAttached,
    LadderDetached,
    Recovered {
        offset: u8,
    },
    Trapped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoverStatus {
    Moved,
    Blocked,
    Crushed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoverResult {
    pub identity: u64,
    pub status: MoverStatus,
    pub displacement: [f32; 3],
    pub support_velocity: [f32; 3],
    pub blocker: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WishState {
    pub direction: [f32; 3],
    pub speed: f32,
    pub uncapped_speed: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepResult {
    pub state: State,
    pub selected_hull: Hull,
    pub wish_state: WishState,
    pub wish_velocity: [f32; 3],
    pub jump_velocity: [f32; 3],
    pub climbed_step: f32,
    pub contacts: Vec<Contact>,
    pub events: Vec<Event>,
    pub queries: Vec<QueryRecord>,
    pub point_queries: Vec<PointQueryRecord>,
    pub mover_result: Option<MoverResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickTrace {
    pub command_number: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub wish_state: WishState,
    pub wish_velocity: [f32; 3],
    pub ground: Option<GroundState>,
    pub hull: Hull,
    pub crouch_phase: CrouchPhase,
    pub crouch_fraction: f32,
    pub view_offset: [f32; 3],
    pub contacts: Vec<Contact>,
    pub events: Vec<Event>,
    pub mover_result: Option<MoverResult>,
}

impl StepResult {
    pub fn tick_trace(&self, command_number: u32) -> TickTrace {
        TickTrace {
            command_number,
            position: self.state.position,
            velocity: self.state.velocity,
            wish_state: self.wish_state,
            wish_velocity: self.wish_velocity,
            ground: self.state.ground,
            hull: self.selected_hull,
            crouch_phase: self.state.crouch.phase,
            crouch_fraction: self.state.crouch.fraction,
            view_offset: self.state.view_offset,
            contacts: self.contacts.clone(),
            events: self.events.clone(),
            mover_result: self.mover_result,
        }
    }
}

pub fn step(
    tracer: &impl Tracer,
    state: State,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> Result<StepResult, Error> {
    movement::step(tracer, state, input, configuration, policy)
        .map_err(|error| error.with_command(input.command_number))
}

#[cfg(test)]
mod tests;
