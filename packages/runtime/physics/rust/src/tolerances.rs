use crate::{
    MotionError,
    units::{METERS_PER_INCH, internal_direction},
};

/// Configured contact distances and speeds in internal meters and seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactTolerances {
    pub real_surface: f32,
    pub collision_distance: f32,
    pub friction_distance: f32,
    pub keeper_distance: f32,
    pub keeper_speed: f32,
    pub keeper_safety: f32,
    pub maximum_friction_distance: f32,
    pub maximum_impact_distance: f32,
    pub feature_change_distance: f32,
    pub minimum_separating_speed: f32,
}

impl ContactTolerances {
    /// Gravity is supplied in Source XYZ inches per second squared.
    pub fn from_gravity(gravity: [f32; 3]) -> Result<Self, MotionError> {
        if gravity.iter().any(|value| !value.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        let gravity = internal_direction(gravity, METERS_PER_INCH).map(f64::from);
        let squared = (gravity[0] * gravity[0] + gravity[1] * gravity[1]) + gravity[2] * gravity[2];
        if !squared.is_finite() {
            return Err(MotionError::NonFinite);
        }
        let gravity_length = squared.sqrt();
        let tolerance = f64::from((0.25_f32 - 1.0e-4_f32) * METERS_PER_INCH);
        let real_surface = (f64::from(0.1_f32) * tolerance) as f32;
        let collision_distance = (f64::from(real_surface) + f64::from(0.9_f32) * tolerance) as f32;
        let friction_distance = (f64::from(collision_distance) + tolerance) as f32;
        let keeper_distance =
            (f64::from(friction_distance) + f64::from(0.3_f32) * tolerance) as f32;
        let fall_distance = keeper_distance - collision_distance;
        let keeper_speed =
            (f64::from(fall_distance + fall_distance) * gravity_length).sqrt() as f32;
        Ok(Self {
            real_surface,
            collision_distance,
            friction_distance,
            keeper_distance,
            keeper_speed,
            keeper_safety: (f64::from(0.01_f32) * tolerance) as f32,
            maximum_friction_distance: (f64::from(friction_distance) + 2.5 * tolerance) as f32,
            maximum_impact_distance: (f64::from(friction_distance) + 20.0 * tolerance) as f32,
            feature_change_distance: collision_distance * 0.1_f32,
            minimum_separating_speed: (tolerance + tolerance) as f32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_contact_thresholds_retain_gravity_length_precision() {
        let zero = ContactTolerances::from_gravity([0.0; 3]).unwrap();
        let downward = ContactTolerances::from_gravity([0.0, 0.0, -800.0]).unwrap();
        assert_eq!(downward.real_surface.to_bits(), 0x3a26_6515);
        assert_eq!(downward.collision_distance.to_bits(), 0x3bcf_fe5a);
        assert_eq!(downward.friction_distance.to_bits(), 0x3c4f_fe5a);
        assert_eq!(downward.keeper_distance.to_bits(), 0x3c6f_314e);
        assert_eq!(downward.keeper_speed.to_bits(), 0x3f14_3f75);
        assert_eq!(
            ContactTolerances {
                keeper_speed: 0.0,
                ..downward
            },
            zero
        );
        assert_eq!(
            ContactTolerances::from_gravity([f32::NAN, 0.0, 0.0]),
            Err(MotionError::NonFinite)
        );
    }
}
