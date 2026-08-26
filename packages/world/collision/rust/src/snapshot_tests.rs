use super::*;

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

fn compare(
    convex: &Convex,
    transform: Transform,
    count: usize,
    random: &mut Random,
) -> (usize, usize) {
    let basis = transform.basis().unwrap();
    let prepared = PreparedConvex::compile(&convex.vertices, &convex.faces, &convex.edges, basis);
    let limits = SnapshotLimits::default();
    for index in 0..count {
        let start = if index % 7 == 0 {
            basis.point(convex.vertices[index % convex.vertices.len()])
        } else {
            add(transform.origin, random.vector(100.0))
        };
        let end = if index % 5 == 0 {
            start
        } else {
            add(transform.origin, random.vector(100.0))
        };
        let hull = match index % 4 {
            0 => Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            },
            1 => Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 82.0],
            },
            2 => Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 62.0],
            },
            _ => Hull {
                mins: [-0.0001; 3],
                maxs: [0.0001; 3],
            },
        };
        let reference = trace_convex_reference(
            start,
            end,
            hull,
            &convex.vertices,
            &convex.faces,
            &convex.edges,
            basis,
            convex.contents,
            limits,
        );
        let actual = prepared.trace(start, end, hull, convex.contents, limits);
        assert_eq!(
            actual, reference,
            "query {index}: {start:?} -> {end:?}, {transform:?}"
        );
        // Debug formatting includes every selected identity/flag/plane field;
        // verify floating-point bits too, including signed zero.
        if let (Ok(actual), Ok(reference)) = (actual, reference) {
            assert_eq!(actual.fraction.to_bits(), reference.fraction.to_bits());
            assert_eq!(
                actual.fraction_left_solid.to_bits(),
                reference.fraction_left_solid.to_bits()
            );
            assert_eq!(
                actual.end.map(f32::to_bits),
                reference.end.map(f32::to_bits)
            );
            assert_eq!(
                actual
                    .plane
                    .map(|p| (p.normal.map(f32::to_bits), p.distance.to_bits())),
                reference
                    .plane
                    .map(|p| (p.normal.map(f32::to_bits), p.distance.to_bits()))
            );
        }
    }
    (prepared.source_direction_count, prepared.directions.len())
}

#[test]
fn retained_convex_intervals_match_original_sweeps_exactly() {
    let mut random = Random(0x183180);
    for _ in 0..32 {
        let shape = PhysicsShape::compile(
            1,
            vec![ConvexInput {
                solid: 0,
                convex: 0,
                contents: 1,
                vertices: vec![
                    [0.0; 3],
                    [random.value(80.0).abs() + 1.0, 0.0, 0.0],
                    [0.0, random.value(80.0).abs() + 1.0, 0.0],
                    [0.0, 0.0, random.value(80.0).abs() + 1.0],
                ],
                triangles: vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2], [0, 1, 2]],
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let transform = Transform {
            origin: random.vector(16000.0),
            angles: random.vector(180.0),
        };
        let (before, after) = compare(&shape.convexes[0], transform, 256, &mut random);
        assert!(after < before);
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
fn geometry_retention_is_bound_to_shape_and_transform_not_revision_or_identity_alone() {
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
            }],
            SnapshotLimits::default(),
        )
        .unwrap(),
    );
    let inputs = vec![
        object(1, [0.0; 3], SnapshotShape::Physics(Arc::clone(&shape))),
        object(2, [20.0; 3], SnapshotShape::Physics(Arc::clone(&shape))),
    ];
    let original = Snapshot::compile(&world, 1, inputs.clone(), SnapshotLimits::default()).unwrap();
    let mut changed = inputs.clone();
    changed[1].transform.angles[1] = 90.0;
    let next = original.recompile(&world, 2, changed.clone()).unwrap();
    assert!(Arc::ptr_eq(
        &original.objects[0].prepared,
        &next.objects[0].prepared
    ));
    assert!(!Arc::ptr_eq(
        &original.objects[1].prepared,
        &next.objects[1].prepared
    ));
    assert_eq!(
        next,
        Snapshot::compile(&world, 2, changed, SnapshotLimits::default()).unwrap()
    );
    let mut replaced = inputs;
    replaced[0].shape = SnapshotShape::Physics(Arc::new((*shape).clone()));
    let replaced = original.recompile(&world, 3, replaced).unwrap();
    assert!(!Arc::ptr_eq(
        &original.objects[0].prepared,
        &replaced.objects[0].prepared
    ));
    assert!(Arc::ptr_eq(
        &original.objects[1].prepared,
        &replaced.objects[1].prepared
    ));
}

#[test]
#[ignore = "requires the exact configured Upward content graph"]
fn configured_upward_convex_topology_and_sweep_equivalence() {
    use playsrc_asset_graph::{ResourceGraph, decode};
    use std::{fs, path::PathBuf, time::Instant};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("playsrc.local.json")).unwrap()).unwrap();
    let cache = PathBuf::from(config["sourceCacheDir"].as_str().unwrap());
    let graph: ResourceGraph = serde_json::from_slice(
        &fs::read(cache.join("browser-bundles/pl_upward.graph.json")).unwrap(),
    )
    .unwrap();
    let started = Instant::now();
    let mut random = Random(0x183180);
    let mut totals = [0_usize; 5];
    let bsp = playsrc_bsp::parse(
        &fs::read(PathBuf::from(config["tf2Dir"].as_str().unwrap()).join("maps/pl_upward.bsp"))
            .unwrap(),
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    let props = playsrc_bsp::parse_static_props(&bsp, playsrc_bsp::Limits::default())
        .unwrap()
        .unwrap();
    let vector = |value: playsrc_bsp::Vector3| [value.x.value(), value.y.value(), value.z.value()];
    let mut occurrence_count = 0;
    for chunk in &graph.chunks {
        if !chunk
            .entries
            .iter()
            .any(|entry| entry.logical_path.ends_with(".phy"))
        {
            continue;
        }
        let encoded = fs::read(
            cache
                .join("browser-bundles/pl_upward.graph/objects")
                .join(&chunk.encoded_sha256),
        )
        .unwrap();
        for entry in decode(chunk, &encoded)
            .unwrap()
            .into_iter()
            .filter(|entry| entry.logical_path.ends_with(".phy"))
        {
            let asset = playsrc_phy::parse_standalone(
                &entry.bytes,
                playsrc_phy::Profile::SourcePcPolygon,
                playsrc_phy::Limits::default(),
            )
            .unwrap();
            if asset.solids.is_empty()
                || asset.solids[0].classification != PhyClassification::Handled
            {
                continue;
            }
            let shape =
                PhysicsShape::from_phy(1, &asset, 0, SnapshotLimits::default(), |_| 1).unwrap();
            let mut directions = [0, 0];
            for prop in props.occurrences.iter().filter(|prop| prop.solidity == 6) {
                let model =
                    String::from_utf8(props.dictionary[usize::from(prop.model)].name.clone())
                        .unwrap();
                if model
                    .strip_suffix(".mdl")
                    .map(|path| format!("{path}.phy"))
                    .as_deref()
                    != Some(&entry.logical_path)
                {
                    continue;
                }
                occurrence_count += 1;
                let transform = Transform {
                    origin: vector(prop.origin),
                    angles: vector(prop.angles),
                };
                for convex in &shape.convexes {
                    compare(convex, transform, 12, &mut random);
                }
            }
            for convex in &shape.convexes {
                let transform = Transform {
                    origin: random.vector(16000.0),
                    angles: random.vector(180.0),
                };
                let (before, after) = compare(convex, transform, 24, &mut random);
                directions[0] += before;
                directions[1] += after;
            }
            let (_, vertices, triangles) = shape.counts();
            println!(
                "{} convexes={} vertices={vertices} triangles={triangles} directions={}/{}",
                entry.logical_path,
                shape.convex_count(),
                directions[0],
                directions[1]
            );
            for (sum, value) in totals.iter_mut().zip([
                shape.convex_count(),
                vertices,
                triangles,
                directions[0],
                directions[1],
            ]) {
                *sum += value;
            }
        }
    }
    println!(
        "configured totals {totals:?}, solid static occurrences={occurrence_count}, elapsed {:?}",
        started.elapsed()
    );
    assert!(totals[0] > 0);
    assert!(totals[4] < totals[3]);
}
