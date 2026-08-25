mod crosshair;

use playsrc_content::{Content, ProviderSpec, Resolution};
use playsrc_keyvalues::{ConditionEnvironment, EscapeMode, Node, ScalarKind, Value};
use playsrc_vmt::{Composition, DependencyResponse, EffectiveNode, EffectiveValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

const MAX_PROVIDERS: usize = 64;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalConfig {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContentBuildContract {
    schema: String,
    app_id: String,
    content_build: String,
    patch_version: String,
    gameinfo_sha256: String,
    custom_mod_providers: String,
    archive_indexes: ArchiveIndexContract,
    installed_depots: Vec<DepotContract>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArchiveIndexContract {
    tf2_misc: String,
    tf2_textures: String,
    tf2_sound_misc: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DepotContract {
    depot: String,
    manifest: String,
    byte_length: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRecord {
    order: usize,
    identity: String,
    kind: &'static str,
    revision: String,
    path_ids: Vec<String>,
    configured_location: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSummary {
    domain: &'static str,
    logical_path: &'static str,
    outcome: &'static str,
    byte_length: Option<usize>,
    sha256: Option<String>,
    provider_identity: Option<String>,
    provider_kind: Option<String>,
    provider_revision: Option<String>,
    encoding: Option<String>,
    roots: usize,
    nodes: usize,
    directives: Vec<String>,
    checked_locations: Vec<String>,
    document: Option<Vec<NodeRecord>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeRecord {
    name: String,
    value: Option<String>,
    scalar_kind: Option<String>,
    condition: Option<ConditionRecord>,
    children: Vec<NodeRecord>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConditionRecord {
    token: String,
    symbol: String,
    negated: bool,
    placement: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema: &'static str,
    content_build: String,
    source_ledger: String,
    source_ledger_sha256: String,
    providers: Vec<ProviderRecord>,
    resources: Vec<ResourceSummary>,
    unique_controls: Vec<String>,
    code_localization_tokens: Vec<String>,
    images: Vec<ImageRecord>,
    fonts: Vec<FontRecord>,
    advanced_options: Vec<AdvancedOptionRecord>,
    keyboard_actions: Vec<KeyboardActionRecord>,
}

const CODE_LOCALIZATION_TOKENS: &[&str] = &[
    "#GameUI_AdjustGamma_Title",
    "#GameUI_Audio",
    "#GameUI_Bilinear",
    "#GameUI_Keyboard",
    "#GameUI_KeyboardAdvanced_Title",
    "#GameUI_Mouse",
    "#GameUI_Multiplayer",
    "#GameUI_Options",
    "#GameUI_ThirdPartyAudio_Title",
    "#GameUI_ThirdPartyVideo_Title",
    "#GameUI_Video",
    "#GameUI_VideoAdvanced_Title",
    "#GameUI_Anisotropic2X",
    "#GameUI_Anisotropic4X",
    "#GameUI_Anisotropic8X",
    "#GameUI_Anisotropic16X",
    "#GameUI_Trilinear",
    "#GameUI_hdr_level0",
    "#GameUI_hdr_level1",
    "#GameUI_hdr_level2",
    "#PropertyDialog_Apply",
    "#PropertyDialog_Cancel",
    "#PropertyDialog_OK",
    "#gameui_disabled",
    "#gameui_enabled",
    "#gameui_high",
    "#gameui_low",
    "#gameui_medium",
    "#gameui_noreflections",
    "#gameui_reflectall",
    "#gameui_reflectonlyworld",
    "#gameui_ultra",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdvancedOptionRecord {
    identity: String,
    category: String,
    prompt: String,
    tooltip: Option<String>,
    kind: String,
    minimum: Option<f64>,
    maximum: Option<f64>,
    choices: Vec<AdvancedOptionChoice>,
    content_default: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdvancedOptionChoice {
    label: String,
    value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScriptToken {
    Text(String),
    Open,
    Close,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyboardActionRecord {
    section: usize,
    section_name: String,
    binding: String,
    description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyRecord {
    logical_path: String,
    outcome: &'static str,
    byte_length: Option<usize>,
    sha256: Option<String>,
    provider_identity: Option<String>,
    provider_kind: Option<String>,
    provider_revision: Option<String>,
    checked_locations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VtfRecord {
    source: DependencyRecord,
    version: String,
    width: u32,
    height: u32,
    depth: u32,
    frames: u16,
    faces: usize,
    mip_count: u8,
    high_format_code: i32,
    low_format_code: i32,
    raw_flags: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageRecord {
    identity: String,
    configured_value: String,
    classification: &'static str,
    material: Option<DependencyRecord>,
    textures: Vec<VtfRecord>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontRecord {
    identity: String,
    configured_value: String,
    classification: &'static str,
    source: Option<DependencyRecord>,
}

const ROOTS: &[(&str, &str, bool)] = &[
    ("main-menu", "resource/ui/mainmenuoverride.res", true),
    ("main-menu", "resource/gamemenu.res", true),
    ("main-menu", "resource/ui/matchmakingdashboard.res", true),
    (
        "main-menu",
        "resource/ui/matchmakingdashboardplaylist.res",
        true,
    ),
    ("main-menu", "resource/ui/matchmakingplaylist.res", true),
    ("main-menu", "resource/ui/mainmenuplaylistentry.res", true),
    (
        "main-menu",
        "resource/ui/mainmenueventplaylistentry.res",
        true,
    ),
    ("main-menu", "scripts/characterbackgrounds.txt", true),
    ("loading", "resource/loadingdialognobanner.res", true),
    ("loading", "resource/loadingdialognobannersingle.res", true),
    ("loading", "resource/loadingdialogvac.res", true),
    ("loading", "resource/loadingdialogdualprogress.res", true),
    ("loading", "resource/loadingdialogerror.res", true),
    ("loading", "resource/loadingdialogerrorvacbanned.res", true),
    (
        "loading",
        "resource/loadingdialogerrornosteamconnection.res",
        true,
    ),
    (
        "loading",
        "resource/loadingdialogerrorloggedinelsewhere.res",
        true,
    ),
    ("scheme", "resource/clientscheme.res", true),
    ("scheme", "resource/sourcescheme.res", true),
    ("scheme-base", "resource/sourceschemebase.res", true),
    ("hud", "scripts/hudlayout.res", true),
    (
        "animation-manifest",
        "scripts/hudanimations_manifest.txt",
        true,
    ),
    ("animation-script", "scripts/hudanimations.txt", false),
    ("animation-script", "scripts/hudanimations_tf.txt", false),
    ("hud", "resource/ui/hudplayerclass.res", true),
    ("hud", "resource/ui/hudplayerhealth.res", true),
    ("hud", "resource/ui/hudammoweapons.res", true),
    ("hud", "resource/ui/hudweaponselection.res", true),
    ("hud", "resource/ui/huddemomanpipes.res", true),
    ("hud", "resource/ui/hudkillstreaknotice.res", true),
    ("hud", "resource/ui/targetid.res", true),
    ("options", "resource/optionssubkeyboard.res", true),
    (
        "options",
        "resource/optionssubkeyboardadvanceddlg.res",
        true,
    ),
    ("options", "resource/optionssubmouse.res", true),
    ("options", "resource/optionssubaudio.res", true),
    ("options", "resource/optionssubaudiothirdpartydlg.res", true),
    ("options", "resource/optionssubvideo.res", true),
    ("options", "resource/optionssubvideoadvanceddlg.res", true),
    ("options", "resource/optionssubvideogammadlg.res", true),
    ("options", "resource/optionssubvideothirdpartydlg.res", true),
    ("options", "resource/optionssubvoice.res", true),
    ("options", "resource/optionssubmultiplayer.res", true),
    ("options", "resource/ui/tfadvancedoptionsdialog.res", true),
    ("options", "cfg/user_default.scr", false),
    ("options", "cfg/user.scr", false),
    ("options", "scripts/kb_act.lst", false),
    ("localization", "resource/tf_english.txt", true),
    ("localization", "resource/gameui_english.txt", true),
    ("localization", "resource/vgui_english.txt", true),
    ("localization", "resource/valve_english.txt", true),
];

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn script_tokens(bytes: &[u8]) -> Result<Vec<ScriptToken>, String> {
    let source = std::str::from_utf8(bytes).map_err(|_| "options script is not UTF-8")?;
    let mut tokens = Vec::new();
    let mut at = 0;
    let characters = source.as_bytes();
    while at < characters.len() {
        if characters[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if characters[at] == b'/' && characters.get(at + 1) == Some(&b'/') {
            at += 2;
            while at < characters.len() && characters[at] != b'\n' && characters[at] != b'\r' {
                at += 1;
            }
            continue;
        }
        if characters[at] == b'{' {
            tokens.push(ScriptToken::Open);
            at += 1;
            continue;
        }
        if characters[at] == b'}' {
            tokens.push(ScriptToken::Close);
            at += 1;
            continue;
        }
        if characters[at] == b'"' {
            at += 1;
            let mut value = String::new();
            let mut closed = false;
            while at < characters.len() {
                let byte = characters[at];
                at += 1;
                if byte == b'"' {
                    closed = true;
                    break;
                }
                if byte == b'\\' && at < characters.len() {
                    let escaped = characters[at];
                    at += 1;
                    value.push(match escaped {
                        b'n' => '\n',
                        b't' => '\t',
                        _ => char::from(escaped),
                    });
                } else {
                    value.push(char::from(byte));
                }
            }
            if !closed {
                return Err("options script quoted token is unterminated".to_owned());
            }
            tokens.push(ScriptToken::Text(value));
            continue;
        }
        let start = at;
        while at < characters.len()
            && !characters[at].is_ascii_whitespace()
            && characters[at] != b'{'
            && characters[at] != b'}'
        {
            at += 1;
        }
        tokens.push(ScriptToken::Text(source[start..at].to_owned()));
    }
    Ok(tokens)
}

fn advanced_options(bytes: &[u8]) -> Result<Vec<AdvancedOptionRecord>, String> {
    let tokens = script_tokens(bytes)?;
    let mut at = 0;
    let text = |at: &mut usize, subject: &str| -> Result<String, String> {
        let Some(ScriptToken::Text(value)) = tokens.get(*at) else {
            return Err(format!(
                "options script expected {subject} at token {}",
                *at
            ));
        };
        *at += 1;
        Ok(value.clone())
    };
    let exact = |at: &mut usize, expected: ScriptToken, subject: &str| -> Result<(), String> {
        if tokens.get(*at) != Some(&expected) {
            return Err(format!(
                "options script expected {subject} at token {}",
                *at
            ));
        }
        *at += 1;
        Ok(())
    };
    if !text(&mut at, "VERSION")?.eq_ignore_ascii_case("VERSION") {
        return Err("options script VERSION is missing".to_owned());
    }
    let version = text(&mut at, "version value")?;
    if version != "1.0" {
        return Err(format!("options script version changed: {version}"));
    }
    if !text(&mut at, "DESCRIPTION")?.eq_ignore_ascii_case("DESCRIPTION") {
        return Err("options script DESCRIPTION is missing".to_owned());
    }
    let _description = text(&mut at, "description identity")?;
    exact(&mut at, ScriptToken::Open, "description open brace")?;
    let mut rows = Vec::new();
    let mut category = String::new();
    while tokens.get(at) != Some(&ScriptToken::Close) {
        let identity = text(&mut at, "option identity")?;
        exact(&mut at, ScriptToken::Open, "option open brace")?;
        let prompt = text(&mut at, "option prompt")?;
        let tooltip = if tokens.get(at) == Some(&ScriptToken::Open) {
            None
        } else {
            Some(text(&mut at, "option tooltip")?)
        };
        exact(&mut at, ScriptToken::Open, "type open brace")?;
        let kind = text(&mut at, "option kind")?.to_ascii_uppercase();
        let mut arguments = Vec::new();
        while tokens.get(at) != Some(&ScriptToken::Close) {
            arguments.push(text(&mut at, "type argument")?);
        }
        exact(&mut at, ScriptToken::Close, "type close brace")?;
        let content_default = if tokens.get(at) == Some(&ScriptToken::Open) {
            exact(&mut at, ScriptToken::Open, "default open brace")?;
            let value = text(&mut at, "content default")?;
            exact(&mut at, ScriptToken::Close, "default close brace")?;
            value
        } else if kind == "CATEGORY" {
            String::new()
        } else {
            return Err(format!("advanced option {identity} default is missing"));
        };
        exact(&mut at, ScriptToken::Close, "option close brace")?;
        if kind == "CATEGORY" {
            category = prompt;
            continue;
        }
        if kind == "BUTTON" {
            continue;
        }
        let mut minimum = None;
        let mut maximum = None;
        let mut choices = Vec::new();
        if kind == "NUMBER" || kind == "SLIDER" {
            if arguments.len() != 2 {
                return Err(format!("advanced option {identity} numeric bounds differ"));
            }
            minimum = Some(
                arguments[0]
                    .parse::<f64>()
                    .map_err(|_| format!("advanced option {identity} minimum is malformed"))?,
            );
            maximum = Some(
                arguments[1]
                    .parse::<f64>()
                    .map_err(|_| format!("advanced option {identity} maximum is malformed"))?,
            );
        } else if kind == "LIST" {
            if arguments.len() % 2 != 0 || arguments.is_empty() {
                return Err(format!("advanced option {identity} choices differ"));
            }
            for pair in arguments.chunks_exact(2) {
                choices.push(AdvancedOptionChoice {
                    label: pair[0].clone(),
                    value: pair[1].clone(),
                });
            }
        } else if kind != "BOOL" && kind != "STRING" {
            return Err(format!(
                "advanced option {identity} kind {kind} is unsupported"
            ));
        }
        if category.is_empty() {
            return Err(format!("advanced option {identity} has no category"));
        }
        rows.push(AdvancedOptionRecord {
            identity,
            category: category.clone(),
            prompt,
            tooltip,
            kind,
            minimum,
            maximum,
            choices,
            content_default,
        });
    }
    exact(&mut at, ScriptToken::Close, "description close brace")?;
    if at != tokens.len() || rows.len() != 88 {
        return Err(format!("advanced option row count differs: {}", rows.len()));
    }
    Ok(rows)
}

fn keyboard_actions(bytes: &[u8]) -> Result<Vec<KeyboardActionRecord>, String> {
    let tokens = script_tokens(bytes)?;
    if tokens
        .iter()
        .any(|token| matches!(token, ScriptToken::Open | ScriptToken::Close))
    {
        return Err("keyboard action list contains structural braces".to_owned());
    }
    let values = tokens
        .into_iter()
        .map(|token| match token {
            ScriptToken::Text(value) => value,
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    if values.len() % 2 != 0 {
        return Err("keyboard action list has an incomplete pair".to_owned());
    }
    let mut section = 0;
    let mut section_name = String::new();
    let mut rows = Vec::new();
    for pair in values.chunks_exact(2) {
        let binding = &pair[0];
        let description = &pair[1];
        if description.starts_with('=') {
            continue;
        }
        if binding.eq_ignore_ascii_case("blank") {
            section += 1;
            section_name = description.clone();
            continue;
        }
        if section == 0 {
            return Err(format!("keyboard action {binding} precedes its section"));
        }
        rows.push(KeyboardActionRecord {
            section,
            section_name: section_name.clone(),
            binding: binding.clone(),
            description: description.clone(),
        });
    }
    if rows.len() != 70 {
        return Err(format!("keyboard action row count differs: {}", rows.len()));
    }
    Ok(rows)
}

fn scalar<'a>(node: &'a Node, key: &[u8]) -> Result<&'a [u8], String> {
    let child = node
        .first_child(key)
        .ok_or_else(|| format!("missing {}", String::from_utf8_lossy(key)))?;
    let Value::Scalar(value) = &child.value else {
        return Err(format!("{} is not scalar", String::from_utf8_lossy(key)));
    };
    Ok(&value.token.bytes)
}

fn object_children<'a>(node: &'a Node, key: &[u8]) -> Result<&'a [Node], String> {
    let child = node
        .first_child(key)
        .ok_or_else(|| format!("missing {}", String::from_utf8_lossy(key)))?;
    let Value::Object(value) = &child.value else {
        return Err(format!("{} is not an object", String::from_utf8_lossy(key)));
    };
    Ok(value)
}

fn app_manifest_path(install: &Path) -> PathBuf {
    let standard_steamapps = install
        .parent()
        .filter(|parent| {
            parent
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("common"))
        })
        .and_then(Path::parent);
    standard_steamapps.map_or_else(
        || install.join("steamapps/appmanifest_440.acf"),
        |steamapps| steamapps.join("appmanifest_440.acf"),
    )
}

fn verify_content_build(
    install: &Path,
    tf2: &Path,
    contract: &ContentBuildContract,
) -> Result<(), String> {
    if contract.schema != "playsrc-tf2-content-build-v1"
        || contract.app_id != "440"
        || contract.custom_mod_providers != "workshop-only"
        || contract.content_build.is_empty()
        || contract.patch_version.is_empty()
        || contract.gameinfo_sha256.len() != 64
        || contract.archive_indexes.tf2_misc.len() != 64
        || contract.archive_indexes.tf2_textures.len() != 64
        || contract.archive_indexes.tf2_sound_misc.len() != 64
        || contract.installed_depots.len() != 3
    {
        return Err("TF2 content-build contract is malformed".to_owned());
    }
    let manifest = app_manifest_path(install);
    let bytes = fs::read(&manifest).map_err(|error| {
        format!(
            "configured TF2 app manifest {}: {error}",
            manifest.display()
        )
    })?;
    let document = playsrc_keyvalues::parse_text(
        &bytes,
        EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| error.to_string())?
    .evaluated(&ConditionEnvironment::default());
    let app = document
        .roots
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(b"AppState"))
        .ok_or_else(|| "app manifest AppState is missing".to_owned())?;
    if scalar(app, b"appid")? != contract.app_id.as_bytes()
        || scalar(app, b"buildid")? != contract.content_build.as_bytes()
    {
        return Err("configured TF2 app or content build changed".to_owned());
    }
    let depots = object_children(app, b"InstalledDepots")?;
    let mut identities = BTreeSet::new();
    for expected in &contract.installed_depots {
        if !identities.insert(expected.depot.as_str()) {
            return Err("TF2 content-build contract contains duplicate depots".to_owned());
        }
        let node = depots
            .iter()
            .find(|node| node.key.bytes == expected.depot.as_bytes())
            .ok_or_else(|| format!("configured TF2 depot {} is missing", expected.depot))?;
        if scalar(node, b"manifest")? != expected.manifest.as_bytes()
            || scalar(node, b"size")? != expected.byte_length.as_bytes()
        {
            return Err(format!(
                "configured TF2 depot {} identity changed",
                expected.depot
            ));
        }
    }
    let patch = fs::read(tf2.join("steam.inf")).map_err(|error| error.to_string())?;
    for key in ["PatchVersion", "ClientVersion", "ServerVersion"] {
        let expected = format!("{key}={}", contract.patch_version);
        if !patch
            .split(|byte| *byte == b'\n' || *byte == b'\r')
            .any(|line| line == expected.as_bytes())
        {
            return Err(format!("configured TF2 {key} changed"));
        }
    }
    for (path, expected) in [
        ("tf2_misc_dir.vpk", &contract.archive_indexes.tf2_misc),
        (
            "tf2_textures_dir.vpk",
            &contract.archive_indexes.tf2_textures,
        ),
        (
            "tf2_sound_misc_dir.vpk",
            &contract.archive_indexes.tf2_sound_misc,
        ),
    ] {
        if digest(&fs::read(tf2.join(path)).map_err(|error| error.to_string())?) != *expected {
            return Err(format!("configured TF2 archive index {path} changed"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_install_manifest_is_inside_the_configured_install() {
        assert_eq!(
            app_manifest_path(Path::new("force-install")),
            PathBuf::from("force-install/steamapps/appmanifest_440.acf")
        );
    }

    #[test]
    fn standard_steam_manifest_is_beside_the_common_directory() {
        assert_eq!(
            app_manifest_path(Path::new("steamapps/common/Team Fortress 2")),
            PathBuf::from("steamapps/appmanifest_440.acf")
        );
    }
}

fn provider_id(order: usize, path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("provider")
        .to_ascii_lowercase()
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.') {
                value
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("ui-{order:02}-{name}")
}

fn configured_location(install: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(install)
        .map_err(|_| "provider escaped configured TF2 install".to_owned())?;
    Ok(format!(
        "tf2-install/{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

fn vpk_index_path(declared: &Path) -> PathBuf {
    let value = declared.to_string_lossy();
    if value.to_ascii_lowercase().ends_with("_dir.vpk") {
        return declared.to_path_buf();
    }
    let base = value
        .get(..value.len().saturating_sub(4))
        .unwrap_or(value.as_ref());
    PathBuf::from(format!("{base}_dir.vpk"))
}

fn wildcard_locations(pattern: &Path) -> Result<Vec<PathBuf>, String> {
    if pattern.file_name().and_then(|value| value.to_str()) != Some("*") {
        return Err("only a terminal configured wildcard is supported".to_owned());
    }
    let parent = pattern
        .parent()
        .ok_or_else(|| "configured wildcard has no parent".to_owned())?;
    let entries = fs::read_dir(parent)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut locations = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| "configured wildcard entry is not UTF-8".to_owned())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() || name.to_ascii_lowercase().ends_with(".vpk") {
            locations.push(entry.path());
        }
    }
    locations.sort_by(|left, right| {
        left.to_string_lossy()
            .to_ascii_lowercase()
            .cmp(&right.to_string_lossy().to_ascii_lowercase())
            .then_with(|| left.cmp(right))
    });
    if locations.len() > MAX_PROVIDERS {
        return Err("configured wildcard exceeds provider bound".to_owned());
    }
    if locations.windows(2).any(|pair| {
        pair[0]
            .to_string_lossy()
            .eq_ignore_ascii_case(&pair[1].to_string_lossy())
    }) {
        return Err("configured wildcard contains an ambiguous ASCII identity".to_owned());
    }
    let directory_families = locations
        .iter()
        .filter_map(|path| {
            path.file_name()?
                .to_str()?
                .to_ascii_lowercase()
                .strip_suffix("_dir.vpk")
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    locations.retain(|path| {
        let Some(value) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let lower = value.to_ascii_lowercase();
        let Some((family, suffix)) = lower.rsplit_once('_') else {
            return true;
        };
        !(suffix.len() == 7
            && suffix[..3].bytes().all(|byte| byte.is_ascii_digit())
            && suffix.ends_with(".vpk")
            && directory_families.contains(family))
    });
    Ok(locations)
}

fn provider_plan(
    install: &Path,
    tf2: &Path,
    gameinfo: &[u8],
    content_build: &str,
) -> Result<(Vec<ProviderSpec>, Vec<ProviderRecord>), String> {
    let gameinfo_revision = digest(gameinfo);
    let document = playsrc_keyvalues::parse_text(
        gameinfo,
        EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| error.to_string())?
    .evaluated(&ConditionEnvironment::default());
    let gameinfo = document
        .roots
        .iter()
        .find(|node| node.key.bytes.eq_ignore_ascii_case(b"GameInfo"))
        .ok_or_else(|| "configured GameInfo root is missing".to_owned())?;
    let filesystem = gameinfo
        .first_child(b"FileSystem")
        .ok_or_else(|| "configured FileSystem is missing".to_owned())?;
    if scalar(filesystem, b"SteamAppId")? != b"440" {
        return Err("configured SteamAppId differs".to_owned());
    }
    let search_paths = object_children(filesystem, b"SearchPaths")?;
    let mut specs = Vec::new();
    let mut records = Vec::new();
    for search_path in search_paths {
        let Value::Scalar(location) = &search_path.value else {
            return Err("configured search path is not scalar".to_owned());
        };
        let path_ids = String::from_utf8(search_path.key.bytes.clone())
            .map_err(|_| "configured path IDs are not UTF-8".to_owned())?
            .split('+')
            .map(|value| value.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        let custom_mod = path_ids.iter().any(|value| value == "custom_mod");
        if path_ids.iter().any(|value| value == "game_lv")
            || !path_ids
                .iter()
                .any(|value| matches!(value.as_str(), "game" | "vgui" | "platform" | "custom_mod"))
        {
            continue;
        }
        let declared = std::str::from_utf8(&location.token.bytes)
            .map_err(|_| "configured search path is not UTF-8".to_owned())?
            .replace('\\', "/");
        let resolved = if let Some(suffix) = declared.strip_prefix("|gameinfo_path|") {
            tf2.join(suffix)
        } else if let Some(suffix) = declared.strip_prefix("|all_source_engine_paths|") {
            install.join(suffix)
        } else {
            install.join(&declared)
        };
        let mut locations = if declared.contains('*') || declared.contains('?') {
            wildcard_locations(&resolved)?
        } else {
            vec![resolved]
        };
        if custom_mod {
            locations.retain(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("workshop"))
            });
        }
        for declared_path in locations {
            if specs.len() >= MAX_PROVIDERS {
                return Err("configured provider count exceeds bound".to_owned());
            }
            let lower = declared_path.to_string_lossy().to_ascii_lowercase();
            let (path, kind, layout) = if lower.ends_with(".vpk") {
                let path = if declared.contains('*') || declared.contains('?') {
                    declared_path
                } else {
                    vpk_index_path(&declared_path)
                };
                let layout = if path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.to_ascii_lowercase().ends_with("_dir.vpk"))
                {
                    playsrc_vpk::Layout::Split
                } else {
                    playsrc_vpk::Layout::Standalone
                };
                (path, "vpk", Some(layout))
            } else {
                (declared_path, "directory", None)
            };
            let order = specs.len();
            let identity = provider_id(order, &path);
            let revision = if kind == "vpk" {
                digest(&fs::read(&path).map_err(|error| error.to_string())?)
            } else {
                format!("{content_build}-{gameinfo_revision}-{order:02}")
            };
            records.push(ProviderRecord {
                order,
                identity: identity.clone(),
                kind,
                revision: revision.clone(),
                path_ids: path_ids.clone(),
                configured_location: configured_location(install, &path)?,
            });
            specs.push(if let Some(layout) = layout {
                ProviderSpec::Vpk {
                    id: identity,
                    revision,
                    directory_file: path,
                    layout,
                }
            } else {
                ProviderSpec::Directory {
                    id: identity,
                    revision,
                    root: path,
                }
            });
        }
    }
    Ok((specs, records))
}

#[derive(Default)]
struct Occurrences {
    nodes: usize,
    controls: BTreeSet<String>,
    localization_tokens: BTreeSet<String>,
    image_values: BTreeSet<String>,
    font_values: BTreeSet<String>,
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn exact_text(bytes: &[u8], context: &str) -> Result<String, String> {
    String::from_utf8(bytes.to_vec())
        .map_err(|_| format!("{context} is not UTF-8 after KeyValues decoding"))
}

fn node_record(node: &Node) -> Result<NodeRecord, String> {
    let condition = node
        .condition
        .as_ref()
        .map(|condition| -> Result<ConditionRecord, String> {
            Ok(ConditionRecord {
                token: exact_text(&condition.token.bytes, "condition token")?,
                symbol: exact_text(&condition.symbol, "condition symbol")?,
                negated: condition.negated,
                placement: format!("{:?}", condition.placement),
            })
        })
        .transpose()?;
    let (value, scalar_kind, children) = match &node.value {
        Value::Scalar(value) => {
            let kind = match value.kind {
                ScalarKind::Bytes => "bytes".to_owned(),
                ScalarKind::Integer(number) => format!("integer:{number}"),
                ScalarKind::Float(bits) => format!("float-bits:{bits:08x}"),
                ScalarKind::Uint64(number) => format!("uint64:{number}"),
            };
            (
                Some(exact_text(&value.token.bytes, "scalar token")?),
                Some(kind),
                Vec::new(),
            )
        }
        Value::Object(children) => (
            None,
            None,
            children
                .iter()
                .map(node_record)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    Ok(NodeRecord {
        name: exact_text(&node.key.bytes, "node name")?,
        value,
        scalar_kind,
        condition,
        children,
    })
}

fn is_image_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "image"
            | "image2"
            | "image_hidef"
            | "image_lodef"
            | "image_minmode"
            | "icon_texture"
            | "subimage"
    ) || key.starts_with("teambg_")
}

fn collect(nodes: &[Node], result: &mut Occurrences) {
    for node in nodes {
        result.nodes += 1;
        let key = text(&node.key.bytes);
        match &node.value {
            Value::Object(children) => collect(children, result),
            Value::Scalar(value) => {
                let value = text(&value.token.bytes);
                if key.eq_ignore_ascii_case("ControlName") {
                    result.controls.insert(value.clone());
                }
                if value.starts_with('#') && value.len() > 1 {
                    result.localization_tokens.insert(value.clone());
                }
                if is_image_key(&key) && !value.is_empty() {
                    result.image_values.insert(value.clone());
                }
                let lower_value = value.to_ascii_lowercase();
                if (key.to_ascii_lowercase().contains("font")
                    || lower_value.ends_with(".ttf")
                    || lower_value.ends_with(".otf")
                    || lower_value.ends_with(".vbf"))
                    && !value.is_empty()
                {
                    result.font_values.insert(value.clone());
                }
            }
        }
    }
}

type ParseSummary = (usize, String, Occurrences, Vec<String>, Vec<NodeRecord>);

fn parse_summary(
    domain: &'static str,
    logical_path: &'static str,
    bytes: &[u8],
) -> Result<ParseSummary, String> {
    if domain == "animation-script"
        || logical_path.ends_with(".scr")
        || logical_path.ends_with(".lst")
    {
        return Ok((
            0,
            "opaque-producer-input".to_owned(),
            Occurrences::default(),
            Vec::new(),
            Vec::new(),
        ));
    }
    let escape_mode = if logical_path == "resource/gamemenu.res" || domain == "localization" {
        EscapeMode::Escaped
    } else {
        EscapeMode::LiteralBackslash
    };
    let document =
        playsrc_keyvalues::parse_text(bytes, escape_mode, playsrc_keyvalues::Limits::default())
            .map_err(|error| error.to_string())?;
    let mut occurrences = Occurrences::default();
    collect(&document.roots, &mut occurrences);
    let directives = document
        .directives
        .iter()
        .map(|directive| {
            format!(
                "{:?}:{}",
                directive.kind,
                text(&directive.target.bytes).to_ascii_lowercase()
            )
        })
        .collect();
    Ok((
        document.roots.len(),
        format!("{:?}", document.encoding),
        occurrences,
        directives,
        document
            .roots
            .iter()
            .map(node_record)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn selected_localization_document(
    document: Vec<NodeRecord>,
    selected_tokens: &BTreeSet<String>,
) -> Vec<NodeRecord> {
    let selected = selected_tokens
        .iter()
        .map(|value| {
            value
                .strip_prefix('#')
                .unwrap_or(value)
                .to_ascii_lowercase()
        })
        .collect::<BTreeSet<_>>();
    document
        .into_iter()
        .map(|mut root| {
            root.children = root
                .children
                .into_iter()
                .filter_map(|mut child| {
                    if child.name.eq_ignore_ascii_case("Tokens") {
                        child
                            .children
                            .retain(|token| selected.contains(&token.name.to_ascii_lowercase()));
                        return Some(child);
                    }
                    child.name.eq_ignore_ascii_case("Language").then_some(child)
                })
                .collect();
            root
        })
        .collect()
}

fn normalize_segments(value: &str) -> Result<String, String> {
    let mut segments = Vec::new();
    for segment in value.replace('\\', "/").split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(format!("logical identity escapes its root: {value}"));
                }
            }
            value => segments.push(value.to_ascii_lowercase()),
        }
    }
    if segments.is_empty() {
        return Err("logical identity is empty".to_owned());
    }
    Ok(segments.join("/"))
}

fn material_logical_path(value: &str) -> Result<String, String> {
    let value = value
        .strip_suffix(".vmt")
        .or_else(|| value.strip_suffix(".VMT"))
        .unwrap_or(value);
    Ok(format!(
        "materials/{}.vmt",
        normalize_segments(&format!("vgui/{value}"))?
    ))
}

fn texture_logical_path(value: &str) -> Result<String, String> {
    let value = value
        .strip_prefix("materials/")
        .unwrap_or(value)
        .strip_suffix(".vtf")
        .or_else(|| {
            value
                .strip_prefix("materials/")
                .unwrap_or(value)
                .strip_suffix(".VTF")
        })
        .unwrap_or(value);
    Ok(format!("materials/{}.vtf", normalize_segments(value)?))
}

fn dependency(
    content: &Content,
    logical_path: &str,
) -> Result<(DependencyRecord, Option<Vec<u8>>), String> {
    match content
        .resolve_resource(logical_path)
        .map_err(|error| error.to_string())?
    {
        Resolution::Found(value) => Ok((
            DependencyRecord {
                logical_path: value.provenance.logical_path,
                outcome: "found",
                byte_length: Some(value.provenance.byte_length),
                sha256: Some(value.provenance.sha256),
                provider_identity: Some(value.provenance.provider_id),
                provider_kind: Some(format!("{:?}", value.provenance.provider_kind)),
                provider_revision: Some(value.provenance.provider_revision),
                checked_locations: Vec::new(),
            },
            Some(value.bytes),
        )),
        Resolution::Missing {
            logical_path,
            checked,
        } => Ok((
            DependencyRecord {
                logical_path,
                outcome: "missing",
                byte_length: None,
                sha256: None,
                provider_identity: None,
                provider_kind: None,
                provider_revision: None,
                checked_locations: checked
                    .into_iter()
                    .map(|value| format!("{}:{}", value.provider_id, value.location))
                    .collect(),
            },
            None,
        )),
    }
}

fn vmt_dependency_path(token: &[u8]) -> Result<String, String> {
    let value =
        std::str::from_utf8(token).map_err(|_| "VMT dependency token is not UTF-8".to_owned())?;
    let value = value
        .strip_prefix("materials/")
        .unwrap_or(value)
        .strip_suffix(".vmt")
        .or_else(|| {
            value
                .strip_prefix("materials/")
                .unwrap_or(value)
                .strip_suffix(".VMT")
        })
        .unwrap_or(value);
    Ok(format!("materials/{}.vmt", normalize_segments(value)?))
}

fn effective_textures(node: &EffectiveNode, output: &mut BTreeSet<String>) -> Result<(), String> {
    match &node.value {
        EffectiveValue::Object(children) => {
            for child in children {
                effective_textures(child, output)?;
            }
        }
        EffectiveValue::Scalar(value) => {
            let key = std::str::from_utf8(&node.key.bytes)
                .map_err(|_| "VMT key is not UTF-8".to_owned())?
                .to_ascii_lowercase();
            if matches!(
                key.as_str(),
                "$basetexture"
                    | "$basetexture2"
                    | "$bumpmap"
                    | "$detail"
                    | "$envmapmask"
                    | "$lightwarptexture"
                    | "$normalmap"
                    | "$phongexponenttexture"
                    | "$selfillummask"
                    | "$blendmodulatetexture"
            ) {
                let token = std::str::from_utf8(&value.token.bytes)
                    .map_err(|_| "VMT texture token is not UTF-8".to_owned())?;
                if !token.starts_with('$') && !token.eq_ignore_ascii_case("env_cubemap") {
                    output.insert(texture_logical_path(token)?);
                }
            }
        }
    }
    Ok(())
}

fn image_record(content: &Content, value: &str, index: usize) -> Result<ImageRecord, String> {
    if value.to_ascii_lowercase().ends_with(".pic") {
        return Ok(ImageRecord {
            identity: format!("image-{index:04}"),
            configured_value: value.to_owned(),
            classification: "unsupported-pic",
            material: None,
            textures: Vec::new(),
        });
    }
    let material_path = material_logical_path(value)?;
    let (material, material_bytes) = dependency(content, &material_path)?;
    let Some(material_bytes) = material_bytes else {
        return Ok(ImageRecord {
            identity: format!("image-{index:04}"),
            configured_value: value.to_owned(),
            classification: "missing-material",
            material: Some(material),
            textures: Vec::new(),
        });
    };
    let mut responses = Vec::new();
    let document = loop {
        match playsrc_vmt::compose(
            &material_bytes,
            material_path.clone(),
            &responses,
            &ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|error| format!("{material_path}: {error}"))?
        {
            Composition::Complete(document) => break document,
            Composition::Needs(requests) => {
                for request in requests {
                    let path = vmt_dependency_path(&request.target_token)?;
                    let (_, bytes) = dependency(content, &path)?;
                    responses.push(DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: path,
                        bytes,
                    });
                }
            }
        }
    };
    let mut texture_paths = BTreeSet::new();
    effective_textures(&document.root, &mut texture_paths)?;
    let mut textures = Vec::new();
    for texture_path in texture_paths {
        let (source, bytes) = dependency(content, &texture_path)?;
        let Some(bytes) = bytes else {
            return Ok(ImageRecord {
                identity: format!("image-{index:04}"),
                configured_value: value.to_owned(),
                classification: "missing-texture",
                material: Some(material),
                textures: vec![VtfRecord {
                    source,
                    version: "missing".to_owned(),
                    width: 0,
                    height: 0,
                    depth: 0,
                    frames: 0,
                    faces: 0,
                    mip_count: 0,
                    high_format_code: -1,
                    low_format_code: -1,
                    raw_flags: 0,
                }],
            });
        };
        let metadata = playsrc_vtf::inspect(
            &bytes,
            playsrc_vtf::Dialect::Source2013Pc,
            playsrc_vtf::Limits::default(),
        )
        .map_err(|error| format!("{texture_path}: {error}"))?;
        textures.push(VtfRecord {
            source,
            version: format!("{}.{}", metadata.version.0, metadata.version.1),
            width: metadata.width,
            height: metadata.height,
            depth: metadata.depth,
            frames: metadata.frame_count,
            faces: metadata.faces.len(),
            mip_count: metadata.mip_count,
            high_format_code: metadata.high_format.code(),
            low_format_code: metadata.low_format.code(),
            raw_flags: metadata.raw_flags,
        });
    }
    Ok(ImageRecord {
        identity: format!("image-{index:04}"),
        configured_value: value.to_owned(),
        classification: if textures.is_empty() {
            "procedural-material"
        } else {
            "content-vtf"
        },
        material: Some(material),
        textures,
    })
}

fn font_record(content: &Content, value: &str, index: usize) -> Result<FontRecord, String> {
    let lower = value.to_ascii_lowercase();
    let classification = if lower.ends_with(".ttf") || lower.ends_with(".otf") {
        "content-sfnt"
    } else if lower.ends_with(".vbf") {
        "content-bitmap"
    } else {
        return Ok(FontRecord {
            identity: format!("font-{index:03}"),
            configured_value: value.to_owned(),
            classification: "scheme-reference",
            source: None,
        });
    };
    let logical_path = normalize_segments(value)?;
    let (source, _) = dependency(content, &logical_path)?;
    Ok(FontRecord {
        identity: format!("font-{index:03}"),
        configured_value: value.to_owned(),
        classification: if source.outcome == "found" {
            classification
        } else {
            "missing-font"
        },
        source: Some(source),
    })
}

fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let class_images: Vec<String> = serde_json::from_str(
        &arguments
            .next()
            .ok_or_else(|| "TF2 HUD class image inventory is missing".to_owned())?,
    )
    .map_err(|error| format!("TF2 HUD class image inventory is malformed: {error}"))?;
    if arguments.next().is_some()
        || class_images.len() != 18
        || class_images
            .iter()
            .any(|image| !image.starts_with("../hud/class_") || image.len() > 128)
        || class_images.iter().collect::<BTreeSet<_>>().len() != class_images.len()
    {
        return Err("TF2 HUD class image inventory is invalid".to_owned());
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository = manifest
        .ancestors()
        .nth(6)
        .ok_or_else(|| "generator is not under the repository root".to_owned())?;
    let contract: ContentBuildContract = serde_json::from_slice(
        &fs::read(repository.join("games/tf2/content-build.json"))
            .map_err(|error| format!("games/tf2/content-build.json: {error}"))?,
    )
    .map_err(|error| format!("games/tf2/content-build.json: {error}"))?;
    let config_bytes = fs::read(repository.join("playsrc.local.json"))
        .map_err(|error| format!("playsrc.local.json: {error}"))?;
    let config: LocalConfig = serde_json::from_slice(&config_bytes)
        .map_err(|error| format!("playsrc.local.json: {error}"))?;
    if config.tf2_dir.is_empty()
        || config.source_cache_dir.is_empty()
        || config.asset_dir.is_empty()
    {
        return Err("playsrc.local.json contains an empty path".to_owned());
    }
    let mut configured_roots = [
        PathBuf::from(config.tf2_dir),
        PathBuf::from(config.source_cache_dir),
        PathBuf::from(config.asset_dir),
    ];
    for root in &mut configured_roots {
        if !root.is_absolute() {
            return Err("playsrc.local.json paths must be absolute".to_owned());
        }
        if !fs::metadata(&*root)
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            return Err("playsrc.local.json paths must identify directories".to_owned());
        }
        *root = fs::canonicalize(&*root).map_err(|error| error.to_string())?;
    }
    for left in 0..configured_roots.len() {
        for right in left + 1..configured_roots.len() {
            if configured_roots[left] == configured_roots[right]
                || configured_roots[left].starts_with(&configured_roots[right])
                || configured_roots[right].starts_with(&configured_roots[left])
            {
                return Err("playsrc.local.json paths must be distinct and non-nested".to_owned());
            }
        }
    }
    let tf2 = configured_roots[0].clone();
    let install = tf2
        .parent()
        .ok_or_else(|| "tf2Dir has no install parent".to_owned())?;
    verify_content_build(install, &tf2, &contract)?;
    let gameinfo = fs::read(tf2.join("gameinfo.txt")).map_err(|error| error.to_string())?;
    if digest(&gameinfo) != contract.gameinfo_sha256 {
        return Err("configured TF2 gameinfo identity changed".to_owned());
    }
    let (specs, providers) = provider_plan(install, &tf2, &gameinfo, &contract.content_build)?;
    let content = Content::open(
        "tf2",
        contract.content_build.clone(),
        specs,
        playsrc_content::Limits::default(),
    )
    .map_err(|error| error.to_string())?;

    let mut resources = Vec::new();
    let mut unique_controls = BTreeSet::new();
    let mut unique_localization_tokens = CODE_LOCALIZATION_TOKENS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    let mut unique_image_values = class_images.into_iter().collect::<BTreeSet<_>>();
    let mut unique_font_values = BTreeSet::new();
    let mut configured_advanced_options = None;
    let mut configured_keyboard_actions = None;
    for &(domain, logical_path, parse) in ROOTS {
        let resolution = content
            .resolve_resource(logical_path)
            .map_err(|error| error.to_string())?;
        let summary = match resolution {
            Resolution::Found(value) => {
                if logical_path == "cfg/user_default.scr" {
                    let rows = advanced_options(&value.bytes)?;
                    for token in rows.iter().flat_map(|row| {
                        std::iter::once(row.category.as_str())
                            .chain(std::iter::once(row.prompt.as_str()))
                            .chain(row.tooltip.as_deref())
                            .chain(row.choices.iter().map(|choice| choice.label.as_str()))
                    }) {
                        if token.starts_with('#') && token.len() > 1 {
                            unique_localization_tokens.insert(token.to_owned());
                        }
                    }
                    configured_advanced_options = Some(rows);
                }
                if logical_path == "scripts/kb_act.lst" {
                    let rows = keyboard_actions(&value.bytes)?;
                    for token in rows
                        .iter()
                        .flat_map(|row| [row.section_name.as_str(), row.description.as_str()])
                    {
                        if token.starts_with('#') && token.len() > 1 {
                            unique_localization_tokens.insert(token.to_owned());
                        }
                    }
                    configured_keyboard_actions = Some(rows);
                }
                let (roots, encoding, mut occurrences, directives, mut document) = if parse {
                    parse_summary(domain, logical_path, &value.bytes)
                        .map_err(|error| format!("{logical_path}: {error}"))?
                } else {
                    (
                        0,
                        "opaque-producer-input".to_owned(),
                        Occurrences::default(),
                        Vec::new(),
                        Vec::new(),
                    )
                };
                if domain == "localization" {
                    document =
                        selected_localization_document(document, &unique_localization_tokens);
                    occurrences = Occurrences::default();
                    for root in &document {
                        for tokens in root
                            .children
                            .iter()
                            .filter(|value| value.name.eq_ignore_ascii_case("Tokens"))
                        {
                            occurrences.nodes += tokens.children.len();
                        }
                    }
                } else {
                    unique_controls.extend(occurrences.controls.iter().cloned());
                    unique_localization_tokens
                        .extend(occurrences.localization_tokens.iter().cloned());
                    unique_image_values.extend(occurrences.image_values.iter().cloned());
                    unique_font_values.extend(occurrences.font_values.iter().cloned());
                }
                ResourceSummary {
                    domain,
                    logical_path,
                    outcome: "found",
                    byte_length: Some(value.provenance.byte_length),
                    sha256: Some(value.provenance.sha256),
                    provider_identity: Some(value.provenance.provider_id),
                    provider_kind: Some(format!("{:?}", value.provenance.provider_kind)),
                    provider_revision: Some(value.provenance.provider_revision),
                    encoding: Some(encoding),
                    roots,
                    nodes: occurrences.nodes,
                    directives,
                    checked_locations: Vec::new(),
                    document: (!document.is_empty()).then_some(document),
                }
            }
            Resolution::Missing { checked, .. } => ResourceSummary {
                domain,
                logical_path,
                outcome: "missing",
                byte_length: None,
                sha256: None,
                provider_identity: None,
                provider_kind: None,
                provider_revision: None,
                encoding: None,
                roots: 0,
                nodes: 0,
                directives: Vec::new(),
                checked_locations: checked
                    .into_iter()
                    .map(|value| format!("{}:{}", value.provider_id, value.location))
                    .collect(),
                document: None,
            },
        };
        resources.push(summary);
    }
    let images = unique_image_values
        .iter()
        .enumerate()
        .map(|(index, value)| image_record(&content, value, index + 1))
        .collect::<Result<Vec<_>, _>>()?;
    let fonts = unique_font_values
        .iter()
        .enumerate()
        .map(|(index, value)| font_record(&content, value, index + 1))
        .collect::<Result<Vec<_>, _>>()?;
    let source_ledger = resources
        .iter()
        .map(|resource| {
            format!(
                "{}={}",
                resource.logical_path,
                resource
                    .sha256
                    .as_deref()
                    .unwrap_or_else(|| if resource.outcome == "missing" {
                        "missing"
                    } else {
                        "invalid"
                    })
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let source_ledger_sha256 = digest(source_ledger.as_bytes());
    let report = Report {
        schema: "playsrc-tf2-ui-resources-v1",
        content_build: contract.content_build,
        source_ledger,
        source_ledger_sha256,
        providers,
        resources,
        unique_controls: unique_controls.into_iter().collect(),
        code_localization_tokens: CODE_LOCALIZATION_TOKENS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        images,
        fonts,
        advanced_options: configured_advanced_options
            .ok_or_else(|| "configured Advanced Options source is missing".to_owned())?,
        keyboard_actions: configured_keyboard_actions
            .ok_or_else(|| "configured keyboard action source is missing".to_owned())?,
    };
    let json = serde_json::to_string(&report)
        .map_err(|error| error.to_string())?
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    let generated = format!(
        "// Generated by generator/src/main.rs from exact configured producer outputs.\n// Do not edit.\nexport const configuredTf2UiResourceInput: unknown = {json}\n"
    );
    let output = manifest
        .parent()
        .ok_or_else(|| "generator has no ui-resources parent".to_owned())?
        .join("configured.generated.ts");
    fs::write(&output, generated.as_bytes()).map_err(|error| error.to_string())?;
    crosshair::write(
        &content,
        &tf2,
        &report.content_build,
        output
            .parent()
            .ok_or_else(|| "generator output has no parent".to_owned())?,
    )?;
    println!(
        "generated {} bytes sha256 {}",
        generated.len(),
        digest(generated.as_bytes())
    );
    Ok(())
}
