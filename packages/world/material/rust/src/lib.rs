use playsrc_vmt::{EffectiveDocument, EffectiveNode, EffectiveValue};
use std::{collections::BTreeMap, fmt};
mod alpha;
mod model;
mod proxy;
mod water;
pub use alpha::*;
pub use model::*;
pub use proxy::*;
pub use water::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shader {
    LightmappedGeneric,
    VertexLitGeneric,
    UnlitGeneric,
    WorldVertexTransition,
    Water,
    Refract,
    Sprite,
    SkyLdr,
    SkyHdr,
    Unsupported,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    Reflection,
    Refraction,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureDisposition {
    Source,
    BuiltInEnvironment,
    BuiltInRenderTarget,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureColorRead {
    Srgb,
    Linear,
    FormatDependent,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureRequest {
    pub role: TextureRole,
    pub parameter: Vec<u8>,
    pub reference: Vec<u8>,
    pub logical_path: Option<String>,
    pub disposition: TextureDisposition,
    pub color_read: TextureColorRead,
}
impl TextureRequest {
    pub fn is_defined(&self) -> bool {
        !self.reference.is_empty()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextureFrameSelection {
    Static {
        parameter: Vec<u8>,
        initial: i32,
        proxy_mutated: bool,
    },
    Unframed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterOrigin {
    Authored,
    ShaderInitializer,
    TypeInitializer,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveParameter<T> {
    pub value: T,
    pub origin: ParameterOrigin,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TextureTransformState {
    pub parameter: Vec<u8>,
    pub matrix: [f32; 16],
    pub origin: ParameterOrigin,
    pub proxy_mutated: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TextureUseState {
    pub role: TextureRole,
    pub frame: TextureFrameSelection,
    pub transform: Option<TextureTransformState>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proxy {
    pub name: Vec<u8>,
    pub scalar_arguments: Vec<(Vec<u8>, Vec<u8>)>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialRole {
    Bottom,
    UnderwaterOverlay,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialRequest {
    pub role: MaterialRole,
    pub parameter: Vec<u8>,
    pub reference: Vec<u8>,
    pub logical_path: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WaterFog {
    pub enabled: bool,
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WaterState {
    pub above_water: bool,
    pub normal_map: TextureRequest,
    pub environment_map: TextureRequest,
    pub reflection: TextureRequest,
    pub refraction: TextureRequest,
    pub bottom_material: Option<MaterialRequest>,
    pub underwater_overlay: Option<MaterialRequest>,
    pub reflect_amount: f32,
    pub refract_amount: f32,
    pub reflect_tint: [f32; 3],
    pub refract_tint: [f32; 3],
    pub fog: WaterFog,
    pub cheap_start: f32,
    pub cheap_end: f32,
    pub force_cheap: bool,
    pub force_expensive: bool,
    pub no_fresnel: bool,
    pub reflect_entities: bool,
    pub blur_refraction: bool,
    pub scroll: [[f32; 3]; 2],
    pub has_proxy_program: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalState {
    pub scale: f32,
    pub suppress_decals: bool,
    pub alpha_tested: bool,
    pub material: bool,
    pub translucent: bool,
    pub additive: bool,
    pub vertex_color: bool,
    pub vertex_alpha: bool,
    pub no_cull: bool,
    pub ignore_z: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Features {
    pub translucent: bool,
    pub additive: bool,
    pub alpha_test: bool,
    pub no_cull: bool,
    pub self_illum: bool,
    pub ss_bump: bool,
    pub vertex_color: bool,
    pub vertex_alpha: bool,
    pub decal: bool,
    pub ignore_z: bool,
    pub z_nearer: bool,
    pub no_fog: bool,
    pub wireframe: bool,
    pub no_draw: bool,
    pub opaque_texture: bool,
    pub base_alpha_environment_mask: bool,
    pub no_alpha_modulation: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DetailState {
    pub texture: TextureRequest,
    pub scale: [f32; 2],
    pub blend_mode: i32,
    pub blend_factor: f32,
    pub tint: [f32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct BumpState {
    pub primary: TextureRequest,
    pub self_shadowed: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentMapState {
    pub texture: TextureRequest,
    pub mask: Option<TextureRequest>,
    pub tint: [f32; 3],
    pub contrast: f32,
    pub saturation: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub shader_token: Vec<u8>,
    pub shader: Shader,
    pub parameters: Vec<(Vec<u8>, Vec<u8>)>,
    pub first_parameters: BTreeMap<Vec<u8>, Vec<u8>>,
    pub textures: Vec<TextureRequest>,
    pub texture_uses: Vec<TextureUseState>,
    pub selected_textures: Vec<TextureRole>,
    pub active_textures: Vec<TextureRole>,
    pub selection_trace: Vec<SelectionDecision>,
    pub proxies: Vec<Proxy>,
    pub surface_property: Option<Vec<u8>>,
    pub features: Features,
    pub material_requests: Vec<MaterialRequest>,
    pub water: Option<WaterState>,
    pub decal: DecalState,
    pub detail: Option<DetailState>,
    pub bump: Option<BumpState>,
    pub environment_map: Option<EnvironmentMapState>,
    pub model: Option<ModelMaterialState>,
    pub model_textures: Vec<ModelTextureRequest>,
    pub particle: Option<ParticleMaterialState>,
    pub proxy_program: ProxyProgram,
    pub selection_environment: SelectionEnvironment,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionDecision {
    pub source_key: Vec<u8>,
    pub effective_key: Option<Vec<u8>>,
    pub active: bool,
    pub conditional: bool,
    pub replaced_earlier_value: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HdrMode {
    None,
    Integer,
    Float,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionEnvironment {
    pub hdr_mode: HdrMode,
    pub shader_model: u16,
    pub pixel_shader_2_b: bool,
    pub srgb_correct_blending: bool,
    pub low_fill: bool,
    pub editor_materials: bool,
    pub model: bool,
    pub sprite_card_default_depth_blend: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendFactor {
    Zero,
    One,
    SourceAlpha,
    OneMinusSourceAlpha,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendEquation {
    Add,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlendState {
    pub enabled: bool,
    pub equation: BlendEquation,
    pub source: BlendFactor,
    pub destination: BlendFactor,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompareFunction {
    Greater,
    GreaterOrEqual,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DepthFunction {
    Nearer,
    NearerOrEqual,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CullState {
    Back,
    None,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolygonOffset {
    None,
    Decal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FogMode {
    Color,
    Black,
    Disabled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingModel {
    Unlit,
    VertexLit,
    Lightmapped,
    BumpedLightmapped,
    Water,
    Particle,
    Sky,
    Unsupported,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureAlphaFacts {
    pub base: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticState {
    pub lighting: LightingModel,
    pub blend: BlendState,
    pub alpha_modulation: f32,
    pub alpha_ownership: AlphaOwnership,
    pub fragment_discard: FragmentDiscardRequirement,
    pub alpha_test: bool,
    pub alpha_test_function: CompareFunction,
    pub alpha_test_reference: f32,
    pub cull: CullState,
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_function: DepthFunction,
    pub polygon_offset: PolygonOffset,
    pub fog: FogMode,
    pub wireframe: bool,
    pub no_draw: bool,
    pub vertex_color: bool,
    pub vertex_alpha: bool,
    pub translucent_queue: bool,
}
impl Default for SelectionEnvironment {
    fn default() -> Self {
        Self {
            hdr_mode: HdrMode::None,
            shader_model: 90,
            pixel_shader_2_b: true,
            srgb_correct_blending: true,
            low_fill: false,
            editor_materials: false,
            model: false,
            sprite_card_default_depth_blend: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    RootKind,
    InvalidReference,
    InvalidPath,
    UnsupportedCondition,
    MissingProfileTexture,
    InvalidParameter,
    InvalidTextureMetadata,
    MissingModelInput,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub parameter: Option<Vec<u8>>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for Error {}
pub fn resolve(document: &EffectiveDocument) -> Result<Material, Error> {
    resolve_for_environment(document, SelectionEnvironment::default())
}
pub fn resolve_for_environment(
    document: &EffectiveDocument,
    environment: SelectionEnvironment,
) -> Result<Material, Error> {
    let EffectiveValue::Object(children) = &document.root.value else {
        return Err(error(ErrorCode::RootKind, None));
    };
    let declared_shader = shader(&document.root.key.bytes);
    let shader = selected_shader(declared_shader, environment);
    let (children, selection_trace) =
        selected_children(children, &document.root.key.bytes, environment)?;
    let mut parameters = Vec::new();
    let mut first = BTreeMap::new();
    let mut proxies = Vec::new();
    for child in &children {
        if child.key.bytes.eq_ignore_ascii_case(b"Proxies") {
            if proxies.is_empty()
                && let EffectiveValue::Object(values) = &child.value
            {
                for proxy in values {
                    let args = match &proxy.value {
                        EffectiveValue::Object(args) => {
                            args.iter().filter_map(scalar_pair).collect()
                        }
                        EffectiveValue::Scalar(_) => Vec::new(),
                    };
                    proxies.push(Proxy {
                        name: proxy.key.bytes.clone(),
                        scalar_arguments: args,
                    });
                }
            }
            continue;
        }
        if let Some((key, value)) = scalar_pair(child) {
            parameters.push((key.clone(), value.clone()));
            first.entry(lower(&key)).or_insert(value);
        }
    }
    let specs = [
        (b"$basetexture".as_slice(), TextureRole::Base),
        (b"$hdrbasetexture", TextureRole::HdrBase),
        (b"$hdrcompressedtexture", TextureRole::HdrCompressed),
        (b"$hdrcompressedtexture0", TextureRole::HdrCompressed0),
        (b"$hdrcompressedtexture1", TextureRole::HdrCompressed1),
        (b"$hdrcompressedtexture2", TextureRole::HdrCompressed2),
        (b"$basetexture2", TextureRole::Base2),
        (b"$bumpmap", TextureRole::Bump),
        (b"$normalmap", TextureRole::Normal),
        (b"$bumpmap2", TextureRole::Bump2),
        (b"$detail", TextureRole::Detail),
        (b"$blendmodulatetexture", TextureRole::BlendModulate),
        (b"$envmap", TextureRole::Environment),
        (b"$envmapmask", TextureRole::EnvironmentMask),
        (b"$selfillummask", TextureRole::SelfIllumMask),
        (b"$flowmap", TextureRole::Flow),
        (b"$reflecttexture", TextureRole::Reflection),
        (b"$refracttexture", TextureRole::Refraction),
    ];
    let detail_blend_mode = integer_or(&first, b"$detailblendmode", 0)?;
    if !(0..=11).contains(&detail_blend_mode) {
        return Err(error(
            ErrorCode::InvalidParameter,
            Some(b"$detailblendmode".to_vec()),
        ));
    }
    let sprite_extract_green_alpha = integer_or(&first, b"$extractgreenalpha", 0)? != 0;
    let gamma_color_read = integer_or(&first, b"$gammacolorread", 0)? != 0;
    let sprite_card = document.root.key.bytes.eq_ignore_ascii_case(b"SpriteCard");
    let mut textures = Vec::new();
    for (name, role) in specs {
        if let Some(reference) = get(&first, name) {
            let mut request = texture(name, role, reference)?;
            request.color_read = texture_color_read(
                shader,
                role,
                environment,
                detail_blend_mode,
                sprite_extract_green_alpha,
                gamma_color_read,
                sprite_card,
            );
            textures.push(request);
        }
    }
    let selected_textures = selected_textures(shader, environment, &textures)?;
    let active_textures = textures.iter().map(|texture| texture.role).collect();
    let material_specs = [
        (b"$bottommaterial".as_slice(), MaterialRole::Bottom),
        (
            b"$underwateroverlay".as_slice(),
            MaterialRole::UnderwaterOverlay,
        ),
    ];
    let material_requests = material_specs
        .into_iter()
        .filter_map(|(name, role)| get(&first, name).map(|reference| (name, role, reference)))
        .map(|(name, role, reference)| material_request(name, role, reference))
        .collect::<Result<Vec<_>, _>>()?;
    let water = (shader == Shader::Water)
        .then(|| water_state(&first, &material_requests, &proxies))
        .transpose()?;
    let detail = textures
        .iter()
        .find(|texture| texture.role == TextureRole::Detail)
        .cloned()
        .map(|texture| {
            Ok(DetailState {
                texture,
                scale: scalar_or_vector2(&first, b"$detailscale", [4.0; 2])?,
                blend_mode: detail_blend_mode,
                blend_factor: float_or(&first, b"$detailblendfactor", 1.0)?,
                tint: color_or(&first, b"$detailtint", [1.0; 3])?,
            })
        })
        .transpose()?;
    let bump = textures
        .iter()
        .find(|texture| texture.role == TextureRole::Bump)
        .cloned()
        .map(|primary| BumpState {
            primary,
            self_shadowed: boolean(&first, b"$ssbump"),
        });
    let environment_map = textures
        .iter()
        .find(|texture| texture.role == TextureRole::Environment)
        .cloned()
        .map(|texture| {
            Ok(EnvironmentMapState {
                texture,
                mask: textures
                    .iter()
                    .find(|texture| texture.role == TextureRole::EnvironmentMask)
                    .cloned(),
                tint: color_or(&first, b"$envmaptint", [1.0; 3])?,
                contrast: float_or(&first, b"$envmapcontrast", 0.0)?,
                saturation: float_or(&first, b"$envmapsaturation", 1.0)?,
            })
        })
        .transpose()?;
    let decal_scale = float_or(&first, b"$decalscale", 1.0)?;
    if decal_scale <= 0.0 {
        return Err(error(
            ErrorCode::InvalidParameter,
            Some(b"$decalscale".to_vec()),
        ));
    }
    let features = Features {
        translucent: boolean(&first, b"$translucent"),
        additive: boolean(&first, b"$additive"),
        alpha_test: boolean(&first, b"$alphatest"),
        no_cull: boolean(&first, b"$nocull"),
        self_illum: boolean(&first, b"$selfillum"),
        ss_bump: boolean(&first, b"$ssbump"),
        vertex_color: boolean(&first, b"$vertexcolor"),
        vertex_alpha: boolean(&first, b"$vertexalpha"),
        decal: boolean(&first, b"$decal"),
        ignore_z: boolean(&first, b"$ignorez") || matches!(shader, Shader::SkyLdr | Shader::SkyHdr),
        z_nearer: boolean(&first, b"$znearer"),
        no_fog: boolean(&first, b"$nofog") || matches!(shader, Shader::SkyLdr | Shader::SkyHdr),
        wireframe: boolean(&first, b"$wireframe"),
        no_draw: boolean(&first, b"$no_draw"),
        opaque_texture: boolean(&first, b"$opaquetexture"),
        base_alpha_environment_mask: boolean(&first, b"$basealphaenvmapmask"),
        no_alpha_modulation: boolean(&first, b"$noalphamod"),
    };
    let proxy_program = proxy::compile_proxy_program(&proxies);
    let texture_uses = texture_use_states(&textures, &first, &proxy_program)?;
    let (model, model_textures) = model::resolve_model_state(
        &document.root.key.bytes,
        &first,
        &textures,
        &proxy_program,
        environment,
    )?;
    let particle =
        alpha::resolve_particle_state(&document.root.key.bytes, &first, &textures, environment)?;
    let effective_alpha_test =
        features.alpha_test && !features.self_illum && !features.base_alpha_environment_mask;
    let decal = DecalState {
        scale: decal_scale,
        suppress_decals: boolean(&first, b"$nodecal"),
        alpha_tested: effective_alpha_test,
        material: boolean(&first, b"$decal"),
        translucent: features.translucent,
        additive: features.additive,
        vertex_color: features.vertex_color,
        vertex_alpha: features.vertex_alpha,
        no_cull: features.no_cull,
        ignore_z: features.ignore_z,
    };
    Ok(Material {
        shader_token: document.root.key.bytes.clone(),
        shader,
        parameters,
        first_parameters: first.clone(),
        textures,
        texture_uses,
        selected_textures,
        active_textures,
        selection_trace,
        proxies,
        surface_property: get(&first, b"$surfaceprop").cloned(),
        features,
        material_requests,
        water,
        decal,
        detail,
        bump,
        environment_map,
        model,
        model_textures,
        particle,
        proxy_program,
        selection_environment: environment,
    })
}

pub fn static_state(
    material: &Material,
    texture_alpha: TextureAlphaFacts,
) -> Result<StaticState, Error> {
    let alpha = float_or(&material.first_parameters, b"$alpha", 1.0)?;
    static_state_with_alpha(material, texture_alpha, alpha)
}

fn static_state_with_alpha(
    material: &Material,
    texture_alpha: TextureAlphaFacts,
    current_alpha: f32,
) -> Result<StaticState, Error> {
    if !current_alpha.is_finite() {
        return Err(error(ErrorCode::InvalidParameter, Some(b"$alpha".to_vec())));
    }
    let features = &material.features;
    let self_illum = effective_self_illumination(material, texture_alpha);
    let base_alpha_environment_mask = features.base_alpha_environment_mask && texture_alpha.base;
    let alpha_test = features.alpha_test && !self_illum && !base_alpha_environment_mask;
    let alpha = if features.no_alpha_modulation {
        1.0
    } else {
        current_alpha.clamp(0.0, 1.0)
    };
    let base_texture_alpha = texture_alpha.base
        && !features.opaque_texture
        && !self_illum
        && !base_alpha_environment_mask
        && (features.translucent || features.alpha_test);
    let alpha_blend = alpha < 1.0 || features.vertex_alpha || (base_texture_alpha && !alpha_test);
    let blend = if material.particle.is_some() {
        alpha::sprite_card_blend(material)?
    } else if features.additive {
        BlendState {
            enabled: true,
            equation: BlendEquation::Add,
            source: if alpha_blend {
                BlendFactor::SourceAlpha
            } else {
                BlendFactor::One
            },
            destination: BlendFactor::One,
        }
    } else if alpha_blend {
        BlendState {
            enabled: true,
            equation: BlendEquation::Add,
            source: BlendFactor::SourceAlpha,
            destination: BlendFactor::OneMinusSourceAlpha,
        }
    } else {
        BlendState {
            enabled: false,
            equation: BlendEquation::Add,
            source: BlendFactor::One,
            destination: BlendFactor::Zero,
        }
    };
    let sprite_card_alpha_test = material.particle.is_some()
        && material
            .particle
            .as_ref()
            .is_some_and(|state| state.alpha_test);
    let alpha_test = if material.particle.is_some() {
        sprite_card_alpha_test
    } else {
        alpha_test
    };
    let (alpha_test_function, alpha_test_reference) = if material.particle.is_some() {
        (CompareFunction::Greater, 0.01)
    } else {
        let declared_reference = float_or(&material.first_parameters, b"$alphatestreference", 0.0)?;
        (
            CompareFunction::GreaterOrEqual,
            if declared_reference > 0.0 {
                declared_reference
            } else {
                0.7
            },
        )
    };
    let lighting = match material.shader {
        Shader::LightmappedGeneric | Shader::WorldVertexTransition => {
            if material.bump.is_some() {
                LightingModel::BumpedLightmapped
            } else {
                LightingModel::Lightmapped
            }
        }
        Shader::VertexLitGeneric => LightingModel::VertexLit,
        Shader::Sprite if material.particle.is_some() => LightingModel::Particle,
        Shader::UnlitGeneric | Shader::Sprite | Shader::Refract => LightingModel::Unlit,
        Shader::Water => LightingModel::Water,
        Shader::SkyLdr | Shader::SkyHdr => LightingModel::Sky,
        Shader::Unsupported if material.model.is_some() => LightingModel::VertexLit,
        Shader::Unsupported => LightingModel::Unsupported,
    };
    let depth_test = !features.ignore_z;
    let alpha_ownership = alpha::alpha_ownership(material, texture_alpha, alpha_test, alpha);
    let fragment_discard = if alpha_test {
        FragmentDiscardRequirement::Alpha {
            source: if material.particle.is_some() {
                FragmentAlphaSource::ShaderOutput
            } else {
                FragmentAlphaSource::BaseTextureOrOne
            },
            pass: alpha_test_function,
            reference: alpha_test_reference,
        }
    } else {
        FragmentDiscardRequirement::None
    };
    Ok(StaticState {
        lighting,
        blend,
        alpha_modulation: alpha,
        alpha_ownership,
        fragment_discard,
        alpha_test,
        alpha_test_function,
        alpha_test_reference,
        cull: if features.no_cull || material.particle.is_some() {
            CullState::None
        } else {
            CullState::Back
        },
        depth_test,
        depth_write: depth_test && !features.decal && !blend.enabled,
        depth_function: if features.z_nearer {
            DepthFunction::Nearer
        } else {
            DepthFunction::NearerOrEqual
        },
        polygon_offset: if features.decal {
            PolygonOffset::Decal
        } else {
            PolygonOffset::None
        },
        fog: if material.particle.is_some() || features.no_fog {
            FogMode::Disabled
        } else if features.additive {
            FogMode::Black
        } else {
            FogMode::Color
        },
        wireframe: features.wireframe,
        no_draw: features.no_draw,
        vertex_color: features.vertex_color || material.particle.is_some(),
        vertex_alpha: features.vertex_alpha || material.particle.is_some(),
        translucent_queue: features.translucent || blend.enabled,
    })
}

fn effective_self_illumination(material: &Material, texture_alpha: TextureAlphaFacts) -> bool {
    if !material.features.self_illum {
        return false;
    }
    if texture_alpha.base
        || material
            .textures
            .iter()
            .any(|texture| texture.role == TextureRole::SelfIllumMask)
    {
        return true;
    }
    material.model.as_ref().is_some_and(|model| {
        let ModelShaderState::VertexLitGeneric(state) = &model.state else {
            return false;
        };
        state
            .self_illumination
            .as_ref()
            .is_some_and(|state| state.source == SelfIllumMaskSource::Fresnel)
    })
}

fn water_state(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    material_requests: &[MaterialRequest],
    proxies: &[Proxy],
) -> Result<WaterState, Error> {
    let request = |parameter: &'static [u8], role| {
        let Some(reference) = get(parameters, parameter) else {
            return Ok(TextureRequest {
                role,
                parameter: parameter.to_vec(),
                reference: Vec::new(),
                logical_path: None,
                disposition: TextureDisposition::Source,
                color_read: if role == TextureRole::Normal {
                    TextureColorRead::Linear
                } else {
                    TextureColorRead::Srgb
                },
            });
        };
        let mut request = texture(parameter, role, reference)?;
        request.color_read = if role == TextureRole::Normal {
            TextureColorRead::Linear
        } else {
            TextureColorRead::Srgb
        };
        Ok(request)
    };
    let force_cheap = boolean_or(parameters, b"$forcecheap", false)?;
    let mut force_expensive = boolean_or(parameters, b"$forceexpensive", true)?;
    if force_cheap && force_expensive {
        force_expensive = false;
    }
    Ok(WaterState {
        above_water: boolean_or(parameters, b"$abovewater", true)?,
        normal_map: request(b"$normalmap", TextureRole::Normal)?,
        environment_map: request(b"$envmap", TextureRole::Environment)?,
        reflection: request(b"$reflecttexture", TextureRole::Reflection)?,
        refraction: request(b"$refracttexture", TextureRole::Refraction)?,
        bottom_material: material_requests
            .iter()
            .find(|request| request.role == MaterialRole::Bottom)
            .cloned(),
        underwater_overlay: material_requests
            .iter()
            .find(|request| request.role == MaterialRole::UnderwaterOverlay)
            .cloned(),
        reflect_amount: float_or(parameters, b"$reflectamount", 0.0)?,
        refract_amount: float_or(parameters, b"$refractamount", 0.0)?,
        reflect_tint: color_or(parameters, b"$reflecttint", [1.0; 3])?,
        refract_tint: color_or(parameters, b"$refracttint", [1.0; 3])?,
        fog: WaterFog {
            enabled: boolean_or(parameters, b"$fogenable", false)?,
            color: color_or(parameters, b"$fogcolor", [1.0, 0.0, 0.0])?,
            start: float_or(parameters, b"$fogstart", 0.0)?,
            end: float_or(parameters, b"$fogend", 0.0)?,
        },
        cheap_start: float_or(parameters, b"$cheapwaterstartdistance", 500.0)?,
        cheap_end: float_or(parameters, b"$cheapwaterenddistance", 1000.0)?,
        force_cheap,
        force_expensive,
        no_fresnel: boolean_or(parameters, b"$nofresnel", false)?,
        reflect_entities: boolean_or(parameters, b"$reflectentities", false)?,
        blur_refraction: boolean_or(parameters, b"$blurrefract", false)?,
        scroll: [
            color_or(parameters, b"$scroll1", [0.0; 3])?,
            color_or(parameters, b"$scroll2", [0.0; 3])?,
        ],
        has_proxy_program: !proxies.is_empty(),
    })
}
fn selected_children(
    children: &[EffectiveNode],
    shader: &[u8],
    environment: SelectionEnvironment,
) -> Result<(Vec<EffectiveNode>, Vec<SelectionDecision>), Error> {
    let mut selected = Vec::with_capacity(children.len());
    let mut first = BTreeMap::<Vec<u8>, usize>::new();
    let mut trace = Vec::with_capacity(children.len());
    for child in children {
        let Some((key, conditional)) = selected_key(&child.key.bytes, environment)? else {
            trace.push(SelectionDecision {
                source_key: child.key.bytes.clone(),
                effective_key: None,
                active: false,
                conditional: true,
                replaced_earlier_value: false,
            });
            continue;
        };
        let mut node = child.clone();
        node.key.bytes = key.clone();
        let lookup = lower(&key);
        let replaced_earlier_value = conditional && first.contains_key(&lookup);
        trace.push(SelectionDecision {
            source_key: child.key.bytes.clone(),
            effective_key: Some(key.clone()),
            active: true,
            conditional,
            replaced_earlier_value,
        });
        if conditional && let Some(index) = first.get(&lookup).copied() {
            selected[index] = node;
        } else {
            first.entry(lookup).or_insert(selected.len());
            selected.push(node);
        }
    }
    let names = override_names(shader, environment);
    let Some(override_children) = names.iter().find_map(|name| {
        selected.iter().find_map(|node| {
            if node.key.bytes.eq_ignore_ascii_case(name)
                && let EffectiveValue::Object(values) = &node.value
            {
                Some(values.clone())
            } else {
                None
            }
        })
    }) else {
        return Ok((selected, trace));
    };
    for node in override_children {
        let Some((key, conditional)) = selected_key(&node.key.bytes, environment)? else {
            trace.push(SelectionDecision {
                source_key: node.key.bytes.clone(),
                effective_key: None,
                active: false,
                conditional: true,
                replaced_earlier_value: false,
            });
            continue;
        };
        let source_key = node.key.bytes.clone();
        let mut node = node;
        node.key.bytes = key.clone();
        let lookup = lower(&key);
        let replaced_earlier_value = first.contains_key(&lookup);
        trace.push(SelectionDecision {
            source_key,
            effective_key: Some(key.clone()),
            active: true,
            conditional,
            replaced_earlier_value,
        });
        if let Some(index) = first.get(&lookup).copied() {
            selected[index] = node;
        } else {
            first.insert(lookup, selected.len());
            selected.push(node);
        }
    }
    Ok((selected, trace))
}
fn selected_key(
    key: &[u8],
    environment: SelectionEnvironment,
) -> Result<Option<(Vec<u8>, bool)>, Error> {
    let Some(separator) = key.iter().position(|byte| *byte == b'?') else {
        return Ok(Some((key.to_vec(), false)));
    };
    if separator == 0 || separator + 1 == key.len() {
        return Err(error(ErrorCode::UnsupportedCondition, Some(key.to_vec())));
    }
    let (condition, key) = (&key[..separator], &key[separator + 1..]);
    let (negated, condition) = condition
        .strip_prefix(b"!")
        .map_or((false, condition), |value| (true, value));
    let active = condition_value(condition, environment)
        .ok_or_else(|| error(ErrorCode::UnsupportedCondition, Some(condition.to_vec())))?;
    Ok((active ^ negated).then(|| (key.to_vec(), true)))
}
fn condition_value(condition: &[u8], environment: SelectionEnvironment) -> Option<bool> {
    if condition.eq_ignore_ascii_case(b"hdr") {
        Some(environment.hdr_mode != HdrMode::None)
    } else if condition.eq_ignore_ascii_case(b"ldr") {
        Some(environment.hdr_mode == HdrMode::None)
    } else if condition.eq_ignore_ascii_case(b"srgb") {
        Some(environment.srgb_correct_blending)
    } else if condition.eq_ignore_ascii_case(b"lowfill") {
        Some(environment.low_fill)
    } else if condition.eq_ignore_ascii_case(b"editor") {
        Some(environment.editor_materials)
    } else if condition.eq_ignore_ascii_case(b"model") {
        Some(environment.model)
    } else if condition.eq_ignore_ascii_case(b"360") {
        Some(false)
    } else if condition.eq_ignore_ascii_case(b"dx90") {
        Some(environment.shader_model >= 90)
    } else if condition.eq_ignore_ascii_case(b"dx90_20b") {
        Some(environment.shader_model >= 90 && environment.pixel_shader_2_b)
    } else {
        None
    }
}
fn override_names(shader: &[u8], environment: SelectionEnvironment) -> Vec<Vec<u8>> {
    let mut output = Vec::new();
    let mut push = |suffix: &[u8]| {
        output.push(suffix.to_vec());
        let mut qualified = shader.to_vec();
        qualified.push(b'_');
        qualified.extend_from_slice(suffix);
        output.push(qualified);
    };
    if environment.hdr_mode != HdrMode::None {
        push(b"hdr_dx9");
        push(b"hdr");
    } else {
        push(b"ldr");
    }
    if environment.srgb_correct_blending {
        push(b"srgb");
    }
    if environment.shader_model >= 90 && environment.pixel_shader_2_b {
        push(b">=dx90_20b");
    }
    if environment.shader_model >= 90 {
        push(b">=dx90");
        push(b"dx9");
    } else {
        push(b"<dx90");
    }
    output
}
fn selected_shader(shader: Shader, environment: SelectionEnvironment) -> Shader {
    if shader == Shader::SkyLdr
        && environment.hdr_mode != HdrMode::None
        && environment.shader_model >= 90
    {
        Shader::SkyHdr
    } else {
        shader
    }
}
fn selected_textures(
    shader: Shader,
    environment: SelectionEnvironment,
    textures: &[TextureRequest],
) -> Result<Vec<TextureRole>, Error> {
    let has = |role| textures.iter().any(|texture| texture.role == role);
    if shader == Shader::SkyHdr {
        if has(TextureRole::HdrCompressed) {
            return Ok(vec![TextureRole::HdrCompressed]);
        }
        if has(TextureRole::HdrCompressed0) {
            if has(TextureRole::HdrCompressed1) && has(TextureRole::HdrCompressed2) {
                return Ok(vec![
                    TextureRole::HdrCompressed0,
                    TextureRole::HdrCompressed1,
                    TextureRole::HdrCompressed2,
                ]);
            }
            return Err(error(ErrorCode::MissingProfileTexture, None));
        }
        if has(TextureRole::HdrBase) {
            return Ok(vec![TextureRole::HdrBase]);
        }
        return Err(error(ErrorCode::MissingProfileTexture, None));
    }
    if matches!(shader, Shader::SkyLdr) && !has(TextureRole::Base) {
        return Err(error(ErrorCode::MissingProfileTexture, None));
    }
    let _ = environment;
    Ok(has(TextureRole::Base)
        .then_some(TextureRole::Base)
        .into_iter()
        .collect())
}

fn texture_use_states(
    textures: &[TextureRequest],
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    proxy_program: &ProxyProgram,
) -> Result<Vec<TextureUseState>, Error> {
    textures
        .iter()
        .map(|texture| {
            let frame_parameter = match texture.role {
                TextureRole::Base
                | TextureRole::HdrBase
                | TextureRole::HdrCompressed
                | TextureRole::HdrCompressed0
                | TextureRole::HdrCompressed1
                | TextureRole::HdrCompressed2 => Some(b"$frame".as_slice()),
                TextureRole::Base2 => Some(b"$frame2".as_slice()),
                TextureRole::Bump | TextureRole::Normal => Some(b"$bumpframe".as_slice()),
                TextureRole::Bump2 => Some(b"$bumpframe2".as_slice()),
                TextureRole::Detail => Some(b"$detailframe".as_slice()),
                TextureRole::Environment => Some(b"$envmapframe".as_slice()),
                TextureRole::EnvironmentMask => Some(b"$envmapmaskframe".as_slice()),
                TextureRole::SelfIllumMask => Some(b"$selfillumtextureframe".as_slice()),
                TextureRole::Flow => Some(b"$flowmapframe".as_slice()),
                TextureRole::BlendModulate | TextureRole::Reflection | TextureRole::Refraction => {
                    None
                }
            };
            let frame =
                frame_parameter.map_or(Ok(TextureFrameSelection::Unframed), |parameter| {
                    Ok(TextureFrameSelection::Static {
                        parameter: parameter.to_vec(),
                        initial: integer_or(parameters, parameter, 0)?,
                        proxy_mutated: proxy_writes(proxy_program, parameter),
                    })
                })?;
            let transform_parameter = match texture.role {
                TextureRole::Base
                | TextureRole::HdrBase
                | TextureRole::HdrCompressed
                | TextureRole::HdrCompressed0
                | TextureRole::HdrCompressed1
                | TextureRole::HdrCompressed2
                | TextureRole::Base2 => Some(b"$basetexturetransform".as_slice()),
                TextureRole::Bump | TextureRole::Normal => Some(b"$bumptransform".as_slice()),
                TextureRole::Bump2 => Some(b"$bumptransform2".as_slice()),
                TextureRole::EnvironmentMask => Some(b"$envmapmasktransform".as_slice()),
                TextureRole::BlendModulate => Some(b"$blendmasktransform".as_slice()),
                TextureRole::Detail
                | TextureRole::Environment
                | TextureRole::SelfIllumMask
                | TextureRole::Flow
                | TextureRole::Reflection
                | TextureRole::Refraction => None,
            };
            let transform = transform_parameter
                .map(|parameter| {
                    let value = get(parameters, parameter);
                    Ok(TextureTransformState {
                        parameter: parameter.to_vec(),
                        matrix: value.map_or(Ok(identity_matrix()), |value| {
                            parse_texture_transform(value, parameter)
                        })?,
                        origin: if value.is_some() {
                            ParameterOrigin::Authored
                        } else {
                            ParameterOrigin::TypeInitializer
                        },
                        proxy_mutated: proxy_writes(proxy_program, parameter),
                    })
                })
                .transpose()?;
            Ok(TextureUseState {
                role: texture.role,
                frame,
                transform,
            })
        })
        .collect()
}

fn proxy_writes(program: &ProxyProgram, variable: &[u8]) -> bool {
    let variable = lower(variable);
    program.entries.iter().any(|entry| {
        let Some(operation) = &entry.operation else {
            return false;
        };
        match operation {
            ProxyOperation::AnimatedTexture { frame, .. } => frame.name == variable,
            ProxyOperation::Sine { result, .. }
            | ProxyOperation::Equals { result, .. }
            | ProxyOperation::Multiply { result, .. }
            | ProxyOperation::LessOrEqual { result, .. }
            | ProxyOperation::SelectFirstIfNonZero { result, .. }
            | ProxyOperation::TextureTransform { result, .. }
            | ProxyOperation::TextureScroll { result, .. }
            | ProxyOperation::ModelGlowColor { result }
            | ProxyOperation::YellowLevel { result }
            | ProxyOperation::ScalarModelInput { result, .. }
            | ProxyOperation::VectorModelInput { result, .. } => result.name == variable,
            ProxyOperation::WaterLod => {
                variable == b"$cheapwaterstartdistance" || variable == b"$cheapwaterenddistance"
            }
            ProxyOperation::AnimatedWeaponSheen => variable == b"$sheenmapmaskframe",
            ProxyOperation::Invisibility { .. } | ProxyOperation::WeaponSkin => false,
        }
    })
}

fn parse_texture_transform(value: &[u8], parameter: &[u8]) -> Result<[f32; 16], Error> {
    let text = std::str::from_utf8(value)
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?
        .trim();
    if let Some(content) = text
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        let values = finite_values(content, parameter)?;
        if values.len() == 16 {
            return values
                .try_into()
                .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
        }
        return Err(error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
    }
    let tokens = text.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.len() != 11
        || !tokens[0].eq_ignore_ascii_case("center")
        || !tokens[3].eq_ignore_ascii_case("scale")
        || !tokens[6].eq_ignore_ascii_case("rotate")
        || !tokens[8].eq_ignore_ascii_case("translate")
    {
        return Err(error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
    }
    let values = [
        tokens[1], tokens[2], tokens[4], tokens[5], tokens[7], tokens[9], tokens[10],
    ]
    .map(|value| value.parse::<f32>().ok().filter(|value| value.is_finite()))
    .into_iter()
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?;
    let center = [values[0], values[1]];
    let scale = [values[2], values[3]];
    let angle = values[4].to_radians();
    let translation = [values[5], values[6]];
    Ok(matrix_multiply(
        translation_matrix(center[0] + translation[0], center[1] + translation[1]),
        matrix_multiply(
            rotation_matrix(angle),
            matrix_multiply(
                scale_matrix(scale[0], scale[1]),
                translation_matrix(-center[0], -center[1]),
            ),
        ),
    ))
}

fn finite_values(text: &str, parameter: &[u8]) -> Result<Vec<f32>, Error> {
    text.split_ascii_whitespace()
        .map(|value| {
            value
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))
        })
        .collect()
}

pub(crate) fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn translation_matrix(x: f32, y: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn scale_matrix(x: f32, y: f32) -> [f32; 16] {
    [
        x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn rotation_matrix(angle: f32) -> [f32; 16] {
    let (sine, cosine) = angle.sin_cos();
    [
        cosine, -sine, 0.0, 0.0, sine, cosine, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn matrix_multiply(left: [f32; 16], right: [f32; 16]) -> [f32; 16] {
    std::array::from_fn(|index| {
        let row = index / 4;
        let column = index % 4;
        (0..4)
            .map(|inner| left[row * 4 + inner] * right[inner * 4 + column])
            .sum()
    })
}

fn texture_color_read(
    shader: Shader,
    role: TextureRole,
    environment: SelectionEnvironment,
    detail_blend_mode: i32,
    sprite_extract_green_alpha: bool,
    gamma_color_read: bool,
    sprite_card: bool,
) -> TextureColorRead {
    match role {
        TextureRole::Bump
        | TextureRole::Normal
        | TextureRole::Bump2
        | TextureRole::BlendModulate
        | TextureRole::EnvironmentMask
        | TextureRole::SelfIllumMask
        | TextureRole::Flow => TextureColorRead::Linear,
        TextureRole::Detail => {
            if detail_blend_mode == 0 {
                TextureColorRead::Linear
            } else {
                TextureColorRead::Srgb
            }
        }
        TextureRole::Environment => {
            if shader == Shader::Water || environment.hdr_mode == HdrMode::None {
                TextureColorRead::Srgb
            } else {
                TextureColorRead::Linear
            }
        }
        TextureRole::HdrCompressed
        | TextureRole::HdrCompressed0
        | TextureRole::HdrCompressed1
        | TextureRole::HdrCompressed2 => TextureColorRead::Linear,
        TextureRole::HdrBase => TextureColorRead::FormatDependent,
        TextureRole::Base | TextureRole::Base2 => {
            if matches!(shader, Shader::SkyLdr | Shader::SkyHdr) {
                TextureColorRead::FormatDependent
            } else if (sprite_card && sprite_extract_green_alpha) || gamma_color_read {
                TextureColorRead::Linear
            } else {
                TextureColorRead::Srgb
            }
        }
        TextureRole::Reflection | TextureRole::Refraction => TextureColorRead::Srgb,
    }
}
fn scalar_pair(node: &EffectiveNode) -> Option<(Vec<u8>, Vec<u8>)> {
    let EffectiveValue::Scalar(v) = &node.value else {
        return None;
    };
    Some((node.key.bytes.clone(), v.token.bytes.clone()))
}
fn lower(v: &[u8]) -> Vec<u8> {
    v.iter().map(u8::to_ascii_lowercase).collect()
}
pub(crate) fn get<'a>(m: &'a BTreeMap<Vec<u8>, Vec<u8>>, k: &[u8]) -> Option<&'a Vec<u8>> {
    m.get(&lower(k))
}
pub(crate) fn boolean(m: &BTreeMap<Vec<u8>, Vec<u8>>, k: &[u8]) -> bool {
    let Some(v) = get(m, k) else { return false };
    source_integer(v) != 0
}
fn boolean_or(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: bool,
) -> Result<bool, Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(default);
    };
    let _ = parameter;
    Ok(source_integer(value) != 0)
}
pub(crate) fn integer_or(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: i32,
) -> Result<i32, Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(default);
    };
    std::str::from_utf8(value)
        .map(source_integer_text)
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))
}
pub(crate) fn source_integer(value: &[u8]) -> i32 {
    std::str::from_utf8(value).map_or(0, source_integer_text)
}
fn source_integer_text(value: &str) -> i32 {
    let value = value.trim_start();
    let (negative, digits) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else if let Some(value) = value.strip_prefix('+') {
        (false, value)
    } else {
        (false, value)
    };
    let mut parsed = 0_i32;
    let mut found = false;
    for byte in digits.bytes().take_while(u8::is_ascii_digit) {
        found = true;
        parsed = parsed
            .saturating_mul(10)
            .saturating_add(i32::from(byte - b'0'));
    }
    if !found {
        0
    } else if negative {
        parsed.saturating_neg()
    } else {
        parsed
    }
}
pub(crate) fn float_or(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: f32,
) -> Result<f32, Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(default);
    };
    std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .ok_or_else(|| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))
}
pub(crate) fn color_or(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: [f32; 3],
) -> Result<[f32; 3], Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(default);
    };
    let text = std::str::from_utf8(value)
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?
        .trim();
    let byte_color = text.starts_with('{') && text.ends_with('}');
    let content = if (text.starts_with('[') && text.ends_with(']'))
        || (text.starts_with('{') && text.ends_with('}'))
    {
        &text[1..text.len() - 1]
    } else {
        text
    };
    let values = content
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?;
    if !(3..=4).contains(&values.len())
        || values.iter().any(|value| !value.is_finite())
        || (byte_color && values.iter().any(|value| !(0.0..=255.0).contains(value)))
    {
        return Err(error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
    }
    let scale = if byte_color { 1.0 / 255.0 } else { 1.0 };
    Ok([values[0] * scale, values[1] * scale, values[2] * scale])
}

fn scalar_or_vector2(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: [f32; 2],
) -> Result<[f32; 2], Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(default);
    };
    let text = std::str::from_utf8(value)
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?
        .trim();
    let vector = (text.starts_with('[') && text.ends_with(']'))
        || (text.starts_with('{') && text.ends_with('}'));
    let content = if vector {
        &text[1..text.len() - 1]
    } else {
        text
    };
    let values = content
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?;
    if values.iter().any(|value| !value.is_finite())
        || (vector && values.len() != 2)
        || (!vector && values.len() != 1)
    {
        return Err(error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
    }
    Ok(if vector {
        [values[0], values[1]]
    } else {
        [values[0]; 2]
    })
}
fn shader(v: &[u8]) -> Shader {
    if v.eq_ignore_ascii_case(b"LightmappedGeneric") {
        Shader::LightmappedGeneric
    } else if v.eq_ignore_ascii_case(b"VertexLitGeneric") {
        Shader::VertexLitGeneric
    } else if v.eq_ignore_ascii_case(b"UnlitGeneric") {
        Shader::UnlitGeneric
    } else if v.eq_ignore_ascii_case(b"WorldVertexTransition") {
        Shader::WorldVertexTransition
    } else if v.eq_ignore_ascii_case(b"Water") {
        Shader::Water
    } else if v.eq_ignore_ascii_case(b"Refract") {
        Shader::Refract
    } else if v.eq_ignore_ascii_case(b"Sprite") || v.eq_ignore_ascii_case(b"SpriteCard") {
        Shader::Sprite
    } else if v.eq_ignore_ascii_case(b"Sky") {
        Shader::SkyLdr
    } else {
        Shader::Unsupported
    }
}
fn texture(parameter: &[u8], role: TextureRole, reference: &[u8]) -> Result<TextureRequest, Error> {
    if reference.is_empty() {
        return Err(error(ErrorCode::InvalidReference, Some(parameter.to_vec())));
    }
    let normalized = String::from_utf8(reference.to_vec())
        .map_err(|_| error(ErrorCode::InvalidPath, Some(parameter.to_vec())))?
        .replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    let disposition = if role == TextureRole::Environment && lower == "env_cubemap" {
        TextureDisposition::BuiltInEnvironment
    } else if lower.starts_with("_rt_") {
        TextureDisposition::BuiltInRenderTarget
    } else {
        TextureDisposition::Source
    };
    let logical_path = if disposition == TextureDisposition::Source {
        Some(logical_path(&normalized, ".vtf", parameter)?)
    } else {
        None
    };
    Ok(TextureRequest {
        role,
        parameter: parameter.to_vec(),
        reference: reference.to_vec(),
        logical_path,
        disposition,
        color_read: TextureColorRead::Linear,
    })
}
fn material_request(
    parameter: &[u8],
    role: MaterialRole,
    reference: &[u8],
) -> Result<MaterialRequest, Error> {
    if reference.is_empty() {
        return Err(error(ErrorCode::InvalidReference, Some(parameter.to_vec())));
    }
    let normalized = String::from_utf8(reference.to_vec())
        .map_err(|_| error(ErrorCode::InvalidPath, Some(parameter.to_vec())))?
        .replace('\\', "/");
    Ok(MaterialRequest {
        role,
        parameter: parameter.to_vec(),
        reference: reference.to_vec(),
        logical_path: logical_path(&normalized, ".vmt", parameter)?,
    })
}
pub(crate) fn logical_path(
    normalized: &str,
    extension: &str,
    parameter: &[u8],
) -> Result<String, Error> {
    let lower = normalized.to_ascii_lowercase();
    let prefix = if lower.starts_with("materials/") {
        ""
    } else {
        "materials/"
    };
    let suffix = if lower.ends_with(extension) {
        ""
    } else {
        extension
    };
    let path = format!("{prefix}{normalized}{suffix}");
    if path
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(error(ErrorCode::InvalidPath, Some(parameter.to_vec())));
    }
    Ok(path)
}
pub(crate) fn error(code: ErrorCode, parameter: Option<Vec<u8>>) -> Error {
    Error { code, parameter }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_keyvalues::ConditionEnvironment;
    use playsrc_vmt::{Composition, Limits, compose};
    #[test]
    fn projects_semantics_without_changing_syntax() {
        let bytes=b"LightmappedGeneric{\"$baseTexture\"\"Wood/Wall\"\"$translucent\"\"1\"\"$envmap\"\"env_cubemap\"\"$surfaceprop\"\"wood\"\"Proxies\"{\"AnimatedTexture\"{\"rate\"\"2\"}}}";
        let Composition::Complete(v) = compose(
            bytes,
            "materials/a.vmt",
            &[],
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap() else {
            panic!()
        };
        let m = resolve(&v).unwrap();
        assert_eq!(m.shader, Shader::LightmappedGeneric);
        assert!(m.features.translucent);
        assert_eq!(
            m.textures[0].logical_path.as_deref(),
            Some("materials/Wood/Wall.vtf")
        );
        assert_eq!(
            m.textures[1].disposition,
            TextureDisposition::BuiltInEnvironment
        );
        assert_eq!(m.proxies[0].name, b"AnimatedTexture");
        assert_eq!(m.surface_property.as_deref(), Some(b"wood".as_slice()));
    }

    fn material(bytes: &[u8], environment: SelectionEnvironment) -> Result<Material, Error> {
        let Composition::Complete(document) = compose(
            bytes,
            "materials/profile.vmt",
            &[],
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("profile material requested a dependency")
        };
        resolve_for_environment(&document, environment)
    }

    #[test]
    fn profile_conditions_replace_only_the_active_value() {
        let bytes = br#"LightmappedGeneric
        {
            "$basetexture" "ordinary"
            "ldr?$basetexture" "selected/ldr"
            "hdr?$basetexture" "selected/hdr"
            "!hdr?$detail" "selected/detail"
        }"#;
        let ldr = material(bytes, SelectionEnvironment::default()).unwrap();
        assert_eq!(
            ldr.first_parameters.get(b"$basetexture".as_slice()),
            Some(&b"selected/ldr".to_vec())
        );
        assert_eq!(
            ldr.first_parameters.get(b"$detail".as_slice()),
            Some(&b"selected/detail".to_vec())
        );
        let hdr = material(
            bytes,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(
            hdr.first_parameters.get(b"$basetexture".as_slice()),
            Some(&b"selected/hdr".to_vec())
        );
        assert!(!hdr.first_parameters.contains_key(b"$detail".as_slice()));
        assert!(hdr.selection_trace.iter().any(|decision| {
            decision
                .source_key
                .eq_ignore_ascii_case(b"ldr?$basetexture")
                && !decision.active
        }));
        assert!(hdr.selection_trace.iter().any(|decision| {
            decision
                .source_key
                .eq_ignore_ascii_case(b"hdr?$basetexture")
                && decision.replaced_earlier_value
        }));
    }

    #[test]
    fn hdr_sky_selects_only_the_complete_hdr_texture_role() {
        let bytes = br#"Sky
        {
            "$basetexture" "sky/ldr"
            "$hdrbasetexture" "sky/hdr"
            "$hdrcompressedtexture" "sky/compressed"
        }"#;
        let hdr = material(
            bytes,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(hdr.shader, Shader::SkyHdr);
        assert_eq!(hdr.selected_textures, [TextureRole::HdrCompressed]);
        let ldr = material(bytes, SelectionEnvironment::default()).unwrap();
        assert_eq!(ldr.shader, Shader::SkyLdr);
        assert_eq!(ldr.selected_textures, [TextureRole::Base]);

        let hdr_base = material(
            br#"Sky { "$basetexture" "sky/ldr" "$hdrbasetexture" "sky/hdr" }"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(hdr_base.selected_textures, [TextureRole::HdrBase]);

        let missing = br#"Sky { "$basetexture" "sky/ldr" }"#;
        assert_eq!(
            material(
                missing,
                SelectionEnvironment {
                    hdr_mode: HdrMode::Integer,
                    ..SelectionEnvironment::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::MissingProfileTexture
        );
    }

    #[test]
    fn unknown_profile_condition_fails_instead_of_becoming_active() {
        let error = material(
            br#"UnlitGeneric { "vulkan?$basetexture" "not-selected" }"#,
            SelectionEnvironment::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::UnsupportedCondition);
        assert_eq!(error.parameter.as_deref(), Some(b"vulkan".as_slice()));
    }

    #[test]
    fn exact_hdr_shader_override_precedes_the_broader_hdr_block() {
        let selected = material(
            br#"LightmappedGeneric {
                "$basetexture" "ordinary"
                "hdr" { "$basetexture" "broad" }
                "hdr_dx9" { "$basetexture" "exact" }
            }"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(
            selected.first_parameters.get(b"$basetexture".as_slice()),
            Some(&b"exact".to_vec())
        );
    }

    #[test]
    fn water_state_retains_render_neutral_requests_and_typed_values() {
        let selected = material(
            br#"Water {
                "$abovewater" "0"
                "$normalmap" "water/current_normal"
                "$envmap" "maps/test/c1_2_3"
                "$reflecttexture" "_rt_WaterReflection"
                "$refracttexture" "_rt_WaterRefraction"
                "$bottommaterial" "water/beneath"
                "$underwateroverlay" "effects/underwater"
                "$reflectamount" "0.25"
                "$refractamount" "0.32"
                "$reflecttint" "[0.5 0.75 1]"
                "$refracttint" "{ 128 64 255 }"
                "$fogenable" "1"
                "$fogcolor" "{ 51 43 13 }"
                "$fogstart" "1"
                "$fogend" "400"
                "$cheapwaterstartdistance" "1000"
                "$cheapwaterenddistance" "2000"
                "$forcecheap" "1"
                "$forceexpensive" "1"
                "$reflectentities" "1"
                "$blurrefract" "1"
                "$scroll1" "[1 2 3]"
                Proxies { TextureScroll { "rate" "0.1" } }
            }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        let water = selected.water.as_ref().unwrap();
        assert!(!water.above_water);
        assert_eq!(
            water.normal_map.logical_path.as_deref(),
            Some("materials/water/current_normal.vtf")
        );
        assert_eq!(
            water.environment_map.logical_path.as_deref(),
            Some("materials/maps/test/c1_2_3.vtf")
        );
        assert_eq!(water.environment_map.color_read, TextureColorRead::Srgb);
        assert_eq!(water.normal_map.color_read, TextureColorRead::Linear);
        assert_eq!(
            water.reflection.disposition,
            TextureDisposition::BuiltInRenderTarget
        );
        assert_eq!(
            water.bottom_material.as_ref().unwrap().logical_path,
            "materials/water/beneath.vmt"
        );
        assert_eq!(water.reflect_amount, 0.25);
        assert_eq!(water.refract_amount, 0.32);
        assert_eq!(water.reflect_tint, [0.5, 0.75, 1.0]);
        assert_eq!(water.refract_tint, [128.0 / 255.0, 64.0 / 255.0, 1.0]);
        let byte_scale = 1.0_f32 / 255.0;
        assert_eq!(
            water.fog.color,
            [51.0 * byte_scale, 43.0 * byte_scale, 13.0 * byte_scale]
        );
        assert!(water.force_cheap);
        assert!(!water.force_expensive);
        assert!(water.reflect_entities);
        assert!(water.blur_refraction);
        assert_eq!(water.scroll[0], [1.0, 2.0, 3.0]);
        assert!(water.has_proxy_program);
    }

    #[test]
    fn water_and_decal_malformed_values_fail_without_defaults() {
        let water = material(
            br#"Water { "$fogcolor" "[1 nope 0]" }"#,
            SelectionEnvironment::default(),
        )
        .unwrap_err();
        assert_eq!(water.code, ErrorCode::InvalidParameter);
        assert_eq!(water.parameter.as_deref(), Some(b"$fogcolor".as_slice()));

        let decal = material(
            br#"UnlitGeneric { "$decalscale" "0" }"#,
            SelectionEnvironment::default(),
        )
        .unwrap_err();
        assert_eq!(decal.code, ErrorCode::InvalidParameter);
        assert_eq!(decal.parameter.as_deref(), Some(b"$decalscale".as_slice()));
    }

    #[test]
    fn detail_scale_preserves_scalar_and_two_axis_vector_forms() {
        let vector = material(
            br#"LightmappedGeneric { "$detail" "overlays/detail001" "$detailscale" "[1.1 2.3]" "$detailblendmode" "0" }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        assert_eq!(vector.detail.unwrap().scale, [1.1, 2.3]);
        let scalar = material(
            br#"LightmappedGeneric { "$detail" "overlays/detail001" "$detailscale" "4" }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        assert_eq!(scalar.detail.unwrap().scale, [4.0, 4.0]);
        for value in ["[1]", "[1 2 3]", "1 2", "[nan 1]"] {
            let source = format!(
                "LightmappedGeneric {{ \"$detail\" \"overlays/detail001\" \"$detailscale\" \"{value}\" }}"
            );
            assert_eq!(
                material(source.as_bytes(), SelectionEnvironment::default())
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidParameter
            );
        }
    }

    #[test]
    fn world_texture_roles_and_static_state_preserve_source_alpha_depth_and_color_reads() {
        let fence = material(
            br#"LightmappedGeneric {
                "$basetexture" "metal/fence"
                "$detail" "detail/metal"
                "$detailblendmode" "0"
                "$bumpmap" "metal/fence_normal"
                "$envmap" "env_cubemap"
                "$alphatest" "1"
                "$alphatestreference" "0.35"
            }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        assert_eq!(fence.active_textures.len(), 4);
        assert_eq!(fence.textures[0].color_read, TextureColorRead::Srgb);
        assert_eq!(
            fence.detail.as_ref().unwrap().texture.color_read,
            TextureColorRead::Linear
        );
        assert_eq!(
            fence.bump.as_ref().unwrap().primary.color_read,
            TextureColorRead::Linear
        );
        assert_eq!(
            fence.environment_map.as_ref().unwrap().texture.color_read,
            TextureColorRead::Srgb
        );
        let state = static_state(&fence, TextureAlphaFacts { base: true }).unwrap();
        assert_eq!(state.lighting, LightingModel::BumpedLightmapped);
        assert!(state.alpha_test);
        assert_eq!(state.alpha_test_reference, 0.35);
        assert!(!state.blend.enabled);
        assert!(state.depth_write);

        let decal = material(
            br#"LightmappedGeneric {
                "$basetexture" "signs/number_01"
                "$decal" "1"
                "$translucent" "1"
                "$vertexcolor" "1"
                "$vertexalpha" "1"
            }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        let state = static_state(&decal, TextureAlphaFacts { base: true }).unwrap();
        assert_eq!(
            state.blend,
            BlendState {
                enabled: true,
                equation: BlendEquation::Add,
                source: BlendFactor::SourceAlpha,
                destination: BlendFactor::OneMinusSourceAlpha
            }
        );
        assert!(!state.depth_write);
        assert_eq!(state.polygon_offset, PolygonOffset::Decal);
        assert!(state.vertex_color);
        assert!(state.vertex_alpha);
        assert!(state.translucent_queue);

        let hdr = material(
            br#"LightmappedGeneric { "$basetexture" "base" "$envmap" "maps/test/c0_0_0" }"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(
            hdr.environment_map.unwrap().texture.color_read,
            TextureColorRead::Linear
        );
    }

    #[test]
    fn sky_compressed_hdr_is_linear_and_author_flags_use_integer_conversion() {
        let sky = material(
            br#"Sky {
                "$basetexture" "sky/ldr"
                "$hdrcompressedtexture" "sky/hdr"
                "$translucent" "true"
                "$nocull" "1.0"
            }"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(sky.selected_textures, [TextureRole::HdrCompressed]);
        assert_eq!(
            sky.textures
                .iter()
                .find(|texture| texture.role == TextureRole::HdrCompressed)
                .unwrap()
                .color_read,
            TextureColorRead::Linear
        );
        assert!(!sky.features.translucent);
        assert!(sky.features.no_cull);
        let state = static_state(&sky, TextureAlphaFacts { base: false }).unwrap();
        assert!(!state.depth_test);
        assert!(!state.depth_write);
        assert_eq!(state.fog, FogMode::Disabled);
    }

    #[test]
    fn target_vertex_lit_and_eye_materials_emit_complete_model_state() {
        let launcher = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/weapons/c_models/c_launcher/c_launcher"
                "$phongexponenttexture" "models/weapons/c_models/c_launcher/c_launcher_exp"
                "$phong" "1" "$phongboost" "2.75" "$phongexponentfactor" "100"
                "$phongalbedotint" "1" "$lightwarptexture" "models/lightwarps/softened_weapon_lightwarp"
                "$phongfresnelranges" "[.3 .5 3]" "$halflambert" "1"
                "$envmap" "env_cubemap" "$basemapalphaphongmask" "1"
                "$rimlight" "1" "$rimlightexponent" "2" "$rimlightboost" "2.5" "$rimmask" "1"
                "$cloakPassEnabled" "1" "$sheenPassEnabled" "1"
                "$sheenmap" "cubemaps/cubemap_sheen001" "$sheenmapmask" "Effects/AnimatedSheen/animatedsheen0"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        let ModelShaderState::VertexLitGeneric(state) = &launcher.model.as_ref().unwrap().state
        else {
            panic!("launcher did not resolve VertexLitGeneric model state")
        };
        let phong = state.phong.as_ref().unwrap();
        assert_eq!(phong.exponent_factor, 100.0);
        assert_eq!(phong.mask_source, PhongMaskSource::BaseAlpha);
        assert_eq!(phong.fresnel_ranges, [0.3, 0.5, 3.0]);
        assert_eq!(phong.packed_fresnel_ranges, [0.39999998, 0.5, 5.0]);
        assert_eq!(
            phong.rim,
            Some(RimLightState {
                exponent: 2.0,
                boost: 2.5,
                exponent_texture_alpha_mask: true,
            })
        );
        assert!(state.half_lambert);
        assert!(state.cloak.enabled);
        assert!(state.sheen.enabled);
        assert!(
            launcher
                .model_textures
                .iter()
                .any(|texture| texture.role == ModelTextureRole::PhongExponent)
        );
        assert!(
            launcher
                .model_textures
                .iter()
                .any(|texture| texture.role == ModelTextureRole::LightWarp)
        );
        let hdr_sheen = material(
            br#"VertexLitGeneric { "$sheenmap" "cubemaps/cubemap_sheen001" }"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(
            hdr_sheen.model_textures[0].color_read,
            TextureColorRead::Linear
        );

        let eye = material(
            br#"EyeRefract {
                "$iris" "models/player/shared/eye-iris-blue"
                "$ambientoccltexture" "models/player/shared/eye-extra"
                "$envmap" "models/player/shared/eye-reflection-cubemap-"
                "$corneatexture" "models/player/shared/eye-cornea"
                "$lightwarptexture" "models/player/shared/eye_lightwarp"
                "$eyeballradius" "0.7" "$ambientocclcolor" "[1 1 1]"
                "$dilation" "0.5" "$parallaxstrength" "0.25" "$corneabumpstrength" "1"
                "$halflambert" "1" "$raytracesphere" "0" "$spheretexkillcombo" "0"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(eye.shader, Shader::Unsupported);
        let ModelShaderState::EyeRefract(state) = &eye.model.as_ref().unwrap().state else {
            panic!("eye did not resolve EyeRefract model state")
        };
        assert_eq!(state.eyeball_radius, 0.7);
        assert_eq!(state.glossiness, 1.0);
        assert_eq!(state.ambient_occlusion_color, [1.0; 3]);
        assert!(!state.raytrace_sphere);
        assert!(!state.sphere_texture_kill);
        assert_eq!(
            static_state(&eye, TextureAlphaFacts { base: false })
                .unwrap()
                .lighting,
            LightingModel::VertexLit
        );
    }

    #[test]
    fn unlit_model_state_excludes_vertex_lighting_and_retains_authored_static_inputs() {
        let unlit = material(
            br#"UnLitGeneric {
                "$basetexture" "models/props_ui/bannerflag_comp"
                "$detail" "overlays/detail001"
                "$detailscale" "[1.1 2.3]"
                "$translucent" "1"
                "$nocull" "1"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        let model = unlit.model.as_ref().unwrap();
        assert_eq!(model.shader, ModelShader::UnlitGeneric);
        assert_eq!(
            model.vertex_requirements,
            ModelVertexRequirements {
                position: true,
                normal: false,
                tangent_space: false,
                texture_coordinate_0: true,
                ambient_cube: false,
                local_lights: false,
                camera_position: false,
                studio_eye_parameters: false,
            }
        );
        let ModelShaderState::UnlitGeneric(state) = &model.state else {
            panic!("unlit model state was not selected")
        };
        assert!(state.base.is_some() && state.detail.is_some());
        let draw = model_draw_state(
            &unlit,
            TextureAlphaFacts { base: true },
            ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: None,
            },
        )
        .unwrap();
        assert_eq!(draw.static_state.lighting, LightingModel::Unlit);
        assert_eq!(draw.static_state.cull, CullState::None);
        assert_eq!(draw.opacity, ModelOpacity::Translucent);
        assert!(!draw.required_inputs.contains(&ModelDrawInput::AmbientCube));
        assert!(!draw.required_inputs.contains(&ModelDrawInput::LocalLights));
    }

    #[test]
    fn model_draw_state_keeps_runtime_opacity_framebuffer_and_missing_inputs_explicit() {
        let cloak = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/test/base"
                "$envmap" "env_cubemap"
                "$cloakPassEnabled" "1"
                Proxies { invis {} }
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert_eq!(
            model_draw_state(
                &cloak,
                TextureAlphaFacts { base: false },
                ModelRuntimeInputs {
                    alpha_modulation: 1.0,
                    cloak_factor: None,
                },
            )
            .unwrap_err()
            .code,
            ErrorCode::MissingModelInput
        );
        assert_eq!(
            model_draw_state(
                &cloak,
                TextureAlphaFacts { base: false },
                ModelRuntimeInputs {
                    alpha_modulation: 1.0,
                    cloak_factor: Some(f32::NAN),
                },
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidParameter
        );
        let endpoint = model_draw_state(
            &cloak,
            TextureAlphaFacts { base: false },
            ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: Some(0.0),
            },
        )
        .unwrap();
        assert_eq!(endpoint.opacity, ModelOpacity::Opaque);
        assert_eq!(endpoint.framebuffer, ModelFramebufferRequirement::Potential);
        assert_eq!(
            endpoint.required_inputs,
            [
                ModelDrawInput::AmbientCube,
                ModelDrawInput::LocalLights,
                ModelDrawInput::CameraPosition,
                ModelDrawInput::LocalEnvironment,
                ModelDrawInput::AuthoredTexturePlanes,
                ModelDrawInput::GameProxyValues,
            ]
        );
        let interior = model_draw_state(
            &cloak,
            TextureAlphaFacts { base: false },
            ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: Some(0.5),
            },
        )
        .unwrap();
        assert_eq!(interior.opacity, ModelOpacity::Translucent);
        assert_eq!(interior.framebuffer, ModelFramebufferRequirement::Current);
        assert!(
            interior
                .required_inputs
                .contains(&ModelDrawInput::CurrentFramebuffer)
        );
        assert_eq!(
            missing_model_draw_inputs(
                &interior,
                &[
                    ModelDrawInput::AmbientCube,
                    ModelDrawInput::LocalLights,
                    ModelDrawInput::CameraPosition,
                ],
            ),
            [
                ModelDrawInput::LocalEnvironment,
                ModelDrawInput::CurrentFramebuffer,
                ModelDrawInput::AuthoredTexturePlanes,
                ModelDrawInput::GameProxyValues,
            ]
        );

        let alpha_modulated = model_draw_state(
            &cloak,
            TextureAlphaFacts { base: false },
            ModelRuntimeInputs {
                alpha_modulation: 0.5,
                cloak_factor: Some(0.0),
            },
        )
        .unwrap();
        assert_eq!(alpha_modulated.opacity, ModelOpacity::Translucent);

        let flagged = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/test/opaque"
                "$translucent" "1"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        let flagged = model_draw_state(
            &flagged,
            TextureAlphaFacts { base: false },
            ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: None,
            },
        )
        .unwrap();
        assert_eq!(flagged.opacity, ModelOpacity::Translucent);
        assert!(!flagged.static_state.blend.enabled);

        let animated_alpha = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/test/opaque"
                "$alpha" "0.25"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert!(
            static_state(&animated_alpha, TextureAlphaFacts { base: false })
                .unwrap()
                .blend
                .enabled
        );
        let current = model_draw_state(
            &animated_alpha,
            TextureAlphaFacts { base: false },
            ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: None,
            },
        )
        .unwrap();
        assert_eq!(current.opacity, ModelOpacity::Opaque);
        assert!(!current.static_state.blend.enabled);
    }

    #[test]
    fn model_alpha_ownership_controls_self_illumination_and_alpha_test() {
        let selected = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/test/base"
                "$selfillum" "1"
                "$alphatest" "1"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        let opaque_base = static_state(&selected, TextureAlphaFacts { base: false }).unwrap();
        assert!(opaque_base.alpha_test);
        let alpha_base = static_state(&selected, TextureAlphaFacts { base: true }).unwrap();
        assert!(!alpha_base.alpha_test);
        assert!(!alpha_base.blend.enabled);

        let eye = material(
            br#"Eyes {
                "$basetexture" "models/test/eye-white"
                "$iris" "models/test/iris"
                "$glint" "models/test/glint"
                "$cloakPassEnabled" "1"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        let draw = model_draw_state(
            &eye,
            TextureAlphaFacts { base: false },
            ModelRuntimeInputs {
                alpha_modulation: 1.0,
                cloak_factor: Some(1.0),
            },
        )
        .unwrap();
        assert_eq!(draw.opacity, ModelOpacity::Opaque);
        assert!(
            draw.required_inputs
                .contains(&ModelDrawInput::StudioEyeParameters)
        );
        assert!(
            draw.required_inputs
                .contains(&ModelDrawInput::CameraPosition)
        );
    }

    #[test]
    fn stock_model_proxy_program_uses_ordered_typed_inputs() {
        let selected = material(
            br#"VertexLitGeneric {
                "$color2" "[1 1 1]" "$yellow" "0"
                Proxies {
                    AnimatedWeaponSheen { "animatedtexturevar" "$sheenmapmask" "animatedtextureframenumvar" "$sheenmapmaskframe" }
                    invis {}
                    ModelGlowColor { "resultVar" "$glowcolor" }
                    Equals { "srcVar1" "$glowcolor" "resultVar" "$selfillumtint" }
                    Equals { "srcVar1" "$glowcolor" "resultVar" "$color2" }
                    YellowLevel { "resultVar" "$yellow" }
                    Multiply { "srcVar1" "$color2" "srcVar2" "$yellow" "resultVar" "$color2" }
                    WeaponSkin {}
                }
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert!(
            selected
                .proxy_program
                .entries
                .iter()
                .all(|entry| entry.disposition == ProxyDisposition::Handled)
        );
        let initial = BTreeMap::from([(
            b"$color2".to_vec(),
            ProxyValue::Vector {
                values: [1.0, 1.0, 1.0, 0.0],
                size: 3,
            },
        )]);
        let context = ProxyEvaluationContext {
            time: 0.0,
            frame_time: 0.0,
            water_lod: None,
            texture_frames: BTreeMap::new(),
            model_inputs: ModelProxyInputs {
                invisibility: Some(InvisibilityInput {
                    factor: 0.0,
                    player_tint: None,
                }),
                model_glow_color: Some([1.0; 3]),
                yellow_level: Some([1.0; 3]),
                weapon_sheen: Some(WeaponSheenInput {
                    frame: 0,
                    tint: [0.0; 4],
                    mask_scale: [1.0; 2],
                    mask_offset: [0.0; 2],
                    mask_direction: 0,
                    shader_index: 0,
                    enabled: false,
                }),
                weapon_skin_base_texture: Some(None),
                ..ModelProxyInputs::default()
            },
        };
        let evaluated =
            evaluate_proxy_program(&selected.proxy_program, &initial, &context).unwrap();
        assert_eq!(evaluated.trace.len(), 8);
        assert_eq!(
            evaluated.variables[b"$color2".as_slice()],
            ProxyValue::Vector {
                values: [1.0, 1.0, 1.0, 0.0],
                size: 3,
            }
        );
        assert_eq!(
            evaluated.variables[b"$cloakfactor".as_slice()],
            ProxyValue::Float(0.0)
        );
        assert_eq!(
            evaluated.effects,
            [ModelProxyEffect::WeaponSkinBaseTexture(None)]
        );
        let mut enabled_context = context.clone();
        enabled_context.model_inputs.weapon_sheen = Some(WeaponSheenInput {
            frame: 7,
            tint: [0.25, 0.5, 0.75, 0.6],
            mask_scale: [32.0, 8.0],
            mask_offset: [-4.0, 2.0],
            mask_direction: 2,
            shader_index: 1,
            enabled: true,
        });
        let enabled =
            evaluate_proxy_program(&selected.proxy_program, &initial, &enabled_context).unwrap();
        assert_eq!(
            enabled.variables[b"$sheenmapmaskframe".as_slice()],
            ProxyValue::Int(7)
        );
        assert_eq!(
            enabled.variables[b"$sheenmapmaskscalex".as_slice()],
            ProxyValue::Float(32.0)
        );
        assert_eq!(
            enabled.variables[b"$sheenmapmaskdirection".as_slice()],
            ProxyValue::Int(2)
        );

        let missing = evaluate_proxy_program(
            &selected.proxy_program,
            &initial,
            &ProxyEvaluationContext::default(),
        )
        .unwrap_err();
        assert_eq!(missing.code, ProxyEvaluationErrorCode::MissingModelInput);
        assert_eq!(missing.operation, 0);
    }

    #[test]
    fn target_player_item_and_sticky_proxies_use_only_supplied_values() {
        let selected = material(
            br#"VertexLitGeneric {
                Proxies {
                    InvulnLevel { "resultVar" "$invuln" }
                    BurnLevel { "resultVar" "$burn" }
                    ItemTintColor { "resultVar" "$tint" }
                    StickybombGlowColor { "resultVar" "$glow" }
                    weapon_invis {}
                    LessOrEqual {
                        "srcVar1" "$burn" "srcVar2" "$threshold"
                        "lessEqualVar" "$low" "greaterVar" "$high" "resultVar" "$selected"
                    }
                    SelectFirstIfNonZero { "srcVar1" "$tint" "srcVar2" "$fallback" "resultVar" "$color" }
                }
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        )
        .unwrap();
        assert!(
            selected
                .proxy_program
                .entries
                .iter()
                .all(|entry| entry.disposition == ProxyDisposition::Handled)
        );
        let vector = |values: [f32; 3]| ProxyValue::Vector {
            values: [values[0], values[1], values[2], 0.0],
            size: 3,
        };
        let initial = BTreeMap::from([
            (b"$threshold".to_vec(), ProxyValue::Float(0.25)),
            (b"$low".to_vec(), ProxyValue::Float(2.0)),
            (b"$high".to_vec(), ProxyValue::Float(4.0)),
            (b"$selected".to_vec(), ProxyValue::Float(0.0)),
            (b"$fallback".to_vec(), vector([0.5; 3])),
            (b"$color".to_vec(), vector([0.0; 3])),
        ]);
        let evaluated = evaluate_proxy_program(
            &selected.proxy_program,
            &initial,
            &ProxyEvaluationContext {
                model_inputs: ModelProxyInputs {
                    invisibility: Some(InvisibilityInput {
                        factor: 0.75,
                        player_tint: None,
                    }),
                    invulnerability_level: Some(1.0),
                    burn_level: Some(0.5),
                    item_tint: Some([0.0; 3]),
                    stickybomb_glow: Some([100.0, 0.0, 0.0]),
                    ..ModelProxyInputs::default()
                },
                ..ProxyEvaluationContext::default()
            },
        )
        .unwrap();
        assert_eq!(
            evaluated.variables[b"$selected".as_slice()],
            ProxyValue::Float(4.0)
        );
        assert_eq!(evaluated.variables[b"$color".as_slice()], vector([0.5; 3]));
        assert_eq!(
            evaluated.variables[b"$cloakfactor".as_slice()],
            ProxyValue::Float(0.75)
        );
        assert_eq!(
            evaluated.variables[b"$glow".as_slice()],
            vector([100.0, 0.0, 0.0])
        );
    }

    #[test]
    fn target_water_proxy_family_evaluates_in_source_order_from_supplied_inputs() {
        let selected = material(
            br#"Water {
                "$normalmap" "water/current_normal"
                "$temp" "[0 0]"
                "$curr" "0"
                "$curr2" "0"
                Proxies {
                    AnimatedTexture {
                        "animatedtexturevar" "$normalmap"
                        "animatedtextureframenumvar" "$bumpframe"
                        "animatedtextureframerate" "30"
                    }
                    Sine { "sineperiod" "24" "sinemin" "-0.5" "sinemax" "0.5" "resultVar" "$curr" }
                    Sine { "sineperiod" "16" "sinemin" "0.5" "sinemax" "-0.5" "resultVar" "$curr2" }
                    Equals { "srcVar1" "$curr2" "resultVar" "$temp[0]" }
                    Equals { "srcVar1" "$curr" "resultVar" "$temp[1]" }
                    TextureTransform { "translateVar" "$temp" "resultVar" "$bumptransform" }
                    WaterLOD {}
                }
            }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        assert_eq!(selected.proxy_program.entries.len(), 7);
        assert!(
            selected
                .proxy_program
                .entries
                .iter()
                .all(|entry| entry.disposition == ProxyDisposition::Handled)
        );
        let mut initial = BTreeMap::from([
            (b"$curr".to_vec(), ProxyValue::Float(0.0)),
            (b"$curr2".to_vec(), ProxyValue::Float(0.0)),
            (
                b"$temp".to_vec(),
                ProxyValue::Vector {
                    values: [0.0; 4],
                    size: 2,
                },
            ),
        ]);
        initial.insert(b"$bumpframe".to_vec(), ProxyValue::Int(0));
        let evaluated = evaluate_proxy_program(
            &selected.proxy_program,
            &initial,
            &ProxyEvaluationContext {
                time: 4.0,
                frame_time: 0.5,
                water_lod: Some([1_000.0, 2_000.0]),
                texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), 60)]),
                model_inputs: ModelProxyInputs::default(),
            },
        )
        .unwrap();
        assert_eq!(
            evaluated.variables[b"$bumpframe".as_slice()],
            ProxyValue::Int(0)
        );
        let ProxyValue::Vector { values, size } = evaluated.variables[b"$temp".as_slice()] else {
            panic!("proxy temp was not a vector")
        };
        assert_eq!(size, 2);
        assert_eq!(values[0], -0.5);
        assert!((values[1] - 0.433_012_7).abs() < 1.0e-6);
        let ProxyValue::Matrix(matrix) = evaluated.variables[b"$bumptransform".as_slice()] else {
            panic!("proxy transform was not a matrix")
        };
        assert_eq!(matrix[3], values[0]);
        assert_eq!(matrix[7], values[1]);
        assert_eq!(
            evaluated.variables[b"$cheapwaterstartdistance".as_slice()],
            ProxyValue::Float(1_000.0)
        );
        assert_eq!(
            evaluated.variables[b"$cheapwaterenddistance".as_slice()],
            ProxyValue::Float(2_000.0)
        );
        assert_eq!(evaluated.trace.len(), 7);
    }

    #[test]
    fn malformed_and_unknown_proxies_remain_explicit_no_operations() {
        let selected = material(
            br#"Water { Proxies { AnimatedTexture {} FutureWaterProxy { "x" "1" } } }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        assert_eq!(
            selected.proxy_program.entries[0].disposition,
            ProxyDisposition::Malformed
        );
        assert_eq!(
            selected.proxy_program.entries[1].disposition,
            ProxyDisposition::Unsupported
        );
        let evaluated = evaluate_proxy_program(
            &selected.proxy_program,
            &BTreeMap::new(),
            &ProxyEvaluationContext {
                time: 0.0,
                frame_time: 0.0,
                water_lod: None,
                texture_frames: BTreeMap::new(),
                model_inputs: ModelProxyInputs::default(),
            },
        )
        .unwrap();
        assert_eq!(evaluated.trace[0].disposition, ProxyDisposition::Malformed);
        assert_eq!(
            evaluated.trace[1].disposition,
            ProxyDisposition::Unsupported
        );
    }

    #[test]
    fn texture_scroll_wraps_offsets_and_animated_no_wrap_clamps_last_frame() {
        let selected = material(
            br#"Water { Proxies {
                TextureScroll {
                    "texturescrollvar" "$bumptransform"
                    "texturescrollrate" "0.05"
                    "texturescrollangle" "45"
                }
                AnimatedTexture {
                    "animatedtexturevar" "$normalmap"
                    "animatedtextureframenumvar" "$bumpframe"
                    "animatedtextureframerate" "4"
                    "animationnowrap" "1"
                }
            } }"#,
            SelectionEnvironment::default(),
        )
        .unwrap();
        let evaluated = evaluate_proxy_program(
            &selected.proxy_program,
            &BTreeMap::new(),
            &ProxyEvaluationContext {
                time: 1.0,
                frame_time: 0.5,
                water_lod: None,
                texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), 4)]),
                model_inputs: ModelProxyInputs::default(),
            },
        )
        .unwrap();
        let ProxyValue::Matrix(matrix) = evaluated.variables[b"$bumptransform".as_slice()] else {
            panic!("texture scroll result was not a matrix")
        };
        let expected = 45.0_f32.to_radians().cos() * 0.05;
        assert!((matrix[3] - expected).abs() < 1.0e-7);
        assert!((matrix[7] - expected).abs() < 1.0e-7);
        assert_eq!(
            evaluated.variables[b"$bumpframe".as_slice()],
            ProxyValue::Int(3)
        );
    }
}
