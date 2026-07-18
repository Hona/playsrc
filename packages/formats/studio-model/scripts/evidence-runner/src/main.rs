use playsrc_studio_model as studio;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const BUILD: &str = "24207079";
const BUNDLE_SHA256: &str = "896132d9b618d0ae521092c1e33d91d3cc05f1692ac434603a31994b8dd51741";
const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const OCCURRENCE_TRANSFORM_SHA256: &str =
    "7a4eff4a2d9ca0892b6f576d21df4d44d03e03f957499c20245740b21b4edee6";
const MODEL_MATERIAL_COUNT: usize = 55;
const MODEL_TEXTURE_COUNT: usize = 71;
const MODEL_MIP_SHA256: &str = "05c7869e3f78b03b2c9f05ebdb9a8ec8f9895a8a8d3ff37530f3e0d26f617033";
const MODEL_DRAW_SHA256: &str = "12770b45364a035c81a0fd96fdd84dd762d97540a640bbfbb44db290d7b2014d";
const EYE_STATE_SHA256: &str = "cd6606f8d35ed20c87ffc33b40190586be2dc94f48eafadb3b7038f99d9d103a";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

struct Target {
    path: &'static str,
    mdl_sha256: &'static str,
    bones: usize,
    animations: usize,
    sequences: usize,
    materials: usize,
    skins: usize,
    body_parts: usize,
    attachments: usize,
    primitives: usize,
    lods: &'static [usize],
    physics: studio::PhysicsStatus,
    first_material: &'static str,
    artifact_bytes: usize,
    artifact_sha256: &'static str,
    timeline_sha256: &'static str,
    activities: &'static [&'static str],
}

trait ExactFiles {
    fn exact(&self, logical_path: &str) -> Result<Option<Vec<u8>>, String>;
}

impl ExactFiles for BTreeMap<String, Vec<u8>> {
    fn exact(&self, logical_path: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.get(logical_path).cloned())
    }
}

struct VpkFiles {
    archives: Vec<ConfiguredArchive>,
}

struct ConfiguredArchive {
    archive: playsrc_vpk::Archive,
    segments: FileSegments,
}

struct FileSegments {
    directory: PathBuf,
    prefix: String,
}

impl playsrc_vpk::SegmentReader for FileSegments {
    fn len(&self, archive_index: u32) -> Result<u64, playsrc_vpk::SourceError> {
        let path = self
            .directory
            .join(format!("{}_{archive_index:03}.vpk", self.prefix));
        fs::metadata(path)
            .map(|metadata| metadata.len())
            .map_err(|error| source_error(error, 0..0))
    }

    fn read(
        &self,
        archive_index: u32,
        range: std::ops::Range<u64>,
    ) -> Result<Vec<u8>, playsrc_vpk::SourceError> {
        let path = self
            .directory
            .join(format!("{}_{archive_index:03}.vpk", self.prefix));
        let mut file = fs::File::open(path).map_err(|error| source_error(error, range.clone()))?;
        file.seek(SeekFrom::Start(range.start))
            .map_err(|error| source_error(error, range.clone()))?;
        let length = usize::try_from(range.end.saturating_sub(range.start)).map_err(|_| {
            playsrc_vpk::SourceError {
                code: playsrc_vpk::SourceErrorCode::Io,
                range: range.clone(),
            }
        })?;
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes)
            .map_err(|error| source_error(error, range))?;
        Ok(bytes)
    }
}

impl VpkFiles {
    fn new(tf2_dir: &Path) -> Result<Self, String> {
        let mut archives = Vec::new();
        for prefix in ["tf2_misc", "tf2_textures"] {
            let identity = format!("{prefix}_dir.vpk");
            let bytes = fs::read(tf2_dir.join(&identity)).map_err(|error| error.to_string())?;
            let archive = playsrc_vpk::parse(
                &bytes,
                identity,
                playsrc_vpk::Layout::Split,
                playsrc_vpk::Limits::default(),
            )
            .map_err(|error| error.to_string())?;
            archives.push(ConfiguredArchive {
                archive,
                segments: FileSegments {
                    directory: tf2_dir.to_owned(),
                    prefix: prefix.to_owned(),
                },
            });
        }
        Ok(Self { archives })
    }
}

impl ExactFiles for VpkFiles {
    fn exact(&self, logical_path: &str) -> Result<Option<Vec<u8>>, String> {
        for configured in &self.archives {
            match configured.archive.entry(logical_path) {
                Ok(_) => {
                    return configured
                        .archive
                        .read_entry(logical_path, &configured.segments)
                        .map(|result| Some(result.bytes))
                        .map_err(|error| error.to_string());
                }
                Err(error) if error.code == playsrc_vpk::ErrorCode::MissingEntry => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(None)
    }
}

fn source_error(error: std::io::Error, range: std::ops::Range<u64>) -> playsrc_vpk::SourceError {
    playsrc_vpk::SourceError {
        code: if error.kind() == std::io::ErrorKind::NotFound {
            playsrc_vpk::SourceErrorCode::Missing
        } else if error.kind() == std::io::ErrorKind::UnexpectedEof {
            playsrc_vpk::SourceErrorCode::ShortRead
        } else {
            playsrc_vpk::SourceErrorCode::Io
        },
        range,
    }
}

const WORLD_ACTIVITIES: &[&str] = &[
    "ACT_MP_STAND_PRIMARY",
    "ACT_MP_RUN_PRIMARY",
    "ACT_MP_CROUCH_PRIMARY",
    "ACT_MP_CROUCHWALK_PRIMARY",
    "ACT_MP_JUMP_START_PRIMARY",
    "ACT_MP_JUMP_FLOAT_PRIMARY",
    "ACT_MP_JUMP_LAND_PRIMARY",
    "ACT_MP_ATTACK_STAND_PRIMARY",
];
struct StockViewModelTarget {
    hand: &'static str,
    hand_sha256: &'static str,
    item: &'static str,
    item_sha256: &'static str,
    activities: &'static [&'static str],
    merged_bones: usize,
    composition_sha256: &'static str,
    producer_sha256: &'static str,
}

const SOLDIER_STOCK_ACTIVITIES: &[&str] = &[
    "ACT_PRIMARY_VM_DRAW",
    "ACT_PRIMARY_VM_IDLE",
    "ACT_PRIMARY_VM_PRIMARYATTACK",
    "ACT_PRIMARY_RELOAD_START",
    "ACT_PRIMARY_VM_RELOAD",
    "ACT_PRIMARY_RELOAD_FINISH",
];

const DEMOMAN_STOCK_ACTIVITIES: &[&str] = &[
    "ACT_SECONDARY_VM_DRAW",
    "ACT_SECONDARY_VM_IDLE",
    "ACT_SECONDARY_VM_PRIMARYATTACK",
    "ACT_SECONDARY_RELOAD_START",
    "ACT_SECONDARY_VM_RELOAD",
    "ACT_SECONDARY_RELOAD_FINISH",
];

const STOCK_VIEWMODELS: &[StockViewModelTarget] = &[
    StockViewModelTarget {
        hand: "models/weapons/c_models/c_soldier_arms.mdl",
        hand_sha256: "4aeba0ceccb87f045349e4604204308c7bf91507defef7ee9301cbbfe1678fd5",
        item: "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
        item_sha256: "e962a3ab43ad731c6b65780c760c61a7d06676f5ca05fd112bcfc74944a605e0",
        activities: SOLDIER_STOCK_ACTIVITIES,
        merged_bones: 3,
        composition_sha256: "160db54bf1706daed8d788ad692ef649ecfe49853cf44c3551332c497dbfcea2",
        producer_sha256: "154c33219634e3f5a8289953fea5ff9224a4913022c1b3ec5b843b4b380ddd4a",
    },
    StockViewModelTarget {
        hand: "models/weapons/c_models/c_demo_arms.mdl",
        hand_sha256: "a49561921958a0d47f34be7b61705973f27813880759783cf573c5b62c4ae073",
        item: "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
        item_sha256: "bbdb99e9a836603b795c8851a16838aab37a5bcf178d2dd4a25fbc9c0fa72108",
        activities: DEMOMAN_STOCK_ACTIVITIES,
        merged_bones: 4,
        composition_sha256: "8faeff88db0154818b5d0882a7c0fc07b37d2ed6381b37e3c7fd518928e980ef",
        producer_sha256: "a61764813635c46a18ccb1326b89974f37128f91a7cce870ee0cb4823099976c",
    },
];

const TARGETS: &[Target] = &[
    Target {
        path: "models/props_2fort/cow001_reference.mdl",
        mdl_sha256: "33b6fdd5a60a146f865157d488baf0d7b945c956e7cca7bfb13cf80e9d5e726e",
        bones: 1,
        animations: 1,
        sequences: 1,
        materials: 1,
        skins: 1,
        body_parts: 1,
        attachments: 0,
        primitives: 1,
        lods: &[0],
        physics: studio::PhysicsStatus::Missing,
        first_material: "materials/models/props_2fort/cow001.vmt",
        artifact_bytes: 52_403,
        artifact_sha256: "af674c5d95989d65479de833c81418f815b9d4112db308c3c3c4e652686fa9f5",
        timeline_sha256: "65a9a91c0b9bc6342a4343db11f8bae8b8ae34c8d4e02273e7c90471572fd6dd",
        activities: &[],
    },
    Target {
        path: "models/props_2fort/frog.mdl",
        mdl_sha256: "6ec4727763b46d37b7aabd85e210c33be1eac694b402fa101551fd1ad3378f78",
        bones: 1,
        animations: 1,
        sequences: 1,
        materials: 1,
        skins: 1,
        body_parts: 1,
        attachments: 0,
        primitives: 1,
        lods: &[0],
        physics: studio::PhysicsStatus::Missing,
        first_material: "materials/models/props_2fort/frog001.vmt",
        artifact_bytes: 120_661,
        artifact_sha256: "56db74a275f5db8c066b325bbbd3e8f91a8327965ca21c1210eec38c417bd8e6",
        timeline_sha256: "65a9a91c0b9bc6342a4343db11f8bae8b8ae34c8d4e02273e7c90471572fd6dd",
        activities: &[],
    },
    Target {
        path: "models/props_gameplay/resupply_locker.mdl",
        mdl_sha256: "cfca762077d5b1f252ccd448e904de4f9207da925215e0487f857c7f707a1bd2",
        bones: 3,
        animations: 3,
        sequences: 3,
        materials: 4,
        skins: 1,
        body_parts: 1,
        attachments: 0,
        primitives: 20,
        lods: &[0, 1, 2, 3, 4],
        physics: studio::PhysicsStatus::Present,
        first_material: "materials/models/props_gameplay/resupply_locker.vmt",
        artifact_bytes: 998_429,
        artifact_sha256: "8356e2c21ec854a3ec5a92596518642b73827023877dd59802753bdeab6c3c6c",
        timeline_sha256: "bcaff50ed60d571f2eba900b1a98f009c43b638bc69c00ae53f331651846fd43",
        activities: &[],
    },
    Target {
        path: "models/player/items/soldier/soldier_viking.mdl",
        mdl_sha256: "2f7f2f7aab04188977985195378e18285204d71e037abc9d5da49b8b140ba561",
        bones: 1,
        animations: 1,
        sequences: 1,
        materials: 2,
        skins: 2,
        body_parts: 1,
        attachments: 0,
        primitives: 3,
        lods: &[0, 1, 2],
        physics: studio::PhysicsStatus::Present,
        first_material: "materials/models/player/items/soldier/soldier_viking.vmt",
        artifact_bytes: 258_490,
        artifact_sha256: "73bf0c80b29130a2b93c9c7f162db5d7875b46c04ce6f9c91d053702eefaeaa3",
        timeline_sha256: "9b08336afb4e9eb30508f8bad1b519e3d7679e59a10b31c16adc577124f02226",
        activities: &[],
    },
    Target {
        path: "models/player/soldier.mdl",
        mdl_sha256: "62bc48c0fa4ac4166151633087feadef103a2d86d4412639fe1adddba91de219",
        bones: 86,
        animations: 934,
        sequences: 495,
        materials: 20,
        skins: 8,
        body_parts: 5,
        attachments: 24,
        primitives: 50,
        lods: &[0, 1, 2, 3, 4, 5, 6],
        physics: studio::PhysicsStatus::Present,
        first_material: "materials/models/player/soldier/soldier_red.vmt",
        artifact_bytes: 41_473_885,
        artifact_sha256: "32f86ecbd068953f677358d53d5021ee987357d705abd37214ae4aef7387fd87",
        timeline_sha256: "0b07964a4ffc54a45c80df4daf7dff2eb5cb088da6abc16c306ea9853211be7d",
        activities: WORLD_ACTIVITIES,
    },
    Target {
        path: "models/player/demo.mdl",
        mdl_sha256: "b8fe45619b062197a975798310d703ec59f8c1ab53f901c462eed5e0fd34ef93",
        bones: 84,
        animations: 852,
        sequences: 406,
        materials: 17,
        skins: 8,
        body_parts: 4,
        attachments: 25,
        primitives: 40,
        lods: &[0, 1, 2, 3, 4, 5, 6],
        physics: studio::PhysicsStatus::Present,
        first_material: "materials/models/player/demo/demoman_red.vmt",
        artifact_bytes: 39_357_818,
        artifact_sha256: "824da007e9d4ea47e430be3145c0cdcb3f8b982adf467e105998b4fd02ae03b1",
        timeline_sha256: "75251b83cdc507c123fdd731d4b94733b6dcd9f752e50d7297862fb62cfec221",
        activities: WORLD_ACTIVITIES,
    },
    Target {
        path: "models/weapons/w_models/w_rocket.mdl",
        mdl_sha256: "c5856b209922950a29e183245976ae76305f4b66aae1997fbe36c41b7d0f1a84",
        bones: 1,
        animations: 1,
        sequences: 1,
        materials: 1,
        skins: 1,
        body_parts: 1,
        attachments: 1,
        primitives: 1,
        lods: &[0],
        physics: studio::PhysicsStatus::Present,
        first_material: "materials/models/weapons/w_rocketlauncher/w_rocket01.vmt",
        artifact_bytes: 60_431,
        artifact_sha256: "93d6da2b1914b56efac46eb7de751a0713ea9c377ef504bbbe9d02e27b61302d",
        timeline_sha256: "6770d4c90a26b5d55316a396ea5b6b4fbe7cd093ebf4ec534c990ce64846d4e0",
        activities: &[],
    },
    Target {
        path: "models/weapons/w_models/w_stickybomb.mdl",
        mdl_sha256: "b577c7f40fec381f6a42128782effe52df76788ca245e82072f76c8d6fda3791",
        bones: 1,
        animations: 1,
        sequences: 1,
        materials: 2,
        skins: 2,
        body_parts: 1,
        attachments: 0,
        primitives: 3,
        lods: &[0, 1, 2],
        physics: studio::PhysicsStatus::Present,
        first_material: "materials/models/weapons/w_stickybomb/w_stickybomb_red.vmt",
        artifact_bytes: 145_017,
        artifact_sha256: "0a1d519d3ada6fe49e0ab9ab5d160de626ec39e4b85204c0e0ecbe3d057e2ab4",
        timeline_sha256: "a4fc0efa81ceb888399cdf12b1c07caed644616c180a0f2b6fe053f81766f3d6",
        activities: &[],
    },
];

fn main() -> Result<(), String> {
    let root = root()?;
    let config: Config = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if !Path::new(&config.tf2_dir).is_absolute()
        || !Path::new(&config.source_cache_dir).is_absolute()
        || !Path::new(&config.asset_dir).is_absolute()
    {
        return Err("local configuration paths must be absolute".to_owned());
    }
    let cache = PathBuf::from(config.source_cache_dir);
    let bundle_bytes = fs::read(cache.join("browser-bundles/jump_beef.psdb"))
        .map_err(|error| error.to_string())?;
    if hex(&studio::content_sha256(&bundle_bytes)) != BUNDLE_SHA256 {
        return Err("configured source bundle identity changed".to_owned());
    }
    let files = parse_bundle(&bundle_bytes)?;
    let vpk_files = VpkFiles::new(Path::new(&config.tf2_dir))?;
    for target in TARGETS {
        verify_target(target, &files)?;
    }
    verify_stock_viewmodels(&vpk_files)?;
    verify_model_materials(&vpk_files)?;
    verify_occurrences(&cache)?;
    println!(
        "{{\"build\":\"{BUILD}\",\"bundleSha256\":\"{BUNDLE_SHA256}\",\"models\":{},\"status\":\"Ready\"}}",
        TARGETS.len()
    );
    Ok(())
}

fn verify_stock_viewmodels(files: &VpkFiles) -> Result<(), String> {
    for target in STOCK_VIEWMODELS {
        let hand_bytes = files
            .exact(target.hand)?
            .ok_or_else(|| format!("missing {}", target.hand))?;
        let item_bytes = files
            .exact(target.item)?
            .ok_or_else(|| format!("missing {}", target.item))?;
        if hex(&studio::content_sha256(&hand_bytes)) != target.hand_sha256
            || hex(&studio::content_sha256(&item_bytes)) != target.item_sha256
        {
            return Err(format!(
                "stock viewmodel source identity changed: {}",
                target.hand
            ));
        }
        let hand_document = load(target.hand, files)?;
        let item_document = load(target.item, files)?;
        let hand = build_artifact_for_profile(
            &hand_document,
            files,
            studio::PresentationProfile::ViewModel,
        )?;
        let item = build_artifact_for_profile(
            &item_document,
            files,
            studio::PresentationProfile::ViewModel,
        )?;
        let (hand_opacity, hand_material_states) = model_material_states(&hand.model, files)?;
        let (item_opacity, item_material_states) = model_material_states(&item.model, files)?;
        let mut digest = Vec::new();
        let mut producer_digest = Vec::new();
        let mut shapes = BTreeSet::new();
        let mut bodygroup_event_names = BTreeSet::new();
        digest.extend_from_slice(target.hand.as_bytes());
        digest.extend_from_slice(target.item.as_bytes());
        digest.extend_from_slice(&hand.sha256);
        digest.extend_from_slice(&item.sha256);
        producer_digest.extend_from_slice(target.hand.as_bytes());
        producer_digest.extend_from_slice(target.item.as_bytes());
        producer_digest.extend_from_slice(&hand.sha256);
        producer_digest.extend_from_slice(&item.sha256);
        append_material_states(&mut producer_digest, &hand.model, &hand_material_states);
        append_material_states(&mut producer_digest, &item.model, &item_material_states);
        for activity in target.activities {
            let sequences = studio::sequences_for_activity_name(&hand.model, activity.as_bytes());
            if sequences.len() != 1 {
                return Err(format!(
                    "{} does not have one {activity} sequence",
                    target.hand
                ));
            }
            for skin in [0_usize, 1] {
                for cycle in [0.0_f32, 0.5, 1.0] {
                    let hand_bodygroups =
                        hand.model.body_parts.iter().map(|_| 0).collect::<Vec<_>>();
                    let mut item_bodygroups =
                        item.model.body_parts.iter().map(|_| 0).collect::<Vec<_>>();
                    let bodygroup_events = studio::viewmodel_item_bodygroup_events(
                        &hand.model,
                        &item.model,
                        sequences[0],
                        studio::Float32((-0.01_f32).to_bits()),
                        studio::Float32(cycle.to_bits()),
                    )
                    .map_err(|error| error.to_string())?;
                    studio::apply_viewmodel_bodygroup_events(
                        &item.model,
                        &mut item_bodygroups,
                        &bodygroup_events,
                    )
                    .map_err(|error| error.to_string())?;
                    for mutation in &bodygroup_events {
                        bodygroup_event_names
                            .insert(String::from_utf8_lossy(&mutation.name).into_owned());
                    }
                    let composition = studio::compose_viewmodel(
                        &hand.model,
                        &item.model,
                        &studio::ViewModelCompositionRequest {
                            translated_activity: activity.as_bytes().to_vec(),
                            hand_sequence: sequences[0],
                            cycle: studio::Float32(cycle.to_bits()),
                            time: studio::Float32(0.0_f32.to_bits()),
                            hand_pose_parameters: hand
                                .model
                                .pose_parameters
                                .iter()
                                .map(|_| studio::Float32(0.0_f32.to_bits()))
                                .collect(),
                            hand_layers: Vec::new(),
                            skin,
                            hand_bodygroups: hand_bodygroups.clone(),
                            item_bodygroups: item_bodygroups.clone(),
                            lod: 0,
                        },
                    )
                    .map_err(|error| error.to_string())?;
                    if composition
                        .item_to_hand_bones
                        .iter()
                        .filter(|bone| bone.is_some())
                        .count()
                        != target.merged_bones
                        || composition.hand.primitives.is_empty()
                        || composition.item.primitives.is_empty()
                    {
                        return Err(format!("{} stock composition changed", target.item));
                    }
                    for part in [&composition.hand, &composition.item] {
                        let model = if part.identity == hand.model.identity {
                            &hand.model
                        } else {
                            &item.model
                        };
                        for selected in &part.primitives {
                            let geometry =
                                model.geometry.get(selected.primitive).ok_or_else(|| {
                                    format!("{} selected missing geometry", part.identity)
                                })?;
                            if geometry.vertices.is_empty() || geometry.triangles.is_empty() {
                                return Err(format!("{} selected empty geometry", part.identity));
                            }
                        }
                    }
                    let plan = studio::viewmodel_draw_plan(
                        &hand.model,
                        &item.model,
                        &composition,
                        &hand_opacity,
                        &item_opacity,
                    )
                    .map_err(|error| error.to_string())?;
                    if plan.item_entity_translucent
                        || plan.parts.len() != 2
                        || plan.parts[0].part != studio::ViewModelPart::Item
                        || plan.parts[1].part != studio::ViewModelPart::Hand
                        || plan.parts.iter().any(|part| {
                            part.opaque_primitives.is_empty()
                                || !part.translucent_primitives.is_empty()
                        })
                    {
                        return Err(format!("{} stock draw partition changed", target.item));
                    }
                    shapes.insert((
                        composition.hand.primitives.len(),
                        composition.item.primitives.len(),
                        composition.hand.pose.attachments.len(),
                        composition.item.pose.attachments.len(),
                    ));
                    digest.extend_from_slice(activity.as_bytes());
                    digest.extend_from_slice(&(skin as u32).to_le_bytes());
                    digest.extend_from_slice(&cycle.to_bits().to_le_bytes());
                    for bone in &composition.item_to_hand_bones {
                        digest.extend_from_slice(
                            &bone.map_or(u32::MAX, |bone| bone as u32).to_le_bytes(),
                        );
                    }
                    append_composed_part(&mut digest, &composition.hand);
                    append_composed_part(&mut digest, &composition.item);
                    producer_digest.extend_from_slice(activity.as_bytes());
                    producer_digest.extend_from_slice(&(skin as u32).to_le_bytes());
                    producer_digest.extend_from_slice(&cycle.to_bits().to_le_bytes());
                    append_bodygroups(&mut producer_digest, &hand.model, &hand_bodygroups);
                    append_bodygroups(&mut producer_digest, &item.model, &item_bodygroups);
                    for mutation in bodygroup_events {
                        producer_digest.extend_from_slice(&(mutation.event as u32).to_le_bytes());
                        producer_digest
                            .extend_from_slice(&(mutation.bodygroup as u32).to_le_bytes());
                        producer_digest.extend_from_slice(&mutation.value.to_le_bytes());
                        producer_digest.extend_from_slice(&mutation.name);
                    }
                    append_producer_part(&mut producer_digest, &hand.model, &composition.hand);
                    append_producer_part(&mut producer_digest, &item.model, &composition.item);
                    append_draw_plan(&mut producer_digest, &plan);
                }
            }
        }
        let composition_sha256 = hex(&studio::content_sha256(&digest));
        println!(
            "stock hand={} item={} composition={composition_sha256}",
            target.hand, target.item
        );
        if composition_sha256 != target.composition_sha256 {
            return Err(format!("{} composition hash changed", target.item));
        }
        let producer_sha256 = hex(&studio::content_sha256(&producer_digest));
        println!(
            "stock hand={} item={} producer={producer_sha256}",
            target.hand, target.item
        );
        println!(
            "stock shapes={shapes:?} bodygroupEvents={bodygroup_event_names:?} handBodyparts={:?} itemBodyparts={:?}",
            hand.model
                .body_parts
                .iter()
                .map(|part| String::from_utf8_lossy(&part.name).into_owned())
                .collect::<Vec<_>>(),
            item.model
                .body_parts
                .iter()
                .map(|part| String::from_utf8_lossy(&part.name).into_owned())
                .collect::<Vec<_>>(),
        );
        if producer_sha256 != target.producer_sha256 {
            return Err(format!(
                "{} producer hash changed: {producer_sha256}",
                target.item
            ));
        }
    }
    Ok(())
}

fn append_composed_part(output: &mut Vec<u8>, part: &studio::ComposedViewModelPart) {
    output.extend_from_slice(part.identity.as_bytes());
    for matrix in &part.pose.model_matrices {
        for value in matrix.0 {
            output.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    for primitive in &part.primitives {
        output.extend_from_slice(&(primitive.primitive as u32).to_le_bytes());
        output.extend_from_slice(&(primitive.material as u32).to_le_bytes());
    }
}

fn model_material_states(
    model: &studio::PresentationModel,
    files: &VpkFiles,
) -> Result<
    (
        Vec<studio::ViewModelMaterialOpacity>,
        Vec<playsrc_material::ModelDrawState>,
    ),
    String,
> {
    let mut opacity = Vec::with_capacity(model.materials.len());
    let mut states = Vec::with_capacity(model.materials.len());
    for presentation in &model.materials {
        let identity = &model
            .dependencies
            .get(presentation.material_dependency)
            .ok_or_else(|| format!("{} material dependency missing", model.identity))?
            .logical_path;
        let (material, _) = resolved_material(identity, files)?;
        let base_alpha = if let Some(base) = material
            .textures
            .iter()
            .find(|texture| texture.role == playsrc_material::TextureRole::Base)
            .and_then(|texture| texture.logical_path.as_ref())
        {
            let bytes = files
                .exact(base)?
                .ok_or_else(|| format!("missing model base texture {base}"))?;
            let metadata = playsrc_vtf::inspect(
                &bytes,
                playsrc_vtf::Dialect::Source2013Pc,
                playsrc_vtf::Limits::default(),
            )
            .map_err(|error| error.to_string())?;
            metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit
        } else {
            false
        };
        let state = playsrc_material::model_draw_state(
            &material,
            playsrc_material::TextureAlphaFacts { base: base_alpha },
            playsrc_material::ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: Some(0.0),
            },
        )
        .map_err(|error| format!("{identity}: {error}"))?;
        opacity.push(match state.opacity {
            playsrc_material::ModelOpacity::Opaque => studio::ViewModelMaterialOpacity::Opaque,
            playsrc_material::ModelOpacity::Translucent => {
                studio::ViewModelMaterialOpacity::Translucent
            }
        });
        states.push(state);
    }
    Ok((opacity, states))
}

fn append_material_states(
    output: &mut Vec<u8>,
    model: &studio::PresentationModel,
    states: &[playsrc_material::ModelDrawState],
) {
    for (material, state) in model.materials.iter().zip(states) {
        let dependency = &model.dependencies[material.material_dependency];
        output.extend_from_slice(dependency.logical_path.as_bytes());
        output.push(match state.opacity {
            playsrc_material::ModelOpacity::Opaque => 0,
            playsrc_material::ModelOpacity::Translucent => 1,
        });
        output.push(match state.framebuffer {
            playsrc_material::ModelFramebufferRequirement::None => 0,
            playsrc_material::ModelFramebufferRequirement::Potential => 1,
            playsrc_material::ModelFramebufferRequirement::Current => 2,
        });
        output.push(u8::from(state.static_state.blend.enabled));
        output.push(u8::from(state.static_state.alpha_test));
        output.push(u8::from(state.static_state.depth_test));
        output.push(u8::from(state.static_state.depth_write));
        output.push(u8::from(state.effective_self_illumination));
        output.push(u8::from(state.effective_base_alpha_environment_mask));
        for required in &state.required_inputs {
            output.push(*required as u8);
        }
        output.push(0xff);
    }
}

fn append_bodygroups(
    output: &mut Vec<u8>,
    model: &studio::PresentationModel,
    bodygroups: &[usize],
) {
    for (part, selected) in model.body_parts.iter().zip(bodygroups) {
        output.extend_from_slice(&part.name);
        output.extend_from_slice(&(*selected as u32).to_le_bytes());
        output.extend_from_slice(&part.model_names[*selected]);
    }
}

fn append_producer_part(
    output: &mut Vec<u8>,
    model: &studio::PresentationModel,
    part: &studio::ComposedViewModelPart,
) {
    append_composed_part(output, part);
    for attachment in &part.pose.attachments {
        output.extend_from_slice(&(attachment.index as u32).to_le_bytes());
        output.extend_from_slice(&attachment.name);
        for value in attachment.model_transform.0 {
            output.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    for selected in &part.primitives {
        let geometry = &model.geometry[selected.primitive];
        let material = &model.materials[selected.material];
        output.extend_from_slice(&(geometry.vertices.len() as u32).to_le_bytes());
        output.extend_from_slice(&(geometry.triangles.len() as u32).to_le_bytes());
        output.extend_from_slice(
            model.dependencies[material.material_dependency]
                .logical_path
                .as_bytes(),
        );
    }
}

fn append_draw_plan(output: &mut Vec<u8>, plan: &studio::ViewModelDrawPlan) {
    output.push(u8::from(plan.item_entity_translucent));
    for part in &plan.parts {
        output.push(match part.part {
            studio::ViewModelPart::Hand => 0,
            studio::ViewModelPart::Item => 1,
        });
        output.extend_from_slice(part.identity.as_bytes());
        for selected in &part.opaque_primitives {
            output.extend_from_slice(&(selected.primitive as u32).to_le_bytes());
            output.extend_from_slice(&(selected.material as u32).to_le_bytes());
        }
        output.extend_from_slice(&u32::MAX.to_le_bytes());
        for selected in &part.translucent_primitives {
            output.extend_from_slice(&(selected.primitive as u32).to_le_bytes());
            output.extend_from_slice(&(selected.material as u32).to_le_bytes());
        }
        output.extend_from_slice(&u32::MAX.to_le_bytes());
    }
}

fn verify_model_materials(files: &VpkFiles) -> Result<(), String> {
    let model_roots = [
        "models/props_2fort/cow001_reference.mdl",
        "models/props_2fort/frog.mdl",
        "models/props_gameplay/resupply_locker.mdl",
        "models/player/items/soldier/soldier_viking.mdl",
        "models/player/soldier.mdl",
        "models/player/demo.mdl",
        "models/weapons/w_models/w_rocket.mdl",
        "models/weapons/w_models/w_stickybomb.mdl",
        "models/weapons/c_models/c_soldier_arms.mdl",
        "models/weapons/c_models/c_demo_arms.mdl",
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
    ];
    let mut material_paths = BTreeSet::new();
    for root in model_roots {
        let document = load(root, files)?;
        for material in &document.materials {
            let mut selected = None;
            for candidate in &material.candidates {
                if files.exact(candidate)?.is_some() {
                    selected = Some(candidate.to_ascii_lowercase());
                    break;
                }
            }
            material_paths.insert(
                selected.ok_or_else(|| format!("{root} has no present material candidate"))?,
            );
        }
    }

    let mut texture_evidence = BTreeMap::<String, TextureEvidence>::new();
    let mut material_count = 0_usize;
    let mut eye_count = 0_usize;
    let mut mip_digest = Vec::new();
    let mut handled_proxies = BTreeSet::new();
    let mut unsupported_proxies = BTreeSet::new();
    let mut draw_digest = Vec::new();
    let mut opaque_materials = 0_usize;
    let mut translucent_materials = 0_usize;
    let mut alpha_tested_materials = 0_usize;
    let mut current_framebuffer_materials = 0_usize;
    for identity in &material_paths {
        let material_bytes = files
            .exact(identity)?
            .ok_or_else(|| format!("missing model material {identity}"))?;
        let (material, _) = resolved_material(identity, files)?;
        let model = material
            .model
            .as_ref()
            .ok_or_else(|| format!("{identity} lacks model shader state"))?;
        material_count += 1;
        if model.shader == playsrc_material::ModelShader::EyeRefract {
            eye_count += 1;
        }
        let draw_state = playsrc_material::model_draw_state(
            &material,
            playsrc_material::TextureAlphaFacts {
                base: base_texture_alpha(&material, files)?,
            },
            playsrc_material::ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: Some(0.0),
            },
        )
        .map_err(|error| format!("{identity}: {error}"))?;
        if playsrc_material::missing_model_draw_inputs(&draw_state, &[])
            != draw_state.required_inputs
        {
            return Err(format!("{identity} model input requirements changed"));
        }
        match draw_state.opacity {
            playsrc_material::ModelOpacity::Opaque => opaque_materials += 1,
            playsrc_material::ModelOpacity::Translucent => translucent_materials += 1,
        }
        alpha_tested_materials += usize::from(draw_state.static_state.alpha_test);
        current_framebuffer_materials += usize::from(
            draw_state.framebuffer == playsrc_material::ModelFramebufferRequirement::Current,
        );
        draw_digest.extend_from_slice(identity.as_bytes());
        draw_digest.push(model.shader as u8);
        draw_digest.push(draw_state.opacity as u8);
        draw_digest.push(draw_state.framebuffer as u8);
        draw_digest.push(u8::from(draw_state.static_state.blend.enabled));
        draw_digest.push(u8::from(draw_state.static_state.alpha_test));
        draw_digest.push(u8::from(draw_state.static_state.depth_test));
        draw_digest.push(u8::from(draw_state.static_state.depth_write));
        draw_digest.push(u8::from(draw_state.effective_self_illumination));
        draw_digest.push(u8::from(draw_state.effective_base_alpha_environment_mask));
        for input in &draw_state.required_inputs {
            draw_digest.push(*input as u8);
        }
        draw_digest.push(0xff);
        mip_digest.extend_from_slice(identity.as_bytes());
        mip_digest.extend_from_slice(&studio::content_sha256(&material_bytes));
        mip_digest.push(match model.shader {
            playsrc_material::ModelShader::VertexLitGeneric => 0,
            playsrc_material::ModelShader::EyeRefract => 1,
            playsrc_material::ModelShader::Eyes => 2,
        });
        for entry in &material.proxy_program.entries {
            let name = String::from_utf8_lossy(&entry.name).into_owned();
            match entry.disposition {
                playsrc_material::ProxyDisposition::Handled => {
                    handled_proxies.insert(name);
                }
                playsrc_material::ProxyDisposition::Unsupported => {
                    unsupported_proxies.insert(name);
                }
                playsrc_material::ProxyDisposition::Malformed => {
                    return Err(format!("{identity} has malformed model proxy {name}"));
                }
            }
        }
        for texture in &material.textures {
            if texture.disposition != playsrc_material::TextureDisposition::Source {
                continue;
            }
            let path = texture
                .logical_path
                .as_ref()
                .ok_or_else(|| format!("{identity} source texture has no path"))?
                .to_ascii_lowercase();
            if !texture_evidence.contains_key(&path) {
                let bytes = files
                    .exact(&path)?
                    .ok_or_else(|| format!("missing model texture {path}"))?;
                let metadata = playsrc_vtf::inspect(
                    &bytes,
                    playsrc_vtf::Dialect::Source2013Pc,
                    playsrc_vtf::Limits::default(),
                )
                .map_err(|error| error.to_string())?;
                let sha256 = studio::content_sha256(&bytes);
                let manifest = texture_manifest(&metadata)?;
                let mut planes = Vec::with_capacity(manifest.subresources.len());
                for identity in &manifest.subresources {
                    let selector = vtf_identity(*identity);
                    let plane = playsrc_vtf::decode(
                        &bytes,
                        playsrc_vtf::Dialect::Source2013Pc,
                        selector,
                        playsrc_vtf::Limits::default(),
                    )
                    .map_err(|error| format!("{path}: {error}"))?;
                    planes.push(playsrc_material::AuthoredTexturePlane {
                        identity: *identity,
                        width: plane.width,
                        height: plane.height,
                        row_stride: plane.row_stride,
                        sample_bytes: plane.samples.len(),
                    });
                }
                texture_evidence.insert(
                    path.clone(),
                    TextureEvidence {
                        sha256,
                        manifest,
                        planes,
                    },
                );
            }
            let evidence = texture_evidence
                .get(&path)
                .ok_or_else(|| format!("missing cached texture evidence {path}"))?;
            let binding = playsrc_material::bind_authored_texture(texture, &evidence.manifest)
                .map_err(|error| error.to_string())?;
            playsrc_material::validate_authored_planes(&binding, &evidence.planes)
                .map_err(|error| error.to_string())?;
            mip_digest.extend_from_slice(&texture_role_code(texture.role).to_le_bytes());
            mip_digest.extend_from_slice(path.as_bytes());
            mip_digest.extend_from_slice(&evidence.sha256);
            mip_digest.push(binding.mip_count);
            mip_digest.extend_from_slice(&binding.frame_count.to_le_bytes());
            mip_digest.extend_from_slice(&(binding.subresources.len() as u32).to_le_bytes());
            for identity in binding.subresources {
                append_texture_identity(&mut mip_digest, identity);
            }
        }
        for texture in &material.model_textures {
            let path = texture.logical_path.to_ascii_lowercase();
            if !texture_evidence.contains_key(&path) {
                let bytes = files
                    .exact(&path)?
                    .ok_or_else(|| format!("missing model texture {path}"))?;
                let metadata = playsrc_vtf::inspect(
                    &bytes,
                    playsrc_vtf::Dialect::Source2013Pc,
                    playsrc_vtf::Limits::default(),
                )
                .map_err(|error| error.to_string())?;
                let sha256 = studio::content_sha256(&bytes);
                let manifest = texture_manifest(&metadata)?;
                let mut planes = Vec::with_capacity(manifest.subresources.len());
                for identity in &manifest.subresources {
                    let plane = playsrc_vtf::decode(
                        &bytes,
                        playsrc_vtf::Dialect::Source2013Pc,
                        vtf_identity(*identity),
                        playsrc_vtf::Limits::default(),
                    )
                    .map_err(|error| format!("{path}: {error}"))?;
                    planes.push(playsrc_material::AuthoredTexturePlane {
                        identity: *identity,
                        width: plane.width,
                        height: plane.height,
                        row_stride: plane.row_stride,
                        sample_bytes: plane.samples.len(),
                    });
                }
                texture_evidence.insert(
                    path.clone(),
                    TextureEvidence {
                        sha256,
                        manifest,
                        planes,
                    },
                );
            }
            let evidence = texture_evidence
                .get(&path)
                .ok_or_else(|| format!("missing cached texture evidence {path}"))?;
            let binding =
                playsrc_material::bind_authored_model_texture(texture, &evidence.manifest)
                    .map_err(|error| error.to_string())?;
            playsrc_material::validate_authored_planes(&binding, &evidence.planes)
                .map_err(|error| error.to_string())?;
            mip_digest
                .extend_from_slice(&(0x8000 | model_texture_role_code(texture.role)).to_le_bytes());
            mip_digest.extend_from_slice(path.as_bytes());
            mip_digest.extend_from_slice(&evidence.sha256);
            mip_digest.push(binding.mip_count);
            mip_digest.extend_from_slice(&binding.frame_count.to_le_bytes());
            mip_digest.extend_from_slice(&(binding.subresources.len() as u32).to_le_bytes());
            for identity in binding.subresources {
                append_texture_identity(&mut mip_digest, identity);
            }
        }
    }
    if eye_count != 3 {
        return Err(format!("target eye material count changed: {eye_count}"));
    }
    let texture_count = texture_evidence.len();
    let mip_sha256 = hex(&studio::content_sha256(&mip_digest));
    let draw_sha256 = hex(&studio::content_sha256(&draw_digest));
    let eye_sha256 = verify_eye_states(files)?;
    let expected_proxies = [
        "AnimatedTexture",
        "AnimatedWeaponSheen",
        "BurnLevel",
        "Equals",
        "InvulnLevel",
        "ItemTintColor",
        "LessOrEqual",
        "ModelGlowColor",
        "Multiply",
        "SelectFirstIfNonZero",
        "Sine",
        "StickybombGlowColor",
        "WeaponSkin",
        "YellowLevel",
        "invis",
        "spy_invis",
        "weapon_invis",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    if !unsupported_proxies.is_empty() || handled_proxies != expected_proxies {
        return Err(format!(
            "model proxy inventory changed: handled={handled_proxies:?} unsupported={unsupported_proxies:?}"
        ));
    }
    println!(
        "modelMaterials={material_count} modelTextures={texture_count} mipSha256={mip_sha256} eyeSha256={eye_sha256}"
    );
    println!(
        "modelDrawSha256={draw_sha256} opaque={opaque_materials} translucent={translucent_materials} alphaTested={alpha_tested_materials} currentFramebuffer={current_framebuffer_materials}"
    );
    println!("handledModelProxies={handled_proxies:?}");
    if material_count != MODEL_MATERIAL_COUNT
        || texture_count != MODEL_TEXTURE_COUNT
        || mip_sha256 != MODEL_MIP_SHA256
        || draw_sha256 != MODEL_DRAW_SHA256
        || eye_sha256 != EYE_STATE_SHA256
    {
        return Err("model material, mip, or eye evidence changed".to_owned());
    }
    Ok(())
}

fn base_texture_alpha(
    material: &playsrc_material::Material,
    files: &VpkFiles,
) -> Result<bool, String> {
    let Some(path) = material
        .textures
        .iter()
        .find(|texture| texture.role == playsrc_material::TextureRole::Base)
        .and_then(|texture| texture.logical_path.as_ref())
    else {
        return Ok(false);
    };
    let bytes = files
        .exact(path)?
        .ok_or_else(|| format!("missing model base texture {path}"))?;
    let metadata = playsrc_vtf::inspect(
        &bytes,
        playsrc_vtf::Dialect::Source2013Pc,
        playsrc_vtf::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    Ok(metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit)
}

struct TextureEvidence {
    sha256: [u8; 32],
    manifest: playsrc_material::TextureMetadataManifest,
    planes: Vec<playsrc_material::AuthoredTexturePlane>,
}

fn texture_manifest(
    metadata: &playsrc_vtf::Metadata,
) -> Result<playsrc_material::TextureMetadataManifest, String> {
    let environment = playsrc_vtf::SamplingEnvironment {
        shader_model: 90,
        force_anisotropy: 1,
        maximum_anisotropy: 16,
        force_trilinear: false,
    };
    let sampling = playsrc_vtf::sampling_state(metadata, environment);
    Ok(playsrc_material::TextureMetadataManifest {
        width: metadata.width,
        height: metadata.height,
        depth: metadata.depth,
        mip_count: metadata.mip_count,
        frame_count: metadata.frame_count,
        faces: metadata.faces.iter().copied().map(material_face).collect(),
        sampling: playsrc_material::TextureSamplingState {
            wrap_s: material_wrap(sampling.wrap_s),
            wrap_t: material_wrap(sampling.wrap_t),
            wrap_u: material_wrap(sampling.wrap_u),
            min_filter: match sampling.min_filter {
                playsrc_vtf::MinFilter::Nearest => playsrc_material::TextureMinFilter::Nearest,
                playsrc_vtf::MinFilter::Linear => playsrc_material::TextureMinFilter::Linear,
                playsrc_vtf::MinFilter::LinearMipmapNearest => {
                    playsrc_material::TextureMinFilter::LinearMipmapNearest
                }
                playsrc_vtf::MinFilter::LinearMipmapLinear => {
                    playsrc_material::TextureMinFilter::LinearMipmapLinear
                }
                playsrc_vtf::MinFilter::Anisotropic => {
                    playsrc_material::TextureMinFilter::Anisotropic
                }
            },
            mag_filter: match sampling.mag_filter {
                playsrc_vtf::MagFilter::Nearest => playsrc_material::TextureMagFilter::Nearest,
                playsrc_vtf::MagFilter::Linear => playsrc_material::TextureMagFilter::Linear,
                playsrc_vtf::MagFilter::Anisotropic => {
                    playsrc_material::TextureMagFilter::Anisotropic
                }
            },
            anisotropy_level: if sampling.min_filter == playsrc_vtf::MinFilter::Anisotropic
                || sampling.mag_filter == playsrc_vtf::MagFilter::Anisotropic
            {
                selected_anisotropy_level(
                    environment.force_anisotropy,
                    environment.maximum_anisotropy,
                )
            } else {
                1
            },
            mipmapped: sampling.mipmapped,
            no_lod: sampling.no_lod,
            all_mips: sampling.all_mips,
        },
        subresources: metadata
            .subresources
            .iter()
            .filter_map(|subresource| match subresource.identity {
                playsrc_vtf::SubresourceIdentity::LowResolution => None,
                playsrc_vtf::SubresourceIdentity::HighResolution {
                    mip,
                    frame,
                    face,
                    slice,
                } => Some(playsrc_material::TextureSubresourceIdentity {
                    mip,
                    frame,
                    face: material_face(face),
                    slice,
                }),
            })
            .collect(),
    })
}

fn selected_anisotropy_level(configured: u8, maximum: u8) -> u8 {
    if configured <= 1 || configured > maximum {
        (maximum / 4).clamp(2, 8)
    } else {
        configured
    }
}

fn material_face(face: playsrc_vtf::Face) -> playsrc_material::TextureFace {
    match face {
        playsrc_vtf::Face::Right => playsrc_material::TextureFace::Right,
        playsrc_vtf::Face::Left => playsrc_material::TextureFace::Left,
        playsrc_vtf::Face::Back => playsrc_material::TextureFace::Back,
        playsrc_vtf::Face::Front => playsrc_material::TextureFace::Front,
        playsrc_vtf::Face::Up => playsrc_material::TextureFace::Up,
        playsrc_vtf::Face::Down => playsrc_material::TextureFace::Down,
        playsrc_vtf::Face::Sphere => playsrc_material::TextureFace::Sphere,
    }
}

fn material_wrap(wrap: playsrc_vtf::WrapMode) -> playsrc_material::TextureWrapMode {
    match wrap {
        playsrc_vtf::WrapMode::Repeat => playsrc_material::TextureWrapMode::Repeat,
        playsrc_vtf::WrapMode::Clamp => playsrc_material::TextureWrapMode::Clamp,
        playsrc_vtf::WrapMode::Border => playsrc_material::TextureWrapMode::Border,
    }
}

fn vtf_identity(
    identity: playsrc_material::TextureSubresourceIdentity,
) -> playsrc_vtf::SubresourceIdentity {
    playsrc_vtf::SubresourceIdentity::HighResolution {
        mip: identity.mip,
        frame: identity.frame,
        face: match identity.face {
            playsrc_material::TextureFace::Right => playsrc_vtf::Face::Right,
            playsrc_material::TextureFace::Left => playsrc_vtf::Face::Left,
            playsrc_material::TextureFace::Back => playsrc_vtf::Face::Back,
            playsrc_material::TextureFace::Front => playsrc_vtf::Face::Front,
            playsrc_material::TextureFace::Up => playsrc_vtf::Face::Up,
            playsrc_material::TextureFace::Down => playsrc_vtf::Face::Down,
            playsrc_material::TextureFace::Sphere => playsrc_vtf::Face::Sphere,
        },
        slice: identity.slice,
    }
}

fn append_texture_identity(
    output: &mut Vec<u8>,
    identity: playsrc_material::TextureSubresourceIdentity,
) {
    output.push(identity.mip);
    output.extend_from_slice(&identity.frame.to_le_bytes());
    output.push(match identity.face {
        playsrc_material::TextureFace::Right => 0,
        playsrc_material::TextureFace::Left => 1,
        playsrc_material::TextureFace::Back => 2,
        playsrc_material::TextureFace::Front => 3,
        playsrc_material::TextureFace::Up => 4,
        playsrc_material::TextureFace::Down => 5,
        playsrc_material::TextureFace::Sphere => 6,
    });
    output.extend_from_slice(&identity.slice.to_le_bytes());
}

fn texture_role_code(role: playsrc_material::TextureRole) -> u16 {
    use playsrc_material::TextureRole as Role;
    match role {
        Role::Base => 0,
        Role::HdrBase => 1,
        Role::HdrCompressed => 2,
        Role::HdrCompressed0 => 3,
        Role::HdrCompressed1 => 4,
        Role::HdrCompressed2 => 5,
        Role::Base2 => 6,
        Role::Bump => 7,
        Role::Normal => 8,
        Role::Bump2 => 9,
        Role::Detail => 10,
        Role::BlendModulate => 11,
        Role::Environment => 12,
        Role::EnvironmentMask => 13,
        Role::SelfIllumMask => 14,
        Role::Flow => 15,
        Role::Reflection => 16,
        Role::Refraction => 17,
    }
}

fn model_texture_role_code(role: playsrc_material::ModelTextureRole) -> u16 {
    use playsrc_material::ModelTextureRole as Role;
    match role {
        Role::Albedo => 0,
        Role::WrinkleCompress => 1,
        Role::WrinkleStretch => 2,
        Role::BumpCompress => 3,
        Role::BumpStretch => 4,
        Role::PhongExponent => 5,
        Role::LightWarp => 6,
        Role::PhongWarp => 7,
        Role::EyeIris => 8,
        Role::EyeCornea => 9,
        Role::EyeAmbientOcclusion => 10,
        Role::EyeGlint => 11,
        Role::SheenEnvironment => 12,
        Role::SheenMask => 13,
    }
}

fn verify_eye_states(files: &VpkFiles) -> Result<String, String> {
    let mut digest = Vec::new();
    for (identity, expected) in [
        ("models/player/soldier.mdl", 2_usize),
        ("models/player/demo.mdl", 1_usize),
    ] {
        let document = load(identity, files)?;
        let artifact =
            build_artifact_for_profile(&document, files, studio::PresentationProfile::World)?;
        let pose = studio::sample_pose(
            &artifact.model,
            &studio::AnimationState {
                base_sequence: 0,
                cycle: studio::Float32(0.0_f32.to_bits()),
                pose_parameters: artifact
                    .model
                    .pose_parameters
                    .iter()
                    .map(|_| studio::Float32(0.0_f32.to_bits()))
                    .collect(),
                layers: Vec::new(),
            },
        )
        .map_err(|error| error.to_string())?;
        let states = studio::eye_draw_states(
            &document,
            &studio::EyeDrawRequest {
                body_part: 0,
                submodel: 0,
                bone_to_world: &pose.model_matrices,
                world_target: vector([100.0, 0.0, 0.0]),
                view_right: vector([0.0, -1.0, 0.0]),
                view_up: vector([0.0, 0.0, 1.0]),
                configuration: studio::EyeConfiguration {
                    move_eyes: true,
                    shift: vector([0.0; 3]),
                    size: studio::Float32(0.0_f32.to_bits()),
                },
            },
        )
        .map_err(|error| error.to_string())?;
        if states.len() != expected {
            return Err(format!("{identity} eye count changed"));
        }
        digest.extend_from_slice(identity.as_bytes());
        for state in states {
            digest.extend_from_slice(&(state.mesh as u32).to_le_bytes());
            digest.extend_from_slice(&(state.eyeball as u32).to_le_bytes());
            digest.extend_from_slice(&(state.texture as u32).to_le_bytes());
            for value in state
                .world_origin
                .0
                .into_iter()
                .chain(state.iris_u)
                .chain(state.iris_v)
                .chain(state.glint_u)
                .chain(state.glint_v)
            {
                digest.extend_from_slice(&value.0.to_le_bytes());
            }
        }
    }
    Ok(hex(&studio::content_sha256(&digest)))
}

fn verify_target(target: &Target, files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let mdl = files
        .get(target.path)
        .ok_or_else(|| format!("missing {}", target.path))?;
    if hex(&studio::content_sha256(mdl)) != target.mdl_sha256 {
        return Err(format!("{} MDL identity changed", target.path));
    }
    let document = load(target.path, files)?;
    let lods = document
        .geometry
        .iter()
        .map(|primitive| primitive.lod)
        .collect::<BTreeSet<_>>();
    if document.bones.len() != target.bones
        || document.animations.len() != target.animations
        || document.sequences.len() != target.sequences
        || document.materials.len() != target.materials
        || document.skins.len() != target.skins
        || document.body_parts.len() != target.body_parts
        || document.attachments.len() != target.attachments
        || document.geometry.len() != target.primitives
        || document.physics_status != target.physics
        || lods != target.lods.iter().copied().collect()
    {
        return Err(format!("{} structural matrix changed", target.path));
    }
    let selected = document.materials[0]
        .candidates
        .iter()
        .find(|candidate| files.contains_key(candidate.as_str()))
        .map(String::as_str);
    if selected != Some(target.first_material) {
        return Err(format!("{} first material changed", target.path));
    }

    let first = build_artifact(&document, files)?;
    let second = build_artifact(&document, files)?;
    if first.bytes != second.bytes
        || first.bytes.len() != target.artifact_bytes
        || hex(&first.sha256) != target.artifact_sha256
        || studio::decode_presentation(&first.bytes, studio::PresentationLimits::default())
            .map_err(|error| error.to_string())?
            != first
        || first.model.hitbox_sets != document.hitbox_sets
        || first.model.physics_status != target.physics
    {
        return Err(format!("{} presentation artifact changed", target.path));
    }
    let timeline = timeline_digest(&first.model, target.activities)?;
    if timeline != target.timeline_sha256 {
        return Err(format!("{} timeline changed: {timeline}", target.path));
    }
    println!(
        "{} bytes={} artifact={} timeline={}",
        target.path, target.artifact_bytes, target.artifact_sha256, target.timeline_sha256
    );
    Ok(())
}

fn timeline_digest(
    model: &studio::PresentationModel,
    activities: &[&str],
) -> Result<String, String> {
    let poses = model
        .pose_parameters
        .iter()
        .map(|_| studio::Float32(0.0_f32.to_bits()))
        .collect::<Vec<_>>();
    let mut bytes = Vec::new();
    if activities.is_empty() {
        append_sample(&mut bytes, model, 0, 0.0, &poses)?;
    }
    for activity in activities {
        let sequence = studio::sequences_for_activity_name(model, activity.as_bytes())
            .first()
            .copied()
            .ok_or_else(|| format!("{} missing {activity}", model.identity))?;
        let timing =
            studio::sequence_timing(model, sequence, &poses).map_err(|error| error.to_string())?;
        if f32::from_bits(timing.frames_per_second.0) <= 0.0
            || f32::from_bits(timing.duration_seconds.0) <= 0.0
        {
            return Err(format!("{} invalid {activity} timing", model.identity));
        }
        bytes.extend_from_slice(activity.as_bytes());
        bytes.extend_from_slice(&(sequence as u32).to_le_bytes());
        for value in [
            timing.frames_per_second,
            timing.weighted_frame_count,
            timing.cycles_per_second,
            timing.duration_seconds,
        ] {
            bytes.extend_from_slice(&value.0.to_le_bytes());
        }
        for cycle in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            append_sample(&mut bytes, model, sequence, cycle, &poses)?;
        }
    }
    Ok(hex(&studio::content_sha256(&bytes)))
}

fn append_sample(
    output: &mut Vec<u8>,
    model: &studio::PresentationModel,
    sequence: usize,
    cycle: f32,
    pose_parameters: &[studio::Float32],
) -> Result<(), String> {
    let pose = studio::sample_pose(
        model,
        &studio::AnimationState {
            base_sequence: sequence,
            cycle: studio::Float32(cycle.to_bits()),
            pose_parameters: pose_parameters.to_vec(),
            layers: Vec::new(),
        },
    )
    .map_err(|error| error.to_string())?;
    output.extend_from_slice(&cycle.to_bits().to_le_bytes());
    for matrix in pose.skinning_matrices.iter().chain(
        pose.attachments
            .iter()
            .map(|attachment| &attachment.model_transform),
    ) {
        for value in matrix.0 {
            output.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    Ok(())
}

fn build_artifact(
    document: &studio::Document,
    files: &impl ExactFiles,
) -> Result<studio::PresentationArtifact, String> {
    let profile = if document.identity.contains("/v_models/") {
        studio::PresentationProfile::ViewModel
    } else {
        studio::PresentationProfile::World
    };
    build_artifact_for_profile(document, files, profile)
}

fn build_artifact_for_profile(
    document: &studio::Document,
    files: &impl ExactFiles,
    profile: studio::PresentationProfile,
) -> Result<studio::PresentationArtifact, String> {
    let mut responses = Vec::new();
    loop {
        match studio::build_presentation(
            document,
            profile,
            &responses,
            studio::PresentationLimits::default(),
            &studio::CancellationToken::default(),
        )
        .map_err(|error| error.to_string())?
        {
            studio::PresentationBuild::Complete(artifact) => return Ok(*artifact),
            studio::PresentationBuild::Needs(requests) => {
                for request in requests {
                    let bytes = files.exact(&request.logical_path)?;
                    let material = if request.role
                        == studio::PresentationDependencyRole::MaterialCandidate
                        && bytes.is_some()
                    {
                        Some(material_manifest(&request.logical_path, files)?)
                    } else {
                        None
                    };
                    let sha256 = bytes.as_deref().map(studio::content_sha256);
                    responses.push(studio::PresentationDependencyResponse {
                        requester: request.requester,
                        role: request.role,
                        logical_path: request.logical_path,
                        material_slot: request.material_slot,
                        texture_role: request.texture_role,
                        bytes,
                        sha256,
                        material,
                    });
                }
            }
        }
    }
}

fn material_manifest(
    identity: &str,
    files: &impl ExactFiles,
) -> Result<studio::MaterialResolutionManifest, String> {
    let identity = identity.to_ascii_lowercase();
    let (material, include_sources) = resolved_material(&identity, files)?;
    let textures = material
        .textures
        .iter()
        .filter_map(|texture| {
            Some(studio::MaterialTextureManifest {
                role: studio_texture_role(texture.role)?,
                parameter: texture.parameter.clone(),
                logical_path: texture
                    .logical_path
                    .as_ref()
                    .map(|path| path.to_ascii_lowercase()),
                disposition: match texture.disposition {
                    playsrc_material::TextureDisposition::Source => {
                        studio::TextureDisposition::Source
                    }
                    playsrc_material::TextureDisposition::BuiltInEnvironment => {
                        studio::TextureDisposition::BuiltInEnvironment
                    }
                    playsrc_material::TextureDisposition::BuiltInRenderTarget => {
                        studio::TextureDisposition::BuiltInRenderTarget
                    }
                },
                selected: material.selected_textures.contains(&texture.role),
            })
        })
        .collect();
    Ok(studio::MaterialResolutionManifest {
        root_identity: identity,
        include_sources,
        textures,
    })
}

fn resolved_material(
    identity: &str,
    files: &impl ExactFiles,
) -> Result<
    (
        playsrc_material::Material,
        Vec<studio::MaterialSourceManifest>,
    ),
    String,
> {
    let root = files
        .exact(identity)?
        .ok_or_else(|| format!("missing {identity}"))?;
    let mut responses = Vec::new();
    let mut include_sources = Vec::new();
    let material = loop {
        match playsrc_vmt::compose(
            &root,
            identity.to_owned(),
            &responses,
            &playsrc_keyvalues::ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|error| error.to_string())?
        {
            playsrc_vmt::Composition::Complete(document) => {
                break playsrc_material::resolve_for_environment(
                    &document,
                    playsrc_material::SelectionEnvironment {
                        model: true,
                        ..Default::default()
                    },
                )
                .map_err(|error| error.to_string())?;
            }
            playsrc_vmt::Composition::Needs(requests) => {
                for request in requests {
                    let path = dependency_path(&request.target_token)?;
                    include_sources.push(studio::MaterialSourceManifest {
                        requester: request.parent_identity.clone(),
                        logical_path: path.clone(),
                    });
                    responses.push(playsrc_vmt::DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: path.clone(),
                        bytes: Some(
                            files
                                .exact(&path)?
                                .ok_or_else(|| format!("missing {path}"))?,
                        ),
                    });
                }
            }
        }
    };
    Ok((material, include_sources))
}

fn studio_texture_role(role: playsrc_material::TextureRole) -> Option<studio::TextureRole> {
    use playsrc_material::TextureRole as Source;
    use studio::TextureRole as Target;
    Some(match role {
        Source::Base => Target::Base,
        Source::HdrBase => Target::HdrBase,
        Source::HdrCompressed => Target::HdrCompressed,
        Source::HdrCompressed0 => Target::HdrCompressed0,
        Source::HdrCompressed1 => Target::HdrCompressed1,
        Source::HdrCompressed2 => Target::HdrCompressed2,
        Source::Base2 => Target::Base2,
        Source::Bump => Target::Bump,
        Source::Normal => Target::Normal,
        Source::Bump2 => Target::Bump2,
        Source::Detail => Target::Detail,
        Source::BlendModulate => Target::BlendModulate,
        Source::Environment => Target::Environment,
        Source::EnvironmentMask => Target::EnvironmentMask,
        Source::SelfIllumMask => Target::SelfIllumMask,
        Source::Flow => Target::Flow,
        Source::Reflection | Source::Refraction => return None,
    })
}

fn dependency_path(token: &[u8]) -> Result<String, String> {
    let value = std::str::from_utf8(token)
        .map_err(|_| "material dependency is not UTF-8")?
        .replace('\\', "/");
    let mut path = if value.to_ascii_lowercase().starts_with("materials/") {
        value
    } else {
        format!("materials/{value}")
    };
    if !path.to_ascii_lowercase().ends_with(".vmt") {
        path.push_str(".vmt");
    }
    Ok(path.to_ascii_lowercase())
}

fn load(path: &str, files: &impl ExactFiles) -> Result<studio::Document, String> {
    let mdl = files
        .exact(path)?
        .ok_or_else(|| format!("missing root {path}"))?;
    let profile = match i32::from_le_bytes(
        mdl.get(4..8)
            .ok_or_else(|| format!("truncated {path}"))?
            .try_into()
            .map_err(|_| format!("malformed {path}"))?,
    ) {
        44 => studio::Profile::SourcePcMdl44,
        45 => studio::Profile::SourcePcMdl45,
        46 => studio::Profile::SourcePcMdl46,
        47 => studio::Profile::SourcePcMdl47,
        48 => studio::Profile::SourcePcMdl48,
        version => return Err(format!("unsupported MDL version {version}")),
    };
    let mut responses = Vec::new();
    loop {
        match studio::load(
            path,
            profile,
            studio::VtxVariant::Dx90,
            &mdl,
            &responses,
            studio::Limits::default(),
        )
        .map_err(|error| error.to_string())?
        {
            studio::Load::Complete(document) => return Ok(*document),
            studio::Load::Needs(requests) => {
                for request in requests {
                    let bytes = files.exact(&request.logical_path)?;
                    if bytes.is_none() && request.role != studio::DependencyRole::Physics {
                        return Err(format!("missing {}", request.logical_path));
                    }
                    responses.push(studio::DependencyResponse {
                        requester: request.requester,
                        role: request.role,
                        logical_path: request.logical_path,
                        bytes,
                    });
                }
            }
        }
    }
}

fn verify_occurrences(cache: &Path) -> Result<(), String> {
    let bsp_bytes = fs::read(
        cache
            .join("objects/sha256")
            .join(&BSP_SHA256[..2])
            .join(BSP_SHA256),
    )
    .map_err(|error| error.to_string())?;
    if hex(&studio::content_sha256(&bsp_bytes)) != BSP_SHA256 {
        return Err("configured BSP identity changed".to_owned());
    }
    let bsp = playsrc_bsp::parse(
        &bsp_bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let graph = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
        .map_err(|error| error.to_string())?;
    let targets = TARGETS
        .iter()
        .map(|target| target.path.as_bytes())
        .collect::<BTreeSet<_>>();
    let mut count = 0_usize;
    let mut bytes = Vec::new();
    for entity in &graph.entities {
        if !entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
        {
            continue;
        }
        let Some(model) = entity.model.as_deref() else {
            continue;
        };
        if !targets.contains(model) {
            continue;
        }
        let position = entity_vector(entity, b"origin")?;
        let angles = entity_vector(entity, b"angles")?;
        let matrix = studio::source_entity_transform(vector(position), vector(angles))
            .map_err(|error| error.to_string())?;
        count += 1;
        bytes.extend_from_slice(&(entity.index as u32).to_le_bytes());
        bytes.extend_from_slice(&(model.len() as u32).to_le_bytes());
        bytes.extend_from_slice(model);
        for value in matrix.0 {
            bytes.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    let digest = hex(&studio::content_sha256(&bytes));
    println!("occurrences={count} transformSha256={digest}");
    if count != 33 || digest != OCCURRENCE_TRANSFORM_SHA256 {
        return Err(format!(
            "target occurrence matrix changed: count={count} digest={digest}"
        ));
    }
    Ok(())
}

fn entity_vector(entity: &playsrc_entity::Entity, key: &[u8]) -> Result<[f32; 3], String> {
    let bytes = entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
        .ok_or_else(|| format!("entity {} lacks vector", entity.index))?;
    let text = std::str::from_utf8(bytes).map_err(|_| "entity vector is not UTF-8")?;
    let values = text
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    values
        .try_into()
        .map_err(|_| "entity vector must have three values".to_owned())
}

fn parse_bundle(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if bytes.get(..4) != Some(b"PSDB") {
        return Err("source bundle signature changed".to_owned());
    }
    let mut offset = 4;
    if u32_field(bytes, &mut offset)? != 1 {
        return Err("source bundle version changed".to_owned());
    }
    let count = u32_field(bytes, &mut offset)? as usize;
    let mut result = BTreeMap::new();
    for _ in 0..count {
        let path = field(bytes, &mut offset)?;
        let path = std::str::from_utf8(path)
            .map_err(|_| "source bundle path is not UTF-8")?
            .to_owned();
        let data = field(bytes, &mut offset)?.to_vec();
        if result.insert(path, data).is_some() {
            return Err("source bundle contains a duplicate".to_owned());
        }
    }
    if offset != bytes.len() {
        return Err("source bundle has trailing bytes".to_owned());
    }
    Ok(result)
}

fn field<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a [u8], String> {
    let length = u32_field(bytes, offset)? as usize;
    let end = offset
        .checked_add(length)
        .ok_or_else(|| "source bundle field overflow".to_owned())?;
    let value = bytes
        .get(*offset..end)
        .ok_or_else(|| "source bundle field is truncated".to_owned())?;
    *offset = end;
    Ok(value)
}

fn u32_field(bytes: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "source bundle u32 overflow".to_owned())?;
    let value = u32::from_le_bytes(
        bytes
            .get(*offset..end)
            .ok_or_else(|| "source bundle u32 is truncated".to_owned())?
            .try_into()
            .map_err(|_| "source bundle u32 is malformed")?,
    );
    *offset = end;
    Ok(value)
}

fn root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../..")
        .canonicalize()
        .map_err(|error| error.to_string())
}

fn vector(values: [f32; 3]) -> studio::Vector3 {
    studio::Vector3(values.map(|value| studio::Float32(value.to_bits())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
