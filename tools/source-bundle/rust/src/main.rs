use playsrc_content::{Content, ProviderSpec, Resolution};
use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::{HdrMode, SelectionEnvironment, TextureDisposition};
use playsrc_vmt::{Composition, DependencyResponse};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

const GAMEINFO_SHA256: &str = "a85196fdeebeb4e2bae9d412862794d18a4970d118ea0a0d84817c44b8c982da";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalConfigFile {
    #[serde(rename = "tf2Dir")]
    tf2_dir: String,
    #[serde(rename = "sourceCacheDir")]
    source_cache_dir: String,
    #[serde(rename = "assetDir")]
    _asset_dir: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapTarget {
    download: Download,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Download {
    decoded_sha256: String,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn object_path(cache: &Path, hash: &str) -> PathBuf {
    cache.join("objects/sha256").join(&hash[..2]).join(hash)
}

fn resolved(content: &Content, path: &str) -> Result<Vec<u8>, String> {
    match content
        .resolve_resource(path)
        .map_err(|error| error.to_string())?
    {
        Resolution::Found(value) => Ok(value.bytes),
        Resolution::Missing { checked, .. } => Err(format!(
            "missing {path} after {} exact candidates",
            checked.len()
        )),
    }
}

fn optional(content: &Content, path: &str) -> Result<Option<Vec<u8>>, String> {
    match content
        .resolve_resource(path)
        .map_err(|error| error.to_string())?
    {
        Resolution::Found(value) => Ok(Some(value.bytes)),
        Resolution::Missing { .. } => Ok(None),
    }
}

fn material_path(token: &[u8]) -> Result<String, String> {
    let value = std::str::from_utf8(token)
        .map_err(|_| "VMT dependency is not UTF-8")?
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

fn collect_material(
    content: &Content,
    root_path: &str,
    bundle: &mut BTreeMap<String, Vec<u8>>,
    include_textures: bool,
    environment: SelectionEnvironment,
    selected_only: bool,
) -> Result<(), String> {
    let identity = root_path.to_ascii_lowercase();
    let root_bytes = resolved(content, &identity)?;
    bundle.insert(identity.clone(), root_bytes.clone());
    let mut responses = Vec::new();
    let material = loop {
        match playsrc_vmt::compose(
            &root_bytes,
            identity.clone(),
            &responses,
            &ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|error| error.to_string())?
        {
            Composition::Complete(document) => {
                break playsrc_material::resolve_for_environment(&document, environment)
                    .map_err(|error| error.to_string())?;
            }
            Composition::Needs(requests) => {
                for request in requests {
                    let path = material_path(&request.target_token)?;
                    let bytes = resolved(content, &path)?;
                    bundle.insert(path.clone(), bytes.clone());
                    responses.push(DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: path,
                        bytes: Some(bytes),
                    });
                }
            }
        }
    };
    for texture in &material.textures {
        if !include_textures {
            continue;
        }
        if texture.disposition != TextureDisposition::Source {
            continue;
        }
        if selected_only && !material.selected_textures.contains(&texture.role) {
            continue;
        }
        let path = texture
            .logical_path
            .as_ref()
            .ok_or_else(|| "source texture has no logical path".to_owned())?
            .to_ascii_lowercase();
        if !bundle.contains_key(&path) {
            bundle.insert(path.clone(), resolved(content, &path)?);
        }
    }
    Ok(())
}

fn model_profile(bytes: &[u8]) -> Result<playsrc_studio_model::Profile, String> {
    let version = i32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or_else(|| "MDL header is truncated".to_owned())?
            .try_into()
            .map_err(|_| "MDL version is malformed")?,
    );
    match version {
        44 => Ok(playsrc_studio_model::Profile::SourcePcMdl44),
        45 => Ok(playsrc_studio_model::Profile::SourcePcMdl45),
        46 => Ok(playsrc_studio_model::Profile::SourcePcMdl46),
        47 => Ok(playsrc_studio_model::Profile::SourcePcMdl47),
        48 => Ok(playsrc_studio_model::Profile::SourcePcMdl48),
        _ => Err(format!("unsupported MDL version {version}")),
    }
}

fn collect_model(
    content: &Content,
    root_path: &str,
    bundle: &mut BTreeMap<String, Vec<u8>>,
) -> Result<Box<playsrc_studio_model::Document>, String> {
    let identity = root_path.to_ascii_lowercase();
    let mdl = resolved(content, &identity)?;
    bundle.insert(identity.clone(), mdl.clone());
    let profile = model_profile(&mdl)?;
    let mut responses = Vec::new();
    loop {
        match playsrc_studio_model::load(
            identity.clone(),
            profile,
            playsrc_studio_model::VtxVariant::Dx90,
            &mdl,
            &responses,
            playsrc_studio_model::Limits::default(),
        )
        .map_err(|error| error.to_string())?
        {
            playsrc_studio_model::Load::Complete(document) => return Ok(document),
            playsrc_studio_model::Load::Needs(requests) => {
                for request in requests {
                    let path = request.logical_path.to_ascii_lowercase();
                    let bytes = optional(content, &path)?;
                    if request.role != playsrc_studio_model::DependencyRole::Physics
                        && bytes.is_none()
                    {
                        return Err(format!("missing required model dependency {path}"));
                    }
                    if let Some(value) = &bytes {
                        bundle.insert(path.clone(), value.clone());
                    }
                    responses.push(playsrc_studio_model::DependencyResponse {
                        requester: request.requester,
                        role: request.role,
                        logical_path: path,
                        bytes,
                    });
                }
            }
        }
    }
}

fn bytesv(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn main() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let target = arguments
        .next()
        .ok_or_else(|| "target is required".to_owned())?;
    let verify_hdr = match arguments.next().as_deref() {
        None => false,
        Some("--verify-hdr") => true,
        Some(_) => return Err("source bundle mode is malformed".to_owned()),
    };
    if arguments.next().is_some() {
        return Err("source bundle accepts one target and one optional mode".to_owned());
    }
    if !target
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err("target is malformed".to_owned());
    }
    let root = root();
    let config: LocalConfigFile = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let maps: BTreeMap<String, MapTarget> = serde_json::from_slice(
        &fs::read(root.join("games/tf2/maps.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let map = maps
        .get(&target)
        .ok_or_else(|| "target is not declared".to_owned())?;
    let cache = PathBuf::from(&config.source_cache_dir);
    let bsp_bytes = fs::read(object_path(&cache, &map.download.decoded_sha256))
        .map_err(|error| error.to_string())?;
    let bsp = playsrc_bsp::parse(
        &bsp_bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let tf2 = PathBuf::from(&config.tf2_dir);
    let gameinfo = fs::read(tf2.join("gameinfo.txt")).map_err(|error| error.to_string())?;
    if digest(&gameinfo) != GAMEINFO_SHA256
        || !gameinfo
            .windows(b"|all_source_engine_paths|hl2".len())
            .any(|window| window.eq_ignore_ascii_case(b"|all_source_engine_paths|hl2"))
    {
        return Err("configured TF2 gameinfo identity or HL2 mount changed".to_owned());
    }
    let install = tf2
        .parent()
        .ok_or_else(|| "tf2Dir has no install parent".to_owned())?;
    let provider = |base: &Path, name: &str, id: &str| -> Result<ProviderSpec, String> {
        let path = base.join(name);
        let revision = digest(&fs::read(&path).map_err(|error| error.to_string())?);
        Ok(ProviderSpec::Vpk {
            id: id.to_owned(),
            revision,
            directory_file: path,
            layout: playsrc_vpk::Layout::Split,
        })
    };
    let content = Content::open(
        "tf2",
        "24207079",
        vec![
            provider(&tf2, "tf2_misc_dir.vpk", "tf2-misc")?,
            provider(&tf2, "tf2_textures_dir.vpk", "tf2-textures")?,
            provider(&tf2, "tf2_sound_misc_dir.vpk", "tf2-sound-misc")?,
            provider(&install.join("hl2"), "hl2_misc_dir.vpk", "hl2-misc")?,
            provider(&install.join("hl2"), "hl2_textures_dir.vpk", "hl2-textures")?,
            provider(
                &install.join("hl2"),
                "hl2_sound_misc_dir.vpk",
                "hl2-sound-misc",
            )?,
        ],
        playsrc_content::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let pak = bsp.lumps[40]
        .pak
        .as_ref()
        .ok_or_else(|| "BSP PAK is unavailable".to_owned())?;
    let content = content
        .with_active_pak(
            "jump-beef-pak",
            map.download.decoded_sha256.clone(),
            format!("maps/{target}.bsp"),
            pak,
        )
        .map_err(|error| error.to_string())?;
    let map = playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Ldr)
        .map_err(|error| error.to_string())?;
    let mut bundle = BTreeMap::new();
    for material in &map.materials {
        collect_material(
            &content,
            &material.logical_path,
            &mut bundle,
            true,
            SelectionEnvironment::default(),
            false,
        )?;
    }
    let graph = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
        .map_err(|error| error.to_string())?;
    let world = graph
        .entities
        .iter()
        .find(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"worldspawn"))
        })
        .ok_or_else(|| "worldspawn is unavailable".to_owned())?;
    let sky = world
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(b"skyname"))
        .ok_or_else(|| "worldspawn skyname is unavailable".to_owned())?;
    let sky = std::str::from_utf8(&sky.value).map_err(|_| "skyname is not UTF-8")?;
    if sky.is_empty()
        || !sky
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err("skyname is malformed".to_owned());
    }
    for (profile_suffix, environment) in [
        ("", SelectionEnvironment::default()),
        (
            "_hdr",
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        ),
    ] {
        for suffix in ["rt", "lf", "bk", "ft", "up", "dn"] {
            collect_material(
                &content,
                &format!("materials/skybox/{sky}{profile_suffix}{suffix}.vmt"),
                &mut bundle,
                true,
                environment,
                true,
            )?;
        }
    }
    let cubemaps = match &bsp.lumps[42].records {
        playsrc_bsp::LumpData::Cubemaps(records) => records.as_slice(),
        _ => return Err("cubemap records are unavailable".to_owned()),
    };
    for cubemap in cubemaps {
        for suffix in ["", ".hdr"] {
            let path = format!(
                "materials/maps/{target}/c{}_{}_{}{suffix}.vtf",
                cubemap.origin[0], cubemap.origin[1], cubemap.origin[2]
            );
            bundle.insert(path.clone(), resolved(&content, &path)?);
        }
    }
    for entity in &graph.entities {
        if !entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"infodecal"))
        {
            continue;
        }
        let Some(reference) = entity
            .pairs
            .iter()
            .find(|pair| pair.key.eq_ignore_ascii_case(b"texture"))
            .map(|pair| pair.value.as_slice())
        else {
            continue;
        };
        let path = material_path(reference)?;
        if optional(&content, &path)?.is_some() {
            collect_material(
                &content,
                &path,
                &mut bundle,
                true,
                SelectionEnvironment::default(),
                true,
            )?;
        }
    }
    let mut model_paths = graph
        .entities
        .iter()
        .filter(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
        })
        .filter_map(|entity| entity.model.as_ref())
        .map(|value| String::from_utf8(value.clone()).map(|path| path.to_ascii_lowercase()))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .map_err(|_| "model identity is not UTF-8")?;
    for path in [
        "models/weapons/w_models/w_rocket.mdl",
        "models/weapons/w_models/w_stickybomb.mdl",
        "models/weapons/v_models/v_rocketlauncher_soldier.mdl",
        "models/weapons/v_models/v_stickybomb_launcher_demo.mdl",
        "models/player/soldier.mdl",
        "models/player/demo.mdl",
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
    ] {
        model_paths.insert(path.to_owned());
    }
    for path in model_paths {
        let document = collect_model(&content, &path, &mut bundle)?;
        for material in &document.materials {
            let mut found = None;
            for candidate in &material.candidates {
                if optional(&content, candidate)?.is_some() {
                    found = Some(candidate.clone());
                    break;
                }
            }
            if let Some(candidate) = found {
                collect_material(
                    &content,
                    &candidate,
                    &mut bundle,
                    true,
                    SelectionEnvironment::default(),
                    false,
                )?;
            }
        }
    }
    let particle_paths = [
        "particles/rockettrail.pcf",
        "particles/rocketbackblast.pcf",
        "particles/stickybomb.pcf",
        "particles/muzzle_flash.pcf",
        "particles/explosion.pcf",
    ];
    let particle_bytes = particle_paths
        .iter()
        .map(|path| resolved(&content, path).map(|bytes| (*path, bytes)))
        .collect::<Result<Vec<_>, _>>()?;
    let particle_sources = particle_bytes
        .iter()
        .map(|(logical_path, bytes)| playsrc_particle::PcfSource {
            logical_path,
            bytes,
        })
        .collect::<Vec<_>>();
    let registry = playsrc_particle::Registry::from_pcf(
        &particle_sources,
        playsrc_particle::RegistryLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    let roots = [
        "rockettrail",
        "rocketbackblast",
        "stickybombtrail_red",
        "stickybombtrail_blue",
        "stickybomb_pulse_red",
        "stickybomb_pulse_blue",
        "muzzle_pipelauncher",
        "ExplosionCore_Wall",
        "ExplosionCore_MidAir",
    ]
    .map(playsrc_particle::DefinitionLookup::Name);
    let closure = registry
        .target_closure(&roots)
        .map_err(|error| error.to_string())?;
    for (path, bytes) in particle_bytes {
        bundle.insert(path.to_owned(), bytes);
    }
    for material in closure.materials {
        let path = material_path(material.as_bytes())?;
        collect_material(
            &content,
            &path,
            &mut bundle,
            true,
            SelectionEnvironment::default(),
            true,
        )?;
    }
    for path in [
        "scripts/game_sounds_weapons.txt",
        "scripts/soundmixers.txt",
        "sound/weapons/rocket_shoot.wav",
        "sound/weapons/stickybomblauncher_shoot.wav",
        "sound/weapons/explode1.wav",
        "sound/weapons/explode2.wav",
        "sound/weapons/explode3.wav",
        "sound/weapons/pipe_bomb1.wav",
        "sound/weapons/pipe_bomb2.wav",
        "sound/weapons/pipe_bomb3.wav",
    ] {
        bundle.insert(path.to_owned(), resolved(&content, path)?);
    }
    let mut output = b"PSDB".to_vec();
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&(bundle.len() as u32).to_le_bytes());
    for (path, bytes) in &bundle {
        bytesv(&mut output, path.as_bytes());
        bytesv(&mut output, bytes);
    }
    let directory = cache.join("browser-bundles");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let destination = directory.join(format!("{target}.psdb"));
    let temporary = directory.join(format!("{target}.psdb.tmp"));
    fs::write(&temporary, &output).map_err(|error| error.to_string())?;
    fs::rename(&temporary, &destination).map_err(|error| error.to_string())?;
    if verify_hdr {
        let artifact = playsrc_tf2_wasm::compile_artifact(&bsp_bytes, 1, &output)
            .map_err(|error| format!("native HDR compilation failed with error {error}"))?;
        let native_destination = directory.join(format!("{target}.native-hdr.psmp"));
        let native_temporary = directory.join(format!("{target}.native-hdr.psmp.tmp"));
        fs::write(&native_temporary, &artifact.payload).map_err(|error| error.to_string())?;
        fs::rename(&native_temporary, &native_destination).map_err(|error| error.to_string())?;
        println!(
            "{{\"target\":\"{}\",\"entries\":{},\"bytes\":{},\"sha256\":\"{}\",\"nativeHdrBytes\":{},\"nativeHdrSha256\":\"{}\",\"nativeHdrDerivedSha256\":\"{}\"}}",
            target,
            bundle.len(),
            output.len(),
            digest(&output),
            artifact.payload.len(),
            digest(&artifact.payload),
            hex(&artifact.derived_sha256),
        );
    } else {
        println!(
            "{{\"target\":\"{}\",\"entries\":{},\"bytes\":{},\"sha256\":\"{}\"}}",
            target,
            bundle.len(),
            output.len(),
            digest(&output)
        );
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
