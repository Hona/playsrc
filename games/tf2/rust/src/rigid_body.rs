use playsrc_collision::{PhysicalModel, PhysicsShape, SnapshotLimits};
use playsrc_material::SurfacePropertyRegistry;
use playsrc_phy::{Asset, SolidProperties};
use playsrc_physics::{BodyInput, BodyKind};
use std::{fmt, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub struct RigidModel {
    shape: Arc<PhysicsShape>,
    material: u32,
    mass: f32,
    inertia_factor: f32,
    rotational_inertia_limit: f32,
    damping: f32,
    rotational_damping: f32,
    drag: f32,
    volume: f32,
    contents: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RigidModelError {
    MissingSolid,
    InvalidSolid,
    MassCenterOverride,
    Geometry,
}
impl fmt::Display for RigidModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingSolid => "model physics requires an authored solid block",
            Self::InvalidSolid => "authored model solid index or parameters are invalid",
            Self::MassCenterOverride => {
                "model mass-center override requires physical frame support"
            }
            Self::Geometry => "authored model collision geometry is invalid",
        })
    }
}
impl std::error::Error for RigidModelError {}
impl RigidModel {
    /// Model-based creation selects the first or explicitly indexed authored solid. Defaults follow
    /// the Source SDK's `g_PhysDefaultObjectParams` and `PhysModelCreate` path.
    pub fn compile(
        identity: u64,
        asset: &Asset,
        solid_index: Option<i32>,
        surfaces: &SurfacePropertyRegistry,
        limits: SnapshotLimits,
    ) -> Result<Self, RigidModelError> {
        Self::compile_with_defaults(
            identity,
            asset,
            solid_index,
            surfaces,
            limits,
            ModelParameters::model_defaults(),
        )
    }
    /// Ragdoll-derived prop followers supply `ParseSolid` results without a defaults handler.
    /// The caller selects each authored solid in order; this does not create ragdoll constraints.
    pub fn compile_ragdoll_follower(
        identity: u64,
        asset: &Asset,
        solid_index: i32,
        surfaces: &SurfacePropertyRegistry,
        limits: SnapshotLimits,
    ) -> Result<Self, RigidModelError> {
        Self::compile_with_defaults(
            identity,
            asset,
            Some(solid_index),
            surfaces,
            limits,
            ModelParameters::zeroed(),
        )
    }
    fn compile_with_defaults(
        identity: u64,
        asset: &Asset,
        solid_index: Option<i32>,
        surfaces: &SurfacePropertyRegistry,
        limits: SnapshotLimits,
        defaults: ModelParameters,
    ) -> Result<Self, RigidModelError> {
        let solid = asset
            .key_data
            .solid_properties(solid_index)
            .map_err(|_| RigidModelError::InvalidSolid)?
            .ok_or(RigidModelError::MissingSolid)?;
        let index = usize::try_from(solid.index).map_err(|_| RigidModelError::InvalidSolid)?;
        if index >= asset.solids.len() {
            return Err(RigidModelError::InvalidSolid);
        }
        let parameters = defaults.apply(solid)?;
        let contents = 1;
        let material = surfaces.surface_index(solid.surface_property) as u32;
        let shape = PhysicsShape::from_phy(identity, asset, index, limits, |_| contents)
            .map_err(|_| RigidModelError::Geometry)?;
        Ok(Self::from_parameters(
            Arc::new(shape),
            material,
            contents,
            parameters,
        ))
    }
    /// Reuses the collision owner's already-decoded brush model instead of parsing PHY again.
    pub fn compile_brush(
        model: &PhysicalModel,
        surfaces: &SurfacePropertyRegistry,
    ) -> Result<Self, RigidModelError> {
        let solid = model
            .key_data
            .solid_properties(None)
            .map_err(|_| RigidModelError::InvalidSolid)?
            .ok_or(RigidModelError::MissingSolid)?;
        let index = usize::try_from(solid.index).map_err(|_| RigidModelError::InvalidSolid)?;
        let geometry = model
            .solids
            .iter()
            .find(|shape| shape.solid == index)
            .ok_or(RigidModelError::MissingSolid)?;
        let parameters = ModelParameters::model_defaults().apply(solid)?;
        Ok(Self::from_parameters(
            Arc::clone(&geometry.shape),
            surfaces.surface_index(solid.surface_property) as u32,
            geometry.contents,
            parameters,
        ))
    }
    fn from_parameters(
        shape: Arc<PhysicsShape>,
        material: u32,
        contents: u32,
        parameters: ModelParameters,
    ) -> Self {
        Self {
            shape,
            material,
            mass: parameters.mass,
            inertia_factor: parameters.inertia_factor,
            rotational_inertia_limit: parameters.rotational_inertia_limit,
            damping: parameters.damping,
            rotational_damping: parameters.rotational_damping,
            drag: parameters.drag,
            volume: parameters.volume,
            contents,
        }
    }
    pub(crate) fn world_piece(
        shape: Arc<PhysicsShape>,
        solid: Option<SolidProperties<'_>>,
        material: u32,
        contents: u32,
    ) -> Result<Self, RigidModelError> {
        let defaults = ModelParameters::model_defaults();
        let parameters = match solid {
            Some(solid) => defaults.apply(solid)?,
            None => defaults,
        };
        Ok(Self::from_parameters(shape, material, contents, parameters))
    }
    pub fn shape(&self) -> &Arc<PhysicsShape> {
        &self.shape
    }
    pub fn contents(&self) -> u32 {
        self.contents
    }
    pub fn body_input(
        &self,
        identity: u64,
        position: [f32; 3],
        angles: [f32; 3],
        kind: BodyKind,
    ) -> BodyInput {
        BodyInput {
            identity,
            shape: Arc::clone(&self.shape),
            kind,
            material: self.material,
            position,
            angles,
            mass: self.mass,
            inertia_factor: self.inertia_factor,
            rotational_inertia_limit: self.rotational_inertia_limit,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            linear_damping: self.damping,
            angular_damping: self.rotational_damping,
            drag: self.drag,
            volume: self.volume,
            collisions_enabled: true,
            gravity_enabled: true,
            drag_enabled: self.drag != 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ModelParameters {
    mass: f32,
    inertia_factor: f32,
    rotational_inertia_limit: f32,
    damping: f32,
    rotational_damping: f32,
    drag: f32,
    volume: f32,
}
impl ModelParameters {
    fn model_defaults() -> Self {
        Self {
            mass: 1.0,
            inertia_factor: 1.0,
            rotational_inertia_limit: 0.05,
            damping: 0.1,
            rotational_damping: 0.1,
            drag: 1.0,
            volume: 0.0,
        }
    }
    fn zeroed() -> Self {
        Self {
            mass: 0.0,
            inertia_factor: 0.0,
            rotational_inertia_limit: 0.0,
            damping: 0.0,
            rotational_damping: 0.0,
            drag: 0.0,
            volume: 0.0,
        }
    }
    fn apply(self, solid: SolidProperties<'_>) -> Result<Self, RigidModelError> {
        if solid.mass_center_override.is_some() {
            return Err(RigidModelError::MassCenterOverride);
        }
        let result = Self {
            mass: solid.mass.map_or(self.mass, |v| f32::from_bits(v.0)),
            inertia_factor: solid
                .inertia_factor
                .map_or(self.inertia_factor, |v| f32::from_bits(v.0)),
            rotational_inertia_limit: self.rotational_inertia_limit,
            damping: solid.damping.map_or(self.damping, |v| f32::from_bits(v.0)),
            rotational_damping: solid
                .rotational_damping
                .map_or(self.rotational_damping, |v| f32::from_bits(v.0)),
            drag: solid.drag.map_or(self.drag, |v| f32::from_bits(v.0)),
            volume: solid.volume.map_or(self.volume, |v| f32::from_bits(v.0)),
        };
        if [
            result.mass,
            result.inertia_factor,
            result.damping,
            result.rotational_damping,
            result.drag,
            result.volume,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err(RigidModelError::InvalidSolid);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty() -> SolidProperties<'static> {
        SolidProperties {
            index: 0,
            name: b"model",
            parent: b"",
            surface_property: b"",
            mass: None,
            inertia_factor: None,
            damping: None,
            rotational_damping: None,
            volume: None,
            drag: None,
            contents: None,
            mass_center_override: None,
        }
    }
    #[test]
    fn ragdoll_follower_parser_does_not_inherit_model_creation_defaults() {
        let defaults = ModelParameters::zeroed();
        assert_eq!(defaults.apply(empty()).unwrap(), defaults);
        let mut properties = empty();
        properties.mass = Some(playsrc_phy::Float32(13.546261_f32.to_bits()));
        properties.inertia_factor = Some(playsrc_phy::Float32(10.0_f32.to_bits()));
        properties.rotational_damping = Some(playsrc_phy::Float32(9.0_f32.to_bits()));
        let result = defaults.apply(properties).unwrap();
        assert_eq!(result.mass, 13.546261);
        assert_eq!(result.inertia_factor, 10.0);
        assert_eq!(result.rotational_damping, 9.0);
        assert_eq!(result.drag, 0.0);
        assert_eq!(result.rotational_inertia_limit, 0.0);
        assert_eq!(result.damping, 0.0);
    }
    #[test]
    fn model_creation_defaults_remain_distinct_from_authored_zero_overrides() {
        assert_eq!(
            ModelParameters::model_defaults().apply(empty()).unwrap(),
            ModelParameters {
                mass: 1.0,
                inertia_factor: 1.0,
                rotational_inertia_limit: 0.05,
                damping: 0.1,
                rotational_damping: 0.1,
                drag: 1.0,
                volume: 0.0
            }
        );
        let mut properties = empty();
        properties.damping = Some(playsrc_phy::Float32(0));
        properties.drag = Some(playsrc_phy::Float32(0));
        properties.mass = Some(playsrc_phy::Float32(5.0_f32.to_bits()));
        let actual = ModelParameters::model_defaults().apply(properties).unwrap();
        assert_eq!(actual.mass, 5.0);
        assert_eq!(actual.damping, 0.0);
        assert_eq!(actual.drag, 0.0);
        properties.mass = Some(playsrc_phy::Float32(f32::INFINITY.to_bits()));
        assert_eq!(
            ModelParameters::model_defaults().apply(properties),
            Err(RigidModelError::InvalidSolid)
        );
        properties = empty();
        properties.mass_center_override = Some(b"1 2 3");
        assert_eq!(
            ModelParameters::model_defaults().apply(properties),
            Err(RigidModelError::MassCenterOverride)
        );
    }
}
