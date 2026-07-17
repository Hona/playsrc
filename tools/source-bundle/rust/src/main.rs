use playsrc_content::{
    CheckedLocation, Content, Provenance, ProviderKind, ProviderSpec, Resolution,
};
use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::{HdrMode, SelectionEnvironment, TextureDisposition};
use playsrc_vmt::{Composition, DependencyResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

const CONTENT_BUILD: &str = "24207079";
const PATCH_VERSION: &str = "10822003";
const GAMEINFO_SHA256: &str = "a85196fdeebeb4e2bae9d412862794d18a4970d118ea0a0d84817c44b8c982da";
const SOURCE_MEDIA_TYPE: &str = "application/octet-stream";
const BUNDLE_MEDIA_TYPE: &str = "application/octet-stream";
const LEDGER_MEDIA_TYPE: &str = "application/vnd.playsrc.source-dependency-ledger+json";
const MAX_GAME_PROVIDERS: usize = 64;
const MAX_DEPENDENCY_REQUESTS: usize = 4_096;
const MAX_BUNDLE_BYTES: usize = 512 * 1024 * 1024;
const MAX_LEDGER_BYTES: usize = 8 * 1024 * 1024;

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
    logical_path: String,
    download: Download,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Download {
    url: String,
    encoded_byte_length: usize,
    encoded_sha256: String,
    decoded_byte_length: usize,
    decoded_sha256: String,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectDescriptor {
    kind: &'static str,
    media_type: &'static str,
    byte_length: String,
    sha256: String,
}

impl ObjectDescriptor {
    fn source(bytes: &[u8]) -> Self {
        Self::new("source-object", SOURCE_MEDIA_TYPE, bytes)
    }

    fn new(kind: &'static str, media_type: &'static str, bytes: &[u8]) -> Self {
        Self {
            kind,
            media_type,
            byte_length: bytes.len().to_string(),
            sha256: digest(bytes),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EncodedMapSource {
    url: String,
    byte_length: String,
    sha256: String,
    compression: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MapSourceRecord {
    logical_path: String,
    encoded: EncodedMapSource,
    decoded: ObjectDescriptor,
    cache_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DepotRecord {
    depot: &'static str,
    manifest: &'static str,
    byte_length: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRecord {
    order: usize,
    identity: String,
    kind: &'static str,
    revision: String,
    configured_location: String,
    path_ids: Vec<String>,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckedRecord {
    provider_identity: String,
    provider_kind: &'static str,
    location: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProvenanceRecord {
    provider_identity: String,
    provider_kind: &'static str,
    provider_revision: String,
    location: String,
}

#[derive(Clone)]
struct MutableRequestRecord {
    requirement: &'static str,
    consumers: BTreeSet<String>,
    descriptor: Option<ObjectDescriptor>,
    provenance: Option<ProvenanceRecord>,
    checked: Option<Vec<CheckedRecord>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestRecord {
    logical_path: String,
    requirement: &'static str,
    consumers: Vec<String>,
    outcome: &'static str,
    descriptor: Option<ObjectDescriptor>,
    provenance: Option<ProvenanceRecord>,
    checked: Vec<CheckedRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyLedger {
    schema: &'static str,
    game: &'static str,
    app_id: &'static str,
    content_build: &'static str,
    patch_version: &'static str,
    installed_depots: Vec<DepotRecord>,
    target: String,
    gameinfo_sha256: &'static str,
    map: MapSourceRecord,
    providers: Vec<ProviderRecord>,
    requests: Vec<RequestRecord>,
    resolved_entries: usize,
    authoritative_absences: usize,
    bundle: ObjectDescriptor,
}

struct Resolver<'a> {
    content: &'a Content,
    bundle: BTreeMap<String, Vec<u8>>,
    requests: BTreeMap<String, MutableRequestRecord>,
}

impl<'a> Resolver<'a> {
    fn new(content: &'a Content) -> Self {
        Self {
            content,
            bundle: BTreeMap::new(),
            requests: BTreeMap::new(),
        }
    }

    fn required(&mut self, path: &str, consumer: impl Into<String>) -> Result<Vec<u8>, String> {
        self.resolve(path, consumer.into(), true)?
            .ok_or_else(|| format!("required dependency {path} has authoritative absence"))
    }

    fn optional(
        &mut self,
        path: &str,
        consumer: impl Into<String>,
    ) -> Result<Option<Vec<u8>>, String> {
        self.resolve(path, consumer.into(), false)
    }

    fn resolve(
        &mut self,
        path: &str,
        consumer: String,
        required: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        let canonical = path.to_ascii_lowercase();
        let result = self
            .content
            .resolve_resource(&canonical)
            .map_err(|error| error.to_string())?;
        match result {
            Resolution::Found(value) => {
                let descriptor = ObjectDescriptor::source(&value.bytes);
                let provenance = provenance_record(&value.provenance);
                let record = self.requests.entry(canonical.clone()).or_insert_with(|| {
                    MutableRequestRecord {
                        requirement: if required { "required" } else { "optional" },
                        consumers: BTreeSet::new(),
                        descriptor: Some(descriptor.clone()),
                        provenance: Some(provenance.clone()),
                        checked: None,
                    }
                });
                if required {
                    record.requirement = "required";
                }
                if record
                    .descriptor
                    .as_ref()
                    .map(|value| (&value.sha256, &value.byte_length))
                    != Some((&descriptor.sha256, &descriptor.byte_length))
                    || record.provenance.as_ref().map(|value| {
                        (
                            &value.provider_identity,
                            &value.provider_revision,
                            &value.location,
                        )
                    }) != Some((
                        &provenance.provider_identity,
                        &provenance.provider_revision,
                        &provenance.location,
                    ))
                    || record.checked.is_some()
                {
                    return Err(format!("dependency {canonical} changed during closure"));
                }
                record.consumers.insert(consumer);
                if let Some(prior) = self.bundle.insert(canonical, value.bytes.clone())
                    && prior != value.bytes
                {
                    return Err("dependency bytes changed during bundle assembly".to_owned());
                }
                Ok(Some(value.bytes))
            }
            Resolution::Missing { checked, .. } => {
                let checked = checked.iter().map(checked_record).collect::<Vec<_>>();
                let record = self.requests.entry(canonical.clone()).or_insert_with(|| {
                    MutableRequestRecord {
                        requirement: if required { "required" } else { "optional" },
                        consumers: BTreeSet::new(),
                        descriptor: None,
                        provenance: None,
                        checked: Some(checked.clone()),
                    }
                });
                if record.descriptor.is_some()
                    || record.provenance.is_some()
                    || record.checked.as_ref() != Some(&checked)
                {
                    return Err(format!("dependency {canonical} changed during closure"));
                }
                record.consumers.insert(consumer);
                if required {
                    record.requirement = "required";
                    let locations = checked
                        .iter()
                        .map(|value| format!("{}:{}", value.provider_identity, value.location))
                        .collect::<Vec<_>>()
                        .join(",");
                    return Err(format!(
                        "missing required dependency {canonical} after [{}]",
                        locations
                    ));
                }
                Ok(None)
            }
        }
    }

    fn records(&self) -> Vec<RequestRecord> {
        self.requests
            .iter()
            .map(|(logical_path, record)| RequestRecord {
                logical_path: logical_path.clone(),
                requirement: record.requirement,
                consumers: record.consumers.iter().cloned().collect(),
                outcome: if record.descriptor.is_some() {
                    "resolved"
                } else {
                    "authoritative-absence"
                },
                descriptor: record.descriptor.clone(),
                provenance: record.provenance.clone(),
                checked: record.checked.clone().unwrap_or_default(),
            })
            .collect()
    }
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

fn provider_kind(value: ProviderKind) -> &'static str {
    match value {
        ProviderKind::BspPak => "bsp-pak",
        ProviderKind::MapSupplement => "map-supplement",
        ProviderKind::Vpk => "vpk",
        ProviderKind::Directory => "directory",
    }
}

fn provenance_record(value: &Provenance) -> ProvenanceRecord {
    ProvenanceRecord {
        provider_identity: value.provider_id.clone(),
        provider_kind: provider_kind(value.provider_kind),
        provider_revision: value.provider_revision.clone(),
        location: value.location.clone(),
    }
}

fn checked_record(value: &CheckedLocation) -> CheckedRecord {
    CheckedRecord {
        provider_identity: value.provider_id.clone(),
        provider_kind: provider_kind(value.provider_kind),
        location: value.location.clone(),
    }
}

fn scalar<'a>(node: &'a playsrc_keyvalues::Node, key: &[u8]) -> Result<&'a [u8], String> {
    let child = node
        .first_child(key)
        .ok_or_else(|| format!("{} is missing", String::from_utf8_lossy(key)))?;
    let playsrc_keyvalues::Value::Scalar(value) = &child.value else {
        return Err(format!("{} is not scalar", String::from_utf8_lossy(key)));
    };
    Ok(&value.token.bytes)
}

fn object_children<'a>(
    node: &'a playsrc_keyvalues::Node,
    key: &[u8],
) -> Result<&'a [playsrc_keyvalues::Node], String> {
    let child = node
        .first_child(key)
        .ok_or_else(|| format!("{} is missing", String::from_utf8_lossy(key)))?;
    let playsrc_keyvalues::Value::Object(value) = &child.value else {
        return Err(format!("{} is not an object", String::from_utf8_lossy(key)));
    };
    Ok(value)
}

fn verify_install_manifest(install: &Path) -> Result<(), String> {
    let bytes = fs::read(install.join("steamapps/appmanifest_440.acf"))
        .map_err(|error| error.to_string())?;
    let document = playsrc_keyvalues::parse_text(
        &bytes,
        playsrc_keyvalues::EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| error.to_string())?
    .evaluated(&ConditionEnvironment::default());
    let app = document
        .roots
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(b"AppState"))
        .ok_or_else(|| "app manifest AppState is missing".to_owned())?;
    if scalar(app, b"appid")? != b"440" || scalar(app, b"buildid")? != CONTENT_BUILD.as_bytes() {
        return Err("configured TF2 app or content build changed".to_owned());
    }
    let depots = object_children(app, b"InstalledDepots")?;
    for (depot, manifest, size) in [
        (
            b"440".as_slice(),
            b"1118032470228587934".as_slice(),
            b"825745".as_slice(),
        ),
        (
            b"441".as_slice(),
            b"1804278129270892792".as_slice(),
            b"32228363932".as_slice(),
        ),
        (
            b"232251".as_slice(),
            b"706600525322138695".as_slice(),
            b"612146219".as_slice(),
        ),
    ] {
        let node = depots
            .iter()
            .find(|node| node.key.bytes == depot)
            .ok_or_else(|| {
                format!(
                    "configured TF2 depot {} is missing",
                    String::from_utf8_lossy(depot)
                )
            })?;
        if scalar(node, b"manifest")? != manifest || scalar(node, b"size")? != size {
            return Err(format!(
                "configured TF2 depot {} identity changed",
                String::from_utf8_lossy(depot)
            ));
        }
    }
    Ok(())
}

fn verify_patch(tf2: &Path) -> Result<(), String> {
    let bytes = fs::read(tf2.join("steam.inf")).map_err(|error| error.to_string())?;
    for key in ["PatchVersion", "ClientVersion", "ServerVersion"] {
        let expected = format!("{key}={PATCH_VERSION}");
        if !bytes
            .split(|byte| *byte == b'\n' || *byte == b'\r')
            .any(|line| line == expected.as_bytes())
        {
            return Err(format!("configured TF2 {key} changed"));
        }
    }
    Ok(())
}

fn configured_location(install: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(install)
        .map_err(|_| "provider escaped the configured TF2 install".to_owned())?;
    Ok(format!(
        "tf2-install/{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

fn provider_id(order: usize, path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("provider")
        .to_ascii_lowercase()
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.') {
                value
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("game-{order:02}-{name}")
}

fn vpk_index_path(declared: &Path) -> PathBuf {
    let value = declared.to_string_lossy();
    if value.to_ascii_lowercase().ends_with("_dir.vpk") {
        return declared.to_path_buf();
    }
    let base = value
        .get(..value.len().saturating_sub(4))
        .unwrap_or(value.as_ref());
    PathBuf::from(format!("{base}_dir.vpk"))
}

fn wildcard_locations(pattern: &Path) -> Result<Vec<PathBuf>, String> {
    if pattern.file_name().and_then(|value| value.to_str()) != Some("*") {
        return Err("configured wildcard search path is unsupported".to_owned());
    }
    let root = pattern
        .parent()
        .ok_or_else(|| "configured wildcard has no parent".to_owned())?;
    let entries = fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut locations = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| "configured wildcard entry is not UTF-8".to_owned())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() || name.to_ascii_lowercase().ends_with(".vpk") {
            locations.push(entry.path());
        }
    }
    if locations.len() > MAX_GAME_PROVIDERS {
        return Err("configured wildcard exceeds provider bound".to_owned());
    }
    locations.sort_by(|left, right| {
        left.to_string_lossy()
            .to_ascii_lowercase()
            .cmp(&right.to_string_lossy().to_ascii_lowercase())
            .then_with(|| left.cmp(right))
    });
    if locations.windows(2).any(|pair| {
        pair[0]
            .to_string_lossy()
            .eq_ignore_ascii_case(&pair[1].to_string_lossy())
    }) {
        return Err("configured wildcard contains an ambiguous ASCII identity".to_owned());
    }
    let directory_families = locations
        .iter()
        .filter_map(|path| {
            let value = path.file_name()?.to_str()?.to_ascii_lowercase();
            value.strip_suffix("_dir.vpk").map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    locations.retain(|path| {
        let Some(value) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let lower = value.to_ascii_lowercase();
        let Some((family, suffix)) = lower.rsplit_once('_') else {
            return true;
        };
        !(suffix.len() == 7
            && suffix[..3].bytes().all(|byte| byte.is_ascii_digit())
            && suffix.ends_with(".vpk")
            && directory_families.contains(family))
    });
    Ok(locations)
}

fn provider_plan(
    install: &Path,
    tf2: &Path,
    gameinfo: &[u8],
) -> Result<(Vec<ProviderSpec>, Vec<ProviderRecord>), String> {
    let document = playsrc_keyvalues::parse_text(
        gameinfo,
        playsrc_keyvalues::EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| error.to_string())?
    .evaluated(&ConditionEnvironment::default());
    let gameinfo = document
        .roots
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(b"GameInfo"))
        .ok_or_else(|| "configured gameinfo root is missing".to_owned())?;
    let filesystem = gameinfo
        .first_child(b"FileSystem")
        .ok_or_else(|| "configured FileSystem is missing".to_owned())?;
    if scalar(filesystem, b"SteamAppId")? != b"440" {
        return Err("configured SteamAppId changed".to_owned());
    }
    let search_paths = object_children(filesystem, b"SearchPaths")?;
    let mut specs = Vec::new();
    let mut records = Vec::new();
    for search_path in search_paths {
        let playsrc_keyvalues::Value::Scalar(location) = &search_path.value else {
            return Err("configured search path is not scalar".to_owned());
        };
        let path_ids = String::from_utf8(search_path.key.bytes.clone())
            .map_err(|_| "configured path IDs are not UTF-8".to_owned())?
            .split('+')
            .map(|value| value.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        if !path_ids.iter().any(|value| value == "game")
            || path_ids.iter().any(|value| value == "game_lv")
        {
            continue;
        }
        let declared = std::str::from_utf8(&location.token.bytes)
            .map_err(|_| "configured search path is not UTF-8".to_owned())?
            .replace('\\', "/");
        let resolved = if let Some(suffix) = declared.strip_prefix("|gameinfo_path|") {
            tf2.join(suffix)
        } else if let Some(suffix) = declared.strip_prefix("|all_source_engine_paths|") {
            install.join(suffix)
        } else {
            install.join(&declared)
        };
        let locations = if declared.contains('*') || declared.contains('?') {
            wildcard_locations(&resolved)?
        } else {
            vec![resolved]
        };
        for declared_path in locations {
            if specs.len() >= MAX_GAME_PROVIDERS {
                return Err("configured provider count exceeds bound".to_owned());
            }
            let lower = declared_path.to_string_lossy().to_ascii_lowercase();
            let (path, kind, layout) = if lower.ends_with(".vpk") {
                let path = if declared.contains('*') || declared.contains('?') {
                    declared_path.clone()
                } else {
                    vpk_index_path(&declared_path)
                };
                let layout = if path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.to_ascii_lowercase().ends_with("_dir.vpk"))
                {
                    playsrc_vpk::Layout::Split
                } else {
                    playsrc_vpk::Layout::Standalone
                };
                (path, "vpk", Some(layout))
            } else {
                (declared_path, "directory", None)
            };
            let order = specs.len();
            let id = provider_id(order, &path);
            let revision = if kind == "vpk" {
                digest(&fs::read(&path).map_err(|error| error.to_string())?)
            } else {
                format!("{CONTENT_BUILD}-{GAMEINFO_SHA256}-{order:02}")
            };
            records.push(ProviderRecord {
                order,
                identity: id.clone(),
                kind,
                revision: revision.clone(),
                configured_location: configured_location(install, &path)?,
                path_ids: path_ids.clone(),
            });
            specs.push(if let Some(layout) = layout {
                ProviderSpec::Vpk {
                    id,
                    revision,
                    directory_file: path,
                    layout,
                }
            } else {
                ProviderSpec::Directory {
                    id,
                    revision,
                    root: path,
                }
            });
        }
    }
    Ok((specs, records))
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
    resolver: &mut Resolver<'_>,
    root_path: &str,
    include_textures: bool,
    environment: SelectionEnvironment,
    selected_only: bool,
    consumer: &str,
) -> Result<(), String> {
    let identity = root_path.to_ascii_lowercase();
    let root_bytes = resolver.required(&identity, format!("{consumer}:material"))?;
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
                    let bytes = resolver.required(
                        &path,
                        format!("{consumer}:material-compose:{}", request.parent_identity),
                    )?;
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
        resolver.required(&path, format!("{consumer}:selected-texture"))?;
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
    resolver: &mut Resolver<'_>,
    root_path: &str,
) -> Result<Box<playsrc_studio_model::Document>, String> {
    let identity = root_path.to_ascii_lowercase();
    let mdl = resolver.required(&identity, format!("studio-model:{identity}:root"))?;
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
                    let role = match request.role {
                        playsrc_studio_model::DependencyRole::VertexData => "vertex-data",
                        playsrc_studio_model::DependencyRole::Topology => "topology",
                        playsrc_studio_model::DependencyRole::AnimationBlocks => "animation-blocks",
                        playsrc_studio_model::DependencyRole::IncludeModel => "include-model",
                        playsrc_studio_model::DependencyRole::Physics => "physics",
                    };
                    let consumer = format!(
                        "studio-model:{}:{role}:{}",
                        request.requester.to_ascii_lowercase(),
                        identity
                    );
                    let bytes = if request.role == playsrc_studio_model::DependencyRole::Physics {
                        resolver.optional(&path, consumer)?
                    } else {
                        Some(resolver.required(&path, consumer)?)
                    };
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

fn bytesv(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(bytes.len())
            .map_err(|_| "source dependency field exceeds u32".to_owned())?
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
    (output.len() <= MAX_BUNDLE_BYTES)
        .then_some(())
        .ok_or_else(|| "source dependency bundle exceeds byte bound".to_owned())
}

fn install_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("artifact"),
        std::process::id()
    ));
    let result = (|| {
        fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        if fs::read(&temporary).map_err(|error| error.to_string())? != bytes {
            return Err("temporary artifact verification failed".to_owned());
        }
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        if fs::read(path).map_err(|error| error.to_string())? != bytes {
            return Err("installed artifact verification failed".to_owned());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildReport {
    target: String,
    content_build: &'static str,
    providers: usize,
    requests: usize,
    authoritative_absences: usize,
    entries: usize,
    bytes: usize,
    sha256: String,
    bundle_descriptor: ObjectDescriptor,
    ledger_bytes: usize,
    ledger_sha256: String,
    ledger_descriptor: ObjectDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    native_hdr_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    native_hdr_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    native_hdr_derived_sha256: Option<String>,
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
    let map_target = maps
        .get(&target)
        .ok_or_else(|| "target is not declared".to_owned())?;
    if map_target.logical_path != format!("maps/{target}.bsp")
        || map_target.download.decoded_byte_length == 0
        || map_target.download.encoded_byte_length == 0
        || map_target.download.encoded_sha256.len() != 64
        || map_target.download.decoded_sha256.len() != 64
    {
        return Err("declared map source is malformed".to_owned());
    }
    let cache = PathBuf::from(&config.source_cache_dir);
    let bsp_bytes = fs::read(object_path(&cache, &map_target.download.decoded_sha256))
        .map_err(|error| error.to_string())?;
    if bsp_bytes.len() != map_target.download.decoded_byte_length
        || digest(&bsp_bytes) != map_target.download.decoded_sha256
    {
        return Err("cached BSP differs from its declared identity".to_owned());
    }
    let bsp = playsrc_bsp::parse(
        &bsp_bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let tf2 = PathBuf::from(&config.tf2_dir);
    let gameinfo = fs::read(tf2.join("gameinfo.txt")).map_err(|error| error.to_string())?;
    if digest(&gameinfo) != GAMEINFO_SHA256 {
        return Err("configured TF2 gameinfo identity changed".to_owned());
    }
    let install = tf2
        .parent()
        .ok_or_else(|| "tf2Dir has no install parent".to_owned())?;
    verify_install_manifest(install)?;
    verify_patch(&tf2)?;
    let (providers, mut provider_records) = provider_plan(install, &tf2, &gameinfo)?;
    let content = Content::open(
        "tf2",
        CONTENT_BUILD,
        providers,
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
            map_target.download.decoded_sha256.clone(),
            map_target.logical_path.clone(),
            pak,
        )
        .map_err(|error| error.to_string())?;
    for provider in &mut provider_records {
        provider.order += 1;
    }
    provider_records.insert(
        0,
        ProviderRecord {
            order: 0,
            identity: "jump-beef-pak".to_owned(),
            kind: "bsp-pak",
            revision: map_target.download.decoded_sha256.clone(),
            configured_location: format!("source-cache/{}", map_target.logical_path),
            path_ids: vec!["game".to_owned()],
        },
    );
    let canonical = playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Ldr)
        .map_err(|error| error.to_string())?;
    let mut resolver = Resolver::new(&content);
    for material in &canonical.materials {
        collect_material(
            &mut resolver,
            &material.logical_path,
            true,
            SelectionEnvironment::default(),
            false,
            "world-material",
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
                &mut resolver,
                &format!("materials/skybox/{sky}{profile_suffix}{suffix}.vmt"),
                true,
                environment,
                true,
                if profile_suffix.is_empty() {
                    "sky-ldr"
                } else {
                    "sky-hdr"
                },
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
            resolver.required(
                &path,
                if suffix.is_empty() {
                    "cubemap-ldr"
                } else {
                    "cubemap-hdr"
                },
            )?;
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
        if resolver.optional(&path, "decal-material-probe")?.is_some() {
            collect_material(
                &mut resolver,
                &path,
                true,
                SelectionEnvironment::default(),
                true,
                "decal",
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
        let document = collect_model(&mut resolver, &path)?;
        for material in &document.materials {
            let mut found = None;
            for candidate in &material.candidates {
                if resolver
                    .optional(candidate, format!("studio-model:{path}:material-candidate"))?
                    .is_some()
                {
                    found = Some(candidate.clone());
                    break;
                }
            }
            if let Some(candidate) = found {
                collect_material(
                    &mut resolver,
                    &candidate,
                    true,
                    SelectionEnvironment::default(),
                    false,
                    &format!("studio-model:{path}:material"),
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
        .map(|path| {
            resolver
                .required(path, format!("particle-registry:{path}"))
                .map(|bytes| (*path, bytes))
        })
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
    for material in closure.materials {
        let path = material_path(material.as_bytes())?;
        collect_material(
            &mut resolver,
            &path,
            true,
            SelectionEnvironment::default(),
            true,
            "particle-material",
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
        let consumer = if path.starts_with("sound/") {
            "audio-wave"
        } else {
            "audio-script"
        };
        resolver.required(path, consumer)?;
    }
    let bundle = &resolver.bundle;
    if bundle.len() > MAX_DEPENDENCY_REQUESTS || resolver.requests.len() > MAX_DEPENDENCY_REQUESTS {
        return Err("source dependency request count exceeds bound".to_owned());
    }
    let mut output = b"PSDB".to_vec();
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(
        &u32::try_from(bundle.len())
            .map_err(|_| "source dependency entry count exceeds u32".to_owned())?
            .to_le_bytes(),
    );
    for (path, bytes) in bundle {
        bytesv(&mut output, path.as_bytes())?;
        bytesv(&mut output, bytes)?;
    }
    let request_records = resolver.records();
    let authoritative_absences = request_records
        .iter()
        .filter(|record| record.outcome == "authoritative-absence")
        .count();
    let resolved_entries = request_records
        .iter()
        .filter(|record| record.outcome == "resolved")
        .count();
    if resolved_entries != bundle.len() {
        return Err("resolved ledger identities differ from bundle entries".to_owned());
    }
    for request in request_records
        .iter()
        .filter(|record| record.outcome == "resolved")
    {
        let bytes = bundle
            .get(&request.logical_path)
            .ok_or_else(|| "resolved ledger entry is absent from bundle".to_owned())?;
        let descriptor = request
            .descriptor
            .as_ref()
            .ok_or_else(|| "resolved ledger descriptor is absent".to_owned())?;
        if descriptor.byte_length != bytes.len().to_string() || descriptor.sha256 != digest(bytes) {
            return Err("resolved ledger descriptor differs from bundle bytes".to_owned());
        }
    }
    let bundle_descriptor = ObjectDescriptor::new("derived-object", BUNDLE_MEDIA_TYPE, &output);
    let ledger = DependencyLedger {
        schema: "playsrc-source-dependency-ledger-v1",
        game: "tf2",
        app_id: "440",
        content_build: CONTENT_BUILD,
        patch_version: PATCH_VERSION,
        installed_depots: vec![
            DepotRecord {
                depot: "440",
                manifest: "1118032470228587934",
                byte_length: "825745",
            },
            DepotRecord {
                depot: "441",
                manifest: "1804278129270892792",
                byte_length: "32228363932",
            },
            DepotRecord {
                depot: "232251",
                manifest: "706600525322138695",
                byte_length: "612146219",
            },
        ],
        target: target.clone(),
        gameinfo_sha256: GAMEINFO_SHA256,
        map: MapSourceRecord {
            logical_path: map_target.logical_path.clone(),
            encoded: EncodedMapSource {
                url: map_target.download.url.clone(),
                byte_length: map_target.download.encoded_byte_length.to_string(),
                sha256: map_target.download.encoded_sha256.clone(),
                compression: "bzip2",
            },
            decoded: ObjectDescriptor {
                kind: "source-object",
                media_type: SOURCE_MEDIA_TYPE,
                byte_length: bsp_bytes.len().to_string(),
                sha256: map_target.download.decoded_sha256.clone(),
            },
            cache_path: format!(
                "objects/sha256/{}/{}",
                &map_target.download.decoded_sha256[..2],
                map_target.download.decoded_sha256
            ),
        },
        providers: provider_records,
        requests: request_records,
        resolved_entries,
        authoritative_absences,
        bundle: bundle_descriptor.clone(),
    };
    let ledger_bytes = serde_json::to_vec(&ledger).map_err(|error| error.to_string())?;
    if ledger_bytes.len() > MAX_LEDGER_BYTES {
        return Err("source dependency ledger exceeds byte bound".to_owned());
    }
    let ledger_descriptor =
        ObjectDescriptor::new("derived-object", LEDGER_MEDIA_TYPE, &ledger_bytes);
    let directory = cache.join("browser-bundles");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let destination = directory.join(format!("{target}.psdb"));
    let ledger_destination = directory.join(format!("{target}.dependencies.json"));
    install_artifact(&destination, &output)?;
    install_artifact(&ledger_destination, &ledger_bytes)?;
    let mut report = BuildReport {
        target: target.clone(),
        content_build: CONTENT_BUILD,
        providers: ledger.providers.len(),
        requests: ledger.resolved_entries + ledger.authoritative_absences,
        authoritative_absences: ledger.authoritative_absences,
        entries: bundle.len(),
        bytes: output.len(),
        sha256: digest(&output),
        bundle_descriptor,
        ledger_bytes: ledger_bytes.len(),
        ledger_sha256: digest(&ledger_bytes),
        ledger_descriptor,
        native_hdr_bytes: None,
        native_hdr_sha256: None,
        native_hdr_derived_sha256: None,
    };
    if verify_hdr {
        let artifact = playsrc_tf2_wasm::compile_artifact(&bsp_bytes, 1, &output)
            .map_err(|error| format!("native HDR compilation failed with error {error}"))?;
        let native_destination = directory.join(format!("{target}.native-hdr.psmp"));
        install_artifact(&native_destination, &artifact.payload)?;
        report.native_hdr_bytes = Some(artifact.payload.len());
        report.native_hdr_sha256 = Some(digest(&artifact.payload));
        report.native_hdr_derived_sha256 = Some(hex(&artifact.derived_sha256));
    }
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
