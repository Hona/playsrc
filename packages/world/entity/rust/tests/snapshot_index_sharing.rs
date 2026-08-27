use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use playsrc_entity::{EntityWorld, EntityWorldConfig, Limits, WorldCommand, parse};

struct CountingAllocator;
thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocations<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.set(Some(0));
    let result = operation();
    (result, ALLOCATIONS.replace(None).unwrap())
}

fn world(count: usize) -> EntityWorld {
    let text = (0..count)
        .map(|index| format!("{{\"classname\"\"func_brush\"\"targetname\"\"brush_{index}\"}}"))
        .collect::<String>();
    EntityWorld::compile(
        &parse(text.as_bytes(), Limits::default()).unwrap(),
        EntityWorldConfig::default(),
    )
    .unwrap()
    .0
}

#[test]
fn unchanged_phase_and_clone_do_not_allocate_per_index_entry() {
    let mut small = world(1);
    let mut large = world(1024);
    let (_, small_clone) = allocations(|| small.clone());
    let (_, large_clone) = allocations(|| large.clone());
    let (_, small_phase) = allocations(|| small.phase(1, &[]).unwrap());
    let (_, large_phase) = allocations(|| large.phase(1, &[]).unwrap());
    println!(
        "clone allocations small={small_clone} large={large_clone}; phase allocations small={small_phase} large={large_phase}"
    );
    assert_eq!(large_clone, small_clone);
    assert_eq!(large_phase, small_phase);
}

#[test]
fn index_mutations_preserve_retained_snapshots_clones_and_rollback() {
    let original = world(3);
    let snapshot = original.snapshot().unwrap();
    let original_bytes = snapshot.bytes().to_vec();
    let first = original.resolve(b"brush_0", None, None, None)[0];
    let second = original.resolve(b"brush_1", None, None, None)[0];
    let third = original.resolve(b"brush_2", None, None, None)[0];
    let definition = parse(
        b"{\"classname\"\"func_brush\"\"targetname\"\"brush_1\"}",
        Limits::default(),
    )
    .unwrap()
    .entities
    .remove(0);
    let mut changed = original.clone();
    changed
        .phase(
            1,
            &[
                WorldCommand::SetTargetname {
                    entity: first,
                    targetname: Some(b"brush_1".to_vec()),
                },
                WorldCommand::Remove(third),
                WorldCommand::Spawn(definition.clone()),
            ],
        )
        .unwrap();
    let duplicates = changed.resolve(b"brush_1", None, None, None);
    assert_eq!(&duplicates[..2], &[first, second]);
    assert_eq!(duplicates.len(), 3);
    assert!(changed.resolve(b"brush_0", None, None, None).is_empty());
    assert!(changed.resolve(b"brush_2", None, None, None).is_empty());
    assert_eq!(original.snapshot().unwrap().bytes(), original_bytes);
    assert_eq!(snapshot.bytes(), original_bytes);

    // Replay the same mutation from a restored checkpoint: indexes, handles,
    // ordering, and snapshot encoding must agree, not just visible entities.
    let changed_bytes = changed.snapshot().unwrap().bytes().to_vec();
    changed.restore(&snapshot).unwrap();
    assert_eq!(changed.snapshot().unwrap().bytes(), original_bytes);
    changed
        .phase(
            1,
            &[
                WorldCommand::SetTargetname {
                    entity: first,
                    targetname: Some(b"brush_1".to_vec()),
                },
                WorldCommand::Remove(third),
                WorldCommand::Spawn(definition),
            ],
        )
        .unwrap();
    assert_eq!(changed.snapshot().unwrap().bytes(), changed_bytes);

    let mut config = EntityWorldConfig::default();
    config.limits.max_entities = 1;
    let graph = parse(
        b"{\"classname\"\"func_brush\"\"targetname\"\"before\"}",
        Limits::default(),
    )
    .unwrap();
    let mut bounded = EntityWorld::compile(&graph, config).unwrap().0;
    let before = bounded.snapshot().unwrap();
    let handle = bounded.resolve(b"before", None, None, None)[0];
    assert!(
        bounded
            .phase(
                1,
                &[
                    WorldCommand::SetTargetname {
                        entity: handle,
                        targetname: Some(b"after".to_vec())
                    },
                    WorldCommand::Spawn(graph.entities[0].clone()),
                ]
            )
            .is_err()
    );
    assert_eq!(bounded.snapshot().unwrap().bytes(), before.bytes());
    assert_eq!(bounded.resolve(b"before", None, None, None), vec![handle]);
    assert!(bounded.resolve(b"after", None, None, None).is_empty());
}
