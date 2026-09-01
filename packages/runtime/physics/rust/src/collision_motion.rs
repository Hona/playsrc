use crate::{ContinuousError, OrientationError, RotationEnvelope, VelocityState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionMotion {
    pub linear_velocity: [f32; 3],
    pub linear_speed: f32,
    pub rotation: RotationEnvelope,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionMotionBounds {
    pub approaching_speed: f64,
    pub maximum_deviation: f64,
    pub worst_case_speed: f64,
    pub maximum_angular_deviation: f64,
}

impl CollisionMotion {
    pub fn from_velocity(
        velocity: VelocityState,
        timestep: f32,
        retained_basis: [f64; 9],
        deviation_radius: f32,
    ) -> Result<Self, OrientationError> {
        let rotation = RotationEnvelope::from_motion(
            velocity.angular,
            timestep,
            retained_basis,
            deviation_radius,
        )?;
        Self::from_rotation(velocity.linear, rotation)
    }
    pub fn from_delta(
        linear_velocity: [f32; 3],
        delta: crate::CoreOrientation,
        timestep: f32,
        retained_basis: [f64; 9],
        deviation_radius: f32,
    ) -> Result<Self, OrientationError> {
        Self::from_rotation(
            linear_velocity,
            RotationEnvelope::from_delta(delta, timestep, retained_basis, deviation_radius)?,
        )
    }
    fn from_rotation(
        linear_velocity: [f32; 3],
        rotation: RotationEnvelope,
    ) -> Result<Self, OrientationError> {
        let squared = dot(linear_velocity, linear_velocity);
        let linear_speed = f64::from(squared).sqrt() as f32;
        if !linear_speed.is_finite() || linear_velocity.iter().any(|value| !value.is_finite()) {
            return Err(OrientationError::NonFinite);
        }
        Ok(Self {
            linear_velocity,
            linear_speed,
            rotation,
        })
    }

    pub const fn stationary() -> Self {
        Self {
            linear_velocity: [0.0; 3],
            linear_speed: 0.0,
            rotation: RotationEnvelope {
                world_axis: [1.0, 0.0, 0.0],
                angular_speed: 0.0,
                surface_speed: 0.0,
            },
        }
    }

    pub(crate) fn combined_speed(endpoints: [Self; 2]) -> f64 {
        let surface = endpoints[0].rotation.surface_speed + endpoints[1].rotation.surface_speed;
        (f64::from(surface) + f64::from(endpoints[0].linear_speed))
            + f64::from(endpoints[1].linear_speed)
    }

    pub(crate) fn validate(self) -> Result<(), ContinuousError> {
        if self
            .linear_velocity
            .iter()
            .chain(self.rotation.world_axis.iter())
            .any(|value| !value.is_finite())
            || [
                self.linear_speed,
                self.rotation.angular_speed,
                self.rotation.surface_speed,
            ]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        if self.linear_speed < 0.0
            || self.rotation.angular_speed < 0.0
            || self.rotation.surface_speed < 0.0
        {
            return Err(ContinuousError::InvalidEventSpeed);
        }
        Ok(())
    }
}

impl CollisionMotionBounds {
    /// The normal points away from the second endpoint toward the first endpoint.
    pub fn from_endpoints(
        endpoints: [CollisionMotion; 2],
        normal: [f32; 3],
    ) -> Result<Self, ContinuousError> {
        for endpoint in endpoints {
            endpoint.validate()?;
        }
        if normal.iter().any(|value| !value.is_finite()) {
            return Err(ContinuousError::NonFinite);
        }
        if normal == [0.0; 3] {
            return Err(ContinuousError::InvalidBracket);
        }
        let mut rotational = [0.0; 2];
        for (result, endpoint) in rotational.iter_mut().zip(endpoints) {
            let projection = f64::from(dot(normal, endpoint.rotation.world_axis));
            let perpendicular = f64::from(1.001_f32) - projection * projection;
            if perpendicular < 0.0 {
                return Err(ContinuousError::InvalidBracket);
            }
            *result = perpendicular.sqrt() * f64::from(endpoint.rotation.surface_speed);
        }
        let approaching_speed = f64::from(dot(normal, endpoints[1].linear_velocity))
            - f64::from(dot(normal, endpoints[0].linear_velocity));
        let maximum_deviation = (rotational[0] + rotational[1]) + approaching_speed;
        let worst_case_speed = CollisionMotion::combined_speed(endpoints);
        let angular_sum = endpoints[0].rotation.angular_speed + endpoints[1].rotation.angular_speed;
        let maximum_angular_deviation = f64::from(angular_sum) + 1.0e-19;
        if [
            maximum_deviation,
            approaching_speed,
            worst_case_speed,
            maximum_angular_deviation,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        Ok(Self {
            approaching_speed,
            maximum_deviation,
            worst_case_speed,
            maximum_angular_deviation,
        })
    }

    pub fn admits(
        self,
        distance: f64,
        collision_distance: f64,
        start: f64,
        end: f64,
    ) -> Result<bool, ContinuousError> {
        if [
            distance,
            collision_distance,
            start,
            end,
            self.maximum_deviation,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        if start > end {
            return Err(ContinuousError::InvalidInterval);
        }
        let elapsed = (end - start) as f32;
        let reach = collision_distance + f64::from(elapsed) * self.maximum_deviation;
        if !reach.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        Ok(self.maximum_deviation >= 1.0e-19 && distance < reach)
    }
}

fn dot(first: [f32; 3], second: [f32; 3]) -> f32 {
    (first[1] * second[1] + first[0] * second[0]) + first[2] * second[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approach_subtracts_widened_projections_and_does_not_round_the_difference_to_float() {
        let mut first = CollisionMotion::stationary();
        let mut second = first;
        first.linear_velocity = [1.0, 0.0, 0.0];
        second.linear_velocity = [f32::from_bits(0x3300_0000), 0.0, 0.0];
        let pair = CollisionMotionBounds::from_endpoints([first, second], [1.0, 0.0, 0.0]).unwrap();
        assert_eq!(
            pair.approaching_speed,
            f64::from(second.linear_velocity[0]) - 1.0
        );
        assert_ne!(
            pair.approaching_speed,
            f64::from(pair.approaching_speed as f32)
        );
        assert_eq!(pair.maximum_deviation, pair.approaching_speed);
        assert!(!pair.admits(0.1, 0.01, 0.0, 0.015).unwrap());
    }

    #[test]
    fn admission_uses_float_remaining_time_and_excludes_the_exact_reach_boundary() {
        let mut moving = CollisionMotion::stationary();
        moving.linear_velocity = [0.0, 1.0, 0.0];
        moving.linear_speed = 1.0;
        let pair = CollisionMotionBounds::from_endpoints(
            [moving, CollisionMotion::stationary()],
            [0.0, -1.0, 0.0],
        )
        .unwrap();
        let end = 0.015_f64;
        let reach = 0.01 + f64::from(end as f32);
        assert!(!pair.admits(reach, 0.01, 0.0, end).unwrap());
        assert!(
            pair.admits(f64::from_bits(reach.to_bits() - 1), 0.01, 0.0, end)
                .unwrap()
        );
        assert_eq!(
            pair.admits(0.0, 0.01, 1.0, 0.0),
            Err(ContinuousError::InvalidInterval)
        );
        assert_eq!(
            CollisionMotionBounds::from_endpoints([moving, moving], [0.0; 3]),
            Err(ContinuousError::InvalidBracket)
        );
    }
}
