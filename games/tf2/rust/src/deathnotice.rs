//! Killing names for the implemented local damage sources, following
//! CTFGameRules::GetKillingWeaponName (Source SDK 2013).
use crate::{Weapon, class::PlayerClass};

#[derive(Clone, Debug, Default)]
pub struct DamagerHistory(Vec<(u32, f32)>);

impl DamagerHistory {
    pub fn record(&mut self, attacker: u32, curtime: f32) {
        if attacker == 0 { return; }
        self.0.retain(|entry| entry.0 != attacker);
        // CHistoryVector / CUtlSortVector: equal timestamps insert just after
        // the binary-search midpoint, not an invented last-event tie break.
        let (mut start, mut end) = (0_i32, self.0.len() as i32 - 1);
        while start <= end {
            let mid = (start + end) >> 1;
            let time = self.0[mid as usize].1;
            if time > curtime { start = mid + 1; }
            else if curtime > time { end = mid - 1; }
            else { end = mid; break; }
        }
        self.0.insert((end + 1) as usize, (attacker, curtime));
        self.0.truncate(4);
    }

    pub fn assister(&self, scorer: u32, curtime: f32) -> Option<u32> {
        self.0.get(1).filter(|entry| entry.0 != scorer && curtime - entry.1 <= 3.0).map(|entry| entry.0)
    }
}

pub fn weapon_name(weapon: Weapon, class: PlayerClass) -> &'static str {
    match weapon {
        Weapon::RocketLauncher => "tf_projectile_rocket",
        Weapon::Original => "quake_rl",
        Weapon::StickybombLauncher => "tf_projectile_pipe_remote",
        Weapon::GrenadeLauncher => "tf_projectile_pipe",
        Weapon::Scattergun => "scattergun",
        Weapon::Pistol => "pistol_scout",
        Weapon::EngineerPistol => "pistol",
        Weapon::Bat => "bat",
        Weapon::Shotgun if class == PlayerClass::Pyro => "shotgun_pyro",
        Weapon::Shotgun => "shotgun_soldier",
        Weapon::HeavyShotgun => "shotgun_hwg",
        Weapon::EngineerShotgun => "shotgun_primary",
        Weapon::Shovel => "shovel",
        Weapon::Minigun => "minigun",
        Weapon::Fists => "fists",
        Weapon::SniperRifle => "sniperrifle",
        Weapon::Smg => "smg",
        Weapon::Kukri => "club",
        Weapon::Bottle => "bottle",
        Weapon::Wrench => "wrench",
        Weapon::Flamethrower => "flamethrower",
        Weapon::FireAxe => "fireaxe",
        Weapon::Revolver => "revolver",
        Weapon::Knife => "knife",
        Weapon::Sapper => "obj_attachment_sapper",
        Weapon::SyringeGun => "syringegun_medic",
        Weapon::MediGun => "medigun",
        Weapon::Bonesaw => "bonesaw",
        Weapon::DisguiseKit | Weapon::InvisibilityWatch | Weapon::BuildPda | Weapon::DestroyPda | Weapon::Toolbox => "world",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_distinct_recent_damager_and_inclusive_three_second_boundary() {
        let mut history = DamagerHistory::default();
        history.record(2, 1.0);
        history.record(3, 2.0);
        history.record(3, 3.0);
        assert_eq!(history.assister(3, 4.0), Some(2));
        assert_eq!(history.assister(3, 4.000_001), None);
        history.record(2, 4.0);
        assert_eq!(history.assister(2, 4.0), Some(3));
        history.record(0, 5.0);
        assert_eq!(history.assister(2, 4.0), Some(3));
    }

    #[test]
    fn equal_times_use_source_sorted_vector_midpoint_and_only_four_entries() {
        let mut history = DamagerHistory::default();
        for identity in 1..=6 { history.record(identity, 1.0); }
        assert_eq!(history.0.iter().map(|entry| entry.0).collect::<Vec<_>>(), [1, 3, 6, 5]);
        assert_eq!(history.assister(6, 1.0), Some(3));
        assert_eq!(history.assister(3, 1.0), None);
    }

    #[test]
    fn killing_names_are_source_inflictors_not_display_names_or_active_slots() {
        assert_eq!(weapon_name(Weapon::RocketLauncher, PlayerClass::Soldier), "tf_projectile_rocket");
        assert_eq!(weapon_name(Weapon::Shotgun, PlayerClass::Pyro), "shotgun_pyro");
        assert_eq!(weapon_name(Weapon::Shotgun, PlayerClass::Soldier), "shotgun_soldier");
        assert_eq!(weapon_name(Weapon::Kukri, PlayerClass::Sniper), "club");
    }
}
