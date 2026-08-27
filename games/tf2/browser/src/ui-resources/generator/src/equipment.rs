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
    #[serde(default = "implemented_by_default")] implemented: bool,
}
fn implemented_by_default() -> bool { true }

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

fn format_localized(format: &str, arguments: &[&str]) -> String {
    let mut text = format.to_owned();
    for (index, value) in arguments.iter().enumerate() { text = text.replace(&format!("%s{}", index + 1), value); }
    text
}

fn display(
    item: &schema::ItemDefinition, registration: &Registration, schema: &ItemSchema,
    attributes: &[SchemaNode], colors: &[SchemaNode], localized: &BTreeMap<String, String>,
) -> Result<(String, Vec<(String, String)>), String> {
    let loc = |token: &str| -> Result<&str, String> {
        localized.get(&token.trim_start_matches('#').to_lowercase()).map(String::as_str).ok_or_else(|| format!("missing equipment localization {token}"))
    };
    let color = |key: &str| -> Result<String, String> {
        scalar(object(colors, key), "color_name").map(str::to_owned).ok_or_else(|| format!("missing item description color {key}"))
    };
    let base = loc(scalar(&item.source, "item_name").ok_or("missing item name")?)?;
    let prefix = match registration.quality {
        0 => String::new(),
        6 if scalar(&item.source, "propername") == Some("1") => loc("TF_Unique_Prepend_Proper")?.to_owned(),
        6 => String::new(),
        5 => format!("{}{}", loc("rarity4")?, loc("Rarity_Spacer")?),
        other => return Err(format!("equipment quality description requires implementation: {other}")),
    };
    let quality = format_localized(loc(if matches!(registration.quality, 0 | 6) { "ItemNameNormalOrUniqueQualityFormat" } else { "ItemNameQualityFormat" })?, &[&prefix, "", "", "", ""]);
    let name = format_localized(loc("ItemNameFormat")?, &[&quality, base, "", "", "", ""]);
    let mut description = Vec::new();
    if let Some(token) = scalar(&item.source, "item_type_name") {
        let level = item.minimum_level.to_string();
        description.push((format_localized(loc("ItemTypeDesc")?, &[&level, loc(token)?]), color("desc_level")?));
    }
    let mut values = item.static_attributes.clone();
    for attribute in &registration.attributes {
        let value = schema::ItemAttribute { definition: attribute.definition, value: schema::ItemAttributeValue::Numeric(attribute.value) };
        if let Some(previous) = values.iter_mut().find(|old| old.definition == value.definition) { *previous = value; }
        else { values.push(value); }
    }
    let effect = |id: u32| -> (u8, &'static str) {
        match scalar(object(attributes, &id.to_string()), "effect_type").unwrap_or("neutral") {
            "positive" => (1, "desc_attrib_positive"), "negative" => (2, "desc_attrib_negative"),
            "strange" => (3, "desc_strange"), "unusual" => (4, "desc_unusual"), _ => (0, "desc_attrib_neutral"),
        }
    };
    values.sort_by_key(|attribute| (effect(attribute.definition).0, attribute.definition));
    for attribute in values {
        let definition = schema.attribute(attribute.definition).ok_or("missing description attribute")?;
        if definition.hidden { continue; }
        let Some(token) = scalar(object(attributes, &attribute.definition.to_string()), "description_string") else { continue; };
        let Ok(format) = loc(token) else { continue; }; // Source omits attributes with no localized description.
        let schema::ItemAttributeValue::Numeric(value) = attribute.value else { continue; };
        use schema::AttributeDescriptionFormat as Format;
        let numeric = |number: f32| if number.abs() <= f32::EPSILON || number.abs() >= 1.0 { format!("{number:.0}") } else { format!("{number:.1}") };
        let value = match definition.description_format {
            Format::Percentage => numeric(value * 100.0 - 100.0),
            Format::InvertedPercentage if value < 1.0 => numeric((1.0 - value) * 100.0),
            Format::InvertedPercentage => numeric(value * 100.0 - 100.0),
            Format::AdditivePercentage => numeric(value * 100.0),
            Format::Additive if definition.stored_as_integer => (value as u32).to_string(),
            Format::Additive => numeric(value),
            Format::ParticleIndex => loc(&format!("Attrib_Particle{}", value as u32))?.to_owned(),
            Format::Or => String::new(),
            other => return Err(format!("visible equipment attribute format requires implementation: {}:{other:?}", definition.index)),
        };
        description.push((format_localized(format, &[&value]), color(effect(attribute.definition).1)?));
    }
    if let Some(token) = scalar(&item.source, "item_description") && let Ok(text) = loc(token) {
        description.push((text.to_owned(), color("desc_attrib_neutral")?));
    }
    Ok((name, description))
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
    let (english_source, english_bytes) = super::dependency(content, "resource/tf_english.txt")?;
    let (_, _, _, _, english) = super::parse_summary("localization", "resource/tf_english.txt", &english_bytes.ok_or("missing configured English localization")?)?;
    let english = english.first().ok_or("missing English root")?.children.iter().find(|node| node.name.eq_ignore_ascii_case("tokens")).ok_or("missing English tokens")?;
    let localized: BTreeMap<_, _> = english.children.iter().filter_map(|node| node.value.as_ref().map(|value| (node.name.to_lowercase(), value.clone()))).collect();
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
    let colors = section("colors")?;
    let schema = ItemSchema::compose(SchemaInput { content_build: schema::CONTENT_BUILD, schema_sha256: schema::ITEM_SCHEMA_SHA256.into(),
        signature_sha256: schema::ITEM_SCHEMA_SIGNATURE_SHA256.into(), game_info: game_info.clone(), prefabs: prefabs.clone(), attributes: attributes.clone(), items: items.clone() })
        .map_err(|error| format!("equipment schema: {error:?}"))?;
    let mut output = format!("// Generated from configured items_game SHA-256 {}.\n// Regenerate with generate:tf2-ui.\npub const ITEM_PRESENTATIONS: &[ItemPresentation] = &[\n", schema::ITEM_SCHEMA_SHA256);
    writeln!(output, "// Configured English localization SHA-256 {}", english_source.sha256.unwrap()).unwrap();
    let mut generated = GeneratedEquipment { images: Vec::new(), tokens: Vec::new() };
    for supported in &supported_items {
        let item = schema.definition(supported.definition_index).ok_or("supported definition missing")?;
        let name = scalar(&item.source, "item_name").ok_or("item localization missing")?;
        let image = scalar(&item.source, "image_inventory").ok_or("item image missing")?;
        if supported.implemented {
            generated.images.push(format!("../{image}"));
            generated.tokens.push(name.into());
            if supported.attributes.iter().any(|attribute| attribute.definition == 134) { generated.images.push("viewmode_unusual".into()); }
        }
        let visuals = object(&item.source, "visuals");
        let (display_name, description) = display(item, supported, &schema, &attributes, &colors, &localized)?;
        writeln!(output, "ItemPresentation {{ definition_index: {}, name: {:?}, display_name: {:?}, description: &[{}], image: {:?}, model_player: {:?}, attach_to_hands: {}, animation_replacements: &[{}], sound_overrides: &[{}], death_notice_icon: {:?}, class_slots: &[{}] }},", item.index, name, display_name,
            description.iter().map(|(text, color)| format!("DescriptionLine {{ text: {text:?}, color: {color:?} }}")).collect::<Vec<_>>().join(", "), image,
            scalar(&item.source, "model_player").unwrap_or(""), scalar(&item.source, "attach_to_hands") == Some("1"), pairs(object(visuals, "animation_replacement"), ""), pairs(visuals, "sound_"),
            scalar(&item.source, "item_iconname"),
            item.class_slots.iter().map(|(class, slot)| format!("(PlayerClass::{class:?}, LoadoutPosition::{slot:?})")).collect::<Vec<_>>().join(", ")).unwrap();
    }
    output.push_str("];\npub const SUPPORTED_ITEMS: &[SupportedItem] = &[\n");
    for item in &supported_items {
        if !item.implemented { continue; }
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
