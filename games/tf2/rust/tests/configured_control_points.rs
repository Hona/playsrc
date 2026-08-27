use std::{fs, path::PathBuf};

use playsrc_tf2::{PlayerTeam, control_point};
use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires exact configured build 24245096 attack/defend BSPs"]
fn attack_defend_authored_stages_and_spawn_sets() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let value = &config[config.find("\"tf2Dir\"").unwrap() + "\"tf2Dir\"".len()..];
    let value = value[value.find(':').unwrap() + 1..].trim_start();
    let tf2 = PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);
    for (name, length, hash, count) in [
        ("cp_dustbowl", 21_945_050, "f2595d3f6af19f21d7beaeed7ecb7a130246a5b934641a44f0f68d54edfc421e", 6),
        ("cp_gorge", 50_238_340, "94db834e88f98048326513133a8c98178866cef2f72d6406515ed1af0a4a5f46", 2),
    ] {
        let bytes = fs::read(tf2.join(format!("maps/{name}.bsp"))).unwrap();
        assert_eq!(bytes.len(), length);
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), hash);
        let bsp = playsrc_bsp::parse(&bytes, playsrc_bsp::Profile::Source2013V20, Default::default()).unwrap();
        let graph = playsrc_entity::parse(bsp.lump(0).unwrap().bytes(&bsp), Default::default()).unwrap();
        let mut world = control_point::World::from_graph(&graph).unwrap().unwrap();
        assert_eq!(world.points().len(), count);
        assert!(world.points().iter().all(|p| p.owner == PlayerTeam::Red));
        let mut random = playsrc_tf2::UniformRandomStream::from_seed(42).unwrap();
        let facts = control_point::Facts { points_may_be_captured: true, round_running: true, ..Default::default() };
        let stages = if count == 6 { 3 } else { 1 };
        for stage in 0..stages {
            let mut events = Vec::new();
            world.select_round(None, &mut random, &mut events);
            if stages == 3 {
                assert_eq!(world.current_round(), Some(stage));
                assert_eq!(world.rounds()[stage].name, format!("round_{}", stage + 1));
                assert_eq!(world.snapshot(vec![]).points.iter().filter(|p| p.visible).map(|p| p.index).collect::<Vec<_>>(), [stage * 2, stage * 2 + 1]);
            }
            for team in [PlayerTeam::Red, PlayerTeam::Blue] {
                assert!(world.spawns().iter().any(|s| s.team == team && !s.disabled), "{name} stage {stage} {team:?}");
            }
            assert!(world.team_may_capture(PlayerTeam::Blue, stage * 2, false));
            assert!(!world.team_may_capture(PlayerTeam::Blue, stage * 2 + 1, false));
            for point in stage * 2..stage * 2 + 2 {
                let identity = world.points()[point].identity;
                world.apply_input(identity, b"SetOwner", &playsrc_entity::Variant::Integer(3), stage as f32, facts, &mut events);
            }
            let full_reset = stage == stages - 1;
            assert!(events.iter().any(|e| matches!(e, control_point::Event::RoundWon { team: PlayerTeam::Blue, full_reset: reset, switch_teams, .. } if *reset == full_reset && *switch_teams == full_reset)));
            world.end_round(&mut events);
            world.round_spawn(stage as f32 + 1.0, full_reset, &mut events);
            if !full_reset { assert!(world.points()[..(stage + 1) * 2].iter().all(|p| p.owner == PlayerTeam::Blue)); }
        }
        assert!(world.points().iter().all(|p| p.owner == PlayerTeam::Red));
        let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else { panic!("missing models"); };
        let bounds = models.iter().enumerate().map(|(model, value)| playsrc_entity::ModelBounds {
            model,
            mins: [value.mins.x.value(), value.mins.y.value(), value.mins.z.value()],
            maxs: [value.maxs.x.value(), value.maxs.y.value(), value.maxs.z.value()],
        }).collect();
        playsrc_tf2::MapRuntime::compile(&graph, 0.015, 42, bounds).unwrap();
    }
}

#[test]
#[ignore = "requires exact configured build 24245096 cp_badlands BSP"]
fn badlands_authored_capture_chain_and_master() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let value = &config[config.find("\"tf2Dir\"").unwrap() + "\"tf2Dir\"".len()..];
    let value = value[value.find(':').unwrap() + 1..].trim_start();
    let tf2 = PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);
    let bytes = fs::read(tf2.join("maps/cp_badlands.bsp")).unwrap();
    assert_eq!(bytes.len(), 25_981_141);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), "872f6e77abda907d095000009cfbe8c50d62d15e304c80c7dc86a6591ebc08e3");
    let bsp = playsrc_bsp::parse(&bytes, playsrc_bsp::Profile::Source2013V20, playsrc_bsp::Limits::default()).unwrap();
    let graph = playsrc_entity::parse(bsp.lump(0).unwrap().bytes(&bsp), playsrc_entity::Limits::default()).unwrap();
    let mut world = control_point::World::from_graph(&graph).unwrap().unwrap();
    assert_eq!(world.points().iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["cap_blue_1", "cap_blue_2", "cap_center", "cap_red_2", "cap_red_1"]);
    assert_eq!(world.points().iter().map(|p| p.owner).collect::<Vec<_>>(), [PlayerTeam::Blue, PlayerTeam::Blue, PlayerTeam::Unassigned, PlayerTeam::Red, PlayerTeam::Red]);
    for team in [PlayerTeam::Red, PlayerTeam::Blue] {
        let targets: Vec<_> = world.points().iter().filter(|p| p.owner != team && world.team_may_capture(team, p.index, false)).map(|p| p.index).collect();
        assert_eq!(targets, [2]);
    }
    let mut areas: Vec<_> = world.areas().iter().collect();
    areas.sort_by_key(|a| a.point);
    assert_eq!(areas.iter().map(|a| a.total_time(PlayerTeam::Red, world.configuration())).collect::<Vec<_>>(), [4.0,16.0,20.0,16.0,4.0]);
    assert_eq!(areas[1].teams[2].spawn_adjust, -4);
    assert_eq!(areas[3].teams[3].spawn_adjust, -4);
    assert_eq!(world.master().base_points[2], Some(4));
    assert_eq!(world.master().base_points[3], Some(0));
    let red_spawns = |world: &control_point::World| world.spawns().iter().filter(|spawn| spawn.team == PlayerTeam::Red && !spawn.disabled).map(|spawn| spawn.point).collect::<Vec<_>>();
    assert_eq!(red_spawns(&world), vec![Some(4); 16]);
    let facts = control_point::Facts { points_may_be_captured: true, round_running: true, ..control_point::Facts::default() };
    let middle = world.points()[2].identity;
    let second = world.points()[1].identity;
    world.apply_input(middle, b"SetOwner", &playsrc_entity::Variant::Integer(2), 1.0, facts, &mut Vec::new());
    assert_eq!(red_spawns(&world), vec![Some(2); 12]);
    world.apply_input(second, b"SetOwner", &playsrc_entity::Variant::Integer(2), 2.0, facts, &mut Vec::new());
    assert_eq!(red_spawns(&world), vec![Some(1); 12]);
    world.apply_input(second, b"SetOwner", &playsrc_entity::Variant::Integer(3), 3.0, facts, &mut Vec::new());
    assert_eq!(red_spawns(&world), vec![Some(2); 12]);
    world.apply_input(middle, b"SetOwner", &playsrc_entity::Variant::Integer(3), 4.0, facts, &mut Vec::new());
    assert_eq!(red_spawns(&world), vec![Some(4); 16]);
    let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else { panic!("missing models"); };
    let bounds = models.iter().enumerate().map(|(model, value)| playsrc_entity::ModelBounds {
        model,
        mins: [value.mins.x.value(), value.mins.y.value(), value.mins.z.value()],
        maxs: [value.maxs.x.value(), value.maxs.y.value(), value.maxs.z.value()],
    }).collect();
    playsrc_tf2::MapRuntime::compile(&graph, 0.015, 0x872f6e77, bounds).unwrap();
}
