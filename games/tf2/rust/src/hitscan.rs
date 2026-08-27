//! Shared bullet and unlock rules from TF2's gun, revolver and shotgun classes.
use crate::{Weapon, ballistics::HitscanProfile, random::{prediction_seed, UniformRandomStream}};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub consecutive_shots: u32,
    pub last_fire_tick: u64,
    pub last_accuracy_tick: u64,
    pub idle_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub position: [f32; 3],
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub eye_forward: [f32; 3],
    pub in_air_due_to_explosion: bool,
    pub healed: bool,
    pub push_immune: bool,
    pub knocked_back: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct DamageGroup {
    pub victim: u32,
    pub amount: f32,
    pub range_multiplier: f32,
    pub position: [f32; 3],
    pub crit: crate::damage::CritKind,
    pub custom: crate::damage::CustomDamage,
}

#[derive(Clone, Copy)]
pub(crate) struct Attack {
    pub owner: u32,
    pub team: crate::PlayerTeam,
    pub weapon: Weapon,
    pub source: Option<crate::weapon::WeaponSource>,
    pub position: [f32; 3],
    pub center: [f32; 3],
    pub rules: BulletRules,
}

impl State {
    pub fn fired(&mut self, tick: u64, interval: f32) {
        self.consecutive_shots = self.consecutive_shots.saturating_add(1);
        self.last_fire_tick = tick;
        self.last_accuracy_tick = tick;
        self.idle_tick = tick + crate::weapon::delay_ticks(5.0, interval);
    }
    pub fn idle(&mut self, tick: u64) {
        if tick > self.idle_tick { self.consecutive_shots = 0; }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BulletRules {
    pub profile: HitscanProfile,
    pub fixed_pattern: bool,
    pub headshots: bool,
    pub backattack_minicrits: bool,
    pub airborne_minicrits: bool,
    pub critical_falloff: bool,
    pub knockback: bool,
    pub knockback_multiplier: f32,
    pub slow_on_hit: f32,
    pub first_shot_variance: f32,
}

impl BulletRules {
    pub fn resolve(weapon: Weapon, mut profile: HitscanProfile, state: State, elapsed_accuracy: f32, disguised: bool, mut query: impl FnMut(&str, f32) -> f32) -> Self {
        profile.damage = query("mult_dmg", profile.damage);
        if disguised { profile.damage = query("mult_dmg_disguised", profile.damage); }
        profile.pellets = query("mult_bullets_per_shot", f32::from(profile.pellets)) as u8;
        profile.spread = query("mult_spread_scale", profile.spread);
        if query("mult_spread_scales_consecutive", 0.0) != 0.0 && state.consecutive_shots != 0 {
            profile.spread *= 1.125 + ((state.consecutive_shots as f32 - 1.0) / 4.0).clamp(0.0, 1.0) * 0.375;
        }
        let headshots = weapon == Weapon::Revolver && query("set_weapon_mode", 0.0).round_ties_even() == 1.0;
        if headshots { profile.spread *= ((1.0 - elapsed_accuracy) / 0.5).clamp(0.0, 1.0); }
        Self { profile, headshots, fixed_pattern: query("fixed_shot_pattern", 0.0) != 0.0,
            backattack_minicrits: query("closerange_backattack_minicrits", 0.0) == 1.0,
            airborne_minicrits: query("mini_crit_airborne", 0.0) == 1.0,
            critical_falloff: query("crit_dmg_falloff", 0.0) != 0.0,
            knockback: query("set_scattergun_has_knockback", 0.0) == 1.0,
            knockback_multiplier: query("scattergun_knockback_mult", 3.0),
            slow_on_hit: query("mult_onhit_enemyspeed", 0.0),
            first_shot_variance: query("mult_spread_scale_first_shot", 0.0),
        }
    }

    pub fn direction(self, command: u32, pellet: u8, elapsed: f32, forward: [f32; 3], right: [f32; 3], up: [f32; 3]) -> [f32; 3] {
        let mut random = UniformRandomStream::from_seed(((prediction_seed(command) & 255) + u32::from(pellet)) as i32).unwrap();
        self.direction_with_random(pellet, elapsed, forward, right, up, |low, high| random.random_float(low, high))
    }

    pub fn direction_with_random(self, pellet: u8, elapsed: f32, forward: [f32; 3], right: [f32; 3], up: [f32; 3], mut random: impl FnMut(f32, f32) -> f32) -> [f32; 3] {
        const SMALL: [[f32; 2]; 10] = [[0.,0.], [1.,0.], [-1.,0.], [0.,-1.], [0.,1.], [0.85,-0.85], [0.85,0.85], [-0.85,-0.85], [-0.85,0.85], [0.,0.]];
        const LARGE: [[f32; 2]; 15] = [[0.,0.],[-0.5,0.],[-1.,0.],[0.5,0.],[1.,0.], [0.,0.5],[-0.5,0.5],[-1.,0.5],[0.5,0.5],[1.,0.5], [0.,-0.5],[-0.5,-0.5],[-1.,-0.5],[0.5,-0.5],[1.,-0.5]];
        let [x,y] = if !self.fixed_pattern {
            let variance = if pellet == 0 && elapsed > self.profile.accurate_after_seconds { self.first_shot_variance } else { 0.5 };
            if variance == 0.0 { [0.0, 0.0] } else { [random(-variance, variance) + random(-variance, variance), random(-variance, variance) + random(-variance, variance)] }
        } else if self.profile.pellets >= 15 {
            let [x,y] = LARGE[usize::from(pellet) % LARGE.len()];
            [x + random(-0.07, 0.07), y + random(-0.07, 0.07)]
        } else { SMALL[usize::from(pellet) % SMALL.len()].map(|value| value * 0.5) };
        let direction = std::array::from_fn(|axis| forward[axis] + x * self.profile.spread * right[axis] + y * self.profile.spread * up[axis]);
        let length = dot(direction, direction).sqrt();
        direction.map(|value| value / length)
    }

    pub fn range_multiplier(self, distance: f32, scattergun: bool) -> f32 {
        if self.profile.damage == 0.0 { return 1.0; }
        self.profile.damage_at_distance(distance, scattergun) / self.profile.damage
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }

pub fn backattack(attacker: [f32; 3], victim: [f32; 3], victim_forward: [f32; 3]) -> bool {
    let mut delta = std::array::from_fn(|axis| victim[axis] - attacker[axis]);
    if dot(delta, delta) >= 512.0 * 512.0 { return false; }
    delta[2] = 0.0;
    let length = dot(delta, delta).sqrt();
    length > 0.0 && dot(delta.map(|value| value / length), victim_forward) > 0.259
}

pub fn ambassador_headshot(elapsed: f32, headshot: bool, distance_squared_2d: f32) -> bool {
    elapsed > 1.0 && headshot && distance_squared_2d <= 1200.0 * 1200.0
}

pub fn ambassador_crosshair_scale(elapsed_accuracy: f32) -> f32 {
    0.75 + ((1.0 - elapsed_accuracy) / 0.5).clamp(0.0, 1.0) * 1.75
}

pub fn natascha_slow(distance_squared: f32) -> f32 {
    0.60 * (1.0 - ((distance_squared - 512.0 * 512.0) / (1536.0 * 1536.0 - 512.0 * 512.0)).clamp(0.0, 1.0))
}

pub fn knockback_allowed(damage: f32, distance_squared: f32, multiplier: f32) -> bool {
    (damage > 30.0 && distance_squared < 160_000.0) || multiplier > 3.0
}

pub fn knockback_impulse(attacker_center: [f32; 3], victim_center: [f32; 3], victim_size: [f32; 3], damage: f32, multiplier: f32) -> [f32; 3] {
    let delta = std::array::from_fn(|axis| victim_center[axis] - attacker_center[axis]);
    let length = dot(delta, delta).sqrt();
    let force = crate::damage::player_damage_force(victim_size, damage, multiplier);
    let mut impulse = if length == 0.0 { [0.0; 3] } else { delta.map(|value| value * force / length) };
    impulse[2] += 268.328_16;
    impulse
}

pub fn scattergun_jump(velocity: [f32; 3], eye_forward: [f32; 3]) -> [f32; 3] {
    let forward_velocity = dot(velocity, eye_forward);
    let mut velocity = std::array::from_fn(|axis| velocity[axis] + (-300.0 - forward_velocity) * eye_forward[axis]);
    velocity[2] += 50.0;
    velocity
}

pub fn bazaar_charge_rate(heads: i32) -> f32 { 50.0 + 0.25 * (heads.min(6) - 2) as f32 * 50.0 }

#[derive(Clone, Copy, Debug, PartialEq)]
struct MovementStun { expires: f32, amount: f32, forward_only: bool }

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MovementStuns {
    events: Vec<MovementStun>,
    active: Option<usize>,
    last_change: f32,
    lerp_target: f32,
    needs_fade_out: bool,
}

impl MovementStuns {
    pub fn active(&self) -> bool { self.active.is_some() }
    pub fn flags(&self) -> Option<u16> { self.active.map(|index| 1 | if self.events[index].forward_only { 4 } else { 0 }) }
    pub fn add(&mut self, now: f32, duration: f32, amount: f32, forward_only: bool) {
        if self.events.len() + 1 >= 250 { return; }
        let event = MovementStun { expires: now + duration, amount: amount.clamp(0.0, 1.0) * 255.0, forward_only };
        let previous = self.active.map(|index| self.events[index]);
        let stronger = previous.is_none_or(|active| event.amount > active.amount);
        if !stronger && previous.is_some_and(|active| event.expires < active.expires) { return; }
        let index = self.events.len();
        self.events.push(event);
        if stronger { self.active = Some(index); }
    }

    /// ConditionThink removes one expired active entry, then picks the first strongest.
    pub fn think(&mut self, now: f32) -> bool {
        if let Some(index) = self.active && now > self.events[index].expires {
            self.events.remove(index);
            self.active = if self.events.is_empty() { None } else {
                let mut strongest = 0;
                for index in 1..self.events.len() { if self.events[index].amount > self.events[strongest].amount { strongest = index; } }
                Some(strongest)
            };
        }
        self.active.is_some()
    }

    pub fn command(&mut self, now: f32, forward: f32, side: f32) -> (f32, f32) {
        let active = self.active.map(|index| self.events[index]);
        let amount = active.filter(|event| event.expires > now).map_or(0.0, |event| event.amount.clamp(0.0, 255.0) * (1.0 / 255.0));
        if amount != 0.0 {
            if self.lerp_target != amount { self.last_change = now; self.lerp_target = amount; self.needs_fade_out = true; }
            return (if active.is_some_and(|event| event.forward_only) { 0.0 } else { forward * (1.0 - amount) }, side * (1.0 - amount));
        }
        if self.last_change != 0.0 {
            if self.needs_fade_out { self.last_change = now; self.needs_fade_out = false; }
            let fade = (1.0 - (now - self.last_change) / 0.2).clamp(0.0, 1.0);
            if fade != 0.0 {
                let multiplier = 1.0 - self.lerp_target * fade;
                return (if active.is_some_and(|event| event.forward_only) { 0.0 } else { forward * multiplier }, side * multiplier);
            }
            self.lerp_target = 0.0; self.last_change = 0.0;
        }
        (forward, side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configured_rules(definition: u32, weapon: Weapon, disguised: bool, state: State) -> BulletRules {
        use crate::attribute::{AttributeGraph, AttributeEntity, ProviderKind, QueryValue};
        let schema = crate::equipment::schema();
        let mut graph = AttributeGraph::default();
        let mut entity = AttributeEntity::new(1, ProviderKind::Weapon);
        entity.attributes = schema.definition(definition).unwrap().static_attributes.clone();
        graph.insert(entity).unwrap();
        BulletRules::resolve(weapon, HitscanProfile::configured(weapon).unwrap(), state, 2.0, disguised, |hook, input| {
            let QueryValue::Numeric(value) = graph.query_numeric(schema, 1, 1, hook, input, false).unwrap().value else { unreachable!() };
            value
        })
    }

    #[test]
    fn configured_unlock_bullets_are_resolved_before_critical_and_range_damage() {
        for (definition, weapon, damage, pellets, spread) in [
            (45, Weapon::Scattergun, 5.4, 12, 0.0675),
            (1103, Weapon::Scattergun, 6.0, 10, 0.081),
            (425, Weapon::HeavyShotgun, 5.1, 10, 0.0675),
            (1153, Weapon::Shotgun, 4.8, 15, 0.0675),
            (415, Weapon::Shotgun, 6.0, 10, 0.0675),
            (424, Weapon::Minigun, 9.0, 4, 0.064),
            (312, Weapon::Minigun, 10.8, 4, 0.08),
            (41, Weapon::Minigun, 6.75, 4, 0.08),
            (61, Weapon::Revolver, 34.0, 1, 0.0),
            (460, Weapon::Revolver, 40.0, 1, 0.025),
            (220, Weapon::HandgunScoutPrimary, 12.0, 4, 0.04),
            (402, Weapon::SniperRifle, 50.0, 1, 0.0),
        ] {
            let rules = configured_rules(definition, weapon, false, State::default());
            assert!((rules.profile.damage - damage).abs() < 0.00001, "{definition}");
            assert_eq!(rules.profile.pellets, pellets, "{definition}");
            assert!((rules.profile.spread - spread).abs() < 0.00001, "{definition}");
        }
        assert_eq!(configured_rules(460, Weapon::Revolver, true, State::default()).profile.damage, 48.0);
        assert!(configured_rules(61, Weapon::Revolver, false, State::default()).critical_falloff);
        assert!(configured_rules(1103, Weapon::Scattergun, false, State::default()).backattack_minicrits);
        assert!(configured_rules(415, Weapon::Shotgun, false, State::default()).airborne_minicrits);
        assert_eq!(configured_rules(41, Weapon::Minigun, false, State::default()).slow_on_hit, 1.0);
    }

    #[test]
    fn panic_attack_uses_wide_authored_grid_and_two_jitter_draws_per_pellet() {
        let rules = configured_rules(1153, Weapon::Shotgun, false, State::default());
        assert!(rules.fixed_pattern);
        let mut calls = 0;
        let direction = rules.direction_with_random(7, 9.0, [1., 0., 0.], [0., 1., 0.], [0., 0., 1.], |lo, hi| {
            calls += 1; assert_eq!((lo, hi), (-0.07, 0.07)); 0.0
        });
        assert_eq!(calls, 2);
        assert!((direction[1] / direction[0] + 0.0675).abs() < 0.00001);
        assert!((direction[2] / direction[0] - 0.03375).abs() < 0.00001);
        let maximum = configured_rules(1153, Weapon::Shotgun, false, State { consecutive_shots: 5, ..State::default() });
        assert!((maximum.profile.spread - 0.0675 * 1.5).abs() < 0.00001);
        let mut state = State::default(); state.fired(100, 0.015);
        state.idle(state.idle_tick); assert_eq!(state.consecutive_shots, 1);
        state.idle(state.idle_tick + 1); assert_eq!(state.consecutive_shots, 0);
    }
    #[test]
    fn movement_stuns_keep_source_priority_expiry_and_fade_order() {
        let mut stuns = MovementStuns::default();
        stuns.add(1.0, 0.2, 0.6, false);
        assert!(stuns.think(1.0));
        let slowed = stuns.command(1.0, 100.0, 50.0);
        assert!((slowed.0 - 40.0).abs() < 0.0001);
        stuns.add(1.05, 0.1, 0.2, false); // weaker and expires first: ignored
        assert_eq!(stuns.events.len(), 1);
        stuns.add(1.1, 0.3, 1.0, true);
        assert_eq!(stuns.command(1.1, 100.0, 50.0), (0.0, 0.0));
        stuns.think(1.41); // the older expired entry becomes active for this think
        assert_eq!(stuns.command(1.41, 100.0, 50.0), (0.0, 0.0));
        assert!(!stuns.think(1.42));
        let faded = stuns.command(1.51, 100.0, 50.0);
        assert!((faded.0 - 50.0).abs() < 0.001);
        assert_eq!(stuns.command(1.7, 100.0, 50.0), (100.0, 50.0));
    }
    #[test]
    fn authored_special_hit_thresholds_are_strict_and_use_the_correct_distance() {
        assert!(backattack([0.;3], [511.,0.,0.], [1.,0.,0.]));
        assert!(!backattack([0.;3], [512.,0.,0.], [1.,0.,0.]));
        assert!(!backattack([0.;3], [511.,0.,40.], [1.,0.,0.]));
        assert!(!backattack([0.;3], [20.,0.,0.], [0.259,0.,0.]));
        assert!(!ambassador_headshot(1.0, true, 1.0));
        assert!(ambassador_headshot(1.001, true, 1200.0*1200.0));
        assert!(!ambassador_headshot(1.001, true, 1200.1*1200.1));
        assert_eq!(ambassador_crosshair_scale(0.0), 2.5);
        assert_eq!(ambassador_crosshair_scale(0.5), 2.5);
        assert_eq!(ambassador_crosshair_scale(0.75), 1.625);
        assert_eq!(ambassador_crosshair_scale(1.0), 0.75);
        assert_eq!(ambassador_crosshair_scale(2.0), 0.75);
        assert!(!knockback_allowed(30.0, 0.0, 3.0));
        assert!(!knockback_allowed(31.0, 160_000.0, 3.0));
        assert!(knockback_allowed(31.0, 159_999.0, 3.0));
    }
    #[test]
    fn authored_spin_slow_charge_and_jump_math_preserve_source_values() {
        assert_eq!(natascha_slow(0.0), 0.6);
        assert_eq!(natascha_slow(512.0*512.0), 0.6);
        assert_eq!(natascha_slow(1536.0*1536.0), 0.0);
        assert_eq!((0..=7).map(bazaar_charge_rate).collect::<Vec<_>>(), [25.,37.5,50.,62.5,75.,87.5,100.,100.]);
        assert_eq!(scattergun_jump([240.,100.,200.], [1.,0.,0.]), [-300.,100.,250.]);
        assert_eq!(knockback_impulse([0.;3], [10.,0.,0.], [48.,48.,82.], 60.,3.), [180.,0.,268.328_16]);
    }
}
