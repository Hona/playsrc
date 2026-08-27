//! Local one-of-each inventory. Only registered, implemented items are owned.

use crate::{Weapon, class::PlayerClass, schema::{CLASS_SLOT_COUNT, LoadoutPosition}};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemAttribute {
    pub definition: u32,
    pub value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EquippedItem {
    pub item_id: u32,
    pub definition_index: u32,
    pub quality: u8,
    pub style: u8,
    pub slot: LoadoutPosition,
    pub attributes: Vec<ItemAttribute>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Implementation {
    Weapon(Weapon),
    Wearable,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SupportedItem {
    pub definition_index: u32,
    pub implementation: Implementation,
    pub quality: u8,
    pub style: u8,
    pub attributes: &'static [ItemAttribute],
}

impl SupportedItem {
    /// Stable local identity, including definition zero. This is not a Steam ID.
    pub const fn item_id(self) -> u32 {
        self.definition_index + 1
    }
}

const fn weapon(definition_index: u32, weapon: Weapon) -> SupportedItem {
    SupportedItem { definition_index, implementation: Implementation::Weapon(weapon), quality: 0, style: 0, attributes: &[] }
}

pub const SUPPORTED_ITEMS: &[SupportedItem] = &[
    weapon(0, Weapon::Bat), weapon(1, Weapon::Bottle), weapon(2, Weapon::FireAxe),
    weapon(3, Weapon::Kukri), weapon(4, Weapon::Knife), weapon(5, Weapon::Fists),
    weapon(6, Weapon::Shovel), weapon(7, Weapon::Wrench), weapon(8, Weapon::Bonesaw),
    weapon(9, Weapon::EngineerShotgun), weapon(10, Weapon::Shotgun),
    weapon(11, Weapon::HeavyShotgun), weapon(12, Weapon::Shotgun),
    weapon(13, Weapon::Scattergun), weapon(14, Weapon::SniperRifle),
    weapon(15, Weapon::Minigun), weapon(16, Weapon::Smg), weapon(17, Weapon::SyringeGun),
    weapon(18, Weapon::RocketLauncher), weapon(19, Weapon::GrenadeLauncher),
    weapon(20, Weapon::StickybombLauncher), weapon(21, Weapon::Flamethrower),
    weapon(22, Weapon::EngineerPistol), weapon(23, Weapon::Pistol),
    weapon(24, Weapon::Revolver), weapon(25, Weapon::DestroyPda),
    weapon(26, Weapon::Toolbox), weapon(27, Weapon::DisguiseKit),
    weapon(28, Weapon::BuildPda), weapon(29, Weapon::MediGun),
    weapon(30, Weapon::InvisibilityWatch), weapon(735, Weapon::Sapper),
];

#[derive(Clone, Copy, Debug)]
pub struct ItemPresentation {
    pub definition_index: u32,
    pub name: &'static str,
    pub image: &'static str,
    pub class_slots: &'static [(PlayerClass, LoadoutPosition)],
}

include!("equipment.generated.rs");

pub fn schema() -> &'static crate::schema::ItemSchema {
    static SCHEMA: std::sync::OnceLock<crate::schema::ItemSchema> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| crate::schema::ItemSchema::compose(configured_schema_input()).expect("generated supported item schema"))
}

#[derive(Clone, Debug)]
pub struct AttributeProviders {
    graph: crate::attribute::AttributeGraph,
    weapons: std::collections::BTreeMap<Weapon, u32>,
}

impl AttributeProviders {
    pub fn new(equipment: &Equipment, class: PlayerClass) -> Self {
        use crate::attribute::{AttributeEntity, AttributeGraph, ProviderKind};
        let mut graph = AttributeGraph::default();
        graph.insert(AttributeEntity::new(1, ProviderKind::Player)).unwrap();
        let mut weapons = std::collections::BTreeMap::new();
        for item in equipment.equipped_items(class) {
            let supported = supported_item(item.definition_index).unwrap();
            let identity = item.item_id + 1;
            let kind = match supported.implementation {
                Implementation::Weapon(weapon) => { weapons.insert(weapon, identity); ProviderKind::Weapon },
                Implementation::Wearable => ProviderKind::Generic,
            };
            let mut entity = AttributeEntity::new(identity, kind);
            entity.attributes = schema().definition(item.definition_index).unwrap().static_attributes.clone();
            for attribute in item.attributes {
                let attribute = crate::schema::ItemAttribute { definition: attribute.definition,
                    value: crate::schema::ItemAttributeValue::Numeric(attribute.value) };
                if let Some(current) = entity.attributes.iter_mut().find(|current| current.definition == attribute.definition) {
                    *current = attribute;
                } else { entity.attributes.push(attribute); }
            }
            graph.insert(entity).unwrap();
            graph.set_owner(identity, Some(1)).unwrap();
            graph.provide_to(identity, 1).unwrap();
        }
        Self { graph, weapons }
    }

    pub fn weapon(&mut self, weapon: Weapon, hook: &str, input: f32) -> f32 {
        let Some(identity) = self.weapons.get(&weapon).copied() else { return input; };
        self.numeric(identity, hook, input)
    }

    pub fn player(&mut self, hook: &str, input: f32) -> f32 { self.numeric(1, hook, input) }

    fn numeric(&mut self, identity: u32, hook: &str, input: f32) -> f32 {
        let result = self.graph.query_numeric(schema(), identity, identity, hook, input, false).expect("validated supported attribute graph");
        let crate::attribute::QueryValue::Numeric(value) = result.value else { unreachable!() };
        value
    }
}

pub fn supported_item(definition_index: u32) -> Option<&'static SupportedItem> {
    SUPPORTED_ITEMS.iter().find(|item| item.definition_index == definition_index)
}

pub fn presentation(definition_index: u32) -> Option<&'static ItemPresentation> {
    ITEM_PRESENTATIONS.iter().find(|item| item.definition_index == definition_index)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquipmentError { UnsupportedItem, IneligibleSlot, InvalidPersistence }

#[derive(Clone, Debug, PartialEq)]
pub struct Equipment {
    revision: u32,
    classes: [[Option<u32>; CLASS_SLOT_COUNT]; 9],
}

impl Default for Equipment {
    fn default() -> Self {
        let mut classes = [[None; CLASS_SLOT_COUNT]; 9];
        for class in PlayerClass::ALL {
            for item in class.data().stock_items {
                if supported_item(item.definition).is_some() {
                    classes[class as usize - 1][item.slot as usize] = Some(item.definition);
                }
            }
        }
        Self { revision: 0, classes }
    }
}

impl Equipment {
    pub fn revision(&self) -> u32 { self.revision }

    pub fn encode_state(&self) -> Vec<u8> {
        let mut out = b"TFEI\x01\0\0\0".to_vec();
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.extend_from_slice(&(SUPPORTED_ITEMS.len() as u32).to_le_bytes());
        for supported in SUPPORTED_ITEMS {
            let metadata = presentation(supported.definition_index).expect("supported metadata");
            let item = EquippedItem { item_id: supported.item_id(), definition_index: supported.definition_index,
                quality: supported.quality, style: supported.style, slot: metadata.class_slots[0].1, attributes: supported.attributes.to_vec() };
            encode_items(&mut out, &[item]);
            out.push(match supported.implementation { Implementation::Weapon(weapon) => weapon as u8, Implementation::Wearable => 0 });
            out.push(metadata.class_slots.len() as u8);
            for (class, slot) in metadata.class_slots { out.extend_from_slice(&[*class as u8, *slot as u8]); }
            for text in [metadata.name, metadata.image] {
                out.extend_from_slice(&(text.len() as u32).to_le_bytes());
                out.extend_from_slice(text.as_bytes());
            }
        }
        for class in PlayerClass::ALL { encode_items(&mut out, &self.equipped_items(class)); }
        let persistence = self.persist();
        out.extend_from_slice(&(persistence.len() as u32).to_le_bytes());
        out.extend_from_slice(&persistence);
        out
    }

    pub fn definition(&self, class: PlayerClass, slot: LoadoutPosition) -> Option<u32> {
        self.classes[class as usize - 1][slot as usize]
    }

    pub fn equip(&mut self, class: PlayerClass, slot: LoadoutPosition, definition: Option<u32>) -> Result<bool, EquipmentError> {
        let definition = definition.or_else(|| class.data().stock_items.iter()
            .find(|item| item.slot == slot as u8).map(|item| item.definition));
        if let Some(definition) = definition {
            supported_item(definition).ok_or(EquipmentError::UnsupportedItem)?;
            if !presentation(definition).is_some_and(|item| item.class_slots.contains(&(class, slot))) {
                return Err(EquipmentError::IneligibleSlot);
            }
        }
        let current = &mut self.classes[class as usize - 1][slot as usize];
        if *current == definition { return Ok(false); }
        *current = definition;
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }

    pub fn equipped_items(&self, class: PlayerClass) -> Vec<EquippedItem> {
        LoadoutPosition::ALL.into_iter().filter_map(|slot| {
            let item = supported_item(self.definition(class, slot)?)?;
            Some(EquippedItem { item_id: item.item_id(), definition_index: item.definition_index,
                quality: item.quality, style: item.style, slot, attributes: item.attributes.to_vec() })
        }).collect()
    }

    pub fn weapons(&self, class: PlayerClass) -> impl Iterator<Item = Weapon> + '_ {
        self.classes[class as usize - 1].iter().filter_map(|definition| {
            match supported_item((*definition)?)?.implementation {
                Implementation::Weapon(weapon) => Some(weapon),
                Implementation::Wearable => None,
            }
        })
    }

    /// Versioned, bounded local storage; definition IDs survive runtime enum changes.
    pub fn persist(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + 9 * CLASS_SLOT_COUNT * 4);
        bytes.extend_from_slice(b"TFEQ\x01\0\0\0");
        for class in &self.classes {
            for definition in class { bytes.extend_from_slice(&definition.unwrap_or(u32::MAX).to_le_bytes()); }
        }
        bytes
    }

    pub fn restore(bytes: &[u8]) -> Result<Self, EquipmentError> {
        if bytes.len() != 8 + 9 * CLASS_SLOT_COUNT * 4 || &bytes[..8] != b"TFEQ\x01\0\0\0" {
            return Err(EquipmentError::InvalidPersistence);
        }
        let mut equipment = Self::default();
        for (index, chunk) in bytes[8..].chunks_exact(4).enumerate() {
            let definition = u32::from_le_bytes(chunk.try_into().unwrap());
            let class = PlayerClass::ALL[index / CLASS_SLOT_COUNT];
            let slot = LoadoutPosition::ALL[index % CLASS_SLOT_COUNT];
            equipment.equip(class, slot, (definition != u32::MAX).then_some(definition))?;
        }
        equipment.revision = 0;
        Ok(equipment)
    }
}

pub fn encode_items(out: &mut Vec<u8>, items: &[EquippedItem]) {
    out.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for item in items {
        out.extend_from_slice(&item.item_id.to_le_bytes());
        out.extend_from_slice(&item.definition_index.to_le_bytes());
        out.extend_from_slice(&[item.quality, item.style, item.slot as u8, item.attributes.len() as u8]);
        for attribute in &item.attributes {
            out.extend_from_slice(&attribute.definition.to_le_bytes());
            out.extend_from_slice(&attribute.value.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implemented_catalog_has_one_stable_instance_per_definition() {
        let mut identities = std::collections::BTreeSet::new();
        for item in SUPPORTED_ITEMS {
            assert!(identities.insert(item.item_id()));
            let definition = schema().definition(item.definition_index).unwrap();
            let metadata = presentation(item.definition_index).unwrap();
            assert_eq!(metadata.class_slots.len(), definition.class_slots.len());
            for (class, slot) in metadata.class_slots {
                assert_eq!(definition.slot_for_class(*class), Some(*slot));
            }
        }
    }

    #[test]
    fn all_nine_stock_loadouts_restore_without_inventory_duplication() {
        let equipment = Equipment::default();
        assert_eq!(Equipment::restore(&equipment.persist()), Ok(equipment.clone()));
        for class in PlayerClass::ALL {
            let items = equipment.equipped_items(class);
            assert_eq!(items.len(), class.data().stock_items.len());
            for stock in class.data().stock_items {
                assert!(items.iter().any(|item| item.definition_index == stock.definition && item.slot as u8 == stock.slot));
            }
            let mut providers = AttributeProviders::new(&equipment, class);
            assert_eq!(providers.player("mult_maxammo_primary", 1.0), 1.0);
        }
    }

    #[test]
    fn invalid_equips_and_corrupt_storage_cannot_mutate_inventory() {
        let mut equipment = Equipment::default();
        let before = equipment.clone();
        assert_eq!(equipment.equip(PlayerClass::Scout, LoadoutPosition::Primary, Some(18)), Err(EquipmentError::IneligibleSlot));
        assert_eq!(equipment.equip(PlayerClass::Soldier, LoadoutPosition::Primary, Some(999_999)), Err(EquipmentError::UnsupportedItem));
        assert_eq!(equipment, before);
        assert_eq!(equipment.equip(PlayerClass::Soldier, LoadoutPosition::Primary, None), Ok(false));
        let mut corrupt = before.persist();
        corrupt[8..12].copy_from_slice(&999_999u32.to_le_bytes());
        assert_eq!(Equipment::restore(&corrupt), Err(EquipmentError::UnsupportedItem));
        assert_eq!(Equipment::restore(&corrupt[..corrupt.len()-1]), Err(EquipmentError::InvalidPersistence));
    }
}
