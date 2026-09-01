use crate::{
    AngularVelocityLimit, CoreOrientation, OrientationError, QueuedVelocity, ResponseError,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MotionError {
    NonFinite,
    NonPositiveTimestep,
    NegativeCoefficient,
    NonPositiveVelocityLimit,
    EmptyGroup,
    Orientation(OrientationError),
    Response(ResponseError),
}

impl fmt::Display for MotionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => formatter.write_str("physical motion contains a non-finite value"),
            Self::NonPositiveTimestep => formatter.write_str("physical timestep must be positive"),
            Self::NegativeCoefficient => {
                formatter.write_str("physical damping, density, and drag cannot be negative")
            }
            Self::NonPositiveVelocityLimit => {
                formatter.write_str("physical velocity limits must be positive")
            }
            Self::EmptyGroup => {
                formatter.write_str("motion activity requires at least one active body")
            }
            Self::Orientation(error) => error.fmt(formatter),
            Self::Response(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for MotionError {}

impl From<OrientationError> for MotionError {
    fn from(error: OrientationError) -> Self {
        Self::Orientation(error)
    }
}

impl From<ResponseError> for MotionError {
    fn from(error: ResponseError) -> Self {
        Self::Response(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocityState {
    pub linear: [f32; 3],
    pub angular: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AerodynamicFactors {
    pub linear: Option<f32>,
    pub angular: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionProfile {
    pub timestep: f32,
    pub gravity: [f32; 3],
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub settling: bool,
    pub drag: f32,
    pub air_density: f32,
    pub linear_drag_basis: [f32; 3],
    pub angular_drag_basis: [f32; 3],
    pub maximum_linear_speed: f32,
    pub maximum_angular_per_step: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedMotionClock {
    pub timestep: f32,
}

impl MotionProfile {
    fn validate(self, velocity: VelocityState) -> Result<(), MotionError> {
        if !self.timestep.is_finite()
            || !self.linear_damping.is_finite()
            || !self.angular_damping.is_finite()
            || !self.drag.is_finite()
            || !self.air_density.is_finite()
            || !self.maximum_linear_speed.is_finite()
            || !self.maximum_angular_per_step.is_finite()
            || self
                .gravity
                .iter()
                .chain(self.linear_drag_basis.iter())
                .chain(self.angular_drag_basis.iter())
                .chain(velocity.linear.iter())
                .chain(velocity.angular.iter())
                .any(|component| !component.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(MotionError::NonPositiveTimestep);
        }
        if self.linear_damping < 0.0
            || self.angular_damping < 0.0
            || self.drag < 0.0
            || self.air_density < 0.0
            || self
                .linear_drag_basis
                .iter()
                .chain(self.angular_drag_basis.iter())
                .any(|component| *component < 0.0)
        {
            return Err(MotionError::NegativeCoefficient);
        }
        if self.maximum_linear_speed <= 0.0 || self.maximum_angular_per_step <= 0.0 {
            return Err(MotionError::NonPositiveVelocityLimit);
        }
        Ok(())
    }

    /// Applies damping, commits queued impulses, then adds gravity.
    /// Pinned bodies leave active and pending velocities unchanged.
    pub fn accelerate(
        self,
        mut velocity: VelocityState,
        queued: &mut QueuedVelocity,
        pinned: bool,
    ) -> Result<VelocityState, MotionError> {
        if pinned {
            return Ok(velocity);
        }
        self.validate(velocity)?;
        if queued
            .linear
            .iter()
            .chain(queued.angular.iter())
            .any(|value| !value.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        let (linear_damping, angular_damping) = if self.settling {
            (
                self.linear_damping + 0.1_f32,
                self.angular_damping + 0.1_f32,
            )
        } else {
            (self.linear_damping, self.angular_damping)
        };
        let linear_adjustment = f64::from(linear_damping) * f64::from(self.timestep);
        let linear_factor = if linear_adjustment < 0.25 {
            1.0 - linear_adjustment
        } else {
            (-linear_adjustment).exp()
        };
        for component in &mut velocity.linear {
            *component = (f64::from(*component) * linear_factor) as f32;
        }
        let angular_adjustment = angular_damping * self.timestep;
        let angular_squared = angular_adjustment * angular_adjustment;
        let total_angular_squared = (angular_squared + angular_squared) + angular_squared;
        let angular_factor = if total_angular_squared < 0.5 {
            1.0 - angular_adjustment
        } else {
            (-angular_adjustment).exp()
        };
        for component in &mut velocity.angular {
            *component *= angular_factor;
        }
        for axis in 0..3 {
            velocity.angular[axis] += queued.angular[axis];
            velocity.linear[axis] += queued.linear[axis];
        }
        for (component, gravity) in velocity.linear.iter_mut().zip(self.gravity) {
            *component =
                (f64::from(*component) + f64::from(gravity) * f64::from(self.timestep)) as f32;
        }
        if velocity
            .linear
            .iter()
            .chain(velocity.angular.iter())
            .any(|value| !value.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        *queued = QueuedVelocity::default();
        Ok(velocity)
    }

    pub fn aerodynamic_factors(
        self,
        velocity: VelocityState,
        orientation: CoreOrientation,
    ) -> Result<AerodynamicFactors, MotionError> {
        self.validate(velocity)?;
        if orientation
            .quaternion
            .iter()
            .any(|component| !component.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        if self.air_density == 0.0 {
            return Ok(AerodynamicFactors {
                linear: None,
                angular: None,
            });
        }
        let basis = orientation.matrix();
        let linear = velocity.linear.map(f64::from);
        let local_linear = [
            ((basis[3] * linear[1] + basis[0] * linear[0]) + basis[6] * linear[2]) as f32,
            ((basis[1] * linear[0] + basis[4] * linear[1]) + basis[7] * linear[2]) as f32,
            ((basis[2] * linear[0] + basis[5] * linear[1]) + basis[8] * linear[2]) as f32,
        ];
        let linear_projection = self.drag * (local_linear[0] * self.linear_drag_basis[0]).abs()
            + (local_linear[1] * self.linear_drag_basis[1]).abs()
            + (local_linear[2] * self.linear_drag_basis[2]).abs();
        let angular_projection = self.drag
            * (velocity.angular[0] * self.angular_drag_basis[0]).abs()
            + (velocity.angular[1] * self.angular_drag_basis[1]).abs()
            + (velocity.angular[2] * self.angular_drag_basis[2]).abs();
        let linear =
            ((self.timestep * self.air_density) * (linear_projection * -0.5_f32)).max(-1.0);
        let angular = (-((angular_projection * self.air_density) * self.timestep)).max(-1.0);
        Ok(AerodynamicFactors {
            linear: (linear < 0.0).then_some(linear),
            angular: (angular < 0.0).then_some(angular),
        })
    }

    pub fn apply_aerodynamics(
        self,
        mut velocity: VelocityState,
        factors: AerodynamicFactors,
    ) -> Result<VelocityState, MotionError> {
        self.validate(velocity)?;
        if factors
            .linear
            .into_iter()
            .chain(factors.angular)
            .any(|factor| !factor.is_finite() || !(-1.0..=0.0).contains(&factor))
        {
            return Err(MotionError::NonFinite);
        }
        for (components, factor) in [
            (&mut velocity.linear, factors.linear),
            (&mut velocity.angular, factors.angular),
        ] {
            if let Some(factor) = factor {
                for component in components {
                    *component += (f64::from(*component) * f64::from(factor)) as f32;
                }
            }
        }
        Ok(velocity)
    }

    pub fn constrain_velocity(self, velocity: VelocityState) -> Result<VelocityState, MotionError> {
        self.validate(velocity)?;
        Ok(crate::CollisionVelocityLimits {
            maximum_linear: self.maximum_linear_speed,
            angular: AngularVelocityLimit {
                maximum_per_step: self.maximum_angular_per_step,
                timestep: self.timestep,
            },
        }
        .apply(velocity)?)
    }

    pub fn advance_velocity(
        self,
        velocity: VelocityState,
        orientation: CoreOrientation,
    ) -> Result<VelocityState, MotionError> {
        let accelerated = self.accelerate(velocity, &mut QueuedVelocity::default(), false)?;
        let factors = self.aerodynamic_factors(accelerated, orientation)?;
        self.constrain_velocity(self.apply_aerodynamics(accelerated, factors)?)
    }
}

impl FixedMotionClock {
    fn validate(self) -> Result<(), MotionError> {
        if !self.timestep.is_finite() {
            return Err(MotionError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(MotionError::NonPositiveTimestep);
        }
        Ok(())
    }

    pub fn elapsed(self) -> Result<f32, MotionError> {
        self.validate()?;
        let first_boundary = self.timestep * crate::clock::FRAME_LOOKAHEAD;
        Ok((f64::from(first_boundary) - f64::from(self.timestep)) as f32)
    }

    pub fn fraction(self) -> Result<f32, MotionError> {
        Ok(self.elapsed()? * (1.0 / f64::from(self.timestep)) as f32)
    }

    pub fn position(self, previous: [f64; 3], velocity: [f32; 3]) -> Result<[f64; 3], MotionError> {
        if previous.iter().any(|component| !component.is_finite())
            || velocity.iter().any(|component| !component.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        let elapsed = f64::from(self.elapsed()?);
        Ok(std::array::from_fn(|axis| {
            previous[axis] + f64::from(velocity[axis]) * elapsed
        }))
    }

    pub fn orientation(
        self,
        previous: CoreOrientation,
        current: CoreOrientation,
    ) -> Result<CoreOrientation, MotionError> {
        Ok(previous.interpolate(current, f64::from(self.fraction()?))?)
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedMotionClock, MotionError, MotionProfile, VelocityState};
    use crate::CoreOrientation;

    fn profile() -> MotionProfile {
        MotionProfile {
            timestep: f32::from_bits(0x3c75_c28f),
            gravity: [0.0; 3],
            linear_damping: 0.0,
            angular_damping: 0.0,
            settling: false,
            drag: 1.0,
            air_density: 0.0,
            linear_drag_basis: [
                f32::from_bits(0x3bf5_7a33),
                f32::from_bits(0x3bf5_7a33),
                f32::from_bits(0x3bef_6422),
            ],
            angular_drag_basis: [
                f32::from_bits(0x3b35_fe26),
                f32::from_bits(0x3b3a_4908),
                f32::from_bits(0x3b38_46f6),
            ],
            maximum_linear_speed: 50.8,
            maximum_angular_per_step: f32::from_bits(0x3f73_b620),
        }
    }

    #[test]
    fn fixed_step_publication_retains_target_elapsed_and_fraction_widths() {
        let ordinary = FixedMotionClock {
            timestep: f32::from_bits(0x3c75_c28f),
        };
        assert_eq!(ordinary.elapsed().unwrap().to_bits(), 0x3c75_c1ed);
        assert_eq!(ordinary.fraction().unwrap().to_bits(), 0x3f7f_ff58);
        let large = FixedMotionClock {
            timestep: f32::from_bits(0x3dcc_cccd),
        };
        assert_eq!(large.elapsed().unwrap().to_bits(), 0x3dcc_cc47);
        assert_eq!(large.fraction().unwrap().to_bits(), 0x3f7f_ff59);
    }

    #[test]
    fn linear_damping_keeps_binary64_factor_until_final_component_rounding() {
        let mut settings = profile();
        settings.linear_damping = 10.0;
        let velocity = settings
            .accelerate(
                VelocityState {
                    linear: [
                        f32::from_bits(0x4022_8f5c),
                        f32::from_bits(0xc0f3_d70a),
                        f32::from_bits(0xc0a2_8f5c),
                    ],
                    angular: [0.0; 3],
                },
                &mut crate::QueuedVelocity::default(),
                false,
            )
            .unwrap();
        assert_eq!(
            velocity.linear.map(f32::to_bits),
            [0x400a_2d0e, 0xc0cf_4395, 0xc08a_2d0e]
        );
    }

    #[test]
    fn settling_adds_binary32_damping_before_gravity_with_distinct_linear_and_angular_widths() {
        let settings = MotionProfile {
            settling: true,
            gravity: [0.0, 800.0 * 0.0254, 0.0],
            ..profile()
        };
        let result = settings
            .accelerate(
                VelocityState {
                    angular: [2932275811, 0, 0].map(f32::from_bits),
                    linear: [0, 3144712400, 756822127].map(f32::from_bits),
                },
                &mut crate::QueuedVelocity::default(),
                false,
            )
            .unwrap();
        assert_eq!(result.angular.map(f32::to_bits), [2932256251, 0, 0]);
        assert_eq!(result.linear.map(f32::to_bits), [0, 1050291830, 756806773]);
    }

    #[test]
    fn controller_impulses_are_committed_after_damping_and_before_gravity() {
        let settings = MotionProfile {
            timestep: 0.1,
            linear_damping: 0.5,
            angular_damping: 0.5,
            gravity: [0.0, 10.0, 0.0],
            ..profile()
        };
        let mut queued = crate::QueuedVelocity {
            linear: [2.0, 3.0, 4.0],
            angular: [5.0, 6.0, 7.0],
        };
        let result = settings
            .accelerate(
                VelocityState {
                    linear: [1.0; 3],
                    angular: [1.0; 3],
                },
                &mut queued,
                false,
            )
            .unwrap();
        assert_eq!(
            result.linear,
            [2.95, f32::from_bits(4.95_f32.to_bits() + 1), 4.95]
        );
        assert_eq!(result.angular, [5.95, 6.95, 7.95]);
        assert_eq!(queued, crate::QueuedVelocity::default());
        let mut queued = crate::QueuedVelocity {
            linear: [2.0; 3],
            angular: [3.0; 3],
        };
        let before = queued;
        assert!(
            MotionProfile {
                timestep: 0.0,
                ..settings
            }
            .accelerate(result, &mut queued, false)
            .is_err()
        );
        assert_eq!(queued, before);
        assert_eq!(
            MotionProfile {
                timestep: 0.0,
                ..settings
            }
            .accelerate(result, &mut queued, true)
            .unwrap(),
            result
        );
        assert_eq!(queued, before);
    }

    #[test]
    fn mixed_axis_angular_clamp_uses_binary64_length_and_component_application() {
        let result = profile()
            .constrain_velocity(VelocityState {
                linear: [0.0; 3],
                angular: [
                    f32::from_bits(0x42fb_53d1),
                    f32::from_bits(0xc27b_53d1),
                    f32::from_bits(0xc2bc_7edd),
                ],
            })
            .unwrap();
        assert_eq!(
            result.angular.map(f32::to_bits),
            [0x4229_b5d6, 0xc1a9_b5d6, 0xc1fe_90c1]
        );
    }

    #[test]
    fn authored_aerodynamic_projection_preserves_configured_linear_drag() {
        let mut settings = profile();
        settings.air_density = 2.0;
        let velocity = settings
            .advance_velocity(
                VelocityState {
                    linear: [f32::from_bits(0x41cb_3333), 0.0, 0.0],
                    angular: [0.0; 3],
                },
                CoreOrientation {
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
            )
            .unwrap();
        assert_eq!(velocity.linear[0].to_bits(), 0x41ca_9eba);
    }

    #[test]
    fn motion_rejects_nonfinite_negative_and_nonpositive_contracts() {
        let zero = VelocityState {
            linear: [0.0; 3],
            angular: [0.0; 3],
        };
        let mut invalid = profile();
        invalid.gravity[0] = f32::NAN;
        assert_eq!(
            invalid.accelerate(zero, &mut crate::QueuedVelocity::default(), false),
            Err(MotionError::NonFinite)
        );
        invalid = profile();
        invalid.linear_damping = -1.0;
        assert_eq!(
            invalid.accelerate(zero, &mut crate::QueuedVelocity::default(), false),
            Err(MotionError::NegativeCoefficient)
        );
        invalid = profile();
        invalid.timestep = 0.0;
        assert_eq!(
            invalid.accelerate(zero, &mut crate::QueuedVelocity::default(), false),
            Err(MotionError::NonPositiveTimestep)
        );
        assert_eq!(
            FixedMotionClock { timestep: 0.0 }.fraction(),
            Err(MotionError::NonPositiveTimestep)
        );
    }
}
