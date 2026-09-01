use crate::{ProjectionKnot, ShapeError, TopologyError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectFrame {
    shift: Option<[f32; 3]>,
}

impl ObjectFrame {
    pub fn place_core(
        origin: [f64; 3],
        orientation: [f64; 9],
        center: [f32; 3],
    ) -> Result<[f64; 3], ShapeError> {
        if origin
            .iter()
            .chain(orientation.iter())
            .any(|value| !value.is_finite())
            || center.iter().any(|value| !value.is_finite())
        {
            return Err(ShapeError::NonFinite);
        }
        let c = center.map(f64::from);
        let position = std::array::from_fn(|axis| {
            (orientation[axis * 3] * c[0] + orientation[axis * 3 + 2] * c[2])
                + (orientation[axis * 3 + 1] * c[1] + origin[axis])
        });
        if position.iter().any(|value| !value.is_finite()) {
            return Err(ShapeError::NonFinite);
        }
        Ok(position)
    }
    pub const fn identity() -> Self {
        Self { shift: None }
    }

    pub fn from_center(center: [f32; 3]) -> Result<Self, ShapeError> {
        if center.iter().any(|value| !value.is_finite()) {
            return Err(ShapeError::NonFinite);
        }
        let center64 = center.map(f64::from);
        let squared =
            (center64[0] * center64[0] + center64[1] * center64[1]) + center64[2] * center64[2];
        let shift = (squared >= 1.0e-8_f64 * 1.0e-8_f64).then(|| center.map(|value| -value));
        Ok(Self { shift })
    }

    pub fn shift(self) -> Option<[f32; 3]> {
        self.shift
    }

    pub fn object_pose(self, core: ProjectionKnot) -> Result<ProjectionKnot, TopologyError> {
        if let Some(component) = core
            .position
            .iter()
            .chain(core.orientation.iter())
            .position(|value| !value.is_finite())
        {
            return Err(TopologyError::NonFiniteTransform { component });
        }
        let Some(shift) = self.shift else {
            return Ok(core);
        };
        let shift = shift.map(f64::from);
        let position = std::array::from_fn(|row| {
            ((core.orientation[row * 3] * shift[0] + core.orientation[row * 3 + 1] * shift[1])
                + core.orientation[row * 3 + 2] * shift[2])
                + core.position[row]
        });
        if let Some(component) = position.iter().position(|value| !value.is_finite()) {
            return Err(TopologyError::NonFiniteTransform { component });
        }
        Ok(ProjectionKnot { position, ..core })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_center_shift_preserves_the_strict_squared_threshold() {
        let below = 1.0e-8_f32;
        assert_eq!(
            ObjectFrame::from_center([below, 0.0, 0.0]).unwrap(),
            ObjectFrame::identity()
        );
        let above = f32::from_bits(below.to_bits() + 1);
        assert_eq!(
            ObjectFrame::from_center([above, 0.0, 0.0]).unwrap().shift(),
            Some([-above, -0.0, -0.0])
        );
        assert_eq!(
            ObjectFrame::from_center([f32::NAN, 0.0, 0.0]),
            Err(ShapeError::NonFinite)
        );
    }

    #[test]
    fn cached_object_pose_retains_the_shifted_frame_until_its_time_code_changes() {
        use crate::{CacheActivity, CoreOrientation, CoreTransformState, TransformCache};
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        let state = CoreTransformState {
            object_frame: ObjectFrame::from_center([1.0, 2.0, 3.0]).unwrap(),
            position: [10.0, 20.0, 30.0],
            prior_orientation: orientation,
            next_orientation: orientation,
            projection_velocity: [0.0; 3],
            core_time: 0.0,
            environment_time: 0.0,
            inverse_step: 1.0,
        };
        let mut cache = TransformCache::default();
        let first = cache
            .resolve(1, CacheActivity::Simulated, 1, state)
            .unwrap();
        assert_eq!(first.object.position, [9.0, 18.0, 27.0]);
        let changed = CoreTransformState {
            object_frame: ObjectFrame::identity(),
            ..state
        };
        assert_eq!(
            cache
                .resolve(1, CacheActivity::Simulated, 1, changed)
                .unwrap(),
            first
        );
        assert_eq!(
            cache
                .resolve(1, CacheActivity::Simulated, 2, changed)
                .unwrap()
                .object
                .position,
            state.position
        );
    }
}
