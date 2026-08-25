use std::{collections::BTreeMap, sync::Arc};

use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph};
use playsrc_movement::{Command as MoveCommand, Configuration, Player, State, StepInput, step};
use playsrc_nav::{Area, Direction, Mesh};

use crate::{
    GameplayWorld, MovementModifiers, MovementPolicy, PlayerClass, PlayerLifecycle, PlayerTeam,
    UniformRandomStream,
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
    pub health: i32,
    pub maximum_health: i32,
    pub target: Option<u32>,
    pub area: Option<u32>,
    pub remaining_path_areas: u32,
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
enum Scenario {
    Payload {
        cart: [f32; 3],
        forward: [f32; 3],
    },
    CaptureTheFlag {
        red_flag: [f32; 3],
        blue_flag: [f32; 3],
        red_capture: [f32; 3],
        blue_capture: [f32; 3],
    },
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
    health: i32,
    afterburn: Option<crate::pyro::Afterburn>,
    last_flame_damage_time: f32,
    objective: ObjectiveKind,
    target: Option<u32>,
    visible_since: Option<u64>,
    next_target_tick: u64,
    next_repath_tick: u64,
    current_area: Option<u32>,
    path: Vec<u32>,
    path_index: usize,
    goal: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct BotWorld {
    mesh: Arc<Mesh>,
    spawns: [Vec<Spawn>; 2],
    scenario: Scenario,
    bots: BTreeMap<u32, Bot>,
    next_identity: u32,
    tick_interval: f32,
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

    pub fn apply_damage(&mut self, identity: u32, damage: f32) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else {
            return false;
        };
        if bot.lifecycle != PlayerLifecycle::Active || damage <= 0.0 || !damage.is_finite() {
            return false;
        }
        bot.health = bot.health.saturating_sub((damage + 0.5) as i32).max(0);
        if bot.health == 0 {
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
            || now - bot.last_flame_damage_time < crate::pyro::FLAME_BURN_FREQUENCY
        {
            return false;
        }
        bot.last_flame_damage_time = now;
        bot.health = bot.health.saturating_sub((damage + 0.5) as i32).max(0);
        if bot.health == 0 {
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
            let (objective, goal) = objective(self.scenario, team, None);
            let identity = self.next_identity;
            self.next_identity = self.next_identity.checked_add(1).ok_or(Error::Limit)?;
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
                    health: class.data().maximum_health,
                    afterburn: None,
                    last_flame_damage_time: f32::NEG_INFINITY,
                    objective,
                    target: None,
                    visible_since: None,
                    next_target_tick: 0,
                    next_repath_tick: 0,
                    current_area: self
                        .mesh
                        .nearest_area(spawn.position)
                        .map(|area| area.identity),
                    path: Vec::new(),
                    path_index: 0,
                    goal,
                },
            );
        }
        Ok(())
    }

    pub fn advance<W: GameplayWorld>(
        &mut self,
        world: &W,
        tick: u64,
        human_team: PlayerTeam,
        human_alive: bool,
        human_position: [f32; 3],
        random: &mut UniformRandomStream,
    ) -> Result<(), Error> {
        if self.bots.is_empty() {
            return Ok(());
        }
        let now = tick as f32 * self.tick_interval;
        for bot in self.bots.values_mut() {
            if let Some(burn) = bot.afterburn.as_mut() {
                if let Some(damage) = burn.advance(now) {
                    bot.health = bot.health.saturating_sub((damage + 0.5) as i32).max(0);
                }
                if burn.duration <= 0.0 || bot.health == 0 || bot.movement.water_level >= 2 {
                    bot.afterburn = None;
                }
                if bot.health == 0 {
                    bot.lifecycle = PlayerLifecycle::Dying;
                }
            }
        }
        let actors: Vec<_> = std::iter::once((
            crate::PLAYER_IDENTITY,
            human_team,
            human_alive,
            human_position,
        ))
        .chain(self.bots.values().map(|bot| {
            (
                bot.identity,
                bot.team,
                bot.lifecycle == PlayerLifecycle::Active,
                bot.movement.position,
            )
        }))
        .collect();
        let mesh = &self.mesh;
        for bot in self.bots.values_mut() {
            if bot.lifecycle != PlayerLifecycle::Active {
                continue;
            }
            if tick >= bot.next_target_tick {
                bot.next_target_tick = tick + ticks(TARGET_SELECTION_INTERVAL, self.tick_interval);
                let visible = actors
                    .iter()
                    .filter(|(identity, team, alive, _)| {
                        *identity != bot.identity && *alive && *team != bot.team
                    })
                    .filter_map(|(identity, _, _, position)| {
                        let d = distance(bot.movement.position, *position);
                        if d > MAX_VISION_RANGE {
                            return None;
                        }
                        let eye = [
                            bot.movement.position[0],
                            bot.movement.position[1],
                            bot.movement.position[2] + 68.0,
                        ];
                        let target = [position[0], position[1], position[2] + 68.0];
                        let trace = world
                            .trace(
                                eye,
                                target,
                                Hull {
                                    mins: [0.0; 3],
                                    maxs: [0.0; 3],
                                },
                                0x0000_400b,
                            )
                            .ok()?;
                        (trace.fraction >= 1.0).then_some((*identity, d, *position))
                    })
                    .min_by(|left, right| {
                        left.1
                            .total_cmp(&right.1)
                            .then_with(|| left.0.cmp(&right.0))
                    });
                match visible {
                    Some((identity, _, _)) if bot.target == Some(identity) => {}
                    Some((identity, _, _)) => {
                        if bot.visible_since.is_none() {
                            bot.visible_since = Some(tick);
                        }
                        if tick.saturating_sub(bot.visible_since.unwrap())
                            >= ticks(bot.difficulty.recognition_seconds(), self.tick_interval)
                        {
                            bot.target = Some(identity);
                        }
                    }
                    None => {
                        bot.target = None;
                        bot.visible_since = None;
                    }
                }
            }
            let threat_position = bot.target.and_then(|target| {
                actors
                    .iter()
                    .find(|(identity, _, _, _)| *identity == target)
                    .map(|(_, _, _, position)| *position)
            });
            (bot.objective, bot.goal) = objective(self.scenario, bot.team, threat_position);
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
                }
                let (minimum, maximum) = if bot.team == PlayerTeam::Blue {
                    (0.2, 0.4)
                } else {
                    (0.5, 1.0)
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
            let delta = [
                waypoint[0] - bot.movement.position[0],
                waypoint[1] - bot.movement.position[1],
                waypoint[2] - bot.movement.position[2],
            ];
            let planar = delta[0].hypot(delta[1]);
            if planar > 0.0 {
                bot.yaw_degrees = delta[1].atan2(delta[0]).to_degrees();
            }
            let policy = MovementPolicy {
                class: bot.class,
                modifiers: MovementModifiers::default(),
            }
            .resolve();
            let should_move = planar > 5.0;
            let jump = delta[2] >= STEP_HEIGHT && delta[2] < MAX_JUMP_HEIGHT;
            let movement = step(
                world,
                bot.movement,
                StepInput {
                    command_number: u32::try_from(tick).unwrap_or(u32::MAX),
                    command: MoveCommand {
                        forward: if should_move {
                            policy.maximum_speed
                        } else {
                            0.0
                        },
                        side: 0.0,
                        yaw_degrees: bot.yaw_degrees,
                        jump,
                        crouch: false,
                    },
                    pitch_degrees: 0.0,
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
        }
        Ok(())
    }

    pub fn damage(&mut self, identity: u32, amount: f32) -> bool {
        let Some(bot) = self.bots.get_mut(&identity) else {
            return false;
        };
        if bot.lifecycle != PlayerLifecycle::Active || !amount.is_finite() || amount <= 0.0 {
            return false;
        }
        bot.health = bot.health.saturating_sub((amount + 0.5) as i32).max(0);
        if bot.health == 0 {
            bot.lifecycle = PlayerLifecycle::Dying;
            bot.target = None;
        }
        true
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
                health: bot.health,
                maximum_health: bot.class.data().maximum_health,
                target: bot.target,
                area: bot.current_area,
                remaining_path_areas: bot.path.len().saturating_sub(bot.path_index) as u32,
            })
            .collect()
    }
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
        Scenario::CaptureTheFlag {
            red_flag,
            blue_flag,
            red_capture,
            blue_capture,
        } => {
            let _ = (red_capture, blue_capture);
            (
                ObjectiveKind::FetchFlag,
                if team == PlayerTeam::Red {
                    blue_flag
                } else {
                    red_flag
                },
            )
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
    let mut flags: [Option<[f32; 3]>; 2] = [None, None];
    let mut captures: [Option<[f32; 3]>; 2] = [None, None];
    for entity in &graph.entities {
        let array = if classname(entity, b"item_teamflag") {
            &mut flags
        } else if classname(entity, b"func_capturezone") {
            &mut captures
        } else {
            continue;
        };
        let team = match scalar(entity, b"TeamNum") {
            Some(b"2") => PlayerTeam::Red,
            Some(b"3") => PlayerTeam::Blue,
            _ => continue,
        };
        array[team_index(team)] = vector(entity, b"origin");
    }
    match (flags, captures) {
        ([Some(red_flag), Some(blue_flag)], [Some(red_capture), Some(blue_capture)]) => {
            Ok(Scenario::CaptureTheFlag {
                red_flag,
                blue_flag,
                red_capture,
                blue_capture,
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
            objective(scenario, PlayerTeam::Blue, None),
            (ObjectiveKind::PayloadPush, [40.0, 200.0, 32.0])
        );
        assert_eq!(
            objective(scenario, PlayerTeam::Blue, Some([100.0, 100.0, 0.0])),
            (ObjectiveKind::PayloadPush, [100.0, 260.0, 32.0])
        );
        assert_eq!(
            objective(scenario, PlayerTeam::Red, None),
            (ObjectiveKind::PayloadGuard, [100.0, 200.0, 32.0])
        );
    }

    #[test]
    fn flame_contact_afterburn_airblast_and_fatal_hits_mutate_real_bot_players() {
        let mut world = BotWorld::new(fixture_mesh(), &fixture_graph(), &Floor, 0.015).unwrap();
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
                    PlayerTeam::Red,
                    true,
                    [190.0, 50.0, 1.0],
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
}
