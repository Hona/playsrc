//! Exact retained query inputs, fresh process per invocation. No game clock.
use playsrc_collision::{compile, replay_diagnostics::ordered_brushes, Hull};
use sha2::{Digest, Sha256};
use std::{alloc::{GlobalAlloc, Layout, System}, cell::Cell, hint::black_box, time::Instant};

struct Allocator;
thread_local! { static COUNTS: Cell<Option<(u64, u64)>> = const { Cell::new(None) }; }
fn count(size: usize) { let _ = COUNTS.try_with(|v| { if let Some((n, b)) = v.get() { v.set(Some((n + 1, b + size as u64))); } }); }
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 { let p = unsafe { System.alloc(l) }; if !p.is_null() { count(l.size()); } p }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 { let p = unsafe { System.alloc_zeroed(l) }; if !p.is_null() { count(l.size()); } p }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, size: usize) -> *mut u8 { let p = unsafe { System.realloc(p, l, size) }; if !p.is_null() { count(size); } p }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { unsafe { System.dealloc(p, l) }; }
}
#[global_allocator] static ALLOCATOR: Allocator = Allocator;

fn cpu_ns() -> u64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    assert_eq!(unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) }, 0);
    t.tv_sec as u64 * 1_000_000_000 + t.tv_nsec as u64
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "brush_queries bsp query-file iterations");
    let bsp = std::fs::read(&args[1]).unwrap();
    let bytes = std::fs::read(&args[2]).unwrap();
    assert_eq!(&bytes[..4], b"BQR1");
    assert_eq!(&bytes[4..36], Sha256::digest(&bsp).as_slice());
    assert_eq!((bytes.len() - 36) % 52, 0);
    let queries: Vec<_> = bytes[36..].chunks_exact(52).map(|row| {
        let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap());
        let v = |i| [f(i), f(i + 4), f(i + 8)];
        (i32::from_le_bytes(row[..4].try_into().unwrap()), v(4), v(16), Hull { mins: v(28), maxs: v(40) })
    }).collect();
    let parsed = playsrc_bsp::parse(&bsp, playsrc_bsp::Profile::Source2013V20, playsrc_bsp::Limits::default()).unwrap();
    let world = compile(&parsed).unwrap();
    let mut digest = Sha256::new();
    // Authenticate every encounter, not just the nearest hit or a count. This
    // separate pass also warms the exact workload; no hashing in timed loops.
    for &(head, start, end, hull) in &queries {
        let order = ordered_brushes(&world, head, start, end, hull).unwrap();
        digest.update((order.len() as u32).to_le_bytes());
        for brush in order { digest.update((brush as u32).to_le_bytes()); }
    }
    let hash: String = digest.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let iterations: usize = args[3].parse().unwrap();
    assert!((1..=20).contains(&iterations));
    for iteration in 0..iterations {
        COUNTS.set(Some((0, 0)));
        let cpu = cpu_ns(); let wall = Instant::now();
        for &(head, start, end, hull) in &queries {
            black_box(ordered_brushes(black_box(&world), head, start, end, hull).unwrap());
        }
        let wall_ns = wall.elapsed().as_nanos(); let cpu_ns = cpu_ns() - cpu;
        let (requests, requested_bytes) = COUNTS.replace(None).unwrap();
        println!("{{\"iteration\":{iteration},\"queries\":{},\"orderSha256\":\"{hash}\",\"wallNs\":{wall_ns},\"cpuNs\":{cpu_ns},\"requests\":{requests},\"requestedBytes\":{requested_bytes}}}", queries.len());
    }
}
