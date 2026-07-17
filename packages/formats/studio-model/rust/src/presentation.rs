//! Runtime-neutral StudioModel presentation artifacts and pose sampling.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    Animation, Attachment, BodyPart, Document, Float32, GeometryPrimitive, SkinFamily, Vector3,
};

const ARTIFACT_MAGIC: &[u8; 4] = b"PSMP";
const ARTIFACT_VERSION: u16 = 1;
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
const STUDIO_LOOPING: i32 = 0x0001;
const STUDIO_DELTA: i32 = 0x0004;
const STUDIO_CYCLE_POSE: i32 = 0x0080;
const STUDIO_REALTIME: i32 = 0x0100;
const STUDIO_OVERRIDE: i32 = 0x0800;

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
    pub identity: String,
    pub checksum: i32,
    pub basis: ModelBasis,
    pub dependencies: Vec<ArtifactDependency>,
    pub base_material_count: usize,
    pub materials: Vec<PresentationMaterial>,
    pub bones: Vec<crate::Bone>,
    pub animations: Vec<Animation>,
    pub sequences: Vec<crate::Sequence>,
    pub pose_parameters: Vec<crate::PoseParameter>,
    pub attachments: Vec<Attachment>,
    pub skins: Vec<SkinFamily>,
    pub body_parts: Vec<PresentationBodyPart>,
    pub geometry: Vec<GeometryPrimitive>,
    pub unsupported_features: Vec<FeatureSupport>,
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
                Some(response) if response.bytes.is_some() => {
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
        return Ok(PresentationBuild::Needs(candidate_needs));
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
        return Ok(PresentationBuild::Needs(closure_needs));
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
            if response.bytes.is_none() {
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
                if response.bytes.is_none() {
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
    let aggregate_dependency_bytes = dependencies.iter().try_fold(0_usize, |total, dependency| {
        total.checked_add(dependency.byte_length)
    });
    if aggregate_dependency_bytes.is_none_or(|bytes| bytes > limits.max_aggregate_dependency_bytes)
    {
        return Err(presentation_error(
            PresentationErrorCode::DependencyLimit,
            &document.identity,
        ));
    }
    if estimated_owned_bytes(document, &dependencies, &materials)
        .is_none_or(|bytes| bytes > limits.max_owned_bytes)
    {
        return Err(presentation_error(
            PresentationErrorCode::ModelLimit,
            &document.identity,
        ));
    }

    let model = PresentationModel {
        profile,
        identity: document.identity.clone(),
        checksum: document.checksum,
        basis: ModelBasis::source(),
        dependencies,
        base_material_count: document.materials.len(),
        materials,
        bones: document.bones.clone(),
        animations: document.animations.clone(),
        sequences: document.sequences.clone(),
        pose_parameters: document.pose_parameters.clone(),
        attachments: document.attachments.clone(),
        skins: document.skins.clone(),
        body_parts: presentation_body_parts(&document.body_parts),
        geometry: document.geometry.clone(),
        unsupported_features: feature_support(document),
    };
    check_cancelled(cancellation, &document.identity)?;
    validate_decoded_model(&model, limits)?;
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
            model,
            bytes,
            sha256,
        },
    )))
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
                animation
                    .frames
                    .len()
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
    let bytes = response.bytes.as_ref().ok_or_else(|| {
        presentation_error(
            PresentationErrorCode::MissingDependency,
            &response.logical_path,
        )
    })?;
    if bytes.len() > limits.max_dependency_bytes {
        return Err(presentation_error(
            PresentationErrorCode::DependencyLimit,
            &response.logical_path,
        ));
    }
    let expected = response.sha256.ok_or_else(|| {
        presentation_error(PresentationErrorCode::HashMismatch, &response.logical_path)
    })?;
    if sha256(bytes) != expected {
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
    let bytes = response.bytes.as_ref().ok_or_else(|| {
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
        byte_length: bytes.len(),
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
    [
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
    .to_vec()
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
    if state.pose_parameters.len() != model.pose_parameters.len() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let mut local = sample_sequence(
        model,
        state.base_sequence,
        state.cycle,
        &state.pose_parameters,
    )?;
    let base_sequence = model
        .sequences
        .get(state.base_sequence)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if base_sequence.flags & STUDIO_DELTA != 0 {
        let bind = bind_pose(model);
        local = apply_delta(&bind, &local, 1.0);
    }
    for layer in &state.layers {
        let weight = finite_unit(layer.weight).ok_or_else(|| {
            presentation_error(PresentationErrorCode::InvalidState, &model.identity)
        })?;
        let sampled = sample_sequence(model, layer.sequence, layer.cycle, &state.pose_parameters)?;
        let sequence = model.sequences.get(layer.sequence).ok_or_else(|| {
            presentation_error(PresentationErrorCode::InvalidState, &model.identity)
        })?;
        local = if sequence.flags & STUDIO_DELTA != 0 {
            apply_delta(&local, &sampled, weight)
        } else {
            blend_pose(&local, &sampled, weight)
        };
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
                model_transform: multiply_matrix(bone, &Matrix3x4(attachment.local)),
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

fn sample_sequence(
    model: &PresentationModel,
    sequence_index: usize,
    cycle: Float32,
    pose_parameters: &[Float32],
) -> Result<LocalPose, PresentationError> {
    let sequence = model
        .sequences
        .get(sequence_index)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if sequence.flags & STUDIO_REALTIME != 0 {
        return Err(presentation_error(
            PresentationErrorCode::UnsupportedState,
            &model.identity,
        ));
    }
    let mut cycle = finite(cycle)
        .ok_or_else(|| presentation_error(PresentationErrorCode::InvalidState, &model.identity))?;
    if sequence.flags & STUDIO_CYCLE_POSE != 0 {
        cycle = usize::try_from(sequence.cycle_pose_parameter)
            .ok()
            .and_then(|index| pose_parameters.get(index))
            .and_then(|value| finite(*value))
            .ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
    }
    cycle = normalized_cycle(cycle, sequence.flags & STUDIO_LOOPING != 0);
    let (index0, setting0) = local_pose_setting(model, sequence, 0, pose_parameters)?;
    let (index1, setting1) = local_pose_setting(model, sequence, 1, pose_parameters)?;
    let x_next = usize::from(sequence.blend_size[0] > 1);
    let y_next = usize::from(sequence.blend_size[1] > 1);
    let sample = |x, y| sample_grid_animation(model, sequence, x, y, cycle);
    let mut blended = if setting0 < 0.001 {
        if setting1 < 0.001 {
            sample(index0, index1)?
        } else if setting1 > 0.999 {
            sample(index0, index1 + y_next)?
        } else {
            blend_pose(
                &sample(index0, index1)?,
                &sample(index0, index1 + y_next)?,
                setting1,
            )
        }
    } else if setting0 > 0.999 {
        if setting1 < 0.001 {
            sample(index0 + x_next, index1)?
        } else if setting1 > 0.999 {
            sample(index0 + x_next, index1 + y_next)?
        } else {
            blend_pose(
                &sample(index0 + x_next, index1)?,
                &sample(index0 + x_next, index1 + y_next)?,
                setting1,
            )
        }
    } else if setting1 < 0.001 {
        blend_pose(
            &sample(index0, index1)?,
            &sample(index0 + x_next, index1)?,
            setting0,
        )
    } else if setting1 > 0.999 {
        blend_pose(
            &sample(index0, index1 + y_next)?,
            &sample(index0 + x_next, index1 + y_next)?,
            setting0,
        )
    } else {
        let (coordinates, weights) = three_way_blend(index0, index1, setting0, setting1);
        let first = sample(coordinates[0].0, coordinates[0].1)?;
        let third = sample(coordinates[2].0, coordinates[2].1)?;
        if weights[1] < 0.001 {
            blend_pose(&first, &third, weights[2] / (weights[0] + weights[2]))
        } else {
            let second = sample(coordinates[1].0, coordinates[1].1)?;
            let first_second = blend_pose(&first, &second, weights[1] / (weights[0] + weights[1]));
            blend_pose(&first_second, &third, weights[2])
        }
    };
    for (bone, authored_weight) in sequence.bone_weights.iter().enumerate() {
        let weight = finite(*authored_weight)
            .filter(|value| (0.0..=1.0).contains(value))
            .ok_or_else(|| {
                presentation_error(PresentationErrorCode::InvalidState, &model.identity)
            })?;
        if sequence.flags & STUDIO_DELTA != 0 {
            blended.0[bone] = scale_vector(blended.0[bone], weight);
            blended.1[bone] = nlerp(identity_quaternion(), blended.1[bone], weight);
        } else {
            blended.0[bone] = lerp_vector(model.bones[bone].position, blended.0[bone], weight);
            blended.1[bone] = nlerp(model.bones[bone].quaternion, blended.1[bone], weight);
        }
    }
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
    if animation.frames.is_empty() {
        return Err(presentation_error(
            PresentationErrorCode::InvalidState,
            &model.identity,
        ));
    }
    let frame = cycle * (animation.frames.len() - 1) as f32;
    let first = frame.floor() as usize;
    let second = (first + 1).min(animation.frames.len() - 1);
    let fraction = frame - first as f32;
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
        let first_frame = animation.frames[first]
            .translations
            .get(local_bone)
            .zip(animation.frames[first].rotations.get(local_bone));
        let second_frame = animation.frames[second]
            .translations
            .get(local_bone)
            .zip(animation.frames[second].rotations.get(local_bone));
        let (Some((first_position, first_rotation)), Some((second_position, second_rotation))) =
            (first_frame, second_frame)
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

fn blend_pose(left: &LocalPose, right: &LocalPose, weight: f32) -> LocalPose {
    (
        left.0
            .iter()
            .zip(&right.0)
            .map(|(left, right)| lerp_vector(*left, *right, weight))
            .collect(),
        left.1
            .iter()
            .zip(&right.1)
            .map(|(left, right)| nlerp(*left, *right, weight))
            .collect(),
    )
}

fn apply_delta(base: &LocalPose, delta: &LocalPose, weight: f32) -> LocalPose {
    (
        base.0
            .iter()
            .zip(&delta.0)
            .map(|(base, delta)| add_vector(*base, scale_vector(*delta, weight)))
            .collect(),
        base.1
            .iter()
            .zip(&delta.1)
            .map(|(base, delta)| {
                quaternion_multiply(*base, nlerp(identity_quaternion(), *delta, weight))
            })
            .collect(),
    )
}

fn finite(value: Float32) -> Option<f32> {
    let value = f32::from_bits(value.0);
    value.is_finite().then_some(value)
}

fn finite_unit(value: Float32) -> Option<f32> {
    finite(value).filter(|value| (0.0..=1.0).contains(value))
}

fn vector(values: [f32; 3]) -> Vector3 {
    Vector3(values.map(|value| Float32(value.to_bits())))
}

fn values3(value: Vector3) -> [f32; 3] {
    value.0.map(|component| f32::from_bits(component.0))
}

fn values4(value: [Float32; 4]) -> [f32; 4] {
    value.map(|component| f32::from_bits(component.0))
}

fn identity_quaternion() -> [Float32; 4] {
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

fn quaternion_matrix(rotation: [Float32; 4], translation: Vector3) -> Matrix3x4 {
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

fn multiply_matrix(left: &Matrix3x4, right: &Matrix3x4) -> Matrix3x4 {
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
    output.text(&model.identity, limits.max_string_bytes)?;
    output.vector(model.basis.forward)?;
    output.vector(model.basis.left)?;
    output.vector(model.basis.up)?;
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
        for section in &animation.sections {
            output.index(section.index)?;
            output.index(section.first_frame)?;
            output.index(section.frame_count)?;
            output.i32(section.block)?;
            output.i32(section.data_offset)?;
            output.count(section.tracks.len())?;
            for track in &section.tracks {
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
    output.count(model.unsupported_features.len())?;
    for feature in &model.unsupported_features {
        output.u8(feature_family_code(feature.family))?;
        output.u8(match feature.disposition {
            FeatureDisposition::NotPresent => 0,
            FeatureDisposition::RetainedNotEvaluated => 1,
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

pub(crate) fn content_sha256(bytes: &[u8]) -> [u8; 32] {
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
    if input.raw(4)? != ARTIFACT_MAGIC || input.u16()? != ARTIFACT_VERSION {
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
    let identity = input.text(limits.max_string_bytes)?;
    let basis = ModelBasis {
        forward: input.vector()?,
        left: input.vector()?,
        up: input.vector()?,
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
        let encoded_frames = input.count(limits.max_animation_samples)?;
        let mut frames = Vec::with_capacity(encoded_frames);
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
            ik_lock_count: input.i32()?,
            cycle_pose_parameter: input.i32()?,
            activity_modifier_count: input.i32()?,
            source_identity: input.text(limits.max_string_bytes)?,
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
    let mut unsupported_features = Vec::new();
    for _ in 0..input.count(9)? {
        unsupported_features.push(FeatureSupport {
            family: decode_feature_family(input.u8()?).ok_or_else(|| input.error())?,
            disposition: match input.u8()? {
                0 => FeatureDisposition::NotPresent,
                1 => FeatureDisposition::RetainedNotEvaluated,
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
        identity: identity.clone(),
        checksum,
        basis,
        dependencies,
        base_material_count,
        materials,
        bones,
        animations,
        sequences,
        pose_parameters,
        attachments,
        skins,
        body_parts,
        geometry,
        unsupported_features,
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
        || model.dependencies.len() > limits.max_dependencies
        || model.base_material_count > model.materials.len()
        || model.materials.len() > limits.max_materials
        || model.bones.len() > limits.max_bones
        || model.animations.len() > limits.max_animations
        || model.sequences.len() > limits.max_sequences
        || model.unsupported_features.len() != 9
    {
        return Err(invalid());
    }
    let mut aggregate_dependency_bytes = 0_usize;
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
        aggregate_dependency_bytes = aggregate_dependency_bytes
            .checked_add(dependency.byte_length)
            .filter(|bytes| *bytes <= limits.max_aggregate_dependency_bytes)
            .ok_or_else(invalid)?;
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
        if animation.index != index
            || animation.frame_count <= 0
            || animation.frames.len() != animation.frame_count as usize
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
            .checked_add(
                animation
                    .frames
                    .len()
                    .saturating_mul(animation.bone_map.len()),
            )
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
        } else {
            populated
                && sequence.animation_indices.len() == sequence.blend_count as usize
                && sequence.bone_weights.len() == model.bones.len()
        };
        if sequence.index != index
            || (!empty_override && !populated)
            || !children_valid
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
        .unsupported_features
        .iter()
        .map(|feature| feature.family)
        .ne(expected)
        || model.unsupported_features.iter().any(|feature| {
            (feature.records == 0) != (feature.disposition == FeatureDisposition::NotPresent)
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
            bounds: Bounds {
                eye: vector([0.0; 3]),
                illumination: vector([0.0; 3]),
                hull_min: vector([-1.0; 3]),
                hull_max: vector([1.0; 3]),
                view_min: vector([-1.0; 3]),
                view_max: vector([1.0; 3]),
            },
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
            sha256: bytes.map(sha256),
            material,
        }
    }

    fn build() -> PresentationArtifact {
        build_profile(PresentationProfile::World)
    }

    fn build_profile(profile: PresentationProfile) -> PresentationArtifact {
        let document = document();
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
    fn emits_exact_material_closure_and_round_trips_byte_identical_artifacts() {
        let first = build();
        let second = build();
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(
            first.sha256,
            hex_hash("f8cc817e20bfaba3c069d2cfd1d7cbd74564b18a56027d9c7425f452d0132613")
        );
        assert_eq!(first.bytes.len(), 3_492);
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
    fn classifies_every_unsupported_presentation_family() {
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
        assert!(support.iter().all(|feature| {
            feature.disposition == FeatureDisposition::RetainedNotEvaluated && feature.records > 0
        }));
        assert_eq!(support[5].records, 4);
        assert_eq!(support[6].records, 7);
        assert_eq!(support[7].records, 3);
        assert_eq!(support[8].records, 1);
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
        let owned =
            estimated_owned_bytes(&document, &exact.model.dependencies, &exact.model.materials)
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
        assert_eq!(
            build_presentation_from_complete_responses(
                &document,
                PresentationProfile::World,
                PresentationLimits {
                    max_owned_bytes: owned - 1,
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
