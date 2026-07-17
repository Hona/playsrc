use playsrc_collision::Hull;
use playsrc_movement::{
    Command as MoveCommand, Error as MoveError, Parameters, Player, Tracer, advance,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    Soldier,
    Demoman,
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
    player: Player,
    health: f32,
    spawn: [f32; 3],
    projectiles: Vec<Projectile>,
    next_projectile: u32,
    next_fire_tick: u64,
    parameters: Parameters,
    entities: playsrc_entity::Runtime,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Movement,
    Entity,
    InvalidCommand,
}
impl From<MoveError> for Error {
    fn from(_: MoveError) -> Self {
        Self::Movement
    }
}
impl From<playsrc_entity::RuntimeError> for Error {
    fn from(_: playsrc_entity::RuntimeError) -> Self {
        Self::Entity
    }
}
impl<T: Tracer> Session<T> {
    pub fn new(tracer: T, spawn: [f32; 3], entities: playsrc_entity::Runtime) -> Self {
        Self {
            tracer,
            tick: 0,
            class: Class::Soldier,
            weapon: Weapon::RocketLauncher,
            player: Player {
                position: spawn,
                velocity: [0.; 3],
                grounded: false,
                crouched: false,
                jump_latched: false,
            },
            health: 200.,
            spawn,
            projectiles: Vec::new(),
            next_projectile: 1,
            next_fire_tick: 0,
            parameters: Parameters::default(),
            entities,
        }
    }
    pub fn teleport_count(&self) -> usize {
        self.entities.teleports.len()
    }
    pub fn teleport_destination_count(&self) -> usize {
        self.entities.destinations.len()
    }
    pub fn advance(&mut self, command: Command) -> Result<Snapshot, Error> {
        if !command.pitch_degrees.is_finite() {
            return Err(Error::InvalidCommand);
        }
        let mut events = Vec::new();
        if let Some(class) = command.select_class
            && class != self.class
        {
            self.class = class;
            self.weapon = match class {
                Class::Soldier => Weapon::RocketLauncher,
                Class::Demoman => Weapon::StickybombLauncher,
            };
            self.health = max_health(class);
            events.push(Event::ClassChanged(class));
        }
        if let Some(weapon) = command.select_weapon
            && allowed(self.class, weapon)
            && weapon != self.weapon
        {
            self.weapon = weapon;
            events.push(Event::WeaponChanged(weapon));
        }
        let mut parameters = self.parameters;
        parameters.max_speed = match self.class {
            Class::Soldier => 240.,
            Class::Demoman => 280.,
        };
        self.player = advance(&self.tracer, self.player, command.movement, parameters)?;
        let hull = if self.player.crouched {
            parameters.crouched
        } else {
            parameters.standing
        };
        if let Some(teleport) = self.entities.teleport(self.player.position, hull)? {
            self.player.position = teleport.position;
            self.player.grounded = false;
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
                    self.next_fire_tick = self.tick + ticks(0.8, parameters.tick);
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
                    self.next_fire_tick = self.tick + ticks(0.6, parameters.tick);
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
        self.advance_projectiles(parameters.tick, &mut events)?;
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
            self.player.position = self.spawn;
            self.player.velocity = [0.; 3];
            self.projectiles.clear();
            events.push(Event::Respawned);
        }
        self.tick += 1;
        Ok(Snapshot {
            tick: self.tick,
            class: self.class,
            weapon: self.weapon,
            player: self.player,
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
                self.player.position[0] + offset[0],
                self.player.position[1] + offset[1],
                self.player.position[2] + offset[2],
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
                self.parameters.solid_mask,
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
            self.player.position[0],
            self.player.position[1],
            self.player.position[2] + 41.,
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
            self.player.velocity[0] += direction[0] * impulse;
            self.player.velocity[1] += direction[1] * impulse;
            self.player.velocity[2] += direction[2] * impulse;
            self.player.grounded = false;
            events.push(Event::BlastImpulse {
                velocity: self.player.velocity,
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
    use playsrc_movement::Trace;
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
                })
            } else {
                Ok(Trace {
                    fraction: 1.,
                    start_solid: false,
                    all_solid: false,
                    end,
                    normal: None,
                })
            }
        }
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
        assert!(s.player.velocity[2] > 0.);
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
}
