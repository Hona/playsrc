// The Source ambient-normal table below is adapted from Valve Source SDK 2013 and is
// governed by the Source 1 SDK License retained at the repository root.
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const AGGREGATE_PATH: &str = "derived/static-prop-lighting.pvha";
const AGGREGATE_VERSION: u32 = 2;
const SECTION_VERSION: u32 = 2;
const MAX_OBJECTS: usize = 8_192;
const MAX_OCCURRENCES: usize = 65_536;
const MAX_LEAVES: usize = 1_000_000;
const MAX_BYTES: usize = 256 * 1024 * 1024;

// Valve Source SDK 2013 `src/mathlib/anorms.cpp`, retained in source order.
pub const SOURCE_AMBIENT_DIRECTIONS: [[f32; 3]; 162] = [
    [-0.525731, 0., 0.850651],
    [-0.442863, 0.238856, 0.864188],
    [-0.295242, 0., 0.955423],
    [-0.309017, 0.5, 0.809017],
    [-0.162460, 0.262866, 0.951056],
    [0., 0., 1.],
    [0., 0.850651, 0.525731],
    [-0.147621, 0.716567, 0.681718],
    [0.147621, 0.716567, 0.681718],
    [0., 0.525731, 0.850651],
    [0.309017, 0.5, 0.809017],
    [0.525731, 0., 0.850651],
    [0.295242, 0., 0.955423],
    [0.442863, 0.238856, 0.864188],
    [0.162460, 0.262866, 0.951056],
    [-0.681718, 0.147621, 0.716567],
    [-0.809017, 0.309017, 0.5],
    [-0.587785, 0.425325, 0.688191],
    [-0.850651, 0.525731, 0.],
    [-0.864188, 0.442863, 0.238856],
    [-0.716567, 0.681718, 0.147621],
    [-0.688191, 0.587785, 0.425325],
    [-0.5, 0.809017, 0.309017],
    [-0.238856, 0.864188, 0.442863],
    [-0.425325, 0.688191, 0.587785],
    [-0.716567, 0.681718, -0.147621],
    [-0.5, 0.809017, -0.309017],
    [-0.525731, 0.850651, 0.],
    [0., 0.850651, -0.525731],
    [-0.238856, 0.864188, -0.442863],
    [0., 0.955423, -0.295242],
    [-0.262866, 0.951056, -0.162460],
    [0., 1., 0.],
    [0., 0.955423, 0.295242],
    [-0.262866, 0.951056, 0.162460],
    [0.238856, 0.864188, 0.442863],
    [0.262866, 0.951056, 0.162460],
    [0.5, 0.809017, 0.309017],
    [0.238856, 0.864188, -0.442863],
    [0.262866, 0.951056, -0.162460],
    [0.5, 0.809017, -0.309017],
    [0.850651, 0.525731, 0.],
    [0.716567, 0.681718, 0.147621],
    [0.716567, 0.681718, -0.147621],
    [0.525731, 0.850651, 0.],
    [0.425325, 0.688191, 0.587785],
    [0.864188, 0.442863, 0.238856],
    [0.688191, 0.587785, 0.425325],
    [0.809017, 0.309017, 0.5],
    [0.681718, 0.147621, 0.716567],
    [0.587785, 0.425325, 0.688191],
    [0.955423, 0.295242, 0.],
    [1., 0., 0.],
    [0.951056, 0.162460, 0.262866],
    [0.850651, -0.525731, 0.],
    [0.955423, -0.295242, 0.],
    [0.864188, -0.442863, 0.238856],
    [0.951056, -0.162460, 0.262866],
    [0.809017, -0.309017, 0.5],
    [0.681718, -0.147621, 0.716567],
    [0.850651, 0., 0.525731],
    [0.864188, 0.442863, -0.238856],
    [0.809017, 0.309017, -0.5],
    [0.951056, 0.162460, -0.262866],
    [0.525731, 0., -0.850651],
    [0.681718, 0.147621, -0.716567],
    [0.681718, -0.147621, -0.716567],
    [0.850651, 0., -0.525731],
    [0.809017, -0.309017, -0.5],
    [0.864188, -0.442863, -0.238856],
    [0.951056, -0.162460, -0.262866],
    [0.147621, 0.716567, -0.681718],
    [0.309017, 0.5, -0.809017],
    [0.425325, 0.688191, -0.587785],
    [0.442863, 0.238856, -0.864188],
    [0.587785, 0.425325, -0.688191],
    [0.688191, 0.587785, -0.425325],
    [-0.147621, 0.716567, -0.681718],
    [-0.309017, 0.5, -0.809017],
    [0., 0.525731, -0.850651],
    [-0.525731, 0., -0.850651],
    [-0.442863, 0.238856, -0.864188],
    [-0.295242, 0., -0.955423],
    [-0.162460, 0.262866, -0.951056],
    [0., 0., -1.],
    [0.295242, 0., -0.955423],
    [0.162460, 0.262866, -0.951056],
    [-0.442863, -0.238856, -0.864188],
    [-0.309017, -0.5, -0.809017],
    [-0.162460, -0.262866, -0.951056],
    [0., -0.850651, -0.525731],
    [-0.147621, -0.716567, -0.681718],
    [0.147621, -0.716567, -0.681718],
    [0., -0.525731, -0.850651],
    [0.309017, -0.5, -0.809017],
    [0.442863, -0.238856, -0.864188],
    [0.162460, -0.262866, -0.951056],
    [0.238856, -0.864188, -0.442863],
    [0.5, -0.809017, -0.309017],
    [0.425325, -0.688191, -0.587785],
    [0.716567, -0.681718, -0.147621],
    [0.688191, -0.587785, -0.425325],
    [0.587785, -0.425325, -0.688191],
    [0., -0.955423, -0.295242],
    [0., -1., 0.],
    [0.262866, -0.951056, -0.162460],
    [0., -0.850651, 0.525731],
    [0., -0.955423, 0.295242],
    [0.238856, -0.864188, 0.442863],
    [0.262866, -0.951056, 0.162460],
    [0.5, -0.809017, 0.309017],
    [0.716567, -0.681718, 0.147621],
    [0.525731, -0.850651, 0.],
    [-0.238856, -0.864188, -0.442863],
    [-0.5, -0.809017, -0.309017],
    [-0.262866, -0.951056, -0.162460],
    [-0.850651, -0.525731, 0.],
    [-0.716567, -0.681718, -0.147621],
    [-0.716567, -0.681718, 0.147621],
    [-0.525731, -0.850651, 0.],
    [-0.5, -0.809017, 0.309017],
    [-0.238856, -0.864188, 0.442863],
    [-0.262866, -0.951056, 0.162460],
    [-0.864188, -0.442863, 0.238856],
    [-0.809017, -0.309017, 0.5],
    [-0.688191, -0.587785, 0.425325],
    [-0.681718, -0.147621, 0.716567],
    [-0.442863, -0.238856, 0.864188],
    [-0.587785, -0.425325, 0.688191],
    [-0.309017, -0.5, 0.809017],
    [-0.147621, -0.716567, 0.681718],
    [-0.425325, -0.688191, 0.587785],
    [-0.162460, -0.262866, 0.951056],
    [0.442863, -0.238856, 0.864188],
    [0.162460, -0.262866, 0.951056],
    [0.309017, -0.5, 0.809017],
    [0.147621, -0.716567, 0.681718],
    [0., -0.525731, 0.850651],
    [0.425325, -0.688191, 0.587785],
    [0.587785, -0.425325, 0.688191],
    [0.688191, -0.587785, 0.425325],
    [-0.955423, 0.295242, 0.],
    [-0.951056, 0.162460, 0.262866],
    [-1., 0., 0.],
    [-0.850651, 0., 0.525731],
    [-0.955423, -0.295242, 0.],
    [-0.951056, -0.162460, 0.262866],
    [-0.864188, 0.442863, -0.238856],
    [-0.951056, 0.162460, -0.262866],
    [-0.809017, 0.309017, -0.5],
    [-0.864188, -0.442863, -0.238856],
    [-0.951056, -0.162460, -0.262866],
    [-0.809017, -0.309017, -0.5],
    [-0.681718, 0.147621, -0.716567],
    [-0.681718, -0.147621, -0.716567],
    [-0.850651, 0., -0.525731],
    [-0.688191, 0.587785, -0.425325],
    [-0.587785, 0.425325, -0.688191],
    [-0.425325, 0.688191, -0.587785],
    [-0.425325, -0.688191, -0.587785],
    [-0.587785, -0.425325, -0.688191],
    [-0.688191, -0.587785, -0.425325],
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VhvMesh {
    pub primitive: u32,
    pub body_part: u32,
    pub model: u32,
    pub lod: u32,
    pub mesh: u32,
    pub strip_group: u32,
    pub vertex_count: u32,
    pub encoded_bgra_range: std::ops::Range<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VhvObject {
    pub occurrence: u32,
    pub model: u32,
    pub profile: u8,
    pub logical_path: String,
    pub source_sha256: [u8; 32],
    pub parsed_sha256: [u8; 32],
    pub join_sha256: [u8; 32],
    pub vertex_count: u32,
    pub meshes: Vec<VhvMesh>,
    pub source_range: std::ops::Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VhvAggregate {
    pub sha256: [u8; 32],
    pub objects: Vec<VhvObject>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewOwnership {
    Main,
    Sky3d,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLight {
    pub source: u32,
    pub kind: i32,
    pub style: u8,
    pub ratio: f32,
    pub direction: [f32; 3],
    pub intensity: [f32; 3],
    pub origin: [f32; 3],
    pub normal: [f32; 3],
    pub stop_dot: f32,
    pub stop_dot2: f32,
    pub exponent: f32,
    pub radius: f32,
    pub attenuation: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lighting {
    Vertex {
        ldr: u32,
        hdr: u32,
    },
    Runtime {
        sample_identity: [u8; 32],
        ambient_cube: [[f32; 3]; 6],
        lights: Vec<RuntimeLight>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    pub source: u32,
    pub dictionary_model: u32,
    pub presentation_model: u32,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub skin: i32,
    pub body: u32,
    pub lod: u32,
    pub fade_minimum: f32,
    pub fade_maximum: f32,
    pub forced_fade_scale: f32,
    pub flags: u32,
    pub solidity: u8,
    pub lighting_origin: Option<[f32; 3]>,
    pub leaves: Vec<u16>,
    pub areas: Vec<u16>,
    pub ownership: ViewOwnership,
    pub lighting: Lighting,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub aggregate_sha256: [u8; 32],
    pub model_count: u32,
    pub occurrences: Vec<Occurrence>,
}

#[allow(clippy::result_unit_err)]
pub fn decode_aggregate(bytes: &[u8]) -> Result<VhvAggregate, ()> {
    if bytes.len() > MAX_BYTES {
        return Err(());
    }
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != b"PVHA" || reader.u32()? != AGGREGATE_VERSION {
        return Err(());
    }
    let count = reader.count(MAX_OBJECTS)?;
    let mut objects = Vec::with_capacity(count);
    let mut previous = None;
    for _ in 0..count {
        let occurrence = reader.u32()?;
        let model = reader.u32()?;
        let profile = reader.u8()?;
        if profile > 1 || reader.take(3)? != [0, 0, 0] {
            return Err(());
        }
        let key = (occurrence, profile);
        if previous.is_some_and(|value| value >= key) {
            return Err(());
        }
        previous = Some(key);
        let mesh_count = reader.count(MAX_OBJECTS)?;
        let vertex_count = reader.u32()?;
        let source_sha256 = reader.hash()?;
        let parsed_sha256 = reader.hash()?;
        let join_sha256 = reader.hash()?;
        if parsed_sha256 != source_sha256 {
            return Err(());
        }
        let mut meshes = Vec::with_capacity(mesh_count);
        let mut joined_vertices = 0u32;
        for _ in 0..mesh_count {
            let primitive = reader.u32()?;
            let body_part = reader.u32()?;
            let child_model = reader.u32()?;
            let lod = reader.u32()?;
            let mesh = reader.u32()?;
            let strip_group = reader.u32()?;
            let mesh_vertices = reader.u32()?;
            let start = reader.u32()?;
            let end = reader.u32()?;
            if start > end {
                return Err(());
            }
            joined_vertices = joined_vertices.checked_add(mesh_vertices).ok_or(())?;
            meshes.push(VhvMesh {
                primitive,
                body_part,
                model: child_model,
                lod,
                mesh,
                strip_group,
                vertex_count: mesh_vertices,
                encoded_bgra_range: start..end,
            });
        }
        if joined_vertices != vertex_count {
            return Err(());
        }
        let logical_path = reader.text(1_024)?;
        let source_range = reader.blob_range(MAX_BYTES)?;
        let source = bytes.get(source_range.clone()).ok_or(())?;
        if source.is_empty()
            || <[u8; 32]>::from(Sha256::digest(source)) != source_sha256
            || meshes
                .iter()
                .any(|mesh| mesh.encoded_bgra_range.end as usize > source.len())
        {
            return Err(());
        }
        objects.push(VhvObject {
            occurrence,
            model,
            profile,
            logical_path,
            source_sha256,
            parsed_sha256,
            join_sha256,
            vertex_count,
            meshes,
            source_range,
        });
    }
    if !reader.done() {
        return Err(());
    }
    Ok(VhvAggregate {
        sha256: Sha256::digest(bytes).into(),
        objects,
    })
}

#[allow(clippy::result_unit_err)]
pub fn encode_section(section: &Section) -> Result<Vec<u8>, ()> {
    encode_section_with_cancel(section, |_| false)
}

#[allow(clippy::result_unit_err)]
pub fn encode_section_with_cancel(
    section: &Section,
    mut cancelled: impl FnMut(usize) -> bool,
) -> Result<Vec<u8>, ()> {
    if section.occurrences.len() > MAX_OCCURRENCES {
        return Err(());
    }
    let mut out = b"PSPA".to_vec();
    out.extend_from_slice(&SECTION_VERSION.to_le_bytes());
    out.extend_from_slice(&section.aggregate_sha256);
    out.extend_from_slice(&section.model_count.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(section.occurrences.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    let mut previous = None;
    for (occurrence_index, occurrence) in section.occurrences.iter().enumerate() {
        if cancelled(occurrence_index) {
            return Err(());
        }
        if previous.is_some_and(|value| value >= occurrence.source)
            || occurrence.presentation_model >= section.model_count
            || occurrence.leaves.len() != occurrence.areas.len()
            || occurrence.leaves.len() > MAX_LEAVES
        {
            return Err(());
        }
        previous = Some(occurrence.source);
        for value in [
            occurrence.source,
            occurrence.dictionary_model,
            occurrence.presentation_model,
            occurrence.body,
            occurrence.lod,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in occurrence.origin.into_iter().chain(occurrence.angles) {
            finite(value)?;
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&occurrence.skin.to_le_bytes());
        for value in [
            occurrence.fade_minimum,
            occurrence.fade_maximum,
            occurrence.forced_fade_scale,
        ] {
            finite(value)?;
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&occurrence.flags.to_le_bytes());
        out.extend_from_slice(&[
            occurrence.solidity,
            match occurrence.ownership {
                ViewOwnership::Main => 0,
                ViewOwnership::Sky3d => 1,
            },
            u8::from(occurrence.lighting_origin.is_some()),
            match occurrence.lighting {
                Lighting::Vertex { .. } => 0,
                Lighting::Runtime { .. } => 1,
            },
        ]);
        for value in occurrence.lighting_origin.unwrap_or([0.0; 3]) {
            finite(value)?;
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(
            &u32::try_from(occurrence.leaves.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for (leaf, area) in occurrence.leaves.iter().zip(&occurrence.areas) {
            out.extend_from_slice(&leaf.to_le_bytes());
            out.extend_from_slice(&area.to_le_bytes());
        }
        match &occurrence.lighting {
            Lighting::Vertex { ldr, hdr } => {
                out.extend_from_slice(&ldr.to_le_bytes());
                out.extend_from_slice(&hdr.to_le_bytes());
            }
            Lighting::Runtime {
                sample_identity,
                ambient_cube,
                lights,
            } => {
                if lights.len() > playsrc_studio_model::MAX_MODEL_LOCAL_LIGHTS {
                    return Err(());
                }
                out.extend_from_slice(sample_identity);
                for value in ambient_cube.iter().flatten() {
                    finite(*value)?;
                    out.extend_from_slice(&value.to_le_bytes());
                }
                out.extend_from_slice(&u32::try_from(lights.len()).map_err(|_| ())?.to_le_bytes());
                for light in lights {
                    out.extend_from_slice(&light.source.to_le_bytes());
                    out.extend_from_slice(&light.kind.to_le_bytes());
                    out.extend_from_slice(&[light.style, 0, 0, 0]);
                    finite(light.ratio)?;
                    out.extend_from_slice(&light.ratio.to_le_bytes());
                    for value in light
                        .direction
                        .into_iter()
                        .chain(light.intensity)
                        .chain(light.origin)
                        .chain(light.normal)
                        .chain([
                            light.stop_dot,
                            light.stop_dot2,
                            light.exponent,
                            light.radius,
                        ])
                        .chain(light.attenuation)
                    {
                        finite(value)?;
                        out.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        if out.len() > MAX_BYTES {
            return Err(());
        }
    }
    Ok(out)
}

#[allow(clippy::result_unit_err)]
pub fn decode_section(bytes: &[u8]) -> Result<Section, ()> {
    if bytes.len() > MAX_BYTES {
        return Err(());
    }
    let mut r = Reader::new(bytes);
    if r.take(4)? != b"PSPA" || r.u32()? != SECTION_VERSION {
        return Err(());
    }
    let aggregate_sha256 = r.hash()?;
    let model_count = r.u32()?;
    let count = r.count(MAX_OCCURRENCES)?;
    let mut occurrences = Vec::with_capacity(count);
    for _ in 0..count {
        let source = r.u32()?;
        let dictionary_model = r.u32()?;
        let presentation_model = r.u32()?;
        let body = r.u32()?;
        let lod = r.u32()?;
        let origin = r.vec3()?;
        let angles = r.vec3()?;
        let skin = r.i32()?;
        let fade_minimum = r.f32()?;
        let fade_maximum = r.f32()?;
        let forced_fade_scale = r.f32()?;
        let flags = r.u32()?;
        let solidity = r.u8()?;
        let ownership = match r.u8()? {
            0 => ViewOwnership::Main,
            1 => ViewOwnership::Sky3d,
            _ => return Err(()),
        };
        let has_origin = r.u8()?;
        let lighting_kind = r.u8()?;
        if has_origin > 1 || lighting_kind > 1 {
            return Err(());
        }
        let stored_origin = r.vec3()?;
        let lighting_origin = (has_origin == 1).then_some(stored_origin);
        let leaf_count = r.count(MAX_LEAVES)?;
        let mut leaves = Vec::with_capacity(leaf_count);
        let mut areas = Vec::with_capacity(leaf_count);
        for _ in 0..leaf_count {
            leaves.push(r.u16()?);
            areas.push(r.u16()?);
        }
        let lighting = if lighting_kind == 0 {
            Lighting::Vertex {
                ldr: r.u32()?,
                hdr: r.u32()?,
            }
        } else {
            let sample_identity = r.hash()?;
            let mut ambient_cube = [[0.0; 3]; 6];
            for value in ambient_cube.iter_mut().flatten() {
                *value = r.f32()?;
            }
            let light_count = r.count(playsrc_studio_model::MAX_MODEL_LOCAL_LIGHTS)?;
            let mut lights = Vec::with_capacity(light_count);
            for _ in 0..light_count {
                let light_source = r.u32()?;
                let kind = r.i32()?;
                let style = r.u8()?;
                if r.take(3)? != [0, 0, 0] {
                    return Err(());
                }
                let ratio = r.f32()?;
                let direction = r.vec3()?;
                let intensity = r.vec3()?;
                let origin = r.vec3()?;
                let normal = r.vec3()?;
                let stop_dot = r.f32()?;
                let stop_dot2 = r.f32()?;
                let exponent = r.f32()?;
                let radius = r.f32()?;
                let attenuation = r.vec3()?;
                lights.push(RuntimeLight {
                    source: light_source,
                    kind,
                    style,
                    ratio,
                    direction,
                    intensity,
                    origin,
                    normal,
                    stop_dot,
                    stop_dot2,
                    exponent,
                    radius,
                    attenuation,
                });
            }
            Lighting::Runtime {
                sample_identity,
                ambient_cube,
                lights,
            }
        };
        occurrences.push(Occurrence {
            source,
            dictionary_model,
            presentation_model,
            origin,
            angles,
            skin,
            body,
            lod,
            fade_minimum,
            fade_maximum,
            forced_fade_scale,
            flags,
            solidity,
            lighting_origin,
            leaves,
            areas,
            ownership,
            lighting,
        });
    }
    if !r.done() {
        return Err(());
    }
    let section = Section {
        aggregate_sha256,
        model_count,
        occurrences,
    };
    if encode_section(&section)? != bytes {
        return Err(());
    }
    Ok(section)
}

#[allow(clippy::result_unit_err)]
pub fn section_from_presentation(bytes: &[u8]) -> Result<&[u8], ()> {
    if bytes.len() < 8 || bytes.get(bytes.len() - 4..) != Some(b"PSPF") {
        return Err(());
    }
    let length = u32::from_le_bytes(
        bytes
            .get(bytes.len() - 8..bytes.len() - 4)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    ) as usize;
    if length > MAX_BYTES || length > bytes.len() - 8 {
        return Err(());
    }
    bytes
        .get(bytes.len() - 8 - length..bytes.len() - 8)
        .ok_or(())
}

fn finite(value: f32) -> Result<(), ()> {
    value.is_finite().then_some(()).ok_or(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], ()> {
        let end = self.offset.checked_add(length).ok_or(())?;
        let value = self.bytes.get(self.offset..end).ok_or(())?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, ()> {
        Ok(*self.take(1)?.first().ok_or(())?)
    }
    fn u16(&mut self) -> Result<u16, ()> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| ())?,
        ))
    }
    fn u32(&mut self) -> Result<u32, ()> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| ())?,
        ))
    }
    fn i32(&mut self) -> Result<i32, ()> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| ())?,
        ))
    }
    fn f32(&mut self) -> Result<f32, ()> {
        let value = f32::from_bits(self.u32()?);
        finite(value)?;
        Ok(value)
    }
    fn vec3(&mut self) -> Result<[f32; 3], ()> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn hash(&mut self) -> Result<[u8; 32], ()> {
        self.take(32)?.try_into().map_err(|_| ())
    }
    fn count(&mut self, maximum: usize) -> Result<usize, ()> {
        let value = usize::try_from(self.u32()?).map_err(|_| ())?;
        (value <= maximum).then_some(value).ok_or(())
    }
    fn blob_range(&mut self, maximum: usize) -> Result<std::ops::Range<usize>, ()> {
        let length = self.count(maximum)?;
        let start = self.offset;
        self.take(length)?;
        Ok(start..self.offset)
    }
    fn text(&mut self, maximum: usize) -> Result<String, ()> {
        let length = self.count(maximum)?;
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| ())
    }
    fn done(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[allow(clippy::result_unit_err)]
pub fn classify_ownership(areas: &[u16], sky_area: Option<u16>) -> Result<ViewOwnership, ()> {
    let distinct = areas.iter().copied().collect::<BTreeSet<_>>();
    match sky_area {
        Some(sky) if distinct == BTreeSet::from([sky]) => Ok(ViewOwnership::Sky3d),
        Some(sky) if distinct.contains(&sky) => Err(()),
        _ => Ok(ViewOwnership::Main),
    }
}

#[allow(clippy::result_unit_err)]
pub fn object_indexes(aggregate: &VhvAggregate) -> Result<BTreeMap<(u32, u8), u32>, ()> {
    aggregate
        .objects
        .iter()
        .enumerate()
        .map(|(index, object)| {
            Ok((
                (object.occurrence, object.profile),
                u32::try_from(index).map_err(|_| ())?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn section_round_trip_is_canonical_and_bounded() {
        let section = Section {
            aggregate_sha256: [1; 32],
            model_count: 1,
            occurrences: vec![Occurrence {
                source: 2,
                dictionary_model: 0,
                presentation_model: 0,
                origin: [1.0, 2.0, 3.0],
                angles: [0.0; 3],
                skin: 0,
                body: 0,
                lod: 0,
                fade_minimum: 0.0,
                fade_maximum: 1.0,
                forced_fade_scale: 1.0,
                flags: 0,
                solidity: 0,
                lighting_origin: None,
                leaves: vec![3],
                areas: vec![1],
                ownership: ViewOwnership::Main,
                lighting: Lighting::Vertex { ldr: 0, hdr: 1 },
            }],
        };
        let bytes = encode_section(&section).unwrap();
        assert_eq!(decode_section(&bytes).unwrap(), section);
        assert!(encode_section_with_cancel(&section, |_| true).is_err());
    }
    #[test]
    fn ownership_never_spans_main_and_sky() {
        assert_eq!(
            classify_ownership(&[1, 1], Some(1)),
            Ok(ViewOwnership::Sky3d)
        );
        assert!(classify_ownership(&[0, 1], Some(1)).is_err());
    }

    #[test]
    fn source_ambient_direction_order_and_cancellation_are_fixed() {
        assert_eq!(SOURCE_AMBIENT_DIRECTIONS.len(), 162);
        assert_eq!(SOURCE_AMBIENT_DIRECTIONS[0], [-0.525731, 0.0, 0.850651]);
        assert_eq!(SOURCE_AMBIENT_DIRECTIONS[52], [1.0, 0.0, 0.0]);
        assert_eq!(
            SOURCE_AMBIENT_DIRECTIONS[161],
            [-0.688191, -0.587785, -0.425325]
        );
        let section = Section {
            aggregate_sha256: [0; 32],
            model_count: 0,
            occurrences: Vec::new(),
        };
        let bytes = encode_section(&section).unwrap();
        let mut presentation = bytes.clone();
        presentation.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        presentation.extend_from_slice(b"PSPF");
        assert_eq!(section_from_presentation(&presentation).unwrap(), bytes);
    }
}
