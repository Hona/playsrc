use std::{
    collections::HashSet,
    io::{Cursor, Read},
    ops::Range,
};

use crate::{ErrorCode, ParseError, failure};
use rayon::prelude::*;

const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const END_SIGNATURE: u32 = 0x0605_4b50;
const MAX_END_SEARCH: usize = 65_557;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PakEntryClassification {
    Handled,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PakEntry {
    pub raw_name: Vec<u8>,
    pub flags: u16,
    pub compression_method: u16,
    pub crc32: u32,
    pub encoded_size: u32,
    pub decoded_size: u32,
    pub local_header_offset: u32,
    pub local_header_range: Range<usize>,
    pub local_extra: Vec<u8>,
    pub encoded_range: Range<usize>,
    pub central_header_range: Range<usize>,
    pub central_extra: Vec<u8>,
    pub comment: Vec<u8>,
    pub classification: PakEntryClassification,
    pub decoded: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pak {
    pub entries: Vec<PakEntry>,
    pub central_directory_range: Range<usize>,
    pub central_directory_tail: Vec<u8>,
    pub end_record_range: Range<usize>,
    pub archive_comment: Vec<u8>,
}

pub(crate) fn parse(
    bytes: &[u8],
    lump: usize,
    max_entries: usize,
    max_entry_decoded_bytes: usize,
    max_total_decoded_bytes: usize,
    max_compression_ratio: usize,
) -> Result<Pak, ParseError> {
    let end_offset = find_end(bytes).ok_or_else(|| malformed(lump, 0..bytes.len()))?;
    let comment_length = u16_at(bytes, end_offset + 20) as usize;
    let end_record_end = end_offset + 22 + comment_length;
    if u16_at(bytes, end_offset + 4) != 0
        || u16_at(bytes, end_offset + 6) != 0
        || u16_at(bytes, end_offset + 8) != u16_at(bytes, end_offset + 10)
    {
        return Err(malformed(lump, end_offset..end_record_end));
    }
    let entry_count = u16_at(bytes, end_offset + 10) as usize;
    if entry_count > max_entries {
        return Err(failure(
            ErrorCode::RecordBudget,
            Some(lump),
            end_offset + 8..end_offset + 12,
            Some(entry_count),
            Some(max_entries),
        ));
    }
    let central_size = u32_at(bytes, end_offset + 12) as usize;
    let central_start = u32_at(bytes, end_offset + 16) as usize;
    let central_end = central_start
        .checked_add(central_size)
        .ok_or_else(|| malformed(lump, end_offset + 12..end_offset + 20))?;
    if central_end > end_offset || central_end > bytes.len() {
        return Err(malformed(lump, central_start..central_end));
    }

    let mut entries = Vec::with_capacity(entry_count);
    let mut cursor = central_start;
    let mut local_offsets = HashSet::with_capacity(entry_count);
    for _ in 0..entry_count {
        let fixed_end = cursor
            .checked_add(46)
            .ok_or_else(|| malformed(lump, cursor..usize::MAX))?;
        if fixed_end > central_end || u32_at(bytes, cursor) != CENTRAL_SIGNATURE {
            return Err(malformed(lump, cursor..fixed_end));
        }
        let name_length = u16_at(bytes, cursor + 28) as usize;
        let extra_length = u16_at(bytes, cursor + 30) as usize;
        let entry_comment_length = u16_at(bytes, cursor + 32) as usize;
        if u16_at(bytes, cursor + 34) != 0 {
            return Err(malformed(lump, cursor + 34..cursor + 36));
        }
        let central_header_end = fixed_end
            .checked_add(name_length)
            .and_then(|value| value.checked_add(extra_length))
            .and_then(|value| value.checked_add(entry_comment_length))
            .ok_or_else(|| malformed(lump, cursor..usize::MAX))?;
        if central_header_end > central_end {
            return Err(malformed(lump, cursor..central_header_end));
        }

        let flags = u16_at(bytes, cursor + 8);
        let method = u16_at(bytes, cursor + 10);
        let crc32 = u32_at(bytes, cursor + 16);
        let encoded_size = u32_at(bytes, cursor + 20);
        let decoded_size = u32_at(bytes, cursor + 24);
        let local_offset = u32_at(bytes, cursor + 42);
        if flags & 1 != 0 || !local_offsets.insert(local_offset) {
            return Err(malformed(lump, cursor..central_header_end));
        }
        let name_start = fixed_end;
        let extra_start = name_start + name_length;
        let comment_start = extra_start + extra_length;
        let raw_name = bytes[name_start..extra_start].to_vec();

        let local_start = local_offset as usize;
        let local_fixed_end = local_start
            .checked_add(30)
            .ok_or_else(|| malformed(lump, local_start..usize::MAX))?;
        if local_fixed_end > bytes.len() || u32_at(bytes, local_start) != LOCAL_SIGNATURE {
            return Err(malformed(lump, local_start..local_fixed_end));
        }
        let local_name_length = u16_at(bytes, local_start + 26) as usize;
        let local_extra_length = u16_at(bytes, local_start + 28) as usize;
        let local_header_end = local_fixed_end
            .checked_add(local_name_length)
            .and_then(|value| value.checked_add(local_extra_length))
            .ok_or_else(|| malformed(lump, local_start..usize::MAX))?;
        let encoded_end = local_header_end
            .checked_add(encoded_size as usize)
            .ok_or_else(|| malformed(lump, local_start..usize::MAX))?;
        if encoded_end > bytes.len()
            || bytes[local_fixed_end..local_fixed_end + local_name_length] != raw_name
            || u16_at(bytes, local_start + 6) != flags
            || u16_at(bytes, local_start + 8) != method
        {
            return Err(malformed(lump, local_start..encoded_end));
        }
        if flags & 0x0008 == 0
            && (u32_at(bytes, local_start + 14) != crc32
                || u32_at(bytes, local_start + 18) != encoded_size
                || u32_at(bytes, local_start + 22) != decoded_size)
        {
            return Err(malformed(lump, local_start..local_header_end));
        }
        if decoded_size as usize > max_entry_decoded_bytes {
            return Err(failure(
                ErrorCode::DecodedBudget,
                Some(lump),
                cursor + 24..cursor + 28,
                Some(decoded_size as usize),
                Some(max_entry_decoded_bytes),
            ));
        }
        let ratio_limit = (encoded_size as usize)
            .saturating_mul(max_compression_ratio)
            .saturating_add(1024 * 1024);
        if decoded_size as usize > ratio_limit {
            return Err(failure(
                ErrorCode::CompressionRatio,
                Some(lump),
                cursor + 20..cursor + 28,
                Some(decoded_size as usize),
                Some(ratio_limit),
            ));
        }

        entries.push(PakEntry {
            raw_name,
            flags,
            compression_method: method,
            crc32,
            encoded_size,
            decoded_size,
            local_header_offset: local_offset,
            local_header_range: local_start..local_header_end,
            local_extra: bytes[local_fixed_end + local_name_length..local_header_end].to_vec(),
            encoded_range: local_header_end..encoded_end,
            central_header_range: cursor..central_header_end,
            central_extra: bytes[extra_start..comment_start].to_vec(),
            comment: bytes[comment_start..central_header_end].to_vec(),
            classification: if matches!(method, 0 | 14) {
                PakEntryClassification::Handled
            } else {
                PakEntryClassification::Unsupported
            },
            decoded: None,
        });
        cursor = central_header_end;
    }

    let mut total_decoded = 0_usize;
    let archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| malformed(lump, 0..bytes.len()))?;
    let selected = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            (entry.classification == PakEntryClassification::Handled).then_some(index)
        })
        .collect::<Vec<_>>();
    let mut next = 0;
    while next < selected.len() {
        // Bound transient decompressor state independently of retained PAK payloads.
        // Cloned archives borrow the same source slice and share their parsed index.
        let mut batch_bytes = 0usize;
        let mut jobs = Vec::with_capacity(4);
        while next < selected.len() && jobs.len() < 4 {
            let index = selected[next];
            let entry = &entries[index];
            let size = entry.decoded_size as usize;
            if !jobs.is_empty() && batch_bytes.saturating_add(size) > 32 * 1024 * 1024 {
                break;
            }
            let budget = total_decoded
                .checked_add(size)
                .ok_or_else(|| malformed(lump, entry.central_header_range.clone()))
                .and_then(|total| {
                    total_decoded = total;
                    if total > max_total_decoded_bytes {
                        Err(failure(
                            ErrorCode::DecodedBudget,
                            Some(lump),
                            entry.central_header_range.clone(),
                            Some(total),
                            Some(max_total_decoded_bytes),
                        ))
                    } else {
                        Ok(())
                    }
                });
            let failed = budget.is_err();
            jobs.push((index, budget));
            next += 1;
            batch_bytes = batch_bytes.saturating_add(size);
            if failed {
                break;
            }
        }
        let results = jobs
            .par_iter()
            .map(|(index, budget)| {
                budget.clone()?;
                let entry = &entries[*index];
                let mut archive = archive.clone();
                let mut file = archive
                    .by_index(*index)
                    .map_err(|_| malformed(lump, entry.local_header_range.clone()))?;
                let mut decoded = Vec::with_capacity(entry.decoded_size as usize);
                file.by_ref()
                    .take(entry.decoded_size as u64 + 1)
                    .read_to_end(&mut decoded)
                    .map_err(|_| malformed(lump, entry.encoded_range.clone()))?;
                if decoded.len() != entry.decoded_size as usize
                    || crc32fast::hash(&decoded) != entry.crc32
                {
                    return Err(malformed(lump, entry.encoded_range.clone()));
                }
                Ok(decoded)
            })
            .collect::<Vec<_>>();
        // Observe errors in central-directory order, including an earlier corrupt
        // entry before a later budget failure. No partial PAK is published.
        for ((index, _), result) in jobs.into_iter().zip(results) {
            entries[index].decoded = Some(result?);
        }
    }

    Ok(Pak {
        entries,
        central_directory_range: central_start..central_end,
        central_directory_tail: bytes[cursor..central_end].to_vec(),
        end_record_range: end_offset..end_record_end,
        archive_comment: bytes[end_offset + 22..end_record_end].to_vec(),
    })
}

fn find_end(bytes: &[u8]) -> Option<usize> {
    let start = bytes.len().saturating_sub(MAX_END_SEARCH);
    if bytes.len() < 22 {
        return None;
    }
    (start..=bytes.len() - 22).rev().find(|offset| {
        u32_at(bytes, *offset) == END_SIGNATURE
            && (*offset + 22 + u16_at(bytes, *offset + 20) as usize == bytes.len())
    })
}

fn malformed(lump: usize, range: Range<usize>) -> ParseError {
    failure(ErrorCode::InvalidPak, Some(lump), range, None, None)
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated ZIP field"),
    )
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated ZIP field"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_archive(name: &[u8], data: &[u8]) -> Vec<u8> {
        archive(name, data, 0, data)
    }

    fn archive(name: &[u8], decoded: &[u8], method: u16, encoded: &[u8]) -> Vec<u8> {
        let crc = crc32fast::hash(decoded);
        let local_extra = [0x11, 0x22];
        let central_extra = [0x33, 0x44, 0x55];
        let entry_comment = [0x66];
        let archive_comment = [0x77, 0x88];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&LOCAL_SIGNATURE.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&method.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(decoded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(local_extra.len() as u16).to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&local_extra);
        bytes.extend_from_slice(encoded);

        let central_start = bytes.len();
        bytes.extend_from_slice(&CENTRAL_SIGNATURE.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&method.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(decoded.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(central_extra.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(entry_comment.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&central_extra);
        bytes.extend_from_slice(&entry_comment);
        let central_size = bytes.len() - central_start;

        bytes.extend_from_slice(&END_SIGNATURE.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&(central_size as u32).to_le_bytes());
        bytes.extend_from_slice(&(central_start as u32).to_le_bytes());
        bytes.extend_from_slice(&(archive_comment.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&archive_comment);
        bytes
    }

    #[test]
    fn retains_zip_headers_and_decodes_stored_entries() {
        let bytes = stored_archive(b"materials/a.vmt", b"content");
        let pak = parse(&bytes, 40, 1, 100, 100, 100).unwrap();
        assert_eq!(pak.entries.len(), 1);
        let entry = &pak.entries[0];
        assert_eq!(entry.raw_name, b"materials/a.vmt");
        assert_eq!(entry.local_extra, [0x11, 0x22]);
        assert_eq!(entry.central_extra, [0x33, 0x44, 0x55]);
        assert_eq!(entry.comment, [0x66]);
        assert_eq!(entry.decoded.as_deref(), Some(b"content".as_slice()));
        assert_eq!(pak.archive_comment, [0x77, 0x88]);
    }

    #[test]
    fn decodes_zip_lzma_entries() {
        let decoded = b"bounded lzma content";
        let mut alone = Vec::new();
        lzma_rs::lzma_compress(&mut Cursor::new(decoded), &mut alone).unwrap();
        let mut encoded = vec![9, 4, 5, 0];
        encoded.extend_from_slice(&alone[..5]);
        encoded.extend_from_slice(&alone[13..]);
        let bytes = archive(b"materials/lzma.vmt", decoded, 14, &encoded);
        let pak = parse(&bytes, 40, 1, 100, 100, 100).unwrap();
        assert_eq!(pak.entries[0].compression_method, 14);
        assert_eq!(
            pak.entries[0].decoded.as_deref(),
            Some(b"bounded lzma content".as_slice())
        );
    }

    #[test]
    fn enforces_entry_and_integrity_boundaries() {
        let bytes = stored_archive(b"a", b"content");
        assert_eq!(
            parse(&bytes, 40, 0, 100, 100, 100).unwrap_err().code,
            ErrorCode::RecordBudget
        );
        assert_eq!(
            parse(&bytes, 40, 1, 6, 100, 100).unwrap_err().code,
            ErrorCode::DecodedBudget
        );
        let mut corrupt = bytes;
        corrupt[33] ^= 1;
        assert_eq!(
            parse(&corrupt, 40, 1, 100, 100, 100).unwrap_err().code,
            ErrorCode::InvalidPak
        );
    }

    #[test]
    fn parallel_pak_decode_preserves_bytes_and_directory_error_precedence() {
        let mut bytes = Vec::new();
        let mut central = Vec::new();
        for index in 0..12 {
            let data = vec![index as u8; 4096 + index];
            let name = format!("materials/{index}.vmt");
            let single = if index % 2 == 0 {
                stored_archive(name.as_bytes(), &data)
            } else {
                let mut alone = Vec::new();
                lzma_rs::lzma_compress(&mut Cursor::new(&data), &mut alone).unwrap();
                let mut encoded = vec![9, 4, 5, 0];
                encoded.extend_from_slice(&alone[..5]);
                encoded.extend_from_slice(&alone[13..]);
                archive(name.as_bytes(), &data, 14, &encoded)
            };
            let end = find_end(&single).unwrap();
            let start = u32_at(&single, end + 16) as usize;
            let mut record = single[start..end].to_vec();
            record[42..46].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            central.extend_from_slice(&record);
            bytes.extend_from_slice(&single[..start]);
        }
        let central_start = bytes.len();
        bytes.extend_from_slice(&central);
        bytes.extend_from_slice(&END_SIGNATURE.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&12_u16.to_le_bytes());
        bytes.extend_from_slice(&12_u16.to_le_bytes());
        bytes.extend_from_slice(&(central.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(central_start as u32).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());

        let pools = [1, 4].map(|threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
        });
        let serial = pools[0]
            .install(|| parse(&bytes, 40, 12, 8192, 65_536, 1024))
            .unwrap();
        let parallel = pools[1]
            .install(|| parse(&bytes, 40, 12, 8192, 65_536, 1024))
            .unwrap();
        assert_eq!(serial, parallel);
        for (index, entry) in parallel.entries.iter().enumerate() {
            assert_eq!(
                entry.decoded.as_deref(),
                Some(vec![index as u8; 4096 + index].as_slice())
            );
        }
        let budget = parse(&bytes, 40, 12, 8192, 4096, 1024).unwrap_err();
        assert_eq!(budget.code, ErrorCode::DecodedBudget);
        bytes[parallel.entries[0].encoded_range.start] ^= 1;
        let errors = pools.map(|pool| {
            pool.install(|| parse(&bytes, 40, 12, 8192, 4096, 1024))
                .unwrap_err()
        });
        assert_eq!(errors[0], errors[1]);
        assert_eq!(errors[0].code, ErrorCode::InvalidPak);
    }
}
