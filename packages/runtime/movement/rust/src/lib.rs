mod movement;

use playsrc_collision::{Hull, World as CollisionWorld};
use std::fmt;

pub const SNAPSHOT_VERSION: u32 = 1;
pub const MAX_COORDINATE: f32 = 16_384.0;
pub const MAX_COMMAND_MAGNITUDE: f32 = 3_500.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    pub tick_interval: f32,
    pub acceleration: f32,
    pub air_acceleration: f32,
    pub friction: f32,
    pub stop_speed: f32,
    pub gravity: f32,
    pub step_height: f32,
    pub step_epsilon: f32,
    pub ground_probe: f32,
    pub standable_normal: f32,
    pub maximum_velocity: f32,
    pub server_max_speed: f32,
    pub noclip_speed: f32,
    pub noclip_acceleration: f32,
    pub solid_mask: u32,
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
            step_height: 18.0,
            step_epsilon: 0.03125,
            ground_probe: 2.0,
            standable_normal: 0.7,
            maximum_velocity: 3_500.0,
            server_max_speed: 320.0,
            noclip_speed: 5.0,
            noclip_acceleration: 5.0,
            solid_mask: 0x0204_000b,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    pub maximum_speed: f32,
    pub air_speed_cap: f32,
    pub bunnyhop_speed_cap: Option<f32>,
    pub backward_speed_factor: f32,
    pub backward_speed_minimum: f32,
    pub ground_detach_speed: f32,
    pub jump_impulse: f32,
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
    pub start_fraction: f32,
    pub elapsed: f32,
    pub duration: f32,
}

impl CrouchState {
    pub const STANDING: Self = Self {
        phase: CrouchPhase::Standing,
        fraction: 0.0,
        start_fraction: 0.0,
        elapsed: 0.0,
        duration: 0.0,
    };

    pub const CROUCHED: Self = Self {
        phase: CrouchPhase::Crouched,
        fraction: 1.0,
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
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub mode: Mode,
    pub ground: Option<GroundState>,
    pub crouch: CrouchState,
    pub view_offset: [f32; 3],
    pub jump_latched: bool,
    pub fall_speed: f32,
    pub water_level: u8,
    pub previous_jump: bool,
    pub previous_crouch: bool,
    pub stuck_offset: u8,
}

impl State {
    pub fn from_player(player: Player, policy: Policy) -> Self {
        let crouch = if player.crouched {
            CrouchState::CROUCHED
        } else {
            CrouchState::STANDING
        };
        Self {
            position: player.position,
            velocity: player.velocity,
            mode: Mode::Walk,
            ground: player.grounded.then_some(GroundState {
                support: None,
                normal: [0.0, 0.0, 1.0],
                surface_friction: policy.surface_friction,
            }),
            crouch,
            view_offset: if player.crouched {
                policy.crouched_view
            } else {
                policy.standing_view
            },
            jump_latched: player.jump_latched,
            fall_speed: 0.0,
            water_level: 0,
            previous_jump: player.jump_latched,
            previous_crouch: player.crouched,
            stuck_offset: 0,
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

pub trait Tracer {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<Trace, Error>;
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
    StepUp,
    StepDown,
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
    Recovered {
        offset: u8,
    },
    Trapped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepResult {
    pub state: State,
    pub wish_velocity: [f32; 3],
    pub jump_velocity: [f32; 3],
    pub climbed_step: f32,
    pub contacts: Vec<Contact>,
    pub events: Vec<Event>,
    pub queries: Vec<QueryRecord>,
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
