use crate::{CollisionMotion, continuous::SORTED_EVENT_CAPACITY};
use std::fmt;
const EMPTY_DISTANCE: f32 = f32::from_bits(0x5015_02f9);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovementRangeClock {
    pub time: f64,
    pub total_gradient: f32,
    pub center_gradient: f32,
    pub total: f32,
    pub center: f32,
    pub next: f32,
    pub reset_at: i32,
}

impl MovementRangeClock {
    pub fn key(self, time: f64, distance: f64) -> Result<f32, MovementRangeError> {
        if !time.is_finite() || !distance.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        let current = ((time - self.time) as f32) * self.total_gradient + self.total;
        let key = (f64::from(current) + distance) as f32;
        if !key.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        Ok(key)
    }
    pub fn rotation_travel(self, time: f64) -> Result<f64, MovementRangeError> {
        if !time.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        let elapsed = (time - self.time) as f32;
        let gradient = self.total_gradient - self.center_gradient;
        let prior = self.total - self.center;
        let result = gradient * elapsed + prior;
        if !result.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        Ok(f64::from(result))
    }

    pub fn projected_rotation_travel(self, time: f64) -> Result<f64, MovementRangeError> {
        if !time.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        let elapsed = (time - self.time) as f32;
        let gradient = self.total_gradient - self.center_gradient;
        let prior = self.total - self.center;
        let result = (f64::from(gradient) * f64::from(elapsed) + f64::from(prior)) as f32;
        if !result.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        Ok(f64::from(result))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeListener {
    pub identity: u64,
    pub slot: u16,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeCallback {
    pub identity: u64,
    pub intrusion: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeReset {
    pub identity: u64,
    pub total_shift: f32,
    pub center_shift: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RangeDispatch {
    pub callbacks: usize,
    pub budget_exhausted: bool,
}

pub(crate) struct RangeBudget {
    remaining: i32,
    supplied: i32,
    callbacks: usize,
}
impl RangeBudget {
    pub(crate) fn new(budget: u32) -> Result<Self, MovementRangeError> {
        let remaining = i32::try_from(budget).map_err(|_| MovementRangeError::InvalidBudget)?;
        Ok(Self {
            remaining,
            supplied: remaining,
            callbacks: 0,
        })
    }
    pub(crate) fn delivered(&mut self, additional: impl FnOnce(i32) -> i32) -> bool {
        self.callbacks += 1;
        self.remaining = self.remaining.wrapping_sub(1);
        if self.remaining < 0 {
            self.remaining = self.remaining.wrapping_add(additional(self.supplied));
            if self.remaining < 0 {
                return false;
            }
            self.supplied = self.supplied.wrapping_add(self.remaining.wrapping_add(1));
        }
        true
    }
    pub(crate) fn result(self, budget_exhausted: bool) -> RangeDispatch {
        RangeDispatch {
            callbacks: self.callbacks,
            budget_exhausted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementRangeError {
    NonFinite,
    InvalidTime,
    InvalidCapacity,
    Capacity,
    DuplicateIdentity,
    MissingIdentity,
    InvalidBudget,
    KeyRange,
}
impl fmt::Display for MovementRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "movement range contains a non-finite value",
            Self::InvalidTime => "movement range time or duration is invalid",
            Self::InvalidCapacity => "movement listener capacity must be between one and 65532",
            Self::Capacity => "movement listener capacity exhausted",
            Self::DuplicateIdentity => "movement listener identity is already present",
            Self::MissingIdentity => "movement listener identity is absent",
            Self::InvalidBudget => "movement listener budget is outside the signed counter domain",
            Self::KeyRange => "the first movement listener cannot exceed the empty-list sentinel",
        })
    }
}
impl std::error::Error for MovementRangeError {}

#[derive(Clone, Debug, PartialEq)]
pub struct MovementRange {
    clock: MovementRangeClock,
    minimum: f32,
    listeners: Vec<RangeListener>,
    free: Vec<u16>,
    allocated: usize,
    maximum: usize,
}

impl MovementRange {
    pub(crate) fn shift_clock_origin(&mut self, shift: f64) {
        self.clock.time -= shift;
    }
    pub(crate) fn shift_position(&mut self, distance: f32) -> Result<(), MovementRangeError> {
        if !distance.is_finite() || distance < 0.0 {
            return Err(MovementRangeError::NonFinite);
        }
        let total = self.clock.next + distance;
        let center = self.clock.center + distance;
        if !total.is_finite() || !center.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        self.clock.total_gradient = 0.0;
        self.clock.center_gradient = 0.0;
        self.clock.next = total;
        self.clock.total = total;
        self.clock.center = center;
        Ok(())
    }
    pub fn new(maximum: usize) -> Result<Self, MovementRangeError> {
        if maximum == 0 || maximum > SORTED_EVENT_CAPACITY {
            return Err(MovementRangeError::InvalidCapacity);
        }
        Ok(Self {
            clock: MovementRangeClock::default(),
            minimum: EMPTY_DISTANCE,
            listeners: Vec::new(),
            free: (0..8).rev().collect(),
            allocated: 8,
            maximum,
        })
    }
    pub fn clock(&self) -> MovementRangeClock {
        self.clock
    }
    pub fn minimum(&self) -> f32 {
        self.minimum
    }
    pub fn listeners(&self) -> &[RangeListener] {
        &self.listeners
    }
    pub fn is_due(&self) -> bool {
        self.minimum - self.clock.next < 0.0
    }
    pub(crate) fn due_callback(&self) -> Result<Option<RangeCallback>, MovementRangeError> {
        if !self.is_due() {
            return Ok(None);
        }
        Ok(Some(RangeCallback {
            identity: self
                .listeners
                .first()
                .ok_or(MovementRangeError::MissingIdentity)?
                .identity,
            intrusion: self.minimum - self.clock.next,
        }))
    }

    pub fn advance(
        &mut self,
        time: f64,
        duration: f32,
        motion: CollisionMotion,
    ) -> Result<bool, MovementRangeError> {
        if !time.is_finite() || !duration.is_finite() || motion.validate().is_err() {
            return Err(MovementRangeError::NonFinite);
        }
        if duration <= 0.0 {
            return Err(MovementRangeError::InvalidTime);
        }
        let elapsed = (time - self.clock.time) as f32;
        let total = self.clock.total + self.clock.total_gradient * elapsed;
        let center = self.clock.center + self.clock.center_gradient * elapsed;
        let total_gradient = (motion.rotation.surface_speed + motion.linear_speed) * 1.00001_f32;
        let next = total + duration * total_gradient;
        if [total, center, total_gradient, next]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(MovementRangeError::NonFinite);
        }
        self.clock = MovementRangeClock {
            time,
            total_gradient,
            center_gradient: motion.linear_speed,
            total,
            center,
            next,
            reset_at: self.clock.reset_at,
        };
        Ok(self.is_due())
    }

    pub fn insert(&mut self, identity: u64, key: f32) -> Result<u16, MovementRangeError> {
        if !key.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        if self.listeners.is_empty() && key > self.minimum {
            return Err(MovementRangeError::KeyRange);
        }
        if self
            .listeners
            .iter()
            .any(|listener| listener.identity == identity)
        {
            return Err(MovementRangeError::DuplicateIdentity);
        }
        if self.listeners.len() == self.maximum {
            return Err(MovementRangeError::Capacity);
        }
        if self.free.is_empty() {
            let capacity = (self.allocated * 2 + 1).min(SORTED_EVENT_CAPACITY);
            self.free
                .extend((self.allocated..capacity).rev().map(|index| index as u16));
            self.allocated = capacity;
        }
        let slot = self.free.pop().expect("bounded listener allocator");
        let index = self
            .listeners
            .partition_point(|listener| listener.distance < key);
        self.listeners.insert(
            index,
            RangeListener {
                identity,
                slot,
                distance: key,
            },
        );
        self.minimum = self.listeners[0].distance;
        Ok(slot)
    }
    pub fn stop_gradients(&mut self, time: f64) -> Result<(), MovementRangeError> {
        if !time.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        if time < self.clock.time {
            return Err(MovementRangeError::InvalidTime);
        }
        let elapsed = (time - self.clock.time) as f32;
        let total = self.clock.total_gradient * elapsed + self.clock.total;
        let center = self.clock.center_gradient * elapsed + self.clock.center;
        if !total.is_finite() || !center.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        self.clock.total = total;
        self.clock.center = center;
        self.clock.total_gradient = 0.0;
        self.clock.center_gradient = 0.0;
        Ok(())
    }

    pub fn remove(&mut self, identity: u64) -> Result<(), MovementRangeError> {
        let index = self
            .listeners
            .iter()
            .position(|listener| listener.identity == identity)
            .ok_or(MovementRangeError::MissingIdentity)?;
        let listener = self.listeners.remove(index);
        self.free.push(listener.slot);
        if index == 0 {
            self.minimum = self
                .listeners
                .first()
                .map_or(EMPTY_DISTANCE, |listener| listener.distance);
        }
        Ok(())
    }

    pub fn renew(
        &mut self,
        identity: u64,
        time: f64,
        distance: f64,
    ) -> Result<u16, MovementRangeError> {
        let key = self.clock.key(time, distance)?;
        if self.listeners.len() == 1 && key > EMPTY_DISTANCE {
            return Err(MovementRangeError::KeyRange);
        }
        self.remove(identity)?;
        self.insert(identity, key)
    }

    /// Listeners may remove/renew themselves or other listeners during delivery.
    /// Additional checks use the Source solver callback's cumulative budget argument.
    pub fn dispatch(
        &mut self,
        budget: u32,
        mut callback: impl FnMut(&mut Self, RangeCallback) -> Result<(), MovementRangeError>,
        mut additional: impl FnMut(i32) -> i32,
    ) -> Result<RangeDispatch, MovementRangeError> {
        let mut budget = RangeBudget::new(budget)?;
        while let Some(event) = self.due_callback()? {
            callback(self, event)?;
            if !budget.delivered(&mut additional) {
                return Ok(budget.result(true));
            }
        }
        Ok(budget.result(false))
    }

    /// Called after the active movement range finishes delivering its due listeners.
    pub fn reset_if_due(
        &mut self,
        notify: impl FnMut(RangeReset),
    ) -> Result<bool, MovementRangeError> {
        if self.clock.time <= f64::from(self.clock.reset_at) {
            return Ok(false);
        }
        let next_reset = self.clock.time + 10.0;
        if next_reset >= 2147483648.0 {
            return Err(MovementRangeError::InvalidTime);
        }
        self.reset_values(notify);
        self.clock.reset_at = next_reset as i32;
        Ok(true)
    }
    pub(crate) fn reset_values(&mut self, mut notify: impl FnMut(RangeReset)) {
        let total_shift = -self.clock.total;
        let center_shift = -self.clock.center;
        for listener in &mut self.listeners {
            listener.distance += total_shift;
        }
        for listener in &self.listeners {
            notify(RangeReset {
                identity: listener.identity,
                total_shift,
                center_shift,
            });
        }
        self.minimum += total_shift;
        self.clock.total = 0.0;
        self.clock.center = 0.0;
        self.clock.next += total_shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_due_keys_and_equal_key_renewal_preserve_order_and_slots() {
        let mut range = MovementRange::new(4).unwrap();
        assert_eq!(range.insert(1, 0.0).unwrap(), 0);
        assert_eq!(range.insert(2, -0.0).unwrap(), 1);
        assert!(!range.is_due());
        assert_eq!(range.renew(1, 0.0, 0.0).unwrap(), 0);
        assert_eq!(
            range
                .listeners()
                .iter()
                .map(|v| v.identity)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        let before = range.clone();
        assert!(range.renew(1, f64::NAN, 1.0).is_err());
        assert_eq!(range, before);
    }
    #[test]
    fn callbacks_consume_one_more_than_the_budget_before_requesting_an_extension() {
        let mut range = MovementRange::new(1).unwrap();
        range.insert(1, -1.0).unwrap();
        let mut requests = Vec::new();
        let result = range
            .dispatch(
                2,
                |_, _| Ok(()),
                |count| {
                    requests.push(count);
                    if count == 2 { 3 } else { 0 }
                },
            )
            .unwrap();
        assert_eq!(requests, [2, 5]);
        assert_eq!(
            result,
            RangeDispatch {
                callbacks: 6,
                budget_exhausted: true
            }
        );
    }

    #[test]
    fn a_live_queue_accepts_the_large_provisional_connector_key() {
        let mut range = MovementRange::new(4).unwrap();
        let large = f32::from_bits(0x60ad_78ec);
        assert_eq!(range.insert(1, large), Err(MovementRangeError::KeyRange));
        range.insert(1, 0.0).unwrap();
        range.insert(2, large).unwrap();
        assert_eq!(range.listeners()[1].distance, large);
        range.remove(1).unwrap();
        let before = range.clone();
        assert_eq!(
            range.renew(2, 0.0, f64::from(large)),
            Err(MovementRangeError::KeyRange)
        );
        assert_eq!(range, before);
    }

    #[test]
    fn admitted_and_projected_rotation_travel_keep_their_distinct_accumulation_widths() {
        let clock = MovementRangeClock {
            total_gradient: 0.1,
            total: 0.01,
            ..MovementRangeClock::default()
        };
        let time = f64::from(0.03_f32);
        assert_eq!(
            (clock.rotation_travel(time).unwrap() as f32).to_bits(),
            0x3c54_fdf4
        );
        assert_eq!(
            (clock.projected_rotation_travel(time).unwrap() as f32).to_bits(),
            0x3c54_fdf3
        );
    }
}
