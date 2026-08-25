use std::{collections::BTreeMap, sync::Arc};

use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph};
use playsrc_movement::{Command as MoveCommand, Configuration, Player, State, StepInput, step};
use playsrc_nav::{Area, Direction, Mesh};

use crate::{
    GameplayWorld, MovementModifiers, MovementPolicy, PlayerClass, PlayerLifecycle, PlayerTeam,
    UniformRandomStream, Weapon, WeaponState, ballistics,
    condition::ConditionState,
    damage::{
        self, CritKind, CustomDamage, DamageInput, DamageModifiers, DamageSourceKind, DamageType,
    },
    health::HealthState,
    weapon::{ActivityEvent, AmmoEvent, PrimaryResult, WeaponRuntime},
};

pub const MAX_BOTS: usize = 31;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ObjectiveKind {
    PayloadPush = 1,
    PayloadGuard = 2,
    FetchFlag = 3,
    DeliverFlag = 4,
    Attack = 5,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub identity: u32,
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
    pub carrying_flag: bool,
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
struct Spawn {
    position: [f32; 3],
    yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptureZone {
    position: [f32; 3],
    model: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Flag {
    home: [f32; 3],
    position: [f32; 3],
    carrier: Option<u32>,
    return_tick: Option<u64>,
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

#[derive(Clone, Debug)]
struct Bot {
    identity: u32,
    class: PlayerClass,
    team: PlayerTeam,
    lifecycle: PlayerLifecycle,
    difficulty: Difficulty,
    movement: State,
    yaw_degrees: f32,
    pitch_degrees: f32,
    health: i32,
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
}

#[derive(Clone, Debug)]
pub struct BotWorld {
    mesh: Arc<Mesh>,
    spawns: [Vec<Spawn>; 2],
    scenario: Scenario,
    bots: BTreeMap<u32, Bot>,
    next_identity: u32,
    tick_interval: f32,
    respawn_waves: [f32; 2],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    MissingScenario,
    MissingPayload,
    MissingPathTrack,
    MissingSpawn(PlayerTeam),
    InvalidEntity,
    Limit,
    Movement,
    Damage,
}

impl BotWorld {
    pub fn new<W: GameplayWorld>(
        mut mesh: Mesh,
        graph: &Graph,
        world: &W,
        tick_interval: f32,
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
                        .map_err(|_| Error::Movement)?
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
        let scenario = scenario(graph)?;
        Ok(Self {
            mesh: Arc::new(mesh),
            spawns,
            scenario,
            bots: BTreeMap::new(),
            next_identity: crate::PLAYER_IDENTITY + 1,
            tick_interval,
            respawn_waves: initial_respawn_waves(graph),
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
            self.bots.insert(
                identity,
                Bot {
                    identity,
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
                    health: class.data().maximum_health,
                    objective,
                    target: None,
                    known_since: BTreeMap::new(),
                    next_target_tick: 0,
                    next_repath_tick: 0,
                    current_area: self
                        .mesh
                        .nearest_area(spawn.position)
                        .map(|area| area.identity),
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
                },
            );
        }
        Ok(())
    }

    pub fn advance<W: GameplayWorld>(
        &mut self,
        world: &W,
        tick: u64,
        human: Human,
        random: &mut UniformRandomStream,
    ) -> Result<Vec<Attack>, Error> {
        if self.bots.is_empty() {
            return Ok(Vec::new());
        }
        self.return_expired_flags(tick);
        let actors = self.actors(human, tick);
        let mut attacks = Vec::new();
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
            if tick >= bot.next_target_tick {
                bot.next_target_tick = tick + ticks(TARGET_SELECTION_INTERVAL, self.tick_interval);
                let visible: Vec<_> = actors
                    .iter()
                    .copied()
                    .filter(|actor| {
                        actor.identity != bot.identity
                            && actor.alive
                            && bot.team.is_enemy(actor.team)
                    })
                    .filter(|actor| visible_actor(world, bot, *actor))
                    .collect();
                bot.known_since
                    .retain(|identity, _| visible.iter().any(|actor| actor.identity == *identity));
                for actor in &visible {
                    bot.known_since.entry(actor.identity).or_insert(tick);
                }
                bot.target = visible
                    .into_iter()
                    .filter(|actor| {
                        tick.saturating_sub(bot.known_since[&actor.identity])
                            >= ticks(bot.difficulty.recognition_seconds(), self.tick_interval)
                    })
                    .min_by(|left, right| threat_order(bot, *left, *right))
                    .map(|actor| actor.identity);
            }
            let threat = bot.target.and_then(|identity| {
                actors
                    .iter()
                    .copied()
                    .find(|actor| actor.identity == identity && actor.alive)
            });
            select_weapon(bot, threat, tick, self.tick_interval);
            update_flag_contact(world, scenario, bot, tick)?;
            let (objective_kind, goal) = objective(
                *scenario,
                bot.team,
                threat.map(|actor| actor.position),
                bot.carrying_flag,
            );
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
                        .build_path(start, goal, |from, destination, direction, length| {
                            path_cost(
                                from,
                                destination,
                                direction,
                                length,
                                bot.team,
                                bot.identity,
                                tick as f32 * self.tick_interval,
                            )
                        })
                        .unwrap_or_default();
                    bot.path_index = 0;
                    bot.current_area = Some(start);
                }
                let (minimum, maximum) = match bot.objective {
                    ObjectiveKind::PayloadPush => (0.2, 0.4),
                    ObjectiveKind::PayloadGuard => (0.5, 1.0),
                    ObjectiveKind::FetchFlag | ObjectiveKind::DeliverFlag => (1.0, 2.0),
                    ObjectiveKind::Attack => (0.3, 0.5),
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
                let portal = from.portal(next, direction);
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
                Configuration {
                    tick_interval: self.tick_interval,
                    water_exit_forward: 30.0,
                    water_exit_up_speed: 300.0,
                    ..Configuration::default()
                },
                policy,
            )
            .map_err(|_| Error::Movement)?;
            bot.movement = movement.state;

            if let Some((due, target, weapon)) = bot.pending_melee
                && tick > due
            {
                bot.pending_melee = None;
                if let Some(victim) = actors
                    .iter()
                    .copied()
                    .find(|actor| actor.identity == target && actor.alive)
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
            let in_range = threat.is_some_and(|target| {
                let range = distance(bot.movement.position, target.position);
                range < max_attack_range(active_weapon)
                    && (!is_melee(active_weapon) || range < MELEE_FIRE_RANGE)
                    && line_of_fire_clear(world, bot_eye(bot), target)
            });
            let mut activities = Vec::<ActivityEvent>::new();
            let mut ammo = Vec::<AmmoEvent>::new();
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

    pub fn contains(&self, identity: u32) -> bool {
        self.bots.contains_key(&identity)
    }

    pub fn active(&self, identity: u32) -> bool {
        self.bots
            .get(&identity)
            .is_some_and(|bot| bot.lifecycle == PlayerLifecycle::Active)
    }

    pub fn team(&self, identity: u32) -> Option<PlayerTeam> {
        self.bots.get(&identity).map(|bot| bot.team)
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
        let mut health = HealthState::spawn(victim.class, 0.0, 0.0).map_err(|_| Error::Damage)?;
        health.current = victim.health;
        let mut conditions = ConditionState::default();
        let damage_type = match input.weapon {
            Weapon::Bat | Weapon::Shovel | Weapon::Fists | Weapon::Kukri | Weapon::Wrench => {
                DamageType::MELEE
            }
            Weapon::RocketLauncher | Weapon::Original | Weapon::StickybombLauncher => {
                DamageType::BLAST
            }
            Weapon::Scattergun
            | Weapon::Shotgun
            | Weapon::HeavyShotgun
            | Weapon::EngineerShotgun => DamageType::BUCKSHOT,
            Weapon::Pistol
            | Weapon::Minigun
            | Weapon::SniperRifle
            | Weapon::Smg
            | Weapon::EngineerPistol => DamageType::BULLET,
        };
        let result = damage::apply_damage(
            victim.lifecycle == PlayerLifecycle::Active,
            &mut health,
            &mut conditions,
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
        victim.health = health.current.max(0);
        let killed = result.death.is_some();
        if killed {
            victim.lifecycle = PlayerLifecycle::Dying;
            victim.deaths = victim.deaths.saturating_add(1);
            victim.respawn_tick = Some(next_respawn_wave(
                tick,
                self.tick_interval,
                self.respawn_waves[team_index(victim.team)],
                team_population,
            ));
            victim.target = None;
            victim.pending_melee = None;
            if let Some(flag_team) = victim.carrying_flag.take()
                && let Scenario::CaptureTheFlag { flags, .. } = &mut self.scenario
            {
                let flag = &mut flags[team_index(flag_team)];
                flag.carrier = None;
                flag.position = victim.movement.position;
                flag.return_tick = Some(tick + ticks(FLAG_RETURN_SECONDS, self.tick_interval));
            }
        }
        if input.attacker != crate::PLAYER_IDENTITY
            && let Some(attacker) = self.bots.get_mut(&input.attacker)
        {
            attacker.hits = attacker.hits.saturating_add(1);
            if killed {
                attacker.kills = attacker.kills.saturating_add(1);
            }
        }
        Ok(Some(result.health_damage))
    }

    pub fn team_population(&self, team: PlayerTeam, human_team: PlayerTeam) -> usize {
        self.bots.values().filter(|bot| bot.team == team).count() + usize::from(human_team == team)
    }

    pub fn record_human_hit(&mut self, attacker: u32, killed: bool) {
        if let Some(bot) = self.bots.get_mut(&attacker) {
            bot.hits = bot.hits.saturating_add(1);
            if killed {
                bot.kills = bot.kills.saturating_add(1);
            }
        }
    }

    pub fn snapshots(&self) -> Vec<Snapshot> {
        self.bots
            .values()
            .map(|bot| Snapshot {
                identity: bot.identity,
                class: bot.class,
                team: bot.team,
                lifecycle: bot.lifecycle,
                difficulty: bot.difficulty,
                objective: bot.objective,
                position: bot.movement.position,
                velocity: bot.movement.velocity,
                yaw_degrees: bot.yaw_degrees,
                pitch_degrees: bot.pitch_degrees,
                health: bot.health,
                maximum_health: bot.class.data().maximum_health,
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
                carrying_flag: bot.carrying_flag.is_some(),
                last_fire_tick: bot.last_fire_tick,
                respawn_tick: bot.respawn_tick,
            })
            .collect()
    }

    fn actors(&self, human: Human, tick: u64) -> Vec<Actor> {
        std::iter::once(Actor {
            identity: crate::PLAYER_IDENTITY,
            class: human.class,
            team: human.team,
            alive: human.alive,
            position: human.position,
            velocity: human.velocity,
            firing_at: None,
        })
        .chain(self.bots.values().map(|bot| {
            Actor {
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
            }
        }))
        .collect()
    }

    fn return_expired_flags(&mut self, tick: u64) {
        if let Scenario::CaptureTheFlag { flags, .. } = &mut self.scenario {
            for flag in flags {
                if flag.return_tick.is_some_and(|due| tick >= due) {
                    flag.position = flag.home;
                    flag.return_tick = None;
                }
            }
        }
    }
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
        Weapon::Bat | Weapon::Shovel | Weapon::Fists | Weapon::Kukri | Weapon::Wrench
    )
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
fn update_flag_contact<W: GameplayWorld>(
    world: &W,
    scenario: &mut Scenario,
    bot: &mut Bot,
    tick: u64,
) -> Result<(), Error> {
    let Scenario::CaptureTheFlag { flags, captures } = scenario else {
        return Ok(());
    };
    if let Some(flag_team) = bot.carrying_flag {
        let zone = captures[team_index(bot.team)];
        let touching = if let Some(model) = zone.model {
            world
                .overlaps_model_hull(model, zone.position, bot.movement.position, PLAYER_HULL)
                .map_err(|_| Error::Movement)?
        } else {
            distance(bot.movement.position, zone.position) <= FLAG_TRIGGER_BLOAT
        };
        if touching {
            let flag = &mut flags[team_index(flag_team)];
            flag.position = flag.home;
            flag.carrier = None;
            flag.return_tick = None;
            bot.carrying_flag = None;
            bot.captures = bot.captures.saturating_add(1);
            bot.next_repath_tick = tick;
        } else {
            flags[team_index(flag_team)].position = bot.movement.position;
        }
        return Ok(());
    }
    let enemy = if bot.team == PlayerTeam::Red {
        PlayerTeam::Blue
    } else {
        PlayerTeam::Red
    };
    let flag = &mut flags[team_index(enemy)];
    if flag.carrier.is_none()
        && (bot.movement.position[0] - flag.position[0]).abs() <= 24.0 + 19.5 + FLAG_TRIGGER_BLOAT
        && (bot.movement.position[1] - flag.position[1]).abs() <= 24.0 + 22.5 + FLAG_TRIGGER_BLOAT
        && bot.movement.position[2] <= flag.position[2] + 6.5 + FLAG_TRIGGER_BLOAT
        && bot.movement.position[2] + 82.0 >= flag.position[2] - 6.5 - FLAG_TRIGGER_BLOAT
    {
        flag.carrier = Some(bot.identity);
        flag.position = bot.movement.position;
        flag.return_tick = None;
        bot.carrying_flag = Some(enemy);
        bot.next_repath_tick = tick;
    }
    Ok(())
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
    bot.health = bot.class.data().maximum_health;
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
    team: PlayerTeam,
    bot_identity: u32,
    now: f32,
) -> Option<f32> {
    let flags = destination.game_attributes;
    if flags & TF_NAV_UNBLOCKABLE == 0
        && (flags & TF_NAV_BLOCKED != 0
            || team == PlayerTeam::Red && flags & TF_NAV_BLUE_ONE_WAY_DOOR != 0
            || team == PlayerTeam::Blue && flags & TF_NAV_RED_ONE_WAY_DOOR != 0)
    {
        return None;
    }
    if team == PlayerTeam::Red && flags & TF_NAV_SPAWN_ROOM_BLUE != 0
        || team == PlayerTeam::Blue && flags & TF_NAV_SPAWN_ROOM_RED != 0
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
    let time_mod = (now / 10.0) as i32 + 1;
    let preference = 1.0
        + 50.0
            * (1.0
                + ((bot_identity as i64 * destination.identity as i64 * i64::from(time_mod))
                    as f32)
                    .cos());
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
                if flag.carrier.is_some() {
                    (ObjectiveKind::Attack, threat.unwrap_or(flag.position))
                } else {
                    (ObjectiveKind::FetchFlag, flag.position)
                }
            }
        }
    }
}

fn scenario(graph: &Graph) -> Result<Scenario, Error> {
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
        let Some(position) = vector(entity, b"origin") else {
            continue;
        };
        if classname(entity, b"item_teamflag") {
            flags[team_index(team)] = Some(Flag {
                home: position,
                position,
                carrier: None,
                return_tick: None,
            });
        } else if classname(entity, b"func_capturezone") {
            captures[team_index(team)] = Some(CaptureZone {
                position,
                model: entity.bsp_model_index,
            });
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
            _model: usize,
            _origin: [f32; 3],
            _position: [f32; 3],
            _hull: Hull,
        ) -> Result<bool, MoveError> {
            Ok(false)
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
            b"{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}\n{\"classname\"\"info_player_teamspawn\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}\n{\"classname\"\"item_teamflag\"\"TeamNum\"\"2\"\"origin\"\"10 50 1\"}\n{\"classname\"\"item_teamflag\"\"TeamNum\"\"3\"\"origin\"\"250 50 1\"}\n{\"classname\"\"func_capturezone\"\"TeamNum\"\"2\"\"origin\"\"20 50 1\"}\n{\"classname\"\"func_capturezone\"\"TeamNum\"\"3\"\"origin\"\"240 50 1\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap()
    }

    #[test]
    fn source_difficulty_reaction_thresholds_are_exact() {
        assert_eq!(Difficulty::Easy.recognition_seconds(), 1.0);
        assert_eq!(Difficulty::Normal.recognition_seconds(), 0.5);
        assert_eq!(Difficulty::Hard.recognition_seconds(), 0.3);
        assert_eq!(Difficulty::Expert.recognition_seconds(), 0.2);
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
    fn player_lifecycle_path_movement_target_recognition_and_team_removal_are_deterministic() {
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
                    &mut random,
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
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
    fn preset_payload_rosters_select_exact_offense_and_defense_classes() {
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
                PlayerTeam::Blue,
                2,
                0.0
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
                PlayerTeam::Red,
                2,
                0.0
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
                PlayerTeam::Red,
                2,
                0.0
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
                PlayerTeam::Red,
                2,
                0.0
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
                PlayerTeam::Red,
                2,
                0.0
            ),
            None
        );
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
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
                    .advance(&Floor, tick, human_far(), &mut random)
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
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
            .advance(&Floor, due - 1, human_far(), &mut random)
            .unwrap();
        assert_eq!(world.snapshots()[0].lifecycle, PlayerLifecycle::Dying);
        world
            .advance(&Floor, due, human_far(), &mut random)
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
    fn ctf_flags_pick_up_deliver_drop_and_return_on_the_exact_authored_timers() {
        let mut world = BotWorld::new(fixture_mesh(), &capture_graph(), &Floor, 0.015).unwrap();
        let mut random = UniformRandomStream::from_seed(0).unwrap();
        add(
            &mut world,
            &mut random,
            PlayerTeam::Blue,
            PlayerClass::Scout,
            Difficulty::Normal,
        );
        world.bots.get_mut(&2).unwrap().movement.position = [10.0, 50.0, 1.0];
        world.advance(&Floor, 0, human_far(), &mut random).unwrap();
        let carrier = world.snapshots()[0].clone();
        assert_eq!(carrier.objective, ObjectiveKind::DeliverFlag);
        assert!(carrier.carrying_flag);
        world.bots.get_mut(&2).unwrap().movement.position = [240.0, 50.0, 1.0];
        world.advance(&Floor, 1, human_far(), &mut random).unwrap();
        let delivered = world.snapshots()[0].clone();
        assert_eq!((delivered.captures, delivered.carrying_flag), (1, false));
        assert_eq!(delivered.objective, ObjectiveKind::FetchFlag);

        world.bots.get_mut(&2).unwrap().movement.position = [10.0, 50.0, 1.0];
        world.advance(&Floor, 2, human_far(), &mut random).unwrap();
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
                3,
                1,
            )
            .unwrap();
        let Scenario::CaptureTheFlag { flags, .. } = world.scenario else {
            panic!("capture scenario")
        };
        assert_eq!(flags[0].carrier, None);
        assert_eq!(flags[0].return_tick, Some(3 + ticks(60.0, 0.015)));
        world.return_expired_flags(3 + ticks(60.0, 0.015));
        let Scenario::CaptureTheFlag { flags, .. } = world.scenario else {
            panic!("capture scenario")
        };
        assert_eq!(
            (flags[0].position, flags[0].return_tick),
            (flags[0].home, None)
        );
    }
}
