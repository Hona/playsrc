// Vector normalization follows Valve Source SDK 2013 public/mathlib/vector.h.
// Copyright Valve Corporation. The Source 1 SDK License applies;
// see the repository's LICENSE.source-sdk-2013.
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceVectorError {
    NonFinite,
    ReciprocalRootDomain,
}
impl fmt::Display for SourceVectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "Source vector normalization contains a non-finite value",
            Self::ReciprocalRootDomain => {
                "Source reciprocal-root argument is outside the normalization domain"
            }
        })
    }
}
impl std::error::Error for SourceVectorError {}

/// The fixed Source PC estimate profile over the complete normalization domain.
pub fn source_reciprocal_root_estimate(argument: f32) -> Result<f32, SourceVectorError> {
    if !argument.is_finite() {
        return Err(SourceVectorError::NonFinite);
    }
    if argument < 1.0e-10_f32 {
        return Err(SourceVectorError::ReciprocalRootDomain);
    }
    let bits = argument.to_bits();
    let exponent = ((bits >> 23) & 255) as i32 - 127;
    let parity = exponent.rem_euclid(2);
    let scale = (exponent - parity) / 2;
    let normalized = (((127 + parity) as u32) << 23) | (bits & 0x7f_ffff);
    let midpoint = f32::from_bits((normalized & !0x1fff) | 0x1000);
    let root = (1.0 / f64::from(midpoint).sqrt()) as f32;
    let quantized = (root.to_bits() + 0x400) & !0x7ff;
    let adjusted = i64::from(quantized) - i64::from(scale) * 0x80_0000;
    Ok(f32::from_bits(adjusted as u32))
}

pub fn normalize_source_vector(vector: [f32; 3]) -> Result<[f32; 3], SourceVectorError> {
    if vector.iter().any(|v| !v.is_finite()) {
        return Err(SourceVectorError::NonFinite);
    }
    let squared =
        ((vector[1] * vector[1] + vector[0] * vector[0]) + vector[2] * vector[2]) + 1.0e-10_f32;
    let estimate = source_reciprocal_root_estimate(squared)?;
    let residual = 3.0_f32 - ((estimate * estimate) * squared);
    let inverse = estimate * (residual * 0.5_f32);
    Ok(vector.map(|value| value * inverse))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_unit_normals_retain_the_sdk_refinement_and_signed_zero() {
        assert_eq!(
            normalize_source_vector([0.0, 0.0, -1.0])
                .unwrap()
                .map(f32::to_bits),
            [0, 0, 0xbf7f_ffff]
        );
        assert_eq!(
            normalize_source_vector([-0.0, 0.0, 0.0])
                .unwrap()
                .map(f32::to_bits),
            [0x8000_0000, 0, 0]
        );
        assert_eq!(
            source_reciprocal_root_estimate(0.0),
            Err(SourceVectorError::ReciprocalRootDomain)
        );
        assert_eq!(
            normalize_source_vector([f32::MAX, 0.0, 0.0]),
            Err(SourceVectorError::NonFinite)
        );
    }
}
