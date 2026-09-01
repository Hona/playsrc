use super::*;
use crate::{
    ContactResponseMass, ContactSurface, ManifoldContact, OwnerId, OwnerSlots, OwnershipError,
    TangentBody, TangentEnergyTracker,
};
use std::collections::BTreeMap;
mod impact;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactError {
    Missing,
    Capacity,
    Ownership(OwnershipError),
}
impl fmt::Display for ContactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("retained contact or group is absent"),
            Self::Capacity => f.write_str("retained contact group capacity exhausted"),
            Self::Ownership(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ContactError {}
impl From<ContactError> for EnvironmentError {
    fn from(e: ContactError) -> Self {
        Self::Contacts(e)
    }
}
impl From<OwnershipError> for EnvironmentError {
    fn from(e: OwnershipError) -> Self {
        ContactError::Ownership(e).into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Refresh,
    Tangent,
    Normal,
}
#[derive(Clone, Debug, PartialEq)]
struct RetainedContact {
    normal_jacobians: [Option<[f32; 3]>; 2],
    owner: OwnerId,
    endpoints: [BodyConvex; 2],
    cores: [u64; 2],
    group: u64,
    contact: ManifoldContact,
    surface: ContactSurface,
    friction: f32,
    normal_history: i16,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrictionContact {
    pub contact: u64,
    pub bodies: [u64; 2],
    pub materials: [u32; 2],
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub normal_force: f32,
    pub absorbed_energy: f32,
    pub friction: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrictionEvent {
    pub bodies: [u64; 2],
    pub materials: [u32; 2],
    pub energy: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactOwnerState {
    pub contact: u64,
    pub owner: u64,
    pub endpoints: [BodyConvex; 2],
    pub state: ManifoldContact,
    pub surface: ContactSurface,
    pub normal_history: i16,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ContactBank {
    pub bodies: [u64; 2],
    pub contacts: Vec<ContactOwnerState>,
}
#[derive(Clone, Debug, PartialEq)]
struct ContactGroup {
    cores: Vec<u64>,
    contacts: Vec<u64>,
    controllers: [u64; 3],
    pairs: Vec<CoreContactPair>,
    connectivity_dirty: bool,
}
#[derive(Clone, Debug, PartialEq)]
struct CoreContactPair {
    cores: [u64; 2],
    contacts: Vec<u64>,
    energy: f32,
    redistribution_in: u32,
    last_impact_time: f64,
}
struct ContactPartition {
    selected: u64,
    roots: BTreeMap<u64, u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ContactGroups {
    groups: BTreeMap<u64, ContactGroup>,
    contacts: BTreeMap<u64, RetainedContact>,
    body_contacts: BTreeMap<u64, Vec<u64>>,
    body_groups: BTreeMap<u64, Vec<u64>>,
    controllers: BTreeMap<u64, (u64, Phase)>,
    slots: OwnerSlots,
    next_contact: u64,
    next_group: u64,
    maximum: usize,
}
impl ContactGroups {
    pub(super) fn validate(&self, snapshot: &EnvironmentSnapshot) -> Result<(), EnvironmentError> {
        let invalid = || EnvironmentError::SnapshotMismatch;
        if self.maximum != snapshot.config.max_events
            || self.contacts.len() > self.maximum
            || self.groups.len() > self.maximum
            || self.slots.capacity() != self.maximum
            || self.slots.active_count() != self.contacts.len()
            || self.next_contact == 0
            || self.next_group == 0
            || snapshot.next_controller < 3
            || self.controllers.len() != self.groups.len() * 3
        {
            return Err(invalid());
        }
        let body = |core: u64| {
            snapshot
                .bodies
                .iter()
                .find(|body| body.core_identity == core)
        };
        for (&id, group) in &self.groups {
            if id == 0
                || id >= self.next_group
                || group.contacts.is_empty()
                || group.cores.is_empty()
                || group
                    .cores
                    .iter()
                    .enumerate()
                    .any(|(i, core)| group.cores[..i].contains(core) || body(*core).is_none())
            {
                return Err(invalid());
            }
            let movable = group
                .cores
                .iter()
                .filter(|core| body(**core).unwrap().kind == BodyKind::Dynamic)
                .copied()
                .collect::<Vec<_>>();
            for ((controller, phase), priority) in group
                .controllers
                .into_iter()
                .zip([Phase::Normal, Phase::Tangent, Phase::Refresh])
                .zip([0, 600, 2000])
            {
                if controller >= snapshot.next_controller
                    || self.controllers.get(&controller) != Some(&(id, phase))
                    || !snapshot
                        .islands
                        .controller(controller)
                        .is_some_and(|value| {
                            value.priority == priority && value.associated == movable
                        })
                    || movable.iter().any(|core| {
                        snapshot
                            .islands
                            .core_controllers(*core)
                            .is_none_or(|controllers| {
                                controllers
                                    .iter()
                                    .filter(|value| **value == controller)
                                    .count()
                                    != 1
                            })
                    })
                {
                    return Err(invalid());
                }
            }
            for core in &group.cores {
                if !self
                    .body_groups
                    .get(core)
                    .is_some_and(|values| values.iter().filter(|value| **value == id).count() == 1)
                    || !group.contacts.iter().any(|contact| {
                        self.contacts
                            .get(contact)
                            .is_some_and(|contact| contact.cores.contains(core))
                    })
                {
                    return Err(invalid());
                }
            }
            let mut owned = Vec::new();
            for pair in &group.pairs {
                if pair.contacts.is_empty()
                    || pair.cores[0] == pair.cores[1]
                    || pair.cores.iter().any(|core| !group.cores.contains(core))
                    || !pair.energy.is_finite()
                    || pair.energy < 0.0
                {
                    return Err(invalid());
                }
                for contact in &pair.contacts {
                    if owned.contains(contact)
                        || !self.contacts.get(contact).is_some_and(|contact| {
                            contact.group == id
                                && (contact.cores == pair.cores
                                    || contact.cores == [pair.cores[1], pair.cores[0]])
                        })
                    {
                        return Err(invalid());
                    }
                    owned.push(*contact);
                }
            }
            if owned.len() != group.contacts.len()
                || group.contacts.iter().enumerate().any(|(i, contact)| {
                    group.contacts[..i].contains(contact) || !owned.contains(contact)
                })
            {
                return Err(invalid());
            }
        }
        for (&id, contact) in &self.contacts {
            if id == 0
                || id >= self.next_contact
                || self.slots.owner(contact.owner.index()).ok() != Some(contact.owner)
                || contact.contact.id != contact.owner.index() as u64
                || !self
                    .groups
                    .get(&contact.group)
                    .is_some_and(|group| group.contacts.contains(&id))
            {
                return Err(invalid());
            }
            let state = contact.contact;
            if state.previous_point.iter().any(|v| !v.is_finite())
                || state
                    .frame
                    .first
                    .iter()
                    .chain(&state.frame.second)
                    .chain(&state.local_offset)
                    .chain(state.synchronized_offsets.iter().flatten())
                    .chain(&state.retained)
                    .chain(contact.surface.normal.iter())
                    .chain(&contact.surface.tangent)
                    .chain(contact.normal_jacobians.iter().flatten().flatten())
                    .any(|v| !v.is_finite())
                || contact.surface.point.iter().any(|v| !v.is_finite())
                || !contact.surface.distance.is_finite()
                || !state.last_update_time.is_finite()
                || !state.normal_force.is_finite()
                || state.normal_force < 0.0
                || !state.absorbed_energy.is_finite()
                || state.absorbed_energy < 0.0
                || !state.response_coefficient.is_finite()
                || state.response_coefficient < 0.0
                || !contact.friction.is_finite()
                || !(0.0..=1.0).contains(&contact.friction)
            {
                return Err(invalid());
            }
            let features = state.binding.features();
            let features = [features.first, features.second];
            for (side, feature) in features.into_iter().enumerate() {
                let endpoint = body(contact.cores[side]).ok_or_else(invalid)?;
                if endpoint.identity != contact.endpoints[side].body
                    || contact.normal_jacobians[side].is_some()
                        != (endpoint.kind != BodyKind::Static)
                    || !self
                        .body_contacts
                        .get(&endpoint.core_identity)
                        .is_some_and(|ids| ids.iter().filter(|value| **value == id).count() == 1)
                {
                    return Err(invalid());
                }
                endpoint
                    .topology(contact.endpoints[side].convex)
                    .ok_or_else(invalid)?
                    .edge(feature.edge)?;
            }
        }
        for (core, ids) in &self.body_contacts {
            if body(*core).is_none()
                || ids.iter().any(|id| {
                    !self
                        .contacts
                        .get(id)
                        .is_some_and(|contact| contact.cores.contains(core))
                })
            {
                return Err(invalid());
            }
        }
        for (core, ids) in &self.body_groups {
            if body(*core).is_none()
                || ids.iter().any(|id| {
                    !self
                        .groups
                        .get(id)
                        .is_some_and(|group| group.cores.contains(core))
                })
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub(super) fn new(maximum: usize) -> Self {
        Self {
            groups: BTreeMap::new(),
            contacts: BTreeMap::new(),
            body_contacts: BTreeMap::new(),
            body_groups: BTreeMap::new(),
            controllers: BTreeMap::new(),
            slots: OwnerSlots::new(maximum),
            next_contact: 1,
            next_group: 1,
            maximum,
        }
    }
}

impl PhysicsEnvironment {
    pub(super) fn shift_contact_controller_time(
        &mut self,
        controller: u64,
        shift: f64,
    ) -> Result<(), EnvironmentError> {
        if let Some(&(group, Phase::Tangent)) = self.contacts.controllers.get(&controller) {
            for pair in self
                .contacts
                .groups
                .get(&group)
                .ok_or(ContactError::Missing)?
                .pairs
                .iter()
                .rev()
            {
                for id in pair.contacts.iter().rev() {
                    self.contacts
                        .contacts
                        .get_mut(id)
                        .ok_or(ContactError::Missing)?
                        .contact
                        .last_update_time -= shift;
                }
            }
        }
        Ok(())
    }
    fn finish_normal_observation(&mut self, cores: &[u64]) -> Result<(), EnvironmentError> {
        if self.normal_observations.is_none() {
            return Ok(());
        }
        let after = cores
            .iter()
            .map(|core| {
                self.core_body_index(*core).map(|index| {
                    (
                        *core,
                        self.bodies[index].velocity,
                        self.bodies[index].queued_velocity,
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.normal_observations
            .as_mut()
            .unwrap()
            .last_mut()
            .ok_or(ContactError::Missing)?
            .after = after;
        Ok(())
    }
    fn contact_normal_jacobians(
        bodies: [Option<TangentBody>; 2],
        offsets: [[f32; 3]; 2],
        normal: [f32; 3],
        distance: f32,
    ) -> Result<[Option<[f32; 3]>; 2], EnvironmentError> {
        let mut jacobians = [None; 2];
        for (side, body) in bodies.into_iter().enumerate() {
            if let Some(body) = body {
                jacobians[side] = Some(
                    crate::ContactNormalRow::from_local(
                        normal,
                        offsets[side],
                        body.orientation,
                        distance,
                        if side == 0 {
                            crate::DynamicEndpoint::First
                        } else {
                            crate::DynamicEndpoint::Second
                        },
                    )?
                    .angular_jacobian,
                );
            }
        }
        Ok(jacobians)
    }
    pub(super) fn recheck_contact_points_inner(
        &mut self,
        identity: u64,
    ) -> Result<(), EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        let core = body.core_identity;
        let mut rejected = Vec::new();
        for id in self.contacts.body_contacts.get(&core).into_iter().flatten() {
            let contact = &self.contacts.contacts[id];
            let peer = contact.endpoints[usize::from(contact.cores[0] == core)].body;
            if !game_collision_allowed(
                &self.collision_solver,
                body,
                self.body(peer).ok_or(EnvironmentError::MissingBody)?,
            ) {
                rejected.push(peer);
            }
        }
        for peer in rejected {
            self.wake(peer)?;
            let contacts = self
                .contacts
                .body_contacts
                .get(&core)
                .cloned()
                .unwrap_or_default();
            for id in contacts {
                if self.contacts.contacts[&id]
                    .endpoints
                    .iter()
                    .any(|endpoint| endpoint.body == peer)
                {
                    self.remove_contact(id)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn has_contact_group(&self, core: u64) -> bool {
        self.contacts
            .body_groups
            .get(&core)
            .is_some_and(|groups| !groups.is_empty())
    }
    pub(super) fn contact_count(&self) -> usize {
        self.contacts.contacts.len()
    }
    pub(super) fn contact_impact_elapsed(&mut self, id: u64) -> Result<f32, EnvironmentError> {
        let contact = self
            .contacts
            .contacts
            .get(&id)
            .ok_or(ContactError::Missing)?;
        let group = contact.group;
        let cores = contact.cores;
        let now = self.time();
        let pair = self
            .contacts
            .groups
            .get_mut(&group)
            .ok_or(ContactError::Missing)?
            .pairs
            .iter_mut()
            .find(|pair| pair.cores == cores || pair.cores == [cores[1], cores[0]])
            .ok_or(ContactError::Missing)?;
        let elapsed = (now - pair.last_impact_time) as f32;
        pair.last_impact_time = now;
        Ok(elapsed)
    }
    fn observe_tangent(
        &mut self,
        id: u64,
        before: crate::ManifoldContact,
        bodies: [Option<TangentBody>; 2],
        result: crate::ManifoldTangentResult,
    ) -> Result<(), EnvironmentError> {
        let time = self.time();
        if let Some(observations) = &mut self.tangent_observations {
            if observations.len() == self.config.max_events {
                return Err(ContactError::Capacity.into());
            }
            observations.push(TangentObservation {
                contact: id,
                response_coefficient: before.response_coefficient,
                time,
                before,
                bodies,
                result,
            });
        }
        Ok(())
    }
    pub fn contact_banks(&self, body: u64) -> Result<Vec<ContactBank>, EnvironmentError> {
        let core = self
            .body(body)
            .ok_or(EnvironmentError::MissingBody)?
            .core_identity;
        let mut output = Vec::new();
        for group in self.contacts.body_groups.get(&core).into_iter().flatten() {
            for pair in &self.contacts.groups[group].pairs {
                if !pair.cores.contains(&core) {
                    continue;
                }
                let bodies = pair
                    .cores
                    .map(|core| self.bodies[self.core_body_index(core).unwrap()].identity);
                let contacts = pair
                    .contacts
                    .iter()
                    .map(|id| {
                        let contact = &self.contacts.contacts[id];
                        ContactOwnerState {
                            contact: *id,
                            owner: contact.owner.index() as u64,
                            endpoints: contact.endpoints,
                            state: contact.contact,
                            surface: contact.surface,
                            normal_history: contact.normal_history,
                        }
                    })
                    .collect();
                output.push(ContactBank { bodies, contacts });
            }
        }
        Ok(output)
    }
    pub fn friction_events(&self) -> &[FrictionEvent] {
        &self.friction_events
    }
    pub(super) fn report_friction(&mut self) -> Result<(), EnvironmentError> {
        let now = self.clock.last_boundary() as f32;
        let elapsed = now - self.last_friction_time;
        if elapsed < self.config.timestep {
            return Ok(());
        }
        let inverse = if elapsed != 0.0 {
            1.0_f32 / elapsed
        } else {
            0.0
        };
        self.last_friction_time = now;
        for core in self.active_objects.clone() {
            let index = self.core_body_index(core)?;
            if self.bodies[index].callback_flags & 2 == 0 {
                continue;
            }
            let ids = self
                .contacts
                .body_contacts
                .get(&core)
                .cloned()
                .unwrap_or_default();
            for id in ids {
                let retained = &self.contacts.contacts[&id];
                let side = usize::from(retained.cores[0] != core);
                let peer = self.core_body_index(retained.cores[1 - side])?;
                if self.bodies[peer].callback_flags & 2 == 0
                    || retained.contact.absorbed_energy == 0.0
                {
                    continue;
                }
                let inverse_mass = if self.bodies[index].motion_enabled {
                    1.0 / self.bodies[index].physical.mass
                } else {
                    0.0
                };
                let energy = (retained.contact.absorbed_energy * inverse) * inverse_mass;
                if energy > 0.05_f32 {
                    let feature = if side == 0 {
                        retained.contact.binding.features().second
                    } else {
                        retained.contact.binding.features().first
                    };
                    let topology = self.bodies[peer]
                        .topology(retained.endpoints[1 - side].convex)
                        .ok_or(ContactError::Missing)?;
                    let encoded = (topology.face_metadata(feature.edge)? >> 24) & 0x7f;
                    let hit_material = if encoded != 0 {
                        u32::from(self.world_materials[encoded as usize])
                    } else {
                        self.bodies[peer].material
                    };
                    let event = FrictionEvent {
                        bodies: [self.bodies[index].identity, self.bodies[peer].identity],
                        materials: [self.bodies[index].material, hit_material],
                        energy: energy
                            * (crate::units::INCHES_PER_METER * crate::units::INCHES_PER_METER),
                    };
                    let point = retained.contact.previous_point;
                    let normal = if side == 0 {
                        retained.surface.normal
                    } else {
                        retained.surface.normal.map(|v| -v)
                    };
                    if self.event_reporting {
                        self.friction_events.push(event);
                        self.emit_callback(
                            PhysicsCallbackKind::Friction {
                                energy: event.energy,
                                materials: event.materials,
                            },
                            [Some(event.bodies[0]), None],
                            Some(PhysicsContactData {
                                point: source_position(point.map(|v| f64::from(v as f32))),
                                normal: source_direction(normal, 1.0),
                                velocity: None,
                            }),
                        )?;
                    }
                }
                self.contacts
                    .contacts
                    .get_mut(&id)
                    .unwrap()
                    .contact
                    .absorbed_energy = 0.0;
            }
        }
        Ok(())
    }
    pub fn friction_contacts(&self, body: u64) -> Result<Vec<FrictionContact>, EnvironmentError> {
        let core = self
            .body(body)
            .ok_or(EnvironmentError::MissingBody)?
            .core_identity;
        self.contacts
            .body_contacts
            .get(&core)
            .into_iter()
            .flatten()
            .map(move |id| {
                let retained = &self.contacts.contacts[id];
                let side = usize::from(retained.cores[0] != core);
                let order = [side, 1 - side];
                let bodies = order.map(|i| retained.endpoints[i].body);
                let materials = self
                    .contact_materials(retained.endpoints, retained.contact.binding.features())?;
                let materials = order.map(|i| materials[i]);
                let normal = if side == 0 {
                    retained.surface.normal
                } else {
                    retained.surface.normal.map(|v| -v)
                };
                Ok(FrictionContact {
                    contact: *id,
                    bodies,
                    materials,
                    point: crate::units::source_direction(
                        retained.contact.previous_point.map(|value| value as f32),
                        crate::units::INCHES_PER_METER,
                    ),
                    normal: crate::normalize_source_vector(crate::units::source_direction(
                        normal, 1.0,
                    ))?,
                    normal_force: retained.contact.source_normal_force(),
                    absorbed_energy: retained.contact.absorbed_energy
                        * (crate::units::INCHES_PER_METER * crate::units::INCHES_PER_METER),
                    friction: retained.friction,
                })
            })
            .collect()
    }
    /// Clears velocity and, in snapshot order, neutralizes and recomputes each
    /// retained contact. This is the physical operation used by the SDK's
    /// `PhysForceClearVelocity`; sleeping remains a separate caller decision.
    // The orchestration of this operation is adapted from Source SDK 2013.
    // Copyright Valve Corporation, All rights reserved. See the repository's
    // LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt.
    pub fn clear_velocity_and_contact_strain(&mut self, body: u64) -> Result<(), EnvironmentError> {
        let core = self
            .body(body)
            .ok_or(EnvironmentError::MissingBody)?
            .core_identity;
        let contacts = self
            .contacts
            .body_contacts
            .get(&core)
            .cloned()
            .unwrap_or_default();
        let mut candidate = self.clone();
        candidate.set_velocity(body, Some([0.0; 3]), Some([0.0; 3]))?;
        for contact in contacts {
            candidate
                .contacts
                .contacts
                .get_mut(&contact)
                .ok_or(ContactError::Missing)?
                .contact
                .retained = [0.0; 2];
            candidate.refresh_contact(contact)?;
        }
        *self = candidate;
        Ok(())
    }
    fn contact_bodies(&self, endpoints: [BodyConvex; 2]) -> Result<[usize; 2], EnvironmentError> {
        Ok([
            self.bodies
                .iter()
                .position(|b| b.identity == endpoints[0].body)
                .ok_or(EnvironmentError::MissingBody)?,
            self.bodies
                .iter()
                .position(|b| b.identity == endpoints[1].body)
                .ok_or(EnvironmentError::MissingBody)?,
        ])
    }
    fn contact_body_states(&self, indices: [usize; 2]) -> [Option<TangentBody>; 2] {
        indices.map(|index| {
            let body = &self.bodies[index];
            if body.kind == BodyKind::Static {
                return None;
            }
            let frame = body.integration_frame();
            Some(TangentBody {
                position: frame.position,
                orientation: frame.orientation,
                linear_velocity: body.velocity.linear,
                angular_velocity: body.velocity.angular,
                inverse_mass: frame.inverse_mass,
                inverse_inertia: frame.inverse_inertia,
            })
        })
    }
    fn contact_equivalent(
        &self,
        contact: &RetainedContact,
        geometry: ContactGeometry,
    ) -> Result<bool, EnvironmentError> {
        let old = contact.contact.binding.features();
        let old = [old.first, old.second];
        let new = geometry.binding.features();
        let new = [new.first, new.second];
        let order = if contact.endpoints == geometry.endpoints {
            [0, 1]
        } else if contact.endpoints == [geometry.endpoints[1], geometry.endpoints[0]] {
            [1, 0]
        } else {
            return Ok(false);
        };
        for side in 0..2 {
            let a = old[side];
            let b = new[order[side]];
            if a.kind != b.kind {
                return Ok(false);
            }
            let topology = self
                .body(contact.endpoints[side].body)
                .ok_or(EnvironmentError::MissingBody)?
                .topology(contact.endpoints[side].convex)
                .ok_or(ContactError::Missing)?;
            let same = match a.kind {
                crate::SurfaceFeatureKind::Vertex => {
                    topology.edge(a.edge)?.start == topology.edge(b.edge)?.start
                }
                crate::SurfaceFeatureKind::Face => {
                    topology.edge(a.edge)?.face == topology.edge(b.edge)?.face
                }
                crate::SurfaceFeatureKind::Edge => {
                    a.edge == b.edge || topology.edge(a.edge)?.opposite == b.edge
                }
                crate::SurfaceFeatureKind::InteriorFace => false,
            };
            if !same {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(super) fn admit_contact(
        &mut self,
        geometry: ContactGeometry,
        preferred_core: u64,
    ) -> Result<u64, EnvironmentError> {
        let indices = self.contact_bodies(geometry.endpoints)?;
        let cores = indices.map(|i| self.bodies[i].core_identity);
        if let Some(contacts) = self.contacts.body_contacts.get(&cores[0]) {
            for id in contacts {
                if self.contact_equivalent(&self.contacts.contacts[id], geometry)? {
                    let id = *id;
                    self.refresh_contact(id)?;
                    return Ok(id);
                }
            }
        }
        if self.contacts.contacts.len() >= self.contacts.maximum {
            return Err(ContactError::Capacity.into());
        }
        self.emit_touch_callback(geometry.endpoints, geometry.surface, true)?;
        let dynamic = usize::from(self.bodies[indices[0]].kind == BodyKind::Static);
        let peer = 1 - dynamic;
        let group = self
            .contacts
            .body_groups
            .get(&cores[dynamic])
            .and_then(|g| g.first())
            .copied()
            .or_else(|| {
                if self.bodies[indices[peer]].kind == BodyKind::Static {
                    None
                } else {
                    self.contacts
                        .body_groups
                        .get(&cores[peer])
                        .and_then(|g| g.first())
                        .copied()
                }
            });
        let group = if let Some(group) = group {
            group
        } else {
            self.create_contact_group()?
        };
        for core in cores {
            if !self
                .islands
                .is_immovable(core)
                .ok_or(crate::IslandError::MissingCore)?
                && let Some(donor) = self
                    .contacts
                    .body_groups
                    .get(&core)
                    .and_then(|groups| groups.iter().find(|id| **id != group))
                    .copied()
            {
                self.merge_contact_groups(group, donor)?;
            }
        }
        for core in [cores[dynamic], cores[peer]] {
            if self.contacts.groups[&group].cores.contains(&core) {
                continue;
            }
            let index = self.core_body_index(core)?;
            self.contacts
                .groups
                .get_mut(&group)
                .unwrap()
                .cores
                .push(core);
            self.contacts
                .body_groups
                .entry(core)
                .or_default()
                .push(group);
            if self.bodies[index].kind != BodyKind::Static {
                for controller in self.contacts.groups[&group].controllers {
                    self.islands.attach(core, controller)?;
                }
            }
        }
        if self.bodies[indices[peer]].kind != BodyKind::Static
            && self.islands.island_of(cores[dynamic]) != self.islands.island_of(cores[peer])
        {
            let first = self
                .islands
                .island_of(cores[dynamic])
                .ok_or(crate::IslandError::MissingCore)?;
            let second = self
                .islands
                .island_of(cores[peer])
                .ok_or(crate::IslandError::MissingCore)?;
            let preferred = self
                .islands
                .island_of(preferred_core)
                .ok_or(crate::IslandError::MissingCore)?;
            if preferred == second {
                self.islands.join(second, first)?;
            } else if preferred == first {
                self.islands.join(first, second)?;
            } else {
                return Err(crate::IslandError::InvalidAssociation.into());
            }
        }
        self.update_group_associations(group)?;
        let id = self.contacts.next_contact;
        self.contacts.next_contact = id.checked_add(1).ok_or(ContactError::Capacity)?;
        let owner = self.contacts.slots.allocate()?;
        let states = self.contact_body_states(indices);
        let response = ManifoldContact::response_coefficient(std::array::from_fn(|side| {
            states[side]
                .filter(|_| self.bodies[indices[side]].motion_enabled)
                .map(|body| ContactResponseMass {
                    local_offset: geometry.synchronized_offsets[side],
                    inverse_mass: body.inverse_mass,
                    inverse_inertia: body.inverse_inertia,
                })
        }))?;
        let pair = SurfacePair::from_registry(&self.surfaces, geometry.materials)
            .map_err(|_| EnvironmentError::DependencyMismatch)?;
        let contact = ManifoldContact {
            id: owner.index() as u64,
            binding: geometry.binding,
            frame: geometry.surface.frame()?,
            local_offset: geometry.synchronized_offsets[0],
            synchronized_offsets: geometry.synchronized_offsets,
            response_coefficient: response,
            previous_point: geometry.surface.point,
            retained: [0.0; 2],
            energy: TangentEnergyTracker::default(),
            last_update_time: self.time(),
            normal_force: 0.0,
            absorbed_energy: 0.0,
        };
        self.contacts.contacts.insert(
            id,
            RetainedContact {
                normal_jacobians: Self::contact_normal_jacobians(
                    states,
                    geometry.synchronized_offsets,
                    geometry.surface.normal,
                    geometry.surface.distance,
                )?,
                owner,
                endpoints: geometry.endpoints,
                cores,
                group,
                contact,
                surface: geometry.surface,
                friction: pair.friction,
                normal_history: 0,
            },
        );
        for core in cores {
            self.contacts
                .body_contacts
                .entry(core)
                .or_default()
                .insert(0, id);
        }
        let group = self.contacts.groups.get_mut(&group).unwrap();
        group.contacts.insert(0, id);
        if let Some(pair) = group
            .pairs
            .iter_mut()
            .find(|pair| pair.cores == cores || pair.cores == [cores[1], cores[0]])
        {
            pair.contacts.push(id);
        } else {
            group.pairs.push(CoreContactPair {
                cores,
                contacts: vec![id],
                energy: 0.0,
                redistribution_in: 1,
                last_impact_time: self
                    .callbacks
                    .admit_pair_clock(storage::pair_keys(&self.bodies, cores)?),
            });
        }
        Ok(id)
    }
    fn create_contact_group(&mut self) -> Result<u64, EnvironmentError> {
        let group = self.contacts.next_group;
        self.contacts.next_group = group.checked_add(1).ok_or(ContactError::Capacity)?;
        let first = self.next_controller;
        self.next_controller = first.checked_add(3).ok_or(ContactError::Capacity)?;
        let controllers = [first, first + 1, first + 2];
        for ((id, phase), priority) in controllers
            .into_iter()
            .zip([Phase::Normal, Phase::Tangent, Phase::Refresh])
            .zip([0, 600, 2000])
        {
            self.islands.register_controller(crate::IslandController {
                identity: id,
                priority,
                associated: Vec::new(),
            })?;
            self.contacts.controllers.insert(id, (group, phase));
        }
        self.contacts.groups.insert(
            group,
            ContactGroup {
                cores: Vec::new(),
                contacts: Vec::new(),
                controllers,
                pairs: Vec::new(),
                connectivity_dirty: false,
            },
        );
        Ok(group)
    }
    fn merge_contact_groups(&mut self, recipient: u64, donor: u64) -> Result<(), EnvironmentError> {
        if recipient == donor {
            return Ok(());
        }
        let mut previous = self
            .contacts
            .groups
            .remove(&donor)
            .ok_or(ContactError::Missing)?;
        for id in previous.contacts {
            let pair = previous
                .pairs
                .iter_mut()
                .find(|pair| pair.contacts.contains(&id))
                .ok_or(ContactError::Missing)?;
            pair.contacts.retain(|contact| *contact != id);
            if pair.contacts.is_empty() {
                self.callbacks.retire_pair_clock(
                    storage::pair_keys(&self.bodies, pair.cores)?,
                    pair.last_impact_time,
                );
            }
            let contact = self
                .contacts
                .contacts
                .get_mut(&id)
                .ok_or(ContactError::Missing)?;
            contact.group = recipient;
            let cores = contact.cores;
            let target = self
                .contacts
                .groups
                .get_mut(&recipient)
                .ok_or(ContactError::Missing)?;
            target.contacts.insert(0, id);
            if let Some(pair) = target
                .pairs
                .iter_mut()
                .find(|pair| pair.cores == cores || pair.cores == [cores[1], cores[0]])
            {
                pair.contacts.push(id);
            } else {
                target.pairs.push(CoreContactPair {
                    cores,
                    contacts: vec![id],
                    energy: 0.0,
                    redistribution_in: 1,
                    last_impact_time: self
                        .callbacks
                        .admit_pair_clock(storage::pair_keys(&self.bodies, cores)?),
                });
            }
        }
        for core in previous.cores.into_iter().rev() {
            self.contacts
                .body_groups
                .get_mut(&core)
                .ok_or(ContactError::Missing)?
                .retain(|id| *id != donor);
            let dynamic = !self
                .islands
                .is_immovable(core)
                .ok_or(crate::IslandError::MissingCore)?;
            if dynamic {
                for controller in previous.controllers.into_iter().rev() {
                    self.islands.detach(core, controller)?;
                }
            }
            if !self.contacts.groups[&recipient].cores.contains(&core) {
                self.contacts
                    .groups
                    .get_mut(&recipient)
                    .unwrap()
                    .cores
                    .push(core);
                self.contacts
                    .body_groups
                    .get_mut(&core)
                    .unwrap()
                    .push(recipient);
                if dynamic {
                    for controller in self.contacts.groups[&recipient].controllers {
                        self.islands.attach(core, controller)?;
                    }
                }
            }
        }
        for controller in previous.controllers {
            self.islands.remove_controller(controller)?;
            self.contacts.controllers.remove(&controller);
        }
        self.update_group_associations(recipient)
    }
    fn update_group_associations(&mut self, group: u64) -> Result<(), EnvironmentError> {
        let group = &self.contacts.groups[&group];
        let movable = group
            .cores
            .iter()
            .filter(|core| !self.islands.is_immovable(**core).unwrap())
            .copied()
            .collect::<Vec<_>>();
        for controller in group.controllers {
            self.islands.set_associated(controller, movable.clone())?;
        }
        Ok(())
    }
    fn group_component_seed(&self, id: u64) -> Result<Option<ContactPartition>, EnvironmentError> {
        let group = self.contacts.groups.get(&id).ok_or(ContactError::Missing)?;
        let mut parents = group
            .cores
            .iter()
            .map(|core| (*core, *core))
            .collect::<BTreeMap<_, _>>();
        let root = |parents: &BTreeMap<u64, u64>, mut core: u64| {
            while parents[&core] != core {
                core = parents[&core];
            }
            core
        };
        for pair in group.pairs.iter().rev() {
            if pair
                .cores
                .iter()
                .any(|core| self.islands.is_immovable(*core) == Some(true))
            {
                continue;
            }
            let first = root(&parents, pair.cores[0]);
            let second = root(&parents, pair.cores[1]);
            if first != second {
                parents.insert(second, first);
            }
        }
        let roots = group
            .cores
            .iter()
            .filter(|core| self.islands.is_immovable(**core) == Some(false))
            .map(|core| (*core, root(&parents, *core)))
            .collect::<Vec<_>>();
        let first = roots.first().ok_or(ContactError::Missing)?.1;
        Ok(roots
            .iter()
            .find(|(_, r)| *r != first)
            .map(|(_, selected)| ContactPartition {
                selected: *selected,
                roots: roots.iter().copied().collect(),
            }))
    }
    fn split_contact_group(&mut self, id: u64) -> Result<(), EnvironmentError> {
        if !self
            .contacts
            .groups
            .get(&id)
            .is_some_and(|group| group.connectivity_dirty)
        {
            return Ok(());
        }
        self.contacts
            .groups
            .get_mut(&id)
            .unwrap()
            .connectivity_dirty = false;
        while let Some(ContactPartition { selected, roots }) = self.group_component_seed(id)? {
            let child = self.create_contact_group()?;
            let previous = self.contacts.groups[&id].cores.clone();
            for core in previous.iter().rev() {
                let fixed = self
                    .islands
                    .is_immovable(*core)
                    .ok_or(crate::IslandError::MissingCore)?;
                if !fixed && roots.get(core) != Some(&selected) {
                    continue;
                }
                if fixed {
                    self.contacts
                        .body_groups
                        .get_mut(core)
                        .ok_or(ContactError::Missing)?
                        .push(child);
                } else {
                    let source = self.contacts.groups.get_mut(&id).unwrap();
                    source.cores.retain(|value| value != core);
                    let controllers = source.controllers;
                    for controller in controllers.into_iter().rev() {
                        self.islands.detach(*core, controller)?;
                    }
                    for controller in self.contacts.groups[&child].controllers {
                        self.islands.attach(*core, controller)?;
                    }
                    let binding = self
                        .contacts
                        .body_groups
                        .get_mut(core)
                        .ok_or(ContactError::Missing)?
                        .iter_mut()
                        .find(|value| **value == id)
                        .ok_or(ContactError::Missing)?;
                    *binding = child;
                }
                self.contacts
                    .groups
                    .get_mut(&child)
                    .unwrap()
                    .cores
                    .push(*core);
            }
            let pair_count = self.contacts.groups[&id].pairs.len();
            for index in (0..pair_count).rev() {
                let pair = &self.contacts.groups[&id].pairs[index];
                let moving = pair
                    .cores
                    .iter()
                    .find(|core| self.islands.is_immovable(**core) == Some(false))
                    .ok_or(ContactError::Missing)?;
                if roots[moving] != selected {
                    continue;
                }
                let pair = self
                    .contacts
                    .groups
                    .get_mut(&id)
                    .unwrap()
                    .pairs
                    .remove(index);
                for contact in pair.contacts.iter().rev() {
                    self.contacts
                        .groups
                        .get_mut(&id)
                        .unwrap()
                        .contacts
                        .retain(|value| value != contact);
                    self.contacts
                        .groups
                        .get_mut(&child)
                        .unwrap()
                        .contacts
                        .insert(0, *contact);
                    self.contacts
                        .contacts
                        .get_mut(contact)
                        .ok_or(ContactError::Missing)?
                        .group = child;
                }
                self.contacts
                    .groups
                    .get_mut(&child)
                    .unwrap()
                    .pairs
                    .push(pair);
            }
            for core in previous
                .iter()
                .rev()
                .filter(|core| self.islands.is_immovable(**core) == Some(true))
            {
                for group in [child, id] {
                    if !self.contacts.groups[&group]
                        .contacts
                        .iter()
                        .any(|contact| self.contacts.contacts[contact].cores.contains(core))
                    {
                        self.contacts
                            .groups
                            .get_mut(&group)
                            .unwrap()
                            .cores
                            .retain(|value| value != core);
                        self.contacts
                            .body_groups
                            .get_mut(core)
                            .unwrap()
                            .retain(|value| *value != group);
                    }
                }
            }
            for group in [child, id] {
                self.update_group_associations(group)?;
            }
            self.islands.request_connectivity_check(selected)?;
        }
        Ok(())
    }
    fn commit_contact_velocities(&mut self, indices: [usize; 2], bodies: [Option<TangentBody>; 2]) {
        for side in 0..2 {
            if let Some(body) = bodies[side] {
                self.bodies[indices[side]].velocity = VelocityState {
                    linear: body.linear_velocity,
                    angular: body.angular_velocity,
                };
            }
        }
    }
    fn refresh_contact(&mut self, id: u64) -> Result<(), EnvironmentError> {
        let mut retained = self
            .contacts
            .contacts
            .get(&id)
            .ok_or(ContactError::Missing)?
            .clone();
        let indices = self.contact_bodies(retained.endpoints)?;
        let bodies = self.contact_body_states(indices);
        let first = self.cached_transform(retained.endpoints[0].body)?.object;
        self.transforms.pin(retained.endpoints[0].body)?;
        let second = self.cached_transform(retained.endpoints[1].body)?.object;
        self.transforms.pin(retained.endpoints[1].body)?;
        let result = retained.contact.refresh(
            bodies,
            self.time(),
            std::array::from_fn(|side| {
                self.bodies[indices[side]]
                    .topology(retained.endpoints[side].convex)
                    .unwrap()
            }),
            [first, second],
            self.tolerances,
        );
        self.transforms.release(retained.endpoints[1].body)?;
        self.transforms.release(retained.endpoints[0].body)?;
        retained.surface = result?;
        retained.normal_jacobians = Self::contact_normal_jacobians(
            bodies,
            retained.contact.synchronized_offsets,
            retained.surface.normal,
            retained.surface.distance,
        )?;
        self.contacts.contacts.insert(id, retained);
        Ok(())
    }
    pub(super) fn refresh_material_contacts(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let contacts = self
            .contacts
            .body_contacts
            .get(&core)
            .cloned()
            .unwrap_or_default();
        for id in contacts {
            self.refresh_contact(id)?;
            let contact = &self.contacts.contacts[&id];
            let indices = self.contact_bodies(contact.endpoints)?;
            let bodies = self.contact_body_states(indices);
            let coefficient = ManifoldContact::response_coefficient(std::array::from_fn(|side| {
                bodies[side]
                    .filter(|_| self.bodies[indices[side]].motion_enabled)
                    .map(|body| ContactResponseMass {
                        local_offset: contact.contact.synchronized_offsets[side],
                        inverse_mass: body.inverse_mass,
                        inverse_inertia: body.inverse_inertia,
                    })
            }))?;
            let materials =
                self.contact_materials(contact.endpoints, contact.contact.binding.features())?;
            let pair = SurfacePair::from_registry(&self.surfaces, materials)
                .map_err(|_| EnvironmentError::DependencyMismatch)?;
            let contact = self.contacts.contacts.get_mut(&id).unwrap();
            contact.friction = pair.friction;
            contact.contact.response_coefficient = coefficient;
        }
        Ok(())
    }
    pub(super) fn contact_controller(&mut self, id: u64) -> Result<bool, EnvironmentError> {
        let Some((group, phase)) = self.contacts.controllers.get(&id).copied() else {
            return Ok(false);
        };
        let contact_id = *self
            .contacts
            .groups
            .get(&group)
            .ok_or(ContactError::Missing)?
            .contacts
            .first()
            .ok_or(ContactError::Missing)?;
        if self.contacts.groups[&group].contacts.len() != 1 {
            self.multiple_contact_controller(group, phase)?;
            return Ok(true);
        }
        let mut retained = self.contacts.contacts[&contact_id].clone();
        let indices = self.contact_bodies(retained.endpoints)?;
        let bodies = self.contact_body_states(indices);
        match phase {
            Phase::Refresh => {
                self.refresh_contact(contact_id)?;
                return Ok(true);
            }
            Phase::Tangent => {
                let step = self.config.timestep;
                let limit = ((step * step) * retained.contact.response_coefficient)
                    * (retained.contact.normal_force * retained.friction);
                retained.contact.clamp_retained(limit, retained.friction)?;
                let before = retained.contact;
                if let Some(result) =
                    retained
                        .contact
                        .solve_tangent(bodies, step, retained.friction)?
                {
                    self.statistics.tangent_solves = self
                        .statistics
                        .tangent_solves
                        .checked_add(1)
                        .ok_or(EnvironmentError::ClockOverflow)?;
                    self.commit_contact_velocities(indices, result.bodies);
                    self.observe_tangent(contact_id, before, bodies, result)?;
                }
            }
            Phase::Normal => {
                let result = retained.contact.solve_normal(
                    bodies,
                    retained.surface.normal,
                    retained.surface.distance,
                    self.tolerances.friction_distance,
                    self.config.timestep.recip(),
                )?;
                for (side, index) in indices.into_iter().enumerate() {
                    if let Some(body) = result.endpoints[side] {
                        self.bodies[index].velocity = VelocityState {
                            linear: body.linear_velocity,
                            angular: body.angular_velocity,
                        };
                    }
                }
                if retained.surface.distance >= self.tolerances.maximum_friction_distance
                    || retained.surface.broken
                {
                    self.remove_contact(contact_id)?;
                    return Ok(true);
                }
            }
        }
        self.contacts.contacts.insert(contact_id, retained);
        if phase == Phase::Normal {
            self.split_contact_group(group)?;
        }
        Ok(true)
    }
    fn multiple_contact_controller(
        &mut self,
        group: u64,
        phase: Phase,
    ) -> Result<(), EnvironmentError> {
        match phase {
            Phase::Refresh => self.refresh_contact_group(group),
            Phase::Tangent => self.solve_group_tangents(group),
            Phase::Normal => {
                self.solve_group_normals(group)?;
                self.split_contact_group(group)
            }
        }
    }
    fn refresh_contact_group(&mut self, group: u64) -> Result<(), EnvironmentError> {
        let pairs = self.contacts.groups[&group].pairs.clone();
        for (index, pair) in pairs.iter().enumerate().rev() {
            let mut gained = 0.0_f32;
            for id in pair.contacts.iter().rev() {
                let previous = &self.contacts.contacts[id];
                let (distance, pressure) =
                    (previous.surface.distance, previous.contact.normal_force);
                self.refresh_contact(*id)?;
                gained += (distance - self.contacts.contacts[id].surface.distance) * pressure;
            }
            if gained > 0.0 {
                self.contacts.groups.get_mut(&group).unwrap().pairs[index].energy += gained;
            }
        }
        let moving = self.contacts.groups[&group]
            .cores
            .iter()
            .find(|core| !self.islands.is_immovable(**core).unwrap())
            .copied()
            .ok_or(ContactError::Missing)?;
        let activity = self
            .islands
            .island(
                self.islands
                    .island_of(moving)
                    .ok_or(crate::IslandError::MissingCore)?,
            )
            .ok_or(crate::IslandError::MissingIsland)?
            .activity;
        if activity.clear_contact_energy {
            for pair in &mut self.contacts.groups.get_mut(&group).unwrap().pairs {
                pair.energy = 0.0;
            }
        }
        if !activity.fast {
            for index in (0..pairs.len()).rev() {
                let pair = &self.contacts.groups[&group].pairs[index];
                let indices = [
                    self.core_body_index(pair.cores[0])?,
                    self.core_body_index(pair.cores[1])?,
                ];
                let result = crate::MutualEnergyInput {
                    endpoints: indices.map(|i| {
                        let body = &self.bodies[i];
                        crate::MutualEnergyEndpoint {
                            linear: body.velocity.linear,
                            angular: body.velocity.angular,
                            orientation: body
                                .collision_orientation
                                .unwrap_or_else(|| body.orientation.matrix()),
                            inertia: body.physical.inertia,
                            mass: body.physical.mass,
                            inverse_mass: 1.0 / body.physical.mass,
                            immovable: body.kind == BodyKind::Static,
                            pending: body.queued_velocity,
                        }
                    }),
                    accumulated: pair.energy,
                    timestep: self.config.timestep,
                }
                .reduce()?;
                for (side, i) in indices.into_iter().enumerate() {
                    self.bodies[i].queued_velocity = result.pending[side];
                }
                self.contacts.groups.get_mut(&group).unwrap().pairs[index].energy =
                    result.remaining;
            }
        }
        Ok(())
    }
    fn solve_group_tangents(&mut self, group: u64) -> Result<(), EnvironmentError> {
        let pairs = self.contacts.groups[&group].pairs.clone();
        for (index, pair) in pairs.iter().enumerate().rev() {
            let owners = pair
                .contacts
                .iter()
                .map(|id| {
                    let contact = &self.contacts.contacts[id];
                    crate::RetainedFrictionOwner {
                        normal_force: contact.contact.normal_force,
                        friction: contact.friction,
                        response_coefficient: contact.contact.response_coefficient,
                    }
                })
                .collect::<Vec<_>>();
            let limit = crate::RetainedFrictionClamp {
                owners: &owners,
                timestep: self.config.timestep,
            }
            .limit()?;
            let mut gained = 0.0_f32;
            for id in pair.contacts.iter().rev() {
                let mut contact = self.contacts.contacts[id].clone();
                let indices = self.contact_bodies(contact.endpoints)?;
                contact.contact.clamp_retained(limit, contact.friction)?;
                let before = contact.contact;
                let bodies = self.contact_body_states(indices);
                if let Some(result) =
                    contact
                        .contact
                        .solve_tangent(bodies, self.config.timestep, contact.friction)?
                {
                    self.statistics.tangent_solves = self
                        .statistics
                        .tangent_solves
                        .checked_add(1)
                        .ok_or(EnvironmentError::ClockOverflow)?;
                    gained += result.energy_change;
                    self.commit_contact_velocities(indices, result.bodies);
                    self.observe_tangent(*id, before, bodies, result)?;
                }
                self.contacts.contacts.insert(*id, contact);
            }
            if gained > 0.0 {
                self.contacts.groups.get_mut(&group).unwrap().pairs[index].energy += gained;
            }
        }
        for index in (0..pairs.len()).rev() {
            let pair = &mut self.contacts.groups.get_mut(&group).unwrap().pairs[index];
            pair.redistribution_in = pair.redistribution_in.wrapping_sub(1);
            if pair.redistribution_in != 0 {
                continue;
            }
            let ids = pair.contacts.clone();
            let mut owners = ids
                .iter()
                .map(|id| {
                    let retained = &self.contacts.contacts[id];
                    let contact = retained.contact;
                    crate::FrictionRedistributionOwner {
                        first_core: retained.cores[0],
                        normal: retained.surface.normal,
                        point: contact.previous_point,
                        frame: contact.frame,
                        coordinates: contact.retained,
                    }
                })
                .collect::<Vec<_>>();
            crate::redistribute_retained_friction(&mut owners)?;
            for (id, owner) in ids.into_iter().zip(owners) {
                self.contacts
                    .contacts
                    .get_mut(&id)
                    .unwrap()
                    .contact
                    .retained = owner.coordinates;
            }
            self.contacts.groups.get_mut(&group).unwrap().pairs[index].redistribution_in = 5;
        }
        Ok(())
    }
    fn group_energy(&self, cores: &[u64]) -> Result<f64, EnvironmentError> {
        let mut energy = 0.0;
        for core in cores.iter().rev() {
            let body = &self.bodies[self.core_body_index(*core)?];
            energy += crate::KineticEnergyInput {
                mass: body.physical.mass,
                inertia: body.physical.inertia,
                linear: body.velocity.linear,
                angular: body.velocity.angular,
                queued: body.queued_velocity,
            }
            .energy()?;
        }
        Ok(energy)
    }
    fn solve_group_normals(&mut self, group: u64) -> Result<(), EnvironmentError> {
        if self
            .normal_observations
            .as_ref()
            .is_some_and(|v| v.len() == self.config.max_events)
        {
            return Err(EnvironmentError::ObservationLimit);
        }
        let mut ids = self.contacts.groups[&group].contacts.clone();
        ids.sort_by_key(|id| self.contacts.contacts[id].normal_history);
        self.contacts.groups.get_mut(&group).unwrap().contacts = ids.clone();
        for id in ids {
            let contact = &self.contacts.contacts[&id];
            if contact.surface.broken
                || contact.surface.distance >= self.tolerances.maximum_friction_distance
            {
                self.remove_contact(id)?;
            } else if contact.surface.distance
                > self.tolerances.friction_distance + self.tolerances.keeper_safety
            {
                let indices = self.contact_bodies(contact.endpoints)?;
                if indices
                    .into_iter()
                    .all(|index| self.bodies[index].crowded_contact_ordering)
                {
                    let contacts = &mut self
                        .contacts
                        .groups
                        .get_mut(&group)
                        .ok_or(ContactError::Missing)?
                        .contacts;
                    contacts.retain(|value| *value != id);
                    contacts.insert(0, id);
                }
            }
        }
        let Some(owner) = self.contacts.groups.get(&group) else {
            return Ok(());
        };
        let ids = owner.contacts.clone();
        let cores = owner.cores.clone();
        if ids.len() > 150 {
            self.statistics.oversized_contact_groups = self
                .statistics
                .oversized_contact_groups
                .checked_add(1)
                .ok_or(EnvironmentError::ClockOverflow)?;
            let objects = cores
                .iter()
                .map(|core| {
                    self.core_body_index(*core)
                        .map(|index| self.bodies[index].identity)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let solver = self
                .collision_solver
                .0
                .as_mut()
                .ok_or(EnvironmentError::CollisionSolverRequired)?;
            if solver.should_freeze_contacts(&objects) {
                let pairs = self.contacts.groups[&group]
                    .pairs
                    .iter()
                    .map(|pair| pair.cores)
                    .collect::<Vec<_>>();
                for core in cores.iter().rev() {
                    let index = self.core_body_index(*core)?;
                    if self.bodies[index].kind == BodyKind::Static {
                        continue;
                    }
                    self.bodies[index].crowded_contact_ordering = true;
                    let mut peers = 0;
                    for pair in &pairs {
                        if pair.contains(core)
                            && self.bodies[self.core_body_index(pair[0])?].kind != BodyKind::Static
                            && self.bodies[self.core_body_index(pair[1])?].kind != BodyKind::Static
                        {
                            peers += 1;
                        }
                    }
                    if peers > 1 {
                        self.bodies[index].velocity = VelocityState {
                            linear: [0.0; 3],
                            angular: [0.0; 3],
                        };
                        self.statistics.contact_freezes = self
                            .statistics
                            .contact_freezes
                            .checked_add(1)
                            .ok_or(EnvironmentError::ClockOverflow)?;
                    }
                }
                return Ok(());
            }
        }
        let indices = cores
            .iter()
            .map(|core| self.core_body_index(*core))
            .collect::<Result<Vec<_>, _>>()?;
        let bodies = indices
            .iter()
            .map(|i| {
                let body = &self.bodies[*i];
                crate::NormalBody {
                    linear_velocity: body.velocity.linear,
                    angular_velocity: body.velocity.angular,
                    inverse_mass: if body.motion_enabled {
                        1.0 / body.physical.mass
                    } else {
                        0.0
                    },
                    inverse_inertia: body
                        .physical
                        .inertia
                        .map(|v| if body.motion_enabled { 1.0 / v } else { 0.0 }),
                }
            })
            .collect::<Vec<_>>();
        let mut per_contact = Vec::with_capacity(ids.len());
        let mut rows = Vec::with_capacity(ids.len());
        let mut history = Vec::with_capacity(ids.len());
        for id in &ids {
            let contact = &self.contacts.contacts[id];
            let mut endpoints = [None; 2];
            let mut jacobians = [None; 2];
            for side in 0..2 {
                let index = cores
                    .iter()
                    .position(|core| *core == contact.cores[side])
                    .ok_or(ContactError::Missing)?;
                let body = &self.bodies[indices[index]];
                if body.kind == BodyKind::Static {
                    continue;
                }
                let row = crate::ContactNormalRow {
                    normal: contact.surface.normal,
                    angular_jacobian: contact.normal_jacobians[side]
                        .ok_or(ContactError::Missing)?,
                    distance: contact.surface.distance,
                    dynamic_endpoint: if side == 0 {
                        crate::DynamicEndpoint::First
                    } else {
                        crate::DynamicEndpoint::Second
                    },
                };
                endpoints[side] = Some(crate::NormalEndpointRow {
                    body: index,
                    angular_jacobian: row.angular_jacobian,
                });
                jacobians[side] = Some((index, row));
            }
            rows.push(crate::NormalContactRow {
                normal: contact.surface.normal,
                distance: contact.surface.distance,
                endpoints,
            });
            per_contact.push(jacobians);
            history.push(contact.normal_history);
        }
        let assembled = crate::NormalAssembly {
            rows: &rows,
            bodies: &bodies,
            target_distance: self.tolerances.friction_distance,
            timestep: self.config.timestep,
            maximum_dimension: self.config.max_events,
        }
        .assemble()?;
        let gravity = internal_position(self.config.gravity);
        let gravity_magnitude = ((gravity[0] * gravity[0] + gravity[1] * gravity[1])
            + gravity[2] * gravity[2])
            .sqrt() as f32;
        let solution = assembled.prepare()?.solve(crate::NormalSolvePolicy {
            history: &history,
            inverse_responses: &assembled.inverse_responses,
            gravity_magnitude,
            maximum_dimension: self.config.max_events,
        })?;
        let time = self.time();
        if let Some(observations) = &mut self.normal_observations {
            observations.push(NormalObservation {
                time,
                bodies: cores
                    .iter()
                    .zip(&bodies)
                    .map(|(id, body)| (*id, *body))
                    .collect(),
                contacts: ids
                    .iter()
                    .zip(&rows)
                    .map(|(id, row)| (*id, self.contacts.contacts[id].contact.previous_point, *row))
                    .collect(),
                system: assembled.clone(),
                solution: solution.clone(),
                after: Vec::new(),
            });
        }
        let Some(solution) = solution else {
            return self.finish_normal_observation(&cores);
        };
        let before = self.group_energy(&cores)?;
        let limits = self.velocity_command_limits();
        for (row_index, id) in ids.iter().enumerate() {
            let contact = self.contacts.contacts.get_mut(id).unwrap();
            contact.contact.normal_force = solution.forces[row_index];
            contact.normal_history = solution.history[row_index];
            let impulse = solution.impulses[row_index];
            if impulse == 0.0 {
                continue;
            }
            for (index, row) in per_contact[row_index].iter().flatten() {
                let body = &mut self.bodies[indices[*index]];
                if !body.motion_enabled {
                    continue;
                }
                let velocity = row.apply_impulse(
                    crate::NormalBody {
                        linear_velocity: body.queued_velocity.linear,
                        angular_velocity: body.queued_velocity.angular,
                        ..bodies[*index]
                    },
                    impulse,
                )?;
                body.queued_velocity = QueuedVelocity {
                    linear: velocity.linear_velocity,
                    angular: velocity.angular_velocity,
                };
                limits.apply(&mut body.velocity, &mut body.queued_velocity)?;
            }
        }
        let after = self.group_energy(&cores)?;
        let mut allowance = 0.0;
        for i in indices.iter().rev() {
            if self.bodies[*i].kind != BodyKind::Static {
                let mass = self.bodies[*i]
                    .physical
                    .mass
                    .max(self.config.performance.minimum_friction_mass)
                    .min(self.config.performance.minimum_friction_mass);
                allowance += f64::from(mass * gravity_magnitude) * f64::from(0.1_f32);
            }
        }
        for i in indices.into_iter().rev() {
            let body = &mut self.bodies[i];
            if body.kind == BodyKind::Static {
                continue;
            }
            if after <= before + allowance {
                body.velocity = body.reported_velocity();
            }
            body.queued_velocity = QueuedVelocity::default();
        }
        self.finish_normal_observation(&cores)
    }
    fn remove_contact(&mut self, id: u64) -> Result<(), EnvironmentError> {
        let event = self
            .contacts
            .contacts
            .get(&id)
            .ok_or(ContactError::Missing)?;
        self.emit_touch_callback(event.endpoints, event.surface, false)?;
        let contact = self
            .contacts
            .contacts
            .remove(&id)
            .ok_or(ContactError::Missing)?;
        self.contacts.slots.release(contact.owner)?;
        for core in contact.cores {
            let index = self.core_body_index(core)?;
            let time = self.time();
            self.bodies[index].quiet.refresh_time(time);
            self.contacts
                .body_contacts
                .get_mut(&core)
                .ok_or(ContactError::Missing)?
                .retain(|v| *v != id);
        }
        let group = self
            .contacts
            .groups
            .get_mut(&contact.group)
            .ok_or(ContactError::Missing)?;
        group.contacts.retain(|v| *v != id);
        for pair in &mut group.pairs {
            pair.contacts.retain(|value| *value != id);
        }
        group.connectivity_dirty |= group.pairs.iter().any(|pair| pair.contacts.is_empty());
        for pair in group.pairs.iter().filter(|pair| pair.contacts.is_empty()) {
            self.callbacks.retire_pair_clock(
                storage::pair_keys(&self.bodies, pair.cores)?,
                pair.last_impact_time,
            );
        }
        group.pairs.retain(|pair| !pair.contacts.is_empty());
        if group.contacts.is_empty() {
            let group = self.contacts.groups.remove(&contact.group).unwrap();
            for core in group.cores {
                self.contacts
                    .body_groups
                    .get_mut(&core)
                    .ok_or(ContactError::Missing)?
                    .retain(|v| *v != contact.group);
                let index = self.core_body_index(core)?;
                if self.bodies[index].kind != BodyKind::Static {
                    for controller in group.controllers.into_iter().rev() {
                        self.islands.detach(core, controller)?;
                    }
                }
                self.islands.request_connectivity_check(core)?;
            }
            for controller in group.controllers {
                self.islands.remove_controller(controller)?;
                self.contacts.controllers.remove(&controller);
            }
        } else {
            let retained = self.contacts.groups[&contact.group].contacts.clone();
            for core in contact.cores {
                if retained
                    .iter()
                    .any(|id| self.contacts.contacts[id].cores.contains(&core))
                {
                    continue;
                }
                let group = self.contacts.groups.get_mut(&contact.group).unwrap();
                group.cores.retain(|v| *v != core);
                let controllers = group.controllers;
                self.contacts
                    .body_groups
                    .get_mut(&core)
                    .ok_or(ContactError::Missing)?
                    .retain(|v| *v != contact.group);
                if !self
                    .islands
                    .is_immovable(core)
                    .ok_or(crate::IslandError::MissingCore)?
                {
                    for controller in controllers.into_iter().rev() {
                        self.islands.detach(core, controller)?;
                    }
                }
                self.islands.request_connectivity_check(core)?;
            }
            self.update_group_associations(contact.group)?;
        }
        Ok(())
    }
    pub(super) fn remove_body_contacts(&mut self, core: u64) -> Result<(), EnvironmentError> {
        for id in self
            .contacts
            .body_contacts
            .get(&core)
            .cloned()
            .unwrap_or_default()
        {
            self.remove_contact(id)?;
        }
        self.contacts.body_contacts.remove(&core);
        self.contacts.body_groups.remove(&core);
        Ok(())
    }
    pub(super) fn disable_body_contacts(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let peers = self
            .contacts
            .body_contacts
            .get(&core)
            .into_iter()
            .flatten()
            .map(|id| {
                let contact = &self.contacts.contacts[id];
                contact.cores[usize::from(contact.cores[0] == core)]
            })
            .collect::<Vec<_>>();
        for peer in peers {
            let identity = self.bodies[self.core_body_index(peer)?].identity;
            self.wake(identity)?;
            let contacts = self
                .contacts
                .body_contacts
                .get(&core)
                .cloned()
                .unwrap_or_default();
            for id in contacts {
                if self.contacts.contacts[&id].cores.contains(&peer) {
                    self.remove_contact(id)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_contact_snapshots_never_replace_the_live_world() {
        let (mut world, pair) = super::super::tests::automatic_pair_world(false);
        let poses = [
            world.cached_transform(1).unwrap().object,
            world.cached_transform(2).unwrap().object,
        ];
        let (_, geometry) = world.project_pair_contact(pair, poses).unwrap();
        let preferred = world
            .body(geometry.endpoints[1].body)
            .unwrap()
            .core_identity();
        world.admit_contact(geometry, preferred).unwrap();
        let expected = world.snapshot();
        world.restore(expected.clone()).unwrap();
        let mut bad = expected.clone();
        bad.contacts
            .contacts
            .values_mut()
            .next()
            .unwrap()
            .contact
            .normal_force = f32::NAN;
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
        let mut bad = expected.clone();
        bad.contacts
            .contacts
            .values_mut()
            .next()
            .unwrap()
            .normal_jacobians
            .iter_mut()
            .flatten()
            .next()
            .unwrap()[0] = f32::NAN;
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
        let mut bad = expected.clone();
        bad.contacts
            .contacts
            .values_mut()
            .next()
            .unwrap()
            .normal_jacobians = [None, None];
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
        let mut bad = expected.clone();
        bad.contacts.groups.values_mut().next().unwrap().pairs[0]
            .contacts
            .clear();
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
        let mut bad = expected.clone();
        let group = bad.contacts.groups.values().next().unwrap();
        let core = group
            .cores
            .iter()
            .find(|core| !bad.islands.is_immovable(**core).unwrap())
            .copied()
            .unwrap();
        let controller = group.controllers[0];
        bad.islands.detach(core, controller).unwrap();
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), expected);
    }
}
