use std::{collections::BTreeMap, fmt, ops::Range, sync::Arc};

use md5::{Digest as _, Md5};
use rsa::{
    RsaPublicKey,
    pkcs1v15::{Signature, VerifyingKey},
    pkcs8::DecodePublicKey,
    signature::Verifier,
};
use sha2::Sha256;

pub const SIGNATURE: u32 = 0x55aa_1234;
pub const EMBEDDED_ARCHIVE: u16 = 0x7fff;
pub const ENTRY_TERMINATOR: u16 = 0xffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    Split,
    Standalone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_directory_bytes: usize,
    pub max_tree_bytes: usize,
    pub max_entries: usize,
    pub max_component_bytes: usize,
    pub max_logical_path_bytes: usize,
    pub max_total_preload_bytes: usize,
    pub max_entry_bytes: usize,
    pub max_integrity_read_bytes: usize,
    pub max_integrity_records: usize,
    pub max_public_key_bytes: usize,
    pub max_signature_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_directory_bytes: 64 * 1024 * 1024,
            max_tree_bytes: 32 * 1024 * 1024,
            max_entries: 250_000,
            max_component_bytes: 255,
            max_logical_path_bytes: 1_024,
            max_total_preload_bytes: 64 * 1024 * 1024,
            max_entry_bytes: 512 * 1024 * 1024,
            max_integrity_read_bytes: 512 * 1024 * 1024,
            max_integrity_records: 1_000_000,
            max_public_key_bytes: 16 * 1024,
            max_signature_bytes: 16 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sections {
    pub header: Range<usize>,
    pub tree: Range<usize>,
    pub file_data: Range<usize>,
    pub archive_md5: Range<usize>,
    pub other_md5: Range<usize>,
    pub signature: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataDescriptor {
    pub archive_index: u16,
    pub offset: u32,
    pub length: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub extension: Vec<u8>,
    pub directory: Vec<u8>,
    pub basename: Vec<u8>,
    pub stored_path: String,
    pub logical_path: String,
    pub crc32: u32,
    pub preload_range: Range<usize>,
    pub data: DataDescriptor,
    pub total_length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveMd5Record {
    pub archive_index: u32,
    pub offset: u32,
    pub length: u32,
    pub digest: [u8; 16],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OtherMd5 {
    pub tree: [u8; 16],
    pub archive_md5: [u8; 16],
    pub whole_file: [u8; 16],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureMaterial {
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
    pub signed_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Integrity {
    NotPresent,
    NotEvaluated,
    Verified,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfIntegrity {
    pub tree_md5: Integrity,
    pub archive_md5: Integrity,
    pub whole_file_md5: Integrity,
    pub signature: SignatureIntegrity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureIntegrity {
    NotPresent,
    WrongKey,
    Verified,
    Invalid,
}

#[derive(Clone, Debug)]
pub struct Archive {
    pub identity: String,
    pub layout: Layout,
    pub version: u32,
    pub sections: Sections,
    pub entries: Vec<Entry>,
    pub archive_md5: Vec<ArchiveMd5Record>,
    pub other_md5: Option<OtherMd5>,
    pub signature: Option<SignatureMaterial>,
    lookup: BTreeMap<String, usize>,
    directory_bytes: Arc<[u8]>,
    limits: Limits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    TruncatedHeader,
    InvalidSignature,
    UnsupportedVersion,
    InvalidIdentity,
    InvalidSections,
    TreeLimit,
    InvalidTree,
    InvalidComponent,
    DuplicatePath,
    EntryLimit,
    PreloadLimit,
    EntrySizeLimit,
    InvalidDescriptor,
    InvalidEmbeddedRange,
    InvalidIntegritySection,
    IntegrityLimit,
    MissingEntry,
    InvalidReadRange,
    MissingSegment,
    ChangedSource,
    ShortRead,
    SourceIo,
    CrcMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub archive: String,
    pub path: Option<String>,
    pub segment: Option<u32>,
    pub range: Range<u64>,
    pub declared: Option<u64>,
    pub limit: Option<u64>,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?} in {} at {}..{}",
            self.code, self.archive, self.range.start, self.range.end
        )
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceErrorCode {
    Missing,
    Changed,
    ShortRead,
    Io,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceError {
    pub code: SourceErrorCode,
    pub range: Range<u64>,
}

pub trait SegmentReader {
    fn len(&self, archive_index: u32) -> Result<u64, SourceError>;
    fn read(&self, archive_index: u32, range: Range<u64>) -> Result<Vec<u8>, SourceError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadResult {
    pub bytes: Vec<u8>,
    pub crc32: Integrity,
}

pub fn parse(
    directory_bytes: &[u8],
    identity: impl Into<String>,
    layout: Layout,
    limits: Limits,
) -> Result<Archive, Error> {
    let identity = identity.into();
    validate_identity(&identity, layout)?;
    if directory_bytes.len() > limits.max_directory_bytes {
        return Err(error(
            ErrorCode::InputLimit,
            &identity,
            0..directory_bytes.len() as u64,
            Some(directory_bytes.len() as u64),
            Some(limits.max_directory_bytes as u64),
        ));
    }
    if directory_bytes.len() < 12 {
        return Err(error(
            ErrorCode::TruncatedHeader,
            &identity,
            directory_bytes.len() as u64..12,
            None,
            None,
        ));
    }
    if u32_at(directory_bytes, 0) != SIGNATURE {
        return Err(error(
            ErrorCode::InvalidSignature,
            &identity,
            0..4,
            None,
            None,
        ));
    }
    let version = u32_at(directory_bytes, 4);
    let tree_size = u32_at(directory_bytes, 8) as usize;
    if tree_size > limits.max_tree_bytes {
        return Err(error(
            ErrorCode::TreeLimit,
            &identity,
            8..12,
            Some(tree_size as u64),
            Some(limits.max_tree_bytes as u64),
        ));
    }
    let sections = sections(directory_bytes, &identity, version, tree_size)?;
    let mut entries = parse_tree(directory_bytes, &identity, &sections, limits)?;
    if layout == Layout::Standalone
        && entries
            .iter()
            .any(|entry| entry.data.archive_index != EMBEDDED_ARCHIVE)
    {
        return Err(error(
            ErrorCode::InvalidDescriptor,
            &identity,
            sections.tree.start as u64..sections.tree.end as u64,
            None,
            None,
        ));
    }
    for entry in &mut entries {
        if entry.data.archive_index == EMBEDDED_ARCHIVE {
            let start = sections
                .file_data
                .start
                .checked_add(entry.data.offset as usize)
                .ok_or_else(|| entry_error(ErrorCode::InvalidEmbeddedRange, &identity, entry))?;
            let end = start
                .checked_add(entry.data.length as usize)
                .ok_or_else(|| entry_error(ErrorCode::InvalidEmbeddedRange, &identity, entry))?;
            if end > sections.file_data.end {
                return Err(entry_error(
                    ErrorCode::InvalidEmbeddedRange,
                    &identity,
                    entry,
                ));
            }
        }
    }
    let archive_md5 = parse_archive_md5(directory_bytes, &identity, &sections, limits)?;
    let other_md5 = parse_other_md5(directory_bytes, &identity, &sections)?;
    let signature = parse_signature(directory_bytes, &identity, &sections, limits)?;
    let lookup = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.logical_path.clone(), index))
        .collect();
    Ok(Archive {
        identity,
        layout,
        version,
        sections,
        entries,
        archive_md5,
        other_md5,
        signature,
        lookup,
        directory_bytes: Arc::from(directory_bytes),
        limits,
    })
}

impl Archive {
    pub fn entry(&self, logical_path: &str) -> Result<&Entry, Error> {
        let canonical = canonical_request(
            logical_path,
            &self.identity,
            self.limits.max_logical_path_bytes,
        )?;
        let Some(index) = self.lookup.get(&canonical) else {
            let mut result = error(ErrorCode::MissingEntry, &self.identity, 0..0, None, None);
            result.path = Some(canonical);
            return Err(result);
        };
        Ok(&self.entries[*index])
    }

    pub fn segment_identity(&self, archive_index: u16) -> Result<String, Error> {
        if archive_index == EMBEDDED_ARCHIVE {
            return Ok(self.identity.clone());
        }
        if self.layout != Layout::Split {
            return Err(error(
                ErrorCode::InvalidDescriptor,
                &self.identity,
                0..0,
                Some(archive_index as u64),
                None,
            ));
        }
        let base = self
            .identity
            .strip_suffix("_dir.vpk")
            .expect("split identity validated at parse");
        Ok(format!("{base}_{archive_index:03}.vpk"))
    }

    pub fn read_entry(
        &self,
        logical_path: &str,
        segments: &impl SegmentReader,
    ) -> Result<ReadResult, Error> {
        let entry = self.entry(logical_path)?;
        let mut result = self.read_entry_range(entry, 0..entry.total_length, segments)?;
        if crc32fast::hash(&result.bytes) != entry.crc32 {
            return Err(entry_error(ErrorCode::CrcMismatch, &self.identity, entry));
        }
        result.crc32 = Integrity::Verified;
        Ok(result)
    }

    pub fn read_range(
        &self,
        logical_path: &str,
        range: Range<usize>,
        segments: &impl SegmentReader,
    ) -> Result<ReadResult, Error> {
        let entry = self.entry(logical_path)?;
        self.read_entry_range(entry, range, segments)
    }

    pub fn verify_self(&self) -> SelfIntegrity {
        let Some(expected) = self.other_md5 else {
            return SelfIntegrity {
                tree_md5: Integrity::NotPresent,
                archive_md5: Integrity::NotPresent,
                whole_file_md5: Integrity::NotPresent,
                signature: self.verify_signature(None),
            };
        };
        let whole_end = self.sections.other_md5.start + 32;
        SelfIntegrity {
            tree_md5: integrity(
                md5(&self.directory_bytes[self.sections.tree.clone()]),
                expected.tree,
            ),
            archive_md5: integrity(
                md5(&self.directory_bytes[self.sections.archive_md5.clone()]),
                expected.archive_md5,
            ),
            whole_file_md5: integrity(md5(&self.directory_bytes[..whole_end]), expected.whole_file),
            signature: self.verify_signature(None),
        }
    }

    pub fn verify_archive_record(
        &self,
        record_index: usize,
        segments: &impl SegmentReader,
    ) -> Result<Integrity, Error> {
        let Some(record) = self.archive_md5.get(record_index) else {
            return Err(error(
                ErrorCode::InvalidReadRange,
                &self.identity,
                record_index as u64..record_index as u64,
                Some(record_index as u64),
                Some(self.archive_md5.len() as u64),
            ));
        };
        let bytes = if record.archive_index == EMBEDDED_ARCHIVE as u32 {
            let start = self
                .sections
                .file_data
                .start
                .checked_add(record.offset as usize)
                .ok_or_else(|| md5_record_error(&self.identity, record))?;
            let end = start
                .checked_add(record.length as usize)
                .ok_or_else(|| md5_record_error(&self.identity, record))?;
            if end > self.sections.file_data.end {
                return Err(md5_record_error(&self.identity, record));
            }
            self.directory_bytes[start..end].to_vec()
        } else {
            read_segment(
                &self.identity,
                None,
                record.archive_index,
                record.offset as u64..record.offset as u64 + record.length as u64,
                segments,
            )?
        };
        Ok(integrity(md5(&bytes), record.digest))
    }

    fn read_entry_range(
        &self,
        entry: &Entry,
        range: Range<usize>,
        segments: &impl SegmentReader,
    ) -> Result<ReadResult, Error> {
        if range.start > range.end || range.end > entry.total_length {
            let mut result = entry_error(ErrorCode::InvalidReadRange, &self.identity, entry);
            result.range = range.start as u64..range.end as u64;
            return Err(result);
        }
        let mut bytes = Vec::with_capacity(range.end - range.start);
        let preload_length = entry.preload_range.len();
        let preload_start = range.start.min(preload_length);
        let preload_end = range.end.min(preload_length);
        if preload_start < preload_end {
            bytes.extend_from_slice(
                &self.directory_bytes[entry.preload_range.start + preload_start
                    ..entry.preload_range.start + preload_end],
            );
        }
        let data_start = range.start.saturating_sub(preload_length);
        let data_end = range.end.saturating_sub(preload_length);
        if data_start < data_end {
            let source_start = entry.data.offset as u64 + data_start as u64;
            let source_end = entry.data.offset as u64 + data_end as u64;
            if entry.data.archive_index == EMBEDDED_ARCHIVE {
                let start = self.sections.file_data.start + source_start as usize;
                let end = self.sections.file_data.start + source_end as usize;
                bytes.extend_from_slice(&self.directory_bytes[start..end]);
            } else {
                bytes.extend_from_slice(&read_segment(
                    &self.identity,
                    Some(&entry.logical_path),
                    entry.data.archive_index as u32,
                    source_start..source_end,
                    segments,
                )?);
            }
        }
        Ok(ReadResult {
            bytes,
            crc32: Integrity::NotEvaluated,
        })
    }

    pub fn verify_signature(&self, expected_public_key: Option<&[u8]>) -> SignatureIntegrity {
        let Some(material) = &self.signature else {
            return SignatureIntegrity::NotPresent;
        };
        if expected_public_key.is_some_and(|expected| expected != material.public_key) {
            return SignatureIntegrity::WrongKey;
        }
        let Ok(key) = RsaPublicKey::from_public_key_der(&material.public_key) else {
            return SignatureIntegrity::Invalid;
        };
        let Ok(signature) = Signature::try_from(material.signature.as_slice()) else {
            return SignatureIntegrity::Invalid;
        };
        let verifier = VerifyingKey::<Sha256>::new(key);
        if verifier
            .verify(
                &self.directory_bytes[material.signed_range.clone()],
                &signature,
            )
            .is_ok()
        {
            SignatureIntegrity::Verified
        } else {
            SignatureIntegrity::Invalid
        }
    }
}

fn sections(
    bytes: &[u8],
    identity: &str,
    version: u32,
    tree_size: usize,
) -> Result<Sections, Error> {
    let (header_end, sizes) = match version {
        1 => (
            12_usize,
            [
                tree_size,
                bytes.len().saturating_sub(12 + tree_size),
                0,
                0,
                0,
            ],
        ),
        2 => {
            if bytes.len() < 28 {
                return Err(error(
                    ErrorCode::TruncatedHeader,
                    identity,
                    bytes.len() as u64..28,
                    None,
                    None,
                ));
            }
            (
                28,
                [
                    tree_size,
                    u32_at(bytes, 12) as usize,
                    u32_at(bytes, 16) as usize,
                    u32_at(bytes, 20) as usize,
                    u32_at(bytes, 24) as usize,
                ],
            )
        }
        _ => {
            return Err(error(
                ErrorCode::UnsupportedVersion,
                identity,
                4..8,
                Some(version as u64),
                None,
            ));
        }
    };
    let mut cursor = header_end;
    let mut next = |length: usize| -> Option<Range<usize>> {
        let start = cursor;
        cursor = cursor.checked_add(length)?;
        Some(start..cursor)
    };
    let tree = next(sizes[0]).ok_or_else(|| section_error(identity, bytes.len()))?;
    let file_data = next(sizes[1]).ok_or_else(|| section_error(identity, bytes.len()))?;
    let archive_md5 = next(sizes[2]).ok_or_else(|| section_error(identity, bytes.len()))?;
    let other_md5 = next(sizes[3]).ok_or_else(|| section_error(identity, bytes.len()))?;
    let signature = next(sizes[4]).ok_or_else(|| section_error(identity, bytes.len()))?;
    if cursor != bytes.len() {
        return Err(section_error(identity, bytes.len()));
    }
    Ok(Sections {
        header: 0..header_end,
        tree,
        file_data,
        archive_md5,
        other_md5,
        signature,
    })
}

fn parse_tree(
    bytes: &[u8],
    identity: &str,
    sections: &Sections,
    limits: Limits,
) -> Result<Vec<Entry>, Error> {
    let tree = &bytes[sections.tree.clone()];
    let mut cursor = 0;
    let mut entries = Vec::new();
    let mut lookup = BTreeMap::new();
    let mut total_preload = 0_usize;
    loop {
        let extension = tree_string(tree, &mut cursor, identity, limits)?;
        if extension.is_empty() {
            break;
        }
        loop {
            let directory = tree_string(tree, &mut cursor, identity, limits)?;
            if directory.is_empty() {
                break;
            }
            loop {
                let basename = tree_string(tree, &mut cursor, identity, limits)?;
                if basename.is_empty() {
                    break;
                }
                if entries.len() == limits.max_entries {
                    return Err(error(
                        ErrorCode::EntryLimit,
                        identity,
                        cursor as u64..cursor as u64,
                        Some(entries.len() as u64 + 1),
                        Some(limits.max_entries as u64),
                    ));
                }
                let metadata_end = cursor
                    .checked_add(18)
                    .ok_or_else(|| tree_error(identity, cursor, tree.len()))?;
                if metadata_end > tree.len() {
                    return Err(tree_error(identity, cursor, tree.len()));
                }
                let crc32 = u32_at(tree, cursor);
                let preload_length = u16_at(tree, cursor + 4) as usize;
                let data = DataDescriptor {
                    archive_index: u16_at(tree, cursor + 6),
                    offset: u32_at(tree, cursor + 8),
                    length: u32_at(tree, cursor + 12),
                };
                if u16_at(tree, cursor + 16) != ENTRY_TERMINATOR {
                    return Err(error(
                        ErrorCode::InvalidDescriptor,
                        identity,
                        cursor as u64 + 16..cursor as u64 + 18,
                        None,
                        None,
                    ));
                }
                cursor = metadata_end;
                let preload_end = cursor
                    .checked_add(preload_length)
                    .ok_or_else(|| tree_error(identity, cursor, tree.len()))?;
                if preload_end > tree.len() {
                    return Err(tree_error(identity, cursor, tree.len()));
                }
                total_preload = total_preload
                    .checked_add(preload_length)
                    .ok_or_else(|| error(ErrorCode::PreloadLimit, identity, 0..0, None, None))?;
                if total_preload > limits.max_total_preload_bytes {
                    return Err(error(
                        ErrorCode::PreloadLimit,
                        identity,
                        cursor as u64..preload_end as u64,
                        Some(total_preload as u64),
                        Some(limits.max_total_preload_bytes as u64),
                    ));
                }
                let (stored_path, logical_path) = path(
                    &extension,
                    &directory,
                    &basename,
                    identity,
                    limits.max_logical_path_bytes,
                )?;
                if lookup.insert(logical_path.clone(), entries.len()).is_some() {
                    let mut result = error(
                        ErrorCode::DuplicatePath,
                        identity,
                        cursor as u64..preload_end as u64,
                        None,
                        None,
                    );
                    result.path = Some(logical_path);
                    return Err(result);
                }
                let total_length = preload_length
                    .checked_add(data.length as usize)
                    .ok_or_else(|| tree_error(identity, cursor, tree.len()))?;
                if total_length > limits.max_entry_bytes {
                    return Err(error(
                        ErrorCode::EntrySizeLimit,
                        identity,
                        cursor as u64..preload_end as u64,
                        Some(total_length as u64),
                        Some(limits.max_entry_bytes as u64),
                    ));
                }
                entries.push(Entry {
                    extension: extension.clone(),
                    directory: directory.clone(),
                    basename,
                    stored_path,
                    logical_path,
                    crc32,
                    preload_range: sections.tree.start + cursor..sections.tree.start + preload_end,
                    data,
                    total_length,
                });
                cursor = preload_end;
            }
        }
    }
    if cursor != tree.len() {
        return Err(tree_error(identity, cursor, tree.len()));
    }
    Ok(entries)
}

fn parse_archive_md5(
    bytes: &[u8],
    identity: &str,
    sections: &Sections,
    limits: Limits,
) -> Result<Vec<ArchiveMd5Record>, Error> {
    let section = &bytes[sections.archive_md5.clone()];
    if !section.len().is_multiple_of(28) {
        return Err(error(
            ErrorCode::InvalidIntegritySection,
            identity,
            sections.archive_md5.start as u64..sections.archive_md5.end as u64,
            Some(section.len() as u64),
            Some(28),
        ));
    }
    let count = section.len() / 28;
    if count > limits.max_integrity_records {
        return Err(error(
            ErrorCode::IntegrityLimit,
            identity,
            sections.archive_md5.start as u64..sections.archive_md5.end as u64,
            Some(count as u64),
            Some(limits.max_integrity_records as u64),
        ));
    }
    let records: Vec<_> = section
        .chunks_exact(28)
        .map(|record| ArchiveMd5Record {
            archive_index: u32_at(record, 0),
            offset: u32_at(record, 4),
            length: u32_at(record, 8),
            digest: record[12..28].try_into().expect("fixed MD5 record"),
        })
        .collect();
    if let Some(record) = records
        .iter()
        .find(|record| record.length as usize > limits.max_integrity_read_bytes)
    {
        return Err(error(
            ErrorCode::IntegrityLimit,
            identity,
            sections.archive_md5.start as u64..sections.archive_md5.end as u64,
            Some(record.length as u64),
            Some(limits.max_integrity_read_bytes as u64),
        ));
    }
    Ok(records)
}

fn parse_other_md5(
    bytes: &[u8],
    identity: &str,
    sections: &Sections,
) -> Result<Option<OtherMd5>, Error> {
    let section = &bytes[sections.other_md5.clone()];
    match section.len() {
        0 => Ok(None),
        48 => Ok(Some(OtherMd5 {
            tree: section[0..16].try_into().expect("fixed MD5 field"),
            archive_md5: section[16..32].try_into().expect("fixed MD5 field"),
            whole_file: section[32..48].try_into().expect("fixed MD5 field"),
        })),
        _ => Err(error(
            ErrorCode::InvalidIntegritySection,
            identity,
            sections.other_md5.start as u64..sections.other_md5.end as u64,
            Some(section.len() as u64),
            Some(48),
        )),
    }
}

fn parse_signature(
    bytes: &[u8],
    identity: &str,
    sections: &Sections,
    limits: Limits,
) -> Result<Option<SignatureMaterial>, Error> {
    let section = &bytes[sections.signature.clone()];
    if section.is_empty() {
        return Ok(None);
    }
    if section.len() < 8 {
        return Err(signature_error(identity, sections));
    }
    let key_length = u32_at(section, 0) as usize;
    if key_length > limits.max_public_key_bytes {
        return Err(error(
            ErrorCode::IntegrityLimit,
            identity,
            sections.signature.start as u64..sections.signature.end as u64,
            Some(key_length as u64),
            Some(limits.max_public_key_bytes as u64),
        ));
    }
    let signature_length_offset = 4_usize
        .checked_add(key_length)
        .ok_or_else(|| signature_error(identity, sections))?;
    if signature_length_offset + 4 > section.len() {
        return Err(signature_error(identity, sections));
    }
    let signature_length = u32_at(section, signature_length_offset) as usize;
    if signature_length > limits.max_signature_bytes {
        return Err(error(
            ErrorCode::IntegrityLimit,
            identity,
            sections.signature.start as u64..sections.signature.end as u64,
            Some(signature_length as u64),
            Some(limits.max_signature_bytes as u64),
        ));
    }
    let end = signature_length_offset
        .checked_add(4)
        .and_then(|value| value.checked_add(signature_length))
        .ok_or_else(|| signature_error(identity, sections))?;
    if end != section.len() {
        return Err(signature_error(identity, sections));
    }
    Ok(Some(SignatureMaterial {
        public_key: section[4..signature_length_offset].to_vec(),
        signature: section[signature_length_offset + 4..end].to_vec(),
        signed_range: 0..sections.signature.start,
    }))
}

fn tree_string(
    tree: &[u8],
    cursor: &mut usize,
    identity: &str,
    limits: Limits,
) -> Result<Vec<u8>, Error> {
    let Some(remaining) = tree.get(*cursor..) else {
        return Err(tree_error(identity, *cursor, tree.len()));
    };
    let Some(length) = remaining.iter().position(|byte| *byte == 0) else {
        return Err(tree_error(identity, *cursor, tree.len()));
    };
    if length > limits.max_component_bytes {
        return Err(error(
            ErrorCode::InvalidComponent,
            identity,
            *cursor as u64..(*cursor + length) as u64,
            Some(length as u64),
            Some(limits.max_component_bytes as u64),
        ));
    }
    let value = remaining[..length].to_vec();
    *cursor += length + 1;
    Ok(value)
}

fn path(
    extension: &[u8],
    directory: &[u8],
    basename: &[u8],
    identity: &str,
    max_length: usize,
) -> Result<(String, String), Error> {
    let extension = component(extension, identity, false)?;
    let directory = component(directory, identity, true)?;
    let basename = component(basename, identity, false)?;
    let extension = (extension != " ").then_some(extension);
    let directory = (directory != " ").then_some(directory);
    let basename = (basename != " ").then_some(basename);
    let Some(basename) = basename else {
        return Err(error(
            ErrorCode::InvalidComponent,
            identity,
            0..0,
            None,
            None,
        ));
    };
    let filename = match extension {
        Some(extension) => format!("{basename}.{extension}"),
        None => basename,
    };
    let stored = match directory {
        Some(directory) => format!("{directory}/{filename}"),
        None => filename,
    };
    if stored.len() > max_length || !valid_path(&stored) {
        return Err(error(
            ErrorCode::InvalidComponent,
            identity,
            0..0,
            Some(stored.len() as u64),
            Some(max_length as u64),
        ));
    }
    Ok((stored.clone(), stored.to_ascii_lowercase()))
}

fn component(bytes: &[u8], identity: &str, directory: bool) -> Result<String, Error> {
    if bytes.is_empty()
        || !bytes.is_ascii()
        || bytes
            .iter()
            .any(|byte| byte.is_ascii_control() || *byte == b'\\' || (!directory && *byte == b'/'))
    {
        return Err(error(
            ErrorCode::InvalidComponent,
            identity,
            0..0,
            None,
            None,
        ));
    }
    Ok(String::from_utf8(bytes.to_vec()).expect("ASCII is UTF-8"))
}

fn canonical_request(path: &str, identity: &str, max_length: usize) -> Result<String, Error> {
    if !path.is_ascii() || path.is_empty() || !valid_path(path) || path.len() > max_length {
        let mut result = error(ErrorCode::InvalidComponent, identity, 0..0, None, None);
        result.path = Some(path.to_owned());
        return Err(result);
    }
    Ok(path.to_ascii_lowercase())
}

fn valid_path(path: &str) -> bool {
    !path.starts_with('/')
        && !path.contains('\\')
        && !path.bytes().any(|byte| byte.is_ascii_control())
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn validate_identity(identity: &str, layout: Layout) -> Result<(), Error> {
    let valid = identity.is_ascii()
        && valid_path(identity)
        && match layout {
            Layout::Split => identity.ends_with("_dir.vpk"),
            Layout::Standalone => identity.ends_with(".vpk") && !identity.ends_with("_dir.vpk"),
        };
    if valid {
        Ok(())
    } else {
        Err(error(
            ErrorCode::InvalidIdentity,
            identity,
            0..0,
            None,
            None,
        ))
    }
}

fn read_segment(
    identity: &str,
    path: Option<&str>,
    archive_index: u32,
    range: Range<u64>,
    segments: &impl SegmentReader,
) -> Result<Vec<u8>, Error> {
    let length = segments
        .len(archive_index)
        .map_err(|source| source_error(identity, path, archive_index, source))?;
    if range.end > length || range.start > range.end {
        let mut result = error(
            ErrorCode::ChangedSource,
            identity,
            range,
            Some(length),
            None,
        );
        result.path = path.map(str::to_owned);
        result.segment = Some(archive_index);
        return Err(result);
    }
    let expected = range.end - range.start;
    let bytes = segments
        .read(archive_index, range.clone())
        .map_err(|source| source_error(identity, path, archive_index, source))?;
    if bytes.len() as u64 != expected {
        let mut result = error(
            ErrorCode::ShortRead,
            identity,
            range,
            Some(bytes.len() as u64),
            Some(expected),
        );
        result.path = path.map(str::to_owned);
        result.segment = Some(archive_index);
        return Err(result);
    }
    Ok(bytes)
}

fn source_error(
    identity: &str,
    path: Option<&str>,
    archive_index: u32,
    source: SourceError,
) -> Error {
    let code = match source.code {
        SourceErrorCode::Missing => ErrorCode::MissingSegment,
        SourceErrorCode::Changed => ErrorCode::ChangedSource,
        SourceErrorCode::ShortRead => ErrorCode::ShortRead,
        SourceErrorCode::Io => ErrorCode::SourceIo,
    };
    let mut result = error(code, identity, source.range, None, None);
    result.path = path.map(str::to_owned);
    result.segment = Some(archive_index);
    result
}

fn integrity<T: Eq>(actual: T, expected: T) -> Integrity {
    if actual == expected {
        Integrity::Verified
    } else {
        Integrity::Invalid
    }
}

fn md5(bytes: &[u8]) -> [u8; 16] {
    Md5::digest(bytes).into()
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated VPK field"),
    )
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated VPK field"),
    )
}

fn error(
    code: ErrorCode,
    identity: &str,
    range: Range<u64>,
    declared: Option<u64>,
    limit: Option<u64>,
) -> Error {
    Error {
        code,
        archive: identity.to_owned(),
        path: None,
        segment: None,
        range,
        declared,
        limit,
    }
}

fn section_error(identity: &str, length: usize) -> Error {
    error(
        ErrorCode::InvalidSections,
        identity,
        0..length as u64,
        Some(length as u64),
        None,
    )
}

fn tree_error(identity: &str, cursor: usize, length: usize) -> Error {
    error(
        ErrorCode::InvalidTree,
        identity,
        cursor as u64..length as u64,
        None,
        None,
    )
}

fn signature_error(identity: &str, sections: &Sections) -> Error {
    error(
        ErrorCode::InvalidIntegritySection,
        identity,
        sections.signature.start as u64..sections.signature.end as u64,
        None,
        None,
    )
}

fn entry_error(code: ErrorCode, identity: &str, entry: &Entry) -> Error {
    let mut result = error(
        code,
        identity,
        entry.data.offset as u64..entry.data.offset as u64 + entry.data.length as u64,
        None,
        None,
    );
    result.path = Some(entry.logical_path.clone());
    result.segment = Some(entry.data.archive_index as u32);
    result
}

fn md5_record_error(identity: &str, record: &ArchiveMd5Record) -> Error {
    let mut result = error(
        ErrorCode::ChangedSource,
        identity,
        record.offset as u64..record.offset as u64 + record.length as u64,
        None,
        None,
    );
    result.segment = Some(record.archive_index);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Segments(BTreeMap<u32, Vec<u8>>);

    impl SegmentReader for Segments {
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
            let bytes = self.0.get(&archive_index).ok_or(SourceError {
                code: SourceErrorCode::Missing,
                range: range.clone(),
            })?;
            bytes
                .get(range.start as usize..range.end as usize)
                .map(<[u8]>::to_vec)
                .ok_or(SourceError {
                    code: SourceErrorCode::ShortRead,
                    range,
                })
        }
    }

    fn tree(
        extension: &[u8],
        directory: &[u8],
        basename: &[u8],
        preload: &[u8],
        archive_index: u16,
        offset: u32,
        data: &[u8],
    ) -> Vec<u8> {
        let mut complete = preload.to_vec();
        complete.extend_from_slice(data);
        let mut tree = Vec::new();
        for component in [extension, directory, basename] {
            tree.extend_from_slice(component);
            tree.push(0);
        }
        tree.extend_from_slice(&crc32fast::hash(&complete).to_le_bytes());
        tree.extend_from_slice(&(preload.len() as u16).to_le_bytes());
        tree.extend_from_slice(&archive_index.to_le_bytes());
        tree.extend_from_slice(&offset.to_le_bytes());
        tree.extend_from_slice(&(data.len() as u32).to_le_bytes());
        tree.extend_from_slice(&ENTRY_TERMINATOR.to_le_bytes());
        tree.extend_from_slice(preload);
        tree.extend_from_slice(&[0, 0, 0]);
        tree
    }

    fn v1(tree: &[u8], embedded: &[u8]) -> Vec<u8> {
        let mut bytes = SIGNATURE.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&(tree.len() as u32).to_le_bytes());
        bytes.extend_from_slice(tree);
        bytes.extend_from_slice(embedded);
        bytes
    }

    fn v2(
        tree: &[u8],
        file_data: &[u8],
        archive_md5: &[u8],
        with_self_md5: bool,
        signature: &[u8],
    ) -> Vec<u8> {
        let other_length = if with_self_md5 { 48 } else { 0 };
        let mut bytes = SIGNATURE.to_le_bytes().to_vec();
        for field in [
            2_u32,
            tree.len() as u32,
            file_data.len() as u32,
            archive_md5.len() as u32,
            other_length,
            signature.len() as u32,
        ] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        bytes.extend_from_slice(tree);
        bytes.extend_from_slice(file_data);
        bytes.extend_from_slice(archive_md5);
        if with_self_md5 {
            bytes.extend_from_slice(&md5(tree));
            bytes.extend_from_slice(&md5(archive_md5));
            let whole = md5(&bytes);
            bytes.extend_from_slice(&whole);
        }
        bytes.extend_from_slice(signature);
        bytes
    }

    #[test]
    fn parses_v1_tree_and_reads_preload_embedded_and_ranges() {
        let tree = tree(
            b"vmt",
            b"materials/test",
            b"Example",
            b"pre",
            EMBEDDED_ARCHIVE,
            0,
            b"data",
        );
        let bytes = v1(&tree, b"data");
        let archive = parse(&bytes, "pak01.vpk", Layout::Standalone, Limits::default()).unwrap();
        assert_eq!(archive.version, 1);
        assert_eq!(archive.entries[0].stored_path, "materials/test/Example.vmt");
        assert_eq!(
            archive.entries[0].logical_path,
            "materials/test/example.vmt"
        );
        let result = archive
            .read_entry("MATERIALS/test/EXAMPLE.VMT", &Segments::default())
            .unwrap();
        assert_eq!(result.bytes, b"predata");
        assert_eq!(result.crc32, Integrity::Verified);
        assert_eq!(
            archive
                .read_range("materials/test/example.vmt", 2..5, &Segments::default())
                .unwrap(),
            ReadResult {
                bytes: b"eda".to_vec(),
                crc32: Integrity::NotEvaluated,
            }
        );
    }

    #[test]
    fn parses_v2_sections_reads_segments_and_verifies_md5() {
        let segment = b"xxsegment-datayy".to_vec();
        let tree = tree(b"vtf", b"materials", b"a", b"p", 3, 2, b"segment-data");
        let mut archive_md5 = Vec::new();
        archive_md5.extend_from_slice(&3_u32.to_le_bytes());
        archive_md5.extend_from_slice(&2_u32.to_le_bytes());
        archive_md5.extend_from_slice(&12_u32.to_le_bytes());
        archive_md5.extend_from_slice(&md5(b"segment-data"));
        let bytes = v2(&tree, &[], &archive_md5, true, &[]);
        let archive = parse(
            &bytes,
            "tf2_textures_dir.vpk",
            Layout::Split,
            Limits::default(),
        )
        .unwrap();
        let segments = Segments(BTreeMap::from([(3, segment)]));
        assert_eq!(archive.segment_identity(3).unwrap(), "tf2_textures_003.vpk");
        assert_eq!(
            archive
                .read_entry("materials/a.vtf", &segments)
                .unwrap()
                .bytes,
            b"psegment-data"
        );
        assert_eq!(
            archive.verify_self(),
            SelfIntegrity {
                tree_md5: Integrity::Verified,
                archive_md5: Integrity::Verified,
                whole_file_md5: Integrity::Verified,
                signature: SignatureIntegrity::NotPresent,
            }
        );
        assert_eq!(
            archive.verify_archive_record(0, &segments),
            Ok(Integrity::Verified)
        );
    }

    #[test]
    fn rejects_structure_identity_duplicates_limits_crc_and_missing_segments() {
        let tree = tree(b"vmt", b" ", b"a", b"", 0, 0, b"data");
        let bytes = v2(&tree, &[], &[], false, &[]);
        assert_eq!(
            parse(&bytes, "bad.vpk", Layout::Split, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidIdentity
        );
        let archive = parse(&bytes, "a_dir.vpk", Layout::Split, Limits::default()).unwrap();
        assert_eq!(
            archive
                .read_entry("a.vmt", &Segments::default())
                .unwrap_err()
                .code,
            ErrorCode::MissingSegment
        );

        let mut bad_crc = bytes.clone();
        let crc_offset = 28 + b"vmt\0 \0a\0".len();
        bad_crc[crc_offset] ^= 1;
        let archive = parse(&bad_crc, "a_dir.vpk", Layout::Split, Limits::default()).unwrap();
        let segments = Segments(BTreeMap::from([(0, b"data".to_vec())]));
        assert_eq!(
            archive.read_entry("a.vmt", &segments).unwrap_err().code,
            ErrorCode::CrcMismatch
        );

        let mut duplicate_tree = tree.clone();
        duplicate_tree.truncate(duplicate_tree.len() - 3);
        duplicate_tree.extend_from_slice(&tree[b"vmt\0".len() + b" \0".len()..tree.len() - 3]);
        duplicate_tree.extend_from_slice(&[0, 0, 0]);
        let duplicate = v2(&duplicate_tree, &[], &[], false, &[]);
        assert_eq!(
            parse(&duplicate, "a_dir.vpk", Layout::Split, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::DuplicatePath
        );

        let limits = Limits {
            max_entries: 0,
            ..Limits::default()
        };
        assert_eq!(
            parse(&bytes, "a_dir.vpk", Layout::Split, limits)
                .unwrap_err()
                .code,
            ErrorCode::EntryLimit
        );
    }

    #[test]
    fn rejects_truncated_sections_tree_and_signature_framing() {
        assert_eq!(
            parse(&[], "a_dir.vpk", Layout::Split, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::TruncatedHeader
        );
        let tree = tree(b"vmt", b" ", b"a", b"", 0, 0, b"");
        let mut trailing = v2(&tree, &[], &[], false, &[]);
        trailing.push(1);
        assert_eq!(
            parse(&trailing, "a_dir.vpk", Layout::Split, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidSections
        );
        let malformed_signature = v2(&tree, &[], &[], false, &[1, 0, 0, 0, 0]);
        assert_eq!(
            parse(
                &malformed_signature,
                "a_dir.vpk",
                Layout::Split,
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidIntegritySection
        );
    }
}
