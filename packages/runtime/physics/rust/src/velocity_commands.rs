use crate::{MotionError, QueuedVelocity, VelocityState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VelocityCommandLimits {
    pub linear_speed: f32,
    pub angular_per_step: f32,
    pub inverse_step: f32,
}

impl VelocityCommandLimits {
    pub fn apply(
        self,
        active: &mut VelocityState,
        queued: &mut QueuedVelocity,
    ) -> Result<(), MotionError> {
        if [self.linear_speed, self.angular_per_step, self.inverse_step]
            .iter()
            .any(|value| !value.is_finite())
            || active
                .linear
                .iter()
                .chain(active.angular.iter())
                .chain(queued.linear.iter())
                .chain(queued.angular.iter())
                .any(|value| !value.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        if self.linear_speed <= 0.0 || self.angular_per_step <= 0.0 || self.inverse_step <= 0.0 {
            return Err(MotionError::NonPositiveVelocityLimit);
        }
        let mut result = *active;
        let mut pending = *queued;
        clamp(&mut result.linear, self.linear_speed)?;
        clamp(&mut pending.linear, self.linear_speed)?;
        let maximum = self.inverse_step * self.angular_per_step;
        clamp(&mut result.angular, maximum)?;
        let linear_length = length(pending.linear)?;
        // Queued angular scaling uses the post-clamp queued linear magnitude.
        if linear_length > maximum {
            let factor = maximum / linear_length;
            pending.angular = pending
                .angular
                .map(|value| (f64::from(value) * f64::from(factor)) as f32);
        }
        *active = result;
        *queued = pending;
        Ok(())
    }
}

fn length(vector: [f32; 3]) -> Result<f32, MotionError> {
    let squared = (vector[0] * vector[0] + vector[1] * vector[1]) + vector[2] * vector[2];
    let length = f64::from(squared).sqrt() as f32;
    if !length.is_finite() {
        return Err(MotionError::NonFinite);
    }
    Ok(length)
}

fn clamp(vector: &mut [f32; 3], maximum: f32) -> Result<(), MotionError> {
    if !maximum.is_finite() {
        return Err(MotionError::NonFinite);
    }
    let magnitude = length(*vector)?;
    if magnitude > maximum {
        let factor = maximum / magnitude;
        *vector = vector.map(|value| (f64::from(value) * f64::from(factor)) as f32);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_limits_preserve_distinct_active_and_queued_angular_rules() {
        let limits = VelocityCommandLimits {
            linear_speed: 20.0,
            angular_per_step: 1.0,
            inverse_step: 10.0,
        };
        let mut active = VelocityState {
            linear: [4.0, 0.0, 0.0],
            angular: [0.0, 20.0, 0.0],
        };
        let mut queued = QueuedVelocity {
            linear: [40.0, 0.0, 0.0],
            angular: [0.0, 40.0, 0.0],
        };
        limits.apply(&mut active, &mut queued).unwrap();
        assert_eq!(active.angular, [0.0, 10.0, 0.0]);
        assert_eq!(queued.linear, [20.0, 0.0, 0.0]);
        assert_eq!(queued.angular, [0.0, 20.0, 0.0]);
        let before = (active, queued);
        assert_eq!(
            VelocityCommandLimits {
                angular_per_step: f32::MAX,
                inverse_step: f32::MAX,
                ..limits
            }
            .apply(&mut active, &mut queued),
            Err(MotionError::NonFinite)
        );
        assert_eq!((active, queued), before);
    }
}
