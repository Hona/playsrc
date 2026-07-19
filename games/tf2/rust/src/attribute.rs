use std::collections::{BTreeMap, BTreeSet};

use crate::schema::{
    AttributeDescriptionFormat, AttributeValueKind, ItemAttribute, ItemAttributeValue, ItemSchema,
};

pub type AttributeEntityId = u32;
pub const MAX_ATTRIBUTE_ENTITIES: usize = 8_192;
pub const MAX_ATTRIBUTES_PER_ENTITY: usize = 4_096;
pub const MAX_PROVIDERS_PER_ENTITY: usize = 256;
pub const MAX_CACHE_RECORDS_PER_ENTITY: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    Generic,
    Player,
    Weapon,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryValue {
    Numeric(f32),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryStep {
    pub entity: AttributeEntityId,
    pub definition: u32,
    pub before: QueryValue,
    pub after: QueryValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    pub value: QueryValue,
    pub visited: Vec<AttributeEntityId>,
    pub steps: Vec<QueryStep>,
    pub matching_items: Vec<AttributeEntityId>,
    pub cache_hit: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct CacheEntry {
    hook: String,
    input: QueryValue,
    output: QueryValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeEntity {
    pub identity: AttributeEntityId,
    pub kind: ProviderKind,
    pub attributes: Vec<ItemAttribute>,
    pub providers: Vec<AttributeEntityId>,
    pub owner: Option<AttributeEntityId>,
    receivers: Vec<AttributeEntityId>,
    cache: Vec<CacheEntry>,
    provision_parity: u8,
}

impl AttributeEntity {
    pub fn new(identity: AttributeEntityId, kind: ProviderKind) -> Self {
        Self {
            identity,
            kind,
            attributes: Vec::new(),
            providers: Vec::new(),
            owner: None,
            receivers: Vec::new(),
            cache: Vec::new(),
            provision_parity: 0,
        }
    }

    pub fn provision_parity(&self) -> u8 {
        self.provision_parity
    }
}

#[derive(Clone, Debug, Default)]
pub struct AttributeGraph {
    entities: BTreeMap<AttributeEntityId, AttributeEntity>,
    global_cache_version: u32,
    entity_cache_versions: BTreeMap<AttributeEntityId, u32>,
}

impl AttributeGraph {
    pub fn insert(&mut self, entity: AttributeEntity) -> Result<(), AttributeError> {
        if self.entities.len() >= MAX_ATTRIBUTE_ENTITIES {
            return Err(AttributeError::EntityLimit);
        }
        if self.entities.contains_key(&entity.identity) {
            return Err(AttributeError::DuplicateEntity);
        }
        self.entities.insert(entity.identity, entity);
        Ok(())
    }

    pub fn entity(&self, identity: AttributeEntityId) -> Option<&AttributeEntity> {
        self.entities.get(&identity)
    }

    pub fn remove(
        &mut self,
        identity: AttributeEntityId,
    ) -> Result<AttributeEntity, AttributeError> {
        self.require_entity(identity)?;
        let removed = self
            .entities
            .remove(&identity)
            .expect("validated attribute entity");
        let mut invalidated = BTreeSet::new();
        for provider in &removed.providers {
            if let Some(node) = self.entities.get_mut(provider)
                && let Some(index) = node
                    .receivers
                    .iter()
                    .position(|receiver| *receiver == identity)
            {
                node.receivers.swap_remove(index);
                increment_parity(node);
            }
        }
        for receiver in &removed.receivers {
            if let Some(node) = self.entities.get_mut(receiver)
                && let Some(index) = node
                    .providers
                    .iter()
                    .position(|provider| *provider == identity)
            {
                node.providers.swap_remove(index);
            }
            self.invalidate_inner(*receiver, &mut invalidated);
        }
        let owners: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(entity, node)| (node.owner == Some(identity)).then_some(*entity))
            .collect();
        for entity in owners {
            self.entities
                .get_mut(&entity)
                .expect("known owner receiver")
                .owner = None;
            self.invalidate_inner(entity, &mut invalidated);
        }
        self.entity_cache_versions.remove(&identity);
        Ok(removed)
    }

    pub fn set_owner(
        &mut self,
        entity: AttributeEntityId,
        owner: Option<AttributeEntityId>,
    ) -> Result<(), AttributeError> {
        self.require_entity(entity)?;
        if let Some(owner) = owner {
            self.require_entity(owner)?;
            if owner == entity || self.owner_chain_contains(owner, entity) {
                return Err(AttributeError::ProviderCycle);
            }
        }
        self.entities
            .get_mut(&entity)
            .expect("validated entity")
            .owner = owner;
        self.invalidate(entity)?;
        Ok(())
    }

    pub fn set_attributes(
        &mut self,
        entity: AttributeEntityId,
        attributes: Vec<ItemAttribute>,
    ) -> Result<(), AttributeError> {
        if attributes.len() > MAX_ATTRIBUTES_PER_ENTITY {
            return Err(AttributeError::AttributeLimit);
        }
        self.require_entity(entity)?;
        self.entities
            .get_mut(&entity)
            .expect("validated entity")
            .attributes = attributes;
        self.invalidate(entity)
    }

    pub fn provide_to(
        &mut self,
        provider: AttributeEntityId,
        receiver: AttributeEntityId,
    ) -> Result<(), AttributeError> {
        self.require_entity(provider)?;
        self.require_entity(receiver)?;
        if provider == receiver
            || self.entities[&receiver].providers.contains(&provider)
            || self.provider_chain_contains(provider, receiver)
        {
            return Err(AttributeError::ProviderCycle);
        }
        if self.entities[&receiver].providers.len() >= MAX_PROVIDERS_PER_ENTITY {
            return Err(AttributeError::ProviderLimit);
        }
        self.entities
            .get_mut(&receiver)
            .expect("validated receiver")
            .providers
            .push(provider);
        self.entities
            .get_mut(&provider)
            .expect("validated provider")
            .receivers
            .push(receiver);
        increment_parity(
            self.entities
                .get_mut(&provider)
                .expect("validated provider"),
        );
        self.invalidate(receiver)
    }

    pub fn stop_providing_to(
        &mut self,
        provider: AttributeEntityId,
        receiver: AttributeEntityId,
    ) -> Result<bool, AttributeError> {
        self.require_entity(provider)?;
        self.require_entity(receiver)?;
        let Some(index) = self.entities[&receiver]
            .providers
            .iter()
            .position(|value| *value == provider)
        else {
            return Ok(false);
        };
        self.entities
            .get_mut(&receiver)
            .expect("validated receiver")
            .providers
            .swap_remove(index);
        if let Some(index) = self.entities[&provider]
            .receivers
            .iter()
            .position(|value| *value == receiver)
        {
            self.entities
                .get_mut(&provider)
                .expect("validated provider")
                .receivers
                .swap_remove(index);
        }
        increment_parity(
            self.entities
                .get_mut(&provider)
                .expect("validated provider"),
        );
        self.invalidate(receiver)?;
        Ok(true)
    }

    pub fn set_global_cache_version(&mut self, version: u32) {
        self.global_cache_version = version;
    }

    pub fn query_numeric(
        &mut self,
        schema: &ItemSchema,
        root: AttributeEntityId,
        initiator: AttributeEntityId,
        hook: &str,
        input: f32,
        collect_items: bool,
    ) -> Result<QueryResult, AttributeError> {
        if !input.is_finite() {
            return Err(AttributeError::NonFiniteValue);
        }
        self.query(
            schema,
            root,
            initiator,
            hook,
            QueryValue::Numeric(input),
            collect_items,
        )
    }

    pub fn query_string(
        &mut self,
        schema: &ItemSchema,
        root: AttributeEntityId,
        initiator: AttributeEntityId,
        hook: &str,
        input: impl Into<String>,
        collect_items: bool,
    ) -> Result<QueryResult, AttributeError> {
        self.query(
            schema,
            root,
            initiator,
            hook,
            QueryValue::String(input.into()),
            collect_items,
        )
    }

    pub fn invalidate(&mut self, entity: AttributeEntityId) -> Result<(), AttributeError> {
        self.require_entity(entity)?;
        let mut visited = BTreeSet::new();
        self.invalidate_inner(entity, &mut visited);
        Ok(())
    }

    fn query(
        &mut self,
        schema: &ItemSchema,
        root: AttributeEntityId,
        initiator: AttributeEntityId,
        hook: &str,
        input: QueryValue,
        collect_items: bool,
    ) -> Result<QueryResult, AttributeError> {
        self.require_entity(root)?;
        self.require_entity(initiator)?;
        self.validate_graph()?;
        let cached_version = self.entity_cache_versions.get(&root).copied().unwrap_or(0);
        if cached_version != self.global_cache_version {
            self.invalidate(root)?;
            self.entity_cache_versions
                .insert(root, self.global_cache_version);
        }
        let mut replacement_index = None;
        if !collect_items {
            let cache = &self.entities.get(&root).expect("validated root").cache;
            if let Some(index) = cache.iter().rposition(|entry| entry.hook == hook) {
                if cache[index].input == input {
                    return Ok(QueryResult {
                        value: cache[index].output.clone(),
                        visited: Vec::new(),
                        steps: Vec::new(),
                        matching_items: Vec::new(),
                        cache_hit: true,
                    });
                }
                replacement_index = Some(index);
            }
        }

        let mut result = QueryResult {
            value: input.clone(),
            visited: Vec::new(),
            steps: Vec::new(),
            matching_items: Vec::new(),
            cache_hit: false,
        };
        let initiator_kind = self.entities[&initiator].kind;
        let mut active = BTreeSet::new();
        self.apply_entity(
            schema,
            root,
            initiator,
            initiator_kind,
            hook,
            collect_items,
            &mut result,
            &mut active,
        )?;
        if !collect_items {
            let cache = &mut self.entities.get_mut(&root).expect("validated root").cache;
            if let Some(index) = replacement_index {
                cache.remove(index);
            }
            if cache.len() >= MAX_CACHE_RECORDS_PER_ENTITY {
                return Err(AttributeError::CacheLimit);
            }
            cache.push(CacheEntry {
                hook: hook.into(),
                input,
                output: result.value.clone(),
            });
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_entity(
        &self,
        schema: &ItemSchema,
        entity: AttributeEntityId,
        initiator: AttributeEntityId,
        initiator_kind: ProviderKind,
        hook: &str,
        collect_items: bool,
        result: &mut QueryResult,
        active: &mut BTreeSet<AttributeEntityId>,
    ) -> Result<(), AttributeError> {
        if !active.insert(entity) {
            return Ok(());
        }
        let node = self
            .entities
            .get(&entity)
            .ok_or(AttributeError::MissingEntity(entity))?;
        result.visited.push(entity);
        let mut found_string = false;
        for attribute in &node.attributes {
            let definition = schema
                .attribute(attribute.definition)
                .ok_or(AttributeError::MissingDefinition(attribute.definition))?;
            if definition.class != hook {
                continue;
            }
            let before = result.value.clone();
            match (&mut result.value, &attribute.value, definition.value_kind) {
                (
                    QueryValue::Numeric(current),
                    ItemAttributeValue::Numeric(modifier),
                    AttributeValueKind::Numeric,
                ) => {
                    if !modifier.is_finite() {
                        return Err(AttributeError::NonFiniteValue);
                    }
                    *current = apply_numeric(*current, *modifier, definition.description_format)
                        .ok_or(AttributeError::UnsupportedDescriptionFormat(
                            attribute.definition,
                        ))?;
                    if !current.is_finite() {
                        return Err(AttributeError::NonFiniteValue);
                    }
                    if collect_items && !result.matching_items.contains(&entity) {
                        result.matching_items.push(entity);
                    }
                }
                (
                    QueryValue::String(current),
                    ItemAttributeValue::String(value),
                    AttributeValueKind::String,
                ) => {
                    if !found_string {
                        *current = value.clone();
                        found_string = true;
                    }
                }
                _ => return Err(AttributeError::ValueTypeMismatch(attribute.definition)),
            }
            result.steps.push(QueryStep {
                entity,
                definition: attribute.definition,
                before,
                after: result.value.clone(),
            });
        }

        for provider in &node.providers {
            if *provider == initiator {
                continue;
            }
            let provider_node = self
                .entities
                .get(provider)
                .ok_or(AttributeError::MissingEntity(*provider))?;
            if provider_node.kind == ProviderKind::Weapon && initiator_kind == ProviderKind::Weapon
            {
                continue;
            }
            self.apply_entity(
                schema,
                *provider,
                initiator,
                initiator_kind,
                hook,
                collect_items,
                result,
                active,
            )?;
        }
        if let Some(owner) = node.owner {
            self.apply_entity(
                schema,
                owner,
                initiator,
                initiator_kind,
                hook,
                collect_items,
                result,
                active,
            )?;
        }
        active.remove(&entity);
        Ok(())
    }

    fn require_entity(&self, identity: AttributeEntityId) -> Result<(), AttributeError> {
        self.entities
            .contains_key(&identity)
            .then_some(())
            .ok_or(AttributeError::MissingEntity(identity))
    }

    fn validate_graph(&self) -> Result<(), AttributeError> {
        for identity in self.entities.keys() {
            if self.provider_chain_contains(*identity, *identity) {
                return Err(AttributeError::ProviderCycle);
            }
        }
        Ok(())
    }

    fn provider_chain_contains(&self, start: AttributeEntityId, target: AttributeEntityId) -> bool {
        self.providers_reach(start, target, &mut BTreeSet::new())
    }

    fn providers_reach(
        &self,
        start: AttributeEntityId,
        target: AttributeEntityId,
        visited: &mut BTreeSet<AttributeEntityId>,
    ) -> bool {
        if !visited.insert(start) {
            return false;
        }
        let Some(node) = self.entities.get(&start) else {
            return false;
        };
        node.providers
            .iter()
            .copied()
            .any(|next| next == target || self.providers_reach(next, target, visited))
    }

    fn owner_chain_contains(&self, start: AttributeEntityId, target: AttributeEntityId) -> bool {
        let mut current = Some(start);
        let mut visited = BTreeSet::new();
        while let Some(identity) = current {
            if identity == target {
                return true;
            }
            if !visited.insert(identity) {
                return true;
            }
            current = self.entities.get(&identity).and_then(|entity| entity.owner);
        }
        false
    }

    fn invalidate_inner(
        &mut self,
        entity: AttributeEntityId,
        visited: &mut BTreeSet<AttributeEntityId>,
    ) {
        if !visited.insert(entity) {
            return;
        }
        let receivers = self
            .entities
            .get(&entity)
            .map(|node| node.receivers.clone())
            .unwrap_or_default();
        if let Some(node) = self.entities.get_mut(&entity) {
            node.cache.clear();
            increment_parity(node);
        }
        for receiver in receivers {
            self.invalidate_inner(receiver, visited);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeError {
    DuplicateEntity,
    EntityLimit,
    AttributeLimit,
    ProviderLimit,
    CacheLimit,
    MissingEntity(AttributeEntityId),
    MissingDefinition(u32),
    ValueTypeMismatch(u32),
    NonFiniteValue,
    ProviderCycle,
    UnsupportedDescriptionFormat(u32),
}

fn increment_parity(entity: &mut AttributeEntity) {
    entity.provision_parity = entity.provision_parity.wrapping_add(1) & 0x3f;
}

fn apply_numeric(value: f32, modifier: f32, format: AttributeDescriptionFormat) -> Option<f32> {
    Some(match format {
        AttributeDescriptionFormat::Percentage | AttributeDescriptionFormat::InvertedPercentage => {
            value * modifier
        }
        AttributeDescriptionFormat::Additive
        | AttributeDescriptionFormat::AdditivePercentage
        | AttributeDescriptionFormat::ParticleIndex => value + modifier,
        AttributeDescriptionFormat::KillstreakEffectIndex
        | AttributeDescriptionFormat::KillstreakIdleEffectIndex
        | AttributeDescriptionFormat::Lookup => modifier,
        AttributeDescriptionFormat::Or => ((value as i32) | (modifier as i32)) as f32,
        AttributeDescriptionFormat::Date
        | AttributeDescriptionFormat::AccountId
        | AttributeDescriptionFormat::ItemDefinition => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{
        ITEM_SCHEMA_SHA256, ITEM_SCHEMA_SIGNATURE_SHA256, SchemaInput, SchemaNode,
    };

    fn schema() -> ItemSchema {
        let attributes = [
            (1, "multiply", "value_is_percentage"),
            (2, "add", "value_is_additive"),
            (3, "bits", "value_is_or"),
            (4, "text", "value_is_additive"),
            (5, "inverted", "value_is_inverted_percentage"),
            (6, "lookup", "value_is_from_lookup_table"),
            (7, "particle", "value_is_particle_index"),
        ]
        .into_iter()
        .map(|(index, class, format)| {
            SchemaNode::object(
                index.to_string(),
                vec![
                    SchemaNode::scalar("name", class),
                    SchemaNode::scalar("attribute_class", class),
                    SchemaNode::scalar("description_format", format),
                    SchemaNode::scalar(
                        "attribute_type",
                        if class == "text" { "string" } else { "float" },
                    ),
                ],
            )
        })
        .collect();
        ItemSchema::compose(SchemaInput {
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
            prefabs: Vec::new(),
            attributes,
            items: vec![SchemaNode::object(
                "default",
                vec![
                    SchemaNode::scalar("name", "default"),
                    SchemaNode::scalar("item_class", "tf_wearable"),
                    SchemaNode::scalar("item_slot", "melee"),
                ],
            )],
        })
        .unwrap()
    }

    fn numeric(definition: u32, value: f32) -> ItemAttribute {
        ItemAttribute {
            definition,
            value: ItemAttributeValue::Numeric(value),
        }
    }

    #[test]
    fn query_order_is_local_then_providers_then_owner_with_weapon_suppression() {
        let schema = schema();
        let mut graph = AttributeGraph::default();
        for (id, kind, attributes) in [
            (1, ProviderKind::Player, vec![numeric(2, 1.0)]),
            (2, ProviderKind::Generic, vec![numeric(1, 2.0)]),
            (3, ProviderKind::Weapon, vec![numeric(2, 10.0)]),
            (4, ProviderKind::Generic, vec![numeric(2, 3.0)]),
        ] {
            let mut entity = AttributeEntity::new(id, kind);
            entity.attributes = attributes;
            graph.insert(entity).unwrap();
        }
        graph.provide_to(2, 1).unwrap();
        graph.provide_to(3, 1).unwrap();
        graph.set_owner(1, Some(4)).unwrap();
        let result = graph
            .query_numeric(&schema, 1, 1, "add", 0.0, true)
            .unwrap();
        assert_eq!(result.value, QueryValue::Numeric(14.0));
        assert_eq!(result.visited, vec![1, 2, 3, 4]);
        assert_eq!(result.matching_items, vec![1, 3, 4]);

        let result = graph
            .query_numeric(&schema, 1, 3, "add", 0.0, true)
            .unwrap();
        assert_eq!(result.value, QueryValue::Numeric(4.0));
        assert_eq!(result.visited, vec![1, 2, 4]);
    }

    #[test]
    fn cache_replaces_same_hook_different_input_and_item_list_bypasses_it() {
        let schema = schema();
        let mut graph = AttributeGraph::default();
        let mut entity = AttributeEntity::new(1, ProviderKind::Player);
        entity.attributes.push(numeric(1, 2.0));
        graph.insert(entity).unwrap();
        assert!(
            !graph
                .query_numeric(&schema, 1, 1, "multiply", 2.0, false)
                .unwrap()
                .cache_hit
        );
        assert!(
            graph
                .query_numeric(&schema, 1, 1, "multiply", 2.0, false)
                .unwrap()
                .cache_hit
        );
        assert_eq!(
            graph
                .query_numeric(&schema, 1, 1, "multiply", 3.0, false)
                .unwrap()
                .value,
            QueryValue::Numeric(6.0)
        );
        assert!(
            !graph
                .query_numeric(&schema, 1, 1, "multiply", 3.0, true)
                .unwrap()
                .cache_hit
        );
        graph.set_attributes(1, vec![numeric(1, 3.0)]).unwrap();
        assert_eq!(
            graph
                .query_numeric(&schema, 1, 1, "multiply", 3.0, false)
                .unwrap()
                .value,
            QueryValue::Numeric(9.0)
        );
    }

    #[test]
    fn every_selected_combination_and_string_branch_is_typed() {
        let schema = schema();
        let mut graph = AttributeGraph::default();
        let mut entity = AttributeEntity::new(1, ProviderKind::Player);
        entity.attributes = vec![
            numeric(5, 0.5),
            numeric(6, 7.0),
            numeric(7, 2.0),
            ItemAttribute {
                definition: 4,
                value: ItemAttributeValue::String("first".into()),
            },
            ItemAttribute {
                definition: 4,
                value: ItemAttributeValue::String("second".into()),
            },
            numeric(3, 4.0),
        ];
        graph.insert(entity).unwrap();
        assert_eq!(
            graph
                .query_numeric(&schema, 1, 1, "inverted", 10.0, true)
                .unwrap()
                .value,
            QueryValue::Numeric(5.0)
        );
        assert_eq!(
            graph
                .query_numeric(&schema, 1, 1, "lookup", 10.0, true)
                .unwrap()
                .value,
            QueryValue::Numeric(7.0)
        );
        assert_eq!(
            graph
                .query_numeric(&schema, 1, 1, "particle", 10.0, true)
                .unwrap()
                .value,
            QueryValue::Numeric(12.0)
        );
        assert_eq!(
            graph
                .query_numeric(&schema, 1, 1, "bits", 3.0, true)
                .unwrap()
                .value,
            QueryValue::Numeric(7.0)
        );
        let string = graph
            .query_string(&schema, 1, 1, "text", "base", true)
            .unwrap();
        assert_eq!(string.value, QueryValue::String("first".into()));
        assert!(string.matching_items.is_empty());
        assert!(matches!(
            graph.query_numeric(&schema, 1, 1, "text", 1.0, true),
            Err(AttributeError::ValueTypeMismatch(4))
        ));
    }

    #[test]
    fn fast_remove_reorders_providers_and_cycles_fail_before_mutation() {
        let mut graph = AttributeGraph::default();
        for id in 1..=4 {
            graph
                .insert(AttributeEntity::new(id, ProviderKind::Generic))
                .unwrap();
        }
        graph.provide_to(2, 1).unwrap();
        graph.provide_to(3, 1).unwrap();
        graph.provide_to(4, 1).unwrap();
        graph.stop_providing_to(2, 1).unwrap();
        assert_eq!(graph.entity(1).unwrap().providers, vec![4, 3]);
        assert!(matches!(
            graph.provide_to(1, 4),
            Err(AttributeError::ProviderCycle)
        ));
        assert_eq!(graph.entity(4).unwrap().providers, Vec::<u32>::new());
    }

    #[test]
    fn six_bit_provision_parity_wraps() {
        let mut graph = AttributeGraph::default();
        graph
            .insert(AttributeEntity::new(1, ProviderKind::Generic))
            .unwrap();
        for _ in 0..64 {
            graph.invalidate(1).unwrap();
        }
        assert_eq!(graph.entity(1).unwrap().provision_parity(), 0);
    }

    #[test]
    fn attribute_and_provider_bounds_fail_before_mutation() {
        let mut graph = AttributeGraph::default();
        graph
            .insert(AttributeEntity::new(1, ProviderKind::Player))
            .unwrap();
        assert_eq!(
            graph.set_attributes(1, vec![numeric(1, 1.0); MAX_ATTRIBUTES_PER_ENTITY + 1]),
            Err(AttributeError::AttributeLimit)
        );
        assert!(graph.entity(1).unwrap().attributes.is_empty());

        for identity in 2..=(MAX_PROVIDERS_PER_ENTITY as u32 + 2) {
            graph
                .insert(AttributeEntity::new(identity, ProviderKind::Generic))
                .unwrap();
        }
        for provider in 2..=(MAX_PROVIDERS_PER_ENTITY as u32 + 1) {
            graph.provide_to(provider, 1).unwrap();
        }
        assert_eq!(
            graph.provide_to(MAX_PROVIDERS_PER_ENTITY as u32 + 2, 1),
            Err(AttributeError::ProviderLimit)
        );
        assert_eq!(
            graph.entity(1).unwrap().providers.len(),
            MAX_PROVIDERS_PER_ENTITY
        );
    }
}
