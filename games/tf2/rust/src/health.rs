use crate::{
    class::PlayerClass,
    condition::{ConditionDuration, ConditionEvent, ConditionId, ConditionState},
};

pub const DEFAULT_MAX_HEALTH_BOOST: f32 = 1.5;
pub const DEFAULT_BOOST_DRAIN_SECONDS: f32 = 15.0;
pub const HEALING_DEBUFF_FACTOR: f32 = 0.8;
pub const MAX_HEALERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealthConfiguration {
    pub maximum_boost: f32,
    pub boost_drain_seconds: f32,
}

impl Default for HealthConfiguration {
    fn default() -> Self {
        Self {
            maximum_boost: DEFAULT_MAX_HEALTH_BOOST,
            boost_drain_seconds: DEFAULT_BOOST_DRAIN_SECONDS,
        }
    }
}

impl HealthConfiguration {
    pub fn validate(self) -> bool {
        self.maximum_boost.is_finite()
            && self.maximum_boost >= 1.0
            && self.boost_drain_seconds.is_finite()
            && self.boost_drain_seconds >= 0.1
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Healer {
    pub identity: u32,
    pub scorer: u32,
    pub rate: f32,
    pub overheal_multiplier: f32,
    pub overheal_decay_multiplier: f32,
    pub dispenser: bool,
    pub accumulated: f32,
    pub healed_last_second: f32,
    pub overheal_fill_rate_multiplier: f32,
    pub healing_from_medics_multiplier: f32,
}

impl Healer {
    pub fn validate(&self) -> bool {
        self.rate.is_finite()
            && self.rate >= 0.0
            && self.overheal_multiplier.is_finite()
            && self.overheal_multiplier >= 1.0
            && self.overheal_decay_multiplier.is_finite()
            && self.overheal_decay_multiplier > 0.0
            && self.accumulated.is_finite()
            && self.accumulated >= 0.0
            && self.healed_last_second.is_finite()
            && self.healed_last_second >= 0.0
            && self.overheal_fill_rate_multiplier.is_finite()
            && self.overheal_fill_rate_multiplier >= 0.0
            && self.healing_from_medics_multiplier.is_finite()
            && self.healing_from_medics_multiplier >= 0.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HealthState {
    pub current: i32,
    pub maximum_for_buffing: i32,
    pub maximum: i32,
    pub healing_fraction: f32,
    pub best_overheal_decay_multiplier: Option<f32>,
    pub healers: Vec<Healer>,
    pub last_damage_time: f32,
}

impl HealthState {
    pub fn spawn(
        class: PlayerClass,
        add_max_health: f32,
        add_nonbuffed_health: f32,
    ) -> Result<Self, HealthError> {
        if !add_max_health.is_finite() || !add_nonbuffed_health.is_finite() {
            return Err(HealthError::NonFiniteInput);
        }
        let maximum_for_buffing = (class.data().maximum_health as f32 + add_max_health) as i32;
        let maximum = ((maximum_for_buffing as f32 + add_nonbuffed_health) as i32).max(1);
        Ok(Self {
            current: maximum,
            maximum_for_buffing,
            maximum,
            healing_fraction: 0.0,
            best_overheal_decay_multiplier: None,
            healers: Vec::new(),
            last_damage_time: f32::NEG_INFINITY,
        })
    }

    pub fn reset_for_spawn(
        &mut self,
        class: PlayerClass,
        add_max_health: f32,
        add_nonbuffed_health: f32,
    ) -> Result<(), HealthError> {
        *self = Self::spawn(class, add_max_health, add_nonbuffed_health)?;
        Ok(())
    }

    pub fn max_buffed_health(
        &self,
        configuration: HealthConfiguration,
        ignore_healer_attributes: bool,
        ignore_current_floor: bool,
    ) -> Result<i32, HealthError> {
        if !configuration.validate() {
            return Err(HealthError::InvalidConfiguration);
        }
        let mut boost = self.maximum_for_buffing as f32 * configuration.maximum_boost;
        if !ignore_healer_attributes
            && let Some(maximum) = self
                .healers
                .iter()
                .map(|healer| healer.overheal_multiplier)
                .reduce(f32::max)
        {
            boost = boost.max(self.maximum_for_buffing as f32 * maximum);
        }
        let mut rounded = (boost / 5.0).floor() as i32 * 5;
        if !ignore_current_floor {
            rounded = rounded.max(self.maximum).max(self.current);
        }
        Ok(rounded)
    }

    pub fn start_healing(
        &mut self,
        mut healer: Healer,
        conditions: &mut ConditionState,
    ) -> Result<Vec<ConditionEvent>, HealthError> {
        if !healer.validate() {
            return Err(HealthError::InvalidHealer);
        }
        let mut events = Vec::new();
        if let Some(index) = self
            .healers
            .iter()
            .position(|current| current.identity == healer.identity)
        {
            let previous = self.healers.remove(index);
            healer.accumulated = previous.accumulated;
        }
        if self.healers.len() >= MAX_HEALERS {
            return Err(HealthError::HealerLimit);
        }
        let was_empty = self.healers.is_empty();
        self.healers.push(healer);
        if was_empty {
            self.healing_fraction = 0.0;
            if let Some(event) = conditions
                .add(
                    ConditionId::HEALTH_BUFF,
                    ConditionDuration::Permanent,
                    self.healers.last().map(|value| value.identity),
                    self.current > 0,
                    false,
                )
                .map_err(HealthError::Condition)?
            {
                events.push(event);
            }
        }
        Ok(events)
    }

    pub fn stop_healing(
        &mut self,
        healer: u32,
        conditions: &mut ConditionState,
    ) -> (f32, Vec<ConditionEvent>) {
        let Some(index) = self
            .healers
            .iter()
            .position(|current| current.identity == healer)
        else {
            return (0.0, Vec::new());
        };
        let accumulated = self.healers.remove(index).accumulated;
        let mut events = Vec::new();
        if self.healers.is_empty() {
            self.healing_fraction = 0.0;
            if let Some(event) = conditions.remove(ConditionId::HEALTH_BUFF, false) {
                events.push(event);
            }
        }
        (accumulated, events)
    }

    pub fn take_health(
        &mut self,
        amount: f32,
        ignore_maximum: bool,
        healing_received_multiplier: f32,
        conditions: &ConditionState,
    ) -> Result<i32, HealthError> {
        if !amount.is_finite()
            || amount < 0.0
            || !healing_received_multiplier.is_finite()
            || healing_received_multiplier < 0.0
        {
            return Err(HealthError::NonFiniteInput);
        }
        if conditions.contains(ConditionId::NO_HEALING_DAMAGE_BUFF) {
            return Ok(0);
        }
        let scaled = amount * healing_received_multiplier;
        if ignore_maximum {
            let before = self.current;
            self.current = (self.current as f32 + scaled) as i32;
            return Ok(self.current - before);
        }
        let allowed = scaled.min((self.maximum - self.current) as f32);
        if allowed <= 0.0 {
            return Ok(0);
        }
        let before = self.current;
        self.current = (self.current as f32 + allowed) as i32;
        Ok(self.current - before)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        now: f32,
        tick_interval: f32,
        configuration: HealthConfiguration,
        health_from_healers_multiplier: f32,
        active_health_from_healers_multiplier: f32,
        healing_received_multiplier: f32,
        conditions: &mut ConditionState,
    ) -> Result<HealthTick, HealthError> {
        if !now.is_finite()
            || !tick_interval.is_finite()
            || tick_interval <= 0.0
            || !configuration.validate()
            || !health_from_healers_multiplier.is_finite()
            || !active_health_from_healers_multiplier.is_finite()
            || !healing_received_multiplier.is_finite()
        {
            return Err(HealthError::NonFiniteInput);
        }
        let before = self.current;
        let mut decay_health = true;
        let mut total_rate = 0.0;
        let mut scale = remap_clamped(now - self.last_damage_time, 10.0, 15.0, 1.0, 3.0);
        let common_multiplier =
            health_from_healers_multiplier * active_health_from_healers_multiplier;
        let maximum_for_each = self.max_buffed_health(configuration, false, false)?;
        let buffable = self.maximum_for_buffing.max(1) as f32;
        let current_overheal = if self.current > self.maximum {
            let buffable_range =
                self.current as f32 - (self.maximum - self.maximum_for_buffing) as f32;
            buffable_range / buffable
        } else {
            self.current as f32 / self.maximum.max(1) as f32
        };
        for healer in &mut self.healers {
            if current_overheal >= healer.overheal_multiplier {
                continue;
            }
            decay_health = false;
            if conditions.contains(ConditionId::new(28).expect("megaheal identity")) {
                scale *= 3.0;
            }
            let fraction = if healer.dispenser {
                tick_interval * healer.rate * common_multiplier
            } else {
                scale *= healer.healing_from_medics_multiplier;
                let overheal_fill = if current_overheal > 1.0 {
                    healer.overheal_fill_rate_multiplier
                } else {
                    1.0
                };
                tick_interval * healer.rate * scale * common_multiplier * overheal_fill
            };
            self.healing_fraction += fraction;
            healer.accumulated += fraction.clamp(0.0, (maximum_for_each - before).max(0) as f32);
            total_rate += healer.rate;
            self.best_overheal_decay_multiplier = Some(
                self.best_overheal_decay_multiplier
                    .map_or(healer.overheal_decay_multiplier, |current| {
                        current.min(healer.overheal_decay_multiplier)
                    }),
            );
        }
        if conditions.contains(ConditionId::HEALING_DEBUFF) {
            self.healing_fraction *= HEALING_DEBUFF_FACTOR;
        }
        let whole = self.healing_fraction as i32;
        let mut healed = 0;
        if whole > 0 {
            self.healing_fraction -= whole as f32;
            let maximum = self.max_buffed_health(configuration, false, false)?;
            let allowed = whole.clamp(0, maximum - self.current);
            healed = self.take_health(
                allowed as f32,
                true,
                healing_received_multiplier,
                conditions,
            )?;
            if total_rate > 0.0 && healed > 0 {
                for healer in &mut self.healers {
                    healer.healed_last_second += healed as f32 * (healer.rate / total_rate);
                }
            }
        }

        let mut drained = 0;
        if decay_health && self.current > self.maximum {
            let decay_target = self.max_buffed_health(configuration, false, true)?;
            let multiplier = self.best_overheal_decay_multiplier.unwrap_or(1.0);
            let drain_per_second = (decay_target - self.maximum) as f32
                / (configuration.boost_drain_seconds * multiplier);
            self.healing_fraction += tick_interval * drain_per_second;
            let whole = self.healing_fraction as i32;
            if whole > 0 {
                self.healing_fraction -= whole as f32;
                self.current -= whole;
                drained = whole;
            }
        } else if self.current <= self.maximum && self.best_overheal_decay_multiplier.is_some() {
            self.best_overheal_decay_multiplier = None;
        }

        let mut condition_events = Vec::new();
        if self.current > self.maximum && !conditions.contains(ConditionId::HEALTH_OVERHEALED) {
            if let Some(event) = conditions
                .add(
                    ConditionId::HEALTH_OVERHEALED,
                    ConditionDuration::Permanent,
                    None,
                    self.current > 0,
                    false,
                )
                .map_err(HealthError::Condition)?
            {
                condition_events.push(event);
            }
        } else if self.current <= self.maximum
            && conditions.contains(ConditionId::HEALTH_OVERHEALED)
            && let Some(event) = conditions.remove(ConditionId::HEALTH_OVERHEALED, false)
        {
            condition_events.push(event);
        }

        Ok(HealthTick {
            before,
            after: self.current,
            healed,
            drained,
            condition_events,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HealthTick {
    pub before: i32,
    pub after: i32,
    pub healed: i32,
    pub drained: i32,
    pub condition_events: Vec<ConditionEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HealthError {
    NonFiniteInput,
    InvalidConfiguration,
    InvalidHealer,
    HealerLimit,
    Condition(crate::condition::ConditionError),
}

fn remap_clamped(
    value: f32,
    input_min: f32,
    input_max: f32,
    output_min: f32,
    output_max: f32,
) -> f32 {
    let fraction = ((value - input_min) / (input_max - input_min)).clamp(0.0, 1.0);
    output_min + (output_max - output_min) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healer(identity: u32, rate: f32, dispenser: bool) -> Healer {
        Healer {
            identity,
            scorer: identity,
            rate,
            overheal_multiplier: 1.5,
            overheal_decay_multiplier: 1.0,
            dispenser,
            accumulated: 0.0,
            healed_last_second: 0.0,
            overheal_fill_rate_multiplier: 1.0,
            healing_from_medics_multiplier: 1.0,
        }
    }

    #[test]
    fn class_and_item_maximums_and_five_point_overheal_rounding_are_exact() {
        for class in PlayerClass::ALL {
            let health = HealthState::spawn(class, 0.0, 0.0).unwrap();
            assert_eq!(health.maximum, class.data().maximum_health);
        }
        let mut health = HealthState::spawn(PlayerClass::Scout, 1.0, 7.0).unwrap();
        assert_eq!((health.maximum_for_buffing, health.maximum), (126, 133));
        assert_eq!(
            health
                .max_buffed_health(HealthConfiguration::default(), false, false)
                .unwrap(),
            185
        );
        health.current = 200;
        assert_eq!(
            health
                .max_buffed_health(HealthConfiguration::default(), false, false)
                .unwrap(),
            200
        );
    }

    #[test]
    fn repeated_healer_moves_to_tail_and_preserves_accumulation() {
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        health.current = 100;
        let mut conditions = ConditionState::default();
        health
            .start_healing(healer(1, 24.0, false), &mut conditions)
            .unwrap();
        health
            .start_healing(healer(2, 10.0, true), &mut conditions)
            .unwrap();
        health.healers[0].accumulated = 12.5;
        health
            .start_healing(healer(1, 30.0, false), &mut conditions)
            .unwrap();
        assert_eq!(
            health
                .healers
                .iter()
                .map(|value| value.identity)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_eq!(health.healers[1].accumulated, 12.5);
        let (accumulated, _) = health.stop_healing(1, &mut conditions);
        assert_eq!(accumulated, 12.5);
        assert!(conditions.contains(ConditionId::HEALTH_BUFF));
        health.stop_healing(2, &mut conditions);
        assert!(!conditions.contains(ConditionId::HEALTH_BUFF));
    }

    #[test]
    fn fractional_healing_ramp_dispenser_and_debuff_branches_match_order() {
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        health.current = 100;
        health.last_damage_time = 0.0;
        let mut conditions = ConditionState::default();
        health
            .start_healing(healer(1, 24.0, false), &mut conditions)
            .unwrap();
        let tick = health
            .advance(
                10.0,
                0.5,
                HealthConfiguration::default(),
                1.0,
                1.0,
                1.0,
                &mut conditions,
            )
            .unwrap();
        assert_eq!(tick.healed, 12);
        health.stop_healing(1, &mut conditions);
        health
            .start_healing(healer(2, 10.0, true), &mut conditions)
            .unwrap();
        let tick = health
            .advance(
                15.0,
                0.5,
                HealthConfiguration::default(),
                1.0,
                1.0,
                1.0,
                &mut conditions,
            )
            .unwrap();
        assert_eq!(tick.healed, 5);
        conditions
            .add(
                ConditionId::HEALING_DEBUFF,
                ConditionDuration::Permanent,
                None,
                true,
                false,
            )
            .unwrap();
        let tick = health
            .advance(
                15.0,
                0.5,
                HealthConfiguration::default(),
                1.0,
                1.0,
                1.0,
                &mut conditions,
            )
            .unwrap();
        assert_eq!(tick.healed, 4);
    }

    #[test]
    fn overheal_decays_over_fifteen_seconds_and_drives_condition_edges() {
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        health.current = 300;
        let mut conditions = ConditionState::default();
        let first = health
            .advance(
                0.0,
                1.0,
                HealthConfiguration::default(),
                1.0,
                1.0,
                1.0,
                &mut conditions,
            )
            .unwrap();
        assert_eq!(first.drained, 6);
        assert!(conditions.contains(ConditionId::HEALTH_OVERHEALED));
        for second in 1..=15 {
            health
                .advance(
                    second as f32,
                    1.0,
                    HealthConfiguration::default(),
                    1.0,
                    1.0,
                    1.0,
                    &mut conditions,
                )
                .unwrap();
        }
        assert!(health.current <= 200);
        assert!(!conditions.contains(ConditionId::HEALTH_OVERHEALED));
    }

    #[test]
    fn no_healing_condition_and_ignore_maximum_conversion_are_exact() {
        let mut health = HealthState::spawn(PlayerClass::Scout, 0.0, 0.0).unwrap();
        health.current = 100;
        let mut conditions = ConditionState::default();
        conditions
            .add(
                ConditionId::NO_HEALING_DAMAGE_BUFF,
                ConditionDuration::Permanent,
                None,
                true,
                false,
            )
            .unwrap();
        assert_eq!(
            health.take_health(10.0, false, 1.0, &conditions).unwrap(),
            0
        );
        conditions.remove(ConditionId::NO_HEALING_DAMAGE_BUFF, true);
        assert_eq!(
            health.take_health(10.8, true, 1.0, &conditions).unwrap(),
            10
        );
        assert_eq!(health.current, 110);
    }

    #[test]
    fn healer_bound_rejects_the_sixty_fifth_source_atomically() {
        let mut health = HealthState::spawn(PlayerClass::Scout, 0.0, 0.0).unwrap();
        let mut conditions = ConditionState::default();
        for identity in 0..MAX_HEALERS as u32 {
            health
                .start_healing(healer(identity, 1.0, false), &mut conditions)
                .unwrap();
        }
        assert_eq!(health.healers.len(), MAX_HEALERS);
        assert_eq!(
            health.start_healing(healer(100, 1.0, false), &mut conditions),
            Err(HealthError::HealerLimit)
        );
        assert_eq!(health.healers.len(), MAX_HEALERS);
    }
}
