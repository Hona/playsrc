use super::*;
use playsrc_simulation::MetricsClock;

const MAX_EVENTS: usize = 8192;
const EVENT_BYTES: usize = 56;

struct Recorder {
    bytes: Vec<u8>,
    dropped: u32,
    clock: RuntimeMetricsClock,
}

impl Recorder {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_EVENTS * EVENT_BYTES),
            dropped: 0,
            clock: RuntimeMetricsClock::new(),
        }
    }

    fn append(
        &mut self,
        event: playsrc_tf2::admission_metrics::Event,
        at: u64,
        live: u64,
        allocations: (u64, u64),
    ) {
        if self.bytes.len() == MAX_EVENTS * EVENT_BYTES {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.bytes.extend_from_slice(&event.stage.to_le_bytes());
        self.bytes.extend_from_slice(&event.actor.to_le_bytes());
        for value in [
            event.tick,
            at,
            live,
            allocations.0,
            allocations.1,
            event.value,
        ] {
            self.bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn records() -> &'static Mutex<Option<Recorder>> {
    static RECORDS: OnceLock<Mutex<Option<Recorder>>> = OnceLock::new();
    RECORDS.get_or_init(|| Mutex::new(None))
}

pub(super) fn begin() {
    *records().lock().expect("admission metrics") = Some(Recorder::new());
    memory::track_allocations(true);
    playsrc_tf2::admission_metrics::set_observer(Some(record));
}

pub(super) fn stop() {
    playsrc_tf2::admission_metrics::set_observer(None);
    memory::track_allocations(false);
}

pub(super) fn dispose() {
    stop();
    *records().lock().expect("admission metrics") = None;
}

fn record(event: playsrc_tf2::admission_metrics::Event) {
    let mut records = records().lock().expect("admission metrics");
    let Some(records) = records.as_mut() else {
        return;
    };
    let at = records.clock.monotonic_nanoseconds();
    let allocations = memory::allocation_totals();
    records.append(event, at, memory::live_bytes() as u64, allocations);
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_admission_metrics_length() -> usize {
    records()
        .lock()
        .expect("admission metrics")
        .as_ref()
        .map_or(0, |records| records.bytes.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_admission_metrics_dropped() -> u32 {
    records()
        .lock()
        .expect("admission metrics")
        .as_ref()
        .map_or(0, |records| records.dropped)
}

#[unsafe(no_mangle)]
/// # Safety
/// The caller supplies `capacity` writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_admission_metrics_copy(
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    let records = records().lock().expect("admission metrics");
    let Some(records) = records.as_ref() else {
        return 0;
    };
    if pointer.is_null() || capacity < records.bytes.len() {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(records.bytes.as_ptr(), pointer, records.bytes.len());
    }
    records.bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn measurement_scope_excludes_competing_stop_but_counts_worker_allocations() {
        use std::alloc::{GlobalAlloc, Layout};
        use std::sync::{TryLockError, mpsc};

        std::thread::scope(|threads| {
            let metrics = memory::TEST_METRICS.lock().expect("test metrics");
            begin();
            let initial_totals = memory::allocation_totals();
            let baseline = memory::live_bytes();
            let (attempted, attempt) = mpsc::channel();
            threads.spawn(move || {
                // Force a competing scope to arrive while measurement is active,
                // without timing sleeps or serializing unrelated native tests.
                attempted
                    .send(matches!(
                        memory::TEST_METRICS.try_lock(),
                        Err(TryLockError::WouldBlock)
                    ))
                    .unwrap();
                let _metrics = memory::TEST_METRICS.lock().expect("test metrics");
                stop();
            });
            assert!(attempt.recv().unwrap());

            let allocate = move || {
                let layout = Layout::from_size_align(256, 8).unwrap();
                let pointer = unsafe { memory::MeasuredAllocator.alloc_zeroed(layout) };
                assert!(!pointer.is_null());
                assert_eq!(memory::live_bytes(), baseline + 256);
                unsafe { memory::MeasuredAllocator.dealloc(pointer, layout) };
            };
            // Workers belong to the owning measurement, not separate test scopes.
            threads.spawn(allocate).join().unwrap();
            let measured = (initial_totals.0 + 1, initial_totals.1 + 256);
            assert_eq!(memory::allocation_totals(), measured);
            assert_eq!(memory::live_bytes(), baseline);
            assert!(memory::high_water_bytes() >= baseline + 256);

            // Tracking disable is global too; it must not disable live accounting.
            threads.spawn(stop).join().unwrap();
            threads.spawn(allocate).join().unwrap();
            assert_eq!(memory::allocation_totals(), measured);
            assert_eq!(memory::live_bytes(), baseline);
            dispose();
            drop(metrics);
        });
    }

    #[test]
    fn records_exact_timestamps_actor_ticks_counters_and_overflow_without_growth() {
        let mut records = Recorder::new();
        let capacity = records.bytes.capacity();
        for index in 0..MAX_EVENTS + 3 {
            records.append(
                playsrc_tf2::admission_metrics::Event {
                    tick: index as u64,
                    stage: 2,
                    actor: 17,
                    value: 4096,
                },
                index as u64 * 5,
                9000,
                (42, 100),
            );
        }
        assert_eq!(records.bytes.len(), MAX_EVENTS * EVENT_BYTES);
        assert_eq!(records.bytes.capacity(), capacity);
        assert_eq!(records.dropped, 3);
        for (index, row) in records.bytes.chunks_exact(EVENT_BYTES).enumerate() {
            assert_eq!(u32::from_le_bytes(row[..4].try_into().unwrap()), 2);
            assert_eq!(u32::from_le_bytes(row[4..8].try_into().unwrap()), 17);
            let values: Vec<_> = row[8..]
                .chunks_exact(8)
                .map(|bytes| u64::from_le_bytes(bytes.try_into().unwrap()))
                .collect();
            assert_eq!(
                values,
                [index as u64, index as u64 * 5, 9000, 42, 100, 4096]
            );
        }
    }

    #[test]
    fn retained_clock_is_monotonic() {
        let mut recorder = Recorder::new();
        let first = recorder.clock.monotonic_nanoseconds();
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(recorder.clock.monotonic_nanoseconds() >= first + 1_000_000);
    }
}
