use playsrc_asset_graph::{Resource, ResourceGraph};
use playsrc_content::{
    CheckedLocation, Content, Provenance, ProviderKind, ProviderSpec, Resolution,
};
use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::{
    HdrMode, SelectionEnvironment, TextureColorRead, TextureDisposition, TextureRole,
};
use playsrc_vmt::{Composition, DependencyResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

const SOURCE_MEDIA_TYPE: &str = "application/octet-stream";
const LEDGER_MEDIA_TYPE: &str = "application/vnd.playsrc.source-dependency-ledger+json";
const GRAPH_MEDIA_TYPE: &str = "application/vnd.playsrc.resource-graph+json";
const MAX_GAME_PROVIDERS: usize = 64;
const MAX_DEPENDENCY_REQUESTS: usize = 8_192;
const MAX_LEDGER_BYTES: usize = 8 * 1024 * 1024;
const MAX_UI_PNG_WORKERS: usize = 8;
const STATIC_PROP_VHV_AGGREGATE_PATH: &str = "derived/static-prop-lighting.pvha";
const STATIC_PROP_VHV_AGGREGATE_VERSION: u32 = 2;
const MAX_STATIC_PROP_VHV_OBJECTS: usize = 8_192;
const MAX_STATIC_PROP_VHV_AGGREGATE_BYTES: usize = 256 * 1024 * 1024;

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
struct ContentBuildContract {
    schema: String,
    app_id: String,
    content_build: String,
    patch_version: String,
    gameinfo_sha256: String,
    custom_mod_providers: String,
    archive_indexes: ArchiveIndexContract,
    installed_depots: Vec<DepotContract>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArchiveIndexContract {
    tf2_misc: String,
    tf2_textures: String,
    tf2_sound_misc: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DepotContract {
    depot: String,
    manifest: String,
    byte_length: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tf2UiBundleManifest {
    schema: String,
    identity: String,
    content_build: String,
    source_ledger: String,
    dependencies: Vec<Tf2UiDependency>,
    images: Vec<Tf2UiImage>,
    dynamic_images: Vec<Tf2UiDynamicImage>,
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
struct Tf2UiDynamicImage {
    configured_value: String,
    material: String,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2UiMaterialRecord {
    configured_value: String,
    material: String,
    material_sha256: String,
    shader: String,
    base_texture: String,
    base_color_read: &'static str,
    second_texture: Option<String>,
    second_color_read: Option<&'static str>,
    detail_texture: Option<String>,
    detail_color_read: Option<&'static str>,
    detail_scale: [f32; 2],
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2GameUiBackgroundSet {
    schema: &'static str,
    content_build: String,
    chapter_source: Tf2GameUiBackgroundSource,
    default_chapter: u32,
    background_name: String,
    variants: Vec<Tf2GameUiBackgroundVariant>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2GameUiBackgroundSource {
    logical_path: String,
    byte_length: usize,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tf2GameUiBackgroundVariant {
    aspect: &'static str,
    configured_value: String,
    material: String,
    material_sha256: String,
    texture: String,
    texture_sha256: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapTarget {
    logical_path: String,
    download: Option<Download>,
    installed: Option<InstalledMap>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledMap {
    content_build: String,
    provider: String,
    byte_length: usize,
    sha256: String,
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
    encoded: Option<EncodedMapSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    decoded: ObjectDescriptor,
    cache_path: String,
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
    app_id: String,
    content_build: String,
    patch_version: String,
    installed_depots: Vec<DepotContract>,
    target: String,
    gameinfo_sha256: String,
    map: MapSourceRecord,
    startup_sources: Vec<DeclaredSourceRecord>,
    providers: Vec<ProviderRecord>,
    requests: Vec<RequestRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    static_prop_vhv_aggregate: Option<StaticPropVhvAggregateDescriptor>,
    resolved_entries: usize,
    authoritative_absences: usize,
    resource_graph: ObjectDescriptor,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticPropVhvAggregateDescriptor {
    logical_path: &'static str,
    object_count: usize,
    descriptor: ObjectDescriptor,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredSourceRecord {
    logical_path: String,
    descriptor: ObjectDescriptor,
    provenance: ProvenanceRecord,
    disposition: &'static str,
}

struct Resolver<'a> {
    content: &'a Content,
    bundle: BTreeMap<String, Vec<u8>>,
    packed_sources: BTreeMap<String, ObjectDescriptor>,
    requests: BTreeMap<String, MutableRequestRecord>,
}

impl<'a> Resolver<'a> {
    fn new(content: &'a Content) -> Self {
        Self {
            content,
            bundle: BTreeMap::new(),
            packed_sources: BTreeMap::new(),
            requests: BTreeMap::new(),
        }
    }

    fn pack_sources(&mut self, paths: &BTreeSet<String>) -> Result<(), String> {
        for path in paths {
            let bytes = self
                .bundle
                .remove(path)
                .ok_or_else(|| format!("packed source is absent from bundle: {path}"))?;
            let descriptor = ObjectDescriptor::source(&bytes);
            let request = self
                .requests
                .get(path)
                .and_then(|record| record.descriptor.as_ref())
                .ok_or_else(|| format!("packed source request is absent: {path}"))?;
            if request != &descriptor
                || self
                    .packed_sources
                    .insert(path.clone(), descriptor)
                    .is_some()
            {
                return Err(format!("packed source identity differs: {path}"));
            }
        }
        Ok(())
    }

    fn required(&mut self, path: &str, consumer: impl Into<String>) -> Result<Vec<u8>, String> {
        self.resolve(path, consumer.into(), true)?
            .ok_or_else(|| format!("required dependency {path} has authoritative absence"))
    }

    fn required_pinned(
        &mut self,
        path: &str,
        consumer: impl Into<String>,
        expected_bytes: usize,
        expected_sha256: &str,
    ) -> Result<Vec<u8>, String> {
        let bytes = self.required(path, consumer)?;
        let actual_sha256 = digest(&bytes);
        if bytes.len() != expected_bytes || actual_sha256 != expected_sha256 {
            return Err(format!(
                "dependency {} differs from its configured identity: bytes={} sha256={}",
                path.to_ascii_lowercase(),
                bytes.len(),
                actual_sha256
            ));
        }
        Ok(bytes)
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
        if let Some(record) = self.requests.get_mut(&canonical) {
            if required {
                record.requirement = "required";
            }
            record.consumers.insert(consumer);
            if let Some(bytes) = self.bundle.get(&canonical) {
                return Ok(Some(bytes.clone()));
            }
            if required {
                let locations = record
                    .checked
                    .as_ref()
                    .into_iter()
                    .flatten()
                    .map(|value| format!("{}:{}", value.provider_identity, value.location))
                    .collect::<Vec<_>>()
                    .join(",");
                return Err(format!(
                    "missing required dependency {canonical} after [{locations}]"
                ));
            }
            return Ok(None);
        }
        let result = self.content.resolve_resource(&canonical).map_err(|error| {
            format!(
                "{error}: {} from {}",
                error.logical_path.as_deref().unwrap_or(&canonical),
                error.provider_id.as_deref().unwrap_or("unknown provider"),
            )
        })?;
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

fn configured_source(
    content: &Content,
    path: &str,
    expected_bytes: usize,
    expected_sha256: &str,
    expected_provider: &str,
    disposition: &'static str,
) -> Result<(DeclaredSourceRecord, Vec<u8>), String> {
    let value = match content
        .resolve_resource(path)
        .map_err(|error| error.to_string())?
    {
        Resolution::Found(value) => value,
        Resolution::Missing { checked, .. } => {
            return Err(format!(
                "configured source {path} is missing after {} locations",
                checked.len()
            ));
        }
    };
    let actual_sha256 = digest(&value.bytes);
    if value.bytes.len() != expected_bytes
        || actual_sha256 != expected_sha256
        || value.provenance.provider_id != expected_provider
    {
        return Err(format!(
            "configured source {path} differs: bytes={} sha256={} provider={}",
            value.bytes.len(),
            actual_sha256,
            value.provenance.provider_id
        ));
    }
    Ok((
        DeclaredSourceRecord {
            logical_path: path.to_owned(),
            descriptor: ObjectDescriptor::source(&value.bytes),
            provenance: provenance_record(&value.provenance),
            disposition,
        },
        value.bytes,
    ))
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

fn app_manifest_path(install: &Path) -> PathBuf {
    let standard_steamapps = install
        .parent()
        .filter(|parent| {
            parent
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("common"))
        })
        .and_then(Path::parent);
    standard_steamapps.map_or_else(
        || install.join("steamapps/appmanifest_440.acf"),
        |steamapps| steamapps.join("appmanifest_440.acf"),
    )
}

fn verify_install_manifest(install: &Path, contract: &ContentBuildContract) -> Result<(), String> {
    if contract.schema != "playsrc-tf2-content-build-v1"
        || contract.app_id != "440"
        || contract.custom_mod_providers != "workshop-only"
        || contract.content_build.is_empty()
        || contract.patch_version.is_empty()
        || contract.gameinfo_sha256.len() != 64
        || contract.archive_indexes.tf2_misc.len() != 64
        || contract.archive_indexes.tf2_textures.len() != 64
        || contract.archive_indexes.tf2_sound_misc.len() != 64
        || contract.installed_depots.len() != 3
    {
        return Err("TF2 content-build contract is malformed".to_owned());
    }
    let manifest = app_manifest_path(install);
    let bytes = fs::read(&manifest).map_err(|error| {
        format!(
            "configured TF2 app manifest {}: {error}",
            manifest.display()
        )
    })?;
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
    if scalar(app, b"appid")? != contract.app_id.as_bytes()
        || scalar(app, b"buildid")? != contract.content_build.as_bytes()
    {
        return Err("configured TF2 app or content build changed".to_owned());
    }
    let depots = object_children(app, b"InstalledDepots")?;
    let mut identities = BTreeSet::new();
    for expected in &contract.installed_depots {
        if !identities.insert(expected.depot.as_str()) {
            return Err("TF2 content-build contract contains duplicate depots".to_owned());
        }
        let node = depots
            .iter()
            .find(|node| node.key.bytes == expected.depot.as_bytes())
            .ok_or_else(|| format!("configured TF2 depot {} is missing", expected.depot))?;
        if scalar(node, b"manifest")? != expected.manifest.as_bytes()
            || scalar(node, b"size")? != expected.byte_length.as_bytes()
        {
            return Err(format!(
                "configured TF2 depot {} identity changed",
                expected.depot
            ));
        }
    }
    let tf2 = install.join("tf");
    for (path, expected) in [
        ("tf2_misc_dir.vpk", &contract.archive_indexes.tf2_misc),
        (
            "tf2_textures_dir.vpk",
            &contract.archive_indexes.tf2_textures,
        ),
        (
            "tf2_sound_misc_dir.vpk",
            &contract.archive_indexes.tf2_sound_misc,
        ),
    ] {
        if digest(&fs::read(tf2.join(path)).map_err(|error| error.to_string())?) != *expected {
            return Err(format!("configured TF2 archive index {path} changed"));
        }
    }
    Ok(())
}

fn verify_patch(tf2: &Path, patch_version: &str) -> Result<(), String> {
    let bytes = fs::read(tf2.join("steam.inf")).map_err(|error| error.to_string())?;
    for key in ["PatchVersion", "ClientVersion", "ServerVersion"] {
        let expected = format!("{key}={patch_version}");
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
    content_build: &str,
    gameinfo_sha256: &str,
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
        let custom_mod = path_ids.iter().any(|value| value == "custom_mod");
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
        let mut locations = if declared.contains('*') || declared.contains('?') {
            wildcard_locations(&resolved)?
        } else {
            vec![resolved]
        };
        if custom_mod {
            locations.retain(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("workshop"))
            });
        }
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
                format!("{content_build}-{gameinfo_sha256}-{order:02}")
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
    material_sha256: &str,
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
        material_sha256: material_sha256.to_owned(),
        shader,
        base_texture,
        base_color_read,
        second_texture,
        second_color_read: ui_parameter(material, b"$texture2").map(|_| "srgb"),
        detail_texture: detail.and_then(|value| value.texture.logical_path.clone()),
        detail_color_read: detail.and_then(|_| ui_role_color_read(material, TextureRole::Detail)),
        detail_scale: detail.map_or([1.0; 2], |value| value.scale),
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

fn surface_property_manifest_files(bytes: &[u8]) -> Result<Vec<String>, String> {
    let document = playsrc_keyvalues::parse_text(
        bytes,
        playsrc_keyvalues::EscapeMode::Escaped,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let nodes = if document.roots.len() == 1 {
        match &document.roots[0].value {
            playsrc_keyvalues::Value::Object(children) => children.as_slice(),
            _ => document.roots.as_slice(),
        }
    } else {
        document.roots.as_slice()
    };
    nodes
        .iter()
        .map(|node| {
            if !node.key.bytes.eq_ignore_ascii_case(b"file") || node.condition.is_some() {
                return Err("surface-property manifest contains a non-file entry".to_owned());
            }
            let playsrc_keyvalues::Value::Scalar(value) = &node.value else {
                return Err("surface-property manifest file entry is not scalar".to_owned());
            };
            std::str::from_utf8(&value.token.bytes)
                .map(|path| path.replace('\\', "/").to_ascii_lowercase())
                .map_err(|_| "surface-property path is not UTF-8".to_owned())
        })
        .collect()
}

fn vhv_limits(source_bytes: usize) -> playsrc_vhv::Limits {
    playsrc_vhv::Limits {
        max_input_bytes: source_bytes,
        max_retained_bytes: source_bytes.saturating_mul(2),
        max_meshes: source_bytes / playsrc_vhv::MESH_HEADER_BYTES,
        max_total_vertices: source_bytes / playsrc_vhv::VERTEX_BYTES,
        max_vertices_per_mesh: source_bytes / playsrc_vhv::VERTEX_BYTES,
        max_lod: u32::MAX as usize,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticPropVhvRecord {
    occurrence: u32,
    model: u32,
    profile: u8,
    logical_path: String,
    source_sha256: [u8; 32],
    parsed_sha256: [u8; 32],
    join_sha256: [u8; 32],
    mesh_count: u32,
    vertex_count: u32,
    meshes: Vec<StaticPropVhvMeshRecord>,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticPropVhvMeshRecord {
    primitive: u32,
    body_part: u32,
    model: u32,
    lod: u32,
    mesh: u32,
    strip_group: u32,
    vertex_count: u32,
    encoded_bgra_start: u32,
    encoded_bgra_end: u32,
}

fn static_prop_vhv_join_identity(
    occurrence: usize,
    model: usize,
    profile: u8,
    path: &str,
    joined: &playsrc_studio_model::StaticLightingJoin,
) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-static-prop-vhv-join-v1\0");
    digest.update(
        u32::try_from(occurrence)
            .map_err(|_| "VHV occurrence overflow")?
            .to_le_bytes(),
    );
    digest.update(
        u32::try_from(model)
            .map_err(|_| "VHV model overflow")?
            .to_le_bytes(),
    );
    digest.update([profile]);
    digest.update(
        u32::try_from(path.len())
            .map_err(|_| "VHV path overflow")?
            .to_le_bytes(),
    );
    digest.update(path.as_bytes());
    digest.update(joined.model_checksum.to_le_bytes());
    digest.update(joined.vhv_sha256);
    digest.update(
        u32::try_from(joined.root_lod)
            .map_err(|_| "VHV root LOD overflow")?
            .to_le_bytes(),
    );
    for mesh in &joined.meshes {
        for value in [
            mesh.primitive,
            mesh.body_part,
            mesh.model,
            mesh.lod,
            mesh.mesh,
            mesh.strip_group,
            mesh.vertex_count,
            mesh.encoded_bgra_range.start,
            mesh.encoded_bgra_range.end,
        ] {
            digest.update(
                u32::try_from(value)
                    .map_err(|_| "VHV mesh field overflow")?
                    .to_le_bytes(),
            );
        }
    }
    Ok(digest.finalize().into())
}

fn encode_static_prop_vhv_aggregate(records: &[StaticPropVhvRecord]) -> Result<Vec<u8>, String> {
    if records.len() > MAX_STATIC_PROP_VHV_OBJECTS {
        return Err("static-prop VHV aggregate object bound exceeded".to_owned());
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"PVHA");
    out.extend_from_slice(&STATIC_PROP_VHV_AGGREGATE_VERSION.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(records.len())
            .map_err(|_| "static-prop VHV record count overflow")?
            .to_le_bytes(),
    );
    let mut previous = None;
    for record in records {
        let key = (record.occurrence, record.profile);
        if previous.is_some_and(|value| value >= key)
            || record.profile > 1
            || record.logical_path.len() > 1_024
            || record.bytes.is_empty()
            || usize::try_from(record.mesh_count).ok() != Some(record.meshes.len())
            || record.source_sha256 != <[u8; 32]>::from(Sha256::digest(&record.bytes))
            || record.parsed_sha256 != record.source_sha256
        {
            return Err("static-prop VHV aggregate record is invalid".to_owned());
        }
        previous = Some(key);
        out.extend_from_slice(&record.occurrence.to_le_bytes());
        out.extend_from_slice(&record.model.to_le_bytes());
        out.extend_from_slice(&[record.profile, 0, 0, 0]);
        out.extend_from_slice(&record.mesh_count.to_le_bytes());
        out.extend_from_slice(&record.vertex_count.to_le_bytes());
        out.extend_from_slice(&record.source_sha256);
        out.extend_from_slice(&record.parsed_sha256);
        out.extend_from_slice(&record.join_sha256);
        for mesh in &record.meshes {
            for value in [
                mesh.primitive,
                mesh.body_part,
                mesh.model,
                mesh.lod,
                mesh.mesh,
                mesh.strip_group,
                mesh.vertex_count,
                mesh.encoded_bgra_start,
                mesh.encoded_bgra_end,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            if mesh.encoded_bgra_start > mesh.encoded_bgra_end
                || usize::try_from(mesh.encoded_bgra_end)
                    .map_or(true, |end| end > record.bytes.len())
            {
                return Err("static-prop VHV mesh range is invalid".to_owned());
            }
        }
        for bytes in [record.logical_path.as_bytes(), record.bytes.as_slice()] {
            out.extend_from_slice(
                &u32::try_from(bytes.len())
                    .map_err(|_| "static-prop VHV aggregate field overflow")?
                    .to_le_bytes(),
            );
            out.extend_from_slice(bytes);
        }
        if out.len() > MAX_STATIC_PROP_VHV_AGGREGATE_BYTES {
            return Err("static-prop VHV aggregate byte bound exceeded".to_owned());
        }
    }
    Ok(out)
}

fn decode_static_prop_vhv_aggregate(bytes: &[u8]) -> Result<Vec<StaticPropVhvRecord>, String> {
    if bytes.len() > MAX_STATIC_PROP_VHV_AGGREGATE_BYTES || bytes.get(..4) != Some(b"PVHA") {
        return Err("static-prop VHV aggregate identity is invalid".to_owned());
    }
    let mut offset = 4usize;
    let version = aggregate_u32(bytes, &mut offset)?;
    let count = usize::try_from(aggregate_u32(bytes, &mut offset)?)
        .map_err(|_| "static-prop VHV count overflow")?;
    if version != STATIC_PROP_VHV_AGGREGATE_VERSION || count > MAX_STATIC_PROP_VHV_OBJECTS {
        return Err("static-prop VHV aggregate header is invalid".to_owned());
    }
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let occurrence = aggregate_u32(bytes, &mut offset)?;
        let model = aggregate_u32(bytes, &mut offset)?;
        let profile = *bytes
            .get(offset)
            .ok_or("static-prop VHV aggregate is truncated")?;
        if profile > 1 || bytes.get(offset + 1..offset + 4) != Some(&[0, 0, 0]) {
            return Err("static-prop VHV aggregate profile is invalid".to_owned());
        }
        offset += 4;
        let mesh_count = aggregate_u32(bytes, &mut offset)?;
        let vertex_count = aggregate_u32(bytes, &mut offset)?;
        let source_sha256 = aggregate_hash(bytes, &mut offset)?;
        let parsed_sha256 = aggregate_hash(bytes, &mut offset)?;
        let join_sha256 = aggregate_hash(bytes, &mut offset)?;
        let mut meshes = Vec::with_capacity(mesh_count as usize);
        for _ in 0..mesh_count {
            meshes.push(StaticPropVhvMeshRecord {
                primitive: aggregate_u32(bytes, &mut offset)?,
                body_part: aggregate_u32(bytes, &mut offset)?,
                model: aggregate_u32(bytes, &mut offset)?,
                lod: aggregate_u32(bytes, &mut offset)?,
                mesh: aggregate_u32(bytes, &mut offset)?,
                strip_group: aggregate_u32(bytes, &mut offset)?,
                vertex_count: aggregate_u32(bytes, &mut offset)?,
                encoded_bgra_start: aggregate_u32(bytes, &mut offset)?,
                encoded_bgra_end: aggregate_u32(bytes, &mut offset)?,
            });
        }
        let logical_path = String::from_utf8(aggregate_blob(bytes, &mut offset, 1_024)?)
            .map_err(|_| "static-prop VHV path is not UTF-8")?;
        let source = aggregate_blob(bytes, &mut offset, MAX_STATIC_PROP_VHV_AGGREGATE_BYTES)?;
        records.push(StaticPropVhvRecord {
            occurrence,
            model,
            profile,
            logical_path,
            source_sha256,
            parsed_sha256,
            join_sha256,
            mesh_count,
            vertex_count,
            meshes,
            bytes: source,
        });
    }
    if offset != bytes.len() {
        return Err("static-prop VHV aggregate has trailing bytes".to_owned());
    }
    encode_static_prop_vhv_aggregate(&records)?;
    Ok(records)
}

fn aggregate_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or("static-prop VHV aggregate overflow")?;
    let value = u32::from_le_bytes(
        bytes
            .get(*offset..end)
            .ok_or("static-prop VHV aggregate is truncated")?
            .try_into()
            .map_err(|_| "static-prop VHV aggregate field is malformed")?,
    );
    *offset = end;
    Ok(value)
}

fn aggregate_hash(bytes: &[u8], offset: &mut usize) -> Result<[u8; 32], String> {
    let end = offset
        .checked_add(32)
        .ok_or("static-prop VHV aggregate overflow")?;
    let value = bytes
        .get(*offset..end)
        .ok_or("static-prop VHV aggregate is truncated")?
        .try_into()
        .map_err(|_| "static-prop VHV aggregate hash is malformed")?;
    *offset = end;
    Ok(value)
}

fn aggregate_blob(bytes: &[u8], offset: &mut usize, maximum: usize) -> Result<Vec<u8>, String> {
    let length = usize::try_from(aggregate_u32(bytes, offset)?)
        .map_err(|_| "static-prop VHV length overflow")?;
    if length > maximum {
        return Err("static-prop VHV aggregate field exceeds bound".to_owned());
    }
    let end = offset
        .checked_add(length)
        .ok_or("static-prop VHV aggregate overflow")?;
    let value = bytes
        .get(*offset..end)
        .ok_or("static-prop VHV aggregate is truncated")?
        .to_vec();
    *offset = end;
    Ok(value)
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
                        bytes: bytes.map(Arc::from),
                    });
                }
            }
        }
    }
}

fn artifact_equals(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
        return Ok(false);
    }
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut offset = 0_usize;
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            return Ok(offset == bytes.len());
        }
        if bytes.get(offset..offset + read) != Some(&buffer[..read]) {
            return Ok(false);
        }
        offset += read;
    }
}

fn install_artifact(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if artifact_equals(path, bytes)? {
        return Ok(());
    }
    let temporary = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("artifact"),
        std::process::id()
    ));
    let result = (|| {
        fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        if !artifact_equals(&temporary, bytes)? {
            return Err("temporary artifact verification failed".to_owned());
        }
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        if !artifact_equals(path, bytes)? {
            return Err("installed artifact verification failed".to_owned());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn load_tf2_ui_manifest(root: &Path, content_build: &str) -> Result<Tf2UiBundleManifest, String> {
    let manifest: Tf2UiBundleManifest = serde_json::from_slice(
        &fs::read(root.join("tools/source-bundle/tf2-ui.generated.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let source_ledger_sha256 = digest(manifest.source_ledger.as_bytes());
    let expected_identity = format!("tf2-ui-{content_build}-{}", &source_ledger_sha256[..16]);
    if manifest.schema != "playsrc-tf2-ui-bundle-v1"
        || manifest.content_build != content_build
        || manifest.identity != expected_identity
        || manifest.source_ledger.is_empty()
        || manifest.dependencies.is_empty()
        || manifest.dependencies.len() > MAX_DEPENDENCY_REQUESTS
        || manifest.images.len() > 2_048
        || manifest.dynamic_images.len() > 128
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
    let static_images = manifest
        .images
        .iter()
        .map(|image| image.configured_value.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut previous_dynamic = None::<String>;
    for image in &manifest.dynamic_images {
        let folded = image.configured_value.to_ascii_lowercase();
        if image.configured_value.is_empty()
            || image.material != image.material.to_ascii_lowercase()
            || !image.material.starts_with("materials/")
            || !image.material.ends_with(".vmt")
            || static_images.contains(&folded)
            || previous_dynamic
                .as_ref()
                .is_some_and(|previous| previous >= &folded)
        {
            return Err(format!(
                "TF2 UI dynamic image descriptor is malformed: {}",
                image.configured_value
            ));
        }
        previous_dynamic = Some(folded);
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
    content_build: String,
    providers: usize,
    requests: usize,
    authoritative_absences: usize,
    entries: usize,
    packed_entries: usize,
    derived_entries: usize,
    graph_entries: usize,
    graph_chunks: usize,
    graph_encoded_bytes: usize,
    graph_descriptor: ObjectDescriptor,
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
    let started = Instant::now();
    let mut stage_started = started;
    let stage = |name: &str, stage_started: &mut Instant| {
        let now = Instant::now();
        eprintln!(
            "source-bundle stage={name} milliseconds={}",
            now.duration_since(*stage_started).as_millis()
        );
        *stage_started = now;
    };
    let mut arguments = env::args().skip(1);
    let target = arguments
        .next()
        .ok_or_else(|| "target is required".to_owned())?;
    let (verify_hdr, diagnose_presentation_bound) = match arguments.next().as_deref() {
        None => (false, false),
        Some("--verify-hdr") => (true, false),
        Some("--diagnose-presentation-bound") => (false, true),
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
    let contract: ContentBuildContract = serde_json::from_slice(
        &fs::read(root.join("games/tf2/content-build.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let tf2_ui = load_tf2_ui_manifest(&root, &contract.content_build)?;
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
        || map_target.download.is_some() == map_target.installed.is_some()
        || map_target.download.as_ref().is_some_and(|download| {
            download.decoded_byte_length == 0
                || download.encoded_byte_length == 0
                || download.encoded_sha256.len() != 64
                || download.decoded_sha256.len() != 64
        })
        || map_target.installed.as_ref().is_some_and(|installed| {
            installed.content_build != contract.content_build
                || installed.provider.is_empty()
                || installed.byte_length == 0
                || installed.sha256.len() != 64
        })
    {
        return Err("declared map source is malformed".to_owned());
    }
    let cache = PathBuf::from(&config.source_cache_dir);
    let cached_bsp = map_target
        .download
        .as_ref()
        .map(|download| {
            let bytes = fs::read(object_path(&cache, &download.decoded_sha256))
                .map_err(|error| error.to_string())?;
            if bytes.len() != download.decoded_byte_length
                || digest(&bytes) != download.decoded_sha256
            {
                return Err("cached BSP differs from its declared identity".to_owned());
            }
            Ok(bytes)
        })
        .transpose()?;
    let tf2 = PathBuf::from(&config.tf2_dir);
    let gameinfo = fs::read(tf2.join("gameinfo.txt")).map_err(|error| error.to_string())?;
    if digest(&gameinfo) != contract.gameinfo_sha256 {
        return Err("configured TF2 gameinfo identity changed".to_owned());
    }
    let install = tf2
        .parent()
        .ok_or_else(|| "tf2Dir has no install parent".to_owned())?;
    verify_install_manifest(install, &contract)?;
    verify_patch(&tf2, &contract.patch_version)?;
    let (providers, mut provider_records) = provider_plan(
        install,
        &tf2,
        &gameinfo,
        &contract.content_build,
        &contract.gameinfo_sha256,
    )?;
    let platform_root = install.join("platform");
    let platform_vpk = vpk_index_path(&install.join("platform/platform_misc.vpk"));
    let platform_vpk_revision =
        digest(&fs::read(&platform_vpk).map_err(|error| error.to_string())?);
    let platform_content = Content::open(
        "platform",
        contract.content_build.clone(),
        vec![
            ProviderSpec::Directory {
                id: "platform-loose".to_owned(),
                revision: contract.content_build.clone(),
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
        revision: contract.content_build.clone(),
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
        contract.content_build.clone(),
        providers,
        playsrc_content::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let (bsp_bytes, map_sha256, map_provider) = if let Some(bytes) = cached_bsp {
        let download = map_target.download.as_ref().expect("validated map source");
        (
            bytes,
            download.decoded_sha256.clone(),
            "source-cache".to_owned(),
        )
    } else {
        let installed = map_target.installed.as_ref().expect("validated map source");
        let playsrc_content::Resolution::Found(resolved) = content
            .resolve_map(&map_target.logical_path)
            .map_err(|error| error.to_string())?
        else {
            return Err(format!(
                "installed map {} is missing",
                map_target.logical_path
            ));
        };
        if resolved.provenance.provider_id != installed.provider
            || resolved.bytes.len() != installed.byte_length
            || resolved.provenance.sha256 != installed.sha256
        {
            return Err("installed BSP differs from its declared provider or identity".to_owned());
        }
        let object = object_path(&cache, &installed.sha256);
        if let Some(parent) = object.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&object, &resolved.bytes).map_err(|error| error.to_string())?;
        (
            resolved.bytes,
            installed.sha256.clone(),
            installed.provider.clone(),
        )
    };
    let bsp = playsrc_bsp::parse(
        &bsp_bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let pak_provider = if target == "jump_beef" {
        "jump-beef-pak".to_owned()
    } else {
        format!("{target}-pak")
    };
    let pak = bsp.lumps[40]
        .pak
        .as_ref()
        .ok_or_else(|| "BSP PAK is unavailable".to_owned())?;
    let content = content
        .with_active_pak(
            pak_provider.clone(),
            map_sha256.clone(),
            map_target.logical_path.clone(),
            pak,
        )
        .map_err(|error| error.to_string())?;
    let mut startup_sources = Vec::new();
    let mut startup_presentation = BTreeMap::new();
    for (path, bytes, sha256, provider, disposition) in [
        (
            "media/startupvids.txt",
            17,
            "b832a9961d1feeb7a723b03a5033a59790cc82c5c742fbffd90f197bead13f7c",
            "game-09-tf",
            "presentation-bundle",
        ),
        (
            "media/valve.bik",
            14_672_796,
            "99a57640d7434a7ef948dd00980e752f237e4b412dbcf502529832f679065381",
            "game-10-hl2",
            "validated-source",
        ),
        (
            "media/valve.webm",
            1_323_798,
            "1cd960acdfe89e99aebe1b5199c2699b5bb17d812ff069d26ee1192435bbd403",
            "game-10-hl2",
            "presentation-bundle",
        ),
    ] {
        let (record, source) =
            configured_source(&content, path, bytes, sha256, provider, disposition)?;
        if disposition == "presentation-bundle" {
            startup_presentation.insert(path.to_owned(), source);
        }
        startup_sources.push(record);
    }
    stage("inputs-and-providers", &mut stage_started);
    for provider in &mut provider_records {
        provider.order += 1;
    }
    provider_records.insert(
        0,
        ProviderRecord {
            order: 0,
            identity: pak_provider,
            kind: "bsp-pak",
            revision: map_sha256.clone(),
            configured_location: format!("{map_provider}/{}", map_target.logical_path),
            path_ids: vec!["game".to_owned()],
        },
    );
    let canonical = playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Ldr)
        .map_err(|error| format!("map-compile:{error:?}"))?;
    let mut resolver = Resolver::new(&content);
    for (path, bytes, sha256, consumer) in [
        (
            "materials/vgui/stamp_background_map.vmt",
            105,
            "3850088d15a9147bc593cab2bbda5bc12eff053ccaa8cec6579bf18513c695d1",
            "tf2-loading-background-material",
        ),
        (
            "materials/vgui/stamp_background_map.vtf",
            1_398_360,
            "2f00d21971c788a51bd254ec5b69ad79af52caad35f0cde2a1ec9f4dbaf4a955",
            "tf2-loading-background-texture",
        ),
    ] {
        resolver.required_pinned(path, consumer, bytes, sha256)?;
    }
    if resolver
        .optional(
            "materials/vgui/maps/menu_photos_jump_beef.vmt",
            "tf2-loading-map-photo",
        )?
        .is_some()
    {
        return Err("jump_beef configured map photo absence changed".to_owned());
    }
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
    let surface_manifest_path = "scripts/surfaceproperties_manifest.txt";
    let surface_manifest = resolver.required(surface_manifest_path, "surface-property-manifest")?;
    for path in surface_property_manifest_files(&surface_manifest)? {
        resolver.required(
            &path,
            format!("surface-property-file:{surface_manifest_path}"),
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
    let mut diagnostic_report = None;
    model_paths.extend(
        canonical
            .static_props
            .models
            .iter()
            .map(|model| model.logical_path.clone()),
    );
    for path in [
        "models/weapons/w_models/w_rocket.mdl",
        "models/weapons/w_models/w_stickybomb.mdl",
        "models/weapons/c_models/c_soldier_arms.mdl",
        "models/weapons/c_models/c_demo_arms.mdl",
        "models/player/scout.mdl",
        "models/player/sniper.mdl",
        "models/player/soldier.mdl",
        "models/player/demo.mdl",
        "models/player/medic.mdl",
        "models/player/heavy.mdl",
        "models/player/pyro.mdl",
        "models/player/spy.mdl",
        "models/player/engineer.mdl",
        "models/vgui/ui_class01.mdl",
        "models/class_menu/random_class_icon.mdl",
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
    ] {
        model_paths.insert(path.to_owned());
    }
    let mut model_documents = BTreeMap::new();
    for path in model_paths {
        let document = collect_model(&mut resolver, &path)
            .map_err(|error| format!("studio-model:{path}:{error}"))?;
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
                )
                .map_err(|error| format!("studio-model:{path}:material:{candidate}:{error}"))?;
            }
        }
        if diagnose_presentation_bound || !canonical.static_props.occurrences.is_empty() {
            model_documents.insert(path, document);
        }
    }
    let static_prop_vhv_aggregate = if canonical.static_props.occurrences.is_empty() {
        None
    } else {
        let mut joined_objects = 0usize;
        let mut joined_meshes = 0usize;
        let mut joined_vertices = 0usize;
        let mut vhv_bytes = 0usize;
        let mut vhv_hashes = BTreeMap::<String, usize>::new();
        let mut packed_paths = BTreeSet::new();
        let mut aggregate_records = Vec::new();
        for prop in &canonical.static_props.occurrences {
            if prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING != 0 {
                continue;
            }
            let model_path = &canonical.static_props.models[prop.model].logical_path;
            let model = model_documents
                .get(model_path)
                .ok_or_else(|| format!("static-prop model document is absent: {model_path}"))?;
            for (profile, path) in [
                (0u8, format!("sp_{}.vhv", prop.source)),
                (1u8, format!("sp_hdr_{}.vhv", prop.source)),
            ] {
                let bytes =
                    resolver.required(&path, format!("static-prop-lighting:{}", prop.source))?;
                let vhv = playsrc_vhv::parse(
                    &bytes,
                    playsrc_vhv::Profile::source_pc_v2_color_bgra8888(model.checksum as u32),
                    vhv_limits(bytes.len()),
                )
                .map_err(|error| format!("{path}: {error}"))?;
                vhv_bytes = vhv_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(|| "static-prop VHV byte count overflow".to_owned())?;
                *vhv_hashes.entry(digest(&bytes)).or_default() += 1;
                let joined = playsrc_studio_model::join_static_lighting(model, &vhv)
                    .map_err(|error| format!("{path}: {error:?}"))?;
                let source_sha256: [u8; 32] = Sha256::digest(&bytes).into();
                let join_sha256 = static_prop_vhv_join_identity(
                    prop.source,
                    prop.model,
                    profile,
                    &path,
                    &joined,
                )?;
                aggregate_records.push(StaticPropVhvRecord {
                    occurrence: u32::try_from(prop.source)
                        .map_err(|_| "static-prop source index overflow")?,
                    model: u32::try_from(prop.model)
                        .map_err(|_| "static-prop model index overflow")?,
                    profile,
                    logical_path: path.clone(),
                    source_sha256,
                    parsed_sha256: vhv.source_identity.sha256,
                    join_sha256,
                    mesh_count: u32::try_from(joined.meshes.len())
                        .map_err(|_| "static-prop VHV mesh count overflow")?,
                    vertex_count: u32::try_from(joined.vertex_count)
                        .map_err(|_| "static-prop VHV vertex count overflow")?,
                    meshes: joined
                        .meshes
                        .iter()
                        .map(|mesh| {
                            Ok(StaticPropVhvMeshRecord {
                                primitive: u32::try_from(mesh.primitive)
                                    .map_err(|_| "static-prop VHV primitive overflow")?,
                                body_part: u32::try_from(mesh.body_part)
                                    .map_err(|_| "static-prop VHV body part overflow")?,
                                model: u32::try_from(mesh.model)
                                    .map_err(|_| "static-prop VHV model overflow")?,
                                lod: u32::try_from(mesh.lod)
                                    .map_err(|_| "static-prop VHV LOD overflow")?,
                                mesh: u32::try_from(mesh.mesh)
                                    .map_err(|_| "static-prop VHV mesh overflow")?,
                                strip_group: u32::try_from(mesh.strip_group)
                                    .map_err(|_| "static-prop VHV strip group overflow")?,
                                vertex_count: u32::try_from(mesh.vertex_count)
                                    .map_err(|_| "static-prop VHV mesh vertex count overflow")?,
                                encoded_bgra_start: u32::try_from(mesh.encoded_bgra_range.start)
                                    .map_err(|_| "static-prop VHV range overflow")?,
                                encoded_bgra_end: u32::try_from(mesh.encoded_bgra_range.end)
                                    .map_err(|_| "static-prop VHV range overflow")?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    bytes,
                });
                packed_paths.insert(path);
                joined_objects += 1;
                joined_meshes = joined_meshes
                    .checked_add(joined.meshes.len())
                    .ok_or_else(|| "static-prop joined mesh count overflow".to_owned())?;
                joined_vertices = joined_vertices
                    .checked_add(joined.vertex_count)
                    .ok_or_else(|| "static-prop joined vertex count overflow".to_owned())?;
            }
        }
        let expected_objects = canonical
            .static_props
            .occurrences
            .iter()
            .filter(|prop| prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING == 0)
            .count()
            .checked_mul(2)
            .ok_or_else(|| "configured static-prop VHV count overflow".to_owned())?;
        if joined_objects != expected_objects {
            return Err(format!(
                "configured static-prop VHV join count differs: expected={expected_objects} actual={joined_objects}"
            ));
        }
        let aggregate = encode_static_prop_vhv_aggregate(&aggregate_records)?;
        let decoded = decode_static_prop_vhv_aggregate(&aggregate)?;
        if decoded != aggregate_records {
            return Err("static-prop VHV aggregate round trip differs".to_owned());
        }
        resolver.pack_sources(&packed_paths)?;
        if diagnose_presentation_bound {
            diagnostic_report = Some(json!({
                "schema": "playsrc-static-prop-producer-diagnostic-v1",
                "target": target,
                "contentBuild": contract.content_build,
                "mapSha256": map_sha256,
                "dictionaryModels": canonical.static_props.models.len(),
                "leafReferences": canonical.static_props.leaf_reference_count,
                "occurrences": canonical.static_props.occurrences.len(),
                "modelDocuments": model_documents.len(),
                "vhvObjects": joined_objects,
                "vhvDistinctByteIdentities": vhv_hashes.len(),
                "vhvBytes": vhv_bytes.to_string(),
                "vhvAggregateBytes": aggregate.len().to_string(),
                "vhvAggregateSha256": digest(&aggregate),
                "joinedMeshes": joined_meshes,
                "joinedVertices": joined_vertices.to_string(),
            }));
        }
        Some(aggregate)
    };
    stage("map-materials-and-models", &mut stage_started);
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
    stage("particles", &mut stage_started);
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
                        provider_revision: contract.content_build.clone(),
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
    stage("declared-dependencies", &mut stage_started);
    #[cfg(not(feature = "presentation-bound-diagnostic"))]
    if diagnostic_report.is_some() {
        return Err("presentation-bound diagnostic feature is unavailable".to_owned());
    }
    #[cfg(feature = "presentation-bound-diagnostic")]
    if let Some(mut report) = diagnostic_report {
        if let Some(bytes) = &static_prop_vhv_aggregate {
            resolver
                .bundle
                .insert(STATIC_PROP_VHV_AGGREGATE_PATH.to_owned(), bytes.clone());
        }
        let presentation = playsrc_tf2_wasm::diagnose_presentation_bound(
            &bsp_bytes,
            &resolver.bundle,
            &model_documents.keys().cloned().collect::<Vec<_>>(),
        )
        .map_err(|error| format!("presentation-bound compilation failed with error {error}"))?;
        report["presentation"] = json!({
            "modelCount": presentation.model_count,
            "modelVertices": presentation.model_vertices.to_string(),
            "modelTriangles": presentation.model_triangles.to_string(),
            "decodedTextureCount": presentation.decoded_texture_count,
            "distinctDecodedTextureCount": presentation.distinct_decoded_texture_count,
            "decodedTextureBytes": presentation.decoded_texture_bytes.to_string(),
            "uniqueDecodedTextureBytes": presentation.unique_decoded_texture_bytes.to_string(),
            "repeatedDecodedTextureBytes": presentation.repeated_decoded_texture_bytes.to_string(),
            "sourceTextureCount": presentation.source_texture_count,
            "distinctSourceTextureCount": presentation.distinct_source_texture_count,
            "sourceTextureBytes": presentation.source_texture_bytes.to_string(),
            "uniqueSourceTextureBytes": presentation.unique_source_texture_bytes.to_string(),
            "sectionEnds": presentation.section_ends.map(|value| value.to_string()),
            "defaultBoundFirstExceededAt": presentation.default_bound_first_exceeded_at.map(|value| value.to_string()),
            "finalLength": presentation.final_length.to_string(),
            "finalCapacity": presentation.final_capacity.to_string(),
            "diagnosticLimit": (512usize * 1024 * 1024).to_string(),
            "phaseMilliseconds": presentation.phase_milliseconds.map(|value| value.to_string()),
            "displacementInputCount": presentation.displacement_input_count,
            "staticPropCollisionCount": presentation.static_prop_collision_count,
            "staticPropOccurrenceCount": presentation.static_prop_occurrence_count,
            "staticPropVhvObjectCount": presentation.static_prop_vhv_object_count,
            "staticPropRuntimeLightingCount": presentation.static_prop_runtime_lighting_count,
            "staticPropSectionBytes": presentation.static_prop_section_bytes.to_string(),
            "staticPropSectionSha256": hex(&presentation.static_prop_section_sha256),
            "staticPropRuntimeSources": presentation.static_prop_runtime_sources,
            "staticPropRuntimeLights": presentation.static_prop_runtime_light_records.iter().map(|(prop, light, style)| json!({"prop":prop,"light":light,"style":style})).collect::<Vec<_>>(),
            "staticPropMainCount": presentation.static_prop_main_count,
            "staticPropSkyCount": presentation.static_prop_sky_count,
            "staticPropVertexLightingCount": presentation.static_prop_vertex_lighting_count,
        });
        println!(
            "{}",
            serde_json::to_string(&report).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    let chapter_path = "scripts/chapterbackgrounds.txt";
    let chapter_bytes = resolver.required(chapter_path, "tf2-gameui-base-background-list")?;
    let chapter_document = playsrc_keyvalues::parse_text(
        &chapter_bytes,
        playsrc_keyvalues::EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| error.to_string())?
    .evaluated(&ConditionEnvironment::default());
    let chapter_root = chapter_document
        .roots
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(b"ChapterBackgrounds"))
        .or_else(|| chapter_document.roots.first())
        .ok_or_else(|| "TF2 ChapterBackgrounds root is missing".to_owned())?;
    let background_name = std::str::from_utf8(scalar(chapter_root, b"1")?)
        .map_err(|_| "TF2 chapter background name is not UTF-8".to_owned())?
        .to_ascii_lowercase();
    if background_name.is_empty()
        || background_name.len() > 128
        || background_name.starts_with('/')
        || background_name.contains("..")
        || background_name.bytes().any(|byte| {
            !byte.is_ascii_lowercase()
                && !byte.is_ascii_digit()
                && byte != b'_'
                && byte != b'/'
                && byte != b'-'
        })
    {
        return Err("TF2 chapter background name is malformed".to_owned());
    }
    let gameui_backgrounds = [
        (
            "standard",
            format!("../console/{background_name}"),
            format!("materials/console/{background_name}.vmt"),
        ),
        (
            "widescreen",
            format!("../console/{background_name}_widescreen"),
            format!("materials/console/{background_name}_widescreen.vmt"),
        ),
    ];
    let mut ui_materials = Vec::new();
    for image in &tf2_ui.images {
        let Some(identity) = image.material.as_deref() else {
            continue;
        };
        let material = resolve_ui_material(&mut resolver, identity)?;
        let material_sha256 = digest(
            resolver
                .bundle
                .get(identity)
                .ok_or_else(|| format!("TF2 UI material source is missing: {identity}"))?,
        );
        let record = ui_material_record(
            &image.configured_value,
            identity,
            &material_sha256,
            &material,
        )?;
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
    runtime_ui_materials.extend(
        tf2_ui
            .dynamic_images
            .iter()
            .map(|image| (image.configured_value.clone(), image.material.clone())),
    );
    if !tf2_ui.images.iter().any(|image| {
        image
            .configured_value
            .eq_ignore_ascii_case("stamp_background_map")
    }) {
        runtime_ui_materials.push((
            "stamp_background_map".to_owned(),
            "materials/vgui/stamp_background_map.vmt".to_owned(),
        ));
    }
    runtime_ui_materials.extend(
        gameui_backgrounds
            .iter()
            .map(|(_, configured, material)| (configured.clone(), material.clone())),
    );
    for (configured_value, identity) in runtime_ui_materials {
        let material = resolve_ui_material(&mut resolver, &identity)?;
        let material_sha256 = digest(
            resolver
                .bundle
                .get(&identity)
                .ok_or_else(|| format!("TF2 UI material source is missing: {identity}"))?,
        );
        let record = ui_material_record(&configured_value, &identity, &material_sha256, &material)?;
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
        images: ui_materials.clone(),
        textures: texture_records,
    })
    .map_err(|error| error.to_string())?;
    let background_variants = gameui_backgrounds
        .iter()
        .map(|(aspect, configured_value, material)| {
            let record = ui_materials
                .iter()
                .find(|record| record.configured_value == *configured_value)
                .ok_or_else(|| format!("TF2 GameUI background material is missing: {material}"))?;
            let material_bytes = resolver
                .bundle
                .get(material)
                .ok_or_else(|| format!("TF2 GameUI background source is missing: {material}"))?;
            let (texture_sha256, width, height, _, _) =
                ui_textures.get(&record.base_texture).ok_or_else(|| {
                    format!(
                        "TF2 GameUI background texture is missing: {}",
                        record.base_texture
                    )
                })?;
            Ok(Tf2GameUiBackgroundVariant {
                aspect,
                configured_value: configured_value.clone(),
                material: material.clone(),
                material_sha256: digest(material_bytes),
                texture: record.base_texture.clone(),
                texture_sha256: texture_sha256.clone(),
                width: *width,
                height: *height,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let gameui_background_bytes = serde_json::to_vec(&Tf2GameUiBackgroundSet {
        schema: "playsrc-tf2-gameui-background-v1",
        content_build: contract.content_build.clone(),
        chapter_source: Tf2GameUiBackgroundSource {
            logical_path: chapter_path.to_owned(),
            byte_length: chapter_bytes.len(),
            sha256: digest(&chapter_bytes),
        },
        default_chapter: 1,
        background_name: background_name.clone(),
        variants: background_variants,
    })
    .map_err(|error| error.to_string())?;
    stage("ui-materials", &mut stage_started);
    let mut ui_bundle = BTreeMap::<String, Vec<u8>>::new();
    if let Some(bytes) = static_prop_vhv_aggregate {
        ui_bundle.insert(STATIC_PROP_VHV_AGGREGATE_PATH.to_owned(), bytes);
    }
    ui_bundle.extend(startup_presentation);
    ui_bundle.insert(
        "playsrc/tf2-ui/materials.json".to_owned(),
        ui_material_bytes,
    );
    ui_bundle.insert(
        "playsrc/tf2-gameui-background.json".to_owned(),
        gameui_background_bytes,
    );
    let textures = ui_textures.into_iter().collect::<Vec<_>>();
    if textures.is_empty() {
        return Err("TF2 UI texture set is empty".to_owned());
    }
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(MAX_UI_PNG_WORKERS)
        .min(textures.len());
    let chunk_size = textures.len().div_ceil(worker_count);
    let source_bundle = &resolver.bundle;
    let rendered = std::thread::scope(|scope| {
        let handles = textures
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    let mut output = Vec::new();
                    for (logical_path, (sha256, width, height, frames, _raw_flags)) in chunk {
                        let bytes = source_bundle.get(logical_path).ok_or_else(|| {
                            format!("TF2 UI texture is absent from bundle: {logical_path}")
                        })?;
                        if digest(bytes) != *sha256 {
                            return Err(format!(
                                "TF2 UI texture changed before presentation: {logical_path}"
                            ));
                        }
                        for frame in 0..*frames {
                            let (decoded_width, decoded_height, png) = tf2_ui_png(bytes, frame)
                                .map_err(|error| {
                                    format!("TF2 UI texture {logical_path} frame {frame}: {error}")
                                })?;
                            if decoded_width != *width || decoded_height != *height {
                                return Err(format!(
                                    "TF2 UI texture dimensions changed: {logical_path}"
                                ));
                            }
                            output.push((format!("playsrc/tf2-ui/png/{sha256}/{frame}.png"), png));
                        }
                    }
                    Ok::<_, String>(output)
                })
            })
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        for handle in handles {
            output.extend(
                handle
                    .join()
                    .map_err(|_| "TF2 UI texture worker panicked".to_owned())??,
            );
        }
        Ok::<_, String>(output)
    })?;
    for (logical_path, bytes) in rendered {
        if let Some(prior) = ui_bundle.get(&logical_path) {
            if prior != &bytes {
                return Err(format!(
                    "conflicting TF2 UI presentation identity: {logical_path}"
                ));
            }
        } else {
            ui_bundle.insert(logical_path, bytes);
        }
    }
    stage("ui-png", &mut stage_started);
    let gameui_presentation_sources = BTreeSet::from([
        chapter_path.to_owned(),
        format!("materials/console/{background_name}.vmt"),
        format!("materials/console/{background_name}.vtf"),
        format!("materials/console/{background_name}_widescreen.vmt"),
        format!("materials/console/{background_name}_widescreen.vtf"),
    ]);
    let bundle = &resolver.bundle;
    if bundle.len() > MAX_DEPENDENCY_REQUESTS || resolver.requests.len() > MAX_DEPENDENCY_REQUESTS {
        return Err("source dependency request count exceeds bound".to_owned());
    }
    let request_records = resolver.records();

    let mut resources = Vec::with_capacity(bundle.len() + ui_bundle.len());
    for (logical_path, bytes) in bundle {
        let request = resolver
            .requests
            .get(logical_path)
            .ok_or_else(|| format!("resource graph request is absent: {logical_path}"))?;
        let mut roles = BTreeSet::new();
        if gameui_presentation_sources.contains(logical_path) {
            roles.insert("menu".to_owned());
        }
        for consumer in &request.consumers {
            if consumer.starts_with("tf2-ui")
                || consumer.starts_with("tf2-gameui")
                || consumer.starts_with("vgui-")
            {
                roles.insert("menu".to_owned());
            } else if !gameui_presentation_sources.contains(logical_path) {
                roles.insert("gameplay".to_owned());
            }
        }
        if roles.is_empty() {
            return Err(format!("resource graph roles are absent: {logical_path}"));
        }
        resources.push(Resource {
            logical_path: logical_path.clone(),
            roles,
            bytes: bytes.clone(),
        });
    }
    for (logical_path, bytes) in &ui_bundle {
        resources.push(Resource {
            logical_path: logical_path.clone(),
            roles: BTreeSet::from([if logical_path == STATIC_PROP_VHV_AGGREGATE_PATH {
                "gameplay".to_owned()
            } else if logical_path == "media/startupvids.txt" || logical_path == "media/valve.webm"
            {
                "startup".to_owned()
            } else {
                "menu".to_owned()
            }]),
            bytes: bytes.clone(),
        });
    }
    let graph_entries = resources.len();
    let packed = playsrc_asset_graph::pack(resources)
        .map_err(|error| format!("resource graph packing failed: {error:?}"))?;
    let graph_encoded_bytes = packed
        .iter()
        .map(|chunk| chunk.encoded.len())
        .sum::<usize>();
    let graph = ResourceGraph {
        schema: "playsrc-resource-graph-v1".to_owned(),
        game: "tf2".to_owned(),
        content_build: contract.content_build.clone(),
        target: target.clone(),
        chunks: packed
            .iter()
            .map(|chunk| chunk.descriptor.clone())
            .collect(),
    };
    let graph_bytes = playsrc_asset_graph::canonical_json(&graph)
        .map_err(|error| format!("resource graph encoding failed: {error:?}"))?;
    let graph_descriptor = ObjectDescriptor::new("source-root", GRAPH_MEDIA_TYPE, &graph_bytes);
    let authoritative_absences = request_records
        .iter()
        .filter(|record| record.outcome == "authoritative-absence")
        .count();
    let resolved_entries = request_records
        .iter()
        .filter(|record| record.outcome == "resolved")
        .count();
    if resolved_entries != bundle.len() + resolver.packed_sources.len() {
        return Err("resolved ledger identities differ from bundle entries".to_owned());
    }
    for request in request_records
        .iter()
        .filter(|record| record.outcome == "resolved")
    {
        let descriptor = request
            .descriptor
            .as_ref()
            .ok_or_else(|| "resolved ledger descriptor is absent".to_owned())?;
        let actual = if let Some(bytes) = bundle.get(&request.logical_path) {
            ObjectDescriptor::source(bytes)
        } else {
            resolver
                .packed_sources
                .get(&request.logical_path)
                .cloned()
                .ok_or_else(|| {
                    "resolved ledger entry is absent from bundle and aggregate".to_owned()
                })?
        };
        if descriptor != &actual {
            return Err("resolved ledger descriptor differs from bundle bytes".to_owned());
        }
    }
    let ledger = DependencyLedger {
        schema: "playsrc-source-dependency-ledger-v1",
        game: "tf2",
        app_id: contract.app_id.clone(),
        content_build: contract.content_build.clone(),
        patch_version: contract.patch_version.clone(),
        installed_depots: contract.installed_depots.clone(),
        target: target.clone(),
        gameinfo_sha256: contract.gameinfo_sha256.clone(),
        map: MapSourceRecord {
            logical_path: map_target.logical_path.clone(),
            encoded: map_target
                .download
                .as_ref()
                .map(|download| EncodedMapSource {
                    url: download.url.clone(),
                    byte_length: download.encoded_byte_length.to_string(),
                    sha256: download.encoded_sha256.clone(),
                    compression: "bzip2",
                }),
            provider: map_target
                .installed
                .as_ref()
                .map(|installed| installed.provider.clone()),
            decoded: ObjectDescriptor {
                kind: "source-object",
                media_type: SOURCE_MEDIA_TYPE,
                byte_length: bsp_bytes.len().to_string(),
                sha256: map_sha256.clone(),
            },
            cache_path: format!("objects/sha256/{}/{}", &map_sha256[..2], map_sha256),
        },
        startup_sources,
        providers: provider_records,
        requests: request_records,
        static_prop_vhv_aggregate: ui_bundle.get(STATIC_PROP_VHV_AGGREGATE_PATH).map(|bytes| {
            StaticPropVhvAggregateDescriptor {
                logical_path: STATIC_PROP_VHV_AGGREGATE_PATH,
                object_count: canonical
                    .static_props
                    .occurrences
                    .iter()
                    .filter(|prop| {
                        prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING == 0
                    })
                    .count()
                    * 2,
                descriptor: ObjectDescriptor::new("derived-object", SOURCE_MEDIA_TYPE, bytes),
            }
        }),
        resolved_entries,
        authoritative_absences,
        resource_graph: graph_descriptor.clone(),
    };
    let ledger_bytes = serde_json::to_vec(&ledger).map_err(|error| error.to_string())?;
    if ledger_bytes.len() > MAX_LEDGER_BYTES {
        return Err("source dependency ledger exceeds byte bound".to_owned());
    }
    let ledger_descriptor =
        ObjectDescriptor::new("derived-object", LEDGER_MEDIA_TYPE, &ledger_bytes);
    let directory = cache.join("browser-bundles");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let ledger_destination = directory.join(format!("{target}.dependencies.json"));
    let graph_destination = directory.join(format!("{target}.graph.json"));
    let graph_object_directory = directory.join(format!("{target}.graph/objects"));
    install_artifact(&ledger_destination, &ledger_bytes)?;
    install_artifact(&graph_destination, &graph_bytes)?;
    fs::create_dir_all(&graph_object_directory).map_err(|error| error.to_string())?;
    for chunk in &packed {
        install_artifact(
            &graph_object_directory.join(&chunk.descriptor.encoded_sha256),
            &chunk.encoded,
        )?;
    }
    stage("serialize-and-install", &mut stage_started);
    #[allow(unused_mut)]
    let mut report = BuildReport {
        target: target.clone(),
        content_build: contract.content_build.clone(),
        providers: ledger.providers.len(),
        requests: ledger.resolved_entries + ledger.authoritative_absences,
        authoritative_absences: ledger.authoritative_absences,
        entries: bundle.len(),
        packed_entries: resolver.packed_sources.len(),
        derived_entries: ui_bundle.len(),
        graph_entries,
        graph_chunks: packed.len(),
        graph_encoded_bytes,
        graph_descriptor,
        ledger_bytes: ledger_bytes.len(),
        ledger_sha256: ledger_descriptor.sha256.clone(),
        ledger_descriptor,
        native_hdr_bytes: None,
        native_hdr_sha256: None,
        native_hdr_derived_sha256: None,
    };
    if verify_hdr {
        #[cfg(not(feature = "verify-hdr"))]
        return Err("source bundle HDR verification feature is unavailable".to_owned());
        #[cfg(feature = "verify-hdr")]
        {
            let gameplay = packed
                .iter()
                .filter(|chunk| chunk.descriptor.roles.iter().any(|role| role == "gameplay"))
                .cloned()
                .collect::<Vec<_>>();
            let batch = playsrc_asset_graph::encode_batch(&gameplay)
                .map_err(|error| format!("native HDR resource batch failed: {error:?}"))?;
            let resources = playsrc_asset_graph::decode_to_resource_set(&batch)
                .map_err(|error| format!("native HDR resource decoding failed: {error:?}"))?;
            let artifact = playsrc_tf2_wasm::compile_artifact(&bsp_bytes, 1, &resources)
                .map_err(|error| format!("native HDR compilation failed with error {error}"))?;
            let native_destination = directory.join(format!("{target}.native-hdr.psmp"));
            install_artifact(&native_destination, &artifact.payload)?;
            report.native_hdr_bytes = Some(artifact.payload.len());
            report.native_hdr_sha256 = Some(digest(&artifact.payload));
            report.native_hdr_derived_sha256 = Some(hex(&artifact.derived_sha256));
        }
    }
    eprintln!(
        "source-bundle stage=total milliseconds={}",
        started.elapsed().as_millis()
    );
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

#[cfg(any(feature = "verify-hdr", feature = "presentation-bound-diagnostic"))]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_install_manifest_is_inside_the_configured_install() {
        assert_eq!(
            app_manifest_path(Path::new("force-install")),
            PathBuf::from("force-install/steamapps/appmanifest_440.acf")
        );
    }

    #[test]
    fn standard_steam_manifest_is_beside_the_common_directory() {
        assert_eq!(
            app_manifest_path(Path::new("steamapps/common/Team Fortress 2")),
            PathBuf::from("steamapps/appmanifest_440.acf")
        );
    }

    #[test]
    fn static_prop_vhv_aggregate_round_trips_and_rejects_trailing_bytes() {
        let source = vec![1, 2, 3, 4];
        let sha256: [u8; 32] = Sha256::digest(&source).into();
        let records = vec![StaticPropVhvRecord {
            occurrence: 7,
            model: 3,
            profile: 1,
            logical_path: "sp_hdr_7.vhv".to_owned(),
            source_sha256: sha256,
            parsed_sha256: sha256,
            join_sha256: [9; 32],
            mesh_count: 1,
            vertex_count: 17,
            meshes: vec![StaticPropVhvMeshRecord {
                primitive: 0,
                body_part: 0,
                model: 0,
                lod: 0,
                mesh: 0,
                strip_group: 0,
                vertex_count: 1,
                encoded_bgra_start: 0,
                encoded_bgra_end: 4,
            }],
            bytes: source,
        }];
        let encoded = encode_static_prop_vhv_aggregate(&records).unwrap();
        assert_eq!(decode_static_prop_vhv_aggregate(&encoded).unwrap(), records);
        let mut malformed = encoded;
        malformed.push(0);
        assert!(decode_static_prop_vhv_aggregate(&malformed).is_err());
    }

    #[test]
    fn static_prop_vhv_aggregate_requires_source_profile_order() {
        let source = vec![1];
        let sha256: [u8; 32] = Sha256::digest(&source).into();
        let record = |profile| StaticPropVhvRecord {
            occurrence: 1,
            model: 0,
            profile,
            logical_path: format!("sp_{profile}.vhv"),
            source_sha256: sha256,
            parsed_sha256: sha256,
            join_sha256: [profile; 32],
            mesh_count: 1,
            vertex_count: 1,
            meshes: vec![StaticPropVhvMeshRecord {
                primitive: 0,
                body_part: 0,
                model: 0,
                lod: 0,
                mesh: 0,
                strip_group: 0,
                vertex_count: 1,
                encoded_bgra_start: 0,
                encoded_bgra_end: 1,
            }],
            bytes: source.clone(),
        };
        assert!(encode_static_prop_vhv_aggregate(&[record(1), record(0)]).is_err());
    }
}
