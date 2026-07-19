pub const CONDITION_COUNT: usize = 131;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionId(u8);

impl ConditionId {
    pub const AIMING: Self = Self(0);
    pub const DISGUISING: Self = Self(2);
    pub const DISGUISED: Self = Self(3);
    pub const STEALTHED: Self = Self(4);
    pub const INVULNERABLE: Self = Self(5);
    pub const CRIT_BOOSTED: Self = Self(11);
    pub const PHASE: Self = Self(14);
    pub const OFFENSE_BUFF: Self = Self(16);
    pub const ENERGY_BUFF: Self = Self(19);
    pub const HEALTH_BUFF: Self = Self(21);
    pub const BURNING: Self = Self(22);
    pub const HEALTH_OVERHEALED: Self = Self(23);
    pub const URINE: Self = Self(24);
    pub const BLEEDING: Self = Self(25);
    pub const DEFENSE_BUFF: Self = Self(26);
    pub const MAD_MILK: Self = Self(27);
    pub const MARKED_FOR_DEATH: Self = Self(30);
    pub const NO_HEALING_DAMAGE_BUFF: Self = Self(31);
    pub const CRIT_BOOSTED_PUMPKIN: Self = Self(33);
    pub const CRIT_BOOSTED_USER: Self = Self(34);
    pub const CRIT_BOOSTED_FIRST_BLOOD: Self = Self(37);
    pub const CRIT_BOOSTED_BONUS_TIME: Self = Self(38);
    pub const CRIT_BOOSTED_CTF_CAPTURE: Self = Self(39);
    pub const CRIT_BOOSTED_ON_KILL: Self = Self(40);
    pub const DEFENSE_BUFF_NO_CRIT_BLOCK: Self = Self(42);
    pub const CRIT_BOOSTED_RAGE: Self = Self(44);
    pub const DEFENSE_BUFF_HIGH: Self = Self(45);
    pub const MARKED_FOR_DEATH_SILENT: Self = Self(48);
    pub const INVULNERABLE_HIDE_UNLESS_DAMAGED: Self = Self(51);
    pub const INVULNERABLE_USER: Self = Self(52);
    pub const CRIT_BOOSTED_CARD: Self = Self(56);
    pub const INVULNERABLE_CARD: Self = Self(57);
    pub const UBER_BULLET_RESIST: Self = Self(58);
    pub const UBER_BLAST_RESIST: Self = Self(59);
    pub const UBER_FIRE_RESIST: Self = Self(60);
    pub const SMALL_BULLET_RESIST: Self = Self(61);
    pub const SMALL_BLAST_RESIST: Self = Self(62);
    pub const SMALL_FIRE_RESIST: Self = Self(63);
    pub const STEALTHED_USER: Self = Self(64);
    pub const BULLET_IMMUNE: Self = Self(67);
    pub const BLAST_IMMUNE: Self = Self(68);
    pub const FIRE_IMMUNE: Self = Self(69);
    pub const PREVENT_DEATH: Self = Self(70);
    pub const GHOST: Self = Self(77);
    pub const PASSTIME_INTERCEPTION: Self = Self(106);
    pub const PLAGUE: Self = Self(112);
    pub const HEALING_DEBUFF: Self = Self(118);
    pub const PASSTIME_PENALTY: Self = Self(119);
    pub const GAS: Self = Self(123);
    pub const COMPETITIVE_WINNER: Self = Self(116);
    pub const COMPETITIVE_LOSER: Self = Self(117);
    pub const CRIT_BOOSTED_RUNE_TEMP: Self = Self(105);

    pub const fn new(value: u8) -> Option<Self> {
        if value < CONDITION_COUNT as u8 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConditionDuration {
    Permanent,
    Finite(f32),
}

impl ConditionDuration {
    fn validate(self) -> bool {
        match self {
            Self::Permanent => true,
            Self::Finite(value) => value.is_finite() && value > 0.0,
        }
    }

    fn remaining(self) -> f32 {
        match self {
            Self::Permanent => -1.0,
            Self::Finite(value) => value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConditionEntry {
    pub identity: ConditionId,
    pub duration: ConditionDuration,
    pub minimum_duration: f32,
    pub provider: Option<u32>,
    pub previous_active: bool,
    pub prevented_damage: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionEventKind {
    Added,
    Refreshed,
    Removed,
    Expired,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConditionEvent {
    pub condition: ConditionId,
    pub kind: ConditionEventKind,
    pub provider: Option<u32>,
    pub duration: Option<f32>,
    pub prevented_damage: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConditionState {
    entries: Vec<Option<ConditionEntry>>,
}

impl Default for ConditionState {
    fn default() -> Self {
        Self {
            entries: vec![None; CONDITION_COUNT],
        }
    }
}

impl ConditionState {
    pub fn contains(&self, condition: ConditionId) -> bool {
        self.entries[condition.0 as usize].is_some()
    }

    pub fn entry(&self, condition: ConditionId) -> Option<&ConditionEntry> {
        self.entries[condition.0 as usize].as_ref()
    }

    pub fn provider(&self, condition: ConditionId) -> Option<u32> {
        self.entry(condition).and_then(|entry| entry.provider)
    }

    pub fn duration(&self, condition: ConditionId) -> Option<f32> {
        self.entry(condition).map(|entry| {
            if condition == ConditionId::CRIT_BOOSTED {
                0.0
            } else {
                entry.duration.remaining()
            }
        })
    }

    pub fn words(&self) -> [u32; 5] {
        let mut words = [0_u32; 5];
        for (value, entry) in self.entries.iter().enumerate() {
            if entry.is_some() {
                words[value / 32] |= 1_u32 << (value % 32);
            }
        }
        words
    }

    pub fn add(
        &mut self,
        condition: ConditionId,
        duration: ConditionDuration,
        provider: Option<u32>,
        alive: bool,
        competitive_summary_allowed: bool,
    ) -> Result<Option<ConditionEvent>, ConditionError> {
        if !duration.validate() {
            return Err(ConditionError::InvalidDuration);
        }
        if !alive {
            return Ok(None);
        }
        if matches!(
            condition,
            ConditionId::COMPETITIVE_WINNER | ConditionId::COMPETITIVE_LOSER
        ) && !competitive_summary_allowed
        {
            return Ok(None);
        }
        if condition == ConditionId::CRIT_BOOSTED {
            return Ok(Some(self.add_crit_boost(duration, provider)));
        }
        let slot = &mut self.entries[condition.0 as usize];
        let previous = slot.as_ref().map(|entry| entry.duration);
        let selected = match (previous, duration) {
            (Some(ConditionDuration::Permanent), ConditionDuration::Finite(_)) => {
                ConditionDuration::Permanent
            }
            (Some(ConditionDuration::Finite(current)), ConditionDuration::Finite(incoming))
                if incoming < current =>
            {
                ConditionDuration::Finite(current)
            }
            (_, incoming) => incoming,
        };
        let refreshed = slot.is_some();
        *slot = Some(ConditionEntry {
            identity: condition,
            duration: selected,
            minimum_duration: 0.0,
            provider,
            previous_active: refreshed,
            prevented_damage: 0,
        });
        Ok(Some(ConditionEvent {
            condition,
            kind: if refreshed {
                ConditionEventKind::Refreshed
            } else {
                ConditionEventKind::Added
            },
            provider,
            duration: finite_value(selected),
            prevented_damage: 0,
        }))
    }

    pub fn remove(&mut self, condition: ConditionId, force: bool) -> Option<ConditionEvent> {
        let slot = &mut self.entries[condition.0 as usize];
        let entry = slot.as_mut()?;
        if condition == ConditionId::CRIT_BOOSTED && !force && entry.minimum_duration > 0.0 {
            entry.duration = ConditionDuration::Finite(entry.minimum_duration);
            return None;
        }
        let entry = slot.take().expect("entry was present");
        Some(ConditionEvent {
            condition,
            kind: ConditionEventKind::Removed,
            provider: entry.provider,
            duration: finite_value(entry.duration),
            prevented_damage: entry.prevented_damage,
        })
    }

    pub fn remove_all(&mut self) -> Vec<ConditionEvent> {
        let mut events = Vec::new();
        for value in 0..CONDITION_COUNT as u8 {
            if let Some(event) = self.remove(ConditionId(value), true) {
                events.push(event);
            }
        }
        events
    }

    pub fn advance(
        &mut self,
        tick_interval: f32,
        healer_count: usize,
    ) -> Result<Vec<ConditionEvent>, ConditionError> {
        if !tick_interval.is_finite() || tick_interval <= 0.0 {
            return Err(ConditionError::InvalidTickInterval);
        }
        let mut expired = Vec::new();
        for value in 0..CONDITION_COUNT as u8 {
            let condition = ConditionId(value);
            let Some(entry) = self.entries[value as usize].as_mut() else {
                continue;
            };
            let reduction = expiry_reduction(condition, tick_interval, healer_count);
            if entry.minimum_duration > 0.0 {
                entry.minimum_duration = (entry.minimum_duration - reduction).max(0.0);
            }
            if let ConditionDuration::Finite(remaining) = &mut entry.duration {
                *remaining = (*remaining - reduction).max(0.0);
                if *remaining < entry.minimum_duration {
                    *remaining = entry.minimum_duration;
                }
                if *remaining == 0.0 {
                    expired.push(condition);
                }
            }
        }
        Ok(expired
            .into_iter()
            .filter_map(|condition| {
                let mut event = self.remove(condition, true)?;
                event.kind = ConditionEventKind::Expired;
                Some(event)
            })
            .collect())
    }

    pub fn add_prevented_damage(&mut self, condition: ConditionId, amount: u32) {
        if let Some(entry) = self.entries[condition.0 as usize].as_mut() {
            entry.prevented_damage = entry.prevented_damage.saturating_add(amount);
        }
    }

    pub fn is_invulnerable(&self) -> bool {
        [
            ConditionId::INVULNERABLE,
            ConditionId::INVULNERABLE_USER,
            ConditionId::INVULNERABLE_HIDE_UNLESS_DAMAGED,
            ConditionId::INVULNERABLE_CARD,
        ]
        .into_iter()
        .any(|condition| self.contains(condition))
    }

    pub fn is_crit_boosted(&self) -> bool {
        [
            ConditionId::CRIT_BOOSTED,
            ConditionId::CRIT_BOOSTED_PUMPKIN,
            ConditionId::CRIT_BOOSTED_USER,
            ConditionId::CRIT_BOOSTED_FIRST_BLOOD,
            ConditionId::CRIT_BOOSTED_BONUS_TIME,
            ConditionId::CRIT_BOOSTED_CTF_CAPTURE,
            ConditionId::CRIT_BOOSTED_ON_KILL,
            ConditionId::CRIT_BOOSTED_CARD,
            ConditionId::CRIT_BOOSTED_RUNE_TEMP,
        ]
        .into_iter()
        .any(|condition| self.contains(condition))
    }

    pub fn condition_assister(&self) -> Option<u32> {
        [
            ConditionId::URINE,
            ConditionId::MAD_MILK,
            ConditionId::MARKED_FOR_DEATH,
            ConditionId::GAS,
        ]
        .into_iter()
        .find_map(|condition| self.provider(condition))
    }

    fn add_crit_boost(
        &mut self,
        duration: ConditionDuration,
        provider: Option<u32>,
    ) -> ConditionEvent {
        let slot = &mut self.entries[ConditionId::CRIT_BOOSTED.0 as usize];
        if let Some(entry) = slot.as_mut() {
            match (entry.duration, duration) {
                (ConditionDuration::Permanent, ConditionDuration::Finite(incoming)) => {
                    entry.minimum_duration = entry.minimum_duration.max(incoming);
                }
                (ConditionDuration::Finite(current), ConditionDuration::Finite(incoming))
                    if incoming < current =>
                {
                    entry.minimum_duration = entry.minimum_duration.max(incoming);
                }
                (ConditionDuration::Finite(current), ConditionDuration::Permanent) => {
                    entry.minimum_duration = entry.minimum_duration.max(current);
                    entry.duration = ConditionDuration::Permanent;
                }
                (_, incoming) => entry.duration = incoming,
            }
            entry.provider = provider;
            return ConditionEvent {
                condition: ConditionId::CRIT_BOOSTED,
                kind: ConditionEventKind::Refreshed,
                provider,
                duration: finite_value(entry.duration),
                prevented_damage: 0,
            };
        }
        *slot = Some(ConditionEntry {
            identity: ConditionId::CRIT_BOOSTED,
            duration,
            minimum_duration: 0.0,
            provider,
            previous_active: false,
            prevented_damage: 0,
        });
        ConditionEvent {
            condition: ConditionId::CRIT_BOOSTED,
            kind: ConditionEventKind::Added,
            provider,
            duration: finite_value(duration),
            prevented_damage: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionError {
    InvalidDuration,
    InvalidTickInterval,
}

fn finite_value(duration: ConditionDuration) -> Option<f32> {
    match duration {
        ConditionDuration::Permanent => None,
        ConditionDuration::Finite(value) => Some(value),
    }
}

fn expiry_reduction(condition: ConditionId, interval: f32, healers: usize) -> f32 {
    if healers == 0 {
        return interval;
    }
    if condition == ConditionId::URINE {
        interval + healers as f32 * interval
    } else if [
        ConditionId::BURNING,
        ConditionId::BLEEDING,
        ConditionId::MAD_MILK,
        ConditionId::GAS,
    ]
    .contains(&condition)
    {
        interval + healers as f32 * interval * 4.0
    } else {
        interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_condition_identities_map_to_five_words() {
        let mut state = ConditionState::default();
        for value in 0..CONDITION_COUNT as u8 {
            let condition = ConditionId::new(value).unwrap();
            state
                .add(
                    condition,
                    ConditionDuration::Permanent,
                    Some(value as u32),
                    true,
                    true,
                )
                .unwrap();
        }
        assert_eq!(
            state.words(),
            [u32::MAX, u32::MAX, u32::MAX, u32::MAX, 0b111]
        );
        assert!(ConditionId::new(131).is_none());
        let removed = state.remove_all();
        assert_eq!(removed.len(), CONDITION_COUNT);
        assert_eq!(removed.first().unwrap().condition.value(), 0);
        assert_eq!(removed.last().unwrap().condition.value(), 130);
    }

    #[test]
    fn ordinary_refresh_cannot_shorten_and_replaces_provider() {
        let mut state = ConditionState::default();
        state
            .add(
                ConditionId::BURNING,
                ConditionDuration::Finite(10.0),
                Some(1),
                true,
                false,
            )
            .unwrap();
        let event = state
            .add(
                ConditionId::BURNING,
                ConditionDuration::Finite(3.0),
                Some(2),
                true,
                false,
            )
            .unwrap()
            .unwrap();
        assert_eq!(event.kind, ConditionEventKind::Refreshed);
        assert_eq!(state.duration(ConditionId::BURNING), Some(10.0));
        assert_eq!(state.provider(ConditionId::BURNING), Some(2));
        state
            .add(
                ConditionId::BURNING,
                ConditionDuration::Permanent,
                Some(3),
                true,
                false,
            )
            .unwrap();
        assert_eq!(state.duration(ConditionId::BURNING), Some(-1.0));
    }

    #[test]
    fn crit_boost_minimum_duration_blocks_remove_until_elapsed() {
        let mut state = ConditionState::default();
        state
            .add(
                ConditionId::CRIT_BOOSTED,
                ConditionDuration::Permanent,
                Some(1),
                true,
                false,
            )
            .unwrap();
        state
            .add(
                ConditionId::CRIT_BOOSTED,
                ConditionDuration::Finite(2.0),
                Some(2),
                true,
                false,
            )
            .unwrap();
        assert_eq!(
            state
                .entry(ConditionId::CRIT_BOOSTED)
                .unwrap()
                .minimum_duration,
            2.0
        );
        assert!(state.remove(ConditionId::CRIT_BOOSTED, false).is_none());
        assert_eq!(state.duration(ConditionId::CRIT_BOOSTED), Some(0.0));
        assert_eq!(
            state
                .entry(ConditionId::CRIT_BOOSTED)
                .unwrap()
                .duration
                .remaining(),
            2.0
        );
        assert!(state.advance(1.0, 0).unwrap().is_empty());
        let events = state.advance(1.0, 0).unwrap();
        assert_eq!(events[0].kind, ConditionEventKind::Expired);
    }

    #[test]
    fn healers_accelerate_only_the_declared_debuffs() {
        let mut state = ConditionState::default();
        for condition in [
            ConditionId::BURNING,
            ConditionId::URINE,
            ConditionId::MARKED_FOR_DEATH,
        ] {
            state
                .add(
                    condition,
                    ConditionDuration::Finite(10.0),
                    None,
                    true,
                    false,
                )
                .unwrap();
        }
        state.advance(1.0, 2).unwrap();
        assert_eq!(state.duration(ConditionId::BURNING), Some(1.0));
        assert_eq!(state.duration(ConditionId::URINE), Some(7.0));
        assert_eq!(state.duration(ConditionId::MARKED_FOR_DEATH), Some(9.0));
    }

    #[test]
    fn dead_and_disallowed_competitive_additions_are_inert() {
        let mut state = ConditionState::default();
        assert!(
            state
                .add(
                    ConditionId::BURNING,
                    ConditionDuration::Finite(1.0),
                    None,
                    false,
                    false
                )
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .add(
                    ConditionId::COMPETITIVE_WINNER,
                    ConditionDuration::Permanent,
                    None,
                    true,
                    false
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(state.words(), [0; 5]);
    }

    #[test]
    fn semantic_condition_sets_and_assist_priority_are_exact() {
        let mut state = ConditionState::default();
        for condition in [
            ConditionId::INVULNERABLE_HIDE_UNLESS_DAMAGED,
            ConditionId::CRIT_BOOSTED_ON_KILL,
            ConditionId::GAS,
            ConditionId::MARKED_FOR_DEATH,
            ConditionId::MAD_MILK,
            ConditionId::URINE,
        ] {
            state
                .add(
                    condition,
                    ConditionDuration::Permanent,
                    Some(condition.value() as u32),
                    true,
                    false,
                )
                .unwrap();
        }
        assert!(state.is_invulnerable());
        assert!(state.is_crit_boosted());
        assert_eq!(
            state.condition_assister(),
            Some(ConditionId::URINE.value() as u32)
        );
    }
}
