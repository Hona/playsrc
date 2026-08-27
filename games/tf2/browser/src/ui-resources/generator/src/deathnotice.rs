use base64::{Engine as _, engine::general_purpose::STANDARD};
use playsrc_content::Content;
use playsrc_keyvalues::{EscapeMode, Value};
use serde::Serialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Atlas {
    material: String,
    material_sha256: String,
    texture: String,
    texture_sha256: String,
    width: u32,
    height: u32,
    png_sha256: String,
    png_data_url: String,
}

#[derive(Serialize)]
struct Icon {
    atlas: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

// CHud::Init loads hud_textures followed by mod_textures. Keep the authored
// atlas rectangles, including their transparent margins; do not trim artwork.
pub(super) fn write(content: &Content, build: &str, directory: &Path) -> Result<(), String> {
    let mut sources = BTreeMap::new();
    let mut icons = BTreeMap::new();
    let mut atlases = BTreeMap::new();
    for path in ["scripts/hud_textures.txt", "scripts/mod_textures.txt"] {
        let (source, bytes) = crate::dependency(content, path)?;
        let bytes = bytes.ok_or_else(|| format!("missing death notice source {path}"))?;
        sources.insert(path, source.sha256);
        let document = playsrc_keyvalues::parse_text(&bytes, EscapeMode::LiteralBackslash,
            playsrc_keyvalues::Limits::default()).map_err(|error| error.to_string())?;
        let data = document.roots.iter().find_map(|root| root.first_child(b"TextureData"))
            .ok_or_else(|| format!("{path} has no TextureData"))?;
        let mut refs = vec![("file".to_owned(), String::new())];
        if let Some(node) = document.roots.iter().find_map(|root| root.first_child(b"TextureFileRefs")) {
            if let Value::Object(children) = &node.value {
                for reference in children {
                    refs.push((String::from_utf8_lossy(&reference.key.bytes).into_owned(),
                        String::from_utf8_lossy(crate::scalar(reference, b"prefix")?).into_owned()));
                }
            }
        }
        let Value::Object(children) = &data.value else { return Err("TextureData is not an object".into()); };
        for node in children {
          for (key, prefix) in &refs {
            let name = format!("{prefix}{}", String::from_utf8_lossy(&node.key.bytes));
            if !name.starts_with("d_") && !name.starts_with("dneg_") && name != "leaderboard_dominated" { continue; }
            let Ok(file) = crate::scalar(node, key.as_bytes()) else { continue; };
            let file = String::from_utf8_lossy(file).into_owned();
            let number = |key: &[u8]| -> Result<u32, String> {
                String::from_utf8_lossy(crate::scalar(node, key)?).parse().map_err(|_| format!("invalid rectangle {name}"))
            };
            let icon = Icon { atlas: file.clone(), x: number(b"x")?, y: number(b"y")?,
                width: number(b"width")?, height: number(b"height")? };
            if !atlases.contains_key(&file) {
                let material = crate::image_record(content, &format!("../{file}"), 0, false)?;
                let texture = material.textures.first().ok_or_else(|| format!("{file} has no texture"))?;
                let (_, bytes) = crate::dependency(content, &texture.source.logical_path)?;
                let bytes = bytes.ok_or_else(|| format!("{file} texture missing"))?;
                let plane = playsrc_vtf::decode(&bytes, playsrc_vtf::Dialect::Source2013Pc,
                    playsrc_vtf::SubresourceIdentity::HighResolution { mip: 0, frame: 0, face: playsrc_vtf::Face::Right, slice: 0 },
                    playsrc_vtf::Limits::default()).map_err(|error| error.to_string())?;
                if plane.channel_layout != playsrc_vtf::ChannelLayout::Rgba || plane.scalar_encoding != playsrc_vtf::ScalarEncoding::U8 {
                    return Err(format!("{file} is not RGBA8"));
                }
                let png = super::crosshair::png_data(plane.width, plane.height, &plane.samples)?;
                let material_source = material.material.as_ref().ok_or("missing material")?;
                atlases.insert(file.clone(), Atlas {
                    material: material_source.logical_path.clone(),
                    material_sha256: material_source.sha256.clone().ok_or("missing material digest")?,
                    texture: texture.source.logical_path.clone(),
                    texture_sha256: texture.source.sha256.clone().ok_or("missing texture digest")?,
                    width: plane.width, height: plane.height,
                    png_sha256: crate::digest(&png), png_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
                });
            }
            let atlas = &atlases[&file];
            if icon.width == 0 || icon.height == 0 || icon.x + icon.width > atlas.width || icon.y + icon.height > atlas.height {
                return Err(format!("{name} exceeds its atlas"));
            }
            icons.insert(name, icon);
          }
        }
    }
    let json = serde_json::to_string(&serde_json::json!({ "contentBuild": build, "sources": sources, "atlases": atlases, "icons": icons }))
        .map_err(|error| error.to_string())?;
    fs::write(directory.join("deathnotice.generated.ts"), format!("// Generated from configured HUD texture scripts and VTF atlases. Do not edit.\nexport const tf2DeathNoticeAssets = {json} as const\n"))
        .map_err(|error| error.to_string())
}
