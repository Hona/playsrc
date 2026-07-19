use std::collections::{BTreeMap, BTreeSet};

use crate::class::{PlayerClass, PlayerTeam};

pub const ITEM_SCHEMA_SHA256: &str =
    "47900e0d174971625a76625fe311a012910031171d0b121ff5f628078c83214d";
pub const ITEM_SCHEMA_SIGNATURE_SHA256: &str =
    "2a9de0701878250a20329bf8bd2b974e54f19dd18ba709778736e4828f7daad6";
pub const CLASS_SLOT_COUNT: usize = 19;
pub const ACCOUNT_SLOT_COUNT: usize = 3;
pub const PRESET_COUNT: u8 = 4;
pub const MAX_SCHEMA_PREFABS: usize = 4_096;
pub const MAX_SCHEMA_ATTRIBUTES: usize = 4_096;
pub const MAX_SCHEMA_ITEMS: usize = 32_768;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum LoadoutPosition {
    Primary = 0,
    Secondary = 1,
    Melee = 2,
    Utility = 3,
    Building = 4,
    Pda = 5,
    Pda2 = 6,
    Head = 7,
    Misc = 8,
    Action = 9,
    Misc2 = 10,
    Taunt = 11,
    Taunt2 = 12,
    Taunt3 = 13,
    Taunt4 = 14,
    Taunt5 = 15,
    Taunt6 = 16,
    Taunt7 = 17,
    Taunt8 = 18,
}

impl LoadoutPosition {
    pub const ALL: [Self; CLASS_SLOT_COUNT] = [
        Self::Primary,
        Self::Secondary,
        Self::Melee,
        Self::Utility,
        Self::Building,
        Self::Pda,
        Self::Pda2,
        Self::Head,
        Self::Misc,
        Self::Action,
        Self::Misc2,
        Self::Taunt,
        Self::Taunt2,
        Self::Taunt3,
        Self::Taunt4,
        Self::Taunt5,
        Self::Taunt6,
        Self::Taunt7,
        Self::Taunt8,
    ];
}

impl TryFrom<u8> for LoadoutPosition {
    type Error = SchemaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|position| *position as u8 == value)
            .ok_or(SchemaError::InvalidClassSlot(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AccountLoadoutPosition {
    Account1 = 0,
    Account2 = 1,
    Account3 = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemQuality {
    Normal,
    Rarity1,
    Rarity2,
    Vintage,
    Rarity3,
    Rarity4,
    Unique,
    Community,
    Developer,
    SelfMade,
    Customized,
    Strange,
    Completed,
    Haunted,
    Collectors,
    PaintKitWeapon,
}

impl ItemQuality {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "normal" => Self::Normal,
            "rarity1" => Self::Rarity1,
            "rarity2" => Self::Rarity2,
            "vintage" => Self::Vintage,
            "rarity3" => Self::Rarity3,
            "rarity4" => Self::Rarity4,
            "unique" => Self::Unique,
            "community" => Self::Community,
            "developer" => Self::Developer,
            "selfmade" => Self::SelfMade,
            "customized" => Self::Customized,
            "strange" => Self::Strange,
            "completed" => Self::Completed,
            "haunted" => Self::Haunted,
            "collectors" => Self::Collectors,
            "paintkitweapon" => Self::PaintKitWeapon,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SchemaValue {
    Scalar(String),
    Object(Vec<SchemaNode>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchemaNode {
    pub key: String,
    pub value: SchemaValue,
}

impl SchemaNode {
    pub fn scalar(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: SchemaValue::Scalar(value.into()),
        }
    }

    pub fn object(key: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            key: key.into(),
            value: SchemaValue::Object(children),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeValueKind {
    Numeric,
    String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeDescriptionFormat {
    Percentage,
    InvertedPercentage,
    Additive,
    AdditivePercentage,
    ParticleIndex,
    KillstreakEffectIndex,
    KillstreakIdleEffectIndex,
    Lookup,
    Or,
    Date,
    AccountId,
    ItemDefinition,
}

impl AttributeDescriptionFormat {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "value_is_percentage" => Self::Percentage,
            "value_is_inverted_percentage" => Self::InvertedPercentage,
            "value_is_additive" => Self::Additive,
            "value_is_additive_percentage" => Self::AdditivePercentage,
            "value_is_particle_index" => Self::ParticleIndex,
            "value_is_killstreakeffect_index" => Self::KillstreakEffectIndex,
            "value_is_killstreak_idleeffect_index" => Self::KillstreakIdleEffectIndex,
            "value_is_from_lookup_table" => Self::Lookup,
            "value_is_or" => Self::Or,
            "value_is_date" => Self::Date,
            "value_is_account_id" => Self::AccountId,
            "value_is_item_def" => Self::ItemDefinition,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeDefinition {
    pub index: u32,
    pub name: String,
    pub class: String,
    pub value_kind: AttributeValueKind,
    pub description_format: AttributeDescriptionFormat,
    pub stored_as_integer: bool,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemAttributeValue {
    Numeric(f32),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemAttribute {
    pub definition: u32,
    pub value: ItemAttributeValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemDefinition {
    pub index: u32,
    pub name: String,
    pub item_class: String,
    pub slot: LoadoutPosition,
    pub quality: ItemQuality,
    pub minimum_level: u8,
    pub maximum_level: u8,
    pub base_item: bool,
    pub usable_by: BTreeSet<PlayerClass>,
    pub styles: BTreeSet<u8>,
    pub static_attributes: Vec<ItemAttribute>,
    pub source: Vec<SchemaNode>,
}

#[derive(Clone, Debug)]
pub struct SchemaInput {
    pub content_build: u32,
    pub schema_sha256: String,
    pub signature_sha256: String,
    pub game_info: Vec<SchemaNode>,
    pub prefabs: Vec<SchemaNode>,
    pub attributes: Vec<SchemaNode>,
    pub items: Vec<SchemaNode>,
}

#[derive(Clone, Debug)]
pub struct ItemSchema {
    content_build: u32,
    schema_sha256: String,
    prefabs_in_source_order: Vec<String>,
    definitions_in_source_order: Vec<u32>,
    attributes_in_source_order: Vec<u32>,
    default_item_source: Vec<SchemaNode>,
    definitions: BTreeMap<u32, ItemDefinition>,
    attributes: BTreeMap<u32, AttributeDefinition>,
}

impl ItemSchema {
    pub fn compose(input: SchemaInput) -> Result<Self, SchemaError> {
        validate_identity(&input)?;
        if input.prefabs.len() > MAX_SCHEMA_PREFABS
            || input.attributes.len() > MAX_SCHEMA_ATTRIBUTES
            || input.items.len() > MAX_SCHEMA_ITEMS
        {
            return Err(SchemaError::RecordLimit);
        }
        validate_game_info(&input.game_info)?;
        let IndexedObjects {
            source_order: prefabs_in_source_order,
            by_identity: prefabs,
        } = indexed_objects(&input.prefabs, false)?;
        let IndexedObjects {
            source_order: attribute_identities,
            by_identity: raw_attributes,
        } = indexed_objects(&input.attributes, true)?;
        let IndexedObjects {
            source_order: item_identities,
            by_identity: raw_items,
        } = indexed_objects(&input.items, true)?;

        let mut attributes = BTreeMap::new();
        let mut attributes_in_source_order = Vec::with_capacity(raw_attributes.len());
        for identity in attribute_identities {
            let fields = raw_attributes
                .get(&identity)
                .expect("source identity was indexed");
            let index = identity
                .parse::<u32>()
                .map_err(|_| SchemaError::InvalidAttributeIdentity(identity.clone()))?;
            if attributes.contains_key(&index) {
                return Err(SchemaError::DuplicateAttribute(index));
            }
            attributes_in_source_order.push(index);
            attributes.insert(index, parse_attribute(index, fields)?);
        }

        let default_item_source = raw_items
            .get("default")
            .map(|fields| resolve_item(fields, &prefabs))
            .transpose()?
            .ok_or_else(|| SchemaError::MissingField {
                record: "items".into(),
                field: "default",
            })?;
        let mut definitions = BTreeMap::new();
        let mut definitions_in_source_order = Vec::with_capacity(raw_items.len());
        for identity in item_identities {
            if identity == "default" {
                continue;
            }
            let fields = raw_items
                .get(&identity)
                .expect("source identity was indexed");
            let index = identity
                .parse::<u32>()
                .map_err(|_| SchemaError::InvalidItemIdentity(identity.clone()))?;
            if definitions.contains_key(&index) {
                return Err(SchemaError::DuplicateItem(index));
            }
            let merged = resolve_item(fields, &prefabs)?;
            definitions_in_source_order.push(index);
            definitions.insert(index, parse_item(index, merged, &attributes)?);
        }

        Ok(Self {
            content_build: input.content_build,
            schema_sha256: input.schema_sha256,
            prefabs_in_source_order,
            definitions_in_source_order,
            attributes_in_source_order,
            default_item_source,
            definitions,
            attributes,
        })
    }

    pub fn content_build(&self) -> u32 {
        self.content_build
    }

    pub fn schema_sha256(&self) -> &str {
        &self.schema_sha256
    }

    pub fn definition(&self, index: u32) -> Option<&ItemDefinition> {
        self.definitions.get(&index)
    }

    pub fn attribute(&self, index: u32) -> Option<&AttributeDefinition> {
        self.attributes.get(&index)
    }

    pub fn default_item_source(&self) -> &[SchemaNode] {
        &self.default_item_source
    }

    pub fn definitions_in_source_order(&self) -> &[u32] {
        &self.definitions_in_source_order
    }

    pub fn attributes_in_source_order(&self) -> &[u32] {
        &self.attributes_in_source_order
    }

    pub fn prefabs_in_source_order(&self) -> &[String] {
        &self.prefabs_in_source_order
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemInstance {
    pub item_id: u64,
    pub definition: u32,
    pub quality: ItemQuality,
    pub level: u8,
    pub style: u8,
    pub paint: Option<u32>,
    pub team: PlayerTeam,
    pub class: PlayerClass,
    pub slot: LoadoutPosition,
    pub runtime_attributes: Vec<ItemAttribute>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemInstanceInput {
    pub item_id: u64,
    pub definition: u32,
    pub quality: Option<ItemQuality>,
    pub level: u8,
    pub style: u8,
    pub paint: Option<u32>,
    pub team: PlayerTeam,
    pub class: PlayerClass,
    pub runtime_attributes: Vec<ItemAttribute>,
}

impl ItemInstance {
    pub fn create(schema: &ItemSchema, input: ItemInstanceInput) -> Result<Self, SchemaError> {
        let ItemInstanceInput {
            item_id,
            definition,
            quality,
            level,
            style,
            paint,
            team,
            class,
            runtime_attributes,
        } = input;
        let item = schema
            .definition(definition)
            .ok_or(SchemaError::MissingItem(definition))?;
        if !team.is_gameplay() {
            return Err(SchemaError::InvalidItemTeam(team));
        }
        if !item.usable_by.contains(&class) {
            return Err(SchemaError::ItemClassMismatch { definition, class });
        }
        if !(item.minimum_level..=item.maximum_level).contains(&level) {
            return Err(SchemaError::InvalidItemLevel { definition, level });
        }
        if style != 0 && !item.styles.contains(&style) {
            return Err(SchemaError::InvalidItemStyle { definition, style });
        }
        for attribute in &runtime_attributes {
            validate_attribute_value(schema, attribute)?;
        }
        let mut seen = BTreeSet::new();
        if runtime_attributes
            .iter()
            .any(|attribute| !seen.insert(attribute.definition))
        {
            return Err(SchemaError::DuplicateInstanceAttribute);
        }
        Ok(Self {
            item_id,
            definition,
            quality: quality.unwrap_or(item.quality),
            level,
            style,
            paint,
            team,
            class,
            slot: item.slot,
            runtime_attributes,
        })
    }

    pub fn ordered_attributes(
        &self,
        schema: &ItemSchema,
    ) -> Result<Vec<ItemAttribute>, SchemaError> {
        let definition = schema
            .definition(self.definition)
            .ok_or(SchemaError::MissingItem(self.definition))?;
        let runtime_ids: BTreeSet<_> = self
            .runtime_attributes
            .iter()
            .map(|attribute| attribute.definition)
            .collect();
        let mut output = self.runtime_attributes.clone();
        output.extend(
            definition
                .static_attributes
                .iter()
                .filter(|attribute| !runtime_ids.contains(&attribute.definition))
                .cloned(),
        );
        Ok(output)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loadout {
    class: PlayerClass,
    preset: u8,
    class_slots: Vec<Option<ItemInstance>>,
    account_slots: Vec<Option<ItemInstance>>,
}

impl Loadout {
    pub fn empty(class: PlayerClass, preset: u8) -> Result<Self, SchemaError> {
        if preset >= PRESET_COUNT {
            return Err(SchemaError::InvalidPreset(preset));
        }
        Ok(Self {
            class,
            preset,
            class_slots: vec![None; CLASS_SLOT_COUNT],
            account_slots: vec![None; ACCOUNT_SLOT_COUNT],
        })
    }

    pub fn stock(
        schema: &ItemSchema,
        class: PlayerClass,
        team: PlayerTeam,
    ) -> Result<Self, SchemaError> {
        let mut loadout = Self::empty(class, 0)?;
        for stock in class.data().stock_items {
            let position = LoadoutPosition::try_from(stock.slot)?;
            let definition = schema
                .definition(stock.definition)
                .ok_or(SchemaError::MissingItem(stock.definition))?;
            let instance = ItemInstance::create(
                schema,
                ItemInstanceInput {
                    item_id: 0,
                    definition: stock.definition,
                    quality: Some(ItemQuality::Normal),
                    level: definition.minimum_level,
                    style: 0,
                    paint: None,
                    team,
                    class,
                    runtime_attributes: Vec::new(),
                },
            )?;
            loadout.equip(position, instance)?;
        }
        Ok(loadout)
    }

    pub fn class(&self) -> PlayerClass {
        self.class
    }

    pub fn preset(&self) -> u8 {
        self.preset
    }

    pub fn item(&self, position: LoadoutPosition) -> Option<&ItemInstance> {
        self.class_slots[position as usize].as_ref()
    }

    pub fn equip(
        &mut self,
        position: LoadoutPosition,
        item: ItemInstance,
    ) -> Result<Option<ItemInstance>, SchemaError> {
        if item.class != self.class {
            return Err(SchemaError::ItemClassMismatch {
                definition: item.definition,
                class: self.class,
            });
        }
        if item.slot != position {
            return Err(SchemaError::ItemSlotMismatch {
                definition: item.definition,
                expected: position,
                actual: item.slot,
            });
        }
        Ok(self.class_slots[position as usize].replace(item))
    }

    pub fn clear(&mut self, position: LoadoutPosition) -> Option<ItemInstance> {
        self.class_slots[position as usize].take()
    }

    pub fn class_slots(&self) -> &[Option<ItemInstance>] {
        &self.class_slots
    }

    pub fn account_slots(&self) -> &[Option<ItemInstance>] {
        &self.account_slots
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SchemaError {
    WrongContentBuild(u32),
    WrongSchemaHash(String),
    WrongSignatureHash(String),
    RecordLimit,
    DuplicateKey(String),
    DuplicatePrefab(String),
    DuplicateAttribute(u32),
    DuplicateItem(u32),
    InvalidGameInfo {
        key: &'static str,
        expected: &'static str,
        actual: String,
    },
    InvalidPrefabRecord(String),
    InvalidAttributeIdentity(String),
    InvalidItemIdentity(String),
    MissingPrefab(String),
    PrefabCycle(String),
    MissingField {
        record: String,
        field: &'static str,
    },
    InvalidField {
        record: String,
        field: &'static str,
        value: String,
    },
    MissingAttribute(u32),
    AttributeTypeMismatch(u32),
    DuplicateInstanceAttribute,
    MissingItem(u32),
    InvalidClassSlot(u8),
    InvalidPreset(u8),
    InvalidItemTeam(PlayerTeam),
    ItemClassMismatch {
        definition: u32,
        class: PlayerClass,
    },
    ItemSlotMismatch {
        definition: u32,
        expected: LoadoutPosition,
        actual: LoadoutPosition,
    },
    InvalidItemLevel {
        definition: u32,
        level: u8,
    },
    InvalidItemStyle {
        definition: u32,
        style: u8,
    },
}

fn validate_identity(input: &SchemaInput) -> Result<(), SchemaError> {
    if input.content_build != 10_822_003 {
        return Err(SchemaError::WrongContentBuild(input.content_build));
    }
    if input.schema_sha256 != ITEM_SCHEMA_SHA256 {
        return Err(SchemaError::WrongSchemaHash(input.schema_sha256.clone()));
    }
    if input.signature_sha256 != ITEM_SCHEMA_SIGNATURE_SHA256 {
        return Err(SchemaError::WrongSignatureHash(
            input.signature_sha256.clone(),
        ));
    }
    Ok(())
}

fn validate_game_info(nodes: &[SchemaNode]) -> Result<(), SchemaError> {
    for (key, expected) in [
        ("first_valid_class", "1"),
        ("last_valid_class", "9"),
        ("account_class_index", "16"),
        ("account_first_valid_item_slot", "0"),
        ("account_last_valid_item_slot", "3"),
        ("first_valid_item_slot", "0"),
        ("last_valid_item_slot", "18"),
        ("num_item_presets", "4"),
    ] {
        let actual = scalar(nodes, key).ok_or_else(|| SchemaError::MissingField {
            record: "game_info".into(),
            field: key,
        })?;
        if actual != expected {
            return Err(SchemaError::InvalidGameInfo {
                key,
                expected,
                actual: actual.into(),
            });
        }
    }
    Ok(())
}

struct IndexedObjects {
    source_order: Vec<String>,
    by_identity: BTreeMap<String, Vec<SchemaNode>>,
}

fn indexed_objects(nodes: &[SchemaNode], numeric: bool) -> Result<IndexedObjects, SchemaError> {
    let mut output = BTreeMap::new();
    let mut source_order = Vec::with_capacity(nodes.len());
    for node in nodes {
        let SchemaValue::Object(children) = &node.value else {
            return Err(SchemaError::InvalidPrefabRecord(node.key.clone()));
        };
        if numeric && node.key != "default" && node.key.parse::<u32>().is_err() {
            return Err(SchemaError::InvalidItemIdentity(node.key.clone()));
        }
        if output.insert(node.key.clone(), children.clone()).is_some() {
            if numeric {
                return Err(SchemaError::DuplicateKey(node.key.clone()));
            }
            return Err(SchemaError::DuplicatePrefab(node.key.clone()));
        }
        source_order.push(node.key.clone());
    }
    Ok(IndexedObjects {
        source_order,
        by_identity: output,
    })
}

fn validate_unique(nodes: &[SchemaNode]) -> Result<(), SchemaError> {
    let mut previous: Option<&str> = None;
    for node in nodes {
        if previous == Some(node.key.as_str()) {
            return Err(SchemaError::DuplicateKey(node.key.clone()));
        }
        if let SchemaValue::Object(children) = &node.value {
            validate_unique(children)?;
        }
        previous = Some(node.key.as_str());
    }
    Ok(())
}

fn resolve_item(
    fields: &[SchemaNode],
    prefabs: &BTreeMap<String, Vec<SchemaNode>>,
) -> Result<Vec<SchemaNode>, SchemaError> {
    let mut output = Vec::new();
    let mut stack = Vec::new();
    apply_prefab_source(&mut output, fields, prefabs, &mut stack)?;
    Ok(output)
}

fn apply_prefab_source(
    output: &mut Vec<SchemaNode>,
    source: &[SchemaNode],
    prefabs: &BTreeMap<String, Vec<SchemaNode>>,
    stack: &mut Vec<String>,
) -> Result<(), SchemaError> {
    validate_unique(source)?;
    if let Some(names) = scalar(source, "prefab") {
        let names: Vec<_> = names.split_ascii_whitespace().collect();
        for name in names.into_iter().rev() {
            if stack.iter().any(|entry| entry == name) {
                return Err(SchemaError::PrefabCycle(name.into()));
            }
            let prefab = prefabs
                .get(name)
                .ok_or_else(|| SchemaError::MissingPrefab(name.into()))?;
            stack.push(name.into());
            apply_prefab_source(output, prefab, prefabs, stack)?;
            stack.pop();
        }
    }
    overlay(output, source);
    Ok(())
}

fn overlay(output: &mut Vec<SchemaNode>, source: &[SchemaNode]) {
    for source_node in source {
        if let Some(target) = output
            .iter_mut()
            .find(|target| target.key == source_node.key)
        {
            match (&mut target.value, &source_node.value) {
                (SchemaValue::Object(target_children), SchemaValue::Object(source_children)) => {
                    overlay(target_children, source_children);
                }
                (target_value, source_value) => *target_value = source_value.clone(),
            }
        } else {
            output.push(source_node.clone());
        }
    }
}

fn parse_attribute(index: u32, fields: &[SchemaNode]) -> Result<AttributeDefinition, SchemaError> {
    let record = format!("attribute:{index}");
    let name = required_scalar(fields, "name", &record)?.to_owned();
    let class = required_scalar(fields, "attribute_class", &record)?.to_owned();
    let description = required_scalar(fields, "description_format", &record)?;
    let description_format = AttributeDescriptionFormat::parse(description).ok_or_else(|| {
        SchemaError::InvalidField {
            record: record.clone(),
            field: "description_format",
            value: description.into(),
        }
    })?;
    let value_kind = match scalar(fields, "attribute_type") {
        None | Some("float") => AttributeValueKind::Numeric,
        Some("string") => AttributeValueKind::String,
        Some(value) => {
            return Err(SchemaError::InvalidField {
                record,
                field: "attribute_type",
                value: value.into(),
            });
        }
    };
    Ok(AttributeDefinition {
        index,
        name,
        class,
        value_kind,
        description_format,
        stored_as_integer: scalar(fields, "stored_as_integer") == Some("1"),
        hidden: scalar(fields, "hidden") == Some("1"),
    })
}

fn parse_item(
    index: u32,
    fields: Vec<SchemaNode>,
    attributes: &BTreeMap<u32, AttributeDefinition>,
) -> Result<ItemDefinition, SchemaError> {
    let record = format!("item:{index}");
    let name = required_scalar(&fields, "name", &record)?.to_owned();
    let item_class = required_scalar(&fields, "item_class", &record)?.to_owned();
    let slot_name = required_scalar(&fields, "item_slot", &record)?;
    let slot = parse_slot(slot_name).ok_or_else(|| SchemaError::InvalidField {
        record: record.clone(),
        field: "item_slot",
        value: slot_name.into(),
    })?;
    let quality_name = scalar(&fields, "item_quality").unwrap_or("normal");
    let quality = ItemQuality::parse(quality_name).ok_or_else(|| SchemaError::InvalidField {
        record: record.clone(),
        field: "item_quality",
        value: quality_name.into(),
    })?;
    let minimum_level = parse_u8(
        scalar(&fields, "min_ilevel").unwrap_or("1"),
        &record,
        "min_ilevel",
    )?;
    let maximum_level = parse_u8(
        scalar(&fields, "max_ilevel").unwrap_or("1"),
        &record,
        "max_ilevel",
    )?;
    if minimum_level > maximum_level {
        return Err(SchemaError::InvalidField {
            record,
            field: "max_ilevel",
            value: maximum_level.to_string(),
        });
    }
    let usable_by = parse_classes(object(&fields, "used_by_classes"), index)?;
    let static_attributes = parse_item_attributes(
        object(&fields, "attributes").or_else(|| object(&fields, "static_attrs")),
        attributes,
        index,
    )?;
    let mut styles = BTreeSet::new();
    collect_styles(&fields, &mut styles, index)?;
    Ok(ItemDefinition {
        index,
        name,
        item_class,
        slot,
        quality,
        minimum_level,
        maximum_level,
        base_item: scalar(&fields, "baseitem") == Some("1"),
        usable_by,
        styles,
        static_attributes,
        source: fields,
    })
}

fn collect_styles(
    nodes: &[SchemaNode],
    styles: &mut BTreeSet<u8>,
    item: u32,
) -> Result<(), SchemaError> {
    for node in nodes {
        let SchemaValue::Object(children) = &node.value else {
            continue;
        };
        if node.key == "styles" {
            for style in children {
                let value = style
                    .key
                    .parse::<u8>()
                    .map_err(|_| SchemaError::InvalidField {
                        record: format!("item:{item}"),
                        field: "styles",
                        value: style.key.clone(),
                    })?;
                styles.insert(value);
            }
        }
        collect_styles(children, styles, item)?;
    }
    Ok(())
}

fn parse_classes(
    fields: Option<&[SchemaNode]>,
    index: u32,
) -> Result<BTreeSet<PlayerClass>, SchemaError> {
    let Some(fields) = fields else {
        return Ok(PlayerClass::ALL.into_iter().collect());
    };
    let mut classes = BTreeSet::new();
    for field in fields {
        let class = match field.key.as_str() {
            "scout" => PlayerClass::Scout,
            "sniper" => PlayerClass::Sniper,
            "soldier" => PlayerClass::Soldier,
            "demoman" => PlayerClass::Demoman,
            "medic" => PlayerClass::Medic,
            "heavy" => PlayerClass::Heavy,
            "pyro" => PlayerClass::Pyro,
            "spy" => PlayerClass::Spy,
            "engineer" => PlayerClass::Engineer,
            value => {
                return Err(SchemaError::InvalidField {
                    record: format!("item:{index}"),
                    field: "used_by_classes",
                    value: value.into(),
                });
            }
        };
        classes.insert(class);
    }
    Ok(classes)
}

fn parse_item_attributes(
    fields: Option<&[SchemaNode]>,
    definitions: &BTreeMap<u32, AttributeDefinition>,
    item: u32,
) -> Result<Vec<ItemAttribute>, SchemaError> {
    let Some(fields) = fields else {
        return Ok(Vec::new());
    };
    let mut output = Vec::new();
    for field in fields {
        let SchemaValue::Object(values) = &field.value else {
            continue;
        };
        let class = required_scalar(values, "attribute_class", &format!("item:{item}"))?;
        let definition = definitions
            .values()
            .find(|definition| definition.name == field.key)
            .ok_or_else(|| SchemaError::InvalidField {
                record: format!("item:{item}"),
                field: "attribute",
                value: field.key.clone(),
            })?;
        if definition.class != class {
            return Err(SchemaError::InvalidField {
                record: format!("item:{item}"),
                field: "attribute_class",
                value: class.into(),
            });
        }
        let raw = required_scalar(values, "value", &format!("item:{item}"))?;
        let value = match definition.value_kind {
            AttributeValueKind::Numeric => {
                ItemAttributeValue::Numeric(raw.parse::<f32>().map_err(|_| {
                    SchemaError::InvalidField {
                        record: format!("item:{item}"),
                        field: "value",
                        value: raw.into(),
                    }
                })?)
            }
            AttributeValueKind::String => ItemAttributeValue::String(raw.into()),
        };
        output.push(ItemAttribute {
            definition: definition.index,
            value,
        });
    }
    Ok(output)
}

fn validate_attribute_value(
    schema: &ItemSchema,
    attribute: &ItemAttribute,
) -> Result<(), SchemaError> {
    let definition = schema
        .attribute(attribute.definition)
        .ok_or(SchemaError::MissingAttribute(attribute.definition))?;
    let valid = match (definition.value_kind, &attribute.value) {
        (AttributeValueKind::Numeric, ItemAttributeValue::Numeric(value)) => value.is_finite(),
        (AttributeValueKind::String, ItemAttributeValue::String(_)) => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(SchemaError::AttributeTypeMismatch(attribute.definition))
    }
}

fn required_scalar<'a>(
    nodes: &'a [SchemaNode],
    key: &'static str,
    record: &str,
) -> Result<&'a str, SchemaError> {
    scalar(nodes, key).ok_or_else(|| SchemaError::MissingField {
        record: record.into(),
        field: key,
    })
}

fn scalar<'a>(nodes: &'a [SchemaNode], key: &str) -> Option<&'a str> {
    nodes.iter().find_map(|node| {
        if node.key != key {
            return None;
        }
        match &node.value {
            SchemaValue::Scalar(value) => Some(value.as_str()),
            _ => None,
        }
    })
}

fn object<'a>(nodes: &'a [SchemaNode], key: &str) -> Option<&'a [SchemaNode]> {
    nodes.iter().find_map(|node| {
        if node.key != key {
            return None;
        }
        match &node.value {
            SchemaValue::Object(value) => Some(value.as_slice()),
            _ => None,
        }
    })
}

fn parse_u8(value: &str, record: &str, field: &'static str) -> Result<u8, SchemaError> {
    value.parse::<u8>().map_err(|_| SchemaError::InvalidField {
        record: record.into(),
        field,
        value: value.into(),
    })
}

fn parse_slot(value: &str) -> Option<LoadoutPosition> {
    Some(match value {
        "primary" => LoadoutPosition::Primary,
        "secondary" => LoadoutPosition::Secondary,
        "melee" => LoadoutPosition::Melee,
        "utility" => LoadoutPosition::Utility,
        "building" => LoadoutPosition::Building,
        "pda" => LoadoutPosition::Pda,
        "pda2" => LoadoutPosition::Pda2,
        "head" => LoadoutPosition::Head,
        "misc" => LoadoutPosition::Misc,
        "action" => LoadoutPosition::Action,
        "misc2" => LoadoutPosition::Misc2,
        "taunt" => LoadoutPosition::Taunt,
        "taunt2" => LoadoutPosition::Taunt2,
        "taunt3" => LoadoutPosition::Taunt3,
        "taunt4" => LoadoutPosition::Taunt4,
        "taunt5" => LoadoutPosition::Taunt5,
        "taunt6" => LoadoutPosition::Taunt6,
        "taunt7" => LoadoutPosition::Taunt7,
        "taunt8" => LoadoutPosition::Taunt8,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input(
        prefabs: Vec<SchemaNode>,
        attributes: Vec<SchemaNode>,
        mut items: Vec<SchemaNode>,
    ) -> SchemaInput {
        items.insert(
            0,
            SchemaNode::object(
                "default",
                vec![
                    SchemaNode::scalar("name", "default"),
                    SchemaNode::scalar("item_class", "tf_wearable"),
                    SchemaNode::scalar("item_slot", "melee"),
                ],
            ),
        );
        SchemaInput {
            content_build: 10_822_003,
            schema_sha256: ITEM_SCHEMA_SHA256.into(),
            signature_sha256: ITEM_SCHEMA_SIGNATURE_SHA256.into(),
            game_info: [
                ("first_valid_class", "1"),
                ("last_valid_class", "9"),
                ("account_class_index", "16"),
                ("account_first_valid_item_slot", "0"),
                ("account_last_valid_item_slot", "3"),
                ("first_valid_item_slot", "0"),
                ("last_valid_item_slot", "18"),
                ("num_item_presets", "4"),
            ]
            .into_iter()
            .map(|(key, value)| SchemaNode::scalar(key, value))
            .collect(),
            prefabs,
            attributes,
            items,
        }
    }

    fn numeric_attribute(index: u32, class: &str, format: &str) -> SchemaNode {
        SchemaNode::object(
            index.to_string(),
            vec![
                SchemaNode::scalar("name", class),
                SchemaNode::scalar("attribute_class", class),
                SchemaNode::scalar("description_format", format),
                SchemaNode::scalar("stored_as_integer", "0"),
            ],
        )
    }

    #[test]
    fn recursive_prefabs_apply_right_to_left_then_item() {
        let prefabs = vec![
            SchemaNode::object(
                "noun",
                vec![
                    SchemaNode::scalar("item_class", "tf_weapon_base"),
                    SchemaNode::scalar("item_slot", "primary"),
                    SchemaNode::scalar("item_quality", "normal"),
                    SchemaNode::object("used_by_classes", vec![SchemaNode::scalar("soldier", "1")]),
                ],
            ),
            SchemaNode::object(
                "adjective",
                vec![
                    SchemaNode::scalar("prefab", "noun"),
                    SchemaNode::scalar("item_slot", "secondary"),
                ],
            ),
        ];
        let items = vec![SchemaNode::object(
            "18",
            vec![
                SchemaNode::scalar("name", "TF_WEAPON_ROCKETLAUNCHER"),
                SchemaNode::scalar("prefab", "adjective noun"),
                SchemaNode::scalar("item_slot", "primary"),
                SchemaNode::scalar("min_ilevel", "1"),
                SchemaNode::scalar("max_ilevel", "1"),
            ],
        )];
        let schema = ItemSchema::compose(base_input(prefabs, Vec::new(), items)).unwrap();
        let item = schema.definition(18).unwrap();
        assert_eq!(item.item_class, "tf_weapon_base");
        assert_eq!(item.slot, LoadoutPosition::Primary);
        assert_eq!(item.usable_by, BTreeSet::from([PlayerClass::Soldier]));
    }

    #[test]
    fn publication_fails_atomically_for_cycles_duplicates_and_wrong_identity() {
        let cyclic = vec![
            SchemaNode::object("a", vec![SchemaNode::scalar("prefab", "b")]),
            SchemaNode::object("b", vec![SchemaNode::scalar("prefab", "a")]),
        ];
        let item = SchemaNode::object(
            "1",
            vec![
                SchemaNode::scalar("name", "x"),
                SchemaNode::scalar("prefab", "a"),
            ],
        );
        assert!(matches!(
            ItemSchema::compose(base_input(cyclic, Vec::new(), vec![item])),
            Err(SchemaError::PrefabCycle(_))
        ));

        let duplicate = SchemaNode::object(
            "1",
            vec![
                SchemaNode::scalar("name", "x"),
                SchemaNode::scalar("name", "y"),
            ],
        );
        assert!(matches!(
            ItemSchema::compose(base_input(Vec::new(), Vec::new(), vec![duplicate])),
            Err(SchemaError::DuplicateKey(_))
        ));

        let mut wrong = base_input(Vec::new(), Vec::new(), Vec::new());
        wrong.schema_sha256 = "changed".into();
        assert!(matches!(
            ItemSchema::compose(wrong),
            Err(SchemaError::WrongSchemaHash(_))
        ));
    }

    #[test]
    fn item_instance_and_loadout_reject_every_atomic_boundary() {
        let attributes = vec![numeric_attribute(26, "add_maxhealth", "value_is_additive")];
        let item = SchemaNode::object(
            "18",
            vec![
                SchemaNode::scalar("name", "TF_WEAPON_ROCKETLAUNCHER"),
                SchemaNode::scalar("item_class", "tf_weapon_rocketlauncher"),
                SchemaNode::scalar("item_slot", "primary"),
                SchemaNode::scalar("item_quality", "normal"),
                SchemaNode::scalar("min_ilevel", "1"),
                SchemaNode::scalar("max_ilevel", "1"),
                SchemaNode::object("used_by_classes", vec![SchemaNode::scalar("soldier", "1")]),
                SchemaNode::object(
                    "attributes",
                    vec![SchemaNode::object(
                        "add_maxhealth",
                        vec![
                            SchemaNode::scalar("attribute_class", "add_maxhealth"),
                            SchemaNode::scalar("value", "10"),
                        ],
                    )],
                ),
            ],
        );
        let schema = ItemSchema::compose(base_input(Vec::new(), attributes, vec![item])).unwrap();
        let input = |class, team, level, runtime_attributes| ItemInstanceInput {
            item_id: 0,
            definition: 18,
            quality: None,
            level,
            style: 0,
            paint: None,
            team,
            class,
            runtime_attributes,
        };
        assert!(
            ItemInstance::create(
                &schema,
                input(PlayerClass::Scout, PlayerTeam::Red, 1, Vec::new())
            )
            .is_err()
        );
        assert!(
            ItemInstance::create(
                &schema,
                input(PlayerClass::Soldier, PlayerTeam::Spectator, 1, Vec::new(),)
            )
            .is_err()
        );
        assert!(
            ItemInstance::create(
                &schema,
                input(PlayerClass::Soldier, PlayerTeam::Red, 2, Vec::new())
            )
            .is_err()
        );
        assert!(matches!(
            ItemInstance::create(
                &schema,
                ItemInstanceInput {
                    style: 1,
                    ..input(PlayerClass::Soldier, PlayerTeam::Red, 1, Vec::new())
                }
            ),
            Err(SchemaError::InvalidItemStyle {
                definition: 18,
                style: 1
            })
        ));
        let instance = ItemInstance::create(
            &schema,
            input(PlayerClass::Soldier, PlayerTeam::Red, 1, Vec::new()),
        )
        .unwrap();
        let mut loadout = Loadout::empty(PlayerClass::Soldier, 0).unwrap();
        assert!(
            loadout
                .equip(LoadoutPosition::Secondary, instance.clone())
                .is_err()
        );
        assert!(
            loadout
                .equip(LoadoutPosition::Primary, instance)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            loadout.item(LoadoutPosition::Primary).unwrap().definition,
            18
        );
        assert!(Loadout::empty(PlayerClass::Soldier, 4).is_err());

        let runtime = ItemAttribute {
            definition: 26,
            value: ItemAttributeValue::Numeric(5.0),
        };
        let instance = ItemInstance::create(
            &schema,
            ItemInstanceInput {
                item_id: 1,
                runtime_attributes: vec![runtime.clone()],
                ..input(PlayerClass::Soldier, PlayerTeam::Red, 1, Vec::new())
            },
        )
        .unwrap();
        assert_eq!(
            instance.ordered_attributes(&schema).unwrap(),
            vec![runtime.clone()]
        );
        assert!(matches!(
            ItemInstance::create(
                &schema,
                ItemInstanceInput {
                    item_id: 1,
                    runtime_attributes: vec![runtime.clone(), runtime],
                    ..input(PlayerClass::Soldier, PlayerTeam::Red, 1, Vec::new())
                },
            ),
            Err(SchemaError::DuplicateInstanceAttribute)
        ));
    }
}
