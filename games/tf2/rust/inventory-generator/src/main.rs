use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    io::{Read, Seek, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
    process::Command,
};

use icefast::Ice;
use playsrc_keyvalues::{EscapeMode, Limits as KeyValuesLimits, Value};
use playsrc_tf2::{
    class::{CLASS_DATA, PlayerClass, PlayerTeam},
    condition::CONDITION_COUNT,
    schema::{
        CONTENT_BUILD, ITEM_SCHEMA_SHA256, ITEM_SCHEMA_SIGNATURE_SHA256, ItemSchema, Loadout,
        SchemaInput, SchemaNode, SchemaValue,
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const SDK_REVISION: &str = "88fa198fba3fb85d46d4c95018254693fdc3af0a";
const MISC_VPK_SHA256: &str = "63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9";

const STOCK_ITEM_DEFINITIONS: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 735,
];

const CORE_ATTRIBUTE_DEFINITIONS: &[u32] = &[
    15, 26, 28, 60, 61, 62, 63, 64, 65, 66, 67, 69, 70, 108, 109, 125, 138, 140, 179, 412, 479,
    491, 492, 503, 504, 505, 506, 507, 508, 516, 517, 526, 740, 794, 797, 852, 854, 869,
];

const CORE_ATTRIBUTE_HOOKS: &[&str] = &[
    "add_maxhealth",
    "add_maxhealth_nonbuffed",
    "mult_health_fromhealers",
    "mult_health_fromhealers_penalty_active",
    "mult_healing_received",
    "mult_health_frompacks",
    "mult_healing_from_medics",
    "overheal_fill_rate",
    "mult_dmg_vs_players",
    "mult_dmgtaken_from_crit",
    "mult_dmgtaken_from_fire",
    "mult_dmgtaken_from_fire_active",
    "mult_dmgtaken_from_explosions",
    "mult_dmgtaken_from_bullets",
    "mult_dmgtaken_from_melee",
    "mult_dmgtaken",
    "mult_dmgtaken_active",
    "mod_pierce_resists_absorbs",
    "minicrits_become_crits",
    "crits_become_minicrits",
    "mult_crit_chance",
];

const PICKUPS: &[(&str, &str)] = &[
    ("pickup.health.small", "item_healthkit_small"),
    ("pickup.health.medium", "item_healthkit_medium"),
    ("pickup.health.full", "item_healthkit_full"),
    ("pickup.ammo.small", "item_ammopack_small"),
    ("pickup.ammo.medium", "item_ammopack_medium"),
    ("pickup.ammo.full", "item_ammopack_full"),
    ("pickup.ammo.dropped", "tf_ammo_pack"),
    ("pickup.weapon.dropped", "tf_dropped_weapon"),
];

const STATE_FIELDS: &[(&str, &str)] = &[
    ("state.tick", "CoreState::tick"),
    ("state.content-build", "ItemSchema::content_build"),
    ("state.schema-hash", "ItemSchema::schema_sha256"),
    ("player.lifecycle", "TF_STATE_*"),
    ("player.team", "TF_TEAM_*"),
    ("player.class", "ETFClass"),
    ("player.desired-class", "m_iDesiredPlayerClass"),
    ("player.health.current", "m_iHealth"),
    ("player.health.maximum-buffable", "GetMaxHealthForBuffing"),
    ("player.health.maximum", "GetMaxHealth"),
    ("player.health.fraction", "m_flHealFraction"),
    ("player.health.healers", "m_aHealers"),
    ("player.health.overheal-decay", "m_flBestOverhealDecayMult"),
    ("player.health.last-damage-time", "m_flLastDamageTime"),
    ("player.ammo.primary", "TF_AMMO_PRIMARY"),
    ("player.ammo.secondary", "TF_AMMO_SECONDARY"),
    ("player.ammo.metal", "TF_AMMO_METAL"),
    ("player.ammo.grenades1", "TF_AMMO_GRENADES1"),
    ("player.ammo.grenades2", "TF_AMMO_GRENADES2"),
    ("player.loadout.class-slots", "loadout_positions_t"),
    (
        "player.loadout.account-slots",
        "account_loadout_positions_t",
    ),
    ("player.loadout.preset", "GetNumAllowedItemPresets"),
    ("player.conditions.words", "m_nPlayerCond{,Ex,Ex2,Ex3,Ex4}"),
    (
        "player.conditions.duration",
        "m_ConditionData.m_flExpireTime",
    ),
    ("player.conditions.provider", "m_ConditionData.m_pProvider"),
    (
        "player.attributes.providers",
        "CAttributeManager::m_Providers",
    ),
    (
        "player.attributes.cache",
        "CAttributeManager::m_CachedResults",
    ),
    ("player.crit.bucket", "m_flCritTokenBucket"),
    ("player.crit.history", "m_DamageEvents"),
    ("player.weapon.active-slot", "GetActiveTFWeapon"),
    ("pickup.lifecycle", "CItem::{Respawn,Materialize}"),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalConfiguration {
    tf2_dir: PathBuf,
    source_cache_dir: PathBuf,
    asset_dir: PathBuf,
}

struct FileSegments {
    directory: PathBuf,
    prefix: String,
}

impl playsrc_vpk::SegmentReader for FileSegments {
    fn len(&self, archive_index: u32) -> Result<u64, playsrc_vpk::SourceError> {
        fs::metadata(
            self.directory
                .join(format!("{}_{archive_index:03}.vpk", self.prefix)),
        )
        .map(|metadata| metadata.len())
        .map_err(|error| source_error(error, 0..0))
    }

    fn read(
        &self,
        archive_index: u32,
        range: Range<u64>,
    ) -> Result<Vec<u8>, playsrc_vpk::SourceError> {
        let mut file = fs::File::open(
            self.directory
                .join(format!("{}_{archive_index:03}.vpk", self.prefix)),
        )
        .map_err(|error| source_error(error, range.clone()))?;
        file.seek(SeekFrom::Start(range.start))
            .map_err(|error| source_error(error, range.clone()))?;
        let length = usize::try_from(range.end - range.start)
            .map_err(|_| source_error_kind(playsrc_vpk::SourceErrorCode::Io, range.clone()))?;
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes)
            .map_err(|error| source_error(error, range))?;
        Ok(bytes)
    }
}

#[derive(Clone)]
struct InventoryItem {
    identity: String,
    authority: String,
    disposition: String,
}

struct ClassRecords {
    hashes: BTreeMap<String, String>,
    summaries: BTreeMap<String, String>,
}

fn main() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .ok_or("inventory generator is not under games/tf2/rust")?;
    let configuration: LocalConfiguration = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if !configuration.asset_dir.is_absolute()
        || !configuration.tf2_dir.is_absolute()
        || !configuration.source_cache_dir.is_absolute()
    {
        return Err("playsrc.local.json paths must be absolute".into());
    }

    let sdk = configuration
        .source_cache_dir
        .join("evidence/tf2-core-gameplay-source-sdk-2013");
    validate_sdk(&sdk)?;
    let item_bytes = fs::read(configuration.tf2_dir.join("scripts/items/items_game.txt"))
        .map_err(|error| error.to_string())?;
    let signature_bytes = fs::read(
        configuration
            .tf2_dir
            .join("scripts/items/items_game.txt.sig"),
    )
    .map_err(|error| error.to_string())?;
    require_hash(
        "scripts/items/items_game.txt",
        &item_bytes,
        ITEM_SCHEMA_SHA256,
    )?;
    require_hash(
        "scripts/items/items_game.txt.sig",
        &signature_bytes,
        ITEM_SCHEMA_SIGNATURE_SHA256,
    )?;
    require_hash(
        "tf2_misc_dir.vpk",
        &fs::read(configuration.tf2_dir.join("tf2_misc_dir.vpk"))
            .map_err(|error| error.to_string())?,
        MISC_VPK_SHA256,
    )?;

    let document =
        playsrc_keyvalues::parse_text(&item_bytes, EscapeMode::Escaped, KeyValuesLimits::default())
            .map_err(|error| error.to_string())?;
    let root_node = document.roots.first().ok_or("items_game.txt has no root")?;
    if root_node.key.bytes != b"items_game" {
        return Err("items_game.txt root is not items_game".into());
    }
    let Value::Object(root_children) = &root_node.value else {
        return Err("items_game root is not an object".into());
    };
    let game_info = section(root_children, b"game_info")?;
    let prefabs = section(root_children, b"prefabs")?;
    let attributes = section(root_children, b"attributes")?;
    let items = section(root_children, b"items")?;

    let item_lookup = lookup(items)?;
    let prefab_lookup = lookup(prefabs)?;
    let attribute_lookup = lookup(attributes)?;
    let mut selected_items = vec![convert_node(
        item_lookup
            .get("default")
            .ok_or("missing selected default item definition")?,
    )?];
    selected_items.extend(select_numeric(
        &item_lookup,
        STOCK_ITEM_DEFINITIONS,
        "item",
    )?);
    let selected_prefab_names = prefab_closure(&selected_items, prefabs, &prefab_lookup)?;
    let selected_prefabs = selected_prefab_names
        .iter()
        .map(|name| convert_node(prefab_lookup.get(name).expect("closure validated")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut referenced_attribute_names = BTreeSet::new();
    for node in selected_items.iter().chain(selected_prefabs.iter()) {
        collect_attribute_names(node, &mut referenced_attribute_names);
    }
    let core_attribute_ids: BTreeSet<_> = CORE_ATTRIBUTE_DEFINITIONS.iter().copied().collect();
    let mut selected_attributes = Vec::new();
    for node in attributes {
        let converted = convert_node(node)?;
        let index = converted
            .key
            .parse::<u32>()
            .map_err(|_| format!("attribute identity {} is not numeric", converted.key))?;
        let SchemaValue::Object(fields) = &converted.value else {
            return Err(format!("attribute {index} is not an object"));
        };
        if core_attribute_ids.contains(&index)
            || scalar(fields, "name").is_some_and(|name| referenced_attribute_names.contains(name))
        {
            selected_attributes.push(converted);
        }
    }
    let selected_schema = ItemSchema::compose(SchemaInput {
        content_build: CONTENT_BUILD,
        schema_sha256: ITEM_SCHEMA_SHA256.into(),
        signature_sha256: ITEM_SCHEMA_SIGNATURE_SHA256.into(),
        game_info: game_info
            .iter()
            .map(convert_node)
            .collect::<Result<Vec<_>, _>>()?,
        prefabs: selected_prefabs,
        attributes: selected_attributes,
        items: selected_items.clone(),
    })
    .map_err(|error| format!("selected schema composition failed: {error:?}"))?;
    if selected_schema.definitions_in_source_order() != STOCK_ITEM_DEFINITIONS {
        return Err("selected stock item order changed".into());
    }
    for class in PlayerClass::ALL {
        for team in [PlayerTeam::Red, PlayerTeam::Blue] {
            let loadout = Loadout::stock(&selected_schema, class, team)
                .map_err(|error| format!("stock {team:?} {class:?} loadout failed: {error:?}"))?;
            if loadout.class_slots().iter().flatten().count() != class.data().stock_items.len() {
                return Err(format!("stock {team:?} {class:?} loadout count changed"));
            }
        }
    }

    let conditions = sdk_conditions(&sdk)?;
    let ClassRecords {
        hashes: class_hashes,
        summaries: class_summaries,
    } = class_record_hashes(&configuration.tf2_dir)?;
    validate_class_hashes(&class_hashes)?;
    validate_sdk_hooks(&sdk)?;
    validate_pickup_symbols(&sdk)?;

    let mut inventory = Vec::new();
    for class in PlayerClass::ALL {
        let data = class.data();
        inventory.push(InventoryItem {
            identity: data.identity.into(),
            authority: format!(
                "ETFClass={} / scripts/playerclasses/{}.ctx {} / {} / stock={}",
                class.source_number(),
                data.class_record_name,
                class_hashes[data.source_name],
                class_summaries[data.source_name],
                data.stock_items
                    .iter()
                    .map(|item| format!("{}@{}", item.definition, item.slot))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            disposition: "Handled: class record and stock spawn state".into(),
        });
    }
    for position in 0..19 {
        inventory.push(InventoryItem {
            identity: format!("loadout.class.{position}"),
            authority: "loadout_positions_t".into(),
            disposition: "Handled: class loadout slot".into(),
        });
    }
    for position in 0..3 {
        inventory.push(InventoryItem {
            identity: format!("loadout.account.{position}"),
            authority: "account_loadout_positions_t".into(),
            disposition: "Handled: account loadout slot".into(),
        });
    }
    for definition in STOCK_ITEM_DEFINITIONS {
        let item = selected_schema
            .definition(*definition)
            .ok_or_else(|| format!("selected item {definition} was not composed"))?;
        inventory.push(InventoryItem {
            identity: format!("item-definition:{definition}"),
            authority: format!("items:{definition} / {} / {}", item.name, item.item_class),
            disposition: "Handled: stock item instance and loadout eligibility".into(),
        });
    }
    inventory.push(InventoryItem {
        identity: "item-definition.default".into(),
        authority: "items:default".into(),
        disposition: "Handled: immutable selected schema default definition".into(),
    });
    for name in &selected_prefab_names {
        inventory.push(InventoryItem {
            identity: format!("prefab:{name}"),
            authority: format!("prefabs:{name}"),
            disposition: "Handled: stock definition recursive prefab closure".into(),
        });
    }
    for definition in CORE_ATTRIBUTE_DEFINITIONS {
        let node = attribute_lookup
            .get(&definition.to_string())
            .ok_or_else(|| format!("missing selected attribute definition {definition}"))?;
        let converted = convert_node(node)?;
        let SchemaValue::Object(fields) = converted.value else {
            return Err(format!("attribute {definition} is not an object"));
        };
        let name =
            scalar(&fields, "name").ok_or_else(|| format!("attribute {definition} has no name"))?;
        let class = scalar(&fields, "attribute_class")
            .ok_or_else(|| format!("attribute {definition} has no class"))?;
        inventory.push(InventoryItem {
            identity: format!("attribute-definition:{definition}"),
            authority: format!("attributes:{definition} / {name} / {class}"),
            disposition: "Handled: bounded core attribute definition".into(),
        });
    }
    for definition in selected_schema.attributes_in_source_order() {
        if core_attribute_ids.contains(definition) {
            continue;
        }
        let attribute = selected_schema
            .attribute(*definition)
            .ok_or_else(|| format!("selected closure attribute {definition} was not composed"))?;
        inventory.push(InventoryItem {
            identity: format!("schema-closure-attribute-definition:{definition}"),
            authority: format!(
                "attributes:{definition} / {} / {}",
                attribute.name, attribute.class
            ),
            disposition: "Handled: retained stock prefab/item attribute; core advancement inert"
                .into(),
        });
    }
    for hook in CORE_ATTRIBUTE_HOOKS {
        inventory.push(InventoryItem {
            identity: format!("attribute-hook:{hook}"),
            authority: "official SDK CALL_ATTRIB_HOOK site".into(),
            disposition: "Handled: bounded core provider query".into(),
        });
    }
    for (value, symbol) in conditions.iter().enumerate() {
        inventory.push(InventoryItem {
            identity: format!("condition:{value}"),
            authority: symbol.clone(),
            disposition: if core_condition(value as u8) {
                "Handled: generic lifecycle and core health/damage semantics".into()
            } else {
                "Handled: generic lifecycle; specialized effect remains downstream".into()
            },
        });
    }
    for (identity, symbol) in PICKUPS {
        inventory.push(InventoryItem {
            identity: (*identity).into(),
            authority: (*symbol).into(),
            disposition: "Handled: ordinary pickup lifecycle and grant".into(),
        });
    }
    for (identity, symbol) in STATE_FIELDS {
        inventory.push(InventoryItem {
            identity: (*identity).into(),
            authority: (*symbol).into(),
            disposition: "Handled: canonical core transition/snapshot field".into(),
        });
    }

    let identities = inventory
        .iter()
        .map(|item| item.identity.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let identity_hash = sha256(identities.as_bytes());
    let output = render(
        &inventory,
        &selected_prefab_names,
        &identity_hash,
        &class_hashes,
    )?;
    let output_path = root.join("games/tf2/inventories/core-state.md");
    let temporary_path = output_path.with_extension("md.tmp");
    fs::write(&temporary_path, output).map_err(|error| error.to_string())?;
    fs::rename(&temporary_path, &output_path).map_err(|error| error.to_string())?;
    println!(
        "generated {} items at games/tf2/inventories/core-state.md ({identity_hash})",
        inventory.len()
    );
    Ok(())
}

fn validate_sdk(sdk: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(sdk)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "configured source-cache SDK checkout is unavailable at {}",
            sdk.display()
        ));
    }
    let revision = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_owned();
    if revision != SDK_REVISION {
        return Err(format!(
            "SDK revision {revision} does not match {SDK_REVISION}"
        ));
    }
    let class_enum = fs::read_to_string(sdk.join("src/game/shared/tf/tf_shareddefs.h"))
        .map_err(|error| error.to_string())?;
    for symbol in [
        "TF_CLASS_SCOUT",
        "TF_CLASS_SNIPER",
        "TF_CLASS_SOLDIER",
        "TF_CLASS_DEMOMAN",
        "TF_CLASS_MEDIC",
        "TF_CLASS_HEAVYWEAPONS",
        "TF_CLASS_PYRO",
        "TF_CLASS_SPY",
        "TF_CLASS_ENGINEER",
    ] {
        if !class_enum.contains(symbol) {
            return Err(format!("SDK class enum is missing {symbol}"));
        }
    }
    Ok(())
}

fn sdk_conditions(sdk: &Path) -> Result<Vec<String>, String> {
    let source = fs::read_to_string(sdk.join("src/game/shared/tf/tf_shareddefs.h"))
        .map_err(|error| error.to_string())?;
    let enum_source = source
        .split("enum ETFCond")
        .nth(1)
        .and_then(|value| value.split("TF_COND_LAST").next())
        .ok_or("SDK ETFCond declaration is missing")?;
    let mut output = Vec::new();
    for line in enum_source.lines() {
        let line = line.trim();
        if !line.starts_with("TF_COND_") || line.starts_with("TF_COND_INVALID") {
            continue;
        }
        let symbol = line
            .split(|character: char| {
                character == ',' || character == '=' || character.is_whitespace()
            })
            .next()
            .expect("non-empty condition line");
        output.push(symbol.to_owned());
    }
    if output.len() != CONDITION_COUNT {
        return Err(format!(
            "SDK condition count {} does not match {CONDITION_COUNT}",
            output.len()
        ));
    }
    Ok(output)
}

fn validate_sdk_hooks(sdk: &Path) -> Result<(), String> {
    let paths = [
        "src/game/shared/econ/attribute_manager.cpp",
        "src/game/shared/tf/tf_gamerules.cpp",
        "src/game/shared/tf/tf_player_shared.cpp",
        "src/game/shared/tf/tf_weaponbase.cpp",
        "src/game/server/tf/entity_ammopack.cpp",
        "src/game/server/tf/entity_healthkit.cpp",
        "src/game/server/tf/tf_player.cpp",
    ];
    let mut source = String::new();
    for path in paths {
        source.push_str(&fs::read_to_string(sdk.join(path)).map_err(|error| error.to_string())?);
    }
    for hook in CORE_ATTRIBUTE_HOOKS {
        if !source.contains(hook) {
            return Err(format!("selected SDK attribute hook {hook} is missing"));
        }
    }
    Ok(())
}

fn validate_pickup_symbols(sdk: &Path) -> Result<(), String> {
    let mut source = String::new();
    for path in [
        "src/game/server/tf/entity_ammopack.cpp",
        "src/game/server/tf/entity_healthkit.cpp",
        "src/game/server/tf/tf_ammo_pack.cpp",
        "src/game/shared/tf/tf_dropped_weapon.cpp",
    ] {
        source.push_str(&fs::read_to_string(sdk.join(path)).map_err(|error| error.to_string())?);
    }
    for (_, symbol) in PICKUPS {
        if !source.contains(symbol) {
            return Err(format!("selected SDK pickup symbol {symbol} is missing"));
        }
    }
    Ok(())
}

fn class_record_hashes(tf2_dir: &Path) -> Result<ClassRecords, String> {
    let directory_bytes =
        fs::read(tf2_dir.join("tf2_misc_dir.vpk")).map_err(|error| error.to_string())?;
    let archive = playsrc_vpk::parse(
        &directory_bytes,
        "tf2_misc_dir.vpk",
        playsrc_vpk::Layout::Split,
        playsrc_vpk::Limits::default(),
    )
    .map_err(|error| error.to_string())?;
    let segments = FileSegments {
        directory: tf2_dir.to_owned(),
        prefix: "tf2_misc".into(),
    };
    let mut output = BTreeMap::new();
    let mut summaries = BTreeMap::new();
    for class in PlayerClass::ALL {
        let name = class.data().source_name;
        let record_name = class.data().class_record_name;
        let candidates = archive
            .entries
            .iter()
            .filter(|entry| {
                entry
                    .directory
                    .eq_ignore_ascii_case(b"scripts/playerclasses")
                    && entry.basename.eq_ignore_ascii_case(record_name.as_bytes())
            })
            .map(|entry| entry.logical_path.clone())
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(format!(
                "expected one configured class record for {name}, found {candidates:?}"
            ));
        }
        let path = &candidates[0];
        let bytes = archive
            .read_entry(path, &segments)
            .map_err(|error| error.to_string())?
            .bytes;
        output.insert(name.into(), sha256(&bytes));
        summaries.insert(name.into(), validate_class_record(class, bytes)?);
    }
    Ok(ClassRecords {
        hashes: output,
        summaries,
    })
}

fn validate_class_record(class: PlayerClass, mut bytes: Vec<u8>) -> Result<String, String> {
    let block_bytes = bytes.len() / 8 * 8;
    if block_bytes > 0 {
        Ice::new(0, b"E2NcUkG2").decrypt(&mut bytes[..block_bytes]);
    }
    let document =
        playsrc_keyvalues::parse_text(&bytes, EscapeMode::Escaped, KeyValuesLimits::default())
            .map_err(|error| format!("{} class record: {error}", class.data().source_name))?;
    let root = document
        .roots
        .first()
        .ok_or_else(|| format!("{} class record has no root", class.data().source_name))?;
    let Value::Object(fields) = &root.value else {
        return Err(format!(
            "{} class record root is not an object",
            class.data().source_name
        ));
    };
    let number = |key: &[u8]| -> Result<f32, String> {
        kv_scalar(fields, key)?.parse::<f32>().map_err(|_| {
            format!(
                "{} class record has malformed {}",
                class.data().source_name,
                String::from_utf8_lossy(key)
            )
        })
    };
    let data = class.data();
    if number(b"speed_max")? != data.maximum_speed
        || number(b"health_max")? as i32 != data.maximum_health
    {
        return Err(format!("{} class speed/health changed", data.source_name));
    }
    let ammo = section(fields, b"AmmoMax")?;
    for (key, expected) in [
        (b"TF_AMMO_PRIMARY".as_slice(), data.maximum_ammo.primary),
        (b"TF_AMMO_SECONDARY".as_slice(), data.maximum_ammo.secondary),
        (b"TF_AMMO_METAL".as_slice(), data.maximum_ammo.metal),
        (b"TF_AMMO_GRENADES1".as_slice(), data.maximum_ammo.grenades1),
        (b"TF_AMMO_GRENADES2".as_slice(), data.maximum_ammo.grenades2),
    ] {
        let actual = kv_scalar(ammo, key)?
            .parse::<u16>()
            .map_err(|_| format!("{} class ammo is malformed", data.source_name))?;
        if actual != expected {
            return Err(format!(
                "{} class {} changed from {} to {}",
                data.source_name,
                String::from_utf8_lossy(key),
                expected,
                actual
            ));
        }
    }
    let model = kv_scalar(fields, b"model")?;
    let hwm = kv_scalar(fields, b"model_hwm")?;
    let hands = kv_scalar(fields, b"model_hands")?;
    let localized = kv_scalar(fields, b"localize_name")?;
    let armor = kv_scalar(fields, b"armor_max")?;
    let weapons = (1..=6)
        .map(|index| kv_scalar_or(fields, format!("weapon{index}").as_bytes(), ""))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let grenades = (1..=2)
        .map(|index| kv_scalar_or(fields, format!("grenade{index}").as_bytes(), ""))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let buildables = (1..=6)
        .map(|index| kv_scalar_or(fields, format!("buildable{index}").as_bytes(), ""))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let animation_flags = format!(
        "{}/{}",
        kv_scalar_or(fields, b"DontDoAirwalk", "0")?,
        kv_scalar_or(fields, b"DontDoNewJump", "0")?
    );
    let death_sounds = [
        kv_scalar_or(fields, b"sound_death", "Player.Death")?,
        kv_scalar_or(fields, b"sound_crit_death", "TFPlayer.CritDeath")?,
        kv_scalar_or(fields, b"sound_melee_death", "Player.MeleeDeath")?,
        kv_scalar_or(fields, b"sound_explosion_death", "Player.ExplosionDeath")?,
    ]
    .join(",");
    let camera = [
        kv_scalar(fields, b"cameraoffset_forward")?,
        kv_scalar(fields, b"cameraoffset_right")?,
        kv_scalar(fields, b"cameraoffset_up")?,
    ]
    .join(",");
    if model != data.model
        || hwm != data.hwm_model
        || hands != data.hand_model
        || localized != data.localization
        || armor.parse::<i32>().ok() != Some(data.maximum_armor)
        || weapons
            .split(',')
            .filter(|weapon| !weapon.is_empty())
            .ne(data.weapon_declarations.iter().copied())
        || grenades.split(',').ne(data.grenade_declarations)
        || animation_flags
            != format!(
                "{}/{}",
                u8::from(data.disable_airwalk),
                u8::from(data.disable_new_jump)
            )
        || camera
            != data
                .third_person_camera
                .iter()
                .map(|value| format!("{value}"))
                .collect::<Vec<_>>()
                .join(",")
        || death_sounds.split(',').ne(data.death_sounds)
    {
        return Err(format!(
            "{} class resource/state fields changed",
            data.source_name
        ));
    }
    Ok(format!(
        "speed={} health={} armor={} ammo={}/{}/{}/{}/{} model={} hwm={} hands={} localize={} weapons={} grenades={} buildables={} animation-flags={} camera={} death-sounds={}",
        data.maximum_speed,
        data.maximum_health,
        armor,
        data.maximum_ammo.primary,
        data.maximum_ammo.secondary,
        data.maximum_ammo.metal,
        data.maximum_ammo.grenades1,
        data.maximum_ammo.grenades2,
        model,
        hwm,
        hands,
        localized,
        weapons,
        grenades,
        buildables,
        animation_flags,
        camera,
        death_sounds
    ))
}

fn kv_scalar<'a>(nodes: &'a [playsrc_keyvalues::Node], key: &[u8]) -> Result<&'a str, String> {
    let node = nodes
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(key))
        .ok_or_else(|| format!("missing class field {}", String::from_utf8_lossy(key)))?;
    let Value::Scalar(value) = &node.value else {
        return Err(format!(
            "class field {} is not scalar",
            String::from_utf8_lossy(key)
        ));
    };
    std::str::from_utf8(&value.token.bytes).map_err(|error| error.to_string())
}

fn kv_scalar_or<'a>(
    nodes: &'a [playsrc_keyvalues::Node],
    key: &[u8],
    default: &'a str,
) -> Result<&'a str, String> {
    let Some(node) = nodes
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(key))
    else {
        return Ok(default);
    };
    let Value::Scalar(value) = &node.value else {
        return Err(format!(
            "class field {} is not scalar",
            String::from_utf8_lossy(key)
        ));
    };
    std::str::from_utf8(&value.token.bytes).map_err(|error| error.to_string())
}

fn validate_class_hashes(hashes: &BTreeMap<String, String>) -> Result<(), String> {
    for data in CLASS_DATA {
        let actual = hashes
            .get(data.source_name)
            .ok_or_else(|| format!("missing class record {}", data.source_name))?;
        if actual != data.content_sha256 {
            return Err(format!(
                "class record {} hash {} does not match {}",
                data.source_name, actual, data.content_sha256
            ));
        }
    }
    Ok(())
}

fn section<'a>(
    nodes: &'a [playsrc_keyvalues::Node],
    key: &[u8],
) -> Result<&'a [playsrc_keyvalues::Node], String> {
    let node = nodes
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(key))
        .ok_or_else(|| format!("missing {} section", String::from_utf8_lossy(key)))?;
    match &node.value {
        Value::Object(children) => Ok(children),
        Value::Scalar(_) => Err(format!(
            "{} section is not an object",
            String::from_utf8_lossy(key)
        )),
    }
}

fn lookup(
    nodes: &[playsrc_keyvalues::Node],
) -> Result<BTreeMap<String, playsrc_keyvalues::Node>, String> {
    let mut output = BTreeMap::new();
    for node in nodes {
        let key = String::from_utf8(node.key.bytes.clone()).map_err(|error| error.to_string())?;
        if output.insert(key.clone(), node.clone()).is_some() {
            return Err(format!("duplicate direct schema key {key}"));
        }
    }
    Ok(output)
}

fn select_numeric(
    lookup: &BTreeMap<String, playsrc_keyvalues::Node>,
    selected: &[u32],
    category: &str,
) -> Result<Vec<SchemaNode>, String> {
    selected
        .iter()
        .map(|identity| {
            lookup
                .get(&identity.to_string())
                .ok_or_else(|| format!("missing selected {category} {identity}"))
                .and_then(convert_node)
        })
        .collect()
}

fn prefab_closure(
    items: &[SchemaNode],
    prefab_nodes: &[playsrc_keyvalues::Node],
    prefabs: &BTreeMap<String, playsrc_keyvalues::Node>,
) -> Result<Vec<String>, String> {
    let mut selected = BTreeSet::new();
    let mut pending = Vec::new();
    for item in items {
        let SchemaValue::Object(fields) = &item.value else {
            continue;
        };
        if let Some(value) = scalar(fields, "prefab") {
            pending.extend(value.split_ascii_whitespace().map(str::to_owned));
        }
    }
    while let Some(name) = pending.pop() {
        if !selected.insert(name.clone()) {
            continue;
        }
        let prefab = prefabs
            .get(&name)
            .ok_or_else(|| format!("missing selected prefab {name}"))?;
        let converted = convert_node(prefab)?;
        let SchemaValue::Object(fields) = converted.value else {
            return Err(format!("prefab {name} is not an object"));
        };
        if let Some(value) = scalar(&fields, "prefab") {
            pending.extend(value.split_ascii_whitespace().map(str::to_owned));
        }
    }
    let mut source_order = Vec::new();
    for node in prefab_nodes {
        let name = String::from_utf8(node.key.bytes.clone()).map_err(|error| error.to_string())?;
        if selected.contains(&name) {
            source_order.push(name);
        }
    }
    Ok(source_order)
}

fn convert_node(node: &playsrc_keyvalues::Node) -> Result<SchemaNode, String> {
    let key = String::from_utf8(node.key.bytes.clone()).map_err(|error| error.to_string())?;
    let value = match &node.value {
        Value::Scalar(value) => SchemaValue::Scalar(
            String::from_utf8(value.token.bytes.clone()).map_err(|error| error.to_string())?,
        ),
        Value::Object(children) => SchemaValue::Object(
            children
                .iter()
                .map(convert_node)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    Ok(SchemaNode { key, value })
}

fn scalar<'a>(nodes: &'a [SchemaNode], key: &str) -> Option<&'a str> {
    nodes.iter().find_map(|node| {
        if node.key != key {
            return None;
        }
        match &node.value {
            SchemaValue::Scalar(value) => Some(value.as_str()),
            SchemaValue::Object(_) => None,
        }
    })
}

fn collect_attribute_names(node: &SchemaNode, output: &mut BTreeSet<String>) {
    let SchemaValue::Object(children) = &node.value else {
        return;
    };
    if node.key == "attributes" {
        output.extend(children.iter().map(|child| child.key.clone()));
    }
    for child in children {
        collect_attribute_names(child, output);
    }
}

fn core_condition(value: u8) -> bool {
    matches!(
        value,
        5 | 11
            | 14
            | 16
            | 19
            | 21
            | 22
            | 23
            | 24
            | 25
            | 26
            | 27
            | 30
            | 31
            | 33
            | 34
            | 37
            | 38
            | 39
            | 40
            | 42
            | 45
            | 48
            | 51
            | 52
            | 56
            | 57
            | 58
            | 59
            | 60
            | 61
            | 62
            | 63
            | 67
            | 68
            | 69
            | 70
            | 77
            | 105
            | 106
            | 112
            | 118
            | 119
            | 123
    )
}

fn render(
    inventory: &[InventoryItem],
    prefab_names: &[String],
    identity_hash: &str,
    class_hashes: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut output = String::new();
    writeln!(output, "# TF2 Core State Inventory\n").map_err(|error| error.to_string())?;
    writeln!(output, "Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)\n")
        .map_err(|error| error.to_string())?;
    writeln!(output, "Authority identity: Valve Source SDK 2013 class, loadout, economy schema, attribute manager, player condition, health/healing, damage/death, health-kit, ammo-pack and dropped-weapon contracts; configured TF2 class records and `scripts/items/items_game.txt`.\n").map_err(|error| error.to_string())?;
    writeln!(output, "Authority revision: SDK `{SDK_REVISION}`; TF2 content build `{CONTENT_BUILD}`; `scripts/items/items_game.txt` SHA-256 `{ITEM_SCHEMA_SHA256}`; signature SHA-256 `{ITEM_SCHEMA_SIGNATURE_SHA256}`; `tf2_misc_dir.vpk` SHA-256 `{MISC_VPK_SHA256}`.\n").map_err(|error| error.to_string())?;
    writeln!(output, "Generator command: `cargo run --locked --manifest-path games/tf2/rust/inventory-generator/Cargo.toml`\n").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Output path: `games/tf2/inventories/core-state.md`\n"
    )
    .map_err(|error| error.to_string())?;
    writeln!(output, "Item count: {}\n", inventory.len()).map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Ordered stable-identity SHA-256: `{identity_hash}`\n"
    )
    .map_err(|error| error.to_string())?;
    writeln!(output, "Generation state: generated bounded inventory. This output enumerates only the selected core family; broader candidate records remain visible and unaccepted in the nine owning inventories.\n").map_err(|error| error.to_string())?;
    writeln!(output, "Provider order: item runtime attributes override equal-definition static attributes; each query applies local attributes in retained order, providers in provider-vector order, then the owner. Provider-equals-initiator and weapon-provider-to-weapon-initiator edges are suppressed. Item-list queries bypass cache; non-item-list cache identity is exact hook plus input value.\n").map_err(|error| error.to_string())?;
    writeln!(output, "Configured bounds: 64 players, 1,024 pickups, 4,096 commands per transition, 8,192 attribute entities, 4,096 attributes and 256 providers per attribute entity, 1,024 cache records per attribute entity, 64 healers, 128 damage-history entries, 19 class slots, three account slots, five condition words, 30-second dropped-pickup lifetime, three active dropped ammo packs per owner, and the SDK-observable 33 active dropped-weapon steady-state maximum produced by the pre-creation 32-item cleanup calculation.\n").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Selected stock prefab closure contains {} records: `{}`.\n",
        prefab_names.len(),
        prefab_names.join("`, `")
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Configured class-record hashes: {}.\n",
        PlayerClass::ALL
            .into_iter()
            .map(|class| {
                let name = class.data().source_name;
                format!("`{name}` `{}`", class_hashes[name])
            })
            .collect::<Vec<_>>()
            .join("; ")
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "| Stable identity | Authority record | Coverage classification |"
    )
    .map_err(|error| error.to_string())?;
    writeln!(output, "|---|---|---|").map_err(|error| error.to_string())?;
    for item in inventory {
        writeln!(
            output,
            "| `{}` | `{}` | {} |",
            item.identity,
            item.authority.replace('|', "\\|"),
            item.disposition
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(output)
}

fn require_hash(identity: &str, bytes: &[u8], expected: &str) -> Result<(), String> {
    let actual = sha256(bytes);
    if actual != expected {
        return Err(format!(
            "{identity} SHA-256 {actual} does not match {expected}"
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn source_error(error: std::io::Error, range: Range<u64>) -> playsrc_vpk::SourceError {
    let code = match error.kind() {
        std::io::ErrorKind::NotFound => playsrc_vpk::SourceErrorCode::Missing,
        std::io::ErrorKind::UnexpectedEof => playsrc_vpk::SourceErrorCode::ShortRead,
        _ => playsrc_vpk::SourceErrorCode::Io,
    };
    source_error_kind(code, range)
}

fn source_error_kind(
    code: playsrc_vpk::SourceErrorCode,
    range: Range<u64>,
) -> playsrc_vpk::SourceError {
    playsrc_vpk::SourceError { code, range }
}
