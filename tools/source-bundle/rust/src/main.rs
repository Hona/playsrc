use playsrc_content::{
    CheckedLocation, Content, Provenance, ProviderKind, ProviderSpec, Resolution,
};
use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::{
    HdrMode, SelectionEnvironment, TextureColorRead, TextureDisposition, TextureRole,
};
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tf2UiBundleManifest {
    schema: String,
    identity: String,
    source_ledger: String,
    dependencies: Vec<Tf2UiDependency>,
    images: Vec<Tf2UiImage>,
    missing_dependencies: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tf2UiDependency {
    logical_path: String,
    sha256: String,
    byte_length: usize,
    kinds: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tf2UiImage {
    identity: String,
    configured_value: String,
    classification: String,
    material: Option<String>,
    textures: Vec<Tf2UiTexture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tf2UiTexture {
    logical_path: String,
    sha256: String,
    width: u32,
    height: u32,
    frames: u16,
    raw_flags: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2UiMaterialSet {
    schema: &'static str,
    descriptor: String,
    images: Vec<Tf2UiMaterialRecord>,
    textures: Vec<Tf2UiMaterialTextureRecord>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2UiMaterialTextureRecord {
    logical_path: String,
    sha256: String,
    width: u32,
    height: u32,
    frames: u16,
    raw_flags: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2UiMaterialRecord {
    configured_value: String,
    material: String,
    shader: String,
    base_texture: String,
    base_color_read: &'static str,
    second_texture: Option<String>,
    second_color_read: Option<&'static str>,
    detail_texture: Option<String>,
    detail_color_read: Option<&'static str>,
    detail_scale: f32,
    detail_blend_mode: i32,
    detail_blend_factor: f32,
    detail_tint: [f32; 3],
    distance_alpha: bool,
    distance_alpha_from_detail: bool,
    soft_edges: bool,
    scale_soft_edges: bool,
    edge_softness_start: f32,
    edge_softness_end: f32,
    outline: bool,
    outline_color: [f32; 3],
    outline_alpha: f32,
    outline_start_0: f32,
    outline_start_1: f32,
    outline_end_0: f32,
    outline_end_1: f32,
    scale_outline: bool,
    glow: bool,
    glow_color: [f32; 3],
    glow_alpha: f32,
    glow_start: f32,
    glow_end: f32,
    glow_x: f32,
    glow_y: f32,
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

    fn required_expected(
        &mut self,
        path: &str,
        consumer: impl Into<String>,
        expected_bytes: usize,
        expected_sha256: &str,
        fallback: &Content,
    ) -> Result<Vec<u8>, String> {
        let canonical = path.to_ascii_lowercase();
        let consumer = consumer.into();
        let bytes = if let Some(bytes) = self.bundle.get(&canonical) {
            let record = self
                .requests
                .get_mut(&canonical)
                .ok_or_else(|| format!("bundled dependency {canonical} has no request record"))?;
            record.consumers.insert(consumer);
            bytes.clone()
        } else {
            match self
                .content
                .resolve_resource(&canonical)
                .map_err(|error| error.to_string())?
            {
                Resolution::Found(_) => self.required(&canonical, consumer)?,
                Resolution::Missing { .. } => {
                    let value = match fallback
                        .resolve_resource(&canonical)
                        .map_err(|error| error.to_string())?
                    {
                        Resolution::Found(value) => value,
                        Resolution::Missing { .. } => {
                            return self.required(&canonical, consumer);
                        }
                    };
                    let bytes = value.bytes.clone();
                    self.inject_platform(
                        &canonical,
                        &consumer,
                        bytes.clone(),
                        provenance_record(&value.provenance),
                    )?;
                    bytes
                }
            }
        };
        if bytes.len() != expected_bytes || digest(&bytes) != expected_sha256 {
            return Err(format!(
                "dependency {canonical} differs from its UI descriptor"
            ));
        }
        Ok(bytes)
    }

    fn inject_platform(
        &mut self,
        path: &str,
        consumer: &str,
        bytes: Vec<u8>,
        provenance: ProvenanceRecord,
    ) -> Result<(), String> {
        let canonical = path.to_ascii_lowercase();
        if self.bundle.contains_key(&canonical) || self.requests.contains_key(&canonical) {
            return Err(format!("duplicate injected dependency {canonical}"));
        }
        let descriptor = ObjectDescriptor::source(&bytes);
        self.bundle.insert(canonical.clone(), bytes);
        self.requests.insert(
            canonical,
            MutableRequestRecord {
                requirement: "required",
                consumers: BTreeSet::from([consumer.to_owned()]),
                descriptor: Some(descriptor),
                provenance: Some(provenance),
                checked: None,
            },
        );
        Ok(())
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
    if include_textures {
        for texture in &material.model_textures {
            resolver.required(
                &texture.logical_path.to_ascii_lowercase(),
                format!("{consumer}:model-texture"),
            )?;
        }
    }
    Ok(())
}

fn resolve_ui_material(
    resolver: &mut Resolver<'_>,
    identity: &str,
) -> Result<playsrc_material::Material, String> {
    let root_bytes = resolver.required(identity, format!("tf2-ui-material:{identity}"))?;
    let mut responses = Vec::new();
    loop {
        match playsrc_vmt::compose(
            &root_bytes,
            identity.to_owned(),
            &responses,
            &ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|error| error.to_string())?
        {
            Composition::Complete(document) => {
                return playsrc_material::resolve_for_environment(
                    &document,
                    SelectionEnvironment::default(),
                )
                .map_err(|error| error.to_string());
            }
            Composition::Needs(requests) => {
                for request in requests {
                    let path = material_path(&request.target_token)?;
                    let bytes = resolver.required(
                        &path,
                        format!("tf2-ui-material-compose:{}", request.parent_identity),
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
    }
}

fn ui_parameter<'a>(material: &'a playsrc_material::Material, name: &[u8]) -> Option<&'a [u8]> {
    material
        .first_parameters
        .get(&name.to_ascii_lowercase())
        .map(Vec::as_slice)
}

fn ui_float(
    material: &playsrc_material::Material,
    name: &[u8],
    default: f32,
) -> Result<f32, String> {
    let Some(value) = ui_parameter(material, name) else {
        return Ok(default);
    };
    let value = std::str::from_utf8(value).map_err(|_| "UI material float is not UTF-8")?;
    value.trim().parse::<f32>().map_err(|_| {
        format!(
            "UI material float is malformed: {}",
            String::from_utf8_lossy(name)
        )
    })
}

fn ui_integer(
    material: &playsrc_material::Material,
    name: &[u8],
    default: i32,
) -> Result<i32, String> {
    Ok(ui_float(material, name, default as f32)? as i32)
}

fn ui_boolean(
    material: &playsrc_material::Material,
    name: &[u8],
    default: bool,
) -> Result<bool, String> {
    Ok(ui_integer(material, name, i32::from(default))? != 0)
}

fn ui_vector(
    material: &playsrc_material::Material,
    name: &[u8],
    default: [f32; 3],
) -> Result<[f32; 3], String> {
    let Some(value) = ui_parameter(material, name) else {
        return Ok(default);
    };
    let text = std::str::from_utf8(value).map_err(|_| "UI material vector is not UTF-8")?;
    let values = text
        .trim_matches(|character: char| {
            character.is_whitespace() || character == '[' || character == ']'
        })
        .split_whitespace()
        .map(|value| {
            value
                .parse::<f32>()
                .map_err(|_| "UI material vector is malformed".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 3 {
        return Err("UI material vector component count differs".to_owned());
    }
    Ok([values[0], values[1], values[2]])
}

fn ui_texture_path(token: &[u8]) -> Result<String, String> {
    let value = std::str::from_utf8(token)
        .map_err(|_| "UI texture identity is not UTF-8")?
        .replace('\\', "/");
    let mut path = if value.to_ascii_lowercase().starts_with("materials/") {
        value
    } else {
        format!("materials/{value}")
    };
    if !path.to_ascii_lowercase().ends_with(".vtf") {
        path.push_str(".vtf");
    }
    let path = path.to_ascii_lowercase();
    if path.contains("..") || path.contains("//") {
        return Err("UI texture identity is malformed".to_owned());
    }
    Ok(path)
}

fn ui_role_texture(material: &playsrc_material::Material, role: TextureRole) -> Option<String> {
    material
        .textures
        .iter()
        .find(|texture| texture.role == role && texture.disposition == TextureDisposition::Source)
        .and_then(|texture| texture.logical_path.clone())
        .map(|path| path.to_ascii_lowercase())
}

fn ui_role_color_read(
    material: &playsrc_material::Material,
    role: TextureRole,
) -> Option<&'static str> {
    material
        .textures
        .iter()
        .find(|texture| texture.role == role && texture.disposition == TextureDisposition::Source)
        .map(|texture| match texture.color_read {
            TextureColorRead::Srgb => "srgb",
            TextureColorRead::Linear => "linear",
            TextureColorRead::FormatDependent => "format-dependent",
        })
}

fn ui_material_record(
    configured_value: &str,
    identity: &str,
    material: &playsrc_material::Material,
) -> Result<Tf2UiMaterialRecord, String> {
    let shader = String::from_utf8(material.shader_token.clone())
        .map_err(|_| "UI material shader is not UTF-8")?;
    let base_texture = ui_role_texture(material, TextureRole::Base)
        .ok_or_else(|| format!("UI material {identity} has no base texture"))?;
    let base_color_read = ui_role_color_read(material, TextureRole::Base)
        .ok_or_else(|| format!("UI material {identity} has no base color-read state"))?;
    let second_texture = ui_parameter(material, b"$texture2")
        .map(ui_texture_path)
        .transpose()?;
    let detail = material.detail.as_ref();
    Ok(Tf2UiMaterialRecord {
        configured_value: configured_value.to_owned(),
        material: identity.to_owned(),
        shader,
        base_texture,
        base_color_read,
        second_texture,
        second_color_read: ui_parameter(material, b"$texture2").map(|_| "srgb"),
        detail_texture: detail.and_then(|value| value.texture.logical_path.clone()),
        detail_color_read: detail.and_then(|_| ui_role_color_read(material, TextureRole::Detail)),
        detail_scale: detail.map_or(1.0, |value| value.scale),
        detail_blend_mode: detail.map_or(0, |value| value.blend_mode),
        detail_blend_factor: detail.map_or(1.0, |value| value.blend_factor),
        detail_tint: detail.map_or([1.0, 1.0, 1.0], |value| value.tint),
        distance_alpha: ui_boolean(material, b"$distancealpha", false)?,
        distance_alpha_from_detail: ui_boolean(material, b"$distancealphafromdetail", false)?,
        soft_edges: ui_boolean(material, b"$softedges", false)?,
        scale_soft_edges: ui_boolean(material, b"$scaleedgesoftnessbasedonscreenres", false)?,
        edge_softness_start: ui_float(material, b"$edgesoftnessstart", 0.6)?,
        edge_softness_end: ui_float(material, b"$edgesoftnessend", 0.5)?,
        outline: ui_boolean(material, b"$outline", false)?,
        outline_color: ui_vector(material, b"$outlinecolor", [1.0, 1.0, 1.0])?,
        outline_alpha: ui_float(material, b"$outlinealpha", 0.0)?,
        outline_start_0: ui_float(material, b"$outlinestart0", 0.0)?,
        outline_start_1: ui_float(material, b"$outlinestart1", 0.0)?,
        outline_end_0: ui_float(material, b"$outlineend0", 0.0)?,
        outline_end_1: ui_float(material, b"$outlineend1", 0.0)?,
        scale_outline: ui_boolean(material, b"$scaleoutlinesoftnessbasedonscreenres", false)?,
        glow: ui_boolean(material, b"$glow", false)?,
        glow_color: ui_vector(material, b"$glowcolor", [1.0, 1.0, 1.0])?,
        glow_alpha: ui_float(material, b"$glowalpha", 1.0)?,
        glow_start: ui_float(material, b"$glowstart", 0.7)?,
        glow_end: ui_float(material, b"$glowend", 0.5)?,
        glow_x: ui_float(material, b"$glowx", 0.0)?,
        glow_y: ui_float(material, b"$glowy", 0.0)?,
    })
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
    let profile = model_profile(&mdl).map_err(|error| format!("{identity}: {error}"))?;
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

fn load_tf2_ui_manifest(root: &Path) -> Result<Tf2UiBundleManifest, String> {
    let manifest: Tf2UiBundleManifest = serde_json::from_slice(
        &fs::read(root.join("tools/source-bundle/tf2-ui.generated.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if manifest.schema != "playsrc-tf2-ui-bundle-v1"
        || manifest.identity != "tf2-ui-24207079-4a097b1e805d9ce1"
        || manifest.source_ledger.is_empty()
        || manifest.dependencies.is_empty()
        || manifest.dependencies.len() > MAX_DEPENDENCY_REQUESTS
        || manifest.images.len() > 2_048
        || manifest.missing_dependencies.len() > 128
    {
        return Err("TF2 UI bundle manifest identity is malformed".to_owned());
    }
    let mut previous = None::<&str>;
    for dependency in &manifest.dependencies {
        if dependency.logical_path != dependency.logical_path.to_ascii_lowercase()
            || dependency.logical_path.is_empty()
            || dependency.sha256.len() != 64
            || !dependency
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || dependency.byte_length == 0
            || dependency.kinds.is_empty()
            || dependency
                .kinds
                .iter()
                .any(|kind| !matches!(kind.as_str(), "resource" | "font" | "material" | "texture"))
            || previous.is_some_and(|value| value >= dependency.logical_path.as_str())
        {
            return Err(format!(
                "TF2 UI dependency descriptor is malformed: {}",
                dependency.logical_path
            ));
        }
        previous = Some(&dependency.logical_path);
    }
    let mut image_identities = BTreeSet::new();
    let mut missing = BTreeSet::new();
    for value in &manifest.missing_dependencies {
        if value.is_empty() || !missing.insert(value) {
            return Err("TF2 UI missing dependency set is malformed".to_owned());
        }
    }
    for image in &manifest.images {
        if image.identity.is_empty()
            || !image_identities.insert(&image.identity)
            || image.configured_value.is_empty()
            || !matches!(
                image.classification.as_str(),
                "content-vtf" | "missing-material"
            )
            || (image.classification == "content-vtf"
                && (image.material.is_none() || image.textures.is_empty()))
            || (image.classification == "missing-material"
                && (image.material.is_some() || !image.textures.is_empty()))
        {
            return Err(format!(
                "TF2 UI image descriptor is malformed: {}",
                image.identity
            ));
        }
        for texture in &image.textures {
            if texture.logical_path != texture.logical_path.to_ascii_lowercase()
                || !texture.logical_path.ends_with(".vtf")
                || texture.sha256.len() != 64
                || texture.width == 0
                || texture.height == 0
                || texture.frames == 0
            {
                return Err(format!(
                    "TF2 UI image texture is malformed: {}",
                    image.identity
                ));
            }
        }
    }
    Ok(manifest)
}

fn tf2_ui_png(bytes: &[u8], frame: u16) -> Result<(u32, u32, Vec<u8>), String> {
    let plane = playsrc_vtf::decode(
        bytes,
        playsrc_vtf::Dialect::Source2013Pc,
        playsrc_vtf::SubresourceIdentity::HighResolution {
            mip: 0,
            frame,
            face: playsrc_vtf::Face::Right,
            slice: 0,
        },
        playsrc_vtf::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    if plane.row_order != playsrc_vtf::RowOrder::TopToBottom
        || plane.scalar_encoding != playsrc_vtf::ScalarEncoding::U8
    {
        return Err("TF2 UI texture plane is not browser-presentable RGBA8".to_owned());
    }
    let rgba = match plane.channel_layout {
        playsrc_vtf::ChannelLayout::Rgba => plane.samples,
        playsrc_vtf::ChannelLayout::Rgb => {
            let mut output = Vec::with_capacity(plane.width as usize * plane.height as usize * 4);
            for sample in plane.samples.chunks_exact(3) {
                output.extend_from_slice(sample);
                output.push(255);
            }
            output
        }
    };
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, plane.width, plane.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::FilterType::Paeth);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(&rgba)
            .map_err(|error| error.to_string())?;
    }
    Ok((plane.width, plane.height, png))
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
    ui_entries: usize,
    ui_bytes: usize,
    ui_sha256: String,
    ui_descriptor: ObjectDescriptor,
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
    let tf2_ui = load_tf2_ui_manifest(&root)?;
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
    let platform_root = install.join("platform");
    let platform_vpk = vpk_index_path(&install.join("platform/platform_misc.vpk"));
    let platform_vpk_revision =
        digest(&fs::read(&platform_vpk).map_err(|error| error.to_string())?);
    let platform_content = Content::open(
        "platform",
        CONTENT_BUILD,
        vec![
            ProviderSpec::Directory {
                id: "platform-loose".to_owned(),
                revision: CONTENT_BUILD.to_owned(),
                root: platform_root.clone(),
            },
            ProviderSpec::Vpk {
                id: "platform-misc".to_owned(),
                revision: platform_vpk_revision.clone(),
                directory_file: platform_vpk.clone(),
                layout: playsrc_vpk::Layout::Split,
            },
        ],
        playsrc_content::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let platform_order = provider_records.len();
    provider_records.push(ProviderRecord {
        order: platform_order,
        identity: "platform-loose".to_owned(),
        kind: "directory",
        revision: CONTENT_BUILD.to_owned(),
        configured_location: configured_location(install, &platform_root)?,
        path_ids: vec!["platform".to_owned()],
    });
    provider_records.push(ProviderRecord {
        order: platform_order + 1,
        identity: "platform-misc".to_owned(),
        kind: "vpk",
        revision: platform_vpk_revision,
        configured_location: configured_location(install, &platform_vpk)?,
        path_ids: vec!["platform".to_owned()],
    });
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
        "models/weapons/c_models/c_soldier_arms.mdl",
        "models/weapons/c_models/c_demo_arms.mdl",
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
                    SelectionEnvironment {
                        model: true,
                        ..SelectionEnvironment::default()
                    },
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
        "resource/sourcescheme.res",
        "resource/sourceschemebase.res",
        "resource/tf2build.ttf",
        "resource/halflife2.ttf",
        "resource/hl2ep2.ttf",
        "resource/marlett.ttf",
        "resource/linux_fonts/dejavusans.ttf",
        "resource/linux_fonts/dejavusans-bold.ttf",
        "resource/linux_fonts/dejavusans-boldoblique.ttf",
        "resource/linux_fonts/dejavusans-oblique.ttf",
        "resource/linux_fonts/liberationsans-regular.ttf",
        "resource/linux_fonts/liberationsans-bold.ttf",
        "resource/linux_fonts/liberationmono-regular.ttf",
        "resource/linux_fonts/firasans-regular.ttf",
        "scripts/game_sounds_weapons.txt",
        "scripts/soundmixers.txt",
        "sound/weapons/rocket_shoot.wav",
        "sound/weapons/stickybomblauncher_shoot.wav",
        "sound/weapons/quake_rpg_fire_remastered.wav",
        "sound/weapons/quake_explosion_remastered.wav",
        "sound/weapons/explode1.wav",
        "sound/weapons/explode2.wav",
        "sound/weapons/explode3.wav",
        "sound/weapons/pipe_bomb1.wav",
        "sound/weapons/pipe_bomb2.wav",
        "sound/weapons/pipe_bomb3.wav",
    ] {
        let consumer = if path.starts_with("sound/") {
            "audio-wave"
        } else if path.ends_with(".ttf") || path.ends_with(".vbf") {
            "vgui-font"
        } else if path.starts_with("resource/") {
            "vgui-scheme"
        } else {
            "audio-script"
        };
        if path != "resource/sourcescheme.res"
            && path != "resource/tf2build.ttf"
            && (path.starts_with("resource/") || path.ends_with(".vbf"))
        {
            let authored = match path {
                "resource/halflife2.ttf" => Some("resource/HALFLIFE2.ttf"),
                "resource/hl2ep2.ttf" => Some("resource/HL2EP2.ttf"),
                "resource/marlett.ttf" => Some("resource/marlett.ttf"),
                "resource/linux_fonts/dejavusans.ttf" => {
                    Some("resource/linux_fonts/DejaVuSans.ttf")
                }
                "resource/linux_fonts/dejavusans-bold.ttf" => {
                    Some("resource/linux_fonts/DejaVuSans-Bold.ttf")
                }
                "resource/linux_fonts/dejavusans-boldoblique.ttf" => {
                    Some("resource/linux_fonts/DejaVuSans-BoldOblique.ttf")
                }
                "resource/linux_fonts/dejavusans-oblique.ttf" => {
                    Some("resource/linux_fonts/DejaVuSans-Oblique.ttf")
                }
                "resource/linux_fonts/liberationsans-regular.ttf" => {
                    Some("resource/linux_fonts/LiberationSans-Regular.ttf")
                }
                "resource/linux_fonts/liberationsans-bold.ttf" => {
                    Some("resource/linux_fonts/LiberationSans-Bold.ttf")
                }
                "resource/linux_fonts/liberationmono-regular.ttf" => {
                    Some("resource/linux_fonts/LiberationMono-Regular.ttf")
                }
                "resource/linux_fonts/firasans-regular.ttf" => {
                    Some("resource/linux_fonts/FiraSans-Regular.ttf")
                }
                _ => None,
            };
            if let Some(authored) = authored {
                let root = if matches!(
                    path,
                    "resource/halflife2.ttf" | "resource/hl2ep2.ttf" | "resource/marlett.ttf"
                ) {
                    install.join("hl2")
                } else {
                    platform_root.clone()
                };
                let bytes = fs::read(root.join(authored))
                    .map_err(|error| format!("platform dependency {authored}: {error}"))?;
                resolver.inject_platform(
                    path,
                    consumer,
                    bytes,
                    ProvenanceRecord {
                        provider_identity: "platform-loose".to_owned(),
                        provider_kind: "directory",
                        provider_revision: CONTENT_BUILD.to_owned(),
                        location: format!("platform/{authored}"),
                    },
                )?;
            } else {
                let value = match platform_content
                    .resolve_resource(path)
                    .map_err(|error| error.to_string())?
                {
                    Resolution::Found(value) => value,
                    Resolution::Missing { .. } => {
                        return Err(format!("platform dependency {path} is missing"));
                    }
                };
                let provenance = provenance_record(&value.provenance);
                resolver.inject_platform(path, consumer, value.bytes, provenance)?;
            }
        } else {
            resolver.required(path, consumer)?;
        }
    }
    for dependency in &tf2_ui.dependencies {
        resolver.required_expected(
            &dependency.logical_path,
            format!("tf2-ui:{}:{}", tf2_ui.identity, dependency.kinds.join("+")),
            dependency.byte_length,
            &dependency.sha256,
            &platform_content,
        )?;
    }
    let mut ui_materials = Vec::new();
    for image in &tf2_ui.images {
        let Some(identity) = image.material.as_deref() else {
            continue;
        };
        let material = resolve_ui_material(&mut resolver, identity)?;
        let record = ui_material_record(&image.configured_value, identity, &material)?;
        if !record.shader.eq_ignore_ascii_case("UnlitGeneric")
            && !record.shader.eq_ignore_ascii_case("UnlitTwoTexture")
        {
            return Err(format!("unsupported selected UI shader: {}", record.shader));
        }
        if record.detail_texture.is_some() && record.detail_blend_mode != 8 {
            return Err(format!(
                "unsupported selected UI detail blend mode: {}:{}",
                identity, record.detail_blend_mode
            ));
        }
        ui_materials.push(record);
    }
    let mut ui_textures = BTreeMap::<String, (String, u32, u32, u16, u32)>::new();
    for image in &tf2_ui.images {
        for texture in &image.textures {
            let candidate = (
                texture.sha256.clone(),
                texture.width,
                texture.height,
                texture.frames,
                texture.raw_flags,
            );
            if let Some(prior) = ui_textures.get(&texture.logical_path)
                && prior != &candidate
            {
                return Err(format!(
                    "TF2 UI texture metadata conflicts: {}",
                    texture.logical_path
                ));
            }
            ui_textures
                .entry(texture.logical_path.clone())
                .or_insert(candidate);
        }
    }
    let mut runtime_ui_materials = (1..=4)
        .map(|index| {
            let configured = format!("vgui/hud/8x800corner{index}");
            let identity = format!("materials/{configured}.vmt");
            (configured, identity)
        })
        .collect::<Vec<_>>();
    runtime_ui_materials.extend([
        (
            "hud/health_color".to_owned(),
            "materials/hud/health_color.vmt".to_owned(),
        ),
        (
            "hud/health_dead".to_owned(),
            "materials/hud/health_dead.vmt".to_owned(),
        ),
    ]);
    for (configured_value, identity) in runtime_ui_materials {
        let material = resolve_ui_material(&mut resolver, &identity)?;
        let record = ui_material_record(&configured_value, &identity, &material)?;
        let bytes = resolver.required(
            &record.base_texture,
            format!("tf2-ui-rounded-background:{configured_value}"),
        )?;
        let metadata = playsrc_vtf::inspect(
            &bytes,
            playsrc_vtf::Dialect::Source2013Pc,
            playsrc_vtf::Limits::default(),
        )
        .map_err(|error| error.to_string())?;
        ui_textures.insert(
            record.base_texture.clone(),
            (
                digest(&bytes),
                metadata.width,
                metadata.height,
                metadata.frame_count,
                metadata.raw_flags,
            ),
        );
        ui_materials.push(record);
    }
    for material in &ui_materials {
        for logical_path in [
            Some(material.base_texture.as_str()),
            material.second_texture.as_deref(),
            material.detail_texture.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let bytes = resolver.required(
                logical_path,
                format!("tf2-ui-material-texture:{}", material.material),
            )?;
            if let Some((sha256, _, _, _, _)) = ui_textures.get(logical_path) {
                if sha256 != &digest(&bytes) {
                    return Err(format!("TF2 UI material texture changed: {logical_path}"));
                }
                continue;
            }
            let metadata = playsrc_vtf::inspect(
                &bytes,
                playsrc_vtf::Dialect::Source2013Pc,
                playsrc_vtf::Limits::default(),
            )
            .map_err(|error| error.to_string())?;
            ui_textures.insert(
                logical_path.to_owned(),
                (
                    digest(&bytes),
                    metadata.width,
                    metadata.height,
                    metadata.frame_count,
                    metadata.raw_flags,
                ),
            );
        }
    }
    let texture_records = ui_textures
        .iter()
        .map(
            |(logical_path, (sha256, width, height, frames, raw_flags))| {
                Tf2UiMaterialTextureRecord {
                    logical_path: logical_path.clone(),
                    sha256: sha256.clone(),
                    width: *width,
                    height: *height,
                    frames: *frames,
                    raw_flags: *raw_flags,
                }
            },
        )
        .collect::<Vec<_>>();
    let ui_material_bytes = serde_json::to_vec(&Tf2UiMaterialSet {
        schema: "playsrc-tf2-ui-materials-v1",
        descriptor: tf2_ui.identity.clone(),
        images: ui_materials,
        textures: texture_records,
    })
    .map_err(|error| error.to_string())?;
    let mut ui_bundle = BTreeMap::<String, Vec<u8>>::new();
    ui_bundle.insert(
        "playsrc/tf2-ui/materials.json".to_owned(),
        ui_material_bytes,
    );
    for (logical_path, (sha256, width, height, frames, _raw_flags)) in ui_textures {
        let bytes = resolver
            .bundle
            .get(&logical_path)
            .ok_or_else(|| format!("TF2 UI texture is absent from bundle: {logical_path}"))?
            .clone();
        if digest(&bytes) != sha256 {
            return Err(format!(
                "TF2 UI texture changed before presentation: {logical_path}"
            ));
        }
        for frame in 0..frames {
            let (decoded_width, decoded_height, png) = tf2_ui_png(&bytes, frame)
                .map_err(|error| format!("TF2 UI texture {logical_path} frame {frame}: {error}"))?;
            if decoded_width != width || decoded_height != height {
                return Err(format!("TF2 UI texture dimensions changed: {logical_path}"));
            }
            ui_bundle.insert(format!("playsrc/tf2-ui/png/{sha256}/{frame}.png"), png);
        }
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
    let mut ui_output = b"PUIB".to_vec();
    ui_output.extend_from_slice(&1_u32.to_le_bytes());
    ui_output.extend_from_slice(
        &u32::try_from(ui_bundle.len())
            .map_err(|_| "TF2 UI presentation entry count exceeds u32".to_owned())?
            .to_le_bytes(),
    );
    for (path, bytes) in &ui_bundle {
        bytesv(&mut ui_output, path.as_bytes())?;
        bytesv(&mut ui_output, bytes)?;
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
    let ui_descriptor = ObjectDescriptor::new("derived-object", BUNDLE_MEDIA_TYPE, &ui_output);
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
    let ui_destination = directory.join(format!("{target}.ui.puib"));
    let ledger_destination = directory.join(format!("{target}.dependencies.json"));
    install_artifact(&destination, &output)?;
    install_artifact(&ui_destination, &ui_output)?;
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
        ui_entries: ui_bundle.len(),
        ui_bytes: ui_output.len(),
        ui_sha256: digest(&ui_output),
        ui_descriptor,
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
