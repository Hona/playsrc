use playsrc_collision::Hull;
use playsrc_movement::{Error as MoveError, FailureKind, Operation, Trace, Tracer};
use playsrc_physics::{EnvironmentConfig, PerformanceSettings};
use playsrc_tf2::rigid_world::RigidWorld;
use playsrc_tf2::{
    Command, GameplayWorld, GrenadeEntityHit, MapRuntime, PlayerClass, PlayerTeam,
    PosedPlayerHitbox, ProjectileEventKind, ProjectileKind, ProjectileState, Session, Weapon,
};
use playsrc_tf2::{RigidProjectileModels, rigid_body::RigidModel};
use std::{collections::BTreeMap, sync::Arc};
#[path = "support/rigid_resources.rs"]
mod rigid_resources;

#[derive(Clone)]
struct World(Arc<playsrc_collision::World>);
impl Tracer for World {
    fn trace(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, MoveError> {
        let trace = self.0.trace_hull(start, end, hull, mask).map_err(|_| {
            MoveError::new(Operation::Trace, FailureKind::Malformed, "configured trace")
        })?;
        Ok(Trace {
            fraction: trace.fraction,
            start_solid: trace.start_solid,
            all_solid: trace.all_solid,
            end: trace.end,
            normal: trace.plane.map(|p| p.normal),
            hit: trace.did_hit().then_some(0),
            contents: trace.contents,
        })
    }
    fn point_contents(&self, point: [f32; 3]) -> Result<u32, MoveError> {
        Ok(self
            .0
            .point_contents(point)
            .map_err(|_| {
                MoveError::new(
                    Operation::Trace,
                    FailureKind::Malformed,
                    "configured contents",
                )
            })?
            .contents)
    }
}
impl GameplayWorld for World {
    fn static_query_bounds(&self) -> Result<Vec<(u64, Hull)>, MoveError> { Ok(Vec::new()) }
    fn trace_projectile_solid(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        mask: u32,
    ) -> Result<playsrc_tf2::ProjectileSolidTrace, MoveError> {
        self.0
            .trace_hull(
                start,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                mask,
            )
            .map(Into::into)
            .map_err(|_| {
                MoveError::new(
                    Operation::Trace,
                    FailureKind::Malformed,
                    "configured solid ray",
                )
            })
    }
    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, MoveError> {
        self.0
            .overlaps_model_hull(model, origin, position, hull)
            .map_err(|_| {
                MoveError::new(
                    Operation::Trace,
                    FailureKind::Malformed,
                    "configured overlap",
                )
            })
    }
    fn trace_grenade_entities(
        &self,
        _start: [f32; 3],
        _end: [f32; 3],
        _thrower: u32,
        hitboxes: &[PosedPlayerHitbox],
    ) -> Result<Option<GrenadeEntityHit>, MoveError> {
        assert!(hitboxes.is_empty());
        Ok(None)
    }
}
fn session() -> Session<World> {
    session_with_entities(None)
}
fn session_with_entities(graph: Option<&playsrc_entity::Graph>) -> Session<World> {
    let (collision, models, registry, projectiles) = rigid_resources::load();
    let config = EnvironmentConfig {
        random_seed: 1,
        gravity: [0.0, 0.0, -800.0],
        air_density: 2.0,
        timestep: 0.015,
        max_bodies: 128,
        max_events: 16384,
        performance: PerformanceSettings {
            max_collisions_per_body: 10,
            ..PerformanceSettings::default()
        },
    };
    let rigid =
        RigidWorld::from_world_model(config, &collision, &models, Arc::clone(&registry)).unwrap();
    let map = graph.map_or_else(
        || MapRuntime::empty(0.015),
        |graph| {
            MapRuntime::compile(
                graph,
                0.015,
                1,
                collision
                    .models
                    .iter()
                    .enumerate()
                    .map(|(model, bounds)| playsrc_entity::ModelBounds {
                        model,
                        mins: [bounds.mins.x, bounds.mins.y, bounds.mins.z].map(|v| v.value()),
                        maxs: [bounds.maxs.x, bounds.maxs.y, bounds.maxs.z].map(|v| v.value()),
                    })
                    .collect(),
            )
            .unwrap()
        },
    );
    let mut session = Session::new(
        World(Arc::new(collision)),
        [5384.0, 3440.0, -2807.96875],
        map,
    );
    session
        .install_rigid_world(rigid, projectiles, &models, &BTreeMap::new())
        .unwrap();
    session
        .advance(Command {
            select_class: Some(PlayerClass::Demoman),
            select_weapon: Some(Weapon::StickybombLauncher),
            ..Command::default()
        })
        .unwrap();
    for _ in 0..40 {
        step(&mut session, false, false);
    }
    session
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn killed_authored_brush_retires_its_body_and_keeps_session_rollback_atomic() {
    let graph = playsrc_entity::parse(
        br#"{"classname" "func_brush" "targetname" "physical_brush" "model" "*26"}"#,
        Default::default(),
    )
    .unwrap();
    let mut session = session_with_entities(Some(&graph));
    let body = {
        let world = session.rigid_world().unwrap();
        world
            .physics()
            .bodies()
            .iter()
            .find(|body| {
                world.owner(body.identity())
                    == Some(playsrc_tf2::rigid_world::BodyOwner::MapEntity(0))
            })
            .unwrap()
            .identity()
    };
    session
        .fire_entity_input(b"physical_brush", b"Kill", b"", 0.0)
        .unwrap();
    let before = session.rigid_world().unwrap().physics().snapshot();
    assert!(
        session
            .advance(Command {
                pitch_degrees: f32::NAN,
                ..Default::default()
            })
            .is_err()
    );
    assert_eq!(session.rigid_world().unwrap().physics().snapshot(), before);
    session.advance(Command::default()).unwrap();
    let world = session.rigid_world().unwrap();
    assert!(world.owner(body).is_none());
    assert!(world.physics().body(body).is_none());
    for _ in 0..4 {
        session.advance(Command::default()).unwrap();
    }
}
fn step(session: &mut Session<World>, fire: bool, detonate: bool) -> playsrc_tf2::Snapshot {
    session
        .advance(Command {
            fire,
            detonate,
            pitch_degrees: 85.0,
            ..Command::default()
        })
        .unwrap()
}
fn launch(session: &mut Session<World>) -> u32 {
    for _ in 0..4 {
        step(session, true, false);
        if session
            .weapon_runtime(Weapon::StickybombLauncher)
            .unwrap()
            .charge_begin_tick
            .is_some()
        {
            break;
        }
    }
    assert!(
        session
            .weapon_runtime(Weapon::StickybombLauncher)
            .unwrap()
            .charge_begin_tick
            .is_some()
    );
    let fired = step(session, false, false);
    let event = fired
        .projectile_events
        .iter()
        .find(|e| e.kind == ProjectileEventKind::Fire)
        .expect("owned sticky launch");
    assert_eq!(event.projectile_kind, ProjectileKind::Sticky);
    assert!(
        session
            .rigid_world()
            .unwrap()
            .projectile_body(event.projectile)
            .is_some()
    );
    event.projectile
}
#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn authored_session_sticks_arms_detonates_and_rolls_back_owned_state() {
    let mut session = session();
    let identity = launch(&mut session);
    let mut stuck = false;
    let mut armed = false;
    for _ in 0..70 {
        let before = session.producer_snapshot();
        let physics = session.rigid_world().unwrap().physics().snapshot();
        let random = session.random_state();
        assert!(
            session
                .advance(Command {
                    pitch_degrees: f32::NAN,
                    ..Command::default()
                })
                .is_err()
        );
        assert_eq!(session.producer_snapshot(), before);
        assert_eq!(session.rigid_world().unwrap().physics().snapshot(), physics);
        assert_eq!(session.random_state(), random);
        let mut replay = session.clone();
        let actual = step(&mut session, false, false);
        let repeated = step(&mut replay, false, false);
        assert_eq!(actual, repeated);
        assert_eq!(
            session.rigid_world().unwrap().physics().snapshot(),
            replay.rigid_world().unwrap().physics().snapshot()
        );
        stuck |= actual
            .projectile_events
            .iter()
            .any(|e| e.kind == ProjectileEventKind::Stick);
        armed |= actual
            .projectile_events
            .iter()
            .any(|e| e.kind == ProjectileEventKind::Arm);
    }
    assert!(stuck && armed);
    let before = session.producer_snapshot();
    assert_eq!(before.projectiles[0].state, ProjectileState::StuckArmed);
    let body = session
        .rigid_world()
        .unwrap()
        .projectile_body(identity)
        .unwrap();
    assert!(
        !session
            .rigid_world()
            .unwrap()
            .physics()
            .body(body)
            .unwrap()
            .published()
            .unwrap()
            .motion_enabled
    );
    let exploded = step(&mut session, false, true);
    let explosion = exploded
        .projectile_events
        .iter()
        .find(|event| event.kind == ProjectileEventKind::Explode)
        .unwrap();
    let position = before.projectiles[0].position;
    assert_eq!(
        explosion.position,
        [position[0], position[1], position[2] + 8.0]
    );
    assert_eq!(
        explosion.contact_normal,
        before.projectiles[0].contact_normal
    );
    assert!(exploded.projectiles.is_empty());
    assert_eq!(session.pipebomb_count(1), 1);
    assert_eq!(session.producer_snapshot().pipebomb_count, 1);
    assert_eq!(
        session.rigid_world().unwrap().projectile_body(identity),
        Some(body)
    );
    let pending = session.rigid_world().unwrap().physics().snapshot();
    assert!(
        session
            .advance(Command {
                pitch_degrees: f32::NAN,
                ..Command::default()
            })
            .is_err()
    );
    assert_eq!(session.rigid_world().unwrap().physics().snapshot(), pending);
    let removed = step(&mut session, false, false);
    assert!(removed.projectile_events.is_empty());
    assert_eq!(session.pipebomb_count(1), 0);
    assert_eq!(session.producer_snapshot().pipebomb_count, 0);
    assert!(
        session
            .rigid_world()
            .unwrap()
            .projectile_body(identity)
            .is_none()
    );
    assert!(
        exploded
            .projectile_events
            .iter()
            .any(|e| e.kind == ProjectileEventKind::Explode)
    );
    assert!(exploded.health < 175.0);
    assert!(session.movement_state().velocity[2] > 500.0);
}
#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn lethal_sticky_blast_and_respawn_publish_the_current_launcher_without_stale_bodies() {
    let mut session = session();
    let first = launch(&mut session);
    for _ in 0..45 { step(&mut session, false, false); }
    let second = launch(&mut session);
    for _ in 0..45 { step(&mut session, false, false); }
    let third = launch(&mut session);
    for _ in 0..70 { step(&mut session, false, false); }
    assert_eq!(session.producer_snapshot().pipebomb_count, 3);
    let dead = step(&mut session, false, true);
    assert_eq!(dead.health, 0.0);
    assert_eq!(session.lifecycle(), playsrc_tf2::PlayerLifecycle::Dying);
    assert!(dead.projectiles.is_empty());
    assert_eq!(dead.projectile_events.iter().filter(|event| event.kind == ProjectileEventKind::Explode).count(), 3);
    let pending = [first, second, third].into_iter().filter(|id| session.rigid_world().unwrap().projectile_body(*id).is_some()).count();
    assert_eq!(session.producer_snapshot().pipebomb_count, pending);
    let mut replay = session.clone();
    let command = Command { respawn: true, ..Command::default() };
    assert_eq!(session.advance(command).unwrap(), replay.advance(command).unwrap());
    assert_eq!(session.lifecycle(), playsrc_tf2::PlayerLifecycle::Active);
    assert_eq!(session.producer_snapshot().pipebomb_count, 0);
    assert_eq!(session.producer_snapshot().charge_progress, None);
    for id in [first, second, third] { assert!(session.rigid_world().unwrap().projectile_body(id).is_none()); }
    assert_eq!(session.rigid_world().unwrap().physics().snapshot(), replay.rigid_world().unwrap().physics().snapshot());
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn authored_session_fizzles_on_team_change_and_retires_the_oldest_body() {
    let mut first = session();
    let identity = launch(&mut first);
    let fizzled = first
        .advance(Command {
            select_team: Some(PlayerTeam::Blue),
            ..Command::default()
        })
        .unwrap();
    assert!(fizzled.projectiles.is_empty());
    assert_eq!(first.producer_snapshot().pipebomb_count, 0);
    assert!(
        first
            .rigid_world()
            .unwrap()
            .projectile_body(identity)
            .is_none()
    );
    assert!(
        fizzled
            .projectile_events
            .iter()
            .any(|e| e.kind == ProjectileEventKind::Fizzle)
    );
    let mut second = session();
    let oldest = launch(&mut second);
    for index in 0..8 {
        for _ in 0..if index == 7 { 160 } else { 45 } {
            step(&mut second, false, false);
        }
        launch(&mut second);
    }
    assert_eq!(second.pipebomb_count(1), 8);
    assert_eq!(second.producer_snapshot().pipebomb_count, 8);
    assert_eq!(second.producer_snapshot().projectiles.len(), 9);
    for _ in 0..16 {
        step(&mut second, false, false);
    }
    assert!(
        second
            .rigid_world()
            .unwrap()
            .projectile_body(oldest)
            .is_none()
    );
    assert_eq!(second.producer_snapshot().projectiles.len(), 8);
    assert_eq!(second.producer_snapshot().pipebomb_count, 8);
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn remote_removal_uses_the_think_roster_captured_before_player_actions() {
    let mut session = session();
    let projectile = launch(&mut session);
    let birth = session.tick() - 1;
    let mut due = (0.5 + (birth as f32 * 0.015 + 0.2) / 0.015) as u64;
    while (due - birth) as f32 * 0.015 < 1.0 {
        due = (0.5 + (due as f32 * 0.015 + 0.2) / 0.015) as u64;
    }
    while session.tick() < due - 1 {
        step(&mut session, false, false);
    }
    let mut admitted = session.clone();
    step(&mut session, false, true);
    assert!(
        session
            .rigid_world()
            .unwrap()
            .projectile_body(projectile)
            .is_some()
    );
    step(&mut admitted, false, false);
    step(&mut admitted, false, true);
    assert!(
        admitted
            .rigid_world()
            .unwrap()
            .projectile_body(projectile)
            .is_none()
    );
    step(&mut session, false, false);
    assert!(
        session
            .rigid_world()
            .unwrap()
            .projectile_body(projectile)
            .is_none()
    );
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn team_cleanup_destroys_already_hidden_projectiles_without_a_second_terminal_event() {
    let mut session = session();
    let projectile = launch(&mut session);
    for _ in 0..70 {
        step(&mut session, false, false);
    }
    step(&mut session, false, true);
    assert!(
        session
            .rigid_world()
            .unwrap()
            .projectile_body(projectile)
            .is_some()
    );
    let changed = session
        .advance(Command {
            select_team: Some(PlayerTeam::Blue),
            ..Command::default()
        })
        .unwrap();
    assert!(changed.projectile_events.is_empty());
    assert!(
        session
            .rigid_world()
            .unwrap()
            .projectile_body(projectile)
            .is_none()
    );
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn authored_lower_lobby_horizontal_shot_has_an_unobstructed_rise_and_fall() {
    let mut session = session();
    session.set_position([5328.0, 3376.0, -3118.0]).unwrap();
    let command = |fire| Command {
        fire,
        pitch_degrees: 0.0,
        movement: playsrc_movement::Command {
            yaw_degrees: 180.0,
            ..Default::default()
        },
        ..Command::default()
    };
    session.advance(command(true)).unwrap();
    session.advance(command(false)).unwrap();
    let mut samples = Vec::new();
    for _ in 0..100 {
        let snapshot = session.advance(command(false)).unwrap();
        assert!(
            snapshot
                .projectile_events
                .iter()
                .filter(|event| event.kind == ProjectileEventKind::Stick)
                .count()
                <= 1,
            "duplicate adhesion transition: {:?}",
            snapshot.projectile_events
        );
        let projectile = &snapshot.projectiles[0];
        samples.push((
            snapshot.tick,
            projectile.state,
            projectile.position,
            projectile.velocity,
        ));
    }
    assert!(
        samples
            .iter()
            .any(|(_, state, _, velocity)| *state == ProjectileState::Flying && velocity[2] > 0.0)
    );
    assert!(
        samples
            .iter()
            .any(|(_, state, _, velocity)| *state == ProjectileState::Flying && velocity[2] < 0.0)
    );
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn sticky_damage_force_uses_the_authored_body_and_damage_position() {
    let mut session = session();
    step(&mut session, true, false);
    step(&mut session, false, false);
    let projectile = session.producer_snapshot().projectiles[0].identity;
    let mut world = session.rigid_world().unwrap().clone();
    let body = world.projectile_body(projectile).unwrap();
    let position = world.physics().body(body).unwrap().published().unwrap().position;
    world.set_projectile_motion(projectile, false).unwrap();
    world.set_projectile_motion(projectile, true).unwrap();
    world.set_body_velocity(body, Some([0.0; 3]), Some([0.0; 3])).unwrap();
    let mut expected = world.physics().clone();
    let force = [2000.0, 0.0, 0.0];
    let offset = [position[0], position[1] + 2.0, position[2]];
    expected.apply_force_offset(body, force, offset).unwrap();
    world.apply_projectile_force(projectile, force, offset).unwrap();
    assert_eq!(world.physics().snapshot(), expected.snapshot());
    assert_ne!(world.physics().body(body).unwrap().published().unwrap().angular_velocity, [0.0; 3]);
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn maximum_sticky_charge_fires_once_and_replays_the_owned_body_after_rollback() {
    let mut session = session();
    session.set_position([5328.0, 3376.0, -3118.0]).unwrap();
    let command = Command { fire: true, ..Command::default() };
    session.advance(command).unwrap();
    assert_eq!(session.audio_events().iter().filter(|event| event.definition == playsrc_tf2::SoundDefinition::StickyCharge && event.action == playsrc_tf2::AudioAction::Play).count(), 1);
    let begin = session.weapon_runtime(Weapon::StickybombLauncher).unwrap().charge_begin_tick.unwrap();
    let due = (begin + 1..begin + 300).find(|tick| *tick as f32 * 0.015 - begin as f32 * 0.015 >= 4.0).unwrap();
    for _ in begin + 1..due {
        let snapshot = session.advance(command).unwrap();
        assert!(snapshot.projectiles.is_empty());
        assert!(!snapshot.projectile_events.iter().any(|event| event.kind == ProjectileEventKind::Fire));
        assert_eq!(session.pipebomb_count(1), 0);
        let producer = session.producer_snapshot();
        assert_eq!(producer.pipebomb_count, 0);
        let expected = ((producer.tick as f32 * 0.015_f32 - begin as f32 * 0.015_f32).max(0.0) / 4.0).min(1.0);
        assert_eq!(producer.charge_progress.unwrap().to_bits(), expected.to_bits());
        assert_eq!(session.weapon_runtime(Weapon::StickybombLauncher).unwrap().clip, 8);
        assert!(!session.audio_events().iter().any(|event| event.definition == playsrc_tf2::SoundDefinition::StickyCharge));
    }
    let before = session.producer_snapshot();
    let physics = session.rigid_world().unwrap().physics().snapshot();
    let random = session.random_state();
    assert!(session.advance(Command { pitch_degrees: f32::NAN, ..command }).is_err());
    assert_eq!(session.producer_snapshot(), before);
    assert_eq!(session.rigid_world().unwrap().physics().snapshot(), physics);
    assert_eq!(session.random_state(), random);
    let mut replay = session.clone();
    let fired = session.advance(command).unwrap();
    assert_eq!(fired, replay.advance(command).unwrap());
    let events = fired.projectile_events.iter().filter(|event| event.kind == ProjectileEventKind::Fire).collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tick, due);
    assert!(session.rigid_world().unwrap().projectile_body(events[0].projectile).is_some());
    assert_eq!(session.pipebomb_count(1), 1);
    assert_eq!(session.weapon_runtime(Weapon::StickybombLauncher).unwrap().clip, 7);
    assert_eq!(session.weapon_runtime(Weapon::StickybombLauncher).unwrap().charge_begin_tick, None);
    assert_eq!(session.producer_snapshot().charge_progress, Some(0.0));
    assert_eq!(session.audio_events().last().map(|event| (event.definition, event.action)), Some((playsrc_tf2::SoundDefinition::StickyCharge, playsrc_tf2::AudioAction::Stop)));
    for _ in 0..16 {
        assert_eq!(session.advance(command).unwrap(), replay.advance(command).unwrap());
        assert_eq!(session.rigid_world().unwrap().physics().snapshot(), replay.rigid_world().unwrap().physics().snapshot());
    }
    session.advance(Command { select_weapon: Some(Weapon::Bottle), ..Command::default() }).unwrap();
    assert_eq!(session.producer_snapshot().pipebomb_count, 1);
    assert_eq!(session.producer_snapshot().charge_progress, None);
    session.advance(Command { select_class: Some(PlayerClass::Soldier), ..Command::default() }).unwrap();
    assert_eq!(session.producer_snapshot().pipebomb_count, 0);
    assert_eq!(session.producer_snapshot().charge_progress, None);
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn sticky_charge_feedback_starts_once_and_stops_after_launch_or_on_holster() {
    let mut session = session();
    let before = session.random_state();
    step(&mut session, true, false);
    let charge = session.audio_events().iter().filter(|event| event.definition.identity() == "Weapon_StickyBombLauncher.ChargeUp").collect::<Vec<_>>();
    assert_eq!(charge.len(), 1);
    assert_eq!(charge[0].action, playsrc_tf2::AudioAction::Play);
    assert!(session.activity_events().iter().any(|event| event.activity == playsrc_tf2::weapon::WeaponActivity::Pullback));
    assert_eq!(session.random_draws().len(), 3);
    assert_eq!(session.random_state().authority, before.authority);
    assert!(session.random_draws().iter().all(|draw| draw.context == playsrc_tf2::RandomContext::PredictedPresentation));
    for _ in 0..10 {
        step(&mut session, true, false);
        assert!(session.audio_events().is_empty());
    }
    step(&mut session, false, false);
    let sounds = session.audio_events();
    assert_eq!(sounds.len(), 2);
    assert_eq!(sounds[0].definition, playsrc_tf2::SoundDefinition::StickySingle);
    assert_eq!(sounds[1].definition.identity(), "Weapon_StickyBombLauncher.ChargeUp");
    assert_eq!(sounds[1].action, playsrc_tf2::AudioAction::Stop);
    assert!(session.random_draws().iter().all(|draw| !matches!(draw.decision,
        playsrc_tf2::RandomDecision::SoundVolume { definition: playsrc_tf2::SoundDefinition::StickyCharge, .. }
        | playsrc_tf2::RandomDecision::SoundPitch { definition: playsrc_tf2::SoundDefinition::StickyCharge, .. }
        | playsrc_tf2::RandomDecision::SoundLevel { definition: playsrc_tf2::SoundDefinition::StickyCharge, .. })));
    for _ in 0..45 { step(&mut session, false, false); }
    for _ in 0..4 {
        step(&mut session, true, false);
        if session.weapon_runtime(Weapon::StickybombLauncher).unwrap().charge_begin_tick.is_some() { break; }
    }
    assert!(session.weapon_runtime(Weapon::StickybombLauncher).unwrap().charge_begin_tick.is_some());
    session.advance(Command { select_weapon: Some(Weapon::Bottle), ..Command::default() }).unwrap();
    assert!(session.audio_events().iter().any(|event| event.definition.identity() == "Weapon_StickyBombLauncher.ChargeUp" && event.action == playsrc_tf2::AudioAction::Stop));
    assert_eq!(session.producer_snapshot().pipebomb_count, 1);
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn authored_launch_preserves_sound_spread_draw_order_and_rejects_mismatched_random_atomically() {
    use playsrc_tf2::{
        RandomContext, RandomDecision, RandomResult, SoundDefinition, SoundQueryPhase,
        StickyLaunchRandom,
    };
    let mut session = session();
    let mut predicted = playsrc_tf2::UniformRandomStream::from_state(session.random_state().predicted_presentation).unwrap();
    // Client-only charge emission, then the single-shot inspection query.
    for _ in 0..6 { predicted.random_float(0.0, 1.0); }
    let expected_sound = [predicted.random_float(0.0, 1.0), predicted.random_float(0.0, 1.0), predicted.random_float(0.0, 1.0)];
    step(&mut session, true, false);
    let producer = session.producer_snapshot();
    let random = session.random_state();
    let physics = session.rigid_world().unwrap().physics().snapshot();
    assert!(
        session
            .advance_with_external(
                Command {
                    pitch_degrees: 85.0,
                    ..Command::default()
                },
                &[],
                Some(StickyLaunchRandom {
                    right_velocity: 0.0,
                    up_velocity: 0.0,
                    angular_y: 0
                })
            )
            .is_err()
    );
    assert_eq!(session.producer_snapshot(), producer);
    assert_eq!(session.random_state(), random);
    assert_eq!(session.rigid_world().unwrap().physics().snapshot(), physics);
    step(&mut session, false, false);
    let definition = SoundDefinition::StickySingle;
    let expected = [
        RandomDecision::SoundVolume {
            definition,
            phase: SoundQueryPhase::Inspect,
        },
        RandomDecision::SoundPitch {
            definition,
            phase: SoundQueryPhase::Inspect,
        },
        RandomDecision::SoundLevel {
            definition,
            phase: SoundQueryPhase::Inspect,
        },
        RandomDecision::SoundVolume {
            definition,
            phase: SoundQueryPhase::Emit,
        },
        RandomDecision::SoundPitch {
            definition,
            phase: SoundQueryPhase::Emit,
        },
        RandomDecision::SoundLevel {
            definition,
            phase: SoundQueryPhase::Emit,
        },
        RandomDecision::StickyRightVelocity,
        RandomDecision::StickyUpVelocity,
        RandomDecision::StickyAngularY,
    ];
    let actual = session
        .random_draws()
        .iter()
        .filter(|draw| draw.context == RandomContext::Authority)
        .collect::<Vec<_>>();
    assert_eq!(
        actual.iter().map(|draw| draw.decision).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(actual[6].result, RandomResult::FloatBits(0x4045_042c));
    assert_eq!(actual[7].result, RandomResult::FloatBits(0xc10a_9c49));
    assert_eq!(actual[8].result, RandomResult::Integer(-563));
    let sounds = session.audio_events();
    assert_eq!(sounds.len(), 2);
    assert_eq!(sounds[0].definition, definition);
    assert_eq!(sounds[0].samples.volume.to_bits(), expected_sound[0].to_bits());
    assert_eq!(sounds[0].samples.pitch.to_bits(), expected_sound[1].to_bits());
    assert_eq!(sounds[0].samples.sound_level.to_bits(), expected_sound[2].to_bits());
    assert_eq!(sounds[1].definition, SoundDefinition::StickyCharge);
    assert_eq!(sounds[1].action, playsrc_tf2::AudioAction::Stop);
}

#[test]
#[ignore = "requires the configured TF2 archives and jump_beef cache object"]
fn stock_grenade_uses_its_authored_body_bounces_and_observes_the_strict_fuse_think() {
    let mut session = session();
    session.set_position([5328.0, 3376.0, -3118.0]).unwrap();
    session
        .advance(Command {
            select_weapon: Some(Weapon::GrenadeLauncher),
            ..Command::default()
        })
        .unwrap();
    let command = |fire| Command {
        fire,
        movement: playsrc_movement::Command {
            yaw_degrees: 180.0,
            ..Default::default()
        },
        ..Command::default()
    };
    for _ in 0..40 {
        session.advance(command(false)).unwrap();
    }
    let launched = session.advance(command(true)).unwrap();
    let fire = launched
        .projectile_events
        .iter()
        .find(|event| event.kind == ProjectileEventKind::Fire)
        .unwrap();
    assert_eq!(fire.projectile_kind, ProjectileKind::Grenade);
    let birth = fire.tick;
    assert_eq!(launched.projectiles[0].damage, 60.0);
    let deadline = birth as f32 * 0.015 + 2.0;
    let mut expected = (0.5 + (birth as f32 * 0.015 + 0.2) / 0.015) as u64;
    while expected as f32 * 0.015 <= deadline {
        expected = (0.5 + (expected as f32 * 0.015 + 0.2) / 0.015) as u64;
    }
    let mut impacts = 0;
    let mut exploded = false;
    for _ in 0..160 {
        let snapshot = session.advance(command(false)).unwrap();
        assert!(
            snapshot
                .projectiles
                .iter()
                .all(|projectile| projectile.state == ProjectileState::Flying)
        );
        impacts += snapshot
            .projectile_events
            .iter()
            .filter(|event| event.kind == ProjectileEventKind::Impact)
            .count();
        if let Some(explosion) = snapshot
            .projectile_events
            .iter()
            .find(|event| event.kind == ProjectileEventKind::Explode)
        {
            assert_eq!(explosion.tick, expected);
            assert!(snapshot.projectiles.is_empty());
            let requests = session.radius_damage_requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(
                (
                    requests[0].base_damage,
                    requests[0].radius,
                    requests[0].self_radius
                ),
                (60.0, 146.0, 146.0)
            );
            exploded = true;
            break;
        }
    }
    assert!(impacts > 0);
    assert!(exploded);
}
