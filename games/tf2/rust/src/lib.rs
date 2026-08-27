pub mod attribute;
pub mod audio;
pub mod ballistics;
pub mod bot;
pub mod admission_metrics;
pub mod building;
pub mod class;
pub mod combat;
pub mod condition;
pub mod control_point;
pub mod ctf;
pub mod damage;
pub mod deathnotice;
pub mod dynamic_prop;
pub mod health;
pub mod koth;
mod map_runtime;
pub mod medic;
pub mod pickup;
pub mod pyro;
pub mod random;
pub mod round;
pub mod schema;
pub mod equipment;
pub mod scoreboard;
pub mod spy;
pub mod state;
pub mod team_selection;
pub mod weapon;

pub use audio::{
    AudioAction, AudioEvent, AudioEventIdentity, AudioSourceKind, SoundDefinition, SoundQueryPhase,
    SoundSamples, SoundSelectionState,
};
pub use random::{
    RandomContext, RandomDecision, RandomDraw, RandomError, RandomResult, RandomSeeds,
    Tf2RandomState, UniformRandomState, UniformRandomStream,
};

#[path = "../../rulesets/jump/rust/src/lib.rs"]
pub mod jump;

pub use map_runtime::{
    ActorContact, CONTENTS_BLUE_TEAM, CONTENTS_RED_TEAM, CombatPlayerFacts, Effect as MapEffect,
    EntityEvent, EntityEventKind, EntityTransform, GameplayWorld, MapCounts, MapPhase,
    MapPickupSnapshot, MapRuntime, MoverRequest, MoverResult, MoverResultKind, PlayerContactFacts,
    RegenerateContact, respawn_barrier_collides, regenerate_associated_model,
};

use std::collections::BTreeMap;

use playsrc_collision::Hull;
use playsrc_movement::{
    Command as MoveCommand, Configuration as MovementConfiguration, Error as MoveError, Mode,
    ModeRequest, Player, Policy as GenericMovementPolicy, State as MovementState, StepInput,
    StepResult as MovementStepResult, StepStrategy, TransitionDisposition, WaterPolicy,
    WaterSampling, step,
};

use audio::SoundSelection;
use map_runtime::{BeginTickInput, MapError};
use weapon::{ActivityEvent, PrimaryResult, ReloadPhase, WeaponRuntime};

pub const PLAYER_IDENTITY: u32 = 1;
pub const FL_ONGROUND: u32 = 1 << 0;
pub const FL_DUCKING: u32 = 1 << 1;
pub const FL_ANIMDUCKING: u32 = 1 << 2;
pub const FL_CLIENT: u32 = 1 << 8;
pub const FL_INWATER: u32 = 1 << 10;
pub const MAX_PROJECTILES: usize = 64;
const MASK_SOLID: u32 = 0x0200_400b;
const MASK_SHOT: u32 = 0x4600_4003;
const MASK_SOLID_BRUSH_ONLY: u32 = 0x0000_400b;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationRevision {
    pub entity: u64,
    pub collision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityPresentationSnapshot {
    pub collision_revision: u64,
    pub entities: playsrc_entity::BrushModelPresentation,
    pub studio_models: Vec<playsrc_entity::StudioModelDrawState>,
    pub studio_animations: Vec<dynamic_prop::AnimationPresentation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationError {
    Entity(playsrc_entity::RuntimeFailure),
    CollisionRevisionUnavailable,
    CollisionRevisionMismatch { expected: u64, actual: u64 },
}

pub use class::{PlayerClass, PlayerTeam};

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
    pub class: PlayerClass,
    pub modifiers: MovementModifiers,
}

impl MovementPolicy {
    pub fn resolve(self) -> GenericMovementPolicy {
        let class_speed = self.class.data().maximum_speed;
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
            air_dash_impulse: (self.class == PlayerClass::Scout).then_some(
                268.328_16 * self.modifiers.condition_jump_factor * self.modifiers.item_jump_factor,
            ),
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
            standing_view: [0.0, 0.0, self.class.standing_eye_height()],
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
            water: WaterPolicy {
                sampling: WaterSampling::EyesThenWaist,
                waist_height_offset: 12.0,
                refresh_before_walk: false,
                apply_currents: false,
                jump_wish_at_waist: false,
                amplify_forward_pitch: false,
                ledge_uses_command_direction: true,
                ledge_jump_overrides_backward: true,
                suppress_airborne_duck: true,
                suppress_submerged_duck: true,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Weapon {
    RocketLauncher = 1,
    Original = 2,
    StickybombLauncher = 3,
    Scattergun = 4,
    Pistol = 5,
    Bat = 6,

    Shotgun = 7,
    Shovel = 8,
    Minigun = 9,
    HeavyShotgun = 10,
    Fists = 11,
    SniperRifle = 12,
    Smg = 13,
    Kukri = 14,
    Bottle = 17,
    GrenadeLauncher = 18,
    EngineerShotgun = 40,
    EngineerPistol = 41,
    Wrench = 42,
    Flamethrower = 15,
    FireAxe = 16,
    Revolver = 50,
    Knife = 51,
    Sapper = 52,
    DisguiseKit = 53,
    InvisibilityWatch = 54,
    BuildPda = 43,
    DestroyPda = 44,
    Toolbox = 45,
    SyringeGun = 19,
    MediGun = 20,
    Bonesaw = 21,
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
    pub drop_item: bool,
    pub nextbot_stop: bool,
    pub select_class: Option<PlayerClass>,
    pub select_random_class: bool,
    pub select_team: Option<PlayerTeam>,
    pub select_weapon: Option<Weapon>,
    pub disguise: Option<spy::Disguise>,
    pub mode_request: Option<Mode>,
    pub activate_entity: Option<u32>,
    pub bot_request: Option<bot::Request>,
    pub building_request: Option<building::Request>,
    pub bot_configuration: Option<bot::Configuration>,
    pub objective_configuration: Option<ctf::RuleConfiguration>,
    pub bot_control: Option<bot::Control>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProjectileKind {
    Rocket = 1,
    Sticky = 2,
    Syringe = 3,
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
    pub team: PlayerTeam,
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
    killing_weapon: &'static str,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileLauncherPose {
    pub eye_position: [f32; 3],
    pub view_orientation: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectileEvent {
    pub kind: ProjectileEventKind,
    pub projectile: u32,
    pub projectile_kind: ProjectileKind,
    pub owner_identity: u32,
    pub launcher_identity: u32,
    pub team: PlayerTeam,
    pub tick: u64,
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub contact_normal: Option<[f32; 3]>,
    pub launcher_pose: Option<ProjectileLauncherPose>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Condition {
    Aiming = 0,

    Zoomed = 1,

    Disguising = 2,
    Disguised = 3,
    Stealthed = 4,
    Invulnerable = 5,
    StealthedBlink = 9,
    Phase = 14,
    HealthBuff = 21,
    Overhealed = 23,
    EnergyBuff = 19,
    Burning = 22,
    Urine = 24,
    Bleeding = 25,
    MadMilk = 27,
    CannotSwitchFromMelee = 41,
    DisguiseWearingOff = 47,
    ParachuteActive = 80,
    BlastJumping = 81,
    Plague = 112,
    Gas = 123,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ConditionSet {
    words: [u32; 5],
    active_stun_flags: Option<u16>,
}

impl ConditionSet {
    fn runtime_state(&self) -> condition::ConditionState {
        let mut state = condition::ConditionState::default();
        for value in 0..condition::CONDITION_COUNT as u8 {
            if self.words[value as usize / 32] & (1 << (value as usize % 32)) != 0 {
                state.add(condition::ConditionId::new(value).unwrap(), condition::ConditionDuration::Permanent, None, true, false).expect("active condition");
            }
        }
        state.set_active_stun_flags(self.active_stun_flags);
        state
    }
    pub fn set_active_stun_flags(&mut self, flags: Option<u16>) { self.active_stun_flags = flags; }
    pub fn is_control_stunned(&self) -> bool { self.words[0] & (1 << 15) != 0 && self.active_stun_flags.is_some_and(|flags| flags & 2 != 0) }
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
        self.active_stun_flags = None;
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
    Welcome,
    Observer,
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
    pub class: PlayerClass,
    pub team: PlayerTeam,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegenerateAnimationEvent {
    pub zone: u32,
    pub associated_model: u32,
    pub open_tick: u64,
    pub close_tick: u64,
    pub body: i32,
    pub open_animation: RegenerateModelAnimation,
    pub close_animation: RegenerateModelAnimation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegenerateModelAnimation {
    Open,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegenerateModelEvent {
    pub zone: u32,
    pub associated_model: u32,
    pub tick: u64,
    pub animation: RegenerateModelAnimation,
    pub body: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRegenerateModelClose {
    zone: u32,
    associated_model: u32,
    tick: u64,
    body: i32,
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
    pub team_win: Option<PlayerTeam>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    ClassChanged(PlayerClass),
    TeamChanged(PlayerTeam),
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
        origin: Option<[f32; 3]>,
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
    HitscanFired {
        weapon: Weapon,
        pellets: u8,
        owner: u32,
        origin: [f32; 3],
    },
    HitscanImpact {
        weapon: Weapon,
        target: Option<u32>,
        pellet: u8,
        hitgroup: u8,
        critical: bool,
        position: [f32; 3],
        damage: f32,
        surface: Option<playsrc_movement::ImpactSurface>,
    },
    MeleeImpact {
        weapon: Weapon,
        owner: u32,
        target: Option<u32>,
        position: [f32; 3],
        damage: f32,
    },
    PickedUp {
        entity: u32,
        player: u32,
        kind: pickup::MapPickupKind,
        size: pickup::PickupSize,
        amount: u16,
        health: i32,
        weapon: Option<Weapon>,
        clip: u16,
        reserve: u16,
    },
    PlayerDamaged {
        attacker: u32,
        victim: u32,
        weapon: Weapon,
        amount: u32,
        health: i32,
        critical: bool,
        custom: u8,
    },
    PlayerKilled {
        attacker: u32,
        victim: u32,
        weapon: Option<Weapon>,
        killing_weapon: &'static str,
        assister: u32,
        damage_bits: u32,
        critical: bool,
        custom: u8,
    },
    PlayerRespawned {
        player: u32,
        team: PlayerTeam,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub class: PlayerClass,
    pub team: PlayerTeam,
    pub weapon: Option<Weapon>,
    pub player_flags: u32,
    pub movement: MovementState,
    pub view_angle_offset: [f32; 3],
    pub health: f32,
    pub maximum_health: f32,
    pub spy: Option<spy::SpyState>,
    pub loadout: Vec<WeaponState>,
    pub equipped_items: Vec<equipment::EquippedItem>,
    pub conditions: u32,
    pub projectiles: Vec<Projectile>,
    pub projectile_events: Vec<ProjectileEvent>,
    pub entity_transforms: Vec<EntityTransform>,
    pub entity_events: Vec<EntityEvent>,
    pub objectives: Option<ctf::Snapshot>,
    pub control_points: Option<control_point::Snapshot>,
    pub round: round::Snapshot,
    pub jump: Option<jump::TickOutput>,
    pub events: Vec<Event>,
    pub bots: Vec<bot::Snapshot>,
    pub pickups: Vec<MapPickupSnapshot>,
    pub buildings: Vec<building::Snapshot>,
    pub placement: Option<building::Placement>,
    pub metal: u16,
    pub scoreboard: scoreboard::Snapshot,
    pub medigun_charge: f32,
    pub medigun_target: Option<u32>,
    pub medigun_releasing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProducerSnapshot {
    pub tick: u64,
    pub lifecycle: PlayerLifecycle,
    pub class: PlayerClass,
    pub team: PlayerTeam,
    pub active_weapon: Option<Weapon>,
    pub player_flags: u32,
    pub health: i32,
    pub maximum_health: i32,
    pub conditions: [u32; 5],
    pub weapons: Vec<WeaponRuntime>,
    pub flame_points: Vec<pyro::FlamePoint>,
    pub shotgun_pellets: Vec<pyro::ShotgunPellet>,
    pub flame_firing: bool,
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

#[derive(Clone, Debug, PartialEq)]
pub struct PosedPlayerHitbox {
    pub entity: u32,
    pub team: PlayerTeam,
    pub hitbox: usize,
    pub group: i32,
    pub bone: usize,
    pub physics_bone: i32,
    pub bone_contents: u32,
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
    pub bone_to_world: [f32; 12],
    pub origin: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSpawn {
    pub position: [f32; 3],
    pub angles: [f32; 3],
}

#[derive(Clone)]
pub struct Session<W: GameplayWorld + Clone> {
    collision: W,
    tick: u64,
    class: PlayerClass,
    team_selection: team_selection::TeamSelection,
    pending_team_change: Option<PlayerTeam>,
    weapon: Option<Weapon>,
    loadout: BTreeMap<Weapon, WeaponRuntime>,
    equipment: equipment::Equipment,
    active_equipment: equipment::Equipment,
    equipment_attributes: equipment::AttributeProviders,
    equipment_respawn_requested: bool,
    bot_equipment: BTreeMap<u32, equipment::Equipment>,
    ammo: class::AmmoLedger,
    movement: MovementState,
    air_dashes: u8,
    in_water: bool,
    movement_modifiers: MovementModifiers,
    last_movement: Option<MovementStepResult>,
    health: i32,
    spy: Option<spy::SpyState>,
    conditions: ConditionSet,
    lifecycle: PlayerLifecycle,
    restrictions: PlayerRestrictions,
    auto_reload: bool,
    flip_viewmodels: bool,
    spawn: [f32; 3],
    spawn_angles: [f32; 3],
    raw_view_angles: [f32; 3],
    view_angle_offset: [f32; 3],
    projectiles: Vec<LiveProjectile>,
    next_projectile: u32,
    fire_was_held: bool,
    flame_firing: bool,
    flame_burst_until: f32,
    fire_on_empty: bool,
    secondary_was_held: bool,
    previous_hitscan_ticks: BTreeMap<Weapon, u64>,
    pending_melee_tick: Option<u64>,
    flames: pyro::FlameManager,
    next_airblast_tick: u64,
    shotgun_pellets: Vec<pyro::ShotgunPellet>,
    authority_random: UniformRandomStream,
    predicted_presentation_random: UniformRandomStream,
    sound_selection: SoundSelection,
    random_draws: Vec<RandomDraw>,
    audio_events: Vec<AudioEvent>,
    physics_requests: Vec<ProjectilePhysicsRequest>,
    activity_events: Vec<ActivityEvent>,
    mover_requests: Vec<MoverRequest>,
    lifecycle_events: Vec<LifecycleEvent>,
    regenerate_animation_events: Vec<RegenerateAnimationEvent>,
    regenerate_model_events: Vec<RegenerateModelEvent>,
    regenerate_model_states: BTreeMap<u32, RegenerateModelAnimation>,
    pending_regenerate_model_closes: Vec<PendingRegenerateModelClose>,
    regenerate_contacts: Vec<RegenerateContact>,
    map_effects: Vec<MapEffect>,
    radius_damage_requests: Vec<RadiusDamageRequest>,
    rocket_trace_requests: Vec<RocketTraceRequest>,
    contact_reconcile_requests: Vec<ContactReconcileRequest>,
    movement_configuration: MovementConfiguration,
    map: MapRuntime,
    round: round::Rules,
    next_regenerate_tick: u64,
    hurt_next_tick: BTreeMap<u32, u64>,
    hurt_active: std::collections::BTreeSet<u32>,
    hurt_applied: std::collections::BTreeSet<u32>,
    respawn_touch_count: u32,
    ctf_capture_bonus_until: Option<u64>,
    jump: Option<jump::Session>,
    bots: Option<bot::BotWorld>,
    scoreboard: scoreboard::State,
    damagers: deathnotice::DamagerHistory,
    posed_player_hitboxes: Vec<PosedPlayerHitbox>,
    respawn_tick: Option<u64>,
    death_tick: Option<u64>,
    buildings: building::World,
    medigun: medic::MedigunState,
}

#[derive(Debug)]
pub enum Error {
    Movement(MoveError),
    PickupTrace(MoveError),
    Entity(playsrc_entity::RuntimeFailure),
    Jump(jump::Error),
    MissingEntity(u32),
    InvalidCourseTrigger(u32),
    UnsupportedJumpClass(PlayerClass),
    ProjectileLimit,
    InvalidStickyLaunchRandom,
    InvalidProjectilePhysics,
    InvalidPlayerPosition,
    Random(RandomError),
    Bot(bot::Error),
    TeamSelection(team_selection::TeamSelectionError),
    Objectives(ctf::Error),
    ControlPoints(control_point::Error),
    Round(round::Error),
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

impl From<RandomError> for Error {
    fn from(error: RandomError) -> Self {
        Self::Random(error)
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
            MapError::PickupTrace(error) => Self::PickupTrace(error),
            MapError::MissingEntity(entity) => Self::MissingEntity(entity),
        }
    }
}

impl<W: GameplayWorld + Clone> Session<W> {
    pub fn new(collision: W, spawn: [f32; 3], map: MapRuntime) -> Self {
        let movement_modifiers = MovementModifiers::default();
        let movement_policy = MovementPolicy {
            class: PlayerClass::Soldier,
            modifiers: movement_modifiers,
        }
        .resolve();
        let movement_configuration = MovementConfiguration {
            water_exit_forward: 30.0,
            water_exit_up_speed: 300.0,
            ..MovementConfiguration::default()
        };
        let mut loadout = default_loadout(PlayerClass::Soldier);
        loadout
            .get_mut(&Weapon::RocketLauncher)
            .expect("stock Soldier loadout")
            .deploy(0, movement_configuration.tick_interval);
        let round = round::Rules::active(map.round_configuration())
            .expect("compiled map round configuration is valid");
        let authority_random = UniformRandomStream::from_seed(RandomSeeds::INVARIANT.authority)
            .expect("invariant authority seed is valid");
        let predicted_presentation_random =
            UniformRandomStream::from_seed(RandomSeeds::INVARIANT.predicted_presentation)
                .expect("invariant presentation seed is valid");
        Self {
            collision,
            tick: 0,
            class: PlayerClass::Soldier,
            team_selection: {
                let mut selection = team_selection::TeamSelection::new(
                    PLAYER_IDENTITY,
                    team_selection::TeamRules::default(),
                )
                .expect("local team selection identity is valid");
                selection
                    .select(team_selection::TeamChoice::Red, false)
                    .expect("initial active RED player is admitted");
                selection
            },
            pending_team_change: None,
            weapon: Some(Weapon::RocketLauncher),
            loadout,
            equipment: equipment::Equipment::default(),
            active_equipment: equipment::Equipment::default(),
            equipment_attributes: equipment::AttributeProviders::new(&equipment::Equipment::default(), PlayerClass::Soldier),
            equipment_respawn_requested: false,
            bot_equipment: BTreeMap::new(),
            ammo: PlayerClass::Soldier.data().maximum_ammo,
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
            air_dashes: 0,
            in_water: false,
            movement_modifiers,
            last_movement: None,
            health: PlayerClass::Soldier.data().maximum_health,
            spy: None,
            conditions: ConditionSet::default(),
            lifecycle: PlayerLifecycle::Active,
            restrictions: PlayerRestrictions::default(),
            auto_reload: true,
            flip_viewmodels: false,
            spawn,
            spawn_angles: [0.0; 3],
            raw_view_angles: [0.0; 3],
            view_angle_offset: [0.0; 3],
            projectiles: Vec::new(),
            next_projectile: 1,
            fire_was_held: false,
            flame_firing: false,
            flame_burst_until: 0.0,
            fire_on_empty: false,
            secondary_was_held: false,
            previous_hitscan_ticks: BTreeMap::new(),
            pending_melee_tick: None,
            flames: pyro::FlameManager::default(),
            next_airblast_tick: 0,
            shotgun_pellets: Vec::with_capacity(pyro::SHOTGUN_PELLETS),
            authority_random,
            predicted_presentation_random,
            sound_selection: SoundSelection::new(),
            random_draws: Vec::new(),
            audio_events: Vec::new(),
            physics_requests: Vec::new(),
            activity_events: Vec::new(),
            mover_requests: Vec::new(),
            lifecycle_events: Vec::new(),
            regenerate_animation_events: Vec::new(),
            regenerate_model_events: Vec::new(),
            regenerate_model_states: BTreeMap::new(),
            pending_regenerate_model_closes: Vec::new(),
            regenerate_contacts: Vec::new(),
            map_effects: Vec::new(),
            radius_damage_requests: Vec::new(),
            rocket_trace_requests: Vec::new(),
            contact_reconcile_requests: Vec::new(),
            movement_configuration,
            map,
            round,
            next_regenerate_tick: 0,
            hurt_next_tick: BTreeMap::new(),
            hurt_active: std::collections::BTreeSet::new(),
            hurt_applied: std::collections::BTreeSet::new(),
            respawn_touch_count: 0,
            ctf_capture_bonus_until: None,
            jump: None,
            bots: None,
            scoreboard: scoreboard::State::default(),
            damagers: deathnotice::DamagerHistory::default(),
            posed_player_hitboxes: Vec::new(),
            respawn_tick: None,
            death_tick: None,
            buildings: building::World::new(movement_configuration.tick_interval),
            medigun: medic::MedigunState::default(),
        }
    }

    pub fn connected(
        collision: W,
        spawn: [f32; 3],
        map: MapRuntime,
        rules: team_selection::TeamRules,
    ) -> Self {
        let mut session = Self::new(collision, spawn, map);
        session.team_selection = team_selection::TeamSelection::new(PLAYER_IDENTITY, rules)
            .expect("local team selection identity is valid");
        session.round = round::Rules::new(session.map.round_configuration())
            .expect("compiled map round configuration is valid");
        session.lifecycle = PlayerLifecycle::Welcome;
        session.weapon = None;
        session.loadout.clear();
        session.health = 0;
        session.activity_events.clear();
        session
    }

    pub fn team_snapshot(&self) -> team_selection::TeamSnapshot {
        self.team_selection.snapshot()
    }

    pub fn equipment(&self) -> &equipment::Equipment { &self.equipment }

    pub fn equip_bot_cosmetic(&mut self, identity: u32, definition_index: Option<u32>) -> Result<bool, equipment::EquipmentError> {
        let class = self.bots.as_ref().and_then(|bots| bots.snapshots().into_iter().find(|bot| bot.identity == identity)).map(|bot| bot.class)
            .ok_or(equipment::EquipmentError::IneligibleSlot)?;
        if let Some(definition) = definition_index {
            if equipment::supported_item(definition).is_none_or(|item| item.implementation != equipment::Implementation::Wearable) {
                return Err(equipment::EquipmentError::UnsupportedItem);
            }
        }
        self.bot_equipment.entry(identity).or_default().equip(class, schema::LoadoutPosition::Misc, definition_index)
    }

    pub fn equip_item(&mut self, class: PlayerClass, slot: schema::LoadoutPosition, definition_index: Option<u32>) -> Result<bool, equipment::EquipmentError> {
        let changed = self.equipment.equip(class, slot, definition_index)?;
        self.equipment_respawn_requested |= changed && class == self.class;
        Ok(changed)
    }

    pub fn restore_equipment(&mut self, bytes: &[u8]) -> Result<(), equipment::EquipmentError> {
        self.equipment = equipment::Equipment::restore(bytes)?;
        self.equipment_respawn_requested |= !self.equipment.class_matches(&self.active_equipment, self.class);
        Ok(())
    }

    pub fn equipped_weapon_attribute(&mut self, owner: u32, weapon: Weapon, hook: &str, input: f32) -> f32 {
        if owner != PLAYER_IDENTITY { return input; }
        self.equipment_attributes.weapon(weapon, hook, input)
    }

    pub fn equipped_player_attribute(&mut self, owner: u32, hook: &str, input: f32) -> f32 {
        if owner != PLAYER_IDENTITY { return input; }
        self.equipment_attributes.player(hook, input)
    }

    fn apply_equipment(&mut self) {
        self.active_equipment = self.equipment.clone();
        self.equipment_attributes = equipment::AttributeProviders::new(&self.active_equipment, self.class);
        self.loadout.clear();
        for weapon in self.active_equipment.weapons(self.class) {
            let context = weapon::ProfileContext {
                ammo: weapon_ammo_kind(weapon),
                gun: weapon_ammo_kind(weapon).is_some(),
                blast_impact: matches!(weapon, Weapon::RocketLauncher | Weapon::Original | Weapon::GrenadeLauncher | Weapon::StickybombLauncher),
                ..Default::default()
            };
            let profile = weapon::WeaponProfile::configured(weapon).with_attributes(context, |target, hook, input| match target {
                weapon::AttributeTarget::Weapon => self.equipment_attributes.weapon(weapon, hook, input),
                weapon::AttributeTarget::Player => self.equipment_attributes.player(hook, input),
            });
            self.loadout.insert(weapon, WeaponRuntime::full_with_profile(weapon, profile));
        }
        self.ammo = self.maximum_ammo();
        if self.weapon.is_none_or(|weapon| !self.loadout.contains_key(&weapon)) {
            self.weapon = self.active_equipment.weapons(self.class).next();
        }
    }

    pub fn round_snapshot(&self) -> round::Snapshot {
        self.round.snapshot(Vec::new())
    }

    pub fn configure_waiting(&mut self, seconds: f32) -> Result<(), round::Error> {
        self.round.configure_waiting(seconds)
    }

    pub fn select_team_choice(
        &mut self,
        choice: team_selection::TeamChoice,
    ) -> Result<Option<class::PlayerTeam>, team_selection::TeamSelectionError> {
        let before = self.team_selection.snapshot();
        let random = if matches!(choice, team_selection::TeamChoice::Auto)
            && before.red_count == before.blue_count
            && !before.rules.attack_defend
            && !before.rules.mann_vs_machine
            && !(before.rules.highlander && before.teams_full)
        {
            self.authority_random
                .random_int(0, 1)
                .expect("auto-assign random interval is valid")
                != 0
        } else {
            false
        };
        let selected = self.team_selection.select(choice, random)?;
        if let Some(team) = selected {
            if team != before.local_team
                && let Some(spy) = self.spy.as_mut()
            {
                *spy = spy::SpyState::default();
                for condition in [
                    Condition::Stealthed,
                    Condition::StealthedBlink,
                    Condition::Disguised,
                    Condition::Disguising,
                    Condition::DisguiseWearingOff,
                ] {
                    self.conditions.remove(condition);
                }
                self.secondary_was_held = false;
            }
            if team.is_gameplay() && team != before.local_team {
                if let Some(spawn) = self.map.team_spawn(team) {
                    self.spawn = spawn.position;
                    self.spawn_angles = spawn.angles;
                }
                self.movement = MovementState::from_player(
                    Player {
                        position: self.spawn,
                        velocity: [0.0; 3],
                        grounded: false,
                        crouched: false,
                        jump_latched: false,
                    },
                    MovementPolicy {
                        class: self.class,
                        modifiers: self.movement_modifiers,
                    }
                    .resolve(),
                );
                self.snap_spawn_angles();
                self.buildings.reset();
                self.ammo = self.class.data().maximum_ammo;
                self.health = self.maximum_health();
                self.air_dashes = 0;
                self.in_water = false;
                self.last_movement = None;
                self.fire_was_held = false;
            }
            self.lifecycle = if team == PlayerTeam::Spectator {
                PlayerLifecycle::Observer
            } else if team == PlayerTeam::Unassigned {
                PlayerLifecycle::Welcome
            } else {
                PlayerLifecycle::Active
            };
            if !team.is_gameplay() {
                self.weapon = None;
                self.loadout.clear();
                self.health = 0;
            } else if self.weapon.is_none() {
                self.weapon = default_weapon(self.class);
                self.apply_equipment();
                self.ammo = self.maximum_ammo();
                self.health = self.maximum_health();
                self.deploy_active_weapon();
            }
            self.pending_team_change = Some(team);
            self.damagers = deathnotice::DamagerHistory::default();
        }
        Ok(selected)
    }

    pub fn set_initial_view_angles(&mut self, angles: [f32; 3]) {
        self.spawn_angles = angles;
        self.raw_view_angles = angles;
        self.view_angle_offset = [0.0; 3];
        self.movement.local_angles = angles;
        self.movement.absolute_view_angles = angles;
    }

    fn snap_spawn_angles(&mut self) {
        self.snap_view_angles(self.spawn_angles);
    }

    fn snap_view_angles(&mut self, angles: [f32; 3]) {
        self.view_angle_offset = std::array::from_fn(|axis| angles[axis] - self.raw_view_angles[axis]);
        self.movement.local_angles = angles;
        self.movement.absolute_view_angles = angles;
    }

    fn select_respawn_transform(&mut self) {
        let selected = self.bots.as_mut().and_then(|bots| bots.select_spawn(self.team_selection.local_team(), self.class, &mut self.authority_random))
            .or_else(|| self.map.team_spawn(self.team_selection.local_team()));
        if let Some(spawn) = selected {
            self.spawn = spawn.position;
            self.spawn_angles = spawn.angles;
        }
    }

    pub fn new_with_random_seeds(
        collision: W,
        spawn: [f32; 3],
        map: MapRuntime,
        seeds: RandomSeeds,
    ) -> Result<Self, Error> {
        let mut session = Self::new(collision, spawn, map);
        session.set_random_seeds(seeds)?;
        Ok(session)
    }

    pub fn set_random_seeds(&mut self, seeds: RandomSeeds) -> Result<(), Error> {
        let authority = UniformRandomStream::from_seed(seeds.authority)?;
        let predicted_presentation = UniformRandomStream::from_seed(seeds.predicted_presentation)?;
        self.authority_random = authority;
        self.predicted_presentation_random = predicted_presentation;
        self.sound_selection = SoundSelection::new();
        self.random_draws.clear();
        self.audio_events.clear();
        Ok(())
    }

    pub fn random_state(&self) -> Tf2RandomState {
        Tf2RandomState {
            authority: self.authority_random.state(),
            predicted_presentation: self.predicted_presentation_random.state(),
            sound_selection: self.sound_selection.state(),
        }
    }

    pub fn restore_random_state(&mut self, state: Tf2RandomState) -> Result<(), Error> {
        let mut sound_selection = self.sound_selection;
        if !sound_selection.restore(state.sound_selection) {
            return Err(Error::Random(RandomError::InvalidState));
        }
        let authority = UniformRandomStream::from_state(state.authority)?;
        let predicted_presentation = UniformRandomStream::from_state(state.predicted_presentation)?;
        self.authority_random = authority;
        self.predicted_presentation_random = predicted_presentation;
        self.sound_selection = sound_selection;
        self.random_draws.clear();
        self.audio_events.clear();
        Ok(())
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

    pub fn set_flip_viewmodels(&mut self, enabled: bool) {
        self.flip_viewmodels = enabled;
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

    pub fn physics_requests(&self) -> &[ProjectilePhysicsRequest] {
        &self.physics_requests
    }

    pub fn activity_events(&self) -> &[ActivityEvent] {
        &self.activity_events
    }

    pub fn random_draws(&self) -> &[RandomDraw] {
        &self.random_draws
    }

    pub fn audio_events(&self) -> &[AudioEvent] {
        &self.audio_events
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

    pub fn regenerate_model_events(&self) -> &[RegenerateModelEvent] {
        &self.regenerate_model_events
    }

    pub fn regenerate_model_animation(
        &self,
        associated_model: u32,
    ) -> Option<RegenerateModelAnimation> {
        self.regenerate_model_states.get(&associated_model).copied()
    }

    pub fn regenerate_contacts(&self) -> &[RegenerateContact] {
        &self.regenerate_contacts
    }

    pub fn map_effects(&self) -> &[MapEffect] {
        &self.map_effects
    }

    pub fn entity_revision(&self) -> u64 {
        self.map.entity_revision()
    }

    pub fn entity_presentation(
        &self,
        expected: PresentationRevision,
    ) -> Result<EntityPresentationSnapshot, PresentationError> {
        let collision_revision = self
            .collision
            .collision_snapshot_revision()
            .ok_or(PresentationError::CollisionRevisionUnavailable)?;
        if collision_revision != expected.collision {
            return Err(PresentationError::CollisionRevisionMismatch {
                expected: expected.collision,
                actual: collision_revision,
            });
        }
        let entities = self
            .map
            .brush_model_presentation(expected.entity)
            .map_err(PresentationError::Entity)?;
        let studio_models = self
            .map
            .studio_model_presentation(expected.entity)
            .map_err(PresentationError::Entity)?;
        let studio_animations = self.map.studio_animation_presentation();
        let final_collision_revision = self
            .collision
            .collision_snapshot_revision()
            .ok_or(PresentationError::CollisionRevisionUnavailable)?;
        if final_collision_revision != expected.collision {
            return Err(PresentationError::CollisionRevisionMismatch {
                expected: expected.collision,
                actual: final_collision_revision,
            });
        }
        Ok(EntityPresentationSnapshot {
            collision_revision,
            entities,
            studio_models,
            studio_animations,
        })
    }

    pub fn player_flags(&self) -> u32 {
        let mut flags = FL_CLIENT;
        if self.movement.ground.is_some() {
            flags |= FL_ONGROUND;
        }
        if self.movement.crouch.uses_crouched_hull() {
            flags |= FL_DUCKING;
        }
        if matches!(
            self.movement.crouch.phase,
            playsrc_movement::CrouchPhase::Ducking
                | playsrc_movement::CrouchPhase::Crouched
                | playsrc_movement::CrouchPhase::Blocked
        ) {
            flags |= FL_ANIMDUCKING;
        }
        if self.in_water {
            flags |= FL_INWATER;
        }
        flags
    }

    pub fn producer_snapshot(&self) -> ProducerSnapshot {
        ProducerSnapshot {
            tick: self.tick,
            lifecycle: self.lifecycle,
            class: self.class,
            team: self.team_selection.local_team(),
            active_weapon: self.weapon,
            player_flags: self.player_flags(),
            health: self.health,
            maximum_health: self.maximum_health(),
            conditions: self.conditions.words(),
            weapons: self.loadout.values().copied().collect(),
            flame_points: self.flames.points().to_vec(),
            shotgun_pellets: self.shotgun_pellets.clone(),
            flame_firing: self.flame_firing,
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
        candidate.mover_requests.retain(|request| {
            !results.iter().any(|result| {
                result.request_id == request.request_id && result.kind == MoverResultKind::Completed
            })
        });
        merge_mover_requests(&mut candidate.mover_requests, &phase.mover_requests);
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
        merge_mover_requests(&mut candidate.mover_requests, &phase.mover_requests);
        *self = candidate;
        Ok(phase)
    }

    pub fn configure_navigation(
        &mut self,
        mesh: playsrc_nav::Mesh,
        graph: &playsrc_entity::Graph,
    ) -> Result<(), Error> {
        self.bots = Some(
            bot::BotWorld::new(
                mesh,
                graph,
                &self.collision,
                self.movement_configuration.tick_interval,
                self.map.objectives(),
            )
            .map_err(Error::Bot)?,
        );
        if let Some(points) = self.map.control_points() {
            self.bots.as_mut().unwrap().configure_control_points(points).map_err(Error::Bot)?;
        }
        Ok(())
    }

    pub fn fire_entity_input(&mut self, target: &[u8], input: &[u8], value: &[u8], delay: f32) -> Result<(), Error> {
        let phase = self.map.fire_input(self.tick, target, input, value, delay)?;
        merge_mover_requests(&mut self.mover_requests, &phase.mover_requests);
        Ok(())
    }

    pub fn bot_world(&self) -> Option<&bot::BotWorld> {
        self.bots.as_ref()
    }

    pub fn set_posed_player_hitboxes(&mut self, hitboxes: Vec<PosedPlayerHitbox>) {
        self.posed_player_hitboxes = hitboxes;
    }

    pub fn medigun(&self) -> &medic::MedigunState {
        &self.medigun
    }

    pub fn configure_jump(&mut self, definition: jump::CourseDefinition) -> Result<(), Error> {
        if !matches!(self.class, PlayerClass::Soldier | PlayerClass::Demoman) {
            return Err(Error::UnsupportedJumpClass(self.class));
        }
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

    pub fn set_position(&mut self, position: [f32; 3]) -> Result<(), Error> {
        if position.into_iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidPlayerPosition);
        }
        self.movement.position = position.map(|value| value.clamp(-16_384.0, 16_384.0));
        Ok(())
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

    pub fn payload_constraint_blocked(&self) -> bool {
        self.map.payload_constraint_blocked()
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn advance(&mut self, command: Command) -> Result<Snapshot, Error> {
        self.advance_with_external(command, &[], &[], None)
    }

    pub fn advance_with_external(
        &mut self,
        command: Command,
        physics_results: &[ProjectilePhysicsResult],
        rocket_results: &[RocketTraceResult],
        expected_sticky_random: Option<StickyLaunchRandom>,
    ) -> Result<Snapshot, Error> {
        let (candidate, snapshot) = self.clone().into_advanced(
            command,
            physics_results,
            rocket_results,
            expected_sticky_random,
        )?;
        *self = candidate;
        Ok(snapshot)
    }

    /// Advance an already-owned transaction candidate. On error the candidate is
    /// discarded; callers retaining a live session use `advance_with_external`.
    pub fn into_advanced(
        mut self,
        command: Command,
        physics_results: &[ProjectilePhysicsResult],
        rocket_results: &[RocketTraceResult],
        expected_sticky_random: Option<StickyLaunchRandom>,
    ) -> Result<(Self, Snapshot), Error> {
        let snapshot = self.advance_inner(
            command,
            physics_results,
            rocket_results,
            expected_sticky_random,
        )?;
        Ok((self, snapshot))
    }

    fn advance_inner(
        &mut self,
        mut command: Command,
        physics_results: &[ProjectilePhysicsResult],
        rocket_results: &[RocketTraceResult],
        expected_sticky_random: Option<StickyLaunchRandom>,
    ) -> Result<Snapshot, Error> {
        self.raw_view_angles = [command.pitch_degrees, command.movement.yaw_degrees, 0.0];
        admission_metrics::begin_tick(self.tick);
        self.map.set_control_point_facts(self.control_point_facts());
        match self.movement.water_level {
            0 => self.in_water = false,
            1 | 2 => self.in_water = true,
            _ => {}
        }

        if self
            .ctf_capture_bonus_until
            .is_some_and(|deadline| self.tick >= deadline)
        {
            let condition = usize::from(ctf::CRIT_BOOSTED_CTF_CAPTURE);
            self.conditions.words[condition / 32] &= !(1_u32 << (condition % 32));
            self.ctf_capture_bonus_until = None;
        }

        let expected_physics_results: Vec<_> = self
            .physics_requests
            .iter()
            .copied()
            .filter(|request| {
                matches!(
                    request.operation,
                    ProjectilePhysicsOperation::Create | ProjectilePhysicsOperation::Step
                )
            })
            .collect();
        let expected_rocket_results = self.rocket_trace_requests.clone();
        self.random_draws.clear();
        self.audio_events.clear();
        self.shotgun_pellets.clear();
        self.physics_requests.clear();
        self.activity_events.clear();
        self.lifecycle_events.clear();
        self.regenerate_animation_events.clear();
        self.regenerate_model_events.clear();
        self.regenerate_contacts.clear();
        self.map_effects.clear();
        self.radius_damage_requests.clear();
        self.rocket_trace_requests.clear();
        self.contact_reconcile_requests.clear();
        let mut events = Vec::new();
        let mut projectile_events = Vec::new();
        let mut objective_events = Vec::new();
        if self.pending_team_change.is_some() {
            objective_events.extend(self.drop_objective(false)?);
        }
        if let Some(team) = self.pending_team_change.take() {
            self.stop_flame(false);
            self.fizzle_projectiles(&mut projectile_events);
            self.lifecycle_events.push(LifecycleEvent {
                tick: self.tick,
                kind: LifecycleEventKind::TeamChanged,
                class: self.class,
                team,
            });
            events.push(Event::TeamChanged(team));
        }
        if command.drop_item
            || command
                .select_team
                .is_some_and(|team| team != self.team_selection.local_team())
            || command
                .select_class
                .is_some_and(|class| class != self.class)
            || command.select_random_class
        {
            objective_events.extend(self.drop_objective(command.drop_item)?);
        }
        self.apply_projectile_physics(
            &expected_physics_results,
            physics_results,
            &mut projectile_events,
        )?;
        let mut map_phase = self.apply_rocket_traces(
            &expected_rocket_results,
            rocket_results,
            &mut projectile_events,
            &mut events,
        )?;
        self.emit_due_regenerate_model_closes();
        self.apply_selection(command, &mut events, &mut projectile_events);
        if std::mem::take(&mut self.equipment_respawn_requested)
            && self.lifecycle == PlayerLifecycle::Active && self.health > 0
            && !self.equipment.class_matches(&self.active_equipment, self.class)
            && self.round.loadout_respawn_allowed(self.team_selection.local_team())
        {
            let hull = self.movement.active_hull(MovementPolicy { class: self.class, modifiers: self.movement_modifiers }.resolve());
            let center = add(self.movement.position, scale(add(hull.mins, hull.maxs), 0.5));
            if self.map.point_in_friendly_respawn_room(&self.collision, self.team_selection.local_team(), center)? {
                self.respawn(&mut projectile_events, &mut events, MovementPolicy { class: self.class, modifiers: self.movement_modifiers }.resolve());
            }
        }
        if let Some(request) = command.building_request {
            self.buildings.request(request, self.class, self.ammo.metal);
            if matches!(request, building::Request::Build(_))
                && self.buildings.placement().is_some()
            {
                self.weapon = Some(Weapon::Toolbox);
                self.loadout
                    .entry(Weapon::Toolbox)
                    .or_insert_with(|| WeaponRuntime::full(Weapon::Toolbox));
                self.deploy_active_weapon();
            }
        }
        if let Some(settings) = command.objective_configuration {
            let objectives = self
                .map
                .objectives_mut()
                .ok_or(Error::Objectives(ctf::Error::InvalidConfiguration))?;
            let mut configuration = objectives.configuration();
            configuration.captures_per_round = settings.captures_per_round;
            configuration.return_on_touch = settings.return_on_touch;
            objectives
                .configure(configuration)
                .map_err(Error::Objectives)?;
        }
        if let Some(configuration) = command.bot_configuration {
            self.bots
                .as_mut()
                .ok_or(Error::Bot(bot::Error::MissingScenario))?
                .configure(configuration)
                .map_err(Error::Bot)?;
        }
        let mut roster_changed = false;
        if let Some(request) = command.bot_request {
            let bots = self
                .bots
                .as_mut()
                .ok_or(Error::Bot(bot::Error::MissingScenario))?;
            let previous = bots.len();
            bots.apply(
                request,
                self.team_selection.local_team(),
                self.class,
                &mut self.authority_random,
            )
            .map_err(Error::Bot)?;
            let current = bots.len();
            bots.forced_change(
                current.saturating_sub(previous),
                previous.saturating_sub(current),
                self.tick,
            );
            roster_changed = current != previous;
        }
        if let Some(control) = command.bot_control {
            match control {
                bot::Control::StealthCondition { identity, enabled } => {
                    let now = self.tick as f32 * self.movement_configuration.tick_interval;
                    if let Some((origin, enabled)) = self
                        .bots
                        .as_mut()
                        .ok_or(Error::Bot(bot::Error::MissingScenario))?
                        .stealth_condition(identity, enabled, now)
                        .map_err(Error::Bot)?
                    {
                        self.emit_entity_weapon_sound(
                            if enabled {
                                SoundDefinition::SpyCloak
                            } else {
                                SoundDefinition::SpyUncloak
                            },
                            origin,
                            identity,
                        );
                    }
                }
                bot::Control::Teleport {
                    identity,
                    position,
                    pitch_degrees,
                    yaw_degrees,
                } => self
                    .bots
                    .as_mut()
                    .ok_or(Error::Bot(bot::Error::MissingScenario))?
                    .teleport(identity, position, pitch_degrees, yaw_degrees)
                    .map_err(Error::Bot)?,
                bot::Control::Whack { identity } => {
                    let weapon = self.weapon.ok_or(Error::Bot(bot::Error::InvalidEntity))?;
                    self.apply_actor_damage(
                        bot::Damage {
                            attacker: PLAYER_IDENTITY,
                            victim: identity,
                            weapon,
                            amount: 1000.0,
                            position: self.movement.position,
                        },
                        self.team_selection.local_team(),
                        &mut events,
                    )?;
                }
            }
        }
        if let Some(bots) = self.bots.as_mut() {
            if bots.configuration().is_some() {
                let scores = self.round.snapshot(Vec::new());
                roster_changed |= bots
                    .maintain_quota(
                        self.tick,
                        self.team_selection.local_team(),
                        self.class,
                        scores.red_score,
                        scores.blue_score,
                        &mut self.authority_random,
                    )
                    .map_err(Error::Bot)?;
            }
        }
        if roster_changed {
            let bots = self.bots.as_ref().expect("changed TF2 bot roster");
            let mut roster = vec![team_selection::RosterPlayer {
                identity: PLAYER_IDENTITY,
                team: self.team_selection.local_team(),
            }];
            roster.extend(bots.roster());
            self.team_selection
                .replace_roster(roster)
                .map_err(Error::TeamSelection)?;
            admission_metrics::emit(admission_metrics::ROSTER, 0);
        }
        let roster = self.team_selection.snapshot();
        let mut round_facts = round::Facts {
            red_players: roster.red_count,
            blue_players: roster.blue_count,
            red_alive: usize::from(
                self.lifecycle == PlayerLifecycle::Active
                    && self.health > 0
                    && roster.local_team == PlayerTeam::Red,
            ),
            blue_alive: usize::from(
                self.lifecycle == PlayerLifecycle::Active
                    && self.health > 0
                    && roster.local_team == PlayerTeam::Blue,
            ),
            objective_contested: self.map.control_points().is_some_and(control_point::World::contested),
            flag_away_from_home: self.map.objectives().is_some_and(|objectives| {
                objectives
                    .flags()
                    .any(|flag| flag.status != ctf::FlagStatus::Home)
            }),
        };
        if let Some(bots) = &self.bots {
            for bot in bots.combat_targets() {
                if bot.team == PlayerTeam::Red {
                    round_facts.red_alive += 1;
                } else {
                    round_facts.blue_alive += 1;
                }
            }
        }
        self.map.apply_round_inputs(&mut self.round, self.tick as f32 * self.movement_configuration.tick_interval);
        let mut round_events = self
            .round
            .advance(
                self.tick as f32 * self.movement_configuration.tick_interval,
                self.movement_configuration.tick_interval,
                round_facts,
            )
            .map_err(Error::Round)?;
        let round_phase = self.map.emit_round_outputs(self.tick, &round_events)?;
        map_phase.append(round_phase);
        for event in &round_events {
            let sound = match *event {
                round::Event::TimerWarning { seconds, .. } => match seconds {
                    60 => Some(SoundDefinition::RoundEnds60), 30 => Some(SoundDefinition::RoundEnds30),
                    10 => Some(SoundDefinition::RoundEnds10), 5 => Some(SoundDefinition::RoundEnds5),
                    4 => Some(SoundDefinition::RoundEnds4), 3 => Some(SoundDefinition::RoundEnds3),
                    2 => Some(SoundDefinition::RoundEnds2), 1 => Some(SoundDefinition::RoundEnds1), _ => None,
                },
                round::Event::OvertimeChanged { active: true } => Some(SoundDefinition::Overtime),
                _ => None,
            };
            if let Some(sound) = sound { self.emit_objective_sound(PLAYER_IDENTITY, sound, self.movement.position); }
        }
        let round_winner = round_events.iter().find_map(|event| match event {
            round::Event::RoundWon { team, .. } => Some(*team),
            _ => None,
        });
        if let Some(team) = round_winner {
            self.enter_bonus_round(team);
            let definition = if team == PlayerTeam::Unassigned { SoundDefinition::Stalemate } else if team == self.team_selection.local_team() {
                SoundDefinition::TeamWon
            } else {
                SoundDefinition::TeamLost
            };
            self.emit_objective_sound(PLAYER_IDENTITY, definition, self.movement.position);
        }
        let reset_round = round_events
            .iter()
            .any(|event| matches!(event, round::Event::RoundRespawn));
        if reset_round {
            self.restrictions.team_win = None;
            if self.map.control_points().is_some() {
                map_phase.append(self.map.restart_control_point_map(self.tick)?);
                self.round.recreate_map_entities(&self.map.round_configuration());
                self.buildings.reset();
                self.mover_requests.clear();
                self.respawn_touch_count = 0;
            }
            let mut point_events = Vec::new();
            let facts = self.control_point_facts();
            if let Some(points) = self.map.control_points_mut() {
                points.reset(self.tick as f32 * self.movement_configuration.tick_interval, &mut point_events);
            }
            let phase = self.map.emit_control_point_outputs(self.tick, &point_events)?;
            map_phase.append(phase);
            map_phase.control_point_events.extend(point_events);
            if self.map.control_points().is_some() {
                map_phase.append(self.map.activate_control_point_round(self.tick, facts)?);
                if let Some(bots) = &mut self.bots { bots.control_point_round_spawn(self.map.control_points().unwrap(), self.tick as f32 * self.movement_configuration.tick_interval); }
            }
            let scores = self.round.snapshot(Vec::new());
            if let Some(objectives) = self.map.objectives_mut() {
                objectives.reset_round(scores.red_score, scores.blue_score);
            }
            if self.team_selection.local_team().is_gameplay()
                && matches!(self.lifecycle, PlayerLifecycle::Active | PlayerLifecycle::Dying)
            {
                self.respawn(&mut projectile_events, &mut events, MovementPolicy {
                    class: self.class, modifiers: self.movement_modifiers,
                }.resolve());
            }
            if let Some(bots) = &mut self.bots {
                if let Some(points) = self.map.control_points() { bots.synchronize_control_point_spawns(points); }
                bots.round_respawn(self.tick, &mut self.authority_random)
                    .map_err(Error::Bot)?;
            }
        }
        if let Some(objectives) = self.map.objectives_mut() {
            let mut configuration = objectives.configuration();
            let waiting = self.round.waiting_for_players();
            if configuration.waiting_for_players != waiting {
                configuration.waiting_for_players = waiting;
                objectives
                    .configure(configuration)
                    .map_err(Error::Objectives)?;
            }
        }
        command.pitch_degrees += self.view_angle_offset[0];
        command.movement.yaw_degrees += self.view_angle_offset[1];
        self.advance_sniper_scope(command);
        self.advance_spy(command);
        let mut movement_policy = MovementPolicy {
            class: self.class,
            modifiers: self.movement_modifiers,
        }
        .resolve();

        if self.conditions.contains(Condition::Aiming) {
            if self.class == PlayerClass::Heavy {
                movement_policy.maximum_speed = movement_policy.maximum_speed.min(110.0);
                movement_policy.allow_jump = false;
            } else if self.class == PlayerClass::Sniper {
                movement_policy.maximum_speed = movement_policy.maximum_speed.min(80.0);
            }
        }
        if self.air_dashes != 0 {
            movement_policy.air_dash_impulse = None;
        }
        if let Some(winner) = self.restrictions.team_win.filter(|team| team.is_gameplay()) {
            movement_policy.maximum_speed *= if winner == self.team_selection.local_team() { 1.1 } else { 0.9 };
        }
        let hull = self.movement.active_hull(movement_policy);
        let begin_phase = self.map.begin_tick(
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
        let materialized = begin_phase
            .events
            .iter()
            .filter(|event| {
                event.kind == EntityEventKind::Input
                    && event.accepted
                    && event.name == b"__pickup_materialize"
            })
            .filter_map(|event| {
                self.map
                    .pickups()
                    .into_iter()
                    .find(|pickup| pickup.identity == event.entity && pickup.available)
                    .map(|pickup| (pickup.identity, pickup.origin))
            })
            .collect::<Vec<_>>();
        map_phase.append(begin_phase);
        for (identity, position) in materialized {
            self.emit_item_sound(
                SoundDefinition::ItemMaterialize,
                identity,
                position,
                PLAYER_IDENTITY,
            );
        }
        self.movement.position = add(self.movement.position, map_phase.carry);
        let airborne_before_movement = self.movement.ground.is_none();

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
        if self.movement.ground.is_some() {
            self.air_dashes = 0;
        } else if airborne_before_movement
            && self.class == PlayerClass::Scout
            && movement_result
                .events
                .contains(&playsrc_movement::Event::Jumped)
        {
            self.air_dashes = self.air_dashes.saturating_add(1);
        }
        self.last_movement = Some(movement_result);
        let supplies = if self.bots.as_ref().is_some_and(|bots| !bots.is_empty()) {
            self.map.supply_targets()
        } else {
            Vec::new()
        };
        if self.class == PlayerClass::Engineer {
            self.buildings.update_placement(
                &self.collision,
                self.movement.position,
                self.movement.view_offset[2],
                command.movement.yaw_degrees,
            )?;
            if let Some(placement) = self.buildings.placement()
                && placement.valid
                && !self.map.building_position_allowed(
                    &self.collision,
                    placement.object,
                    self.team_selection.local_team(),
                    placement.position,
                )?
            {
                self.buildings.invalidate_placement();
            }
            if command.fire
                && self.weapon == Some(Weapon::Toolbox)
                && self.buildings.confirm(
                    self.tick,
                    PLAYER_IDENTITY,
                    self.team_selection.local_team(),
                    &mut self.ammo.metal,
                )
            {
                self.weapon = Some(Weapon::Wrench);
                self.deploy_active_weapon();
                self.activity_events.push(ActivityEvent {
                    tick: self.tick,
                    weapon: Weapon::Wrench,
                    activity: weapon::WeaponActivity::Draw,
                });
                events.push(Event::WeaponChanged(Weapon::Wrench));
            }
        }
        let bot_attacks = if command.nextbot_stop {
            Vec::new()
        } else if let Some(bots) = &mut self.bots {
            bots.advance(
                &self.collision,
                self.tick,
                bot::Human {
                    team: self.team_selection.local_team(),
                    class: self.class,
                    alive: self.lifecycle == PlayerLifecycle::Active,
                    position: self.movement.position,
                    velocity: self.movement.velocity,
                },
                &supplies,
                &mut self.authority_random,
                Some(bot::Objectives {
                    rules: &self.round,
                    flags: self.map.objectives(),
                    points: self.map.control_points(),
                    in_setup: self.round.snapshot(Vec::new()).in_setup,
                    in_overtime: self.round.snapshot(Vec::new()).in_overtime,
                    time_left: self.round.bot_capture_time_left(),
                }),
            )
            .map_err(Error::Bot)?
        } else {
            Vec::new()
        };
        for attack in bot_attacks {
            self.execute_bot_attack(attack, &mut projectile_events, &mut events)?;
        }

        let bot_snapshots = self
            .bots
            .as_ref()
            .map_or_else(Vec::new, bot::BotWorld::snapshots);
        let building_effects = self.buildings.advance(
            &self.collision,
            self.tick,
            self.team_selection.local_team(),
            self.movement.position,
            self.health,
            self.maximum_health(),
            &bot_snapshots,
            &mut self.ammo.metal,
        )?;
        if building_effects.healing > 0 && self.health < self.maximum_health() {
            self.health = (self.health + building_effects.healing).min(self.maximum_health());
        }
        if let Some((target, damage, level)) = building_effects.sentry_target {
            self.apply_actor_damage_from(
                bot::Damage {
                    attacker: PLAYER_IDENTITY,
                    victim: target,
                    weapon: Weapon::EngineerPistol,
                    amount: damage as f32,
                    position: self.movement.position,
                },
                self.team_selection.local_team(),
                Some(match level { 2 => "obj_sentrygun2", 3 => "obj_sentrygun3", _ => "obj_sentrygun" }),
                Some(0),
                &mut events,
            )?;
        }
        let mut teleported = false;
        if let Some((position, _yaw)) = building_effects.teleport {
            self.movement.position = position;
            self.movement.ground = None;
            teleported = true;
        }
        let bot_contacts = self.bots.as_ref().map_or_else(Vec::new, |bots| {
            bots.contact_actors(self.restrictions.team_win).collect()
        });
        let phase = self.map.contact_phase(
            &self.collision,
            self.tick,
            self.movement.position,
            self.movement.active_hull(movement_policy),
            map_runtime::PlayerContactFacts {
                team: self.team_selection.local_team().source_number(),
                class: self.class.source_number(),
                observer: self.lifecycle != PlayerLifecycle::Active,
                conditions: self.conditions.words(),
                winning_team: self.restrictions.team_win.map(PlayerTeam::source_number),
            },
            &bot_contacts,
        )?;
        let discontinuity = self.apply_map_effects(&phase, &mut events, &mut teleported);
        let jump_contacts = phase.contacts.clone();
        map_phase.append(phase);
        if self.map.objectives().is_some() {
            let mut actor = ctf::Actor::active(
                PLAYER_IDENTITY,
                self.team_selection.local_team(),
                self.movement.position,
                self.movement.active_hull(movement_policy),
            );
            actor.alive = self.lifecycle == PlayerLifecycle::Active && self.health > 0;
            actor.invulnerable = [5_u8, 8, 51, 52, 57]
                .into_iter()
                .any(|condition| self.condition_word_contains(condition));
            actor.stealthed = self.condition_word_contains(4)
                || self.condition_word_contains(9)
                || self.condition_word_contains(64);
            actor.selected_to_teleport = self.condition_word_contains(10);
            actor.phased = self.condition_word_contains(14);
            actor.in_respawn_room = self.respawn_touch_count != 0;
            let mut actors = vec![actor];
            if let Some(bots) = &self.bots {
                actors.extend(bots.snapshots().into_iter().map(|bot| {
                    let mut actor = ctf::Actor::active(
                        bot.identity,
                        bot.team,
                        bot.position,
                        MovementPolicy {
                            class: bot.class,
                            modifiers: MovementModifiers::default(),
                        }
                        .resolve()
                        .standing_hull,
                    );
                    actor.alive = bot.lifecycle == PlayerLifecycle::Active && bot.health > 0;
                    actor
                }));
            }
            let current = self
                .map
                .objectives_mut()
                .expect("known objective world")
                .advance(
                    &self.collision,
                    self.tick as f32 * self.movement_configuration.tick_interval,
                    &actors,
                )
                .map_err(Error::Objectives)?;
            if let Some(bots) = &mut self.bots {
                bots.synchronize_objectives(
                    self.map.objectives().expect("known objective world"),
                    &current,
                );
            }
            objective_events.extend(current);
        }
        self.apply_pickup_contacts(movement_policy, &mut events, &mut map_phase)?;
        if self.map.control_points().is_some() {
            let facts = self.control_point_facts();
            self.map.set_control_point_facts(facts);
            let mut actor = control_point::Actor::active(PLAYER_IDENTITY, self.team_selection.local_team(), self.class, self.movement.position, self.movement.active_hull(movement_policy));
            actor.alive = self.lifecycle == PlayerLifecycle::Active && self.health > 0;
            actor.invulnerable = [5_u8, 51, 52, 57].into_iter().any(|c| self.condition_word_contains(c));
            actor.stealthed = [4_u8, 64, 66].into_iter().any(|c| self.condition_word_contains(c));
            actor.control_stunned = self.conditions.is_control_stunned();
            actor.megaheal = self.condition_word_contains(28);
            actor.phased = self.condition_word_contains(14);
            actor.enemy_disguise = self.spy.and_then(|s| s.disguise).is_some_and(|d| d.team != actor.team);
            let base_capture_value = actor.capture_value(self.map.control_points().unwrap().configuration().scales_with_players);
            actor.capture_value_bonus = self.equipped_player_attribute(PLAYER_IDENTITY, "add_player_capturevalue", base_capture_value as f32) as i32 - base_capture_value;
            let mut actors = vec![actor];
            if let Some(bots) = &self.bots {
                actors.extend(bots.control_point_actors());
            }
            let mut point_events = Vec::new();
            self.map.control_points_mut().unwrap().step(self.tick as f32 * self.movement_configuration.tick_interval, facts, &actors, &self.collision, &mut point_events).map_err(Error::ControlPoints)?;
            let phase = self.map.emit_control_point_outputs(self.tick, &point_events)?;
            map_phase.append(phase);
            map_phase.control_point_events.extend(point_events);
            if let Some(bots) = &mut self.bots { bots.synchronize_control_point_spawns(self.map.control_points().unwrap()); }
        }
        if self.weapon != Some(Weapon::Flamethrower)
            || self.lifecycle != PlayerLifecycle::Active
            || self.health <= 0
        {
            self.stop_flame(false);
        }
        if discontinuity {
            self.contact_reconcile_requests
                .push(ContactReconcileRequest {
                    tick: self.tick,
                    position: self.movement.position,
                    hull: self.movement.active_hull(movement_policy),
                });
        }

        let mut ammo_events = Vec::new();
        if self.lifecycle == PlayerLifecycle::Active
            && self.health > 0
            && let Some(active_weapon) = self.weapon
        {
            let now = self.tick as f32 * self.movement_configuration.tick_interval;
            let attack_allowed = self.spy.is_none_or(|state| state.can_attack(now)) && self.restrictions.team_win.is_none_or(|winner| winner == self.team_selection.local_team());
            let released_primary = !command.fire && self.fire_was_held;
            let previous_minigun_state = self.loadout[&active_weapon].minigun_state;
            let primary = if active_weapon == Weapon::Flamethrower {
                self.advance_flamethrower(Command { fire: command.fire && attack_allowed, detonate: command.detonate && attack_allowed, ..command }, &mut ammo_events, &mut events)?;
                PrimaryResult::None
            } else {
                let state = self
                    .loadout
                    .get_mut(&active_weapon)
                    .expect("active weapon belongs to loadout");
                state.attack(
                    self.tick,
                    self.movement_configuration.tick_interval,
                    command.fire && attack_allowed,
                    command.detonate
                        && self.spy.is_none()
                        && active_weapon != Weapon::StickybombLauncher,
                    released_primary,
                    &mut self.activity_events,
                )
            };
            if active_weapon == Weapon::Minigun {
                let state = self.loadout[&active_weapon].minigun_state;
                if state == weapon::MinigunState::Idle {
                    self.conditions.remove(Condition::Aiming);
                } else {
                    self.conditions.insert(Condition::Aiming);
                }
                if state != previous_minigun_state {
                    let definition = match state {
                        weapon::MinigunState::Idle => Some(SoundDefinition::MinigunWindDown),
                        weapon::MinigunState::Starting => Some(SoundDefinition::MinigunWindUp),
                        weapon::MinigunState::Firing => Some(SoundDefinition::MinigunFire),
                        weapon::MinigunState::Spinning => Some(SoundDefinition::MinigunSpin),
                        weapon::MinigunState::DryFire => None,
                    };
                    if let Some(definition) = definition {
                        self.emit_weapon_sound(definition, self.movement.position);
                    }
                }
            }
            if let PrimaryResult::Fired { charge_seconds } = primary {
                self.fire_on_empty = false;
                if ballistics::HitscanProfile::configured(active_weapon).is_some() {
                    self.fire_hitscan(
                        active_weapon,
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                        &mut events,
                    )?;
                } else if active_weapon == Weapon::Minigun {
                    let phase = self.fire_minigun_hitscan(
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                        &mut events,
                    )?;
                    map_phase.append(phase);
                } else if matches!(
                    active_weapon,
                    Weapon::Bat
                        | Weapon::Shovel
                        | Weapon::Kukri
                        | Weapon::Wrench
                        | Weapon::FireAxe
                        | Weapon::Bottle
                        | Weapon::Bonesaw
                ) {
                    self.swing_melee(active_weapon);
                } else if active_weapon == Weapon::Knife {
                    self.resolve_knife(
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                        &mut events,
                    )?;
                } else if active_weapon == Weapon::Fists {
                    self.emit_weapon_sound(SoundDefinition::FistMiss, self.movement.position);
                } else if !matches!(
                    active_weapon,
                    Weapon::BuildPda | Weapon::DestroyPda | Weapon::Toolbox
                ) {
                    self.fire_projectile(
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                        charge_seconds,
                        expected_sticky_random,
                        None,
                        &mut projectile_events,
                    )?;
                }
            }
            if active_weapon == Weapon::Fists
                && self.loadout[&active_weapon]
                    .smack_due_tick
                    .is_some_and(|due| self.tick > due)
            {
                self.loadout.get_mut(&active_weapon).unwrap().smack_due_tick = None;
                let phase =
                    self.smack_fists(command.pitch_degrees, command.movement.yaw_degrees)?;
                map_phase.append(phase);
            }
            if self.pending_melee_tick.is_some_and(|due| self.tick > due) {
                self.pending_melee_tick = None;

                if matches!(
                    active_weapon,
                    Weapon::Bat
                        | Weapon::Shovel
                        | Weapon::Kukri
                        | Weapon::Wrench
                        | Weapon::FireAxe
                        | Weapon::Bottle
                        | Weapon::Bonesaw
                ) {
                    self.resolve_melee(
                        active_weapon,
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                        &mut events,
                    )?;
                }
            }
            let mut empty_reload = false;
            if matches!(
                active_weapon,
                Weapon::EngineerShotgun | Weapon::EngineerPistol
            ) && command.fire
            {
                let state = self.loadout[&active_weapon];
                if state.clip == 0
                    && state.reload == ReloadPhase::Ready
                    && self.tick >= state.next_primary_tick
                {
                    if self.fire_on_empty {
                        self.fire_on_empty = false;
                        empty_reload = state.reserve > 0;
                    } else {
                        self.fire_on_empty = true;
                        self.emit_weapon_sound(
                            if active_weapon == Weapon::EngineerShotgun {
                                SoundDefinition::ShotgunEmpty
                            } else {
                                SoundDefinition::PistolEmpty
                            },
                            self.movement.position,
                        );
                    }
                }
            }
            let reload_activity_start = self.activity_events.len();
            {
                let state = self
                    .loadout
                    .get_mut(&active_weapon)
                    .expect("active weapon belongs to loadout");
                if !self.spy.is_some_and(|spy| spy.cloaked)
                    && (command.reload
                        || empty_reload
                        || (self.auto_reload && !command.fire && !command.detonate))
                {
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
            let reload_sound = self.activity_events[reload_activity_start..]
                .iter()
                .find_map(|event| match (active_weapon, event.activity) {
                    (Weapon::Scattergun, weapon::WeaponActivity::ReloadLoop) => {
                        Some(SoundDefinition::ScattergunReload)
                    }
                    (
                        Weapon::Pistol | Weapon::EngineerPistol,
                        weapon::WeaponActivity::ReloadStart,
                    ) => Some(SoundDefinition::PistolReload),
                    (
                        Weapon::Shotgun | Weapon::HeavyShotgun | Weapon::EngineerShotgun,
                        weapon::WeaponActivity::ReloadLoop,
                    ) => Some(SoundDefinition::ShotgunReload),
                    (Weapon::Smg, weapon::WeaponActivity::ReloadStart) => {
                        Some(SoundDefinition::SmgReload)
                    }

                    (Weapon::Revolver, weapon::WeaponActivity::ReloadStart) => {
                        Some(SoundDefinition::RevolverReload)
                    }
                    (Weapon::SyringeGun, weapon::WeaponActivity::ReloadLoop) => {
                        Some(SoundDefinition::SyringeReload)
                    }
                    _ => None,
                });
            if let Some(definition) = reload_sound {
                self.emit_weapon_sound(definition, self.movement.position);
            }
            if active_weapon == Weapon::MediGun {
                self.advance_medigun(command)?;
            } else if self.medigun.target.is_some() || self.medigun.releasing {
                self.advance_medigun(Command {
                    fire: false,
                    detonate: false,
                    ..command
                })?;
            }
            self.fire_was_held = command.fire
                && (active_weapon != Weapon::Flamethrower
                    || (self.movement.water_level != 3 && !command.detonate));
        } else {
            self.fire_was_held = false;
        }
        if let Some(bots) = &mut self.bots {
            bots.advance_health(self.tick as f32 * self.movement_configuration.tick_interval)
                .map_err(Error::Bot)?;
        }
        for event in ammo_events {
            events.push(Event::Reloaded {
                weapon: event.weapon,
                clip: event.clip,
                reserve: event.reserve,
            });
        }
        self.advance_projectiles(
            self.movement_configuration.tick_interval,
            &mut projectile_events,
            &mut events,
        )?;
        if command.detonate
            && self.weapon != Some(Weapon::Flamethrower)
            && self.spy.is_none()
            && self.lifecycle == PlayerLifecycle::Active
            && self.health > 0
        {
            self.detonate(&mut projectile_events, &mut events)?;
        }

        if self.health <= 0 && self.lifecycle == PlayerLifecycle::Active {
            objective_events.extend(self.drop_objective(false)?);
            self.die(&mut projectile_events);
        }
        let mut respawned = false;
        let eligible = self.lifecycle == PlayerLifecycle::Dying && self.bots.is_some() && self.death_tick.is_some_and(|death| self.round.player_can_respawn(self.team_selection.local_team(), death as f32 * self.movement_configuration.tick_interval));
        if self.restrictions.team_win.is_none() && ((command.respawn
            && (self.lifecycle == PlayerLifecycle::Active
                || eligible
                || self.jump.is_some()
                || self.bots.is_none()))
            || eligible)
        {
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
                        PlayerClass::Soldier => jump::Class::Soldier,
                        PlayerClass::Demoman => jump::Class::Demoman,
                        _ => return Err(Error::UnsupportedJumpClass(self.class)),
                    },
                    team: match self.team_selection.local_team() {
                        PlayerTeam::Unassigned => jump::Team::Unassigned,
                        PlayerTeam::Spectator => jump::Team::Spectator,
                        PlayerTeam::Red => jump::Team::Red,
                        PlayerTeam::Blue => jump::Team::Blue,
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

        if !objective_events.is_empty() {
            let phase = self
                .map
                .emit_objective_outputs(self.tick, &objective_events)?;
            map_phase.append(phase);
            for event in &objective_events {
                if let ctf::Event::Captured { player, .. } = event {
                    self.round.record_capture(&[*player]);
                    if *player == PLAYER_IDENTITY { self.scoreboard.local_capture(); }
                }
                if let ctf::Event::CaptureBonus {
                    team,
                    condition,
                    duration,
                } = event
                    && *team == self.team_selection.local_team()
                    && self.lifecycle == PlayerLifecycle::Active
                {
                    let index = usize::from(*condition);
                    self.conditions.words[index / 32] |= 1_u32 << (index % 32);
                    self.ctf_capture_bonus_until = Some(
                        self.tick
                            + (*duration / self.movement_configuration.tick_interval).ceil() as u64,
                    );
                }
                if let ctf::Event::RoundWon { team, reason, .. } = event {
                    round_events.extend(self.round.win(*team, *reason).map_err(Error::Round)?);
                    self.enter_bonus_round(*team);
                    let scores = self.round.snapshot(Vec::new());
                    if let Some(objectives) = self.map.objectives_mut() {
                        objectives.set_round_scores(scores.red_score, scores.blue_score);
                    }
                }
            }
            let sounds = objective_events
                .iter()
                .filter_map(|event| match event {
                    ctf::Event::Announcer {
                        flag,
                        recipient,
                        sound,
                        exclude_player,
                    } if (*recipient == self.team_selection.local_team()
                        || *recipient == PlayerTeam::Unassigned)
                        && *exclude_player != Some(PLAYER_IDENTITY)
                        && *sound != ctf::AnnouncerSound::FlagSpawn =>
                    {
                        Some((
                            *flag,
                            match sound {
                                ctf::AnnouncerSound::EnemyStolen => {
                                    SoundDefinition::FlagEnemyStolen
                                }
                                ctf::AnnouncerSound::EnemyDropped => {
                                    SoundDefinition::FlagEnemyDropped
                                }
                                ctf::AnnouncerSound::EnemyCaptured => {
                                    SoundDefinition::FlagEnemyCaptured
                                }
                                ctf::AnnouncerSound::EnemyReturned => {
                                    SoundDefinition::FlagEnemyReturned
                                }
                                ctf::AnnouncerSound::TeamStolen => SoundDefinition::FlagTeamStolen,
                                ctf::AnnouncerSound::TeamDropped => {
                                    SoundDefinition::FlagTeamDropped
                                }
                                ctf::AnnouncerSound::TeamCaptured => {
                                    SoundDefinition::FlagTeamCaptured
                                }
                                ctf::AnnouncerSound::TeamReturned => {
                                    SoundDefinition::FlagTeamReturned
                                }
                                ctf::AnnouncerSound::FlagSpawn => SoundDefinition::FlagSpawn,
                            },
                        ))
                    }
                    ctf::Event::RoundWon { team, .. } => Some((
                        PLAYER_IDENTITY,
                        if *team == self.team_selection.local_team() {
                            SoundDefinition::TeamWon
                        } else {
                            SoundDefinition::TeamLost
                        },
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (identity, definition) in sounds {
                let position = self
                    .map
                    .objectives()
                    .and_then(|objectives| objectives.flag(identity))
                    .map_or(self.movement.position, |flag| flag.position);
                self.emit_objective_sound(identity, definition, position);
            }
        }
        if let Some(points) = self.map.control_points() {
            let team = |identity| if identity == PLAYER_IDENTITY { self.team_selection.local_team() } else { self.bots.as_ref().and_then(|bots| bots.team(identity)).unwrap_or(PlayerTeam::Unassigned) };
            for event in &events {
                if let Event::PlayerKilled { attacker, victim, .. } = event { points.check_death_causes_block(*victim, team(*victim), *attacker, team(*attacker), &mut map_phase.control_point_events); }
            }
        }
        for event in &map_phase.control_point_events {
            match event {
                control_point::Event::TeamScore { team } => self.round.add_team_score(*team),
                control_point::Event::Sound { point, recipient, definition, action } => {
                    if recipient.is_none_or(|team| team == PlayerTeam::Unassigned || team == self.team_selection.local_team()) {
                        let point = &self.map.control_points().unwrap().points()[*point];
                        let (identity, position) = (point.identity, point.position);
                        let samples = if *action == AudioAction::Stop { SoundSamples { volume: 0.0, pitch: 0.0, wave: 0, sound_level: 0.0 } }
                            else { self.sample_sound(if recipient.is_some() { RandomContext::PredictedPresentation } else { RandomContext::Authority }, *definition, SoundQueryPhase::Emit) };
                        self.push_audio_event(AudioEvent { action: *action, tick: self.tick, ordinal: 0, identity: AudioEventIdentity::WeaponSingle, definition: *definition, source_kind: if recipient.is_some() { AudioSourceKind::LocalListener } else { AudioSourceKind::ControlPoint }, source_identity: identity, owner_identity: None, position, samples });
                    }
                }
                control_point::Event::RespawnWaveAdjustment { team, seconds } => self.round.add_respawn_wave(*team, *seconds as f32),
                control_point::Event::Captured { point, cappers, .. } => {
                    if !cappers.is_empty() && self.map.control_points().is_some_and(|world| !world.points()[*point].print_name.is_empty()) {
                        self.round.record_capture(cappers);
                        if cappers.contains(&PLAYER_IDENTITY) { self.scoreboard.local_capture(); }
                        if let Some(bots) = &mut self.bots { bots.record_point_capture(cappers); }
                    }
                },
                control_point::Event::Blocked { point, player, .. } if self.map.control_points().is_some_and(|world| !world.points()[*point].print_name.is_empty()) => {
                    if *player == PLAYER_IDENTITY { self.scoreboard.local_defense(); }
                    else if let Some(bots) = &mut self.bots { bots.record_point_defense(*player); }
                }
                control_point::Event::RoundWon { team, reason, .. } => {
                    round_events.extend(if *team == PlayerTeam::Unassigned { self.round.set_stalemate() } else { self.round.win(*team, *reason).map_err(Error::Round)? });
                    self.enter_bonus_round(*team);
                    let definition = if *team == PlayerTeam::Unassigned { SoundDefinition::Stalemate } else if *team == self.team_selection.local_team() { SoundDefinition::TeamWon } else { SoundDefinition::TeamLost };
                    self.emit_objective_sound(PLAYER_IDENTITY, definition, self.movement.position);
                }
                _ => {}
            }
        }
        self.map.apply_round_inputs(&mut self.round, self.tick as f32 * self.movement_configuration.tick_interval);
        let input_events = self.round.take_events();
        for event in &input_events {
            if let round::Event::RoundWon { team, .. } = event {
                self.enter_bonus_round(*team);
                self.emit_objective_sound(PLAYER_IDENTITY, if *team == PlayerTeam::Unassigned { SoundDefinition::Stalemate } else if *team == self.team_selection.local_team() { SoundDefinition::TeamWon } else { SoundDefinition::TeamLost }, self.movement.position);
            }
        }
        map_phase.append(self.map.emit_round_outputs(self.tick, &input_events)?);
        round_events.extend(input_events);
        if let Some(bots) = &mut self.bots { bots.synchronize_respawn_times(&self.round); }
        if let (Some(bots), Some(points)) = (&mut self.bots, self.map.control_points()) { bots.control_point_events(points, &map_phase.control_point_events, self.tick as f32 * self.movement_configuration.tick_interval); }
        if self.bots.is_some() {
            if self.lifecycle != PlayerLifecycle::Dying { self.death_tick = None; }
            self.respawn_tick = self.death_tick.and_then(|death| self.round.player_respawn_time(self.team_selection.local_team(), death as f32 * self.movement_configuration.tick_interval)).map(|time| (time / self.movement_configuration.tick_interval).ceil().max(0.0) as u64);
        }
        if let Some(bots) = &mut self.bots {
            let fired = self.activity_events.iter().rev().find(|event| matches!(event.activity, weapon::WeaponActivity::PrimaryAttack | weapon::WeaponActivity::SecondaryAttack)).map(|event| (event.weapon, self.movement.position));
            bots.record_point_combat(self.tick, fired);
        }
        self.tick += 1;
        merge_mover_requests(&mut self.mover_requests, &map_phase.mover_requests);
        self.map_effects = map_phase.effects.clone();
        self.regenerate_contacts = map_phase.regenerate_contacts.clone();
        let bots = self
            .bots
            .as_ref()
            .map_or_else(Vec::new, bot::BotWorld::snapshots);
        let round = self.round.snapshot(round_events);
        let scoreboard = self.scoreboard.snapshot(
            self.team_selection.local_team(),
            self.class,
            self.lifecycle,
            &bots,
            self.map.objectives().map(ctf::World::scores),
            (round.red_score, round.blue_score),
        );
        Ok(Snapshot {
            equipped_items: self.active_equipment.equipped_items(self.class),
            tick: self.tick,
            class: self.class,
            team: self.team_selection.local_team(),
            weapon: self.weapon,
            player_flags: self.player_flags(),
            movement: self.movement,
            view_angle_offset: self.view_angle_offset,
            health: self.health as f32,
            maximum_health: self.maximum_health() as f32,
            spy: self.spy,
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
            objectives: self
                .map
                .objectives()
                .map(|objectives| objectives.snapshot(objective_events)),
            control_points: self.map.control_points().map(|points| points.snapshot(map_phase.control_point_events)),
            round,
            jump: jump_output,
            events,
            bots: bots.into_iter().map(|mut bot| {
                if let Some(equipment) = self.bot_equipment.get(&bot.identity) {
                    bot.equipped_items = equipment.equipped_items(bot.class).into_iter().filter(|item|
                        equipment::supported_item(item.definition_index).is_some_and(|item| item.implementation == equipment::Implementation::Wearable)).collect();
                }
                bot
            }).collect(),
            pickups: self.map.pickups(),
            metal: self.ammo.metal,
            scoreboard,
            buildings: self.buildings.snapshots(),
            placement: self.buildings.placement(),
            medigun_charge: self.medigun.charge,
            medigun_target: self.medigun.target,
            medigun_releasing: self.medigun.releasing,
        })
    }

    fn condition_word_contains(&self, condition: u8) -> bool {
        let index = usize::from(condition);
        self.conditions.words[index / 32] & (1_u32 << (index % 32)) != 0
    }

    fn enter_bonus_round(&mut self, winner: PlayerTeam) {
        if self.restrictions.team_win == Some(winner) { return; }
        self.restrictions.team_win = Some(winner);
        if winner.is_gameplay() && winner == self.team_selection.local_team() && self.lifecycle == PlayerLifecycle::Active && self.health > 0 {
            self.conditions.words[1] |= 1 << (38 - 32);
        }
        if let Some(bots) = &mut self.bots { bots.set_round_winner(winner, self.round.bonus_round_seconds() + 1.0); }
    }

    fn control_point_facts(&self) -> control_point::Facts {
        let round = self.round.snapshot(Vec::new());
        control_point::Facts {
            points_may_be_captured: matches!(round.state, round::State::Running | round::State::Stalemate) && !round.waiting_for_players,
            round_running: round.state == round::State::Running,
            waiting_for_players: round.waiting_for_players,
            in_overtime: round.in_overtime,
            koth_timer_remaining: round.koth_timers.map(|timers| timers.map(|timer| timer.remaining)),
            timer_may_expire: self.round.timer_may_expire(),
        }
    }

    fn drop_objective(&mut self, thrown: bool) -> Result<Vec<ctf::Event>, Error> {
        let Some(flag_team) = self
            .map
            .objectives()
            .and_then(|objectives| objectives.carrier_flag(PLAYER_IDENTITY))
            .map(|flag| flag.team)
        else {
            return Ok(Vec::new());
        };
        let carrier_team = if flag_team == PlayerTeam::Red {
            PlayerTeam::Blue
        } else {
            PlayerTeam::Red
        };
        let hull = self.movement.active_hull(
            MovementPolicy {
                class: self.class,
                modifiers: self.movement_modifiers,
            }
            .resolve(),
        );
        let center = [
            self.movement.position[0],
            self.movement.position[1],
            self.movement.position[2] + (hull.mins[2] + hull.maxs[2]) * 0.5,
        ];
        let end = [center[0], center[1], center[2] - 8_000.0];
        let trace = self
            .collision
            .trace(center, end, ctf::FLAG_COLLISION_HULL, MASK_SOLID)?;
        let position = if trace.start_solid { center } else { trace.end };
        self.map
            .objectives_mut()
            .expect("known carried objective")
            .drop(
                ctf::Actor::active(PLAYER_IDENTITY, carrier_team, self.movement.position, hull),
                position,
                thrown,
                false,
            )
            .map_err(Error::Objectives)
    }

    fn apply_pickup_contacts(
        &mut self,
        policy: GenericMovementPolicy,
        events: &mut Vec<Event>,
        phase: &mut MapPhase,
    ) -> Result<(), Error> {
        if self.lifecycle == PlayerLifecycle::Active && self.health > 0 {
            for state in self.loadout.values() {
                if let Some(kind) = weapon_ammo_kind(state.weapon) {
                    self.ammo.set(kind, state.reserve);
                }
            }
            let candidates = self.map.pickup_candidates(
                &self.collision,
                self.movement.position,
                self.movement.active_hull(policy),
                self.class.standing_eye_height(),
            )?;
            for candidate in candidates {
                phase.append(self.map.begin_pickup(
                    self.tick,
                    candidate.identity,
                    PLAYER_IDENTITY,
                )?);
                let team_eligible = candidate
                    .team
                    .is_none_or(|team| team == self.team_selection.local_team().source_number());
                let amount = if !team_eligible {
                    None
                } else {
                    match candidate.definition.kind {
                        pickup::MapPickupKind::Health => {
                            let maximum = self.maximum_health();
                            let requested =
                                (maximum as f32 * candidate.definition.size.ratio()).ceil() as i32;
                            let before = self.health;
                            if self.health < maximum {
                                self.health = self.health.saturating_add(requested).min(maximum);
                            }
                            let cleansed =
                                [Condition::Burning, Condition::Bleeding, Condition::Plague]
                                    .into_iter()
                                    .fold(false, |cleansed, condition| {
                                        if self.conditions.contains(condition) {
                                            self.conditions.remove(condition);
                                            true
                                        } else {
                                            cleansed
                                        }
                                    });
                            let given = (self.health - before) as u16;
                            (given > 0 || cleansed).then_some(given)
                        }
                        pickup::MapPickupKind::Ammo => {
                            let mut grants = Vec::new();
                            pickup::grant_map_ammo(
                                self.class,
                                self.maximum_ammo(),
                                candidate.definition.size,
                                &mut self.ammo,
                                &mut grants,
                            );
                            if grants.is_empty() {
                                None
                            } else {
                                for state in self.loadout.values_mut() {
                                    if let Some(kind) = weapon_ammo_kind(state.weapon) {
                                        state.reserve = self
                                            .ammo
                                            .get(kind)
                                            .min(state.profile().maximum_reserve);
                                    }
                                }
                                Some(
                                    grants
                                        .iter()
                                        .filter_map(|grant| match grant {
                                            pickup::PickupGrant::Ammo { amount, .. } => {
                                                Some(*amount)
                                            }
                                            pickup::PickupGrant::Health(_) => None,
                                        })
                                        .sum(),
                                )
                            }
                        }
                    }
                };
                phase.append(self.map.finish_pickup(
                    self.tick,
                    candidate.identity,
                    PLAYER_IDENTITY,
                    amount.is_some(),
                )?);
                if let Some(amount) = amount {
                    let active = self.weapon.and_then(|weapon| self.loadout.get(&weapon));
                    events.push(Event::PickedUp {
                        entity: candidate.identity,
                        player: PLAYER_IDENTITY,
                        kind: candidate.definition.kind,
                        size: candidate.definition.size,
                        amount,
                        health: self.health,
                        weapon: self.weapon,
                        clip: active.map_or(0, |state| state.clip),
                        reserve: active.map_or(0, |state| state.reserve),
                    });
                    self.emit_item_sound(
                        match candidate.definition.kind {
                            pickup::MapPickupKind::Health => SoundDefinition::HealthKitTouch,
                            pickup::MapPickupKind::Ammo => SoundDefinition::AmmoPackTouch,
                        },
                        candidate.identity,
                        candidate.origin,
                        PLAYER_IDENTITY,
                    );
                }
            }
        }

        let bots = self
            .bots
            .as_ref()
            .map_or_else(Vec::new, bot::BotWorld::snapshots);
        for bot in bots {
            if bot.lifecycle != PlayerLifecycle::Active {
                continue;
            }
            let policy = MovementPolicy {
                class: bot.class,
                modifiers: MovementModifiers::default(),
            }
            .resolve();
            let hull = policy.standing_hull;
            for zone in
                self.map
                    .bot_regenerate_zones(&self.collision, bot.position, hull, bot.team)?
            {
                if self
                    .bots
                    .as_mut()
                    .is_some_and(|world| world.regenerate(bot.identity, self.tick))
                {
                    let _ = zone;
                }
            }
            let candidates = self.map.pickup_candidates(
                &self.collision,
                bot.position,
                hull,
                bot.class.standing_eye_height(),
            )?;
            for candidate in candidates {
                phase.append(
                    self.map
                        .begin_pickup(self.tick, candidate.identity, bot.identity)?,
                );
                let amount = if candidate
                    .team
                    .is_some_and(|team| team != bot.team.source_number())
                {
                    None
                } else {
                    self.bots
                        .as_mut()
                        .and_then(|world| world.grant_pickup(bot.identity, candidate.definition))
                };
                phase.append(self.map.finish_pickup(
                    self.tick,
                    candidate.identity,
                    bot.identity,
                    amount.is_some(),
                )?);
                if let Some(amount) = amount {
                    let current = self
                        .bots
                        .as_ref()
                        .and_then(|world| {
                            world
                                .snapshots()
                                .into_iter()
                                .find(|value| value.identity == bot.identity)
                        })
                        .expect("authoritative bot remains present");
                    events.push(Event::PickedUp {
                        entity: candidate.identity,
                        player: bot.identity,
                        kind: candidate.definition.kind,
                        size: candidate.definition.size,
                        amount,
                        health: current.health,
                        weapon: current.weapon.map(|value| value.weapon),
                        clip: current.weapon.map_or(0, |value| value.clip),
                        reserve: current.weapon.map_or(0, |value| value.reserve),
                    });
                }
            }
        }
        Ok(())
    }

    fn advance_spy(&mut self, command: Command) {
        let policy = MovementPolicy {
            class: self.class,
            modifiers: MovementModifiers::default(),
        }
        .resolve();
        let hull = if self.movement.crouch.uses_crouched_hull() {
            policy.crouched_hull
        } else {
            policy.standing_hull
        };
        if let Some(bots) = self.bots.as_mut() {
            let human = bot::Human {
                team: self.team_selection.local_team(),
                class: self.class,
                alive: self.health > 0,
                position: self.movement.position,
                velocity: self.movement.velocity,
            };
            let decloaks = bots.advance_spies(self.tick, human, hull);
            for (identity, origin) in decloaks {
                self.emit_entity_weapon_sound(SoundDefinition::SpyUncloak, origin, identity);
            }
        }
        if self.lifecycle != PlayerLifecycle::Active || self.health <= 0 {
            self.secondary_was_held = command.detonate;
            return;
        }
        let Some(state) = self.spy.as_mut() else {
            self.secondary_was_held = command.detonate;
            return;
        };
        let now = self.tick as f32 * self.movement_configuration.tick_interval;
        if self.bots.as_ref().is_some_and(|bots| {
            bots.touches_enemy(
                self.team_selection.local_team(),
                self.movement.position,
                hull,
            )
        }) {
            state.expose(now);
        }
        let mut notifications = [None; 4];
        if command.detonate && !self.secondary_was_held {
            notifications[0] = state.toggle_cloak(now);
        }
        self.secondary_was_held = command.detonate;
        if let Some(disguise) = command.disguise
            && !self.restrictions.taunting
        {
            notifications[1] =
                state.request_disguise(self.team_selection.local_team(), disguise, now);
        }
        [notifications[2], notifications[3]] =
            state.advance(now, self.movement_configuration.tick_interval);
        state.reveal_invisibility(now, self.conditions.contains(Condition::Urine));
        if state.blink(now) {
            self.conditions.insert(Condition::StealthedBlink);
        } else {
            self.conditions.remove(Condition::StealthedBlink);
        }
        if state.cloaked {
            self.conditions.insert(Condition::Stealthed);
        } else {
            self.conditions.remove(Condition::Stealthed);
        }
        if state.desired_disguise.is_some() {
            self.conditions.insert(Condition::Disguising);
        } else {
            self.conditions.remove(Condition::Disguising);
        }
        if state.disguise.is_some() {
            self.conditions.insert(Condition::Disguised);
        } else {
            self.conditions.remove(Condition::Disguised);
        }
        if state.disguise_wear_off_until > now {
            self.conditions.insert(Condition::DisguiseWearingOff);
        } else {
            self.conditions.remove(Condition::DisguiseWearingOff);
        }
        for event in notifications.into_iter().flatten() {
            match event {
                spy::SpyEvent::Cloaked => {
                    self.emit_weapon_sound(SoundDefinition::SpyCloak, self.movement.position);
                }
                spy::SpyEvent::Uncloaked => {
                    self.emit_weapon_sound(SoundDefinition::SpyUncloak, self.movement.position);
                }
                _ => {}
            }
        }
    }

    fn remove_spy_disguise(&mut self) {
        if let Some(state) = self.spy.as_mut() {
            let now = self.tick as f32 * self.movement_configuration.tick_interval;
            if state.remove_disguise(now).is_some() {
                self.conditions.remove(Condition::Disguised);
                self.conditions.remove(Condition::Disguising);
                self.conditions.insert(Condition::DisguiseWearingOff);
            }
        }
    }

    fn apply_selection(
        &mut self,
        command: Command,
        events: &mut Vec<Event>,
        projectile_events: &mut Vec<ProjectileEvent>,
    ) {
        if let Some(team) = command.select_team
            && team.is_gameplay()
            && team != self.team_selection.local_team()
            && self
                .select_team_choice(
                    if team == PlayerTeam::Red {
                        team_selection::TeamChoice::Red
                    } else {
                        team_selection::TeamChoice::Blue
                    },
                )
                .is_ok()
        {
            self.pending_team_change = None;
            self.stop_flame(false);
            self.buildings.reset();
            self.fizzle_projectiles(projectile_events);
            self.lifecycle_events.push(LifecycleEvent {
                tick: self.tick,
                kind: LifecycleEventKind::TeamChanged,
                class: self.class,
                team,
            });
            events.push(Event::TeamChanged(team));
        }
        let selected_class = if command.select_random_class {
            let choices = PlayerClass::ALL
                .into_iter()
                .filter(|class| {
                    *class != self.class
                        && (self.jump.is_none()
                            || matches!(class, PlayerClass::Soldier | PlayerClass::Demoman))
                })
                .collect::<Vec<_>>();
            if choices.is_empty() {
                None
            } else {
                let index = self.draw_random_int(
                    RandomContext::Authority,
                    RandomDecision::ClassSelection,
                    0,
                    choices.len() as i32 - 1,
                ) as usize;
                Some(choices[index])
            }
        } else {
            command.select_class
        };
        if let Some(class) = selected_class
            && class != self.class
            && (self.jump.is_none() || matches!(class, PlayerClass::Soldier | PlayerClass::Demoman))
        {
            self.class = class;
            self.spy = (class == PlayerClass::Spy).then(spy::SpyState::default);
            self.buildings.reset();
            self.weapon = default_weapon(class);
            self.apply_equipment();
            self.ammo = self.maximum_ammo();
            self.deploy_active_weapon();
            self.health = self.maximum_health();
            self.conditions.clear();
            self.ctf_capture_bonus_until = None;
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
            self.snap_spawn_angles();
            self.air_dashes = 0;
            self.in_water = false;
            self.last_movement = None;
            self.lifecycle_events.extend([
                LifecycleEvent {
                    tick: self.tick,
                    kind: LifecycleEventKind::ClassChanged,
                    class: self.class,
                    team: self.team_selection.local_team(),
                },
                LifecycleEvent {
                    tick: self.tick,
                    kind: LifecycleEventKind::Respawned,
                    class: self.class,
                    team: self.team_selection.local_team(),
                },
            ]);
            if let Some(weapon) = self.weapon {
                self.activity_events.push(ActivityEvent {
                    tick: self.tick,
                    weapon,
                    activity: weapon::WeaponActivity::Draw,
                });
            }
            events.push(Event::ClassChanged(class));
            if let Some(weapon) = self.weapon {
                events.push(Event::WeaponChanged(weapon));
            }
            events.push(Event::Respawned);
        }
        if let Some(weapon) = command.select_weapon
            && self.loadout.contains_key(&weapon)
            && Some(weapon) != self.weapon
        {
            if let Some(active_weapon) = self.weapon
                && let Some(previous) = self.loadout.get_mut(&active_weapon)
            {
                previous.charge_begin_tick = None;
                previous.abort_reload();
                if matches!(
                    active_weapon,
                    Weapon::Bat
                        | Weapon::Shovel
                        | Weapon::Kukri
                        | Weapon::Wrench
                        | Weapon::FireAxe
                        | Weapon::Bottle
                ) {
                    self.pending_melee_tick = None;
                }
            }
            self.weapon = Some(weapon);
            self.fire_on_empty = false;
            self.loadout
                .entry(weapon)
                .or_insert_with(|| WeaponRuntime::full(weapon));
            self.deploy_active_weapon();
            self.activity_events.push(ActivityEvent {
                tick: self.tick,
                weapon,
                activity: weapon::WeaponActivity::Draw,
            });
            events.push(Event::WeaponChanged(weapon));
        }
    }

    fn advance_sniper_scope(&mut self, command: Command) {
        if self.weapon != Some(Weapon::SniperRifle) {
            if self.class == PlayerClass::Sniper {
                self.conditions.remove(Condition::Aiming);
                self.conditions.remove(Condition::Zoomed);
            }
            return;
        }
        let interval = self.movement_configuration.tick_interval;
        let state = self
            .loadout
            .get_mut(&Weapon::SniperRifle)
            .expect("Sniper rifle belongs to loadout");
        if command.movement.jump && self.movement.ground.is_none() {
            self.conditions.remove(Condition::Aiming);
            self.conditions.remove(Condition::Zoomed);
            state.charged_damage = 0.0;
            state.charge_begin_tick = None;
            state.unzoom_due_tick = None;
            state.rezoom_due_tick = None;
            state.rezoom_after_shot = false;
            return;
        }
        if state.unzoom_due_tick.is_some_and(|due| self.tick > due) {
            self.conditions.remove(Condition::Aiming);
            self.conditions.remove(Condition::Zoomed);
            state.charged_damage = 0.0;
            state.charge_begin_tick = None;
            state.unzoom_due_tick = None;
            if state.rezoom_after_shot {
                state.rezoom_due_tick = Some(self.tick + weapon::delay_ticks(0.9, interval));
                state.rezoom_after_shot = false;
            }
        }
        if state.rezoom_due_tick.is_some_and(|due| self.tick > due) && state.reserve > 0 {
            state.rezoom_due_tick = None;
            state.charge_begin_tick = Some(self.tick);
            self.conditions.insert(Condition::Aiming);
            self.conditions.insert(Condition::Zoomed);
        }
        if command.detonate && self.tick >= state.next_secondary_tick {
            if state.rezoom_due_tick.is_some() || state.unzoom_due_tick.is_some() {
                state.next_secondary_tick =
                    state.rezoom_due_tick.unwrap_or(self.tick) + weapon::delay_ticks(0.3, interval);
                state.rezoom_due_tick = None;
            } else if self.conditions.contains(Condition::Zoomed) {
                self.conditions.remove(Condition::Aiming);
                self.conditions.remove(Condition::Zoomed);
                state.charged_damage = 0.0;
                state.charge_begin_tick = None;
                state.next_primary_tick = state
                    .next_primary_tick
                    .max(self.tick + weapon::delay_ticks(0.1, interval));
                state.next_secondary_tick = self.tick + weapon::delay_ticks(0.3, interval);
            } else if state.reserve > 0 && !command.movement.jump {
                self.conditions.insert(Condition::Aiming);
                self.conditions.insert(Condition::Zoomed);
                state.charge_begin_tick = Some(self.tick);
                state.next_primary_tick = state
                    .next_primary_tick
                    .max(self.tick + weapon::delay_ticks(0.1, interval));
                state.next_secondary_tick = self.tick + weapon::delay_ticks(0.3, interval);
            }
        }
        if self.tick >= state.next_secondary_tick {
            if self.conditions.contains(Condition::Aiming) && !state.rezoom_after_shot {
                state.charged_damage = (state.charged_damage + interval * 50.0).min(150.0);
            } else {
                state.charged_damage = (state.charged_damage - interval * 75.0).max(0.0);
            }
        }
    }

    fn deploy_active_weapon(&mut self) {
        let Some(active_weapon) = self.weapon else {
            return;
        };
        let interval = self.movement_configuration.tick_interval;
        let first_primary_tick = {
            let active = self
                .loadout
                .get_mut(&active_weapon)
                .expect("active weapon belongs to loadout");
            active.deploy(self.tick, interval);
            active.first_primary_tick
        };
        for weapon in self.loadout.values_mut() {
            weapon.first_primary_tick = first_primary_tick;
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
                    if let Some(angles) = angles { self.snap_view_angles(angles); }
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
                    damage_type,
                    contact,
                } => match contact {
                    playsrc_entity::ContactKind::Enter => {
                        self.hurt_active.insert(trigger);
                        self.hurt_applied.remove(&trigger);
                        self.apply_hurt_pulse(trigger, damage_per_second, damage_type, events);
                    }
                    playsrc_entity::ContactKind::Stay => {
                        let due = self.hurt_next_tick.get(&trigger).copied().unwrap_or(0);
                        if self.tick >= due {
                            self.apply_hurt_pulse(trigger, damage_per_second, damage_type, events);
                        }
                    }
                    playsrc_entity::ContactKind::Exit => {
                        if self.hurt_active.remove(&trigger) && !self.hurt_applied.remove(&trigger)
                        {
                            self.apply_hurt_pulse(trigger, damage_per_second, damage_type, events);
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
                        Some(winner) => winner == self.team_selection.local_team(),
                        None => team.is_none_or(|team| {
                            team == self.team_selection.local_team().source_number()
                        }),
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

    fn apply_hurt_pulse(&mut self, trigger: u32, damage_per_second: f32, damage_type: u32, events: &mut Vec<Event>) {
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
            let before = self.health;
            self.health = self.health.saturating_sub(points).max(0);
            events.push(Event::Damaged {
                amount: points as f32,
                health: self.health as f32,
                origin: None,
            });
            if before > 0 && self.health == 0 {
                events.push(Event::PlayerKilled { attacker: 0, victim: PLAYER_IDENTITY,
                    weapon: None, killing_weapon: "trigger_hurt", assister: 0, damage_bits: damage_type,
                    critical: false, custom: 0 });
            }
        }
        self.hurt_applied.insert(trigger);
        self.hurt_next_tick.insert(
            trigger,
            self.tick + ticks(0.5, self.movement_configuration.tick_interval),
        );
    }

    fn regenerate(&mut self, entity: u32, associated_model: Option<u32>, events: &mut Vec<Event>) {
        if !self.equipment.class_matches(&self.active_equipment, self.class) { self.apply_equipment(); }
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
        self.ammo = self.maximum_ammo();
        if let Some(state) = self.spy.as_mut() {
            state.cloak_meter = spy::CLOAK_MAXIMUM;
        }
        self.next_regenerate_tick =
            self.tick + ticks(3.0, self.movement_configuration.tick_interval);
        self.emit_item_sound(
            SoundDefinition::RegenerateTouch,
            PLAYER_IDENTITY,
            self.movement.position,
            PLAYER_IDENTITY,
        );
        if let Some(associated_model) = associated_model {
            let body = self
                .map
                .regenerate_associated_body(entity)
                .expect("validated associated dynamic prop body state");
            let close_tick = self.tick + ticks(2.0, self.movement_configuration.tick_interval);
            self.regenerate_animation_events
                .push(RegenerateAnimationEvent {
                    zone: entity,
                    associated_model,
                    open_tick: self.tick,
                    close_tick,
                    body,
                    open_animation: RegenerateModelAnimation::Open,
                    close_animation: RegenerateModelAnimation::Close,
                });
            self.regenerate_model_states
                .insert(associated_model, RegenerateModelAnimation::Open);
            self.regenerate_model_events.push(RegenerateModelEvent {
                zone: entity,
                associated_model,
                tick: self.tick,
                animation: RegenerateModelAnimation::Open,
                body,
            });
            self.pending_regenerate_model_closes
                .push(PendingRegenerateModelClose {
                    zone: entity,
                    associated_model,
                    tick: close_tick,
                    body,
                });
        }
        if let Some(weapon) = self.weapon {
            let active = self.loadout[&weapon];
            events.push(Event::Resupplied {
                entity,
                health: self.health as f32,
                weapon,
                clip: active.clip,
                reserve: active.reserve,
            });
        }
    }

    fn emit_due_regenerate_model_closes(&mut self) {
        let mut pending = Vec::new();
        for close in std::mem::take(&mut self.pending_regenerate_model_closes) {
            if close.tick <= self.tick {
                self.regenerate_model_states
                    .insert(close.associated_model, RegenerateModelAnimation::Close);
                self.regenerate_model_events.push(RegenerateModelEvent {
                    zone: close.zone,
                    associated_model: close.associated_model,
                    tick: self.tick,
                    animation: RegenerateModelAnimation::Close,
                    body: close.body,
                });
            } else {
                pending.push(close);
            }
        }
        self.pending_regenerate_model_closes = pending;
    }

    fn draw_random_float(
        &mut self,
        context: RandomContext,
        decision: RandomDecision,
        minimum: f32,
        maximum: f32,
    ) -> f32 {
        let (stream, draws) = match context {
            RandomContext::Authority => (&mut self.authority_random, &mut self.random_draws),
            RandomContext::PredictedPresentation => (
                &mut self.predicted_presentation_random,
                &mut self.random_draws,
            ),
        };
        let (raw, value) = stream.random_float_observed(minimum, maximum);
        draws.push(RandomDraw {
            context,
            decision,
            raw,
            result: RandomResult::FloatBits(value.to_bits()),
        });
        value
    }

    fn draw_random_int(
        &mut self,
        context: RandomContext,
        decision: RandomDecision,
        minimum: i32,
        maximum: i32,
    ) -> i32 {
        let (stream, draws) = match context {
            RandomContext::Authority => (&mut self.authority_random, &mut self.random_draws),
            RandomContext::PredictedPresentation => (
                &mut self.predicted_presentation_random,
                &mut self.random_draws,
            ),
        };
        stream
            .random_int_observed(minimum, maximum, |raw, accepted, result| {
                draws.push(RandomDraw {
                    context,
                    decision,
                    raw,
                    result: if accepted {
                        RandomResult::Integer(result)
                    } else {
                        RandomResult::RejectedIntegerCandidate
                    },
                });
            })
            .expect("fixed TF2 random integer range is valid")
    }

    fn sample_sound(
        &mut self,
        context: RandomContext,
        definition: SoundDefinition,
        phase: SoundQueryPhase,
    ) -> SoundSamples {
        let volume = self.draw_random_float(
            context,
            RandomDecision::SoundVolume { definition, phase },
            0.0,
            1.0,
        );
        let pitch = self.draw_random_float(
            context,
            RandomDecision::SoundPitch { definition, phase },
            0.0,
            1.0,
        );
        let wave = if definition.wave_count() == 1 {
            0
        } else {
            let available = self.sound_selection.available_count(definition);
            let rank = self.draw_random_int(
                context,
                RandomDecision::SoundWave { definition, phase },
                0,
                i32::from(available) - 1,
            ) as u8;
            self.sound_selection
                .original_ordinal(definition, rank, phase == SoundQueryPhase::Emit)
        };
        let sound_level = self.draw_random_float(
            context,
            RandomDecision::SoundLevel { definition, phase },
            0.0,
            1.0,
        );
        SoundSamples {
            volume,
            pitch,
            wave,
            sound_level,
        }
    }

    fn sample_weapon_sound(&mut self, definition: SoundDefinition) -> SoundSamples {
        self.sample_sound(
            RandomContext::Authority,
            definition,
            SoundQueryPhase::Inspect,
        );
        self.sample_sound(RandomContext::Authority, definition, SoundQueryPhase::Emit);
        self.sample_sound(
            RandomContext::PredictedPresentation,
            definition,
            SoundQueryPhase::Inspect,
        );
        self.sample_sound(
            RandomContext::PredictedPresentation,
            definition,
            SoundQueryPhase::Emit,
        )
    }

    fn push_audio_event(&mut self, mut event: AudioEvent) {
        event.ordinal =
            u16::try_from(self.audio_events.len()).expect("bounded TF2 audio event count");
        self.audio_events.push(event);
    }

    fn emit_item_sound(
        &mut self,
        definition: SoundDefinition,
        source: u32,
        position: [f32; 3],
        owner: u32,
    ) {
        let samples = self.sample_weapon_sound(definition);
        self.push_audio_event(AudioEvent {
            action: AudioAction::Play,
            tick: self.tick,
            ordinal: 0,
            identity: if definition == SoundDefinition::ItemMaterialize {
                AudioEventIdentity::ItemMaterialize
            } else {
                AudioEventIdentity::ItemPickup
            },
            definition,
            source_kind: AudioSourceKind::Entity,
            source_identity: source,
            owner_identity: Some(owner),
            position,
            samples,
        });
    }

    fn emit_weapon_sound(&mut self, definition: SoundDefinition, position: [f32; 3]) {
        self.emit_entity_weapon_sound(definition, position, PLAYER_IDENTITY);
    }

    fn emit_entity_weapon_sound(
        &mut self,
        definition: SoundDefinition,
        position: [f32; 3],
        identity: u32,
    ) {
        let samples = self.sample_weapon_sound(definition);
        self.push_audio_event(AudioEvent {
            action: AudioAction::Play,
            tick: self.tick,
            ordinal: 0,
            identity: AudioEventIdentity::WeaponSingle,
            definition,
            source_kind: AudioSourceKind::Entity,
            source_identity: identity,
            owner_identity: Some(identity),
            position,
            samples,
        });
    }

    fn execute_bot_attack(
        &mut self,
        attack: bot::Attack,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        if !self
            .bots
            .as_ref()
            .is_some_and(|bots| bots.active(attack.attacker))
        {
            return Ok(());
        }
        if matches!(
            attack.weapon,
            Weapon::RocketLauncher | Weapon::Original | Weapon::SyringeGun
        ) {
            return self.fire_projectile(
                attack.pitch_degrees,
                attack.yaw_degrees,
                0.0,
                None,
                Some(attack),
                projectile_events,
            );
        }
        if matches!(
            attack.weapon,
            Weapon::Bat
                | Weapon::Shovel
                | Weapon::Fists
                | Weapon::Kukri
                | Weapon::Wrench
                | Weapon::Bottle
                | Weapon::FireAxe
                | Weapon::Knife
                | Weapon::Bonesaw
        ) {
            let (amount, definition) = match attack.weapon {
                Weapon::Bat => (ballistics::BAT_DAMAGE, SoundDefinition::BatHitFlesh),
                Weapon::Shovel => (ballistics::SHOVEL_DAMAGE, SoundDefinition::ShovelHitFlesh),
                Weapon::Fists => (65.0, SoundDefinition::FistHitFlesh),
                Weapon::Kukri => (ballistics::KUKRI_DAMAGE, SoundDefinition::KukriHitFlesh),
                Weapon::Wrench => (ballistics::WRENCH_DAMAGE, SoundDefinition::WrenchHitFlesh),
                Weapon::Bottle => (ballistics::BOTTLE_DAMAGE, SoundDefinition::BottleHitFlesh),
                Weapon::FireAxe => (pyro::FIRE_AXE_DAMAGE, SoundDefinition::FireAxeHitFlesh),
                Weapon::Knife => (spy::KNIFE_DAMAGE, SoundDefinition::KnifeHitFlesh),
                Weapon::Bonesaw => (
                    medic::BONESAW_DAMAGE as f32,
                    SoundDefinition::BonesawHitFlesh,
                ),
                _ => unreachable!("only melee weapons reach this branch"),
            };
            let position = if attack.target == PLAYER_IDENTITY {
                add(self.movement.position, self.movement.view_offset)
            } else if let Some(target) = self
                .bots
                .as_ref()
                .and_then(|bots| bots.combat_player(attack.target))
            {
                target.world_center
            } else {
                return Ok(());
            };
            events.push(Event::MeleeImpact {
                weapon: attack.weapon,
                owner: attack.attacker,
                target: Some(attack.target),
                position,
                damage: amount,
            });
            self.apply_actor_damage(
                bot::Damage {
                    attacker: attack.attacker,
                    victim: attack.target,
                    weapon: attack.weapon,
                    amount,
                    position: attack.eye_position,
                },
                attack.team,
                events,
            )?;
            self.emit_entity_weapon_sound(definition, attack.position, attack.attacker);
            return Ok(());
        }
        let (profile, sound) = if attack.weapon == Weapon::Minigun {
            (
                ballistics::HitscanProfile {
                    pellets: 4,
                    damage: 9.0 * attack.damage_multiplier,
                    range: 8192.0,
                    spread: 0.08 * attack.spread_multiplier,
                    accurate_after_seconds: f32::MAX,
                },
                SoundDefinition::MinigunFire,
            )
        } else {
            let Some(profile) = ballistics::HitscanProfile::configured(attack.weapon) else {
                return Ok(());
            };
            let sound = match attack.weapon {
                Weapon::Scattergun => SoundDefinition::ScattergunSingle,
                Weapon::Pistol | Weapon::EngineerPistol => SoundDefinition::PistolSingle,
                Weapon::Shotgun | Weapon::HeavyShotgun | Weapon::EngineerShotgun => {
                    SoundDefinition::ShotgunSingle
                }
                Weapon::SniperRifle => SoundDefinition::SniperSingle,
                Weapon::Smg => SoundDefinition::SmgSingle,
                Weapon::Revolver => SoundDefinition::RevolverSingle,
                _ => unreachable!("hitscan profiles own their firing sounds"),
            };
            (profile, sound)
        };
        self.emit_entity_weapon_sound(sound, attack.position, attack.attacker);
        let (forward, right, up) = angle_vectors(attack.pitch_degrees, attack.yaw_degrees, 0.0);
        events.push(Event::HitscanFired {
            weapon: attack.weapon,
            pellets: profile.pellets,
            owner: attack.attacker,
            origin: attack.eye_position,
        });
        let mut damage = BTreeMap::<u32, (f32, [f32; 3])>::new();
        for pellet in 0..profile.pellets {
            let direction = profile.pellet_direction(
                (self.tick as u32).wrapping_add(attack.attacker),
                pellet,
                attack.seconds_since_previous_shot,
                forward,
                right,
                up,
            );
            let end = add(attack.eye_position, scale(direction, profile.range));
            let wall = self.collision.trace(
                attack.eye_position,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                MASK_SOLID,
            )?;
            let mut hit = self.bots.as_ref().and_then(|bots| {
                bots.intersect_enemy(attack.team, attack.eye_position, end, attack.attacker)
            });
            if self.lifecycle == PlayerLifecycle::Active
                && attack.team.is_enemy(self.team_selection.local_team())
                && let Some(fraction) =
                    bot::segment_player(attack.eye_position, end, self.movement.position)
                && hit.is_none_or(|(_, prior, _)| fraction < prior)
            {
                hit = Some((
                    PLAYER_IDENTITY,
                    fraction,
                    add(
                        attack.eye_position,
                        scale(sub(end, attack.eye_position), fraction),
                    ),
                ));
            }
            if let Some((identity, fraction, position)) = hit
                && fraction <= wall.fraction
            {
                let points = profile.damage_at_distance(
                    length(sub(position, attack.eye_position)),
                    attack.weapon == Weapon::Scattergun,
                );
                damage.entry(identity).or_insert((0.0, position)).0 += points;
                events.push(Event::HitscanImpact {
                    weapon: attack.weapon,
                    target: Some(identity),
                    pellet,
                    hitgroup: 0,
                    critical: false,
                    position,
                    damage: points,
                    surface: None,
                });
            } else if wall.fraction < 1.0 || wall.start_solid {
                events.push(Event::HitscanImpact {
                    weapon: attack.weapon,
                    target: None,
                    pellet,
                    hitgroup: 0,
                    critical: false,
                    position: wall.end,
                    damage: profile.damage,
                    surface: self
                        .collision
                        .impact_surface(attack.eye_position, end, MASK_SOLID)?,
                });
            }
        }
        for (victim, (amount, position)) in damage {
            self.apply_actor_damage(
                bot::Damage {
                    attacker: attack.attacker,
                    victim,
                    weapon: attack.weapon,
                    amount,
                    position,
                },
                attack.team,
                events,
            )?;
        }
        Ok(())
    }

    pub fn equipped_weapon_definition(&self, owner: u32, weapon: Weapon) -> Option<u32> {
        if owner == PLAYER_IDENTITY { return self.active_equipment.weapon_definition(self.class, weapon); }
        let class = self.bots.as_ref()?.class(owner)?;
        self.bot_equipment.get(&owner).cloned().unwrap_or_default().weapon_definition(class, weapon)
    }

    fn killing_weapon_name(&self, owner: u32, weapon: Weapon) -> &'static str {
        let class = if owner == PLAYER_IDENTITY { self.class }
            else { self.bots.as_ref().and_then(|bots| bots.class(owner)).unwrap_or(self.class) };
        self.equipped_weapon_definition(owner, weapon).and_then(equipment::presentation).and_then(|item| item.death_notice_icon)
            .unwrap_or_else(|| deathnotice::weapon_name(weapon, class))
    }

    fn apply_actor_damage(
        &mut self,
        input: bot::Damage,
        attacker_team: PlayerTeam,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        self.apply_actor_damage_from(input, attacker_team, None, None, events)
    }

    fn apply_actor_damage_from(
        &mut self,
        input: bot::Damage,
        attacker_team: PlayerTeam,
        killing_weapon: Option<&'static str>,
        custom_kill: Option<u8>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let (critical, custom) = events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::HitscanImpact {
                    weapon,
                    target: Some(target),
                    hitgroup,
                    critical,
                    ..
                } if *weapon == input.weapon && *target == input.victim => {
                    Some((*critical, if *critical && *hitgroup == 1 { 1 } else { 0 }))
                }
                _ => None,
            })
            .unwrap_or((false, 0));
        let before = if input.victim == PLAYER_IDENTITY {
            self.health
        } else {
            self.bots
                .as_ref()
                .and_then(|bots| bots.health(input.victim))
                .unwrap_or(0)
        };
        let (critical, custom) = custom_kill.map_or((critical, custom), |custom| (custom == 2, custom));
        let attacker_conditions = if input.attacker == PLAYER_IDENTITY { self.conditions.runtime_state() }
            else { self.bots.as_ref().and_then(|bots| bots.player_conditions(input.attacker)).cloned().unwrap_or_default() };
        let normalized = bot::Damage { amount: if critical { input.amount / damage::CRIT_MULTIPLIER } else { input.amount }, ..input };
        let requested_crit = if critical { damage::CritKind::Full } else { damage::CritKind::None };
        let critical = critical || attacker_conditions.is_crit_boosted();
        let after;
        if input.victim == PLAYER_IDENTITY {
            if self.lifecycle != PlayerLifecycle::Active
                || !attacker_team.is_enemy(self.team_selection.local_team())
            {
                return Ok(());
            }
            let Some(damage_type) = bot::weapon_damage_type(input.weapon) else {
                return Ok(());
            };
            let mut health = health::HealthState::spawn(self.class, 0.0, 0.0)
                .map_err(|_| Error::Bot(bot::Error::Damage))?;
            health.current = self.health;
            health.maximum = self.maximum_health();
            let mut conditions = condition::ConditionState::default();
            if self.conditions.contains(Condition::Invulnerable) {
                conditions
                    .add(
                        condition::ConditionId::INVULNERABLE,
                        condition::ConditionDuration::Permanent,
                        Some(PLAYER_IDENTITY),
                        true,
                        false,
                    )
                    .map_err(|_| Error::Bot(bot::Error::Damage))?;
            }
            if self.conditions.contains(Condition::Phase) {
                conditions
                    .add(
                        condition::ConditionId::PHASE,
                        condition::ConditionDuration::Permanent,
                        None,
                        true,
                        false,
                    )
                    .map_err(|_| Error::Bot(bot::Error::Damage))?;
            }
            let result = damage::apply_damage(
                true,
                &mut health,
                &mut conditions,
                &damage::DamageInput {
                    attacker: input.attacker,
                    attacker_team,
                    attacker_conditions,
                    source: damage::DamageSourceKind::Player,
                    weapon_position: None,
                    victim: PLAYER_IDENTITY,
                    victim_team: self.team_selection.local_team(),
                    base_damage: normalized.amount,
                    range_multiplier: 1.0,
                    damage_type,
                    custom: if custom == 2 {
                        damage::CustomDamage::Backstab
                    } else if custom == 1 {
                        damage::CustomDamage::Headshot
                    } else {
                        damage::CustomDamage::None
                    },
                    crit: requested_crit,
                    friendly_fire: false,
                    force_friendly_fire: false,
                    bypass_invulnerability: false,
                    force: [0.0; 3],
                },
                damage::DamageModifiers::default(),
            )
            .map_err(|_| Error::Bot(bot::Error::Damage))?;
            if !result.admitted {
                return Ok(());
            }
            if input.attacker != input.victim {
                self.damagers.record(input.attacker, self.tick as f32 * self.movement_configuration.tick_interval);
            }
            self.health = health.current.max(0);
            if let Some(spy) = self.spy.as_mut() {
                spy.note_damage(
                    self.tick as f32 * self.movement_configuration.tick_interval,
                    result.health_damage,
                    self.conditions.contains(Condition::Bleeding),
                );
            }
            after = self.health;
            events.push(Event::Damaged {
                amount: result.health_damage as f32,
                health: after as f32,
                origin: self
                    .bots
                    .as_ref()
                    .and_then(|bots| bots.combat_player(input.attacker))
                    .map(|attacker| attacker.world_center),
            });
            if let Some(bots) = &mut self.bots {
                bots.record_human_hit(
                    input.attacker,
                    u32::try_from((before - after).max(0)).unwrap_or(0),
                    after == 0,
                );
            }
        } else if let Some(bots) = &mut self.bots {
            if bots
                .damage(normalized, attacker_team, self.tick, attacker_conditions, requested_crit)
                .map_err(Error::Bot)?
                .is_none()
            {
                return Ok(());
            }
            after = bots.health(input.victim).unwrap_or(0);
        } else {
            return Ok(());
        }
        let dealt = u32::try_from((before - after).max(0)).unwrap_or(0);
        events.push(Event::PlayerDamaged {
            attacker: input.attacker,
            victim: input.victim,
            weapon: input.weapon,
            amount: dealt,
            health: after,
            critical,
            custom,
        });
        if input.attacker == PLAYER_IDENTITY {
            self.scoreboard.local_damage(dealt);
            if after == 0 {
                self.scoreboard.local_kill();
            }
        }
        if after == 0 {
            let curtime = self.tick as f32 * self.movement_configuration.tick_interval;
            let assister = if input.attacker == input.victim { None }
                else if !killing_weapon.is_some_and(|name| name.starts_with("obj_sentrygun")) && self.class == PlayerClass::Medic
                    && self.medigun.target == Some(input.attacker) { Some(PLAYER_IDENTITY) }
                else if input.victim == PLAYER_IDENTITY { self.damagers.assister(input.attacker, curtime) }
                else { self.bots.as_ref().and_then(|bots| bots.damage_assister(input.victim, input.attacker, curtime)) };
            let assister = assister.filter(|identity| *identity == PLAYER_IDENTITY || self.bots.as_ref().is_some_and(|bots| bots.contains(*identity)));
            if assister == Some(PLAYER_IDENTITY) { self.scoreboard.local_assist(); }
            else if let Some(identity) = assister && let Some(bots) = self.bots.as_mut() { bots.record_assist(identity); }
            events.push(Event::PlayerKilled {
                attacker: input.attacker,
                victim: input.victim,
                weapon: Some(input.weapon),
                killing_weapon: killing_weapon.unwrap_or_else(|| self.killing_weapon_name(input.attacker, input.weapon)),
                assister: assister.unwrap_or(0),
                damage_bits: 0,
                critical,
                custom,
            });
        }
        Ok(())
    }

    fn bot_intersection(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        radius: f32,
        maximum_fraction: f32,
        enemies_only: bool,
    ) -> Option<(bot::CombatTarget, f32, [f32; 3])> {
        let bots = self.bots.as_ref()?;
        bots.combat_targets()
            .filter(|target| !enemies_only || target.team != self.team_selection.local_team())
            .filter_map(|target| {
                let mins = [
                    target.position[0] - 24.0 - radius,
                    target.position[1] - 24.0 - radius,
                    target.position[2] - radius,
                ];
                let maxs = [
                    target.position[0] + 24.0 + radius,
                    target.position[1] + 24.0 + radius,
                    target.position[2] + 82.0 + radius,
                ];
                let fraction = ray_box_fraction(start, end, mins, maxs)?;
                (fraction <= maximum_fraction).then_some((
                    target,
                    fraction,
                    add(start, scale(sub(end, start), fraction)),
                ))
            })
            .min_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| left.0.identity.cmp(&right.0.identity))
            })
    }

    fn stop_flame(&mut self, abrupt: bool) {
        if !self.flame_firing {
            return;
        }
        self.flame_firing = false;
        self.flame_burst_until = 0.0;
        // StopFlame emits the authored winddown, then destroys both sound patches.
        if !abrupt {
            self.emit_weapon_sound(SoundDefinition::FlameEnd, self.movement.position);
        }
        for definition in [SoundDefinition::FlameLoop, SoundDefinition::FlameFire] {
            self.push_audio_event(AudioEvent {
                action: AudioAction::Stop,
                tick: self.tick,
                ordinal: 0,
                identity: AudioEventIdentity::WeaponSingle,
                definition,
                source_kind: AudioSourceKind::Entity,
                source_identity: PLAYER_IDENTITY,
                owner_identity: Some(PLAYER_IDENTITY),
                position: self.movement.position,
                samples: SoundSamples {
                    volume: 0.0,
                    pitch: 0.0,
                    wave: 0,
                    sound_level: 0.0,
                },
            });
        }
    }

    fn advance_flamethrower(
        &mut self,
        command: Command,
        ammo_events: &mut Vec<weapon::AmmoEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let interval = self.movement_configuration.tick_interval;
        let now = self.tick as f32 * interval;
        if !command.fire && now > self.flame_burst_until {
            self.flame_burst_until = 0.0;
        }
        let fire = command.fire || now < self.flame_burst_until;
        if !fire
            || self.loadout[&Weapon::Flamethrower].reserve == 0
            || self.movement.water_level == 3
        {
            self.stop_flame(false);
        }
        if command.detonate
            && self.movement.water_level != 3
            && self.tick >= self.next_airblast_tick
        {
            let state = self
                .loadout
                .get_mut(&Weapon::Flamethrower)
                .expect("Pyro has the stock Flamethrower");
            if state.reserve >= pyro::AIRBLAST_AMMO {
                state.reserve -= pyro::AIRBLAST_AMMO;
                state.next_primary_tick =
                    self.tick + weapon::delay_ticks(pyro::AIRBLAST_PRIMARY_DELAY, interval);
                self.next_airblast_tick =
                    self.tick + weapon::delay_ticks(pyro::AIRBLAST_SECONDARY_DELAY, interval);
                self.activity_events.push(ActivityEvent {
                    tick: self.tick,
                    weapon: Weapon::Flamethrower,
                    activity: weapon::WeaponActivity::SecondaryAttack,
                });
                ammo_events.push(weapon::AmmoEvent {
                    tick: self.tick,
                    weapon: Weapon::Flamethrower,
                    clip: 0,
                    reserve: state.reserve,
                });
                self.stop_flame(false);
                self.emit_weapon_sound(SoundDefinition::FlameAirblast, self.movement.position);
                let (forward, _, _) =
                    angle_vectors(command.pitch_degrees, command.movement.yaw_degrees, 0.0);
                let eye = add(self.movement.position, self.movement.view_offset);
                let aim = self
                    .collision
                    .trace(
                        eye,
                        add(eye, scale(forward, 1.732_050_8 * 32_768.0)),
                        Hull {
                            mins: [0.0; 3],
                            maxs: [0.0; 3],
                        },
                        MASK_SOLID,
                    )?
                    .end;
                let player_team = self.team_selection.local_team();
                for projectile in &mut self.projectiles {
                    let delta = sub(projectile.presentation.position, eye);
                    let distance = length(delta);
                    if projectile.presentation.team == player_team
                        || distance <= 0.0
                        || distance > pyro::AIRBLAST_RADIUS * 2.0
                        || delta
                            .iter()
                            .zip(forward)
                            .map(|(component, axis)| component * axis)
                            .sum::<f32>()
                            / distance
                            < pyro::AIRBLAST_CONE_DEGREES.to_radians().cos()
                    {
                        continue;
                    }
                    let target = sub(aim, projectile.presentation.position);
                    let target_length = length(target);
                    if target_length == 0.0 {
                        continue;
                    }
                    let direction = scale(target, 1.0 / target_length);
                    let speed = length(projectile.presentation.velocity);
                    projectile.presentation.velocity = scale(direction, speed);
                    projectile.presentation.orientation = quaternion_from_direction(direction);
                    projectile.presentation.team = player_team;
                    projectile.presentation.owner_identity = PLAYER_IDENTITY;
                    projectile.presentation.launcher_identity = Weapon::Flamethrower as u32;
                    projectile.presentation.contact_normal = None;
                    projectile.motion_enabled = true;
                    projectile.direct_target = None;
                }
                let targets: Vec<_> = self.bots.as_ref().map_or_else(Vec::new, |bots| {
                    bots.combat_targets()
                        .filter(|target| {
                            let center = add(target.position, [0.0, 0.0, 41.0]);
                            let delta = sub(center, eye);
                            let distance = length(delta);
                            distance <= pyro::AIRBLAST_RADIUS * 2.0
                                && distance > 0.0
                                && delta
                                    .iter()
                                    .zip(forward)
                                    .map(|(component, axis)| component * axis)
                                    .sum::<f32>()
                                    / distance
                                    >= pyro::AIRBLAST_CONE_DEGREES.to_radians().cos()
                        })
                        .collect()
                });
                let maximum_health = self.maximum_health();
                if let Some(bots) = self.bots.as_mut() {
                    for target in targets {
                        if target.team == self.team_selection.local_team() {
                            if bots.extinguish(target.identity) {
                                self.health = (self.health + 20).min(maximum_health);
                            }
                        } else {
                            let impulse = pyro::airblast_impulse(
                                forward,
                                target.velocity,
                                Some([0.0, 0.0, 1.0]),
                            );
                            bots.apply_impulse(target.identity, impulse);
                        }
                    }
                }
            }
        } else if fire && self.movement.water_level != 3 {
            let state = self
                .loadout
                .get_mut(&Weapon::Flamethrower)
                .expect("Pyro has the stock Flamethrower");
            if state.reserve > 0 && self.tick >= state.next_primary_tick {
                let (forward, right, up) =
                    angle_vectors(command.pitch_degrees, command.movement.yaw_degrees, 0.0);
                let eye = add(self.movement.position, self.movement.view_offset);
                let origin = add(add(eye, scale(forward, 40.0)), scale(right, 5.0));
                let muzzle = self.collision.trace(
                    eye,
                    origin,
                    Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    MASK_SOLID,
                )?;
                if muzzle.fraction == 1.0 {
                    let began_firing = !self.flame_firing;
                    self.flames.add_authored_point(
                        pyro::FlameSpawn {
                            tick: self.tick,
                            now,
                            position: origin,
                            forward,
                            right,
                            up,
                            attacker_velocity: self.movement.velocity,
                        },
                        &mut self.authority_random,
                    )?;
                    let consumed = self.flames.consume_primary_ammo(&mut state.reserve);
                    state.next_primary_tick =
                        self.tick + weapon::delay_ticks(pyro::FLAME_FIRE_DELAY, interval);
                    if began_firing {
                        self.activity_events.push(ActivityEvent {
                            tick: self.tick,
                            weapon: Weapon::Flamethrower,
                            activity: weapon::WeaponActivity::PrimaryAttack,
                        });
                    }
                    if consumed > 0 {
                        ammo_events.push(weapon::AmmoEvent {
                            tick: self.tick,
                            weapon: Weapon::Flamethrower,
                            clip: 0,
                            reserve: state.reserve,
                        });
                    }
                    if began_firing {
                        self.flame_firing = true;
                        if self.flame_burst_until == 0.0 {
                            self.flame_burst_until = now + 0.2;
                        }
                        for (definition, action) in [
                            (SoundDefinition::FlameFire, AudioAction::FadeOut(3.5)),
                            (SoundDefinition::FlameLoop, AudioAction::FadeIn(3.5)),
                        ] {
                            self.emit_weapon_sound(definition, self.movement.position);
                            self.audio_events.last_mut().unwrap().action = action;
                        }
                    }
                } else {
                    self.stop_flame(false);
                }
            }
        }
        let world = &self.collision;
        self.flames.advance(
            now,
            interval,
            |point, destination| {
                let radius = pyro::FlameConfiguration::STOCK.radius;
                let trace = world.trace(
                    point.position,
                    destination,
                    Hull {
                        mins: [-radius; 3],
                        maxs: [radius; 3],
                    },
                    MASK_SOLID,
                )?;
                Ok::<_, MoveError>(if trace.start_solid || trace.fraction < 1.0 {
                    Some(pyro::FlameWorldContact {
                        fraction: trace.fraction,
                        start_solid: trace.start_solid,
                        end: trace.end,
                        normal: trace.normal.unwrap_or([0.0, 0.0, 1.0]),
                    })
                } else {
                    None
                })
            },
            |position| Ok::<_, MoveError>(world.point_contents(position)? & 0x30 != 0),
        )?;
        let contacts: Vec<_> = self
            .flames
            .points()
            .iter()
            .filter_map(|point| {
                self.bot_intersection(
                    point.previous_position,
                    point.position,
                    pyro::FlameConfiguration::STOCK.radius,
                    1.0,
                    true,
                )
                .map(|(target, _, position)| {
                    let elapsed = (now - point.spawn_time).max(0.0);
                    let fraction = (elapsed / (point.lifetime * 0.5)).clamp(0.0, 1.0);
                    (
                        target.identity,
                        position,
                        (pyro::FLAME_DAMAGE * (1.0 - fraction * 0.5)).max(1.0),
                    )
                })
            })
            .collect();
        if let Some(bots) = self.bots.as_mut() {
            for (target, position, damage) in contacts {
                if bots.apply_flame_contact(target, PLAYER_IDENTITY, now, damage) {
                    events.push(Event::HitscanImpact {
                        weapon: Weapon::Flamethrower,
                        target: Some(target),
                        pellet: 0,
                        position,
                        damage,
                        hitgroup: 0,
                        critical: false,
                        surface: None,
                    });
                }
            }
        }
        Ok(())
    }

    fn emit_objective_sound(
        &mut self,
        identity: u32,
        definition: SoundDefinition,
        position: [f32; 3],
    ) {
        let samples = self.sample_weapon_sound(definition);
        self.push_audio_event(AudioEvent {
            action: AudioAction::Play,
            tick: self.tick,
            ordinal: 0,
            identity: AudioEventIdentity::WeaponSingle,
            definition,
            source_kind: AudioSourceKind::LocalListener,
            source_identity: identity,
            owner_identity: None,
            position,
            samples,
        });
    }

    fn fire_hitscan(
        &mut self,
        weapon: Weapon,
        pitch: f32,
        yaw: f32,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let profile = ballistics::HitscanProfile::configured(weapon)
            .expect("hitscan weapon has a configured profile");
        let definition = match weapon {
            Weapon::Scattergun => SoundDefinition::ScattergunSingle,
            Weapon::Pistol | Weapon::EngineerPistol => SoundDefinition::PistolSingle,

            Weapon::Shotgun | Weapon::HeavyShotgun | Weapon::EngineerShotgun => {
                SoundDefinition::ShotgunSingle
            }
            Weapon::SniperRifle => SoundDefinition::SniperSingle,
            Weapon::Smg => SoundDefinition::SmgSingle,
            Weapon::Revolver => SoundDefinition::RevolverSingle,

            _ => unreachable!("only configured firearms use hitscan profiles"),
        };
        self.emit_weapon_sound(definition, self.movement.position);
        if weapon == Weapon::Revolver {
            self.remove_spy_disguise();
        }
        let sniper_state = self
            .loadout
            .get(&weapon)
            .copied()
            .expect("active hitscan weapon");
        let sniper_damage = if weapon == Weapon::SniperRifle {
            sniper_state.charged_damage.max(50.0)
        } else {
            profile.damage
        };
        if weapon == Weapon::SniperRifle {
            let state = self.loadout.get_mut(&weapon).expect("active Sniper rifle");
            if self.conditions.contains(Condition::Zoomed) {
                state.unzoom_due_tick = Some(
                    self.tick + weapon::delay_ticks(0.5, self.movement_configuration.tick_interval),
                );
                state.rezoom_after_shot = state.reserve > 0;
            }
            state.charged_damage = 0.0;
        }
        let previous = self.previous_hitscan_ticks.insert(weapon, self.tick);
        let elapsed = previous.map_or(f32::INFINITY, |tick| {
            self.tick.saturating_sub(tick) as f32 * self.movement_configuration.tick_interval
        });
        let (forward, right, up) = angle_vectors(pitch, yaw, 0.0);
        let origin = add(self.movement.position, self.movement.view_offset);
        if weapon == Weapon::Shotgun {
            self.shotgun_pellets.clear();
        }
        events.push(Event::HitscanFired {
            weapon,
            pellets: profile.pellets,
            owner: PLAYER_IDENTITY,
            origin,
        });
        let mut damage = BTreeMap::<u32, (f32, [f32; 3])>::new();
        for pellet in 0..profile.pellets {
            let direction =
                profile.pellet_direction(self.tick as u32, pellet, elapsed, forward, right, up);
            if weapon == Weapon::Shotgun {
                self.shotgun_pellets.push(pyro::ShotgunPellet {
                    index: pellet,
                    direction,
                    damage: profile.damage,
                    range: profile.range,
                });
            }
            let end = add(origin, scale(direction, profile.range));
            let impact = self.collision.trace(
                origin,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                MASK_SHOT,
            )?;
            let mut player_hit: Option<playsrc_collision::StudioHitboxTrace> = None;
            for candidate in &self.posed_player_hitboxes {
                if candidate.team == self.team_selection.local_team() {
                    continue;
                }
                let entry = playsrc_collision::StudioHitbox {
                    identity: candidate.hitbox,
                    group: candidate.group,
                    bone: candidate.bone,
                    physics_bone: candidate.physics_bone,
                    bone_contents: candidate.bone_contents,
                    surface: None,
                    minimum: candidate.minimum,
                    maximum: candidate.maximum,
                    bone_to_world: &candidate.bone_to_world,
                };
                let trace = playsrc_collision::trace_studio_hitboxes(
                    playsrc_collision::StudioHitboxRequest {
                        entity: u64::from(candidate.entity),
                        origin: candidate.origin,
                        scale: 1.0,
                        start: origin,
                        end,
                        hull: Hull {
                            mins: [0.0; 3],
                            maxs: [0.0; 3],
                        },
                        mask: MASK_SHOT,
                        hitboxes: std::slice::from_ref(&entry),
                    },
                )
                .map_err(|_| Error::InvalidProjectilePhysics)?;
                if let Some(trace) = trace
                    && trace.fraction <= impact.fraction
                    && player_hit
                        .as_ref()
                        .is_none_or(|prior| trace.fraction < prior.fraction)
                {
                    player_hit = Some(trace);
                }
            }
            let boxed_hit = if player_hit.is_none() {
                self.bots
                    .as_ref()
                    .and_then(|bots| {
                        bots.intersect_enemy(
                            self.team_selection.local_team(),
                            origin,
                            end,
                            PLAYER_IDENTITY,
                        )
                    })
                    .filter(|(_, fraction, _)| *fraction <= impact.fraction)
            } else {
                None
            };
            if player_hit.is_some()
                || boxed_hit.is_some()
                || impact.fraction < 1.0
                || impact.start_solid
            {
                let position = player_hit
                    .as_ref()
                    .map(|hit| hit.end)
                    .or_else(|| boxed_hit.map(|(_, _, position)| position))
                    .unwrap_or(impact.end);
                let distance = length(sub(position, origin));
                let target = player_hit
                    .as_ref()
                    .and_then(|hit| u32::try_from(hit.entity).ok())
                    .or_else(|| boxed_hit.map(|(identity, _, _)| identity))
                    .or_else(|| {
                        impact
                            .hit
                            .filter(|identity| !self.collision.is_world(*identity))
                            .and_then(|identity| u32::try_from(identity).ok())
                    });
                let hitgroup = player_hit
                    .as_ref()
                    .map_or(0, |hit| hit.hitgroup.max(0) as u8);
                let critical = sniper_state.sniper_headshot_is_critical(
                    self.tick,
                    self.movement_configuration.tick_interval,
                    self.conditions.contains(Condition::Zoomed),
                    hitgroup == 1,
                    false,
                );
                let amount = if weapon == Weapon::SniperRifle {
                    sniper_damage
                        * if critical {
                            damage::CRIT_MULTIPLIER
                        } else {
                            1.0
                        }
                } else {
                    profile.damage_at_distance(distance, weapon == Weapon::Scattergun)
                };
                if let Some(identity) = target
                    && self
                        .bots
                        .as_ref()
                        .is_some_and(|bots| bots.contains(identity))
                {
                    damage.entry(identity).or_insert((0.0, position)).0 += amount;
                }
                let damage = amount;
                events.push(Event::HitscanImpact {
                    weapon,
                    target,
                    pellet,
                    hitgroup,
                    critical,
                    position,
                    damage,
                    surface: if target.is_none() {
                        self.collision.impact_surface(origin, end, MASK_SHOT)?
                    } else {
                        None
                    },
                });
            }
        }
        for (victim, (amount, position)) in damage {
            self.apply_actor_damage(
                bot::Damage {
                    attacker: PLAYER_IDENTITY,
                    victim,
                    weapon,
                    amount,
                    position,
                },
                self.team_selection.local_team(),
                events,
            )?;
        }
        Ok(())
    }

    fn advance_medigun(&mut self, command: Command) -> Result<(), Error> {
        let interval = self.movement_configuration.tick_interval;
        let now = self.tick as f32 * interval;
        let origin = add(self.movement.position, self.movement.view_offset);
        let direction = angle_vectors(command.pitch_degrees, command.movement.yaw_degrees, 0.0).0;
        if !command.fire {
            if let Some(identity) = self.medigun.target
                && let Some((health, conditions)) = self
                    .bots
                    .as_mut()
                    .and_then(|bots| bots.patient_state(identity))
            {
                self.medigun
                    .detach(now, PLAYER_IDENTITY, health, conditions);
                self.emit_weapon_sound(SoundDefinition::MedigunDetach, self.movement.position);
            }
        } else {
            if let Some(identity) = self.medigun.target {
                let facts = self
                    .bots
                    .as_ref()
                    .and_then(|bots| bots.patient(identity, origin));
                let retained = if let Some(patient) = facts {
                    let visible = |point| {
                        self.collision
                            .trace(
                                origin,
                                point,
                                Hull {
                                    mins: [0.0; 3],
                                    maxs: [0.0; 3],
                                },
                                MASK_SOLID,
                            )
                            .is_ok_and(|trace| trace.fraction >= 1.0)
                    };
                    self.medigun.maintain(
                        now,
                        origin,
                        patient,
                        false,
                        visible(patient.center),
                        visible(patient.eyes),
                    )
                } else {
                    false
                };
                if !retained {
                    if let Some((health, conditions)) = self
                        .bots
                        .as_mut()
                        .and_then(|bots| bots.patient_state(identity))
                    {
                        self.medigun
                            .detach(now, PLAYER_IDENTITY, health, conditions);
                    } else {
                        self.medigun.target = None;
                    }
                    self.emit_weapon_sound(SoundDefinition::MedigunDetach, self.movement.position);
                }
            }
            if self.medigun.target.is_none() {
                let candidate = self.bots.as_ref().and_then(|bots| {
                    bots.patient_identities()
                        .filter_map(|identity| bots.patient(identity, origin))
                        .filter(|patient| patient.allowed(self.team_selection.local_team(), false))
                        .filter_map(|patient| {
                            let relative = sub(patient.center, origin);
                            let along = relative[0] * direction[0]
                                + relative[1] * direction[1]
                                + relative[2] * direction[2];
                            if !(0.0..=medic::MEDIGUN_TARGET_RANGE).contains(&along) {
                                return None;
                            }
                            let closest = sub(relative, scale(direction, along));
                            if length(closest) > 24.0 {
                                return None;
                            }
                            let trace = self
                                .collision
                                .trace(
                                    origin,
                                    patient.center,
                                    Hull {
                                        mins: [0.0; 3],
                                        maxs: [0.0; 3],
                                    },
                                    MASK_SOLID,
                                )
                                .ok()?;
                            (trace.fraction >= 1.0).then_some((along, patient))
                        })
                        .min_by(|left, right| left.0.total_cmp(&right.0))
                        .map(|(_, patient)| patient)
                });
                if let Some(patient) = candidate
                    && let Some((health, conditions)) = self
                        .bots
                        .as_mut()
                        .and_then(|bots| bots.patient_state(patient.identity))
                {
                    self.medigun
                        .attach(
                            now,
                            PLAYER_IDENTITY,
                            self.team_selection.local_team(),
                            false,
                            origin,
                            patient,
                            health,
                            conditions,
                        )
                        .map_err(|_| Error::InvalidProjectilePhysics)?;
                    self.activity_events.push(ActivityEvent {
                        tick: self.tick,
                        weapon: Weapon::MediGun,
                        activity: weapon::WeaponActivity::PrimaryAttack,
                    });
                    self.emit_weapon_sound(SoundDefinition::MedigunHealing, self.movement.position);
                }
            }
            if let Some(patient) = self
                .medigun
                .target
                .and_then(|identity| self.bots.as_ref()?.patient(identity, origin))
                && self.medigun.build_charge(
                    interval,
                    patient,
                    self.round
                        .timer()
                        .is_some_and(|timer| timer.state == round::TimerState::Setup),
                ) == Some(medic::BeamTransition::ChargeReady)
            {
                self.emit_weapon_sound(SoundDefinition::MedigunCharged, self.movement.position);
            }
        }
        if command.detonate && !self.medigun.releasing && self.medigun.charge >= 1.0 {
            let mut medic_conditions = condition::ConditionState::default();
            let patient_conditions = self
                .medigun
                .target
                .and_then(|identity| self.bots.as_mut()?.patient_state(identity))
                .map(|(_, conditions)| conditions);
            self.medigun
                .activate(PLAYER_IDENTITY, &mut medic_conditions, patient_conditions)
                .map_err(|_| Error::InvalidProjectilePhysics)?;
            if self.medigun.releasing {
                self.conditions.insert(Condition::Invulnerable);
            }
        }
        if self.medigun.releasing {
            let alive = self
                .bots
                .as_ref()
                .map(|bots| {
                    bots.patient_identities()
                        .filter(|identity| {
                            bots.patient(*identity, origin)
                                .is_some_and(|patient| patient.alive)
                        })
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            if self
                .medigun
                .drain(now, interval, |identity| alive.contains(&identity))
                == Some(medic::BeamTransition::ChargeEnded)
            {
                self.conditions.remove(Condition::Invulnerable);
                if let Some((_, conditions)) = self
                    .medigun
                    .target
                    .and_then(|identity| self.bots.as_mut()?.patient_state(identity))
                {
                    conditions.remove(condition::ConditionId::INVULNERABLE, false);
                }
            }
        }
        Ok(())
    }

    fn enemy_bot_trace(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        expand: f32,
    ) -> Option<(u32, [f32; 3], [f32; 3])> {
        let bots = self.bots.as_ref()?;
        let delta = sub(end, start);
        bots.patient_identities()
            .filter_map(|identity| {
                let patient = bots.patient(identity, start)?;
                if !patient.alive || patient.team == self.team_selection.local_team() {
                    return None;
                }
                let origin = [
                    patient.center[0],
                    patient.center[1],
                    patient.center[2] - 41.0,
                ];
                let minimum = [
                    origin[0] - 24.0 - expand,
                    origin[1] - 24.0 - expand,
                    origin[2] - expand,
                ];
                let maximum = [
                    origin[0] + 24.0 + expand,
                    origin[1] + 24.0 + expand,
                    origin[2] + 82.0 + expand,
                ];
                let mut entering = 0.0_f32;
                let mut exiting = 1.0_f32;
                let mut normal = [0.0_f32; 3];
                for axis in 0..3 {
                    if delta[axis].abs() < f32::EPSILON {
                        if start[axis] < minimum[axis] || start[axis] > maximum[axis] {
                            return None;
                        }
                        continue;
                    }
                    let mut first = (minimum[axis] - start[axis]) / delta[axis];
                    let mut second = (maximum[axis] - start[axis]) / delta[axis];
                    let sign = if first > second {
                        std::mem::swap(&mut first, &mut second);
                        1.0
                    } else {
                        -1.0
                    };
                    if first > entering {
                        entering = first;
                        normal = [0.0; 3];
                        normal[axis] = sign;
                    }
                    exiting = exiting.min(second);
                    if entering > exiting {
                        return None;
                    }
                }
                if !(0.0..=1.0).contains(&entering) {
                    return None;
                }
                if normal == [0.0; 3] {
                    normal = normalized(scale(delta, -1.0));
                }
                Some((
                    entering,
                    identity,
                    add(start, scale(delta, entering)),
                    normal,
                ))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, identity, position, normal)| (identity, position, normal))
    }

    fn swing_melee(&mut self, weapon: Weapon) {
        let (definition, smack_delay) = match weapon {
            Weapon::Bat => (SoundDefinition::BatMiss, ballistics::MELEE_SMACK_DELAY),
            Weapon::Shovel => (SoundDefinition::ShovelMiss, ballistics::MELEE_SMACK_DELAY),
            Weapon::Kukri => (SoundDefinition::KukriMiss, ballistics::MELEE_SMACK_DELAY),
            Weapon::Bottle => (SoundDefinition::BottleMiss, ballistics::MELEE_SMACK_DELAY),
            Weapon::Wrench => (SoundDefinition::WrenchMiss, ballistics::MELEE_SMACK_DELAY),
            Weapon::FireAxe => (SoundDefinition::FireAxeMiss, ballistics::MELEE_SMACK_DELAY),
            Weapon::Bonesaw => (SoundDefinition::BonesawMiss, medic::BONESAW_SMACK_DELAY),
            _ => unreachable!("only melee weapons swing"),
        };
        self.emit_weapon_sound(definition, self.movement.position);
        let delay = if weapon == Weapon::Bonesaw {
            weapon::delay_ticks(smack_delay, self.movement_configuration.tick_interval)
        } else {
            (smack_delay / self.movement_configuration.tick_interval).floor() as u64
        };
        self.pending_melee_tick = Some(self.tick.saturating_add(delay));
    }

    fn resolve_melee(
        &mut self,
        weapon: Weapon,
        pitch: f32,
        yaw: f32,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let (direction, _, _) = angle_vectors(pitch, yaw, 0.0);
        let origin = add(self.movement.position, self.movement.view_offset);
        if weapon == Weapon::Wrench
            && let Some(target) = self.buildings.nearest_wrench_target(
                origin,
                direction,
                self.team_selection.local_team(),
            )
            && self.buildings.wrench(
                target,
                self.team_selection.local_team(),
                self.tick,
                &mut self.ammo.metal,
            )
        {
            events.push(Event::MeleeImpact {
                weapon,
                owner: PLAYER_IDENTITY,
                target: Some(target),
                position: origin,
                damage: 0.0,
            });
            return Ok(());
        }
        if weapon == Weapon::Wrench {
            let building_end = add(origin, scale(direction, ballistics::WRENCH_BUILDING_RANGE));
            let building = self.collision.trace(
                origin,
                building_end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                MASK_SOLID,
            )?;
            if building.fraction >= 1.0 && !building.start_solid {
                self.collision.trace(
                    origin,
                    building_end,
                    Hull {
                        mins: [-ballistics::MELEE_HULL_RADIUS; 3],
                        maxs: [ballistics::MELEE_HULL_RADIUS; 3],
                    },
                    MASK_SOLID,
                )?;
            }
        }
        let end = add(origin, scale(direction, ballistics::MELEE_RANGE));
        let line_hull = Hull {
            mins: [0.0; 3],
            maxs: [0.0; 3],
        };
        let line = self.collision.trace(origin, end, line_hull, MASK_SOLID)?;
        let line_player = self.trace_melee_players(origin, end, line_hull, line.fraction)?;
        let (impact, player) = if line.fraction < 1.0 || line.start_solid || line_player.is_some() {
            (line, line_player)
        } else {
            let hull = Hull {
                mins: [-ballistics::MELEE_HULL_RADIUS; 3],
                maxs: [ballistics::MELEE_HULL_RADIUS; 3],
            };
            let impact = self.collision.trace(origin, end, hull, MASK_SOLID)?;
            let player = self.trace_melee_players(origin, end, hull, impact.fraction)?;
            (impact, player)
        };
        let actor = player
            .as_ref()
            .and_then(|hit| {
                let identity = u32::try_from(hit.entity).ok()?;
                self.bots
                    .as_ref()?
                    .contains(identity)
                    .then_some((identity, hit.fraction, hit.end))
            })
            .or_else(|| {
                self.bots.as_ref()?.intersect_enemy(
                    self.team_selection.local_team(),
                    origin,
                    end,
                    PLAYER_IDENTITY,
                )
            })
            .filter(|(_, fraction, _)| *fraction <= impact.fraction);
        if player.is_some() || actor.is_some() || impact.fraction < 1.0 || impact.start_solid {
            let position = actor.map_or_else(
                || player.as_ref().map_or(impact.end, |hit| hit.end),
                |(_, _, position)| position,
            );
            let target = actor
                .map(|(identity, _, _)| identity)
                .or_else(|| {
                    player
                        .as_ref()
                        .and_then(|hit| u32::try_from(hit.entity).ok())
                })
                .or_else(|| {
                    impact
                        .hit
                        .filter(|identity| !self.collision.is_world(*identity))
                        .and_then(|identity| u32::try_from(identity).ok())
                });

            let (definition, damage) = match (weapon, target.is_some()) {
                (Weapon::Bat, true) => (SoundDefinition::BatHitFlesh, ballistics::BAT_DAMAGE),
                (Weapon::Bat, false) => (SoundDefinition::BatHitWorld, ballistics::BAT_DAMAGE),
                (Weapon::Shovel, true) => {
                    (SoundDefinition::ShovelHitFlesh, ballistics::SHOVEL_DAMAGE)
                }
                (Weapon::Shovel, false) => {
                    (SoundDefinition::ShovelHitWorld, ballistics::SHOVEL_DAMAGE)
                }
                (Weapon::Bottle, true) => {
                    (SoundDefinition::BottleHitFlesh, ballistics::BOTTLE_DAMAGE)
                }
                (Weapon::Bottle, false) => {
                    (SoundDefinition::BottleHitWorld, ballistics::BOTTLE_DAMAGE)
                }
                (Weapon::Wrench, true) => {
                    (SoundDefinition::WrenchHitFlesh, ballistics::WRENCH_DAMAGE)
                }
                (Weapon::Wrench, false) => {
                    (SoundDefinition::WrenchHitWorld, ballistics::WRENCH_DAMAGE)
                }
                (Weapon::Kukri, true) => (SoundDefinition::KukriHitFlesh, ballistics::KUKRI_DAMAGE),
                (Weapon::Kukri, false) => {
                    (SoundDefinition::KukriHitWorld, ballistics::KUKRI_DAMAGE)
                }
                (Weapon::FireAxe, true) => {
                    (SoundDefinition::FireAxeHitFlesh, pyro::FIRE_AXE_DAMAGE)
                }
                (Weapon::FireAxe, false) => {
                    (SoundDefinition::FireAxeHitWorld, pyro::FIRE_AXE_DAMAGE)
                }
                (Weapon::Bonesaw, true) => (
                    SoundDefinition::BonesawHitFlesh,
                    medic::BONESAW_DAMAGE as f32,
                ),
                (Weapon::Bonesaw, false) => (
                    SoundDefinition::BonesawHitWorld,
                    medic::BONESAW_DAMAGE as f32,
                ),
                _ => unreachable!("only melee weapons resolve swings"),
            };
            self.emit_weapon_sound(definition, position);
            events.push(Event::MeleeImpact {
                weapon,
                owner: PLAYER_IDENTITY,
                target,
                position,
                damage,
            });
            if let Some(victim) = actor.map(|(identity, _, _)| identity) {
                self.apply_actor_damage(
                    bot::Damage {
                        attacker: PLAYER_IDENTITY,
                        victim,
                        weapon,
                        amount: damage,
                        position,
                    },
                    self.team_selection.local_team(),
                    events,
                )?;
            }
        }

        Ok(())
    }

    fn resolve_knife(
        &mut self,
        pitch: f32,
        yaw: f32,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let (direction, _, _) = angle_vectors(pitch, yaw, 0.0);
        let origin = add(self.movement.position, self.movement.view_offset);
        let end = add(origin, scale(direction, ballistics::MELEE_RANGE));
        let line = self.collision.trace(
            origin,
            end,
            Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            },
            MASK_SOLID,
        )?;
        let impact = if line.fraction < 1.0 || line.start_solid {
            line
        } else {
            self.collision.trace(
                origin,
                end,
                Hull {
                    mins: [-ballistics::MELEE_HULL_RADIUS; 3],
                    maxs: [ballistics::MELEE_HULL_RADIUS; 3],
                },
                MASK_SOLID,
            )?
        };
        let mut player_hit: Option<playsrc_collision::StudioHitboxTrace> = None;
        for candidate in &self.posed_player_hitboxes {
            if !candidate.team.is_enemy(self.team_selection.local_team()) {
                continue;
            }
            let hitbox = playsrc_collision::StudioHitbox {
                identity: candidate.hitbox,
                group: candidate.group,
                bone: candidate.bone,
                physics_bone: candidate.physics_bone,
                bone_contents: candidate.bone_contents,
                surface: None,
                minimum: candidate.minimum,
                maximum: candidate.maximum,
                bone_to_world: &candidate.bone_to_world,
            };
            let hit =
                playsrc_collision::trace_studio_hitboxes(playsrc_collision::StudioHitboxRequest {
                    entity: u64::from(candidate.entity),
                    origin: candidate.origin,
                    scale: 1.0,
                    start: origin,
                    end,
                    hull: Hull {
                        mins: [-ballistics::MELEE_HULL_RADIUS; 3],
                        maxs: [ballistics::MELEE_HULL_RADIUS; 3],
                    },
                    mask: MASK_SHOT,
                    hitboxes: std::slice::from_ref(&hitbox),
                })
                .map_err(|_| Error::InvalidProjectilePhysics)?;
            if let Some(hit) = hit
                && hit.fraction <= impact.fraction
                && player_hit
                    .as_ref()
                    .is_none_or(|prior| hit.fraction < prior.fraction)
            {
                player_hit = Some(hit);
            }
        }
        self.emit_weapon_sound(SoundDefinition::KnifeMiss, self.movement.position);
        if player_hit.is_some() || impact.fraction < 1.0 || impact.start_solid {
            let target = player_hit
                .as_ref()
                .and_then(|hit| u32::try_from(hit.entity).ok())
                .or_else(|| {
                    impact
                        .hit
                        .filter(|identity| !self.collision.is_world(*identity))
                        .and_then(|identity| u32::try_from(identity).ok())
                });
            let position = player_hit.as_ref().map_or(impact.end, |hit| hit.end);
            let facts = target.and_then(|identity| {
                self.bots
                    .as_ref()
                    .and_then(|bots| bots.combat_player(identity))
                    .or_else(|| self.collision.combat_player(identity))
            });
            let backstab = facts
                .filter(|facts| {
                    facts.team.is_enemy(self.team_selection.local_team())
                        && !facts.backstab_immune
                        && spy::can_backstab(
                            add(self.movement.position, [0.0, 0.0, 41.0]),
                            direction,
                            facts.world_center,
                            facts.eye_forward,
                        )
                });
            let damage = backstab.map_or(spy::KNIFE_DAMAGE, |facts| {
                    spy::backstab_damage(facts.health)
                });
            self.emit_weapon_sound(
                if target.is_some() {
                    SoundDefinition::KnifeHitFlesh
                } else {
                    SoundDefinition::KnifeHitWorld
                },
                position,
            );
            if let Some(identity) = target {
                self.apply_actor_damage_from(
                    bot::Damage {
                        attacker: PLAYER_IDENTITY,
                        victim: identity,
                        weapon: Weapon::Knife,
                        amount: damage,
                        position,
                    },
                    self.team_selection.local_team(),
                    None,
                    Some(if backstab.is_some() { 2 } else { 0 }),
                    events,
                )?;
            }
            events.push(Event::MeleeImpact {
                weapon: Weapon::Knife,
                owner: PLAYER_IDENTITY,
                target,
                position,
                damage,
            });
        }
        self.remove_spy_disguise();
        Ok(())
    }

    fn trace_melee_players(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        maximum_fraction: f32,
    ) -> Result<Option<playsrc_collision::StudioHitboxTrace>, Error> {
        let mut nearest: Option<playsrc_collision::StudioHitboxTrace> = None;
        for candidate in &self.posed_player_hitboxes {
            if candidate.team == self.team_selection.local_team() {
                continue;
            }
            let hitbox = playsrc_collision::StudioHitbox {
                identity: candidate.hitbox,
                group: candidate.group,
                bone: candidate.bone,
                physics_bone: candidate.physics_bone,
                bone_contents: candidate.bone_contents,
                surface: None,
                minimum: candidate.minimum,
                maximum: candidate.maximum,
                bone_to_world: &candidate.bone_to_world,
            };
            let trace =
                playsrc_collision::trace_studio_hitboxes(playsrc_collision::StudioHitboxRequest {
                    entity: u64::from(candidate.entity),
                    origin: candidate.origin,
                    scale: 1.0,
                    start,
                    end,
                    hull,
                    mask: MASK_SOLID,
                    hitboxes: std::slice::from_ref(&hitbox),
                })
                .map_err(|_| Error::InvalidProjectilePhysics)?;
            if let Some(trace) = trace
                && trace.fraction <= maximum_fraction
                && nearest
                    .as_ref()
                    .is_none_or(|previous| trace.fraction < previous.fraction)
            {
                nearest = Some(trace);
            }
        }
        Ok(nearest)
    }

    fn fire_minigun_hitscan(
        &mut self,
        pitch: f32,
        yaw: f32,
        events: &mut Vec<Event>,
    ) -> Result<MapPhase, Error> {
        let (forward, right, up) = angle_vectors(pitch, yaw, 0.0);
        let source = add(self.movement.position, self.movement.view_offset);
        let (damage_penalty, spread_penalty) = self.loadout[&Weapon::Minigun]
            .minigun_penalties(self.tick, self.movement_configuration.tick_interval);
        let spread = 0.08 * spread_penalty;
        let mut phase = MapPhase::default();
        let mut victims = BTreeMap::<u32, (f32, [f32; 3])>::new();
        events.push(Event::HitscanFired {
            weapon: Weapon::Minigun,
            pellets: 4,
            owner: PLAYER_IDENTITY,
            origin: source,
        });
        for pellet in 0..4 {
            let mut random = UniformRandomStream::from_seed(((self.tick as i32) & 255) + pellet)
                .expect("bounded Source bullet seed");
            let (x, y) = (
                random.random_float(-0.5, 0.5) + random.random_float(-0.5, 0.5),
                random.random_float(-0.5, 0.5) + random.random_float(-0.5, 0.5),
            );
            let direction = normalized(add(
                add(forward, scale(right, x * spread)),
                scale(up, y * spread),
            ));
            let end = add(source, scale(direction, 8192.0));
            let trace = self.collision.trace(
                source,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                MASK_SOLID,
            )?;
            if let Some((target, fraction, position)) = self.bots.as_ref().and_then(|bots| {
                bots.intersect_enemy(
                    self.team_selection.local_team(),
                    source,
                    end,
                    PLAYER_IDENTITY,
                )
            }) && fraction <= trace.fraction
            {
                let profile = ballistics::HitscanProfile {
                    pellets: 4,
                    damage: 9.0 * damage_penalty,
                    range: 8192.0,
                    spread,
                    accurate_after_seconds: f32::MAX,
                };
                let damage = profile.damage_at_distance(length(sub(position, source)), false);
                victims.entry(target).or_insert((0.0, position)).0 += damage;
                events.push(Event::HitscanImpact {
                    weapon: Weapon::Minigun,
                    target: Some(target),
                    pellet: pellet as u8,
                    hitgroup: 0,
                    critical: false,
                    position,
                    damage,
                    surface: None,
                });
            } else if let Some(target) = trace.hit.and_then(|identity| u32::try_from(identity).ok())
                && let Ok(damage) = self.map.damage(self.tick, target)
            {
                phase.append(damage);
                events.push(Event::HitscanImpact {
                    weapon: Weapon::Minigun,
                    target: (!self.collision.is_world(u64::from(target))).then_some(target),
                    pellet: pellet as u8,
                    hitgroup: 0,
                    critical: false,
                    position: trace.end,
                    damage: 9.0 * damage_penalty,
                    surface: self.collision.impact_surface(source, end, MASK_SOLID)?,
                });
            } else if trace.fraction < 1.0 || trace.start_solid {
                events.push(Event::HitscanImpact {
                    weapon: Weapon::Minigun,
                    target: None,
                    pellet: pellet as u8,
                    hitgroup: 0,
                    critical: false,
                    position: trace.end,
                    damage: 9.0 * damage_penalty,
                    surface: self.collision.impact_surface(source, end, MASK_SOLID)?,
                });
            }
        }
        for (victim, (amount, position)) in victims {
            self.apply_actor_damage(
                bot::Damage {
                    attacker: PLAYER_IDENTITY,
                    victim,
                    weapon: Weapon::Minigun,
                    amount,
                    position,
                },
                self.team_selection.local_team(),
                events,
            )?;
        }
        Ok(phase)
    }

    fn smack_fists(&mut self, pitch: f32, yaw: f32) -> Result<MapPhase, Error> {
        let (forward, _, _) = angle_vectors(pitch, yaw, 0.0);
        let source = add(self.movement.position, self.movement.view_offset);
        let trace = self.collision.trace(
            source,
            add(source, scale(forward, 48.0)),
            Hull {
                mins: [-18.0; 3],
                maxs: [18.0; 3],
            },
            MASK_SOLID,
        )?;
        if let Some(target) = trace.hit.and_then(|identity| u32::try_from(identity).ok())
            && let Ok(phase) = self.map.damage(self.tick, target)
        {
            self.emit_weapon_sound(SoundDefinition::FistHitFlesh, self.movement.position);
            return Ok(phase);
        }
        if trace.fraction < 1.0 {
            self.emit_weapon_sound(SoundDefinition::FistHitWorld, self.movement.position);
        }
        Ok(MapPhase::default())
    }

    fn fire_projectile(
        &mut self,
        pitch: f32,
        yaw: f32,
        charge_seconds: f32,
        expected_sticky_random: Option<StickyLaunchRandom>,
        bot_attack: Option<bot::Attack>,
        projectile_events: &mut Vec<ProjectileEvent>,
    ) -> Result<(), Error> {
        let weapon = bot_attack.map_or_else(
            || self.weapon.expect("firing requires an active weapon"),
            |attack| attack.weapon,
        );
        let owner = bot_attack.map_or(PLAYER_IDENTITY, |attack| attack.attacker);
        let team = bot_attack.map_or(self.team_selection.local_team(), |attack| attack.team);
        let player_position = bot_attack.map_or(self.movement.position, |attack| attack.position);
        let eye = bot_attack.map_or_else(
            || add(self.movement.position, self.movement.view_offset),
            |attack| attack.eye_position,
        );
        let definition = match weapon {
            Weapon::RocketLauncher => SoundDefinition::RocketSingle,
            Weapon::Original => SoundDefinition::OriginalSingle,
            Weapon::StickybombLauncher => SoundDefinition::StickySingle,
            Weapon::Scattergun
            | Weapon::Pistol
            | Weapon::Bat
            | Weapon::Shotgun
            | Weapon::Shovel
            | Weapon::Minigun
            | Weapon::HeavyShotgun
            | Weapon::Fists
            | Weapon::SniperRifle
            | Weapon::Smg
            | Weapon::Kukri
            | Weapon::Bottle
            | Weapon::GrenadeLauncher
            | Weapon::EngineerShotgun
            | Weapon::EngineerPistol
            | Weapon::Wrench
            | Weapon::Flamethrower
            | Weapon::FireAxe
            | Weapon::Revolver
            | Weapon::Knife
            | Weapon::Sapper
            | Weapon::DisguiseKit
            | Weapon::InvisibilityWatch
            | Weapon::BuildPda
            | Weapon::DestroyPda
            | Weapon::Toolbox => {
                unreachable!("hitscan and melee weapons do not spawn projectiles")
            }
            Weapon::SyringeGun => SoundDefinition::SyringeSingle,
            Weapon::MediGun | Weapon::Bonesaw => return Err(Error::InvalidProjectilePhysics),
        };
        let sound_samples = self.sample_weapon_sound(definition);
        self.push_audio_event(AudioEvent {
            action: AudioAction::Play,
            tick: self.tick,
            ordinal: 0,
            identity: AudioEventIdentity::WeaponSingle,
            definition,
            source_kind: AudioSourceKind::Entity,
            source_identity: owner,
            owner_identity: Some(owner),
            position: player_position,
            samples: sound_samples,
        });
        if self.projectiles.len() >= MAX_PROJECTILES {
            return Err(Error::ProjectileLimit);
        }
        let kind = match weapon {
            Weapon::RocketLauncher | Weapon::Original => ProjectileKind::Rocket,
            Weapon::StickybombLauncher => ProjectileKind::Sticky,
            Weapon::Scattergun
            | Weapon::Pistol
            | Weapon::Bat
            | Weapon::Shotgun
            | Weapon::Shovel
            | Weapon::Minigun
            | Weapon::HeavyShotgun
            | Weapon::Fists
            | Weapon::SniperRifle
            | Weapon::Smg
            | Weapon::Kukri
            | Weapon::Bottle
            | Weapon::GrenadeLauncher
            | Weapon::EngineerShotgun
            | Weapon::EngineerPistol
            | Weapon::Wrench
            | Weapon::Flamethrower
            | Weapon::FireAxe
            | Weapon::Revolver
            | Weapon::Knife
            | Weapon::Sapper
            | Weapon::DisguiseKit
            | Weapon::InvisibilityWatch
            | Weapon::BuildPda
            | Weapon::DestroyPda
            | Weapon::Toolbox => {
                unreachable!("hitscan and melee weapons do not spawn projectiles")
            }
            Weapon::SyringeGun => ProjectileKind::Syringe,
            Weapon::MediGun | Weapon::Bonesaw => return Err(Error::InvalidProjectilePhysics),
        };
        let profile = weapon::WeaponProfile::configured(weapon);
        let (mut direction, right, up) = angle_vectors(pitch, yaw, 0.0);
        let viewmodel_flipped = profile.flip_viewmodel != self.flip_viewmodels;
        let (position, orientation) = if kind == ProjectileKind::Sticky {
            let right_offset = if viewmodel_flipped { -8.0 } else { 8.0 };
            let position = add(
                add(add(eye, scale(direction, 16.0)), scale(right, right_offset)),
                scale(up, -6.0),
            );
            let muzzle = self.collision.trace(
                eye,
                position,
                Hull {
                    mins: [-8.0; 3],
                    maxs: [8.0; 3],
                },
                MASK_SOLID_BRUSH_ONLY,
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
                MASK_SOLID,
            )?;
            let mut offset = if kind == ProjectileKind::Syringe {
                [16.0, 6.0, -8.0]
            } else {
                [23.5, 12.0, -3.0]
            };
            if viewmodel_flipped {
                offset[1] *= -1.0;
            }
            if profile.center_fire_projectile {
                offset[1] = 0.0;
            }
            if bot_attack.is_none() && self.movement.crouch.uses_crouched_hull() {
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
            if kind == ProjectileKind::Syringe {
                let aim_pitch = (-direction[2])
                    .atan2(direction[0].hypot(direction[1]))
                    .to_degrees();
                let aim_yaw = direction[1].atan2(direction[0]).to_degrees();
                let pitch_spread = self.draw_random_float(
                    RandomContext::Authority,
                    RandomDecision::SyringePitchSpread,
                    -1.5,
                    1.5,
                );
                let yaw_spread = self.draw_random_float(
                    RandomContext::Authority,
                    RandomDecision::SyringeYawSpread,
                    -1.5,
                    1.5,
                );
                direction = angle_vectors(aim_pitch + pitch_spread, aim_yaw + yaw_spread, 0.0).0;
                (source, quaternion_from_direction(direction))
            } else {
                let clipped = self.collision.trace(
                    eye,
                    source,
                    Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    MASK_SOLID_BRUSH_ONLY,
                )?;
                (clipped.end, quaternion_from_direction(direction))
            }
        };
        let (velocity, angular_velocity) = if kind == ProjectileKind::Sticky {
            let random = StickyLaunchRandom {
                right_velocity: self.draw_random_float(
                    RandomContext::Authority,
                    RandomDecision::StickyRightVelocity,
                    -10.0,
                    10.0,
                ),
                up_velocity: self.draw_random_float(
                    RandomContext::Authority,
                    RandomDecision::StickyUpVelocity,
                    -10.0,
                    10.0,
                ),
                angular_y: self.draw_random_int(
                    RandomContext::Authority,
                    RandomDecision::StickyAngularY,
                    -1200,
                    1200,
                ),
            };
            if !random.validate() || expected_sticky_random.is_some_and(|value| value != random) {
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
            if expected_sticky_random.is_some() {
                return Err(Error::InvalidStickyLaunchRandom);
            }
            let speed = if kind == ProjectileKind::Syringe {
                medic::SYRINGE_SPEED
            } else {
                1_100.0
            };
            (scale(direction, speed), [0.0; 3])
        };
        let identity = self.next_projectile;
        self.next_projectile = self.next_projectile.wrapping_add(1).max(1);
        let tick_interval = self.movement_configuration.tick_interval;
        let projectile = LiveProjectile {
            killing_weapon: self.killing_weapon_name(owner, weapon),
            presentation: Projectile {
                identity,
                kind,
                team,
                owner_identity: owner,
                launcher_identity: if owner == PLAYER_IDENTITY {
                    weapon as u32
                } else {
                    0x4000_0000 + owner
                },
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
            arm_tick: self.tick.saturating_add(ticks(0.8, tick_interval)),
            next_think_tick: self.tick.saturating_add(ticks(0.2, tick_interval)),
            forced_detonate_tick: None,
            motion_enabled: true,
            direct_target: None,
        };
        let mut fire = projectile_event(
            ProjectileEventKind::Fire,
            &projectile.presentation,
            self.tick,
        );
        fire.launcher_pose = Some(ProjectileLauncherPose {
            eye_position: eye,
            view_orientation: quaternion_from_angles(pitch, yaw, 0.0),
        });
        projectile_events.push(fire);
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
        expected: &[ProjectilePhysicsRequest],
        results: &[ProjectilePhysicsResult],
        projectile_events: &mut Vec<ProjectileEvent>,
    ) -> Result<(), Error> {
        if results.len() != expected.len() {
            return Err(Error::InvalidProjectilePhysics);
        }
        for (request, result) in expected.iter().zip(results) {
            if result.tick != self.tick
                || request.tick.checked_add(1) != Some(result.tick)
                || result.projectile != request.projectile
                || !result.motion_enabled
                || result
                    .position
                    .into_iter()
                    .chain(result.velocity)
                    .chain(result.orientation)
                    .chain(result.angular_velocity)
                    .any(|value| !value.is_finite())
                || result.contact.is_some_and(|contact| {
                    contact.normal.into_iter().any(|value| !value.is_finite())
                        || length(contact.normal) <= f32::EPSILON
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
                    projectile_events.push(projectile_event_with_contact(
                        ProjectileEventKind::Impact,
                        &projectile.presentation,
                        self.tick,
                        Some(normal),
                    ));
                    if matches!(
                        contact.kind,
                        ProjectileContactKind::World | ProjectileContactKind::DynamicProp
                    ) {
                        projectile.presentation.contact_normal = Some(normal);
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
        expected: &[RocketTraceRequest],
        results: &[RocketTraceResult],
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<MapPhase, Error> {
        if results.len() != expected.len() {
            return Err(Error::InvalidProjectilePhysics);
        }
        let mut map_phase = MapPhase::default();
        for (request, result) in expected.iter().zip(results) {
            if result.tick != self.tick
                || request.tick.checked_add(1) != Some(result.tick)
                || result.projectile != request.projectile
                || result.end.into_iter().any(|value| !value.is_finite())
                || result.normal.is_some_and(|normal| {
                    normal.into_iter().any(|value| !value.is_finite())
                        || length(normal) <= f32::EPSILON
                })
                || result.sky && !result.solid
                || result.sky && result.direct_target.is_some()
                || result.solid && !result.sky && result.normal.is_none()
                || !result.solid && result.direct_target.is_some()
            {
                return Err(Error::InvalidProjectilePhysics);
            }
            let Some(index) = self.projectiles.iter().position(|projectile| {
                projectile.presentation.identity == result.projectile
                    && matches!(
                        projectile.presentation.kind,
                        ProjectileKind::Rocket | ProjectileKind::Syringe
                    )
                    && projectile.presentation.state == ProjectileState::Flying
            }) else {
                return Err(Error::InvalidProjectilePhysics);
            };
            let mut result = *result;
            if self.projectiles[index].presentation.kind == ProjectileKind::Syringe
                && !result.sky
                && let Some((identity, impact, normal)) =
                    self.enemy_bot_trace(request.start, result.end, 0.0)
            {
                result.end = impact;
                result.solid = true;
                result.sky = false;
                result.normal = Some(normal);
                result.direct_target = Some(identity);
            }
            let mut projectile = self.projectiles.remove(index);
            let mut actor_hit = self.bots.as_ref().and_then(|bots| {
                bots.intersect_enemy(
                    projectile.presentation.team,
                    request.start,
                    result.end,
                    projectile.presentation.owner_identity,
                )
            });
            if self.lifecycle == PlayerLifecycle::Active
                && projectile
                    .presentation
                    .team
                    .is_enemy(self.team_selection.local_team())
                && let Some(fraction) =
                    bot::segment_player(request.start, result.end, self.movement.position)
                && actor_hit.is_none_or(|(_, prior, _)| fraction < prior)
            {
                actor_hit = Some((
                    PLAYER_IDENTITY,
                    fraction,
                    add(
                        request.start,
                        scale(sub(result.end, request.start), fraction),
                    ),
                ));
            }
            if let Some((identity, _, position)) = actor_hit {
                result.end = position;
                result.solid = true;
                result.sky = false;
                result.direct_target = Some(identity);
                result.normal = Some(scale(normalized(projectile.presentation.velocity), -1.0));
            }
            projectile.presentation.position = result.end;
            projectile.direct_target = result.direct_target;
            if result.sky {
                projectile_events.push(projectile_event(
                    ProjectileEventKind::Fizzle,
                    &projectile.presentation,
                    self.tick,
                ));
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
                if projectile.presentation.kind == ProjectileKind::Syringe {
                    projectile_events.push(projectile_event(
                        ProjectileEventKind::Fizzle,
                        &projectile.presentation,
                        self.tick,
                    ));
                    if let Some(target) = result.direct_target {
                        if self.bots.as_ref().is_some_and(|bots| bots.contains(target)) {
                            self.apply_actor_damage(
                                bot::Damage {
                                    attacker: projectile.presentation.owner_identity,
                                    victim: target,
                                    weapon: Weapon::SyringeGun,
                                    amount: medic::SYRINGE_DAMAGE as f32,
                                    position: result.end,
                                },
                                projectile.presentation.team,
                                events,
                            )?;
                        } else {
                            map_phase.append(self.map.damage(self.tick, target)?);
                        }
                    }
                    continue;
                }
                if let Some(target) = result.direct_target {
                    let (presentation, killing_weapon) = self.publish_explosion(projectile, projectile_events);
                    if target != PLAYER_IDENTITY
                        && !self.bots.as_ref().is_some_and(|bots| bots.contains(target))
                    {
                        map_phase.append(self.map.damage(self.tick, target)?);
                    }
                    self.apply_blast(&presentation, killing_weapon, Some(target), events)?;
                } else {
                    self.explode(projectile, projectile_events, events)?;
                }
            } else {
                self.projectiles.push(projectile);
            }
        }
        Ok(map_phase)
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
                    self.explode(projectile, projectile_events, events)?;
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
            if projectile.presentation.kind == ProjectileKind::Syringe {
                projectile.presentation.velocity[2] -= self.movement_configuration.gravity
                    * medic::SYRINGE_GRAVITY_SCALE
                    * tick_interval;
                projectile.presentation.orientation =
                    quaternion_from_direction(projectile.presentation.velocity);
            }
            self.rocket_trace_requests.push(RocketTraceRequest {
                projectile: projectile.presentation.identity,
                tick: self.tick,
                start: projectile.presentation.position,
                end,
                mask: rocket_flight_mask(projectile.presentation.team),
            });
            retained.push(projectile);
        }
        self.projectiles = retained;
        Ok(())
    }

    fn detonate(
        &mut self,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let mut retained = Vec::new();
        for projectile in std::mem::take(&mut self.projectiles) {
            let detonatable = self.tick.saturating_sub(projectile.creation_tick) as f32
                * self.movement_configuration.tick_interval
                >= 0.8;
            if projectile.presentation.kind == ProjectileKind::Sticky && detonatable {
                self.explode(projectile, projectile_events, events)?;
            } else {
                retained.push(projectile);
            }
        }
        self.projectiles = retained;
        Ok(())
    }

    fn explode(
        &mut self,
        projectile: LiveProjectile,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let direct = projectile.direct_target;
        let (presentation, killing_weapon) = self.publish_explosion(projectile, projectile_events);
        self.apply_blast(&presentation, killing_weapon, direct, events)
    }

    fn apply_blast(
        &mut self,
        projectile: &Projectile,
        killing_weapon: &'static str,
        direct: Option<u32>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let weapon = if projectile.kind != ProjectileKind::Rocket { Weapon::StickybombLauncher }
            else if projectile.owner_identity == PLAYER_IDENTITY && projectile.launcher_identity == Weapon::Original as u32 { Weapon::Original }
            else { Weapon::RocketLauncher };
        if projectile.owner_identity == PLAYER_IDENTITY {
            self.apply_self_blast(projectile, weapon, killing_weapon, events);
        } else if projectile.team.is_enemy(self.team_selection.local_team())
            && self.lifecycle == PlayerLifecycle::Active
        {
            let center = add(self.movement.position, [0.0, 0.0, 41.0]);
            let visible = self
                .collision
                .trace(
                    projectile.position,
                    center,
                    Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    MASK_SOLID_BRUSH_ONLY,
                )
                .is_ok_and(|trace| trace.fraction >= 1.0 || trace.start_solid);
            let kind = if projectile.kind == ProjectileKind::Rocket {
                combat::BlastKind::Rocket
            } else {
                combat::BlastKind::Sticky
            };
            if let Some(damage) = combat::player_blast_damage(
                kind,
                projectile.position,
                combat::PlayerBlastTarget {
                    origin: self.movement.position,
                    world_center: center,
                    direct_hit: direct == Some(PLAYER_IDENTITY),
                    visible,
                    self_damage: false,
                },
            ) {
                self.apply_actor_damage_from(
                    bot::Damage {
                        attacker: projectile.owner_identity,
                        victim: PLAYER_IDENTITY,
                        weapon,
                        amount: damage.damage,
                        position: projectile.position,
                    },
                    projectile.team,
                    Some(killing_weapon),
                    Some(0),
                    events,
                )?;
            }
        }
        let victims = self
            .bots
            .as_ref()
            .map_or_else(Vec::new, |bots| bots.snapshots());
        for victim in victims {
            if victim.lifecycle != PlayerLifecycle::Active
                || !(projectile.team.is_enemy(victim.team)
                    || victim.identity == projectile.owner_identity)
            {
                continue;
            }
            let center = add(victim.position, [0.0, 0.0, 41.0]);
            let visible = self
                .collision
                .trace(
                    projectile.position,
                    center,
                    Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    MASK_SOLID_BRUSH_ONLY,
                )
                .is_ok_and(|trace| trace.fraction >= 1.0 || trace.start_solid);
            let kind = if projectile.kind == ProjectileKind::Rocket {
                combat::BlastKind::Rocket
            } else {
                combat::BlastKind::Sticky
            };
            if let Some(mut damage) = combat::player_blast_damage(
                kind,
                projectile.position,
                combat::PlayerBlastTarget {
                    origin: victim.position,
                    world_center: center,
                    direct_hit: direct == Some(victim.identity),
                    visible,
                    self_damage: victim.identity == projectile.owner_identity,
                },
            ) {
                if victim.identity == projectile.owner_identity
                    && victim.class == PlayerClass::Soldier
                {
                    damage = combat::apply_self_damage_rules(
                        damage,
                        combat::BlastClass::Soldier,
                        victim.velocity[2] == 0.0,
                        false,
                    );
                }
                self.apply_actor_damage_from(
                    bot::Damage {
                        attacker: projectile.owner_identity,
                        victim: victim.identity,
                        weapon,
                        amount: damage.damage,
                        position: projectile.position,
                    },
                    projectile.team,
                    Some(killing_weapon),
                    Some(0),
                    events,
                )?;
            }
        }
        Ok(())
    }

    fn publish_explosion(
        &mut self,
        projectile: LiveProjectile,
        projectile_events: &mut Vec<ProjectileEvent>,
    ) -> (Projectile, &'static str) {
        let definition = match (
            projectile.presentation.kind,
            projectile.presentation.launcher_identity,
        ) {
            (ProjectileKind::Rocket, launcher) if launcher == Weapon::Original as u32 => {
                SoundDefinition::OriginalExplosion
            }
            (ProjectileKind::Rocket, _) => SoundDefinition::RocketExplosion,
            (ProjectileKind::Sticky, _) => SoundDefinition::StickyExplosion,
            (ProjectileKind::Syringe, _) => unreachable!("syringes do not explode"),
        };
        let sound_samples = self.sample_sound(
            RandomContext::PredictedPresentation,
            definition,
            SoundQueryPhase::Emit,
        );
        self.push_audio_event(AudioEvent {
            action: AudioAction::Play,
            tick: self.tick,
            ordinal: 0,
            identity: AudioEventIdentity::ExplosionSpecial1,
            definition,
            source_kind: AudioSourceKind::World,
            source_identity: projectile.presentation.identity,
            owner_identity: Some(projectile.presentation.owner_identity),
            position: projectile.presentation.position,
            samples: sound_samples,
        });
        let (base_damage, radius, self_radius) = match projectile.presentation.kind {
            ProjectileKind::Rocket => (90.0, 146.0, 121.0),
            ProjectileKind::Sticky => (120.0, 146.0, 146.0),
            ProjectileKind::Syringe => unreachable!("syringes do not cause blast damage"),
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
            self.physics_requests.retain(|request| {
                request.projectile != projectile.presentation.identity
                    || request.operation != ProjectilePhysicsOperation::Step
            });
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
        (projectile.presentation, projectile.killing_weapon)
    }

    fn apply_self_blast(&mut self, projectile: &Projectile, weapon: Weapon, killing_weapon: &'static str, events: &mut Vec<Event>) {
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
                projectile.position,
                center,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                self.movement_configuration.solid_mask,
            )
            .is_ok_and(|trace| trace.fraction == 1.0 || trace.start_solid);
        let kind = if projectile.kind == ProjectileKind::Rocket {
            combat::BlastKind::Rocket
        } else {
            combat::BlastKind::Sticky
        };
        let grounded = self.movement.ground.is_some();
        let class = match self.class {
            PlayerClass::Soldier => combat::BlastClass::Soldier,
            PlayerClass::Demoman => combat::BlastClass::Demoman,
            _ => return,
        };
        if let Some(base_damage) = combat::player_blast_damage(
            kind,
            projectile.position,
            combat::PlayerBlastTarget {
                origin: self.movement.position,
                world_center: center,
                direct_hit: false,
                visible,
                self_damage: true,
            },
        ) {
            let damage =
                combat::apply_self_damage_rules(base_damage, class, grounded, self.in_water);
            let health_before = self.health;
            self.health = self.health.saturating_sub(damage.health_points).max(0);
            events.push(Event::Damaged {
                amount: damage.health_points as f32,
                health: self.health as f32,
                origin: Some(projectile.position),
            });
            if health_before > 0 && self.health == 0 {
                events.push(Event::PlayerKilled { attacker: PLAYER_IDENTITY, victim: PLAYER_IDENTITY,
                    weapon: Some(weapon), killing_weapon,
                    assister: 0, damage_bits: 64, critical: false, custom: 0 });
            }
            let impulse = combat::self_blast_impulse(
                class,
                grounded,
                self.movement.crouch.uses_crouched_hull(),
                size,
                center,
                projectile.position,
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
        let mut retained = Vec::new();
        for projectile in std::mem::take(&mut self.projectiles) {
            if projectile.presentation.owner_identity != PLAYER_IDENTITY {
                retained.push(projectile);
                continue;
            }
            self.rocket_trace_requests
                .retain(|request| request.projectile != projectile.presentation.identity);
            if projectile.presentation.kind == ProjectileKind::Sticky {
                self.physics_requests.retain(|request| {
                    request.projectile != projectile.presentation.identity
                        || request.operation != ProjectilePhysicsOperation::Step
                });
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
        self.projectiles = retained;
    }

    fn die(&mut self, projectile_events: &mut Vec<ProjectileEvent>) {
        self.stop_flame(false);
        self.lifecycle = PlayerLifecycle::Dying;
        self.death_tick = Some(self.tick);
        self.scoreboard.local_death();
        if self.jump.is_none() {
            self.respawn_tick = self.bots.as_ref().and_then(|_| self.round.player_respawn_time(self.team_selection.local_team(), self.tick as f32 * self.movement_configuration.tick_interval)).map(|time| (time / self.movement_configuration.tick_interval).ceil().max(0.0) as u64);
        }
        self.lifecycle_events.push(LifecycleEvent {
            tick: self.tick,
            kind: LifecycleEventKind::Died,
            class: self.class,
            team: self.team_selection.local_team(),
        });
        self.fizzle_projectiles(projectile_events);
        self.fire_was_held = false;
        self.pending_melee_tick = None;
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
        // WeaponReset destroys sound patches without emitting a winddown.
        self.stop_flame(true);
        self.fizzle_projectiles(projectile_events);
        self.damagers = deathnotice::DamagerHistory::default();
        self.health = self.maximum_health();
        self.ammo = self.class.data().maximum_ammo;
        self.conditions.clear();
        self.ctf_capture_bonus_until = None;
        if let Some(state) = self.spy.as_mut() {
            *state = spy::SpyState::default();
        }
        self.lifecycle = PlayerLifecycle::Active;
        self.respawn_tick = None;
        self.death_tick = None;
        self.lifecycle_events.push(LifecycleEvent {
            tick: self.tick,
            kind: LifecycleEventKind::Respawned,
            class: self.class,
            team: self.team_selection.local_team(),
        });
        self.apply_equipment();
        for weapon in self.loadout.values_mut() {
            weapon.reset_for_spawn();
        }
        self.deploy_active_weapon();
        self.select_respawn_transform();
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
        self.snap_spawn_angles();
        self.air_dashes = 0;
        self.in_water = false;
        self.last_movement = None;
        self.fire_was_held = false;
        self.pending_melee_tick = None;
        if let Some(weapon) = self.weapon {
            self.activity_events.push(ActivityEvent {
                tick: self.tick,
                weapon,
                activity: weapon::WeaponActivity::Draw,
            });
        }
        events.push(Event::Respawned);
        events.push(Event::PlayerRespawned {
            player: PLAYER_IDENTITY,
            team: self.team_selection.local_team(),
        });
    }

    pub fn maximum_ammo(&self) -> class::AmmoLedger {
        let mut maximum = self.class.data().maximum_ammo;
        for runtime in self.loadout.values() {
            if let Some(kind) = weapon_ammo_kind(runtime.weapon) {
                maximum.set(kind, runtime.profile().maximum_reserve);
            }
        }
        maximum
    }

    fn maximum_health(&self) -> i32 {
        if self.jump.is_some() && self.class == PlayerClass::Soldier {
            900
        } else {
            self.class.data().maximum_health
        }
    }
}

pub(crate) const fn weapon_ammo_kind(weapon: Weapon) -> Option<class::AmmoType> {
    match weapon {
        Weapon::RocketLauncher
        | Weapon::Original
        | Weapon::Scattergun
        | Weapon::Minigun
        | Weapon::SniperRifle
        | Weapon::EngineerShotgun
        | Weapon::GrenadeLauncher
        | Weapon::Flamethrower
        | Weapon::SyringeGun => Some(class::AmmoType::Primary),
        Weapon::StickybombLauncher
        | Weapon::Pistol
        | Weapon::Shotgun
        | Weapon::HeavyShotgun
        | Weapon::Smg
        | Weapon::EngineerPistol
        | Weapon::Revolver => Some(class::AmmoType::Secondary),
        Weapon::Bat
        | Weapon::Shovel
        | Weapon::Fists
        | Weapon::Kukri
        | Weapon::Wrench
        | Weapon::Bottle
        | Weapon::FireAxe
        | Weapon::Knife
        | Weapon::Sapper
        | Weapon::DisguiseKit
        | Weapon::InvisibilityWatch
        | Weapon::BuildPda
        | Weapon::DestroyPda
        | Weapon::Toolbox
        | Weapon::MediGun
        | Weapon::Bonesaw => None,
    }
}

fn default_weapon(class: PlayerClass) -> Option<Weapon> {
    match class {
        PlayerClass::Scout => Some(Weapon::Scattergun),
        PlayerClass::Sniper => Some(Weapon::SniperRifle),
        PlayerClass::Soldier => Some(Weapon::RocketLauncher),
        PlayerClass::Demoman => Some(Weapon::GrenadeLauncher),
        PlayerClass::Heavy => Some(Weapon::Minigun),
        PlayerClass::Engineer => Some(Weapon::EngineerShotgun),
        PlayerClass::Pyro => Some(Weapon::Flamethrower),
        PlayerClass::Spy => Some(Weapon::Revolver),
        PlayerClass::Medic => Some(Weapon::SyringeGun),
    }
}

fn default_loadout(class: PlayerClass) -> BTreeMap<Weapon, WeaponRuntime> {
    equipment::Equipment::default().weapons(class)
        .map(|weapon| (weapon, WeaponRuntime::full(weapon))).collect()
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
    projectile_event_with_contact(kind, projectile, tick, projectile.contact_normal)
}

fn projectile_event_with_contact(
    kind: ProjectileEventKind,
    projectile: &Projectile,
    tick: u64,
    contact_normal: Option<[f32; 3]>,
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
        contact_normal,
        launcher_pose: None,
    }
}

fn ticks(seconds: f32, tick: f32) -> u64 {
    (seconds / tick).ceil() as u64
}

fn angle_vectors(pitch: f32, yaw: f32, roll: f32) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (pitch, yaw, roll) = (pitch.to_radians(), yaw.to_radians(), roll.to_radians());
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

fn rocket_flight_mask(team: PlayerTeam) -> u32 {
    MASK_SOLID
        | match team {
            PlayerTeam::Red => CONTENTS_BLUE_TEAM,
            PlayerTeam::Blue => CONTENTS_RED_TEAM,
            PlayerTeam::Unassigned | PlayerTeam::Spectator => 0,
        }
}

fn merge_mover_requests(current: &mut Vec<MoverRequest>, emitted: &[MoverRequest]) {
    for request in emitted {
        current.retain(|existing| {
            existing.entity != request.entity && existing.request_id != request.request_id
        });
        if request.speed > 0.0 || request.angular_velocity != [0.0; 3] {
            current.push(*request);
        }
    }
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

fn ray_box_fraction(start: [f32; 3], end: [f32; 3], mins: [f32; 3], maxs: [f32; 3]) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = 1.0_f32;
    for axis in 0..3 {
        let delta = end[axis] - start[axis];
        if delta.abs() < f32::EPSILON {
            if start[axis] < mins[axis] || start[axis] > maxs[axis] {
                return None;
            }
            continue;
        }
        let first = (mins[axis] - start[axis]) / delta;
        let second = (maxs[axis] - start[axis]) / delta;
        near = near.max(first.min(second));
        far = far.min(first.max(second));
        if near > far {
            return None;
        }
    }
    Some(near)
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
        Arc, Mutex,
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

    #[test]
    fn resupply_uses_resolved_weapon_ammo_caps_without_changing_stock_class_ledgers() {
        let mut session = Session::new(Floor, [0.0, 0.0, 1.0], MapRuntime::empty(0.015));
        for class in PlayerClass::ALL {
            session.class = class;
            session.loadout = default_loadout(class);
            assert_eq!(session.maximum_ammo(), class.data().maximum_ammo);
        }
        session.class = PlayerClass::Soldier;
        session.loadout = default_loadout(PlayerClass::Soldier);
        let runtime = session.loadout.get_mut(&Weapon::RocketLauncher).unwrap();
        runtime.resolved_profile.maximum_reserve = 60;
        runtime.resolved_profile.maximum_clip = 5;
        runtime.clip = 0;
        runtime.reserve = 0;
        session.ammo.primary = 0;
        session.regenerate(0, None, &mut Vec::new());
        assert_eq!(session.ammo.primary, 60);
        assert_eq!(session.ammo.secondary, 32);
        let runtime = session.weapon_runtime(Weapon::RocketLauncher).unwrap();
        assert_eq!((runtime.clip, runtime.reserve), (5, 60));
    }

    #[test]
    fn owned_snapshot_transactions_preserve_randomized_state_events_and_rollback() {
        let mut live = Session::new(Floor, [0.0, 0.0, 1.0], MapRuntime::empty(0.015));
        let mut owned = live.clone();
        let mut seed = 0x74c1_2fe3_u32;
        for tick in 0..500 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let command = Command {
                fire: seed & 1 != 0,
                reload: seed & 8 != 0,
                respawn: tick % 37 == 0,
                select_class: (tick % 31 == 0).then_some(match seed % 9 {
                    0 => PlayerClass::Scout,
                    1 => PlayerClass::Sniper,
                    2 => PlayerClass::Soldier,
                    3 => PlayerClass::Demoman,
                    4 => PlayerClass::Medic,
                    5 => PlayerClass::Heavy,
                    6 => PlayerClass::Pyro,
                    7 => PlayerClass::Spy,
                    _ => PlayerClass::Engineer,
                }),
                ..Command::default()
            };
            let before = live.producer_snapshot();
            match (
                live.advance(command),
                owned.into_advanced(command, &[], &[], None),
            ) {
                (Ok(expected), Ok((next, actual))) => {
                    assert_eq!(actual, expected);
                    assert_eq!(next.producer_snapshot(), live.producer_snapshot());
                    assert_eq!(next.random_state(), live.random_state());
                    assert_eq!(next.audio_events(), live.audio_events());
                    assert_eq!(next.random_draws(), live.random_draws());
                    owned = next;
                }
                (Err(expected), Err(actual)) => {
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                    assert_eq!(live.producer_snapshot(), before);
                    owned = live.clone();
                }
                _ => panic!("owned and borrowed transactions differ"),
            }
        }
    }

    #[test]
    fn authored_map_pickups_restore_exact_resources_and_materialize_after_ten_seconds() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"item_healthkit_medium\"\"origin\"\"0 0 1\"}\n{\"classname\"\"item_ammopack_small\"\"origin\"\"80 0 1\"}\n{\"classname\"\"item_healthkit_full\"\"origin\"\"160 0 1\"\"TeamNum\"\"3\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut session = Session::new(
            Floor,
            [0.0, 0.0, 1.0],
            MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap(),
        );
        session.health = 50;
        let picked = session.advance(Command::default()).unwrap();
        assert_eq!(picked.health, 150.0);
        assert!(matches!(
            picked.events.as_slice(),
            [Event::PickedUp {
                entity: 0,
                player: PLAYER_IDENTITY,
                kind: pickup::MapPickupKind::Health,
                size: pickup::PickupSize::Medium,
                amount: 100,
                ..
            }]
        ));
        assert!(!picked.pickups[0].available);
        assert_eq!(picked.pickups[0].respawn_tick, Some(667));
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::HealthKitTouch
        );
        session.movement.position = [80.0, 0.0, 1.0];
        session
            .loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .reserve = 2;
        session.loadout.get_mut(&Weapon::Shotgun).unwrap().reserve = 1;
        let ammo = session.advance(Command::default()).unwrap();
        assert_eq!(session.loadout[&Weapon::RocketLauncher].reserve, 6);
        assert_eq!(session.loadout[&Weapon::Shotgun].reserve, 8);
        assert!(ammo.events.iter().any(|event| matches!(
            event,
            Event::PickedUp {
                entity: 1,
                kind: pickup::MapPickupKind::Ammo,
                ..
            }
        )));
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::AmmoPackTouch
        );
        session.movement.position = [160.0, 0.0, 1.0];
        let denied = session.advance(Command::default()).unwrap();
        assert!(denied.pickups[2].available);
        assert!(
            !denied
                .events
                .iter()
                .any(|event| matches!(event, Event::PickedUp { entity: 2, .. }))
        );
        session.movement.position = [400.0, 0.0, 1.0];
        while session.tick < 667 {
            session.advance(Command::default()).unwrap();
        }
        let respawned = session.advance(Command::default()).unwrap();
        assert!(respawned.pickups[0].available);
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::ItemMaterialize)
        );
    }

    #[test]
    fn pickup_inputs_preserve_disabled_and_manual_materialization_contracts() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"item_healthkit_small\"\"origin\"\"0 0 1\"\"StartDisabled\"\"1\"\"AutoMaterialize\"\"0\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut session = Session::new(
            Floor,
            [0.0, 0.0, 1.0],
            MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap(),
        );
        session.health = 100;
        let disabled = session.advance(Command::default()).unwrap();
        assert!(!disabled.pickups[0].available);
        session
            .map_input(0, b"Enable", playsrc_entity::Variant::Void)
            .unwrap();
        let consumed = session.advance(Command::default()).unwrap();
        assert_eq!(consumed.health, 140.0);
        assert!(!consumed.pickups[0].available);
        session.movement.position = [200.0, 0.0, 1.0];
        for _ in 0..670 {
            session.advance(Command::default()).unwrap();
        }
        assert!(!session.map.pickups()[0].available);
        session
            .map_input(0, b"Enable", playsrc_entity::Variant::Void)
            .unwrap();
        assert!(session.map.pickups()[0].available);
        session
            .map_input(0, b"Disable", playsrc_entity::Variant::Void)
            .unwrap();
        assert!(!session.map.pickups()[0].available);
    }

    #[test]
    fn health_pickups_preserve_overheal_and_cleanse_conditions_without_inventing_health() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"item_healthkit_small\"\"origin\"\"0 0 1\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut session = Session::new(
            Floor,
            [0.0, 0.0, 1.0],
            MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap(),
        );
        session.health = 260;
        let full = session.advance(Command::default()).unwrap();
        assert_eq!(full.health, 260.0);
        assert!(full.pickups[0].available);
        session.conditions.insert(Condition::Burning);
        let cleansed = session.advance(Command::default()).unwrap();
        assert_eq!(cleansed.health, 260.0);
        assert!(!session.conditions.contains(Condition::Burning));
        assert!(!cleansed.pickups[0].available);
        assert!(
            cleansed
                .events
                .iter()
                .any(|event| matches!(event, Event::PickedUp { amount: 0, .. }))
        );
    }

    #[test]
    fn connected_session_starts_unassigned_and_selects_authoritative_teams() {
        let mut session = Session::connected(
            Floor,
            [0.0; 3],
            MapRuntime::empty(0.015),
            team_selection::TeamRules::default(),
        );
        let initial = session.team_snapshot();
        assert_eq!(initial.local_team, class::PlayerTeam::Unassigned);
        assert_eq!((initial.red_count, initial.blue_count), (0, 0));
        assert!(!initial.cancel_visible);
        assert_eq!(
            session.select_team_choice(team_selection::TeamChoice::Blue),
            Ok(Some(class::PlayerTeam::Blue))
        );
        let selected = session.team_snapshot();
        assert_eq!(selected.local_team, class::PlayerTeam::Blue);
        assert_eq!((selected.red_count, selected.blue_count), (0, 1));
        assert!(selected.cancel_visible);
        let first = session.advance(Command::default()).unwrap();
        assert_eq!(first.team, PlayerTeam::Blue);
        assert!(first.events.contains(&Event::TeamChanged(PlayerTeam::Blue)));
        assert!(
            session
                .producer_snapshot()
                .lifecycle_events
                .iter()
                .any(|event| event.kind == LifecycleEventKind::TeamChanged
                    && event.team == PlayerTeam::Blue)
        );
    }

    #[test]
    fn team_selection_spawns_at_the_first_enabled_authored_point_for_each_team() {
        let graph = playsrc_entity::parse(
            br#"
{"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "10 20 1" "StartDisabled" "1"}
{"classname" "info_player_teamspawn" "TeamNum" "3" "origin" "300 400 1" "StartDisabled" "1"}
{"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "100 200 1"}
{"classname" "info_player_teamspawn" "TeamNum" "3" "origin" "500 600 1"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap();
        let mut session = Session::connected(
            Floor,
            [10.0, 20.0, 1.0],
            map,
            team_selection::TeamRules::default(),
        );
        session
            .select_team_choice(team_selection::TeamChoice::Red)
            .unwrap();
        assert_eq!(session.movement.position, [100.0, 200.0, 1.0]);
        session.advance(Command::default()).unwrap();
        session.ammo.metal = 20;
        session
            .select_team_choice(team_selection::TeamChoice::Blue)
            .unwrap();
        assert_eq!(session.movement.position, [500.0, 600.0, 1.0]);
        assert_eq!(session.ammo.metal, session.class.data().maximum_ammo.metal);
        assert_eq!(
            session.advance(Command::default()).unwrap().team,
            PlayerTeam::Blue
        );
    }

    #[test]
    fn authored_spawn_angles_reset_team_class_and_respawn_without_losing_later_input() {
        let graph = playsrc_entity::parse(br#"
{"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "100 200 1" "angles" "-5 180 0"}
{"classname" "info_player_teamspawn" "TeamNum" "3" "origin" "500 600 1" "angles" "7 330 0"}
"#, playsrc_entity::Limits::default()).unwrap();
        let map = MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap();
        let mut session = Session::connected(Floor, [0.0; 3], map, team_selection::TeamRules::default());
        session.set_initial_view_angles([0.0, 330.0, 0.0]);
        let command = |yaw, pitch| Command { movement: MoveCommand { yaw_degrees: yaw, ..MoveCommand::default() }, pitch_degrees: pitch, ..Command::default() };
        session.select_team_choice(team_selection::TeamChoice::Red).unwrap();
        assert_eq!(session.movement.position, [100.0, 200.0, 1.0]);
        assert_eq!(session.movement.local_angles, [-5.0, 180.0, 0.0]);
        assert_eq!(session.movement.absolute_view_angles, [-5.0, 180.0, 0.0]);
        assert_eq!(session.advance(command(330.0, 0.0)).unwrap().movement.absolute_view_angles, [-5.0, 180.0, 0.0]);
        assert_eq!(session.advance(command(335.0, 2.0)).unwrap().movement.absolute_view_angles, [-3.0, 185.0, 0.0]);
        let changed = session.advance(Command { select_class: Some(PlayerClass::Medic), ..command(337.0, 4.0) }).unwrap();
        assert_eq!(changed.movement.absolute_view_angles, [-5.0, 180.0, 0.0]);
        let respawned = session.advance(Command { respawn: true, ..command(339.0, 3.0) }).unwrap();
        assert_eq!(respawned.movement.absolute_view_angles, [-5.0, 180.0, 0.0]);
        assert_eq!(session.advance(command(340.0, 2.0)).unwrap().movement.absolute_view_angles, [-6.0, 181.0, 0.0]);
        session.select_team_choice(team_selection::TeamChoice::Blue).unwrap();
        assert_eq!(session.movement.position, [500.0, 600.0, 1.0]);
        assert_eq!(session.movement.local_angles, [7.0, 330.0, 0.0]);
        assert_eq!(session.advance(command(341.0, 3.0)).unwrap().movement.absolute_view_angles, [8.0, 331.0, 0.0]);
    }

    #[test]
    fn spectator_team_change_drops_the_carried_enemy_intelligence_before_publication() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"item_teamflag\"\"TeamNum\"\"2\"\"origin\"\"100 0 8\"}{\"classname\"\"item_teamflag\"\"TeamNum\"\"3\"\"origin\"\"0 0 8\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap();
        let mut session =
            Session::connected(Floor, [0.0; 3], map, team_selection::TeamRules::default());
        session
            .select_team_choice(team_selection::TeamChoice::Red)
            .unwrap();
        let stolen = session.advance(Command::default()).unwrap();
        assert_eq!(
            stolen
                .objectives
                .as_ref()
                .unwrap()
                .flags
                .iter()
                .find(|flag| flag.team == PlayerTeam::Blue)
                .unwrap()
                .carrier,
            Some(PLAYER_IDENTITY)
        );
        session
            .select_team_choice(team_selection::TeamChoice::Spectator)
            .unwrap();
        let observed = session.advance(Command::default()).unwrap();
        assert_eq!(observed.team, PlayerTeam::Spectator);
        let objectives = observed.objectives.unwrap();
        assert_eq!(
            objectives
                .flags
                .iter()
                .find(|flag| flag.team == PlayerTeam::Blue)
                .unwrap()
                .status,
            ctf::FlagStatus::Dropped
        );
        assert!(objectives.events.iter().any(|event| matches!(
            event,
            ctf::Event::Flag {
                kind: ctf::FlagEventKind::Dropped,
                player: Some(PLAYER_IDENTITY),
                ..
            }
        )));
    }

    #[test]
    fn spectator_selection_owns_observer_lifecycle_without_weapons_or_health() {
        let mut session = Session::connected(
            Floor,
            [0.0; 3],
            MapRuntime::empty(0.015),
            team_selection::TeamRules::default(),
        );
        assert_eq!(session.lifecycle, PlayerLifecycle::Welcome);
        assert!(session.weapon.is_none());
        assert_eq!(
            session.select_team_choice(team_selection::TeamChoice::Spectator),
            Ok(Some(PlayerTeam::Spectator))
        );
        assert_eq!(session.lifecycle, PlayerLifecycle::Observer);
        assert_eq!(session.team_selection.local_team(), PlayerTeam::Spectator);
        assert!(session.weapon.is_none() && session.loadout.is_empty());
        assert_eq!(session.health, 0);
        let snapshot = session.advance(Command::default()).unwrap();
        assert_eq!(snapshot.team, PlayerTeam::Spectator);
        assert!(snapshot.weapon.is_none());
    }

    #[test]
    fn auto_assign_consumes_randomness_only_for_equal_unrestricted_teams() {
        let mut session = Session::connected(
            Floor,
            [0.0; 3],
            MapRuntime::empty(0.015),
            team_selection::TeamRules::default(),
        );
        let before = session.random_state().authority;
        let selected = session
            .select_team_choice(team_selection::TeamChoice::Auto)
            .unwrap()
            .unwrap();
        assert!(selected.is_gameplay());
        assert_ne!(session.random_state().authority, before);
    }

    #[derive(Clone)]
    struct MeleeWall;

    impl Tracer for MeleeWall {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            hull: Hull,
            mask: u32,
        ) -> Result<Trace, MoveError> {
            if start[2] > 40.0 && start[0] < 32.0 && end[0] >= 32.0 {
                let fraction = (32.0 - start[0]) / (end[0] - start[0]);
                return Ok(Trace {
                    fraction,
                    start_solid: false,
                    all_solid: false,
                    end: [
                        32.0,
                        start[1] + (end[1] - start[1]) * fraction,
                        start[2] + (end[2] - start[2]) * fraction,
                    ],
                    normal: Some([-1.0, 0.0, 0.0]),
                    hit: Some(0),
                    contents: 1,
                });
            }
            Floor.trace(start, end, hull, mask)
        }
    }

    impl GameplayWorld for MeleeWall {
        fn overlaps_model_hull(
            &self,
            model: usize,
            origin: [f32; 3],
            position: [f32; 3],
            hull: Hull,
        ) -> Result<bool, MoveError> {
            Floor.overlaps_model_hull(model, origin, position, hull)
        }
    }

    #[derive(Clone)]
    struct WaterFloor {
        surface: f32,
    }

    impl Tracer for WaterFloor {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            hull: Hull,
            mask: u32,
        ) -> Result<Trace, MoveError> {
            Floor.trace(start, end, hull, mask)
        }

        fn point_contents(&self, point: [f32; 3]) -> Result<u32, MoveError> {
            Ok(if point[2] < self.surface {
                playsrc_movement::CONTENTS_WATER
            } else {
                0
            })
        }
    }

    impl GameplayWorld for WaterFloor {
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

    type RecordedTrace = ([f32; 3], [f32; 3], Hull, u32);

    #[derive(Clone, Default)]
    struct RecordingWorld {
        traces: Arc<Mutex<Vec<RecordedTrace>>>,
    }

    impl Tracer for RecordingWorld {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            hull: Hull,
            mask: u32,
        ) -> Result<Trace, MoveError> {
            self.traces.lock().unwrap().push((start, end, hull, mask));
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

    impl GameplayWorld for RecordingWorld {
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

    #[test]
    fn random_class_selection_uses_authority_random_and_excludes_the_current_class() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        let original = session.class;
        let before = session.random_state();
        let snapshot = session
            .advance(Command {
                select_random_class: true,
                ..Command::default()
            })
            .unwrap();
        assert_ne!(snapshot.class, original);
        assert!(PlayerClass::ALL.contains(&snapshot.class));
        assert_ne!(session.random_state().authority, before.authority);
        assert!(session.random_draws().iter().any(|draw| {
            draw.context == RandomContext::Authority
                && draw.decision == RandomDecision::ClassSelection
        }));
    }

    #[test]
    fn spy_stock_loadout_cloak_disguise_fire_and_immediate_knife_preserve_source_order() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        let selected = session
            .advance(Command {
                select_class: Some(PlayerClass::Spy),
                ..Command::default()
            })
            .unwrap();
        assert_eq!(selected.weapon, Some(Weapon::Revolver));
        assert_eq!(
            selected
                .loadout
                .iter()
                .map(|state| state.weapon)
                .collect::<Vec<_>>(),
            vec![
                Weapon::Revolver,
                Weapon::Knife,
                Weapon::Sapper,
                Weapon::DisguiseKit,
                Weapon::InvisibilityWatch,
            ]
        );
        let cloak = session
            .advance(Command {
                detonate: true,
                ..Command::default()
            })
            .unwrap();
        assert!(session.conditions.contains(Condition::Stealthed));
        assert!(cloak.spy.unwrap().cloak_meter < 100.0);
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::SpyCloak)
        );
        session.advance(Command::default()).unwrap();
        while session.spy.unwrap().next_stealth_time
            > session.tick as f32 * session.movement_configuration.tick_interval
        {
            session.advance(Command::default()).unwrap();
        }
        session
            .advance(Command {
                detonate: true,
                ..Command::default()
            })
            .unwrap();
        assert!(!session.conditions.contains(Condition::Stealthed));
        session.advance(Command::default()).unwrap();
        let request = session
            .advance(Command {
                disguise: Some(spy::Disguise {
                    class: PlayerClass::Soldier,
                    team: PlayerTeam::Blue,
                }),
                ..Command::default()
            })
            .unwrap();
        assert!(request.spy.unwrap().desired_disguise.is_some());
        while !session.conditions.contains(Condition::Disguised) {
            session.advance(Command::default()).unwrap();
        }
        session
            .advance(Command {
                select_weapon: Some(Weapon::Knife),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Knife)
            .unwrap()
            .next_primary_tick = session.tick;
        let stabbed = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(stabbed
            .events
            .iter()
            .any(|event| matches!(event, Event::MeleeImpact { weapon: Weapon::Knife, damage, .. } if *damage == 40.0)));
        assert!(!session.conditions.contains(Condition::Disguised));
        assert!(session.conditions.contains(Condition::DisguiseWearingOff));
    }

    #[test]
    fn scout_scattergun_and_pistol_publish_exact_hitscan_pellets_without_projectiles() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Scout),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Scattergun)
            .unwrap()
            .next_primary_tick = 0;
        let scatter = session
            .advance(Command {
                fire: true,
                pitch_degrees: 60.0,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(scatter.weapon, Some(Weapon::Scattergun));
        assert_eq!(
            scatter
                .loadout
                .iter()
                .find(|item| item.weapon == Weapon::Scattergun)
                .unwrap()
                .clip,
            5
        );
        assert!(scatter.projectiles.is_empty());
        assert!(scatter.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::Scattergun,
                pellets: 10,
                ..
            }
        )));
        assert_eq!(
            scatter
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    Event::HitscanImpact {
                        weapon: Weapon::Scattergun,
                        target: None,
                        ..
                    }
                ))
                .count(),
            10
        );
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::ScattergunSingle)
        );

        session
            .advance(Command {
                select_weapon: Some(Weapon::Pistol),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Pistol)
            .unwrap()
            .next_primary_tick = 0;
        let pistol = session
            .advance(Command {
                fire: true,
                pitch_degrees: 60.0,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            pistol
                .loadout
                .iter()
                .find(|item| item.weapon == Weapon::Pistol)
                .unwrap()
                .clip,
            11
        );
        assert!(pistol.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::Pistol,
                pellets: 1,
                ..
            }
        )));
        assert_eq!(
            pistol
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    Event::HitscanImpact {
                        weapon: Weapon::Pistol,
                        target: None,
                        ..
                    }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn heavy_stock_weapons_preserve_spin_conditions_audio_ammo_and_delayed_fist_contact() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        let selected = session
            .advance(Command {
                select_class: Some(PlayerClass::Heavy),
                ..Command::default()
            })
            .unwrap();
        assert_eq!(selected.weapon, Some(Weapon::Minigun));
        assert_eq!(
            selected
                .loadout
                .iter()
                .map(|item| item.weapon)
                .collect::<Vec<_>>(),
            vec![Weapon::Minigun, Weapon::HeavyShotgun, Weapon::Fists]
        );
        session
            .loadout
            .get_mut(&Weapon::Minigun)
            .unwrap()
            .next_primary_tick = 0;

        session
            .advance(Command {
                detonate: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            session.loadout[&Weapon::Minigun].minigun_state,
            weapon::MinigunState::Starting
        );
        assert!(session.conditions.contains(Condition::Aiming));
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::MinigunWindUp)
        );
        while session.loadout[&Weapon::Minigun].minigun_state == weapon::MinigunState::Starting {
            session
                .advance(Command {
                    detonate: true,
                    ..Command::default()
                })
                .unwrap();
        }
        assert_eq!(
            session.loadout[&Weapon::Minigun].minigun_state,
            weapon::MinigunState::Spinning
        );
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::MinigunSpin)
        );
        let mut fire_sound = false;
        while session.loadout[&Weapon::Minigun].reserve == 200 {
            let snapshot = session
                .advance(Command {
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
            assert!(snapshot.projectiles.is_empty());
            fire_sound |= session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::MinigunFire);
        }
        assert_eq!(session.loadout[&Weapon::Minigun].reserve, 199);
        assert!(fire_sound);
        while session.loadout[&Weapon::Minigun].minigun_state != weapon::MinigunState::Idle {
            session.advance(Command::default()).unwrap();
        }
        assert!(!session.conditions.contains(Condition::Aiming));
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::MinigunWindDown)
        );

        session
            .advance(Command {
                select_weapon: Some(Weapon::HeavyShotgun),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::HeavyShotgun)
            .unwrap()
            .next_primary_tick = 0;
        let shotgun = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(shotgun.projectiles.is_empty());
        assert!(shotgun.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::HeavyShotgun,
                pellets: 10,
                ..
            }
        )));
        assert_eq!(session.loadout[&Weapon::HeavyShotgun].clip, 5);
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::ShotgunSingle)
        );

        session
            .advance(Command {
                select_weapon: Some(Weapon::Fists),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Fists)
            .unwrap()
            .next_primary_tick = 0;
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::FistMiss)
        );
        let due = session.loadout[&Weapon::Fists].smack_due_tick.unwrap();
        while session.loadout[&Weapon::Fists].smack_due_tick.is_some() {
            session.advance(Command::default()).unwrap();
            if session.loadout[&Weapon::Fists].smack_due_tick.is_some() {
                assert!(session.tick <= due + 1);
            }
        }
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::FistHitWorld)
        );
    }

    #[test]
    fn soldier_shotgun_fires_ten_independent_source_pellets_without_scattergun_ramp() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_weapon: Some(Weapon::Shotgun),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Shotgun)
            .unwrap()
            .next_primary_tick = 0;
        let snapshot = session
            .advance(Command {
                fire: true,
                pitch_degrees: 60.0,
                ..Command::default()
            })
            .unwrap();
        assert!(snapshot.projectiles.is_empty());
        assert!(snapshot.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::Shotgun,
                pellets: 10,
                ..
            }
        )));
        let impacts = snapshot
            .events
            .iter()
            .filter_map(|event| match event {
                Event::HitscanImpact {
                    weapon: Weapon::Shotgun,
                    damage,
                    ..
                } => Some(*damage),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(impacts.len(), 10);
        assert!(impacts.iter().all(|damage| *damage <= 9.0));
        let weapon = snapshot
            .loadout
            .iter()
            .find(|item| item.weapon == Weapon::Shotgun)
            .unwrap();
        assert_eq!((weapon.clip, weapon.reserve), (5, 32));
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::ShotgunSingle)
        );
    }

    #[test]
    fn soldier_shovel_delays_world_impact_and_preserves_zero_ammunition() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_weapon: Some(Weapon::Shovel),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Shovel)
            .unwrap()
            .next_primary_tick = 0;
        let started = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(
            !started
                .events
                .iter()
                .any(|event| matches!(event, Event::MeleeImpact { .. }))
        );
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::ShovelMiss)
        );
        let swing_tick = started.tick;
        while session.pending_melee_tick.is_some() {
            let snapshot = session.advance(Command::default()).unwrap();
            if session.pending_melee_tick.is_none() {
                assert_eq!(snapshot.tick - swing_tick, 14);
                assert!(snapshot.events.iter().any(|event| matches!(event,
                    Event::MeleeImpact { weapon: Weapon::Shovel, target: None, damage, .. }
                    if *damage == 65.0
                )));
                assert!(
                    session
                        .audio_events()
                        .iter()
                        .any(|event| event.definition == SoundDefinition::ShovelHitWorld)
                );
            }
        }
        let weapon = session.weapon_runtime(Weapon::Shovel).unwrap();
        assert_eq!((weapon.clip, weapon.reserve), (0, 0));
    }

    #[test]
    fn demoman_bottle_delays_authored_world_impact_and_preserves_zero_ammunition() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Demoman),
                select_weapon: Some(Weapon::Bottle),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Bottle)
            .unwrap()
            .next_primary_tick = 0;
        let started = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::BottleMiss
        );
        assert!(
            !started
                .events
                .iter()
                .any(|event| matches!(event, Event::MeleeImpact { .. }))
        );
        let swing_tick = started.tick;
        while session.pending_melee_tick.is_some() {
            let snapshot = session.advance(Command::default()).unwrap();
            if session.pending_melee_tick.is_none() {
                assert_eq!(snapshot.tick - swing_tick, 14);
                assert!(snapshot.events.iter().any(|event| matches!(event,
                    Event::MeleeImpact { weapon: Weapon::Bottle, target: None, damage, .. }
                    if *damage == 65.0
                )));
                assert_eq!(
                    session.audio_events()[0].definition,
                    SoundDefinition::BottleHitWorld
                );
            }
        }
        assert_eq!(
            (
                session.weapon_runtime(Weapon::Bottle).unwrap().clip,
                session.weapon_runtime(Weapon::Bottle).unwrap().reserve
            ),
            (0, 0)
        );
    }

    #[test]
    fn demoman_bottle_traces_enemy_line_then_hull_without_hitting_friendly_players() {
        for (lateral, enemy) in [(0.0, true), (20.0, true), (0.0, false)] {
            let world = RecordingWorld::default();
            let traces = world.traces.clone();
            let mut session = Session::new(world, [0.0; 3], MapRuntime::empty(0.015));
            session.movement_configuration.gravity = 0.0;
            session
                .advance(Command {
                    select_class: Some(PlayerClass::Demoman),
                    select_weapon: Some(Weapon::Bottle),
                    ..Command::default()
                })
                .unwrap();
            session.set_posed_player_hitboxes(vec![PosedPlayerHitbox {
                entity: 7,
                team: if enemy {
                    PlayerTeam::Blue
                } else {
                    PlayerTeam::Red
                },
                hitbox: 0,
                group: 2,
                bone: 0,
                physics_bone: 0,
                bone_contents: 0x0200_0000,
                minimum: [-3.0; 3],
                maximum: [3.0; 3],
                bone_to_world: [
                    1.0, 0.0, 0.0, 30.0, 0.0, 1.0, 0.0, lateral, 0.0, 0.0, 1.0, 68.0,
                ],
                origin: [30.0, lateral, 0.0],
            }]);
            session
                .loadout
                .get_mut(&Weapon::Bottle)
                .unwrap()
                .next_primary_tick = 0;
            session
                .advance(Command {
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
            traces.lock().unwrap().clear();
            let mut impact = None;
            while session.pending_melee_tick.is_some() {
                let snapshot = session.advance(Command::default()).unwrap();
                impact = snapshot
                    .events
                    .into_iter()
                    .find(|event| matches!(event, Event::MeleeImpact { .. }));
            }
            if enemy {
                assert!(
                    matches!(impact,
                        Some(Event::MeleeImpact { weapon: Weapon::Bottle, target: Some(7), damage, .. })
                        if damage == 65.0
                    ),
                    "lateral={lateral}, impact={impact:?}"
                );
                assert_eq!(
                    session.audio_events()[0].definition,
                    SoundDefinition::BottleHitFlesh
                );
                let melee = traces.lock().unwrap();
                let swings = melee
                    .iter()
                    .filter(|(_, _, _, mask)| *mask == MASK_SOLID)
                    .collect::<Vec<_>>();
                assert_eq!(swings.len(), if lateral == 0.0 { 1 } else { 2 });
                assert_eq!(swings[0].2.mins, [0.0; 3]);
                if lateral != 0.0 {
                    assert_eq!(swings[1].2.mins, [-18.0; 3]);
                }
            } else {
                assert!(impact.is_none());
                assert!(session.audio_events().is_empty());
            }
        }
    }

    #[test]
    fn demoman_bottle_does_not_damage_an_enemy_behind_world_geometry() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Demoman),
                select_weapon: Some(Weapon::Bottle),
                ..Command::default()
            })
            .unwrap();
        session.set_posed_player_hitboxes(vec![PosedPlayerHitbox {
            entity: 7,
            team: PlayerTeam::Blue,
            hitbox: 0,
            group: 2,
            bone: 0,
            physics_bone: 0,
            bone_contents: 0x0200_0000,
            minimum: [-2.0; 3],
            maximum: [2.0; 3],
            bone_to_world: [1.0, 0.0, 0.0, 40.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 68.0],
            origin: [40.0, 0.0, 0.0],
        }]);
        session
            .loadout
            .get_mut(&Weapon::Bottle)
            .unwrap()
            .next_primary_tick = 0;
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        while session.pending_melee_tick.is_some() {
            let snapshot = session.advance(Command::default()).unwrap();
            if session.pending_melee_tick.is_none() {
                assert!(snapshot.events.iter().any(|event| matches!(
                    event,
                    Event::MeleeImpact {
                        weapon: Weapon::Bottle,
                        target: None,
                        ..
                    }
                )));
                assert_eq!(
                    session.audio_events()[0].definition,
                    SoundDefinition::BottleHitWorld
                );
            }
        }
    }

    #[test]
    fn engineer_stock_firearms_preserve_distinct_item_profiles_and_damage_ramp() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Engineer),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::EngineerShotgun)
            .unwrap()
            .next_primary_tick = 0;
        let shotgun = session
            .advance(Command {
                fire: true,
                pitch_degrees: 60.0,
                ..Command::default()
            })
            .unwrap();
        assert!(shotgun.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::EngineerShotgun,
                pellets: 10,
                ..
            }
        )));
        let pellets = shotgun
            .events
            .iter()
            .filter_map(|event| match event {
                Event::HitscanImpact {
                    weapon: Weapon::EngineerShotgun,
                    damage,
                    ..
                } => Some(*damage),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pellets.len(), 10);
        assert!(pellets.iter().all(|damage| *damage > 6.0 && *damage < 9.1));
        assert_eq!(
            session
                .weapon_runtime(Weapon::EngineerShotgun)
                .unwrap()
                .reserve,
            32
        );
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::ShotgunSingle
        );

        session
            .advance(Command {
                select_weapon: Some(Weapon::EngineerPistol),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::EngineerPistol)
            .unwrap()
            .next_primary_tick = 0;
        let pistol = session
            .advance(Command {
                fire: true,
                pitch_degrees: 60.0,
                ..Command::default()
            })
            .unwrap();
        assert!(pistol.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::EngineerPistol,
                pellets: 1,
                ..
            }
        )));
        let state = session.weapon_runtime(Weapon::EngineerPistol).unwrap();
        assert_eq!((state.clip, state.reserve), (11, 200));
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::PistolSingle
        );
    }

    #[test]
    fn engineer_wrench_checks_building_reach_before_the_delayed_world_smack() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Engineer),
                select_weapon: Some(Weapon::Wrench),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Wrench)
            .unwrap()
            .next_primary_tick = 0;
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::WrenchMiss
        );
        let mut hit = None;
        while session.pending_melee_tick.is_some() {
            hit = Some(session.advance(Command::default()).unwrap());
        }
        assert!(hit.unwrap().events.iter().any(|event| matches!(
            event,
            Event::MeleeImpact {
                weapon: Weapon::Wrench,
                target: None,
                damage,
                ..
            } if *damage == 65.0
        )));
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::WrenchHitWorld
        );
        assert_eq!(session.weapon_runtime(Weapon::Wrench).unwrap().clip, 0);
    }

    #[test]
    fn held_empty_engineer_pistol_emits_empty_then_reload_and_refills_all_rounds() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Engineer),
                select_weapon: Some(Weapon::EngineerPistol),
                ..Command::default()
            })
            .unwrap();
        let tick = session.tick;
        {
            let pistol = session.loadout.get_mut(&Weapon::EngineerPistol).unwrap();
            pistol.clip = 0;
            pistol.reserve = 12;
            pistol.next_primary_tick = tick;
        }
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::PistolEmpty
        );
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::PistolReload
        );
        let due = session
            .weapon_runtime(Weapon::EngineerPistol)
            .unwrap()
            .reload_due_tick
            .unwrap();
        while session.tick <= due {
            session
                .advance(Command {
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
        }
        let state = session.weapon_runtime(Weapon::EngineerPistol).unwrap();
        assert_eq!((state.clip, state.reserve), (12, 0));
    }

    #[test]
    fn scout_bat_resolves_world_contact_only_after_the_authored_smack_delay() {
        let mut session = Session::new(MeleeWall, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Scout),
                select_weapon: Some(Weapon::Bat),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Bat)
            .unwrap()
            .next_primary_tick = 0;
        let started = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(
            !started
                .events
                .iter()
                .any(|event| matches!(event, Event::MeleeImpact { .. }))
        );
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::BatMiss)
        );
        let due = session.pending_melee_tick.unwrap();
        while session.pending_melee_tick.is_some() {
            let snapshot = session.advance(Command::default()).unwrap();
            if session.pending_melee_tick.is_some() {
                assert!(session.tick <= due + 1);
                assert!(
                    !snapshot
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::MeleeImpact { .. }))
                );
            } else {
                assert!(snapshot.events.iter().any(|event| matches!(event, Event::MeleeImpact { weapon: Weapon::Bat, target: None, damage, .. } if *damage == 35.0)));
                assert!(
                    session
                        .audio_events()
                        .iter()
                        .any(|event| event.definition == SoundDefinition::BatHitWorld)
                );
            }
        }
    }

    #[test]
    fn pyro_sound_patches_crossfade_once_and_release_both_without_cutting_winddown() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Pyro),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::Flamethrower)
            .unwrap()
            .next_primary_tick = 0;
        for cycle in 0..3 {
            session
                .advance(Command {
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
            assert_eq!(
                session
                    .audio_events
                    .iter()
                    .map(|event| (event.definition, event.action))
                    .collect::<Vec<_>>(),
                vec![
                    (SoundDefinition::FlameFire, AudioAction::FadeOut(3.5)),
                    (SoundDefinition::FlameLoop, AudioAction::FadeIn(3.5)),
                ],
                "cycle {cycle}"
            );
            for _ in 0..20 {
                session
                    .advance(Command {
                        fire: true,
                        ..Command::default()
                    })
                    .unwrap();
                assert!(session.audio_events.is_empty());
            }
            session.advance(Command::default()).unwrap();
            assert_eq!(
                session
                    .audio_events
                    .iter()
                    .map(|event| (event.definition, event.action))
                    .collect::<Vec<_>>(),
                vec![
                    (SoundDefinition::FlameEnd, AudioAction::Play),
                    (SoundDefinition::FlameLoop, AudioAction::Stop),
                    (SoundDefinition::FlameFire, AudioAction::Stop),
                ]
            );
            assert!(
                session
                    .audio_events
                    .iter()
                    .enumerate()
                    .all(|(ordinal, event)| event.ordinal == ordinal as u16
                        && event.source_identity == PLAYER_IDENTITY)
            );
            assert!(!session.producer_snapshot().flame_firing);
            assert!(
                !session.flames.points().is_empty(),
                "release must not remove live damaging flame points"
            );
            for _ in 0..10 {
                session.advance(Command::default()).unwrap();
                assert!(session.audio_events.is_empty());
            }
        }
    }

    #[test]
    fn pyro_sound_patch_lifetime_is_not_the_mouse_button_or_live_flame_count() {
        let mut base = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        base.advance(Command {
            select_class: Some(PlayerClass::Pyro),
            ..Command::default()
        })
        .unwrap();
        base.loadout
            .get_mut(&Weapon::Flamethrower)
            .unwrap()
            .next_primary_tick = 0;
        base.advance(Command {
            fire: true,
            ..Command::default()
        })
        .unwrap();
        let until = base.flame_burst_until;
        let mut tap = base.clone();
        while (tap.tick as f32 * 0.015) < until {
            tap.advance(Command::default()).unwrap();
            assert!(tap.flame_firing);
        }
        tap.advance(Command::default()).unwrap();
        assert!(!tap.flame_firing);
        for command in [
            Command {
                detonate: true,
                ..Command::default()
            },
            Command {
                select_weapon: Some(Weapon::Shotgun),
                ..Command::default()
            },
            Command {
                select_class: Some(PlayerClass::Scout),
                ..Command::default()
            },
            Command {
                select_team: Some(PlayerTeam::Blue),
                ..Command::default()
            },
        ] {
            let mut session = base.clone();
            session.advance(command).unwrap();
            assert!(!session.flame_firing);
            assert_eq!(
                session
                    .audio_events
                    .iter()
                    .filter(|event| event.action == AudioAction::Stop)
                    .count(),
                2
            );
        }
        let mut spectator = base.clone();
        spectator
            .select_team_choice(team_selection::TeamChoice::Spectator)
            .unwrap();
        spectator.advance(Command::default()).unwrap();
        assert!(!spectator.flame_firing);
        assert_eq!(
            spectator
                .audio_events
                .iter()
                .filter(|event| event.action == AudioAction::Stop)
                .count(),
            2
        );
        let mut empty = base.clone();
        let mut reset = base.clone();
        reset
            .advance(Command {
                respawn: true,
                ..Command::default()
            })
            .unwrap();
        assert!(!reset.flame_firing);
        assert_eq!(
            reset
                .audio_events
                .iter()
                .map(|event| (event.definition, event.action))
                .collect::<Vec<_>>(),
            vec![
                (SoundDefinition::FlameLoop, AudioAction::Stop),
                (SoundDefinition::FlameFire, AudioAction::Stop),
            ]
        );
        empty
            .loadout
            .get_mut(&Weapon::Flamethrower)
            .unwrap()
            .reserve = 0;
        empty
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(!empty.flame_firing);
        assert_eq!(
            empty
                .audio_events
                .iter()
                .filter(|event| event.action == AudioAction::Stop)
                .count(),
            2
        );
        base.audio_events.clear();
        base.die(&mut Vec::new());
        assert!(!base.flame_firing);
        assert_eq!(
            base.audio_events
                .iter()
                .filter(|event| event.action == AudioAction::Stop)
                .count(),
            2
        );
    }

    #[test]
    fn pyro_held_attack_plays_first_flame_sound_after_viewmodel_draw() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Pyro),
                ..Command::default()
            })
            .unwrap();
        let mut fired = false;
        for _ in 0..50 {
            session
                .advance(Command {
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
            if !session.flames.points().is_empty() {
                assert!(
                    session
                        .audio_events()
                        .iter()
                        .any(|event| event.definition == SoundDefinition::FlameFire)
                );
                assert!(
                    session
                        .activity_events()
                        .iter()
                        .any(|event| event.activity == weapon::WeaponActivity::PrimaryAttack)
                );
                fired = true;
                break;
            }
        }
        assert!(fired);
    }

    #[test]
    fn pyro_airblast_reflects_enemy_projectiles_without_touching_friendly_rockets() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Pyro),
                ..Command::default()
            })
            .unwrap();
        for _ in 0..40 {
            session.advance(Command::default()).unwrap();
        }
        let team = session.team_selection.local_team();
        let opposite = if team == PlayerTeam::Red {
            PlayerTeam::Blue
        } else {
            PlayerTeam::Red
        };
        let create = |identity, team| LiveProjectile {
            killing_weapon: "tf_projectile_rocket",
            presentation: Projectile {
                identity,
                kind: ProjectileKind::Rocket,
                team,
                owner_identity: 30,
                launcher_identity: 1,
                state: ProjectileState::Flying,
                position: [80.0, 0.0, 68.0],
                velocity: [-1_100.0, 0.0, 0.0],
                orientation: [0.0, 0.0, 0.0, 1.0],
                angular_velocity: [0.0; 3],
                contact_normal: None,
                age_seconds: 0.1,
            },
            armed: false,
            creation_tick: 0,
            arm_tick: 0,
            next_think_tick: 0,
            forced_detonate_tick: None,
            motion_enabled: true,
            direct_target: None,
        };
        session.projectiles.push(create(1, opposite));
        session.projectiles.push(create(2, team));
        session
            .advance_flamethrower(
                Command {
                    detonate: true,
                    ..Command::default()
                },
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(session.projectiles[0].presentation.team, team);
        assert_eq!(
            session.projectiles[0].presentation.owner_identity,
            PLAYER_IDENTITY
        );
        assert!((length(session.projectiles[0].presentation.velocity) - 1_100.0).abs() < 0.01);
        assert!(session.projectiles[0].presentation.velocity[0] > 1_099.0);
        assert_eq!(
            session.projectiles[1].presentation.velocity,
            [-1_100.0, 0.0, 0.0]
        );
    }

    #[test]
    fn pyro_stock_weapons_preserve_flame_airblast_shotgun_and_melee_authority() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Pyro),
                ..Command::default()
            })
            .unwrap();
        for _ in 0..40 {
            session.advance(Command::default()).unwrap();
        }
        assert_eq!(
            session
                .weapon_runtime(Weapon::Flamethrower)
                .unwrap()
                .reserve,
            200
        );
        for _ in 0..10 {
            session
                .advance(Command {
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
        }
        let producer = session.producer_snapshot();
        assert!(!producer.flame_points.is_empty());
        assert!(producer.flame_firing);
        let reserve = session
            .weapon_runtime(Weapon::Flamethrower)
            .unwrap()
            .reserve;
        session
            .advance(Command {
                detonate: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(
            session
                .weapon_runtime(Weapon::Flamethrower)
                .unwrap()
                .reserve,
            reserve - 20
        );
        assert!(
            session
                .activity_events()
                .iter()
                .any(|event| event.activity == weapon::WeaponActivity::SecondaryAttack)
        );
        session
            .advance(Command {
                select_weapon: Some(Weapon::Shotgun),
                ..Command::default()
            })
            .unwrap();
        for _ in 0..40 {
            session.advance(Command::default()).unwrap();
        }
        let fired = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(session.producer_snapshot().shotgun_pellets.len(), 10);
        assert!(fired.events.iter().any(|event| matches!(
            event,
            Event::HitscanFired {
                weapon: Weapon::Shotgun,
                pellets: 10,
                ..
            }
        )));
        assert_eq!(session.weapon_runtime(Weapon::Shotgun).unwrap().clip, 5);
        session
            .advance(Command {
                select_weapon: Some(Weapon::FireAxe),
                ..Command::default()
            })
            .unwrap();
        for _ in 0..40 {
            session.advance(Command::default()).unwrap();
        }
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(session.pending_melee_tick.is_some());
        assert!(
            session
                .audio_events()
                .iter()
                .any(|event| event.definition == SoundDefinition::FireAxeMiss)
        );
        for _ in 0..15 {
            session.advance(Command::default()).unwrap();
        }
        assert!(session.pending_melee_tick.is_none());
    }

    #[test]
    fn every_class_uses_one_script_backed_identity_spawn_and_movement_policy() {
        let spawn = [16.0, -24.0, 0.0];
        for class in PlayerClass::ALL {
            let mut session = Session::new(Floor, spawn, MapRuntime::empty(0.015));
            let snapshot = session
                .advance(Command {
                    select_class: Some(class),
                    select_team: Some(PlayerTeam::Blue),
                    movement: MoveCommand {
                        forward: 450.0,
                        ..MoveCommand::default()
                    },
                    fire: true,
                    reload: true,
                    ..Command::default()
                })
                .unwrap();
            let policy = MovementPolicy {
                class,
                modifiers: MovementModifiers::default(),
            }
            .resolve();
            assert_eq!(snapshot.class, class);
            assert_eq!(snapshot.team, PlayerTeam::Blue);
            assert_eq!(snapshot.maximum_health, class.data().maximum_health as f32);
            assert_eq!(snapshot.health, class.data().maximum_health as f32);
            assert_eq!(policy.maximum_speed, class.data().maximum_speed);
            assert_eq!(policy.standing_hull.mins, [-24.0, -24.0, 0.0]);
            assert_eq!(policy.standing_hull.maxs, [24.0, 24.0, 82.0]);
            assert_eq!(policy.crouched_hull.maxs, [24.0, 24.0, 62.0]);
            assert_eq!(
                snapshot.movement.view_offset[2],
                class.standing_eye_height()
            );
            assert!(snapshot.movement.velocity[0] > 0.0);
            match class {
                PlayerClass::Scout => {
                    assert_eq!(snapshot.weapon, Some(Weapon::Scattergun));
                    assert_eq!(snapshot.loadout.len(), 3);
                    assert_eq!(
                        snapshot
                            .loadout
                            .iter()
                            .map(|weapon| weapon.weapon)
                            .collect::<Vec<_>>(),
                        vec![Weapon::Scattergun, Weapon::Pistol, Weapon::Bat],
                    );
                }
                PlayerClass::Sniper => {
                    assert_eq!(snapshot.weapon, Some(Weapon::SniperRifle));
                    assert_eq!(
                        snapshot
                            .loadout
                            .iter()
                            .map(|weapon| weapon.weapon)
                            .collect::<Vec<_>>(),
                        vec![Weapon::SniperRifle, Weapon::Smg, Weapon::Kukri]
                    );
                }
                PlayerClass::Soldier => {
                    assert_eq!(snapshot.weapon, Some(Weapon::RocketLauncher));
                    assert_eq!(
                        snapshot
                            .loadout
                            .iter()
                            .map(|weapon| weapon.weapon)
                            .collect::<Vec<_>>(),
                        vec![Weapon::RocketLauncher, Weapon::Shotgun, Weapon::Shovel],
                    );
                }
                PlayerClass::Demoman => {
                    assert_eq!(snapshot.weapon, Some(Weapon::GrenadeLauncher));
                    assert_eq!(
                        snapshot
                            .loadout
                            .iter()
                            .map(|weapon| weapon.weapon)
                            .collect::<Vec<_>>(),
                        vec![
                            Weapon::StickybombLauncher,
                            Weapon::Bottle,
                            Weapon::GrenadeLauncher
                        ],
                    );
                }
                PlayerClass::Heavy => {
                    assert_eq!(snapshot.weapon, Some(Weapon::Minigun));
                    assert_eq!(snapshot.loadout.len(), 3);
                }
                PlayerClass::Engineer => {
                    assert_eq!(snapshot.weapon, Some(Weapon::EngineerShotgun));
                    assert_eq!(
                        snapshot
                            .loadout
                            .iter()
                            .map(|weapon| weapon.weapon)
                            .collect::<Vec<_>>(),
                        vec![
                            Weapon::EngineerShotgun,
                            Weapon::EngineerPistol,
                            Weapon::Wrench,
                            Weapon::BuildPda,
                            Weapon::DestroyPda,
                            Weapon::Toolbox
                        ],
                    );
                    assert_eq!(snapshot.metal, 200);
                }
                PlayerClass::Pyro => {
                    assert_eq!(snapshot.weapon, Some(Weapon::Flamethrower));
                    assert_eq!(snapshot.loadout.len(), 3);
                    assert_eq!(
                        (snapshot.loadout[0].clip, snapshot.loadout[0].reserve),
                        (6, 32)
                    );
                    assert_eq!(
                        (snapshot.loadout[1].clip, snapshot.loadout[1].reserve),
                        (0, 200)
                    );
                    assert_eq!(snapshot.loadout[2].weapon, Weapon::FireAxe);
                }
                PlayerClass::Spy => {
                    assert_eq!(snapshot.weapon, Some(Weapon::Revolver));
                    assert_eq!(snapshot.loadout.len(), 5);
                    assert_eq!(snapshot.spy, Some(spy::SpyState::default()));
                }
                PlayerClass::Medic => {
                    assert_eq!(snapshot.weapon, Some(Weapon::SyringeGun));
                    assert_eq!(
                        snapshot
                            .loadout
                            .iter()
                            .map(|weapon| weapon.weapon)
                            .collect::<Vec<_>>(),
                        vec![Weapon::SyringeGun, Weapon::MediGun, Weapon::Bonesaw],
                    );
                }
            }
            let respawned = session
                .advance(Command {
                    respawn: true,
                    ..Command::default()
                })
                .unwrap();
            assert_eq!(respawned.movement.position, spawn);
            assert_eq!(respawned.health, class.data().maximum_health as f32);
        }
    }

    #[test]
    fn scout_air_dash_requires_a_new_edge_and_resets_only_on_ground_or_spawn() {
        for class in PlayerClass::ALL {
            let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
            session
                .advance(Command {
                    select_class: Some(class),
                    ..Command::default()
                })
                .unwrap();
            session
                .advance(Command {
                    movement: MoveCommand {
                        forward: 450.0,
                        jump: true,
                        ..MoveCommand::default()
                    },
                    ..Command::default()
                })
                .unwrap();
            let released = session.advance(Command::default()).unwrap();
            assert!(released.movement.ground.is_none());
            let second = session
                .advance(Command {
                    movement: MoveCommand {
                        side: 450.0,
                        jump: true,
                        ..MoveCommand::default()
                    },
                    ..Command::default()
                })
                .unwrap();
            let dashed = session
                .last_movement_result()
                .unwrap()
                .events
                .contains(&playsrc_movement::Event::Jumped);
            assert_eq!(dashed, class == PlayerClass::Scout);
            if class == PlayerClass::Scout {
                assert!((second.movement.velocity[2] - 262.328_16).abs() < 0.001);
                assert!(second.movement.velocity[1].abs() > 300.0);
                assert_eq!(session.air_dashes, 1);
                session.advance(Command::default()).unwrap();
                session
                    .advance(Command {
                        movement: MoveCommand {
                            jump: true,
                            ..MoveCommand::default()
                        },
                        ..Command::default()
                    })
                    .unwrap();
                assert!(
                    !session
                        .last_movement_result()
                        .unwrap()
                        .events
                        .contains(&playsrc_movement::Event::Jumped)
                );
                session
                    .advance(Command {
                        respawn: true,
                        ..Command::default()
                    })
                    .unwrap();
                assert_eq!(session.air_dashes, 0);
            }
        }
    }

    fn explosive(kind: ProjectileKind, position: [f32; 3]) -> LiveProjectile {
        LiveProjectile {
            killing_weapon: if kind == ProjectileKind::Rocket { "tf_projectile_rocket" } else { "tf_projectile_pipe_remote" },
            presentation: Projectile {
                identity: 99,
                kind,
                team: PlayerTeam::Red,
                owner_identity: PLAYER_IDENTITY,
                launcher_identity: match kind {
                    ProjectileKind::Rocket => Weapon::RocketLauncher as u32,
                    ProjectileKind::Sticky => Weapon::StickybombLauncher as u32,
                    ProjectileKind::Syringe => Weapon::SyringeGun as u32,
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

    fn launch_sticky(tick_interval: f32) -> (Session<Floor>, u32) {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(tick_interval));
        session.movement_configuration.tick_interval = tick_interval;
        session
            .advance(Command {
                select_class: Some(PlayerClass::Demoman),
                select_weapon: Some(Weapon::StickybombLauncher),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::StickybombLauncher)
            .unwrap()
            .next_primary_tick = session.tick;
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        let fired = session.advance(Command::default()).unwrap();
        let projectile = fired.projectiles[0].identity;
        assert_eq!(
            session
                .physics_requests()
                .iter()
                .map(|request| request.operation)
                .collect::<Vec<_>>(),
            [ProjectilePhysicsOperation::Create]
        );
        (session, projectile)
    }

    fn sticky_result<W: GameplayWorld + Clone>(
        session: &Session<W>,
        projectile: u32,
        position: [f32; 3],
        contact: Option<ProjectileContact>,
    ) -> ProjectilePhysicsResult {
        ProjectilePhysicsResult {
            projectile,
            tick: session.tick,
            position,
            velocity: [700.0, 50.0, -100.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            angular_velocity: [600.0, 1200.0, 0.0],
            motion_enabled: true,
            contact,
        }
    }

    #[test]
    fn source_basis_muzzle_flip_center_fire_masks_and_orientation_are_preserved() {
        let (forward, right, up) = angle_vectors(0.0, 0.0, 0.0);
        assert_eq!(forward, [1.0, 0.0, 0.0]);
        assert_eq!(right, [0.0, -1.0, 0.0]);
        assert_eq!(up, [0.0, 0.0, 1.0]);
        let (_, right_yaw_90, _) = angle_vectors(0.0, 90.0, 0.0);
        assert!((right_yaw_90[0] - 1.0).abs() < 0.000_001);
        assert!(right_yaw_90[1].abs() < 0.000_001);

        let launch = |weapon: Weapon, flipped: bool, crouched: bool| {
            let collision = RecordingWorld::default();
            let traces = collision.traces.clone();
            let mut session = Session::new(collision, [0.0; 3], MapRuntime::empty(0.015));
            session.weapon = Some(weapon);
            session
                .loadout
                .entry(weapon)
                .or_insert_with(|| WeaponRuntime::full(weapon));
            session.flip_viewmodels = flipped;
            if crouched {
                session.movement = MovementState::from_player(
                    Player {
                        position: [0.0; 3],
                        velocity: [0.0; 3],
                        grounded: true,
                        crouched: true,
                        jump_latched: false,
                    },
                    MovementPolicy {
                        class: PlayerClass::Soldier,
                        modifiers: MovementModifiers::default(),
                    }
                    .resolve(),
                );
            }
            session
                .fire_projectile(0.0, 0.0, 0.0, None, None, &mut Vec::new())
                .unwrap();
            let projectile = session.projectiles[0].presentation.clone();
            (projectile, traces.lock().unwrap().clone())
        };

        let (stock, traces) = launch(Weapon::RocketLauncher, false, false);
        assert_eq!(stock.position, [23.5, -12.0, 65.0]);
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].3, MASK_SOLID);
        assert_eq!(traces[0].2.mins, [0.0; 3]);
        assert_eq!(traces[1].3, MASK_SOLID_BRUSH_ONLY);
        assert_eq!(traces[1].1, stock.position);
        assert_eq!(
            stock.orientation,
            quaternion_from_direction(normalized(stock.velocity))
        );

        let (flipped, _) = launch(Weapon::RocketLauncher, true, false);
        assert_eq!(flipped.position, [23.5, 12.0, 65.0]);
        let (centered, _) = launch(Weapon::Original, true, false);
        assert_eq!(centered.position, [23.5, 0.0, 65.0]);
        let (crouched, _) = launch(Weapon::RocketLauncher, false, true);
        assert_eq!(crouched.position, [23.5, -12.0, 53.0]);
        assert_eq!(
            rocket_flight_mask(PlayerTeam::Red),
            MASK_SOLID | CONTENTS_BLUE_TEAM
        );
        assert_eq!(
            rocket_flight_mask(PlayerTeam::Blue),
            MASK_SOLID | CONTENTS_RED_TEAM
        );
    }

    #[test]
    fn projectile_contract_emits_oriented_ordered_transitions() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .next_primary_tick = 0;
        let fired = session
            .advance(Command {
                pitch_degrees: 89.0,
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(fired.projectile_events[0].kind, ProjectileEventKind::Fire);
        assert_eq!(
            fired.projectile_events[0].launcher_pose,
            Some(ProjectileLauncherPose {
                eye_position: add(fired.movement.position, fired.movement.view_offset),
                view_orientation: quaternion_from_angles(89.0, 0.0, 0.0),
            })
        );
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
                    direct_target: None,
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
        for event in &exploded.projectile_events {
            assert_eq!(event.position, add(trace.end, [0.0, 0.0, 1.0]));
            assert_eq!(event.contact_normal, Some([0.0, 0.0, 1.0]));
        }
        assert_eq!(session.radius_damage_requests().len(), 1);
        assert_eq!(session.radius_damage_requests()[0].direct_target, None);
        assert_eq!(session.audio_events().len(), 1);
        assert_eq!(
            session.audio_events()[0].identity,
            AudioEventIdentity::ExplosionSpecial1
        );
        assert_eq!(
            session.audio_events()[0].definition,
            SoundDefinition::RocketExplosion
        );
        assert_eq!(
            session.audio_events()[0].source_kind,
            AudioSourceKind::World
        );
        assert_eq!(session.audio_events()[0].source_identity, trace.projectile);

        let mut sky = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        sky.loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .next_primary_tick = 0;
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
        assert_eq!(removed.projectile_events.len(), 1);
        assert_eq!(
            removed.projectile_events[0].kind,
            ProjectileEventKind::Fizzle
        );
        assert!(sky.radius_damage_requests().is_empty());
    }

    #[test]
    fn rocket_wall_impact_preserves_one_normal_unit_and_rejects_invalid_normals() {
        for normal in [
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 2.0],
            [3.0, 4.0, 0.0],
        ] {
            let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
            session
                .loadout
                .get_mut(&Weapon::RocketLauncher)
                .unwrap()
                .next_primary_tick = 0;
            session
                .advance(Command {
                    pitch_degrees: 89.0,
                    fire: true,
                    ..Command::default()
                })
                .unwrap();
            let trace = session.rocket_trace_requests()[0];
            let expected_normal = normalized(normal);
            let result = RocketTraceResult {
                projectile: trace.projectile,
                tick: session.tick,
                end: trace.end,
                solid: true,
                sky: false,
                normal: Some([0.0; 3]),
                direct_target: None,
            };
            let before_tick = session.tick;
            assert!(matches!(
                session.advance_with_external(Command::default(), &[], &[result], None),
                Err(Error::InvalidProjectilePhysics)
            ));
            assert_eq!(session.tick, before_tick);
            assert_eq!(session.rocket_trace_requests(), &[trace]);

            let exploded = session
                .advance_with_external(
                    Command::default(),
                    &[],
                    &[RocketTraceResult {
                        normal: Some(normal),
                        ..result
                    }],
                    None,
                )
                .unwrap();
            assert_eq!(exploded.projectile_events.len(), 2);
            for event in exploded.projectile_events {
                assert_eq!(event.contact_normal, Some(expected_normal));
                for (axis, expected) in expected_normal.into_iter().enumerate() {
                    assert!((event.position[axis] - trace.end[axis] - expected).abs() < 0.0001);
                }
            }
        }
    }

    #[test]
    fn direct_rocket_damage_drives_button_outputs_and_linked_door_io() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"func_button\"\"model\"\"*1\"\"targetname\"\"button\"\"spawnflags\"\"512\"\"OnDamaged\"\"door,Open,,0,-1\"}{\"classname\"\"func_door\"\"model\"\"*2\"\"targetname\"\"door\"\"movedir\"\"-90 0 0\"\"speed\"\"100\"}",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            7,
            vec![
                playsrc_entity::ModelBounds {
                    model: 1,
                    mins: [-4.0, -16.0, -16.0],
                    maxs: [4.0, 16.0, 16.0],
                },
                playsrc_entity::ModelBounds {
                    model: 2,
                    mins: [-8.0, -48.0, -96.0],
                    maxs: [8.0, 48.0, 96.0],
                },
            ],
        )
        .unwrap();
        let mut session = Session::new(Floor, [0.0; 3], map);
        session
            .loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .next_primary_tick = 0;
        let fired = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(fired.projectile_events[0].kind, ProjectileEventKind::Fire);
        let request = session.rocket_trace_requests()[0];
        let before = session.producer_snapshot();
        assert!(matches!(
            session.advance(Command::default()),
            Err(Error::InvalidProjectilePhysics)
        ));
        assert_eq!(session.producer_snapshot(), before);

        let damaged = session
            .advance_with_external(
                Command::default(),
                &[],
                &[RocketTraceResult {
                    projectile: request.projectile,
                    tick: session.tick,
                    end: request.end,
                    solid: true,
                    sky: false,
                    normal: Some([0.0, 0.0, 1.0]),
                    direct_target: Some(0),
                }],
                None,
            )
            .unwrap();
        assert_eq!(
            damaged
                .projectile_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [ProjectileEventKind::Impact, ProjectileEventKind::Explode]
        );
        assert_eq!(session.radius_damage_requests()[0].direct_target, Some(0));
        let on_damaged = damaged
            .entity_events
            .iter()
            .position(|event| {
                event.kind == EntityEventKind::Output
                    && event.entity == 0
                    && event.name == b"OnDamaged"
            })
            .unwrap();
        let door_open = damaged
            .entity_events
            .iter()
            .position(|event| {
                event.kind == EntityEventKind::Input
                    && event.entity == 1
                    && event.name.eq_ignore_ascii_case(b"Open")
            })
            .unwrap();
        assert!(on_damaged < door_open);
        assert!(
            session
                .mover_requests()
                .iter()
                .any(|request| request.entity == 1)
        );
    }

    #[test]
    fn rocket_blast_water_depth_matrix_preserves_damage_force_and_next_tick() {
        for (name, surface, prior_level, level, damage, impulse, next_velocity, next_level) in [
            ("dry", -100.0, 0, 0, 54, 540.00006, 528.00006, 0),
            ("surface", 10.0, 0, 0, 54, 540.00006, 528.00006, 0),
            ("feet", 20.0, 1, 1, 90, 900.0, 888.0, 0),
            ("waist", 70.0, 2, 2, 90, 900.0, 846.0, 1),
            ("eyes", 90.0, 2, 3, 90, 900.0, 846.0, 2),
            ("submerged", 200.0, 2, 3, 90, 900.0, 846.0, 3),
        ] {
            let mut session = Session::new(
                WaterFloor { surface },
                [0.0, 0.0, 10.0],
                MapRuntime::empty(0.015),
            );
            session.movement.water_level = prior_level;
            session.movement.water_type = if prior_level == 0 {
                0
            } else {
                playsrc_movement::CONTENTS_WATER
            };
            session.advance(Command::default()).unwrap();
            assert_eq!(session.movement.water_level, level, "{name} starting depth");
            session.movement.velocity = [0.0; 3];
            let origin = session.movement.position;
            session
                .explode(
                    explosive(ProjectileKind::Rocket, origin),
                    &mut Vec::new(),
                    &mut Vec::new(),
                )
                .unwrap();
            assert_eq!(session.health, 200 - damage, "{name} damage");
            assert_eq!(session.movement.velocity[2], impulse, "{name} force");
            let accepted = session.advance(Command::default()).unwrap();
            assert_eq!(
                accepted.movement.velocity[2], next_velocity,
                "{name} next tick"
            );
            assert_eq!(
                accepted.movement.water_level, next_level,
                "{name} next depth"
            );
        }
    }

    #[test]
    fn water_flag_transitions_preserve_source_airborne_soldier_damage_order() {
        let mut direct_submersion = Session::new(
            WaterFloor { surface: 200.0 },
            [0.0, 0.0, 10.0],
            MapRuntime::empty(0.015),
        );
        let direct = direct_submersion.advance(Command::default()).unwrap();
        assert_eq!(direct_submersion.movement.water_level, 3);
        assert_eq!(direct.player_flags & FL_INWATER, 0);
        assert_eq!(
            direct_submersion.producer_snapshot().player_flags & FL_INWATER,
            0
        );
        let direct = direct_submersion.advance(Command::default()).unwrap();
        assert_eq!(direct.player_flags & FL_INWATER, 0);
        direct_submersion.movement.velocity = [0.0; 3];
        direct_submersion
            .explode(
                explosive(ProjectileKind::Rocket, direct_submersion.movement.position),
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(direct_submersion.health, 146);
        assert_eq!(direct_submersion.movement.velocity[2], 540.00006);

        let mut wading = Session::new(
            WaterFloor { surface: 12.0 },
            [0.0, 0.0, 10.0],
            MapRuntime::empty(0.015),
        );
        let first_wade = wading.advance(Command::default()).unwrap();
        assert_eq!(wading.movement.water_level, 1);
        assert_eq!(first_wade.player_flags & FL_INWATER, 0);
        wading.collision.surface = 200.0;
        let submerged = wading.advance(Command::default()).unwrap();
        assert_eq!(wading.movement.water_level, 3);
        assert_eq!(submerged.player_flags & FL_INWATER, FL_INWATER);
        assert_eq!(
            wading.producer_snapshot().player_flags & FL_INWATER,
            FL_INWATER
        );
        wading.movement.velocity = [0.0; 3];
        wading
            .explode(
                explosive(ProjectileKind::Rocket, wading.movement.position),
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(wading.health, 110);
        assert_eq!(wading.movement.velocity[2], 900.0);

        wading.collision.surface = -100.0;
        let exited = wading.advance(Command::default()).unwrap();
        assert_eq!(wading.movement.water_level, 0);
        assert_eq!(exited.player_flags & FL_INWATER, FL_INWATER);
        let dry = wading.advance(Command::default()).unwrap();
        assert_eq!(dry.player_flags & FL_INWATER, 0);
    }

    #[test]
    fn underwater_rocket_jump_rejects_airborne_crouch_force_amplification() {
        let mut submerged = Session::new(
            WaterFloor { surface: 200.0 },
            [0.0, 0.0, 10.0],
            MapRuntime::empty(0.015),
        );
        submerged.movement.water_level = 2;
        submerged.movement.water_type = playsrc_movement::CONTENTS_WATER;
        submerged
            .advance(Command {
                movement: MoveCommand {
                    crouch: true,
                    ..MoveCommand::default()
                },
                ..Command::default()
            })
            .unwrap();
        submerged.movement.velocity = [0.0; 3];
        let explosion = submerged.movement.position;
        submerged
            .explode(
                explosive(ProjectileKind::Rocket, explosion),
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
        let submerged_impulse = submerged.movement.velocity[2];
        let submerged_next = submerged
            .advance(Command {
                movement: MoveCommand {
                    crouch: true,
                    ..MoveCommand::default()
                },
                ..Command::default()
            })
            .unwrap()
            .movement
            .velocity[2];
        assert_eq!(
            submerged_impulse, 900.0,
            "an airborne underwater crouch incorrectly amplified the stock rocket impulse to {submerged_impulse}; the next-tick velocity was {submerged_next}"
        );
        assert_eq!(submerged_next, 846.0);
        assert!(!submerged.movement.crouch.uses_crouched_hull());
    }

    #[test]
    fn self_blast_integration_preserves_damage_force_and_ground_order() {
        let policy = MovementPolicy {
            class: PlayerClass::Soldier,
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
        grounded
            .explode(
                explosive(ProjectileKind::Rocket, [0.0; 3]),
                &mut projectile_events,
                &mut events,
            )
            .unwrap();
        assert_eq!(grounded.health, 110);
        assert_eq!(grounded.movement.velocity[2], 450.0);
        assert!(grounded.movement.ground.is_some());
        assert!(grounded.conditions.contains(Condition::BlastJumping));

        let mut airborne = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        airborne
            .explode(
                explosive(ProjectileKind::Rocket, [0.0; 3]),
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
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
        crouched
            .explode(
                explosive(ProjectileKind::Rocket, [0.0; 3]),
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(crouched.health, 110);
        let crouched_expected = 90.0_f32 * ((48.0 * 48.0 * 82.0) / (48.0 * 48.0 * 55.0)) * 5.0;
        assert!((crouched.movement.velocity[2] - crouched_expected).abs() <= f32::EPSILON * 512.0);

        let demo_policy = MovementPolicy {
            class: PlayerClass::Demoman,
            modifiers: MovementModifiers::default(),
        }
        .resolve();
        let mut demo = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        demo.class = PlayerClass::Demoman;
        demo.weapon = Some(Weapon::StickybombLauncher);
        demo.loadout = default_loadout(PlayerClass::Demoman);
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
        )
        .unwrap();
        assert_eq!(demo.health, 85);
        assert_eq!(demo.movement.velocity[2], 810.0);
    }

    #[test]
    fn sticky_launch_uses_exact_stream_order_and_typed_physics_results() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.01));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Demoman),
                select_weapon: Some(Weapon::StickybombLauncher),
                ..Command::default()
            })
            .unwrap();
        session
            .loadout
            .get_mut(&Weapon::StickybombLauncher)
            .unwrap()
            .next_primary_tick = session.tick;
        session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        let before = session.weapon_runtime(Weapon::StickybombLauncher).unwrap();
        let producer_before = session.producer_snapshot();
        let random_before = session.random_state();
        assert!(before.charge_begin_tick.is_some());
        assert!(matches!(
            session.advance_with_external(
                Command::default(),
                &[],
                &[],
                Some(StickyLaunchRandom {
                    right_velocity: 0.0,
                    up_velocity: 0.0,
                    angular_y: 0,
                }),
            ),
            Err(Error::InvalidStickyLaunchRandom)
        ));
        assert_eq!(session.producer_snapshot(), producer_before);
        assert_eq!(session.random_state(), random_before);

        let fired = session.advance(Command::default()).unwrap();
        assert_eq!(fired.projectiles.len(), 1);
        let projectile = fired.projectiles[0].clone();
        assert_eq!(projectile.velocity[0], 905.625);
        assert_eq!(projectile.velocity[1].to_bits(), 0xc045_042c);
        assert_eq!(projectile.velocity[2], 200.0 + f32::from_bits(0xc10a_9c49));
        assert_eq!(projectile.angular_velocity, [600.0, -563.0, 0.0]);
        assert_eq!(session.audio_events().len(), 1);
        let sound = session.audio_events()[0];
        assert_eq!(sound.identity, AudioEventIdentity::WeaponSingle);
        assert_eq!(sound.definition, SoundDefinition::StickySingle);
        assert_eq!(sound.source_identity, PLAYER_IDENTITY);
        assert_eq!(sound.samples.volume.to_bits(), 0x3f07_9a6f);
        assert_eq!(sound.samples.pitch.to_bits(), 0x3f6e_3116);
        assert_eq!(sound.samples.wave, 0);
        assert_eq!(sound.samples.sound_level.to_bits(), 0x3ec4_5a62);
        assert_eq!(
            session
                .random_draws()
                .iter()
                .filter(|draw| draw.context == RandomContext::Authority)
                .map(|draw| draw.decision)
                .collect::<Vec<_>>(),
            [
                RandomDecision::SoundVolume {
                    definition: SoundDefinition::StickySingle,
                    phase: SoundQueryPhase::Inspect,
                },
                RandomDecision::SoundPitch {
                    definition: SoundDefinition::StickySingle,
                    phase: SoundQueryPhase::Inspect,
                },
                RandomDecision::SoundLevel {
                    definition: SoundDefinition::StickySingle,
                    phase: SoundQueryPhase::Inspect,
                },
                RandomDecision::SoundVolume {
                    definition: SoundDefinition::StickySingle,
                    phase: SoundQueryPhase::Emit,
                },
                RandomDecision::SoundPitch {
                    definition: SoundDefinition::StickySingle,
                    phase: SoundQueryPhase::Emit,
                },
                RandomDecision::SoundLevel {
                    definition: SoundDefinition::StickySingle,
                    phase: SoundQueryPhase::Emit,
                },
                RandomDecision::StickyRightVelocity,
                RandomDecision::StickyUpVelocity,
                RandomDecision::StickyAngularY,
            ]
        );
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
    fn sticky_requires_ordered_results_before_flying_arm_state_advances() {
        let (mut session, projectile) = launch_sticky(0.4);
        let before = session.producer_snapshot();
        assert!(matches!(
            session.advance(Command::default()),
            Err(Error::InvalidProjectilePhysics)
        ));
        assert_eq!(session.producer_snapshot(), before);

        let first = sticky_result(&session, projectile, [10.0, 0.0, 20.0], None);
        let flying = session
            .advance_with_external(Command::default(), &[first], &[], None)
            .unwrap();
        assert_eq!(flying.projectiles[0].age_seconds, 0.4);
        assert_eq!(flying.projectiles[0].state, ProjectileState::Flying);
        assert!(flying.projectile_events.is_empty());
        assert_eq!(
            session
                .physics_requests()
                .iter()
                .map(|request| request.operation)
                .collect::<Vec<_>>(),
            [ProjectilePhysicsOperation::Step]
        );

        let second = sticky_result(&session, projectile, [20.0, 1.0, 15.0], None);
        let armed = session
            .advance_with_external(Command::default(), &[second], &[], None)
            .unwrap();
        assert_eq!(armed.projectiles[0].age_seconds, 0.8);
        assert_eq!(armed.projectiles[0].state, ProjectileState::Flying);
        assert_eq!(armed.projectiles[0].contact_normal, None);
        assert_eq!(armed.projectile_events.len(), 1);
        assert_eq!(armed.projectile_events[0].kind, ProjectileEventKind::Arm);
        assert_eq!(armed.projectile_events[0].contact_normal, None);
    }

    #[test]
    fn sticky_rejects_stale_duplicate_reordered_unsolicited_and_malformed_results() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.1));
        session.tick = 1;
        let mut first = explosive(ProjectileKind::Sticky, [0.0; 3]);
        first.presentation.identity = 10;
        first.armed = false;
        first.arm_tick = 100;
        let mut second = first.clone();
        second.presentation.identity = 20;
        session.projectiles = vec![first.clone(), second.clone()];
        session.physics_requests = vec![
            sticky_physics_request(ProjectilePhysicsOperation::Step, &first, 0),
            sticky_physics_request(ProjectilePhysicsOperation::Step, &second, 0),
        ];
        let first_result = sticky_result(&session, 10, [1.0, 0.0, 0.0], None);
        let second_result = sticky_result(&session, 20, [2.0, 0.0, 0.0], None);
        let before = session.producer_snapshot();

        for invalid in [
            vec![first_result],
            vec![second_result, first_result],
            vec![first_result, first_result],
            vec![
                ProjectilePhysicsResult {
                    tick: 0,
                    ..first_result
                },
                second_result,
            ],
            vec![
                ProjectilePhysicsResult {
                    motion_enabled: false,
                    ..first_result
                },
                second_result,
            ],
            vec![
                ProjectilePhysicsResult {
                    contact: Some(ProjectileContact {
                        kind: ProjectileContactKind::Other,
                        normal: [0.0; 3],
                    }),
                    ..first_result
                },
                second_result,
            ],
        ] {
            assert!(matches!(
                session.advance_with_external(Command::default(), &invalid, &[], None),
                Err(Error::InvalidProjectilePhysics)
            ));
            assert_eq!(session.producer_snapshot(), before);
        }

        let mut unsolicited = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.1));
        let result = sticky_result(&unsolicited, 10, [1.0, 0.0, 0.0], None);
        assert!(matches!(
            unsolicited.advance_with_external(Command::default(), &[result], &[], None),
            Err(Error::InvalidProjectilePhysics)
        ));
    }

    #[test]
    fn sticky_bounce_dynamic_stick_arm_and_remote_detonation_preserve_order() {
        let (mut session, projectile) = launch_sticky(0.4);
        let bounce = sticky_result(
            &session,
            projectile,
            [10.0, 0.0, 20.0],
            Some(ProjectileContact {
                kind: ProjectileContactKind::Other,
                normal: [0.0, 1.0, 0.0],
            }),
        );
        let bounced = session
            .advance_with_external(Command::default(), &[bounce], &[], None)
            .unwrap();
        assert_eq!(bounced.projectiles[0].state, ProjectileState::Flying);
        assert_eq!(bounced.projectiles[0].contact_normal, None);
        assert_eq!(bounced.projectile_events.len(), 1);
        assert_eq!(
            bounced.projectile_events[0].kind,
            ProjectileEventKind::Impact
        );
        assert_eq!(
            bounced.projectile_events[0].contact_normal,
            Some([0.0, 1.0, 0.0])
        );

        let contact = sticky_result(
            &session,
            projectile,
            [20.0, 1.0, 15.0],
            Some(ProjectileContact {
                kind: ProjectileContactKind::DynamicProp,
                normal: [0.0, 0.0, 1.0],
            }),
        );
        let stuck = session
            .advance_with_external(Command::default(), &[contact], &[], None)
            .unwrap();
        assert_eq!(stuck.projectiles[0].state, ProjectileState::StuckArmed);
        assert_eq!(stuck.projectiles[0].contact_normal, Some([0.0, 0.0, 1.0]));
        assert_eq!(
            stuck
                .projectile_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [
                ProjectileEventKind::Impact,
                ProjectileEventKind::Stick,
                ProjectileEventKind::Arm
            ]
        );
        assert_eq!(
            session
                .physics_requests()
                .iter()
                .map(|request| request.operation)
                .collect::<Vec<_>>(),
            [ProjectilePhysicsOperation::DisableMotion]
        );

        let exploded = session
            .advance(Command {
                detonate: true,
                ..Command::default()
            })
            .unwrap();
        assert!(exploded.projectiles.is_empty());
        assert_eq!(
            exploded.projectile_events[0].kind,
            ProjectileEventKind::Explode
        );
        assert_eq!(
            session
                .physics_requests()
                .iter()
                .map(|request| request.operation)
                .collect::<Vec<_>>(),
            [ProjectilePhysicsOperation::Destroy]
        );
    }

    #[test]
    fn sticky_fizzle_and_oldest_bomb_cleanup_emit_current_physics_requests() {
        let (mut fizzle, projectile) = launch_sticky(0.4);
        let result = sticky_result(&fizzle, projectile, [10.0, 0.0, 20.0], None);
        let fizzled = fizzle
            .advance_with_external(
                Command {
                    select_team: Some(PlayerTeam::Blue),
                    ..Command::default()
                },
                &[result],
                &[],
                None,
            )
            .unwrap();
        assert!(fizzled.projectiles.is_empty());
        assert_eq!(
            fizzled.projectile_events[0].kind,
            ProjectileEventKind::Fizzle
        );
        assert_eq!(
            fizzle
                .physics_requests()
                .iter()
                .map(|request| (request.operation, request.projectile))
                .collect::<Vec<_>>(),
            [(ProjectilePhysicsOperation::Destroy, projectile)]
        );

        let mut oldest = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.2));
        oldest.class = PlayerClass::Demoman;
        oldest.weapon = Some(Weapon::StickybombLauncher);
        oldest.loadout = default_loadout(PlayerClass::Demoman);
        for identity in 1..=8 {
            let mut sticky = explosive(ProjectileKind::Sticky, [identity as f32, 0.0, 0.0]);
            sticky.presentation.identity = identity;
            oldest.projectiles.push(sticky);
        }
        oldest.next_projectile = 9;
        let mut events = Vec::new();
        oldest
            .fire_projectile(0.0, 0.0, 0.0, None, None, &mut events)
            .unwrap();
        assert_eq!(oldest.projectiles[0].forced_detonate_tick, Some(0));
        assert_eq!(oldest.projectiles.last().unwrap().presentation.identity, 9);
        oldest.tick = 1;
        oldest
            .advance_projectiles(0.2, &mut events, &mut Vec::new())
            .unwrap();
        assert!(
            oldest
                .projectiles
                .iter()
                .all(|sticky| sticky.presentation.identity != 1)
        );
        assert!(
            events.iter().any(|event| {
                event.kind == ProjectileEventKind::Explode && event.projectile == 1
            })
        );
        assert!(oldest.physics_requests().iter().any(|request| {
            request.operation == ProjectilePhysicsOperation::Destroy && request.projectile == 1
        }));
    }

    #[test]
    fn explosion_samples_preserve_source_order_no_repeat_and_restore_state() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        let initial = session.random_state();
        let expected = [
            (0x3ed4_fdde, 0x3dbc_5817, 1, 0x3f07_9a6f),
            (0x3f6e_3116, 0x3ec4_5a62, 0, 0x3d88_e495),
            (0x3f39_0046, 0x3f2b_d072, 2, 0x3ec4_4f0e),
            (0x3f21_b2d0, 0x3f62_7c2b, 2, 0x3f26_c9ec),
            (0x3e73_7b24, 0x3e86_603d, 0, 0x3f40_dbee),
            (0x3f68_c1dd, 0x3d94_dc56, 1, 0x3e8b_a0a4),
        ];
        for (volume, pitch, wave, level) in expected {
            let samples = session.sample_sound(
                RandomContext::PredictedPresentation,
                SoundDefinition::RocketExplosion,
                SoundQueryPhase::Emit,
            );
            assert_eq!(samples.volume.to_bits(), volume);
            assert_eq!(samples.pitch.to_bits(), pitch);
            assert_eq!(samples.wave, wave);
            assert_eq!(samples.sound_level.to_bits(), level);
        }
        assert_eq!(
            session.random_state().sound_selection,
            SoundSelectionState {
                rocket_explosion_available: 0,
                sticky_explosion_available: 0b111,
                bat_hit_world_available: 0b11,

                shovel_hit_world_available: 0b11,
                shovel_hit_flesh_available: 0b111,
                fist_miss_available: 0b11,
                fist_hit_world_available: 0b11,
                fist_hit_flesh_available: 0b111,
                kukri_hit_flesh_available: 0b111,
                kukri_hit_world_available: 0b11,
                wrench_hit_flesh_available: 0b111,
                fire_axe_hit_world_available: 0b11,
                fire_axe_hit_flesh_available: 0b111,
                flag_enemy_stolen_available: 0b1111,
                flag_enemy_dropped_available: 0b11,
                flag_enemy_captured_available: 0b111,
                flag_enemy_returned_available: 0b111,
                flag_team_dropped_available: 0b11,
                bottle_hit_flesh_available: 0b111,
                bottle_hit_world_available: 0b111,
                knife_hit_flesh_available: 0b111,
                bonesaw_hit_flesh_available: 0b111,
                bonesaw_hit_world_available: 0b11,
                overtime_available: 0b1111,
                control_point_available: 0x1fff,
            }
        );

        session.restore_random_state(initial).unwrap();
        let replay = session.sample_sound(
            RandomContext::PredictedPresentation,
            SoundDefinition::RocketExplosion,
            SoundQueryPhase::Emit,
        );
        assert_eq!(
            replay,
            SoundSamples {
                volume: f32::from_bits(0x3ed4_fdde),
                pitch: f32::from_bits(0x3dbc_5817),
                wave: 1,
                sound_level: f32::from_bits(0x3f07_9a6f),
            }
        );
        let sticky = session.sample_sound(
            RandomContext::PredictedPresentation,
            SoundDefinition::StickyExplosion,
            SoundQueryPhase::Emit,
        );
        assert_eq!(sticky.wave, 0);
        assert_eq!(
            session.random_state().sound_selection,
            SoundSelectionState {
                rocket_explosion_available: 0b101,
                sticky_explosion_available: 0b110,
                bat_hit_world_available: 0b11,

                shovel_hit_world_available: 0b11,
                shovel_hit_flesh_available: 0b111,
                fist_miss_available: 0b11,
                fist_hit_world_available: 0b11,
                fist_hit_flesh_available: 0b111,
                kukri_hit_flesh_available: 0b111,
                kukri_hit_world_available: 0b11,
                wrench_hit_flesh_available: 0b111,
                fire_axe_hit_world_available: 0b11,
                fire_axe_hit_flesh_available: 0b111,
                flag_enemy_stolen_available: 0b1111,
                flag_enemy_dropped_available: 0b11,
                flag_enemy_captured_available: 0b111,
                flag_enemy_returned_available: 0b111,
                flag_team_dropped_available: 0b11,
                bottle_hit_flesh_available: 0b111,
                bottle_hit_world_available: 0b111,
                knife_hit_flesh_available: 0b111,
                bonesaw_hit_flesh_available: 0b111,
                bonesaw_hit_world_available: 0b11,
                overtime_available: 0b1111,
                control_point_available: 0x1fff,
            }
        );
    }

    #[test]
    fn movement_mode_ammo_reload_and_atomic_failure_share_one_state() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        session
            .loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .next_primary_tick = 0;
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
        let request = session.rocket_trace_requests()[0];
        let failure = session
            .advance_with_external(
                Command {
                    pitch_degrees: f32::NAN,
                    select_class: Some(PlayerClass::Demoman),
                    ..Command::default()
                },
                &[],
                &[RocketTraceResult {
                    projectile: request.projectile,
                    tick: session.tick,
                    end: request.end,
                    solid: false,
                    sky: false,
                    normal: None,
                    direct_target: None,
                }],
                None,
            )
            .unwrap_err();
        assert!(matches!(
            failure,
            Error::Movement(MoveError {
                operation: Operation::Validate,
                kind: FailureKind::Malformed,
                ..
            })
        ));
        assert_eq!(session.class, PlayerClass::Soldier);
        assert_eq!(session.movement_snapshot_bytes(), before);
    }

    #[test]
    fn equipment_restore_does_not_arm_an_unassigned_or_spectating_player() {
        let mut session = Session::connected(Floor, [0.0; 3], MapRuntime::empty(0.015), team_selection::TeamRules::default());
        let saved = equipment::Equipment::default().persist();
        session.restore_equipment(&saved).unwrap();
        let welcome = session.advance(Command::default()).unwrap();
        assert_eq!(welcome.weapon, None);
        assert!(welcome.loadout.is_empty());
        session.select_team_choice(team_selection::TeamChoice::Spectator).unwrap();
        session.restore_equipment(&saved).unwrap();
        let observer = session.advance(Command::default()).unwrap();
        assert_eq!(observer.weapon, None);
        assert!(observer.loadout.is_empty());
        assert_eq!(session.equipment().persist(), saved);
        let before = session.equipment().clone();
        assert!(session.restore_equipment(&saved[..10]).is_err());
        assert_eq!(session.equipment(), &before);
    }

    #[test]
    fn class_and_weapon_switches_apply_deploy_before_held_primary() {
        let mut session = Session::new(Floor, [0.0; 3], MapRuntime::empty(0.015));
        for _ in 0..10 {
            session.advance(Command::default()).unwrap();
        }
        let switched = session
            .advance(Command {
                select_weapon: Some(Weapon::Shotgun),
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(switched.weapon, Some(Weapon::Shotgun));
        assert!(switched.projectile_events.is_empty());
        let shotgun = session.weapon_runtime(Weapon::Shotgun).unwrap();
        assert_eq!(shotgun.next_primary_tick, 44);
        assert_eq!(shotgun.first_primary_tick, 44);

        let changed = session
            .advance(Command {
                select_class: Some(PlayerClass::Demoman),
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(changed.class, PlayerClass::Demoman);
        assert!(changed.projectile_events.is_empty());
        let grenade = session.weapon_runtime(Weapon::GrenadeLauncher).unwrap();
        assert_eq!(grenade.next_primary_tick, 45);
        assert_eq!(grenade.first_primary_tick, 45);
        assert!(grenade.charge_begin_tick.is_none());
    }

    #[test]
    fn lethal_state_waits_for_explicit_respawn_and_cleans_owned_projectiles() {
        let mut session = Session::new(Floor, [8.0, 4.0, 2.0], MapRuntime::empty(0.01));
        session.movement_configuration.tick_interval = 0.01;
        session.health = 0;
        session
            .projectiles
            .push(explosive(ProjectileKind::Rocket, [0.0; 3]));
        session
            .projectiles
            .push(explosive(ProjectileKind::Sticky, [0.0; 3]));
        let dead = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
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
                fire: true,
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
        assert_eq!(
            session
                .weapon_runtime(Weapon::RocketLauncher)
                .unwrap()
                .next_primary_tick,
            51
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
                body: 0,
                open_animation: RegenerateModelAnimation::Open,
                close_animation: RegenerateModelAnimation::Close,
            }]
        );
        assert_eq!(
            session.regenerate_contacts(),
            &[RegenerateContact {
                sequence: 1,
                entity: 1,
                kind: playsrc_entity::ContactKind::Enter,
                enabled: true,
            }]
        );
        assert_eq!(
            session.regenerate_model_events(),
            &[RegenerateModelEvent {
                zone: 1,
                associated_model: 0,
                tick: 1,
                animation: RegenerateModelAnimation::Open,
                body: 0,
            }]
        );
        assert_eq!(
            session.regenerate_model_animation(0),
            Some(RegenerateModelAnimation::Open)
        );
        session.health = 100;
        let cooling_down = session.advance(Command::default()).unwrap();
        assert_eq!(cooling_down.health, 100.0);
        assert!(cooling_down.events.is_empty());
        assert_eq!(
            session.regenerate_contacts()[0].kind,
            playsrc_entity::ContactKind::Stay
        );

        session.next_regenerate_tick = 0;
        session.health = 50;
        session.set_player_restrictions(PlayerRestrictions {
            taunting: true,
            ..PlayerRestrictions::default()
        });
        let taunting = session.advance(Command::default()).unwrap();
        assert_eq!(taunting.health, 50.0);
        assert!(taunting.events.is_empty());

        touching.store(false, Ordering::Relaxed);
        session.advance(Command::default()).unwrap();
        assert_eq!(
            session.regenerate_contacts()[0].kind,
            playsrc_entity::ContactKind::Exit
        );
        let mut close = None;
        while session.tick <= 135 {
            session.advance(Command::default()).unwrap();
            if let Some(event) = session.regenerate_model_events().first().copied() {
                close = Some(event);
            }
        }
        assert_eq!(
            close,
            Some(RegenerateModelEvent {
                zone: 1,
                associated_model: 0,
                tick: 135,
                animation: RegenerateModelAnimation::Close,
                body: 0,
            })
        );
        assert_eq!(
            session.regenerate_model_animation(0),
            Some(RegenerateModelAnimation::Close)
        );

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
    fn medic_syringes_preserve_stock_clip_offset_spread_gravity_and_nonexplosive_impact() {
        let mut session = Session::new(Floor, [0.0, 0.0, 20.0], MapRuntime::empty(0.015));
        session
            .advance(Command {
                select_class: Some(PlayerClass::Medic),
                ..Command::default()
            })
            .unwrap();
        let deploy = session
            .weapon_runtime(Weapon::SyringeGun)
            .unwrap()
            .next_primary_tick;
        while session.tick < deploy {
            session.advance(Command::default()).unwrap();
        }
        let fired = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert_eq!(fired.projectiles.len(), 1);
        let syringe = &fired.projectiles[0];
        assert_eq!(syringe.kind, ProjectileKind::Syringe);
        assert_eq!(session.weapon_runtime(Weapon::SyringeGun).unwrap().clip, 39);
        assert!((syringe.position[0] - 16.0).abs() < 0.001);
        assert!((syringe.position[1] + 6.0).abs() < 0.001);
        assert!((length(syringe.velocity) - medic::SYRINGE_SPEED).abs() < 5.0);
        assert!(
            session
                .random_draws()
                .iter()
                .any(|draw| { matches!(draw.decision, RandomDecision::SyringePitchSpread) })
        );
        let request = session.rocket_trace_requests()[0];
        let impact = session
            .advance_with_external(
                Command::default(),
                &[],
                &[RocketTraceResult {
                    projectile: request.projectile,
                    tick: session.tick,
                    end: request.end,
                    solid: true,
                    sky: false,
                    normal: Some([0.0, 0.0, 1.0]),
                    direct_target: None,
                }],
                None,
            )
            .unwrap();
        assert!(impact.projectiles.is_empty());
        assert_eq!(
            impact
                .projectile_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![ProjectileEventKind::Impact, ProjectileEventKind::Fizzle]
        );
        assert!(session.radius_damage_requests().is_empty());
    }

    #[test]
    fn held_stock_fire_inside_regenerate_preserves_cadence_across_refills() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"prop_dynamic\"\"targetname\"\"locker\"\"SetBodyGroup\"\"0\"}{\"classname\"\"func_regenerate\"\"model\"\"*1\"\"associatedmodel\"\"locker\"}",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            8,
            vec![playsrc_entity::ModelBounds {
                model: 1,
                mins: [-16.0; 3],
                maxs: [16.0; 3],
            }],
        )
        .unwrap();
        let mut session = Session::new(SwitchWorld(Arc::new(AtomicBool::new(true))), [0.0; 3], map);
        let mut fire_ticks = Vec::new();
        let mut resupply_ticks = Vec::new();
        for _ in 0..=250 {
            let rocket_results = session
                .rocket_trace_requests()
                .iter()
                .map(|request| RocketTraceResult {
                    projectile: request.projectile,
                    tick: session.tick,
                    end: request.end,
                    solid: true,
                    sky: true,
                    normal: None,
                    direct_target: None,
                })
                .collect::<Vec<_>>();
            let snapshot = session
                .advance_with_external(
                    Command {
                        fire: true,
                        ..Command::default()
                    },
                    &[],
                    &rocket_results,
                    None,
                )
                .unwrap();
            fire_ticks.extend(
                snapshot
                    .projectile_events
                    .iter()
                    .filter(|event| event.kind == ProjectileEventKind::Fire)
                    .map(|event| event.tick),
            );
            if snapshot
                .events
                .iter()
                .any(|event| matches!(event, Event::Resupplied { .. }))
            {
                resupply_ticks.push(snapshot.tick - 1);
            }
        }
        assert_eq!(fire_ticks, [34, 88, 142, 196, 250]);
        assert_eq!(resupply_ticks, [0, 200]);
        assert_eq!(
            fire_ticks
                .windows(2)
                .map(|ticks| ticks[1] - ticks[0])
                .collect::<Vec<_>>(),
            [54, 54, 54, 54]
        );
        let weapon = session.weapon_runtime(Weapon::RocketLauncher).unwrap();
        assert_eq!((weapon.clip, weapon.reserve), (3, 20));
        assert_eq!(weapon.first_primary_tick, 267);
        assert!(
            session
                .activity_events()
                .iter()
                .all(|event| event.activity != weapon::WeaponActivity::ReloadStart)
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

    #[test]
    fn cyclic_linear_completion_requests_survive_until_the_movement_owner_consumes_them() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"logic_auto\"\"OnMapSpawn\"\"platform,Open,,0,-1\"}{\"classname\"\"func_movelinear\"\"model\"\"*1\"\"targetname\"\"platform\"\"speed\"\"75\"\"MoveDistance\"\"650\"\"movedir\"\"0 90 0\"\"OnFullyOpen\"\"platform,Close,,0,-1\"\"OnFullyClosed\"\"platform,Open,,0,-1\"}",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            9,
            vec![playsrc_entity::ModelBounds {
                model: 1,
                mins: [-73.3, -56.0, -4.0],
                maxs: [46.7, 56.0, 4.0],
            }],
        )
        .unwrap();
        let mut session = Session::new(Floor, [0.0; 3], map);
        let opening = loop {
            session.advance(Command::default()).unwrap();
            if let Some(request) = session.mover_requests().first().copied() {
                break request;
            }
            assert!(session.producer_snapshot().tick <= 15);
        };
        assert!(opening.opening);
        let opened = session
            .apply_mover_results(&[MoverResult {
                request_id: opening.request_id,
                entity: opening.entity,
                kind: MoverResultKind::Completed,
                transform: playsrc_entity::Transform {
                    origin: opening.destination,
                    angles: [0.0; 3],
                },
                carry: [12.0, 0.0, 0.0],
            }])
            .unwrap();
        assert_eq!(opened.carry, [12.0, 0.0, 0.0]);
        let closing = session.mover_requests()[0];
        assert!(!closing.opening);
        assert_ne!(closing.request_id, opening.request_id);
        assert_eq!(session.movement.position[0], 12.0);

        session.advance(Command::default()).unwrap();
        assert_eq!(session.mover_requests(), &[closing]);
        session
            .apply_mover_results(&[MoverResult {
                request_id: closing.request_id,
                entity: closing.entity,
                kind: MoverResultKind::Completed,
                transform: playsrc_entity::Transform {
                    origin: closing.destination,
                    angles: [0.0; 3],
                },
                carry: [0.0; 3],
            }])
            .unwrap();
        let reopened = session.mover_requests()[0];
        assert!(reopened.opening);
        assert_eq!(reopened.destination, opening.destination);
    }
}
pub mod presentation;
pub mod class_selection;
