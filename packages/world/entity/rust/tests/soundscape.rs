use playsrc_entity::soundscape::*;
use playsrc_entity::{EntityWorld, EntityWorldConfig, Limits, parse};

fn world(script: &[u8]) -> (EntityWorld, Systems) {
    let graph = parse(script, Limits::default()).unwrap();
    let (world, _) = EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            external_classes: bindings(),
            ..EntityWorldConfig::default()
        },
    )
    .unwrap();
    let systems = Systems::from_world(&world, |name| match name {
        b"outside" => Some(0),
        b"inside" => Some(1),
        _ => None,
    });
    (world, systems)
}
fn clear(_: Position, _: Position) -> Trace {
    Trace {
        fraction: 1.0,
        start_solid: false,
    }
}

#[test]
fn nearest_visible_strict_radius_ties_and_no_candidate_retains_prior_audio() {
    let (world, mut systems) = world(br#"
        {"classname" "env_soundscape" "targetname" "a" "origin" "-10 0 0" "radius" "20" "soundscape" "outside"}
        {"classname" "env_soundscape" "targetname" "b" "origin" "10 0 0" "radius" "20" "soundscape" "inside"}
    "#);
    let mut player = Player::default();
    let mut plays = vec![];
    assert_eq!(
        systems.update(&world, &mut player, [0.0; 3], &[0, 1], clear, &mut plays),
        1
    );
    assert_eq!(player.selection.soundscape, 0);
    assert_eq!(plays.len(), 1);
    systems.update(&world, &mut player, [0.0; 3], &[1, 0], clear, &mut plays);
    assert_eq!(plays.len(), 1); // equal-distance contender does not replace current
    systems.update(
        &world,
        &mut player,
        [8.0, 0.0, 0.0],
        &[0, 1],
        clear,
        &mut plays,
    );
    assert_eq!(player.selection.soundscape, 1);
    systems.update(
        &world,
        &mut player,
        [30.0, 0.0, 0.0],
        &[0, 1],
        |_, _| panic!("strict radius boundary must not trace"),
        &mut plays,
    );
    assert_eq!(player.selection.soundscape, 1);
    systems.input(systems.zones()[1].entity, b"Disable");
    systems.update(
        &world,
        &mut player,
        [8.0, 0.0, 0.0],
        &[0, 1],
        clear,
        &mut plays,
    );
    assert_eq!(player.selection.soundscape, 0);
}

#[test]
fn startsolid_and_partial_visibility_reject_and_negative_one_is_unbounded() {
    let (world, systems) =
        world(br#"{"classname" "env_soundscape" "radius" "-1" "soundscape" "outside"}"#);
    let mut player = Player::default();
    let mut plays = vec![];
    for hit in [
        Trace {
            fraction: 1.0,
            start_solid: true,
        },
        Trace {
            fraction: 0.99,
            start_solid: false,
        },
    ] {
        systems.update(
            &world,
            &mut player,
            [10000.0; 3],
            &[0],
            |_, _| hit,
            &mut plays,
        );
        assert_eq!(player.selection.entity, 0);
    }
    systems.update(&world, &mut player, [10000.0; 3], &[0], clear, &mut plays);
    assert_eq!(player.selection.soundscape, 0);
}

#[test]
fn proxy_copies_bindings_but_not_radius_or_enabled_and_resolves_positions_at_play() {
    let (world, systems) = world(br#"
        {"classname" "env_soundscape" "targetname" "main" "soundscape" "inside" "StartDisabled" "1" "position0" "point" "position1" "missing"}
        {"classname" "env_soundscape_proxy" "MainSoundscapeName" "main" "radius" "-1"}
        {"classname" "info_target" "targetname" "point" "origin" "12 30 50"}
    "#);
    let mut player = Player::default();
    let mut plays = vec![];
    systems.update(&world, &mut player, [0.0; 3], &[0, 1], clear, &mut plays);
    assert_eq!(player.selection.entity, 2);
    assert_eq!(player.selection.soundscape, 1);
    assert_eq!(player.selection.position_bits, 1);
    assert_eq!(player.selection.positions[0], [12.0, 30.0, 50.0]);
    assert_eq!(plays, [systems.zones()[1].entity]);
}

#[test]
fn trigger_stack_latest_enter_deduplicates_and_exit_restores_even_disabled_zone() {
    let (world, systems) = world(br#"
        {"classname" "env_soundscape_triggerable" "targetname" "a" "soundscape" "outside" "StartDisabled" "1"}
        {"classname" "env_soundscape_triggerable" "targetname" "b" "soundscape" "inside"}
        {"classname" "trigger_soundscape" "targetname" "ta" "soundscape" "a"}
        {"classname" "trigger_soundscape" "targetname" "tb" "soundscape" "b"}
    "#);
    let a = world.resolve(b"ta", None, None, None)[0];
    let b = world.resolve(b"tb", None, None, None)[0];
    let mut player = Player::default();
    let mut plays = vec![];
    systems.touch(&world, a, true, &mut player, &mut plays);
    systems.touch(&world, a, true, &mut player, &mut plays);
    systems.touch(&world, b, true, &mut player, &mut plays);
    assert_eq!(player.selection.soundscape, 1);
    systems.touch(&world, b, false, &mut player, &mut plays);
    assert_eq!(player.selection.soundscape, 0);
    systems.touch(&world, a, false, &mut player, &mut plays);
    assert_eq!(player.selection.entity, 0);
    assert_eq!(player.selection.soundscape, 0);
    assert_eq!(plays.len(), 4);
}
