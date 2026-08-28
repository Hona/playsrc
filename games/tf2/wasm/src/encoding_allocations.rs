//! Thread-local native test instrumentation; never part of the game allocator.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNTS: Cell<Option<Metrics>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics { pub requests: usize, pub bytes: usize, pub live: isize, pub peak: isize }

pub struct Allocator;

fn requested(size: usize, freed: usize, request: bool) {
    let _ = COUNTS.try_with(|counts| {
        if let Some(mut metrics) = counts.get() {
            metrics.requests += usize::from(request);
            metrics.bytes += size;
            metrics.live += size as isize - freed as isize;
            metrics.peak = metrics.peak.max(metrics.live);
            counts.set(Some(metrics));
        }
    });
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() { requested(layout.size(), 0, true); }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() { requested(layout.size(), 0, true); }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() { requested(size, layout.size(), true); }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        requested(0, layout.size(), false);
        unsafe { System.dealloc(pointer, layout) };
    }
}

pub fn measure<T>(encode: impl FnOnce() -> T) -> (T, Metrics) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) { COUNTS.with(|counts| counts.set(None)); }
    }
    COUNTS.with(|counts| { assert!(counts.get().is_none()); counts.set(Some(Metrics::default())); });
    let _reset = Reset;
    let value = encode();
    let metrics = COUNTS.with(|counts| counts.get().unwrap());
    (value, metrics)
}
