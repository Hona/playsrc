use std::collections::BTreeSet;

use crate::{Error, ErrorCode};

const HEADER: &[u8] = b"<!-- dmx encoding binary 2 format pcf 1 -->\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Limits {
    pub max_bytes: usize,
    pub max_strings: usize,
    pub max_string_bytes: usize,
    pub max_elements: usize,
    pub max_attributes_per_element: usize,
    pub max_array_items: usize,
    pub max_total_values: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub elements: Vec<Element>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub element_type: String,
    pub name: String,
    pub uuid: [u8; 16],
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Element(Option<usize>),
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    ObjectId([u8; 16]),
    Color([u8; 4]),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    Angle([f32; 3]),
    Quaternion([f32; 4]),
    Matrix([f32; 16]),
    Array(Vec<Value>),
}

pub(crate) fn parse(bytes: &[u8], source: &str, limits: Limits) -> Result<Document, Error> {
    if bytes.len() > limits.max_bytes {
        return Err(Error::new(
            ErrorCode::InputLimit,
            source,
            0,
            "PCF bytes exceed max_bytes",
        ));
    }
    let mut reader = Reader {
        bytes,
        cursor: 0,
        source,
        limits,
        total_values: 0,
    };
    if reader.take(HEADER.len())? != HEADER || reader.byte()? != 0 {
        return Err(reader.error(
            ErrorCode::MalformedHeader,
            "expected binary-v2 PCF-v1 header",
        ));
    }

    let string_count = reader.u16()? as usize;
    if string_count > limits.max_strings {
        return Err(reader.error(ErrorCode::BoundExceeded, "string table exceeds max_strings"));
    }
    let mut strings = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        strings.push(reader.string()?);
    }

    let element_count = reader.count(limits.max_elements, "element count")?;
    let mut elements = Vec::with_capacity(element_count);
    for _ in 0..element_count {
        let type_index = reader.u16()? as usize;
        let Some(element_type) = strings.get(type_index) else {
            return Err(reader.error(
                ErrorCode::InvalidReference,
                "element type string index is invalid",
            ));
        };
        let name = reader.string()?;
        let mut uuid = [0; 16];
        uuid.copy_from_slice(reader.take(16)?);
        elements.push(Element {
            element_type: element_type.clone(),
            name,
            uuid,
            attributes: Vec::new(),
        });
    }

    for element in &mut elements {
        let attribute_count = reader.count(limits.max_attributes_per_element, "attribute count")?;
        let mut names = BTreeSet::new();
        element.attributes.reserve(attribute_count);
        for _ in 0..attribute_count {
            let name_index = reader.u16()? as usize;
            let Some(name) = strings.get(name_index) else {
                return Err(reader.error(
                    ErrorCode::InvalidReference,
                    "attribute name string index is invalid",
                ));
            };
            let canonical = name.to_ascii_lowercase();
            if !names.insert(canonical) {
                return Err(reader.error(
                    ErrorCode::DuplicateAttribute,
                    "element contains duplicate ASCII-insensitive attribute",
                ));
            }
            let value_type = reader.byte()?;
            let value = reader.value(value_type, element_count)?;
            element.attributes.push(Attribute {
                name: name.clone(),
                value,
            });
        }
    }
    if reader.cursor != bytes.len() {
        return Err(reader.error(
            ErrorCode::TrailingData,
            "PCF has bytes after its element graph",
        ));
    }
    Ok(Document { elements })
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
    source: &'a str,
    limits: Limits,
    total_values: usize,
}

impl Reader<'_> {
    fn error(&self, code: ErrorCode, detail: impl Into<String>) -> Error {
        Error::new(code, self.source, self.cursor, detail)
    }

    fn take(&mut self, length: usize) -> Result<&[u8], Error> {
        let end = self.cursor.checked_add(length).ok_or_else(|| {
            self.error(ErrorCode::Truncated, "byte range overflows address space")
        })?;
        let Some(value) = self.bytes.get(self.cursor..end) else {
            return Err(self.error(ErrorCode::Truncated, "PCF ends inside a value"));
        };
        self.cursor = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        let bytes: [u8; 2] = self.take(2)?.try_into().expect("two bytes");
        Ok(u16::from_le_bytes(bytes))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        let bytes: [u8; 4] = self.take(4)?.try_into().expect("four bytes");
        Ok(i32::from_le_bytes(bytes))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        let value = f32::from_bits(self.i32()? as u32);
        if !value.is_finite() {
            return Err(self.error(ErrorCode::NonFinite, "floating-point value is non-finite"));
        }
        Ok(value)
    }

    fn count(&mut self, limit: usize, field: &str) -> Result<usize, Error> {
        let value = self.i32()?;
        if value < 0 {
            return Err(self.error(ErrorCode::InvalidValue, format!("{field} is negative")));
        }
        let value = value as usize;
        if value > limit {
            return Err(self.error(
                ErrorCode::BoundExceeded,
                format!("{field} exceeds its limit"),
            ));
        }
        Ok(value)
    }

    fn string(&mut self) -> Result<String, Error> {
        let remaining = &self.bytes[self.cursor..];
        let Some(length) = remaining
            .iter()
            .take(self.limits.max_string_bytes + 1)
            .position(|byte| *byte == 0)
        else {
            return Err(self.error(
                ErrorCode::InvalidString,
                "string is unterminated or over limit",
            ));
        };
        let start = self.cursor;
        let source = self.source;
        let value = self.take(length + 1)?;
        let value = std::str::from_utf8(&value[..length]).map_err(|_| {
            Error::new(ErrorCode::InvalidUtf8, source, start, "string is not UTF-8")
        })?;
        if value.chars().any(char::is_control) {
            return Err(Error::new(
                ErrorCode::InvalidString,
                self.source,
                start,
                "string contains a control character",
            ));
        }
        Ok(value.to_owned())
    }

    fn reference(&mut self, element_count: usize) -> Result<Value, Error> {
        let index = self.i32()?;
        if index == -1 {
            return Ok(Value::Element(None));
        }
        if index < 0 || index as usize >= element_count {
            return Err(self.error(ErrorCode::InvalidReference, "element index is invalid"));
        }
        Ok(Value::Element(Some(index as usize)))
    }

    fn value(&mut self, value_type: u8, element_count: usize) -> Result<Value, Error> {
        self.total_values = self
            .total_values
            .checked_add(1)
            .ok_or_else(|| self.error(ErrorCode::BoundExceeded, "total value count overflowed"))?;
        if self.total_values > self.limits.max_total_values {
            return Err(self.error(
                ErrorCode::BoundExceeded,
                "total value count exceeds its limit",
            ));
        }
        if (15..=28).contains(&value_type) {
            let count = self.count(self.limits.max_array_items, "array item count")?;
            self.total_values = self.total_values.checked_add(count).ok_or_else(|| {
                self.error(ErrorCode::BoundExceeded, "total value count overflowed")
            })?;
            if self.total_values > self.limits.max_total_values {
                return Err(self.error(
                    ErrorCode::BoundExceeded,
                    "total value count exceeds its limit",
                ));
            }
            let scalar_type = value_type - 14;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(self.scalar(scalar_type, element_count)?);
            }
            return Ok(Value::Array(values));
        }
        self.scalar(value_type, element_count)
    }

    fn scalar(&mut self, value_type: u8, element_count: usize) -> Result<Value, Error> {
        match value_type {
            1 => self.reference(element_count),
            2 => Ok(Value::Int(self.i32()?)),
            3 => Ok(Value::Float(self.f32()?)),
            4 => match self.byte()? {
                0 => Ok(Value::Bool(false)),
                1 => Ok(Value::Bool(true)),
                _ => Err(self.error(ErrorCode::InvalidValue, "boolean is not zero or one")),
            },
            5 => Ok(Value::String(self.string()?)),
            6 => {
                let length = self.count(self.limits.max_bytes, "byte value length")?;
                Ok(Value::Bytes(self.take(length)?.to_vec()))
            }
            7 => {
                let mut value = [0; 16];
                value.copy_from_slice(self.take(16)?);
                Ok(Value::ObjectId(value))
            }
            8 => {
                let mut value = [0; 4];
                value.copy_from_slice(self.take(4)?);
                Ok(Value::Color(value))
            }
            9 => Ok(Value::Vector2([self.f32()?, self.f32()?])),
            10 => Ok(Value::Vector3([self.f32()?, self.f32()?, self.f32()?])),
            11 => Ok(Value::Vector4([
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
            ])),
            12 => Ok(Value::Angle([self.f32()?, self.f32()?, self.f32()?])),
            13 => Ok(Value::Quaternion([
                self.f32()?,
                self.f32()?,
                self.f32()?,
                self.f32()?,
            ])),
            14 => {
                let mut value = [0.0; 16];
                for item in &mut value {
                    *item = self.f32()?;
                }
                Ok(Value::Matrix(value))
            }
            _ => Err(self.error(ErrorCode::InvalidType, "DMX attribute type is unsupported")),
        }
    }
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&Value> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case(name))
            .map(|attribute| &attribute.value)
    }
}
