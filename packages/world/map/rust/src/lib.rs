use playsrc_bsp::{Bsp, Face, LumpData, Primitive, TextureData, TextureInfo, Vector3};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};
mod lighting;
pub use lighting::*;
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialReference {
    pub index: usize,
    pub name: String,
    pub logical_path: String,
    pub width: i32,
    pub height: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub face: usize,
    pub model: usize,
    pub material: usize,
    pub flags: i32,
    pub draw: bool,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub lightmap_uv: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub light_offset: i32,
    pub light_styles: [u8; 4],
    pub lightmap_size: [i32; 2],
    pub compiled_primitives: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalMap {
    pub bsp_version: i32,
    pub map_revision: i32,
    pub lighting_profile: LightingProfile,
    pub materials: Vec<MaterialReference>,
    pub surfaces: Vec<Surface>,
    pub lighting: LightingData,
    pub world_model: usize,
    pub triangle_count: usize,
    pub vertex_count: usize,
}
pub struct Runtime {
    pub map: CanonicalMap,
    pub collision: playsrc_collision::World,
    pub visibility: playsrc_visibility::World,
    pub entities: playsrc_entity::Graph,
    pub descriptor: RuntimeDescriptor,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDescriptor {
    pub schema: u32,
    pub bsp_sha256: [u8; 32],
    pub compiler_identity: String,
    pub configuration_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
    pub derived_sha256: [u8; 32],
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTexture {
    pub logical_path: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMaterial {
    pub logical_path: String,
    pub shader: u8,
    pub features: u8,
    pub texture_role: u8,
    pub base_texture: Option<RuntimeTexture>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeInput {
    pub role: u8,
    pub logical_path: String,
    pub sha256: [u8; 32],
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProfileTexture {
    pub logical_path: String,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub source_sha256: [u8; 32],
    pub source_bytes: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProfileMaterial {
    pub logical_path: String,
    pub shader: u8,
    pub features: u8,
    pub texture_role: u8,
    pub texture: RuntimeProfileTexture,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeModelPrimitive {
    pub material: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeModel {
    pub logical_path: String,
    pub materials: Vec<RuntimeMaterial>,
    pub primitives: Vec<RuntimeModelPrimitive>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeModelOccurrence {
    pub entity: usize,
    pub model: usize,
    pub position: [f32; 3],
    pub angles: [f32; 3],
}
pub struct RuntimeAssembly<'a> {
    pub compiler_identity: &'a str,
    pub configuration: &'a [u8],
    pub materials: &'a [RuntimeMaterial],
    pub profile_materials: &'a [RuntimeProfileMaterial],
    pub inputs: &'a [RuntimeInput],
    pub output_role: &'a str,
    pub models: &'a [RuntimeModel],
    pub model_occurrences: &'a [RuntimeModelOccurrence],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidReference,
    InvalidRange,
    UnsupportedPrimitive,
    UnsupportedDisplacement,
    InvalidMaterial,
    NonFinite,
    IncompleteLightingProfile,
    InvalidLightingProfile,
    BoundExceeded,
    UnsupportedPropLighting,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub item: Option<usize>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for Error {}
pub fn compile(bsp: &Bsp, profile: LightingProfile) -> Result<CanonicalMap, Error> {
    if !bsp.lumps[26].bytes(bsp).is_empty() {
        return Err(error(ErrorCode::UnsupportedDisplacement, None));
    }
    let faces = match &bsp.lumps[if profile == LightingProfile::Hdr {
        58
    } else {
        7
    }]
    .records
    {
        LumpData::Faces(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let vertices = match &bsp.lumps[3].records {
        LumpData::Vertices(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let edges = match &bsp.lumps[12].records {
        LumpData::Edges(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let surfedges = match &bsp.lumps[13].records {
        LumpData::SurfaceEdges(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let texinfo = match &bsp.lumps[6].records {
        LumpData::TextureInfo(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let texdata = match &bsp.lumps[2].records {
        LumpData::TextureData(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let strings = match &bsp.lumps[43].records {
        LumpData::TextureStringData(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let offsets = match &bsp.lumps[44].records {
        LumpData::TextureStringOffsets(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let models = match &bsp.lumps[14].records {
        LumpData::Models(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let normals = match &bsp.lumps[30].records {
        LumpData::VertexNormals(v) => v.as_slice(),
        _ => &[],
    };
    let normal_indices = match &bsp.lumps[31].records {
        LumpData::VertexNormalIndices(v) => v.as_slice(),
        _ => &[],
    };
    let primitives = match &bsp.lumps[37].records {
        LumpData::Primitives(v) => v.as_slice(),
        _ => &[],
    };
    let primitive_vertices = match &bsp.lumps[38].records {
        LumpData::PrimitiveVertices(v) => v.as_slice(),
        _ => &[],
    };
    let primitive_indices = match &bsp.lumps[39].records {
        LumpData::PrimitiveIndices(v) => v.as_slice(),
        _ => &[],
    };
    let materials = materials(texdata, offsets, strings)?;
    let lighting = compile_lighting(bsp, profile, faces, texinfo, LightingLimits::default())?;
    let mut face_models = vec![usize::MAX; faces.len()];
    for (model_index, model) in models.iter().enumerate() {
        let start = usize::try_from(model.first_face)
            .map_err(|_| error(ErrorCode::InvalidRange, Some(model_index)))?;
        let count = usize::try_from(model.face_count)
            .map_err(|_| error(ErrorCode::InvalidRange, Some(model_index)))?;
        let end = start
            .checked_add(count)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(model_index)))?;
        if end > faces.len() || face_models[start..end].iter().any(|v| *v != usize::MAX) {
            return Err(error(ErrorCode::InvalidRange, Some(model_index)));
        }
        face_models[start..end].fill(model_index);
    }
    let mut output = Vec::with_capacity(faces.len());
    let mut normal_cursor = 0usize;
    let mut triangles = 0usize;
    let mut output_vertices = 0usize;
    for (face_index, face) in faces.iter().enumerate() {
        let positions = face_positions(face, face_index, vertices, edges, surfedges)?;
        if positions.len() < 3 {
            return Err(error(ErrorCode::InvalidRange, Some(face_index)));
        }
        let info = texinfo
            .get(
                usize::try_from(face.texture_info_index)
                    .map_err(|_| error(ErrorCode::InvalidReference, Some(face_index)))?,
            )
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let material = usize::try_from(info.texture_data_index)
            .map_err(|_| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let material_info = materials
            .get(material)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let face_normals = face_normals(
            face,
            face_index,
            &positions,
            normals,
            normal_indices,
            normal_cursor,
            bsp,
        )?;
        normal_cursor = normal_cursor
            .checked_add(face.surface_edge_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(face_index)))?;
        let uv = positions
            .iter()
            .map(|p| {
                uv(
                    p,
                    &info.texture_vectors,
                    material_info.width,
                    material_info.height,
                )
            })
            .collect();
        let lightmap_uv = positions
            .iter()
            .map(|p| lightmap_uv(p, info, face))
            .collect();
        let (mut indices, compiled) = triangles_for(
            face,
            face_index,
            positions.len(),
            primitives,
            primitive_vertices,
            primitive_indices,
        )?;
        normalize_triangle_winding(&positions, &face_normals, &mut indices);
        triangles += indices.len();
        output_vertices += positions.len();
        let flags = info.flags;
        let draw = flags & 0x0002 == 0
            && flags & 0x0004 == 0
            && flags & 0x0040 == 0
            && flags & 0x0080 == 0
            && flags & 0x0100 == 0
            && flags & 0x0200 == 0;
        output.push(Surface {
            face: face_index,
            model: face_models[face_index],
            material,
            flags,
            draw,
            positions,
            normals: face_normals,
            uv,
            lightmap_uv,
            triangles: indices,
            light_offset: face.light_offset,
            light_styles: face.styles,
            lightmap_size: face.lightmap_size,
            compiled_primitives: compiled,
        });
    }
    Ok(CanonicalMap {
        bsp_version: bsp.container_version,
        map_revision: bsp.map_revision,
        lighting_profile: profile,
        materials,
        surfaces: output,
        lighting,
        world_model: 0,
        triangle_count: triangles,
        vertex_count: output_vertices,
    })
}

pub fn compile_runtime(
    bsp: &Bsp,
    bsp_sha256: [u8; 32],
    profile: LightingProfile,
    assembly: RuntimeAssembly<'_>,
) -> Result<Runtime, Error> {
    let RuntimeAssembly {
        compiler_identity,
        configuration,
        materials: resolved_materials,
        profile_materials,
        inputs,
        output_role,
        models: runtime_models,
        model_occurrences,
    } = assembly;
    let map = compile(bsp, profile)?;
    let collision =
        playsrc_collision::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let visibility =
        playsrc_visibility::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let entities =
        playsrc_entity::parse(bsp.lumps[0].bytes(bsp), playsrc_entity::Limits::default())
            .map_err(|_| error(ErrorCode::InvalidReference, None))?;
    if !resolved_materials.is_empty() {
        if resolved_materials.len() != map.materials.len() {
            return Err(error(ErrorCode::InvalidMaterial, None));
        }
        for (index, material) in resolved_materials.iter().enumerate() {
            validate_runtime_material(material, index)?;
            if !material
                .logical_path
                .eq_ignore_ascii_case(&map.materials[index].logical_path)
            {
                return Err(error(ErrorCode::InvalidMaterial, Some(index)));
            }
        }
    }
    if profile == LightingProfile::Hdr && resolved_materials.len() != map.materials.len() {
        return Err(error(ErrorCode::InvalidMaterial, None));
    }
    for (index, material) in profile_materials.iter().enumerate() {
        validate_profile_material(material, index)?;
    }
    if profile_materials.len() > 64
        || inputs.len() > 4_096
        || inputs.iter().any(|input| input.logical_path.len() > 1_024)
    {
        return Err(error(ErrorCode::BoundExceeded, None));
    }
    let mut input_identities = BTreeSet::new();
    if inputs
        .iter()
        .any(|input| !input_identities.insert((input.role, input.logical_path.as_str())))
    {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    if profile == LightingProfile::Hdr && output_role.is_empty() {
        return Err(error(ErrorCode::IncompleteLightingProfile, None));
    }
    if inputs.iter().any(|input| input.logical_path.is_empty()) {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    for (model_index, model) in runtime_models.iter().enumerate() {
        if model.logical_path.is_empty() {
            return Err(error(ErrorCode::InvalidMaterial, Some(model_index)));
        }
        for primitive in &model.primitives {
            if primitive.material >= model.materials.len()
                || primitive.positions.len() != primitive.normals.len()
                || primitive.positions.len() != primitive.uv.len()
                || primitive
                    .triangles
                    .iter()
                    .flatten()
                    .any(|index| *index as usize >= primitive.positions.len())
            {
                return Err(error(ErrorCode::InvalidReference, Some(model_index)));
            }
        }
    }
    if model_occurrences.iter().any(|occurrence| {
        occurrence.model >= runtime_models.len()
            || occurrence
                .position
                .iter()
                .chain(occurrence.angles.iter())
                .any(|value| !value.is_finite())
    }) {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    let payload = serialize(
        &map,
        &entities,
        resolved_materials,
        profile_materials,
        inputs,
        compiler_identity,
        bsp_sha256,
        Sha256::digest(configuration).into(),
        output_role,
        runtime_models,
        model_occurrences,
    );
    if payload.len() > 512 * 1024 * 1024 {
        return Err(error(ErrorCode::BoundExceeded, None));
    }
    let payload_sha256 = Sha256::digest(&payload).into();
    let configuration_sha256 = Sha256::digest(configuration).into();
    let derived_sha256 = derived_identity(
        profile,
        output_role,
        compiler_identity,
        bsp_sha256,
        configuration_sha256,
        map.lighting.closure_sha256,
        inputs,
        payload_sha256,
    );
    let descriptor = RuntimeDescriptor {
        schema: 1,
        bsp_sha256,
        compiler_identity: compiler_identity.to_owned(),
        configuration_sha256,
        payload_sha256,
        derived_sha256,
        payload,
    };
    Ok(Runtime {
        map,
        collision,
        visibility,
        entities,
        descriptor,
    })
}
fn serialize(
    map: &CanonicalMap,
    entities: &playsrc_entity::Graph,
    materials: &[RuntimeMaterial],
    profile_materials: &[RuntimeProfileMaterial],
    inputs: &[RuntimeInput],
    compiler_identity: &str,
    bsp_sha256: [u8; 32],
    configuration_sha256: [u8; 32],
    output_role: &str,
    models: &[RuntimeModel],
    occurrences: &[RuntimeModelOccurrence],
) -> Vec<u8> {
    let mut out = b"PSMP".to_vec();
    u32v(
        &mut out,
        if map.lighting_profile == LightingProfile::Hdr {
            4
        } else if !models.is_empty() {
            3
        } else if !materials.is_empty() {
            2
        } else {
            1
        },
    );
    u32v(&mut out, map.bsp_version as u32);
    u32v(&mut out, map.map_revision as u32);
    out.push(match map.lighting_profile {
        LightingProfile::Ldr => 0,
        LightingProfile::Hdr => 1,
    });
    u32v(&mut out, map.materials.len() as u32);
    u32v(&mut out, map.surfaces.len() as u32);
    u32v(
        &mut out,
        lighting_sample_count(&map.lighting.samples) as u32,
    );
    u32v(&mut out, entities.entities.len() as u32);
    for m in &map.materials {
        bytesv(&mut out, m.logical_path.as_bytes());
        i32v(&mut out, m.width);
        i32v(&mut out, m.height);
    }
    for s in &map.surfaces {
        u32v(&mut out, s.face as u32);
        u32v(&mut out, s.model as u32);
        u32v(&mut out, s.material as u32);
        i32v(&mut out, s.flags);
        out.push(u8::from(s.draw));
        u32v(&mut out, s.positions.len() as u32);
        u32v(&mut out, s.triangles.len() as u32);
        for p in &s.positions {
            for v in p {
                f32v(&mut out, *v)
            }
        }
        for n in &s.normals {
            for v in n {
                f32v(&mut out, *v)
            }
        }
        for uv in &s.uv {
            f32v(&mut out, uv[0]);
            f32v(&mut out, uv[1]);
        }
        for uv in &s.lightmap_uv {
            f32v(&mut out, uv[0]);
            f32v(&mut out, uv[1]);
        }
        for t in &s.triangles {
            for v in t {
                u32v(&mut out, *v)
            }
        }
        i32v(&mut out, s.light_offset);
        out.extend_from_slice(&s.light_styles);
        i32v(&mut out, s.lightmap_size[0]);
        i32v(&mut out, s.lightmap_size[1]);
    }
    match &map.lighting.samples {
        LightingSamples::RgbExp32(samples) => {
            for sample in samples {
                out.extend_from_slice(sample)
            }
        }
        LightingSamples::LinearRgb32(samples) => {
            for sample in samples {
                for value in sample {
                    f32v(&mut out, *value)
                }
            }
        }
    }
    bytesv(&mut out, &entities.source);
    if !materials.is_empty() || map.lighting_profile == LightingProfile::Hdr {
        u32v(&mut out, materials.len() as u32);
        for material in materials {
            materialv(&mut out, material);
        }
    }
    if !models.is_empty() || map.lighting_profile == LightingProfile::Hdr {
        u32v(&mut out, models.len() as u32);
        for model in models {
            bytesv(&mut out, model.logical_path.as_bytes());
            u32v(&mut out, model.materials.len() as u32);
            for material in &model.materials {
                bytesv(&mut out, material.logical_path.as_bytes());
                materialv(&mut out, material);
            }
            u32v(&mut out, model.primitives.len() as u32);
            for primitive in &model.primitives {
                u32v(&mut out, primitive.material as u32);
                u32v(&mut out, primitive.positions.len() as u32);
                u32v(&mut out, primitive.triangles.len() as u32);
                for position in &primitive.positions {
                    for value in position {
                        f32v(&mut out, *value);
                    }
                }
                for normal in &primitive.normals {
                    for value in normal {
                        f32v(&mut out, *value);
                    }
                }
                for uv in &primitive.uv {
                    f32v(&mut out, uv[0]);
                    f32v(&mut out, uv[1]);
                }
                for triangle in &primitive.triangles {
                    for index in triangle {
                        u32v(&mut out, *index);
                    }
                }
            }
        }
        u32v(&mut out, occurrences.len() as u32);
        for occurrence in occurrences {
            u32v(&mut out, occurrence.entity as u32);
            u32v(&mut out, occurrence.model as u32);
            for value in occurrence.position {
                f32v(&mut out, value);
            }
            for value in occurrence.angles {
                f32v(&mut out, value);
            }
        }
    }
    if map.lighting_profile == LightingProfile::Hdr {
        serialize_hdr(
            &mut out,
            map,
            profile_materials,
            inputs,
            compiler_identity,
            bsp_sha256,
            configuration_sha256,
            output_role,
            materials,
        );
    }
    out
}
fn materialv(out: &mut Vec<u8>, material: &RuntimeMaterial) {
    out.push(material.shader);
    out.push(material.features);
    out.push(u8::from(material.base_texture.is_some()));
    out.push(material.texture_role);
    if let Some(texture) = &material.base_texture {
        bytesv(out, texture.logical_path.as_bytes());
        u32v(out, texture.width);
        u32v(out, texture.height);
        bytesv(out, &texture.rgba);
    }
}
fn lighting_sample_count(samples: &LightingSamples) -> usize {
    match samples {
        LightingSamples::RgbExp32(samples) => samples.len(),
        LightingSamples::LinearRgb32(samples) => samples.len(),
    }
}
#[allow(clippy::too_many_arguments)]
fn serialize_hdr(
    out: &mut Vec<u8>,
    map: &CanonicalMap,
    profile_materials: &[RuntimeProfileMaterial],
    inputs: &[RuntimeInput],
    compiler_identity: &str,
    bsp_sha256: [u8; 32],
    configuration_sha256: [u8; 32],
    output_role: &str,
    materials: &[RuntimeMaterial],
) {
    out.extend_from_slice(b"PSHD");
    u32v(out, 1);
    out.push(1); // Linear RGB binary32, three components per sample.
    out.extend_from_slice(&[0; 3]);
    bytesv(out, output_role.as_bytes());
    bytesv(out, compiler_identity.as_bytes());
    out.extend_from_slice(&bsp_sha256);
    out.extend_from_slice(&configuration_sha256);
    out.extend_from_slice(&map.lighting.closure_sha256);
    u32v(out, map.lighting.members.len() as u32);
    for member in &map.lighting.members {
        out.push(member.role as u8);
        match member.source {
            Some(LightingSource::StandardLump { slot, version }) => {
                out.extend_from_slice(&[1, slot, 0, 0]);
                i32v(out, version);
            }
            Some(LightingSource::GameLump { id, version }) => {
                out.extend_from_slice(&[2, 0, 0, 0]);
                out.extend_from_slice(&id);
                u32v(out, u32::from(version));
            }
            None => out.extend_from_slice(&[0; 8]),
        }
        u32v(out, member.encoded_bytes);
        u32v(out, member.decoded_bytes);
        out.extend_from_slice(&member.encoded_sha256);
        out.extend_from_slice(&member.decoded_sha256);
        u32v(out, member.item_count);
    }
    u32v(out, map.lighting.lightmapped_faces);
    u32v(out, map.lighting.directional_faces);
    u32v(out, map.lighting.surfaces.len() as u32);
    for surface in &map.lighting.surfaces {
        u32v(out, surface.face);
        let kind = match surface.kind {
            SurfaceLightingKind::Unlit => 0,
            SurfaceLightingKind::Flat => 1,
            SurfaceLightingKind::Directional => {
                let ssbump = map
                    .surfaces
                    .get(surface.face as usize)
                    .and_then(|map_surface| materials.get(map_surface.material))
                    .is_some_and(|material| material.features & (1 << 5) != 0);
                if ssbump { 3 } else { 2 }
            }
        };
        out.push(kind);
        out.push(surface.style_count);
        out.push(surface.layer_count);
        out.push(0);
        u32v(out, surface.sample_start);
        u32v(out, surface.samples_per_layer);
        out.extend_from_slice(&surface.styles);
    }
    u32v(out, map.lighting.world_lights.len() as u32);
    for light in &map.lighting.world_lights {
        for value in light
            .origin
            .into_iter()
            .chain(light.intensity)
            .chain(light.normal)
        {
            f32v(out, value);
        }
        i32v(out, light.cluster);
        i32v(out, light.kind);
        out.push(light.style);
        out.extend_from_slice(&[0; 3]);
        for value in [
            light.stop_dot,
            light.stop_dot2,
            light.exponent,
            light.radius,
            light.constant_attenuation,
            light.linear_attenuation,
            light.quadratic_attenuation,
        ] {
            f32v(out, value);
        }
        i32v(out, light.flags);
        i32v(out, light.texture_info);
        i32v(out, light.owner);
    }
    u32v(out, map.lighting.ambient_indexes.len() as u32);
    for index in &map.lighting.ambient_indexes {
        out.extend_from_slice(&index.sample_count.to_le_bytes());
        out.extend_from_slice(&index.first_sample.to_le_bytes());
    }
    u32v(out, map.lighting.ambient_samples.len() as u32);
    for sample in &map.lighting.ambient_samples {
        for side in sample.cube {
            for value in side {
                f32v(out, value);
            }
        }
        out.extend_from_slice(&sample.position);
        out.push(0);
    }
    u32v(out, map.lighting.prop_lighting.detail_props);
    u32v(out, map.lighting.prop_lighting.detail_style_samples);
    u32v(out, map.lighting.prop_lighting.static_props);
    u32v(out, map.lighting.prop_lighting.map_flags);
    u32v(out, profile_materials.len() as u32);
    for material in profile_materials {
        bytesv(out, material.logical_path.as_bytes());
        out.extend_from_slice(&[material.shader, material.features, material.texture_role, 0]);
        bytesv(out, material.texture.logical_path.as_bytes());
        u32v(out, material.texture.width);
        u32v(out, material.texture.height);
        i32v(out, material.texture.format);
        out.extend_from_slice(&material.texture.source_sha256);
        bytesv(out, &material.texture.source_bytes);
    }
    let mut inputs: Vec<_> = inputs.iter().collect();
    inputs.sort_by(|left, right| {
        (left.role, &left.logical_path, left.sha256).cmp(&(
            right.role,
            &right.logical_path,
            right.sha256,
        ))
    });
    u32v(out, inputs.len() as u32);
    for input in inputs {
        out.push(input.role);
        out.extend_from_slice(&[0; 3]);
        bytesv(out, input.logical_path.as_bytes());
        out.extend_from_slice(&input.sha256);
    }
}
fn validate_runtime_material(material: &RuntimeMaterial, item: usize) -> Result<(), Error> {
    if material.logical_path.is_empty() {
        return Err(error(ErrorCode::InvalidMaterial, Some(item)));
    }
    if let Some(texture) = &material.base_texture {
        let pixels = usize::try_from(texture.width)
            .ok()
            .and_then(|width| {
                usize::try_from(texture.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4));
        if texture.logical_path.is_empty()
            || texture.width == 0
            || texture.height == 0
            || pixels != Some(texture.rgba.len())
        {
            return Err(error(ErrorCode::InvalidMaterial, Some(item)));
        }
    }
    Ok(())
}
fn validate_profile_material(material: &RuntimeProfileMaterial, item: usize) -> Result<(), Error> {
    let texture = &material.texture;
    if material.logical_path.is_empty()
        || texture.logical_path.is_empty()
        || texture.width == 0
        || texture.height == 0
        || texture.width > 4_096
        || texture.height > 4_096
        || texture.source_bytes.is_empty()
        || texture.source_bytes.len() > 64 * 1024 * 1024
        || <[u8; 32]>::from(Sha256::digest(&texture.source_bytes)) != texture.source_sha256
    {
        return Err(error(ErrorCode::InvalidMaterial, Some(item)));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn derived_identity(
    profile: LightingProfile,
    output_role: &str,
    compiler_identity: &str,
    bsp_sha256: [u8; 32],
    configuration_sha256: [u8; 32],
    lighting_sha256: [u8; 32],
    inputs: &[RuntimeInput],
    payload_sha256: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-derived-map-v1");
    digest.update([match profile {
        LightingProfile::Ldr => 0,
        LightingProfile::Hdr => 1,
    }]);
    digest.update((output_role.len() as u32).to_le_bytes());
    digest.update(output_role.as_bytes());
    digest.update((compiler_identity.len() as u32).to_le_bytes());
    digest.update(compiler_identity.as_bytes());
    digest.update(bsp_sha256);
    digest.update(configuration_sha256);
    digest.update(lighting_sha256);
    let mut inputs: Vec<_> = inputs.iter().collect();
    inputs.sort_by(|left, right| {
        (left.role, &left.logical_path, left.sha256).cmp(&(
            right.role,
            &right.logical_path,
            right.sha256,
        ))
    });
    for input in inputs {
        digest.update([input.role]);
        digest.update((input.logical_path.len() as u32).to_le_bytes());
        digest.update(input.logical_path.as_bytes());
        digest.update(input.sha256);
    }
    digest.update(payload_sha256);
    digest.finalize().into()
}
fn u32v(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn i32v(o: &mut Vec<u8>, v: i32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn f32v(o: &mut Vec<u8>, v: f32) {
    u32v(o, v.to_bits())
}
fn bytesv(o: &mut Vec<u8>, v: &[u8]) {
    u32v(o, v.len() as u32);
    o.extend_from_slice(v)
}
fn materials(
    data: &[TextureData],
    table: &[i32],
    strings: &[u8],
) -> Result<Vec<MaterialReference>, Error> {
    data.iter()
        .enumerate()
        .map(|(index, v)| {
            let table_index = usize::try_from(v.name_string_table_index)
                .map_err(|_| error(ErrorCode::InvalidMaterial, Some(index)))?;
            let offset = usize::try_from(
                *table
                    .get(table_index)
                    .ok_or_else(|| error(ErrorCode::InvalidMaterial, Some(index)))?,
            )
            .map_err(|_| error(ErrorCode::InvalidMaterial, Some(index)))?;
            let tail = strings
                .get(offset..)
                .ok_or_else(|| error(ErrorCode::InvalidMaterial, Some(index)))?;
            let end = tail
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| error(ErrorCode::InvalidMaterial, Some(index)))?;
            let name = std::str::from_utf8(&tail[..end])
                .map_err(|_| error(ErrorCode::InvalidMaterial, Some(index)))?
                .replace('\\', "/");
            let suffix = if name.to_ascii_lowercase().ends_with(".vmt") {
                ""
            } else {
                ".vmt"
            };
            Ok(MaterialReference {
                index,
                name: name.clone(),
                logical_path: format!("materials/{name}{suffix}"),
                width: v.width,
                height: v.height,
            })
        })
        .collect()
}
fn face_positions(
    face: &Face,
    index: usize,
    vertices: &[playsrc_bsp::Vertex],
    edges: &[playsrc_bsp::Edge],
    surfedges: &[i32],
) -> Result<Vec<[f32; 3]>, Error> {
    let first = usize::try_from(face.first_surface_edge)
        .map_err(|_| error(ErrorCode::InvalidRange, Some(index)))?;
    let count = usize::try_from(face.surface_edge_count)
        .map_err(|_| error(ErrorCode::InvalidRange, Some(index)))?;
    let refs = surfedges
        .get(first..first + count)
        .ok_or_else(|| error(ErrorCode::InvalidRange, Some(index)))?;
    refs.iter()
        .map(|&reference| {
            let edge = edges
                .get(reference.unsigned_abs() as usize)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            let vertex = if reference >= 0 {
                edge.vertex_indices[0]
            } else {
                edge.vertex_indices[1]
            };
            let p = vertices
                .get(vertex as usize)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?
                .position;
            let out = [p.x.value(), p.y.value(), p.z.value()];
            if out.iter().any(|v| !v.is_finite()) {
                return Err(error(ErrorCode::NonFinite, Some(index)));
            }
            Ok(out)
        })
        .collect()
}
fn face_normals(
    face: &Face,
    index: usize,
    positions: &[[f32; 3]],
    normals: &[Vector3],
    indices: &[u16],
    start: usize,
    bsp: &Bsp,
) -> Result<Vec<[f32; 3]>, Error> {
    if !normals.is_empty() && !indices.is_empty() {
        return indices
            .get(start..start + positions.len())
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(index)))?
            .iter()
            .map(|v| {
                let n = normals
                    .get(*v as usize)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
                Ok([n.x.value(), n.y.value(), n.z.value()])
            })
            .collect();
    }
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let p = planes
        .get(face.plane_index as usize)
        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
    let normal = oriented_plane_normal(
        [p.normal.x.value(), p.normal.y.value(), p.normal.z.value()],
        face.side,
    );
    Ok(vec![normal; positions.len()])
}
fn oriented_plane_normal(normal: [f32; 3], side: u8) -> [f32; 3] {
    let sign = if side == 0 { 1. } else { -1. };
    normal.map(|component| component * sign)
}
fn normalize_triangle_winding(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    triangles: &mut [[u32; 3]],
) {
    for triangle in triangles {
        let [a, b, c] = triangle.map(|index| positions[index as usize]);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let geometric = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let supplied = triangle.iter().fold([0.; 3], |mut sum, index| {
            let normal = normals[*index as usize];
            for axis in 0..3 {
                sum[axis] += normal[axis];
            }
            sum
        });
        let facing = (0..3)
            .map(|axis| geometric[axis] * supplied[axis])
            .sum::<f32>();
        if facing < 0. {
            triangle.swap(1, 2);
        }
    }
}
fn uv(p: &[f32; 3], v: &[[playsrc_bsp::Float32; 4]; 2], w: i32, h: i32) -> [f32; 2] {
    let d = |a: usize| {
        p[0] * v[a][0].value() + p[1] * v[a][1].value() + p[2] * v[a][2].value() + v[a][3].value()
    };
    [d(0) / w.max(1) as f32, d(1) / h.max(1) as f32]
}
fn lightmap_uv(p: &[f32; 3], i: &TextureInfo, f: &Face) -> [f32; 2] {
    let d = |a: usize| {
        p[0] * i.lightmap_vectors[a][0].value()
            + p[1] * i.lightmap_vectors[a][1].value()
            + p[2] * i.lightmap_vectors[a][2].value()
            + i.lightmap_vectors[a][3].value()
    };
    [
        d(0) - f.lightmap_mins[0] as f32,
        d(1) - f.lightmap_mins[1] as f32,
    ]
}
fn triangles_for(
    face: &Face,
    index: usize,
    vertices: usize,
    primitives: &[Primitive],
    primitive_vertices: &[Vector3],
    indices: &[u16],
) -> Result<(Vec<[u32; 3]>, bool), Error> {
    let count = (face.primitive_and_shadow_bits & 0x7fff) as usize;
    if count == 0 {
        return Ok((
            (1..vertices - 1)
                .map(|v| [0, v as u32, v as u32 + 1])
                .collect(),
            false,
        ));
    }
    let first = face.first_primitive as usize;
    let selected = primitives
        .get(first..first + count)
        .ok_or_else(|| error(ErrorCode::InvalidRange, Some(index)))?;
    let mut out = Vec::new();
    for p in selected {
        if p.vertex_count != 0
            || p.first_vertex as usize + p.vertex_count as usize > primitive_vertices.len()
        {
            return Err(error(ErrorCode::UnsupportedPrimitive, Some(index)));
        }
        let s = p.first_index as usize;
        let values = indices
            .get(s..s + p.index_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(index)))?;
        let mut push = |t: [u16; 3]| -> Result<(), Error> {
            if t.iter().any(|v| *v as usize >= vertices) {
                return Err(error(ErrorCode::InvalidReference, Some(index)));
            };
            out.push(t.map(u32::from));
            Ok(())
        };
        match p.kind {
            0 if values.len().is_multiple_of(3) => {
                for t in values.chunks_exact(3) {
                    push([t[0], t[1], t[2]])?
                }
            }
            1 => {
                for at in 0..values.len().saturating_sub(2) {
                    let t = if at % 2 == 0 {
                        [values[at], values[at + 1], values[at + 2]]
                    } else {
                        [values[at + 1], values[at], values[at + 2]]
                    };
                    push(t)?
                }
            }
            _ => return Err(error(ErrorCode::UnsupportedPrimitive, Some(index))),
        }
    }
    Ok((out, true))
}
fn error(code: ErrorCode, item: Option<usize>) -> Error {
    Error { code, item }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_bsp::{Edge, Face, Float32, Vector3, Vertex};

    fn scalar(value: f32) -> Float32 {
        Float32(value.to_bits())
    }
    fn vertex(x: f32, y: f32) -> Vertex {
        Vertex {
            position: Vector3 {
                x: scalar(x),
                y: scalar(y),
                z: scalar(0.),
            },
        }
    }
    fn face(first_surface_edge: i32, surface_edge_count: i16) -> Face {
        Face {
            plane_index: 0,
            side: 0,
            on_node: 1,
            first_surface_edge,
            surface_edge_count,
            texture_info_index: 0,
            displacement_info_index: -1,
            surface_fog_volume_id: -1,
            styles: [255; 4],
            light_offset: -1,
            area: scalar(1.),
            lightmap_mins: [0; 2],
            lightmap_size: [0; 2],
            original_face: -1,
            primitive_and_shadow_bits: 0,
            first_primitive: 0,
            smoothing_groups: 0,
        }
    }

    #[test]
    fn signed_surfedges_and_face_side_retain_source_orientation() {
        let vertices = [
            vertex(0., 0.),
            vertex(1., 0.),
            vertex(1., 1.),
            vertex(0., 1.),
        ];
        let edges = [
            Edge {
                vertex_indices: [0, 0],
            },
            Edge {
                vertex_indices: [0, 1],
            },
            Edge {
                vertex_indices: [1, 2],
            },
            Edge {
                vertex_indices: [2, 3],
            },
            Edge {
                vertex_indices: [3, 0],
            },
        ];
        let surfedges = [1, 2, 3, 4, -4, -3, -2, -1];
        let forward = face_positions(&face(0, 4), 0, &vertices, &edges, &surfedges).unwrap();
        let reverse = face_positions(&face(4, 4), 0, &vertices, &edges, &surfedges).unwrap();
        assert_eq!(
            forward,
            vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
        );
        assert_eq!(
            reverse,
            vec![[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]]
        );
        assert_eq!(oriented_plane_normal([0., 0., 1.], 0), [0., 0., 1.]);
        assert_eq!(oriented_plane_normal([0., 0., 1.], 1), [0., 0., -1.]);
    }

    #[test]
    fn triangles_face_their_supplied_normals_without_changing_vertices() {
        let positions = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];
        let normals = [[0., 0., -1.]; 4];
        let mut triangles = [[0, 1, 2], [0, 2, 3]];
        normalize_triangle_winding(&positions, &normals, &mut triangles);
        assert_eq!(triangles, [[0, 2, 1], [0, 3, 2]]);
        normalize_triangle_winding(&positions, &normals, &mut triangles);
        assert_eq!(triangles, [[0, 2, 1], [0, 3, 2]]);
    }
}
