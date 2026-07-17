use playsrc_bsp::{Bsp, Leaf, LumpData, Model, Node, Visibility as BspVisibility};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub distance: f32,
}
#[derive(Clone, Debug)]
pub struct World {
    pub identity: [u8; 32],
    pub visibility_mode: VisibilityMode,
    pub cluster_count: usize,
    pub words_per_row: usize,
    pub pvs: Vec<u32>,
    pub pas: Vec<u32>,
    pub planes: Vec<Plane>,
    pub nodes: Vec<Node>,
    pub leaves: Vec<Leaf>,
    pub leaf_faces: Vec<u16>,
    pub models: Vec<Model>,
    pub areas: Vec<Area>,
    pub portals: Vec<AreaPortal>,
    pub portal_vertices: Vec<[f32; 3]>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibilityMode {
    Compressed,
    NoVis,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Area {
    pub first_portal: usize,
    pub portal_count: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AreaPortal {
    pub key: u16,
    pub destination_area: usize,
    pub first_vertex: usize,
    pub vertex_count: usize,
    pub plane: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}
impl Aabb {
    pub fn new(minimum: [f32; 3], maximum: [f32; 3]) -> Result<Self, Error> {
        if minimum
            .iter()
            .chain(maximum.iter())
            .any(|value| !value.is_finite())
        {
            return Err(error(ErrorCode::NonFinite, None));
        }
        if (0..3).any(|axis| minimum[axis] > maximum[axis]) {
            return Err(error(ErrorCode::InvalidRange, None));
        }
        Ok(Self { minimum, maximum })
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CandidateKind {
    WorldModel,
    BrushModel,
    StaticProp,
    DynamicObject,
    DetailProp,
    Entity,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CandidateId {
    pub kind: CandidateKind,
    pub index: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub enum CandidateMembership {
    CompiledLeaves(Vec<usize>),
    Bounds(Aabb),
}
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateInput {
    pub id: CandidateId,
    pub membership: CandidateMembership,
    pub bounds: Option<Aabb>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub id: CandidateId,
    pub leaves: Vec<usize>,
    pub bounds: Option<Aabb>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateSet {
    pub revision: u64,
    pub identity: [u8; 32],
    pub candidates: Vec<Candidate>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AreaState {
    pub revision: u64,
    open: BTreeMap<u16, bool>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkyVisibility {
    NotVisible,
    Sky2d,
    Sky3d,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ViewQuery {
    pub origins: Vec<[f32; 3]>,
    pub bypass_pvs: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ViewResult {
    pub cache_identity: [u8; 32],
    pub origin_leaves: Vec<usize>,
    pub origin_clusters: Vec<i16>,
    pub outside_world: bool,
    pub merged_pvs: Vec<u32>,
    pub visible_areas: Vec<usize>,
    pub sky: SkyVisibility,
    pub leaves: Vec<usize>,
    pub world_surfaces: Vec<u16>,
    pub candidates: Vec<CandidateId>,
}
pub struct ViewCache {
    max_entries: usize,
    max_bytes: usize,
    used_bytes: usize,
    order: VecDeque<[u8; 32]>,
    entries: BTreeMap<[u8; 32], (usize, ViewResult)>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidCount,
    InvalidOffset,
    TruncatedRow,
    InvalidRun,
    InvalidReference,
    NonFinite,
    DepthLimit,
    InvalidRange,
    InvalidArea,
    InvalidPortal,
    InvalidCandidate,
    OriginLimit,
    CacheLimit,
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
pub fn compile(bsp: &Bsp) -> Result<World, Error> {
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let nodes = match &bsp.lumps[5].records {
        LumpData::Nodes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaves = match &bsp.lumps[10].records {
        LumpData::Leaves(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaf_faces = match &bsp.lumps[16].records {
        LumpData::LeafFaces(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let models = match &bsp.lumps[14].records {
        LumpData::Models(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    if nodes.len() > 65_536
        || leaves.len() > 65_536
        || planes.len() > 65_536
        || models.len() > 65_536
    {
        return Err(error(ErrorCode::InvalidCount, None));
    }
    let leaf_cluster_count = leaves
        .iter()
        .filter_map(|leaf| usize::try_from(leaf.cluster).ok())
        .max()
        .map_or(0, |cluster| cluster + 1);
    let (visibility_mode, cluster_count, pvs, pas) = match &bsp.lumps[4].records {
        LumpData::Visibility(visibility) => {
            let cluster_count = usize::try_from(visibility.cluster_count)
                .map_err(|_| error(ErrorCode::InvalidCount, None))?;
            if leaf_cluster_count > cluster_count {
                return Err(error(ErrorCode::InvalidCount, None));
            }
            let words = cluster_count.div_ceil(32);
            (
                VisibilityMode::Compressed,
                cluster_count,
                decode_rows(visibility, 0, cluster_count, words)?,
                decode_rows(visibility, 1, cluster_count, words)?,
            )
        }
        LumpData::Opaque if bsp.lumps[4].bytes(bsp).is_empty() => (
            VisibilityMode::NoVis,
            leaf_cluster_count,
            Vec::new(),
            Vec::new(),
        ),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let words = cluster_count.div_ceil(32);
    if cluster_count > 65_536
        || cluster_count
            .checked_mul(words)
            .is_none_or(|expanded_words| expanded_words > 134_217_728)
    {
        return Err(error(ErrorCode::InvalidCount, None));
    }
    let mut output_planes = Vec::with_capacity(planes.len());
    for (i, p) in planes.iter().enumerate() {
        let normal = [p.normal.x.value(), p.normal.y.value(), p.normal.z.value()];
        let distance = p.distance.value();
        if normal.iter().any(|v| !v.is_finite()) || !distance.is_finite() {
            return Err(error(ErrorCode::NonFinite, Some(i)));
        }
        output_planes.push(Plane { normal, distance });
    }
    for (i, node) in nodes.iter().enumerate() {
        if node.plane_index < 0 || node.plane_index as usize >= output_planes.len() {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
        for child in node.children {
            if child >= 0 && child as usize >= nodes.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
            if child < 0 && (-1_i64 - child as i64) as usize >= leaves.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
        }
    }
    let face_count = match &bsp.lumps[7].records {
        LumpData::Faces(faces) => faces.len(),
        _ => match &bsp.lumps[58].records {
            LumpData::Faces(faces) => faces.len(),
            _ => return Err(error(ErrorCode::MissingLump, None)),
        },
    };
    for (i, leaf) in leaves.iter().enumerate() {
        if leaf.cluster >= 0 && leaf.cluster as usize >= cluster_count {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
        let start = leaf.first_leaf_face as usize;
        let end = start
            .checked_add(leaf.leaf_face_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(i)))?;
        if end > leaf_faces.len() {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
        for face in &leaf_faces[start..end] {
            if usize::from(*face) >= face_count {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
        }
    }
    let areas = parse_areas(bsp)?;
    let portals = parse_portals(bsp)?;
    let portal_vertices = parse_portal_vertices(bsp)?;
    validate_areas(&areas, &portals, &portal_vertices, output_planes.len())?;
    for (index, leaf) in leaves.iter().enumerate() {
        let area = usize::from(leaf.area_and_flags & 0x01ff);
        if !areas.is_empty() && area >= areas.len() {
            return Err(error(ErrorCode::InvalidArea, Some(index)));
        }
    }
    let mut world = World {
        identity: [0; 32],
        visibility_mode,
        cluster_count,
        words_per_row: words,
        pvs,
        pas,
        planes: output_planes,
        nodes,
        leaves,
        leaf_faces,
        models,
        areas,
        portals,
        portal_vertices,
    };
    validate_heads(&world)?;
    world.identity = world_identity(&world);
    Ok(world)
}

fn decode_rows(
    v: &BspVisibility,
    kind: usize,
    clusters: usize,
    words: usize,
) -> Result<Vec<u32>, Error> {
    let mut result = Vec::with_capacity(clusters * words);
    let row_bytes = clusters.div_ceil(8);
    for cluster in 0..clusters {
        let raw = v
            .offsets
            .get(cluster)
            .ok_or_else(|| error(ErrorCode::InvalidOffset, Some(cluster)))?[kind];
        let offset =
            usize::try_from(raw).map_err(|_| error(ErrorCode::InvalidOffset, Some(cluster)))?;
        if offset < v.compressed_range.start || offset > v.compressed_range.end {
            return Err(error(ErrorCode::InvalidOffset, Some(cluster)));
        }
        let mut at = offset - v.compressed_range.start;
        let mut decoded = Vec::with_capacity(row_bytes);
        while decoded.len() < row_bytes {
            let Some(&byte) = v.compressed_bytes.get(at) else {
                return Err(error(ErrorCode::TruncatedRow, Some(cluster)));
            };
            at += 1;
            if byte != 0 {
                decoded.push(byte)
            } else {
                let Some(&run) = v.compressed_bytes.get(at) else {
                    return Err(error(ErrorCode::TruncatedRow, Some(cluster)));
                };
                at += 1;
                if run == 0 || decoded.len() + run as usize > row_bytes {
                    return Err(error(ErrorCode::InvalidRun, Some(cluster)));
                }
                decoded.resize(decoded.len() + run as usize, 0);
            }
        }
        if !clusters.is_multiple_of(8)
            && decoded
                .last()
                .is_some_and(|byte| byte & !((1_u8 << (clusters % 8)) - 1) != 0)
        {
            return Err(error(ErrorCode::InvalidRun, Some(cluster)));
        }
        let mut row = vec![0_u32; words];
        for bit in 0..clusters {
            if decoded[bit / 8] & (1 << (bit % 8)) != 0 {
                row[bit / 32] |= 1 << (bit % 32);
            }
        }
        result.extend(row);
    }
    Ok(result)
}

fn parse_areas(bsp: &Bsp) -> Result<Vec<Area>, Error> {
    let lump = &bsp.lumps[20];
    let bytes = lump.bytes(bsp);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if lump.version != 0 || !bytes.len().is_multiple_of(8) || bytes.len() / 8 > 256 {
        return Err(error(ErrorCode::InvalidArea, None));
    }
    bytes
        .chunks_exact(8)
        .enumerate()
        .map(|(index, record)| {
            let portal_count = usize::try_from(i32_at(record, 0))
                .map_err(|_| error(ErrorCode::InvalidArea, Some(index)))?;
            let first_portal = usize::try_from(i32_at(record, 4))
                .map_err(|_| error(ErrorCode::InvalidArea, Some(index)))?;
            Ok(Area {
                first_portal,
                portal_count,
            })
        })
        .collect()
}

fn parse_portals(bsp: &Bsp) -> Result<Vec<AreaPortal>, Error> {
    let lump = &bsp.lumps[21];
    let bytes = lump.bytes(bsp);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if lump.version != 0 || !bytes.len().is_multiple_of(12) || bytes.len() / 12 > 1_024 {
        return Err(error(ErrorCode::InvalidPortal, None));
    }
    bytes
        .chunks_exact(12)
        .enumerate()
        .map(|(index, record)| {
            Ok(AreaPortal {
                key: u16_at(record, 0),
                destination_area: usize::from(u16_at(record, 2)),
                first_vertex: usize::from(u16_at(record, 4)),
                vertex_count: usize::from(u16_at(record, 6)),
                plane: usize::try_from(i32_at(record, 8))
                    .map_err(|_| error(ErrorCode::InvalidPortal, Some(index)))?,
            })
        })
        .collect()
}

fn parse_portal_vertices(bsp: &Bsp) -> Result<Vec<[f32; 3]>, Error> {
    let lump = &bsp.lumps[41];
    let bytes = lump.bytes(bsp);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if lump.version != 0 || !bytes.len().is_multiple_of(12) || bytes.len() / 12 > 128_000 {
        return Err(error(ErrorCode::InvalidPortal, None));
    }
    bytes
        .chunks_exact(12)
        .enumerate()
        .map(|(index, record)| {
            let point = [f32_at(record, 0), f32_at(record, 4), f32_at(record, 8)];
            if point.iter().any(|value| !value.is_finite()) {
                return Err(error(ErrorCode::NonFinite, Some(index)));
            }
            Ok(point)
        })
        .collect()
}

fn validate_areas(
    areas: &[Area],
    portals: &[AreaPortal],
    vertices: &[[f32; 3]],
    plane_count: usize,
) -> Result<(), Error> {
    for (index, area) in areas.iter().enumerate() {
        if area
            .first_portal
            .checked_add(area.portal_count)
            .is_none_or(|end| end > portals.len())
        {
            return Err(error(ErrorCode::InvalidArea, Some(index)));
        }
    }
    for (index, portal) in portals.iter().enumerate() {
        if portal.destination_area >= areas.len()
            || portal.plane >= plane_count
            || portal.vertex_count > 32
            || portal
                .first_vertex
                .checked_add(portal.vertex_count)
                .is_none_or(|end| end > vertices.len())
        {
            return Err(error(ErrorCode::InvalidPortal, Some(index)));
        }
    }
    Ok(())
}

fn validate_heads(world: &World) -> Result<(), Error> {
    for (model_index, model) in world.models.iter().enumerate() {
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        validate_child(world, model.head_node, &mut visiting, &mut visited).map_err(
            |mut error| {
                if error.item.is_none() {
                    error.item = Some(model_index);
                }
                error
            },
        )?;
    }
    Ok(())
}

fn validate_child(
    world: &World,
    child: i32,
    visiting: &mut BTreeSet<i32>,
    visited: &mut BTreeSet<i32>,
) -> Result<(), Error> {
    if child < 0 {
        let leaf = (-1_i64 - i64::from(child)) as usize;
        return (leaf < world.leaves.len())
            .then_some(())
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf)));
    }
    if visited.contains(&child) {
        return Ok(());
    }
    if !visiting.insert(child) {
        return Err(error(ErrorCode::DepthLimit, None));
    }
    let node = world
        .nodes
        .get(child as usize)
        .ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
    validate_child(world, node.children[0], visiting, visited)?;
    validate_child(world, node.children[1], visiting, visited)?;
    visiting.remove(&child);
    visited.insert(child);
    Ok(())
}

fn world_identity(world: &World) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-visibility-world-v1");
    digest.update([match world.visibility_mode {
        VisibilityMode::Compressed => 0,
        VisibilityMode::NoVis => 1,
    }]);
    digest.update((world.cluster_count as u64).to_le_bytes());
    digest.update((world.nodes.len() as u64).to_le_bytes());
    digest.update((world.leaves.len() as u64).to_le_bytes());
    for plane in &world.planes {
        for value in plane.normal.into_iter().chain([plane.distance]) {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    for node in &world.nodes {
        digest.update(node.plane_index.to_le_bytes());
        for child in node.children {
            digest.update(child.to_le_bytes());
        }
        for value in node.mins.into_iter().chain(node.maxs) {
            digest.update(value.to_le_bytes());
        }
        digest.update(node.first_face.to_le_bytes());
        digest.update(node.face_count.to_le_bytes());
        digest.update(node.area.to_le_bytes());
        digest.update(node.padding.to_le_bytes());
    }
    for leaf in &world.leaves {
        digest.update(leaf.contents.to_le_bytes());
        digest.update(leaf.cluster.to_le_bytes());
        digest.update(leaf.area_and_flags.to_le_bytes());
        for value in leaf.mins.into_iter().chain(leaf.maxs) {
            digest.update(value.to_le_bytes());
        }
        digest.update(leaf.first_leaf_face.to_le_bytes());
        digest.update(leaf.leaf_face_count.to_le_bytes());
        digest.update(leaf.first_leaf_brush.to_le_bytes());
        digest.update(leaf.leaf_brush_count.to_le_bytes());
        digest.update(leaf.leaf_water_data_id.to_le_bytes());
        digest.update(leaf.padding.to_le_bytes());
        if let Some(cube) = leaf.ambient_cube {
            digest.update([1]);
            digest.update(cube);
        } else {
            digest.update([0]);
        }
    }
    for face in &world.leaf_faces {
        digest.update(face.to_le_bytes());
    }
    for model in &world.models {
        for value in [model.mins, model.maxs, model.origin]
            .into_iter()
            .flat_map(|vector| [vector.x.0, vector.y.0, vector.z.0])
        {
            digest.update(value.to_le_bytes());
        }
        digest.update(model.head_node.to_le_bytes());
        digest.update(model.first_face.to_le_bytes());
        digest.update(model.face_count.to_le_bytes());
    }
    for word in world.pvs.iter().chain(&world.pas) {
        digest.update(word.to_le_bytes());
    }
    for area in &world.areas {
        digest.update((area.first_portal as u64).to_le_bytes());
        digest.update((area.portal_count as u64).to_le_bytes());
    }
    for portal in &world.portals {
        digest.update(portal.key.to_le_bytes());
        digest.update((portal.destination_area as u64).to_le_bytes());
        digest.update((portal.first_vertex as u64).to_le_bytes());
        digest.update((portal.vertex_count as u64).to_le_bytes());
        digest.update((portal.plane as u64).to_le_bytes());
    }
    for vertex in &world.portal_vertices {
        for value in vertex {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    digest.finalize().into()
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated record"),
    )
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated record"),
    )
}
fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated record"),
    )
}
impl World {
    pub fn visible(&self, from: usize, to: usize) -> bool {
        from < self.cluster_count
            && to < self.cluster_count
            && (self.visibility_mode == VisibilityMode::NoVis
                || self.pvs[from * self.words_per_row + to / 32] & (1 << (to % 32)) != 0)
    }
    pub fn audible(&self, from: usize, to: usize) -> bool {
        from < self.cluster_count
            && to < self.cluster_count
            && (self.visibility_mode == VisibilityMode::NoVis
                || self.pas[from * self.words_per_row + to / 32] & (1 << (to % 32)) != 0)
    }
    pub fn locate_leaf(&self, point: [f32; 3]) -> Result<usize, Error> {
        if point.iter().any(|v| !v.is_finite()) {
            return Err(error(ErrorCode::NonFinite, None));
        }
        let head = self
            .models
            .first()
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?
            .head_node;
        let mut child = head;
        for _ in 0..=self.nodes.len() {
            if child < 0 {
                let leaf = (-1_i64 - i64::from(child)) as usize;
                return (leaf < self.leaves.len())
                    .then_some(leaf)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf)));
            }
            let node = self
                .nodes
                .get(child as usize)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
            let plane = self.planes[node.plane_index as usize];
            let d = point[0] * plane.normal[0]
                + point[1] * plane.normal[1]
                + point[2] * plane.normal[2]
                - plane.distance;
            child = node.children[usize::from(d < 0.)];
        }
        Err(error(ErrorCode::DepthLimit, None))
    }

    pub fn leaves_in_box(&self, bounds: Aabb) -> Result<Vec<usize>, Error> {
        let model = self
            .models
            .first()
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
        let center = [
            (bounds.minimum[0] + bounds.maximum[0]) * 0.5,
            (bounds.minimum[1] + bounds.maximum[1]) * 0.5,
            (bounds.minimum[2] + bounds.maximum[2]) * 0.5,
        ];
        let extents = [
            bounds.maximum[0] - center[0],
            bounds.maximum[1] - center[1],
            bounds.maximum[2] - center[2],
        ];
        let mut stack = vec![model.head_node];
        let mut seen = BTreeSet::new();
        let mut output = Vec::new();
        while let Some(child) = stack.pop() {
            if child < 0 {
                let leaf = (-1_i64 - i64::from(child)) as usize;
                if leaf >= self.leaves.len() {
                    return Err(error(ErrorCode::InvalidReference, Some(leaf)));
                }
                if seen.insert(leaf) {
                    output.push(leaf);
                }
                continue;
            }
            let node = self
                .nodes
                .get(child as usize)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
            let plane = self.planes[node.plane_index as usize];
            let distance = dot3(plane.normal, center) - plane.distance;
            let radius = dot3(plane.normal.map(f32::abs), extents);
            if distance >= radius {
                stack.push(node.children[0]);
            } else if distance < -radius {
                stack.push(node.children[1]);
            } else {
                stack.push(node.children[1]);
                stack.push(node.children[0]);
            }
        }
        Ok(output)
    }

    pub fn box_visible(&self, from_cluster: usize, bounds: Aabb) -> Result<bool, Error> {
        Ok(self.leaves_in_box(bounds)?.into_iter().any(|leaf| {
            self.leaves[leaf].cluster >= 0
                && self.visible(from_cluster, self.leaves[leaf].cluster as usize)
        }))
    }

    pub fn point_visible(&self, from_cluster: usize, point: [f32; 3]) -> Result<bool, Error> {
        let leaf = self.locate_leaf(point)?;
        let cluster = self.leaves[leaf].cluster;
        Ok(cluster >= 0 && self.visible(from_cluster, cluster as usize))
    }

    pub fn view(
        &self,
        state: &AreaState,
        candidates: &CandidateSet,
        query: &ViewQuery,
    ) -> Result<ViewResult, Error> {
        if query.origins.is_empty() || query.origins.len() > 32 {
            return Err(error(ErrorCode::OriginLimit, None));
        }
        let mut origin_leaves = Vec::with_capacity(query.origins.len());
        let mut origin_clusters = Vec::with_capacity(query.origins.len());
        for origin in &query.origins {
            let leaf = self.locate_leaf(*origin)?;
            origin_leaves.push(leaf);
            origin_clusters.push(self.leaves[leaf].cluster);
        }
        let outside_world = origin_clusters.iter().any(|cluster| *cluster < 0);
        let mut unique_clusters: Vec<_> = origin_clusters
            .iter()
            .filter_map(|cluster| usize::try_from(*cluster).ok())
            .collect();
        unique_clusters.sort_unstable();
        unique_clusters.dedup();
        let all_clusters =
            outside_world || query.bypass_pvs || self.visibility_mode == VisibilityMode::NoVis;
        let merged_pvs = if all_clusters {
            all_cluster_words(self.cluster_count, self.words_per_row)
        } else {
            let mut merged = vec![0; self.words_per_row];
            for cluster in &unique_clusters {
                let start = cluster * self.words_per_row;
                for (output, input) in merged
                    .iter_mut()
                    .zip(&self.pvs[start..start + self.words_per_row])
                {
                    *output |= *input;
                }
            }
            merged
        };
        let start_area = usize::from(self.leaves[origin_leaves[0]].area_and_flags & 0x01ff);
        let visible_areas = if outside_world || start_area == 0 || self.areas.is_empty() {
            (0..self.areas.len()).collect()
        } else {
            state.visible_areas(self, start_area)?
        };
        let visible_area_set: BTreeSet<_> = visible_areas.iter().copied().collect();
        let mut allowed = vec![false; self.leaves.len()];
        for (index, leaf) in self.leaves.iter().enumerate() {
            let area = usize::from(leaf.area_and_flags & 0x01ff);
            let cluster_visible = outside_world
                || (leaf.cluster >= 0
                    && cluster_bit(&merged_pvs, self.cluster_count, leaf.cluster as usize));
            let area_visible = outside_world
                || start_area == 0
                || self.areas.is_empty()
                || visible_area_set.contains(&area);
            allowed[index] = cluster_visible && area_visible;
        }
        let leaves = self.front_to_back_leaves(query.origins[0], &allowed)?;
        let mut seen_faces = BTreeSet::new();
        let mut world_surfaces = Vec::new();
        for leaf in &leaves {
            let record = &self.leaves[*leaf];
            let start = usize::from(record.first_leaf_face);
            let end = start + usize::from(record.leaf_face_count);
            for face in &self.leaf_faces[start..end] {
                if seen_faces.insert(*face) {
                    world_surfaces.push(*face);
                }
            }
        }
        let visible_leaf_set: BTreeSet<_> = leaves.iter().copied().collect();
        let candidate_ids = candidates
            .candidates
            .iter()
            .filter(|candidate| {
                outside_world
                    || candidate
                        .leaves
                        .iter()
                        .any(|leaf| visible_leaf_set.contains(leaf))
            })
            .map(|candidate| candidate.id)
            .collect();
        let sky = sky_visibility(&origin_leaves, &self.leaves);
        let cache_identity = view_identity(ViewIdentityInput {
            world: self,
            state,
            candidates,
            clusters: &unique_clusters,
            origins: &query.origins,
            origin_leaves: &origin_leaves,
            bypass: query.bypass_pvs,
            outside: outside_world,
        });
        Ok(ViewResult {
            cache_identity,
            origin_leaves,
            origin_clusters,
            outside_world,
            merged_pvs,
            visible_areas,
            sky,
            leaves,
            world_surfaces,
            candidates: candidate_ids,
        })
    }

    fn front_to_back_leaves(
        &self,
        origin: [f32; 3],
        allowed: &[bool],
    ) -> Result<Vec<usize>, Error> {
        let mut output = Vec::new();
        let mut seen = BTreeSet::new();
        let mut stack = vec![
            self.models
                .first()
                .ok_or_else(|| error(ErrorCode::InvalidReference, None))?
                .head_node,
        ];
        while let Some(child) = stack.pop() {
            if child < 0 {
                let leaf = (-1_i64 - i64::from(child)) as usize;
                if allowed.get(leaf).copied().unwrap_or(false) && seen.insert(leaf) {
                    output.push(leaf);
                }
                continue;
            }
            let node = self
                .nodes
                .get(child as usize)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
            let plane = self.planes[node.plane_index as usize];
            let distance = dot3(plane.normal, origin) - plane.distance;
            let near_side = usize::from(distance < 0.0);
            stack.push(node.children[1 - near_side]);
            stack.push(node.children[near_side]);
        }
        Ok(output)
    }
}

impl CandidateSet {
    pub fn compile(world: &World, revision: u64, inputs: &[CandidateInput]) -> Result<Self, Error> {
        if inputs.len() > 1_000_000 {
            return Err(error(ErrorCode::InvalidCandidate, None));
        }
        let mut candidates = Vec::with_capacity(inputs.len());
        let mut total_memberships = 0_usize;
        for input in inputs {
            let mut leaves = match &input.membership {
                CandidateMembership::CompiledLeaves(leaves) => leaves.clone(),
                CandidateMembership::Bounds(bounds) => world.leaves_in_box(*bounds)?,
            };
            leaves.sort_unstable();
            leaves.dedup();
            total_memberships = total_memberships
                .checked_add(leaves.len())
                .filter(|count| *count <= 16_777_216)
                .ok_or_else(|| error(ErrorCode::InvalidCandidate, None))?;
            if leaves.iter().any(|leaf| *leaf >= world.leaves.len()) {
                return Err(error(
                    ErrorCode::InvalidCandidate,
                    Some(input.id.index as usize),
                ));
            }
            candidates.push(Candidate {
                id: input.id,
                leaves,
                bounds: input.bounds,
            });
        }
        candidates.sort_by_key(|candidate| candidate.id);
        if candidates.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(error(ErrorCode::InvalidCandidate, None));
        }
        let identity = candidate_identity(world, revision, &candidates);
        Ok(Self {
            revision,
            identity,
            candidates,
        })
    }
}

impl AreaState {
    pub fn new(world: &World) -> Self {
        Self {
            revision: 0,
            open: world
                .portals
                .iter()
                .map(|portal| (portal.key, false))
                .collect(),
        }
    }

    pub fn portal_open(&self, key: u16) -> Option<bool> {
        self.open.get(&key).copied()
    }

    pub fn set_portals(&mut self, updates: &[(u16, bool)]) -> Result<bool, Error> {
        if updates.len() > 1_024 {
            return Err(error(ErrorCode::InvalidPortal, None));
        }
        let mut normalized = BTreeMap::new();
        for (key, open) in updates {
            if !self.open.contains_key(key) || normalized.insert(*key, *open).is_some() {
                return Err(error(ErrorCode::InvalidPortal, Some(usize::from(*key))));
            }
        }
        let changed = normalized
            .iter()
            .any(|(key, open)| self.open.get(key) != Some(open));
        if changed {
            let revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| error(ErrorCode::InvalidPortal, None))?;
            for (key, open) in normalized {
                self.open.insert(key, open);
            }
            self.revision = revision;
        }
        Ok(changed)
    }

    pub fn connected(
        &self,
        world: &World,
        start: usize,
        destination: usize,
    ) -> Result<bool, Error> {
        if start >= world.areas.len() || destination >= world.areas.len() {
            return Err(error(ErrorCode::InvalidArea, Some(start.max(destination))));
        }
        Ok(self.visible_areas(world, start)?.contains(&destination))
    }

    pub fn visible_areas(&self, world: &World, start: usize) -> Result<Vec<usize>, Error> {
        if start >= world.areas.len() {
            return Err(error(ErrorCode::InvalidArea, Some(start)));
        }
        if start == 0 {
            return Ok((0..world.areas.len()).collect());
        }
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        visited.insert(start);
        while let Some(area_index) = queue.pop_front() {
            let area = world.areas[area_index];
            for portal in &world.portals[area.first_portal..area.first_portal + area.portal_count] {
                if self.open.get(&portal.key).copied().unwrap_or(false)
                    && visited.insert(portal.destination_area)
                {
                    queue.push_back(portal.destination_area);
                }
            }
        }
        Ok(visited.into_iter().collect())
    }
}

impl ViewCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Result<Self, Error> {
        if max_entries == 0 || max_bytes == 0 {
            return Err(error(ErrorCode::CacheLimit, None));
        }
        Ok(Self {
            max_entries,
            max_bytes,
            used_bytes: 0,
            order: VecDeque::new(),
            entries: BTreeMap::new(),
        })
    }

    pub fn get(&mut self, identity: &[u8; 32]) -> Option<&ViewResult> {
        if self.entries.contains_key(identity) {
            self.order.retain(|key| key != identity);
            self.order.push_back(*identity);
        }
        self.entries.get(identity).map(|(_, result)| result)
    }

    pub fn insert(&mut self, result: ViewResult) -> Result<(), Error> {
        let bytes = view_result_bytes(&result);
        if bytes > self.max_bytes {
            return Err(error(ErrorCode::CacheLimit, None));
        }
        if let Some((old_bytes, _)) = self.entries.remove(&result.cache_identity) {
            self.used_bytes -= old_bytes;
            self.order.retain(|key| key != &result.cache_identity);
        }
        while self.entries.len() >= self.max_entries || self.used_bytes + bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                return Err(error(ErrorCode::CacheLimit, None));
            };
            if let Some((old_bytes, _)) = self.entries.remove(&oldest) {
                self.used_bytes -= old_bytes;
            }
        }
        self.used_bytes += bytes;
        self.order.push_back(result.cache_identity);
        self.entries.insert(result.cache_identity, (bytes, result));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn all_cluster_words(clusters: usize, words: usize) -> Vec<u32> {
    let mut result = vec![u32::MAX; words];
    if let Some(last) = result.last_mut()
        && !clusters.is_multiple_of(32)
    {
        *last = (1_u32 << (clusters % 32)) - 1;
    }
    result
}

fn cluster_bit(words: &[u32], clusters: usize, cluster: usize) -> bool {
    cluster < clusters && words[cluster / 32] & (1 << (cluster % 32)) != 0
}

fn sky_visibility(origin_leaves: &[usize], leaves: &[Leaf]) -> SkyVisibility {
    let mut output = SkyVisibility::NotVisible;
    for leaf in origin_leaves {
        let flags = leaves[*leaf].area_and_flags >> 9;
        if flags & 0x01 != 0 {
            return SkyVisibility::Sky3d;
        }
        if flags & 0x04 != 0 {
            output = SkyVisibility::Sky2d;
        }
    }
    output
}

fn candidate_identity(world: &World, revision: u64, candidates: &[Candidate]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-visibility-candidates-v1");
    digest.update(world.identity);
    digest.update(revision.to_le_bytes());
    for candidate in candidates {
        digest.update([candidate.id.kind as u8]);
        digest.update(candidate.id.index.to_le_bytes());
        digest.update((candidate.leaves.len() as u64).to_le_bytes());
        for leaf in &candidate.leaves {
            digest.update((*leaf as u64).to_le_bytes());
        }
        if let Some(bounds) = candidate.bounds {
            digest.update([1]);
            for value in bounds.minimum.into_iter().chain(bounds.maximum) {
                digest.update(value.to_bits().to_le_bytes());
            }
        } else {
            digest.update([0]);
        }
    }
    digest.finalize().into()
}

struct ViewIdentityInput<'a> {
    world: &'a World,
    state: &'a AreaState,
    candidates: &'a CandidateSet,
    clusters: &'a [usize],
    origins: &'a [[f32; 3]],
    origin_leaves: &'a [usize],
    bypass: bool,
    outside: bool,
}

fn view_identity(input: ViewIdentityInput<'_>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-visibility-view-v1");
    digest.update(input.world.identity);
    digest.update(input.state.revision.to_le_bytes());
    for (key, open) in &input.state.open {
        digest.update(key.to_le_bytes());
        digest.update([u8::from(*open)]);
    }
    digest.update(input.candidates.identity);
    digest.update([u8::from(input.bypass), u8::from(input.outside)]);
    for cluster in input.clusters {
        digest.update((*cluster as u64).to_le_bytes());
    }
    for (origin, leaf) in input.origins.iter().zip(input.origin_leaves) {
        digest.update((*leaf as u64).to_le_bytes());
        for value in origin {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    digest.finalize().into()
}

fn view_result_bytes(result: &ViewResult) -> usize {
    32 + result.origin_leaves.len() * size_of::<usize>()
        + result.origin_clusters.len() * size_of::<i16>()
        + result.merged_pvs.len() * size_of::<u32>()
        + result.visible_areas.len() * size_of::<usize>()
        + result.leaves.len() * size_of::<usize>()
        + result.world_surfaces.len() * size_of::<u16>()
        + result.candidates.len() * size_of::<CandidateId>()
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}
fn error(code: ErrorCode, item: Option<usize>) -> Error {
    Error { code, item }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_rows_and_queries_bits() {
        let v = BspVisibility {
            cluster_count: 10,
            offsets: vec![[84, 84]; 10],
            compressed_bytes: vec![0b0000_0011, 0b0000_0010],
            compressed_range: 84..86,
        };
        let rows = decode_rows(&v, 0, 10, 1).unwrap();
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0] & 3, 3);
        let w = World {
            identity: [0; 32],
            visibility_mode: VisibilityMode::Compressed,
            cluster_count: 10,
            words_per_row: 1,
            pvs: rows.clone(),
            pas: rows,
            planes: vec![],
            nodes: vec![],
            leaves: vec![],
            leaf_faces: vec![],
            models: vec![],
            areas: vec![],
            portals: vec![],
            portal_vertices: vec![],
        };
        assert!(w.visible(0, 1));
        assert!(!w.visible(0, 2));
    }
    #[test]
    fn traverses_nodes_to_leaf() {
        let w = World {
            identity: [0; 32],
            visibility_mode: VisibilityMode::Compressed,
            cluster_count: 0,
            words_per_row: 0,
            pvs: vec![],
            pas: vec![],
            planes: vec![Plane {
                normal: [1., 0., 0.],
                distance: 0.,
            }],
            nodes: vec![Node {
                plane_index: 0,
                children: [-1, -2],
                mins: [0; 3],
                maxs: [0; 3],
                first_face: 0,
                face_count: 0,
                area: 0,
                padding: 0,
            }],
            leaves: vec![leaf(-1, 0, 0, 0, 0), leaf(-1, 0, 0, 0, 0)],
            leaf_faces: vec![],
            models: vec![Model {
                mins: playsrc_bsp::Vector3 {
                    x: playsrc_bsp::Float32(0),
                    y: playsrc_bsp::Float32(0),
                    z: playsrc_bsp::Float32(0),
                },
                maxs: playsrc_bsp::Vector3 {
                    x: playsrc_bsp::Float32(0),
                    y: playsrc_bsp::Float32(0),
                    z: playsrc_bsp::Float32(0),
                },
                origin: playsrc_bsp::Vector3 {
                    x: playsrc_bsp::Float32(0),
                    y: playsrc_bsp::Float32(0),
                    z: playsrc_bsp::Float32(0),
                },
                head_node: 0,
                first_face: 0,
                face_count: 0,
            }],
            areas: vec![],
            portals: vec![],
            portal_vertices: vec![],
        };
        assert_eq!(w.locate_leaf([1., 0., 0.]).unwrap(), 0);
        assert_eq!(w.locate_leaf([-1., 0., 0.]).unwrap(), 1);
    }

    fn model(head_node: i32) -> Model {
        Model {
            mins: playsrc_bsp::Vector3 {
                x: playsrc_bsp::Float32((-16.0_f32).to_bits()),
                y: playsrc_bsp::Float32((-16.0_f32).to_bits()),
                z: playsrc_bsp::Float32((-16.0_f32).to_bits()),
            },
            maxs: playsrc_bsp::Vector3 {
                x: playsrc_bsp::Float32(16.0_f32.to_bits()),
                y: playsrc_bsp::Float32(16.0_f32.to_bits()),
                z: playsrc_bsp::Float32(16.0_f32.to_bits()),
            },
            origin: playsrc_bsp::Vector3 {
                x: playsrc_bsp::Float32(0),
                y: playsrc_bsp::Float32(0),
                z: playsrc_bsp::Float32(0),
            },
            head_node,
            first_face: 0,
            face_count: 14,
        }
    }

    fn leaf(cluster: i16, area: u16, flags: u16, first_face: u16, face_count: u16) -> Leaf {
        Leaf {
            contents: 0,
            cluster,
            area_and_flags: area | (flags << 9),
            mins: [-16; 3],
            maxs: [16; 3],
            first_leaf_face: first_face,
            leaf_face_count: face_count,
            first_leaf_brush: 0,
            leaf_brush_count: 0,
            leaf_water_data_id: -1,
            padding: 0,
            ambient_cube: None,
        }
    }

    fn view_world() -> World {
        let mut world = World {
            identity: [0; 32],
            visibility_mode: VisibilityMode::Compressed,
            cluster_count: 3,
            words_per_row: 1,
            pvs: vec![0b111, 0b010, 0b100],
            pas: vec![0b111, 0b010, 0b100],
            planes: vec![
                Plane {
                    normal: [1.0, 0.0, 0.0],
                    distance: 0.0,
                },
                Plane {
                    normal: [0.0, 1.0, 0.0],
                    distance: 0.0,
                },
            ],
            nodes: vec![
                Node {
                    plane_index: 0,
                    children: [1, -3],
                    mins: [-16; 3],
                    maxs: [16; 3],
                    first_face: 0,
                    face_count: 0,
                    area: 0,
                    padding: 0,
                },
                Node {
                    plane_index: 1,
                    children: [-1, -2],
                    mins: [-16; 3],
                    maxs: [16; 3],
                    first_face: 0,
                    face_count: 0,
                    area: 0,
                    padding: 0,
                },
            ],
            leaves: vec![
                leaf(0, 1, 0, 0, 2),
                leaf(1, 1, 4, 2, 2),
                leaf(2, 2, 1, 4, 1),
            ],
            leaf_faces: vec![10, 11, 11, 12, 13],
            models: vec![model(0)],
            areas: vec![
                Area {
                    first_portal: 0,
                    portal_count: 0,
                },
                Area {
                    first_portal: 0,
                    portal_count: 1,
                },
                Area {
                    first_portal: 1,
                    portal_count: 1,
                },
            ],
            portals: vec![
                AreaPortal {
                    key: 7,
                    destination_area: 2,
                    first_vertex: 0,
                    vertex_count: 0,
                    plane: 0,
                },
                AreaPortal {
                    key: 7,
                    destination_area: 1,
                    first_vertex: 0,
                    vertex_count: 0,
                    plane: 0,
                },
            ],
            portal_vertices: vec![],
        };
        world.identity = world_identity(&world);
        world
    }

    #[test]
    fn aabb_enumeration_is_complete_and_plane_touch_stays_front() {
        let world = view_world();
        assert_eq!(
            world
                .leaves_in_box(Aabb::new([-0.1, -0.1, -1.0], [0.1, 0.1, 1.0]).unwrap())
                .unwrap(),
            [0, 1, 2]
        );
        assert_eq!(
            world
                .leaves_in_box(Aabb::new([0.0, -1.0, -1.0], [1.0, 1.0, 1.0]).unwrap())
                .unwrap(),
            [0, 1]
        );
        assert_eq!(
            Aabb::new([1.0, 0.0, 0.0], [0.0, 1.0, 1.0])
                .unwrap_err()
                .code,
            ErrorCode::InvalidRange
        );
    }

    #[test]
    fn area_state_candidates_view_order_and_cache_identity_are_stable() {
        let world = view_world();
        let mut state = AreaState::new(&world);
        assert!(!state.connected(&world, 1, 2).unwrap());
        assert_eq!(state.visible_areas(&world, 0).unwrap(), [0, 1, 2]);
        let candidates = CandidateSet::compile(
            &world,
            4,
            &[
                CandidateInput {
                    id: CandidateId {
                        kind: CandidateKind::BrushModel,
                        index: 8,
                    },
                    membership: CandidateMembership::CompiledLeaves(vec![2, 2]),
                    bounds: None,
                },
                CandidateInput {
                    id: CandidateId {
                        kind: CandidateKind::Entity,
                        index: 3,
                    },
                    membership: CandidateMembership::Bounds(
                        Aabb::new([1.0, 1.0, 0.0], [1.0, 1.0, 0.0]).unwrap(),
                    ),
                    bounds: Some(Aabb::new([1.0, 1.0, 0.0], [1.0, 1.0, 0.0]).unwrap()),
                },
            ],
        )
        .unwrap();
        let query = ViewQuery {
            origins: vec![[1.0, 1.0, 0.0]],
            bypass_pvs: false,
        };
        let closed = world.view(&state, &candidates, &query).unwrap();
        assert_eq!(closed.leaves, [0, 1]);
        assert_eq!(closed.world_surfaces, [10, 11, 12]);
        assert_eq!(
            closed.candidates,
            [CandidateId {
                kind: CandidateKind::Entity,
                index: 3
            }]
        );
        assert_eq!(closed.sky, SkyVisibility::NotVisible);

        assert!(state.set_portals(&[(7, true)]).unwrap());
        assert!(state.connected(&world, 1, 2).unwrap());
        let open = world.view(&state, &candidates, &query).unwrap();
        assert_eq!(open.leaves, [0, 1, 2]);
        assert_eq!(
            open.candidates,
            [
                CandidateId {
                    kind: CandidateKind::BrushModel,
                    index: 8
                },
                CandidateId {
                    kind: CandidateKind::Entity,
                    index: 3
                },
            ]
        );
        assert_ne!(closed.cache_identity, open.cache_identity);
        assert_eq!(open, world.view(&state, &candidates, &query).unwrap());

        let sky = world
            .view(
                &state,
                &candidates,
                &ViewQuery {
                    origins: vec![[1.0, -1.0, 0.0], [-1.0, 0.0, 0.0]],
                    bypass_pvs: false,
                },
            )
            .unwrap();
        assert_eq!(sky.sky, SkyVisibility::Sky3d);

        let mut cache = ViewCache::new(1, 4_096).unwrap();
        cache.insert(closed.clone()).unwrap();
        assert_eq!(cache.get(&closed.cache_identity), Some(&closed));
        cache.insert(open.clone()).unwrap();
        assert!(cache.get(&closed.cache_identity).is_none());
        assert_eq!(cache.get(&open.cache_identity), Some(&open));
    }

    #[test]
    fn outside_world_and_bypass_remain_distinct_cache_inputs() {
        let mut world = view_world();
        world.leaves[0].cluster = -1;
        world.identity = world_identity(&world);
        let state = AreaState::new(&world);
        let candidates = CandidateSet::compile(
            &world,
            0,
            &[CandidateInput {
                id: CandidateId {
                    kind: CandidateKind::StaticProp,
                    index: 1,
                },
                membership: CandidateMembership::CompiledLeaves(Vec::new()),
                bounds: None,
            }],
        )
        .unwrap();
        let outside = world
            .view(
                &state,
                &candidates,
                &ViewQuery {
                    origins: vec![[1.0, 1.0, 0.0]],
                    bypass_pvs: false,
                },
            )
            .unwrap();
        assert!(outside.outside_world);
        assert_eq!(
            outside.candidates,
            [CandidateId {
                kind: CandidateKind::StaticProp,
                index: 1
            }]
        );
        let bypass = world
            .view(
                &state,
                &candidates,
                &ViewQuery {
                    origins: vec![[1.0, -1.0, 0.0]],
                    bypass_pvs: true,
                },
            )
            .unwrap();
        assert!(!bypass.outside_world);
        assert!(bypass.candidates.is_empty());
        assert_ne!(outside.cache_identity, bypass.cache_identity);
    }

    #[test]
    fn no_vis_rows_expose_clusters_without_stored_expansion() {
        let mut world = view_world();
        world.visibility_mode = VisibilityMode::NoVis;
        world.pvs.clear();
        world.pas.clear();
        world.identity = world_identity(&world);
        assert!(world.visible(0, 2));
        assert!(world.audible(2, 0));
        let result = world
            .view(
                &AreaState::new(&world),
                &CandidateSet::compile(&world, 0, &[]).unwrap(),
                &ViewQuery {
                    origins: vec![[1.0, 1.0, 0.0]],
                    bypass_pvs: false,
                },
            )
            .unwrap();
        assert_eq!(result.merged_pvs, [0b111]);
        assert_eq!(result.leaves, [0, 1]);
    }
}
