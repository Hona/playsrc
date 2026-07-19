use std::collections::BTreeMap;

use crate::{BitPayload, BitReader, Classification, CodecError, ErrorCode, Limits, failure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventFieldKind {
    String,
    Float,
    Integer32,
    Integer16,
    Byte,
    Boolean,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventField {
    pub name: Vec<u8>,
    pub kind: EventFieldKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventSchema {
    pub id: u16,
    pub name: Vec<u8>,
    pub fields: Vec<EventField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventValue {
    String(Vec<u8>),
    FloatBits(u32),
    Integer32(i32),
    Integer16(i16),
    Byte(u8),
    Boolean(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub id: u16,
    pub name: Vec<u8>,
    pub fields: Vec<(Vec<u8>, EventValue)>,
}

pub(crate) fn parse_event_schemas(
    count: usize,
    payload: &BitPayload,
    limits: Limits,
) -> Result<Vec<EventSchema>, CodecError> {
    if count > limits.max_event_schemas {
        return Err(limit_error(
            ErrorCode::EventLimit,
            "event_schema.count",
            count,
            limits.max_event_schemas,
        ));
    }
    let mut reader = payload.reader();
    let mut schemas = Vec::with_capacity(count);
    let mut ids = BTreeMap::new();
    for _ in 0..count {
        let id = reader.read_unsigned(9, "event_schema.id")? as u16;
        if ids.insert(id, ()).is_some() {
            return Err(simple_error(
                Classification::Malformed,
                ErrorCode::DuplicateEvent,
                "event_schema.id",
            ));
        }
        let name = reader.read_c_string(limits.max_string_bytes, "event_schema.name")?;
        let mut fields = Vec::new();
        loop {
            let raw_kind = reader.read_unsigned(3, "event_schema.field.kind")?;
            if raw_kind == 0 {
                break;
            }
            if fields.len() >= limits.max_fields_per_event {
                return Err(limit_error(
                    ErrorCode::EventLimit,
                    "event_schema.fields",
                    fields.len() + 1,
                    limits.max_fields_per_event,
                ));
            }
            let kind = match raw_kind {
                1 => EventFieldKind::String,
                2 => EventFieldKind::Float,
                3 => EventFieldKind::Integer32,
                4 => EventFieldKind::Integer16,
                5 => EventFieldKind::Byte,
                6 => EventFieldKind::Boolean,
                _ => {
                    return Err(simple_error(
                        Classification::Unknown,
                        ErrorCode::InvalidEventField,
                        "event_schema.field.kind",
                    ));
                }
            };
            fields.push(EventField {
                name: reader.read_c_string(limits.max_string_bytes, "event_schema.field.name")?,
                kind,
            });
        }
        schemas.push(EventSchema { id, name, fields });
    }
    require_zero_padding(&mut reader, "event_schema.padding")?;
    Ok(schemas)
}

pub(crate) fn parse_event(
    payload: &BitPayload,
    schemas: &BTreeMap<u16, EventSchema>,
    limits: Limits,
) -> Result<Event, CodecError> {
    let mut reader = payload.reader();
    let id = reader.read_unsigned(9, "event.id")? as u16;
    let schema = schemas.get(&id).ok_or_else(|| {
        simple_error(Classification::Missing, ErrorCode::MissingEvent, "event.id")
    })?;
    let mut fields = Vec::with_capacity(schema.fields.len());
    for field in &schema.fields {
        let value = match field.kind {
            EventFieldKind::String => EventValue::String(
                reader.read_c_string(limits.max_string_bytes, "event.field.string")?,
            ),
            EventFieldKind::Float => EventValue::FloatBits(reader.read_u32("event.field.float")?),
            EventFieldKind::Integer32 => {
                EventValue::Integer32(reader.read_i32("event.field.integer32")?)
            }
            EventFieldKind::Integer16 => {
                EventValue::Integer16(reader.read_i16("event.field.integer16")?)
            }
            EventFieldKind::Byte => EventValue::Byte(reader.read_u8("event.field.byte")?),
            EventFieldKind::Boolean => EventValue::Boolean(reader.read_bit("event.field.boolean")?),
        };
        fields.push((field.name.clone(), value));
    }
    require_zero_padding(&mut reader, "event.padding")?;
    Ok(Event {
        id,
        name: schema.name.clone(),
        fields,
    })
}

fn require_zero_padding(reader: &mut BitReader<'_>, field: &'static str) -> Result<(), CodecError> {
    if reader.bits_left() > 7
        || (reader.bits_left() > 0 && reader.read_unsigned(reader.bits_left(), field)? != 0)
    {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::TrailingBits,
            field,
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
