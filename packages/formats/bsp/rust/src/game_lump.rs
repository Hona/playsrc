use std::ops::Range;

use crate::{
    Bsp, ErrorCode, Float32, Limits, ParseError, Vector3, decode_source_lzma_member, failure,
};

pub const GAME_LUMP_SLOT: usize = 35;
pub const STATIC_PROP_VERSION: u16 = 10;
pub const STATIC_PROP_RECORD_BYTES: usize = 72;
pub const STATIC_PROP_USE_LIGHTING_ORIGIN: u32 = 0x0002;
pub const STATIC_PROP_NO_PER_VERTEX_LIGHTING: u32 = 0x0040;

const STATIC_PROP_ID: u32 = u32::from_be_bytes(*b"sprp");
const DICTIONARY_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticPropDictionaryEntry {
    pub index: usize,
    pub raw_name: [u8; DICTIONARY_BYTES],
    pub name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticPropOccurrence {
    pub index: usize,
    pub origin: Vector3,
    pub angles: Vector3,
    pub model: u16,
    pub first_leaf: u16,
    pub leaf_count: u16,
    pub leaves: Vec<u16>,
    pub solidity: u8,
    pub padding: u8,
    pub skin: i32,
    pub fade_minimum: Float32,
    pub fade_maximum: Float32,
    pub raw_lighting_origin: Vector3,
    pub lighting_origin: Option<Vector3>,
    pub forced_fade_scale: Float32,
    pub minimum_dx_level: u16,
    pub maximum_dx_level: u16,
    pub flags: u32,
    pub lightmap_resolution: [u16; 2],
    pub decoded_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticProps {
    pub directory_ordinal: usize,
    pub flags: u16,
    pub version: u16,
    pub encoded_range: Range<usize>,
    pub decoded_length: usize,
    pub dictionary: Vec<StaticPropDictionaryEntry>,
    pub leaf_references: Vec<u16>,
    pub occurrences: Vec<StaticPropOccurrence>,
}

#[derive(Clone, Copy)]
struct Header {
    ordinal: usize,
    id: u32,
    flags: u16,
    version: u16,
    start: usize,
    decoded_length: usize,
}

pub fn parse_static_props(bsp: &Bsp, limits: Limits) -> Result<Option<StaticProps>, ParseError> {
    let directory = bsp
        .lump(GAME_LUMP_SLOT)
        .ok_or_else(|| invalid(0..0))?
        .bytes(bsp);
    if directory.is_empty() {
        return Ok(None);
    }
    let count = nonnegative_i32(directory, 0)?;
    if count > limits.max_records_per_lump {
        return Err(failure(
            ErrorCode::RecordBudget,
            Some(GAME_LUMP_SLOT),
            0..4,
            Some(count),
            Some(limits.max_records_per_lump),
        ));
    }
    let directory_end = count
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or_else(|| invalid(0..4))?;
    if directory_end > directory.len() {
        return Err(invalid(directory.len()..directory_end));
    }
    let mut headers = Vec::with_capacity(count);
    for ordinal in 0..count {
        let at = 4 + ordinal * 16;
        let flags = u16_at(directory, at + 4)?;
        if flags & !1 != 0 {
            return Err(invalid(at + 4..at + 6));
        }
        headers.push(Header {
            ordinal,
            id: u32_at(directory, at)?,
            flags,
            version: u16_at(directory, at + 6)?,
            start: nonnegative_i32(directory, at + 8)?,
            decoded_length: nonnegative_i32(directory, at + 12)?,
        });
    }
    let mut selected = headers
        .iter()
        .copied()
        .filter(|entry| entry.id == STATIC_PROP_ID);
    let Some(header) = selected.next() else {
        return Ok(None);
    };
    if selected.next().is_some() {
        return Err(invalid(0..directory_end));
    }
    if header.version != STATIC_PROP_VERSION {
        return Err(failure(
            ErrorCode::UnsupportedLumpVersion,
            Some(GAME_LUMP_SLOT),
            4 + header.ordinal * 16 + 6..4 + header.ordinal * 16 + 8,
            Some(usize::from(header.version)),
            Some(usize::from(STATIC_PROP_VERSION)),
        ));
    }
    let encoded_end = if header.flags & 1 != 0 {
        headers
            .iter()
            .filter_map(|candidate| (candidate.start > header.start).then_some(candidate.start))
            .min()
            .unwrap_or(bsp.source_bytes().len())
    } else {
        header
            .start
            .checked_add(header.decoded_length)
            .ok_or_else(|| invalid(header.start..header.start))?
    };
    let encoded = bsp
        .source_bytes()
        .get(header.start..encoded_end)
        .ok_or_else(|| invalid(header.start..encoded_end))?;
    let decoded = if header.flags & 1 != 0 {
        let payload = encoded
            .get(8..12)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("fixed range")) as usize)
            .ok_or_else(|| invalid(header.start..encoded_end))?;
        let member_end = 17usize
            .checked_add(payload)
            .ok_or_else(|| invalid(header.start..encoded_end))?;
        decode_source_lzma_member(
            encoded
                .get(..member_end)
                .ok_or_else(|| invalid(header.start..encoded_end))?,
            header.decoded_length,
            limits,
        )?
    } else {
        if encoded.len() != header.decoded_length {
            return Err(invalid(header.start..encoded_end));
        }
        encoded.to_vec()
    };
    decode_static_props(header, header.start..encoded_end, &decoded, limits).map(Some)
}

fn decode_static_props(
    header: Header,
    encoded_range: Range<usize>,
    bytes: &[u8],
    limits: Limits,
) -> Result<StaticProps, ParseError> {
    let model_count = nonnegative_i32(bytes, 0)?;
    bound(model_count, limits.max_records_per_lump, 0..4)?;
    let dictionary_end = model_count
        .checked_mul(DICTIONARY_BYTES)
        .and_then(|length| length.checked_add(4))
        .ok_or_else(|| invalid(0..4))?;
    let dictionary_bytes = bytes
        .get(4..dictionary_end)
        .ok_or_else(|| invalid(bytes.len()..dictionary_end))?;
    let mut dictionary = Vec::with_capacity(model_count);
    for (index, field) in dictionary_bytes.chunks_exact(DICTIONARY_BYTES).enumerate() {
        let raw_name: [u8; DICTIONARY_BYTES] = field.try_into().expect("fixed chunk");
        let nul = field.iter().position(|byte| *byte == 0).ok_or_else(|| {
            invalid(4 + index * DICTIONARY_BYTES..4 + (index + 1) * DICTIONARY_BYTES)
        })?;
        if nul == 0 || field[nul + 1..].iter().any(|byte| *byte != 0) {
            return Err(invalid(
                4 + index * DICTIONARY_BYTES..4 + (index + 1) * DICTIONARY_BYTES,
            ));
        }
        dictionary.push(StaticPropDictionaryEntry {
            index,
            raw_name,
            name: field[..nul].to_vec(),
        });
    }
    let leaf_count = nonnegative_i32(bytes, dictionary_end)?;
    bound(
        leaf_count,
        limits.max_records_per_lump,
        dictionary_end..dictionary_end + 4,
    )?;
    let leaves_start = dictionary_end + 4;
    let leaves_end = leaf_count
        .checked_mul(2)
        .and_then(|length| leaves_start.checked_add(length))
        .ok_or_else(|| invalid(leaves_start..leaves_start))?;
    let leaves = bytes
        .get(leaves_start..leaves_end)
        .ok_or_else(|| invalid(bytes.len()..leaves_end))?
        .chunks_exact(2)
        .map(|field| u16::from_le_bytes(field.try_into().expect("fixed chunk")))
        .collect::<Vec<_>>();
    let occurrence_count = nonnegative_i32(bytes, leaves_end)?;
    bound(
        occurrence_count,
        limits.max_records_per_lump,
        leaves_end..leaves_end + 4,
    )?;
    let records_start = leaves_end + 4;
    let records_end = occurrence_count
        .checked_mul(STATIC_PROP_RECORD_BYTES)
        .and_then(|length| records_start.checked_add(length))
        .ok_or_else(|| invalid(records_start..records_start))?;
    if records_end != bytes.len() {
        return Err(invalid(
            records_end.min(bytes.len())..records_end.max(bytes.len()),
        ));
    }
    let mut occurrences = Vec::with_capacity(occurrence_count);
    for (index, record) in bytes[records_start..]
        .chunks_exact(STATIC_PROP_RECORD_BYTES)
        .enumerate()
    {
        let model = u16_at(record, 24)?;
        let first_leaf = u16_at(record, 26)?;
        let leaf_count = u16_at(record, 28)?;
        let leaf_end = usize::from(first_leaf)
            .checked_add(usize::from(leaf_count))
            .ok_or_else(|| {
                invalid(
                    records_start + index * STATIC_PROP_RECORD_BYTES + 26
                        ..records_start + index * STATIC_PROP_RECORD_BYTES + 30,
                )
            })?;
        if usize::from(model) >= dictionary.len() || leaf_end > leaves.len() {
            return Err(invalid(
                records_start + index * STATIC_PROP_RECORD_BYTES + 24
                    ..records_start + index * STATIC_PROP_RECORD_BYTES + 30,
            ));
        }
        let flags = u32_at(record, 64)?;
        let raw_lighting_origin = vector(record, 44)?;
        occurrences.push(StaticPropOccurrence {
            index,
            origin: vector(record, 0)?,
            angles: vector(record, 12)?,
            model,
            first_leaf,
            leaf_count,
            leaves: leaves[usize::from(first_leaf)..leaf_end].to_vec(),
            solidity: record[30],
            padding: record[31],
            skin: i32_at(record, 32)?,
            fade_minimum: float(record, 36)?,
            fade_maximum: float(record, 40)?,
            raw_lighting_origin,
            lighting_origin: (flags & STATIC_PROP_USE_LIGHTING_ORIGIN != 0)
                .then_some(raw_lighting_origin),
            forced_fade_scale: float(record, 56)?,
            minimum_dx_level: u16_at(record, 60)?,
            maximum_dx_level: u16_at(record, 62)?,
            flags,
            lightmap_resolution: [u16_at(record, 68)?, u16_at(record, 70)?],
            decoded_range: records_start + index * STATIC_PROP_RECORD_BYTES
                ..records_start + (index + 1) * STATIC_PROP_RECORD_BYTES,
        });
    }
    Ok(StaticProps {
        directory_ordinal: header.ordinal,
        flags: header.flags,
        version: header.version,
        encoded_range,
        decoded_length: header.decoded_length,
        dictionary,
        leaf_references: leaves,
        occurrences,
    })
}

fn bound(value: usize, limit: usize, range: Range<usize>) -> Result<(), ParseError> {
    if value > limit {
        return Err(failure(
            ErrorCode::RecordBudget,
            Some(GAME_LUMP_SLOT),
            range,
            Some(value),
            Some(limit),
        ));
    }
    Ok(())
}

fn invalid(range: Range<usize>) -> ParseError {
    failure(
        ErrorCode::InvalidRecord,
        Some(GAME_LUMP_SLOT),
        range,
        None,
        None,
    )
}

fn vector(bytes: &[u8], at: usize) -> Result<Vector3, ParseError> {
    Ok(Vector3 {
        x: float(bytes, at)?,
        y: float(bytes, at + 4)?,
        z: float(bytes, at + 8)?,
    })
}

fn float(bytes: &[u8], at: usize) -> Result<Float32, ParseError> {
    u32_at(bytes, at).map(Float32)
}

fn nonnegative_i32(bytes: &[u8], at: usize) -> Result<usize, ParseError> {
    usize::try_from(i32_at(bytes, at)?).map_err(|_| invalid(at..at + 4))
}

fn i32_at(bytes: &[u8], at: usize) -> Result<i32, ParseError> {
    u32_at(bytes, at).map(|value| i32::from_le_bytes(value.to_le_bytes()))
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, ParseError> {
    bytes
        .get(at..at + 2)
        .map(|field| u16::from_le_bytes(field.try_into().expect("fixed range")))
        .ok_or_else(|| invalid(at..at + 2))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, ParseError> {
    bytes
        .get(at..at + 4)
        .map(|field| u32::from_le_bytes(field.try_into().expect("fixed range")))
        .ok_or_else(|| invalid(at..at + 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1i32.to_le_bytes());
        let mut name = [0u8; 128];
        name[..21].copy_from_slice(b"models/props/test.mdl");
        bytes.extend_from_slice(&name);
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&9u16.to_le_bytes());
        bytes.extend_from_slice(&1i32.to_le_bytes());
        let mut record = [0u8; STATIC_PROP_RECORD_BYTES];
        for (index, bits) in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]
            .into_iter()
            .map(f32::to_bits)
            .enumerate()
        {
            record[index * 4..index * 4 + 4].copy_from_slice(&bits.to_le_bytes());
        }
        record[24..26].copy_from_slice(&0u16.to_le_bytes());
        record[26..28].copy_from_slice(&0u16.to_le_bytes());
        record[28..30].copy_from_slice(&2u16.to_le_bytes());
        record[30] = 6;
        record[31] = 0x7f;
        record[32..36].copy_from_slice(&3i32.to_le_bytes());
        record[36..40].copy_from_slice(&100.0f32.to_bits().to_le_bytes());
        record[40..44].copy_from_slice(&200.0f32.to_bits().to_le_bytes());
        record[44..48].copy_from_slice(&7.0f32.to_bits().to_le_bytes());
        record[48..52].copy_from_slice(&8.0f32.to_bits().to_le_bytes());
        record[52..56].copy_from_slice(&9.0f32.to_bits().to_le_bytes());
        record[56..60].copy_from_slice(&1.5f32.to_bits().to_le_bytes());
        record[60..62].copy_from_slice(&90u16.to_le_bytes());
        record[62..64].copy_from_slice(&95u16.to_le_bytes());
        record[64..68].copy_from_slice(&0x143u32.to_le_bytes());
        record[68..70].copy_from_slice(&32u16.to_le_bytes());
        record[70..72].copy_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(&record);
        bytes
    }

    #[test]
    fn decodes_source_v10_fields_and_flag_selected_lighting_origin() {
        let bytes = payload();
        let props = decode_static_props(
            Header {
                ordinal: 2,
                id: STATIC_PROP_ID,
                flags: 1,
                version: STATIC_PROP_VERSION,
                start: 1_000,
                decoded_length: bytes.len(),
            },
            1_000..1_100,
            &bytes,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(props.dictionary[0].name, b"models/props/test.mdl");
        assert_eq!(props.leaf_references, [4, 9]);
        let prop = &props.occurrences[0];
        assert_eq!(prop.origin.x.value(), 1.0);
        assert_eq!(prop.angles.z.value(), 6.0);
        assert_eq!(prop.leaves, [4, 9]);
        assert_eq!((prop.solidity, prop.padding, prop.skin), (6, 0x7f, 3));
        assert_eq!(prop.lighting_origin, Some(prop.raw_lighting_origin));
        assert_eq!((prop.minimum_dx_level, prop.maximum_dx_level), (90, 95));
        assert_eq!(prop.flags, 0x143);
        assert_eq!(prop.lightmap_resolution, [32, 16]);
        assert_eq!(prop.decoded_range.end - prop.decoded_range.start, 72);

        let mut without_origin = bytes;
        let flags = without_origin.len() - 8;
        without_origin[flags..flags + 4].copy_from_slice(&0x140u32.to_le_bytes());
        let decoded = decode_static_props(
            Header {
                ordinal: 0,
                id: STATIC_PROP_ID,
                flags: 0,
                version: STATIC_PROP_VERSION,
                start: 0,
                decoded_length: without_origin.len(),
            },
            0..without_origin.len(),
            &without_origin,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(decoded.occurrences[0].lighting_origin, None);
        assert_eq!(decoded.occurrences[0].raw_lighting_origin.x.value(), 7.0);
    }

    #[test]
    fn rejects_truncation_references_tails_and_record_bounds() {
        let bytes = payload();
        let header = Header {
            ordinal: 0,
            id: STATIC_PROP_ID,
            flags: 0,
            version: STATIC_PROP_VERSION,
            start: 0,
            decoded_length: bytes.len(),
        };
        assert_eq!(
            decode_static_props(
                header,
                0..bytes.len(),
                &bytes[..bytes.len() - 1],
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidRecord
        );
        let mut bad_model = bytes.clone();
        let record = bad_model.len() - STATIC_PROP_RECORD_BYTES;
        bad_model[record + 24..record + 26].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            decode_static_props(header, 0..bytes.len(), &bad_model, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidRecord
        );
        let limits = Limits {
            max_records_per_lump: 0,
            ..Limits::default()
        };
        assert_eq!(
            decode_static_props(header, 0..bytes.len(), &bytes, limits)
                .unwrap_err()
                .code,
            ErrorCode::RecordBudget
        );
    }
}
