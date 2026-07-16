use std::{fmt, ops::Range};

const MDL_HEADER_BYTES: usize = 408;
const BONE_BYTES: usize = 216;
const ANIMATION_BYTES: usize = 100;
const SEQUENCE_BYTES: usize = 212;
const TEXTURE_BYTES: usize = 64;
const BODY_PART_BYTES: usize = 16;
const MODEL_BYTES: usize = 148;
const MESH_BYTES: usize = 116;
const ATTACHMENT_BYTES: usize = 92;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    SourcePcMdl44,
    SourcePcMdl45,
    SourcePcMdl46,
    SourcePcMdl47,
    SourcePcMdl48,
}

impl Profile {
    fn version(self) -> i32 {
        match self {
            Self::SourcePcMdl44 => 44,
            Self::SourcePcMdl45 => 45,
            Self::SourcePcMdl46 => 46,
            Self::SourcePcMdl47 => 47,
            Self::SourcePcMdl48 => 48,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VtxVariant {
    Plain,
    Dx80,
    Dx90,
    Software,
}

impl VtxVariant {
    fn suffix(self) -> &'static str {
        match self {
            Self::Plain => ".vtx",
            Self::Dx80 => ".dx80.vtx",
            Self::Dx90 => ".dx90.vtx",
            Self::Software => ".sw.vtx",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_file_bytes: usize,
    pub max_aggregate_input_bytes: usize,
    pub max_dependency_count: usize,
    pub max_include_depth: usize,
    pub max_records: usize,
    pub max_strings: usize,
    pub max_string_bytes: usize,
    pub max_owned_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 256 * 1024 * 1024,
            max_aggregate_input_bytes: 1024 * 1024 * 1024,
            max_dependency_count: 4_096,
            max_include_depth: 64,
            max_records: 4_000_000,
            max_strings: 262_144,
            max_string_bytes: 4_096,
            max_owned_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyRole {
    VertexData,
    Topology,
    AnimationBlocks,
    IncludeModel,
    Physics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRequest {
    pub requester: String,
    pub role: DependencyRole,
    pub logical_path: String,
    pub expected_checksum: i32,
    pub dependency_chain: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyResponse {
    pub requester: String,
    pub role: DependencyRole,
    pub logical_path: String,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Float32(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Vector3(pub [Float32; 3]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bounds {
    pub eye: Vector3,
    pub illumination: Vector3,
    pub hull_min: Vector3,
    pub hull_max: Vector3,
    pub view_min: Vector3,
    pub view_max: Vector3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bone {
    pub index: usize,
    pub name: Vec<u8>,
    pub parent: i32,
    pub position: Vector3,
    pub quaternion: [Float32; 4],
    pub rotation: Vector3,
    pub position_scale: Vector3,
    pub rotation_scale: Vector3,
    pub pose_to_bone: [Float32; 12],
    pub alignment: [Float32; 4],
    pub flags: i32,
    pub physics_bone: i32,
    pub surface_property: Vec<u8>,
    pub contents: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Animation {
    pub index: usize,
    pub name: Vec<u8>,
    pub fps: Float32,
    pub flags: i32,
    pub frame_count: i32,
    pub movement_count: i32,
    pub animation_block: i32,
    pub animation_offset: i32,
    pub ik_rule_count: i32,
    pub local_hierarchy_count: i32,
    pub section_offset: i32,
    pub section_frame_count: i32,
    pub zero_frame_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sequence {
    pub index: usize,
    pub label: Vec<u8>,
    pub activity_name: Vec<u8>,
    pub flags: i32,
    pub activity: i32,
    pub activity_weight: i32,
    pub event_count: i32,
    pub bounds_min: Vector3,
    pub bounds_max: Vector3,
    pub blend_size: [i32; 2],
    pub next_sequence: i32,
    pub pose: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Material {
    pub index: usize,
    pub name: Vec<u8>,
    pub search_paths: Vec<Vec<u8>>,
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkinFamily {
    pub index: usize,
    pub texture_indices: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mesh {
    pub index: usize,
    pub material_slot: i32,
    pub model_index: i32,
    pub vertex_count: i32,
    pub vertex_offset: i32,
    pub flex_count: i32,
    pub material_type: i32,
    pub material_parameter: i32,
    pub mesh_id: i32,
    pub center: Vector3,
    pub lod_vertex_counts: [i32; 8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubModel {
    pub index: usize,
    pub name: Vec<u8>,
    pub mesh_count: i32,
    pub vertex_count: i32,
    pub vertex_offset_bytes: i32,
    pub tangent_offset_bytes: i32,
    pub attachment_count: i32,
    pub eyeball_count: i32,
    pub meshes: Vec<Mesh>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyPart {
    pub index: usize,
    pub name: Vec<u8>,
    pub base: i32,
    pub models: Vec<SubModel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    pub index: usize,
    pub name: Vec<u8>,
    pub flags: u32,
    pub bone: i32,
    pub local: [Float32; 12],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionSummary {
    pub vvd_lod_vertex_counts: Vec<i32>,
    pub vvd_fixup_count: i32,
    pub vvd_vertex_offset: i32,
    pub vvd_tangent_offset: i32,
    pub vtx_lod_count: i32,
    pub vtx_body_part_count: i32,
    pub vtx_max_bones_per_vertex: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vertex {
    pub source_index: usize,
    pub weights: [Float32; 3],
    pub bones: [u8; 3],
    pub bone_count: u8,
    pub position: Vector3,
    pub normal: Vector3,
    pub uv: [Float32; 2],
    pub tangent: [Float32; 4],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Strip {
    pub index_count: usize,
    pub first_index: usize,
    pub vertex_count: usize,
    pub first_vertex: usize,
    pub flags: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeometryPrimitive {
    pub body_part: usize,
    pub model: usize,
    pub lod: usize,
    pub mesh: usize,
    pub strip_group: usize,
    pub switch_point: Float32,
    pub material_slot: usize,
    pub source_vertex_ids: Vec<usize>,
    pub vertices: Vec<Vertex>,
    pub encoded_indices: Vec<u16>,
    pub strips: Vec<Strip>,
    pub triangles: Vec<[u32; 3]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicsStatus {
    Present,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub identity: String,
    pub profile: Profile,
    pub checksum: i32,
    pub internal_name: Vec<u8>,
    pub declared_length: usize,
    pub flags: i32,
    pub bounds: Bounds,
    pub bones: Vec<Bone>,
    pub animations: Vec<Animation>,
    pub sequences: Vec<Sequence>,
    pub materials: Vec<Material>,
    pub skins: Vec<SkinFamily>,
    pub body_parts: Vec<BodyPart>,
    pub attachments: Vec<Attachment>,
    pub include_models: Vec<String>,
    pub animation_blocks: Vec<Range<usize>>,
    pub companions: CompanionSummary,
    pub physics_status: PhysicsStatus,
    pub source_identities: Vec<String>,
    pub geometry: Vec<GeometryPrimitive>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Load {
    Needs(Vec<DependencyRequest>),
    Complete(Box<Document>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Malformed,
    Unsupported,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidLimits,
    InvalidIdentity,
    InputLimit,
    InvalidSignature,
    ProfileMismatch,
    DeclaredLength,
    InvalidCount,
    InvalidRange,
    InvalidString,
    InvalidReference,
    DependencyLimit,
    IncludeCycle,
    MissingDependency,
    ChecksumMismatch,
    UnsupportedCompanion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub classification: Classification,
    pub code: ErrorCode,
    pub identity: String,
    pub range: Option<Range<usize>>,
    pub dependency_chain: Vec<String>,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?}: {:?} in {}",
            self.classification, self.code, self.identity
        )
    }
}

impl std::error::Error for Error {}

struct Mdl {
    document: Document,
    needs_animation: bool,
}

struct ParsedVvd {
    counts: Vec<i32>,
    fixups: Vec<VvdFixup>,
    vertices: Vec<Vertex>,
    vertex_offset: i32,
    tangent_offset: i32,
}

#[derive(Clone, Copy)]
struct VvdFixup {
    lod: usize,
    source: usize,
    destination: usize,
    count: usize,
}

impl ParsedVvd {
    fn vertex(&self, lod: usize, destination: usize) -> Option<Vertex> {
        let source = if self.fixups.is_empty() {
            destination
        } else {
            let fixup = self.fixups.iter().find(|fixup| {
                fixup.lod >= lod
                    && destination >= fixup.destination
                    && destination < fixup.destination + fixup.count
            })?;
            fixup.source + destination - fixup.destination
        };
        self.vertices.get(source).cloned()
    }
}

struct ParsedVtx {
    lod_count: i32,
    body_parts: Vec<VtxBodyPart>,
    max_bones: i32,
}
struct VtxBodyPart {
    models: Vec<VtxModel>,
}
struct VtxModel {
    lods: Vec<VtxLod>,
}
struct VtxLod {
    switch_point: Float32,
    meshes: Vec<VtxMesh>,
}
struct VtxMesh {
    groups: Vec<VtxGroup>,
}
struct VtxGroup {
    source_vertex_ids: Vec<usize>,
    indices: Vec<u16>,
    strips: Vec<Strip>,
    triangles: Vec<[u32; 3]>,
}

pub fn load(
    identity: impl Into<String>,
    profile: Profile,
    vtx_variant: VtxVariant,
    mdl_bytes: &[u8],
    responses: &[DependencyResponse],
    limits: Limits,
) -> Result<Load, Error> {
    validate_limits(limits)?;
    let identity = identity.into();
    validate_model_identity(&identity)?;
    let mut aggregate = mdl_bytes.len();
    let root = parse_mdl(&identity, profile, mdl_bytes, limits)?;
    let stem = identity
        .strip_suffix(".mdl")
        .expect("validated model suffix");
    let mut requests = Vec::new();
    if !root.document.body_parts.is_empty() {
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::VertexData,
            logical_path: format!("{stem}.vvd"),
            expected_checksum: root.document.checksum,
            dependency_chain: vec![identity.clone()],
        });
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::Topology,
            logical_path: format!("{stem}{}", vtx_variant.suffix()),
            expected_checksum: root.document.checksum,
            dependency_chain: vec![identity.clone()],
        });
    }
    if root.needs_animation {
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::AnimationBlocks,
            logical_path: format!("{stem}.ani"),
            expected_checksum: root.document.checksum,
            dependency_chain: vec![identity.clone()],
        });
    }
    requests.push(DependencyRequest {
        requester: identity.clone(),
        role: DependencyRole::Physics,
        logical_path: format!("{stem}.phy"),
        expected_checksum: root.document.checksum,
        dependency_chain: vec![identity.clone()],
    });
    for include in &root.document.include_models {
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::IncludeModel,
            logical_path: include.clone(),
            expected_checksum: root.document.checksum,
            dependency_chain: vec![identity.clone()],
        });
    }
    if requests.len() > limits.max_dependency_count {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::DependencyLimit,
            &identity,
            None,
        ));
    }
    let mut missing_requests = Vec::new();
    for request in &requests {
        if !responses
            .iter()
            .any(|response| response_matches(response, request))
        {
            missing_requests.push(request.clone());
        }
    }
    if !missing_requests.is_empty() {
        return Ok(Load::Needs(missing_requests));
    }

    let mut document = root.document;
    let mut source_identities = vec![identity.clone()];
    let mut physics_status = PhysicsStatus::Missing;
    let mut parsed_vvd = None;
    let mut parsed_vtx = None;
    for request in requests {
        let response = responses
            .iter()
            .find(|response| response_matches(response, &request))
            .expect("all requests matched");
        let Some(bytes) = &response.bytes else {
            if request.role == DependencyRole::Physics {
                continue;
            }
            return Err(Error {
                classification: Classification::Missing,
                code: ErrorCode::MissingDependency,
                identity: request.logical_path,
                range: None,
                dependency_chain: request.dependency_chain,
            });
        };
        aggregate = aggregate.checked_add(bytes.len()).ok_or_else(|| {
            failure(
                Classification::Malformed,
                ErrorCode::InputLimit,
                &response.logical_path,
                None,
            )
        })?;
        if bytes.len() > limits.max_file_bytes || aggregate > limits.max_aggregate_input_bytes {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::InputLimit,
                &response.logical_path,
                None,
            ));
        }
        match request.role {
            DependencyRole::VertexData => {
                let parsed = parse_vvd(&response.logical_path, bytes, document.checksum, limits)?;
                document.companions.vvd_lod_vertex_counts = parsed.counts.clone();
                document.companions.vvd_fixup_count = parsed.fixups.len() as i32;
                document.companions.vvd_vertex_offset = parsed.vertex_offset;
                document.companions.vvd_tangent_offset = parsed.tangent_offset;
                parsed_vvd = Some(parsed);
            }
            DependencyRole::Topology => {
                let parsed = parse_vtx(
                    &response.logical_path,
                    bytes,
                    document.checksum,
                    document.body_parts.len(),
                    limits,
                )?;
                document.companions.vtx_lod_count = parsed.lod_count;
                document.companions.vtx_body_part_count = parsed.body_parts.len() as i32;
                document.companions.vtx_max_bones_per_vertex = parsed.max_bones;
                parsed_vtx = Some(parsed);
            }
            DependencyRole::AnimationBlocks => {
                if bytes.len() < 12 || bytes.get(..4) != Some(b"IDAG") {
                    return Err(failure(
                        Classification::Malformed,
                        ErrorCode::InvalidSignature,
                        &response.logical_path,
                        Some(0..bytes.len().min(4)),
                    ));
                }
                if i32_at(bytes, 4, &response.logical_path)? != profile.version()
                    || i32_at(bytes, 8, &response.logical_path)? != document.checksum
                {
                    return Err(failure(
                        Classification::Malformed,
                        ErrorCode::ChecksumMismatch,
                        &response.logical_path,
                        Some(4..12),
                    ));
                }
            }
            DependencyRole::IncludeModel => {
                let include_profile = profile_for_version(
                    i32_at(bytes, 4, &response.logical_path)?,
                    &response.logical_path,
                )?;
                let include = parse_mdl(&response.logical_path, include_profile, bytes, limits)?;
                if include.document.identity.eq_ignore_ascii_case(&identity) {
                    return Err(failure(
                        Classification::Malformed,
                        ErrorCode::IncludeCycle,
                        &response.logical_path,
                        None,
                    ));
                }
            }
            DependencyRole::Physics => physics_status = PhysicsStatus::Present,
        }
        source_identities.push(response.logical_path.clone());
    }
    document.source_identities = source_identities;
    document.physics_status = physics_status;
    if let (Some(vvd), Some(vtx)) = (&parsed_vvd, &parsed_vtx) {
        document.geometry = assemble_geometry(&document.body_parts, vvd, vtx, &identity)?;
    }
    Ok(Load::Complete(Box::new(document)))
}

fn parse_mdl(identity: &str, profile: Profile, bytes: &[u8], limits: Limits) -> Result<Mdl, Error> {
    if bytes.len() > limits.max_file_bytes {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InputLimit,
            identity,
            None,
        ));
    }
    range(bytes, 0, MDL_HEADER_BYTES, identity)?;
    if bytes.get(..4) != Some(b"IDST") {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidSignature,
            identity,
            Some(0..4),
        ));
    }
    if i32_at(bytes, 4, identity)? != profile.version() {
        return Err(failure(
            Classification::Unsupported,
            ErrorCode::ProfileMismatch,
            identity,
            Some(4..8),
        ));
    }
    let declared_length = count(bytes, 76, identity)?;
    if declared_length < MDL_HEADER_BYTES || declared_length > bytes.len() {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::DeclaredLength,
            identity,
            Some(76..80),
        ));
    }
    let bytes = &bytes[..declared_length];
    let checksum = i32_at(bytes, 8, identity)?;
    let internal_name = fixed_string(&bytes[12..76], limits, identity)?;
    let flags = i32_at(bytes, 152, identity)?;
    let bounds = Bounds {
        eye: vector3(bytes, 80, identity)?,
        illumination: vector3(bytes, 92, identity)?,
        hull_min: vector3(bytes, 104, identity)?,
        hull_max: vector3(bytes, 116, identity)?,
        view_min: vector3(bytes, 128, identity)?,
        view_max: vector3(bytes, 140, identity)?,
    };

    let bone_count = count(bytes, 156, identity)?;
    let bone_offset = count(bytes, 160, identity)?;
    table(bytes, bone_offset, bone_count, BONE_BYTES, limits, identity)?;
    if bone_count > 128 {
        return Err(invalid_count(identity, 156));
    }
    let mut bones = Vec::with_capacity(bone_count);
    for index in 0..bone_count {
        let offset = bone_offset + index * BONE_BYTES;
        let parent = i32_at(bytes, offset + 4, identity)?;
        if parent < -1 || parent >= index as i32 {
            return Err(invalid_reference(identity, offset + 4));
        }
        let surface_relative = i32_at(bytes, offset + 164, identity)?;
        bones.push(Bone {
            index,
            name: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset, identity)?,
                limits,
                identity,
            )?,
            parent,
            position: vector3(bytes, offset + 32, identity)?,
            quaternion: float4(bytes, offset + 44, identity)?,
            rotation: vector3(bytes, offset + 60, identity)?,
            position_scale: vector3(bytes, offset + 72, identity)?,
            rotation_scale: vector3(bytes, offset + 84, identity)?,
            pose_to_bone: float12(bytes, offset + 96, identity)?,
            alignment: float4(bytes, offset + 144, identity)?,
            flags: i32_at(bytes, offset + 160, identity)?,
            physics_bone: i32_at(bytes, offset + 168, identity)?,
            surface_property: if surface_relative == 0 {
                Vec::new()
            } else {
                relative_string(bytes, offset, surface_relative, limits, identity)?
            },
            contents: i32_at(bytes, offset + 180, identity)?,
        });
    }

    let animation_count = count(bytes, 180, identity)?;
    let animation_offset = count(bytes, 184, identity)?;
    table(
        bytes,
        animation_offset,
        animation_count,
        ANIMATION_BYTES,
        limits,
        identity,
    )?;
    let mut animations = Vec::with_capacity(animation_count);
    let mut needs_animation = false;
    for index in 0..animation_count {
        let offset = animation_offset + index * ANIMATION_BYTES;
        let animation_block = i32_at(bytes, offset + 52, identity)?;
        needs_animation |= animation_block > 0;
        animations.push(Animation {
            index,
            name: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset + 4, identity)?,
                limits,
                identity,
            )?,
            fps: float(bytes, offset + 8, identity)?,
            flags: i32_at(bytes, offset + 12, identity)?,
            frame_count: i32_at(bytes, offset + 16, identity)?,
            movement_count: i32_at(bytes, offset + 20, identity)?,
            animation_block,
            animation_offset: i32_at(bytes, offset + 56, identity)?,
            ik_rule_count: i32_at(bytes, offset + 60, identity)?,
            local_hierarchy_count: i32_at(bytes, offset + 72, identity)?,
            section_offset: i32_at(bytes, offset + 80, identity)?,
            section_frame_count: i32_at(bytes, offset + 84, identity)?,
            zero_frame_count: if profile.version() >= 47 {
                u16_at(bytes, offset + 90, identity)?
            } else {
                0
            },
        });
    }

    let sequence_count = count(bytes, 188, identity)?;
    let sequence_offset = count(bytes, 192, identity)?;
    table(
        bytes,
        sequence_offset,
        sequence_count,
        SEQUENCE_BYTES,
        limits,
        identity,
    )?;
    let mut sequences = Vec::with_capacity(sequence_count);
    for index in 0..sequence_count {
        let offset = sequence_offset + index * SEQUENCE_BYTES;
        sequences.push(Sequence {
            index,
            label: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset + 4, identity)?,
                limits,
                identity,
            )?,
            activity_name: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset + 8, identity)?,
                limits,
                identity,
            )?,
            flags: i32_at(bytes, offset + 12, identity)?,
            activity: i32_at(bytes, offset + 16, identity)?,
            activity_weight: i32_at(bytes, offset + 20, identity)?,
            event_count: i32_at(bytes, offset + 24, identity)?,
            bounds_min: vector3(bytes, offset + 32, identity)?,
            bounds_max: vector3(bytes, offset + 44, identity)?,
            blend_size: [
                i32_at(bytes, offset + 68, identity)?,
                i32_at(bytes, offset + 72, identity)?,
            ],
            next_sequence: i32_at(bytes, offset + 156, identity)?,
            pose: i32_at(bytes, offset + 160, identity)?,
        });
    }

    let search_path_count = count(bytes, 212, identity)?;
    let search_path_offset = count(bytes, 216, identity)?;
    table(
        bytes,
        search_path_offset,
        search_path_count,
        4,
        limits,
        identity,
    )?;
    let mut search_paths = Vec::with_capacity(search_path_count);
    for index in 0..search_path_count {
        search_paths.push(c_string(
            bytes,
            count(bytes, search_path_offset + index * 4, identity)?,
            limits,
            identity,
        )?);
    }
    let texture_count = count(bytes, 204, identity)?;
    let texture_offset = count(bytes, 208, identity)?;
    table(
        bytes,
        texture_offset,
        texture_count,
        TEXTURE_BYTES,
        limits,
        identity,
    )?;
    let mut materials = Vec::with_capacity(texture_count);
    for index in 0..texture_count {
        let offset = texture_offset + index * TEXTURE_BYTES;
        let name = relative_string(
            bytes,
            offset,
            i32_at(bytes, offset, identity)?,
            limits,
            identity,
        )?;
        let name_text = String::from_utf8_lossy(&name)
            .trim_end_matches(".vmt")
            .to_owned();
        let candidates = search_paths
            .iter()
            .map(|path| {
                let path = String::from_utf8_lossy(path)
                    .trim_matches('/')
                    .to_ascii_lowercase();
                if path.is_empty() {
                    format!("materials/{name_text}.vmt").to_ascii_lowercase()
                } else {
                    format!("materials/{path}/{name_text}.vmt").to_ascii_lowercase()
                }
            })
            .collect();
        materials.push(Material {
            index,
            name,
            search_paths: search_paths.clone(),
            candidates,
        });
    }

    let skin_reference_count = count(bytes, 220, identity)?;
    let skin_family_count = count(bytes, 224, identity)?;
    let skin_offset = count(bytes, 228, identity)?;
    let skin_entries = skin_reference_count
        .checked_mul(skin_family_count)
        .ok_or_else(|| invalid_range(identity, skin_offset))?;
    table(bytes, skin_offset, skin_entries, 2, limits, identity)?;
    let mut skins = Vec::with_capacity(skin_family_count);
    for family in 0..skin_family_count {
        let mut texture_indices = Vec::with_capacity(skin_reference_count);
        for slot in 0..skin_reference_count {
            let value = u16_at(
                bytes,
                skin_offset + (family * skin_reference_count + slot) * 2,
                identity,
            )?;
            if value as usize >= texture_count {
                return Err(invalid_reference(identity, skin_offset));
            }
            texture_indices.push(value);
        }
        skins.push(SkinFamily {
            index: family,
            texture_indices,
        });
    }

    let body_part_count = count(bytes, 232, identity)?;
    let body_part_offset = count(bytes, 236, identity)?;
    table(
        bytes,
        body_part_offset,
        body_part_count,
        BODY_PART_BYTES,
        limits,
        identity,
    )?;
    let mut body_parts = Vec::with_capacity(body_part_count);
    for body_index in 0..body_part_count {
        let offset = body_part_offset + body_index * BODY_PART_BYTES;
        let model_count = count(bytes, offset + 4, identity)?;
        let model_offset =
            relative_offset(offset, i32_at(bytes, offset + 12, identity)?, identity)?;
        table(
            bytes,
            model_offset,
            model_count,
            MODEL_BYTES,
            limits,
            identity,
        )?;
        let mut models = Vec::with_capacity(model_count);
        for model_index in 0..model_count {
            let model_offset = model_offset + model_index * MODEL_BYTES;
            let mesh_count = count(bytes, model_offset + 72, identity)?;
            let mesh_offset = relative_offset(
                model_offset,
                i32_at(bytes, model_offset + 76, identity)?,
                identity,
            )?;
            table(bytes, mesh_offset, mesh_count, MESH_BYTES, limits, identity)?;
            let mut meshes = Vec::with_capacity(mesh_count);
            for mesh_index in 0..mesh_count {
                let mesh_offset = mesh_offset + mesh_index * MESH_BYTES;
                let material_slot = i32_at(bytes, mesh_offset, identity)?;
                if material_slot < 0 || material_slot as usize >= skin_reference_count {
                    return Err(invalid_reference(identity, mesh_offset));
                }
                let mut lod_vertex_counts = [0; 8];
                for (lod, output) in lod_vertex_counts.iter_mut().enumerate() {
                    *output = i32_at(bytes, mesh_offset + 52 + lod * 4, identity)?;
                }
                meshes.push(Mesh {
                    index: mesh_index,
                    material_slot,
                    model_index: i32_at(bytes, mesh_offset + 4, identity)?,
                    vertex_count: i32_at(bytes, mesh_offset + 8, identity)?,
                    vertex_offset: i32_at(bytes, mesh_offset + 12, identity)?,
                    flex_count: i32_at(bytes, mesh_offset + 16, identity)?,
                    material_type: i32_at(bytes, mesh_offset + 24, identity)?,
                    material_parameter: i32_at(bytes, mesh_offset + 28, identity)?,
                    mesh_id: i32_at(bytes, mesh_offset + 32, identity)?,
                    center: vector3(bytes, mesh_offset + 36, identity)?,
                    lod_vertex_counts,
                });
            }
            models.push(SubModel {
                index: model_index,
                name: fixed_string(&bytes[model_offset..model_offset + 64], limits, identity)?,
                mesh_count: mesh_count as i32,
                vertex_count: i32_at(bytes, model_offset + 80, identity)?,
                vertex_offset_bytes: i32_at(bytes, model_offset + 84, identity)?,
                tangent_offset_bytes: i32_at(bytes, model_offset + 88, identity)?,
                attachment_count: i32_at(bytes, model_offset + 92, identity)?,
                eyeball_count: i32_at(bytes, model_offset + 100, identity)?,
                meshes,
            });
        }
        body_parts.push(BodyPart {
            index: body_index,
            name: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset, identity)?,
                limits,
                identity,
            )?,
            base: i32_at(bytes, offset + 8, identity)?,
            models,
        });
    }

    let attachment_count = count(bytes, 240, identity)?;
    let attachment_offset = count(bytes, 244, identity)?;
    table(
        bytes,
        attachment_offset,
        attachment_count,
        ATTACHMENT_BYTES,
        limits,
        identity,
    )?;
    let mut attachments = Vec::with_capacity(attachment_count);
    for index in 0..attachment_count {
        let offset = attachment_offset + index * ATTACHMENT_BYTES;
        let bone = i32_at(bytes, offset + 8, identity)?;
        if bone < 0 || bone as usize >= bone_count {
            return Err(invalid_reference(identity, offset + 8));
        }
        attachments.push(Attachment {
            index,
            name: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset, identity)?,
                limits,
                identity,
            )?,
            flags: u32_at(bytes, offset + 4, identity)?,
            bone,
            local: float12(bytes, offset + 12, identity)?,
        });
    }

    let include_count = count(bytes, 336, identity)?;
    let include_offset = count(bytes, 340, identity)?;
    table(bytes, include_offset, include_count, 8, limits, identity)?;
    let mut include_models = Vec::with_capacity(include_count);
    for index in 0..include_count {
        let offset = include_offset + index * 8;
        let stored = relative_string(
            bytes,
            offset,
            i32_at(bytes, offset + 4, identity)?,
            limits,
            identity,
        )?;
        include_models.push(canonical_model_path(&stored, identity)?);
    }

    let animation_block_count = count(bytes, 352, identity)?;
    let animation_block_offset = count(bytes, 356, identity)?;
    table(
        bytes,
        animation_block_offset,
        animation_block_count,
        8,
        limits,
        identity,
    )?;
    let mut animation_blocks = Vec::with_capacity(animation_block_count);
    for index in 0..animation_block_count {
        let offset = animation_block_offset + index * 8;
        let start = count(bytes, offset, identity)?;
        let end = count(bytes, offset + 4, identity)?;
        if end < start {
            return Err(invalid_range(identity, offset));
        }
        animation_blocks.push(start..end);
    }
    needs_animation |= animation_block_count > 0;

    let document = Document {
        identity: identity.to_owned(),
        profile,
        checksum,
        internal_name,
        declared_length,
        flags,
        bounds,
        bones,
        animations,
        sequences,
        materials,
        skins,
        body_parts,
        attachments,
        include_models,
        animation_blocks,
        companions: CompanionSummary {
            vvd_lod_vertex_counts: Vec::new(),
            vvd_fixup_count: 0,
            vvd_vertex_offset: 0,
            vvd_tangent_offset: 0,
            vtx_lod_count: 0,
            vtx_body_part_count: 0,
            vtx_max_bones_per_vertex: 0,
        },
        physics_status: PhysicsStatus::Missing,
        source_identities: vec![identity.to_owned()],
        geometry: Vec::new(),
    };
    if owned_bytes(&document) > limits.max_owned_bytes {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InputLimit,
            identity,
            None,
        ));
    }
    Ok(Mdl {
        document,
        needs_animation,
    })
}

fn parse_vvd(
    identity: &str,
    bytes: &[u8],
    checksum: i32,
    limits: Limits,
) -> Result<ParsedVvd, Error> {
    range(bytes, 0, 64, identity)?;
    if bytes.get(..4) != Some(b"IDSV") {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidSignature,
            identity,
            Some(0..4),
        ));
    }
    if i32_at(bytes, 4, identity)? != 4 {
        return Err(failure(
            Classification::Unsupported,
            ErrorCode::UnsupportedCompanion,
            identity,
            Some(4..8),
        ));
    }
    if i32_at(bytes, 8, identity)? != checksum {
        return Err(checksum_error(identity));
    }
    let lod_count = count(bytes, 12, identity)?;
    if lod_count == 0 || lod_count > 8 {
        return Err(invalid_count(identity, 12));
    }
    let mut counts = Vec::with_capacity(lod_count);
    for lod in 0..lod_count {
        let value = i32_at(bytes, 16 + lod * 4, identity)?;
        if value < 0 || (lod > 0 && value > counts[lod - 1]) {
            return Err(invalid_count(identity, 16 + lod * 4));
        }
        counts.push(value);
    }
    let fixup_count = i32_at(bytes, 48, identity)?;
    let fixup_offset = count(bytes, 52, identity)?;
    if fixup_count < 0 {
        return Err(invalid_count(identity, 48));
    }
    table(
        bytes,
        fixup_offset,
        fixup_count as usize,
        12,
        limits,
        identity,
    )?;
    let vertex_offset = i32_at(bytes, 56, identity)?;
    let tangent_offset = i32_at(bytes, 60, identity)?;
    let vertices = counts[0] as usize;
    table(
        bytes,
        vertex_offset as usize,
        vertices,
        48,
        limits,
        identity,
    )?;
    table(
        bytes,
        tangent_offset as usize,
        vertices,
        16,
        limits,
        identity,
    )?;
    let mut vertices_out = Vec::with_capacity(vertices);
    for index in 0..vertices {
        let vertex = vertex_offset as usize + index * 48;
        let tangent = tangent_offset as usize + index * 16;
        let bone_count = bytes[vertex + 15];
        if bone_count > 3 {
            return Err(invalid_count(identity, vertex + 15));
        }
        vertices_out.push(Vertex {
            source_index: index,
            weights: [
                float(bytes, vertex, identity)?,
                float(bytes, vertex + 4, identity)?,
                float(bytes, vertex + 8, identity)?,
            ],
            bones: bytes[vertex + 12..vertex + 15]
                .try_into()
                .expect("VVD bone indexes"),
            bone_count,
            position: vector3(bytes, vertex + 16, identity)?,
            normal: vector3(bytes, vertex + 28, identity)?,
            uv: [
                float(bytes, vertex + 40, identity)?,
                float(bytes, vertex + 44, identity)?,
            ],
            tangent: float4(bytes, tangent, identity)?,
        });
    }
    let mut fixups = Vec::with_capacity(fixup_count as usize);
    let mut destination = 0_usize;
    for index in 0..fixup_count as usize {
        let offset = fixup_offset + index * 12;
        let lod = count(bytes, offset, identity)?;
        let source = count(bytes, offset + 4, identity)?;
        let count = count(bytes, offset + 8, identity)?;
        if lod >= lod_count || source.checked_add(count).is_none_or(|end| end > vertices) {
            return Err(invalid_reference(identity, offset));
        }
        fixups.push(VvdFixup {
            lod,
            source,
            destination,
            count,
        });
        destination = destination
            .checked_add(count)
            .ok_or_else(|| invalid_range(identity, offset))?;
    }
    for (lod, &expected) in counts.iter().enumerate() {
        let produced = if fixups.is_empty() {
            expected as usize
        } else {
            fixups
                .iter()
                .filter(|fixup| fixup.lod >= lod)
                .map(|fixup| fixup.count)
                .sum()
        };
        if produced != expected as usize {
            return Err(invalid_reference(identity, fixup_offset));
        }
    }
    Ok(ParsedVvd {
        counts,
        fixups,
        vertices: vertices_out,
        vertex_offset,
        tangent_offset,
    })
}

fn parse_vtx(
    identity: &str,
    bytes: &[u8],
    checksum: i32,
    expected_body_parts: usize,
    limits: Limits,
) -> Result<ParsedVtx, Error> {
    range(bytes, 0, 36, identity)?;
    if i32_at(bytes, 0, identity)? != 7 {
        return Err(failure(
            Classification::Unsupported,
            ErrorCode::UnsupportedCompanion,
            identity,
            Some(0..4),
        ));
    }
    let max_bones = i32_at(bytes, 12, identity)?;
    if !(1..=3).contains(&max_bones) {
        return Err(invalid_count(identity, 12));
    }
    if i32_at(bytes, 16, identity)? != checksum {
        return Err(checksum_error(identity));
    }
    let lod_count = i32_at(bytes, 20, identity)?;
    if !(1..=8).contains(&lod_count) {
        return Err(invalid_count(identity, 20));
    }
    let replacements = count(bytes, 24, identity)?;
    table(bytes, replacements, lod_count as usize, 8, limits, identity)?;
    let body_parts = i32_at(bytes, 28, identity)?;
    if body_parts < 0 || body_parts as usize != expected_body_parts {
        return Err(invalid_reference(identity, 28));
    }
    let body_offset = count(bytes, 32, identity)?;
    table(bytes, body_offset, body_parts as usize, 8, limits, identity)?;
    let mut parsed_body_parts = Vec::with_capacity(body_parts as usize);
    for body_index in 0..body_parts as usize {
        let body = body_offset + body_index * 8;
        let model_count = count(bytes, body, identity)?;
        let model_offset = relative_offset(body, i32_at(bytes, body + 4, identity)?, identity)?;
        table(bytes, model_offset, model_count, 8, limits, identity)?;
        let mut models = Vec::with_capacity(model_count);
        for model_index in 0..model_count {
            let model = model_offset + model_index * 8;
            let model_lods = count(bytes, model, identity)?;
            if model_lods != lod_count as usize {
                return Err(invalid_reference(identity, model));
            }
            let lod_offset = relative_offset(model, i32_at(bytes, model + 4, identity)?, identity)?;
            table(bytes, lod_offset, model_lods, 12, limits, identity)?;
            let mut lods = Vec::with_capacity(model_lods);
            for lod_index in 0..model_lods {
                let lod = lod_offset + lod_index * 12;
                let mesh_count = count(bytes, lod, identity)?;
                let mesh_offset =
                    relative_offset(lod, i32_at(bytes, lod + 4, identity)?, identity)?;
                table(bytes, mesh_offset, mesh_count, 9, limits, identity)?;
                let mut meshes = Vec::with_capacity(mesh_count);
                for mesh_index in 0..mesh_count {
                    let mesh = mesh_offset + mesh_index * 9;
                    let group_count = count(bytes, mesh, identity)?;
                    let group_offset =
                        relative_offset(mesh, i32_at(bytes, mesh + 4, identity)?, identity)?;
                    table(bytes, group_offset, group_count, 25, limits, identity)?;
                    let mut groups = Vec::with_capacity(group_count);
                    for group_index in 0..group_count {
                        let group = group_offset + group_index * 25;
                        let vertex_count = count(bytes, group, identity)?;
                        let vertex_offset =
                            relative_offset(group, i32_at(bytes, group + 4, identity)?, identity)?;
                        table(bytes, vertex_offset, vertex_count, 9, limits, identity)?;
                        let mut source_vertex_ids = Vec::with_capacity(vertex_count);
                        for vertex in 0..vertex_count {
                            source_vertex_ids.push(u16_at(
                                bytes,
                                vertex_offset + vertex * 9 + 4,
                                identity,
                            )? as usize);
                        }
                        let index_count = count(bytes, group + 8, identity)?;
                        let index_offset =
                            relative_offset(group, i32_at(bytes, group + 12, identity)?, identity)?;
                        table(bytes, index_offset, index_count, 2, limits, identity)?;
                        let mut indices = Vec::with_capacity(index_count);
                        for index in 0..index_count {
                            let value = u16_at(bytes, index_offset + index * 2, identity)?;
                            if value as usize >= vertex_count {
                                return Err(invalid_reference(identity, index_offset + index * 2));
                            }
                            indices.push(value);
                        }
                        let strip_count = count(bytes, group + 16, identity)?;
                        let strip_offset =
                            relative_offset(group, i32_at(bytes, group + 20, identity)?, identity)?;
                        table(bytes, strip_offset, strip_count, 27, limits, identity)?;
                        let mut strips = Vec::with_capacity(strip_count);
                        let mut triangles = Vec::new();
                        for strip_index in 0..strip_count {
                            let strip = strip_offset + strip_index * 27;
                            let strip_index_count = count(bytes, strip, identity)?;
                            let first = count(bytes, strip + 4, identity)?;
                            let vertex_count_in_strip = count(bytes, strip + 8, identity)?;
                            let first_vertex = count(bytes, strip + 12, identity)?;
                            if first
                                .checked_add(strip_index_count)
                                .is_none_or(|end| end > indices.len())
                                || first_vertex
                                    .checked_add(vertex_count_in_strip)
                                    .is_none_or(|end| end > vertex_count)
                            {
                                return Err(invalid_reference(identity, strip));
                            }
                            let flags = bytes[strip + 18];
                            let selected = &indices[first..first + strip_index_count];
                            match flags {
                                1 if strip_index_count.is_multiple_of(3) => {
                                    triangles.extend(selected.chunks_exact(3).filter_map(
                                        |triangle| {
                                            let value = [
                                                triangle[0] as u32,
                                                triangle[1] as u32,
                                                triangle[2] as u32,
                                            ];
                                            (value[0] != value[1]
                                                && value[1] != value[2]
                                                && value[0] != value[2])
                                                .then_some(value)
                                        },
                                    ));
                                }
                                2 => {
                                    for at in 0..strip_index_count.saturating_sub(2) {
                                        let mut value = [
                                            selected[at] as u32,
                                            selected[at + 1] as u32,
                                            selected[at + 2] as u32,
                                        ];
                                        if at % 2 == 1 {
                                            value.swap(0, 1);
                                        }
                                        if value[0] != value[1]
                                            && value[1] != value[2]
                                            && value[0] != value[2]
                                        {
                                            triangles.push(value);
                                        }
                                    }
                                }
                                _ => {
                                    return Err(failure(
                                        Classification::Unsupported,
                                        ErrorCode::UnsupportedCompanion,
                                        identity,
                                        Some(strip + 18..strip + 19),
                                    ));
                                }
                            }
                            strips.push(Strip {
                                index_count: strip_index_count,
                                first_index: first,
                                vertex_count: vertex_count_in_strip,
                                first_vertex,
                                flags,
                            });
                        }
                        groups.push(VtxGroup {
                            source_vertex_ids,
                            indices,
                            strips,
                            triangles,
                        });
                    }
                    meshes.push(VtxMesh { groups });
                }
                lods.push(VtxLod {
                    switch_point: float(bytes, lod + 8, identity)?,
                    meshes,
                });
            }
            models.push(VtxModel { lods });
        }
        parsed_body_parts.push(VtxBodyPart { models });
    }
    Ok(ParsedVtx {
        lod_count,
        body_parts: parsed_body_parts,
        max_bones,
    })
}

fn assemble_geometry(
    mdl: &[BodyPart],
    vvd: &ParsedVvd,
    vtx: &ParsedVtx,
    identity: &str,
) -> Result<Vec<GeometryPrimitive>, Error> {
    if mdl.len() != vtx.body_parts.len() {
        return Err(invalid_reference(identity, 0));
    }
    let mut output = Vec::new();
    for (body_index, (mdl_body, vtx_body)) in mdl.iter().zip(&vtx.body_parts).enumerate() {
        if mdl_body.models.len() != vtx_body.models.len() {
            return Err(invalid_reference(identity, body_index));
        }
        for (model_index, (mdl_model, vtx_model)) in
            mdl_body.models.iter().zip(&vtx_body.models).enumerate()
        {
            for (lod_index, lod) in vtx_model.lods.iter().enumerate() {
                if lod.meshes.len() != mdl_model.meshes.len() || lod_index >= vvd.counts.len() {
                    return Err(invalid_reference(identity, model_index));
                }
                for (mesh_index, (mdl_mesh, vtx_mesh)) in
                    mdl_model.meshes.iter().zip(&lod.meshes).enumerate()
                {
                    for (group_index, group) in vtx_mesh.groups.iter().enumerate() {
                        let mut vertices = Vec::with_capacity(group.source_vertex_ids.len());
                        for &mesh_vertex in &group.source_vertex_ids {
                            if mesh_vertex >= mdl_mesh.vertex_count as usize {
                                return Err(invalid_reference(identity, mesh_vertex));
                            }
                            let model_start = usize::try_from(mdl_model.vertex_offset_bytes)
                                .ok()
                                .filter(|value| value.is_multiple_of(48))
                                .map(|value| value / 48)
                                .ok_or_else(|| invalid_reference(identity, model_index))?;
                            let destination = model_start
                                .checked_add(mdl_mesh.vertex_offset as usize)
                                .and_then(|value| value.checked_add(mesh_vertex))
                                .ok_or_else(|| invalid_reference(identity, mesh_vertex))?;
                            vertices.push(
                                vvd.vertex(lod_index, destination)
                                    .ok_or_else(|| invalid_reference(identity, destination))?,
                            );
                        }
                        output.push(GeometryPrimitive {
                            body_part: body_index,
                            model: model_index,
                            lod: lod_index,
                            mesh: mesh_index,
                            strip_group: group_index,
                            switch_point: lod.switch_point,
                            material_slot: mdl_mesh.material_slot as usize,
                            source_vertex_ids: group.source_vertex_ids.clone(),
                            vertices,
                            encoded_indices: group.indices.clone(),
                            strips: group.strips.clone(),
                            triangles: group.triangles.clone(),
                        });
                    }
                }
            }
        }
    }
    Ok(output)
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    if limits.max_file_bytes == 0
        || limits.max_aggregate_input_bytes == 0
        || limits.max_dependency_count == 0
        || limits.max_include_depth == 0
        || limits.max_records == 0
        || limits.max_strings == 0
        || limits.max_string_bytes == 0
        || limits.max_owned_bytes == 0
    {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidLimits,
            "limits",
            None,
        ));
    }
    Ok(())
}

fn validate_model_identity(identity: &str) -> Result<(), Error> {
    if !identity.starts_with("models/")
        || !identity.ends_with(".mdl")
        || identity.contains('\\')
        || identity
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidIdentity,
            identity,
            None,
        ));
    }
    Ok(())
}

fn canonical_model_path(bytes: &[u8], identity: &str) -> Result<String, Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        failure(
            Classification::Malformed,
            ErrorCode::InvalidString,
            identity,
            None,
        )
    })?;
    let canonical = text.replace('\\', "/").to_ascii_lowercase();
    validate_model_identity(&canonical)?;
    Ok(canonical)
}

fn response_matches(response: &DependencyResponse, request: &DependencyRequest) -> bool {
    response.requester == request.requester
        && response.role == request.role
        && response.logical_path == request.logical_path
}

fn profile_for_version(version: i32, identity: &str) -> Result<Profile, Error> {
    match version {
        44 => Ok(Profile::SourcePcMdl44),
        45 => Ok(Profile::SourcePcMdl45),
        46 => Ok(Profile::SourcePcMdl46),
        47 => Ok(Profile::SourcePcMdl47),
        48 => Ok(Profile::SourcePcMdl48),
        _ => Err(failure(
            Classification::Unsupported,
            ErrorCode::ProfileMismatch,
            identity,
            Some(4..8),
        )),
    }
}

fn table(
    bytes: &[u8],
    offset: usize,
    count: usize,
    stride: usize,
    limits: Limits,
    identity: &str,
) -> Result<(), Error> {
    if count > limits.max_records {
        return Err(invalid_count(identity, offset));
    }
    let length = count
        .checked_mul(stride)
        .ok_or_else(|| invalid_range(identity, offset))?;
    range(bytes, offset, length, identity)
}

fn range(bytes: &[u8], offset: usize, length: usize, identity: &str) -> Result<(), Error> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid_range(identity, offset))?;
    if end > bytes.len() {
        return Err(invalid_range(identity, offset));
    }
    Ok(())
}

fn relative_offset(base: usize, relative: i32, identity: &str) -> Result<usize, Error> {
    if relative >= 0 {
        base.checked_add(relative as usize)
    } else {
        base.checked_sub(relative.unsigned_abs() as usize)
    }
    .ok_or_else(|| invalid_range(identity, base))
}

fn relative_string(
    bytes: &[u8],
    base: usize,
    relative: i32,
    limits: Limits,
    identity: &str,
) -> Result<Vec<u8>, Error> {
    let offset = relative_offset(base, relative, identity)?;
    c_string(bytes, offset, limits, identity)
}

fn c_string(bytes: &[u8], offset: usize, limits: Limits, identity: &str) -> Result<Vec<u8>, Error> {
    let remaining = bytes
        .get(offset..)
        .ok_or_else(|| invalid_range(identity, offset))?;
    let Some(length) = remaining
        .iter()
        .take(limits.max_string_bytes + 1)
        .position(|byte| *byte == 0)
    else {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidString,
            identity,
            Some(offset..offset + remaining.len().min(limits.max_string_bytes + 1)),
        ));
    };
    Ok(remaining[..length].to_vec())
}

fn fixed_string(bytes: &[u8], limits: Limits, identity: &str) -> Result<Vec<u8>, Error> {
    c_string(bytes, 0, limits, identity)
}

fn owned_bytes(document: &Document) -> usize {
    document.internal_name.len()
        + document
            .bones
            .iter()
            .map(|bone| bone.name.len() + bone.surface_property.len())
            .sum::<usize>()
        + document
            .animations
            .iter()
            .map(|value| value.name.len())
            .sum::<usize>()
        + document
            .sequences
            .iter()
            .map(|value| value.label.len() + value.activity_name.len())
            .sum::<usize>()
        + document
            .materials
            .iter()
            .map(|value| {
                value.name.len()
                    + value.search_paths.iter().map(Vec::len).sum::<usize>()
                    + value.candidates.iter().map(String::len).sum::<usize>()
            })
            .sum::<usize>()
}

fn count(bytes: &[u8], offset: usize, identity: &str) -> Result<usize, Error> {
    usize::try_from(i32_at(bytes, offset, identity)?).map_err(|_| invalid_count(identity, offset))
}

fn float(bytes: &[u8], offset: usize, identity: &str) -> Result<Float32, Error> {
    Ok(Float32(u32_at(bytes, offset, identity)?))
}

fn vector3(bytes: &[u8], offset: usize, identity: &str) -> Result<Vector3, Error> {
    Ok(Vector3([
        float(bytes, offset, identity)?,
        float(bytes, offset + 4, identity)?,
        float(bytes, offset + 8, identity)?,
    ]))
}

fn float4(bytes: &[u8], offset: usize, identity: &str) -> Result<[Float32; 4], Error> {
    Ok([
        float(bytes, offset, identity)?,
        float(bytes, offset + 4, identity)?,
        float(bytes, offset + 8, identity)?,
        float(bytes, offset + 12, identity)?,
    ])
}

fn float12(bytes: &[u8], offset: usize, identity: &str) -> Result<[Float32; 12], Error> {
    let mut output = [Float32(0); 12];
    for (index, value) in output.iter_mut().enumerate() {
        *value = float(bytes, offset + index * 4, identity)?;
    }
    Ok(output)
}

fn u16_at(bytes: &[u8], offset: usize, identity: &str) -> Result<u16, Error> {
    range(bytes, offset, 2, identity)?;
    Ok(u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated field"),
    ))
}

fn i32_at(bytes: &[u8], offset: usize, identity: &str) -> Result<i32, Error> {
    range(bytes, offset, 4, identity)?;
    Ok(i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated field"),
    ))
}

fn u32_at(bytes: &[u8], offset: usize, identity: &str) -> Result<u32, Error> {
    range(bytes, offset, 4, identity)?;
    Ok(u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated field"),
    ))
}

fn checksum_error(identity: &str) -> Error {
    failure(
        Classification::Malformed,
        ErrorCode::ChecksumMismatch,
        identity,
        None,
    )
}

fn invalid_count(identity: &str, offset: usize) -> Error {
    failure(
        Classification::Malformed,
        ErrorCode::InvalidCount,
        identity,
        Some(offset..offset + 4),
    )
}

fn invalid_range(identity: &str, offset: usize) -> Error {
    failure(
        Classification::Malformed,
        ErrorCode::InvalidRange,
        identity,
        Some(offset..offset),
    )
}

fn invalid_reference(identity: &str, offset: usize) -> Error {
    failure(
        Classification::Malformed,
        ErrorCode::InvalidReference,
        identity,
        Some(offset..offset + 4),
    )
}

fn failure(
    classification: Classification,
    code: ErrorCode,
    identity: &str,
    range: Option<Range<usize>>,
) -> Error {
    Error {
        classification,
        code,
        identity: identity.to_owned(),
        range,
        dependency_chain: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mdl(version: i32, body_part: bool) -> Vec<u8> {
        let mut bytes = vec![0; MDL_HEADER_BYTES];
        bytes[..4].copy_from_slice(b"IDST");
        bytes[4..8].copy_from_slice(&version.to_le_bytes());
        bytes[8..12].copy_from_slice(&1234_i32.to_le_bytes());
        bytes[12..17].copy_from_slice(b"test\0");
        for offset in [80, 92, 104, 116, 128, 140] {
            bytes[offset..offset + 4].copy_from_slice(&0x3f80_0000_u32.to_le_bytes());
        }
        if body_part {
            bytes[232..236].copy_from_slice(&1_i32.to_le_bytes());
            bytes[236..240].copy_from_slice(&(MDL_HEADER_BYTES as i32).to_le_bytes());
            bytes.resize(MDL_HEADER_BYTES + BODY_PART_BYTES + 5, 0);
            bytes[MDL_HEADER_BYTES..MDL_HEADER_BYTES + 4]
                .copy_from_slice(&(BODY_PART_BYTES as i32).to_le_bytes());
            bytes[MDL_HEADER_BYTES + 4..MDL_HEADER_BYTES + 8].copy_from_slice(&0_i32.to_le_bytes());
            bytes[MDL_HEADER_BYTES + 8..MDL_HEADER_BYTES + 12]
                .copy_from_slice(&1_i32.to_le_bytes());
            bytes[MDL_HEADER_BYTES + BODY_PART_BYTES..].copy_from_slice(b"body\0");
        }
        let length = bytes.len() as i32;
        bytes[76..80].copy_from_slice(&length.to_le_bytes());
        bytes
    }

    fn vvd(checksum: i32) -> Vec<u8> {
        let mut bytes = vec![0; 64];
        bytes[..4].copy_from_slice(b"IDSV");
        bytes[4..8].copy_from_slice(&4_i32.to_le_bytes());
        bytes[8..12].copy_from_slice(&checksum.to_le_bytes());
        bytes[12..16].copy_from_slice(&1_i32.to_le_bytes());
        bytes[52..56].copy_from_slice(&64_i32.to_le_bytes());
        bytes[56..60].copy_from_slice(&64_i32.to_le_bytes());
        bytes[60..64].copy_from_slice(&64_i32.to_le_bytes());
        bytes
    }

    fn vtx(checksum: i32) -> Vec<u8> {
        let mut bytes = vec![0; 52];
        bytes[..4].copy_from_slice(&7_i32.to_le_bytes());
        bytes[12..16].copy_from_slice(&3_i32.to_le_bytes());
        bytes[16..20].copy_from_slice(&checksum.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_i32.to_le_bytes());
        bytes[24..28].copy_from_slice(&44_i32.to_le_bytes());
        bytes[28..32].copy_from_slice(&1_i32.to_le_bytes());
        bytes[32..36].copy_from_slice(&36_i32.to_le_bytes());
        bytes
    }

    fn response(request: &DependencyRequest, bytes: Option<Vec<u8>>) -> DependencyResponse {
        DependencyResponse {
            requester: request.requester.clone(),
            role: request.role,
            logical_path: request.logical_path.clone(),
            bytes,
        }
    }

    #[test]
    fn metadata_only_model_requests_only_optional_physics() {
        let bytes = mdl(48, false);
        let Load::Needs(requests) = load(
            "models/test.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &bytes,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("model completed without physics disposition")
        };
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].role, DependencyRole::Physics);
        assert_eq!(requests[0].logical_path, "models/test.phy");
        let Load::Complete(document) = load(
            "models/test.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &bytes,
            &[response(&requests[0], None)],
            Limits::default(),
        )
        .unwrap() else {
            panic!("complete responses requested more data")
        };
        assert_eq!(document.checksum, 1234);
        assert_eq!(document.internal_name, b"test");
        assert_eq!(document.physics_status, PhysicsStatus::Missing);
    }

    #[test]
    fn bodypart_model_requires_and_validates_vvd_vtx_checksums() {
        let bytes = mdl(44, true);
        let Load::Needs(requests) = load(
            "models/prop.mdl",
            Profile::SourcePcMdl44,
            VtxVariant::Dx90,
            &bytes,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("model completed without companions")
        };
        assert_eq!(
            requests
                .iter()
                .map(|request| request.role)
                .collect::<Vec<_>>(),
            [
                DependencyRole::VertexData,
                DependencyRole::Topology,
                DependencyRole::Physics
            ]
        );
        let responses: Vec<_> = requests
            .iter()
            .map(|request| {
                response(
                    request,
                    match request.role {
                        DependencyRole::VertexData => Some(vvd(1234)),
                        DependencyRole::Topology => Some(vtx(1234)),
                        DependencyRole::Physics => None,
                        _ => unreachable!(),
                    },
                )
            })
            .collect();
        let Load::Complete(document) = load(
            "models/prop.mdl",
            Profile::SourcePcMdl44,
            VtxVariant::Dx90,
            &bytes,
            &responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("validated companions requested more data")
        };
        assert_eq!(document.body_parts[0].name, b"body");
        assert_eq!(document.companions.vvd_lod_vertex_counts, [0]);
        assert_eq!(document.companions.vtx_body_part_count, 1);

        let mut bad = responses;
        bad[0].bytes = Some(vvd(5678));
        assert_eq!(
            load(
                "models/prop.mdl",
                Profile::SourcePcMdl44,
                VtxVariant::Dx90,
                &bytes,
                &bad,
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::ChecksumMismatch
        );
    }

    #[test]
    fn rejects_profile_identity_range_and_limit_failures() {
        let bytes = mdl(48, false);
        assert_eq!(
            load(
                "models/test.mdl",
                Profile::SourcePcMdl44,
                VtxVariant::Dx90,
                &bytes,
                &[],
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::ProfileMismatch
        );
        assert_eq!(
            load(
                "../test.mdl",
                Profile::SourcePcMdl48,
                VtxVariant::Dx90,
                &bytes,
                &[],
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidIdentity
        );
        assert_eq!(
            load(
                "models/test.mdl",
                Profile::SourcePcMdl48,
                VtxVariant::Dx90,
                &bytes,
                &[],
                Limits {
                    max_file_bytes: bytes.len() - 1,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InputLimit
        );
    }
}
