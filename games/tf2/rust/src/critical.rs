//! Player-wide damage history and per-weapon critical-shot state.
//! Callers supply pre-distance, schema-resolved shot inputs and damage facts.

use crate::damage::{
    self, CritCheckInput, CritCheckResult, CritKind, CritState, DamageHistory, DamageHistoryInput,
    DamageType, WeaponCritKind,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub command_number: u32,
    pub launcher_identity: u32,
    pub kind: WeaponCritKind,
    pub raw_damage: f32,
    pub projectiles_per_shot: f32,
    pub fire_delay: f32,
    pub can_fire_critical: bool,
    pub guaranteed_critical: bool,
    pub random_crits_enabled: bool,
}

impl Shot {
    pub fn seed(self, owner: u32) -> i32 {
        (crate::random::prediction_seed(self.command_number)
            ^ (self.launcher_identity.wrapping_shl(8) | owner)) as i32
    }

    pub fn input(self, now: f32, player_multiplier: f32, chance_multiplier: f32) -> CritCheckInput {
        CritCheckInput {
            now,
            kind: self.kind,
            random_crits_enabled: self.random_crits_enabled,
            can_fire_critical: self.can_fire_critical,
            guaranteed_critical: self.guaranteed_critical,
            raw_damage: self.raw_damage,
            projectiles_per_shot: self.projectiles_per_shot,
            fire_delay: self.fire_delay,
            player_crit_multiplier: player_multiplier,
            chance_multiplier,
            roll: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponState {
    pub bucket: CritState,
    pub current_seed: i32,
    pub last_tick: Option<u64>,
    pub result: Option<CritCheckResult>,
}

impl Default for WeaponState {
    fn default() -> Self {
        Self {
            bucket: CritState {
                last_rapid_check_time: 0.0,
                ..CritState::default()
            },
            current_seed: -1,
            last_tick: None,
            result: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerHistory {
    history: DamageHistory,
    ranged_damage: u32,
    random_ranged_critical_damage: u32,
    next_update: f32,
    multiplier: f32,
}

impl Default for PlayerHistory {
    fn default() -> Self {
        Self {
            history: DamageHistory::default(),
            ranged_damage: 0,
            random_ranged_critical_damage: 0,
            next_update: 0.0,
            multiplier: 1.0,
        }
    }
}

impl PlayerHistory {
    pub fn advance(&mut self, now: f32) -> Result<(), damage::DamageError> {
        if !now.is_finite() {
            return Err(damage::DamageError::NonFiniteInput);
        }
        if self.next_update < now {
            self.multiplier = self.history.crit_multiplier(now)?;
            self.next_update = now + 0.5;
        }
        Ok(())
    }

    pub fn multiplier(&self) -> f32 {
        self.multiplier
    }

    /// Damage statistics use actual health lost, not unclamped overkill. The
    /// random/boosted split observes the attacker's boost at damage time.
    pub fn record(
        &mut self,
        input: DamageHistoryInput,
        health_damage: u32,
        critical: CritKind,
        attacker_is_crit_boosted: bool,
    ) -> Result<(), damage::DamageError> {
        self.history.record(input)?;
        if health_damage <= 1500 && !input.damage_type.contains(DamageType::MELEE) {
            self.ranged_damage = self.ranged_damage.saturating_add(health_damage);
            if critical == CritKind::Full && !attacker_is_crit_boosted {
                self.random_ranged_critical_damage = self
                    .random_ranged_critical_damage
                    .saturating_add(health_damage);
            }
        }
        Ok(())
    }

    pub fn supply_observed_damage(&self, weapon: &mut CritState) {
        weapon.total_ranged_damage = self.ranged_damage;
        weapon.random_ranged_crit_damage = self.random_ranged_critical_damage;
    }

    pub fn reset_round_statistics(&mut self) {
        self.ranged_damage = 0;
        self.random_ranged_critical_damage = 0;
    }

    pub fn reset_for_spawn(&mut self) {
        self.history = DamageHistory::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_seed_uses_full_prediction_seed_and_source_entity_mask() {
        let shot = Shot {
            command_number: 17,
            launcher_identity: 92,
            kind: WeaponCritKind::SingleShot,
            raw_damage: 90.0,
            projectiles_per_shot: 1.0,
            fire_delay: 0.8,
            can_fire_critical: true,
            guaranteed_critical: false,
            random_crits_enabled: true,
        };
        assert_eq!(
            shot.seed(1),
            (crate::random::prediction_seed(17) ^ ((92 << 8) | 1)) as i32
        );
        assert_ne!(shot.seed(1), shot.seed(2));
        assert_ne!(
            shot.seed(1),
            (crate::random::prediction_seed(17) & 255) as i32
        );
    }

    #[test]
    fn observed_ranged_damage_is_player_wide_and_uses_impact_time_boost() {
        let mut player = PlayerHistory::default();
        let input = DamageHistoryInput {
            now: 1.0,
            damage: 270.0,
            bonus_damage: 180.0,
            victim_previous_health: 125,
            lethal: true,
            damage_type: DamageType::BLAST,
            counts_toward_crit_rate: true,
        };
        player.record(input, 125, CritKind::Full, false).unwrap();
        player
            .record(
                DamageHistoryInput { now: 2.0, ..input },
                125,
                CritKind::Full,
                true,
            )
            .unwrap();
        let mut first = CritState::default();
        let mut second = CritState::default();
        player.supply_observed_damage(&mut first);
        player.supply_observed_damage(&mut second);
        assert_eq!(
            (first.total_ranged_damage, first.random_ranged_crit_damage),
            (250, 125)
        );
        assert_eq!(first.total_ranged_damage, second.total_ranged_damage);
        player.reset_round_statistics();
        player.supply_observed_damage(&mut first);
        assert_eq!(first.total_ranged_damage, 0);
    }

    #[test]
    fn damage_multiplier_updates_on_the_strict_half_second_player_timer() {
        let mut player = PlayerHistory::default();
        player
            .record(
                DamageHistoryInput {
                    now: 0.0,
                    damage: 800.0,
                    bonus_damage: 0.0,
                    victim_previous_health: 1000,
                    lethal: false,
                    damage_type: DamageType::BULLET,
                    counts_toward_crit_rate: true,
                },
                800,
                CritKind::None,
                false,
            )
            .unwrap();
        player.advance(1.9).unwrap();
        assert_eq!(player.multiplier(), 1.0);
        player.advance(2.0).unwrap();
        assert_eq!(player.multiplier(), 1.0);
        player.advance(2.4).unwrap();
        assert_eq!(player.multiplier(), 1.0);
        player.advance(2.401).unwrap();
        assert_eq!(player.multiplier(), 4.0);
        player.reset_for_spawn();
        assert_eq!(player.multiplier(), 4.0);
        player.advance(3.0).unwrap();
        assert_eq!(player.multiplier(), 1.0);
    }

    #[test]
    fn resolved_zero_projectiles_does_not_invent_a_bucket_damage_contribution() {
        let mut weapon = CritState::default();
        let result = weapon
            .check(CritCheckInput {
                now: 1.0,
                kind: WeaponCritKind::SingleShot,
                random_crits_enabled: true,
                can_fire_critical: true,
                guaranteed_critical: false,
                raw_damage: 90.0,
                projectiles_per_shot: 0.0,
                fire_delay: 0.8,
                player_crit_multiplier: 1.0,
                chance_multiplier: 1.0,
                roll: Some(9999),
            })
            .unwrap();
        assert_eq!(result.bucket_before, result.bucket_after);
    }

    #[test]
    fn an_empty_new_round_does_not_apply_the_previous_observed_rate() {
        let mut weapon = CritState {
            observed_chance: 0.9,
            ..CritState::default()
        };
        let result = weapon
            .check(CritCheckInput {
                now: 1.0,
                kind: WeaponCritKind::SingleShot,
                random_crits_enabled: true,
                can_fire_critical: true,
                guaranteed_critical: false,
                raw_damage: 1.0,
                projectiles_per_shot: 1.0,
                fire_delay: 0.8,
                player_crit_multiplier: 1.0,
                chance_multiplier: 1.0,
                roll: Some(0),
            })
            .unwrap();
        assert_eq!(result.kind, CritKind::Full);
        assert!(!result.denied_by_observed_rate);
    }
}
