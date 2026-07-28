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
        for (source_file, file) in files.iter().enumerate() {
            digest.update((file.logical_path.len() as u64).to_le_bytes());
            digest.update(file.logical_path.as_bytes());
            digest.update((file.bytes.len() as u64).to_le_bytes());
            digest.update(file.bytes);
            let document = parse_text(file.bytes, EscapeMode::Escaped, Limits::default())
                .map_err(|_| SurfacePropertyError::InvalidDocument)?;
            for (source_record, node) in document.roots.iter().enumerate() {
                if node.condition.is_some() || !matches!(node.value, Value::Object(_)) {
                    return Err(SurfacePropertyError::InvalidDocument);
                }
                let normalized = node.key.bytes.to_ascii_lowercase();
                if normalized.is_empty() {
                    return Err(SurfacePropertyError::InvalidInput);
                }
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
                    });
                }
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
}
