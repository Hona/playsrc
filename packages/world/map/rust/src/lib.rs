use playsrc_bsp::{Bsp, Face, LumpData, Model, Primitive, TextureData, TextureInfo, Vector3};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt, ops::Range};
mod lighting;
pub use lighting::*;
mod environment;
pub use environment::*;
pub mod pixel_visibility;
pub mod legacy_glow;
pub mod legacy_sun;
pub mod legacy_spotlight;
pub mod legacy_sprite;
pub mod render_fx;
mod displacement;
pub use displacement::{CollisionDisplacement, DisplacementSurface};
mod area_portals;
pub use area_portals::compile_area_portal_state;
mod model_lighting;
mod static_props;
mod surface_lighting;
mod serialization;
pub use model_lighting::*;
pub use static_props::{StaticPropModel, StaticPropOccurrence, StaticProps};
pub use surface_lighting::*;
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialReference {
    pub index: usize,
    pub name: String,
    pub logical_path: String,
    pub width: i32,
    pub height: i32,
    pub reflectivity: [f32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub face: usize,
    pub model: usize,
    pub material: usize,
    pub texture_info: usize,
    pub flags: i32,
    pub draw: bool,
    pub plane: [f32; 4],
    pub plane_back: bool,
    pub texture_vectors: [[f32; 4]; 2],
    pub lightmap_vectors: [[f32; 4]; 2],
    pub lightmap_mins: [i32; 2],
    pub texture_size: [i32; 2],
    pub uv_origin: TextureCoordinateOrigin,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub alpha: Vec<f32>,
    pub uv: Vec<[f32; 2]>,
    pub lightmap_uv: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub light_offset: i32,
    pub light_styles: [u8; 4],
    pub lightmap_size: [i32; 2],
    pub compiled_primitives: bool,
    pub displacement: Option<DisplacementSurface>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrushModelIdentity {
    World,
    Inline(usize),
}
#[derive(Clone, Debug, PartialEq)]
pub struct BrushModelGeometry {
    pub index: usize,
    pub identity: BrushModelIdentity,
    pub bounds: [[f32; 3]; 2],
    pub origin: [f32; 3],
    pub head_node: i32,
    pub surface_range: Range<usize>,
    pub materials: Vec<usize>,
    pub entities: Vec<usize>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub collision_brushes: Vec<usize>,
    pub collision_contents: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BrushModelOccurrence {
    pub entity: usize,
    pub model: usize,
    pub classname: Vec<u8>,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub parent_name: Option<Vec<u8>>,
    pub spawn_flags: Option<Vec<u8>>,
    pub start_disabled: Option<Vec<u8>>,
    pub solidity: Option<Vec<u8>>,
    pub solid_bsp: Option<Vec<u8>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureCoordinateOrigin {
    TopLeft,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalMap {
    pub bsp_version: i32,
    pub map_revision: i32,
    pub lighting_profile: LightingProfile,
    pub materials: Vec<MaterialReference>,
    pub surfaces: Vec<Surface>,
    pub brush_models: Vec<BrushModelGeometry>,
    pub brush_model_occurrences: Vec<BrushModelOccurrence>,
    pub static_props: StaticProps,
    pub collision_displacements: Vec<CollisionDisplacement>,
    pub collision_world_identity: [u8; 32],
    pub lighting: LightingData,
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
    pub payload_bytes: usize,
    /// Empty only when assembly requested identity-only serialization.
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTexture {
    pub logical_path: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeMaterial {
    pub logical_path: String,
    pub shader: u8,
    pub features: u8,
    pub texture_role: u8,
    pub base_texture: Option<RuntimeTexture>,
    pub second_texture: Option<RuntimeTexture>,
    pub detail: Option<RuntimeDetail>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDetail {
    pub texture: RuntimeTexture,
    pub scale: [f32; 2],
    pub blend_mode: i32,
    pub blend_factor: f32,
    pub tint: [f32; 3],
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
    pub bind_positions: Vec<[f32; 3]>,
    pub bind_normals: Vec<[f32; 3]>,
    pub bind_tangents: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u16; 4]>,
    pub bone_weights: Vec<[f32; 4]>,
    pub bone_palette: Vec<u16>,
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
    /// Hash every serialized byte in either mode. Cache-hit callers need the
    /// independent identity, but must not retain an unused second payload.
    pub retain_payload: bool,
    pub compiler_identity: &'a str,
    pub configuration_sha256: [u8; 32],
    pub materials: &'a [RuntimeMaterial],
    pub profile_materials: &'a [RuntimeProfileMaterial],
    pub inputs: &'a [RuntimeInput],
    pub output_role: &'a str,
    pub models: &'a [RuntimeModel],
    pub model_occurrences: &'a [RuntimeModelOccurrence],
}
struct SerializationContext<'a> {
    map: &'a CanonicalMap,
    entities: &'a playsrc_entity::Graph,
    materials: &'a [RuntimeMaterial],
    profile_materials: &'a [RuntimeProfileMaterial],
    inputs: &'a [RuntimeInput],
    compiler_identity: &'a str,
    bsp_sha256: [u8; 32],
    configuration_sha256: [u8; 32],
    output_role: &'a str,
    models: &'a [RuntimeModel],
    occurrences: &'a [RuntimeModelOccurrence],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidReference,
    InvalidRange,
    UnsupportedPrimitive,
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
    let entities =
        playsrc_entity::parse(bsp.lumps[0].bytes(bsp), playsrc_entity::Limits::default())
            .map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let collision =
        playsrc_collision::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    compile_prepared(bsp, profile, &entities, &collision)
}

pub fn compile_prepared(
    bsp: &Bsp,
    profile: LightingProfile,
    entities: &playsrc_entity::Graph,
    collision: &playsrc_collision::World,
) -> Result<CanonicalMap, Error> {
    if entities.source != bsp.lumps[0].bytes(bsp) {
        return Err(error(ErrorCode::InvalidReference, Some(0)));
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
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
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
    let node_count = match &bsp.lumps[5].records {
        LumpData::Nodes(v) => v.len(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaf_count = match &bsp.lumps[10].records {
        LumpData::Leaves(v) => v.len(),
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
    let displacement_info = match &bsp.lumps[26].records {
        LumpData::DispInfo(v) => v.as_slice(),
        LumpData::Opaque if bsp.lumps[26].bytes(bsp).is_empty() => &[],
        _ => return Err(error(ErrorCode::MissingLump, Some(26))),
    };
    let displacement_vertices = match &bsp.lumps[33].records {
        LumpData::DispVertices(v) => v.as_slice(),
        LumpData::Opaque if bsp.lumps[33].bytes(bsp).is_empty() => &[],
        _ => return Err(error(ErrorCode::MissingLump, Some(33))),
    };
    let displacement_triangles = match &bsp.lumps[48].records {
        LumpData::DispTriangles(v) => v.as_slice(),
        LumpData::Opaque if bsp.lumps[48].bytes(bsp).is_empty() => &[],
        _ => return Err(error(ErrorCode::MissingLump, Some(48))),
    };
    let materials = materials(texdata, offsets, strings)?;
    let lighting = compile_lighting(bsp, profile, faces, texinfo, LightingLimits::default())?;
    let static_props = static_props::compile(bsp)?;
    let entity_models = entities
        .entities
        .iter()
        .filter_map(|entity| entity.bsp_model_index.map(|model| (entity.index, model)))
        .collect::<Vec<_>>();
    let (mut brush_models, face_models) =
        brush_model_layout(models, node_count, leaf_count, faces.len(), &entity_models)?;
    fill_brush_model_collision(&mut brush_models, collision)?;
    let brush_model_occurrences = brush_model_occurrences(entities, brush_models.len())?;
    let mut output = Vec::with_capacity(faces.len());
    let mut normal_cursor = 0usize;
    let mut triangles = 0usize;
    let mut output_vertices = 0usize;
    let mut collision_displacements = Vec::with_capacity(displacement_info.len());
    for (face_index, face) in faces.iter().enumerate() {
        let positions = face_positions(face, face_index, vertices, edges, surfedges)?;
        if positions.len() < 3 {
            return Err(error(ErrorCode::InvalidRange, Some(face_index)));
        }
        let texture_info = usize::try_from(face.texture_info_index)
            .map_err(|_| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let info = texinfo
            .get(texture_info)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let material = usize::try_from(info.texture_data_index)
            .map_err(|_| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let material_info = materials
            .get(material)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let plane = planes
            .get(face.plane_index as usize)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(face_index)))?;
        let stored_plane = [
            plane.normal.x.value(),
            plane.normal.y.value(),
            plane.normal.z.value(),
            plane.distance.value(),
        ];
        let texture_vectors = std::array::from_fn(|axis| {
            std::array::from_fn(|component| info.texture_vectors[axis][component].value())
        });
        let lightmap_vectors = std::array::from_fn(|axis| {
            std::array::from_fn(|component| info.lightmap_vectors[axis][component].value())
        });
        if stored_plane
            .iter()
            .chain(texture_vectors.iter().flatten())
            .chain(lightmap_vectors.iter().flatten())
            .any(|value| !value.is_finite())
        {
            return Err(error(ErrorCode::NonFinite, Some(face_index)));
        }
        let mut displacement = (face.displacement_info_index >= 0)
            .then(|| {
                displacement::compile(displacement::Inputs {
                    face_index,
                    face,
                    corners: &positions,
                    info,
                    material: &texdata[material],
                    displacements: displacement_info,
                    vertices: displacement_vertices,
                    triangle_tags: displacement_triangles,
                })
            })
            .transpose()?;
        let face_normals = if let Some(displacement) = &mut displacement {
            std::mem::take(&mut displacement.normals)
        } else {
            face_normals(
                face,
                face_index,
                &positions,
                normals,
                normal_indices,
                normal_cursor,
                bsp,
            )?
        };
        normal_cursor = normal_cursor
            .checked_add(face.surface_edge_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(face_index)))?;
        let (positions, uv, lightmap_uv, alpha, indices, compiled, displacement_descriptor) =
            if let Some(displacement) = displacement {
                collision_displacements.push(displacement.collision);
                (
                    displacement.positions,
                    displacement.uv,
                    displacement.lightmap_uv,
                    displacement.alpha,
                    displacement.triangles,
                    false,
                    Some(displacement.descriptor),
                )
            } else {
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
                let alpha = vec![0.0; positions.len()];
                (positions, uv, lightmap_uv, alpha, indices, compiled, None)
            };
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
            texture_info,
            flags,
            draw,
            plane: stored_plane,
            plane_back: face.side != 0,
            texture_vectors,
            lightmap_vectors,
            lightmap_mins: face.lightmap_mins,
            texture_size: [material_info.width, material_info.height],
            uv_origin: TextureCoordinateOrigin::TopLeft,
            positions,
            normals: face_normals,
            alpha,
            uv,
            lightmap_uv,
            triangles: indices,
            light_offset: face.light_offset,
            light_styles: face.styles,
            lightmap_size: face.lightmap_size,
            compiled_primitives: compiled,
            displacement: displacement_descriptor,
        });
    }
    let face_materials = output
        .iter()
        .map(|surface| surface.material)
        .collect::<Vec<_>>();
    let face_vertices = output
        .iter()
        .map(|surface| surface.positions.len())
        .collect::<Vec<_>>();
    let face_triangles = output
        .iter()
        .map(|surface| surface.triangles.len())
        .collect::<Vec<_>>();
    fill_brush_model_associations(
        &mut brush_models,
        &face_materials,
        &face_vertices,
        &face_triangles,
    )?;
    Ok(CanonicalMap {
        bsp_version: bsp.container_version,
        map_revision: bsp.map_revision,
        lighting_profile: profile,
        materials,
        surfaces: output,
        brush_models,
        brush_model_occurrences,
        static_props,
        collision_displacements,
        collision_world_identity: collision.identity,
        lighting,
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
    let entities =
        playsrc_entity::parse(bsp.lumps[0].bytes(bsp), playsrc_entity::Limits::default())
            .map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let collision =
        playsrc_collision::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    let map = compile_prepared(bsp, profile, &entities, &collision)?;
    let visibility =
        playsrc_visibility::compile(bsp).map_err(|_| error(ErrorCode::InvalidReference, None))?;
    assemble_runtime(map, entities, collision, visibility, bsp_sha256, assembly)
}

pub fn attach_displacement_visibility(
    map: &CanonicalMap,
    visibility: &playsrc_visibility::World,
) -> Result<playsrc_visibility::World, Error> {
    let inputs = map
        .surfaces
        .iter()
        .filter(|surface| surface.displacement.is_some())
        .map(|surface| {
            let mut minimum = [f32::INFINITY; 3];
            let mut maximum = [f32::NEG_INFINITY; 3];
            for position in &surface.positions {
                for axis in 0..3 {
                    minimum[axis] = minimum[axis].min(position[axis]);
                    maximum[axis] = maximum[axis].max(position[axis]);
                }
            }
            Ok((
                u16::try_from(surface.face)
                    .map_err(|_| error(ErrorCode::BoundExceeded, Some(surface.face)))?,
                playsrc_visibility::Aabb::new(minimum, maximum)
                    .map_err(|_| error(ErrorCode::InvalidRange, Some(surface.face)))?,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    visibility
        .with_displacement_surfaces(&inputs)
        .map_err(|_| error(ErrorCode::InvalidReference, None))
}

pub fn assemble_runtime(
    map: CanonicalMap,
    entities: playsrc_entity::Graph,
    collision: playsrc_collision::World,
    visibility: playsrc_visibility::World,
    bsp_sha256: [u8; 32],
    assembly: RuntimeAssembly<'_>,
) -> Result<Runtime, Error> {
    let visibility = attach_displacement_visibility(&map, &visibility)?;
    assemble_prepared_runtime(map, entities, collision, visibility, bsp_sha256, assembly)
}

pub fn assemble_prepared_runtime(
    map: CanonicalMap,
    entities: playsrc_entity::Graph,
    collision: playsrc_collision::World,
    visibility: playsrc_visibility::World,
    bsp_sha256: [u8; 32],
    assembly: RuntimeAssembly<'_>,
) -> Result<Runtime, Error> {
    let RuntimeAssembly {
        retain_payload,
        compiler_identity,
        configuration_sha256,
        materials: resolved_materials,
        profile_materials,
        inputs,
        output_role,
        models: runtime_models,
        model_occurrences,
    } = assembly;
    let profile = map.lighting_profile;
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
                || primitive.positions.len() != primitive.bind_positions.len()
                || primitive.positions.len() != primitive.bind_normals.len()
                || primitive.positions.len() != primitive.bind_tangents.len()
                || primitive.positions.len() != primitive.bone_indices.len()
                || primitive.positions.len() != primitive.bone_weights.len()
                || primitive.bone_palette.len() > 256
                || primitive
                    .bone_palette
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || primitive
                    .bone_indices
                    .iter()
                    .zip(&primitive.bone_weights)
                    .any(|(indices, weights)| {
                        weights
                            .iter()
                            .any(|weight| !weight.is_finite() || *weight < 0.0)
                            || weights.iter().enumerate().any(|(index, weight)| {
                                *weight != 0.0
                                    && primitive
                                        .bone_palette
                                        .binary_search(&indices[index])
                                        .is_err()
                            })
                    })
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
    let serialization = SerializationContext {
        map: &map,
        entities: &entities,
        materials: resolved_materials,
        profile_materials,
        inputs,
        compiler_identity,
        bsp_sha256,
        configuration_sha256,
        output_role,
        models: runtime_models,
        occurrences: model_occurrences,
    };
    let schema = runtime_schema(&map, resolved_materials, runtime_models);
    let payload_bytes = serialized_length(&serialization, schema)?;
    if payload_bytes > 512 * 1024 * 1024 {
        return Err(error(ErrorCode::BoundExceeded, None));
    }
    let (payload, payload_sha256) = if retain_payload {
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(payload_bytes)
            .map_err(|_| error(ErrorCode::BoundExceeded, None))?;
        serialize(&serialization, schema, payload_bytes, &mut payload)?;
        let hash = Sha256::digest(&payload).into();
        (payload, hash)
    } else {
        let mut sink = serialization::HashSink::new()
            .map_err(|_| error(ErrorCode::BoundExceeded, None))?;
        serialize(&serialization, schema, payload_bytes, &mut sink)?;
        (Vec::new(), sink.finish())
    };
    let derived_sha256 = derived_identity(&serialization, payload_sha256);
    let descriptor = RuntimeDescriptor {
        schema: 1,
        bsp_sha256,
        compiler_identity: compiler_identity.to_owned(),
        configuration_sha256,
        payload_sha256,
        derived_sha256,
        payload_bytes,
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
    context: &SerializationContext<'_>,
    schema: u32,
    expected: usize,
    mut out: &mut impl serialization::ByteSink,
) -> Result<(), Error> {
    let map = context.map;
    let entities = context.entities;
    let materials = context.materials;
    let models = context.models;
    let occurrences = context.occurrences;
    out.extend_from_slice(b"PSMP");
    u32v(&mut out, schema);
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
        serialization::f32_plane(&mut out, &s.positions);
        serialization::f32_plane(&mut out, &s.normals);
        serialization::f32_plane(&mut out, &s.uv);
        serialization::f32_plane(&mut out, &s.lightmap_uv);
        if matches!(schema, 8 | 9 | 10 | 11) {
            for alpha in &s.alpha {
                f32v(&mut out, *alpha);
            }
        }
        serialization::u32_plane(&mut out, &s.triangles);
        i32v(&mut out, s.light_offset);
        out.extend_from_slice(&s.light_styles);
        i32v(&mut out, s.lightmap_size[0]);
        i32v(&mut out, s.lightmap_size[1]);
        if matches!(schema, 8 | 9 | 10 | 11) {
            if let Some(displacement) = &s.displacement {
                out.push(1);
                out.push(displacement.power);
                out.extend_from_slice(&[0; 2]);
                u32v(&mut out, displacement.source as u32);
                i32v(&mut out, displacement.minimum_tessellation);
                f32v(&mut out, displacement.smoothing_angle);
                u32v(&mut out, displacement.contents);
                out.extend_from_slice(&displacement.map_face.to_le_bytes());
                out.extend_from_slice(&[0; 2]);
                for value in displacement.start_position {
                    f32v(&mut out, value);
                }
                u32v(&mut out, displacement.vertex_start);
                u32v(&mut out, displacement.triangle_start);
                for value in displacement.allowed_vertices {
                    u32v(&mut out, value);
                }
                for edge in displacement.edge_neighbors {
                    for neighbor in edge.sub_neighbors {
                        out.extend_from_slice(&neighbor.neighbor.to_le_bytes());
                        out.extend_from_slice(&[
                            neighbor.orientation,
                            neighbor.span,
                            neighbor.neighbor_span,
                            neighbor.padding,
                        ]);
                    }
                }
                for corner in displacement.corner_neighbors {
                    for neighbor in corner.neighbors {
                        out.extend_from_slice(&neighbor.to_le_bytes());
                    }
                    out.extend_from_slice(&[corner.neighbor_count, corner.padding]);
                }
                u32v(&mut out, displacement.triangle_tags.len() as u32);
                for value in &displacement.triangle_tags {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            } else {
                out.extend_from_slice(&[0; 4]);
            }
        }
    }
    match &map.lighting.samples {
        LightingSamples::RgbExp32(samples) => {
            for sample in samples {
                out.extend_from_slice(sample)
            }
        }
        LightingSamples::LinearRgb32(samples) => {
            serialization::f32_plane(&mut out, samples);
        }
    }
    bytesv(&mut out, &entities.source);
    if !materials.is_empty() || map.lighting_profile == LightingProfile::Hdr {
        u32v(&mut out, materials.len() as u32);
        for material in materials {
            materialv(&mut out, material, matches!(schema, 8 | 9 | 10 | 11));
        }
    }
    if !models.is_empty() || map.lighting_profile == LightingProfile::Hdr {
        model_registry(&mut out, models, matches!(schema, 8 | 9 | 10 | 11));
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
        serialize_hdr(&mut out, context);
    }
    if out.len() != expected {
        return Err(error(ErrorCode::InvalidRange, None));
    }
    Ok(())
}

/// Authored model geometry shared by map admission and incremental model-only
/// admission. This does not construct a map, collision world, or game session.
pub fn serialize_model_registry(out: &mut Vec<u8>, models: &[RuntimeModel], modern_materials: bool) {
    model_registry(out, models, modern_materials);
}

fn model_registry(
    out: &mut impl serialization::ByteSink,
    models: &[RuntimeModel],
    modern_materials: bool,
) {
    u32v(out, models.len() as u32);
    for model in models {
        bytesv(out, model.logical_path.as_bytes());
        u32v(out, model.materials.len() as u32);
        for material in &model.materials {
            bytesv(out, material.logical_path.as_bytes());
            materialv(out, material, modern_materials);
        }
        u32v(out, model.primitives.len() as u32);
        for primitive in &model.primitives {
            u32v(out, primitive.material as u32);
            u32v(out, primitive.positions.len() as u32);
            u32v(out, primitive.triangles.len() as u32);
            for vectors in [&primitive.positions, &primitive.normals, &primitive.bind_positions, &primitive.bind_normals] {
                serialization::f32_plane(out, vectors);
            }
            serialization::f32_plane(out, &primitive.bind_tangents);
            serialization::u16_plane(out, &primitive.bone_indices);
            serialization::f32_plane(out, &primitive.bone_weights);
            u32v(out, primitive.bone_palette.len() as u32);
            for index in &primitive.bone_palette { out.extend_from_slice(&index.to_le_bytes()); }
            serialization::f32_plane(out, &primitive.uv);
            serialization::u32_plane(out, &primitive.triangles);
        }
    }
}

fn runtime_schema(
    map: &CanonicalMap,
    materials: &[RuntimeMaterial],
    models: &[RuntimeModel],
) -> u32 {
    let displaced = map
        .surfaces
        .iter()
        .any(|surface| surface.displacement.is_some());
    if map.lighting_profile == LightingProfile::Hdr {
        if !models.is_empty() {
            10
        } else if displaced {
            8
        } else {
            7
        }
    } else if displaced && !models.is_empty() {
        11
    } else if !models.is_empty() {
        11
    } else if !materials.is_empty() {
        2
    } else {
        1
    }
}

#[derive(Default)]
struct SerializationLength {
    bytes: usize,
}

impl SerializationLength {
    fn add(&mut self, bytes: usize) -> Result<(), Error> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| error(ErrorCode::BoundExceeded, None))?;
        Ok(())
    }

    fn records(&mut self, count: usize, stride: usize) -> Result<(), Error> {
        self.add(
            count
                .checked_mul(stride)
                .ok_or_else(|| error(ErrorCode::BoundExceeded, None))?,
        )
    }

    fn field(&mut self, bytes: usize) -> Result<(), Error> {
        self.add(4)?;
        self.add(bytes)
    }
}

fn serialized_length(context: &SerializationContext<'_>, schema: u32) -> Result<usize, Error> {
    let map = context.map;
    let mut length = SerializationLength::default();
    length.add(33)?;
    for material in &map.materials {
        length.field(material.logical_path.len())?;
        length.add(8)?;
    }
    for surface in &map.surfaces {
        length.add(41)?;
        length.records(surface.positions.len(), 12)?;
        length.records(surface.normals.len(), 12)?;
        length.records(surface.uv.len(), 8)?;
        length.records(surface.lightmap_uv.len(), 8)?;
        if matches!(schema, 8 | 9 | 10 | 11) {
            length.records(surface.alpha.len(), 4)?;
        }
        length.records(surface.triangles.len(), 12)?;
        if matches!(schema, 8 | 9 | 10 | 11) {
            if let Some(displacement) = &surface.displacement {
                length.add(176)?;
                length.records(displacement.triangle_tags.len(), 2)?;
            } else {
                length.add(4)?;
            }
        }
    }
    match &map.lighting.samples {
        LightingSamples::RgbExp32(samples) => length.records(samples.len(), 4)?,
        LightingSamples::LinearRgb32(samples) => length.records(samples.len(), 12)?,
    }
    length.field(context.entities.source.len())?;
    if !context.materials.is_empty() || map.lighting_profile == LightingProfile::Hdr {
        length.add(4)?;
        for material in context.materials {
            add_material_length(&mut length, material, matches!(schema, 8 | 9 | 10 | 11))?;
        }
    }
    if !context.models.is_empty() || map.lighting_profile == LightingProfile::Hdr {
        length.add(4)?;
        for model in context.models {
            length.field(model.logical_path.len())?;
            length.add(4)?;
            for material in &model.materials {
                length.field(material.logical_path.len())?;
                add_material_length(&mut length, material, matches!(schema, 8 | 9 | 10 | 11))?;
            }
            length.add(4)?;
            for primitive in &model.primitives {
                length.add(12)?;
                length.records(primitive.positions.len(), 12)?;
                length.records(primitive.normals.len(), 12)?;
                length.records(primitive.bind_positions.len(), 12)?;
                length.records(primitive.bind_normals.len(), 12)?;
                length.records(primitive.bind_tangents.len(), 16)?;
                length.records(primitive.bone_indices.len(), 8)?;
                length.records(primitive.bone_weights.len(), 16)?;
                length.add(4)?;
                length.records(primitive.bone_palette.len(), 2)?;
                length.records(primitive.uv.len(), 8)?;
                length.records(primitive.triangles.len(), 12)?;
            }
        }
        length.add(4)?;
        length.records(context.occurrences.len(), 32)?;
    }
    if map.lighting_profile == LightingProfile::Hdr {
        add_hdr_length(&mut length, context)?;
    }
    Ok(length.bytes)
}

fn add_texture_length(
    length: &mut SerializationLength,
    texture: &RuntimeTexture,
) -> Result<(), Error> {
    length.field(texture.logical_path.len())?;
    length.add(8)
}

fn add_material_length(
    length: &mut SerializationLength,
    material: &RuntimeMaterial,
    include_detail: bool,
) -> Result<(), Error> {
    length.add(4)?;
    if let Some(texture) = &material.base_texture {
        add_texture_length(length, texture)?;
    }
    if include_detail {
        length.add(4)?;
        if let Some(texture) = &material.second_texture {
            add_texture_length(length, texture)?;
        }
        length.add(4)?;
        if let Some(detail) = &material.detail {
            add_texture_length(length, &detail.texture)?;
            length.add(28)?;
        }
    }
    Ok(())
}

fn add_hdr_length(
    length: &mut SerializationLength,
    context: &SerializationContext<'_>,
) -> Result<(), Error> {
    let lighting = &context.map.lighting;
    length.add(12)?;
    length.field(context.output_role.len())?;
    length.field(context.compiler_identity.len())?;
    length.add(96)?;
    length.add(4)?;
    for member in &lighting.members {
        length.add(
            if matches!(member.source, Some(LightingSource::GameLump { .. })) {
                89
            } else {
                85
            },
        )?;
    }
    length.add(12)?;
    length.records(lighting.surfaces.len(), 20)?;
    length.add(4)?;
    length.records(lighting.world_lights.len(), 88)?;
    length.add(4)?;
    length.records(lighting.ambient_indexes.len(), 4)?;
    length.add(4)?;
    length.records(lighting.ambient_samples.len(), 76)?;
    length.add(16)?;
    length.add(4)?;
    for material in context.profile_materials {
        length.field(material.logical_path.len())?;
        length.add(4)?;
        length.field(material.texture.logical_path.len())?;
        length.add(44)?;
        length.field(material.texture.source_bytes.len())?;
    }
    length.add(4)?;
    for input in context.inputs {
        length.add(4)?;
        length.field(input.logical_path.len())?;
        length.add(32)?;
    }
    Ok(())
}

fn materialv(out: &mut impl serialization::ByteSink, material: &RuntimeMaterial, include_detail: bool) {
    out.push(material.shader);
    out.push(material.features);
    out.push(u8::from(material.base_texture.is_some()));
    out.push(material.texture_role);
    if let Some(texture) = &material.base_texture {
        bytesv(out, texture.logical_path.as_bytes());
        u32v(out, texture.width);
        u32v(out, texture.height);
    }
    if include_detail {
        out.push(u8::from(material.second_texture.is_some()));
        out.extend_from_slice(&[0; 3]);
        if let Some(texture) = &material.second_texture {
            bytesv(out, texture.logical_path.as_bytes());
            u32v(out, texture.width);
            u32v(out, texture.height);
        }
        out.push(u8::from(material.detail.is_some()));
        out.extend_from_slice(&[0; 3]);
        if let Some(detail) = &material.detail {
            bytesv(out, detail.texture.logical_path.as_bytes());
            u32v(out, detail.texture.width);
            u32v(out, detail.texture.height);
            for value in detail.scale {
                f32v(out, value);
            }
            i32v(out, detail.blend_mode);
            f32v(out, detail.blend_factor);
            for value in detail.tint {
                f32v(out, value);
            }
        }
    }
}
fn lighting_sample_count(samples: &LightingSamples) -> usize {
    match samples {
        LightingSamples::RgbExp32(samples) => samples.len(),
        LightingSamples::LinearRgb32(samples) => samples.len(),
    }
}
fn serialize_hdr(out: &mut impl serialization::ByteSink, context: &SerializationContext<'_>) {
    let map = context.map;
    let profile_materials = context.profile_materials;
    let inputs = context.inputs;
    let compiler_identity = context.compiler_identity;
    let bsp_sha256 = context.bsp_sha256;
    let configuration_sha256 = context.configuration_sha256;
    let output_role = context.output_role;
    let materials = context.materials;
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
    let valid_texture = |texture: &RuntimeTexture| {
        !texture.logical_path.is_empty()
            && texture.width > 0
            && texture.height > 0
            && usize::try_from(texture.width)
                .ok()
                .and_then(|width| {
                    usize::try_from(texture.height)
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .is_some()
    };
    if material
        .base_texture
        .as_ref()
        .is_some_and(|texture| !valid_texture(texture))
        || material
            .second_texture
            .as_ref()
            .is_some_and(|texture| !valid_texture(texture))
    {
        return Err(error(ErrorCode::InvalidMaterial, Some(item)));
    }
    if let Some(detail) = &material.detail
        && (!valid_texture(&detail.texture)
            || detail
                .scale
                .iter()
                .chain([detail.blend_factor].iter())
                .any(|value| !value.is_finite())
            || !(0..=11).contains(&detail.blend_mode)
            || detail.tint.iter().any(|value| !value.is_finite()))
    {
        return Err(error(ErrorCode::InvalidMaterial, Some(item)));
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
fn derived_identity(context: &SerializationContext<'_>, payload_sha256: [u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-derived-map-v1");
    digest.update([match context.map.lighting_profile {
        LightingProfile::Ldr => 0,
        LightingProfile::Hdr => 1,
    }]);
    digest.update((context.output_role.len() as u32).to_le_bytes());
    digest.update(context.output_role.as_bytes());
    digest.update((context.compiler_identity.len() as u32).to_le_bytes());
    digest.update(context.compiler_identity.as_bytes());
    digest.update(context.bsp_sha256);
    digest.update(context.configuration_sha256);
    digest.update(context.map.lighting.closure_sha256);
    let mut inputs: Vec<_> = context.inputs.iter().collect();
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
fn u32v(o: &mut impl serialization::ByteSink, v: u32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn i32v(o: &mut impl serialization::ByteSink, v: i32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn f32v(o: &mut impl serialization::ByteSink, v: f32) {
    u32v(o, v.to_bits())
}
fn bytesv(o: &mut impl serialization::ByteSink, v: &[u8]) {
    u32v(o, v.len() as u32);
    o.extend_from_slice(v)
}
fn brush_model_layout(
    source: &[Model],
    node_count: usize,
    leaf_count: usize,
    face_count: usize,
    entity_models: &[(usize, usize)],
) -> Result<(Vec<BrushModelGeometry>, Vec<usize>), Error> {
    if source.is_empty() {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    let mut face_models = vec![usize::MAX; face_count];
    let mut output = Vec::with_capacity(source.len());
    for (index, model) in source.iter().enumerate() {
        let mins = vector(model.mins);
        let maxs = vector(model.maxs);
        let origin = vector(model.origin);
        if mins
            .iter()
            .chain(maxs.iter())
            .chain(origin.iter())
            .any(|value| !value.is_finite())
        {
            return Err(error(ErrorCode::NonFinite, Some(index)));
        }
        if (0..3).any(|axis| mins[axis] > maxs[axis]) {
            return Err(error(ErrorCode::InvalidRange, Some(index)));
        }
        let valid_head_node = if model.head_node >= 0 {
            (model.head_node as usize) < node_count
        } else {
            model
                .head_node
                .checked_neg()
                .and_then(|value| value.checked_sub(1))
                .and_then(|value| usize::try_from(value).ok())
                .is_some_and(|leaf| leaf < leaf_count)
        };
        if !valid_head_node {
            return Err(error(ErrorCode::InvalidReference, Some(index)));
        }
        let start = usize::try_from(model.first_face)
            .map_err(|_| error(ErrorCode::InvalidRange, Some(index)))?;
        let count = usize::try_from(model.face_count)
            .map_err(|_| error(ErrorCode::InvalidRange, Some(index)))?;
        let end = start
            .checked_add(count)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(index)))?;
        if end > face_count
            || face_models[start..end]
                .iter()
                .any(|model| *model != usize::MAX)
        {
            return Err(error(ErrorCode::InvalidRange, Some(index)));
        }
        face_models[start..end].fill(index);
        output.push(BrushModelGeometry {
            index,
            identity: if index == 0 {
                BrushModelIdentity::World
            } else {
                BrushModelIdentity::Inline(index)
            },
            bounds: [mins, maxs],
            origin,
            head_node: model.head_node,
            surface_range: start..end,
            materials: Vec::new(),
            entities: Vec::new(),
            vertex_count: 0,
            triangle_count: 0,
            collision_brushes: Vec::new(),
            collision_contents: 0,
        });
    }
    if let Some(face) = face_models.iter().position(|model| *model == usize::MAX) {
        return Err(error(ErrorCode::InvalidRange, Some(face)));
    }
    for &(entity, model) in entity_models {
        let Some(geometry) = output.get_mut(model) else {
            return Err(error(ErrorCode::InvalidReference, Some(entity)));
        };
        geometry.entities.push(entity);
    }
    Ok((output, face_models))
}

fn fill_brush_model_collision(
    models: &mut [BrushModelGeometry],
    collision: &playsrc_collision::World,
) -> Result<(), Error> {
    if models.len() != collision.model_brushes.len()
        || models.len() != collision.model_contents.len()
    {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    for (model, (brushes, contents)) in models.iter_mut().zip(
        collision
            .model_brushes
            .iter()
            .zip(&collision.model_contents),
    ) {
        model.collision_brushes.clone_from(brushes);
        model.collision_contents = *contents;
    }
    Ok(())
}

fn brush_model_occurrences(
    graph: &playsrc_entity::Graph,
    model_count: usize,
) -> Result<Vec<BrushModelOccurrence>, Error> {
    graph
        .entities
        .iter()
        .filter_map(|entity| {
            entity
                .bsp_model_index
                .filter(|model| *model != 0)
                .map(|model| (entity, model))
        })
        .map(|(entity, model)| {
            if model >= model_count {
                return Err(error(ErrorCode::InvalidReference, Some(entity.index)));
            }
            Ok(BrushModelOccurrence {
                entity: entity.index,
                model,
                classname: entity.classname.clone().unwrap_or_default(),
                origin: source_vector(entity, b"origin")?,
                angles: source_vector(entity, b"angles")?,
                parent_name: entity.parentname.clone(),
                spawn_flags: source_bytes(entity, b"spawnflags"),
                start_disabled: source_bytes(entity, b"StartDisabled"),
                solidity: source_bytes(entity, b"Solidity"),
                solid_bsp: source_bytes(entity, b"solidbsp"),
            })
        })
        .collect()
}

fn source_bytes(entity: &playsrc_entity::Entity, key: &[u8]) -> Option<Vec<u8>> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.clone())
}

fn source_vector(entity: &playsrc_entity::Entity, key: &[u8]) -> Result<[f32; 3], Error> {
    let Some(value) = source_bytes(entity, key) else {
        return Ok([0.0; 3]);
    };
    let values = std::str::from_utf8(&value)
        .ok()
        .map(|value| {
            value
                .split_ascii_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .ok()
        .flatten()
        .filter(|values| values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(entity.index)))?;
    Ok([values[0], values[1], values[2]])
}

fn fill_brush_model_associations(
    models: &mut [BrushModelGeometry],
    face_materials: &[usize],
    face_vertices: &[usize],
    face_triangles: &[usize],
) -> Result<(), Error> {
    if face_materials.len() != face_vertices.len() || face_materials.len() != face_triangles.len() {
        return Err(error(ErrorCode::InvalidRange, None));
    }
    for model in models {
        let materials = face_materials
            .get(model.surface_range.clone())
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(model.index)))?;
        let vertices = face_vertices
            .get(model.surface_range.clone())
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(model.index)))?;
        let triangles = face_triangles
            .get(model.surface_range.clone())
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(model.index)))?;
        for &material in materials {
            if !model.materials.contains(&material) {
                model.materials.push(material);
            }
        }
        model.vertex_count = vertices.iter().try_fold(0usize, |total, count| {
            total
                .checked_add(*count)
                .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(model.index)))
        })?;
        model.triangle_count = triangles.iter().try_fold(0usize, |total, count| {
            total
                .checked_add(*count)
                .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(model.index)))
        })?;
    }
    Ok(())
}

fn vector(value: Vector3) -> [f32; 3] {
    [value.x.value(), value.y.value(), value.z.value()]
}
fn materials(
    data: &[TextureData],
    table: &[i32],
    strings: &[u8],
) -> Result<Vec<MaterialReference>, Error> {
    data.iter()
        .enumerate()
        .map(|(index, v)| {
            if v.width <= 0 || v.height <= 0 {
                return Err(error(ErrorCode::InvalidMaterial, Some(index)));
            }
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
                reflectivity: [
                    v.reflectivity.x.value(),
                    v.reflectivity.y.value(),
                    v.reflectivity.z.value(),
                ],
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
    [d(0) / w as f32, d(1) / h as f32]
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
    fn model(
        mins: [f32; 3],
        maxs: [f32; 3],
        origin: [f32; 3],
        head_node: i32,
        first_face: i32,
        face_count: i32,
    ) -> Model {
        let vector = |value: [f32; 3]| Vector3 {
            x: scalar(value[0]),
            y: scalar(value[1]),
            z: scalar(value[2]),
        };
        Model {
            mins: vector(mins),
            maxs: vector(maxs),
            origin: vector(origin),
            head_node,
            first_face,
            face_count,
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
    fn runtime_textures_retain_only_authored_identity_and_dimensions() {
        let texture = RuntimeTexture {
            logical_path: "materials/test.vtf".to_owned(),
            width: 256,
            height: 128,
        };
        assert_eq!(texture.clone(), texture);
        assert_eq!((texture.width, texture.height), (256, 128));
    }

    #[test]
    fn serialization_size_rejects_overflow_before_output_allocation() {
        let mut length = SerializationLength::default();
        length.add(usize::MAX).unwrap();
        assert_eq!(length.add(1).unwrap_err().code, ErrorCode::BoundExceeded);
        let mut length = SerializationLength::default();
        assert_eq!(
            length.records(usize::MAX, 2).unwrap_err().code,
            ErrorCode::BoundExceeded
        );
    }

    #[test]
    fn model_serialization_sinks_preserve_binary32_and_integer_planes() {
        use serialization::ByteSink;
        let vectors = vec![[f32::from_bits(0x8000_0000), f32::from_bits(0x0000_0001), f32::from_bits(0x7fc0_1234)]; 4097];
        let models = [RuntimeModel {
            logical_path: "models/test.mdl".into(),
            materials: vec![],
            primitives: vec![RuntimeModelPrimitive {
                material: 0, positions: vectors.clone(), normals: vectors.clone(),
                bind_positions: vectors.clone(), bind_normals: vectors,
                bind_tangents: vec![[0.0, -0.0, 1.0, -1.0]; 4097],
                bone_indices: vec![[0, 255, 256, u16::MAX]; 4097],
                bone_weights: vec![[0.0, 0.25, 0.5, 0.25]; 4097],
                bone_palette: vec![0, 255, 256, u16::MAX],
                uv: vec![[0.0, -0.0]; 4097], triangles: vec![[0, 1, 4096]],
            }],
        }];
        for modern in [false, true] {
            let mut bytes = Vec::new();
            serialize_model_registry(&mut bytes, &models, modern);
            assert!(bytes.windows(4).any(|value| value == 0x7fc0_1234_u32.to_le_bytes()));
            let mut sink = serialization::HashSink::new().unwrap();
            model_registry(&mut sink, &models, modern);
            assert_eq!(sink.len(), bytes.len());
            assert_eq!(sink.finish(), <[u8; 32]>::from(Sha256::digest(bytes)));
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

    #[test]
    fn brush_models_retain_ranges_bounds_materials_and_duplicate_entity_joins() {
        let source = [
            model([-64.0, -64.0, -16.0], [64.0, 64.0, 16.0], [0.0; 3], 0, 0, 2),
            model(
                [-8.0, -4.0, 0.0],
                [8.0, 4.0, 16.0],
                [128.0, 32.0, 4.0],
                1,
                2,
                2,
            ),
        ];
        let (mut models, owners) =
            brush_model_layout(&source, 2, 1, 4, &[(0, 0), (7, 1), (9, 1)]).unwrap();
        fill_brush_model_associations(&mut models, &[3, 3, 3, 4], &[4, 3, 4, 5], &[2, 1, 2, 3])
            .unwrap();

        assert_eq!(owners, [0, 0, 1, 1]);
        assert_eq!(models[0].identity, BrushModelIdentity::World);
        assert_eq!(models[0].surface_range, 0..2);
        assert_eq!(models[0].materials, [3]);
        assert_eq!(models[0].entities, [0]);
        assert_eq!(models[0].vertex_count, 7);
        assert_eq!(models[0].triangle_count, 3);
        assert_eq!(models[1].identity, BrushModelIdentity::Inline(1));
        assert_eq!(models[1].bounds, [[-8.0, -4.0, 0.0], [8.0, 4.0, 16.0]]);
        assert_eq!(models[1].origin, [128.0, 32.0, 4.0]);
        assert_eq!(models[1].surface_range, 2..4);
        assert_eq!(models[1].materials, [3, 4]);
        assert_eq!(models[1].entities, [7, 9]);
    }

    #[test]
    fn brush_model_layout_rejects_missing_overlap_unowned_and_invalid_references() {
        assert_eq!(
            brush_model_layout(&[], 1, 1, 0, &[]).unwrap_err().code,
            ErrorCode::InvalidReference
        );
        let world = model([0.0; 3], [1.0; 3], [0.0; 3], 0, 0, 2);
        let overlap = model([0.0; 3], [1.0; 3], [0.0; 3], 0, 1, 1);
        assert_eq!(
            brush_model_layout(&[world, overlap], 1, 1, 2, &[])
                .unwrap_err()
                .code,
            ErrorCode::InvalidRange
        );
        assert_eq!(
            brush_model_layout(&[world], 1, 1, 3, &[]).unwrap_err().code,
            ErrorCode::InvalidRange
        );
        assert_eq!(
            brush_model_layout(&[world], 1, 1, 2, &[(4, 1)])
                .unwrap_err()
                .code,
            ErrorCode::InvalidReference
        );
        let bad_node = model([0.0; 3], [1.0; 3], [0.0; 3], 1, 0, 2);
        assert_eq!(
            brush_model_layout(&[bad_node], 1, 1, 2, &[])
                .unwrap_err()
                .code,
            ErrorCode::InvalidReference
        );
        let leaf_model = model([0.0; 3], [1.0; 3], [0.0; 3], -1, 0, 2);
        assert!(brush_model_layout(&[leaf_model], 1, 1, 2, &[]).is_ok());
        let bad_leaf = model([0.0; 3], [1.0; 3], [0.0; 3], -2, 0, 2);
        assert_eq!(
            brush_model_layout(&[bad_leaf], 1, 1, 2, &[])
                .unwrap_err()
                .code,
            ErrorCode::InvalidReference
        );
        let bad_bounds = model([2.0; 3], [1.0; 3], [0.0; 3], 0, 0, 2);
        assert_eq!(
            brush_model_layout(&[bad_bounds], 1, 1, 2, &[])
                .unwrap_err()
                .code,
            ErrorCode::InvalidRange
        );
    }
}
