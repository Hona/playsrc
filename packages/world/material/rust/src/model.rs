use crate::{
    Error, ErrorCode, HdrMode, SelectionEnvironment, TextureColorRead, TextureDisposition,
    TextureRequest, TextureRole, boolean, color_or, error, float_or, get, integer_or, logical_path,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelShader {
    VertexLitGeneric,
    EyeRefract,
    Eyes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelTextureRole {
    Albedo,
    WrinkleCompress,
    WrinkleStretch,
    BumpCompress,
    BumpStretch,
    PhongExponent,
    LightWarp,
    PhongWarp,
    EyeIris,
    EyeCornea,
    EyeAmbientOcclusion,
    EyeGlint,
    SheenEnvironment,
    SheenMask,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelTextureRequest {
    pub role: ModelTextureRole,
    pub parameter: Vec<u8>,
    pub reference: Vec<u8>,
    pub logical_path: String,
    pub color_read: TextureColorRead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhongMaskSource {
    BaseAlpha,
    NormalAlpha,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RimLightState {
    pub exponent: f32,
    pub boost: f32,
    pub exponent_texture_alpha_mask: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhongState {
    pub exponent: f32,
    pub exponent_texture: Option<ModelTextureRequest>,
    pub exponent_factor: f32,
    pub tint: [f32; 3],
    pub albedo_tint: bool,
    pub boost: f32,
    pub fresnel_ranges: [f32; 3],
    pub packed_fresnel_ranges: [f32; 3],
    pub warp_texture: Option<ModelTextureRequest>,
    pub mask_source: PhongMaskSource,
    pub invert_mask: bool,
    pub rim: Option<RimLightState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelfIllumMaskSource {
    BaseAlpha,
    Texture,
    EnvironmentMaskAlpha,
    Fresnel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfIllumState {
    pub source: SelfIllumMaskSource,
    pub tint: [f32; 3],
    pub fresnel_min_max_exponent: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloakState {
    pub enabled: bool,
    pub factor: f32,
    pub color_tint: [f32; 3],
    pub refract_amount: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheenState {
    pub enabled: bool,
    pub environment: Option<ModelTextureRequest>,
    pub mask: Option<ModelTextureRequest>,
    pub mask_frame: i32,
    pub tint: [f32; 3],
    pub mask_scale: [f32; 2],
    pub mask_offset: [f32; 2],
    pub mask_direction: i32,
    pub shader_index: i32,
    pub source_alpha_blend: bool,
    pub depth_write: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelVertexRequirements {
    pub position: bool,
    pub normal: bool,
    pub tangent_space: bool,
    pub texture_coordinate_0: bool,
    pub ambient_cube: bool,
    pub local_lights: bool,
    pub camera_position: bool,
    pub studio_eye_parameters: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VertexLitGenericState {
    pub base: Option<TextureRequest>,
    pub bump: Option<TextureRequest>,
    pub diffuse_warp: Option<ModelTextureRequest>,
    pub half_lambert: bool,
    pub self_illumination: Option<SelfIllumState>,
    pub phong: Option<PhongState>,
    pub cloak: CloakState,
    pub sheen: SheenState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EyeRefractState {
    pub iris: Option<ModelTextureRequest>,
    pub cornea: Option<ModelTextureRequest>,
    pub ambient_occlusion: Option<ModelTextureRequest>,
    pub environment: Option<TextureRequest>,
    pub diffuse_warp: Option<ModelTextureRequest>,
    pub dilation: f32,
    pub glossiness: f32,
    pub sphere_texture_kill: bool,
    pub raytrace_sphere: bool,
    pub parallax_strength: f32,
    pub cornea_bump_strength: f32,
    pub ambient_occlusion_color: [f32; 3],
    pub eyeball_radius: f32,
    pub half_lambert: bool,
    pub cloak: CloakState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EyesState {
    pub base: Option<TextureRequest>,
    pub iris: Option<ModelTextureRequest>,
    pub glint: Option<ModelTextureRequest>,
    pub dilation: f32,
    pub half_lambert: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModelShaderState {
    VertexLitGeneric(VertexLitGenericState),
    EyeRefract(EyeRefractState),
    Eyes(EyesState),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelMaterialState {
    pub shader: ModelShader,
    pub state: ModelShaderState,
    pub vertex_requirements: ModelVertexRequirements,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredTextureBinding {
    pub role: TextureBindingRole,
    pub logical_path: String,
    pub color_read: TextureColorRead,
    pub sampling: TextureSamplingState,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u8,
    pub frame_count: u16,
    pub faces: Vec<TextureFace>,
    pub subresources: Vec<TextureSubresourceIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureBindingRole {
    Material(TextureRole),
    Model(ModelTextureRole),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureFace {
    Right,
    Left,
    Back,
    Front,
    Up,
    Down,
    Sphere,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureSubresourceIdentity {
    pub mip: u8,
    pub frame: u16,
    pub face: TextureFace,
    pub slice: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureWrapMode {
    Repeat,
    Clamp,
    Border,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureMinFilter {
    Nearest,
    Linear,
    LinearMipmapNearest,
    LinearMipmapLinear,
    Anisotropic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureMagFilter {
    Nearest,
    Linear,
    Anisotropic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureSamplingState {
    pub wrap_s: TextureWrapMode,
    pub wrap_t: TextureWrapMode,
    pub wrap_u: TextureWrapMode,
    pub min_filter: TextureMinFilter,
    pub mag_filter: TextureMagFilter,
    pub anisotropy_level: u8,
    pub mipmapped: bool,
    pub no_lod: bool,
    pub all_mips: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureMetadataManifest {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_count: u8,
    pub frame_count: u16,
    pub faces: Vec<TextureFace>,
    pub sampling: TextureSamplingState,
    pub subresources: Vec<TextureSubresourceIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoredTexturePlane {
    pub identity: TextureSubresourceIdentity,
    pub width: u32,
    pub height: u32,
    pub row_stride: usize,
    pub sample_bytes: usize,
}

pub fn bind_authored_texture(
    request: &TextureRequest,
    metadata: &TextureMetadataManifest,
) -> Result<AuthoredTextureBinding, Error> {
    bind_authored(
        TextureBindingRole::Material(request.role),
        &request.parameter,
        request.logical_path.as_deref(),
        request.disposition,
        request.color_read,
        metadata,
    )
}

pub fn bind_authored_model_texture(
    request: &ModelTextureRequest,
    metadata: &TextureMetadataManifest,
) -> Result<AuthoredTextureBinding, Error> {
    bind_authored(
        TextureBindingRole::Model(request.role),
        &request.parameter,
        Some(&request.logical_path),
        TextureDisposition::Source,
        request.color_read,
        metadata,
    )
}

fn bind_authored(
    role: TextureBindingRole,
    parameter: &[u8],
    logical_path: Option<&str>,
    disposition: TextureDisposition,
    color_read: TextureColorRead,
    metadata: &TextureMetadataManifest,
) -> Result<AuthoredTextureBinding, Error> {
    if disposition != TextureDisposition::Source {
        return Err(error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
    }
    let logical_path = logical_path
        .map(str::to_owned)
        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(parameter.to_vec())))?;
    let expected = expected_subresources(metadata)
        .ok_or_else(|| error(ErrorCode::InvalidTextureMetadata, Some(parameter.to_vec())))?;
    let anisotropic = metadata.sampling.min_filter == TextureMinFilter::Anisotropic
        || metadata.sampling.mag_filter == TextureMagFilter::Anisotropic;
    if (anisotropic && metadata.sampling.anisotropy_level < 2)
        || (!anisotropic && metadata.sampling.anisotropy_level != 1)
    {
        return Err(error(
            ErrorCode::InvalidTextureMetadata,
            Some(parameter.to_vec()),
        ));
    }
    let actual = metadata.subresources.clone();
    if actual != expected {
        return Err(error(
            ErrorCode::InvalidTextureMetadata,
            Some(parameter.to_vec()),
        ));
    }
    Ok(AuthoredTextureBinding {
        role,
        logical_path,
        color_read,
        sampling: metadata.sampling,
        width: metadata.width,
        height: metadata.height,
        depth: metadata.depth,
        mip_count: metadata.mip_count,
        frame_count: metadata.frame_count,
        faces: metadata.faces.clone(),
        subresources: actual,
    })
}

pub fn validate_authored_planes(
    binding: &AuthoredTextureBinding,
    planes: &[AuthoredTexturePlane],
) -> Result<(), Error> {
    if planes.len() != binding.subresources.len()
        || planes
            .iter()
            .zip(&binding.subresources)
            .any(|(plane, identity)| {
                plane.identity != *identity
                    || plane.width != (binding_width(binding, identity.mip))
                    || plane.height != (binding_height(binding, identity.mip))
                    || plane.row_stride == 0
                    || plane.sample_bytes == 0
            })
    {
        return Err(error(ErrorCode::InvalidTextureMetadata, None));
    }
    Ok(())
}

fn binding_width(binding: &AuthoredTextureBinding, mip: u8) -> u32 {
    binding
        .width
        .checked_shr(u32::from(mip))
        .unwrap_or(0)
        .max(1)
}

fn binding_height(binding: &AuthoredTextureBinding, mip: u8) -> u32 {
    binding
        .height
        .checked_shr(u32::from(mip))
        .unwrap_or(0)
        .max(1)
}

fn expected_subresources(
    metadata: &TextureMetadataManifest,
) -> Option<Vec<TextureSubresourceIdentity>> {
    if metadata.width == 0
        || metadata.height == 0
        || metadata.depth == 0
        || metadata.mip_count == 0
        || metadata.frame_count == 0
        || metadata.faces.is_empty()
        || metadata
            .faces
            .iter()
            .enumerate()
            .any(|(index, face)| metadata.faces[..index].iter().any(|prior| prior == face))
    {
        return None;
    }
    let maximum_dimension = metadata.width.max(metadata.height).max(metadata.depth);
    let maximum_mips = u8::try_from(u32::BITS - maximum_dimension.leading_zeros()).ok()?;
    if metadata.mip_count > maximum_mips {
        return None;
    }
    let mut expected = Vec::new();
    for mip in (0..metadata.mip_count).rev() {
        let slices = (metadata.depth >> mip).max(1);
        for frame in 0..metadata.frame_count {
            for &face in &metadata.faces {
                for slice in 0..slices {
                    expected.push(TextureSubresourceIdentity {
                        mip,
                        frame,
                        face,
                        slice: u16::try_from(slice).ok()?,
                    });
                }
            }
        }
    }
    Some(expected)
}

pub(crate) fn resolve_model_state(
    shader: &[u8],
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[TextureRequest],
    environment: SelectionEnvironment,
) -> Result<(Option<ModelMaterialState>, Vec<ModelTextureRequest>), Error> {
    if shader.eq_ignore_ascii_case(b"VertexLitGeneric") {
        let model_textures = collect_model_textures(parameters, environment)?;
        Ok((
            Some(vertex_lit(
                parameters,
                textures,
                &model_textures,
                environment,
            )?),
            model_textures,
        ))
    } else if shader.eq_ignore_ascii_case(b"EyeRefract") {
        let model_textures = collect_model_textures(parameters, environment)?;
        Ok((
            Some(eye_refract(parameters, textures, &model_textures)?),
            model_textures,
        ))
    } else if shader.eq_ignore_ascii_case(b"Eyes") || shader.eq_ignore_ascii_case(b"Eyes_dx9") {
        let model_textures = collect_model_textures(parameters, environment)?;
        Ok((
            Some(eyes(parameters, textures, &model_textures)?),
            model_textures,
        ))
    } else {
        Ok((None, Vec::new()))
    }
}

fn vertex_lit(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[TextureRequest],
    model_textures: &[ModelTextureRequest],
    _environment: SelectionEnvironment,
) -> Result<ModelMaterialState, Error> {
    let bump = core_texture(textures, TextureRole::Bump).cloned();
    let diffuse_warp = model_texture(model_textures, ModelTextureRole::LightWarp).cloned();
    let phong_enabled = boolean(parameters, b"$phong");
    let exponent_texture = model_texture(model_textures, ModelTextureRole::PhongExponent).cloned();
    let base_alpha_mask = boolean(parameters, b"$basemapalphaphongmask");
    let normal_alpha_mask = boolean(parameters, b"$normalmapalphaenvmapmask");
    let phong = phong_enabled
        .then(|| {
            let ranges = color_or(parameters, b"$phongfresnelranges", [0.0, 0.5, 1.0])?;
            let rim_enabled = boolean(parameters, b"$rimlight");
            Ok(PhongState {
                exponent: float_or(parameters, b"$phongexponent", 5.0)?,
                exponent_texture,
                exponent_factor: float_or(parameters, b"$phongexponentfactor", 0.0)?,
                tint: color_or(parameters, b"$phongtint", [1.0; 3])?,
                albedo_tint: integer_or(parameters, b"$phongalbedotint", 1)? != 0,
                boost: float_or(parameters, b"$phongboost", 1.0)?,
                fresnel_ranges: ranges,
                packed_fresnel_ranges: [
                    (ranges[1] - ranges[0]) * 2.0,
                    ranges[1],
                    (ranges[2] - ranges[1]) * 2.0,
                ],
                warp_texture: model_texture(model_textures, ModelTextureRole::PhongWarp).cloned(),
                mask_source: if base_alpha_mask {
                    PhongMaskSource::BaseAlpha
                } else if bump.is_some() || normal_alpha_mask {
                    PhongMaskSource::NormalAlpha
                } else {
                    PhongMaskSource::None
                },
                invert_mask: boolean(parameters, b"$invertphongmask"),
                rim: rim_enabled
                    .then(|| {
                        Ok(RimLightState {
                            exponent: float_or(parameters, b"$rimlightexponent", 4.0)?.max(1.0),
                            boost: float_or(parameters, b"$rimlightboost", 1.0)?,
                            exponent_texture_alpha_mask: exponent_texture_is_masked(
                                parameters,
                                model_textures,
                            ),
                        })
                    })
                    .transpose()?,
            })
        })
        .transpose()?;
    let self_illumination = boolean(parameters, b"$selfillum")
        .then(|| {
            let source = if core_texture(textures, TextureRole::SelfIllumMask).is_some() {
                SelfIllumMaskSource::Texture
            } else if float_or(parameters, b"$selfillum_envmapmask_alpha", 0.0)? != 0.0 {
                SelfIllumMaskSource::EnvironmentMaskAlpha
            } else if boolean(parameters, b"$selfillumfresnel") {
                SelfIllumMaskSource::Fresnel
            } else {
                SelfIllumMaskSource::BaseAlpha
            };
            Ok(SelfIllumState {
                source,
                tint: color_or(parameters, b"$selfillumtint", [1.0; 3])?,
                fresnel_min_max_exponent: color_or(
                    parameters,
                    b"$selfillumfresnelminmaxexp",
                    [0.0, 1.0, 1.0],
                )?,
            })
        })
        .transpose()?;
    let cloak = cloak(parameters)?;
    let sheen = sheen(parameters, model_textures)?;
    let tangent_space =
        bump.is_some() || diffuse_warp.is_some() || phong.is_some() || sheen.enabled;
    let camera_position = phong_enabled
        || core_texture(textures, TextureRole::Environment).is_some()
        || self_illumination
            .as_ref()
            .is_some_and(|state| state.source == SelfIllumMaskSource::Fresnel);
    Ok(ModelMaterialState {
        shader: ModelShader::VertexLitGeneric,
        state: ModelShaderState::VertexLitGeneric(VertexLitGenericState {
            base: core_texture(textures, TextureRole::Base).cloned(),
            bump,
            diffuse_warp,
            half_lambert: boolean(parameters, b"$halflambert"),
            self_illumination,
            phong,
            cloak,
            sheen,
        }),
        vertex_requirements: ModelVertexRequirements {
            position: true,
            normal: true,
            tangent_space,
            texture_coordinate_0: true,
            ambient_cube: true,
            local_lights: true,
            camera_position,
            studio_eye_parameters: false,
        },
    })
}

fn eye_refract(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[TextureRequest],
    model_textures: &[ModelTextureRequest],
) -> Result<ModelMaterialState, Error> {
    Ok(ModelMaterialState {
        shader: ModelShader::EyeRefract,
        state: ModelShaderState::EyeRefract(EyeRefractState {
            iris: model_texture(model_textures, ModelTextureRole::EyeIris).cloned(),
            cornea: model_texture(model_textures, ModelTextureRole::EyeCornea).cloned(),
            ambient_occlusion: model_texture(model_textures, ModelTextureRole::EyeAmbientOcclusion)
                .cloned(),
            environment: core_texture(textures, TextureRole::Environment).cloned(),
            diffuse_warp: model_texture(model_textures, ModelTextureRole::LightWarp).cloned(),
            dilation: float_or(parameters, b"$dilation", 0.5)?,
            glossiness: float_or(parameters, b"$glossiness", 1.0)?,
            sphere_texture_kill: integer_or(parameters, b"$spheretexkillcombo", 0)? != 0,
            raytrace_sphere: integer_or(parameters, b"$raytracesphere", 0)? != 0,
            parallax_strength: float_or(parameters, b"$parallaxstrength", 0.25)?,
            cornea_bump_strength: float_or(parameters, b"$corneabumpstrength", 1.0)?,
            ambient_occlusion_color: color_or(parameters, b"$ambientocclcolor", [0.33; 3])?,
            eyeball_radius: float_or(parameters, b"$eyeballradius", 0.5)?,
            half_lambert: boolean(parameters, b"$halflambert"),
            cloak: cloak(parameters)?,
        }),
        vertex_requirements: ModelVertexRequirements {
            position: true,
            normal: true,
            tangent_space: false,
            texture_coordinate_0: true,
            ambient_cube: true,
            local_lights: true,
            camera_position: true,
            studio_eye_parameters: true,
        },
    })
}

fn eyes(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[TextureRequest],
    model_textures: &[ModelTextureRequest],
) -> Result<ModelMaterialState, Error> {
    Ok(ModelMaterialState {
        shader: ModelShader::Eyes,
        state: ModelShaderState::Eyes(EyesState {
            base: core_texture(textures, TextureRole::Base).cloned(),
            iris: model_texture(model_textures, ModelTextureRole::EyeIris).cloned(),
            glint: model_texture(model_textures, ModelTextureRole::EyeGlint).cloned(),
            dilation: float_or(parameters, b"$dilation", 0.0)?,
            half_lambert: boolean(parameters, b"$halflambert"),
        }),
        vertex_requirements: ModelVertexRequirements {
            position: true,
            normal: true,
            tangent_space: false,
            texture_coordinate_0: true,
            ambient_cube: true,
            local_lights: true,
            camera_position: true,
            studio_eye_parameters: true,
        },
    })
}

fn cloak(parameters: &BTreeMap<Vec<u8>, Vec<u8>>) -> Result<CloakState, Error> {
    Ok(CloakState {
        enabled: boolean(parameters, b"$cloakpassenabled"),
        factor: float_or(parameters, b"$cloakfactor", 0.0)?,
        color_tint: color_or(parameters, b"$cloakcolortint", [1.0; 3])?,
        refract_amount: float_or(parameters, b"$refractamount", 2.0)?,
    })
}

fn sheen(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[ModelTextureRequest],
) -> Result<SheenState, Error> {
    Ok(SheenState {
        enabled: boolean(parameters, b"$sheenpassenabled"),
        environment: model_texture(textures, ModelTextureRole::SheenEnvironment).cloned(),
        mask: model_texture(textures, ModelTextureRole::SheenMask).cloned(),
        mask_frame: integer_or(parameters, b"$sheenmapmaskframe", 0)?,
        tint: color_or(parameters, b"$sheenmaptint", [1.0; 3])?,
        mask_scale: [
            float_or(parameters, b"$sheenmapmaskscalex", 1.0)?,
            float_or(parameters, b"$sheenmapmaskscaley", 1.0)?,
        ],
        mask_offset: [
            float_or(parameters, b"$sheenmapmaskoffsetx", 0.0)?,
            float_or(parameters, b"$sheenmapmaskoffsety", 0.0)?,
        ],
        mask_direction: integer_or(parameters, b"$sheenmapmaskdirection", 0)?,
        shader_index: integer_or(parameters, b"$sheenindex", 0)?,
        source_alpha_blend: true,
        depth_write: true,
    })
}

fn exponent_texture_is_masked(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[ModelTextureRequest],
) -> bool {
    boolean(parameters, b"$rimmask")
        && model_texture(textures, ModelTextureRole::PhongExponent).is_some()
}

fn collect_model_textures(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    environment: SelectionEnvironment,
) -> Result<Vec<ModelTextureRequest>, Error> {
    let specs = [
        (b"$albedo".as_slice(), ModelTextureRole::Albedo),
        (b"$compress", ModelTextureRole::WrinkleCompress),
        (b"$stretch", ModelTextureRole::WrinkleStretch),
        (b"$bumpcompress", ModelTextureRole::BumpCompress),
        (b"$bumpstretch", ModelTextureRole::BumpStretch),
        (b"$phongexponenttexture", ModelTextureRole::PhongExponent),
        (b"$lightwarptexture", ModelTextureRole::LightWarp),
        (b"$phongwarptexture", ModelTextureRole::PhongWarp),
        (b"$iris", ModelTextureRole::EyeIris),
        (b"$corneatexture", ModelTextureRole::EyeCornea),
        (
            b"$ambientoccltexture",
            ModelTextureRole::EyeAmbientOcclusion,
        ),
        (b"$glint", ModelTextureRole::EyeGlint),
        (b"$sheenmap", ModelTextureRole::SheenEnvironment),
        (b"$sheenmapmask", ModelTextureRole::SheenMask),
    ];
    specs
        .into_iter()
        .filter_map(|(parameter, role)| {
            get(parameters, parameter).map(|reference| (parameter, role, reference))
        })
        .map(|(parameter, role, reference)| {
            let normalized = String::from_utf8(reference.clone())
                .map_err(|_| error(ErrorCode::InvalidPath, Some(parameter.to_vec())))?
                .replace('\\', "/");
            let color_read = match role {
                ModelTextureRole::Albedo
                | ModelTextureRole::WrinkleCompress
                | ModelTextureRole::WrinkleStretch
                | ModelTextureRole::EyeIris
                | ModelTextureRole::EyeAmbientOcclusion => TextureColorRead::Srgb,
                ModelTextureRole::SheenEnvironment | ModelTextureRole::SheenMask => {
                    if environment.hdr_mode == HdrMode::None {
                        TextureColorRead::Srgb
                    } else {
                        TextureColorRead::Linear
                    }
                }
                ModelTextureRole::BumpCompress
                | ModelTextureRole::BumpStretch
                | ModelTextureRole::PhongExponent
                | ModelTextureRole::LightWarp
                | ModelTextureRole::PhongWarp
                | ModelTextureRole::EyeCornea
                | ModelTextureRole::EyeGlint => TextureColorRead::Linear,
            };
            Ok(ModelTextureRequest {
                role,
                parameter: parameter.to_vec(),
                reference: reference.clone(),
                logical_path: logical_path(&normalized, ".vtf", parameter)?,
                color_read,
            })
        })
        .collect()
}

fn core_texture(textures: &[TextureRequest], role: TextureRole) -> Option<&TextureRequest> {
    textures.iter().find(|texture| texture.role == role)
}

fn model_texture(
    textures: &[ModelTextureRequest],
    role: ModelTextureRole,
) -> Option<&ModelTextureRequest> {
    textures.iter().find(|texture| texture.role == role)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_binding_requires_every_declared_subresource_and_plane() {
        let sampling = TextureSamplingState {
            wrap_s: TextureWrapMode::Repeat,
            wrap_t: TextureWrapMode::Repeat,
            wrap_u: TextureWrapMode::Repeat,
            min_filter: TextureMinFilter::LinearMipmapNearest,
            mag_filter: TextureMagFilter::Linear,
            anisotropy_level: 1,
            mipmapped: true,
            no_lod: false,
            all_mips: false,
        };
        let metadata = TextureMetadataManifest {
            width: 2,
            height: 2,
            depth: 1,
            mip_count: 2,
            frame_count: 1,
            faces: vec![TextureFace::Right],
            sampling,
            subresources: vec![
                TextureSubresourceIdentity {
                    mip: 1,
                    frame: 0,
                    face: TextureFace::Right,
                    slice: 0,
                },
                TextureSubresourceIdentity {
                    mip: 0,
                    frame: 0,
                    face: TextureFace::Right,
                    slice: 0,
                },
            ],
        };
        let request = TextureRequest {
            role: TextureRole::Base,
            parameter: b"$basetexture".to_vec(),
            reference: b"models/test".to_vec(),
            logical_path: Some("materials/models/test.vtf".to_owned()),
            disposition: TextureDisposition::Source,
            color_read: TextureColorRead::Srgb,
        };
        let binding = bind_authored_texture(&request, &metadata).unwrap();
        assert_eq!(binding.mip_count, 2);
        assert_eq!(binding.subresources.len(), 2);
        let planes = binding
            .subresources
            .iter()
            .map(|identity| AuthoredTexturePlane {
                identity: *identity,
                width: (2_u32 >> identity.mip).max(1),
                height: (2_u32 >> identity.mip).max(1),
                row_stride: 4,
                sample_bytes: 4,
            })
            .collect::<Vec<_>>();
        validate_authored_planes(&binding, &planes).unwrap();
        assert_eq!(
            validate_authored_planes(&binding, &planes[..1])
                .unwrap_err()
                .code,
            ErrorCode::InvalidTextureMetadata
        );
        let mut anisotropic = metadata;
        anisotropic.sampling.min_filter = TextureMinFilter::Anisotropic;
        assert_eq!(
            bind_authored_texture(&request, &anisotropic)
                .unwrap_err()
                .code,
            ErrorCode::InvalidTextureMetadata
        );
        anisotropic.sampling.anisotropy_level = 4;
        assert_eq!(
            bind_authored_texture(&request, &anisotropic)
                .unwrap()
                .sampling
                .anisotropy_level,
            4
        );
    }
}
