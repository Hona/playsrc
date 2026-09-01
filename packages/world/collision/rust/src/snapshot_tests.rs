use super::*;

#[test]
fn static_prop_filter_scope_does_not_change_snapshot_state() {
    let world = World::empty();
    let snapshot = Snapshot::compile(&world, 7, vec![ObjectInput {
        identity: 42, role: ObjectRole::StaticProp, enabled: true, volume_contents: false,
        transform: Transform::IDENTITY, linear_velocity: [0.0; 3], angular_velocity: [0.0; 3],
        collision_group: 0, contents: 1, surface_flags: 0,
        shape: SnapshotShape::BoundingBox { bounds: Hull { mins: [-1.0; 3], maxs: [1.0; 3] } },
    }], SnapshotLimits::default()).unwrap();
    let before = snapshot.snapshot_bytes().unwrap();
    let request = SnapshotRayRequest { start: [-10.0, 0.0, 0.0], end: [10.0, 0.0, 0.0], mask: 1, scope: TraceScope::Everything, ignored: &[] };
    let hit = world.trace_snapshot_ray(&snapshot, request, |_| false).unwrap();
    assert!(matches!(hit.hit, Some(Hit::Object { identity: 42, role: ObjectRole::StaticProp, .. })));
    assert!(!world.trace_snapshot_ray(&snapshot, SnapshotRayRequest { scope: TraceScope::EntitiesOnly, ..request }, |_| true).unwrap().did_hit());
    assert!(!world.trace_snapshot_ray(&snapshot, SnapshotRayRequest { scope: TraceScope::EverythingFilterProps, ..request }, |_| false).unwrap().did_hit());
    assert!(!world.trace_snapshot_ray(&snapshot, SnapshotRayRequest { scope: TraceScope::WorldOnly, ..request }, |_| true).unwrap().did_hit());
    assert_eq!(snapshot.snapshot_bytes().unwrap(), before);
}

#[test]
fn authored_hierarchy_validates_references_cycles_bounds_and_limits() {
    let shape = PhysicsShape::compile(
        1,
        vec![ConvexInput {
            solid: 0,
            convex: 0,
            contents: 1,
            vertices: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            triangles: vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
            authored: None,
        }],
        SnapshotLimits::default(),
    )
    .unwrap();
    let mut raw = [0_u8; 28];
    raw[4..8].copy_from_slice(&32_i32.to_le_bytes());
    raw[20..24].copy_from_slice(&2.0_f32.to_le_bytes());
    raw[24..27].copy_from_slice(&[250; 3]);
    let hierarchy = AuthoredHierarchy {
        nodes: vec![AuthoredHierarchyNode {
            raw,
            children: None,
            hull: Some(AuthoredHullRef::Piece(0)),
        }],
        enclosures: Vec::new(),
    };
    assert!(
        hierarchy
            .validate(&shape, SnapshotLimits::default())
            .is_ok()
    );
    let mut missing = hierarchy.clone();
    missing.nodes[0].hull = Some(AuthoredHullRef::Piece(1));
    assert_eq!(
        missing
            .validate(&shape, SnapshotLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidReference
    );
    let mut cycle = hierarchy.clone();
    cycle.nodes[0].raw[..4].copy_from_slice(&28_i32.to_le_bytes());
    cycle.nodes[0].children = Some([0, 0]);
    assert_eq!(
        cycle
            .validate(&shape, SnapshotLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidSnapshot
    );
    let mut extra = hierarchy.clone();
    extra.nodes.push(extra.nodes[0].clone());
    assert_eq!(
        extra
            .validate(&shape, SnapshotLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidReference
    );
    assert_eq!(
        extra
            .validate(
                &shape,
                SnapshotLimits {
                    max_hierarchy_nodes: 1,
                    ..SnapshotLimits::default()
                }
            )
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
    let mut bounds = hierarchy;
    bounds.nodes[0].raw[20..24].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(
        bounds
            .validate(&shape, SnapshotLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::InvalidSnapshot
    );
}

struct Random(u32);
impl Random {
    fn value(&mut self, scale: f32) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0) * scale
    }
    fn vector(&mut self, scale: f32) -> [f32; 3] {
        std::array::from_fn(|_| self.value(scale))
    }
}

fn object(identity: u64, origin: [f32; 3], shape: SnapshotShape) -> ObjectInput {
    ObjectInput {
        identity,
        role: ObjectRole::Entity,
        enabled: true,
        volume_contents: false,
        transform: Transform {
            origin,
            angles: [0.0; 3],
        },
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
        collision_group: 0,
        contents: 1,
        surface_flags: 17,
        shape,
    }
}

#[test]
fn spatial_candidates_match_linear_source_order_and_reuse_query_storage() {
    let world = World::empty();
    let mut random = Random(0x183);
    let inputs = (0..512)
        .map(|id| {
            object(
                id,
                random.vector(1000.0),
                SnapshotShape::BoundingBox {
                    bounds: Hull {
                        mins: [-32.0; 3],
                        maxs: [32.0; 3],
                    },
                },
            )
        })
        .collect();
    let snapshot = Snapshot::compile(&world, 1, inputs, SnapshotLimits::default()).unwrap();
    assert!(snapshot.order.windows(2).any(|pair| pair[0] > pair[1]));
    let mut scratch = QueryScratch::default();
    for _ in 0..512 {
        let origin = random.vector(1000.0);
        let bounds = Hull {
            mins: sub(origin, [100.0; 3]),
            maxs: add(origin, [100.0; 3]),
        };
        snapshot.candidate_indices(bounds, &mut scratch);
        let expected = snapshot
            .objects
            .iter()
            .enumerate()
            .filter(|(_, object)| bounds_intersect(bounds, object.bounds))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(scratch.candidates, expected);
    }
}

#[test]
fn physical_query_resources_are_shared_across_poses_but_not_replacements() {
    let world = World::empty();
    let shape = Arc::new(
        PhysicsShape::compile(
            1,
            vec![ConvexInput {
                solid: 0,
                convex: 0,
                contents: 1,
                vertices: vec![
                    [0.0; 3],
                    [10.0, 0.0, 0.0],
                    [0.0, 10.0, 0.0],
                    [0.0, 0.0, 10.0],
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]],
                authored: None,
            }],
            SnapshotLimits::default(),
        )
        .unwrap(),
    );
    let implementation = Arc::new(FixturePhysicsQuery { geometry: Arc::clone(&shape), calls: Default::default() });
    let inputs = vec![
        object(1, [0.0; 3], SnapshotShape::Physics(implementation.clone())),
        object(2, [20.0; 3], SnapshotShape::Physics(implementation.clone())),
    ];
    let original = Snapshot::compile(&world, 1, inputs.clone(), SnapshotLimits::default()).unwrap();
    assert_eq!(implementation.calls.load(std::sync::atomic::Ordering::Relaxed), 0);
    world
        .trace_snapshot_ray(
            &original,
            SnapshotRayRequest {
                start: [-5.0, 1.0, 1.0],
                end: [15.0, 1.0, 1.0],
                mask: 1,
                scope: TraceScope::Everything,
                ignored: &[],
            },
            |_| true,
        )
        .unwrap();
    assert_eq!(implementation.calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(
        original,
        Snapshot::compile(&world, 1, inputs.clone(), SnapshotLimits::default()).unwrap()
    );
    let mut changed = inputs.clone();
    changed[1].transform.angles[1] = 90.0;
    let next = original.recompile(&world, 2, changed.clone()).unwrap();
    let query = |snapshot: &Snapshot, index: usize| match &snapshot.objects[index].shape {
        SnapshotShape::Physics(query) => Arc::clone(query),
        _ => panic!("physical query"),
    };
    assert!(Arc::ptr_eq(&query(&original, 0), &query(&next, 0)));
    assert!(Arc::ptr_eq(&query(&original, 1), &query(&next, 1)));
    assert_eq!(
        next,
        Snapshot::compile(&world, 2, changed, SnapshotLimits::default()).unwrap()
    );
    let mut replaced = inputs;
    replaced[0].shape = SnapshotShape::Physics(Arc::new(FixturePhysicsQuery { geometry: Arc::new((*shape).clone()), calls: Default::default() }));
    let replaced = original.recompile(&world, 3, replaced).unwrap();
    assert!(!Arc::ptr_eq(&query(&original, 0), &query(&replaced, 0)));
    assert!(Arc::ptr_eq(&query(&original, 1), &query(&replaced, 1)));
    let follower = Snapshot::compile(&world, 4, vec![object(100, [0.0; 3], SnapshotShape::Follower {
        parent: 7, query: implementation.clone(),
    })], SnapshotLimits::default()).unwrap();
    let request = SnapshotRayRequest { start: [-5.0, 1.0, 1.0], end: [15.0, 1.0, 1.0], mask: 1, scope: TraceScope::Everything, ignored: &[] };
    let hit = world.trace_snapshot_ray(&follower, request, |candidate| { assert_eq!(candidate.identity, 100); true }).unwrap();
    assert_eq!(hit.entity_identity(), Some(7));
    assert!(!world.trace_snapshot_ray(&follower, SnapshotRayRequest { ignored: &[7], ..request }, |_| panic!("owner rejection precedes filtering")).unwrap().did_hit());
    assert!(!world.trace_snapshot_ray(&follower, SnapshotRayRequest { ignored: &[100], ..request }, |_| panic!("collider rejection precedes filtering")).unwrap().did_hit());
    let bytes = follower.snapshot_bytes().unwrap();
    assert_eq!(bytes[52 + 68], 4);
    assert_eq!(u64::from_le_bytes(bytes[52 + 69..52 + 77].try_into().unwrap()), 7);
}
