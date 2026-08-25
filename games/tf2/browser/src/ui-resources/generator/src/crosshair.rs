use base64::{Engine as _, engine::general_purpose::STANDARD};
use playsrc_content::{Content, Resolution};
use playsrc_keyvalues::{EscapeMode, Node, Value};
use serde::Serialize;
use std::{collections::BTreeSet, fs, path::Path};

use crate::{digest, image_record, scalar};

const MAX_STYLES: usize = 128;
const MAX_FRAMES: usize = 64;
const MAX_DIMENSION: u32 = 512;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceIdentity {
    logical_path: String,
    byte_length: usize,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredFrame {
    index: u16,
    png_sha256: String,
    png_data_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredCrosshair {
    file: String,
    material: SourceIdentity,
    texture: SourceIdentity,
    texture_width: u32,
    texture_height: u32,
    crop: Option<Crop>,
    frames: Vec<AuthoredFrame>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponCrosshair {
    weapon_identities: Vec<u8>,
    source: SourceIdentity,
    crosshair: AuthoredCrosshair,
    autoaim: Option<AuthoredCrosshair>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopeTexture {
    source: SourceIdentity,
    width: u32,
    height: u32,
    frame: AuthoredFrame,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredScope {
    schema: &'static str,
    content_build: String,
    quadrants: Vec<SourceIdentity>,
    charge_material: SourceIdentity,
    tint: ScopeTexture,
    normal: ScopeTexture,
    charge_base: ScopeTexture,
    charge_mask: ScopeTexture,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredCrosshairs {
    schema: &'static str,
    content_build: String,
    icon_source: SourceIdentity,
    stock: AuthoredCrosshair,
    weapons: Vec<WeaponCrosshair>,
    styles: Vec<AuthoredCrosshair>,
}

fn identity(record: &crate::DependencyRecord) -> Result<SourceIdentity, String> {
    Ok(SourceIdentity {
        logical_path: record.logical_path.clone(),
        byte_length: record
            .byte_length
            .ok_or_else(|| format!("crosshair source {} has no length", record.logical_path))?,
        sha256: record
            .sha256
            .clone()
            .ok_or_else(|| format!("crosshair source {} has no digest", record.logical_path))?,
    })
}

fn child<'a>(node: &'a Node, name: &[u8]) -> Result<&'a Node, String> {
    node.first_child(name).ok_or_else(|| {
        format!(
            "authored HUD icon is missing {}",
            String::from_utf8_lossy(name)
        )
    })
}

fn coordinate(node: &Node, name: &[u8]) -> Result<u32, String> {
    std::str::from_utf8(scalar(node, name)?)
        .map_err(|_| "authored HUD icon coordinate is not UTF-8".to_owned())?
        .parse()
        .map_err(|_| "authored HUD icon coordinate is not an unsigned integer".to_owned())
}

fn png_data(width: u32, height: u32, samples: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(samples)
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

fn frame(bytes: &[u8], frame_index: u16, crop: Option<&Crop>) -> Result<AuthoredFrame, String> {
    let plane = playsrc_vtf::decode(
        bytes,
        playsrc_vtf::Dialect::Source2013Pc,
        playsrc_vtf::SubresourceIdentity::HighResolution {
            mip: 0,
            frame: frame_index,
            face: playsrc_vtf::Face::Right,
            slice: 0,
        },
        playsrc_vtf::Limits::default(),
    )
    .map_err(|error| format!("authored crosshair frame {frame_index}: {error}"))?;
    if plane.scalar_encoding != playsrc_vtf::ScalarEncoding::U8
        || !matches!(
            plane.channel_layout,
            playsrc_vtf::ChannelLayout::Rgba | playsrc_vtf::ChannelLayout::Rgb
        )
    {
        return Err(format!(
            "authored frame uses unsupported channels: {:?}/{:?}",
            plane.channel_layout, plane.scalar_encoding
        ));
    }
    let rgb = plane.channel_layout == playsrc_vtf::ChannelLayout::Rgb;
    let rgba_samples = if rgb {
        plane
            .samples
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect::<Vec<_>>()
    } else {
        plane.samples.clone()
    };
    let row_stride = if rgb {
        plane.width as usize * 4
    } else {
        plane.row_stride
    };
    let (width, height, samples) = if let Some(region) = crop {
        if region.width == 0
            || region.height == 0
            || region
                .x
                .checked_add(region.width)
                .is_none_or(|right| right > plane.width)
            || region
                .y
                .checked_add(region.height)
                .is_none_or(|bottom| bottom > plane.height)
        {
            return Err("authored crosshair source crop exceeds its texture".to_owned());
        }
        let row = usize::try_from(region.width)
            .map_err(|_| "crosshair crop width is invalid".to_owned())?
            .checked_mul(4)
            .ok_or_else(|| "crosshair crop row overflows".to_owned())?;
        let mut cropped = Vec::with_capacity(
            row.checked_mul(region.height as usize)
                .ok_or_else(|| "crosshair crop allocation overflows".to_owned())?,
        );
        for index in 0..region.height {
            let start = (region.y + index) as usize * row_stride + region.x as usize * 4;
            cropped.extend_from_slice(&rgba_samples[start..start + row]);
        }
        (region.width, region.height, cropped)
    } else {
        (plane.width, plane.height, rgba_samples)
    };
    let png = png_data(width, height, &samples)?;
    Ok(AuthoredFrame {
        index: frame_index,
        png_sha256: digest(&png),
        png_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
    })
}

fn authored(
    content: &Content,
    file: &str,
    configured_image: &str,
    crop: Option<Crop>,
) -> Result<AuthoredCrosshair, String> {
    let image = image_record(content, configured_image, 1)?;
    if image.classification != "content-vtf" || image.textures.len() != 1 {
        return Err(format!(
            "authored crosshair {file} does not resolve one exact material and texture"
        ));
    }
    let material = identity(
        image
            .material
            .as_ref()
            .ok_or_else(|| format!("authored crosshair {file} has no material"))?,
    )?;
    let texture_record = &image.textures[0];
    if texture_record.width == 0
        || texture_record.height == 0
        || texture_record.width > MAX_DIMENSION
        || texture_record.height > MAX_DIMENSION
        || texture_record.frames == 0
        || usize::from(texture_record.frames) > MAX_FRAMES
    {
        return Err(format!(
            "authored crosshair {file} exceeds its texture bounds"
        ));
    }
    let (_, bytes) = crate::dependency(content, &texture_record.source.logical_path)?;
    let bytes = bytes.ok_or_else(|| format!("authored crosshair {file} texture disappeared"))?;
    let frames = (0..texture_record.frames)
        .map(|index| frame(&bytes, index, crop.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AuthoredCrosshair {
        file: file.to_owned(),
        material,
        texture: identity(&texture_record.source)?,
        texture_width: texture_record.width,
        texture_height: texture_record.height,
        crop,
        frames,
    })
}

fn icon(content: &Content, node: &Node, name: &str) -> Result<AuthoredCrosshair, String> {
    let atlas = std::str::from_utf8(scalar(node, b"file")?)
        .map_err(|_| "authored crosshair atlas is not UTF-8".to_owned())?;
    authored(
        content,
        name,
        &format!("../{}", atlas.trim_start_matches('/')),
        Some(Crop {
            x: coordinate(node, b"x")?,
            y: coordinate(node, b"y")?,
            width: coordinate(node, b"width")?,
            height: coordinate(node, b"height")?,
        }),
    )
}

fn weapon(
    content: &Content,
    logical_path: &str,
    identities: &[u8],
) -> Result<WeaponCrosshair, String> {
    let mut source = match content
        .resolve_resource(logical_path)
        .map_err(|error| error.to_string())?
    {
        Resolution::Found(value) => value,
        Resolution::Missing { checked, .. } => {
            return Err(format!(
                "authored weapon crosshair {logical_path} is missing from {}",
                checked
                    .iter()
                    .map(|location| format!("{}:{}", location.provider_id, location.location))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    };
    let identity = SourceIdentity {
        logical_path: source.provenance.logical_path,
        byte_length: source.provenance.byte_length,
        sha256: source.provenance.sha256,
    };
    let blocks = source.bytes.len() / 8 * 8;
    if blocks > 0 {
        icefast::Ice::new(0, b"E2NcUkG2").decrypt(&mut source.bytes[..blocks]);
    }
    let document = playsrc_keyvalues::parse_text(
        &source.bytes,
        EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| format!("{logical_path}: {error}"))?;
    let textures = document
        .roots
        .iter()
        .find_map(|root| child(root, b"TextureData").ok())
        .ok_or_else(|| format!("{logical_path} has no authored weapon textures"))?;
    let crosshair = icon(content, child(textures, b"crosshair")?, "crosshair")?;
    let autoaim = child(textures, b"autoaim")
        .ok()
        .map(|value| icon(content, value, "autoaim"))
        .transpose()?;
    Ok(WeaponCrosshair {
        weapon_identities: identities.to_vec(),
        source: identity,
        crosshair,
        autoaim,
    })
}

fn scope_source(
    content: &Content,
    logical_path: &str,
) -> Result<(SourceIdentity, Vec<u8>), String> {
    let (record, bytes) = crate::dependency(content, logical_path)?;
    Ok((
        identity(&record)?,
        bytes.ok_or_else(|| format!("authored scope source is absent: {logical_path}"))?,
    ))
}

fn scope_texture(content: &Content, logical_path: &str) -> Result<ScopeTexture, String> {
    let (source, bytes) = scope_source(content, logical_path)?;
    let metadata = playsrc_vtf::inspect(
        &bytes,
        playsrc_vtf::Dialect::Source2013Pc,
        playsrc_vtf::Limits::default(),
    )
    .map_err(|error| format!("{logical_path}: {error}"))?;
    Ok(ScopeTexture {
        source,
        width: u32::from(metadata.width),
        height: u32::from(metadata.height),
        frame: frame(&bytes, 0, None).map_err(|error| format!("{logical_path}: {error}"))?,
    })
}

pub(crate) fn write(
    content: &Content,
    tf2: &Path,
    content_build: &str,
    output_directory: &Path,
) -> Result<(), String> {
    let icon_resource = match content
        .resolve_resource("scripts/mod_textures.txt")
        .map_err(|error| error.to_string())?
    {
        Resolution::Found(value) => value,
        Resolution::Missing { checked, .. } => {
            return Err(format!(
                "authored HUD icons are missing from configured providers: {}",
                checked
                    .iter()
                    .map(|location| format!("{}:{}", location.provider_id, location.location))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    };
    let document = playsrc_keyvalues::parse_text(
        &icon_resource.bytes,
        EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|error| format!("scripts/mod_textures.txt: {error}"))?;
    let default_icon = document
        .roots
        .iter()
        .find_map(|root| child(root, b"TextureData").ok())
        .and_then(|textures| child(textures, b"crosshair_default").ok())
        .ok_or_else(|| "authored HUD icon crosshair_default is absent".to_owned())?;
    if matches!(default_icon.value, Value::Scalar(_)) {
        return Err("authored HUD icon crosshair_default is not an object".to_owned());
    }
    let stock = icon(content, default_icon, "")?;
    let weapons = vec![
        weapon(content, "scripts/tf_weapon_rocketlauncher.ctx", &[1, 2])?,
        weapon(content, "scripts/tf_weapon_pipebomblauncher.ctx", &[3])?,
        weapon(content, "scripts/tf_weapon_scattergun.ctx", &[4])?,
        weapon(content, "scripts/tf_weapon_pistol_scout.ctx", &[5])?,
        weapon(content, "scripts/tf_weapon_bat.ctx", &[6])?,
      weapon(content, "scripts/tf_weapon_shotgun_soldier.ctx", &[7])?,
        weapon(content, "scripts/tf_weapon_shovel.ctx", &[8])?,
      weapon(content, "scripts/tf_weapon_minigun.ctx", &[9])?,
        weapon(content, "scripts/tf_weapon_shotgun_hwg.ctx", &[10])?,
        weapon(content, "scripts/tf_weapon_fists.ctx", &[11])?,
        weapon(content, "scripts/tf_weapon_sniperrifle.ctx", &[12])?,
        weapon(content, "scripts/tf_weapon_smg.ctx", &[13])?,
        weapon(content, "scripts/tf_weapon_club.ctx", &[14])?,
        weapon(content, "scripts/tf_weapon_bottle.ctx", &[17])?,
        weapon(content, "scripts/tf_weapon_grenadelauncher.ctx", &[18])?,
        weapon(content, "scripts/tf_weapon_shotgun_primary.ctx", &[40])?,
        weapon(content, "scripts/tf_weapon_pistol.ctx", &[41])?,
        weapon(content, "scripts/tf_weapon_wrench.ctx", &[42])?,
        weapon(content, "scripts/tf_weapon_flamethrower.ctx", &[15])?,
        weapon(content, "scripts/tf_weapon_fireaxe.ctx", &[16])?,
        weapon(content, "scripts/tf_weapon_revolver.ctx", &[50])?,
        weapon(content, "scripts/tf_weapon_knife.ctx", &[51])?,
        weapon(content, "scripts/tf_weapon_builder.ctx", &[52])?,
        weapon(content, "scripts/tf_weapon_pda_spy.ctx", &[53])?,
        weapon(content, "scripts/tf_weapon_invis.ctx", &[54])?,
    ];

    let directory_bytes = fs::read(tf2.join("tf2_textures_dir.vpk"))
        .map_err(|error| format!("crosshair texture index: {error}"))?;
    let archive = playsrc_vpk::parse(
        &directory_bytes,
        "tf2_textures_dir.vpk",
        playsrc_vpk::Layout::Split,
        playsrc_vpk::Limits::default(),
    )
    .map_err(|error| format!("crosshair texture index: {error}"))?;
    let names = archive
        .entries
        .iter()
        .filter_map(|entry| {
            entry
                .logical_path
                .strip_prefix("materials/vgui/crosshairs/")?
                .strip_suffix(".vtf")
                .filter(|name| !name.is_empty() && !name.contains('/'))
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    if names.len() > MAX_STYLES {
        return Err("authored crosshair style inventory exceeds its bound".to_owned());
    }
    let mut styles = Vec::with_capacity(names.len());
    for name in names {
        let material_path = format!("materials/vgui/crosshairs/{name}.vmt");
        if matches!(
            content
                .resolve_resource(&material_path)
                .map_err(|error| error.to_string())?,
            Resolution::Missing { .. }
        ) {
            continue;
        }
        styles.push(authored(
            content,
            &name,
            &format!("crosshairs/{name}"),
            None,
        )?);
    }
    if styles.is_empty() {
        return Err("configured content contains no paired authored crosshair styles".to_owned());
    }
    let report = AuthoredCrosshairs {
        schema: "playsrc-tf2-authored-crosshairs-v1",
        content_build: content_build.to_owned(),
        icon_source: SourceIdentity {
            logical_path: icon_resource.provenance.logical_path,
            byte_length: icon_resource.provenance.byte_length,
            sha256: icon_resource.provenance.sha256,
        },
        stock,
        weapons,
        styles,
    };
    let json = serde_json::to_string(&report)
        .map_err(|error| error.to_string())?
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    let generated = format!(
        "// Generated by generator/src/main.rs from exact configured producer outputs.\n// Do not edit.\nexport const configuredTf2AuthoredCrosshairInput: unknown = {json}\n"
    );
    fs::write(
        output_directory.join("crosshair.generated.ts"),
        generated.as_bytes(),
    )
    .map_err(|error| format!("authored crosshair output: {error}"))?;

    let scope = AuthoredScope {
        schema: "playsrc-tf2-authored-sniper-scope-v1",
        content_build: content_build.to_owned(),
        quadrants: ["ul", "ur", "lr", "ll"]
            .into_iter()
            .map(|suffix| {
                scope_source(content, &format!("materials/hud/scope_sniper_{suffix}.vmt"))
                    .map(|value| value.0)
            })
            .collect::<Result<Vec<_>, _>>()?,
        charge_material: scope_source(content, "materials/hud/sniperscope_numbers.vmt")?.0,
        tint: scope_texture(content, "materials/hud/scope_sniper_ul.vtf")?,
        normal: scope_texture(content, "materials/hud/scope_normal_ul.vtf")?,
        charge_base: scope_texture(content, "materials/hud/sniperscope_numbers.vtf")?,
        charge_mask: scope_texture(content, "materials/hud/sniperscope_numbers2.vtf")?,
    };
    let json = serde_json::to_string(&scope).map_err(|error| error.to_string())?;
    fs::write(
        output_directory.join("scope.generated.ts"),
        format!("// Generated by generator/src/main.rs from exact configured producer outputs.\\n// Do not edit.\\nexport const configuredTf2AuthoredScopeInput: unknown = {json}\\n").replace("\\n", "\n"),
    ).map_err(|error| format!("authored scope output: {error}"))
}
