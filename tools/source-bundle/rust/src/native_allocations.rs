//! Calling-thread allocation requests during native diagnostic observations.
//! Not RSS, committed memory, a WASM heap measurement, or a GC counter.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! { static COUNTS: Cell<Option<(u64, u64)>> = const { Cell::new(None) }; }
pub struct Allocator;
fn requested(size: usize) {
    let _ = COUNTS.try_with(|counts| {
        if let Some((requests, bytes)) = counts.get() { counts.set(Some((requests + 1, bytes + size as u64))); }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() { requested(layout.size()); }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() { requested(layout.size()); }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() { requested(size); }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) { unsafe { System.dealloc(pointer, layout) }; }
}
pub fn measure<T>(operation: impl FnOnce() -> T) -> (T, (u64, u64)) {
    struct Reset;
    impl Drop for Reset { fn drop(&mut self) { COUNTS.with(|counts| counts.set(None)); } }
    COUNTS.with(|counts| { assert!(counts.get().is_none()); counts.set(Some((0, 0))); });
    let _reset = Reset;
    let value = operation();
    (value, COUNTS.with(|counts| counts.get().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_requests_and_full_reallocation_sizes_only_within_the_scope() {
        let layout = Layout::from_size_align(16, 8).unwrap();
        let (_, counts) = measure(|| unsafe {
            let pointer = Allocator.alloc_zeroed(layout);
            assert!(!pointer.is_null());
            let pointer = Allocator.realloc(pointer, layout, 64);
            assert!(!pointer.is_null());
            Allocator.dealloc(pointer, Layout::from_size_align(64, 8).unwrap());
        });
        assert_eq!(counts, (2, 80));
        assert_eq!(measure(|| 7), (7, (0, 0)));
    }
}
