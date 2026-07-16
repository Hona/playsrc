use playsrc_bsp::{Bsp, Face, LumpData, Primitive, TextureData, TextureInfo, Vector3};
use sha2::{Digest, Sha256};
use std::fmt;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingProfile {
    Ldr,
    Hdr,
}
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
    pub lighting_samples: Vec<[u8; 4]>,
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
    pub payload: Vec<u8>,
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
    let lighting = match &bsp.lumps[if profile == LightingProfile::Hdr {
        53
    } else {
        8
    }]
    .records
    {
        LumpData::Lighting(v) => v,
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
        let (indices, compiled) = triangles_for(
            face,
            face_index,
            positions.len(),
            primitives,
            primitive_vertices,
            primitive_indices,
        )?;
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
        lighting_samples: lighting
            .iter()
            .map(|v| [v.red, v.green, v.blue, v.exponent as u8])
            .collect(),
        world_model: 0,
        triangle_count: triangles,
        vertex_count: output_vertices,
    })
}

pub fn compile_runtime(
    bsp: &Bsp,
    bsp_sha256: [u8; 32],
    profile: LightingProfile,
    compiler_identity: &str,
    configuration: &[u8],
) -> Result<Runtime, Error> {
    let map = compile(bsp, profile)?;
    let collision =
        playsrc_collision::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let visibility =
        playsrc_visibility::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let entities =
        playsrc_entity::parse(bsp.lumps[0].bytes(bsp), playsrc_entity::Limits::default())
            .map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let payload = serialize(&map, &entities);
    let payload_sha256 = Sha256::digest(&payload).into();
    let configuration_sha256 = Sha256::digest(configuration).into();
    let descriptor = RuntimeDescriptor {
        schema: 1,
        bsp_sha256,
        compiler_identity: compiler_identity.to_owned(),
        configuration_sha256,
        payload_sha256,
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
fn serialize(map: &CanonicalMap, entities: &playsrc_entity::Graph) -> Vec<u8> {
    let mut out = b"PSMP".to_vec();
    u32v(&mut out, 1);
    u32v(&mut out, map.bsp_version as u32);
    u32v(&mut out, map.map_revision as u32);
    out.push(match map.lighting_profile {
        LightingProfile::Ldr => 0,
        LightingProfile::Hdr => 1,
    });
    u32v(&mut out, map.materials.len() as u32);
    u32v(&mut out, map.surfaces.len() as u32);
    u32v(&mut out, map.lighting_samples.len() as u32);
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
    for sample in &map.lighting_samples {
        out.extend_from_slice(sample)
    }
    bytesv(&mut out, &entities.source);
    out
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
    let sign = if face.side == 0 { 1. } else { -1. };
    Ok(vec![
        [
            p.normal.x.value() * sign,
            p.normal.y.value() * sign,
            p.normal.z.value() * sign
        ];
        positions.len()
    ])
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
