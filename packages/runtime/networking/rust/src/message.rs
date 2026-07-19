use std::ops::Range;

use crate::{
    BitPayload, BitReader, Classification, CodecError, ErrorCode, Limits, MESSAGE_TYPE_BITS,
    entity::PacketEntities,
    failure,
    schema::{Class, SendTable, parse_send_table},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageIdentity {
    Padding,
    Disconnect,
    File,
    Tick,
    StringCommand,
    ConVarSet,
    SignonState,
    Print,
    ServerInfo,
    SendTable,
    ClassInfo,
    Pause,
    StringTableCreate,
    StringTableUpdate,
    VoiceInit,
    VoiceData,
    Sounds,
    SetView,
    FixAngle,
    CrosshairAngle,
    BspDecal,
    UserMessage,
    EntityMessage,
    GameEvent,
    PacketEntities,
    TempEntities,
    Prefetch,
    Menu,
    GameEventList,
    GetCvar,
    CommandKeyValues,
    PauseTimed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserMessage {
    pub message_type: u8,
    pub payload: BitPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateStringTable {
    pub name: Vec<u8>,
    pub is_filenames: bool,
    pub max_entries: u16,
    pub entry_count: u16,
    pub user_data_fixed_size: bool,
    pub user_data_size_bytes: u16,
    pub user_data_size_bits: u8,
    pub compressed: bool,
    pub payload: BitPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateStringTable {
    pub table_id: u8,
    pub changed_entries: u16,
    pub payload: BitPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessageBody {
    Padding,
    Tick {
        server_tick: i32,
        host_frame_time_units: u16,
        host_frame_time_std_deviation_units: u16,
    },
    SignonState {
        state: u8,
        spawn_count: i32,
    },
    SendTable(SendTable),
    ClassInfo {
        class_count: u16,
        classes: Option<Vec<Class>>,
    },
    CreateStringTable(CreateStringTable),
    UpdateStringTable(UpdateStringTable),
    UserMessage(UserMessage),
    GameEvent(BitPayload),
    PacketEntities(PacketEntities),
    GameEventList {
        event_count: u16,
        payload: BitPayload,
    },
    Opaque(BitPayload),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub ordinal: usize,
    pub encoded_id: u8,
    pub identity: MessageIdentity,
    pub bit_range: Range<usize>,
    pub body: MessageBody,
}

pub(crate) fn decode_message(
    reader: &mut BitReader<'_>,
    ordinal: usize,
    limits: Limits,
) -> Result<Message, CodecError> {
    let start = reader.position();
    let id = reader.read_unsigned(MESSAGE_TYPE_BITS, "message.id")? as u8;
    let body_start = reader.position();
    let (identity, body) = match id {
        0 => (MessageIdentity::Padding, MessageBody::Padding),
        1 => opaque(reader, body_start, MessageIdentity::Disconnect, |reader| {
            reader.read_c_string(limits.max_string_bytes, "disconnect.reason")?;
            Ok(())
        })?,
        2 => opaque(reader, body_start, MessageIdentity::File, |reader| {
            reader.read_u32("file.transfer_id")?;
            reader.read_c_string(limits.max_string_bytes, "file.name")?;
            reader.read_bit("file.requested")?;
            Ok(())
        })?,
        3 => (
            MessageIdentity::Tick,
            MessageBody::Tick {
                server_tick: reader.read_i32("tick.server_tick")?,
                host_frame_time_units: reader.read_u16("tick.host_frame_time")?,
                host_frame_time_std_deviation_units: reader
                    .read_u16("tick.host_frame_time_std_deviation")?,
            },
        ),
        4 => opaque(
            reader,
            body_start,
            MessageIdentity::StringCommand,
            |reader| {
                reader.read_c_string(limits.max_string_bytes, "string_command.value")?;
                Ok(())
            },
        )?,
        5 => opaque(reader, body_start, MessageIdentity::ConVarSet, |reader| {
            let count = reader.read_u8("convar.count")? as usize;
            for _ in 0..count {
                reader.read_c_string(limits.max_string_bytes, "convar.name")?;
                reader.read_c_string(limits.max_string_bytes, "convar.value")?;
            }
            Ok(())
        })?,
        6 => (
            MessageIdentity::SignonState,
            MessageBody::SignonState {
                state: reader.read_u8("signon.state")?,
                spawn_count: reader.read_i32("signon.spawn_count")?,
            },
        ),
        7 => opaque(reader, body_start, MessageIdentity::Print, |reader| {
            reader.read_c_string(limits.max_string_bytes, "print.text")?;
            Ok(())
        })?,
        8 => opaque(reader, body_start, MessageIdentity::ServerInfo, |reader| {
            reader.read_i16("server_info.protocol")?;
            reader.read_i32("server_info.server_count")?;
            reader.read_bit("server_info.hltv")?;
            reader.read_bit("server_info.dedicated")?;
            reader.read_u32("server_info.legacy_crc")?;
            reader.read_u16("server_info.max_classes")?;
            reader.skip(128, "server_info.map_md5")?;
            reader.read_u8("server_info.player_slot")?;
            reader.read_u8("server_info.max_clients")?;
            reader.read_u32("server_info.tick_interval")?;
            reader.read_u8("server_info.os")?;
            for field in [
                "server_info.game_directory",
                "server_info.map",
                "server_info.sky",
                "server_info.host",
            ] {
                reader.read_c_string(limits.max_string_bytes, field)?;
            }
            reader.read_bit("server_info.replay")?;
            Ok(())
        })?,
        9 => {
            let needs_decoder = reader.read_bit("send_table.needs_decoder")?;
            let length = reader.read_i16("send_table.bit_length")?;
            let length = usize::try_from(length).map_err(|_| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidLength,
                    ordinal,
                    "send_table.bit_length",
                )
            })?;
            let payload = reader.read_payload(length, "send_table.payload")?;
            let mut table_reader = payload.reader().with_message_ordinal(ordinal);
            let table = parse_send_table(&mut table_reader, needs_decoder, limits)?;
            if table_reader.bits_left() != 0 {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::TrailingBits,
                    ordinal,
                    "send_table.payload",
                ));
            }
            (MessageIdentity::SendTable, MessageBody::SendTable(table))
        }
        10 => {
            let count = reader.read_i16("class_info.count")?;
            let count = usize::try_from(count).map_err(|_| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::ClassLimit,
                    ordinal,
                    "class_info.count",
                )
            })?;
            if count == 0 || count > limits.max_classes {
                return Err(limit_error(
                    ErrorCode::ClassLimit,
                    ordinal,
                    "class_info.count",
                    count,
                    limits.max_classes,
                ));
            }
            let class_bits = bit_width(count);
            if reader.read_bit("class_info.create_on_client")? {
                (
                    MessageIdentity::ClassInfo,
                    MessageBody::ClassInfo {
                        class_count: count as u16,
                        classes: None,
                    },
                )
            } else {
                let mut classes = Vec::with_capacity(count);
                for _ in 0..count {
                    classes.push(Class {
                        id: reader.read_unsigned(class_bits, "class_info.class.id")? as u16,
                        name: reader
                            .read_c_string(limits.max_string_bytes, "class_info.class.name")?,
                        table_name: reader
                            .read_c_string(limits.max_string_bytes, "class_info.class.table")?,
                    });
                }
                (
                    MessageIdentity::ClassInfo,
                    MessageBody::ClassInfo {
                        class_count: count as u16,
                        classes: Some(classes),
                    },
                )
            }
        }
        11 => opaque(reader, body_start, MessageIdentity::Pause, |reader| {
            reader.read_bit("pause.value")?;
            Ok(())
        })?,
        12 => {
            let before_name = reader.position();
            let prefix = reader.read_u8("string_table_create.name_prefix")?;
            reader.seek(before_name)?;
            let is_filenames = prefix == b':';
            if is_filenames {
                reader.read_u8("string_table_create.filename_marker")?;
            }
            let name = reader.read_c_string(limits.max_string_bytes, "string_table_create.name")?;
            let max_entries = reader.read_u16("string_table_create.max_entries")?;
            if max_entries == 0 {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidStringIndex,
                    ordinal,
                    "string_table_create.max_entries",
                ));
            }
            let entry_count = reader.read_unsigned(
                bit_width(usize::from(max_entries)),
                "string_table_create.entry_count",
            )? as u16;
            let bit_length = reader.read_var_u32("string_table_create.bit_length")? as usize;
            let fixed = reader.read_bit("string_table_create.user_data_fixed")?;
            let (user_data_size_bytes, user_data_size_bits) = if fixed {
                (
                    reader.read_unsigned(12, "string_table_create.user_data_bytes")? as u16,
                    reader.read_unsigned(4, "string_table_create.user_data_bits")? as u8,
                )
            } else {
                (0, 0)
            };
            let compressed = reader.read_bit("string_table_create.compressed")?;
            let payload = reader.read_payload(bit_length, "string_table_create.payload")?;
            (
                MessageIdentity::StringTableCreate,
                MessageBody::CreateStringTable(CreateStringTable {
                    name,
                    is_filenames,
                    max_entries,
                    entry_count,
                    user_data_fixed_size: fixed,
                    user_data_size_bytes,
                    user_data_size_bits,
                    compressed,
                    payload,
                }),
            )
        }
        13 => {
            let table_id = reader.read_unsigned(5, "string_table_update.table_id")? as u8;
            let changed_entries = if reader.read_bit("string_table_update.has_count")? {
                reader.read_u16("string_table_update.changed_entries")?
            } else {
                1
            };
            let bit_length = reader.read_unsigned(20, "string_table_update.bit_length")? as usize;
            let payload = reader.read_payload(bit_length, "string_table_update.payload")?;
            (
                MessageIdentity::StringTableUpdate,
                MessageBody::UpdateStringTable(UpdateStringTable {
                    table_id,
                    changed_entries,
                    payload,
                }),
            )
        }
        14 => opaque(reader, body_start, MessageIdentity::VoiceInit, |reader| {
            reader.read_c_string(limits.max_string_bytes, "voice_init.codec")?;
            if reader.read_u8("voice_init.version")? == 255 {
                reader.read_i16("voice_init.sample_rate")?;
            }
            Ok(())
        })?,
        15 => opaque(reader, body_start, MessageIdentity::VoiceData, |reader| {
            reader.read_u8("voice_data.client")?;
            reader.read_u8("voice_data.proximity")?;
            let length = reader.read_u16("voice_data.bit_length")? as usize;
            reader.skip(length, "voice_data.payload")?;
            Ok(())
        })?,
        16 | 22 => {
            return Err(simple_error(
                Classification::Unknown,
                ErrorCode::UnknownMessage,
                ordinal,
                "message.id",
            ));
        }
        17 => opaque(reader, body_start, MessageIdentity::Sounds, |reader| {
            let reliable = reader.read_bit("sounds.reliable")?;
            let length = if reliable {
                reader.read_unsigned(8, "sounds.bit_length")? as usize
            } else {
                reader.read_u8("sounds.count")?;
                reader.read_u16("sounds.bit_length")? as usize
            };
            reader.skip(length, "sounds.payload")?;
            Ok(())
        })?,
        18 => opaque(reader, body_start, MessageIdentity::SetView, |reader| {
            reader.read_unsigned(11, "set_view.entity")?;
            Ok(())
        })?,
        19 => opaque(reader, body_start, MessageIdentity::FixAngle, |reader| {
            reader.read_bit("fix_angle.relative")?;
            reader.skip(48, "fix_angle.angles")?;
            Ok(())
        })?,
        20 => opaque(
            reader,
            body_start,
            MessageIdentity::CrosshairAngle,
            |reader| {
                reader.skip(48, "crosshair_angle.angles")?;
                Ok(())
            },
        )?,
        21 => opaque(reader, body_start, MessageIdentity::BspDecal, |reader| {
            skip_vector_coordinate(reader)?;
            reader.read_unsigned(9, "bsp_decal.texture")?;
            if reader.read_bit("bsp_decal.has_entity")? {
                reader.read_unsigned(11, "bsp_decal.entity")?;
                reader.read_unsigned(13, "bsp_decal.model")?;
            }
            reader.read_bit("bsp_decal.low_priority")?;
            Ok(())
        })?,
        23 => {
            let message_type = reader.read_u8("user_message.type")?;
            let length = reader.read_unsigned(11, "user_message.bit_length")? as usize;
            let payload = reader.read_payload(length, "user_message.payload")?;
            (
                MessageIdentity::UserMessage,
                MessageBody::UserMessage(UserMessage {
                    message_type,
                    payload,
                }),
            )
        }
        24 => opaque(
            reader,
            body_start,
            MessageIdentity::EntityMessage,
            |reader| {
                reader.read_unsigned(11, "entity_message.entity")?;
                reader.read_unsigned(9, "entity_message.class")?;
                let length = reader.read_unsigned(11, "entity_message.bit_length")? as usize;
                reader.skip(length, "entity_message.payload")?;
                Ok(())
            },
        )?,
        25 => {
            let length = reader.read_unsigned(11, "game_event.bit_length")? as usize;
            let payload = reader.read_payload(length, "game_event.payload")?;
            (MessageIdentity::GameEvent, MessageBody::GameEvent(payload))
        }
        26 => {
            let max_entries = reader.read_unsigned(11, "packet_entities.max_entries")? as u16;
            let is_delta = reader.read_bit("packet_entities.is_delta")?;
            let delta_from = if is_delta {
                Some(reader.read_i32("packet_entities.delta_from")?)
            } else {
                None
            };
            let baseline_slot = reader.read_bit("packet_entities.baseline_slot")? as u8;
            let updated_entries =
                reader.read_unsigned(11, "packet_entities.updated_entries")? as u16;
            let length = reader.read_unsigned(20, "packet_entities.bit_length")? as usize;
            let update_baseline = reader.read_bit("packet_entities.update_baseline")?;
            let payload = reader.read_payload(length, "packet_entities.payload")?;
            (
                MessageIdentity::PacketEntities,
                MessageBody::PacketEntities(PacketEntities {
                    max_entries,
                    delta_from,
                    baseline_slot,
                    updated_entries,
                    update_baseline,
                    payload,
                }),
            )
        }
        27 => opaque(
            reader,
            body_start,
            MessageIdentity::TempEntities,
            |reader| {
                reader.read_u8("temp_entities.count")?;
                let length = reader.read_var_u32("temp_entities.bit_length")? as usize;
                reader.skip(length, "temp_entities.payload")?;
                Ok(())
            },
        )?,
        28 => opaque(reader, body_start, MessageIdentity::Prefetch, |reader| {
            reader.read_unsigned(14, "prefetch.sound_index")?;
            Ok(())
        })?,
        29 => opaque(reader, body_start, MessageIdentity::Menu, |reader| {
            reader.read_i16("menu.type")?;
            let bytes = reader.read_u16("menu.byte_length")? as usize;
            reader.skip(bytes * 8, "menu.payload")?;
            Ok(())
        })?,
        30 => {
            let event_count = reader.read_unsigned(9, "game_event_list.count")? as u16;
            let length = reader.read_unsigned(20, "game_event_list.bit_length")? as usize;
            let payload = reader.read_payload(length, "game_event_list.payload")?;
            (
                MessageIdentity::GameEventList,
                MessageBody::GameEventList {
                    event_count,
                    payload,
                },
            )
        }
        31 => opaque(reader, body_start, MessageIdentity::GetCvar, |reader| {
            reader.read_i32("get_cvar.cookie")?;
            reader.read_c_string(limits.max_string_bytes, "get_cvar.name")?;
            Ok(())
        })?,
        32 => opaque(
            reader,
            body_start,
            MessageIdentity::CommandKeyValues,
            |reader| {
                let bytes = reader.read_i32("command_keyvalues.byte_length")?;
                let bytes = usize::try_from(bytes).map_err(|_| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::InvalidLength,
                        ordinal,
                        "command_keyvalues.byte_length",
                    )
                })?;
                reader.skip(bytes * 8, "command_keyvalues.payload")?;
                Ok(())
            },
        )?,
        33 => opaque(reader, body_start, MessageIdentity::PauseTimed, |reader| {
            reader.read_bit("pause_timed.value")?;
            reader.read_u32("pause_timed.expire_time")?;
            Ok(())
        })?,
        _ => {
            return Err(simple_error(
                Classification::Unknown,
                ErrorCode::UnknownMessage,
                ordinal,
                "message.id",
            ));
        }
    };
    Ok(Message {
        ordinal,
        encoded_id: id,
        identity,
        bit_range: start..reader.position(),
        body,
    })
}

fn opaque<F>(
    reader: &mut BitReader<'_>,
    body_start: usize,
    identity: MessageIdentity,
    parse: F,
) -> Result<(MessageIdentity, MessageBody), CodecError>
where
    F: FnOnce(&mut BitReader<'_>) -> Result<(), CodecError>,
{
    parse(reader)?;
    Ok((
        identity,
        MessageBody::Opaque(reader.copy_span(crate::BitSpan {
            start: body_start,
            length: reader.position() - body_start,
        })),
    ))
}

fn skip_vector_coordinate(reader: &mut BitReader<'_>) -> Result<(), CodecError> {
    let x = reader.read_bit("vector_coordinate.has_x")?;
    let y = reader.read_bit("vector_coordinate.has_y")?;
    let z = reader.read_bit("vector_coordinate.has_z")?;
    for present in [x, y, z] {
        if !present {
            continue;
        }
        let integer = reader.read_bit("vector_coordinate.has_integer")?;
        let fraction = reader.read_bit("vector_coordinate.has_fraction")?;
        if integer || fraction {
            reader.read_bit("vector_coordinate.sign")?;
            if integer {
                reader.skip(14, "vector_coordinate.integer")?;
            }
            if fraction {
                reader.skip(5, "vector_coordinate.fraction")?;
            }
        }
    }
    Ok(())
}

fn bit_width(maximum: usize) -> usize {
    usize::BITS as usize - maximum.leading_zeros() as usize
}

fn simple_error(
    classification: Classification,
    code: ErrorCode,
    ordinal: usize,
    field: &'static str,
) -> CodecError {
    failure(
        classification,
        code,
        0..0,
        Some(ordinal),
        field,
        None,
        None,
        None,
    )
}

fn limit_error(
    code: ErrorCode,
    ordinal: usize,
    field: &'static str,
    actual: usize,
    limit: usize,
) -> CodecError {
    failure(
        Classification::Malformed,
        code,
        0..0,
        Some(ordinal),
        field,
        Some(actual as i64),
        None,
        Some(limit as u64),
    )
}
