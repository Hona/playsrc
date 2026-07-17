use playsrc_content::{Content, ProviderSpec, Resolution};
use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::TextureDisposition;
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
                break playsrc_material::resolve(&document).map_err(|error| error.to_string())?;
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
    for texture in material.textures {
        if texture.disposition != TextureDisposition::Source {
            continue;
        }
        let path = texture
            .logical_path
            .ok_or_else(|| "source texture has no logical path".to_owned())?
            .to_ascii_lowercase();
        if !bundle.contains_key(&path) {
            bundle.insert(path.clone(), resolved(content, &path)?);
        }
    }
    Ok(())
}

fn bytesv(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn main() -> Result<(), String> {
    let target = env::args()
        .nth(1)
        .ok_or_else(|| "target is required".to_owned())?;
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
            provider(&install.join("hl2"), "hl2_misc_dir.vpk", "hl2-misc")?,
            provider(&install.join("hl2"), "hl2_textures_dir.vpk", "hl2-textures")?,
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
        collect_material(&content, &material.logical_path, &mut bundle)?;
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
    println!(
        "{{\"target\":\"{}\",\"entries\":{},\"bytes\":{},\"sha256\":\"{}\"}}",
        target,
        bundle.len(),
        output.len(),
        digest(&output)
    );
    Ok(())
}
