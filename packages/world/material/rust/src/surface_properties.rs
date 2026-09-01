use playsrc_keyvalues::{EscapeMode, Limits, NumericValue, Value, parse_text};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct SurfacePropertyFile<'a> {
    pub logical_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalSurfaceProperties {
    pub friction: f32,
    pub elasticity: f32,
    pub density: f32,
    pub thickness: f32,
    pub dampening: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceImpactAudio {
    pub hard_sound: Option<Vec<u8>>,
    pub soft_sound: Option<Vec<u8>>,
    pub hardness: f32,
    pub hard_threshold: f32,
    pub hard_velocity_threshold: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfacePropertyRecord {
    pub index: u32,
    pub name: Vec<u8>,
    pub source_file: usize,
    pub source_record: usize,
    pub game_material: u8,
    pub bullet_impact: Option<Vec<u8>>,
    pub audio_reflectivity_bits: u32,
    pub impact_audio: SurfaceImpactAudio,
    pub physics: PhysicalSurfaceProperties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfacePropertyRegistry {
    pub identity: [u8; 32],
    pub records: Vec<SurfacePropertyRecord>,
    lookup: BTreeMap<Vec<u8>, u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfacePropertyError {
    InvalidInput,
    InvalidDocument,
    MissingDefault,
    Limit,
}

impl SurfacePropertyRegistry {
    pub fn surface_index(&self, name: &[u8]) -> i32 {
        if name.eq_ignore_ascii_case(b"$MATERIAL_INDEX_SHADOW") {
            return 0xf000;
        }
        self.lookup
            .get(&name.to_ascii_lowercase())
            .map_or(-1, |index| *index as i32)
    }
    pub fn surface_data(&self, index: i32) -> Option<&SurfacePropertyRecord> {
        let index = if index == 0xf000 {
            self.lookup
                .get(b"$material_index_shadow".as_slice())
                .copied()
                .unwrap_or(0) as i32
        } else if index > 127 {
            0
        } else {
            index
        };
        usize::try_from(index)
            .ok()
            .and_then(|index| self.records.get(index))
            .or_else(|| self.records.first())
    }
    pub fn compile(files: &[SurfacePropertyFile<'_>]) -> Result<Self, SurfacePropertyError> {
        if files.is_empty() || files.len() > 4_096 {
            return Err(SurfacePropertyError::Limit);
        }
        let mut digest = Sha256::new();
        digest.update(b"playsrc-surface-property-registry-v1\0");
        let mut records = Vec::<SurfacePropertyRecord>::new();
        let mut lookup = BTreeMap::<Vec<u8>, u32>::new();
        for (source_file, file) in files.iter().enumerate() {
            digest.update((file.logical_path.len() as u64).to_le_bytes());
            digest.update(file.logical_path.as_bytes());
            digest.update((file.bytes.len() as u64).to_le_bytes());
            digest.update(file.bytes);
            let document = parse_text(file.bytes, EscapeMode::Escaped, Limits::default())
                .map_err(|_| SurfacePropertyError::InvalidDocument)?;
            for (source_record, node) in document.roots.iter().enumerate() {
                let Value::Object(fields) = &node.value else {
                    return Err(SurfacePropertyError::InvalidDocument);
                };
                if node.condition.is_some() {
                    return Err(SurfacePropertyError::InvalidDocument);
                }
                let normalized = node.key.bytes.to_ascii_lowercase();
                if normalized.is_empty() {
                    return Err(SurfacePropertyError::InvalidInput);
                }
                let existing = lookup.get(&normalized).copied();
                let inherited = existing.or_else(|| lookup.get(b"default".as_slice()).copied());
                let mut audio_reflectivity_bits = inherited
                    .and_then(|index| records.get(index as usize))
                    .map_or(0, |record| record.audio_reflectivity_bits);
                let mut physics = inherited
                    .and_then(|index| records.get(index as usize))
                    .map_or_else(PhysicalSurfaceProperties::default, |record| record.physics);
                let mut game_material = inherited
                    .and_then(|index| records.get(index as usize))
                    .map_or(b'C', |record| record.game_material);
                let mut bullet_impact = inherited
                    .and_then(|index| records.get(index as usize))
                    .and_then(|record| record.bullet_impact.clone());
                let mut impact_audio = inherited.and_then(|index| records.get(index as usize))
                    .map_or_else(SurfaceImpactAudio::default, |record| record.impact_audio.clone());

                for field in fields {
                    if field.condition.is_some() {
                        return Err(SurfacePropertyError::InvalidDocument);
                    }
                    let Value::Scalar(value) = &field.value else {
                        return Err(SurfacePropertyError::InvalidDocument);
                    };
                    if field.key.bytes.eq_ignore_ascii_case(b"base") {
                        if let Some(index) = lookup.get(&value.token.bytes.to_ascii_lowercase()) {
                            let base = records
                                .get(*index as usize)
                                .ok_or(SurfacePropertyError::InvalidInput)?;
                            physics = base.physics;
                            game_material = base.game_material;
                            bullet_impact = base.bullet_impact.clone();
                            audio_reflectivity_bits = base.audio_reflectivity_bits;
                            impact_audio = base.impact_audio.clone();
                        }
                        continue;
                    }
                    if field.key.bytes.eq_ignore_ascii_case(b"audioReflectivity") {
                        let reflectivity = NumericValue::Bytes(&value.token.bytes).get_float();
                        if !reflectivity.is_finite() {
                            return Err(SurfacePropertyError::InvalidDocument);
                        }
                        audio_reflectivity_bits = reflectivity.to_bits();
                        continue;
                    }
                    if field.key.bytes.eq_ignore_ascii_case(b"gamematerial") {
                        let raw = &value.token.bytes;
                        game_material = if raw.len() == 1 && !raw[0].is_ascii_digit() {
                            raw[0].to_ascii_uppercase()
                        } else {
                            std::str::from_utf8(raw)
                                .ok()
                                .and_then(|value| value.parse::<u8>().ok())
                                .ok_or(SurfacePropertyError::InvalidDocument)?
                        };
                        continue;
                    }
                    if field.key.bytes.eq_ignore_ascii_case(b"bulletimpact") {
                        bullet_impact = Some(value.token.bytes.to_ascii_lowercase());
                        continue;
                    }
                    if field.key.bytes.eq_ignore_ascii_case(b"impacthard") {
                        impact_audio.hard_sound = Some(value.token.bytes.to_ascii_lowercase());
                        continue;
                    }
                    if field.key.bytes.eq_ignore_ascii_case(b"impactsoft") {
                        impact_audio.soft_sound = Some(value.token.bytes.to_ascii_lowercase());
                        continue;
                    }
                    let destination = if field.key.bytes.eq_ignore_ascii_case(b"friction") {
                        Some(&mut physics.friction)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"elasticity") {
                        Some(&mut physics.elasticity)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"density") {
                        Some(&mut physics.density)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"thickness") {
                        Some(&mut physics.thickness)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"dampening") {
                        Some(&mut physics.dampening)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"audioHardnessFactor") {
                        Some(&mut impact_audio.hardness)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"impactHardThreshold") {
                        Some(&mut impact_audio.hard_threshold)
                    } else if field.key.bytes.eq_ignore_ascii_case(b"audioHardMinVelocity") {
                        Some(&mut impact_audio.hard_velocity_threshold)
                    } else {
                        None
                    };
                    if let Some(destination) = destination {
                        *destination = NumericValue::Bytes(&value.token.bytes).get_float();
                    }
                }

                if let Some(index) = existing {
                    let record = records
                        .get_mut(index as usize)
                        .ok_or(SurfacePropertyError::InvalidInput)?;
                    record.source_file = source_file;
                    record.source_record = source_record;
                    record.audio_reflectivity_bits = audio_reflectivity_bits;
                    record.physics = physics;
                    record.game_material = game_material;
                    record.bullet_impact = bullet_impact;
                    record.impact_audio = impact_audio;
                } else {
                    let index =
                        u32::try_from(records.len()).map_err(|_| SurfacePropertyError::Limit)?;
                    lookup.insert(normalized.clone(), index);
                    records.push(SurfacePropertyRecord {
                        index,
                        name: normalized,
                        source_file,
                        source_record,
                        audio_reflectivity_bits,
                        game_material,
                        bullet_impact,
                        physics,
                        impact_audio,
                    });
                }
            }

            if source_file == 0 {
                let inherited = lookup
                    .get(b"default".as_slice())
                    .and_then(|index| records.get(*index as usize));
                let mut physics = inherited
                    .map_or_else(PhysicalSurfaceProperties::default, |record| record.physics);
                physics.friction = 0.8;
                physics.elasticity = 0.001;
                let game_material = inherited.map_or(b'C', |record| record.game_material);
                let bullet_impact = inherited.and_then(|record| record.bullet_impact.clone());
                let audio_reflectivity_bits =
                    inherited.map_or(0, |record| record.audio_reflectivity_bits);
                let index =
                    u32::try_from(records.len()).map_err(|_| SurfacePropertyError::Limit)?;
                let name = b"$MATERIAL_INDEX_SHADOW".to_vec();
                lookup.insert(name.to_ascii_lowercase(), index);
                records.push(SurfacePropertyRecord {
                    index,
                    name,
                    source_file,
                    source_record: document.roots.len(),
                    game_material,
                    bullet_impact,
                    physics,
                    audio_reflectivity_bits,
                    impact_audio: inherited.map_or_else(SurfaceImpactAudio::default, |record| record.impact_audio.clone()),
                });
            }
        }
        if !lookup.contains_key(b"default".as_slice()) {
            return Err(SurfacePropertyError::MissingDefault);
        }
        Ok(Self {
            identity: digest.finalize().into(),
            records,
            lookup,
        })
    }

    pub fn resolve(&self, name: Option<&[u8]>) -> Option<&SurfacePropertyRecord> {
        let name = name?;
        let index = self
            .lookup
            .get(&name.to_ascii_lowercase())
            .or_else(|| self.lookup.get(b"default".as_slice()))?;
        self.records.get(*index as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impact_audio_inherits_and_applies_fields_in_file_order() {
        let registry = SurfacePropertyRegistry::compile(&[SurfacePropertyFile {
            logical_path: "surfaces.txt",
            bytes: br#"default { impacthard "Default.Hard" impactsoft "Default.Soft" audioHardnessFactor .25 impactHardThreshold .5 audioHardMinVelocity 80 }
                metal { audioHardnessFactor .9 impacthard "Metal.Hard" }
                late { impacthard "discarded" base metal audioHardMinVelocity 100 }
                metal { impactsoft "Metal.Soft" }"#,
        }]).unwrap();
        let metal = &registry.resolve(Some(b"metal")).unwrap().impact_audio;
        assert_eq!(metal.hard_sound.as_deref(), Some(b"metal.hard".as_slice()));
        assert_eq!(metal.soft_sound.as_deref(), Some(b"metal.soft".as_slice()));
        assert_eq!((metal.hardness, metal.hard_threshold, metal.hard_velocity_threshold), (0.9, 0.5, 80.0));
        let late = &registry.resolve(Some(b"late")).unwrap().impact_audio;
        assert_eq!(late.hard_sound.as_deref(), Some(b"metal.hard".as_slice()));
        assert_eq!(late.soft_sound.as_deref(), Some(b"default.soft".as_slice()));
        assert_eq!(late.hard_velocity_threshold, 100.0);
    }

    #[test]
    fn reserved_tokens_are_not_ordinary_record_indices() {
        let registry = SurfacePropertyRegistry::compile(&[SurfacePropertyFile {
            logical_path: "surfaces.txt",
            bytes: b"default { friction .2 } metal { density 5000 }",
        }])
        .unwrap();
        assert_eq!(registry.surface_index(b"MeTaL"), 1);
        assert_eq!(registry.surface_index(b"missing"), -1);
        assert_eq!(registry.surface_index(b"$material_index_shadow"), 0xf000);
        let shadow = registry.surface_data(0xf000).unwrap();
        assert_eq!(shadow.index, 2);
        assert_eq!(shadow.physics.friction, 0.8);
        assert_eq!(shadow.physics.elasticity, 0.001);
        for index in [-1, 127, 128, 0xefff, 0xffff] {
            assert_eq!(registry.surface_data(index).unwrap().index, 0);
        }
        let mut large = registry.clone();
        for index in 3..130 {
            let mut record = large.records[1].clone();
            record.index = index;
            large.records.push(record);
        }
        assert_eq!(large.surface_data(127).unwrap().index, 127);
        assert_eq!(large.surface_data(128).unwrap().index, 0);
    }

    #[test]
    fn acoustic_reflectivity_uses_parse_order_base_copy_and_partial_overrides() {
        let registry = SurfacePropertyRegistry::compile(&[
            SurfacePropertyFile {
                logical_path: "first.txt",
                bytes: br#"default { audioReflectivity 0.66 }
                metal { audioReflectivity 0.83 } copied { audioReflectivity 0.2 base metal }
                inherited { base metal audioReflectivity 0.25 }"#,
            },
            SurfacePropertyFile {
                logical_path: "second.txt",
                bytes: br#"metal { friction 0.8 } default { audioReflectivity 0.5 }"#,
            },
        ])
        .unwrap();
        for (name, expected) in [
            (b"metal".as_slice(), 0.83_f32),
            (b"copied", 0.83),
            (b"inherited", 0.25),
            (b"default", 0.5),
        ] {
            assert_eq!(
                registry
                    .resolve(Some(name))
                    .unwrap()
                    .audio_reflectivity_bits,
                expected.to_bits()
            );
        }
    }

    #[test]
    fn registry_preserves_first_index_and_later_override_source() {
        let first = br#"default { friction 0.8 } rock { base default }"#;
        let second = br#"rock { friction 0.9 } metal { base default }"#;
        let registry = SurfacePropertyRegistry::compile(&[
            SurfacePropertyFile {
                logical_path: "scripts/base.txt",
                bytes: first,
            },
            SurfacePropertyFile {
                logical_path: "scripts/game.txt",
                bytes: second,
            },
        ])
        .unwrap();
        assert_eq!(registry.records.len(), 4);
        assert_eq!(registry.resolve(Some(b"rock")).unwrap().index, 1);
        assert_eq!(registry.resolve(Some(b"ROCK")).unwrap().source_file, 1);
        let shadow = registry.resolve(Some(b"$material_index_shadow")).unwrap();
        assert_eq!(shadow.index, 2);
        assert_eq!(shadow.name, b"$MATERIAL_INDEX_SHADOW");
        assert_eq!(shadow.physics.friction.to_bits(), 0.8_f32.to_bits());
        assert_eq!(shadow.physics.elasticity.to_bits(), 0.001_f32.to_bits());
        assert_eq!(
            registry
                .resolve(Some(b"ROCK"))
                .unwrap()
                .physics
                .friction
                .to_bits(),
            0.9_f32.to_bits()
        );
        assert_eq!(
            registry
                .resolve(Some(b"metal"))
                .unwrap()
                .physics
                .friction
                .to_bits(),
            0.8_f32.to_bits()
        );
        assert_eq!(registry.resolve(Some(b"missing")).unwrap().name, b"default");
        assert_eq!(registry.resolve(None), None);
    }

    #[test]
    fn game_material_and_bullet_impact_follow_authored_base_inheritance() {
        let registry = SurfacePropertyRegistry::compile(&[SurfacePropertyFile {
            logical_path: "scripts/surfaceproperties.txt",
            bytes: br#"default { gamematerial C bulletimpact Default.BulletImpact }
                wood { base default gamematerial W bulletimpact Wood.BulletImpact }
                wood_plank { base wood }
                metal { base default gamematerial 77 }"#,
        }])
        .unwrap();
        let wood = registry.resolve(Some(b"wood_plank")).unwrap();
        assert_eq!(wood.game_material, b'W');
        assert_eq!(
            wood.bullet_impact.as_deref(),
            Some(b"wood.bulletimpact".as_slice())
        );
        let metal = registry.resolve(Some(b"metal")).unwrap();
        assert_eq!(metal.game_material, b'M');
        assert_eq!(
            metal.bullet_impact.as_deref(),
            Some(b"default.bulletimpact".as_slice())
        );
        let shadow = registry.resolve(Some(b"$MATERIAL_INDEX_SHADOW")).unwrap();
        assert_eq!(shadow.game_material, b'C');
        assert_eq!(
            shadow.bullet_impact.as_deref(),
            Some(b"default.bulletimpact".as_slice())
        );
    }

    #[test]
    fn physical_surface_inheritance_retains_source_order_and_all_binary32_fields() {
        let first = br#"
            default { friction .8 elasticity .25 density 2000 thickness .125 dampening 1.5 }
            metal { base default density 2700 elasticity .1 }
            sheet { friction .2 base metal thickness .04 }
            unknown_base { base does_not_exist dampening 200 }
        "#;
        let second = br#"metal { FRICTION .9 } extreme { elasticity 1000 }"#;
        let registry = SurfacePropertyRegistry::compile(&[
            SurfacePropertyFile {
                logical_path: "scripts/surfaceproperties.txt",
                bytes: first,
            },
            SurfacePropertyFile {
                logical_path: "scripts/surfaceproperties_tf.txt",
                bytes: second,
            },
        ])
        .unwrap();
        let metal = registry.resolve(Some(b"metal")).unwrap();
        assert_eq!(metal.index, 1);
        assert_eq!(metal.physics.friction.to_bits(), 0.9_f32.to_bits());
        assert_eq!(metal.physics.elasticity.to_bits(), 0.1_f32.to_bits());
        assert_eq!(metal.physics.density.to_bits(), 2700.0_f32.to_bits());
        assert_eq!(metal.physics.thickness.to_bits(), 0.125_f32.to_bits());
        assert_eq!(metal.physics.dampening.to_bits(), 1.5_f32.to_bits());
        let sheet = registry.resolve(Some(b"sheet")).unwrap();
        assert_eq!(sheet.physics.friction.to_bits(), 0.8_f32.to_bits());
        assert_eq!(sheet.physics.thickness.to_bits(), 0.04_f32.to_bits());
        let unknown = registry.resolve(Some(b"unknown_base")).unwrap();
        assert_eq!(unknown.physics.friction.to_bits(), 0.8_f32.to_bits());
        assert_eq!(unknown.physics.dampening.to_bits(), 200.0_f32.to_bits());
        assert_eq!(
            registry
                .resolve(Some(b"extreme"))
                .unwrap()
                .physics
                .elasticity
                .to_bits(),
            1000.0_f32.to_bits()
        );
    }

    #[test]
    fn malformed_physical_surface_children_fail_without_partial_publication() {
        for invalid in [
            br#"default { friction { value 1 } }"#.as_slice(),
            br#"default { base { value default } }"#.as_slice(),
        ] {
            assert_eq!(
                SurfacePropertyRegistry::compile(&[SurfacePropertyFile {
                    logical_path: "scripts/surfaceproperties.txt",
                    bytes: invalid,
                }]),
                Err(SurfacePropertyError::InvalidDocument)
            );
        }
    }
}
