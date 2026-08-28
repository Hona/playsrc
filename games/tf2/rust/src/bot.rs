use std::{collections::BTreeMap, sync::Arc};

use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph};
use playsrc_movement::{
    Command as MoveCommand, Configuration as MovementConfiguration, Player, State, StepInput, step,
};
use playsrc_nav::{Area, Direction, Mesh, PathScratch};

#[path = "bot_control_point.rs"]
mod control_point;
#[path = "bot_path.rs"]
mod path;
#[path = "bot_locomotion.rs"]
mod locomotion;

use crate::{
    GameplayWorld, MovementModifiers, MovementPolicy, PlayerClass, PlayerLifecycle, PlayerTeam,
    UniformRandomStream, Weapon, WeaponState, ballistics,
    condition::{ConditionId, ConditionState},
    damage::{
        self, CritKind, CustomDamage, DamageInput, DamageModifiers, DamageSourceKind, DamageType,
    },
    health::{HealthConfiguration, HealthState},
    medic::PatientFacts,
    weapon::{ActivityEvent, AmmoEvent, PrimaryResult, WeaponRuntime},
};

pub const MAX_BOTS: usize = 31;
pub const QUOTA_THINK_INTERVAL: f32 = 0.25;

const BOT_NAMES: &[&str] = &[
    "Chucklenuts",
    "CryBaby",
    "WITCH",
    "ThatGuy",
    "Still Alive",
    "Hat-Wearing MAN",
    "Me",
    "Numnutz",
    "H@XX0RZ",
    "The G-Man",
    "Chell",
    "The Combine",
    "Totally Not A Bot",
    "Pow!",
    "Zepheniah Mann",
    "THEM",
    "LOS LOS LOS",
    "10001011101",
    "DeadHead",
    "ZAWMBEEZ",
    "MindlessElectrons",
    "TAAAAANK!",
    "The Freeman",
    "Black Mesa",
    "Soulless",
    "CEDA",
    "BeepBeepBoop",
    "NotMe",
    "CreditToTeam",
    "BoomerBile",
    "Someone Else",
    "Mann Co.",
    "Dog",
    "Kaboom!",
    "AmNot",
    "0xDEADBEEF",
    "HI THERE",
    "SomeDude",
    "GLaDOS",
    "Hostage",
    "Headful of Eyeballs",
    "CrySomeMore",
    "Aperture Science Prototype XR7",
    "Humans Are Weak",
    "AimBot",
    "C++",
    "GutsAndGlory!",
    "Nobody",
    "Saxton Hale",
    "RageQuit",
    "Screamin' Eagles",
    "Ze Ubermensch",
    "Maggot",
    "CRITRAWKETS",
    "Herr Doktor",
    "Gentlemanne of Leisure",
    "Companion Cube",
    "Target Practice",
    "One-Man Cheeseburger Apocalypse",
    "Crowbar",
    "Delicious Cake",
    "IvanTheSpaceBiker",
    "I LIVE!",
    "Cannon Fodder",
    "trigger_hurt",
    "Nom Nom Nom",
    "Divide by Zero",
    "GENTLE MANNE of LEISURE",
    "MoreGun",
    "Tiny Baby Man",
    "Big Mean Muther Hubbard",
    "Force of Nature",
    "Crazed Gunman",
    "Grim Bloody Fable",
    "Poopy Joe",
    "A Professional With Standards",
    "Freakin' Unbelievable",
    "SMELLY UNFORTUNATE",
    "The Administrator",
    "Mentlegen",
    "Archimedes!",
    "Ribs Grow Back",
    "It's Filthy in There!",
    "Mega Baboon",
    "Kill Me",
    "Glorified Toaster with Legs",
];
pub const TF_NAV_BLOCKED: u32 = 0x0000_0001;
pub const TF_NAV_SPAWN_ROOM_RED: u32 = 0x0000_0002;
pub const TF_NAV_SPAWN_ROOM_BLUE: u32 = 0x0000_0004;
pub const TF_NAV_BLUE_SETUP_GATE: u32 = 0x0000_0800;
pub const TF_NAV_RED_SETUP_GATE: u32 = 0x0000_1000;
pub const TF_NAV_BLUE_ONE_WAY_DOOR: u32 = 0x0000_8000;
pub const TF_NAV_RED_ONE_WAY_DOOR: u32 = 0x0001_0000;
pub const TF_NAV_UNBLOCKABLE: u32 = 0x4000_0000;
pub const STEP_HEIGHT: f32 = 18.0;
pub const MAX_JUMP_HEIGHT: f32 = 72.0;
pub const DEATH_DROP_HEIGHT: f32 = 1000.0;
pub const PATH_LOOKAHEAD_RANGE: f32 = 300.0;
pub const PAYLOAD_PUSH_RADIUS: f32 = 60.0;
pub const PAYLOAD_GUARD_RANGE: f32 = 1000.0;
pub const TARGET_SELECTION_INTERVAL: f32 = 0.3;
pub const MAX_VISION_RANGE: f32 = 6000.0;
pub const IMMEDIATE_THREAT_RANGE: f32 = 500.0;
pub const SOLDIER_SECONDARY_RANGE: f32 = 500.0;
pub const SNIPER_SECONDARY_RANGE: f32 = 750.0;
pub const ROCKET_MAX_ATTACK_RANGE: f32 = 3000.0;
pub const ROCKET_DESIRED_ATTACK_RANGE: f32 = 1250.0;
pub const RANGED_DESIRED_ATTACK_RANGE: f32 = 500.0;
pub const MELEE_MAX_ATTACK_RANGE: f32 = 100.0;
pub const MELEE_FIRE_RANGE: f32 = 250.0;
pub const ROCKET_LEAD_MINIMUM_RANGE: f32 = 150.0;
pub const ROCKET_SPEED: f32 = 1100.0;
pub const HEALTH_CRITICAL_RATIO: f32 = 0.3;
pub const HEALTH_OK_RATIO: f32 = 0.8;
pub const HEALTH_SEARCH_NEAR_RANGE: f32 = 1000.0;
pub const HEALTH_SEARCH_FAR_RANGE: f32 = 2000.0;
pub const AMMO_SEARCH_RANGE: f32 = 5000.0;
pub const LOW_AMMO_RATIO: f32 = 0.2;
pub const FLAG_TRIGGER_BLOAT: f32 = 24.0;
pub const FLAG_RETURN_SECONDS: f32 = 60.0;
pub const RESPAWN_WAVE_SECONDS: f32 = 10.0;
pub const DEATH_ANIMATION_SECONDS: f32 = 6.4;
const PLAYER_HULL: Hull = Hull {
    mins: [-24.0, -24.0, 0.0],
    maxs: [24.0, 24.0, 82.0],
};
const POINT_HULL: Hull = Hull {
    mins: [0.0; 3],
    maxs: [0.0; 3],
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Difficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
    Expert = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QuotaMode {
    Normal = 0,
    Fill = 1,
    Match = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Configuration {
    pub quota: u8,
    pub maximum_players: u8,
    pub mode: QuotaMode,
    pub difficulty: Difficulty,
    pub join_after_player: bool,
    pub auto_vacate: bool,
    pub offline_practice: bool,
}

impl Difficulty {
    pub const fn recognition_seconds(self) -> f32 {
        match self {
            Self::Easy => 1.0,
            Self::Normal => 0.5,
            Self::Hard => 0.3,
            Self::Expert => 0.2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Add,
    KickAll,
    KickTeam(PlayerTeam),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub operation: Operation,
    pub count: u8,
    pub class: Option<PlayerClass>,
    pub team: Option<PlayerTeam>,
    pub difficulty: Difficulty,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    Teleport {
        identity: u32,
        position: [f32; 3],
        pitch_degrees: f32,
        yaw_degrees: f32,
    },
    Whack {
        identity: u32,
    },
    StealthCondition {
        identity: u32,
        enabled: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ObjectiveKind {
    PayloadPush = 1,
    PayloadGuard = 2,
    FetchFlag = 3,
    DeliverFlag = 4,
    Attack = 5,
    GetHealth = 6,
    GetAmmo = 7,
    CapturePoint = 8,
    DefendPoint = 9,
    BlockCapture = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    Default,
    Fastest,
    Safest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AnimationRole {
    Primary = 1,
    Secondary = 2,
    Melee = 3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathContext {
    pub team: PlayerTeam,
    pub bot_identity: u32,
    pub now: f32,
    pub route: Route,
    pub combat_intensity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub weapon_definition: Option<u32>,
    pub conditions: [u32; 5],
    pub equipped_items: Vec<crate::equipment::EquippedItem>,
    pub identity: u32,
    pub spy: Option<crate::spy::SpyState>,
    pub name: String,
    pub class: PlayerClass,
    pub team: PlayerTeam,
    pub lifecycle: PlayerLifecycle,
    pub difficulty: Difficulty,
    pub objective: ObjectiveKind,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub health: i32,
    pub maximum_health: i32,
    pub target: Option<u32>,
    pub area: Option<u32>,
    pub remaining_path_areas: u32,
    pub weapon: Option<WeaponState>,
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    pub assists: u32,
    pub deaths: u32,
    pub captures: u32,
    pub defenses: u32,
    pub damage: u32,
    pub killstreak: u32,
    pub carrying_flag: bool,
    pub animation_role: AnimationRole,
    pub last_fire_tick: Option<u64>,
    pub respawn_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Human {
    pub team: PlayerTeam,
    pub class: PlayerClass,
    pub alive: bool,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
}

#[derive(Clone, Copy)]
pub struct Objectives<'a> {
    pub rules: &'a crate::round::Rules,
    pub flags: Option<&'a crate::ctf::World>,
    pub points: Option<&'a crate::control_point::World>,
    pub in_setup: bool,
    pub in_overtime: bool,
    pub time_left: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SupplyTarget {
    pub identity: u32,
    pub kind: Option<crate::pickup::MapPickupKind>,
    pub team: Option<PlayerTeam>,
    pub position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttackPhase { Fire, MeleeSwing, MeleeSmack }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attack {
    pub phase: AttackPhase,
    pub attacker: u32,
    pub team: PlayerTeam,
    pub weapon: Weapon,
    pub target: u32,
    pub position: [f32; 3],
    pub eye_position: [f32; 3],
    pub pitch_degrees: f32,
    pub yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Damage {
    /// Captured launcher lifetime. None is an absent source, never a request to
    /// substitute whichever item the attacker currently holds.
    pub source_weapon: Option<crate::weapon::WeaponSource>,
    pub damage_type: DamageType,
    pub force: [f32; 3],
    pub crit: CritKind,
    pub range_multiplier: f32,
    pub custom: CustomDamage,
    pub modifiers: DamageModifiers,
    pub killing_weapon: Option<&'static str>,
    pub attacker: u32,
    pub victim: u32,
    pub weapon: Weapon,
    pub amount: f32,
    pub position: [f32; 3],
}

impl Damage {
    pub fn normalized_damage_type(self) -> DamageType {
        match self.custom {
            CustomDamage::Bleeding => DamageType::SLASH,
            CustomDamage::Burning => DamageType::BURN | DamageType::PREVENT_FORCE,
            _ => self.damage_type,
        }
    }

    pub fn input(self, attacker_team: PlayerTeam, victim_team: PlayerTeam,
        attacker_conditions: ConditionState) -> DamageInput {
        DamageInput {
            attacker: self.attacker, attacker_team, attacker_conditions,
            source: if self.attacker == 0 { DamageSourceKind::World } else { DamageSourceKind::Player },
            weapon_position: None, victim: self.victim, victim_team, base_damage: self.amount,
            range_multiplier: self.range_multiplier, damage_type: self.normalized_damage_type(),
            custom: self.custom, crit: self.crit, friendly_fire: false,
            force_friendly_fire: false, bypass_invulnerability: false, force: self.force,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatTarget {
    pub blast_jump_state: bool,
    pub crouched: bool,
    pub in_water: bool,
    pub identity: u32,
    pub class: PlayerClass,
    pub team: PlayerTeam,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub burning: bool,
    pub hull: Hull,
    pub grounded: bool,
    pub water_level: u8,
    pub conditions: [u32; 5],
}

impl CombatTarget {
    pub fn world_center(self) -> [f32; 3] {
        std::array::from_fn(|axis| self.position[axis] + (self.hull.mins[axis] + self.hull.maxs[axis]) * 0.5)
    }

    pub fn condition(self, value: usize) -> bool {
        self.conditions[value / 32] & (1 << (value % 32)) != 0
    }

    pub fn in_air_due_to_explosion(self) -> bool {
        (!self.grounded && self.water_level == 0 && self.blast_jump_state) || self.condition(125)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Spawn {
    identity: u32,
    class_flags: u32,
    position: [f32; 3],
    angles: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptureZone {
    position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Flag {
    home: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scenario {
    ControlPoints { initial: [f32; 3] },
    Payload {
        cart: [f32; 3],
        forward: [f32; 3],
    },
    CaptureTheFlag {
        flags: [Flag; 2],
        captures: [CaptureZone; 2],
    },
}

#[derive(Clone, Copy, Debug)]
struct Actor {
    identity: u32,
    class: PlayerClass,
    team: PlayerTeam,
    alive: bool,
    position: [f32; 3],
    velocity: [f32; 3],
    firing_at: Option<u32>,
}

impl Actor {
    fn eye(self) -> [f32; 3] {
        [
            self.position[0],
            self.position[1],
            self.position[2] + self.class.standing_eye_height(),
        ]
    }
    fn center(self) -> [f32; 3] {
        [self.position[0], self.position[1], self.position[2] + 41.0]
    }
}

struct ActorFrame {
    actors: [Actor; MAX_BOTS + 1],
    count: usize,
    teams: [[u8; MAX_BOTS + 1]; 2],
    team_counts: [usize; 2],
}

impl ActorFrame {
    fn insert(&mut self, actor: Actor) {
        let index = self.count;
        self.actors[index] = actor;
        self.count += 1;
        if actor.alive && matches!(actor.team, PlayerTeam::Red | PlayerTeam::Blue) {
            let team = team_index(actor.team);
            self.teams[team][self.team_counts[team]] = index as u8;
            self.team_counts[team] += 1;
        }
    }

    fn all(&self) -> &[Actor] {
        &self.actors[..self.count]
    }

    fn enemies(&self, team: PlayerTeam) -> impl Iterator<Item = Actor> + '_ {
        let enemy = 1 - team_index(team);
        self.teams[enemy][..self.team_counts[enemy]]
            .iter()
            .map(|index| self.actors[usize::from(*index)])
    }

    fn get(&self, identity: u32) -> Option<Actor> {
        self.all()
            .binary_search_by_key(&identity, |actor| actor.identity)
            .ok()
            .map(|index| self.actors[index])
    }
}

#[derive(Clone, Copy)]
struct SupplyFacts {
    area: u32,
    closest_team: Option<PlayerTeam>,
}

#[derive(Clone, Copy)]
struct SupplyRoute {
    team: PlayerTeam,
    start: u32,
    goal: u32,
    travel: Option<f32>,
}

#[derive(Default)]
struct SupplyCache {
    facts: Vec<Option<Option<SupplyFacts>>>,
    routes: Vec<SupplyRoute>,
}

#[derive(Clone, Debug)]
struct BotEquipment {
    selected: crate::equipment::Equipment,
    active: Vec<crate::equipment::EquippedItem>,
    class: PlayerClass,
    providers: crate::equipment::AttributeProviders,
    next_generation: u64,
}

#[derive(Clone, Debug)]
struct Bot {
    equipment: Option<Box<BotEquipment>>,
    movement_stuns: crate::hitscan::MovementStuns,
    weapon_knockback: bool,
    scattergun_jumped: bool,
    blast_since_movement: bool,
    blast_jump_state: bool,
    in_water: bool,
    decapitations: i32,
    critical_history: crate::critical::PlayerHistory,
    identity: u32,
    spy: Option<crate::spy::SpyState>,
    name: String,
    class: PlayerClass,
    team: PlayerTeam,
    lifecycle: PlayerLifecycle,
    difficulty: Difficulty,
    movement: State,
    yaw_degrees: f32,
    pitch_degrees: f32,
    health: HealthState,
    damagers: crate::deathnotice::DamagerHistory,
    conditions: ConditionState,
    afterburn: Option<crate::pyro::Afterburn>,
    last_flame_damage_time: f32,
    ammo: crate::class::AmmoLedger,
    next_regenerate_tick: u64,
    objective: ObjectiveKind,
    target: Option<u32>,
    known_since: BTreeMap<u32, u64>,
    next_target_tick: u64,
    next_repath_tick: u64,
    current_area: Option<u32>,
    path: Vec<u32>,
    path_index: usize,
    crossing: Option<path::Crossing>,
    path_crossings: Arc<Vec<path::Crossing>>,
    nav_area_mark: Option<[f32; 3]>,
    avoid_at: f32,
    locomotion_command: MoveCommand,
    stuck: locomotion::Monitor,
    goal: [f32; 3],
    point_action: control_point::Action,
    loadout: BTreeMap<Weapon, WeaponRuntime>,
    active_weapon: Option<Weapon>,
    last_fire_tick: Option<u64>,
    pending_melee: Option<(u64, u32, Weapon)>,
    respawn_tick: Option<u64>,
    death_tick: Option<u64>,
    carrying_flag: Option<PlayerTeam>,
    shots: u32,
    hits: u32,
    kills: u32,
    assists: u32,
    deaths: u32,
    captures: u32,
    defenses: u32,
    damage_dealt: u32,
    killstreak: u32,
}

#[derive(Clone, Debug)]
pub struct BotWorld {
    scheduler: locomotion::Scheduler,
    mesh: Arc<Mesh>,
    spawns: [Vec<Spawn>; 2],
    scenario: Scenario,
    bots: BTreeMap<u32, Bot>,
    next_identity: u32,
    next_name: Option<usize>,
    tick_interval: f32,
    configuration: Option<Configuration>,
    next_quota_think: f32,
    path_scratch: PathScratch,
    point_navigation: control_point::Navigation,
    point_spawn_revision: Option<u64>,
    last_spawn: [Option<u32>; 2],
    round_winner: Option<PlayerTeam>,
    navigation_recompute_at: Option<f32>,
    had_bots: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    MissingScenario,
    MissingPayload,
    MissingPathTrack,
    MissingSpawn(PlayerTeam),
    InvalidEntity,
    Limit,
    Movement(playsrc_movement::Error),
    Damage,
    InvalidWorkClock,
}

impl BotWorld {
    pub fn configure_control_points(&mut self, points: &crate::control_point::World) -> Result<(), Error> {
        self.synchronize_control_point_spawns(points);
        self.point_navigation = control_point::Navigation::compile(&self.mesh, points, &self.spawns)?;
        Ok(())
    }

    pub fn synchronize_control_point_spawns(&mut self, points: &crate::control_point::World) {
        if self.point_spawn_revision == Some(points.spawn_revision()) { return; }
        self.point_spawn_revision = Some(points.spawn_revision());
        for team in [PlayerTeam::Red, PlayerTeam::Blue] {
            self.spawns[team_index(team)] = points.spawns().iter().filter(|s| s.team == team && !s.disabled).map(|s| Spawn { identity: s.identity, class_flags: s.class_flags, position: s.position, angles: s.angles }).collect();
        }
    }

    pub fn control_point_round_spawn(&mut self, points: &crate::control_point::World, now: f32) {
        self.last_spawn = [None; 2];
        self.synchronize_control_point_spawns(points);
        self.point_navigation.reset_combat();
        self.navigation_recompute_at = Some(now + 2.0);
    }

    pub fn control_point_events(&mut self, points: &crate::control_point::World, events: &[crate::control_point::Event], now: f32) {
        for event in events {
            if matches!(event, crate::control_point::Event::LockChanged { locked: false, .. }) || matches!(event, crate::control_point::Event::Captured { point, cappers, .. } if !cappers.is_empty() && !points.points()[*point].print_name.is_empty()) {
                self.navigation_recompute_at = Some(now + 2.0);
                for bot in self.bots.values_mut() { bot.next_repath_tick = 0; }
            }
        }
    }

    pub fn switch_teams(&mut self) {
        for bot in self.bots.values_mut() {
            bot.team = if bot.team == PlayerTeam::Red { PlayerTeam::Blue } else { PlayerTeam::Red };
        }
    }

    pub fn new<W: GameplayWorld>(
        mut mesh: Mesh,
        graph: &Graph,
        world: &W,
        tick_interval: f32,
        objectives: Option<&crate::ctf::World>,
    ) -> Result<Self, Error> {
        let mut spawns: [Vec<Spawn>; 2] = std::array::from_fn(|_| Vec::new());
        let mut rooms = Vec::new();
        for entity in &graph.entities {
            if classname(entity, b"info_player_teamspawn") {
                if scalar(entity, b"StartDisabled") == Some(b"1".as_slice()) {
                    continue;
                }
                let team = match scalar(entity, b"TeamNum") {
                    Some(b"2") => PlayerTeam::Red,
                    Some(b"3") => PlayerTeam::Blue,
                    _ => continue,
                };
                let position = vector(entity, b"origin").ok_or(Error::InvalidEntity)?;
                let angles = vector(entity, b"angles").unwrap_or([0.0; 3]);
                spawns[team_index(team)].push(Spawn {
                    identity: entity.index as u32,
                    class_flags: scalar(entity, b"spawnflags").and_then(|v| std::str::from_utf8(v).ok()).and_then(|v| v.parse().ok()).unwrap_or(0),
                    position,
                    angles,
                });
            } else if classname(entity, b"func_respawnroom")
                && scalar(entity, b"StartDisabled") != Some(b"1".as_slice())
            {
                let team = match scalar(entity, b"TeamNum") {
                    Some(b"2") => PlayerTeam::Red,
                    Some(b"3") => PlayerTeam::Blue,
                    _ => continue,
                };
                if let Some(model) = entity.bsp_model_index {
                    rooms.push((team, model, vector(entity, b"origin").unwrap_or([0.0; 3])));
                }
            }
        }
        for (team, model, origin) in rooms {
            for area in &mut mesh.areas {
                let points = [
                    area.center(),
                    area.northwest,
                    [area.southeast[0], area.northwest[1], area.northeast_z],
                    [area.northwest[0], area.southeast[1], area.southwest_z],
                    area.southeast,
                ];
                for mut point in points {
                    point[2] += STEP_HEIGHT;
                    if world
                        .overlaps_model_hull(
                            model,
                            origin,
                            point,
                            Hull {
                                mins: [0.0; 3],
                                maxs: [0.0; 3],
                            },
                        )
                        .map_err(Error::Movement)?
                    {
                        area.game_attributes |= match team {
                            PlayerTeam::Red => TF_NAV_SPAWN_ROOM_RED,
                            PlayerTeam::Blue => TF_NAV_SPAWN_ROOM_BLUE,
                            PlayerTeam::Unassigned | PlayerTeam::Spectator => unreachable!(),
                        };
                        break;
                    }
                }
            }
        }
        let scenario = scenario(graph, objectives)?;
        let point_navigation = control_point::Navigation::default();
        Ok(Self {
            mesh: Arc::new(mesh),
            spawns,
            scenario,
            point_navigation,
            point_spawn_revision: None,
            last_spawn: [None; 2],
            round_winner: None,
            navigation_recompute_at: None,
            had_bots: false,
            bots: BTreeMap::new(),
            scheduler: locomotion::Scheduler::default(),
            next_identity: crate::PLAYER_IDENTITY + 1,
            next_name: None,
            tick_interval,
            configuration: None,
            next_quota_think: 0.0,
            path_scratch: PathScratch::default(),
        })
    }

    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    pub fn len(&self) -> usize {
        self.bots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bots.is_empty()
    }

    pub(crate) fn roster(&self) -> impl ExactSizeIterator<Item = crate::team_selection::RosterPlayer> + '_ {
        self.bots.values().map(|bot| crate::team_selection::RosterPlayer {
            identity: bot.identity,
            team: bot.team,
        })
    }

    pub fn configuration(&self) -> Option<Configuration> {
        self.configuration
    }

    pub fn configure(&mut self, configuration: Configuration) -> Result<(), Error> {
        if configuration.quota > MAX_BOTS as u8
            || configuration.maximum_players == 0
            || configuration.maximum_players > MAX_BOTS as u8 + 1
        {
            return Err(Error::Limit);
        }
        self.configuration = Some(configuration);
        self.next_quota_think = 0.0;
        Ok(())
    }

    pub fn forced_change(&mut self, added: usize, removed: usize, tick: u64) {
        let Some(configuration) = &mut self.configuration else {
            return;
        };
        configuration.quota = configuration
            .quota
            .saturating_add(added.min(MAX_BOTS) as u8)
            .saturating_sub(removed.min(MAX_BOTS) as u8)
            .min(MAX_BOTS as u8);
        self.next_quota_think =
            tick as f32 * self.tick_interval + if removed == 0 { 1.0 } else { 2.0 };
    }

    pub fn maintain_quota(
        &mut self,
        tick: u64,
        human_team: PlayerTeam,
        human_class: PlayerClass,
        red_score: u16,
        blue_score: u16,
        random: &mut UniformRandomStream,
    ) -> Result<bool, Error> {
        crate::admission_metrics::begin_tick(tick);
        let Some(configuration) = self.configuration else {
            return Ok(false);
        };
        let now = tick as f32 * self.tick_interval;
        if now < self.next_quota_think {
            return Ok(false);
        }
        self.next_quota_think = now + QUOTA_THINK_INTERVAL;
        crate::admission_metrics::emit(crate::admission_metrics::QUOTA, 0);

        let human_on_team = usize::from(human_team.is_gameplay());
        let spectators = usize::from(human_team == PlayerTeam::Spectator);
        let mut desired = match configuration.mode {
            QuotaMode::Normal => usize::from(configuration.quota),
            QuotaMode::Fill => usize::from(configuration.quota).saturating_sub(human_on_team),
            QuotaMode::Match => usize::from(configuration.quota) * human_on_team,
        };
        if configuration.join_after_player && human_on_team == 0 && spectators == 0 {
            desired = 0;
        }
        desired = desired.min(
            usize::from(configuration.maximum_players)
                .saturating_sub(1)
                .saturating_sub(usize::from(configuration.auto_vacate)),
        );

        if desired > self.bots.len() {
            self.apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: None,
                    team: None,
                    difficulty: configuration.difficulty,
                },
                human_team,
                human_class,
                random,
            )?;
            return Ok(true);
        }
        if desired >= self.bots.len() {
            return Ok(false);
        }

        let red = self
            .bots
            .values()
            .filter(|bot| bot.team == PlayerTeam::Red)
            .count()
            + usize::from(human_team == PlayerTeam::Red);
        let blue = self
            .bots
            .values()
            .filter(|bot| bot.team == PlayerTeam::Blue)
            .count()
            + usize::from(human_team == PlayerTeam::Blue);
        let team = if blue > red {
            PlayerTeam::Blue
        } else if red > blue {
            PlayerTeam::Red
        } else if blue_score > red_score {
            PlayerTeam::Blue
        } else if red_score > blue_score {
            PlayerTeam::Red
        } else if random.random_int(0, 1).map_err(|_| Error::Limit)? == 0 {
            PlayerTeam::Blue
        } else {
            PlayerTeam::Red
        };
        let candidate = |team| {
            self.bots
                .values()
                .filter(|bot| bot.team == team)
                .min_by_key(|bot| (bot.lifecycle == PlayerLifecycle::Active, bot.identity))
                .map(|bot| bot.identity)
        };
        let identity = candidate(team).or_else(|| {
            candidate(if team == PlayerTeam::Blue {
                PlayerTeam::Red
            } else {
                PlayerTeam::Blue
            })
        });
        if let Some(identity) = identity {
            self.bots.remove(&identity);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn round_respawn(
        &mut self,
        tick: u64,
        random: &mut crate::UniformRandomStream,
    ) -> Result<(), Error> {
        self.round_winner = None;
        for bot in self.bots.values_mut() {
            let candidates = &self.spawns[team_index(bot.team)];
            if candidates.is_empty() {
                return Err(Error::MissingSpawn(bot.team));
            }
            let choice = if matches!(self.scenario, Scenario::ControlPoints { .. }) {
                next_control_point_spawn(candidates, &mut self.last_spawn[team_index(bot.team)], bot.class).ok_or(Error::MissingSpawn(bot.team))?
            } else { random
                .random_int(
                    0,
                    i32::try_from(candidates.len() - 1).map_err(|_| Error::Limit)?,
                )
                .map_err(|_| Error::Limit)? as usize };
            respawn_bot(
                bot,
                candidates[choice],
                &self.mesh,
                tick,
                self.tick_interval,
            );
            self.scheduler.reset(bot.identity);
        }
        Ok(())
    }

    pub fn combat_targets(&self) -> impl Iterator<Item = CombatTarget> + '_ {
        self.bots
            .values()
            .filter(|bot| bot.lifecycle == PlayerLifecycle::Active)
            .map(|bot| CombatTarget {
                blast_jump_state: bot.blast_jump_state,
                crouched: bot.movement.crouch.uses_crouched_hull(),
                in_water: bot.in_water,
                hull: bot.movement.active_hull(MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve()),
                grounded: bot.movement.ground.is_some(),
                water_level: bot.movement.water_level,
                conditions: bot.conditions.words(),
                identity: bot.identity,
                class: bot.class,
                team: bot.team,
                position: bot.movement.position,
                velocity: bot.movement.velocity,
                burning: bot.afterburn.is_some(),
            })
    }

    pub fn control_point_actors(&self) -> impl Iterator<Item = crate::control_point::Actor> + '_ {
        self.bots.values().map(control_point_actor)
    }

    pub(crate) fn contact_actors(
        &self,
        winning_team: Option<PlayerTeam>,
    ) -> impl Iterator<Item = crate::map_runtime::ActorContact> + '_ {
        self.bots
            .values()
            .map(move |bot| crate::map_runtime::ActorContact {
                identity: bot.identity,
                position: bot.movement.position,
                hull: MovementPolicy {
                    class: bot.class,
                    modifiers: MovementModifiers::default(),
                }
                .resolve()
                .standing_hull,
                facts: crate::map_runtime::PlayerContactFacts {
                    team: bot.team.source_number(),
                    class: bot.class.source_number(),
                    observer: bot.lifecycle != PlayerLifecycle::Active,
                    conditions: [0; 5],
                    winning_team: winning_team.map(PlayerTeam::source_number),
                },
                alive: bot.lifecycle == PlayerLifecycle::Active,
            })
    }

    pub fn apply_damage(&mut self, identity: u32, damage: f32) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else {
            return false;
        };
        if bot.lifecycle != PlayerLifecycle::Active
            || bot.conditions.is_invulnerable()
            || damage <= 0.0
            || !damage.is_finite()
        {
            return false;
        }
        bot.health.current = bot
            .health
            .current
            .saturating_sub((damage + 0.5) as i32)
            .max(0);
        if bot.health.current == 0 {
            bot.lifecycle = PlayerLifecycle::Dying;
            bot.afterburn = None;
        }
        true
    }

    pub fn apply_flame_contact(
        &mut self,
        identity: u32,
        attacker: u32,
        source_weapon: Option<crate::weapon::WeaponSource>,
        now: f32,
        damage: f32,
    ) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else {
            return false;
        };
        if bot.lifecycle != PlayerLifecycle::Active
            || bot.conditions.is_invulnerable()
            || now - bot.last_flame_damage_time < crate::pyro::FLAME_BURN_FREQUENCY
        {
            return false;
        }
        bot.last_flame_damage_time = now;
        bot.health.current = bot
            .health
            .current
            .saturating_sub((damage + 0.5) as i32)
            .max(0);
        if bot.health.current == 0 {
            bot.lifecycle = PlayerLifecycle::Dying;
            bot.afterburn = None;
            bot.conditions.remove(ConditionId::BURNING, true);
            bot.conditions.remove(ConditionId::HEALING_DEBUFF, true);
        } else {
            let starting = bot.afterburn.is_none();
            bot.afterburn = Some(crate::pyro::Afterburn::ignite(
                bot.afterburn,
                bot.class,
                attacker,
                Weapon::Flamethrower,
                "flamethrower",
                source_weapon,
                now,
                crate::pyro::FLAME_INITIAL_AFTERBURN,
                crate::pyro::FLAME_AFTERBURN_PER_HIT,
            ));
            let _ = bot.conditions.add(ConditionId::BURNING, crate::condition::ConditionDuration::Permanent, Some(attacker), true, false);
            if starting {
                let _ = bot.conditions.add(ConditionId::HEALING_DEBUFF, crate::condition::ConditionDuration::Finite(crate::pyro::FLAME_INITIAL_AFTERBURN), Some(attacker), true, false);
            }
        }
        true
    }

    pub fn extinguish(&mut self, identity: u32) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else { return false; };
        let was_burning = bot.afterburn.take().is_some() || bot.conditions.contains(ConditionId::BURNING);
        bot.conditions.remove(ConditionId::BURNING, true);
        bot.conditions.remove(ConditionId::HEALING_DEBUFF, true);
        was_burning
    }

    pub fn burn_attacker(&self, identity: u32) -> Option<u32> {
        self.bots.get(&identity)?.afterburn.and_then(|burn| burn.original_attacker)
    }

    pub fn afterburn_damage(&mut self, now: f32) -> Vec<Damage> {
        let mut damage = Vec::new();
        for bot in self.bots.values_mut() {
            if bot.conditions.contains(ConditionId::HEALTH_BUFF) && let Some(burn) = bot.afterburn.as_mut() {
                burn.duration -= 2.0 * self.tick_interval;
            }
            if bot.lifecycle != PlayerLifecycle::Active || bot.health.current <= 0
                || bot.movement.water_level >= 2
                || bot.afterburn.is_some_and(|burn| burn.duration <= 0.0) {
                bot.afterburn = None;
                bot.conditions.remove(ConditionId::BURNING, true);
                bot.conditions.remove(ConditionId::HEALING_DEBUFF, true);
                continue;
            }
            if let Some(burn) = bot.afterburn.as_mut()
                && let Some(amount) = burn.advance(now) {
                damage.push(Damage {
                    source_weapon: burn.source_weapon,
                    damage_type: DamageType::BURN | DamageType::PREVENT_FORCE,
                    force: [0.0; 3], modifiers: DamageModifiers::default(), killing_weapon: Some(burn.killing_weapon),
                    attacker: burn.attacker, victim: bot.identity, weapon: burn.weapon,
                    amount, position: bot.movement.position,
                    crit: CritKind::None, range_multiplier: 1.0, custom: CustomDamage::Burning,
                });
            }
        }
        damage
    }

    pub fn ignite_projectile(&mut self, identity: u32, attacker: u32, weapon: Weapon, killing_weapon: &'static str, source_weapon: Option<crate::weapon::WeaponSource>, now: f32, initial_duration: f32, rate: f32) {
        if let Some(bot) = self.bots.get_mut(&identity)
            && bot.lifecycle == PlayerLifecycle::Active && !bot.conditions.is_invulnerable()
            && !bot.conditions.contains(ConditionId::PHASE) && !bot.conditions.contains(ConditionId::PASSTIME_INTERCEPTION) {
            let starting = bot.afterburn.is_none();
            bot.afterburn = Some(crate::pyro::Afterburn::ignite(bot.afterburn, bot.class,
                attacker, weapon, killing_weapon, source_weapon, now, initial_duration, rate));
            let _ = bot.conditions.add(ConditionId::BURNING, crate::condition::ConditionDuration::Permanent, Some(attacker), true, false);
            if starting && initial_duration > 0.0 {
                let _ = bot.conditions.add(ConditionId::HEALING_DEBUFF, crate::condition::ConditionDuration::Finite(initial_duration), Some(attacker), true, false);
            }
        }
    }

    pub fn actor_center(&self, identity: u32) -> Option<[f32; 3]> {
        let bot = self.bots.get(&identity)?;
        let hull = bot.movement.active_hull(MovementPolicy {
            class: bot.class, modifiers: MovementModifiers::default(),
        }.resolve());
        Some(std::array::from_fn(|axis| bot.movement.position[axis] + (hull.mins[axis] + hull.maxs[axis]) * 0.5))
    }

    pub fn apply_impulse(&mut self, identity: u32, impulse: [f32; 3]) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else {
            return false;
        };
        if bot.lifecycle != PlayerLifecycle::Active
            || !impulse.iter().all(|value| value.is_finite())
        {
            return false;
        }
        for (velocity, added) in bot.movement.velocity.iter_mut().zip(impulse) {
            *velocity += added;
        }
        bot.movement.ground = None;
        true
    }

    pub fn apply_damage_impulse(&mut self, identity: u32, impulse: [f32; 3], enemy_blast: bool) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else { return false; };
        if !matches!(bot.lifecycle, PlayerLifecycle::Active | PlayerLifecycle::Dying) || !impulse.iter().all(|value| value.is_finite()) { return false; }
        for (velocity, added) in bot.movement.velocity.iter_mut().zip(impulse) { *velocity += added; }
        bot.blast_since_movement |= enemy_blast;
        true
    }

    pub fn apply(
        &mut self,
        request: Request,
        human_team: PlayerTeam,
        human_class: PlayerClass,
        random: &mut UniformRandomStream,
    ) -> Result<(), Error> {
        match request.operation {
            Operation::KickAll => {
                self.bots.clear();
                return Ok(());
            }
            Operation::KickTeam(team) => {
                self.bots.retain(|_, bot| bot.team != team);
                return Ok(());
            }
            Operation::Add => {}
        }
        if request.count == 0 || self.bots.len() + usize::from(request.count) > MAX_BOTS {
            return Err(Error::Limit);
        }
        for _ in 0..request.count {
            crate::admission_metrics::emit(crate::admission_metrics::REQUEST, self.next_identity);
            let red = self
                .bots
                .values()
                .filter(|bot| bot.team == PlayerTeam::Red)
                .count()
                + usize::from(human_team == PlayerTeam::Red);
            let blue = self
                .bots
                .values()
                .filter(|bot| bot.team == PlayerTeam::Blue)
                .count()
                + usize::from(human_team == PlayerTeam::Blue);
            let team = request.team.unwrap_or(if red <= blue {
                PlayerTeam::Red
            } else {
                PlayerTeam::Blue
            });
            let candidates = &self.spawns[team_index(team)];
            if candidates.is_empty() {
                return Err(Error::MissingSpawn(team));
            }
            let class = request.class.or_else(|| preset_spawn_class(&self.bots, team, human_team, human_class)).ok_or(Error::InvalidEntity)?;
            let choice = if matches!(self.scenario, Scenario::ControlPoints { .. }) {
                next_control_point_spawn(candidates, &mut self.last_spawn[team_index(team)], class).ok_or(Error::MissingSpawn(team))?
            } else { random
                .random_int(
                    0,
                    i32::try_from(candidates.len() - 1).map_err(|_| Error::Limit)?,
                )
                .map_err(|_| Error::Limit)? as usize };
            let spawn = candidates[choice];
            let policy = MovementPolicy {
                class,
                modifiers: MovementModifiers::default(),
            }
            .resolve();
            let (objective, goal) = objective(self.scenario, team, None, None);
            let identity = self.next_identity;
            self.next_identity = self.next_identity.checked_add(1).ok_or(Error::Limit)?;
            let mut loadout = crate::default_loadout(class);
            let active_weapon = crate::default_weapon(class);
            if let Some(weapon) = active_weapon.and_then(|weapon| loadout.get_mut(&weapon)) {
                weapon.deploy(0, self.tick_interval);
            }
            crate::admission_metrics::emit(crate::admission_metrics::LOADOUT, identity);
            let name_index = match self.next_name {
                Some(index) => index,
                None => random
                    .random_int(
                        0,
                        i32::try_from(BOT_NAMES.len() - 1).map_err(|_| Error::Limit)?,
                    )
                    .map_err(|_| Error::Limit)? as usize,
            };
            let name = BOT_NAMES[name_index].to_owned();
            self.next_name = Some((name_index + 1) % BOT_NAMES.len());
            let current_area = self.mesh.nearest_area(spawn.position).map(|area| area.identity);
            crate::admission_metrics::emit(crate::admission_metrics::NAVIGATION, identity);
            self.bots.insert(
                identity,
                Bot {
                    equipment: None,
                    movement_stuns: crate::hitscan::MovementStuns::default(),
                    weapon_knockback: false,
                    scattergun_jumped: false,
                    blast_since_movement: false,
                    blast_jump_state: false,
                    in_water: false,
                    decapitations: 0,
                    damagers: crate::deathnotice::DamagerHistory::default(),
                    critical_history: crate::critical::PlayerHistory::default(),
                    identity,
                    name,
                    class,
                    team,
                    lifecycle: PlayerLifecycle::Active,
                    difficulty: request.difficulty,
                    movement: State::from_player(
                        Player {
                            position: spawn.position,
                            velocity: [0.0; 3],
                            grounded: false,
                            crouched: false,
                            jump_latched: false,
                        },
                        policy,
                    ),
                    yaw_degrees: spawn.angles[1],
                    pitch_degrees: 0.0,
                    health: HealthState::spawn(class, 0.0, 0.0)
                        .map_err(|_| Error::InvalidEntity)?,
                    conditions: ConditionState::default(),
                    spy: (class == PlayerClass::Spy).then(crate::spy::SpyState::default),
                    afterburn: None,
                    last_flame_damage_time: f32::NEG_INFINITY,
                    ammo: class.data().maximum_ammo,
                    next_regenerate_tick: 0,
                    objective,
                    target: None,
                    known_since: BTreeMap::new(),
                    next_target_tick: 0,
                    next_repath_tick: 0,
                    current_area,
                    path: Vec::new(),
                    path_index: 0,
                    crossing: None,
                    path_crossings: Arc::default(),
                    nav_area_mark: None,
                    avoid_at: 0.0,
                    locomotion_command: MoveCommand::default(),
                    stuck: locomotion::Monitor::default(),
                    goal,
                    point_action: control_point::Action::default(),
                    loadout,
                    active_weapon,
                    last_fire_tick: None,
                    pending_melee: None,
                    respawn_tick: None,
                    death_tick: None,
                    carrying_flag: None,
                    shots: 0,
                    hits: 0,
                    kills: 0,
                    assists: 0,
                    deaths: 0,
                    captures: 0,
                    defenses: 0,
                    damage_dealt: 0,
                    killstreak: 0,
                },
            );
            crate::admission_metrics::emit(crate::admission_metrics::CONSTRUCTED, identity);
        }
        Ok(())
    }

    pub fn advance<W: GameplayWorld>(
        &mut self,
        world: &W,
        tick: u64,
        human: Human,
        supplies: &[SupplyTarget],
        random: &mut UniformRandomStream,
        objectives: Option<Objectives<'_>>,
    ) -> Result<Vec<Attack>, Error> {
        if self.bots.is_empty() {
            self.had_bots = false;
            return Ok(Vec::new());
        }
        let now = tick as f32 * self.tick_interval;
        if let Some(points) = objectives.and_then(|frame| frame.points) {
            if !self.had_bots { self.navigation_recompute_at = Some(now + 2.0); }
            if self.navigation_recompute_at.is_some_and(|deadline| now >= deadline) {
                self.point_navigation.recompute(&self.mesh, points, &self.spawns)?;
                self.navigation_recompute_at = None;
            }
        }
        self.had_bots = true;
        let actors = self.actors(human, tick);
        let mut attacks = Vec::new();
        let mut supply_cache = SupplyCache::default();
        let mut activities = Vec::<ActivityEvent>::new();
        let mut ammo = Vec::<AmmoEvent>::new();
        self.scheduler.prepare(tick,self.tick_interval,self.bots.values().map(|bot|(bot.identity,bot.lifecycle==PlayerLifecycle::Active)));
        let mesh = &self.mesh;
        let scenario = &mut self.scenario;
        let spawns = &self.spawns;
        for bot in self.bots.values_mut() {
            if bot.lifecycle != PlayerLifecycle::Active {
                if objectives.is_some_and(|frame| bot.death_tick.is_some_and(|death| frame.rules.player_can_respawn(bot.team, death as f32 * self.tick_interval))) {
                    let candidates = &spawns[team_index(bot.team)];
                    let choice = if matches!(scenario, Scenario::ControlPoints { .. }) {
                        next_control_point_spawn(candidates, &mut self.last_spawn[team_index(bot.team)], bot.class).ok_or(Error::MissingSpawn(bot.team))?
                    } else { random
                        .random_int(
                            0,
                            i32::try_from(candidates.len() - 1).map_err(|_| Error::Limit)?,
                        )
                        .map_err(|_| Error::Limit)? as usize };
                    respawn_bot(bot, candidates[choice], mesh, tick, self.tick_interval);
                    self.scheduler.reset(bot.identity);
                }
                continue;
            }
            let think_due=self.scheduler.begin(bot.identity,tick);
            let think_started=if think_due{world.bot_update_milliseconds()/1000.0}else{0.0};
            if !think_started.is_finite()||think_started<0.0{return Err(Error::InvalidWorkClock);}
            let maintenance_due = think_due&&tick >= bot.next_target_tick;
            if maintenance_due {
                bot.next_target_tick = tick + ticks(TARGET_SELECTION_INTERVAL, self.tick_interval);
                let mut visible = [None; MAX_BOTS + 1];
                let mut visible_count = 0;
                for actor in actors.enemies(bot.team) {
                    if visible_actor(world, bot, actor) {
                        visible[visible_count] = Some(actor);
                        visible_count += 1;
                    }
                }
                let visible = &visible[..visible_count];
                bot.known_since.retain(|identity, _| {
                    visible
                        .iter()
                        .flatten()
                        .any(|actor| actor.identity == *identity)
                });
                for actor in visible.iter().flatten() {
                    bot.known_since.entry(actor.identity).or_insert(tick);
                }
                bot.target = visible
                    .iter()
                    .flatten()
                    .copied()
                    .filter(|actor| {
                        tick.saturating_sub(bot.known_since[&actor.identity])
                            >= ticks(bot.difficulty.recognition_seconds(), self.tick_interval)
                    })
                    .min_by(|left, right| threat_order(bot, *left, *right))
                    .map(|actor| actor.identity);
            }
            let threat = bot
                .target
                .and_then(|identity| actors.get(identity).filter(|actor| actor.alive));
            let mut policy = bot_movement_policy(bot);
            if let Some(winner) = self.round_winner.filter(|team| team.is_gameplay()) { policy.maximum_speed *= if winner == bot.team { 1.1 } else { 0.9 }; }
            if think_due {
            let stuck_event=bot.stuck.update(now,bot.movement.position,policy.maximum_speed);
            let stuck_left=stuck_event.then(||random.random_int(0,100).expect("bounded stuck side")<50);
            if stuck_event&&bot.objective==ObjectiveKind::CapturePoint{bot.next_repath_tick=0;bot.stuck.clear(now,bot.movement.position);}
            select_weapon(bot, threat, tick, self.tick_interval);
            let authoritative =
                objectives.and_then(|o| o.flags).and_then(|world| world.bot_objective(bot.identity, bot.team));
            if let Some(flag) = authoritative {
                bot.carrying_flag = (flag.carrier == Some(bot.identity)).then_some(
                    if bot.team == PlayerTeam::Red {
                        PlayerTeam::Blue
                    } else {
                        PlayerTeam::Red
                    },
                );
            }
            sync_bot_ammo(bot);
            let health_ratio = bot.health.current as f32 / bot.class.data().maximum_health as f32;
            let health_needed = bot.afterburn.is_some()
                || health_ratio
                    < if threat.is_some() || bot.class == PlayerClass::Sniper {
                        HEALTH_CRITICAL_RATIO
                    } else {
                        HEALTH_OK_RATIO
                    };
            let ammo_needed = bot.active_weapon.is_some_and(|weapon| {
                !is_melee(weapon)
                    && (bot.ammo.primary as f32 / bot.class.data().maximum_ammo.primary as f32)
                        < LOW_AMMO_RATIO
            });
            let selected_supply = if health_needed {
                if !maintenance_due
                    && bot.objective == ObjectiveKind::GetHealth
                    && supplies.iter().any(|supply| supply.position == bot.goal)
                {
                    Some((ObjectiveKind::GetHealth, bot.goal))
                } else {
                    select_supply(
                        mesh,
                        bot,
                        &actors,
                        supplies,
                        crate::pickup::MapPickupKind::Health,
                        &mut supply_cache,
                        &mut self.path_scratch,
                    )
                    .map(|target| (ObjectiveKind::GetHealth, target.position))
                }
            } else if ammo_needed {
                if !maintenance_due
                    && bot.objective == ObjectiveKind::GetAmmo
                    && supplies.iter().any(|supply| supply.position == bot.goal)
                {
                    Some((ObjectiveKind::GetAmmo, bot.goal))
                } else {
                    select_supply(
                        mesh,
                        bot,
                        &actors,
                        supplies,
                        crate::pickup::MapPickupKind::Ammo,
                        &mut supply_cache,
                        &mut self.path_scratch,
                    )
                    .map(|target| (ObjectiveKind::GetAmmo, target.position))
                }
            } else {
                None
            };
            path::update_last_known_area(mesh, world, bot).map_err(Error::Movement)?;
            let (objective_kind, goal) = if let Some(supply) = selected_supply {
                supply
            } else if let Some(frame) = objectives.filter(|o| o.points.is_some()) {
                control_point::goal(bot, frame, &self.point_navigation, mesh, self.tick_interval, now, tick, threat, random)?
            } else if let Some(flag) = authoritative {
                if flag.carrier == Some(bot.identity) {
                    (
                        ObjectiveKind::DeliverFlag,
                        flag.capture_position.ok_or(Error::MissingScenario)?,
                    )
                } else if flag.carrier.is_some() {
                    (
                        ObjectiveKind::Attack,
                        threat.map_or(flag.position, |actor| actor.position),
                    )
                } else {
                    (ObjectiveKind::FetchFlag, flag.position)
                }
            } else {
                objective(
                    *scenario,
                    bot.team,
                    threat.map(|actor| actor.position),
                    bot.carrying_flag,
                )
            };
            if objective_kind != bot.objective {
                bot.next_repath_tick = 0;
            }
            bot.objective = objective_kind;
            bot.goal = goal;
            if tick >= bot.next_repath_tick || bot.path.is_empty() {
                let start = bot.current_area;
                let goal = mesh.nearest_area(bot.goal).map(|area| area.identity);
                if let (Some(start), Some(goal)) = (start, goal) {
                    bot.path = mesh
                        .build_path_with_scratch(
                            start,
                            goal,
                            &mut self.path_scratch,
                            |from, destination, direction, length| {
                                path_cost(
                                    from,
                                    destination,
                                    direction,
                                    length,
                                    PathContext {
                                        combat_intensity: self.point_navigation.combat_intensity(destination.identity, now),
                                        team: bot.team,
                                        bot_identity: bot.identity,
                                        now: tick as f32 * self.tick_interval,
                                        route: if frame_has_points(objectives) { bot.point_action.route.unwrap_or(Route::Default) } else if matches!(
                                            bot.objective,
                                            ObjectiveKind::DeliverFlag
                                                | ObjectiveKind::GetHealth
                                                | ObjectiveKind::GetAmmo
                                        ) {
                                            Route::Fastest
                                        } else {
                                            Route::Default
                                        },
                                    },
                                )
                            },
                        )
                        .unwrap_or_default();
                    bot.path_index = 0;
                    bot.path_crossings = Arc::new(path::compute(world, mesh, &bot.path, bot.movement.position, bot.class).map_err(Error::Movement)?);
                    bot.crossing = None;
                    bot.avoid_at = 0.0;
                }
                let (minimum, maximum) = match bot.objective {
                    ObjectiveKind::PayloadPush => (0.2, 0.4),
                    ObjectiveKind::PayloadGuard => (0.5, 1.0),
                    ObjectiveKind::FetchFlag | ObjectiveKind::DeliverFlag => (1.0, 2.0),
                    ObjectiveKind::Attack | ObjectiveKind::GetHealth | ObjectiveKind::GetAmmo => {
                        (0.3, 0.5)
                    }
                    ObjectiveKind::CapturePoint if bot.point_action.on_point => (0.5, 1.0),
                    ObjectiveKind::CapturePoint | ObjectiveKind::DefendPoint => (2.0, 3.0),
                    ObjectiveKind::BlockCapture => (0.5, 1.0),
                };
                bot.next_repath_tick =
                    tick + ticks(random.random_float(minimum, maximum), self.tick_interval);
            }
            while bot.path_index + 1 < bot.path.len() {
                let crossing = &bot.path_crossings[bot.path_index];
                let reached = crossing.reached(bot.movement.position);
                if reached
                {
                    bot.path_index += 1;
                    bot.crossing = None;
                } else {
                    break;
                }
            }
            let waypoint = if bot.path_index + 1 < bot.path.len() {
                let crossing = &bot.path_crossings[bot.path_index];
                bot.crossing = Some(crossing.clone());
                crossing.drop_position.unwrap_or(crossing.position)
            } else {
                bot.crossing = None;
                bot.goal
            };
            let jump_height = waypoint[2] - bot.movement.position[2];
            let waypoint = path::avoid(world, mesh, bot, waypoint, now).map_err(Error::Movement)?;
            let delta = crate::sub(waypoint, bot.movement.position);
            let planar = delta[0].hypot(delta[1]);
            if let Some(threat) = threat {
                let toward = crate::sub(aim_point(world, bot, threat), bot_eye(bot));
                bot.yaw_degrees = toward[1].atan2(toward[0]).to_degrees();
                bot.pitch_degrees = (-toward[2]).atan2(toward[0].hypot(toward[1])).to_degrees();
            } else if planar > 0.0 && bot.movement.ground.is_some() {
                // PathFollower only calls FaceTowards on the ground. Retaining
                // the facing while airborne preserves a stuck-recovery jump.
                bot.yaw_degrees = delta[1].atan2(delta[0]).to_degrees();
                bot.pitch_degrees = 0.0;
            }
            let should_move = planar > 5.0 && !(matches!(scenario, Scenario::ControlPoints { .. }) && objectives.is_some_and(|o| o.in_setup));
            if should_move{bot.stuck.request_move(now);}
            let move_yaw = if planar > 0.0 {
                delta[1].atan2(delta[0])
            } else {
                bot.yaw_degrees.to_radians()
            };
            let relative = move_yaw - bot.yaw_degrees.to_radians();
            let (forward,side)=locomotion::approach(relative,should_move,stuck_left,policy.maximum_speed);
            bot.locomotion_command=MoveCommand{forward,side,yaw_degrees:bot.yaw_degrees,jump:stuck_event||bot.crossing.as_ref().is_none_or(|crossing|crossing.drop_position.is_none())&&jump_height>=STEP_HEIGHT&&jump_height<MAX_JUMP_HEIGHT,crouch:false};
            if !self.scheduler.finish(world.bot_update_milliseconds()/1000.0-think_started){return Err(Error::InvalidWorkClock);}
            }
            let mut command=bot.locomotion_command;
            (command.forward,command.side)=bot.movement_stuns.command(now,command.forward,command.side);
            let movement = step(
                world,
                bot.movement,
                StepInput {
                    command_number: u32::try_from(tick).unwrap_or(u32::MAX),
                    command,
                    pitch_degrees: bot.pitch_degrees,
                    up: 0.0,
                    speed_button: false,
                    mode_request: None,
                },
                MovementConfiguration {
                    tick_interval: self.tick_interval,
                    water_exit_forward: 30.0,
                    water_exit_up_speed: 300.0,
                    ..MovementConfiguration::default()
                },
                policy,
            )
            .map_err(Error::Movement)?;
            bot.movement = movement.state;
            if bot.blast_since_movement && movement.ground_detach_by_upward_speed {
                bot.blast_jump_state = true;
                bot.conditions.add(ConditionId::new(81).unwrap(), crate::condition::ConditionDuration::Permanent, None, true, false).unwrap();
            }
            bot.blast_since_movement = false;
            if bot.movement.ground.is_some() {
                bot.weapon_knockback = false;
                bot.scattergun_jumped = false;
                bot.blast_jump_state = false;
                bot.conditions.remove(ConditionId::KNOCKED_INTO_AIR, false);
                bot.conditions.remove(ConditionId::BLAST_JUMPING, false);
            }

            if let Some(attack) = take_melee_smack(bot, tick) { attacks.push(attack); }
            let Some(active_weapon) = bot.active_weapon else {
                continue;
            };
            let in_range = self.round_winner.is_none_or(|winner| winner == bot.team) && bot.spy.is_none_or(|spy| spy.can_attack(now))
                && threat.is_some_and(|target| {
                    let range = distance(bot.movement.position, target.position);
                    range < max_attack_range(active_weapon)
                        && (!is_melee(active_weapon) || range < MELEE_FIRE_RANGE)
                        && line_of_fire_clear(world, bot_eye(bot), target)
                });
            activities.clear();
            ammo.clear();
            let eye_position = bot_eye(bot);
            let state = bot
                .loadout
                .get_mut(&active_weapon)
                .ok_or(Error::InvalidEntity)?;
            let previous_minigun = state.minigun_state;
            let primary = state.primary(tick, self.tick_interval, in_range, false, &mut activities);
            if active_weapon == Weapon::Minigun {
                apply_minigun_aiming(&mut bot.conditions, previous_minigun, state.minigun_state)?;
            }
            if matches!(primary, PrimaryResult::Fired { .. }) {
                bot.last_fire_tick = Some(tick);
                bot.shots = bot.shots.saturating_add(1);
                if is_melee(active_weapon) {
                    bot.pending_melee = Some((
                        tick + (ballistics::MELEE_SMACK_DELAY / self.tick_interval).floor() as u64,
                        threat.unwrap().identity,
                        active_weapon,
                    ));
                    attacks.push(Attack { phase: AttackPhase::MeleeSwing, attacker: bot.identity, team: bot.team,
                        weapon: active_weapon, target: threat.unwrap().identity, position: bot.movement.position,
                        eye_position, pitch_degrees: bot.pitch_degrees, yaw_degrees: bot.yaw_degrees });
                } else {
                    attacks.push(Attack {
                        phase: AttackPhase::Fire,
                        attacker: bot.identity,
                        team: bot.team,
                        weapon: active_weapon,
                        target: threat.unwrap().identity,
                        position: bot.movement.position,
                        eye_position,
                        pitch_degrees: bot.pitch_degrees,
                        yaw_degrees: bot.yaw_degrees,
                    });
                }
            }
            if active_weapon != Weapon::Minigun && (!in_range || state.clip == 0) {
                state.start_reload(tick, self.tick_interval, &mut activities);
            }
            state.advance_reload(tick, self.tick_interval, &mut activities, &mut ammo);
        }
        Ok(attacks)
    }

    pub fn due_melee_attacks(&mut self, tick: u64) -> Vec<Attack> {
        self.bots.values_mut().filter_map(|bot| take_melee_smack(bot, tick)).collect()
    }

    pub fn patient(&self, identity: u32, origin: [f32; 3]) -> Option<PatientFacts> {
        let bot = self.bots.get(&identity)?;
        let position = bot.movement.position;
        let nearest_point = [
            origin[0].clamp(position[0] - 24.0, position[0] + 24.0),
            origin[1].clamp(position[1] - 24.0, position[1] + 24.0),
            origin[2].clamp(position[2], position[2] + 82.0),
        ];
        Some(PatientFacts {
            identity,
            team: bot.team,
            alive: bot.lifecycle == PlayerLifecycle::Active && bot.health.current > 0,
            stealthed: bot.conditions.contains(ConditionId::STEALTHED),
            disguised_as: None,
            blocks_healing: bot.conditions.contains(ConditionId::NO_HEALING_DAMAGE_BUFF),
            nearest_point,
            center: [position[0], position[1], position[2] + 41.0],
            eyes: [
                position[0],
                position[1],
                position[2] + bot.class.standing_eye_height(),
            ],
            current_health: bot.health.current,
            maximum_health: bot.health.maximum,
            maximum_buffed_health: bot
                .health
                .max_buffed_health(HealthConfiguration::default(), false, false)
                .ok()?,
            healer_count: bot.health.healers.len().max(1),
        })
    }

    pub fn patient_identities(&self) -> impl Iterator<Item = u32> + '_ {
        self.bots.keys().copied()
    }

    pub fn patient_state(
        &mut self,
        identity: u32,
    ) -> Option<(&mut HealthState, &mut ConditionState)> {
        let bot = self.bots.get_mut(&identity)?;
        Some((&mut bot.health, &mut bot.conditions))
    }

    pub fn has_condition(&self, identity: u32, condition: ConditionId) -> bool {
        self.bots.get(&identity).is_some_and(|bot| bot.conditions.contains(condition))
    }

    pub(crate) fn melee_actor(&self, identity: u32) -> Option<crate::melee::Actor> {
        let bot = self.bots.get(&identity)?;
        if bot.lifecycle != PlayerLifecycle::Active || bot.health.current <= 0 { return None; }
        let hull = bot.movement.active_hull(MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve());
        Some(crate::melee::Actor { class: bot.class, team: bot.team, position: bot.movement.position, eye: bot_eye(bot),
            center: crate::add(bot.movement.position, crate::scale(crate::add(hull.mins, hull.maxs), 0.5)),
            hull, health: bot.health.current, maximum_health: bot.health.maximum })
    }

    /// MASK_SOLID melee clips player collision hulls, including teammates.
    pub fn melee_trace(&self, owner: u32, start: [f32; 3], end: [f32; 3], hull: Hull, maximum_fraction: f32) -> Option<(u32, f32, [f32; 3])> {
        self.bots.values().filter(|bot| bot.identity != owner && bot.lifecycle == PlayerLifecycle::Active && bot.health.current > 0)
            .filter_map(|bot| {
                let bounds = bot.movement.active_hull(MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve());
                let mins = std::array::from_fn(|axis| bot.movement.position[axis] + bounds.mins[axis] - hull.maxs[axis]);
                let maxs = std::array::from_fn(|axis| bot.movement.position[axis] + bounds.maxs[axis] - hull.mins[axis]);
                let fraction = crate::ray_box_fraction(start, end, mins, maxs)?;
                (fraction <= maximum_fraction).then_some((bot.identity, fraction, crate::add(start, crate::scale(crate::sub(end, start), fraction))))
            }).min_by(|left, right| left.1.total_cmp(&right.1).then_with(|| left.0.cmp(&right.0)))
    }

    pub fn advance_health(&mut self, now: f32) -> Result<(), Error> {
        for bot in self.bots.values_mut() {
            if bot.lifecycle == PlayerLifecycle::Active {
                let (healing, active_penalty, received) = if let Some(equipment) = &mut bot.equipment {
                    equipment.providers.set_active(bot.active_weapon);
                    let healing = equipment.providers.player("mult_health_fromhealers", 1.0);
                    let penalty = bot.active_weapon.map_or(1.0, |weapon| equipment.providers.weapon(weapon, "mult_health_fromhealers_penalty_active", 1.0));
                    let received = bot.active_weapon.map_or(1.0, |weapon| equipment.providers.weapon(weapon, "mult_healing_received", 1.0));
                    (healing, penalty, received)
                } else { (1.0, 1.0, 1.0) };
                bot.health
                    .advance(
                        now,
                        self.tick_interval,
                        HealthConfiguration::default(),
                        healing,
                        active_penalty,
                        received,
                        &mut bot.conditions,
                    )
                    .map_err(|_| Error::InvalidEntity)?;
            }
        }
        Ok(())
    }

    pub fn intersect_enemy(
        &self,
        attacker_team: PlayerTeam,
        start: [f32; 3],
        end: [f32; 3],
        excluded: u32,
        eligible: impl Fn(u32) -> bool,
    ) -> Option<(u32, f32, [f32; 3])> {
        self.bots
            .values()
            .filter(|bot| {
                bot.identity != excluded
                    && eligible(bot.identity)
                    && bot.lifecycle == PlayerLifecycle::Active
                    && attacker_team.is_enemy(bot.team)
            })
            .filter_map(|bot| {
                segment_player(start, end, bot.movement.position).map(|fraction| {
                    (
                        bot.identity,
                        fraction,
                        crate::add(start, crate::scale(crate::sub(end, start), fraction)),
                    )
                })
            })
            .min_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| left.0.cmp(&right.0))
            })
    }

    pub fn intersect_players_hull(&self, start: [f32; 3], end: [f32; 3], hull: Hull) -> Option<(u32, f32, PlayerTeam)> {
        self.bots.values().filter(|bot| bot.lifecycle == PlayerLifecycle::Active).filter_map(|bot| {
            let target = bot.movement.active_hull(MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve());
            let mins = crate::sub(crate::add(bot.movement.position, target.mins), hull.maxs);
            let maxs = crate::sub(crate::add(bot.movement.position, target.maxs), hull.mins);
            segment_bounds(start, end, mins, maxs).map(|fraction| (bot.identity, fraction, bot.team))
        }).min_by(|left, right| left.1.total_cmp(&right.1).then_with(|| left.0.cmp(&right.0)))
    }

    pub fn generic_push(&mut self, identity: u32, attacker: u32, impulse: [f32; 3], horizontal_scale: f32, vertical_scale: f32) -> Result<bool, Error> {
        if self.hitscan_target(identity).is_none_or(|target| target.push_immune) || self.bots[&identity].conditions.contains(ConditionId::RUNE_KNOCKOUT) { return Ok(false); }
        let bot = self.bots.get_mut(&identity).unwrap();
        let force = crate::combat::generic_push_impulse(impulse, bot.movement.ground.is_some(), horizontal_scale, vertical_scale);
        bot.conditions.add(ConditionId::KNOCKED_INTO_AIR, crate::condition::ConditionDuration::Permanent, Some(attacker), true, false).map_err(|_| Error::InvalidEntity)?;
        Ok(self.apply_impulse(identity, force))
    }

    pub(crate) fn grant_pickup(
        &mut self,
        identity: u32,
        definition: crate::pickup::MapPickupDefinition,
    ) -> Option<u16> {
        let bot = self.bots.get_mut(&identity)?;
        if bot.lifecycle != PlayerLifecycle::Active {
            return None;
        }
        match definition.kind {
            crate::pickup::MapPickupKind::Health => {
                let maximum = bot.health.maximum;
                if bot.health.current >= maximum && bot.afterburn.is_none() && !bot.conditions.contains(ConditionId::BLEEDING) && !bot.conditions.contains(ConditionId::BURNING) {
                    return None;
                }
                let (packs, received) = if let Some(equipment) = &mut bot.equipment {
                    equipment.providers.set_active(bot.active_weapon);
                    (equipment.providers.player("mult_health_frompacks", 1.0), bot.active_weapon.map_or(1.0, |weapon| equipment.providers.weapon(weapon, "mult_healing_received", 1.0)))
                } else { (1.0, 1.0) };
                let requested = (maximum as f32 * definition.size.ratio()).ceil() * packs;
                let gained = bot.health.take_health(requested, false, received, &mut bot.conditions).ok()?;
                bot.afterburn = None;
                bot.conditions.remove(ConditionId::BLEEDING, false);
                bot.conditions.remove(ConditionId::BURNING, false);
                Some(gained as u16)
            }
            crate::pickup::MapPickupKind::Ammo => {
                sync_bot_ammo(bot);
                let mut grants = Vec::new();
                crate::pickup::grant_map_ammo(
                    bot.class,
                    bot.class.data().maximum_ammo,
                    definition.size,
                    &mut bot.ammo,
                    &mut grants,
                );
                if grants.is_empty() {
                    return None;
                }
                for state in bot.loadout.values_mut() {
                    if let Some(kind) = crate::weapon_ammo_kind(state.weapon) {
                        state.reserve = bot.ammo.get(kind).min(state.profile().maximum_reserve);
                    }
                }
                Some(
                    grants
                        .iter()
                        .filter_map(|grant| match grant {
                            crate::pickup::PickupGrant::Ammo { amount, .. } => Some(*amount),
                            crate::pickup::PickupGrant::Health(_) => None,
                        })
                        .sum(),
                )
            }
        }
    }

    pub(crate) fn regenerate(&mut self, identity: u32, tick: u64) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else {
            return false;
        };
        if bot.lifecycle != PlayerLifecycle::Active || tick < bot.next_regenerate_tick {
            return false;
        }
        apply_bot_equipment(bot);
        bot.health.current = bot.health.maximum.max(bot.health.current);
        bot.afterburn = None;
        let energy = bot.conditions.contains(ConditionId::ENERGY_BUFF);
        for condition in crate::resupply_removed_conditions(energy) {
            bot.conditions.remove(ConditionId::new(condition as u8).unwrap(), false);
        }
        bot.ammo = bot_maximum_ammo(bot);
        for state in bot.loadout.values_mut() {
            state.regenerate(tick, self.tick_interval);
        }
        bot.next_regenerate_tick = tick + ticks(3.0, self.tick_interval);
        true
    }

    pub fn contains(&self, identity: u32) -> bool {
        self.bots.contains_key(&identity)
    }

    /// Weapon changes take effect at the next resupply/spawn. Cosmetic changes
    /// preserve the existing immediate local-bot preview contract.
    pub fn equip_item(&mut self, identity: u32, slot: crate::schema::LoadoutPosition, definition: Option<u32>) -> Result<bool, crate::equipment::EquipmentError> {
        let bot = self.bots.get_mut(&identity).ok_or(crate::equipment::EquipmentError::IneligibleSlot)?;
        let mut selected = bot.equipment.as_ref().map_or_else(crate::equipment::Equipment::default, |equipment| equipment.selected.clone());
        let changed = selected.equip(bot.class, slot, definition)?;
        if !changed { return Ok(false); }
        if bot.equipment.is_none() {
            let active = crate::equipment::Equipment::default().equipped_items(bot.class);
            let providers = crate::equipment::AttributeProviders::new(&active, bot.class);
            bot.equipment = Some(Box::new(BotEquipment { selected: selected.clone(), active, class: bot.class, providers,
                next_generation: bot.loadout.values().map(|runtime| runtime.generation).max().unwrap_or(0).checked_add(1).expect("bounded bot weapon generation") }));
        }
        let equipment = bot.equipment.as_mut().unwrap();
        equipment.selected = selected;
        if changed && matches!(slot, crate::schema::LoadoutPosition::Head | crate::schema::LoadoutPosition::Misc | crate::schema::LoadoutPosition::Misc2) {
            equipment.active.retain(|item| item.slot != slot);
            equipment.active.extend(equipment.selected.equipped_items(bot.class).into_iter().filter(|item| item.slot == slot));
            equipment.active.sort_by_key(|item| item.slot);
            equipment.providers = crate::equipment::AttributeProviders::new(&equipment.active, bot.class);
        }
        Ok(changed)
    }

    pub(crate) fn weapon_definition(&self, identity: u32, weapon: Weapon) -> Option<u32> {
        let bot = self.bots.get(&identity)?;
        if let Some(equipment) = &bot.equipment { return equipped_definition(&equipment.active, bot.class, weapon); }
        bot.class.data().stock_items.iter().find_map(|item| crate::equipment::registered_item(item.definition)
            .filter(|item| item.weapon_for_class(bot.class) == Some(weapon)).map(|item| item.definition_index))
    }

    pub(crate) fn equipped_weapon_attribute(&mut self, identity: u32, weapon: Weapon, hook: &str, input: f32) -> f32 {
        let Some(bot) = self.bots.get_mut(&identity) else { return input; };
        let Some(equipment) = &mut bot.equipment else { return input; };
        equipment.providers.set_active(bot.active_weapon);
        equipment.providers.weapon(weapon, hook, input)
    }

    pub(crate) fn equipped_player_attribute(&mut self, identity: u32, hook: &str, input: f32) -> f32 {
        let Some(bot) = self.bots.get_mut(&identity) else { return input; };
        bot_player_attribute(bot, hook, input)
    }

    pub fn teleport(
        &mut self,
        identity: u32,
        position: [f32; 3],
        pitch_degrees: f32,
        yaw_degrees: f32,
    ) -> Result<(), Error> {
        if position
            .into_iter()
            .chain([pitch_degrees, yaw_degrees])
            .any(|value| !value.is_finite())
        {
            return Err(Error::InvalidEntity);
        }
        let bot = self.bots.get_mut(&identity).ok_or(Error::InvalidEntity)?;
        bot.movement.position = position;
        bot.movement.ground = None;
        bot.pitch_degrees = pitch_degrees;
        bot.yaw_degrees = yaw_degrees;
        bot.current_area = self.mesh.nearest_area(position).map(|area| area.identity);
        bot.path.clear();
        bot.path_index = 0;
        bot.crossing = None;
        bot.path_crossings = Arc::default();
        bot.nav_area_mark = None;
        bot.avoid_at = 0.0;
        bot.locomotion_command=MoveCommand::default();bot.stuck=locomotion::Monitor::default();
        bot.next_repath_tick = 0;
        Ok(())
    }

    pub fn active(&self, identity: u32) -> bool {
        self.bots
            .get(&identity)
            .is_some_and(|bot| bot.lifecycle == PlayerLifecycle::Active)
    }

    pub fn team(&self, identity: u32) -> Option<PlayerTeam> {
        self.bots.get(&identity).map(|bot| bot.team)
    }

    pub fn class(&self, identity: u32) -> Option<PlayerClass> {
        self.bots.get(&identity).map(|bot| bot.class)
    }

    pub fn damage_assister(&self, victim: u32, scorer: u32, curtime: f32) -> Option<u32> {
        self.bots.get(&victim)?.damagers.assister(scorer, curtime)
    }

    pub fn damage(
        &mut self,
        input: Damage,
        attacker_team: PlayerTeam,
        tick: u64,
        attacker_conditions: ConditionState,
    ) -> Result<Option<damage::DamageResult>, Error> {
        let Some(victim) = self.bots.get_mut(&input.victim) else {
            return Ok(None);
        };
        let result = damage::apply_damage(
            victim.lifecycle == PlayerLifecycle::Active,
            &mut victim.health,
            &mut victim.conditions,
            &input.input(attacker_team, victim.team, attacker_conditions),
            input.modifiers,
        )
        .map_err(|_| Error::Damage)?;
        if !result.admitted {
            return Ok(Some(result));
        }
        if input.attacker != input.victim {
            victim.damagers.record(input.attacker, tick as f32 * self.tick_interval);
        }
        victim.health.current = victim.health.current.max(0);
        if let Some(spy) = victim.spy.as_mut() {
            spy.note_damage(
                tick as f32 * self.tick_interval,
                result.health_damage,
                victim.conditions.contains(ConditionId::BLEEDING),
            );
        }
        let killed = result.death.is_some();
        if killed {
            victim.lifecycle = PlayerLifecycle::Dying;
            victim.conditions.remove_all();
            victim.afterburn = None;
            victim.deaths = victim.deaths.saturating_add(1);
            victim.killstreak = 0;
            victim.death_tick = Some(tick);
            victim.respawn_tick = None;
            victim.target = None;
            victim.pending_melee = None;
            victim.carrying_flag = None;
        }
        if input.attacker != input.victim && input.attacker != crate::PLAYER_IDENTITY
            && let Some(attacker) = self.bots.get_mut(&input.attacker)
        {
            attacker.hits = attacker.hits.saturating_add(1);
            attacker.damage_dealt = attacker.damage_dealt.saturating_add(
                u32::try_from(result.health_damage.min(result.health_before)).unwrap_or(0),
            );
            if killed {
                attacker.kills = attacker.kills.saturating_add(1);
                attacker.killstreak = attacker.killstreak.saturating_add(1);
            }
        }
        Ok(Some(result))
    }

    pub fn health(&self, identity: u32) -> Option<i32> {
        self.bots.get(&identity).map(|bot| bot.health.current)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn navigation_diagnostics(&self) -> Vec<String> {
        self.bots.values().map(|bot| format!("bot={} area={:?} feet={:?} goal={:?} path={:?} crossing={:?} surfaces={:?}",bot.identity,bot.current_area,bot.movement.position,bot.goal,&bot.path[bot.path_index..bot.path.len().min(bot.path_index+4)],bot.crossing,bot.path[bot.path_index..bot.path.len().min(bot.path_index+2)].iter().filter_map(|id|self.mesh.area(*id)).map(|a|(a.identity,a.northwest,a.southeast,a.northeast_z,a.southwest_z,a.attributes,a.game_attributes,&a.connections)).collect::<Vec<_>>())).collect()
    }
    pub(crate) fn position(&self, identity: u32) -> Option<[f32; 3]> { self.bots.get(&identity).map(|bot| bot.movement.position) }
    pub(crate) fn take_health(&mut self, identity: u32, amount: f32, multiplier: f32) -> Result<i32, Error> {
        let Some(bot) = self.bots.get_mut(&identity) else { return Ok(0); };
        if bot.health.current <= 0 { return Ok(0); }
        bot.health.take_health(amount, false, multiplier, &bot.conditions).map_err(|_| Error::Damage)
    }

    pub(crate) fn apply_self_blast_impulse(&mut self, identity: u32, impulse: [f32; 3], blast_jumping: bool) {
        if let Some(bot) = self.bots.get_mut(&identity) {
            bot.movement.velocity = crate::add(bot.movement.velocity, impulse);
            if blast_jumping {
                bot.blast_jump_state = true;
                bot.conditions.add(ConditionId::new(81).unwrap(), crate::condition::ConditionDuration::Permanent, None, true, false).unwrap();
            }
        }
    }

    pub(crate) fn update_water_flags(&mut self) {
        for bot in self.bots.values_mut() {
            match bot.movement.water_level { 0 => bot.in_water = false, 1 | 2 => bot.in_water = true, _ => {} }
            if bot.movement.ground.is_some() || bot.movement.water_level != 0 {
                bot.blast_jump_state = false;
                bot.conditions.remove(ConditionId::new(81).unwrap(), true);
            }
        }
    }

    pub(crate) fn decapitations(&self, identity: u32) -> i32 {
        self.bots.get(&identity).map_or(0, |bot| bot.decapitations)
    }

    pub(crate) fn add_decapitations(&mut self, identity: u32, count: i32) {
        if let Some(bot) = self.bots.get_mut(&identity) { bot.decapitations = bot.decapitations.saturating_add(count); }
    }

    pub(crate) fn conditions(&self, identity: u32) -> Option<&ConditionState> {
        Some(&self.bots.get(&identity)?.conditions)
    }

    pub fn active_weapon(&self, identity: u32) -> Option<Weapon> {
        self.bots.get(&identity).and_then(|bot| bot.active_weapon)
    }

    pub fn advance_conditions(&mut self, now: f32) {
        for bot in self.bots.values_mut() {
            if bot.lifecycle != PlayerLifecycle::Active { bot.conditions.remove_all(); continue; }
            bot.conditions.advance(self.tick_interval, bot.health.healers.len()).expect("valid bot condition tick");
            if bot.movement_stuns.active() {
                if !bot.movement_stuns.think(now) { bot.conditions.remove(ConditionId::STUNNED, true); }
                bot.conditions.set_active_stun_flags(bot.movement_stuns.flags());
            }
        }
    }

    pub(crate) fn weapon_runtime(&self, identity: u32, weapon: Weapon) -> Option<&WeaponRuntime> {
        self.bots.get(&identity)?.loadout.get(&weapon)
    }

    pub(crate) fn active_miniguns(&self) -> impl Iterator<Item = u32> + '_ {
        self.bots.values().filter(|bot| bot.lifecycle == PlayerLifecycle::Active && bot.active_weapon == Some(Weapon::Minigun)).map(|bot| bot.identity)
    }

    pub(crate) fn weapon_runtime_mut(&mut self, identity: u32, weapon: Weapon) -> Option<&mut WeaponRuntime> {
        self.bots.get_mut(&identity)?.loadout.get_mut(&weapon)
    }

    pub fn weapons(&self, identity: u32) -> impl Iterator<Item = Weapon> + '_ {
        self.bots.get(&identity).into_iter().flat_map(|bot| bot.loadout.keys().copied())
    }

    pub(crate) fn critical_history_mut(&mut self, identity: u32) -> Option<&mut crate::critical::PlayerHistory> {
        Some(&mut self.bots.get_mut(&identity)?.critical_history)
    }

    pub(crate) fn reset_critical_round_statistics(&mut self) {
        for bot in self.bots.values_mut() { bot.critical_history.reset_round_statistics(); }
    }

    pub(crate) fn advance_critical_histories(&mut self, now: f32) -> Result<(), crate::damage::DamageError> {
        for bot in self.bots.values_mut() { bot.critical_history.advance(now)?; }
        Ok(())
    }

    pub fn select_spawn(
        &mut self,
        team: PlayerTeam,
        class: PlayerClass,
        random: &mut UniformRandomStream,
    ) -> Option<crate::PlayerSpawn> {
        let candidates = self.spawns.get(team_index(team))?;
        if matches!(self.scenario, Scenario::ControlPoints { .. }) {
            let index = next_control_point_spawn(candidates, &mut self.last_spawn[team_index(team)], class)?;
            return Some(crate::PlayerSpawn { position: candidates[index].position, angles: candidates[index].angles });
        }
        let maximum = i32::try_from(candidates.len().checked_sub(1)?).ok()?;
        let index = usize::try_from(random.random_int(0, maximum).ok()?).ok()?;
        let spawn = candidates.get(index)?;
        Some(crate::PlayerSpawn { position: spawn.position, angles: spawn.angles })
    }

    pub fn record_point_combat(&mut self, tick: u64, human: Option<(Weapon, [f32; 3])>) {
        if !matches!(self.scenario, Scenario::ControlPoints { .. }) { return; }
        let now = tick as f32 * self.tick_interval;
        if let Some((weapon, position)) = human { self.point_navigation.record_combat(&self.mesh, crate::PLAYER_IDENTITY, weapon, position, now); }
        for bot in self.bots.values() {
            if bot.last_fire_tick == Some(tick) { if let Some(weapon) = bot.active_weapon { self.point_navigation.record_combat(&self.mesh, bot.identity, weapon, bot.movement.position, now); } }
        }
    }

    pub fn player_conditions(&self, identity: u32) -> Option<&ConditionState> { self.bots.get(&identity).map(|bot| &bot.conditions) }
    pub fn set_round_winner(&mut self, winner: PlayerTeam, duration: f32) {
        self.round_winner = Some(winner);
        for bot in self.bots.values_mut() {
            if winner.is_gameplay() && bot.team == winner && bot.lifecycle == PlayerLifecycle::Active {
                bot.conditions.add(ConditionId::CRIT_BOOSTED_BONUS_TIME, crate::condition::ConditionDuration::Finite(duration), None, true, false).expect("bonus duration");
            }
        }
    }
    pub fn record_point_capture(&mut self, cappers: &[u32]) {
        for identity in cappers { if let Some(bot) = self.bots.get_mut(identity) { bot.captures = bot.captures.saturating_add(1); } }
    }
    pub fn record_point_defense(&mut self, identity: u32) {
        if let Some(bot) = self.bots.get_mut(&identity) { bot.defenses = bot.defenses.saturating_add(1); }
    }

    pub fn synchronize_respawn_times(&mut self, rules: &crate::round::Rules) {
        for bot in self.bots.values_mut() {
            if let Some(death) = bot.death_tick {
                bot.respawn_tick = rules.player_respawn_time(bot.team, death as f32 * self.tick_interval).map(|time| (time / self.tick_interval).ceil().max(0.0) as u64);
            }
        }
    }

    pub fn team_population(&self, team: PlayerTeam, human_team: PlayerTeam) -> usize {
        self.bots.values().filter(|bot| bot.team == team).count() + usize::from(human_team == team)
    }

    pub fn record_human_hit(&mut self, attacker: u32, amount: u32, killed: bool) {
        if let Some(bot) = self.bots.get_mut(&attacker) {
            bot.hits = bot.hits.saturating_add(1);
            bot.damage_dealt = bot.damage_dealt.saturating_add(amount);
            if killed {
                bot.kills = bot.kills.saturating_add(1);
                bot.killstreak = bot.killstreak.saturating_add(1);
            }
        }
    }

    pub fn record_assist(&mut self, player: u32) {
        if let Some(bot) = self.bots.get_mut(&player) { bot.assists = bot.assists.saturating_add(1); }
    }

    pub fn hitbox_poses(&self) -> Vec<crate::PlayerHitboxPose> {
        self.bots.values().filter(|bot| bot.lifecycle == PlayerLifecycle::Active).filter_map(|bot| {
            Some(crate::PlayerHitboxPose { identity: bot.identity, team: bot.team, class: bot.class,
                definition: self.weapon_definition(bot.identity, bot.active_weapon?)?, position: bot.movement.position,
                velocity: bot.movement.velocity, yaw_degrees: bot.yaw_degrees })
        }).collect()
    }

    pub fn hitscan_target(&self, identity: u32) -> Option<crate::hitscan::Target> {
        let bot = self.bots.get(&identity)?;
        if bot.lifecycle != PlayerLifecycle::Active { return None; }
        let hull = bot.movement.active_hull(MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve());
        let size = crate::sub(hull.maxs, hull.mins);
        let center = crate::add(bot.movement.position, crate::scale(crate::add(hull.mins, hull.maxs), 0.5));
        Some(crate::hitscan::Target { position: bot.movement.position, center, size,
            eye_forward: crate::angle_vectors(bot.pitch_degrees, bot.yaw_degrees, 0.0).0,
            in_air_due_to_explosion: bot.conditions.contains(ConditionId::ROCKETPACK) || bot.movement.ground.is_none() && bot.movement.water_level == 0 && bot.blast_jump_state,
            healed: bot.conditions.contains(ConditionId::HEALTH_BUFF),
            push_immune: bot.conditions.contains(ConditionId::MEGAHEAL) || bot.conditions.contains(ConditionId::PHASE),
            knocked_back: bot.weapon_knockback,
        })
    }

    pub fn stun_movement(&mut self, identity: u32, attacker: u32, now: f32, duration: f32, amount: f32, forward_only: bool) -> Result<bool, Error> {
        let Some(bot) = self.bots.get_mut(&identity) else { return Ok(false); };
        if bot.lifecycle != PlayerLifecycle::Active || [ConditionId::PHASE, ConditionId::MEGAHEAL, ConditionId::PASSTIME_INTERCEPTION,
            ConditionId::INVULNERABLE_HIDE_UNLESS_DAMAGED].into_iter().any(|condition| bot.conditions.contains(condition)) { return Ok(false); }
        bot.movement_stuns.add(now, duration, amount, forward_only);
        bot.conditions.add(ConditionId::STUNNED, crate::condition::ConditionDuration::Permanent, Some(attacker), true, false).map_err(|_| Error::InvalidEntity)?;
        bot.conditions.set_active_stun_flags(bot.movement_stuns.flags());
        Ok(true)
    }

    pub fn scattergun_push(&mut self, identity: u32, attacker: u32, now: f32, impulse: [f32; 3], horizontal_scale: f32, vertical_scale: f32) -> Result<bool, Error> {
        if self.hitscan_target(identity).is_none_or(|target| target.push_immune || target.knocked_back) { return Ok(false); }
        self.generic_push(identity, attacker, impulse, horizontal_scale, vertical_scale)?;
        self.stun_movement(identity, attacker, now, 0.3, 1.0, true)?;
        self.bots.get_mut(&identity).unwrap().weapon_knockback = true;
        Ok(true)
    }

    pub(crate) fn scattergun_jump(&mut self, identity: u32, forward: [f32; 3], now: f32) -> Result<(), Error> {
        let Some(bot) = self.bots.get_mut(&identity) else { return Ok(()); };
        if bot.movement.ground.is_some() || bot.scattergun_jumped { return Ok(()); }
        bot.scattergun_jumped = true;
        bot.movement.velocity = crate::hitscan::scattergun_jump(bot.movement.velocity, forward);
        self.stun_movement(identity, identity, now, 0.3, 1.0, true)?;
        Ok(())
    }

    pub(crate) fn remove_disguise(&mut self, identity: u32, now: f32) {
        if let Some(bot) = self.bots.get_mut(&identity) {
            if let Some(spy) = &mut bot.spy { spy.remove_disguise(now); }
            bot.conditions.remove(ConditionId::DISGUISED, false);
            bot.conditions.remove(ConditionId::DISGUISING, false);
        }
    }

    pub(crate) fn add_head(&mut self, identity: u32) {
        if let Some(bot) = self.bots.get_mut(&identity) { bot.decapitations = bot.decapitations.saturating_add(1); }
    }

    pub fn combat_player(&self, identity: u32) -> Option<crate::CombatPlayerFacts> {
        let bot = self.bots.get(&identity)?;
        if bot.lifecycle != PlayerLifecycle::Active {
            return None;
        }
        let yaw = bot.yaw_degrees.to_radians();
        Some(crate::CombatPlayerFacts {
            team: bot.team,
            health: bot.health.current,
            world_center: [
                bot.movement.position[0],
                bot.movement.position[1],
                bot.movement.position[2] + 41.0,
            ],
            eye_forward: [yaw.cos(), yaw.sin(), 0.0],
            backstab_immune: false,
        })
    }

    pub fn snapshots(&self) -> Vec<Snapshot> {
        self.bots
            .values()
            .map(|bot| Snapshot {
                weapon_definition: bot.active_weapon.and_then(|weapon| self.weapon_definition(bot.identity, weapon)),
                conditions: bot.conditions.words(),
                equipped_items: bot.equipment.as_ref().map_or_else(Vec::new, |equipment| equipment.active.iter().filter(|item|
                    !bot.class.data().stock_items.iter().any(|stock| stock.definition == item.definition_index && stock.slot == item.slot as u8))
                    .cloned().collect()),
                identity: bot.identity,
                spy: bot.spy,
                name: bot.name.clone(),
                class: bot.class,
                team: bot.team,
                lifecycle: bot.lifecycle,
                difficulty: bot.difficulty,
                objective: bot.objective,
                position: bot.movement.position,
                velocity: bot.movement.velocity,
                yaw_degrees: bot.yaw_degrees,
                pitch_degrees: bot.pitch_degrees,
                health: bot.health.current,
                maximum_health: bot.health.maximum,
                target: bot.target,
                area: bot.current_area,
                remaining_path_areas: bot.path.len().saturating_sub(bot.path_index) as u32,
                weapon: bot
                    .active_weapon
                    .and_then(|weapon| bot.loadout.get(&weapon).copied())
                    .map(WeaponState::from_runtime),
                shots: bot.shots,
                hits: bot.hits,
                kills: bot.kills,
                assists: bot.assists,
                deaths: bot.deaths,
                captures: bot.captures,
                defenses: bot.defenses,
                damage: bot.damage_dealt,
                killstreak: bot.killstreak,
                carrying_flag: bot.carrying_flag.is_some(),
                animation_role: animation_role(bot.class, bot.active_weapon),
                last_fire_tick: bot.last_fire_tick,
                respawn_tick: bot.respawn_tick,
            })
            .collect()
    }

    pub fn stealth_condition(
        &mut self,
        identity: u32,
        enabled: bool,
        now: f32,
    ) -> Result<Option<([f32; 3], bool)>, Error> {
        let bot = self.bots.get_mut(&identity).ok_or(Error::InvalidEntity)?;
        let spy = bot.spy.as_mut().ok_or(Error::InvalidEntity)?;
        if bot.lifecycle != PlayerLifecycle::Active || spy.cloaked == enabled {
            return Ok(None);
        }
        spy.cloaked = enabled;
        if enabled {
            spy.invisibility_complete_time = now + crate::spy::CLOAK_FADE_IN_SECONDS;
        }
        Ok(Some((bot.movement.position, enabled)))
    }

    pub fn advance_spies(
        &mut self,
        tick: u64,
        human: Human,
        human_hull: Hull,
    ) -> Vec<(u32, [f32; 3])> {
        let now = tick as f32 * self.tick_interval;
        let actors = self.actors(human, tick);
        let mut decloaks = Vec::new();
        for bot in self.bots.values_mut() {
            let Some(spy) = bot.spy.as_mut() else {
                continue;
            };
            if bot.lifecycle != PlayerLifecycle::Active {
                continue;
            }
            if actors.enemies(bot.team).any(|actor| {
                crate::spy::player_hulls_touch(
                    bot.movement.position,
                    PLAYER_HULL,
                    actor.position,
                    if actor.identity == crate::PLAYER_IDENTITY {
                        human_hull
                    } else {
                        PLAYER_HULL
                    },
                )
            }) {
                spy.expose(now);
            }
            if spy.advance(now, self.tick_interval)[0] == Some(crate::spy::SpyEvent::Uncloaked) {
                decloaks.push((bot.identity, bot.movement.position));
            }
            spy.reveal_invisibility(now, bot.conditions.contains(ConditionId::URINE));
            if spy.cloaked && !bot.conditions.contains(ConditionId::STEALTHED) {
                bot.conditions
                    .add(
                        ConditionId::STEALTHED,
                        crate::condition::ConditionDuration::Permanent,
                        None,
                        true,
                        false,
                    )
                    .expect("permanent stock stealth condition");
            } else if !spy.cloaked {
                bot.conditions.remove(ConditionId::STEALTHED, true);
            }
        }
        decloaks
    }

    pub fn touches_enemy(&self, team: PlayerTeam, position: [f32; 3], hull: Hull) -> bool {
        self.bots.values().any(|bot| {
            bot.team != team
                && bot.lifecycle == PlayerLifecycle::Active
                && crate::spy::player_hulls_touch(
                    position,
                    hull,
                    bot.movement.position,
                    PLAYER_HULL,
                )
        })
    }

    fn actors(&self, human: Human, tick: u64) -> ActorFrame {
        let player = Actor {
            identity: crate::PLAYER_IDENTITY,
            class: human.class,
            team: human.team,
            alive: human.alive,
            position: human.position,
            velocity: human.velocity,
            firing_at: None,
        };
        let mut frame = ActorFrame {
            actors: [player; MAX_BOTS + 1],
            count: 0,
            teams: [[0; MAX_BOTS + 1]; 2],
            team_counts: [0; 2],
        };
        frame.insert(player);
        for bot in self.bots.values() {
            frame.insert(Actor {
                identity: bot.identity,
                class: bot.class,
                team: bot.team,
                alive: bot.lifecycle == PlayerLifecycle::Active,
                position: bot.movement.position,
                velocity: bot.movement.velocity,
                firing_at: bot
                    .last_fire_tick
                    .filter(|fired| tick.saturating_sub(*fired) <= ticks(1.0, self.tick_interval))
                    .and(bot.target),
            });
        }
        frame
    }

    pub(crate) fn synchronize_objectives(
        &mut self,
        objectives: &crate::ctf::World,
        events: &[crate::ctf::Event],
    ) {
        for bot in self.bots.values_mut() {
            let carrying = objectives.carrier_flag(bot.identity).map(|flag| flag.team);
            let carrier_changed = bot.carrying_flag != carrying;
            if carrier_changed {
                bot.carrying_flag = carrying;
                bot.next_repath_tick = 0;
            }
            if !carrier_changed
                && matches!(
                    bot.objective,
                    ObjectiveKind::GetHealth | ObjectiveKind::GetAmmo
                )
            {
                continue;
            }
            if let Some(flag) = objectives.bot_objective(bot.identity, bot.team) {
                let (objective, goal) = if flag.carrier == Some(bot.identity) {
                    (
                        ObjectiveKind::DeliverFlag,
                        flag.capture_position.unwrap_or(flag.position),
                    )
                } else if flag.carrier.is_some() {
                    (ObjectiveKind::Attack, flag.position)
                } else {
                    (ObjectiveKind::FetchFlag, flag.position)
                };
                if objective != bot.objective {
                    bot.next_repath_tick = 0;
                }
                bot.objective = objective;
                bot.goal = goal;
            }
        }
        for event in events {
            if let crate::ctf::Event::Captured { player, .. } = event
                && let Some(bot) = self.bots.get_mut(player)
            {
                bot.captures = bot.captures.saturating_add(1);
            }
        }
    }

}

fn sync_bot_ammo(bot: &mut Bot) {
    for state in bot.loadout.values() {
        if let Some(kind) = crate::weapon_ammo_kind(state.weapon) {
            bot.ammo.set(kind, state.reserve);
        }
    }
}

fn select_supply(
    mesh: &Mesh,
    bot: &Bot,
    actors: &ActorFrame,
    supplies: &[SupplyTarget],
    wanted: crate::pickup::MapPickupKind,
    cache: &mut SupplyCache,
    scratch: &mut PathScratch,
) -> Option<SupplyTarget> {
    let ratio = bot.health.current as f32 / bot.class.data().maximum_health as f32;
    let range = if wanted == crate::pickup::MapPickupKind::Health {
        let blend = if bot.afterburn.is_some() {
            0.0
        } else {
            ((ratio - HEALTH_CRITICAL_RATIO) / (HEALTH_OK_RATIO - HEALTH_CRITICAL_RATIO))
                .clamp(0.0, 1.0)
        };
        HEALTH_SEARCH_FAR_RANGE + blend * (HEALTH_SEARCH_NEAR_RANGE - HEALTH_SEARCH_FAR_RANGE)
    } else {
        AMMO_SEARCH_RANGE
    };
    let start = mesh.nearest_area(bot.movement.position)?.identity;
    if cache.facts.is_empty() {
        cache.facts.resize(supplies.len(), None);
    }
    supplies
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, supply)| supply.kind.is_none_or(|kind| kind == wanted))
        .filter(|(_, supply)| supply.team.is_none_or(|team| team == bot.team))
        .filter_map(|(index, supply)| {
            let facts = (*cache.facts[index].get_or_insert_with(|| {
                let area = mesh.nearest_area(supply.position)?;
                let closest_team = actors
                    .all()
                    .iter()
                    .filter(|actor| actor.alive)
                    .min_by(|left, right| {
                        distance(left.position, supply.position)
                            .total_cmp(&distance(right.position, supply.position))
                    })
                    .map(|actor| actor.team);
                Some(SupplyFacts {
                    area: area.identity,
                    closest_team,
                })
            }))?;
            let area = mesh.area(facts.area)?;
            if supply.kind.is_none() {
                let required = if bot.team == PlayerTeam::Red {
                    TF_NAV_SPAWN_ROOM_RED
                } else {
                    TF_NAV_SPAWN_ROOM_BLUE
                };
                if area.game_attributes & required == 0 {
                    return None;
                }
            }
            if facts
                .closest_team
                .is_some_and(|team| bot.team.is_enemy(team))
            {
                return None;
            }
            let travel = if let Some(route) = cache.routes.iter().find(|route| {
                route.team == bot.team && route.start == start && route.goal == area.identity
            }) {
                route.travel
            } else {
                let travel = mesh
                    .build_path_with_scratch(
                        start,
                        area.identity,
                        scratch,
                        |from, destination, direction, length| {
                            path_cost(
                                from,
                                destination,
                                direction,
                                length,
                                PathContext {
                                    team: bot.team,
                                    bot_identity: bot.identity,
                                    now: 0.0,
                                    route: Route::Fastest,
                                    combat_intensity: 0.0,
                                },
                            )
                        },
                    )
                    .map(|path| {
                        path.windows(2)
                            .filter_map(|pair| Some((mesh.area(pair[0])?, mesh.area(pair[1])?)))
                            .map(|(left, right)| distance(left.center(), right.center()))
                            .sum::<f32>()
                    });
                cache.routes.push(SupplyRoute {
                    team: bot.team,
                    start,
                    goal: area.identity,
                    travel,
                });
                travel
            }?;
            (travel <= range).then_some((supply, travel))
        })
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then(left.0.identity.cmp(&right.0.identity))
        })
        .map(|(supply, _)| supply)
}

fn take_melee_smack(bot: &mut Bot, tick: u64) -> Option<Attack> {
    let (due, target, weapon) = bot.pending_melee?;
    if tick <= due { return None; }
    bot.pending_melee = None;
    if bot.lifecycle != PlayerLifecycle::Active || bot.health.current <= 0 || bot.active_weapon != Some(weapon) { return None; }
    Some(Attack { phase: AttackPhase::MeleeSmack, attacker: bot.identity, team: bot.team, weapon, target,
        position: bot.movement.position, eye_position: bot_eye(bot), pitch_degrees: bot.pitch_degrees,
        yaw_degrees: bot.yaw_degrees })
}

fn bot_eye(bot: &Bot) -> [f32; 3] {
    [
        bot.movement.position[0],
        bot.movement.position[1],
        bot.movement.position[2] + bot.class.standing_eye_height(),
    ]
}
fn is_melee(weapon: Weapon) -> bool {
    matches!(
        weapon,
        Weapon::Bat
            | Weapon::Shovel
            | Weapon::Fists
            | Weapon::Kukri
            | Weapon::Bottle
            | Weapon::Wrench
            | Weapon::FireAxe
            | Weapon::Knife
            | Weapon::Bonesaw
    )
}

pub fn animation_role(class: PlayerClass, weapon: Option<Weapon>) -> AnimationRole {
    if matches!(
        weapon,
        Some(
            Weapon::Bat
                | Weapon::Shovel
                | Weapon::Fists
                | Weapon::Kukri
                | Weapon::Wrench
                | Weapon::FireAxe
                | Weapon::Bottle
                | Weapon::Knife
                | Weapon::Bonesaw
        )
    ) || class == PlayerClass::Spy
    {
        AnimationRole::Melee
    } else if class == PlayerClass::Demoman
        || matches!(
            weapon,
            Some(
                Weapon::Pistol
                    | Weapon::Shotgun
                    | Weapon::HeavyShotgun
                    | Weapon::Smg
                    | Weapon::EngineerPistol
                    | Weapon::StickybombLauncher
                    | Weapon::MediGun
            )
        )
    {
        AnimationRole::Secondary
    } else {
        AnimationRole::Primary
    }
}

fn visible_actor<W: GameplayWorld>(world: &W, bot: &Bot, actor: Actor) -> bool {
    distance(bot.movement.position, actor.position) <= MAX_VISION_RANGE
        && [actor.eye(), actor.center(), actor.position]
            .into_iter()
            .any(|point| {
                world
                    .trace(
                        bot_eye(bot),
                        point,
                        POINT_HULL,
                        crate::MASK_SOLID_BRUSH_ONLY,
                    )
                    .is_ok_and(|trace| trace.fraction >= 1.0)
            })
}
fn line_of_fire_clear<W: GameplayWorld>(world: &W, eye: [f32; 3], actor: Actor) -> bool {
    [actor.eye(), actor.center(), actor.position]
        .into_iter()
        .any(|point| {
            world
                .trace(eye, point, POINT_HULL, crate::MASK_SOLID)
                .is_ok_and(|trace| trace.fraction >= 1.0)
        })
}
fn immediate_threat(bot: &Bot, actor: Actor) -> bool {
    distance(bot.movement.position, actor.position) < IMMEDIATE_THREAT_RANGE
        || actor.firing_at == Some(bot.identity)
        || matches!(bot.difficulty, Difficulty::Hard | Difficulty::Expert)
            && matches!(actor.class, PlayerClass::Medic | PlayerClass::Engineer)
}
fn threat_order(bot: &Bot, left: Actor, right: Actor) -> std::cmp::Ordering {
    let first = immediate_threat(bot, left);
    let second = immediate_threat(bot, right);
    second
        .cmp(&first)
        .then_with(|| {
            if first && second {
                (right.class == PlayerClass::Spy)
                    .cmp(&(left.class == PlayerClass::Spy))
                    .then_with(|| {
                        (right.firing_at == Some(bot.identity))
                            .cmp(&(left.firing_at == Some(bot.identity)))
                    })
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .then_with(|| {
            distance(bot.movement.position, left.position)
                .total_cmp(&distance(bot.movement.position, right.position))
        })
        .then_with(|| left.identity.cmp(&right.identity))
}
fn equipped_definition(items: &[crate::equipment::EquippedItem], class: PlayerClass, weapon: Weapon) -> Option<u32> {
    items.iter().find_map(|item| crate::equipment::registered_item(item.definition_index)
        .filter(|definition| definition.weapon_for_class(class) == Some(weapon)).map(|_| item.definition_index))
}

fn bot_maximum_ammo(bot: &Bot) -> crate::class::AmmoLedger {
    let mut maximum = bot.class.data().maximum_ammo;
    for runtime in bot.loadout.values() {
        if let Some(ammo) = crate::weapon_ammo_kind(runtime.weapon) { maximum.set(ammo, runtime.profile().maximum_reserve); }
    }
    maximum
}

fn bot_default_weapon(bot: &Bot) -> Option<Weapon> {
    let default = crate::default_weapon(bot.class)?;
    let Some(equipment) = &bot.equipment else { return Some(default); };
    let slot = bot.class.data().stock_items.iter().find(|item| crate::equipment::registered_item(item.definition)
        .is_some_and(|item| item.weapon_for_class(bot.class) == Some(default)))?.slot;
    equipment.active.iter().find(|item| item.slot as u8 == slot)
        .and_then(|item| crate::equipment::registered_item(item.definition_index))
        .and_then(|item| item.weapon_for_class(bot.class))
        .filter(|weapon| bot.loadout.contains_key(weapon))
}

fn apply_bot_equipment(bot: &mut Bot) {
    let Some(equipment) = &mut bot.equipment else { return; };
    let items = equipment.selected.equipped_items(bot.class);
    let old_items = std::mem::replace(&mut equipment.active, items);
    let same_class = equipment.class == bot.class;
    if !same_class || old_items != equipment.active {
        equipment.providers = crate::equipment::AttributeProviders::new(&equipment.active, bot.class);
    }
    equipment.class = bot.class;
    let mut previous = std::mem::take(&mut bot.loadout);
    for weapon in equipment.selected.weapons(bot.class) {
        let context = crate::weapon::ProfileContext { decapitations: bot.decapitations,
            ammo: crate::weapon_ammo_kind(weapon), gun: crate::weapon_ammo_kind(weapon).is_some(),
            blast_impact: weapon == Weapon::GrenadeLauncher || weapon_damage_type(weapon).is_some_and(|kind| kind.contains(DamageType::BLAST)), ..Default::default() };
        let mut runtime = WeaponRuntime::full_with_attributes(weapon, context, |target, hook, input| match target {
            crate::weapon::AttributeTarget::Weapon => equipment.providers.weapon(weapon, hook, input),
            crate::weapon::AttributeTarget::Player => equipment.providers.player(hook, input),
        });
        if same_class && equipped_definition(&old_items, bot.class, weapon) == equipped_definition(&equipment.active, bot.class, weapon)
            && let Some(mut old) = previous.remove(&weapon) {
            old.resolved_profile = runtime.resolved_profile;
            old.discard_chambered_on_reload = runtime.discard_chambered_on_reload;
            old.spinup_seconds = runtime.spinup_seconds;
            runtime = old;
        } else {
            runtime.generation = equipment.next_generation;
            equipment.next_generation = equipment.next_generation.checked_add(1).expect("bounded bot weapon generation");
        }
        bot.loadout.insert(weapon, runtime);
    }
    let maximum = HealthState::spawn(bot.class, equipment.providers.player("add_maxhealth", 0.0),
        equipment.providers.player("add_maxhealth_nonbuffed", 0.0)).expect("validated equipped health attributes");
    bot.health.maximum = maximum.maximum;
    bot.health.maximum_for_buffing = maximum.maximum_for_buffing;
    if bot.active_weapon.is_none_or(|weapon| !bot.loadout.contains_key(&weapon)) { bot.active_weapon = bot_default_weapon(bot); }
    bot.equipment.as_mut().unwrap().providers.set_active(bot.active_weapon);
}

fn bot_player_attribute(bot: &mut Bot, hook: &str, input: f32) -> f32 {
    let Some(equipment) = &mut bot.equipment else { return input; };
    equipment.providers.set_active(bot.active_weapon);
    equipment.providers.player(hook, input)
}

fn apply_aiming_policy(bot: &mut Bot, policy: &mut playsrc_movement::Policy) {
    if !bot.conditions.contains(ConditionId::AIMING) { return; }
    if bot.class == PlayerClass::Heavy {
        policy.maximum_speed = policy.maximum_speed.min(bot_player_attribute(bot, "mult_player_aiming_movespeed", 110.0));
        policy.allow_jump = false;
    } else if bot.class == PlayerClass::Sniper { policy.maximum_speed = policy.maximum_speed.min(80.0); }
}

fn apply_minigun_aiming(conditions: &mut ConditionState, previous: crate::weapon::MinigunState, current: crate::weapon::MinigunState) -> Result<(), Error> {
    if let Some(aiming) = crate::weapon::minigun_aiming_transition(previous, current) {
        if aiming { conditions.add(ConditionId::AIMING, crate::condition::ConditionDuration::Permanent, None, true, false).map_err(|_| Error::InvalidEntity)?; }
        else { conditions.remove(ConditionId::AIMING, false); }
    }
    Ok(())
}

fn bot_movement_policy(bot: &mut Bot) -> playsrc_movement::Policy {
    let mut policy = MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve();
    apply_aiming_policy(bot, &mut policy);
    policy.maximum_speed = bot_player_attribute(bot, "mult_player_movespeed", policy.maximum_speed);
    if policy.bunnyhop_speed_cap.is_some() { policy.bunnyhop_speed_cap = Some(policy.maximum_speed * 1.2); }
    policy
}

fn select_weapon(bot: &mut Bot, threat: Option<Actor>, tick: u64, interval: f32) {
    let Some(primary) = bot_default_weapon(bot) else {
        return;
    };
    let mut selected = primary;
    if bot.difficulty != Difficulty::Easy
        && let Some(threat) = threat
    {
        match bot.class {
            PlayerClass::Scout
                if bot
                    .loadout
                    .get(&primary)
                    .is_some_and(|weapon| weapon.clip == 0)
                    && bot
                        .loadout
                        .get(&Weapon::Pistol)
                        .is_some_and(|weapon| weapon.clip > 0 || weapon.reserve > 0) =>
            {
                selected = Weapon::Pistol
            }
            PlayerClass::Soldier
                if bot
                    .loadout
                    .get(&primary)
                    .is_some_and(|weapon| weapon.clip == 0)
                    && bot
                        .loadout
                        .get(&Weapon::Shotgun)
                        .is_some_and(|weapon| weapon.clip > 0)
                    && distance(bot.movement.position, threat.position)
                        < SOLDIER_SECONDARY_RANGE =>
            {
                selected = Weapon::Shotgun
            }
            PlayerClass::Sniper
                if bot
                    .loadout
                    .get(&Weapon::Smg)
                    .is_some_and(|weapon| weapon.clip > 0 || weapon.reserve > 0)
                    && distance(bot.movement.position, threat.position)
                        < SNIPER_SECONDARY_RANGE =>
            {
                selected = Weapon::Smg
            }
            _ => {}
        }
    }
    if bot.active_weapon != Some(selected) {
        bot.pending_melee = None;
        if let Some(previous) = bot
            .active_weapon
            .and_then(|weapon| bot.loadout.get_mut(&weapon))
        {
            previous.abort_reload();
            previous.charge_begin_tick = None;
            previous.smack_due_tick = None;
        }
        if let Some(weapon) = bot.loadout.get_mut(&selected) {
            weapon.deploy(tick, interval);
            bot.active_weapon = Some(selected);
        }
    }
}
fn aim_point<W: GameplayWorld>(world: &W, bot: &Bot, threat: Actor) -> [f32; 3] {
    if bot.active_weapon == Some(Weapon::RocketLauncher) && bot.difficulty != Difficulty::Easy {
        if threat.position[2] - 30.0 > bot.movement.position[2] {
            for point in [threat.position, threat.center(), threat.eye()] {
                if world
                    .trace(
                        bot_eye(bot),
                        point,
                        POINT_HULL,
                        crate::MASK_SOLID_BRUSH_ONLY,
                    )
                    .is_ok_and(|trace| trace.fraction >= 1.0)
                {
                    return point;
                }
            }
        }
        let range = distance(bot.movement.position, threat.position);
        if range > ROCKET_LEAD_MINIMUM_RANGE {
            let travel = range / ROCKET_SPEED;
            let led = crate::add(threat.position, crate::scale(threat.velocity, travel));
            if world
                .trace(bot_eye(bot), led, POINT_HULL, crate::MASK_SOLID_BRUSH_ONLY)
                .is_ok_and(|trace| trace.fraction >= 1.0)
            {
                return led;
            }
            return crate::add(threat.eye(), crate::scale(threat.velocity, travel));
        }
        return threat.eye();
    }
    threat.center()
}
fn max_attack_range(weapon: Weapon) -> f32 {
    if is_melee(weapon) {
        MELEE_MAX_ATTACK_RANGE
    } else if matches!(weapon, Weapon::RocketLauncher | Weapon::Original) {
        ROCKET_MAX_ATTACK_RANGE
    } else {
        f32::MAX
    }
}
pub(crate) fn weapon_damage_type(weapon: Weapon) -> Option<DamageType> {
    Some(match weapon {
        Weapon::Bat
        | Weapon::Shovel
        | Weapon::Fists
        | Weapon::Kukri
        | Weapon::Wrench
        | Weapon::FireAxe
        | Weapon::Bottle
        | Weapon::Knife
        | Weapon::Bonesaw => DamageType::MELEE | DamageType::NEVER_GIB | DamageType::CLUB,
        Weapon::Flamethrower => DamageType::IGNITE | DamageType::PREVENT_FORCE,
        Weapon::RocketLauncher
        | Weapon::DirectHit | Weapon::BlackBox | Weapon::LibertyLauncher
        | Weapon::RocketJumper | Weapon::AirStrike
        | Weapon::Original
        | Weapon::GrenadeLauncher => DamageType::BLAST | DamageType::HALF_FALLOFF | DamageType::USE_DISTANCE,
        Weapon::StickybombLauncher => DamageType::BLAST | DamageType::HALF_FALLOFF | DamageType::NO_CLOSE_DISTANCE,
        Weapon::Scattergun | Weapon::Shotgun | Weapon::HeavyShotgun | Weapon::EngineerShotgun => {
            DamageType::BUCKSHOT | DamageType::USE_DISTANCE
        }
        Weapon::HandgunScoutPrimary => DamageType::BUCKSHOT | DamageType::BULLET | DamageType::USE_DISTANCE,
        Weapon::Pistol
        | Weapon::Minigun
        | Weapon::Smg
        | Weapon::EngineerPistol
        | Weapon::Revolver => DamageType::BULLET | DamageType::USE_DISTANCE,
        Weapon::SniperRifle => DamageType::BULLET | DamageType::USE_HITLOCATIONS,
        Weapon::SyringeGun => DamageType::BULLET | DamageType::USE_DISTANCE | DamageType::NO_CLOSE_DISTANCE | DamageType::PREVENT_FORCE,
        Weapon::FlareGun | Weapon::Detonator | Weapon::ScorchShot | Weapon::Manmelter => DamageType::BULLET | DamageType::IGNITE,
        Weapon::Sapper
        | Weapon::DisguiseKit
        | Weapon::InvisibilityWatch
        | Weapon::BuildPda
        | Weapon::DestroyPda
        | Weapon::Toolbox
        | Weapon::MediGun => return None,
    })
}

fn next_control_point_spawn(candidates: &[Spawn], last: &mut Option<u32>, class: PlayerClass) -> Option<usize> {
    let start = last.and_then(|id| candidates.iter().position(|spawn| spawn.identity > id)).unwrap_or(0);
    for restrict_class in [true, false] {
        for offset in 0..candidates.len() {
            let index = (start + offset) % candidates.len();
            let spawn = candidates[index];
            if spawn.position == [0.0;3] || (restrict_class && spawn.class_flags != 0 && spawn.class_flags & (1 << (class as u8 - 1)) == 0) { continue; }
            *last = Some(spawn.identity);
            return Some(index);
        }
    }
    None
}

fn frame_has_points(frame: Option<Objectives<'_>>) -> bool { frame.is_some_and(|frame| frame.points.is_some()) }

fn control_point_actor(bot: &Bot) -> crate::control_point::Actor {
    let policy = MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve();
    let mut actor = crate::control_point::Actor::active(bot.identity, bot.team, bot.class, bot.movement.position, bot.movement.active_hull(policy));
    actor.alive = bot.lifecycle == PlayerLifecycle::Active && bot.health.current > 0;
    actor.invulnerable = bot.conditions.is_invulnerable();
    actor.stealthed = [ConditionId::STEALTHED, ConditionId::STEALTHED_USER, ConditionId::STEALTHED_USER_FADING].into_iter().any(|c| bot.conditions.contains(c));
    actor.megaheal = bot.conditions.contains(ConditionId::MEGAHEAL);
    actor.phased = bot.conditions.contains(ConditionId::PHASE);
    actor.control_stunned = bot.conditions.is_control_stunned();
    actor.enemy_disguise = bot.spy.and_then(|s| s.disguise).is_some_and(|d| d.team != bot.team);
    actor
}

fn respawn_bot(bot: &mut Bot, spawn: Spawn, mesh: &Mesh, tick: u64, interval: f32) {
    bot.point_action = control_point::Action::default();
    if matches!(bot.objective, ObjectiveKind::CapturePoint | ObjectiveKind::DefendPoint | ObjectiveKind::BlockCapture) {
        bot.objective = ObjectiveKind::CapturePoint;
    }
    bot.movement_stuns = crate::hitscan::MovementStuns::default();
    bot.weapon_knockback = false;
    bot.scattergun_jumped = false;
    bot.blast_since_movement = false;
    bot.blast_jump_state = false;
    crate::admission_metrics::begin_tick(tick);
    crate::admission_metrics::emit(crate::admission_metrics::RESPAWN, bot.identity);
    let policy = MovementPolicy {
        class: bot.class,
        modifiers: MovementModifiers::default(),
    }
    .resolve();
    bot.movement = State::from_player(
        Player {
            position: spawn.position,
            velocity: [0.0; 3],
            grounded: false,
            crouched: false,
            jump_latched: false,
        },
        policy,
    );
    bot.lifecycle = PlayerLifecycle::Active;
    bot.in_water = false;
    bot.blast_since_movement = false;
    bot.blast_jump_state = false;
    bot.damagers = crate::deathnotice::DamagerHistory::default();
    bot.health =
        HealthState::spawn(bot.class, 0.0, 0.0).expect("authored bot class health is valid");
    bot.conditions = ConditionState::default();
    bot.critical_history.reset_for_spawn();
    bot.decapitations = 0;
    bot.spy = (bot.class == PlayerClass::Spy).then(crate::spy::SpyState::default);
    bot.ammo = bot.class.data().maximum_ammo;
    bot.next_regenerate_tick = 0;
    bot.yaw_degrees = spawn.angles[1];
    bot.pitch_degrees = 0.0;
    bot.target = None;
    bot.known_since.clear();
    bot.next_target_tick = tick;
    bot.next_repath_tick = tick;
    bot.current_area = mesh.nearest_area(spawn.position).map(|area| area.identity);
    bot.path.clear();
    bot.path_index = 0;
    bot.crossing = None;
    bot.path_crossings = Arc::default();
    bot.nav_area_mark = None;
    bot.avoid_at = 0.0;
    bot.locomotion_command=MoveCommand::default();bot.stuck=locomotion::Monitor::default();
    apply_bot_equipment(bot);
    bot.health.current = bot.health.maximum;
    bot.ammo = bot_maximum_ammo(bot);
    bot.active_weapon = bot_default_weapon(bot);
    bot.pending_melee = None;
    bot.respawn_tick = None;
    bot.death_tick = None;
    for weapon in bot.loadout.values_mut() {
        weapon.reset_for_spawn();
    }
    if let Some(weapon) = bot
        .active_weapon
        .and_then(|weapon| bot.loadout.get_mut(&weapon))
    {
        weapon.deploy(tick, interval);
    }
}
pub fn segment_player(start: [f32; 3], end: [f32; 3], position: [f32; 3]) -> Option<f32> {
    let mins = crate::add(position, PLAYER_HULL.mins);
    let maxs = crate::add(position, PLAYER_HULL.maxs);
    segment_bounds(start, end, mins, maxs)
}

pub(crate) fn segment_bounds(start: [f32; 3], end: [f32; 3], mins: [f32; 3], maxs: [f32; 3]) -> Option<f32> {
    let mut enter = 0.0_f32;
    let mut leave = 1.0_f32;
    for axis in 0..3 {
        let delta = end[axis] - start[axis];
        if delta.abs() <= f32::EPSILON {
            if start[axis] < mins[axis] || start[axis] > maxs[axis] {
                return None;
            }
            continue;
        }
        let first = (mins[axis] - start[axis]) / delta;
        let second = (maxs[axis] - start[axis]) / delta;
        enter = enter.max(first.min(second));
        leave = leave.min(first.max(second));
        if enter > leave {
            return None;
        }
    }
    (leave >= 0.0 && enter <= 1.0).then_some(enter.max(0.0))
}

fn preset_spawn_class(
    bots: &BTreeMap<u32, Bot>,
    team: PlayerTeam,
    human_team: PlayerTeam,
    human_class: PlayerClass,
) -> Option<PlayerClass> {
    const OFFENSE: [PlayerClass; 12] = [
        PlayerClass::Medic,
        PlayerClass::Engineer,
        PlayerClass::Soldier,
        PlayerClass::Heavy,
        PlayerClass::Demoman,
        PlayerClass::Scout,
        PlayerClass::Pyro,
        PlayerClass::Soldier,
        PlayerClass::Demoman,
        PlayerClass::Sniper,
        PlayerClass::Medic,
        PlayerClass::Spy,
    ];
    const DEFENSE: [PlayerClass; 12] = [
        PlayerClass::Medic,
        PlayerClass::Engineer,
        PlayerClass::Soldier,
        PlayerClass::Demoman,
        PlayerClass::Scout,
        PlayerClass::Heavy,
        PlayerClass::Sniper,
        PlayerClass::Engineer,
        PlayerClass::Soldier,
        PlayerClass::Medic,
        PlayerClass::Pyro,
        PlayerClass::Spy,
    ];
    let roster = if team == PlayerTeam::Red {
        DEFENSE
    } else {
        OFFENSE
    };
    let mut consumed = [0_usize; 10];
    for class in roster {
        let index = class as usize;
        let present = bots
            .values()
            .filter(|bot| bot.team == team && bot.class == class)
            .count()
            + usize::from(human_team == team && human_class == class);
        if present > consumed[index] {
            consumed[index] += 1;
        } else {
            return Some(class);
        }
    }
    None
}

pub fn path_cost(
    from: &Area,
    destination: &Area,
    direction: Direction,
    length: f32,
    context: PathContext,
) -> Option<f32> {
    let flags = destination.game_attributes;
    if flags & TF_NAV_UNBLOCKABLE == 0
        && (flags & TF_NAV_BLOCKED != 0
            || context.team == PlayerTeam::Red && flags & TF_NAV_BLUE_ONE_WAY_DOOR != 0
            || context.team == PlayerTeam::Blue && flags & TF_NAV_RED_ONE_WAY_DOOR != 0)
    {
        return None;
    }
    if context.team == PlayerTeam::Red && flags & TF_NAV_SPAWN_ROOM_BLUE != 0
        || context.team == PlayerTeam::Blue && flags & TF_NAV_SPAWN_ROOM_RED != 0
    {
        return None;
    }
    let delta = from.connection_height_change(destination, direction);
    let mut cost = length;
    if delta >= STEP_HEIGHT {
        if delta >= MAX_JUMP_HEIGHT {
            return None;
        }
        cost *= 2.0;
    } else if delta < -DEATH_DROP_HEIGHT {
        return None;
    }
    let preference = if context.route == Route::Default {
        let time_mod = (context.now / 10.0) as i32 + 1;
        1.0 + 50.0
            * (1.0 + ((context.bot_identity as i64
                * destination.identity as i64
                * i64::from(time_mod)) as f32)
                .cos())
    } else {
        1.0
    };
    if context.route == Route::Safest {
        if context.combat_intensity > 0.01 { cost *= 4.0 * context.combat_intensity; }
        if (context.team == PlayerTeam::Red && flags & 0x80 != 0) || (context.team == PlayerTeam::Blue && flags & 0x100 != 0) { cost *= 5.0; }
    }
    Some(cost * preference)
}

fn objective(
    scenario: Scenario,
    team: PlayerTeam,
    threat: Option<[f32; 3]>,
    carrying_flag: Option<PlayerTeam>,
) -> (ObjectiveKind, [f32; 3]) {
    match scenario {
        Scenario::ControlPoints { initial } => (ObjectiveKind::CapturePoint, initial),
        Scenario::Payload { cart, forward } if team == PlayerTeam::Blue => {
            let position = if let Some(threat) = threat {
                let away = [cart[0] - threat[0], cart[1] - threat[1]];
                let length = away[0].hypot(away[1]);
                if length > 0.0 {
                    [
                        cart[0] + PAYLOAD_PUSH_RADIUS * away[0] / length,
                        cart[1] + PAYLOAD_PUSH_RADIUS * away[1] / length,
                        cart[2],
                    ]
                } else {
                    cart
                }
            } else {
                [
                    cart[0] - forward[0] * PAYLOAD_PUSH_RADIUS,
                    cart[1] - forward[1] * PAYLOAD_PUSH_RADIUS,
                    cart[2],
                ]
            };
            (ObjectiveKind::PayloadPush, position)
        }
        Scenario::Payload { cart, .. } => (ObjectiveKind::PayloadGuard, cart),
        Scenario::CaptureTheFlag { flags, captures } => {
            if carrying_flag.is_some() {
                (
                    ObjectiveKind::DeliverFlag,
                    captures[team_index(team)].position,
                )
            } else {
                let enemy = if team == PlayerTeam::Red {
                    PlayerTeam::Blue
                } else {
                    PlayerTeam::Red
                };
                let flag = flags[team_index(enemy)];
                (ObjectiveKind::FetchFlag, flag.home)
            }
        }
    }
}

fn scenario(graph: &Graph, objectives: Option<&crate::ctf::World>) -> Result<Scenario, Error> {
    if !graph.entities.iter().any(|e| classname(e, b"team_train_watcher")) {
        if let Some(point) = graph.entities.iter().find(|e| classname(e, b"team_control_point")) {
            return Ok(Scenario::ControlPoints { initial: vector(point, b"origin").ok_or(Error::InvalidEntity)? });
        }
    }
    if let Some(watcher) = graph
        .entities
        .iter()
        .find(|entity| classname(entity, b"team_train_watcher"))
    {
        let train_name = scalar(watcher, b"train").ok_or(Error::MissingPayload)?;
        let train = graph
            .entities
            .iter()
            .find(|entity| {
                classname(entity, b"func_tracktrain")
                    && entity
                        .targetname
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(train_name))
            })
            .ok_or(Error::MissingPayload)?;
        let cart = vector(train, b"origin").ok_or(Error::MissingPayload)?;
        let start_name = scalar(watcher, b"start_node").ok_or(Error::MissingPathTrack)?;
        let start = graph
            .entities
            .iter()
            .find(|entity| {
                classname(entity, b"path_track")
                    && entity
                        .targetname
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(start_name))
            })
            .ok_or(Error::MissingPathTrack)?;
        let next_name = scalar(start, b"target").ok_or(Error::MissingPathTrack)?;
        let next = graph
            .entities
            .iter()
            .find(|entity| {
                classname(entity, b"path_track")
                    && entity
                        .targetname
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(next_name))
            })
            .ok_or(Error::MissingPathTrack)?;
        let begin = vector(start, b"origin").ok_or(Error::MissingPathTrack)?;
        let end = vector(next, b"origin").ok_or(Error::MissingPathTrack)?;
        let delta = [end[0] - begin[0], end[1] - begin[1], end[2] - begin[2]];
        let length = delta[0].hypot(delta[1]).hypot(delta[2]);
        if length == 0.0 {
            return Err(Error::MissingPathTrack);
        }
        return Ok(Scenario::Payload {
            cart,
            forward: [delta[0] / length, delta[1] / length, delta[2] / length],
        });
    }
    let mut flags: [Option<Flag>; 2] = [None, None];
    let mut captures: [Option<CaptureZone>; 2] = [None, None];
    for entity in &graph.entities {
        let team = match scalar(entity, b"TeamNum") {
            Some(b"2") => PlayerTeam::Red,
            Some(b"3") => PlayerTeam::Blue,
            _ => continue,
        };
        let Some(position) = vector(entity, b"origin").or_else(|| {
            objectives.and_then(|world| {
                world
                    .zones()
                    .find(|zone| zone.identity == entity.index as u32 && zone.team == Some(team))
                    .map(|zone| zone.center)
            })
        }) else {
            continue;
        };
        if classname(entity, b"item_teamflag") {
            flags[team_index(team)] = Some(Flag { home: position });
        } else if classname(entity, b"func_capturezone") {
            captures[team_index(team)] = Some(CaptureZone { position });
        }
    }
    match (flags, captures) {
        ([Some(red), Some(blue)], [Some(red_capture), Some(blue_capture)]) => {
            Ok(Scenario::CaptureTheFlag {
                flags: [red, blue],
                captures: [red_capture, blue_capture],
            })
        }
        _ => Err(Error::MissingScenario),
    }
}

fn ticks(seconds: f32, interval: f32) -> u64 {
    (seconds / interval).ceil().max(1.0) as u64
}

fn distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] - right[0])
        .hypot(left[1] - right[1])
        .hypot(left[2] - right[2])
}

fn team_index(team: PlayerTeam) -> usize {
    match team {
        PlayerTeam::Red => 0,
        PlayerTeam::Blue => 1,
        PlayerTeam::Unassigned | PlayerTeam::Spectator => unreachable!(),
    }
}

fn classname(entity: &Entity, name: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|classname| classname.eq_ignore_ascii_case(name))
}

fn scalar<'a>(entity: &'a Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}

fn vector(entity: &Entity, key: &[u8]) -> Option<[f32; 3]> {
    let value = std::str::from_utf8(scalar(entity, key)?).ok()?;
    let mut values = value.split_whitespace();
    let result = [
        values.next()?.parse::<f32>().ok()?,
        values.next()?.parse::<f32>().ok()?,
        values.next()?.parse::<f32>().ok()?,
    ];
    (values.next().is_none() && result.iter().all(|value| value.is_finite())).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_movement::{Error as MoveError, Trace, Tracer};

    #[test]
    fn stuck_buttons_survive_between_behavior_updates_while_physics_advances_every_tick(){
        let mut world=BotWorld::new(fixture_mesh(),&fixture_graph(),&Floor,0.015,None).unwrap();
        let mut random=UniformRandomStream::from_seed(7).unwrap();
        world.apply(Request{operation:Operation::Add,count:1,class:Some(PlayerClass::Scout),team:Some(PlayerTeam::Red),difficulty:Difficulty::Normal},PlayerTeam::Blue,PlayerClass::Soldier,&mut random).unwrap();
        let bot=world.bots.values_mut().next().unwrap();
        bot.stuck.clear(-5.0,bot.movement.position);bot.stuck.request_move(0.0);
        world.advance(&Floor,0,human_far(),&[],&mut random,None).unwrap();
        let bot=world.bots.values().next().unwrap();let command=bot.locomotion_command;
        assert!(command.jump);assert_ne!(command.side,0.0);assert_eq!(world.scheduler.last_update(bot.identity),0);
        let mut position=bot.movement.position;
        for tick in 1..7{
            world.advance(&Floor,tick,human_far(),&[],&mut random,None).unwrap();
            let bot=world.bots.values().next().unwrap();
            assert_eq!(bot.locomotion_command,command);assert_eq!(world.scheduler.last_update(bot.identity),0);
            assert_ne!(bot.movement.position,position,"movement still runs on every 15ms tick");position=bot.movement.position;
        }
        world.advance(&Floor,7,human_far(),&[],&mut random,None).unwrap();
        assert_eq!(world.scheduler.last_update(world.bots.values().next().unwrap().identity),7);
    }

    #[test]
    fn path_facing_does_not_snap_an_airborne_bot_toward_its_waypoint(){
        let mut world=BotWorld::new(fixture_mesh(),&fixture_graph(),&Floor,0.015,None).unwrap();
        let mut random=UniformRandomStream::from_seed(7).unwrap();
        world.apply(Request{operation:Operation::Add,count:1,class:Some(PlayerClass::Scout),team:Some(PlayerTeam::Blue),difficulty:Difficulty::Normal},PlayerTeam::Red,PlayerClass::Soldier,&mut random).unwrap();
        let bot=world.bots.values_mut().next().unwrap();
        bot.movement.position[2]=100.0;bot.movement.ground=None;bot.yaw_degrees=90.0;
        world.advance(&Floor,0,human_far(),&[],&mut random,None).unwrap();
        let bot=world.bots.values().next().unwrap();
        assert!(bot.movement.ground.is_none());assert_eq!(bot.yaw_degrees,90.0);
        assert_eq!(bot.locomotion_command.yaw_degrees,90.0);
    }

    #[test]
    fn drop_crossings_trace_the_full_hull_beyond_the_ledge_and_wait_for_landing() {
        struct Ledge(std::cell::RefCell<Vec<f32>>);
        impl Tracer for Ledge {
            fn trace(&self, start:[f32;3], end:[f32;3], hull:Hull, mask:u32)->Result<Trace,MoveError> {
                if hull.mins[2] == STEP_HEIGHT {
                    assert_eq!(hull.mins[0],-26.5,"Path::ComputePathDetails inflates the player's hull width by five units");
                    self.0.borrow_mut().push(start[0]);
                    if start[0] + hull.mins[0] < 100.0 {
                        return Ok(Trace { fraction:0.0,start_solid:true,all_solid:false,end:start,normal:None,hit:Some(0),contents:1 });
                    }
                }
                Floor.trace(start,end,hull,mask)
            }
        }
        impl GameplayWorld for Ledge { fn overlaps_model_hull(&self, _:usize, _:[f32;3], _:[f32;3], _:Hull)->Result<bool,MoveError>{Ok(false)} }
        let mesh=fixture_mesh();
        let mut from=mesh.areas[0].clone(); let mut to=mesh.areas[1].clone();
        from.northwest=[0.0,0.0,200.0]; from.southeast=[100.0,100.0,200.0]; from.northeast_z=200.0; from.southwest_z=200.0;
        to.northwest=[100.0,0.0,0.0]; to.southeast=[200.0,100.0,0.0]; to.northeast_z=0.0; to.southwest_z=0.0;
        let world=Ledge(Default::default());
        let crossing=path::Crossing::compute(&world,&from,&to,Direction::East,[100.0,50.0,200.0],[50.0,50.0,200.0],PlayerClass::Scout).unwrap();
        assert_eq!(*world.0.borrow(),[100.0,110.0,120.0,130.0]);
        assert_eq!(crossing.drop_position,Some([130.0,50.0,200.0]));
        assert!(!crossing.landed([130.0,50.0,18.0]));
        assert!(crossing.landed([130.0,50.0,17.9]));
        to.northwest[2]=200.0; to.southeast[2]=200.0; to.northeast_z=200.0; to.southwest_z=200.0;
        let crossing=path::Crossing::compute(&world,&from,&to,Direction::East,[100.0,50.0,200.0],[50.0,50.0,200.0],PlayerClass::Scout).unwrap();
        assert_eq!(crossing.drop_position,None);
        assert!(crossing.reached([124.0,50.0,400.0]));
        assert!(!crossing.reached([125.0,50.0,200.0]));
    }

    #[test]
    fn path_details_start_at_area_center_when_last_known_area_does_not_contain_feet(){
        let mesh=fixture_mesh();
        for feet in [[50.0,-100.0,0.0],[50.0,80.0,-50.0]]{
            let crossings=path::compute(&Floor,&mesh,&[1,2],feet,PlayerClass::Scout).unwrap();
            assert_eq!(crossings[0].position,[100.0,50.0,0.0]);
        }
        let crossings=path::compute(&Floor,&mesh,&[1,2],[50.0,80.0,0.0],PlayerClass::Scout).unwrap();
        assert_eq!(crossings[0].position,[100.0,75.0,0.0]);
    }

    #[derive(Clone)]
    struct Floor;

    impl Tracer for Floor {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            hull: Hull,
            _mask: u32,
        ) -> Result<Trace, MoveError> {
            let floor = -hull.mins[2];
            if end[2] < floor {
                let fraction = ((start[2] - floor) / (start[2] - end[2])).clamp(0.0, 1.0);
                Ok(Trace {
                    fraction,
                    start_solid: false,
                    all_solid: false,
                    end: [
                        start[0] + (end[0] - start[0]) * fraction,
                        start[1] + (end[1] - start[1]) * fraction,
                        floor,
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
            model: usize,
            origin: [f32; 3],
            position: [f32; 3],
            _hull: Hull,
        ) -> Result<bool, MoveError> {
            Ok(matches!(model, 1 | 2)
                && (position[0] - origin[0]).abs() <= 24.0
                && (position[1] - origin[1]).abs() <= 24.0)
        }
    }

    fn fixture_mesh() -> Mesh {
        let mut bytes = Vec::new();
        bytes.extend(playsrc_nav::MAGIC.to_le_bytes());
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(2_u32.to_le_bytes());
        bytes.extend(128_u32.to_le_bytes());
        bytes.push(1);
        bytes.extend(0_u16.to_le_bytes());
        bytes.push(1);
        bytes.extend(3_u32.to_le_bytes());
        for identity in 1_u32..=3 {
            let x = (identity - 1) as f32 * 100.0;
            bytes.extend(identity.to_le_bytes());
            bytes.extend(0_u32.to_le_bytes());
            for value in [x, 0.0, 0.0, x + 100.0, 100.0, 0.0, 0.0, 0.0] {
                bytes.extend(value.to_le_bytes());
            }
            for direction in 0..4 {
                let target = match (direction, identity) {
                    (1, 1 | 2) => Some(identity + 1),
                    (3, 2 | 3) => Some(identity - 1),
                    _ => None,
                };
                bytes.extend(u32::from(target.is_some()).to_le_bytes());
                if let Some(target) = target {
                    bytes.extend(target.to_le_bytes());
                }
            }
            bytes.push(0);
            bytes.extend(0_u32.to_le_bytes());
            bytes.extend(0_u16.to_le_bytes());
            bytes.extend([0; 8 + 8 + 16 + 8 + 4]);
        }
        bytes.extend(0_u32.to_le_bytes());
        playsrc_nav::parse(
            &bytes,
            playsrc_nav::Profile::TeamFortress2,
            Some(128),
            playsrc_nav::Limits::default(),
        )
        .unwrap()
    }

    fn fixture_graph() -> Graph {
        playsrc_entity::parse(
            b"{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"\"angles\"\"0 0 0\"}\n{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"\"angles\"\"0 180 0\"}\n{\"classname\"\"team_train_watcher\"\"train\"\"cart\"\"start_node\"\"first\"}\n{\"classname\"\"func_tracktrain\"\"targetname\"\"cart\"\"origin\"\"150 50 1\"}\n{\"classname\"\"path_track\"\"targetname\"\"first\"\"target\"\"second\"\"origin\"\"150 50 1\"}\n{\"classname\"\"path_track\"\"targetname\"\"second\"\"origin\"\"200 50 1\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap()
    }

    fn direct_damage(weapon: Weapon, victim: u32, amount: f32) -> Damage {
        Damage { attacker: crate::PLAYER_IDENTITY, victim, weapon, amount, position: [0.0; 3],
            source_weapon: None,
            damage_type: weapon_damage_type(weapon).unwrap(), force: [0.0; 3],
            crit: CritKind::None, range_multiplier: 1.0, custom: CustomDamage::None,
            modifiers: DamageModifiers::default(), killing_weapon: None }
    }

    fn melee_session(definition: u32, class: PlayerClass, weapon: Weapon) -> (crate::Session<Floor>, u32) {
        melee_session_map(definition, class, weapon, crate::MapRuntime::empty(0.015))
    }

    fn melee_session_map(definition: u32, class: PlayerClass, weapon: Weapon, map: crate::MapRuntime) -> (crate::Session<Floor>, u32) {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], map);
        session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
        session.equip_item(class, crate::schema::LoadoutPosition::Melee, Some(definition)).unwrap();
        session.advance(crate::Command { nextbot_stop: true, select_class: Some(class), respawn: true, bot_request: Some(Request {
            operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy), team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal,
        }), ..Default::default() }).unwrap();
        session.advance(crate::Command { nextbot_stop: true, select_weapon: Some(weapon), ..Default::default() }).unwrap();
        for _ in 0..400 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        let target = session.bots.as_ref().unwrap().snapshots()[0].identity;
        session.movement.position = [0.0; 3];
        let bot = session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap();
        bot.movement.position = [50.0, 0.0, 0.0];
        session.loadout.get_mut(&weapon).unwrap().critical.bucket.token_bucket = -250.0;
        (session, target)
    }

    fn melee_swing(session: &mut crate::Session<Floor>) -> crate::Snapshot {
        let started = session.advance(crate::Command { fire: true, nextbot_stop: true, ..Default::default() }).unwrap();
        assert!(session.pending_melee_tick.is_some() || session.loadout.get(&Weapon::Fists).is_some_and(|weapon| weapon.smack_due_tick.is_some()));
        for _ in 0..13 {
            let frame = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
            assert!(!frame.events.iter().any(|event| matches!(event, crate::Event::MeleeImpact { .. })));
        }
        let hit = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(hit.tick - started.tick, 14);
        hit
    }

    fn equipped_bot_melee(definition: u32, class: PlayerClass, weapon: Weapon) -> (crate::Session<Floor>, u32) {
        let mut session = crate::Session::new(Floor, [0.0,0.0,1.0], crate::MapRuntime::empty(0.015));
        session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
        session.advance(crate::Command { select_class: Some(PlayerClass::Heavy), respawn: true, nextbot_stop: true,
            bot_request: Some(Request { operation: Operation::Add, count: 1, class: Some(class), team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal }), ..Default::default() }).unwrap();
        let identity = session.bots.as_ref().unwrap().snapshots()[0].identity;
        session.equip_bot_item(identity, crate::schema::LoadoutPosition::Melee, Some(definition)).unwrap();
        assert_ne!(session.equipped_weapon_definition(identity, weapon), Some(definition));
        assert!(session.bots.as_mut().unwrap().regenerate(identity, session.tick));
        assert_eq!(session.equipped_weapon_definition(identity, weapon), Some(definition));
        for _ in 0..400 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        session.movement.position = [0.0;3];
        let bot = session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap();
        bot.movement.position = [50.0,0.0,0.0]; bot.active_weapon = Some(weapon); bot.yaw_degrees = 180.0; bot.pitch_degrees = 0.0;
        bot.loadout.get_mut(&weapon).unwrap().critical.bucket.token_bucket = -250.0;
        (session, identity)
    }

    fn begin_bot_swing(session: &mut crate::Session<Floor>, identity: u32, weapon: Weapon) {
        let tick = session.tick;
        let bot = session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap();
        assert!(matches!(bot.loadout.get_mut(&weapon).unwrap().primary(tick, 0.015, true, false, &mut Vec::new()), PrimaryResult::Fired { .. }));
        bot.pending_melee = Some((tick + (ballistics::MELEE_SMACK_DELAY / 0.015).floor() as u64, crate::PLAYER_IDENTITY, weapon));
        let attack = Attack { phase: AttackPhase::MeleeSwing, attacker: identity, team: bot.team, weapon, target: crate::PLAYER_IDENTITY,
            position: bot.movement.position, eye_position: bot_eye(bot), pitch_degrees: bot.pitch_degrees, yaw_degrees: bot.yaw_degrees };
        let predicted = session.random_state().predicted_presentation;
        session.execute_bot_attack(attack, &mut Vec::new(), &mut Vec::new(), &mut crate::MapPhase::default()).unwrap();
        assert_eq!(session.random_state().predicted_presentation, predicted, "bots do not predict weapon sounds or critical rolls");
    }

    fn finish_bot_swing(session: &mut crate::Session<Floor>) -> crate::Snapshot {
        for _ in 0..14 {
            let frame = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
            assert!(!frame.events.iter().any(|event| matches!(event, crate::Event::MeleeImpact { .. })));
        }
        session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap()
    }

    fn submit_bot_swing(session: &mut crate::Session<Floor>, identity: u32, weapon: Weapon) -> crate::Snapshot {
        begin_bot_swing(session, identity, weapon);
        finish_bot_swing(session)
    }

    #[test]
    fn ten_bot_melee_sources_use_the_same_delayed_trace_damage_and_condition_owner() {
        for (definition, class, weapon, amount, bleed) in [
            (155, PlayerClass::Engineer, Weapon::Wrench, 65, true), (171, PlayerClass::Sniper, Weapon::Kukri, 33, true),
            (214, PlayerClass::Pyro, Weapon::FireAxe, 65, false), (232, PlayerClass::Sniper, Weapon::Kukri, 65, false),
            (310, PlayerClass::Heavy, Weapon::Fists, 85, false), (325, PlayerClass::Scout, Weapon::Bat, 35, true),
            (326, PlayerClass::Pyro, Weapon::FireAxe, 81, false), (355, PlayerClass::Scout, Weapon::Bat, 9, false),
            (401, PlayerClass::Sniper, Weapon::Kukri, 49, false), (416, PlayerClass::Soldier, Weapon::Shovel, 65, false),
        ] {
            let (mut session, identity) = equipped_bot_melee(definition, class, weapon);
            let hit = submit_bot_swing(&mut session, identity, weapon);
            assert_eq!(hit.health, (300 - amount) as f32, "definition={definition}");
            assert_eq!(session.conditions.contains(crate::Condition::Bleeding), bleed);
            assert_eq!(session.conditions.contains(crate::Condition::MarkedForDeath), definition == 355);
            assert!(hit.events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { attacker, victim: crate::PLAYER_IDENTITY, amount: actual, .. } if *attacker == identity && *actual == amount)));
        }
    }

    #[test]
    fn bot_melee_kill_healing_and_active_speed_follow_the_equipped_provider() {
        for (definition, class, weapon, before, after) in [(214, PlayerClass::Pyro, Weapon::FireAxe, 100, 125), (310, PlayerClass::Heavy, Weapon::Fists, 200, 250)] {
            let (mut session, identity) = equipped_bot_melee(definition, class, weapon);
            session.health = 20;
            let bot = session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap(); bot.health.current = before;
            if definition == 214 {
                assert_eq!(bot_movement_policy(bot).maximum_speed, 345.0);
                bot.active_weapon = Some(Weapon::Flamethrower);
                assert_eq!(bot_movement_policy(bot).maximum_speed, 300.0);
                bot.active_weapon = Some(weapon);
            }
            submit_bot_swing(&mut session, identity, weapon);
            assert_eq!(session.bots.as_ref().unwrap().health(identity), Some(after));
        }
    }

    #[test]
    fn bot_basher_misses_and_holsters_use_the_live_delayed_swing_not_the_old_target() {
        let (mut session, identity) = equipped_bot_melee(325, PlayerClass::Scout, Weapon::Bat);
        session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap().movement.position[0] = 60.0;
        begin_bot_swing(&mut session, identity, Weapon::Bat);
        session.movement.position[0] = 500.0;
        finish_bot_swing(&mut session);
        let world = session.bots.as_ref().unwrap();
        assert_eq!(world.health(identity), Some(107));
        assert!(world.has_condition(identity, ConditionId::BLEEDING));
        assert_eq!(world.bots[&identity].movement.velocity[2], 157.5);
        let (mut session, identity) = equipped_bot_melee(325, PlayerClass::Scout, Weapon::Bat);
        begin_bot_swing(&mut session, identity, Weapon::Bat);
        let bot = session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap();
        select_weapon(bot, None, session.tick, 0.015);
        assert!(bot.pending_melee.is_none());
        bot.active_weapon = Some(Weapon::Bat);
        finish_bot_swing(&mut session);
        assert_eq!(session.health, 300);
        assert_eq!(session.bots.as_ref().unwrap().health(identity), Some(125));
    }

    #[test]
    fn bot_melee_impact_uses_live_health_airborne_state_and_mark_conversion() {
        let (mut session, identity) = equipped_bot_melee(401, PlayerClass::Sniper, Weapon::Kukri);
        begin_bot_swing(&mut session, identity, Weapon::Kukri);
        session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap().health.current = 62;
        assert_eq!(finish_bot_swing(&mut session).health, 219.0);
        let (mut session, identity) = equipped_bot_melee(416, PlayerClass::Soldier, Weapon::Shovel);
        begin_bot_swing(&mut session, identity, Weapon::Shovel);
        session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap().conditions.add(ConditionId::BLAST_JUMPING, crate::condition::ConditionDuration::Permanent, None, true, false).unwrap();
        assert_eq!(finish_bot_swing(&mut session).health, 105.0);
        let (mut session, identity) = equipped_bot_melee(355, PlayerClass::Scout, Weapon::Bat);
        assert_eq!(submit_bot_swing(&mut session, identity, Weapon::Bat).health, 291.0);
        for _ in 0..40 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        session.movement.position = [0.0;3]; session.movement.velocity = [0.0;3];
        assert_eq!(submit_bot_swing(&mut session, identity, Weapon::Bat).health, 265.0);
    }

    #[test]
    fn bot_back_scratcher_pack_scaling_and_full_health_bleed_cleansing_are_passive() {
        let (mut session, identity) = equipped_bot_melee(326, PlayerClass::Pyro, Weapon::FireAxe);
        let world = session.bots.as_mut().unwrap();
        let bot = world.bots.get_mut(&identity).unwrap();
        bot.active_weapon = Some(Weapon::Flamethrower); bot.health.current = 100;
        let definition = crate::pickup::map_pickup_definition(b"item_healthkit_small").unwrap();
        assert_eq!(world.grant_pickup(identity, definition), Some(52));
        assert_eq!(world.health(identity), Some(152));
        let bot = world.bots.get_mut(&identity).unwrap(); bot.health.current = 175;
        bot.conditions.add(ConditionId::BLEEDING, crate::condition::ConditionDuration::Permanent, None, true, false).unwrap();
        assert_eq!(world.grant_pickup(identity, definition), Some(0));
        assert!(!world.has_condition(identity, ConditionId::BLEEDING));
    }

    #[test]
    fn ten_equipped_melee_unlocks_resolve_real_session_swing_damage_and_bleed() {
        for (definition, class, weapon, expected, bleed) in [
            (155, PlayerClass::Engineer, Weapon::Wrench, 65, true),
            (171, PlayerClass::Sniper, Weapon::Kukri, 33, true),
            (214, PlayerClass::Pyro, Weapon::FireAxe, 65, false),
            (232, PlayerClass::Sniper, Weapon::Kukri, 65, false),
            (310, PlayerClass::Heavy, Weapon::Fists, 85, false),
            (325, PlayerClass::Scout, Weapon::Bat, 35, true),
            (326, PlayerClass::Pyro, Weapon::FireAxe, 81, false),
            (355, PlayerClass::Scout, Weapon::Bat, 9, false),
            (401, PlayerClass::Sniper, Weapon::Kukri, 49, false),
            (416, PlayerClass::Soldier, Weapon::Shovel, 65, false),
        ] {
            let (mut session, target) = melee_session(definition, class, weapon);
            assert_eq!(session.equipped_weapon_definition(crate::PLAYER_IDENTITY, weapon), Some(definition));
            let hit = melee_swing(&mut session);
            assert!(hit.events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { victim, amount, .. } if *victim == target && *amount == expected)), "item={definition}: {:?}", hit.events);
            assert_eq!(session.bots.as_ref().unwrap().has_condition(target, ConditionId::BLEEDING), bleed);
            let frame = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
            assert_eq!(session.bots.as_ref().unwrap().health(target), Some(300 - expected as i32 - if bleed { 4 } else { 0 }));
            assert_eq!(frame.events.iter().filter(|event| matches!(event, crate::Event::PlayerDamaged { custom: 34, .. })).count(), usize::from(bleed));
            let persisted = session.equipment.persist();
            assert_eq!(crate::equipment::Equipment::restore(&persisted).unwrap().weapon_definition(class, weapon), Some(definition));
        }
    }

    #[test]
    fn melee_holster_cancels_pending_bash_and_active_powerjack_provision() {
        let (mut session, target) = melee_session(325, PlayerClass::Scout, Weapon::Bat);
        session.advance(crate::Command { fire: true, nextbot_stop: true, ..Default::default() }).unwrap();
        session.advance(crate::Command { select_weapon: Some(Weapon::Pistol), nextbot_stop: true, ..Default::default() }).unwrap();
        for _ in 0..20 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(300));
        assert!(session.melee.bleeds.is_empty());
        let (mut session, target) = melee_session(214, PlayerClass::Pyro, Weapon::FireAxe);
        assert_eq!(session.equipped_player_attribute(crate::PLAYER_IDENTITY, "mult_player_movespeed", 300.0), 345.0);
        let mut incoming = direct_damage(Weapon::Pistol, crate::PLAYER_IDENTITY, 10.0); incoming.attacker = target;
        assert_eq!(session.apply_actor_damage(incoming, PlayerTeam::Blue, &mut Vec::new()).unwrap().unwrap().health_damage, 12);
        session.advance(crate::Command { select_weapon: Some(Weapon::Shotgun), nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(session.equipped_player_attribute(crate::PLAYER_IDENTITY, "mult_player_movespeed", 300.0), 300.0);
        assert_eq!(session.apply_actor_damage(incoming, PlayerTeam::Blue, &mut Vec::new()).unwrap().unwrap().health_damage, 10);
    }

    #[test]
    fn basher_clean_miss_self_damage_bleed_push_and_respawn_cleanup_are_live() {
        let (mut session, target) = melee_session(325, PlayerClass::Scout, Weapon::Bat);
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().movement.position = [500.0, 0.0, 0.0];
        let hit = melee_swing(&mut session);
        assert_eq!(hit.health, 107.0);
        assert!(session.conditions.contains(crate::Condition::Bleeding));
        assert_eq!(session.movement.velocity[2], 157.5);
        let bleed = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(bleed.health, 103.0);
        session.advance(crate::Command { respawn: true, nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(session.health, 125);
        assert!(!session.conditions.contains(crate::Condition::Bleeding));
        assert!(!session.melee.bleeds.contains_key(&crate::PLAYER_IDENTITY));
        assert_eq!(session.equipment.weapon_definition(PlayerClass::Scout, Weapon::Bat), Some(325));
    }

    #[test]
    fn fan_marks_after_first_damage_and_bushwacka_promotes_minicrits() {
        for (definition, class, weapon, first, promoted) in [(355, PlayerClass::Scout, Weapon::Bat, 9, 26), (232, PlayerClass::Sniper, Weapon::Kukri, 65, 195)] {
            let (mut session, target) = melee_session(definition, class, weapon);
            melee_swing(&mut session);
            assert_eq!(session.bots.as_ref().unwrap().health(target), Some(300 - first));
            if definition == 355 { assert!(session.bots.as_ref().unwrap().has_condition(target, ConditionId::MARKED_FOR_DEATH)); }
            else { session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().conditions.add(ConditionId::URINE, crate::condition::ConditionDuration::Finite(10.0), None, true, false).unwrap(); }
            for _ in 0..60 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
            let hit = melee_swing(&mut session);
            assert!(hit.events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { amount, crit: CritKind::Full, .. } if *amount == promoted)));
        }
    }

    #[test]
    fn warriors_spirit_heals_on_kill_and_provides_vulnerability_only_while_active() {
        let (mut session, target) = melee_session(310, PlayerClass::Heavy, Weapon::Fists);
        session.health = 200;
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().health.current = 40;
        melee_swing(&mut session);
        assert_eq!(session.health, 250);
        assert!((session.equipped_player_attribute(crate::PLAYER_IDENTITY, "mult_dmgtaken", 1.0) - 1.3).abs() < 0.00001);
        session.advance(crate::Command { select_weapon: Some(Weapon::HeavyShotgun), nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(session.equipped_player_attribute(crate::PLAYER_IDENTITY, "mult_dmgtaken", 1.0), 1.0);
    }

    #[test]
    fn equipped_fists_keep_left_right_and_critical_swing_activities_with_one_smack() {
        for (secondary, critical, activity) in [
            (false, false, crate::weapon::WeaponActivity::FistLeft),
            (true, false, crate::weapon::WeaponActivity::FistRight),
            (true, true, crate::weapon::WeaponActivity::MeleeCritical),
        ] {
            let (mut session, target) = melee_session(310, PlayerClass::Heavy, Weapon::Fists);
            if critical { session.conditions.words[0] |= 1 << crate::condition::ConditionId::CRIT_BOOSTED.value(); }
            let swing_tick = session.tick;
            session.advance(crate::Command { fire: !secondary, detonate: secondary, nextbot_stop: true, ..Default::default() }).unwrap();
            assert!(session.activity_events.iter().any(|event| event.tick == swing_tick && event.activity == activity));
            let mut smacks = 0;
            for _ in 0..20 {
                let frame = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
                smacks += frame.events.iter().filter(|event| matches!(event, crate::Event::MeleeImpact { target: Some(victim), .. } if *victim == target)).count();
            }
            assert_eq!(smacks, 1);
            assert_eq!(session.bots.as_ref().unwrap().health(target), Some(if critical { 46 } else { 215 }));
        }
    }

    #[test]
    fn market_gardener_checks_blast_jump_at_impact_and_landing_clears_it() {
        for blast in [false, true] {
            let (mut session, target) = melee_session(416, PlayerClass::Soldier, Weapon::Shovel);
            session.movement.position[2] = 100.0;
            session.movement.ground = None;
            session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().movement.position[2] = 100.0;
            if blast { session.conditions.insert(crate::Condition::BlastJumping); }
            let hit = melee_swing(&mut session);
            assert!(hit.events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { amount, crit, .. } if *amount == if blast { 195 } else { 65 } && *crit == if blast { CritKind::Full } else { CritKind::None })));
            assert!((session.loadout[&Weapon::Shovel].profile().fire_delay - 0.96).abs() < 0.00001);
        }
        let (mut session, target) = melee_session(416, PlayerClass::Soldier, Weapon::Shovel);
        session.movement.position[2] = 100.0; session.movement.ground = None;
        session.conditions.insert(crate::Condition::BlastJumping);
        session.advance(crate::Command { fire: true, nextbot_stop: true, ..Default::default() }).unwrap();
        session.movement.position[2] = 0.0; session.movement.velocity = [0.0; 3];
        for _ in 0..14 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        assert!(!session.conditions.contains(crate::Condition::BlastJumping));
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(235));
    }

    #[test]
    fn shahanshah_uses_smack_health_and_powerjack_heals_only_a_real_kill() {
        let (mut session, target) = melee_session(401, PlayerClass::Sniper, Weapon::Kukri);
        session.advance(crate::Command { fire: true, nextbot_stop: true, ..Default::default() }).unwrap();
        session.health = 62;
        for _ in 0..14 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(219));
        for (before, target_health, after) in [(100, 300, 100), (100, 40, 125), (160, 40, 175)] {
            let (mut session, target) = melee_session(214, PlayerClass::Pyro, Weapon::FireAxe);
            session.health = before;
            session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().health.current = target_health;
            melee_swing(&mut session);
            assert_eq!(session.health, after);
        }
    }

    #[test]
    fn back_scratcher_scales_the_authored_pack_before_takehealth_truncation() {
        let graph = playsrc_entity::parse(b"{\"classname\"\"item_healthkit_small\"\"origin\"\"0 0 0\"}\0", playsrc_entity::Limits::default()).unwrap();
        let map = crate::MapRuntime::compile(&graph, 0.015, 7, Vec::new()).unwrap();
        let (mut session, _) = melee_session_map(326, PlayerClass::Pyro, Weapon::FireAxe, map);
        session.health = 100;
        session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(session.health, 152, "ceil(175*.2)*1.5=52.5, TakeHealth truncates the sum");
        assert!(!session.map.pickups()[0].available);
        session.advance(crate::Command { select_weapon: Some(Weapon::Shotgun), nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(session.equipped_player_attribute(crate::PLAYER_IDENTITY, "mult_health_fromhealers", 24.0), 6.0);
    }

    #[test]
    fn southern_hospitality_fire_vulnerability_is_passive_and_does_not_affect_bullets() {
        let (mut session, target) = melee_session(155, PlayerClass::Engineer, Weapon::Wrench);
        for held in [Weapon::Wrench, Weapon::EngineerShotgun] {
            session.advance(crate::Command { select_weapon: Some(held), nextbot_stop: true, ..Default::default() }).unwrap();
            let mut fire = direct_damage(Weapon::Flamethrower, crate::PLAYER_IDENTITY, 10.0); fire.attacker = target;
            assert_eq!(session.apply_actor_damage(fire, PlayerTeam::Blue, &mut Vec::new()).unwrap().unwrap().health_damage, 12);
            let mut bullet = direct_damage(Weapon::Pistol, crate::PLAYER_IDENTITY, 10.0); bullet.attacker = target;
            assert_eq!(session.apply_actor_damage(bullet, PlayerTeam::Blue, &mut Vec::new()).unwrap().unwrap().health_damage, 10);
        }
    }

    #[test]
    fn delayed_bleed_does_not_acquire_a_replacement_fans_crit_promotion() {
        let (mut session, target) = melee_session(325, PlayerClass::Scout, Weapon::Bat);
        melee_swing(&mut session);
        let source = session.melee.bleeds[&target][0].source_weapon.unwrap();
        session.equip_item(PlayerClass::Scout, crate::schema::LoadoutPosition::Melee, Some(355)).unwrap();
        session.advance(crate::Command { respawn: true, nextbot_stop: true, ..Default::default() }).unwrap();
        assert!(!session.source_weapon_is_live(source, Weapon::Bat));
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().conditions.add(ConditionId::MARKED_FOR_DEATH, crate::condition::ConditionDuration::Finite(15.0), Some(crate::PLAYER_IDENTITY), true, false).unwrap();
        let mut amounts = Vec::new();
        for _ in 0..40 {
            let snapshot = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
            amounts.extend(snapshot.events.into_iter().filter_map(|event| match event {
                crate::Event::PlayerDamaged { custom: 34, amount, .. } => Some(amount), _ => None,
            }));
        }
        assert_eq!(amounts, [5], "expired weapon handle cannot query the new Fan's attributes");
    }

    #[test]
    fn cleansing_bleed_does_not_turn_a_mark_timer_permanent() {
        let graph = playsrc_entity::parse(b"{\"classname\"\"item_healthkit_small\"\"origin\"\"0 0 0\"}\0", playsrc_entity::Limits::default()).unwrap();
        let map = crate::MapRuntime::compile(&graph, 0.015, 8, Vec::new()).unwrap();
        let (mut session, target) = melee_session_map(325, PlayerClass::Scout, Weapon::Bat, map);
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().movement.position = [500.0, 0.0, 0.0];
        session.add_melee_condition(crate::PLAYER_IDENTITY, crate::Condition::MarkedForDeath, 3.6, target);
        melee_swing(&mut session);
        session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
        assert_eq!(session.health, 125);
        assert!(!session.conditions.contains(crate::Condition::Bleeding));
        assert!(!session.melee.bleeds.contains_key(&crate::PLAYER_IDENTITY));
        assert!(session.melee.local_mark_remaining.is_some());
        for _ in 0..245 { session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap(); }
        assert!(!session.conditions.contains(crate::Condition::MarkedForDeath));
    }

    #[test]
    fn critical_feedback_retains_full_and_mini_kinds_and_uses_only_client_sound_randomness() {
        for (crit, name) in [(CritKind::None, None), (CritKind::Full, Some("TFPlayer.CritHit")), (CritKind::Mini, Some("TFPlayer.CritHitMini"))] {
            let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
            session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
            session.advance(crate::Command { nextbot_stop: true, bot_request: Some(Request {
                operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy), team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal,
            }), ..Default::default() }).unwrap();
            let victim = session.bots.as_ref().unwrap().snapshots()[0].identity;
            let mut input = direct_damage(Weapon::Bat, victim, 10.0); input.crit = crit;
            let authority = session.random_state().authority;
            let mut events = Vec::new();
            session.apply_actor_damage(input, PlayerTeam::Red, &mut events).unwrap();
            assert_eq!(session.random_state().authority, authority);
            assert!(events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { crit: actual, .. } if *actual == crit)));
            let feedback: Vec<_> = session.audio_events().iter().filter(|event| event.definition.identity().starts_with("TFPlayer.CritHit")).collect();
            assert_eq!(feedback.len(), usize::from(name.is_some()));
            if let Some(name) = name {
                assert_eq!(feedback[0].definition.identity(), name);
                assert_eq!(feedback[0].source_identity, crate::PLAYER_IDENTITY);
                assert_eq!(feedback[0].owner_identity, Some(crate::PLAYER_IDENTITY));
            }
        }
    }

    #[test]
    fn finite_conditions_advance_once_per_session_tick_when_ai_is_stopped() {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
        session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
        session.advance(crate::Command { nextbot_stop: true, bot_request: Some(Request {
            operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy),
            team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal,
        }), ..Default::default() }).unwrap();
        let target = session.bots.as_ref().unwrap().snapshots()[0].identity;
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().conditions.add(
            ConditionId::MARKED_FOR_DEATH, crate::condition::ConditionDuration::Finite(0.04), None, true, false).unwrap();
        for expected in [true, true, false] {
            let frame = session.advance(crate::Command { nextbot_stop: true, ..Default::default() }).unwrap();
            assert_eq!(frame.bots[0].conditions[0] & (1 << 30) != 0, expected);
        }
    }

    #[test]
    fn native_actor_damage_resolves_typed_crit_range_and_resistance_and_records_once() {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
        session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
        session.advance(crate::Command { nextbot_stop: true, bot_request: Some(Request {
            operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy),
            team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal,
        }), ..Default::default() }).unwrap();
        let target = session.bots.as_ref().unwrap().snapshots()[0].identity;
        let mut hit = direct_damage(Weapon::RocketLauncher, target, 90.0);
        hit.crit = CritKind::Full;
        hit.range_multiplier = 0.5;
        hit.modifiers.critical_bonus_taken = 0.5;
        hit.force = [1.0, 2.0, 3.0];
        let mut events = Vec::new();
        let result = session.apply_actor_damage(hit, PlayerTeam::Red, &mut events).unwrap().unwrap();
        assert_eq!(result.pre_resistance_base_damage, 45.0);
        assert_eq!(result.pre_resistance_bonus_damage, 225.0);
        assert_eq!(result.final_damage, 157.5);
        assert_eq!(result.health_damage, 158);
        assert_eq!(result.force, [1.0, 2.0, 3.0]);
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(142));
        let mut observed = damage::CritState::default();
        session.critical_history.supply_observed_damage(&mut observed);
        assert_eq!((observed.total_ranged_damage, observed.random_ranged_crit_damage), (158, 158));
        assert_eq!(events.iter().filter(|event| matches!(event, crate::Event::PlayerDamaged { .. })).count(), 1);
    }

    #[test]
    fn native_self_hit_is_not_friendly_fire_and_bleeding_is_slash_without_reapplication() {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
        session.advance(crate::Command { select_class: Some(PlayerClass::Scout), ..Default::default() }).unwrap();
        let mut hit = direct_damage(Weapon::Bat, crate::PLAYER_IDENTITY, 17.5);
        hit.force = [0.0, 0.0, 100.0];
        let result = session.apply_actor_damage(hit, PlayerTeam::Red, &mut Vec::new()).unwrap().unwrap();
        assert!(result.admitted);
        assert_eq!(session.health, 107);
        assert_eq!(result.force, hit.force);
        let mut observed = damage::CritState::default();
        session.critical_history.supply_observed_damage(&mut observed);
        assert_eq!(observed.total_ranged_damage, 0);
        hit.amount = 4.0;
        hit.custom = CustomDamage::Bleeding;
        hit.force = [0.0; 3];
        assert_eq!(hit.normalized_damage_type().source_bits(CritKind::None), 4);
        let result = session.apply_actor_damage(hit, PlayerTeam::Red, &mut Vec::new()).unwrap().unwrap();
        assert_eq!(result.health_damage, 4);
        assert!(!session.conditions.contains(crate::Condition::Bleeding));
    }

    #[test]
    fn native_damage_denial_retains_force_and_does_not_record_history() {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
        session.conditions.insert(crate::Condition::Invulnerable);
        let mut hit = direct_damage(Weapon::RocketLauncher, crate::PLAYER_IDENTITY, 90.0);
        hit.attacker = 0;
        hit.force = [5.0, 6.0, 7.0];
        let mut events = Vec::new();
        let result = session.apply_actor_damage(hit, PlayerTeam::Unassigned, &mut events).unwrap().unwrap();
        assert!(!result.admitted);
        assert_eq!(result.denial, Some(damage::DamageDenial::Invulnerable));
        assert_eq!(result.force, hit.force);
        assert_eq!(session.health, 200);
        assert!(events.is_empty());
    }

    #[test]
    fn native_bot_provider_hooks_and_weapon_source_use_the_same_runtime_owner() {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
        session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
        projectile_step(&mut session, crate::Command { bot_request: Some(Request { operation: Operation::Add,
            count: 1, class: Some(PlayerClass::Soldier), team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal }), ..Default::default() });
        let owner = session.bots.as_ref().unwrap().snapshots()[0].identity;
        assert!(session.bots.as_ref().unwrap().bots[&owner].equipment.is_none(), "the default bot roster adds no per-bot equipment graph");
        assert_eq!(session.equipped_weapon_attribute(owner, Weapon::RocketLauncher, "mult_dmg", 90.0), 90.0);
        let original = session.weapon_source(owner, Weapon::RocketLauncher).unwrap();
        session.equip_bot_item(owner, crate::schema::LoadoutPosition::Primary, Some(127)).unwrap();
        assert!(session.source_weapon_is_live(original, Weapon::RocketLauncher));
        assert!(session.bots.as_mut().unwrap().regenerate(owner, session.tick));
        assert!(!session.source_weapon_is_live(original, Weapon::RocketLauncher));
        let source = session.weapon_source(owner, Weapon::DirectHit).unwrap();
        assert_eq!(source.owner, owner);
        assert_eq!(source.definition_index, 127);
        assert_eq!(session.source_weapon_attribute(Some(source), Weapon::DirectHit, "mult_dmg", 90.0), 112.5);
        assert_eq!(session.equipped_weapon_attribute(crate::PLAYER_IDENTITY, Weapon::RocketLauncher, "mult_dmg", 90.0), 90.0);
        session.bots.as_mut().unwrap().weapon_runtime_mut(owner, Weapon::DirectHit).unwrap().generation += 1;
        assert_eq!(session.source_weapon_attribute(Some(source), Weapon::DirectHit, "mult_dmg", 90.0), 90.0);
        projectile_step(&mut session, crate::Command { bot_request: Some(Request { operation: Operation::KickAll,
            count: 0, class: None, team: None, difficulty: Difficulty::Normal }), ..Default::default() });
        assert!(!session.bots.as_ref().unwrap().contains(owner));
    }

    #[test]
    fn bot_health_uses_equipped_passive_and_active_provider_rates_without_changing_healer_attribution() {
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(7).unwrap();
        world.apply(Request { operation: Operation::Add, count: 1, class: Some(PlayerClass::Pyro),
            team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal }, PlayerTeam::Red, PlayerClass::Soldier, &mut random).unwrap();
        let identity = world.snapshots()[0].identity;
        let bot = world.bots.get_mut(&identity).unwrap();
        let selected = crate::equipment::Equipment::default();
        let mut active = selected.equipped_items(PlayerClass::Pyro);
        // Registered metadata is a fixture, not an inventory capability grant.
        *active.iter_mut().find(|item| item.slot == crate::schema::LoadoutPosition::Melee).unwrap() = crate::equipment::EquippedItem {
            item_id: 327, definition_index: 326, quality: 6, style: 0, slot: crate::schema::LoadoutPosition::Melee, attributes: Vec::new(),
        };
        let providers = crate::equipment::AttributeProviders::new(&active, PlayerClass::Pyro);
        bot.equipment = Some(Box::new(BotEquipment { selected, active, providers, class: PlayerClass::Pyro, next_generation: 1 }));
        bot.active_weapon = Some(Weapon::Flamethrower);
        bot.health.current = 50;
        bot.health.last_damage_time = 0.0;
        bot.health.start_healing(crate::health::Healer { identity: 999, scorer: 999, rate: 24.0, overheal_multiplier: 1.0,
            overheal_decay_multiplier: 1.0, dispenser: false, accumulated: 0.0, healed_last_second: 0.0,
            overheal_fill_rate_multiplier: 1.0, healing_from_medics_multiplier: 1.0 }, &mut bot.conditions).unwrap();
        let mut expected_health = bot.health.clone();
        let mut expected_conditions = bot.conditions.clone();
        assert_eq!(world.equipped_player_attribute(identity, "mult_health_fromhealers", 1.0), 0.25);
        assert_eq!(world.equipped_weapon_attribute(identity, Weapon::Flamethrower, "mult_healing_received", 1.0), 1.0);
        for tick in 1..=100 {
            let now = tick as f32 * 0.015;
            expected_health.advance(now, 0.015, HealthConfiguration::default(), 0.25, 1.0, 1.0, &mut expected_conditions).unwrap();
            world.advance_health(now).unwrap();
        }
        assert_eq!(world.bots[&identity].health, expected_health);
        assert_eq!(world.bots[&identity].health.healers[0].scorer, 999);
        assert!(world.bots[&identity].health.current < 60);
    }

    #[test]
    fn bot_equipment_is_lazy_validated_and_dropped_with_its_actor() {
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(7).unwrap();
        world.apply(Request { operation: Operation::Add, count: 1, class: Some(PlayerClass::Soldier),
            team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal }, PlayerTeam::Red, PlayerClass::Soldier, &mut random).unwrap();
        let identity = world.snapshots()[0].identity;
        let generation = world.weapon_runtime(identity, Weapon::RocketLauncher).unwrap().generation;
        assert!(!world.equip_item(identity, crate::schema::LoadoutPosition::Primary, Some(18)).unwrap());
        assert!(world.bots[&identity].equipment.is_none());
        assert_eq!(world.equip_item(identity, crate::schema::LoadoutPosition::Primary, Some(u32::MAX)), Err(crate::equipment::EquipmentError::UnsupportedItem));
        assert!(world.bots[&identity].equipment.is_none());
        assert!(world.equip_item(identity, crate::schema::LoadoutPosition::Misc, Some(378)).unwrap());
        assert_eq!(world.snapshots()[0].equipped_items.len(), 1);
        assert_eq!(world.snapshots()[0].equipped_items[0].definition_index, 378);
        assert_eq!(world.weapon_runtime(identity, Weapon::RocketLauncher).unwrap().generation, generation);
        assert_eq!(world.equipped_player_attribute(identity, "mult_health_fromhealers", 1.0), 1.0);
        world.apply(Request { operation: Operation::KickAll, count: 0, class: None, team: None, difficulty: Difficulty::Normal },
            PlayerTeam::Red, PlayerClass::Soldier, &mut random).unwrap();
        assert!(world.bots.is_empty());
    }

    #[test]
    fn minigun_bot_transitions_enable_aiming_cap_and_spunup_resistance() {
        use crate::weapon::MinigunState;
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(7).unwrap();
        world.apply(Request { operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy),
            team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal }, PlayerTeam::Red, PlayerClass::Soldier, &mut random).unwrap();
        let identity = world.snapshots()[0].identity;
        let bot = world.bots.get_mut(&identity).unwrap();
        let selected = crate::equipment::Equipment::default();
        let mut active = selected.equipped_items(PlayerClass::Heavy);
        *active.iter_mut().find(|item| item.slot == crate::schema::LoadoutPosition::Primary).unwrap() = crate::equipment::EquippedItem {
            item_id: 313, definition_index: 312, quality: 6, style: 0, slot: crate::schema::LoadoutPosition::Primary, attributes: Vec::new(),
        };
        let providers = crate::equipment::AttributeProviders::new(&active, PlayerClass::Heavy);
        bot.equipment = Some(Box::new(BotEquipment { selected, active, providers, class: PlayerClass::Heavy, next_generation: 1 }));
        apply_minigun_aiming(&mut bot.conditions, MinigunState::Idle, MinigunState::Starting).unwrap();
        apply_minigun_aiming(&mut bot.conditions, MinigunState::Starting, MinigunState::Spinning).unwrap();
        assert!(bot.conditions.contains(ConditionId::AIMING));
        let mut policy = MovementPolicy { class: bot.class, modifiers: MovementModifiers::default() }.resolve();
        apply_aiming_policy(bot, &mut policy);
        assert_eq!(policy.maximum_speed, 44.0);
        assert!(!policy.allow_jump);
        let mut session = crate::Session::new(Floor, [0.0; 3], crate::MapRuntime::empty(0.015));
        session.bots = Some(world);
        assert_eq!(session.equipped_victim_damage_modifiers(identity, DamageModifiers::default()).spunup_taken, 0.8);
        let bot = session.bots.as_mut().unwrap().bots.get_mut(&identity).unwrap();
        apply_minigun_aiming(&mut bot.conditions, MinigunState::Spinning, MinigunState::Idle).unwrap();
        assert!(!bot.conditions.contains(ConditionId::AIMING));
        assert_eq!(session.equipped_victim_damage_modifiers(identity, DamageModifiers::default()).spunup_taken, 1.0);
    }

    #[test]
    fn bot_hitscan_uses_the_same_item_rules_and_damage_transaction_as_the_local_player() {
        use crate::schema::LoadoutPosition::{Primary, Secondary};
        for (definition, class, slot, weapon, headshot) in [
            (45, PlayerClass::Scout, Primary, Weapon::Scattergun, false),
            (1103, PlayerClass::Scout, Primary, Weapon::Scattergun, false),
            (41, PlayerClass::Heavy, Primary, Weapon::Minigun, false),
            (61, PlayerClass::Spy, Secondary, Weapon::Revolver, true),
            (460, PlayerClass::Spy, Secondary, Weapon::Revolver, false),
            (220, PlayerClass::Scout, Primary, Weapon::HandgunScoutPrimary, false),
            (402, PlayerClass::Sniper, Primary, Weapon::SniperRifle, true),
            (415, PlayerClass::Soldier, Secondary, Weapon::Shotgun, false),
        ] {
            let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
            let mut random = UniformRandomStream::from_seed(7).unwrap();
            world.apply(Request { operation: Operation::Add, count: 1, class: Some(class), team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal }, PlayerTeam::Red, PlayerClass::Soldier, &mut random).unwrap();
            let identity = world.snapshots()[0].identity;
            world.equip_item(identity, slot, Some(definition)).unwrap();
            let bot = world.bots.get_mut(&identity).unwrap();
            apply_bot_equipment(bot);
            bot.active_weapon = Some(weapon); bot.movement.position = [-64.0, 0.0, 0.0];
            if definition == 460 { bot.conditions.add(ConditionId::DISGUISED, crate::condition::ConditionDuration::Permanent, None, true, false).unwrap(); }
            if definition == 402 { bot.conditions.add(ConditionId::ZOOMED, crate::condition::ConditionDuration::Permanent, None, true, false).unwrap(); }
            let state = bot.loadout.get_mut(&weapon).unwrap();
            state.charge_begin_tick = Some(0);
            if weapon == Weapon::Minigun { state.minigun_state = crate::weapon::MinigunState::Firing; state.spin_begin_tick = Some(0); state.firing_begin_tick = Some(0); }
            let mut session = crate::Session::new(Floor, [0.0; 3], crate::MapRuntime::empty(0.015));
            session.bots = Some(world); session.tick = 200;
            session.movement.absolute_view_angles = [0.0; 3];
            if definition == 415 { session.conditions.insert(crate::Condition::BlastJumping); session.movement.ground = None; }
            if definition == 402 { session.health = 100; }
            if headshot { session.set_posed_player_hitboxes(vec![crate::PosedPlayerHitbox {
                entity: crate::PLAYER_IDENTITY, team: PlayerTeam::Red, hitbox: 0, group: 1, bone: 0, physics_bone: 0,
                bone_contents: 0x4000_0000, minimum: [-4.0; 3], maximum: [4.0; 3], origin: [0.0; 3],
                bone_to_world: [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,41.0],
            }]); }
            let attack = Attack { phase: AttackPhase::Fire, attacker: identity, team: PlayerTeam::Blue, weapon, target: crate::PLAYER_IDENTITY,
                position: [-64.0, 0.0, 0.0], eye_position: [-64.0, 0.0, 41.0], pitch_degrees: 0.0, yaw_degrees: 0.0 };
            let mut events = Vec::new();
            session.fire_hitscan(weapon, 0.0, 0.0, &mut events, Some(attack)).unwrap();
            assert!(session.random_draws.iter().all(|draw| draw.context == crate::RandomContext::Authority), "bots have no predicted bullet or sound pass");
            assert!(session.health < if definition == 402 { 100 } else { 200 }, "{definition}: {events:?}");
            assert_eq!(session.bots.as_ref().unwrap().weapon_runtime(identity, weapon).unwrap().hitscan.consecutive_shots, 1);
            if matches!(definition, 1103 | 415) { assert!(events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { crit: CritKind::Mini, .. })), "{definition}"); }
            if definition == 45 { assert!(session.weapon_knockback); assert!(session.movement.velocity[2] > 268.0); }
            if definition == 41 { assert!(session.conditions.contains(crate::Condition::Stunned)); assert!((session.movement_stuns.command(3.0, 100.0, 0.0).0 - 40.0).abs() < 0.001); }
            if definition == 61 { assert!(events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { crit: CritKind::Full, custom: 1, .. }))); }
            if definition == 460 { assert!(!session.actor_condition(identity, ConditionId::DISGUISED)); }
            if definition == 402 { assert_eq!(session.bots.as_ref().unwrap().bots[&identity].decapitations, 1); }
        }
    }

    fn projectile_step(session: &mut crate::Session<Floor>, mut command: crate::Command) -> crate::Snapshot {
        command.nextbot_stop = true;
        assert!(session.physics_requests.is_empty());
        let results = session.rocket_trace_requests.iter().map(|request| {
            let trace = Floor.trace(request.start, request.end, Hull { mins: [0.0; 3], maxs: [0.0; 3] }, request.mask).unwrap();
            crate::RocketTraceResult {
                projectile: request.projectile, tick: session.tick,
                end: trace.end, solid: trace.fraction < 1.0 || trace.start_solid,
                sky: false, normal: trace.normal, direct_target: None,
            }
        }).collect::<Vec<_>>();
        session.advance_with_external(command, &[], &results, None).unwrap_or_else(|error| {
            panic!("tick={} weapon={:?} command={command:?} results={results:?}: {error:?}", session.tick, session.weapon)
        })
    }

    fn projectile_session(definition: u32, weapon: Weapon) -> (crate::Session<Floor>, u32) {
        let mut session = crate::Session::new(Floor, [0.0, 0.0, 1.0], crate::MapRuntime::empty(0.015));
        let class = if weapon.is_rocket_launcher() { PlayerClass::Soldier } else { PlayerClass::Pyro };
        projectile_step(&mut session, crate::Command { select_class: Some(class), ..Default::default() });
        session.equip_item(class, if weapon.is_rocket_launcher() { crate::schema::LoadoutPosition::Primary }
            else { crate::schema::LoadoutPosition::Secondary }, Some(definition)).unwrap();
        projectile_step(&mut session, crate::Command { respawn: true, ..Default::default() });
        projectile_step(&mut session, crate::Command { select_weapon: Some(weapon), ..Default::default() });
        session.configure_navigation(fixture_mesh(), &fixture_graph()).unwrap();
        projectile_step(&mut session, crate::Command { bot_request: Some(Request {
            operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy),
            team: Some(PlayerTeam::Blue), difficulty: Difficulty::Normal,
        }), ..Default::default() });
        let identity = session.bots.as_ref().unwrap().snapshots()[0].identity;
        session.bots.as_mut().unwrap().teleport(identity, [200.0, 0.0, 1.0], 0.0, 180.0).unwrap();
        for _ in 0..35 { projectile_step(&mut session, crate::Command::default()); }
        (session, identity)
    }

    #[test]
    fn native_flare_family_launch_impact_and_ammunition_are_item_backed() {
        for (definition, weapon, damage, reserve, expected_clip) in [
            (39, Weapon::FlareGun, 30.0, 16, 0),
            (351, Weapon::Detonator, 22.5, 16, 0),
            (740, Weapon::ScorchShot, 19.5, 16, 0),
            (595, Weapon::Manmelter, 30.0, 32, 20),
        ] {
            let (mut session, target) = projectile_session(definition, weapon);
            let fired = projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
            assert_eq!(fired.projectiles.len(), 1);
            assert_eq!(fired.projectiles[0].kind, crate::ProjectileKind::Flare);
            assert_eq!(fired.projectiles[0].damage, damage);
            let runtime = session.weapon_runtime(weapon).unwrap();
            assert_eq!(runtime.clip, expected_clip);
            assert_eq!(runtime.reserve, reserve - u16::from(weapon != Weapon::Manmelter));
            let gravity = if weapon == Weapon::Manmelter { 0.3 * 1.5 } else { 0.3 };
            assert_eq!(session.projectiles[0].gravity_scale, gravity);
            let mut impact = None;
            for _ in 0..20 {
                let frame = projectile_step(&mut session, crate::Command::default());
                impact = frame.events.into_iter().find(|event| matches!(event, crate::Event::PlayerDamaged { victim, .. } if *victim == target));
                if impact.is_some() { break; }
            }
            assert!(matches!(impact, Some(crate::Event::PlayerDamaged { amount, .. }) if amount == (damage + 0.5) as u32), "{weapon:?}: {impact:?}");
            assert!(session.bots.as_ref().unwrap().combat_targets().find(|victim| victim.identity == target).unwrap().burning);
            if weapon == Weapon::ScorchShot {
                assert_eq!(session.projectiles.len(), 1);
                assert!(session.projectiles[0].flare_debris);
                assert_eq!(session.projectiles[0].direct_target, Some(target));
                assert!(session.projectiles[0].presentation.angular_velocity.iter().all(|value| value.abs() >= 180.0 && value.abs() <= 720.0));
            } else { assert!(session.projectiles.is_empty()); }
        }
    }

    #[test]
    fn native_standard_flare_crits_burning_targets_and_afterburn_keeps_ticking_with_ai_stopped() {
        let (mut session, target) = projectile_session(39, Weapon::FlareGun);
        projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(270));
        let health_after_hit = session.bots.as_ref().unwrap().health(target).unwrap();
        for _ in 0..35 { projectile_step(&mut session, crate::Command::default()); }
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(health_after_hit - 4));
        while session.tick < session.weapon_runtime(Weapon::FlareGun).unwrap().next_primary_tick {
            projectile_step(&mut session, crate::Command::default());
        }
        projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        let mut hit = None;
        while !session.projectiles.is_empty() {
            let frame = projectile_step(&mut session, crate::Command::default());
            hit = frame.events.into_iter().find(|event| matches!(event,
                crate::Event::PlayerDamaged { victim, weapon: Weapon::FlareGun, amount: 90, crit: damage::CritKind::Full, .. } if *victim == target)).or(hit);
        }
        assert!(hit.is_some());
    }

    #[test]
    fn native_detonator_secondary_precedes_primary_and_cancels_queued_flight() {
        let (mut session, target) = projectile_session(351, Weapon::Detonator);
        let both = projectile_step(&mut session, crate::Command { fire: true, detonate: true, ..Default::default() });
        assert!(both.projectiles.is_empty());
        assert_eq!(session.weapon_runtime(Weapon::Detonator).unwrap().reserve, 16);
        projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        let burst = projectile_step(&mut session, crate::Command { detonate: true, ..Default::default() });
        assert!(burst.projectiles.is_empty());
        assert!(session.rocket_trace_requests.is_empty());
        assert_eq!(session.radius_damage_requests[0].radius, 110.0);
        assert_eq!(session.radius_damage_requests[0].self_radius, 100.0);
        assert!(burst.projectile_events.iter().any(|event| event.kind == crate::ProjectileEventKind::Explode));
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(300));
        projectile_step(&mut session, crate::Command::default());
    }

    #[test]
    fn native_black_box_heals_once_and_rocket_jumper_does_not_damage_enemy_players() {
        let (mut black_box, target) = projectile_session(228, Weapon::BlackBox);
        black_box.health = 100;
        projectile_step(&mut black_box, crate::Command { fire: true, ..Default::default() });
        while !black_box.projectiles.is_empty() { projectile_step(&mut black_box, crate::Command::default()); }
        assert_eq!(black_box.health, 120);
        assert!(black_box.bots.as_ref().unwrap().health(target).unwrap() < 300);
        let (mut jumper, target) = projectile_session(237, Weapon::RocketJumper);
        let before = jumper.health;
        projectile_step(&mut jumper, crate::Command { fire: true, ..Default::default() });
        while !jumper.projectiles.is_empty() { projectile_step(&mut jumper, crate::Command::default()); }
        assert_eq!(jumper.bots.as_ref().unwrap().health(target), Some(300));
        assert_eq!(jumper.health, before);
    }

    #[test]
    fn native_radius_healing_precedes_self_damage_and_respects_no_healing_admission() {
        let mut outcomes = Vec::new();
        for denied in [false, true] {
            let (mut session, target) = projectile_session(228, Weapon::BlackBox);
            session.bots.as_mut().unwrap().teleport(target, [70.0, 0.0, 1.0], 0.0, 180.0).unwrap();
            session.health = 60;
            if denied { session.conditions.words[0] |= 1 << 31; }
            projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
            while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
            outcomes.push(session.health);
        }
        assert!(outcomes[0] > 0, "accepted radius healing must save the owner before the self pass: {outcomes:?}");
        assert_eq!(outcomes[1], 0, "NOHEALINGDAMAGEBUFF must not admit the Black Box heal");
    }

    #[test]
    fn native_liberty_and_jumper_preserve_stock_jump_force_with_distinct_health_costs() {
        let mut outcomes = Vec::new();
        for (definition, weapon) in [(18, Weapon::RocketLauncher), (414, Weapon::LibertyLauncher), (237, Weapon::RocketJumper)] {
            let (mut session, _) = projectile_session(definition, weapon);
            projectile_step(&mut session, crate::Command { fire: true, pitch_degrees: 90.0, ..Default::default() });
            while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
            outcomes.push((session.health, session.movement.velocity));
        }
        assert!(outcomes[0].0 < outcomes[1].0 && outcomes[1].0 < 200);
        assert_eq!(outcomes[2].0, 200);
        for value in outcomes.iter().skip(1) {
            for axis in 0..3 { assert!((value.1[axis] - outcomes[0].1[axis]).abs() < 0.0001); }
        }
        assert!(outcomes[0].1[2] > 250.0);
    }

    #[test]
    fn native_rocket_hit_hooks_follow_the_original_weapon_lifetime_not_current_selection() {
        for replace in [false, true] {
            let (mut session, target) = projectile_session(228, Weapon::BlackBox);
            session.health = 100;
            projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
            let launched_source = session.projectiles[0].presentation.source_weapon.unwrap();
            if replace {
                session.equip_item(PlayerClass::Soldier, crate::schema::LoadoutPosition::Primary, Some(127)).unwrap();
                session.regenerate(0, None, &mut Vec::new());
                session.equip_item(PlayerClass::Soldier, crate::schema::LoadoutPosition::Primary, Some(228)).unwrap();
                session.regenerate(0, None, &mut Vec::new());
                // Equip may regenerate health; isolate the original launcher's hit hook.
                session.health = 100;
                assert!(!session.source_weapon_is_live(launched_source, Weapon::BlackBox));
            } else {
                projectile_step(&mut session, crate::Command { select_weapon: Some(Weapon::Shotgun), ..Default::default() });
                assert!(session.source_weapon_is_live(launched_source, Weapon::BlackBox));
            }
            while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
            assert_eq!(session.health, if replace { 100 } else { 120 });
            assert!(session.bots.as_ref().unwrap().health(target).unwrap() < 300);
        }
    }

    #[test]
    fn native_airstrike_does_not_credit_a_destroyed_launcher_recreated_with_the_same_definition() {
        let (mut session, target) = projectile_session(1104, Weapon::AirStrike);
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().health.current = 70;
        projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        session.equip_item(PlayerClass::Soldier, crate::schema::LoadoutPosition::Primary, Some(127)).unwrap();
        session.regenerate(0, None, &mut Vec::new());
        session.equip_item(PlayerClass::Soldier, crate::schema::LoadoutPosition::Primary, Some(1104)).unwrap();
        session.regenerate(0, None, &mut Vec::new());
        while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
        assert_eq!(session.decapitations(), 0);
        assert_eq!(session.weapon_runtime(Weapon::AirStrike).unwrap().profile().maximum_clip, 4);
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(0));
    }

    #[test]
    fn native_delayed_rocket_keeps_damage_source_but_resolves_sound_and_death_icon_at_their_sdk_lifetimes() {
        let (mut session, target) = projectile_session(228, Weapon::BlackBox);
        session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().health.current = 70;
        projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        session.equip_item(PlayerClass::Soldier, crate::schema::LoadoutPosition::Primary, Some(1104)).unwrap();
        session.regenerate(0, None, &mut Vec::new());
        session.health = 100;
        let mut killed = None;
        let mut explosion_sound = None;
        while !session.projectiles.is_empty() {
            let snapshot = projectile_step(&mut session, crate::Command::default());
            killed = snapshot.events.into_iter().find(|event| matches!(event, crate::Event::PlayerKilled { victim, .. } if *victim == target)).or(killed);
            explosion_sound = session.audio_events.iter().find(|event| event.identity == crate::AudioEventIdentity::ExplosionSpecial1).map(|event| event.definition).or(explosion_sound);
        }
        assert!(matches!(killed, Some(crate::Event::PlayerKilled { weapon: Some(Weapon::BlackBox), killing_weapon: "airstrike", .. })));
        assert_eq!(explosion_sound, Some(crate::SoundDefinition::RocketExplosion));
        assert_eq!(session.health, 100, "the recreated weapon cannot supply the old launcher's radius hit hook");
        assert_eq!(session.decapitations(), 0, "the new Air Strike only changes the displayed icon, not OnPlayerKill");
    }

    #[test]
    fn native_direct_hit_minicrits_explosive_airborne_state_not_a_normal_jump() {
        for (blast_jumping, expected_crit) in [(false, false), (true, true)] {
            let (mut session, target) = projectile_session(127, Weapon::DirectHit);
            if blast_jumping {
                session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().blast_jump_state = true;
                session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap().conditions
                    .add(ConditionId::new(81).unwrap(), crate::condition::ConditionDuration::Permanent, None, true, false).unwrap();
            }
            projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
            let mut hit = None;
            while !session.projectiles.is_empty() {
                hit = projectile_step(&mut session, crate::Command::default()).events.into_iter()
                    .find(|event| matches!(event, crate::Event::PlayerDamaged { victim, .. } if *victim == target)).or(hit);
            }
            assert!(matches!(hit, Some(crate::Event::PlayerDamaged { crit, .. }) if crit == if expected_crit { damage::CritKind::Mini } else { damage::CritKind::None }));
        }
    }

    #[test]
    fn native_airstrike_steals_heads_and_increases_capacity_without_granting_rockets() {
        let (mut session, target) = projectile_session(1104, Weapon::AirStrike);
        let victim = session.bots.as_mut().unwrap().bots.get_mut(&target).unwrap();
        victim.health.current = 70;
        victim.decapitations = 3;
        projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
        assert_eq!(session.decapitations(), 4);
        let runtime = session.weapon_runtime(Weapon::AirStrike).unwrap();
        assert_eq!(runtime.profile().maximum_clip, 8);
        assert_eq!(runtime.clip, 3);
        assert_eq!(runtime.reserve, 20);
        assert_eq!(session.bots.as_ref().unwrap().health(target), Some(0));
    }

    #[test]
    fn native_self_blast_uses_shared_invulnerability_admission_without_losing_jump_force() {
        let (mut session, _) = projectile_session(414, Weapon::LibertyLauncher);
        session.conditions.insert(crate::Condition::Invulnerable);
        let health = session.health;
        let mut damage_events = Vec::new();
        damage_events.extend(projectile_step(&mut session, crate::Command { fire: true, pitch_degrees: 90.0, ..Default::default() }).events);
        while !session.projectiles.is_empty() { damage_events.extend(projectile_step(&mut session, crate::Command::default()).events); }
        assert_eq!(session.health, health);
        assert!(session.movement.velocity[2] > 250.0);
        assert!(!damage_events.iter().any(|event| matches!(event, crate::Event::PlayerDamaged { .. } | crate::Event::Damaged { .. })));
    }

    #[test]
    fn native_manmelter_extinguish_credits_only_the_original_enemy_burner_and_spends_one_crit() {
        let (mut session, enemy) = projectile_session(595, Weapon::Manmelter);
        projectile_step(&mut session, crate::Command { bot_request: Some(Request {
            operation: Operation::Add, count: 1, class: Some(PlayerClass::Heavy),
            team: Some(PlayerTeam::Red), difficulty: Difficulty::Normal,
        }), ..Default::default() });
        let friend = session.bots.as_ref().unwrap().snapshots().into_iter().find(|bot| bot.team == PlayerTeam::Red).unwrap().identity;
        session.bots.as_mut().unwrap().teleport(friend, [100.0, 0.0, 1.0], 0.0, 0.0).unwrap();
        session.health = 100;
        let now = session.tick as f32 * 0.015;
        session.bots.as_mut().unwrap().ignite_projectile(friend, enemy, Weapon::FlareGun, "flaregun", None, now, 3.0, 7.5);
        projectile_step(&mut session, crate::Command { detonate: true, ..Default::default() });
        assert!(!session.bots.as_ref().unwrap().combat_targets().find(|bot| bot.identity == friend).unwrap().burning);
        assert_eq!(session.revenge_crits(), 1);
        assert_eq!(session.health, 120);
        assert!(session.weapon_runtime(Weapon::Manmelter).unwrap().charge_begin_tick.is_some());
        projectile_step(&mut session, crate::Command::default());
        let shot = projectile_step(&mut session, crate::Command { fire: true, ..Default::default() });
        assert!(shot.projectiles[0].critical);
        assert_eq!(session.revenge_crits(), 0);
        assert!(session.audio_events.iter().any(|event| event.definition == crate::SoundDefinition::ManmelterCrit));
        assert_eq!(session.weapon_runtime(Weapon::Manmelter).unwrap().clip, 20);
        assert_eq!(session.weapon_runtime(Weapon::Manmelter).unwrap().reserve, 32);
        // Burning from our team still extinguishes, but never awards a crit or heal.
        while !session.projectiles.is_empty() { projectile_step(&mut session, crate::Command::default()); }
        for _ in 0..36 { projectile_step(&mut session, crate::Command::default()); }
        let now = session.tick as f32 * 0.015;
        session.bots.as_mut().unwrap().ignite_projectile(friend, crate::PLAYER_IDENTITY, Weapon::FlareGun, "flaregun", None, now, 3.0, 7.5);
        projectile_step(&mut session, crate::Command { detonate: true, ..Default::default() });
        assert_eq!(session.revenge_crits(), 0);
        assert_eq!(session.health, 120);
    }

    #[test]
    fn native_manmelter_effect_clock_and_charge_stop_survive_holster_and_resupply() {
        use crate::projectile_weapon::WeaponEffect;
        let (mut session, _) = projectile_session(595, Weapon::Manmelter);
        projectile_step(&mut session, crate::Command { select_weapon: Some(Weapon::Flamethrower), ..Default::default() });
        projectile_step(&mut session, crate::Command { select_weapon: Some(Weapon::Manmelter), ..Default::default() });
        let mut idle_ticks = Vec::new();
        let mut ready_sounds = 0;
        for _ in 0..80 {
            let snapshot = projectile_step(&mut session, crate::Command::default());
            if snapshot.events.iter().any(|event| matches!(event, crate::Event::ProjectileWeaponEffect { effect: WeaponEffect::Idle, .. })) {
                idle_ticks.push(snapshot.tick);
            }
            ready_sounds += session.audio_events.iter().filter(|event| event.definition == crate::SoundDefinition::ManmelterReady).count();
            assert!(session.random_draws.iter().all(|draw| draw.context == crate::RandomContext::PredictedPresentation));
        }
        assert!(idle_ticks.len() >= 3);
        assert!(idle_ticks.windows(2).all(|ticks| ticks[1] - ticks[0] == 17));
        assert_eq!(ready_sounds, 1);
        let charging = projectile_step(&mut session, crate::Command { detonate: true, ..Default::default() });
        assert!(charging.events.iter().any(|event| matches!(event, crate::Event::ProjectileWeaponEffect { effect: WeaponEffect::ChargeStart, .. })));
        session.regenerate(0, None, &mut Vec::new());
        let stopped = projectile_step(&mut session, crate::Command::default());
        assert!(stopped.events.iter().any(|event| matches!(event, crate::Event::ProjectileWeaponEffect { effect: WeaponEffect::ChargeStop, .. })));
        assert!(session.weapon_runtime(Weapon::Manmelter).unwrap().charge_begin_tick.is_none());
        projectile_step(&mut session, crate::Command { select_weapon: Some(Weapon::Flamethrower), ..Default::default() });
        for _ in 0..35 {
            let snapshot = projectile_step(&mut session, crate::Command::default());
            assert!(!snapshot.events.iter().any(|event| matches!(event, crate::Event::ProjectileWeaponEffect { .. })));
        }
    }

    fn capture_graph() -> Graph {
        playsrc_entity::parse(
            b"{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}\n{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}\n{\"classname\"\"item_teamflag\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}\n{\"classname\"\"item_teamflag\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}\n{\"classname\"\"func_capturezone\"\"TeamNum\"\"2\"\"model\"\"*1\"\"origin\"\"20 50 1\"}\n{\"classname\"\"func_capturezone\"\"TeamNum\"\"3\"\"model\"\"*2\"\"origin\"\"240 50 1\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap()
    }

    #[test]
    fn control_point_bot_walks_nav_to_capture_then_defends_authoritative_owner() {
        let graph = playsrc_entity::parse(br#"
            {"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "10 50 1"}
            {"classname" "info_player_teamspawn" "TeamNum" "3" "origin" "250 50 1"}
            {"classname" "team_control_point_master"}
            {"classname" "tf_logic_koth"}
            {"classname" "team_control_point" "targetname" "point" "point_index" "0" "origin" "150 50 1"}
            {"classname" "trigger_capture_area" "area_cap_point" "point" "model" "*1" "origin" "150 50 1" "area_time_to_cap" "1" "team_cancap_2" "1" "team_cancap_3" "1"}
        "#, playsrc_entity::Limits::default()).unwrap();
        let mut points = crate::control_point::World::from_graph(&graph).unwrap().unwrap();
        points.set_model_bounds(&[playsrc_entity::ModelBounds { model: 1, mins: [-24.0,-24.0,0.0], maxs: [24.0,24.0,100.0] }]);
        let mut bots = BotWorld::new(fixture_mesh(), &graph, &Floor, 0.015, None).unwrap();
        bots.configure_control_points(&points).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        bots.apply(Request { operation: Operation::Add, count: 1, class: Some(PlayerClass::Soldier), team: Some(PlayerTeam::Red), difficulty: Difficulty::Hard }, PlayerTeam::Blue, PlayerClass::Soldier, &mut random).unwrap();
        let initial = bots.snapshots()[0].position;
        let facts = crate::control_point::Facts { points_may_be_captured: true, round_running: true, koth_timer_remaining: Some([180.0;2]), timer_may_expire: true, ..crate::control_point::Facts::default() };
        let mut captured = false;
        let mut defended = false;
        let mut maximum_travel = 0.0_f32;
        for tick in 0..1000 {
            bots.advance(&Floor, tick, human_far(), &[], &mut random, Some(Objectives { rules: &crate::round::Rules::active(Default::default()).unwrap(), flags: None, points: Some(&points), in_setup: false, in_overtime: false, time_left: [100.0;2] })).unwrap();
            let bot = &bots.snapshots()[0];
            maximum_travel = maximum_travel.max(distance(initial,bot.position));
            defended |= bot.objective == ObjectiveKind::DefendPoint;
            let actor = crate::control_point::Actor::active(bot.identity,bot.team,bot.class,bot.position,PLAYER_HULL);
            let mut events = Vec::new();
            points.step(tick as f32 * 0.015,facts,&[actor],&Floor,&mut events).unwrap();
            captured |= events.iter().any(|e| matches!(e, crate::control_point::Event::Captured { .. }));
        }
        assert!(maximum_travel > 100.0);
        assert!(captured);
        assert!(defended);
        assert_eq!(points.points()[0].owner, PlayerTeam::Red);
    }

    #[test]
    fn capture_objectives_follow_live_flag_carriers_and_authored_brush_centers() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}{\"classname\"\"item_teamflag\"\"TeamNum\"\"2\"\"origin\"\"250 50 1\"}{\"classname\"\"item_teamflag\"\"TeamNum\"\"3\"\"origin\"\"10 50 1\"}{\"classname\"\"func_capturezone\"\"TeamNum\"\"2\"\"model\"\"*1\"}{\"classname\"\"func_capturezone\"\"TeamNum\"\"3\"\"model\"\"*2\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut objectives =
            crate::ctf::World::compile(&graph, crate::ctf::Configuration::default())
                .unwrap()
                .unwrap();
        objectives.set_model_bounds(&[
            playsrc_entity::ModelBounds {
                model: 1,
                mins: [0.0, 0.0, 0.0],
                maxs: [100.0, 100.0, 100.0],
            },
            playsrc_entity::ModelBounds {
                model: 2,
                mins: [200.0, 0.0, 0.0],
                maxs: [300.0, 100.0, 100.0],
            },
        ]);
        let mut world =
            BotWorld::new(fixture_mesh(), &graph, &Floor, 0.015, Some(&objectives)).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Normal,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                &mut random,
            )
            .unwrap();
        let initial = world.snapshots()[0].clone();
        assert_eq!(initial.objective, ObjectiveKind::FetchFlag);
        objectives
            .advance(
                &Floor,
                0.0,
                &[crate::ctf::Actor::active(
                    initial.identity,
                    initial.team,
                    initial.position,
                    Hull {
                        mins: [-24.0, -24.0, 0.0],
                        maxs: [24.0, 24.0, 82.0],
                    },
                )],
            )
            .unwrap();
        assert_eq!(
            objectives.carrier_flag(initial.identity).unwrap().team,
            PlayerTeam::Red
        );
        world
            .advance(
                &Floor,
                1,
                Human {
                    team: PlayerTeam::Red,
                    class: PlayerClass::Soldier,
                    alive: false,
                    position: [0.0; 3],
                    velocity: [0.0; 3],
                },
                &[],
                &mut random,
                Some(Objectives { rules: &crate::round::Rules::active(Default::default()).unwrap(), flags: Some(&objectives), points: None, in_setup: false, in_overtime: false, time_left: [0.0;2] }),
            )
            .unwrap();
        assert_eq!(world.snapshots()[0].objective, ObjectiveKind::DeliverFlag);
        assert_eq!(
            objectives
                .bot_objective(initial.identity, PlayerTeam::Blue)
                .unwrap()
                .capture_position,
            Some([250.0, 50.0, 50.0])
        );
    }

    #[test]
    fn injured_and_ammo_low_bots_follow_authored_health_and_ammo_supplies() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Normal,
                },
                PlayerTeam::Blue,
                PlayerClass::Scout,
                &mut random,
            )
            .unwrap();
        world.bots.get_mut(&2).unwrap().health.current = 100;
        let supplies = [
            SupplyTarget {
                identity: 80,
                kind: Some(crate::pickup::MapPickupKind::Health),
                team: None,
                position: [125.0, 50.0, 1.0],
            },
            SupplyTarget {
                identity: 81,
                kind: Some(crate::pickup::MapPickupKind::Ammo),
                team: None,
                position: [175.0, 50.0, 1.0],
            },
        ];
        world
            .advance(&Floor, 0, human_far(), &supplies, &mut random, None)
            .unwrap();
        assert_eq!(world.snapshots()[0].objective, ObjectiveKind::GetHealth);
        let health = crate::pickup::map_pickup_definition(b"item_healthkit_medium").unwrap();
        assert_eq!(world.grant_pickup(2, health), Some(100));
        world
            .bots
            .get_mut(&2)
            .unwrap()
            .loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .reserve = 1;
        world
            .advance(&Floor, 1, human_far(), &supplies, &mut random, None)
            .unwrap();
        assert_eq!(world.snapshots()[0].objective, ObjectiveKind::GetHealth);
        for tick in 2..=7 { world.advance(&Floor,tick,human_far(),&supplies,&mut random,None).unwrap(); }
        assert_eq!(world.snapshots()[0].objective, ObjectiveKind::GetAmmo);
        let ammo = crate::pickup::map_pickup_definition(b"item_ammopack_small").unwrap();
        assert!(world.grant_pickup(2, ammo).unwrap() >= 4);
        assert_eq!(world.snapshots()[0].weapon.unwrap().reserve, 5);
    }

    #[test]
    fn indexed_actor_perception_matches_reference_order_for_full_mixed_roster() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(19).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 23,
                    class: None,
                    team: None,
                    difficulty: Difficulty::Expert,
                },
                PlayerTeam::Red,
                PlayerClass::Medic,
                &mut random,
            )
            .unwrap();
        for (index, bot) in world.bots.values_mut().enumerate() {
            if index % 5 == 0 {
                bot.lifecycle = PlayerLifecycle::Dying;
            }
            bot.last_fire_tick = (index % 3 == 0).then_some(11);
            bot.target = (index % 3 == 0).then_some(crate::PLAYER_IDENTITY);
        }

        for human in [
            Human {
                team: PlayerTeam::Red,
                class: PlayerClass::Medic,
                alive: true,
                position: [40.0, 50.0, 1.0],
                velocity: [0.0; 3],
            },
            Human {
                team: PlayerTeam::Spectator,
                class: PlayerClass::Spy,
                alive: false,
                position: [40.0, 50.0, 1.0],
                velocity: [0.0; 3],
            },
        ] {
            let actors = world.actors(human, 12);
            for team in [PlayerTeam::Red, PlayerTeam::Blue] {
                let reference: Vec<_> = actors
                    .all()
                    .iter()
                    .filter(|actor| actor.alive && team.is_enemy(actor.team))
                    .map(|actor| actor.identity)
                    .collect();
                let indexed: Vec<_> = actors.enemies(team).map(|actor| actor.identity).collect();
                assert_eq!(indexed, reference);
            }
            for actor in actors.all() {
                let indexed = actors.get(actor.identity).unwrap();
                assert_eq!(indexed.identity, actor.identity);
                assert_eq!(indexed.firing_at, actor.firing_at);
            }
            assert!(actors.get(u32::MAX).is_none());
        }
    }

    #[test]
    fn immutable_supply_facts_and_fastest_routes_preserve_selection_and_reuse_paths() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(7).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 4,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Expert,
                },
                PlayerTeam::Blue,
                PlayerClass::Scout,
                &mut random,
            )
            .unwrap();
        for bot in world.bots.values_mut() {
            bot.health.current = 40;
        }
        let supplies = [
            SupplyTarget {
                identity: 82,
                kind: Some(crate::pickup::MapPickupKind::Health),
                team: None,
                position: [125.0, 50.0, 1.0],
            },
            SupplyTarget {
                identity: 80,
                kind: Some(crate::pickup::MapPickupKind::Health),
                team: None,
                position: [125.0, 50.0, 1.0],
            },
            SupplyTarget {
                identity: 81,
                kind: Some(crate::pickup::MapPickupKind::Ammo),
                team: None,
                position: [175.0, 50.0, 1.0],
            },
        ];
        let actors = world.actors(human_far(), 1);
        let mut cache = SupplyCache::default();
        let mut scratch = PathScratch::default();
        for bot in world.bots.values() {
            let chosen = select_supply(
                &world.mesh,
                bot,
                &actors,
                &supplies,
                crate::pickup::MapPickupKind::Health,
                &mut cache,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(chosen.identity, 80);
            assert_eq!(cache.routes.len(), 1);
        }
        assert!(cache.facts[0].is_some());
        assert!(cache.facts[1].is_some());
        assert!(cache.facts[2].is_none());
    }

    #[test]
    fn medic_healing_and_uber_share_the_authoritative_combat_bot_health() {
        let graph = fixture_graph();
        let mut session =
            crate::Session::new(Floor, [-90.0, 50.0, 1.0], crate::MapRuntime::empty(0.015));
        session
            .configure_navigation(fixture_mesh(), &graph)
            .unwrap();
        session
            .advance(crate::Command {
                select_class: Some(PlayerClass::Medic),
                select_weapon: Some(crate::Weapon::MediGun),
                bot_request: Some(Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Red),
                    difficulty: Difficulty::Normal,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        assert!(session.bots.as_mut().unwrap().apply_damage(2, 100.0));
        let stopped_position = session.bot_world().unwrap().snapshots()[0].position;
        for _ in 0..60 {
            session
                .advance(crate::Command {
                    fire: true,
                    pitch_degrees: 16.7,
                    nextbot_stop: true,
                    ..crate::Command::default()
                })
                .unwrap();
        }
        assert_eq!(session.medigun().target, Some(2));
        assert!(session.medigun().charge > 0.0);
        assert!(session.bot_world().unwrap().snapshots()[0].health > 100);
        assert_eq!(
            session.bot_world().unwrap().snapshots()[0].position,
            stopped_position
        );
        session.medigun.charge = 1.0;
        let uber = session
            .advance(crate::Command {
                fire: true,
                detonate: true,
                pitch_degrees: 16.7,
                ..crate::Command::default()
            })
            .unwrap();
        assert!(uber.medigun_releasing);
        assert!(session.conditions.contains(crate::Condition::Invulnerable));
        let medic_health = session.health;
        let mut combat_events = Vec::new();
        session
            .apply_actor_damage(
                Damage {
                    source_weapon: None,
                    damage_type: weapon_damage_type(Weapon::Knife).unwrap(), force: [0.0; 3],
                    crit: CritKind::None, range_multiplier: 1.0, custom: CustomDamage::None,
                    modifiers: DamageModifiers::default(), killing_weapon: None,
                    attacker: 3,
                    victim: crate::PLAYER_IDENTITY,
                    weapon: Weapon::Knife,
                    amount: 65.0,
                    position: [0.0; 3],
                },
                PlayerTeam::Blue,
                &mut combat_events,
            )
            .unwrap();
        assert_eq!(session.health, medic_health);
        assert!(combat_events.is_empty());
        let before = session.bot_world().unwrap().snapshots()[0].health;
        let result = session.bots.as_mut().unwrap().damage(
            Damage {
                source_weapon: None,
                damage_type: weapon_damage_type(Weapon::Bonesaw).unwrap(), force: [0.0; 3],
                crit: CritKind::None, range_multiplier: 1.0, custom: CustomDamage::None,
                modifiers: DamageModifiers::default(), killing_weapon: None,
                attacker: 3,
                victim: 2,
                weapon: Weapon::Bonesaw,
                amount: 65.0,
                position: [0.0; 3],
            },
            PlayerTeam::Blue,
            100,
            ConditionState::default(),
        );
        assert_eq!(result.unwrap().unwrap().denial, Some(damage::DamageDenial::Invulnerable));
        assert_eq!(session.bot_world().unwrap().snapshots()[0].health, before);
        let detached = session.advance(crate::Command::default()).unwrap();
        assert_eq!(detached.medigun_target, None);
        assert!(
            session
                .bots
                .as_mut()
                .unwrap()
                .patient_state(2)
                .unwrap()
                .0
                .healers
                .is_empty()
        );
    }

    #[test]
    fn capture_supply_objectives_survive_authoritative_flag_synchronization() {
        let graph = capture_graph();
        let objectives = crate::ctf::World::compile(&graph, crate::ctf::Configuration::default())
            .unwrap()
            .unwrap();
        let mut world =
            BotWorld::new(fixture_mesh(), &graph, &Floor, 0.015, Some(&objectives)).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Soldier,
            Difficulty::Normal,
        );
        world.bots.get_mut(&2).unwrap().health.current = 100;
        world
            .advance(
                &Floor,
                0,
                human_far(),
                &[SupplyTarget {
                    identity: 80,
                    kind: Some(crate::pickup::MapPickupKind::Health),
                    team: None,
                    position: [175.0, 50.0, 1.0],
                }],
                &mut random,
                Some(Objectives { rules: &crate::round::Rules::active(Default::default()).unwrap(), flags: Some(&objectives), points: None, in_setup: false, in_overtime: false, time_left: [0.0;2] }),
            )
            .unwrap();
        assert_eq!(world.snapshots()[0].objective, ObjectiveKind::GetHealth);
        world.synchronize_objectives(&objectives, &[]);
        assert_eq!(world.snapshots()[0].objective, ObjectiveKind::GetHealth);
    }

    #[test]
    fn source_difficulty_reaction_thresholds_are_exact() {
        assert_eq!(Difficulty::Easy.recognition_seconds(), 1.0);
        assert_eq!(Difficulty::Normal.recognition_seconds(), 0.5);
        assert_eq!(Difficulty::Hard.recognition_seconds(), 0.3);
        assert_eq!(Difficulty::Expert.recognition_seconds(), 0.2);
    }

    #[test]
    fn source_player_activity_translates_demoman_secondary_and_spy_melee_roles() {
        assert_eq!(
            animation_role(PlayerClass::Demoman, Some(Weapon::GrenadeLauncher)),
            AnimationRole::Secondary
        );
        assert_eq!(
            animation_role(PlayerClass::Demoman, Some(Weapon::Bottle)),
            AnimationRole::Melee
        );
        assert_eq!(
            animation_role(PlayerClass::Spy, Some(Weapon::Revolver)),
            AnimationRole::Melee
        );
        assert_eq!(
            animation_role(PlayerClass::Soldier, Some(Weapon::RocketLauncher)),
            AnimationRole::Primary
        );
        assert_eq!(
            animation_role(PlayerClass::Soldier, Some(Weapon::Shotgun)),
            AnimationRole::Secondary
        );
        assert_eq!(
            animation_role(PlayerClass::Medic, Some(Weapon::SyringeGun)),
            AnimationRole::Primary
        );
        assert_eq!(
            animation_role(PlayerClass::Medic, Some(Weapon::MediGun)),
            AnimationRole::Secondary
        );
        assert_eq!(
            animation_role(PlayerClass::Medic, Some(Weapon::Bonesaw)),
            AnimationRole::Melee
        );
    }

    #[test]
    fn medic_bots_fire_real_authored_syringe_projectiles_instead_of_empty_shots() {
        let graph = fixture_graph();
        let map = crate::MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap();
        let mut session = crate::Session::new(Floor, [190.0, 50.0, 1.0], map);
        session
            .configure_navigation(fixture_mesh(), &graph)
            .unwrap();
        let spawned = session
            .advance(crate::Command {
                bot_request: Some(Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Medic),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Expert,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        let identity = spawned.bots[0].identity;
        assert_eq!(spawned.bots[0].weapon.unwrap().weapon, Weapon::SyringeGun);

        let mut fired = false;
        for _ in 0..90 {
            let snapshot = session.advance(crate::Command::default()).unwrap();
            fired |= snapshot.projectile_events.iter().any(|event| {
                event.projectile_kind == crate::ProjectileKind::Syringe
                    && event.owner_identity == identity
            });
            if fired {
                break;
            }
        }
        assert!(fired);
    }

    #[test]
    fn source_quota_manager_waits_for_a_human_and_adds_one_balanced_bot_per_quarter_second() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .configure(Configuration {
                quota: 4,
                maximum_players: 24,
                mode: QuotaMode::Normal,
                difficulty: Difficulty::Hard,
                join_after_player: true,
                auto_vacate: false,
                offline_practice: true,
            })
            .unwrap();
        assert!(
            !world
                .maintain_quota(
                    0,
                    PlayerTeam::Unassigned,
                    PlayerClass::Soldier,
                    0,
                    0,
                    &mut random,
                )
                .unwrap()
        );
        assert!(
            !world
                .maintain_quota(16, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut random)
                .unwrap()
        );
        assert!(
            world
                .maintain_quota(17, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut random)
                .unwrap()
        );
        assert_eq!(world.snapshots()[0].team, PlayerTeam::Blue);
        assert_eq!(world.snapshots()[0].difficulty, Difficulty::Hard);
        assert!(
            !world
                .maintain_quota(33, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut random)
                .unwrap()
        );
        assert!(
            world
                .maintain_quota(34, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut random)
                .unwrap()
        );
        assert_eq!(world.len(), 2);
        assert_eq!(world.snapshots()[1].team, PlayerTeam::Red);
    }

    #[test]
    fn full_local_quota_balances_twenty_four_players_for_either_human_team() {
        for human_team in [PlayerTeam::Red, PlayerTeam::Blue] {
            let mut world =
                BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
            let mut random = UniformRandomStream::from_seed(0).unwrap();
            world
                .configure(Configuration {
                    quota: 23,
                    maximum_players: 24,
                    mode: QuotaMode::Normal,
                    difficulty: Difficulty::Expert,
                    join_after_player: true,
                    auto_vacate: false,
                    offline_practice: false,
                })
                .unwrap();

            for admission in 0..23 {
                let tick = admission * 17;
                assert!(
                    world
                        .maintain_quota(tick, human_team, PlayerClass::Soldier, 0, 0, &mut random,)
                        .unwrap()
                );
                assert_eq!(world.len(), admission as usize + 1);
            }

            let snapshots = world.snapshots();
            let red = snapshots
                .iter()
                .filter(|bot| bot.team == PlayerTeam::Red)
                .count()
                + usize::from(human_team == PlayerTeam::Red);
            let blue = snapshots
                .iter()
                .filter(|bot| bot.team == PlayerTeam::Blue)
                .count()
                + usize::from(human_team == PlayerTeam::Blue);
            assert_eq!((red, blue), (12, 12));
            assert!(
                !world
                    .maintain_quota(23 * 17, human_team, PlayerClass::Soldier, 0, 0, &mut random)
                    .unwrap()
            );
        }
    }

    #[test]
    fn admission_observation_preserves_roster_rng_cadence_and_failed_transaction() {
        use crate::admission_metrics as metrics;
        use std::cell::RefCell;
        thread_local! { static EVENTS: RefCell<Vec<metrics::Event>> = const { RefCell::new(Vec::new()) }; }
        fn record(event: metrics::Event) { EVENTS.with_borrow_mut(|events| events.push(event)); }
        struct Reset;
        impl Drop for Reset { fn drop(&mut self) { metrics::set_observer(None); } }
        let _reset = Reset;
        for quota in [15, 23] {
            let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
            world.configure(Configuration { quota, maximum_players: 24, mode: QuotaMode::Normal, difficulty: Difficulty::Easy, join_after_player: true, auto_vacate: false, offline_practice: true }).unwrap();
            let mut expected = world.clone();
            let mut rng = UniformRandomStream::from_seed(0).unwrap();
            let mut expected_rng = rng.clone();
            EVENTS.with_borrow_mut(Vec::clear);
            for tick in 0..(u64::from(quota) + 2) * 17 {
                metrics::set_observer(None);
                let change = expected.maintain_quota(tick, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut expected_rng).unwrap();
                metrics::set_observer(Some(record));
                assert_eq!(world.maintain_quota(tick, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut rng).unwrap(), change);
                assert_eq!(world.snapshots(), expected.snapshots());
                assert_eq!(world.roster().collect::<Vec<_>>(), expected.snapshots().into_iter().map(|bot| crate::team_selection::RosterPlayer { identity: bot.identity, team: bot.team }).collect::<Vec<_>>());
                assert_eq!(rng.state(), expected_rng.state());
            }
            let events = EVENTS.with_borrow(|events| events.clone());
            let requests: Vec<_> = events.iter().filter(|event| event.stage == metrics::REQUEST).collect();
            assert_eq!(requests.len(), quota as usize);
            for (index, request) in requests.iter().enumerate() {
                assert_eq!((request.tick, request.actor), (index as u64 * 17, index as u32 + crate::PLAYER_IDENTITY + 1));
                let stages: Vec<_> = events.iter().filter(|event| event.actor == request.actor).map(|event| event.stage).collect();
                assert_eq!(stages, [metrics::REQUEST, metrics::LOADOUT, metrics::NAVIGATION, metrics::CONSTRUCTED]);
            }
        }
        let graph = fixture_graph();
        let mut session = crate::Session::new(Floor, [190.0, 50.0, 1.0], crate::MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap());
        session.configure_navigation(fixture_mesh(), &graph).unwrap();
        session.bots.as_mut().unwrap().spawns[team_index(PlayerTeam::Blue)].clear();
        let before = session.clone();
        EVENTS.with_borrow_mut(Vec::clear);
        let result = session.advance(crate::Command { bot_request: Some(Request { operation: Operation::Add, count: 2, class: Some(PlayerClass::Soldier), team: None, difficulty: Difficulty::Easy }), ..crate::Command::default() });
        assert!(result.is_err());
        assert_eq!(session.tick(), before.tick());
        assert_eq!(session.producer_snapshot(), before.producer_snapshot());
        assert_eq!(session.random_state(), before.random_state());
        assert_eq!(session.bot_world().unwrap().snapshots(), before.bot_world().unwrap().snapshots());
        assert_eq!(session.bot_world().unwrap().next_identity, before.bot_world().unwrap().next_identity);
        assert!(EVENTS.with_borrow(|events| events.iter().all(|event| event.stage != metrics::ROSTER)));
    }

    #[test]
    fn default_upward_offline_practice_keeps_fifteen_active_payload_bots_on_both_teams() {
        for human_team in [PlayerTeam::Red, PlayerTeam::Blue] {
            let mut world =
                BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
            let mut random = UniformRandomStream::from_seed(0).unwrap();
            world
                .configure(Configuration {
                    quota: 15,
                    maximum_players: 24,
                    mode: QuotaMode::Normal,
                    difficulty: Difficulty::Easy,
                    join_after_player: true,
                    auto_vacate: false,
                    offline_practice: true,
                })
                .unwrap();
            for admission in 0..15 {
                let tick = admission * 17;
                assert!(
                    world
                        .maintain_quota(tick, human_team, PlayerClass::Soldier, 0, 0, &mut random)
                        .unwrap()
                );
                world
                    .advance(&Floor, tick, human_far(), &[], &mut random, None)
                    .unwrap();
            }
            let bots = world.snapshots();
            assert_eq!(bots.len(), 15);
            assert_eq!(
                bots.iter()
                    .filter(|bot| bot.team == PlayerTeam::Red)
                    .count()
                    + usize::from(human_team == PlayerTeam::Red),
                8
            );
            assert_eq!(
                bots.iter()
                    .filter(|bot| bot.team == PlayerTeam::Blue)
                    .count()
                    + usize::from(human_team == PlayerTeam::Blue),
                8
            );
            assert!(bots.iter().all(|bot| {
                bot.area.is_some()
                    && bot.difficulty == Difficulty::Easy
                    && bot.objective
                        == if bot.team == PlayerTeam::Blue {
                            ObjectiveKind::PayloadPush
                        } else {
                            ObjectiveKind::PayloadGuard
                        }
            }));
        }
    }

    #[test]
    fn source_quota_modes_honor_fill_match_vacancy_and_balanced_removal() {
        for (mode, expected) in [
            (QuotaMode::Normal, 3),
            (QuotaMode::Fill, 2),
            (QuotaMode::Match, 3),
        ] {
            let mut world =
                BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
            let mut random = UniformRandomStream::from_seed(0).unwrap();
            world
                .configure(Configuration {
                    quota: 3,
                    maximum_players: 5,
                    mode,
                    difficulty: Difficulty::Normal,
                    join_after_player: true,
                    auto_vacate: true,
                    offline_practice: false,
                })
                .unwrap();
            for tick in (0..100).step_by(17) {
                world
                    .maintain_quota(
                        tick,
                        PlayerTeam::Red,
                        PlayerClass::Soldier,
                        0,
                        0,
                        &mut random,
                    )
                    .unwrap();
            }
            assert_eq!(world.len(), expected, "{mode:?}");
            assert!(
                world
                    .snapshots()
                    .iter()
                    .filter(|bot| bot.team == PlayerTeam::Red)
                    .count()
                    .abs_diff(
                        world
                            .snapshots()
                            .iter()
                            .filter(|bot| bot.team == PlayerTeam::Blue)
                            .count()
                            .saturating_sub(1),
                    )
                    <= 1
            );
        }

        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .configure(Configuration {
                quota: 3,
                maximum_players: 32,
                mode: QuotaMode::Normal,
                difficulty: Difficulty::Normal,
                join_after_player: true,
                auto_vacate: false,
                offline_practice: false,
            })
            .unwrap();
        for tick in [0, 17, 34] {
            world
                .maintain_quota(
                    tick,
                    PlayerTeam::Red,
                    PlayerClass::Soldier,
                    0,
                    0,
                    &mut random,
                )
                .unwrap();
        }
        world
            .configure(Configuration {
                quota: 1,
                ..world.configuration().unwrap()
            })
            .unwrap();
        assert!(
            world
                .maintain_quota(51, PlayerTeam::Red, PlayerClass::Soldier, 0, 0, &mut random)
                .unwrap()
        );
        assert_eq!(world.len(), 2);
    }

    #[test]
    fn explicit_nine_class_roster_survives_quota_updates_without_replacement() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .configure(Configuration {
                quota: 0,
                maximum_players: 32,
                mode: QuotaMode::Normal,
                difficulty: Difficulty::Expert,
                join_after_player: true,
                auto_vacate: true,
                offline_practice: false,
            })
            .unwrap();
        for (index, class) in PlayerClass::ALL.into_iter().enumerate() {
            let team = if index % 2 == 0 {
                PlayerTeam::Blue
            } else {
                PlayerTeam::Red
            };
            world
                .apply(
                    Request {
                        operation: Operation::Add,
                        count: 1,
                        class: Some(class),
                        team: Some(team),
                        difficulty: Difficulty::Expert,
                    },
                    PlayerTeam::Red,
                    PlayerClass::Soldier,
                    &mut random,
                )
                .unwrap();
            let tick = index as u64 * 20;
            world.forced_change(1, 0, tick);
            world
                .maintain_quota(
                    tick,
                    PlayerTeam::Red,
                    PlayerClass::Soldier,
                    0,
                    0,
                    &mut random,
                )
                .unwrap();
            assert_eq!(world.len(), index + 1);
        }
    }

    #[test]
    fn payload_push_hides_behind_the_cart_or_from_a_known_threat() {
        let scenario = Scenario::Payload {
            cart: [100.0, 200.0, 32.0],
            forward: [1.0, 0.0, 0.0],
        };
        assert_eq!(
            objective(scenario, PlayerTeam::Blue, None, None),
            (ObjectiveKind::PayloadPush, [40.0, 200.0, 32.0])
        );
        assert_eq!(
            objective(scenario, PlayerTeam::Blue, Some([100.0, 100.0, 0.0]), None),
            (ObjectiveKind::PayloadPush, [100.0, 260.0, 32.0])
        );
        assert_eq!(
            objective(scenario, PlayerTeam::Red, None, None),
            (ObjectiveKind::PayloadGuard, [100.0, 200.0, 32.0])
        );
    }

    #[test]
    fn flame_contact_afterburn_airblast_and_fatal_hits_mutate_real_bot_players() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Scout),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Normal,
                },
                PlayerTeam::Red,
                PlayerClass::Pyro,
                &mut random,
            )
            .unwrap();
        let identity = world.snapshots()[0].identity;
        assert!(world.apply_flame_contact(identity, 1, None, 1.0, 13.0));
        assert_eq!(world.snapshots()[0].health, 112);
        assert!(world.combat_targets().next().unwrap().burning);
        assert!(!world.apply_flame_contact(identity, 1, None, 1.04, 13.0));
        assert!(world.apply_flame_contact(identity, 1, None, 1.08, 13.0));
        assert_eq!(world.snapshots()[0].health, 99);
        assert!(world.apply_impulse(identity, [500.0, 0.0, 100.0]));
        assert_eq!(world.snapshots()[0].velocity, [500.0, 0.0, 100.0]);
        assert!(world.extinguish(identity));
        assert!(!world.combat_targets().next().unwrap().burning);
        assert!(world.apply_damage(identity, 100.0));
        assert_eq!(world.snapshots()[0].lifecycle, PlayerLifecycle::Dying);
        assert_eq!(world.snapshots()[0].health, 0);
    }

    #[test]
    fn player_lifecycle_path_movement_target_recognition_and_team_removal_are_deterministic() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Normal,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                &mut random,
            )
            .unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Demoman),
                    team: Some(PlayerTeam::Red),
                    difficulty: Difficulty::Hard,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                &mut random,
            )
            .unwrap();
        let initial = world.snapshots();
        assert_eq!(initial.len(), 2);
        assert_eq!(initial[0].identity, 2);
        assert_eq!(initial[0].team, PlayerTeam::Blue);
        assert_eq!(initial[0].class, PlayerClass::Soldier);
        assert_eq!(initial[0].objective, ObjectiveKind::PayloadPush);
        assert_eq!(initial[0].health, 200);
        assert_eq!(initial[1].team, PlayerTeam::Red);
        assert_eq!(initial[1].class, PlayerClass::Demoman);
        assert_eq!(initial[1].objective, ObjectiveKind::PayloadGuard);
        assert_eq!(initial[1].health, 175);
        for tick in 0..61 {
            world
                .advance(
                    &Floor,
                    tick,
                    Human {
                        team: PlayerTeam::Red,
                        class: PlayerClass::Soldier,
                        alive: true,
                        position: [190.0, 50.0, 1.0],
                        velocity: [0.0; 3],
                    },
                    &[],
                    &mut random,
                    None,
                )
                .unwrap();
        }
        let advanced = world.snapshots();
        assert_ne!(advanced[0].position, initial[0].position);
        assert!(advanced[0].area.is_some());
        assert_eq!(advanced[0].target, Some(initial[1].identity));
        assert!(advanced[0].remaining_path_areas > 0);
        world
            .apply(
                Request {
                    operation: Operation::KickTeam(PlayerTeam::Red),
                    count: 0,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Red),
                    difficulty: Difficulty::Easy,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                &mut random,
            )
            .unwrap();
        assert_eq!(
            world
                .snapshots()
                .iter()
                .map(|bot| bot.team)
                .collect::<Vec<_>>(),
            vec![PlayerTeam::Blue]
        );
        world
            .apply(
                Request {
                    operation: Operation::KickAll,
                    count: 0,
                    class: Some(PlayerClass::Soldier),
                    team: None,
                    difficulty: Difficulty::Easy,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                &mut random,
            )
            .unwrap();
        assert!(world.is_empty());
    }

    #[test]
    fn every_authored_player_class_joins_both_source_teams_with_exact_health() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        for team in [PlayerTeam::Red, PlayerTeam::Blue] {
            for class in PlayerClass::ALL {
                world
                    .apply(
                        Request {
                            operation: Operation::Add,
                            count: 1,
                            class: Some(class),
                            team: Some(team),
                            difficulty: Difficulty::Normal,
                        },
                        PlayerTeam::Red,
                        PlayerClass::Soldier,
                        &mut random,
                    )
                    .unwrap();
                let snapshot = world.snapshots().last().cloned().unwrap();
                assert_eq!(snapshot.class, class);
                assert_eq!(snapshot.team, team);
                assert_eq!(snapshot.health, class.data().maximum_health);
                assert_eq!(snapshot.maximum_health, class.data().maximum_health);
                assert_eq!(snapshot.lifecycle, PlayerLifecycle::Active);
                assert_eq!(
                    snapshot.weapon.map(|weapon| weapon.weapon),
                    crate::default_weapon(class)
                );
            }
        }
        assert_eq!(world.len(), 18);
    }

    #[test]
    fn spy_bot_advances_with_its_authored_stock_revolver() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Spy),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Expert,
                },
                PlayerTeam::Red,
                PlayerClass::Heavy,
                &mut random,
            )
            .unwrap();
        for tick in 0..60 {
            world
                .advance(
                    &Floor,
                    tick,
                    Human {
                        team: PlayerTeam::Red,
                        class: PlayerClass::Heavy,
                        alive: true,
                        position: [190.0, 50.0, 1.0],
                        velocity: [0.0; 3],
                    },
                    &[],
                    &mut random,
                    None,
                )
                .unwrap();
        }
        assert_eq!(world.snapshots()[0].class, PlayerClass::Spy);
        assert_eq!(
            world.snapshots()[0].weapon.unwrap().weapon,
            Weapon::Revolver
        );
        assert!(world.snapshots()[0].shots > 0);

        let graph = fixture_graph();
        let map = crate::MapRuntime::compile(&graph, 0.015, 0, Vec::new()).unwrap();
        let mut session = crate::Session::new(Floor, [190.0, 50.0, 1.0], map);
        session
            .configure_navigation(fixture_mesh(), &graph)
            .unwrap();
        let snapshot = session
            .advance(crate::Command {
                bot_request: Some(Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Spy),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Expert,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        assert_eq!(snapshot.bots[0].class, PlayerClass::Spy);
        assert_eq!(snapshot.bots[0].weapon.unwrap().weapon, Weapon::Revolver);
        let mut heard_revolver = false;
        for _ in 0..60 {
            session.advance(crate::Command::default()).unwrap();
            heard_revolver |= session
                .audio_events()
                .iter()
                .any(|event| event.definition == crate::SoundDefinition::RevolverSingle);
        }
        assert!(heard_revolver);
    }

    #[test]
    fn preset_payload_rosters_select_exact_offense_and_defense_classes() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        for team in [PlayerTeam::Blue, PlayerTeam::Red] {
            world
                .apply(
                    Request {
                        operation: Operation::Add,
                        count: 6,
                        class: None,
                        team: Some(team),
                        difficulty: Difficulty::Normal,
                    },
                    PlayerTeam::Red,
                    PlayerClass::Soldier,
                    &mut random,
                )
                .unwrap();
        }
        let snapshots = world.snapshots();
        let blue: Vec<_> = snapshots
            .iter()
            .filter(|bot| bot.team == PlayerTeam::Blue)
            .map(|bot| bot.class)
            .collect();
        let red: Vec<_> = snapshots
            .iter()
            .filter(|bot| bot.team == PlayerTeam::Red)
            .map(|bot| bot.class)
            .collect();
        assert_eq!(
            blue,
            vec![
                PlayerClass::Medic,
                PlayerClass::Engineer,
                PlayerClass::Soldier,
                PlayerClass::Heavy,
                PlayerClass::Demoman,
                PlayerClass::Scout,
            ]
        );
        assert_eq!(
            red,
            vec![
                PlayerClass::Medic,
                PlayerClass::Engineer,
                PlayerClass::Demoman,
                PlayerClass::Scout,
                PlayerClass::Heavy,
                PlayerClass::Sniper,
            ]
        );
    }

    #[test]
    fn independent_spy_conditions_advance_without_bot_ai_and_reset_on_respawn() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 2,
                    class: Some(PlayerClass::Spy),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Easy,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                &mut random,
            )
            .unwrap();
        let initial = world.snapshots();
        let first = initial[0].identity;
        let second = initial[1].identity;
        let human = Human {
            team: PlayerTeam::Red,
            class: PlayerClass::Soldier,
            alive: true,
            position: [5000.0; 3],
            velocity: [0.0; 3],
        };
        world.stealth_condition(first, true, 0.0).unwrap();
        for tick in 1..=67 {
            world.advance_spies(tick, human, PLAYER_HULL);
        }
        let snapshot = world.snapshots();
        assert_eq!(snapshot[0].spy.unwrap().invisibility, 1.0);
        assert_eq!(snapshot[1].spy.unwrap().invisibility, 0.0);
        world.stealth_condition(second, true, 1.005).unwrap();
        for tick in 68..=135 {
            world.advance_spies(tick, human, PLAYER_HULL);
        }
        let snapshot = world.snapshots();
        assert_eq!(snapshot[1].spy.unwrap().invisibility, 1.0);
        assert!(snapshot[0].spy.unwrap().cloak_meter < snapshot[1].spy.unwrap().cloak_meter);
        world.stealth_condition(first, false, 2.025).unwrap();
        world.advance_spies(136, human, PLAYER_HULL);
        assert_eq!(world.snapshots()[0].spy.unwrap().invisibility, 0.0);
        assert_eq!(world.snapshots()[1].spy.unwrap().invisibility, 1.0);
        world.round_respawn(137, &mut random).unwrap();
        assert!(
            world
                .snapshots()
                .iter()
                .all(|bot| bot.spy == Some(crate::spy::SpyState::default()))
        );
    }

    #[test]
    fn source_path_cost_rejects_enemy_spawn_blocks_high_jumps_and_death_drops() {
        let mut mesh = fixture_mesh();
        let from = mesh.area(1).unwrap().clone();
        let destination = mesh
            .areas
            .iter_mut()
            .find(|area| area.identity == 2)
            .unwrap();
        destination.game_attributes = TF_NAV_SPAWN_ROOM_RED;
        assert_eq!(
            path_cost(
                &from,
                destination,
                Direction::East,
                100.0,
                PathContext {
                    team: PlayerTeam::Blue,
                    bot_identity: 2,
                    now: 0.0,
                    route: Route::Default,
                    combat_intensity: 0.0,
                }
            ),
            None
        );
        destination.game_attributes = TF_NAV_BLOCKED;
        assert_eq!(
            path_cost(
                &from,
                destination,
                Direction::East,
                100.0,
                PathContext {
                    team: PlayerTeam::Red,
                    bot_identity: 2,
                    now: 0.0,
                    route: Route::Default,
                    combat_intensity: 0.0,
                }
            ),
            None
        );
        destination.game_attributes = TF_NAV_BLOCKED | TF_NAV_UNBLOCKABLE;
        assert!(
            path_cost(
                &from,
                destination,
                Direction::East,
                100.0,
                PathContext {
                    team: PlayerTeam::Red,
                    bot_identity: 2,
                    now: 0.0,
                    route: Route::Default,
                    combat_intensity: 0.0,
                }
            )
            .is_some()
        );
        destination.game_attributes = 0;
        destination.northwest[2] = 72.0;
        destination.northeast_z = 72.0;
        destination.southeast[2] = 72.0;
        destination.southwest_z = 72.0;
        assert_eq!(
            path_cost(
                &from,
                destination,
                Direction::East,
                100.0,
                PathContext {
                    team: PlayerTeam::Red,
                    bot_identity: 2,
                    now: 0.0,
                    route: Route::Default,
                    combat_intensity: 0.0,
                }
            ),
            None
        );
        destination.northwest[2] = -1001.0;
        destination.northeast_z = -1001.0;
        destination.southeast[2] = -1001.0;
        destination.southwest_z = -1001.0;
        assert_eq!(
            path_cost(
                &from,
                destination,
                Direction::East,
                100.0,
                PathContext {
                    team: PlayerTeam::Red,
                    bot_identity: 2,
                    now: 0.0,
                    route: Route::Default,
                    combat_intensity: 0.0,
                }
            ),
            None
        );
    }

    #[test]
    fn intelligence_delivery_uses_source_fastest_route_without_fetch_route_preferences() {
        let mesh = fixture_mesh();
        let from = mesh.area(1).unwrap();
        let destination = mesh.area(2).unwrap();
        let fastest = path_cost(
            from,
            destination,
            Direction::East,
            100.0,
            PathContext {
                team: PlayerTeam::Blue,
                bot_identity: 2,
                now: 30.0,
                route: Route::Fastest,
                combat_intensity: 0.0,
            },
        )
        .unwrap();
        let default = path_cost(
            from,
            destination,
            Direction::East,
            100.0,
            PathContext {
                team: PlayerTeam::Blue,
                bot_identity: 2,
                now: 30.0,
                route: Route::Default,
                combat_intensity: 0.0,
            },
        )
        .unwrap();
        assert_eq!(fastest, 100.0);
        assert!(default > fastest);
    }

    #[test]
    fn source_bot_teleport_preserves_velocity_and_restarts_navigation_from_new_area() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Scout,
            Difficulty::Normal,
        );
        world.bots.get_mut(&2).unwrap().movement.velocity = [10.0, 20.0, 30.0];
        world.teleport(2, [25.0, 40.0, 1.0], -5.0, 135.0).unwrap();
        let bot = world.snapshots().remove(0);
        assert_eq!(bot.position, [25.0, 40.0, 1.0]);
        assert_eq!(bot.velocity, [10.0, 20.0, 30.0]);
        assert_eq!((bot.pitch_degrees, bot.yaw_degrees), (-5.0, 135.0));
        assert_eq!(bot.area, Some(1));
        assert_eq!(bot.remaining_path_areas, 0);
        assert!(world.teleport(2, [f32::NAN, 0.0, 0.0], 0.0, 0.0).is_err());
        assert!(world.teleport(99, [0.0; 3], 0.0, 0.0).is_err());
    }

    fn human_far() -> Human {
        Human {
            team: PlayerTeam::Red,
            class: PlayerClass::Soldier,
            alive: true,
            position: [10_000.0, 50.0, 1.0],
            velocity: [0.0; 3],
        }
    }

    fn add(
        world: &mut BotWorld,
        random: &mut UniformRandomStream,
        team: PlayerTeam,
        class: PlayerClass,
        difficulty: Difficulty,
    ) {
        world
            .apply(
                Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(class),
                    team: Some(team),
                    difficulty,
                },
                PlayerTeam::Red,
                PlayerClass::Soldier,
                random,
            )
            .unwrap();
    }

    #[test]
    fn recognized_scout_and_soldier_fire_authored_weapons_with_exact_cadence() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Scout,
            Difficulty::Expert,
        );
        add(
            &mut world,
            &mut random,
            PlayerTeam::Red,
            PlayerClass::Soldier,
            Difficulty::Expert,
        );
        let mut attacks = Vec::new();
        for tick in 0..150 {
            attacks.extend(
                world
                    .advance(&Floor, tick, human_far(), &[], &mut random, None)
                    .unwrap(),
            );
        }
        assert!(attacks.iter().any(|attack| attack.attacker == 2
            && attack.weapon == Weapon::Scattergun
            && attack.target == 3));
        assert!(attacks.iter().any(|attack| attack.attacker == 3
            && attack.weapon == Weapon::RocketLauncher
            && attack.target == 2));
        let scout = world
            .snapshots()
            .into_iter()
            .find(|bot| bot.identity == 2)
            .unwrap();
        assert!(scout.shots >= 2);
        assert!(scout.weapon.unwrap().clip < 6);
        let soldier = world
            .snapshots()
            .into_iter()
            .find(|bot| bot.identity == 3)
            .unwrap();
        assert!(soldier.shots >= 2);
        assert!(soldier.pitch_degrees.is_finite());
    }

    #[test]
    fn authored_scout_and_soldier_switch_to_stock_secondaries_only_at_source_boundaries() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Scout,
            Difficulty::Normal,
        );
        add(
            &mut world,
            &mut random,
            PlayerTeam::Red,
            PlayerClass::Soldier,
            Difficulty::Hard,
        );
        let threat = Actor {
            identity: 3,
            class: PlayerClass::Soldier,
            team: PlayerTeam::Red,
            alive: true,
            position: [150.0, 50.0, 0.0],
            velocity: [0.0; 3],
            firing_at: None,
        };
        let scout = world.bots.get_mut(&2).unwrap();
        scout.loadout.get_mut(&Weapon::Scattergun).unwrap().clip = 0;
        select_weapon(scout, Some(threat), 70, 0.015);
        assert_eq!(scout.active_weapon, Some(Weapon::Pistol));
        assert_eq!(scout.loadout[&Weapon::Pistol].next_primary_tick, 104);
        scout.difficulty = Difficulty::Easy;
        select_weapon(scout, Some(threat), 100, 0.015);
        assert_eq!(scout.active_weapon, Some(Weapon::Scattergun));

        let soldier = world.bots.get_mut(&3).unwrap();
        soldier
            .loadout
            .get_mut(&Weapon::RocketLauncher)
            .unwrap()
            .clip = 0;
        let near = Actor {
            identity: 2,
            class: PlayerClass::Scout,
            team: PlayerTeam::Blue,
            alive: true,
            position: [499.0, 50.0, 1.0],
            velocity: [0.0; 3],
            firing_at: None,
        };
        select_weapon(soldier, Some(near), 100, 0.015);
        assert_eq!(soldier.active_weapon, Some(Weapon::Shotgun));
        let far = Actor {
            position: [510.0, 50.0, 1.0],
            ..near
        };
        select_weapon(soldier, Some(far), 120, 0.015);
        assert_eq!(soldier.active_weapon, Some(Weapon::RocketLauncher));
    }

    #[test]
    fn threat_priority_matches_immediate_medic_spy_and_recent_attacker_rules() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Soldier,
            Difficulty::Hard,
        );
        let bot = world.bots.get(&2).unwrap();
        let distant = Actor {
            identity: 3,
            class: PlayerClass::Soldier,
            team: PlayerTeam::Red,
            alive: true,
            position: [900.0, 50.0, 1.0],
            velocity: [0.0; 3],
            firing_at: None,
        };
        let medic = Actor {
            identity: 4,
            class: PlayerClass::Medic,
            position: [1000.0, 50.0, 1.0],
            ..distant
        };
        assert_eq!(threat_order(bot, medic, distant), std::cmp::Ordering::Less);
        let spy = Actor {
            identity: 5,
            class: PlayerClass::Spy,
            position: [300.0, 50.0, 1.0],
            ..distant
        };
        let attacker = Actor {
            identity: 6,
            class: PlayerClass::Soldier,
            position: [280.0, 50.0, 1.0],
            firing_at: Some(bot.identity),
            ..distant
        };
        assert_eq!(threat_order(bot, spy, attacker), std::cmp::Ordering::Less);
    }

    #[test]
    fn soldier_leads_feet_at_1100_units_per_second_but_aims_at_close_enemy_eyes() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Soldier,
            Difficulty::Hard,
        );
        let bot = world.bots.get(&2).unwrap();
        let far = Actor {
            identity: 3,
            class: PlayerClass::Scout,
            team: PlayerTeam::Red,
            alive: true,
            position: [450.0, 50.0, 1.0],
            velocity: [100.0, 0.0, 0.0],
            firing_at: None,
        };
        let aim = aim_point(&Floor, bot, far);
        assert!((aim[0] - (450.0 + 200.0 / 1100.0 * 100.0)).abs() < 0.01);
        assert_eq!(aim[2], 1.0);
        let near = Actor {
            position: [300.0, 50.0, 1.0],
            ..far
        };
        assert_eq!(aim_point(&Floor, bot, near), near.eye());
    }

    #[test]
    fn damage_death_wave_and_respawn_restore_exact_stock_weapon_ledgers() {
        let mut world =
            BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015, None).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Scout,
            Difficulty::Normal,
        );
        add(
            &mut world,
            &mut random,
            PlayerTeam::Red,
            PlayerClass::Soldier,
            Difficulty::Normal,
        );
        world
            .bots
            .get_mut(&2)
            .unwrap()
            .loadout
            .get_mut(&Weapon::Scattergun)
            .unwrap()
            .clip = 1;
        let points = world
            .damage(
                Damage {
                    source_weapon: None,
                    damage_type: weapon_damage_type(Weapon::RocketLauncher).unwrap(), force: [0.0; 3],
                    crit: CritKind::None, range_multiplier: 1.0, custom: CustomDamage::None,
                    modifiers: DamageModifiers::default(), killing_weapon: None,
                    attacker: 3,
                    victim: 2,
                    weapon: Weapon::RocketLauncher,
                    amount: 130.0,
                    position: [200.0, 50.0, 1.0],
                },
                PlayerTeam::Red,
                50,
                ConditionState::default(),
            )
            .unwrap();
        assert_eq!(points.unwrap().health_damage, 130);
        let dead = world
            .snapshots()
            .into_iter()
            .find(|bot| bot.identity == 2)
            .unwrap();
        assert_eq!(
            (dead.lifecycle, dead.health, dead.deaths),
            (PlayerLifecycle::Dying, 0, 1)
        );
        assert_eq!(
            world
                .snapshots()
                .into_iter()
                .find(|bot| bot.identity == 3)
                .unwrap()
                .kills,
            1
        );
        let mut rules = crate::round::Rules::active(Default::default()).unwrap();
        let facts = crate::round::Facts { red_players: 2, blue_players: 1, red_alive: 2, blue_alive: 0, ..Default::default() };
        rules.advance(50.0 * 0.015, 0.015, facts).unwrap();
        for tick in 51..2000 {
            rules.advance(tick as f32 * 0.015, 0.015, facts).unwrap();
            let eligible = rules.player_can_respawn(dead.team, 50.0 * 0.015);
            world.advance(&Floor, tick, human_far(), &[], &mut random, Some(Objectives { rules: &rules, flags: None, points: None, in_setup: false, in_overtime: false, time_left: [0.0;2] })).unwrap();
            world.synchronize_respawn_times(&rules);
            assert_eq!(world.snapshots()[0].lifecycle == PlayerLifecycle::Active, eligible);
            if eligible { break; }
        }
        let alive = world.snapshots()[0].clone();
        assert_eq!(
            (alive.lifecycle, alive.health, alive.deaths),
            (PlayerLifecycle::Active, 125, 1)
        );
        assert_eq!(
            (alive.weapon.unwrap().clip, alive.weapon.unwrap().reserve),
            (6, 32)
        );
        assert_eq!(alive.respawn_tick, None);
    }

    #[test]
    fn upward_logic_auto_applies_authored_team_specific_respawn_waves() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"tf_gamerules\"\"targetname\"\"gamerules\"}{\"classname\"\"logic_auto\"\"OnMultiNewMap\"\"gamerules,SetRedTeamRespawnWaveTime,9,0,-1\"\"OnMultiNewMap\"\"gamerules,SetBlueTeamRespawnWaveTime,4,0,-1\"}\0",
            playsrc_entity::Limits::default(),
        ).unwrap();
        let mut map = crate::MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap();
        let mut rules = crate::round::Rules::active(Default::default()).unwrap();
        map.begin_tick(&Floor, crate::map_runtime::BeginTickInput { tick: 14, tick_interval: 0.015, activate_entity: None, player_position: [0.0;3], player_hull: PLAYER_HULL, grounded: true }).unwrap();
        map.apply_round_inputs(&mut rules, 0.21);
        assert_eq!(rules.respawn_waves(), [Some(9.0), Some(4.0)]);
    }

    #[test]
    fn player_hull_segments_preserve_nearest_entry_and_reject_clear_misses() {
        assert_eq!(
            segment_player([0.0, 0.0, 41.0], [200.0, 0.0, 41.0], [100.0, 0.0, 0.0]),
            Some(0.38)
        );
        assert_eq!(
            segment_player([0.0, 50.0, 41.0], [200.0, 50.0, 41.0], [100.0, 0.0, 0.0]),
            None
        );
        assert_eq!(
            segment_player([100.0, 0.0, 41.0], [200.0, 0.0, 41.0], [100.0, 0.0, 0.0]),
            Some(0.0)
        );
    }

    #[test]
    fn ctf_flags_use_one_objective_authority_for_delivery_death_and_return() {
        let graph = capture_graph();
        let mut objectives =
            crate::ctf::World::compile(&graph, crate::ctf::Configuration::default())
                .unwrap()
                .unwrap();
        let mut world =
            BotWorld::new(fixture_mesh(), &graph, &Floor, 0.015, Some(&objectives)).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Scout,
            Difficulty::Normal,
        );
        let facts = |world: &BotWorld| {
            world
                .snapshots()
                .into_iter()
                .map(|bot| {
                    let mut actor = crate::ctf::Actor::active(
                        bot.identity,
                        bot.team,
                        bot.position,
                        PLAYER_HULL,
                    );
                    actor.alive = bot.lifecycle == PlayerLifecycle::Active;
                    actor
                })
                .collect::<Vec<_>>()
        };
        world.bots.get_mut(&2).unwrap().movement.position = [10.0, 50.0, 1.0];
        let events = objectives.advance(&Floor, 0.0, &facts(&world)).unwrap();
        world.synchronize_objectives(&objectives, &events);
        let carrier = world.snapshots()[0].clone();
        assert_eq!(carrier.objective, ObjectiveKind::DeliverFlag);
        assert!(carrier.carrying_flag);
        world.bots.get_mut(&2).unwrap().movement.position = [240.0, 50.0, 1.0];
        let events = objectives.advance(&Floor, 0.015, &facts(&world)).unwrap();
        world.synchronize_objectives(&objectives, &events);
        let delivered = world.snapshots()[0].clone();
        assert_eq!((delivered.captures, delivered.carrying_flag), (1, false));
        assert_eq!(delivered.objective, ObjectiveKind::FetchFlag);
        assert_eq!(objectives.scores().blue_captures, 1);

        world.bots.get_mut(&2).unwrap().movement.position = [10.0, 50.0, 1.0];
        let events = objectives.advance(&Floor, 0.30, &facts(&world)).unwrap();
        world.synchronize_objectives(&objectives, &events);
        assert!(world.snapshots()[0].carrying_flag);
        world
            .damage(
                Damage {
                    source_weapon: None,
                    damage_type: weapon_damage_type(Weapon::Shotgun).unwrap(), force: [0.0; 3],
                    crit: CritKind::None, range_multiplier: 1.0, custom: CustomDamage::None,
                    modifiers: DamageModifiers::default(), killing_weapon: None,
                    attacker: 1,
                    victim: 2,
                    weapon: Weapon::Shotgun,
                    amount: 200.0,
                    position: [10.0, 50.0, 1.0],
                },
                PlayerTeam::Red,
                21,
                ConditionState::default(),
            )
            .unwrap();
        let events = objectives.advance(&Floor, 0.315, &facts(&world)).unwrap();
        world.synchronize_objectives(&objectives, &events);
        let red = objectives
            .flags()
            .find(|flag| flag.team == PlayerTeam::Red)
            .unwrap();
        assert_eq!(red.status, crate::ctf::FlagStatus::Dropped);
        assert_eq!(red.return_deadline, Some(60.315));
        assert!(events.iter().any(|event| matches!(
            event,
            crate::ctf::Event::Flag {
                kind: crate::ctf::FlagEventKind::Dropped,
                player: Some(2),
                ..
            }
        )));
        let returned = objectives.advance(&Floor, 60.316, &[]).unwrap();
        let red = objectives
            .flags()
            .find(|flag| flag.team == PlayerTeam::Red)
            .unwrap();
        assert_eq!(
            (red.status, red.position),
            (crate::ctf::FlagStatus::Home, red.home)
        );
        assert!(returned.iter().any(|event| matches!(
            event,
            crate::ctf::Event::Flag {
                kind: crate::ctf::FlagEventKind::Returned,
                ..
            }
        )));
    }

    #[test]
    fn complete_short_ctf_match_keeps_bot_death_return_capture_score_and_opposing_victory_real() {
        let graph = capture_graph();
        let map = crate::MapRuntime::compile(
            &graph,
            0.015,
            1,
            [1, 2]
                .into_iter()
                .map(|model| playsrc_entity::ModelBounds {
                    model,
                    mins: [-24.0; 3],
                    maxs: [24.0; 3],
                })
                .collect(),
        )
        .unwrap();
        let mut session = crate::Session::new(Floor, [150.0, 50.0, 1.0], map);
        session
            .configure_navigation(fixture_mesh(), &graph)
            .unwrap();

        let spawned = session
            .advance(crate::Command {
                objective_configuration: Some(crate::ctf::RuleConfiguration {
                    captures_per_round: 1,
                    return_on_touch: true,
                }),
                bot_request: Some(Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Scout),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Normal,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        let first = spawned.bots[0].identity;
        let taken = session
            .advance(crate::Command {
                bot_control: Some(Control::Teleport {
                    identity: first,
                    position: [10.0, 50.0, 1.0],
                    pitch_degrees: 0.0,
                    yaw_degrees: 0.0,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        assert_eq!(taken.bots[0].objective, ObjectiveKind::DeliverFlag);
        assert!(taken.bots[0].carrying_flag);

        let dropped = session
            .advance(crate::Command {
                bot_control: Some(Control::Whack { identity: first }),
                ..crate::Command::default()
            })
            .unwrap();
        assert_eq!(dropped.bots[0].lifecycle, PlayerLifecycle::Dying);
        assert_eq!(dropped.bots[0].deaths, 1);
        assert_eq!(dropped.scoreboard.players[0].counters.kills, 1);
        let flag = dropped
            .objectives
            .as_ref()
            .unwrap()
            .flags
            .iter()
            .find(|flag| flag.team == PlayerTeam::Red)
            .unwrap();
        assert_eq!(flag.status, crate::ctf::FlagStatus::Dropped);
        session.set_position(flag.position).unwrap();
        let returned = session.advance(crate::Command::default()).unwrap();
        assert!(
            returned
                .objectives
                .as_ref()
                .unwrap()
                .events
                .iter()
                .any(|event| matches!(
                    event,
                    crate::ctf::Event::Flag {
                        kind: crate::ctf::FlagEventKind::Returned,
                        ..
                    }
                ))
        );

        let added = session
            .advance(crate::Command {
                bot_request: Some(Request {
                    operation: Operation::Add,
                    count: 1,
                    class: Some(PlayerClass::Soldier),
                    team: Some(PlayerTeam::Blue),
                    difficulty: Difficulty::Normal,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        let finisher = added.bots[1].identity;
        session.set_position([150.0, 50.0, 1.0]).unwrap();
        let carried = session
            .advance(crate::Command {
                bot_control: Some(Control::Teleport {
                    identity: finisher,
                    position: [10.0, 50.0, 1.0],
                    pitch_degrees: 0.0,
                    yaw_degrees: 0.0,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        assert!(carried.bots[1].carrying_flag);
        let victory = session
            .advance(crate::Command {
                bot_control: Some(Control::Teleport {
                    identity: finisher,
                    position: [240.0, 50.0, 1.0],
                    pitch_degrees: 0.0,
                    yaw_degrees: 0.0,
                }),
                ..crate::Command::default()
            })
            .unwrap();
        assert_eq!(victory.round.state, crate::round::State::TeamWin);
        assert_eq!(victory.round.winning_team, Some(PlayerTeam::Blue));
        assert_eq!(victory.objectives.unwrap().scores.blue_captures, 1);
        assert_eq!(victory.scoreboard.blue_score, 1);
        assert_eq!(victory.bots[1].captures, 1);
    }
}
