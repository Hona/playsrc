#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlastKind {
    Rocket,
    Sticky,
    ModifiedRocket { damage: f32, radius: f32 },
    Flare { damage: f32, radius: f32 },
    Grenade { damage: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlastClass {
    Soldier,
    Demoman,
    Pyro,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerBlastTarget {
    pub origin: [f32; 3],
    pub world_center: [f32; 3],
    pub direct_hit: bool,
    pub visible: bool,
    pub self_damage: bool,
    pub nearest_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlastDamage {
    pub radius: f32,
    pub distance: f32,
    pub damage: f32,
    pub health_points: i32,
    pub damage_for_force: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlastImpulse {
    pub impulse: [f32; 3],
    pub magnitude: f32,
}

pub fn player_blast_damage(
    kind: BlastKind,
    explosion: [f32; 3],
    target: PlayerBlastTarget,
) -> Option<BlastDamage> {
    if !target.visible {
        return None;
    }
    let (base_damage, ordinary_radius, self_radius) = match kind {
        BlastKind::Rocket => (90.0_f32, 146.0_f32, 121.0_f32),
        BlastKind::Sticky => (120.0_f32, 146.0_f32, 146.0_f32),
        BlastKind::Grenade {damage,radius} => (if target.self_damage {100.0}else{damage},radius,146.0),
        BlastKind::ModifiedRocket { damage, radius } => (
            if target.self_damage { 90.0 } else { damage }, radius, 121.0),
        BlastKind::Flare { damage, radius } => (
            if target.self_damage { 30.0 } else { damage }, radius, 100.0),
    };
    let radius = if target.self_damage {
        self_radius
    } else {
        ordinary_radius
    };
    let distance = if target.direct_hit {
        0.0
    } else {
        length(sub(explosion, target.world_center)).min(length(sub(explosion, target.origin)))
    };
    if target.nearest_distance > radius || !target.self_damage && radius <= 0.0 {
        return None;
    }
    let fraction = (distance / radius).clamp(0.0, 1.0);
    let mut damage = base_damage + (base_damage * 0.5 - base_damage) * fraction;
    if target.self_damage && matches!(kind,BlastKind::Sticky|BlastKind::Grenade {..}) {
        damage *= 0.75;
    }
    Some(BlastDamage {
        radius,
        distance,
        damage,
        health_points: (damage + 0.5) as i32,
        damage_for_force: damage,
    })
}

pub fn apply_self_damage_rules(
    mut damage: BlastDamage,
    class: BlastClass,
    grounded: bool,
    in_water_flag: bool,
) -> BlastDamage {
    if class == BlastClass::Soldier && !grounded && !in_water_flag {
        damage.damage *= 0.60;
        damage.damage_for_force = damage.damage;
        damage.health_points = (damage.damage + 0.5) as i32;
    }
    damage
}

pub fn self_blast_impulse(
    class: BlastClass,
    grounded: bool,
    crouched_hull: bool,
    world_size: [f32; 3],
    player_world_center: [f32; 3],
    inflictor_world_center: [f32; 3],
    damage_for_force: f32,
) -> BlastImpulse {
    let mut size = world_size;
    if crouched_hull {
        size[2] = 55.0;
    }
    let scale = match class {
        BlastClass::Soldier if grounded => 5.0,
        BlastClass::Soldier => 10.0,
        BlastClass::Demoman | BlastClass::Other => 9.0,
        BlastClass::Pyro => 8.5,
    };
    damage_impulse(size, player_world_center, inflictor_world_center, damage_for_force, scale)
}

pub fn damage_impulse(
    size: [f32; 3],
    player_world_center: [f32; 3],
    inflictor_world_center: [f32; 3],
    damage_for_force: f32,
    scale: f32,
) -> BlastImpulse {
    let volume = size[0] * size[1] * size[2];
    let magnitude = if volume > 0.0 {
        (damage_for_force * ((48.0 * 48.0 * 82.0) / volume) * scale).min(1000.0)
    } else {
        0.0
    };
    let incoming = sub(
        sub(inflictor_world_center, [0.0, 0.0, 10.0]),
        player_world_center,
    );
    let direction = normalize(incoming);
    BlastImpulse {
        impulse: scale_vector(direction, -magnitude),
        magnitude,
    }
}

pub fn generic_push_impulse(impulse: [f32; 3], grounded: bool, horizontal: f32, vertical: f32) -> [f32; 3] {
    let mut force = impulse.map(|value| value * horizontal);
    if grounded { force[2] = force[2].max(268.328_16); }
    force[2] *= vertical;
    force
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale_vector(value: [f32; 3], scale: f32) -> [f32; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn length(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

pub fn nearest_hull_distance(source: [f32; 3], origin: [f32; 3], mins: [f32; 3], maxs: [f32; 3]) -> f32 {
    let nearest = std::array::from_fn(|axis| source[axis].clamp(origin[axis] + mins[axis], origin[axis] + maxs[axis]));
    length(sub(source, nearest))
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = length(value);
    if length > 0.0 {
        scale_vector(value, 1.0 / length)
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_grenade_timer_damage_does_not_replace_weapon_base_self_damage() {
        let kind=BlastKind::Grenade {damage:60.0,radius:146.0};
        let enemy=player_blast_damage(kind,[0.0;3],target(0.0,false)).unwrap();
        let own=player_blast_damage(kind,[0.0;3],target(0.0,true)).unwrap();
        assert_eq!((enemy.damage,enemy.health_points),(60.0,60));
        assert_eq!((own.damage,own.damage_for_force,own.health_points),(75.0,75.0,75));
    }

    fn target(distance: f32, self_damage: bool) -> PlayerBlastTarget {
        PlayerBlastTarget {
            nearest_distance: distance,
            origin: [distance, 0.0, 0.0],
            world_center: [distance, 0.0, 41.0],
            direct_hit: false,
            visible: true,
            self_damage,
        }
    }

    #[test]
    fn direct_splash_and_self_radius_matrix_uses_half_falloff() {
        let direct = player_blast_damage(
            BlastKind::Rocket,
            [0.0; 3],
            PlayerBlastTarget {
                direct_hit: true,
                ..target(146.0, false)
            },
        )
        .unwrap();
        assert_eq!(direct.damage, 90.0);
        assert_eq!(direct.health_points, 90);

        let edge = player_blast_damage(BlastKind::Rocket, [0.0; 3], target(146.0, false)).unwrap();
        assert_eq!(edge.damage, 45.0);
        assert_eq!(edge.health_points, 45);
        assert!(player_blast_damage(BlastKind::Rocket, [0.0; 3], target(146.01, false)).is_none());
        assert!(
            player_blast_damage(
                BlastKind::Rocket,
                [0.0; 3],
                PlayerBlastTarget {
                    visible: false,
                    ..target(0.0, false)
                },
            )
            .is_none()
        );

        let rocket_self =
            player_blast_damage(BlastKind::Rocket, [0.0; 3], target(121.0, true)).unwrap();
        assert_eq!(rocket_self.radius, 121.0);
        assert_eq!(rocket_self.damage, 45.0);
        assert!(player_blast_damage(BlastKind::Rocket, [0.0; 3], target(121.01, true)).is_none());

        let airborne = apply_self_damage_rules(
            player_blast_damage(BlastKind::Rocket, [0.0; 3], target(0.0, true)).unwrap(),
            BlastClass::Soldier,
            false,
            false,
        );
        assert_eq!(airborne.damage, 54.000004);
        assert_eq!(airborne.health_points, 54);
        assert_eq!(airborne.damage_for_force, airborne.damage);

        let sticky_self =
            player_blast_damage(BlastKind::Sticky, [0.0; 3], target(0.0, true)).unwrap();
        assert_eq!(sticky_self.damage, 90.0);
        assert_eq!(sticky_self.health_points, 90);
    }

    #[test]
    fn airborne_soldier_reduction_uses_the_retained_in_water_flag() {
        let rocket = player_blast_damage(BlastKind::Rocket, [0.0; 3], target(0.0, true)).unwrap();
        for (grounded, in_water_flag, health, damage_for_force) in [
            (false, false, 54, 54.000004),
            (false, true, 90, 90.0),
            (true, false, 90, 90.0),
            (true, true, 90, 90.0),
        ] {
            let damage =
                apply_self_damage_rules(rocket, BlastClass::Soldier, grounded, in_water_flag);
            assert_eq!(damage.health_points, health);
            assert_eq!(damage.damage_for_force, damage_for_force);
        }

        let sticky = player_blast_damage(BlastKind::Sticky, [0.0; 3], target(0.0, true)).unwrap();
        for grounded in [false, true] {
            for in_water_flag in [false, true] {
                let damage =
                    apply_self_damage_rules(sticky, BlastClass::Demoman, grounded, in_water_flag);
                assert_eq!(damage.health_points, 90);
                assert_eq!(damage.damage_for_force, 90.0);
            }
        }
    }

    #[test]
    fn force_matrix_uses_ground_class_and_current_hull_volume() {
        let center = [0.0, 0.0, 41.0];
        let source = [0.0, 0.0, 0.0];
        let standing_ground = self_blast_impulse(
            BlastClass::Soldier,
            true,
            false,
            [48.0, 48.0, 82.0],
            center,
            source,
            90.0,
        );
        assert_eq!(standing_ground.magnitude, 450.0);
        assert!(standing_ground.impulse[2] > 0.0);

        let standing_air = self_blast_impulse(
            BlastClass::Soldier,
            false,
            false,
            [48.0, 48.0, 82.0],
            center,
            source,
            90.0,
        );
        assert_eq!(standing_air.magnitude, 900.0);

        let crouched_ground = self_blast_impulse(
            BlastClass::Soldier,
            true,
            true,
            [48.0, 48.0, 62.0],
            [0.0, 0.0, 31.0],
            source,
            90.0,
        );
        assert_eq!(crouched_ground.magnitude.to_bits(), 670.9091_f32.to_bits());
        assert!(crouched_ground.magnitude > standing_ground.magnitude);

        let crouched_air = self_blast_impulse(
            BlastClass::Soldier,
            false,
            true,
            [48.0, 48.0, 62.0],
            [0.0, 0.0, 31.0],
            source,
            90.0,
        );
        assert_eq!(crouched_air.magnitude, 1000.0);

        let demo = self_blast_impulse(
            BlastClass::Demoman,
            true,
            false,
            [48.0, 48.0, 82.0],
            center,
            source,
            90.0,
        );
        assert_eq!(demo.magnitude, 810.0);
    }
}
