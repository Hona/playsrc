use super::{RigidModel, RigidProjectileModels};
use playsrc_content::{Content, ProviderSpec, Resolution};
use playsrc_material::{SurfacePropertyFile, SurfacePropertyRegistry};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, sync::Arc};

fn checked(bytes: Vec<u8>, hash: &str) -> Vec<u8> {
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        hash,
        "configured resource identity changed"
    );
    bytes
}
pub fn load() -> (
    playsrc_collision::World,
    playsrc_collision::PhysicalModelInventory,
    Arc<SurfacePropertyRegistry>,
    RigidProjectileModels,
) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("playsrc.local.json")).unwrap()).unwrap();
    assert_eq!(
        config
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["assetDir", "sourceCacheDir", "tf2Dir"]
    );
    for value in config.as_object().unwrap().values() {
        assert!(PathBuf::from(value.as_str().unwrap()).is_absolute());
    }
    let tf = PathBuf::from(config["tf2Dir"].as_str().unwrap());
    let providers = [
        (
            tf.join("tf2_misc_dir.vpk"),
            "63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9",
        ),
        (
            tf.parent().unwrap().join("hl2/hl2_misc_dir.vpk"),
            "cde295f333208563f3cc02566814133712d2590c1dc62362503a62ea93752fe3",
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (file, hash))| {
        checked(fs::read(&file).unwrap(), hash);
        ProviderSpec::Vpk {
            id: format!("configured-{index}"),
            revision: hash.into(),
            directory_file: file,
            layout: playsrc_vpk::Layout::Split,
        }
    })
    .collect();
    let content = Content::open(
        "tf2",
        "24245096",
        providers,
        playsrc_content::Limits::default(),
    )
    .unwrap();
    let resolve = |path: &str, hash: &str| {
        let Resolution::Found(resource) = content.resolve_resource(path).unwrap() else {
            panic!("configured resource missing: {path}");
        };
        checked(resource.bytes, hash)
    };
    let files = [
        (
            "scripts/surfaceproperties.txt",
            "2aec45713c84d9dd8dd7fb40db3f1c63986778857c3700cc9c4885f2edbca7d4",
        ),
        (
            "scripts/surfaceproperties_hl2.txt",
            "6eb6c622f9d566515d0909d610c6f7213d327ed69545807696d74162ee3c0280",
        ),
        (
            "scripts/surfaceproperties_tf.txt",
            "c50e75a206a26d447eaa73e0d8bbc540c350f0c18570d3f76f4a80234587c7e0",
        ),
    ]
    .map(|(path, hash)| (path, resolve(path, hash)));
    let surfaces = SurfacePropertyRegistry::compile(
        &files
            .iter()
            .map(|(path, bytes)| SurfacePropertyFile {
                logical_path: path,
                bytes,
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let compile = |identity, path, hash| {
        let bytes = resolve(path, hash);
        let asset = playsrc_phy::parse_standalone(
            &bytes,
            playsrc_phy::Profile::SourcePcPolygon,
            playsrc_phy::Limits::default(),
        )
        .unwrap();
        RigidModel::compile(
            identity,
            &asset,
            None,
            &surfaces,
            playsrc_collision::SnapshotLimits::default(),
        )
        .unwrap()
    };
    let projectiles = RigidProjectileModels {
        sticky: compile(
            700,
            "models/weapons/w_models/w_stickybomb.phy",
            "7e70a4a90eca8bb74aafddf01bdbd755532aa1229397f39f9121be792143ce57",
        ),
        grenade: compile(
            701,
            "models/weapons/w_models/w_grenade_grenadelauncher.phy",
            "2e7c3a347f7042b257a57e20a1bc3c627116fda7e49e8f93521ca9883966bc19",
        ),
    };
    let hash = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
    let bytes = checked(
        fs::read(
            PathBuf::from(config["sourceCacheDir"].as_str().unwrap())
                .join("objects/sha256")
                .join(&hash[..2])
                .join(hash),
        )
        .unwrap(),
        hash,
    );
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    let world = playsrc_collision::compile(&bsp).unwrap();
    let models = playsrc_collision::PhysicalModelInventory::compile(
        &world,
        &bsp,
        playsrc_collision::SnapshotLimits::default(),
    )
    .unwrap();
    (world, models, Arc::new(surfaces), projectiles)
}
