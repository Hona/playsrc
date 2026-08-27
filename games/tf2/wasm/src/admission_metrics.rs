use super::*;
use playsrc_simulation::MetricsClock;

const MAX_EVENTS: usize = 8192;
const EVENT_BYTES: usize = 40;
fn records() -> &'static Mutex<Vec<u8>> {
    static RECORDS: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    RECORDS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(super) fn begin() {
    *records().lock().expect("admission metrics") = Vec::with_capacity(MAX_EVENTS * EVENT_BYTES);
    playsrc_tf2::admission_metrics::set_observer(Some(record));
}

fn record(event: playsrc_tf2::admission_metrics::Event) {
    let at = RuntimeMetricsClock::new().monotonic_nanoseconds();
    let live = memory::live_bytes() as u64;
    let mut records = records().lock().expect("admission metrics");
    if records.len() == MAX_EVENTS * EVENT_BYTES { return; }
    records.extend_from_slice(&event.stage.to_le_bytes());
    records.extend_from_slice(&event.actor.to_le_bytes());
    records.extend_from_slice(&event.tick.to_le_bytes());
    records.extend_from_slice(&at.to_le_bytes());
    records.extend_from_slice(&live.to_le_bytes());
    records.extend_from_slice(&0_u64.to_le_bytes());
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_admission_metrics_length() -> usize {
    records().lock().expect("admission metrics").len()
}

#[unsafe(no_mangle)]
/// # Safety
/// The caller supplies `capacity` writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_admission_metrics_copy(pointer: *mut u8, capacity: usize) -> usize {
    let records = records().lock().expect("admission metrics");
    if pointer.is_null() || capacity < records.len() { return 0; }
    unsafe { std::ptr::copy_nonoverlapping(records.as_ptr(), pointer, records.len()); }
    records.len()
}
