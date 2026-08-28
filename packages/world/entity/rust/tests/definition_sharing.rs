use playsrc_entity::{
    ContactKind, ContactRecord, Entity, EntityWorld, EntityWorldConfig, EventTarget, InputRecord,
    Limits, RuntimeRequest, Transition, Variant, WorldCommand, parse,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

#[derive(Clone, Copy, Debug, Default)]
struct Metrics {
    requests: usize,
    bytes: usize,
    live: isize,
    peak: isize,
}
thread_local! { static METRICS: Cell<Metrics> = const { Cell::new(Metrics { requests: 0, bytes: 0, live: 0, peak: 0 }) }; }
thread_local! {
    static PEAKS: Cell<[isize; 8]> = const { Cell::new([0; 8]) };
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
fn record(allocated: usize, freed: usize, request: bool) {
    let _ = METRICS.try_with(|cell| {
        let mut m = cell.get();
        m.requests += usize::from(request);
        m.bytes += allocated;
        m.live += allocated as isize - freed as isize;
        m.peak = m.peak.max(m.live);
        cell.set(m);
        let _ = PEAKS.try_with(|cell| {
            let mut peaks = cell.get();
            for peak in &mut peaks[..DEPTH.get()] {
                *peak = (*peak).max(m.live);
            }
            cell.set(peaks);
        });
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            record(l.size(), 0, true);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            record(l.size(), 0, true);
        }
        p
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(p, l, size) };
        if !p.is_null() {
            record(size, l.size(), true);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        record(0, l.size(), false);
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn measure<T>(f: impl FnOnce() -> T) -> (T, Metrics) {
    let before = METRICS.get();
    let depth = DEPTH.get();
    let mut peaks = PEAKS.get();
    peaks[depth] = before.live;
    PEAKS.set(peaks);
    DEPTH.set(depth + 1);
    struct Reset(usize);
    impl Drop for Reset {
        fn drop(&mut self) {
            DEPTH.set(self.0);
        }
    }
    let _reset = Reset(depth);
    let result = f();
    let after = METRICS.get();
    (
        result,
        Metrics {
            requests: after.requests - before.requests,
            bytes: after.bytes - before.bytes,
            live: after.live - before.live,
            peak: PEAKS.get()[depth] - before.live,
        },
    )
}

fn fixture(extra_pairs: usize) -> EntityWorld {
    // Unbound authored keys change only the installed definition, not runtime
    // fields, outputs or behavior. Their slope isolates definition copying.
    let padding = (0..extra_pairs)
        .map(|i| format!("\"unused_{i}\"\"{}\"", "x".repeat(128)))
        .collect::<String>();
    let text = format!(
        "{{\"classname\"\"player\"\"targetname\"\"subject\"}}\
        {{\"classname\"\"trigger_multiple\"\"targetname\"\"zone\"{padding}}}\
        {{\"classname\"\"func_movelinear\"\"targetname\"\"mover\"\"MoveDistance\"\"32\"\"speed\"\"50\"{padding}}}"
    );
    EntityWorld::compile(
        &parse(text.as_bytes(), Limits::default()).unwrap(),
        EntityWorldConfig::default(),
    )
    .unwrap()
    .0
}

#[derive(Clone, Copy, Debug, Default)]
struct PhaseTotals {
    requests: usize,
    bytes: usize,
    maximum_extra_live_bytes: isize,
}

fn run(extra_pairs: usize) -> (PhaseTotals, Metrics, Metrics) {
    let mut world = fixture(extra_pairs);
    let subject = world.resolve(b"subject", None, None, None)[0];
    let zone = world.resolve(b"zone", None, None, None)[0];
    let mover = world.resolve(b"mover", None, None, None)[0];
    let definition: &Entity = &world.entity(zone).unwrap().definition;
    let (_, copy) = measure(|| black_box(definition.clone()));
    let (_, runtime_copy) = measure(|| black_box(world.entity(zone).unwrap().clone()));
    let mut total = PhaseTotals::default();
    for tick in 1..=120 {
        let checkpoint = world.snapshot().unwrap();
        let commands = [
            WorldCommand::Contact(ContactRecord {
                trigger: zone,
                subject,
                kind: if tick % 2 == 1 {
                    ContactKind::Enter
                } else {
                    ContactKind::Exit
                },
                external_filter_result: None,
                producer_sequence: tick,
            }),
            WorldCommand::Input(InputRecord {
                target: EventTarget::Direct(mover),
                input: if tick % 2 == 1 {
                    b"Open".to_vec()
                } else {
                    b"Close".to_vec()
                },
                value: Variant::Void,
                activator: None,
                caller: None,
                output_action: None,
                producer_sequence: tick,
            }),
        ];
        let (batch, m) = measure(|| world.phase(tick, &commands).unwrap());
        total.requests += m.requests;
        total.bytes += m.bytes;
        total.maximum_extra_live_bytes = total.maximum_extra_live_bytes.max(m.peak);
        let after = world.snapshot().unwrap();
        // Complete bytes, transitions and rollback, with the original snapshot
        // retained while contact and mover state detach from it.
        world.restore(&checkpoint).unwrap();
        assert_eq!(world.snapshot().unwrap().bytes(), checkpoint.bytes());
        assert_eq!(world.phase(tick, &commands).unwrap(), batch);
        assert_eq!(world.snapshot().unwrap().bytes(), after.bytes());
        let request_id = batch
            .records
            .iter()
            .find_map(|r| match r.transition {
                Transition::Request(RuntimeRequest::Mover { request_id, .. }) => Some(request_id),
                _ => None,
            })
            .expect("actual mover request");
        world
            .phase(
                tick,
                &[WorldCommand::MoverCompleted {
                    entity: mover,
                    request_id,
                }],
            )
            .unwrap();
    }
    (total, copy, runtime_copy)
}

#[test]
fn checkpoint_mutation_cost_is_independent_of_authored_definition_size() {
    let ((small, small_definition, small_runtime), small_lifetime) = measure(|| run(0));
    let ((large, large_definition, large_runtime), large_lifetime) = measure(|| run(64));
    println!("definition clones: small={small_definition:?}, large={large_definition:?}");
    println!("runtime clones: small={small_runtime:?}, large={large_runtime:?}");
    println!("120 contact/mover/checkpoint phases: small={small:?}, large={large:?}");
    println!("whole fixture lifetimes: small={small_lifetime:?}, large={large_lifetime:?}");
    assert_eq!(small_lifetime.live, 0, "all fixture ownership released");
    assert_eq!(large_lifetime.live, 0, "all fixture ownership released");
    assert_eq!(small.requests, large.requests);
    assert_eq!(small.bytes, large.bytes);
    assert_eq!(small.maximum_extra_live_bytes, large.maximum_extra_live_bytes);
    assert_eq!(small_runtime.requests, large_runtime.requests);
    assert_eq!(small_runtime.bytes, large_runtime.bytes);
}
