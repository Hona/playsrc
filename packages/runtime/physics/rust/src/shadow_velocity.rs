use crate::MotionError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowVelocityInput {
    pub velocity: [f32; 3],
    pub displacement: [f32; 3],
    pub maximum_speed: f32,
    pub maximum_damping_speed: f32,
    pub scale: f32,
    pub damping: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowVelocityOutput {
    pub velocity: [f32; 3],
    pub impulse: [f32; 3],
}
impl ShadowVelocityInput {
    pub fn solve(self) -> Result<ShadowVelocityOutput, MotionError> {
        if self
            .velocity
            .iter()
            .chain(&self.displacement)
            .chain([
                &self.maximum_speed,
                &self.maximum_damping_speed,
                &self.scale,
                &self.damping,
            ])
            .any(|v| !v.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        let velocity = if f64::from(squared(self.velocity)) < 1.0e-6 {
            [0.0; 3]
        } else {
            self.velocity
        };
        let impulse = bounded(self.displacement, self.scale, self.maximum_speed);
        let damping = bounded(velocity, -self.damping, self.maximum_damping_speed);
        let next =
            std::array::from_fn::<_, 3, _>(|axis| (velocity[axis] + damping[axis]) + impulse[axis]);
        if next.iter().chain(&impulse).any(|v| !v.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        Ok(ShadowVelocityOutput {
            velocity: next,
            impulse,
        })
    }
}
fn squared(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1]) + value[2] * value[2]
}
fn bounded(value: [f32; 3], scale: f32, maximum: f32) -> [f32; 3] {
    if maximum <= 0.0 {
        return [0.0; 3];
    }
    let scaled = value.map(|v| (f64::from(v) * f64::from(scale)) as f32);
    let length = f64::from(squared(scaled)).sqrt() as f32;
    if length > maximum {
        let factor = f64::from(maximum / length);
        scaled.map(|v| (f64::from(v) * factor) as f32)
    } else {
        scaled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_drive_and_damping_limits_preserve_velocity_accumulation() {
        let input = ShadowVelocityInput {
            velocity: [3.0, 4.0, 0.0],
            displacement: [3.0, 0.0, 0.0],
            maximum_speed: 1.0,
            maximum_damping_speed: 2.0,
            scale: 1.0,
            damping: 1.0,
        };
        let output = input.solve().unwrap();
        assert_eq!(output.impulse, [1.0, 0.0, 0.0]);
        assert_eq!(output.velocity, [(3.0_f32 - 1.2) + 1.0, 4.0 - 1.6, 0.0]);
        assert_eq!(
            ShadowVelocityInput {
                velocity: [0.0001, 0.0, 0.0],
                maximum_speed: 0.0,
                maximum_damping_speed: 0.0,
                ..input
            }
            .solve()
            .unwrap()
            .velocity,
            [0.0; 3]
        );
        assert_eq!(
            ShadowVelocityInput {
                scale: f32::NAN,
                ..input
            }
            .solve(),
            Err(MotionError::NonFinite)
        );
    }
}
