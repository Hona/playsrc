use super::*;
mod recursive;
use crate::{
    AuthoredHullPair, ClosestFeatureInputs, ClosestFeatureMode, ClosestFeaturePair,
    ClosestFeatureStatus, ClosestFeatureUpdate, HullCandidates, HullPairEndpoint, HullPairSet,
    HullSearch, MovementPairHint, ObjectPairChanges, ObjectPairGraph, ObjectPairState,
    PairCoreProjection, PairRangeAction, PairRangeEndpoint, PairRangeQuery, PairResidence,
    PairResidenceInput, RecoveryEndpoint, SpatialIndex, SurfaceFeatureKind,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairError {
    Capacity,
    MissingPair,
    MissingListener,
    MissingRecursiveOwner,
    Spatial(crate::SpatialIndexError),
    Graph(crate::ObjectPairError),
    Hierarchy(crate::HierarchyError),
    Recovery(crate::RecoveryError),
}
impl fmt::Display for PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity => f.write_str("physical pair storage exhausted"),
            Self::MissingPair => f.write_str("retained physical pair is absent"),
            Self::MissingListener => f.write_str("physical movement listener is absent"),
            Self::MissingRecursiveOwner => f.write_str("convex pair has no recursive owner"),
            Self::Spatial(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::Hierarchy(e) => e.fmt(f),
            Self::Recovery(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for PairError {}
impl From<crate::SpatialIndexError> for EnvironmentError {
    fn from(e: crate::SpatialIndexError) -> Self {
        PairError::Spatial(e).into()
    }
}
impl From<crate::ObjectPairError> for EnvironmentError {
    fn from(e: crate::ObjectPairError) -> Self {
        PairError::Graph(e).into()
    }
}
impl From<crate::HierarchyError> for EnvironmentError {
    fn from(e: crate::HierarchyError) -> Self {
        PairError::Hierarchy(e).into()
    }
}
impl From<crate::RecoveryError> for EnvironmentError {
    fn from(e: crate::RecoveryError) -> Self {
        PairError::Recovery(e).into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RangeOwner {
    World(u64),
    Parent([u64; 2]),
    Child(u64),
}
#[derive(Clone, Debug, PartialEq)]
struct ParentPair {
    bodies: [u64; 2],
    listeners: [u64; 2],
    hulls: HullPairSet,
    children: BTreeMap<AuthoredHullPair, u64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Residence {
    Exact,
    Invalid,
    Movement {
        listeners: [u64; 2],
        rotation_travel: f64,
    },
    Refining {
        listeners: [u64; 2],
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PairOwner {
    Object([u64; 2]),
    Recursive(u64),
}
#[derive(Clone, Debug, Default, PartialEq)]
struct RecursivePair {
    hulls: HullPairSet,
    children: BTreeMap<AuthoredHullPair, u64>,
    refined: Option<usize>,
    descendants: u32,
    retired_descendants: u32,
}
#[derive(Clone, Debug, PartialEq)]
struct ChildPair {
    owner: PairOwner,
    recursive: Option<RecursivePair>,
    bodies: [u64; 2],
    hulls: AuthoredHullPair,
    closest: ClosestFeaturePair,
    residence: Residence,
    event: Option<ConvexPairEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConvexPairResidence {
    Exact,
    Invalid,
    Movement,
    Refining,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecursivePairState {
    pub children: Vec<u64>,
    pub descendants: u32,
    pub total_descendants: u32,
    pub refined_side: Option<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexPairState {
    pub identity: u64,
    pub parent: Option<u64>,
    pub residence: ConvexPairResidence,
    pub recursive: Option<RecursivePairState>,
    pub endpoints: [BodyConvex; 2],
    pub closest: ClosestFeaturePair,
    pub event: Option<ConvexPairEvent>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvexPairObservation {
    pub identity: u64,
    pub endpoints: [BodyConvex; 2],
    pub time: f64,
    pub boundary_time: f64,
    pub time_code: u32,
    pub mode: ClosestFeatureMode,
    pub update: ClosestFeatureUpdate,
    pub before: ClosestFeaturePair,
    pub after: ClosestFeaturePair,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PairSpace {
    observations: Option<Vec<ConvexPairObservation>>,
    spatial: SpatialIndex,
    graph: ObjectPairGraph,
    world_listeners: BTreeMap<u64, u64>,
    listeners: BTreeMap<u64, RangeOwner>,
    parents: BTreeMap<[u64; 2], ParentPair>,
    children: BTreeMap<u64, ChildPair>,
    exact: Vec<u64>,
    invalid: BTreeMap<u64, Vec<u64>>,
    next_listener: u64,
    next_child: u64,
    maximum: usize,
}
fn key(mut pair: [u64; 2]) -> [u64; 2] {
    pair.sort_unstable();
    pair
}
impl PairSpace {
    pub(super) fn has_fluid_pair(
        &self,
        id: u64,
        core: u64,
        fluid_body: u64,
        bodies: &[RigidBody],
    ) -> bool {
        self.children.get(&id).is_some_and(|child| {
            child.bodies.contains(&core)
                && child.bodies.iter().any(|other| {
                    bodies
                        .iter()
                        .any(|body| body.core_identity == *other && body.identity == fluid_body)
                })
        })
    }
    pub(super) fn validate(&self, snapshot: &EnvironmentSnapshot) -> Result<(), EnvironmentError> {
        let invalid = || EnvironmentError::SnapshotMismatch;
        if self.observations.as_ref().is_some_and(|values| {
            values.len() > self.maximum
                || values
                    .iter()
                    .any(|v| !v.time.is_finite() || v.time > snapshot.clock.current_time())
        }) {
            return Err(invalid());
        }
        if self.maximum != snapshot.config.max_events
            || self.graph.pair_count() != self.parents.len()
            || self.children.len() > self.maximum
            || self.invalid.len() != snapshot.bodies.len()
            || self.spatial.element_count()
                != snapshot
                    .bodies
                    .iter()
                    .filter(|b| b.collisions_enabled)
                    .count()
        {
            return Err(invalid());
        }
        for body in &snapshot.bodies {
            if self.graph.links(body.core_identity).is_none()
                || !self.invalid.contains_key(&body.core_identity)
                || self.spatial.contains(body.core_identity) != body.collisions_enabled
            {
                return Err(invalid());
            }
        }
        let has_listener = |core: u64, id: u64, owner: RangeOwner| {
            self.listeners.get(&id) == Some(&owner)
                && snapshot
                    .bodies
                    .iter()
                    .find(|b| b.core_identity == core)
                    .is_some_and(|body| {
                        body.movement_range
                            .listeners()
                            .iter()
                            .any(|listener| listener.identity == id)
                    })
        };
        for (&core, &id) in &self.world_listeners {
            if !has_listener(core, id, RangeOwner::World(core)) {
                return Err(invalid());
            }
        }
        for (&identity, parent) in &self.parents {
            if key(parent.bodies) != identity
                || !self
                    .graph
                    .links(parent.bodies[0])
                    .is_some_and(|links| links.contains(&parent.bodies))
                || parent.children.len() != parent.hulls.pairs().len()
            {
                return Err(invalid());
            }
            for side in 0..2 {
                if !has_listener(
                    parent.bodies[side],
                    parent.listeners[side],
                    RangeOwner::Parent(identity),
                ) {
                    return Err(invalid());
                }
            }
            for (hulls, id) in &parent.children {
                if !parent.hulls.pairs().contains(hulls)
                    || !self.children.get(id).is_some_and(|child| {
                        child.bodies == parent.bodies
                            && child.hulls == *hulls
                            && child.owner == PairOwner::Object(identity)
                    })
                {
                    return Err(invalid());
                }
            }
        }
        for (&id, child) in &self.children {
            if id == 0 || id >= self.next_child || !self.valid_child_owner(id, child) {
                return Err(invalid());
            }
            let features = child.closest.selection().pair;
            let features = [features.first, features.second];
            for (side, feature) in features.into_iter().enumerate() {
                let body = snapshot
                    .bodies
                    .iter()
                    .find(|b| b.core_identity == child.bodies[side])
                    .ok_or_else(invalid)?;
                let index = body.hull_index(child.hulls.0[side])?;
                body.topology(index)
                    .ok_or_else(invalid)?
                    .edge(feature.edge)?;
            }
            if child.closest.geometry().is_some_and(|geometry| {
                !geometry.separation.is_finite()
                    || !geometry.core_projection.is_finite()
                    || geometry.normal.iter().any(|v| !v.is_finite())
            }) {
                return Err(invalid());
            }
            let exact = self.exact.iter().filter(|value| **value == id).count();
            let invalid_count = child.bodies.map(|core| {
                self.invalid
                    .get(&core)
                    .map_or(0, |ids| ids.iter().filter(|value| **value == id).count())
            });
            match child.residence {
                Residence::Exact => {
                    if exact != 1 || invalid_count != [0, 0] {
                        return Err(invalid());
                    }
                }
                Residence::Invalid => {
                    if exact != 0 || invalid_count != [1, 1] {
                        return Err(invalid());
                    }
                }
                Residence::Movement {
                    listeners,
                    rotation_travel,
                } => {
                    if exact != 0
                        || invalid_count != [0, 0]
                        || !rotation_travel.is_finite()
                        || ((child.closest.status() != ClosestFeatureStatus::Separated
                            || child.closest.geometry().is_none())
                            && !snapshot.fluids.contains_pair(id))
                    {
                        return Err(invalid());
                    }
                    for (side, listener) in listeners.into_iter().enumerate() {
                        if !has_listener(child.bodies[side], listener, RangeOwner::Child(id)) {
                            return Err(invalid());
                        }
                    }
                }
                Residence::Refining { listeners } => {
                    if exact != 0
                        || invalid_count != [0, 0]
                        || !child
                            .recursive
                            .as_ref()
                            .is_some_and(|state| state.refined.is_some())
                    {
                        return Err(invalid());
                    }
                    for (side, listener) in listeners.into_iter().enumerate() {
                        if !has_listener(child.bodies[side], listener, RangeOwner::Child(id)) {
                            return Err(invalid());
                        }
                    }
                }
            }
            let queued = snapshot
                .queue
                .entries()
                .iter()
                .filter(|event| event.identity == EnvironmentEvent::Convex(id))
                .count();
            if queued != usize::from(child.event.is_some())
                || child.event.is_some_and(|event| {
                    !event.time.is_finite() || child.residence != Residence::Exact
                })
            {
                return Err(invalid());
            }
        }
        if self.exact.iter().any(|id| !self.children.contains_key(id)) {
            return Err(invalid());
        }
        for (core, ids) in &self.invalid {
            if ids.iter().any(|id| {
                !self.children.get(id).is_some_and(|child| {
                    child.bodies.contains(core) && child.residence == Residence::Invalid
                })
            }) {
                return Err(invalid());
            }
        }
        for (id, owner) in &self.listeners {
            if *id == 0 || *id >= self.next_listener {
                return Err(invalid());
            }
            let retained=match owner {RangeOwner::World(core)=>self.world_listeners.get(core)==Some(id),RangeOwner::Parent(parent)=>self.parents.get(parent).is_some_and(|parent|parent.listeners.contains(id)),RangeOwner::Child(child)=>self.children.get(child).is_some_and(|child|matches!(child.residence,Residence::Movement {listeners,..}|Residence::Refining {listeners} if listeners.contains(id)))};
            if !retained {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub(super) fn event_count(&self) -> usize {
        self.children.values().filter(|c| c.event.is_some()).count()
    }
    pub(super) fn new(bodies: usize, maximum: usize) -> Result<Self, PairError> {
        let spatial = SpatialIndex::new(
            bodies,
            bodies
                .checked_mul(128)
                .and_then(|v| v.checked_add(1))
                .ok_or(PairError::Capacity)?,
        )
        .map_err(PairError::Spatial)?;
        Ok(Self {
            observations: None,
            spatial,
            graph: ObjectPairGraph::from_links(Vec::new(), maximum).map_err(PairError::Graph)?,
            world_listeners: BTreeMap::new(),
            listeners: BTreeMap::new(),
            parents: BTreeMap::new(),
            children: BTreeMap::new(),
            exact: Vec::new(),
            invalid: BTreeMap::new(),
            next_listener: 1,
            next_child: 1,
            maximum,
        })
    }
    pub(super) fn register_body(&mut self, id: u64) -> Result<(), PairError> {
        self.graph.register(id).map_err(PairError::Graph)?;
        self.invalid.insert(id, Vec::new());
        Ok(())
    }
    fn listener(&mut self, owner: RangeOwner) -> Result<u64, PairError> {
        let id = self.next_listener;
        self.next_listener = id.checked_add(1).ok_or(PairError::Capacity)?;
        self.listeners.insert(id, owner);
        Ok(id)
    }
}

impl PhysicsEnvironment {
    pub(super) fn shift_pair_event_times(&mut self, shift: f64) {
        for child in self.pairs.children.values_mut() {
            if let Some(event) = &mut child.event {
                event.time -= shift;
            }
        }
    }
    pub(super) fn reset_body_ranges(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let index = self.core_body_index(core)?;
        let mut shifts = Vec::new();
        self.bodies[index]
            .movement_range
            .reset_values(|shift| shifts.push(shift));
        for shift in shifts {
            self.apply_range_reset(shift)?;
        }
        Ok(())
    }
    fn apply_range_reset(&mut self, reset: crate::RangeReset) -> Result<(), EnvironmentError> {
        if let Some(RangeOwner::Child(id)) = self.pairs.listeners.get(&reset.identity)
            && let Residence::Movement {
                rotation_travel, ..
            } = &mut self
                .pairs
                .children
                .get_mut(id)
                .ok_or(PairError::MissingPair)?
                .residence
        {
            *rotation_travel = MovementPairHint::shift_rotation_travel(
                *rotation_travel,
                reset.total_shift,
                reset.center_shift,
            )?;
        }
        Ok(())
    }
    pub(super) fn move_dormant_pairs(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let ids = self
            .pairs
            .exact
            .iter()
            .filter(|id| self.pairs.children[id].bodies.contains(&core))
            .copied()
            .collect::<Vec<_>>();
        for id in ids {
            self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
            self.update_pair_policy(id, false, EventTimingHint::Immediate)?;
        }
        Ok(())
    }
    pub(super) fn grow_revival_contacts(&mut self, core: u64) -> Result<bool, EnvironmentError> {
        let ids = self
            .pairs
            .exact
            .iter()
            .filter(|id| self.pairs.children[id].bodies.contains(&core))
            .copied()
            .collect::<Vec<_>>();
        let mut grew = false;
        for id in ids {
            let Some(child) = self.pairs.children.get(&id) else {
                continue;
            };
            let peer = child.bodies[usize::from(child.bodies[0] == core)];
            let index = self.core_body_index(peer)?;
            if self.bodies[index].kind == BodyKind::Static || self.has_contact_group(peer) {
                continue;
            }
            self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
            let child = &self.pairs.children[&id];
            if child.closest.status() != ClosestFeatureStatus::Separated
                || child.closest.geometry().unwrap().separation
                    >= self.tolerances.maximum_friction_distance
            {
                continue;
            }
            let selection = child.closest.selection();
            let endpoints = self.child_endpoints(child)?;
            let endpoints = selection.order().map(|side| endpoints[side]);
            let first = self.cached_transform(endpoints[0].body)?.object;
            self.transforms.pin(endpoints[0].body)?;
            let second = self.cached_transform(endpoints[1].body)?.object;
            self.transforms.pin(endpoints[1].body)?;
            let geometry = self
                .project_pair_contact(
                    ConvexContactPair {
                        endpoints,
                        seed: selection.ordered_pair(),
                        start: self.time(),
                        end: self.clock.next_boundary(),
                        maximum_feature_transitions: 1024,
                    },
                    [first, second],
                )?
                .1;
            self.transforms.release(endpoints[1].body)?;
            self.transforms.release(endpoints[0].body)?;
            let before = self.contact_count();
            self.admit_contact(geometry, core)?;
            if self.contact_count() != before {
                let now = self.time();
                self.bodies[index].quiet.refresh_time(now);
                grew = true;
            }
        }
        Ok(grew)
    }
    pub(super) fn collision_source_cores(&self, id: u64) -> Result<[u64; 2], EnvironmentError> {
        Ok(self
            .pairs
            .children
            .get(&id)
            .ok_or(PairError::MissingPair)?
            .bodies)
    }
    pub fn record_convex_observations(&mut self, enabled: bool) {
        self.pairs.observations = enabled.then(Vec::new);
    }
    pub fn convex_observations(&self) -> Option<&[ConvexPairObservation]> {
        self.pairs.observations.as_deref()
    }
    pub(super) fn clear_convex_observations(&mut self) {
        if let Some(values) = &mut self.pairs.observations {
            values.clear();
        }
    }
    pub fn convex_pairs(&self) -> Result<Vec<ConvexPairState>, EnvironmentError> {
        self.pairs
            .children
            .iter()
            .map(|(&identity, child)| {
                Ok(ConvexPairState {
                    identity,
                    parent: match child.owner {
                        PairOwner::Object(_) => None,
                        PairOwner::Recursive(id) => Some(id),
                    },
                    residence: match child.residence {
                        Residence::Exact => ConvexPairResidence::Exact,
                        Residence::Invalid => ConvexPairResidence::Invalid,
                        Residence::Movement { .. } => ConvexPairResidence::Movement,
                        Residence::Refining { .. } => ConvexPairResidence::Refining,
                    },
                    recursive: child
                        .recursive
                        .as_ref()
                        .map(|state| {
                            Ok::<_, EnvironmentError>(RecursivePairState {
                                children: state
                                    .hulls
                                    .pairs()
                                    .iter()
                                    .map(|hull| state.children[hull])
                                    .collect(),
                                descendants: state.descendants,
                                total_descendants: self.recursive_count(identity)?,
                                refined_side: state.refined,
                            })
                        })
                        .transpose()?,
                    endpoints: self.child_endpoints(child)?,
                    closest: child.closest,
                    event: child.event,
                })
            })
            .collect()
    }
    fn pair_bodies(&self, ids: [u64; 2]) -> Result<[usize; 2], EnvironmentError> {
        Ok([self.core_body_index(ids[0])?, self.core_body_index(ids[1])?])
    }
    fn retained_motion(&self, index: usize) -> CollisionMotion {
        self.bodies[index]
            .motion_phase
            .map_or_else(CollisionMotion::stationary, |phase| phase.motion)
    }
    fn core_projection(&self, index: usize) -> PairCoreProjection {
        let body = &self.bodies[index];
        let phase = body.motion_phase();
        PairCoreProjection {
            position: phase.map_or(body.core_position, |p| p.position),
            velocity: phase.map_or([0.0; 3], |p| p.projection_velocity),
            time: phase.map_or(body.core_time, |p| p.start),
            radius: body.physical.radius,
        }
    }
    fn range_remove(&mut self, core: u64, id: u64) -> Result<(), EnvironmentError> {
        let index = self.core_body_index(core)?;
        self.bodies[index].movement_range.remove(id)?;
        self.pairs.listeners.remove(&id);
        Ok(())
    }
    fn range_renew(&mut self, core: u64, id: u64, distance: f64) -> Result<(), EnvironmentError> {
        let index = self.core_body_index(core)?;
        self.bodies[index]
            .movement_range
            .renew(id, self.clock.current_time(), distance)?;
        Ok(())
    }
    fn range_insert(
        &mut self,
        core: u64,
        owner: RangeOwner,
        distance: f64,
    ) -> Result<u64, EnvironmentError> {
        let index = self.core_body_index(core)?;
        let id = self.pairs.listener(owner)?;
        let key = self.bodies[index]
            .movement_range
            .clock()
            .key(self.clock.current_time(), distance)?;
        self.bodies[index].movement_range.insert(id, key)?;
        Ok(id)
    }
    pub(super) fn recheck_spatial(&mut self, core: u64) -> Result<(), EnvironmentError> {
        self.update_spatial(core, false)
    }
    pub(super) fn enable_spatial(&mut self, core: u64) -> Result<(), EnvironmentError> {
        self.update_spatial(core, true)
    }
    fn update_spatial(&mut self, core: u64, enabling: bool) -> Result<(), EnvironmentError> {
        let index = self.core_body_index(core)?;
        if !self.bodies[index].collisions_enabled && !enabling {
            return Ok(());
        }
        self.pairs.spatial.remove(core);
        let body = &self.bodies[index];
        let radius = body.physical.radius;
        let range = self
            .search_ranges
            .world(self.retained_motion(index), radius)?;
        let pose = self.impulse_frame(body.identity)?;
        let inserted = self.pairs.spatial.insert(
            core,
            pose.position.map(|v| v as f32),
            range + f64::from(radius),
            range + f64::from(radius),
            true,
        )?;
        if let Some(listener) = self.pairs.world_listeners.get(&core).copied() {
            self.range_renew(core, listener, inserted.radius - f64::from(radius))?;
        } else {
            let id = self.range_insert(
                core,
                RangeOwner::World(core),
                inserted.radius - f64::from(radius),
            )?;
            self.pairs.world_listeners.insert(core, id);
        }
        let states = self
            .bodies
            .iter()
            .map(|body| ObjectPairState {
                identity: body.core_identity,
                friction_core: body.core_identity,
                moving: self
                    .islands
                    .movement(body.core_identity)
                    .is_some_and(crate::CoreMovement::can_collide),
                immovable: body.kind == BodyKind::Static,
                pinned: !body.motion_enabled,
                enabled: body.collisions_enabled || (enabling && body.core_identity == core),
            })
            .collect::<Vec<_>>();
        let filter = &self.collision_solver;
        let bodies = &self.bodies;
        let changes = self
            .pairs
            .graph
            .reconcile(core, &states, &inserted.partners, |a, b| {
                let a = bodies.iter().find(|v| v.core_identity == a).unwrap();
                let b = bodies.iter().find(|v| v.core_identity == b).unwrap();
                game_collision_allowed(filter, a, b)
            })?;
        self.apply_object_pair_changes(changes)
    }
    fn apply_object_pair_changes(
        &mut self,
        changes: ObjectPairChanges,
    ) -> Result<(), EnvironmentError> {
        for pair in changes.retired {
            self.remove_parent(key(pair))?;
        }
        for pair in changes.created {
            let id = key(pair);
            let listeners = [
                self.range_insert(
                    pair[0],
                    RangeOwner::Parent(id),
                    f64::from(f32::from_bits(0x60ad78ec)),
                )?,
                self.range_insert(
                    pair[1],
                    RangeOwner::Parent(id),
                    f64::from(f32::from_bits(0x60ad78ec)),
                )?,
            ];
            self.pairs.parents.insert(
                id,
                ParentPair {
                    bodies: pair,
                    listeners,
                    hulls: HullPairSet::default(),
                    children: BTreeMap::new(),
                },
            );
            self.refresh_parent(id)?;
        }
        Ok(())
    }
    fn refresh_parent(&mut self, id: [u64; 2]) -> Result<(), EnvironmentError> {
        let parent = self
            .pairs
            .parents
            .get(&id)
            .ok_or(PairError::MissingPair)?
            .clone();
        let indices = self.pair_bodies(parent.bodies)?;
        let distances = self.search_ranges.pair(
            indices.map(|i| self.retained_motion(i)),
            indices.map(|i| self.bodies[i].physical.radius),
        )?;
        for (side, distance) in distances.into_iter().enumerate() {
            self.range_renew(parent.bodies[side], parent.listeners[side], distance)?;
        }
        let poses = [
            self.cached_transform(self.bodies[indices[0]].identity)?
                .object,
            self.cached_transform(self.bodies[indices[1]].identity)?
                .object,
        ];
        let searches = [
            HullSearch::Spatial {
                pose: poses[0],
                refine: None,
            },
            HullSearch::Spatial {
                pose: poses[1],
                refine: None,
            },
        ];
        let mut maximum_visits = 1;
        for index in indices {
            maximum_visits = maximum_visits.max(
                self.bodies[index]
                    .shape
                    .authored_hierarchy()
                    .map_or(1, |h| h.nodes.len()),
            );
        }
        let candidates = crate::query_hull_pairs(
            std::array::from_fn(|side| HullPairEndpoint {
                shape: &self.bodies[indices[side]].shape,
                core: self.core_projection(indices[side]),
                extra_radius: 0.0,
                search: searches[side],
            }),
            self.clock.current_time(),
            distances[0] + distances[1],
            maximum_visits,
        )?;
        self.reconcile_hulls(PairOwner::Object(id), candidates)
    }
    fn reconcile_hulls(
        &mut self,
        parent: PairOwner,
        candidates: HullCandidates,
    ) -> Result<(), EnvironmentError> {
        let mut ordered = self
            .pairs
            .roster(parent)
            .ok_or(PairError::MissingPair)?
            .0
            .clone();
        let before = ordered.pairs().len();
        let changes = ordered.reconcile(&candidates, self.pairs.maximum)?;
        let cores = match parent {
            PairOwner::Object(id) => self.pairs.parents[&id].bodies,
            PairOwner::Recursive(id) => self.pairs.children[&id].bodies,
        };
        for hulls in changes.created {
            if self.pairs.children.len() == self.pairs.maximum {
                return Err(PairError::Capacity.into());
            }
            let indices = self.pair_bodies(cores)?;
            let convexes = [
                self.bodies[indices[0]].hull_index(hulls.0[0])?,
                self.bodies[indices[1]].hull_index(hulls.0[1])?,
            ];
            let edges = std::array::from_fn(|side| {
                self.bodies[indices[side]]
                    .topology(convexes[side])
                    .unwrap()
                    .edge_id(0)
                    .unwrap()
            });
            let id = self.pairs.next_child;
            self.pairs.next_child = id.checked_add(1).ok_or(PairError::Capacity)?;
            self.pairs.children.insert(
                id,
                ChildPair {
                    owner: parent,
                    recursive: hulls
                        .0
                        .iter()
                        .any(|hull| matches!(hull, AuthoredHullRef::Enclosure(_)))
                        .then(RecursivePair::default),
                    bodies: cores,
                    hulls,
                    closest: ClosestFeaturePair::new(edges, 0.0)?,
                    residence: Residence::Exact,
                    event: None,
                },
            );
            self.pairs.exact.insert(0, id);
            self.pairs
                .roster_mut(parent)
                .ok_or(PairError::MissingPair)?
                .1
                .insert(hulls, id);
            self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
            let allow = cores
                .iter()
                .all(|core| self.islands.movement(*core) != Some(crate::CoreMovement::Discovering));
            self.update_pair_policy(id, allow, EventTimingHint::Immediate)?;
        }
        for hulls in changes.retired {
            let id = self
                .pairs
                .roster_mut(parent)
                .ok_or(PairError::MissingPair)?
                .1
                .remove(&hulls)
                .ok_or(PairError::MissingPair)?;
            self.remove_child(id)?;
        }
        let after = ordered.pairs().len();
        *self
            .pairs
            .roster_mut(parent)
            .ok_or(PairError::MissingPair)?
            .0 = ordered;
        if let PairOwner::Recursive(id) = parent {
            self.adjust_recursive_count(id, after as i32 - before as i32)?;
        }
        Ok(())
    }
    fn child_endpoints(&self, child: &ChildPair) -> Result<[BodyConvex; 2], EnvironmentError> {
        let indices = self.pair_bodies(child.bodies)?;
        Ok([
            BodyConvex {
                body: self.bodies[indices[0]].identity,
                convex: self.bodies[indices[0]].hull_index(child.hulls.0[0])?,
            },
            BodyConvex {
                body: self.bodies[indices[1]].identity,
                convex: self.bodies[indices[1]].hull_index(child.hulls.0[1])?,
            },
        ])
    }
    fn recalculate_child(
        &mut self,
        id: u64,
        mode: ClosestFeatureMode,
    ) -> Result<ClosestFeatureUpdate, EnvironmentError> {
        if self
            .pairs
            .observations
            .as_ref()
            .is_some_and(|values| values.len() == self.config.max_events)
        {
            return Err(EnvironmentError::ObservationLimit);
        }
        let before = self
            .pairs
            .children
            .get(&id)
            .ok_or(PairError::MissingPair)?
            .closest;
        let result = self.recalculate_child_inner(id, mode)?;
        if self.pairs.observations.is_some() {
            let child = &self.pairs.children[&id];
            let observation = ConvexPairObservation {
                identity: id,
                endpoints: self.child_endpoints(child)?,
                time: self.time(),
                boundary_time: self.clock.last_boundary(),
                time_code: self.clock.time_code(),
                mode,
                update: result,
                before,
                after: child.closest,
            };
            if let Some(observations) = &mut self.pairs.observations {
                observations.push(observation);
            }
        }
        if matches!(
            result,
            ClosestFeatureUpdate::Calculated(
                ClosestFeatureStatus::Intruded | ClosestFeatureStatus::RecoveryLimit
            )
        ) {
            if self.is_fluid_pair(id)? {
                self.update_fluid_pair(id)?;
                return Ok(result);
            }
            if self.pairs.children[&id].recursive.is_some() {
                if !matches!(
                    self.pairs.children[&id].residence,
                    Residence::Refining { .. }
                ) {
                    self.refine_recursive_pair(id, crate::RecursiveHullEvent::InvalidOverlap)?;
                }
            } else {
                self.set_invalid(id)?;
                self.recover_child(id)?;
            }
        }
        if self.is_fluid_pair(id)? {
            self.update_fluid_pair(id)?;
        }
        Ok(result)
    }
    fn recalculate_child_inner(
        &mut self,
        id: u64,
        mode: ClosestFeatureMode,
    ) -> Result<ClosestFeatureUpdate, EnvironmentError> {
        self.statistics.feature_updates = self
            .statistics
            .feature_updates
            .checked_add(1)
            .ok_or(EnvironmentError::ClockOverflow)?;
        let child = self
            .pairs
            .children
            .get(&id)
            .ok_or(PairError::MissingPair)?
            .clone();
        if child.closest.time_code() == self.clock.time_code() {
            return Ok(ClosestFeatureUpdate::Cached);
        }
        let endpoints = self.child_endpoints(&child)?;
        let indices = self.pair_bodies(child.bodies)?;
        let first = self.cached_transform(endpoints[0].body)?;
        self.transforms.pin(endpoints[0].body)?;
        let second = self.cached_transform(endpoints[1].body)?;
        self.transforms.pin(endpoints[1].body)?;
        let caches = [first, second];
        let mut closest = child.closest;
        let update = closest.recalculate(
            self.clock.time_code(),
            mode,
            1024,
            || {
                Ok(ClosestFeatureInputs {
                    placements: std::array::from_fn(|side| FeaturePlacement {
                        topology: self.bodies[indices[side]]
                            .topology(endpoints[side].convex)
                            .unwrap(),
                        position: caches[side].object.position,
                        orientation: caches[side].object.orientation,
                    }),
                    core_positions: caches.map(|p| p.core_position),
                })
            },
            |_| {},
        );
        self.transforms.release(endpoints[1].body)?;
        self.transforms.release(endpoints[0].body)?;
        let update = update?;
        self.pairs.children.get_mut(&id).unwrap().closest = closest;
        Ok(update)
    }
    fn set_invalid(&mut self, id: u64) -> Result<(), EnvironmentError> {
        let child = self.pairs.children[&id].clone();
        if child.residence == Residence::Invalid {
            return Ok(());
        }
        self.clear_residence(id)?;
        self.pairs.children.get_mut(&id).unwrap().residence = Residence::Invalid;
        for core in child.bodies {
            self.pairs
                .invalid
                .get_mut(&core)
                .ok_or(PairError::MissingPair)?
                .insert(0, id);
        }
        Ok(())
    }
    fn set_exact(&mut self, id: u64) -> Result<(), EnvironmentError> {
        if self.pairs.children[&id].residence == Residence::Exact {
            return Ok(());
        }
        self.clear_residence(id)?;
        self.pairs.children.get_mut(&id).unwrap().residence = Residence::Exact;
        self.pairs.exact.insert(0, id);
        Ok(())
    }
    fn clear_residence(&mut self, id: u64) -> Result<(), EnvironmentError> {
        self.cancel_child_event(id)?;
        let child = self.pairs.children[&id].clone();
        match child.residence {
            Residence::Exact => self.pairs.exact.retain(|value| *value != id),
            Residence::Invalid => {
                for core in child.bodies {
                    self.pairs
                        .invalid
                        .get_mut(&core)
                        .unwrap()
                        .retain(|value| *value != id);
                }
            }
            Residence::Movement { listeners, .. } | Residence::Refining { listeners } => {
                for side in (0..2).rev() {
                    self.range_remove(child.bodies[side], listeners[side])?;
                }
            }
        }
        self.pairs.children.get_mut(&id).unwrap().event = None;
        Ok(())
    }
    fn recover_child(&mut self, id: u64) -> Result<(), EnvironmentError> {
        let child = self.pairs.children[&id].clone();
        let indices = self.pair_bodies(child.bodies)?;
        let endpoints = self.child_endpoints(&child)?;
        if let Some(solver) = &mut self.collision_solver.0 {
            if indices
                .iter()
                .any(|index| self.bodies[*index].callback_flags & 0x0400 != 0)
            {
                return Ok(());
            }
            if indices
                .iter()
                .all(|index| self.bodies[*index].is_moveable())
            {
                let pair = key(child.bodies);
                if self.recovered_pairs.contains(&pair) {
                    return Ok(());
                }
                if self.recovered_pairs.len() == self.config.max_events {
                    return Err(EnvironmentError::CollisionLimit);
                }
                self.recovered_pairs.push(pair);
            }
            if !solver.should_solve_penetration(
                endpoints[0].body,
                endpoints[1].body,
                self.config.timestep,
            ) {
                return Ok(());
            }
        }
        let features = [
            child.closest.selection().pair.first,
            child.closest.selection().pair.second,
        ];
        let frames = [
            self.sampled_impulse_frame(endpoints[0].body)?,
            self.sampled_impulse_frame(endpoints[1].body)?,
        ];
        let positions = [
            self.impulse_frame(endpoints[0].body)?.position,
            self.impulse_frame(endpoints[1].body)?.position,
        ];
        let bodies = std::array::from_fn(|side| {
            let body = &self.bodies[indices[side]];
            RecoveryEndpoint {
                position: positions[side],
                impulse_position: frames[side].position,
                orientation: frames[side].orientation,
                phase_linear: body
                    .motion_phase()
                    .map_or([0.0; 3], |p| p.projection_velocity),
                queued: body.queued_velocity,
                mass: body.physical.mass,
                inverse_mass: frames[side].inverse_mass,
                inverse_inertia: frames[side].inverse_inertia,
                simulated: self
                    .islands
                    .movement(body.core_identity)
                    .is_some_and(crate::CoreMovement::is_simulated),
                immovable: body.kind == BodyKind::Static,
                pinned: !body.motion_enabled,
                feature: features[side].kind,
            }
        });
        let fixed = usize::from(bodies[1].immovable);
        let mut fixed_pose = None;
        if bodies[fixed].immovable
            && bodies[fixed].feature != SurfaceFeatureKind::InteriorFace
            && bodies[1 - fixed].simulated
        {
            fixed_pose = Some(self.cached_transform(endpoints[fixed].body)?.object);
        }
        let speed = crate::penetration_recovery_speed(
            internal_position(self.config.gravity),
            self.config.timestep,
        )?;
        let result =
            crate::recover_overlap(bodies, f64::from(speed), self.config.timestep, |side| {
                let pose = fixed_pose.expect("required fixed transform");
                Ok(FeaturePlacement {
                    topology: self.bodies[indices[side]]
                        .topology(endpoints[side].convex)
                        .unwrap(),
                    position: pose.position,
                    orientation: pose.orientation,
                })
            })?;
        for (side, index) in indices.into_iter().enumerate() {
            self.bodies[index].queued_velocity = result.queued[side];
            if result.refreshed[side] {
                let now = self.time();
                self.bodies[index].quiet.refresh_time(now);
            }
        }
        Ok(())
    }
    pub(super) fn recheck_invalid_body(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let ids = self
            .pairs
            .invalid
            .get(&core)
            .cloned()
            .ok_or(PairError::MissingPair)?;
        for id in ids {
            if !self.pairs.children.contains_key(&id) {
                continue;
            }
            self.recalculate_child(id, ClosestFeatureMode::Invalid)?;
            if self.pairs.children[&id].closest.status() == ClosestFeatureStatus::Separated {
                self.set_exact(id)?;
            }
        }
        Ok(())
    }
    fn update_pair_policy(
        &mut self,
        id: u64,
        allow_movement: bool,
        hint: EventTimingHint,
    ) -> Result<(), EnvironmentError> {
        if matches!(
            self.pairs.children[&id].residence,
            Residence::Refining { .. }
        ) || self.pairs.children[&id].closest.status() != ClosestFeatureStatus::Separated
        {
            return Ok(());
        }
        let child = self.pairs.children[&id].clone();
        let indices = self.pair_bodies(child.bodies)?;
        let geometry = child.closest.geometry().ok_or(PairError::MissingPair)?;
        let motion = indices.map(|i| self.retained_motion(i));
        let moving = child.bodies.map(|core| {
            self.islands
                .movement(core)
                .is_some_and(crate::CoreMovement::can_collide)
        });
        match PairResidence::select(
            PairResidenceInput {
                gap: geometry.separation,
                selected_first: child.closest.selection().order()[0],
                moving,
                timestep: self.config.timestep,
                allow_movement,
            },
            motion,
            self.tolerances,
        )? {
            PairResidence::Movement { distances } => {
                self.clear_residence(id)?;
                let listeners = [
                    self.range_insert(
                        child.bodies[0],
                        RangeOwner::Child(id),
                        f64::from(distances[0]),
                    )?,
                    self.range_insert(
                        child.bodies[1],
                        RangeOwner::Child(id),
                        f64::from(distances[1]),
                    )?,
                ];
                let rotation = self.bodies[indices[0]]
                    .movement_range
                    .clock()
                    .rotation_travel(self.time())?
                    + self.bodies[indices[1]]
                        .movement_range
                        .clock()
                        .rotation_travel(self.time())?;
                self.pairs.children.get_mut(&id).unwrap().residence = Residence::Movement {
                    listeners,
                    rotation_travel: rotation,
                };
            }
            PairResidence::Exact => {
                self.set_exact(id)?;
                self.schedule_child(id, hint)?;
            }
        }
        Ok(())
    }
    fn schedule_child(&mut self, id: u64, hint: EventTimingHint) -> Result<(), EnvironmentError> {
        self.cancel_child_event(id)?;
        if self.is_fluid_pair(id)? {
            return Ok(());
        }
        let child = &self.pairs.children[&id];
        let indices = self.pair_bodies(child.bodies)?;
        let end = self.clock.next_boundary();
        let start = self.time();
        if end <= start {
            return Ok(());
        }
        let endpoints = self.child_endpoints(child)?;
        let poses = [
            self.cached_transform(endpoints[0].body)?.object,
            self.cached_transform(endpoints[1].body)?.object,
        ];
        let child = &self.pairs.children[&id];
        let motions: [FeatureMotion; 2] = std::array::from_fn(|side| {
            let body = &self.bodies[indices[side]];
            body.motion_phase()
                .map_or(FeatureMotion::Stationary(poses[side]), |phase| {
                    FeatureMotion::Moving {
                        phase,
                        frame: body.frame,
                        cache_time: start,
                        cached: poses[side],
                    }
                })
        });
        let query = ConvexPairQuery {
            endpoints: std::array::from_fn(|side| ConvexEndpoint {
                topology: self.bodies[indices[side]]
                    .topology(endpoints[side].convex)
                    .unwrap(),
                physical: &self.bodies[indices[side]].physical,
                motion: motions[side],
                bounds: self.retained_motion(indices[side]),
            }),
            seed: child.closest.selection().pair,
            start,
            end,
            extra_radius: 0.0,
            tolerances: self.tolerances,
            maximum_feature_transitions: 1024,
        };
        let result = query.predict_selected(
            child.closest.selection(),
            child.closest.geometry().ok_or(PairError::MissingPair)?,
        )?;
        if let Some(mut event) = result.event
            && let Some(time) = (ContinuousEventDelay {
                separation: result.separation,
                collision_distance: self.tolerances.real_surface,
                speed: result.bounds.worst_case_speed,
                timestep: self.config.timestep,
                scale: 1.0,
                current_time: start,
                proposed_time: event.time,
                phase_end: end,
                hint,
                kind: event.kind,
            })
            .candidate()?
        {
            event.time = time;
            self.queue.insert(EnvironmentEvent::Convex(id), time)?;
            self.pairs.children.get_mut(&id).unwrap().event = Some(event);
        }
        Ok(())
    }
    pub(super) fn refresh_exact_pairs(&mut self) -> Result<(), EnvironmentError> {
        let ids = self.pairs.exact.clone();
        for id in ids {
            if self
                .pairs
                .children
                .get(&id)
                .is_some_and(|c| c.residence == Residence::Exact)
            {
                self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
            }
        }
        let ids = self.pairs.exact.clone();
        for id in ids {
            if self
                .pairs
                .children
                .get(&id)
                .is_some_and(|c| c.residence == Residence::Exact)
            {
                self.update_pair_policy(id, true, EventTimingHint::ShortDelay)?;
            }
        }
        Ok(())
    }
    pub(super) fn dispatch_ranges(&mut self, active: &[u64]) -> Result<(), EnvironmentError> {
        for core in active.iter().rev() {
            let index = self.core_body_index(*core)?;
            let mut budget = crate::movement_range::RangeBudget::new(
                self.config.performance.max_collision_checks as u32,
            )?;
            while let Some(event) = self.bodies[index].movement_range.due_callback()? {
                let owner = self
                    .pairs
                    .listeners
                    .get(&event.identity)
                    .copied()
                    .ok_or(PairError::MissingListener)?;
                match owner {
                    RangeOwner::World(core) => self.recheck_spatial(core)?,
                    RangeOwner::Parent(parent) => self.refresh_parent(parent)?,
                    RangeOwner::Child(id) => {
                        if self.is_fluid_pair(id)?
                            && self.pairs.children[&id].closest.status()
                                != ClosestFeatureStatus::Separated
                        {
                            self.set_exact(id)?;
                            self.recalculate_child(id, ClosestFeatureMode::Invalid)?;
                            self.update_pair_policy(id, true, EventTimingHint::Immediate)?;
                        } else if matches!(
                            self.pairs.children[&id].residence,
                            Residence::Refining { .. }
                        ) {
                            self.refresh_recursive_pair(id)?;
                        } else {
                            let child = self.pairs.children[&id].clone();
                            let indices = self.pair_bodies(child.bodies)?;
                            let Residence::Movement {
                                listeners,
                                rotation_travel,
                            } = child.residence
                            else {
                                return Err(PairError::MissingListener.into());
                            };
                            let geometry =
                                child.closest.geometry().ok_or(PairError::MissingPair)?;
                            let mut hint = MovementPairHint {
                                gap: geometry.separation,
                                normal: geometry.normal,
                                projection: geometry.core_projection,
                                rotation_travel,
                                selected_first: child.closest.selection().order()[0],
                            };
                            let query = PairRangeQuery {
                                endpoints: std::array::from_fn(|side| PairRangeEndpoint {
                                    core: self.core_projection(indices[side]),
                                    motion: self.retained_motion(indices[side]),
                                    range: self.bodies[indices[side]].movement_range.clock(),
                                }),
                                time: self.time(),
                                timestep: self.config.timestep,
                                intrusion: event.intrusion,
                            };
                            match MovementPairHint::on_range_exceeded(Some(&mut hint), query)
                                .map_err(|_| EnvironmentError::NonFinite)?
                            {
                                PairRangeAction::Renew { distances } => {
                                    for side in 0..2 {
                                        self.range_renew(
                                            child.bodies[side],
                                            listeners[side],
                                            distances[side],
                                        )?;
                                    }
                                    let child = self.pairs.children.get_mut(&id).unwrap();
                                    child.closest.renew_separation(hint.gap, hint.projection)?;
                                    child.residence = Residence::Movement {
                                        listeners,
                                        rotation_travel: hint.rotation_travel,
                                    };
                                }
                                PairRangeAction::Recalculate => {
                                    self.set_exact(id)?;
                                    self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
                                    self.update_pair_policy(id, true, EventTimingHint::Immediate)?;
                                }
                            }
                        }
                    }
                }
                if !budget.delivered(|current| {
                    self.collision_solver.0.as_mut().map_or(0, |solver| {
                        solver.additional_collision_checks_this_tick(current)
                    })
                }) {
                    break;
                }
            }
            let mut resets = Vec::new();
            self.bodies[index]
                .movement_range
                .reset_if_due(|event| resets.push(event))?;
            for reset in resets {
                self.apply_range_reset(reset)?;
            }
        }
        Ok(())
    }
    fn remove_child(&mut self, id: u64) -> Result<(), EnvironmentError> {
        self.leave_fluid_pair(id)?;
        self.clear_recursive_children(id)?;
        self.clear_residence(id)?;
        let child = self
            .pairs
            .children
            .remove(&id)
            .ok_or(PairError::MissingPair)?;
        if let PairOwner::Recursive(parent) = child.owner
            && let Some(state) = child.recursive
            && state.retired_descendants != 0
        {
            let owner = self
                .pairs
                .children
                .get_mut(&parent)
                .and_then(|pair| pair.recursive.as_mut())
                .ok_or(PairError::MissingRecursiveOwner)?;
            owner.retired_descendants = owner
                .retired_descendants
                .checked_add(state.retired_descendants)
                .ok_or(EnvironmentError::ClockOverflow)?;
        }
        Ok(())
    }
    fn is_fluid_pair(&self, id: u64) -> Result<bool, EnvironmentError> {
        let child = self.pairs.children.get(&id).ok_or(PairError::MissingPair)?;
        Ok(child.bodies.iter().any(|core| {
            self.bodies.iter().any(|body| {
                body.core_identity == *core && self.fluids.at_body(body.identity).is_some()
            })
        }))
    }
    fn update_fluid_pair(&mut self, id: u64) -> Result<(), EnvironmentError> {
        let child = self.pairs.children[&id].clone();
        let gap = child
            .closest
            .geometry()
            .map_or(0.0, |geometry| geometry.separation);
        if child.closest.status() == ClosestFeatureStatus::Separated && gap > 0.0 {
            self.leave_fluid_pair(id)?;
            return Ok(());
        }
        let both = child.bodies.iter().all(|core| {
            self.bodies.iter().any(|body| {
                body.core_identity == *core && self.fluids.at_body(body.identity).is_some()
            })
        });
        if both {
            return Ok(());
        }
        let inside = self.fluids.contains_pair(id);
        if !inside {
            self.enter_fluid_pair(id, child.bodies)?;
        }
        if inside && !matches!(child.residence, Residence::Exact) {
            return Ok(());
        }
        let allowance = if child.closest.status() == ClosestFeatureStatus::Separated {
            -gap + 0.1_f32
        } else {
            0.1_f32
        };
        let indices = self.pair_bodies(child.bodies)?;
        let moving = child.bodies.map(|core| {
            self.islands
                .movement(core)
                .is_some_and(crate::CoreMovement::can_collide)
        });
        let tiny = 1.0e-10_f32;
        let distances = if !moving[0] {
            [tiny, allowance]
        } else if !moving[1] {
            [allowance, tiny]
        } else {
            let motion = indices.map(|i| self.retained_motion(i));
            let speeds = motion.map(|m| (m.rotation.surface_speed + m.linear_speed) + tiny);
            let weights = [speeds[0] + 0.1 * speeds[1], speeds[1] + 0.1 * speeds[0]];
            let factor = allowance / (weights[0] + weights[1]);
            weights.map(|v| v * factor)
        };
        self.clear_residence(id)?;
        let mut listeners = [0; 2];
        for side in 0..2 {
            let key = self.bodies[indices[side]].movement_range.clock().next + distances[side];
            let listener = self.pairs.listener(RangeOwner::Child(id))?;
            self.bodies[indices[side]]
                .movement_range
                .insert(listener, key)?;
            listeners[side] = listener;
        }
        let rotation = self.bodies[indices[0]]
            .movement_range
            .clock()
            .rotation_travel(self.time())?
            + self.bodies[indices[1]]
                .movement_range
                .clock()
                .rotation_travel(self.time())?;
        self.pairs.children.get_mut(&id).unwrap().residence = Residence::Movement {
            listeners,
            rotation_travel: rotation,
        };
        Ok(())
    }
    pub(super) fn retire_impact_pair(&mut self, id: u64) -> Result<(), EnvironmentError> {
        if self.retired_impact_pairs.len() >= self.config.max_events {
            return Err(EnvironmentError::ObservationLimit);
        }
        let child = self
            .pairs
            .children
            .get(&id)
            .ok_or(PairError::MissingPair)?
            .clone();
        self.remove_child(id)?;
        let (hulls, children) = self
            .pairs
            .roster_mut(child.owner)
            .ok_or(PairError::MissingPair)?;
        if !hulls.remove(child.hulls) || children.remove(&child.hulls) != Some(id) {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        if let PairOwner::Recursive(parent) = child.owner {
            let owner = self
                .pairs
                .children
                .get_mut(&parent)
                .and_then(|pair| pair.recursive.as_mut())
                .ok_or(PairError::MissingRecursiveOwner)?;
            owner.retired_descendants = owner
                .retired_descendants
                .checked_add(1)
                .ok_or(EnvironmentError::ClockOverflow)?;
        }
        self.statistics.retired_impact_pairs = self
            .statistics
            .retired_impact_pairs
            .checked_add(1)
            .ok_or(EnvironmentError::ClockOverflow)?;
        self.retired_impact_pairs.push(RetiredImpactPair {
            identity: id,
            bodies: [
                self.bodies[self.core_body_index(child.bodies[0])?].identity,
                self.bodies[self.core_body_index(child.bodies[1])?].identity,
            ],
        });
        Ok(())
    }
    fn remove_parent(&mut self, id: [u64; 2]) -> Result<(), EnvironmentError> {
        let parent = self
            .pairs
            .parents
            .remove(&id)
            .ok_or(PairError::MissingPair)?;
        for hull in parent.hulls.pairs().iter().rev() {
            self.remove_child(parent.children[hull])?;
        }
        for side in (0..2).rev() {
            self.range_remove(parent.bodies[side], parent.listeners[side])?;
        }
        Ok(())
    }
    pub(super) fn remove_spatial_body(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let changes = self.pairs.graph.remove(core)?;
        self.apply_object_pair_changes(changes)?;
        self.pairs.spatial.remove(core);
        if let Some(id) = self.pairs.world_listeners.remove(&core) {
            self.range_remove(core, id)?;
        }
        self.pairs.invalid.remove(&core);
        Ok(())
    }
    fn cancel_child_event(&mut self, id: u64) -> Result<(), EnvironmentError> {
        if self
            .pairs
            .children
            .get_mut(&id)
            .ok_or(PairError::MissingPair)?
            .event
            .take()
            .is_some()
        {
            self.queue.remove(EnvironmentEvent::Convex(id))?;
        }
        Ok(())
    }
    pub(super) fn convex_event(
        &mut self,
        id: u64,
    ) -> Result<Option<QueuedCollision>, EnvironmentError> {
        let child = self.pairs.children.get(&id).ok_or(PairError::MissingPair)?;
        let predicted = child.event.ok_or(PairError::MissingPair)?;
        self.pairs.children.get_mut(&id).unwrap().event = None;
        self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
        let child = &self.pairs.children[&id];
        if child.closest.status() != ClosestFeatureStatus::Separated {
            return Ok(None);
        }
        let threshold =
            self.tolerances.collision_distance + self.tolerances.feature_change_distance;
        if predicted.kind != EventTimingKind::Collision
            || child
                .closest
                .geometry()
                .ok_or(PairError::MissingPair)?
                .separation
                >= threshold
        {
            let hint = if predicted.kind == EventTimingKind::Collision {
                EventTimingHint::LongDelay
            } else {
                EventTimingHint::ShortDelay
            };
            self.update_pair_policy(id, false, hint)?;
            return Ok(None);
        }
        if child.recursive.is_some()
            && self.refine_recursive_pair(id, crate::RecursiveHullEvent::Collision)?
        {
            return Ok(None);
        }
        let child = &self.pairs.children[&id];
        let endpoints = self.child_endpoints(child)?;
        let selection = child.closest.selection();
        let pair = ConvexContactPair {
            endpoints: selection.order().map(|side| endpoints[side]),
            seed: selection.ordered_pair(),
            start: self.time(),
            end: self.clock.next_boundary(),
            maximum_feature_transitions: 1024,
        };
        Ok(Some(QueuedCollision {
            identity: id,
            input: QueuedCollisionInput::Pair { pair, predicted },
        }))
    }
    pub(super) fn after_convex_event(&mut self, id: u64) -> Result<(), EnvironmentError> {
        if !self.pairs.children.contains_key(&id) {
            return Ok(());
        }
        self.recalculate_child(id, ClosestFeatureMode::Ordinary)?;
        self.update_pair_policy(id, false, EventTimingHint::LongDelay)
    }
    pub(super) fn refresh_impact_pairs(&mut self, cores: &[u64]) -> Result<(), EnvironmentError> {
        let mut visited = std::collections::BTreeSet::new();
        for core in cores.iter().rev() {
            let ids = self
                .pairs
                .exact
                .iter()
                .filter(|id| self.pairs.children[id].bodies.contains(core))
                .copied()
                .collect::<Vec<_>>();
            for id in ids {
                if visited.insert(id) {
                    self.after_convex_event(id)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn impact_retirement_releases_scheduled_child_but_keeps_parent_for_rediscovery() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        let core = world.body(1).unwrap().core_identity();
        world.recheck_spatial(core).unwrap();
        let before = world.snapshot();
        let id = *world.pairs.children.keys().next().unwrap();
        let owner = world.pairs.children[&id].owner;
        world.retire_impact_pair(id).unwrap();
        assert!(!world.pairs.children.contains_key(&id));
        let (hulls, children) = world.pairs.roster(owner).unwrap();
        assert!(hulls.pairs().is_empty() && children.is_empty());
        assert_eq!(world.statistics.retired_impact_pairs, 1);
        let retired = world.snapshot();
        world.restore(retired.clone()).unwrap();
        world.recheck_spatial(core).unwrap();
        assert!(world.pairs.children.keys().all(|created| *created > id));
        world.restore(before).unwrap();
        world.retire_impact_pair(id).unwrap();
        assert_eq!(world.snapshot(), retired);
    }
    use super::*;
    #[test]
    fn malformed_pair_snapshots_are_rejected_atomically() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        let core = world.body(1).unwrap().core_identity();
        world.recheck_spatial(core).unwrap();
        let expected = world.snapshot();
        assert!(!expected.pairs.children.is_empty());
        world.restore(expected.clone()).unwrap();
        let mut bad = expected.clone();
        bad.pairs.exact.push(bad.pairs.exact[0]);
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
        let mut bad = expected.clone();
        bad.pairs.parents.values_mut().next().unwrap().listeners[0] = u64::MAX;
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
        let mut bad = expected.clone();
        bad.pairs.world_listeners.clear();
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
    }
}
