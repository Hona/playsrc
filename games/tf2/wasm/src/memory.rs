use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static HIGH_WATER_BYTES: AtomicUsize = AtomicUsize::new(0);

pub struct MeasuredAllocator;

fn retain(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    HIGH_WATER_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            retain(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
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
    }
}
