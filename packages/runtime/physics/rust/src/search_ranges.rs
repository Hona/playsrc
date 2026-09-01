use crate::{CollisionMotion, ContinuousError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionSearchRanges {
    timestep: f32,
    world_lookahead: f32,
    pair_lookahead: f32,
}

impl CollisionSearchRanges {
    pub fn new(
        timestep: f32,
        world_lookahead: f32,
        pair_lookahead: f32,
    ) -> Result<Self, ContinuousError> {
        if [timestep, world_lookahead, pair_lookahead]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        if timestep <= 0.0 || world_lookahead < 0.0 || pair_lookahead < 0.0 {
            return Err(ContinuousError::InvalidInterval);
        }
        Ok(Self {
            timestep,
            world_lookahead,
            pair_lookahead,
        })
    }

    pub fn world(self, motion: CollisionMotion, radius: f32) -> Result<f64, ContinuousError> {
        let speed = search_speed(motion, radius)?;
        let radius = f64::from(radius);
        let distance = (speed * f64::from(self.world_lookahead))
            .min(radius * 5.0)
            .clamp(0.5, 15.0);
        Ok((distance - speed * f64::from(self.timestep)).max(radius + speed * f64::from(0.06_f32)))
    }

    pub fn pair(
        self,
        motion: [CollisionMotion; 2],
        radii: [f32; 2],
    ) -> Result<[f64; 2], ContinuousError> {
        let mut speed = [
            search_speed(motion[0], radii[0])?,
            search_speed(motion[1], radii[1])?,
        ];
        let sum = speed[0] + speed[1];
        let radius = f64::from(radii[0].min(radii[1]));
        let capped = (radius * f64::from(0.9_f32))
            .min(sum * f64::from(self.pair_lookahead))
            .clamp(f64::from(0.8_f32), 10.0);
        let distance = (capped - sum * f64::from(self.timestep)).max(sum * f64::from(0.06_f32));
        speed[0] += speed[1] * f64::from(0.2_f32);
        speed[1] += speed[0] * f64::from(0.18_f32);
        let inverse_sum = 1.0 / (speed[1] + speed[0]);
        Ok(speed.map(|speed| (distance * speed) * inverse_sum))
    }
}

fn search_speed(motion: CollisionMotion, radius: f32) -> Result<f64, ContinuousError> {
    motion.validate()?;
    if !radius.is_finite() {
        return Err(ContinuousError::NonFinite);
    }
    if radius < 0.0 {
        return Err(ContinuousError::InvalidEventSpeed);
    }
    let sum = motion.linear_speed + motion.rotation.surface_speed;
    if !sum.is_finite() {
        return Err(ContinuousError::NonFinite);
    }
    Ok(f64::from(sum) + 1.0e-19)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_search_includes_radius_in_its_time_floor() {
        let ranges = CollisionSearchRanges::new(0.015, 1.0, 0.5).unwrap();
        let mut motion = CollisionMotion::stationary();
        assert_eq!(ranges.world(motion, 1.0).unwrap(), 1.0);
        motion.linear_speed = 500.0;
        assert_eq!(
            ranges.world(motion, 1.0).unwrap(),
            1.0 + 500.0 * f64::from(0.06_f32)
        );
    }

    #[test]
    fn paired_range_sharing_is_ordered_and_quiet_bodies_keep_nonzero_ranges() {
        let ranges = CollisionSearchRanges::new(0.015, 1.0, 0.5).unwrap();
        let quiet = [CollisionMotion::stationary(); 2];
        let output = ranges.pair(quiet, [0.1; 2]).unwrap();
        assert!(output[0] > 0.0 && output[1] > output[0]);
        assert!(((output[0] + output[1]) - f64::from(0.8_f32)).abs() < 1.0e-15);
        let mut fast = CollisionMotion::stationary();
        fast.linear_speed = 10.0;
        let forward = ranges.pair([fast, quiet[0]], [0.1; 2]).unwrap();
        let reversed = ranges.pair([quiet[0], fast], [0.1; 2]).unwrap();
        assert_ne!(
            forward.map(f64::to_bits),
            [reversed[1], reversed[0]].map(f64::to_bits)
        );
        assert!(ranges.pair(quiet, [f32::NAN, 1.0]).is_err());
        assert!(CollisionSearchRanges::new(0.0, 1.0, 0.5).is_err());
    }
}
