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
    NonFinitePoint { point: usize, axis: usize },
    EdgeCountOverflow,
    InvalidEndpoint { face: usize, edge: usize },
    InvalidOpposite { face: usize, edge: usize },
    NonReciprocalOpposite { face: usize, edge: usize },
    InvalidEdge,
    OpenFan,
    NonFiniteSeparation { axis: usize },
}

impl fmt::Display for TopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("contact topology requires points and faces"),
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
            Self::InvalidEdge => formatter.write_str("contact edge identity is invalid"),
            Self::OpenFan => formatter.write_str("contact edge fan does not close"),
            Self::NonFiniteSeparation { axis } => {
                write!(formatter, "contact separation axis {axis} is not finite")
            }
        }
    }
}

impl std::error::Error for TopologyError {}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureTopology {
    points: Vec<[f32; 3]>,
    edges: Vec<DirectedEdge>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkEntry {
    pub edge: EdgeId,
    pub projection: f64,
    pub previous_best: f64,
    pub accepted: bool,
}

impl FeatureTopology {
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
        Ok(Self { points, edges })
    }

    pub fn points(&self) -> &[[f32; 3]] {
        &self.points
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
    let value = f64::from(value);
    let high = (value.to_bits() >> 32) as u32;
    let exponent = (0x7ff0_0000_u32.wrapping_sub(high) as i32) >> 1;
    let mut estimate = f64::from_bits(u64::from((exponent as u32).wrapping_add(0x1ff0_0000)) << 32);
    let half = value * 0.5;
    for _ in 0..4 {
        estimate *= 1.5 - estimate * estimate * half;
    }
    estimate as f32
}

#[cfg(test)]
mod tests {
    use super::{AuthoredFace, FeatureTopology, TopologyError};

    fn fixture() -> ([AuthoredFace; 2], Vec<[f32; 3]>) {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        (
            [
                AuthoredFace {
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        )
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
