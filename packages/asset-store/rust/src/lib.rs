use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};

use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_CHUNK_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CHUNK_ENTRIES: usize = 2_048;
pub const MAX_GRAPH_ENTRIES: usize = 4_096;
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
        && path == path.to_ascii_lowercase()
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

pub fn pack(resources: Vec<Resource>) -> Result<Vec<PackedChunk>, GraphError> {
    if resources.is_empty() || resources.len() > MAX_GRAPH_ENTRIES {
        return Err(GraphError::BoundExceeded);
    }
    let mut identities = BTreeSet::new();
    let mut groups = BTreeMap::<(Vec<String>, u8, String), Vec<Resource>>::new();
    for resource in resources {
        if !valid_logical_path(&resource.logical_path)
            || resource.roles.is_empty()
            || resource.roles.iter().any(|role| !valid_role(role))
        {
            return Err(GraphError::MalformedIdentity);
        }
        if !identities.insert(resource.logical_path.clone()) {
            return Err(GraphError::DuplicateIdentity);
        }
        let roles = resource.roles.iter().cloned().collect::<Vec<_>>();
        let path_hash = hash(resource.logical_path.as_bytes());
        let bucket = path_hash[0] >> 2;
        let individual = if resource.bytes.len() >= LARGE_RESOURCE_BYTES {
            resource.logical_path.clone()
        } else {
            String::new()
        };
        groups
            .entry((roles, bucket, individual))
            .or_default()
            .push(resource);
    }

    let mut packed = Vec::with_capacity(groups.len());
    for ((roles, _, _), mut resources) in groups {
        resources.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
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
        packed.push(PackedChunk {
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
        });
    }
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

pub fn decode(
    descriptor: &ChunkDescriptor,
    encoded: &[u8],
) -> Result<Vec<DecodedEntry>, GraphError> {
    if encoded.len().to_string() != descriptor.encoded_byte_length
        || hex_hash(encoded) != descriptor.encoded_sha256
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
        Codec::Identity => encoded.to_vec(),
        Codec::Deflate => {
            let mut output = Vec::with_capacity(expected_decoded);
            DeflateDecoder::new(encoded)
                .take((MAX_CHUNK_BYTES + 1) as u64)
                .read_to_end(&mut output)
                .map_err(|_| GraphError::MalformedChunk)?;
            output
        }
    };
    if decoded.len() != expected_decoded || hex_hash(&decoded) != descriptor.decoded_sha256 {
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
    let mut prior = None::<String>;
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
        .map_err(|_| GraphError::MalformedIdentity)?
        .to_owned();
        cursor = path_end;
        if !valid_logical_path(&logical_path)
            || prior.as_ref().is_some_and(|value| value >= &logical_path)
        {
            return Err(GraphError::MalformedIdentity);
        }
        prior = Some(logical_path.clone());
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
            logical_path,
            entry_offset,
            entry_length,
            entry_hash.to_vec(),
        ));
    }
    if records.first().is_none_or(|record| record.1 != cursor) {
        return Err(GraphError::MalformedChunk);
    }
    let mut output = Vec::with_capacity(count);
    let mut next_offset = cursor;
    for (logical_path, entry_offset, entry_length, expected_hash) in records {
        if entry_offset != next_offset {
            return Err(GraphError::MalformedChunk);
        }
        let end = entry_offset
            .checked_add(entry_length)
            .ok_or(GraphError::MalformedChunk)?;
        let bytes = decoded
            .get(entry_offset..end)
            .ok_or(GraphError::MalformedChunk)?;
        if hash(bytes).as_slice() != expected_hash {
            return Err(GraphError::IntegrityFailure);
        }
        output.push(DecodedEntry {
            logical_path,
            bytes: bytes.to_vec(),
        });
        next_offset = end;
    }
    if next_offset != decoded.len() {
        return Err(GraphError::MalformedChunk);
    }
    Ok(output)
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

pub fn decode_batch(bytes: &[u8]) -> Result<Vec<DecodedEntry>, GraphError> {
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
    let mut entries = Vec::new();
    let mut identities = BTreeSet::new();
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
        for entry in decode(&descriptor, encoded)? {
            if !identities.insert(entry.logical_path.clone()) {
                return Err(GraphError::DuplicateIdentity);
            }
            entries.push(entry);
        }
    }
    if offset != bytes.len() {
        return Err(GraphError::MalformedChunk);
    }
    entries.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    Ok(entries)
}

pub fn encode_resource_set(entries: &[DecodedEntry]) -> Result<Vec<u8>, GraphError> {
    if entries.is_empty() || entries.len() > 4_096 {
        return Err(GraphError::BoundExceeded);
    }
    let mut output = Vec::new();
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
    encode_resource_set(&decode_batch(batch)?)
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
