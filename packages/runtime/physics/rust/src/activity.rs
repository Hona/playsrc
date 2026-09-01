use crate::MotionError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MotionActivity {
    pub fast: bool,
    pub clear_contact_energy: bool,
}

impl MotionActivity {
    /// Consumes all active core velocities in internal meters/second.
    /// Returns whether the group's quiet-reference clocks must be reset.
    pub fn advance(&mut self, linear_velocities: &[[f32; 3]]) -> Result<bool, MotionError> {
        if linear_velocities.is_empty() {
            return Err(MotionError::EmptyGroup);
        }
        let mut fast = false;
        for velocity in linear_velocities {
            if velocity.iter().any(|value| !value.is_finite()) {
                return Err(MotionError::NonFinite);
            }
            let squared =
                (velocity[0] * velocity[0] + velocity[1] * velocity[1]) + velocity[2] * velocity[2];
            if !squared.is_finite() {
                return Err(MotionError::NonFinite);
            }
            fast |= (1.0_f32 - squared).is_sign_negative();
        }
        let reset_quiet_time = !fast && self.fast;
        if !fast {
            self.clear_contact_energy = self.fast;
        }
        self.fast = fast;
        Ok(reset_quiet_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_preserves_strict_threshold_and_the_retained_energy_reset_flag() {
        let mut state = MotionActivity::default();
        assert!(!state.advance(&[[1.0, 0.0, 0.0]]).unwrap());
        assert!(!state.fast);
        assert!(
            !state
                .advance(&[[0.0; 3], [f32::from_bits(1.0_f32.to_bits() + 1), 0.0, 0.0]])
                .unwrap()
        );
        assert!(state.fast);
        assert!(state.advance(&[[1.0, 0.0, 0.0]]).unwrap());
        assert_eq!(
            state,
            MotionActivity {
                fast: false,
                clear_contact_energy: true
            }
        );
        assert!(!state.advance(&[[2.0, 0.0, 0.0]]).unwrap());
        assert_eq!(
            state,
            MotionActivity {
                fast: true,
                clear_contact_energy: true
            }
        );
        assert!(state.advance(&[[0.0; 3]]).unwrap());
        assert!(!state.advance(&[[0.0; 3]]).unwrap());
        assert_eq!(state, MotionActivity::default());
    }

    #[test]
    fn failed_activity_input_preserves_both_flags() {
        let mut state = MotionActivity {
            fast: true,
            clear_contact_energy: true,
        };
        let before = state;
        assert_eq!(state.advance(&[]), Err(MotionError::EmptyGroup));
        assert_eq!(
            state.advance(&[[0.0; 3], [f32::NAN, 0.0, 0.0]]),
            Err(MotionError::NonFinite)
        );
        assert_eq!(
            state.advance(&[[f32::MAX, 0.0, 0.0]]),
            Err(MotionError::NonFinite)
        );
        assert_eq!(state, before);
    }
}
