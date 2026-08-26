use std::{
    fmt,
    io::{Cursor, Read},
    ops::Range,
    sync::Arc,
};

use rayon::prelude::*;

mod game_lump;
mod pak;
mod records;

pub use game_lump::*;
pub use pak::{Pak, PakEntry, PakEntryClassification};
pub use records::*;

pub const HEADER_BYTES: usize = 1_036;
pub const LUMP_COUNT: usize = 64;
const PARALLEL_LZMA_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Source2013V20,
}

impl Profile {
    const fn version(self) -> i32 {
        match self {
            Self::Source2013V20 => 20,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_encoded_bytes: usize,
    pub max_total_decoded_bytes: usize,
    pub max_decoded_bytes_per_lump: usize,
    pub max_compression_ratio: usize,
    pub max_records_per_lump: usize,
    pub max_pak_entries: usize,
    pub max_decoded_pak_bytes: usize,
    pub max_decoded_bytes_per_pak_entry: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 512 * 1024 * 1024,
            max_encoded_bytes: 512 * 1024 * 1024,
            max_total_decoded_bytes: 1024 * 1024 * 1024,
            max_decoded_bytes_per_lump: 512 * 1024 * 1024,
            max_compression_ratio: 1_024,
            max_records_per_lump: 16 * 1024 * 1024,
            max_pak_entries: 65_535,
            max_decoded_pak_bytes: 512 * 1024 * 1024,
            max_decoded_bytes_per_pak_entry: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compression {
    pub decoded_size: usize,
    pub encoded_payload_size: usize,
    pub properties: [u8; 5],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lump {
    pub index: usize,
    pub version: i32,
    pub raw_fourth_field: [u8; 4],
    pub encoded_range: Range<usize>,
    pub alignment_residue: usize,
    pub overlaps: Vec<usize>,
    pub compression: Option<Compression>,
    pub coverage: LumpCoverage,
    pub records: LumpData,
    pub pak: Option<Pak>,
    decoded: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LumpCoverage {
    Handled,
    IntentionallyInert,
    Unsupported,
    Unknown,
}

impl Lump {
    pub fn bytes<'a>(&'a self, bsp: &'a Bsp) -> &'a [u8] {
        self.decoded
            .as_deref()
            .unwrap_or(&bsp.source[self.encoded_range.clone()])
    }

    pub fn encoded_bytes<'a>(&self, bsp: &'a Bsp) -> &'a [u8] {
        &bsp.source[self.encoded_range.clone()]
    }
}

#[derive(Clone, Debug)]
pub struct Bsp {
    pub profile: Profile,
    pub container_version: i32,
    pub map_revision: i32,
    pub lumps: Vec<Lump>,
    source: Arc<[u8]>,
}

impl Bsp {
    pub fn source_bytes(&self) -> &[u8] {
        &self.source
    }

    pub fn lump(&self, index: usize) -> Option<&Lump> {
        self.lumps.get(index)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    TruncatedHeader,
    InvalidIdentifier,
    ProfileVersionMismatch,
    NegativeRange,
    RangeOverflow,
    TruncatedRange,
    EncodedBudget,
    InvalidCompression,
    DecodedBudget,
    CompressionRatio,
    DecompressionFailed,
    UnsupportedLumpVersion,
    InvalidRecord,
    RecordBudget,
    InvalidPak,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub code: ErrorCode,
    pub lump: Option<usize>,
    pub range: Range<usize>,
    pub declared: Option<usize>,
    pub limit: Option<usize>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?} at {}..{}",
            self.code, self.range.start, self.range.end
        )
    }
}

impl std::error::Error for ParseError {}

pub fn parse(source: &[u8], profile: Profile, limits: Limits) -> Result<Bsp, ParseError> {
    if source.len() > limits.max_input_bytes {
        return Err(failure(
            ErrorCode::InputLimit,
            None,
            source.len()..source.len(),
            Some(source.len()),
            Some(limits.max_input_bytes),
        ));
    }
    if source.len() < HEADER_BYTES {
        return Err(failure(
            ErrorCode::TruncatedHeader,
            None,
            source.len()..HEADER_BYTES,
            Some(source.len()),
            Some(HEADER_BYTES),
        ));
    }
    if &source[..4] != b"VBSP" {
        return Err(failure(
            ErrorCode::InvalidIdentifier,
            None,
            0..4,
            None,
            None,
        ));
    }
    let container_version = i32_at(source, 4);
    if container_version != profile.version() {
        return Err(failure(
            ErrorCode::ProfileVersionMismatch,
            None,
            4..8,
            Some(container_version.unsigned_abs() as usize),
            Some(profile.version() as usize),
        ));
    }

    let mut encoded_total = 0_usize;
    let mut lumps = Vec::with_capacity(LUMP_COUNT);
    for index in 0..LUMP_COUNT {
        let header = 8 + index * 16;
        let offset = i32_at(source, header);
        let length = i32_at(source, header + 4);
        if offset < 0 || length < 0 {
            return Err(failure(
                ErrorCode::NegativeRange,
                Some(index),
                header..header + 8,
                None,
                None,
            ));
        }
        let start = offset as usize;
        let length = length as usize;
        let Some(end) = start.checked_add(length) else {
            return Err(failure(
                ErrorCode::RangeOverflow,
                Some(index),
                header..header + 8,
                Some(length),
                None,
            ));
        };
        if end > source.len() {
            return Err(failure(
                ErrorCode::TruncatedRange,
                Some(index),
                start..end,
                Some(end),
                Some(source.len()),
            ));
        }
        encoded_total = encoded_total.checked_add(length).ok_or_else(|| {
            failure(
                ErrorCode::EncodedBudget,
                Some(index),
                start..end,
                None,
                Some(limits.max_encoded_bytes),
            )
        })?;
        if encoded_total > limits.max_encoded_bytes {
            return Err(failure(
                ErrorCode::EncodedBudget,
                Some(index),
                start..end,
                Some(encoded_total),
                Some(limits.max_encoded_bytes),
            ));
        }
        lumps.push(Lump {
            index,
            version: i32_at(source, header + 8),
            raw_fourth_field: source[header + 12..header + 16]
                .try_into()
                .expect("fixed range"),
            encoded_range: start..end,
            alignment_residue: start % 4,
            overlaps: Vec::new(),
            compression: None,
            coverage: LumpCoverage::IntentionallyInert,
            records: LumpData::Opaque,
            pak: None,
            decoded: None,
        });
    }

    for left in 0..LUMP_COUNT {
        if lumps[left].encoded_range.is_empty() {
            continue;
        }
        for right in left + 1..LUMP_COUNT {
            if ranges_overlap(&lumps[left].encoded_range, &lumps[right].encoded_range) {
                lumps[left].overlaps.push(right);
                lumps[right].overlaps.push(left);
            }
        }
    }

    let compressed = lumps
        .iter()
        .enumerate()
        .filter_map(|(position, lump)| {
            let declared = u32::from_le_bytes(lump.raw_fourth_field) as usize;
            (!lump.encoded_range.is_empty() && declared != 0).then_some((position, declared))
        })
        .collect::<Vec<_>>();
    let mut total_decoded = 0_usize;
    let mut next = 0;
    while next < compressed.len() {
        let budget = limits
            .max_total_decoded_bytes
            .saturating_sub(total_decoded)
            .clamp(1, PARALLEL_LZMA_BYTES);
        let mut batch_end = next;
        let mut batch_bytes = 0_usize;
        while batch_end < compressed.len() {
            let declared = compressed[batch_end].1;
            if batch_end > next && batch_bytes.saturating_add(declared) > budget {
                break;
            }
            batch_bytes = batch_bytes.saturating_add(declared);
            batch_end += 1;
            if batch_bytes >= budget {
                break;
            }
        }
        let results = compressed[next..batch_end]
            .par_iter()
            .map(|&(position, declared)| {
                let lump = &lumps[position];
                decode_lzma(
                    &source[lump.encoded_range.clone()],
                    declared,
                    lump.index,
                    limits,
                )
            })
            .collect::<Vec<_>>();
        for (&(position, _), result) in compressed[next..batch_end].iter().zip(results) {
            let lump = &mut lumps[position];
            let (compression, decoded) = result?;
            total_decoded = total_decoded.checked_add(decoded.len()).ok_or_else(|| {
                failure(
                    ErrorCode::DecodedBudget,
                    Some(lump.index),
                    lump.encoded_range.clone(),
                    None,
                    Some(limits.max_total_decoded_bytes),
                )
            })?;
            if total_decoded > limits.max_total_decoded_bytes {
                return Err(failure(
                    ErrorCode::DecodedBudget,
                    Some(lump.index),
                    lump.encoded_range.clone(),
                    Some(total_decoded),
                    Some(limits.max_total_decoded_bytes),
                ));
            }
            lump.compression = Some(compression);
            lump.decoded = Some(decoded);
        }
        next = batch_end;
    }

    for lump in &mut lumps {
        let bytes = lump
            .decoded
            .as_deref()
            .unwrap_or(&source[lump.encoded_range.clone()]);
        lump.records =
            records::parse_lump(lump.index, lump.version, bytes, limits.max_records_per_lump)?;
        lump.coverage = if bytes.is_empty() {
            LumpCoverage::IntentionallyInert
        } else if !records::is_implemented(lump.index) {
            LumpCoverage::Unknown
        } else if !records::version_supported(lump.index, lump.version) {
            LumpCoverage::Unsupported
        } else {
            LumpCoverage::Handled
        };
        if lump.index == records::PAKFILE
            && !bytes.is_empty()
            && lump.coverage == LumpCoverage::Handled
        {
            lump.pak = Some(pak::parse(
                bytes,
                lump.index,
                limits.max_pak_entries,
                limits.max_decoded_bytes_per_pak_entry,
                limits.max_decoded_pak_bytes,
                limits.max_compression_ratio,
            )?);
        }
    }

    Ok(Bsp {
        profile,
        container_version,
        map_revision: i32_at(source, 1_032),
        lumps,
        source: Arc::from(source),
    })
}

pub fn decode_source_lzma_member(
    encoded: &[u8],
    decoded_size: usize,
    limits: Limits,
) -> Result<Vec<u8>, ParseError> {
    decode_lzma(encoded, decoded_size, 35, limits).map(|(_, decoded)| decoded)
}

fn decode_lzma(
    encoded: &[u8],
    declared: usize,
    index: usize,
    limits: Limits,
) -> Result<(Compression, Vec<u8>), ParseError> {
    let range = 8 + index * 16 + 12..8 + index * 16 + 16;
    if declared > limits.max_decoded_bytes_per_lump {
        return Err(failure(
            ErrorCode::DecodedBudget,
            Some(index),
            range,
            Some(declared),
            Some(limits.max_decoded_bytes_per_lump),
        ));
    }
    if encoded.len() < 17 || &encoded[..4] != b"LZMA" {
        return Err(failure(
            ErrorCode::InvalidCompression,
            Some(index),
            range,
            None,
            None,
        ));
    }
    let header_decoded = u32_at(encoded, 4) as usize;
    let payload_size = u32_at(encoded, 8) as usize;
    if header_decoded != declared || payload_size != encoded.len() - 17 {
        return Err(failure(
            ErrorCode::InvalidCompression,
            Some(index),
            range,
            Some(header_decoded),
            Some(declared),
        ));
    }
    if payload_size == 0 || declared > payload_size.saturating_mul(limits.max_compression_ratio) {
        return Err(failure(
            ErrorCode::CompressionRatio,
            Some(index),
            range,
            Some(declared),
            Some(payload_size.saturating_mul(limits.max_compression_ratio)),
        ));
    }
    let properties: [u8; 5] = encoded[12..17].try_into().expect("fixed range");
    let dictionary = u32::from_le_bytes(properties[1..5].try_into().expect("LZMA dictionary"));
    let decoder = lzma_rust2::LzmaReader::new_with_props(
        Cursor::new(&encoded[17..]),
        declared as u64,
        properties[0],
        dictionary,
        None,
    )
    .map_err(|_| {
        failure(
            ErrorCode::DecompressionFailed,
            Some(index),
            range.clone(),
            None,
            None,
        )
    })?;
    let mut decoded = Vec::with_capacity(declared);
    decoder
        .take((declared as u64).saturating_add(1))
        .read_to_end(&mut decoded)
        .map_err(|_| {
            failure(
                ErrorCode::DecompressionFailed,
                Some(index),
                range.clone(),
                None,
                None,
            )
        })?;
    if decoded.len() != declared {
        return Err(failure(
            ErrorCode::DecompressionFailed,
            Some(index),
            range,
            Some(decoded.len()),
            Some(declared),
        ));
    }
    Ok((
        Compression {
            decoded_size: declared,
            encoded_payload_size: payload_size,
            properties,
        },
        decoded,
    ))
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    !left.is_empty() && !right.is_empty() && left.start < right.end && right.start < left.end
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated BSP range"),
    )
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated BSP range"),
    )
}

pub(crate) fn failure(
    code: ErrorCode,
    lump: Option<usize>,
    range: Range<usize>,
    declared: Option<usize>,
    limit: Option<usize>,
) -> ParseError {
    ParseError {
        code,
        lump,
        range,
        declared,
        limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_bsp() -> Vec<u8> {
        let mut bytes = vec![0; HEADER_BYTES];
        bytes[..4].copy_from_slice(b"VBSP");
        bytes[4..8].copy_from_slice(&20_i32.to_le_bytes());
        bytes[1_032..].copy_from_slice(&7_i32.to_le_bytes());
        bytes
    }

    fn set_lump(
        bytes: &mut [u8],
        index: usize,
        offset: i32,
        length: i32,
        version: i32,
        fourth: i32,
    ) {
        let header = 8 + index * 16;
        bytes[header..header + 4].copy_from_slice(&offset.to_le_bytes());
        bytes[header + 4..header + 8].copy_from_slice(&length.to_le_bytes());
        bytes[header + 8..header + 12].copy_from_slice(&version.to_le_bytes());
        bytes[header + 12..header + 16].copy_from_slice(&fourth.to_le_bytes());
    }

    #[test]
    fn parses_all_descriptors_revision_zero_lengths_and_unaligned_ranges() {
        let mut bytes = empty_bsp();
        bytes.extend_from_slice(b"abc");
        set_lump(&mut bytes, 0, HEADER_BYTES as i32, 3, 4, 0);
        set_lump(&mut bytes, 1, 17, 0, 9, 123);
        let bsp = parse(&bytes, Profile::Source2013V20, Limits::default()).unwrap();
        assert_eq!(bsp.lumps.len(), LUMP_COUNT);
        assert_eq!(bsp.map_revision, 7);
        assert_eq!(bsp.lumps[0].version, 4);
        assert_eq!(bsp.lumps[0].bytes(&bsp), b"abc");
        assert_eq!(bsp.lumps[1].encoded_range, 17..17);
        assert_eq!(bsp.lumps[1].raw_fourth_field, 123_i32.to_le_bytes());
        assert_eq!(bsp.source_bytes(), bytes);
    }

    #[test]
    fn retains_partial_and_exact_overlaps() {
        let mut bytes = empty_bsp();
        bytes.extend_from_slice(b"abcdefgh");
        set_lump(&mut bytes, 61, HEADER_BYTES as i32, 4, 0, 0);
        set_lump(&mut bytes, 62, (HEADER_BYTES + 2) as i32, 4, 0, 0);
        set_lump(&mut bytes, 63, HEADER_BYTES as i32, 4, 0, 0);
        let bsp = parse(&bytes, Profile::Source2013V20, Limits::default()).unwrap();
        assert_eq!(bsp.lumps[61].overlaps, vec![62, 63]);
        assert_eq!(bsp.lumps[62].overlaps, vec![61, 63]);
        assert_eq!(bsp.lumps[63].overlaps, vec![61, 62]);
    }

    #[test]
    fn decodes_declared_source_lzma() {
        let payload = b"bounded compressed lump";
        let mut alone = Vec::new();
        lzma_rs::lzma_compress(&mut Cursor::new(payload), &mut alone).unwrap();
        let mut encoded = b"LZMA".to_vec();
        encoded.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        encoded.extend_from_slice(&((alone.len() - 13) as u32).to_le_bytes());
        encoded.extend_from_slice(&alone[..5]);
        encoded.extend_from_slice(&alone[13..]);
        let mut bytes = empty_bsp();
        let offset = bytes.len();
        bytes.extend_from_slice(&encoded);
        set_lump(
            &mut bytes,
            4,
            offset as i32,
            encoded.len() as i32,
            2,
            payload.len() as i32,
        );
        let bsp = parse(&bytes, Profile::Source2013V20, Limits::default()).unwrap();
        assert_eq!(bsp.lumps[4].bytes(&bsp), payload);
        assert_eq!(bsp.lumps[4].encoded_bytes(&bsp), encoded);
    }

    #[test]
    fn parallel_source_lumps_retain_exact_order_bytes_and_budget_failures() {
        let encode = |payload: &[u8]| {
            let mut alone = Vec::new();
            lzma_rs::lzma_compress(&mut Cursor::new(payload), &mut alone).unwrap();
            let mut encoded = b"LZMA".to_vec();
            encoded.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            encoded.extend_from_slice(&((alone.len() - 13) as u32).to_le_bytes());
            encoded.extend_from_slice(&alone[..5]);
            encoded.extend_from_slice(&alone[13..]);
            encoded
        };
        let first = vec![0x31; 4_096];
        let second = vec![0x72; 8_192];
        let mut bytes = empty_bsp();
        for (index, payload) in [(61, &first), (62, &second)] {
            let encoded = encode(payload);
            let offset = bytes.len();
            bytes.extend_from_slice(&encoded);
            set_lump(
                &mut bytes,
                index,
                offset as i32,
                encoded.len() as i32,
                0,
                payload.len() as i32,
            );
        }

        let parsed = parse(&bytes, Profile::Source2013V20, Limits::default()).unwrap();
        assert_eq!(parsed.lumps[61].bytes(&parsed), first);
        assert_eq!(parsed.lumps[62].bytes(&parsed), second);

        let limited = parse(
            &bytes,
            Profile::Source2013V20,
            Limits {
                max_total_decoded_bytes: first.len() + second.len() - 1,
                ..Limits::default()
            },
        )
        .unwrap_err();
        assert_eq!(limited.code, ErrorCode::DecodedBudget);
        assert_eq!(limited.lump, Some(62));

        let mut malformed = bytes;
        for index in [61, 62] {
            let offset = i32_at(&malformed, 8 + index * 16) as usize;
            malformed[offset + 12] = 0xff;
        }
        let failure = parse(&malformed, Profile::Source2013V20, Limits::default()).unwrap_err();
        assert_eq!(failure.code, ErrorCode::DecompressionFailed);
        assert_eq!(failure.lump, Some(61));
    }

    #[test]
    fn rejects_header_profile_range_and_budget_failures() {
        let mut wrong = empty_bsp();
        wrong[..4].copy_from_slice(b"XXXX");
        assert_eq!(
            parse(&wrong, Profile::Source2013V20, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidIdentifier
        );
        assert_eq!(
            parse(
                &empty_bsp()[..100],
                Profile::Source2013V20,
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::TruncatedHeader
        );

        let mut version = empty_bsp();
        version[4..8].copy_from_slice(&19_i32.to_le_bytes());
        assert_eq!(
            parse(&version, Profile::Source2013V20, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::ProfileVersionMismatch
        );

        let mut negative = empty_bsp();
        set_lump(&mut negative, 0, -1, 1, 0, 0);
        assert_eq!(
            parse(&negative, Profile::Source2013V20, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::NegativeRange
        );

        let mut truncated = empty_bsp();
        set_lump(&mut truncated, 0, HEADER_BYTES as i32, 1, 0, 0);
        assert_eq!(
            parse(&truncated, Profile::Source2013V20, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::TruncatedRange
        );

        let limits = Limits {
            max_input_bytes: 10,
            ..Limits::default()
        };
        assert_eq!(
            parse(&empty_bsp(), Profile::Source2013V20, limits)
                .unwrap_err()
                .code,
            ErrorCode::InputLimit
        );
    }

    #[test]
    fn rejects_declared_compression_mismatches_before_decoding() {
        let mut bytes = empty_bsp();
        let offset = bytes.len();
        bytes.extend_from_slice(b"not lzma");
        set_lump(&mut bytes, 0, offset as i32, 8, 0, 100);
        assert_eq!(
            parse(&bytes, Profile::Source2013V20, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidCompression
        );
    }
}
