#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// Supplies monotonic nanoseconds for diagnostic phase measurements only.
///
/// Values must never decrease. They cannot affect scheduling, command admission,
/// simulation inputs, publications, faults, or state identity.
pub trait MetricsClock: Send {
    fn monotonic_nanoseconds(&mut self) -> u64;
}

/// The default native metrics clock backed by [`std::time::Instant`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct NativeMetricsClock {
    origin: Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for NativeMetricsClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl MetricsClock for NativeMetricsClock {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        self.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
}

/// Uninhabited on bare WebAssembly because that target has no ambient monotonic clock.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug)]
pub enum NativeMetricsClock {}

#[cfg(target_arch = "wasm32")]
impl MetricsClock for NativeMetricsClock {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        match *self {}
    }
}

pub(crate) fn elapsed_nanoseconds(start: u64, end: u64) -> u64 {
    end.saturating_sub(start)
}
