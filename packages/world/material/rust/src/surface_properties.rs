use playsrc_keyvalues::{EscapeMode, Limits, Value, parse_text};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct SurfacePropertyFile<'a> {
    pub logical_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfacePropertyRecord {
    pub index: u32,
    pub name: Vec<u8>,
    pub source_file: usize,
    pub source_record: usize,
    pub game_material: u8,
    pub bullet_impact: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    pub fn compile(files: &[SurfacePropertyFile<'_>]) -> Result<Self, SurfacePropertyError> {
        if files.is_empty() || files.len() > 4_096 {
            return Err(SurfacePropertyError::Limit);
        }
        let mut digest = Sha256::new();
        digest.update(b"playsrc-surface-property-registry-v1\0");
        let mut records = Vec::<SurfacePropertyRecord>::new();
        let mut lookup = BTreeMap::<Vec<u8>, u32>::new();
        let mut definitions =
            BTreeMap::<Vec<u8>, (Option<Vec<u8>>, Option<u8>, Option<Vec<u8>>)>::new();
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
                let mut base = None;
                let mut game_material = None;
                let mut bullet_impact = None;
                for field in fields {
                    let Value::Scalar(value) = &field.value else {
                        continue;
                    };
                    if field.key.bytes.eq_ignore_ascii_case(b"base") {
                        base = Some(value.token.bytes.to_ascii_lowercase());
                    } else if field.key.bytes.eq_ignore_ascii_case(b"gamematerial") {
                        let raw = &value.token.bytes;
                        game_material = Some(if raw.len() == 1 && !raw[0].is_ascii_digit() {
                            raw[0].to_ascii_uppercase()
                        } else {
                            std::str::from_utf8(raw)
                                .ok()
                                .and_then(|value| value.parse::<u8>().ok())
                                .ok_or(SurfacePropertyError::InvalidDocument)?
                        });
                    } else if field.key.bytes.eq_ignore_ascii_case(b"bulletimpact") {
                        bullet_impact = Some(value.token.bytes.clone());
                    }
                }
                definitions.insert(normalized.clone(), (base, game_material, bullet_impact));
                if let Some(index) = lookup.get(&normalized).copied() {
                    let record = records
                        .get_mut(index as usize)
                        .ok_or(SurfacePropertyError::InvalidInput)?;
                    record.source_file = source_file;
                    record.source_record = source_record;
                } else {
                    let index =
                        u32::try_from(records.len()).map_err(|_| SurfacePropertyError::Limit)?;
                    lookup.insert(normalized.clone(), index);
                    records.push(SurfacePropertyRecord {
                        index,
                        name: normalized,
                        source_file,
                        source_record,
                        game_material: b'C',
                        bullet_impact: None,
                    });
                }
            }
        }
        if !lookup.contains_key(b"default".as_slice()) {
            return Err(SurfacePropertyError::MissingDefault);
        }
        for record in &mut records {
            let mut cursor = record.name.as_slice();
            let mut seen = std::collections::BTreeSet::new();
            let mut material = None;
            let mut impact = None;
            loop {
                if !seen.insert(cursor.to_vec()) {
                    return Err(SurfacePropertyError::InvalidDocument);
                }
                let Some((base, game_material, bullet_impact)) = definitions.get(cursor) else {
                    return Err(SurfacePropertyError::InvalidDocument);
                };
                material = material.or(*game_material);
                impact = impact.or_else(|| bullet_impact.clone());
                match base {
                    Some(parent) => cursor = parent,
                    None if cursor != b"default" => cursor = b"default",
                    None => break,
                }
            }
            record.game_material = material.unwrap_or(b'C');
            record.bullet_impact = impact;
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
        assert_eq!(registry.records.len(), 3);
        assert_eq!(registry.resolve(Some(b"rock")).unwrap().index, 1);
        assert_eq!(registry.resolve(Some(b"ROCK")).unwrap().source_file, 1);
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
            Some(b"Wood.BulletImpact".as_slice())
        );
        let metal = registry.resolve(Some(b"metal")).unwrap();
        assert_eq!(metal.game_material, b'M');
        assert_eq!(
            metal.bullet_impact.as_deref(),
            Some(b"Default.BulletImpact".as_slice())
        );
    }
}
