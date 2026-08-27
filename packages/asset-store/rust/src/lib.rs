use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::sync::Mutex;

use flate2::{Compression, write::DeflateEncoder};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_CHUNK_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CHUNK_ENTRIES: usize = 2_048;
pub const MAX_CHUNK_ROLES: usize = 2_048;
pub const MAX_GRAPH_ENTRIES: usize = 8_192;
pub const MAX_GRAPH_CHUNKS: usize = 1_024;
pub const MAX_LOGICAL_PATH_BYTES: usize = 4_096;
pub const LARGE_RESOURCE_BYTES: usize = 4 * 1024 * 1024;
const COMPRESSION_PERCENT: usize = 95;
const CHUNK_HEADER_BYTES: usize = 12;
const CHUNK_ENTRY_FIXED_BYTES: usize = 4 + 8 + 8 + 32;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    Identity,
    Deflate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub logical_path: String,
    pub roles: BTreeSet<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDescriptor {
    pub logical_path: String,
    pub offset: String,
    pub byte_length: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDescriptor {
    pub codec: Codec,
    pub encoded_byte_length: String,
    pub encoded_sha256: String,
    pub decoded_byte_length: String,
    pub decoded_sha256: String,
    pub roles: Vec<String>,
    pub entries: Vec<EntryDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedChunk {
    pub descriptor: ChunkDescriptor,
    pub encoded: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGraph {
    pub schema: String,
    pub game: String,
    pub content_build: String,
    pub target: String,
    pub chunks: Vec<ChunkDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEntry {
    pub logical_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphError {
    MalformedIdentity,
    DuplicateIdentity,
    BoundExceeded,
    IntegrityFailure,
    MalformedChunk,
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn hex_hash(bytes: &[u8]) -> String {
    hash(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, GraphError> {
    fn write(value: &serde_json::Value, output: &mut Vec<u8>) -> Result<(), GraphError> {
        match value {
            serde_json::Value::Null => output.extend_from_slice(b"null"),
            serde_json::Value::Bool(value) => {
                output.extend_from_slice(if *value { b"true" } else { b"false" })
            }
            serde_json::Value::Number(value) => {
                if !value.is_i64() && !value.is_u64() {
                    return Err(GraphError::MalformedIdentity);
                }
                output.extend_from_slice(value.to_string().as_bytes());
            }
            serde_json::Value::String(value) => {
                serde_json::to_writer(output, value).map_err(|_| GraphError::MalformedIdentity)?
            }
            serde_json::Value::Array(values) => {
                output.push(b'[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        output.push(b',');
                    }
                    write(value, output)?;
                }
                output.push(b']');
            }
            serde_json::Value::Object(values) => {
                output.push(b'{');
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort();
                for (index, key) in keys.into_iter().enumerate() {
                    if !key.is_ascii() {
                        return Err(GraphError::MalformedIdentity);
                    }
                    if index > 0 {
                        output.push(b',');
                    }
                    serde_json::to_writer(&mut *output, key)
                        .map_err(|_| GraphError::MalformedIdentity)?;
                    output.push(b':');
                    write(&values[key], output)?;
                }
                output.push(b'}');
            }
        }
        Ok(())
    }
    let value = serde_json::to_value(value).map_err(|_| GraphError::MalformedIdentity)?;
    let mut output = Vec::new();
    write(&value, &mut output)?;
    Ok(output)
}

fn valid_logical_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_LOGICAL_PATH_BYTES
        && !path.bytes().any(|byte| byte.is_ascii_uppercase())
        && !path.starts_with('/')
        && !path.ends_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && !path.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
}

fn valid_role(role: &str) -> bool {
    !role.is_empty()
        && role.len() <= 64
        && role
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn write_u32(output: &mut Vec<u8>, value: usize) -> Result<(), GraphError> {
    output.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| GraphError::BoundExceeded)?
            .to_le_bytes(),
    );
    Ok(())
}

fn write_u64(output: &mut Vec<u8>, value: usize) -> Result<(), GraphError> {
    output.extend_from_slice(
        &u64::try_from(value)
            .map_err(|_| GraphError::BoundExceeded)?
            .to_le_bytes(),
    );
    Ok(())
}

fn decoded_chunk(resources: &[&Resource]) -> Result<(Vec<u8>, Vec<EntryDescriptor>), GraphError> {
    if resources.is_empty() || resources.len() > MAX_CHUNK_ENTRIES {
        return Err(GraphError::BoundExceeded);
    }
    let table_bytes = resources
        .iter()
        .try_fold(CHUNK_HEADER_BYTES, |total, resource| {
            total
                .checked_add(CHUNK_ENTRY_FIXED_BYTES)
                .and_then(|value| value.checked_add(resource.logical_path.len()))
                .ok_or(GraphError::BoundExceeded)
        })?;
    let complete_bytes = resources.iter().try_fold(table_bytes, |total, resource| {
        total
            .checked_add(resource.bytes.len())
            .ok_or(GraphError::BoundExceeded)
    })?;
    if complete_bytes > MAX_CHUNK_BYTES {
        return Err(GraphError::BoundExceeded);
    }
    let mut output = Vec::with_capacity(complete_bytes);
    output.extend_from_slice(b"PSCH");
    output.extend_from_slice(&1_u32.to_le_bytes());
    write_u32(&mut output, resources.len())?;
    let mut offset = table_bytes;
    let mut descriptors = Vec::with_capacity(resources.len());
    for resource in resources {
        write_u32(&mut output, resource.logical_path.len())?;
        output.extend_from_slice(resource.logical_path.as_bytes());
        write_u64(&mut output, offset)?;
        write_u64(&mut output, resource.bytes.len())?;
        let digest = hash(&resource.bytes);
        output.extend_from_slice(&digest);
        descriptors.push(EntryDescriptor {
            logical_path: resource.logical_path.clone(),
            offset: offset.to_string(),
            byte_length: resource.bytes.len().to_string(),
            sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
        });
        offset += resource.bytes.len();
    }
    for resource in resources {
        output.extend_from_slice(&resource.bytes);
    }
    Ok((output, descriptors))
}

pub fn valid_resource_identity(resource: &Resource) -> bool {
    valid_logical_path(&resource.logical_path) && !resource.roles.is_empty() && resource.roles.iter().all(|role| valid_role(role))
}

pub fn pack(resources: Vec<Resource>) -> Result<Vec<PackedChunk>, GraphError> {
    if resources.is_empty() || resources.len() > MAX_GRAPH_ENTRIES {
        return Err(GraphError::BoundExceeded);
    }
    let mut identities = BTreeSet::new();
    let mut groups = BTreeMap::<(Vec<String>, String, u8, String), Vec<Resource>>::new();
    for resource in resources {
        if resource.roles.len() > MAX_CHUNK_ROLES { return Err(GraphError::BoundExceeded); }
        if !valid_resource_identity(&resource) {
            return Err(GraphError::MalformedIdentity);
        }
        if !identities.insert(resource.logical_path.clone()) {
            return Err(GraphError::DuplicateIdentity);
        }
        let roles = resource.roles.iter().cloned().collect::<Vec<_>>();
        let path_hash = hash(resource.logical_path.as_bytes());
        // Directory regions keep unrelated map additions from changing a shared
        // model/texture section. Within each region the path (not content) picks
        // a stable shard; entry bytes and the entire table remain authenticated.
        let region = resource
            .logical_path
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent)
            .to_owned();
        let bucket = path_hash[0] >> 7;
        let individual = if resource.bytes.len() >= LARGE_RESOURCE_BYTES {
            resource.logical_path.clone()
        } else {
            String::new()
        };
        groups
            .entry((roles, region, bucket, individual))
            .or_default()
            .push(resource);
    }

    let mut bounded = Vec::new();
    for ((roles, _, _, _), mut resources) in groups {
        resources.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
        let mut section = Vec::new();
        let mut bytes = CHUNK_HEADER_BYTES;
        for resource in resources {
            let length =
                CHUNK_ENTRY_FIXED_BYTES + resource.logical_path.len() + resource.bytes.len();
            if length > MAX_CHUNK_BYTES - CHUNK_HEADER_BYTES {
                return Err(GraphError::BoundExceeded);
            }
            if bytes + length > MAX_CHUNK_BYTES || section.len() == MAX_CHUNK_ENTRIES {
                bounded.push((roles.clone(), std::mem::take(&mut section)));
                bytes = CHUNK_HEADER_BYTES;
            }
            section.push(resource);
            bytes += length;
        }
        bounded.push((roles, section));
        if bounded.len() > MAX_GRAPH_CHUNKS {
            return Err(GraphError::BoundExceeded);
        }
    }
    let groups = bounded;
    let results = groups
        .into_par_iter()
        .map(|(roles, resources)| {
            let references = resources.iter().collect::<Vec<_>>();
            let (decoded, entries) = decoded_chunk(&references)?;
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
            encoder
                .write_all(&decoded)
                .map_err(|_| GraphError::IntegrityFailure)?;
            let compressed = encoder.finish().map_err(|_| GraphError::IntegrityFailure)?;
            let (codec, encoded) = if compressed.len().saturating_mul(100)
                <= decoded.len().saturating_mul(COMPRESSION_PERCENT)
            {
                (Codec::Deflate, compressed)
            } else {
                (Codec::Identity, decoded.clone())
            };
            if encoded.len() > MAX_CHUNK_BYTES {
                return Err(GraphError::BoundExceeded);
            }
            Ok(PackedChunk {
                descriptor: ChunkDescriptor {
                    codec,
                    encoded_byte_length: encoded.len().to_string(),
                    encoded_sha256: hex_hash(&encoded),
                    decoded_byte_length: decoded.len().to_string(),
                    decoded_sha256: hex_hash(&decoded),
                    roles,
                    entries,
                },
                encoded,
            })
        })
        .collect::<Vec<Result<PackedChunk, GraphError>>>();
    let mut packed = results.into_iter().collect::<Result<Vec<_>, _>>()?;
    packed.sort_by(|left, right| {
        left.descriptor
            .encoded_sha256
            .cmp(&right.descriptor.encoded_sha256)
    });
    Ok(packed)
}

fn u32_at(bytes: &[u8], offset: &mut usize) -> Result<usize, GraphError> {
    let end = offset.checked_add(4).ok_or(GraphError::MalformedChunk)?;
    let field: [u8; 4] = bytes
        .get(*offset..end)
        .ok_or(GraphError::MalformedChunk)?
        .try_into()
        .map_err(|_| GraphError::MalformedChunk)?;
    *offset = end;
    Ok(u32::from_le_bytes(field) as usize)
}

fn u64_at(bytes: &[u8], offset: &mut usize) -> Result<usize, GraphError> {
    let end = offset.checked_add(8).ok_or(GraphError::MalformedChunk)?;
    let field: [u8; 8] = bytes
        .get(*offset..end)
        .ok_or(GraphError::MalformedChunk)?
        .try_into()
        .map_err(|_| GraphError::MalformedChunk)?;
    *offset = end;
    usize::try_from(u64::from_le_bytes(field)).map_err(|_| GraphError::BoundExceeded)
}

struct VerifiedChunk<'encoded> {
    bytes: Cow<'encoded, [u8]>,
    entries: Vec<(usize, usize)>,
}

fn verified_chunk<'encoded>(
    descriptor: &ChunkDescriptor,
    encoded: &'encoded [u8],
) -> Result<VerifiedChunk<'encoded>, GraphError> {
    verify_chunk(descriptor, encoded, false)
}

fn verify_chunk<'encoded>(
    descriptor: &ChunkDescriptor,
    encoded: &'encoded [u8],
    authenticated_encoded: bool,
) -> Result<VerifiedChunk<'encoded>, GraphError> {
    if encoded.len().to_string() != descriptor.encoded_byte_length
        || (!authenticated_encoded && hex_hash(encoded) != descriptor.encoded_sha256)
    {
        return Err(GraphError::IntegrityFailure);
    }
    let expected_decoded = descriptor
        .decoded_byte_length
        .parse::<usize>()
        .map_err(|_| GraphError::MalformedIdentity)?;
    if expected_decoded > MAX_CHUNK_BYTES {
        return Err(GraphError::BoundExceeded);
    }
    let decoded = match descriptor.codec {
        Codec::Identity => Cow::Borrowed(encoded),
        Codec::Deflate => {
            let mut output = vec![0; expected_decoded];
            let (decoded, result) = zlib_rs::decompress_slice(
                &mut output,
                encoded,
                zlib_rs::InflateConfig { window_bits: -15 },
            );
            if result == zlib_rs::ReturnCode::BufError || decoded.len() != expected_decoded {
                return Err(GraphError::IntegrityFailure);
            }
            if result != zlib_rs::ReturnCode::Ok {
                return Err(GraphError::MalformedChunk);
            }
            Cow::Owned(output)
        }
    };
    if decoded.len() != expected_decoded
        || match descriptor.codec {
            // Identity chunks have already authenticated these exact bytes above. Their
            // decoded identity must therefore equal the verified encoded identity.
            Codec::Identity => descriptor.decoded_sha256 != descriptor.encoded_sha256,
            Codec::Deflate => {
                !authenticated_encoded && hex_hash(&decoded) != descriptor.decoded_sha256
            }
        }
    {
        return Err(GraphError::IntegrityFailure);
    }
    if decoded.len() < CHUNK_HEADER_BYTES || &decoded[..4] != b"PSCH" {
        return Err(GraphError::MalformedChunk);
    }
    let mut cursor = 4;
    if u32_at(&decoded, &mut cursor)? != 1 {
        return Err(GraphError::MalformedChunk);
    }
    let count = u32_at(&decoded, &mut cursor)?;
    if count == 0 || count > MAX_CHUNK_ENTRIES || count != descriptor.entries.len() {
        return Err(GraphError::BoundExceeded);
    }
    let mut records = Vec::with_capacity(count);
    let mut prior = None::<&str>;
    for expected in &descriptor.entries {
        let path_length = u32_at(&decoded, &mut cursor)?;
        if path_length == 0 || path_length > MAX_LOGICAL_PATH_BYTES {
            return Err(GraphError::MalformedIdentity);
        }
        let path_end = cursor
            .checked_add(path_length)
            .ok_or(GraphError::MalformedChunk)?;
        let logical_path = std::str::from_utf8(
            decoded
                .get(cursor..path_end)
                .ok_or(GraphError::MalformedChunk)?,
        )
        .map_err(|_| GraphError::MalformedIdentity)?;
        cursor = path_end;
        if !valid_logical_path(&logical_path) || prior.is_some_and(|value| value >= logical_path) {
            return Err(GraphError::MalformedIdentity);
        }
        prior = Some(logical_path);
        let entry_offset = u64_at(&decoded, &mut cursor)?;
        let entry_length = u64_at(&decoded, &mut cursor)?;
        let hash_end = cursor.checked_add(32).ok_or(GraphError::MalformedChunk)?;
        let entry_hash = decoded
            .get(cursor..hash_end)
            .ok_or(GraphError::MalformedChunk)?;
        cursor = hash_end;
        if logical_path != expected.logical_path
            || entry_offset.to_string() != expected.offset
            || entry_length.to_string() != expected.byte_length
            || entry_hash
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                != expected.sha256
        {
            return Err(GraphError::IntegrityFailure);
        }
        records.push((
            entry_offset,
            entry_length,
            <[u8; 32]>::try_from(entry_hash).map_err(|_| GraphError::MalformedChunk)?,
        ));
    }
    if records.first().is_none_or(|record| record.0 != cursor) {
        return Err(GraphError::MalformedChunk);
    }
    let mut entries = Vec::with_capacity(count);
    let mut next_offset = cursor;
    for (entry_offset, entry_length, expected_hash) in records {
        if entry_offset != next_offset {
            return Err(GraphError::MalformedChunk);
        }
        let end = entry_offset
            .checked_add(entry_length)
            .ok_or(GraphError::MalformedChunk)?;
        let bytes = decoded
            .get(entry_offset..end)
            .ok_or(GraphError::MalformedChunk)?;
        if hash(bytes) != expected_hash {
            return Err(GraphError::IntegrityFailure);
        }
        entries.push((entry_offset, entry_length));
        next_offset = end;
    }
    if next_offset != decoded.len() {
        return Err(GraphError::MalformedChunk);
    }
    Ok(VerifiedChunk {
        bytes: decoded,
        entries,
    })
}

pub fn decode(
    descriptor: &ChunkDescriptor,
    encoded: &[u8],
) -> Result<Vec<DecodedEntry>, GraphError> {
    let verified = verified_chunk(descriptor, encoded)?;
    Ok(verified
        .entries
        .into_iter()
        .enumerate()
        .map(|(index, (offset, length))| DecodedEntry {
            logical_path: descriptor.entries[index].logical_path.clone(),
            bytes: verified.bytes[offset..offset + length].to_vec(),
        })
        .collect())
}

pub fn encode_batch(chunks: &[PackedChunk]) -> Result<Vec<u8>, GraphError> {
    if chunks.is_empty() || chunks.len() > 1_024 {
        return Err(GraphError::BoundExceeded);
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"PSGB");
    output.extend_from_slice(&1_u32.to_le_bytes());
    write_u32(&mut output, chunks.len())?;
    for chunk in chunks {
        let descriptor =
            serde_json::to_vec(&chunk.descriptor).map_err(|_| GraphError::MalformedIdentity)?;
        write_u32(&mut output, descriptor.len())?;
        output.extend_from_slice(&descriptor);
        write_u32(&mut output, chunk.encoded.len())?;
        output.extend_from_slice(&chunk.encoded);
    }
    Ok(output)
}

fn batch_chunks(bytes: &[u8]) -> Result<Vec<(ChunkDescriptor, &[u8])>, GraphError> {
    if bytes.len() < 12 || &bytes[..4] != b"PSGB" {
        return Err(GraphError::MalformedChunk);
    }
    let mut offset = 4;
    if u32_at(bytes, &mut offset)? != 1 {
        return Err(GraphError::MalformedChunk);
    }
    let count = u32_at(bytes, &mut offset)?;
    if count == 0 || count > 1_024 {
        return Err(GraphError::BoundExceeded);
    }
    let mut chunks = Vec::with_capacity(count);
    for _ in 0..count {
        let descriptor_length = u32_at(bytes, &mut offset)?;
        if descriptor_length == 0 || descriptor_length > 8 * 1024 * 1024 {
            return Err(GraphError::BoundExceeded);
        }
        let descriptor_end = offset
            .checked_add(descriptor_length)
            .ok_or(GraphError::MalformedChunk)?;
        let descriptor = serde_json::from_slice::<ChunkDescriptor>(
            bytes
                .get(offset..descriptor_end)
                .ok_or(GraphError::MalformedChunk)?,
        )
        .map_err(|_| GraphError::MalformedIdentity)?;
        offset = descriptor_end;
        let encoded_length = u32_at(bytes, &mut offset)?;
        let encoded_end = offset
            .checked_add(encoded_length)
            .ok_or(GraphError::MalformedChunk)?;
        let encoded = bytes
            .get(offset..encoded_end)
            .ok_or(GraphError::MalformedChunk)?;
        offset = encoded_end;
        chunks.push((descriptor, encoded));
    }
    if offset != bytes.len() {
        return Err(GraphError::MalformedChunk);
    }
    Ok(chunks)
}

pub fn decode_batch(bytes: &[u8]) -> Result<Vec<DecodedEntry>, GraphError> {
    let decoded = batch_chunks(bytes)?
        .par_iter()
        .map(|(descriptor, encoded)| decode(descriptor, encoded))
        .collect::<Result<Vec<_>, _>>()?;
    let mut entries = Vec::new();
    let mut identities = BTreeSet::new();
    for chunk in decoded {
        for entry in chunk {
            if !identities.insert(entry.logical_path.clone()) {
                return Err(GraphError::DuplicateIdentity);
            }
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    Ok(entries)
}

pub fn encode_resource_set(entries: &[DecodedEntry]) -> Result<Vec<u8>, GraphError> {
    if entries.is_empty() || entries.len() > MAX_GRAPH_ENTRIES {
        return Err(GraphError::BoundExceeded);
    }
    let capacity = entries.iter().try_fold(12_usize, |total, entry| {
        total
            .checked_add(8)
            .and_then(|value| value.checked_add(entry.logical_path.len()))
            .and_then(|value| value.checked_add(entry.bytes.len()))
            .ok_or(GraphError::BoundExceeded)
    })?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(b"PSRE");
    output.extend_from_slice(&1_u32.to_le_bytes());
    write_u32(&mut output, entries.len())?;
    let mut prior = None::<&str>;
    for entry in entries {
        if !valid_logical_path(&entry.logical_path)
            || prior.is_some_and(|value| value >= entry.logical_path.as_str())
        {
            return Err(GraphError::MalformedIdentity);
        }
        prior = Some(&entry.logical_path);
        write_u32(&mut output, entry.logical_path.len())?;
        output.extend_from_slice(entry.logical_path.as_bytes());
        write_u32(&mut output, entry.bytes.len())?;
        output.extend_from_slice(&entry.bytes);
    }
    Ok(output)
}

pub fn decode_to_resource_set(batch: &[u8]) -> Result<Vec<u8>, GraphError> {
    decode_resource_set(batch, false)
}

/// Decode chunks whose descriptors and encoded bytes have already been authenticated
/// together against the selected immutable resource-graph root. Every decoded header,
/// path, range, entry digest, and payload remains independently verified.
pub fn decode_authenticated_resource_set(batch: &[u8]) -> Result<Vec<u8>, GraphError> {
    decode_resource_set(batch, true)
}

fn decode_resource_set(batch: &[u8], authenticated_encoded: bool) -> Result<Vec<u8>, GraphError> {
    let chunks = batch_chunks(batch)?;
    if let [(descriptor, encoded)] = chunks.as_slice() {
        let verified = verify_chunk(descriptor, encoded, authenticated_encoded)?;
        return compact_verified_section(descriptor, verified);
    }
    let entry_count = chunks.iter().try_fold(0usize, |count, (descriptor, _)| {
        count
            .checked_add(descriptor.entries.len())
            .ok_or(GraphError::BoundExceeded)
    })?;
    if entry_count == 0 || entry_count > MAX_GRAPH_ENTRIES {
        return Err(GraphError::BoundExceeded);
    }

    let mut order = chunks
        .iter()
        .enumerate()
        .flat_map(|(chunk, (descriptor, _))| {
            descriptor
                .entries
                .iter()
                .enumerate()
                .map(move |(entry, descriptor)| (chunk, entry, descriptor))
        })
        .collect::<Vec<_>>();
    order.sort_by(|left, right| left.2.logical_path.cmp(&right.2.logical_path));

    let capacity = order.iter().try_fold(12usize, |total, (_, _, entry)| {
        let length = entry
            .byte_length
            .parse::<usize>()
            .map_err(|_| GraphError::IntegrityFailure)?;
        total
            .checked_add(8)
            .and_then(|value| value.checked_add(entry.logical_path.len()))
            .and_then(|value| value.checked_add(length))
            .ok_or(GraphError::BoundExceeded)
    })?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(b"PSRE");
    output.extend_from_slice(&1u32.to_le_bytes());
    write_u32(&mut output, entry_count)?;
    let mut destinations = chunks
        .iter()
        .map(|(descriptor, _)| vec![(0usize, 0usize); descriptor.entries.len()])
        .collect::<Vec<_>>();
    for (chunk, entry, descriptor) in order {
        let length = descriptor
            .byte_length
            .parse::<usize>()
            .map_err(|_| GraphError::IntegrityFailure)?;
        write_u32(&mut output, descriptor.logical_path.len())?;
        output.extend_from_slice(descriptor.logical_path.as_bytes());
        write_u32(&mut output, length)?;
        destinations[chunk][entry] = (output.len(), length);
        output.resize(output.len() + length, 0);
    }

    let destination = Mutex::new(output.as_mut_slice());
    chunks
        .par_iter()
        .enumerate()
        .map(|(chunk, (descriptor, encoded))| {
            let verified = verify_chunk(descriptor, encoded, authenticated_encoded)?;
            let mut destination = destination
                .lock()
                .map_err(|_| GraphError::IntegrityFailure)?;
            for (entry, (source, length)) in verified.entries.iter().enumerate() {
                let (offset, expected) = destinations[chunk][entry];
                if *length != expected {
                    return Err(GraphError::IntegrityFailure);
                }
                destination[offset..offset + length]
                    .copy_from_slice(&verified.bytes[*source..source + length]);
            }
            Ok(())
        })
        .collect::<Result<Vec<_>, GraphError>>()?;
    drop(destination);

    let mut identities = BTreeSet::new();
    for (descriptor, _) in &chunks {
        for entry in &descriptor.entries {
            if !identities.insert(&entry.logical_path) {
                return Err(GraphError::DuplicateIdentity);
            }
        }
    }
    Ok(output)
}

// Browser admission is one bounded chunk at a time. After full verification the
// inflated allocation becomes its final PSRE owner, rather than coexisting with
// another source-sized output. PSCH's larger table makes every move leftward.
fn compact_verified_section(
    descriptor: &ChunkDescriptor,
    verified: VerifiedChunk<'_>,
) -> Result<Vec<u8>, GraphError> {
    let mut output = verified.bytes.into_owned();
    output[..4].copy_from_slice(b"PSRE");
    let mut cursor = CHUNK_HEADER_BYTES;
    for (entry, (source, length)) in descriptor.entries.iter().zip(verified.entries) {
        let header_end = cursor + 8 + entry.logical_path.len();
        if header_end > source {
            return Err(GraphError::MalformedChunk);
        }
        output[cursor..cursor + 4]
            .copy_from_slice(&(entry.logical_path.len() as u32).to_le_bytes());
        cursor += 4;
        output[cursor..cursor + entry.logical_path.len()]
            .copy_from_slice(entry.logical_path.as_bytes());
        cursor += entry.logical_path.len();
        output[cursor..cursor + 4].copy_from_slice(&(length as u32).to_le_bytes());
        cursor += 4;
        output.copy_within(source..source + length, cursor);
        cursor += length;
    }
    output.truncate(cursor);
    Ok(output)
}

pub fn read_resource_set(
    graph_path: &std::path::Path,
    selected_role: Option<&str>,
) -> Result<Vec<u8>, GraphError> {
    let graph_bytes = std::fs::read(graph_path).map_err(|_| GraphError::IntegrityFailure)?;
    let graph = serde_json::from_slice::<ResourceGraph>(&graph_bytes)
        .map_err(|_| GraphError::MalformedIdentity)?;
    if graph.schema != "playsrc-resource-graph-v1"
        || graph.target.is_empty()
        || selected_role.is_some_and(|role| !valid_role(role))
    {
        return Err(GraphError::MalformedIdentity);
    }
    let parent = graph_path.parent().ok_or(GraphError::MalformedIdentity)?;
    let objects = parent.join(format!("{}.graph/objects", graph.target));
    let mut entries = Vec::new();
    let mut identities = BTreeSet::new();
    for descriptor in graph.chunks.iter().filter(|chunk| {
        selected_role.is_none_or(|role| chunk.roles.iter().any(|candidate| candidate == role))
    }) {
        let encoded = std::fs::read(objects.join(&descriptor.encoded_sha256))
            .map_err(|_| GraphError::IntegrityFailure)?;
        for entry in decode(descriptor, &encoded)? {
            if !identities.insert(entry.logical_path.clone()) {
                return Err(GraphError::DuplicateIdentity);
            }
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    encode_resource_set(&entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(path: &str, role: &str, bytes: Vec<u8>) -> Resource {
        Resource {
            logical_path: path.to_owned(),
            roles: BTreeSet::from([role.to_owned()]),
            bytes,
        }
    }

    #[test]
    fn shared_item_roles_have_the_same_bounded_contract_as_graph_consumers() {
        let mut item = resource("models/player/soldier.mdl", "gameplay", vec![1; 32]);
        item.roles = (0..MAX_CHUNK_ROLES).map(|index| format!("equipment-{index:04}")).collect();
        let packed = pack(vec![item.clone()]).unwrap();
        assert_eq!(packed.len(), 1);
        assert_eq!(packed[0].descriptor.roles.len(), MAX_CHUNK_ROLES);
        item.roles.insert("gameplay".into());
        assert_eq!(pack(vec![item]).unwrap_err(), GraphError::BoundExceeded);
    }

    #[test]
    fn packing_is_deterministic_and_decodes_exact_entries() {
        let resources = vec![
            resource("materials/a.vmt", "gameplay", vec![b'a'; 8_192]),
            resource("materials/b.vmt", "gameplay", vec![b'b'; 8_192]),
        ];
        let first = pack(resources.clone()).unwrap();
        let second = pack(resources).unwrap();
        assert_eq!(first, second);
        let decoded = first
            .iter()
            .flat_map(|chunk| decode(&chunk.descriptor, &chunk.encoded).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(decoded.len(), 2);
        assert!(
            first
                .iter()
                .all(|chunk| chunk.descriptor.codec == Codec::Deflate)
        );
    }

    #[test]
    fn parallel_packing_preserves_every_serial_chunk_byte_and_descriptor() {
        let resources = (0..192)
            .map(|index| {
                resource(
                    &format!("materials/{}/{index}.vtf", index % 7),
                    if index % 3 == 0 { "menu" } else { "gameplay" },
                    (0..8_192)
                        .map(|offset| ((index * 31 + offset * 17) % 251) as u8)
                        .collect(),
                )
            })
            .collect::<Vec<_>>();
        let mut groups = BTreeMap::<(Vec<String>, String, u8, String), Vec<Resource>>::new();
        for resource in resources.clone() {
            let roles = resource.roles.iter().cloned().collect::<Vec<_>>();
            groups
                .entry((
                    roles,
                    resource.logical_path.rsplit_once('/').unwrap().0.to_owned(),
                    hash(resource.logical_path.as_bytes())[0] >> 7,
                    String::new(),
                ))
                .or_default()
                .push(resource);
        }
        let mut expected = groups
            .into_iter()
            .map(|((roles, _, _, _), mut resources)| {
                resources.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
                let references = resources.iter().collect::<Vec<_>>();
                let (decoded, entries) = decoded_chunk(&references).unwrap();
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
                encoder.write_all(&decoded).unwrap();
                let compressed = encoder.finish().unwrap();
                let (codec, encoded) = if compressed.len().saturating_mul(100)
                    <= decoded.len().saturating_mul(COMPRESSION_PERCENT)
                {
                    (Codec::Deflate, compressed)
                } else {
                    (Codec::Identity, decoded.clone())
                };
                PackedChunk {
                    descriptor: ChunkDescriptor {
                        codec,
                        encoded_byte_length: encoded.len().to_string(),
                        encoded_sha256: hex_hash(&encoded),
                        decoded_byte_length: decoded.len().to_string(),
                        decoded_sha256: hex_hash(&decoded),
                        roles,
                        entries,
                    },
                    encoded,
                }
            })
            .collect::<Vec<_>>();
        expected.sort_by(|left, right| {
            left.descriptor
                .encoded_sha256
                .cmp(&right.descriptor.encoded_sha256)
        });
        assert_eq!(pack(resources).unwrap(), expected);
    }

    #[test]
    fn changed_entry_does_not_change_unrelated_path_bucket() {
        let mut paths = (0..64)
            .map(|index| {
                resource(
                    &format!("materials/{index}.vmt"),
                    "gameplay",
                    vec![index as u8; 1_024],
                )
            })
            .collect::<Vec<_>>();
        let before = pack(paths.clone()).unwrap();
        paths[0].bytes[0] ^= 1;
        let after = pack(paths).unwrap();
        let unchanged = before
            .iter()
            .filter(|chunk| {
                after.iter().any(|candidate| {
                    candidate.descriptor.encoded_sha256 == chunk.descriptor.encoded_sha256
                })
            })
            .count();
        assert!(unchanged + 1 >= before.len());
    }

    #[test]
    fn map_regions_preserve_shared_sections_and_exact_resource_closure() {
        let common = (0..32)
            .map(|index| {
                resource(
                    &format!("materials/models/player/scout/{index}.vtf"),
                    "gameplay",
                    vec![index; 1024],
                )
            })
            .collect::<Vec<_>>();
        let shared = pack(common.clone()).unwrap();
        for region in ["materials/brick", "materials/models/props_farm", "maps"] {
            let mut inputs = common.clone();
            inputs.extend((0..32).map(|index| {
                resource(
                    &format!("{region}/{index}.vtf"),
                    "gameplay",
                    vec![255 - index; 1024],
                )
            }));
            let forward = pack(inputs.clone()).unwrap();
            inputs.reverse();
            assert_eq!(forward, pack(inputs.clone()).unwrap());
            for chunk in &shared {
                assert!(
                    forward.contains(chunk),
                    "unrelated region changed an authenticated section"
                );
            }
            let actual = forward
                .iter()
                .flat_map(|chunk| decode(&chunk.descriptor, &chunk.encoded).unwrap())
                .map(|entry| (entry.logical_path, entry.bytes))
                .collect::<BTreeMap<_, _>>();
            let expected = inputs
                .into_iter()
                .map(|entry| (entry.logical_path, entry.bytes))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn region_count_never_exceeds_the_consumer_generation_bound() {
        let resources = (0..=MAX_GRAPH_CHUNKS)
            .map(|index| {
                resource(
                    &format!("materials/region{index}/a.vmt"),
                    "gameplay",
                    vec![0],
                )
            })
            .collect();
        assert_eq!(pack(resources), Err(GraphError::BoundExceeded));
    }

    #[test]
    fn verified_admission_compacts_in_place_without_changing_any_resource_byte() {
        let resources = (0..256)
            .map(|index| {
                resource(
                    &format!("materials/shared/{index:04}.vtf"),
                    "gameplay",
                    vec![index as u8; if index % 3 == 0 { 0 } else { index * 127 }],
                )
            })
            .collect::<Vec<_>>();
        let references = resources.iter().collect::<Vec<_>>();
        let (decoded, entries) = decoded_chunk(&references).unwrap();
        let descriptor = ChunkDescriptor {
            codec: Codec::Identity,
            encoded_byte_length: decoded.len().to_string(),
            encoded_sha256: hex_hash(&decoded),
            decoded_byte_length: decoded.len().to_string(),
            decoded_sha256: hex_hash(&decoded),
            roles: vec!["gameplay".into()],
            entries,
        };
        let verified = verify_chunk(&descriptor, &decoded, false).unwrap();
        let owned = VerifiedChunk {
            entries: verified.entries,
            bytes: Cow::Owned(decoded.clone()),
        };
        let pointer = owned.bytes.as_ptr();
        let actual = compact_verified_section(&descriptor, owned).unwrap();
        assert_eq!(pointer, actual.as_ptr());
        let expected = encode_resource_set(
            &resources
                .into_iter()
                .map(|entry| DecodedEntry {
                    logical_path: entry.logical_path,
                    bytes: entry.bytes,
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(actual, expected);
        let mut chunk = PackedChunk {
            descriptor,
            encoded: decoded,
        };
        assert_eq!(
            decode_to_resource_set(&encode_batch(&[chunk.clone()]).unwrap()).unwrap(),
            expected
        );
        chunk.encoded[0] ^= 1;
        assert!(decode_to_resource_set(&encode_batch(&[chunk]).unwrap()).is_err());
    }

    #[test]
    fn large_resources_are_individual_objects() {
        let chunks = pack(vec![
            resource("materials/a.vtf", "gameplay", vec![0; LARGE_RESOURCE_BYTES]),
            resource("materials/b.vtf", "gameplay", vec![0; LARGE_RESOURCE_BYTES]),
        ])
        .unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.descriptor.entries.len() == 1)
        );
    }

    #[test]
    fn corruption_and_duplicate_paths_fail_closed() {
        assert_eq!(
            pack(vec![
                resource("a", "menu", vec![1]),
                resource("a", "menu", vec![1])
            ]),
            Err(GraphError::DuplicateIdentity)
        );
        let mut chunks = pack(vec![resource("a", "menu", vec![1; 8_192])]).unwrap();
        chunks[0].encoded[0] ^= 1;
        assert_eq!(
            decode(&chunks[0].descriptor, &chunks[0].encoded),
            Err(GraphError::IntegrityFailure)
        );
    }

    #[test]
    fn identity_chunks_reuse_only_the_exact_authenticated_encoded_digest() {
        let mut state = 0x9e37_79b9_u32;
        let bytes = (0..16_384)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect::<Vec<_>>();
        let chunk = pack(vec![resource("materials/noise.vtf", "gameplay", bytes)])
            .unwrap()
            .remove(0);
        assert_eq!(chunk.descriptor.codec, Codec::Identity);
        assert_eq!(
            chunk.descriptor.encoded_sha256,
            chunk.descriptor.decoded_sha256
        );
        assert!(decode(&chunk.descriptor, &chunk.encoded).is_ok());

        let mut descriptor = chunk.descriptor.clone();
        descriptor.decoded_sha256.replace_range(..1, "f");
        if descriptor.decoded_sha256 == chunk.descriptor.decoded_sha256 {
            descriptor.decoded_sha256.replace_range(..1, "0");
        }
        assert_eq!(
            decode(&descriptor, &chunk.encoded),
            Err(GraphError::IntegrityFailure)
        );
    }

    #[test]
    fn bounded_raw_deflate_preserves_exact_bytes_and_rejects_corrupt_identities() {
        let chunk = pack(vec![resource(
            "materials/compressed.vtf",
            "gameplay",
            (0..65_536).map(|index| (index % 251) as u8).collect(),
        )])
        .unwrap()
        .remove(0);
        assert_eq!(chunk.descriptor.codec, Codec::Deflate);

        let mut legacy = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::DeflateDecoder::new(chunk.encoded.as_slice()),
            &mut legacy,
        )
        .unwrap();
        let verified = verified_chunk(&chunk.descriptor, &chunk.encoded).unwrap();
        assert_eq!(verified.bytes.as_ref(), legacy.as_slice());

        let mut descriptor = chunk.descriptor.clone();
        descriptor.decoded_byte_length = (legacy.len() - 1).to_string();
        assert_eq!(
            decode(&descriptor, &chunk.encoded),
            Err(GraphError::IntegrityFailure)
        );

        let mut descriptor = chunk.descriptor.clone();
        descriptor.entries[0].sha256.replace_range(..1, "f");
        if descriptor.entries[0].sha256 == chunk.descriptor.entries[0].sha256 {
            descriptor.entries[0].sha256.replace_range(..1, "0");
        }
        assert_eq!(
            decode(&descriptor, &chunk.encoded),
            Err(GraphError::IntegrityFailure)
        );

        let mut truncated = chunk.encoded.clone();
        truncated.truncate(truncated.len() / 2);
        let mut descriptor = chunk.descriptor.clone();
        descriptor.encoded_byte_length = truncated.len().to_string();
        descriptor.encoded_sha256 = hex_hash(&truncated);
        assert!(matches!(
            decode(&descriptor, &truncated),
            Err(GraphError::MalformedChunk | GraphError::IntegrityFailure)
        ));
    }

    #[test]
    fn batch_decodes_to_one_sorted_resource_set() {
        let chunks = pack(vec![
            resource("materials/b.vmt", "gameplay", vec![2; 8_192]),
            resource("materials/a.vmt", "menu", vec![1; 8_192]),
        ])
        .unwrap();
        let batch = encode_batch(&chunks).unwrap();
        let set = decode_to_resource_set(&batch).unwrap();
        assert_eq!(&set[..4], b"PSRE");
        assert_eq!(u32::from_le_bytes(set[8..12].try_into().unwrap()), 2);
        assert_eq!(
            set,
            encode_resource_set(&decode_batch(&batch).unwrap()).unwrap()
        );
    }

    #[test]
    fn parallel_resource_set_preserves_interleaved_chunk_order_and_integrity() {
        let chunks = pack(
            (0..96)
                .map(|index| {
                    resource(
                        &format!("materials/{:03}.vtf", 95 - index),
                        if index % 2 == 0 { "gameplay" } else { "menu" },
                        vec![index as u8; 4096 + index],
                    )
                })
                .collect(),
        )
        .unwrap();
        let batch = encode_batch(&chunks).unwrap();
        assert_eq!(
            decode_to_resource_set(&batch).unwrap(),
            encode_resource_set(&decode_batch(&batch).unwrap()).unwrap(),
        );
        assert_eq!(
            decode_authenticated_resource_set(&batch).unwrap(),
            decode_to_resource_set(&batch).unwrap(),
        );

        let mut damaged = batch.clone();
        *damaged.last_mut().unwrap() ^= 1;
        assert_eq!(
            decode_to_resource_set(&damaged),
            Err(GraphError::IntegrityFailure)
        );
        assert!(matches!(
            decode_authenticated_resource_set(&damaged),
            Err(GraphError::MalformedChunk | GraphError::IntegrityFailure)
        ));
    }

    #[test]
    fn canonical_json_sorts_every_object_key() {
        let value = serde_json::json!({"z": 1, "a": {"d": true, "b": null}});
        assert_eq!(
            canonical_json(&value).unwrap(),
            br#"{"a":{"b":null,"d":true},"z":1}"#
        );
    }

    #[test]
    fn cross_map_packing_reuses_unaffected_shared_chunks() {
        let shared = (0..256)
            .map(|index| {
                resource(
                    &format!("materials/shared/{index}.vmt"),
                    "gameplay",
                    vec![index as u8; 1_024],
                )
            })
            .collect::<Vec<_>>();
        let mut first = shared.clone();
        first.push(resource(
            "materials/maps/first.vmt",
            "gameplay",
            vec![1; 1_024],
        ));
        let mut second = shared;
        second.push(resource(
            "materials/maps/second.vmt",
            "gameplay",
            vec![2; 1_024],
        ));
        let first = pack(first).unwrap();
        let second = pack(second).unwrap();
        let reused = first
            .iter()
            .filter(|chunk| {
                second.iter().any(|candidate| {
                    candidate.descriptor.encoded_sha256 == chunk.descriptor.encoded_sha256
                })
            })
            .count();
        assert!(reused >= first.len().saturating_sub(2));
    }

    #[test]
    fn graph_and_chunk_bounds_fail_before_output() {
        let over_graph = (0..=MAX_GRAPH_ENTRIES)
            .map(|index| resource(&format!("materials/{index}.vmt"), "gameplay", vec![1]))
            .collect();
        assert_eq!(pack(over_graph), Err(GraphError::BoundExceeded));
        assert_eq!(
            pack(vec![resource(
                "materials/large.vtf",
                "gameplay",
                vec![0; MAX_CHUNK_BYTES]
            )]),
            Err(GraphError::BoundExceeded),
        );
        assert_eq!(
            pack(vec![resource("materials/a.vmt", "bad_role", vec![1])]),
            Err(GraphError::MalformedIdentity),
        );
    }
}
