mod map_runtime;

#[path = "../../rulesets/jump/rust/src/lib.rs"]
pub mod jump;

pub use map_runtime::{
    EntityEvent, EntityEventKind, EntityTransform, GameplayWorld, MapCounts, MapRuntime,
};

use std::collections::BTreeMap;

use playsrc_collision::Hull;
use playsrc_movement::{
    Command as MoveCommand, Configuration as MovementConfiguration, Error as MoveError, Mode,
    ModeRequest, Player, Policy as GenericMovementPolicy, State as MovementState, StepInput,
    StepResult as MovementStepResult, StepStrategy, TransitionDisposition, step,
};

use map_runtime::{BeginTickInput, Effect as MapEffect, MapError, MapPhase};

pub const PLAYER_IDENTITY: u32 = 1;
pub const MAX_PROJECTILES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Class {
    Soldier = 1,
    Demoman = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Team {
    Red = 1,
    Blue = 2,
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
    fn full(weapon: Weapon) -> Self {
        let (maximum_clip, maximum_reserve) = match weapon {
            Weapon::RocketLauncher | Weapon::Original => (4, 20),
            Weapon::StickybombLauncher => (8, 24),
        };
        Self {
            weapon,
            clip: maximum_clip,
            reserve: maximum_reserve,
            maximum_clip,
            maximum_reserve,
            reload: ReloadState::Idle,
            next_primary_tick: 0,
            next_reload_tick: 0,
        }
    }

    fn regenerate(&mut self) {
        self.clip = self.maximum_clip;
        self.reserve = self.maximum_reserve;
        self.reload = ReloadState::Idle;
        self.next_reload_tick = 0;
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
    Burning = 0,
    Urine = 1,
    MadMilk = 2,
    Gas = 3,
    Bleeding = 4,
    EnergyBuff = 5,
    CannotSwitchFromMelee = 6,
    Phase = 7,
    ParachuteActive = 8,
    Plague = 9,
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

#[derive(Clone)]
pub struct Session<W: GameplayWorld + Clone> {
    collision: W,
    tick: u64,
    class: Class,
    team: Team,
    weapon: Weapon,
    loadout: BTreeMap<Weapon, WeaponState>,
    movement: MovementState,
    movement_modifiers: MovementModifiers,
    last_movement: Option<MovementStepResult>,
    health: f32,
    conditions: u32,
    spawn: [f32; 3],
    projectiles: Vec<LiveProjectile>,
    next_projectile: u32,
    movement_configuration: MovementConfiguration,
    map: MapRuntime,
    next_regenerate_tick: u64,
    hurt_next_tick: BTreeMap<u32, u64>,
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
                WeaponState::full(Weapon::RocketLauncher),
            ),
            (Weapon::Original, WeaponState::full(Weapon::Original)),
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
            conditions: 0,
            spawn,
            projectiles: Vec::new(),
            next_projectile: 1,
            movement_configuration: MovementConfiguration::default(),
            map,
            next_regenerate_tick: 0,
            hurt_next_tick: BTreeMap::new(),
            jump: None,
        }
    }

    pub fn set_movement_modifiers(&mut self, modifiers: MovementModifiers) {
        self.movement_modifiers = modifiers;
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
        let mut candidate = self.clone();
        let snapshot = candidate.advance_inner(command)?;
        *self = candidate;
        Ok(snapshot)
    }

    fn advance_inner(&mut self, command: Command) -> Result<Snapshot, Error> {
        let mut events = Vec::new();
        let mut projectile_events = Vec::new();
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
        let mut jump_contacts = Vec::new();
        for _ in 0..4 {
            let phase = self.map.contact_phase(
                &self.collision,
                self.tick,
                self.movement.position,
                self.movement.active_hull(movement_policy),
            )?;
            let discontinuity = self.apply_map_effects(&phase, &mut events, &mut teleported);
            jump_contacts.extend(phase.contacts.iter().copied());
            map_phase.append(phase);
            if !discontinuity {
                break;
            }
        }

        self.advance_reload(command.reload, &mut events);
        if command.fire {
            self.fire(
                command.pitch_degrees,
                command.movement.yaw_degrees,
                &mut projectile_events,
                &mut events,
            )?;
        }
        self.advance_projectiles(
            self.movement_configuration.tick_interval,
            &mut projectile_events,
            &mut events,
        )?;
        if command.detonate {
            self.detonate(&mut projectile_events, &mut events);
        }

        let mut respawned = false;
        if command.respawn || self.health <= 0.0 {
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
                    alive: true,
                    active: true,
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
        Ok(Snapshot {
            tick: self.tick,
            class: self.class,
            team: self.team,
            weapon: self.weapon,
            movement: self.movement,
            health: self.health,
            maximum_health: self.maximum_health(),
            loadout: self.loadout.values().copied().collect(),
            conditions: self.conditions,
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
            events.push(Event::TeamChanged(team));
        }
        if let Some(class) = command.select_class
            && class != self.class
        {
            self.class = class;
            self.weapon = default_weapon(class);
            self.loadout = default_loadout(class);
            self.health = self.maximum_health();
            self.conditions = 0;
            self.fizzle_projectiles(projectile_events);
            events.push(Event::ClassChanged(class));
            events.push(Event::WeaponChanged(self.weapon));
        }
        if let Some(weapon) = command.select_weapon
            && allowed(self.class, weapon)
            && weapon != self.weapon
        {
            self.weapon = weapon;
            self.loadout
                .entry(weapon)
                .or_insert_with(|| WeaponState::full(weapon));
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
                    yaw_degrees,
                } if !discontinuity => {
                    self.movement.position = position;
                    self.movement.ground = None;
                    *teleported = true;
                    discontinuity = true;
                    events.push(Event::Teleported {
                        trigger,
                        destination,
                        position,
                        yaw_degrees,
                    });
                }
                MapEffect::Hurt {
                    trigger,
                    damage_per_second,
                } => {
                    let due = self.hurt_next_tick.get(&trigger).copied().unwrap_or(0);
                    if self.tick >= due {
                        let amount = damage_per_second * 0.5;
                        if amount < 0.0 {
                            let before = self.health;
                            self.health = (self.health - amount).min(self.maximum_health());
                            events.push(Event::Healed {
                                amount: self.health - before,
                                health: self.health,
                                trigger,
                            });
                        } else if amount > 0.0 {
                            self.health = (self.health - amount).max(0.0);
                            events.push(Event::Damaged {
                                amount,
                                health: self.health,
                            });
                        }
                        self.hurt_next_tick.insert(
                            trigger,
                            self.tick + ticks(0.5, self.movement_configuration.tick_interval),
                        );
                    }
                }
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
                MapEffect::Regenerate { entity, team }
                    if self.tick >= self.next_regenerate_tick
                        && team.is_none_or(|team| team == self.team as u8) =>
                {
                    self.regenerate(entity, events);
                }
                _ => {}
            }
        }
        discontinuity
    }

    fn regenerate(&mut self, entity: u32, events: &mut Vec<Event>) {
        self.health = self.maximum_health();
        self.conditions &= !resupply_removed_conditions();
        for weapon in self.loadout.values_mut() {
            weapon.regenerate();
        }
        let active = self.loadout[&self.weapon];
        self.next_regenerate_tick =
            self.tick + ticks(3.0, self.movement_configuration.tick_interval);
        events.push(Event::Resupplied {
            entity,
            health: self.health,
            weapon: self.weapon,
            clip: active.clip,
            reserve: active.reserve,
        });
    }

    fn advance_reload(&mut self, requested: bool, events: &mut Vec<Event>) {
        let tick_interval = self.movement_configuration.tick_interval;
        let state = self
            .loadout
            .get_mut(&self.weapon)
            .expect("active weapon belongs to loadout");
        if state.reload == ReloadState::Idle
            && requested
            && state.clip < state.maximum_clip
            && state.reserve > 0
            && self.tick >= state.next_primary_tick
        {
            state.reload = ReloadState::Starting;
            state.next_reload_tick = self.tick + ticks(0.5, tick_interval);
        }
        if state.reload != ReloadState::Idle && self.tick >= state.next_reload_tick {
            state.clip += 1;
            state.reserve -= 1;
            events.push(Event::Reloaded {
                weapon: state.weapon,
                clip: state.clip,
                reserve: state.reserve,
            });
            if state.clip == state.maximum_clip || state.reserve == 0 {
                state.reload = ReloadState::Idle;
                state.next_reload_tick = 0;
            } else {
                state.reload = ReloadState::Loading;
                state.next_reload_tick = self.tick + ticks(0.8, tick_interval);
            }
        }
    }

    fn fire(
        &mut self,
        pitch: f32,
        yaw: f32,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let state = self
            .loadout
            .get_mut(&self.weapon)
            .expect("active weapon belongs to loadout");
        if self.tick < state.next_primary_tick || state.clip == 0 {
            return Ok(());
        }
        if self.projectiles.len() >= MAX_PROJECTILES {
            return Err(Error::ProjectileLimit);
        }
        state.clip -= 1;
        state.reload = ReloadState::Idle;
        state.next_reload_tick = 0;
        let fire_delay = match self.weapon {
            Weapon::RocketLauncher | Weapon::Original => 0.8,
            Weapon::StickybombLauncher => 0.6,
        };
        state.next_primary_tick =
            self.tick + ticks(fire_delay, self.movement_configuration.tick_interval);
        let kind = match self.weapon {
            Weapon::RocketLauncher | Weapon::Original => ProjectileKind::Rocket,
            Weapon::StickybombLauncher => ProjectileKind::Sticky,
        };
        let direction = direction(pitch, yaw);
        let (right, up) = aim_basis(pitch, yaw);
        let eye = add(self.movement.position, self.movement.view_offset);
        let position = if kind == ProjectileKind::Sticky {
            add(
                add(add(eye, scale(direction, 16.0)), scale(right, 8.0)),
                scale(up, -6.0),
            )
        } else {
            add(eye, scale(direction, 16.0))
        };
        let speed = if kind == ProjectileKind::Rocket {
            1_100.0
        } else {
            900.0
        };
        let mut velocity = scale(direction, speed);
        if kind == ProjectileKind::Sticky {
            velocity = add(velocity, scale(up, 200.0));
        }
        let identity = self.next_projectile;
        self.next_projectile = self.next_projectile.wrapping_add(1).max(1);
        let angular_velocity = if kind == ProjectileKind::Sticky {
            [600.0, sticky_spin(identity), 0.0]
        } else {
            [0.0; 3]
        };
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
                orientation: quaternion_from_angles(pitch, yaw, 0.0),
                angular_velocity,
                contact_normal: None,
                age_seconds: 0.0,
            },
            armed: false,
        };
        projectile_events.push(projectile_event(
            ProjectileEventKind::Fire,
            &projectile.presentation,
            self.tick,
        ));
        self.projectiles.push(projectile);
        if kind == ProjectileKind::Sticky
            && self
                .projectiles
                .iter()
                .filter(|projectile| projectile.presentation.kind == ProjectileKind::Sticky)
                .count()
                > 8
            && let Some(index) = self
                .projectiles
                .iter()
                .position(|projectile| projectile.presentation.kind == ProjectileKind::Sticky)
        {
            let projectile = self.projectiles.remove(index);
            self.explode(projectile, projectile_events, events);
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
            let previous_age = projectile.presentation.age_seconds;
            projectile.presentation.age_seconds += tick_interval;
            if projectile.presentation.kind == ProjectileKind::Sticky
                && !projectile.armed
                && projectile.presentation.age_seconds >= 0.8
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
            if projectile.presentation.state != ProjectileState::Flying {
                retained.push(projectile);
                continue;
            }
            if projectile.presentation.kind == ProjectileKind::Sticky {
                projectile.presentation.velocity[2] -= 800.0 * tick_interval;
                projectile.presentation.orientation = integrate_orientation(
                    projectile.presentation.orientation,
                    projectile.presentation.angular_velocity,
                    tick_interval,
                );
            }
            let end = add(
                projectile.presentation.position,
                scale(projectile.presentation.velocity, tick_interval),
            );
            let trace = self.collision.trace(
                projectile.presentation.position,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                self.movement_configuration.solid_mask,
            )?;
            projectile.presentation.position = trace.end;
            if trace.fraction < 1.0 {
                let normal = normalized(trace.normal.unwrap_or([0.0, 0.0, 1.0]));
                projectile.presentation.contact_normal = Some(normal);
                projectile_events.push(projectile_event(
                    ProjectileEventKind::Impact,
                    &projectile.presentation,
                    self.tick,
                ));
                if projectile.presentation.kind == ProjectileKind::Rocket {
                    self.explode(projectile, projectile_events, events);
                } else {
                    projectile.presentation.velocity = [0.0; 3];
                    projectile.presentation.angular_velocity = [0.0; 3];
                    projectile.presentation.state = if projectile.armed {
                        ProjectileState::StuckArmed
                    } else {
                        ProjectileState::StuckUnarmed
                    };
                    projectile_events.push(projectile_event(
                        ProjectileEventKind::Stick,
                        &projectile.presentation,
                        self.tick,
                    ));
                    retained.push(projectile);
                }
            } else if projectile.presentation.kind == ProjectileKind::Rocket
                && previous_age < 10.0
                && projectile.presentation.age_seconds >= 10.0
            {
                projectile_events.push(projectile_event(
                    ProjectileEventKind::Fizzle,
                    &projectile.presentation,
                    self.tick,
                ));
            } else {
                retained.push(projectile);
            }
        }
        self.projectiles = retained;
        Ok(())
    }

    fn detonate(&mut self, projectile_events: &mut Vec<ProjectileEvent>, events: &mut Vec<Event>) {
        let mut retained = Vec::new();
        for projectile in std::mem::take(&mut self.projectiles) {
            if projectile.presentation.kind == ProjectileKind::Sticky && projectile.armed {
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
        projectile_events.push(projectile_event(
            ProjectileEventKind::Explode,
            &projectile.presentation,
            self.tick,
        ));
        let center = add(self.movement.position, [0.0, 0.0, 41.0]);
        let delta = sub(center, projectile.presentation.position);
        let distance = length(delta);
        if distance <= 146.0 {
            let base = if projectile.presentation.kind == ProjectileKind::Rocket {
                90.0
            } else {
                120.0
            };
            let amount = base * (1.0 - 0.5 * distance / 146.0) * 0.6;
            self.health = (self.health - amount).max(0.0);
            events.push(Event::Damaged {
                amount,
                health: self.health,
            });
            let direction = if distance > 0.001 {
                scale(delta, 1.0 / distance)
            } else {
                [0.0, 0.0, 1.0]
            };
            self.movement.velocity = add(self.movement.velocity, scale(direction, amount * 10.0));
            self.movement.ground = None;
            events.push(Event::BlastImpulse {
                velocity: self.movement.velocity,
            });
        }
    }

    fn fizzle_projectiles(&mut self, events: &mut Vec<ProjectileEvent>) {
        for projectile in std::mem::take(&mut self.projectiles) {
            events.push(projectile_event(
                ProjectileEventKind::Fizzle,
                &projectile.presentation,
                self.tick,
            ));
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
        self.conditions = 0;
        for weapon in self.loadout.values_mut() {
            weapon.regenerate();
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
        events.push(Event::Respawned);
    }

    fn maximum_health(&self) -> f32 {
        if self.jump.is_some() && self.class == Class::Soldier {
            900.0
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

fn default_loadout(class: Class) -> BTreeMap<Weapon, WeaponState> {
    match class {
        Class::Soldier => BTreeMap::from([
            (
                Weapon::RocketLauncher,
                WeaponState::full(Weapon::RocketLauncher),
            ),
            (Weapon::Original, WeaponState::full(Weapon::Original)),
        ]),
        Class::Demoman => BTreeMap::from([(
            Weapon::StickybombLauncher,
            WeaponState::full(Weapon::StickybombLauncher),
        )]),
    }
}

fn stock_maximum_health(class: Class) -> f32 {
    match class {
        Class::Soldier => 200.0,
        Class::Demoman => 175.0,
    }
}

fn allowed(class: Class, weapon: Weapon) -> bool {
    matches!(
        (class, weapon),
        (Class::Soldier, Weapon::RocketLauncher | Weapon::Original)
            | (Class::Demoman, Weapon::StickybombLauncher)
    )
}

fn resupply_removed_conditions() -> u32 {
    (1 << (Condition::Burning as u32))
        | (1 << (Condition::Urine as u32))
        | (1 << (Condition::MadMilk as u32))
        | (1 << (Condition::Gas as u32))
        | (1 << (Condition::Bleeding as u32))
        | (1 << (Condition::EnergyBuff as u32))
        | (1 << (Condition::CannotSwitchFromMelee as u32))
        | (1 << (Condition::Phase as u32))
        | (1 << (Condition::ParachuteActive as u32))
        | (1 << (Condition::Plague as u32))
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

fn sticky_spin(identity: u32) -> f32 {
    let value = identity
        .wrapping_mul(747_796_405)
        .wrapping_add(2_891_336_453);
    -1_200.0 + (value % 2_401) as f32
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

fn integrate_orientation(orientation: [f32; 4], angular: [f32; 3], dt: f32) -> [f32; 4] {
    let speed = length(angular);
    if speed <= f32::EPSILON {
        return orientation;
    }
    let angle = speed.to_radians() * dt;
    let sine = (angle * 0.5).sin();
    let axis = scale(angular, 1.0 / speed);
    let delta = [
        axis[0] * sine,
        axis[1] * sine,
        axis[2] * sine,
        (angle * 0.5).cos(),
    ];
    normalized_quaternion(quaternion_multiply(delta, orientation))
}

fn quaternion_multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
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
        let mut found = false;
        for _ in 0..20 {
            let snapshot = session.advance(Command::default()).unwrap();
            let kinds: Vec<_> = snapshot
                .projectile_events
                .iter()
                .map(|event| event.kind)
                .collect();
            if kinds.contains(&ProjectileEventKind::Explode) {
                assert_eq!(
                    kinds,
                    [ProjectileEventKind::Impact, ProjectileEventKind::Explode]
                );
                found = true;
                break;
            }
        }
        assert!(found);
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
    fn regenerate_restores_stock_state_once_per_touch_cooldown() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"func_regenerate\"\"model\"\"*1\"\"TeamNum\"\"0\"\"StartDisabled\"\"0\"}\0",
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
        session.health = 50.0;
        session.conditions = u32::MAX;
        touching.store(true, Ordering::Relaxed);
        let supplied = session.advance(Command::default()).unwrap();
        assert_eq!(supplied.health, 200.0);
        assert_eq!(supplied.loadout[0].clip, 4);
        assert_eq!(supplied.loadout[0].reserve, 20);
        assert_eq!(supplied.conditions & resupply_removed_conditions(), 0);
        assert!(matches!(
            supplied.events.as_slice(),
            [Event::Resupplied { entity: 0, .. }]
        ));
        session.health = 100.0;
        let cooling_down = session.advance(Command::default()).unwrap();
        assert_eq!(cooling_down.health, 100.0);
        assert!(cooling_down.events.is_empty());
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
        assert!(
            door.entity_transforms
                .iter()
                .any(|transform| { transform.identity == 1 && transform.position[2] > 0.0 })
        );
        let mut platform_moved = false;
        for _ in 0..15 {
            let snapshot = session.advance(Command::default()).unwrap();
            platform_moved |= snapshot
                .entity_transforms
                .iter()
                .any(|transform| transform.identity == 3 && transform.position[1] > 0.0);
        }
        assert!(platform_moved);
    }
}
