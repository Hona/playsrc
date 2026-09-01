use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId(usize);

impl EdgeId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoredFace {
    pub metadata: u32,
    pub vertices: [u32; 3],
    pub edge_words: [u32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectedEdge {
    pub face: usize,
    pub position: usize,
    pub start: u32,
    pub end: u32,
    pub word: u32,
    pub opposite: EdgeId,
    pub virtual_edge: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyError {
    Empty,
    MissingAuthoredConvex { convex: usize },
    MissingAuthoredEnclosure { enclosure: usize },
    NonFinitePoint { point: usize, axis: usize },
    EdgeCountOverflow,
    InvalidEndpoint { face: usize, edge: usize },
    InvalidOpposite { face: usize, edge: usize },
    NonReciprocalOpposite { face: usize, edge: usize },
    DegenerateFace { face: usize },
    InvalidEdge,
    InvalidRecoveryFace,
    OpenFan,
    NonFiniteSeparation { axis: usize },
    NonFiniteTransform { component: usize },
    InvalidTransformInterval,
    InvalidTransformOrientation,
}

impl fmt::Display for TopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("contact topology requires points and faces"),
            Self::MissingAuthoredConvex { convex } => {
                write!(
                    formatter,
                    "collision convex {convex} has no authored topology"
                )
            }
            Self::MissingAuthoredEnclosure { enclosure } => write!(
                formatter,
                "collision enclosure {enclosure} has no authored topology"
            ),
            Self::NonFinitePoint { point, axis } => {
                write!(formatter, "contact point {point} axis {axis} is not finite")
            }
            Self::EdgeCountOverflow => formatter.write_str("contact topology edge count overflows"),
            Self::InvalidEndpoint { face, edge } => {
                write!(
                    formatter,
                    "contact face {face} edge {edge} has invalid endpoints"
                )
            }
            Self::InvalidOpposite { face, edge } => {
                write!(
                    formatter,
                    "contact face {face} edge {edge} has an invalid opposite"
                )
            }
            Self::NonReciprocalOpposite { face, edge } => {
                write!(
                    formatter,
                    "contact face {face} edge {edge} has a nonreciprocal opposite"
                )
            }
            Self::DegenerateFace { face } => {
                write!(formatter, "contact face {face} has zero area")
            }
            Self::InvalidEdge => formatter.write_str("contact edge identity is invalid"),
            Self::InvalidRecoveryFace => {
                formatter.write_str("authored interior-recovery face link is invalid")
            }
            Self::OpenFan => formatter.write_str("contact edge fan does not close"),
            Self::NonFiniteSeparation { axis } => {
                write!(formatter, "contact separation axis {axis} is not finite")
            }
            Self::NonFiniteTransform { component } => {
                write!(
                    formatter,
                    "contact transform component {component} is not finite"
                )
            }
            Self::InvalidTransformInterval => {
                formatter.write_str("contact transform inverse timestep must be positive")
            }
            Self::InvalidTransformOrientation => {
                formatter.write_str("contact transform orientation cannot be interpolated")
            }
        }
    }
}

impl std::error::Error for TopologyError {}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureTopology {
    points: Vec<[f32; 3]>,
    edges: Vec<DirectedEdge>,
    face_metadata: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkEntry {
    pub edge: EdgeId,
    pub projection: f64,
    pub previous_best: f64,
    pub accepted: bool,
}

impl FeatureTopology {
    pub fn from_collision(
        shape: &playsrc_collision::PhysicsShape,
        convex: usize,
    ) -> Result<Self, TopologyError> {
        let authored = shape
            .authored_convex(convex)
            .ok_or(TopologyError::MissingAuthoredConvex { convex })?;
        Self::from_authored(authored)
    }

    pub fn from_collision_hull(
        shape: &playsrc_collision::PhysicsShape,
        hull: playsrc_collision::AuthoredHullRef,
    ) -> Result<Self, TopologyError> {
        let authored = shape.authored_hull(hull).ok_or(match hull {
            playsrc_collision::AuthoredHullRef::Piece(convex) => {
                TopologyError::MissingAuthoredConvex { convex }
            }
            playsrc_collision::AuthoredHullRef::Enclosure(enclosure) => {
                TopologyError::MissingAuthoredEnclosure { enclosure }
            }
        })?;
        Self::from_authored(authored)
    }

    fn from_authored(authored: &playsrc_collision::AuthoredConvex) -> Result<Self, TopologyError> {
        let faces = authored
            .triangles
            .iter()
            .map(|triangle| AuthoredFace {
                metadata: triangle.metadata(),
                vertices: triangle.vertices,
                edge_words: triangle.edge_words(),
            })
            .collect::<Vec<_>>();
        Self::new(authored.points.clone(), &faces)
    }

    pub fn new(points: Vec<[f32; 3]>, faces: &[AuthoredFace]) -> Result<Self, TopologyError> {
        if points.is_empty() || faces.is_empty() {
            return Err(TopologyError::Empty);
        }
        for (point, value) in points.iter().enumerate() {
            if let Some((axis, _)) = value
                .iter()
                .enumerate()
                .find(|(_, coordinate)| !coordinate.is_finite())
            {
                return Err(TopologyError::NonFinitePoint { point, axis });
            }
        }
        let capacity = faces
            .len()
            .checked_mul(3)
            .ok_or(TopologyError::EdgeCountOverflow)?;
        let mut edges = Vec::with_capacity(capacity);
        for (face, authored) in faces.iter().enumerate() {
            for edge in 0..3 {
                let start = authored.vertices[2 - edge];
                let end = authored.vertices[2 - (edge + 1) % 3];
                if start == end || start as usize >= points.len() || end as usize >= points.len() {
                    return Err(TopologyError::InvalidEndpoint { face, edge });
                }
                let slot = face
                    .checked_mul(4)
                    .and_then(|value| value.checked_add(edge + 1))
                    .ok_or(TopologyError::EdgeCountOverflow)?;
                let word = authored.edge_words[edge];
                let offset = ((word.wrapping_shl(1) as i32) >> 17) as isize;
                let opposite_slot = slot
                    .checked_add_signed(offset)
                    .ok_or(TopologyError::InvalidOpposite { face, edge })?;
                let opposite_face = opposite_slot / 4;
                let opposite_position = opposite_slot % 4;
                if opposite_face >= faces.len() || opposite_position == 0 {
                    return Err(TopologyError::InvalidOpposite { face, edge });
                }
                edges.push(DirectedEdge {
                    face,
                    position: edge,
                    start,
                    end,
                    word,
                    opposite: EdgeId(opposite_face * 3 + opposite_position - 1),
                    virtual_edge: word & 0x8000_0000 != 0,
                });
            }
        }
        for (index, edge) in edges.iter().enumerate() {
            let opposite = edges[edge.opposite.0];
            if opposite.opposite.0 != index
                || opposite.start != edge.end
                || opposite.end != edge.start
            {
                return Err(TopologyError::NonReciprocalOpposite {
                    face: edge.face,
                    edge: edge.position,
                });
            }
        }
        Ok(Self {
            points,
            edges,
            face_metadata: faces.iter().map(|face| face.metadata).collect(),
        })
    }

    pub fn points(&self) -> &[[f32; 3]] {
        &self.points
    }

    pub fn storage_bytes(&self) -> usize {
        self.points.capacity() * std::mem::size_of::<[f32; 3]>()
            + self.edges.capacity() * std::mem::size_of::<DirectedEdge>()
            + self.face_metadata.capacity() * std::mem::size_of::<u32>()
    }

    pub fn edges(&self) -> &[DirectedEdge] {
        &self.edges
    }

    pub fn edge_id(&self, index: usize) -> Option<EdgeId> {
        (index < self.edges.len()).then_some(EdgeId(index))
    }

    pub fn edge(&self, identity: EdgeId) -> Result<DirectedEdge, TopologyError> {
        self.edges
            .get(identity.0)
            .copied()
            .ok_or(TopologyError::InvalidEdge)
    }

    pub fn face_metadata(&self, identity: EdgeId) -> Result<u32, TopologyError> {
        Ok(self.face_metadata[self.edge(identity)?.face])
    }

    pub fn next(&self, identity: EdgeId) -> Result<EdgeId, TopologyError> {
        let edge = self.edge(identity)?;
        Ok(EdgeId(edge.face * 3 + (edge.position + 1) % 3))
    }

    pub fn previous(&self, identity: EdgeId) -> Result<EdgeId, TopologyError> {
        let edge = self.edge(identity)?;
        Ok(EdgeId(edge.face * 3 + (edge.position + 2) % 3))
    }

    pub fn opposite(&self, identity: EdgeId) -> Result<EdgeId, TopologyError> {
        Ok(self.edge(identity)?.opposite)
    }

    pub fn fan(&self, first: EdgeId) -> Result<Vec<EdgeId>, TopologyError> {
        self.edge(first)?;
        let mut visited = vec![first];
        let mut current = first;
        for _ in 0..self.edges.len() {
            current = self.previous(self.opposite(current)?)?;
            if current == first {
                return Ok(visited);
            }
            if visited.contains(&current) {
                return Err(TopologyError::OpenFan);
            }
            visited.push(current);
        }
        Err(TopologyError::OpenFan)
    }

    pub fn walk(
        &self,
        first: EdgeId,
        separation: [f64; 3],
    ) -> Result<Vec<WalkEntry>, TopologyError> {
        if let Some((axis, _)) = separation
            .iter()
            .enumerate()
            .find(|(_, coordinate)| !coordinate.is_finite())
        {
            return Err(TopologyError::NonFiniteSeparation { axis });
        }
        let mut entries = Vec::new();
        let mut best = 0.0;
        for identity in self.fan(first)? {
            let edge = self.edge(identity)?;
            let start = self.points[edge.start as usize];
            let end = self.points[edge.end as usize];
            let displacement: [f32; 3] = std::array::from_fn(|axis| start[axis] - end[axis]);
            let projection = separation[1] * f64::from(displacement[1])
                + separation[0] * f64::from(displacement[0])
                + separation[2] * f64::from(displacement[2]);
            let previous_best = best;
            if projection < 0.0 {
                let squared = (f64::from(displacement[1]) * f64::from(displacement[1])
                    + f64::from(displacement[0]) * f64::from(displacement[0])
                    + f64::from(displacement[2]) * f64::from(displacement[2]))
                    as f32;
                let normalized = projection * f64::from(reciprocal_square_root(squared));
                if normalized < best {
                    best = normalized;
                }
            }
            entries.push(WalkEntry {
                edge: identity,
                projection,
                previous_best,
                accepted: best < previous_best,
            });
        }
        Ok(entries)
    }
}

fn reciprocal_square_root(value: f32) -> f32 {
    reciprocal_square_root_f64(f64::from(value)) as f32
}

pub(crate) fn reciprocal_square_root_f64(value: f64) -> f64 {
    let high = (value.to_bits() >> 32) as u32;
    let exponent = (0x7ff0_0000_u32.wrapping_sub(high) as i32) >> 1;
    let mut estimate = f64::from_bits(u64::from((exponent as u32).wrapping_add(0x1ff0_0000)) << 32);
    let half = value * 0.5;
    for _ in 0..4 {
        estimate *= 1.5 - estimate * estimate * half;
    }
    estimate
}

#[cfg(test)]
mod tests {
    use super::{AuthoredFace, FeatureTopology, TopologyError};

    fn fixture() -> ([AuthoredFace; 2], Vec<[f32; 3]>) {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        (
            [
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
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        )
    }

    #[test]
    fn collision_handoff_preserves_authored_points_and_directed_words() {
        let source_points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let internal_points = source_points
            .iter()
            .map(|point| point.map(|axis| axis * 0.0254))
            .collect::<Vec<_>>();
        let faces = vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
        let authored = faces
            .iter()
            .enumerate()
            .map(|(face, vertices)| {
                let mut raw = [0; 16];
                raw[..4].copy_from_slice(&(0x5200_0000_u32 | face as u32).to_le_bytes());
                for edge in 0..3 {
                    let start = vertices[2 - edge];
                    let end = vertices[2 - (edge + 1) % 3];
                    let (opposite_face, opposite_edge) = faces
                        .iter()
                        .enumerate()
                        .flat_map(|(index, candidate)| {
                            (0..3).map(move |position| (index, position, candidate))
                        })
                        .find_map(|(index, position, candidate)| {
                            (candidate[2 - position] == end
                                && candidate[2 - (position + 1) % 3] == start)
                                .then_some((index, position))
                        })
                        .unwrap();
                    let slot = face * 4 + edge + 1;
                    let opposite_slot = opposite_face * 4 + opposite_edge + 1;
                    let relative = opposite_slot as i32 - slot as i32;
                    let word = start | (((relative as u32) & 0x7fff) << 16);
                    raw[4 + edge * 4..8 + edge * 4].copy_from_slice(&word.to_le_bytes());
                }
                playsrc_collision::AuthoredTriangle {
                    vertices: *vertices,
                    raw,
                }
            })
            .collect::<Vec<_>>();
        let shape = playsrc_collision::PhysicsShape::compile(
            19,
            vec![playsrc_collision::ConvexInput {
                solid: 2,
                convex: 4,
                contents: 1,
                vertices: source_points.clone(),
                triangles: faces.clone(),
                authored: Some(playsrc_collision::AuthoredConvex {
                    raw_header: [80, 0, 0, 0, 0, 0, 0, 0, 4, 9, 0, 0, 4, 0, 0, 0],
                    points: internal_points.clone(),
                    triangles: authored.clone(),
                }),
            }],
            playsrc_collision::SnapshotLimits::default(),
        )
        .unwrap();
        let topology = FeatureTopology::from_collision(&shape, 0).unwrap();
        assert_eq!(topology.points(), internal_points);
        assert_eq!(topology.edges().len(), 12);
        for (face, triangle) in authored.iter().enumerate() {
            assert_eq!(triangle.metadata(), 0x5200_0000 | face as u32);
            assert_eq!(
                topology
                    .face_metadata(topology.edge_id(face * 3).unwrap())
                    .unwrap(),
                triangle.metadata()
            );
            assert_eq!(
                triangle.edge_words(),
                std::array::from_fn(|edge| topology.edges()[face * 3 + edge].word)
            );
        }

        let ordinary_shape = playsrc_collision::PhysicsShape::compile(
            20,
            vec![playsrc_collision::ConvexInput {
                solid: 2,
                convex: 4,
                contents: 1,
                vertices: source_points,
                triangles: faces,
                authored: None,
            }],
            playsrc_collision::SnapshotLimits::default(),
        )
        .unwrap();
        assert_eq!(
            FeatureTopology::from_collision(&ordinary_shape, 0).unwrap_err(),
            TopologyError::MissingAuthoredConvex { convex: 0 }
        );
    }

    #[test]
    fn directed_edges_skip_face_headers_and_keep_reciprocal_links() {
        let (faces, points) = fixture();
        let topology = FeatureTopology::new(points, &faces).unwrap();
        assert_eq!(topology.edges().len(), 6);
        assert_eq!(topology.edges()[0].opposite.index(), 5);
        assert_eq!(topology.edges()[1].opposite.index(), 4);
        assert_eq!(topology.edges()[2].opposite.index(), 3);
        let first = topology.edge_id(0).unwrap();
        assert_eq!(topology.previous(first).unwrap().index(), 2);
        assert_eq!(topology.next(topology.edge_id(2).unwrap()).unwrap(), first);
        assert_eq!(topology.fan(first).unwrap().len(), 2);
    }

    #[test]
    fn strict_projection_preserves_the_first_equal_feature() {
        let (faces, points) = fixture();
        let topology = FeatureTopology::new(points, &faces).unwrap();
        let walk = topology
            .walk(topology.edge_id(0).unwrap(), [-1.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(walk.len(), 2);
        assert!(!walk[0].accepted);
        assert!(!walk[1].accepted);
    }

    #[test]
    fn target_width_projection_and_normalized_replacement_are_bit_exact() {
        let (faces, _) = fixture();
        let points = vec![
            [
                f32::from_bits(0xbced_674a),
                f32::from_bits(0x3ddd_8014),
                f32::from_bits(0xb0ae_19de),
            ],
            [
                f32::from_bits(0xbd65_5066),
                f32::from_bits(0x3dc6_977b),
                f32::from_bits(0xb128_2b2f),
            ],
            [0.0, 0.0, 0.0],
        ];
        let topology = FeatureTopology::new(points, &faces).unwrap();
        let walk = topology
            .walk(
                topology.edge_id(0).unwrap(),
                [
                    0.382_602_181_017_636_8,
                    -0.923_913_183_735_472_3,
                    5.620_819_447_971_243e-7,
                ],
            )
            .unwrap();
        assert_eq!(walk[0].projection.to_bits(), 0xbec5_a6aa_88b8_9856);
        assert!(walk[0].accepted);
        assert_eq!(walk[1].previous_best.to_bits(), 0xbf17_25b6_1f55_5ee6);
    }

    #[test]
    fn malformed_endpoints_and_links_are_rejected() {
        let (mut faces, points) = fixture();
        faces[0].vertices[0] = 9;
        assert!(matches!(
            FeatureTopology::new(points.clone(), &faces),
            Err(TopologyError::InvalidEndpoint { face: 0, .. })
        ));
        let (mut faces, _) = fixture();
        faces[0].edge_words[0] = 0;
        assert!(matches!(
            FeatureTopology::new(points, &faces),
            Err(TopologyError::NonReciprocalOpposite { .. })
                | Err(TopologyError::InvalidOpposite { .. })
        ));
    }

    #[test]
    fn non_finite_shape_and_walk_inputs_are_rejected() {
        let (faces, mut points) = fixture();
        points[1][2] = f32::NAN;
        assert_eq!(
            FeatureTopology::new(points, &faces).unwrap_err(),
            TopologyError::NonFinitePoint { point: 1, axis: 2 }
        );
        let (faces, points) = fixture();
        let topology = FeatureTopology::new(points, &faces).unwrap();
        assert_eq!(
            topology
                .walk(topology.edge_id(0).unwrap(), [0.0, f64::INFINITY, 0.0])
                .unwrap_err(),
            TopologyError::NonFiniteSeparation { axis: 1 }
        );
    }
}
