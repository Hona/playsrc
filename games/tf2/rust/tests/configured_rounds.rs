use std::{fs, path::PathBuf};

use playsrc_tf2::{PlayerTeam, round};

fn configured_graph(map: &str) -> playsrc_entity::Graph {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let marker = "\"tf2Dir\"";
    let value = &config[config.find(marker).unwrap() + marker.len()..];
    let value = value[value.find(':').unwrap() + 1..].trim_start();
    let tf2 = PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);
    let bytes = fs::read(tf2.join("maps").join(format!("{map}.bsp"))).unwrap();
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    playsrc_entity::parse(
        bsp.lump(0).unwrap().bytes(&bsp),
        playsrc_entity::Limits::default(),
    )
    .unwrap()
}

#[test]
#[ignore = "requires playsrc.local.json and the exact configured ctf_2fort and pl_upward BSPs"]
fn configured_maps_select_only_their_authored_round_rules() {
    let two_fort = round::Configuration::from_graph(&configured_graph("ctf_2fort")).unwrap();
    assert!(two_fort.timers.is_empty());
    assert_eq!(two_fort.defending_team, None);
    assert_eq!(two_fort.waiting_seconds, 30.0);
    assert_eq!(two_fort.preround_seconds, 5.0);
    assert_eq!(two_fort.bonus_seconds, 15.0);
    assert!(!two_fort.stalemate_enabled);

    let upward = round::Configuration::from_graph(&configured_graph("pl_upward")).unwrap();
    assert_eq!(upward.timers.len(), 1);
    let timer = upward.timers[0];
    assert_eq!(timer.initial_seconds, 330);
    assert_eq!(timer.setup_seconds, 70);
    assert_eq!(timer.maximum_seconds, 600);
    assert!(timer.show_in_hud && timer.auto_countdown && !timer.start_paused);
    assert_eq!(upward.defending_team, Some(PlayerTeam::Red));
}
