use std::fmt;

pub const DEFAULT_ENVIRONMENT_TIMESTEP: f32 = 1.0 / 66.0;
pub(crate) const FRAME_LOOKAHEAD: f32 = f32::from_bits(0x3fff_ffac);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    NonFinite,
    NonPositiveTimestep,
    NonIncreasingTime,
    BoundaryLimit,
}

impl fmt::Display for ClockError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => output.write_str("physical clock contains a non-finite time"),
            Self::NonPositiveTimestep => {
                output.write_str("physical clock timestep must be positive")
            }
            Self::NonIncreasingTime => {
                output.write_str("physical clock cannot advance its current time")
            }
            Self::BoundaryLimit => {
                output.write_str("fixed physical frame exceeds two integration boundaries")
            }
        }
    }
}
impl std::error::Error for ClockError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedStepClock {
    fixed_submissions: bool,
    timestep: f32,
    current: f64,
    last_boundary: f64,
    next_boundary: f64,
    next_frame: f32,
    time_code: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedStepInterval {
    pub begin: f64,
    pub end: f64,
    boundaries: [f64; 2],
    count: usize,
}

impl FixedStepInterval {
    pub fn boundaries(&self) -> &[f64] {
        &self.boundaries[..self.count]
    }
}

impl FixedStepClock {
    pub fn new(timestep: f32) -> Result<Self, ClockError> {
        if !timestep.is_finite() {
            return Err(ClockError::NonFinite);
        }
        if timestep <= 0.0 {
            return Err(ClockError::NonPositiveTimestep);
        }
        Ok(Self {
            fixed_submissions: true,
            timestep,
            current: 0.0,
            last_boundary: 0.0,
            next_boundary: 0.0,
            next_frame: DEFAULT_ENVIRONMENT_TIMESTEP,
            time_code: 1,
        })
    }

    pub fn timestep(self) -> f32 {
        self.timestep
    }
    pub(crate) fn set_timestep(&mut self, timestep: f32) -> Result<(), ClockError> {
        if !timestep.is_finite() {
            return Err(ClockError::NonFinite);
        }
        if timestep <= 0.0 {
            return Err(ClockError::NonPositiveTimestep);
        }
        self.timestep = timestep;
        Ok(())
    }
    pub fn current_time(self) -> f64 {
        self.current
    }
    pub fn last_boundary(self) -> f64 {
        self.last_boundary
    }
    pub fn next_boundary(self) -> f64 {
        self.next_boundary
    }
    pub fn next_frame_time(self) -> f32 {
        self.next_frame
    }
    pub fn time_code(self) -> u32 {
        self.time_code
    }
    pub(crate) fn reset_simulation(&mut self) {
        self.fixed_submissions = true;
        self.current = 0.0;
        self.last_boundary = 0.0;
        self.next_boundary = f64::from(self.timestep);
        self.next_frame = self.timestep;
        self.time_code = self.time_code.wrapping_add(3);
    }

    pub fn frame_terminal(self) -> Result<f64, ClockError> {
        let end = self.last_boundary + f64::from(self.timestep * FRAME_LOOKAHEAD);
        if !end.is_finite() {
            return Err(ClockError::NonFinite);
        }
        Ok(end)
    }
    pub(crate) fn submission_terminal(&mut self, duration: f32) -> Result<Option<f64>, ClockError> {
        if !duration.is_finite() {
            return Err(ClockError::NonFinite);
        }
        if duration > 1.0 || f64::from(duration) <= 0.0001 {
            return Ok(None);
        }
        let duration = if f64::from(duration) > 0.1 {
            0.1_f32
        } else {
            duration
        };
        let terminal = if self.fixed_submissions && duration == self.timestep {
            self.frame_terminal()?
        } else {
            self.fixed_submissions = false;
            self.current + f64::from(duration)
        };
        if !terminal.is_finite() {
            return Err(ClockError::NonIncreasingTime);
        }
        Ok(Some(terminal))
    }

    pub(crate) fn visit(&mut self, time: f64) -> Result<(), ClockError> {
        if !time.is_finite() {
            return Err(ClockError::NonFinite);
        }
        if time < self.current {
            return Err(ClockError::NonIncreasingTime);
        }
        self.current = time;
        self.time_code = self.time_code.wrapping_add(1);
        Ok(())
    }
    pub(crate) fn finish_submission(&mut self, time: f64) -> Result<(), ClockError> {
        if !time.is_finite() {
            return Err(ClockError::NonFinite);
        }
        self.current = time;
        self.time_code = self.time_code.wrapping_add(1);
        Ok(())
    }

    pub(crate) fn cross_boundary(&mut self, time: f64) -> Result<(), ClockError> {
        if time != self.next_boundary {
            return Err(ClockError::NonIncreasingTime);
        }
        let next = time + f64::from(self.timestep);
        if !next.is_finite() || next <= time {
            return Err(ClockError::NonIncreasingTime);
        }
        self.visit(time)?;
        self.last_boundary = time;
        self.next_boundary = next;
        self.next_frame = next as f32;
        Ok(())
    }

    pub fn advance(&mut self) -> Result<FixedStepInterval, ClockError> {
        let end = self.frame_terminal()?;
        let mut next = *self;
        let mut interval = FixedStepInterval {
            begin: self.current,
            end,
            boundaries: [0.0; 2],
            count: 0,
        };
        while next.next_boundary < end {
            if interval.count == interval.boundaries.len() {
                return Err(ClockError::BoundaryLimit);
            }
            interval.boundaries[interval.count] = next.next_boundary;
            interval.count += 1;
            next.cross_boundary(next.next_boundary)?;
        }
        next.visit(end)?;
        *self = next;
        Ok(interval)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn submission_duration_keeps_fast_path_until_the_first_admitted_mismatch() {
        let mut clock = super::FixedStepClock::new(0.015).unwrap();
        let original = clock;
        for duration in [-1.0, 0.0, 0.0001, 1.1] {
            assert_eq!(clock.submission_terminal(duration).unwrap(), None);
            assert_eq!(clock, original);
        }
        assert_eq!(
            clock.submission_terminal(0.015).unwrap(),
            Some(clock.frame_terminal().unwrap())
        );
        let end = clock.submission_terminal(0.0075).unwrap().unwrap();
        assert_eq!(end, f64::from(0.0075_f32));
        clock.visit(end).unwrap();
        assert_eq!(
            clock.submission_terminal(0.015).unwrap(),
            Some(end + f64::from(0.015_f32))
        );
        assert_eq!(
            clock.submission_terminal(0.2).unwrap(),
            Some(end + f64::from(0.1_f32))
        );
    }
    use super::*;

    #[test]
    fn fresh_and_warmed_frames_retain_distinct_boundary_counts_and_float_publication() {
        let mut clock = FixedStepClock::new(0.015).unwrap();
        assert_eq!(clock.next_frame_time(), DEFAULT_ENVIRONMENT_TIMESTEP);
        let first = clock.advance().unwrap();
        assert_eq!(first.boundaries(), [0.0, f64::from(0.015_f32)]);
        assert_eq!(
            first.end.to_bits(),
            f64::from(0.015_f32 * FRAME_LOOKAHEAD).to_bits()
        );
        let saved = clock;
        let second = clock.advance().unwrap();
        assert_eq!(second.boundaries(), [f64::from(0.015_f32) * 2.0]);
        assert_eq!(second.begin, first.end);
        let mut restored = saved;
        assert_eq!(restored.advance().unwrap(), second);
        assert_eq!(clock, restored);
    }

    #[test]
    fn invalid_periods_fail_before_constructing_a_clock() {
        assert_eq!(
            FixedStepClock::new(0.0),
            Err(ClockError::NonPositiveTimestep)
        );
        assert_eq!(FixedStepClock::new(f32::NAN), Err(ClockError::NonFinite));
    }
}
