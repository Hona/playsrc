use crate::{Error, ErrorCode, Float32, KeyData, KeyValue, err};
use playsrc_keyvalues::NumericValue;

/// Authored overrides. Object defaults and contents policy belong to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolidProperties<'a> {
    pub index: i32,
    pub name: &'a [u8],
    pub parent: &'a [u8],
    pub surface_property: &'a [u8],
    pub mass: Option<Float32>,
    pub inertia_factor: Option<Float32>,
    pub damping: Option<Float32>,
    pub rotational_damping: Option<Float32>,
    pub volume: Option<Float32>,
    pub drag: Option<Float32>,
    pub contents: Option<u32>,
    pub mass_center_override: Option<&'a [u8]>,
}

impl KeyData {
    /// `None` selects the first solid block, independently of its declared index.
    pub fn solid_properties(
        &self,
        index: Option<i32>,
    ) -> Result<Option<SolidProperties<'_>>, Error> {
        for block in self
            .blocks
            .iter()
            .filter(|block| block.name.eq_ignore_ascii_case(b"solid"))
        {
            let solid = self.parse_solid(block)?;
            if index.is_none_or(|index| index == solid.index) {
                return Ok(Some(solid));
            }
        }
        Ok(None)
    }
    /// Projects one ordered model/world block without changing the caller's selection policy.
    pub fn solid_properties_at(&self, block: usize) -> Result<Option<SolidProperties<'_>>, Error> {
        let Some(block) = self.blocks.get(block) else {
            return Ok(None);
        };
        if !block.name.eq_ignore_ascii_case(b"solid")
            && !block.name.eq_ignore_ascii_case(b"staticsolid")
        {
            return Ok(None);
        }
        self.parse_solid(block).map(Some)
    }
    fn parse_solid<'a>(&self, block: &'a crate::KeyBlock) -> Result<SolidProperties<'a>, Error> {
        let mut solid = SolidProperties {
            index: 0,
            name: &[],
            parent: &[],
            surface_property: &[],
            mass: None,
            inertia_factor: None,
            damping: None,
            rotational_damping: None,
            volume: None,
            drag: None,
            contents: None,
            mass_center_override: None,
        };
        for entry in &block.entries {
            let KeyValue::Scalar { key, value } = entry else {
                return Err(err(
                    ErrorCode::InvalidKeydata,
                    self.document_range.clone(),
                    None,
                ));
            };
            let text = &value[..value.len().min(511)];
            let number = || Float32(NumericValue::Bytes(value).get_float().to_bits());
            if key.eq_ignore_ascii_case(b"index") {
                solid.index = NumericValue::Bytes(value).get_int();
            } else if key.eq_ignore_ascii_case(b"name") {
                solid.name = text;
            } else if key.eq_ignore_ascii_case(b"parent") {
                solid.parent = text;
            } else if key.eq_ignore_ascii_case(b"surfaceprop") {
                solid.surface_property = text;
            } else if key.eq_ignore_ascii_case(b"mass") {
                solid.mass = Some(number());
            } else if key.eq_ignore_ascii_case(b"inertia") {
                solid.inertia_factor = Some(number());
            } else if key.eq_ignore_ascii_case(b"damping") {
                solid.damping = Some(number());
            } else if key.eq_ignore_ascii_case(b"rotdamping") {
                solid.rotational_damping = Some(number());
            } else if key.eq_ignore_ascii_case(b"volume") {
                solid.volume = Some(number());
            } else if key.eq_ignore_ascii_case(b"drag") {
                solid.drag = Some(number());
            } else if key.eq_ignore_ascii_case(b"contents") {
                solid.contents = Some(NumericValue::Bytes(value).get_int() as u32);
            } else if key.eq_ignore_ascii_case(b"massCenterOverride") {
                solid.mass_center_override = Some(value);
            }
        }
        Ok(solid)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn solid_projection_keeps_selection_order_absence_prefix_numbers_and_name_width() {
        let mut text =
            b"unknown { value 1 } SoLiD { index 2 mass 3 mass 5.5tail contents -1 name \"".to_vec();
        text.extend(vec![b'a'; 600]);
        text.extend_from_slice(b"\" surfaceprop Metal volume 12.25 drag .5 } solid { index 0 parent root massCenterOverride \"1 2 3\" }\0");
        let limits = Limits::default();
        let mut budget = RetainedBudget::new(limits, text.len());
        let data = parse_keydata(&text, 0, limits, &mut budget).unwrap();
        let first = data.solid_properties(None).unwrap().unwrap();
        assert_eq!(first.index, 2);
        assert_eq!(first.name.len(), 511);
        assert_eq!(first.mass, Some(Float32(5.5_f32.to_bits())));
        assert_eq!(first.contents, Some(u32::MAX));
        assert_eq!(first.surface_property, b"Metal");
        assert_eq!(first.damping, None);
        let zero = data.solid_properties(Some(0)).unwrap().unwrap();
        assert_eq!(zero.parent, b"root");
        assert_eq!(zero.mass, None);
        assert_eq!(zero.mass_center_override, Some(b"1 2 3".as_slice()));
        assert!(data.solid_properties(Some(4)).unwrap().is_none());
    }
    #[test]
    fn world_solid_projection_preserves_block_order_without_selecting_fluids() {
        let text=b"staticsolid { index 0 contents 33570827 } fluid { index 2 } staticsolid { index 1 contents 65536 } solid { index 0 mass 3 }\0";
        let limits = Limits::default();
        let mut budget = RetainedBudget::new(limits, text.len());
        let data = parse_keydata(text, 0, limits, &mut budget).unwrap();
        assert_eq!(
            data.solid_properties_at(0).unwrap().unwrap().contents,
            Some(33570827)
        );
        assert!(data.solid_properties_at(1).unwrap().is_none());
        assert!(data.solid_properties_at(4).unwrap().is_none());
        assert_eq!(data.solid_properties_at(2).unwrap().unwrap().index, 1);
        assert_eq!(
            data.solid_properties(None).unwrap().unwrap().mass,
            Some(Float32(3.0_f32.to_bits()))
        );
    }
}
