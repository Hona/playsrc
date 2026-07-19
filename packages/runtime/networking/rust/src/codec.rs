use std::{collections::BTreeMap, sync::Arc};

use snap::raw::{Decoder, decompress_len};

use crate::{
    BitPayload, BitReader, Classification, CodecError, ErrorCode, Limits,
    entity::{EntityChange, EntitySnapshot, reduce_packet_entities, refresh_class_baselines},
    event::{Event, EventSchema, parse_event, parse_event_schemas},
    failure,
    message::{CreateStringTable, Message, MessageBody, UserMessage, decode_message},
    schema::{SchemaRegistry, parse_demo_data_tables},
    string_table::{StringTable, parse_demo_string_tables},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodecState {
    pub schema: Arc<SchemaRegistry>,
    pub string_tables: Arc<BTreeMap<u8, StringTable>>,
    pub event_schemas: Arc<BTreeMap<u16, EventSchema>>,
    pub class_baselines: Arc<BTreeMap<u16, BTreeMap<u16, crate::FieldValue>>>,
    pub rolling_baselines: [Arc<BTreeMap<u16, crate::Entity>>; 2],
    pub snapshots: BTreeMap<i32, Arc<EntitySnapshot>>,
    pub current_server_tick: Option<i32>,
    pub signon_state: Option<(u8, i32)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkOperation {
    ServerTick(i32),
    SignonState {
        state: u8,
        spawn_count: i32,
    },
    SchemaChanged,
    StringTableCreated {
        id: u8,
    },
    StringTableUpdated {
        id: u8,
        entries: Vec<usize>,
    },
    EventSchemasChanged,
    Event(Event),
    UserMessage(UserMessage),
    EntitySnapshot {
        snapshot: Arc<EntitySnapshot>,
        changes: Vec<EntityChange>,
        delta_from: Option<i32>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPacket {
    pub messages: Vec<Message>,
    pub operations: Vec<NetworkOperation>,
    pub trailing_padding: BitPayload,
    pub consumed_bits: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedStateCodec {
    limits: Limits,
    state: CodecState,
}

impl RecordedStateCodec {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            state: CodecState::default(),
        }
    }

    pub fn state(&self) -> &CodecState {
        &self.state
    }

    pub fn snapshot(&self) -> CodecState {
        self.state.clone()
    }

    pub fn restore(&mut self, state: CodecState) -> Result<(), CodecError> {
        validate_state(&state, self.limits)?;
        self.state = state;
        Ok(())
    }

    pub fn replace_demo_data_tables(
        &mut self,
        bytes: &[u8],
        bit_length: usize,
    ) -> Result<NetworkOperation, CodecError> {
        let schema = parse_demo_data_tables(bytes, bit_length, self.limits)?;
        let mut next = self.state.clone();
        next.schema = Arc::new(schema);
        next.class_baselines = Arc::new(BTreeMap::new());
        next.rolling_baselines = [Arc::new(BTreeMap::new()), Arc::new(BTreeMap::new())];
        next.snapshots.clear();
        self.state = next;
        Ok(NetworkOperation::SchemaChanged)
    }

    pub fn replace_demo_string_tables(
        &mut self,
        bytes: &[u8],
        bit_length: usize,
    ) -> Result<(), CodecError> {
        let mut next = self.state.clone();
        parse_demo_string_tables(
            bytes,
            bit_length,
            Arc::make_mut(&mut next.string_tables),
            self.limits,
        )?;
        if !next.schema.classes.is_empty() {
            next.class_baselines = Arc::new(refresh_class_baselines(
                &next.schema,
                &next.string_tables,
                self.limits,
            )?);
        }
        self.state = next;
        Ok(())
    }

    pub fn decode_packet(
        &mut self,
        bytes: &[u8],
        bit_length: usize,
    ) -> Result<DecodedPacket, CodecError> {
        if bit_length > self.limits.max_payload_bits {
            return Err(limit_error(
                ErrorCode::PayloadLimit,
                "packet.bit_length",
                bit_length,
                self.limits.max_payload_bits,
            ));
        }
        let mut next = self.state.clone();
        let mut reader = BitReader::new(bytes, bit_length)?;
        let mut messages = Vec::new();
        let mut operations = Vec::new();
        while reader.bits_left() >= crate::MESSAGE_TYPE_BITS {
            if messages.len() >= self.limits.max_messages_per_payload {
                return Err(limit_error(
                    ErrorCode::MessageLimit,
                    "packet.messages",
                    messages.len() + 1,
                    self.limits.max_messages_per_payload,
                ));
            }
            let ordinal = messages.len();
            reader.set_message_ordinal(ordinal);
            let message = decode_message(&mut reader, ordinal, self.limits)?;
            apply_message(&mut next, &message, self.limits, &mut operations).map_err(
                |mut error| {
                    error.message_ordinal = Some(ordinal);
                    error
                },
            )?;
            messages.push(message);
        }
        let trailing_padding = reader.read_payload(reader.bits_left(), "packet.padding")?;
        let consumed_bits = reader.position();
        self.state = next;
        Ok(DecodedPacket {
            messages,
            operations,
            trailing_padding,
            consumed_bits,
        })
    }
}

fn apply_message(
    state: &mut CodecState,
    message: &Message,
    limits: Limits,
    operations: &mut Vec<NetworkOperation>,
) -> Result<(), CodecError> {
    match &message.body {
        MessageBody::Tick { server_tick, .. } => {
            state.current_server_tick = Some(*server_tick);
            operations.push(NetworkOperation::ServerTick(*server_tick));
        }
        MessageBody::SignonState {
            state: value,
            spawn_count,
        } => {
            state.signon_state = Some((*value, *spawn_count));
            operations.push(NetworkOperation::SignonState {
                state: *value,
                spawn_count: *spawn_count,
            });
        }
        MessageBody::SendTable(table) => {
            Arc::make_mut(&mut state.schema).insert_table(table.clone(), limits)?;
            operations.push(NetworkOperation::SchemaChanged);
        }
        MessageBody::ClassInfo {
            class_count,
            classes,
        } => {
            if let Some(classes) = classes {
                Arc::make_mut(&mut state.schema).replace_classes(classes.clone(), limits)?;
                state.class_baselines = Arc::new(BTreeMap::new());
                operations.push(NetworkOperation::SchemaChanged);
            } else if state.schema.classes.len() != usize::from(*class_count) {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidClass,
                    "class_info.existing_class_count",
                ));
            }
        }
        MessageBody::CreateStringTable(message) => {
            let id = u8::try_from(state.string_tables.len()).map_err(|_| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::StringTableLimit,
                    "string_table.id",
                )
            })?;
            if state.string_tables.len() >= limits.max_string_tables {
                return Err(limit_error(
                    ErrorCode::StringTableLimit,
                    "string_tables",
                    state.string_tables.len() + 1,
                    limits.max_string_tables,
                ));
            }
            let mut table = StringTable::create(
                id,
                message.name.clone(),
                message.max_entries,
                message.user_data_fixed_size,
                message.user_data_size_bytes,
                message.user_data_size_bits,
                limits,
            )?;
            let payload = decode_string_table_payload(message, limits)?;
            let mut reader = payload.reader();
            table.apply_update(&mut reader, usize::from(message.entry_count), limits)?;
            if message.compressed {
                if reader.bits_left() > 7 {
                    return Err(simple_error(
                        Classification::Malformed,
                        ErrorCode::TrailingBits,
                        "string_table_create.compressed_padding",
                    ));
                }
            } else if reader.bits_left() != 0 {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::TrailingBits,
                    "string_table_create.trailing_bits",
                ));
            }
            if Arc::make_mut(&mut state.string_tables)
                .insert(id, table)
                .is_some()
            {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::DuplicateStringTable,
                    "string_table.id",
                ));
            }
            operations.push(NetworkOperation::StringTableCreated { id });
        }
        MessageBody::UpdateStringTable(message) => {
            let table = Arc::make_mut(&mut state.string_tables)
                .get_mut(&message.table_id)
                .ok_or_else(|| {
                    simple_error(
                        Classification::Missing,
                        ErrorCode::MissingStringTable,
                        "string_table_update.table_id",
                    )
                })?;
            let mut reader = message.payload.reader();
            let changed =
                table.apply_update(&mut reader, usize::from(message.changed_entries), limits)?;
            require_zero_padding(&mut reader, "string_table_update.padding")?;
            operations.push(NetworkOperation::StringTableUpdated {
                id: message.table_id,
                entries: changed,
            });
        }
        MessageBody::GameEventList {
            event_count,
            payload,
        } => {
            let schemas = parse_event_schemas(usize::from(*event_count), payload, limits)?;
            let mut next_schemas = BTreeMap::new();
            for schema in schemas {
                if next_schemas.insert(schema.id, schema).is_some() {
                    return Err(simple_error(
                        Classification::Malformed,
                        ErrorCode::DuplicateEvent,
                        "event_schema.id",
                    ));
                }
            }
            state.event_schemas = Arc::new(next_schemas);
            operations.push(NetworkOperation::EventSchemasChanged);
        }
        MessageBody::GameEvent(payload) => {
            operations.push(NetworkOperation::Event(parse_event(
                payload,
                &state.event_schemas,
                limits,
            )?));
        }
        MessageBody::UserMessage(message) => {
            operations.push(NetworkOperation::UserMessage(message.clone()));
        }
        MessageBody::PacketEntities(message) => {
            let server_tick = state.current_server_tick.ok_or_else(|| {
                simple_error(
                    Classification::Missing,
                    ErrorCode::TickRequired,
                    "packet_entities.server_tick",
                )
            })?;
            if state.class_baselines.is_empty() {
                state.class_baselines = Arc::new(refresh_class_baselines(
                    &state.schema,
                    &state.string_tables,
                    limits,
                )?);
            }
            let mut rolling_baselines = [
                (*state.rolling_baselines[0]).clone(),
                (*state.rolling_baselines[1]).clone(),
            ];
            let (snapshot, changes) = reduce_packet_entities(
                message,
                server_tick,
                &state.schema,
                &state.class_baselines,
                &mut rolling_baselines,
                &state.snapshots,
                limits,
            )?;
            state.rolling_baselines = [
                Arc::new(rolling_baselines[0].clone()),
                Arc::new(rolling_baselines[1].clone()),
            ];
            let snapshot = Arc::new(snapshot);
            state.snapshots.insert(server_tick, snapshot.clone());
            while state.snapshots.len() > limits.max_snapshot_history {
                let oldest = *state.snapshots.keys().next().expect("non-empty history");
                state.snapshots.remove(&oldest);
            }
            operations.push(NetworkOperation::EntitySnapshot {
                snapshot,
                changes,
                delta_from: message.delta_from,
            });
        }
        MessageBody::Padding | MessageBody::Opaque(_) => {}
    }
    Ok(())
}

fn decode_string_table_payload(
    message: &CreateStringTable,
    limits: Limits,
) -> Result<BitPayload, CodecError> {
    if !message.compressed {
        return Ok(message.payload.clone());
    }
    let mut reader = message.payload.reader();
    let expected = reader.read_u32("string_table_compression.decoded_bytes")? as usize;
    let encoded = reader.read_u32("string_table_compression.encoded_bytes")? as usize;
    if expected > limits.max_decompressed_bytes {
        return Err(limit_error(
            ErrorCode::DecompressionLimit,
            "string_table_compression.decoded_bytes",
            expected,
            limits.max_decompressed_bytes,
        ));
    }
    if encoded > reader.bits_left() / 8 || encoded.saturating_mul(8) != reader.bits_left() {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidLength,
            "string_table_compression.encoded_bytes",
        ));
    }
    let encoded = reader
        .read_payload(encoded * 8, "string_table_compression.payload")?
        .bytes;
    let decoded = decompress(&encoded, expected, limits)?;
    Ok(BitPayload {
        bit_length: decoded.len() * 8,
        bytes: decoded,
    })
}

fn decompress(encoded: &[u8], expected: usize, limits: Limits) -> Result<Vec<u8>, CodecError> {
    if expected > limits.max_decompressed_bytes {
        return Err(limit_error(
            ErrorCode::DecompressionLimit,
            "compression.decoded_bytes",
            expected,
            limits.max_decompressed_bytes,
        ));
    }
    let magic = encoded.get(..4).ok_or_else(|| {
        simple_error(
            Classification::Malformed,
            ErrorCode::UnknownCompression,
            "compression.magic",
        )
    })?;
    let decoded = match magic {
        b"SNAP" => {
            let declared = decompress_len(&encoded[4..]).map_err(|_| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::DecompressionFailed,
                    "compression.snappy.length",
                )
            })?;
            if declared != expected || declared > limits.max_decompressed_bytes {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::DecompressionLimit,
                    "compression.snappy.length",
                ));
            }
            Decoder::new().decompress_vec(&encoded[4..]).map_err(|_| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::DecompressionFailed,
                    "compression.snappy",
                )
            })?
        }
        b"LZSS" => decode_lzss(encoded, expected)?,
        _ => {
            return Err(simple_error(
                Classification::Unknown,
                ErrorCode::UnknownCompression,
                "compression.magic",
            ));
        }
    };
    if decoded.len() != expected {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::DecompressionFailed,
            "compression.decoded_bytes",
        ));
    }
    Ok(decoded)
}

fn decode_lzss(encoded: &[u8], expected: usize) -> Result<Vec<u8>, CodecError> {
    if encoded.len() < 8
        || u32::from_le_bytes(encoded[4..8].try_into().expect("fixed header")) as usize != expected
    {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::DecompressionFailed,
            "compression.lzss.header",
        ));
    }
    let mut input = 8;
    let mut output = Vec::with_capacity(expected);
    'commands: loop {
        let command = *encoded.get(input).ok_or_else(|| {
            simple_error(
                Classification::Malformed,
                ErrorCode::DecompressionFailed,
                "compression.lzss.command",
            )
        })?;
        input += 1;
        for bit in 0..8 {
            if command & (1 << bit) == 0 {
                let byte = *encoded.get(input).ok_or_else(|| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::DecompressionFailed,
                        "compression.lzss.literal",
                    )
                })?;
                input += 1;
                output.push(byte);
            } else {
                let first = *encoded.get(input).ok_or_else(|| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::DecompressionFailed,
                        "compression.lzss.reference",
                    )
                })?;
                let second = *encoded.get(input + 1).ok_or_else(|| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::DecompressionFailed,
                        "compression.lzss.reference",
                    )
                })?;
                input += 2;
                let distance = (usize::from(first) << 4) | (usize::from(second) >> 4);
                let count = usize::from(second & 0x0f) + 1;
                if count == 1 {
                    break 'commands;
                }
                let source = output.len().checked_sub(distance + 1).ok_or_else(|| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::DecompressionFailed,
                        "compression.lzss.distance",
                    )
                })?;
                for offset in 0..count {
                    let byte = *output.get(source + offset).ok_or_else(|| {
                        simple_error(
                            Classification::Malformed,
                            ErrorCode::DecompressionFailed,
                            "compression.lzss.reference",
                        )
                    })?;
                    output.push(byte);
                }
            }
            if output.len() > expected {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::DecompressionFailed,
                    "compression.lzss.output",
                ));
            }
        }
    }
    Ok(output)
}

fn validate_state(state: &CodecState, limits: Limits) -> Result<(), CodecError> {
    if state.string_tables.len() > limits.max_string_tables {
        return Err(limit_error(
            ErrorCode::StringTableLimit,
            "codec_state.string_tables",
            state.string_tables.len(),
            limits.max_string_tables,
        ));
    }
    if state.snapshots.len() > limits.max_snapshot_history {
        return Err(limit_error(
            ErrorCode::EntityLimit,
            "codec_state.snapshots",
            state.snapshots.len(),
            limits.max_snapshot_history,
        ));
    }
    Ok(())
}

fn require_zero_padding(reader: &mut BitReader<'_>, field: &'static str) -> Result<(), CodecError> {
    let remaining = reader.bits_left();
    if remaining > 7 || (remaining > 0 && reader.read_unsigned(remaining, field)? != 0) {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::TrailingBits,
            reader.position().saturating_sub(remaining)..reader.position(),
            None,
            field,
            Some(remaining as i64),
            None,
            Some(7),
        ));
    }
    Ok(())
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
