use playsrc_collision::Hull;
use playsrc_movement::{
    Command as MoveCommand, Configuration as MovementConfiguration, Error as MoveError,
    ModeRequest, Player, Policy as GenericMovementPolicy, State as MovementState, StepInput,
    StepResult as MovementStepResult, StepStrategy, Tracer, step,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    Soldier,
    Demoman,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weapon {
    RocketLauncher,
    Original,
    StickybombLauncher,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub movement: MoveCommand,
    pub pitch_degrees: f32,
    pub fire: bool,
    pub detonate: bool,
    pub select_class: Option<Class>,
    pub select_weapon: Option<Weapon>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectileKind {
    Rocket,
    Sticky,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub id: u32,
    pub kind: ProjectileKind,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
    pub armed: bool,
    pub stuck: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    ClassChanged(Class),
    WeaponChanged(Weapon),
    Fired {
        projectile: u32,
        kind: ProjectileKind,
    },
    Explosion {
        projectile: u32,
        kind: ProjectileKind,
        position: [f32; 3],
    },
    Damaged {
        amount: f32,
        health: f32,
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
    Respawned,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub class: Class,
    pub weapon: Weapon,
    pub player: Player,
    pub health: f32,
    pub projectiles: Vec<Projectile>,
    pub events: Vec<Event>,
}
#[derive(Clone)]
pub struct Session<T: Tracer> {
    tracer: T,
    tick: u64,
    class: Class,
    weapon: Weapon,
    movement: MovementState,
    movement_modifiers: MovementModifiers,
    pending_mode_request: Option<ModeRequest>,
    last_movement: Option<MovementStepResult>,
    health: f32,
    spawn: [f32; 3],
    projectiles: Vec<Projectile>,
    next_projectile: u32,
    next_fire_tick: u64,
    movement_configuration: MovementConfiguration,
    entities: playsrc_entity::Runtime,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Movement(MoveError),
    Entity(playsrc_entity::RuntimeError),
}
impl From<MoveError> for Error {
    fn from(error: MoveError) -> Self {
        Self::Movement(error)
    }
}
impl From<playsrc_entity::RuntimeError> for Error {
    fn from(error: playsrc_entity::RuntimeError) -> Self {
        Self::Entity(error)
    }
}
impl<T: Tracer> Session<T> {
    pub fn new(tracer: T, spawn: [f32; 3], entities: playsrc_entity::Runtime) -> Self {
        let movement_modifiers = MovementModifiers::default();
        let movement_policy = MovementPolicy {
            class: Class::Soldier,
            modifiers: movement_modifiers,
        }
        .resolve();
        Self {
            tracer,
            tick: 0,
            class: Class::Soldier,
            weapon: Weapon::RocketLauncher,
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
            pending_mode_request: None,
            last_movement: None,
            health: 200.,
            spawn,
            projectiles: Vec::new(),
            next_projectile: 1,
            next_fire_tick: 0,
            movement_configuration: MovementConfiguration::default(),
            entities,
        }
    }

    pub fn set_movement_modifiers(&mut self, modifiers: MovementModifiers) {
        self.movement_modifiers = modifiers;
    }

    pub fn request_movement_mode(&mut self, request: ModeRequest) {
        self.pending_mode_request = Some(request);
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
    pub fn teleport_count(&self) -> usize {
        self.entities.teleports.len()
    }
    pub fn teleport_destination_count(&self) -> usize {
        self.entities.destinations.len()
    }
    pub fn advance(&mut self, command: Command) -> Result<Snapshot, Error> {
        let mut events = Vec::new();
        let next_class = command.select_class.unwrap_or(self.class);
        let class_changed = next_class != self.class;
        let mut next_weapon = if class_changed {
            match next_class {
                Class::Soldier => Weapon::RocketLauncher,
                Class::Demoman => Weapon::StickybombLauncher,
            }
        } else {
            self.weapon
        };
        let mut selected_weapon_changed = false;
        if let Some(weapon) = command.select_weapon
            && allowed(next_class, weapon)
            && weapon != next_weapon
        {
            next_weapon = weapon;
            selected_weapon_changed = true;
        }
        let movement_policy = MovementPolicy {
            class: next_class,
            modifiers: self.movement_modifiers,
        }
        .resolve();
        let movement_result = step(
            &self.tracer,
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
                up: 0.0,
                speed_button: false,
                mode_request: self.pending_mode_request,
            },
            self.movement_configuration,
            movement_policy,
        )?;
        if class_changed {
            self.class = next_class;
            self.health = max_health(next_class);
            events.push(Event::ClassChanged(next_class));
        }
        self.weapon = next_weapon;
        if selected_weapon_changed {
            events.push(Event::WeaponChanged(next_weapon));
        }
        self.pending_mode_request = None;
        self.movement = movement_result.state;
        self.last_movement = Some(movement_result);
        let hull = self.movement.active_hull(movement_policy);
        if let Some(teleport) = self.entities.teleport(self.movement.position, hull)? {
            self.movement.position = teleport.position;
            self.movement.ground = None;
            events.push(Event::Teleported {
                trigger: teleport.trigger_entity as u32,
                destination: teleport.destination_entity as u32,
                position: teleport.position,
                yaw_degrees: teleport.yaw_degrees,
            });
        }
        if command.fire && self.tick >= self.next_fire_tick {
            match self.weapon {
                Weapon::RocketLauncher | Weapon::Original => {
                    let id = self.spawn_projectile(
                        ProjectileKind::Rocket,
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                    );
                    events.push(Event::Fired {
                        projectile: id,
                        kind: ProjectileKind::Rocket,
                    });
                    self.next_fire_tick =
                        self.tick + ticks(0.8, self.movement_configuration.tick_interval);
                }
                Weapon::StickybombLauncher => {
                    let id = self.spawn_projectile(
                        ProjectileKind::Sticky,
                        command.pitch_degrees,
                        command.movement.yaw_degrees,
                    );
                    events.push(Event::Fired {
                        projectile: id,
                        kind: ProjectileKind::Sticky,
                    });
                    self.next_fire_tick =
                        self.tick + ticks(0.6, self.movement_configuration.tick_interval);
                    if self
                        .projectiles
                        .iter()
                        .filter(|p| p.kind == ProjectileKind::Sticky)
                        .count()
                        > 8
                        && let Some(index) = self
                            .projectiles
                            .iter()
                            .position(|p| p.kind == ProjectileKind::Sticky)
                    {
                        let p = self.projectiles.remove(index);
                        self.explode(p, &mut events);
                    }
                }
            }
        }
        self.advance_projectiles(self.movement_configuration.tick_interval, &mut events)?;
        if command.detonate {
            let mut retained = Vec::new();
            for projectile in std::mem::take(&mut self.projectiles) {
                if projectile.kind == ProjectileKind::Sticky && projectile.armed {
                    self.explode(projectile, &mut events)
                } else {
                    retained.push(projectile)
                }
            }
            self.projectiles = retained;
        }
        if self.health <= 0. {
            self.health = max_health(self.class);
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
            self.pending_mode_request = None;
            self.last_movement = None;
            self.projectiles.clear();
            events.push(Event::Respawned);
        }
        self.tick += 1;
        Ok(Snapshot {
            tick: self.tick,
            class: self.class,
            weapon: self.weapon,
            player: self.movement.player(),
            health: self.health,
            projectiles: self.projectiles.clone(),
            events,
        })
    }
    fn spawn_projectile(&mut self, kind: ProjectileKind, pitch: f32, yaw: f32) -> u32 {
        let direction = direction(pitch, yaw);
        let id = self.next_projectile;
        self.next_projectile = self.next_projectile.wrapping_add(1).max(1);
        let offset = match self.weapon {
            Weapon::Original => [
                direction[0] * 16.,
                direction[1] * 16.,
                68. + direction[2] * 16.,
            ],
            _ => [
                direction[0] * 16.,
                direction[1] * 16.,
                68. + direction[2] * 16.,
            ],
        };
        let speed = if kind == ProjectileKind::Rocket {
            1100.
        } else {
            900.
        };
        let mut velocity = [
            direction[0] * speed,
            direction[1] * speed,
            direction[2] * speed,
        ];
        if kind == ProjectileKind::Sticky {
            velocity[2] += 200.;
        }
        self.projectiles.push(Projectile {
            id,
            kind,
            position: [
                self.movement.position[0] + offset[0],
                self.movement.position[1] + offset[1],
                self.movement.position[2] + offset[2],
            ],
            velocity,
            age: 0.,
            armed: false,
            stuck: false,
        });
        id
    }
    fn advance_projectiles(&mut self, dt: f32, events: &mut Vec<Event>) -> Result<(), Error> {
        let mut retained = Vec::new();
        for mut p in std::mem::take(&mut self.projectiles) {
            p.age += dt;
            if p.kind == ProjectileKind::Sticky {
                p.armed = p.age >= 0.7;
                if !p.stuck {
                    p.velocity[2] -= 800. * dt;
                }
            }
            if p.stuck {
                retained.push(p);
                continue;
            }
            let end = [
                p.position[0] + p.velocity[0] * dt,
                p.position[1] + p.velocity[1] * dt,
                p.position[2] + p.velocity[2] * dt,
            ];
            let trace = self.tracer.trace(
                p.position,
                end,
                Hull {
                    mins: [0.; 3],
                    maxs: [0.; 3],
                },
                self.movement_configuration.solid_mask,
            )?;
            p.position = trace.end;
            if trace.fraction < 1. {
                if p.kind == ProjectileKind::Rocket {
                    self.explode(p, events)
                } else {
                    p.stuck = true;
                    p.velocity = [0.; 3];
                    retained.push(p)
                }
            } else if p.age < 10. {
                retained.push(p)
            }
        }
        self.projectiles = retained;
        Ok(())
    }
    fn explode(&mut self, p: Projectile, events: &mut Vec<Event>) {
        events.push(Event::Explosion {
            projectile: p.id,
            kind: p.kind,
            position: p.position,
        });
        let center = [
            self.movement.position[0],
            self.movement.position[1],
            self.movement.position[2] + 41.,
        ];
        let delta = [
            center[0] - p.position[0],
            center[1] - p.position[1],
            center[2] - p.position[2],
        ];
        let distance = length(delta);
        if distance <= 146. {
            let base = if p.kind == ProjectileKind::Rocket {
                90.
            } else {
                120.
            };
            let amount = base * (1. - 0.5 * (distance / 146.)) * 0.6;
            self.health = (self.health - amount).max(0.);
            events.push(Event::Damaged {
                amount,
                health: self.health,
            });
            let direction = if distance > 0.001 {
                [
                    delta[0] / distance,
                    delta[1] / distance,
                    delta[2] / distance,
                ]
            } else {
                [0., 0., 1.]
            };
            let impulse = amount * 10.;
            self.movement.velocity[0] += direction[0] * impulse;
            self.movement.velocity[1] += direction[1] * impulse;
            self.movement.velocity[2] += direction[2] * impulse;
            self.movement.ground = None;
            events.push(Event::BlastImpulse {
                velocity: self.movement.velocity,
            });
        }
    }
}
fn max_health(class: Class) -> f32 {
    match class {
        Class::Soldier => 200.,
        Class::Demoman => 175.,
    }
}
fn allowed(class: Class, weapon: Weapon) -> bool {
    matches!(
        (class, weapon),
        (Class::Soldier, Weapon::RocketLauncher | Weapon::Original)
            | (Class::Demoman, Weapon::StickybombLauncher)
    )
}
fn ticks(seconds: f32, tick: f32) -> u64 {
    (seconds / tick).ceil() as u64
}
fn direction(pitch: f32, yaw: f32) -> [f32; 3] {
    let (pitch, yaw) = (pitch.to_radians(), yaw.to_radians());
    let cp = pitch.cos();
    [cp * yaw.cos(), cp * yaw.sin(), -pitch.sin()]
}
fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_movement::{
        Event as MovementEvent, Mode, StateDisposition, Trace, TransitionDisposition,
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
            if end[2] < 0. {
                let f = (start[2] / (start[2] - end[2])).clamp(0., 1.);
                Ok(Trace {
                    fraction: f,
                    start_solid: false,
                    all_solid: false,
                    end: [
                        start[0] + (end[0] - start[0]) * f,
                        start[1] + (end[1] - start[1]) * f,
                        0.,
                    ],
                    normal: Some([0., 0., 1.]),
                    hit: Some(0),
                    contents: 1,
                })
            } else {
                Ok(Trace {
                    fraction: 1.,
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

    #[derive(Clone)]
    struct FloorAt(f32);
    impl Tracer for FloorAt {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            _: Hull,
            _: u32,
        ) -> Result<Trace, MoveError> {
            if end[2] < self.0 {
                let fraction = ((start[2] - self.0) / (start[2] - end[2])).clamp(0.0, 1.0);
                Ok(Trace {
                    fraction,
                    start_solid: start[2] < self.0,
                    all_solid: start[2] < self.0 && end[2] < self.0,
                    end: [
                        start[0] + (end[0] - start[0]) * fraction,
                        start[1] + (end[1] - start[1]) * fraction,
                        self.0,
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

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.001,
            "expected {expected}, got {actual}"
        );
    }
    #[test]
    fn soldier_rocket_explodes_and_impulses_player() {
        let mut s = Session::new(Floor, [0., 0., 0.], playsrc_entity::Runtime::empty());
        let mut command = Command {
            pitch_degrees: 89.,
            fire: true,
            ..Command::default()
        };
        let fired = s.advance(command).unwrap();
        assert!(fired.events.iter().any(|e| matches!(
            e,
            Event::Fired {
                kind: ProjectileKind::Rocket,
                ..
            }
        )));
        command.fire = false;
        let mut impulse = false;
        for _ in 0..20 {
            let snap = s.advance(command).unwrap();
            impulse |= snap
                .events
                .iter()
                .any(|e| matches!(e, Event::BlastImpulse { .. }));
            if impulse {
                break;
            }
        }
        assert!(impulse);
        assert!(s.movement.velocity[2] > 0.);
    }
    #[test]
    fn demoman_retains_arms_and_detonates_stickies() {
        let mut s = Session::new(Floor, [0., 0., 0.], playsrc_entity::Runtime::empty());
        let mut c = Command {
            select_class: Some(Class::Demoman),
            fire: true,
            ..Command::default()
        };
        s.advance(c).unwrap();
        c.select_class = None;
        c.fire = false;
        for _ in 0..60 {
            s.advance(c).unwrap();
        }
        let snap = s
            .advance(Command {
                detonate: true,
                ..Command::default()
            })
            .unwrap();
        assert!(
            snap.events
                .iter()
                .any(|e| matches!(e, Event::Explosion { .. }))
        );
        assert!(snap.projectiles.is_empty());
    }

    #[test]
    fn movement_policy_owns_tf2_values_modifiers_and_noclip_permission() {
        let policy = MovementPolicy {
            class: Class::Demoman,
            modifiers: MovementModifiers {
                condition_speed_factor: 0.5,
                item_speed_factor: 2.0,
                condition_jump_factor: 2.0,
                item_jump_factor: 0.5,
                air_control_factor: 2.0,
                noclip_allowed: true,
                ..MovementModifiers::default()
            },
        }
        .resolve();
        assert_eq!(policy.maximum_speed, 280.0);
        assert_eq!(policy.air_speed_cap, 60.0);
        assert_eq!(policy.bunnyhop_speed_cap, Some(336.0));
        assert_eq!(policy.jump_impulse, 289.0);
        assert_eq!(policy.backward_speed_factor, 0.9);
        assert_eq!(policy.ground_detach_speed, 250.0);
        assert_eq!(policy.standing_hull.maxs, [24.0, 24.0, 82.0]);
        assert_eq!(policy.crouched_hull.maxs, [24.0, 24.0, 62.0]);
        assert_eq!(policy.standing_view, [0.0, 0.0, 68.0]);
        assert_eq!(policy.crouched_view, [0.0, 0.0, 45.0]);
        assert!(policy.allow_noclip);

        let mut session = Session::new(Floor, [0.0; 3], playsrc_entity::Runtime::empty());
        session.request_movement_mode(ModeRequest {
            mode: Mode::Noclip,
            disposition: TransitionDisposition {
                velocity: StateDisposition::Reset,
                ground: StateDisposition::Reset,
                water: StateDisposition::Reset,
            },
        });
        session.advance(Command::default()).unwrap();
        assert_eq!(session.movement_state().mode, Mode::Walk);
        assert!(session.last_movement_result().unwrap().events.contains(
            &MovementEvent::ModeDenied {
                requested: Mode::Noclip
            }
        ));

        session.set_movement_modifiers(MovementModifiers {
            noclip_allowed: true,
            ..MovementModifiers::default()
        });
        session.request_movement_mode(ModeRequest {
            mode: Mode::Noclip,
            disposition: TransitionDisposition::RESET_ENVIRONMENT,
        });
        session.advance(Command::default()).unwrap();
        assert_eq!(session.movement_state().mode, Mode::Noclip);
        assert!(session.last_movement_result().unwrap().queries.is_empty());

        let before_failure = session.movement_snapshot_bytes();
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
                command_number: 2,
                field: "pitch",
                ..
            })
        ));
        assert_eq!(session.class, Class::Soldier);
        assert_eq!(session.movement_snapshot_bytes(), before_failure);
    }

    #[test]
    fn jump_beef_spawn_browser_schedule_matches_target_not_replaced_approximation() {
        const SPAWN: [f32; 3] = [5328.0, 3376.0, -3120.0];
        const REPLACED_JUMP_HORIZONTAL_SPEED: f32 = 66.0;
        const REPLACED_JUMP_VERTICAL_SPEED: f32 = 283.0;
        const REPLACED_JUMP_HORIZONTAL_DISTANCE: f32 = 0.99;
        const REPLACED_JUMP_VERTICAL_DISTANCE: f32 = 4.335;
        const TARGET_JUMP_HORIZONTAL_SPEED: f32 = 36.0;
        const TARGET_JUMP_VERTICAL_SPEED: f32 = 271.0;
        const TARGET_JUMP_HORIZONTAL_DISTANCE: f32 = 0.54;
        const TARGET_JUMP_VERTICAL_DISTANCE: f32 = 4.155;

        let mut session = Session::new(FloorAt(SPAWN[2]), SPAWN, playsrc_entity::Runtime::empty());
        let mut command = Command {
            movement: MoveCommand {
                forward: 450.0,
                yaw_degrees: 180.0,
                ..MoveCommand::default()
            },
            ..Command::default()
        };
        let accelerated = session.advance(command).unwrap();
        close(accelerated.player.velocity[0], -36.0);
        close(accelerated.player.position[0], SPAWN[0] - 0.54);

        let before_jump = accelerated.player.position;
        command.movement.jump = true;
        let jumped = session.advance(command).unwrap();
        close(
            jumped.player.velocity[0].hypot(jumped.player.velocity[1]),
            TARGET_JUMP_HORIZONTAL_SPEED,
        );
        close(jumped.player.velocity[2], TARGET_JUMP_VERTICAL_SPEED);
        close(
            (jumped.player.position[0] - before_jump[0])
                .hypot(jumped.player.position[1] - before_jump[1]),
            TARGET_JUMP_HORIZONTAL_DISTANCE,
        );
        close(
            jumped.player.position[2] - before_jump[2],
            TARGET_JUMP_VERTICAL_DISTANCE,
        );
        assert_ne!(
            TARGET_JUMP_HORIZONTAL_SPEED.to_bits(),
            REPLACED_JUMP_HORIZONTAL_SPEED.to_bits()
        );
        assert_ne!(
            TARGET_JUMP_VERTICAL_SPEED.to_bits(),
            REPLACED_JUMP_VERTICAL_SPEED.to_bits()
        );
        assert_ne!(
            TARGET_JUMP_HORIZONTAL_DISTANCE.to_bits(),
            REPLACED_JUMP_HORIZONTAL_DISTANCE.to_bits()
        );
        assert_ne!(
            TARGET_JUMP_VERTICAL_DISTANCE.to_bits(),
            REPLACED_JUMP_VERTICAL_DISTANCE.to_bits()
        );

        command.movement.jump = false;
        let released = session.advance(command).unwrap();
        close(released.player.velocity[2], 259.0);
        close(
            released.player.position[2] - jumped.player.position[2],
            3.975,
        );
        let repeated = session.advance(command).unwrap();
        close(repeated.player.velocity[2], 247.0);
        close(
            repeated.player.position[2] - released.player.position[2],
            3.795,
        );

        let snapshot = session.movement_snapshot_bytes();
        assert_eq!(&snapshot[..8], b"PMOV\x01\0\0\0");
    }
}
