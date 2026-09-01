use playsrc_material::{PhysicalSurfaceProperties, SurfacePropertyRegistry};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceError {
    MissingDefault,
    NonFinite,
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDefault => {
                formatter.write_str("physical surface registry has no default record")
            }
            Self::NonFinite => {
                formatter.write_str("physical surface contains a non-finite property")
            }
        }
    }
}

impl std::error::Error for SurfaceError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfacePair {
    pub identities: [u32; 2],
    pub surfaces: [PhysicalSurfaceProperties; 2],
    pub friction: f32,
    pub elasticity: f32,
}

impl SurfacePair {
    pub fn from_registry(
        registry: &SurfacePropertyRegistry,
        identities: [u32; 2],
    ) -> Result<Self, SurfaceError> {
        let records = [
            registry
                .surface_data(identities[0] as i32)
                .ok_or(SurfaceError::MissingDefault)?,
            registry
                .surface_data(identities[1] as i32)
                .ok_or(SurfaceError::MissingDefault)?,
        ];
        let surfaces = records.map(|record| record.physics);
        if surfaces.iter().any(|surface| {
            !surface.friction.is_finite()
                || !surface.elasticity.is_finite()
                || !surface.density.is_finite()
                || !surface.thickness.is_finite()
                || !surface.dampening.is_finite()
        }) {
            return Err(SurfaceError::NonFinite);
        }
        Ok(Self {
            identities: records.map(|record| record.index),
            friction: (f64::from(surfaces[0].friction) * f64::from(surfaces[1].friction))
                .clamp(0.0, 1.0) as f32,
            elasticity: (f64::from(surfaces[0].elasticity) * f64::from(surfaces[1].elasticity))
                .clamp(0.0, 1.0) as f32,
            surfaces,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SurfacePair;
    use playsrc_material::{SurfacePropertyFile, SurfacePropertyRegistry};

    fn registry() -> SurfacePropertyRegistry {
        SurfacePropertyRegistry::compile(&[SurfacePropertyFile {
            logical_path: "scripts/surfaceproperties.txt",
            bytes: br#"
                default { friction .8 elasticity .25 density 2000 }
                slippery { friction .1 elasticity .5 thickness .125 dampening 6 }
                extreme { friction 2 elasticity 1000 }
                negative { friction -1 elasticity -.5 }
            "#,
        }])
        .unwrap()
    }

    #[test]
    fn paired_surfaces_preserve_exact_product_rounding_and_source_properties() {
        let registry = registry();
        let pair = SurfacePair::from_registry(&registry, [0, 0]).unwrap();
        assert_eq!(pair.identities, [0, 0]);
        assert_eq!(pair.friction.to_bits(), 0x3f23_d70b);
        assert_eq!(pair.elasticity.to_bits(), 0x3d80_0000);
        assert_eq!(pair.surfaces[0].density, 2000.0);
        let mixed = SurfacePair::from_registry(&registry, [1, 0]).unwrap();
        assert_eq!(mixed.friction.to_bits(), (0.1_f32 * 0.8_f32).to_bits());
        assert_eq!(mixed.surfaces[0].thickness.to_bits(), 0.125_f32.to_bits());
        assert_eq!(mixed.surfaces[0].dampening.to_bits(), 6.0_f32.to_bits());
    }

    #[test]
    fn paired_surfaces_clamp_products_and_resolve_invalid_indices_to_default() {
        let registry = registry();
        let extreme = SurfacePair::from_registry(&registry, [2, 2]).unwrap();
        assert_eq!(extreme.friction, 1.0);
        assert_eq!(extreme.elasticity, 1.0);
        let negative = SurfacePair::from_registry(&registry, [3, 0]).unwrap();
        assert_eq!(negative.friction, 0.0);
        assert_eq!(negative.elasticity, 0.0);
        let fallback = SurfacePair::from_registry(&registry, [u32::MAX, 0]).unwrap();
        assert_eq!(fallback.identities, [0, 0]);
        assert_eq!(fallback.friction.to_bits(), 0x3f23_d70b);
    }
}
