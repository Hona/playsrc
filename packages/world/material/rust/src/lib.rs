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
    Unsupported,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureRole {
    Base,
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
    pub proxies: Vec<Proxy>,
    pub surface_property: Option<Vec<u8>>,
    pub features: Features,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    RootKind,
    InvalidReference,
    InvalidPath,
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
    let EffectiveValue::Object(children) = &document.root.value else {
        return Err(error(ErrorCode::RootKind, None));
    };
    let shader = shader(&document.root.key.bytes);
    let mut parameters = Vec::new();
    let mut first = BTreeMap::new();
    let mut proxies = Vec::new();
    for child in children {
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
    Ok(Material {
        shader_token: document.root.key.bytes.clone(),
        shader,
        parameters,
        first_parameters: first.clone(),
        textures,
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
}
