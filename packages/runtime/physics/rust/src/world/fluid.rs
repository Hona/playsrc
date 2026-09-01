use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidInput {
    pub identity: u64,
    pub body: u64,
    pub surface_plane: [f32; 4],
    pub current_velocity: [f32; 3],
    pub damping: f32,
    pub contents: u32,
}
#[derive(Clone, Debug, PartialEq)]
struct FluidMember {
    controller: u64,
    pairs: Vec<u64>,
    state: crate::FluidBodyState,
}
#[derive(Clone, Debug, PartialEq)]
struct Fluid {
    input: FluidInput,
    surface: FluidSurface,
    members: BTreeMap<u64, FluidMember>,
    dormant: std::collections::BTreeSet<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct FluidSurface {
    normal: [f32; 3],
    distance: f32,
    current: [f32; 3],
}
impl FluidSurface {
    fn new(input: FluidInput, pose: PublishedBody) -> Self {
        let rotate = |value: [f32; 3]| {
            std::array::from_fn(|column| {
                (pose.orientation[column] * value[0] + pose.orientation[column + 3] * value[1])
                    + pose.orientation[column + 6] * value[2]
            })
        };
        let normal = rotate([
            input.surface_plane[0],
            input.surface_plane[1],
            input.surface_plane[2],
        ]);
        let translation = rotate(pose.position);
        let square = (normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2];
        let distance = input.surface_plane[3] * square
            - ((normal[1] * translation[1] + normal[0] * translation[0])
                + normal[2] * translation[2]);
        Self {
            normal,
            distance,
            current: rotate(input.current_velocity),
        }
    }
    fn sample(self, pose: PublishedBody) -> ([f32; 4], [f32; 3]) {
        let rotate = |v: [f32; 3]| {
            std::array::from_fn::<_, 3, _>(|row| {
                (pose.orientation[row * 3] * v[0] + pose.orientation[row * 3 + 1] * v[1])
                    + pose.orientation[row * 3 + 2] * v[2]
            })
        };
        let normal = rotate(self.normal);
        let current = rotate(self.current);
        let square = (normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2];
        let distance = (square * self.distance + normal[2] * pose.position[2])
            + (normal[0] * pose.position[0] + normal[1] * pose.position[1]);
        (
            [
                -normal[0],
                normal[2],
                -normal[1],
                distance * METERS_PER_INCH,
            ],
            [current[0], -current[2], current[1]],
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct FluidSpace {
    fluids: BTreeMap<u64, Fluid>,
}
impl FluidSpace {
    pub(super) fn remove_body(&mut self, identity: u64) -> Result<(), EnvironmentError> {
        if let Some(id) = self.at_body(identity) {
            if !self.fluids[&id].members.is_empty() {
                return Err(EnvironmentError::SnapshotMismatch);
            }
            self.fluids.remove(&id);
        }
        Ok(())
    }
    pub(super) fn at_body(&self, identity: u64) -> Option<u64> {
        self.fluids
            .iter()
            .find_map(|(id, fluid)| (fluid.input.body == identity).then_some(*id))
    }
    pub(super) fn contains_pair(&self, id: u64) -> bool {
        self.fluids
            .values()
            .any(|f| f.members.values().any(|member| member.pairs.contains(&id)))
    }
    pub(super) fn validate(&self, snapshot: &EnvironmentSnapshot) -> bool {
        if self.fluids.len() > snapshot.config.max_bodies {
            return false;
        }
        let mut controllers = std::collections::BTreeSet::new();
        let mut volume_bodies = std::collections::BTreeSet::new();
        for (identity, fluid) in &self.fluids {
            if !volume_bodies.insert(fluid.input.body)
                || fluid
                    .surface
                    .normal
                    .iter()
                    .chain(&fluid.surface.current)
                    .chain([&fluid.surface.distance])
                    .any(|value| !value.is_finite())
            {
                return false;
            }
            if fluid
                .input
                .surface_plane
                .iter()
                .chain(&fluid.input.current_velocity)
                .chain([&fluid.input.damping])
                .any(|value| !value.is_finite())
            {
                return false;
            }
            if *identity != fluid.input.identity
                || !snapshot
                    .bodies
                    .iter()
                    .any(|b| b.identity == fluid.input.body)
                || fluid.members.len() > snapshot.config.max_bodies
            {
                return false;
            }
            if fluid.dormant.len() > snapshot.config.max_bodies
                || fluid.dormant.iter().any(|core| {
                    fluid.members.contains_key(core)
                        || !snapshot
                            .bodies
                            .iter()
                            .any(|body| body.core_identity == *core)
                })
            {
                return false;
            }
            for (core, member) in &fluid.members {
                if member.controller < 3
                    || !snapshot
                        .islands
                        .controller(member.controller)
                        .is_some_and(|controller| {
                            controller.priority == 1600 && controller.associated.is_empty()
                        })
                    || !snapshot
                        .islands
                        .core_controllers(*core)
                        .is_some_and(|controllers| controllers.contains(&member.controller))
                {
                    return false;
                }
                if member.controller >= snapshot.next_controller
                    || !controllers.insert(member.controller)
                    || member.pairs.is_empty()
                    || member.pairs.len() > snapshot.config.max_events
                    || !snapshot.bodies.iter().any(|b| b.core_identity == *core)
                {
                    return false;
                }
                if !member
                    .state
                    .entrained_velocity
                    .iter()
                    .chain([&member.state.visible_area, &member.state.previous_area])
                    .all(|v| v.is_finite())
                {
                    return false;
                }
                for (i, id) in member.pairs.iter().enumerate() {
                    if member.pairs[..i].contains(id)
                        || !snapshot.pairs.has_fluid_pair(
                            *id,
                            *core,
                            fluid.input.body,
                            &snapshot.bodies,
                        )
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}
impl PhysicsEnvironment {
    pub fn create_fluid(&mut self, input: FluidInput) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        if input
            .surface_plane
            .iter()
            .chain(&input.current_velocity)
            .chain([&input.damping])
            .any(|v| !v.is_finite())
        {
            return Err(EnvironmentError::NonFinite);
        }
        if candidate.fluids.fluids.contains_key(&input.identity)
            || candidate.fluids.at_body(input.body).is_some()
        {
            return Err(EnvironmentError::DuplicateBody);
        }
        if candidate.fluids.fluids.len() >= candidate.config.max_bodies {
            return Err(EnvironmentError::BodyLimit);
        }
        let body = candidate
            .body(input.body)
            .ok_or(EnvironmentError::MissingBody)?;
        let surface = FluidSurface::new(input, body.published()?);
        candidate.set_drag_enabled(input.body, false)?;
        candidate.set_gravity_enabled(input.body, false)?;
        let enabled = candidate.body(input.body).unwrap().collisions_enabled;
        candidate.set_collisions_enabled(input.body, false)?;
        candidate.fluids.fluids.insert(
            input.identity,
            Fluid {
                input,
                surface,
                members: BTreeMap::new(),
                dormant: std::collections::BTreeSet::new(),
            },
        );
        candidate.set_collisions_enabled(input.body, enabled)?;
        *self = candidate;
        Ok(())
    }
    pub fn fluid_members(&self, identity: u64) -> Result<Vec<u64>, EnvironmentError> {
        let fluid = self
            .fluids
            .fluids
            .get(&identity)
            .ok_or(EnvironmentError::MissingBody)?;
        fluid
            .members
            .keys()
            .map(|core| Ok(self.bodies[self.core_body_index(*core)?].identity))
            .collect()
    }
    pub(super) fn enter_fluid_pair(
        &mut self,
        pair: u64,
        cores: [u64; 2],
    ) -> Result<(), EnvironmentError> {
        for side in 0..2 {
            let identity = self.bodies[self.core_body_index(cores[side])?].identity;
            let Some(fluid) = self.fluids.at_body(identity) else {
                continue;
            };
            let core = cores[1 - side];
            if let Some(member) = self
                .fluids
                .fluids
                .get_mut(&fluid)
                .unwrap()
                .members
                .get_mut(&core)
            {
                if !member.pairs.contains(&pair) {
                    member.pairs.push(pair);
                }
                continue;
            }
            let controller = self.next_controller;
            self.next_controller = self
                .next_controller
                .checked_add(1)
                .ok_or(EnvironmentError::ClockOverflow)?;
            self.islands.register_controller(crate::IslandController {
                identity: controller,
                priority: 1600,
                associated: Vec::new(),
            })?;
            self.islands.attach(core, controller)?;
            self.fluids.fluids.get_mut(&fluid).unwrap().members.insert(
                core,
                FluidMember {
                    controller,
                    pairs: vec![pair],
                    state: crate::FluidBodyState::default(),
                },
            );
            let was_dormant = self
                .fluids
                .fluids
                .get_mut(&fluid)
                .unwrap()
                .dormant
                .remove(&core);
            let body = &self.bodies[self.core_body_index(core)?];
            if !was_dormant && self.event_reporting && body.callback_flags & 0x0100 != 0 {
                self.emit_callback(
                    PhysicsCallbackKind::Fluid {
                        controller: fluid,
                        entered: true,
                    },
                    [Some(body.identity), None],
                    None,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn leave_fluid_pair(&mut self, pair: u64) -> Result<(), EnvironmentError> {
        let affected =
            self.fluids
                .fluids
                .iter()
                .flat_map(|(id, fluid)| {
                    fluid.members.iter().filter_map(move |(core, m)| {
                        m.pairs.contains(&pair).then_some((*id, *core))
                    })
                })
                .collect::<Vec<_>>();
        for (fluid, core) in affected {
            let members = &mut self.fluids.fluids.get_mut(&fluid).unwrap().members;
            let member = members.get_mut(&core).unwrap();
            member.pairs.retain(|id| *id != pair);
            if !member.pairs.is_empty() {
                continue;
            }
            let member = members.remove(&core).unwrap();
            self.islands.detach(core, member.controller)?;
            self.islands.remove_controller(member.controller)?;
            if self.islands.movement(core) == Some(crate::CoreMovement::Dormant) {
                self.fluids
                    .fluids
                    .get_mut(&fluid)
                    .unwrap()
                    .dormant
                    .insert(core);
                continue;
            }
            let body = &self.bodies[self.core_body_index(core)?];
            if self.event_reporting && body.callback_flags & 0x0100 != 0 {
                self.emit_callback(
                    PhysicsCallbackKind::Fluid {
                        controller: fluid,
                        entered: false,
                    },
                    [Some(body.identity), None],
                    None,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn finish_fluid_revival(&mut self, core: u64) -> Result<(), EnvironmentError> {
        let ended = self
            .fluids
            .fluids
            .iter_mut()
            .filter_map(|(id, fluid)| fluid.dormant.remove(&core).then_some(*id))
            .collect::<Vec<_>>();
        let body = &self.bodies[self.core_body_index(core)?];
        let identity = body.identity;
        let notify = self.event_reporting && body.callback_flags & 0x0100 != 0;
        if notify {
            for fluid in ended {
                self.emit_callback(
                    PhysicsCallbackKind::Fluid {
                        controller: fluid,
                        entered: false,
                    },
                    [Some(identity), None],
                    None,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn forget_fluid_core(&mut self, core: u64) {
        for fluid in self.fluids.fluids.values_mut() {
            fluid.dormant.remove(&core);
        }
    }
    pub(super) fn run_fluid_controller(
        &mut self,
        controller: u64,
    ) -> Result<bool, EnvironmentError> {
        let Some((fluid_id, core)) = self.fluids.fluids.iter().find_map(|(id, fluid)| {
            fluid
                .members
                .iter()
                .find_map(|(core, m)| (m.controller == controller).then_some((*id, *core)))
        }) else {
            return Ok(false);
        };
        let fluid = self.fluids.fluids[&fluid_id].input;
        let index = self.core_body_index(core)?;
        let identity = self.bodies[index].identity;
        let body = &self.bodies[index];
        if body.callback_flags & 0x1000 == 0 {
            return Ok(true);
        }
        let fluid_body = self.body(fluid.body).ok_or(EnvironmentError::MissingBody)?;
        let density = self.surfaces.records[fluid_body.material as usize]
            .physics
            .density
            * body.buoyancy_ratio;
        if density == 0.0 {
            return Ok(true);
        }
        let (world_plane, current) = self.fluids.fluids[&fluid_id]
            .surface
            .sample(fluid_body.published()?);
        let object = self.cached_transform(identity)?.object;
        let body = &self.bodies[index];
        let normal = [world_plane[0], world_plane[1], world_plane[2]];
        let mut plane = [0.0; 4];
        for (axis, component) in plane[..3].iter_mut().enumerate() {
            *component = ((object.orientation[axis + 3] * f64::from(normal[1])
                + object.orientation[axis] * f64::from(normal[0]))
                + object.orientation[axis + 6] * f64::from(normal[2]))
                as f32;
        }
        plane[3] = ((f64::from(normal[0]) * object.position[0]
            + f64::from(normal[1]) * object.position[1])
            + (f64::from(normal[2]) * object.position[2] + f64::from(world_plane[3])))
            as f32;
        let input = crate::FluidBodyInput {
            frame: crate::FluidPressureFrame {
                object_basis: object.orientation,
                object_position: object.position,
                core_basis: body
                    .collision_orientation
                    .unwrap_or_else(|| body.orientation.matrix()),
                core_position: body.core_position,
                angular: body.velocity.angular,
                linear: body.velocity.linear,
                current: [0.0; 3],
                pressure: 0.0,
                friction: 0.0,
                aerodynamic: false,
            },
            plane,
            current,
            gravity: internal_direction(self.config.gravity, METERS_PER_INCH),
            inverse_mass: if body.motion_enabled {
                1.0 / body.physical.mass
            } else {
                0.0
            },
            inverse_inertia: body
                .physical
                .inertia
                .map(|v| if body.motion_enabled { 1.0 / v } else { 0.0 }),
            queued: body.queued_velocity,
            timestep: self.config.timestep,
        };
        let settings = crate::FluidSettings {
            density,
            pressure_damping: fluid.damping,
            friction_damping: 0.05,
            torque_factor: 0.01,
            epsilon: 1.0e-10,
            viscosity: 0.0,
            entrainment: 0.1,
            aerodynamic: false,
        };
        let member = self
            .fluids
            .fluids
            .get_mut(&fluid_id)
            .unwrap()
            .members
            .get_mut(&core)
            .unwrap();
        let output =
            member
                .state
                .advance(&body.topology[..body.shape.convex_count()], settings, input)?;
        self.bodies[index].queued_velocity = output.queued;
        Ok(true)
    }
}

pub(super) fn volume_ratio(mass: f32, volume: f32, density: f32) -> Result<f32, EnvironmentError> {
    if !mass.is_finite() || !volume.is_finite() || !density.is_finite() {
        return Err(EnvironmentError::NonFinite);
    }
    if volume == 0.0 {
        return Ok(1.0);
    }
    let cubic_units = (METERS_PER_INCH * METERS_PER_INCH) * METERS_PER_INCH;
    let converted = cubic_units * volume.max(5.0);
    let ratio = (mass / converted) / density;
    if !ratio.is_finite() {
        return Err(EnvironmentError::NonFinite);
    }
    Ok(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_fluid_member_controller_cannot_be_detached_or_reassigned_in_a_snapshot() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world
            .create_fluid(FluidInput {
                identity: 10,
                body: 2,
                surface_plane: [0.0, 0.0, 1.0, 0.0],
                current_velocity: [0.0; 3],
                damping: 0.01,
                contents: 32,
            })
            .unwrap();
        world.simulate(world.config.timestep).unwrap();
        world.simulate(world.config.timestep).unwrap();
        let members = world.fluid_members(10).unwrap();
        assert_eq!(members, [1]);
        let saved = world.snapshot();
        let core = world.body(1).unwrap().core_identity;
        let mut bad = saved.clone();
        bad.fluids
            .fluids
            .get_mut(&10)
            .unwrap()
            .members
            .get_mut(&core)
            .unwrap()
            .controller = DEFAULT_CONTROLLER;
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), saved);
        let mut bad = saved.clone();
        bad.fluids
            .fluids
            .get_mut(&10)
            .unwrap()
            .members
            .get_mut(&core)
            .unwrap()
            .state
            .entrained_velocity[0] = f32::NAN;
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), saved);
        let mut bad = saved.clone();
        let pairs = &mut bad
            .fluids
            .fluids
            .get_mut(&10)
            .unwrap()
            .members
            .get_mut(&core)
            .unwrap()
            .pairs;
        pairs.push(pairs[0]);
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), saved);
    }
    #[test]
    fn fluid_creation_and_restoration_reject_bad_inputs_atomically() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        let before = world.snapshot();
        let input = FluidInput {
            identity: 10,
            body: 2,
            surface_plane: [0.0, 0.0, 1.0, 0.0],
            current_velocity: [0.0; 3],
            damping: 0.01,
            contents: 32,
        };
        assert_eq!(
            world.create_fluid(FluidInput { body: 3, ..input }),
            Err(EnvironmentError::MissingBody)
        );
        assert_eq!(world.snapshot(), before);
        assert_eq!(
            world.create_fluid(FluidInput {
                damping: f32::NAN,
                ..input
            }),
            Err(EnvironmentError::NonFinite)
        );
        assert_eq!(world.snapshot(), before);
        world.create_fluid(input).unwrap();
        let saved = world.snapshot();
        world.restore(saved.clone()).unwrap();
        assert_eq!(
            world.create_fluid(input),
            Err(EnvironmentError::DuplicateBody)
        );
        assert_eq!(world.snapshot(), saved);
        let mut bad = saved.clone();
        bad.fluids.fluids.get_mut(&10).unwrap().input.surface_plane[3] = f32::NAN;
        assert_eq!(world.restore(bad), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(world.snapshot(), saved);
    }
    #[test]
    fn object_volume_ratio_preserves_zero_default_minimum_and_conversion_order() {
        assert_eq!(volume_ratio(5.0, 0.0, 0.0).unwrap(), 1.0);
        assert_eq!(
            volume_ratio(5.0, -10.0, 2000.0).unwrap(),
            volume_ratio(5.0, 5.0, 2000.0).unwrap()
        );
        assert_eq!(
            volume_ratio(5.0, 336.82, 2000.0).unwrap().to_bits(),
            0.452_940_34_f32.to_bits()
        );
        assert_eq!(
            volume_ratio(5.0, 10.0, 0.0),
            Err(EnvironmentError::NonFinite)
        );
    }
}
