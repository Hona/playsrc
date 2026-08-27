use std::{fs, path::PathBuf};

use playsrc_tf2::{PlayerTeam, control_point};
use sha2::{Digest, Sha256};

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
    let world = control_point::World::from_graph(&graph).unwrap().unwrap();
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
    let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else { panic!("missing models"); };
    let bounds = models.iter().enumerate().map(|(model, value)| playsrc_entity::ModelBounds {
        model,
        mins: [value.mins.x.value(), value.mins.y.value(), value.mins.z.value()],
        maxs: [value.maxs.x.value(), value.maxs.y.value(), value.maxs.z.value()],
    }).collect();
    playsrc_tf2::MapRuntime::compile(&graph, 0.015, 0x872f6e77, bounds).unwrap();
}
