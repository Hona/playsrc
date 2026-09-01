//! TF2's ordered world-object construction over prepared collision resources.
// Portions adapted from Valve's official Source SDK 2013.
// Copyright Valve Corporation, All rights reserved.
// See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt at the repository root.
use crate::rigid_body::{RigidModel, RigidModelError};
use playsrc_collision::{PhysicalModelInventory, World};
use playsrc_keyvalues::NumericValue;
use playsrc_material::SurfacePropertyRegistry;
use playsrc_phy::KeyValue;
use playsrc_physics::{
    BodyKind, EnvironmentConfig, EnvironmentError, FluidInput, PhysicsEnvironment,
};
use std::{fmt, sync::Arc};
mod policy;
use policy::{BodyRule, Policy};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum BodyOwner {
    World,
    MapEntity(u32),
    BoneFollower { entity: u32, solid: usize },
    Projectile(u32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapBodyKind {
    Static,
    Shadow,
    ParentedShadow,
    BoneFollower(usize),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapBodyInput {
    pub entity: u32,
    pub handle: playsrc_entity::EntityHandle,
    pub kind: MapBodyKind,
    pub position: [f32; 3],
    pub angles: [f32; 3],
    pub solid: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileBodyInput {
    pub projectile: u32,
    pub position: [f32; 3],
    pub angles: [f32; 3],
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldBody {
    pub identity: u64,
    pub solid: usize,
    pub contents: u32,
    pub fluid: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RigidWorld {
    physics: PhysicsEnvironment,
    surfaces: Arc<SurfacePropertyRegistry>,
    world_bodies: Vec<WorldBody>,
    next_body: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    MissingWorld,
    DependencyMismatch,
    InvalidKeydata,
    MissingSolid,
    MissingEntity,
    VirtualTerrain,
    DuplicateProjectile,
    MissingProjectile,
    PolicyLimit,
    Model(RigidModelError),
    Physics(EnvironmentError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingWorld => f.write_str("physical world model is missing"),
            Self::DependencyMismatch => {
                f.write_str("physical model inventory belongs to a different collision world")
            }
            Self::InvalidKeydata => f.write_str("physical world keydata is invalid or unsupported"),
            Self::MissingSolid => f.write_str("physical world refers to an unavailable solid"),
            Self::MissingEntity => f.write_str("physical map body requires an Entity handle"),
            Self::VirtualTerrain => f.write_str(
                "physical virtual terrain requires its authored collision implementation",
            ),
            Self::DuplicateProjectile => f.write_str("projectile already owns a physical body"),
            Self::MissingProjectile => f.write_str("projectile physical body is missing"),
            Self::PolicyLimit => {
                f.write_str("physical game policy exceeded its admitted state bounds")
            }
            Self::Model(error) => error.fmt(f),
            Self::Physics(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
impl From<EnvironmentError> for Error {
    fn from(error: EnvironmentError) -> Self {
        Self::Physics(error)
    }
}
impl From<RigidModelError> for Error {
    fn from(error: RigidModelError) -> Self {
        Self::Model(error)
    }
}

impl RigidWorld {
    /// Static-prop representations, when present, must precede this world-model phase.
    /// The configured jump map has no static-prop occurrences.
    pub fn from_world_model(
        config: EnvironmentConfig,
        collision: &World,
        models: &PhysicalModelInventory,
        surfaces: Arc<SurfacePropertyRegistry>,
    ) -> Result<Self, Error> {
        if collision.identity != models.world_identity() {
            return Err(Error::DependencyMismatch);
        }
        let model = models.model(0).ok_or(Error::MissingWorld)?;
        let primary = model
            .solids
            .iter()
            .find(|solid| solid.solid == 0)
            .ok_or(Error::MissingWorld)?;
        let default = surfaces.surface_index(b"default") as u32;
        let mut result = Self {
            physics: PhysicsEnvironment::new(config, Arc::clone(&surfaces))?,
            surfaces: Arc::clone(&surfaces),
            world_bodies: Vec::new(),
            next_body: 1,
        };
        result.physics.enable_delete_queue(true);
        result.physics.set_event_reporting(true);
        result.physics.set_object_event_reporting(true);
        result
            .physics
            .set_collision_solver(Some(Box::new(Policy::new(
                config.max_bodies,
                config.max_events,
            ))));
        let first = RigidModel::world_piece(Arc::clone(&primary.shape), None, default, 1)?;
        result.add_world_body(&first, 0, 1, None)?;
        for (ordinal, block) in model.key_data.blocks.iter().enumerate() {
            if let Some(solid) = model
                .key_data
                .solid_properties_at(ordinal)
                .map_err(|_| Error::InvalidKeydata)?
            {
                if solid.index == 0 {
                    continue;
                }
                let index = usize::try_from(solid.index).map_err(|_| Error::MissingSolid)?;
                let geometry = model
                    .solids
                    .iter()
                    .find(|solid| solid.solid == index)
                    .ok_or(Error::MissingSolid)?;
                let contents = solid.contents.unwrap_or(1);
                let resource = RigidModel::world_piece(
                    Arc::clone(&geometry.shape),
                    Some(solid),
                    default,
                    contents,
                )?;
                result.add_world_body(&resource, index, contents, None)?;
            } else if let Some(fluid) = model
                .key_data
                .fluid_properties_at(ordinal)
                .map_err(|_| Error::InvalidKeydata)?
            {
                if fluid.index <= 0 || fluid.index as usize >= model.solids.len() {
                    continue;
                }
                let index = fluid.index as usize;
                let geometry = model
                    .solids
                    .iter()
                    .find(|solid| solid.solid == index)
                    .ok_or(Error::MissingSolid)?;
                let material =
                    surfaces.surface_index(fluid.surface_property.unwrap_or(b"water")) as u32;
                let resource =
                    RigidModel::world_piece(Arc::clone(&geometry.shape), None, material, 1)?;
                let identity = result.next_body;
                let body = result.add_world_body(&resource, index, 1, Some(identity))?;
                result.physics.create_fluid(FluidInput {
                    identity,
                    body,
                    surface_plane: fluid
                        .surface_plane
                        .unwrap_or([playsrc_phy::Float32(0); 4])
                        .map(|v| f32::from_bits(v.0)),
                    current_velocity: fluid
                        .current_velocity
                        .unwrap_or([playsrc_phy::Float32(0); 3])
                        .map(|v| f32::from_bits(v.0)),
                    damping: fluid.damping.map_or(0.0, |v| f32::from_bits(v.0)),
                    contents: fluid.contents.unwrap_or(0),
                })?;
            } else if block.name.eq_ignore_ascii_case(b"materialtable") {
                let mut table = [0; 128];
                for entry in &block.entries {
                    let KeyValue::Scalar { key, value } = entry else {
                        return Err(Error::InvalidKeydata);
                    };
                    let index = NumericValue::Bytes(value).get_int();
                    if (0..128).contains(&index) {
                        table[index as usize] = surfaces.surface_index(key);
                    }
                }
                result.physics.set_world_material_index_table(&table);
            } else if block.name.eq_ignore_ascii_case(b"virtualterrain")
                && !collision.displacements.is_empty()
            {
                return Err(Error::VirtualTerrain);
            }
        }
        Ok(result)
    }
    fn add_world_body(
        &mut self,
        model: &RigidModel,
        solid: usize,
        contents: u32,
        fluid: Option<u64>,
    ) -> Result<u64, Error> {
        let identity = self.next_body;
        self.next_body = identity
            .checked_add(1)
            .ok_or(Error::Physics(EnvironmentError::ClockOverflow))?;
        self.register(
            identity,
            BodyRule {
                owner: BodyOwner::World,
                entity_handle: None,
                contents,
                movable: false,
                static_body: true,
                shadow: false,
                push: false,
                solid: true,
            },
        )?;
        self.physics.create_body(model.body_input(
            identity,
            [0.0; 3],
            [0.0; 3],
            BodyKind::Static,
        ))?;
        let flags = self
            .physics
            .body(identity)
            .ok_or(Error::MissingWorld)?
            .callback_flags();
        self.physics.set_callback_flags(identity, flags | 0x0200)?;
        self.world_bodies.push(WorldBody {
            identity,
            solid,
            contents,
            fluid,
        });
        Ok(identity)
    }
    pub fn world_bodies(&self) -> &[WorldBody] {
        &self.world_bodies
    }
    pub fn physics(&self) -> &PhysicsEnvironment {
        &self.physics
    }
    pub fn surfaces(&self) -> &SurfacePropertyRegistry {
        &self.surfaces
    }
    pub fn game_material(&self, surface: u32) -> Option<u8> {
        self.surfaces
            .surface_data(surface as i32)
            .map(|record| record.game_material)
    }
    pub fn set_body_velocity(
        &mut self,
        body: u64,
        linear: Option<[f32; 3]>,
        angular: Option<[f32; 3]>,
    ) -> Result<(), Error> {
        self.physics.set_velocity(body, linear, angular)?;
        Ok(())
    }
    pub fn update_map_body(
        &mut self,
        body: u64,
        position: [f32; 3],
        angles: [f32; 3],
        solid: bool,
        arrival: f32,
    ) -> Result<(), Error> {
        let mut candidate = self.clone();
        let rule = candidate
            .policy_mut()
            .bodies
            .get_mut(&body)
            .ok_or(Error::MissingSolid)?;
        if !matches!(
            rule.owner,
            BodyOwner::MapEntity(_) | BodyOwner::BoneFollower { .. }
        ) {
            return Err(Error::MissingSolid);
        }
        let changed = rule.solid != solid;
        rule.solid = solid;
        let shadow = rule.shadow;
        if changed {
            candidate.physics.recheck_collision_filter(body)?;
        }
        if shadow {
            candidate
                .physics
                .update_shadow(body, position, angles, false, arrival)?;
        }
        *self = candidate;
        Ok(())
    }
    pub fn apply_projectile_force(
        &mut self,
        projectile: u32,
        force: [f32; 3],
        position: [f32; 3],
    ) -> Result<(), Error> {
        let body = self
            .projectile_body(projectile)
            .ok_or(Error::MissingProjectile)?;
        self.physics.apply_force_offset(body, force, position)?;
        Ok(())
    }
    fn policy(&self) -> &Policy {
        self.physics
            .collision_solver()
            .and_then(|solver| solver.as_any().downcast_ref())
            .expect("owned TF2 physical policy")
    }
    fn policy_mut(&mut self) -> &mut Policy {
        self.physics
            .collision_solver_mut()
            .and_then(|solver| solver.as_any_mut().downcast_mut())
            .expect("owned TF2 physical policy")
    }
    fn register(&mut self, body: u64, rule: BodyRule) -> Result<(), Error> {
        if self.policy_mut().insert(body, rule) {
            Ok(())
        } else {
            Err(Error::PolicyLimit)
        }
    }
    pub fn begin_frame(&mut self, game_time: f32) -> Result<(), Error> {
        if self.policy_mut().begin_frame(game_time) {
            Ok(())
        } else {
            Err(Error::Physics(EnvironmentError::NonFinite))
        }
    }
    pub fn simulate(&mut self) -> Result<(), Error> {
        let mut candidate = self.clone();
        candidate
            .physics
            .simulate(candidate.physics.config().timestep)?;
        if candidate.policy().rejected {
            return Err(Error::PolicyLimit);
        }
        *self = candidate;
        Ok(())
    }
    /// Runs after active-object publication and game touch processing.
    pub fn finish_frame(&mut self) -> Result<(), Error> {
        let mut candidate = self.clone();
        let sleep = candidate.policy_mut().finish_frame();
        if candidate.policy().rejected {
            return Err(Error::PolicyLimit);
        }
        for body in sleep {
            candidate.force_sleep_body(body)?;
        }
        *self = candidate;
        Ok(())
    }
    fn force_sleep_body(&mut self, body: u64) -> Result<(), Error> {
        if self
            .physics
            .body(body)
            .is_some_and(|body| body.is_moveable())
        {
            self.physics.clear_velocity_and_contact_strain(body)?;
            self.physics.sleep(body)?;
        }
        Ok(())
    }
    pub fn force_sleep_projectile(&mut self, projectile: u32) -> Result<(), Error> {
        let body = self
            .projectile_body(projectile)
            .ok_or(Error::MissingProjectile)?;
        let mut candidate = self.clone();
        candidate.force_sleep_body(body)?;
        *self = candidate;
        Ok(())
    }
    pub fn owner(&self, body: u64) -> Option<BodyOwner> {
        self.policy().bodies.get(&body).map(|rule| rule.owner)
    }
    pub fn projectile_body(&self, projectile: u32) -> Option<u64> {
        self.policy().bodies.iter().find_map(|(body, rule)| {
            (rule.owner == BodyOwner::Projectile(projectile)).then_some(*body)
        })
    }
    /// Normal grenade initialization followed by the pipebomb entity's angular impulse.
    pub fn create_projectile(
        &mut self,
        model: &RigidModel,
        input: ProjectileBodyInput,
    ) -> Result<u64, Error> {
        if self.projectile_body(input.projectile).is_some() {
            return Err(Error::DuplicateProjectile);
        }
        let mut candidate = self.clone();
        let identity = candidate.next_body;
        candidate.next_body = identity
            .checked_add(1)
            .ok_or(Error::Physics(EnvironmentError::ClockOverflow))?;
        candidate.register(
            identity,
            BodyRule {
                owner: BodyOwner::Projectile(input.projectile),
                entity_handle: None,
                contents: 1,
                movable: true,
                static_body: false,
                shadow: false,
                push: false,
                solid: true,
            },
        )?;
        candidate.physics.create_body(model.body_input(
            identity,
            input.position,
            input.angles,
            BodyKind::Dynamic,
        ))?;
        candidate.physics.wake(identity)?;
        candidate.physics.add_velocity(
            identity,
            Some(input.velocity),
            Some(input.angular_velocity),
        )?;
        if input.angular_velocity != [0.0; 3]
            && input
                .angular_velocity
                .iter()
                .all(|value| *value > -36000.0 && *value < 36000.0)
        {
            candidate
                .physics
                .add_velocity(identity, None, Some(input.angular_velocity))?;
        }
        if candidate.policy().rejected {
            return Err(Error::PolicyLimit);
        }
        *self = candidate;
        Ok(identity)
    }
    pub fn create_map_body(
        &mut self,
        model: &RigidModel,
        input: MapBodyInput,
    ) -> Result<u64, Error> {
        let MapBodyInput {
            entity,
            handle,
            kind,
            position,
            angles,
            solid,
        } = input;
        if handle == playsrc_entity::EntityHandle::NULL {
            return Err(Error::MissingEntity);
        }
        let mut candidate = self.clone();
        let identity = candidate.next_body;
        candidate.next_body = identity
            .checked_add(1)
            .ok_or(Error::Physics(EnvironmentError::ClockOverflow))?;
        let owner = match kind {
            MapBodyKind::BoneFollower(solid) => BodyOwner::BoneFollower { entity, solid },
            _ => BodyOwner::MapEntity(entity),
        };
        let contents = if kind != MapBodyKind::Static
            && model.contents() & playsrc_collision::MASK_WATER != 0
        {
            model.contents() | 1
        } else {
            model.contents()
        };
        candidate.register(
            identity,
            BodyRule {
                owner,
                entity_handle: Some(handle),
                contents,
                movable: kind != MapBodyKind::Static,
                static_body: kind == MapBodyKind::Static,
                shadow: kind != MapBodyKind::Static,
                push: matches!(kind, MapBodyKind::Shadow | MapBodyKind::BoneFollower(_)),
                solid,
            },
        )?;
        candidate.physics.create_body(model.body_input(
            identity,
            position,
            angles,
            if kind == MapBodyKind::Static {
                BodyKind::Static
            } else {
                BodyKind::Dynamic
            },
        ))?;
        if kind != MapBodyKind::Static {
            candidate
                .physics
                .set_shadow(identity, 10000.0, 10000.0, false, false)?;
            candidate
                .physics
                .update_shadow(identity, position, angles, false, 0.0)?;
            if matches!(kind, MapBodyKind::BoneFollower(_)) {
                let flags = candidate
                    .physics
                    .body(identity)
                    .ok_or(Error::MissingSolid)?
                    .callback_flags();
                candidate
                    .physics
                    .set_callback_flags(identity, flags | 0x0004)?;
                candidate.physics.set_gravity_enabled(identity, false)?;
            }
        }
        if candidate.policy().rejected {
            return Err(Error::PolicyLimit);
        }
        *self = candidate;
        Ok(identity)
    }
    pub fn destroy_map_body(&mut self, identity: u64) -> Result<(), Error> {
        if !matches!(
            self.owner(identity),
            Some(BodyOwner::MapEntity(_) | BodyOwner::BoneFollower { .. })
        ) {
            return Err(Error::MissingSolid);
        }
        self.physics.destroy_body(identity)?;
        self.policy_mut().bodies.remove(&identity);
        Ok(())
    }
    pub fn destroy_projectile(&mut self, projectile: u32) -> Result<(), Error> {
        let identity = self
            .projectile_body(projectile)
            .ok_or(Error::MissingProjectile)?;
        self.physics.destroy_body(identity)?;
        self.policy_mut().bodies.remove(&identity);
        Ok(())
    }
    pub fn set_projectile_solid(&mut self, projectile: u32, solid: bool) -> Result<(), Error> {
        let identity = self
            .projectile_body(projectile)
            .ok_or(Error::MissingProjectile)?;
        if self.policy().bodies[&identity].solid == solid {
            return Ok(());
        }
        let mut candidate = self.clone();
        candidate
            .policy_mut()
            .bodies
            .get_mut(&identity)
            .ok_or(Error::MissingProjectile)?
            .solid = solid;
        candidate.physics.recheck_collision_filter(identity)?;
        *self = candidate;
        Ok(())
    }
    pub fn cleanup_delete_list(&mut self) -> Result<(), Error> {
        self.physics.cleanup_delete_list().map_err(Into::into)
    }
    pub fn set_projectile_motion(&mut self, projectile: u32, enabled: bool) -> Result<(), Error> {
        let identity = self
            .projectile_body(projectile)
            .ok_or(Error::MissingProjectile)?;
        let mut candidate = self.clone();
        candidate
            .policy_mut()
            .bodies
            .get_mut(&identity)
            .ok_or(Error::MissingProjectile)?
            .movable = enabled;
        candidate.physics.set_motion_enabled(identity, enabled)?;
        *self = candidate;
        Ok(())
    }
}
