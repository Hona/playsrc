use std::ops::Range;

use playsrc_vhv::Vhv;

use crate::Document;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLightingMesh {
    pub primitive: usize,
    pub body_part: usize,
    pub model: usize,
    pub lod: usize,
    pub mesh: usize,
    pub strip_group: usize,
    pub vertex_count: usize,
    pub encoded_bgra_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticLightingJoin {
    pub model_checksum: u32,
    pub vhv_sha256: [u8; 32],
    pub root_lod: usize,
    pub meshes: Vec<StaticLightingMesh>,
    pub vertex_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticLightingJoinError {
    ChecksumMismatch,
    RootLodUnavailable,
    MeshCountMismatch,
    MeshLodMismatch,
    VertexCountMismatch,
    VertexTotalOverflow,
}

pub fn join_static_lighting(
    model: &Document,
    vhv: &Vhv,
) -> Result<StaticLightingJoin, StaticLightingJoinError> {
    let checksum = model.checksum as u32;
    if vhv.header.checksum != checksum {
        return Err(StaticLightingJoinError::ChecksumMismatch);
    }
    let root_lod = usize::from(model.root_lod);
    let mut expected = model
        .geometry
        .iter()
        .enumerate()
        .filter(|(_, primitive)| primitive.lod >= root_lod)
        .collect::<Vec<_>>();
    expected.sort_by_key(|(_, primitive)| {
        (
            primitive.lod,
            primitive.body_part,
            primitive.model,
            primitive.mesh,
            primitive.strip_group,
        )
    });
    if expected.is_empty() {
        return Err(StaticLightingJoinError::RootLodUnavailable);
    }
    let start = vhv
        .meshes
        .iter()
        .position(|mesh| mesh.lod as usize == root_lod)
        .ok_or(StaticLightingJoinError::RootLodUnavailable)?;
    let selected = &vhv.meshes[start..];
    if selected.len() != expected.len() {
        return Err(StaticLightingJoinError::MeshCountMismatch);
    }
    let mut meshes = Vec::with_capacity(selected.len());
    let mut vertex_count = 0usize;
    for ((primitive_index, primitive), lighting) in expected.into_iter().zip(selected) {
        if lighting.lod as usize != primitive.lod {
            return Err(StaticLightingJoinError::MeshLodMismatch);
        }
        if lighting.vertex_count as usize != primitive.vertices.len() {
            return Err(StaticLightingJoinError::VertexCountMismatch);
        }
        vertex_count = vertex_count
            .checked_add(primitive.vertices.len())
            .ok_or(StaticLightingJoinError::VertexTotalOverflow)?;
        meshes.push(StaticLightingMesh {
            primitive: primitive_index,
            body_part: primitive.body_part,
            model: primitive.model,
            lod: primitive.lod,
            mesh: primitive.mesh,
            strip_group: primitive.strip_group,
            vertex_count: primitive.vertices.len(),
            encoded_bgra_range: lighting.data_range.clone(),
        });
    }
    Ok(StaticLightingJoin {
        model_checksum: checksum,
        vhv_sha256: vhv.source_identity.sha256,
        root_lod,
        meshes,
        vertex_count,
    })
}
