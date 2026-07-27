use crate::{Error, ErrorCode, error};
use playsrc_bsp::{Bsp, Face, Lump, LumpData, TextureInfo};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const WORLD_LIGHT_BYTES: usize = 88;
const AMBIENT_INDEX_BYTES: usize = 4;
const AMBIENT_SAMPLE_BYTES: usize = 28;
const DETAIL_PROP_BYTES: usize = 52;
const DETAIL_STYLE_BYTES: usize = 5;
const SURF_BUMPLIGHT: i32 = 0x0800;
const GAME_LUMP: usize = 35;
const MAP_FLAGS: usize = 59;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingProfile {
    Ldr,
    Hdr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightingLimits {
    pub max_faces: usize,
    pub max_samples: usize,
    pub max_world_lights: usize,
    pub max_ambient_samples: usize,
    pub max_game_lumps: usize,
}
impl Default for LightingLimits {
    fn default() -> Self {
        Self {
            max_faces: 1_000_000,
            max_samples: 16_777_216,
            max_world_lights: 1_000_000,
            max_ambient_samples: 4_000_000,
            max_game_lumps: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum LightingMemberRole {
    Faces = 1,
    Samples = 2,
    WorldLights = 3,
    AmbientIndexes = 4,
    AmbientSamples = 5,
    MapFlags = 6,
    GameLumpDirectory = 7,
    DetailProps = 8,
    DetailLighting = 9,
    StaticProps = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingSource {
    StandardLump { slot: u8, version: i32 },
    GameLump { id: [u8; 4], version: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingMember {
    pub role: LightingMemberRole,
    pub source: Option<LightingSource>,
    pub encoded_bytes: u32,
    pub decoded_bytes: u32,
    pub encoded_sha256: [u8; 32],
    pub decoded_sha256: [u8; 32],
    pub item_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LightingSamples {
    RgbExp32(Vec<[u8; 4]>),
    LinearRgb32(Vec<[f32; 3]>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceLightingKind {
    Unlit,
    Flat,
    Directional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceLighting {
    pub face: u32,
    pub kind: SurfaceLightingKind,
    pub styles: [u8; 4],
    pub style_count: u8,
    pub sample_start: u32,
    pub samples_per_layer: u32,
    pub layer_count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldLight {
    pub origin: [f32; 3],
    pub intensity: [f32; 3],
    pub normal: [f32; 3],
    pub cluster: i32,
    pub kind: i32,
    pub style: u8,
    pub stop_dot: f32,
    pub stop_dot2: f32,
    pub exponent: f32,
    pub radius: f32,
    pub constant_attenuation: f32,
    pub linear_attenuation: f32,
    pub quadratic_attenuation: f32,
    pub flags: i32,
    pub texture_info: i32,
    pub owner: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmbientIndex {
    pub sample_count: u16,
    pub first_sample: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientSample {
    pub cube: [[f32; 3]; 6],
    pub position: [u8; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropLighting {
    pub detail_props: u32,
    pub detail_style_samples: u32,
    pub static_props: u32,
    pub map_flags: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightingData {
    pub profile: LightingProfile,
    pub closure_sha256: [u8; 32],
    pub members: Vec<LightingMember>,
    pub samples: LightingSamples,
    pub surfaces: Vec<SurfaceLighting>,
    pub world_lights: Vec<WorldLight>,
    pub ambient_indexes: Vec<AmbientIndex>,
    pub ambient_samples: Vec<AmbientSample>,
    pub prop_lighting: PropLighting,
    pub lightmapped_faces: u32,
    pub directional_faces: u32,
}

pub(crate) fn compile_lighting(
    bsp: &Bsp,
    profile: LightingProfile,
    faces: &[Face],
    texture_info: &[TextureInfo],
    limits: LightingLimits,
) -> Result<LightingData, Error> {
    if faces.len() > limits.max_faces {
        return Err(error(ErrorCode::BoundExceeded, None));
    }
    let (face_slot, sample_slot, world_slot, ambient_index_slot, ambient_sample_slot) =
        match profile {
            LightingProfile::Ldr => (7, 8, 15, 52, 56),
            LightingProfile::Hdr => (58, 53, 54, 51, 55),
        };
    let face_lump = required_lump(bsp, face_slot, 1)?;
    let sample_lump = required_lump(bsp, sample_slot, 1)?;
    let world_lump = required_lump(bsp, world_slot, 0)?;
    let ambient_index_lump = required_lump(bsp, ambient_index_slot, 0)?;
    let ambient_sample_lump = required_lump(bsp, ambient_sample_slot, 1)?;
    let map_flags_lump = required_lump(bsp, MAP_FLAGS, 0)?;
    let raw_samples = match &sample_lump.records {
        LumpData::Lighting(samples) => samples,
        _ => {
            return Err(error(
                ErrorCode::IncompleteLightingProfile,
                Some(sample_slot),
            ));
        }
    };
    if raw_samples.len() > limits.max_samples {
        return Err(error(ErrorCode::BoundExceeded, Some(sample_slot)));
    }

    let mut surfaces = Vec::with_capacity(faces.len());
    let mut lightmapped_faces = 0_u32;
    let mut directional_faces = 0_u32;
    for (face_index, face) in faces.iter().enumerate() {
        let style_count = styles(face.styles, face_index)?;
        if face.light_offset < 0 {
            surfaces.push(SurfaceLighting {
                face: as_u32(face_index)?,
                kind: SurfaceLightingKind::Unlit,
                styles: face.styles,
                style_count,
                sample_start: 0,
                samples_per_layer: 0,
                layer_count: 0,
            });
            continue;
        }
        if style_count == 0 || face.light_offset % 4 != 0 {
            return Err(error(ErrorCode::InvalidLightingProfile, Some(face_index)));
        }
        let info_index = usize::try_from(face.texture_info_index)
            .map_err(|_| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let info = texture_info
            .get(info_index)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let width = usize::try_from(face.lightmap_size[0])
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| error(ErrorCode::InvalidLightingProfile, Some(face_index)))?;
        let height = usize::try_from(face.lightmap_size[1])
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| error(ErrorCode::InvalidLightingProfile, Some(face_index)))?;
        let samples_per_layer = width
            .checked_mul(height)
            .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(face_index)))?;
        let directional = info.flags & SURF_BUMPLIGHT != 0;
        let layer_count = if directional { 4_u8 } else { 1_u8 };
        let sample_start = usize::try_from(face.light_offset / 4)
            .map_err(|_| error(ErrorCode::InvalidLightingProfile, Some(face_index)))?;
        let sample_count = samples_per_layer
            .checked_mul(usize::from(layer_count))
            .and_then(|value| value.checked_mul(style_count as usize))
            .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(face_index)))?;
        let sample_end = sample_start
            .checked_add(sample_count)
            .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(face_index)))?;
        if sample_end > raw_samples.len() {
            return Err(error(ErrorCode::InvalidLightingProfile, Some(face_index)));
        }
        lightmapped_faces = lightmapped_faces
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(face_index)))?;
        if directional {
            directional_faces = directional_faces
                .checked_add(1)
                .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(face_index)))?;
        }
        surfaces.push(SurfaceLighting {
            face: as_u32(face_index)?,
            kind: if directional {
                SurfaceLightingKind::Directional
            } else {
                SurfaceLightingKind::Flat
            },
            styles: face.styles,
            style_count,
            sample_start: as_u32(sample_start)?,
            samples_per_layer: as_u32(samples_per_layer)?,
            layer_count,
        });
    }

    let world_lights = parse_world_lights(world_lump.bytes(bsp), limits, world_slot)?;
    let ambient_indexes = parse_ambient_indexes(ambient_index_lump.bytes(bsp))?;
    let ambient_samples = parse_ambient_samples(ambient_sample_lump.bytes(bsp), limits)?;
    let leaf_count = match &bsp.lumps[10].records {
        LumpData::Leaves(leaves) => leaves.len(),
        _ => return Err(error(ErrorCode::MissingLump, Some(10))),
    };
    if ambient_indexes.len() != leaf_count {
        return Err(error(
            ErrorCode::InvalidLightingProfile,
            Some(ambient_index_slot),
        ));
    }
    for (index, ambient) in ambient_indexes.iter().enumerate() {
        let end = usize::from(ambient.first_sample)
            .checked_add(usize::from(ambient.sample_count))
            .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(index)))?;
        if end > ambient_samples.len() {
            return Err(error(ErrorCode::InvalidLightingProfile, Some(index)));
        }
    }
    let map_flag_bytes = map_flags_lump.bytes(bsp);
    if map_flag_bytes.len() != 4 {
        return Err(error(ErrorCode::IncompleteLightingProfile, Some(MAP_FLAGS)));
    }
    let map_flags = u32_at(map_flag_bytes, 0);
    let game = parse_game_lumps(bsp, profile, limits)?;
    for (index, (first, count)) in game.detail_style_ranges.iter().copied().enumerate() {
        let end = first
            .checked_add(count)
            .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(index)))?;
        if end > game.detail_style_samples {
            return Err(error(ErrorCode::IncompleteLightingProfile, Some(index)));
        }
    }

    let mut members = vec![
        standard_member(bsp, face_lump, LightingMemberRole::Faces, faces.len())?,
        standard_member(
            bsp,
            sample_lump,
            LightingMemberRole::Samples,
            raw_samples.len(),
        )?,
        standard_member(
            bsp,
            world_lump,
            LightingMemberRole::WorldLights,
            world_lights.len(),
        )?,
        standard_member(
            bsp,
            ambient_index_lump,
            LightingMemberRole::AmbientIndexes,
            ambient_indexes.len(),
        )?,
        standard_member(
            bsp,
            ambient_sample_lump,
            LightingMemberRole::AmbientSamples,
            ambient_samples.len(),
        )?,
        standard_member(bsp, map_flags_lump, LightingMemberRole::MapFlags, 1)?,
    ];
    members.extend(game.members);
    members.sort_by_key(|member| member.role);
    let closure_sha256 = member_closure(profile, &members);
    let samples = match profile {
        LightingProfile::Ldr => LightingSamples::RgbExp32(
            raw_samples
                .iter()
                .map(|sample| [sample.red, sample.green, sample.blue, sample.exponent as u8])
                .collect(),
        ),
        LightingProfile::Hdr => LightingSamples::LinearRgb32(
            raw_samples
                .iter()
                .map(|sample| rgbexp(sample.red, sample.green, sample.blue, sample.exponent))
                .collect(),
        ),
    };
    Ok(LightingData {
        profile,
        closure_sha256,
        members,
        samples,
        surfaces,
        world_lights,
        ambient_indexes,
        ambient_samples,
        prop_lighting: PropLighting {
            detail_props: as_u32(game.detail_props)?,
            detail_style_samples: as_u32(game.detail_style_samples)?,
            static_props: as_u32(game.static_props)?,
            map_flags,
        },
        lightmapped_faces,
        directional_faces,
    })
}

fn required_lump(bsp: &Bsp, slot: usize, version: i32) -> Result<&Lump, Error> {
    let lump = bsp
        .lumps
        .get(slot)
        .ok_or_else(|| error(ErrorCode::IncompleteLightingProfile, Some(slot)))?;
    if lump.bytes(bsp).is_empty() || lump.version != version {
        return Err(error(ErrorCode::IncompleteLightingProfile, Some(slot)));
    }
    Ok(lump)
}

fn styles(styles: [u8; 4], face: usize) -> Result<u8, Error> {
    let count = styles.iter().position(|style| *style == 255).unwrap_or(4);
    if styles[count..].iter().any(|style| *style != 255)
        || styles[..count].iter().any(|style| *style > 63)
    {
        return Err(error(ErrorCode::InvalidLightingProfile, Some(face)));
    }
    Ok(count as u8)
}

fn parse_world_lights(
    bytes: &[u8],
    limits: LightingLimits,
    slot: usize,
) -> Result<Vec<WorldLight>, Error> {
    if !bytes.len().is_multiple_of(WORLD_LIGHT_BYTES) {
        return Err(error(ErrorCode::InvalidLightingProfile, Some(slot)));
    }
    if bytes.len() / WORLD_LIGHT_BYTES > limits.max_world_lights {
        return Err(error(ErrorCode::BoundExceeded, Some(slot)));
    }
    bytes
        .chunks_exact(WORLD_LIGHT_BYTES)
        .enumerate()
        .map(|(index, record)| {
            let origin = vector(record, 0, index)?;
            let intensity = vector(record, 12, index)?;
            let normal = vector(record, 24, index)?;
            let kind = i32_at(record, 40);
            let style = i32_at(record, 44);
            if !(0..=5).contains(&kind) || !(0..=63).contains(&style) {
                return Err(error(ErrorCode::InvalidLightingProfile, Some(index)));
            }
            let scalars = [
                finite(record, 48, index)?,
                finite(record, 52, index)?,
                finite(record, 56, index)?,
                finite(record, 60, index)?,
                finite(record, 64, index)?,
                finite(record, 68, index)?,
                finite(record, 72, index)?,
            ];
            Ok(WorldLight {
                origin,
                intensity,
                normal,
                cluster: i32_at(record, 36),
                kind,
                style: style as u8,
                stop_dot: scalars[0],
                stop_dot2: scalars[1],
                exponent: scalars[2],
                radius: scalars[3],
                constant_attenuation: scalars[4],
                linear_attenuation: scalars[5],
                quadratic_attenuation: scalars[6],
                flags: i32_at(record, 76),
                texture_info: i32_at(record, 80),
                owner: i32_at(record, 84),
            })
        })
        .collect()
}

fn parse_ambient_indexes(bytes: &[u8]) -> Result<Vec<AmbientIndex>, Error> {
    if !bytes.len().is_multiple_of(AMBIENT_INDEX_BYTES) {
        return Err(error(ErrorCode::InvalidLightingProfile, None));
    }
    Ok(bytes
        .chunks_exact(AMBIENT_INDEX_BYTES)
        .map(|record| AmbientIndex {
            sample_count: u16_at(record, 0),
            first_sample: u16_at(record, 2),
        })
        .collect())
}

fn parse_ambient_samples(
    bytes: &[u8],
    limits: LightingLimits,
) -> Result<Vec<AmbientSample>, Error> {
    if !bytes.len().is_multiple_of(AMBIENT_SAMPLE_BYTES) {
        return Err(error(ErrorCode::InvalidLightingProfile, None));
    }
    if bytes.len() / AMBIENT_SAMPLE_BYTES > limits.max_ambient_samples {
        return Err(error(ErrorCode::BoundExceeded, None));
    }
    Ok(bytes
        .chunks_exact(AMBIENT_SAMPLE_BYTES)
        .map(|record| AmbientSample {
            cube: std::array::from_fn(|side| {
                let at = side * 4;
                rgbexp(
                    record[at],
                    record[at + 1],
                    record[at + 2],
                    record[at + 3] as i8,
                )
            }),
            position: [record[24], record[25], record[26]],
        })
        .collect())
}

struct GameLighting {
    members: Vec<LightingMember>,
    detail_props: usize,
    detail_style_ranges: Vec<(usize, usize)>,
    detail_style_samples: usize,
    static_props: usize,
}

struct GameEntry {
    id: u32,
    version: u16,
    encoded: Vec<u8>,
    decoded: Vec<u8>,
}

fn parse_game_lumps(
    bsp: &Bsp,
    profile: LightingProfile,
    limits: LightingLimits,
) -> Result<GameLighting, Error> {
    let directory = &bsp.lumps[GAME_LUMP];
    let bytes = directory.bytes(bsp);
    if bytes.is_empty() {
        return Ok(GameLighting {
            members: vec![
                absent_member(LightingMemberRole::GameLumpDirectory),
                absent_member(LightingMemberRole::DetailProps),
                absent_member(LightingMemberRole::DetailLighting),
                absent_member(LightingMemberRole::StaticProps),
            ],
            detail_props: 0,
            detail_style_ranges: Vec::new(),
            detail_style_samples: 0,
            static_props: 0,
        });
    }
    if bytes.len() < 4 {
        return Err(error(ErrorCode::InvalidLightingProfile, Some(GAME_LUMP)));
    }
    let count = usize::try_from(i32_at(bytes, 0))
        .map_err(|_| error(ErrorCode::InvalidLightingProfile, Some(GAME_LUMP)))?;
    if count > limits.max_game_lumps {
        return Err(error(ErrorCode::BoundExceeded, Some(GAME_LUMP)));
    }
    let directory_end = count
        .checked_mul(16)
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(GAME_LUMP)))?;
    if directory_end > bytes.len() {
        return Err(error(ErrorCode::InvalidLightingProfile, Some(GAME_LUMP)));
    }
    let mut headers = Vec::with_capacity(count);
    let mut identities = BTreeSet::new();
    for index in 0..count {
        let at = 4 + index * 16;
        let id = u32_at(bytes, at);
        if !identities.insert(id) {
            return Err(error(ErrorCode::InvalidLightingProfile, Some(index)));
        }
        let flags = u16_at(bytes, at + 4);
        let version = u16_at(bytes, at + 6);
        let start = usize::try_from(i32_at(bytes, at + 8))
            .map_err(|_| error(ErrorCode::InvalidLightingProfile, Some(index)))?;
        let length = usize::try_from(i32_at(bytes, at + 12))
            .map_err(|_| error(ErrorCode::InvalidLightingProfile, Some(index)))?;
        if flags & !1 != 0 || start > bsp.source_bytes().len() {
            return Err(error(ErrorCode::InvalidLightingProfile, Some(index)));
        }
        headers.push((id, flags, version, start, length));
    }
    let mut entries = Vec::with_capacity(count);
    for (index, (id, flags, version, start, length)) in headers.iter().copied().enumerate() {
        let encoded_end = if flags & 1 != 0 {
            headers
                .iter()
                .map(|entry| entry.3)
                .filter(|candidate| *candidate > start)
                .min()
                .unwrap_or(directory.encoded_range.end)
        } else {
            start
                .checked_add(length)
                .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(index)))?
        };
        let encoded_range = bsp
            .source_bytes()
            .get(start..encoded_end)
            .ok_or_else(|| error(ErrorCode::InvalidLightingProfile, Some(index)))?;
        let encoded = if flags & 1 != 0 {
            let payload = encoded_range
                .get(8..12)
                .map(|value| u32::from_le_bytes(value.try_into().expect("fixed range")) as usize)
                .ok_or_else(|| error(ErrorCode::InvalidLightingProfile, Some(index)))?;
            encoded_range
                .get(
                    ..17_usize
                        .checked_add(payload)
                        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(index)))?,
                )
                .ok_or_else(|| error(ErrorCode::InvalidLightingProfile, Some(index)))?
        } else {
            encoded_range
        };
        let decoded = if flags & 1 != 0 {
            playsrc_bsp::decode_source_lzma_member(encoded, length, playsrc_bsp::Limits::default())
                .map_err(|_| error(ErrorCode::InvalidLightingProfile, Some(index)))?
        } else {
            encoded.to_vec()
        };
        entries.push(GameEntry {
            id,
            version,
            encoded: encoded.to_vec(),
            decoded,
        });
    }
    let mut members = vec![standard_member(
        bsp,
        directory,
        LightingMemberRole::GameLumpDirectory,
        count,
    )?];
    let detail = find_game(&entries, *b"dprp");
    let (detail_props, detail_style_ranges) = if let Some(entry) = detail {
        if entry.version != 4 {
            return Err(error(ErrorCode::InvalidLightingProfile, None));
        }
        let (props, styles) = detail_props(&entry.decoded)?;
        members.push(game_member(
            LightingMemberRole::DetailProps,
            *b"dprp",
            entry,
            props,
        )?);
        (props, styles)
    } else {
        members.push(absent_member(LightingMemberRole::DetailProps));
        (0, Vec::new())
    };
    let detail_id = match profile {
        LightingProfile::Ldr => *b"dplt",
        LightingProfile::Hdr => *b"dplh",
    };
    let detail_style_samples = if let Some(entry) = find_game(&entries, detail_id) {
        if entry.version != 0 {
            return Err(error(ErrorCode::InvalidLightingProfile, None));
        }
        let samples = detail_styles(&entry.decoded)?;
        members.push(game_member(
            LightingMemberRole::DetailLighting,
            detail_id,
            entry,
            samples,
        )?);
        samples
    } else {
        members.push(absent_member(LightingMemberRole::DetailLighting));
        0
    };
    let static_props = if let Some(entry) = find_game(&entries, *b"sprp") {
        let props = static_prop_count(&entry.decoded)?;
        members.push(game_member(
            LightingMemberRole::StaticProps,
            *b"sprp",
            entry,
            props,
        )?);
        props
    } else {
        members.push(absent_member(LightingMemberRole::StaticProps));
        0
    };
    Ok(GameLighting {
        members,
        detail_props,
        detail_style_ranges,
        detail_style_samples,
        static_props,
    })
}

fn find_game(entries: &[GameEntry], id: [u8; 4]) -> Option<&GameEntry> {
    let id = u32::from_be_bytes(id);
    entries.iter().find(|entry| entry.id == id)
}

fn detail_props(bytes: &[u8]) -> Result<(usize, Vec<(usize, usize)>), Error> {
    let mut at = 0;
    let models = count(bytes, &mut at)?;
    advance(bytes, &mut at, models, 128)?;
    let sprites = count(bytes, &mut at)?;
    advance(bytes, &mut at, sprites, 32)?;
    let props = count(bytes, &mut at)?;
    let end = advance(bytes, &mut at, props, DETAIL_PROP_BYTES)?;
    if end != bytes.len() {
        return Err(error(ErrorCode::InvalidLightingProfile, None));
    }
    let records = &bytes[end - props * DETAIL_PROP_BYTES..end];
    let mut styles = Vec::with_capacity(props);
    for record in records.chunks_exact(DETAIL_PROP_BYTES) {
        let first = u32_at(record, 32) as usize;
        let count = record[36] as usize;
        first
            .checked_add(count)
            .ok_or_else(|| error(ErrorCode::BoundExceeded, None))?;
        styles.push((first, count));
    }
    Ok((props, styles))
}

fn detail_styles(bytes: &[u8]) -> Result<usize, Error> {
    let mut at = 0;
    let count = count(bytes, &mut at)?;
    let end = advance(bytes, &mut at, count, DETAIL_STYLE_BYTES)?;
    if end != bytes.len()
        || bytes[4..]
            .chunks_exact(DETAIL_STYLE_BYTES)
            .any(|record| record[4] > 63)
    {
        return Err(error(ErrorCode::InvalidLightingProfile, None));
    }
    Ok(count)
}

fn static_prop_count(bytes: &[u8]) -> Result<usize, Error> {
    let mut at = 0;
    let models = count(bytes, &mut at)?;
    advance(bytes, &mut at, models, 128)?;
    let leaves = count(bytes, &mut at)?;
    advance(bytes, &mut at, leaves, 2)?;
    let props = count(bytes, &mut at)?;
    if props == 0 && at != bytes.len() {
        return Err(error(ErrorCode::InvalidLightingProfile, None));
    }
    Ok(props)
}

fn count(bytes: &[u8], at: &mut usize) -> Result<usize, Error> {
    let end = at
        .checked_add(4)
        .ok_or_else(|| error(ErrorCode::BoundExceeded, None))?;
    let value = usize::try_from(i32_at(
        bytes
            .get(*at..end)
            .ok_or_else(|| error(ErrorCode::InvalidLightingProfile, None))?,
        0,
    ))
    .map_err(|_| error(ErrorCode::InvalidLightingProfile, None))?;
    *at = end;
    Ok(value)
}

fn advance(bytes: &[u8], at: &mut usize, count: usize, stride: usize) -> Result<usize, Error> {
    let end = count
        .checked_mul(stride)
        .and_then(|value| at.checked_add(value))
        .ok_or_else(|| error(ErrorCode::BoundExceeded, None))?;
    if end > bytes.len() {
        return Err(error(ErrorCode::InvalidLightingProfile, None));
    }
    *at = end;
    Ok(end)
}

fn standard_member(
    bsp: &Bsp,
    lump: &Lump,
    role: LightingMemberRole,
    count: usize,
) -> Result<LightingMember, Error> {
    Ok(LightingMember {
        role,
        source: Some(LightingSource::StandardLump {
            slot: u8::try_from(lump.index)
                .map_err(|_| error(ErrorCode::BoundExceeded, Some(lump.index)))?,
            version: lump.version,
        }),
        encoded_bytes: as_u32(lump.encoded_bytes(bsp).len())?,
        decoded_bytes: as_u32(lump.bytes(bsp).len())?,
        encoded_sha256: Sha256::digest(lump.encoded_bytes(bsp)).into(),
        decoded_sha256: Sha256::digest(lump.bytes(bsp)).into(),
        item_count: as_u32(count)?,
    })
}

fn game_member(
    role: LightingMemberRole,
    id: [u8; 4],
    entry: &GameEntry,
    count: usize,
) -> Result<LightingMember, Error> {
    Ok(LightingMember {
        role,
        source: Some(LightingSource::GameLump {
            id,
            version: entry.version,
        }),
        encoded_bytes: as_u32(entry.encoded.len())?,
        decoded_bytes: as_u32(entry.decoded.len())?,
        encoded_sha256: Sha256::digest(&entry.encoded).into(),
        decoded_sha256: Sha256::digest(&entry.decoded).into(),
        item_count: as_u32(count)?,
    })
}

fn absent_member(role: LightingMemberRole) -> LightingMember {
    LightingMember {
        role,
        source: None,
        encoded_bytes: 0,
        decoded_bytes: 0,
        encoded_sha256: [0; 32],
        decoded_sha256: [0; 32],
        item_count: 0,
    }
}

fn member_closure(profile: LightingProfile, members: &[LightingMember]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-lighting-profile-v1");
    digest.update([match profile {
        LightingProfile::Ldr => 0,
        LightingProfile::Hdr => 1,
    }]);
    for member in members {
        digest.update([member.role as u8]);
        match member.source {
            Some(LightingSource::StandardLump { slot, version }) => {
                digest.update([1, slot]);
                digest.update(version.to_le_bytes());
            }
            Some(LightingSource::GameLump { id, version }) => {
                digest.update([2]);
                digest.update(id);
                digest.update(version.to_le_bytes());
            }
            None => digest.update([0]),
        }
        digest.update(member.encoded_bytes.to_le_bytes());
        digest.update(member.decoded_bytes.to_le_bytes());
        digest.update(member.encoded_sha256);
        digest.update(member.decoded_sha256);
        digest.update(member.item_count.to_le_bytes());
    }
    digest.finalize().into()
}

fn rgbexp(red: u8, green: u8, blue: u8, exponent: i8) -> [f32; 3] {
    let scale = 2_f32.powi(exponent as i32) / 255.;
    [
        red as f32 * scale,
        green as f32 * scale,
        blue as f32 * scale,
    ]
}

fn vector(bytes: &[u8], at: usize, item: usize) -> Result<[f32; 3], Error> {
    Ok([
        finite(bytes, at, item)?,
        finite(bytes, at + 4, item)?,
        finite(bytes, at + 8, item)?,
    ])
}

fn finite(bytes: &[u8], at: usize, item: usize) -> Result<f32, Error> {
    let value = f32::from_bits(u32_at(bytes, at));
    if !value.is_finite() {
        return Err(error(ErrorCode::NonFinite, Some(item)));
    }
    Ok(value)
}

fn as_u32(value: usize) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| error(ErrorCode::BoundExceeded, None))
}

fn i32_at(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(bytes[at..at + 4].try_into().expect("validated fixed field"))
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("validated fixed field"))
}
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().expect("validated fixed field"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_bsp::{Limits as BspLimits, Profile as BspProfile};

    fn set_lump(bytes: &mut [u8], slot: usize, offset: usize, length: usize, version: i32) {
        let header = 8 + slot * 16;
        bytes[header..header + 4].copy_from_slice(&(offset as i32).to_le_bytes());
        bytes[header + 4..header + 8].copy_from_slice(&(length as i32).to_le_bytes());
        bytes[header + 8..header + 12].copy_from_slice(&version.to_le_bytes());
    }

    fn profile_bsp(hdr_world: bool, oversized_face: bool) -> Bsp {
        let mut bytes = vec![0; playsrc_bsp::HEADER_BYTES];
        bytes[..4].copy_from_slice(b"VBSP");
        bytes[4..8].copy_from_slice(&20_i32.to_le_bytes());
        let mut add = |slot: usize, version: i32, value: &[u8]| {
            let offset = bytes.len();
            bytes.extend_from_slice(value);
            set_lump(&mut bytes, slot, offset, value.len(), version);
        };
        let mut face = [0_u8; 56];
        face[16..20].copy_from_slice(&[0, 255, 255, 255]);
        if oversized_face {
            face[36..40].copy_from_slice(&1_i32.to_le_bytes());
        }
        let mut world = [0_u8; WORLD_LIGHT_BYTES];
        world[80..84].copy_from_slice(&(-1_i32).to_le_bytes());
        world[84..88].copy_from_slice(&(-1_i32).to_le_bytes());
        let mut leaf = [0_u8; 32];
        leaf[28..30].copy_from_slice(&(-1_i16).to_le_bytes());
        add(6, 0, &[0; 72]);
        add(10, 1, &leaf);
        for (face_slot, sample_slot, world_slot, index_slot, ambient_slot) in
            [(7, 8, 15, 52, 56), (58, 53, 54, 51, 55)]
        {
            add(face_slot, 1, &face);
            add(sample_slot, 1, &[255, 255, 255, 0]);
            if world_slot != 54 || hdr_world {
                add(world_slot, 0, &world);
            }
            add(index_slot, 0, &[1, 0, 0, 0]);
            add(ambient_slot, 1, &[0; AMBIENT_SAMPLE_BYTES]);
        }
        add(MAP_FLAGS, 0, &[0; 4]);
        playsrc_bsp::parse(&bytes, BspProfile::Source2013V20, BspLimits::default()).unwrap()
    }

    fn selected(bsp: &Bsp, profile: LightingProfile) -> (&[Face], &[TextureInfo]) {
        let face_slot = if profile == LightingProfile::Hdr {
            58
        } else {
            7
        };
        let LumpData::Faces(faces) = &bsp.lumps[face_slot].records else {
            panic!("faces were not parsed")
        };
        let LumpData::TextureInfo(info) = &bsp.lumps[6].records else {
            panic!("texture info was not parsed")
        };
        (faces, info)
    }

    #[test]
    fn rgbexp_is_finite_linear_radiance_below_at_and_above_one() {
        assert_eq!(rgbexp(0, 0, 0, -128), [0.; 3]);
        assert_eq!(rgbexp(255, 255, 255, 0), [1.; 3]);
        assert_eq!(rgbexp(255, 128, 0, 1), [2., 256. / 255., 0.]);
    }

    #[test]
    fn light_styles_are_contiguous_and_bounded() {
        assert_eq!(styles([0, 1, 255, 255], 0).unwrap(), 2);
        assert_eq!(styles([255; 4], 0).unwrap(), 0);
        assert_eq!(
            styles([0, 255, 1, 255], 0).unwrap_err().code,
            ErrorCode::InvalidLightingProfile
        );
        assert_eq!(
            styles([64, 255, 255, 255], 0).unwrap_err().code,
            ErrorCode::InvalidLightingProfile
        );
    }

    #[test]
    fn explicit_profiles_select_complete_members_without_fallback() {
        let bsp = profile_bsp(true, false);
        for profile in [LightingProfile::Ldr, LightingProfile::Hdr] {
            let (faces, info) = selected(&bsp, profile);
            let data =
                compile_lighting(&bsp, profile, faces, info, LightingLimits::default()).unwrap();
            assert_eq!(data.profile, profile);
            assert_eq!(data.lightmapped_faces, 1);
            assert_eq!(data.world_lights.len(), 1);
            assert_eq!(data.ambient_indexes.len(), 1);
            assert_eq!(data.ambient_samples.len(), 1);
            assert_eq!(data.members.len(), 10);
        }

        let bsp = profile_bsp(false, false);
        let (ldr_faces, info) = selected(&bsp, LightingProfile::Ldr);
        assert!(
            compile_lighting(
                &bsp,
                LightingProfile::Ldr,
                ldr_faces,
                info,
                LightingLimits::default()
            )
            .is_ok()
        );
        let (hdr_faces, info) = selected(&bsp, LightingProfile::Hdr);
        let failure = compile_lighting(
            &bsp,
            LightingProfile::Hdr,
            hdr_faces,
            info,
            LightingLimits::default(),
        )
        .unwrap_err();
        assert_eq!(failure.code, ErrorCode::IncompleteLightingProfile);
        assert_eq!(failure.item, Some(54));
    }

    #[test]
    fn profile_sample_ranges_and_limits_fail_before_output() {
        let bsp = profile_bsp(true, true);
        let (faces, info) = selected(&bsp, LightingProfile::Hdr);
        assert_eq!(
            compile_lighting(
                &bsp,
                LightingProfile::Hdr,
                faces,
                info,
                LightingLimits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidLightingProfile
        );

        let bsp = profile_bsp(true, false);
        let (faces, info) = selected(&bsp, LightingProfile::Hdr);
        assert_eq!(
            compile_lighting(
                &bsp,
                LightingProfile::Hdr,
                faces,
                info,
                LightingLimits {
                    max_samples: 0,
                    ..LightingLimits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::BoundExceeded
        );
    }
}
