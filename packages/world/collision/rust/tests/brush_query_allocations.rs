use playsrc_bsp::{Float32, Leaf, Model, Vector3};
use playsrc_collision::{Brush, Hull, World};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

struct CountingAllocator;
thread_local! { static COUNTS: Cell<Option<(usize, usize)>> = const { Cell::new(None) }; }
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
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() {
            allocated(size);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn repeated_brush_queries_do_not_allocate_membership_nodes() {
    let mut world = World::empty();
    let zero = Vector3 {
        x: Float32(0),
        y: Float32(0),
        z: Float32(0),
    };
    world.models.push(Model {
        mins: zero,
        maxs: zero,
        origin: zero,
        head_node: -1,
        first_face: 0,
        face_count: 0,
    });
    world.leaves.push(Leaf {
        contents: 0,
        cluster: 0,
        area_and_flags: 0,
        mins: [0; 3],
        maxs: [0; 3],
        first_leaf_face: 0,
        leaf_face_count: 0,
        first_leaf_brush: 0,
        leaf_brush_count: 512,
        leaf_water_data_id: -1,
        padding: 0,
        ambient_cube: None,
    });
    world.brushes = vec![
        Brush {
            first_side: 0,
            side_count: 0,
            contents: 0
        };
        256
    ];
    // Descending order and duplicates are intentional; membership is not ordering.
    world.leaf_brushes = (0..256).rev().chain(0..256).collect();
    let hull = Hull {
        mins: [0.0; 3],
        maxs: [0.0; 3],
    };
    let expected = world.trace_hull([0.0; 3], [1.0; 3], hull, 1).unwrap();
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COUNTS.set(None);
        }
    }
    assert!(COUNTS.replace(Some((0, 0))).is_none());
    let reset = Reset;
    let began = std::time::Instant::now();
    for _ in 0..1000 {
        assert_eq!(
            black_box(&world)
                .trace_hull([0.0; 3], [1.0; 3], hull, 1)
                .unwrap(),
            expected
        );
    }
    let elapsed = began.elapsed();
    let counts = COUNTS.replace(None).unwrap();
    drop(reset);
    eprintln!("brush queries: {counts:?}, {elapsed:?}");
    // One traversal stack and seven ordered-list capacities per query. There
    // must be no membership allocation, retained scratch, or map-sized clearing.
    assert_eq!(counts.0, 8000);
}
