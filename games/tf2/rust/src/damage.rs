use std::ops::{BitOr, BitOrAssign};

use crate::{
    class::PlayerTeam,
    condition::{ConditionEvent, ConditionId, ConditionState},
    health::HealthState,
    schema::LoadoutPosition,
};

pub const CRIT_MULTIPLIER: f32 = 3.0;
pub const MINICRIT_MULTIPLIER: f32 = 1.35;
pub const RANDOM_RANGE: u16 = 10_000;
pub const CRIT_BUCKET_DEFAULT: f32 = 300.0;
pub const CRIT_BUCKET_CAP: f32 = 1_000.0;
pub const CRIT_BUCKET_BOTTOM: f32 = -250.0;

/// CTFPlayer's damage-to-velocity magnitude, distinct from ragdoll damage force.
pub fn player_damage_force(size: [f32; 3], amount: f32, multiplier: f32) -> f32 {
    (amount * (48.0 * 48.0 * 82.0 / (size[0] * size[1] * size[2])) * multiplier).min(1000.0)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DamageType(u32);

impl DamageType {
    pub const fn from_source_bits(bits: u32) -> Self { Self(bits) }
    pub const GENERIC: Self = Self(0);
    pub const BULLET: Self = Self(1 << 1);
    pub const BUCKSHOT: Self = Self(1 << 29);
    pub const BLAST: Self = Self(1 << 6);
    pub const SONIC: Self = Self(1 << 9);
    pub const BURN: Self = Self(1 << 3);
    pub const IGNITE: Self = Self(1 << 24);
    pub const MELEE: Self = Self(1 << 27);
    pub const CRUSH: Self = Self(1 << 0);
    pub const FALL: Self = Self(1 << 5);
    pub const DROWN: Self = Self(1 << 14);
    pub const PREVENT_FORCE: Self = Self(1 << 11);
    pub const SLASH: Self = Self(1 << 2);
    pub const CLUB: Self = Self(1 << 7);
    pub const NEVER_GIB: Self = Self(1 << 12);
    pub const USE_DISTANCE: Self = Self(1 << 21);
    pub const HALF_FALLOFF: Self = Self(1 << 18);
    pub const NO_CLOSE_DISTANCE: Self = Self(1 << 17);
    pub const USE_HITLOCATIONS: Self = Self(1 << 25);
    pub const NO_CRIT_RATE: Self = Self(1 << 26);

    pub const fn source_bits(self, crit: CritKind) -> u32 {
        self.0 | if !matches!(crit, CritKind::None) { 1 << 20 } else { 0 }
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for DamageType {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for DamageType {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CritKind {
    None,
    Mini,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponCritKind {
    SingleShot,
    RapidFire,
    Melee,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageHistoryEvent {
    pub time: f32,
    pub damage: f32,
    pub counts_toward_crit_rate: bool,
    pub blast: bool,
    pub kills: u16,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DamageHistory {
    events: Vec<DamageHistoryEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageHistoryInput {
    pub now: f32,
    pub damage: f32,
    pub bonus_damage: f32,
    pub victim_previous_health: i32,
    pub lethal: bool,
    pub damage_type: DamageType,
    pub counts_toward_crit_rate: bool,
}

impl DamageHistory {
    pub fn record(&mut self, input: DamageHistoryInput) -> Result<(), DamageError> {
        let DamageHistoryInput {
            now,
            damage,
            bonus_damage,
            victim_previous_health,
            lethal,
            damage_type,
            counts_toward_crit_rate,
        } = input;
        if !now.is_finite() || !damage.is_finite() || !bonus_damage.is_finite() {
            return Err(DamageError::NonFiniteInput);
        }
        if self.events.len() >= 128 {
            self.events.pop();
        }
        let mut base = damage - bonus_damage;
        if lethal && base > victim_previous_health as f32 {
            base = victim_previous_health as f32;
        }
        if damage_type.contains(DamageType::BLAST) {
            let mut overridden = false;
            for event in &mut self.events {
                if event.blast && now - event.time < 0.1 {
                    if lethal {
                        event.kills = event.kills.saturating_add(1);
                    }
                    if base > event.damage {
                        event.damage = base;
                        event.counts_toward_crit_rate = counts_toward_crit_rate;
                        event.time = now;
                    }
                    overridden = true;
                }
            }
            if overridden {
                return Ok(());
            }
        }
        self.events.push(DamageHistoryEvent {
            time: now,
            damage: base,
            counts_toward_crit_rate,
            blast: damage_type.contains(DamageType::BLAST),
            kills: u16::from(lethal),
        });
        Ok(())
    }

    pub fn crit_multiplier(&mut self, now: f32) -> Result<f32, DamageError> {
        if !now.is_finite() {
            return Err(DamageError::NonFiniteInput);
        }
        self.events.retain(|event| now - event.time <= 30.0);
        let total = self
            .events
            .iter()
            .filter(|event| (2.0..=20.0).contains(&(now - event.time)))
            .map(|event| {
                if event.counts_toward_crit_rate {
                    event.damage
                } else {
                    0.0
                }
            })
            .sum::<f32>();
        let unquantized = remap_clamped(total, 0.0, 800.0, 1.0, 6.0);
        let network_value = remap_clamped(unquantized, 1.0, 4.0, 0.0, 255.0) as u8;
        Ok(remap_clamped(network_value as f32, 0.0, 255.0, 1.0, 4.0))
    }

    pub fn events(&self) -> &[DamageHistoryEvent] {
        &self.events
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CritState {
    pub token_bucket: f32,
    pub checks: u32,
    pub seed_requests: u32,
    pub observed_chance: f32,
    pub random_ranged_crit_damage: u32,
    pub total_ranged_damage: u32,
    pub last_rapid_check_time: f32,
    pub rapid_crit_end_time: f32,
}

impl Default for CritState {
    fn default() -> Self {
        Self {
            token_bucket: CRIT_BUCKET_DEFAULT,
            checks: 0,
            seed_requests: 0,
            observed_chance: 0.0,
            random_ranged_crit_damage: 0,
            total_ranged_damage: 0,
            last_rapid_check_time: f32::NEG_INFINITY,
            rapid_crit_end_time: f32::NEG_INFINITY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CritCheckInput {
    pub now: f32,
    pub kind: WeaponCritKind,
    pub random_crits_enabled: bool,
    pub can_fire_critical: bool,
    pub guaranteed_critical: bool,
    pub raw_damage: f32,
    pub projectiles_per_shot: f32,
    pub fire_delay: f32,
    pub player_crit_multiplier: f32,
    pub chance_multiplier: f32,
    pub roll: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CritCheckResult {
    pub kind: CritKind,
    pub random: bool,
    pub roll: Option<u16>,
    pub threshold: f32,
    pub bucket_before: f32,
    pub bucket_after: f32,
    pub checked: bool,
    pub denied_by_observed_rate: bool,
    pub denied_by_bucket: bool,
}

fn validate_crit_input(input: CritCheckInput) -> Result<(), DamageError> {
    if !input.now.is_finite() || !input.raw_damage.is_finite() || input.raw_damage < 0.0
        || !input.projectiles_per_shot.is_finite() || input.projectiles_per_shot < 0.0
        || !input.fire_delay.is_finite() || input.fire_delay <= 0.0
        || !input.chance_multiplier.is_finite() || input.chance_multiplier < 0.0
        || !input.player_crit_multiplier.is_finite() || input.player_crit_multiplier < 0.0
        || input.roll.is_some_and(|roll| roll >= RANDOM_RANGE) {
        Err(DamageError::InvalidCritInput)
    } else { Ok(()) }
}

impl CritState {
    pub fn needs_roll(&self, input: CritCheckInput) -> Result<bool, DamageError> {
        validate_crit_input(input)?;
        Ok(input.can_fire_critical && !input.guaranteed_critical && input.random_crits_enabled
            && !(input.kind == WeaponCritKind::RapidFire
                && (self.rapid_crit_end_time > input.now || input.now < self.last_rapid_check_time + 1.0)))
    }

    pub fn check(&mut self, input: CritCheckInput) -> Result<CritCheckResult, DamageError> {
        validate_crit_input(input)?;
        let bucket_before = self.token_bucket;
        let base_result = |kind, random, roll, threshold, bucket_after, checked| CritCheckResult {
            kind,
            random,
            roll,
            threshold,
            bucket_before,
            bucket_after,
            checked,
            denied_by_observed_rate: false,
            denied_by_bucket: false,
        };
        if !input.can_fire_critical {
            return Ok(base_result(
                CritKind::None,
                false,
                None,
                0.0,
                self.token_bucket,
                false,
            ));
        }
        if input.guaranteed_critical {
            return Ok(base_result(
                CritKind::Full,
                false,
                None,
                0.0,
                self.token_bucket,
                false,
            ));
        }
        if !input.random_crits_enabled {
            return Ok(base_result(
                CritKind::None,
                false,
                None,
                0.0,
                self.token_bucket,
                false,
            ));
        }
        if input.kind == WeaponCritKind::RapidFire && self.rapid_crit_end_time > input.now {
            return Ok(base_result(
                CritKind::Full,
                true,
                None,
                0.0,
                self.token_bucket,
                false,
            ));
        }

        let raw = input.raw_damage * input.projectiles_per_shot;
        self.token_bucket = (self.token_bucket + raw).min(CRIT_BUCKET_CAP);
        let chance = match input.kind {
            WeaponCritKind::SingleShot => {
                0.02 * input.player_crit_multiplier * input.chance_multiplier
            }
            WeaponCritKind::Melee => 0.15 * input.player_crit_multiplier * input.chance_multiplier,
            WeaponCritKind::RapidFire => {
                if input.now < self.last_rapid_check_time + 1.0 {
                    return Ok(base_result(
                        CritKind::None,
                        true,
                        None,
                        0.0,
                        self.token_bucket,
                        false,
                    ));
                }
                self.last_rapid_check_time = input.now;
                let total = (0.02 * input.player_crit_multiplier).clamp(0.01, 0.99);
                let non_crit_duration = (2.0 / total) - 2.0;
                (1.0 / non_crit_duration) * input.chance_multiplier
            }
        };
        let roll = input.roll.ok_or(DamageError::MissingCritRoll)?;
        self.checks = self.checks.saturating_add(1);
        let threshold = chance * RANDOM_RANGE as f32;
        if roll as f32 >= threshold {
            return Ok(base_result(
                CritKind::None,
                true,
                Some(roll),
                threshold,
                self.token_bucket,
                true,
            ));
        }

        if input.kind != WeaponCritKind::Melee && self.total_ranged_damage > 0 {
            let normalized = self.random_ranged_crit_damage as f32 / CRIT_MULTIPLIER;
            self.observed_chance = normalized
                / (normalized + self.total_ranged_damage as f32
                    - self.random_ranged_crit_damage as f32);
        }
        if input.kind != WeaponCritKind::Melee && self.total_ranged_damage > 0
            && self.observed_chance > chance + 0.1 {
            let mut result = base_result(
                CritKind::None,
                true,
                Some(roll),
                threshold,
                self.token_bucket,
                true,
            );
            result.denied_by_observed_rate = true;
            return Ok(result);
        }

        let mut cost_damage = raw;
        if input.kind == WeaponCritKind::RapidFire {
            cost_damage *= 2.0 / input.fire_delay;
            if cost_damage * CRIT_MULTIPLIER > CRIT_BUCKET_CAP {
                cost_damage = CRIT_BUCKET_CAP / CRIT_MULTIPLIER;
            }
        }
        self.seed_requests = self.seed_requests.saturating_add(1);
        let cost_multiplier = if input.kind == WeaponCritKind::Melee {
            0.5
        } else {
            remap_clamped(
                self.seed_requests as f32 / self.checks.max(1) as f32,
                0.1,
                1.0,
                1.0,
                3.0,
            )
        };
        let cost = cost_damage * CRIT_MULTIPLIER * cost_multiplier;
        if cost > self.token_bucket {
            let mut result = base_result(
                CritKind::None,
                true,
                Some(roll),
                threshold,
                self.token_bucket,
                true,
            );
            result.denied_by_bucket = true;
            return Ok(result);
        }
        self.token_bucket = (self.token_bucket - cost).max(CRIT_BUCKET_BOTTOM);
        if input.kind == WeaponCritKind::RapidFire {
            self.rapid_crit_end_time = input.now + 2.0;
        }
        Ok(base_result(
            CritKind::Full,
            true,
            Some(roll),
            threshold,
            self.token_bucket,
            true,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageSourceKind {
    Player,
    World,
    Trigger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomDamage {
    None,
    Headshot,
    Backstab,
    Burning,
    Bleeding,
    Telefrag,
    TriggerHurt,
    Other(u16),
}

impl CustomDamage {
    pub const fn source_code(self) -> u16 {
        match self {
            Self::None => 0, Self::Headshot => 1, Self::Backstab => 2,
            Self::Burning => 3, Self::Telefrag => 16, Self::Bleeding => 34,
            Self::TriggerHurt => 40, Self::Other(value) => value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageInput {
    pub attacker: u32,
    pub attacker_team: PlayerTeam,
    pub attacker_conditions: ConditionState,
    pub source: DamageSourceKind,
    pub weapon_position: Option<LoadoutPosition>,
    pub victim: u32,
    pub victim_team: PlayerTeam,
    pub base_damage: f32,
    pub range_multiplier: f32,
    pub damage_type: DamageType,
    pub custom: CustomDamage,
    pub crit: CritKind,
    pub friendly_fire: bool,
    pub force_friendly_fire: bool,
    pub bypass_invulnerability: bool,
    pub force: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageModifiers {
    pub outgoing_vs_players: f32,
    pub critical_bonus_taken: f32,
    pub fire_taken: f32,
    pub blast_taken: f32,
    pub bullet_taken: f32,
    pub melee_taken: f32,
    pub general_taken: f32,
    pub active_taken: f32,
    pub spunup_taken: f32,
    pub critical_falloff: bool,
    pub pierces_resists: bool,
    pub minicrits_become_crits: bool,
    pub crits_become_minicrits: bool,
}

impl Default for DamageModifiers {
    fn default() -> Self {
        Self {
            outgoing_vs_players: 1.0,
            critical_bonus_taken: 1.0,
            fire_taken: 1.0,
            blast_taken: 1.0,
            bullet_taken: 1.0,
            melee_taken: 1.0,
            general_taken: 1.0,
            active_taken: 1.0,
            spunup_taken: 1.0,
            critical_falloff: false,
            pierces_resists: false,
            minicrits_become_crits: false,
            crits_become_minicrits: false,
        }
    }
}

impl DamageModifiers {
    fn validate(self) -> bool {
        [
            self.outgoing_vs_players,
            self.critical_bonus_taken,
            self.fire_taken,
            self.blast_taken,
            self.bullet_taken,
            self.melee_taken,
            self.general_taken,
            self.active_taken,
            self.spunup_taken,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value >= 0.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageDenial {
    VictimNotActive,
    VictimDead,
    ZeroDamage,
    FriendlyFire,
    Ghost,
    Invulnerable,
    Phased,
    TypedImmunity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageStageKind {
    Base,
    OutgoingPlayerModifier,
    DefenseBuff,
    Range,
    CriticalBonus,
    CriticalResistance,
    TypedResistance,
    GeneralResistance,
    RoundedHealth,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageStage {
    pub kind: DamageStageKind,
    pub base: f32,
    pub bonus: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorpseKind {
    Ragdoll,
    Gib,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeathFact {
    pub victim: u32,
    pub scorer: Option<u32>,
    pub assister: Option<u32>,
    pub custom: CustomDamage,
    pub corpse: CorpseKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DamageResult {
    pub admitted: bool,
    pub denial: Option<DamageDenial>,
    pub crit: CritKind,
    pub pre_resistance_base_damage: f32,
    pub pre_resistance_bonus_damage: f32,
    pub base_damage: f32,
    pub bonus_damage: f32,
    pub final_damage: f32,
    pub health_damage: i32,
    pub health_before: i32,
    pub health_after: i32,
    pub force: [f32; 3],
    pub stages: Vec<DamageStage>,
    pub condition_events: Vec<ConditionEvent>,
    pub death: Option<DeathFact>,
}

pub fn apply_damage(
    active: bool,
    health: &mut HealthState,
    victim_conditions: &mut ConditionState,
    input: &DamageInput,
    modifiers: DamageModifiers,
) -> Result<DamageResult, DamageError> {
    if !input.base_damage.is_finite()
        || input.base_damage < 0.0
        || !input.range_multiplier.is_finite()
        || input.range_multiplier < 0.0
        || input.force.into_iter().any(|value| !value.is_finite())
        || !modifiers.validate()
    {
        return Err(DamageError::NonFiniteInput);
    }
    let before = health.current;
    let denied = |denial, force| DamageResult {
        admitted: false,
        denial: Some(denial),
        crit: input.crit,
        pre_resistance_base_damage: 0.0,
        pre_resistance_bonus_damage: 0.0,
        base_damage: 0.0,
        bonus_damage: 0.0,
        final_damage: 0.0,
        health_damage: 0,
        health_before: before,
        health_after: before,
        force,
        stages: Vec::new(),
        condition_events: Vec::new(),
        death: None,
    };
    if !active {
        return Ok(denied(DamageDenial::VictimNotActive, [0.0; 3]));
    }
    if health.current <= 0 {
        return Ok(denied(DamageDenial::VictimDead, [0.0; 3]));
    }
    if input.base_damage == 0.0 {
        return Ok(denied(DamageDenial::ZeroDamage, [0.0; 3]));
    }
    let self_damage = input.attacker == input.victim;
    if !self_damage
        && input.attacker_team == input.victim_team
        && !input.friendly_fire
        && !input.force_friendly_fire
    {
        return Ok(denied(DamageDenial::FriendlyFire, [0.0; 3]));
    }
    if victim_conditions.contains(ConditionId::GHOST) {
        return Ok(denied(DamageDenial::Ghost, [0.0; 3]));
    }
    let force = if input.damage_type.contains(DamageType::PREVENT_FORCE) {
        [0.0; 3]
    } else {
        input.force
    };
    if victim_conditions.is_invulnerable() && !input.bypass_invulnerability {
        victim_conditions
            .add_prevented_damage(ConditionId::INVULNERABLE, input.base_damage.max(0.0) as u32);
        return Ok(denied(DamageDenial::Invulnerable, force));
    }
    if (victim_conditions.contains(ConditionId::PHASE)
        || victim_conditions.contains(ConditionId::PASSTIME_INTERCEPTION))
        && !input.bypass_invulnerability
    {
        victim_conditions
            .add_prevented_damage(ConditionId::PHASE, input.base_damage.max(0.0) as u32);
        return Ok(denied(DamageDenial::Phased, force));
    }

    let mut stages = vec![DamageStage {
        kind: DamageStageKind::Base,
        base: input.base_damage,
        bonus: 0.0,
    }];
    let mut base = input.base_damage * modifiers.outgoing_vs_players;
    let mut crit = input.crit;
    if crit == CritKind::None
        && !self_damage
        && ([
            ConditionId::URINE,
            ConditionId::MARKED_FOR_DEATH,
            ConditionId::MARKED_FOR_DEATH_SILENT,
            ConditionId::PASSTIME_PENALTY,
        ]
        .into_iter()
        .any(|condition| victim_conditions.contains(condition))
            || [
                ConditionId::OFFENSE_BUFF,
                ConditionId::NO_HEALING_DAMAGE_BUFF,
                ConditionId::ENERGY_BUFF,
            ]
            .into_iter()
            .any(|condition| input.attacker_conditions.contains(condition)))
    {
        crit = CritKind::Mini;
    }
    if crit == CritKind::Mini && modifiers.minicrits_become_crits {
        crit = CritKind::Full;
    }
    stages.push(DamageStage {
        kind: DamageStageKind::OutgoingPlayerModifier,
        base,
        bonus: 0.0,
    });

    let defense_applies =
        input.custom != CustomDamage::Backstab && !input.damage_type.contains(DamageType::CRUSH);
    if defense_applies {
        if victim_conditions.contains(ConditionId::DEFENSE_BUFF) {
            crit = CritKind::None;
        }
        if !modifiers.pierces_resists {
            if victim_conditions.contains(ConditionId::DEFENSE_BUFF_HIGH) {
                base *= 0.25;
            } else if victim_conditions.contains(ConditionId::DEFENSE_BUFF)
                || victim_conditions.contains(ConditionId::DEFENSE_BUFF_NO_CRIT_BLOCK)
            {
                base *= 0.65;
            }
        }
    }
    let ranged_as_full_crit = crit == CritKind::Full;
    if ranged_as_full_crit && modifiers.crits_become_minicrits {
        crit = CritKind::Mini;
    }
    stages.push(DamageStage {
        kind: DamageStageKind::DefenseBuff,
        base,
        bonus: 0.0,
    });

    let unvaried_base = base;
    let effective_range = if self_damage { 1.0 }
        else if ranged_as_full_crit { if modifiers.critical_falloff { input.range_multiplier.min(1.0) } else { 1.0 } }
        else if crit == CritKind::Mini && !modifiers.critical_falloff { input.range_multiplier.max(1.0) }
        else { input.range_multiplier };
    base *= effective_range;
    stages.push(DamageStage {
        kind: DamageStageKind::Range,
        base,
        bonus: 0.0,
    });
    let critical_damage = if self_damage { 0.0 } else { match crit {
        CritKind::None => 0.0,
        CritKind::Mini => (MINICRIT_MULTIPLIER - 1.0) * base,
        CritKind::Full => (CRIT_MULTIPLIER - 1.0) * base,
    }};
    let variance_bonus = if self_damage || crit == CritKind::None || modifiers.critical_falloff
        || ranged_as_full_crit && input.range_multiplier > 1.0 { 0.0 }
        else { (unvaried_base * (input.range_multiplier - 1.0)).abs() };
    let mut bonus = critical_damage + variance_bonus;
    base -= variance_bonus;
    let pre_resistance_base_damage = base;
    let pre_resistance_bonus_damage = bonus;
    stages.push(DamageStage {
        kind: DamageStageKind::CriticalBonus,
        base,
        bonus,
    });

    if typed_immune(input.damage_type, victim_conditions) {
        return Ok(DamageResult {
            admitted: false,
            denial: Some(DamageDenial::TypedImmunity),
            crit,
            pre_resistance_base_damage,
            pre_resistance_bonus_damage,
            base_damage: 0.0,
            bonus_damage: 0.0,
            final_damage: 0.0,
            health_damage: 0,
            health_before: before,
            health_after: before,
            force,
            stages,
            condition_events: Vec::new(),
            death: None,
        });
    }
    if !modifiers.pierces_resists {
        bonus *= modifiers.critical_bonus_taken;
    }
    stages.push(DamageStage {
        kind: DamageStageKind::CriticalResistance,
        base,
        bonus,
    });
    if !modifiers.pierces_resists {
        if input
            .damage_type
            .intersects(DamageType::BURN | DamageType::IGNITE)
        {
            base *= modifiers.fire_taken;
        }
        if input.damage_type.contains(DamageType::BLAST) && !self_damage {
            base *= modifiers.blast_taken;
        }
        if input
            .damage_type
            .intersects(DamageType::BULLET | DamageType::BUCKSHOT)
        {
            base *= modifiers.bullet_taken;
        }
        if input.damage_type.contains(DamageType::MELEE) {
            base *= modifiers.melee_taken;
        }
        if (before as f32 - (pre_resistance_base_damage + pre_resistance_bonus_damage)) / health.maximum as f32 <= 0.5 {
            base *= modifiers.spunup_taken;
        }
    }
    stages.push(DamageStage {
        kind: DamageStageKind::TypedResistance,
        base,
        bonus,
    });
    let total = (base + bonus) * modifiers.general_taken * modifiers.active_taken;
    stages.push(DamageStage {
        kind: DamageStageKind::GeneralResistance,
        base: total - bonus,
        bonus,
    });
    if total <= 0.0 {
        return Ok(denied(DamageDenial::ZeroDamage, force));
    }
    if total + 0.5 > i32::MAX as f32 {
        return Err(DamageError::DamageOutOfRange);
    }
    let health_damage = (total + 0.5) as i32;
    health.current = health.current.saturating_sub(health_damage);
    stages.push(DamageStage {
        kind: DamageStageKind::RoundedHealth,
        base: health_damage as f32,
        bonus: 0.0,
    });

    let mut condition_events = Vec::new();
    let mut death = None;
    if health.current <= 0 {
        if victim_conditions.contains(ConditionId::PREVENT_DEATH) {
            if let Some(event) = victim_conditions.remove(ConditionId::PREVENT_DEATH, false) {
                condition_events.push(event);
            }
            health.current = 1;
        } else {
            let assister = victim_conditions
                .condition_assister()
                .filter(|identity| *identity != input.attacker);
            death = Some(DeathFact {
                victim: input.victim,
                scorer: (input.source == DamageSourceKind::Player).then_some(input.attacker),
                assister,
                custom: input.custom,
                corpse: if input.damage_type.contains(DamageType::BLAST)
                    && (crit == CritKind::Full || health.current <= -10)
                {
                    CorpseKind::Gib
                } else {
                    CorpseKind::Ragdoll
                },
            });
        }
    }
    Ok(DamageResult {
        admitted: true,
        denial: None,
        crit,
        pre_resistance_base_damage,
        pre_resistance_bonus_damage,
        base_damage: base,
        bonus_damage: bonus,
        final_damage: total,
        health_damage,
        health_before: before,
        health_after: health.current,
        force,
        stages,
        condition_events,
        death,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageError {
    NonFiniteInput,
    InvalidCritInput,
    MissingCritRoll,
    DamageOutOfRange,
}

fn typed_immune(damage_type: DamageType, conditions: &ConditionState) -> bool {
    if damage_type.intersects(DamageType::BURN | DamageType::IGNITE) {
        conditions.contains(ConditionId::FIRE_IMMUNE)
    } else if damage_type.intersects(DamageType::BULLET | DamageType::BUCKSHOT) {
        conditions.contains(ConditionId::BULLET_IMMUNE)
    } else if damage_type.contains(DamageType::BLAST) {
        conditions.contains(ConditionId::BLAST_IMMUNE)
    } else {
        false
    }
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
    use crate::{class::PlayerClass, condition::ConditionDuration};

    fn conditions(values: &[ConditionId]) -> ConditionState {
        let mut state = ConditionState::default();
        for value in values {
            state
                .add(*value, ConditionDuration::Permanent, Some(99), true, true)
                .unwrap();
        }
        state
    }

    fn input() -> DamageInput {
        DamageInput {
            attacker: 1,
            attacker_team: PlayerTeam::Red,
            attacker_conditions: ConditionState::default(),
            source: DamageSourceKind::Player,
            weapon_position: Some(LoadoutPosition::Primary),
            victim: 2,
            victim_team: PlayerTeam::Blue,
            base_damage: 100.0,
            range_multiplier: 1.0,
            damage_type: DamageType::BULLET,
            custom: CustomDamage::None,
            crit: CritKind::None,
            friendly_fire: false,
            force_friendly_fire: false,
            bypass_invulnerability: false,
            force: [1.0, 2.0, 3.0],
        }
    }

    #[test]
    fn damage_history_windows_quantization_merge_and_limit_are_exact() {
        let mut history = DamageHistory::default();
        history
            .record(DamageHistoryInput {
                now: 0.0,
                damage: 300.0,
                bonus_damage: 100.0,
                victim_previous_health: 500,
                lethal: false,
                damage_type: DamageType::BULLET,
                counts_toward_crit_rate: true,
            })
            .unwrap();
        assert_eq!(history.crit_multiplier(1.99).unwrap(), 1.0);
        assert!(history.crit_multiplier(2.0).unwrap() > 2.0);
        history
            .record(DamageHistoryInput {
                now: 3.0,
                damage: 50.0,
                bonus_damage: 0.0,
                victim_previous_health: 50,
                lethal: true,
                damage_type: DamageType::BLAST,
                counts_toward_crit_rate: true,
            })
            .unwrap();
        history
            .record(DamageHistoryInput {
                now: 3.05,
                damage: 40.0,
                bonus_damage: 0.0,
                victim_previous_health: 40,
                lethal: true,
                damage_type: DamageType::BLAST,
                counts_toward_crit_rate: true,
            })
            .unwrap();
        assert_eq!(history.events().len(), 2);
        assert_eq!(history.events()[1].kills, 2);
        for index in 0..130 {
            history
                .record(DamageHistoryInput {
                    now: 5.0 + index as f32,
                    damage: 1.0,
                    bonus_damage: 0.0,
                    victim_previous_health: 10,
                    lethal: false,
                    damage_type: DamageType::BULLET,
                    counts_toward_crit_rate: true,
                })
                .unwrap();
        }
        assert_eq!(history.events().len(), 128);
    }

    #[test]
    fn single_rapid_and_melee_crit_branches_consume_bucket_exactly() {
        let mut state = CritState::default();
        let single = state
            .check(CritCheckInput {
                now: 0.0,
                kind: WeaponCritKind::SingleShot,
                random_crits_enabled: true,
                can_fire_critical: true,
                guaranteed_critical: false,
                raw_damage: 20.0,
                projectiles_per_shot: 1.0,
                fire_delay: 0.5,
                player_crit_multiplier: 1.0,
                chance_multiplier: 1.0,
                roll: Some(0),
            })
            .unwrap();
        assert_eq!(single.kind, CritKind::Full);
        assert_eq!(single.threshold, 200.0);
        assert_eq!(state.token_bucket, 140.0);

        let guaranteed = state
            .check(CritCheckInput {
                guaranteed_critical: true,
                roll: None,
                ..CritCheckInput {
                    now: 0.1,
                    kind: WeaponCritKind::SingleShot,
                    random_crits_enabled: true,
                    can_fire_critical: true,
                    guaranteed_critical: false,
                    raw_damage: 100.0,
                    projectiles_per_shot: 1.0,
                    fire_delay: 1.0,
                    player_crit_multiplier: 1.0,
                    chance_multiplier: 1.0,
                    roll: Some(0),
                }
            })
            .unwrap();
        assert_eq!(guaranteed.kind, CritKind::Full);
        assert_eq!(state.token_bucket, 140.0);

        let mut rapid = CritState::default();
        let result = rapid
            .check(CritCheckInput {
                now: 0.0,
                kind: WeaponCritKind::RapidFire,
                random_crits_enabled: true,
                can_fire_critical: true,
                guaranteed_critical: false,
                raw_damage: 10.0,
                projectiles_per_shot: 1.0,
                fire_delay: 0.1,
                player_crit_multiplier: 1.0,
                chance_multiplier: 1.0,
                roll: Some(0),
            })
            .unwrap();
        assert_eq!(result.kind, CritKind::None);
        assert!(result.denied_by_bucket);
        assert!(
            rapid
                .check(CritCheckInput {
                    now: 0.5,
                    roll: None,
                    ..CritCheckInput {
                        now: 0.0,
                        kind: WeaponCritKind::RapidFire,
                        random_crits_enabled: true,
                        can_fire_critical: true,
                        guaranteed_critical: false,
                        raw_damage: 10.0,
                        projectiles_per_shot: 1.0,
                        fire_delay: 0.1,
                        player_crit_multiplier: 1.0,
                        chance_multiplier: 1.0,
                        roll: Some(0),
                    }
                })
                .is_ok()
        );

        let mut melee = CritState::default();
        let result = melee
            .check(CritCheckInput {
                now: 0.0,
                kind: WeaponCritKind::Melee,
                random_crits_enabled: true,
                can_fire_critical: true,
                guaranteed_critical: false,
                raw_damage: 65.0,
                projectiles_per_shot: 1.0,
                fire_delay: 0.8,
                player_crit_multiplier: 1.0,
                chance_multiplier: 1.0,
                roll: Some(1499),
            })
            .unwrap();
        assert_eq!(result.kind, CritKind::Full);
        assert_eq!(melee.token_bucket, 267.5);
    }

    #[test]
    fn admission_invulnerability_phase_and_typed_immunity_preserve_atomic_health() {
        for (victim_conditions, denial) in [
            (
                conditions(&[ConditionId::INVULNERABLE]),
                DamageDenial::Invulnerable,
            ),
            (conditions(&[ConditionId::PHASE]), DamageDenial::Phased),
            (
                conditions(&[ConditionId::BULLET_IMMUNE]),
                DamageDenial::TypedImmunity,
            ),
            (conditions(&[ConditionId::GHOST]), DamageDenial::Ghost),
        ] {
            let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
            let mut conditions = victim_conditions;
            let result = apply_damage(
                true,
                &mut health,
                &mut conditions,
                &input(),
                DamageModifiers::default(),
            )
            .unwrap();
            assert_eq!(result.denial, Some(denial));
            assert_eq!(health.current, 200);
        }
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut state = ConditionState::default();
        let mut friendly = input();
        friendly.victim_team = PlayerTeam::Red;
        assert_eq!(
            apply_damage(
                true,
                &mut health,
                &mut state,
                &friendly,
                DamageModifiers::default()
            )
            .unwrap()
            .denial,
            Some(DamageDenial::FriendlyFire)
        );
    }

    #[test]
    fn forced_critical_falloff_never_adds_close_range_ramp_or_variance_bonus() {
        for (range, crit, expected) in [(0.528, CritKind::Full, 54), (1.5, CritKind::Full, 102), (0.528, CritKind::Mini, 24)] {
            let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
            let mut info = input(); info.base_damage = 34.0; info.range_multiplier = range; info.crit = crit;
            let result = apply_damage(true, &mut health, &mut ConditionState::default(), &info,
                DamageModifiers { critical_falloff: true, ..Default::default() }).unwrap();
            assert_eq!(result.health_damage, expected);
            assert!((result.pre_resistance_bonus_damage - result.pre_resistance_base_damage * if crit == CritKind::Full { 2.0 } else { 0.35 }).abs() < 0.0001);
        }
    }

    #[test]
    fn spunup_resistance_tests_post_hit_health_but_only_scales_base_damage() {
        for (before, pierces, expected) in [(241, false, 90), (240, false, 84), (100, false, 84), (100, true, 90)] {
            let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
            health.current = before;
            let mut info = input();
            info.base_damage = 30.0;
            info.crit = CritKind::Full;
            let result = apply_damage(true, &mut health, &mut ConditionState::default(), &info,
                DamageModifiers { spunup_taken: 0.8, pierces_resists: pierces, ..Default::default() }).unwrap();
            assert_eq!(result.health_damage, expected);
            assert_eq!(result.bonus_damage, 60.0);
        }
    }

    #[test]
    fn crit_defense_resistance_and_integer_health_order_is_exact() {
        let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
        let mut state = conditions(&[ConditionId::MARKED_FOR_DEATH, ConditionId::DEFENSE_BUFF]);
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &input(),
            DamageModifiers::default(),
        )
        .unwrap();
        assert_eq!(result.crit, CritKind::None);
        assert_eq!(result.health_damage, 65);
        assert_eq!(health.current, 235);

        let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
        let mut state = ConditionState::default();
        let mut critical = input();
        critical.crit = CritKind::Full;
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &critical,
            DamageModifiers {
                critical_bonus_taken: 0.5,
                bullet_taken: 0.8,
                general_taken: 1.1,
                ..DamageModifiers::default()
            },
        )
        .unwrap();
        assert_eq!(result.base_damage, 80.0);
        assert_eq!(result.bonus_damage, 100.0);
        assert_eq!(result.health_damage, 198);
        assert_eq!(health.current, 102);
    }

    #[test]
    fn prevent_death_and_death_attribution_corpse_branches_are_exact() {
        let mut health = HealthState::spawn(PlayerClass::Scout, 0.0, 0.0).unwrap();
        let mut state = conditions(&[ConditionId::PREVENT_DEATH]);
        let mut lethal = input();
        lethal.base_damage = 200.0;
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &lethal,
            DamageModifiers::default(),
        )
        .unwrap();
        assert_eq!(health.current, 1);
        assert!(result.death.is_none());
        assert!(!state.contains(ConditionId::PREVENT_DEATH));

        let mut health = HealthState::spawn(PlayerClass::Scout, 0.0, 0.0).unwrap();
        let mut state = ConditionState::default();
        state
            .add(
                ConditionId::URINE,
                ConditionDuration::Permanent,
                Some(3),
                true,
                false,
            )
            .unwrap();
        lethal.damage_type = DamageType::BLAST;
        lethal.crit = CritKind::Full;
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &lethal,
            DamageModifiers::default(),
        )
        .unwrap();
        assert_eq!(result.death.as_ref().unwrap().scorer, Some(1));
        assert_eq!(result.death.as_ref().unwrap().assister, Some(3));
        assert_eq!(result.death.as_ref().unwrap().corpse, CorpseKind::Gib);
    }

    #[test]
    fn damage_time_boosts_do_not_retroactively_crit_and_self_hits_skip_range_and_crit_bonus() {
        let mut hit = input();
        hit.attacker_conditions = conditions(&[ConditionId::CRIT_BOOSTED]);
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let result = apply_damage(true, &mut health, &mut ConditionState::default(), &hit, DamageModifiers::default()).unwrap();
        assert_eq!(result.crit, CritKind::None);
        hit.attacker = hit.victim;
        hit.attacker_team = hit.victim_team;
        hit.crit = CritKind::Full;
        hit.range_multiplier = 0.5;
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let result = apply_damage(true, &mut health, &mut ConditionState::default(), &hit, DamageModifiers::default()).unwrap();
        assert_eq!(result.final_damage, hit.base_damage);
        assert_eq!(result.bonus_damage, 0.0);
    }

    #[test]
    fn minicrit_bonus_accounts_for_range_variance_and_full_crit_demotion_does_not_restore_ramp() {
        for (crit, range, demote, expected, base, bonus) in [
            (CritKind::Full, 0.5, false, 270.0, 45.0, 225.0),
            (CritKind::Full, 1.25, false, 270.0, 90.0, 180.0),
            (CritKind::Mini, 0.5, false, 121.5, 45.0, 76.5),
            (CritKind::Mini, 1.25, false, 151.875, 90.0, 61.875),
            (CritKind::Full, 1.25, true, 121.5, 90.0, 31.5),
        ] {
            let mut hit = input();
            hit.base_damage = 90.0;
            hit.crit = crit;
            hit.range_multiplier = range;
            let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
            let result = apply_damage(true, &mut health, &mut ConditionState::default(), &hit,
                DamageModifiers { crits_become_minicrits: demote, ..Default::default() }).unwrap();
            assert!((result.final_damage - expected).abs() < 0.0001);
            assert!((result.base_damage - base).abs() < 0.0001);
            assert!((result.bonus_damage - bonus).abs() < 0.0001);
        }
        assert_eq!(DamageType::SLASH.source_bits(CritKind::None), 4);
        assert_eq!(DamageType::BLAST.source_bits(CritKind::Mini), 64 | (1 << 20));
    }

    #[test]
    fn crit_threshold_observed_rate_and_no_random_states_cover_boundaries() {
        let base = CritCheckInput {
            now: 0.0,
            kind: WeaponCritKind::SingleShot,
            random_crits_enabled: true,
            can_fire_critical: true,
            guaranteed_critical: false,
            raw_damage: 1.0,
            projectiles_per_shot: 1.0,
            fire_delay: 1.0,
            player_crit_multiplier: 1.0,
            chance_multiplier: 1.0,
            roll: Some(199),
        };
        let mut granted = CritState::default();
        assert_eq!(granted.check(base).unwrap().kind, CritKind::Full);
        let mut boundary = CritState::default();
        assert_eq!(
            boundary
                .check(CritCheckInput {
                    roll: Some(200),
                    ..base
                })
                .unwrap()
                .kind,
            CritKind::None
        );
        let mut observed = CritState {
            random_ranged_crit_damage: 300,
            total_ranged_damage: 300,
            ..CritState::default()
        };
        assert!(
            observed
                .check(CritCheckInput {
                    roll: Some(0),
                    ..base
                })
                .unwrap()
                .denied_by_observed_rate
        );
        let mut disabled = CritState::default();
        assert_eq!(
            disabled
                .check(CritCheckInput {
                    random_crits_enabled: false,
                    roll: None,
                    ..base
                })
                .unwrap()
                .kind,
            CritKind::None
        );
        assert_eq!(disabled.token_bucket, CRIT_BUCKET_DEFAULT);
    }

    #[test]
    fn self_world_friendly_force_and_overlapping_type_precedence_are_exact() {
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut state = ConditionState::default();
        let mut self_hit = input();
        self_hit.attacker = self_hit.victim;
        self_hit.attacker_team = self_hit.victim_team;
        assert!(
            apply_damage(
                true,
                &mut health,
                &mut state,
                &self_hit,
                DamageModifiers::default()
            )
            .unwrap()
            .admitted
        );

        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut state = ConditionState::default();
        let mut friendly = input();
        friendly.victim_team = friendly.attacker_team;
        friendly.force_friendly_fire = true;
        assert!(
            apply_damage(
                true,
                &mut health,
                &mut state,
                &friendly,
                DamageModifiers::default()
            )
            .unwrap()
            .admitted
        );

        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut state = conditions(&[ConditionId::BULLET_IMMUNE]);
        let mut overlapping = input();
        overlapping.damage_type = DamageType::IGNITE | DamageType::BULLET;
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &overlapping,
            DamageModifiers {
                fire_taken: 0.5,
                bullet_taken: 0.5,
                ..DamageModifiers::default()
            },
        )
        .unwrap();
        assert!(result.admitted);
        assert_eq!(result.health_damage, 25);

        let mut world = input();
        world.source = DamageSourceKind::World;
        world.weapon_position = None;
        world.attacker = 0;
        world.attacker_team = PlayerTeam::Unassigned;
        let mut health = HealthState::spawn(PlayerClass::Soldier, 0.0, 0.0).unwrap();
        let mut state = ConditionState::default();
        assert!(
            apply_damage(
                true,
                &mut health,
                &mut state,
                &world,
                DamageModifiers::default()
            )
            .unwrap()
            .admitted
        );
    }

    #[test]
    fn defense_high_pierce_and_backstab_exclusions_are_distinct() {
        let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
        let mut state = conditions(&[ConditionId::DEFENSE_BUFF_HIGH]);
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &input(),
            DamageModifiers::default(),
        )
        .unwrap();
        assert_eq!(result.health_damage, 25);

        let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
        let mut state = conditions(&[ConditionId::DEFENSE_BUFF_HIGH]);
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &input(),
            DamageModifiers {
                pierces_resists: true,
                ..DamageModifiers::default()
            },
        )
        .unwrap();
        assert_eq!(result.health_damage, 100);

        let mut health = HealthState::spawn(PlayerClass::Heavy, 0.0, 0.0).unwrap();
        let mut state = conditions(&[ConditionId::DEFENSE_BUFF]);
        let result = apply_damage(
            true,
            &mut health,
            &mut state,
            &DamageInput {
                custom: CustomDamage::Backstab,
                crit: CritKind::Full,
                ..input()
            },
            DamageModifiers::default(),
        )
        .unwrap();
        assert_eq!(result.health_damage, 300);
    }
}
