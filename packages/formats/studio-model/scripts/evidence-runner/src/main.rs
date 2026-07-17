use playsrc_studio_model as studio;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const BUILD: &str = "24207079";
const BUNDLE_SHA256: &str = "34cbd09a63f1ba8407c7a775de20467773f87a41db78e34447734799fa2dba78";
const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const OCCURRENCE_TRANSFORM_SHA256: &str =
    "7a4eff4a2d9ca0892b6f576d21df4d44d03e03f957499c20245740b21b4edee6";

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
const VIEWMODEL_ACTIVITIES: &[&str] = &[
    "ACT_VM_DRAW",
    "ACT_VM_IDLE",
    "ACT_VM_PRIMARYATTACK",
    "ACT_RELOAD_START",
    "ACT_VM_RELOAD",
    "ACT_RELOAD_FINISH",
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
    Target {
        path: "models/weapons/v_models/v_rocketlauncher_soldier.mdl",
        mdl_sha256: "0eca831c2733188494763419c0e3ca6971fdc2715519f1a1e248d832e1509738",
        bones: 29,
        animations: 7,
        sequences: 7,
        materials: 6,
        skins: 10,
        body_parts: 1,
        attachments: 0,
        primitives: 4,
        lods: &[0],
        physics: studio::PhysicsStatus::Missing,
        first_material: "materials/models/player/soldier/soldier_sleeves_red.vmt",
        artifact_bytes: 547_213,
        artifact_sha256: "a29079d338c25eea18a2e947d1c58dc4b193938ccf07e672a5961590a48be6eb",
        timeline_sha256: "d86487b69a77adbe0d454192b092fb8ea6c36ae2eb4475ba01e4f28ea2e34882",
        activities: VIEWMODEL_ACTIVITIES,
    },
    Target {
        path: "models/weapons/v_models/v_stickybomb_launcher_demo.mdl",
        mdl_sha256: "1cbb38e6908762b0255437604ca4f7266ac9958b4a334456b665a6d9a9f15a84",
        bones: 51,
        animations: 8,
        sequences: 8,
        materials: 5,
        skins: 10,
        body_parts: 1,
        attachments: 2,
        primitives: 3,
        lods: &[0],
        physics: studio::PhysicsStatus::Missing,
        first_material: "materials/models/player/demo/demoman_hands.vmt",
        artifact_bytes: 929_616,
        artifact_sha256: "1d13b532c529270a2b0eabf0b87b284b49d4fc6dcc45dd18b5d061bd9c47f2d0",
        timeline_sha256: "e09068d6d4657ee260ab1eb5c8fbf735ac6527421ba11216cda592146726cb14",
        activities: VIEWMODEL_ACTIVITIES,
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
    for target in TARGETS {
        verify_target(target, &files)?;
    }
    verify_occurrences(&cache)?;
    println!(
        "{{\"build\":\"{BUILD}\",\"bundleSha256\":\"{BUNDLE_SHA256}\",\"models\":{},\"status\":\"Ready\"}}",
        TARGETS.len()
    );
    Ok(())
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
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<studio::PresentationArtifact, String> {
    let profile = if document.identity.contains("/v_models/") {
        studio::PresentationProfile::ViewModel
    } else {
        studio::PresentationProfile::World
    };
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
                    let bytes = files.get(&request.logical_path).cloned();
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
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<studio::MaterialResolutionManifest, String> {
    let identity = identity.to_ascii_lowercase();
    let root = files
        .get(&identity)
        .ok_or_else(|| format!("missing {identity}"))?;
    let mut responses = Vec::new();
    let mut include_sources = Vec::new();
    let material = loop {
        match playsrc_vmt::compose(
            root,
            identity.clone(),
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
                                .get(&path)
                                .ok_or_else(|| format!("missing {path}"))?
                                .clone(),
                        ),
                    });
                }
            }
        }
    };
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

fn load(path: &str, files: &BTreeMap<String, Vec<u8>>) -> Result<studio::Document, String> {
    let mdl = files
        .get(path)
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
            mdl,
            &responses,
            studio::Limits::default(),
        )
        .map_err(|error| error.to_string())?
        {
            studio::Load::Complete(document) => return Ok(*document),
            studio::Load::Needs(requests) => {
                for request in requests {
                    let bytes = files.get(&request.logical_path).cloned();
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
