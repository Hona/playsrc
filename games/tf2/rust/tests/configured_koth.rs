use playsrc_tf2::{PlayerTeam, control_point, koth, round};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[test]
#[ignore = "requires exact configured build 24245096 koth_viaduct BSP"]
fn viaduct_authored_logic_capture_io_and_generated_clocks() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let value = &config[config.find("\"tf2Dir\"").unwrap() + "\"tf2Dir\"".len()..];
    let value = value[value.find(':').unwrap() + 1..].trim_start();
    let tf2 = PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);
    let bytes = fs::read(tf2.join("maps/koth_viaduct.bsp")).unwrap();
    assert_eq!(bytes.len(), 41_690_668);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "b3574e496550311f5036997ed7bf3d1007be7fe28236f8f33a2352fe0518729c"
    );
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        Default::default(),
    )
    .unwrap();
    let graph =
        playsrc_entity::parse(bsp.lump(0).unwrap().bytes(&bsp), Default::default()).unwrap();
    let logic = koth::Configuration::from_graph(&graph).unwrap().unwrap();
    assert_eq!(logic.timer_length, 180);
    assert_eq!(logic.unlock_point, 30);
    let world = control_point::World::from_graph(&graph).unwrap().unwrap();
    assert_eq!(world.points().len(), 1);
    assert_eq!(world.points()[0].position, [-1536.0, 0.0, 224.0]);
    assert_eq!(world.points()[0].owner, PlayerTeam::Unassigned);
    assert_eq!(
        world.areas()[0].total_time(PlayerTeam::Red, world.configuration()),
        12.0
    );
    let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else {
        panic!("missing brush models");
    };
    let bounds = models
        .iter()
        .enumerate()
        .map(|(model, value)| playsrc_entity::ModelBounds {
            model,
            mins: [
                value.mins.x.value(),
                value.mins.y.value(),
                value.mins.z.value(),
            ],
            maxs: [
                value.maxs.x.value(),
                value.maxs.y.value(),
                value.maxs.z.value(),
            ],
        })
        .collect();
    let map = playsrc_tf2::MapRuntime::compile(&graph, 0.015, 0xb3574e49, bounds).unwrap();
    assert!(map.source_handle(logic.red_timer).is_some());
    assert!(map.source_handle(logic.blue_timer).is_some());
    let rules = round::Rules::active(map.round_configuration()).unwrap();
    assert_eq!(
        rules
            .koth_timers()
            .unwrap()
            .map(|timer| (timer.remaining, timer.paused)),
        [(180.0, true); 2]
    );
}
