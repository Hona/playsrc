pub mod combat;
mod map_runtime;
pub mod weapon;

#[path = "../../rulesets/jump/rust/src/lib.rs"]
pub mod jump;

pub use map_runtime::{
    CONTENTS_BLUE_TEAM, CONTENTS_RED_TEAM, Effect as MapEffect, EntityEvent, EntityEventKind,
    EntityTransform, GameplayWorld, MapCounts, MapPhase, MapRuntime, MoverRequest, MoverResult,
    MoverResultKind, PlayerContactFacts, respawn_barrier_collides,
};

use std::collections::BTreeMap;

use playsrc_collision::Hull;
use playsrc_movement::{
    Command as MoveCommand, Configuration as MovementConfiguration, Error as MoveError, Mode,
    ModeRequest, Player, Policy as GenericMovementPolicy, State as MovementState, StepInput,
    StepResult as MovementStepResult, StepStrategy, TransitionDisposition, step,
};

use map_runtime::{BeginTickInput, MapError};
use weapon::{ActivityEvent, PrimaryResult, ReloadPhase, WeaponRuntime};

pub const PLAYER_IDENTITY: u32 = 1;
pub const MAX_PROJECTILES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Class {
    Soldier = 1,
    Demoman = 2,
}

impl Class {
    pub const fn source_number(self) -> u8 {
        match self {
            Self::Soldier => 3,
            Self::Demoman => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Team {
    Red = 1,
    Blue = 2,
}

impl Team {
    pub const fn source_number(self) -> u8 {
        match self {
            Self::Red => 2,
            Self::Blue => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementModifiers {
    pub condition_speed_factor: f32,
    pub item_speed_factor: f32,
    pub condition_jump_factor: f32,
    pub item_jump_factor: f32,
    pub air_control_factor: f32,
    pub surface_friction: f32,
    pub surface_jump_factor: f32,
    pub can_jump: bool,
    pub can_duck: bool,
    pub noclip_allowed: bool,
}

impl Default for MovementModifiers {
    fn default() -> Self {
        Self {
            condition_speed_factor: 1.0,
            item_speed_factor: 1.0,
            condition_jump_factor: 1.0,
            item_jump_factor: 1.0,
            air_control_factor: 1.0,
            surface_friction: 1.0,
            surface_jump_factor: 1.0,
            can_jump: true,
            can_duck: true,
            noclip_allowed: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementPolicy {
    pub class: Class,
    pub modifiers: MovementModifiers,
}

impl MovementPolicy {
    pub fn resolve(self) -> GenericMovementPolicy {
        let class_speed = match self.class {
            Class::Soldier => 240.0,
            Class::Demoman => 280.0,
        };
        let maximum_speed =
            class_speed * self.modifiers.condition_speed_factor * self.modifiers.item_speed_factor;
        GenericMovementPolicy {
            maximum_speed,
            air_speed_cap: 30.0 * self.modifiers.air_control_factor,
            bunnyhop_speed_cap: Some(maximum_speed * 1.2),
            backward_speed_factor: 0.9,
            backward_speed_minimum: 100.0,
            ground_detach_speed: 250.0,
            jump_impulse: 289.0
                * self.modifiers.condition_jump_factor
                * self.modifiers.item_jump_factor,
            surface_friction: self.modifiers.surface_friction,
            surface_jump_factor: self.modifiers.surface_jump_factor,
            standing_hull: Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 82.0],
            },
            crouched_hull: Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 62.0],
            },
            standing_view: [0.0, 0.0, 68.0],
            crouched_view: [0.0, 0.0, 45.0],
            duck_duration: 0.2,
            unduck_duration: 0.2,
            crouched_command_factor: 0.333_333_34,
            allow_jump: self.modifiers.can_jump,
            allow_duck: self.modifiers.can_duck,
            allow_noclip: self.modifiers.noclip_allowed,
            allow_crouched_jump: false,
            replace_vertical_while_ducking: true,
            step_strategy: StepStrategy::HighFirst,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Weapon {
    RocketLauncher = 1,
    Original = 2,
    StickybombLauncher = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReloadState {
    Idle = 0,
    Starting = 1,
    Loading = 2,
    Finishing = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponState {
    pub weapon: Weapon,
    pub clip: u16,
    pub reserve: u16,
    pub maximum_clip: u16,
    pub maximum_reserve: u16,
    pub reload: ReloadState,
    pub next_primary_tick: u64,
    pub next_reload_tick: u64,
}

impl WeaponState {
    fn from_runtime(runtime: WeaponRuntime) -> Self {
        let profile = runtime.profile();
        Self {
            weapon: runtime.weapon,
            clip: runtime.clip,
            reserve: runtime.reserve,
            maximum_clip: profile.maximum_clip,
            maximum_reserve: profile.maximum_reserve,
            reload: match runtime.reload {
                ReloadPhase::Ready => ReloadState::Idle,
                ReloadPhase::Start => ReloadState::Starting,
                ReloadPhase::Insert => ReloadState::Loading,
                ReloadPhase::Finish => ReloadState::Finishing,
            },
            next_primary_tick: runtime.next_primary_tick,
            next_reload_tick: runtime.reload_due_tick.unwrap_or(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub movement: MoveCommand,
    pub pitch_degrees: f32,
    pub up: f32,
    pub speed_button: bool,
    pub fire: bool,
    pub detonate: bool,
    pub reload: bool,
    pub reset: bool,
    pub respawn: bool,
    pub select_class: Option<Class>,
    pub select_team: Option<Team>,
    pub select_weapon: Option<Weapon>,
    pub mode_request: Option<Mode>,
    pub activate_entity: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectileKind {
    Rocket = 1,
    Sticky = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectileState {
    Flying = 1,
    StuckUnarmed = 2,
    StuckArmed = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectileContactKind {
    World,
    DynamicProp,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileContact {
    pub kind: ProjectileContactKind,
    pub normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectilePhysicsResult {
    pub projectile: u32,
    pub tick: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub orientation: [f32; 4],
    pub angular_velocity: [f32; 3],
    pub motion_enabled: bool,
    pub contact: Option<ProjectileContact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectilePhysicsOperation {
    Create,
    Step,
    DisableMotion,
    Destroy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectilePhysicsRequest {
    pub operation: ProjectilePhysicsOperation,
    pub projectile: u32,
    pub tick: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub orientation: [f32; 4],
    pub angular_velocity: [f32; 3],
    pub hull: Hull,
    pub gravity_scale: f32,
    pub friction: f32,
    pub elasticity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RocketTraceRequest {
    pub projectile: u32,
    pub tick: u64,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RocketTraceResult {
    pub projectile: u32,
    pub tick: u64,
    pub end: [f32; 3],
    pub solid: bool,
    pub sky: bool,
    pub normal: Option<[f32; 3]>,
    pub direct_target: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickyLaunchRandom {
    pub right_velocity: f32,
    pub up_velocity: f32,
    pub angular_y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiusDamageRequest {
    pub projectile: u32,
    pub kind: ProjectileKind,
    pub source: [f32; 3],
    pub base_damage: f32,
    pub radius: f32,
    pub self_radius: f32,
    pub direct_target: Option<u32>,
}

impl StickyLaunchRandom {
    pub fn validate(self) -> bool {
        self.right_velocity.is_finite()
            && (-10.0..=10.0).contains(&self.right_velocity)
            && self.up_velocity.is_finite()
            && (-10.0..=10.0).contains(&self.up_velocity)
            && (-1200..=1200).contains(&self.angular_y)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub identity: u32,
    pub kind: ProjectileKind,
    pub team: Team,
    pub owner_identity: u32,
    pub launcher_identity: u32,
    pub state: ProjectileState,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub orientation: [f32; 4],
    pub angular_velocity: [f32; 3],
    pub contact_normal: Option<[f32; 3]>,
    pub age_seconds: f32,
}

#[derive(Clone, Debug)]
struct LiveProjectile {
    presentation: Projectile,
    armed: bool,
    creation_tick: u64,
    arm_tick: u64,
    next_think_tick: u64,
    forced_detonate_tick: Option<u64>,
    motion_enabled: bool,
    direct_target: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectileEventKind {
    Fire = 1,
    Impact = 2,
    Stick = 3,
    Arm = 4,
    Fizzle = 5,
    Explode = 6,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectileEvent {
    pub kind: ProjectileEventKind,
    pub projectile: u32,
    pub projectile_kind: ProjectileKind,
    pub owner_identity: u32,
    pub launcher_identity: u32,
    pub team: Team,
    pub tick: u64,
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub contact_normal: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Condition {
    Phase = 14,
    EnergyBuff = 19,
    Burning = 22,
    Urine = 24,
    Bleeding = 25,
    MadMilk = 27,
    CannotSwitchFromMelee = 41,
    ParachuteActive = 80,
    BlastJumping = 81,
    Plague = 112,
    Gas = 123,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConditionSet {
    words: [u32; 5],
}

impl ConditionSet {
    pub fn contains(self, condition: Condition) -> bool {
        let value = condition as usize;
        self.words[value / 32] & (1_u32 << (value % 32)) != 0
    }

    pub fn insert(&mut self, condition: Condition) {
        let value = condition as usize;
        self.words[value / 32] |= 1_u32 << (value % 32);
    }

    pub fn remove(&mut self, condition: Condition) {
        let value = condition as usize;
        self.words[value / 32] &= !(1_u32 << (value % 32));
    }

    pub fn clear(&mut self) {
        self.words = [0; 5];
    }

    pub fn words(self) -> [u32; 5] {
        self.words
    }

    fn first_word(self) -> u32 {
        self.words[0]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerLifecycle {
    Active,
    Dying,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleEventKind {
    Died,
    Respawned,
    ClassChanged,
    TeamChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleEvent {
    pub tick: u64,
    pub kind: LifecycleEventKind,
    pub class: Class,
    pub team: Team,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegenerateAnimationEvent {
    pub zone: u32,
    pub associated_model: u32,
    pub open_tick: u64,
    pub close_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactReconcileRequest {
    pub tick: u64,
    pub position: [f32; 3],
    pub hull: Hull,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerRestrictions {
    pub taunting: bool,
    pub stalemate: bool,
    pub team_win: Option<Team>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    ClassChanged(Class),
    TeamChanged(Team),
    WeaponChanged(Weapon),
    Reloaded {
        weapon: Weapon,
        clip: u16,
        reserve: u16,
    },
    Resupplied {
        entity: u32,
        health: f32,
        weapon: Weapon,
        clip: u16,
        reserve: u16,
    },
    Damaged {
        amount: f32,
        health: f32,
    },
    Healed {
        amount: f32,
        health: f32,
        trigger: u32,
    },
    BlastImpulse {
        velocity: [f32; 3],
    },
    Teleported {
        trigger: u32,
        destination: u32,
        position: [f32; 3],
        yaw_degrees: Option<f32>,
    },
    TriggerVelocity {
        trigger: u32,
        velocity: [f32; 3],
    },
    Respawned,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub class: Class,
    pub team: Team,
    pub weapon: Weapon,
    pub movement: MovementState,
    pub health: f32,
    pub maximum_health: f32,
    pub loadout: Vec<WeaponState>,
    pub conditions: u32,
    pub projectiles: Vec<Projectile>,
    pub projectile_events: Vec<ProjectileEvent>,
    pub entity_transforms: Vec<EntityTransform>,
    pub entity_events: Vec<EntityEvent>,
    pub jump: Option<jump::TickOutput>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProducerSnapshot {
    pub tick: u64,
    pub lifecycle: PlayerLifecycle,
    pub class: Class,
    pub team: Team,
    pub active_weapon: Weapon,
    pub health: i32,
    pub maximum_health: i32,
    pub conditions: [u32; 5],
    pub weapons: Vec<WeaponRuntime>,
    pub projectiles: Vec<Projectile>,
    pub activities: Vec<ActivityEvent>,
    pub lifecycle_events: Vec<LifecycleEvent>,
    pub physics_requests: Vec<ProjectilePhysicsRequest>,
    pub rocket_trace_requests: Vec<RocketTraceRequest>,
    pub radius_damage_requests: Vec<RadiusDamageRequest>,
    pub mover_requests: Vec<MoverRequest>,
    pub contact_reconcile_requests: Vec<ContactReconcileRequest>,
    pub map_effects: Vec<MapEffect>,
    pub regenerate_animation_events: Vec<RegenerateAnimationEvent>,
}

#[derive(Clone)]
pub struct Session<W: GameplayWorld + Clone> {
    collision: W,
    tick: u64,
    class: Class,
    team: Team,
    weapon: Weapon,
    loadout: BTreeMap<Weapon, WeaponRuntime>,
    movement: MovementState,
    movement_modifiers: MovementModifiers,
    last_movement: Option<MovementStepResult>,
    health: i32,
    conditions: ConditionSet,
    lifecycle: PlayerLifecycle,
    restrictions: PlayerRestrictions,
    auto_reload: bool,
    spawn: [f32; 3],
    projectiles: Vec<LiveProjectile>,
    next_projectile: u32,
    fire_was_held: bool,
    physics_requests: Vec<ProjectilePhysicsRequest>,
    activity_events: Vec<ActivityEvent>,
    mover_requests: Vec<MoverRequest>,
    lifecycle_events: Vec<LifecycleEvent>,
    regenerate_animation_events: Vec<RegenerateAnimationEvent>,
    map_effects: Vec<MapEffect>,
    radius_damage_requests: Vec<RadiusDamageRequest>,
    rocket_trace_requests: Vec<RocketTraceRequest>,
    contact_reconcile_requests: Vec<ContactReconcileRequest>,
    movement_configuration: MovementConfiguration,
    map: MapRuntime,
    next_regenerate_tick: u64,
    hurt_next_tick: BTreeMap<u32, u64>,
    hurt_active: std::collections::BTreeSet<u32>,
    hurt_applied: std::collections::BTreeSet<u32>,
    respawn_touch_count: u32,
    jump: Option<jump::Session>,
}

#[derive(Debug)]
pub enum Error {
    Movement(MoveError),
    Entity(playsrc_entity::RuntimeFailure),
    Jump(jump::Error),
    MissingEntity(u32),
    InvalidCourseTrigger(u32),
    ProjectileLimit,
    MissingStickyLaunchRandom,
    InvalidStickyLaunchRandom,
    InvalidProjectilePhysics,
}

impl From<MoveError> for Error {
    fn from(error: MoveError) -> Self {
        Self::Movement(error)
    }
}

impl From<playsrc_entity::RuntimeFailure> for Error {
    fn from(error: playsrc_entity::RuntimeFailure) -> Self {
        Self::Entity(error)
    }
}

impl From<jump::Error> for Error {
    fn from(error: jump::Error) -> Self {
        Self::Jump(error)
    }
}

impl From<MapError> for Error {
    fn from(error: MapError) -> Self {
        match error {
            MapError::Entity(error) => Self::Entity(error),
            MapError::Movement(error) => Self::Movement(error),
            MapError::MissingEntity(entity) => Self::MissingEntity(entity),
        }
    }
}

impl<W: GameplayWorld + Clone> Session<W> {
    pub fn new(collision: W, spawn: [f32; 3], map: MapRuntime) -> Self {
        let movement_modifiers = MovementModifiers::default();
        let movement_policy = MovementPolicy {
            class: Class::Soldier,
            modifiers: movement_modifiers,
        }
        .resolve();
        let loadout = BTreeMap::from([
            (
                Weapon::RocketLauncher,
                WeaponRuntime::full(Weapon::RocketLauncher),
            ),
            (Weapon::Original, WeaponRuntime::full(Weapon::Original)),
        ]);
        Self {
            collision,
            tick: 0,
            class: Class::Soldier,
            team: Team::Red,
            weapon: Weapon::RocketLauncher,
            loadout,
            movement: MovementState::from_player(
                Player {
                    position: spawn,
                    velocity: [0.0; 3],
                    grounded: false,
                    crouched: false,
                    jump_latched: false,
                },
                movement_policy,
            ),
            movement_modifiers,
            last_movement: None,
            health: stock_maximum_health(Class::Soldier),
            conditions: ConditionSet::default(),
            lifecycle: PlayerLifecycle::Active,
            restrictions: PlayerRestrictions::default(),
            auto_reload: true,
            spawn,
            projectiles: Vec::new(),
            next_projectile: 1,
            fire_was_held: false,
            physics_requests: Vec::new(),
            activity_events: Vec::new(),
            mover_requests: Vec::new(),
            lifecycle_events: Vec::new(),
            regenerate_animation_events: Vec::new(),
            map_effects: Vec::new(),
            radius_damage_requests: Vec::new(),
            rocket_trace_requests: Vec::new(),
            contact_reconcile_requests: Vec::new(),
            movement_configuration: MovementConfiguration::default(),
            map,
            next_regenerate_tick: 0,
            hurt_next_tick: BTreeMap::new(),
            hurt_active: std::collections::BTreeSet::new(),
            hurt_applied: std::collections::BTreeSet::new(),
            respawn_touch_count: 0,
            jump: None,
        }
    }

    pub fn set_movement_modifiers(&mut self, modifiers: MovementModifiers) {
        self.movement_modifiers = modifiers;
    }

    pub fn set_player_restrictions(&mut self, restrictions: PlayerRestrictions) {
        self.restrictions = restrictions;
    }

    pub fn set_auto_reload(&mut self, enabled: bool) {
        self.auto_reload = enabled;
    }

    pub fn lifecycle(&self) -> PlayerLifecycle {
        self.lifecycle
    }

    pub fn condition_set(&self) -> ConditionSet {
        self.conditions
    }

    pub fn respawn_touch_count(&self) -> u32 {
        self.respawn_touch_count
    }

    pub fn weapon_runtime(&self, weapon: Weapon) -> Option<WeaponRuntime> {
        self.loadout.get(&weapon).copied()
    }

    pub fn requires_sticky_launch_random(&self, command: Command) -> bool {
        let Some(weapon) = self.loadout.get(&self.weapon) else {
            return false;
        };
        if self.weapon != Weapon::StickybombLauncher || weapon.clip == 0 {
            return false;
        }
        let Some(begin) = weapon.charge_begin_tick else {
            return false;
        };
        let charge =
            self.tick.saturating_sub(begin) as f32 * self.movement_configuration.tick_interval;
        (!command.fire && self.fire_was_held) || charge >= 4.0
    }

    pub fn physics_requests(&self) -> &[ProjectilePhysicsRequest] {
        &self.physics_requests
    }

    pub fn activity_events(&self) -> &[ActivityEvent] {
        &self.activity_events
    }

    pub fn mover_requests(&self) -> &[MoverRequest] {
        &self.mover_requests
    }

    pub fn lifecycle_events(&self) -> &[LifecycleEvent] {
        &self.lifecycle_events
    }

    pub fn regenerate_animation_events(&self) -> &[RegenerateAnimationEvent] {
        &self.regenerate_animation_events
    }

    pub fn map_effects(&self) -> &[MapEffect] {
        &self.map_effects
    }

    pub fn producer_snapshot(&self) -> ProducerSnapshot {
        ProducerSnapshot {
            tick: self.tick,
            lifecycle: self.lifecycle,
            class: self.class,
            team: self.team,
            active_weapon: self.weapon,
            health: self.health,
            maximum_health: self.maximum_health(),
            conditions: self.conditions.words(),
            weapons: self.loadout.values().copied().collect(),
            projectiles: self
                .projectiles
                .iter()
                .map(|projectile| projectile.presentation.clone())
                .collect(),
            activities: self.activity_events.clone(),
            lifecycle_events: self.lifecycle_events.clone(),
            physics_requests: self.physics_requests.clone(),
            rocket_trace_requests: self.rocket_trace_requests.clone(),
            radius_damage_requests: self.radius_damage_requests.clone(),
            mover_requests: self.mover_requests.clone(),
            contact_reconcile_requests: self.contact_reconcile_requests.clone(),
            map_effects: self.map_effects.clone(),
            regenerate_animation_events: self.regenerate_animation_events.clone(),
        }
    }

    pub fn radius_damage_requests(&self) -> &[RadiusDamageRequest] {
        &self.radius_damage_requests
    }

    pub fn rocket_trace_requests(&self) -> &[RocketTraceRequest] {
        &self.rocket_trace_requests
    }

    pub fn contact_reconcile_requests(&self) -> &[ContactReconcileRequest] {
        &self.contact_reconcile_requests
    }

    pub fn apply_mover_results(&mut self, results: &[MoverResult]) -> Result<MapPhase, Error> {
        let mut candidate = self.clone();
        let phase = candidate.map.apply_mover_results(candidate.tick, results)?;
        candidate.movement.position = add(candidate.movement.position, phase.carry);
        candidate.mover_requests = phase.mover_requests.clone();
        *self = candidate;
        Ok(phase)
    }

    pub fn map_input(
        &mut self,
        source: u32,
        input: &[u8],
        value: playsrc_entity::Variant,
    ) -> Result<MapPhase, Error> {
        let mut candidate = self.clone();
        let phase = candidate.map.input(candidate.tick, source, input, value)?;
        *self = candidate;
        Ok(phase)
    }

    pub fn configure_jump(&mut self, definition: jump::CourseDefinition) -> Result<(), Error> {
        for zone in &definition.zones {
            if !self.map.accepts_course_trigger(zone.trigger_entity) {
                return Err(Error::InvalidCourseTrigger(zone.trigger_entity));
            }
        }
        self.jump = Some(jump::Session::new(definition, jump::Limits::default()));
        self.health = self.maximum_health();
        Ok(())
    }

    pub fn movement_state(&self) -> MovementState {
        self.movement
    }

    pub fn movement_snapshot_bytes(&self) -> Vec<u8> {
        self.movement.snapshot_bytes()
    }

    pub fn last_movement_result(&self) -> Option<&MovementStepResult> {
        self.last_movement.as_ref()
    }

    pub fn map_counts(&self) -> MapCounts {
        self.map.counts()
    }

    pub fn advance(&mut self, command: Command) -> Result<Snapshot, Error> {
        self.advance_with_external(command, &[], &[], None)
    }

    pub fn advance_with_external(
        &mut self,
        command: Command,
        physics_results: &[ProjectilePhysicsResult],
        rocket_results: &[RocketTraceResult],
        sticky_random: Option<StickyLaunchRandom>,
    ) -> Result<Snapshot, Error> {
        let mut candidate = self.clone();
        let snapshot =
            candidate.advance_inner(command, physics_results, rocket_results, sticky_random)?;
        *self = candidate;
        Ok(snapshot)
    }

    fn advance_inner(
        &mut self,
        command: Command,
        physics_results: &[ProjectilePhysicsResult],
        rocket_results: &[RocketTraceResult],
        sticky_random: Option<StickyLaunchRandom>,
    ) -> Result<Snapshot, Error> {
        self.physics_requests.clear();
        self.activity_events.clear();
        self.mover_requests.clear();
        self.lifecycle_events.clear();
        self.regenerate_animation_events.clear();
        self.map_effects.clear();
        self.radius_damage_requests.clear();
        self.rocket_trace_requests.clear();
        self.contact_reconcile_requests.clear();
        let mut events = Vec::new();
        let mut projectile_events = Vec::new();
        self.apply_projectile_physics(physics_results, &mut projectile_events)?;
        self.apply_rocket_traces(rocket_results, &mut projectile_events, &mut events)?;
        self.apply_selection(command, &mut events, &mut projectile_events);
        let movement_policy = MovementPolicy {
            class: self.class,
            modifiers: self.movement_modifiers,
        }
        .resolve();
        let hull = self.movement.active_hull(movement_policy);
        let mut map_phase = self.map.begin_tick(
            &self.collision,
            BeginTickInput {
                tick: self.tick,
                tick_interval: self.movement_configuration.tick_interval,
                activate_entity: command.activate_entity,
                player_position: self.movement.position,
                player_hull: hull,
                grounded: self.movement.ground.is_some(),
            },
        )?;
        self.movement.position = add(self.movement.position, map_phase.carry);

        let movement_result = step(
            &self.collision,
            self.movement,
            StepInput {
                command_number: u32::try_from(self.tick).unwrap_or(u32::MAX),
                command: MoveCommand {
                    forward: command.movement.forward,
                    side: -command.movement.side,
                    yaw_degrees: command.movement.yaw_degrees,
                    jump: command.movement.jump,
                    crouch: command.movement.crouch,
                },
                pitch_degrees: command.pitch_degrees,
                up: command.up,
                speed_button: command.speed_button,
                mode_request: command.mode_request.map(|mode| ModeRequest {
                    mode,
                    disposition: TransitionDisposition::RESET_ENVIRONMENT,
                }),
            },
            self.movement_configuration,
            movement_policy,
        )?;
        self.movement = movement_result.state;
        self.last_movement = Some(movement_result);

        let mut teleported = false;
        let phase = self.map.contact_phase(
            &self.collision,
            self.tick,
            self.movement.position,
            self.movement.active_hull(movement_policy),
            map_runtime::PlayerContactFacts {
                team: self.team.source_number(),
                class: self.class.source_number(),
                observer: self.lifecycle != PlayerLifecycle::Active,
                conditions: self.conditions.words(),
                winning_team: self.restrictions.team_win.map(Team::source_number),
            },
        )?;
        let discontinuity = self.apply_map_effects(&phase, &mut events, &mut teleported);
        let jump_contacts = phase.contacts.clone();
        map_phase.append(phase);
        if discontinuity {
            self.contact_reconcile_requests
                .push(ContactReconcileRequest {
                    tick: self.tick,
                    position: self.movement.position,
                    hull: self.movement.active_hull(movement_policy),
                });
        }

        let released_primary = !command.fire && self.fire_was_held;
        let mut ammo_events = Vec::new();
        let primary = {
            let state = self
                .loadout
                .get_mut(&self.weapon)
                .expect("active weapon belongs to loadout");
            state.primary(
                self.tick,
                self.movement_configuration.tick_interval,
                command.fire,
                released_primary,
                &mut self.activity_events,
            )
        };
        if let PrimaryResult::Fired { charge_seconds } = primary {
            self.fire_projectile(
                command.pitch_degrees,
                command.movement.yaw_degrees,
                charge_seconds,
                sticky_random,
                &mut projectile_events,
            )?;
        }
        {
            let state = self
                .loadout
                .get_mut(&self.weapon)
                .expect("active weapon belongs to loadout");
            if command.reload || (self.auto_reload && !command.fire && !command.detonate) {
                state.start_reload(
                    self.tick,
                    self.movement_configuration.tick_interval,
                    &mut self.activity_events,
                );
            }
            state.advance_reload(
                self.tick,
                self.movement_configuration.tick_interval,
                &mut self.activity_events,
                &mut ammo_events,
            );
        }
        for event in ammo_events {
            events.push(Event::Reloaded {
                weapon: event.weapon,
                clip: event.clip,
                reserve: event.reserve,
            });
        }
        self.fire_was_held = command.fire;
        self.advance_projectiles(
            self.movement_configuration.tick_interval,
            &mut projectile_events,
            &mut events,
        )?;
        if command.detonate {
            self.detonate(&mut projectile_events, &mut events);
        }

        if self.health <= 0 && self.lifecycle == PlayerLifecycle::Active {
            self.die(&mut projectile_events);
        }
        let mut respawned = false;
        if command.respawn {
            self.respawn(&mut projectile_events, &mut events, movement_policy);
            respawned = true;
        }

        let jump_output = if let Some(jump) = &mut self.jump {
            let contacts: Vec<_> = jump_contacts
                .into_iter()
                .filter(|contact| {
                    jump.definition()
                        .zone_for_trigger(contact.trigger_entity)
                        .is_some()
                })
                .map(|contact| jump::Contact {
                    sequence: contact.sequence,
                    trigger_entity: contact.trigger_entity,
                    kind: match contact.kind {
                        playsrc_entity::ContactKind::Enter => jump::ContactKind::Enter,
                        playsrc_entity::ContactKind::Stay => jump::ContactKind::Stay,
                        playsrc_entity::ContactKind::Exit => jump::ContactKind::Exit,
                    },
                })
                .collect();
            let output = jump.advance(jump::TickInput {
                tick: self.tick,
                tick_interval: self.movement_configuration.tick_interval,
                player: jump::PlayerFacts {
                    identity: PLAYER_IDENTITY,
                    class: match self.class {
                        Class::Soldier => jump::Class::Soldier,
                        Class::Demoman => jump::Class::Demoman,
                    },
                    team: match self.team {
                        Team::Red => jump::Team::Red,
                        Team::Blue => jump::Team::Blue,
                    },
                    alive: self.lifecycle == PlayerLifecycle::Active,
                    active: self.lifecycle == PlayerLifecycle::Active,
                    noclip: self.movement.mode == Mode::Noclip,
                    respawned,
                    teleported,
                },
                contacts: &contacts,
                command: jump::Command {
                    reset: command.reset,
                },
            })?;
            for request in &output.requests {
                match request {
                    jump::Request::FizzleOwnedProjectiles { player_identity }
                        if *player_identity == PLAYER_IDENTITY =>
                    {
                        self.fizzle_projectiles(&mut projectile_events)
                    }
                    jump::Request::Respawn { player_identity }
                        if *player_identity == PLAYER_IDENTITY =>
                    {
                        self.respawn(&mut projectile_events, &mut events, movement_policy)
                    }
                    _ => {}
                }
            }
            Some(output)
        } else {
            None
        };

        self.tick += 1;
        self.mover_requests = map_phase.mover_requests.clone();
        self.map_effects = map_phase.effects.clone();
        Ok(Snapshot {
            tick: self.tick,
            class: self.class,
            team: self.team,
            weapon: self.weapon,
            movement: self.movement,
            health: self.health as f32,
            maximum_health: self.maximum_health() as f32,
            loadout: self
                .loadout
                .values()
                .copied()
                .map(WeaponState::from_runtime)
                .collect(),
            conditions: self.conditions.first_word(),
            projectiles: self
                .projectiles
                .iter()
                .map(|projectile| projectile.presentation.clone())
                .collect(),
            projectile_events,
            entity_transforms: self.map.transforms(),
            entity_events: map_phase.events,
            jump: jump_output,
            events,
        })
    }

    fn apply_selection(
        &mut self,
        command: Command,
        events: &mut Vec<Event>,
        projectile_events: &mut Vec<ProjectileEvent>,
    ) {
        if let Some(team) = command.select_team
            && team != self.team
        {
            self.team = team;
            self.fizzle_projectiles(projectile_events);
            self.lifecycle_events.push(LifecycleEvent {
                tick: self.tick,
                kind: LifecycleEventKind::TeamChanged,
                class: self.class,
                team: self.team,
            });
            events.push(Event::TeamChanged(team));
        }
        if let Some(class) = command.select_class
            && class != self.class
        {
            self.class = class;
            self.weapon = default_weapon(class);
            self.loadout = default_loadout(class);
            self.health = self.maximum_health();
            self.conditions.clear();
            self.lifecycle = PlayerLifecycle::Active;
            self.fizzle_projectiles(projectile_events);
            self.movement = MovementState::from_player(
                Player {
                    position: self.spawn,
                    velocity: [0.0; 3],
                    grounded: false,
                    crouched: false,
                    jump_latched: false,
                },
                MovementPolicy {
                    class,
                    modifiers: self.movement_modifiers,
                }
                .resolve(),
            );
            self.last_movement = None;
            self.lifecycle_events.extend([
                LifecycleEvent {
                    tick: self.tick,
                    kind: LifecycleEventKind::ClassChanged,
                    class: self.class,
                    team: self.team,
                },
                LifecycleEvent {
                    tick: self.tick,
                    kind: LifecycleEventKind::Respawned,
                    class: self.class,
                    team: self.team,
                },
            ]);
            self.activity_events.push(ActivityEvent {
                tick: self.tick,
                weapon: self.weapon,
                activity: weapon::WeaponActivity::Draw,
            });
            events.push(Event::ClassChanged(class));
            events.push(Event::WeaponChanged(self.weapon));
            events.push(Event::Respawned);
        }
        if let Some(weapon) = command.select_weapon
            && allowed(self.class, weapon)
            && weapon != self.weapon
        {
            if let Some(previous) = self.loadout.get_mut(&self.weapon) {
                previous.charge_begin_tick = None;
                previous.abort_reload();
            }
            self.weapon = weapon;
            self.loadout
                .entry(weapon)
                .or_insert_with(|| WeaponRuntime::full(weapon));
            self.activity_events.push(ActivityEvent {
                tick: self.tick,
                weapon,
                activity: weapon::WeaponActivity::Draw,
            });
            events.push(Event::WeaponChanged(weapon));
        }
    }

    fn apply_map_effects(
        &mut self,
        phase: &MapPhase,
        events: &mut Vec<Event>,
        teleported: &mut bool,
    ) -> bool {
        let mut discontinuity = false;
        for effect in &phase.effects {
            match *effect {
                MapEffect::Teleport {
                    trigger,
                    destination,
                    position,
                    angles,
                } if !discontinuity => {
                    self.movement.position = position;
                    self.movement.ground = None;
                    *teleported = true;
                    discontinuity = true;
                    events.push(Event::Teleported {
                        trigger,
                        destination,
                        position,
                        yaw_degrees: angles.map(|value| value[1]),
                    });
                }
                MapEffect::Hurt {
                    trigger,
                    damage_per_second,
                    contact,
                } => match contact {
                    playsrc_entity::ContactKind::Enter => {
                        self.hurt_active.insert(trigger);
                        self.hurt_applied.remove(&trigger);
                        self.apply_hurt_pulse(trigger, damage_per_second, events);
                    }
                    playsrc_entity::ContactKind::Stay => {
                        let due = self.hurt_next_tick.get(&trigger).copied().unwrap_or(0);
                        if self.tick >= due {
                            self.apply_hurt_pulse(trigger, damage_per_second, events);
                        }
                    }
                    playsrc_entity::ContactKind::Exit => {
                        if self.hurt_active.remove(&trigger) && !self.hurt_applied.remove(&trigger)
                        {
                            self.apply_hurt_pulse(trigger, damage_per_second, events);
                        }
                        self.hurt_next_tick.remove(&trigger);
                    }
                },
                MapEffect::Push {
                    trigger,
                    velocity,
                    replace,
                } => {
                    self.movement.velocity = if replace {
                        velocity
                    } else {
                        add(self.movement.velocity, velocity)
                    };
                    self.movement.ground = None;
                    events.push(Event::TriggerVelocity {
                        trigger,
                        velocity: self.movement.velocity,
                    });
                }
                MapEffect::Regenerate {
                    entity,
                    team,
                    associated_model,
                } if self.tick >= self.next_regenerate_tick
                    && self.lifecycle == PlayerLifecycle::Active
                    && !self.restrictions.taunting
                    && !self.restrictions.stalemate
                    && match self.restrictions.team_win {
                        Some(winner) => winner == self.team,
                        None => team.is_none_or(|team| team == self.team.source_number()),
                    } =>
                {
                    self.regenerate(entity, associated_model, events);
                }
                MapEffect::RespawnRoom { contact, .. } => match contact {
                    playsrc_entity::ContactKind::Enter => {
                        self.respawn_touch_count = self.respawn_touch_count.saturating_add(1)
                    }
                    playsrc_entity::ContactKind::Exit => {
                        self.respawn_touch_count = self.respawn_touch_count.saturating_sub(1)
                    }
                    playsrc_entity::ContactKind::Stay => {}
                },
                _ => {}
            }
        }
        discontinuity
    }

    fn apply_hurt_pulse(&mut self, trigger: u32, damage_per_second: f32, events: &mut Vec<Event>) {
        let amount = damage_per_second * 0.5;
        if amount < 0.0 {
            let before = self.health;
            self.health = self
                .health
                .saturating_add((-amount) as i32)
                .min(self.maximum_health());
            events.push(Event::Healed {
                amount: (self.health - before) as f32,
                health: self.health as f32,
                trigger,
            });
        } else if amount > 0.0 {
            let points = (amount + 0.5) as i32;
            self.health = self.health.saturating_sub(points).max(0);
            events.push(Event::Damaged {
                amount: points as f32,
                health: self.health as f32,
            });
        }
        self.hurt_applied.insert(trigger);
        self.hurt_next_tick.insert(
            trigger,
            self.tick + ticks(0.5, self.movement_configuration.tick_interval),
        );
    }

    fn regenerate(&mut self, entity: u32, associated_model: Option<u32>, events: &mut Vec<Event>) {
        let maximum = self.maximum_health();
        self.health = if self.health > maximum {
            self.health.max(maximum)
        } else {
            maximum
        };
        let remove_melee_lock = self.conditions.contains(Condition::EnergyBuff);
        for condition in resupply_removed_conditions() {
            if !matches!(
                condition,
                Condition::EnergyBuff | Condition::CannotSwitchFromMelee
            ) {
                self.conditions.remove(condition);
            }
        }
        if remove_melee_lock {
            self.conditions.remove(Condition::EnergyBuff);
            self.conditions.remove(Condition::CannotSwitchFromMelee);
        }
        for weapon in self.loadout.values_mut() {
            weapon.regenerate(self.tick, self.movement_configuration.tick_interval);
        }
        let active = self.loadout[&self.weapon];
        self.next_regenerate_tick =
            self.tick + ticks(3.0, self.movement_configuration.tick_interval);
        if let Some(associated_model) = associated_model {
            self.regenerate_animation_events
                .push(RegenerateAnimationEvent {
                    zone: entity,
                    associated_model,
                    open_tick: self.tick,
                    close_tick: self.tick + ticks(2.0, self.movement_configuration.tick_interval),
                });
        }
        events.push(Event::Resupplied {
            entity,
            health: self.health as f32,
            weapon: self.weapon,
            clip: active.clip,
            reserve: active.reserve,
        });
    }

    fn fire_projectile(
        &mut self,
        pitch: f32,
        yaw: f32,
        charge_seconds: f32,
        sticky_random: Option<StickyLaunchRandom>,
        projectile_events: &mut Vec<ProjectileEvent>,
    ) -> Result<(), Error> {
        if self.projectiles.len() >= MAX_PROJECTILES {
            return Err(Error::ProjectileLimit);
        }
        let kind = match self.weapon {
            Weapon::RocketLauncher | Weapon::Original => ProjectileKind::Rocket,
            Weapon::StickybombLauncher => ProjectileKind::Sticky,
        };
        let mut direction = direction(pitch, yaw);
        let (right, up) = aim_basis(pitch, yaw);
        let eye = add(self.movement.position, self.movement.view_offset);
        let (position, orientation) = if kind == ProjectileKind::Sticky {
            let position = add(
                add(add(eye, scale(direction, 16.0)), scale(right, 8.0)),
                scale(up, -6.0),
            );
            let muzzle = self.collision.trace(
                eye,
                position,
                Hull {
                    mins: [-8.0; 3],
                    maxs: [8.0; 3],
                },
                self.movement_configuration.solid_mask,
            )?;
            if muzzle.start_solid {
                return Ok(());
            }
            (muzzle.end, quaternion_from_angles(pitch, yaw, 0.0))
        } else {
            let forward_end = add(eye, scale(direction, 2000.0));
            let aim = self.collision.trace(
                eye,
                forward_end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                self.movement_configuration.solid_mask,
            )?;
            let mut offset = [23.5, 12.0, -3.0];
            if self.weapon == Weapon::Original {
                offset[1] = 0.0;
            }
            if self.movement.crouch.uses_crouched_hull() {
                offset[2] = 8.0;
            }
            let source = add(
                add(
                    add(eye, scale(direction, offset[0])),
                    scale(right, offset[1]),
                ),
                scale(up, offset[2]),
            );
            direction = normalized(if aim.fraction > 0.1 {
                sub(aim.end, source)
            } else {
                sub(forward_end, source)
            });
            let clipped = self.collision.trace(
                eye,
                source,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                self.movement_configuration.solid_mask,
            )?;
            (clipped.end, quaternion_from_direction(direction))
        };
        let (velocity, angular_velocity) = if kind == ProjectileKind::Sticky {
            let random = sticky_random.ok_or(Error::MissingStickyLaunchRandom)?;
            if !random.validate() {
                return Err(Error::InvalidStickyLaunchRandom);
            }
            let speed = remap_clamped(charge_seconds, 0.0, 4.0, 900.0, 2400.0);
            (
                add(
                    add(
                        add(scale(direction, speed), scale(up, 200.0)),
                        scale(right, random.right_velocity),
                    ),
                    scale(up, random.up_velocity),
                ),
                [600.0, random.angular_y as f32, 0.0],
            )
        } else {
            (scale(direction, 1_100.0), [0.0; 3])
        };
        let identity = self.next_projectile;
        self.next_projectile = self.next_projectile.wrapping_add(1).max(1);
        let tick_interval = self.movement_configuration.tick_interval;
        let prearm_tick = self.tick.saturating_add(ticks(0.001, tick_interval));
        let projectile = LiveProjectile {
            presentation: Projectile {
                identity,
                kind,
                team: self.team,
                owner_identity: PLAYER_IDENTITY,
                launcher_identity: self.weapon as u32,
                state: ProjectileState::Flying,
                position,
                velocity,
                orientation,
                angular_velocity,
                contact_normal: None,
                age_seconds: 0.0,
            },
            armed: false,
            creation_tick: self.tick,
            arm_tick: prearm_tick.saturating_add(ticks(0.8, tick_interval)),
            next_think_tick: self.tick.saturating_add(ticks(0.2, tick_interval)),
            forced_detonate_tick: None,
            motion_enabled: true,
            direct_target: None,
        };
        projectile_events.push(projectile_event(
            ProjectileEventKind::Fire,
            &projectile.presentation,
            self.tick,
        ));
        if kind == ProjectileKind::Sticky {
            self.physics_requests.push(sticky_physics_request(
                ProjectilePhysicsOperation::Create,
                &projectile,
                self.tick,
            ));
            let sticky_count = self
                .projectiles
                .iter()
                .filter(|value| value.presentation.kind == ProjectileKind::Sticky)
                .count();
            if sticky_count >= 8
                && let Some(oldest) = self
                    .projectiles
                    .iter_mut()
                    .find(|value| value.presentation.kind == ProjectileKind::Sticky)
            {
                oldest.forced_detonate_tick = Some(self.tick);
            }
        }
        self.projectiles.push(projectile);
        Ok(())
    }

    fn apply_projectile_physics(
        &mut self,
        results: &[ProjectilePhysicsResult],
        projectile_events: &mut Vec<ProjectileEvent>,
    ) -> Result<(), Error> {
        let mut seen = std::collections::BTreeSet::new();
        for result in results {
            if result.tick != self.tick
                || !seen.insert(result.projectile)
                || result
                    .position
                    .into_iter()
                    .chain(result.velocity)
                    .chain(result.orientation)
                    .chain(result.angular_velocity)
                    .any(|value| !value.is_finite())
                || result.contact.is_some_and(|contact| {
                    contact.normal.into_iter().any(|value| !value.is_finite())
                })
                || (quaternion_length(result.orientation) - 1.0).abs() > 0.001
            {
                return Err(Error::InvalidProjectilePhysics);
            }
            let Some(index) = self.projectiles.iter().position(|projectile| {
                projectile.presentation.identity == result.projectile
                    && projectile.presentation.kind == ProjectileKind::Sticky
                    && projectile.presentation.state == ProjectileState::Flying
            }) else {
                return Err(Error::InvalidProjectilePhysics);
            };
            let mut disable = false;
            let detonatable =
                self.tick
                    .saturating_sub(self.projectiles[index].creation_tick) as f32
                    * self.movement_configuration.tick_interval
                    >= 0.8;
            {
                let projectile = &mut self.projectiles[index];
                projectile.presentation.position = result.position;
                projectile.presentation.velocity = result.velocity;
                projectile.presentation.orientation = result.orientation;
                projectile.presentation.angular_velocity = result.angular_velocity;
                projectile.motion_enabled = result.motion_enabled;
                if let Some(contact) = result.contact {
                    let normal = normalized(contact.normal);
                    projectile.presentation.contact_normal = Some(normal);
                    projectile_events.push(projectile_event(
                        ProjectileEventKind::Impact,
                        &projectile.presentation,
                        self.tick,
                    ));
                    if matches!(
                        contact.kind,
                        ProjectileContactKind::World | ProjectileContactKind::DynamicProp
                    ) {
                        projectile.motion_enabled = false;
                        projectile.presentation.state = if projectile.armed || detonatable {
                            ProjectileState::StuckArmed
                        } else {
                            ProjectileState::StuckUnarmed
                        };
                        projectile_events.push(projectile_event(
                            ProjectileEventKind::Stick,
                            &projectile.presentation,
                            self.tick,
                        ));
                        disable = true;
                    }
                }
            }
            if disable {
                self.physics_requests.push(sticky_physics_request(
                    ProjectilePhysicsOperation::DisableMotion,
                    &self.projectiles[index],
                    self.tick,
                ));
            }
        }
        Ok(())
    }

    fn apply_rocket_traces(
        &mut self,
        results: &[RocketTraceResult],
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let mut seen = std::collections::BTreeSet::new();
        for result in results {
            if result.tick != self.tick
                || !seen.insert(result.projectile)
                || result.end.into_iter().any(|value| !value.is_finite())
                || result
                    .normal
                    .is_some_and(|normal| normal.into_iter().any(|value| !value.is_finite()))
                || result.sky && !result.solid
                || result.solid && !result.sky && result.normal.is_none()
                || !result.solid && result.direct_target.is_some()
            {
                return Err(Error::InvalidProjectilePhysics);
            }
            let Some(index) = self.projectiles.iter().position(|projectile| {
                projectile.presentation.identity == result.projectile
                    && projectile.presentation.kind == ProjectileKind::Rocket
                    && projectile.presentation.state == ProjectileState::Flying
            }) else {
                return Err(Error::InvalidProjectilePhysics);
            };
            let mut projectile = self.projectiles.remove(index);
            projectile.presentation.position = result.end;
            projectile.direct_target = result.direct_target;
            if result.sky {
                continue;
            }
            if result.solid {
                let normal = normalized(result.normal.expect("validated solid normal"));
                projectile.presentation.position = add(result.end, scale(normal, 1.0));
                projectile.presentation.contact_normal = Some(normal);
                projectile_events.push(projectile_event(
                    ProjectileEventKind::Impact,
                    &projectile.presentation,
                    self.tick,
                ));
                self.explode(projectile, projectile_events, events);
            } else {
                self.projectiles.push(projectile);
            }
        }
        Ok(())
    }

    fn advance_projectiles(
        &mut self,
        tick_interval: f32,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let mut retained = Vec::new();
        for mut projectile in std::mem::take(&mut self.projectiles) {
            projectile.presentation.age_seconds =
                self.tick.saturating_sub(projectile.creation_tick) as f32 * tick_interval;
            if projectile.presentation.kind == ProjectileKind::Sticky
                && !projectile.armed
                && self.tick >= projectile.arm_tick
            {
                projectile.armed = true;
                if projectile.presentation.state == ProjectileState::StuckUnarmed {
                    projectile.presentation.state = ProjectileState::StuckArmed;
                }
                projectile_events.push(projectile_event(
                    ProjectileEventKind::Arm,
                    &projectile.presentation,
                    self.tick,
                ));
            }
            if projectile.presentation.kind == ProjectileKind::Sticky {
                let think_due = self.tick >= projectile.next_think_tick;
                if think_due {
                    projectile.next_think_tick = self
                        .tick
                        .saturating_add(ticks(0.2, self.movement_configuration.tick_interval));
                }
                if think_due
                    && projectile
                        .forced_detonate_tick
                        .is_some_and(|detonate| self.tick > detonate)
                {
                    self.explode(projectile, projectile_events, events);
                    continue;
                }
                if projectile.presentation.state == ProjectileState::Flying
                    && projectile.motion_enabled
                    && projectile.creation_tick < self.tick
                {
                    self.physics_requests.push(sticky_physics_request(
                        ProjectilePhysicsOperation::Step,
                        &projectile,
                        self.tick,
                    ));
                }
                retained.push(projectile);
                continue;
            }
            if projectile.presentation.state != ProjectileState::Flying {
                retained.push(projectile);
                continue;
            }
            let end = add(
                projectile.presentation.position,
                scale(projectile.presentation.velocity, tick_interval),
            );
            self.rocket_trace_requests.push(RocketTraceRequest {
                projectile: projectile.presentation.identity,
                tick: self.tick,
                start: projectile.presentation.position,
                end,
                mask: self.movement_configuration.solid_mask,
            });
            retained.push(projectile);
        }
        self.projectiles = retained;
        Ok(())
    }

    fn detonate(&mut self, projectile_events: &mut Vec<ProjectileEvent>, events: &mut Vec<Event>) {
        let mut retained = Vec::new();
        for projectile in std::mem::take(&mut self.projectiles) {
            let detonatable = self.tick.saturating_sub(projectile.creation_tick) as f32
                * self.movement_configuration.tick_interval
                >= 0.8;
            if projectile.presentation.kind == ProjectileKind::Sticky && detonatable {
                self.explode(projectile, projectile_events, events);
            } else {
                retained.push(projectile);
            }
        }
        self.projectiles = retained;
    }

    fn explode(
        &mut self,
        projectile: LiveProjectile,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) {
        let (base_damage, radius, self_radius) = match projectile.presentation.kind {
            ProjectileKind::Rocket => (90.0, 146.0, 121.0),
            ProjectileKind::Sticky => (120.0, 146.0, 146.0),
        };
        self.radius_damage_requests.push(RadiusDamageRequest {
            projectile: projectile.presentation.identity,
            kind: projectile.presentation.kind,
            source: projectile.presentation.position,
            base_damage,
            radius,
            self_radius,
            direct_target: projectile.direct_target,
        });
        if projectile.presentation.kind == ProjectileKind::Sticky {
            self.physics_requests.push(sticky_physics_request(
                ProjectilePhysicsOperation::Destroy,
                &projectile,
                self.tick,
            ));
        }
        projectile_events.push(projectile_event(
            ProjectileEventKind::Explode,
            &projectile.presentation,
            self.tick,
        ));
        let policy = MovementPolicy {
            class: self.class,
            modifiers: self.movement_modifiers,
        }
        .resolve();
        let hull = self.movement.active_hull(policy);
        let size = sub(hull.maxs, hull.mins);
        let center = add(
            self.movement.position,
            scale(add(hull.mins, hull.maxs), 0.5),
        );
        let visible = self
            .collision
            .trace(
                projectile.presentation.position,
                center,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                self.movement_configuration.solid_mask,
            )
            .is_ok_and(|trace| trace.fraction == 1.0 || trace.start_solid);
        let kind = if projectile.presentation.kind == ProjectileKind::Rocket {
            combat::BlastKind::Rocket
        } else {
            combat::BlastKind::Sticky
        };
        let grounded = self.movement.ground.is_some();
        let class = match self.class {
            Class::Soldier => combat::BlastClass::Soldier,
            Class::Demoman => combat::BlastClass::Demoman,
        };
        if let Some(base_damage) = combat::player_blast_damage(
            kind,
            projectile.presentation.position,
            combat::PlayerBlastTarget {
                origin: self.movement.position,
                world_center: center,
                direct_hit: false,
                visible,
                self_damage: true,
            },
        ) {
            let damage = combat::apply_self_damage_rules(
                base_damage,
                class,
                grounded,
                self.movement.water_level > 0,
            );
            let health_before = self.health;
            self.health = self.health.saturating_sub(damage.health_points).max(0);
            events.push(Event::Damaged {
                amount: damage.health_points as f32,
                health: self.health as f32,
            });
            let impulse = combat::self_blast_impulse(
                class,
                grounded,
                self.movement.crouch.uses_crouched_hull(),
                size,
                center,
                projectile.presentation.position,
                damage.damage_for_force,
            );
            if class != combat::BlastClass::Soldier
                || grounded
                || health_before as f32 - damage.damage > 0.0
            {
                self.conditions.insert(Condition::BlastJumping);
            }
            self.movement.velocity = add(self.movement.velocity, impulse.impulse);
            events.push(Event::BlastImpulse {
                velocity: self.movement.velocity,
            });
        }
    }

    fn fizzle_projectiles(&mut self, events: &mut Vec<ProjectileEvent>) {
        for projectile in std::mem::take(&mut self.projectiles) {
            if projectile.presentation.kind == ProjectileKind::Sticky {
                self.physics_requests.push(sticky_physics_request(
                    ProjectilePhysicsOperation::Destroy,
                    &projectile,
                    self.tick,
                ));
            }
            events.push(projectile_event(
                ProjectileEventKind::Fizzle,
                &projectile.presentation,
                self.tick,
            ));
        }
    }

    fn die(&mut self, projectile_events: &mut Vec<ProjectileEvent>) {
        self.lifecycle = PlayerLifecycle::Dying;
        self.lifecycle_events.push(LifecycleEvent {
            tick: self.tick,
            kind: LifecycleEventKind::Died,
            class: self.class,
            team: self.team,
        });
        self.fizzle_projectiles(projectile_events);
        self.fire_was_held = false;
        for weapon in self.loadout.values_mut() {
            weapon.abort_reload();
            weapon.charge_begin_tick = None;
        }
    }

    fn respawn(
        &mut self,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
        movement_policy: GenericMovementPolicy,
    ) {
        self.fizzle_projectiles(projectile_events);
        self.health = self.maximum_health();
        self.conditions.clear();
        self.lifecycle = PlayerLifecycle::Active;
        self.lifecycle_events.push(LifecycleEvent {
            tick: self.tick,
            kind: LifecycleEventKind::Respawned,
            class: self.class,
            team: self.team,
        });
        for weapon in self.loadout.values_mut() {
            weapon.reset_for_spawn(self.tick);
        }
        self.movement = MovementState::from_player(
            Player {
                position: self.spawn,
                velocity: [0.0; 3],
                grounded: false,
                crouched: false,
                jump_latched: false,
            },
            movement_policy,
        );
        self.last_movement = None;
        self.fire_was_held = false;
        self.activity_events.push(ActivityEvent {
            tick: self.tick,
            weapon: self.weapon,
            activity: weapon::WeaponActivity::Draw,
        });
        events.push(Event::Respawned);
    }

    fn maximum_health(&self) -> i32 {
        if self.jump.is_some() && self.class == Class::Soldier {
            900
        } else {
            stock_maximum_health(self.class)
        }
    }
}

fn default_weapon(class: Class) -> Weapon {
    match class {
        Class::Soldier => Weapon::RocketLauncher,
        Class::Demoman => Weapon::StickybombLauncher,
    }
}

fn default_loadout(class: Class) -> BTreeMap<Weapon, WeaponRuntime> {
    match class {
        Class::Soldier => BTreeMap::from([
            (
                Weapon::RocketLauncher,
                WeaponRuntime::full(Weapon::RocketLauncher),
            ),
            (Weapon::Original, WeaponRuntime::full(Weapon::Original)),
        ]),
        Class::Demoman => BTreeMap::from([(
            Weapon::StickybombLauncher,
            WeaponRuntime::full(Weapon::StickybombLauncher),
        )]),
    }
}

fn stock_maximum_health(class: Class) -> i32 {
    match class {
        Class::Soldier => 200,
        Class::Demoman => 175,
    }
}

fn allowed(class: Class, weapon: Weapon) -> bool {
    matches!(
        (class, weapon),
        (Class::Soldier, Weapon::RocketLauncher | Weapon::Original)
            | (Class::Demoman, Weapon::StickybombLauncher)
    )
}

fn resupply_removed_conditions() -> [Condition; 10] {
    [
        Condition::Burning,
        Condition::Urine,
        Condition::MadMilk,
        Condition::Gas,
        Condition::Bleeding,
        Condition::EnergyBuff,
        Condition::CannotSwitchFromMelee,
        Condition::Phase,
        Condition::ParachuteActive,
        Condition::Plague,
    ]
}

fn projectile_event(
    kind: ProjectileEventKind,
    projectile: &Projectile,
    tick: u64,
) -> ProjectileEvent {
    ProjectileEvent {
        kind,
        projectile: projectile.identity,
        projectile_kind: projectile.kind,
        owner_identity: projectile.owner_identity,
        launcher_identity: projectile.launcher_identity,
        team: projectile.team,
        tick,
        position: projectile.position,
        orientation: projectile.orientation,
        contact_normal: projectile.contact_normal,
    }
}

fn ticks(seconds: f32, tick: f32) -> u64 {
    (seconds / tick).ceil() as u64
}

fn direction(pitch: f32, yaw: f32) -> [f32; 3] {
    let (pitch, yaw) = (pitch.to_radians(), yaw.to_radians());
    let cp = pitch.cos();
    [cp * yaw.cos(), cp * yaw.sin(), -pitch.sin()]
}

fn aim_basis(pitch: f32, yaw: f32) -> ([f32; 3], [f32; 3]) {
    let (pitch, yaw) = (pitch.to_radians(), yaw.to_radians());
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    ([-sy, cy, 0.0], [sp * cy, sp * sy, cp])
}

fn remap_clamped(value: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    let fraction = ((value - a) / (b - a)).clamp(0.0, 1.0);
    c + (d - c) * fraction
}

fn sticky_physics_request(
    operation: ProjectilePhysicsOperation,
    projectile: &LiveProjectile,
    tick: u64,
) -> ProjectilePhysicsRequest {
    ProjectilePhysicsRequest {
        operation,
        projectile: projectile.presentation.identity,
        tick,
        position: projectile.presentation.position,
        velocity: projectile.presentation.velocity,
        orientation: projectile.presentation.orientation,
        angular_velocity: projectile.presentation.angular_velocity,
        hull: Hull {
            mins: [-2.0; 3],
            maxs: [2.0; 3],
        },
        gravity_scale: 0.4,
        friction: 0.2,
        elasticity: 0.45,
    }
}

fn quaternion_from_angles(pitch: f32, yaw: f32, roll: f32) -> [f32; 4] {
    let (sp, cp) = (pitch.to_radians() * 0.5).sin_cos();
    let (sy, cy) = (yaw.to_radians() * 0.5).sin_cos();
    let (sr, cr) = (roll.to_radians() * 0.5).sin_cos();
    normalized_quaternion([
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ])
}

fn quaternion_from_direction(direction: [f32; 3]) -> [f32; 4] {
    let yaw = direction[1].atan2(direction[0]).to_degrees();
    let pitch = (-direction[2])
        .atan2((direction[0] * direction[0] + direction[1] * direction[1]).sqrt())
        .to_degrees();
    quaternion_from_angles(pitch, yaw, 0.0)
}

fn quaternion_length(value: [f32; 4]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2] + value[3] * value[3]).sqrt()
}

fn normalized(value: [f32; 3]) -> [f32; 3] {
    let magnitude = length(value);
    if magnitude > f32::EPSILON {
        scale(value, 1.0 / magnitude)
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn normalized_quaternion(value: [f32; 4]) -> [f32; 4] {
    let magnitude =
        (value[0] * value[0] + value[1] * value[1] + value[2] * value[2] + value[3] * value[3])
            .sqrt();
    if magnitude > f32::EPSILON {
        value.map(|component| component / magnitude)
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn length(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_movement::{FailureKind, Operation, Trace, Tracer};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[derive(Clone)]
    struct Floor;

    impl Tracer for Floor {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            _: Hull,
            _: u32,
        ) -> Result<Trace, MoveError> {
            if end[2] < 0.0 {
                let fraction = (start[2] / (start[2] - end[2])).clamp(0.0, 1.0);
                Ok(Trace {
                    fraction,
                    start_solid: false,
                    all_solid: false,
                    end: [
                        start[0] + (end[0] - start[0]) * fraction,
                        start[1] + (end[1] - start[1]) * fraction,
                        0.0,
                    ],
                    normal: Some([0.0, 0.0, 1.0]),
                    hit: Some(0),
                    contents: 1,
                })
            } else {
                Ok(Trace {
                    fraction: 1.0,
                    start_solid: false,
                    all_solid: false,
                    end,
                    normal: None,
                    hit: None,
                    contents: 0,
                })
            }
        }
    }

    impl GameplayWorld for Floor {
        fn overlaps_model_hull(
            &self,
            _: usize,
            _: [f32; 3],
            _: [f32; 3],
            _: Hull,
        ) -> Result<bool, MoveError> {
            Ok(false)
        }
    }

    #[derive(Clone)]
    struct SwitchWorld(Arc<AtomicBool>);

    impl Tracer for SwitchWorld {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            hull: Hull,
            mask: u32,
        ) -> Result<Trace, MoveError> {
            Floor.trace(start, end, hull, mask)
        }
    }

    impl GameplayWorld for SwitchWorld {
        fn overlaps_model_hull(
            &self,
            _: usize,
            _: [f32; 3],
            _: [f32; 3],
            _: Hull,
        ) -> Result<bool, MoveError> {
            Ok(self.0.load(Ordering::Relaxed))
        }
    }

    fn explosive(kind: ProjectileKind, position: [f32; 3]) -> LiveProjectile {
        LiveProjectile {
            presentation: Projectile {
                identity: 99,
                kind,
                team: Team::Red,
                owner_identity: PLAYER_IDENTITY,
                launcher_identity: match kind {
                    ProjectileKind::Rocket => Weapon::RocketLauncher as u32,
                    ProjectileKind::Sticky => Weapon::StickybombLauncher as u32,
                },
                state: ProjectileState::Flying,
                position,
                velocity: [0.0; 3],
                orientation: [0.0, 0.0, 0.0, 1.0],
                angular_velocity: [0.0; 3],
                contact_normal: None,
                age_seconds: 1.0,
            },
            armed: true,
            creation_tick: 0,
            arm_tick: 1,
            next_think_tick: 1,
            forced_detonate_tick: None,
            motion_enabled: true,
            direct_target: None,
        }
    }

    #[test]
    fn projectile_contract_emits_oriented_ordered_transitions() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        let fired = session
            .advance(Command {
                pitch_degrees: 89.0,
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(fired.projectile_events[0].kind, ProjectileEventKind::Fire);
        let orientation = fired.projectiles[0].orientation;
        let magnitude = orientation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        assert!((magnitude - 1.0).abs() < 0.0001);
        let trace = session.rocket_trace_requests()[0];
        let exploded = session
            .advance_with_external(
                Command::default(),
                &[],
                &[RocketTraceResult {
                    projectile: trace.projectile,
                    tick: session.tick,
                    end: trace.end,
                    solid: true,
                    sky: false,
                    normal: Some([0.0, 0.0, 1.0]),
                    direct_target: Some(42),
                }],
                None,
            )
            .unwrap();
        assert_eq!(
            exploded
                .projectile_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [ProjectileEventKind::Impact, ProjectileEventKind::Explode]
        );
        assert_eq!(session.radius_damage_requests().len(), 1);
        assert_eq!(session.radius_damage_requests()[0].direct_target, Some(42));

        let mut sky = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        sky.advance(Command {
            fire: true,
            ..Command::default()
        })
        .unwrap();
        let trace = sky.rocket_trace_requests()[0];
        let removed = sky
            .advance_with_external(
                Command::default(),
                &[],
                &[RocketTraceResult {
                    projectile: trace.projectile,
                    tick: sky.tick,
                    end: trace.end,
                    solid: true,
                    sky: true,
                    normal: None,
                    direct_target: None,
                }],
                None,
            )
            .unwrap();
        assert!(removed.projectiles.is_empty());
        assert!(removed.projectile_events.is_empty());
        assert!(sky.radius_damage_requests().is_empty());
    }

    #[test]
    fn self_blast_integration_preserves_damage_force_and_ground_order() {
        let policy = MovementPolicy {
            class: Class::Soldier,
            modifiers: MovementModifiers::default(),
        }
        .resolve();
        let mut grounded = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        grounded.movement = MovementState::from_player(
            Player {
                position: [0.0; 3],
                velocity: [0.0; 3],
                grounded: true,
                crouched: false,
                jump_latched: false,
            },
            policy,
        );
        let mut projectile_events = Vec::new();
        let mut events = Vec::new();
        grounded.explode(
            explosive(ProjectileKind::Rocket, [0.0; 3]),
            &mut projectile_events,
            &mut events,
        );
        assert_eq!(grounded.health, 110);
        assert_eq!(grounded.movement.velocity[2], 450.0);
        assert!(grounded.movement.ground.is_some());
        assert!(grounded.conditions.contains(Condition::BlastJumping));

        let mut airborne = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        airborne.explode(
            explosive(ProjectileKind::Rocket, [0.0; 3]),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert_eq!(airborne.health, 146);
        assert_eq!(airborne.movement.velocity[2], 540.00006);

        let mut crouched = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        crouched.movement = MovementState::from_player(
            Player {
                position: [0.0; 3],
                velocity: [0.0; 3],
                grounded: true,
                crouched: true,
                jump_latched: false,
            },
            policy,
        );
        crouched.explode(
            explosive(ProjectileKind::Rocket, [0.0; 3]),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert_eq!(crouched.health, 110);
        let crouched_expected = 90.0_f32 * ((48.0 * 48.0 * 82.0) / (48.0 * 48.0 * 55.0)) * 5.0;
        assert!((crouched.movement.velocity[2] - crouched_expected).abs() <= f32::EPSILON * 512.0);

        let demo_policy = MovementPolicy {
            class: Class::Demoman,
            modifiers: MovementModifiers::default(),
        }
        .resolve();
        let mut demo = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        demo.class = Class::Demoman;
        demo.weapon = Weapon::StickybombLauncher;
        demo.loadout = default_loadout(Class::Demoman);
        demo.health = 175;
        demo.movement = MovementState::from_player(
            Player {
                position: [0.0; 3],
                velocity: [0.0; 3],
                grounded: true,
                crouched: false,
                jump_latched: false,
            },
            demo_policy,
        );
        demo.explode(
            explosive(ProjectileKind::Sticky, [0.0; 3]),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert_eq!(demo.health, 85);
        assert_eq!(demo.movement.velocity[2], 810.0);
    }

    #[test]
    fn sticky_launch_and_contact_use_only_typed_external_results() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.01));
        session
            .advance(Command {
                select_class: Some(Class::Demoman),
                fire: true,
                ..Command::default()
            })
            .unwrap();
        let before = session.weapon_runtime(Weapon::StickybombLauncher).unwrap();
        let producer_before = session.producer_snapshot();
        assert!(session.requires_sticky_launch_random(Command::default()));
        assert!(before.charge_begin_tick.is_some());
        assert!(matches!(
            session.advance(Command::default()),
            Err(Error::MissingStickyLaunchRandom)
        ));
        assert_eq!(
            session.weapon_runtime(Weapon::StickybombLauncher).unwrap(),
            before
        );
        assert_eq!(session.producer_snapshot(), producer_before);

        assert!(matches!(
            session.advance_with_external(
                Command::default(),
                &[],
                &[],
                Some(StickyLaunchRandom {
                    right_velocity: 10.01,
                    up_velocity: 0.0,
                    angular_y: 0,
                }),
            ),
            Err(Error::InvalidStickyLaunchRandom)
        ));
        assert_eq!(session.producer_snapshot(), producer_before);

        let fired = session
            .advance_with_external(
                Command::default(),
                &[],
                &[],
                Some(StickyLaunchRandom {
                    right_velocity: -10.0,
                    up_velocity: 10.0,
                    angular_y: 1200,
                }),
            )
            .unwrap();
        assert_eq!(fired.projectiles.len(), 1);
        let projectile = fired.projectiles[0].clone();
        assert_eq!(projectile.velocity, [905.625, -10.0, 210.0]);
        assert!(session.physics_requests().iter().any(|request| {
            request.operation == ProjectilePhysicsOperation::Create
                && request.projectile == projectile.identity
                && request.hull.mins == [-2.0; 3]
                && request.gravity_scale == 0.4
                && request.friction == 0.2
                && request.elasticity == 0.45
        }));

        let contact = session
            .advance_with_external(
                Command::default(),
                &[ProjectilePhysicsResult {
                    projectile: projectile.identity,
                    tick: session.tick,
                    position: [16.0, 0.0, -2.0],
                    velocity: [700.0, 50.0, -100.0],
                    orientation: [0.0, 0.0, 0.0, 1.0],
                    angular_velocity: [600.0, 1200.0, 0.0],
                    motion_enabled: true,
                    contact: Some(ProjectileContact {
                        kind: ProjectileContactKind::World,
                        normal: [0.0, 0.0, 1.0],
                    }),
                }],
                &[],
                None,
            )
            .unwrap();
        assert_eq!(contact.projectiles[0].state, ProjectileState::StuckUnarmed);
        assert_eq!(
            contact
                .projectile_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [ProjectileEventKind::Impact, ProjectileEventKind::Stick]
        );
        assert!(session.physics_requests().iter().any(|request| {
            request.operation == ProjectilePhysicsOperation::DisableMotion
                && request.projectile == projectile.identity
        }));
        let stuck = session.producer_snapshot();
        assert!(matches!(
            session.advance_with_external(
                Command::default(),
                &[ProjectilePhysicsResult {
                    projectile: projectile.identity,
                    tick: session.tick,
                    position: [f32::NAN, 0.0, 0.0],
                    velocity: [0.0; 3],
                    orientation: [0.0, 0.0, 0.0, 1.0],
                    angular_velocity: [0.0; 3],
                    motion_enabled: false,
                    contact: None,
                }],
                &[],
                None,
            ),
            Err(Error::InvalidProjectilePhysics)
        ));
        assert_eq!(session.producer_snapshot(), stuck);
    }

    #[test]
    fn movement_mode_ammo_reload_and_atomic_failure_share_one_state() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session.set_movement_modifiers(MovementModifiers {
            noclip_allowed: true,
            ..MovementModifiers::default()
        });
        let snapshot = session
            .advance(Command {
                up: 450.0,
                speed_button: true,
                mode_request: Some(Mode::Noclip),
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(snapshot.movement.mode, Mode::Noclip);
        assert_eq!(snapshot.loadout[0].clip, 3);
        let before = session.movement_snapshot_bytes();
        let failure = session
            .advance(Command {
                pitch_degrees: f32::NAN,
                select_class: Some(Class::Demoman),
                ..Command::default()
            })
            .unwrap_err();
        assert!(matches!(
            failure,
            Error::Movement(MoveError {
                operation: Operation::Validate,
                kind: FailureKind::Malformed,
                ..
            })
        ));
        assert_eq!(session.class, Class::Soldier);
        assert_eq!(session.movement_snapshot_bytes(), before);
    }

    #[test]
    fn lethal_state_waits_for_explicit_respawn_and_cleans_owned_projectiles() {
        let mut session = Session::new(Floor, [8.0, 4.0, 2.0], MapRuntime::empty(0.01));
        session.health = 0;
        session
            .projectiles
            .push(explosive(ProjectileKind::Rocket, [0.0; 3]));
        session
            .projectiles
            .push(explosive(ProjectileKind::Sticky, [0.0; 3]));
        let dead = session.advance(Command::default()).unwrap();
        assert_eq!(dead.health, 0.0);
        assert_eq!(session.lifecycle(), PlayerLifecycle::Dying);
        assert!(dead.projectiles.is_empty());
        assert_eq!(
            dead.projectile_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [ProjectileEventKind::Fizzle, ProjectileEventKind::Fizzle]
        );
        assert!(
            session
                .lifecycle_events()
                .iter()
                .any(|event| { event.kind == LifecycleEventKind::Died })
        );

        let respawned = session
            .advance(Command {
                respawn: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(session.lifecycle(), PlayerLifecycle::Active);
        assert_eq!(respawned.health, 200.0);
        assert_eq!(respawned.movement.position, [8.0, 4.0, 2.0]);
        assert!(
            session
                .lifecycle_events()
                .iter()
                .any(|event| { event.kind == LifecycleEventKind::Respawned })
        );
        assert!(
            session
                .activity_events()
                .iter()
                .any(|event| { event.activity == weapon::WeaponActivity::Draw })
        );
    }

    #[test]
    fn regenerate_restores_stock_state_once_per_touch_cooldown() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"prop_dynamic\"\"targetname\"\"aaresupply\"\"origin\"\"0 0 0\"}{\"classname\"\"func_regenerate\"\"model\"\"*1\"\"TeamNum\"\"0\"\"StartDisabled\"\"0\"\"associatedmodel\"\"aaresupply\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![playsrc_entity::ModelBounds {
                model: 1,
                mins: [-16.0; 3],
                maxs: [16.0; 3],
            }],
        )
        .unwrap();
        assert_eq!(map.counts().regenerate_zones, 1);
        let touching = Arc::new(AtomicBool::new(false));
        let mut session = Session::new(SwitchWorld(touching.clone()), [0.0; 3], map);
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        session.health = 50;
        for condition in resupply_removed_conditions() {
            session.conditions.insert(condition);
        }
        touching.store(true, Ordering::Relaxed);
        let supplied = session.advance(Command::default()).unwrap();
        assert_eq!(supplied.health, 200.0);
        assert_eq!(supplied.loadout[0].clip, 4);
        assert_eq!(supplied.loadout[0].reserve, 20);
        assert!(
            resupply_removed_conditions()
                .into_iter()
                .all(|condition| !session.conditions.contains(condition))
        );
        assert!(matches!(
            supplied.events.as_slice(),
            [Event::Resupplied { entity: 1, .. }]
        ));
        assert_eq!(
            session.regenerate_animation_events(),
            &[RegenerateAnimationEvent {
                zone: 1,
                associated_model: 0,
                open_tick: 1,
                close_tick: 135,
            }]
        );
        session.health = 100;
        let cooling_down = session.advance(Command::default()).unwrap();
        assert_eq!(cooling_down.health, 100.0);
        assert!(cooling_down.events.is_empty());

        session.next_regenerate_tick = 0;
        session.health = 50;
        session.set_player_restrictions(PlayerRestrictions {
            taunting: true,
            ..PlayerRestrictions::default()
        });
        let taunting = session.advance(Command::default()).unwrap();
        assert_eq!(taunting.health, 50.0);
        assert!(taunting.events.is_empty());

        session.conditions.clear();
        session.conditions.insert(Condition::CannotSwitchFromMelee);
        session.regenerate(1, None, &mut Vec::new());
        assert!(
            session
                .conditions
                .contains(Condition::CannotSwitchFromMelee)
        );
    }

    #[test]
    fn respawn_room_contacts_are_edge_counted_with_source_team_identity() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"func_respawnroom\"\"model\"\"*1\"\"TeamNum\"\"2\"\"StartDisabled\"\"0\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            4,
            vec![playsrc_entity::ModelBounds {
                model: 1,
                mins: [-16.0; 3],
                maxs: [16.0; 3],
            }],
        )
        .unwrap();
        assert_eq!(map.counts().respawn_rooms, 1);
        let touching = Arc::new(AtomicBool::new(false));
        let mut session = Session::new(SwitchWorld(touching.clone()), [0.0; 3], map);
        session.advance(Command::default()).unwrap();
        assert_eq!(session.respawn_touch_count(), 0);
        touching.store(true, Ordering::Relaxed);
        session.advance(Command::default()).unwrap();
        assert_eq!(session.respawn_touch_count(), 1);
        session.advance(Command::default()).unwrap();
        assert_eq!(session.respawn_touch_count(), 1);
        touching.store(false, Ordering::Relaxed);
        session.advance(Command::default()).unwrap();
        assert_eq!(session.respawn_touch_count(), 0);
    }

    #[test]
    fn map_io_drives_button_door_and_logic_auto_movelinear_requests() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"func_button\"\"model\"\"*1\"\"targetname\"\"button\"\"OnDamaged\"\"door,Open,,0,-1\"}{\"classname\"\"func_door\"\"model\"\"*2\"\"targetname\"\"door\"\"speed\"\"100\"\"movedir\"\"-90 0 0\"}{\"classname\"\"logic_auto\"\"OnMapSpawn\"\"platform,Open,,0,-1\"}{\"classname\"\"func_movelinear\"\"model\"\"*3\"\"targetname\"\"platform\"\"speed\"\"75\"\"MoveDistance\"\"650\"\"movedir\"\"0 90 0\"}{\"classname\"\"trigger_multiple\"\"model\"\"*4\"\"spawnflags\"\"1\"}{\"classname\"\"trigger_multiple\"\"model\"\"*5\"\"spawnflags\"\"1\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            2,
            (1..=5)
                .map(|model| playsrc_entity::ModelBounds {
                    model,
                    mins: [-16.0; 3],
                    maxs: [16.0; 3],
                })
                .collect(),
        )
        .unwrap();
        assert_eq!(map.counts().buttons, 1);
        assert_eq!(map.counts().doors, 1);
        assert_eq!(map.counts().linear_movers, 1);
        assert_eq!(map.counts().multiple_triggers, 2);
        let mut session = Session::new(Floor, [0.0; 3], map);
        session
            .configure_jump(
                jump::CourseDefinition::linear(
                    1,
                    [2; 32],
                    vec![
                        jump::Zone {
                            identity: 1,
                            trigger_entity: 4,
                            kind: jump::ZoneKind::MapStart,
                            index: 1,
                        },
                        jump::Zone {
                            identity: 2,
                            trigger_entity: 5,
                            kind: jump::ZoneKind::MapEnd,
                            index: 1,
                        },
                    ],
                    jump::Limits::default(),
                )
                .unwrap(),
            )
            .unwrap();
        let door = session
            .advance(Command {
                activate_entity: Some(0),
                ..Command::default()
            })
            .unwrap();
        assert_eq!(door.maximum_health, 900.0);
        assert!(
            door.entity_events
                .iter()
                .any(|event| { event.kind == EntityEventKind::MoverStarted && event.entity == 1 })
        );
        let requests = session.mover_requests().to_vec();
        assert!(requests.iter().any(|request| request.entity == 1));
        let results: Vec<_> = requests
            .iter()
            .map(|request| {
                let delta = sub(request.destination, request.start);
                let distance = length(delta);
                MoverResult {
                    request_id: request.request_id,
                    entity: request.entity,
                    kind: MoverResultKind::Progress,
                    transform: playsrc_entity::Transform {
                        origin: add(request.start, scale(delta, 1.0 / distance)),
                        angles: [0.0; 3],
                    },
                    carry: [0.0; 3],
                }
            })
            .collect();
        session.apply_mover_results(&results).unwrap();
        let mut moved = session.advance(Command::default()).unwrap();
        assert!(
            moved
                .entity_transforms
                .iter()
                .any(|transform| { transform.identity == 1 && transform.position[2] > 0.0 })
        );
        let mut platform = None;
        for _ in 0..20 {
            session.advance(Command::default()).unwrap();
            if let Some(request) = session
                .mover_requests()
                .iter()
                .find(|request| request.entity == 3)
                .copied()
            {
                platform = Some(request);
                break;
            }
        }
        let request = platform.expect("logic_auto platform request");
        let delta = sub(request.destination, request.start);
        let distance = length(delta);
        session
            .apply_mover_results(&[MoverResult {
                request_id: request.request_id,
                entity: request.entity,
                kind: MoverResultKind::Progress,
                transform: playsrc_entity::Transform {
                    origin: add(request.start, scale(delta, 1.0 / distance)),
                    angles: [0.0; 3],
                },
                carry: [0.0; 3],
            }])
            .unwrap();
        moved = session.advance(Command::default()).unwrap();
        assert!(
            moved
                .entity_transforms
                .iter()
                .any(|transform| { transform.identity == 3 && transform.position[1] > 0.0 })
        );
    }
}
