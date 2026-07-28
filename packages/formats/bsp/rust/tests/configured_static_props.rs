use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalConfig {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[test]
#[ignore = "requires playsrc.local.json and configured TF2 build 24245096"]
fn configured_pl_upward_static_props_are_complete_and_deterministic() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .to_path_buf();
    let config: LocalConfig = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).expect("configured local paths"),
    )
    .expect("valid local configuration");
    assert!(!config.source_cache_dir.is_empty() && !config.asset_dir.is_empty());
    let bytes = fs::read(PathBuf::from(config.tf2_dir).join("maps/pl_upward.bsp"))
        .expect("exact configured maps/pl_upward.bsp");
    assert_eq!(bytes.len(), 25_446_018);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709"
    );
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .expect("configured BSP");
    let first = playsrc_bsp::parse_static_props(&bsp, playsrc_bsp::Limits::default())
        .expect("configured static props")
        .expect("sprp");
    let second = playsrc_bsp::parse_static_props(&bsp, playsrc_bsp::Limits::default())
        .expect("repeated configured static props")
        .expect("sprp");
    assert_eq!(first, second);
    assert_eq!(first.version, 10);
    assert_eq!(first.decoded_length, 125_044);
    assert_eq!(first.dictionary.len(), 234);
    assert_eq!(first.leaf_references.len(), 2_756);
    assert_eq!(first.occurrences.len(), 1_244);
    assert_eq!(
        first
            .occurrences
            .iter()
            .filter(|prop| prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING != 0)
            .count(),
        4
    );
    assert_eq!(
        first
            .occurrences
            .iter()
            .filter(|prop| prop.lighting_origin.is_some())
            .count(),
        5
    );
    assert_eq!(
        first
            .occurrences
            .iter()
            .filter(|prop| prop.solidity == 6)
            .count(),
        554
    );
    assert!(first.occurrences.iter().all(|prop| {
        usize::from(prop.model) < first.dictionary.len()
            && !prop.leaves.is_empty()
            && prop.lightmap_resolution == [0, 0]
    }));
}
