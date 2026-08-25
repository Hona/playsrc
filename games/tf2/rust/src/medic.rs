use crate::{
    PlayerTeam,
    condition::{ConditionDuration, ConditionId, ConditionState},
    health::{Healer, HealthConfiguration, HealthState},
};

pub const SYRINGE_DAMAGE: i32 = 10;
pub const SYRINGE_SPEED: f32 = 1_000.0;
pub const SYRINGE_GRAVITY_SCALE: f32 = 0.3;
pub const SYRINGE_CLIP: u16 = 40;
pub const SYRINGE_RESERVE: u16 = 150;
pub const SYRINGE_FIRE_DELAY: f32 = 0.1;
pub const SYRINGE_RELOAD_SECONDS: f32 = 1.3;
pub const MEDIGUN_HEAL_RATE: f32 = 24.0;
pub const MEDIGUN_TARGET_RANGE: f32 = 450.0;
pub const MEDIGUN_STICK_RANGE: f32 = MEDIGUN_TARGET_RANGE * 1.2;
pub const MEDIGUN_TARGET_CHECK_INTERVAL: f32 = 1.0;
pub const MEDIGUN_FULL_CHARGE_SECONDS: f32 = 40.0;
pub const MEDIGUN_CHARGE_RELEASE_SECONDS: f32 = 8.0;
pub const MEDIGUN_DETACHED_INVULNERABILITY_SECONDS: f32 = 1.0;
pub const BONESAW_DAMAGE: i32 = 65;
pub const BONESAW_RANGE: f32 = 48.0;
pub const BONESAW_HULL_EXTENT: f32 = 18.0;
pub const BONESAW_FIRE_DELAY: f32 = 0.8;
pub const BONESAW_SMACK_DELAY: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatientFacts {
    pub identity: u32,
    pub team: PlayerTeam,
    pub alive: bool,
    pub stealthed: bool,
    pub disguised_as: Option<PlayerTeam>,
    pub blocks_healing: bool,
    pub nearest_point: [f32; 3],
    pub center: [f32; 3],
    pub eyes: [f32; 3],
    pub current_health: i32,
    pub maximum_health: i32,
    pub maximum_buffed_health: i32,
    pub healer_count: usize,
}

impl PatientFacts {
    pub fn allowed(self, medic_team: PlayerTeam, medic_stealthed: bool) -> bool {
        self.alive
            && !self.blocks_healing
            && (!self.stealthed || medic_stealthed)
            && (self.team == medic_team || self.disguised_as == Some(medic_team))
    }

    pub fn charge_modifier(self, setup: bool) -> f32 {
        let threshold = (self.maximum_buffed_health as f32 * 0.95).floor() as i32;
        let fullness = if !setup && self.current_health >= threshold {
            0.5
        } else {
            1.0
        };
        fullness / self.healer_count.max(1) as f32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeamTransition {
    Attached(u32),
    Detached(u32),
    ChargeReady,
    ChargeStarted,
    ChargeEnded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DetachedPatient {
    identity: u32,
    detached_at: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MedigunState {
    pub target: Option<u32>,
    pub charge: f32,
    pub releasing: bool,
    next_target_check: f32,
    detached: Vec<DetachedPatient>,
}

impl Default for MedigunState {
    fn default() -> Self {
        Self {
            target: None,
            charge: 0.0,
            releasing: false,
            next_target_check: 0.0,
            detached: Vec::new(),
        }
    }
}

impl MedigunState {
    pub fn stock_healer(identity: u32) -> Healer {
        Healer {
            identity,
            scorer: identity,
            rate: MEDIGUN_HEAL_RATE,
            overheal_multiplier: 1.5,
            overheal_decay_multiplier: 1.0,
            dispenser: false,
            accumulated: 0.0,
            healed_last_second: 0.0,
            overheal_fill_rate_multiplier: 1.0,
            healing_from_medics_multiplier: 1.0,
        }
    }

    pub fn attach(
        &mut self,
        now: f32,
        medic_identity: u32,
        medic_team: PlayerTeam,
        medic_stealthed: bool,
        origin: [f32; 3],
        patient: PatientFacts,
        health: &mut HealthState,
        conditions: &mut ConditionState,
    ) -> Result<Option<BeamTransition>, crate::health::HealthError> {
        if self.target == Some(patient.identity)
            || !patient.allowed(medic_team, medic_stealthed)
            || distance(origin, patient.nearest_point) > MEDIGUN_TARGET_RANGE
        {
            return Ok(None);
        }
        health.start_healing(Self::stock_healer(medic_identity), conditions)?;
        self.target = Some(patient.identity);
        self.next_target_check = now + MEDIGUN_TARGET_CHECK_INTERVAL;
        if self.releasing {
            let _ = conditions
                .add(
                    ConditionId::INVULNERABLE,
                    ConditionDuration::Permanent,
                    Some(medic_identity),
                    true,
                    false,
                )
                .map_err(crate::health::HealthError::Condition)?;
        }
        Ok(Some(BeamTransition::Attached(patient.identity)))
    }

    pub fn maintain(
        &mut self,
        now: f32,
        origin: [f32; 3],
        patient: PatientFacts,
        aimed_at_patient: bool,
        center_visible: bool,
        eyes_visible: bool,
    ) -> bool {
        if self.target != Some(patient.identity) || !patient.alive {
            return false;
        }
        if distance(origin, patient.nearest_point) >= MEDIGUN_STICK_RANGE {
            return false;
        }
        if self.next_target_check > now {
            return true;
        }
        self.next_target_check = now + MEDIGUN_TARGET_CHECK_INTERVAL;
        aimed_at_patient || center_visible || eyes_visible
    }

    pub fn detach(
        &mut self,
        now: f32,
        medic_identity: u32,
        health: &mut HealthState,
        conditions: &mut ConditionState,
    ) -> Option<BeamTransition> {
        let patient = self.target.take()?;
        health.stop_healing(medic_identity, conditions);
        if self.releasing {
            self.detached.push(DetachedPatient {
                identity: patient,
                detached_at: now,
            });
        } else {
            conditions.remove(ConditionId::INVULNERABLE, false);
        }
        Some(BeamTransition::Detached(patient))
    }

    pub fn build_charge(
        &mut self,
        tick_interval: f32,
        patient: PatientFacts,
        setup: bool,
    ) -> Option<BeamTransition> {
        if self.releasing || self.target != Some(patient.identity) {
            return None;
        }
        let before = self.charge;
        let setup_multiplier = if setup { 3.0 } else { 1.0 };
        self.charge = (self.charge
            + tick_interval / MEDIGUN_FULL_CHARGE_SECONDS
                * patient.charge_modifier(setup)
                * setup_multiplier)
            .min(1.0);
        (before < 1.0 && self.charge >= 1.0).then_some(BeamTransition::ChargeReady)
    }

    pub fn activate(
        &mut self,
        medic_identity: u32,
        medic_conditions: &mut ConditionState,
        patient_conditions: Option<&mut ConditionState>,
    ) -> Result<Option<BeamTransition>, crate::condition::ConditionError> {
        if self.releasing || self.charge < 1.0 {
            return Ok(None);
        }
        self.releasing = true;
        medic_conditions.add(
            ConditionId::INVULNERABLE,
            ConditionDuration::Permanent,
            Some(medic_identity),
            true,
            false,
        )?;
        if let Some(conditions) = patient_conditions {
            conditions.add(
                ConditionId::INVULNERABLE,
                ConditionDuration::Permanent,
                Some(medic_identity),
                true,
                false,
            )?;
        }
        Ok(Some(BeamTransition::ChargeStarted))
    }

    pub fn drain(
        &mut self,
        now: f32,
        tick_interval: f32,
        alive: impl Fn(u32) -> bool,
    ) -> Option<BeamTransition> {
        if !self.releasing {
            return None;
        }
        self.detached.retain(|patient| {
            Some(patient.identity) != self.target
                && alive(patient.identity)
                && patient.detached_at >= now - MEDIGUN_DETACHED_INVULNERABILITY_SECONDS
        });
        let base = tick_interval / MEDIGUN_CHARGE_RELEASE_SECONDS;
        self.charge = (self.charge - base * (1.0 + self.detached.len() as f32 * 0.5)).max(0.0);
        if self.charge == 0.0 {
            self.releasing = false;
            self.detached.clear();
            return Some(BeamTransition::ChargeEnded);
        }
        None
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn stock_maximum_buffed_health(
    health: &HealthState,
) -> Result<i32, crate::health::HealthError> {
    health.max_buffed_health(HealthConfiguration::default(), false, false)
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerClass;

    fn patient(identity: u32, health: i32) -> PatientFacts {
        PatientFacts {
            identity,
            team: PlayerTeam::Red,
            alive: true,
            stealthed: false,
            disguised_as: None,
            blocks_healing: false,
            nearest_point: [100.0, 0.0, 0.0],
            center: [100.0, 0.0, 41.0],
            eyes: [100.0, 0.0, 68.0],
            current_health: health,
            maximum_health: 200,
            maximum_buffed_health: 300,
            healer_count: 1,
        }
    }

    #[test]
    fn stock_content_values_match_weapon_scripts_and_projectile_class() {
        assert_eq!(
            (SYRINGE_DAMAGE, SYRINGE_CLIP, SYRINGE_RESERVE),
            (10, 40, 150)
        );
        assert_eq!(
            (SYRINGE_SPEED, SYRINGE_GRAVITY_SCALE, SYRINGE_FIRE_DELAY),
            (1000.0, 0.3, 0.1)
        );
        assert_eq!(
            (MEDIGUN_HEAL_RATE, MEDIGUN_TARGET_RANGE, MEDIGUN_STICK_RANGE),
            (24.0, 450.0, 540.0)
        );
        assert_eq!(
            (BONESAW_DAMAGE, BONESAW_RANGE, BONESAW_SMACK_DELAY),
            (65, 48.0, 0.2)
        );
    }

    #[test]
    fn healing_eligibility_accepts_teammates_and_disguised_enemies_only() {
        assert!(patient(2, 100).allowed(PlayerTeam::Red, false));
        assert!(!patient(2, 100).allowed(PlayerTeam::Blue, false));
        assert!(
            PatientFacts {
                disguised_as: Some(PlayerTeam::Blue),
                ..patient(2, 100)
            }
            .allowed(PlayerTeam::Blue, false)
        );
        assert!(
            !PatientFacts {
                stealthed: true,
                ..patient(2, 100)
            }
            .allowed(PlayerTeam::Red, false)
        );
        assert!(
            PatientFacts {
                stealthed: true,
                ..patient(2, 100)
            }
            .allowed(PlayerTeam::Red, true)
        );
        assert!(
            !PatientFacts {
                blocks_healing: true,
                ..patient(2, 100)
            }
            .allowed(PlayerTeam::Red, false)
        );
        assert!(
            !PatientFacts {
                alive: false,
                ..patient(2, 100)
            }
            .allowed(PlayerTeam::Red, false)
        );
    }

    #[test]
    fn acquisition_and_retention_preserve_ranges_and_one_second_visibility_cadence() {
        let mut gun = MedigunState::default();
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut conditions = ConditionState::default();
        let target = patient(2, 100);
        assert_eq!(
            gun.attach(
                3.0,
                1,
                PlayerTeam::Red,
                false,
                [0.0; 3],
                target,
                &mut health,
                &mut conditions
            )
            .unwrap(),
            Some(BeamTransition::Attached(2))
        );
        assert!(conditions.contains(ConditionId::HEALTH_BUFF));
        assert!(gun.maintain(3.99, [0.0; 3], target, false, false, false));
        assert!(!gun.maintain(4.0, [0.0; 3], target, false, false, false));
        assert!(gun.maintain(5.0, [0.0; 3], target, false, false, true));
        assert!(!gun.maintain(
            5.1,
            [0.0; 3],
            PatientFacts {
                nearest_point: [540.0, 0.0, 0.0],
                ..target
            },
            true,
            true,
            true
        ));
        assert_eq!(
            gun.detach(5.1, 1, &mut health, &mut conditions),
            Some(BeamTransition::Detached(2))
        );
        assert!(!conditions.contains(ConditionId::HEALTH_BUFF));
    }

    #[test]
    fn charge_uses_forty_seconds_full_patient_penalty_and_multiple_healers() {
        let mut gun = MedigunState {
            target: Some(2),
            ..MedigunState::default()
        };
        gun.build_charge(1.0, patient(2, 284), false);
        assert_eq!(gun.charge, 0.025);
        gun.build_charge(1.0, patient(2, 285), false);
        assert_eq!(gun.charge, 0.0375);
        gun.build_charge(
            1.0,
            PatientFacts {
                healer_count: 2,
                ..patient(2, 100)
            },
            false,
        );
        assert_eq!(gun.charge, 0.05);
        gun.build_charge(1.0, patient(2, 300), true);
        assert_eq!(gun.charge, 0.125);
        gun.charge = 0.99;
        assert_eq!(
            gun.build_charge(1.0, patient(2, 100), false),
            Some(BeamTransition::ChargeReady)
        );
        assert_eq!(gun.charge, 1.0);
    }

    #[test]
    fn full_charge_applies_invulnerability_and_drains_over_eight_seconds() {
        let mut gun = MedigunState {
            charge: 0.99,
            target: Some(2),
            ..MedigunState::default()
        };
        let mut medic = ConditionState::default();
        let mut target = ConditionState::default();
        assert_eq!(
            gun.activate(1, &mut medic, Some(&mut target)).unwrap(),
            None
        );
        gun.charge = 1.0;
        assert_eq!(
            gun.activate(1, &mut medic, Some(&mut target)).unwrap(),
            Some(BeamTransition::ChargeStarted)
        );
        assert!(medic.contains(ConditionId::INVULNERABLE));
        assert!(target.contains(ConditionId::INVULNERABLE));
        for second in 0..7 {
            assert_eq!(gun.drain(second as f32, 1.0, |_| true), None);
        }
        assert_eq!(gun.charge, 0.125);
        assert_eq!(
            gun.drain(7.0, 1.0, |_| true),
            Some(BeamTransition::ChargeEnded)
        );
        assert_eq!((gun.charge, gun.releasing), (0.0, false));
    }

    #[test]
    fn detached_patients_increase_charge_drain_by_half_for_one_second() {
        let mut gun = MedigunState {
            charge: 1.0,
            target: Some(2),
            releasing: true,
            next_target_check: 0.0,
            detached: vec![],
        };
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut conditions = ConditionState::default();
        health
            .start_healing(MedigunState::stock_healer(1), &mut conditions)
            .unwrap();
        gun.detach(3.0, 1, &mut health, &mut conditions);
        gun.target = Some(3);
        gun.drain(3.5, 1.0, |_| true);
        assert_eq!(gun.charge, 0.8125);
        gun.drain(4.01, 1.0, |_| true);
        assert_eq!(gun.charge, 0.6875);
    }
}
