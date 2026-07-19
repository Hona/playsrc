use crate::EntityHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldType {
    Void,
    Boolean,
    Integer,
    Float,
    String,
    Vector,
    PositionVector,
    Color,
    Handle,
    Variant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueConversionError {
    Unsupported,
    NonFinite,
    MissingHandle,
}

pub(crate) fn convert_value(
    value: &super::Variant,
    target: FieldType,
    handle_name: impl FnOnce(EntityHandle) -> Option<Vec<u8>>,
    resolve_handle: impl FnOnce(&[u8]) -> Option<EntityHandle>,
) -> Result<super::Variant, ValueConversionError> {
    use super::Variant;

    if target == FieldType::Variant || value.field_type() == target {
        return Ok(value.clone());
    }
    if target == FieldType::Void {
        return Ok(Variant::Void);
    }
    match (value, target) {
        (Variant::Integer(value), FieldType::Float) => Ok(Variant::float(*value as f32)),
        (Variant::Integer(value), FieldType::Boolean) => Ok(Variant::Bool(*value != 0)),
        (Variant::Float(bits), FieldType::Integer) => {
            let value = f32::from_bits(*bits);
            if !value.is_finite() {
                return Err(ValueConversionError::NonFinite);
            }
            Ok(Variant::Integer(float_to_i32(value)))
        }
        (Variant::Float(bits), FieldType::Boolean) => {
            let value = f32::from_bits(*bits);
            if !value.is_finite() {
                return Err(ValueConversionError::NonFinite);
            }
            Ok(Variant::Bool(value != 0.0))
        }
        (Variant::String(value), FieldType::Integer) => Ok(Variant::Integer(source_integer(value))),
        (Variant::String(value), FieldType::Float) => {
            let value = source_float(value);
            if !value.is_finite() {
                return Err(ValueConversionError::NonFinite);
            }
            Ok(Variant::float(value))
        }
        (Variant::String(value), FieldType::Boolean) => {
            Ok(Variant::Bool(source_integer(value) != 0))
        }
        (Variant::String(value), FieldType::Vector) => Ok(Variant::vector(source_vector(value))),
        (Variant::String(value), FieldType::Color) => Ok(Variant::Color(source_color(value))),
        (Variant::String(value), FieldType::Handle) => Ok(resolve_handle(value)
            .map(Variant::Handle)
            .unwrap_or(Variant::Handle(EntityHandle::NULL))),
        (Variant::Handle(handle), FieldType::String) => handle_name(*handle)
            .map(Variant::String)
            .ok_or(ValueConversionError::MissingHandle),
        (Variant::Void, FieldType::String) => Ok(Variant::String(Vec::new())),
        _ => Err(ValueConversionError::Unsupported),
    }
}

pub(crate) fn project_string(
    value: &[u8],
    target: FieldType,
    resolve_handle: impl FnOnce(&[u8]) -> Option<EntityHandle>,
) -> Result<super::Variant, ValueConversionError> {
    use super::Variant;
    match target {
        FieldType::Void => Ok(Variant::Void),
        FieldType::Boolean => Ok(Variant::Bool(source_integer(value) != 0)),
        FieldType::Integer => Ok(Variant::Integer(source_integer(value))),
        FieldType::Float => {
            let value = source_float(value);
            value
                .is_finite()
                .then(|| Variant::float(value))
                .ok_or(ValueConversionError::NonFinite)
        }
        FieldType::String | FieldType::Variant => Ok(Variant::String(value.to_vec())),
        FieldType::Vector => Ok(Variant::vector(source_vector(value))),
        FieldType::PositionVector => Ok(Variant::position_vector(source_vector(value))),
        FieldType::Color => Ok(Variant::Color(source_color(value))),
        FieldType::Handle => Ok(Variant::Handle(
            resolve_handle(value).unwrap_or(EntityHandle::NULL),
        )),
    }
}

pub(crate) fn source_integer(value: &[u8]) -> i32 {
    let Ok(text) = std::str::from_utf8(value) else {
        return 0;
    };
    let bytes = text.trim_start().as_bytes();
    let (negative, start) = match bytes.first() {
        Some(b'-') => (true, 1),
        Some(b'+') => (false, 1),
        _ => (false, 0),
    };
    let mut magnitude = 0_u64;
    let mut any = false;
    for byte in bytes[start..]
        .iter()
        .copied()
        .take_while(u8::is_ascii_digit)
    {
        any = true;
        magnitude = magnitude
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'));
    }
    if !any {
        return 0;
    }
    if negative {
        (-(magnitude.min(i32::MAX as u64 + 1) as i64)).clamp(i32::MIN as i64, i32::MAX as i64)
            as i32
    } else {
        magnitude.min(i32::MAX as u64) as i32
    }
}

pub(crate) fn source_float(value: &[u8]) -> f32 {
    let Ok(text) = std::str::from_utf8(value) else {
        return 0.0;
    };
    let text = text.trim_start();
    let bytes = text.as_bytes();
    let mut end = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        end = 1;
    }
    let mut digits = 0;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
        digits += 1;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    let exponent = end;
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        end += 1;
        if matches!(bytes.get(end), Some(b'+' | b'-')) {
            end += 1;
        }
        let exponent_start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if exponent_start == end {
            end = exponent;
        }
    }
    text[..end].parse().unwrap_or(0.0)
}

pub(crate) fn source_vector(value: &[u8]) -> [f32; 3] {
    let Ok(text) = std::str::from_utf8(value) else {
        return [0.0; 3];
    };
    let trimmed = text.trim_start();
    let body = trimmed.strip_prefix('[').unwrap_or(trimmed);
    let body = body.strip_suffix(']').unwrap_or(body);
    let mut output = [0.0; 3];
    for (index, part) in body.split_ascii_whitespace().take(3).enumerate() {
        output[index] = source_float(part.as_bytes());
    }
    output
}

pub(crate) fn source_color(value: &[u8]) -> [u8; 4] {
    let mut output = [0, 0, 0, 255];
    for (index, part) in value
        .split(u8::is_ascii_whitespace)
        .filter(|part| !part.is_empty())
        .take(4)
        .enumerate()
    {
        output[index] = source_integer(part) as u8;
    }
    output
}

fn float_to_i32(value: f32) -> i32 {
    if value >= i32::MAX as f32 {
        i32::MAX
    } else if value <= i32::MIN as f32 {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Variant;

    #[test]
    fn source_string_projection_and_conversion_matrix_are_categorical() {
        assert_eq!(source_integer(b"  -12tail"), -12);
        assert_eq!(source_integer(b"bad"), 0);
        assert_eq!(source_float(b" 1.25tail").to_bits(), 1.25_f32.to_bits());
        assert_eq!(source_float(b"bad"), 0.0);
        assert_eq!(source_vector(b"[1 2]"), [1.0, 2.0, 0.0]);
        assert_eq!(source_color(b"1 2 3"), [1, 2, 3, 255]);

        let handle = EntityHandle {
            slot: 7,
            generation: 3,
        };
        let converted = convert_value(
            &Variant::String(b"target".to_vec()),
            FieldType::Handle,
            |_| None,
            |_| Some(handle),
        )
        .unwrap();
        assert_eq!(converted, Variant::Handle(handle));
        assert_eq!(
            convert_value(
                &Variant::Handle(handle),
                FieldType::String,
                |_| Some(b"target".to_vec()),
                |_| None,
            ),
            Ok(Variant::String(b"target".to_vec()))
        );
        assert_eq!(
            convert_value(&Variant::Bool(true), FieldType::Integer, |_| None, |_| None,),
            Err(ValueConversionError::Unsupported)
        );
    }
}
