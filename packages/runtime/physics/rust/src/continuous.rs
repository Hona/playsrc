use std::fmt;

pub(crate) const SEARCH_CELL_SECONDS: f32 = 0.005;
pub(crate) const SORTED_EVENT_CAPACITY: usize = 65_532;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuousError {
    NonFinite,
    InvalidInterval,
    InvalidBracket,
    EventBeforeBase,
    InvalidEventSpeed,
    InvalidEventDelay,
    InvalidEventCapacity,
    EventCapacity,
    DuplicateEvent,
    MissingEvent,
    EventBeforeDispatched,
    InvalidTraversalLimit,
}

impl fmt::Display for ContinuousError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => {
                formatter.write_str("continuous collision contains a non-finite value")
            }
            Self::InvalidInterval => {
                formatter.write_str("continuous collision interval must increase")
            }
            Self::InvalidBracket => {
                formatter.write_str("continuous collision values do not bracket the target")
            }
            Self::EventBeforeBase => {
                formatter.write_str("continuous event cannot precede its fixed-step base")
            }
            Self::InvalidEventSpeed => {
                formatter.write_str("continuous event speed must be positive")
            }
            Self::InvalidEventDelay => {
                formatter.write_str("continuous event delay scale cannot be negative")
            }
            Self::InvalidEventCapacity => {
                formatter.write_str("continuous event capacity must be between one and 65532")
            }
            Self::EventCapacity => formatter.write_str("continuous event queue capacity exceeded"),
            Self::DuplicateEvent => formatter.write_str("continuous event identity already exists"),
            Self::MissingEvent => formatter.write_str("continuous event identity does not exist"),
            Self::EventBeforeDispatched => {
                formatter.write_str("continuous event cannot precede the last dispatched event")
            }
            Self::InvalidTraversalLimit => formatter
                .write_str("continuous selected-feature limit must be between one and twenty"),
        }
    }
}

impl std::error::Error for ContinuousError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousRefinement {
    pub lower: f64,
    pub upper: f64,
    pub target: f64,
    pub initial_value: Option<f64>,
    pub final_value: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContinuousSample {
    Raster { time: f64, index: u8 },
    Refinement { time: f64 },
}

impl ContinuousSample {
    pub fn time(self) -> f64 {
        match self {
            Self::Raster { time, .. } | Self::Refinement { time } => time,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousRoot {
    pub time: f64,
    pub value: f64,
    pub iterations: u8,
    pub exhausted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousTraversal {
    pub lower: f64,
    pub upper: f64,
    pub target: f64,
    pub real_surface: f64,
    pub maximum_deviation: f64,
    pub initial_value: Option<f64>,
    pub maximum_cells: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousFeatureWindow {
    pub clock: ContinuousEventClock,
    pub phase_start: f64,
    pub phase_end: f64,
    pub sector_transition: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousCrossing {
    pub lower: f64,
    pub upper: f64,
    pub lower_value: f64,
    pub root: ContinuousRoot,
    pub visited_cells: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionDeviation {
    pub rotational_projection: f32,
    pub surface_speed: f32,
    pub approaching_linear_speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousEventClock {
    base: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousEventTime {
    pub offset: f32,
    pub absolute: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousEvent<Id = u64> {
    pub identity: Id,
    pub time: ContinuousEventTime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContinuousEventQueue<Id = u64> {
    clock: ContinuousEventClock,
    entries: Vec<ContinuousEvent<Id>>,
    capacity: usize,
    last_offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContinuousEventDelay {
    pub separation: f32,
    pub collision_distance: f32,
    pub speed: f64,
    pub timestep: f32,
    pub scale: f64,
    pub current_time: f64,
    pub proposed_time: f64,
    pub phase_end: f64,
    pub hint: EventTimingHint,
    pub kind: EventTimingKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventTimingHint {
    Immediate,
    ShortDelay,
    LongDelay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventTimingKind {
    Collision,
    FeatureTransition,
}

impl ContinuousEventDelay {
    pub fn candidate(self) -> Result<Option<f64>, ContinuousError> {
        if !self.separation.is_finite()
            || !self.collision_distance.is_finite()
            || !self.speed.is_finite()
            || !self.timestep.is_finite()
            || !self.scale.is_finite()
            || !self.current_time.is_finite()
            || !self.proposed_time.is_finite()
            || !self.phase_end.is_finite()
        {
            return Err(ContinuousError::NonFinite);
        }
        if self.speed < 0.0 {
            return Err(ContinuousError::InvalidEventSpeed);
        }
        if self.timestep <= 0.0
            || self.phase_end <= self.current_time
            || self.proposed_time < self.current_time
        {
            return Err(ContinuousError::InvalidInterval);
        }
        if self.scale < 0.0 {
            return Err(ContinuousError::InvalidEventDelay);
        }
        if (self.proposed_time - self.current_time) as f32 >= 1.0e-6_f32 {
            return Ok(Some(self.proposed_time));
        }
        if self.hint == EventTimingHint::Immediate {
            return Ok(Some(self.current_time));
        }
        let distance = f64::from(self.separation) - f64::from(self.collision_distance);
        let long = self.hint == EventTimingHint::LongDelay
            || self.kind == EventTimingKind::FeatureTransition;
        let candidate = if distance >= 1.0e-12 {
            if self.speed == 0.0 {
                return Err(ContinuousError::InvalidEventSpeed);
            }
            let advance = if long {
                distance / self.speed
            } else {
                (f64::from(0.1_f32) * distance) / self.speed
            };
            let minimum = self.scale
                * f64::from(if long { 1.0e-4_f32 } else { 1.0e-7_f32 })
                * f64::from(self.timestep);
            (advance + self.current_time) + minimum
        } else {
            let minimum = (self.scale * f64::from(if long { 1.0e-3_f32 } else { 1.0e-5_f32 }))
                * f64::from(self.timestep);
            self.current_time + minimum
        };
        if !candidate.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        Ok((((candidate - self.phase_end) as f32) < 0.0).then_some(candidate))
    }
}

impl ContinuousEventClock {
    pub fn new(base: f64) -> Result<Self, ContinuousError> {
        if !base.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        Ok(Self { base })
    }

    pub fn base(self) -> f64 {
        self.base
    }

    pub fn schedule(self, candidate: f64) -> Result<ContinuousEventTime, ContinuousError> {
        if !candidate.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        if candidate < self.base {
            return Err(ContinuousError::EventBeforeBase);
        }
        let offset = (candidate - self.base) as f32;
        if !offset.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        let absolute = self.base + f64::from(offset);
        if !absolute.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        Ok(ContinuousEventTime { offset, absolute })
    }
}

impl<Id: Copy + Eq> ContinuousEventQueue<Id> {
    pub fn new(base: f64, capacity: usize) -> Result<Self, ContinuousError> {
        if capacity == 0 || capacity > SORTED_EVENT_CAPACITY {
            return Err(ContinuousError::InvalidEventCapacity);
        }
        Ok(Self {
            clock: ContinuousEventClock::new(base)?,
            entries: Vec::new(),
            capacity,
            last_offset: 0.0,
        })
    }

    pub fn base(&self) -> f64 {
        self.clock.base()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[ContinuousEvent<Id>] {
        &self.entries
    }

    pub fn insert(
        &mut self,
        identity: Id,
        candidate: f64,
    ) -> Result<ContinuousEvent<Id>, ContinuousError> {
        if self.entries.len() == self.capacity {
            return Err(ContinuousError::EventCapacity);
        }
        if self.entries.iter().any(|event| event.identity == identity) {
            return Err(ContinuousError::DuplicateEvent);
        }
        let event = ContinuousEvent {
            identity,
            time: self.clock.schedule(candidate)?,
        };
        if event.time.offset < self.last_offset {
            return Err(ContinuousError::EventBeforeDispatched);
        }
        let position = self
            .entries
            .iter()
            .position(|prior| prior.time.offset >= event.time.offset)
            .unwrap_or(self.entries.len());
        self.entries.insert(position, event);
        Ok(event)
    }

    pub fn remove(&mut self, identity: Id) -> Result<ContinuousEvent<Id>, ContinuousError> {
        let position = self
            .entries
            .iter()
            .position(|event| event.identity == identity)
            .ok_or(ContinuousError::MissingEvent)?;
        Ok(self.entries.remove(position))
    }

    pub fn update(
        &mut self,
        identity: Id,
        candidate: f64,
    ) -> Result<ContinuousEvent<Id>, ContinuousError> {
        let position = self
            .entries
            .iter()
            .position(|event| event.identity == identity)
            .ok_or(ContinuousError::MissingEvent)?;
        let scheduled = self.clock.schedule(candidate)?;
        if scheduled.offset < self.last_offset {
            return Err(ContinuousError::EventBeforeDispatched);
        }
        self.entries.remove(position);
        let updated = ContinuousEvent {
            identity,
            time: scheduled,
        };
        let position = self
            .entries
            .iter()
            .position(|prior| prior.time.offset >= updated.time.offset)
            .unwrap_or(self.entries.len());
        self.entries.insert(position, updated);
        Ok(updated)
    }

    pub fn pop_before(
        &mut self,
        terminal: f64,
    ) -> Result<Option<ContinuousEvent<Id>>, ContinuousError> {
        if !terminal.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        if terminal < self.clock.base() {
            return Err(ContinuousError::EventBeforeBase);
        }
        if self
            .entries
            .first()
            .is_some_and(|event| f64::from(event.time.offset) < terminal - self.clock.base())
        {
            Ok(self.pop_next())
        } else {
            Ok(None)
        }
    }

    pub fn pop_next(&mut self) -> Option<ContinuousEvent<Id>> {
        if self.entries.is_empty() {
            return None;
        }
        let event = self.entries.remove(0);
        self.last_offset = event.time.offset;
        Some(event)
    }

    pub fn reset(&mut self, base: f64) -> Result<(), ContinuousError> {
        let clock = ContinuousEventClock::new(base)?;
        let mut replacement = Vec::with_capacity(self.entries.len());
        for event in &self.entries {
            replacement.push(ContinuousEvent {
                identity: event.identity,
                time: clock.schedule(event.time.absolute)?,
            });
        }
        self.clock = clock;
        self.entries = replacement;
        self.last_offset = 0.0;
        Ok(())
    }
    pub(crate) fn shift_clock_origin(&mut self, shift: f64) -> Result<(), ContinuousError> {
        let delta = (shift - self.clock.base) as f32;
        if !delta.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        for event in &mut self.entries {
            event.time.offset -= delta;
            event.time.absolute = f64::from(event.time.offset);
        }
        self.last_offset -= delta;
        self.clock = ContinuousEventClock::new(0.0)?;
        Ok(())
    }
}

impl ContinuousRefinement {
    pub fn solve(
        self,
        mut value_at: impl FnMut(f64) -> f64,
    ) -> Result<ContinuousRoot, ContinuousError> {
        if !self.lower.is_finite()
            || !self.upper.is_finite()
            || !self.target.is_finite()
            || self.initial_value.is_some_and(|value| !value.is_finite())
            || self.final_value.is_some_and(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        if self.lower >= self.upper {
            return Err(ContinuousError::InvalidInterval);
        }
        let mut lower = self.lower;
        let mut upper = self.upper;
        let mut lower_value = self.initial_value.unwrap_or_else(|| value_at(lower));
        let mut upper_value = self.final_value.unwrap_or_else(|| value_at(upper));
        if !lower_value.is_finite() || !upper_value.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        if lower_value < self.target || upper_value > self.target {
            return Err(ContinuousError::InvalidBracket);
        }
        for iteration in 0..68_u8 {
            let span = f64::from((upper - lower) as f32);
            let mut time = lower + span * (self.target - lower_value) / (upper_value - lower_value);
            if iteration & 3 == 3 {
                if iteration > 64 {
                    return Ok(ContinuousRoot {
                        time: lower,
                        value: lower_value,
                        iterations: iteration,
                        exhausted: true,
                    });
                }
                time +=
                    (f64::from((lower - time) as f32) + f64::from((upper - time) as f32)) * 0.375;
            }
            let value = value_at(time);
            if !value.is_finite() {
                return Err(ContinuousError::NonFinite);
            }
            if (value - self.target).abs() < 1.0e-8 {
                return Ok(ContinuousRoot {
                    time,
                    value,
                    iterations: iteration + 1,
                    exhausted: false,
                });
            }
            if value >= self.target {
                lower = time;
                lower_value = value;
            } else {
                upper = time;
                upper_value = value;
            }
        }
        Ok(ContinuousRoot {
            time: lower,
            value: lower_value,
            iterations: 68,
            exhausted: true,
        })
    }
}

impl ContinuousTraversal {
    pub fn solve(
        self,
        mut value_at: impl FnMut(ContinuousSample) -> f64,
    ) -> Result<Option<ContinuousCrossing>, ContinuousError> {
        if !self.lower.is_finite()
            || !self.upper.is_finite()
            || !self.target.is_finite()
            || !self.real_surface.is_finite()
            || !self.maximum_deviation.is_finite()
            || self.initial_value.is_some_and(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        if self.lower >= self.upper {
            return Err(ContinuousError::InvalidInterval);
        }
        if self.maximum_deviation <= 0.0 {
            return Err(ContinuousError::InvalidEventSpeed);
        }
        if self.maximum_cells == 0 || self.maximum_cells > 20 {
            return Err(ContinuousError::InvalidTraversalLimit);
        }
        if self.real_surface > self.target {
            return Err(ContinuousError::InvalidBracket);
        }

        let mut lower = self.lower;
        let mut value = self.initial_value.unwrap_or_else(|| {
            value_at(ContinuousSample::Raster {
                time: lower,
                index: 0,
            })
        });
        if !value.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        let original = value;
        let mut cells = 0_u8;
        let mut overlapping = value <= self.target;
        let inverse_deviation = 1.0 / self.maximum_deviation;

        while cells < self.maximum_cells {
            let remaining_interval = f64::from((self.upper - lower) as f32);
            let raw = if overlapping {
                ((value - self.real_surface) * inverse_deviation).max(0.0)
            } else {
                let minimum = (value - self.target) * inverse_deviation;
                if f64::from((lower - self.upper) as f32) + minimum > 0.0 {
                    return Ok(None);
                }
                minimum * 2.0
            };
            let bounded = if f64::from((lower - self.upper) as f32) + raw > 0.0 {
                remaining_interval + 1.0e-8
            } else {
                raw
            };
            let remaining = self.maximum_cells - cells;
            let advance = ((bounded * 200.0) as u8).max(1).min(remaining);
            let upper = lower + f64::from(f32::from(advance) * SEARCH_CELL_SECONDS);
            let next = value_at(ContinuousSample::Raster {
                time: upper,
                index: cells + advance,
            });
            if !next.is_finite() {
                return Err(ContinuousError::NonFinite);
            }
            cells += advance;

            if overlapping {
                if next > self.target {
                    if upper > self.upper {
                        return Ok(None);
                    }
                    lower = upper;
                    value = next;
                    overlapping = false;
                    continue;
                }
                if next <= original {
                    return Ok(Some(ContinuousCrossing {
                        lower,
                        upper,
                        lower_value: value,
                        root: ContinuousRoot {
                            time: lower,
                            value,
                            iterations: 0,
                            exhausted: false,
                        },
                        visited_cells: cells,
                    }));
                }
            } else if next <= self.target {
                let root = ContinuousRefinement {
                    lower,
                    upper,
                    target: self.target,
                    initial_value: Some(value),
                    final_value: Some(next),
                }
                .solve(|time| value_at(ContinuousSample::Refinement { time }))?;
                return Ok((root.time <= self.upper).then_some(ContinuousCrossing {
                    lower,
                    upper,
                    lower_value: value,
                    root,
                    visited_cells: cells,
                }));
            }
            if upper >= self.upper {
                return Ok(None);
            }
            lower = upper;
            value = next;
        }
        Ok(None)
    }
}

impl CollisionDeviation {
    pub fn maximum(self) -> Result<f64, ContinuousError> {
        if !self.rotational_projection.is_finite()
            || !self.surface_speed.is_finite()
            || !self.approaching_linear_speed.is_finite()
        {
            return Err(ContinuousError::NonFinite);
        }
        if self.surface_speed < 0.0 {
            return Err(ContinuousError::InvalidEventSpeed);
        }
        let projection = f64::from(self.rotational_projection);
        let perpendicular = f64::from(f32::from_bits(0x3f80_20c5)) - projection * projection;
        if perpendicular < 0.0 {
            return Err(ContinuousError::InvalidBracket);
        }
        let maximum = perpendicular.sqrt() * f64::from(self.surface_speed)
            + f64::from(self.approaching_linear_speed);
        if maximum <= 0.0 {
            return Err(ContinuousError::InvalidEventSpeed);
        }
        Ok(maximum)
    }
}

impl ContinuousFeatureWindow {
    pub fn start(self) -> Result<f64, ContinuousError> {
        if !self.phase_start.is_finite()
            || !self.phase_end.is_finite()
            || self
                .sector_transition
                .is_some_and(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite);
        }
        if self.phase_start >= self.phase_end {
            return Err(ContinuousError::InvalidInterval);
        }
        if self.phase_start < self.clock.base() {
            return Err(ContinuousError::EventBeforeBase);
        }
        let Some(transition) = self.sector_transition else {
            return Ok(self.phase_start);
        };
        if !(self.phase_start..self.phase_end).contains(&transition) {
            return Err(ContinuousError::InvalidInterval);
        }
        Ok(self.clock.schedule(transition)?.absolute)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CollisionDeviation, ContinuousError, ContinuousEventClock, ContinuousEventDelay,
        ContinuousEventQueue, ContinuousFeatureWindow, ContinuousRefinement, ContinuousTraversal,
        EventTimingHint, EventTimingKind,
    };

    #[test]
    fn cached_initial_distance_and_binary32_interval_produce_the_target_root() {
        let root = ContinuousRefinement {
            lower: f64::from_bits(0x3fb9_9999_9000_0000),
            upper: f64::from_bits(0x3fba_e147_a000_0000),
            target: 0.006_347_459_740_936_756,
            initial_value: Some(f64::from(f32::from_bits(0x3c17_077d))),
            final_value: None,
        }
        .solve(|time| 0.110_999_997_541_308_4 - time)
        .unwrap();
        assert!((root.value - 0.006_347_459_740_936_756).abs() < 1.0e-8);
        assert!(!root.exhausted);
        assert!(root.iterations <= 68);
    }

    #[test]
    fn refinement_reuses_both_retained_bracket_values() {
        let root = ContinuousRefinement {
            lower: 0.0,
            upper: 1.0,
            target: 0.0,
            initial_value: Some(1.0),
            final_value: Some(-1.0),
        }
        .solve(|time| {
            assert!(
                time > 0.0 && time < 1.0,
                "bracket endpoints are retained, not projected again"
            );
            1.0 - 2.0 * time
        })
        .unwrap();
        assert_eq!(root.time, 0.5);
        assert_eq!(root.value, 0.0);
    }

    #[test]
    fn nonfinite_reversed_and_unbracketed_intervals_fail_atomically() {
        assert_eq!(
            ContinuousRefinement {
                lower: f64::NAN,
                upper: 1.0,
                target: 0.0,
                initial_value: None,
                final_value: None,
            }
            .solve(|_| 0.0),
            Err(ContinuousError::NonFinite)
        );
        assert_eq!(
            ContinuousRefinement {
                lower: 1.0,
                upper: 1.0,
                target: 0.0,
                initial_value: None,
                final_value: None,
            }
            .solve(|_| 0.0),
            Err(ContinuousError::InvalidInterval)
        );
        assert_eq!(
            ContinuousRefinement {
                lower: 0.0,
                upper: 1.0,
                target: 2.0,
                initial_value: None,
                final_value: None,
            }
            .solve(|time| 1.0 - time),
            Err(ContinuousError::InvalidBracket)
        );
    }

    #[test]
    fn event_queue_rounds_against_the_fixed_step_base_instead_of_the_current_core() {
        let vectors = [
            (
                0x3fbe_b851_e000_0000,
                0x3fbf_a680_bd49_e8a7,
                0x3b6e_2edd,
                0x3fbf_a680_bd00_0000,
            ),
            (
                0x3fd1_47ae_0e00_0000,
                0x3fd1_f6b7_267b_ffc2,
                0x3c2f_0918,
                0x3fd1_f6b7_2600_0000,
            ),
            (
                0x3fdf_ae14_6f00_0000,
                0x3fe0_2f01_5a1c_288b,
                0x3c2f_ee45,
                0x3fe0_2f01_5a00_0000,
            ),
        ];
        for (base, candidate, offset, absolute) in vectors {
            let clock = ContinuousEventClock::new(f64::from_bits(base)).unwrap();
            let event = clock.schedule(f64::from_bits(candidate)).unwrap();
            assert_eq!(event.offset.to_bits(), offset);
            assert_eq!(event.absolute.to_bits(), absolute);
        }
    }

    #[test]
    fn event_clock_rejects_nonfinite_and_pre_boundary_events() {
        assert_eq!(
            ContinuousEventClock::new(f64::NAN),
            Err(ContinuousError::NonFinite)
        );
        let clock = ContinuousEventClock::new(2.0).unwrap();
        assert_eq!(
            clock.schedule(f64::INFINITY),
            Err(ContinuousError::NonFinite)
        );
        assert_eq!(clock.schedule(1.0), Err(ContinuousError::EventBeforeBase));
        assert_eq!(clock.base(), 2.0);
    }

    #[test]
    fn overlap_delay_preserves_authored_separation_and_binary32_speed_inputs() {
        let delayed = ContinuousEventDelay {
            separation: f32::from_bits(0x3b18_b767),
            collision_distance: f32::from_bits(0x3a26_6515),
            speed: f64::from_bits(0x3fd3_dde2_1780_0000),
            timestep: f32::from_bits(0x3c75_c28f),
            scale: 1.0,
            current_time: 0.0,
            proposed_time: 0.0,
            phase_end: 0.015,
            hint: EventTimingHint::ShortDelay,
            kind: EventTimingKind::Collision,
        }
        .candidate()
        .unwrap()
        .unwrap();
        let queued = ContinuousEventClock::new(0.0)
            .unwrap()
            .schedule(delayed)
            .unwrap();
        assert_eq!(queued.offset.to_bits(), 0x3a0f_2f67);
    }

    #[test]
    fn event_delay_rejects_zero_divisors_and_negative_scale_but_handles_subsurface_minimum() {
        let delay = ContinuousEventDelay {
            separation: 1.0,
            collision_distance: 0.25,
            speed: 2.0,
            timestep: 0.015,
            scale: 1.0,
            current_time: 0.0,
            proposed_time: 0.0,
            phase_end: 1.0,
            hint: EventTimingHint::ShortDelay,
            kind: EventTimingKind::Collision,
        };
        assert_eq!(
            ContinuousEventDelay {
                speed: 0.0,
                ..delay
            }
            .candidate(),
            Err(ContinuousError::InvalidEventSpeed)
        );
        assert_eq!(
            ContinuousEventDelay {
                scale: -1.0,
                ..delay
            }
            .candidate(),
            Err(ContinuousError::InvalidEventDelay)
        );
        assert_eq!(
            ContinuousEventDelay {
                separation: 0.0,
                ..delay
            }
            .candidate(),
            Ok(Some(f64::from(1.0e-5_f32) * f64::from(delay.timestep)))
        );
    }

    #[test]
    fn timing_modes_preserve_target_widths_and_exclude_adjusted_terminal_equality() {
        let base = ContinuousEventDelay {
            separation: 1.0,
            collision_distance: 0.25,
            speed: 2.0,
            timestep: 0.015,
            scale: 1.0,
            current_time: 0.125,
            proposed_time: 0.125,
            phase_end: 1.0,
            hint: EventTimingHint::ShortDelay,
            kind: EventTimingKind::Collision,
        };
        for (hint, kind, separation, expected) in [
            (
                EventTimingHint::ShortDelay,
                EventTimingKind::Collision,
                1.0,
                0x3fc4_cccc_d138_a23b,
            ),
            (
                EventTimingHint::LongDelay,
                EventTimingKind::Collision,
                1.0,
                0x3fe0_0003_254e_6b9f,
            ),
            (
                EventTimingHint::ShortDelay,
                EventTimingKind::FeatureTransition,
                1.0,
                0x3fe0_0003_254e_6b9f,
            ),
            (
                EventTimingHint::ShortDelay,
                EventTimingKind::Collision,
                0.0,
                0x3fc0_0001_421f_5e40,
            ),
            (
                EventTimingHint::LongDelay,
                EventTimingKind::Collision,
                0.0,
                0x3fc0_007d_d441_6a6a,
            ),
        ] {
            let input = ContinuousEventDelay {
                hint,
                kind,
                separation,
                ..base
            };
            assert_eq!(input.candidate().unwrap().unwrap().to_bits(), expected);
            assert_eq!(
                ContinuousEventDelay {
                    phase_end: f64::from_bits(expected),
                    ..input
                }
                .candidate()
                .unwrap(),
                None
            );
        }
        assert_eq!(
            ContinuousEventDelay {
                hint: EventTimingHint::Immediate,
                ..base
            }
            .candidate()
            .unwrap(),
            Some(base.current_time)
        );
        let proposed = base.current_time + f64::from(1.0e-6_f32);
        assert_eq!(
            ContinuousEventDelay {
                proposed_time: proposed,
                ..base
            }
            .candidate()
            .unwrap(),
            Some(proposed)
        );
    }

    #[test]
    fn queue_orders_equal_keys_newest_first_and_excludes_the_terminal_boundary() {
        let mut queue = ContinuousEventQueue::new(0.0, 4).unwrap();
        queue.insert(4, 0.25).unwrap();
        queue.insert(8, 0.5).unwrap();
        queue.insert(9, 0.25).unwrap();
        assert_eq!(
            queue
                .entries()
                .iter()
                .map(|event| event.identity)
                .collect::<Vec<_>>(),
            [9, 4, 8]
        );
        assert_eq!(queue.pop_before(0.25).unwrap(), None);
        assert_eq!(queue.pop_before(0.26).unwrap().unwrap().identity, 9);
        assert_eq!(queue.pop_before(0.26).unwrap().unwrap().identity, 4);
        assert_eq!(queue.pop_before(0.5).unwrap(), None);
        assert_eq!(queue.pop_before(0.51).unwrap().unwrap().identity, 8);
        assert!(queue.is_empty());
    }

    #[test]
    fn queue_update_reset_and_failures_preserve_atomic_ordered_continuation() {
        let mut queue = ContinuousEventQueue::new(0.0, 2).unwrap();
        queue.insert(1, 0.5).unwrap();
        queue.insert(2, 0.75).unwrap();
        let prior = queue.clone();
        assert_eq!(queue.insert(3, 1.0), Err(ContinuousError::EventCapacity));
        assert_eq!(queue, prior);
        assert_eq!(queue.update(3, 1.0), Err(ContinuousError::MissingEvent));
        assert_eq!(queue.update(1, f64::NAN), Err(ContinuousError::NonFinite));
        assert_eq!(queue, prior);
        queue.update(2, 0.5).unwrap();
        assert_eq!(queue.entries()[0].identity, 2);
        queue.reset(0.25).unwrap();
        assert_eq!(queue.base(), 0.25);
        assert_eq!(queue.entries()[0].time.offset.to_bits(), 0.25_f32.to_bits());
        let restored = queue.clone();
        assert_eq!(queue.pop_before(0.51).unwrap().unwrap().identity, 2);
        assert_eq!(
            queue.insert(3, 0.3),
            Err(ContinuousError::EventBeforeDispatched)
        );
        assert_eq!(queue.pop_before(0.51).unwrap().unwrap().identity, 1);
        assert_eq!(restored.entries().len(), 2);
        assert_eq!(queue.remove(8), Err(ContinuousError::MissingEvent));
        assert_eq!(
            ContinuousEventQueue::<u64>::new(0.0, 0),
            Err(ContinuousError::InvalidEventCapacity)
        );
    }

    #[test]
    fn selected_feature_preserves_the_complete_target_two_cell_bracket() {
        let lower = 0.959_999_978_542_327_9;
        let upper = 0.974_999_978_207_051_8;
        let initial = 0.008_049_875_497_817_993;
        let target = 0.006_347_459_740_936_756;
        let crossing = ContinuousTraversal {
            lower,
            upper,
            target,
            real_surface: 0.0,
            maximum_deviation: 0.332_056_386_589_506_37,
            initial_value: Some(initial),
            maximum_cells: 20,
        }
        .solve(|sample| initial - (sample.time() - lower) * 0.18)
        .unwrap()
        .unwrap();
        assert_eq!(crossing.lower.to_bits(), lower.to_bits());
        assert_eq!(
            crossing.upper.to_bits(),
            0.969_999_978_318_810_5_f64.to_bits()
        );
        assert_eq!(crossing.lower_value.to_bits(), initial.to_bits());
        assert_eq!(crossing.visited_cells, 2);
    }

    #[test]
    fn sorted_queue_capacity_excludes_reserved_allocator_indices() {
        assert!(ContinuousEventQueue::<u64>::new(0.0, 65_532).is_ok());
        for capacity in [65_533, 65_534, 65_535] {
            assert_eq!(
                ContinuousEventQueue::<u64>::new(0.0, capacity),
                Err(ContinuousError::InvalidEventCapacity)
            );
        }
    }

    #[test]
    fn initially_overlapping_feature_collides_only_after_moving_closer() {
        let search = ContinuousTraversal {
            lower: 0.0,
            upper: 0.015,
            target: 0.01,
            real_surface: 0.0,
            maximum_deviation: 1.0,
            initial_value: Some(0.006),
            maximum_cells: 20,
        };
        let collision = search
            .solve(|sample| 0.006 - sample.time())
            .unwrap()
            .unwrap();
        assert_eq!(collision.root.time, 0.0);
        assert_eq!(collision.root.iterations, 0);
        assert_eq!(
            search.solve(|sample| 0.006 + sample.time() * 0.1).unwrap(),
            None
        );
        assert_eq!(
            ContinuousTraversal {
                maximum_cells: 0,
                ..search
            }
            .solve(|_| 0.0),
            Err(ContinuousError::InvalidTraversalLimit)
        );
    }

    #[test]
    fn selected_feature_deviation_preserves_configured_binary64_envelope() {
        let deviation = CollisionDeviation {
            rotational_projection: -0.000_123_950_07,
            surface_speed: 0.225_917_83,
            approaching_linear_speed: 0.106_025_62,
        }
        .maximum()
        .unwrap();
        assert_eq!(deviation.to_bits(), 0.332_056_386_589_506_37_f64.to_bits());
        assert_eq!(
            CollisionDeviation {
                rotational_projection: f32::NAN,
                surface_speed: 1.0,
                approaching_linear_speed: 1.0,
            }
            .maximum(),
            Err(ContinuousError::NonFinite)
        );
    }

    #[test]
    fn selected_feature_window_rebases_transition_with_one_binary32_offset() {
        let phase = 0.479_999_989_271_163_94;
        let transition = 0.483_721_614_937_384_4;
        let window = ContinuousFeatureWindow {
            clock: ContinuousEventClock::new(phase).unwrap(),
            phase_start: phase,
            phase_end: phase + f64::from(0.015_f32),
            sector_transition: Some(transition),
        };
        assert_eq!(
            window.start().unwrap().to_bits(),
            (phase + f64::from((transition - phase) as f32)).to_bits()
        );
        assert_eq!(
            ContinuousFeatureWindow {
                sector_transition: None,
                ..window
            }
            .start()
            .unwrap()
            .to_bits(),
            phase.to_bits()
        );
        assert_eq!(
            ContinuousFeatureWindow {
                sector_transition: Some(window.phase_end),
                ..window
            }
            .start(),
            Err(ContinuousError::InvalidInterval)
        );
    }

    #[test]
    fn post_collision_sector_transition_keeps_the_fixed_boundary_time_base() {
        let window = ContinuousFeatureWindow {
            clock: ContinuousEventClock::new(0.1649999963119626).unwrap(),
            phase_start: 0.1663745978148654,
            phase_end: 0.17999999597668648,
            sector_transition: Some(0.1700065860034529),
        };
        assert_eq!(window.start().unwrap(), 0.1700065857730806);
        assert_ne!(
            window.start().unwrap(),
            window.phase_start
                + f64::from((window.sector_transition.unwrap() - window.phase_start) as f32)
        );
    }
}
