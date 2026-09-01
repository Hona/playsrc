use crate::{FeatureTopology, FeatureWalkError, SurfaceFeature, SurfaceFeatureKind};
use playsrc_collision::{AuthoredHullRef, PhysicsShape};

const MAXIMUM_SPAWNED_PAIRS: u32 = 1000;
const NO_SUBTREE_RADIUS: f32 = f32::from_bits(0x5863_5fa9);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecursiveHullEvent {
    Collision,
    InvalidOverlap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecursiveHullDecision {
    Contact,
    Refine(usize),
    RetainInvalid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecursiveHullFeature {
    kind: SurfaceFeatureKind,
    virtual_edge: bool,
    virtual_face: bool,
    subtree_radius: Option<f32>,
}

impl RecursiveHullFeature {
    pub fn from_collision(
        shape: &PhysicsShape,
        hull: AuthoredHullRef,
        topology: &FeatureTopology,
        feature: SurfaceFeature,
    ) -> Result<Self, FeatureWalkError> {
        let authored = shape
            .authored_hull(hull)
            .ok_or(FeatureWalkError::UnsupportedFeaturePair)?;
        let edge = topology.edge(feature.edge)?;
        let metadata = topology.face_metadata(feature.edge)?;
        let triangle = authored
            .triangles
            .get(edge.face)
            .ok_or(FeatureWalkError::UnsupportedFeaturePair)?;
        if triangle.metadata() != metadata || triangle.edge_words()[edge.position] != edge.word {
            return Err(FeatureWalkError::UnsupportedFeaturePair);
        }
        let subtree_radius = match hull {
            AuthoredHullRef::Piece(_) => None,
            AuthoredHullRef::Enclosure(index) => {
                let hierarchy = shape
                    .authored_hierarchy()
                    .ok_or(FeatureWalkError::UnsupportedFeaturePair)?;
                hierarchy.enclosures[index]
                    .subtree
                    .map(|node| hierarchy.nodes[node].radius())
            }
        };
        Ok(Self {
            kind: feature.kind,
            virtual_edge: edge.virtual_edge,
            virtual_face: metadata & 0x8000_0000 != 0,
            subtree_radius,
        })
    }
}

pub fn decide_recursive_hulls(
    event: RecursiveHullEvent,
    features: [RecursiveHullFeature; 2],
    spawned_pairs: u32,
) -> Result<RecursiveHullDecision, FeatureWalkError> {
    if spawned_pairs > MAXIMUM_SPAWNED_PAIRS {
        return Ok(match event {
            RecursiveHullEvent::Collision => RecursiveHullDecision::Contact,
            RecursiveHullEvent::InvalidOverlap => RecursiveHullDecision::RetainInvalid,
        });
    }
    let larger_subtree = || {
        let first = features[0].subtree_radius.unwrap_or(NO_SUBTREE_RADIUS);
        let second = features[1].subtree_radius.unwrap_or(NO_SUBTREE_RADIUS);
        usize::from(first <= second)
    };
    let selected = if event == RecursiveHullEvent::InvalidOverlap {
        Some(if features[0].subtree_radius.is_none() {
            1
        } else if features[1].subtree_radius.is_none() {
            0
        } else {
            larger_subtree()
        })
    } else {
        use SurfaceFeatureKind::{Edge, Face, Vertex};
        match [features[0].kind, features[1].kind] {
            [Vertex, Vertex] => None,
            [Vertex, Edge] => features[1].virtual_edge.then_some(1),
            [Vertex, Face] => features[1].virtual_face.then_some(1),
            [Edge, Vertex] | [Face, _] => features[0].virtual_face.then_some(0),
            [Edge, Edge] => match [features[0].virtual_edge, features[1].virtual_edge] {
                [false, false] => None,
                [true, false] => Some(0),
                [false, true] => Some(1),
                [true, true] => Some(larger_subtree()),
            },
            _ => return Err(FeatureWalkError::UnsupportedFeaturePair),
        }
    };
    Ok(selected.map_or(
        RecursiveHullDecision::Contact,
        RecursiveHullDecision::Refine,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(
        kind: SurfaceFeatureKind,
        virtual_edge: bool,
        virtual_face: bool,
        radius: Option<f32>,
    ) -> RecursiveHullFeature {
        RecursiveHullFeature {
            kind,
            virtual_edge,
            virtual_face,
            subtree_radius: radius,
        }
    }

    #[test]
    fn face_and_edge_virtual_flags_are_not_interchangeable() {
        use SurfaceFeatureKind::{Edge, Face, Vertex};
        let vertex = feature(Vertex, false, false, None);
        let edge = feature(Edge, false, true, Some(2.0));
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [edge, vertex], 0).unwrap(),
            RecursiveHullDecision::Refine(0)
        );
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [vertex, edge], 0).unwrap(),
            RecursiveHullDecision::Contact
        );
        let face = feature(Face, true, false, Some(2.0));
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [vertex, face], 0).unwrap(),
            RecursiveHullDecision::Contact
        );
    }

    #[test]
    fn recursive_radius_ties_choose_second_and_limit_is_strict() {
        let edge = feature(SurfaceFeatureKind::Edge, true, true, Some(2.0));
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [edge; 2], 1000).unwrap(),
            RecursiveHullDecision::Refine(1)
        );
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [edge; 2], 1001).unwrap(),
            RecursiveHullDecision::Contact
        );
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::InvalidOverlap, [edge; 2], 1001).unwrap(),
            RecursiveHullDecision::RetainInvalid
        );
        let smaller = feature(SurfaceFeatureKind::Edge, true, true, Some(1.0));
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [edge, smaller], 0).unwrap(),
            RecursiveHullDecision::Refine(0)
        );
        let absent = feature(SurfaceFeatureKind::Edge, true, true, None);
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::Collision, [absent, edge], 0).unwrap(),
            RecursiveHullDecision::Refine(0)
        );
        assert_eq!(
            decide_recursive_hulls(RecursiveHullEvent::InvalidOverlap, [absent, edge], 0).unwrap(),
            RecursiveHullDecision::Refine(1)
        );
    }
}
