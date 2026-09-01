use crate::{
    EdgeId, FeaturePlacement, FeatureWalkError, SurfaceFeature, SurfaceFeatureKind,
    SurfaceFeaturePair,
};

pub(crate) fn select_triangle_features(
    shapes: [FeaturePlacement<'_>; 2],
    seeds: [EdgeId; 2],
) -> Result<(SurfaceFeaturePair, usize), FeatureWalkError> {
    let triangle = |side: usize| -> Result<[EdgeId; 3], FeatureWalkError> {
        let first = seeds[side];
        let second = shapes[side].topology.next(first)?;
        Ok([first, second, shapes[side].topology.next(second)?])
    };
    let triangles = [triangle(0)?, triangle(1)?];
    let mut best = 1.0e101;
    let mut choice = None;
    let mut retain = |squared: f64,
                      weighted: bool,
                      edges: [EdgeId; 2],
                      kinds: [SurfaceFeatureKind; 2],
                      first: usize| {
        let comparison = if weighted {
            squared * 1.000000000001
        } else {
            squared
        };
        if comparison < best {
            best = squared;
            choice = Some((
                SurfaceFeaturePair {
                    first: SurfaceFeature {
                        edge: edges[0],
                        kind: kinds[0],
                    },
                    second: SurfaceFeature {
                        edge: edges[1],
                        kind: kinds[1],
                    },
                },
                first,
            ));
        }
    };
    for first in triangles[0] {
        for second in triangles[1] {
            let a = shapes[0].world_point(first)?;
            let b = shapes[1].world_point(second)?;
            let delta: [f64; 3] = std::array::from_fn(|axis| a[axis] - b[axis]);
            retain(
                (delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2],
                false,
                [first, second],
                [SurfaceFeatureKind::Vertex; 2],
                0,
            );
        }
    }
    let inside = |checks: &[f32]| {
        checks
            .iter()
            .all(|value| value.to_bits() & 0x8000_0000 == 0)
    };
    for vertex_side in 0..2 {
        let face_side = 1 - vertex_side;
        let face = seeds[face_side];
        let fixed = shapes[face_side];
        let (normal, offset) = fixed.topology.unit_face_plane(face)?;
        for vertex in triangles[vertex_side] {
            let point = fixed.local_point(shapes[vertex_side].world_point(vertex)?);
            if inside(&fixed.topology.triangle_region_checks(face, point)?[..3]) {
                let distance =
                    ((normal[0] * point[0] + normal[1] * point[1]) + normal[2] * point[2]) + offset;
                let mut edges = seeds;
                edges[vertex_side] = vertex;
                let mut kinds = [SurfaceFeatureKind::Face; 2];
                kinds[vertex_side] = SurfaceFeatureKind::Vertex;
                retain(distance * distance, true, edges, kinds, vertex_side);
            }
        }
    }
    for edge_side in 0..2 {
        let vertex_side = 1 - edge_side;
        let fixed = shapes[edge_side];
        for vertex in triangles[vertex_side] {
            let point = fixed.local_point(shapes[vertex_side].world_point(vertex)?);
            for edge in triangles[edge_side] {
                if inside(&fixed.topology.segment_region_checks(edge, point)?) {
                    let squared = fixed.topology.point_line_distance_squared(edge, point)?;
                    let mut edges = [edge; 2];
                    edges[vertex_side] = vertex;
                    let mut kinds = [SurfaceFeatureKind::Edge; 2];
                    kinds[vertex_side] = SurfaceFeatureKind::Vertex;
                    retain(squared, true, edges, kinds, vertex_side);
                }
            }
        }
    }
    for first in triangles[0] {
        for second in triangles[1] {
            let geometry =
                crate::segment_metric::SegmentPairGeometry::new(shapes, [first, second])?;
            if inside(&geometry.regions()?) {
                retain(
                    geometry.line_squared_distance()?,
                    true,
                    [first, second],
                    [SurfaceFeatureKind::Edge; 2],
                    0,
                );
            }
        }
    }
    choice.ok_or(FeatureWalkError::NonFiniteTransform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthoredFace, FeatureTopology};
    #[test]
    fn relative_improvement_preserves_vertex_ties_and_admits_a_closer_face() {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        let topology = FeatureTopology::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            &[
                AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    metadata: 1,
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap();
        let edge = topology.edge_id(0).unwrap();
        let fixed = FeaturePlacement {
            topology: &topology,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let moving = FeaturePlacement {
            position: [0.0, -1.0, 0.0],
            ..fixed
        };
        let (choice, side) = select_triangle_features([moving, fixed], [edge; 2]).unwrap();
        assert_eq!(side, 0);
        assert_eq!(
            choice.first,
            SurfaceFeature {
                edge,
                kind: SurfaceFeatureKind::Vertex
            }
        );
        assert_eq!(choice.second, choice.first);
        let moving = FeaturePlacement {
            position: [0.25, -1.0, 0.25],
            ..moving
        };
        let (choice, side) = select_triangle_features([moving, fixed], [edge; 2]).unwrap();
        assert_eq!(side, 0);
        assert_eq!(
            choice.first,
            SurfaceFeature {
                edge,
                kind: SurfaceFeatureKind::Vertex
            }
        );
        assert_eq!(
            choice.second,
            SurfaceFeature {
                edge,
                kind: SurfaceFeatureKind::Face
            }
        );
    }
}
