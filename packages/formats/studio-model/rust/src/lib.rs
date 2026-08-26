use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
    fmt,
    ops::Range,
};

mod eye;
mod lighting;
mod presentation;
mod static_lighting;
mod viewmodel;
pub use eye::*;
pub use lighting::*;
pub use presentation::*;
pub use static_lighting::*;
pub use viewmodel::*;

const MDL_HEADER_BYTES: usize = 408;
const BONE_BYTES: usize = 216;
const ANIMATION_BYTES: usize = 100;
const SEQUENCE_BYTES: usize = 212;
const TEXTURE_BYTES: usize = 64;
const BODY_PART_BYTES: usize = 16;
const MODEL_BYTES: usize = 148;
const MESH_BYTES: usize = 116;
const ATTACHMENT_BYTES: usize = 92;
const POSE_PARAMETER_BYTES: usize = 20;
const HITBOX_SET_BYTES: usize = 12;
const HITBOX_BYTES: usize = 68;
const SEQUENCE_EVENT_BYTES: usize = 80;
const SEQUENCE_AUTO_LAYER_BYTES: usize = 24;
const EYEBALL_BYTES: usize = 172;
const STUDIO_OVERRIDE: i32 = 0x0800;
const STUDIO_CYCLE_POSE: i32 = 0x0080;
const STUDIO_AUTO_LAYER_POSE: i32 = 0x4000;

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
    pub max_included_models: usize,
    pub max_records: usize,
    pub max_strings: usize,
    pub max_string_bytes: usize,
    pub max_owned_bytes: usize,
    pub max_animation_frames: usize,
    pub max_decoded_animation_samples: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 256 * 1024 * 1024,
            max_aggregate_input_bytes: 1024 * 1024 * 1024,
            max_dependency_count: 4_096,
            max_include_depth: 64,
            max_included_models: 64,
            max_records: 4_000_000,
            max_strings: 262_144,
            max_string_bytes: 4_096,
            max_owned_bytes: 512 * 1024 * 1024,
            max_animation_frames: 1_000_000,
            max_decoded_animation_samples: 64_000_000,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelDependencyRole {
    RootModel,
    VertexData,
    Topology,
    AnimationBlocks,
    IncludeModel,
    Physics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDependency {
    pub requester: String,
    pub role: ModelDependencyRole,
    pub logical_path: String,
    pub sha256: Option<[u8; 32]>,
    pub byte_length: usize,
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
pub struct DependencyResponse<'source> {
    pub requester: String,
    pub role: DependencyRole,
    pub logical_path: String,
    pub bytes: Option<Cow<'source, [u8]>>,
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
    pub controllers: [i32; 6],
    pub position: Vector3,
    pub quaternion: [Float32; 4],
    pub rotation: Vector3,
    pub position_scale: Vector3,
    pub rotation_scale: Vector3,
    pub pose_to_bone: [Float32; 12],
    pub alignment: [Float32; 4],
    pub flags: i32,
    pub procedural_type: i32,
    pub procedural_offset: i32,
    pub physics_bone: i32,
    pub surface_property: Vec<u8>,
    pub contents: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RotationCodec {
    Bind,
    DeltaIdentity,
    Quaternion48,
    Quaternion64,
    RleEuler,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationCodec {
    Bind,
    DeltaZero,
    Vector48,
    RleVector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationValueRun {
    pub valid: u8,
    pub total: u8,
    pub values: Vec<i16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationValueStream {
    pub relative_offset: i16,
    pub runs: Vec<AnimationValueRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationTrack {
    pub bone: usize,
    pub flags: u8,
    pub source_offset: usize,
    pub next_offset: u16,
    pub rotation_codec: RotationCodec,
    pub translation_codec: TranslationCodec,
    pub rotation_values: [Option<AnimationValueStream>; 3],
    pub translation_values: [Option<AnimationValueStream>; 3],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationSection {
    pub index: usize,
    pub first_frame: usize,
    pub frame_count: usize,
    pub block: i32,
    pub data_offset: i32,
    pub tracks: Vec<AnimationTrack>,
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
    pub source_identity: String,
    pub bone_map: Vec<Option<usize>>,
    pub sections: Vec<AnimationSection>,
    pub frames: Vec<AnimationFrame>,
    pub compact_frames: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationFrame {
    pub translations: Vec<Vector3>,
    pub rotations: Vec<[Float32; 4]>,
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
    pub blend_count: i32,
    pub blend_size: [i32; 2],
    pub animation_indices: Vec<i16>,
    pub pose_parameter_indices: [i32; 2],
    pub pose_parameter_start: [Float32; 2],
    pub pose_parameter_end: [Float32; 2],
    pub fade_in: Float32,
    pub fade_out: Float32,
    pub entry_node: i32,
    pub exit_node: i32,
    pub node_flags: i32,
    pub entry_phase: Float32,
    pub exit_phase: Float32,
    pub last_frame: Float32,
    pub next_sequence: i32,
    pub pose: i32,
    pub auto_layer_count: i32,
    pub bone_weights: Vec<Float32>,
    pub pose_keys: [Vec<Float32>; 2],
    pub ik_lock_count: i32,
    pub cycle_pose_parameter: i32,
    pub activity_modifier_count: i32,
    pub events: Vec<SequenceEvent>,
    pub auto_layers: Vec<SequenceAutoLayer>,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceEvent {
    pub index: usize,
    pub cycle: Float32,
    pub event: i32,
    pub event_type: i32,
    pub options: [u8; 64],
    pub name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceAutoLayer {
    pub index: usize,
    pub sequence: i16,
    pub pose: i16,
    pub flags: i32,
    pub start: Float32,
    pub peak: Float32,
    pub tail: Float32,
    pub end: Float32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoseParameter {
    pub index: usize,
    pub name: Vec<u8>,
    pub flags: i32,
    pub start: Float32,
    pub end: Float32,
    pub looping_range: Float32,
    pub source_identity: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedMetadata {
    pub flex_descriptors: usize,
    pub flex_controllers: usize,
    pub flex_rules: usize,
    pub ik_chains: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Material {
    pub index: usize,
    pub name: Vec<u8>,
    pub search_paths: Vec<Vec<u8>>,
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialReplacement {
    pub lod: usize,
    pub material_slot: usize,
    pub name: Vec<u8>,
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
    pub eyeballs: Vec<Eyeball>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Eyeball {
    pub index: usize,
    pub name: Vec<u8>,
    pub bone: i32,
    pub origin: Vector3,
    pub z_offset: Float32,
    pub radius: Float32,
    pub up: Vector3,
    pub forward: Vector3,
    pub texture: i32,
    pub unused_1: i32,
    pub iris_scale: Float32,
    pub unused_2: i32,
    pub upper_flex_descriptors: [i32; 3],
    pub lower_flex_descriptors: [i32; 3],
    pub upper_targets: Vector3,
    pub lower_targets: Vector3,
    pub upper_lid_flex_descriptor: i32,
    pub lower_lid_flex_descriptor: i32,
    pub unused: [i32; 4],
    pub non_facs: bool,
    pub unused_3: [u8; 3],
    pub unused_4: [i32; 7],
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
pub struct Hitbox {
    pub index: usize,
    pub bone: i32,
    pub group: i32,
    pub bounds_min: Vector3,
    pub bounds_max: Vector3,
    pub name_offset: i32,
    pub name_resolved: bool,
    pub name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HitboxSet {
    pub index: usize,
    pub name: Vec<u8>,
    pub hitboxes: Vec<Hitbox>,
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
    pub root_lod: u8,
    pub allowed_root_lods: u8,
    pub bounds: Bounds,
    pub illumination_attachment: i32,
    pub raw_max_eye_deflection: Float32,
    pub max_eye_deflection: Float32,
    pub bones: Vec<Bone>,
    pub animations: Vec<Animation>,
    pub sequences: Vec<Sequence>,
    pub materials: Vec<Material>,
    pub material_replacements: Vec<MaterialReplacement>,
    pub skins: Vec<SkinFamily>,
    pub body_parts: Vec<BodyPart>,
    pub attachments: Vec<Attachment>,
    pub hitbox_sets: Vec<HitboxSet>,
    pub pose_parameters: Vec<PoseParameter>,
    pub unsupported: UnsupportedMetadata,
    pub include_models: Vec<String>,
    pub animation_blocks: Vec<Range<usize>>,
    pub animation_block_identity: Option<String>,
    pub companions: CompanionSummary,
    pub physics_status: PhysicsStatus,
    pub source_identities: Vec<String>,
    pub model_dependencies: Vec<ModelDependency>,
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
    animation_sources: Vec<AnimationSource>,
}

struct AnimationSource {
    descriptor_offset: usize,
    data_offset: i32,
    sections: Vec<(i32, i32)>,
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
    material_replacements: Vec<ParsedMaterialReplacement>,
}
struct ParsedMaterialReplacement {
    lod: usize,
    material_slot: usize,
    name: Vec<u8>,
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

struct LoadContext<'a, 'source> {
    vtx_variant: VtxVariant,
    responses: &'a [DependencyResponse<'source>],
    limits: Limits,
    included_models: usize,
    dependency_count: usize,
}

pub fn load(
    identity: impl Into<String>,
    profile: Profile,
    vtx_variant: VtxVariant,
    mdl_bytes: &[u8],
    responses: &[DependencyResponse<'_>],
    limits: Limits,
) -> Result<Load, Error> {
    let mut context = LoadContext {
        vtx_variant,
        responses,
        limits,
        included_models: 0,
        dependency_count: 0,
    };
    load_with_chain(
        identity.into(),
        profile,
        mdl_bytes,
        Vec::new(),
        &mut context,
    )
}

fn load_with_chain(
    identity: String,
    profile: Profile,
    mdl_bytes: &[u8],
    mut dependency_chain: Vec<String>,
    context: &mut LoadContext<'_, '_>,
) -> Result<Load, Error> {
    let limits = context.limits;
    let responses = context.responses;
    validate_limits(limits)?;
    validate_model_identity(&identity)?;
    if dependency_chain
        .iter()
        .any(|value| value.eq_ignore_ascii_case(&identity))
    {
        return Err(Error {
            classification: Classification::Malformed,
            code: ErrorCode::IncludeCycle,
            identity,
            range: None,
            dependency_chain,
        });
    }
    if dependency_chain.len() >= limits.max_include_depth {
        return Err(Error {
            classification: Classification::Malformed,
            code: ErrorCode::DependencyLimit,
            identity,
            range: None,
            dependency_chain,
        });
    }
    if !dependency_chain.is_empty() {
        context.included_models = context.included_models.checked_add(1).ok_or_else(|| {
            failure(
                Classification::Malformed,
                ErrorCode::DependencyLimit,
                &identity,
                None,
            )
        })?;
        if context.included_models > limits.max_included_models {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::DependencyLimit,
                &identity,
                None,
            ));
        }
    }
    dependency_chain.push(identity.clone());
    let mut aggregate = mdl_bytes.len();
    let mut root = parse_mdl(&identity, profile, mdl_bytes, limits)?;
    root.document.model_dependencies.push(ModelDependency {
        requester: identity.clone(),
        role: ModelDependencyRole::RootModel,
        logical_path: identity.clone(),
        sha256: Some(presentation::content_sha256(mdl_bytes)),
        byte_length: mdl_bytes.len(),
    });
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
            dependency_chain: dependency_chain.clone(),
        });
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::Topology,
            logical_path: format!("{stem}{}", context.vtx_variant.suffix()),
            expected_checksum: root.document.checksum,
            dependency_chain: dependency_chain.clone(),
        });
    }
    if root.needs_animation {
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::AnimationBlocks,
            logical_path: root
                .document
                .animation_block_identity
                .clone()
                .expect("external animation identity validated"),
            expected_checksum: root.document.checksum,
            dependency_chain: dependency_chain.clone(),
        });
    }
    requests.push(DependencyRequest {
        requester: identity.clone(),
        role: DependencyRole::Physics,
        logical_path: format!("{stem}.phy"),
        expected_checksum: root.document.checksum,
        dependency_chain: dependency_chain.clone(),
    });
    for include in &root.document.include_models {
        requests.push(DependencyRequest {
            requester: identity.clone(),
            role: DependencyRole::IncludeModel,
            logical_path: include.clone(),
            expected_checksum: 0,
            dependency_chain: dependency_chain.clone(),
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
    context.dependency_count = context
        .dependency_count
        .checked_add(requests.len())
        .ok_or_else(|| {
            failure(
                Classification::Malformed,
                ErrorCode::DependencyLimit,
                &identity,
                None,
            )
        })?;
    if context.dependency_count > limits.max_dependency_count {
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

    let animation_sources = root.animation_sources;
    let mut document = root.document;
    let mut source_identities = vec![identity.clone()];
    let mut physics_status = PhysicsStatus::Missing;
    let mut parsed_vvd = None;
    let mut parsed_vtx = None;
    let mut ani_bytes = None;
    let mut included_documents = Vec::new();
    let mut nested_requests = Vec::new();
    for request in requests {
        let response = responses
            .iter()
            .find(|response| response_matches(response, &request))
            .expect("all requests matched");
        document.model_dependencies.push(ModelDependency {
            requester: response.requester.clone(),
            role: match response.role {
                DependencyRole::VertexData => ModelDependencyRole::VertexData,
                DependencyRole::Topology => ModelDependencyRole::Topology,
                DependencyRole::AnimationBlocks => ModelDependencyRole::AnimationBlocks,
                DependencyRole::IncludeModel => ModelDependencyRole::IncludeModel,
                DependencyRole::Physics => ModelDependencyRole::Physics,
            },
            logical_path: response.logical_path.clone(),
            sha256: response.bytes.as_deref().map(presentation::content_sha256),
            byte_length: response.bytes.as_ref().map_or(0, |bytes| bytes.len()),
        });
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
                    document.materials.len(),
                    limits,
                )?;
                document.companions.vtx_lod_count = parsed.lod_count;
                document.companions.vtx_body_part_count = parsed.body_parts.len() as i32;
                document.companions.vtx_max_bones_per_vertex = parsed.max_bones;
                document.material_replacements = parsed
                    .material_replacements
                    .iter()
                    .map(|replacement| {
                        let material = &document.materials[replacement.material_slot];
                        Ok(MaterialReplacement {
                            lod: replacement.lod,
                            material_slot: replacement.material_slot,
                            name: replacement.name.clone(),
                            candidates: material
                                .search_paths
                                .iter()
                                .map(|path| {
                                    material_candidate(path, &replacement.name, &document.identity)
                                })
                                .collect::<Result<Vec<_>, Error>>()?,
                        })
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
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
                ani_bytes = Some(bytes.as_ref());
            }
            DependencyRole::IncludeModel => {
                let include_profile = profile_for_version(
                    i32_at(bytes, 4, &response.logical_path)?,
                    &response.logical_path,
                )?;
                match load_with_chain(
                    response.logical_path.clone(),
                    include_profile,
                    bytes,
                    dependency_chain.clone(),
                    context,
                )? {
                    Load::Needs(requests) => nested_requests.extend(requests),
                    Load::Complete(include) => included_documents.push(*include),
                }
            }
            DependencyRole::Physics => physics_status = PhysicsStatus::Present,
        }
        source_identities.push(response.logical_path.clone());
    }
    if !nested_requests.is_empty() {
        if nested_requests.len() > limits.max_dependency_count {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::DependencyLimit,
                &identity,
                None,
            ));
        }
        return Ok(Load::Needs(nested_requests));
    }
    document.source_identities = source_identities;
    document.physics_status = physics_status;
    if let (Some(vvd), Some(vtx)) = (&parsed_vvd, &parsed_vtx) {
        document.geometry = assemble_geometry(&document.body_parts, vvd, vtx, &identity)?;
    }
    let decode_context = AnimationDecodeContext {
        mdl: mdl_bytes,
        ani: ani_bytes,
        blocks: &document.animation_blocks,
        bones: &document.bones,
        identity: &identity,
        limits,
    };
    decode_animation_frames(
        &mut document.animations,
        &animation_sources,
        &decode_context,
    )?;
    for include in included_documents {
        compose_include(&mut document, include, &identity)?;
    }
    Ok(Load::Complete(Box::new(document)))
}

fn compose_include(root: &mut Document, include: Document, identity: &str) -> Result<(), Error> {
    let bone_map: Vec<_> = include
        .bones
        .iter()
        .map(|bone| {
            root.bones
                .iter()
                .position(|candidate| candidate.name.eq_ignore_ascii_case(&bone.name))
        })
        .collect();

    let base_pose_count = root.pose_parameters.len();
    let mut pose_map = Vec::with_capacity(include.pose_parameters.len());
    let mut new_poses = Vec::new();
    for pose in &include.pose_parameters {
        let mapped = root.pose_parameters[..base_pose_count]
            .iter()
            .position(|candidate| candidate.name.eq_ignore_ascii_case(&pose.name))
            .unwrap_or_else(|| {
                let index = base_pose_count + new_poses.len();
                let mut pose = pose.clone();
                pose.index = index;
                new_poses.push(pose);
                index
            });
        pose_map.push(mapped);
    }
    root.pose_parameters.extend(new_poses);

    let base_animation_count = root.animations.len();
    let mut animation_map = Vec::with_capacity(include.animations.len());
    let mut new_animations = Vec::new();
    for animation in &include.animations {
        let mapped = root.animations[..base_animation_count]
            .iter()
            .position(|candidate| candidate.name.eq_ignore_ascii_case(&animation.name))
            .unwrap_or_else(|| {
                let index = base_animation_count + new_animations.len();
                let mut animation = animation.clone();
                animation.index = index;
                animation.bone_map = animation
                    .bone_map
                    .iter()
                    .map(|bone| bone.and_then(|bone| bone_map.get(bone).copied().flatten()))
                    .collect();
                new_animations.push(animation);
                index
            });
        animation_map.push(mapped);
    }
    root.animations.extend(new_animations);

    let base_sequence_count = root.sequences.len();
    let mut sequence_map = Vec::with_capacity(include.sequences.len());
    let mut sequence_destinations = Vec::with_capacity(include.sequences.len());
    let mut next_new_sequence = base_sequence_count;
    for sequence in &include.sequences {
        let existing = root.sequences[..base_sequence_count]
            .iter()
            .position(|candidate| candidate.label.eq_ignore_ascii_case(&sequence.label));
        let destination = if let Some(existing) = existing {
            existing
        } else {
            let destination = next_new_sequence;
            next_new_sequence += 1;
            destination
        };
        sequence_map.push(destination);
        sequence_destinations.push((destination, existing.is_some()));
    }
    for (source, (destination, duplicate)) in include.sequences.iter().zip(sequence_destinations) {
        let mut sequence = source.clone();
        sequence.index = destination;
        let mut root_weights = vec![Float32(0.0_f32.to_bits()); root.bones.len()];
        for (local_bone, weight) in source.bone_weights.iter().enumerate() {
            if let Some(root_bone) = bone_map.get(local_bone).copied().flatten() {
                root_weights[root_bone] = *weight;
            }
        }
        sequence.bone_weights = root_weights;
        for animation in &mut sequence.animation_indices {
            let local = usize::try_from(*animation)
                .ok()
                .and_then(|index| animation_map.get(index).copied())
                .ok_or_else(|| invalid_reference(identity, source.index))?;
            *animation =
                i16::try_from(local).map_err(|_| invalid_reference(identity, source.index))?;
        }
        for layer in &mut sequence.auto_layers {
            let mapped = usize::try_from(layer.sequence)
                .ok()
                .and_then(|index| sequence_map.get(index).copied())
                .and_then(|index| i16::try_from(index).ok())
                .ok_or_else(|| invalid_reference(identity, source.index))?;
            layer.sequence = mapped;
            if layer.flags & STUDIO_AUTO_LAYER_POSE != 0 {
                layer.pose = usize::try_from(layer.pose)
                    .ok()
                    .and_then(|index| pose_map.get(index).copied())
                    .and_then(|index| i16::try_from(index).ok())
                    .ok_or_else(|| invalid_reference(identity, source.index))?;
            }
        }
        for (axis, pose) in sequence.pose_parameter_indices.iter_mut().enumerate() {
            if sequence.blend_size[axis] > 1 {
                *pose = usize::try_from(*pose)
                    .ok()
                    .and_then(|index| pose_map.get(index).copied())
                    .and_then(|index| i32::try_from(index).ok())
                    .ok_or_else(|| invalid_reference(identity, source.index))?;
            }
        }
        if sequence.flags & STUDIO_CYCLE_POSE != 0 {
            sequence.cycle_pose_parameter = usize::try_from(sequence.cycle_pose_parameter)
                .ok()
                .and_then(|index| pose_map.get(index).copied())
                .and_then(|index| i32::try_from(index).ok())
                .ok_or_else(|| invalid_reference(identity, source.index))?;
        }
        if sequence.next_sequence >= 0 {
            sequence.next_sequence = usize::try_from(sequence.next_sequence)
                .ok()
                .and_then(|index| sequence_map.get(index).copied())
                .and_then(|index| i32::try_from(index).ok())
                .ok_or_else(|| invalid_reference(identity, source.index))?;
        }
        if sequence.pose >= 0 {
            sequence.pose = usize::try_from(sequence.pose)
                .ok()
                .and_then(|index| sequence_map.get(index).copied())
                .and_then(|index| i32::try_from(index).ok())
                .ok_or_else(|| invalid_reference(identity, source.index))?;
        }
        if duplicate {
            if root.sequences[destination].flags & STUDIO_OVERRIDE != 0 {
                root.sequences[destination] = sequence;
            }
        } else {
            root.sequences.push(sequence);
        }
    }

    let base_attachment_count = root.attachments.len();
    for attachment in include.attachments {
        if root.attachments[..base_attachment_count]
            .iter()
            .any(|candidate| candidate.name.eq_ignore_ascii_case(&attachment.name))
        {
            continue;
        }
        let Some(bone) = usize::try_from(attachment.bone)
            .ok()
            .and_then(|index| bone_map.get(index).copied().flatten())
        else {
            continue;
        };
        let mut attachment = attachment;
        attachment.index = root.attachments.len();
        attachment.bone = bone as i32;
        root.attachments.push(attachment);
    }
    root.unsupported.flex_descriptors = root
        .unsupported
        .flex_descriptors
        .checked_add(include.unsupported.flex_descriptors)
        .ok_or_else(|| invalid_count(identity, 260))?;
    root.unsupported.flex_controllers = root
        .unsupported
        .flex_controllers
        .checked_add(include.unsupported.flex_controllers)
        .ok_or_else(|| invalid_count(identity, 268))?;
    root.unsupported.flex_rules = root
        .unsupported
        .flex_rules
        .checked_add(include.unsupported.flex_rules)
        .ok_or_else(|| invalid_count(identity, 276))?;
    root.unsupported.ik_chains = root
        .unsupported
        .ik_chains
        .checked_add(include.unsupported.ik_chains)
        .ok_or_else(|| invalid_count(identity, 284))?;
    root.source_identities.extend(include.source_identities);
    root.model_dependencies.extend(include.model_dependencies);
    Ok(())
}

fn validate_procedure(
    bytes: &[u8],
    bone_offset: usize,
    procedure_type: i32,
    procedure_offset: i32,
    bone_count: usize,
    limits: Limits,
    identity: &str,
) -> Result<(), Error> {
    if procedure_type == 0 {
        return Ok(());
    }
    if procedure_offset == 0 {
        return Err(invalid_reference(identity, bone_offset + 168));
    }
    let procedure = relative_offset(bone_offset, procedure_offset, identity)?;
    match procedure_type {
        1 => {
            range(bytes, procedure, 176, identity)?;
            let control = i32_at(bytes, procedure, identity)?;
            let axis = i32_at(bytes, procedure + 4, identity)?;
            if control < 0 || control as usize >= bone_count || !(0..=2).contains(&axis) {
                return Err(invalid_reference(identity, procedure));
            }
        }
        2 => {
            range(bytes, procedure, 12, identity)?;
            let control = i32_at(bytes, procedure, identity)?;
            let trigger_count = count(bytes, procedure + 4, identity)?;
            if control < 0 || control as usize >= bone_count {
                return Err(invalid_reference(identity, procedure));
            }
            let triggers =
                relative_offset(procedure, i32_at(bytes, procedure + 8, identity)?, identity)?;
            table(bytes, triggers, trigger_count, 48, limits, identity)?;
        }
        3 | 4 => {
            range(bytes, procedure, 44, identity)?;
            let parent = i32_at(bytes, procedure, identity)?;
            if parent < -1 || (parent >= 0 && parent as usize >= bone_count) {
                return Err(invalid_reference(identity, procedure));
            }
        }
        5 => range(bytes, procedure, 140, identity)?,
        _ => range(bytes, procedure, 1, identity)?,
    }
    Ok(())
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
    let root_lod = bytes[377];
    let allowed_root_lods = bytes[378];
    if root_lod >= 8 || allowed_root_lods > 8 {
        return Err(invalid_range(identity, 377));
    }
    let bounds = Bounds {
        eye: vector3(bytes, 80, identity)?,
        illumination: vector3(bytes, 92, identity)?,
        hull_min: vector3(bytes, 104, identity)?,
        hull_max: vector3(bytes, 116, identity)?,
        view_min: vector3(bytes, 128, identity)?,
        view_max: vector3(bytes, 140, identity)?,
    };
    let secondary_header_offset = i32_at(bytes, 400, identity)?;
    let (illumination_attachment, raw_max_eye_deflection, max_eye_deflection) =
        if secondary_header_offset == 0 {
            (0, Float32(0.0_f32.to_bits()), Float32(0.866_f32.to_bits()))
        } else if secondary_header_offset > 0 {
            let secondary_header_offset = secondary_header_offset as usize;
            range(bytes, secondary_header_offset, 16, identity)?;
            let raw = float(bytes, secondary_header_offset + 12, identity)?;
            let raw_value = f32::from_bits(raw.0);
            if !raw_value.is_finite() {
                return Err(invalid_range(identity, secondary_header_offset + 12));
            }
            (
                i32_at(bytes, secondary_header_offset + 8, identity)?,
                raw,
                if raw_value == 0.0 {
                    Float32(0.866_f32.to_bits())
                } else {
                    raw
                },
            )
        } else {
            return Err(invalid_range(identity, 400));
        };
    if illumination_attachment < 0 {
        return Err(invalid_reference(identity, 400));
    }

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
        let surface_relative = i32_at(bytes, offset + 176, identity)?;
        let procedural_type = i32_at(bytes, offset + 164, identity)?;
        let procedural_offset = i32_at(bytes, offset + 168, identity)?;
        validate_procedure(
            bytes,
            offset,
            procedural_type,
            procedural_offset,
            bone_count,
            limits,
            identity,
        )?;
        let mut controllers = [0; 6];
        for (controller, output) in controllers.iter_mut().enumerate() {
            *output = i32_at(bytes, offset + 8 + controller * 4, identity)?;
            if *output < -1 || *output >= 4 {
                return Err(invalid_reference(identity, offset + 8 + controller * 4));
            }
        }
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
            controllers,
            position: vector3(bytes, offset + 32, identity)?,
            quaternion: float4(bytes, offset + 44, identity)?,
            rotation: vector3(bytes, offset + 60, identity)?,
            position_scale: vector3(bytes, offset + 72, identity)?,
            rotation_scale: vector3(bytes, offset + 84, identity)?,
            pose_to_bone: float12(bytes, offset + 96, identity)?,
            alignment: float4(bytes, offset + 144, identity)?,
            flags: i32_at(bytes, offset + 160, identity)?,
            procedural_type,
            procedural_offset,
            physics_bone: i32_at(bytes, offset + 172, identity)?,
            surface_property: if surface_relative == 0 {
                Vec::new()
            } else {
                relative_string(bytes, offset, surface_relative, limits, identity)?
            },
            contents: i32_at(bytes, offset + 180, identity)?,
        });
    }

    let hitbox_set_count = count(bytes, 172, identity)?;
    let hitbox_set_offset = count(bytes, 176, identity)?;
    table(
        bytes,
        hitbox_set_offset,
        hitbox_set_count,
        HITBOX_SET_BYTES,
        limits,
        identity,
    )?;
    let mut hitbox_sets = Vec::with_capacity(hitbox_set_count);
    for set_index in 0..hitbox_set_count {
        let set_offset = hitbox_set_offset + set_index * HITBOX_SET_BYTES;
        let hitbox_count = count(bytes, set_offset + 4, identity)?;
        let hitbox_offset = if hitbox_count == 0 {
            0
        } else {
            relative_offset(
                set_offset,
                i32_at(bytes, set_offset + 8, identity)?,
                identity,
            )?
        };
        table(
            bytes,
            hitbox_offset,
            hitbox_count,
            HITBOX_BYTES,
            limits,
            identity,
        )?;
        let mut hitboxes = Vec::with_capacity(hitbox_count);
        for hitbox_index in 0..hitbox_count {
            let hitbox_offset = hitbox_offset + hitbox_index * HITBOX_BYTES;
            let bone = i32_at(bytes, hitbox_offset, identity)?;
            if bone < 0 || bone as usize >= bone_count {
                return Err(invalid_reference(identity, hitbox_offset));
            }
            let name_offset = i32_at(bytes, hitbox_offset + 32, identity)?;
            let (name_resolved, name) = if name_offset == 0 {
                (true, Vec::new())
            } else {
                let offset = relative_offset(hitbox_offset, name_offset, identity)?;
                if offset < bytes.len() {
                    (true, c_string(bytes, offset, limits, identity)?)
                } else {
                    (false, Vec::new())
                }
            };
            hitboxes.push(Hitbox {
                index: hitbox_index,
                bone,
                group: i32_at(bytes, hitbox_offset + 4, identity)?,
                bounds_min: vector3(bytes, hitbox_offset + 8, identity)?,
                bounds_max: vector3(bytes, hitbox_offset + 20, identity)?,
                name_offset,
                name_resolved,
                name,
            });
        }
        hitbox_sets.push(HitboxSet {
            index: set_index,
            name: relative_string(
                bytes,
                set_offset,
                i32_at(bytes, set_offset, identity)?,
                limits,
                identity,
            )?,
            hitboxes,
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
    let mut animation_sources = Vec::with_capacity(animation_count);
    let mut needs_animation = false;
    for index in 0..animation_count {
        let offset = animation_offset + index * ANIMATION_BYTES;
        let animation_block = i32_at(bytes, offset + 52, identity)?;
        needs_animation |= animation_block > 0;
        let section_offset = i32_at(bytes, offset + 80, identity)?;
        let section_frame_count = i32_at(bytes, offset + 84, identity)?;
        let frame_count = i32_at(bytes, offset + 16, identity)?;
        if frame_count <= 0 || frame_count as usize > limits.max_animation_frames {
            return Err(invalid_count(identity, offset + 16));
        }
        let mut sections = Vec::new();
        if section_frame_count > 0 && frame_count > 0 {
            let section_count = (frame_count as usize / section_frame_count as usize) + 2;
            let table_offset = relative_offset(offset, section_offset, identity)?;
            table(bytes, table_offset, section_count, 8, limits, identity)?;
            for section in 0..section_count {
                let block = i32_at(bytes, table_offset + section * 8, identity)?;
                needs_animation |= block > 0;
                sections.push((
                    block,
                    i32_at(bytes, table_offset + section * 8 + 4, identity)?,
                ));
            }
        }
        let data_offset = i32_at(bytes, offset + 56, identity)?;
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
            frame_count,
            movement_count: i32_at(bytes, offset + 20, identity)?,
            animation_block,
            animation_offset: data_offset,
            ik_rule_count: i32_at(bytes, offset + 60, identity)?,
            local_hierarchy_count: i32_at(bytes, offset + 72, identity)?,
            section_offset,
            section_frame_count,
            zero_frame_count: if profile.version() >= 47 {
                u16_at(bytes, offset + 90, identity)?
            } else {
                0
            },
            source_identity: identity.to_owned(),
            bone_map: (0..bone_count).map(Some).collect(),
            sections: Vec::new(),
            frames: Vec::new(),
            compact_frames: Vec::new(),
        });
        animation_sources.push(AnimationSource {
            descriptor_offset: offset,
            data_offset,
            sections,
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
        let event_count = count(bytes, offset + 24, identity)?;
        let mut events = Vec::with_capacity(event_count);
        if event_count > 0 {
            let event_offset =
                relative_offset(offset, i32_at(bytes, offset + 28, identity)?, identity)?;
            table(
                bytes,
                event_offset,
                event_count,
                SEQUENCE_EVENT_BYTES,
                limits,
                identity,
            )?;
            for event_index in 0..event_count {
                let event_offset = event_offset + event_index * SEQUENCE_EVENT_BYTES;
                let name_offset = i32_at(bytes, event_offset + 76, identity)?;
                events.push(SequenceEvent {
                    index: event_index,
                    cycle: float(bytes, event_offset, identity)?,
                    event: i32_at(bytes, event_offset + 4, identity)?,
                    event_type: i32_at(bytes, event_offset + 8, identity)?,
                    options: bytes[event_offset + 12..event_offset + 76]
                        .try_into()
                        .expect("validated sequence event options"),
                    name: if name_offset == 0 {
                        Vec::new()
                    } else {
                        relative_string(bytes, event_offset, name_offset, limits, identity)?
                    },
                });
            }
        }
        let auto_layer_count = count(bytes, offset + 148, identity)?;
        let mut auto_layers = Vec::with_capacity(auto_layer_count);
        if auto_layer_count > 0 {
            let auto_layer_offset =
                relative_offset(offset, i32_at(bytes, offset + 152, identity)?, identity)?;
            table(
                bytes,
                auto_layer_offset,
                auto_layer_count,
                SEQUENCE_AUTO_LAYER_BYTES,
                limits,
                identity,
            )?;
            for auto_layer_index in 0..auto_layer_count {
                let auto_layer_offset =
                    auto_layer_offset + auto_layer_index * SEQUENCE_AUTO_LAYER_BYTES;
                let sequence = i16_at(bytes, auto_layer_offset, identity)?;
                if sequence < 0 || sequence as usize >= sequence_count {
                    return Err(invalid_reference(identity, auto_layer_offset));
                }
                auto_layers.push(SequenceAutoLayer {
                    index: auto_layer_index,
                    sequence,
                    pose: i16_at(bytes, auto_layer_offset + 2, identity)?,
                    flags: i32_at(bytes, auto_layer_offset + 4, identity)?,
                    start: float(bytes, auto_layer_offset + 8, identity)?,
                    peak: float(bytes, auto_layer_offset + 12, identity)?,
                    tail: float(bytes, auto_layer_offset + 16, identity)?,
                    end: float(bytes, auto_layer_offset + 20, identity)?,
                });
            }
        }
        let blend_count = i32_at(bytes, offset + 56, identity)?;
        let blend_size = [
            i32_at(bytes, offset + 68, identity)?,
            i32_at(bytes, offset + 72, identity)?,
        ];
        let flags = i32_at(bytes, offset + 12, identity)?;
        let empty_override =
            flags & STUDIO_OVERRIDE != 0 && blend_count == 0 && blend_size == [0, 0];
        let populated = blend_count > 0
            && blend_size[0] > 0
            && blend_size[1] > 0
            && blend_size[0]
                .checked_mul(blend_size[1])
                .is_some_and(|count| count == blend_count);
        if !empty_override && !populated {
            return Err(invalid_count(identity, offset + 56));
        }
        if empty_override {
            for child_count_offset in [24, 144, 148, 164, 176, 188] {
                if i32_at(bytes, offset + child_count_offset, identity)? != 0 {
                    return Err(invalid_count(identity, offset + child_count_offset));
                }
            }
        }
        let mut animation_indices = Vec::new();
        let mut bone_weights = Vec::new();
        let mut pose_keys = [Vec::new(), Vec::new()];
        if populated {
            let animation_index_offset =
                relative_offset(offset, i32_at(bytes, offset + 60, identity)?, identity)?;
            table(
                bytes,
                animation_index_offset,
                blend_count as usize,
                2,
                limits,
                identity,
            )?;
            animation_indices.reserve(blend_count as usize);
            for animation in 0..blend_count as usize {
                let value = i16_at(bytes, animation_index_offset + animation * 2, identity)?;
                if value < 0 || value as usize >= animation_count {
                    return Err(invalid_reference(
                        identity,
                        animation_index_offset + animation * 2,
                    ));
                }
                animation_indices.push(value);
            }
            let weight_offset =
                relative_offset(offset, i32_at(bytes, offset + 156, identity)?, identity)?;
            table(bytes, weight_offset, bone_count, 4, limits, identity)?;
            bone_weights.reserve(bone_count);
            for bone in 0..bone_count {
                bone_weights.push(float(bytes, weight_offset + bone * 4, identity)?);
            }
            let pose_key_relative = i32_at(bytes, offset + 160, identity)?;
            if pose_key_relative != 0 {
                let pose_key_offset = relative_offset(offset, pose_key_relative, identity)?;
                let pose_key_count = blend_size[0] as usize + blend_size[1] as usize;
                table(bytes, pose_key_offset, pose_key_count, 4, limits, identity)?;
                let mut cursor = pose_key_offset;
                for axis in 0..2 {
                    for _ in 0..blend_size[axis] as usize {
                        pose_keys[axis].push(float(bytes, cursor, identity)?);
                        cursor += 4;
                    }
                }
            }
        }
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
            flags,
            activity: i32_at(bytes, offset + 16, identity)?,
            activity_weight: i32_at(bytes, offset + 20, identity)?,
            event_count: event_count as i32,
            bounds_min: vector3(bytes, offset + 32, identity)?,
            bounds_max: vector3(bytes, offset + 44, identity)?,
            blend_count,
            blend_size,
            animation_indices,
            pose_parameter_indices: [
                i32_at(bytes, offset + 76, identity)?,
                i32_at(bytes, offset + 80, identity)?,
            ],
            pose_parameter_start: [
                float(bytes, offset + 84, identity)?,
                float(bytes, offset + 88, identity)?,
            ],
            pose_parameter_end: [
                float(bytes, offset + 92, identity)?,
                float(bytes, offset + 96, identity)?,
            ],
            fade_in: float(bytes, offset + 104, identity)?,
            fade_out: float(bytes, offset + 108, identity)?,
            entry_node: i32_at(bytes, offset + 112, identity)?,
            exit_node: i32_at(bytes, offset + 116, identity)?,
            node_flags: i32_at(bytes, offset + 120, identity)?,
            entry_phase: float(bytes, offset + 124, identity)?,
            exit_phase: float(bytes, offset + 128, identity)?,
            last_frame: float(bytes, offset + 132, identity)?,
            next_sequence: i32_at(bytes, offset + 136, identity)?,
            pose: i32_at(bytes, offset + 140, identity)?,
            auto_layer_count: auto_layer_count as i32,
            bone_weights,
            pose_keys,
            ik_lock_count: i32_at(bytes, offset + 164, identity)?,
            cycle_pose_parameter: i32_at(bytes, offset + 180, identity)?,
            activity_modifier_count: i32_at(bytes, offset + 188, identity)?,
            events,
            auto_layers,
            source_identity: identity.to_owned(),
        });
    }

    let pose_parameter_count = count(bytes, 300, identity)?;
    let pose_parameter_offset = count(bytes, 304, identity)?;
    if pose_parameter_count > 24 {
        return Err(invalid_count(identity, 300));
    }

    let flex_descriptors = count(bytes, 260, identity)?;
    let flex_descriptor_offset = count(bytes, 264, identity)?;
    if flex_descriptors > 1_024 {
        return Err(invalid_count(identity, 260));
    }
    table(
        bytes,
        flex_descriptor_offset,
        flex_descriptors,
        4,
        limits,
        identity,
    )?;
    let flex_controllers = count(bytes, 268, identity)?;
    let flex_controller_offset = count(bytes, 272, identity)?;
    if flex_controllers > 96 {
        return Err(invalid_count(identity, 268));
    }
    table(
        bytes,
        flex_controller_offset,
        flex_controllers,
        20,
        limits,
        identity,
    )?;
    let flex_rules = count(bytes, 276, identity)?;
    let flex_rule_offset = count(bytes, 280, identity)?;
    table(bytes, flex_rule_offset, flex_rules, 12, limits, identity)?;
    let ik_chains = count(bytes, 284, identity)?;
    let ik_chain_offset = count(bytes, 288, identity)?;
    table(bytes, ik_chain_offset, ik_chains, 16, limits, identity)?;
    table(
        bytes,
        pose_parameter_offset,
        pose_parameter_count,
        POSE_PARAMETER_BYTES,
        limits,
        identity,
    )?;
    let mut pose_parameters = Vec::with_capacity(pose_parameter_count);
    for index in 0..pose_parameter_count {
        let offset = pose_parameter_offset + index * POSE_PARAMETER_BYTES;
        pose_parameters.push(PoseParameter {
            index,
            name: relative_string(
                bytes,
                offset,
                i32_at(bytes, offset, identity)?,
                limits,
                identity,
            )?,
            flags: i32_at(bytes, offset + 4, identity)?,
            start: float(bytes, offset + 8, identity)?,
            end: float(bytes, offset + 12, identity)?,
            looping_range: float(bytes, offset + 16, identity)?,
            source_identity: identity.to_owned(),
        });
    }
    for sequence in &sequences {
        for (axis, &pose) in sequence.pose_parameter_indices.iter().enumerate() {
            if sequence.blend_size[axis] > 1 && (pose < 0 || pose as usize >= pose_parameter_count)
            {
                return Err(invalid_reference(
                    identity,
                    sequence_offset + sequence.index * SEQUENCE_BYTES + 76,
                ));
            }
        }
        if sequence.flags & STUDIO_CYCLE_POSE != 0
            && (sequence.cycle_pose_parameter < 0
                || sequence.cycle_pose_parameter as usize >= pose_parameter_count)
        {
            return Err(invalid_reference(
                identity,
                sequence_offset + sequence.index * SEQUENCE_BYTES + 180,
            ));
        }
        for layer in &sequence.auto_layers {
            if layer.flags & STUDIO_AUTO_LAYER_POSE != 0
                && (layer.pose < 0 || layer.pose as usize >= pose_parameter_count)
            {
                return Err(invalid_reference(
                    identity,
                    sequence_offset
                        + sequence.index * SEQUENCE_BYTES
                        + 152
                        + layer.index * SEQUENCE_AUTO_LAYER_BYTES
                        + 2,
                ));
            }
        }
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
        let mut candidates = Vec::with_capacity(search_paths.len());
        let mut rejected = None;
        for path in &search_paths {
            match material_candidate(path, &name, identity) {
                Ok(candidate) => candidates.push(candidate),
                Err(error) => rejected = Some(error),
            }
        }
        if candidates.is_empty()
            && let Some(error) = rejected
        {
            return Err(error);
        }
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
            let eyeball_count = count(bytes, model_offset + 100, identity)?;
            if eyeball_count > 16 {
                return Err(invalid_count(identity, model_offset + 100));
            }
            let eyeball_offset = if eyeball_count == 0 {
                0
            } else {
                relative_offset(
                    model_offset,
                    i32_at(bytes, model_offset + 104, identity)?,
                    identity,
                )?
            };
            table(
                bytes,
                eyeball_offset,
                eyeball_count,
                EYEBALL_BYTES,
                limits,
                identity,
            )?;
            let mut eyeballs = Vec::with_capacity(eyeball_count);
            for eyeball_index in 0..eyeball_count {
                let eyeball_offset = eyeball_offset + eyeball_index * EYEBALL_BYTES;
                let bone = i32_at(bytes, eyeball_offset + 4, identity)?;
                let texture = i32_at(bytes, eyeball_offset + 52, identity)?;
                if bone < 0 || bone as usize >= bone_count {
                    return Err(invalid_reference(identity, eyeball_offset + 4));
                }
                if texture < 0 || texture as usize >= texture_count {
                    return Err(invalid_reference(identity, eyeball_offset + 52));
                }
                let upper_flex_descriptors = std::array::from_fn(|index| {
                    i32_at(bytes, eyeball_offset + 68 + index * 4, identity)
                        .expect("validated eyeball flex descriptor")
                });
                let lower_flex_descriptors = std::array::from_fn(|index| {
                    i32_at(bytes, eyeball_offset + 80 + index * 4, identity)
                        .expect("validated eyeball flex descriptor")
                });
                let upper_lid_flex_descriptor = i32_at(bytes, eyeball_offset + 116, identity)?;
                let lower_lid_flex_descriptor = i32_at(bytes, eyeball_offset + 120, identity)?;
                for (field, value) in [
                    (68, upper_flex_descriptors[0]),
                    (72, upper_flex_descriptors[1]),
                    (76, upper_flex_descriptors[2]),
                    (80, lower_flex_descriptors[0]),
                    (84, lower_flex_descriptors[1]),
                    (88, lower_flex_descriptors[2]),
                    (116, upper_lid_flex_descriptor),
                    (120, lower_lid_flex_descriptor),
                ] {
                    if value < -1 || (value >= 0 && value as usize >= flex_descriptors) {
                        return Err(invalid_reference(identity, eyeball_offset + field));
                    }
                }
                let origin = vector3(bytes, eyeball_offset + 8, identity)?;
                let z_offset = float(bytes, eyeball_offset + 20, identity)?;
                let radius = float(bytes, eyeball_offset + 24, identity)?;
                let up = vector3(bytes, eyeball_offset + 28, identity)?;
                let forward = vector3(bytes, eyeball_offset + 40, identity)?;
                let iris_scale = float(bytes, eyeball_offset + 60, identity)?;
                let upper_targets = vector3(bytes, eyeball_offset + 92, identity)?;
                let lower_targets = vector3(bytes, eyeball_offset + 104, identity)?;
                if !vector_is_finite(origin)
                    || !float_is_finite(z_offset)
                    || !float_is_positive_finite(radius)
                    || !vector_is_finite(up)
                    || !vector_is_finite(forward)
                    || !float_is_positive_finite(iris_scale)
                    || !vector_is_finite(upper_targets)
                    || !vector_is_finite(lower_targets)
                {
                    return Err(invalid_range(identity, eyeball_offset));
                }
                eyeballs.push(Eyeball {
                    index: eyeball_index,
                    name: relative_string(
                        bytes,
                        eyeball_offset,
                        i32_at(bytes, eyeball_offset, identity)?,
                        limits,
                        identity,
                    )?,
                    bone,
                    origin,
                    z_offset,
                    radius,
                    up,
                    forward,
                    texture,
                    unused_1: i32_at(bytes, eyeball_offset + 56, identity)?,
                    iris_scale,
                    unused_2: i32_at(bytes, eyeball_offset + 64, identity)?,
                    upper_flex_descriptors,
                    lower_flex_descriptors,
                    upper_targets,
                    lower_targets,
                    upper_lid_flex_descriptor,
                    lower_lid_flex_descriptor,
                    unused: std::array::from_fn(|index| {
                        i32_at(bytes, eyeball_offset + 124 + index * 4, identity)
                            .expect("validated eyeball reserved field")
                    }),
                    non_facs: bytes[eyeball_offset + 140] != 0,
                    unused_3: bytes[eyeball_offset + 141..eyeball_offset + 144]
                        .try_into()
                        .expect("validated eyeball reserved bytes"),
                    unused_4: std::array::from_fn(|index| {
                        i32_at(bytes, eyeball_offset + 144 + index * 4, identity)
                            .expect("validated eyeball reserved field")
                    }),
                });
            }
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
                let material_type = i32_at(bytes, mesh_offset + 24, identity)?;
                let material_parameter = i32_at(bytes, mesh_offset + 28, identity)?;
                if material_type == 1
                    && (material_parameter < 0 || material_parameter as usize >= eyeballs.len())
                {
                    return Err(invalid_reference(identity, mesh_offset + 28));
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
                    material_type,
                    material_parameter,
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
                eyeball_count: eyeball_count as i32,
                meshes,
                eyeballs,
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
    if illumination_attachment > 0 && illumination_attachment as usize > attachment_count {
        return Err(invalid_reference(identity, 400));
    }

    let include_count = count(bytes, 336, identity)?;
    if include_count > limits.max_included_models || include_count > 64 {
        return Err(invalid_count(identity, 336));
    }
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
    let animation_block_identity = match i32_at(bytes, 348, identity)? {
        0 => None,
        offset if offset > 0 => {
            let stored = c_string(bytes, offset as usize, limits, identity)?;
            if stored.is_empty() {
                None
            } else {
                Some(canonical_dependency_path(&stored, ".ani", identity)?)
            }
        }
        _ => return Err(invalid_range(identity, 348)),
    };
    if needs_animation && animation_block_identity.is_none() {
        return Err(invalid_reference(identity, 348));
    }

    let document = Document {
        identity: identity.to_owned(),
        profile,
        checksum,
        internal_name,
        declared_length,
        flags,
        root_lod,
        allowed_root_lods,
        bounds,
        illumination_attachment,
        raw_max_eye_deflection,
        max_eye_deflection,
        bones,
        animations,
        sequences,
        materials,
        material_replacements: Vec::new(),
        skins,
        body_parts,
        attachments,
        hitbox_sets,
        pose_parameters,
        unsupported: UnsupportedMetadata {
            flex_descriptors,
            flex_controllers,
            flex_rules,
            ik_chains,
        },
        include_models,
        animation_blocks,
        animation_block_identity,
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
        model_dependencies: Vec::new(),
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
        animation_sources,
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
    expected_materials: usize,
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
    let mut material_replacements = Vec::new();
    for lod in 0..lod_count as usize {
        let list = replacements + lod * 8;
        let count = count(bytes, list, identity)?;
        let offset = relative_offset(list, i32_at(bytes, list + 4, identity)?, identity)?;
        table(bytes, offset, count, 6, limits, identity)?;
        for index in 0..count {
            let record = offset + index * 6;
            let material_slot = i16_at(bytes, record, identity)?;
            if material_slot < 0 || material_slot as usize >= expected_materials {
                return Err(invalid_reference(identity, record));
            }
            material_replacements.push(ParsedMaterialReplacement {
                lod,
                material_slot: material_slot as usize,
                name: relative_string(
                    bytes,
                    record,
                    i32_at(bytes, record + 2, identity)?,
                    limits,
                    identity,
                )?,
            });
        }
    }
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
                            triangles.extend(derived_strip_triangles(selected, flags).ok_or_else(
                                || {
                                    failure(
                                        Classification::Unsupported,
                                        ErrorCode::UnsupportedCompanion,
                                        identity,
                                        Some(strip + 18..strip + 19),
                                    )
                                },
                            )?);
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
        material_replacements,
    })
}

fn derived_strip_triangles(indices: &[u16], flags: u8) -> Option<Vec<[u32; 3]>> {
    let triangles = match flags {
        1 if indices.len().is_multiple_of(3) => indices
            .chunks_exact(3)
            .map(|triangle| [triangle[0] as u32, triangle[1] as u32, triangle[2] as u32])
            .collect::<Vec<_>>(),
        2 => (0..indices.len().saturating_sub(2))
            .map(|at| {
                if at.is_multiple_of(2) {
                    [
                        indices[at] as u32,
                        indices[at + 2] as u32,
                        indices[at + 1] as u32,
                    ]
                } else {
                    [
                        indices[at] as u32,
                        indices[at + 1] as u32,
                        indices[at + 2] as u32,
                    ]
                }
            })
            .collect::<Vec<_>>(),
        _ => return None,
    };
    Some(
        triangles
            .into_iter()
            .filter(|triangle| {
                triangle[0] != triangle[1]
                    && triangle[1] != triangle[2]
                    && triangle[0] != triangle[2]
            })
            .collect(),
    )
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

struct AnimationDecodeContext<'a> {
    mdl: &'a [u8],
    ani: Option<&'a [u8]>,
    blocks: &'a [Range<usize>],
    bones: &'a [Bone],
    identity: &'a str,
    limits: Limits,
}

#[derive(Clone, Copy)]
struct AnimationValueCursor {
    offset: usize,
    first_frame: usize,
}

type AnimationValueCursors = HashMap<(usize, usize, i16), AnimationValueCursor>;

fn decode_animation_frames(
    animations: &mut [Animation],
    sources: &[AnimationSource],
    context: &AnimationDecodeContext<'_>,
) -> Result<(), Error> {
    let mut decoded_samples = 0_usize;
    for (animation, source) in animations.iter_mut().zip(sources) {
        if animation.frame_count <= 0 {
            return Err(invalid_count(
                context.identity,
                source.descriptor_offset + 16,
            ));
        }
        decoded_samples = decoded_samples
            .checked_add(
                (animation.frame_count as usize)
                    .checked_mul(context.bones.len())
                    .ok_or_else(|| {
                        invalid_count(context.identity, source.descriptor_offset + 16)
                    })?,
            )
            .ok_or_else(|| invalid_count(context.identity, source.descriptor_offset + 16))?;
        if decoded_samples > context.limits.max_decoded_animation_samples {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::InputLimit,
                context.identity,
                Some(source.descriptor_offset + 16..source.descriptor_offset + 20),
            ));
        }
        let mut frames = Vec::with_capacity(animation.frame_count as usize);
        let mut value_cursors = AnimationValueCursors::new();
        let mut visited_bones = vec![0_usize; context.bones.len()];
        for frame in 0..animation.frame_count as usize {
            let (data, offset, local_frame) = animation_data(
                context.mdl,
                context.ani,
                context.blocks,
                animation,
                source,
                frame,
                context.identity,
            )?;
            frames.push(decode_frame(
                data,
                offset,
                local_frame,
                context.bones,
                animation.flags,
                context.identity,
                &mut value_cursors,
                &mut visited_bones,
                frame + 1,
            )?);
        }
        let section_count = if animation.section_frame_count > 0 {
            source.sections.len()
        } else {
            1
        };
        let mut sections = Vec::with_capacity(section_count);
        for section_index in 0..section_count {
            let selected: Vec<_> = (0..animation.frame_count as usize)
                .filter(|&frame| animation_section_index(animation, frame) == section_index)
                .collect();
            let (block, data_offset) = if animation.section_frame_count > 0 {
                source.sections[section_index]
            } else {
                (animation.animation_block, source.data_offset)
            };
            let (first_frame, frame_count, tracks) = if let Some(&first_frame) = selected.first() {
                let (data, offset, _) = animation_data(
                    context.mdl,
                    context.ani,
                    context.blocks,
                    animation,
                    source,
                    first_frame,
                    context.identity,
                )?;
                (
                    first_frame,
                    selected.len(),
                    parse_animation_tracks(
                        data,
                        offset,
                        selected.len(),
                        context.bones,
                        context.identity,
                    )?,
                )
            } else {
                (0, 0, Vec::new())
            };
            sections.push(AnimationSection {
                index: section_index,
                first_frame,
                frame_count,
                block,
                data_offset,
                tracks,
            });
        }
        animation.frames = frames;
        animation.sections = sections;
    }
    Ok(())
}

fn animation_section_index(animation: &Animation, frame: usize) -> usize {
    let section_frames = animation.section_frame_count.max(0) as usize;
    if section_frames == 0 {
        0
    } else if animation.frame_count as usize > section_frames
        && frame == animation.frame_count as usize - 1
    {
        frame / section_frames + 1
    } else {
        frame / section_frames
    }
}

fn animation_data<'a>(
    mdl: &'a [u8],
    ani: Option<&'a [u8]>,
    blocks: &[Range<usize>],
    animation: &Animation,
    source: &AnimationSource,
    frame: usize,
    identity: &str,
) -> Result<(&'a [u8], usize, usize), Error> {
    let section_frames = animation.section_frame_count.max(0) as usize;
    let (block, relative, local_frame) = if section_frames == 0 {
        (animation.animation_block, source.data_offset, frame)
    } else {
        let section = animation_section_index(animation, frame);
        let &(block, relative) = source
            .sections
            .get(section)
            .ok_or_else(|| invalid_reference(identity, source.descriptor_offset))?;
        (block, relative, frame % section_frames)
    };
    let relative =
        usize::try_from(relative).map_err(|_| invalid_range(identity, source.descriptor_offset))?;
    if block == 0 {
        let offset = source
            .descriptor_offset
            .checked_add(relative)
            .ok_or_else(|| invalid_range(identity, source.descriptor_offset))?;
        range(mdl, offset, 4, identity)?;
        Ok((mdl, offset, local_frame))
    } else {
        let ani = ani.ok_or_else(|| {
            failure(
                Classification::Missing,
                ErrorCode::MissingDependency,
                identity,
                None,
            )
        })?;
        let block_index = usize::try_from(block)
            .map_err(|_| invalid_reference(identity, source.descriptor_offset))?;
        let block_range = blocks
            .get(block_index)
            .ok_or_else(|| invalid_reference(identity, source.descriptor_offset))?;
        if block_range.end > ani.len() {
            return Err(invalid_range(identity, block_range.start));
        }
        let offset = block_range
            .start
            .checked_add(relative)
            .ok_or_else(|| invalid_range(identity, block_range.start))?;
        range(ani, offset, 4, identity)?;
        Ok((ani, offset, local_frame))
    }
}

fn decode_frame(
    data: &[u8],
    offset: usize,
    frame: usize,
    bones: &[Bone],
    flags: i32,
    identity: &str,
    value_cursors: &mut AnimationValueCursors,
    visited_bones: &mut [usize],
    visit_generation: usize,
) -> Result<AnimationFrame, Error> {
    let delta = flags & 0x0004 != 0;
    let mut translations: Vec<_> = bones
        .iter()
        .map(|bone| {
            if delta {
                Vector3([Float32(0); 3])
            } else {
                bone.position
            }
        })
        .collect();
    let mut rotations: Vec<_> = bones
        .iter()
        .map(|bone| {
            if delta {
                [
                    Float32(0),
                    Float32(0),
                    Float32(0),
                    Float32(1.0_f32.to_bits()),
                ]
            } else {
                bone.quaternion
            }
        })
        .collect();
    let mut cursor = offset;
    for _ in 0..=bones.len() {
        range(data, cursor, 4, identity)?;
        let bone_index = data[cursor] as usize;
        if bone_index == 255 {
            return Ok(AnimationFrame {
                translations,
                rotations,
            });
        }
        let bone = bones
            .get(bone_index)
            .ok_or_else(|| invalid_reference(identity, cursor))?;
        let visited = visited_bones
            .get_mut(bone_index)
            .ok_or_else(|| invalid_reference(identity, cursor))?;
        if *visited == visit_generation {
            return Err(invalid_reference(identity, cursor));
        }
        *visited = visit_generation;
        let track_flags = data[cursor + 1];
        let next = u16_at(data, cursor + 2, identity)? as usize;
        let mut payload = cursor + 4;
        let track_delta = track_flags & 0x10 != 0;
        let mut rotation = if track_flags & 0x02 != 0 {
            let (value, size) = compressed_quaternion(data, payload, false, identity)?;
            payload += size;
            value
        } else if track_flags & 0x20 != 0 {
            let (value, size) = compressed_quaternion(data, payload, true, identity)?;
            payload += size;
            value
        } else if track_flags & 0x08 != 0 {
            let mut euler = [0.0; 3];
            for (axis, component) in euler.iter_mut().enumerate() {
                *component = animation_value(
                    data,
                    payload,
                    i16_at(data, payload + axis * 2, identity)?,
                    frame,
                    f32::from_bits(bone.rotation_scale.0[axis].0),
                    identity,
                    value_cursors,
                )?;
                if !track_delta {
                    *component += f32::from_bits(bone.rotation.0[axis].0);
                }
            }
            payload += 6;
            euler_quaternion(euler).map(Float32)
        } else if track_delta {
            [
                Float32(0),
                Float32(0),
                Float32(0),
                Float32(1.0_f32.to_bits()),
            ]
        } else {
            bone.quaternion
        };
        if !track_delta && bone.flags & 0x0010_0000 != 0 {
            let alignment_distance: f32 = rotation
                .iter()
                .zip(bone.alignment)
                .map(|(left, right)| {
                    let value = f32::from_bits(left.0) - f32::from_bits(right.0);
                    value * value
                })
                .sum();
            let inverse_distance: f32 = rotation
                .iter()
                .zip(bone.alignment)
                .map(|(left, right)| {
                    let value = f32::from_bits(left.0) + f32::from_bits(right.0);
                    value * value
                })
                .sum();
            if alignment_distance > inverse_distance {
                rotation = rotation.map(|value| Float32((-f32::from_bits(value.0)).to_bits()));
            }
        }
        rotations[bone_index] = rotation;
        translations[bone_index] = if track_flags & 0x01 != 0 {
            let value = [
                half_to_f32(u16_at(data, payload, identity)?),
                half_to_f32(u16_at(data, payload + 2, identity)?),
                half_to_f32(u16_at(data, payload + 4, identity)?),
            ];
            Vector3(value.map(|value| Float32(value.to_bits())))
        } else if track_flags & 0x04 != 0 {
            let mut value = [0.0; 3];
            for (axis, component) in value.iter_mut().enumerate() {
                *component = animation_value(
                    data,
                    payload,
                    i16_at(data, payload + axis * 2, identity)?,
                    frame,
                    f32::from_bits(bone.position_scale.0[axis].0),
                    identity,
                    value_cursors,
                )?;
                if !track_delta {
                    *component += f32::from_bits(bone.position.0[axis].0);
                }
            }
            Vector3(value.map(|value| Float32(value.to_bits())))
        } else if track_delta {
            Vector3([Float32(0); 3])
        } else {
            bone.position
        };
        if next == 0 {
            return Ok(AnimationFrame {
                translations,
                rotations,
            });
        }
        cursor = cursor
            .checked_add(next)
            .ok_or_else(|| invalid_range(identity, cursor))?;
    }
    Err(invalid_reference(identity, cursor))
}

fn parse_animation_tracks(
    data: &[u8],
    offset: usize,
    frame_count: usize,
    bones: &[Bone],
    identity: &str,
) -> Result<Vec<AnimationTrack>, Error> {
    let mut output = Vec::new();
    let mut cursor = offset;
    let mut visited = BTreeSet::new();
    for _ in 0..=bones.len() {
        range(data, cursor, 4, identity)?;
        let bone = data[cursor] as usize;
        if bone == 255 {
            return Ok(output);
        }
        if bone >= bones.len() || !visited.insert(bone) {
            return Err(invalid_reference(identity, cursor));
        }
        let flags = data[cursor + 1];
        let next_offset = u16_at(data, cursor + 2, identity)?;
        let mut payload = cursor + 4;
        let rotation_codec = if flags & 0x02 != 0 {
            range(data, payload, 6, identity)?;
            payload += 6;
            RotationCodec::Quaternion48
        } else if flags & 0x20 != 0 {
            range(data, payload, 8, identity)?;
            payload += 8;
            RotationCodec::Quaternion64
        } else if flags & 0x08 != 0 {
            range(data, payload, 6, identity)?;
            RotationCodec::RleEuler
        } else if flags & 0x10 != 0 {
            RotationCodec::DeltaIdentity
        } else {
            RotationCodec::Bind
        };
        let rotation_values = if rotation_codec == RotationCodec::RleEuler {
            let streams = parse_animation_value_streams(data, payload, frame_count, identity)?;
            payload += 6;
            streams
        } else {
            std::array::from_fn(|_| None)
        };
        let translation_codec = if flags & 0x01 != 0 {
            range(data, payload, 6, identity)?;
            TranslationCodec::Vector48
        } else if flags & 0x04 != 0 {
            range(data, payload, 6, identity)?;
            TranslationCodec::RleVector
        } else if flags & 0x10 != 0 {
            TranslationCodec::DeltaZero
        } else {
            TranslationCodec::Bind
        };
        let translation_values = if translation_codec == TranslationCodec::RleVector {
            parse_animation_value_streams(data, payload, frame_count, identity)?
        } else {
            std::array::from_fn(|_| None)
        };
        output.push(AnimationTrack {
            bone,
            flags,
            source_offset: cursor,
            next_offset,
            rotation_codec,
            translation_codec,
            rotation_values,
            translation_values,
        });
        if next_offset == 0 {
            return Ok(output);
        }
        cursor = cursor
            .checked_add(next_offset as usize)
            .ok_or_else(|| invalid_range(identity, cursor))?;
    }
    Err(invalid_reference(identity, cursor))
}

fn parse_animation_value_streams(
    data: &[u8],
    table_offset: usize,
    frame_count: usize,
    identity: &str,
) -> Result<[Option<AnimationValueStream>; 3], Error> {
    let mut streams = std::array::from_fn(|_| None);
    for (axis, output) in streams.iter_mut().enumerate() {
        let relative_offset = i16_at(data, table_offset + axis * 2, identity)?;
        if relative_offset <= 0 {
            continue;
        }
        let mut cursor = table_offset
            .checked_add(relative_offset as usize)
            .ok_or_else(|| invalid_range(identity, table_offset))?;
        let mut covered = 0_usize;
        let mut runs = Vec::new();
        while covered < frame_count {
            range(data, cursor, 2, identity)?;
            let valid = data[cursor];
            let total = data[cursor + 1];
            if valid == 0 || total == 0 || valid > total {
                return Err(invalid_reference(identity, cursor));
            }
            range(data, cursor + 2, valid as usize * 2, identity)?;
            let mut values = Vec::with_capacity(valid as usize);
            for value in 0..valid as usize {
                values.push(i16_at(data, cursor + 2 + value * 2, identity)?);
            }
            runs.push(AnimationValueRun {
                valid,
                total,
                values,
            });
            covered = covered
                .checked_add(total as usize)
                .ok_or_else(|| invalid_range(identity, cursor))?;
            cursor += 2 + valid as usize * 2;
        }
        *output = Some(AnimationValueStream {
            relative_offset,
            runs,
        });
    }
    Ok(streams)
}

fn compressed_quaternion(
    data: &[u8],
    offset: usize,
    wide: bool,
    identity: &str,
) -> Result<([Float32; 4], usize), Error> {
    let (x, y, z, negative, size) = if wide {
        let low = u32_at(data, offset, identity)?;
        let high = u32_at(data, offset + 4, identity)?;
        (
            (low & 0x1f_ffff) as f32 / 1_048_576.5 - 1_048_576.0 / 1_048_576.5,
            ((((high & 0x03ff) << 11) | (low >> 21)) & 0x1f_ffff) as f32 / 1_048_576.5
                - 1_048_576.0 / 1_048_576.5,
            ((high >> 10) & 0x1f_ffff) as f32 / 1_048_576.5 - 1_048_576.0 / 1_048_576.5,
            high & 0x8000_0000 != 0,
            8,
        )
    } else {
        let x = u16_at(data, offset, identity)?;
        let y = u16_at(data, offset + 2, identity)?;
        let z = u16_at(data, offset + 4, identity)?;
        (
            (x as i32 - 32_768) as f32 / 32_768.0,
            (y as i32 - 32_768) as f32 / 32_768.0,
            ((z & 0x7fff) as i32 - 16_384) as f32 / 16_384.0,
            z & 0x8000 != 0,
            6,
        )
    };
    let squared = x * x + y * y + z * z;
    if squared > 1.001 {
        return Err(invalid_reference(identity, offset));
    }
    let w = (1.0 - squared).max(0.0).sqrt() * if negative { -1.0 } else { 1.0 };
    Ok(([x, y, z, w].map(|value| Float32(value.to_bits())), size))
}

fn animation_value(
    data: &[u8],
    table: usize,
    relative: i16,
    frame: usize,
    scale: f32,
    identity: &str,
    cursors: &mut AnimationValueCursors,
) -> Result<f32, Error> {
    if relative <= 0 {
        return Ok(0.0);
    }
    let initial = table
        .checked_add(relative as usize)
        .ok_or_else(|| invalid_range(identity, table))?;
    let key = (data.as_ptr() as usize, table, relative);
    let cursor = cursors.entry(key).or_insert(AnimationValueCursor {
        offset: initial,
        first_frame: 0,
    });
    if frame < cursor.first_frame {
        *cursor = AnimationValueCursor {
            offset: initial,
            first_frame: 0,
        };
    }
    loop {
        range(data, cursor.offset, 2, identity)?;
        let valid = data[cursor.offset] as usize;
        let total = data[cursor.offset + 1] as usize;
        if valid == 0 || total == 0 || valid > total {
            return Err(invalid_reference(identity, cursor.offset));
        }
        range(data, cursor.offset + 2, valid * 2, identity)?;
        let remaining = frame - cursor.first_frame;
        if remaining < total {
            return Ok(i16_at(
                data,
                cursor.offset + 2 + remaining.min(valid - 1) * 2,
                identity,
            )? as f32
                * scale);
        }
        cursor.first_frame = cursor
            .first_frame
            .checked_add(total)
            .ok_or_else(|| invalid_range(identity, cursor.offset))?;
        cursor.offset = cursor
            .offset
            .checked_add(2 + valid * 2)
            .ok_or_else(|| invalid_range(identity, cursor.offset))?;
    }
}

fn euler_quaternion([roll, pitch, yaw]: [f32; 3]) -> [u32; 4] {
    let (sr, cr) = (roll * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    let (sy, cy) = (yaw * 0.5).sin_cos();
    [
        (sr * cp * cy - cr * sp * sy).to_bits(),
        (cr * sp * cy + sr * cp * sy).to_bits(),
        (cr * cp * sy - sr * sp * cy).to_bits(),
        (cr * cp * cy + sr * sp * sy).to_bits(),
    ]
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let fraction = bits & 0x03ff;
    let encoded = match exponent {
        0 if fraction == 0 => sign,
        0 => {
            let mut fraction = fraction as u32;
            let mut exponent = 113_u32;
            while fraction & 0x400 == 0 {
                fraction <<= 1;
                exponent -= 1;
            }
            sign | (exponent << 23) | ((fraction & 0x3ff) << 13)
        }
        31 => sign | 0x7f80_0000 | ((fraction as u32) << 13),
        value => sign | (((value as u32) + 112) << 23) | ((fraction as u32) << 13),
    };
    f32::from_bits(encoded)
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    if limits.max_file_bytes == 0
        || limits.max_aggregate_input_bytes == 0
        || limits.max_dependency_count == 0
        || limits.max_include_depth == 0
        || limits.max_included_models == 0
        || limits.max_records == 0
        || limits.max_strings == 0
        || limits.max_string_bytes == 0
        || limits.max_owned_bytes == 0
        || limits.max_animation_frames == 0
        || limits.max_decoded_animation_samples == 0
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
        || !identity.bytes().all(safe_canonical_path_byte)
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
    if !text.bytes().all(safe_stored_path_byte) {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidIdentity,
            identity,
            None,
        ));
    }
    let canonical = text.replace('\\', "/").to_ascii_lowercase();
    validate_model_identity(&canonical)?;
    Ok(canonical)
}

fn canonical_dependency_path(bytes: &[u8], suffix: &str, identity: &str) -> Result<String, Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        failure(
            Classification::Malformed,
            ErrorCode::InvalidString,
            identity,
            None,
        )
    })?;
    if !text.bytes().all(safe_stored_path_byte) {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidIdentity,
            identity,
            None,
        ));
    }
    let canonical = text.replace('\\', "/").to_ascii_lowercase();
    if !canonical.starts_with("models/")
        || !canonical.ends_with(suffix)
        || !canonical.bytes().all(safe_canonical_path_byte)
        || canonical
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
    Ok(canonical)
}

fn material_candidate(path: &[u8], name: &[u8], identity: &str) -> Result<String, Error> {
    let normalize = |bytes: &[u8]| -> Result<String, Error> {
        let text = std::str::from_utf8(bytes).map_err(|_| {
            failure(
                Classification::Malformed,
                ErrorCode::InvalidString,
                identity,
                None,
            )
        })?;
        if !text.bytes().all(safe_stored_path_byte) {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::InvalidIdentity,
                identity,
                None,
            ));
        }
        let normalized = text.replace('\\', "/");
        Ok(normalized
            .strip_prefix('/')
            .unwrap_or(&normalized)
            .to_ascii_lowercase())
    };
    let path = normalize(path)?;
    if path.split('/').any(|part| part == "." || part == "..") {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidIdentity,
            identity,
            None,
        ));
    }
    let mut name = normalize(name)?;
    if name.ends_with(".vmt") {
        name.truncate(name.len() - 4);
    }
    let relative = if path.is_empty() {
        name
    } else if path.ends_with('/') {
        format!("{path}{name}")
    } else {
        format!("{path}/{name}")
    };
    let mut canonical = Vec::new();
    for part in relative.split('/') {
        match part {
            "" | "." => {
                return Err(failure(
                    Classification::Malformed,
                    ErrorCode::InvalidIdentity,
                    identity,
                    None,
                ));
            }
            ".." => {
                if canonical.pop().is_none() {
                    return Err(failure(
                        Classification::Malformed,
                        ErrorCode::InvalidIdentity,
                        identity,
                        None,
                    ));
                }
            }
            _ => canonical.push(part),
        }
    }
    if canonical.is_empty() {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::InvalidIdentity,
            identity,
            None,
        ));
    }
    Ok(format!("materials/{}.vmt", canonical.join("/")))
}

fn safe_stored_path_byte(byte: u8) -> bool {
    byte.is_ascii() && !byte.is_ascii_control() && !matches!(byte, b':' | b'?' | b'#')
}

fn safe_canonical_path_byte(byte: u8) -> bool {
    safe_stored_path_byte(byte) && !byte.is_ascii_uppercase()
}

fn response_matches(response: &DependencyResponse<'_>, request: &DependencyRequest) -> bool {
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
            .map(|value| {
                value.label.len()
                    + value.activity_name.len()
                    + value
                        .events
                        .iter()
                        .map(|event| event.name.len())
                        .sum::<usize>()
                    + value.events.len() * std::mem::size_of::<SequenceEvent>()
                    + value.auto_layers.len() * std::mem::size_of::<SequenceAutoLayer>()
            })
            .sum::<usize>()
        + document
            .hitbox_sets
            .iter()
            .map(|set| {
                set.name.len()
                    + set
                        .hitboxes
                        .iter()
                        .map(|hitbox| hitbox.name.len())
                        .sum::<usize>()
                    + set.hitboxes.len() * std::mem::size_of::<Hitbox>()
            })
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
        + document
            .body_parts
            .iter()
            .map(|part| {
                part.name.len()
                    + part
                        .models
                        .iter()
                        .map(|model| {
                            model.name.len()
                                + model.eyeballs.len() * std::mem::size_of::<Eyeball>()
                                + model
                                    .eyeballs
                                    .iter()
                                    .map(|eyeball| eyeball.name.len())
                                    .sum::<usize>()
                        })
                        .sum::<usize>()
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

fn float_is_finite(value: Float32) -> bool {
    f32::from_bits(value.0).is_finite()
}

fn float_is_positive_finite(value: Float32) -> bool {
    let value = f32::from_bits(value.0);
    value.is_finite() && value > 0.0
}

fn vector_is_finite(value: Vector3) -> bool {
    value.0.into_iter().all(float_is_finite)
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

fn i16_at(bytes: &[u8], offset: usize, identity: &str) -> Result<i16, Error> {
    range(bytes, offset, 2, identity)?;
    Ok(i16::from_le_bytes(
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

    fn response(
        request: &DependencyRequest,
        bytes: Option<Vec<u8>>,
    ) -> DependencyResponse<'static> {
        DependencyResponse {
            requester: request.requester.clone(),
            role: request.role,
            logical_path: request.logical_path.clone(),
            bytes: bytes.map(Cow::Owned),
        }
    }

    #[test]
    fn model_dependency_responses_can_borrow_immutable_authored_source_bytes() {
        let source = [1_u8, 2, 3, 4];
        let response = DependencyResponse {
            requester: "models/test.mdl".to_owned(),
            role: DependencyRole::VertexData,
            logical_path: "models/test.vvd".to_owned(),
            bytes: Some(Cow::Borrowed(&source)),
        };
        let retained = response.bytes.as_ref().unwrap();
        assert!(matches!(retained, Cow::Borrowed(_)));
        assert_eq!(retained.as_ptr(), source.as_ptr());
        assert_eq!(retained.as_ref(), source.as_slice());
    }

    fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn animated_mdl(external: bool) -> Vec<u8> {
        let mut bytes = vec![0; MDL_HEADER_BYTES];
        bytes[..4].copy_from_slice(b"IDST");
        put_i32(&mut bytes, 4, 48);
        put_i32(&mut bytes, 8, 1234);
        bytes[12..17].copy_from_slice(b"anim\0");
        let bone_offset = bytes.len();
        bytes.resize(bone_offset + BONE_BYTES, 0);
        put_i32(&mut bytes, 156, 1);
        put_i32(&mut bytes, 160, bone_offset as i32);
        put_i32(&mut bytes, bone_offset + 4, -1);
        for controller in 0..6 {
            put_i32(&mut bytes, bone_offset + 8 + controller * 4, -1);
        }
        put_i32(&mut bytes, bone_offset + 56, 1.0_f32.to_bits() as i32);
        let bone_name = bytes.len();
        bytes.extend_from_slice(b"root\0");
        put_i32(&mut bytes, bone_offset, (bone_name - bone_offset) as i32);

        let animation_offset = bytes.len();
        bytes.resize(animation_offset + ANIMATION_BYTES, 0);
        put_i32(&mut bytes, 180, 1);
        put_i32(&mut bytes, 184, animation_offset as i32);
        put_i32(&mut bytes, animation_offset + 8, 30.0_f32.to_bits() as i32);
        put_i32(&mut bytes, animation_offset + 16, 2);
        put_i32(&mut bytes, animation_offset + 52, i32::from(external));
        let animation_name = bytes.len();
        bytes.extend_from_slice(b"idle_anim\0");
        put_i32(
            &mut bytes,
            animation_offset + 4,
            (animation_name - animation_offset) as i32,
        );
        if !external {
            let track = bytes.len();
            bytes.extend_from_slice(&[255, 0, 0, 0]);
            put_i32(
                &mut bytes,
                animation_offset + 56,
                (track - animation_offset) as i32,
            );
        }

        let sequence_offset = bytes.len();
        bytes.resize(sequence_offset + SEQUENCE_BYTES, 0);
        put_i32(&mut bytes, 188, 1);
        put_i32(&mut bytes, 192, sequence_offset as i32);
        put_i32(&mut bytes, sequence_offset + 16, 99);
        put_i32(&mut bytes, sequence_offset + 20, 1);
        put_i32(&mut bytes, sequence_offset + 56, 1);
        put_i32(&mut bytes, sequence_offset + 68, 1);
        put_i32(&mut bytes, sequence_offset + 72, 1);
        let animation_grid = bytes.len();
        bytes.extend_from_slice(&0_i16.to_le_bytes());
        put_i32(
            &mut bytes,
            sequence_offset + 60,
            (animation_grid - sequence_offset) as i32,
        );
        let weights = bytes.len();
        bytes.extend_from_slice(&1.0_f32.to_le_bytes());
        put_i32(
            &mut bytes,
            sequence_offset + 156,
            (weights - sequence_offset) as i32,
        );
        let label = bytes.len();
        bytes.extend_from_slice(b"idle\0");
        put_i32(
            &mut bytes,
            sequence_offset + 4,
            (label - sequence_offset) as i32,
        );
        let activity = bytes.len();
        bytes.extend_from_slice(b"ACT_VM_IDLE\0");
        put_i32(
            &mut bytes,
            sequence_offset + 8,
            (activity - sequence_offset) as i32,
        );

        if external {
            let ani_name = bytes.len();
            bytes.extend_from_slice(b"models/custom/shared.ani\0");
            put_i32(&mut bytes, 348, ani_name as i32);
            let blocks = bytes.len();
            bytes.resize(blocks + 16, 0);
            put_i32(&mut bytes, 352, 2);
            put_i32(&mut bytes, 356, blocks as i32);
            put_i32(&mut bytes, blocks + 8, 12);
            put_i32(&mut bytes, blocks + 12, 16);
        }
        let length = bytes.len() as i32;
        put_i32(&mut bytes, 76, length);
        bytes
    }

    fn mdl_with_include(path: &str) -> Vec<u8> {
        let mut bytes = mdl(48, false);
        let include = bytes.len();
        bytes.resize(include + 8, 0);
        put_i32(&mut bytes, 336, 1);
        put_i32(&mut bytes, 340, include as i32);
        let name = bytes.len();
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
        put_i32(&mut bytes, include + 4, (name - include) as i32);
        let length = bytes.len() as i32;
        put_i32(&mut bytes, 76, length);
        bytes
    }

    fn mdl_with_material() -> Vec<u8> {
        let mut bytes = mdl(44, true);
        let texture = bytes.len();
        bytes.resize(texture + TEXTURE_BYTES, 0);
        put_i32(&mut bytes, 204, 1);
        put_i32(&mut bytes, 208, texture as i32);
        let texture_name = bytes.len();
        bytes.extend_from_slice(b"base_material\0");
        put_i32(&mut bytes, texture, (texture_name - texture) as i32);
        let search_table = bytes.len();
        bytes.resize(search_table + 4, 0);
        put_i32(&mut bytes, 212, 1);
        put_i32(&mut bytes, 216, search_table as i32);
        let search_path = bytes.len();
        bytes.extend_from_slice(b"models/test\0");
        put_i32(&mut bytes, search_table, search_path as i32);
        let skin = bytes.len();
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        put_i32(&mut bytes, 220, 1);
        put_i32(&mut bytes, 224, 1);
        put_i32(&mut bytes, 228, skin as i32);
        let length = bytes.len() as i32;
        put_i32(&mut bytes, 76, length);
        bytes
    }

    fn nested_item_mdl() -> Vec<u8> {
        let mut bytes = mdl_with_material();
        put_i32(&mut bytes, 4, 48);
        let texture =
            i32::from_le_bytes(bytes[208..212].try_into().expect("texture offset")) as usize;
        let texture_name = bytes.len();
        bytes.extend_from_slice(b"models/player/items/soldier/soldier_viking\0");
        put_i32(&mut bytes, texture, (texture_name - texture) as i32);
        let search_table = bytes.len();
        bytes.resize(search_table + 8, 0);
        put_i32(&mut bytes, 212, 2);
        put_i32(&mut bytes, 216, search_table as i32);
        let nested_directory = bytes.len();
        bytes.extend_from_slice(b"\\models\\player\\items\\soldier\\\0");
        put_i32(&mut bytes, search_table, nested_directory as i32);
        let root_directory = bytes.len();
        bytes.push(0);
        put_i32(&mut bytes, search_table + 4, root_directory as i32);
        let empty_animation_identity = bytes.len();
        bytes.push(0);
        put_i32(&mut bytes, 348, empty_animation_identity as i32);
        let animation_blocks = bytes.len();
        bytes.resize(animation_blocks + 8, 0);
        put_i32(&mut bytes, 352, 1);
        put_i32(&mut bytes, 356, animation_blocks as i32);
        let length = bytes.len() as i32;
        put_i32(&mut bytes, 76, length);
        bytes
    }

    fn mdl_with_empty_override_sequence() -> (Vec<u8>, usize) {
        let mut bytes = mdl(48, false);
        let sequence = bytes.len();
        bytes.resize(sequence + SEQUENCE_BYTES, 0);
        put_i32(&mut bytes, 188, 1);
        put_i32(&mut bytes, 192, sequence as i32);
        put_i32(&mut bytes, sequence + 12, STUDIO_OVERRIDE);
        put_i32(&mut bytes, sequence + 60, 76_804);
        let label = bytes.len();
        bytes.extend_from_slice(b"user_ref\0");
        put_i32(&mut bytes, sequence + 4, (label - sequence) as i32);
        let length = bytes.len() as i32;
        put_i32(&mut bytes, 76, length);
        (bytes, sequence)
    }

    fn vtx_with_replacement(checksum: i32) -> Vec<u8> {
        let mut bytes = vtx(checksum);
        put_i32(&mut bytes, 44, 1);
        put_i32(&mut bytes, 48, 8);
        bytes.resize(58, 0);
        put_i32(&mut bytes, 54, 6);
        bytes.extend_from_slice(b"lod_material\0");
        bytes
    }

    #[test]
    fn animation_value_cursors_preserve_authored_runs_and_rewind_exactly() {
        let mut bytes = vec![0; 6];
        bytes.extend_from_slice(&[2, 4]);
        bytes.extend_from_slice(&10_i16.to_le_bytes());
        bytes.extend_from_slice(&20_i16.to_le_bytes());
        bytes.extend_from_slice(&[1, 2]);
        bytes.extend_from_slice(&30_i16.to_le_bytes());
        let mut cursors = AnimationValueCursors::new();

        let values = (0..6)
            .map(|frame| animation_value(&bytes, 0, 6, frame, 0.5, "animation", &mut cursors))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(values, [5.0, 10.0, 10.0, 10.0, 15.0, 15.0]);
        assert_eq!(cursors.len(), 1);
        assert_eq!(
            animation_value(&bytes, 0, 6, 1, 0.5, "animation", &mut cursors).unwrap(),
            10.0,
        );
        assert_eq!(
            animation_value(&bytes, 0, 6, 5, 0.5, "animation", &mut cursors).unwrap(),
            15.0,
        );
    }

    #[test]
    fn animation_value_cursors_visit_long_frame_streams_once() {
        const FRAMES: usize = 8_192;
        let mut bytes = vec![0; 6];
        for frame in 0..FRAMES {
            bytes.extend_from_slice(&[1, 1]);
            bytes.extend_from_slice(&(frame as i16).to_le_bytes());
        }
        let mut cursors = AnimationValueCursors::new();
        for frame in 0..FRAMES {
            assert_eq!(
                animation_value(&bytes, 0, 6, frame, 1.0, "animation", &mut cursors).unwrap(),
                frame as f32,
            );
        }
        let cursor = cursors.values().next().unwrap();
        assert_eq!(cursor.first_frame, FRAMES - 1);
        assert_eq!(cursor.offset, 6 + (FRAMES - 1) * 4);
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
        bad[0].bytes = Some(vvd(5678).into());
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

        let mut malformed_procedure = animated_mdl(false);
        let bone_offset = i32::from_le_bytes(
            malformed_procedure[160..164]
                .try_into()
                .expect("bone offset"),
        ) as usize;
        put_i32(&mut malformed_procedure, bone_offset + 164, 1);
        assert_eq!(
            load(
                "models/procedure.mdl",
                Profile::SourcePcMdl48,
                VtxVariant::Dx90,
                &malformed_procedure,
                &[],
                Limits::default(),
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidReference
        );
    }

    #[test]
    fn parses_sequence_children_and_uses_the_authored_ani_identity() {
        let bytes = animated_mdl(true);
        let Load::Needs(requests) = load(
            "models/animated.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &bytes,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("animated model did not request dependencies")
        };
        assert_eq!(
            requests
                .iter()
                .map(|request| (request.role, request.logical_path.as_str()))
                .collect::<Vec<_>>(),
            [
                (DependencyRole::AnimationBlocks, "models/custom/shared.ani"),
                (DependencyRole::Physics, "models/animated.phy"),
            ]
        );
        let mut ani = vec![0; 16];
        ani[..4].copy_from_slice(b"IDAG");
        put_i32(&mut ani, 4, 48);
        put_i32(&mut ani, 8, 1234);
        ani[12] = 255;
        let responses = [
            response(&requests[0], Some(ani)),
            response(&requests[1], None),
        ];
        let Load::Complete(document) = load(
            "models/animated.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &bytes,
            &responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("closed animated model requested more dependencies")
        };
        assert_eq!(document.sequences[0].animation_indices, [0]);
        assert_eq!(document.sequences[0].next_sequence, 0);
        assert_eq!(document.sequences[0].pose, 0);
        assert_eq!(
            document.sequences[0].bone_weights,
            [Float32(1.0_f32.to_bits())]
        );
        assert_eq!(document.animations[0].frames.len(), 2);
        assert_eq!(document.animations[0].sections[0].block, 1);
        assert_eq!(
            document
                .model_dependencies
                .iter()
                .map(|dependency| dependency.role)
                .collect::<Vec<_>>(),
            [
                ModelDependencyRole::RootModel,
                ModelDependencyRole::AnimationBlocks,
                ModelDependencyRole::Physics,
            ]
        );
        assert!(document.model_dependencies[0].sha256.is_some());
        assert!(document.model_dependencies[1].sha256.is_some());
        assert!(document.model_dependencies[2].sha256.is_none());

        let mut missing_external_identity = animated_mdl(true);
        let identity_offset = i32::from_le_bytes(
            missing_external_identity[348..352]
                .try_into()
                .expect("animation identity offset"),
        ) as usize;
        missing_external_identity[identity_offset] = 0;
        assert_eq!(
            load(
                "models/animated.mdl",
                Profile::SourcePcMdl48,
                VtxVariant::Dx90,
                &missing_external_identity,
                &[],
                Limits::default(),
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidReference
        );
    }

    #[test]
    fn closes_include_dependencies_depth_first_and_rejects_cycles() {
        let root = mdl_with_include("models/shared/a.mdl");
        let include = animated_mdl(false);
        let Load::Needs(root_requests) = load(
            "models/root.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("root include dependencies were not requested")
        };
        let mut responses = vec![
            response(&root_requests[0], None),
            response(&root_requests[1], Some(include.clone())),
        ];
        let Load::Needs(include_requests) = load(
            "models/root.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("included model dependencies were not requested")
        };
        assert_eq!(include_requests.len(), 1);
        assert_eq!(include_requests[0].requester, "models/shared/a.mdl");
        assert_eq!(
            include_requests[0].dependency_chain,
            ["models/root.mdl", "models/shared/a.mdl"]
        );
        responses.push(response(&include_requests[0], None));
        let Load::Complete(document) = load(
            "models/root.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("closed include graph requested more dependencies")
        };
        assert_eq!(
            document
                .model_dependencies
                .iter()
                .map(|dependency| dependency.role)
                .collect::<Vec<_>>(),
            [
                ModelDependencyRole::RootModel,
                ModelDependencyRole::Physics,
                ModelDependencyRole::IncludeModel,
                ModelDependencyRole::RootModel,
                ModelDependencyRole::Physics,
            ]
        );
        assert_eq!(document.animations.len(), 1);
        assert_eq!(document.animations[0].name, b"idle_anim");
        assert_eq!(document.animations[0].bone_map, [None]);
        assert_eq!(document.sequences.len(), 1);
        assert_eq!(document.sequences[0].label, b"idle");
        assert_eq!(
            load(
                "models/root.mdl",
                Profile::SourcePcMdl48,
                VtxVariant::Dx90,
                &root,
                &responses,
                Limits {
                    max_dependency_count: 2,
                    ..Limits::default()
                },
            )
            .unwrap_err()
            .code,
            ErrorCode::DependencyLimit
        );

        let cyclic_include = mdl_with_include("models/root.mdl");
        let mut cycle_responses = vec![
            response(&root_requests[0], None),
            response(&root_requests[1], Some(cyclic_include)),
        ];
        let Load::Needs(cycle_requests) = load(
            "models/root.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &cycle_responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("cyclic include children were not requested")
        };
        for request in &cycle_requests {
            cycle_responses.push(response(
                request,
                (request.role == DependencyRole::IncludeModel).then(|| root.clone()),
            ));
        }
        assert_eq!(
            load(
                "models/root.mdl",
                Profile::SourcePcMdl48,
                VtxVariant::Dx90,
                &root,
                &cycle_responses,
                Limits::default(),
            )
            .unwrap_err()
            .code,
            ErrorCode::IncludeCycle
        );
    }

    #[test]
    fn nested_include_with_terminated_material_directory_and_local_only_block_zero_is_valid() {
        let include_identity = "models/player/items/soldier/soldier_viking.mdl";
        let root = mdl_with_include(include_identity);
        let item = nested_item_mdl();
        let parsed = parse_mdl(
            include_identity,
            Profile::SourcePcMdl48,
            &item,
            Limits::default(),
        )
        .unwrap();
        assert!(!parsed.needs_animation);
        assert_eq!(parsed.document.animation_block_identity, None);
        assert_eq!(
            parsed.document.materials[0].candidates,
            [
                "materials/models/player/items/soldier/models/player/items/soldier/soldier_viking.vmt",
                "materials/models/player/items/soldier/soldier_viking.vmt",
            ]
        );

        let Load::Needs(root_requests) = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("root dependencies were not requested")
        };
        let root_responses: Vec<_> = root_requests
            .iter()
            .map(|request| {
                response(
                    request,
                    (request.role == DependencyRole::IncludeModel).then(|| item.clone()),
                )
            })
            .collect();
        let Load::Needs(item_requests) = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &root_responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("nested item dependencies were not requested")
        };
        assert_eq!(
            item_requests
                .iter()
                .map(|request| (request.role, request.logical_path.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    DependencyRole::VertexData,
                    "models/player/items/soldier/soldier_viking.vvd",
                ),
                (
                    DependencyRole::Topology,
                    "models/player/items/soldier/soldier_viking.dx90.vtx",
                ),
                (
                    DependencyRole::Physics,
                    "models/player/items/soldier/soldier_viking.phy",
                ),
            ]
        );
        assert!(item_requests.iter().all(|request| {
            request.dependency_chain == ["models/player/soldier.mdl", include_identity]
        }));
        let mut responses = root_responses;
        responses.extend(item_requests.iter().map(|request| {
            response(
                request,
                match request.role {
                    DependencyRole::VertexData => Some(vvd(1234)),
                    DependencyRole::Topology => Some(vtx(1234)),
                    DependencyRole::Physics => None,
                    _ => unreachable!(),
                },
            )
        }));
        let Load::Complete(document) = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &root,
            &responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("supplied nested item dependencies did not complete")
        };
        let PresentationBuild::Complete(artifact) = build_presentation(
            &document,
            PresentationProfile::World,
            &[],
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("composed nested include did not produce an artifact")
        };
        assert!(artifact.model.dependencies.iter().any(|dependency| {
            dependency.role == PresentationDependencyRole::IncludeModel
                && dependency.logical_path == include_identity
        }));
        assert_eq!(
            decode_presentation(&artifact.bytes, PresentationLimits::default()).unwrap(),
            *artifact
        );

        for rejected in [
            b"\\\\server\\share".as_slice(),
            b"models//player".as_slice(),
            b"models/../player".as_slice(),
        ] {
            assert_eq!(
                material_candidate(rejected, b"item", include_identity)
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidIdentity
            );
        }
        for rejected in [
            b"player/items/item.mdl".as_slice(),
            b"/models/player/items/item.mdl".as_slice(),
            b"models/player/../items/item.mdl".as_slice(),
            b"https://example.invalid/item.mdl".as_slice(),
        ] {
            assert_eq!(
                canonical_model_path(rejected, "models/player/root.mdl")
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidIdentity
            );
        }
        assert_eq!(
            validate_model_identity("models/player/Item.mdl")
                .unwrap_err()
                .code,
            ErrorCode::InvalidIdentity
        );
        assert_eq!(
            canonical_model_path(
                b"Models\\Player\\Items\\Soldier\\Soldier_Viking.MDL",
                "models/player/soldier.mdl",
            )
            .unwrap(),
            include_identity
        );
    }

    #[test]
    fn empty_override_sequence_is_a_valid_forward_declaration() {
        let (bytes, sequence_offset) = mdl_with_empty_override_sequence();
        let Load::Needs(requests) = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &bytes,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("forward-declaration model did not request optional physics")
        };
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].role, DependencyRole::Physics);
        let Load::Complete(document) = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &bytes,
            &[response(&requests[0], None)],
            Limits::default(),
        )
        .unwrap() else {
            panic!("forward-declaration model requested more dependencies")
        };
        assert_eq!(document.sequences.len(), 1);
        assert_eq!(document.sequences[0].label, b"user_ref");
        assert_eq!(document.sequences[0].flags, STUDIO_OVERRIDE);
        assert_eq!(document.sequences[0].blend_count, 0);
        assert_eq!(document.sequences[0].blend_size, [0, 0]);
        assert!(document.sequences[0].animation_indices.is_empty());
        assert!(document.sequences[0].bone_weights.is_empty());
        assert_eq!(sequence_offset + 56, 464);

        let PresentationBuild::Complete(artifact) = build_presentation(
            &document,
            PresentationProfile::World,
            &[],
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("forward declaration did not survive the artifact path")
        };
        assert_eq!(
            decode_presentation(&artifact.bytes, PresentationLimits::default()).unwrap(),
            *artifact
        );

        let mut ordinary_empty = bytes.clone();
        put_i32(&mut ordinary_empty, sequence_offset + 12, 0);
        let ordinary_error = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &ordinary_empty,
            &[],
            Limits::default(),
        )
        .unwrap_err();
        assert_eq!(ordinary_error.code, ErrorCode::InvalidCount);
        assert_eq!(ordinary_error.range, Some(464..468));

        let mut nonempty_forward = bytes;
        put_i32(&mut nonempty_forward, sequence_offset + 24, 1);
        let child_error = load(
            "models/player/soldier.mdl",
            Profile::SourcePcMdl48,
            VtxVariant::Dx90,
            &nonempty_forward,
            &[],
            Limits::default(),
        )
        .unwrap_err();
        assert_eq!(child_error.code, ErrorCode::InvalidCount);
        assert_eq!(
            child_error.range,
            Some(sequence_offset + 24..sequence_offset + 28)
        );
    }

    #[test]
    fn retains_lod_material_replacements_and_exact_candidates() {
        let bytes = mdl_with_material();
        let Load::Needs(requests) = load(
            "models/material.mdl",
            Profile::SourcePcMdl44,
            VtxVariant::Dx90,
            &bytes,
            &[],
            Limits::default(),
        )
        .unwrap() else {
            panic!("material model dependencies were not requested")
        };
        let responses: Vec<_> = requests
            .iter()
            .map(|request| {
                response(
                    request,
                    match request.role {
                        DependencyRole::VertexData => Some(vvd(1234)),
                        DependencyRole::Topology => Some(vtx_with_replacement(1234)),
                        DependencyRole::Physics => None,
                        _ => unreachable!(),
                    },
                )
            })
            .collect();
        let Load::Complete(document) = load(
            "models/material.mdl",
            Profile::SourcePcMdl44,
            VtxVariant::Dx90,
            &bytes,
            &responses,
            Limits::default(),
        )
        .unwrap() else {
            panic!("closed material model requested more dependencies")
        };
        assert_eq!(document.material_replacements.len(), 1);
        assert_eq!(document.material_replacements[0].lod, 0);
        assert_eq!(document.material_replacements[0].material_slot, 0);
        assert_eq!(document.material_replacements[0].name, b"lod_material");
        assert_eq!(
            document.material_replacements[0].candidates,
            ["materials/models/test/lod_material.vmt"]
        );
    }

    #[test]
    fn derives_source_list_and_strip_winding_without_rewriting_indices() {
        assert_eq!(
            derived_strip_triangles(&[0, 1, 2, 3, 4, 5], 1).unwrap(),
            [[0, 1, 2], [3, 4, 5]]
        );
        assert_eq!(
            derived_strip_triangles(&[0, 1, 2, 3, 4], 2).unwrap(),
            [[0, 2, 1], [1, 2, 3], [2, 4, 3]]
        );
        assert_eq!(
            derived_strip_triangles(&[0, 1, 1, 2, 3], 2).unwrap(),
            [[1, 3, 2]]
        );
        assert!(derived_strip_triangles(&[0, 1], 1).is_none());
        assert!(derived_strip_triangles(&[0, 1, 2], 3).is_none());
    }
}
