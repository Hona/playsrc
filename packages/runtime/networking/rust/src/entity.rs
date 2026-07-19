use std::collections::BTreeMap;

use crate::{
    BitPayload, BitReader, Classification, CodecError, ErrorCode, Limits, MAX_ENTITY_INDEX,
    failure,
    schema::{
        FLAG_COORDINATE, FLAG_COORDINATE_MP, FLAG_COORDINATE_MP_INTEGRAL, FLAG_COORDINATE_MP_LOW,
        FLAG_NO_SCALE, FLAG_NORMAL_OR_VARINT, FLAG_UNSIGNED, FlatProperty, Property, PropertyKind,
        SchemaRegistry,
    },
    string_table::StringTable,
};

const INSTANCE_BASELINE: &[u8] = b"instancebaseline";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldValue {
    SignedInteger(i32),
    UnsignedInteger(u32),
    FloatBits(u32),
    Vector3([u32; 3]),
    Vector2([u32; 2]),
    String(Vec<u8>),
    Array(Vec<FieldValue>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entity {
    pub index: u16,
    pub serial: u16,
    pub class_id: u16,
    pub fields: BTreeMap<u16, FieldValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityChange {
    Create(Entity),
    Update {
        index: u16,
        fields: BTreeMap<u16, FieldValue>,
    },
    Preserve {
        index: u16,
    },
    LeaveRelevance {
        index: u16,
    },
    Delete {
        index: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntitySnapshot {
    pub server_tick: i32,
    pub entities: BTreeMap<u16, Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketEntities {
    pub max_entries: u16,
    pub delta_from: Option<i32>,
    pub baseline_slot: u8,
    pub updated_entries: u16,
    pub update_baseline: bool,
    pub payload: BitPayload,
}

pub(crate) fn refresh_class_baselines(
    schema: &SchemaRegistry,
    string_tables: &BTreeMap<u8, StringTable>,
    limits: Limits,
) -> Result<BTreeMap<u16, BTreeMap<u16, FieldValue>>, CodecError> {
    let table = string_tables
        .values()
        .find(|table| table.name == INSTANCE_BASELINE)
        .ok_or_else(|| {
            simple_error(
                Classification::Missing,
                ErrorCode::MissingClassBaseline,
                "class_baseline.table",
            )
        })?;
    let mut output = BTreeMap::new();
    for entry in &table.entries {
        let Ok(key) = std::str::from_utf8(&entry.value) else {
            continue;
        };
        let Ok(class_id) = key.parse::<u16>() else {
            continue;
        };
        if !schema.classes.contains_key(&class_id) {
            continue;
        }
        let data = entry.user_data.as_ref().ok_or_else(|| {
            simple_error(
                Classification::Missing,
                ErrorCode::MissingClassBaseline,
                "class_baseline.user_data",
            )
        })?;
        let properties = schema.flat_properties(class_id).ok_or_else(|| {
            simple_error(
                Classification::Missing,
                ErrorCode::InvalidClass,
                "class_baseline.properties",
            )
        })?;
        let mut reader = BitReader::new(data, data.len() * 8)?;
        let fields = decode_field_delta(&mut reader, properties, limits)?;
        require_byte_padding(&reader, "class_baseline.padding")?;
        output.insert(class_id, fields);
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reduce_packet_entities(
    message: &PacketEntities,
    server_tick: i32,
    schema: &SchemaRegistry,
    class_baselines: &BTreeMap<u16, BTreeMap<u16, FieldValue>>,
    rolling_baselines: &mut [BTreeMap<u16, Entity>; 2],
    snapshots: &BTreeMap<i32, std::sync::Arc<EntitySnapshot>>,
    limits: Limits,
) -> Result<(EntitySnapshot, Vec<EntityChange>), CodecError> {
    let empty_base = EntitySnapshot {
        server_tick,
        entities: BTreeMap::new(),
    };
    let base = match message.delta_from {
        Some(tick) => snapshots.get(&tick).ok_or_else(|| {
            simple_error(
                Classification::Missing,
                ErrorCode::MissingSnapshotBase,
                "packet_entities.delta_from",
            )
        })?,
        None => &empty_base,
    };
    if usize::from(message.max_entries) > limits.max_entities.min(MAX_ENTITY_INDEX) {
        return Err(limit_error(
            ErrorCode::EntityLimit,
            "packet_entities.max_entries",
            usize::from(message.max_entries),
            limits.max_entities.min(MAX_ENTITY_INDEX),
        ));
    }
    if message.baseline_slot > 1 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidBaseline,
            "packet_entities.baseline_slot",
        ));
    }
    let selected_baseline = usize::from(message.baseline_slot);
    let update_slot = 1 - selected_baseline;
    if message.update_baseline {
        rolling_baselines[update_slot] = rolling_baselines[selected_baseline].clone();
    }

    let mut reader = message.payload.reader();
    let mut next_entities = BTreeMap::new();
    let mut changes = Vec::new();
    let mut old = base.entities.iter().peekable();
    let mut header_base = -1_i32;

    for _ in 0..message.updated_entries {
        let delta = reader.read_ubit_var("packet_entities.entity_delta")?;
        let index_i32 = header_base
            .checked_add(1)
            .and_then(|value| value.checked_add(delta as i32))
            .ok_or_else(|| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidEntityHeader,
                    "packet_entities.entity_delta",
                )
            })?;
        let index = u16::try_from(index_i32).map_err(|_| {
            simple_error(
                Classification::Malformed,
                ErrorCode::InvalidEntityHeader,
                "packet_entities.entity_index",
            )
        })?;
        if usize::from(index) >= usize::from(message.max_entries)
            || usize::from(index) >= limits.max_entities.min(MAX_ENTITY_INDEX)
        {
            return Err(limit_error(
                ErrorCode::EntityLimit,
                "packet_entities.entity_index",
                usize::from(index),
                usize::from(message.max_entries).saturating_sub(1),
            ));
        }
        header_base = index_i32;

        while old.peek().is_some_and(|entry| *entry.0 < index) {
            let (old_index, entity) = old.next().expect("peeked entity");
            let old_index = *old_index;
            let entity = entity.clone();
            next_entities.insert(old_index, entity);
            changes.push(EntityChange::Preserve { index: old_index });
        }

        let leave = reader.read_bit("packet_entities.leave")?;
        if leave {
            if message.delta_from.is_none() {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidEntityHeader,
                    "packet_entities.full_leave",
                ));
            }
            let delete = reader.read_bit("packet_entities.delete")?;
            let old_index = *old
                .peek()
                .ok_or_else(|| {
                    simple_error(
                        Classification::Missing,
                        ErrorCode::MissingEntity,
                        "packet_entities.leave_entity",
                    )
                })?
                .0;
            if old_index != index {
                return Err(simple_error(
                    Classification::Missing,
                    ErrorCode::MissingEntity,
                    "packet_entities.leave_entity",
                ));
            }
            old.next();
            changes.push(if delete {
                EntityChange::Delete { index }
            } else {
                EntityChange::LeaveRelevance { index }
            });
            continue;
        }

        let enter = reader.read_bit("packet_entities.enter")?;
        if enter {
            let class_id = reader.read_unsigned(
                usize::from(schema.class_id_bits),
                "packet_entities.class_id",
            )? as u16;
            let serial = reader.read_unsigned(10, "packet_entities.serial")? as u16;
            let properties = schema.flat_properties(class_id).ok_or_else(|| {
                simple_error(
                    Classification::Missing,
                    ErrorCode::InvalidClass,
                    "packet_entities.class_id",
                )
            })?;
            let mut fields = rolling_baselines[selected_baseline]
                .get(&index)
                .filter(|entity| entity.class_id == class_id)
                .map(|entity| entity.fields.clone())
                .or_else(|| class_baselines.get(&class_id).cloned())
                .ok_or_else(|| {
                    simple_error(
                        Classification::Missing,
                        ErrorCode::MissingClassBaseline,
                        "packet_entities.class_baseline",
                    )
                })?;
            let delta = decode_field_delta(&mut reader, properties, limits)?;
            apply_fields(&mut fields, &delta, limits)?;
            let entity = Entity {
                index,
                serial,
                class_id,
                fields,
            };
            if old.peek().is_some_and(|entry| *entry.0 == index) {
                old.next();
            }
            if message.update_baseline {
                rolling_baselines[update_slot].insert(index, entity.clone());
            }
            next_entities.insert(index, entity.clone());
            changes.push(EntityChange::Create(entity));
            continue;
        }

        let (old_index, mut entity, class_id) = old
            .peek()
            .map(|entry| (*entry.0, entry.1.clone(), entry.1.class_id))
            .ok_or_else(|| {
                simple_error(
                    Classification::Missing,
                    ErrorCode::MissingEntity,
                    "packet_entities.update_entity",
                )
            })?;
        if old_index != index {
            return Err(simple_error(
                Classification::Missing,
                ErrorCode::MissingEntity,
                "packet_entities.update_entity",
            ));
        }
        let properties = schema.flat_properties(class_id).ok_or_else(|| {
            simple_error(
                Classification::Missing,
                ErrorCode::InvalidClass,
                "packet_entities.update_class",
            )
        })?;
        let delta = decode_field_delta(&mut reader, properties, limits)?;
        apply_fields(&mut entity.fields, &delta, limits)?;
        next_entities.insert(index, entity);
        changes.push(EntityChange::Update {
            index,
            fields: delta,
        });
        old.next();
    }

    for (&index, entity) in old {
        next_entities.insert(index, entity.clone());
        changes.push(EntityChange::Preserve { index });
    }

    if message.delta_from.is_some() {
        while reader.read_bit("packet_entities.has_explicit_delete")? {
            let index = reader.read_unsigned(11, "packet_entities.explicit_delete")? as u16;
            next_entities.remove(&index);
            changes.push(EntityChange::Delete { index });
        }
    }
    require_zero_padding(&mut reader, "packet_entities.padding")?;
    if next_entities.len() > limits.max_entities.min(MAX_ENTITY_INDEX) {
        return Err(limit_error(
            ErrorCode::EntityLimit,
            "packet_entities.entities",
            next_entities.len(),
            limits.max_entities.min(MAX_ENTITY_INDEX),
        ));
    }
    Ok((
        EntitySnapshot {
            server_tick,
            entities: next_entities,
        },
        changes,
    ))
}

fn decode_field_delta(
    reader: &mut BitReader<'_>,
    properties: &[FlatProperty],
    limits: Limits,
) -> Result<BTreeMap<u16, FieldValue>, CodecError> {
    let mut fields = BTreeMap::new();
    let mut last = -1_i32;
    loop {
        if !reader.read_bit("entity_fields.has_field")? {
            break;
        }
        let delta = reader.read_ubit_var("entity_fields.field_delta")?;
        let index = last
            .checked_add(1)
            .and_then(|value| value.checked_add(delta as i32))
            .ok_or_else(|| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::EntityFieldLimit,
                    "entity_fields.field_delta",
                )
            })?;
        let property = properties.get(index as usize).ok_or_else(|| {
            simple_error(
                Classification::Malformed,
                ErrorCode::EntityFieldLimit,
                "entity_fields.field_index",
            )
        })?;
        if fields.len() >= limits.max_fields_per_entity {
            return Err(limit_error(
                ErrorCode::EntityFieldLimit,
                "entity_fields.count",
                fields.len() + 1,
                limits.max_fields_per_entity,
            ));
        }
        fields.insert(index as u16, decode_property(reader, property, limits)?);
        last = index;
    }
    Ok(fields)
}

fn decode_property(
    reader: &mut BitReader<'_>,
    property: &FlatProperty,
    limits: Limits,
) -> Result<FieldValue, CodecError> {
    match property.property.kind {
        PropertyKind::Integer => decode_integer(reader, &property.property),
        PropertyKind::Float => Ok(FieldValue::FloatBits(
            decode_float(reader, &property.property)?.to_bits(),
        )),
        PropertyKind::Vector3 => {
            let x = decode_float(reader, &property.property)?;
            let y = decode_float(reader, &property.property)?;
            let z = if property.property.flags & FLAG_NORMAL_OR_VARINT != 0 {
                let negative = reader.read_bit("entity_field.vector_normal_z_sign")?;
                let square = x * x + y * y;
                let value = if square < 1.0 {
                    (1.0 - square).sqrt()
                } else {
                    0.0
                };
                if negative { -value } else { value }
            } else {
                decode_float(reader, &property.property)?
            };
            Ok(FieldValue::Vector3([x.to_bits(), y.to_bits(), z.to_bits()]))
        }
        PropertyKind::Vector2 => Ok(FieldValue::Vector2([
            decode_float(reader, &property.property)?.to_bits(),
            decode_float(reader, &property.property)?.to_bits(),
        ])),
        PropertyKind::String => {
            let length = reader.read_unsigned(9, "entity_field.string_length")? as usize;
            if length > limits.max_string_bytes {
                return Err(limit_error(
                    ErrorCode::StringLimit,
                    "entity_field.string_length",
                    length,
                    limits.max_string_bytes,
                ));
            }
            Ok(FieldValue::String(
                reader
                    .read_payload(length * 8, "entity_field.string")?
                    .bytes,
            ))
        }
        PropertyKind::Array => {
            let count_limit = usize::from(property.property.array_elements.ok_or_else(|| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidArray,
                    "entity_field.array_count",
                )
            })?);
            let width = bit_width(count_limit);
            let count = reader.read_unsigned(width, "entity_field.array_count")? as usize;
            if count > count_limit {
                return Err(limit_error(
                    ErrorCode::InvalidArray,
                    "entity_field.array_count",
                    count,
                    count_limit,
                ));
            }
            let element = property.array_element.as_ref().ok_or_else(|| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidArray,
                    "entity_field.array_element",
                )
            })?;
            let synthetic = FlatProperty {
                source_table: property.source_table.clone(),
                property: element.clone(),
                array_element: None,
            };
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(decode_property(reader, &synthetic, limits)?);
            }
            Ok(FieldValue::Array(values))
        }
        PropertyKind::Table => Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidPropertyType,
            "entity_field.table",
        )),
    }
}

fn decode_integer(
    reader: &mut BitReader<'_>,
    property: &Property,
) -> Result<FieldValue, CodecError> {
    let unsigned = property.flags & FLAG_UNSIGNED != 0;
    if property.flags & FLAG_NORMAL_OR_VARINT != 0 {
        let raw = reader.read_var_u32("entity_field.integer_varint")?;
        return Ok(if unsigned {
            FieldValue::UnsignedInteger(raw)
        } else {
            FieldValue::SignedInteger(((raw >> 1) as i32) ^ -((raw & 1) as i32))
        });
    }
    let width = usize::from(property.bit_count.ok_or_else(|| {
        simple_error(
            Classification::Malformed,
            ErrorCode::InvalidPropertyType,
            "entity_field.integer_bits",
        )
    })?);
    if width == 0 || width > 32 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidPropertyType,
            "entity_field.integer_bits",
        ));
    }
    Ok(if unsigned {
        FieldValue::UnsignedInteger(reader.read_unsigned(width, "entity_field.integer")?)
    } else {
        FieldValue::SignedInteger(reader.read_signed(width, "entity_field.integer")?)
    })
}

fn decode_float(reader: &mut BitReader<'_>, property: &Property) -> Result<f32, CodecError> {
    let flags = property.flags;
    if flags & FLAG_COORDINATE != 0 {
        return decode_coordinate(reader);
    }
    if flags & (FLAG_COORDINATE_MP | FLAG_COORDINATE_MP_LOW | FLAG_COORDINATE_MP_INTEGRAL) != 0 {
        return decode_multiplayer_coordinate(
            reader,
            flags & FLAG_COORDINATE_MP_INTEGRAL != 0,
            flags & FLAG_COORDINATE_MP_LOW != 0,
        );
    }
    if flags & FLAG_NO_SCALE != 0 {
        return Ok(f32::from_bits(reader.read_u32("entity_field.float_bits")?));
    }
    if flags & FLAG_NORMAL_OR_VARINT != 0 {
        let negative = reader.read_bit("entity_field.normal_sign")?;
        let fraction = reader.read_unsigned(11, "entity_field.normal_fraction")?;
        let value = fraction as f32 / 2_047.0;
        return Ok(if negative { -value } else { value });
    }
    let width = usize::from(property.bit_count.ok_or_else(|| {
        simple_error(
            Classification::Malformed,
            ErrorCode::InvalidPropertyType,
            "entity_field.float_bit_count",
        )
    })?);
    if width == 0 || width > 32 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidPropertyType,
            "entity_field.float_bit_count",
        ));
    }
    let raw = reader.read_unsigned(width, "entity_field.float")?;
    let denominator = if width == 32 {
        u32::MAX as f32
    } else {
        ((1_u32 << width) - 1) as f32
    };
    let low = f32::from_bits(property.low_value_bits.unwrap_or(0));
    let high = f32::from_bits(property.high_value_bits.unwrap_or(0));
    Ok(low + (high - low) * (raw as f32 / denominator))
}

fn decode_coordinate(reader: &mut BitReader<'_>) -> Result<f32, CodecError> {
    let has_integer = reader.read_bit("entity_field.coordinate.has_integer")?;
    let has_fraction = reader.read_bit("entity_field.coordinate.has_fraction")?;
    if !has_integer && !has_fraction {
        return Ok(0.0);
    }
    let negative = reader.read_bit("entity_field.coordinate.sign")?;
    let integer = if has_integer {
        reader.read_unsigned(14, "entity_field.coordinate.integer")? + 1
    } else {
        0
    };
    let fraction = if has_fraction {
        reader.read_unsigned(5, "entity_field.coordinate.fraction")?
    } else {
        0
    };
    let value = integer as f32 + fraction as f32 / 32.0;
    Ok(if negative { -value } else { value })
}

fn decode_multiplayer_coordinate(
    reader: &mut BitReader<'_>,
    integral: bool,
    low_precision: bool,
) -> Result<f32, CodecError> {
    let in_bounds = reader.read_bit("entity_field.coordinate_mp.in_bounds")?;
    let has_integer = reader.read_bit("entity_field.coordinate_mp.has_integer")?;
    if integral {
        if !has_integer {
            return Ok(0.0);
        }
        let negative = reader.read_bit("entity_field.coordinate_mp.sign")?;
        let width = if in_bounds { 11 } else { 14 };
        let value = reader.read_unsigned(width, "entity_field.coordinate_mp.integer")? + 1;
        return Ok(if negative {
            -(value as f32)
        } else {
            value as f32
        });
    }
    let negative = reader.read_bit("entity_field.coordinate_mp.sign")?;
    let integer = if has_integer {
        let width = if in_bounds { 11 } else { 14 };
        reader.read_unsigned(width, "entity_field.coordinate_mp.integer")? + 1
    } else {
        0
    };
    let fraction_width = if low_precision { 3 } else { 5 };
    let fraction = reader.read_unsigned(fraction_width, "entity_field.coordinate_mp.fraction")?;
    let value = integer as f32 + fraction as f32 / (1_u32 << fraction_width) as f32;
    Ok(if negative { -value } else { value })
}

fn apply_fields(
    destination: &mut BTreeMap<u16, FieldValue>,
    delta: &BTreeMap<u16, FieldValue>,
    limits: Limits,
) -> Result<(), CodecError> {
    if destination.len().saturating_add(delta.len()) > limits.max_fields_per_entity {
        return Err(limit_error(
            ErrorCode::EntityFieldLimit,
            "entity_fields.count",
            destination.len().saturating_add(delta.len()),
            limits.max_fields_per_entity,
        ));
    }
    destination.extend(delta.iter().map(|(index, value)| (*index, value.clone())));
    Ok(())
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

fn require_byte_padding(reader: &BitReader<'_>, field: &'static str) -> Result<(), CodecError> {
    if reader.bits_left() > 7 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::TrailingBits,
            field,
        ));
    }
    Ok(())
}

fn bit_width(maximum: usize) -> usize {
    usize::BITS as usize - maximum.leading_zeros() as usize
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
