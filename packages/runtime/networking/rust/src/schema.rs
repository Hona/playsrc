use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BitReader, Classification, CodecError, ErrorCode, Limits, MAX_CLASS_COUNT, MAX_FLAT_PROPERTIES,
    failure,
};

pub const FLAG_UNSIGNED: u16 = 1 << 0;
pub const FLAG_COORDINATE: u16 = 1 << 1;
pub const FLAG_NO_SCALE: u16 = 1 << 2;
pub const FLAG_ROUND_DOWN: u16 = 1 << 3;
pub const FLAG_ROUND_UP: u16 = 1 << 4;
pub const FLAG_NORMAL_OR_VARINT: u16 = 1 << 5;
pub const FLAG_EXCLUDE: u16 = 1 << 6;
pub const FLAG_SHARED_EXPONENT: u16 = 1 << 7;
pub const FLAG_INSIDE_ARRAY: u16 = 1 << 8;
pub const FLAG_PROXY_ALWAYS: u16 = 1 << 9;
pub const FLAG_CHANGES_OFTEN: u16 = 1 << 10;
pub const FLAG_VECTOR_ELEMENT: u16 = 1 << 11;
pub const FLAG_COLLAPSIBLE: u16 = 1 << 12;
pub const FLAG_COORDINATE_MP: u16 = 1 << 13;
pub const FLAG_COORDINATE_MP_LOW: u16 = 1 << 14;
pub const FLAG_COORDINATE_MP_INTEGRAL: u16 = 1 << 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyKind {
    Integer,
    Float,
    Vector3,
    Vector2,
    String,
    Array,
    Table,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Property {
    pub name: Vec<u8>,
    pub kind: PropertyKind,
    pub flags: u16,
    pub table_name: Option<Vec<u8>>,
    pub array_elements: Option<u16>,
    pub low_value_bits: Option<u32>,
    pub high_value_bits: Option<u32>,
    pub bit_count: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendTable {
    pub name: Vec<u8>,
    pub needs_decoder: bool,
    pub properties: Vec<Property>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Class {
    pub id: u16,
    pub name: Vec<u8>,
    pub table_name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatProperty {
    pub source_table: Vec<u8>,
    pub property: Property,
    pub array_element: Option<Property>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemaRegistry {
    pub tables: BTreeMap<Vec<u8>, SendTable>,
    pub classes: BTreeMap<u16, Class>,
    pub flattened: BTreeMap<u16, Vec<FlatProperty>>,
    pub class_id_bits: u8,
}

impl SchemaRegistry {
    pub fn replace(
        &mut self,
        tables: Vec<SendTable>,
        classes: Vec<Class>,
        limits: Limits,
    ) -> Result<(), CodecError> {
        if tables.len() > limits.max_data_tables {
            return Err(limit_error(
                ErrorCode::TableLimit,
                "schema.tables",
                tables.len(),
                limits.max_data_tables,
            ));
        }
        if classes.is_empty()
            || classes.len() > limits.max_classes
            || classes.len() > MAX_CLASS_COUNT
        {
            return Err(limit_error(
                ErrorCode::ClassLimit,
                "schema.classes",
                classes.len(),
                limits.max_classes.min(MAX_CLASS_COUNT),
            ));
        }
        let mut next = Self::default();
        for table in tables {
            if table.properties.len() > limits.max_properties_per_table {
                return Err(limit_error(
                    ErrorCode::PropertyLimit,
                    "schema.table.properties",
                    table.properties.len(),
                    limits.max_properties_per_table,
                ));
            }
            if next.tables.insert(table.name.clone(), table).is_some() {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::DuplicateTable,
                    "schema.table.name",
                ));
            }
        }
        for class in classes {
            if usize::from(class.id) >= limits.max_classes.min(MAX_CLASS_COUNT) {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidClass,
                    "schema.class.id",
                ));
            }
            if !next.tables.contains_key(&class.table_name) {
                return Err(simple_error(
                    Classification::Missing,
                    ErrorCode::MissingTable,
                    "schema.class.table",
                ));
            }
            if next.classes.insert(class.id, class).is_some() {
                return Err(simple_error(
                    Classification::Malformed,
                    ErrorCode::DuplicateClass,
                    "schema.class.id",
                ));
            }
        }
        if next
            .classes
            .keys()
            .copied()
            .ne(0..next.classes.len() as u16)
        {
            return Err(simple_error(
                Classification::Malformed,
                ErrorCode::InvalidClass,
                "schema.class.id",
            ));
        }
        next.class_id_bits = bit_width(next.classes.len()) as u8;
        for (id, class) in &next.classes {
            let flattened = flatten_class(&next.tables, &class.table_name, limits)?;
            next.flattened.insert(*id, flattened);
        }
        *self = next;
        Ok(())
    }

    pub(crate) fn insert_table(
        &mut self,
        table: SendTable,
        limits: Limits,
    ) -> Result<(), CodecError> {
        if self.tables.len() >= limits.max_data_tables {
            return Err(limit_error(
                ErrorCode::TableLimit,
                "schema.tables",
                self.tables.len() + 1,
                limits.max_data_tables,
            ));
        }
        if table.properties.len() > limits.max_properties_per_table {
            return Err(limit_error(
                ErrorCode::PropertyLimit,
                "schema.table.properties",
                table.properties.len(),
                limits.max_properties_per_table,
            ));
        }
        if self.tables.insert(table.name.clone(), table).is_some() {
            return Err(simple_error(
                Classification::Malformed,
                ErrorCode::DuplicateTable,
                "schema.table.name",
            ));
        }
        Ok(())
    }

    pub(crate) fn replace_classes(
        &mut self,
        classes: Vec<Class>,
        limits: Limits,
    ) -> Result<(), CodecError> {
        let tables = self.tables.values().cloned().collect();
        self.replace(tables, classes, limits)
    }

    pub fn flat_properties(&self, class_id: u16) -> Option<&[FlatProperty]> {
        self.flattened.get(&class_id).map(Vec::as_slice)
    }
}

pub fn parse_demo_data_tables(
    bytes: &[u8],
    bit_length: usize,
    limits: Limits,
) -> Result<SchemaRegistry, CodecError> {
    if bit_length > limits.max_payload_bits {
        return Err(limit_error(
            ErrorCode::PayloadLimit,
            "demo_data_tables.payload",
            bit_length,
            limits.max_payload_bits,
        ));
    }
    let mut reader = BitReader::new(bytes, bit_length)?;
    let mut tables = Vec::new();
    while reader.read_bit("demo_data_tables.has_table")? {
        if tables.len() >= limits.max_data_tables {
            return Err(limit_error(
                ErrorCode::TableLimit,
                "demo_data_tables.tables",
                tables.len() + 1,
                limits.max_data_tables,
            ));
        }
        let needs_decoder = reader.read_bit("demo_data_tables.needs_decoder")?;
        tables.push(parse_send_table(&mut reader, needs_decoder, limits)?);
    }
    let count = reader.read_i16("demo_data_tables.class_count")?;
    let count = usize::try_from(count).map_err(|_| {
        simple_error(
            Classification::Malformed,
            ErrorCode::ClassLimit,
            "demo_data_tables.class_count",
        )
    })?;
    if count == 0 || count > limits.max_classes.min(MAX_CLASS_COUNT) {
        return Err(limit_error(
            ErrorCode::ClassLimit,
            "demo_data_tables.class_count",
            count,
            limits.max_classes.min(MAX_CLASS_COUNT),
        ));
    }
    let mut classes = Vec::with_capacity(count);
    for _ in 0..count {
        let id = reader.read_i16("demo_data_tables.class.id")?;
        let id = u16::try_from(id).map_err(|_| {
            simple_error(
                Classification::Malformed,
                ErrorCode::InvalidClass,
                "demo_data_tables.class.id",
            )
        })?;
        classes.push(Class {
            id,
            name: reader.read_c_string(limits.max_string_bytes, "demo_data_tables.class.name")?,
            table_name: reader
                .read_c_string(limits.max_string_bytes, "demo_data_tables.class.table")?,
        });
    }
    if reader.bits_left() > 7 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::TrailingBits,
            "demo_data_tables.trailing_bits",
        ));
    }
    let mut registry = SchemaRegistry::default();
    registry.replace(tables, classes, limits)?;
    Ok(registry)
}

pub(crate) fn parse_send_table(
    reader: &mut BitReader<'_>,
    needs_decoder: bool,
    limits: Limits,
) -> Result<SendTable, CodecError> {
    let name = reader.read_c_string(limits.max_string_bytes, "send_table.name")?;
    let property_count = reader.read_unsigned(10, "send_table.property_count")? as usize;
    if property_count > limits.max_properties_per_table {
        return Err(limit_error(
            ErrorCode::PropertyLimit,
            "send_table.property_count",
            property_count,
            limits.max_properties_per_table,
        ));
    }
    let mut properties = Vec::with_capacity(property_count);
    for _ in 0..property_count {
        let raw_kind = reader.read_unsigned(5, "send_table.property.kind")?;
        let kind = match raw_kind {
            0 => PropertyKind::Integer,
            1 => PropertyKind::Float,
            2 => PropertyKind::Vector3,
            3 => PropertyKind::Vector2,
            4 => PropertyKind::String,
            5 => PropertyKind::Array,
            6 => PropertyKind::Table,
            _ => {
                return Err(simple_error(
                    Classification::Unknown,
                    ErrorCode::InvalidPropertyType,
                    "send_table.property.kind",
                ));
            }
        };
        let property_name =
            reader.read_c_string(limits.max_string_bytes, "send_table.property.name")?;
        let flags = reader.read_unsigned(16, "send_table.property.flags")? as u16;
        let mut property = Property {
            name: property_name,
            kind,
            flags,
            table_name: None,
            array_elements: None,
            low_value_bits: None,
            high_value_bits: None,
            bit_count: None,
        };
        if kind == PropertyKind::Table {
            property.table_name =
                Some(reader.read_c_string(limits.max_string_bytes, "send_table.property.table")?);
        } else if flags & FLAG_EXCLUDE != 0 {
            property.table_name = Some(
                reader
                    .read_c_string(limits.max_string_bytes, "send_table.property.exclude_table")?,
            );
        } else if kind == PropertyKind::Array {
            property.array_elements =
                Some(reader.read_unsigned(10, "send_table.property.array_elements")? as u16);
        } else {
            property.low_value_bits = Some(reader.read_u32("send_table.property.low")?);
            property.high_value_bits = Some(reader.read_u32("send_table.property.high")?);
            property.bit_count =
                Some(reader.read_unsigned(7, "send_table.property.bit_count")? as u8);
        }
        validate_property(&property)?;
        properties.push(property);
    }
    Ok(SendTable {
        name,
        needs_decoder,
        properties,
    })
}

fn validate_property(property: &Property) -> Result<(), CodecError> {
    let coordinate_flags = [
        FLAG_COORDINATE,
        FLAG_COORDINATE_MP,
        FLAG_COORDINATE_MP_LOW,
        FLAG_COORDINATE_MP_INTEGRAL,
    ]
    .into_iter()
    .filter(|flag| property.flags & flag != 0)
    .count();
    if coordinate_flags > 1 {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidPropertyFlags,
            "send_table.property.flags.coordinate",
        ));
    }
    if property.kind == PropertyKind::Array && property.array_elements == Some(0) {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::InvalidArray,
            "send_table.property.array_elements",
        ));
    }
    Ok(())
}

fn flatten_class(
    tables: &BTreeMap<Vec<u8>, SendTable>,
    root: &[u8],
    limits: Limits,
) -> Result<Vec<FlatProperty>, CodecError> {
    let mut exclusions = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    collect_exclusions(tables, root, &mut visiting, &mut exclusions)?;
    visiting.clear();
    let mut output = Vec::new();
    flatten_table(
        tables,
        root,
        &exclusions,
        &mut visiting,
        &mut output,
        limits,
    )?;
    let mut start = 0;
    while start < output.len() {
        let Some(index) = (start..output.len())
            .find(|index| output[*index].property.flags & FLAG_CHANGES_OFTEN != 0)
        else {
            break;
        };
        output.swap(start, index);
        start += 1;
    }
    Ok(output)
}

fn collect_exclusions(
    tables: &BTreeMap<Vec<u8>, SendTable>,
    table_name: &[u8],
    visiting: &mut BTreeSet<Vec<u8>>,
    output: &mut BTreeSet<(Vec<u8>, Vec<u8>)>,
) -> Result<(), CodecError> {
    if !visiting.insert(table_name.to_vec()) {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::CyclicTable,
            "schema.table.cycle",
        ));
    }
    let table = tables.get(table_name).ok_or_else(|| {
        simple_error(
            Classification::Missing,
            ErrorCode::MissingTable,
            "schema.table.reference",
        )
    })?;
    for property in &table.properties {
        if property.flags & FLAG_EXCLUDE != 0 {
            let target = property.table_name.clone().ok_or_else(|| {
                simple_error(
                    Classification::Malformed,
                    ErrorCode::InvalidPropertyFlags,
                    "schema.property.exclude_table",
                )
            })?;
            output.insert((target, property.name.clone()));
        } else if property.kind == PropertyKind::Table {
            collect_exclusions(
                tables,
                property.table_name.as_deref().ok_or_else(|| {
                    simple_error(
                        Classification::Missing,
                        ErrorCode::MissingTable,
                        "schema.property.table",
                    )
                })?,
                visiting,
                output,
            )?;
        }
    }
    visiting.remove(table_name);
    Ok(())
}

fn flatten_table(
    tables: &BTreeMap<Vec<u8>, SendTable>,
    table_name: &[u8],
    exclusions: &BTreeSet<(Vec<u8>, Vec<u8>)>,
    visiting: &mut BTreeSet<Vec<u8>>,
    output: &mut Vec<FlatProperty>,
    limits: Limits,
) -> Result<(), CodecError> {
    if !visiting.insert(table_name.to_vec()) {
        return Err(simple_error(
            Classification::Malformed,
            ErrorCode::CyclicTable,
            "schema.table.cycle",
        ));
    }
    let mut local = Vec::new();
    flatten_table_properties(
        tables, table_name, exclusions, visiting, output, &mut local, limits,
    )?;
    for property in local {
        if output.len() >= limits.max_flat_properties.min(MAX_FLAT_PROPERTIES) {
            return Err(limit_error(
                ErrorCode::FlatPropertyLimit,
                "schema.flat_properties",
                output.len() + 1,
                limits.max_flat_properties.min(MAX_FLAT_PROPERTIES),
            ));
        }
        output.push(property);
    }
    visiting.remove(table_name);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn flatten_table_properties(
    tables: &BTreeMap<Vec<u8>, SendTable>,
    table_name: &[u8],
    exclusions: &BTreeSet<(Vec<u8>, Vec<u8>)>,
    visiting: &mut BTreeSet<Vec<u8>>,
    output: &mut Vec<FlatProperty>,
    local: &mut Vec<FlatProperty>,
    limits: Limits,
) -> Result<(), CodecError> {
    let table = tables.get(table_name).ok_or_else(|| {
        simple_error(
            Classification::Missing,
            ErrorCode::MissingTable,
            "schema.table.reference",
        )
    })?;
    for (index, property) in table.properties.iter().enumerate() {
        if property.flags & (FLAG_EXCLUDE | FLAG_INSIDE_ARRAY) != 0
            || exclusions.contains(&(table.name.clone(), property.name.clone()))
        {
            continue;
        }
        if property.kind == PropertyKind::Table {
            let child = property.table_name.as_deref().ok_or_else(|| {
                simple_error(
                    Classification::Missing,
                    ErrorCode::MissingTable,
                    "schema.property.table",
                )
            })?;
            if property.flags & FLAG_COLLAPSIBLE != 0 {
                if !visiting.insert(child.to_vec()) {
                    return Err(simple_error(
                        Classification::Malformed,
                        ErrorCode::CyclicTable,
                        "schema.table.cycle",
                    ));
                }
                flatten_table_properties(
                    tables, child, exclusions, visiting, output, local, limits,
                )?;
                visiting.remove(child);
            } else {
                flatten_table(tables, child, exclusions, visiting, output, limits)?;
            }
            continue;
        }
        let array_element = if property.kind == PropertyKind::Array {
            let element = index
                .checked_sub(1)
                .and_then(|previous| table.properties.get(previous))
                .filter(|element| element.flags & FLAG_INSIDE_ARRAY != 0)
                .cloned()
                .ok_or_else(|| {
                    simple_error(
                        Classification::Malformed,
                        ErrorCode::InvalidArray,
                        "schema.property.array_element",
                    )
                })?;
            Some(element)
        } else {
            None
        };
        if output.len().saturating_add(local.len())
            >= limits.max_flat_properties.min(MAX_FLAT_PROPERTIES)
        {
            return Err(limit_error(
                ErrorCode::FlatPropertyLimit,
                "schema.flat_properties",
                output.len() + local.len() + 1,
                limits.max_flat_properties.min(MAX_FLAT_PROPERTIES),
            ));
        }
        local.push(FlatProperty {
            source_table: table.name.clone(),
            property: property.clone(),
            array_element,
        });
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
