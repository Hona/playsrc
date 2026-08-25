use crate::{
    class::PlayerClass,
    random::{RandomError, UniformRandomStream},
};

pub const MAX_FLAME_POINTS: usize = 30;
pub const FLAME_FIRE_DELAY: f32 = 0.02;
pub const FLAME_DAMAGE: f32 = 13.0;
pub const FLAME_BURN_FREQUENCY: f32 = 0.075;
pub const FLAME_AMMO_PER_SECOND: f32 = 14.0;
pub const FLAME_INITIAL_AFTERBURN: f32 = 3.0;
pub const FLAME_AFTERBURN_PER_HIT: f32 = 0.4;
pub const AFTERBURN_MAXIMUM: f32 = 10.0;
pub const AFTERBURN_FREQUENCY: f32 = 0.5;
pub const AFTERBURN_DAMAGE: f32 = 4.0;
pub const PYRO_VISIBLE_BURN_DURATION: f32 = 0.25;
pub const AIRBLAST_AMMO: u16 = 20;
pub const AIRBLAST_SECONDARY_DELAY: f32 = 0.75;
pub const AIRBLAST_PRIMARY_DELAY: f32 = 1.0;
pub const AIRBLAST_RADIUS: f32 = 128.0;
pub const AIRBLAST_CONE_DEGREES: f32 = 35.0;
pub const SHOTGUN_PELLETS: usize = 10;
pub const SHOTGUN_DAMAGE: f32 = 6.0;
pub const SHOTGUN_RANGE: f32 = 8_192.0;
pub const SHOTGUN_SPREAD: f32 = 0.0675;
pub const FIRE_AXE_DAMAGE: f32 = 65.0;
pub const FIRE_AXE_SWING_RANGE: f32 = 48.0;
pub const FIRE_AXE_SMACK_DELAY: f32 = 0.2;
pub const FIRE_AXE_HULL_RADIUS: f32 = 18.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlameConfiguration {
    pub speed: f32,
    pub lifetime: f32,
    pub lifetime_variance: f32,
    pub spread_degrees: f32,
    pub drag: f32,
    pub upward_velocity: f32,
    pub gravity: f32,
    pub radius: f32,
}

impl FlameConfiguration {
    pub const STOCK: Self = Self {
        speed: 2_450.0,
        lifetime: 0.6,
        lifetime_variance: 0.1,
        spread_degrees: 2.8,
        drag: 8.5,
        upward_velocity: 50.0,
        gravity: 0.0,
        radius: 12.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlamePoint {
    pub slot: u8,
    pub spawn_tick: u64,
    pub spawn_time: f32,
    pub lifetime: f32,
    pub initial_position: [f32; 3],
    pub previous_position: [f32; 3],
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub attacker_velocity: [f32; 3],
    pub walls_hit: u8,
}

impl FlamePoint {
    pub fn damage_scale(self, now: f32, heat_index: f32) -> f32 {
        let elapsed = (now - self.spawn_time).max(0.0);
        let lifetime_fraction = if self.lifetime > 0.0 {
            (elapsed / (self.lifetime * 0.5)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let lifetime_scale = 1.0 - lifetime_fraction * 0.5;
        let heat_fraction = ((heat_index - 10.0) / 40.0).clamp(0.0, 1.0);
        lifetime_scale * (0.5 + heat_fraction * 0.5)
    }

    pub fn expired(self, now: f32) -> bool {
        now > self.spawn_time + self.lifetime
    }

    pub fn advance(
        &mut self,
        interval: f32,
        configuration: FlameConfiguration,
        collision: Option<FlameWorldContact>,
    ) -> bool {
        if interval <= 0.0 {
            return true;
        }
        if collision.is_some_and(|contact| contact.start_solid) {
            return false;
        }

        let gravity = [0.0, 0.0, configuration.gravity * interval];
        let direction = normalize(self.velocity);
        let inherited = scale(direction, dot(self.attacker_velocity, direction));
        let additional = add(inherited, [0.0, 0.0, configuration.upward_velocity]);
        let mut next_position = add(
            self.position,
            scale(add(add(self.velocity, gravity), additional), interval),
        );
        let next_velocity = if let Some(contact) = collision {
            if contact.fraction < 1.0 {
                self.walls_hit = self.walls_hit.saturating_add(1);
                next_position = add(contact.end, scale(contact.normal, configuration.radius));
                [0.0; 3]
            } else {
                self.velocity
            }
        } else {
            self.velocity
        };
        let velocity_length = length(next_velocity);
        let retained_speed = (velocity_length - interval * configuration.drag * velocity_length)
            .clamp(0.0, velocity_length);
        self.velocity = add(scale(normalize(next_velocity), retained_speed), gravity);
        self.previous_position = self.position;
        self.position = next_position;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlameWorldContact {
    pub fraction: f32,
    pub start_solid: bool,
    pub end: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlameSpawn {
    pub tick: u64,
    pub now: f32,
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub attacker_velocity: [f32; 3],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlameManager {
    points: Vec<FlamePoint>,
    next_slot: u8,
    manager_seed: Option<i32>,
    pub ammo_remainder: f32,
}

impl FlameManager {
    pub fn points(&self) -> &[FlamePoint] {
        &self.points
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.next_slot = 0;
        self.manager_seed = None;
        self.ammo_remainder = 0.0;
    }

    pub fn add_authored_point(
        &mut self,
        spawn: FlameSpawn,
        authority: &mut UniformRandomStream,
    ) -> Result<bool, RandomError> {
        if self.points.len() >= MAX_FLAME_POINTS {
            return Ok(false);
        }
        let seed = match self.manager_seed {
            Some(seed) => seed,
            None => {
                let seed = authority.random_int(0, 9_999)?;
                self.manager_seed = Some(seed);
                seed
            }
        };
        let point_seed = (spawn.tick as i32)
            .wrapping_add(seed)
            .wrapping_add(i32::from(self.next_slot));
        let mut random = UniformRandomStream::from_seed(point_seed)?;
        let (x, y) = loop {
            let x = (random.random_float(-1.0, 1.0) + random.random_float(-1.0, 1.0)) * 0.5;
            let y = (random.random_float(-1.0, 1.0) + random.random_float(-1.0, 1.0)) * 0.5;
            if x * x + y * y <= 1.0 {
                break (x, y);
            }
        };
        let spread = FlameConfiguration::STOCK.spread_degrees.to_radians();
        let direction = add(
            add(spawn.forward, scale(spawn.right, x * spread)),
            scale(spawn.up, y * spread),
        );
        let offset = random.random_float(
            -FlameConfiguration::STOCK.lifetime_variance,
            FlameConfiguration::STOCK.lifetime_variance,
        );
        Ok(self.add_point(
            spawn.tick,
            spawn.now,
            spawn.position,
            direction,
            spawn.attacker_velocity,
            offset,
        ))
    }

    pub fn add_point(
        &mut self,
        tick: u64,
        now: f32,
        position: [f32; 3],
        direction: [f32; 3],
        attacker_velocity: [f32; 3],
        lifetime_offset: f32,
    ) -> bool {
        if self.points.len() >= MAX_FLAME_POINTS {
            return false;
        }
        let configuration = FlameConfiguration::STOCK;
        let point = FlamePoint {
            slot: self.next_slot,
            spawn_tick: tick,
            spawn_time: now,
            lifetime: configuration.lifetime + lifetime_offset,
            initial_position: position,
            previous_position: position,
            position,
            velocity: scale(normalize(direction), configuration.speed),
            attacker_velocity,
            walls_hit: 0,
        };
        self.points.push(point);
        self.next_slot = (self.next_slot + 1) % MAX_FLAME_POINTS as u8;
        true
    }

    pub fn consume_primary_ammo(&mut self, ammunition: &mut u16) -> u16 {
        self.ammo_remainder += FLAME_AMMO_PER_SECOND * FLAME_FIRE_DELAY;
        let consumed = (self.ammo_remainder as u16).min(*ammunition);
        if consumed > 0 {
            *ammunition -= consumed;
            self.ammo_remainder -= f32::from(consumed);
            self.ammo_remainder = ((self.ammo_remainder * 100.0) as i32) as f32 / 100.0;
        }
        consumed
    }

    pub fn advance<E>(
        &mut self,
        now: f32,
        interval: f32,
        mut contact: impl FnMut(&FlamePoint, [f32; 3]) -> Result<Option<FlameWorldContact>, E>,
        mut submerged: impl FnMut([f32; 3]) -> Result<bool, E>,
    ) -> Result<(), E> {
        let configuration = FlameConfiguration::STOCK;
        for index in (0..self.points.len()).rev() {
            let point = self.points[index];
            if point.expired(now) || submerged(point.position)? {
                self.points.remove(index);
                continue;
            }
            let direction = normalize(point.velocity);
            let inherited = scale(direction, dot(point.attacker_velocity, direction));
            let destination = add(
                point.position,
                scale(
                    add(
                        add(point.velocity, [0.0, 0.0, configuration.gravity * interval]),
                        add(inherited, [0.0, 0.0, configuration.upward_velocity]),
                    ),
                    interval,
                ),
            );
            let hit = contact(&point, destination)?;
            if !self.points[index].advance(interval, configuration, hit) {
                self.points.remove(index);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Afterburn {
    pub attacker: u32,
    pub weapon: u32,
    pub duration: f32,
    pub next_damage_time: f32,
    pub immune: bool,
}

impl Afterburn {
    pub fn ignite(
        previous: Option<Self>,
        victim_class: PlayerClass,
        attacker: u32,
        weapon: u32,
        now: f32,
    ) -> Self {
        let immune = victim_class == PlayerClass::Pyro;
        match previous {
            Some(mut state) => {
                state.attacker = attacker;
                state.weapon = weapon;
                state.immune = immune;
                state.duration = if immune {
                    PYRO_VISIBLE_BURN_DURATION
                } else {
                    (state.duration + FLAME_AFTERBURN_PER_HIT).min(AFTERBURN_MAXIMUM)
                };
                state
            }
            None => Self {
                attacker,
                weapon,
                duration: if immune {
                    PYRO_VISIBLE_BURN_DURATION
                } else {
                    FLAME_INITIAL_AFTERBURN + FLAME_AFTERBURN_PER_HIT
                },
                next_damage_time: now + AFTERBURN_FREQUENCY,
                immune,
            },
        }
    }

    pub fn advance(&mut self, now: f32) -> Option<f32> {
        if now < self.next_damage_time || self.duration <= 0.0 {
            return None;
        }
        self.next_damage_time = now + AFTERBURN_FREQUENCY;
        self.duration = (self.duration - AFTERBURN_FREQUENCY).max(0.0);
        (!self.immune).then_some(AFTERBURN_DAMAGE)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotgunPellet {
    pub index: u8,
    pub direction: [f32; 3],
    pub damage: f32,
    pub range: f32,
}

pub fn shotgun_pellets(
    direction: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    seed: i32,
    seconds_since_last_shot: f32,
    fixed_spread: bool,
) -> Result<[ShotgunPellet; SHOTGUN_PELLETS], RandomError> {
    const FIXED: [[f32; 2]; SHOTGUN_PELLETS] = [
        [0.0, 0.0],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.0, -1.0],
        [0.0, 1.0],
        [0.85, -0.85],
        [0.85, 0.85],
        [-0.85, -0.85],
        [-0.85, 0.85],
        [0.0, 0.0],
    ];
    let mut output = [ShotgunPellet {
        index: 0,
        direction: [0.0; 3],
        damage: SHOTGUN_DAMAGE,
        range: SHOTGUN_RANGE,
    }; SHOTGUN_PELLETS];
    for (index, pellet) in output.iter_mut().enumerate() {
        let (x, y) = if fixed_spread {
            (FIXED[index][0] * 0.5, FIXED[index][1] * 0.5)
        } else if index == 0 && seconds_since_last_shot > 0.25 {
            (0.0, 0.0)
        } else {
            let mut random = UniformRandomStream::from_seed(seed.wrapping_add(index as i32))?;
            (
                random.random_float(-0.5, 0.5) + random.random_float(-0.5, 0.5),
                random.random_float(-0.5, 0.5) + random.random_float(-0.5, 0.5),
            )
        };
        *pellet = ShotgunPellet {
            index: index as u8,
            direction: normalize(add(
                add(direction, scale(right, x * SHOTGUN_SPREAD)),
                scale(up, y * SHOTGUN_SPREAD),
            )),
            damage: SHOTGUN_DAMAGE,
            range: SHOTGUN_RANGE,
        };
    }
    Ok(output)
}

pub fn airblast_impulse(
    aim: [f32; 3],
    target_velocity: [f32; 3],
    ground_normal: Option<[f32; 3]>,
) -> [f32; 3] {
    let aim = normalize(aim);
    let momentum = dot(target_velocity, aim);
    let mut result = if momentum < 0.0 {
        scale(aim, -2.0 * momentum)
    } else {
        [0.0; 3]
    };
    let remaining = 600.0 - length(result) * 0.5;
    if remaining > 0.0 {
        result = add(result, scale(aim, remaining));
    }
    if let Some(normal) = ground_normal {
        let into_ground = dot(add(target_velocity, result), normal);
        if into_ground < 0.0 {
            result = add(result, scale(normal, -2.0 * into_ground));
        }
        let additional_z = 100.0 - (target_velocity[2] + result[2]);
        if additional_z > 0.0 {
            let horizontal_squared = result[0] * result[0] + result[1] * result[1];
            let horizontal = horizontal_squared.sqrt();
            let additional_z = additional_z.min(horizontal);
            let magnitude_squared = dot(result, result);
            result = if magnitude_squared < additional_z * additional_z {
                [0.0, 0.0, magnitude_squared.sqrt()]
            } else if horizontal > 0.0 {
                let scale_factor = (-(additional_z * additional_z)
                    - 2.0 * additional_z * result[2]
                    + horizontal_squared)
                    .max(0.0)
                    .sqrt()
                    / horizontal;
                [
                    result[0] * scale_factor,
                    result[1] * scale_factor,
                    result[2] + additional_z,
                ]
            } else {
                result
            };
        }
    }
    result
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn length(value: [f32; 3]) -> f32 {
    dot(value, value).sqrt()
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let magnitude = length(value);
    if magnitude > 0.0 {
        scale(value, 1.0 / magnitude)
    } else {
        value
    }
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale(value: [f32; 3], multiplier: f32) -> [f32; 3] {
    [
        value[0] * multiplier,
        value[1] * multiplier,
        value[2] * multiplier,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_flame_attributes_match_configured_item_prefabs() {
        assert_eq!(FlameConfiguration::STOCK.speed, 2_450.0);
        assert_eq!(FlameConfiguration::STOCK.lifetime, 0.6);
        assert_eq!(FlameConfiguration::STOCK.lifetime_variance, 0.1);
        assert_eq!(FlameConfiguration::STOCK.drag, 8.5);
        assert_eq!(FlameConfiguration::STOCK.spread_degrees, 2.8);
    }

    #[test]
    fn authored_points_reseed_spread_and_lifetime_without_advancing_global_random_per_point() {
        let mut manager = FlameManager::default();
        let mut authority = UniformRandomStream::from_seed(0).unwrap();
        let input = FlameSpawn {
            tick: 40,
            now: 0.6,
            position: [40.0, 5.0, 68.0],
            forward: [1.0, 0.0, 0.0],
            right: [0.0, -1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            attacker_velocity: [0.0; 3],
        };
        assert!(manager.add_authored_point(input, &mut authority).unwrap());
        let state = authority.state();
        let first = manager.points()[0];
        assert!((0.5..=0.7).contains(&first.lifetime));
        assert!(first.velocity[1] != 0.0 || first.velocity[2] != 0.0);
        assert!(
            manager
                .add_authored_point(
                    FlameSpawn {
                        tick: 42,
                        now: 0.63,
                        ..input
                    },
                    &mut authority
                )
                .unwrap()
        );
        assert_eq!(authority.state(), state);
        assert_ne!(manager.points()[1].velocity, first.velocity);
    }

    #[test]
    fn primary_ammunition_preserves_fractional_remainder_and_truncation() {
        let mut manager = FlameManager::default();
        let mut ammunition = 200;
        let consumed = (0..25)
            .map(|_| manager.consume_primary_ammo(&mut ammunition))
            .sum::<u16>();
        assert_eq!(consumed, 6);
        assert_eq!(ammunition, 194);
        assert!(manager.ammo_remainder >= 0.9);
    }

    #[test]
    fn flame_points_apply_authored_drag_float_wall_contact_and_expiry() {
        let mut manager = FlameManager::default();
        assert!(manager.add_point(
            4,
            0.06,
            [40.0, 5.0, 68.0],
            [1.0, 0.0, 0.0],
            [100.0, 20.0, 0.0],
            0.0
        ));
        manager
            .advance::<()>(0.075, 0.015, |_, _| Ok(None), |_| Ok(false))
            .unwrap();
        let point = manager.points()[0];
        assert_eq!(point.velocity[0], 2_137.625);
        assert!((point.position[0] - 78.25).abs() < 0.001);
        assert!((point.position[2] - 68.75).abs() < 0.001);

        manager
            .advance::<()>(
                0.09,
                0.015,
                |_, _| {
                    Ok(Some(FlameWorldContact {
                        fraction: 0.5,
                        start_solid: false,
                        end: [100.0, 5.0, 69.0],
                        normal: [-1.0, 0.0, 0.0],
                    }))
                },
                |_| Ok(false),
            )
            .unwrap();
        assert_eq!(manager.points()[0].position, [88.0, 5.0, 69.0]);
        assert_eq!(manager.points()[0].velocity, [0.0; 3]);
        manager
            .advance::<()>(0.661, 0.015, |_, _| Ok(None), |_| Ok(false))
            .unwrap();
        assert!(manager.points().is_empty());
    }

    #[test]
    fn afterburn_stacks_caps_ticks_and_exempts_pyros() {
        let mut burn = Afterburn::ignite(None, PlayerClass::Scout, 7, 21, 1.0);
        assert_eq!(burn.duration, 3.4);
        assert_eq!(burn.next_damage_time, 1.5);
        assert_eq!(burn.advance(1.49), None);
        assert_eq!(burn.advance(1.5), Some(4.0));
        for _ in 0..30 {
            burn = Afterburn::ignite(Some(burn), PlayerClass::Scout, 7, 21, 1.5);
        }
        assert_eq!(burn.duration, 10.0);

        let mut pyro = Afterburn::ignite(None, PlayerClass::Pyro, 7, 21, 2.0);
        assert_eq!(pyro.duration, 0.25);
        assert_eq!(pyro.advance(2.5), None);
        assert_eq!(pyro.duration, 0.0);
    }

    #[test]
    fn shotgun_emits_ten_authored_pellets_with_centered_first_shot() {
        let pellets = shotgun_pellets(
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            42,
            0.625,
            false,
        )
        .unwrap();
        assert_eq!(pellets[0].direction, [1.0, 0.0, 0.0]);
        assert_eq!(pellets.len(), 10);
        assert!(
            pellets
                .iter()
                .all(|pellet| pellet.damage == 6.0 && pellet.range == 8_192.0)
        );
        assert!(pellets[1..].iter().any(|pellet| pellet.direction[1] != 0.0));

        let fixed = shotgun_pellets(
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            42,
            0.0,
            true,
        )
        .unwrap();
        assert_eq!(fixed[0].direction, fixed[9].direction);
        assert!(fixed[1].direction[1] < 0.0);
        assert!(fixed[2].direction[1] > 0.0);
    }

    #[test]
    fn airblast_reflects_incoming_velocity_and_redirects_ground_force() {
        let stationary = airblast_impulse([1.0, 0.0, 0.0], [0.0; 3], None);
        assert_eq!(stationary, [600.0, 0.0, 0.0]);
        let approaching = airblast_impulse([1.0, 0.0, 0.0], [-300.0, 0.0, 0.0], None);
        assert_eq!(approaching, [900.0, 0.0, 0.0]);
        let grounded = airblast_impulse([1.0, 0.0, 0.0], [0.0; 3], Some([0.0, 0.0, 1.0]));
        assert_eq!(grounded[2], 100.0);
        assert!((length(grounded) - 600.0).abs() < 0.001);
    }
}
