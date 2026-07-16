use std::ops::Range;

use crate::{ErrorCode, ParseError, failure};

pub const ENTITIES: usize = 0;
pub const PLANES: usize = 1;
pub const TEXDATA: usize = 2;
pub const VERTEXES: usize = 3;
pub const VISIBILITY: usize = 4;
pub const NODES: usize = 5;
pub const TEXINFO: usize = 6;
pub const FACES: usize = 7;
pub const LIGHTING: usize = 8;
pub const LEAFS: usize = 10;
pub const EDGES: usize = 12;
pub const SURFEDGES: usize = 13;
pub const MODELS: usize = 14;
pub const LEAFFACES: usize = 16;
pub const LEAFBRUSHES: usize = 17;
pub const BRUSHES: usize = 18;
pub const BRUSHSIDES: usize = 19;
pub const VERTNORMALS: usize = 30;
pub const VERTNORMALINDICES: usize = 31;
pub const PRIMITIVES: usize = 37;
pub const PRIMVERTS: usize = 38;
pub const PRIMINDICES: usize = 39;
pub const PAKFILE: usize = 40;
pub const CUBEMAPS: usize = 42;
pub const TEXDATA_STRING_DATA: usize = 43;
pub const TEXDATA_STRING_TABLE: usize = 44;
pub const LIGHTING_HDR: usize = 53;
pub const FACES_HDR: usize = 58;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Float32(pub u32);

impl Float32 {
    pub fn value(self) -> f32 {
        f32::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vector3 {
    pub x: Float32,
    pub y: Float32,
    pub z: Float32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plane {
    pub normal: Vector3,
    pub distance: Float32,
    pub kind: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureData {
    pub reflectivity: Vector3,
    pub name_string_table_index: i32,
    pub width: i32,
    pub height: i32,
    pub view_width: i32,
    pub view_height: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vertex {
    pub position: Vector3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Node {
    pub plane_index: i32,
    pub children: [i32; 2],
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    pub first_face: u16,
    pub face_count: u16,
    pub area: i16,
    pub padding: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureInfo {
    pub texture_vectors: [[Float32; 4]; 2],
    pub lightmap_vectors: [[Float32; 4]; 2],
    pub flags: i32,
    pub texture_data_index: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Face {
    pub plane_index: u16,
    pub side: u8,
    pub on_node: u8,
    pub first_surface_edge: i32,
    pub surface_edge_count: i16,
    pub texture_info_index: i16,
    pub displacement_info_index: i16,
    pub surface_fog_volume_id: i16,
    pub styles: [u8; 4],
    pub light_offset: i32,
    pub area: Float32,
    pub lightmap_mins: [i32; 2],
    pub lightmap_size: [i32; 2],
    pub original_face: i32,
    pub primitive_and_shadow_bits: u16,
    pub first_primitive: u16,
    pub smoothing_groups: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightSample {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub exponent: i8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Leaf {
    pub contents: i32,
    pub cluster: i16,
    pub area_and_flags: u16,
    pub mins: [i16; 3],
    pub maxs: [i16; 3],
    pub first_leaf_face: u16,
    pub leaf_face_count: u16,
    pub first_leaf_brush: u16,
    pub leaf_brush_count: u16,
    pub leaf_water_data_id: i16,
    pub padding: i16,
    pub ambient_cube: Option<[u8; 24]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    pub vertex_indices: [u16; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Model {
    pub mins: Vector3,
    pub maxs: Vector3,
    pub origin: Vector3,
    pub head_node: i32,
    pub first_face: i32,
    pub face_count: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Brush {
    pub first_side: i32,
    pub side_count: i32,
    pub contents: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrushSide {
    pub plane_index: u16,
    pub texture_info_index: i16,
    pub displacement_info_index: i16,
    pub bevel: i16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Visibility {
    pub cluster_count: i32,
    pub offsets: Vec<[i32; 2]>,
    pub compressed_bytes: Vec<u8>,
    pub compressed_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cubemap {
    pub origin: [i32; 3],
    pub size: u8,
    pub padding: [u8; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Primitive {
    pub kind: u8,
    pub padding: u8,
    pub first_index: u16,
    pub index_count: u16,
    pub first_vertex: u16,
    pub vertex_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LumpData {
    Opaque,
    EntityBytes(Vec<u8>),
    Planes(Vec<Plane>),
    TextureData(Vec<TextureData>),
    Vertices(Vec<Vertex>),
    Visibility(Visibility),
    Nodes(Vec<Node>),
    TextureInfo(Vec<TextureInfo>),
    Faces(Vec<Face>),
    Lighting(Vec<LightSample>),
    Leaves(Vec<Leaf>),
    Edges(Vec<Edge>),
    SurfaceEdges(Vec<i32>),
    Models(Vec<Model>),
    LeafFaces(Vec<u16>),
    LeafBrushes(Vec<u16>),
    Brushes(Vec<Brush>),
    BrushSides(Vec<BrushSide>),
    VertexNormals(Vec<Vector3>),
    VertexNormalIndices(Vec<u16>),
    Primitives(Vec<Primitive>),
    PrimitiveVertices(Vec<Vector3>),
    PrimitiveIndices(Vec<u16>),
    Cubemaps(Vec<Cubemap>),
    TextureStringData(Vec<u8>),
    TextureStringOffsets(Vec<i32>),
}

pub(crate) fn parse_lump(
    index: usize,
    version: i32,
    bytes: &[u8],
    max_records: usize,
) -> Result<LumpData, ParseError> {
    if bytes.is_empty() {
        return Ok(LumpData::Opaque);
    }
    if !version_supported(index, version) {
        return Ok(LumpData::Opaque);
    }
    match index {
        ENTITIES => Ok(LumpData::EntityBytes(bytes.to_vec())),
        PLANES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 20, max_records, |record| Plane {
                normal: vector3(record, 0),
                distance: float32(record, 12),
                kind: i32_at(record, 16),
            })
            .map(LumpData::Planes)
        }
        TEXDATA => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 32, max_records, |record| TextureData {
                reflectivity: vector3(record, 0),
                name_string_table_index: i32_at(record, 12),
                width: i32_at(record, 16),
                height: i32_at(record, 20),
                view_width: i32_at(record, 24),
                view_height: i32_at(record, 28),
            })
            .map(LumpData::TextureData)
        }
        VERTEXES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 12, max_records, |record| Vertex {
                position: vector3(record, 0),
            })
            .map(LumpData::Vertices)
        }
        VISIBILITY => {
            require_version(index, version, 0, bytes.len())?;
            parse_visibility(index, bytes, max_records).map(LumpData::Visibility)
        }
        NODES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 32, max_records, |record| Node {
                plane_index: i32_at(record, 0),
                children: [i32_at(record, 4), i32_at(record, 8)],
                mins: [i16_at(record, 12), i16_at(record, 14), i16_at(record, 16)],
                maxs: [i16_at(record, 18), i16_at(record, 20), i16_at(record, 22)],
                first_face: u16_at(record, 24),
                face_count: u16_at(record, 26),
                area: i16_at(record, 28),
                padding: i16_at(record, 30),
            })
            .map(LumpData::Nodes)
        }
        TEXINFO => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 72, max_records, |record| TextureInfo {
                texture_vectors: [float4(record, 0), float4(record, 16)],
                lightmap_vectors: [float4(record, 32), float4(record, 48)],
                flags: i32_at(record, 64),
                texture_data_index: i32_at(record, 68),
            })
            .map(LumpData::TextureInfo)
        }
        FACES | FACES_HDR => {
            require_version(index, version, 1, bytes.len())?;
            fixed(index, bytes, 56, max_records, |record| Face {
                plane_index: u16_at(record, 0),
                side: record[2],
                on_node: record[3],
                first_surface_edge: i32_at(record, 4),
                surface_edge_count: i16_at(record, 8),
                texture_info_index: i16_at(record, 10),
                displacement_info_index: i16_at(record, 12),
                surface_fog_volume_id: i16_at(record, 14),
                styles: record[16..20].try_into().expect("fixed face field"),
                light_offset: i32_at(record, 20),
                area: float32(record, 24),
                lightmap_mins: [i32_at(record, 28), i32_at(record, 32)],
                lightmap_size: [i32_at(record, 36), i32_at(record, 40)],
                original_face: i32_at(record, 44),
                primitive_and_shadow_bits: u16_at(record, 48),
                first_primitive: u16_at(record, 50),
                smoothing_groups: u32_at(record, 52),
            })
            .map(LumpData::Faces)
        }
        LIGHTING | LIGHTING_HDR => {
            require_version(index, version, 1, bytes.len())?;
            fixed(index, bytes, 4, max_records, |record| LightSample {
                red: record[0],
                green: record[1],
                blue: record[2],
                exponent: record[3] as i8,
            })
            .map(LumpData::Lighting)
        }
        LEAFS => parse_leaves(index, version, bytes, max_records).map(LumpData::Leaves),
        EDGES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 4, max_records, |record| Edge {
                vertex_indices: [u16_at(record, 0), u16_at(record, 2)],
            })
            .map(LumpData::Edges)
        }
        SURFEDGES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 4, max_records, |record| i32_at(record, 0))
                .map(LumpData::SurfaceEdges)
        }
        MODELS => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 48, max_records, |record| Model {
                mins: vector3(record, 0),
                maxs: vector3(record, 12),
                origin: vector3(record, 24),
                head_node: i32_at(record, 36),
                first_face: i32_at(record, 40),
                face_count: i32_at(record, 44),
            })
            .map(LumpData::Models)
        }
        LEAFFACES => scalar_u16(index, version, bytes, max_records).map(LumpData::LeafFaces),
        LEAFBRUSHES => scalar_u16(index, version, bytes, max_records).map(LumpData::LeafBrushes),
        BRUSHES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 12, max_records, |record| Brush {
                first_side: i32_at(record, 0),
                side_count: i32_at(record, 4),
                contents: i32_at(record, 8),
            })
            .map(LumpData::Brushes)
        }
        BRUSHSIDES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 8, max_records, |record| BrushSide {
                plane_index: u16_at(record, 0),
                texture_info_index: i16_at(record, 2),
                displacement_info_index: i16_at(record, 4),
                bevel: i16_at(record, 6),
            })
            .map(LumpData::BrushSides)
        }
        VERTNORMALS => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 12, max_records, |record| vector3(record, 0))
                .map(LumpData::VertexNormals)
        }
        VERTNORMALINDICES => {
            scalar_u16(index, version, bytes, max_records).map(LumpData::VertexNormalIndices)
        }
        PRIMITIVES => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 10, max_records, |record| Primitive {
                kind: record[0],
                padding: record[1],
                first_index: u16_at(record, 2),
                index_count: u16_at(record, 4),
                first_vertex: u16_at(record, 6),
                vertex_count: u16_at(record, 8),
            })
            .map(LumpData::Primitives)
        }
        PRIMVERTS => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 12, max_records, |record| vector3(record, 0))
                .map(LumpData::PrimitiveVertices)
        }
        PRIMINDICES => {
            scalar_u16(index, version, bytes, max_records).map(LumpData::PrimitiveIndices)
        }
        CUBEMAPS => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 16, max_records, |record| Cubemap {
                origin: [i32_at(record, 0), i32_at(record, 4), i32_at(record, 8)],
                size: record[12],
                padding: record[13..16].try_into().expect("fixed cubemap padding"),
            })
            .map(LumpData::Cubemaps)
        }
        TEXDATA_STRING_DATA => {
            require_version(index, version, 0, bytes.len())?;
            Ok(LumpData::TextureStringData(bytes.to_vec()))
        }
        TEXDATA_STRING_TABLE => {
            require_version(index, version, 0, bytes.len())?;
            fixed(index, bytes, 4, max_records, |record| i32_at(record, 0))
                .map(LumpData::TextureStringOffsets)
        }
        PAKFILE => Ok(LumpData::Opaque),
        _ => Ok(LumpData::Opaque),
    }
}

pub(crate) fn is_implemented(index: usize) -> bool {
    matches!(
        index,
        ENTITIES
            | PLANES
            | TEXDATA
            | VERTEXES
            | VISIBILITY
            | NODES
            | TEXINFO
            | FACES
            | LIGHTING
            | LEAFS
            | EDGES
            | SURFEDGES
            | MODELS
            | LEAFFACES
            | LEAFBRUSHES
            | BRUSHES
            | BRUSHSIDES
            | VERTNORMALS
            | VERTNORMALINDICES
            | PRIMITIVES
            | PRIMVERTS
            | PRIMINDICES
            | PAKFILE
            | CUBEMAPS
            | TEXDATA_STRING_DATA
            | TEXDATA_STRING_TABLE
            | LIGHTING_HDR
            | FACES_HDR
    )
}

pub(crate) fn version_supported(index: usize, version: i32) -> bool {
    match index {
        FACES | LIGHTING | LIGHTING_HDR | FACES_HDR => version == 1,
        LEAFS => matches!(version, 0 | 1),
        ENTITIES | PAKFILE | PLANES | TEXDATA | VERTEXES | VISIBILITY | NODES | TEXINFO | EDGES
        | SURFEDGES | MODELS | LEAFFACES | LEAFBRUSHES | BRUSHES | BRUSHSIDES | VERTNORMALS
        | VERTNORMALINDICES | PRIMITIVES | PRIMVERTS | PRIMINDICES | CUBEMAPS
        | TEXDATA_STRING_DATA | TEXDATA_STRING_TABLE => version == 0,
        _ => true,
    }
}

fn parse_visibility(
    index: usize,
    bytes: &[u8],
    max_records: usize,
) -> Result<Visibility, ParseError> {
    if bytes.len() < 4 {
        return Err(record_error(index, bytes.len(), 4));
    }
    let cluster_count = i32_at(bytes, 0);
    let count = usize::try_from(cluster_count).map_err(|_| {
        failure(
            ErrorCode::InvalidRecord,
            Some(index),
            0..4,
            Some(cluster_count.unsigned_abs() as usize),
            Some(max_records),
        )
    })?;
    if count > max_records {
        return Err(failure(
            ErrorCode::RecordBudget,
            Some(index),
            0..4,
            Some(count),
            Some(max_records),
        ));
    }
    let table_end = count
        .checked_mul(8)
        .and_then(|length| length.checked_add(4))
        .ok_or_else(|| record_error(index, bytes.len(), usize::MAX))?;
    if table_end > bytes.len() {
        return Err(record_error(index, bytes.len(), table_end));
    }
    let offsets = (0..count)
        .map(|entry| {
            let offset = 4 + entry * 8;
            [i32_at(bytes, offset), i32_at(bytes, offset + 4)]
        })
        .collect();
    Ok(Visibility {
        cluster_count,
        offsets,
        compressed_bytes: bytes[table_end..].to_vec(),
        compressed_range: table_end..bytes.len(),
    })
}

fn parse_leaves(
    index: usize,
    version: i32,
    bytes: &[u8],
    max_records: usize,
) -> Result<Vec<Leaf>, ParseError> {
    let size = match version {
        0 => 56,
        1 => 32,
        _ => return Err(version_error(index, version, bytes.len())),
    };
    fixed(index, bytes, size, max_records, |record| Leaf {
        contents: i32_at(record, 0),
        cluster: i16_at(record, 4),
        area_and_flags: u16_at(record, 6),
        mins: [i16_at(record, 8), i16_at(record, 10), i16_at(record, 12)],
        maxs: [i16_at(record, 14), i16_at(record, 16), i16_at(record, 18)],
        first_leaf_face: u16_at(record, 20),
        leaf_face_count: u16_at(record, 22),
        first_leaf_brush: u16_at(record, 24),
        leaf_brush_count: u16_at(record, 26),
        leaf_water_data_id: i16_at(record, 28),
        padding: i16_at(record, 30),
        ambient_cube: (version == 0)
            .then(|| record[32..56].try_into().expect("fixed ambient cube")),
    })
}

fn scalar_u16(
    index: usize,
    version: i32,
    bytes: &[u8],
    max_records: usize,
) -> Result<Vec<u16>, ParseError> {
    require_version(index, version, 0, bytes.len())?;
    fixed(index, bytes, 2, max_records, |record| u16_at(record, 0))
}

fn fixed<T>(
    index: usize,
    bytes: &[u8],
    size: usize,
    max_records: usize,
    parse: impl Fn(&[u8]) -> T,
) -> Result<Vec<T>, ParseError> {
    if !bytes.len().is_multiple_of(size) {
        return Err(record_error(index, bytes.len(), size));
    }
    let count = bytes.len() / size;
    if count > max_records {
        return Err(failure(
            ErrorCode::RecordBudget,
            Some(index),
            0..bytes.len(),
            Some(count),
            Some(max_records),
        ));
    }
    Ok(bytes.chunks_exact(size).map(parse).collect())
}

fn require_version(
    index: usize,
    actual: i32,
    expected: i32,
    length: usize,
) -> Result<(), ParseError> {
    if actual != expected {
        return Err(version_error(index, actual, length));
    }
    Ok(())
}

fn version_error(index: usize, actual: i32, length: usize) -> ParseError {
    failure(
        ErrorCode::UnsupportedLumpVersion,
        Some(index),
        0..length,
        Some(actual.unsigned_abs() as usize),
        None,
    )
}

fn record_error(index: usize, length: usize, required: usize) -> ParseError {
    failure(
        ErrorCode::InvalidRecord,
        Some(index),
        0..length,
        Some(length),
        Some(required),
    )
}

fn float4(bytes: &[u8], offset: usize) -> [Float32; 4] {
    [
        float32(bytes, offset),
        float32(bytes, offset + 4),
        float32(bytes, offset + 8),
        float32(bytes, offset + 12),
    ]
}

fn vector3(bytes: &[u8], offset: usize) -> Vector3 {
    Vector3 {
        x: float32(bytes, offset),
        y: float32(bytes, offset + 4),
        z: float32(bytes, offset + 8),
    }
}

fn float32(bytes: &[u8], offset: usize) -> Float32 {
    Float32(u32_at(bytes, offset))
}

fn i16_at(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed record field"),
    )
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed record field"),
    )
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed record field"),
    )
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed record field"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixed_records_without_normalizing_float_bits_or_padding() {
        let mut plane = Vec::new();
        for bits in [0x3f80_0000_u32, 0x8000_0000, 0x7fc0_1234, 0x4120_0000] {
            plane.extend_from_slice(&bits.to_le_bytes());
        }
        plane.extend_from_slice(&5_i32.to_le_bytes());
        let LumpData::Planes(records) = parse_lump(PLANES, 0, &plane, 1).unwrap() else {
            panic!("plane records were not selected")
        };
        assert_eq!(records[0].normal.x.0, 0x3f80_0000);
        assert_eq!(records[0].normal.y.0, 0x8000_0000);
        assert_eq!(records[0].normal.z.0, 0x7fc0_1234);
        assert_eq!(records[0].kind, 5);

        let mut cubemap = Vec::new();
        cubemap.extend_from_slice(&1_i32.to_le_bytes());
        cubemap.extend_from_slice(&(-2_i32).to_le_bytes());
        cubemap.extend_from_slice(&3_i32.to_le_bytes());
        cubemap.extend_from_slice(&[4, 5, 6, 7]);
        let LumpData::Cubemaps(records) = parse_lump(CUBEMAPS, 0, &cubemap, 1).unwrap() else {
            panic!("cubemap records were not selected")
        };
        assert_eq!(records[0].origin, [1, -2, 3]);
        assert_eq!(records[0].size, 4);
        assert_eq!(records[0].padding, [5, 6, 7]);
    }

    #[test]
    fn frames_visibility_and_rejects_record_boundaries() {
        let mut bytes = 2_i32.to_le_bytes().to_vec();
        for value in [20_i32, 30, 40, 50] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[0xaa, 0xbb]);
        let LumpData::Visibility(visibility) = parse_lump(VISIBILITY, 0, &bytes, 2).unwrap() else {
            panic!("visibility framing was not selected")
        };
        assert_eq!(visibility.offsets, vec![[20, 30], [40, 50]]);
        assert_eq!(visibility.compressed_range, 20..22);
        assert_eq!(visibility.compressed_bytes, [0xaa, 0xbb]);

        assert_eq!(
            parse_lump(PLANES, 0, &[0; 19], 1).unwrap_err().code,
            ErrorCode::InvalidRecord
        );
        assert_eq!(
            parse_lump(PLANES, 0, &[0; 40], 1).unwrap_err().code,
            ErrorCode::RecordBudget
        );
        assert_eq!(
            parse_lump(PLANES, 9, &[0; 19], 1).unwrap(),
            LumpData::Opaque
        );
    }
}
