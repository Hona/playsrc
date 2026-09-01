use playsrc_collision::{AuthoredHierarchy, AuthoredHullRef, PhysicsShape};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullQuery {
    pub center: [f64; 3],
    pub radius: f64,
    pub refine: Option<AuthoredHullRef>,
    pub maximum_visits: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HierarchyError {
    MissingHierarchy,
    InvalidRefinement,
    NonFinite,
    NegativeRadius,
    VisitLimit,
    InvalidHull,
    PairLimit,
    DuplicateHull,
}

impl fmt::Display for HierarchyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingHierarchy => "collision shape has no authored hull hierarchy",
            Self::InvalidRefinement => "hull refinement requires an authored recursive enclosure",
            Self::NonFinite => "hierarchy query contains a non-finite bound",
            Self::NegativeRadius => "hierarchy query radius must be nonnegative",
            Self::VisitLimit => "hierarchy query exceeds its node-visit bound",
            Self::InvalidHull => "selected hull is absent from its authored shape",
            Self::PairLimit => "hull-pair set exceeds its configured bound",
            Self::DuplicateHull => "hull-pair candidates contain duplicate identities",
        })
    }
}
impl std::error::Error for HierarchyError {}

/// Borrows the immutable Collision owner; no spatial tree or hull geometry is rebuilt.
#[derive(Clone, Copy, Debug)]
pub struct HullHierarchy<'shape> {
    hierarchy: &'shape AuthoredHierarchy,
}

impl<'shape> HullHierarchy<'shape> {
    pub fn from_collision(shape: &'shape PhysicsShape) -> Result<Self, HierarchyError> {
        Ok(Self {
            hierarchy: shape
                .authored_hierarchy()
                .ok_or(HierarchyError::MissingHierarchy)?,
        })
    }

    /// A root enclosure is returned even when the shape has several terminal pieces.
    pub fn single_hull(self) -> Option<AuthoredHullRef> {
        self.hierarchy.nodes[0].hull
    }

    pub fn terminal_hulls(
        self,
        maximum_visits: usize,
    ) -> Result<Vec<AuthoredHullRef>, HierarchyError> {
        let mut pending = vec![0];
        let mut output = Vec::new();
        let mut visits = 0;
        while let Some(index) = pending.pop() {
            if visits == maximum_visits {
                return Err(HierarchyError::VisitLimit);
            }
            visits += 1;
            let node = &self.hierarchy.nodes[index];
            if let Some([left, right]) = node.children {
                pending.push(right);
                pending.push(left);
            } else {
                output.push(
                    node.hull
                        .expect("validated terminal hierarchy node owns a hull"),
                );
            }
        }
        Ok(output)
    }

    pub fn query(self, input: HullQuery) -> Result<Vec<AuthoredHullRef>, HierarchyError> {
        if input.center.iter().any(|value| !value.is_finite()) || !input.radius.is_finite() {
            return Err(HierarchyError::NonFinite);
        }
        if input.radius < 0.0 {
            return Err(HierarchyError::NegativeRadius);
        }
        let mut pending = if let Some(reference) = input.refine {
            let AuthoredHullRef::Enclosure(index) = reference else {
                return Err(HierarchyError::InvalidRefinement);
            };
            let [left, right] = self
                .hierarchy
                .enclosures
                .get(index)
                .and_then(|hull| hull.subtree)
                .and_then(|root| self.hierarchy.nodes[root].children)
                .ok_or(HierarchyError::InvalidRefinement)?;
            vec![right, left]
        } else {
            vec![0]
        };
        let mut output = Vec::new();
        let mut visits = 0;
        while let Some(index) = pending.pop() {
            if visits == input.maximum_visits {
                return Err(HierarchyError::VisitLimit);
            }
            visits += 1;
            let node = &self.hierarchy.nodes[index];
            let delta: [f64; 3] =
                std::array::from_fn(|axis| f64::from(node.center()[axis]) - input.center[axis]);
            let squared = (delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2];
            let radius = f64::from(node.radius()) + input.radius;
            if squared > radius * radius {
                continue;
            }
            let step = (f64::from(node.radius()) * 0.004_f64) as f32;
            if delta
                .into_iter()
                .zip(node.box_sizes())
                .any(|(distance, size)| {
                    distance.abs() >= f64::from(f32::from(size) * step) + input.radius
                })
            {
                continue;
            }
            if let Some(hull) = node.hull {
                output.push(hull);
            } else if let Some([left, right]) = node.children {
                pending.push(right);
                pending.push(left);
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_collision::{AuthoredConvex, AuthoredEnclosure, AuthoredHierarchyNode};

    fn node(
        center: [f32; 3],
        radius: f32,
        hull: Option<AuthoredHullRef>,
        children: Option<[usize; 2]>,
    ) -> AuthoredHierarchyNode {
        let mut raw = [0; 28];
        for (axis, value) in center.into_iter().enumerate() {
            raw[8 + axis * 4..12 + axis * 4].copy_from_slice(&value.to_le_bytes());
        }
        raw[20..24].copy_from_slice(&radius.to_le_bytes());
        raw[24..27].copy_from_slice(&[250; 3]);
        AuthoredHierarchyNode {
            raw,
            hull,
            children,
        }
    }

    #[test]
    fn selection_stops_at_hulls_but_refinement_starts_at_their_children() {
        let tree = AuthoredHierarchy {
            nodes: vec![
                node(
                    [0.0; 3],
                    4.0,
                    Some(AuthoredHullRef::Enclosure(0)),
                    Some([2, 1]),
                ),
                node([1.0, 0.0, 0.0], 1.0, Some(AuthoredHullRef::Piece(0)), None),
                node([-1.0, 0.0, 0.0], 1.0, Some(AuthoredHullRef::Piece(1)), None),
            ],
            enclosures: vec![AuthoredEnclosure {
                subtree: Some(0),
                geometry: AuthoredConvex {
                    raw_header: [0; 16],
                    points: Vec::new(),
                    triangles: Vec::new(),
                },
            }],
        };
        let hierarchy = HullHierarchy { hierarchy: &tree };
        let input = HullQuery {
            center: [0.0; 3],
            radius: 10.0,
            refine: None,
            maximum_visits: 3,
        };
        assert_eq!(hierarchy.single_hull(), Some(AuthoredHullRef::Enclosure(0)));
        assert_eq!(
            hierarchy.query(input).unwrap(),
            [AuthoredHullRef::Enclosure(0)]
        );
        assert_eq!(
            hierarchy.terminal_hulls(3).unwrap(),
            [AuthoredHullRef::Piece(1), AuthoredHullRef::Piece(0)]
        );
        assert_eq!(
            hierarchy
                .query(HullQuery {
                    refine: hierarchy.single_hull(),
                    ..input
                })
                .unwrap(),
            hierarchy.terminal_hulls(3).unwrap()
        );
        assert_eq!(
            hierarchy.query(HullQuery {
                refine: Some(AuthoredHullRef::Piece(0)),
                ..input
            }),
            Err(HierarchyError::InvalidRefinement)
        );
        assert_eq!(
            hierarchy.query(HullQuery {
                refine: hierarchy.single_hull(),
                maximum_visits: 1,
                ..input
            }),
            Err(HierarchyError::VisitLimit)
        );
    }

    #[test]
    fn box_equality_rejects_while_sphere_equality_remains_admitted() {
        let tree = AuthoredHierarchy {
            nodes: vec![node([0.0; 3], 1.0, Some(AuthoredHullRef::Piece(0)), None)],
            enclosures: Vec::new(),
        };
        let hierarchy = HullHierarchy { hierarchy: &tree };
        let input = HullQuery {
            center: [1.0, 0.0, 0.0],
            radius: 0.0,
            refine: None,
            maximum_visits: 1,
        };
        assert!(hierarchy.query(input).unwrap().is_empty());
        assert_eq!(
            hierarchy
                .query(HullQuery {
                    center: [f64::from_bits(1.0_f64.to_bits() - 1), 0.0, 0.0],
                    ..input
                })
                .unwrap(),
            [AuthoredHullRef::Piece(0)]
        );
        assert_eq!(
            hierarchy.query(HullQuery {
                radius: -1.0,
                ..input
            }),
            Err(HierarchyError::NegativeRadius)
        );
        assert_eq!(
            hierarchy.query(HullQuery {
                radius: f64::NAN,
                ..input
            }),
            Err(HierarchyError::NonFinite)
        );
        assert_eq!(hierarchy.terminal_hulls(0), Err(HierarchyError::VisitLimit));
    }
}
