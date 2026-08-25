use crate::{Error, ErrorCode, error};
use playsrc_bsp::{Bsp, Leaf, LumpData, STATIC_PROP_USE_LIGHTING_ORIGIN};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticPropModel {
    pub source: usize,
    pub logical_path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StaticPropOccurrence {
    pub source: usize,
    pub model: usize,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub first_leaf: u16,
    pub leaves: Vec<u16>,
    pub solidity: u8,
    pub inert_padding: u8,
    pub skin: i32,
    pub fade_minimum: f32,
    pub fade_maximum: f32,
    pub lighting_origin: Option<[f32; 3]>,
    pub forced_fade_scale: f32,
    pub minimum_dx_level: u16,
    pub maximum_dx_level: u16,
    pub flags: u32,
    pub lightmap_resolution: [u16; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct StaticProps {
    pub source_version: u16,
    pub models: Vec<StaticPropModel>,
    pub leaf_reference_count: usize,
    pub occurrences: Vec<StaticPropOccurrence>,
}

pub(crate) fn compile(bsp: &Bsp) -> Result<StaticProps, Error> {
    let Some(source) = playsrc_bsp::parse_static_props(bsp, playsrc_bsp::Limits::default())
        .map_err(|_| error(ErrorCode::InvalidReference, Some(35)))?
    else {
        return Ok(StaticProps {
            source_version: playsrc_bsp::STATIC_PROP_VERSION,
            models: Vec::new(),
            leaf_reference_count: 0,
            occurrences: Vec::new(),
        });
    };
    let leaves = match &bsp.lumps[10].records {
        LumpData::Leaves(leaves) => leaves.as_slice(),
        _ => return Err(error(ErrorCode::MissingLump, Some(10))),
    };
    let mut models = Vec::with_capacity(source.dictionary.len());
    for model in source.dictionary {
        let path = std::str::from_utf8(&model.name)
            .map_err(|_| error(ErrorCode::InvalidReference, Some(model.index)))?
            .replace('\\', "/")
            .to_ascii_lowercase();
        if !path.starts_with("models/")
            || !path.ends_with(".mdl")
            || path.contains("//")
            || path.split('/').any(|component| component == "..")
            || path.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(error(ErrorCode::InvalidReference, Some(model.index)));
        }
        models.push(StaticPropModel {
            source: model.index,
            logical_path: path,
        });
    }
    let mut occurrences = Vec::with_capacity(source.occurrences.len());
    for prop in source.occurrences {
        let origin = vector(prop.origin);
        let angles = vector(prop.angles);
        let lighting_origin = prop.lighting_origin.map(vector);
        let fade_minimum = prop.fade_minimum.value();
        let fade_maximum = prop.fade_maximum.value();
        let forced_fade_scale = prop.forced_fade_scale.value();
        if origin
            .iter()
            .chain(angles.iter())
            .chain([fade_minimum, fade_maximum, forced_fade_scale].iter())
            .any(|value| !value.is_finite())
            || lighting_origin
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            || usize::from(prop.model) >= models.len()
            || (prop.flags & STATIC_PROP_USE_LIGHTING_ORIGIN != 0) != lighting_origin.is_some()
        {
            return Err(error(ErrorCode::NonFinite, Some(prop.index)));
        }
        validate_leaf_memberships(prop.index, &prop.leaves, leaves)?;
        occurrences.push(StaticPropOccurrence {
            source: prop.index,
            model: usize::from(prop.model),
            origin,
            angles,
            first_leaf: prop.first_leaf,
            leaves: prop.leaves,
            solidity: prop.solidity,
            inert_padding: prop.padding,
            skin: prop.skin,
            fade_minimum,
            fade_maximum,
            lighting_origin,
            forced_fade_scale,
            minimum_dx_level: prop.minimum_dx_level,
            maximum_dx_level: prop.maximum_dx_level,
            flags: prop.flags,
            lightmap_resolution: prop.lightmap_resolution,
        });
    }
    Ok(StaticProps {
        source_version: source.version,
        models,
        leaf_reference_count: source.leaf_references.len(),
        occurrences,
    })
}

fn validate_leaf_memberships(
    source: usize,
    membership: &[u16],
    leaves: &[Leaf],
) -> Result<(), Error> {
    if membership.is_empty()
        || membership.iter().any(|index| {
            leaves
                .get(usize::from(*index))
                .is_none_or(|leaf| leaf.contents as u32 & playsrc_collision::CONTENTS_SOLID != 0)
        })
    {
        return Err(error(ErrorCode::InvalidReference, Some(source)));
    }
    Ok(())
}

fn vector(value: playsrc_bsp::Vector3) -> [f32; 3] {
    [value.x.value(), value.y.value(), value.z.value()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(contents: i32) -> playsrc_bsp::Leaf {
        playsrc_bsp::Leaf {
            contents,
            cluster: 0,
            area_and_flags: 1,
            mins: [-1; 3],
            maxs: [1; 3],
            first_leaf_face: 0,
            leaf_face_count: 0,
            first_leaf_brush: 0,
            leaf_brush_count: 0,
            leaf_water_data_id: -1,
            padding: 0,
            ambient_cube: None,
        }
    }

    #[test]
    fn static_props_reject_empty_missing_and_solid_bsp_leaf_memberships() {
        let leaves = [leaf(0), leaf(playsrc_collision::CONTENTS_SOLID as i32)];
        assert!(validate_leaf_memberships(4, &[0, 0], &leaves).is_ok());
        assert_eq!(
            validate_leaf_memberships(4, &[], &leaves),
            Err(error(ErrorCode::InvalidReference, Some(4)))
        );
        assert_eq!(
            validate_leaf_memberships(4, &[2], &leaves),
            Err(error(ErrorCode::InvalidReference, Some(4)))
        );
        assert_eq!(
            validate_leaf_memberships(4, &[1], &leaves),
            Err(error(ErrorCode::InvalidReference, Some(4)))
        );
    }
}
