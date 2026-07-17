use playsrc_vmt::{EffectiveDocument, EffectiveNode, EffectiveValue};
use std::{collections::BTreeMap, fmt};
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureRequest {
    pub role: TextureRole,
    pub parameter: Vec<u8>,
    pub reference: Vec<u8>,
    pub logical_path: Option<String>,
    pub disposition: TextureDisposition,
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
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Features {
    pub translucent: bool,
    pub additive: bool,
    pub alpha_test: bool,
    pub no_cull: bool,
    pub self_illum: bool,
    pub ss_bump: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub shader_token: Vec<u8>,
    pub shader: Shader,
    pub parameters: Vec<(Vec<u8>, Vec<u8>)>,
    pub first_parameters: BTreeMap<Vec<u8>, Vec<u8>>,
    pub textures: Vec<TextureRequest>,
    pub selected_textures: Vec<TextureRole>,
    pub selection_trace: Vec<SelectionDecision>,
    pub proxies: Vec<Proxy>,
    pub surface_property: Option<Vec<u8>>,
    pub features: Features,
    pub material_requests: Vec<MaterialRequest>,
    pub water: Option<WaterState>,
    pub decal: DecalState,
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
    let mut textures = Vec::new();
    for (name, role) in specs {
        if let Some(reference) = get(&first, name) {
            textures.push(texture(name, role, reference)?);
        }
    }
    let selected_textures = selected_textures(shader, environment, &textures)?;
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
    let decal_scale = float_or(&first, b"$decalscale", 1.0)?;
    if decal_scale <= 0.0 {
        return Err(error(
            ErrorCode::InvalidParameter,
            Some(b"$decalscale".to_vec()),
        ));
    }
    Ok(Material {
        shader_token: document.root.key.bytes.clone(),
        shader,
        parameters,
        first_parameters: first.clone(),
        textures,
        selected_textures,
        selection_trace,
        proxies,
        surface_property: get(&first, b"$surfaceprop").cloned(),
        features: Features {
            translucent: boolean(&first, b"$translucent"),
            additive: boolean(&first, b"$additive"),
            alpha_test: boolean(&first, b"$alphatest"),
            no_cull: boolean(&first, b"$nocull"),
            self_illum: boolean(&first, b"$selfillum"),
            ss_bump: boolean(&first, b"$ssbump"),
        },
        material_requests,
        water,
        decal: DecalState {
            scale: decal_scale,
            suppress_decals: boolean(&first, b"$nodecal"),
            alpha_tested: boolean(&first, b"$alphatest"),
        },
    })
}

fn water_state(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    material_requests: &[MaterialRequest],
    proxies: &[Proxy],
) -> Result<WaterState, Error> {
    let request = |parameter: &'static [u8], role, default: &'static [u8]| {
        texture(
            parameter,
            role,
            get(parameters, parameter).map_or(default, Vec::as_slice),
        )
    };
    let force_cheap = boolean_or(parameters, b"$forcecheap", false)?;
    let mut force_expensive = boolean_or(parameters, b"$forceexpensive", true)?;
    if force_cheap && force_expensive {
        force_expensive = false;
    }
    Ok(WaterState {
        above_water: boolean_or(parameters, b"$abovewater", true)?,
        normal_map: request(b"$normalmap", TextureRole::Normal, b"dev/water_normal")?,
        environment_map: request(b"$envmap", TextureRole::Environment, b"env_cubemap")?,
        reflection: request(
            b"$reflecttexture",
            TextureRole::Reflection,
            b"_rt_WaterReflection",
        )?,
        refraction: request(
            b"$refracttexture",
            TextureRole::Refraction,
            b"_rt_WaterRefraction",
        )?,
        bottom_material: material_requests
            .iter()
            .find(|request| request.role == MaterialRole::Bottom)
            .cloned(),
        underwater_overlay: material_requests
            .iter()
            .find(|request| request.role == MaterialRole::UnderwaterOverlay)
            .cloned(),
        reflect_amount: float_or(parameters, b"$reflectamount", 0.8)?,
        refract_amount: float_or(parameters, b"$refractamount", 0.0)?,
        reflect_tint: color_or(parameters, b"$reflecttint", [1.0; 3])?,
        refract_tint: color_or(parameters, b"$refracttint", [1.0; 3])?,
        fog: WaterFog {
            enabled: boolean_or(parameters, b"$fogenable", true)?,
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
fn scalar_pair(node: &EffectiveNode) -> Option<(Vec<u8>, Vec<u8>)> {
    let EffectiveValue::Scalar(v) = &node.value else {
        return None;
    };
    Some((node.key.bytes.clone(), v.token.bytes.clone()))
}
fn lower(v: &[u8]) -> Vec<u8> {
    v.iter().map(u8::to_ascii_lowercase).collect()
}
fn get<'a>(m: &'a BTreeMap<Vec<u8>, Vec<u8>>, k: &[u8]) -> Option<&'a Vec<u8>> {
    m.get(&lower(k))
}
fn boolean(m: &BTreeMap<Vec<u8>, Vec<u8>>, k: &[u8]) -> bool {
    let Some(v) = get(m, k) else { return false };
    if v.eq_ignore_ascii_case(b"true") {
        return true;
    }
    if v.eq_ignore_ascii_case(b"false") {
        return false;
    }
    std::str::from_utf8(v)
        .ok()
        .and_then(|x| x.parse::<f32>().ok())
        .is_some_and(|x| x != 0.)
}
fn boolean_or(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: bool,
) -> Result<bool, Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(default);
    };
    if value.eq_ignore_ascii_case(b"true") {
        return Ok(true);
    }
    if value.eq_ignore_ascii_case(b"false") {
        return Ok(false);
    }
    let parsed = std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .ok_or_else(|| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?;
    Ok(parsed != 0.0)
}
fn float_or(
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
fn color_or(
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
    } else if v.eq_ignore_ascii_case(b"Sprite") {
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
fn logical_path(normalized: &str, extension: &str, parameter: &[u8]) -> Result<String, Error> {
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
fn error(code: ErrorCode, parameter: Option<Vec<u8>>) -> Error {
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
}
