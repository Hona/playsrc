use crate::{
    AuthoredContactPlane, ContactTolerances, EdgeId, FeatureTopology, FeatureWalkError,
    ProjectionKnot, SurfaceFeatureKind, SurfaceFeaturePair, TangentFrame, TopologyError,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactSurface {
    pub point: [f64; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub distance: f32,
    pub check_features: bool,
    pub broken: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactFeatureBinding {
    features: SurfaceFeaturePair,
    face: Option<AuthoredContactPlane>,
    check_features: bool,
}

impl ContactFeatureBinding {
    /// The same immutable topologies must be supplied to each projection.
    pub fn new(
        topologies: [&FeatureTopology; 2],
        features: SurfaceFeaturePair,
        check_features: bool,
    ) -> Result<Self, FeatureWalkError> {
        topologies[0].edge(features.first.edge)?;
        topologies[1].edge(features.second.edge)?;
        let face = match (features.first.kind, features.second.kind) {
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Face) => Some(
                AuthoredContactPlane::from_edge(topologies[1], features.second.edge)?,
            ),
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Vertex | SurfaceFeatureKind::Edge)
            | (SurfaceFeatureKind::Edge, SurfaceFeatureKind::Edge) => None,
            _ => return Err(FeatureWalkError::UnsupportedFeaturePair),
        };
        Ok(Self {
            features,
            face,
            check_features,
        })
    }

    pub fn features(self) -> SurfaceFeaturePair {
        self.features
    }
    pub fn face_scale(self) -> Option<f32> {
        self.face.map(|plane| plane.scale)
    }
    pub fn needs_feature_check(self) -> bool {
        self.check_features
    }
    pub fn request_feature_check(&mut self) {
        self.check_features = true;
    }

    pub fn project(
        &mut self,
        topologies: [&FeatureTopology; 2],
        poses: [ProjectionKnot; 2],
        tolerances: ContactTolerances,
    ) -> Result<ContactSurface, FeatureWalkError> {
        for pose in poses {
            if pose
                .position
                .iter()
                .chain(pose.orientation.iter())
                .any(|value| !value.is_finite())
            {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
        }
        let edges = [self.features.first.edge, self.features.second.edge];
        let point = |side: usize, edge: EdgeId| -> Result<[f64; 3], FeatureWalkError> {
            let local = topologies[side].points()[topologies[side].edge(edge)?.start as usize];
            Ok(world_point(poses[side], local.map(f64::from)))
        };
        let result = match (self.features.first.kind, self.features.second.kind) {
            (SurfaceFeatureKind::Edge, SurfaceFeatureKind::Edge) => ContactSurface::edge_pair(
                topologies,
                edges,
                poses,
                self.check_features,
                tolerances.keeper_distance,
            )?,
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Vertex) => {
                ContactSurface::vertex_pair(
                    [point(0, edges[0])?, point(1, edges[1])?],
                    self.check_features,
                )?
            }
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Edge) => ContactSurface::vertex_edge(
                point(0, edges[0])?,
                [
                    point(1, edges[1])?,
                    point(1, topologies[1].next(edges[1])?)?,
                ],
                self.check_features,
            )?,
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Face) => {
                let world = point(0, edges[0])?;
                let plane = self.face.expect("face binding retains its authored plane");
                let offset = sub(world, poses[1].position);
                let local = std::array::from_fn(|axis| {
                    (poses[1].orientation[axis + 3] * offset[1]
                        + poses[1].orientation[axis] * offset[0])
                        + poses[1].orientation[axis + 6] * offset[2]
                });
                let distance = plane.distance(local)?;
                let edge = topologies[1].edge(edges[1])?;
                let start = topologies[1].points()[edge.start as usize];
                let end = topologies[1].points()[edge.end as usize];
                let tangent = world_vector(
                    poses[1].orientation,
                    std::array::from_fn(|axis| f64::from(end[axis] - start[axis])),
                )
                .map(|value| value as f32);
                let normal =
                    world_vector(poses[1].orientation, plane.normal).map(|value| (-value) as f32);
                ContactSurface {
                    point: world,
                    normal,
                    tangent,
                    distance,
                    check_features: false,
                    broken: self.check_features && !plane.contains(topologies[1], local)?,
                }
            }
            _ => return Err(FeatureWalkError::UnsupportedFeaturePair),
        };
        if !result.distance.is_finite()
            || result.point.iter().any(|value| !value.is_finite())
            || result
                .normal
                .iter()
                .chain(result.tangent.iter())
                .any(|value| !value.is_finite())
        {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        self.check_features = result.check_features;
        Ok(result)
    }
}

impl ContactSurface {
    pub(crate) fn clamp_penetration(&mut self) {
        if self.distance < 0.0 {
            self.distance = 0.0;
        }
    }
    pub fn frame(self) -> Result<TangentFrame, TopologyError> {
        if self
            .tangent
            .iter()
            .chain(self.normal.iter())
            .any(|value| !value.is_finite())
        {
            return Err(TopologyError::NonFiniteSeparation { axis: 0 });
        }
        let first = normalize32(self.tangent)?;
        let normal = self.normal;
        Ok(TangentFrame {
            first,
            second: [
                normal[1] * first[2] - normal[2] * first[1],
                normal[2] * first[0] - normal[0] * first[2],
                normal[0] * first[1] - normal[1] * first[0],
            ],
        })
    }
    fn edge_pair(
        topologies: [&FeatureTopology; 2],
        edges: [EdgeId; 2],
        transforms: [ProjectionKnot; 2],
        check_segments: bool,
        parallel_distance: f32,
    ) -> Result<Self, TopologyError> {
        let mut points = [[[0.0; 3]; 2]; 2];
        for side in 0..2 {
            let edge = topologies[side].edge(edges[side])?;
            for (index, vertex) in [edge.start, edge.end].into_iter().enumerate() {
                let local = topologies[side].points()[vertex as usize].map(f64::from);
                points[side][index] = std::array::from_fn(|axis| {
                    let base = axis * 3;
                    ((transforms[side].orientation[base + 1] * local[1]
                        + transforms[side].orientation[base] * local[0])
                        + transforms[side].orientation[base + 2] * local[2])
                        + transforms[side].position[axis]
                });
            }
        }
        Self::edge_pair_world(points, check_segments, parallel_distance)
    }

    fn edge_pair_world(
        points: [[[f64; 3]; 2]; 2],
        check_segments: bool,
        parallel_distance: f32,
    ) -> Result<Self, TopologyError> {
        if let Some(component) = points
            .iter()
            .flatten()
            .flatten()
            .position(|value| !value.is_finite())
        {
            return Err(TopologyError::NonFiniteTransform { component });
        }
        if !parallel_distance.is_finite() {
            return Err(TopologyError::NonFiniteSeparation { axis: 0 });
        }
        let retired = Self {
            point: points[0][0],
            normal: [1.0, 0.0, 0.0],
            tangent: [0.0, 1.0, 0.0],
            distance: parallel_distance,
            check_features: check_segments,
            broken: true,
        };
        let normalize = |value: [f64; 3]| {
            let squared = dot(value, value);
            if squared < 1.0e-19 {
                return value;
            }
            let reciprocal = crate::arithmetic::refined_inverse_root::<4>(squared);
            value.map(|value| value * reciprocal)
        };
        let directions = points.map(|segment| normalize(sub(segment[1], segment[0])));
        let perpendicular = cross(directions[0], directions[1]);
        let squared = dot(perpendicular, perpendicular);
        if !squared.is_finite() {
            return Err(TopologyError::NonFiniteSeparation { axis: 0 });
        }
        if squared <= f64::from(1.0e-10_f32) {
            return Ok(retired);
        }
        let planes = directions.map(|direction| cross(direction, perpendicular));
        let mut fractions = [0.0; 2];
        for side in [1, 0] {
            let normal = planes[1 - side];
            let start = dot(normal, points[side][0]);
            let denominator = start - dot(normal, points[side][1]);
            if denominator.abs() < 1.0e-19 {
                return Ok(retired);
            }
            fractions[side] = (start - dot(normal, points[1 - side][0])) / denominator;
        }
        let nearest = std::array::from_fn::<_, 2, _>(|side| {
            std::array::from_fn::<_, 3, _>(|axis| {
                (1.0 - fractions[side]) * points[side][0][axis]
                    + fractions[side] * points[side][1][axis]
            })
        });
        let delta = sub(nearest[1], nearest[0]);
        let distance = dot(delta, delta).sqrt();
        let (distance, normal) = if distance > 1.0e-19 {
            (
                distance as f32,
                delta.map(|value| (value * (1.0 / distance)) as f32),
            )
        } else {
            (
                0.0,
                perpendicular.map(|value| (value * (1.0 / squared.sqrt())) as f32),
            )
        };
        if !distance.is_finite()
            || nearest.iter().flatten().any(|value| !value.is_finite())
            || normal.iter().any(|value| !value.is_finite())
        {
            return Err(TopologyError::NonFiniteSeparation { axis: 0 });
        }
        Ok(Self {
            point: nearest[0],
            normal,
            tangent: directions[0].map(|value| value as f32),
            distance,
            check_features: false,
            broken: check_segments && fractions.iter().any(|value| !(0.0..=1.0).contains(value)),
        })
    }

    fn vertex_pair(points: [[f64; 3]; 2], check_features: bool) -> Result<Self, TopologyError> {
        let mut normal = sub(points[1], points[0]);
        let squared = dot(normal, normal);
        let distance = if squared < 1.0e-19 {
            0.0
        } else {
            let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
            normal = normal.map(|value| value * inverse);
            squared * inverse
        } as f32;
        let normal = normal.map(|value| value as f32);
        let (x, z) = if normal[0] * normal[0] < 0.9_f32 {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let tangent = normalize32([
            normal[1] * z,
            normal[2] * x - normal[0] * z,
            -(normal[1] * x),
        ])?;
        Ok(Self {
            point: points[0],
            normal,
            tangent,
            distance,
            check_features,
            broken: false,
        })
    }

    fn vertex_edge(
        point: [f64; 3],
        edge: [[f64; 3]; 2],
        check_features: bool,
    ) -> Result<Self, TopologyError> {
        let direction = sub(edge[1], edge[0]).map(|value| value as f32);
        let offset = sub(point, edge[0]).map(|value| value as f32);
        let inverse =
            crate::arithmetic::refined_inverse_root::<5>(f64::from(dot32(direction, direction)));
        let perpendicular = cross32(direction, offset);
        let distance = f64::from(dot32(perpendicular, perpendicular)).sqrt() * inverse;
        let tangent = if distance * distance > 1.0e-19 {
            let scale = inverse / distance;
            perpendicular.map(|value| (f64::from(value) * scale) as f32)
        } else {
            [1.0, 0.0, 0.0]
        };
        let normal = normalize32(cross32(direction, tangent))?;
        let projection = f64::from(
            (direction[0] * offset[0] + direction[1] * offset[1]) + direction[2] * offset[2],
        ) * (inverse * inverse);
        Ok(Self {
            point,
            normal,
            tangent,
            distance: distance as f32,
            check_features: false,
            broken: check_features && !(0.0..=1.0).contains(&projection),
        })
    }
}

fn world_vector(matrix: [f64; 9], value: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| {
        (matrix[axis * 3 + 1] * value[1] + matrix[axis * 3] * value[0])
            + matrix[axis * 3 + 2] * value[2]
    })
}
fn world_point(pose: ProjectionKnot, value: [f64; 3]) -> [f64; 3] {
    let vector = world_vector(pose.orientation, value);
    std::array::from_fn(|axis| pose.position[axis] + vector[axis])
}
fn dot32(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2]
}
fn cross32(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize32(value: [f32; 3]) -> Result<[f32; 3], TopologyError> {
    let squared = (value[0] * value[0] + value[1] * value[1]) + value[2] * value[2];
    if !squared.is_finite() {
        return Err(TopologyError::NonFiniteSeparation { axis: 0 });
    }
    if f64::from(squared) < 1.0e-19 {
        return Ok(value);
    }
    let inverse = crate::arithmetic::refined_inverse_root::<4>(f64::from(squared));
    Ok(value.map(|value| (f64::from(value) * inverse) as f32))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| a[axis] - b[axis])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn physical_gap_clamping_preserves_geometry_and_signed_zero() {
        let surface = super::ContactSurface {
            point: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            distance: -0.003,
            check_features: true,
            broken: false,
        };
        let mut contact = surface;
        contact.clamp_penetration();
        assert_eq!(
            contact,
            super::ContactSurface {
                distance: 0.0,
                ..surface
            }
        );
        contact.distance = -0.0;
        contact.clamp_penetration();
        assert_eq!(contact.distance.to_bits(), (-0.0_f32).to_bits());
    }
    use super::*;

    fn topology(points: Vec<[f32; 3]>) -> FeatureTopology {
        let word = |point: u32, offset: i32| point | (((offset as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            points,
            &[
                crate::AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                crate::AuthoredFace {
                    metadata: 1,
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn vertex_pair_tangent_keeps_the_producer_signed_zero() {
        let topology = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let vertex = crate::SurfaceFeature {
            edge: topology.edge_id(0).unwrap(),
            kind: SurfaceFeatureKind::Vertex,
        };
        let mut binding = ContactFeatureBinding::new(
            [&topology; 2],
            SurfaceFeaturePair {
                first: vertex,
                second: vertex,
            },
            true,
        )
        .unwrap();
        let identity = ProjectionKnot {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let surface = binding
            .project(
                [&topology; 2],
                [
                    identity,
                    ProjectionKnot {
                        position: [-3.0, -1.0, -3.0],
                        ..identity
                    },
                ],
                ContactTolerances::from_gravity([0.0; 3]).unwrap(),
            )
            .unwrap();
        assert_eq!(surface.tangent[0].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(
            surface.frame().unwrap().first[0].to_bits(),
            (-0.0_f32).to_bits()
        );
        assert!(binding.needs_feature_check());
    }

    #[test]
    fn tiny_authored_faces_keep_the_regularized_scale_before_projection() {
        let size = 1.0e-9_f32;
        let topology = topology(vec![[0.0; 3], [size, 0.0, 0.0], [0.0, size, 0.0]]);
        let plane =
            AuthoredContactPlane::from_edge(&topology, topology.edge_id(0).unwrap()).unwrap();
        let area = f64::from(size) * f64::from(size);
        let expected = (1.0 / (area + f64::from(1.0e-18_f32))) as f32;
        assert_eq!(plane.scale.to_bits(), expected.to_bits());
        assert_ne!(plane.scale.to_bits(), ((1.0 / area) as f32).to_bits());
        assert_eq!(plane.normal[2], area * f64::from(expected));
    }

    #[test]
    fn binding_snapshots_own_one_time_segment_checks_and_atomic_failure() {
        let first = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let second = topology(vec![[2.0, -1.0, 1.0], [2.0, 1.0, 1.0], [3.0, 0.0, 1.0]]);
        let features = SurfaceFeaturePair {
            first: crate::SurfaceFeature {
                edge: first.edge_id(0).unwrap(),
                kind: SurfaceFeatureKind::Edge,
            },
            second: crate::SurfaceFeature {
                edge: second.edge_id(0).unwrap(),
                kind: SurfaceFeatureKind::Edge,
            },
        };
        let mut binding = ContactFeatureBinding::new([&first, &second], features, true).unwrap();
        let mut restored = binding;
        let pose = ProjectionKnot {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let tolerances = ContactTolerances::from_gravity([0.0; 3]).unwrap();
        let first_surface = binding
            .project([&first, &second], [pose; 2], tolerances)
            .unwrap();
        assert!(first_surface.broken && !binding.needs_feature_check());
        assert_eq!(
            restored
                .project([&first, &second], [pose; 2], tolerances)
                .unwrap(),
            first_surface
        );
        assert_eq!(binding, restored);
        assert!(
            !binding
                .project([&first, &second], [pose; 2], tolerances)
                .unwrap()
                .broken
        );
        let before = binding;
        let bad = ProjectionKnot {
            position: [f64::NAN; 3],
            ..pose
        };
        assert!(
            binding
                .project([&first, &second], [bad, pose], tolerances)
                .is_err()
        );
        assert_eq!(binding, before);
    }

    #[test]
    fn finite_edges_preserve_closest_points_normal_and_tangent_words() {
        let result = ContactSurface::edge_pair_world(
            [
                [
                    [
                        4612944751416400255,
                        13794007912221108000,
                        13736374082121451840,
                    ]
                    .map(f64::from_bits),
                    [
                        4612879601621049661,
                        13799046651972144192,
                        13749560966200152960,
                    ]
                    .map(f64::from_bits),
                ],
                [
                    [4612901990240878592, 0, 13836274027095654400].map(f64::from_bits),
                    [4612901990240878592, 0, 4612901990240878592].map(f64::from_bits),
                ],
            ],
            true,
            f32::from_bits(1013920078),
        )
        .unwrap();
        assert_eq!(
            result.point.map(f64::to_bits),
            [
                4612904024912965252,
                13797265653423456923,
                13746661315372162454
            ]
        );
        assert_eq!(
            result.normal.map(f32::to_bits),
            [3188835539, 1065182358, 2827308146]
        );
        assert_eq!(
            result.tangent.map(f32::to_bits),
            [3212666005, 3188835539, 3102789533]
        );
        assert_eq!(result.distance.to_bits(), 1003486808);
        assert!(!result.broken && !result.check_features);
    }

    #[test]
    fn segment_bounds_are_checked_once_without_clamping_closest_points() {
        let points = [
            [[0.0; 3], [1.0, 0.0, 0.0]],
            [[2.0, -1.0, 1.0], [2.0, 1.0, 1.0]],
        ];
        let checked = ContactSurface::edge_pair_world(points, true, 0.01).unwrap();
        let retained = ContactSurface::edge_pair_world(points, false, 0.01).unwrap();
        assert!(checked.broken && !retained.broken);
        assert_eq!(checked.point, [2.0, 0.0, 0.0]);
        assert_eq!(checked.normal, [0.0, 0.0, 1.0]);
        assert_eq!(checked.distance, 1.0);
        assert!(!checked.check_features);
    }

    #[test]
    fn parallel_and_intersecting_edges_keep_distinct_retirement_and_normal_rules() {
        let parallel = ContactSurface::edge_pair_world(
            [
                [[0.0; 3], [1.0, 0.0, 0.0]],
                [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]],
            ],
            true,
            0.012,
        )
        .unwrap();
        assert_eq!(
            parallel,
            ContactSurface {
                point: [0.0; 3],
                normal: [1.0, 0.0, 0.0],
                tangent: [0.0, 1.0, 0.0],
                distance: 0.012,
                check_features: true,
                broken: true
            }
        );
        let collapsed = ContactSurface::edge_pair_world(
            [[[0.0; 3]; 2], [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]]],
            true,
            0.012,
        )
        .unwrap();
        assert_eq!(collapsed, parallel);
        let crossing = ContactSurface::edge_pair_world(
            [
                [[0.0; 3], [1.0, 0.0, 0.0]],
                [[0.5, -1.0, 0.0], [0.5, 1.0, 0.0]],
            ],
            true,
            0.012,
        )
        .unwrap();
        assert_eq!(crossing.point, [0.5, 0.0, 0.0]);
        assert_eq!(crossing.distance, 0.0);
        assert!(!crossing.broken);
        assert_eq!(crossing.normal, [0.0, 0.0, 1.0]);
        assert_eq!(
            crossing.frame().unwrap(),
            TangentFrame {
                first: [1.0, 0.0, 0.0],
                second: [0.0, 1.0, 0.0]
            }
        );
        assert!(ContactSurface::edge_pair_world([[[f64::NAN; 3]; 2]; 2], true, 0.012).is_err());
    }
}
