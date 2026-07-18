use crate::{MAXIMUM_HOST_ELAPSED, MINIMUM_HOST_ELAPSED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElapsedAdjustment {
    None,
    RaisedToMinimum,
    ClampedToMaximum,
    ClockReversal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockFrame {
    pub raw_elapsed: f32,
    pub admitted_elapsed: f32,
    pub adjustment: ElapsedAdjustment,
}

impl ClockFrame {
    pub fn discarded_seconds(self) -> f32 {
        if matches!(self.adjustment, ElapsedAdjustment::ClampedToMaximum) {
            self.raw_elapsed - self.admitted_elapsed
        } else {
            0.0
        }
    }

    pub fn added_seconds(self) -> f32 {
        if matches!(self.adjustment, ElapsedAdjustment::RaisedToMinimum) {
            self.admitted_elapsed - self.raw_elapsed
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClockObservation {
    Baseline,
    Suspended,
    Frame(ClockFrame),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WallClock {
    previous: Option<f64>,
}

impl WallClock {
    pub(crate) const fn new() -> Self {
        Self { previous: None }
    }

    pub(crate) fn observe(
        &mut self,
        now: f64,
        duration_to_next_tick: f32,
        suspended: bool,
    ) -> ClockObservation {
        let Some(previous) = self.previous.replace(now) else {
            return ClockObservation::Baseline;
        };
        if suspended {
            return ClockObservation::Suspended;
        }

        if now < previous {
            return ClockObservation::Frame(ClockFrame {
                raw_elapsed: duration_to_next_tick,
                admitted_elapsed: duration_to_next_tick
                    .clamp(MINIMUM_HOST_ELAPSED, MAXIMUM_HOST_ELAPSED),
                adjustment: ElapsedAdjustment::ClockReversal,
            });
        }

        let raw_elapsed = (now - previous) as f32;
        let admitted_elapsed = raw_elapsed.clamp(MINIMUM_HOST_ELAPSED, MAXIMUM_HOST_ELAPSED);
        let adjustment = if raw_elapsed > MAXIMUM_HOST_ELAPSED {
            ElapsedAdjustment::ClampedToMaximum
        } else if raw_elapsed < MINIMUM_HOST_ELAPSED {
            ElapsedAdjustment::RaisedToMinimum
        } else {
            ElapsedAdjustment::None
        };
        ClockObservation::Frame(ClockFrame {
            raw_elapsed,
            admitted_elapsed,
            adjustment,
        })
    }

    pub(crate) fn rebase(&mut self, now: f64) {
        self.previous = Some(now);
    }
}
