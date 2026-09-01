use crate::{Error, ErrorCode, Float32, KeyData, KeyValue, err};
use playsrc_keyvalues::NumericValue;

/// Authored values only; missing parameters and surface lookup remain caller policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FluidProperties<'a> {
    pub index: i32,
    pub surface_property: Option<&'a [u8]>,
    pub damping: Option<Float32>,
    pub surface_plane: Option<[Float32; 4]>,
    pub current_velocity: Option<[Float32; 3]>,
    pub contents: Option<u32>,
}

impl KeyData {
    pub fn fluid_properties_at(&self, index: usize) -> Result<Option<FluidProperties<'_>>, Error> {
        let Some(block) = self
            .blocks
            .get(index)
            .filter(|block| block.name.eq_ignore_ascii_case(b"fluid"))
        else {
            return Ok(None);
        };
        let mut fluid = FluidProperties {
            index: 0,
            surface_property: None,
            damping: None,
            surface_plane: None,
            current_velocity: None,
            contents: None,
        };
        for entry in &block.entries {
            let KeyValue::Scalar { key, value } = entry else {
                return Err(err(
                    ErrorCode::InvalidKeydata,
                    self.document_range.clone(),
                    None,
                ));
            };
            if key.eq_ignore_ascii_case(b"index") {
                fluid.index = NumericValue::Bytes(value).get_int();
            } else if key.eq_ignore_ascii_case(b"surfaceprop") {
                fluid.surface_property = Some(&value[..value.len().min(511)]);
            } else if key.eq_ignore_ascii_case(b"damping") {
                fluid.damping = Some(Float32(NumericValue::Bytes(value).get_float().to_bits()));
            } else if key.eq_ignore_ascii_case(b"contents") {
                fluid.contents = Some(NumericValue::Bytes(value).get_int() as u32);
            } else if key.eq_ignore_ascii_case(b"surfaceplane") {
                fluid.surface_plane = Some(decimal_vector(value).ok_or_else(|| {
                    err(ErrorCode::InvalidKeydata, self.document_range.clone(), None)
                })?);
            } else if key.eq_ignore_ascii_case(b"currentvelocity") {
                fluid.current_velocity = Some(decimal_vector(value).ok_or_else(|| {
                    err(ErrorCode::InvalidKeydata, self.document_range.clone(), None)
                })?);
            }
        }
        Ok(Some(fluid))
    }
}

// The accepted decimal vector spelling permits adjacent signed numbers and
// ignores text after the final component. Failed conversions never expose
// uninitialized components. Other numeric spellings require separate admission.
fn decimal_vector<const N: usize>(bytes: &[u8]) -> Option<[Float32; N]> {
    let mut cursor = 0;
    let mut output = [Float32(0); N];
    for component in &mut output {
        while bytes
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        {
            cursor += 1;
        }
        let start = cursor;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'0') && matches!(bytes.get(cursor + 1), Some(b'x' | b'X')) {
            return None;
        }
        let integer = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        let mut digits = cursor - integer;
        if bytes.get(cursor) == Some(&b'.') {
            cursor += 1;
            let fraction = cursor;
            while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                cursor += 1;
            }
            digits += cursor - fraction;
        }
        if digits == 0 {
            return None;
        }
        if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
            cursor += 1;
            if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
                cursor += 1;
            }
            let exponent = cursor;
            while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                cursor += 1;
            }
            if exponent == cursor {
                return None;
            }
        }
        let value = std::str::from_utf8(&bytes[start..cursor])
            .ok()?
            .parse::<f32>()
            .ok()?;
        *component = Float32(value.to_bits());
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_vectors_keep_component_rounding_signs_and_conversion_boundaries() {
        assert_eq!(
            decimal_vector::<4>(b"0 0 1 -2160"),
            Some([0.0_f32, 0.0, 1.0, -2160.0].map(|v| Float32(v.to_bits())))
        );
        assert_eq!(
            decimal_vector::<3>(b"1-2+.3e1tail"),
            Some([1.0_f32, -2.0, 3.0].map(|v| Float32(v.to_bits())))
        );
        assert_eq!(decimal_vector::<3>(b"1 2"), None);
        assert_eq!(decimal_vector::<3>(b"1x 2 3"), None);
        assert_eq!(decimal_vector::<3>(b"1 2 3e"), None);
        assert_eq!(decimal_vector::<3>(b"1 2 0x1p0"), None);
        assert_eq!(
            decimal_vector::<3>(b"1\x0b-0\x0c3"),
            Some([
                Float32(1.0_f32.to_bits()),
                Float32((-0.0_f32).to_bits()),
                Float32(3.0_f32.to_bits())
            ])
        );
    }
}
