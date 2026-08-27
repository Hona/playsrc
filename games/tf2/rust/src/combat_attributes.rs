//! Live equipment hooks at the shared damage transaction boundary.

use crate::{GameplayWorld, Session, Weapon, PLAYER_IDENTITY, PlayerClass, Condition};
use crate::damage::DamageModifiers;

impl<W: GameplayWorld + Clone> Session<W> {
    pub(crate) fn equipped_damage_modifiers(&mut self, source: Option<crate::weapon::WeaponSource>, victim: u32, weapon: Weapon, mut modifiers: DamageModifiers) -> DamageModifiers {
        modifiers.outgoing_vs_players *= self.source_weapon_attribute(source, weapon, "mult_dmg_vs_players", 1.0);
        modifiers.minicrits_become_crits |= self.source_weapon_attribute(source, weapon, "minicrits_become_crits", 0.0) != 0.0;
        modifiers.crits_become_minicrits |= self.source_weapon_attribute(source, weapon, "crits_become_minicrits", 0.0) != 0.0;
        modifiers.pierces_resists |= self.source_weapon_attribute(source, weapon, "mod_pierce_resists_absorbs", 0.0) != 0.0;
        self.equipped_victim_damage_modifiers(victim, modifiers)
    }

    pub(crate) fn equipped_victim_damage_modifiers(&mut self, victim: u32, mut modifiers: DamageModifiers) -> DamageModifiers {
        modifiers.critical_bonus_taken *= self.equipped_player_attribute(victim, "mult_dmgtaken_from_crit", 1.0);
        modifiers.fire_taken *= self.equipped_player_attribute(victim, "mult_dmgtaken_from_fire", 1.0);
        modifiers.blast_taken *= self.equipped_player_attribute(victim, "mult_dmgtaken_from_explosions", 1.0);
        modifiers.bullet_taken *= self.equipped_player_attribute(victim, "mult_dmgtaken_from_bullets", 1.0);
        modifiers.melee_taken *= self.equipped_player_attribute(victim, "mult_dmgtaken_from_melee", 1.0);
        modifiers.general_taken *= self.equipped_player_attribute(victim, "mult_dmgtaken", 1.0);
        let active = if victim == PLAYER_IDENTITY { self.weapon } else { self.bots.as_ref().and_then(|bots| bots.active_weapon(victim)) };
        if let Some(active) = active {
            modifiers.active_taken *= self.equipped_weapon_attribute(victim, active, "mult_dmgtaken_active", 1.0);
            modifiers.fire_taken *= self.equipped_weapon_attribute(victim, active, "mult_dmgtaken_from_fire_active", 1.0);
        }
        let class = if victim == PLAYER_IDENTITY { Some(self.class) } else { self.bots.as_ref().and_then(|bots| bots.class(victim)) };
        let aiming = if victim == PLAYER_IDENTITY { self.conditions.contains(Condition::Aiming) }
            else { self.bots.as_ref().and_then(|bots| bots.conditions(victim)).is_some_and(|conditions| conditions.contains(crate::condition::ConditionId::AIMING)) };
        if class == Some(PlayerClass::Heavy) && aiming {
            modifiers.spunup_taken *= self.equipped_player_attribute(victim, "spunup_damage_resistance", 1.0);
        }
        modifiers
    }

    /// ApplyPushFromDamage scales the final velocity impulse, not the damage-force fact.
    pub fn equipped_damage_push_multiplier(&mut self, victim: u32) -> f32 {
        self.equipped_player_attribute(victim, "damage_force_reduction", 1.0)
    }
}
