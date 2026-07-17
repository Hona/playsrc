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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Features {
    pub translucent: bool,
    pub additive: bool,
    pub alpha_test: bool,
    pub no_cull: bool,
    pub self_illum: bool,
    pub ss_bump: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
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
    ];
    let mut textures = Vec::new();
    for (name, role) in specs {
        if let Some(reference) = get(&first, name) {
            textures.push(texture(name, role, reference)?);
        }
    }
    let selected_textures = selected_textures(shader, environment, &textures)?;
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
        let suffix = if lower.ends_with(".vtf") { "" } else { ".vtf" };
        let path = format!("materials/{normalized}{suffix}");
        if path
            .split('/')
            .any(|x| x.is_empty() || x == "." || x == "..")
        {
            return Err(error(ErrorCode::InvalidPath, Some(parameter.to_vec())));
        }
        Some(path)
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
}
