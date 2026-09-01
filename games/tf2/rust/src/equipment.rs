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

    pub fn weapon_for_class(self, class: PlayerClass) -> Option<Weapon> {
        let Implementation::Weapon(weapon) = self.implementation else { return None; };
        Some(match (weapon, class) {
            (Weapon::Shotgun, PlayerClass::Engineer) => Weapon::EngineerShotgun,
            (Weapon::Shotgun, PlayerClass::Heavy) => Weapon::HeavyShotgun,
            (Weapon::Bottle, PlayerClass::Soldier) => Weapon::Shovel,
            _ => weapon,
        })
    }

    pub fn selection_slot(self, class: PlayerClass, slot: LoadoutPosition) -> Option<u8> {
        match self.weapon_for_class(class)? {
            Weapon::InvisibilityWatch | Weapon::Toolbox => None,
            Weapon::Revolver => Some(0),
            Weapon::Sapper => Some(1),
            Weapon::DisguiseKit | Weapon::BuildPda => Some(3),
            Weapon::DestroyPda => Some(4),
            _ if matches!(slot, LoadoutPosition::Primary | LoadoutPosition::Secondary | LoadoutPosition::Melee) => Some(slot as u8),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DescriptionLine {
    pub text: &'static str,
    pub color: &'static str,
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum AmmoDisplay { Hidden = 0, Total = 1, ClipAndReserve = 2 }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CountMeter { None = 0, Kills = 1, RevengeActive = 2, Heads = 3, Revenge = 4 }

#[derive(Clone, Copy, Debug)]
pub struct WeaponHud {
    pub allows_auto_switch_to: bool,
    pub count_meter: CountMeter,
    pub script: &'static str,
    pub ammo: AmmoDisplay,
    pub bucket: u8,
    pub position: u8,
    pub draws_crosshair: bool,
    pub suppress_crosshair: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemPresentation {
    pub class_hud: &'static [(PlayerClass, WeaponHud)],
    pub definition_index: u32,
    pub name: &'static str,
    pub display_name: &'static str,
    pub description: &'static [DescriptionLine],
    pub animation_slot: Option<&'static str>,
    pub extra_sounds: &'static [&'static str],
    pub image: &'static str,
    pub model_player: &'static str,
    pub class_models: &'static [(PlayerClass, &'static str)],
    pub attach_to_hands: bool,
    pub animation_replacements: &'static [(&'static str, &'static str)],
    pub sound_overrides: &'static [(&'static str, &'static str)],
    pub death_notice_icon: Option<&'static str>,
    pub class_slots: &'static [(PlayerClass, LoadoutPosition)],
}

impl ItemPresentation {
    pub fn model_for_class(&self, class: PlayerClass) -> Option<&'static str> {
        self.class_models.iter().find(|(eligible, _)| *eligible == class).map(|(_, model)| *model).filter(|model| !model.is_empty())
    }
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
    active_only: Vec<(Weapon, u32)>,
    active: Option<Weapon>,
}

impl AttributeProviders {
    pub fn new(items: &[EquippedItem], class: PlayerClass) -> Self {
        use crate::attribute::{AttributeEntity, AttributeGraph, ProviderKind};
        let mut graph = AttributeGraph::default();
        graph.insert(AttributeEntity::new(1, ProviderKind::Player)).unwrap();
        let mut weapons = std::collections::BTreeMap::new();
        for item in items {
            let supported = registered_item(item.definition_index).expect("validated equipped item definition");
            let identity = item.item_id + 1;
            let kind = match supported.weapon_for_class(class) {
                Some(weapon) => { weapons.insert(weapon, identity); ProviderKind::Weapon },
                None => ProviderKind::Generic,
            };
            let mut entity = AttributeEntity::new(identity, kind);
            entity.attributes = schema().definition(item.definition_index).unwrap().static_attributes.clone();
            for attribute in &item.attributes {
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
        let mut providers = Self { graph, weapons, active_only: Vec::new(), active: None };
        for (weapon, identity) in providers.weapons.clone() {
            if providers.numeric(identity, "provide_on_active", 0.0).round_ties_even() == 1.0 {
                providers.active_only.push((weapon, identity));
                providers.graph.stop_providing_to(identity, 1).unwrap();
            }
        }
        providers
    }

    /// Reapply only active-state-dependent provision. The weapon keeps its own
    /// attributes while holstered; its owner and sibling weapons lose them.
    pub fn set_active(&mut self, active: Option<Weapon>) {
        if self.active == active { return; }
        for &(weapon, identity) in &self.active_only {
            if self.active == Some(weapon) {
                self.graph.stop_providing_to(identity, 1).unwrap();
            }
            if active == Some(weapon) {
                self.graph.provide_to(identity, 1).unwrap();
            }
        }
        self.active = active;
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

pub(crate) fn registered_item(definition_index: u32) -> Option<&'static SupportedItem> {
    REGISTERED_ITEMS.iter().find(|item| item.definition_index == definition_index)
}

pub fn presentation(definition_index: u32) -> Option<&'static ItemPresentation> {
    ITEM_PRESENTATIONS.iter().find(|item| item.definition_index == definition_index)
}

pub fn stock_weapon_models() -> impl Iterator<Item = &'static str> {
    PlayerClass::ALL.into_iter().flat_map(|class| class.data().stock_items.iter().filter_map(move |stock| {
        let item = supported_item(stock.definition)?;
        if item.weapon_for_class(class).is_none() { return None; }
        let model = presentation(stock.definition)?.model_for_class(class)?;
        (!model.is_empty()).then_some(model)
    }))
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
    pub fn class_matches(&self, other: &Self, class: PlayerClass) -> bool {
        self.classes[class as usize - 1] == other.classes[class as usize - 1]
    }

    pub fn encode_state(&self) -> Vec<u8> {
        let mut out = b"TFEI\x06\0\0\0".to_vec();
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.extend_from_slice(&(SUPPORTED_ITEMS.len() as u32).to_le_bytes());
        for supported in SUPPORTED_ITEMS {
            let metadata = presentation(supported.definition_index).expect("supported metadata");
            let item = EquippedItem { item_id: supported.item_id(), definition_index: supported.definition_index,
                quality: supported.quality, style: supported.style, slot: metadata.class_slots[0].1, attributes: supported.attributes.to_vec() };
            encode_items(&mut out, &[item]);
            out.push(match supported.implementation { Implementation::Weapon(weapon) => weapon as u8, Implementation::Wearable => 0 });
            let eligible: Vec<_> = metadata.class_slots.iter().flat_map(|(class, slot)| {
                if misc_slot(*slot) { vec![(*class, LoadoutPosition::Head), (*class, LoadoutPosition::Misc), (*class, LoadoutPosition::Misc2)] }
                else { vec![(*class, *slot)] }
            }).collect();
            out.push(eligible.len() as u8);
            for (class, slot) in eligible {
                out.extend_from_slice(&[class as u8, slot as u8, supported.weapon_for_class(class).map_or(0, |weapon| weapon as u8), supported.selection_slot(class, slot).unwrap_or(u8::MAX)]);
                let hud = metadata.class_hud.iter().find(|(eligible, _)| *eligible == class).map(|(_, hud)| hud);
                out.extend_from_slice(&hud.map_or([0; 4], |hud| [hud.ammo as u8, hud.bucket, hud.position,
                    u8::from(hud.draws_crosshair) | (u8::from(hud.suppress_crosshair) << 1) | ((hud.count_meter as u8) << 2)]));
                let script = hud.map_or("", |hud| hud.script);
                out.extend_from_slice(&(script.len() as u32).to_le_bytes()); out.extend_from_slice(script.as_bytes());
            }
            for text in [metadata.name, metadata.display_name, metadata.image] {
                out.extend_from_slice(&(text.len() as u32).to_le_bytes());
                out.extend_from_slice(text.as_bytes());
            }
            out.extend_from_slice(&(metadata.description.len() as u32).to_le_bytes());
            for line in metadata.description {
                for text in [line.text, line.color] {
                    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
                    out.extend_from_slice(text.as_bytes());
                }
            }
            let animation_slot = metadata.animation_slot.unwrap_or("");
            out.extend_from_slice(&(animation_slot.len() as u32).to_le_bytes()); out.extend_from_slice(animation_slot.as_bytes());
            out.extend_from_slice(&(metadata.extra_sounds.len() as u32).to_le_bytes());
            for sound in metadata.extra_sounds { out.extend_from_slice(&(sound.len() as u32).to_le_bytes()); out.extend_from_slice(sound.as_bytes()); }
            out.push(u8::from(metadata.attach_to_hands));
            let death_icon = metadata.death_notice_icon.unwrap_or("");
            out.extend_from_slice(&(death_icon.len() as u32).to_le_bytes());
            out.extend_from_slice(death_icon.as_bytes());
            out.extend_from_slice(&(metadata.model_player.len() as u32).to_le_bytes());
            out.extend_from_slice(metadata.model_player.as_bytes());
            for pairs in [metadata.animation_replacements, metadata.sound_overrides] {
                out.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
                for (key, value) in pairs {
                    for text in [key, value] {
                        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
                        out.extend_from_slice(text.as_bytes());
                    }
                }
            }
        }
        let base = Self::default();
        for class in PlayerClass::ALL {
            encode_items(&mut out, &self.equipped_items(class));
            encode_items(&mut out, &base.equipped_items(class));
        }
        let persistence = self.persist();
        out.extend_from_slice(&(persistence.len() as u32).to_le_bytes());
        out.extend_from_slice(&persistence);
        out
    }

    pub fn definition(&self, class: PlayerClass, slot: LoadoutPosition) -> Option<u32> {
        self.classes[class as usize - 1][slot as usize]
    }

    pub fn weapon_definition(&self, class: PlayerClass, weapon: Weapon) -> Option<u32> {
        self.classes[class as usize - 1].iter().copied().flatten().find(|definition| {
            supported_item(*definition).is_some_and(|item| item.weapon_for_class(class) == Some(weapon))
        })
    }

    pub fn equip(&mut self, class: PlayerClass, slot: LoadoutPosition, definition: Option<u32>) -> Result<bool, EquipmentError> {
        let definition = definition.or_else(|| class.data().stock_items.iter()
            .find(|item| item.slot == slot as u8 && supported_item(item.definition).is_some()).map(|item| item.definition));
        if let Some(definition) = definition {
            supported_item(definition).ok_or(EquipmentError::UnsupportedItem)?;
            if !presentation(definition).is_some_and(|item| item.class_slots.iter().any(|(eligible, position)|
                *eligible == class && (*position == slot || (misc_slot(*position) && misc_slot(slot))))) {
                return Err(EquipmentError::IneligibleSlot);
            }
        }
        if let Some(definition) = definition && misc_slot(slot) {
            for other in [LoadoutPosition::Head, LoadoutPosition::Misc, LoadoutPosition::Misc2] {
                if other != slot && self.classes[class as usize - 1][other as usize] == Some(definition) {
                    self.classes[class as usize - 1][other as usize] = None;
                }
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
        self.classes[class as usize - 1].iter().filter_map(move |definition| {
            supported_item((*definition)?)?.weapon_for_class(class)
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
        let mut normalized = bytes.to_vec();
        for (index, chunk) in bytes[8..].chunks_exact(4).enumerate() {
            let mut definition = u32::from_le_bytes(chunk.try_into().unwrap());
            let class = PlayerClass::ALL[index / CLASS_SLOT_COUNT];
            let slot = LoadoutPosition::ALL[index % CLASS_SLOT_COUNT];
            if definition != u32::MAX && supported_item(definition).is_none() && registered_item(definition).is_some() {
                // A no-longer-owned implementation cannot be resurrected by local storage.
                // Keep the class's available base item, as for an absent inventory item.
                if !presentation(definition).is_some_and(|item| item.class_slots.iter().any(|(eligible, position)|
                    *eligible == class && (*position == slot || misc_slot(*position) && misc_slot(slot)))) { return Err(EquipmentError::IneligibleSlot); }
                definition = equipment.definition(class, slot).unwrap_or(u32::MAX);
                normalized[8 + index * 4..12 + index * 4].copy_from_slice(&definition.to_le_bytes());
            }
            equipment.equip(class, slot, (definition != u32::MAX).then_some(definition))?;
        }
        equipment.revision = 0;
        if equipment.persist() != normalized { return Err(EquipmentError::InvalidPersistence); }
        Ok(equipment)
    }
}

fn misc_slot(slot: LoadoutPosition) -> bool { matches!(slot, LoadoutPosition::Head | LoadoutPosition::Misc | LoadoutPosition::Misc2) }

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
    fn active_provision_transitions_are_idempotent_and_leave_weapon_ownership_intact() {
        let mut providers = AttributeProviders::new(&Equipment::default().equipped_items(PlayerClass::Pyro), PlayerClass::Pyro);
        let identity = providers.weapons[&Weapon::FireAxe];
        providers.graph.stop_providing_to(identity, 1).unwrap();
        providers.active_only.push((Weapon::FireAxe, identity));
        for active in [Some(Weapon::FireAxe), Some(Weapon::FireAxe), Some(Weapon::Shotgun), None, Some(Weapon::FireAxe), None] {
            providers.set_active(active);
            assert_eq!(providers.graph.entity(1).unwrap().providers.contains(&identity), active == Some(Weapon::FireAxe));
            assert_eq!(providers.graph.entity(identity).unwrap().owner, Some(1));
        }
    }

    #[test]
    fn engineer_pda_definitions_bind_the_authored_class_models_and_slots() {
        let equipment = Equipment::default();
        for (definition, weapon, slot, model) in [
            (25, Weapon::BuildPda, LoadoutPosition::Pda, "models/weapons/c_models/c_builder/c_builder.mdl"),
            (26, Weapon::DestroyPda, LoadoutPosition::Pda2, "models/weapons/c_models/c_pda_engineer/c_pda_engineer.mdl"),
            (28, Weapon::Toolbox, LoadoutPosition::Building, "models/weapons/c_models/c_toolbox/c_toolbox.mdl"),
        ] {
            assert_eq!(equipment.weapon_definition(PlayerClass::Engineer, weapon), Some(definition));
            assert_eq!(equipment.definition(PlayerClass::Engineer, slot), Some(definition));
            assert_eq!(presentation(definition).unwrap().model_player, model);
        }
    }

    #[test]
    fn configured_unusual_description_keeps_quality_effect_and_authored_flavor() {
        let item = presentation(378).unwrap();
        assert_eq!(item.display_name, "Unusual Team Captain");
        assert_eq!(item.description.iter().map(|line| (line.text, line.color)).collect::<Vec<_>>(), [
            ("Level 1 Hat", "ItemAttribLevel"),
            ("★ Unusual Effect: Burning Flames", "QualityColorrarity4"),
            ("Our lawyers say 'YES! YES!'", "ItemAttribNeutral"),
        ]);
        assert_eq!(SUPPORTED_ITEMS.iter().filter(|item| item.definition_index == 378).count(), 1);
    }

    #[test]
    fn generic_weapon_classes_translate_before_runtime_and_provider_keying() {
        let shotgun = *supported_item(10).unwrap();
        assert_eq!(shotgun.weapon_for_class(PlayerClass::Soldier), Some(Weapon::Shotgun));
        assert_eq!(shotgun.weapon_for_class(PlayerClass::Pyro), Some(Weapon::Shotgun));
        assert_eq!(shotgun.weapon_for_class(PlayerClass::Heavy), Some(Weapon::HeavyShotgun));
        assert_eq!(shotgun.weapon_for_class(PlayerClass::Engineer), Some(Weapon::EngineerShotgun));
        assert_eq!(supported_item(1).unwrap().weapon_for_class(PlayerClass::Soldier), Some(Weapon::Shovel));
    }

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
            assert_eq!(items.len(), class.data().stock_items.iter().filter(|stock| supported_item(stock.definition).is_some()).count());
            for stock in class.data().stock_items.iter().filter(|stock| supported_item(stock.definition).is_some()) {
                assert!(items.iter().any(|item| item.definition_index == stock.definition && item.slot as u8 == stock.slot));
            }
            let mut providers = AttributeProviders::new(&equipment.equipped_items(class), class);
            assert_eq!(providers.player("mult_maxammo_primary", 1.0), 1.0);
        }
    }

    #[test]
    fn storage_cannot_restore_known_unimplemented_items() {
        let base = Equipment::default();
        let mut old = base.persist();
        let slot = 8 + ((PlayerClass::Spy as usize - 1) * CLASS_SLOT_COUNT + LoadoutPosition::Building as usize) * 4;
        old[slot..slot + 4].copy_from_slice(&735_u32.to_le_bytes());
        assert_eq!(Equipment::restore(&old), Ok(base));
        assert_eq!(Equipment::default().equip(PlayerClass::Spy, LoadoutPosition::Building, Some(735)), Err(EquipmentError::UnsupportedItem));
    }

    #[test]
    fn stock_sticky_launcher_is_admitted_and_persisted() {
        let mut equipment = Equipment::default();
        equipment.equip(PlayerClass::Demoman, LoadoutPosition::Primary, Some(19)).unwrap();
        equipment.equip(PlayerClass::Demoman, LoadoutPosition::Secondary, Some(20)).unwrap();
        assert!(equipment.equipped_items(PlayerClass::Demoman).iter().any(|item| item.definition_index == 19));
        assert!(equipment.equipped_items(PlayerClass::Demoman).iter().any(|item| item.definition_index == 20));
        assert_eq!(Equipment::restore(&equipment.persist()), Ok(equipment));
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
    #[test]
    fn configured_weapon_hud_uses_class_translated_scripts_not_runtime_family_guesses() {
        let hud = |definition, class| presentation(definition).unwrap().class_hud.iter().find(|(eligible, _)| *eligible == class).unwrap().1;
        assert_eq!(hud(10, PlayerClass::Soldier).script, "scripts/tf_weapon_shotgun_soldier.ctx");
        assert_eq!(hud(12, PlayerClass::Pyro).script, "scripts/tf_weapon_shotgun_pyro.ctx");
        assert_eq!(hud(23, PlayerClass::Scout).script, "scripts/tf_weapon_pistol_scout.ctx");
        assert_eq!(hud(22, PlayerClass::Engineer).script, "scripts/tf_weapon_pistol.ctx");
        assert_eq!(hud(220, PlayerClass::Scout).script, "scripts/tf_weapon_handgun_scout_primary.ctx");
        assert!(matches!(hud(220, PlayerClass::Scout).ammo, AmmoDisplay::ClipAndReserve));
        assert!(matches!(hud(15, PlayerClass::Heavy).ammo, AmmoDisplay::Total));
        assert!(matches!(hud(25, PlayerClass::Engineer).ammo, AmmoDisplay::Hidden));
        assert!(hud(25, PlayerClass::Engineer).suppress_crosshair);
        assert_eq!(hud(14, PlayerClass::Sniper).count_meter, CountMeter::None);
        assert_eq!(hud(402, PlayerClass::Sniper).count_meter, CountMeter::Heads);
    }
}
