use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SleepState {
    Moving,
    QuietPending,
    Sleeping,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SleepReference {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
    pub time: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SleepSample {
    pub reference: SleepReference,
    pub position: [f64; 3],
    pub orientation: [f64; 4],
    pub recent_orientation: [f32; 4],
    pub angular_velocity: [f32; 3],
    pub radius: f32,
    pub quiet_interval: f32,
    pub now: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SleepError {
    NonFinite,
    NegativeRadius,
    NonPositiveQuietInterval,
}

impl fmt::Display for SleepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => formatter.write_str("sleep state contains a non-finite value"),
            Self::NegativeRadius => formatter.write_str("sleep radius cannot be negative"),
            Self::NonPositiveQuietInterval => {
                formatter.write_str("sleep quiet interval must be positive")
            }
        }
    }
}

impl std::error::Error for SleepError {}

impl SleepSample {
    pub fn classify(self) -> Result<SleepState, SleepError> {
        if self
            .reference
            .position
            .iter()
            .any(|value| !value.is_finite())
            || self
                .reference
                .orientation
                .iter()
                .any(|value| !value.is_finite())
            || !self.reference.time.is_finite()
            || self.position.iter().any(|value| !value.is_finite())
            || self.orientation.iter().any(|value| !value.is_finite())
            || self
                .recent_orientation
                .iter()
                .any(|value| !value.is_finite())
            || self.angular_velocity.iter().any(|value| !value.is_finite())
            || !self.radius.is_finite()
            || !self.quiet_interval.is_finite()
            || !self.now.is_finite()
        {
            return Err(SleepError::NonFinite);
        }
        if self.radius < 0.0 {
            return Err(SleepError::NegativeRadius);
        }
        if self.quiet_interval <= 0.0 {
            return Err(SleepError::NonPositiveQuietInterval);
        }

        let delta: [f64; 3] = std::array::from_fn(|axis| {
            self.position[axis] - f64::from(self.reference.position[axis])
        });
        let displacement_squared = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
        let angular_displacement =
            |reference| angular_distance(reference, self.orientation, self.radius);
        let orientation_limit = 0.000_024_999_998_882_412_923;
        if displacement_squared > 0.000_099_999_995_529_651_69
            || angular_displacement(self.reference.orientation) > orientation_limit
        {
            return Ok(SleepState::Moving);
        }
        if ((self.now - self.reference.time) as f32) <= self.quiet_interval {
            return Ok(SleepState::QuietPending);
        }
        let angular_squared = self.angular_velocity[0] * self.angular_velocity[0]
            + self.angular_velocity[1] * self.angular_velocity[1]
            + self.angular_velocity[2] * self.angular_velocity[2];
        let angular_limit = (2.356_194_490_192_345 / f64::from(self.quiet_interval)) as f32;
        if angular_squared > angular_limit * angular_limit
            && angular_displacement(self.recent_orientation) > orientation_limit
        {
            Ok(SleepState::Moving)
        } else {
            Ok(SleepState::Sleeping)
        }
    }
}

fn angular_distance(reference: [f32; 4], orientation: [f64; 4], radius: f32) -> f64 {
    let dot = (f64::from(reference[3]) * orientation[3] + f64::from(reference[2]) * orientation[2])
        + (f64::from(reference[0]) * orientation[0] + f64::from(reference[1]) * orientation[1]);
    (((1.0 - dot * dot) * 2.0) * f64::from(radius)) * f64::from(radius)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct QuietMotion {
    references: [SleepReference; 2],
}
impl QuietMotion {
    pub(crate) fn request_sleep(
        &mut self,
        position: [f64; 3],
        orientation: [f64; 4],
        now: f64,
    ) -> Result<(), SleepError> {
        let reference = SleepReference {
            position: position.map(|value| value as f32),
            orientation: orientation.map(|value| value as f32),
            time: f64::from((now - 20.0) as f32),
        };
        if reference
            .position
            .iter()
            .chain(&reference.orientation)
            .any(|value| !value.is_finite())
            || !reference.time.is_finite()
        {
            return Err(SleepError::NonFinite);
        }
        self.references[0] = reference;
        Ok(())
    }
    pub(crate) fn new() -> Self {
        Self {
            references: [SleepReference {
                position: [0.0; 3],
                orientation: [0.0; 4],
                time: 0.0,
            }; 2],
        }
    }
    pub(crate) fn refresh_time(&mut self, time: f64) {
        for reference in &mut self.references {
            reference.time = time;
        }
    }
    pub(crate) fn advance(
        &mut self,
        mut sample: SleepSample,
        prior_orientation: [f64; 4],
    ) -> Result<SleepState, SleepError> {
        sample.reference = self.references[0];
        sample.recent_orientation = prior_orientation.map(|v| v as f32);
        let short = sample.classify()?;
        if short != SleepState::Moving {
            return Ok(short);
        }
        let next_reference = SleepReference {
            position: sample.position.map(|v| v as f32),
            orientation: sample.orientation.map(|v| v as f32),
            time: sample.now,
        };
        let long = self.references[1];
        let delta =
            std::array::from_fn::<_, 3, _>(|i| sample.position[i] - f64::from(long.position[i]));
        let shifted = (delta[0] * delta[0] + delta[1] * delta[1]) + delta[2] * delta[2]
            > 0.010_000_000_298_023_226
            || angular_distance(long.orientation, prior_orientation, sample.radius)
                > 0.040_000_001_192_092_904;
        self.references[0] = next_reference;
        if shifted {
            self.references[1] = SleepReference {
                orientation: prior_orientation.map(|v| v as f32),
                ..next_reference
            };
            Ok(SleepState::Moving)
        } else if (sample.now - long.time) as f32 > 4.0 {
            Ok(SleepState::Sleeping)
        } else {
            Ok(SleepState::Moving)
        }
    }
    pub(crate) fn reference(&self) -> SleepReference {
        self.references[0]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SleepScheduler {
    countdown: u16,
    random_state: u32,
}

impl SleepScheduler {
    pub const fn new(countdown: u16, random_state: u32) -> Self {
        Self {
            countdown,
            random_state,
        }
    }

    pub const fn countdown(self) -> u16 {
        self.countdown
    }

    pub const fn random_state(self) -> u32 {
        self.random_state
    }

    pub fn advance(&mut self) -> bool {
        self.countdown = self.countdown.wrapping_sub(1);
        if self.countdown != 0 {
            return false;
        }
        self.random_state = self.random_state.wrapping_mul(75);
        let fraction = (self.random_state & 0xffff) as f32 / 65_536.0;
        self.countdown = (15_i32 - (fraction * -5.0) as i32) as u16;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{SleepError, SleepReference, SleepSample, SleepScheduler, SleepState};

    fn sample() -> SleepSample {
        SleepSample {
            reference: SleepReference {
                position: [0.0; 3],
                orientation: [0.0, 0.0, 0.0, 1.0],
                time: 0.0,
            },
            position: [0.0; 3],
            orientation: [0.0, 0.0, 0.0, 1.0],
            recent_orientation: [0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            radius: 0.111_969_75,
            quiet_interval: 0.3,
            now: 0.0,
        }
    }

    #[test]
    fn sleep_classification_distinguishes_motion_pending_and_terminal_quiet() {
        assert_eq!(sample().classify().unwrap(), SleepState::QuietPending);
        assert_eq!(
            SleepSample {
                position: [0.02, 0.0, 0.0],
                ..sample()
            }
            .classify()
            .unwrap(),
            SleepState::Moving
        );
        assert_eq!(
            SleepSample {
                now: 0.31,
                ..sample()
            }
            .classify()
            .unwrap(),
            SleepState::Sleeping
        );
        assert_eq!(
            SleepSample {
                quiet_interval: 0.0,
                ..sample()
            }
            .classify(),
            Err(SleepError::NonPositiveQuietInterval)
        );
    }

    #[test]
    fn sleep_scheduler_preserves_random_state_until_a_due_check() {
        let mut scheduler = SleepScheduler::new(2, 17);
        assert!(!scheduler.advance());
        assert_eq!(scheduler.countdown(), 1);
        assert_eq!(scheduler.random_state(), 17);
        assert!(scheduler.advance());
        assert_eq!(scheduler.random_state(), 1_275);
        assert_eq!(scheduler.countdown(), 15);
    }

    #[test]
    fn angular_motion_requires_both_escape_checks() {
        let still = SleepSample {
            now: 0.31,
            angular_velocity: [100.0, 0.0, 0.0],
            ..sample()
        };
        assert_eq!(still.classify().unwrap(), SleepState::Sleeping);
        let changed = SleepSample {
            recent_orientation: [1.0, 0.0, 0.0, 0.0],
            ..still
        };
        assert_eq!(changed.classify().unwrap(), SleepState::Moving);
        assert_eq!(
            SleepSample {
                angular_velocity: [0.0; 3],
                ..changed
            }
            .classify()
            .unwrap(),
            SleepState::Sleeping
        );
    }

    #[test]
    fn zero_countdown_wraps_without_consuming_random_state() {
        let mut scheduler = SleepScheduler::new(0, 17);
        assert!(!scheduler.advance());
        assert_eq!(scheduler.countdown(), u16::MAX);
        assert_eq!(scheduler.random_state(), 17);
    }
}
