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
fn configured_pl_upward_displacements_are_complete_and_deterministic() {
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
    let first =
        playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Hdr).expect("configured HDR map");
    let second = playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Hdr)
        .expect("repeated configured HDR map");
    assert_eq!(first, second);
    assert_eq!(
        first
            .surfaces
            .iter()
            .filter(|surface| surface.displacement.is_some())
            .count(),
        558
    );
    assert_eq!(
        first
            .surfaces
            .iter()
            .filter(|surface| surface.draw && surface.model == 0)
            .count(),
        15_072
    );
    assert_eq!(first.vertex_count, 96_880);
    let collision = playsrc_collision::compile(&bsp).expect("configured collision producer");
    assert_eq!(collision.displacements.len(), 558);
    assert_eq!(
        collision
            .displacements
            .iter()
            .map(|patch| patch.vertices.len())
            .sum::<usize>(),
        14_174
    );
    assert_eq!(
        collision
            .displacements
            .iter()
            .map(|patch| patch.triangle_tags.len())
            .sum::<usize>(),
        18_240
    );
}
