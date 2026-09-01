use super::*;

impl PairSpace {
    pub(super) fn roster(
        &self,
        owner: PairOwner,
    ) -> Option<(&HullPairSet, &BTreeMap<AuthoredHullPair, u64>)> {
        match owner {
            PairOwner::Object(id) => {
                let parent = self.parents.get(&id)?;
                Some((&parent.hulls, &parent.children))
            }
            PairOwner::Recursive(id) => {
                let parent = self.children.get(&id)?.recursive.as_ref()?;
                Some((&parent.hulls, &parent.children))
            }
        }
    }
    pub(super) fn roster_mut(
        &mut self,
        owner: PairOwner,
    ) -> Option<(&mut HullPairSet, &mut BTreeMap<AuthoredHullPair, u64>)> {
        match owner {
            PairOwner::Object(id) => {
                let parent = self.parents.get_mut(&id)?;
                Some((&mut parent.hulls, &mut parent.children))
            }
            PairOwner::Recursive(id) => {
                let parent = self.children.get_mut(&id)?.recursive.as_mut()?;
                Some((&mut parent.hulls, &mut parent.children))
            }
        }
    }
    pub(super) fn valid_child_owner(&self, id: u64, child: &ChildPair) -> bool {
        if let PairOwner::Recursive(parent) = child.owner
            && (parent >= id
                || !self
                    .children
                    .get(&parent)
                    .is_some_and(|p| p.bodies == child.bodies))
        {
            return false;
        }
        if !self.roster(child.owner).is_some_and(|(hulls, ids)| {
            hulls.pairs().contains(&child.hulls) && ids.get(&child.hulls) == Some(&id)
        }) {
            return false;
        }
        if child.recursive.is_some()
            != child
                .hulls
                .0
                .iter()
                .any(|hull| matches!(hull, AuthoredHullRef::Enclosure(_)))
        {
            return false;
        }
        if let Some(recursive) = &child.recursive {
            if recursive.children.len() != recursive.hulls.pairs().len()
                || recursive.refined.is_some_and(|side| {
                    side > 1 || !matches!(child.hulls.0[side], AuthoredHullRef::Enclosure(_))
                })
            {
                return false;
            }
            if recursive.refined.is_some() != matches!(child.residence, Residence::Refining { .. })
                || (recursive.refined.is_none() && !recursive.children.is_empty())
            {
                return false;
            }
            let mut count = recursive.retired_descendants as usize;
            if count > self.maximum {
                return false;
            }
            for (hulls, nested) in &recursive.children {
                let Some(pair) = self.children.get(nested) else {
                    return false;
                };
                if *nested <= id
                    || pair.owner != PairOwner::Recursive(id)
                    || pair.hulls != *hulls
                    || !recursive.hulls.pairs().contains(hulls)
                {
                    return false;
                }
                let Some(next) = count.checked_add(1).and_then(|value| {
                    value.checked_add(
                        pair.recursive
                            .as_ref()
                            .map_or(0, |state| state.descendants as usize),
                    )
                }) else {
                    return false;
                };
                count = next;
                if count > self.maximum {
                    return false;
                }
            }
            if count != recursive.descendants as usize {
                return false;
            }
        }
        true
    }
}

impl PhysicsEnvironment {
    pub(super) fn adjust_recursive_count(
        &mut self,
        mut id: u64,
        change: i32,
    ) -> Result<(), EnvironmentError> {
        loop {
            let pair = self
                .pairs
                .children
                .get_mut(&id)
                .ok_or(PairError::MissingPair)?;
            let recursive = pair
                .recursive
                .as_mut()
                .ok_or(PairError::MissingRecursiveOwner)?;
            recursive.descendants = recursive
                .descendants
                .checked_add_signed(change)
                .ok_or(PairError::Capacity)?;
            if let PairOwner::Recursive(parent) = pair.owner {
                id = parent;
            } else {
                break;
            }
        }
        Ok(())
    }
    pub(super) fn recursive_count(&self, mut id: u64) -> Result<u32, EnvironmentError> {
        let mut count = 0;
        loop {
            let pair = self.pairs.children.get(&id).ok_or(PairError::MissingPair)?;
            let own = pair
                .recursive
                .as_ref()
                .ok_or(PairError::MissingRecursiveOwner)?
                .descendants;
            if own > 0 {
                count = own;
            }
            if let PairOwner::Recursive(parent) = pair.owner {
                if parent >= id {
                    return Err(EnvironmentError::SnapshotMismatch);
                }
                id = parent;
            } else {
                break;
            }
        }
        Ok(count)
    }
    pub(super) fn clear_recursive_children(&mut self, id: u64) -> Result<(), EnvironmentError> {
        let Some(state) = self
            .pairs
            .children
            .get_mut(&id)
            .ok_or(PairError::MissingPair)?
            .recursive
            .as_mut()
        else {
            return Ok(());
        };
        let hulls = std::mem::take(&mut state.hulls);
        let children = std::mem::take(&mut state.children);
        for hull in hulls.pairs().iter().rev() {
            self.remove_child(children[hull])?;
        }
        self.adjust_recursive_count(id, -(hulls.pairs().len() as i32))?;
        Ok(())
    }
    pub(super) fn refine_recursive_pair(
        &mut self,
        id: u64,
        event: crate::RecursiveHullEvent,
    ) -> Result<bool, EnvironmentError> {
        let pair = &self.pairs.children[&id];
        let indices = self.pair_bodies(pair.bodies)?;
        let features = pair.closest.selection().pair;
        let features = [features.first, features.second];
        let mut authored = Vec::with_capacity(2);
        for side in 0..2 {
            let body = &self.bodies[indices[side]];
            authored.push(crate::RecursiveHullFeature::from_collision(
                &body.shape,
                pair.hulls.0[side],
                body.topology(body.hull_index(pair.hulls.0[side])?)
                    .ok_or(PairError::MissingPair)?,
                features[side],
            )?);
        }
        match crate::decide_recursive_hulls(
            event,
            [authored[0], authored[1]],
            self.recursive_count(id)?,
        )? {
            crate::RecursiveHullDecision::Contact => Ok(false),
            crate::RecursiveHullDecision::RetainInvalid => {
                self.set_invalid(id)?;
                Ok(true)
            }
            crate::RecursiveHullDecision::Refine(side) => {
                self.statistics.recursive_refinements = self
                    .statistics
                    .recursive_refinements
                    .checked_add(1)
                    .ok_or(EnvironmentError::ClockOverflow)?;
                let cores = pair.bodies;
                let distances = self.search_ranges.pair(
                    indices.map(|i| self.retained_motion(i)),
                    indices.map(|i| self.bodies[i].physical.radius),
                )?;
                let total = (distances[0] + distances[1]) as f32;
                let moving = cores.map(|core| {
                    self.islands
                        .movement(core)
                        .is_some_and(crate::CoreMovement::can_collide)
                });
                let allowances = if !moving[0] {
                    [1.0e-10_f32, total]
                } else if !moving[1] {
                    [total, 1.0e-10_f32]
                } else {
                    crate::pair_residence::split_moving_distance(
                        total,
                        indices.map(|i| self.retained_motion(i)),
                    )?
                };
                self.clear_residence(id)?;
                let listeners = [
                    self.range_insert(cores[0], RangeOwner::Child(id), f64::from(allowances[0]))?,
                    self.range_insert(cores[1], RangeOwner::Child(id), f64::from(allowances[1]))?,
                ];
                let pair = self.pairs.children.get_mut(&id).unwrap();
                pair.residence = Residence::Refining { listeners };
                pair.recursive
                    .as_mut()
                    .ok_or(PairError::MissingRecursiveOwner)?
                    .refined = Some(side);
                self.scan_recursive_pair(id, distances[0] + distances[1])?;
                Ok(true)
            }
        }
    }
    fn scan_recursive_pair(&mut self, id: u64, distance: f64) -> Result<(), EnvironmentError> {
        if self.recursive_count(id)? > 1000 {
            return Ok(());
        }
        self.statistics.recursive_scans = self
            .statistics
            .recursive_scans
            .checked_add(1)
            .ok_or(EnvironmentError::ClockOverflow)?;
        let pair = self.pairs.children[&id].clone();
        let selected = pair
            .recursive
            .as_ref()
            .and_then(|state| state.refined)
            .ok_or(PairError::MissingRecursiveOwner)?;
        let indices = self.pair_bodies(pair.bodies)?;
        let pose = self
            .cached_transform(self.bodies[indices[selected]].identity)?
            .object;
        let candidates = crate::query_hull_pairs(
            std::array::from_fn(|side| HullPairEndpoint {
                shape: &self.bodies[indices[side]].shape,
                core: self.core_projection(indices[side]),
                extra_radius: 0.0,
                search: if side == selected {
                    HullSearch::Spatial {
                        pose,
                        refine: Some(pair.hulls.0[side]),
                    }
                } else {
                    HullSearch::Selected(pair.hulls.0[side])
                },
            }),
            self.time(),
            distance,
            self.bodies[indices[selected]]
                .shape
                .authored_hierarchy()
                .ok_or(crate::HierarchyError::MissingHierarchy)?
                .nodes
                .len(),
        )?;
        self.reconcile_hulls(PairOwner::Recursive(id), candidates)
    }
    pub(super) fn refresh_recursive_pair(&mut self, id: u64) -> Result<(), EnvironmentError> {
        self.recalculate_child(id, ClosestFeatureMode::Invalid)?;
        let pair = &self.pairs.children[&id];
        if pair.closest.status() == ClosestFeatureStatus::Separated
            && pair
                .closest
                .geometry()
                .ok_or(PairError::MissingPair)?
                .separation
                > self.tolerances.friction_distance
        {
            self.clear_recursive_children(id)?;
            self.set_exact(id)?;
            self.pairs
                .children
                .get_mut(&id)
                .unwrap()
                .recursive
                .as_mut()
                .unwrap()
                .refined = None;
        } else {
            let cores = pair.bodies;
            let Residence::Refining { listeners } = pair.residence else {
                return Err(PairError::MissingListener.into());
            };
            let indices = self.pair_bodies(cores)?;
            let distances = self.search_ranges.pair(
                indices.map(|i| self.retained_motion(i)),
                indices.map(|i| self.bodies[i].physical.radius),
            )?;
            self.scan_recursive_pair(id, distances[0] + distances[1])?;
            for side in 0..2 {
                self.range_renew(cores[side], listeners[side], distances[side])?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursive_ownership_checks_parent_links_ordered_children_and_descendant_counts() {
        let (mut world, _) = super::super::super::tests::automatic_pair_world(false);
        let core = world.body(1).unwrap().core_identity();
        world.recheck_spatial(core).unwrap();
        let original = world.snapshot();
        let (&id, child) = world.pairs.children.iter().next().unwrap();
        let mut root = child.clone();
        root.hulls.0[1] = AuthoredHullRef::Enclosure(0);
        root.recursive = Some(RecursivePair::default());
        let mut nested = root.clone();
        nested.owner = PairOwner::Recursive(id);
        nested.hulls.0[1] = AuthoredHullRef::Enclosure(1);
        root.recursive
            .as_mut()
            .unwrap()
            .children
            .insert(nested.hulls, id + 1);
        root.recursive.as_mut().unwrap().hulls =
            HullPairSet::from_pairs(vec![nested.hulls], 8).unwrap();
        root.recursive.as_mut().unwrap().descendants = 1;
        root.recursive.as_mut().unwrap().refined = Some(1);
        root.residence = Residence::Refining { listeners: [0, 0] };
        let (hulls, children) = world.pairs.roster_mut(root.owner).unwrap();
        *hulls = HullPairSet::from_pairs(vec![root.hulls], 8).unwrap();
        children.clear();
        children.insert(root.hulls, id);
        world.pairs.children.insert(id, root);
        world.pairs.children.insert(id + 1, nested);
        for (&id, child) in &world.pairs.children {
            assert!(world.pairs.valid_child_owner(id, child));
        }
        assert_eq!(world.recursive_count(id + 1).unwrap(), 1);
        let mut bad = world.pairs.children[&id].clone();
        bad.recursive.as_mut().unwrap().descendants = 2;
        assert!(!world.pairs.valid_child_owner(id, &bad));
        let mut bad = world.pairs.children[&(id + 1)].clone();
        bad.owner = PairOwner::Recursive(id + 1);
        assert!(!world.pairs.valid_child_owner(id + 1, &bad));
        let mut counted = world.pairs.children[&id].clone();
        let state = counted.recursive.as_mut().unwrap();
        state.children.clear();
        state.hulls = HullPairSet::default();
        state.retired_descendants = 1;
        assert!(world.pairs.valid_child_owner(id, &counted));
        counted.recursive.as_mut().unwrap().retired_descendants = 2;
        assert!(!world.pairs.valid_child_owner(id, &counted));
        world.restore(original.clone()).unwrap();
        let mut bad = original.clone();
        bad.pairs.children.get_mut(&id).unwrap().recursive = Some(RecursivePair::default());
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), original);
    }
}
