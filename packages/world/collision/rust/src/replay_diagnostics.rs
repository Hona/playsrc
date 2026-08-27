//! Offline differential replay only; absent from ordinary game builds.
use std::cell::Cell;
/// Triangles, hierarchy nodes, distinct edge planes, retained heap bytes.
pub fn displacement_storage(world: &crate::World) -> [usize; 4] {
    crate::displacement::storage(world)
}
thread_local! {
    static REFERENCE: Cell<bool> = const { Cell::new(false) };
    static DISPLACEMENT_REFERENCE: Cell<bool> = const { Cell::new(false) };
    static COUNTERS: Cell<[u64; 11]> = const { Cell::new([0; 11]) };
}
pub fn reference() -> bool {
    REFERENCE.get()
}
pub fn select_reference(reference: bool) {
    REFERENCE.set(reference);
    reset();
}
pub fn displacement_reference() -> bool {
    DISPLACEMENT_REFERENCE.get()
}
pub fn select_displacement_reference(reference: bool) {
    DISPLACEMENT_REFERENCE.set(reference);
    reset();
}
pub fn reset() {
    COUNTERS.set([0; 11]);
}
pub fn counters() -> [u64; 11] {
    COUNTERS.get()
}
pub(crate) fn count(index: usize, amount: usize) {
    COUNTERS.with(|value| {
        let mut counts = value.get();
        counts[index] += amount as u64;
        value.set(counts);
    });
}
pub(crate) struct Planes<I> {
    inner: I,
    visited: usize,
}
impl<I: Iterator> Planes<I> {
    pub fn new(inner: I) -> Self {
        Self { inner, visited: 0 }
    }
}
impl<I: Iterator> Iterator for Planes<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.inner.next();
        self.visited += usize::from(value.is_some());
        value
    }
}
impl<I> Drop for Planes<I> {
    fn drop(&mut self) {
        count(4, self.visited);
    }
}
