use md5::{Digest, Md5};

use crate::{Weapon, random::UniformRandomStream};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitscanProfile {
    pub pellets: u8,
    pub damage: f32,
    pub range: f32,
    pub spread: f32,
    pub accurate_after_seconds: f32,
}

impl HitscanProfile {
    pub const fn configured(weapon: Weapon) -> Option<Self> {
        match weapon {
            Weapon::Scattergun | Weapon::Shotgun => Some(Self {
                pellets: 10,
                damage: 6.0,
                range: 8192.0,
                spread: 0.0675,
                accurate_after_seconds: 0.25,
            }),
            Weapon::Pistol => Some(Self {
                pellets: 1,
                damage: 15.0,
                range: 4096.0,
                spread: 0.04,
                accurate_after_seconds: 1.25,
            }),
            _ => None,
        }
    }

    pub fn pellet_direction(
        self,
        command_number: u32,
        pellet: u8,
        seconds_since_previous_shot: f32,
        forward: [f32; 3],
        right: [f32; 3],
        up: [f32; 3],
    ) -> [f32; 3] {
        if pellet == 0 && seconds_since_previous_shot > self.accurate_after_seconds {
            return forward;
        }

        let digest = Md5::digest(command_number.to_le_bytes());
        let prediction_seed = u32::from_le_bytes(digest[6..10].try_into().unwrap()) & 0x7fff_ffff;
        let seed = (prediction_seed & 255).wrapping_add(u32::from(pellet)) as i32;
        let mut random = UniformRandomStream::from_seed(seed).unwrap();
        let x = random.random_float(-0.5, 0.5) + random.random_float(-0.5, 0.5);
        let y = random.random_float(-0.5, 0.5) + random.random_float(-0.5, 0.5);
        let direction = [
            forward[0] + x * self.spread * right[0] + y * self.spread * up[0],
            forward[1] + x * self.spread * right[1] + y * self.spread * up[1],
            forward[2] + x * self.spread * right[2] + y * self.spread * up[2],
        ];
        let length = direction
            .iter()
            .map(|component| component * component)
            .sum::<f32>()
            .sqrt();
        direction.map(|component| component / length)
    }

    pub fn damage_at_distance(self, distance: f32, scattergun: bool) -> f32 {
        let center = (1.0 - distance.max(1.0) / 1024.0).clamp(0.0, 1.0);
        let value = (center - 0.1).max(0.0) + 0.1;
        let mut range = self.damage * 0.5;
        if scattergun && value > 0.5 {
            range *= 1.5;
        }
        let spline = value * value * (3.0 - 2.0 * value);
        self.damage + (-range + 2.0 * range * spline)
    }
}

pub const MELEE_RANGE: f32 = 48.0;
pub const MELEE_HULL_RADIUS: f32 = 18.0;
pub const BAT_DAMAGE: f32 = 35.0;
pub const SHOVEL_DAMAGE: f32 = 65.0;
pub const MELEE_SMACK_DELAY: f32 = 0.2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_scout_hitscan_profiles_match_weapon_scripts() {
        assert_eq!(
            HitscanProfile::configured(Weapon::Scattergun),
            Some(HitscanProfile {
                pellets: 10,
                damage: 6.0,
                range: 8192.0,
                spread: 0.0675,
                accurate_after_seconds: 0.25,
            })
        );
        assert_eq!(
            HitscanProfile::configured(Weapon::Shotgun),
            HitscanProfile::configured(Weapon::Scattergun),
        );
        assert_eq!(
            HitscanProfile::configured(Weapon::Pistol),
            Some(HitscanProfile {
                pellets: 1,
                damage: 15.0,
                range: 4096.0,
                spread: 0.04,
                accurate_after_seconds: 1.25,
            })
        );
    }

    #[test]
    fn first_pellet_accuracy_preserves_source_weapon_specific_delay() {
        let scattergun = HitscanProfile::configured(Weapon::Scattergun).unwrap();
        let pistol = HitscanProfile::configured(Weapon::Pistol).unwrap();
        let forward = [1.0, 0.0, 0.0];
        let right = [0.0, 1.0, 0.0];
        let up = [0.0, 0.0, 1.0];
        assert_eq!(
            scattergun.pellet_direction(17, 0, 0.26, forward, right, up),
            forward
        );
        assert_ne!(
            scattergun.pellet_direction(17, 1, 0.26, forward, right, up),
            forward
        );
        assert_ne!(
            pistol.pellet_direction(17, 0, 1.25, forward, right, up),
            forward
        );
        assert_eq!(
            pistol.pellet_direction(17, 0, 1.26, forward, right, up),
            forward
        );
    }

    #[test]
    fn deterministic_damage_preserves_scattergun_ramp_and_long_range_falloff() {
        let scattergun = HitscanProfile::configured(Weapon::Scattergun).unwrap();
        let pistol = HitscanProfile::configured(Weapon::Pistol).unwrap();
        assert!((scattergun.damage_at_distance(1.0, true) - 10.5).abs() < 0.001);
        assert_eq!(scattergun.damage_at_distance(512.0, true), 6.0);
        assert!((pistol.damage_at_distance(1.0, false) - 22.5).abs() < 0.001);
        assert!(pistol.damage_at_distance(1024.0, false) < 8.0);
    }
}
