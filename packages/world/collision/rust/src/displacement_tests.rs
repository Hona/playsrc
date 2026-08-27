use super::*;

struct Random(u32);
impl Random {
    fn value(&mut self, range: f32) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0) * range
    }
    fn vector(&mut self, range: f32) -> [f32; 3] {
        std::array::from_fn(|_| self.value(range))
    }
}

fn tree(random: &mut Random) -> (Tree, Vec<[f32; 3]>) {
    let mut tree = Tree {
        source: 17,
        parent_face: 91,
        contents: 1,
        surface_flags: 0,
        bounds: empty_bounds(),
        triangles: Vec::new(),
        nodes: Vec::new(),
        edge_planes: Vec::new(),
        primary_surface: SurfaceIdentity {
            registry: [19; 32],
            index: 7,
        },
        secondary_surface: Some(SurfaceIdentity {
            registry: [19; 32],
            index: 11,
        }),
    };
    let mut positions = Vec::new();
    let mut indices = HashMap::new();
    for index in 0..512 {
        let origin = [
            (index / 32) as f32 * 64.0 - 512.0,
            (index % 32) as f32 * 64.0 - 1024.0,
            random.value(128.0),
        ];
        let mut points = [
            origin,
            add(origin, [32.0, 0.0, 0.0]),
            add(origin, [32.0, 32.0, 0.0]),
        ];
        if index % 3 != 0 {
            points[1] = add(points[1], random.vector(30.0));
            points[2] = add(points[2], random.vector(30.0));
        }
        let normal =
            normalize(cross(sub(points[2], points[0]), sub(points[1], points[0]))).unwrap();
        let bounds = points.into_iter().fold(empty_bounds(), include);
        let first = positions.len() as u16;
        positions.extend(points);
        tree.triangles.push(Triangle {
            vertices: [first, first + 1, first + 2],
            normal,
            distance: dot(normal, points[0]),
            bounds_vertices: bounds_vertices(points),
            flags: (index as u16 & 0x1f) | if index % 2 != 0 { SECONDARY_SURFACE } else { 0 },
            edges: cache_edges(points, &mut tree.edge_planes, &mut indices, index).unwrap(),
        });
        tree.bounds = union(tree.bounds, bounds);
    }
    tree.bounds = expand(tree.bounds, [1.0; 3]);
    build_nodes(
        &tree
            .triangles
            .iter()
            .map(|t| t.bounds(&positions))
            .collect::<Vec<_>>(),
        0,
        &mut tree.nodes,
        8,
    );
    (tree, positions)
}

fn assert_bits(actual: Trace, expected: Trace) {
    assert_eq!(actual, expected);
    assert_eq!(actual.fraction.to_bits(), expected.fraction.to_bits());
    assert_eq!(
        actual.fraction_left_solid.to_bits(),
        expected.fraction_left_solid.to_bits()
    );
    assert_eq!(actual.end.map(f32::to_bits), expected.end.map(f32::to_bits));
    assert_eq!(
        actual
            .plane
            .map(|p| (p.normal.map(f32::to_bits), p.distance.to_bits())),
        expected
            .plane
            .map(|p| (p.normal.map(f32::to_bits), p.distance.to_bits()))
    );
}

#[test]
fn compact_bounds_and_planes_reconstruct_exact_float_bits() {
    let mut random = Random(0x55818240);
    let (tree, positions) = tree(&mut random);
    for triangle in &tree.triangles {
        let points = triangle.vertices.map(|i| positions[usize::from(i)]);
        let expected = points.into_iter().fold(empty_bounds(), include);
        let actual = triangle.bounds(&positions);
        assert_eq!(
            actual.mins.map(f32::to_bits),
            expected.mins.map(f32::to_bits)
        );
        assert_eq!(
            actual.maxs.map(f32::to_bits),
            expected.maxs.map(f32::to_bits)
        );
        for (slot, index) in triangle.edges.into_iter().enumerate() {
            let expected = edge_plane(points, slot / 3, slot % 3);
            if index == NO_PLANE {
                assert!(expected.is_none());
            } else {
                let actual = tree.edge_planes[usize::from(index & 0x7fff)].unpack(slot / 3, index);
                let expected = expected.unwrap();
                assert_eq!(
                    actual.normal.map(f32::to_bits),
                    expected.normal.map(f32::to_bits)
                );
                assert_eq!(actual.distance.to_bits(), expected.distance.to_bits());
            }
        }
    }
    for zeros in [[0.0, -0.0, 0.0], [-0.0, 0.0, -0.0]] {
        let points = zeros.map(|zero| [zero; 3]);
        let triangle = Triangle {
            vertices: [0, 1, 2],
            normal: [0.0; 3],
            distance: 0.0,
            bounds_vertices: bounds_vertices(points),
            flags: 0,
            edges: [NO_PLANE; 9],
        };
        let expected = points.into_iter().fold(empty_bounds(), include);
        let actual = triangle.bounds(&points);
        assert_eq!(
            actual.mins.map(f32::to_bits),
            expected.mins.map(f32::to_bits)
        );
        assert_eq!(
            actual.maxs.map(f32::to_bits),
            expected.maxs.map(f32::to_bits)
        );
    }
}

#[test]
fn ordered_hierarchy_and_cached_intervals_match_direct_traces_bit_for_bit() {
    let mut random = Random(0x189185);
    let mut world = World::empty();
    let (first, positions) = tree(&mut random);
    let mut duplicate = first.clone();
    duplicate.source = 18;
    duplicate.primary_surface.index = 99;
    world.displacement_inputs = (0..2)
        .map(|_| crate::DisplacementInput {
            source: 0,
            parent_face: 0,
            contents: 1,
            positions: positions.clone(),
            triangles: Vec::new(),
            triangle_tags: Vec::new(),
            primary_surface: first.primary_surface,
            secondary_surface: first.secondary_surface,
            use_secondary_surface: Vec::new(),
        })
        .collect();
    world.displacement_trees.trees = vec![first, duplicate];
    build_nodes(
        &world
            .displacement_trees
            .trees
            .iter()
            .map(|t| t.bounds)
            .collect::<Vec<_>>(),
        0,
        &mut world.displacement_trees.nodes,
        4,
    );
    let empty = World::empty();
    let mut hits = [0; 3];
    for index in 0..30000 {
        let triangle = &world.displacement_trees.trees[0].triangles[index % 512];
        let points = triangle.vertices.map(|i| positions[usize::from(i)]);
        let center = scale(add(add(points[0], points[1]), points[2]), 1.0 / 3.0);
        let start = match index % 4 {
            0 => points[index % 3], // exact vertices/edges, including signed-zero directions
            1 => center,
            _ => add(center, random.vector(64.0)),
        };
        let end = match index % 8 {
            0 => start,                                 // stationary point/box
            1 => add(start, [0.0, -0.0, 18.0]),         // step up
            2 => add(start, [0.0, 0.0, -18.0]),         // ground step
            3 => add(start, [64.0, 0.0, DIST_EPSILON]), // slide near a boundary
            4 => add(start, [0.0, DIST_EPSILON, 0.0]),
            _ => add(start, random.vector(160.0)),
        };
        let hull = match (index / 8) % 4 {
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
                mins: random.vector(32.0).map(|v| -v.abs()),
                maxs: random.vector(32.0).map(f32::abs),
            },
        };
        for flags in [0, NO_RAY_COLLISION, NO_HULL_COLLISION] {
            for tree in &mut world.displacement_trees.trees {
                tree.surface_flags = flags;
            }
            let mut expected = empty.trace_hull(start, end, hull, u32::MAX).unwrap();
            if index % 7 == 0 {
                expected.fraction = 0.5;
            }
            let mut actual = expected;
            trace_inner::<true>(&world, start, end, hull, u32::MAX, &mut expected).unwrap();
            trace_inner::<false>(&world, start, end, hull, u32::MAX, &mut actual).unwrap();
            assert_bits(actual, expected);
            if actual.displacement.is_some() {
                assert_eq!(
                    actual.displacement.unwrap().source,
                    17,
                    "duplicate tie order"
                );
                hits[if actual.start_solid {
                    2
                } else {
                    usize::from(hull.maxs == [0.0; 3])
                }] += 1;
            }
        }
    }
    assert!(
        hits.into_iter().all(|count| count > 100),
        "coverage {hits:?}"
    );
}

#[test]
fn hierarchy_only_skips_disjoint_contiguous_ranges() {
    let mut random = Random(0x185189);
    let (tree, positions) = tree(&mut random);
    for _ in 0..2000 {
        let start = random.vector(1500.0);
        let end = add(start, random.vector(100.0));
        let extents = random.vector(82.0).map(f32::abs);
        let query = expand(swept_bounds(start, end, extents), [DIST_EPSILON; 3]);
        let selected = |reference| {
            Candidates::new(&tree.nodes, tree.triangles.len(), query, extents, reference)
                .filter(|i| {
                    intersects(
                        query,
                        expand(tree.triangles[*i].bounds(&positions), extents),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(selected(false), selected(true));
    }
}
