//! Transport-independent Source 1 protocol-24 recorded-state codecs.
//!
//! Games bind class, field, event, and user-message meaning. This crate owns
//! protocol framing, schema declarations, quantized values, and canonical
//! full/delta state reduction; it never advances gameplay.

mod bit;
mod codec;
mod entity;
mod event;
mod message;
mod schema;
mod string_table;

pub use bit::{BitPayload, BitReader, BitSpan};
pub use codec::{CodecState, DecodedPacket, NetworkOperation, RecordedStateCodec};
pub use entity::{Entity, EntityChange, EntitySnapshot, FieldValue, PacketEntities};
pub use event::{Event, EventField, EventFieldKind, EventSchema, EventValue};
pub use message::{Message, MessageBody, MessageIdentity, UserMessage};
pub use schema::{
    Class, FLAG_CHANGES_OFTEN, FLAG_COLLAPSIBLE, FLAG_COORDINATE, FLAG_COORDINATE_MP,
    FLAG_COORDINATE_MP_INTEGRAL, FLAG_COORDINATE_MP_LOW, FLAG_EXCLUDE, FLAG_INSIDE_ARRAY,
    FLAG_NO_SCALE, FLAG_NORMAL_OR_VARINT, FLAG_PROXY_ALWAYS, FLAG_ROUND_DOWN, FLAG_ROUND_UP,
    FLAG_SHARED_EXPONENT, FLAG_UNSIGNED, FLAG_VECTOR_ELEMENT, FlatProperty, Property, PropertyKind,
    SchemaRegistry, SendTable, parse_demo_data_tables,
};
pub use string_table::{StringEntry, StringTable};

use std::{fmt, ops::Range};

pub const PROTOCOL: i32 = 24;
pub const MESSAGE_TYPE_BITS: usize = 6;
pub const MAX_ENTITY_INDEX: usize = 2_048;
pub const MAX_CLASS_COUNT: usize = 512;
pub const MAX_FLAT_PROPERTIES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_payload_bits: usize,
    pub max_messages_per_payload: usize,
    pub max_string_bytes: usize,
    pub max_data_tables: usize,
    pub max_properties_per_table: usize,
    pub max_flat_properties: usize,
    pub max_classes: usize,
    pub max_string_tables: usize,
    pub max_entries_per_string_table: usize,
    pub max_string_user_data_bytes: usize,
    pub max_event_schemas: usize,
    pub max_fields_per_event: usize,
    pub max_entities: usize,
    pub max_fields_per_entity: usize,
    pub max_snapshot_history: usize,
    pub max_decompressed_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Malformed,
    Unsupported,
    Unknown,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    BitLimit,
    Truncated,
    InvalidVarint,
    UnterminatedString,
    StringLimit,
    MessageLimit,
    UnknownMessage,
    InvalidLength,
    PayloadLimit,
    TrailingBits,
    TableLimit,
    PropertyLimit,
    InvalidPropertyType,
    InvalidPropertyFlags,
    DuplicateTable,
    MissingTable,
    CyclicTable,
    InvalidArray,
    FlatPropertyLimit,
    ClassLimit,
    DuplicateClass,
    InvalidClass,
    StringTableLimit,
    DuplicateStringTable,
    MissingStringTable,
    InvalidStringIndex,
    InvalidStringHistory,
    UserDataLimit,
    UnknownCompression,
    DecompressionFailed,
    DecompressionLimit,
    EventLimit,
    DuplicateEvent,
    MissingEvent,
    InvalidEventField,
    EntityLimit,
    EntityFieldLimit,
    MissingSnapshotBase,
    MissingClassBaseline,
    MissingEntity,
    EntityIdentityMismatch,
    InvalidEntityHeader,
    InvalidBaseline,
    TickRequired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodecError {
    pub classification: Classification,
    pub code: ErrorCode,
    pub bit_range: Range<usize>,
    pub message_ordinal: Option<usize>,
    pub field: &'static str,
    pub declared: Option<i64>,
    pub available: Option<u64>,
    pub limit: Option<u64>,
}

impl fmt::Display for CodecError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?} {:?} at bits {}..{} message {:?} ({})",
            self.classification,
            self.code,
            self.bit_range.start,
            self.bit_range.end,
            self.message_ordinal,
            self.field
        )
    }
}

impl std::error::Error for CodecError {}

#[allow(clippy::too_many_arguments)]
pub(crate) fn failure(
    classification: Classification,
    code: ErrorCode,
    bit_range: Range<usize>,
    message_ordinal: Option<usize>,
    field: &'static str,
    declared: Option<i64>,
    available: Option<u64>,
    limit: Option<u64>,
) -> CodecError {
    CodecError {
        classification,
        code,
        bit_range,
        message_ordinal,
        field,
        declared,
        available,
        limit,
    }
}
