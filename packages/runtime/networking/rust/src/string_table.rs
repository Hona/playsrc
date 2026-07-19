use std::collections::BTreeMap;

use crate::{BitReader, Classification, CodecError, ErrorCode, Limits, failure};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringEntry {
    pub value: Vec<u8>,
    pub user_data: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringTable {
    pub id: u8,
    pub name: Vec<u8>,
    pub max_entries: u16,
    pub entry_bits: u8,
    pub user_data_fixed_size: bool,
    pub user_data_size_bytes: u16,
    pub user_data_size_bits: u8,
    pub entries: Vec<StringEntry>,
    pub client_entries: Vec<StringEntry>,
}

impl StringTable {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create(
        id: u8,
        name: Vec<u8>,
        max_entries: u16,
        user_data_fixed_size: bool,
        user_data_size_bytes: u16,
        user_data_size_bits: u8,
        limits: Limits,
    ) -> Result<Self, CodecError> {
        let max = usize::from(max_entries);
        if max == 0 || max > limits.max_entries_per_string_table {
            return Err(limit_error(
                ErrorCode::InvalidStringIndex,
                "string_table.max_entries",
                max,
                limits.max_entries_per_string_table,
            ));
        }
        if usize::from(user_data_size_bytes) > limits.max_string_user_data_bytes {
            return Err(limit_error(
                ErrorCode::UserDataLimit,
                "string_table.user_data_size",
                usize::from(user_data_size_bytes),
                limits.max_string_user_data_bytes,
            ));
        }
        Ok(Self {
            id,
            name,
            max_entries,
            entry_bits: (usize::BITS as usize - max.leading_zeros() as usize - 1) as u8,
            user_data_fixed_size,
            user_data_size_bytes,
            user_data_size_bits,
            entries: Vec::new(),
            client_entries: Vec::new(),
        })
    }

    pub(crate) fn apply_update(
        &mut self,
        reader: &mut BitReader<'_>,
        changed_entries: usize,
        limits: Limits,
    ) -> Result<Vec<usize>, CodecError> {
        if changed_entries > usize::from(self.max_entries) {
            return Err(limit_error(
                ErrorCode::InvalidStringIndex,
                "string_table.changed_entries",
                changed_entries,
                usize::from(self.max_entries),
            ));
        }
        let mut last_entry = None;
        let mut history: Vec<Vec<u8>> = Vec::new();
        let mut changed = Vec::with_capacity(changed_entries);
        for _ in 0..changed_entries {
            let sequential = reader.read_bit("string_table.entry.sequential")?;
            let entry_index = if sequential {
                last_entry.map_or(0, |last| last + 1)
            } else {
                reader.read_unsigned(usize::from(self.entry_bits), "string_table.entry.index")?
                    as usize
            };
            if entry_index >= usize::from(self.max_entries) {
                return Err(limit_error(
                    ErrorCode::InvalidStringIndex,
                    "string_table.entry.index",
                    entry_index,
                    usize::from(self.max_entries) - 1,
                ));
            }
            last_entry = Some(entry_index);
            let has_string = reader.read_bit("string_table.entry.has_string")?;
            let encoded_string = if has_string {
                if reader.read_bit("string_table.entry.substring")? {
                    let history_index =
                        reader.read_unsigned(5, "string_table.entry.history_index")? as usize;
                    let prefix_bytes =
                        reader.read_unsigned(5, "string_table.entry.prefix_bytes")? as usize;
                    let prior = history.get(history_index).ok_or_else(|| {
                        simple_error(
                            Classification::Malformed,
                            ErrorCode::InvalidStringHistory,
                            "string_table.entry.history_index",
                        )
                    })?;
                    if prefix_bytes > prior.len() {
                        return Err(simple_error(
                            Classification::Malformed,
                            ErrorCode::InvalidStringHistory,
                            "string_table.entry.prefix_bytes",
                        ));
                    }
                    let suffix = reader.read_c_string(
                        limits.max_string_bytes,
                        "string_table.entry.substring_suffix",
                    )?;
                    if prefix_bytes + suffix.len() > limits.max_string_bytes {
                        return Err(limit_error(
                            ErrorCode::StringLimit,
                            "string_table.entry.value",
                            prefix_bytes + suffix.len(),
                            limits.max_string_bytes,
                        ));
                    }
                    let mut value = prior[..prefix_bytes].to_vec();
                    value.extend_from_slice(&suffix);
                    Some(value)
                } else {
                    Some(
                        reader
                            .read_c_string(limits.max_string_bytes, "string_table.entry.value")?,
                    )
                }
            } else {
                None
            };
            let user_data = if reader.read_bit("string_table.entry.has_user_data")? {
                if self.user_data_fixed_size {
                    let bit_length = usize::from(self.user_data_size_bits);
                    let payload =
                        reader.read_payload(bit_length, "string_table.entry.user_data")?;
                    Some(payload.bytes)
                } else {
                    let byte_length =
                        reader.read_unsigned(14, "string_table.entry.user_data_bytes")? as usize;
                    if byte_length > limits.max_string_user_data_bytes {
                        return Err(limit_error(
                            ErrorCode::UserDataLimit,
                            "string_table.entry.user_data_bytes",
                            byte_length,
                            limits.max_string_user_data_bytes,
                        ));
                    }
                    Some(
                        reader
                            .read_payload(byte_length * 8, "string_table.entry.user_data")?
                            .bytes,
                    )
                }
            } else {
                None
            };

            if entry_index < self.entries.len() {
                if let Some(value) = encoded_string.as_ref()
                    && value != &self.entries[entry_index].value
                {
                    return Err(simple_error(
                        Classification::Malformed,
                        ErrorCode::InvalidStringIndex,
                        "string_table.entry.replacement_value",
                    ));
                }
                self.entries[entry_index].user_data = user_data;
            } else if entry_index == self.entries.len() {
                let value = encoded_string.ok_or_else(|| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::InvalidStringIndex,
                        "string_table.entry.new_value",
                    )
                })?;
                self.entries.push(StringEntry { value, user_data });
            } else {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidStringIndex,
                    "string_table.entry.gap",
                ));
            }

            if history.len() == 32 {
                history.remove(0);
            }
            history.push(self.entries[entry_index].value.clone());
            changed.push(entry_index);
        }
        Ok(changed)
    }
}

pub(crate) fn parse_demo_string_tables(
    bytes: &[u8],
    bit_length: usize,
    tables: &mut BTreeMap<u8, StringTable>,
    limits: Limits,
) -> Result<(), CodecError> {
    let mut reader = BitReader::new(bytes, bit_length)?;
    let table_count = reader.read_u8("demo_string_tables.table_count")? as usize;
    if table_count > limits.max_string_tables || table_count != tables.len() {
        return Err(limit_error(
            ErrorCode::StringTableLimit,
            "demo_string_tables.table_count",
            table_count,
            limits.max_string_tables,
        ));
    }
    for _ in 0..table_count {
        let name =
            reader.read_c_string(limits.max_string_bytes, "demo_string_tables.table_name")?;
        let (_, table) = tables
            .iter_mut()
            .find(|(_, table)| table.name == name)
            .ok_or_else(|| {
                simple_error(
                    Classification::Missing,
                    ErrorCode::MissingStringTable,
                    "demo_string_tables.table_name",
                )
            })?;
        table.entries = parse_standalone_entries(&mut reader, limits)?;
        table.client_entries = if reader.read_bit("demo_string_tables.has_client_entries")? {
            parse_standalone_entries(&mut reader, limits)?
        } else {
            Vec::new()
        };
        if table.entries.len() > usize::from(table.max_entries) {
            return Err(limit_error(
                ErrorCode::InvalidStringIndex,
                "demo_string_tables.entries",
                table.entries.len(),
                usize::from(table.max_entries),
            ));
        }
    }
    if reader.bits_left() > 7 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::TrailingBits,
            "demo_string_tables.trailing_bits",
        ));
    }
    Ok(())
}

fn parse_standalone_entries(
    reader: &mut BitReader<'_>,
    limits: Limits,
) -> Result<Vec<StringEntry>, CodecError> {
    let count = reader.read_u16("demo_string_tables.entry_count")? as usize;
    if count > limits.max_entries_per_string_table {
        return Err(limit_error(
            ErrorCode::InvalidStringIndex,
            "demo_string_tables.entry_count",
            count,
            limits.max_entries_per_string_table,
        ));
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let value =
            reader.read_c_string(limits.max_string_bytes, "demo_string_tables.entry.value")?;
        let user_data = if reader.read_bit("demo_string_tables.entry.has_user_data")? {
            let bytes = reader.read_u16("demo_string_tables.entry.user_data_bytes")? as usize;
            if bytes == 0 || bytes > limits.max_string_user_data_bytes {
                return Err(limit_error(
                    ErrorCode::UserDataLimit,
                    "demo_string_tables.entry.user_data_bytes",
                    bytes,
                    limits.max_string_user_data_bytes,
                ));
            }
            Some(
                reader
                    .read_payload(bytes * 8, "demo_string_tables.entry.user_data")?
                    .bytes,
            )
        } else {
            None
        };
        entries.push(StringEntry { value, user_data });
    }
    Ok(entries)
}

fn simple_error(
    classification: Classification,
    code: ErrorCode,
    field: &'static str,
) -> CodecError {
    failure(classification, code, 0..0, None, field, None, None, None)
}

fn limit_error(code: ErrorCode, field: &'static str, actual: usize, limit: usize) -> CodecError {
    failure(
        Classification::Malformed,
        code,
        0..0,
        None,
        field,
        Some(actual as i64),
        None,
        Some(limit as u64),
    )
}
