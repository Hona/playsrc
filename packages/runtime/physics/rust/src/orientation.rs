// Source angle basis construction follows Valve's Source SDK 2013 AngleVectors.
// Copyright Valve Corporation. See LICENSE.source-sdk-2013 at the repository root.
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrientationError {
    NonFinite,
    UnsupportedAngle,
    Degenerate,
    NegativeDeviationRadius,
}

impl fmt::Display for OrientationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => {
                formatter.write_str("physical orientation contains a non-finite value")
            }
            Self::UnsupportedAngle => {
                formatter.write_str("physical orientation exceeds the established angle domain")
            }
            Self::Degenerate => formatter.write_str("physical orientation has zero magnitude"),
            Self::NegativeDeviationRadius => {
                formatter.write_str("physical angular envelope radius cannot be negative")
            }
        }
    }
}

impl std::error::Error for OrientationError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceAngleBasis {
    pub matrix: [f32; 9],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoreOrientation {
    pub quaternion: [f64; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationEnvelope {
    pub world_axis: [f32; 3],
    pub angular_speed: f32,
    pub surface_speed: f32,
}

impl RotationEnvelope {
    pub fn from_motion(
        angular_velocity: [f32; 3],
        timestep: f32,
        retained_basis: [f64; 9],
        deviation_radius: f32,
    ) -> Result<Self, OrientationError> {
        let delta = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        }
        .advance(angular_velocity, timestep)?;
        Self::from_delta(delta, timestep, retained_basis, deviation_radius)
    }
    pub fn from_delta(
        delta: CoreOrientation,
        timestep: f32,
        retained_basis: [f64; 9],
        deviation_radius: f32,
    ) -> Result<Self, OrientationError> {
        if !deviation_radius.is_finite() || retained_basis.iter().any(|value| !value.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        if deviation_radius < 0.0 {
            return Err(OrientationError::NegativeDeviationRadius);
        }
        let (vector, inverse, angular_speed) = rotation_measure_delta(delta, timestep)?;
        let world_axis = if inverse == 0.0 {
            [1.0, 0.0, 0.0]
        } else {
            std::array::from_fn(|row| {
                let first = retained_basis[row * 3] * vector[0];
                let second = retained_basis[row * 3 + 1] * vector[1];
                let third = retained_basis[row * 3 + 2] * vector[2];
                ((first + second + third) * inverse) as f32
            })
        };
        let surface_speed = angular_speed * deviation_radius;
        if !surface_speed.is_finite() || world_axis.iter().any(|value| !value.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        Ok(Self {
            world_axis,
            angular_speed,
            surface_speed,
        })
    }
}

fn rotation_measure(
    angular_velocity: [f32; 3],
    timestep: f32,
) -> Result<([f64; 3], f64, f32), OrientationError> {
    if !timestep.is_finite() || timestep <= 0.0 {
        return Err(OrientationError::NonFinite);
    }
    let delta = CoreOrientation {
        quaternion: [0.0, 0.0, 0.0, 1.0],
    }
    .advance(angular_velocity, timestep)?;
    rotation_measure_delta(delta, timestep)
}
fn rotation_measure_delta(
    delta: CoreOrientation,
    timestep: f32,
) -> Result<([f64; 3], f64, f32), OrientationError> {
    if !timestep.is_finite() || timestep <= 0.0 || delta.quaternion.iter().any(|v| !v.is_finite()) {
        return Err(OrientationError::NonFinite);
    }
    let delta = delta.quaternion;
    let vector = [delta[0], delta[1], delta[2]];
    let squared = (vector[0] * vector[0] + vector[1] * vector[1]) + vector[2] * vector[2];
    if squared <= 1.0e-19 {
        return Ok((vector, 0.0, 0.0));
    }
    let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
    let sine = (squared * inverse) as f32;
    let square = sine * sine;
    let cube = square * sine;
    let fifth = (cube * square) * 0.404_14_f32;
    let angle = ((sine + sine) + cube * (1.0_f32 / 3.0)) + (fifth + fifth);
    let speed = angle * (1.0 / f64::from(timestep)) as f32;
    Ok((vector, inverse, speed))
}

impl SourceAngleBasis {
    pub fn from_degrees(angles: [f32; 3]) -> Result<Self, OrientationError> {
        if angles.iter().any(|angle| !angle.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        let radians = angles.map(|angle| angle * f32::from_bits(0x3c8e_fa35));
        let half_pi = f32::from_bits(0x3fc9_0fdb);
        if [
            radians[0],
            radians[0] + half_pi,
            radians[1],
            radians[1] + half_pi,
        ]
        .iter()
        .any(|angle| angle.abs() > 10_000.0)
            || (radians[2].abs() >= f32::from_bits(0x3900_0000)
                && (radians[2].abs() + half_pi).abs() > 10_000.0)
        {
            return Err(OrientationError::UnsupportedAngle);
        }
        let pitch = radians[0];
        let yaw = radians[1];
        let roll = radians[2];
        let sp = vectorized_sine(pitch);
        let cp = vectorized_sine(pitch + half_pi);
        let sy = vectorized_sine(yaw);
        let cy = vectorized_sine(yaw + half_pi);
        let (sr, cr) = if roll.abs() < f32::from_bits(0x3900_0000) {
            (roll, 1.0 - roll * roll * 0.5)
        } else {
            (vectorized_sine(roll), vectorized_sine(roll.abs() + half_pi))
        };
        let right = [
            (-sr * sp) * cy + -cr * -sy,
            (-sr * sp) * sy + -cr * cy,
            -sr * cp,
        ];
        let up = [
            (cr * sp) * cy + -sr * -sy,
            (cr * sp) * sy + -sr * cy,
            cr * cp,
        ];
        Ok(Self {
            matrix: [
                cp * cy,
                -right[0],
                up[0],
                cp * sy,
                -right[1],
                up[1],
                -sp,
                -right[2],
                up[2],
            ],
        })
    }

    pub fn core_orientation(self) -> Result<CoreOrientation, OrientationError> {
        Ok(self.body_orientations()?.1)
    }

    /// Returns the initial mass-center placement orientation and the post-rebase core orientation.
    pub fn body_orientations(self) -> Result<(CoreOrientation, CoreOrientation), OrientationError> {
        let object = self.object_orientation()?.normalized()?;
        let core = CoreOrientation::from_matrix(object.matrix())?;
        Ok((object, core))
    }

    pub fn object_orientation(self) -> Result<CoreOrientation, OrientationError> {
        let source = self.matrix.map(f64::from);
        let internal = [
            source[0], -source[2], source[1], -source[6], source[8], -source[7], source[3],
            -source[5], source[4],
        ];
        CoreOrientation::from_matrix(internal)
    }
}

impl CoreOrientation {
    pub fn normalized(self) -> Result<Self, OrientationError> {
        if self.quaternion.iter().any(|value| !value.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        let q = self.quaternion;
        let squared = (q[3] * q[3] + q[2] * q[2]) + (q[0] * q[0] + q[1] * q[1]);
        if !squared.is_finite() {
            return Err(OrientationError::NonFinite);
        }
        if squared <= 1.0e-19 {
            return Ok(self);
        }
        let inverse = 1.0 / squared.sqrt();
        Ok(Self {
            quaternion: q.map(|value| value * inverse),
        })
    }
    pub fn from_matrix(matrix: [f64; 9]) -> Result<Self, OrientationError> {
        if matrix.iter().any(|component| !component.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        let trace = (matrix[0] + matrix[4]) + matrix[8];
        let quaternion = if trace > 0.0 {
            let root = (trace + 1.0).sqrt();
            let scale = 0.5 / root;
            [
                (matrix[7] - matrix[5]) * scale,
                (matrix[2] - matrix[6]) * scale,
                (matrix[3] - matrix[1]) * scale,
                root * 0.5,
            ]
        } else {
            let mut axis = usize::from(matrix[4] > matrix[0]);
            if matrix[8] > matrix[axis * 3 + axis] {
                axis = 2;
            }
            let next = (axis + 1) % 3;
            let last = (next + 1) % 3;
            let root = ((matrix[axis * 3 + axis]
                - (matrix[last * 3 + last] + matrix[next * 3 + next]))
                + 1.0)
                .sqrt();
            let scale = if root != 0.0 { 0.5 / root } else { root };
            let mut value = [0.0; 4];
            value[axis] = root * 0.5;
            value[3] = (matrix[last * 3 + next] - matrix[next * 3 + last]) * scale;
            value[next] = (matrix[next * 3 + axis] + matrix[axis * 3 + next]) * scale;
            value[last] = (matrix[last * 3 + axis] + matrix[axis * 3 + last]) * scale;
            value
        };
        let squared = (quaternion[3] * quaternion[3] + quaternion[2] * quaternion[2])
            + (quaternion[0] * quaternion[0] + quaternion[1] * quaternion[1]);
        if squared == 0.0 || !squared.is_finite() {
            return Err(OrientationError::Degenerate);
        }
        let scale = 1.0 / squared.sqrt();
        Ok(Self {
            quaternion: quaternion.map(|component| component * scale),
        })
    }

    pub fn matrix(self) -> [f64; 9] {
        let [x, y, z, w] = self.quaternion;
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ]
    }

    pub fn source_matrix(self) -> [f32; 9] {
        let matrix = self.matrix();
        [
            matrix[0], matrix[2], -matrix[1], matrix[6], matrix[8], -matrix[7], -matrix[3],
            -matrix[5], matrix[4],
        ]
        .map(|component| component as f32)
    }
    pub fn source_angles(self) -> Result<[f32; 3], OrientationError> {
        let matrix = self.matrix().map(|value| value as f32);
        if matrix.iter().any(|value| !value.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        let horizontal = (matrix[6] * matrix[6] + matrix[0] * matrix[0]).sqrt();
        let degrees = f32::from_bits(0x4265_2ee1);
        let pitch = f64::from(matrix[3]).atan2(f64::from(horizontal)) as f32;
        let (yaw, roll) = if f64::from(horizontal) > 0.001 {
            (
                (f64::from(matrix[6]).atan2(f64::from(matrix[0])) as f32) * degrees,
                (f64::from(matrix[5]).atan2(f64::from(-matrix[4])) as f32) * degrees + 180.0,
            )
        } else {
            (
                (f64::from(matrix[2]).atan2(f64::from(-matrix[8])) as f32) * degrees,
                180.0,
            )
        };
        Ok([pitch * degrees, yaw, roll])
    }

    pub fn advance(
        self,
        angular_velocity: [f32; 3],
        timestep: f32,
    ) -> Result<Self, OrientationError> {
        if !timestep.is_finite()
            || angular_velocity
                .iter()
                .any(|component| !component.is_finite())
            || self
                .quaternion
                .iter()
                .any(|component| !component.is_finite())
        {
            return Err(OrientationError::NonFinite);
        }
        let half_timestep = f64::from(timestep) * 0.5;
        let arguments = angular_velocity.map(|component| f64::from(component) * half_timestep);
        if arguments
            .iter()
            .any(|argument| argument.abs() > std::f64::consts::FRAC_PI_4)
        {
            return Err(OrientationError::UnsupportedAngle);
        }
        let vector = arguments.map(small_sine);
        let squared = (vector[0] * vector[0] + vector[1] * vector[1]) + vector[2] * vector[2];
        if squared > 1.0 {
            return Err(OrientationError::UnsupportedAngle);
        }
        let delta = [vector[0], vector[1], vector[2], (1.0 - squared).sqrt()];
        self.apply_local_delta(Self { quaternion: delta })
    }
    pub(crate) fn product(self, delta: Self) -> Result<Self, OrientationError> {
        let delta = delta.quaternion;
        if self.quaternion.iter().chain(&delta).any(|v| !v.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        let current = self.quaternion;
        let quaternion = [
            current[3] * delta[0] + current[0] * delta[3] + current[1] * delta[2]
                - current[2] * delta[1],
            current[3] * delta[1] + current[1] * delta[3] + current[2] * delta[0]
                - current[0] * delta[2],
            current[2] * delta[3] + current[3] * delta[2] + current[0] * delta[1]
                - current[1] * delta[0],
            current[3] * delta[3]
                - current[0] * delta[0]
                - current[1] * delta[1]
                - current[2] * delta[2],
        ];
        Ok(Self { quaternion })
    }
    pub(crate) fn apply_local_delta(self, delta: Self) -> Result<Self, OrientationError> {
        let mut quaternion = self.product(delta)?.quaternion;
        let squared = (quaternion[2] * quaternion[2] + quaternion[3] * quaternion[3])
            + (quaternion[0] * quaternion[0] + quaternion[1] * quaternion[1]);
        if !squared.is_finite() || squared <= 0.0 {
            return Err(OrientationError::Degenerate);
        }
        if (1.0 - squared).abs() > 1.0e-12 {
            let mut scale = 1.5 - 0.5 * squared;
            loop {
                scale += 0.5 * (1.0 - scale * scale * squared);
                if (1.0 - scale * scale * squared).abs() <= 1.0e-12 {
                    break;
                }
            }
            for component in &mut quaternion {
                *component *= scale;
            }
        }
        Ok(Self { quaternion })
    }

    pub fn predicted_angular_speed(
        angular_velocity: [f32; 3],
        timestep: f32,
    ) -> Result<f32, OrientationError> {
        Ok(rotation_measure(angular_velocity, timestep)?.2)
    }
    pub fn phase_angular_velocity(
        self,
        next: Self,
        inverse_step: f32,
    ) -> Result<[f32; 3], OrientationError> {
        if !inverse_step.is_finite()
            || self
                .quaternion
                .iter()
                .chain(&next.quaternion)
                .any(|v| !v.is_finite())
        {
            return Err(OrientationError::NonFinite);
        }
        if inverse_step <= 0.0 {
            return Err(OrientationError::UnsupportedAngle);
        }
        let p = self.quaternion;
        let a = [-p[0], -p[1], -p[2], p[3]];
        let b = next.quaternion;
        let delta = [
            ((a[3] * b[0] + a[0] * b[3]) + a[1] * b[2]) - a[2] * b[1],
            ((a[3] * b[1] + a[1] * b[3]) + a[2] * b[0]) - a[0] * b[2],
            ((a[2] * b[3] + a[3] * b[2]) + a[0] * b[1]) - a[1] * b[0],
        ];
        let factor = f64::from(inverse_step + inverse_step);
        let mut output = [0.0; 3];
        for axis in 0..3 {
            output[axis] = (unit_arc_sine(delta[axis])? * factor) as f32;
        }
        if output.iter().any(|v| !v.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        Ok(output)
    }

    pub fn interpolate(self, next: Self, fraction: f64) -> Result<Self, OrientationError> {
        if !fraction.is_finite()
            || self
                .quaternion
                .iter()
                .any(|component| !component.is_finite())
            || next
                .quaternion
                .iter()
                .any(|component| !component.is_finite())
        {
            return Err(OrientationError::NonFinite);
        }
        let start = self.quaternion;
        let mut end = next.quaternion;
        let mut dot =
            (start[3] * end[3] + start[2] * end[2]) + (start[1] * end[1] + start[0] * end[0]);
        if dot < 0.0 {
            end = end.map(|component| -component);
            dot = -dot;
        }
        if dot >= 0.998_999_999_952_502_5 {
            let mut quaternion: [f64; 4] =
                std::array::from_fn(|axis| start[axis] + (end[axis] - start[axis]) * fraction);
            let half_squared = ((quaternion[2] * quaternion[2] + quaternion[3] * quaternion[3])
                + (quaternion[0] * quaternion[0] + quaternion[1] * quaternion[1]))
                * 0.5;
            let mut factor = 1.5 - half_squared;
            factor += 0.5 - factor * factor * half_squared;
            factor += 0.5 - factor * factor * half_squared;
            for component in &mut quaternion {
                *component *= factor;
            }
            return Ok(Self { quaternion });
        }
        if !(0.5..=1.0).contains(&dot) {
            return Err(OrientationError::UnsupportedAngle);
        }
        let angle = unit_arc_cosine(dot);
        let first_angle = (1.0 - fraction) * angle;
        let second_angle = fraction * angle;
        if first_angle.abs() > std::f64::consts::FRAC_PI_4
            || second_angle.abs() > std::f64::consts::FRAC_PI_4
        {
            return Err(OrientationError::UnsupportedAngle);
        }
        let inverse_sine = 1.0 / (1.0 - dot * dot).sqrt();
        let start_scale = small_sine(first_angle) * inverse_sine;
        let end_scale = small_sine(second_angle) * inverse_sine;
        Ok(Self {
            quaternion: std::array::from_fn(|axis| {
                start[axis] * start_scale + end[axis] * end_scale
            }),
        })
    }
}

fn vectorized_sine(angle: f32) -> f32 {
    let absolute = f32::from_bits(angle.to_bits() & 0x7fff_ffff);
    let rounded = absolute * f32::from_bits(0x3ea2_f983) + f32::from_bits(0x4b40_0000);
    let periods = rounded - f32::from_bits(0x4b40_0000);
    let mut reduced = absolute;
    for part in [0x4049_0000, 0x3a7d_a000, 0x3422_2000, 0x2cb4_611a] {
        reduced -= f32::from_bits(part) * periods;
    }
    reduced = f32::from_bits(reduced.to_bits() ^ (rounded.to_bits() << 31));
    let squared = reduced * reduced;
    let mut polynomial = f32::from_bits(0x362e_def8) * squared;
    polynomial += f32::from_bits(0xb94f_b7ff);
    polynomial *= squared;
    polynomial += f32::from_bits(0x3c08_8766);
    polynomial *= squared;
    polynomial += f32::from_bits(0xbe2a_aaa6);
    let result = reduced + (squared * polynomial) * reduced;
    f32::from_bits(result.to_bits() ^ (angle.to_bits() & 0x8000_0000))
}

fn small_sine(angle: f64) -> f64 {
    let absolute = angle.abs();
    if absolute < f64::from_bits(0x3e40_0000_0000_0000) {
        return angle;
    }
    let squared = angle * angle;
    if absolute < f64::from_bits(0x3f20_0000_0000_0000) {
        return (-(squared * angle)).mul_add(f64::from_bits(0x3fc5_5555_5555_5555), angle);
    }
    let mut polynomial = f64::from_bits(0x3de5_e0b2_f9a4_3bb8);
    for coefficient in [
        0xbe5a_e600_b42f_dfa7,
        0x3ec7_1de3_796c_de01,
        0xbf2a_01a0_19e8_3e5c,
        0x3f81_1111_1111_0bb3,
        0xbfc5_5555_5555_5555,
    ] {
        polynomial = squared.mul_add(polynomial, f64::from_bits(coefficient));
    }
    (angle * squared).mul_add(polynomial, angle)
}

fn arc_ratio(reduced: f64) -> f64 {
    let mut numerator = f64::from_bits(0x3f09_5166_5d32_1061) * reduced;
    let mut denominator = f64::from_bits(0x3fbb_1a42_2982_ce76) * reduced;
    numerator += f64::from_bits(0x3f51_e5f8_87a6_2135);
    denominator -= f64::from_bits(0x3fee_324a_b418_f78d);
    numerator *= reduced;
    denominator *= reduced;
    numerator -= f64::from_bits(0x3fac_28d3_90c2_9690);
    denominator += f64::from_bits(0x4006_2021_571d_ccfc);
    numerator *= reduced;
    denominator *= reduced;
    numerator += f64::from_bits(0x3fd1_a2be_c1b7_ef59);
    denominator -= f64::from_bits(0x400a_4646_f903_cdea);
    numerator *= reduced;
    denominator *= reduced;
    numerator -= f64::from_bits(0x3fdc_7b29_7e26_9eac);
    denominator += f64::from_bits(0x3ff5_d6b1_2001_f228);
    numerator *= reduced;
    numerator += f64::from_bits(0x3fcd_1e41_8002_9834);
    numerator *= reduced;
    numerator / denominator
}

fn unit_arc_cosine(value: f64) -> f64 {
    let reduced = (1.0 - value) * 0.5;
    let root = reduced.sqrt();
    let ratio = arc_ratio(reduced);
    let truncated = f64::from_bits(root.to_bits() & 0xffff_ffff_0000_0000);
    let mut result = (root + root) * ratio;
    let correction = (reduced - truncated * truncated) / (truncated + root);
    result += correction + correction;
    result + truncated * 2.0
}

fn unit_arc_sine(value: f64) -> Result<f64, OrientationError> {
    let absolute = value.abs();
    if !value.is_finite() {
        return Err(OrientationError::NonFinite);
    }
    if absolute > 1.0 {
        return Err(OrientationError::UnsupportedAngle);
    }
    if absolute == 1.0 {
        return Ok(std::f64::consts::FRAC_PI_2.copysign(value));
    }
    if absolute < f64::from_bits(0x3e30_0000_0000_0000) {
        return Ok(value);
    }
    let result = if absolute < 0.5 {
        let ratio = arc_ratio(absolute * absolute);
        ratio * absolute + absolute
    } else {
        let reduced = (1.0 - absolute) * 0.5;
        let root = reduced.sqrt();
        let truncated = f64::from_bits(root.to_bits() & 0xffff_ffff_0000_0000);
        let ratio = arc_ratio(reduced);
        let correction = (reduced - truncated * truncated) / (truncated + root);
        let low = f64::from_bits(0x3c91_a626_3314_5c07) - (correction + correction);
        let high = std::f64::consts::FRAC_PI_4 - truncated * 2.0;
        std::f64::consts::FRAC_PI_4 - ((root * 2.0 * ratio - low) - high)
    };
    Ok(result.copysign(value))
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_angle_getter_preserves_roll_origin_and_vertical_branch() {
        let identity = super::CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(identity.source_angles().unwrap(), [0.0, 0.0, 360.0]);
        assert_eq!(
            super::CoreOrientation {
                quaternion: [1.0, 0.0, 0.0, 0.0]
            }
            .source_angles()
            .unwrap(),
            [0.0, 0.0, 180.0]
        );
        let half = 0.5_f64.sqrt();
        assert_eq!(
            super::CoreOrientation {
                quaternion: [0.0, 0.0, half, half]
            }
            .source_angles()
            .unwrap(),
            [90.0, 180.0, 180.0]
        );
        assert_eq!(
            super::CoreOrientation {
                quaternion: [f64::NAN, 0.0, 0.0, 1.0]
            }
            .source_angles(),
            Err(super::OrientationError::NonFinite)
        );
    }
    #[test]
    fn angular_envelope_preserves_increment_axis_speed_and_deviation_bits() {
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let envelope = super::RotationEnvelope::from_motion(
            [0xa9ae_52f8, 0x274f_2ec9, 0x3fb5_4f27].map(f32::from_bits),
            f32::from_bits(0x3c6c_ad58),
            identity,
            f32::from_bits(0x3d04_15bc),
        )
        .unwrap();
        assert_eq!(
            envelope.world_axis.map(f32::to_bits),
            [0xa976_2445, 0x2712_44c8, 0x3f80_0000]
        );
        assert_eq!(envelope.angular_speed.to_bits(), 0x3fb5_4f28);
        assert_eq!(envelope.surface_speed.to_bits(), 0x3d3b_186b);
        let quiet =
            super::RotationEnvelope::from_motion([1.0e-10; 3], 0.015, identity, 0.1).unwrap();
        assert_eq!(quiet.world_axis, [1.0, 0.0, 0.0]);
        assert_eq!(quiet.angular_speed.to_bits(), 0);
        assert_eq!(quiet.surface_speed.to_bits(), 0);
        assert_eq!(
            super::RotationEnvelope::from_motion([0.0; 3], 0.015, identity, -1.0),
            Err(super::OrientationError::NegativeDeviationRadius)
        );
        assert_eq!(
            super::RotationEnvelope::from_motion([0.0; 3], 0.015, [f64::NAN; 9], 0.1),
            Err(super::OrientationError::NonFinite)
        );
    }
    use super::{CoreOrientation, OrientationError, SourceAngleBasis};

    #[test]
    fn source_yaw_and_core_rebase_preserve_target_binary_widths() {
        let basis = SourceAngleBasis::from_degrees([0.0, 90.0, 0.0]).unwrap();
        assert_eq!(
            basis.matrix.map(f32::to_bits),
            [
                0xb3bb_bd2d,
                0xbf7f_ffff,
                0x0000_0000,
                0x3f7f_fffe,
                0xb3bb_bd2e,
                0x0000_0000,
                0x8000_0000,
                0x0000_0000,
                0x3f7f_ffff,
            ]
        );
        assert_eq!(
            basis
                .core_orientation()
                .unwrap()
                .quaternion
                .map(f64::to_bits),
            [0, 0xbfe6_a09e_6bc6_f8f2, 0, 0x3fe6_a09e_6137_7ea7]
        );
    }

    #[test]
    fn quaternion_interpolation_uses_target_arccos_and_sine_polynomials() {
        let start = CoreOrientation {
            quaternion: [
                f64::from_bits(0xbe96_3754_ebf6_1509),
                f64::from_bits(0xbea5_3618_2c8b_5a4e),
                f64::from_bits(0xbfe4_f93a_48e5_c611),
                f64::from_bits(0xbfe8_2b1a_abc2_d4c8),
            ],
        };
        let end = CoreOrientation {
            quaternion: [
                f64::from_bits(0xbe97_045b_c4ab_e2a4),
                f64::from_bits(0xbea4_bee2_0ad7_b336),
                f64::from_bits(0xbfe7_65fe_22da_bbc9),
                f64::from_bits(0xbfe5_d447_2c47_c1e3),
            ],
        };
        assert_eq!(
            start
                .interpolate(end, f64::from_bits(0x3fe2_c372_c000_0000))
                .unwrap()
                .quaternion
                .map(f64::to_bits),
            [
                0xbe96_b75b_d02a_b63e,
                0xbea4_f76c_8e7d_ee01,
                0xbfe6_6ce4_d3be_090e,
                0xbfe6_d3e2_c68b_6346,
            ]
        );
    }

    #[test]
    fn body_local_angular_integration_preserves_target_binary64_quaternion() {
        let start = CoreOrientation {
            quaternion: [
                f64::from_bits(0xbfde_d318_ef83_7938),
                f64::from_bits(0xbfe0_911e_166a_c15c),
                f64::from_bits(0x3fde_d319_0ebc_1c06),
                f64::from_bits(0x3fe0_911e_00c1_0b33),
            ],
        };
        let next = start
            .advance(
                [
                    f32::from_bits(0x3545_b177),
                    f32::from_bits(0xb582_2c62),
                    f32::from_bits(0x4197_9ee4),
                ],
                f32::from_bits(0x3c75_c28f),
            )
            .unwrap();
        assert_eq!(
            next.quaternion.map(f64::to_bits),
            [
                0xbfe1_9a94_25dc_f71a,
                0xbfdc_6ec9_261c_8230,
                0x3fe1_9a94_3914_62df,
                0x3fdc_6ec8_f46c_e015,
            ]
        );
    }

    #[test]
    fn predicted_angular_speed_uses_exact_rotation_upper_bound_polynomial() {
        assert_eq!(
            CoreOrientation::predicted_angular_speed(
                [0.0, 0.0, f32::from_bits(0x418b_6bb8)],
                f32::from_bits(0x3c75_c28f),
            )
            .unwrap()
            .to_bits(),
            0x418b_6f18
        );
    }

    #[test]
    fn orientation_rejects_nonfinite_and_unestablished_domains() {
        assert_eq!(
            SourceAngleBasis::from_degrees([f32::NAN, 0.0, 0.0]),
            Err(OrientationError::NonFinite)
        );
        assert_eq!(
            SourceAngleBasis::from_degrees([1_000_000.0, 0.0, 0.0]),
            Err(OrientationError::UnsupportedAngle)
        );
        let identity = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(
            identity.interpolate(identity, f64::NAN),
            Err(OrientationError::NonFinite)
        );
    }
}
