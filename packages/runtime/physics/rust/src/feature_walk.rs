use crate::topology::reciprocal_square_root_f64;
use crate::{EdgeId, FeatureTopology, TopologyError};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SurfaceFeatureKind {
    Vertex,
    Edge,
    Face,
    InteriorFace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceFeature {
    pub edge: EdgeId,
    pub kind: SurfaceFeatureKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceFeaturePair {
    pub first: SurfaceFeature,
    pub second: SurfaceFeature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureSelection {
    /// Feature identities in the input placement order.
    pub pair: SurfaceFeaturePair,
    /// Input placement selected as the first endpoint by minimization.
    first_side: usize,
}

impl FeatureSelection {
    pub(crate) const fn initial(pair: SurfaceFeaturePair) -> Self {
        Self {
            pair,
            first_side: 0,
        }
    }
    pub fn order(self) -> [usize; 2] {
        [self.first_side, 1 - self.first_side]
    }
    pub fn ordered_pair(self) -> SurfaceFeaturePair {
        if self.first_side == 0 {
            self.pair
        } else {
            SurfaceFeaturePair {
                first: self.pair.second,
                second: self.pair.first,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePairSeparation {
    pub distance: f32,
    pub normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexPairSeparation {
    pub distance: f32,
    pub normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexFaceSeparation {
    pub distance: f32,
    pub normal: [f32; 3],
}

impl VertexFaceSeparation {
    pub fn evaluate(
        placements: [FeaturePlacement<'_>; 2],
        features: [EdgeId; 2],
        extra_radius: f32,
    ) -> Result<Self, FeatureWalkError> {
        for placement in placements {
            if placement
                .position
                .iter()
                .chain(placement.orientation.iter())
                .any(|value| !value.is_finite())
            {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
        }
        if !extra_radius.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let [point, face] = placements;
        let local = face.local_point(point.world_point(features[0])?);
        let origin = face.point(features[1])?.map(f64::from);
        let next = face.point(face.topology.next(features[1])?)?.map(f64::from);
        let previous = face
            .point(face.topology.previous(features[1])?)?
            .map(f64::from);
        let unscaled = cross(sub(next, origin), sub(previous, origin));
        let squared = dot(unscaled, unscaled);
        if !squared.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let normal = if squared >= 1.0e-19 {
            let inverse = crate::arithmetic::refined_inverse_root::<4>(squared);
            unscaled.map(|value| value * inverse)
        } else {
            unscaled
        };
        let distance = (dot(normal, local) - dot(normal, origin)) as f32 - extra_radius;
        let normal = face.world_vector(normal).map(|value| value as f32);
        if !distance.is_finite() || normal.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(Self { distance, normal })
    }
}

impl VertexPairSeparation {
    pub fn evaluate(
        placements: [FeaturePlacement<'_>; 2],
        vertices: [EdgeId; 2],
        extra_radius: f32,
    ) -> Result<Self, FeatureWalkError> {
        for placement in placements {
            if placement
                .position
                .iter()
                .chain(placement.orientation.iter())
                .any(|value| !value.is_finite())
            {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
        }
        if !extra_radius.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let delta = sub(
            placements[0].world_point(vertices[0])?,
            placements[1].world_point(vertices[1])?,
        );
        let squared = dot(delta, delta);
        if !squared.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        if squared <= 1.0e-12 {
            return Err(FeatureWalkError::CoincidentVertices);
        }
        let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
        let result = Self {
            distance: (squared * inverse - f64::from(extra_radius)) as f32,
            normal: delta.map(|value| (value * inverse) as f32),
        };
        if !result.distance.is_finite() || result.normal.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexEdgeSeparation {
    pub distance: f32,
    pub normal: [f32; 3],
}

impl VertexEdgeSeparation {
    pub fn evaluate(
        placements: [FeaturePlacement<'_>; 2],
        features: [EdgeId; 2],
        extra_radius: f32,
    ) -> Result<Self, FeatureWalkError> {
        for placement in placements {
            if placement
                .position
                .iter()
                .chain(placement.orientation.iter())
                .any(|value| !value.is_finite())
            {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
        }
        if !extra_radius.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let [vertex, edge] = placements;
        let point = edge.local_point(vertex.world_point(features[0])?);
        let origin = edge.point(features[1])?.map(f64::from);
        let direction = edge.edge_direction(features[1])?;
        let inverse_edge = 1.0 / dot(direction, direction);
        let perpendicular = cross(direction, sub(point, origin));
        let squared = dot(perpendicular, perpendicular) * inverse_edge;
        if !squared.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let (distance, normal) = if squared > 1.0e-19 {
            let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
            let scale = -(inverse * inverse_edge);
            (
                (squared * inverse - f64::from(extra_radius)) as f32,
                edge.world_vector(cross(direction, perpendicular))
                    .map(|value| (value * scale) as f32),
            )
        } else {
            let mut maximum = 0.0_f64;
            let mut selected = 0;
            for axis in (0..3).rev() {
                if direction[axis].abs() > maximum {
                    maximum = direction[axis].abs();
                    selected = axis;
                }
            }
            let previous = (selected + 2) % 3;
            let mut other = direction;
            other[selected] = direction[previous];
            other[previous] = -direction[selected];
            let mut normal = cross(other, direction);
            let squared = dot(normal, normal);
            if squared >= 1.0e-19 {
                let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
                normal = normal.map(|value| value * inverse);
            }
            (-extra_radius, normal.map(|value| value as f32))
        };
        if !distance.is_finite() || normal.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(Self { distance, normal })
    }
}

impl EdgePairSeparation {
    pub fn evaluate(
        placements: [FeaturePlacement<'_>; 2],
        edges: [EdgeId; 2],
        extra_radius: f32,
    ) -> Result<Self, FeatureWalkError> {
        for shape in placements {
            if shape
                .position
                .iter()
                .chain(shape.orientation.iter())
                .any(|value| !value.is_finite())
            {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
        }
        if !extra_radius.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let [first, second] = placements;
        let a = second.local_point(first.world_point(edges[0])?);
        let c = second.point(edges[1])?.map(f64::from);
        let perpendicular = cross(
            second.local_vector(first.world_vector(first.edge_direction(edges[0])?)),
            second.edge_direction(edges[1])?,
        );
        let projected = dot(sub(c, a), perpendicular) as f32;
        let squared = (perpendicular[0] * perpendicular[0] + perpendicular[1] * perpendicular[1])
            + perpendicular[2] * perpendicular[2];
        if squared == 0.0 {
            return Err(FeatureWalkError::ParallelEdges);
        }
        let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
        let scale = if projected.is_sign_negative() {
            inverse
        } else {
            -inverse
        };
        let normal = second
            .world_vector(perpendicular)
            .map(|value| (value * scale) as f32);
        let distance = (f64::from(projected) * inverse).abs() as f32 - extra_radius;
        if !distance.is_finite() || normal.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(Self { distance, normal })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeaturePlacement<'a> {
    pub topology: &'a FeatureTopology,
    pub position: [f64; 3],
    pub orientation: [f64; 9],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureTransitionKind {
    VertexPair,
    VertexEdge,
    EdgeRegion,
    VertexFace,
    FaceRegion,
    EdgePair,
    EdgePairRegion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureTransition {
    pub kind: FeatureTransitionKind,
    pub first_side: usize,
    pub first_edge: EdgeId,
    pub second_side: usize,
    pub second_edge: EdgeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureWalkError {
    NonFiniteTransform,
    InvalidTransitionLimit,
    TransitionLimit,
    UnsupportedFeaturePair,
    ParallelEdges,
    CoincidentVertices,
    Intruded,
    Topology(TopologyError),
}

impl fmt::Display for FeatureWalkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteTransform => {
                formatter.write_str("compact feature placement contains a non-finite scalar")
            }
            Self::InvalidTransitionLimit => {
                formatter.write_str("compact feature transition limit must be positive")
            }
            Self::TransitionLimit => {
                formatter.write_str("compact feature transition limit reached")
            }
            Self::UnsupportedFeaturePair => {
                formatter.write_str("compact feature pair requires an unestablished transition")
            }
            Self::ParallelEdges => {
                formatter.write_str("parallel compact edges require an unestablished transition")
            }
            Self::CoincidentVertices => {
                formatter.write_str("vertex-pair separation is below the collision resolution")
            }
            Self::Intruded => formatter.write_str("compact convex features are intruding"),
            Self::Topology(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FeatureWalkError {}

impl From<TopologyError> for FeatureWalkError {
    fn from(error: TopologyError) -> Self {
        Self::Topology(error)
    }
}

#[derive(Clone, Copy)]
struct FeatureState {
    kind: FeatureTransitionKind,
    first_side: usize,
    first_edge: EdgeId,
    second_side: usize,
    second_edge: EdgeId,
}

impl FeatureState {
    fn transition(self) -> FeatureTransition {
        FeatureTransition {
            kind: self.kind,
            first_side: self.first_side,
            first_edge: self.first_edge,
            second_side: self.second_side,
            second_edge: self.second_edge,
        }
    }
}

impl FeaturePlacement<'_> {
    pub(crate) fn edge_direction(self, edge: EdgeId) -> Result<[f64; 3], FeatureWalkError> {
        let start = self.point(edge)?;
        let end = self.point(self.topology.next(edge)?)?;
        Ok(std::array::from_fn(|axis| {
            f64::from(end[axis] - start[axis])
        }))
    }
    pub(crate) fn point(self, edge: EdgeId) -> Result<[f32; 3], FeatureWalkError> {
        Ok(self.topology.points()[self.topology.edge(edge)?.start as usize])
    }

    pub(crate) fn world_point(self, edge: EdgeId) -> Result<[f64; 3], FeatureWalkError> {
        let point = self.point(edge)?.map(f64::from);
        Ok(add(self.position, self.world_vector(point)))
    }

    pub(crate) fn world_vector(self, point: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|row| {
            self.orientation[row * 3 + 1] * point[1]
                + self.orientation[row * 3] * point[0]
                + self.orientation[row * 3 + 2] * point[2]
        })
    }

    pub(crate) fn local_point(self, point: [f64; 3]) -> [f64; 3] {
        self.local_vector(sub(point, self.position))
    }

    pub(crate) fn local_vector(self, point: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|column| {
            self.orientation[3 + column] * point[1]
                + self.orientation[column] * point[0]
                + self.orientation[6 + column] * point[2]
        })
    }
}

pub fn walk_compact_features(
    first: FeaturePlacement<'_>,
    second: FeaturePlacement<'_>,
    seed: SurfaceFeaturePair,
    maximum_transitions: usize,
    observe: impl FnMut(FeatureTransition),
) -> Result<FeatureSelection, FeatureWalkError> {
    let result = minimize_features(
        [first, second],
        FeatureSelection::initial(seed),
        0.0,
        crate::ClosestFeatureMode::Ordinary,
        maximum_transitions,
        observe,
    )?;
    if result.status == crate::ClosestFeatureStatus::Separated {
        Ok(result.selection)
    } else {
        Err(FeatureWalkError::Intruded)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WalkGeometry {
    pub first_side: usize,
    pub separation: f32,
    pub normal: [f32; 3],
}
pub(crate) struct MinimizedFeatures {
    pub selection: FeatureSelection,
    pub geometry: Option<WalkGeometry>,
    pub status: crate::ClosestFeatureStatus,
}
struct WalkContext {
    selection: FeatureSelection,
    geometry: Option<WalkGeometry>,
    extra: f32,
    fast_left: i32,
    cycles: Vec<[(usize, SurfaceFeatureKind); 2]>,
    shared_geometry: bool,
    transitions: usize,
}
#[derive(Clone, Copy)]
enum WalkOutcome {
    Separated,
    Intruded,
    Interior { side: usize, point: [f64; 3] },
}
impl WalkContext {
    fn admit(&mut self, state: FeatureState) -> bool {
        let kinds = match state.kind {
            FeatureTransitionKind::VertexPair => [SurfaceFeatureKind::Vertex; 2],
            FeatureTransitionKind::EdgeRegion => {
                [SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Edge]
            }
            FeatureTransitionKind::FaceRegion => {
                [SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Face]
            }
            FeatureTransitionKind::EdgePairRegion => [SurfaceFeatureKind::Edge; 2],
            _ => return true,
        };
        self.selection.first_side = state.first_side;
        self.fast_left -= 1;
        if self.fast_left >= 0 {
            return true;
        }
        let mut key = [(0, SurfaceFeatureKind::Vertex); 2];
        key[state.first_side] = (state.first_edge.index(), kinds[0]);
        key[state.second_side] = (state.second_edge.index(), kinds[1]);
        if self.shared_geometry {
            key.sort_unstable();
        }
        if self.cycles.len() == 256 || self.cycles.iter().rev().any(|entry| *entry == key) {
            return false;
        }
        self.cycles.push(key);
        true
    }
    fn geometry(&mut self, state: FeatureState, separation: f32, normal: [f32; 3]) {
        self.geometry = Some(WalkGeometry {
            first_side: state.first_side,
            separation,
            normal,
        });
    }
    fn finish(
        &mut self,
        state: FeatureState,
        first: SurfaceFeatureKind,
        second: SurfaceFeatureKind,
    ) -> Result<WalkOutcome, FeatureWalkError> {
        self.selection = finish(state, first, second)?;
        Ok(WalkOutcome::Separated)
    }
    fn interior(
        &mut self,
        state: FeatureState,
        point: [f64; 3],
    ) -> Result<WalkOutcome, FeatureWalkError> {
        self.selection = finish(
            state,
            SurfaceFeatureKind::Vertex,
            SurfaceFeatureKind::InteriorFace,
        )?;
        Ok(WalkOutcome::Interior {
            side: state.second_side,
            point,
        })
    }
}

pub(crate) fn minimize_features(
    shapes: [FeaturePlacement<'_>; 2],
    seed: FeatureSelection,
    extra: f32,
    mode: crate::ClosestFeatureMode,
    maximum_transitions: usize,
    mut observe: impl FnMut(FeatureTransition),
) -> Result<MinimizedFeatures, FeatureWalkError> {
    if maximum_transitions == 0 {
        return Err(FeatureWalkError::InvalidTransitionLimit);
    }
    if !extra.is_finite() {
        return Err(FeatureWalkError::NonFiniteTransform);
    }
    for shape in shapes {
        if shape
            .position
            .iter()
            .chain(shape.orientation.iter())
            .any(|value| !value.is_finite())
        {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
    }
    let mut context = WalkContext {
        selection: seed,
        geometry: None,
        extra,
        fast_left: if mode == crate::ClosestFeatureMode::Ordinary {
            20
        } else {
            0
        },
        cycles: Vec::new(),
        shared_geometry: std::ptr::eq(shapes[0].topology, shapes[1].topology),
        transitions: 0,
    };
    let mut recoveries = 0;
    let status = loop {
        match walk_attempt(shapes, &mut context, maximum_transitions, &mut observe)? {
            WalkOutcome::Separated => break crate::ClosestFeatureStatus::Separated,
            WalkOutcome::Intruded => break crate::ClosestFeatureStatus::Intruded,
            WalkOutcome::Interior { side, point } => {
                let feature = if side == 0 {
                    &mut context.selection.pair.first
                } else {
                    &mut context.selection.pair.second
                };
                feature.edge =
                    shapes[side]
                        .topology
                        .recover_interior_face(feature.edge, point, |_| {})?;
                feature.kind = SurfaceFeatureKind::Face;
                recoveries += 1;
                if recoveries == 2 {
                    break crate::ClosestFeatureStatus::RecoveryLimit;
                }
            }
        }
    };
    if context.geometry.is_some_and(|geometry| {
        !geometry.separation.is_finite() || geometry.normal.iter().any(|value| !value.is_finite())
    }) {
        return Err(FeatureWalkError::NonFiniteTransform);
    }
    Ok(MinimizedFeatures {
        selection: context.selection,
        geometry: context.geometry,
        status,
    })
}

fn walk_attempt(
    shapes: [FeaturePlacement<'_>; 2],
    context: &mut WalkContext,
    maximum_transitions: usize,
    observe: &mut impl FnMut(FeatureTransition),
) -> Result<WalkOutcome, FeatureWalkError> {
    let order = context.selection.order();
    let seed = context.selection.ordered_pair();
    shapes[order[0]].topology.edge(seed.first.edge)?;
    shapes[order[1]].topology.edge(seed.second.edge)?;
    if seed.first.kind == SurfaceFeatureKind::Face && seed.second.kind == SurfaceFeatureKind::Face {
        let (pair, first) = crate::triangle_pair::select_triangle_features(
            order.map(|side| shapes[side]),
            [seed.first.edge, seed.second.edge],
        )?;
        context.selection = FeatureSelection {
            pair: if order[0] == 0 {
                pair
            } else {
                SurfaceFeaturePair {
                    first: pair.second,
                    second: pair.first,
                }
            },
            first_side: order[first],
        };
        return walk_attempt(shapes, context, maximum_transitions, observe);
    }
    let mut state = match (seed.first.kind, seed.second.kind) {
        (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Vertex) => FeatureState {
            kind: FeatureTransitionKind::VertexPair,
            first_side: order[0],
            first_edge: seed.first.edge,
            second_side: order[1],
            second_edge: seed.second.edge,
        },
        (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Edge) => FeatureState {
            kind: FeatureTransitionKind::VertexEdge,
            first_side: order[0],
            first_edge: seed.first.edge,
            second_side: order[1],
            second_edge: seed.second.edge,
        },
        (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Face) => FeatureState {
            kind: FeatureTransitionKind::VertexFace,
            first_side: order[0],
            first_edge: seed.first.edge,
            second_side: order[1],
            second_edge: seed.second.edge,
        },
        (SurfaceFeatureKind::Edge, SurfaceFeatureKind::Vertex) => FeatureState {
            kind: FeatureTransitionKind::VertexEdge,
            first_side: order[1],
            first_edge: seed.second.edge,
            second_side: order[0],
            second_edge: seed.first.edge,
        },
        (SurfaceFeatureKind::Face, SurfaceFeatureKind::Vertex) => FeatureState {
            kind: FeatureTransitionKind::VertexFace,
            first_side: order[1],
            first_edge: seed.second.edge,
            second_side: order[0],
            second_edge: seed.first.edge,
        },
        (SurfaceFeatureKind::Edge, SurfaceFeatureKind::Edge) => FeatureState {
            kind: FeatureTransitionKind::EdgePair,
            first_side: order[0],
            first_edge: seed.first.edge,
            second_side: order[1],
            second_edge: seed.second.edge,
        },
        _ => return Err(FeatureWalkError::UnsupportedFeaturePair),
    };
    for _ in 0..maximum_transitions {
        if context.transitions == maximum_transitions {
            return Err(FeatureWalkError::TransitionLimit);
        }
        context.transitions += 1;
        observe(state.transition());
        if !context.admit(state) {
            return Ok(WalkOutcome::Intruded);
        }
        match state.kind {
            FeatureTransitionKind::VertexPair => {
                let sides = [state.first_side, state.second_side];
                let edges = [state.first_edge, state.second_edge];
                let points = [
                    shapes[sides[0]].world_point(edges[0])?,
                    shapes[sides[1]].world_point(edges[1])?,
                ];
                let delta = sub(points[0], points[1]);
                if dot(delta, delta) <= 1.0e-12 {
                    return Ok(WalkOutcome::Intruded);
                }
                let geometry = VertexPairSeparation::evaluate(
                    sides.map(|side| shapes[side]),
                    edges,
                    context.extra,
                )?;
                context.geometry(state, geometry.distance, geometry.normal);
                let mut winner = None;
                let mut maximum = 0.0;
                for point_index in (0..2).rev() {
                    let edge_index = 1 - point_index;
                    let shape = shapes[sides[edge_index]];
                    let source = shape.point(edges[edge_index])?;
                    let toward = sub(
                        shape.local_point(points[point_index]),
                        source.map(f64::from),
                    );
                    let baseline = dot(toward, source.map(f64::from));
                    for candidate in adjacent_vertices(shape.topology, edges[edge_index])? {
                        let destination = shape.point(candidate)?;
                        let mut gradient = dot(toward, destination.map(f64::from)) - baseline;
                        if gradient <= 0.0 {
                            continue;
                        }
                        let delta: [f32; 3] =
                            std::array::from_fn(|axis| destination[axis] - source[axis]);
                        let squared = dot(delta.map(f64::from), delta.map(f64::from)) as f32;
                        gradient *=
                            f64::from(reciprocal_square_root_f64(f64::from(squared)) as f32);
                        if gradient > maximum {
                            maximum = gradient;
                            winner = Some((
                                sides[edge_index],
                                candidate,
                                sides[point_index],
                                edges[point_index],
                            ));
                        }
                    }
                }
                let Some((edge_side, edge, point_side, point)) = winner else {
                    return context.finish(
                        state,
                        SurfaceFeatureKind::Vertex,
                        SurfaceFeatureKind::Vertex,
                    );
                };
                let projection = segment_checks(
                    shapes[edge_side],
                    edge,
                    shapes[edge_side].local_point(shapes[point_side].world_point(point)?),
                )?;
                if projection[1] <= 0.0 {
                    return context.finish(
                        state,
                        SurfaceFeatureKind::Vertex,
                        SurfaceFeatureKind::Vertex,
                    );
                }
                state = if projection[0] < 0.0 {
                    FeatureState {
                        kind: FeatureTransitionKind::VertexPair,
                        first_side: edge_side,
                        first_edge: edge,
                        second_side: point_side,
                        second_edge: point,
                    }
                } else {
                    FeatureState {
                        kind: FeatureTransitionKind::EdgeRegion,
                        first_side: point_side,
                        first_edge: point,
                        second_side: edge_side,
                        second_edge: edge,
                    }
                };
            }
            FeatureTransitionKind::VertexEdge => {
                let shape = shapes[state.second_side];
                let point =
                    shape.local_point(shapes[state.first_side].world_point(state.first_edge)?);
                let projection = segment_checks(shape, state.second_edge, point)?;
                state = if projection[0] < 0.0 {
                    FeatureState {
                        kind: FeatureTransitionKind::VertexPair,
                        ..state
                    }
                } else if projection[1] < 0.0 {
                    FeatureState {
                        kind: FeatureTransitionKind::VertexPair,
                        second_edge: shape.topology.next(state.second_edge)?,
                        ..state
                    }
                } else {
                    FeatureState {
                        kind: FeatureTransitionKind::EdgeRegion,
                        ..state
                    }
                };
            }
            FeatureTransitionKind::EdgeRegion => {
                let shape = shapes[state.second_side];
                let point =
                    shape.local_point(shapes[state.first_side].world_point(state.first_edge)?);
                let other = shape.topology.opposite(state.second_edge)?;
                let origin = shape.point(state.second_edge)?;
                let along = shape.edge_direction(state.second_edge)?;
                let corner = shape.point(shape.topology.previous(state.second_edge)?)?;
                let other_corner = shape.point(shape.topology.previous(other)?)?;
                let corner: [f64; 3] =
                    std::array::from_fn(|axis| f64::from(corner[axis] - origin[axis]));
                let other_corner: [f64; 3] =
                    std::array::from_fn(|axis| f64::from(other_corner[axis] - origin[axis]));
                let toward = sub(point, origin.map(f64::from));
                let plane_values = [
                    dot(cross(along, corner), toward),
                    dot(cross(other_corner, along), toward),
                ];
                let current_checks = face_checks(shape, state.second_edge, point)?;
                let other_checks = face_checks(shape, other, point)?;
                let selected = if current_checks[0] > 0.0 {
                    if other_checks[0] > 0.0 {
                        if plane_values[1] > 0.0 {
                            other
                        } else {
                            state.second_edge
                        }
                    } else {
                        state.second_edge
                    }
                } else if other_checks[0] > 0.0 {
                    other
                } else {
                    if plane_values.iter().all(|value| *value < 0.0) {
                        return context.interior(state, point);
                    }
                    let start = shape.point(state.second_edge)?.map(f64::from);
                    let toward = sub(point, start);
                    let perpendicular = cross(along, toward);
                    let edge_squared = dot(along, along);
                    if edge_squared == 0.0 {
                        return Err(FeatureWalkError::UnsupportedFeaturePair);
                    }
                    let separation_squared =
                        dot(perpendicular, perpendicular) * (1.0 / edge_squared);
                    let geometry = VertexEdgeSeparation::evaluate(
                        [shapes[state.first_side], shape],
                        [state.first_edge, state.second_edge],
                        context.extra,
                    )?;
                    context.geometry(state, geometry.distance, geometry.normal);
                    let local = shapes[state.first_side]
                        .local_vector(shape.world_vector(cross(along, perpendicular)));
                    let current = shapes[state.first_side].point(state.first_edge)?;
                    let mut best = 1.0e-12 * separation_squared;
                    let mut candidate = None;
                    for edge in
                        adjacent_vertices(shapes[state.first_side].topology, state.first_edge)?
                    {
                        let target = shapes[state.first_side].point(edge)?;
                        let difference: [f32; 3] =
                            std::array::from_fn(|axis| target[axis] - current[axis]);
                        let mut slope = dot(difference.map(f64::from), local);
                        if slope <= 0.0 {
                            continue;
                        }
                        let squared =
                            dot(difference.map(f64::from), difference.map(f64::from)) as f32;
                        slope *= f64::from(reciprocal_square_root_f64(f64::from(squared)) as f32);
                        if slope > best {
                            best = slope;
                            candidate = Some(edge);
                        }
                    }
                    if let Some(edge) = candidate {
                        if best < 1.0e-8 {
                            let check = crate::segment_metric::SegmentPairGeometry::new(
                                [shape, shapes[state.first_side]],
                                [state.second_edge, edge],
                            )?;
                            if !check.direct_regions() || check.regions()?[2] < 0.0 {
                                return context.finish(
                                    state,
                                    SurfaceFeatureKind::Vertex,
                                    SurfaceFeatureKind::Edge,
                                );
                            }
                        }
                        state = FeatureState {
                            kind: FeatureTransitionKind::EdgePair,
                            first_edge: edge,
                            ..state
                        };
                        continue;
                    }
                    return context.finish(
                        state,
                        SurfaceFeatureKind::Vertex,
                        SurfaceFeatureKind::Edge,
                    );
                };
                state = FeatureState {
                    kind: FeatureTransitionKind::VertexFace,
                    second_edge: selected,
                    ..state
                };
            }
            FeatureTransitionKind::VertexFace => {
                let shape = shapes[state.second_side];
                let point =
                    shape.local_point(shapes[state.first_side].world_point(state.first_edge)?);
                let checks = face_checks(shape, state.second_edge, point)?;
                if checks
                    .iter()
                    .all(|check| check.to_bits() & 0x8000_0000 == 0)
                {
                    state.kind = FeatureTransitionKind::FaceRegion;
                    continue;
                }
                let mut selected = state.second_edge;
                let mut minimum = f64::INFINITY;
                let mut edge = state.second_edge;
                for _ in 0..3 {
                    let distance = squared_segment_distance(shape, edge, point)?;
                    if distance < minimum {
                        minimum = distance;
                        selected = edge;
                    }
                    edge = shape.topology.next(edge)?;
                }
                state = FeatureState {
                    kind: FeatureTransitionKind::VertexEdge,
                    second_edge: selected,
                    ..state
                };
            }
            FeatureTransitionKind::FaceRegion => {
                let moving = shapes[state.first_side];
                let fixed = shapes[state.second_side];
                let start = fixed.point(state.second_edge)?.map(f64::from);
                let along = sub(
                    fixed
                        .point(fixed.topology.next(state.second_edge)?)?
                        .map(f64::from),
                    start,
                );
                let across = sub(
                    fixed
                        .point(fixed.topology.previous(state.second_edge)?)?
                        .map(f64::from),
                    start,
                );
                let normal = cross(along, across);
                let squared = dot(normal, normal);
                let normal = if squared >= 1.0e-19 {
                    let inverse = crate::arithmetic::refined_inverse_root::<4>(squared);
                    normal.map(|value| value * inverse)
                } else {
                    normal
                };
                let point = fixed.local_point(moving.world_point(state.first_edge)?);
                let raw_distance = (dot(normal, point) - dot(normal, start)) as f32;
                let mut local_normal = moving.local_vector(fixed.world_vector(normal));
                if raw_distance < 0.0 {
                    local_normal = local_normal.map(|value| -value);
                }
                let distance = raw_distance - context.extra;
                context.geometry(
                    state,
                    distance,
                    fixed.world_vector(normal).map(|value| value as f32),
                );
                let origin = moving.point(state.first_edge)?;
                let mut winner = None;
                let mut minimum = 0.0;
                for candidate in adjacent_vertices(moving.topology, state.first_edge)? {
                    let target = moving.point(candidate)?;
                    let difference: [f32; 3] =
                        std::array::from_fn(|axis| target[axis] - origin[axis]);
                    let mut projection = dot(local_normal, difference.map(f64::from));
                    if projection >= 0.0 {
                        continue;
                    }
                    let squared = dot(difference.map(f64::from), difference.map(f64::from)) as f32;
                    projection *= f64::from(reciprocal_square_root_f64(f64::from(squared)) as f32);
                    if projection < minimum {
                        minimum = projection;
                        winner = Some(candidate);
                    }
                }
                let Some(candidate) = winner else {
                    if distance + context.extra < 0.0 {
                        return context.interior(state, point);
                    }
                    return context.finish(
                        state,
                        SurfaceFeatureKind::Vertex,
                        SurfaceFeatureKind::Face,
                    );
                };
                let point = fixed.local_point(moving.world_point(candidate)?);
                let checks = face_checks(fixed, state.second_edge, point)?;
                if checks
                    .iter()
                    .all(|check| check.to_bits() & 0x8000_0000 == 0)
                {
                    state.first_edge = candidate;
                    continue;
                }
                let negative = checks.iter().filter(|check| **check < 0.0).count();
                let mut selected = state.second_edge;
                if negative == 1 {
                    for check in checks {
                        if check < 0.0 {
                            break;
                        }
                        selected = fixed.topology.next(selected)?;
                    }
                } else {
                    let mut best = 1.0e101;
                    let mut choice = None;
                    let mut edge = state.second_edge;
                    for check in checks {
                        if check <= 0.0 {
                            let distance = crate::SegmentPairMetric::evaluate(
                                [moving, fixed],
                                [candidate, edge],
                            )?
                            .squared_distance;
                            if distance < best {
                                best = distance;
                                choice = Some(edge);
                            }
                        }
                        edge = fixed.topology.next(edge)?;
                    }
                    selected = choice.ok_or(FeatureWalkError::UnsupportedFeaturePair)?;
                }
                state = FeatureState {
                    kind: FeatureTransitionKind::EdgePair,
                    first_edge: candidate,
                    second_edge: selected,
                    ..state
                };
            }
            FeatureTransitionKind::EdgePair => {
                let first_shape = shapes[state.first_side];
                let second_shape = shapes[state.second_side];
                let geometry = crate::segment_metric::SegmentPairGeometry::new(
                    [first_shape, second_shape],
                    [state.first_edge, state.second_edge],
                )?;
                let [a, b, c, d] = geometry.regions()?;
                let first_checks = [a, b];
                let second_checks = [c, d];
                if second_checks
                    .iter()
                    .all(|check| check.to_bits() & 0x8000_0000 == 0)
                {
                    if first_checks[0] < 0.0 {
                        state.kind = FeatureTransitionKind::VertexEdge;
                    } else if first_checks[1] < 0.0 {
                        state = FeatureState {
                            kind: FeatureTransitionKind::VertexEdge,
                            first_edge: first_shape.topology.next(state.first_edge)?,
                            ..state
                        };
                    } else {
                        state.kind = FeatureTransitionKind::EdgePairRegion;
                    }
                } else if first_checks
                    .iter()
                    .all(|check| check.to_bits() & 0x8000_0000 == 0)
                {
                    state = FeatureState {
                        kind: FeatureTransitionKind::VertexEdge,
                        first_side: state.second_side,
                        first_edge: if second_checks[0] < 0.0 {
                            state.second_edge
                        } else {
                            second_shape.topology.next(state.second_edge)?
                        },
                        second_side: state.first_side,
                        second_edge: state.first_edge,
                    };
                } else {
                    let (first_nearest, first_other) = if first_checks[0] < first_checks[1] {
                        (
                            state.first_edge,
                            first_shape.topology.next(state.first_edge)?,
                        )
                    } else {
                        (
                            first_shape.topology.next(state.first_edge)?,
                            state.first_edge,
                        )
                    };
                    let (second_nearest, second_other) = if second_checks[0] < second_checks[1] {
                        (
                            state.second_edge,
                            second_shape.topology.next(state.second_edge)?,
                        )
                    } else {
                        (
                            second_shape.topology.next(state.second_edge)?,
                            state.second_edge,
                        )
                    };
                    let first_on_second = segment_checks(
                        second_shape,
                        state.second_edge,
                        second_shape.local_point(first_shape.world_point(first_nearest)?),
                    )?;
                    if first_on_second
                        .iter()
                        .all(|value| value.to_bits() & 0x8000_0000 == 0)
                    {
                        state = FeatureState {
                            kind: FeatureTransitionKind::VertexEdge,
                            first_edge: first_nearest,
                            ..state
                        };
                        continue;
                    }
                    let second_on_first = segment_checks(
                        first_shape,
                        state.first_edge,
                        first_shape.local_point(second_shape.world_point(second_nearest)?),
                    )?;
                    if second_on_first
                        .iter()
                        .all(|value| value.to_bits() & 0x8000_0000 == 0)
                    {
                        state = FeatureState {
                            kind: FeatureTransitionKind::VertexEdge,
                            first_side: state.second_side,
                            first_edge: second_nearest,
                            second_side: state.first_side,
                            second_edge: state.first_edge,
                        };
                        continue;
                    }
                    state = if first_on_second[0] * second_checks[0] < 0.0 {
                        FeatureState {
                            kind: FeatureTransitionKind::VertexPair,
                            first_edge: first_nearest,
                            second_edge: second_other,
                            ..state
                        }
                    } else if second_on_first[0] * first_checks[0] < 0.0 {
                        FeatureState {
                            kind: FeatureTransitionKind::VertexPair,
                            first_side: state.second_side,
                            first_edge: second_nearest,
                            second_side: state.first_side,
                            second_edge: first_other,
                        }
                    } else {
                        FeatureState {
                            kind: FeatureTransitionKind::VertexPair,
                            first_edge: first_nearest,
                            second_edge: second_nearest,
                            ..state
                        }
                    };
                }
            }
            FeatureTransitionKind::EdgePairRegion => {
                let geometry = EdgePairSeparation::evaluate(
                    [shapes[state.first_side], shapes[state.second_side]],
                    [state.first_edge, state.second_edge],
                    context.extra,
                )?;
                context.geometry(state, geometry.distance, geometry.normal);
                match adjacent_face_transition(shapes, state)? {
                    AdjacentTransition::Next(next) => state = next,
                    AdjacentTransition::Finished => {
                        return context.finish(
                            state,
                            SurfaceFeatureKind::Edge,
                            SurfaceFeatureKind::Edge,
                        );
                    }
                    AdjacentTransition::Interior { side, point } => {
                        let kinds = if side == state.first_side {
                            [SurfaceFeatureKind::InteriorFace, SurfaceFeatureKind::Face]
                        } else {
                            [SurfaceFeatureKind::Face, SurfaceFeatureKind::InteriorFace]
                        };
                        context.selection = finish(state, kinds[0], kinds[1])?;
                        return Ok(WalkOutcome::Interior { side, point });
                    }
                }
            }
        }
    }
    Err(FeatureWalkError::TransitionLimit)
}

#[derive(Clone, Copy)]
struct AdjacentFace {
    face_side: usize,
    face_edge: EdgeId,
    vertex_side: usize,
    vertex_edge: EdgeId,
    projected_vertex: [f64; 3],
    perpendicular: [f64; 3],
    normal: [f64; 3],
}

enum AdjacentTransition {
    Next(FeatureState),
    Finished,
    Interior { side: usize, point: [f64; 3] },
}

fn adjacent_face_transition(
    shapes: [FeaturePlacement<'_>; 2],
    state: FeatureState,
) -> Result<AdjacentTransition, FeatureWalkError> {
    let first = shapes[state.first_side];
    let second = shapes[state.second_side];
    let first_start = second.local_point(first.world_point(state.first_edge)?);
    let first_end = second.local_point(first.world_point(first.topology.next(state.first_edge)?)?);
    let second_start = second.point(state.second_edge)?.map(f64::from);
    let first_direction =
        second.local_vector(first.world_vector(first.edge_direction(state.first_edge)?));
    let second_direction = second.edge_direction(state.second_edge)?;
    let second_perpendicular = cross(first_direction, second_direction);
    let first_perpendicular = first.local_vector(second.world_vector(second_perpendicular));
    let side =
        (dot(sub(second_start, first_start), second_perpendicular) as f32).to_bits() as usize >> 31;
    let second_start_in_first = first.local_point(second.world_point(state.second_edge)?);
    let second_end_in_first =
        first.local_point(second.world_point(second.topology.next(state.second_edge)?)?);
    let mut faces = [
        AdjacentFace {
            face_side: state.first_side,
            face_edge: first.topology.opposite(state.first_edge)?,
            vertex_side: state.second_side,
            vertex_edge: state.second_edge,
            projected_vertex: second_start_in_first,
            perpendicular: first_perpendicular,
            normal: [0.0; 3],
        },
        AdjacentFace {
            face_side: state.first_side,
            face_edge: state.first_edge,
            vertex_side: state.second_side,
            vertex_edge: second.topology.opposite(state.second_edge)?,
            projected_vertex: second_end_in_first,
            perpendicular: first_perpendicular,
            normal: [0.0; 3],
        },
        AdjacentFace {
            face_side: state.second_side,
            face_edge: second.topology.opposite(state.second_edge)?,
            vertex_side: state.first_side,
            vertex_edge: state.first_edge,
            projected_vertex: first_start,
            perpendicular: second_perpendicular,
            normal: [0.0; 3],
        },
        AdjacentFace {
            face_side: state.second_side,
            face_edge: state.second_edge,
            vertex_side: state.first_side,
            vertex_edge: first.topology.opposite(state.first_edge)?,
            projected_vertex: first_end,
            perpendicular: second_perpendicular,
            normal: [0.0; 3],
        },
    ];
    let mut reversed = [0; 4];
    for (index, face) in faces.iter_mut().enumerate() {
        let shape = shapes[face.face_side];
        let start = shape.point(face.face_edge)?.map(f64::from);
        let along = sub(
            shape
                .point(shape.topology.next(face.face_edge)?)?
                .map(f64::from),
            start,
        );
        let across = sub(
            shape
                .point(shape.topology.previous(face.face_edge)?)?
                .map(f64::from),
            start,
        );
        face.normal = cross(along, across);
        let facing = (dot(face.perpendicular, face.normal) as f32).to_bits() as usize >> 31;
        reversed[index] = facing ^ (index >> 1) ^ side;
    }
    let mut selected = None;
    let mut minimum = -4.0e-12_f64;
    for (index, face) in faces.iter().enumerate() {
        let vertex = faces[(index ^ side) ^ reversed[index]];
        let first_candidate = faces[index ^ side];
        let second_candidate = faces[1 ^ (index ^ side)];
        let direction = sub(
            first_candidate.projected_vertex,
            second_candidate.projected_vertex,
        );
        let slope = dot(direction, face.normal);
        if slope >= 0.0 {
            continue;
        }
        let edge_length = dot(direction, direction) as f32;
        let face_length = dot(face.normal, face.normal) as f32;
        if edge_length == 0.0 || face_length == 0.0 {
            return Err(FeatureWalkError::UnsupportedFeaturePair);
        }
        let gradient = slope
            * f64::from(reciprocal_square_root_f64(f64::from(edge_length)) as f32)
            * f64::from(reciprocal_square_root_f64(f64::from(face_length)) as f32);
        if gradient >= minimum {
            continue;
        }
        let checks = face_checks(
            shapes[face.face_side],
            face.face_edge,
            vertex.projected_vertex,
        )?;
        if checks[0] > 0.0 {
            minimum = gradient;
            selected = Some((*face, vertex, checks));
        }
    }
    let Some((face, vertex, checks)) = selected else {
        if reversed[0] + reversed[1] == 2 || reversed[2] + reversed[3] == 2 {
            let regions = crate::segment_metric::SegmentPairGeometry::new(
                [first, second],
                [state.first_edge, state.second_edge],
            )?
            .regions()?;
            let (side, points, checks) = if reversed[0] + reversed[1] == 2 {
                (
                    state.first_side,
                    [second_start_in_first, second_end_in_first],
                    [regions[2], regions[3]],
                )
            } else {
                (
                    state.second_side,
                    [first_start, first_end],
                    [regions[0], regions[1]],
                )
            };
            let factor = f64::from(checks[0] / (checks[0] + checks[1]));
            let point = std::array::from_fn(|axis| {
                (1.0 - factor) * points[0][axis] + factor * points[1][axis]
            });
            return Ok(AdjacentTransition::Interior { side, point });
        }
        return Ok(AdjacentTransition::Finished);
    };
    if checks
        .iter()
        .all(|check| check.to_bits() & 0x8000_0000 == 0)
    {
        return Ok(AdjacentTransition::Next(FeatureState {
            kind: FeatureTransitionKind::FaceRegion,
            first_side: vertex.vertex_side,
            first_edge: vertex.vertex_edge,
            second_side: face.face_side,
            second_edge: face.face_edge,
        }));
    }
    let face_shape = shapes[face.face_side];
    let boundary = if checks[2] >= 0.0 {
        face_shape.topology.next(face.face_edge)?
    } else if checks[1] >= 0.0 {
        face_shape.topology.previous(face.face_edge)?
    } else {
        let next = face_shape.topology.next(face.face_edge)?;
        let previous = face_shape.topology.previous(face.face_edge)?;
        let placements = [shapes[vertex.vertex_side], face_shape];
        let next_distance =
            crate::SegmentPairMetric::evaluate(placements, [vertex.vertex_edge, next])?
                .squared_distance;
        let previous_distance = crate::SegmentPairMetric::evaluate(
            placements,
            [vertex.vertex_edge, face_shape.topology.opposite(previous)?],
        )?
        .squared_distance;
        if next_distance > previous_distance {
            previous
        } else {
            next
        }
    };
    Ok(AdjacentTransition::Next(FeatureState {
        kind: FeatureTransitionKind::EdgePair,
        first_side: vertex.vertex_side,
        first_edge: vertex.vertex_edge,
        second_side: face.face_side,
        second_edge: boundary,
    }))
}

fn finish(
    state: FeatureState,
    first_kind: SurfaceFeatureKind,
    second_kind: SurfaceFeatureKind,
) -> Result<FeatureSelection, FeatureWalkError> {
    let mut result = [None; 2];
    result[state.first_side] = Some(SurfaceFeature {
        edge: state.first_edge,
        kind: first_kind,
    });
    result[state.second_side] = Some(SurfaceFeature {
        edge: state.second_edge,
        kind: second_kind,
    });
    Ok(FeatureSelection {
        pair: SurfaceFeaturePair {
            first: result[0].ok_or(FeatureWalkError::UnsupportedFeaturePair)?,
            second: result[1].ok_or(FeatureWalkError::UnsupportedFeaturePair)?,
        },
        first_side: state.first_side,
    })
}

fn adjacent_vertices(
    topology: &FeatureTopology,
    edge: EdgeId,
) -> Result<Vec<EdgeId>, TopologyError> {
    let last = topology.previous(edge)?;
    let mut current = topology.previous(topology.opposite(last)?)?;
    let mut result = Vec::new();
    for _ in 0..topology.edges().len() {
        result.push(current);
        if current == last {
            return Ok(result);
        }
        current = topology.previous(topology.opposite(current)?)?;
    }
    Err(TopologyError::OpenFan)
}

fn segment_checks(
    shape: FeaturePlacement<'_>,
    edge: EdgeId,
    point: [f64; 3],
) -> Result<[f32; 2], FeatureWalkError> {
    shape.topology.segment_region_checks(edge, point)
}

fn face_checks(
    shape: FeaturePlacement<'_>,
    edge: EdgeId,
    point: [f64; 3],
) -> Result<[f32; 3], FeatureWalkError> {
    let [first, second, third, _] = shape.topology.triangle_region_checks(edge, point)?;
    Ok([first, second, third])
}

fn squared_segment_distance(
    shape: FeaturePlacement<'_>,
    edge: EdgeId,
    point: [f64; 3],
) -> Result<f64, FeatureWalkError> {
    shape.topology.point_segment_distance_squared(edge, point)
}

fn add(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] + second[axis])
}

fn sub(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] - second[axis])
}

fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    first[1] * second[1] + first[0] * second[0] + first[2] * second[2]
}

fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        FeaturePlacement, FeatureTransitionKind, FeatureWalkError, SurfaceFeature,
        SurfaceFeatureKind, SurfaceFeaturePair, walk_compact_features,
    };
    use crate::{AuthoredFace, FeatureTopology};

    fn topology() -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
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
        .unwrap()
    }

    #[test]
    fn native_cycle_history_starts_after_twenty_checks_and_respects_geometry_sharing() {
        let topology = topology();
        let a = topology.edge_id(0).unwrap();
        let b = topology.edge_id(1).unwrap();
        let seed = super::FeatureSelection::initial(SurfaceFeaturePair {
            first: SurfaceFeature {
                edge: a,
                kind: SurfaceFeatureKind::Vertex,
            },
            second: SurfaceFeature {
                edge: b,
                kind: SurfaceFeatureKind::Vertex,
            },
        });
        let state = super::FeatureState {
            kind: FeatureTransitionKind::VertexPair,
            first_side: 0,
            first_edge: a,
            second_side: 1,
            second_edge: b,
        };
        let make = |shared_geometry| super::WalkContext {
            selection: seed,
            geometry: None,
            extra: 0.0,
            fast_left: 20,
            cycles: Vec::new(),
            shared_geometry,
            transitions: 0,
        };
        for shared in [false, true] {
            let mut context = make(shared);
            for _ in 0..20 {
                assert!(context.admit(state));
                assert!(context.cycles.is_empty());
            }
            assert!(context.admit(state));
            assert_eq!(context.cycles.len(), 1);
            let reversed = super::FeatureState {
                first_edge: b,
                second_edge: a,
                ..state
            };
            assert_eq!(context.admit(reversed), !shared);
        }
        let mut context = make(false);
        context.fast_left = 0;
        context.cycles = (0..256)
            .map(|value| {
                [
                    (value, SurfaceFeatureKind::Vertex),
                    (value + 1000, SurfaceFeatureKind::Edge),
                ]
            })
            .collect();
        assert!(!context.admit(state));
        assert_eq!(context.cycles.len(), 256);
    }

    #[test]
    fn face_contact_retains_its_authored_directed_edge() {
        let moving = topology();
        let fixed = topology();
        let moving_placement = FeaturePlacement {
            topology: &moving,
            position: [0.25, -1.0, 0.25],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let fixed_placement = FeaturePlacement {
            topology: &fixed,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        for edge in 0..3 {
            let seed = SurfaceFeaturePair {
                first: SurfaceFeature {
                    edge: moving.edge_id(0).unwrap(),
                    kind: SurfaceFeatureKind::Vertex,
                },
                second: SurfaceFeature {
                    edge: fixed.edge_id(edge).unwrap(),
                    kind: SurfaceFeatureKind::Face,
                },
            };
            let mut transitions = Vec::new();
            let result =
                walk_compact_features(moving_placement, fixed_placement, seed, 16, |transition| {
                    transitions.push(transition.kind)
                })
                .unwrap();
            assert_eq!(result.pair.second, seed.second);
            assert_eq!(result.order(), [0, 1]);
            let reversed = walk_compact_features(
                fixed_placement,
                moving_placement,
                SurfaceFeaturePair {
                    first: seed.second,
                    second: seed.first,
                },
                16,
                |_| {},
            )
            .unwrap();
            assert_eq!(reversed.order(), [1, 0]);
            assert_eq!(reversed.ordered_pair(), result.ordered_pair());
            assert_eq!(
                transitions,
                [
                    FeatureTransitionKind::VertexFace,
                    FeatureTransitionKind::FaceRegion
                ]
            );
        }
    }

    #[test]
    fn feature_walk_rejects_invalid_limits_and_nonfinite_transforms() {
        let moving = topology();
        let fixed = topology();
        let first = FeaturePlacement {
            topology: &moving,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let second = FeaturePlacement {
            topology: &fixed,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let seed = SurfaceFeaturePair {
            first: SurfaceFeature {
                edge: moving.edge_id(0).unwrap(),
                kind: SurfaceFeatureKind::Vertex,
            },
            second: SurfaceFeature {
                edge: fixed.edge_id(0).unwrap(),
                kind: SurfaceFeatureKind::Face,
            },
        };
        assert_eq!(
            walk_compact_features(first, second, seed, 0, |_| {}).unwrap_err(),
            FeatureWalkError::InvalidTransitionLimit
        );
        assert_eq!(
            walk_compact_features(
                FeaturePlacement {
                    position: [f64::NAN, 0.0, 0.0],
                    ..first
                },
                second,
                seed,
                16,
                |_| {},
            )
            .unwrap_err(),
            FeatureWalkError::NonFiniteTransform
        );
    }
}
