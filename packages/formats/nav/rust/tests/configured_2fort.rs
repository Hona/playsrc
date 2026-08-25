use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires playsrc.local.json and the exact configured Source-generated ctf_2fort NAV"]
fn configured_2fort_nav_retains_source_generation_and_both_intelligence_routes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let marker = "\"tf2Dir\"";
    let tail = &config[config.find(marker).unwrap() + marker.len()..];
    let tail = tail[tail.find(':').unwrap() + 1..].trim_start();
    let content = PathBuf::from(&tail[1..tail[1..].find('"').unwrap() + 1]);
    assert!(content.is_absolute());

    let bytes = fs::read(content.join("maps/ctf_2fort.nav")).unwrap();
    assert_eq!(bytes.len(), 307_701);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "6c1e5b37b3cffb9ad97c554aa9e104119a5c5fb38bd6c9d2903a4d405f609017"
    );
    let map_size = fs::metadata(content.join("maps/ctf_2fort.bsp"))
        .unwrap()
        .len();
    assert_eq!(map_size, 22_751_863);

    let mesh = playsrc_nav::parse(
        &bytes,
        playsrc_nav::Profile::TeamFortress2,
        Some(map_size as u32),
        playsrc_nav::Limits::default(),
    )
    .unwrap();
    assert_eq!((mesh.version, mesh.subversion), (16, 2));
    assert!(mesh.analyzed);
    assert_eq!(mesh.areas.len(), 1_128);
    assert!(mesh.ladders.is_empty());

    let connections = mesh
        .areas
        .iter()
        .flat_map(|area| area.connections.iter())
        .map(Vec::len)
        .sum::<usize>();
    let hiding_spots = mesh
        .areas
        .iter()
        .map(|area| area.hiding_spots.len())
        .sum::<usize>();
    let visible_areas = mesh
        .areas
        .iter()
        .map(|area| area.visible_areas.len())
        .sum::<usize>();
    assert_eq!(connections, 4_233);
    assert_eq!(hiding_spots, 385);
    assert_eq!(visible_areas, 32_700);

    for (spawn, enemy_flag, capture) in [
        (
            [1888.0, 1368.0, 276.0],
            [489.005, -3348.51, -131.106],
            [-500.0, 3366.0, -98.0],
        ),
        (
            [-1888.0, -1280.0, 276.0],
            [-488.66, 3348.51, -131.026],
            [500.3447, -3366.0, -98.0],
        ),
    ] {
        let spawn = mesh.nearest_area(spawn).unwrap().identity;
        let flag = mesh.nearest_area(enemy_flag).unwrap().identity;
        let capture = mesh.nearest_area(capture).unwrap().identity;
        let fetch = mesh
            .build_path(spawn, flag, |_, _, _, length| Some(length))
            .unwrap();
        let deliver = mesh
            .build_path(flag, capture, |_, _, _, length| Some(length))
            .unwrap();
        assert!(fetch.len() > 4);
        assert!(deliver.len() > 4);
        assert_eq!(fetch.first().copied(), Some(spawn));
        assert_eq!(fetch.last().copied(), Some(flag));
        assert_eq!(deliver.first().copied(), Some(flag));
        assert_eq!(deliver.last().copied(), Some(capture));
    }
}
