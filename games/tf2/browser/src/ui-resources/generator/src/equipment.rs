use std::{collections::{BTreeMap, BTreeSet}, fmt::Write, fs, path::Path};
use playsrc_content::Content;
use crate::schema::{self, ItemSchema, SchemaInput, SchemaNode, SchemaValue};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    definition_index: u32,
    weapon: Option<String>,
    #[serde(default)] quality: u8,
    #[serde(default)] style: u8,
    #[serde(default)] attributes: Vec<Attribute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attribute { definition: u32, value: f32 }

fn convert(node: &super::NodeRecord) -> SchemaNode {
    match &node.value {
        Some(value) => SchemaNode::scalar(&node.name, value),
        None => SchemaNode::object(&node.name, node.children.iter().map(convert).collect()),
    }
}

fn scalar<'a>(nodes: &'a [SchemaNode], key: &str) -> Option<&'a str> {
    nodes.iter().find_map(|node| match &node.value {
        SchemaValue::Scalar(value) if node.key == key => Some(value.as_str()), _ => None,
    })
}

fn object<'a>(nodes: &'a [SchemaNode], key: &str) -> &'a [SchemaNode] {
    nodes.iter().find_map(|node| match &node.value {
        SchemaValue::Object(value) if node.key == key => Some(value.as_slice()), _ => None,
    }).unwrap_or(&[])
}

fn pairs(nodes: &[SchemaNode], prefix: &str) -> String {
    nodes.iter().filter_map(|node| match &node.value {
        SchemaValue::Scalar(value) if node.key.starts_with(prefix) => Some(format!("({:?}, {:?})", node.key, value)), _ => None,
    }).collect::<Vec<_>>().join(", ")
}

fn closure(node: &SchemaNode, prefabs: &BTreeMap<String, SchemaNode>, selected: &mut BTreeSet<String>) -> Result<(), String> {
    if let SchemaValue::Object(fields) = &node.value {
        for name in scalar(fields, "prefab").unwrap_or("").split_whitespace() {
            if selected.insert(name.into()) {
                closure(prefabs.get(name).ok_or_else(|| format!("missing prefab {name}"))?, prefabs, selected)?;
            }
        }
    }
    Ok(())
}

fn attribute_names(node: &SchemaNode, names: &mut BTreeSet<String>) {
    if let SchemaValue::Object(fields) = &node.value {
        if matches!(node.key.as_str(), "attributes" | "static_attrs") {
            names.extend(fields.iter().map(|field| field.key.clone()));
        } else {
            for field in fields { attribute_names(field, names); }
        }
    }
}

fn emit(nodes: &[SchemaNode]) -> String {
    format!("vec![{}]", nodes.iter().map(|node| match &node.value {
        SchemaValue::Scalar(value) => format!("SchemaNode::scalar({:?}, {:?})", node.key, value),
        SchemaValue::Object(fields) => format!("SchemaNode::object({:?}, {})", node.key, emit(fields)),
    }).collect::<Vec<_>>().join(",\n"))
}

pub struct GeneratedEquipment { pub images: Vec<String>, pub tokens: Vec<String> }

pub fn generate(content: &Content, repository: &Path) -> Result<GeneratedEquipment, String> {
    let supported_items: Vec<Registration> = serde_json::from_slice(&fs::read(repository.join("games/tf2/rust/equipment/catalog.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut unique = BTreeSet::new();
    for item in &supported_items {
        if !unique.insert(item.definition_index) || item.definition_index == u32::MAX || item.quality > 15 || item.attributes.len() > 16
            || item.attributes.iter().any(|attribute| !attribute.value.is_finite())
            || item.weapon.as_ref().is_some_and(|name| name.is_empty() || !name.bytes().all(|c| c.is_ascii_alphanumeric())) {
            return Err("invalid supported item registration".into());
        }
    }
    let (provenance, bytes) = super::dependency(content, "scripts/items/items_game.txt")?;
    if provenance.sha256.as_deref() != Some(schema::ITEM_SCHEMA_SHA256) {
        return Err("configured equipment item schema identity changed".into());
    }
    let (_, _, _, _, nodes) = super::parse_summary("equipment", "scripts/items/items_game.txt", &bytes.ok_or("missing item schema")?)?;
    let root = &nodes.first().ok_or("missing item root")?.children;
    let section = |key: &str| -> Result<Vec<SchemaNode>, String> {
        Ok(root.iter().find(|node| node.name == key).ok_or_else(|| format!("missing section {key}"))?.children.iter().map(convert).collect())
    };
    let definitions: BTreeSet<_> = supported_items.iter().map(|item| item.definition_index.to_string()).collect();
    let items: Vec<_> = section("items")?.into_iter().filter(|item| item.key == "default" || definitions.contains(&item.key)).collect();
    let prefab_nodes = section("prefabs")?;
    let prefabs: BTreeMap<_, _> = prefab_nodes.iter().map(|node| (node.key.clone(), node.clone())).collect();
    let mut selected = BTreeSet::new();
    for node in &items { closure(node, &prefabs, &mut selected)?; }
    let prefabs: Vec<_> = prefab_nodes.into_iter().filter(|node| selected.contains(&node.key)).collect();
    let mut names = BTreeSet::new();
    for node in items.iter().chain(&prefabs) { attribute_names(node, &mut names); }
    for item in &supported_items { names.extend(item.attributes.iter().map(|attribute| attribute.definition.to_string())); }
    let attributes: Vec<_> = section("attributes")?.into_iter().filter(|node| {
        names.contains(&node.key) || match &node.value {
            SchemaValue::Object(fields) => scalar(fields, "name").is_some_and(|name| names.contains(name)), _ => false,
        }
    }).collect();
    let game_info = section("game_info")?;
    let schema = ItemSchema::compose(SchemaInput { content_build: schema::CONTENT_BUILD, schema_sha256: schema::ITEM_SCHEMA_SHA256.into(),
        signature_sha256: schema::ITEM_SCHEMA_SIGNATURE_SHA256.into(), game_info: game_info.clone(), prefabs: prefabs.clone(), attributes: attributes.clone(), items: items.clone() })
        .map_err(|error| format!("equipment schema: {error:?}"))?;
    let mut output = format!("// Generated from configured items_game SHA-256 {}.\n// Regenerate with generate:tf2-ui.\npub const ITEM_PRESENTATIONS: &[ItemPresentation] = &[\n", schema::ITEM_SCHEMA_SHA256);
    let mut generated = GeneratedEquipment { images: Vec::new(), tokens: Vec::new() };
    for supported in &supported_items {
        let item = schema.definition(supported.definition_index).ok_or("supported definition missing")?;
        let name = scalar(&item.source, "item_name").ok_or("item localization missing")?;
        let image = scalar(&item.source, "image_inventory").ok_or("item image missing")?;
        generated.images.push(format!("../{image}"));
        generated.tokens.push(name.into());
        let visuals = object(&item.source, "visuals");
        writeln!(output, "ItemPresentation {{ definition_index: {}, name: {:?}, image: {:?}, model_player: {:?}, attach_to_hands: {}, animation_replacements: &[{}], sound_overrides: &[{}], death_notice_icon: {:?}, class_slots: &[{}] }},", item.index, name, image,
            scalar(&item.source, "model_player").unwrap_or(""), scalar(&item.source, "attach_to_hands") == Some("1"), pairs(object(visuals, "animation_replacement"), ""), pairs(visuals, "sound_"),
            scalar(&item.source, "item_iconname"),
            item.class_slots.iter().map(|(class, slot)| format!("(PlayerClass::{class:?}, LoadoutPosition::{slot:?})")).collect::<Vec<_>>().join(", ")).unwrap();
    }
    output.push_str("];\npub const SUPPORTED_ITEMS: &[SupportedItem] = &[\n");
    for item in &supported_items {
        let implementation = item.weapon.as_ref().map_or("Implementation::Wearable".into(), |weapon| format!("Implementation::Weapon(Weapon::{weapon})"));
        writeln!(output, "SupportedItem {{ definition_index: {}, implementation: {}, quality: {}, style: {}, attributes: &[{}] }},", item.definition_index, implementation, item.quality, item.style,
            item.attributes.iter().map(|attribute| format!("ItemAttribute {{ definition: {}, value: {:?} }}", attribute.definition, attribute.value)).collect::<Vec<_>>().join(", ")).unwrap();
    }
    output.push_str("];\nfn configured_schema_input() -> crate::schema::SchemaInput {\nuse crate::schema::{SchemaInput, SchemaNode};\nSchemaInput {\n");
    writeln!(output, "content_build: {}, schema_sha256: {:?}.into(), signature_sha256: {:?}.into(),", schema::CONTENT_BUILD, schema::ITEM_SCHEMA_SHA256, schema::ITEM_SCHEMA_SIGNATURE_SHA256).unwrap();
    for (name, nodes) in [("game_info", game_info), ("prefabs", prefabs), ("attributes", attributes), ("items", items)] {
        writeln!(output, "{name}: {},", emit(&nodes)).unwrap();
    }
    output.push_str("}\n}\n");
    fs::write(repository.join("games/tf2/rust/src/equipment.generated.rs"), output).map_err(|e| e.to_string())?;
    Ok(generated)
}
