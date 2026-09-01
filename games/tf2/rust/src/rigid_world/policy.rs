//! Policy for admitted world, static-prop, prop-shadow and stock sticky bodies.
//! NPC, vehicle and freely articulated ragdoll bodies require their own admission.
// Portions adapted from Valve's official Source SDK 2013.
// Copyright Valve Corporation, All rights reserved.
// See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt at the repository root.
use super::BodyOwner;
use playsrc_collision::MASK_SOLID;
use playsrc_physics::CollisionSolver;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BodyRule {
    pub owner: BodyOwner,
    pub entity_handle: Option<playsrc_entity::EntityHandle>,
    pub contents: u32,
    pub movable: bool,
    pub static_body: bool,
    pub shadow: bool,
    pub push: bool,
    pub solid: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PenetrationState {
    Enabled,
    TrySleep,
    Disabled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Entity {
    World,
    Map(playsrc_entity::EntityHandle),
    Follower(u64),
    Projectile(u32),
}
fn entity(identity: u64, rule: &BodyRule) -> Entity {
    match rule.owner {
        BodyOwner::World => Entity::World,
        BodyOwner::MapEntity(_) => {
            Entity::Map(rule.entity_handle.expect("map body has an Entity lifetime"))
        }
        BodyOwner::BoneFollower { .. } => Entity::Follower(identity),
        BodyOwner::Projectile(projectile) => Entity::Projectile(projectile),
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Penetration {
    entities: [Entity; 2],
    started: f32,
    touched: f32,
    state: PenetrationState,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Policy {
    pub bodies: BTreeMap<u64, BodyRule>,
    penetrations: Vec<Penetration>,
    penetrating: BTreeSet<Entity>,
    now: f32,
    paused: bool,
    max_bodies: usize,
    max_events: usize,
    pub rejected: bool,
}
impl Policy {
    pub fn new(max_bodies: usize, max_events: usize) -> Self {
        Self {
            bodies: BTreeMap::new(),
            penetrations: Vec::new(),
            penetrating: BTreeSet::new(),
            now: 0.0,
            paused: true,
            max_bodies,
            max_events,
            rejected: false,
        }
    }
    pub fn begin_frame(&mut self, now: f32) -> bool {
        if !now.is_finite() {
            return false;
        }
        self.now = now;
        self.paused = false;
        true
    }
    pub fn insert(&mut self, body: u64, rule: BodyRule) -> bool {
        if self.bodies.len() >= self.max_bodies || self.bodies.contains_key(&body) {
            return false;
        }
        self.bodies.insert(body, rule);
        true
    }
    pub fn finish_frame(&mut self) -> Vec<u64> {
        let mut sleep = Vec::new();
        for index in (0..self.penetrations.len()).rev() {
            let event = &self.penetrations[index];
            let present = event.entities.map(|owner| {
                self.bodies
                    .iter()
                    .any(|(id, body)| entity(*id, body) == owner)
            });
            if event.state == PenetrationState::TrySleep {
                if present == [true, true] {
                    for owner in event.entities {
                        for (&body, rule) in &self.bodies {
                            if entity(body, rule) == owner && rule.movable {
                                if sleep.len() >= self.max_events.saturating_mul(2) {
                                    self.rejected = true;
                                    return Vec::new();
                                }
                                sleep.push(body);
                            }
                        }
                    }
                    self.penetrations[index].state = PenetrationState::Disabled;
                    continue;
                }
            } else if self.now - event.touched > 1.0 {
                if event.state == PenetrationState::Disabled && present == [true, true] {
                    self.penetrations[index].state = PenetrationState::Enabled;
                    continue;
                }
            } else {
                continue;
            }
            for entity in event.entities {
                self.penetrating.remove(&entity);
            }
            self.penetrations.swap_remove(index);
        }
        sleep
    }
}
impl CollisionSolver for Policy {
    fn should_collide(&self, first: u64, second: u64) -> bool {
        let ids = [first, second];
        let (Some(first), Some(second)) = (self.bodies.get(&first), self.bodies.get(&second))
        else {
            return true;
        };
        if entity(ids[0], first) == entity(ids[1], second) {
            return true;
        }
        if (first.shadow && (!second.movable || second.shadow)) || (second.shadow && !first.movable)
        {
            return false;
        }
        if !first.solid || !second.solid {
            return false;
        }
        if matches!(first.owner, BodyOwner::Projectile(_))
            && matches!(second.owner, BodyOwner::Projectile(_))
        {
            return false;
        }
        first.contents & MASK_SOLID != 0 && second.contents & MASK_SOLID != 0
    }
    fn should_solve_penetration(&mut self, first: u64, second: u64, _: f32) -> bool {
        if self.paused {
            return true;
        }
        let ids = [first, second];
        let (Some(first), Some(second)) = (self.bodies.get(&first), self.bodies.get(&second))
        else {
            self.rejected = true;
            return false;
        };
        let shadow = first.shadow || second.shadow;
        let mut entities = [entity(ids[0], first), entity(ids[1], second)];
        entities.sort();
        let index = if let Some(index) = self
            .penetrations
            .iter()
            .rposition(|event| event.entities == entities)
        {
            index
        } else {
            if self.penetrations.len() >= self.max_events {
                self.rejected = true;
                return false;
            }
            for owner in entities {
                if self
                    .bodies
                    .iter()
                    .any(|(id, body)| entity(*id, body) == owner && !body.static_body)
                {
                    self.penetrating.insert(owner);
                }
            }
            self.penetrations.push(Penetration {
                entities,
                started: self.now,
                touched: self.now,
                state: PenetrationState::Enabled,
            });
            self.penetrations.len() - 1
        };
        let event = &mut self.penetrations[index];
        event.touched = self.now;
        if self.now - event.started > 3.0 {
            event.started = self.now;
            if !shadow {
                event.state = PenetrationState::TrySleep;
                return false;
            }
        }
        true
    }
    fn should_freeze_object(&mut self, body: u64) -> bool {
        match self.bodies.get(&body) {
            Some(body) => !body.push,
            None => {
                self.rejected = true;
                true
            }
        }
    }
    fn additional_collision_checks_this_tick(&mut self, current: i32) -> i32 {
        if current < 1200 { 1200 - current } else { 0 }
    }
    fn should_freeze_contacts(&mut self, _: &[u64]) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> Policy {
        let mut policy = Policy::new(8, 8);
        for (identity, owner, contents, movable, shadow, push) in [
            (1, BodyOwner::World, 1, false, false, false),
            (2, BodyOwner::World, 65536, false, false, false),
            (3, BodyOwner::Projectile(1), 1, true, false, false),
            (4, BodyOwner::Projectile(2), 1, true, false, false),
            (5, BodyOwner::MapEntity(1), 1, true, true, true),
        ] {
            assert!(policy.insert(
                identity,
                BodyRule {
                    owner,
                    entity_handle: matches!(owner, BodyOwner::MapEntity(_)).then_some(
                        playsrc_entity::EntityHandle {
                            slot: 1,
                            generation: 1
                        }
                    ),
                    contents,
                    movable,
                    shadow,
                    push,
                    static_body: !movable,
                    solid: true
                }
            ));
        }
        policy
    }
    #[test]
    fn stock_projectile_world_masks_and_push_exemptions_follow_game_policy() {
        let mut policy = policy();
        assert!(policy.should_collide(1, 3));
        assert!(!policy.should_collide(2, 3));
        assert!(!policy.should_collide(3, 4));
        assert!(!policy.should_collide(1, 5));
        assert!(policy.should_collide(3, 5));
        assert!(!policy.should_freeze_object(5));
        assert!(policy.should_freeze_object(3));
        assert!(!policy.should_freeze_contacts(&[3, 5]));
        assert_eq!(policy.additional_collision_checks_this_tick(250), 950);
        assert_eq!(policy.additional_collision_checks_this_tick(1200), 0);
    }
    #[test]
    fn recreated_map_entities_do_not_reuse_old_penetration_records() {
        let mut policy = policy();
        policy.begin_frame(0.0);
        assert!(policy.should_solve_penetration(3, 5, 0.015));
        let mut replacement = policy.bodies.remove(&5).unwrap();
        replacement.entity_handle.as_mut().unwrap().generation += 1;
        assert!(policy.insert(6, replacement));
        policy.begin_frame(0.5);
        assert!(policy.should_solve_penetration(3, 6, 0.015));
        assert_eq!(policy.penetrations.len(), 2);
        assert_ne!(
            policy.penetrations[0].entities,
            policy.penetrations[1].entities
        );
        assert_eq!(policy.penetrations[1].started, 0.5);
    }
    #[test]
    fn penetration_clock_preserves_strict_threshold_reset_and_disabled_expiry_phase() {
        let mut policy = policy();
        assert!(policy.should_solve_penetration(1, 3, 0.015));
        assert!(policy.penetrations.is_empty());
        policy.begin_frame(0.0);
        assert!(policy.should_solve_penetration(1, 3, 0.015));
        policy.begin_frame(3.0);
        assert!(policy.should_solve_penetration(3, 1, 0.015));
        assert!(policy.finish_frame().is_empty());
        let above = f32::from_bits(3.0_f32.to_bits() + 1);
        policy.begin_frame(above);
        assert!(!policy.should_solve_penetration(1, 3, 0.015));
        assert!(policy.should_solve_penetration(1, 3, 0.015));
        assert_eq!(policy.finish_frame(), [3]);
        policy.begin_frame(above + 1.0);
        policy.finish_frame();
        assert_eq!(policy.penetrations[0].state, PenetrationState::Disabled);
        policy.begin_frame(above + 1.25);
        policy.finish_frame();
        assert_eq!(policy.penetrations[0].state, PenetrationState::Enabled);
        policy.finish_frame();
        assert!(policy.penetrations.is_empty());
        assert!(policy.penetrating.is_empty());
    }
    #[test]
    fn shadow_penetration_never_queues_sleep_and_exhaustion_does_not_grow_state() {
        let mut policy = policy();
        policy.begin_frame(0.0);
        assert!(policy.should_solve_penetration(3, 5, 0.015));
        policy.begin_frame(4.0);
        assert!(policy.should_solve_penetration(3, 5, 0.015));
        assert!(policy.finish_frame().is_empty());
        policy.max_events = 1;
        assert!(!policy.should_solve_penetration(1, 3, 0.015));
        assert!(policy.rejected);
        assert_eq!(policy.penetrations.len(), 1);
    }
}
