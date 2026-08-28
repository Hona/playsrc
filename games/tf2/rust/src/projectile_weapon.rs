//! Non-rigidbody projectile weapon rules. Attribute inputs are resolved by the
//! equipped item's provider graph; this module does not own an item catalog.

use crate::damage::CritKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponId {
    RocketLauncher,
    RocketLauncherDirectHit,
    FlareGun,
    FlareGunRevenge,
}

pub fn weapon_id(weapon: crate::Weapon) -> Option<WeaponId> {
    use crate::Weapon;
    Some(match weapon {
        Weapon::RocketLauncher
        | Weapon::Original
        | Weapon::BlackBox
        | Weapon::LibertyLauncher
        | Weapon::RocketJumper
        | Weapon::AirStrike => WeaponId::RocketLauncher,
        Weapon::DirectHit => WeaponId::RocketLauncherDirectHit,
        Weapon::FlareGun | Weapon::Detonator | Weapon::ScorchShot => WeaponId::FlareGun,
        Weapon::Manmelter => WeaponId::FlareGunRevenge,
        _ => return None,
    })
}

pub const ROCKET_SPEED: f32 = 1100.0;
pub const ROCKET_RADIUS: f32 = 146.0;
pub const ROCKET_JUMP_RADIUS: f32 = 121.0;
pub const FLARE_SPEED: f32 = 2000.0;
pub const FLARE_GRAVITY: f32 = 0.3;
pub const FLARE_RADIUS: f32 = 110.0;
pub const FLARE_JUMP_RADIUS: f32 = 100.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WeaponEffect {
    ChargeStart = 1,
    ChargeStop = 2,
    Absorb = 3,
    Idle = 4,
}

pub fn rocket_distance_multiplier(distance: f32) -> f32 {
    let center = (1.0 - distance.max(1.0) / 1024.0).clamp(0.0, 1.0);
    let value = (center - 0.1).max(0.0) + 0.1;
    let range = if value > 0.5 { 0.25 } else { 0.5 };
    let spline = value * value * (3.0 - 2.0 * value);
    1.0 + (-range + 2.0 * range * spline)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BowLaunch {
    pub damage: f32,
    pub speed: f32,
    pub gravity: f32,
    pub inaccurate: bool,
}

/// Charge is capped at one second *before* dividing by the attribute-adjusted
/// charge time. Holding past five seconds changes spread, not damage or speed.
pub fn bow_launch(
    elapsed: f32,
    charge_max_time: f32,
    modified_base_damage: f32,
    modified_script_damage: f32,
) -> BowLaunch {
    let charge = (elapsed.min(1.0) / charge_max_time).clamp(0.0, 1.0);
    BowLaunch {
        damage: modified_base_damage + modified_script_damage * charge,
        speed: 1800.0 + 800.0 * charge,
        gravity: 0.5 + (0.1 - 0.5) * charge,
        inaccurate: elapsed >= 5.0,
    }
}

/// Crossbow ramp is based on projectile lifetime, not straight-line distance
/// from the owner (who can move or reflect the projectile after launch).
pub fn healing_bolt_damage(launch_damage: f32, lifetime: f32) -> f32 {
    launch_damage * (0.5 + (lifetime / 0.6).clamp(0.0, 1.0) * 0.5)
}

pub fn healing_bolt_charge(
    actual_healed: i32,
    time_since_patient_damaged: f32,
    tick_interval: f32,
) -> f32 {
    let scale = 3.0 + ((time_since_patient_damaged - 10.0) / 5.0).clamp(0.0, 1.0) * -2.0;
    (actual_healed as f32 / (24.0 * scale)) * tick_interval
}

/// CTFBaseRocket::GetRadius queries the launcher's radius and the *current*
/// attacker's blast-jump attribute at explosion time, not at launch time.
pub fn rocket_radius(
    launcher_radius: f32,
    attacker_blast_jumping: bool,
    attacker_jump_attack_rate: f32,
) -> f32 {
    if attacker_blast_jumping && attacker_jump_attack_rate != 1.0 {
        launcher_radius * 0.8
    } else {
        launcher_radius
    }
}

/// RadiusDamage accumulates accepted damage to enemy players before healing
/// the attacker, then runs the separate base-weapon rocket-jump damage pass.
/// Buildings and rejected damage never contribute to this sum.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RadiusHits {
    pub enemy_players: u32,
    pub enemy_damage: i32,
}

impl RadiusHits {
    pub fn record(&mut self, accepted_damage: i32, enemy_player: bool) {
        if accepted_damage != 0 && enemy_player {
            self.enemy_players += 1;
            self.enemy_damage += accepted_damage;
        }
    }

    pub fn healing(self, maximum_heal: i32, script_damage: f32) -> i32 {
        if self.enemy_damage <= 0 || maximum_heal == 0 {
            return 0;
        }
        (maximum_heal as f32 * (self.enemy_damage as f32 / script_damage).clamp(0.0, 1.0)) as i32
    }
}

/// These promotions happen during damage processing, not when the projectile
/// is spawned. A normal jump is deliberately not an explosive airborne state.
pub fn conditional_minicrit(
    current: CritKind,
    burning: bool,
    airborne_due_to_explosion: bool,
    minicrit_burning: i32,
    minicrit_airborne: i32,
) -> CritKind {
    if current == CritKind::Full {
        current
    } else if current == CritKind::Mini
        || burning && minicrit_burning == 1
        || airborne_due_to_explosion && minicrit_airborne == 1
    {
        CritKind::Mini
    } else {
        CritKind::None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlareType {
    Standard,
    Detonator,
    Manmelter,
    ScorchShot,
}

impl FlareType {
    pub fn from_attribute(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Standard),
            1 => Some(Self::Detonator),
            2 => Some(Self::Manmelter),
            3 => Some(Self::ScorchShot),
            _ => None,
        }
    }

    pub fn direct_crit(self, current: CritKind, victim_burning: bool) -> CritKind {
        if self == Self::Standard && victim_burning {
            CritKind::Full
        } else {
            current
        }
    }

    pub fn trail(self, blue: bool, critical: bool) -> &'static str {
        match (self, blue, critical) {
            (Self::Manmelter, _, _) => "drg_manmelter_projectile",
            (Self::ScorchShot, false, false) => "scorchshot_trail_red",
            (Self::ScorchShot, false, true) => "scorchshot_trail_crit_red",
            (Self::ScorchShot, true, false) => "scorchshot_trail_blue",
            (Self::ScorchShot, true, true) => "scorchshot_trail_crit_blue",
            (_, false, false) => "flaregun_trail_red",
            (_, false, true) => "flaregun_trail_crit_red",
            (_, true, false) => "flaregun_trail_blue",
            (_, true, true) => "flaregun_trail_crit_blue",
        }
    }
}

/// Both launch speed and gravity use mult_projectile_speed in the SDK.
pub fn flare_flight(speed_multiplier: f32) -> (f32, f32) {
    (
        FLARE_SPEED * speed_multiplier,
        FLARE_GRAVITY * speed_multiplier,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlareImpact {
    /// Direct damage precedes the separate push, then the flare becomes debris.
    Bounce {
        push: [f32; 3],
        stun: bool,
        velocity: [f32; 3],
    },
    /// World impact starts a strict-time delayed effect for ordinary flares.
    Embed {
        effect_due: f32,
    },
    /// Detonator wall contact can blast-jump but cannot ignite nearby enemies.
    Detonate {
        self_only: bool,
    },
    Remove,
}

pub fn flare_impact(
    kind: FlareType,
    now: f32,
    player: bool,
    burning_before_damage: bool,
    enemy: bool,
    immune_to_push: bool,
    knocked_into_air: bool,
    velocity: [f32; 3],
) -> FlareImpact {
    if kind == FlareType::ScorchShot && player {
        let length = velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        let mut direction = if length > 0.0 {
            velocity.map(|v| v / length)
        } else {
            [0.0; 3]
        };
        direction[2] = 1.0;
        let can_push = enemy && !immune_to_push;
        let force = if !can_push {
            0.0
        } else if burning_before_damage {
            400.0
        } else {
            100.0
        };
        return FlareImpact::Bounce {
            push: direction.map(|v| v * force),
            stun: can_push && !knocked_into_air,
            // The caller adds the three authority RandomVector(-2, 2) draws
            // after calculating the orientation from this unjittered velocity.
            velocity: [velocity[0] * 0.07, velocity[1] * 0.07, 100.0],
        };
    }
    if player {
        return FlareImpact::Remove;
    }
    match kind {
        FlareType::Detonator => FlareImpact::Detonate { self_only: true },
        FlareType::ScorchShot => FlareImpact::Detonate { self_only: false },
        _ => FlareImpact::Embed {
            effect_due: now + 0.1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bow_charge_boundaries_preserve_damage_speed_gravity_and_overdraw() {
        let initial = bow_launch(0.0, 1.0, 50.0, 70.0);
        assert_eq!(
            (initial.damage, initial.speed, initial.gravity),
            (50.0, 1800.0, 0.5)
        );
        let full = bow_launch(1.0, 1.0, 50.0, 70.0);
        assert_eq!((full.damage, full.speed), (120.0, 2600.0));
        assert!((full.gravity - 0.1).abs() < 0.000001);
        assert!(!bow_launch(4.999, 1.0, 50.0, 70.0).inaccurate);
        let overdrawn = bow_launch(5.0, 1.0, 50.0, 70.0);
        assert!(overdrawn.inaccurate);
        assert_eq!(overdrawn.damage, full.damage);
        assert_eq!(overdrawn.speed, full.speed);
        assert_eq!(bow_launch(0.5, 0.5, 50.0, 70.0).damage, 120.0);
        // A slower charge still observes GetCurrentCharge's one-second cap.
        assert_eq!(bow_launch(2.0, 2.0, 50.0, 70.0).damage, 85.0);
    }

    #[test]
    fn crossbow_damage_ramps_by_age_and_charge_uses_actual_not_requested_healing() {
        assert_eq!(healing_bolt_damage(75.0, 0.0), 37.5);
        assert_eq!(healing_bolt_damage(75.0, 0.3), 56.25);
        assert_eq!(healing_bolt_damage(75.0, 0.6), 75.0);
        assert_eq!(healing_bolt_damage(75.0, 5.0), 75.0);
        assert_eq!(healing_bolt_charge(0, 15.0, 0.015), 0.0);
        assert_eq!(healing_bolt_charge(24, 15.0, 0.015), 0.015);
        assert_eq!(healing_bolt_charge(72, 10.0, 0.015), 0.015);
        assert_eq!(healing_bolt_charge(48, 12.5, 0.015), 0.015);
    }

    #[test]
    fn radius_uses_live_attacker_state_and_never_changes_the_jump_radius() {
        let direct_hit_radius = ROCKET_RADIUS * 0.3;
        assert_eq!(
            rocket_radius(direct_hit_radius, false, 1.0),
            direct_hit_radius
        );
        assert_eq!(
            rocket_radius(direct_hit_radius, true, 1.0),
            direct_hit_radius
        );
        assert_eq!(
            rocket_radius(ROCKET_RADIUS * 0.9, false, 0.4),
            ROCKET_RADIUS * 0.9
        );
        assert_eq!(
            rocket_radius(ROCKET_RADIUS * 0.9, true, 0.4),
            ROCKET_RADIUS * 0.9 * 0.8
        );
        assert_eq!(ROCKET_JUMP_RADIUS, 121.0);
    }

    #[test]
    fn black_box_heals_once_per_explosion_from_accepted_enemy_player_damage() {
        let mut hits = RadiusHits::default();
        hits.record(90, false); // Building, friendly, or self damage.
        hits.record(0, true); // Invulnerable enemy.
        assert_eq!(hits.healing(20, 90.0), 0);
        hits.record(22, true);
        assert_eq!(hits.healing(20, 90.0), 4); // Truncation, not rounding.
        hits.record(23, true);
        assert_eq!(hits.healing(20, 90.0), 10);
        hits.record(90, true);
        assert_eq!(hits.enemy_players, 3);
        assert_eq!(hits.healing(20, 90.0), 20);
        assert_eq!(hits.healing(0, 90.0), 0);
    }

    #[test]
    fn direct_hit_does_not_minicrit_a_normal_jump_or_demote_full_crits() {
        assert_eq!(
            conditional_minicrit(CritKind::None, false, false, 0, 1),
            CritKind::None
        );
        assert_eq!(
            conditional_minicrit(CritKind::None, false, true, 0, 1),
            CritKind::Mini
        );
        assert_eq!(
            conditional_minicrit(CritKind::Full, false, true, 0, 1),
            CritKind::Full
        );
        assert_eq!(
            conditional_minicrit(CritKind::None, true, false, 1, 0),
            CritKind::Mini
        );
    }

    #[test]
    fn manmelter_speed_attribute_also_scales_gravity() {
        assert_eq!(flare_flight(1.0), (2000.0, 0.3));
        assert_eq!(flare_flight(1.5), (3000.0, 0.3 * 1.5));
    }

    #[test]
    fn only_standard_flare_promotes_a_burning_direct_target_to_full_crit() {
        for kind in [
            FlareType::Standard,
            FlareType::Detonator,
            FlareType::Manmelter,
            FlareType::ScorchShot,
        ] {
            assert_eq!(kind.direct_crit(CritKind::None, false), CritKind::None);
            assert_eq!(kind.direct_crit(CritKind::Full, false), CritKind::Full);
            assert_eq!(
                kind.direct_crit(CritKind::None, true),
                if kind == FlareType::Standard {
                    CritKind::Full
                } else {
                    CritKind::None
                }
            );
        }
    }

    #[test]
    fn detonator_world_and_air_bursts_are_not_the_same_damage_operation() {
        assert_eq!(
            flare_impact(
                FlareType::Detonator,
                1.0,
                false,
                false,
                false,
                false,
                false,
                [0.0; 3]
            ),
            FlareImpact::Detonate { self_only: true }
        );
        assert_eq!(
            flare_impact(
                FlareType::ScorchShot,
                1.0,
                false,
                false,
                false,
                false,
                false,
                [0.0; 3]
            ),
            FlareImpact::Detonate { self_only: false }
        );
        assert_eq!(
            flare_impact(
                FlareType::Standard,
                1.0,
                false,
                false,
                false,
                false,
                false,
                [0.0; 3]
            ),
            FlareImpact::Embed { effect_due: 1.1 }
        );
    }

    #[test]
    fn scorchshot_bounces_even_when_push_is_rejected() {
        for (burning, immune, knocked, expected_force, stun) in [
            (false, false, false, 100.0, true),
            (true, false, false, 400.0, true),
            (true, false, true, 400.0, false),
            (true, true, false, 0.0, false),
        ] {
            assert_eq!(
                flare_impact(
                    FlareType::ScorchShot,
                    1.0,
                    true,
                    burning,
                    true,
                    immune,
                    knocked,
                    [2000.0, 0.0, 0.0]
                ),
                FlareImpact::Bounce {
                    push: [expected_force, 0.0, expected_force],
                    stun,
                    velocity: [140.0, 0.0, 100.0]
                }
            );
        }
    }
}
/// Additional configured presentation roots shared by bundle closure and runtime admission.
pub const PARTICLE_ROOTS: &[&str] = &[
    "rockettrail_underwater",
    "rockettrail_RocketJumper",
    "rockettrail_airstrike",
    "rockettrail_airstrike_line",
    "critical_rocket_red",
    "critical_rocket_blue",
    "ExplosionCore_Wall_Jumper",
    "ExplosionCore_MidAir_underwater",
    "flaregun_trail_red",
    "flaregun_trail_blue",
    "flaregun_trail_crit_red",
    "flaregun_trail_crit_blue",
    "scorchshot_trail_red",
    "scorchshot_trail_blue",
    "scorchshot_trail_crit_red",
    "scorchshot_trail_crit_blue",
    "drg_manmelter_projectile",
    "drg_manmelter_muzzleflash",
    "drg_manmelter_vacuum",
    "drg_manmelter_vacuum_flames",
    "drg_manmelter_idle",
    "flaregun_destroyed",
    "Explosions_MA_FlyingEmbers",
    "ExplosionCore_MidAir_Flare",
    "drg_bison_idle",
];
