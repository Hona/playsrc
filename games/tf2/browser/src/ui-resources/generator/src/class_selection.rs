use playsrc_content::Content;
use std::{fmt::Write, fs, path::Path};

pub fn generate(content: &Content, repository: &Path) -> Result<(), String> {
    let (provenance, bytes) = super::dependency(content, "scenes/scenes.image")?;
    let bytes = bytes.ok_or_else(|| {
        format!(
            "class-select scene image missing: {:?}",
            provenance.checked_locations
        )
    })?;
    let (items_provenance, items_bytes) =
        super::dependency(content, "scripts/items/items_game.txt")?;
    let items_bytes = items_bytes.ok_or("configured item schema is missing")?;
    let (_, _, _, _, schema) = super::parse_summary(
        "class-selection",
        "scripts/items/items_game.txt",
        &items_bytes,
    )?;
    let root = schema
        .first()
        .ok_or("configured item schema root is missing")?;
    let items = root
        .children
        .iter()
        .find(|node| node.name == "items")
        .ok_or("configured item definitions are missing")?;
    let prefabs = root
        .children
        .iter()
        .find(|node| node.name == "prefabs")
        .ok_or("configured item prefabs are missing")?;
    let mut output = format!(
        "// Generated from configured scenes/scenes.image sha256 {}.\n// Regenerate with generate:tf2-ui.\npub static CLASS_SELECTION_SCENES: &[SceneDefinition] = &[\n",
        provenance.sha256.unwrap()
    );
    writeln!(
        output,
        "// Stock loadout schema sha256 {}",
        items_provenance.sha256.unwrap()
    )
    .unwrap();
    for (class, model, weapon) in [
        ("scout", "scout", "tf_weapon_scattergun"),
        ("sniper", "sniper", "tf_weapon_sniperrifle"),
        ("soldier", "soldier", "tf_weapon_rocketlauncher"),
        ("demoman", "demo", "tf_weapon_grenadelauncher"),
        ("medic", "medic", "tf_weapon_medigun"),
        ("heavy", "heavy", "tf_weapon_minigun"),
        ("pyro", "pyro", "tf_weapon_flamethrower"),
        ("spy", "spy", "tf_weapon_knife"),
        ("engineer", "engineer", "tf_weapon_wrench"),
    ] {
        let matching = items
            .children
            .iter()
            .filter(|item| {
                property(item, prefabs, "baseitem", 0).as_deref() == Some("1")
                    && property(item, prefabs, "item_class", 0).as_deref() == Some(weapon)
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(format!(
                "configured stock class-select item is ambiguous: {class}:{weapon}:{}",
                matching.len()
            ));
        }
        let held_model = property(matching[0], prefabs, "model_player", 0)
            .ok_or_else(|| format!("configured stock player model is missing: {weapon}"))?;
        let path = format!("scenes/player/{class}/low/class_select.vcd");
        let (_, model_bytes) = super::dependency(content, &format!("models/player/{model}.mdl"))?;
        let flex = playsrc_studio_model::read_model_flex(
            &model_bytes.ok_or("configured player model missing")?,
        )?;
        flex.weights(&std::collections::BTreeMap::new())?;
        let scene = playsrc_choreography::read_scene_image(&bytes, &path)?;
        if !scene.ramp.is_empty() {
            return Err(format!(
                "class-select scene ramp requires consumption: {path}"
            ));
        }
        writeln!(
            output,
            "SceneDefinition {{ model: {:?}, path: {:?}, held_model: {:?}, events: &[",
            format!("models/player/{model}.mdl"),
            path,
            held_model
        )
        .unwrap();
        for event in scene
            .events
            .iter()
            .filter(|event| event.active && (event.kind != 7 || event.actor))
        {
            if !matches!(event.kind, 2 | 7 | 12)
                || (event.kind == 12
                    && (event.loops != -1 || event.parameters[0].parse::<f32>().is_err()))
            {
                return Err(format!(
                    "class-select scene event requires consumption: {path}:{}",
                    event.kind
                ));
            }
            let expression = if event.kind == 2 {
                let path = format!(
                    "expressions/{}.vfe",
                    event.parameters[0].replace('\\', "/").to_lowercase()
                );
                let (source, bytes) = super::dependency(content, &path)?;
                let bytes = bytes
                    .ok_or_else(|| format!("configured class-select expression missing: {path}"))?;
                writeln!(output, "// {} sha256 {}", path, source.sha256.unwrap()).unwrap();
                playsrc_choreography::read_expression(&bytes, &event.parameters[1])?
            } else {
                Vec::new()
            };
            writeln!(output, "SceneEvent {{ kind: {}, start: {:?}, end: {:?}, parameters: {:?}, ramp: &{:?}, expression: &{:?} }},", event.kind, event.start, event.end, event.parameters, event.ramp, expression).unwrap();
        }
        output.push_str("] },\n");
    }
    output.push_str("];\n");
    fs::write(
        repository.join("games/tf2/rust/src/class_selection.generated.rs"),
        output,
    )
    .map_err(|e| e.to_string())
}

fn property(
    node: &super::NodeRecord,
    prefabs: &super::NodeRecord,
    name: &str,
    depth: usize,
) -> Option<String> {
    if depth > 16 {
        return None;
    }
    if let Some(value) = node
        .children
        .iter()
        .find(|n| n.name.eq_ignore_ascii_case(name))
        .and_then(|n| n.value.clone())
    {
        return Some(value);
    }
    let names = node
        .children
        .iter()
        .find(|n| n.name == "prefab")?
        .value
        .as_ref()?;
    for name_ in names.split_whitespace().rev() {
        let prefab = prefabs.children.iter().find(|n| n.name == name_)?;
        if let Some(value) = property(prefab, prefabs, name, depth + 1) {
            return Some(value);
        }
    }
    None
}
