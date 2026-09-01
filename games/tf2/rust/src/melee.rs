//! Melee effects shared by the local session and its combat actors.
//! Ordering follows the SDK melee weapon and player shared-condition paths.

use crate::{Weapon, damage::CritKind};
use std::collections::{BTreeMap, BTreeSet};

pub const BLEED_DAMAGE: f32 = 4.0;
pub const BLEED_INTERVAL: f32 = 0.5;
pub const MARK_DURATION: f32 = 15.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Actor {
    pub class: crate::PlayerClass,
    pub team: crate::PlayerTeam,
    pub position: [f32; 3],
    pub eye: [f32; 3],
    pub center: [f32; 3],
    pub hull: playsrc_collision::Hull,
    pub health: i32,
    pub maximum_health: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct Swing {
    pub pending: bool,
    pub weapon: Weapon,
    pub source: crate::weapon::WeaponSource,
    pub crit: CritKind,
    pub potential_victims: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bleed {
    pub attacker: u32,
    pub weapon: Weapon,
    pub source_weapon: Option<crate::weapon::WeaponSource>,
    pub next_damage: f32,
    pub expires: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub swings: BTreeMap<u32, Swing>,
    pub bleeds: BTreeMap<u32, Vec<Bleed>>,
    pub marked_targets: BTreeMap<u32, u32>,
    pub local_mark_remaining: Option<f32>,
    presentation_actors: BTreeSet<u32>,
    presented: BTreeMap<u32, bool>,
}

impl Default for State {
    fn default() -> Self {
        Self { swings: BTreeMap::new(), bleeds: BTreeMap::new(),
            marked_targets: BTreeMap::new(), local_mark_remaining: None,
            presentation_actors: BTreeSet::new(), presented: BTreeMap::new() }
    }
}

impl State {
    pub fn bleed(&mut self, victim: u32, attacker: u32, weapon: Weapon, source_weapon: Option<crate::weapon::WeaponSource>, now: f32, duration: f32) {
        let entries = self.bleeds.entry(victim).or_default();
        let expires = now + duration;
        // The SDK extends only a strictly later expiration. An equal or earlier
        // expiration adds a new source; do not silently normalize that behavior.
        for bleed in entries.iter_mut() {
            if bleed.attacker == attacker && bleed.source_weapon.is_some() && bleed.source_weapon == source_weapon && expires > bleed.expires {
                bleed.expires = expires;
                return;
            }
        }
        entries.push(Bleed { attacker, weapon, source_weapon, next_damage: duration, expires });
    }

    pub fn reset_victim(&mut self, victim: u32) {
        self.bleeds.remove(&victim);
        self.swings.remove(&victim);
        if victim == crate::PLAYER_IDENTITY {
            self.local_mark_remaining = None;
        }
    }
}

pub fn damage(base: f32, health: i32, maximum: i32, mut attribute: impl FnMut(&str, f32) -> f32) -> f32 {
    let mut damage = attribute("mult_dmg", base);
    damage = attribute(if (health as f32) < maximum as f32 * 0.5 {
        "mult_dmg_bonus_while_half_dead"
    } else { "mult_dmg_penalty_while_half_alive" }, damage);
    let reduced_health = attribute("mult_dmg_with_reduced_health", 1.0);
    if reduced_health != 1.0 {
        let fraction = (health as f32 / maximum as f32).clamp(0.0, 1.0);
        damage *= reduced_health + fraction * (1.0 - reduced_health);
    }
    damage
}

pub fn impact_force(amount: f32, direction: [f32; 3], scale: f32) -> Result<[f32; 3], playsrc_physics::SourceVectorError> {
    if amount.abs() < 1.0 { return Ok([0.0; 3]); }
    let direction = playsrc_physics::normalize_source_vector(direction)?;
    Ok(crate::scale(crate::scale(direction, amount * (75.0 * 4.0)), 1.0 / amount * scale))
}

pub fn player_push(size: [f32; 3], amount: f32, self_damage: bool) -> f32 {
    let height = if self_damage && size[2] == 62.0 { 55.0 } else { size[2] };
    (amount * ((48.0 * 48.0 * 82.0) / (size[0] * size[1] * height)) * if self_damage { 9.0 } else { 6.0 }).min(1000.0)
}

impl<W: crate::GameplayWorld + Clone> crate::Session<W> {
    pub(crate) fn melee_actor(&self, identity: u32) -> Option<Actor> {
        if identity != crate::PLAYER_IDENTITY { return self.bots.as_ref()?.melee_actor(identity); }
        if self.health <= 0 || self.lifecycle != crate::PlayerLifecycle::Active { return None; }
        let hull = self.movement.active_hull(crate::MovementPolicy { class: self.class, modifiers: self.movement_modifiers }.resolve());
        Some(Actor { class: self.class, team: self.team_selection.local_team(), position: self.movement.position,
            eye: crate::add(self.movement.position, self.movement.view_offset),
            center: crate::add(self.movement.position, crate::scale(crate::add(hull.mins, hull.maxs), 0.5)),
            hull, health: self.health, maximum_health: self.maximum_health() })
    }

    pub(crate) fn trace_melee_scene(&mut self, owner: u32, start: [f32; 3], end: [f32; 3], hull: playsrc_collision::Hull, ignore_players: bool)
        -> Result<(playsrc_movement::Trace, Option<crate::hitscan::DamageTarget>), crate::Error> {
        use crate::entity_queries::Entity;
        self.sync_bot_queries()?;
        self.flush_entity_queries()?;
        let world = self.collision.trace_world(start, end, hull, crate::MASK_SOLID)?;
        if world.start_solid { return Ok((world, None)); }
        let mut selected = None;
        let mut impact = playsrc_movement::Trace { fraction: 1.0, start_solid: false, all_solid: false, end: world.end, normal: None, hit: None, contents: 0 };
        for entity in self.entity_queries.sweep(start, world.end, hull, 1, usize::MAX, |entity|
            entity != Entity::Actor(owner) && (!ignore_players || !matches!(entity, Entity::Actor(_)))).map_err(crate::Error::ProjectileTrace)? {
            if let Entity::Projectile(identity) = entity {
                let projectile = self.projectiles.iter().find(|projectile| projectile.presentation.identity == identity).ok_or(crate::Error::InvalidProjectilePhysics)?;
                if !projectile.presentation.kind.uses_rigid_physics() && projectile.presentation.owner_identity == owner { continue; }
            }
            let (candidate, target) = self.trace_query_entity(entity, start, world.end, hull, crate::MASK_SOLID)?;
            if candidate.all_solid || candidate.start_solid || candidate.fraction < impact.fraction {
                let start_solid = impact.start_solid || candidate.start_solid;
                impact = candidate;
                impact.start_solid = start_solid;
                selected = target;
            }
            if impact.all_solid { break; }
        }
        if impact.hit.is_none() && !impact.start_solid && !impact.all_solid { return Ok((world, None)); }
        impact.fraction *= world.fraction;
        impact.end = crate::add(start, crate::scale(crate::sub(end, start), impact.fraction));
        Ok((impact, selected))
    }

    pub(crate) fn melee_hull_intersection(&mut self, owner: u32, origin: [f32; 3], mut impact: playsrc_movement::Trace, mut selected: Option<crate::hitscan::DamageTarget>) -> Result<(playsrc_movement::Trace, Option<crate::hitscan::DamageTarget>), crate::Error> {
        let end = crate::add(origin, crate::scale(crate::sub(impact.end, origin), 2.0));
        let hull = playsrc_collision::Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let (center, target) = self.trace_melee_scene(owner, origin, end, hull, false)?;
        if center.fraction < 1.0 { return Ok((center, target)); }
        let mut nearest = 1_000_000.0;
        for x in [-24.0, 24.0] { for y in [-24.0, 24.0] { for z in [0.0, 62.0] {
            let (candidate, target) = self.trace_melee_scene(owner, origin, crate::add(end, [x, y, z]), hull, false)?;
            if candidate.fraction < 1.0 {
                let distance = crate::length(crate::sub(candidate.end, origin));
                if distance < nearest { nearest = distance; impact = candidate; selected = target; }
            }
        } } }
        Ok((impact, selected))
    }

    pub(crate) fn advance_bleeding(&mut self, events: &mut Vec<crate::Event>) -> Result<(), crate::Error> {
        use crate::{PLAYER_IDENTITY, Condition, condition::ConditionId, damage::{DamageType, CustomDamage, DamageModifiers}, bot::Damage};
        let now = self.tick as f32 * self.movement_configuration.tick_interval;
        let victims: Vec<_> = self.melee.bleeds.keys().copied().collect();
        for victim in victims {
            let alive = if victim == PLAYER_IDENTITY { self.health > 0 } else { self.bots.as_ref().is_some_and(|bots| bots.active(victim)) };
            if !alive || !self.has_combat_condition(victim, ConditionId::BLEEDING)
                || self.has_combat_condition(victim, ConditionId::INVULNERABLE) {
                self.melee.bleeds.remove(&victim);
                self.remove_melee_condition(victim, Condition::Bleeding);
                continue;
            }
            let healed = self.has_combat_condition(victim, ConditionId::HEALTH_BUFF);
            let count = self.melee.bleeds[&victim].len();
            for index in (0..count).rev() {
                let entries = self.melee.bleeds.get_mut(&victim).unwrap();
                if healed { entries[index].expires -= 2.0 * self.movement_configuration.tick_interval; }
                if now >= entries[index].expires { entries.swap_remove(index); continue; }
                if now < entries[index].next_damage { continue; }
                entries[index].next_damage = now + BLEED_INTERVAL;
                let bleed = entries[index];
                let team = if bleed.attacker == PLAYER_IDENTITY { self.team_selection.local_team() }
                    else { self.bots.as_ref().and_then(|bots| bots.team(bleed.attacker)).unwrap_or(crate::PlayerTeam::Unassigned) };
                let position = if victim == PLAYER_IDENTITY { self.movement.position }
                    else { self.bots.as_ref().and_then(|bots| bots.combat_player(victim)).map_or([0.0; 3], |facts| facts.world_center) };
                let result = self.apply_actor_damage(Damage { attacker: bleed.attacker, victim, weapon: bleed.weapon,
                    source_weapon: bleed.source_weapon,
                    amount: BLEED_DAMAGE, position, force: [0.0; 3], crit: CritKind::None, range_multiplier: 1.0,
                    custom: CustomDamage::Bleeding, damage_type: DamageType::SLASH,
                    modifiers: DamageModifiers::default(), killing_weapon: Some("bleed_kill") }, team, events)?;
                if let Some(result) = &result && result.admitted {
                    self.melee_push(bleed.attacker, victim, result.pre_resistance_base_damage + result.pre_resistance_bonus_damage)?;
                }
                if result.is_some_and(|result| result.death.is_some()) {
                    self.melee.bleeds.get_mut(&victim).unwrap().clear();
                    break;
                }
            }
            if self.melee.bleeds.get(&victim).is_some_and(Vec::is_empty) {
                self.melee.bleeds.remove(&victim);
                self.remove_melee_condition(victim, Condition::Bleeding);
            }
        }
        Ok(())
    }

    pub(crate) fn melee_damage(&mut self, owner: u32, weapon: Weapon, base: f32) -> f32 {
        let actor = self.melee_actor(owner).expect("live melee owner");
        damage(base, actor.health, actor.maximum_health, |hook, value| {
            self.equipped_weapon_attribute(owner, weapon, hook, value)
        })
    }

    pub(crate) fn has_combat_condition(&self, identity: u32, condition: crate::condition::ConditionId) -> bool {
        if identity == crate::PLAYER_IDENTITY {
            let value = condition.value() as usize;
            self.conditions.words()[value / 32] & (1 << (value % 32)) != 0
        } else {
            self.bots.as_ref().is_some_and(|bots| bots.has_condition(identity, condition))
        }
    }

    pub(crate) fn add_melee_condition(&mut self, victim: u32, condition: crate::Condition, duration: f32, provider: u32) {
        if condition == crate::Condition::MarkedForDeath { self.melee.presentation_actors.insert(victim); }
        if victim == crate::PLAYER_IDENTITY {
            if self.health <= 0 { return; }
            self.conditions.insert(condition);
            let until = match condition {
                crate::Condition::MarkedForDeath => &mut self.melee.local_mark_remaining,
                crate::Condition::Bleeding => return,
                _ => unreachable!("melee-owned condition"),
            };
            *until = Some(until.unwrap_or(0.0).max(duration));
        } else if let Some((health, conditions)) = self.bots.as_mut().and_then(|bots| bots.patient_state(victim)) {
            let duration = if condition == crate::Condition::Bleeding { crate::condition::ConditionDuration::Permanent }
                else { crate::condition::ConditionDuration::Finite(duration) };
            conditions.add(crate::condition::ConditionId::new(condition as u8).unwrap(), duration, Some(provider), health.current > 0, false).unwrap();
        }
    }

    pub(crate) fn remove_melee_condition(&mut self, victim: u32, condition: crate::Condition) {
        if condition == crate::Condition::Bleeding { self.melee.bleeds.remove(&victim); }
        if victim == crate::PLAYER_IDENTITY {
            self.conditions.remove(condition);
            if condition == crate::Condition::MarkedForDeath { self.melee.local_mark_remaining = None; }
        } else if let Some((_, conditions)) = self.bots.as_mut().and_then(|bots| bots.patient_state(victim)) {
            conditions.remove(crate::condition::ConditionId::new(condition as u8).unwrap(), false);
        }
    }

    pub(crate) fn melee_hit_effects(&mut self, attacker: u32, victim: u32, weapon: Weapon, result: &crate::damage::DamageResult) -> Result<(), crate::Error> {
        if !result.admitted { return Ok(()); }
        self.melee_push(attacker, victim, result.pre_resistance_base_damage + result.pre_resistance_bonus_damage)?;
        let now = self.tick as f32 * self.movement_configuration.tick_interval;
        let duration = self.equipped_weapon_attribute(attacker, weapon, "bleeding_duration", 0.0);
        if duration > 0.0 && result.health_after > 0 {
            self.melee.bleed(victim, attacker, weapon, self.weapon_source(attacker, weapon), now, duration);
            self.add_melee_condition(victim, crate::Condition::Bleeding, duration, attacker);
        }
        if attacker != victim && self.equipped_weapon_attribute(attacker, weapon, "mark_for_death", 0.0) != 0.0 {
            if let Some(previous) = self.melee.marked_targets.get(&attacker).copied() {
                self.remove_melee_condition(previous, crate::Condition::MarkedForDeath);
            }
            self.add_melee_condition(victim, crate::Condition::MarkedForDeath, MARK_DURATION, attacker);
            self.melee.marked_targets.insert(attacker, victim);
        }
        if attacker != victim && result.death.is_some() {
            let amount = self.equipped_weapon_attribute(attacker, weapon, "heal_on_kill", 0.0).round_ties_even() as i32;
            if attacker == crate::PLAYER_IDENTITY && amount != 0 && self.health > 0 && self.health < self.maximum_health() {
                // TakeHealth(DMG_GENERIC) clamps at normal health, even though
                // the caller computes an allowance against max buffed health.
                self.health = (self.health + amount).min(self.maximum_health());
            } else if attacker != crate::PLAYER_IDENTITY && amount != 0
                && let Some((health, conditions)) = self.bots.as_mut().and_then(|bots| bots.patient_state(attacker)) {
                health.take_health(amount as f32, false, 1.0, conditions).map_err(|_| crate::Error::Bot(crate::bot::Error::InvalidEntity))?;
            }
        }
        Ok(())
    }

    fn melee_push(&mut self, attacker: u32, victim: u32, amount: f32) -> Result<(), crate::Error> {
        if self.has_combat_condition(victim, crate::condition::ConditionId::DISGUISED) { return Ok(()); }
        if attacker == victim && let Some(actor) = self.melee_actor(victim) && actor.class != crate::PlayerClass::Soldier {
            let impulse = player_push(crate::sub(actor.hull.maxs, actor.hull.mins), amount, true);
            if victim == crate::PLAYER_IDENTITY { self.movement.velocity[2] += impulse; }
            else if let Some(bots) = &mut self.bots { bots.apply_damage_impulse(victim, [0.0, 0.0, impulse], false); }
        } else {
            let origin = if attacker == crate::PLAYER_IDENTITY {
                let hull = self.movement.active_hull(crate::MovementPolicy { class: self.class, modifiers: self.movement_modifiers }.resolve());
                crate::add(self.movement.position, crate::scale(crate::add(hull.mins, hull.maxs), 0.5))
            }
                else { self.bots.as_ref().and_then(|bots| bots.combat_player(attacker)).map_or([0.0; 3], |facts| facts.world_center) };
            self.apply_actor_weapon_push(victim, origin, amount, None)?;
        }
        Ok(())
    }

    pub(crate) fn advance_melee_conditions(&mut self) {
        let active = self.lifecycle == crate::PlayerLifecycle::Active;
        let bots = &self.bots;
        self.melee.swings.retain(|owner, _| if *owner == crate::PLAYER_IDENTITY { active } else { bots.as_ref().is_some_and(|bots| bots.active(*owner)) });
        self.melee.marked_targets.retain(|owner, _| *owner == crate::PLAYER_IDENTITY || bots.as_ref().is_some_and(|bots| bots.contains(*owner)));
        for (condition, expiry) in [(crate::Condition::MarkedForDeath, &mut self.melee.local_mark_remaining)] {
            if let Some(remaining) = expiry { *remaining = (*remaining - self.movement_configuration.tick_interval).max(0.0); }
            if *expiry == Some(0.0) {
                *expiry = None;
                self.conditions.remove(condition);
            }
        }
    }

    pub(crate) fn publish_melee_condition_audio(&mut self) {
        use crate::{PLAYER_IDENTITY, condition::ConditionId, SoundDefinition, RandomContext, SoundQueryPhase, AudioEvent, AudioAction, AudioEventIdentity, AudioSourceKind};
        let actors: Vec<_> = self.melee.presentation_actors.iter().copied().collect();
        for actor in actors {
            let position = if actor == PLAYER_IDENTITY { Some(self.movement.position) } else { self.bots.as_ref().and_then(|bots| bots.position(actor)) };
            let Some(position) = position else { self.melee.presentation_actors.remove(&actor); self.melee.presented.remove(&actor); continue; };
            let current = self.has_combat_condition(actor, ConditionId::MARKED_FOR_DEATH);
            let previous = self.melee.presented.insert(actor, current).unwrap_or(false);
            let mut names = [None];
            if current && !previous {
                names[0] = if actor == PLAYER_IDENTITY { Some("Weapon_Marked_for_Death.Indicator") }
                    else if ![ConditionId::STEALTHED, ConditionId::STEALTHED_USER, ConditionId::STEALTHED_USER_FADING, ConditionId::DISGUISED].into_iter().any(|condition| self.has_combat_condition(actor, condition)) { Some("Weapon_Marked_for_Death.Initial") } else { None };
            }
            for name in names.into_iter().flatten() {
                let Some(definition) = SoundDefinition::configured(name) else {
                    assert!(crate::audio::MISSING_CONFIGURED_SOUNDS.contains(&name), "condition sound was not resolved against the configured manifest");
                    continue;
                };
                let samples = self.sample_sound(RandomContext::PredictedPresentation, definition, SoundQueryPhase::Emit);
                self.push_audio_event(AudioEvent { action: AudioAction::Play, tick: self.tick, ordinal: 0,
                    identity: AudioEventIdentity::PlayerFeedback, definition, source_kind: AudioSourceKind::Entity,
                    source_identity: actor, owner_identity: Some(actor), position, samples });
            }
            if !current { self.melee.presentation_actors.remove(&actor); self.melee.presented.remove(&actor); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shahanshah_half_health_boundary_is_strict_and_attributes_are_ordered() {
        for (health, expected) in [(63, 48.75), (62, 81.25), (125, 48.75)] {
            assert_eq!(damage(65.0, health, 125, |hook, value| value * match hook {
                "mult_dmg_bonus_while_half_dead" => 1.25,
                "mult_dmg_penalty_while_half_alive" => 0.75,
                _ => 1.0,
            }), expected);
        }
    }

    #[test]
    fn bleed_source_extension_keeps_phase_and_equal_expiry_adds_source() {
        let mut state = State::default();
        let source = Some(crate::weapon::WeaponSource { owner: 1, definition_index: 325, generation: 1 });
        state.bleed(2, 1, Weapon::Bat, source, 20.0, 5.0);
        assert_eq!(state.bleeds[&2][0].next_damage, 5.0);
        state.bleeds.get_mut(&2).unwrap()[0].next_damage = 20.5;
        state.bleed(2, 1, Weapon::Bat, source, 21.0, 5.0);
        assert_eq!(state.bleeds[&2].len(), 1);
        assert_eq!(state.bleeds[&2][0].next_damage, 20.5);
        assert_eq!(state.bleeds[&2][0].expires, 26.0);
        state.bleed(2, 1, Weapon::Bat, source, 21.0, 5.0);
        assert_eq!(state.bleeds[&2].len(), 2);
        state.bleed(2, 3, Weapon::Kukri, None, 21.0, 6.0);
        assert_eq!(state.bleeds[&2].len(), 3);
        state.reset_victim(1);
        assert_eq!(state.bleeds[&2].len(), 3, "attacker death does not cure victims");
        state.reset_victim(2);
        assert!(state.bleeds.is_empty());
    }

}
