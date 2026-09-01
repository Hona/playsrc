use std::{collections::BTreeMap, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectPairState {
    pub identity: u64,
    pub friction_core: u64,
    pub moving: bool,
    pub immovable: bool,
    pub pinned: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPairLinks {
    pub identity: u64,
    pub pairs: Vec<[u64; 2]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPairGraph {
    links: BTreeMap<u64, Vec<[u64; 2]>>,
    pairs: BTreeMap<[u64; 2], [u64; 2]>,
    maximum: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectPairChanges {
    pub retired: Vec<[u64; 2]>,
    pub created: Vec<[u64; 2]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectPairError {
    InvalidLimit,
    InvalidIdentity,
    InvalidLinks,
    MissingBody,
    DuplicateCandidate,
    Capacity,
}
impl fmt::Display for ObjectPairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimit => "object-pair capacity must be positive",
            Self::InvalidIdentity => "object-pair identities must be unique and nonzero",
            Self::InvalidLinks => "object-pair links do not agree at both endpoints",
            Self::MissingBody => "object-pair body state is absent",
            Self::DuplicateCandidate => "spatial candidates contain duplicate objects",
            Self::Capacity => "object-pair graph exceeds its configured capacity",
        })
    }
}
impl std::error::Error for ObjectPairError {}

fn key(pair: [u64; 2]) -> [u64; 2] {
    if pair[0] < pair[1] {
        pair
    } else {
        [pair[1], pair[0]]
    }
}

impl ObjectPairGraph {
    pub fn register(&mut self, identity: u64) -> Result<(), ObjectPairError> {
        if identity == 0 || self.links.contains_key(&identity) {
            return Err(ObjectPairError::InvalidIdentity);
        }
        self.links.insert(identity, Vec::new());
        Ok(())
    }
    pub fn remove(&mut self, identity: u64) -> Result<ObjectPairChanges, ObjectPairError> {
        let changes = self.disconnect(identity)?;
        self.links.remove(&identity);
        Ok(changes)
    }
    pub fn from_links(
        input: Vec<ObjectPairLinks>,
        maximum: usize,
    ) -> Result<Self, ObjectPairError> {
        if maximum == 0 {
            return Err(ObjectPairError::InvalidLimit);
        }
        let mut links = BTreeMap::new();
        let mut pairs = BTreeMap::new();
        for body in input {
            if body.identity == 0 || links.insert(body.identity, body.pairs).is_some() {
                return Err(ObjectPairError::InvalidIdentity);
            }
        }
        for (identity, body) in &links {
            let mut found = BTreeMap::new();
            for pair in body {
                if pair[0] == pair[1]
                    || !pair.contains(identity)
                    || pair.iter().any(|id| !links.contains_key(id))
                    || found.insert(key(*pair), ()).is_some()
                {
                    return Err(ObjectPairError::InvalidLinks);
                }
                if pairs
                    .insert(key(*pair), *pair)
                    .is_some_and(|old| old != *pair)
                {
                    return Err(ObjectPairError::InvalidLinks);
                }
            }
        }
        for pair in pairs.values() {
            if pair
                .iter()
                .any(|id| links[id].iter().filter(|value| **value == *pair).count() != 1)
            {
                return Err(ObjectPairError::InvalidLinks);
            }
        }
        if pairs.len() > maximum {
            return Err(ObjectPairError::Capacity);
        }
        Ok(Self {
            links,
            pairs,
            maximum,
        })
    }
    pub fn links(&self, identity: u64) -> Option<&[[u64; 2]]> {
        self.links.get(&identity).map(Vec::as_slice)
    }
    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }

    pub fn disconnect(&mut self, identity: u64) -> Result<ObjectPairChanges, ObjectPairError> {
        let links = self
            .links
            .get(&identity)
            .ok_or(ObjectPairError::MissingBody)?;
        let retired = links.iter().rev().copied().collect::<Vec<_>>();
        for pair in &retired {
            self.pairs.remove(&key(*pair));
            let peer = if pair[0] == identity {
                pair[1]
            } else {
                pair[0]
            };
            let links = self.links.get_mut(&peer).expect("validated pair endpoint");
            let index = links
                .iter()
                .position(|value| value == pair)
                .expect("validated backlink");
            links.swap_remove(index);
        }
        self.links
            .get_mut(&identity)
            .expect("validated owner")
            .clear();
        Ok(ObjectPairChanges {
            retired,
            created: Vec::new(),
        })
    }

    pub fn reconcile(
        &mut self,
        identity: u64,
        states: &[ObjectPairState],
        candidates: &[u64],
        mut should_collide: impl FnMut(u64, u64) -> bool,
    ) -> Result<ObjectPairChanges, ObjectPairError> {
        let state_count = states.len();
        let states = states
            .iter()
            .map(|state| (state.identity, *state))
            .collect::<BTreeMap<_, _>>();
        if states.len() != state_count
            || states
                .values()
                .any(|state| state.identity == 0 || state.friction_core == 0)
        {
            return Err(ObjectPairError::InvalidIdentity);
        }
        let body = *states.get(&identity).ok_or(ObjectPairError::MissingBody)?;
        let old = self
            .links
            .get(&identity)
            .ok_or(ObjectPairError::MissingBody)?;
        for candidate in candidates {
            if !states.contains_key(candidate) || !self.links.contains_key(candidate) {
                return Err(ObjectPairError::MissingBody);
            }
        }
        let unique = candidates
            .iter()
            .map(|id| (*id, ()))
            .collect::<BTreeMap<_, _>>();
        if unique.len() != candidates.len() {
            return Err(ObjectPairError::DuplicateCandidate);
        }
        if !body.enabled {
            return Ok(ObjectPairChanges::default());
        }
        let mut ordered = old.clone();
        let mut retained = 0;
        let mut new = Vec::new();
        for other in candidates.iter().rev() {
            let other = states[other];
            if (!body.moving && !other.moving)
                || body.friction_core == other.friction_core
                || (body.immovable || body.pinned) && (other.immovable || other.pinned)
            {
                continue;
            }
            if !should_collide(identity, other.identity) {
                continue;
            }
            if let Some(index) = ordered
                .iter()
                .position(|pair| key(*pair) == key([identity, other.identity]))
            {
                if index > retained {
                    ordered.swap(index, retained);
                }
                retained += 1;
            } else {
                new.push([identity, other.identity]);
            }
        }
        let retired = ordered[retained..]
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        let created = new.into_iter().rev().collect::<Vec<_>>();
        if self.pairs.len() - retired.len() + created.len() > self.maximum {
            return Err(ObjectPairError::Capacity);
        }
        ordered.truncate(retained);
        for pair in &retired {
            self.pairs.remove(&key(*pair));
            let other = if pair[0] == identity {
                pair[1]
            } else {
                pair[0]
            };
            let peers = self.links.get_mut(&other).expect("validated peer links");
            let index = peers
                .iter()
                .position(|item| *item == *pair)
                .expect("validated pair backlink");
            peers.swap_remove(index);
        }
        for pair in &created {
            self.pairs.insert(key(*pair), *pair);
            self.links
                .get_mut(&pair[1])
                .expect("validated candidate")
                .push(*pair);
            ordered.push(*pair);
        }
        self.links.insert(identity, ordered);
        Ok(ObjectPairChanges { retired, created })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(identity: u64) -> ObjectPairState {
        ObjectPairState {
            identity,
            friction_core: identity,
            moving: true,
            immovable: false,
            pinned: false,
            enabled: true,
        }
    }
    #[test]
    fn retiring_peer_links_moves_the_last_link_into_the_vacated_slot() {
        let mut graph = ObjectPairGraph::from_links(
            vec![
                ObjectPairLinks {
                    identity: 1,
                    pairs: vec![[2, 1], [3, 1], [4, 1]],
                },
                ObjectPairLinks {
                    identity: 2,
                    pairs: vec![[2, 1]],
                },
                ObjectPairLinks {
                    identity: 3,
                    pairs: vec![[3, 1]],
                },
                ObjectPairLinks {
                    identity: 4,
                    pairs: vec![[4, 1]],
                },
            ],
            8,
        )
        .unwrap();
        assert_eq!(graph.disconnect(2).unwrap().retired, [[2, 1]]);
        assert_eq!(graph.links(1).unwrap(), [[4, 1], [3, 1]]);
        let states = (1..=4).map(state).collect::<Vec<_>>();
        let before = graph.clone();
        assert!(graph.reconcile(1, &states, &[2, 2], |_, _| true).is_err());
        assert_eq!(graph, before);
        assert!(
            graph
                .reconcile(1, &[state(1), state(1)], &[], |_, _| true)
                .is_err()
        );
        assert_eq!(graph, before);
        assert_eq!(graph.disconnect(1).unwrap().retired, [[3, 1], [4, 1]]);
        assert_eq!(graph.pair_count(), 0);
    }
    #[test]
    fn new_pairs_are_bounded_before_either_endpoint_changes() {
        let mut graph = ObjectPairGraph::from_links(
            (1..=3)
                .map(|identity| ObjectPairLinks {
                    identity,
                    pairs: vec![],
                })
                .collect(),
            1,
        )
        .unwrap();
        let before = graph.clone();
        let states = (1..=3).map(state).collect::<Vec<_>>();
        assert_eq!(
            graph.reconcile(1, &states, &[1, 2, 3], |_, _| true),
            Err(ObjectPairError::Capacity)
        );
        assert_eq!(graph, before);
        assert_eq!(
            graph
                .reconcile(1, &states, &[1, 2], |_, _| true)
                .unwrap()
                .created,
            [[1, 2]]
        );
    }
}
