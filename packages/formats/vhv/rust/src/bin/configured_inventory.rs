use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs::{self, File},
    io::{Cursor, Read, Seek, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
};

use playsrc_bsp::{Limits as BspLimits, Profile as BspProfile};
use playsrc_vhv::{Limits, Profile, parse};
use playsrc_vpk::{
    Archive, Layout, SegmentReader, SourceError, SourceErrorCode, parse as parse_vpk,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CONFIG_PATH: &str = "playsrc.local.json";
const LEDGER_RELATIVE_PATH: &str = "browser-bundles/pl_upward.dependencies.json";
const STATIC_PROP_LUMP: usize = 35;
const PAK_LUMP: usize = 40;
const STATIC_PROP_RECORD_BYTES: usize = 72;
const NO_PER_VERTEX_LIGHTING: u32 = 0x40;

type ModelRead = Option<(Vec<u8>, String)>;

#[derive(Clone, Debug)]
struct Config {
    tf2_dir: PathBuf,
    source_cache_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct StaticProps {
    model_paths: Vec<String>,
    occurrences: Vec<StaticPropOccurrence>,
}

#[derive(Clone, Copy, Debug)]
struct StaticPropOccurrence {
    model_index: usize,
    flags: u32,
}

struct LocalSegments {
    directory: PathBuf,
    stem: String,
}

impl SegmentReader for LocalSegments {
    fn len(&self, archive_index: u32) -> Result<u64, SourceError> {
        fs::metadata(self.path(archive_index))
            .map(|metadata| metadata.len())
            .map_err(|_| source_error(SourceErrorCode::Missing, 0..0))
    }

    fn read(&self, archive_index: u32, range: Range<u64>) -> Result<Vec<u8>, SourceError> {
        let mut file = File::open(self.path(archive_index))
            .map_err(|_| source_error(SourceErrorCode::Missing, range.clone()))?;
        file.seek(SeekFrom::Start(range.start))
            .map_err(|_| source_error(SourceErrorCode::Io, range.clone()))?;
        let length = range
            .end
            .checked_sub(range.start)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| source_error(SourceErrorCode::Io, range.clone()))?;
        let mut bytes = vec![0_u8; length];
        file.read_exact(&mut bytes)
            .map_err(|_| source_error(SourceErrorCode::ShortRead, range))?;
        Ok(bytes)
    }
}

impl LocalSegments {
    fn path(&self, archive_index: u32) -> PathBuf {
        self.directory
            .join(format!("{}_{archive_index:03}.vpk", self.stem))
    }
}

struct VpkProvider {
    identity: String,
    archive: Archive,
    segments: LocalSegments,
}

#[derive(Clone, Debug)]
struct ModelIdentity {
    checksum: u32,
    provider_identity: String,
}

#[derive(Clone, Debug)]
struct InventoryRow {
    logical_path: String,
    byte_length: usize,
    sha256: String,
    checksum: u32,
    total_vertices: u32,
    mesh_count: usize,
    min_lod: u32,
    max_lod: u32,
    min_mesh_vertices: u32,
    max_mesh_vertices: u32,
    min_mesh_offset: usize,
    max_mesh_offset: usize,
    static_prop_index: usize,
    model_path: String,
    model_provider_identity: String,
    lighting_profile: &'static str,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = read_config(Path::new(CONFIG_PATH))?;
    let ledger_path = config.source_cache_dir.join(LEDGER_RELATIVE_PATH);
    let ledger_bytes = fs::read(&ledger_path)?;
    let ledger: Value = serde_json::from_slice(&ledger_bytes)?;
    require_string(&ledger, "schema", "playsrc-source-dependency-ledger-v1")?;
    require_string(&ledger, "target", "pl_upward")?;
    let content_build = string_field(&ledger, "contentBuild")?.to_owned();
    let patch_version = string_field(&ledger, "patchVersion")?.to_owned();
    let map = object_field(&ledger, "map")?;
    let map_cache_path = string_field(map, "cachePath")?;
    let map_sha256 = string_field(object_field(map, "decoded")?, "sha256")?;
    let map_bytes = fs::read(config.source_cache_dir.join(map_cache_path))?;
    require_sha256(&map_bytes, map_sha256, "configured map")?;

    let bsp = playsrc_bsp::parse(&map_bytes, BspProfile::Source2013V20, BspLimits::default())?;
    let pak = bsp
        .lump(PAK_LUMP)
        .and_then(|lump| lump.pak.as_ref())
        .ok_or("configured map has no parsed BSP PAK")?;
    let mut pak_entries = BTreeMap::<String, &[u8]>::new();
    for entry in &pak.entries {
        let name = std::str::from_utf8(&entry.raw_name)?.to_ascii_lowercase();
        let bytes = entry
            .decoded
            .as_deref()
            .ok_or_else(|| format!("unsupported BSP PAK entry {name}"))?;
        if pak_entries.insert(name.clone(), bytes).is_some() {
            return Err(format!("duplicate normalized BSP PAK path {name}").into());
        }
    }
    let vhv_paths: BTreeSet<_> = pak_entries
        .keys()
        .filter(|path| path.ends_with(".vhv"))
        .cloned()
        .collect();

    let static_props = parse_static_props(&map_bytes, &bsp)?;
    let provider_revisions = provider_revisions(&ledger)?;
    let hl2_dir = config
        .tf2_dir
        .parent()
        .ok_or("tf2Dir has no game-install parent")?
        .join("hl2");
    let provider_specs = [
        (
            config.tf2_dir.join("tf2_textures_dir.vpk"),
            "game-01-tf2_textures_dir.vpk",
        ),
        (
            config.tf2_dir.join("tf2_sound_vo_english_dir.vpk"),
            "game-02-tf2_sound_vo_english_dir.vpk",
        ),
        (
            config.tf2_dir.join("tf2_sound_misc_dir.vpk"),
            "game-03-tf2_sound_misc_dir.vpk",
        ),
        (
            config.tf2_dir.join("tf2_misc_dir.vpk"),
            "game-04-tf2_misc_dir.vpk",
        ),
        (
            hl2_dir.join("hl2_textures_dir.vpk"),
            "game-05-hl2_textures_dir.vpk",
        ),
        (
            hl2_dir.join("hl2_sound_vo_english_dir.vpk"),
            "game-06-hl2_sound_vo_english_dir.vpk",
        ),
        (
            hl2_dir.join("hl2_sound_misc_dir.vpk"),
            "game-07-hl2_sound_misc_dir.vpk",
        ),
        (hl2_dir.join("hl2_misc_dir.vpk"), "game-08-hl2_misc_dir.vpk"),
    ];
    let mut vpk_providers = Vec::with_capacity(provider_specs.len());
    for (path, identity) in provider_specs {
        vpk_providers.push(load_vpk_provider(
            &path,
            identity,
            provider_revisions
                .get(identity)
                .ok_or_else(|| format!("missing provider record {identity}"))?,
        )?);
    }

    let mut model_identities = Vec::with_capacity(static_props.model_paths.len());
    for model_path in &static_props.model_paths {
        let (bytes, provider_identity) = resolve_model(
            model_path,
            &config.tf2_dir,
            &hl2_dir,
            &pak_entries,
            &vpk_providers,
        )?;
        if bytes.len() < 12 || &bytes[..4] != b"IDST" {
            return Err(format!("static-prop model is not an IDST MDL: {model_path}").into());
        }
        model_identities.push(ModelIdentity {
            checksum: u32::from_le_bytes(bytes[8..12].try_into()?),
            provider_identity,
        });
    }

    let mut expected_vhv_paths = BTreeSet::new();
    let mut rows = Vec::new();
    let mut unlit_occurrences = 0_usize;
    for (static_prop_index, occurrence) in static_props.occurrences.iter().enumerate() {
        if occurrence.flags & NO_PER_VERTEX_LIGHTING != 0 {
            unlit_occurrences += 1;
            continue;
        }
        let model_path = &static_props.model_paths[occurrence.model_index];
        let model = &model_identities[occurrence.model_index];
        for (logical_path, lighting_profile) in [
            (format!("sp_{static_prop_index}.vhv"), "ldr"),
            (format!("sp_hdr_{static_prop_index}.vhv"), "hdr"),
        ] {
            expected_vhv_paths.insert(logical_path.clone());
            let bytes = pak_entries
                .get(&logical_path)
                .ok_or_else(|| format!("missing expected BSP PAK VHV {logical_path}"))?;
            let limits = source_derived_limits(bytes.len());
            let vhv = parse(
                bytes,
                Profile::source_pc_v2_color_bgra8888(model.checksum),
                limits,
            )?;
            let min_lod = vhv.meshes.iter().map(|mesh| mesh.lod).min().unwrap_or(0);
            let max_lod = vhv.meshes.iter().map(|mesh| mesh.lod).max().unwrap_or(0);
            let min_mesh_vertices = vhv
                .meshes
                .iter()
                .map(|mesh| mesh.vertex_count)
                .min()
                .unwrap_or(0);
            let max_mesh_vertices = vhv
                .meshes
                .iter()
                .map(|mesh| mesh.vertex_count)
                .max()
                .unwrap_or(0);
            let min_mesh_offset = vhv
                .meshes
                .iter()
                .map(|mesh| mesh.data_range.start)
                .min()
                .unwrap_or(vhv.header_padding_range.end);
            let max_mesh_offset = vhv
                .meshes
                .iter()
                .map(|mesh| mesh.data_range.start)
                .max()
                .unwrap_or(vhv.header_padding_range.end);
            rows.push(InventoryRow {
                logical_path,
                byte_length: bytes.len(),
                sha256: hex(&vhv.source_identity.sha256),
                checksum: vhv.header.checksum,
                total_vertices: vhv.header.total_vertices,
                mesh_count: vhv.meshes.len(),
                min_lod,
                max_lod,
                min_mesh_vertices,
                max_mesh_vertices,
                min_mesh_offset,
                max_mesh_offset,
                static_prop_index,
                model_path: model_path.clone(),
                model_provider_identity: model.provider_identity.clone(),
                lighting_profile,
            });
        }
    }
    if expected_vhv_paths != vhv_paths {
        let missing: Vec<_> = expected_vhv_paths.difference(&vhv_paths).cloned().collect();
        let extra: Vec<_> = vhv_paths.difference(&expected_vhv_paths).cloned().collect();
        return Err(format!("VHV closure mismatch: missing={missing:?}, extra={extra:?}").into());
    }
    rows.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));

    let inventory_identity = inventory_identity(&rows);
    let mut byte_identities = BTreeMap::<&str, usize>::new();
    for row in &rows {
        *byte_identities.entry(&row.sha256).or_default() += 1;
    }
    let duplicate_groups = byte_identities.values().filter(|count| **count > 1).count();
    let repeated_objects: usize = byte_identities
        .values()
        .filter(|count| **count > 1)
        .map(|count| count - 1)
        .sum();
    let total_bytes: usize = rows.iter().map(|row| row.byte_length).sum();
    let bounds = bounds(&rows)?;
    let row_values: Vec<_> = rows
        .iter()
        .map(|row| {
            json!({
                "byteLength": row.byte_length.to_string(),
                "checksum": row.checksum.to_string(),
                "lightingProfile": row.lighting_profile,
                "logicalPath": row.logical_path,
                "maxLod": row.max_lod,
                "maxMeshVertices": row.max_mesh_vertices,
                "maxMeshOffset": row.max_mesh_offset.to_string(),
                "meshCount": row.mesh_count,
                "minLod": row.min_lod,
                "minMeshVertices": row.min_mesh_vertices,
                "minMeshOffset": row.min_mesh_offset.to_string(),
                "modelPath": row.model_path,
                "modelProviderIdentity": row.model_provider_identity,
                "profile": "source-pc-v2-color-bgra8888",
                "sha256": row.sha256,
                "staticPropIndex": row.static_prop_index,
                "totalVertices": row.total_vertices,
            })
        })
        .collect();
    let report = json!({
        "bounds": bounds,
        "contentBuild": content_build,
        "inventoryIdentitySha256": inventory_identity,
        "map": {
            "byteLength": map_bytes.len().to_string(),
            "logicalPath": string_field(map, "logicalPath")?,
            "pakEntryCount": pak.entries.len(),
            "providerIdentity": string_field(map, "provider")?,
            "sha256": map_sha256,
        },
        "objects": {
            "byteLength": total_bytes.to_string(),
            "count": rows.len(),
            "distinctByteIdentityCount": byte_identities.len(),
            "duplicateByteIdentityGroupCount": duplicate_groups,
            "repeatedByteObjectCount": repeated_objects,
        },
        "patchVersion": patch_version,
        "profileFamilies": [{
            "count": rows.len(),
            "fileReservedWords": [0, 0, 0, 0],
            "profile": "source-pc-v2-color-bgra8888",
            "streamAlignment": 512,
            "vertexFlags": 4,
            "vertexSize": 4,
            "version": 2,
        }],
        "providerRevisions": provider_revisions,
        "rows": row_values,
        "schema": "playsrc-vhv-configured-inventory-v1",
        "staticProps": {
            "checksumJoinCount": rows.len(),
            "dictionaryCount": static_props.model_paths.len(),
            "noPerVertexLightingOccurrenceCount": unlit_occurrences,
            "occurrenceCount": static_props.occurrences.len(),
            "perVertexLightingOccurrenceCount": static_props.occurrences.len() - unlit_occurrences,
        },
    });
    let mut output = serde_json::to_vec_pretty(&report)?;
    output.push(b'\n');
    std::io::Write::write_all(&mut std::io::stdout().lock(), &output)?;
    Ok(())
}

fn read_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    let value: Value = serde_json::from_slice(&fs::read(path)?)?;
    let object = value
        .as_object()
        .ok_or("playsrc.local.json must be an object")?;
    let actual: BTreeSet<_> = object.keys().map(String::as_str).collect();
    let expected = BTreeSet::from(["assetDir", "sourceCacheDir", "tf2Dir"]);
    if actual != expected {
        return Err(
            "playsrc.local.json must contain exactly assetDir, sourceCacheDir, and tf2Dir".into(),
        );
    }
    let tf2_dir = absolute_existing_directory(string_field(&value, "tf2Dir")?, "tf2Dir")?;
    let source_cache_dir =
        absolute_existing_directory(string_field(&value, "sourceCacheDir")?, "sourceCacheDir")?;
    let _asset_dir = absolute_existing_directory(string_field(&value, "assetDir")?, "assetDir")?;
    Ok(Config {
        tf2_dir,
        source_cache_dir,
    })
}

fn absolute_existing_directory(value: &str, name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || !path.is_dir() {
        return Err(format!("{name} must be an existing absolute directory: {value}").into());
    }
    Ok(path)
}

fn provider_revisions(ledger: &Value) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut revisions = BTreeMap::new();
    for provider in ledger
        .get("providers")
        .and_then(Value::as_array)
        .ok_or("ledger providers must be an array")?
    {
        let identity = string_field(provider, "identity")?.to_owned();
        let revision = string_field(provider, "revision")?.to_owned();
        if revisions.insert(identity.clone(), revision).is_some() {
            return Err(format!("duplicate provider identity {identity}").into());
        }
    }
    Ok(revisions)
}

fn parse_static_props(
    map_bytes: &[u8],
    bsp: &playsrc_bsp::Bsp,
) -> Result<StaticProps, Box<dyn Error>> {
    let directory = bsp
        .lump(STATIC_PROP_LUMP)
        .ok_or("missing game-lump directory")?
        .bytes(bsp);
    if directory.len() < 4 {
        return Err("truncated game-lump directory".into());
    }
    let count = i32_at(directory, 0)?;
    if count < 0 || 4_usize.saturating_add(count as usize * 16) > directory.len() {
        return Err("invalid game-lump directory count".into());
    }
    let mut records = Vec::with_capacity(count as usize);
    for ordinal in 0..count as usize {
        let start = 4 + ordinal * 16;
        let id: [u8; 4] = directory[start..start + 4].try_into()?;
        let flags = u16_at(directory, start + 4)?;
        let version = u16_at(directory, start + 6)?;
        let offset = i32_at(directory, start + 8)?;
        let decoded_size = i32_at(directory, start + 12)?;
        if offset < 0 || decoded_size < 0 {
            return Err("negative game-lump child range".into());
        }
        records.push((id, flags, version, offset as usize, decoded_size as usize));
    }
    let record = records
        .iter()
        .find(|record| record.0 == *b"prps")
        .ok_or("missing static-prop game lump")?;
    if record.2 != 10 {
        return Err(format!("unsupported static-prop game-lump version {}", record.2).into());
    }
    let encoded_end = records
        .iter()
        .filter(|candidate| candidate.3 > record.3)
        .map(|candidate| candidate.3)
        .min()
        .unwrap_or(map_bytes.len());
    if encoded_end > map_bytes.len() || record.3 > encoded_end {
        return Err("invalid static-prop encoded range".into());
    }
    let encoded = &map_bytes[record.3..encoded_end];
    let decoded = if record.1 & 1 != 0 {
        decode_source_lzma(encoded, record.4)?
    } else {
        if encoded.len() != record.4 {
            return Err("uncompressed static-prop length mismatch".into());
        }
        encoded.to_vec()
    };
    parse_static_prop_payload(&decoded)
}

fn decode_source_lzma(encoded: &[u8], decoded_size: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    if encoded.len() < 17 || &encoded[..4] != b"LZMA" {
        return Err("invalid Source LZMA static-prop envelope".into());
    }
    let header_decoded = u32_at(encoded, 4)? as usize;
    let payload_size = u32_at(encoded, 8)? as usize;
    let envelope_end = 17_usize
        .checked_add(payload_size)
        .ok_or("Source LZMA static-prop size overflow")?;
    if header_decoded != decoded_size
        || envelope_end > encoded.len()
        || encoded[envelope_end..].iter().any(|value| *value != 0)
    {
        return Err("Source LZMA static-prop size mismatch".into());
    }
    let mut alone = Vec::with_capacity(13 + payload_size);
    alone.extend_from_slice(&encoded[12..17]);
    alone.extend_from_slice(&(decoded_size as u64).to_le_bytes());
    alone.extend_from_slice(&encoded[17..envelope_end]);
    let mut decoded = Vec::with_capacity(decoded_size);
    lzma_rs::lzma_decompress(&mut Cursor::new(alone), &mut decoded)?;
    if decoded.len() != decoded_size {
        return Err("Source LZMA static-prop decoded length mismatch".into());
    }
    Ok(decoded)
}

fn parse_static_prop_payload(bytes: &[u8]) -> Result<StaticProps, Box<dyn Error>> {
    let dictionary_count = nonnegative_i32_at(bytes, 0, "static-prop dictionary count")?;
    let mut cursor = 4_usize;
    let dictionary_bytes = dictionary_count
        .checked_mul(128)
        .and_then(|length| cursor.checked_add(length))
        .ok_or("static-prop dictionary range overflow")?;
    if dictionary_bytes > bytes.len() {
        return Err("truncated static-prop dictionary".into());
    }
    let mut model_paths = Vec::with_capacity(dictionary_count);
    for ordinal in 0..dictionary_count {
        let field = &bytes[cursor + ordinal * 128..cursor + (ordinal + 1) * 128];
        let nul = field
            .iter()
            .position(|value| *value == 0)
            .ok_or("unterminated static-prop model path")?;
        if field[nul + 1..].iter().any(|value| *value != 0) {
            return Err("nonzero static-prop model-path suffix".into());
        }
        let path = std::str::from_utf8(&field[..nul])?.to_ascii_lowercase();
        validate_logical_path(&path)?;
        model_paths.push(path);
    }
    cursor = dictionary_bytes;
    let leaf_count = nonnegative_i32_at(bytes, cursor, "static-prop leaf count")?;
    cursor = cursor
        .checked_add(4)
        .and_then(|value| value.checked_add(leaf_count.checked_mul(2)?))
        .ok_or("static-prop leaf range overflow")?;
    let occurrence_count = nonnegative_i32_at(bytes, cursor, "static-prop occurrence count")?;
    cursor = cursor
        .checked_add(4)
        .ok_or("static-prop occurrence range overflow")?;
    let end = occurrence_count
        .checked_mul(STATIC_PROP_RECORD_BYTES)
        .and_then(|length| cursor.checked_add(length))
        .ok_or("static-prop occurrence range overflow")?;
    if end != bytes.len() {
        return Err("static-prop occurrence extent mismatch".into());
    }
    let mut occurrences = Vec::with_capacity(occurrence_count);
    for ordinal in 0..occurrence_count {
        let start = cursor + ordinal * STATIC_PROP_RECORD_BYTES;
        let model_index = u16_at(bytes, start + 24)? as usize;
        if model_index >= model_paths.len() {
            return Err("static-prop model index out of range".into());
        }
        occurrences.push(StaticPropOccurrence {
            model_index,
            flags: u32_at(bytes, start + 64)?,
        });
    }
    Ok(StaticProps {
        model_paths,
        occurrences,
    })
}

fn load_vpk_provider(
    path: &Path,
    identity: &str,
    expected_sha256: &str,
) -> Result<VpkProvider, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    require_sha256(&bytes, expected_sha256, identity)?;
    let archive = parse_vpk(
        &bytes,
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or("non-UTF-8 VPK filename")?,
        Layout::Split,
        playsrc_vpk::Limits::default(),
    )?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("non-UTF-8 VPK filename")?;
    let stem = file_name
        .strip_suffix("_dir.vpk")
        .ok_or("split VPK filename does not end in _dir.vpk")?
        .to_owned();
    Ok(VpkProvider {
        identity: identity.to_owned(),
        archive,
        segments: LocalSegments {
            directory: path
                .parent()
                .ok_or("VPK directory has no parent")?
                .to_owned(),
            stem,
        },
    })
}

fn resolve_model(
    logical_path: &str,
    tf2_dir: &Path,
    hl2_dir: &Path,
    pak_entries: &BTreeMap<String, &[u8]>,
    vpk_providers: &[VpkProvider],
) -> Result<(Vec<u8>, String), Box<dyn Error>> {
    if let Some(bytes) = pak_entries.get(logical_path) {
        return Ok((bytes.to_vec(), "pl_upward-pak".to_owned()));
    }
    let workshop = [(tf2_dir.join("custom/workshop"), "game-00-workshop")];
    if let Some((bytes, identity)) = read_first_exact(logical_path, &workshop)? {
        return Ok((bytes, identity));
    }
    for provider in vpk_providers {
        match provider
            .archive
            .read_entry(logical_path, &provider.segments)
        {
            Ok(result) => return Ok((result.bytes, provider.identity.clone())),
            Err(error) if error.code == playsrc_vpk::ErrorCode::MissingEntry => {}
            Err(error) => return Err(error.into()),
        }
    }
    let trailing_directories = [
        (tf2_dir.to_owned(), "game-09-tf"),
        (hl2_dir.to_owned(), "game-10-hl2"),
        (tf2_dir.join("download"), "game-11-download"),
    ];
    if let Some((bytes, identity)) = read_first_exact(logical_path, &trailing_directories)? {
        return Ok((bytes, identity));
    }
    Err(format!("model missing from exact configured providers: {logical_path}").into())
}

fn read_first_exact(
    logical_path: &str,
    providers: &[(PathBuf, &str)],
) -> Result<ModelRead, Box<dyn Error>> {
    for (root, identity) in providers {
        let path = root.join(logical_path);
        match fs::read(path) {
            Ok(bytes) => return Ok(Some((bytes, (*identity).to_owned()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(None)
}

fn source_derived_limits(source_bytes: usize) -> Limits {
    Limits {
        max_input_bytes: source_bytes,
        max_retained_bytes: source_bytes.saturating_mul(2),
        max_meshes: source_bytes / playsrc_vhv::MESH_HEADER_BYTES,
        max_total_vertices: source_bytes / playsrc_vhv::VERTEX_BYTES,
        max_vertices_per_mesh: source_bytes / playsrc_vhv::VERTEX_BYTES,
        max_lod: u32::MAX as usize,
    }
}

fn inventory_identity(rows: &[InventoryRow]) -> String {
    let mut hasher = Sha256::new();
    for row in rows {
        hasher.update(row.logical_path.as_bytes());
        hasher.update(b"\t");
        hasher.update(row.byte_length.to_string().as_bytes());
        hasher.update(b"\t");
        hasher.update(row.sha256.as_bytes());
        hasher.update(b"\n");
    }
    hex(&hasher.finalize())
}

fn bounds(rows: &[InventoryRow]) -> Result<Value, Box<dyn Error>> {
    let first = rows.first().ok_or("configured VHV inventory is empty")?;
    let mut min_file = first.byte_length;
    let mut max_file = first.byte_length;
    let mut min_vertices = first.total_vertices;
    let mut max_vertices = first.total_vertices;
    let mut min_meshes = first.mesh_count;
    let mut max_meshes = first.mesh_count;
    let mut min_lod = first.min_lod;
    let mut max_lod = first.max_lod;
    let mut min_mesh_vertices = first.min_mesh_vertices;
    let mut max_mesh_vertices = first.max_mesh_vertices;
    let mut min_mesh_offset = first.min_mesh_offset;
    let mut max_mesh_offset = first.max_mesh_offset;
    for row in &rows[1..] {
        min_file = min_file.min(row.byte_length);
        max_file = max_file.max(row.byte_length);
        min_vertices = min_vertices.min(row.total_vertices);
        max_vertices = max_vertices.max(row.total_vertices);
        min_meshes = min_meshes.min(row.mesh_count);
        max_meshes = max_meshes.max(row.mesh_count);
        min_lod = min_lod.min(row.min_lod);
        max_lod = max_lod.max(row.max_lod);
        min_mesh_vertices = min_mesh_vertices.min(row.min_mesh_vertices);
        max_mesh_vertices = max_mesh_vertices.max(row.max_mesh_vertices);
        min_mesh_offset = min_mesh_offset.min(row.min_mesh_offset);
        max_mesh_offset = max_mesh_offset.max(row.max_mesh_offset);
    }
    Ok(json!({
        "fileBytes": {"min": min_file.to_string(), "max": max_file.to_string()},
        "lod": {"min": min_lod, "max": max_lod},
        "meshCount": {"min": min_meshes, "max": max_meshes},
        "meshOffset": {"min": min_mesh_offset.to_string(), "max": max_mesh_offset.to_string()},
        "meshVertices": {"min": min_mesh_vertices, "max": max_mesh_vertices},
        "totalVertices": {"min": min_vertices, "max": max_vertices},
    }))
}

fn validate_logical_path(path: &str) -> Result<(), Box<dyn Error>> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(format!("invalid logical path {path:?}").into());
    }
    Ok(())
}

fn require_sha256(bytes: &[u8], expected: &str, identity: &str) -> Result<(), Box<dyn Error>> {
    let actual = hex(&Sha256::digest(bytes));
    if actual != expected {
        return Err(
            format!("SHA-256 mismatch for {identity}: expected {expected}, got {actual}").into(),
        );
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for value in bytes {
        result.push(DIGITS[(value >> 4) as usize] as char);
        result.push(DIGITS[(value & 0x0f) as usize] as char);
    }
    result
}

fn require_string(value: &Value, key: &str, expected: &str) -> Result<(), Box<dyn Error>> {
    let actual = string_field(value, key)?;
    if actual != expected {
        return Err(format!("{key} must be {expected:?}, got {actual:?}").into());
    }
    Ok(())
}

fn object_field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .get(key)
        .filter(|field| field.is_object())
        .ok_or_else(|| format!("{key} must be an object").into())
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, Box<dyn Error>> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string").into())
}

fn nonnegative_i32_at(bytes: &[u8], offset: usize, name: &str) -> Result<usize, Box<dyn Error>> {
    let value = i32_at(bytes, offset)?;
    if value < 0 {
        return Err(format!("negative {name}").into());
    }
    Ok(value as usize)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, Box<dyn Error>> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or("truncated u16")?
            .try_into()?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Box<dyn Error>> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("truncated u32")?
            .try_into()?,
    ))
}

fn i32_at(bytes: &[u8], offset: usize) -> Result<i32, Box<dyn Error>> {
    Ok(i32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("truncated i32")?
            .try_into()?,
    ))
}

fn source_error(code: SourceErrorCode, range: Range<u64>) -> SourceError {
    SourceError { code, range }
}
