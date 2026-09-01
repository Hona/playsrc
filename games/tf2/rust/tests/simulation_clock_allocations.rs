use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

use playsrc_collision::Hull;
use playsrc_movement::{Error as MoveError, Trace, Tracer};
use playsrc_tf2::{Command, GameplayWorld, MapRuntime, RocketTraceResult, Session};

struct CountingAllocator;
thread_local! {
    static COUNTS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

fn allocated(bytes: usize) {
    let _ = COUNTS.try_with(|counts| {
        if let Some((calls, total)) = counts.get() {
            counts.set(Some((calls + 1, total + bytes)));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() { allocated(layout.size()); }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() { allocated(layout.size()); }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() { allocated(size); }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure<T>(operation: impl FnOnce() -> T) -> (T, (usize, usize)) {
    struct Reset;
    impl Drop for Reset { fn drop(&mut self) { COUNTS.set(None); } }
    assert!(COUNTS.replace(Some((0, 0))).is_none());
    let _reset = Reset;
    let value = operation();
    (value, COUNTS.replace(None).unwrap())
}

#[derive(Clone)]
struct EmptyWorld;
impl Tracer for EmptyWorld {
    fn trace(&self, _: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, MoveError> {
        Ok(Trace { fraction: 1.0, start_solid: false, all_solid: false, end,
            normal: None, hit: None, contents: 0 })
    }
}
impl GameplayWorld for EmptyWorld {
    fn overlaps_model_hull(&self, _: usize, _: [f32; 3], _: [f32; 3], _: Hull) -> Result<bool, MoveError> {
        Ok(false)
    }
}

// The two pre-advance clock reads in the compiled gameplay transaction used to
// build full producer snapshots. Compare those reads on identical live states,
// including outstanding projectile requests, without measuring test setup or
// substituting this allocation test for headed gameplay evidence.
#[test]
fn pre_advance_clock_reads_do_not_copy_producer_state() {
    let mut session = Session::new(EmptyWorld, [0.0, 0.0, 128.0], MapRuntime::empty(0.015));
    let mut removed = (0, 0);
    let mut projectile_ticks = 0;
    for tick in 0..120 {
        let before = session.producer_snapshot();
        let random = session.random_state();
        let movement = session.movement_snapshot_bytes();
        let (old, counts) = measure(|| {
            let trace_tick = black_box(&session).producer_snapshot().tick;
            let movement_time = black_box(&session).producer_snapshot().tick as f32 * 0.015;
            (trace_tick, movement_time.to_bits())
        });
        let (current, direct_counts) = measure(|| {
            let trace_tick = black_box(&session).tick();
            let movement_time = black_box(&session).tick() as f32 * 0.015;
            (trace_tick, movement_time.to_bits())
        });
        assert_eq!(old, current);
        assert_eq!(current.0, tick);
        assert_eq!(direct_counts, (0, 0));
        assert!(counts.0 >= 2 && counts.1 > 0);
        removed.0 += counts.0;
        removed.1 += counts.1;
        assert_eq!(session.producer_snapshot(), before);
        assert_eq!(session.random_state(), random);
        assert_eq!(session.movement_snapshot_bytes(), movement);
        projectile_ticks += usize::from(!before.projectiles.is_empty());

        let results: Vec<_> = session.rocket_trace_requests().iter().map(|request| RocketTraceResult {
            projectile: request.projectile, tick: current.0, end: request.end,
            solid: tick % 29 == 0, sky: tick % 29 == 0,
            normal: (tick % 29 == 0).then_some([0.0, 0.0, 1.0]), direct_target: None,
        }).collect();
        if !results.is_empty() {
            // Failed pending-results validation must retain the live clock and
            // every producer record, rather than acknowledging another tick.
            assert!(session.advance(Command::default()).is_err());
            assert_eq!(session.tick(), tick);
            assert_eq!(session.producer_snapshot(), before);
            assert_eq!(session.random_state(), random);
        }
        let command = Command { fire: tick >= 35, respawn: tick == 90, ..Command::default() };
        let mut reference = session.clone();
        let expected = reference.advance_with_external(command, &results, None).unwrap();
        let actual = session.advance_with_external(command, &results, None).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(session.producer_snapshot(), reference.producer_snapshot());
    }
    assert!(projectile_ticks > 0, "the test must exercise live projectile state");
    println!("120 ticks: removed {} allocation requests / {} requested bytes; {projectile_ticks} projectile ticks; direct clock reads allocate zero", removed.0, removed.1);
}
