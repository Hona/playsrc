//! Runtime-neutral StudioModel presentation artifacts and pose sampling.

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    Animation, Attachment, BodyPart, Document, Float32, GeometryPrimitive, SkinFamily, Vector3,
};

const ARTIFACT_MAGIC: &[u8; 4] = b"PSMP";
const ARTIFACT_VERSION: u16 = 4;
const COMPACT_FRAME_MARKER: u32 = u32::MAX;
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const STUDIO_LOOPING: i32 = 0x0001;
const STUDIO_DELTA: i32 = 0x0004;
const STUDIO_CYCLE_POSE: i32 = 0x0080;
const STUDIO_REALTIME: i32 = 0x0100;
const STUDIO_OVERRIDE: i32 = 0x0800;
const STUDIO_WORLD: i32 = 0x4000;
const STUDIO_POST: i32 = 0x0010;
const STUDIO_LOCAL: i32 = 0x0200;
const STUDIO_AUTO_LAYER_SPLINE: i32 = 0x0040;
const STUDIO_AUTO_LAYER_CROSSFADE: i32 = 0x0080;
const STUDIO_AUTO_LAYER_NO_BLEND: i32 = 0x0200;
const STUDIO_AUTO_LAYER_LOCAL: i32 = 0x1000;
const STUDIO_AUTO_LAYER_POSE: i32 = 0x4000;
const STUDIO_HEADER_STATIC_PROP: i32 = 0x0010;
const BONE_FIXED_ALIGNMENT: i32 = 0x0010_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationProfile {
    World,
    ViewModel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationLimits {
    pub max_dependencies: usize,
    pub max_dependency_bytes: usize,
    pub max_aggregate_dependency_bytes: usize,
    pub max_materials: usize,
    pub max_textures_per_material: usize,
    pub max_bones: usize,
    pub max_animations: usize,
    pub max_sequences: usize,
    pub max_animation_samples: usize,
    pub max_geometry_vertices: usize,
    pub max_geometry_indices: usize,
    pub max_string_bytes: usize,
    pub max_owned_bytes: usize,
    pub max_artifact_bytes: usize,
}

impl Default for PresentationLimits {
    fn default() -> Self {
        Self {
            max_dependencies: 4_096,
            max_dependency_bytes: 256 * 1024 * 1024,
            max_aggregate_dependency_bytes: 1024 * 1024 * 1024,
            max_materials: 512,
            max_textures_per_material: 32,
            max_bones: 128,
            max_animations: 65_535,
            max_sequences: 65_535,
            max_animation_samples: 64_000_000,
            max_geometry_vertices: 4_000_000,
            max_geometry_indices: 12_000_000,
            max_string_bytes: 4_096,
            max_owned_bytes: 63 * 1024 * 1024,
            max_artifact_bytes: 63 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output
            .debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TextureRole {
    Base,
    HdrBase,
    HdrCompressed,
    HdrCompressed0,
    HdrCompressed1,
    HdrCompressed2,
    Base2,
    Bump,
    Normal,
    Bump2,
    Detail,
    BlendModulate,
    Environment,
    EnvironmentMask,
    SelfIllumMask,
    Flow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureDisposition {
    Source,
    BuiltInEnvironment,
    BuiltInRenderTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialSourceManifest {
    pub requester: String,
    pub logical_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialTextureManifest {
    pub role: TextureRole,
    pub parameter: Vec<u8>,
    pub logical_path: Option<String>,
    pub disposition: TextureDisposition,
    pub selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialResolutionManifest {
    pub root_identity: String,
    pub include_sources: Vec<MaterialSourceManifest>,
    pub textures: Vec<MaterialTextureManifest>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PresentationDependencyRole {
    RootModel,
    VertexData,
    Topology,
    AnimationBlocks,
    IncludeModel,
    Physics,
    MaterialCandidate,
    MaterialInclude,
    Texture,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationDependencyRequest {
    pub requester: String,
    pub role: PresentationDependencyRole,
    pub logical_path: String,
    pub material_slot: usize,
    pub texture_role: Option<TextureRole>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationDependencyResponse {
    pub requester: String,
    pub role: PresentationDependencyRole,
    pub logical_path: String,
    pub material_slot: usize,
    pub texture_role: Option<TextureRole>,
    pub bytes: Option<Vec<u8>>,
    pub verified_byte_length: Option<usize>,
    pub sha256: Option<[u8; 32]>,
    pub material: Option<MaterialResolutionManifest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactDependency {
    pub requester: String,
    pub role: PresentationDependencyRole,
    pub logical_path: String,
    pub material_slot: Option<usize>,
    pub texture_role: Option<TextureRole>,
    pub sha256: Option<[u8; 32]>,
    pub byte_length: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationTexture {
    pub role: TextureRole,
    pub parameter: Vec<u8>,
    pub dependency: Option<usize>,
    pub disposition: TextureDisposition,
    pub selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationMaterial {
    pub slot: usize,
    pub source_slot: usize,
    pub lod: Option<usize>,
    pub authored_name: Vec<u8>,
    pub material_dependency: usize,
    pub include_dependencies: Vec<usize>,
    pub textures: Vec<PresentationTexture>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelBasis {
    pub forward: Vector3,
    pub left: Vector3,
    pub up: Vector3,
}

impl ModelBasis {
    fn source() -> Self {
        Self {
            forward: vector([1.0, 0.0, 0.0]),
            left: vector([0.0, 1.0, 0.0]),
            up: vector([0.0, 0.0, 1.0]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VertexAttributeTransform {
    AuthoredSourceValues,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureCoordinateConvention {
    AuthoredUTowardRightVDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TangentHandednessConvention {
    TangentSWComponent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VertexDeformationContract {
    FlexBeforeLinearBoneSkinningWithoutTopologyChanges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriangleWinding {
    Clockwise,
    CounterClockwise,
}

impl TriangleWinding {
    fn reversed(self) -> Self {
        match self {
            Self::Clockwise => Self::CounterClockwise,
            Self::CounterClockwise => Self::Clockwise,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CullFace {
    Back,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeometryFacing {
    pub front_face: TriangleWinding,
    pub cull_face: CullFace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformOrientation {
    Preserving,
    Reversing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityAngleConvention {
    DegreesPitchYawRollForwardLeftUpColumns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootBoneContract {
    AnimatedBelowEntity,
    StaticPropBoneZeroIsEntity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeometryOrientation {
    pub positions: VertexAttributeTransform,
    pub normals: VertexAttributeTransform,
    pub tangents: VertexAttributeTransform,
    pub texture_coordinates: TextureCoordinateConvention,
    pub tangent_handedness: TangentHandednessConvention,
    pub deformation: VertexDeformationContract,
    pub facing: GeometryFacing,
}

impl GeometryOrientation {
    fn source() -> Self {
        Self {
            positions: VertexAttributeTransform::AuthoredSourceValues,
            normals: VertexAttributeTransform::AuthoredSourceValues,
            tangents: VertexAttributeTransform::AuthoredSourceValues,
            texture_coordinates: TextureCoordinateConvention::AuthoredUTowardRightVDown,
            tangent_handedness: TangentHandednessConvention::TangentSWComponent,
            deformation:
                VertexDeformationContract::FlexBeforeLinearBoneSkinningWithoutTopologyChanges,
            facing: GeometryFacing {
                front_face: TriangleWinding::Clockwise,
                cull_face: CullFace::Back,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FarPlaneContract {
    SuppliedWorldFarPlane,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewmodelHandednessContract {
    OptionalViewSpaceYReflection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationDescriptor {
    World {
        geometry: GeometryOrientation,
        entity_angles: EntityAngleConvention,
        root_bone: RootBoneContract,
        depth_range: [Float32; 2],
    },
    ViewModel {
        geometry: GeometryOrientation,
        entity_angles: EntityAngleConvention,
        default_horizontal_fov_4_by_3: Float32,
        minimum_fov: Float32,
        maximum_fov: Float32,
        near_plane: Float32,
        far_plane: FarPlaneContract,
        depth_range: [Float32; 2],
        draws_after_world: bool,
        opaque_before_translucent: bool,
        handedness: ViewmodelHandednessContract,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureFamily {
    ProceduralAxisInterpolation,
    ProceduralQuaternionInterpolation,
    ProceduralJiggle,
    ProceduralAimAtBone,
    ProceduralAimAtAttachment,
    InverseKinematics,
    Flex,
    SequenceAutoLayers,
    UnknownProcedural,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureDisposition {
    NotPresent,
    RetainedNotEvaluated,
    Evaluated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureSupport {
    pub family: FeatureFamily,
    pub disposition: FeatureDisposition,
    pub records: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationBodyPart {
    pub index: usize,
    pub name: Vec<u8>,
    pub base: i32,
    pub model_names: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationModel {
    pub profile: PresentationProfile,
    pub descriptor: PresentationDescriptor,
    pub identity: String,
    pub checksum: i32,
    pub flags: i32,
    pub basis: ModelBasis,
    pub collision_bounds: [Vector3; 2],
    pub dependencies: Vec<ArtifactDependency>,
    pub base_material_count: usize,
    pub materials: Vec<PresentationMaterial>,
    pub bones: Vec<crate::Bone>,
    pub animations: Vec<Animation>,
    pub sequences: Vec<crate::Sequence>,
    pub pose_parameters: Vec<crate::PoseParameter>,
    pub attachments: Vec<Attachment>,
    pub hitbox_sets: Vec<crate::HitboxSet>,
    pub skins: Vec<SkinFamily>,
    pub body_parts: Vec<PresentationBodyPart>,
    pub geometry: Vec<GeometryPrimitive>,
    pub physics_status: crate::PhysicsStatus,
    pub features: Vec<FeatureSupport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationArtifact {
    pub model: PresentationModel,
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationBuild {
    Needs(Vec<PresentationDependencyRequest>),
    Complete(Box<PresentationArtifact>),
}

pub enum PresentationModelBuild {
    Needs(Vec<PresentationDependencyRequest>),
    Complete(Box<PresentationModel>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationErrorCode {
    InvalidLimits,
    Cancelled,
    DependencyLimit,
    MissingMaterial,
    MissingDependency,
    InvalidManifest,
    InvalidIdentity,
    HashMismatch,
    ModelLimit,
    ArtifactLimit,
    InvalidArtifact,
    InvalidState,
    UnsupportedState,
    InvalidSelection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationError {
    pub code: PresentationErrorCode,
    pub identity: String,
}

impl std::fmt::Display for PresentationError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{:?} in {}", self.code, self.identity)
    }
}

impl std::error::Error for PresentationError {}

struct MaterialTarget<'a> {
    source_slot: usize,
    lod: Option<usize>,
    name: &'a [u8],
    candidates: &'a [String],
}

pub fn build_presentation(
    document: &Document,
    profile: PresentationProfile,
    responses: &[PresentationDependencyResponse],
    limits: PresentationLimits,
    cancellation: &CancellationToken,
) -> Result<PresentationBuild, PresentationError> {
    match build_presentation_model(document, profile, responses, limits, cancellation)? {
        PresentationModelBuild::Needs(requests) => Ok(PresentationBuild::Needs(requests)),
        PresentationModelBuild::Complete(model) => {
            let bytes = encode_model(&model, limits)?;
            if bytes.len() >= MAX_MESSAGE_BYTES || bytes.len() > limits.max_artifact_bytes {
                return Err(presentation_error(
                    PresentationErrorCode::ArtifactLimit,
                    &document.identity,
                ));
            }
            let sha256 = sha256(&bytes);
            Ok(PresentationBuild::Complete(Box::new(
                PresentationArtifact {
                    model: *model,
                    bytes,
                    sha256,
                },
            )))
        }
    }
}

pub fn build_presentation_model(
    document: &Document,
    profile: PresentationProfile,
    responses: &[PresentationDependencyResponse],
    limits: PresentationLimits,
    cancellation: &CancellationToken,
) -> Result<PresentationModelBuild, PresentationError> {
    validate_limits(limits)?;
    check_cancelled(cancellation, &document.identity)?;
    validate_model_limits(document, limits)?;

    let material_targets: Vec<_> = document
        .materials
        .iter()
        .map(|material| MaterialTarget {
            source_slot: material.index,
            lod: None,
            name: &material.name,
            candidates: &material.candidates,
        })
        .chain(
            document
                .material_replacements
                .iter()
                .map(|replacement| MaterialTarget {
                    source_slot: replacement.material_slot,
                    lod: Some(replacement.lod),
                    name: &replacement.name,
                    candidates: &replacement.candidates,
                }),
        )
        .collect();
    if material_targets.len() > limits.max_materials {
        return Err(presentation_error(
            PresentationErrorCode::ModelLimit,
            &document.identity,
        ));
    }

    let mut candidate_needs = Vec::new();
    let mut selected = Vec::with_capacity(material_targets.len());
    for (target_index, material) in material_targets.iter().enumerate() {
        check_cancelled(cancellation, &document.identity)?;
        let mut selected_response = None;
        for candidate in material.candidates {
            let request = PresentationDependencyRequest {
                requester: document.identity.clone(),
                role: PresentationDependencyRole::MaterialCandidate,
                logical_path: candidate.clone(),
                material_slot: target_index,
                texture_role: None,
            };
            match responses
                .iter()
                .find(|response| response_matches(response, &request))
            {
                Some(response)
                    if response.bytes.is_some() || response.verified_byte_length.is_some() =>
                {
                    validate_response(response, limits)?;
                    selected_response = Some(response);
                    break;
                }
                Some(_) => {}
                None => candidate_needs.push(request),
            }
        }
        if let Some(response) = selected_response {
            selected.push(response);
        } else if candidate_needs
            .iter()
            .any(|request| request.material_slot == target_index)
        {
            continue;
        } else {
            return Err(presentation_error(
                PresentationErrorCode::MissingMaterial,
                material
                    .candidates
                    .last()
                    .map_or(document.identity.as_str(), String::as_str),
            ));
        }
    }
    if !candidate_needs.is_empty() {
        bound_requests(&candidate_needs, limits, &document.identity)?;
        return Ok(PresentationModelBuild::Needs(candidate_needs));
    }

    let mut closure_needs = Vec::new();
    let mut manifests = Vec::with_capacity(selected.len());
    for response in &selected {
        let manifest = response.material.as_ref().ok_or_else(|| {
            presentation_error(
                PresentationErrorCode::InvalidManifest,
                &response.logical_path,
            )
        })?;
        validate_manifest(manifest, response, limits)?;
        for source in &manifest.include_sources {
            let request = PresentationDependencyRequest {
                requester: source.requester.clone(),
                role: PresentationDependencyRole::MaterialInclude,
                logical_path: source.logical_path.clone(),
                material_slot: response.material_slot,
                texture_role: None,
            };
            if !responses
                .iter()
                .any(|response| response_matches(response, &request))
            {
                closure_needs.push(request);
            }
        }
        for texture in &manifest.textures {
            if texture.disposition != TextureDisposition::Source {
                continue;
            }
            let logical_path = texture.logical_path.clone().ok_or_else(|| {
                presentation_error(
                    PresentationErrorCode::InvalidManifest,
                    &manifest.root_identity,
                )
            })?;
            let request = PresentationDependencyRequest {
                requester: manifest.root_identity.clone(),
                role: PresentationDependencyRole::Texture,
                logical_path,
                material_slot: response.material_slot,
                texture_role: Some(texture.role),
            };
            if !responses
                .iter()
                .any(|response| response_matches(response, &request))
            {
                closure_needs.push(request);
            }
        }
        manifests.push(manifest);
    }
    if !closure_needs.is_empty() {
        bound_requests(&closure_needs, limits, &document.identity)?;
        return Ok(PresentationModelBuild::Needs(closure_needs));
    }

    let mut dependencies: Vec<_> = document
        .model_dependencies
        .iter()
        .map(|dependency| ArtifactDependency {
            requester: dependency.requester.clone(),
            role: match dependency.role {
                crate::ModelDependencyRole::RootModel => PresentationDependencyRole::RootModel,
                crate::ModelDependencyRole::VertexData => PresentationDependencyRole::VertexData,
                crate::ModelDependencyRole::Topology => PresentationDependencyRole::Topology,
                crate::ModelDependencyRole::AnimationBlocks => {
                    PresentationDependencyRole::AnimationBlocks
                }
                crate::ModelDependencyRole::IncludeModel => {
                    PresentationDependencyRole::IncludeModel
                }
                crate::ModelDependencyRole::Physics => PresentationDependencyRole::Physics,
            },
            logical_path: dependency.logical_path.clone(),
            material_slot: None,
            texture_role: None,
            sha256: dependency.sha256,
            byte_length: dependency.byte_length,
        })
        .collect();
    let mut materials = Vec::with_capacity(selected.len());
    for (root, manifest) in selected.iter().zip(manifests) {
        check_cancelled(cancellation, &document.identity)?;
        let material_dependency = push_dependency(&mut dependencies, root)?;
        let mut include_dependencies = Vec::with_capacity(manifest.include_sources.len());
        for source in &manifest.include_sources {
            let request = PresentationDependencyRequest {
                requester: source.requester.clone(),
                role: PresentationDependencyRole::MaterialInclude,
                logical_path: source.logical_path.clone(),
                material_slot: root.material_slot,
                texture_role: None,
            };
            let response = responses
                .iter()
                .find(|response| response_matches(response, &request))
                .expect("closure requests checked");
            if response.bytes.is_none() && response.verified_byte_length.is_none() {
                return Err(presentation_error(
                    PresentationErrorCode::MissingDependency,
                    &response.logical_path,
                ));
            }
            validate_response(response, limits)?;
            include_dependencies.push(push_dependency(&mut dependencies, response)?);
        }
        let mut textures = Vec::with_capacity(manifest.textures.len());
        for texture in &manifest.textures {
            let dependency = if texture.disposition == TextureDisposition::Source {
                let request = PresentationDependencyRequest {
                    requester: manifest.root_identity.clone(),
                    role: PresentationDependencyRole::Texture,
                    logical_path: texture.logical_path.clone().expect("manifest validated"),
                    material_slot: root.material_slot,
                    texture_role: Some(texture.role),
                };
                let response = responses
                    .iter()
                    .find(|response| response_matches(response, &request))
                    .expect("closure requests checked");
                if response.bytes.is_none() && response.verified_byte_length.is_none() {
                    return Err(presentation_error(
                        PresentationErrorCode::MissingDependency,
                        &response.logical_path,
                    ));
                }
                validate_response(response, limits)?;
                Some(push_dependency(&mut dependencies, response)?)
            } else {
                None
            };
            textures.push(PresentationTexture {
                role: texture.role,
                parameter: texture.parameter.clone(),
                dependency,
                disposition: texture.disposition,
                selected: texture.selected,
            });
        }
        let source_material = material_targets.get(root.material_slot).ok_or_else(|| {
            presentation_error(PresentationErrorCode::InvalidManifest, &root.logical_path)
        })?;
        materials.push(PresentationMaterial {
            slot: root.material_slot,
            source_slot: source_material.source_slot,
            lod: source_material.lod,
            authored_name: source_material.name.to_vec(),
            material_dependency,
            include_dependencies,
            textures,
        });
    }
    if dependencies.len() > limits.max_dependencies {
        return Err(presentation_error(
            PresentationErrorCode::DependencyLimit,
            &document.identity,
        ));
    }
    let aggregate_dependency_bytes = unique_dependency_bytes(&dependencies);
    if aggregate_dependency_bytes.is_none_or(|bytes| bytes > limits.max_aggregate_dependency_bytes)
    {
        return Err(presentation_error(
            PresentationErrorCode::DependencyLimit,
            &document.identity,
        ));
    }
    let expanded_owned = estimated_owned_bytes(document, &dependencies, &materials, false);
    let authored = document
        .animations
        .iter()
        .any(|animation| animation.authored_frames.is_some());
    let compact = !authored && expanded_owned.is_none_or(|bytes| bytes > limits.max_owned_bytes);
    let retained_owned = if compact {
        estimated_owned_bytes(document, &dependencies, &materials, true)
    } else {
        expanded_owned
    };
    if retained_owned.is_none_or(|bytes| bytes > limits.max_owned_bytes) {
        return Err(presentation_error(
            PresentationErrorCode::ModelLimit,
            &document.identity,
        ));
    }

    let animations = if compact {
        document
            .animations
            .iter()
            .map(|animation| compact_animation(animation, &document.identity))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        document.animations.clone()
    };
    let model = PresentationModel {
        profile,
        descriptor: presentation_descriptor(profile, document.flags),
        identity: document.identity.clone(),
        checksum: document.checksum,
        flags: document.flags,
        basis: ModelBasis::source(),
        collision_bounds: [document.bounds.hull_min, document.bounds.hull_max],
        dependencies,
        base_material_count: document.materials.len(),
        materials,
        bones: document.bones.clone(),
        animations,
        sequences: document.sequences.clone(),
        pose_parameters: document.pose_parameters.clone(),
        attachments: document.attachments.clone(),
        hitbox_sets: document.hitbox_sets.clone(),
        skins: document.skins.clone(),
        body_parts: presentation_body_parts(&document.body_parts),
        geometry: document.geometry.clone(),
        physics_status: document.physics_status,
        features: feature_support(document),
    };
    check_cancelled(cancellation, &document.identity)?;
    validate_decoded_model(&model, limits)?;
    Ok(PresentationModelBuild::Complete(Box::new(model)))
}

fn validate_limits(limits: PresentationLimits) -> Result<(), PresentationError> {
    if limits.max_dependencies == 0
        || limits.max_dependency_bytes == 0
        || limits.max_aggregate_dependency_bytes == 0
        || limits.max_materials == 0
        || limits.max_textures_per_material == 0
        || limits.max_bones == 0
        || limits.max_animations == 0
        || limits.max_sequences == 0
        || limits.max_animation_samples == 0
        || limits.max_geometry_vertices == 0
        || limits.max_geometry_indices == 0
        || limits.max_string_bytes == 0
        || limits.max_owned_bytes == 0
        || limits.max_artifact_bytes == 0
        || limits.max_artifact_bytes >= MAX_MESSAGE_BYTES
        || limits.max_owned_bytes >= MAX_MESSAGE_BYTES
    {
        return Err(presentation_error(
            PresentationErrorCode::InvalidLimits,
            "presentation-limits",
        ));
    }
    Ok(())
}

fn validate_model_limits(
    document: &Document,
    limits: PresentationLimits,
) -> Result<(), PresentationError> {
    let samples = document
        .animations
        .iter()
        .try_fold(0_usize, |total, animation| {
            total.checked_add(
                usize::try_from(animation.frame_count)
                    .ok()?
                    .checked_mul(animation.bone_map.len())?,
            )
        });
    let vertices = document
        .geometry
        .iter()
        .try_fold(0_usize, |total, primitive| {
            total.checked_add(primitive.vertices.len())
        });
    let indices = document
        .geometry
        .iter()
        .try_fold(0_usize, |total, primitive| {
            total.checked_add(primitive.encoded_indices.len())
        });
    if document
        .materials
        .len()
        .checked_add(document.material_replacements.len())
        .is_none_or(|count| count > limits.max_materials)
        || document.bones.len() > limits.max_bones
        || document.animations.len() > limits.max_animations
        || document.sequences.len() > limits.max_sequences
        || samples.is_none_or(|value| value > limits.max_animation_samples)
        || vertices.is_none_or(|value| value > limits.max_geometry_vertices)
        || indices.is_none_or(|value| value > limits.max_geometry_indices)
    {
        return Err(presentation_error(
            PresentationErrorCode::ModelLimit,
            &document.identity,
        ));
    }
    Ok(())
}

fn estimated_owned_bytes(
    document: &Document,
    dependencies: &[ArtifactDependency],
    materials: &[PresentationMaterial],
    compact: bool,
) -> Option<usize> {
    let mut total = std::mem::size_of::<PresentationModel>();
    let mut add = |bytes: usize| {
        total = total.checked_add(bytes)?;
        Some(())
    };
    add(document.identity.len())?;
    add(document
        .bones
        .len()
        .checked_mul(std::mem::size_of::<crate::Bone>())?)?;
    for bone in &document.bones {
        add(bone.name.len().checked_add(bone.surface_property.len())?)?;
    }
    add(document
        .animations
        .len()
        .checked_mul(std::mem::size_of::<Animation>())?)?;
    let mut authored_contexts = BTreeSet::new();
    for animation in &document.animations {
        add(animation
            .name
            .len()
            .checked_add(animation.source_identity.len())?)?;
        add(animation
            .bone_map
            .len()
            .checked_mul(std::mem::size_of::<Option<usize>>())?)?;
        add(animation
            .sections
            .len()
            .checked_mul(std::mem::size_of::<crate::AnimationSection>())?)?;
        for section in &animation.sections {
            add(section
                .tracks
                .len()
                .checked_mul(std::mem::size_of::<crate::AnimationTrack>())?)?;
            for track in &section.tracks {
                for stream in track
                    .rotation_values
                    .iter()
                    .chain(&track.translation_values)
                    .flatten()
                {
                    add(stream
                        .runs
                        .len()
                        .checked_mul(std::mem::size_of::<crate::AnimationValueRun>())?)?;
                    for run in &stream.runs {
                        add(run.values.len().checked_mul(std::mem::size_of::<i16>())?)?;
                    }
                }
            }
        }
        if let Some(authored) = &animation.authored_frames {
            add(std::mem::size_of::<crate::AuthoredAnimationFrames>())?;
            add(authored
                .source
                .sections
                .len()
                .checked_mul(std::mem::size_of::<(i32, i32)>())?)?;
            if authored_contexts.insert(Arc::as_ptr(&authored.context) as usize) {
                add(std::mem::size_of::<crate::AuthoredAnimationContext>())?;
                add(authored.context.mdl.len())?;
                add(authored.context.ani.as_ref().map_or(0, |bytes| bytes.len()))?;
                add(authored
                    .context
                    .blocks
                    .len()
                    .checked_mul(std::mem::size_of::<std::ops::Range<usize>>())?)?;
                add(authored
                    .context
                    .bones
                    .len()
                    .checked_mul(std::mem::size_of::<crate::Bone>())?)?;
                for bone in &*authored.context.bones {
                    add(bone.name.len().checked_add(bone.surface_property.len())?)?;
                }
            }
        } else if compact {
            add(compact_frame_bytes_len(animation)?)?;
        } else {
            add(animation
                .frames
                .len()
                .checked_mul(std::mem::size_of::<crate::AnimationFrame>())?)?;
            for frame in &animation.frames {
                add(frame
                    .translations
                    .len()
                    .checked_mul(std::mem::size_of::<Vector3>())?)?;
                add(frame
                    .rotations
                    .len()
                    .checked_mul(std::mem::size_of::<[Float32; 4]>())?)?;
            }
        }
    }
    add(document
        .sequences
        .len()
        .checked_mul(std::mem::size_of::<crate::Sequence>())?)?;
    for sequence in &document.sequences {
        add(sequence
            .label
            .len()
            .checked_add(sequence.activity_name.len())?
            .checked_add(sequence.source_identity.len())?)?;
        add(sequence
            .animation_indices
            .len()
            .checked_mul(std::mem::size_of::<i16>())?)?;
        add(sequence
            .bone_weights
            .len()
            .checked_mul(std::mem::size_of::<Float32>())?)?;
        for keys in &sequence.pose_keys {
            add(keys.len().checked_mul(std::mem::size_of::<Float32>())?)?;
        }
        add(sequence
            .events
            .len()
            .checked_mul(std::mem::size_of::<crate::SequenceEvent>())?)?;
        for event in &sequence.events {
            add(event.name.len())?;
        }
        add(sequence
            .auto_layers
            .len()
            .checked_mul(std::mem::size_of::<crate::SequenceAutoLayer>())?)?;
    }
    add(document
        .pose_parameters
        .len()
        .checked_mul(std::mem::size_of::<crate::PoseParameter>())?)?;
    for pose in &document.pose_parameters {
        add(pose.name.len().checked_add(pose.source_identity.len())?)?;
    }
    add(document
        .attachments
        .len()
        .checked_mul(std::mem::size_of::<Attachment>())?)?;
    for attachment in &document.attachments {
        add(attachment.name.len())?;
    }
    add(document
        .hitbox_sets
        .len()
        .checked_mul(std::mem::size_of::<crate::HitboxSet>())?)?;
    for set in &document.hitbox_sets {
        add(set.name.len())?;
        add(set
            .hitboxes
            .len()
            .checked_mul(std::mem::size_of::<crate::Hitbox>())?)?;
        for hitbox in &set.hitboxes {
            add(hitbox.name.len())?;
        }
    }
    add(document
        .skins
        .len()
        .checked_mul(std::mem::size_of::<SkinFamily>())?)?;
    for skin in &document.skins {
        add(skin
            .texture_indices
            .len()
            .checked_mul(std::mem::size_of::<u16>())?)?;
    }
    add(document
        .body_parts
        .len()
        .checked_mul(std::mem::size_of::<PresentationBodyPart>())?)?;
    for part in &document.body_parts {
        add(part.name.len())?;
        for model in &part.models {
            add(model.name.len())?;
        }
    }
    add(document
        .geometry
        .len()
        .checked_mul(std::mem::size_of::<GeometryPrimitive>())?)?;
    for primitive in &document.geometry {
        add(primitive
            .source_vertex_ids
            .len()
            .checked_mul(std::mem::size_of::<usize>())?)?;
        add(primitive
            .vertices
            .len()
            .checked_mul(std::mem::size_of::<crate::Vertex>())?)?;
        add(primitive
            .encoded_indices
            .len()
            .checked_mul(std::mem::size_of::<u16>())?)?;
        add(primitive
            .strips
            .len()
            .checked_mul(std::mem::size_of::<crate::Strip>())?)?;
        add(primitive
            .triangles
            .len()
            .checked_mul(std::mem::size_of::<[u32; 3]>())?)?;
    }
    add(dependencies
        .len()
        .checked_mul(std::mem::size_of::<ArtifactDependency>())?)?;
    for dependency in dependencies {
        add(dependency
            .requester
            .len()
            .checked_add(dependency.logical_path.len())?)?;
    }
    add(materials
        .len()
        .checked_mul(std::mem::size_of::<PresentationMaterial>())?)?;
    for material in materials {
        add(material.authored_name.len())?;
        add(material
            .include_dependencies
            .len()
            .checked_mul(std::mem::size_of::<usize>())?)?;
        add(material
            .textures
            .len()
            .checked_mul(std::mem::size_of::<PresentationTexture>())?)?;
        for texture in &material.textures {
            add(texture.parameter.len())?;
        }
    }
    Some(total)
}

fn unique_dependency_bytes(dependencies: &[ArtifactDependency]) -> Option<usize> {
    let mut identities = BTreeSet::new();
    let mut total = 0_usize;
    for dependency in dependencies {
        if identities.insert((
            dependency.logical_path.as_str(),
            dependency.sha256,
            dependency.byte_length,
        )) {
            total = total.checked_add(dependency.byte_length)?;
        }
    }
    Some(total)
}

fn compact_frame_bytes_len(animation: &Animation) -> Option<usize> {
    if animation.frames.len() != animation.frame_count as usize || animation.frames.is_empty() {
        return None;
    }
    let bone_count = animation.bone_map.len();
    if animation
        .frames
        .iter()
        .any(|frame| frame.translations.len() != bone_count || frame.rotations.len() != bone_count)
    {
        return None;
    }
    let mut length = 8_usize;
    for bone in 0..bone_count {
        let translation = animation.frames[0].translations[bone];
        let translation_count = if animation
            .frames
            .iter()
            .all(|frame| frame.translations[bone] == translation)
        {
            1
        } else {
            animation.frames.len()
        };
        length = length
            .checked_add(1)?
            .checked_add(translation_count.checked_mul(12)?)?;
        let rotation = animation.frames[0].rotations[bone];
        let rotation_count = if animation
            .frames
            .iter()
            .all(|frame| frame.rotations[bone] == rotation)
        {
            1
        } else {
            animation.frames.len()
        };
        length = length
            .checked_add(1)?
            .checked_add(rotation_count.checked_mul(16)?)?;
    }
    Some(length)
}

fn compact_animation(
    animation: &Animation,
    identity: &str,
) -> Result<Animation, PresentationError> {
    let length = compact_frame_bytes_len(animation)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidArtifact, identity))?;
    let mut compact_frames = Vec::with_capacity(length);
    compact_frames.extend_from_slice(&(animation.bone_map.len() as u32).to_le_bytes());
    compact_frames.extend_from_slice(&(animation.frames.len() as u32).to_le_bytes());
    for bone in 0..animation.bone_map.len() {
        let translation = animation.frames[0].translations[bone];
        let constant_translation = animation
            .frames
            .iter()
            .all(|frame| frame.translations[bone] == translation);
        compact_frames.push(u8::from(!constant_translation));
        let translation_frames = if constant_translation {
            &animation.frames[..1]
        } else {
            &animation.frames
        };
        for frame in translation_frames {
            for value in frame.translations[bone].0 {
                compact_frames.extend_from_slice(&value.0.to_le_bytes());
            }
        }
        let rotation = animation.frames[0].rotations[bone];
        let constant_rotation = animation
            .frames
            .iter()
            .all(|frame| frame.rotations[bone] == rotation);
        compact_frames.push(u8::from(!constant_rotation));
        let rotation_frames = if constant_rotation {
            &animation.frames[..1]
        } else {
            &animation.frames
        };
        for frame in rotation_frames {
            for value in frame.rotations[bone] {
                compact_frames.extend_from_slice(&value.0.to_le_bytes());
            }
        }
    }
    if compact_frames.len() != length {
        return Err(presentation_error(
            PresentationErrorCode::InvalidArtifact,
            identity,
        ));
    }
    Ok(Animation {
        index: animation.index,
        name: animation.name.clone(),
        fps: animation.fps,
        flags: animation.flags,
        frame_count: animation.frame_count,
        movement_count: animation.movement_count,
        animation_block: animation.animation_block,
        animation_offset: animation.animation_offset,
        ik_rule_count: animation.ik_rule_count,
        local_hierarchy_count: animation.local_hierarchy_count,
        section_offset: animation.section_offset,
        section_frame_count: animation.section_frame_count,
        zero_frame_count: animation.zero_frame_count,
        source_identity: animation.source_identity.clone(),
        bone_map: animation.bone_map.clone(),
        sections: animation.sections.clone(),
        frames: Vec::new(),
        compact_frames,
        authored_frames: None,
    })
}

fn compact_frame(
    animation: &Animation,
    frame: usize,
    identity: &str,
) -> Result<crate::AnimationFrame, PresentationError> {
    let invalid = || presentation_error(PresentationErrorCode::InvalidArtifact, identity);
    let bytes = &animation.compact_frames;
    if bytes.len() < 8 {
        return Err(invalid());
    }
    let bone_count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| invalid())?) as usize;
    let frame_count = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| invalid())?) as usize;
    if bone_count != animation.bone_map.len()
        || frame_count != animation.frame_count as usize
        || frame >= frame_count
    {
        return Err(invalid());
    }
    let mut cursor = 8_usize;
    let mut translations = Vec::with_capacity(bone_count);
    let mut rotations = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        let translation_mode = *bytes.get(cursor).ok_or_else(invalid)?;
        cursor += 1;
        let translation_count = match translation_mode {
            0 => 1,
            1 => frame_count,
            _ => return Err(invalid()),
        };
        let translation_bytes = translation_count.checked_mul(12).ok_or_else(invalid)?;
        let translation_end = cursor
            .checked_add(translation_bytes)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(invalid)?;
        let translation_at = cursor + if translation_mode == 0 { 0 } else { frame * 12 };
        translations.push(Vector3(std::array::from_fn(|axis| {
            let offset = translation_at + axis * 4;
            Float32(u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("validated compact translation"),
            ))
        })));
        cursor = translation_end;

        let rotation_mode = *bytes.get(cursor).ok_or_else(invalid)?;
        cursor += 1;
        let rotation_count = match rotation_mode {
            0 => 1,
            1 => frame_count,
            _ => return Err(invalid()),
        };
        let rotation_bytes = rotation_count.checked_mul(16).ok_or_else(invalid)?;
        let rotation_end = cursor
            .checked_add(rotation_bytes)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(invalid)?;
        let rotation_at = cursor + if rotation_mode == 0 { 0 } else { frame * 16 };
        rotations.push(std::array::from_fn(|axis| {
            let offset = rotation_at + axis * 4;
            Float32(u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("validated compact rotation"),
            ))
        }));
        cursor = rotation_end;
    }
    if cursor != bytes.len() {
        return Err(invalid());
    }
    Ok(crate::AnimationFrame {
        translations,
        rotations,
    })
}

fn validate_manifest(
    manifest: &MaterialResolutionManifest,
    response: &PresentationDependencyResponse,
    limits: PresentationLimits,
) -> Result<(), PresentationError> {
    if manifest.root_identity != response.logical_path
        || !valid_path(&manifest.root_identity, ".vmt")
        || manifest.textures.len() > limits.max_textures_per_material
    {
        return Err(presentation_error(
            PresentationErrorCode::InvalidManifest,
            &response.logical_path,
        ));
    }
    let mut known_sources = vec![manifest.root_identity.as_str()];
    for source in &manifest.include_sources {
        if !known_sources.contains(&source.requester.as_str())
            || !valid_path(&source.logical_path, ".vmt")
            || source.logical_path == manifest.root_identity
            || known_sources.contains(&source.logical_path.as_str())
        {
            return Err(presentation_error(
                PresentationErrorCode::InvalidManifest,
                &source.logical_path,
            ));
        }
        known_sources.push(&source.logical_path);
    }
    for texture in &manifest.textures {
        if texture.parameter.is_empty()
            || texture.parameter.len() > limits.max_string_bytes
            || match texture.disposition {
                TextureDisposition::Source => texture
                    .logical_path
                    .as_deref()
                    .is_none_or(|path| !valid_path(path, ".vtf")),
                TextureDisposition::BuiltInEnvironment
                | TextureDisposition::BuiltInRenderTarget => texture.logical_path.is_some(),
            }
        {
            return Err(presentation_error(
                PresentationErrorCode::InvalidManifest,
                &manifest.root_identity,
            ));
        }
    }
    Ok(())
}

fn valid_path(path: &str, suffix: &str) -> bool {
    path.starts_with("materials/")
        && path.ends_with(suffix)
        && !path.contains('\\')
        && path.bytes().all(|byte| !byte.is_ascii_uppercase())
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
}

fn response_matches(
    response: &PresentationDependencyResponse,
    request: &PresentationDependencyRequest,
) -> bool {
    response.requester == request.requester
        && response.role == request.role
        && response.logical_path == request.logical_path
        && response.material_slot == request.material_slot
        && response.texture_role == request.texture_role
}

fn validate_response(
    response: &PresentationDependencyResponse,
    limits: PresentationLimits,
) -> Result<(), PresentationError> {
    let byte_length = response
        .verified_byte_length
        .or_else(|| response.bytes.as_ref().map(Vec::len))
        .ok_or_else(|| {
            presentation_error(
                PresentationErrorCode::MissingDependency,
                &response.logical_path,
            )
        })?;
    if byte_length > limits.max_dependency_bytes {
        return Err(presentation_error(
            PresentationErrorCode::DependencyLimit,
            &response.logical_path,
        ));
    }
    let expected = response.sha256.ok_or_else(|| {
        presentation_error(PresentationErrorCode::HashMismatch, &response.logical_path)
    })?;
    if response.verified_byte_length.is_none()
        && sha256(response.bytes.as_ref().expect("response bytes checked")) != expected
    {
        return Err(presentation_error(
            PresentationErrorCode::HashMismatch,
            &response.logical_path,
        ));
    }
    Ok(())
}

fn push_dependency(
    dependencies: &mut Vec<ArtifactDependency>,
    response: &PresentationDependencyResponse,
) -> Result<usize, PresentationError> {
    let byte_length = response
        .verified_byte_length
        .or_else(|| response.bytes.as_ref().map(Vec::len))
        .ok_or_else(|| {
            presentation_error(
                PresentationErrorCode::MissingDependency,
                &response.logical_path,
            )
        })?;
    let index = dependencies.len();
    dependencies.push(ArtifactDependency {
        requester: response.requester.clone(),
        role: response.role,
        logical_path: response.logical_path.clone(),
        texture_role: response.texture_role,
        material_slot: Some(response.material_slot),
        sha256: response.sha256,
        byte_length,
    });
    Ok(index)
}

fn bound_requests(
    requests: &[PresentationDependencyRequest],
    limits: PresentationLimits,
    identity: &str,
) -> Result<(), PresentationError> {
    if requests.len() > limits.max_dependencies {
        return Err(presentation_error(
            PresentationErrorCode::DependencyLimit,
            identity,
        ));
    }
    Ok(())
}

fn presentation_body_parts(parts: &[BodyPart]) -> Vec<PresentationBodyPart> {
    parts
        .iter()
        .map(|part| PresentationBodyPart {
            index: part.index,
            name: part.name.clone(),
            base: part.base,
            model_names: part.models.iter().map(|model| model.name.clone()).collect(),
        })
        .collect()
}

fn presentation_descriptor(profile: PresentationProfile, flags: i32) -> PresentationDescriptor {
    let geometry = GeometryOrientation::source();
    let entity_angles = EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns;
    match profile {
        PresentationProfile::World => PresentationDescriptor::World {
            geometry,
            entity_angles,
            root_bone: if flags & STUDIO_HEADER_STATIC_PROP != 0 {
                RootBoneContract::StaticPropBoneZeroIsEntity
            } else {
                RootBoneContract::AnimatedBelowEntity
            },
            depth_range: [Float32(0.0_f32.to_bits()), Float32(1.0_f32.to_bits())],
        },
        PresentationProfile::ViewModel => PresentationDescriptor::ViewModel {
            geometry,
            entity_angles,
            default_horizontal_fov_4_by_3: Float32(54.0_f32.to_bits()),
            minimum_fov: Float32(0.1_f32.to_bits()),
            maximum_fov: Float32(179.9_f32.to_bits()),
            near_plane: Float32(1.0_f32.to_bits()),
            far_plane: FarPlaneContract::SuppliedWorldFarPlane,
            depth_range: [Float32(0.0_f32.to_bits()), Float32(0.1_f32.to_bits())],
            draws_after_world: true,
            opaque_before_translucent: true,
            handedness: ViewmodelHandednessContract::OptionalViewSpaceYReflection,
        },
    }
}

fn feature_support(document: &Document) -> Vec<FeatureSupport> {
    let support = |family, records| FeatureSupport {
        family,
        disposition: if records == 0 {
            FeatureDisposition::NotPresent
        } else {
            FeatureDisposition::RetainedNotEvaluated
        },
        records,
    };
    let procedure = |kind| {
        document
            .bones
            .iter()
            .filter(|bone| bone.procedural_type == kind)
            .count()
    };
    let ik = document
        .animations
        .iter()
        .map(|animation| animation.ik_rule_count.max(0) as usize)
        .sum::<usize>()
        + document
            .sequences
            .iter()
            .map(|sequence| sequence.ik_lock_count.max(0) as usize)
            .sum::<usize>()
        + document.unsupported.ik_chains;
    let flex = document
        .body_parts
        .iter()
        .flat_map(|part| &part.models)
        .flat_map(|model| &model.meshes)
        .map(|mesh| mesh.flex_count.max(0) as usize)
        .sum::<usize>()
        + document.unsupported.flex_descriptors
        + document.unsupported.flex_controllers
        + document.unsupported.flex_rules;
    let autolayers = document
        .sequences
        .iter()
        .map(|sequence| sequence.auto_layer_count.max(0) as usize)
        .sum();
    let unknown_procedures = document
        .bones
        .iter()
        .filter(|bone| !matches!(bone.procedural_type, 0..=5))
        .count();
    let mut result = [
        (FeatureFamily::ProceduralAxisInterpolation, procedure(1)),
        (
            FeatureFamily::ProceduralQuaternionInterpolation,
            procedure(2),
        ),
        (FeatureFamily::ProceduralJiggle, procedure(5)),
        (FeatureFamily::ProceduralAimAtBone, procedure(3)),
        (FeatureFamily::ProceduralAimAtAttachment, procedure(4)),
        (FeatureFamily::InverseKinematics, ik),
        (FeatureFamily::Flex, flex),
        (FeatureFamily::SequenceAutoLayers, autolayers),
        (FeatureFamily::UnknownProcedural, unknown_procedures),
    ]
    .map(|(family, records)| support(family, records))
    .to_vec();
    if let Some(value) = result
        .iter_mut()
        .find(|value| value.family == FeatureFamily::SequenceAutoLayers && value.records > 0)
    {
        value.disposition = FeatureDisposition::Evaluated;
    }
    result
}

fn check_cancelled(token: &CancellationToken, identity: &str) -> Result<(), PresentationError> {
    if token.is_cancelled() {
        Err(presentation_error(
            PresentationErrorCode::Cancelled,
            identity,
        ))
    } else {
        Ok(())
    }
}

fn presentation_error(code: PresentationErrorCode, identity: &str) -> PresentationError {
    PresentationError {
        code,
        identity: identity.to_owned(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Matrix3x4(pub [Float32; 12]);

pub fn affine_transform_orientation(
    transform: Matrix3x4,
) -> Result<TransformOrientation, PresentationError> {
    let values = transform.0.map(|value| f32::from_bits(value.0));
    if values.iter().any(|value| !value.is_finite()) {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            "geometry-facing",
        ));
    }
    let determinant = values[0] * (values[5] * values[10] - values[6] * values[9])
        - values[1] * (values[4] * values[10] - values[6] * values[8])
        + values[2] * (values[4] * values[9] - values[5] * values[8]);
    if !determinant.is_finite() || determinant == 0.0 {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            "geometry-facing",
        ));
    }
    Ok(if determinant.is_sign_negative() {
        TransformOrientation::Reversing
    } else {
        TransformOrientation::Preserving
    })
}

pub fn combine_transform_orientations(
    orientations: impl IntoIterator<Item = TransformOrientation>,
) -> TransformOrientation {
    if orientations
        .into_iter()
        .filter(|orientation| *orientation == TransformOrientation::Reversing)
        .count()
        .is_multiple_of(2)
    {
        TransformOrientation::Preserving
    } else {
        TransformOrientation::Reversing
    }
}

pub fn transformed_geometry_facing(
    facing: GeometryFacing,
    orientation: TransformOrientation,
) -> GeometryFacing {
    GeometryFacing {
        front_face: if orientation == TransformOrientation::Reversing {
            facing.front_face.reversed()
        } else {
            facing.front_face
        },
        cull_face: facing.cull_face,
    }
}

pub fn source_skin_family(skin: i32, family_count: usize) -> usize {
    usize::try_from(skin)
        .ok()
        .filter(|&selected| selected < family_count)
        .unwrap_or(0)
}

pub fn source_entity_transform(
    position: Vector3,
    angles: Vector3,
) -> Result<Matrix3x4, PresentationError> {
    let [pitch, yaw, roll] = values3(angles);
    let [x, y, z] = values3(position);
    if ![pitch, yaw, roll, x, y, z].into_iter().all(f32::is_finite) {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            "source-entity-transform",
        ));
    }
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sr, cr) = roll.to_radians().sin_cos();
    let crcy = cr * cy;
    let crsy = cr * sy;
    let srcy = sr * cy;
    let srsy = sr * sy;
    Ok(Matrix3x4(
        [
            cp * cy,
            sp * srcy - crsy,
            sp * crcy + srsy,
            x,
            cp * sy,
            sp * srsy + crcy,
            sp * crsy - srcy,
            y,
            -sp,
            sr * cp,
            cr * cp,
            z,
        ]
        .map(|value| Float32(value.to_bits())),
    ))
}

pub fn reflect_viewmodel_handedness(view_to_world: Matrix3x4, transform: Matrix3x4) -> Matrix3x4 {
    let mut view_space = multiply_matrix(&inverse_rigid_matrix(&view_to_world), &transform);
    for column in 0..4 {
        let index = 4 + column;
        view_space.0[index] = Float32((-f32::from_bits(view_space.0[index].0)).to_bits());
    }
    multiply_matrix(&view_to_world, &view_space)
}

/// Source MatrixAngles extraction for an authored bone-to-world transform.
// Adapted from Valve's Source SDK 2013. Copyright Valve Corporation, All rights reserved.
// See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt at the repository root.
pub fn source_transform_components(matrix:Matrix3x4)->Result<(Vector3,Vector3),PresentationError> {
    let values=matrix.0.map(|value|f32::from_bits(value.0));
    if values.iter().any(|value|!value.is_finite()) {return Err(presentation_error(PresentationErrorCode::InvalidState,"source-transform-components"));}
    let horizontal=(values[0]*values[0]+values[4]*values[4]).sqrt();
    let pitch=(-values[8]).atan2(horizontal).to_degrees();
    let (yaw,roll)=if horizontal>0.001 {(values[4].atan2(values[0]).to_degrees(),values[9].atan2(values[10]).to_degrees())}else{((-values[1]).atan2(values[5]).to_degrees(),0.0)};
    let vector=|value:[f32;3]|Vector3(value.map(|value|Float32(value.to_bits())));
    Ok((vector([values[3],values[7],values[11]]),vector([pitch,yaw,roll])))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationLayer {
    pub sequence: usize,
    pub cycle: Float32,
    pub weight: Float32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationState {
    pub base_sequence: usize,
    pub cycle: Float32,
    pub pose_parameters: Vec<Float32>,
    pub layers: Vec<AnimationLayer>,
    /// Local rotations applied by StandardBlendingRules before hierarchy setup.
    pub bone_rotations: Vec<(usize, [Float32; 4])>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceTiming {
    pub frames_per_second: Float32,
    pub weighted_frame_count: Float32,
    pub cycles_per_second: Float32,
    pub duration_seconds: Float32,
    pub looping: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampledAttachment {
    pub index: usize,
    pub name: Vec<u8>,
    pub world_aligned: bool,
    pub model_transform: Matrix3x4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampledPose {
    pub local_translations: Vec<Vector3>,
    pub local_rotations: Vec<[Float32; 4]>,
    pub model_matrices: Vec<Matrix3x4>,
    pub skinning_matrices: Vec<Matrix3x4>,
    pub attachments: Vec<SampledAttachment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampledWorldPose {
    pub bone_matrices: Vec<Matrix3x4>,
    pub skinning_matrices: Vec<Matrix3x4>,
    pub attachments: Vec<SampledAttachment>,
}

pub fn apply_entity_transform(
    model: &PresentationModel,
    pose: &SampledPose,
    model_to_world: Matrix3x4,
) -> Result<SampledWorldPose, PresentationError> {
    if pose.model_matrices.len() != model.bones.len()
        || pose.skinning_matrices.len() != model.bones.len()
    {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let static_prop = matches!(
        model.descriptor,
        PresentationDescriptor::World {
            root_bone: RootBoneContract::StaticPropBoneZeroIsEntity,
            ..
        }
    );
    let mut bone_matrices = pose
        .model_matrices
        .iter()
        .map(|matrix| multiply_matrix(&model_to_world, matrix))
        .collect::<Vec<_>>();
    let mut skinning_matrices = pose
        .skinning_matrices
        .iter()
        .map(|matrix| multiply_matrix(&model_to_world, matrix))
        .collect::<Vec<_>>();
    if static_prop && !model.bones.is_empty() {
        bone_matrices[0] = model_to_world;
        skinning_matrices[0] = model_to_world;
    }
    let attachments = pose
        .attachments
        .iter()
        .map(|attachment| SampledAttachment {
            index: attachment.index,
            name: attachment.name.clone(),
            world_aligned: attachment.world_aligned,
            model_transform: if attachment.world_aligned {
                let position = transform_point(
                    &model_to_world,
                    values3(matrix_translation(&attachment.model_transform)),
                );
                quaternion_matrix(identity_quaternion(), vector(position))
            } else {
                multiply_matrix(&model_to_world, &attachment.model_transform)
            },
        })
        .collect();
    Ok(SampledWorldPose {
        bone_matrices,
        skinning_matrices,
        attachments,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedPrimitive {
    pub primitive: usize,
    pub material: usize,
}

pub fn sequences_for_activity_name(model: &PresentationModel, activity: &[u8]) -> Vec<usize> {
    model
        .sequences
        .iter()
        .filter(|sequence| sequence.activity_name.eq_ignore_ascii_case(activity))
        .map(|sequence| sequence.index)
        .collect()
}

pub fn sequences_for_activity(model: &PresentationModel, activity: i32) -> Vec<usize> {
    model
        .sequences
        .iter()
        .filter(|sequence| sequence.activity == activity)
        .map(|sequence| sequence.index)
        .collect()
}

pub fn sequence_timing(
    model: &PresentationModel,
    sequence_index: usize,
    pose_parameters: &[Float32],
) -> Result<SequenceTiming, PresentationError> {
    let sequence = model
        .sequences
        .get(sequence_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if pose_parameters.len() != model.pose_parameters.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let samples = sequence_timing_samples(model, sequence, pose_parameters)?;
    let mut fps = 0.0_f32;
    let mut frame_count = 0.0_f32;
    let mut cycles_per_second = 0.0_f32;
    for (animation, weight) in samples {
        if weight <= 0.0 {
            continue;
        }
        let animation = model.animations.get(animation).ok_or_else(|| {
            presentation_error(PresentationErrorCode::InvalidState, &model.identity)
        })?;
        let animation_fps = finite(animation.fps).ok_or_else(|| {
            presentation_error(PresentationErrorCode::InvalidState, &model.identity)
        })?;
        let frames = animation.frame_count as f32;
        fps += animation_fps * weight;
        frame_count += frames * weight;
        if animation.frame_count > 1 {
            cycles_per_second += animation_fps / (frames - 1.0) * weight;
        }
    }
    if frame_count > 1.0 {
        frame_count -= 1.0;
    }
    let duration = if cycles_per_second == 0.0 {
        0.0
    } else {
        1.0 / cycles_per_second
    };
    Ok(SequenceTiming {
        frames_per_second: Float32(fps.to_bits()),
        weighted_frame_count: Float32(frame_count.to_bits()),
        cycles_per_second: Float32(cycles_per_second.to_bits()),
        duration_seconds: Float32(duration.to_bits()),
        looping: sequence.flags & STUDIO_LOOPING != 0,
    })
}

fn sequence_cycles_per_second(
    model: &PresentationModel,
    sequence_index: usize,
    pose_parameters: &[Float32],
) -> Result<f32, PresentationError> {
    finite(sequence_timing(model, sequence_index, pose_parameters)?.cycles_per_second)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))
}

fn sequence_timing_samples(
    model: &PresentationModel,
    sequence: &crate::Sequence,
    pose_parameters: &[Float32],
) -> Result<[(usize, f32); 4], PresentationError> {
    let (x, sx) = local_pose_setting(model, sequence, 0, pose_parameters)?;
    let (y, sy) = local_pose_setting(model, sequence, 1, pose_parameters)?;
    let at = |x: usize, y: usize| -> Result<usize, PresentationError> {
        let width = sequence.blend_size[0] as usize;
        let height = sequence.blend_size[1] as usize;
        let x = x.min(width.saturating_sub(1));
        let y = y.min(height.saturating_sub(1));
        sequence
            .animation_indices
            .get(y * width + x)
            .and_then(|value| usize::try_from(*value).ok())
            .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))
    };
    Ok([
        (at(x, y)?, (1.0 - sx) * (1.0 - sy)),
        (at(x + 1, y)?, sx * (1.0 - sy)),
        (at(x, y + 1)?, (1.0 - sx) * sy),
        (at(x + 1, y + 1)?, sx * sy),
    ])
}

pub fn sequence_events_between(
    model: &PresentationModel,
    sequence_index: usize,
    previous_cycle: Float32,
    current_cycle: Float32,
) -> Result<Vec<&crate::SequenceEvent>, PresentationError> {
    let sequence = model
        .sequences
        .get(sequence_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let previous = finite(previous_cycle)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let current = finite(current_cycle)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if previous == current {
        return Ok(Vec::new());
    }
    let event_cycle = |event: &&crate::SequenceEvent| finite(event.cycle);
    if current <= previous {
        if sequence.flags & STUDIO_LOOPING == 0 || previous - current <= 0.5 {
            return Ok(Vec::new());
        }
        let mut result = sequence
            .events
            .iter()
            .filter(|event| event_cycle(event).is_some_and(|cycle| cycle > previous))
            .collect::<Vec<_>>();
        let restarted_previous = current - 0.001;
        result.extend(sequence.events.iter().filter(|event| {
            event_cycle(event).is_some_and(|cycle| cycle > restarted_previous && cycle <= current)
        }));
        Ok(result)
    } else {
        Ok(sequence
            .events
            .iter()
            .filter(|event| {
                event_cycle(event).is_some_and(|cycle| cycle > previous && cycle <= current)
            })
            .collect())
    }
}

pub fn presentation_events_between(
    model: &PresentationModel,
    sequence_index: usize,
    previous_cycle: Float32,
    current_cycle: Float32,
) -> Result<Vec<&crate::SequenceEvent>, PresentationError> {
    const EVENT_CLIENT: i32 = 5_000;
    const EVENT_TYPE_CLIENT: i32 = 1 << 4;
    const EVENT_TYPE_NEW: i32 = 1 << 10;
    Ok(
        sequence_events_between(model, sequence_index, previous_cycle, current_cycle)?
            .into_iter()
            .filter(|event| {
                if event.event_type & EVENT_TYPE_NEW != 0 {
                    event.event_type & EVENT_TYPE_CLIENT != 0
                } else {
                    event.event >= EVENT_CLIENT
                }
            })
            .collect(),
    )
}

pub fn select_primitives(
    model: &PresentationModel,
    bodygroups: &[usize],
    skin: usize,
    lod: usize,
) -> Result<Vec<SelectedPrimitive>, PresentationError> {
    if bodygroups.len() != model.body_parts.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidSelection,
            &model.identity,
        ));
    }
    for (part, &selected) in model.body_parts.iter().zip(bodygroups) {
        if selected >= part.model_names.len() {
            return Err(presentation_error(
                PresentationErrorCode::InvalidSelection,
                &model.identity,
            ));
        }
    }
    let skin = model.skins.get(skin).ok_or_else(|| {
        presentation_error(PresentationErrorCode::InvalidSelection, &model.identity)
    })?;
    model
        .geometry
        .iter()
        .enumerate()
        .filter(|(_, primitive)| {
            primitive.lod == lod
                && bodygroups
                    .get(primitive.body_part)
                    .is_some_and(|selected| *selected == primitive.model)
        })
        .map(|(primitive_index, primitive)| {
            let source_material = skin
                .texture_indices
                .get(primitive.material_slot)
                .copied()
                .map(usize::from)
                .filter(|material| *material < model.base_material_count)
                .ok_or_else(|| {
                    presentation_error(PresentationErrorCode::InvalidSelection, &model.identity)
                })?;
            let material = model
                .materials
                .iter()
                .position(|material| {
                    material.source_slot == source_material && material.lod == Some(lod)
                })
                .or_else(|| {
                    model.materials.iter().position(|material| {
                        material.source_slot == source_material && material.lod.is_none()
                    })
                })
                .ok_or_else(|| {
                    presentation_error(PresentationErrorCode::InvalidSelection, &model.identity)
                })?;
            Ok(SelectedPrimitive {
                primitive: primitive_index,
                material,
            })
        })
        .collect()
}

pub fn sample_pose(
    model: &PresentationModel,
    state: &AnimationState,
) -> Result<SampledPose, PresentationError> {
    sample_pose_at_time(model, state, Float32(0.0_f32.to_bits()))
}

pub fn sample_pose_at_time(
    model: &PresentationModel,
    state: &AnimationState,
    time: Float32,
) -> Result<SampledPose, PresentationError> {
    if state.pose_parameters.len() != model.pose_parameters.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let time = finite(time)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let mut local = bind_pose(model);
    let mut stack = Vec::new();
    accumulate_sequence(
        model,
        &mut local,
        state.base_sequence,
        state.cycle,
        1.0,
        time,
        &state.pose_parameters,
        &mut stack,
    )?;
    for layer in &state.layers {
        let weight = finite_unit(layer.weight).ok_or_else(|| {
            presentation_error(PresentationErrorCode::InvalidState, &model.identity)
        })?;
        accumulate_sequence(
            model,
            &mut local,
            layer.sequence,
            layer.cycle,
            weight,
            time,
            &state.pose_parameters,
            &mut stack,
        )?;
    }
    for &(bone, rotation) in &state.bone_rotations {
        if bone >= local.1.len() || rotation.iter().any(|value| !f32::from_bits(value.0).is_finite()) {
            return Err(presentation_error(PresentationErrorCode::InvalidState, &model.identity));
        }
        local.1[bone] = rotation;
    }
    let mut model_matrices = Vec::with_capacity(model.bones.len());
    let mut skinning_matrices = Vec::with_capacity(model.bones.len());
    for (index, bone) in model.bones.iter().enumerate() {
        let local_matrix = quaternion_matrix(local.1[index], local.0[index]);
        let model_matrix = if bone.parent == -1 {
            local_matrix
        } else {
            let parent = usize::try_from(bone.parent).map_err(|_| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
            multiply_matrix(
                model_matrices.get(parent).ok_or_else(|| {
                    presentation_error(PresentationErrorCode::InvalidState, &model.identity)
                })?,
                &local_matrix,
            )
        };
        skinning_matrices.push(multiply_matrix(
            &model_matrix,
            &Matrix3x4(bone.pose_to_bone),
        ));
        model_matrices.push(model_matrix);
    }
    let attachments = model
        .attachments
        .iter()
        .map(|attachment| {
            let bone = usize::try_from(attachment.bone)
                .ok()
                .and_then(|index| model_matrices.get(index))
                .ok_or_else(|| {
                    presentation_error(PresentationErrorCode::InvalidState, &model.identity)
                })?;
            Ok(SampledAttachment {
                index: attachment.index,
                name: attachment.name.clone(),
                world_aligned: attachment.flags & 0x0001_0000 != 0,
                model_transform: if attachment.flags & 0x0001_0000 != 0 {
                    let local = Matrix3x4(attachment.local);
                    let position = transform_point(bone, values3(matrix_translation(&local)));
                    quaternion_matrix(identity_quaternion(), vector(position))
                } else {
                    multiply_matrix(bone, &Matrix3x4(attachment.local))
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SampledPose {
        local_translations: local.0,
        local_rotations: local.1,
        model_matrices,
        skinning_matrices,
        attachments,
    })
}

type LocalPose = (Vec<Vector3>, Vec<[Float32; 4]>);

#[allow(clippy::too_many_arguments)]
fn accumulate_sequence(
    model: &PresentationModel,
    destination: &mut LocalPose,
    sequence_index: usize,
    supplied_cycle: Float32,
    weight: f32,
    time: f32,
    pose_parameters: &[Float32],
    stack: &mut Vec<usize>,
) -> Result<(), PresentationError> {
    if !(0.0..=1.0).contains(&weight) || !weight.is_finite() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    if weight == 0.0 {
        return Ok(());
    }
    if stack.contains(&sequence_index) || stack.len() >= model.sequences.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let sequence = model
        .sequences
        .get(sequence_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let mut cycle = finite(supplied_cycle)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if sequence.flags & STUDIO_REALTIME != 0 {
        cycle = time * sequence_cycles_per_second(model, sequence_index, pose_parameters)?;
    } else if sequence.flags & STUDIO_CYCLE_POSE != 0 {
        cycle = usize::try_from(sequence.cycle_pose_parameter)
            .ok()
            .and_then(|index| pose_parameters.get(index))
            .and_then(|value| finite(*value))
            .ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
    }
    cycle = normalized_cycle(cycle, sequence.flags & STUDIO_LOOPING != 0);
    let local_layers = sequence
        .auto_layers
        .iter()
        .filter(|layer| layer.flags & STUDIO_AUTO_LAYER_LOCAL != 0)
        .cloned()
        .collect::<Vec<_>>();
    let ordinary_layers = sequence
        .auto_layers
        .iter()
        .filter(|layer| layer.flags & STUDIO_AUTO_LAYER_LOCAL == 0)
        .cloned()
        .collect::<Vec<_>>();
    let sequence_flags = sequence.flags;
    stack.push(sequence_index);
    let result = (|| {
        let mut sampled = sample_sequence_raw(model, sequence_index, cycle, pose_parameters)?;
        if sequence_flags & STUDIO_LOCAL != 0 {
            for layer in &local_layers {
                accumulate_auto_layer(
                    model,
                    &mut sampled,
                    layer,
                    cycle,
                    1.0,
                    time,
                    pose_parameters,
                    true,
                    stack,
                )?;
            }
        }
        blend_sequence_into(model, destination, &sampled, sequence_index, weight)?;
        for layer in &ordinary_layers {
            accumulate_auto_layer(
                model,
                destination,
                layer,
                cycle,
                weight,
                time,
                pose_parameters,
                false,
                stack,
            )?;
        }
        Ok(())
    })();
    stack.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn accumulate_auto_layer(
    model: &PresentationModel,
    destination: &mut LocalPose,
    layer: &crate::SequenceAutoLayer,
    parent_cycle: f32,
    parent_weight: f32,
    time: f32,
    pose_parameters: &[Float32],
    local: bool,
    stack: &mut Vec<usize>,
) -> Result<(), PresentationError> {
    let start = finite(layer.start)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let peak = finite(layer.peak)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let tail = finite(layer.tail)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let end = finite(layer.end)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let mut layer_cycle = parent_cycle;
    let mut layer_weight = parent_weight;
    if start != end {
        let index = if !local && layer.flags & STUDIO_AUTO_LAYER_POSE != 0 {
            let pose_index = usize::try_from(layer.pose)
                .ok()
                .filter(|index| *index < model.pose_parameters.len())
                .ok_or_else(|| {
                    presentation_error(PresentationErrorCode::InvalidState, &model.identity)
                })?;
            let pose = &model.pose_parameters[pose_index];
            let value = finite(pose_parameters[pose_index]).ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
            let pose_start = finite(pose.start).ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
            let pose_end = finite(pose.end).ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
            value * (pose_end - pose_start) + pose_start
        } else {
            parent_cycle
        };
        if index < start || index >= end {
            return Ok(());
        }
        let mut ramp = 1.0_f32;
        if index < peak && start != peak {
            ramp = (index - start) / (peak - start);
        } else if index > tail && end != tail {
            ramp = (end - index) / (end - tail);
        }
        if layer.flags & STUDIO_AUTO_LAYER_SPLINE != 0 {
            ramp = ramp * ramp * (3.0 - 2.0 * ramp);
        }
        layer_weight = if layer.flags & STUDIO_AUTO_LAYER_CROSSFADE != 0 && index > tail {
            let denominator = 1.0 - parent_weight + ramp * parent_weight;
            if denominator == 0.0 {
                return Err(presentation_error(
                    PresentationErrorCode::InvalidState,
                    &model.identity,
                ));
            }
            ramp * parent_weight / denominator
        } else if layer.flags & STUDIO_AUTO_LAYER_NO_BLEND != 0 {
            ramp
        } else {
            parent_weight * ramp
        };
        if local || layer.flags & STUDIO_AUTO_LAYER_POSE == 0 {
            layer_cycle = (parent_cycle - start) / (end - start);
        }
    }
    if !layer_cycle.is_finite() || !(0.0..=1.0).contains(&layer_weight) {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let sequence = usize::try_from(layer.sequence)
        .ok()
        .filter(|index| *index < model.sequences.len())
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    accumulate_sequence(
        model,
        destination,
        sequence,
        Float32(layer_cycle.to_bits()),
        layer_weight,
        time,
        pose_parameters,
        stack,
    )
}

fn blend_sequence_into(
    model: &PresentationModel,
    destination: &mut LocalPose,
    sampled: &LocalPose,
    sequence_index: usize,
    weight: f32,
) -> Result<(), PresentationError> {
    let sequence = model
        .sequences
        .get(sequence_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if sequence.bone_weights.len() != model.bones.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    if sequence.flags & STUDIO_WORLD != 0 {
        return blend_sequence_world(model, destination, sampled, sequence, weight);
    }
    for (bone, authored_weight) in sequence.bone_weights.iter().enumerate() {
        let bone_weight = finite(*authored_weight)
            .filter(|value| (0.0..=1.0).contains(value))
            .ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
        let blend = weight * bone_weight;
        if blend <= 0.0 {
            continue;
        }
        if sequence.flags & STUDIO_DELTA != 0 {
            let scaled = quaternion_scale(sampled.1[bone], blend)?;
            destination.1[bone] = if sequence.flags & STUDIO_POST != 0 {
                quaternion_multiply(destination.1[bone], scaled)
            } else {
                quaternion_multiply(scaled, destination.1[bone])
            };
            destination.0[bone] =
                add_vector(destination.0[bone], scale_vector(sampled.0[bone], blend));
        } else {
            destination.0[bone] = lerp_vector(destination.0[bone], sampled.0[bone], blend);
            destination.1[bone] = slerp(
                destination.1[bone],
                sampled.1[bone],
                blend,
                model.bones[bone].flags & BONE_FIXED_ALIGNMENT == 0,
            );
        }
    }
    Ok(())
}

fn sample_sequence_raw(
    model: &PresentationModel,
    sequence_index: usize,
    cycle: f32,
    pose_parameters: &[Float32],
) -> Result<LocalPose, PresentationError> {
    let sequence = model
        .sequences
        .get(sequence_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let (index0, setting0) = local_pose_setting(model, sequence, 0, pose_parameters)?;
    let (index1, setting1) = local_pose_setting(model, sequence, 1, pose_parameters)?;
    let x_next = usize::from(sequence.blend_size[0] > 1);
    let y_next = usize::from(sequence.blend_size[1] > 1);
    let sample = |x, y| sample_grid_animation(model, sequence, x, y, cycle);
    let blended = if setting0 < 0.001 {
        if setting1 < 0.001 {
            sample(index0, index1)?
        } else if setting1 > 0.999 {
            sample(index0, index1 + y_next)?
        } else {
            blend_grid_pose(
                model,
                sequence,
                &sample(index0, index1)?,
                &sample(index0, index1 + y_next)?,
                setting1,
            )?
        }
    } else if setting0 > 0.999 {
        if setting1 < 0.001 {
            sample(index0 + x_next, index1)?
        } else if setting1 > 0.999 {
            sample(index0 + x_next, index1 + y_next)?
        } else {
            blend_grid_pose(
                model,
                sequence,
                &sample(index0 + x_next, index1)?,
                &sample(index0 + x_next, index1 + y_next)?,
                setting1,
            )?
        }
    } else if setting1 < 0.001 {
        blend_grid_pose(
            model,
            sequence,
            &sample(index0, index1)?,
            &sample(index0 + x_next, index1)?,
            setting0,
        )?
    } else if setting1 > 0.999 {
        blend_grid_pose(
            model,
            sequence,
            &sample(index0, index1 + y_next)?,
            &sample(index0 + x_next, index1 + y_next)?,
            setting0,
        )?
    } else {
        let (coordinates, weights) = three_way_blend(index0, index1, setting0, setting1);
        let first = sample(coordinates[0].0, coordinates[0].1)?;
        let third = sample(coordinates[2].0, coordinates[2].1)?;
        if weights[1] < 0.001 {
            blend_grid_pose(
                model,
                sequence,
                &first,
                &third,
                weights[2] / (weights[0] + weights[2]),
            )?
        } else {
            let second = sample(coordinates[1].0, coordinates[1].1)?;
            let first_second = blend_grid_pose(
                model,
                sequence,
                &first,
                &second,
                weights[1] / (weights[0] + weights[1]),
            )?;
            blend_grid_pose(model, sequence, &first_second, &third, weights[2])?
        }
    };
    Ok(blended)
}

fn sample_grid_animation(
    model: &PresentationModel,
    sequence: &crate::Sequence,
    x: usize,
    y: usize,
    cycle: f32,
) -> Result<LocalPose, PresentationError> {
    let width = sequence.blend_size[0] as usize;
    let grid = y
        .checked_mul(width)
        .and_then(|value| value.checked_add(x))
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let animation = sequence
        .animation_indices
        .get(grid)
        .and_then(|value| usize::try_from(*value).ok())
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    sample_animation(model, animation, cycle)
}

fn three_way_blend(
    x: usize,
    y: usize,
    setting_x: f32,
    setting_y: f32,
) -> ([(usize, usize); 3], [f32; 3]) {
    let (coordinates, first, mut second) = if (x + y).is_multiple_of(2) {
        if setting_x > setting_y {
            (
                [(x, y), (x + 1, y), (x + 1, y + 1)],
                1.0 - setting_x,
                setting_x - setting_y,
            )
        } else {
            (
                [(x + 1, y + 1), (x, y + 1), (x, y)],
                setting_x,
                setting_y - setting_x,
            )
        }
    } else if setting_x + setting_y > 1.0 {
        (
            [(x + 1, y), (x + 1, y + 1), (x, y + 1)],
            1.0 - setting_y,
            setting_x - 1.0 + setting_y,
        )
    } else {
        (
            [(x, y + 1), (x, y), (x + 1, y)],
            setting_y,
            1.0 - setting_x - setting_y,
        )
    };
    if second < 0.001 {
        second = 0.0;
    }
    (coordinates, [first, second, 1.0 - first - second])
}

fn local_pose_setting(
    model: &PresentationModel,
    sequence: &crate::Sequence,
    axis: usize,
    values: &[Float32],
) -> Result<(usize, f32), PresentationError> {
    let size = sequence.blend_size[axis] as usize;
    if size <= 1 {
        return Ok((0, 0.0));
    }
    let pose_index = usize::try_from(sequence.pose_parameter_indices[axis])
        .ok()
        .filter(|index| *index < model.pose_parameters.len())
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let pose = &model.pose_parameters[pose_index];
    let mut value = finite(values[pose_index])
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let start = finite(pose.start)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let end = finite(pose.end)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let looping = finite(pose.looping_range)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if looping != 0.0 {
        let wrap = (start + end) * 0.5 + looping * 0.5;
        value -= looping * ((value + looping - wrap) / looping).floor();
    }
    let setting = if sequence.pose_keys[axis].is_empty() {
        if end == start {
            return Err(presentation_error(
                PresentationErrorCode::InvalidState,
                &model.identity,
            ));
        }
        let local_start =
            (finite(sequence.pose_parameter_start[axis]).unwrap_or(start) - start) / (end - start);
        let local_end =
            (finite(sequence.pose_parameter_end[axis]).unwrap_or(end) - start) / (end - start);
        if local_end == local_start {
            return Err(presentation_error(
                PresentationErrorCode::InvalidState,
                &model.identity,
            ));
        }
        ((value - local_start) / (local_end - local_start)).clamp(0.0, 1.0)
    } else {
        value = value * (end - start) + start;
        let keys = &sequence.pose_keys[axis];
        if keys.len() != size {
            return Err(presentation_error(
                PresentationErrorCode::InvalidState,
                &model.identity,
            ));
        }
        let mut index = 0;
        loop {
            let left = finite(keys[index]).ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
            let right = finite(keys[index + 1]).ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
            if right == left {
                return Err(presentation_error(
                    PresentationErrorCode::InvalidState,
                    &model.identity,
                ));
            }
            let setting = (value - left) / (right - left);
            if index < size - 2 && setting > 1.0 {
                index += 1;
            } else {
                return Ok((index, setting.clamp(0.0, 1.0)));
            }
        }
    };
    let position = setting * (size - 1) as f32;
    let index = (position.floor() as usize).min(size - 2);
    Ok((index, position - index as f32))
}

fn sample_animation(
    model: &PresentationModel,
    animation_index: usize,
    cycle: f32,
) -> Result<LocalPose, PresentationError> {
    let animation = model
        .animations
        .get(animation_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    let frame_count = usize::try_from(animation.frame_count)
        .ok()
        .filter(|count| *count > 0)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if animation.frames.is_empty()
        && animation.compact_frames.is_empty()
        && animation.authored_frames.is_none()
    {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let frame = cycle * (frame_count - 1) as f32;
    let first = frame.floor() as usize;
    let second = (first + 1).min(frame_count - 1);
    let fraction = frame - first as f32;
    let authored_frames;
    let compact_frames;
    let (first_frame, second_frame) = if animation.authored_frames.is_some() {
        authored_frames = (
            animation.authored_frame(first).map_err(|_| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?,
            animation.authored_frame(second).map_err(|_| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?,
        );
        (authored_frames.0.as_ref(), authored_frames.1.as_ref())
    } else if animation.frames.is_empty() {
        compact_frames = (
            compact_frame(animation, first, &model.identity)?,
            compact_frame(animation, second, &model.identity)?,
        );
        (&compact_frames.0, &compact_frames.1)
    } else {
        (&animation.frames[first], &animation.frames[second])
    };
    let mut output = if animation.flags & STUDIO_DELTA != 0 {
        (
            vec![vector([0.0; 3]); model.bones.len()],
            vec![identity_quaternion(); model.bones.len()],
        )
    } else {
        bind_pose(model)
    };
    for (local_bone, mapped) in animation.bone_map.iter().enumerate() {
        let Some(root_bone) = mapped else { continue };
        let first_sample = first_frame
            .translations
            .get(local_bone)
            .zip(first_frame.rotations.get(local_bone));
        let second_sample = second_frame
            .translations
            .get(local_bone)
            .zip(second_frame.rotations.get(local_bone));
        let (Some((first_position, first_rotation)), Some((second_position, second_rotation))) =
            (first_sample, second_sample)
        else {
            return Err(presentation_error(
                PresentationErrorCode::InvalidState,
                &model.identity,
            ));
        };
        output.0[*root_bone] = lerp_vector(*first_position, *second_position, fraction);
        output.1[*root_bone] = nlerp(*first_rotation, *second_rotation, fraction);
    }
    Ok(output)
}

fn bind_pose(model: &PresentationModel) -> LocalPose {
    (
        model.bones.iter().map(|bone| bone.position).collect(),
        model.bones.iter().map(|bone| bone.quaternion).collect(),
    )
}

fn blend_grid_pose(
    model: &PresentationModel,
    sequence: &crate::Sequence,
    left: &LocalPose,
    right: &LocalPose,
    weight: f32,
) -> Result<LocalPose, PresentationError> {
    if left.0.len() != model.bones.len()
        || left.1.len() != model.bones.len()
        || right.0.len() != model.bones.len()
        || right.1.len() != model.bones.len()
        || sequence.bone_weights.len() != model.bones.len()
    {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let mut output = left.clone();
    for bone in 0..model.bones.len() {
        let authored = finite(sequence.bone_weights[bone])
            .filter(|value| (0.0..=1.0).contains(value))
            .ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
        if authored > 0.0 {
            output.0[bone] = lerp_vector(left.0[bone], right.0[bone], weight);
            output.1[bone] = if model.bones[bone].flags & BONE_FIXED_ALIGNMENT != 0 {
                nlerp_no_align(left.1[bone], right.1[bone], weight)
            } else {
                nlerp(left.1[bone], right.1[bone], weight)
            };
        }
    }
    Ok(output)
}

fn finite(value: Float32) -> Option<f32> {
    let value = f32::from_bits(value.0);
    value.is_finite().then_some(value)
}

fn finite_unit(value: Float32) -> Option<f32> {
    finite(value).filter(|value| (0.0..=1.0).contains(value))
}

pub(crate) fn vector(values: [f32; 3]) -> Vector3 {
    Vector3(values.map(|value| Float32(value.to_bits())))
}

pub(crate) fn values3(value: Vector3) -> [f32; 3] {
    value.0.map(|component| f32::from_bits(component.0))
}

fn values4(value: [Float32; 4]) -> [f32; 4] {
    value.map(|component| f32::from_bits(component.0))
}

pub(crate) fn identity_quaternion() -> [Float32; 4] {
    [
        Float32(0.0_f32.to_bits()),
        Float32(0.0_f32.to_bits()),
        Float32(0.0_f32.to_bits()),
        Float32(1.0_f32.to_bits()),
    ]
}

fn normalized_cycle(value: f32, looping: bool) -> f32 {
    if looping {
        value - value.floor()
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn lerp_vector(left: Vector3, right: Vector3, weight: f32) -> Vector3 {
    let left = values3(left);
    let right = values3(right);
    vector(std::array::from_fn(|axis| {
        left[axis] * (1.0 - weight) + right[axis] * weight
    }))
}

fn add_vector(left: Vector3, right: Vector3) -> Vector3 {
    let left = values3(left);
    let right = values3(right);
    vector(std::array::from_fn(|axis| left[axis] + right[axis]))
}

fn scale_vector(value: Vector3, scale: f32) -> Vector3 {
    vector(values3(value).map(|component| component * scale))
}

fn dot4(left: [f32; 4], right: [f32; 4]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2] + left[3] * right[3]
}

fn normalized_quaternion(value: [f32; 4]) -> [f32; 4] {
    let length = dot4(value, value).sqrt();
    if length <= f32::EPSILON || !length.is_finite() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        value.map(|component| component / length)
    }
}

fn nlerp(left: [Float32; 4], right: [Float32; 4], weight: f32) -> [Float32; 4] {
    let left = values4(left);
    let mut right = values4(right);
    if dot4(left, right) < 0.0 {
        right = right.map(|component| -component);
    }
    normalized_quaternion(std::array::from_fn(|axis| {
        left[axis] * (1.0 - weight) + right[axis] * weight
    }))
    .map(|component| Float32(component.to_bits()))
}

fn nlerp_no_align(left: [Float32; 4], right: [Float32; 4], weight: f32) -> [Float32; 4] {
    let left = values4(left);
    let right = values4(right);
    normalized_quaternion(std::array::from_fn(|axis| {
        left[axis] * (1.0 - weight) + right[axis] * weight
    }))
    .map(|component| Float32(component.to_bits()))
}

fn slerp(left: [Float32; 4], right: [Float32; 4], weight: f32, align: bool) -> [Float32; 4] {
    let left = values4(left);
    let mut right = values4(right);
    if align {
        let difference = (0..4)
            .map(|axis| (left[axis] - right[axis]).powi(2))
            .sum::<f32>();
        let sum = (0..4)
            .map(|axis| (left[axis] + right[axis]).powi(2))
            .sum::<f32>();
        if difference > sum {
            right = right.map(|value| -value);
        }
    }
    let cosine = dot4(left, right);
    let output = if 1.0 + cosine > 0.000_001 {
        let (left_scale, right_scale) = if 1.0 - cosine > 0.000_001 {
            let omega = cosine.clamp(-1.0, 1.0).acos();
            let sine = omega.sin();
            (
                ((1.0 - weight) * omega).sin() / sine,
                (weight * omega).sin() / sine,
            )
        } else {
            (1.0 - weight, weight)
        };
        std::array::from_fn(|axis| left_scale * left[axis] + right_scale * right[axis])
    } else {
        let perpendicular = [-right[1], right[0], -right[3], right[2]];
        let left_scale = ((1.0 - weight) * std::f32::consts::FRAC_PI_2).sin();
        let right_scale = (weight * std::f32::consts::FRAC_PI_2).sin();
        [
            left_scale * left[0] + right_scale * perpendicular[0],
            left_scale * left[1] + right_scale * perpendicular[1],
            left_scale * left[2] + right_scale * perpendicular[2],
            perpendicular[3],
        ]
    };
    output.map(|value| Float32(value.to_bits()))
}

fn quaternion_scale(
    quaternion: [Float32; 4],
    scale: f32,
) -> Result<[Float32; 4], PresentationError> {
    let value = values4(quaternion);
    let sine = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2])
        .sqrt()
        .min(1.0);
    let scaled_sine = (sine.asin() * scale).sin();
    let vector_scale = scaled_sine / (sine + f32::EPSILON);
    let scalar_squared = (1.0 - scaled_sine * scaled_sine).max(0.0);
    let scalar = scalar_squared.sqrt().copysign(value[3]);
    let output = [
        value[0] * vector_scale,
        value[1] * vector_scale,
        value[2] * vector_scale,
        scalar,
    ];
    if output.into_iter().any(|value| !value.is_finite()) {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            "quaternion-scale",
        ));
    }
    Ok(output.map(|value| Float32(value.to_bits())))
}

fn quaternion_multiply(left: [Float32; 4], right: [Float32; 4]) -> [Float32; 4] {
    let [lx, ly, lz, lw] = values4(left);
    let [rx, ry, rz, rw] = values4(right);
    normalized_quaternion([
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ])
    .map(|component| Float32(component.to_bits()))
}

pub(crate) fn quaternion_matrix(rotation: [Float32; 4], translation: Vector3) -> Matrix3x4 {
    let [x, y, z, w] = normalized_quaternion(values4(rotation));
    let [tx, ty, tz] = values3(translation);
    Matrix3x4(
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
            tx,
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
            ty,
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
            tz,
        ]
        .map(|component| Float32(component.to_bits())),
    )
}

pub(crate) fn multiply_matrix(left: &Matrix3x4, right: &Matrix3x4) -> Matrix3x4 {
    let left = left.0.map(|value| f32::from_bits(value.0));
    let right = right.0.map(|value| f32::from_bits(value.0));
    let mut output = [0.0_f32; 12];
    for row in 0..3 {
        for column in 0..3 {
            output[row * 4 + column] = (0..3)
                .map(|axis| left[row * 4 + axis] * right[axis * 4 + column])
                .sum();
        }
        output[row * 4 + 3] = left[row * 4 + 3]
            + (0..3)
                .map(|axis| left[row * 4 + axis] * right[axis * 4 + 3])
                .sum::<f32>();
    }
    Matrix3x4(output.map(|component| Float32(component.to_bits())))
}

pub(crate) fn transform_point(matrix: &Matrix3x4, point: [f32; 3]) -> [f32; 3] {
    let matrix = matrix.0.map(|value| f32::from_bits(value.0));
    std::array::from_fn(|row| {
        matrix[row * 4] * point[0]
            + matrix[row * 4 + 1] * point[1]
            + matrix[row * 4 + 2] * point[2]
            + matrix[row * 4 + 3]
    })
}

fn pose_model_matrices(
    model: &PresentationModel,
    pose: &LocalPose,
) -> Result<Vec<Matrix3x4>, PresentationError> {
    if pose.0.len() != model.bones.len() || pose.1.len() != model.bones.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let mut matrices = Vec::with_capacity(model.bones.len());
    for (bone, definition) in model.bones.iter().enumerate() {
        let local = quaternion_matrix(pose.1[bone], pose.0[bone]);
        let matrix = if definition.parent < 0 {
            local
        } else {
            let parent = usize::try_from(definition.parent)
                .ok()
                .and_then(|index| matrices.get(index))
                .ok_or_else(|| {
                    presentation_error(PresentationErrorCode::InvalidState, &model.identity)
                })?;
            multiply_matrix(parent, &local)
        };
        matrices.push(matrix);
    }
    Ok(matrices)
}

pub(crate) fn matrix_translation(matrix: &Matrix3x4) -> Vector3 {
    Vector3([matrix.0[3], matrix.0[7], matrix.0[11]])
}

fn matrix_quaternion(matrix: &Matrix3x4) -> [Float32; 4] {
    let m = matrix.0.map(|value| f32::from_bits(value.0));
    let trace = m[0] + m[5] + m[10];
    let value = if trace > 0.0 {
        let scale = (trace + 1.0).sqrt() * 2.0;
        [
            (m[9] - m[6]) / scale,
            (m[2] - m[8]) / scale,
            (m[4] - m[1]) / scale,
            0.25 * scale,
        ]
    } else if m[0] > m[5] && m[0] > m[10] {
        let scale = (1.0 + m[0] - m[5] - m[10]).sqrt() * 2.0;
        [
            0.25 * scale,
            (m[1] + m[4]) / scale,
            (m[2] + m[8]) / scale,
            (m[9] - m[6]) / scale,
        ]
    } else if m[5] > m[10] {
        let scale = (1.0 + m[5] - m[0] - m[10]).sqrt() * 2.0;
        [
            (m[1] + m[4]) / scale,
            0.25 * scale,
            (m[6] + m[9]) / scale,
            (m[2] - m[8]) / scale,
        ]
    } else {
        let scale = (1.0 + m[10] - m[0] - m[5]).sqrt() * 2.0;
        [
            (m[2] + m[8]) / scale,
            (m[6] + m[9]) / scale,
            0.25 * scale,
            (m[4] - m[1]) / scale,
        ]
    };
    normalized_quaternion(value).map(|value| Float32(value.to_bits()))
}

fn inverse_rigid_matrix(matrix: &Matrix3x4) -> Matrix3x4 {
    let m = matrix.0.map(|value| f32::from_bits(value.0));
    let mut output = [0.0_f32; 12];
    for row in 0..3 {
        for column in 0..3 {
            output[row * 4 + column] = m[column * 4 + row];
        }
        output[row * 4 + 3] = -(0..3)
            .map(|axis| output[row * 4 + axis] * m[axis * 4 + 3])
            .sum::<f32>();
    }
    Matrix3x4(output.map(|value| Float32(value.to_bits())))
}

fn blend_sequence_world(
    model: &PresentationModel,
    destination: &mut LocalPose,
    sampled: &LocalPose,
    sequence: &crate::Sequence,
    weight: f32,
) -> Result<(), PresentationError> {
    let destination_world = pose_model_matrices(model, destination)?;
    let sampled_world = pose_model_matrices(model, sampled)?;
    let mut target_world = destination_world.clone();
    for bone in 0..model.bones.len() {
        let authored = finite(sequence.bone_weights[bone])
            .filter(|value| (0.0..=1.0).contains(value))
            .ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
        let blend = weight * authored;
        let parent = usize::try_from(model.bones[bone].parent).ok();
        let parent_blend = if let Some(parent) = parent {
            weight
                * finite(sequence.bone_weights[parent])
                    .filter(|value| (0.0..=1.0).contains(value))
                    .ok_or_else(|| {
                        presentation_error(PresentationErrorCode::InvalidState, &model.identity)
                    })?
        } else {
            0.0
        };
        if parent_blend == 1.0 && blend == 1.0 {
            destination.0[bone] = sampled.0[bone];
            destination.1[bone] = sampled.1[bone];
            target_world[bone] = sampled_world[bone];
            continue;
        }
        if blend <= 0.0 {
            target_world[bone] = if let Some(parent) = parent {
                multiply_matrix(
                    &target_world[parent],
                    &quaternion_matrix(destination.1[bone], destination.0[bone]),
                )
            } else {
                quaternion_matrix(destination.1[bone], destination.0[bone])
            };
            continue;
        }
        let target_rotation = slerp(
            matrix_quaternion(&destination_world[bone]),
            matrix_quaternion(&sampled_world[bone]),
            blend,
            true,
        );
        target_world[bone] = quaternion_matrix(
            target_rotation,
            matrix_translation(&destination_world[bone]),
        );
        if let Some(parent) = parent {
            let local = multiply_matrix(
                &inverse_rigid_matrix(&target_world[parent]),
                &target_world[bone],
            );
            destination.1[bone] = matrix_quaternion(&local);
            destination.0[bone] = lerp_vector(destination.0[bone], sampled.0[bone], blend);
        } else {
            destination.1[bone] = target_rotation;
        }
    }
    Ok(())
}

fn encode_model(
    model: &PresentationModel,
    limits: PresentationLimits,
) -> Result<Vec<u8>, PresentationError> {
    let mut output = ArtifactWriter::new(
        limits
            .max_artifact_bytes
            .min(limits.max_owned_bytes)
            .min(MAX_MESSAGE_BYTES - 1),
        &model.identity,
    );
    output.raw(ARTIFACT_MAGIC)?;
    output.u16(ARTIFACT_VERSION)?;
    output.u8(match model.profile {
        PresentationProfile::World => 0,
        PresentationProfile::ViewModel => 1,
    })?;
    output.u8(0)?;
    output.u64(0)?;
    output.i32(model.checksum)?;
    output.i32(model.flags)?;
    output.text(&model.identity, limits.max_string_bytes)?;
    output.vector(model.basis.forward)?;
    output.vector(model.basis.left)?;
    output.vector(model.basis.up)?;
    for bounds in model.collision_bounds { output.vector(bounds)?; }
    match model.descriptor {
        PresentationDescriptor::World {
            geometry,
            entity_angles,
            root_bone,
            depth_range,
        } => {
            output.u8(0)?;
            output.geometry_orientation(geometry)?;
            output.u8(entity_angles as u8)?;
            output.u8(root_bone as u8)?;
            output.float_slice(&depth_range)?;
        }
        PresentationDescriptor::ViewModel {
            geometry,
            entity_angles,
            default_horizontal_fov_4_by_3,
            minimum_fov,
            maximum_fov,
            near_plane,
            far_plane,
            depth_range,
            draws_after_world,
            opaque_before_translucent,
            handedness,
        } => {
            output.u8(1)?;
            output.geometry_orientation(geometry)?;
            output.u8(entity_angles as u8)?;
            output.float(default_horizontal_fov_4_by_3)?;
            output.float(minimum_fov)?;
            output.float(maximum_fov)?;
            output.float(near_plane)?;
            output.u8(far_plane as u8)?;
            output.float_slice(&depth_range)?;
            output.bool(draws_after_world)?;
            output.bool(opaque_before_translucent)?;
            output.u8(handedness as u8)?;
        }
    }
    output.count(model.base_material_count)?;

    output.count(model.dependencies.len())?;
    for dependency in &model.dependencies {
        output.text(&dependency.requester, limits.max_string_bytes)?;
        output.u8(dependency_role_code(dependency.role))?;
        output.text(&dependency.logical_path, limits.max_string_bytes)?;
        output.option_index(dependency.material_slot)?;
        output.option_texture_role(dependency.texture_role)?;
        output.bool(dependency.sha256.is_some())?;
        if let Some(hash) = dependency.sha256 {
            output.raw(&hash)?;
        }
        output.u64(dependency.byte_length as u64)?;
    }
    output.count(model.materials.len())?;
    for material in &model.materials {
        output.index(material.slot)?;
        output.index(material.source_slot)?;
        output.option_index(material.lod)?;
        output.bytes(&material.authored_name, limits.max_string_bytes)?;
        output.index(material.material_dependency)?;
        output.indices(&material.include_dependencies)?;
        output.count(material.textures.len())?;
        for texture in &material.textures {
            output.u8(texture_role_code(texture.role))?;
            output.bytes(&texture.parameter, limits.max_string_bytes)?;
            output.option_index(texture.dependency)?;
            output.u8(match texture.disposition {
                TextureDisposition::Source => 0,
                TextureDisposition::BuiltInEnvironment => 1,
                TextureDisposition::BuiltInRenderTarget => 2,
            })?;
            output.bool(texture.selected)?;
        }
    }
    output.count(model.bones.len())?;
    for bone in &model.bones {
        output.index(bone.index)?;
        output.bytes(&bone.name, limits.max_string_bytes)?;
        output.i32(bone.parent)?;
        for value in bone.controllers {
            output.i32(value)?;
        }
        output.vector(bone.position)?;
        output.float4(bone.quaternion)?;
        output.vector(bone.rotation)?;
        output.vector(bone.position_scale)?;
        output.vector(bone.rotation_scale)?;
        output.float_slice(&bone.pose_to_bone)?;
        output.float4(bone.alignment)?;
        output.i32(bone.flags)?;
        output.i32(bone.procedural_type)?;
        output.i32(bone.procedural_offset)?;
        output.i32(bone.physics_bone)?;
        output.bytes(&bone.surface_property, limits.max_string_bytes)?;
        output.i32(bone.contents)?;
    }
    output.count(model.animations.len())?;
    for animation in &model.animations {
        output.index(animation.index)?;
        output.bytes(&animation.name, limits.max_string_bytes)?;
        output.float(animation.fps)?;
        output.i32(animation.flags)?;
        output.i32(animation.frame_count)?;
        output.i32(animation.movement_count)?;
        output.i32(animation.animation_block)?;
        output.i32(animation.animation_offset)?;
        output.i32(animation.ik_rule_count)?;
        output.i32(animation.local_hierarchy_count)?;
        output.i32(animation.section_offset)?;
        output.i32(animation.section_frame_count)?;
        output.u16(animation.zero_frame_count)?;
        output.text(&animation.source_identity, limits.max_string_bytes)?;
        output.count(animation.bone_map.len())?;
        for bone in &animation.bone_map {
            output.option_index(*bone)?;
        }
        output.count(animation.sections.len())?;
        for (section_index, section) in animation.sections.iter().enumerate() {
            output.index(section.index)?;
            output.index(section.first_frame)?;
            output.index(section.frame_count)?;
            output.i32(section.block)?;
            output.i32(section.data_offset)?;
            let authored_tracks = if animation.authored_frames.is_some() {
                Some(
                    animation
                        .authored_section_tracks(section_index)
                        .map_err(|_| {
                            presentation_error(
                                PresentationErrorCode::InvalidArtifact,
                                &model.identity,
                            )
                        })?,
                )
            } else {
                None
            };
            let tracks = authored_tracks.as_deref().unwrap_or(&section.tracks);
            output.count(tracks.len())?;
            for track in tracks {
                output.index(track.bone)?;
                output.u8(track.flags)?;
                output.u64(track.source_offset as u64)?;
                output.u16(track.next_offset)?;
                output.u8(rotation_codec_code(track.rotation_codec))?;
                output.u8(translation_codec_code(track.translation_codec))?;
                output.value_streams(&track.rotation_values)?;
                output.value_streams(&track.translation_values)?;
            }
        }
        if animation.authored_frames.is_some()
            && animation.frames.is_empty()
            && animation.compact_frames.is_empty()
        {
            let frame_count = usize::try_from(animation.frame_count).map_err(|_| {
                presentation_error(PresentationErrorCode::InvalidArtifact, &model.identity)
            })?;
            output.count(frame_count)?;
            for index in 0..frame_count {
                let frame = animation.authored_frame(index).map_err(|_| {
                    presentation_error(PresentationErrorCode::InvalidArtifact, &model.identity)
                })?;
                output.count(frame.translations.len())?;
                for translation in &frame.translations {
                    output.vector(*translation)?;
                }
                output.count(frame.rotations.len())?;
                for rotation in &frame.rotations {
                    output.float4(*rotation)?;
                }
            }
        } else if !animation.frames.is_empty() && animation.compact_frames.is_empty() {
            output.count(animation.frames.len())?;
            for frame in &animation.frames {
                output.count(frame.translations.len())?;
                for translation in &frame.translations {
                    output.vector(*translation)?;
                }
                output.count(frame.rotations.len())?;
                for rotation in &frame.rotations {
                    output.float4(*rotation)?;
                }
            }
        } else if animation.frames.is_empty() && !animation.compact_frames.is_empty() {
            output.u32(COMPACT_FRAME_MARKER)?;
            output.count(animation.compact_frames.len())?;
            output.raw(&animation.compact_frames)?;
        } else {
            return Err(presentation_error(
                PresentationErrorCode::InvalidArtifact,
                &model.identity,
            ));
        }
    }
    output.count(model.sequences.len())?;
    for sequence in &model.sequences {
        output.index(sequence.index)?;
        output.bytes(&sequence.label, limits.max_string_bytes)?;
        output.bytes(&sequence.activity_name, limits.max_string_bytes)?;
        output.i32(sequence.flags)?;
        output.i32(sequence.activity)?;
        output.i32(sequence.activity_weight)?;
        output.i32(sequence.event_count)?;
        output.vector(sequence.bounds_min)?;
        output.vector(sequence.bounds_max)?;
        output.i32(sequence.blend_count)?;
        output.i32(sequence.blend_size[0])?;
        output.i32(sequence.blend_size[1])?;
        output.count(sequence.animation_indices.len())?;
        for value in &sequence.animation_indices {
            output.i16(*value)?;
        }
        for value in sequence.pose_parameter_indices {
            output.i32(value)?;
        }
        output.float_slice(&sequence.pose_parameter_start)?;
        output.float_slice(&sequence.pose_parameter_end)?;
        output.float(sequence.fade_in)?;
        output.float(sequence.fade_out)?;
        output.i32(sequence.entry_node)?;
        output.i32(sequence.exit_node)?;
        output.i32(sequence.node_flags)?;
        output.float(sequence.entry_phase)?;
        output.float(sequence.exit_phase)?;
        output.float(sequence.last_frame)?;
        output.i32(sequence.next_sequence)?;
        output.i32(sequence.pose)?;
        output.i32(sequence.auto_layer_count)?;
        output.count(sequence.bone_weights.len())?;
        output.float_slice(&sequence.bone_weights)?;
        for keys in &sequence.pose_keys {
            output.count(keys.len())?;
            output.float_slice(keys)?;
        }
        output.i32(sequence.ik_lock_count)?;
        output.i32(sequence.cycle_pose_parameter)?;
        output.i32(sequence.activity_modifier_count)?;
        output.count(sequence.events.len())?;
        for event in &sequence.events {
            output.index(event.index)?;
            output.float(event.cycle)?;
            output.i32(event.event)?;
            output.i32(event.event_type)?;
            output.raw(&event.options)?;
            output.bytes(&event.name, limits.max_string_bytes)?;
        }
        output.count(sequence.auto_layers.len())?;
        for layer in &sequence.auto_layers {
            output.index(layer.index)?;
            output.i16(layer.sequence)?;
            output.i16(layer.pose)?;
            output.i32(layer.flags)?;
            output.float(layer.start)?;
            output.float(layer.peak)?;
            output.float(layer.tail)?;
            output.float(layer.end)?;
        }
        output.text(&sequence.source_identity, limits.max_string_bytes)?;
    }
    output.count(model.pose_parameters.len())?;
    for pose in &model.pose_parameters {
        output.index(pose.index)?;
        output.bytes(&pose.name, limits.max_string_bytes)?;
        output.i32(pose.flags)?;
        output.float(pose.start)?;
        output.float(pose.end)?;
        output.float(pose.looping_range)?;
        output.text(&pose.source_identity, limits.max_string_bytes)?;
    }
    output.count(model.attachments.len())?;
    for attachment in &model.attachments {
        output.index(attachment.index)?;
        output.bytes(&attachment.name, limits.max_string_bytes)?;
        output.u32(attachment.flags)?;
        output.i32(attachment.bone)?;
        output.float_slice(&attachment.local)?;
    }
    output.count(model.hitbox_sets.len())?;
    for set in &model.hitbox_sets {
        output.index(set.index)?;
        output.bytes(&set.name, limits.max_string_bytes)?;
        output.count(set.hitboxes.len())?;
        for hitbox in &set.hitboxes {
            output.index(hitbox.index)?;
            output.i32(hitbox.bone)?;
            output.i32(hitbox.group)?;
            output.vector(hitbox.bounds_min)?;
            output.vector(hitbox.bounds_max)?;
            output.i32(hitbox.name_offset)?;
            output.u8(u8::from(hitbox.name_resolved))?;
            output.bytes(&hitbox.name, limits.max_string_bytes)?;
        }
    }
    output.count(model.skins.len())?;
    for skin in &model.skins {
        output.index(skin.index)?;
        output.count(skin.texture_indices.len())?;
        for texture in &skin.texture_indices {
            output.u16(*texture)?;
        }
    }
    output.count(model.body_parts.len())?;
    for body_part in &model.body_parts {
        output.index(body_part.index)?;
        output.bytes(&body_part.name, limits.max_string_bytes)?;
        output.i32(body_part.base)?;
        output.count(body_part.model_names.len())?;
        for name in &body_part.model_names {
            output.bytes(name, limits.max_string_bytes)?;
        }
    }
    output.count(model.geometry.len())?;
    for primitive in &model.geometry {
        output.index(primitive.body_part)?;
        output.index(primitive.model)?;
        output.index(primitive.lod)?;
        output.index(primitive.mesh)?;
        output.index(primitive.strip_group)?;
        output.float(primitive.switch_point)?;
        output.index(primitive.material_slot)?;
        output.indices(&primitive.source_vertex_ids)?;
        output.count(primitive.vertices.len())?;
        for vertex in &primitive.vertices {
            output.index(vertex.source_index)?;
            output.float_slice(&vertex.weights)?;
            output.raw(&vertex.bones)?;
            output.u8(vertex.bone_count)?;
            output.vector(vertex.position)?;
            output.vector(vertex.normal)?;
            output.float_slice(&vertex.uv)?;
            output.float_slice(&vertex.tangent)?;
        }
        output.count(primitive.encoded_indices.len())?;
        for index in &primitive.encoded_indices {
            output.u16(*index)?;
        }
        output.count(primitive.strips.len())?;
        for strip in &primitive.strips {
            output.index(strip.index_count)?;
            output.index(strip.first_index)?;
            output.index(strip.vertex_count)?;
            output.index(strip.first_vertex)?;
            output.u8(strip.flags)?;
        }
        output.count(primitive.triangles.len())?;
        for triangle in &primitive.triangles {
            for index in triangle {
                output.u32(*index)?;
            }
        }
    }
    output.u8(match model.physics_status {
        crate::PhysicsStatus::Missing => 0,
        crate::PhysicsStatus::Present => 1,
    })?;
    output.count(model.features.len())?;
    for feature in &model.features {
        output.u8(feature_family_code(feature.family))?;
        output.u8(match feature.disposition {
            FeatureDisposition::NotPresent => 0,
            FeatureDisposition::RetainedNotEvaluated => 1,
            FeatureDisposition::Evaluated => 2,
        })?;
        output.index(feature.records)?;
    }
    let length = output.bytes.len() as u64;
    output.bytes[8..16].copy_from_slice(&length.to_le_bytes());
    Ok(output.bytes)
}

struct ArtifactWriter<'a> {
    bytes: Vec<u8>,
    max: usize,
    identity: &'a str,
}

impl<'a> ArtifactWriter<'a> {
    fn new(max: usize, identity: &'a str) -> Self {
        Self {
            bytes: Vec::new(),
            max,
            identity,
        }
    }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), PresentationError> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > self.max)
        {
            return Err(presentation_error(
                PresentationErrorCode::ArtifactLimit,
                self.identity,
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), PresentationError> {
        self.raw(&[value])
    }

    fn bool(&mut self, value: bool) -> Result<(), PresentationError> {
        self.u8(u8::from(value))
    }

    fn u16(&mut self, value: u16) -> Result<(), PresentationError> {
        self.raw(&value.to_le_bytes())
    }

    fn i16(&mut self, value: i16) -> Result<(), PresentationError> {
        self.raw(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), PresentationError> {
        self.raw(&value.to_le_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), PresentationError> {
        self.raw(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), PresentationError> {
        self.raw(&value.to_le_bytes())
    }

    fn count(&mut self, value: usize) -> Result<(), PresentationError> {
        self.u32(
            u32::try_from(value).map_err(|_| {
                presentation_error(PresentationErrorCode::ArtifactLimit, self.identity)
            })?,
        )
    }

    fn index(&mut self, value: usize) -> Result<(), PresentationError> {
        self.count(value)
    }

    fn option_index(&mut self, value: Option<usize>) -> Result<(), PresentationError> {
        match value {
            Some(value) => {
                self.bool(true)?;
                self.index(value)
            }
            None => self.bool(false),
        }
    }

    fn bytes(&mut self, value: &[u8], max: usize) -> Result<(), PresentationError> {
        if value.len() > max {
            return Err(presentation_error(
                PresentationErrorCode::ModelLimit,
                self.identity,
            ));
        }
        self.count(value.len())?;
        self.raw(value)
    }

    fn text(&mut self, value: &str, max: usize) -> Result<(), PresentationError> {
        self.bytes(value.as_bytes(), max)
    }

    fn float(&mut self, value: Float32) -> Result<(), PresentationError> {
        self.u32(value.0)
    }

    fn float4(&mut self, value: [Float32; 4]) -> Result<(), PresentationError> {
        self.float_slice(&value)
    }

    fn float_slice(&mut self, values: &[Float32]) -> Result<(), PresentationError> {
        for value in values {
            self.float(*value)?;
        }
        Ok(())
    }

    fn vector(&mut self, value: Vector3) -> Result<(), PresentationError> {
        self.float_slice(&value.0)
    }

    fn indices(&mut self, values: &[usize]) -> Result<(), PresentationError> {
        self.count(values.len())?;
        for value in values {
            self.index(*value)?;
        }
        Ok(())
    }

    fn option_texture_role(&mut self, role: Option<TextureRole>) -> Result<(), PresentationError> {
        match role {
            Some(role) => {
                self.bool(true)?;
                self.u8(texture_role_code(role))
            }
            None => self.bool(false),
        }
    }

    fn geometry_orientation(
        &mut self,
        orientation: GeometryOrientation,
    ) -> Result<(), PresentationError> {
        self.u8(orientation.positions as u8)?;
        self.u8(orientation.normals as u8)?;
        self.u8(orientation.tangents as u8)?;
        self.u8(orientation.texture_coordinates as u8)?;
        self.u8(orientation.tangent_handedness as u8)?;
        self.u8(orientation.deformation as u8)?;
        self.u8(orientation.facing.front_face as u8)?;
        self.u8(orientation.facing.cull_face as u8)
    }

    fn value_streams(
        &mut self,
        streams: &[Option<crate::AnimationValueStream>; 3],
    ) -> Result<(), PresentationError> {
        for stream in streams {
            self.bool(stream.is_some())?;
            let Some(stream) = stream else { continue };
            self.i16(stream.relative_offset)?;
            self.count(stream.runs.len())?;
            for run in &stream.runs {
                self.u8(run.valid)?;
                self.u8(run.total)?;
                self.count(run.values.len())?;
                for value in &run.values {
                    self.i16(*value)?;
                }
            }
        }
        Ok(())
    }
}

fn dependency_role_code(role: PresentationDependencyRole) -> u8 {
    match role {
        PresentationDependencyRole::RootModel => 0,
        PresentationDependencyRole::VertexData => 1,
        PresentationDependencyRole::Topology => 2,
        PresentationDependencyRole::AnimationBlocks => 3,
        PresentationDependencyRole::IncludeModel => 4,
        PresentationDependencyRole::Physics => 5,
        PresentationDependencyRole::MaterialCandidate => 6,
        PresentationDependencyRole::MaterialInclude => 7,
        PresentationDependencyRole::Texture => 8,
    }
}

fn texture_role_code(role: TextureRole) -> u8 {
    role as u8
}

fn rotation_codec_code(codec: crate::RotationCodec) -> u8 {
    codec as u8
}

fn translation_codec_code(codec: crate::TranslationCodec) -> u8 {
    codec as u8
}

fn feature_family_code(family: FeatureFamily) -> u8 {
    family as u8
}

pub fn content_sha256(bytes: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut hash = INITIAL;
    let compress = |hash: &mut [u32; 8], block: &[u8]| {
        let mut schedule = [0_u32; 64];
        for (index, value) in schedule[..16].iter_mut().enumerate() {
            *value = u32::from_be_bytes(
                block[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("SHA-256 block word"),
            );
        }
        for index in 16..64 {
            let small0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let small1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(small0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(small1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *hash;
        for index in 0..64 {
            let big1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(big1)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(schedule[index]);
            let big0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = big0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }
        for (value, next) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *value = value.wrapping_add(next);
        }
    };
    let complete_bytes = bytes.len() / 64 * 64;
    for block in bytes[..complete_bytes].chunks_exact(64) {
        compress(&mut hash, block);
    }
    let remaining = &bytes[complete_bytes..];
    let mut final_blocks = [0_u8; 128];
    final_blocks[..remaining.len()].copy_from_slice(remaining);
    final_blocks[remaining.len()] = 0x80;
    let final_length = if remaining.len() < 56 { 64 } else { 128 };
    final_blocks[final_length - 8..final_length]
        .copy_from_slice(&(bytes.len() as u64).wrapping_mul(8).to_be_bytes());
    for block in final_blocks[..final_length].chunks_exact(64) {
        compress(&mut hash, block);
    }
    let mut output = [0_u8; 32];
    for (index, value) in hash.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }
    output
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    content_sha256(bytes)
}

pub fn decode_presentation(
    bytes: &[u8],
    limits: PresentationLimits,
) -> Result<PresentationArtifact, PresentationError> {
    validate_limits(limits)?;
    if bytes.len() >= MAX_MESSAGE_BYTES || bytes.len() > limits.max_artifact_bytes {
        return Err(presentation_error(
            PresentationErrorCode::ArtifactLimit,
            "presentation-artifact",
        ));
    }
    let mut input = ArtifactReader::new(bytes);
    if input.raw(4)? != ARTIFACT_MAGIC {
        return Err(input.error());
    }
    if input.u16()? != ARTIFACT_VERSION {
        return Err(input.error());
    }
    let profile = match input.u8()? {
        0 => PresentationProfile::World,
        1 => PresentationProfile::ViewModel,
        _ => return Err(input.error()),
    };
    if input.u8()? != 0 || input.u64()? != bytes.len() as u64 {
        return Err(input.error());
    }
    let checksum = input.i32()?;
    let flags = input.i32()?;
    let identity = input.text(limits.max_string_bytes)?;
    let basis = ModelBasis {
        forward: input.vector()?,
        left: input.vector()?,
        up: input.vector()?,
    };
    let collision_bounds = [input.vector()?, input.vector()?];
    let geometry =
        |input: &mut ArtifactReader<'_>| -> Result<GeometryOrientation, PresentationError> {
            if input.u8()? != 0
                || input.u8()? != 0
                || input.u8()? != 0
                || input.u8()? != 0
                || input.u8()? != 0
                || input.u8()? != 0
                || input.u8()? != 0
                || input.u8()? != 0
            {
                return Err(input.error());
            }
            Ok(GeometryOrientation::source())
        };
    let descriptor = match input.u8()? {
        0 if profile == PresentationProfile::World => {
            let geometry = geometry(&mut input)?;
            if input.u8()? != 0 {
                return Err(input.error());
            }
            let root_bone = match input.u8()? {
                0 => RootBoneContract::AnimatedBelowEntity,
                1 => RootBoneContract::StaticPropBoneZeroIsEntity,
                _ => return Err(input.error()),
            };
            PresentationDescriptor::World {
                geometry,
                entity_angles: EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns,
                root_bone,
                depth_range: input.floats()?,
            }
        }
        1 if profile == PresentationProfile::ViewModel => {
            let geometry = geometry(&mut input)?;
            if input.u8()? != 0 {
                return Err(input.error());
            }
            let default_horizontal_fov_4_by_3 = input.float()?;
            let minimum_fov = input.float()?;
            let maximum_fov = input.float()?;
            let near_plane = input.float()?;
            if input.u8()? != 0 {
                return Err(input.error());
            }
            let depth_range = input.floats()?;
            let draws_after_world = input.bool()?;
            let opaque_before_translucent = input.bool()?;
            if input.u8()? != 0 {
                return Err(input.error());
            }
            PresentationDescriptor::ViewModel {
                geometry,
                entity_angles: EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns,
                default_horizontal_fov_4_by_3,
                minimum_fov,
                maximum_fov,
                near_plane,
                far_plane: FarPlaneContract::SuppliedWorldFarPlane,
                depth_range,
                draws_after_world,
                opaque_before_translucent,
                handedness: ViewmodelHandednessContract::OptionalViewSpaceYReflection,
            }
        }
        _ => return Err(input.error()),
    };
    let base_material_count = input.count(limits.max_materials)?;
    let mut dependencies = Vec::new();
    for _ in 0..input.count(limits.max_dependencies)? {
        dependencies.push(ArtifactDependency {
            requester: input.text(limits.max_string_bytes)?,
            role: decode_dependency_role(input.u8()?).ok_or_else(|| input.error())?,
            logical_path: input.text(limits.max_string_bytes)?,
            material_slot: input.option_index(limits.max_materials)?,
            texture_role: input.option_texture_role()?,
            sha256: input.bool()?.then(|| input.array()).transpose()?,
            byte_length: input.usize()?,
        });
    }
    let mut materials = Vec::new();
    for _ in 0..input.count(limits.max_materials)? {
        let slot = input.index(limits.max_materials)?;
        let source_slot = input.index(limits.max_materials)?;
        let lod = input.option_index(8)?;
        let authored_name = input.bytes(limits.max_string_bytes)?;
        let material_dependency = input.index(limits.max_dependencies)?;
        let include_dependencies =
            input.indices(limits.max_dependencies, limits.max_dependencies)?;
        let mut textures = Vec::new();
        for _ in 0..input.count(limits.max_textures_per_material)? {
            let role = decode_texture_role(input.u8()?).ok_or_else(|| input.error())?;
            let parameter = input.bytes(limits.max_string_bytes)?;
            let dependency = input.option_index(limits.max_dependencies)?;
            let disposition = match input.u8()? {
                0 => TextureDisposition::Source,
                1 => TextureDisposition::BuiltInEnvironment,
                2 => TextureDisposition::BuiltInRenderTarget,
                _ => return Err(input.error()),
            };
            let selected = input.bool()?;
            textures.push(PresentationTexture {
                role,
                parameter,
                dependency,
                disposition,
                selected,
            });
        }
        materials.push(PresentationMaterial {
            slot,
            source_slot,
            lod,
            authored_name,
            material_dependency,
            include_dependencies,
            textures,
        });
    }
    let bone_count = input.count(limits.max_bones)?;
    let mut bones = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        let index = input.index(limits.max_bones)?;
        let name = input.bytes(limits.max_string_bytes)?;
        let parent = input.i32()?;
        let mut controllers = [0; 6];
        for controller in &mut controllers {
            *controller = input.i32()?;
        }
        bones.push(crate::Bone {
            index,
            name,
            parent,
            controllers,
            position: input.vector()?,
            quaternion: input.floats()?,
            rotation: input.vector()?,
            position_scale: input.vector()?,
            rotation_scale: input.vector()?,
            pose_to_bone: input.floats()?,
            alignment: input.floats()?,
            flags: input.i32()?,
            procedural_type: input.i32()?,
            procedural_offset: input.i32()?,
            physics_bone: input.i32()?,
            surface_property: input.bytes(limits.max_string_bytes)?,
            contents: input.i32()?,
        });
    }
    let mut sample_count = 0_usize;
    let mut animations = Vec::new();
    for _ in 0..input.count(limits.max_animations)? {
        let index = input.index(limits.max_animations)?;
        let name = input.bytes(limits.max_string_bytes)?;
        let fps = input.float()?;
        let flags = input.i32()?;
        let frame_count = input.i32()?;
        let movement_count = input.i32()?;
        let animation_block = input.i32()?;
        let animation_offset = input.i32()?;
        let ik_rule_count = input.i32()?;
        let local_hierarchy_count = input.i32()?;
        let section_offset = input.i32()?;
        let section_frame_count = input.i32()?;
        let zero_frame_count = input.u16()?;
        let source_identity = input.text(limits.max_string_bytes)?;
        let mut bone_map = Vec::new();
        for _ in 0..input.count(limits.max_bones)? {
            bone_map.push(input.option_index(limits.max_bones)?);
        }
        let mut sections = Vec::new();
        for _ in 0..input.count(limits.max_animation_samples)? {
            let section_index = input.index(limits.max_animation_samples)?;
            let first_frame = input.index(limits.max_animation_samples)?;
            let section_frames = input.index(limits.max_animation_samples)?;
            let block = input.i32()?;
            let data_offset = input.i32()?;
            let mut tracks = Vec::new();
            for _ in 0..input.count(limits.max_bones)? {
                tracks.push(crate::AnimationTrack {
                    bone: input.index(limits.max_bones)?,
                    flags: input.u8()?,
                    source_offset: input.usize()?,
                    next_offset: input.u16()?,
                    rotation_codec: decode_rotation_codec(input.u8()?)
                        .ok_or_else(|| input.error())?,
                    translation_codec: decode_translation_codec(input.u8()?)
                        .ok_or_else(|| input.error())?,
                    rotation_values: input.value_streams(limits.max_animation_samples)?,
                    translation_values: input.value_streams(limits.max_animation_samples)?,
                });
            }
            sections.push(crate::AnimationSection {
                index: section_index,
                first_frame,
                frame_count: section_frames,
                block,
                data_offset,
                tracks,
            });
        }
        let mut frames = Vec::new();
        let mut compact_frames = Vec::new();
        let encoded_frame_field = input.u32()?;
        if encoded_frame_field != COMPACT_FRAME_MARKER {
            let encoded_frames = encoded_frame_field as usize;
            if encoded_frames > limits.max_animation_samples {
                return Err(input.error());
            }
            frames.reserve(encoded_frames);
            for _ in 0..encoded_frames {
                let translation_count = input.count(limits.max_bones)?;
                sample_count = sample_count
                    .checked_add(translation_count)
                    .filter(|value| *value <= limits.max_animation_samples)
                    .ok_or_else(|| input.error())?;
                let mut translations = Vec::with_capacity(translation_count);
                for _ in 0..translation_count {
                    translations.push(input.vector()?);
                }
                let rotation_count = input.count(limits.max_bones)?;
                let mut rotations = Vec::with_capacity(rotation_count);
                for _ in 0..rotation_count {
                    rotations.push(input.floats()?);
                }
                frames.push(crate::AnimationFrame {
                    translations,
                    rotations,
                });
            }
        } else {
            compact_frames = input.bytes(limits.max_artifact_bytes)?;
            sample_count = sample_count
                .checked_add(
                    usize::try_from(frame_count)
                        .ok()
                        .and_then(|frames| frames.checked_mul(bone_map.len()))
                        .ok_or_else(|| input.error())?,
                )
                .filter(|value| *value <= limits.max_animation_samples)
                .ok_or_else(|| input.error())?;
        }
        animations.push(Animation {
            index,
            name,
            fps,
            flags,
            frame_count,
            movement_count,
            animation_block,
            animation_offset,
            ik_rule_count,
            local_hierarchy_count,
            section_offset,
            section_frame_count,
            zero_frame_count,
            source_identity,
            bone_map,
            sections,
            frames,
            compact_frames,
            authored_frames: None,
        });
    }
    let mut sequences = Vec::new();
    for _ in 0..input.count(limits.max_sequences)? {
        let index = input.index(limits.max_sequences)?;
        let label = input.bytes(limits.max_string_bytes)?;
        let activity_name = input.bytes(limits.max_string_bytes)?;
        let flags = input.i32()?;
        let activity = input.i32()?;
        let activity_weight = input.i32()?;
        let event_count = input.i32()?;
        let bounds_min = input.vector()?;
        let bounds_max = input.vector()?;
        let blend_count = input.i32()?;
        let blend_size = [input.i32()?, input.i32()?];
        let animation_index_count = input.count(limits.max_animations)?;
        let mut animation_indices = Vec::with_capacity(animation_index_count);
        for _ in 0..animation_index_count {
            animation_indices.push(input.i16()?);
        }
        let pose_parameter_indices = [input.i32()?, input.i32()?];
        let pose_parameter_start = input.floats()?;
        let pose_parameter_end = input.floats()?;
        let fade_in = input.float()?;
        let fade_out = input.float()?;
        let entry_node = input.i32()?;
        let exit_node = input.i32()?;
        let node_flags = input.i32()?;
        let entry_phase = input.float()?;
        let exit_phase = input.float()?;
        let last_frame = input.float()?;
        let next_sequence = input.i32()?;
        let pose = input.i32()?;
        let auto_layer_count = input.i32()?;
        let bone_weight_count = input.count(limits.max_bones)?;
        let mut bone_weights = Vec::with_capacity(bone_weight_count);
        for _ in 0..bone_weight_count {
            bone_weights.push(input.float()?);
        }
        let mut pose_keys = [Vec::new(), Vec::new()];
        for keys in &mut pose_keys {
            for _ in 0..input.count(limits.max_animation_samples)? {
                keys.push(input.float()?);
            }
        }
        let ik_lock_count = input.i32()?;
        let cycle_pose_parameter = input.i32()?;
        let activity_modifier_count = input.i32()?;
        let mut events = Vec::new();
        for _ in 0..input.count(limits.max_animation_samples)? {
            events.push(crate::SequenceEvent {
                index: input.index(limits.max_animation_samples)?,
                cycle: input.float()?,
                event: input.i32()?,
                event_type: input.i32()?,
                options: input.array()?,
                name: input.bytes(limits.max_string_bytes)?,
            });
        }
        let mut auto_layers = Vec::new();
        for _ in 0..input.count(limits.max_sequences)? {
            auto_layers.push(crate::SequenceAutoLayer {
                index: input.index(limits.max_sequences)?,
                sequence: input.i16()?,
                pose: input.i16()?,
                flags: input.i32()?,
                start: input.float()?,
                peak: input.float()?,
                tail: input.float()?,
                end: input.float()?,
            });
        }
        let source_identity = input.text(limits.max_string_bytes)?;
        sequences.push(crate::Sequence {
            index,
            label,
            activity_name,
            flags,
            activity,
            activity_weight,
            event_count,
            bounds_min,
            bounds_max,
            blend_count,
            blend_size,
            animation_indices,
            pose_parameter_indices,
            pose_parameter_start,
            pose_parameter_end,
            fade_in,
            fade_out,
            entry_node,
            exit_node,
            node_flags,
            entry_phase,
            exit_phase,
            last_frame,
            next_sequence,
            pose,
            auto_layer_count,
            bone_weights,
            pose_keys,
            ik_lock_count,
            cycle_pose_parameter,
            activity_modifier_count,
            events,
            auto_layers,
            source_identity,
        });
    }
    let mut pose_parameters = Vec::new();
    for _ in 0..input.count(24)? {
        pose_parameters.push(crate::PoseParameter {
            index: input.index(24)?,
            name: input.bytes(limits.max_string_bytes)?,
            flags: input.i32()?,
            start: input.float()?,
            end: input.float()?,
            looping_range: input.float()?,
            source_identity: input.text(limits.max_string_bytes)?,
        });
    }
    let mut attachments = Vec::new();
    for _ in 0..input.count(limits.max_sequences)? {
        attachments.push(Attachment {
            index: input.index(limits.max_sequences)?,
            name: input.bytes(limits.max_string_bytes)?,
            flags: input.u32()?,
            bone: input.i32()?,
            local: input.floats()?,
        });
    }
    let mut hitbox_sets = Vec::new();
    for _ in 0..input.count(limits.max_sequences)? {
        let index = input.index(limits.max_sequences)?;
        let name = input.bytes(limits.max_string_bytes)?;
        let mut hitboxes = Vec::new();
        for _ in 0..input.count(limits.max_animation_samples)? {
            hitboxes.push(crate::Hitbox {
                index: input.index(limits.max_animation_samples)?,
                bone: input.i32()?,
                group: input.i32()?,
                bounds_min: input.vector()?,
                bounds_max: input.vector()?,
                name_offset: input.i32()?,
                name_resolved: match input.u8()? {
                    0 => false,
                    1 => true,
                    _ => {
                        return Err(presentation_error(
                            PresentationErrorCode::InvalidArtifact,
                            "hitbox name disposition",
                        ));
                    }
                },
                name: input.bytes(limits.max_string_bytes)?,
            });
        }
        hitbox_sets.push(crate::HitboxSet {
            index,
            name,
            hitboxes,
        });
    }
    let mut skins = Vec::new();
    for _ in 0..input.count(32)? {
        let index = input.index(32)?;
        let count = input.count(limits.max_materials)?;
        let mut texture_indices = Vec::with_capacity(count);
        for _ in 0..count {
            texture_indices.push(input.u16()?);
        }
        skins.push(SkinFamily {
            index,
            texture_indices,
        });
    }
    let mut body_parts = Vec::new();
    for _ in 0..input.count(limits.max_materials)? {
        let index = input.index(limits.max_materials)?;
        let name = input.bytes(limits.max_string_bytes)?;
        let base = input.i32()?;
        let mut model_names = Vec::new();
        for _ in 0..input.count(limits.max_sequences)? {
            model_names.push(input.bytes(limits.max_string_bytes)?);
        }
        body_parts.push(PresentationBodyPart {
            index,
            name,
            base,
            model_names,
        });
    }
    let mut total_vertices = 0_usize;
    let mut total_indices = 0_usize;
    let mut geometry = Vec::new();
    for _ in 0..input.count(limits.max_geometry_indices)? {
        let body_part = input.index(limits.max_materials)?;
        let model_index = input.index(limits.max_sequences)?;
        let lod = input.index(8)?;
        let mesh = input.index(limits.max_geometry_indices)?;
        let strip_group = input.index(limits.max_geometry_indices)?;
        let switch_point = input.float()?;
        let material_slot = input.index(limits.max_materials)?;
        let source_vertex_ids =
            input.indices(limits.max_geometry_vertices, limits.max_geometry_vertices)?;
        let vertex_count = input.count(limits.max_geometry_vertices)?;
        total_vertices = total_vertices
            .checked_add(vertex_count)
            .filter(|value| *value <= limits.max_geometry_vertices)
            .ok_or_else(|| input.error())?;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(crate::Vertex {
                source_index: input.index(limits.max_geometry_vertices)?,
                weights: input.floats()?,
                bones: input.array()?,
                bone_count: input.u8()?,
                position: input.vector()?,
                normal: input.vector()?,
                uv: input.floats()?,
                tangent: input.floats()?,
            });
        }
        let index_count = input.count(limits.max_geometry_indices)?;
        total_indices = total_indices
            .checked_add(index_count)
            .filter(|value| *value <= limits.max_geometry_indices)
            .ok_or_else(|| input.error())?;
        let mut encoded_indices = Vec::with_capacity(index_count);
        for _ in 0..index_count {
            encoded_indices.push(input.u16()?);
        }
        let mut strips = Vec::new();
        for _ in 0..input.count(limits.max_geometry_indices)? {
            strips.push(crate::Strip {
                index_count: input.index(limits.max_geometry_indices)?,
                first_index: input.index(limits.max_geometry_indices)?,
                vertex_count: input.index(limits.max_geometry_vertices)?,
                first_vertex: input.index(limits.max_geometry_vertices)?,
                flags: input.u8()?,
            });
        }
        let mut triangles = Vec::new();
        for _ in 0..input.count(limits.max_geometry_indices)? {
            triangles.push([input.u32()?, input.u32()?, input.u32()?]);
        }
        geometry.push(GeometryPrimitive {
            body_part,
            model: model_index,
            lod,
            mesh,
            strip_group,
            switch_point,
            material_slot,
            source_vertex_ids,
            vertices,
            encoded_indices,
            strips,
            triangles,
        });
    }
    let physics_status = match input.u8()? {
        0 => crate::PhysicsStatus::Missing,
        1 => crate::PhysicsStatus::Present,
        _ => return Err(input.error()),
    };
    let mut features = Vec::new();
    for _ in 0..input.count(9)? {
        features.push(FeatureSupport {
            family: decode_feature_family(input.u8()?).ok_or_else(|| input.error())?,
            disposition: match input.u8()? {
                0 => FeatureDisposition::NotPresent,
                1 => FeatureDisposition::RetainedNotEvaluated,
                2 => FeatureDisposition::Evaluated,
                _ => return Err(input.error()),
            },
            records: input.count(limits.max_animation_samples)?,
        });
    }
    if !input.finished() {
        return Err(input.error());
    }
    let model = PresentationModel {
        profile,
        descriptor,
        identity: identity.clone(),
        checksum,
        flags,
        basis,
        collision_bounds,
        dependencies,
        base_material_count,
        materials,
        bones,
        animations,
        sequences,
        pose_parameters,
        attachments,
        hitbox_sets,
        skins,
        body_parts,
        geometry,
        physics_status,
        features,
    };
    validate_decoded_model(&model, limits)?;
    let canonical = encode_model(&model, limits)?;
    if canonical != bytes {
        return Err(presentation_error(
            PresentationErrorCode::InvalidArtifact,
            &identity,
        ));
    }
    Ok(PresentationArtifact {
        model,
        bytes: canonical,
        sha256: sha256(bytes),
    })
}

struct ArtifactReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> ArtifactReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn error(&self) -> PresentationError {
        presentation_error(
            PresentationErrorCode::InvalidArtifact,
            "presentation-artifact",
        )
    }

    fn raw(&mut self, length: usize) -> Result<&'a [u8], PresentationError> {
        let end = self
            .cursor
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| self.error())?;
        let output = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(output)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PresentationError> {
        self.raw(N)?.try_into().map_err(|_| self.error())
    }

    fn u8(&mut self) -> Result<u8, PresentationError> {
        Ok(self.raw(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, PresentationError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.error()),
        }
    }

    fn u16(&mut self) -> Result<u16, PresentationError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn i16(&mut self) -> Result<i16, PresentationError> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, PresentationError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, PresentationError> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, PresentationError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn usize(&mut self) -> Result<usize, PresentationError> {
        usize::try_from(self.u64()?).map_err(|_| self.error())
    }

    fn count(&mut self, max: usize) -> Result<usize, PresentationError> {
        let value = self.u32()? as usize;
        (value <= max).then_some(value).ok_or_else(|| self.error())
    }

    fn index(&mut self, max: usize) -> Result<usize, PresentationError> {
        let value = self.u32()? as usize;
        (value < max).then_some(value).ok_or_else(|| self.error())
    }

    fn option_index(&mut self, max: usize) -> Result<Option<usize>, PresentationError> {
        self.bool()?.then(|| self.index(max)).transpose()
    }

    fn bytes(&mut self, max: usize) -> Result<Vec<u8>, PresentationError> {
        let length = self.count(max)?;
        Ok(self.raw(length)?.to_vec())
    }

    fn text(&mut self, max: usize) -> Result<String, PresentationError> {
        String::from_utf8(self.bytes(max)?).map_err(|_| self.error())
    }

    fn float(&mut self) -> Result<Float32, PresentationError> {
        Ok(Float32(self.u32()?))
    }

    fn floats<const N: usize>(&mut self) -> Result<[Float32; N], PresentationError> {
        let mut output = [Float32(0); N];
        for value in &mut output {
            *value = self.float()?;
        }
        Ok(output)
    }

    fn vector(&mut self) -> Result<Vector3, PresentationError> {
        Ok(Vector3(self.floats()?))
    }

    fn indices(
        &mut self,
        max_count: usize,
        max_index: usize,
    ) -> Result<Vec<usize>, PresentationError> {
        let count = self.count(max_count)?;
        let mut output = Vec::with_capacity(count);
        for _ in 0..count {
            output.push(self.index(max_index)?);
        }
        Ok(output)
    }

    fn option_texture_role(&mut self) -> Result<Option<TextureRole>, PresentationError> {
        if self.bool()? {
            decode_texture_role(self.u8()?)
                .map(Some)
                .ok_or_else(|| self.error())
        } else {
            Ok(None)
        }
    }

    fn value_streams(
        &mut self,
        max: usize,
    ) -> Result<[Option<crate::AnimationValueStream>; 3], PresentationError> {
        let mut streams = std::array::from_fn(|_| None);
        for stream in &mut streams {
            if !self.bool()? {
                continue;
            }
            let relative_offset = self.i16()?;
            let mut runs = Vec::new();
            for _ in 0..self.count(max)? {
                let valid = self.u8()?;
                let total = self.u8()?;
                let count = self.count(max)?;
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(self.i16()?);
                }
                if valid == 0 || total == 0 || valid > total || count != valid as usize {
                    return Err(self.error());
                }
                runs.push(crate::AnimationValueRun {
                    valid,
                    total,
                    values,
                });
            }
            *stream = Some(crate::AnimationValueStream {
                relative_offset,
                runs,
            });
        }
        Ok(streams)
    }

    fn finished(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

fn decode_dependency_role(value: u8) -> Option<PresentationDependencyRole> {
    match value {
        0 => Some(PresentationDependencyRole::RootModel),
        1 => Some(PresentationDependencyRole::VertexData),
        2 => Some(PresentationDependencyRole::Topology),
        3 => Some(PresentationDependencyRole::AnimationBlocks),
        4 => Some(PresentationDependencyRole::IncludeModel),
        5 => Some(PresentationDependencyRole::Physics),
        6 => Some(PresentationDependencyRole::MaterialCandidate),
        7 => Some(PresentationDependencyRole::MaterialInclude),
        8 => Some(PresentationDependencyRole::Texture),
        _ => None,
    }
}

fn decode_texture_role(value: u8) -> Option<TextureRole> {
    Some(match value {
        0 => TextureRole::Base,
        1 => TextureRole::HdrBase,
        2 => TextureRole::HdrCompressed,
        3 => TextureRole::HdrCompressed0,
        4 => TextureRole::HdrCompressed1,
        5 => TextureRole::HdrCompressed2,
        6 => TextureRole::Base2,
        7 => TextureRole::Bump,
        8 => TextureRole::Normal,
        9 => TextureRole::Bump2,
        10 => TextureRole::Detail,
        11 => TextureRole::BlendModulate,
        12 => TextureRole::Environment,
        13 => TextureRole::EnvironmentMask,
        14 => TextureRole::SelfIllumMask,
        15 => TextureRole::Flow,
        _ => return None,
    })
}

fn decode_rotation_codec(value: u8) -> Option<crate::RotationCodec> {
    Some(match value {
        0 => crate::RotationCodec::Bind,
        1 => crate::RotationCodec::DeltaIdentity,
        2 => crate::RotationCodec::Quaternion48,
        3 => crate::RotationCodec::Quaternion64,
        4 => crate::RotationCodec::RleEuler,
        _ => return None,
    })
}

fn decode_translation_codec(value: u8) -> Option<crate::TranslationCodec> {
    Some(match value {
        0 => crate::TranslationCodec::Bind,
        1 => crate::TranslationCodec::DeltaZero,
        2 => crate::TranslationCodec::Vector48,
        3 => crate::TranslationCodec::RleVector,
        _ => return None,
    })
}

fn decode_feature_family(value: u8) -> Option<FeatureFamily> {
    Some(match value {
        0 => FeatureFamily::ProceduralAxisInterpolation,
        1 => FeatureFamily::ProceduralQuaternionInterpolation,
        2 => FeatureFamily::ProceduralJiggle,
        3 => FeatureFamily::ProceduralAimAtBone,
        4 => FeatureFamily::ProceduralAimAtAttachment,
        5 => FeatureFamily::InverseKinematics,
        6 => FeatureFamily::Flex,
        7 => FeatureFamily::SequenceAutoLayers,
        8 => FeatureFamily::UnknownProcedural,
        _ => return None,
    })
}

fn validate_decoded_model(
    model: &PresentationModel,
    limits: PresentationLimits,
) -> Result<(), PresentationError> {
    let invalid = || presentation_error(PresentationErrorCode::InvalidArtifact, &model.identity);
    if !model.identity.starts_with("models/")
        || !model.identity.ends_with(".mdl")
        || model.identity.contains('\\')
        || model.basis != ModelBasis::source()
        || model.descriptor != presentation_descriptor(model.profile, model.flags)
        || model.dependencies.len() > limits.max_dependencies
        || model.base_material_count > model.materials.len()
        || model.materials.len() > limits.max_materials
        || model.bones.len() > limits.max_bones
        || model.animations.len() > limits.max_animations
        || model.sequences.len() > limits.max_sequences
        || model.features.len() != 9
    {
        return Err(invalid());
    }
    let mut aggregate_dependency_bytes = 0_usize;
    let mut unique_dependencies = BTreeSet::new();
    for (index, dependency) in model.dependencies.iter().enumerate() {
        let _ = index;
        let (prefix, suffix, material_owned) = match dependency.role {
            PresentationDependencyRole::RootModel | PresentationDependencyRole::IncludeModel => {
                ("models/", ".mdl", false)
            }
            PresentationDependencyRole::VertexData => ("models/", ".vvd", false),
            PresentationDependencyRole::Topology => ("models/", ".vtx", false),
            PresentationDependencyRole::AnimationBlocks => ("models/", ".ani", false),
            PresentationDependencyRole::Physics => ("models/", ".phy", false),
            PresentationDependencyRole::MaterialCandidate
            | PresentationDependencyRole::MaterialInclude => ("materials/", ".vmt", true),
            PresentationDependencyRole::Texture => ("materials/", ".vtf", true),
        };
        let path_valid = dependency.logical_path.starts_with(prefix)
            && dependency.logical_path.ends_with(suffix)
            && !dependency.logical_path.contains('\\')
            && !dependency
                .logical_path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..");
        if !path_valid
            || material_owned
                != dependency
                    .material_slot
                    .is_some_and(|slot| slot < model.materials.len())
            || (dependency.role == PresentationDependencyRole::Texture)
                != dependency.texture_role.is_some()
            || dependency.role != PresentationDependencyRole::Physics && dependency.sha256.is_none()
            || dependency.sha256.is_none() && dependency.byte_length != 0
            || dependency.byte_length > limits.max_dependency_bytes
        {
            return Err(invalid());
        }
        if unique_dependencies.insert((
            dependency.logical_path.as_str(),
            dependency.sha256,
            dependency.byte_length,
        )) {
            aggregate_dependency_bytes = aggregate_dependency_bytes
                .checked_add(dependency.byte_length)
                .filter(|bytes| *bytes <= limits.max_aggregate_dependency_bytes)
                .ok_or_else(invalid)?;
        }
    }
    for (index, material) in model.materials.iter().enumerate() {
        if material.slot != index
            || material.source_slot >= model.base_material_count
            || material.lod.is_some_and(|lod| lod >= 8)
            || model.materials[..index].iter().any(|existing| {
                existing.source_slot == material.source_slot && existing.lod == material.lod
            })
            || model
                .dependencies
                .get(material.material_dependency)
                .is_none_or(|dependency| {
                    dependency.role != PresentationDependencyRole::MaterialCandidate
                        || dependency.material_slot != Some(index)
                })
            || material.include_dependencies.iter().any(|&dependency| {
                model.dependencies.get(dependency).is_none_or(|dependency| {
                    dependency.role != PresentationDependencyRole::MaterialInclude
                        || dependency.material_slot != Some(index)
                })
            })
            || material.textures.len() > limits.max_textures_per_material
        {
            return Err(invalid());
        }
        for texture in &material.textures {
            match (texture.disposition, texture.dependency) {
                (TextureDisposition::Source, Some(dependency))
                    if model
                        .dependencies
                        .get(dependency)
                        .is_some_and(|dependency| {
                            dependency.role == PresentationDependencyRole::Texture
                                && dependency.material_slot == Some(index)
                                && dependency.texture_role == Some(texture.role)
                        }) => {}
                (TextureDisposition::BuiltInEnvironment, None)
                | (TextureDisposition::BuiltInRenderTarget, None) => {}
                _ => return Err(invalid()),
            }
        }
    }
    for source_slot in 0..model.base_material_count {
        if model
            .materials
            .iter()
            .filter(|material| material.source_slot == source_slot && material.lod.is_none())
            .count()
            != 1
        {
            return Err(invalid());
        }
    }
    for (index, bone) in model.bones.iter().enumerate() {
        if bone.index != index || bone.parent < -1 || bone.parent >= index as i32 {
            return Err(invalid());
        }
    }
    let mut sample_count = 0_usize;
    for (index, animation) in model.animations.iter().enumerate() {
        let expanded = animation.frames.len() == animation.frame_count as usize
            && animation.compact_frames.is_empty();
        let compact = animation.frames.is_empty()
            && !animation.compact_frames.is_empty()
            && compact_frame(animation, 0, &model.identity).is_ok();
        let authored = animation.frames.is_empty()
            && animation.compact_frames.is_empty()
            && animation
                .authored_frames
                .as_ref()
                .is_some_and(|retained| retained.context.bones.len() == animation.bone_map.len());
        if animation.index != index
            || animation.frame_count <= 0
            || (!expanded && !compact && !authored)
            || animation.bone_map.is_empty() && !model.bones.is_empty()
            || animation
                .bone_map
                .iter()
                .flatten()
                .any(|bone| *bone >= model.bones.len())
            || animation.frames.iter().any(|frame| {
                frame.translations.len() != animation.bone_map.len()
                    || frame.rotations.len() != animation.bone_map.len()
            })
            || animation.sections.iter().any(|section| {
                section
                    .tracks
                    .iter()
                    .any(|track| track.bone >= animation.bone_map.len())
            })
        {
            return Err(invalid());
        }
        sample_count = sample_count
            .checked_add((animation.frame_count as usize).saturating_mul(animation.bone_map.len()))
            .filter(|value| *value <= limits.max_animation_samples)
            .ok_or_else(invalid)?;
    }
    for (index, sequence) in model.sequences.iter().enumerate() {
        let populated_blend_count = sequence.blend_size[0]
            .checked_mul(sequence.blend_size[1])
            .filter(|value| *value > 0);
        let empty_override = sequence.flags & STUDIO_OVERRIDE != 0
            && sequence.blend_count == 0
            && sequence.blend_size == [0, 0];
        let populated = populated_blend_count == Some(sequence.blend_count);
        let children_valid = if empty_override {
            sequence.animation_indices.is_empty()
                && sequence.bone_weights.is_empty()
                && sequence.pose_keys.iter().all(Vec::is_empty)
                && sequence.events.is_empty()
                && sequence.auto_layers.is_empty()
        } else {
            populated
                && sequence.animation_indices.len() == sequence.blend_count as usize
                && sequence.bone_weights.len() == model.bones.len()
        };
        if sequence.index != index
            || (!empty_override && !populated)
            || !children_valid
            || sequence.event_count != sequence.events.len() as i32
            || sequence.auto_layer_count != sequence.auto_layers.len() as i32
            || sequence
                .events
                .iter()
                .enumerate()
                .any(|(event_index, event)| {
                    event.index != event_index || finite(event.cycle).is_none()
                })
            || sequence
                .auto_layers
                .iter()
                .enumerate()
                .any(|(layer_index, layer)| {
                    layer.index != layer_index
                        || layer.sequence < 0
                        || layer.sequence as usize >= model.sequences.len()
                        || layer.flags & STUDIO_AUTO_LAYER_POSE != 0
                            && (layer.pose < 0
                                || layer.pose as usize >= model.pose_parameters.len())
                })
            || sequence
                .animation_indices
                .iter()
                .any(|animation| *animation < 0 || *animation as usize >= model.animations.len())
            || (0..2).any(|axis| {
                sequence.blend_size[axis] > 1
                    && (sequence.pose_parameter_indices[axis] < 0
                        || sequence.pose_parameter_indices[axis] as usize
                            >= model.pose_parameters.len())
                    || !sequence.pose_keys[axis].is_empty()
                        && sequence.pose_keys[axis].len() != sequence.blend_size[axis] as usize
            })
            || sequence.flags & STUDIO_CYCLE_POSE != 0
                && (sequence.cycle_pose_parameter < 0
                    || sequence.cycle_pose_parameter as usize >= model.pose_parameters.len())
        {
            return Err(invalid());
        }
    }
    for (index, pose) in model.pose_parameters.iter().enumerate() {
        if pose.index != index {
            return Err(invalid());
        }
    }
    for (index, attachment) in model.attachments.iter().enumerate() {
        if attachment.index != index
            || attachment.bone < 0
            || attachment.bone as usize >= model.bones.len()
        {
            return Err(invalid());
        }
    }
    for (set_index, set) in model.hitbox_sets.iter().enumerate() {
        if set.index != set_index
            || set
                .hitboxes
                .iter()
                .enumerate()
                .any(|(hitbox_index, hitbox)| {
                    hitbox.index != hitbox_index
                        || hitbox.bone < 0
                        || hitbox.bone as usize >= model.bones.len()
                })
        {
            return Err(invalid());
        }
    }
    for (index, skin) in model.skins.iter().enumerate() {
        if skin.index != index
            || skin
                .texture_indices
                .iter()
                .any(|material| *material as usize >= model.base_material_count)
        {
            return Err(invalid());
        }
    }
    for (index, body_part) in model.body_parts.iter().enumerate() {
        if body_part.index != index || body_part.model_names.is_empty() {
            return Err(invalid());
        }
    }
    let mut vertices = 0_usize;
    let mut indices = 0_usize;
    for primitive in &model.geometry {
        if primitive.body_part >= model.body_parts.len()
            || primitive.model >= model.body_parts[primitive.body_part].model_names.len()
            || primitive.lod >= 8
            || primitive.material_slot
                >= model
                    .skins
                    .first()
                    .map_or(0, |skin| skin.texture_indices.len())
        {
            return Err(invalid());
        }
        vertices = vertices
            .checked_add(primitive.vertices.len())
            .filter(|value| *value <= limits.max_geometry_vertices)
            .ok_or_else(invalid)?;
        indices = indices
            .checked_add(primitive.encoded_indices.len())
            .filter(|value| *value <= limits.max_geometry_indices)
            .ok_or_else(invalid)?;
    }
    let expected = [
        FeatureFamily::ProceduralAxisInterpolation,
        FeatureFamily::ProceduralQuaternionInterpolation,
        FeatureFamily::ProceduralJiggle,
        FeatureFamily::ProceduralAimAtBone,
        FeatureFamily::ProceduralAimAtAttachment,
        FeatureFamily::InverseKinematics,
        FeatureFamily::Flex,
        FeatureFamily::SequenceAutoLayers,
        FeatureFamily::UnknownProcedural,
    ];
    if model
        .features
        .iter()
        .map(|feature| feature.family)
        .ne(expected)
        || model.features.iter().any(|feature| match feature.family {
            FeatureFamily::SequenceAutoLayers if feature.records > 0 => {
                feature.disposition != FeatureDisposition::Evaluated
            }
            _ => {
                (feature.records == 0) != (feature.disposition == FeatureDisposition::NotPresent)
                    || feature.records > 0
                        && feature.disposition != FeatureDisposition::RetainedNotEvaluated
            }
        })
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnimationFrame, AnimationSection, AnimationTrack, Bounds, CompanionSummary, Material,
        PhysicsStatus, Profile, RotationCodec, Sequence, SubModel, TranslationCodec,
    };

    fn float(value: f32) -> Float32 {
        Float32(value.to_bits())
    }

    fn identity_matrix() -> [Float32; 12] {
        [
            float(1.0),
            float(0.0),
            float(0.0),
            float(0.0),
            float(0.0),
            float(1.0),
            float(0.0),
            float(0.0),
            float(0.0),
            float(0.0),
            float(1.0),
            float(0.0),
        ]
    }

    fn bone(index: usize, name: &[u8], parent: i32, position: [f32; 3]) -> crate::Bone {
        crate::Bone {
            index,
            name: name.to_vec(),
            parent,
            controllers: [-1; 6],
            position: vector(position),
            quaternion: identity_quaternion(),
            rotation: vector([0.0; 3]),
            position_scale: vector([1.0; 3]),
            rotation_scale: vector([1.0; 3]),
            pose_to_bone: identity_matrix(),
            alignment: identity_quaternion(),
            flags: 0,
            procedural_type: 0,
            procedural_offset: 0,
            physics_bone: -1,
            surface_property: b"default".to_vec(),
            contents: 0,
        }
    }

    fn animation(index: usize, name: &[u8], end: f32, flags: i32) -> Animation {
        Animation {
            index,
            name: name.to_vec(),
            fps: float(30.0),
            flags,
            frame_count: 2,
            movement_count: 0,
            animation_block: 0,
            animation_offset: 0,
            ik_rule_count: 0,
            local_hierarchy_count: 0,
            section_offset: 0,
            section_frame_count: 0,
            zero_frame_count: 0,
            source_identity: "models/test.mdl".to_owned(),
            bone_map: vec![Some(0), Some(1)],
            sections: vec![AnimationSection {
                index: 0,
                first_frame: 0,
                frame_count: 2,
                block: 0,
                data_offset: 0,
                tracks: vec![AnimationTrack {
                    bone: 0,
                    flags: 0,
                    source_offset: 512,
                    next_offset: 0,
                    rotation_codec: RotationCodec::Bind,
                    translation_codec: TranslationCodec::Bind,
                    rotation_values: std::array::from_fn(|_| None),
                    translation_values: std::array::from_fn(|_| None),
                }],
            }],
            frames: vec![
                AnimationFrame {
                    translations: vec![vector([0.0; 3]), vector([0.0, 1.0, 0.0])],
                    rotations: vec![identity_quaternion(); 2],
                },
                AnimationFrame {
                    translations: vec![vector([end, 0.0, 0.0]), vector([0.0, 1.0, 0.0])],
                    rotations: vec![identity_quaternion(); 2],
                },
            ],
            compact_frames: Vec::new(),
            authored_frames: None,
        }
    }

    fn sequence(index: usize, label: &[u8], activity: &[u8], animation: i16) -> Sequence {
        Sequence {
            index,
            label: label.to_vec(),
            activity_name: activity.to_vec(),
            flags: 0,
            activity: index as i32 + 100,
            activity_weight: 1,
            event_count: 0,
            bounds_min: vector([-1.0; 3]),
            bounds_max: vector([1.0; 3]),
            blend_count: 1,
            blend_size: [1, 1],
            animation_indices: vec![animation],
            pose_parameter_indices: [0, 0],
            pose_parameter_start: [float(0.0); 2],
            pose_parameter_end: [float(1.0); 2],
            fade_in: float(0.2),
            fade_out: float(0.2),
            entry_node: 0,
            exit_node: 0,
            node_flags: 0,
            entry_phase: float(0.0),
            exit_phase: float(0.0),
            last_frame: float(1.0),
            next_sequence: 0,
            pose: 0,
            auto_layer_count: 0,
            bone_weights: vec![float(1.0); 2],
            pose_keys: [Vec::new(), Vec::new()],
            ik_lock_count: 0,
            cycle_pose_parameter: 0,
            activity_modifier_count: 0,
            events: Vec::new(),
            auto_layers: Vec::new(),
            source_identity: "models/test.mdl".to_owned(),
        }
    }

    fn document() -> Document {
        let activities = [
            (b"idle".as_slice(), b"ACT_MP_STAND_PRIMARY".as_slice()),
            (b"run", b"ACT_MP_RUN_PRIMARY"),
            (b"jump", b"ACT_MP_JUMP_PRIMARY"),
            (b"crouch", b"ACT_MP_CROUCH_PRIMARY"),
            (b"fire", b"ACT_MP_ATTACK_STAND_PRIMARYFIRE"),
            (b"draw", b"ACT_VM_DRAW"),
            (b"vm_idle", b"ACT_VM_IDLE"),
            (b"recoil", b"ACT_VM_PRIMARYATTACK"),
            (b"reload", b"ACT_VM_RELOAD"),
        ];
        Document {
            identity: "models/test.mdl".to_owned(),
            profile: Profile::SourcePcMdl48,
            checksum: 7,
            internal_name: b"test".to_vec(),
            declared_length: 1_024,
            flags: 0,
            root_lod: 0,
            allowed_root_lods: 0,
            bounds: Bounds {
                eye: vector([0.0; 3]),
                illumination: vector([0.0; 3]),
                hull_min: vector([-1.0; 3]),
                hull_max: vector([1.0; 3]),
                view_min: vector([-1.0; 3]),
                view_max: vector([1.0; 3]),
            },
            illumination_attachment: 0,
            raw_max_eye_deflection: float(0.0),
            max_eye_deflection: float(0.866),
            bones: vec![
                bone(0, b"root", -1, [0.0; 3]),
                bone(1, b"child", 0, [0.0, 1.0, 0.0]),
            ],
            animations: vec![animation(0, b"move", 2.0, 0)],
            sequences: activities
                .iter()
                .enumerate()
                .map(|(index, (label, activity))| sequence(index, label, activity, 0))
                .collect(),
            materials: vec![Material {
                index: 0,
                name: b"test/material".to_vec(),
                search_paths: vec![b"missing".to_vec(), b"models/test".to_vec()],
                candidates: vec![
                    "materials/missing/test/material.vmt".to_owned(),
                    "materials/models/test/test/material.vmt".to_owned(),
                ],
            }],
            material_replacements: Vec::new(),
            skins: vec![SkinFamily {
                index: 0,
                texture_indices: vec![0],
            }],
            body_parts: vec![BodyPart {
                index: 0,
                name: b"body".to_vec(),
                base: 1,
                models: vec![SubModel {
                    index: 0,
                    name: b"body_default".to_vec(),
                    mesh_count: 0,
                    vertex_count: 0,
                    vertex_offset_bytes: 0,
                    tangent_offset_bytes: 0,
                    attachment_count: 0,
                    eyeball_count: 0,
                    meshes: Vec::new(),
                    eyeballs: Vec::new(),
                }],
            }],
            attachments: vec![Attachment {
                index: 0,
                name: b"muzzle".to_vec(),
                flags: 0,
                bone: 1,
                local: [
                    float(1.0),
                    float(0.0),
                    float(0.0),
                    float(0.0),
                    float(0.0),
                    float(1.0),
                    float(0.0),
                    float(0.0),
                    float(0.0),
                    float(0.0),
                    float(1.0),
                    float(1.0),
                ],
            }],
            hitbox_sets: Vec::new(),
            pose_parameters: vec![crate::PoseParameter {
                index: 0,
                name: b"move_x".to_vec(),
                flags: 0,
                start: float(0.0),
                end: float(1.0),
                looping_range: float(0.0),
                source_identity: "models/test.mdl".to_owned(),
            }],
            unsupported: crate::UnsupportedMetadata {
                flex_descriptors: 0,
                flex_controllers: 0,
                flex_rules: 0,
                ik_chains: 0,
            },
            include_models: Vec::new(),
            animation_blocks: Vec::new(),
            animation_block_identity: None,
            companions: CompanionSummary {
                vvd_lod_vertex_counts: vec![0],
                vvd_fixup_count: 0,
                vvd_vertex_offset: 0,
                vvd_tangent_offset: 0,
                vtx_lod_count: 1,
                vtx_body_part_count: 1,
                vtx_max_bones_per_vertex: 3,
            },
            physics_status: PhysicsStatus::Missing,
            source_identities: vec!["models/test.mdl".to_owned()],
            model_dependencies: vec![crate::ModelDependency {
                requester: "models/test.mdl".to_owned(),
                role: crate::ModelDependencyRole::RootModel,
                logical_path: "models/test.mdl".to_owned(),
                sha256: Some(sha256(b"model")),
                byte_length: 5,
            }],
            geometry: vec![GeometryPrimitive {
                body_part: 0,
                model: 0,
                lod: 0,
                mesh: 0,
                strip_group: 0,
                switch_point: float(0.0),
                material_slot: 0,
                source_vertex_ids: Vec::new(),
                vertices: Vec::new(),
                encoded_indices: Vec::new(),
                strips: Vec::new(),
                triangles: Vec::new(),
            }],
        }
    }

    fn response(
        request: &PresentationDependencyRequest,
        bytes: Option<&[u8]>,
        material: Option<MaterialResolutionManifest>,
    ) -> PresentationDependencyResponse {
        PresentationDependencyResponse {
            requester: request.requester.clone(),
            role: request.role,
            logical_path: request.logical_path.clone(),
            material_slot: request.material_slot,
            texture_role: request.texture_role,
            bytes: bytes.map(<[u8]>::to_vec),
            verified_byte_length: None,
            sha256: bytes.map(sha256),
            material,
        }
    }

    fn build() -> PresentationArtifact {
        build_profile(PresentationProfile::World)
    }

    #[test]
    fn static_lighting_joins_root_lod_meshes_without_copying_color_bytes() {
        let mut model = document();
        model.checksum = 7;
        model.root_lod = 0;
        let vertex = |source_index| crate::Vertex {
            source_index,
            weights: [float(1.0), float(0.0), float(0.0)],
            bones: [0; 3],
            bone_count: 1,
            position: vector([0.0; 3]),
            normal: vector([0.0, 0.0, 1.0]),
            uv: [float(0.0), float(0.0)],
            tangent: [float(1.0), float(0.0), float(0.0), float(1.0)],
        };
        model.geometry[0].vertices = vec![vertex(0), vertex(1)];
        let mut bytes = vec![0u8; 1_024];
        bytes[0..4].copy_from_slice(&2i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&7u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&4u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&2u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&1i32.to_le_bytes());
        bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&2u32.to_le_bytes());
        bytes[48..52].copy_from_slice(&512u32.to_le_bytes());
        bytes[512..520].copy_from_slice(&[1, 2, 3, 255, 4, 5, 6, 255]);
        let vhv = playsrc_vhv::parse(
            &bytes,
            playsrc_vhv::Profile::source_pc_v2_color_bgra8888(7),
            playsrc_vhv::Limits {
                max_input_bytes: 1_024,
                max_retained_bytes: 2_048,
                max_meshes: 1,
                max_total_vertices: 2,
                max_vertices_per_mesh: 2,
                max_lod: 7,
            },
        )
        .unwrap();
        let joined = crate::join_static_lighting(&model, &vhv).unwrap();
        assert_eq!(joined.root_lod, 0);
        assert_eq!(joined.vertex_count, 2);
        assert_eq!(joined.meshes[0].primitive, 0);
        assert_eq!(joined.meshes[0].encoded_bgra_range, 512..520);

        model.geometry[0].vertices.push(vertex(2));
        assert_eq!(
            crate::join_static_lighting(&model, &vhv),
            Err(crate::StaticLightingJoinError::VertexCountMismatch)
        );
    }

    fn build_profile(profile: PresentationProfile) -> PresentationArtifact {
        build_document(profile, document())
    }

    fn build_document(profile: PresentationProfile, document: Document) -> PresentationArtifact {
        let PresentationBuild::Needs(candidates) = build_presentation(
            &document,
            profile,
            &[],
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("material candidates were not requested")
        };
        assert_eq!(candidates.len(), 2);
        let manifest = MaterialResolutionManifest {
            root_identity: candidates[1].logical_path.clone(),
            include_sources: vec![MaterialSourceManifest {
                requester: candidates[1].logical_path.clone(),
                logical_path: "materials/models/test/base.vmt".to_owned(),
            }],
            textures: vec![
                MaterialTextureManifest {
                    role: TextureRole::Base,
                    parameter: b"$basetexture".to_vec(),
                    logical_path: Some("materials/models/test/base.vtf".to_owned()),
                    disposition: TextureDisposition::Source,
                    selected: true,
                },
                MaterialTextureManifest {
                    role: TextureRole::Environment,
                    parameter: b"$envmap".to_vec(),
                    logical_path: None,
                    disposition: TextureDisposition::BuiltInEnvironment,
                    selected: false,
                },
            ],
        };
        let mut responses = vec![
            response(&candidates[0], None, None),
            response(&candidates[1], Some(b"patch-vmt"), Some(manifest)),
        ];
        let PresentationBuild::Needs(closure) = build_presentation(
            &document,
            profile,
            &responses,
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("material closure was not requested")
        };
        assert_eq!(
            closure
                .iter()
                .map(|request| (request.role, request.logical_path.as_str()))
                .collect::<Vec<_>>(),
            [
                (
                    PresentationDependencyRole::MaterialInclude,
                    "materials/models/test/base.vmt"
                ),
                (
                    PresentationDependencyRole::Texture,
                    "materials/models/test/base.vtf"
                ),
            ]
        );
        responses.extend([
            response(&closure[0], Some(b"base-vmt"), None),
            response(&closure[1], Some(b"base-vtf"), None),
        ]);
        let PresentationBuild::Complete(artifact) = build_presentation(
            &document,
            profile,
            &responses,
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("closed dependencies did not produce an artifact")
        };
        *artifact
    }

    #[test]
    fn collision_bounds_preserve_authored_header_bits_in_both_profiles() {
        for profile in [PresentationProfile::World, PresentationProfile::ViewModel] {
            for bounds in [
                [vector([-13.25, -0.0, -0.03125]), vector([42.5, 0.0, 73.0])],
                [vector([0.0; 3]); 2],
            ] {
                let mut source = document();
                source.bounds.hull_min = bounds[0];
                source.bounds.hull_max = bounds[1];
                let artifact = build_document(profile, source);
                assert_eq!(artifact.model.collision_bounds, bounds);
                assert_eq!(decode_presentation(&artifact.bytes, PresentationLimits::default()).unwrap().model.collision_bounds, bounds);
                let mut old_version = artifact.bytes;
                old_version[4..6].copy_from_slice(&3_u16.to_le_bytes());
                assert!(decode_presentation(&old_version, PresentationLimits::default()).is_err());
            }
        }
    }

    #[test]
    fn emits_exact_material_closure_and_round_trips_byte_identical_artifacts() {
        let first = build();
        let second = build();
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(
            first.sha256,
            hex_hash("dd3035ca25c79675be0e0ef1de44416a6e55b35c633291b987c09fc7b90eae76")
        );
        assert_eq!(first.bytes.len(), 3_616);
        assert!(first.bytes.len() < MAX_MESSAGE_BYTES);
        assert_eq!(first.model.dependencies.len(), 4);
        assert_eq!(
            first.model.dependencies[0].role,
            PresentationDependencyRole::RootModel
        );
        assert_eq!(first.model.materials[0].textures.len(), 2);
        let viewmodel = build_profile(PresentationProfile::ViewModel);
        assert_eq!(viewmodel.model.profile, PresentationProfile::ViewModel);
        assert_ne!(viewmodel.bytes, first.bytes);
        assert_eq!(
            decode_presentation(&first.bytes, PresentationLimits::default()).unwrap(),
            first
        );
        assert_eq!(
            sha256(b"abc"),
            hex_hash("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            sha256(b""),
            hex_hash("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            hex_hash("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")
        );
    }

    #[test]
    fn source_skin_family_selects_zero_for_invalid_or_outdated_indexes() {
        assert_eq!(source_skin_family(-1, 2), 0);
        assert_eq!(source_skin_family(0, 2), 0);
        assert_eq!(source_skin_family(1, 2), 1);
        assert_eq!(source_skin_family(1, 1), 0);
        assert_eq!(source_skin_family(i32::MAX, 2), 0);
    }

    #[test]
    fn exposes_source_world_and_viewmodel_transform_contracts() {
        let world = build();
        assert_eq!(
            world.model.descriptor,
            PresentationDescriptor::World {
                geometry: GeometryOrientation::source(),
                entity_angles: EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns,
                root_bone: RootBoneContract::AnimatedBelowEntity,
                depth_range: [float(0.0), float(1.0)],
            }
        );
        let viewmodel = build_profile(PresentationProfile::ViewModel);
        assert_eq!(
            viewmodel.model.descriptor,
            PresentationDescriptor::ViewModel {
                geometry: GeometryOrientation::source(),
                entity_angles: EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns,
                default_horizontal_fov_4_by_3: float(54.0),
                minimum_fov: float(0.1),
                maximum_fov: float(179.9),
                near_plane: float(1.0),
                far_plane: FarPlaneContract::SuppliedWorldFarPlane,
                depth_range: [float(0.0), float(0.1)],
                draws_after_world: true,
                opaque_before_translucent: true,
                handedness: ViewmodelHandednessContract::OptionalViewSpaceYReflection,
            }
        );

        let transform =
            source_entity_transform(vector([10.0, 20.0, 30.0]), vector([0.0, 90.0, 0.0])).unwrap();
        let values = transform.0.map(|value| f32::from_bits(value.0));
        let expected = [
            0.0, -1.0, 0.0, 10.0, 1.0, 0.0, 0.0, 20.0, 0.0, 0.0, 1.0, 30.0,
        ];
        for (actual, expected) in values.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1.0e-6);
        }

        let reflected = reflect_viewmodel_handedness(
            source_entity_transform(vector([0.0; 3]), vector([0.0; 3])).unwrap(),
            source_entity_transform(vector([1.0, 2.0, 3.0]), vector([0.0; 3])).unwrap(),
        );
        assert_eq!(
            reflected.0.map(|value| f32::from_bits(value.0)),
            [
                1.0, 0.0, 0.0, 1.0, -0.0, -1.0, -0.0, -2.0, 0.0, 0.0, 1.0, 3.0
            ]
        );
        let facing = GeometryOrientation::source().facing;
        assert_eq!(facing.front_face, TriangleWinding::Clockwise);
        assert_eq!(facing.cull_face, CullFace::Back);
        assert_eq!(
            affine_transform_orientation(transform).unwrap(),
            TransformOrientation::Preserving
        );
        assert_eq!(
            affine_transform_orientation(reflected).unwrap(),
            TransformOrientation::Reversing
        );
        assert_eq!(
            transformed_geometry_facing(facing, TransformOrientation::Reversing).front_face,
            TriangleWinding::CounterClockwise
        );
        assert_eq!(
            combine_transform_orientations([
                TransformOrientation::Reversing,
                TransformOrientation::Reversing,
            ]),
            TransformOrientation::Preserving
        );
        let mut singular = transform;
        singular.0[0] = float(0.0);
        singular.0[4] = float(0.0);
        singular.0[8] = float(0.0);
        assert_eq!(
            affine_transform_orientation(singular).unwrap_err().code,
            PresentationErrorCode::InvalidState
        );
        let mut non_finite = transform;
        non_finite.0[0] = Float32(f32::NAN.to_bits());
        assert_eq!(
            affine_transform_orientation(non_finite).unwrap_err().code,
            PresentationErrorCode::InvalidState
        );
    }

    #[test]
    fn entity_panel_transitions_preserve_authored_rotation_sign_hierarchy_and_interruption_order() {
        let mut model = build().model;
        let root = [float(0.5); 4];
        let handle = [float(0.0), float(0.0), float((std::f32::consts::FRAC_PI_4 / 2.0).sin()), float((std::f32::consts::FRAC_PI_4 / 2.0).cos())];
        model.animations.clear();
        for index in 0..2 {
            model.animations.push(animation(index, b"panel", 0.0, 0));
            model.sequences[index].animation_indices = vec![index as i16];
            for frame in &mut model.animations[index].frames {
                frame.rotations = vec![root, if index == 0 { identity_quaternion() } else { handle }];
            }
        }
        let parameters = vec![float(0.0)];
        let mut transitions = crate::SequenceTransitioner::default();
        assert!(transitions.update(&model, 0, 1.0, 1.0, 0.99, &parameters, 0).unwrap().is_empty());
        let layers = transitions.update(&model, 1, 0.0, 1.02, 1.0, &parameters, 0).unwrap();
        assert!(transitions.is_transitioning());
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].sequence, 0);
        assert!((f32::from_bits(layers[0].weight.0) - 0.972).abs() < 1e-5);
        let state = AnimationState { base_sequence: 1, cycle: float(0.0), pose_parameters: parameters.clone(), layers, bone_rotations: Vec::new() };
        let pose = sample_pose(&model, &state).unwrap();
        assert!(values4(pose.local_rotations[1])[2] > 0.0);
        assert!(values4(pose.local_rotations[1])[2] < values4(handle)[2]);
        let world = apply_entity_transform(&model, &pose, source_entity_transform(vector([290.0, 0.0, -34.0]), vector([0.0, 180.0, 0.0])).unwrap()).unwrap();
        let matrix = world.bone_matrices[1].0.map(|v| f32::from_bits(v.0));
        assert!(matrix[8] > 0.0 && -matrix[4] > 0.0, "positive local rotation raises the screen-right axis; do not negate it");
        let interrupted = transitions.update(&model, 0, 0.0, 1.05, 1.04, &parameters, 0).unwrap();
        assert_eq!(interrupted.iter().map(|v| v.sequence).collect::<Vec<_>>(), [1, 0]);
        assert!(transitions.update(&model, 0, 1.0, 1.3, 1.29, &parameters, 0).unwrap().is_empty());
        assert!(!transitions.is_transitioning());
        // A terminal pose may have been reused for many paints; its next fade
        // starts at the previous paint, not at its last expensive pose sample.
        let resumed = transitions.update(&model, 1, 0.0, 5.02, 5.0, &parameters, 0).unwrap();
        assert!((f32::from_bits(resumed[0].weight.0) - 0.972).abs() < 1e-5);
        transitions.clear();
        assert!(transitions.update(&model, 1, 0.0, 6.0, 5.99, &parameters, 0).unwrap().is_empty());
        assert_eq!(transitions.update(&model, 1, 0.0, 6.01, 6.0, &parameters, 1).unwrap().len(), 1);
        assert!(transitions.update(&model, 1, 1.0, 6.3, 6.29, &parameters, 1).unwrap().is_empty());
        assert!(transitions.update(&model, 1, 0.0, 6.4, 6.39, &parameters, 9).unwrap().is_empty(), "three-bit parity wraps without a forced transition");
        model.sequences[0].flags |= 2;
        assert!(transitions.update(&model, 0, 0.0, 6.5, 6.49, &parameters, 10).unwrap().is_empty());
    }

    #[test]
    fn standard_blending_rotations_feed_descendants_palettes_and_attachments() {
        let model = build().model;
        let mut state = AnimationState { base_sequence: 0, cycle: float(0.0), pose_parameters: vec![float(0.0)], layers: Vec::new(), bone_rotations: Vec::new() };
        let base = sample_pose(&model, &state).unwrap();
        state.bone_rotations.push((0, [float(0.0), float(0.0), float(1.0), float(0.0)]));
        let rotated = sample_pose(&model, &state).unwrap();
        assert_eq!(rotated.local_translations, base.local_translations);
        assert_eq!(rotated.model_matrices[1].0[7], float(-1.0));
        assert_ne!(rotated.skinning_matrices[1], base.skinning_matrices[1]);
        assert_ne!(rotated.attachments[0].model_transform, base.attachments[0].model_transform);
        state.bone_rotations[0].0 = model.bones.len();
        assert!(sample_pose(&model, &state).is_err());
    }

    #[test]
    fn evaluates_sequence_timing_autolayers_events_hitboxes_and_static_roots() {
        let artifact = build();
        let timing = sequence_timing(&artifact.model, 0, &[float(0.0)]).unwrap();
        assert_eq!(timing.frames_per_second, float(30.0));
        assert_eq!(timing.weighted_frame_count, float(1.0));
        assert_eq!(timing.cycles_per_second, float(30.0));
        assert_eq!(timing.duration_seconds, float(1.0 / 30.0));

        let mut model = artifact.model;
        let animation_index = model.animations.len();
        model
            .animations
            .push(animation(animation_index, b"auto_delta", 1.0, STUDIO_DELTA));
        let sequence_index = model.sequences.len();
        let mut delta = sequence(sequence_index, b"auto_delta", b"", animation_index as i16);
        delta.flags = STUDIO_DELTA;
        model.sequences.push(delta);
        model.sequences[0]
            .auto_layers
            .push(crate::SequenceAutoLayer {
                index: 0,
                sequence: sequence_index as i16,
                pose: 0,
                flags: 0,
                start: float(0.0),
                peak: float(0.0),
                tail: float(0.0),
                end: float(0.0),
            });
        model.sequences[0].auto_layer_count = 1;
        model.sequences[0].events = vec![
            crate::SequenceEvent {
                index: 0,
                cycle: float(0.25),
                event: 5_004,
                event_type: 0,
                options: [0; 64],
                name: Vec::new(),
            },
            crate::SequenceEvent {
                index: 1,
                cycle: float(0.95),
                event: 5_005,
                event_type: 0,
                options: [1; 64],
                name: Vec::new(),
            },
            crate::SequenceEvent {
                index: 2,
                cycle: float(0.1),
                event: 5_006,
                event_type: 0,
                options: [2; 64],
                name: b"client".to_vec(),
            },
        ];
        model.sequences[0].event_count = 3;
        model.hitbox_sets.push(crate::HitboxSet {
            index: 0,
            name: b"main".to_vec(),
            hitboxes: vec![crate::Hitbox {
                index: 0,
                bone: 1,
                group: 3,
                bounds_min: vector([-1.0; 3]),
                bounds_max: vector([1.0; 3]),
                name_offset: 0,
                name_resolved: true,
                name: Vec::new(),
            }],
        });
        let autolayers = model
            .features
            .iter_mut()
            .find(|feature| feature.family == FeatureFamily::SequenceAutoLayers)
            .unwrap();
        autolayers.records = 1;
        autolayers.disposition = FeatureDisposition::Evaluated;

        let pose = sample_pose(
            &model,
            &AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: 0,
                cycle: float(1.0),
                pose_parameters: vec![float(0.0)],
                layers: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(pose.local_translations[0].0[0], float(3.0));
        model.sequences[0].flags |= STUDIO_LOOPING;
        assert_eq!(
            presentation_events_between(&model, 0, float(0.0), float(0.5))
                .unwrap()
                .iter()
                .map(|event| event.event)
                .collect::<Vec<_>>(),
            [5_004, 5_006]
        );
        assert_eq!(
            presentation_events_between(&model, 0, float(0.9), float(0.1))
                .unwrap()
                .iter()
                .map(|event| event.event)
                .collect::<Vec<_>>(),
            [5_005, 5_006]
        );

        let bytes = encode_model(&model, PresentationLimits::default()).unwrap();
        assert_eq!(
            decode_presentation(&bytes, PresentationLimits::default())
                .unwrap()
                .model,
            model
        );

        model.flags |= STUDIO_HEADER_STATIC_PROP;
        model.descriptor = presentation_descriptor(model.profile, model.flags);
        let entity = source_entity_transform(vector([4.0, 5.0, 6.0]), vector([0.0; 3])).unwrap();
        let world = apply_entity_transform(&model, &pose, entity).unwrap();
        assert_eq!(world.bone_matrices[0], entity);
        assert_eq!(world.skinning_matrices[0], entity);
    }

    fn hex_hash(value: &str) -> [u8; 32] {
        let mut output = [0; 32];
        for (index, byte) in output.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
        }
        output
    }

    #[test]
    fn samples_frames_parent_matrices_attachments_activities_and_selection() {
        let artifact = build();
        let pose = sample_pose(
            &artifact.model,
            &AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: 0,
                cycle: float(0.5),
                pose_parameters: vec![float(0.0)],
                layers: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(values3(pose.local_translations[0]), [1.0, 0.0, 0.0]);
        assert_eq!(f32::from_bits(pose.model_matrices[1].0[3].0), 1.0);
        assert_eq!(f32::from_bits(pose.model_matrices[1].0[7].0), 1.0);
        assert_eq!(
            f32::from_bits(pose.attachments[0].model_transform.0[11].0),
            1.0
        );
        for activity in [
            b"ACT_MP_STAND_PRIMARY".as_slice(),
            b"ACT_MP_RUN_PRIMARY",
            b"ACT_MP_JUMP_PRIMARY",
            b"ACT_MP_CROUCH_PRIMARY",
            b"ACT_MP_ATTACK_STAND_PRIMARYFIRE",
            b"ACT_VM_DRAW",
            b"ACT_VM_IDLE",
            b"ACT_VM_PRIMARYATTACK",
            b"ACT_VM_RELOAD",
        ] {
            assert_eq!(
                sequences_for_activity_name(&artifact.model, activity).len(),
                1
            );
        }
        assert_eq!(
            select_primitives(&artifact.model, &[0], 0, 0).unwrap(),
            [SelectedPrimitive {
                primitive: 0,
                material: 0,
            }]
        );
        assert_eq!(artifact.model.basis.forward, vector([1.0, 0.0, 0.0]));
    }

    #[test]
    fn interpolates_pose_grids_and_ordered_delta_layers() {
        let mut artifact = build();
        artifact.model.animations.push(animation(1, b"far", 6.0, 0));
        artifact.model.sequences[0].blend_count = 2;
        artifact.model.sequences[0].blend_size = [2, 1];
        artifact.model.sequences[0].animation_indices = vec![0, 1];
        artifact.model.sequences[0].pose_parameter_indices[0] = 0;
        let pose = sample_pose(
            &artifact.model,
            &AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: 0,
                cycle: float(0.5),
                pose_parameters: vec![float(0.5)],
                layers: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(values3(pose.local_translations[0]), [2.0, 0.0, 0.0]);

        artifact.model.pose_parameters.push(crate::PoseParameter {
            index: 1,
            name: b"move_y".to_vec(),
            flags: 0,
            start: float(0.0),
            end: float(1.0),
            looping_range: float(0.0),
            source_identity: "models/test.mdl".to_owned(),
        });
        artifact
            .model
            .animations
            .push(animation(2, b"upper_left", 4.0, 0));
        artifact
            .model
            .animations
            .push(animation(3, b"upper_right", 8.0, 0));
        artifact.model.sequences[0].blend_count = 4;
        artifact.model.sequences[0].blend_size = [2, 2];
        artifact.model.sequences[0].animation_indices = vec![0, 1, 2, 3];
        artifact.model.sequences[0].pose_parameter_indices = [0, 1];
        let three_way = sample_pose(
            &artifact.model,
            &AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: 0,
                cycle: float(0.5),
                pose_parameters: vec![float(0.25), float(0.75)],
                layers: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(values3(three_way.local_translations[0]), [2.25, 0.0, 0.0]);

        let delta_index = artifact.model.animations.len();
        artifact
            .model
            .animations
            .push(animation(delta_index, b"recoil_delta", 2.0, STUDIO_DELTA));
        let mut delta = sequence(
            artifact.model.sequences.len(),
            b"recoil_delta",
            b"ACT_VM_RECOIL",
            delta_index as i16,
        );
        delta.flags = STUDIO_DELTA;
        let delta_sequence = delta.index;
        artifact.model.sequences.push(delta);
        let layered = sample_pose(
            &artifact.model,
            &AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: 1,
                cycle: float(0.5),
                pose_parameters: vec![float(0.0), float(0.0)],
                layers: vec![AnimationLayer {
                    sequence: delta_sequence,
                    cycle: float(0.5),
                    weight: float(0.5),
                }],
            },
        )
        .unwrap();
        assert_eq!(values3(layered.local_translations[0]), [1.5, 0.0, 0.0]);
    }

    #[test]
    fn retains_and_selects_lod_material_replacements() {
        let mut document = document();
        document
            .material_replacements
            .push(crate::MaterialReplacement {
                lod: 0,
                material_slot: 0,
                name: b"lod_material".to_vec(),
                candidates: vec!["materials/models/test/lod_material.vmt".to_owned()],
            });
        let PresentationBuild::Needs(candidates) = build_presentation(
            &document,
            PresentationProfile::World,
            &[],
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("LOD material candidates were not requested")
        };
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[2].material_slot, 1);
        let base_manifest = MaterialResolutionManifest {
            root_identity: candidates[1].logical_path.clone(),
            include_sources: Vec::new(),
            textures: vec![MaterialTextureManifest {
                role: TextureRole::Base,
                parameter: b"$basetexture".to_vec(),
                logical_path: Some("materials/models/test/base.vtf".to_owned()),
                disposition: TextureDisposition::Source,
                selected: true,
            }],
        };
        let lod_manifest = MaterialResolutionManifest {
            root_identity: candidates[2].logical_path.clone(),
            include_sources: Vec::new(),
            textures: vec![MaterialTextureManifest {
                role: TextureRole::Base,
                parameter: b"$basetexture".to_vec(),
                logical_path: Some("materials/models/test/lod.vtf".to_owned()),
                disposition: TextureDisposition::Source,
                selected: true,
            }],
        };
        let mut responses = vec![
            response(&candidates[0], None, None),
            response(&candidates[1], Some(b"base"), Some(base_manifest)),
            response(&candidates[2], Some(b"lod"), Some(lod_manifest)),
        ];
        let PresentationBuild::Needs(textures) = build_presentation(
            &document,
            PresentationProfile::World,
            &responses,
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("LOD textures were not requested")
        };
        assert_eq!(textures.len(), 2);
        responses.extend(
            textures
                .iter()
                .map(|request| response(request, Some(b"vtf"), None)),
        );
        let PresentationBuild::Complete(artifact) = build_presentation(
            &document,
            PresentationProfile::World,
            &responses,
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("LOD material closure did not produce an artifact")
        };
        assert_eq!(artifact.model.materials.len(), 2);
        assert_eq!(artifact.model.materials[1].source_slot, 0);
        assert_eq!(artifact.model.materials[1].lod, Some(0));
        assert_eq!(
            select_primitives(&artifact.model, &[0], 0, 0).unwrap()[0].material,
            1
        );
        assert_eq!(
            decode_presentation(&artifact.bytes, PresentationLimits::default()).unwrap(),
            *artifact
        );
    }

    #[test]
    fn classifies_retained_and_evaluated_presentation_families() {
        let mut document = document();
        for procedure in [1, 2, 5, 3, 4, 99] {
            let index = document.bones.len();
            document.bones.push(bone(
                index,
                format!("proc_{procedure}").as_bytes(),
                -1,
                [0.0; 3],
            ));
            document.bones[index].procedural_type = procedure;
        }
        document.animations[0].ik_rule_count = 2;
        document.sequences[0].ik_lock_count = 1;
        document.sequences[0].auto_layer_count = 3;
        document.unsupported.flex_descriptors = 2;
        document.unsupported.flex_controllers = 1;
        document.unsupported.flex_rules = 4;
        document.unsupported.ik_chains = 1;
        let support = feature_support(&document);
        assert_eq!(support.len(), 9);
        assert!(support.iter().enumerate().all(|(index, feature)| {
            feature.records > 0
                && feature.disposition
                    == if index == 7 {
                        FeatureDisposition::Evaluated
                    } else {
                        FeatureDisposition::RetainedNotEvaluated
                    }
        }));
        assert_eq!(support[5].records, 4);
        assert_eq!(support[6].records, 7);
        assert_eq!(support[7].records, 3);
        assert_eq!(support[8].records, 1);
    }

    #[test]
    fn dense_constant_animation_channels_are_retained_once() {
        let mut document = document();
        let frame = document.animations[0].frames[0].clone();
        document.animations[0].frame_count = 256;
        document.animations[0].frames = vec![frame; 256];
        let limits = PresentationLimits {
            max_owned_bytes: 10_000,
            ..PresentationLimits::default()
        };
        let first = build_presentation_from_complete_responses(
            &document,
            PresentationProfile::World,
            limits,
        )
        .unwrap();
        let second = build_presentation_from_complete_responses(
            &document,
            PresentationProfile::World,
            limits,
        )
        .unwrap();
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(
            u16::from_le_bytes(first.bytes[4..6].try_into().unwrap()),
            ARTIFACT_VERSION
        );
        assert!(first.model.animations[0].frames.is_empty());
        assert!(!first.model.animations[0].compact_frames.is_empty());
        assert_eq!(decode_presentation(&first.bytes, limits).unwrap(), first);
        let sampled = sample_pose(
            &first.model,
            &AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: 0,
                cycle: float(0.5),
                pose_parameters: vec![float(0.0)],
                layers: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(sampled.local_translations[0], vector([0.0; 3]));
    }

    #[test]
    fn shared_dependency_occurrences_charge_unique_source_bytes() {
        let first = ArtifactDependency {
            requester: "models/a.mdl".to_owned(),
            role: PresentationDependencyRole::Texture,
            logical_path: "materials/shared.vtf".to_owned(),
            material_slot: Some(0),
            texture_role: Some(TextureRole::Base),
            sha256: Some([1; 32]),
            byte_length: 1_024,
        };
        let mut second = first.clone();
        second.requester = "models/b.mdl".to_owned();
        second.material_slot = Some(1);
        let mut changed = second.clone();
        changed.sha256 = Some([2; 32]);
        assert_eq!(
            unique_dependency_bytes(&[first.clone(), second]),
            Some(1_024)
        );
        assert_eq!(unique_dependency_bytes(&[first, changed]), Some(2_048));
    }

    #[test]
    fn rejects_missing_hashes_cancellation_bounds_and_noncanonical_artifacts() {
        let document = document();
        let PresentationBuild::Needs(requests) = build_presentation(
            &document,
            PresentationProfile::ViewModel,
            &[],
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap() else {
            panic!("expected candidates")
        };
        let missing: Vec<_> = requests
            .iter()
            .map(|request| response(request, None, None))
            .collect();
        assert_eq!(
            build_presentation(
                &document,
                PresentationProfile::ViewModel,
                &missing,
                PresentationLimits::default(),
                &CancellationToken::default(),
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::MissingMaterial
        );
        let mut corrupt = response(&requests[0], Some(b"material"), None);
        corrupt.sha256 = Some([0; 32]);
        assert_eq!(
            build_presentation(
                &document,
                PresentationProfile::World,
                &[corrupt],
                PresentationLimits::default(),
                &CancellationToken::default(),
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::HashMismatch
        );
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert_eq!(
            build_presentation(
                &document,
                PresentationProfile::World,
                &[],
                PresentationLimits::default(),
                &cancellation,
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::Cancelled
        );
        assert_eq!(
            build_presentation(
                &document,
                PresentationProfile::World,
                &[],
                PresentationLimits {
                    max_artifact_bytes: MAX_MESSAGE_BYTES,
                    ..PresentationLimits::default()
                },
                &CancellationToken::default(),
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::InvalidLimits
        );
        let mut artifact = build().bytes;
        artifact.push(0);
        assert_eq!(
            decode_presentation(&artifact, PresentationLimits::default())
                .unwrap_err()
                .code,
            PresentationErrorCode::InvalidArtifact
        );

        let exact = build();
        let exact_limit = PresentationLimits {
            max_artifact_bytes: exact.bytes.len(),
            ..PresentationLimits::default()
        };
        let rebuilt = build_presentation_from_complete_responses(
            &document,
            PresentationProfile::World,
            exact_limit,
        )
        .unwrap();
        assert_eq!(rebuilt.bytes.len(), exact.bytes.len());
        assert_eq!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_artifact_bytes: exact.bytes.len() - 1,
                    ..PresentationLimits::default()
                },
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::ArtifactLimit
        );
        let owned = estimated_owned_bytes(
            &document,
            &exact.model.dependencies,
            &exact.model.materials,
            false,
        )
        .unwrap();
        assert!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_owned_bytes: owned,
                    ..PresentationLimits::default()
                },
            )
            .is_ok()
        );
        let compact = build_presentation_from_complete_responses(
            &document,
            PresentationProfile::World,
            PresentationLimits {
                max_owned_bytes: owned - 1,
                ..PresentationLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            u16::from_le_bytes(compact.bytes[4..6].try_into().unwrap()),
            ARTIFACT_VERSION
        );
        let compact_owned = estimated_owned_bytes(
            &document,
            &exact.model.dependencies,
            &exact.model.materials,
            true,
        )
        .unwrap();
        assert!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_owned_bytes: compact_owned,
                    ..PresentationLimits::default()
                },
            )
            .is_ok()
        );
        assert_eq!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_owned_bytes: compact_owned - 1,
                    ..PresentationLimits::default()
                },
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::ModelLimit
        );
        assert_eq!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_dependency_bytes: b"patch-vmt".len() - 1,
                    ..PresentationLimits::default()
                },
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::DependencyLimit
        );
        let aggregate = exact
            .model
            .dependencies
            .iter()
            .map(|dependency| dependency.byte_length)
            .sum::<usize>();
        assert!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_aggregate_dependency_bytes: aggregate,
                    ..PresentationLimits::default()
                },
            )
            .is_ok()
        );
        assert_eq!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_aggregate_dependency_bytes: aggregate - 1,
                    ..PresentationLimits::default()
                },
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::DependencyLimit
        );
        assert!(matches!(
            build_presentation(
                &document,
                PresentationProfile::World,
                &[],
                PresentationLimits {
                    max_animation_samples: 4,
                    ..PresentationLimits::default()
                },
                &CancellationToken::default(),
            )
            .unwrap(),
            PresentationBuild::Needs(_)
        ));
        assert_eq!(
            build_presentation(
                &document,
                PresentationProfile::World,
                &[],
                PresentationLimits {
                    max_animation_samples: 3,
                    ..PresentationLimits::default()
                },
                &CancellationToken::default(),
            )
            .unwrap_err()
            .code,
            PresentationErrorCode::ModelLimit
        );
    }

    fn build_presentation_from_complete_responses(
        document: &Document,
        profile: PresentationProfile,
        limits: PresentationLimits,
    ) -> Result<PresentationArtifact, PresentationError> {
        let PresentationBuild::Needs(candidates) = build_presentation(
            document,
            profile,
            &[],
            limits,
            &CancellationToken::default(),
        )?
        else {
            unreachable!()
        };
        let manifest = MaterialResolutionManifest {
            root_identity: candidates[1].logical_path.clone(),
            include_sources: vec![MaterialSourceManifest {
                requester: candidates[1].logical_path.clone(),
                logical_path: "materials/models/test/base.vmt".to_owned(),
            }],
            textures: vec![
                MaterialTextureManifest {
                    role: TextureRole::Base,
                    parameter: b"$basetexture".to_vec(),
                    logical_path: Some("materials/models/test/base.vtf".to_owned()),
                    disposition: TextureDisposition::Source,
                    selected: true,
                },
                MaterialTextureManifest {
                    role: TextureRole::Environment,
                    parameter: b"$envmap".to_vec(),
                    logical_path: None,
                    disposition: TextureDisposition::BuiltInEnvironment,
                    selected: false,
                },
            ],
        };
        let mut responses = vec![
            response(&candidates[0], None, None),
            response(&candidates[1], Some(b"patch-vmt"), Some(manifest)),
        ];
        let PresentationBuild::Needs(closure) = build_presentation(
            document,
            profile,
            &responses,
            limits,
            &CancellationToken::default(),
        )?
        else {
            unreachable!()
        };
        responses.extend([
            response(&closure[0], Some(b"base-vmt"), None),
            response(&closure[1], Some(b"base-vtf"), None),
        ]);
        let PresentationBuild::Complete(artifact) = build_presentation(
            document,
            profile,
            &responses,
            limits,
            &CancellationToken::default(),
        )?
        else {
            unreachable!()
        };
        Ok(*artifact)
    }
}
