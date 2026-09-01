use super::*;
use crate::{ShadowControlBody, ShadowControlState};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
struct ShadowBody {
    controller: u64,
    state: ShadowControlState,
    arrival: f32,
    enabled: bool,
    translation: bool,
    rotation: bool,
    temporary_gravity: bool,
    saved_mass: f32,
    saved_inertia: [f32; 3],
    saved_damping: f32,
    saved_material: u16,
    saved_flags: u16,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ShadowSpace {
    bodies: BTreeMap<u64, ShadowBody>,
    controllers: BTreeMap<u64, u64>,
    observations: Option<Vec<ShadowObservation>>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowObservation {
    pub body: u64,
    pub input: ShadowControlBody,
    pub before: ShadowControlState,
    pub after: ShadowControlState,
    pub arrival_before: f32,
    pub arrival_after: f32,
    pub velocity: VelocityState,
}
impl ShadowSpace {
    pub(super) fn begin(&mut self) {
        if let Some(observations) = &mut self.observations {
            observations.clear();
        }
    }
    pub(super) fn valid(&self, snapshot: &EnvironmentSnapshot) -> bool {
        if self.bodies.len() > snapshot.config.max_bodies
            || self.controllers.len() != self.bodies.len()
            || self.observations.as_ref().is_some_and(|v| {
                v.len() > snapshot.config.max_events
                    || v.iter().any(|observation| {
                        !observation.arrival_after.is_finite()
                            || observation.arrival_after < 0.0
                            || observation
                                .velocity
                                .linear
                                .iter()
                                .chain(&observation.velocity.angular)
                                .any(|value| !value.is_finite())
                            || observation
                                .before
                                .prepare(
                                    observation.input,
                                    observation.arrival_before,
                                    snapshot.config.timestep,
                                )
                                .is_err()
                            || observation
                                .after
                                .prepare(
                                    observation.input,
                                    observation.arrival_after,
                                    snapshot.config.timestep,
                                )
                                .is_err()
                    })
            })
        {
            return false;
        }
        let mut controllers = std::collections::BTreeSet::new();
        self.bodies.iter().all(|(id, shadow)| {
            let Some(body) = snapshot.bodies.iter().find(|body| body.identity == *id) else {
                return false;
            };
            self.controllers.get(&shadow.controller) == Some(id)
                && controllers.insert(shadow.controller)
                && shadow.controller >= 3
                && shadow.controller < snapshot.next_controller
                && snapshot
                    .islands
                    .controller(shadow.controller)
                    .is_some_and(|controller| {
                        controller.priority == 500 && controller.associated.is_empty()
                    })
                && snapshot
                    .islands
                    .core_controllers(body.core_identity)
                    .is_some_and(|controllers| controllers.contains(&shadow.controller))
                && shadow.arrival.is_finite()
                && shadow.arrival >= 0.0
                && shadow.saved_mass.is_finite()
                && shadow.saved_mass > 0.0
                && shadow
                    .saved_inertia
                    .iter()
                    .all(|v| v.is_finite() && *v > 0.0)
                && shadow.saved_damping.is_finite()
                && shadow
                    .state
                    .prepare(control_body(body), shadow.arrival, snapshot.config.timestep)
                    .is_ok()
        })
    }
}
fn control_body(body: &RigidBody) -> ShadowControlBody {
    ShadowControlBody {
        position: body.core_position,
        basis: body
            .collision_orientation
            .unwrap_or_else(|| body.orientation.matrix()),
        orientation: body.orientation,
        shift: body.frame.shift(),
        velocity: body.velocity,
    }
}
impl PhysicsEnvironment {
    pub fn record_shadow_observations(&mut self, enabled: bool) {
        self.shadows.observations = enabled.then(Vec::new);
    }
    pub fn shadow_observations(&self) -> Option<&[ShadowObservation]> {
        self.shadows.observations.as_deref()
    }
    pub fn set_shadow(
        &mut self,
        identity: u64,
        maximum_speed: f32,
        maximum_angular: f32,
        allow_translation: bool,
        allow_rotation: bool,
    ) -> Result<(), EnvironmentError> {
        if !maximum_speed.is_finite() || !maximum_angular.is_finite() {
            return Err(EnvironmentError::NonFinite);
        }
        let mut candidate = self.clone();
        if let Some(shadow) = candidate.shadows.bodies.get_mut(&identity) {
            shadow.state.maximum_linear_speed = maximum_speed;
            shadow.state.maximum_linear_damping = maximum_speed;
            shadow.state.maximum_angular_speed = maximum_angular;
            shadow.state.maximum_angular_damping = maximum_angular;
            *self = candidate;
            return Ok(());
        }
        let body = candidate
            .body(identity)
            .ok_or(EnvironmentError::MissingBody)?;
        if body.kind == BodyKind::Static {
            return Err(EnvironmentError::StaticBody);
        }
        let pose = body.published()?;
        let original = body.physical;
        let core = body.core_identity;
        let controller = candidate.next_controller;
        let shadow = ShadowBody {
            controller,
            state: ShadowControlState {
                target_position: internal_position(pose.position),
                target_orientation: SourceAngleBasis::from_degrees(pose.angles)?
                    .object_orientation()?,
                previous_position: [0.0; 3],
                last_impulse: [0.0; 3],
                maximum_linear_speed: maximum_speed,
                maximum_linear_damping: maximum_speed,
                maximum_angular_speed: maximum_angular,
                maximum_angular_damping: maximum_angular,
                damping: 1.0,
                teleport_distance: 0.0,
            },
            arrival: 0.0,
            enabled: false,
            translation: allow_translation,
            rotation: allow_rotation,
            temporary_gravity: false,
            saved_mass: original.mass,
            saved_inertia: original.inertia,
            saved_damping: body.angular_damping,
            saved_material: body.material_token,
            saved_flags: body.callback_flags,
        };
        candidate.next_controller = controller
            .checked_add(1)
            .ok_or(EnvironmentError::ClockOverflow)?;
        let material = candidate
            .surfaces
            .surface_data(0xf000)
            .ok_or(EnvironmentError::DependencyMismatch)?
            .index;
        let body = candidate.body_mut(identity)?;
        body.material_token = 0xf000;
        body.material = material;
        let island = candidate
            .islands
            .island_of(core)
            .ok_or(crate::IslandError::MissingCore)?;
        if candidate.islands.movement(core) == Some(crate::CoreMovement::Dormant) {
            candidate.revive_island(island)?;
        }
        candidate.refresh_material_contacts(core)?;
        let body = candidate.body_mut(identity)?;
        body.angular_damping = 100.0;
        let mut inertia = if allow_rotation {
            original.inertia
        } else {
            [1.0e15_f32; 3]
        };
        let mass = if allow_translation {
            original.mass
        } else {
            50_000.0
        };
        if !allow_translation {
            let ratio = mass / original.mass;
            inertia =
                inertia.map(|value| ((f64::from(value) * f64::from(ratio)) as f32).min(1.0e18));
        }
        body.physical = if allow_translation {
            original.with_core_mass_frame(&body.shape, mass, inertia)?
        } else {
            PhysicalShape::from_archive(&body.shape, mass, inertia)?
        };
        let volume = body.volume;
        if !allow_translation {
            let density = candidate.surfaces.records[material as usize]
                .physics
                .density;
            candidate.body_mut(identity)?.buoyancy_ratio =
                fluid::volume_ratio(mass, volume, density)?;
            candidate.set_gravity_enabled(identity, false)?;
        }
        candidate.body_mut(identity)?.callback_flags = (shadow.saved_flags & !0x22) | 0x10;
        candidate.set_drag_enabled(identity, false)?;
        candidate
            .islands
            .register_controller(crate::IslandController {
                identity: controller,
                priority: 500,
                associated: Vec::new(),
            })?;
        candidate.islands.attach(core, controller)?;
        candidate.shadows.controllers.insert(controller, identity);
        candidate.shadows.bodies.insert(identity, shadow);
        candidate.recheck_collision_filter_inner(identity)?;
        *self = candidate;
        Ok(())
    }
    pub fn update_shadow(
        &mut self,
        identity: u64,
        position: [f32; 3],
        angles: [f32; 3],
        temporary_gravity: bool,
        duration: f32,
    ) -> Result<(), EnvironmentError> {
        if position
            .iter()
            .chain(&angles)
            .chain([&duration])
            .any(|v| !v.is_finite())
        {
            return Err(EnvironmentError::NonFinite);
        }
        let position = internal_position(position);
        let orientation = SourceAngleBasis::from_degrees(angles)?.object_orientation()?;
        let mut candidate = self.clone();
        let Some(shadow) = candidate.shadows.bodies.get(&identity) else {
            return Err(EnvironmentError::MissingShadow);
        };
        if shadow.temporary_gravity != temporary_gravity && shadow.translation {
            candidate.set_gravity_enabled(identity, !temporary_gravity)?;
        }
        let shadow = candidate.shadows.bodies.get_mut(&identity).unwrap();
        shadow.temporary_gravity = temporary_gravity;
        let delta = std::array::from_fn::<_, 3, _>(|axis| {
            shadow.state.target_position[axis] - position[axis]
        });
        let squared = (delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2];
        let mut angle_delta = (shadow.state.target_orientation.quaternion[0]
            - orientation.quaternion[0])
            .abs() as f32;
        for axis in 1..4 {
            angle_delta = (f64::from(angle_delta)
                + (shadow.state.target_orientation.quaternion[axis] - orientation.quaternion[axis])
                    .abs()) as f32;
        }
        shadow.state.target_position = position;
        shadow.state.target_orientation = orientation;
        shadow.arrival = duration.max(0.0);
        shadow.enabled = true;
        if squared >= f64::from(1.0e-8_f32) || angle_delta >= 1.0e-8_f32 {
            candidate.wake(identity)?;
        }
        *self = candidate;
        Ok(())
    }
    pub(super) fn run_shadow_controller(
        &mut self,
        controller: u64,
    ) -> Result<bool, EnvironmentError> {
        let Some(identity) = self.shadows.controllers.get(&controller).copied() else {
            return Ok(false);
        };
        let mut shadow = self.shadows.bodies[&identity].clone();
        if !shadow.enabled {
            shadow.state.previous_position = [0.0; 3];
            self.shadows.bodies.insert(identity, shadow);
            return Ok(true);
        }
        let body = control_body(self.body(identity).ok_or(EnvironmentError::MissingBody)?);
        let before = shadow.state;
        let arrival_before = shadow.arrival;
        let plan = shadow
            .state
            .prepare(body, shadow.arrival, self.config.timestep)?;
        if plan.teleport {
            self.change_object_pose(
                identity,
                shadow.state.target_position,
                shadow.state.target_orientation,
                true,
            )?;
        }
        let mut velocity = shadow
            .state
            .finish(plan, control_body(self.body(identity).unwrap()))?;
        if shadow.translation {
            let gravity = (f64::from(internal_direction(self.config.gravity, METERS_PER_INCH)[1])
                * f64::from(self.config.timestep)) as f32;
            if shadow.state.last_impulse[1] > gravity
                && self
                    .friction_contacts(identity)?
                    .iter()
                    .any(|contact| contact.normal[2] < -0.7)
            {
                let delta = gravity - shadow.state.last_impulse[1];
                velocity.linear[1] += delta;
                shadow.state.last_impulse[1] += delta;
            }
        }
        shadow.arrival = (shadow.arrival - self.config.timestep).max(0.0);
        self.body_mut(identity)?.velocity = velocity;
        if let Some(observations) = &mut self.shadows.observations {
            if observations.len() >= self.config.max_events {
                return Err(EnvironmentError::ObservationLimit);
            }
            observations.push(ShadowObservation {
                body: identity,
                input: body,
                before,
                after: shadow.state,
                arrival_before,
                arrival_after: shadow.arrival,
                velocity,
            });
        }
        self.shadows.bodies.insert(identity, shadow);
        Ok(true)
    }
    pub(super) fn destroy_body_shadow(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        if let Some(shadow) = self.shadows.bodies.remove(&identity) {
            self.shadows.controllers.remove(&shadow.controller);
            let core = self
                .body(identity)
                .ok_or(EnvironmentError::MissingBody)?
                .core_identity;
            self.islands.detach(core, shadow.controller)?;
            self.islands.remove_controller(shadow.controller)?;
        }
        Ok(())
    }
    pub fn remove_shadow(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        let Some(shadow) = candidate.shadows.bodies.get(&identity).cloned() else {
            candidate
                .body(identity)
                .ok_or(EnvironmentError::MissingBody)?;
            return Ok(());
        };
        if candidate.body(identity).unwrap().callback_flags & 0x0400 == 0 {
            let body = candidate.body_mut(identity)?;
            body.angular_damping = shadow.saved_damping;
            let ratio = shadow.saved_mass / body.physical.mass;
            let interim = body
                .physical
                .inertia
                .map(|v| ((f64::from(v) * f64::from(ratio)) as f32).min(1.0e18));
            body.physical =
                body.physical
                    .with_core_mass_frame(&body.shape, shadow.saved_mass, interim)?;
            body.callback_flags = shadow.saved_flags;
            candidate.set_drag_enabled(identity, true)?;
            candidate.set_gravity_enabled(identity, true)?;
            let material = candidate
                .surfaces
                .surface_data(i32::from(shadow.saved_material))
                .ok_or(EnvironmentError::DependencyMismatch)?
                .index;
            let body = candidate.body_mut(identity)?;
            body.material_token = shadow.saved_material;
            body.material = material;
            let core = body.core_identity;
            let island = candidate
                .islands
                .island_of(core)
                .ok_or(crate::IslandError::MissingCore)?;
            if candidate.islands.movement(core) == Some(crate::CoreMovement::Dormant) {
                candidate.revive_island(island)?;
            }
            candidate.refresh_material_contacts(core)?;
            let body = candidate.body_mut(identity)?;
            body.physical = body.physical.with_core_mass_frame(
                &body.shape,
                shadow.saved_mass,
                shadow.saved_inertia,
            )?;
        }
        candidate.destroy_body_shadow(identity)?;
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
#[path = "shadow_tests.rs"]
mod tests;
