use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static HIGH_WATER_BYTES: AtomicUsize = AtomicUsize::new(0);
static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

pub fn track_allocations(enabled: bool) {
    TRACK_ALLOCATIONS.store(enabled, Ordering::Relaxed);
}

// Successful allocation/reallocation requests and their requested sizes. A
// reallocation counts its complete new size, not just retained heap growth.
pub fn allocation_totals() -> (u64, u64) {
    (
        ALLOCATIONS.load(Ordering::Relaxed),
        ALLOCATED_BYTES.load(Ordering::Relaxed),
    )
}

fn allocation(bytes: usize) {
    if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

pub struct MeasuredAllocator;

fn retain(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    HIGH_WATER_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocation(layout.size());
            retain(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocation(layout.size());
            retain(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() {
            allocation(size);
            if size >= layout.size() {
                retain(size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - size, Ordering::Relaxed);
            }
        }
        replacement
    }
}

pub fn live_bytes() -> usize {
    LIVE_BYTES.load(Ordering::Relaxed)
}

pub fn high_water_bytes() -> usize {
    HIGH_WATER_BYTES.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_allocator_tracks_growth_shrink_and_release() {
        track_allocations(true);
        let initial_totals = allocation_totals();
        let baseline = live_bytes();
        let layout = Layout::from_size_align(256, 8).unwrap();
        let pointer = unsafe { MeasuredAllocator.alloc(layout) };
        assert!(!pointer.is_null());
        assert_eq!(live_bytes(), baseline + 256);
        let pointer = unsafe { MeasuredAllocator.realloc(pointer, layout, 512) };
        assert_eq!(live_bytes(), baseline + 512);
        let grown = Layout::from_size_align(512, 8).unwrap();
        let pointer = unsafe { MeasuredAllocator.realloc(pointer, grown, 64) };
        assert_eq!(live_bytes(), baseline + 64);
        unsafe { MeasuredAllocator.dealloc(pointer, Layout::from_size_align(64, 8).unwrap()) };
        assert_eq!(live_bytes(), baseline);
        assert!(high_water_bytes() >= baseline + 512);
        assert_eq!(
            allocation_totals(),
            (initial_totals.0 + 3, initial_totals.1 + 256 + 512 + 64)
        );
        track_allocations(false);
        let before = allocation_totals();
        let pointer = unsafe { MeasuredAllocator.alloc_zeroed(layout) };
        assert!(!pointer.is_null());
        unsafe { MeasuredAllocator.dealloc(pointer, layout) };
        assert_eq!(allocation_totals(), before);
    }
}
