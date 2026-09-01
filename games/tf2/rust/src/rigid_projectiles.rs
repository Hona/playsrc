// Portions adapted from Valve's official Source SDK 2013.
// Copyright Valve Corporation, All rights reserved.
// See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt at the repository root.
use super::*;
use crate::rigid_body::RigidModel;
use crate::rigid_world::{BodyOwner, MapBodyInput, MapBodyKind, ProjectileBodyInput, RigidWorld};
use playsrc_physics::PhysicsCallbackKind;
use std::sync::Arc;

struct StickyDamageResponse {
    force: [f32; 3],
    destroy: bool,
    detach: bool,
    apply_force: bool,
}

fn sticky_damage_response(kind: damage::DamageType, mut force: [f32; 3]) -> Option<StickyDamageResponse> {
    use damage::DamageType as D;
    if !kind.intersects(D::BULLET | D::MELEE | D::SONIC | D::BUCKSHOT | D::BLAST) { return None; }
    let mut destroy = false;
    if kind.intersects(D::BULLET | D::MELEE) {
        force = scale(force, 2.0);
        destroy = true;
    }
    if kind.contains(D::SONIC) {
        force = scale(force, 2.0);
    } else if kind.contains(D::BUCKSHOT) {
        force = scale(force, 0.75);
        destroy = true;
    } else if kind.contains(D::BLAST) {
        destroy |= kind.contains(D::IGNITE);
        force = scale(force, 0.15);
    }
    // CMultiplayRules' force mask uses TF2's narrower time-based damage mask.
    let no_force = D::FALL | D::BURN | D::DROWN | D::CRUSH | D::PREVENT_FORCE
        | D::from_source_bits((1 << 15) | (1 << 16) | (1 << 19) | (1 << 23) | (1 << 24));
    Some(StickyDamageResponse { force, destroy, detach: dot(force, force) > 1500.0 * 1500.0,
        apply_force: !kind.intersects(no_force) })
}

fn prop_blocks_los(bounds: Hull) -> bool {
    let size = sub(bounds.maxs, bounds.mins);
    size.into_iter().all(|value| value > 30.0) && size.into_iter().any(|value| value > 40.0)
}

#[cfg(test)]
mod sticky_damage_tests {
    use super::*;
    use damage::DamageType as D;

    #[test]
    fn prop_los_uses_unrotated_collision_sizes_and_strict_thresholds() {
        for (size, expected) in [([41.0, 31.0, 31.0], true), ([40.0, 40.0, 40.0], false),
            ([41.0, 30.0, 31.0], false), ([31.0, 31.0, 41.0], true), ([1000.0, 1000.0, 0.0], false)] {
            assert_eq!(prop_blocks_los(Hull { mins: [-10.0; 3], maxs: size.map(|value| value - 10.0) }), expected);
        }
    }

    #[test]
    fn airblast_visibility_skips_both_endpoints_and_owners_but_not_grenade_throwers() {
        use entity_queries::Entity as Q;
        let mut session = Session::new(World(Arc::new(playsrc_collision::World::empty())), [0.0; 3], MapRuntime::empty(0.015));
        session.sync_local_query();
        session.fire_projectile(0.0, 0.0, 0.0, None, None, &mut Vec::new()).unwrap();
        let target = Q::Projectile(session.projectiles[0].presentation.identity);
        session.entity_queries.set_origin(target, &mut session.projectiles[0].presentation.position, [100.0, 0.0, 64.0]).unwrap();
        assert!(session.airblast_visible([0.0, 0.0, 64.0], target).unwrap());
        session.fire_projectile(0.0, 0.0, 0.0, None, None, &mut Vec::new()).unwrap();
        let blocker = &mut session.projectiles[1].presentation;
        let entity = Q::Projectile(blocker.identity);
        session.entity_queries.set_origin(entity, &mut blocker.position, [50.0, 0.0, 64.0]).unwrap();
        blocker.owner_identity = 30;
        assert!(!session.airblast_visible([0.0, 0.0, 64.0], target).unwrap());
        session.projectiles[1].presentation.owner_identity = PLAYER_IDENTITY;
        assert!(session.airblast_visible([0.0, 0.0, 64.0], target).unwrap());
        let blocker = &mut session.projectiles[1].presentation;
        blocker.kind = ProjectileKind::Sticky;
        session.entity_queries.bind_bounds(entity, blocker.position, blocker.kind.entity_hull(), None).unwrap();
        assert!(!session.airblast_visible([0.0, 0.0, 64.0], target).unwrap());
        session.entity_queries.set_lists(entity, 0).unwrap();
        assert!(session.airblast_visible([0.0, 0.0, 64.0], target).unwrap());
    }

    #[test]
    fn actor_query_keeps_the_specified_standing_bounds_while_crouched() {
        let mut session = Session::new(crate::tests::Floor, [0.0, 0.0, 1.0], MapRuntime::empty(0.015));
        let actor = entity_queries::Entity::Actor(PLAYER_IDENTITY);
        session.entity_queries.set_lists(actor, 0x11).unwrap();
        session.entity_queries.mark_dirty(actor).unwrap();
        let mut command = Command::default();
        command.movement.crouch = true;
        for _ in 0..40 { session.advance(command).unwrap(); }
        let policy = MovementPolicy { class: session.class, modifiers: session.movement_modifiers }.resolve();
        assert_eq!(session.movement.active_hull(policy).maxs[2], 62.0);
        session.flush_entity_queries().unwrap();
        let point = add(session.movement.position, [0.0, 0.0, 70.0]);
        assert_eq!(session.entity_queries.sphere(point, 0.0, 0x10, 512, |_| true).unwrap(), [actor]);
    }

    #[test]
    fn rocket_query_uses_zero_radius_and_retires_with_the_projectile() {
        let mut session = Session::new(crate::tests::Floor, [0.0, 0.0, 1.0], MapRuntime::empty(0.015));
        for _ in 0..40 { session.advance(Command::default()).unwrap(); }
        session.advance(Command { fire: true, pitch_degrees: 85.0, ..Default::default() }).unwrap();
        let projectile = session.projectiles[0].presentation.clone();
        assert_eq!(projectile.kind, ProjectileKind::Rocket);
        session.flush_entity_queries().unwrap();
        let only_projectiles = |entity| matches!(entity, entity_queries::Entity::Projectile(_));
        assert_eq!(session.entity_queries.sphere(projectile.position, 0.0, 16, 64, only_projectiles).unwrap(), [entity_queries::Entity::Projectile(projectile.identity)]);
        assert!(session.entity_queries.sphere(add(projectile.position, [0.04, 0.0, 0.0]), 0.0, 16, 64, only_projectiles).unwrap().is_empty());
        for _ in 0..20 {
            if session.projectiles.is_empty() { break; }
            let results = session.rocket_trace_requests().iter().map(|request| {
                let trace = playsrc_movement::Tracer::trace(&session.collision, request.start, request.end,
                    Hull { mins: [0.0; 3], maxs: [0.0; 3] }, request.mask).unwrap();
                RocketTraceResult { projectile: request.projectile, tick: session.tick, end: trace.end,
                    solid: trace.fraction < 1.0 || trace.start_solid, sky: false, normal: trace.normal, direct_target: None }
            }).collect::<Vec<_>>();
            session.advance_with_external(Command::default(), &results, None).unwrap();
        }
        assert!(session.projectiles.is_empty());
        assert!(session.entity_queries.sphere([0.0; 3], 16384.0, 16, 64, only_projectiles).unwrap().is_empty());
    }

    #[test]
    fn airblast_candidates_filter_flags_before_the_shared_cap_but_not_teams_or_owner() {
        use entity_queries::Entity as Q;
        let mut session = Session::new(crate::tests::Floor, [0.0; 3], MapRuntime::empty(0.015));
        session.fire_projectile(0.0, 0.0, 0.0, None, None, &mut Vec::new()).unwrap();
        let prototype = session.projectiles.pop().unwrap();
        session.entity_queries.destroy(Q::Projectile(prototype.presentation.identity)).unwrap();
        session.sync_local_query();
        session.flush_entity_queries().unwrap();
        for identity in 1..=140 {
            let mut projectile = prototype.clone();
            projectile.presentation.identity = identity;
            projectile.presentation.position = [128.0, 0.0, 0.0];
            projectile.presentation.kind = if identity > 70 { ProjectileKind::Syringe }
                else { [ProjectileKind::Rocket, ProjectileKind::Flare, ProjectileKind::Grenade, ProjectileKind::Sticky][identity as usize % 4] };
            let entity = Q::Projectile(identity);
            session.entity_queries.register(entity).unwrap();
            session.entity_queries.bind_bounds(entity, projectile.presentation.position, projectile.presentation.kind.entity_hull(), None).unwrap();
            session.entity_queries.set_lists(entity, 0x11).unwrap();
            session.projectiles.push(projectile);
            session.flush_entity_queries().unwrap();
        }
        for slot in 1..=70 {
            let entity = Q::Map(playsrc_entity::EntityHandle { slot, generation: 1 });
            session.entity_queries.register(entity).unwrap();
            session.entity_queries.bind_bounds(entity, [128.0, 0.0, 0.0], Hull { mins: [-2.0; 3], maxs: [2.0; 3] }, None).unwrap();
            session.entity_queries.set_lists(entity, 0x11).unwrap();
            session.flush_entity_queries().unwrap();
        }
        let actual = session.airblast_candidates([0.0; 3], [1.0, 0.0, 0.0], 128.0).unwrap();
        assert_eq!(actual, (7..=70).rev().map(Q::Projectile).collect::<Vec<_>>());
        // Re-entering the owner's bounds moves it ahead of same-cell projectiles.
        // Self/team rejection belongs after the 64-entity collection limit.
        session.entity_queries.actor_state(PLAYER_IDENTITY, [128.0, 0.0, 0.0], true).unwrap();
        let actual = session.airblast_candidates([0.0; 3], [1.0, 0.0, 0.0], 128.0).unwrap();
        assert_eq!(actual[0], Q::Actor(PLAYER_IDENTITY));
        assert_eq!(&actual[1..], &(8..=70).rev().map(Q::Projectile).collect::<Vec<_>>());
        let mut restored = session.clone();
        assert_eq!(restored.airblast_candidates([0.0; 3], [1.0, 0.0, 0.0], 128.0).unwrap(), actual);
    }

    #[test]
    fn airblast_candidates_use_padded_box_membership_not_a_distance_or_cone_test() {
        use entity_queries::Entity as Q;
        let mut session = Session::new(crate::tests::Floor, [0.0; 3], MapRuntime::empty(0.015));
        session.fire_projectile(0.0, 0.0, 0.0, None, None, &mut Vec::new()).unwrap();
        let projectile = &mut session.projectiles[0].presentation;
        let identity = projectile.identity;
        session.entity_queries.set_origin(Q::Projectile(identity), &mut projectile.position, [0.0, 128.03, 128.03]).unwrap();
        assert!(session.airblast_candidates([0.0; 3], [1.0, 0.0, 0.0], 128.0).unwrap().contains(&Q::Projectile(identity)));
        let projectile = &mut session.projectiles[0].presentation;
        session.entity_queries.set_origin(Q::Projectile(identity), &mut projectile.position, [0.0, 128.04, 128.03]).unwrap();
        assert!(!session.airblast_candidates([0.0; 3], [1.0, 0.0, 0.0], 128.0).unwrap().contains(&Q::Projectile(identity)));
    }

    #[derive(Clone)]
    struct World(Arc<playsrc_collision::World>);
    impl playsrc_movement::Tracer for World {
        fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<playsrc_movement::Trace, MoveError> {
            let trace = self.0.trace_hull(start, end, hull, mask).unwrap();
            Ok(playsrc_movement::Trace { fraction: trace.fraction, start_solid: trace.start_solid,
                all_solid: trace.all_solid, end: trace.end, normal: trace.plane.map(|plane| plane.normal),
                hit: trace.did_hit().then_some(0), contents: trace.contents })
        }
        fn point_contents(&self, point: [f32; 3]) -> Result<u32, MoveError> { Ok(self.0.point_contents(point).unwrap().contents) }
    }
    impl GameplayWorld for World {
        fn static_query_bounds(&self) -> Result<Vec<(u64, Hull)>, MoveError> { Ok(Vec::new()) }
        fn trace_projectile_solid(&self, start: [f32; 3], end: [f32; 3], mask: u32) -> Result<ProjectileSolidTrace, MoveError> {
            Ok(self.0.trace_hull(start, end, Hull { mins: [0.0; 3], maxs: [0.0; 3] }, mask).unwrap().into())
        }
        fn overlaps_model_hull(&self, model: usize, origin: [f32; 3], position: [f32; 3], hull: Hull) -> Result<bool, MoveError> {
            Ok(self.0.overlaps_model_hull(model, origin, position, hull).unwrap())
        }
        fn trace_grenade_entities(&self, _: [f32; 3], _: [f32; 3], _: u32, hitboxes: &[PosedPlayerHitbox]) -> Result<Option<GrenadeEntityHit>, MoveError> {
            assert!(hitboxes.is_empty());
            Ok(None)
        }
    }

    fn configured_session(class: PlayerClass, weapon: Weapon) -> Session<World> {
        let (collision, models, surfaces, projectiles) = configured_resources::load();
        let rigid = RigidWorld::from_world_model(playsrc_physics::EnvironmentConfig {
            random_seed: 1, gravity: [0.0, 0.0, -800.0], air_density: 2.0, timestep: 0.015,
            max_bodies: 128, max_events: 16384, performance: playsrc_physics::PerformanceSettings {
                max_collisions_per_body: 10, ..Default::default() },
        }, &collision, &models, surfaces).unwrap();
        let mut session = Session::new(World(Arc::new(collision)), [5384.0, 3440.0, -2807.96875], MapRuntime::empty(0.015));
        session.install_rigid_world(rigid, projectiles, &models, &BTreeMap::new()).unwrap();
        session.advance(Command { select_class: Some(class), select_weapon: Some(weapon), ..Default::default() }).unwrap();
        for _ in 0..40 { session.advance(Command::default()).unwrap(); }
        session
    }

    fn stuck_session() -> Session<World> {
        let mut session = configured_session(PlayerClass::Demoman, Weapon::StickybombLauncher);
        session.advance(Command { fire: true, pitch_degrees: 85.0, ..Default::default() }).unwrap();
        session.advance(Command { pitch_degrees: 85.0, ..Default::default() }).unwrap();
        let mut impact_count = 0;
        for _ in 0..150 {
            if session.projectiles[0].armed && session.projectiles[0].touched_time.is_some() { break; }
            session.advance(Command::default()).unwrap();
            for event in session.audio_events().iter().filter(|event| event.identity == AudioEventIdentity::PhysicsImpact) {
                impact_count += 1;
                assert_eq!(event.source_kind, AudioSourceKind::World);
                assert_eq!(event.source_identity, 0);
                assert_eq!(event.owner_identity, None);
                assert!(matches!(event.action, AudioAction::PlayAtVolume(volume) if volume > 0.0 && volume <= 0.6));
                assert!(matches!(event.definition, SoundDefinition::DefaultImpactHard | SoundDefinition::DefaultImpactSoft));
                assert!(event.samples.wave < 3);
            }
        }
        assert!(impact_count > 0, "actual configured contact must publish physical impact audio");
        session.flush_entity_queries().unwrap();
        assert!(session.projectiles[0].armed && session.projectiles[0].touched_time.is_some());
        session
    }

    fn enemy_sticky_session() -> Session<World> {
        let mut session = configured_session(PlayerClass::Pyro, Weapon::Flamethrower);
        let team = if session.team_selection.local_team() == PlayerTeam::Red { PlayerTeam::Blue } else { PlayerTeam::Red };
        let attack = bot::Attack { phase: bot::AttackPhase::Fire, attacker: 30, team, weapon: Weapon::StickybombLauncher,
            target: PLAYER_IDENTITY, position: session.movement.position, eye_position: add(session.movement.position, session.movement.view_offset),
            pitch_degrees: 85.0, yaw_degrees: 0.0 };
        session.fire_projectile(85.0, 0.0, 0.0, None, Some(attack), &mut Vec::new()).unwrap();
        for _ in 0..150 {
            if session.projectiles[0].armed && session.projectiles[0].touched_time.is_some() { break; }
            session.advance(Command::default()).unwrap();
        }
        assert!(session.projectiles[0].armed && session.projectiles[0].touched_time.is_some());
        session
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_airblast_command_selects_and_dislodges_an_enemy_sticky() {
        let mut session = enemy_sticky_session();
        let original = session.projectiles[0].clone();
        assert!(original.armed && original.touched_time.is_some());
        let eye = add(session.movement.position, session.movement.view_offset);
        assert!(session.airblast_visible(eye, entity_queries::Entity::Projectile(original.presentation.identity)).unwrap());
        let result = session.advance(Command { detonate: true, pitch_degrees: 85.0, ..Default::default() }).unwrap();
        assert!(result.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Deflect && event.projectile == original.presentation.identity));
        let sounds = session.audio_events().iter().filter(|event| event.definition == SoundDefinition::FlameDeflect).collect::<Vec<_>>();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].source_identity, original.presentation.identity);
        assert_eq!(sounds[0].source_kind, AudioSourceKind::Projectile);
        assert_eq!(sounds[0].owner_identity, Some(original.presentation.owner_identity));
        assert_eq!(sounds[0].position, original.presentation.position);
        assert_eq!(sounds[0].samples.wave, 0);
        let expected = [RandomDecision::SoundVolume { definition: SoundDefinition::FlameDeflect, phase: SoundQueryPhase::Emit },
            RandomDecision::SoundPitch { definition: SoundDefinition::FlameDeflect, phase: SoundQueryPhase::Emit },
            RandomDecision::SoundLevel { definition: SoundDefinition::FlameDeflect, phase: SoundQueryPhase::Emit }];
        let draws = session.random_draws().iter().filter(|draw| expected.contains(&draw.decision)).collect::<Vec<_>>();
        assert_eq!(draws.iter().map(|draw| draw.decision).collect::<Vec<_>>(), expected);
        assert!(draws.iter().all(|draw| draw.context == RandomContext::Authority));
        let projectile = &session.projectiles[0];
        assert_eq!(projectile.presentation.owner_identity, original.presentation.owner_identity);
        assert_eq!(projectile.presentation.team, original.presentation.team);
        assert_eq!(projectile.presentation.launcher_source, original.presentation.launcher_source);
        assert_eq!(projectile.deflect_owner, Some(PLAYER_IDENTITY));
        assert!(projectile.touched_time.is_none());
        assert!(projectile.minimum_sleep_time >= session.tick as f32 * 0.015);
        let minimum_sleep_time = projectile.minimum_sleep_time;
        for _ in 0..150 {
            session.advance(Command::default()).unwrap();
            if let Some(touched) = session.projectiles[0].touched_time {
                assert!(touched >= minimum_sleep_time);
                break;
            }
        }
        assert!(session.projectiles[0].touched_time.is_some(), "the command-driven dislodged sticky must re-adhere");
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_shotgun_command_selects_and_fizzles_sticky_without_actor_aliasing() {
        let mut session = enemy_sticky_session();
        session.advance(Command { select_weapon: Some(Weapon::Shotgun), ..Default::default() }).unwrap();
        for _ in 0..40 { session.advance(Command::default()).unwrap(); }
        let original = session.projectiles[0].presentation.clone();
        assert_eq!(original.identity, PLAYER_IDENTITY);
        let delta = sub(original.position, add(session.movement.position, session.movement.view_offset));
        let mut command = Command::default();
        command.fire = true;
        command.pitch_degrees = (-delta[2]).atan2((delta[0] * delta[0] + delta[1] * delta[1]).sqrt()).to_degrees();
        command.movement.yaw_degrees = delta[1].atan2(delta[0]).to_degrees();
        let health = session.health;
        let snapshot = session.advance(command).unwrap();
        assert!(snapshot.events.iter().any(|event| matches!(event, Event::HitscanImpact { target: Some(identity), .. } if *identity == original.identity)));
        assert!(snapshot.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Fizzle && event.projectile == original.identity));
        assert_eq!(session.health, health);
        assert!(!snapshot.events.iter().any(|event| matches!(event, Event::Damaged { .. } | Event::PlayerDamaged { .. })));
        for _ in 0..20 {
            if session.projectiles.is_empty() { break; }
            session.advance(Command::default()).unwrap();
        }
        assert!(session.rigid_world().unwrap().projectile_body(original.identity).is_none());
        session.flush_entity_queries().unwrap();
        assert!(session.entity_queries.sphere(original.position, 8.0, 0x10, 512, |entity| matches!(entity, entity_queries::Entity::Projectile(_))).unwrap().is_empty());
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_melee_commands_use_line_or_hull_to_destroy_stickies() {
        let template = enemy_sticky_session();
        for (class, weapon) in [(PlayerClass::Demoman, Weapon::Bottle), (PlayerClass::Soldier, Weapon::Shovel), (PlayerClass::Spy, Weapon::Knife)] {
            let mut session = template.clone();
            session.advance(Command { select_class: Some(class), respawn: true, ..Default::default() }).unwrap();
            session.advance(Command { select_weapon: Some(weapon), ..Default::default() }).unwrap();
            for _ in 0..40 { session.advance(Command::default()).unwrap(); }
            assert_eq!(session.weapon, Some(weapon));
            if class == PlayerClass::Spy {
                let mut crouch = Command::default();
                crouch.movement.crouch = true;
                for _ in 0..40 { session.advance(crouch).unwrap(); }
            }
            let original = session.projectiles[0].presentation.clone();
            let eye = add(session.movement.position, session.movement.view_offset);
            let delta = sub(original.position, eye);
            let mut command = Command::default();
            command.fire = true;
            command.movement.crouch = class == PlayerClass::Spy;
            command.pitch_degrees = (-delta[2]).atan2((delta[0] * delta[0] + delta[1] * delta[1]).sqrt()).to_degrees();
            command.movement.yaw_degrees = delta[1].atan2(delta[0]).to_degrees();
            let (direction, _, _) = angle_vectors(command.pitch_degrees, command.movement.yaw_degrees, 0.0);
            let end = add(eye, scale(direction, ballistics::MELEE_RANGE));
            let line = session.trace_melee_scene(PLAYER_IDENTITY, eye, end, Hull { mins: [0.0; 3], maxs: [0.0; 3] }, false).unwrap().0;
            if class == PlayerClass::Spy { assert!(line.fraction < 1.0); }
            else { assert_eq!(line.fraction, 1.0); }
            let (trace, target) = session.trace_melee_scene(PLAYER_IDENTITY, eye, end, Hull { mins: [-18.0; 3], maxs: [18.0; 3] }, false).unwrap();
            assert_eq!(target, Some(hitscan::DamageTarget::Projectile(original.identity)), "{class:?}/{weapon:?}: eye={eye:?}, sticky={:?}, trace={trace:?}", original.position);
            let health = session.health;
            let first = session.advance(command).unwrap();
            let mut fizzled = first.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Fizzle && event.projectile == original.identity);
            command.fire = false;
            for _ in 0..20 {
                let frame = session.advance(command).unwrap();
                fizzled |= frame.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Fizzle && event.projectile == original.identity);
            }
            assert!(fizzled, "{weapon:?} must deliver its melee damage to the sticky");
            assert_eq!(session.health, health);
            assert!(session.rigid_world().unwrap().projectile_body(original.identity).is_none());
        }
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_detonation_dislodges_enemy_sticky_and_preserves_its_launcher() {
        let mut session = enemy_sticky_session();
        let enemy = session.projectiles[0].presentation.clone();
        session.advance(Command { select_class: Some(PlayerClass::Demoman), respawn: true, ..Default::default() }).unwrap();
        session.advance(Command { select_weapon: Some(Weapon::StickybombLauncher), ..Default::default() }).unwrap();
        for _ in 0..40 { session.advance(Command::default()).unwrap(); }
        session.advance(Command { fire: true, pitch_degrees: 85.0, ..Default::default() }).unwrap();
        session.advance(Command { pitch_degrees: 85.0, ..Default::default() }).unwrap();
        for _ in 0..150 {
            if session.projectiles.len() == 2 && session.projectiles[1].armed && session.projectiles[1].touched_time.is_some() { break; }
            session.advance(Command::default()).unwrap();
        }
        assert_eq!(session.projectiles.len(), 2);
        assert!(session.projectiles.iter().all(|projectile| projectile.touched_time.is_some()));
        let mut friendly = session.clone();
        friendly.projectiles[0].presentation.team = friendly.team_selection.local_team();
        friendly.advance(Command { detonate: true, ..Default::default() }).unwrap();
        assert!(friendly.projectiles.iter().find(|projectile| projectile.presentation.identity == enemy.identity).unwrap().touched_time.is_some());
        let frame = session.advance(Command { detonate: true, ..Default::default() }).unwrap();
        assert!(frame.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Explode && event.projectile != enemy.identity));
        let retained = session.projectiles.iter().find(|projectile| projectile.presentation.identity == enemy.identity).unwrap();
        assert!(retained.touched_time.is_none(), "an adjacent enemy explosion must detach the sticky");
        assert_eq!(retained.presentation.owner_identity, enemy.owner_identity);
        assert_eq!(retained.presentation.team, enemy.team);
        assert_eq!(retained.presentation.launcher_source, enemy.launcher_source);
        let force_draws = |session: &Session<World>| session.random_draws().iter().filter(|draw| draw.decision == RandomDecision::BlastForce).copied().collect::<Vec<_>>();
        assert_eq!(force_draws(&session), force_draws(&friendly), "friendly rejection happens after radius force generation");
        assert!(force_draws(&session).iter().all(|draw| draw.context == RandomContext::Authority));
        assert_eq!(force_draws(&session).len(), 2, "enemy sticky followed by the separate self-blast pass");
        let minimum = retained.minimum_sleep_time;
        for _ in 0..200 {
            session.advance(Command::default()).unwrap();
            let retained = session.projectiles.iter().find(|projectile| projectile.presentation.identity == enemy.identity).unwrap();
            if let Some(touched) = retained.touched_time { assert!(touched >= minimum); return; }
        }
        panic!("blast-displaced enemy sticky must re-adhere");
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_airblast_command_reflects_grenade_body_and_current_launcher() {
        let mut session = configured_session(PlayerClass::Pyro, Weapon::Flamethrower);
        let local_team = session.team_selection.local_team();
        let team = if local_team == PlayerTeam::Red { PlayerTeam::Blue } else { PlayerTeam::Red };
        let position = add(session.movement.position, [80.0, 0.0, 0.0]);
        let attack = bot::Attack { phase: bot::AttackPhase::Fire, attacker: 30, team, weapon: Weapon::GrenadeLauncher,
            target: PLAYER_IDENTITY, position, eye_position: add(position, session.movement.view_offset), pitch_degrees: 0.0, yaw_degrees: 180.0 };
        session.fire_projectile(0.0, 180.0, 0.0, None, Some(attack), &mut Vec::new()).unwrap();
        let original = session.projectiles[0].presentation.clone();
        session.conditions.insert(Condition::CritBoosted);
        let result = session.advance(Command { detonate: true, ..Default::default() }).unwrap();
        assert!(result.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Deflect && event.projectile == original.identity));
        let projectile = &session.projectiles[0];
        assert_eq!(projectile.presentation.kind, ProjectileKind::Grenade);
        assert_eq!(projectile.presentation.team, local_team);
        assert_eq!(projectile.presentation.owner_identity, PLAYER_IDENTITY);
        assert_eq!(projectile.presentation.source_weapon, original.source_weapon);
        assert_eq!(projectile.presentation.launcher_source, session.weapon_source(PLAYER_IDENTITY, Weapon::Flamethrower));
        assert_eq!(projectile.presentation.launcher_weapon, Weapon::Flamethrower);
        assert!(projectile.presentation.critical && projectile.presentation.deflected);
        assert_eq!(projectile.deflected_until, None);
        assert_eq!(session.projectile_death_icon(&projectile.presentation, damage::CustomDamage::None, "tf_projectile_pipe"), "deflect_promode");
        let body = session.rigid_world().unwrap().projectile_body(original.identity).unwrap();
        assert!(session.rigid_world().unwrap().physics().body(body).unwrap().published().unwrap().linear_velocity[0] > 0.0);
        assert_eq!(session.audio_events().iter().filter(|event| event.definition == SoundDefinition::FlameDeflect).count(), 1);
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_sticky_damage_detaches_without_changing_ownership_or_arm_state() {
        let mut session = stuck_session();
        let original = session.projectiles[0].clone();
        assert!(original.armed && original.touched_time.is_some());
        let identity = original.presentation.identity;
        assert_eq!(session.entity_queries.sphere(original.presentation.position, 0.0, 0x10, 512, |entity| matches!(entity, entity_queries::Entity::Projectile(_))).unwrap(),
            [entity_queries::Entity::Projectile(identity)]);
        let body = session.rigid_world().unwrap().projectile_body(identity).unwrap();
        let world = session.rigid_world().unwrap();
        let surface = world.surfaces().surface_data(world.physics().body(body).unwrap().material_index() as i32).unwrap();
        assert_eq!(surface.name, b"default");
        assert_eq!(surface.impact_audio.hard_sound.as_deref(), Some(b"default.impacthard".as_slice()));
        assert_eq!(surface.impact_audio.soft_sound.as_deref(), Some(b"default.impactsoft".as_slice()));
        assert_eq!((surface.impact_audio.hardness, surface.impact_audio.hard_threshold, surface.impact_audio.hard_velocity_threshold), (1.0, 0.5, 0.0));
        let mut queued = session.clone();
        queued.audio_events.clear();
        queued.random_draws.clear();
        let selection = queued.random_state().sound_selection;
        let rigid = queued.rigid.as_mut().unwrap();
        rigid.impact_sounds.clear();
        rigid.impact_sound_time = 0.0;
        rigid.impact_sound(0, 0, [1.0; 3], 0.049, 320.0);
        rigid.impact_sound(0, 0, [1.0; 3], 0.05, 69.0);
        assert!(rigid.impact_sounds.is_empty());
        rigid.impact_sound(0, 0, [1.0; 3], 0.05, 100.0);
        rigid.impact_sound(0, 0, [2.0; 3], 0.05, 80.0);
        assert_eq!(rigid.impact_sounds.len(), 1);
        assert_eq!(rigid.impact_sounds[0].position, [1.0; 3]);
        assert_eq!(rigid.impact_sounds[0].speed.to_bits(), (100.0_f32 + 1e-4).to_bits());
        for _ in 0..3 {
            queued.flush_rigid_impact_sounds().unwrap();
            assert!(queued.audio_events.is_empty());
        }
        let pending_sounds = queued.rigid.as_ref().unwrap().impact_sounds.clone();
        let pending_time = queued.rigid.as_ref().unwrap().impact_sound_time;
        let pending_random = queued.random_state();
        assert!(queued.advance(Command { pitch_degrees: f32::NAN, ..Default::default() }).is_err());
        assert_eq!(queued.rigid.as_ref().unwrap().impact_sounds, pending_sounds);
        assert_eq!(queued.rigid.as_ref().unwrap().impact_sound_time, pending_time);
        assert_eq!(queued.random_state(), pending_random);
        queued.flush_rigid_impact_sounds().unwrap();
        assert_eq!(queued.audio_events.len(), 1);
        assert_eq!(queued.audio_events[0].position, [1.0; 3]);
        assert_eq!(queued.audio_events[0].action, AudioAction::PlayAtVolume(0.6 * (100.0_f32 * 100.0 * (1.0 / 102400.0) + 80.0_f32 * 80.0 * (1.0 / 102400.0))));
        assert_eq!(queued.random_state().sound_selection, selection);
        assert_eq!(queued.random_draws.len(), 4);
        assert!(queued.random_draws.iter().all(|draw| draw.context == RandomContext::Authority));
        assert_eq!(queued.rigid.as_ref().unwrap().impact_sound_time, 0.0);
        assert!(queued.rigid.as_ref().unwrap().impact_sounds.is_empty());
        queued.audio_events.clear();
        let rigid = queued.rigid.as_mut().unwrap();
        let metal = rigid.world.surfaces().surface_index(b"metal") as u32;
        rigid.impact_sound(0, metal, [3.0; 3], 0.05, 320.0);
        rigid.impact_sound(metal, 0, [4.0; 3], 0.05, 320.0);
        for _ in 0..4 { queued.flush_rigid_impact_sounds().unwrap(); }
        assert_eq!(queued.audio_events.iter().map(|event| (event.definition, event.action)).collect::<Vec<_>>(), [
            (SoundDefinition::SolidMetalImpactHard, AudioAction::PlayAtVolume(0.4)),
            (SoundDefinition::DefaultImpactHard, AudioAction::PlayAtVolume(0.6)),
        ]);
        let flesh = queued.rigid.as_ref().unwrap().world.surfaces().surface_index(b"flesh") as u32;
        for (surface, hit, speed, expected) in [
            (flesh, 0, 499.0, SoundDefinition::FleshImpactSoft),
            (flesh, 0, 500.0, SoundDefinition::FleshImpactHard),
            (0, flesh, 600.0, SoundDefinition::DefaultImpactSoft),
        ] {
            queued.audio_events.clear();
            queued.rigid.as_mut().unwrap().impact_sound(surface, hit, [5.0; 3], 0.05, speed);
            for _ in 0..4 { queued.flush_rigid_impact_sounds().unwrap(); }
            assert_eq!(queued.audio_events.len(), 1);
            assert_eq!(queued.audio_events[0].definition, expected);
        }
        let team = original.presentation.team;
        let enemy = if team == PlayerTeam::Red { PlayerTeam::Blue } else { PlayerTeam::Red };
        let position = original.presentation.position;
        assert_eq!(identity, PLAYER_IDENTITY, "fixture deliberately shares numeric IDs across namespaces");
        let mut namespace = session.clone();
        let attack = hitscan::Attack { owner: 2, team: enemy, weapon: Weapon::Shotgun, source: None,
            position: [0.0; 3], center: [0.0; 3], rules: hitscan::BulletRules::resolve(Weapon::Shotgun,
                ballistics::HitscanProfile::configured(Weapon::Shotgun).unwrap(), Default::default(), 1.0, false, |_, value| value) };
        let mut group = hitscan::DamageGroup { target: hitscan::DamageTarget::Actor(identity), amount: 1.0, range_multiplier: 1.0,
            position, force: [2000.0, 0.0, 0.0], crit: damage::CritKind::None, custom: damage::CustomDamage::None };
        let mut namespace_events = Vec::new();
        namespace.apply_bullet_group(attack, group, &mut Vec::new(), &mut MapPhase::default(), &mut namespace_events).unwrap();
        assert!(!namespace.projectiles[0].retiring);
        assert!(namespace_events.is_empty());
        group.target = hitscan::DamageTarget::Projectile(identity);
        namespace.apply_bullet_group(attack, group, &mut Vec::new(), &mut MapPhase::default(), &mut namespace_events).unwrap();
        assert!(namespace.projectiles[0].retiring);
        assert_eq!(namespace_events[0].kind, ProjectileEventKind::Fizzle);
        let mut events = Vec::new();
        let before = session.rigid_world().unwrap().physics().snapshot();
        assert!(!session.damage_sticky(identity, team, D::SONIC, [1000.0, 0.0, 0.0], position, &mut events).unwrap());
        assert!(!session.damage_sticky(identity, enemy, D::SONIC, [750.0, 0.0, 0.0], position, &mut events).unwrap());
        assert_eq!(session.rigid_world().unwrap().physics().snapshot(), before);
        let mut broken = session.clone();
        let mut broken_events = Vec::new();
        assert!(!broken.damage_sticky(identity, enemy, D::BULLET, [0.0; 3], position, &mut broken_events).unwrap());
        assert_eq!(broken_events.len(), 1);
        assert_eq!(broken_events[0].kind, ProjectileEventKind::Fizzle);
        assert!(broken.projectiles[0].retiring);
        assert!(broken.rigid_world().unwrap().physics().body(body).is_some());
        assert_eq!(broken.entity_queries.sphere(position, 0.0, 0x10, 512, |entity| matches!(entity, entity_queries::Entity::Projectile(_))).unwrap(), [entity_queries::Entity::Projectile(identity)]);
        broken.advance(Command::default()).unwrap();
        assert!(broken.rigid_world().unwrap().physics().body(body).is_none());
        assert!(broken.entity_queries.sphere(position, 0.0, 0x10, 512, |entity| matches!(entity, entity_queries::Entity::Projectile(_))).unwrap().is_empty());
        assert_eq!(broken.pipebomb_count(PLAYER_IDENTITY), 0);
        let mut restick = session.clone();
        assert!(restick.damage_sticky(identity, enemy, D::SONIC, [0.0, 0.0, 1500.0], position, &mut events).unwrap());
        let deadline = restick.projectiles[0].minimum_sleep_time;
        let mut touched = false;
        for _ in 0..300 {
            restick.advance(Command::default()).unwrap();
            if let Some(time) = restick.projectiles[0].touched_time {
                assert!(time > deadline);
                assert_eq!(restick.projectiles[0].presentation.state, ProjectileState::StuckArmed);
                touched = true;
                break;
            }
        }
        assert!(touched, "configured detached sticky must collide and re-adhere");
        let mut suppressed = session.clone();
        assert!(suppressed.damage_sticky(identity, enemy, D::SONIC | D::PREVENT_FORCE, [1000.0, 0.0, 0.0], position, &mut events).unwrap());
        let suppressed_body = suppressed.rigid_world().unwrap().physics().body(body).unwrap().published().unwrap();
        assert!(suppressed_body.motion_enabled);
        assert_eq!(suppressed_body.linear_velocity, [0.0; 3]);
        assert!(session.damage_sticky(identity, enemy, D::SONIC, [1000.0, 0.0, 0.0], position, &mut events).unwrap());
        let projectile = &session.projectiles[0];
        assert_eq!(projectile.presentation.state, ProjectileState::Flying);
        assert_eq!(projectile.presentation.contact_normal, None);
        assert_eq!(projectile.touched_time, None);
        assert_eq!(projectile.minimum_sleep_time.to_bits(), (session.tick as f32 * 0.015 + 1.0).to_bits());
        assert_eq!(projectile.presentation.owner_identity, original.presentation.owner_identity);
        assert_eq!(projectile.presentation.launcher_source, original.presentation.launcher_source);
        assert_eq!(projectile.presentation.team, team);
        assert!(projectile.armed && projectile.launcher_linked);
        assert!(events.is_empty());
        assert_ne!(session.rigid_world().unwrap().physics().body(body).unwrap().published().unwrap().linear_velocity, [0.0; 3]);
    }

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_sticky_airblast_preserves_owner_applies_offset_force_and_expires_on_think() {
        let mut session = stuck_session();
        let original = session.projectiles[0].presentation.clone();
        let identity = original.identity;
        let body = session.rigid_world().unwrap().projectile_body(identity).unwrap();
        let physical = session.rigid_world().unwrap().physics().body(body).unwrap().published().unwrap();
        assert!(!physical.motion_enabled);
        let center = sub(original.position, [0.0, 0.0, 100.0]);
        let direction = playsrc_physics::normalize_source_vector(sub(original.position, center)).unwrap();
        let mut expected = session.rigid_world().unwrap().clone();
        expected.set_projectile_motion(identity, true).unwrap();
        expected.set_body_velocity(body, Some(scale(direction, length(physical.linear_velocity))), Some(physical.angular_velocity)).unwrap();
        expected.apply_projectile_force(identity, scale(scale(direction, 1000.0), 2.0), [0.0; 3]).unwrap();
        let enemy = if original.team == PlayerTeam::Red { PlayerTeam::Blue } else { PlayerTeam::Red };
        let mut events = Vec::new();
        let before = session.rigid_world().unwrap().physics().snapshot();
        assert!(!session.airblast_rigid(identity, 2, original.team, center, original.position, &mut events).unwrap());
        assert_eq!(session.rigid_world().unwrap().physics().snapshot(), before);
        let mut zero = session.clone();
        assert!(zero.airblast_rigid(identity, 2, enemy, original.position, original.position, &mut events).unwrap());
        assert!(zero.projectiles[0].touched_time.is_some());
        assert!(zero.rigid_world().unwrap().physics().body(body).unwrap().published().unwrap().motion_enabled);
        assert_eq!(zero.projectiles[0].presentation.orientation, quaternion_from_angles(90.0, 0.0, 0.0));
        events.clear();
        assert!(session.airblast_rigid(identity, 2, enemy, center, add(original.position, [100.0, 0.0, 0.0]), &mut events).unwrap());
        assert_eq!(session.rigid_world().unwrap().physics().snapshot(), expected.physics().snapshot());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, ProjectileEventKind::Deflect);
        assert_eq!(session.projectiles[0].presentation.owner_identity, original.owner_identity);
        assert_eq!(session.projectiles[0].presentation.team, original.team);
        assert_eq!(session.projectiles[0].presentation.launcher_source, original.launcher_source);
        assert_eq!(session.projectiles[0].deflect_owner, Some(2));
        assert_eq!(session.projectiles[0].touched_time, None);
        let deadline = session.tick as f32 * 0.015 + 10.0;
        assert_eq!(session.projectiles[0].deflected_until, Some(deadline));
        let mut flying = session.clone();
        let moving = flying.rigid_world().unwrap().physics().body(body).unwrap().published().unwrap();
        let direction = playsrc_physics::normalize_source_vector([100.0, 0.0, 0.0]).unwrap();
        let mut expected_flying = flying.rigid_world().unwrap().clone();
        expected_flying.set_projectile_motion(identity, true).unwrap();
        expected_flying.set_body_velocity(body, Some(scale(direction, length(moving.linear_velocity))), Some(moving.angular_velocity)).unwrap();
        assert!(flying.airblast_rigid(identity, 3, enemy, center, add(original.position, [100.0, 0.0, 0.0]), &mut Vec::new()).unwrap());
        assert_eq!(flying.rigid_world().unwrap().physics().snapshot(), expected_flying.physics().snapshot());
        assert_eq!(flying.projectiles[0].deflect_owner, Some(3));
        let mut expired = false;
        for _ in 0..750 {
            let next_think = session.projectiles[0].next_think_tick;
            session.advance(Command::default()).unwrap();
            let projectile = session.projectiles.iter().find(|projectile| projectile.presentation.identity == identity).expect("sticky remains in the configured room");
            if !projectile.presentation.deflected {
                assert!(session.tick as f32 * 0.015 >= deadline);
                assert!(next_think <= session.tick);
                assert_eq!(projectile.deflect_owner, None);
                assert_eq!(projectile.deflected_until, None);
                expired = true;
                break;
            }
        }
        assert!(expired);
    }

    #[test]
    fn sticky_damage_scales_in_source_branch_order_and_uses_a_strict_threshold() {
        for (kind, input, force, destroy, detach) in [
            (D::BULLET, 750.0, 1500.0_f32, true, false),
            (D::BULLET, 751.0, 1502.0, true, true),
            (D::MELEE, 0.0, 0.0, true, false),
            (D::BUCKSHOT, 2000.0, 1500.0, true, false),
            (D::BLAST, 10000.0, 1500.0, false, false),
            (D::BLAST, 10001.0, 10001.0 * 0.15, false, true),
            (D::SONIC, 751.0, 1502.0, false, true),
            (D::BLAST | D::IGNITE, 1.0, 0.15, true, false),
            (D::BULLET | D::SONIC | D::BUCKSHOT | D::BLAST, 500.0, 2000.0, true, true),
            (D::BULLET | D::BUCKSHOT | D::BLAST, 1000.0, 1500.0, true, false),
        ] {
            let reaction = sticky_damage_response(kind, [input, 0.0, 0.0]).unwrap();
            assert_eq!(reaction.force[0].to_bits(), force.to_bits());
            assert_eq!((reaction.destroy, reaction.detach), (destroy, detach));
        }
        assert!(sticky_damage_response(D::BURN, [100000.0; 3]).is_none());
        let reaction = sticky_damage_response(D::SONIC | D::PREVENT_FORCE, [1000.0, 0.0, 0.0]).unwrap();
        assert!(reaction.detach);
        assert!(!reaction.apply_force);
    }
}

#[derive(Clone, Debug)]
pub struct RigidProjectileModels {
    pub sticky: RigidModel,
    pub grenade: RigidModel,
}

#[derive(Clone, Debug)]
pub struct StudioRigidResource {
    pub physics: Arc<playsrc_phy::Asset>,
    pub model: Arc<playsrc_studio_model::PresentationModel>,
    pub shape_identities: Vec<u64>,
    pub follower_bones: Option<Vec<Vec<u8>>>,
}
#[derive(Clone, Debug)]
struct MapBinding {
    entity: u32,
    handle: Option<playsrc_entity::EntityHandle>,
    query_handle: Option<playsrc_entity::EntityHandle>,
    collision_bounds: Hull,
    axis_aligned_bounds: bool,
    blocks_los: bool,
    model: MapBindingModel,
}
#[derive(Clone, Debug)]
struct BoundBody {
    identity: Option<u64>,
    resource: RigidModel,
    kind: MapBodyKind,
    query: Option<Arc<playsrc_physics::ShapeCastModel>>,
}
#[derive(Clone, Debug)]
enum MapBindingModel {
    Body(BoundBody),
    Followers {
        model: Arc<playsrc_studio_model::PresentationModel>,
        bodies: Vec<(BoundBody, usize)>,
        last: Option<(playsrc_entity::Transform, usize, u32, u32)>,
        solid: bool,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub world: RigidWorld,
    models: RigidProjectileModels,
    map_bindings: Vec<MapBinding>,
    impact_sounds: Vec<ImpactSound>,
    impact_sound_time: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ImpactSound {
    surface: u16,
    hit_surface: u16,
    position: [f32; 3],
    volume: f32,
    speed: f32,
}

impl State {
    fn impact_sound(&mut self, surface: u32, hit_surface: u32, position: [f32; 3], elapsed: f32, speed: f32) {
        if elapsed < 0.05 || speed < 70.0 || self.world.game_material(surface) == Some(b'X') || self.world.game_material(hit_surface) == Some(b'X') { return; }
        let volume = (speed * speed * (1.0 / (320.0 * 320.0))).min(1.0);
        let speed = (f64::from(speed) + 1e-4) as f32;
        let crowded = self.impact_sounds.len() > 4;
        if let Some(sound) = self.impact_sounds.iter_mut().rev().find(|sound| sound.surface == surface as u16 || crowded) {
            if volume > sound.volume {
                sound.position = position;
                sound.hit_surface = hit_surface as u16;
            }
            sound.volume += volume;
            sound.speed = speed.max(sound.speed);
        } else {
            self.impact_sounds.push(ImpactSound { surface: surface as u16, hit_surface: hit_surface as u16, position, volume, speed });
        }
    }
}

impl<W: GameplayWorld + Clone> Session<W> {
    pub fn follower_collisions(&self) -> Result<BTreeMap<u32, Vec<playsrc_collision::ObjectInput>>, Error> {
        let mut output = BTreeMap::new();
        let Some(rigid) = &self.rigid else { return Ok(output); };
        for binding in &rigid.map_bindings {
            let MapBindingModel::Followers { bodies, .. } = &binding.model else { continue; };
            let objects = output.entry(binding.entity).or_insert_with(Vec::new);
            let Some(parent) = binding.query_handle else { continue; };
            if self.map.source_handle(binding.entity) != Some(parent) || binding.handle != Some(parent) || self.map.collision_entity(binding.entity).is_none() { continue; }
            for (body, _) in bodies {
                let Some(identity) = body.identity else { continue; };
                if identity >= 1 << 62 { return Err(Error::InvalidProjectilePhysics); }
                let MapBodyKind::BoneFollower(solid) = body.kind else { return Err(Error::InvalidProjectilePhysics); };
                let entity = entity_queries::Entity::Follower { parent, solid: u16::try_from(solid).map_err(|_| Error::MissingRigidResources)? };
                let (transform, _) = self.entity_queries.collision_geometry(entity).map_err(Error::ProjectileTrace)?;
                objects.push(playsrc_collision::ObjectInput {
                    identity: (1 << 62) | identity, role: playsrc_collision::ObjectRole::Entity,
                    enabled: self.entity_queries.solid(entity).map_err(Error::ProjectileTrace)?, volume_contents: false,
                    transform: playsrc_collision::Transform { origin: transform.origin, angles: transform.angles },
                    linear_velocity: [0.0; 3], angular_velocity: [0.0; 3], collision_group: 0,
                    contents: body.resource.contents(), surface_flags: 0,
                    shape: playsrc_collision::SnapshotShape::Follower { parent: u64::from(binding.entity),
                        query: body.query.as_ref().ok_or(Error::MissingRigidResources)?.clone() },
                });
            }
        }
        Ok(output)
    }

    pub(super) fn radius_trace(&mut self, start: [f32; 3], end: [f32; 3], ignored: Option<entity_queries::Entity>, retry: bool)
        -> Result<(playsrc_movement::Trace, Option<entity_queries::Entity>), Error> {
        use entity_queries::Entity as Q;
        self.flush_entity_queries()?;
        let hull = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let mask = MASK_SHOT & !playsrc_collision::CONTENTS_HITBOX;
        let world = self.collision.trace_world(start, end, hull, mask)?;
        if world.start_solid { return Ok((world, None)); }
        let mut result = playsrc_movement::Trace { fraction: 1.0, start_solid: false, all_solid: false, end: world.end, normal: None, hit: None, contents: 0 };
        let mut selected = None;
        for entity in self.entity_queries.ray(start, world.end, 1, usize::MAX, |entity| !matches!(entity, Q::Actor(_))).map_err(Error::ProjectileTrace)? {
            let logical = match entity { Q::Follower { parent, .. } => Q::Map(parent), _ => entity };
            if Some(logical) == ignored { continue; }
            if let Q::Projectile(identity) = entity {
                if !retry || !self.projectiles.iter().any(|projectile| projectile.presentation.identity == identity && projectile.presentation.kind.uses_rigid_physics()) { continue; }
            }
            let (candidate, _) = self.trace_query_entity(entity, start, world.end, hull, mask)?;
            if candidate.all_solid || candidate.start_solid || candidate.fraction < result.fraction {
                let start_solid = result.start_solid || candidate.start_solid;
                result = candidate;
                result.start_solid = start_solid;
                selected = Some(logical);
            }
            if result.all_solid { break; }
        }
        if result.hit.is_none() && !result.start_solid && !result.all_solid { return Ok((world, None)); }
        result.fraction *= world.fraction;
        result.end = add(start, scale(sub(end, start), result.fraction));
        Ok((result, selected))
    }

    pub(super) fn trace_query_entity(&self, entity: entity_queries::Entity, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32)
        -> Result<(playsrc_movement::Trace, Option<hitscan::DamageTarget>), Error> {
        use entity_queries::Entity as Q;
        if let Q::StaticProp(index) = entity {
            let identity = self.entity_queries.static_identity(index).map_err(Error::ProjectileTrace)?;
            return Ok((self.collision.trace_static(identity, start, end, hull, mask)?, None));
        }
        let (transform, mut bounds) = self.entity_queries.collision_geometry(entity).map_err(Error::ProjectileTrace)?;
        let mut physical = None;
        let target = match entity {
            Q::Map(handle) => {
                let source = self.map.query_source(handle).ok_or(Error::InvalidProjectilePhysics)?;
                if let Some((model, _)) = self.map.brush_query_data(handle).map_err(Error::Entity)? {
                    return Ok((self.collision.trace_brush(model, playsrc_collision::ObjectTraceRequest {
                        identity: u64::from(source), transform: playsrc_collision::Transform { origin: transform.origin, angles: transform.angles },
                        start, end, hull, mask,
                    })?, Some(hitscan::DamageTarget::Map(source))));
                }
                let binding = self.rigid.as_ref().and_then(|rigid| rigid.map_bindings.iter().find(|binding| binding.query_handle == Some(handle))).ok_or(Error::MissingRigidResources)?;
                if !binding.axis_aligned_bounds {
                    let MapBindingModel::Body(body) = &binding.model else { return Err(Error::InvalidProjectilePhysics); };
                    physical = Some(body.query.as_deref().ok_or(Error::MissingRigidResources)?);
                }
                hitscan::DamageTarget::Map(source)
            }
            Q::Follower { parent, solid } => {
                let binding = self.rigid.as_ref().and_then(|rigid| rigid.map_bindings.iter().find(|binding| binding.query_handle == Some(parent))).ok_or(Error::MissingRigidResources)?;
                let MapBindingModel::Followers { bodies, .. } = &binding.model else { return Err(Error::InvalidProjectilePhysics); };
                let body = bodies.iter().find(|(body, _)| body.kind == MapBodyKind::BoneFollower(usize::from(solid))).ok_or(Error::MissingRigidResources)?;
                physical = Some(body.0.query.as_deref().ok_or(Error::MissingRigidResources)?);
                hitscan::DamageTarget::Map(binding.entity)
            }
            Q::Actor(identity) => {
                bounds = self.projectile_target(identity).ok_or(Error::InvalidProjectilePhysics)?.hull;
                hitscan::DamageTarget::Actor(identity)
            }
            Q::Projectile(identity) => hitscan::DamageTarget::Projectile(identity),
            Q::Building(identity) => hitscan::DamageTarget::Building(identity),
            Q::StaticProp(_) => unreachable!(),
        };
        let trace = if let Some(physical) = physical {
            let result = physical.trace(start, end, hull, transform.origin, transform.angles, mask)
                .map_err(|error| Error::Rigid(playsrc_physics::EnvironmentError::from(error).into()))?;
            playsrc_movement::Trace { fraction: result.fraction, start_solid: result.start_solid, all_solid: result.all_solid,
                end: result.end, normal: (result.fraction < 1.0 || result.start_solid || result.all_solid).then_some(result.normal),
                hit: result.convex.map(|_| u64::from(target.identity())), contents: result.contents }
        } else {
            let result = playsrc_collision::trace_bounds(start, end, hull, Hull { mins: add(transform.origin, bounds.mins), maxs: add(transform.origin, bounds.maxs) })
                .map_err(Error::ProjectileTrace)?;
            playsrc_movement::Trace { fraction: result.fraction, start_solid: result.start_solid, all_solid: result.all_solid,
                end: result.end, normal: result.plane.map(|plane| plane.normal), hit: (result.contents != 0).then_some(u64::from(target.identity())), contents: result.contents }
        };
        Ok((trace, Some(target)))
    }

    pub(super) fn airblast_aim(&mut self, eye: [f32; 3], forward: [f32; 3]) -> Result<[f32; 3], Error> {
        let end = add(eye, scale(forward, 1.732_050_8 * 32_768.0));
        self.flush_entity_queries()?;
        let hull = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let world = self.collision.trace_world(eye, end, hull, MASK_SOLID)?;
        if world.start_solid { return Ok(world.end); }
        let mut fraction = 1.0;
        for entity in self.entity_queries.ray(eye, world.end, 1, usize::MAX, |entity| !matches!(entity, entity_queries::Entity::Actor(_) | entity_queries::Entity::Building(_)))
            .map_err(Error::ProjectileTrace)? {
            if let entity_queries::Entity::Projectile(identity) = entity {
                let projectile = self.projectiles.iter().find(|projectile| projectile.presentation.identity == identity).ok_or(Error::InvalidProjectilePhysics)?;
                if !projectile.presentation.kind.uses_rigid_physics() && projectile.presentation.owner_identity == PLAYER_IDENTITY { continue; }
            }
            let (hit, _) = self.trace_query_entity(entity, eye, world.end, hull, MASK_SOLID)?;
            if hit.all_solid || hit.start_solid || hit.fraction < fraction { fraction = hit.fraction; }
            if hit.all_solid { break; }
        }
        Ok(add(eye, scale(sub(end, eye), fraction * world.fraction)))
    }

    pub(super) fn airblast_visible(&mut self, eye: [f32; 3], target: entity_queries::Entity) -> Result<bool, Error> {
        use entity_queries::Entity as Q;
        let target_owner = match target {
            Q::Projectile(identity) => self.projectiles.iter().find(|projectile| projectile.presentation.identity == identity)
                .filter(|projectile| !projectile.presentation.kind.uses_rigid_physics())
                .map(|projectile| Q::Actor(projectile.presentation.owner_identity)),
            _ => None,
        };
        let end = match target {
            Q::Projectile(identity) => self.projectiles.iter().find(|projectile| projectile.presentation.identity == identity)
                .map(|projectile| projectile.presentation.position),
            Q::Actor(identity) => self.bots.as_ref().and_then(|bots| bots.eye_position(identity)),
            _ => None,
        }.ok_or(Error::InvalidProjectilePhysics)?;
        self.flush_entity_queries()?;
        let hull = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let world = self.collision.trace_world(eye, end, hull, MASK_SOLID)?;
        if world.fraction != 1.0 || world.start_solid { return Ok(false); }
        for entity in self.entity_queries.ray(eye, end, 1, usize::MAX, |entity|
            entity != target && entity != Q::Actor(PLAYER_IDENTITY) && Some(entity) != target_owner)
            .map_err(Error::ProjectileTrace)? {
            if let Q::Projectile(identity) = entity {
                let projectile = self.projectiles.iter().find(|projectile| projectile.presentation.identity == identity).ok_or(Error::InvalidProjectilePhysics)?;
                if !projectile.presentation.kind.uses_rigid_physics()
                    && (projectile.presentation.owner_identity == PLAYER_IDENTITY || Q::Actor(projectile.presentation.owner_identity) == target) { continue; }
            }
            let parent = match entity { Q::Map(handle) | Q::Follower { parent: handle, .. } => Some(handle), _ => None };
            if parent.is_some_and(|handle| self.rigid.as_ref().is_some_and(|rigid| rigid.map_bindings.iter().any(|binding|
                binding.query_handle == Some(handle) && !binding.blocks_los))) { continue; }
            let (trace, _) = self.trace_query_entity(entity, eye, end, hull, MASK_SOLID)?;
            if trace.fraction != 1.0 || trace.start_solid { return Ok(false); }
        }
        Ok(true)
    }

    pub(super) fn airblast_candidates(&mut self, eye: [f32; 3], forward: [f32; 3], radius: f32)
        -> Result<Vec<entity_queries::Entity>, Error> {
        self.flush_entity_queries()?;
        let projectiles = &self.projectiles;
        self.entity_queries.sphere(add(eye, scale(forward, radius)), radius, 0x10, 64, |entity| match entity {
            entity_queries::Entity::Actor(_) => true,
            entity_queries::Entity::Projectile(identity) => projectiles.iter().any(|projectile|
                projectile.presentation.identity == identity && projectile.presentation.kind != ProjectileKind::Syringe),
            _ => false,
        }).map_err(Error::ProjectileTrace)
    }

    pub(super) fn airblast_rigid(&mut self, identity: u32, deflector: u32, team: PlayerTeam,
        deflector_center: [f32; 3], aim: [f32; 3], events: &mut Vec<ProjectileEvent>) -> Result<bool, Error> {
        let Some(projectile) = self.projectiles.iter_mut().find(|projectile| projectile.presentation.identity == identity) else { return Ok(false); };
        if !projectile.presentation.kind.uses_rigid_physics() || projectile.presentation.team == team { return Ok(false); }
        let sticky = projectile.presentation.kind == ProjectileKind::Sticky;
        let rigid = self.rigid.as_mut().ok_or(Error::MissingRigidResources)?;
        let body = rigid.world.projectile_body(identity).ok_or(Error::InvalidProjectilePhysics)?;
        let physical = rigid.world.physics().body(body).ok_or(Error::InvalidProjectilePhysics)?.published()
            .map_err(|error| Error::Rigid(error.into()))?;
        let direction = playsrc_physics::normalize_source_vector(if physical.motion_enabled {
            sub(aim, projectile.presentation.position)
        } else { sub(projectile.presentation.position, deflector_center) }).map_err(|_| Error::InvalidProjectilePhysics)?;
        let velocity = scale(direction, length(physical.linear_velocity));
        rigid.world.set_projectile_motion(identity, true).map_err(Error::Rigid)?;
        rigid.world.set_body_velocity(body, Some(velocity), Some(physical.angular_velocity)).map_err(Error::Rigid)?;
        projectile.motion_enabled = true;
        // The stock launcher's four-unit box saturates DeflectionForce at 1000.
        // The default damage object leaves its damage position at world zero.
        if sticky {
            self.damage_sticky(identity, team, damage::DamageType::SONIC, scale(direction, 1000.0), [0.0; 3], events)?;
        } else {
            let weapon = if deflector == PLAYER_IDENTITY { self.weapon } else { self.bots.as_ref().and_then(|bots| bots.active_weapon(deflector)) }.ok_or(Error::InvalidProjectilePhysics)?;
            let source = self.weapon_source(deflector, weapon);
            let critical = condition::all_weapon_crit_boost(|condition| self.actor_condition(deflector, condition));
            let projectile = self.projectiles.iter_mut().find(|projectile| projectile.presentation.identity == identity).ok_or(Error::InvalidProjectilePhysics)?;
            projectile.presentation.team = team;
            projectile.presentation.owner_identity = deflector;
            projectile.presentation.launcher_identity = if deflector == PLAYER_IDENTITY { weapon as u32 } else { 0x4000_0000 + deflector };
            projectile.presentation.launcher_weapon = weapon;
            projectile.presentation.launcher_source = source;
            projectile.presentation.critical |= critical;
        }
        let projectile = self.projectiles.iter_mut().find(|projectile| projectile.presentation.identity == identity).ok_or(Error::InvalidProjectilePhysics)?;
        projectile.deflect_owner = Some(deflector);
        projectile.deflected_until = sticky.then_some(self.tick as f32 * self.movement_configuration.tick_interval + 10.0);
        projectile.presentation.deflected = true;
        projectile.presentation.orientation = deflection_orientation(direction);
        events.push(projectile_event(ProjectileEventKind::Deflect, &projectile.presentation, self.tick));
        Ok(true)
    }

    pub(super) fn flush_entity_queries(&mut self) -> Result<(), Error> {
        self.sync_map_queries()?;
        self.sync_building_queries()?;
        self.entity_queries.flush_bound().map_err(Error::ProjectileTrace)
    }

    pub(super) fn sync_map_queries(&mut self) -> Result<(), Error> {
        if !self.entity_queries.brushes_initialized {
            let initial = self.map.initial_brush_queries().map_err(Error::Entity)?;
            self.apply_map_query_changes(initial)?;
            self.entity_queries.brushes_initialized = true;
        }
        let changes = self.map.take_query_changes();
        self.apply_map_query_changes(changes)
    }

    fn apply_map_query_changes(&mut self, changes: Vec<map_runtime::QueryChange>) -> Result<(), Error> {
        for change in changes {
            match change {
                map_runtime::QueryChange::State { handle, state, .. } => {
                    if let Some((_, bounds)) = self.map.brush_query_data(handle).map_err(Error::Entity)? {
                        let entity = entity_queries::Entity::Map(handle);
                        if !self.entity_queries.contains(entity) { self.entity_queries.register(entity).map_err(Error::ProjectileTrace)?; }
                        self.entity_queries.bind_bounds(entity, state.transform.origin, bounds, Some(state.transform.angles)).map_err(Error::ProjectileTrace)?;
                        self.entity_queries.set_solid(entity, state.enabled).map_err(Error::ProjectileTrace)?;
                    }
                }
                map_runtime::QueryChange::Remove(handle) => {
                    let entity = entity_queries::Entity::Map(handle);
                    if self.entity_queries.contains(entity) { self.entity_queries.destroy(entity).map_err(Error::ProjectileTrace)?; }
                }
            }
            let Some(rigid) = &mut self.rigid else { continue; };
            let binding = match change {
                map_runtime::QueryChange::State { source, .. } => rigid.map_bindings.iter_mut().find(|binding| binding.entity == source),
                map_runtime::QueryChange::Remove(handle) => rigid.map_bindings.iter_mut().find(|binding| binding.query_handle == Some(handle)),
            };
            let Some(binding) = binding else { continue; };
            let current = match change { map_runtime::QueryChange::State { handle, .. } => Some(handle), _ => None };
            let created = binding.query_handle != current;
            if created {
                if let Some(handle) = binding.query_handle {
                    let entity = entity_queries::Entity::Map(handle);
                    if self.entity_queries.contains(entity) { self.entity_queries.destroy(entity).map_err(Error::ProjectileTrace)?; }
                    if let MapBindingModel::Followers { bodies, .. } = &binding.model {
                        for (body, _) in bodies {
                            let MapBodyKind::BoneFollower(solid) = body.kind else { return Err(Error::InvalidProjectilePhysics); };
                            self.entity_queries.destroy(entity_queries::Entity::Follower { parent: handle, solid: u16::try_from(solid).map_err(|_| Error::MissingRigidResources)? }).map_err(Error::ProjectileTrace)?;
                        }
                    }
                }
                if let Some(handle) = current {
                    let entity = entity_queries::Entity::Map(handle);
                    if !self.entity_queries.contains(entity) { self.entity_queries.register(entity).map_err(Error::ProjectileTrace)?; }
                }
                binding.query_handle = current;
            }
            if let map_runtime::QueryChange::State { handle, state, .. } = change {
                let entity = entity_queries::Entity::Map(handle);
                self.entity_queries.bind_bounds(entity, state.transform.origin, binding.collision_bounds,
                    (!binding.axis_aligned_bounds).then_some(state.transform.angles)).map_err(Error::ProjectileTrace)?;
                let solid = state.enabled && matches!(binding.model, MapBindingModel::Body(_));
                self.entity_queries.set_solid(entity, solid).map_err(Error::ProjectileTrace)?;
                if created && let MapBindingModel::Followers { model, bodies, .. } = &binding.model {
                    let (sequence, cycle, _) = self.map.rigid_animation(binding.entity).unwrap_or((0, 0.0, 0.0));
                    let pose = follower_pose(model, state.transform, sequence, cycle, self.tick as f32 * self.movement_configuration.tick_interval)?;
                    let basis = playsrc_physics::SourceAngleBasis::from_degrees([0.0; 3]).expect("zero rotation");
                    for (body, bone) in bodies {
                        let MapBodyKind::BoneFollower(solid) = body.kind else { return Err(Error::InvalidProjectilePhysics); };
                        let entity = entity_queries::Entity::Follower { parent: handle, solid: u16::try_from(solid).map_err(|_| Error::MissingRigidResources)? };
                        let bounds = playsrc_physics::source_shape_bounds(body.resource.shape(), [0.0; 3], basis)
                            .map_err(|error| Error::Rigid(playsrc_physics::EnvironmentError::from(error).into()))?;
                        let (origin, angles) = playsrc_studio_model::source_transform_components(*pose.bone_matrices.get(*bone).ok_or(Error::MissingRigidResources)?)
                            .map_err(|_| Error::MissingRigidResources)?;
                        self.entity_queries.register(entity).map_err(Error::ProjectileTrace)?;
                        self.entity_queries.bind_bounds(entity, origin.0.map(|value| f32::from_bits(value.0)), bounds, Some(angles.0.map(|value| f32::from_bits(value.0))))
                            .map_err(Error::ProjectileTrace)?;
                        self.entity_queries.set_solid(entity, state.enabled).map_err(Error::ProjectileTrace)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn flush_rigid_impact_sounds(&mut self) -> Result<(), Error> {
        let Some(rigid) = self.rigid.as_mut() else { return Ok(()); };
        rigid.impact_sound_time += self.movement_configuration.tick_interval;
        if rigid.impact_sound_time <= 0.05 { return Ok(()); }
        rigid.impact_sound_time = 0.0;
        let mut sounds = std::mem::take(&mut rigid.impact_sounds);
        for sound in sounds.iter().rev() {
            let surfaces = self.rigid.as_ref().unwrap().world.surfaces();
            let surface = surfaces.surface_data(i32::from(sound.surface)).ok_or(Error::MissingRigidResources)?;
            let Some(hard) = surface.impact_audio.hard_sound.as_deref() else { continue; };
            let hit = surfaces.surface_data(i32::from(sound.hit_surface)).ok_or(Error::MissingRigidResources)?;
            let name = if hit.impact_audio.hardness < surface.impact_audio.hard_threshold
                || (surface.impact_audio.hard_velocity_threshold > 0.0 && surface.impact_audio.hard_velocity_threshold > sound.speed) {
                surface.impact_audio.soft_sound.as_deref().unwrap_or(hard)
            } else { hard };
            let (definition, volume) = SoundDefinition::NATIVE.iter().find_map(|definition| {
                definition.identity().as_bytes().eq_ignore_ascii_case(name)
                    .then(|| definition.physical_impact_volume().map(|volume| (*definition, volume))).flatten()
            }).ok_or(Error::MissingRigidResources)?;
            let samples = self.sample_sound(RandomContext::Authority, definition, SoundQueryPhase::Inspect);
            self.push_audio_event(AudioEvent { action: AudioAction::PlayAtVolume(volume * sound.volume.min(1.0)),
                tick: self.tick, ordinal: 0, identity: AudioEventIdentity::PhysicsImpact, definition,
                source_kind: AudioSourceKind::World, source_identity: 0, owner_identity: None,
                position: sound.position, samples });
        }
        sounds.clear();
        self.rigid.as_mut().unwrap().impact_sounds = sounds;
        Ok(())
    }

    pub(super) fn damage_sticky(
        &mut self,
        identity: u32,
        attacker_team: PlayerTeam,
        kind: damage::DamageType,
        force: [f32; 3],
        position: [f32; 3],
        events: &mut Vec<ProjectileEvent>,
    ) -> Result<bool, Error> {
        let Some(projectile) = self.projectiles.iter_mut().find(|value| value.presentation.identity == identity) else { return Ok(false); };
        if projectile.presentation.kind != ProjectileKind::Sticky
            || projectile.presentation.team == attacker_team || projectile.touched_time.is_none() {
            return Ok(false);
        }
        let Some(response) = sticky_damage_response(kind, force) else { return Ok(false); };
        if response.destroy {
            events.push(projectile_event(ProjectileEventKind::Fizzle, &projectile.presentation, self.tick));
            projectile.retiring = true;
            projectile.next_think_tick = self.tick;
            projectile.detonate_on_think = false;
        }
        if response.detach {
            let rigid = self.rigid.as_mut().ok_or(Error::MissingRigidResources)?;
            rigid.world.set_projectile_motion(identity, true).map_err(Error::Rigid)?;
            if response.apply_force {
                rigid.world.apply_projectile_force(identity, response.force, position).map_err(Error::Rigid)?;
            }
            projectile.minimum_sleep_time = self.tick as f32 * self.movement_configuration.tick_interval + 1.0;
            projectile.touched_time = None;
            projectile.presentation.contact_normal = None;
            projectile.presentation.state = ProjectileState::Flying;
            projectile.motion_enabled = true;
        }
        Ok(response.detach)
    }

    fn grenade_solid_trace(
        &mut self,
        projectile: u32,
        start: [f32; 3],
        end: [f32; 3],
        mask: u32,
    ) -> Result<ProjectileSolidTrace, Error> {
        self.entity_queries.flush_bound().map_err(Error::ProjectileTrace)?;
        let mut trace = self.collision.trace_projectile_solid(start, end, mask)?;
        let mut bounds = |identity: u64,
                          origin: [f32; 3],
                          hull: Hull,
                          player: bool|
         -> Result<(), Error> {
            let candidate = playsrc_collision::trace_bounds(
                start,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                Hull {
                    mins: add(origin, hull.mins),
                    maxs: add(origin, hull.maxs),
                },
            )
            .map_err(Error::ProjectileTrace)?;
            if candidate.all_solid || candidate.start_solid || candidate.fraction < trace.fraction {
                trace = ProjectileSolidTrace {
                    fraction: candidate.fraction,
                    start_solid: trace.start_solid || candidate.start_solid,
                    all_solid: candidate.all_solid,
                    end: candidate.end,
                    normal: candidate.plane.map_or([0.0; 3], |plane| plane.normal),
                    hit: Some(identity),
                    player_hit: player,
                    sky: false,
                };
            }
            Ok(())
        };
        for actor in self.projectile_target(PLAYER_IDENTITY).into_iter().chain(
            self.bots
                .as_ref()
                .into_iter()
                .flat_map(|bots| bots.combat_targets()),
        ) {
            bounds(u64::from(actor.identity), actor.position, actor.hull, true)?;
        }
        for other in &self.projectiles {
            if other.presentation.identity != projectile
                && other.presentation.kind.uses_rigid_physics()
                && other.solid
            {
                bounds(
                    u64::from(other.presentation.identity),
                    other.presentation.position,
                    Hull {
                        mins: [-2.0; 3],
                        maxs: [2.0; 3],
                    },
                    false,
                )?;
            }
        }
        Ok(trace)
    }
    pub(super) fn detonate_grenade(
        &mut self,
        mut projectile: LiveProjectile,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<MapPhase, Error> {
        let start = add(projectile.presentation.position, [0.0, 0.0, 8.0]);
        let trace = self.grenade_solid_trace(
            projectile.presentation.identity,
            start,
            add(start, [0.0, 0.0, -32.0]),
            playsrc_collision::MASK_SHOT_HULL,
        )?;
        if trace.fraction != 1.0 {
            self.entity_queries.set_origin(entity_queries::Entity::Projectile(projectile.presentation.identity),
                &mut projectile.presentation.position, add(trace.end, trace.normal)).map_err(Error::ProjectileTrace)?;
        }
        if projectile.presentation.contact_normal.is_none() && trace.normal != [0.0; 3] {
            projectile.presentation.contact_normal = Some(trace.normal);
        }
        self.explode(projectile, projectile_events, events)
    }
    pub fn install_rigid_world(
        &mut self,
        mut world: RigidWorld,
        models: RigidProjectileModels,
        brushes: &playsrc_collision::PhysicalModelInventory,
        studio: &BTreeMap<String, StudioRigidResource>,
    ) -> Result<(), Error> {
        if self.tick != 0
            || self.rigid.is_some()
            || !self.projectiles.is_empty()
            || world.physics().config().timestep.to_bits()
                != self.movement_configuration.tick_interval.to_bits()
        {
            return Err(Error::InvalidProjectilePhysics);
        }
        let static_queries = self.collision.static_query_bounds()?;
        let mut map_bindings = Vec::new();
        for (source, definition, transform, solid, parented) in self.map.rigid_entities() {
            let handle = self
                .map
                .source_handle(source)
                .ok_or(Error::MissingEntity(source))?;
            let Some(class) = definition.classname.as_deref() else {
                continue;
            };
            if let Some(index) = definition.bsp_model_index {
                if !map_runtime::rigid_brush_class(class) {
                    continue;
                }
                let physical = brushes.model(index).ok_or(Error::MissingRigidResources)?;
                let collision_bounds = Hull { mins: physical.authored_bounds.mins.map(|value| value - 1.0), maxs: physical.authored_bounds.maxs.map(|value| value + 1.0) };
                if self.map.brush_query_data(handle).map_err(Error::Entity)?.map(|(_, bounds)| bounds) != Some(collision_bounds) {
                    return Err(Error::MissingRigidResources);
                }
                let resource = RigidModel::compile_brush(
                    physical,
                    world.surfaces(),
                )
                .map_err(|error| Error::Rigid(error.into()))?;
                let body = world
                    .create_map_body(
                        &resource,
                        MapBodyInput {
                            entity: source,
                            handle,
                            kind: MapBodyKind::Shadow,
                            position: transform.origin,
                            angles: transform.angles,
                            solid,
                        },
                    )
                    .map_err(Error::Rigid)?;
                map_bindings.push(MapBinding {
                    entity: source,
                    handle: Some(handle),
                    query_handle: None,
                    collision_bounds,
                    axis_aligned_bounds: false,
                    blocks_los: true,
                    model: MapBindingModel::Body(BoundBody {
                        identity: Some(body),
                        resource,
                        kind: MapBodyKind::Shadow,
                        query: None,
                    }),
                });
                continue;
            }
            if ![
                b"prop_dynamic".as_slice(),
                b"dynamic_prop",
                b"prop_dynamic_override",
                b"training_prop_dynamic",
            ]
            .iter()
            .any(|name| class.eq_ignore_ascii_case(name))
            {
                continue;
            }
            let Some(path) = definition
                .model
                .as_ref()
                .and_then(|path| std::str::from_utf8(path).ok())
            else {
                continue;
            };
            let Some(resource) = studio.get(&path.to_ascii_lowercase()) else {
                continue;
            };
            let collision_bounds = Hull {
                mins: resource.model.collision_bounds[0].0.map(|value| f32::from_bits(value.0)),
                maxs: resource.model.collision_bounds[1].0.map(|value| f32::from_bits(value.0)),
            };
            let axis_aligned_bounds = definition.pairs.iter().rev().find(|pair| pair.key.eq_ignore_ascii_case(b"solid"))
                .is_some_and(|pair| playsrc_keyvalues::NumericValue::Bytes(&pair.value).get_int() == 2);
            let mut selected = Vec::new();
            if let Some(names) = &resource.follower_bones {
                for name in names {
                    let requested = resource
                        .model
                        .bones
                        .iter()
                        .position(|bone| bone.name.eq_ignore_ascii_case(name))
                        .ok_or(Error::MissingRigidResources)?;
                    let index = resource.model.bones[requested].physics_bone;
                    let solid = resource
                        .physics
                        .key_data
                        .solid_properties(Some(index))
                        .map_err(|_| Error::MissingRigidResources)?
                        .ok_or(Error::MissingRigidResources)?;
                    let bone = resource
                        .model
                        .bones
                        .iter()
                        .position(|bone| bone.name.eq_ignore_ascii_case(solid.name))
                        .unwrap_or(requested);
                    selected.push((index, bone, false));
                }
            }
            if selected.is_empty() && resource.physics.solids.len() > 1 {
                for (ordinal, _) in resource.physics.key_data.blocks.iter().enumerate() {
                    let Some(solid) = resource
                        .physics
                        .key_data
                        .solid_properties_at(ordinal)
                        .map_err(|_| Error::MissingRigidResources)?
                    else {
                        continue;
                    };
                    let bone = resource
                        .model
                        .bones
                        .iter()
                        .position(|bone| bone.name.eq_ignore_ascii_case(solid.name))
                        .ok_or(Error::MissingRigidResources)?;
                    selected.push((solid.index, bone, true));
                }
            }
            if selected.is_empty() {
                let shape = *resource
                    .shape_identities
                    .first()
                    .ok_or(Error::MissingRigidResources)?;
                let physics = RigidModel::compile(
                    shape,
                    &resource.physics,
                    None,
                    world.surfaces(),
                    playsrc_collision::SnapshotLimits::default(),
                )
                .map_err(|error| Error::Rigid(error.into()))?;
                let kind = if parented {
                    MapBodyKind::ParentedShadow
                } else {
                    MapBodyKind::Static
                };
                let body = world
                    .create_map_body(
                        &physics,
                        MapBodyInput {
                            entity: source,
                            handle,
                            kind,
                            position: transform.origin,
                            angles: transform.angles,
                            solid,
                        },
                    )
                    .map_err(Error::Rigid)?;
                map_bindings.push(MapBinding {
                    entity: source,
                    handle: Some(handle),
                    query_handle: None,
                    collision_bounds,
                    axis_aligned_bounds,
                    blocks_los: prop_blocks_los(collision_bounds),
                    model: MapBindingModel::Body(BoundBody {
                        identity: Some(body),
                        query: Some(Arc::new(playsrc_physics::ShapeCastModel::new(Arc::clone(physics.shape()))
                            .map_err(|error| Error::Rigid(playsrc_physics::EnvironmentError::from(error).into()))?)),
                        resource: physics,
                        kind,
                    }),
                });
            } else {
                let (sequence, cycle, think) =
                    self.map.rigid_animation(source).unwrap_or((0, 0.0, 0.0));
                let pose = follower_pose(&resource.model, transform, sequence, cycle, 0.0)?;
                let mut bodies = Vec::new();
                for (index, bone, zero_defaults) in selected {
                    let shape = *resource
                        .shape_identities
                        .get(usize::try_from(index).map_err(|_| Error::MissingRigidResources)?)
                        .ok_or(Error::MissingRigidResources)?;
                    let physics = if zero_defaults {
                        RigidModel::compile_ragdoll_follower(
                            shape,
                            &resource.physics,
                            index,
                            world.surfaces(),
                            playsrc_collision::SnapshotLimits::default(),
                        )
                    } else {
                        RigidModel::compile(
                            shape,
                            &resource.physics,
                            Some(index),
                            world.surfaces(),
                            playsrc_collision::SnapshotLimits::default(),
                        )
                    }
                    .map_err(|error| Error::Rigid(error.into()))?;
                    let (position, angles) = playsrc_studio_model::source_transform_components(
                        *pose
                            .bone_matrices
                            .get(bone)
                            .ok_or(Error::MissingRigidResources)?,
                    )
                    .map_err(|_| Error::MissingRigidResources)?;
                    let body = world
                        .create_map_body(
                            &physics,
                            MapBodyInput {
                                entity: source,
                                handle,
                                kind: MapBodyKind::BoneFollower(index as usize),
                                position: position.0.map(|v| f32::from_bits(v.0)),
                                angles: angles.0.map(|v| f32::from_bits(v.0)),
                                solid,
                            },
                        )
                        .map_err(Error::Rigid)?;
                    bodies.push((
                        BoundBody {
                            identity: Some(body),
                            query: Some(Arc::new(playsrc_physics::ShapeCastModel::new(Arc::clone(physics.shape()))
                                .map_err(|error| Error::Rigid(playsrc_physics::EnvironmentError::from(error).into()))?)),
                            resource: physics,
                            kind: MapBodyKind::BoneFollower(index as usize),
                        },
                        bone,
                    ));
                }
                map_bindings.push(MapBinding {
                    entity: source,
                    handle: Some(handle),
                    query_handle: None,
                    collision_bounds,
                    axis_aligned_bounds,
                    blocks_los: prop_blocks_los(collision_bounds),
                    model: MapBindingModel::Followers {
                        model: Arc::clone(&resource.model),
                        bodies,
                        last: Some((transform, sequence, cycle.to_bits(), think.to_bits())),
                        solid,
                    },
                });
            }
        }
        self.rigid = Some(State {
            world,
            models,
            map_bindings,
            impact_sounds: Vec::new(),
            impact_sound_time: 0.0,
        });
        let changes = self.rigid.as_ref().unwrap().map_bindings.iter().filter_map(|binding| {
            let handle = binding.handle?;
            let state = self.map.collision_entity(binding.entity)?;
            Some(map_runtime::QueryChange::State { source: binding.entity, handle, state })
        }).collect();
        let previous_queries = self.entity_queries.clone();
        let initialized = (|| {
            for (identity, bounds) in static_queries { self.entity_queries.register_static(identity, bounds).map_err(Error::ProjectileTrace)?; }
            self.apply_map_query_changes(changes)
        })();
        if let Err(error) = initialized {
            self.entity_queries = previous_queries;
            self.rigid = None;
            return Err(error);
        }
        self.map.take_query_changes();
        Ok(())
    }
    pub fn rigid_world(&self) -> Option<&RigidWorld> {
        self.rigid.as_ref().map(|state| &state.world)
    }
    pub(super) fn retain_rigid_entity(&mut self, projectile: LiveProjectile) {
        let order = (projectile.creation_tick, projectile.presentation.identity);
        let index = self
            .projectiles
            .partition_point(|other| (other.creation_tick, other.presentation.identity) < order);
        self.projectiles.insert(index, projectile);
    }
    pub(super) fn fizzle_launcher_stickies(&mut self, events: &mut Vec<ProjectileEvent>) {
        let Some(launcher) = self.weapon_source(PLAYER_IDENTITY, Weapon::StickybombLauncher) else {
            return;
        };
        for projectile in &mut self.projectiles {
            if !projectile.launcher_linked
                || projectile.presentation.launcher_source != Some(launcher)
                || projectile.retiring
            {
                continue;
            }
            if projectile.solid {
                events.push(projectile_event(
                    ProjectileEventKind::Fizzle,
                    &projectile.presentation,
                    self.tick,
                ));
            }
            projectile.retiring = true;
            projectile.next_think_tick = self.tick;
            projectile.detonate_on_think = false;
        }
    }
    pub(super) fn create_rigid_projectile(
        &mut self,
        projectile: &Projectile,
        angles: [f32; 3],
    ) -> Result<(), Error> {
        let rigid = self.rigid.as_mut().ok_or(Error::MissingRigidResources)?;
        rigid
            .world
            .create_projectile(
                if projectile.weapon == Weapon::GrenadeLauncher {
                    &rigid.models.grenade
                } else {
                    &rigid.models.sticky
                },
                ProjectileBodyInput {
                    projectile: projectile.identity,
                    position: projectile.position,
                    angles,
                    velocity: projectile.velocity,
                    angular_velocity: projectile.angular_velocity,
                },
            )
            .map_err(Error::Rigid)?;
        Ok(())
    }
    pub(super) fn destroy_rigid_projectile(&mut self, projectile: u32) -> Result<(), Error> {
        self.rigid
            .as_mut()
            .ok_or(Error::MissingRigidResources)?
            .world
            .destroy_projectile(projectile)
            .map_err(Error::Rigid)?;
        self.entity_queries.destroy(entity_queries::Entity::Projectile(projectile)).map_err(Error::ProjectileTrace)
    }
    pub(super) fn advance_rigid_projectiles(
        &mut self,
        projectile_events: &mut Vec<ProjectileEvent>,
        events: &mut Vec<Event>,
    ) -> Result<MapPhase, Error> {
        let mut phase = MapPhase::default();
        self.sync_map_queries()?;
        if self
            .projectiles
            .iter()
            .any(|projectile| projectile.presentation.kind.uses_rigid_physics())
        {
            self.refresh_player_hitboxes()?;
        }
        let Some(mut rigid) = self.rigid.take() else {
            return if self
                .projectiles
                .iter()
                .any(|projectile| projectile.presentation.kind.uses_rigid_physics())
            {
                Err(Error::MissingRigidResources)
            } else {
                Ok(phase)
            };
        };
        let now = self.tick as f32 * self.movement_configuration.tick_interval;
        // Retire the old roster before creating any replacement entity bodies.
        for binding in &mut rigid.map_bindings {
            let current = self
                .map
                .source_handle(binding.entity)
                .filter(|_| self.map.collision_entity(binding.entity).is_some());
            if binding.handle == current {
                continue;
            }
            let mut destroy = |body: &mut BoundBody| -> Result<(), Error> {
                if let Some(identity) = body.identity.take() {
                    rigid
                        .world
                        .destroy_map_body(identity)
                        .map_err(Error::Rigid)?;
                }
                Ok(())
            };
            match &mut binding.model {
                MapBindingModel::Body(body) => destroy(body)?,
                MapBindingModel::Followers { bodies, last, .. } => {
                    for (body, _) in bodies {
                        destroy(body)?;
                    }
                    *last = None;
                }
            }
            binding.handle = current;
        }
        for binding in &mut rigid.map_bindings {
            let Some(handle) = binding.handle else {
                continue;
            };
            let state = self
                .map
                .collision_entity(binding.entity)
                .ok_or(Error::MissingEntity(binding.entity))?;
            let transform = state.transform;
            match &mut binding.model {
                MapBindingModel::Body(body) => {
                    let identity = if let Some(identity) = body.identity {
                        identity
                    } else {
                        let identity = rigid
                            .world
                            .create_map_body(
                                &body.resource,
                                MapBodyInput {
                                    entity: binding.entity,
                                    handle,
                                    kind: body.kind,
                                    position: transform.origin,
                                    angles: transform.angles,
                                    solid: state.enabled,
                                },
                            )
                            .map_err(Error::Rigid)?;
                        body.identity = Some(identity);
                        identity
                    };
                    rigid
                        .world
                        .update_map_body(
                            identity,
                            transform.origin,
                            transform.angles,
                            state.enabled,
                            self.movement_configuration.tick_interval,
                        )
                        .map_err(Error::Rigid)?;
                }
                MapBindingModel::Followers {
                    model,
                    bodies,
                    last,
                    solid,
                } => {
                    if bodies.iter().all(|(body, _)| body.identity.is_none()) {
                        *solid = state.enabled;
                    }
                    let (sequence, cycle, think) = self
                        .map
                        .rigid_animation(binding.entity)
                        .unwrap_or((0, 0.0, 0.0));
                    let key = (transform, sequence, cycle.to_bits(), think.to_bits());
                    if last.as_ref() == Some(&key) {
                        continue;
                    }
                    let pose = follower_pose(model, transform, sequence, cycle, now)?;
                    for (body, bone) in bodies {
                        let (position, angles) = playsrc_studio_model::source_transform_components(
                            *pose
                                .bone_matrices
                                .get(*bone)
                                .ok_or(Error::MissingRigidResources)?,
                        )
                        .map_err(|_| Error::MissingRigidResources)?;
                        let position = position.0.map(|v| f32::from_bits(v.0));
                        let angles = angles.0.map(|v| f32::from_bits(v.0));
                        let identity = if let Some(identity) = body.identity {
                            identity
                        } else {
                            let identity = rigid
                                .world
                                .create_map_body(
                                    &body.resource,
                                    MapBodyInput {
                                        entity: binding.entity,
                                        handle,
                                        kind: body.kind,
                                        position,
                                        angles,
                                        solid: *solid,
                                    },
                                )
                                .map_err(Error::Rigid)?;
                            body.identity = Some(identity);
                            identity
                        };
                        rigid
                            .world
                            .update_map_body(identity, position, angles, *solid, 0.1)
                            .map_err(Error::Rigid)?;
                    }
                    *last = Some(key);
                }
            }
        }
        rigid.world.simulate().map_err(Error::Rigid)?;
        let mut disable = Vec::new();
        for callback_index in 0..rigid.world.physics().callbacks().len() {
            let callback = rigid.world.physics().callbacks()[callback_index];
            let PhysicsCallbackKind::PostCollision { collision, speed } = callback.kind else {
                continue;
            };
            if !collision.collision {
                continue;
            }
            let Some(contact) = callback.contact else {
                return Err(Error::InvalidProjectilePhysics);
            };
            for side in 0..2 {
                let (Some(body), Some(other)) = (callback.bodies[side], callback.bodies[1 - side])
                else {
                    return Err(Error::InvalidProjectilePhysics);
                };
                let owner = rigid.world.owner(body.identity);
                let other_owner = rigid.world.owner(other.identity);
                if !matches!(owner, None | Some(BodyOwner::World))
                    && (matches!(owner, Some(BodyOwner::Projectile(_))) || matches!(other_owner, Some(BodyOwner::Projectile(_)))) {
                    rigid.impact_sound(collision.materials[side], collision.materials[1 - side], body.position, collision.elapsed, speed);
                }
                let Some(BodyOwner::Projectile(identity)) = owner else {
                    continue;
                };
                let Some(projectile) = self
                    .projectiles
                    .iter_mut()
                    .find(|value| value.presentation.identity == identity)
                else {
                    continue;
                };
                let normal = contact.normal.map(|value| -value);
                if !projectile.retiring {
                    projectile_events.push(projectile_event_with_contact(
                        ProjectileEventKind::Impact,
                        &projectile.presentation,
                        self.tick,
                        Some(normal),
                    ));
                }
                if projectile.presentation.kind == ProjectileKind::Grenade {
                    if projectile.touched_time.is_none()
                        && matches!(rigid.world.owner(other.identity),Some(BodyOwner::MapEntity(entity)) if self.map.grenade_sensitive_prop(entity))
                    {
                        projectile.detonate_on_think = true;
                        projectile.next_think_tick = self.tick;
                    }
                    projectile.touched_time = Some(now);
                    continue;
                }
                if rigid.world.game_material(collision.materials[1 - side]) == Some(b'X') {
                    continue;
                }
                let adheres = match rigid.world.owner(other.identity) {
                    Some(BodyOwner::World) => true,
                    Some(BodyOwner::MapEntity(entity)) => self.map.sticky_adhesion_prop(entity),
                    _ => false,
                };
                if adheres && now > projectile.minimum_sleep_time {
                    projectile.presentation.contact_normal = Some(normal);
                    projectile.touched_time = Some(now);
                    projectile.presentation.state = if projectile.armed {
                        ProjectileState::StuckArmed
                    } else {
                        ProjectileState::StuckUnarmed
                    };
                    disable.push(identity);
                }
            }
        }
        let active = rigid
            .world
            .physics()
            .active_bodies()
            .map(|body| body.identity())
            .collect::<Vec<_>>();
        for identity in active {
            let Some(physical) = rigid.world.physics().body(identity) else {
                continue;
            };
            let body = physical
                .published()
                .map_err(|error| Error::Rigid(error.into()))?;
            if let Some(BodyOwner::BoneFollower { entity, solid }) = rigid.world.owner(body.identity) {
                let parent = rigid.map_bindings.iter().find(|binding| binding.entity == entity).and_then(|binding| binding.query_handle).ok_or(Error::InvalidProjectilePhysics)?;
                self.entity_queries.set_transform(entity_queries::Entity::Follower { parent, solid: u16::try_from(solid).map_err(|_| Error::MissingRigidResources)? },
                    playsrc_entity::Transform { origin: body.position, angles: body.angles }).map_err(Error::ProjectileTrace)?;
                continue;
            }
            let Some(BodyOwner::Projectile(identity)) = rigid.world.owner(body.identity) else {
                continue;
            };
            let Some(projectile) = self
                .projectiles
                .iter_mut()
                .find(|value| value.presentation.identity == identity)
            else {
                continue;
            };
            if body
                .position
                .iter()
                .all(|value| *value > -16384.0 && *value < 16384.0)
            {
                self.entity_queries.set_origin(entity_queries::Entity::Projectile(identity), &mut projectile.presentation.position, body.position)
                    .map_err(Error::ProjectileTrace)?;
            }
            let angles = body.angles.map(|value| {
                let mut angle = value % 360.0;
                if angle > 180.0 {
                    angle -= 360.0;
                }
                if angle < -180.0 {
                    angle += 360.0;
                }
                angle
            });
            projectile.presentation.orientation =
                quaternion_from_angles(angles[0], angles[1], angles[2]);
            projectile.presentation.velocity = body.linear_velocity;
            projectile.presentation.angular_velocity = body.angular_velocity;
            let start = projectile.presentation.position;
            let end = add(
                start,
                scale(
                    body.linear_velocity,
                    self.movement_configuration.tick_interval,
                ),
            );
            self.entity_queries.flush_bound().map_err(Error::ProjectileTrace)?;
            let hit = self
                .collision
                .trace_grenade_entities(
                    start,
                    end,
                    projectile.presentation.owner_identity,
                    &self.posed_player_hitboxes,
                )
                .map_err(Error::Movement)?;
            let Some(hit) = hit else {
                projectile.in_solid = false;
                continue;
            };
            let enemy = matches!(
                (projectile.presentation.team, hit.team),
                (PlayerTeam::Red, PlayerTeam::Blue) | (PlayerTeam::Blue, PlayerTeam::Red)
            );
            let friendly =
                hit.team == projectile.presentation.team && projectile.teammate_collision_enabled;
            let mut removed = false;
            if hit.combat_item && enemy && !projectile.retiring {
                let position = if hit.fraction != 1.0 {
                    add(hit.end, hit.normal)
                } else {
                    start
                };
                self.entity_queries.set_origin(entity_queries::Entity::Projectile(identity), &mut projectile.presentation.position, position)
                    .map_err(Error::ProjectileTrace)?;
                let index = self
                    .projectiles
                    .iter()
                    .position(|value| value.presentation.identity == identity)
                    .ok_or(Error::InvalidProjectilePhysics)?;
                let projectile = self.projectiles.remove(index);
                self.rigid = Some(rigid);
                phase.append(self.explode(projectile, projectile_events, events)?);
                rigid = self.rigid.take().ok_or(Error::MissingRigidResources)?;
                continue;
            }
            if projectile.presentation.kind == ProjectileKind::Grenade
                && !projectile.retiring
                && (hit.start_solid && enemy || !hit.start_solid && hit.fraction < 1.0)
            {
                let direction =
                    playsrc_physics::normalize_source_vector(projectile.rigid_entity_velocity)
                        .map_err(|_| Error::InvalidProjectilePhysics)?;
                let start = sub(projectile.presentation.position, scale(direction, 32.0));
                let touched = projectile.touched_time.is_some();
                let trace = self.grenade_solid_trace(
                    identity,
                    start,
                    add(start, scale(direction, 64.0)),
                    playsrc_collision::MASK_SOLID,
                )?;
                if trace.fraction < 1.0 && trace.sky {
                    let index = self
                        .projectiles
                        .iter()
                        .position(|value| value.presentation.identity == identity)
                        .ok_or(Error::InvalidProjectilePhysics)?;
                    let projectile = self.projectiles.remove(index);
                    rigid
                        .world
                        .destroy_projectile(identity)
                        .map_err(Error::Rigid)?;
                    self.entity_queries.destroy(entity_queries::Entity::Projectile(identity)).map_err(Error::ProjectileTrace)?;
                    projectile_events.push(projectile_event(
                        ProjectileEventKind::Fizzle,
                        &projectile.presentation,
                        self.tick,
                    ));
                    removed = true;
                }
                if enemy && !touched && !removed {
                    let projectile = self
                        .projectiles
                        .iter_mut()
                        .find(|value| value.presentation.identity == identity)
                        .ok_or(Error::InvalidProjectilePhysics)?;
                    projectile.presentation.damage = projectile.full_damage;
                    projectile.direct_target = Some(hit.identity);
                    if trace.fraction != 1.0 {
                        self.entity_queries.set_origin(entity_queries::Entity::Projectile(identity), &mut projectile.presentation.position, add(trace.end, trace.normal))
                            .map_err(Error::ProjectileTrace)?;
                    }
                    projectile.presentation.contact_normal =
                        (trace.normal != [0.0; 3]).then_some(trace.normal);
                    let index = self
                        .projectiles
                        .iter()
                        .position(|value| value.presentation.identity == identity)
                        .ok_or(Error::InvalidProjectilePhysics)?;
                    let projectile = self.projectiles.remove(index);
                    self.rigid = Some(rigid);
                    phase.append(self.explode(projectile, projectile_events, events)?);
                    rigid = self.rigid.take().ok_or(Error::MissingRigidResources)?;
                }
            }
            let projectile = self
                .projectiles
                .iter_mut()
                .find(|value| value.presentation.identity == identity);
            if hit.start_solid {
                if let Some(projectile) = projectile {
                    if !enemy && !projectile.in_solid && friendly {
                        rigid
                            .world
                            .set_body_velocity(
                                body.identity,
                                Some(scale(body.linear_velocity, -0.2)),
                                None,
                            )
                            .map_err(Error::Rigid)?;
                    }
                    projectile.in_solid = true;
                }
                continue;
            }
            if let Some(projectile) = projectile {
                projectile.in_solid = false;
            }
            if hit.fraction < 1.0 && (enemy || friendly) {
                let dot = dot(body.linear_velocity, hit.normal);
                let mut velocity = add(scale(hit.normal, -2.0 * dot), body.linear_velocity);
                velocity = scale(velocity, 0.45);
                if enemy {
                    velocity = scale(velocity, 0.5);
                }
                rigid
                    .world
                    .set_body_velocity(
                        body.identity,
                        Some(velocity),
                        Some(scale(body.angular_velocity, -0.5)),
                    )
                    .map_err(Error::Rigid)?;
            }
        }
        rigid.world.finish_frame().map_err(Error::Rigid)?;
        for identity in disable {
            rigid
                .world
                .set_projectile_motion(identity, false)
                .map_err(Error::Rigid)?;
            if let Some(projectile) = self
                .projectiles
                .iter_mut()
                .find(|value| value.presentation.identity == identity)
            {
                if projectile.motion_enabled && !projectile.retiring {
                    projectile_events.push(projectile_event(
                        ProjectileEventKind::Stick,
                        &projectile.presentation,
                        self.tick,
                    ));
                }
                projectile.motion_enabled = false;
            }
        }
        self.rigid = Some(rigid);
        self.rigid
            .as_mut()
            .ok_or(Error::MissingRigidResources)?
            .world
            .cleanup_delete_list()
            .map_err(Error::Rigid)?;
        Ok(phase)
    }
}

fn follower_pose(
    model: &playsrc_studio_model::PresentationModel,
    transform: playsrc_entity::Transform,
    sequence: usize,
    cycle: f32,
    time: f32,
) -> Result<playsrc_studio_model::SampledWorldPose, Error> {
    use playsrc_studio_model::{AnimationState, Float32, Vector3};
    let state = AnimationState {
        base_sequence: sequence,
        cycle: Float32(cycle.to_bits()),
        pose_parameters: vec![Float32(0); model.pose_parameters.len()],
        layers: Vec::new(),
        bone_rotations: Vec::new(),
    };
    let pose = playsrc_studio_model::sample_pose_at_time(model, &state, Float32(time.to_bits()))
        .map_err(|_| Error::MissingRigidResources)?;
    let matrix = playsrc_studio_model::source_entity_transform(
        Vector3(transform.origin.map(|v| Float32(v.to_bits()))),
        Vector3(transform.angles.map(|v| Float32(v.to_bits()))),
    )
    .map_err(|_| Error::MissingRigidResources)?;
    playsrc_studio_model::apply_entity_transform(model, &pose, matrix)
        .map_err(|_| Error::MissingRigidResources)
}

#[cfg(test)]
#[path = "../tests/support/rigid_resources.rs"]
mod configured_resources;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the configured TF2 archives and jump_beef cache object"]
    fn configured_map_generations_replace_bodies_without_recompiling_shapes() {
        let (collision, models, surfaces, projectiles) = configured_resources::load();
        let graph = playsrc_entity::parse(
            br#"
            {"classname" "team_control_point_master"}
            {"classname" "team_control_point" "targetname" "point"}
            {"classname" "func_brush" "targetname" "physical_brush" "model" "*26"}
        "#,
            Default::default(),
        )
        .unwrap();
        let bounds = collision
            .models
            .iter()
            .enumerate()
            .map(|(model, bounds)| playsrc_entity::ModelBounds {
                model,
                mins: [bounds.mins.x, bounds.mins.y, bounds.mins.z].map(|v| v.value()),
                maxs: [bounds.maxs.x, bounds.maxs.y, bounds.maxs.z].map(|v| v.value()),
            })
            .collect();
        let map = MapRuntime::compile(&graph, 0.015, 1, bounds).unwrap();
        let config = playsrc_physics::EnvironmentConfig {
            timestep: 0.015,
            gravity: [0.0, 0.0, -800.0],
            air_density: 2.0,
            max_bodies: 128,
            max_events: 16384,
            random_seed: 1,
            performance: playsrc_physics::PerformanceSettings {
                max_collisions_per_body: 10,
                ..Default::default()
            },
        };
        let world = RigidWorld::from_world_model(config, &collision, &models, surfaces).unwrap();
        let mut session = Session::new(crate::tests::Floor, [5328.0, 3376.0, -3118.0], map);
        session
            .install_rigid_world(world, projectiles, &models, &BTreeMap::new())
            .unwrap();
        session.flush_entity_queries().unwrap();
        let original = match &session.rigid.as_ref().unwrap().map_bindings[0].model {
            MapBindingModel::Body(body) => Arc::clone(body.resource.shape()),
            _ => panic!("brush binding"),
        };
        let rigid = session.rigid.as_mut().unwrap();
        let MapBindingModel::Body(body) = &rigid.map_bindings[0].model else {
            panic!("brush binding")
        };
        let before = rigid.world.physics().snapshot();
        assert_eq!(
            rigid.world.create_map_body(
                &body.resource,
                MapBodyInput {
                    entity: 2,
                    handle: playsrc_entity::EntityHandle::NULL,
                    kind: body.kind,
                    position: [0.0; 3],
                    angles: [0.0; 3],
                    solid: true
                }
            ),
            Err(crate::rigid_world::Error::MissingEntity)
        );
        assert_eq!(rigid.world.physics().snapshot(), before);
        for _ in 0..3 {
            let prior = session.map.source_handle(2).unwrap();
            let old = {
                let rigid = &session.rigid.as_ref().unwrap().world;
                rigid
                    .physics()
                    .bodies()
                    .iter()
                    .find(|body| rigid.owner(body.identity()) == Some(BodyOwner::MapEntity(2)))
                    .unwrap()
                    .identity()
            };
            session.tick += 1;
            session.map.restart_control_point_map(session.tick).unwrap();
            let current = session.map.source_handle(2).unwrap();
            assert_ne!(current, prior);
            session.flush_entity_queries().unwrap();
            let bounds = models.model(26).unwrap().authored_bounds;
            let center = scale(add(bounds.mins, bounds.maxs), 0.5);
            let selected = session.entity_queries.sphere(center, 0.0, 0x10, 64, |entity| matches!(entity, entity_queries::Entity::Map(_))).unwrap();
            assert!(selected.contains(&entity_queries::Entity::Map(current)));
            assert!(!selected.contains(&entity_queries::Entity::Map(prior)));
            session
                .rigid
                .as_mut()
                .unwrap()
                .world
                .begin_frame(session.tick as f32 * 0.015)
                .unwrap();
            session
                .advance_rigid_projectiles(&mut Vec::new(), &mut Vec::new())
                .unwrap();
            let rigid = session.rigid.as_ref().unwrap();
            assert!(rigid.world.owner(old).is_none());
            assert!(rigid.world.physics().body(old).is_none());
            assert_eq!(rigid.map_bindings[0].handle, Some(current));
            let MapBindingModel::Body(body) = &rigid.map_bindings[0].model else {
                panic!("brush binding")
            };
            assert!(body.identity.unwrap() > old);
            assert!(Arc::ptr_eq(&original, body.resource.shape()));
        }
    }
}
