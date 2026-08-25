use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

use playsrc_bsp::{Pak, PakEntryClassification};
use playsrc_vpk::{
    ErrorCode as VpkErrorCode, Layout as VpkLayout, Limits as VpkLimits, SegmentReader,
    SourceError, SourceErrorCode,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_providers: usize,
    pub max_path_bytes: usize,
    pub max_read_bytes: usize,
    pub max_directory_file_bytes: usize,
    pub max_checked_locations: usize,
    pub max_supplement_objects: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_providers: 64,
            max_path_bytes: 1_024,
            max_read_bytes: 512 * 1024 * 1024,
            max_directory_file_bytes: 64 * 1024 * 1024,
            max_checked_locations: 66,
            max_supplement_objects: 65_536,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    BspPak,
    MapSupplement,
    Vpk,
    Directory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provenance {
    pub game: String,
    pub content_build: String,
    pub logical_path: String,
    pub provider_id: String,
    pub provider_kind: ProviderKind,
    pub provider_revision: String,
    pub location: String,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolved {
    pub bytes: Vec<u8>,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLocation {
    pub provider_id: String,
    pub provider_kind: ProviderKind,
    pub location: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolution {
    Found(Resolved),
    Missing {
        logical_path: String,
        checked: Vec<CheckedLocation>,
    },
}

/// Supplies one selected VPK family's immutable segment ranges. Implementors
/// verify each segment's declared complete-object identity before returning a
/// range; Content additionally verifies the assembled entry CRC through VPK.
/// A WASM compiler implements this interface over a response batch already
/// transferred into Rust memory, never as a per-read JavaScript callback.
pub trait SharedSegmentReader: SegmentReader + Send + Sync + fmt::Debug {}

impl<T: SegmentReader + Send + Sync + fmt::Debug> SharedSegmentReader for T {}

#[derive(Clone, Debug)]
pub enum ProviderSpec {
    Directory {
        id: String,
        revision: String,
        root: PathBuf,
    },
    Vpk {
        id: String,
        revision: String,
        directory_file: PathBuf,
        layout: VpkLayout,
    },
    IndexedVpk {
        id: String,
        revision: String,
        directory_identity: String,
        directory_bytes: Vec<u8>,
        layout: VpkLayout,
        segments: Arc<dyn SharedSegmentReader>,
    },
}

#[derive(Clone, Debug)]
pub enum SupplementBytes {
    Available(Vec<u8>),
    Missing,
}

#[derive(Clone, Debug)]
pub struct SupplementObject {
    pub logical_path: String,
    pub byte_length: usize,
    pub sha256: String,
    pub bytes: SupplementBytes,
}

#[derive(Clone, Debug)]
enum Provider {
    Directory(DirectoryProvider),
    Vpk(Box<VpkProvider>),
}

#[derive(Clone, Debug)]
struct DirectoryProvider {
    id: String,
    revision: String,
    root: PathBuf,
}

#[derive(Clone, Debug)]
struct VpkProvider {
    id: String,
    revision: String,
    archive: playsrc_vpk::Archive,
    segments: Arc<dyn SharedSegmentReader>,
}

#[derive(Clone, Debug)]
struct PakProvider {
    id: String,
    revision: String,
    map_identity: String,
    pak: Pak,
    lookup: BTreeMap<String, usize>,
}

#[derive(Clone, Debug)]
struct SupplementProvider {
    id: String,
    revision: String,
    lookup: BTreeMap<String, SupplementObject>,
}

#[derive(Clone, Debug)]
pub struct Content {
    game: String,
    content_build: String,
    providers: Arc<[Provider]>,
    active_pak: Option<Arc<PakProvider>>,
    supplement: Option<Arc<SupplementProvider>>,
    limits: Limits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidPlan,
    InvalidPath,
    ProviderLimit,
    ReadLimit,
    AmbiguousDirectory,
    EscapedDirectory,
    SelectedProviderIo,
    SelectedProviderMissing,
    SelectedProviderChanged,
    SelectedProviderCorrupt,
    SelectedProviderUnsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub logical_path: Option<String>,
    pub provider_id: Option<String>,
    pub operation: &'static str,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?} during {}", self.code, self.operation)
    }
}

impl std::error::Error for Error {}

impl Content {
    pub fn open(
        game: impl Into<String>,
        content_build: impl Into<String>,
        specs: Vec<ProviderSpec>,
        limits: Limits,
    ) -> Result<Self, Error> {
        let game = game.into();
        let content_build = content_build.into();
        if !valid_identity(&game)
            || !valid_identity(&content_build)
            || specs.len() > limits.max_providers
            || limits.max_checked_locations < specs.len() + 2
        {
            return Err(plan_error(ErrorCode::InvalidPlan, "validate plan"));
        }
        let mut identities = BTreeSet::new();
        let mut sources = BTreeSet::new();
        let mut providers = Vec::with_capacity(specs.len());
        for spec in specs {
            let provider = match spec {
                ProviderSpec::Directory { id, revision, root } => {
                    validate_provider(&id, &revision, &root, &mut identities, &mut sources)?;
                    let metadata = fs::symlink_metadata(&root).map_err(|_| {
                        provider_error(ErrorCode::InvalidPlan, &id, "open directory")
                    })?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(provider_error(
                            ErrorCode::InvalidPlan,
                            &id,
                            "open directory",
                        ));
                    }
                    Provider::Directory(DirectoryProvider { id, revision, root })
                }
                ProviderSpec::Vpk {
                    id,
                    revision,
                    directory_file,
                    layout,
                } => {
                    validate_provider(
                        &id,
                        &revision,
                        &directory_file,
                        &mut identities,
                        &mut sources,
                    )?;
                    let bytes =
                        read_bounded_file(&directory_file, limits.max_directory_file_bytes, &id)?;
                    let identity = directory_file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| {
                            provider_error(ErrorCode::InvalidPlan, &id, "name VPK directory")
                        })?;
                    let archive = playsrc_vpk::parse(
                        &bytes,
                        identity,
                        layout,
                        VpkLimits {
                            max_directory_bytes: limits.max_directory_file_bytes,
                            max_logical_path_bytes: limits.max_path_bytes,
                            max_entry_bytes: limits.max_read_bytes,
                            max_integrity_read_bytes: limits.max_read_bytes,
                            ..VpkLimits::default()
                        },
                    )
                    .map_err(|_| provider_error(ErrorCode::InvalidPlan, &id, "parse VPK index"))?;
                    let root = directory_file
                        .parent()
                        .filter(|path| !path.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf();
                    let segments = Arc::new(LocalSegments {
                        root,
                        directory_identity: archive.identity.clone(),
                        layout,
                    });
                    Provider::Vpk(Box::new(VpkProvider {
                        id,
                        revision,
                        archive,
                        segments,
                    }))
                }
                ProviderSpec::IndexedVpk {
                    id,
                    revision,
                    directory_identity,
                    directory_bytes,
                    layout,
                    segments,
                } => {
                    if !valid_identity(&id)
                        || !valid_identity(&revision)
                        || !identities.insert(id.to_ascii_lowercase())
                        || !sources.insert(PathBuf::from(format!("indexed:{directory_identity}")))
                        || directory_bytes.len() > limits.max_directory_file_bytes
                    {
                        return Err(plan_error(ErrorCode::InvalidPlan, "validate VPK provider"));
                    }
                    let archive = playsrc_vpk::parse(
                        &directory_bytes,
                        directory_identity,
                        layout,
                        VpkLimits {
                            max_directory_bytes: limits.max_directory_file_bytes,
                            max_logical_path_bytes: limits.max_path_bytes,
                            max_entry_bytes: limits.max_read_bytes,
                            max_integrity_read_bytes: limits.max_read_bytes,
                            ..VpkLimits::default()
                        },
                    )
                    .map_err(|_| provider_error(ErrorCode::InvalidPlan, &id, "parse VPK index"))?;
                    Provider::Vpk(Box::new(VpkProvider {
                        id,
                        revision,
                        archive,
                        segments,
                    }))
                }
            };
            providers.push(provider);
        }
        Ok(Self {
            game,
            content_build,
            providers: providers.into(),
            active_pak: None,
            supplement: None,
            limits,
        })
    }

    pub fn with_active_pak(
        &self,
        id: impl Into<String>,
        revision: impl Into<String>,
        map_identity: impl Into<String>,
        pak: &Pak,
    ) -> Result<Self, Error> {
        let id = id.into();
        let revision = revision.into();
        let map_identity = normalize(&map_identity.into(), self.limits.max_path_bytes)?;
        if !valid_identity(&id) || !valid_identity(&revision) {
            return Err(plan_error(ErrorCode::InvalidPlan, "activate BSP PAK"));
        }
        let mut lookup = BTreeMap::new();
        for (index, entry) in pak.entries.iter().enumerate() {
            let path = std::str::from_utf8(&entry.raw_name)
                .map_err(|_| provider_error(ErrorCode::InvalidPlan, &id, "index BSP PAK"))?;
            let path = normalize(path, self.limits.max_path_bytes)
                .map_err(|_| provider_error(ErrorCode::InvalidPlan, &id, "index BSP PAK"))?;
            if lookup.insert(path, index).is_some() {
                return Err(provider_error(ErrorCode::InvalidPlan, &id, "index BSP PAK"));
            }
        }
        let mut next = self.clone();
        next.active_pak = Some(Arc::new(PakProvider {
            id,
            revision,
            map_identity,
            pak: pak.clone(),
            lookup,
        }));
        Ok(next)
    }

    pub fn with_map_supplement(
        &self,
        id: impl Into<String>,
        revision: impl Into<String>,
        objects: Vec<SupplementObject>,
    ) -> Result<Self, Error> {
        let id = id.into();
        let revision = revision.into();
        if !valid_identity(&id)
            || !valid_identity(&revision)
            || objects.len() > self.limits.max_supplement_objects
        {
            return Err(plan_error(ErrorCode::InvalidPlan, "mount map supplement"));
        }
        let mut lookup = BTreeMap::new();
        for mut object in objects {
            object.logical_path = normalize(&object.logical_path, self.limits.max_path_bytes)
                .map_err(|_| provider_error(ErrorCode::InvalidPlan, &id, "index map supplement"))?;
            if !is_sha256(&object.sha256)
                || object.byte_length > self.limits.max_read_bytes
                || lookup.insert(object.logical_path.clone(), object).is_some()
            {
                return Err(provider_error(
                    ErrorCode::InvalidPlan,
                    &id,
                    "index map supplement",
                ));
            }
        }
        let mut next = self.clone();
        next.supplement = Some(Arc::new(SupplementProvider {
            id,
            revision,
            lookup,
        }));
        Ok(next)
    }

    pub fn resolve_map(&self, requested_path: &str) -> Result<Resolution, Error> {
        let logical_path = normalize(requested_path, self.limits.max_path_bytes)?;
        if !logical_path.starts_with("maps/") || !logical_path.ends_with(".bsp") {
            return Err(path_error(requested_path));
        }
        self.resolve_external(logical_path, Vec::new())
    }

    pub fn resolve_resource(&self, requested_path: &str) -> Result<Resolution, Error> {
        let logical_path = normalize(requested_path, self.limits.max_path_bytes)?;
        let mut checked = Vec::with_capacity(self.providers.len() + 2);
        if let Some(provider) = &self.active_pak {
            let location = format!("{}!{logical_path}", provider.map_identity);
            checked.push(CheckedLocation {
                provider_id: provider.id.clone(),
                provider_kind: ProviderKind::BspPak,
                location: location.clone(),
            });
            if let Some(index) = provider.lookup.get(&logical_path) {
                let entry = &provider.pak.entries[*index];
                if entry.classification != PakEntryClassification::Handled {
                    return Err(selected_error(
                        ErrorCode::SelectedProviderUnsupported,
                        &logical_path,
                        &provider.id,
                        "read BSP PAK entry",
                    ));
                }
                let bytes = entry.decoded.clone().ok_or_else(|| {
                    selected_error(
                        ErrorCode::SelectedProviderUnsupported,
                        &logical_path,
                        &provider.id,
                        "decode BSP PAK entry",
                    )
                })?;
                return self.found(
                    logical_path,
                    provider.id.clone(),
                    ProviderKind::BspPak,
                    provider.revision.clone(),
                    location,
                    bytes,
                );
            }
        }

        if let Some(provider) = &self.supplement {
            let location = format!("supplement:{}:{logical_path}", provider.revision);
            checked.push(CheckedLocation {
                provider_id: provider.id.clone(),
                provider_kind: ProviderKind::MapSupplement,
                location: location.clone(),
            });
            if let Some(object) = provider.lookup.get(&logical_path) {
                let bytes = match &object.bytes {
                    SupplementBytes::Available(bytes) => bytes.clone(),
                    SupplementBytes::Missing => {
                        return Err(selected_error(
                            ErrorCode::SelectedProviderMissing,
                            &logical_path,
                            &provider.id,
                            "read map supplement object",
                        ));
                    }
                };
                if bytes.len() != object.byte_length
                    || format!("{:x}", Sha256::digest(&bytes)) != object.sha256
                {
                    return Err(selected_error(
                        ErrorCode::SelectedProviderCorrupt,
                        &logical_path,
                        &provider.id,
                        "verify map supplement object",
                    ));
                }
                return self.found(
                    logical_path,
                    provider.id.clone(),
                    ProviderKind::MapSupplement,
                    provider.revision.clone(),
                    location,
                    bytes,
                );
            }
        }

        self.resolve_external(logical_path, checked)
    }

    fn resolve_external(
        &self,
        logical_path: String,
        mut checked: Vec<CheckedLocation>,
    ) -> Result<Resolution, Error> {
        for provider in self.providers.iter() {
            match provider {
                Provider::Directory(provider) => {
                    checked.push(CheckedLocation {
                        provider_id: provider.id.clone(),
                        provider_kind: ProviderKind::Directory,
                        location: logical_path.clone(),
                    });
                    let Some(path) = resolve_directory_path(provider, &logical_path)? else {
                        continue;
                    };
                    let bytes = read_selected_file(
                        &path,
                        self.limits.max_read_bytes,
                        &logical_path,
                        &provider.id,
                    )?;
                    return self.found(
                        logical_path,
                        provider.id.clone(),
                        ProviderKind::Directory,
                        provider.revision.clone(),
                        path.strip_prefix(&provider.root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .replace('\\', "/"),
                        bytes,
                    );
                }
                Provider::Vpk(provider) => {
                    checked.push(CheckedLocation {
                        provider_id: provider.id.clone(),
                        provider_kind: ProviderKind::Vpk,
                        location: format!("{}!{logical_path}", provider.archive.identity),
                    });
                    let entry = match provider.archive.entry(&logical_path) {
                        Ok(entry) => entry,
                        Err(error) if error.code == VpkErrorCode::MissingEntry => continue,
                        Err(_) => {
                            return Err(selected_error(
                                ErrorCode::SelectedProviderCorrupt,
                                &logical_path,
                                &provider.id,
                                "lookup VPK entry",
                            ));
                        }
                    };
                    let result = provider
                        .archive
                        .read_entry(&logical_path, provider.segments.as_ref())
                        .map_err(|error| map_vpk_error(error, &logical_path, &provider.id))?;
                    return self.found(
                        logical_path,
                        provider.id.clone(),
                        ProviderKind::Vpk,
                        provider.revision.clone(),
                        format!(
                            "{}[{}]@{}+{}",
                            provider.archive.identity,
                            entry.data.archive_index,
                            entry.data.offset,
                            entry.data.length
                        ),
                        result.bytes,
                    );
                }
            }
        }
        Ok(Resolution::Missing {
            logical_path,
            checked,
        })
    }

    fn found(
        &self,
        logical_path: String,
        provider_id: String,
        provider_kind: ProviderKind,
        provider_revision: String,
        location: String,
        bytes: Vec<u8>,
    ) -> Result<Resolution, Error> {
        if bytes.len() > self.limits.max_read_bytes {
            return Err(selected_error(
                ErrorCode::ReadLimit,
                &logical_path,
                &provider_id,
                "publish content result",
            ));
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        Ok(Resolution::Found(Resolved {
            provenance: Provenance {
                game: self.game.clone(),
                content_build: self.content_build.clone(),
                logical_path,
                provider_id,
                provider_kind,
                provider_revision,
                location,
                byte_length: bytes.len(),
                sha256,
            },
            bytes,
        }))
    }
}

#[derive(Clone, Debug)]
struct LocalSegments {
    root: PathBuf,
    directory_identity: String,
    layout: VpkLayout,
}

impl LocalSegments {
    fn segment_identity(&self, index: u16) -> Result<String, SourceError> {
        if index == playsrc_vpk::EMBEDDED_ARCHIVE {
            return Ok(self.directory_identity.clone());
        }
        if self.layout != VpkLayout::Split {
            return Err(SourceError {
                code: SourceErrorCode::Missing,
                range: 0..0,
            });
        }
        let base = self
            .directory_identity
            .strip_suffix("_dir.vpk")
            .ok_or(SourceError {
                code: SourceErrorCode::Missing,
                range: 0..0,
            })?;
        Ok(format!("{base}_{index:03}.vpk"))
    }
}

impl SegmentReader for LocalSegments {
    fn len(&self, archive_index: u32) -> Result<u64, SourceError> {
        let index = u16::try_from(archive_index).map_err(|_| SourceError {
            code: SourceErrorCode::Missing,
            range: 0..0,
        })?;
        let identity = self.segment_identity(index)?;
        fs::metadata(self.root.join(identity))
            .map(|metadata| metadata.len())
            .map_err(|error| SourceError {
                code: if error.kind() == std::io::ErrorKind::NotFound {
                    SourceErrorCode::Missing
                } else {
                    SourceErrorCode::Io
                },
                range: 0..0,
            })
    }

    fn read(&self, archive_index: u32, range: Range<u64>) -> Result<Vec<u8>, SourceError> {
        let index = u16::try_from(archive_index).map_err(|_| SourceError {
            code: SourceErrorCode::Missing,
            range: range.clone(),
        })?;
        let identity = self.segment_identity(index).map_err(|mut error| {
            error.range = range.clone();
            error
        })?;
        let mut file = File::open(self.root.join(identity)).map_err(|error| SourceError {
            code: if error.kind() == std::io::ErrorKind::NotFound {
                SourceErrorCode::Missing
            } else {
                SourceErrorCode::Io
            },
            range: range.clone(),
        })?;
        file.seek(SeekFrom::Start(range.start))
            .map_err(|_| SourceError {
                code: SourceErrorCode::Io,
                range: range.clone(),
            })?;
        let mut bytes = vec![0; (range.end - range.start) as usize];
        file.read_exact(&mut bytes).map_err(|_| SourceError {
            code: SourceErrorCode::ShortRead,
            range,
        })?;
        Ok(bytes)
    }
}

fn normalize(path: &str, max_bytes: usize) -> Result<String, Error> {
    if path.is_empty()
        || path.len() > max_bytes
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains(':')
        || path.contains('?')
        || path.contains('#')
        || path.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(path_error(path));
    }
    let replaced = path.replace('\\', "/");
    let mut components = Vec::new();
    for component in replaced.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(path_error(path)),
            value => components.push(value.to_ascii_lowercase()),
        }
    }
    let normalized = components.join("/");
    if normalized.is_empty() || normalized.len() > max_bytes {
        return Err(path_error(path));
    }
    Ok(normalized)
}

fn resolve_directory_path(
    provider: &DirectoryProvider,
    logical_path: &str,
) -> Result<Option<PathBuf>, Error> {
    let mut current = provider.root.clone();
    for component in logical_path.split('/') {
        let entries = fs::read_dir(&current).map_err(|_| {
            selected_error(
                ErrorCode::SelectedProviderIo,
                logical_path,
                &provider.id,
                "enumerate directory component",
            )
        })?;
        let mut matched = None;
        for entry in entries {
            let entry = entry.map_err(|_| {
                selected_error(
                    ErrorCode::SelectedProviderIo,
                    logical_path,
                    &provider.id,
                    "enumerate directory component",
                )
            })?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.eq_ignore_ascii_case(component) && matched.replace(entry.path()).is_some() {
                return Err(selected_error(
                    ErrorCode::AmbiguousDirectory,
                    logical_path,
                    &provider.id,
                    "resolve directory component",
                ));
            }
        }
        let Some(next) = matched else {
            return Ok(None);
        };
        let metadata = fs::symlink_metadata(&next).map_err(|_| {
            selected_error(
                ErrorCode::SelectedProviderChanged,
                logical_path,
                &provider.id,
                "inspect directory component",
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(selected_error(
                ErrorCode::EscapedDirectory,
                logical_path,
                &provider.id,
                "resolve directory component",
            ));
        }
        current = next;
    }
    let metadata = fs::symlink_metadata(&current).map_err(|_| {
        selected_error(
            ErrorCode::SelectedProviderChanged,
            logical_path,
            &provider.id,
            "inspect directory file",
        )
    })?;
    if metadata.is_file() {
        Ok(Some(current))
    } else {
        Ok(None)
    }
}

fn read_bounded_file(path: &Path, limit: usize, provider_id: &str) -> Result<Vec<u8>, Error> {
    let metadata = fs::metadata(path).map_err(|_| {
        provider_error(ErrorCode::InvalidPlan, provider_id, "inspect VPK directory")
    })?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(provider_error(
            ErrorCode::InvalidPlan,
            provider_id,
            "inspect VPK directory",
        ));
    }
    let bytes = fs::read(path)
        .map_err(|_| provider_error(ErrorCode::InvalidPlan, provider_id, "read VPK directory"))?;
    if bytes.len() as u64 != metadata.len() {
        return Err(provider_error(
            ErrorCode::InvalidPlan,
            provider_id,
            "read VPK directory",
        ));
    }
    Ok(bytes)
}

fn read_selected_file(
    path: &Path,
    limit: usize,
    logical_path: &str,
    provider_id: &str,
) -> Result<Vec<u8>, Error> {
    let metadata = fs::metadata(path).map_err(|_| {
        selected_error(
            ErrorCode::SelectedProviderChanged,
            logical_path,
            provider_id,
            "inspect selected file",
        )
    })?;
    if metadata.len() > limit as u64 {
        return Err(selected_error(
            ErrorCode::ReadLimit,
            logical_path,
            provider_id,
            "inspect selected file",
        ));
    }
    let bytes = fs::read(path).map_err(|_| {
        selected_error(
            ErrorCode::SelectedProviderIo,
            logical_path,
            provider_id,
            "read selected file",
        )
    })?;
    if bytes.len() as u64 != metadata.len() {
        return Err(selected_error(
            ErrorCode::SelectedProviderChanged,
            logical_path,
            provider_id,
            "read selected file",
        ));
    }
    Ok(bytes)
}

fn validate_provider(
    id: &str,
    revision: &str,
    source: &Path,
    identities: &mut BTreeSet<String>,
    sources: &mut BTreeSet<PathBuf>,
) -> Result<(), Error> {
    if !valid_identity(id)
        || !valid_identity(revision)
        || !source.is_absolute()
        || !identities.insert(id.to_ascii_lowercase())
        || !sources.insert(source.to_path_buf())
    {
        return Err(plan_error(ErrorCode::InvalidPlan, "validate provider"));
    }
    Ok(())
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn map_vpk_error(error: playsrc_vpk::Error, path: &str, provider_id: &str) -> Error {
    let code = match error.code {
        VpkErrorCode::MissingSegment => ErrorCode::SelectedProviderMissing,
        VpkErrorCode::ChangedSource | VpkErrorCode::ShortRead => ErrorCode::SelectedProviderChanged,
        VpkErrorCode::SourceIo => ErrorCode::SelectedProviderIo,
        VpkErrorCode::CrcMismatch => ErrorCode::SelectedProviderCorrupt,
        _ => ErrorCode::SelectedProviderCorrupt,
    };
    selected_error(code, path, provider_id, "read VPK entry")
}

fn path_error(path: &str) -> Error {
    Error {
        code: ErrorCode::InvalidPath,
        logical_path: Some(path.to_owned()),
        provider_id: None,
        operation: "normalize logical path",
    }
}

fn plan_error(code: ErrorCode, operation: &'static str) -> Error {
    Error {
        code,
        logical_path: None,
        provider_id: None,
        operation,
    }
}

fn provider_error(code: ErrorCode, provider_id: &str, operation: &'static str) -> Error {
    Error {
        code,
        logical_path: None,
        provider_id: Some(provider_id.to_owned()),
        operation,
    }
}

fn selected_error(
    code: ErrorCode,
    path: &str,
    provider_id: &str,
    operation: &'static str,
) -> Error {
    Error {
        code,
        logical_path: Some(path.to_owned()),
        provider_id: Some(provider_id.to_owned()),
        operation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_bsp::{PakEntry, PakEntryClassification};
    use tempfile::TempDir;

    #[derive(Debug)]
    struct MemorySegments(BTreeMap<u32, Vec<u8>>);

    impl SegmentReader for MemorySegments {
        fn len(&self, archive_index: u32) -> Result<u64, SourceError> {
            self.0
                .get(&archive_index)
                .map(|bytes| bytes.len() as u64)
                .ok_or(SourceError {
                    code: SourceErrorCode::Missing,
                    range: 0..0,
                })
        }

        fn read(&self, archive_index: u32, range: Range<u64>) -> Result<Vec<u8>, SourceError> {
            self.0
                .get(&archive_index)
                .and_then(|bytes| bytes.get(range.start as usize..range.end as usize))
                .map(<[u8]>::to_vec)
                .ok_or(SourceError {
                    code: SourceErrorCode::ShortRead,
                    range,
                })
        }
    }

    fn directory_spec(id: &str, root: &Path) -> ProviderSpec {
        ProviderSpec::Directory {
            id: id.to_owned(),
            revision: "r1".to_owned(),
            root: root.to_path_buf(),
        }
    }

    fn pak(path: &[u8], bytes: Option<&[u8]>, classification: PakEntryClassification) -> Pak {
        Pak {
            entries: vec![PakEntry {
                raw_name: path.to_vec(),
                flags: 0,
                compression_method: 0,
                crc32: bytes.map(crc32fast::hash).unwrap_or(0),
                encoded_size: bytes.map(|value| value.len() as u32).unwrap_or(0),
                decoded_size: bytes.map(|value| value.len() as u32).unwrap_or(0),
                local_header_offset: 0,
                local_header_range: 0..0,
                local_extra: Vec::new(),
                encoded_range: 0..0,
                central_header_range: 0..0,
                central_extra: Vec::new(),
                comment: Vec::new(),
                classification,
                decoded: bytes.map(<[u8]>::to_vec),
            }],
            central_directory_range: 0..0,
            central_directory_tail: Vec::new(),
            end_record_range: 0..0,
            archive_comment: Vec::new(),
        }
    }

    #[test]
    fn normalizes_requests_and_rejects_escapes_before_provider_access() {
        assert_eq!(
            normalize("Materials\\A//./B.VMT", 100).unwrap(),
            "materials/a/b.vmt"
        );
        for invalid in ["", "/a", "../a", "a/../b", "C:/a", "a?b", "a#b", "a\0b"] {
            assert_eq!(
                normalize(invalid, 100).unwrap_err().code,
                ErrorCode::InvalidPath
            );
        }
    }

    #[test]
    fn resolves_first_directory_and_reports_all_exact_missing_candidates() {
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        fs::create_dir(first.path().join("Materials")).unwrap();
        fs::create_dir(second.path().join("materials")).unwrap();
        fs::write(first.path().join("Materials/A.VMT"), b"first").unwrap();
        fs::write(second.path().join("materials/a.vmt"), b"second").unwrap();
        let content = Content::open(
            "tf2",
            "24207079",
            vec![
                directory_spec("first", first.path()),
                directory_spec("second", second.path()),
            ],
            Limits::default(),
        )
        .unwrap();
        let Resolution::Found(found) = content.resolve_resource("materials/a.vmt").unwrap() else {
            panic!("first directory did not resolve")
        };
        assert_eq!(found.bytes, b"first");
        assert_eq!(found.provenance.provider_id, "first");
        assert_eq!(found.provenance.provider_kind, ProviderKind::Directory);
        assert_eq!(found.provenance.byte_length, 5);
        assert_eq!(
            found.provenance.sha256,
            "a7937b64b8caa58f03721bb6bacf5c78cb235febe0e70b1b84cd99541461a08e"
        );

        let Resolution::Missing { checked, .. } =
            content.resolve_resource("materials/missing.vmt").unwrap()
        else {
            panic!("missing request resolved")
        };
        assert_eq!(checked.len(), 2);
        assert_eq!(checked[0].provider_id, "first");
        assert_eq!(checked[1].provider_id, "second");
    }

    #[test]
    fn active_pak_precedes_external_providers_and_selected_failure_does_not_fall_back() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("materials")).unwrap();
        fs::write(root.path().join("materials/a.vmt"), b"external").unwrap();
        let content = Content::open(
            "tf2",
            "24207079",
            vec![directory_spec("loose", root.path())],
            Limits::default(),
        )
        .unwrap();
        let supplement_bytes = b"supplement".to_vec();
        let supplemented = content
            .with_map_supplement(
                "map-atlas",
                "atlas-sha",
                vec![SupplementObject {
                    logical_path: "materials/a.vmt".to_owned(),
                    byte_length: supplement_bytes.len(),
                    sha256: format!("{:x}", Sha256::digest(&supplement_bytes)),
                    bytes: SupplementBytes::Available(supplement_bytes),
                }],
            )
            .unwrap();
        assert!(Arc::ptr_eq(&content.providers, &supplemented.providers));
        let Resolution::Found(supplemented_result) =
            supplemented.resolve_resource("materials/a.vmt").unwrap()
        else {
            panic!("map supplement did not resolve")
        };
        assert_eq!(supplemented_result.bytes, b"supplement");
        assert_eq!(
            supplemented_result.provenance.provider_kind,
            ProviderKind::MapSupplement
        );

        let active = supplemented
            .with_active_pak(
                "map-pak",
                "map-sha",
                "maps/jump_beef.bsp",
                &pak(
                    b"materials/a.vmt",
                    Some(b"embedded"),
                    PakEntryClassification::Handled,
                ),
            )
            .unwrap();
        assert!(Arc::ptr_eq(&content.providers, &active.providers));
        assert!(Arc::ptr_eq(
            supplemented.supplement.as_ref().unwrap(),
            active.supplement.as_ref().unwrap()
        ));
        let Resolution::Found(found) = active.resolve_resource("materials/a.vmt").unwrap() else {
            panic!("active PAK did not resolve")
        };
        assert_eq!(found.bytes, b"embedded");
        assert_eq!(found.provenance.provider_kind, ProviderKind::BspPak);

        let unsupported = supplemented
            .with_active_pak(
                "map-pak",
                "map-sha",
                "maps/jump_beef.bsp",
                &pak(
                    b"materials/a.vmt",
                    None,
                    PakEntryClassification::Unsupported,
                ),
            )
            .unwrap();
        let error = unsupported.resolve_resource("materials/a.vmt").unwrap_err();
        assert_eq!(error.code, ErrorCode::SelectedProviderUnsupported);
        assert_eq!(error.provider_id.as_deref(), Some("map-pak"));

        let missing = content
            .with_map_supplement(
                "map-atlas",
                "atlas-sha",
                vec![SupplementObject {
                    logical_path: "materials/a.vmt".to_owned(),
                    byte_length: 8,
                    sha256: "0000000000000000000000000000000000000000000000000000000000000000"
                        .to_owned(),
                    bytes: SupplementBytes::Missing,
                }],
            )
            .unwrap();
        assert_eq!(
            missing
                .resolve_resource("materials/a.vmt")
                .unwrap_err()
                .code,
            ErrorCode::SelectedProviderMissing
        );
    }

    #[test]
    fn map_resolution_uses_external_providers_before_an_active_map_exists() {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join("maps")).unwrap();
        fs::write(root.path().join("maps/jump_beef.bsp"), b"bsp").unwrap();
        let content = Content::open(
            "tf2",
            "24207079",
            vec![directory_spec("loose", root.path())],
            Limits::default(),
        )
        .unwrap()
        .with_active_pak(
            "prior-map-pak",
            "prior-sha",
            "maps/prior.bsp",
            &pak(
                b"maps/jump_beef.bsp",
                Some(b"wrong"),
                PakEntryClassification::Handled,
            ),
        )
        .unwrap();
        let Resolution::Found(found) = content.resolve_map("maps/jump_beef.bsp").unwrap() else {
            panic!("map did not resolve externally")
        };
        assert_eq!(found.bytes, b"bsp");
        assert_eq!(found.provenance.provider_kind, ProviderKind::Directory);
    }

    fn vpk_tree(path_data: &[u8]) -> Vec<u8> {
        let mut tree = b"vmt\0materials\0a\0".to_vec();
        tree.extend_from_slice(&crc32fast::hash(path_data).to_le_bytes());
        tree.extend_from_slice(&0_u16.to_le_bytes());
        tree.extend_from_slice(&0_u16.to_le_bytes());
        tree.extend_from_slice(&0_u32.to_le_bytes());
        tree.extend_from_slice(&(path_data.len() as u32).to_le_bytes());
        tree.extend_from_slice(&playsrc_vpk::ENTRY_TERMINATOR.to_le_bytes());
        tree.extend_from_slice(&[0, 0, 0]);
        tree
    }

    fn vpk_directory(tree: &[u8]) -> Vec<u8> {
        let mut bytes = playsrc_vpk::SIGNATURE.to_le_bytes().to_vec();
        for field in [2_u32, tree.len() as u32, 0, 0, 0, 0] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        bytes.extend_from_slice(tree);
        bytes
    }

    #[test]
    fn resolves_vpk_segment_and_stops_on_selected_crc_failure() {
        let root = TempDir::new().unwrap();
        let data = b"vpk material";
        fs::write(
            root.path().join("pak_dir.vpk"),
            vpk_directory(&vpk_tree(data)),
        )
        .unwrap();
        fs::write(root.path().join("pak_000.vpk"), data).unwrap();
        fs::create_dir(root.path().join("materials")).unwrap();
        fs::write(root.path().join("materials/a.vmt"), b"fallback").unwrap();
        let content = Content::open(
            "tf2",
            "24207079",
            vec![
                ProviderSpec::Vpk {
                    id: "vpk".to_owned(),
                    revision: "index-sha".to_owned(),
                    directory_file: root.path().join("pak_dir.vpk"),
                    layout: VpkLayout::Split,
                },
                directory_spec("loose", root.path()),
            ],
            Limits::default(),
        )
        .unwrap();
        let Resolution::Found(found) = content.resolve_resource("materials/a.vmt").unwrap() else {
            panic!("VPK did not resolve")
        };
        assert_eq!(found.bytes, data);
        assert_eq!(found.provenance.provider_kind, ProviderKind::Vpk);
        assert_eq!(found.provenance.location, "pak_dir.vpk[0]@0+12");

        fs::write(root.path().join("pak_000.vpk"), b"bad material").unwrap();
        let error = content.resolve_resource("materials/a.vmt").unwrap_err();
        assert_eq!(error.code, ErrorCode::SelectedProviderCorrupt);
        assert_eq!(error.provider_id.as_deref(), Some("vpk"));
    }

    #[test]
    fn indexed_vpk_consumes_supplied_directory_bytes_and_ranged_segment_adapter() {
        let data = b"remote range";
        let directory_bytes = vpk_directory(&vpk_tree(data));
        let content = Content::open(
            "tf2",
            "24207079",
            vec![ProviderSpec::IndexedVpk {
                id: "remote-vpk".to_owned(),
                revision: "object-sha".to_owned(),
                directory_identity: "pak_dir.vpk".to_owned(),
                directory_bytes,
                layout: VpkLayout::Split,
                segments: Arc::new(MemorySegments(BTreeMap::from([(0, data.to_vec())]))),
            }],
            Limits::default(),
        )
        .unwrap();
        let Resolution::Found(found) = content.resolve_resource("materials/a.vmt").unwrap() else {
            panic!("indexed VPK did not resolve")
        };
        assert_eq!(found.bytes, data);
        assert_eq!(found.provenance.location, "pak_dir.vpk[0]@0+12");
    }
}
