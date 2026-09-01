use crate::{HierarchyError, HullHierarchy, HullQuery, ProjectionKnot};
use playsrc_collision::{AuthoredHullRef, PhysicsShape};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairCoreProjection {
    pub position: [f64; 3],
    pub velocity: [f32; 3],
    pub time: f64,
    pub radius: f32,
}

impl PairCoreProjection {
    pub fn at(self, time: f64) -> Result<[f64; 3], HierarchyError> {
        if !time.is_finite()
            || !self.time.is_finite()
            || !self.radius.is_finite()
            || self.position.iter().any(|value| !value.is_finite())
            || self.velocity.iter().any(|value| !value.is_finite())
        {
            return Err(HierarchyError::NonFinite);
        }
        if self.radius < 0.0 {
            return Err(HierarchyError::NegativeRadius);
        }
        let elapsed = f64::from((time - self.time) as f32);
        let position = std::array::from_fn(|axis| {
            self.position[axis] + f64::from(self.velocity[axis]) * elapsed
        });
        if position.iter().any(|value| !value.is_finite()) {
            return Err(HierarchyError::NonFinite);
        }
        Ok(position)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HullSearch {
    Selected(AuthoredHullRef),
    Spatial {
        pose: ProjectionKnot,
        refine: Option<AuthoredHullRef>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct HullPairEndpoint<'shape> {
    pub shape: &'shape PhysicsShape,
    pub core: PairCoreProjection,
    pub extra_radius: f32,
    pub search: HullSearch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HullCandidates {
    pub endpoints: [Vec<AuthoredHullRef>; 2],
}

pub fn query_hull_pairs(
    endpoints: [HullPairEndpoint<'_>; 2],
    time: f64,
    scan_range: f64,
    maximum_visits: usize,
) -> Result<HullCandidates, HierarchyError> {
    if !time.is_finite() || !scan_range.is_finite() {
        return Err(HierarchyError::NonFinite);
    }
    if scan_range < 0.0 {
        return Err(HierarchyError::NegativeRadius);
    }
    let mut output = [Vec::new(), Vec::new()];
    for side in 0..2 {
        let endpoint = endpoints[side];
        if !endpoint.extra_radius.is_finite() {
            return Err(HierarchyError::NonFinite);
        }
        if endpoint.extra_radius < 0.0 {
            return Err(HierarchyError::NegativeRadius);
        }
        output[side] = match endpoint.search {
            HullSearch::Selected(hull) => {
                if endpoint.shape.authored_hull(hull).is_none() {
                    return Err(HierarchyError::InvalidHull);
                }
                vec![hull]
            }
            HullSearch::Spatial { pose, refine } => {
                if pose
                    .position
                    .iter()
                    .chain(pose.orientation.iter())
                    .any(|value| !value.is_finite())
                {
                    return Err(HierarchyError::NonFinite);
                }
                let opposing = endpoints[1 - side].core;
                let position = opposing.at(time)?;
                let delta: [f64; 3] =
                    std::array::from_fn(|axis| position[axis] - pose.position[axis]);
                let center = std::array::from_fn(|axis| {
                    (delta[1] * pose.orientation[3 + axis] + delta[0] * pose.orientation[axis])
                        + delta[2] * pose.orientation[6 + axis]
                });
                let radius = f64::from(endpoint.extra_radius + opposing.radius) + scan_range;
                HullHierarchy::from_collision(endpoint.shape)?.query(HullQuery {
                    center,
                    radius,
                    refine,
                    maximum_visits,
                })?
            }
        };
    }
    Ok(HullCandidates { endpoints: output })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AuthoredHullPair(pub [AuthoredHullRef; 2]);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HullPairSet {
    pairs: Vec<AuthoredHullPair>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HullPairChanges {
    /// Creation order; newly created entries join the set in reverse order.
    pub created: Vec<AuthoredHullPair>,
    /// Retirement order, after creating all new entries.
    pub retired: Vec<AuthoredHullPair>,
}

impl HullPairSet {
    pub(crate) fn remove(&mut self, pair: AuthoredHullPair) -> bool {
        let Some(index) = self.pairs.iter().position(|value| *value == pair) else {
            return false;
        };
        self.pairs.swap_remove(index);
        true
    }
    pub fn from_pairs(
        pairs: Vec<AuthoredHullPair>,
        maximum_pairs: usize,
    ) -> Result<Self, HierarchyError> {
        if pairs.len() > maximum_pairs {
            return Err(HierarchyError::PairLimit);
        }
        let index = pairs
            .iter()
            .enumerate()
            .map(|(index, pair)| (*pair, index))
            .collect::<BTreeMap<_, _>>();
        if index.len() != pairs.len() {
            return Err(HierarchyError::DuplicateHull);
        }
        Ok(Self { pairs })
    }

    pub fn pairs(&self) -> &[AuthoredHullPair] {
        &self.pairs
    }

    pub fn reconcile(
        &mut self,
        candidates: &HullCandidates,
        maximum_pairs: usize,
    ) -> Result<HullPairChanges, HierarchyError> {
        if candidates.endpoints[0]
            .len()
            .checked_mul(candidates.endpoints[1].len())
            .is_none_or(|count| count > maximum_pairs)
            || self.pairs.len() > maximum_pairs
        {
            return Err(HierarchyError::PairLimit);
        }
        for hulls in &candidates.endpoints {
            let mut unique = hulls.clone();
            unique.sort_unstable();
            unique.dedup();
            if unique.len() != hulls.len() {
                return Err(HierarchyError::DuplicateHull);
            }
        }
        let mut pairs = self.pairs.clone();
        let mut positions = pairs
            .iter()
            .enumerate()
            .map(|(index, pair)| (*pair, index))
            .collect::<BTreeMap<_, _>>();
        let mut retained = 0;
        let mut created = Vec::new();
        for first in candidates.endpoints[0].iter().rev() {
            for second in candidates.endpoints[1].iter().rev() {
                let pair = AuthoredHullPair([*first, *second]);
                if let Some(index) = positions.get(&pair).copied() {
                    if index > retained {
                        positions.insert(pairs[retained], index);
                        positions.insert(pair, retained);
                        pairs.swap(retained, index);
                    }
                    retained += 1;
                } else {
                    created.push(pair);
                }
            }
        }
        let retired = pairs[retained..].iter().rev().copied().collect();
        pairs.truncate(retained);
        pairs.extend(created.iter().rev().copied());
        self.pairs = pairs;
        Ok(HullPairChanges { created, retired })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_keeps_survivors_then_appends_new_pairs_in_reverse_creation_order() {
        let p = AuthoredHullRef::Piece;
        let pair = |a, b| AuthoredHullPair([p(a), p(b)]);
        let mut set = HullPairSet::from_pairs(vec![pair(0, 0), pair(1, 1), pair(2, 2)], 9).unwrap();
        let input = HullCandidates {
            endpoints: [vec![p(0), p(1)], vec![p(0), p(1)]],
        };
        let changes = set.reconcile(&input, 9).unwrap();
        assert_eq!(changes.created, [pair(1, 0), pair(0, 1)]);
        assert_eq!(changes.retired, [pair(2, 2)]);
        assert_eq!(
            set.pairs(),
            [pair(1, 1), pair(0, 0), pair(0, 1), pair(1, 0)]
        );
        let before = set.clone();
        assert_eq!(
            set.reconcile(
                &HullCandidates {
                    endpoints: [vec![p(0), p(0)], vec![p(0)]]
                },
                9
            ),
            Err(HierarchyError::DuplicateHull)
        );
        assert_eq!(set, before);
        assert_eq!(set.reconcile(&input, 3), Err(HierarchyError::PairLimit));
        assert_eq!(set, before);
        let changes = set
            .reconcile(
                &HullCandidates {
                    endpoints: [Vec::new(), Vec::new()],
                },
                9,
            )
            .unwrap();
        assert_eq!(
            changes.retired,
            before.pairs().iter().rev().copied().collect::<Vec<_>>()
        );
        assert!(set.pairs().is_empty());
    }

    #[test]
    fn query_core_projection_rounds_elapsed_time_before_multiplying_velocity() {
        let core = PairCoreProjection {
            position: [1.0, 2.0, 3.0],
            velocity: [2.0, -3.0, 4.0],
            time: 0.1,
            radius: 1.0,
        };
        let elapsed = f64::from((0.15_f64 - 0.1) as f32);
        assert_eq!(
            core.at(0.15).unwrap(),
            [
                1.0 + 2.0 * elapsed,
                2.0 - 3.0 * elapsed,
                3.0 + 4.0 * elapsed
            ]
        );
        assert_ne!(
            core.at(0.15).unwrap()[0].to_bits(),
            (1.0 + 2.0 * (0.15_f64 - 0.1)).to_bits()
        );
    }
}
