use std::{collections::BTreeMap, sync::Arc};

use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph};
use playsrc_movement::{
    Command as MoveCommand, Configuration as MovementConfiguration, Player, State, StepInput, step,
};
use playsrc_nav::{Area, Direction, Mesh, PathScratch};

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    Default,
    Fastest,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
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
    pub deaths: u32,
    pub captures: u32,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SupplyTarget {
    pub identity: u32,
    pub kind: Option<crate::pickup::MapPickupKind>,
    pub team: Option<PlayerTeam>,
    pub position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attack {
    pub attacker: u32,
    pub team: PlayerTeam,
    pub weapon: Weapon,
    pub target: u32,
    pub position: [f32; 3],
    pub eye_position: [f32; 3],
    pub pitch_degrees: f32,
    pub yaw_degrees: f32,
    pub seconds_since_previous_shot: f32,
    pub damage_multiplier: f32,
    pub spread_multiplier: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Damage {
    pub attacker: u32,
    pub victim: u32,
    pub weapon: Weapon,
    pub amount: f32,
    pub position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatTarget {
    pub identity: u32,
    pub class: PlayerClass,
    pub team: PlayerTeam,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub burning: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Spawn {
    position: [f32; 3],
    yaw_degrees: f32,
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
struct Bot {
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
    goal: [f32; 3],
    loadout: BTreeMap<Weapon, WeaponRuntime>,
    active_weapon: Option<Weapon>,
    last_fire_tick: Option<u64>,
    pending_melee: Option<(u64, u32, Weapon)>,
    respawn_tick: Option<u64>,
    carrying_flag: Option<PlayerTeam>,
    shots: u32,
    hits: u32,
    kills: u32,
    deaths: u32,
    captures: u32,
    damage_dealt: u32,
    killstreak: u32,
}

#[derive(Clone, Debug)]
pub struct BotWorld {
    mesh: Arc<Mesh>,
    spawns: [Vec<Spawn>; 2],
    scenario: Scenario,
    bots: BTreeMap<u32, Bot>,
    next_identity: u32,
    next_name: Option<usize>,
    tick_interval: f32,
    respawn_waves: [f32; 2],
    configuration: Option<Configuration>,
    next_quota_think: f32,
    path_scratch: PathScratch,
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
}

impl BotWorld {
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
                let yaw_degrees = vector(entity, b"angles").map_or(0.0, |angles| angles[1]);
                spawns[team_index(team)].push(Spawn {
                    position,
                    yaw_degrees,
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
        Ok(Self {
            mesh: Arc::new(mesh),
            spawns,
            scenario,
            bots: BTreeMap::new(),
            next_identity: crate::PLAYER_IDENTITY + 1,
            next_name: None,
            tick_interval,
            respawn_waves: initial_respawn_waves(graph),
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
        for bot in self.bots.values_mut() {
            let candidates = &self.spawns[team_index(bot.team)];
            if candidates.is_empty() {
                return Err(Error::MissingSpawn(bot.team));
            }
            let choice = random
                .random_int(
                    0,
                    i32::try_from(candidates.len() - 1).map_err(|_| Error::Limit)?,
                )
                .map_err(|_| Error::Limit)? as usize;
            respawn_bot(
                bot,
                candidates[choice],
                &self.mesh,
                tick,
                self.tick_interval,
            );
        }
        Ok(())
    }

    pub fn combat_targets(&self) -> impl Iterator<Item = CombatTarget> + '_ {
        self.bots
            .values()
            .filter(|bot| bot.lifecycle == PlayerLifecycle::Active)
            .map(|bot| CombatTarget {
                identity: bot.identity,
                class: bot.class,
                team: bot.team,
                position: bot.movement.position,
                velocity: bot.movement.velocity,
                burning: bot.afterburn.is_some(),
            })
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
        } else {
            bot.afterburn = Some(crate::pyro::Afterburn::ignite(
                bot.afterburn,
                bot.class,
                attacker,
                21,
                now,
            ));
        }
        true
    }

    pub fn extinguish(&mut self, identity: u32) -> bool {
        self.bots
            .get_mut(&identity)
            .and_then(|bot| bot.afterburn.take())
            .is_some()
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
            let choice = random
                .random_int(
                    0,
                    i32::try_from(candidates.len() - 1).map_err(|_| Error::Limit)?,
                )
                .map_err(|_| Error::Limit)? as usize;
            let spawn = candidates[choice];
            let class = request
                .class
                .or_else(|| preset_spawn_class(&self.bots, team, human_team, human_class))
                .ok_or(Error::InvalidEntity)?;
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
                    damagers: crate::deathnotice::DamagerHistory::default(),
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
                    yaw_degrees: spawn.yaw_degrees,
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
                    goal,
                    loadout,
                    active_weapon,
                    last_fire_tick: None,
                    pending_melee: None,
                    respawn_tick: None,
                    carrying_flag: None,
                    shots: 0,
                    hits: 0,
                    kills: 0,
                    deaths: 0,
                    captures: 0,
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
        objectives: Option<&crate::ctf::World>,
    ) -> Result<Vec<Attack>, Error> {
        if self.bots.is_empty() {
            return Ok(Vec::new());
        }
        let now = tick as f32 * self.tick_interval;
        for bot in self.bots.values_mut() {
            if let Some(burn) = bot.afterburn.as_mut() {
                if let Some(damage) = burn.advance(now)
                    && !bot.conditions.is_invulnerable()
                {
                    bot.health.current = bot
                        .health
                        .current
                        .saturating_sub((damage + 0.5) as i32)
                        .max(0);
                }
                if burn.duration <= 0.0 || bot.health.current == 0 || bot.movement.water_level >= 2
                {
                    bot.afterburn = None;
                }
                if bot.health.current == 0 {
                    bot.lifecycle = PlayerLifecycle::Dying;
                }
            }
        }
        let actors = self.actors(human, tick);
        let mut attacks = Vec::new();
        let mut supply_cache = SupplyCache::default();
        let mut activities = Vec::<ActivityEvent>::new();
        let mut ammo = Vec::<AmmoEvent>::new();
        let mesh = &self.mesh;
        let scenario = &mut self.scenario;
        let spawns = &self.spawns;
        for bot in self.bots.values_mut() {
            if bot.lifecycle != PlayerLifecycle::Active {
                if bot.respawn_tick.is_some_and(|due| tick >= due) {
                    let candidates = &spawns[team_index(bot.team)];
                    let choice = random
                        .random_int(
                            0,
                            i32::try_from(candidates.len() - 1).map_err(|_| Error::Limit)?,
                        )
                        .map_err(|_| Error::Limit)? as usize;
                    respawn_bot(bot, candidates[choice], mesh, tick, self.tick_interval);
                }
                continue;
            }
            let maintenance_due = tick >= bot.next_target_tick;
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
            select_weapon(bot, threat, tick, self.tick_interval);
            let authoritative =
                objectives.and_then(|world| world.bot_objective(bot.identity, bot.team));
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
            let (objective_kind, goal) = if let Some(supply) = selected_supply {
                supply
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
                let start = mesh
                    .nearest_area(bot.movement.position)
                    .map(|area| area.identity);
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
                                        team: bot.team,
                                        bot_identity: bot.identity,
                                        now: tick as f32 * self.tick_interval,
                                        route: if matches!(
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
                    bot.current_area = Some(start);
                }
                let (minimum, maximum) = match bot.objective {
                    ObjectiveKind::PayloadPush => (0.2, 0.4),
                    ObjectiveKind::PayloadGuard => (0.5, 1.0),
                    ObjectiveKind::FetchFlag | ObjectiveKind::DeliverFlag => (1.0, 2.0),
                    ObjectiveKind::Attack | ObjectiveKind::GetHealth | ObjectiveKind::GetAmmo => {
                        (0.3, 0.5)
                    }
                };
                bot.next_repath_tick =
                    tick + ticks(random.random_float(minimum, maximum), self.tick_interval);
            }
            while bot.path_index + 1 < bot.path.len() {
                let next = mesh
                    .area(bot.path[bot.path_index + 1])
                    .ok_or(Error::InvalidEntity)?;
                if next.contains_xy(bot.movement.position)
                    && (bot.movement.position[2]
                        - next.height(bot.movement.position[0], bot.movement.position[1]))
                    .abs()
                        <= MAX_JUMP_HEIGHT
                {
                    bot.path_index += 1;
                    bot.current_area = Some(next.identity);
                } else {
                    break;
                }
            }
            let waypoint = if bot.path_index + 1 < bot.path.len() {
                let from = mesh
                    .area(bot.path[bot.path_index])
                    .ok_or(Error::InvalidEntity)?;
                let next = mesh
                    .area(bot.path[bot.path_index + 1])
                    .ok_or(Error::InvalidEntity)?;
                let direction = Direction::ALL
                    .into_iter()
                    .find(|direction| {
                        from.connections[*direction as usize].contains(&next.identity)
                    })
                    .ok_or(Error::InvalidEntity)?;
                let portal =
                    mesh.closest_point_in_portal(from, next, direction, bot.movement.position);
                let center = next.center();
                [
                    portal[0] + (center[0] - portal[0]).clamp(-STEP_HEIGHT, STEP_HEIGHT),
                    portal[1] + (center[1] - portal[1]).clamp(-STEP_HEIGHT, STEP_HEIGHT),
                    next.height(portal[0], portal[1]),
                ]
            } else {
                bot.goal
            };
            let delta = crate::sub(waypoint, bot.movement.position);
            let planar = delta[0].hypot(delta[1]);
            if let Some(threat) = threat {
                let toward = crate::sub(aim_point(world, bot, threat), bot_eye(bot));
                bot.yaw_degrees = toward[1].atan2(toward[0]).to_degrees();
                bot.pitch_degrees = (-toward[2]).atan2(toward[0].hypot(toward[1])).to_degrees();
            } else if planar > 0.0 {
                bot.yaw_degrees = delta[1].atan2(delta[0]).to_degrees();
                bot.pitch_degrees = 0.0;
            }
            let policy = MovementPolicy {
                class: bot.class,
                modifiers: MovementModifiers::default(),
            }
            .resolve();
            let should_move = planar > 5.0;
            let move_yaw = if planar > 0.0 {
                delta[1].atan2(delta[0])
            } else {
                bot.yaw_degrees.to_radians()
            };
            let relative = move_yaw - bot.yaw_degrees.to_radians();
            let movement = step(
                world,
                bot.movement,
                StepInput {
                    command_number: u32::try_from(tick).unwrap_or(u32::MAX),
                    command: MoveCommand {
                        forward: if should_move {
                            policy.maximum_speed * relative.cos()
                        } else {
                            0.0
                        },
                        side: if should_move {
                            policy.maximum_speed * relative.sin()
                        } else {
                            0.0
                        },
                        yaw_degrees: bot.yaw_degrees,
                        jump: delta[2] >= STEP_HEIGHT && delta[2] < MAX_JUMP_HEIGHT,
                        crouch: false,
                    },
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

            if let Some((due, target, weapon)) = bot.pending_melee
                && tick > due
            {
                bot.pending_melee = None;
                if let Some(victim) = actors.get(target).filter(|actor| actor.alive)
                    && distance(bot.movement.position, victim.position)
                        <= ballistics::MELEE_RANGE + ballistics::MELEE_HULL_RADIUS
                {
                    attacks.push(Attack {
                        attacker: bot.identity,
                        team: bot.team,
                        weapon,
                        target,
                        position: bot.movement.position,
                        eye_position: bot_eye(bot),
                        pitch_degrees: bot.pitch_degrees,
                        yaw_degrees: bot.yaw_degrees,
                        seconds_since_previous_shot: f32::INFINITY,
                        damage_multiplier: 1.0,
                        spread_multiplier: 1.0,
                    });
                }
            }
            let Some(active_weapon) = bot.active_weapon else {
                continue;
            };
            let in_range = bot.spy.is_none_or(|spy| spy.can_attack(now))
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
            let primary = state.primary(tick, self.tick_interval, in_range, false, &mut activities);
            if matches!(primary, PrimaryResult::Fired { .. }) {
                let previous = bot.last_fire_tick.replace(tick);
                bot.shots = bot.shots.saturating_add(1);
                let elapsed = previous.map_or(f32::INFINITY, |value| {
                    tick.saturating_sub(value) as f32 * self.tick_interval
                });
                if is_melee(active_weapon) {
                    bot.pending_melee = Some((
                        tick + (ballistics::MELEE_SMACK_DELAY / self.tick_interval).floor() as u64,
                        threat.unwrap().identity,
                        active_weapon,
                    ));
                } else {
                    let (damage_multiplier, spread_multiplier) = if active_weapon == Weapon::Minigun
                    {
                        state.minigun_penalties(tick, self.tick_interval)
                    } else {
                        (1.0, 1.0)
                    };
                    attacks.push(Attack {
                        attacker: bot.identity,
                        team: bot.team,
                        weapon: active_weapon,
                        target: threat.unwrap().identity,
                        position: bot.movement.position,
                        eye_position,
                        pitch_degrees: bot.pitch_degrees,
                        yaw_degrees: bot.yaw_degrees,
                        seconds_since_previous_shot: elapsed,
                        damage_multiplier,
                        spread_multiplier,
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

    pub fn advance_health(&mut self, now: f32) -> Result<(), Error> {
        for bot in self.bots.values_mut() {
            if bot.lifecycle == PlayerLifecycle::Active {
                bot.health
                    .advance(
                        now,
                        self.tick_interval,
                        HealthConfiguration::default(),
                        1.0,
                        1.0,
                        1.0,
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
    ) -> Option<(u32, f32, [f32; 3])> {
        self.bots
            .values()
            .filter(|bot| {
                bot.identity != excluded
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
                let maximum = bot.class.data().maximum_health;
                if bot.health.current >= maximum && bot.afterburn.is_none() {
                    return None;
                }
                let requested = (maximum as f32 * definition.size.ratio()).ceil() as i32;
                let before = bot.health.current;
                if bot.health.current < maximum {
                    bot.health.current = bot.health.current.saturating_add(requested).min(maximum);
                }
                bot.afterburn = None;
                Some((bot.health.current - before) as u16)
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
        bot.health.current = bot.class.data().maximum_health.max(bot.health.current);
        bot.afterburn = None;
        bot.ammo = bot.class.data().maximum_ammo;
        for state in bot.loadout.values_mut() {
            state.regenerate(tick, self.tick_interval);
        }
        bot.next_regenerate_tick = tick + ticks(3.0, self.tick_interval);
        true
    }

    pub fn contains(&self, identity: u32) -> bool {
        self.bots.contains_key(&identity)
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
        team_population: usize,
    ) -> Result<Option<i32>, Error> {
        let Some(victim) = self.bots.get_mut(&input.victim) else {
            return Ok(None);
        };
        let Some(damage_type) = weapon_damage_type(input.weapon) else {
            return Ok(None);
        };
        let result = damage::apply_damage(
            victim.lifecycle == PlayerLifecycle::Active,
            &mut victim.health,
            &mut victim.conditions,
            &DamageInput {
                attacker: input.attacker,
                attacker_team,
                attacker_conditions: ConditionState::default(),
                source: DamageSourceKind::Player,
                weapon_position: None,
                victim: input.victim,
                victim_team: victim.team,
                base_damage: input.amount,
                range_multiplier: 1.0,
                damage_type,
                custom: CustomDamage::None,
                crit: CritKind::None,
                friendly_fire: false,
                force_friendly_fire: false,
                bypass_invulnerability: false,
                force: [0.0; 3],
            },
            DamageModifiers::default(),
        )
        .map_err(|_| Error::Damage)?;
        if !result.admitted {
            return Ok(None);
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
            victim.deaths = victim.deaths.saturating_add(1);
            victim.killstreak = 0;
            victim.respawn_tick = Some(next_respawn_wave(
                tick,
                self.tick_interval,
                self.respawn_waves[team_index(victim.team)],
                team_population,
            ));
            victim.target = None;
            victim.pending_melee = None;
            victim.carrying_flag = None;
        }
        if input.attacker != crate::PLAYER_IDENTITY
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
        Ok(Some(result.health_damage))
    }

    pub fn health(&self, identity: u32) -> Option<i32> {
        self.bots.get(&identity).map(|bot| bot.health.current)
    }

    pub fn select_spawn(
        &self,
        team: PlayerTeam,
        random: &mut UniformRandomStream,
    ) -> Option<[f32; 3]> {
        let candidates = self.spawns.get(team_index(team))?;
        let maximum = i32::try_from(candidates.len().checked_sub(1)?).ok()?;
        let index = usize::try_from(random.random_int(0, maximum).ok()?).ok()?;
        Some(candidates.get(index)?.position)
    }

    pub fn respawn_tick(&self, team: PlayerTeam, tick: u64) -> u64 {
        next_respawn_wave(
            tick,
            self.tick_interval,
            self.respawn_waves[team_index(team)],
            self.team_population(team, team),
        )
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
                equipped_items: Vec::new(),
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
                deaths: bot.deaths,
                captures: bot.captures,
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
fn select_weapon(bot: &mut Bot, threat: Option<Actor>, tick: u64, interval: f32) {
    let Some(primary) = crate::default_weapon(bot.class) else {
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
        if let Some(previous) = bot
            .active_weapon
            .and_then(|weapon| bot.loadout.get_mut(&weapon))
        {
            previous.abort_reload();
            previous.charge_begin_tick = None;
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
        | Weapon::Bonesaw => DamageType::MELEE,
        Weapon::Flamethrower => DamageType::BURN | DamageType::IGNITE,
        Weapon::RocketLauncher
        | Weapon::Original
        | Weapon::StickybombLauncher
        | Weapon::GrenadeLauncher => DamageType::BLAST,
        Weapon::Scattergun | Weapon::Shotgun | Weapon::HeavyShotgun | Weapon::EngineerShotgun => {
            DamageType::BUCKSHOT
        }
        Weapon::Pistol
        | Weapon::Minigun
        | Weapon::SniperRifle
        | Weapon::Smg
        | Weapon::EngineerPistol
        | Weapon::Revolver
        | Weapon::SyringeGun => DamageType::BULLET,
        Weapon::Sapper
        | Weapon::DisguiseKit
        | Weapon::InvisibilityWatch
        | Weapon::BuildPda
        | Weapon::DestroyPda
        | Weapon::Toolbox
        | Weapon::MediGun => return None,
    })
}

fn initial_respawn_waves(graph: &Graph) -> [f32; 2] {
    let mut waves = [RESPAWN_WAVE_SECONDS; 2];
    for entity in &graph.entities {
        if !classname(entity, b"logic_auto") {
            continue;
        }
        for connection in &entity.connections {
            if let playsrc_entity::Connection::Parsed {
                output,
                input,
                parameter,
                delay_bits,
                ..
            } = connection
                && output.eq_ignore_ascii_case(b"OnMultiNewMap")
                && f32::from_bits(*delay_bits) == 0.0
            {
                let index = if input.eq_ignore_ascii_case(b"SetRedTeamRespawnWaveTime") {
                    Some(0)
                } else if input.eq_ignore_ascii_case(b"SetBlueTeamRespawnWaveTime") {
                    Some(1)
                } else {
                    None
                };
                if let Some(index) = index
                    && let Ok(text) = std::str::from_utf8(parameter)
                    && let Ok(value) = text.parse::<f32>()
                    && value.is_finite()
                    && value >= 0.0
                {
                    waves[index] = value;
                }
            }
        }
    }
    waves
}
fn next_respawn_wave(death_tick: u64, interval: f32, configured: f32, population: usize) -> u64 {
    let earliest = death_tick + ticks(DEATH_ANIMATION_SECONDS + configured, interval);
    let scalar = (0.25 + ((population as f32 - 1.0) / 7.0).clamp(0.0, 1.0) * 0.75).clamp(0.25, 1.0);
    let duration = if configured > 5.0 {
        (configured * scalar).max(5.0)
    } else {
        configured
    };
    if duration <= 0.0 {
        return earliest;
    }
    let wave = ticks(duration, interval);
    earliest.div_ceil(wave) * wave
}
fn respawn_bot(bot: &mut Bot, spawn: Spawn, mesh: &Mesh, tick: u64, interval: f32) {
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
    bot.damagers = crate::deathnotice::DamagerHistory::default();
    bot.health =
        HealthState::spawn(bot.class, 0.0, 0.0).expect("authored bot class health is valid");
    bot.conditions = ConditionState::default();
    bot.spy = (bot.class == PlayerClass::Spy).then(crate::spy::SpyState::default);
    bot.ammo = bot.class.data().maximum_ammo;
    bot.next_regenerate_tick = 0;
    bot.yaw_degrees = spawn.yaw_degrees;
    bot.pitch_degrees = 0.0;
    bot.target = None;
    bot.known_since.clear();
    bot.next_target_tick = tick;
    bot.next_repath_tick = tick;
    bot.current_area = mesh.nearest_area(spawn.position).map(|area| area.identity);
    bot.path.clear();
    bot.path_index = 0;
    bot.active_weapon = crate::default_weapon(bot.class);
    bot.pending_melee = None;
    bot.respawn_tick = None;
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
    Some(cost * preference)
}

fn objective(
    scenario: Scenario,
    team: PlayerTeam,
    threat: Option<[f32; 3]>,
    carrying_flag: Option<PlayerTeam>,
) -> (ObjectiveKind, [f32; 3]) {
    match scenario {
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

    fn capture_graph() -> Graph {
        playsrc_entity::parse(
            b"{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}\n{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}\n{\"classname\"\"item_teamflag\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}\n{\"classname\"\"item_teamflag\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}\n{\"classname\"\"func_capturezone\"\"TeamNum\"\"2\"\"model\"\"*1\"\"origin\"\"20 50 1\"}\n{\"classname\"\"func_capturezone\"\"TeamNum\"\"3\"\"model\"\"*2\"\"origin\"\"240 50 1\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap()
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
                Some(&objectives),
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
                attacker: 3,
                victim: 2,
                weapon: Weapon::Bonesaw,
                amount: 65.0,
                position: [0.0; 3],
            },
            PlayerTeam::Blue,
            100,
            1,
        );
        assert_eq!(result, Ok(None));
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
                Some(&objectives),
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
        assert!(world.apply_flame_contact(identity, 1, 1.0, 13.0));
        assert_eq!(world.snapshots()[0].health, 112);
        assert!(world.combat_targets().next().unwrap().burning);
        assert!(!world.apply_flame_contact(identity, 1, 1.04, 13.0));
        assert!(world.apply_flame_contact(identity, 1, 1.08, 13.0));
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
                    attacker: 3,
                    victim: 2,
                    weapon: Weapon::RocketLauncher,
                    amount: 130.0,
                    position: [200.0, 50.0, 1.0],
                },
                PlayerTeam::Red,
                50,
                1,
            )
            .unwrap();
        assert_eq!(points, Some(130));
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
        let due = dead.respawn_tick.unwrap();
        assert_eq!(due, next_respawn_wave(50, 0.015, 10.0, 1));
        world
            .advance(&Floor, due - 1, human_far(), &[], &mut random, None)
            .unwrap();
        assert_eq!(world.snapshots()[0].lifecycle, PlayerLifecycle::Dying);
        world
            .advance(&Floor, due, human_far(), &[], &mut random, None)
            .unwrap();
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
            b"{\"classname\"\"logic_auto\"\"OnMultiNewMap\"\"gamerules,SetRedTeamRespawnWaveTime,9,0,-1\"\"OnMultiNewMap\"\"gamerules,SetBlueTeamRespawnWaveTime,4,0,-1\"}\0",
            playsrc_entity::Limits::default(),
        ).unwrap();
        assert_eq!(initial_respawn_waves(&graph), [9.0, 4.0]);
        assert_eq!(next_respawn_wave(0, 0.01, 4.0, 1), 1200);
        assert_eq!(next_respawn_wave(0, 0.01, 9.0, 1), 2000);
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
                    attacker: 1,
                    victim: 2,
                    weapon: Weapon::Shotgun,
                    amount: 200.0,
                    position: [10.0, 50.0, 1.0],
                },
                PlayerTeam::Red,
                21,
                1,
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
