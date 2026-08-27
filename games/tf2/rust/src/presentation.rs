use playsrc_material::{HdrMode, SelectionEnvironment, TextureDisposition, TextureRole};
use playsrc_studio_model::{
    CancellationToken, DependencyResponse, DependencyRole, Document, Limits, Load,
    MaterialResolutionManifest, MaterialSourceManifest, MaterialTextureManifest,
    PresentationDependencyResponse, PresentationDependencyRole, PresentationLimits,
    PresentationModel, PresentationModelBuild, PresentationProfile, Profile,
    TextureDisposition as StudioTextureDisposition, TextureRole as StudioTextureRole, VtxVariant,
    build_presentation_model, load_authored, retain_authored_source,
};
use std::{borrow::Cow, collections::BTreeMap};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelPresentationError {
    Missing,
    Invalid,
}

pub struct BuiltModelPresentation {
    pub model: Box<PresentationModel>,
    pub illumination_position: playsrc_studio_model::Vector3,
    pub illumination_attachment: i32,
    pub eyes: Vec<playsrc_studio_model::EyeDefinition>,
    pub flex: std::sync::Arc<playsrc_studio_model::ModelFlex>,
}

pub fn build_model(
    identity: &str,
    resources: &BTreeMap<String, &[u8]>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    integer_hdr: bool,
    profile: PresentationProfile,
) -> Result<BuiltModelPresentation, ModelPresentationError> {
    let document = load_model(identity, resources, resource_hashes)?;
    let mut responses = Vec::new();
    loop {
        match build_presentation_model(
            &document,
            profile,
            &responses,
            PresentationLimits::default(),
            &CancellationToken::default(),
        )
        .map_err(|_| ModelPresentationError::Invalid)?
        {
            PresentationModelBuild::Complete(model) => {
                return Ok(BuiltModelPresentation {
                    model,
                    illumination_position: document.bounds.illumination,
                    illumination_attachment: document.illumination_attachment,
                    eyes: playsrc_studio_model::eye_definitions(&document)
                        .map_err(|_| ModelPresentationError::Invalid)?,
                    flex: std::sync::Arc::new(playsrc_studio_model::read_model_flex(resources.get(identity).ok_or(ModelPresentationError::Missing)?)
                        .map_err(|_| ModelPresentationError::Invalid)?),
                });
            }
            PresentationModelBuild::Needs(requests) => {
                for request in requests {
                    let path = request.logical_path.to_ascii_lowercase();
                    let source = resources.get(&path);
                    let bytes = source.map(|_| Vec::new());
                    let material = if request.role == PresentationDependencyRole::MaterialCandidate
                        && bytes.is_some()
                    {
                        Some(material_manifest(
                            &request.logical_path,
                            resources,
                            integer_hdr,
                        )?)
                    } else {
                        None
                    };
                    let sha256 = source.map(|_| {
                        *resource_hashes
                            .get(&path)
                            .expect("resource hash set is complete")
                    });
                    responses.push(PresentationDependencyResponse {
                        requester: request.requester,
                        role: request.role,
                        logical_path: request.logical_path,
                        material_slot: request.material_slot,
                        texture_role: request.texture_role,
                        bytes,
                        verified_byte_length: source.map(|bytes| bytes.len()),
                        sha256,
                        material,
                    });
                }
            }
        }
    }
}

fn model_profile(bytes: &[u8]) -> Result<Profile, ModelPresentationError> {
    let version = i32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or(ModelPresentationError::Invalid)?
            .try_into()
            .map_err(|_| ModelPresentationError::Invalid)?,
    );
    match version {
        44 => Ok(Profile::SourcePcMdl44),
        45 => Ok(Profile::SourcePcMdl45),
        46 => Ok(Profile::SourcePcMdl46),
        47 => Ok(Profile::SourcePcMdl47),
        48 => Ok(Profile::SourcePcMdl48),
        _ => Err(ModelPresentationError::Invalid),
    }
}

fn load_model(
    identity: &str,
    resources: &BTreeMap<String, &[u8]>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
) -> Result<Box<Document>, ModelPresentationError> {
    let path = identity.to_ascii_lowercase();
    let bytes = *resources
        .get(&path)
        .ok_or(ModelPresentationError::Missing)?;
    let sha256 = *resource_hashes
        .get(&path)
        .ok_or(ModelPresentationError::Invalid)?;
    let mdl = retain_authored_source(&path, bytes, sha256);
    let mut responses = Vec::new();
    loop {
        match load_authored(
            identity,
            model_profile(&mdl)?,
            VtxVariant::Dx90,
            mdl.clone(),
            &responses,
            resource_hashes,
            Limits::default(),
        )
        .map_err(|_| ModelPresentationError::Invalid)?
        {
            Load::Complete(document) => return Ok(document),
            Load::Needs(requests) => {
                for request in requests {
                    let path = request.logical_path.to_ascii_lowercase();
                    let bytes = resources.get(&path).copied().map(Cow::Borrowed);
                    if request.role != DependencyRole::Physics && bytes.is_none() {
                        return Err(ModelPresentationError::Missing);
                    }
                    responses.push(DependencyResponse {
                        requester: request.requester,
                        role: request.role,
                        logical_path: path,
                        bytes,
                    });
                }
            }
        }
    }
}

fn dependency_path(token: &[u8]) -> Result<String, ModelPresentationError> {
    let value = std::str::from_utf8(token)
        .map_err(|_| ModelPresentationError::Invalid)?
        .replace('\\', "/");
    let mut path = if value.to_ascii_lowercase().starts_with("materials/") {
        value
    } else {
        format!("materials/{value}")
    };
    if !path.to_ascii_lowercase().ends_with(".vmt") {
        path.push_str(".vmt");
    }
    Ok(path.to_ascii_lowercase())
}

fn material_manifest(
    identity: &str,
    resources: &BTreeMap<String, &[u8]>,
    integer_hdr: bool,
) -> Result<MaterialResolutionManifest, ModelPresentationError> {
    let identity = identity.to_ascii_lowercase();
    let root = *resources
        .get(&identity)
        .ok_or(ModelPresentationError::Missing)?;
    let mut responses = Vec::new();
    let mut include_sources = Vec::new();
    let material = loop {
        match playsrc_vmt::compose(
            root,
            identity.clone(),
            &responses,
            &playsrc_keyvalues::ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|_| ModelPresentationError::Invalid)?
        {
            playsrc_vmt::Composition::Complete(document) => {
                break playsrc_material::resolve_for_environment(
                    &document,
                    SelectionEnvironment {
                        hdr_mode: if integer_hdr {
                            HdrMode::Integer
                        } else {
                            HdrMode::None
                        },
                        model: true,
                        ..SelectionEnvironment::default()
                    },
                )
                .map_err(|_| ModelPresentationError::Invalid)?;
            }
            playsrc_vmt::Composition::Needs(requests) => {
                for request in requests {
                    let path = dependency_path(&request.target_token)?;
                    include_sources.push(MaterialSourceManifest {
                        requester: request.parent_identity.clone(),
                        logical_path: path.clone(),
                    });
                    responses.push(playsrc_vmt::DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: path.clone(),
                        bytes: Some(
                            resources
                                .get(&path)
                                .ok_or(ModelPresentationError::Missing)?
                                .to_vec(),
                        ),
                    });
                }
            }
        }
    };
    let textures = material
        .textures
        .iter()
        .filter_map(|texture| {
            Some(MaterialTextureManifest {
                role: studio_texture_role(texture.role)?,
                parameter: texture.parameter.clone(),
                logical_path: texture
                    .logical_path
                    .as_ref()
                    .map(|value| value.to_ascii_lowercase()),
                disposition: match texture.disposition {
                    TextureDisposition::Source => StudioTextureDisposition::Source,
                    TextureDisposition::BuiltInEnvironment => {
                        StudioTextureDisposition::BuiltInEnvironment
                    }
                    TextureDisposition::BuiltInRenderTarget => {
                        StudioTextureDisposition::BuiltInRenderTarget
                    }
                },
                selected: material.selected_textures.contains(&texture.role),
            })
        })
        .collect();
    Ok(MaterialResolutionManifest {
        root_identity: identity,
        include_sources,
        textures,
    })
}

fn studio_texture_role(role: TextureRole) -> Option<StudioTextureRole> {
    use StudioTextureRole as Target;
    use TextureRole as Source;
    Some(match role {
        Source::Base => Target::Base,
        Source::HdrBase => Target::HdrBase,
        Source::HdrCompressed => Target::HdrCompressed,
        Source::HdrCompressed0 => Target::HdrCompressed0,
        Source::HdrCompressed1 => Target::HdrCompressed1,
        Source::HdrCompressed2 => Target::HdrCompressed2,
        Source::Base2 => Target::Base2,
        Source::Bump => Target::Bump,
        Source::Normal => Target::Normal,
        Source::Bump2 => Target::Bump2,
        Source::Detail => Target::Detail,
        Source::BlendModulate => Target::BlendModulate,
        Source::Environment => Target::Environment,
        Source::EnvironmentMask => Target::EnvironmentMask,
        Source::SelfIllumMask => Target::SelfIllumMask,
        Source::Flow => Target::Flow,
        Source::Reflection | Source::Refraction => return None,
    })
}
