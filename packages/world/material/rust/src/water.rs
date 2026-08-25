use crate::{
    EffectiveParameter, Error, ErrorCode, EvaluatedProxyState, HdrMode, Material, MaterialRequest,
    ParameterOrigin, ProxyEvaluationContext, ProxyEvaluationError, ProxyOperation, ProxyValue,
    Shader, TextureDisposition, TextureRequest, TextureRole, TextureTransformState, color_or,
    error, float_or, get, identity_matrix, integer_or,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterShaderVariant {
    Dx90,
    Dx9Hdr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterSurfaceOpacity {
    Opaque,
    Translucent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterInputRequirement {
    AuthoredTexturePlanes(TextureRole),
    Lightmap,
    LocalEnvironment,
    ReflectionFramebuffer,
    RefractionFramebuffer,
    CameraPosition,
    WaterSurfacePlane,
    EyeWaterSide,
    DistanceToWater,
    RuntimeWaterPolicy,
    WaterFogVolume,
    PresentationTime,
    WaterLodController,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterTextureBindings {
    pub base: Option<TextureRequest>,
    pub normal: Option<TextureRequest>,
    pub flow: Option<TextureRequest>,
    pub environment: Option<TextureRequest>,
    pub reflection: Option<TextureRequest>,
    pub refraction: Option<TextureRequest>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterFogOutput {
    pub enabled: Option<EffectiveParameter<bool>>,
    pub color: EffectiveParameter<[f32; 3]>,
    pub start: EffectiveParameter<f32>,
    pub end: EffectiveParameter<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterFresnelOutput {
    pub cheap_enabled: bool,
    pub expensive_constant: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterMaterialOutput {
    pub shader: WaterShaderVariant,
    pub textures: WaterTextureBindings,
    pub bottom_material: Option<MaterialRequest>,
    pub underwater_overlay: Option<MaterialRequest>,
    pub base_frame: EffectiveParameter<i32>,
    pub normal_frame: EffectiveParameter<i32>,
    pub environment_frame: EffectiveParameter<i32>,
    pub normal_transform: TextureTransformState,
    pub scale: EffectiveParameter<[f32; 2]>,
    pub time: EffectiveParameter<f32>,
    pub water_depth: EffectiveParameter<f32>,
    pub above_water: EffectiveParameter<bool>,
    pub reflect_amount: EffectiveParameter<f32>,
    pub refract_amount: EffectiveParameter<f32>,
    pub reflect_tint: EffectiveParameter<[f32; 3]>,
    pub refract_tint: EffectiveParameter<[f32; 3]>,
    pub reflection_blend_factor: EffectiveParameter<f32>,
    pub fog: WaterFogOutput,
    pub cheap_start: EffectiveParameter<f32>,
    pub cheap_end: EffectiveParameter<f32>,
    pub force_cheap: EffectiveParameter<bool>,
    pub force_expensive: EffectiveParameter<bool>,
    pub reflect_entities: EffectiveParameter<bool>,
    pub blur_refraction: EffectiveParameter<bool>,
    pub no_low_end_lightmap: EffectiveParameter<bool>,
    pub scroll: [EffectiveParameter<[f32; 3]>; 2],
    pub fresnel: WaterFresnelOutput,
    pub opacity: WaterSurfaceOpacity,
    pub required_inputs: Vec<WaterInputRequirement>,
}

pub fn missing_water_inputs(
    state: &WaterMaterialOutput,
    available: &[WaterInputRequirement],
) -> Vec<WaterInputRequirement> {
    state
        .required_inputs
        .iter()
        .copied()
        .filter(|required| !available.contains(required))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedWaterState {
    pub normal_frame: i32,
    pub normal_transform: [f32; 16],
    pub cheap_start: f32,
    pub cheap_end: f32,
    pub proxy: EvaluatedProxyState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaterEvaluationError {
    Material(Error),
    Proxy(ProxyEvaluationError),
    MissingVariable(Vec<u8>),
    InvalidVariable(Vec<u8>),
}

pub fn water_material_output(material: &Material) -> Result<Option<WaterMaterialOutput>, Error> {
    if material.shader != Shader::Water {
        return Ok(None);
    }
    let parameters = &material.first_parameters;
    let textures = WaterTextureBindings {
        base: texture(material, TextureRole::Base),
        normal: texture(material, TextureRole::Normal),
        flow: texture(material, TextureRole::Flow),
        environment: texture(material, TextureRole::Environment),
        reflection: texture(material, TextureRole::Reflection),
        refraction: texture(material, TextureRole::Refraction),
    };
    let base_frame = effective_integer(parameters, b"$frame", 0, ParameterOrigin::TypeInitializer)?;
    let normal_frame = effective_integer(
        parameters,
        b"$bumpframe",
        0,
        ParameterOrigin::TypeInitializer,
    )?;
    let environment_frame = effective_integer(
        parameters,
        b"$envmapframe",
        0,
        ParameterOrigin::TypeInitializer,
    )?;
    let normal_transform = material
        .texture_uses
        .iter()
        .find(|usage| usage.role == TextureRole::Normal)
        .and_then(|usage| usage.transform.clone())
        .unwrap_or_else(|| TextureTransformState {
            parameter: b"$bumptransform".to_vec(),
            matrix: identity_matrix(),
            origin: ParameterOrigin::TypeInitializer,
            proxy_mutated: proxy_writes_bump_transform(material),
        });
    let force_cheap = effective_bool(
        parameters,
        b"$forcecheap",
        false,
        ParameterOrigin::TypeInitializer,
    );
    let mut force_expensive = effective_bool(
        parameters,
        b"$forceexpensive",
        true,
        ParameterOrigin::ShaderInitializer,
    );
    if force_cheap.value && force_expensive.value {
        force_expensive.value = false;
        force_expensive.origin = ParameterOrigin::ShaderInitializer;
    }
    let no_fresnel = effective_bool(
        parameters,
        b"$nofresnel",
        false,
        ParameterOrigin::TypeInitializer,
    );
    let fog_enabled = get(parameters, b"$fogenable")
        .map(|_| effective_bool(parameters, b"$fogenable", false, ParameterOrigin::Authored));
    let mut required_inputs = Vec::new();
    for request in [
        textures.base.as_ref(),
        textures.normal.as_ref(),
        textures.flow.as_ref(),
        textures.environment.as_ref(),
        textures.reflection.as_ref(),
        textures.refraction.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        match request.disposition {
            TextureDisposition::Source => {
                required_inputs.push(WaterInputRequirement::AuthoredTexturePlanes(request.role))
            }
            TextureDisposition::BuiltInEnvironment => {
                required_inputs.push(WaterInputRequirement::LocalEnvironment)
            }
            TextureDisposition::BuiltInRenderTarget => match request.role {
                TextureRole::Reflection => {
                    required_inputs.push(WaterInputRequirement::ReflectionFramebuffer)
                }
                TextureRole::Refraction => {
                    required_inputs.push(WaterInputRequirement::RefractionFramebuffer)
                }
                _ => {}
            },
        }
    }
    if textures.base.is_some() {
        required_inputs.push(WaterInputRequirement::Lightmap);
    }
    required_inputs.extend([
        WaterInputRequirement::CameraPosition,
        WaterInputRequirement::WaterSurfacePlane,
        WaterInputRequirement::EyeWaterSide,
        WaterInputRequirement::DistanceToWater,
        WaterInputRequirement::RuntimeWaterPolicy,
    ]);
    if fog_enabled.as_ref().is_some_and(|value| value.value) {
        required_inputs.push(WaterInputRequirement::WaterFogVolume);
    }
    if water_uses_time(material) {
        required_inputs.push(WaterInputRequirement::PresentationTime);
    }
    if material
        .proxy_program
        .entries
        .iter()
        .any(|entry| matches!(entry.operation, Some(ProxyOperation::WaterLod)))
    {
        required_inputs.push(WaterInputRequirement::WaterLodController);
    }
    let opacity = if material.features.translucent {
        WaterSurfaceOpacity::Translucent
    } else {
        WaterSurfaceOpacity::Opaque
    };

    Ok(Some(WaterMaterialOutput {
        shader: if material.selection_environment.hdr_mode == HdrMode::None {
            WaterShaderVariant::Dx90
        } else {
            WaterShaderVariant::Dx9Hdr
        },
        textures,
        bottom_material: material
            .material_requests
            .iter()
            .find(|request| request.role == crate::MaterialRole::Bottom)
            .cloned(),
        underwater_overlay: material
            .material_requests
            .iter()
            .find(|request| request.role == crate::MaterialRole::UnderwaterOverlay)
            .cloned(),
        base_frame,
        normal_frame,
        environment_frame,
        normal_transform,
        scale: effective_vec2(
            parameters,
            b"$scale",
            [1.0, 1.0],
            ParameterOrigin::ShaderInitializer,
        )?,
        time: effective_float(parameters, b"$time", 0.0, ParameterOrigin::TypeInitializer)?,
        water_depth: effective_float(
            parameters,
            b"$waterdepth",
            0.0,
            ParameterOrigin::TypeInitializer,
        )?,
        above_water: effective_bool(
            parameters,
            b"$abovewater",
            true,
            ParameterOrigin::ShaderInitializer,
        ),
        reflect_amount: effective_float(
            parameters,
            b"$reflectamount",
            0.0,
            ParameterOrigin::TypeInitializer,
        )?,
        refract_amount: effective_float(
            parameters,
            b"$refractamount",
            0.0,
            ParameterOrigin::TypeInitializer,
        )?,
        reflect_tint: effective_color(
            parameters,
            b"$reflecttint",
            [1.0; 3],
            ParameterOrigin::TypeInitializer,
        )?,
        refract_tint: effective_color(
            parameters,
            b"$refracttint",
            [1.0; 3],
            ParameterOrigin::TypeInitializer,
        )?,
        reflection_blend_factor: effective_float(
            parameters,
            b"$reflectblendfactor",
            1.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        fog: WaterFogOutput {
            enabled: fog_enabled,
            color: effective_color(
                parameters,
                b"$fogcolor",
                [1.0, 0.0, 0.0],
                ParameterOrigin::ShaderInitializer,
            )?,
            start: effective_float(
                parameters,
                b"$fogstart",
                0.0,
                ParameterOrigin::TypeInitializer,
            )?,
            end: effective_float(
                parameters,
                b"$fogend",
                0.0,
                ParameterOrigin::TypeInitializer,
            )?,
        },
        cheap_start: effective_float(
            parameters,
            b"$cheapwaterstartdistance",
            500.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        cheap_end: effective_float(
            parameters,
            b"$cheapwaterenddistance",
            1000.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        force_cheap,
        force_expensive,
        reflect_entities: effective_bool(
            parameters,
            b"$reflectentities",
            false,
            ParameterOrigin::ShaderInitializer,
        ),
        blur_refraction: effective_bool(
            parameters,
            b"$blurrefract",
            false,
            ParameterOrigin::TypeInitializer,
        ),
        no_low_end_lightmap: effective_bool(
            parameters,
            b"$nolowendlightmap",
            false,
            ParameterOrigin::TypeInitializer,
        ),
        scroll: [
            effective_color(
                parameters,
                b"$scroll1",
                [0.0; 3],
                ParameterOrigin::ShaderInitializer,
            )?,
            effective_color(
                parameters,
                b"$scroll2",
                [0.0; 3],
                ParameterOrigin::ShaderInitializer,
            )?,
        ],
        fresnel: WaterFresnelOutput {
            cheap_enabled: !no_fresnel.value,
            expensive_constant: [1.0, 0.0, 0.0, 0.0],
        },
        opacity,
        required_inputs,
    }))
}

pub fn evaluate_water_material(
    material: &Material,
    context: &ProxyEvaluationContext,
) -> Result<EvaluatedWaterState, WaterEvaluationError> {
    let output = water_material_output(material)
        .map_err(WaterEvaluationError::Material)?
        .ok_or_else(|| WaterEvaluationError::Material(error(ErrorCode::InvalidParameter, None)))?;
    let initial = initial_proxy_variables(material, &output)?;
    let proxy = crate::evaluate_proxy_program(&material.proxy_program, &initial, context)
        .map_err(WaterEvaluationError::Proxy)?;
    let normal_frame = proxy_integer(&proxy, b"$bumpframe")?;
    let normal_transform = proxy_matrix(&proxy, b"$bumptransform")?;
    let cheap_start = proxy_float(&proxy, b"$cheapwaterstartdistance")?;
    let cheap_end = proxy_float(&proxy, b"$cheapwaterenddistance")?;
    Ok(EvaluatedWaterState {
        normal_frame,
        normal_transform,
        cheap_start,
        cheap_end,
        proxy,
    })
}

fn texture(material: &Material, role: TextureRole) -> Option<TextureRequest> {
    material
        .textures
        .iter()
        .find(|texture| texture.role == role)
        .cloned()
}

fn proxy_writes_bump_transform(material: &Material) -> bool {
    material.proxy_program.entries.iter().any(|entry| {
        matches!(
            &entry.operation,
            Some(ProxyOperation::TextureTransform { result, .. }
                | ProxyOperation::TextureScroll { result, .. })
                if result.name == b"$bumptransform"
        )
    })
}

fn water_uses_time(material: &Material) -> bool {
    material
        .water
        .as_ref()
        .is_some_and(|water| water.scroll.iter().flatten().any(|value| *value != 0.0))
        || material.proxy_program.entries.iter().any(|entry| {
            matches!(
                entry.operation,
                Some(
                    ProxyOperation::AnimatedTexture { .. }
                        | ProxyOperation::Sine { .. }
                        | ProxyOperation::TextureScroll { .. }
                )
            )
        })
}

fn initial_proxy_variables(
    material: &Material,
    output: &WaterMaterialOutput,
) -> Result<BTreeMap<Vec<u8>, ProxyValue>, WaterEvaluationError> {
    let mut variables = BTreeMap::new();
    for (name, value) in &material.first_parameters {
        if let Some(value) = proxy_parameter(value) {
            variables.insert(name.clone(), value);
        }
    }
    variables.insert(
        b"$bumpframe".to_vec(),
        ProxyValue::Int(output.normal_frame.value),
    );
    variables.insert(
        b"$bumptransform".to_vec(),
        ProxyValue::Matrix(output.normal_transform.matrix),
    );
    variables.insert(
        b"$cheapwaterstartdistance".to_vec(),
        ProxyValue::Float(output.cheap_start.value),
    );
    variables.insert(
        b"$cheapwaterenddistance".to_vec(),
        ProxyValue::Float(output.cheap_end.value),
    );
    Ok(variables)
}

pub(crate) fn proxy_parameter(value: &[u8]) -> Option<ProxyValue> {
    let text = std::str::from_utf8(value).ok()?.trim();
    if let Some(content) = text
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .or_else(|| {
            text.strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
        })
    {
        let values = content
            .split_ascii_whitespace()
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if !(2..=4).contains(&values.len()) || values.iter().any(|value| !value.is_finite()) {
            return None;
        }
        let mut output = [0.0; 4];
        output[..values.len()].copy_from_slice(&values);
        return Some(ProxyValue::Vector {
            values: output,
            size: values.len() as u8,
        });
    }
    text.parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
        .map(ProxyValue::Float)
}

fn proxy_integer(state: &EvaluatedProxyState, name: &[u8]) -> Result<i32, WaterEvaluationError> {
    match state.variables.get(name) {
        Some(ProxyValue::Int(value)) => Ok(*value),
        Some(ProxyValue::Float(value)) if value.is_finite() => Ok(*value as i32),
        Some(_) => Err(WaterEvaluationError::InvalidVariable(name.to_vec())),
        None => Err(WaterEvaluationError::MissingVariable(name.to_vec())),
    }
}

fn proxy_float(state: &EvaluatedProxyState, name: &[u8]) -> Result<f32, WaterEvaluationError> {
    match state.variables.get(name) {
        Some(ProxyValue::Int(value)) => Ok(*value as f32),
        Some(ProxyValue::Float(value)) if value.is_finite() => Ok(*value),
        Some(_) => Err(WaterEvaluationError::InvalidVariable(name.to_vec())),
        None => Err(WaterEvaluationError::MissingVariable(name.to_vec())),
    }
}

fn proxy_matrix(
    state: &EvaluatedProxyState,
    name: &[u8],
) -> Result<[f32; 16], WaterEvaluationError> {
    match state.variables.get(name) {
        Some(ProxyValue::Matrix(value)) => Ok(*value),
        Some(_) => Err(WaterEvaluationError::InvalidVariable(name.to_vec())),
        None => Err(WaterEvaluationError::MissingVariable(name.to_vec())),
    }
}

fn effective_bool(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: bool,
    default_origin: ParameterOrigin,
) -> EffectiveParameter<bool> {
    match get(parameters, parameter) {
        Some(value) => EffectiveParameter {
            value: crate::source_integer(value) != 0,
            origin: ParameterOrigin::Authored,
        },
        None => EffectiveParameter {
            value: default,
            origin: default_origin,
        },
    }
}

fn effective_integer(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: i32,
    default_origin: ParameterOrigin,
) -> Result<EffectiveParameter<i32>, Error> {
    Ok(EffectiveParameter {
        value: integer_or(parameters, parameter, default)?,
        origin: origin(parameters, parameter, default_origin),
    })
}

fn effective_float(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: f32,
    default_origin: ParameterOrigin,
) -> Result<EffectiveParameter<f32>, Error> {
    Ok(EffectiveParameter {
        value: float_or(parameters, parameter, default)?,
        origin: origin(parameters, parameter, default_origin),
    })
}

fn effective_color(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: [f32; 3],
    default_origin: ParameterOrigin,
) -> Result<EffectiveParameter<[f32; 3]>, Error> {
    Ok(EffectiveParameter {
        value: color_or(parameters, parameter, default)?,
        origin: origin(parameters, parameter, default_origin),
    })
}

fn effective_vec2(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: [f32; 2],
    default_origin: ParameterOrigin,
) -> Result<EffectiveParameter<[f32; 2]>, Error> {
    let Some(value) = get(parameters, parameter) else {
        return Ok(EffectiveParameter {
            value: default,
            origin: default_origin,
        });
    };
    let text = std::str::from_utf8(value)
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?
        .trim();
    let content = text
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(text);
    let values = content
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error(ErrorCode::InvalidParameter, Some(parameter.to_vec())))?;
    if values.len() != 2 || values.iter().any(|value| !value.is_finite()) {
        return Err(error(ErrorCode::InvalidParameter, Some(parameter.to_vec())));
    }
    Ok(EffectiveParameter {
        value: [values[0], values[1]],
        origin: ParameterOrigin::Authored,
    })
}

fn origin(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: ParameterOrigin,
) -> ParameterOrigin {
    if get(parameters, parameter).is_some() {
        ParameterOrigin::Authored
    } else {
        default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SelectionEnvironment, resolve_for_environment};
    use playsrc_keyvalues::ConditionEnvironment;
    use playsrc_vmt::{Composition, Limits, compose};

    fn material(bytes: &[u8], environment: crate::SelectionEnvironment) -> Material {
        let Composition::Complete(document) = compose(
            bytes,
            "materials/water/test.vmt",
            &[],
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("test material requested a dependency")
        };
        resolve_for_environment(&document, environment).unwrap()
    }

    #[test]
    fn water_output_distinguishes_registry_metadata_from_effective_bindings() {
        let selected = material(br#"Water {}"#, SelectionEnvironment::default());
        let declared_state = selected.water.as_ref().unwrap();
        assert!(!declared_state.normal_map.is_defined());
        assert!(!declared_state.environment_map.is_defined());
        assert!(!declared_state.reflection.is_defined());
        assert!(!declared_state.refraction.is_defined());
        let output = water_material_output(&selected).unwrap().unwrap();
        assert_eq!(output.shader, WaterShaderVariant::Dx90);
        assert_eq!(output.textures, WaterTextureBindings::default());
        assert_eq!(output.reflect_amount.value, 0.0);
        assert_eq!(
            output.reflect_amount.origin,
            ParameterOrigin::TypeInitializer
        );
        assert_eq!(output.refract_amount.value, 0.0);
        assert_eq!(output.reflect_tint.value, [1.0; 3]);
        assert_eq!(output.reflect_tint.origin, ParameterOrigin::TypeInitializer);
        assert!(output.above_water.value);
        assert_eq!(
            output.above_water.origin,
            ParameterOrigin::ShaderInitializer
        );
        assert!(!output.force_cheap.value);
        assert_eq!(output.force_cheap.origin, ParameterOrigin::TypeInitializer);
        assert!(output.force_expensive.value);
        assert_eq!(
            output.force_expensive.origin,
            ParameterOrigin::ShaderInitializer
        );
        assert_eq!(output.cheap_start.value, 500.0);
        assert_eq!(output.cheap_end.value, 1000.0);
        assert_eq!(output.scale.value, [1.0, 1.0]);
        assert_eq!(output.normal_transform.matrix, identity_matrix());
        assert_eq!(output.fog.enabled, None);
        assert_eq!(output.fog.color.value, [1.0, 0.0, 0.0]);
        assert_eq!(output.opacity, WaterSurfaceOpacity::Opaque);
        assert!(!output.required_inputs.iter().any(|input| matches!(
            input,
            WaterInputRequirement::LocalEnvironment
                | WaterInputRequirement::ReflectionFramebuffer
                | WaterInputRequirement::RefractionFramebuffer
                | WaterInputRequirement::AuthoredTexturePlanes(_)
        )));

        let hdr = material(
            br#"Water {}"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        );
        assert_eq!(
            water_material_output(&hdr).unwrap().unwrap().shader,
            WaterShaderVariant::Dx9Hdr
        );
    }

    #[test]
    fn configured_water_family_retains_optional_bindings_origins_and_proxy_order() {
        let surface = material(
            br#"Water {
                "$abovewater" "1"
                "$normalmap" "water/tfwater001_normal"
                "$envmap" "maps/jump_beef/c-4787_3137_-2159"
                "$reflecttexture" "_rt_WaterReflection"
                "$refracttexture" "_rt_WaterRefraction"
                "$bottommaterial" "water/water_2fort_beneath.vmt"
                "$reflectamount" ".25"
                "$refractamount" ".32"
                "$bumpframe" "0"
                "$fogenable" "1"
                "$fogcolor" "{51 43 13}"
                "$fogstart" "1"
                "$fogend" "400"
                "$temp" "[0 0]" "$curr" "0.0" "$curr2" "0.0"
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
                    WaterLOD { "dummy" "0" }
                }
            }"#,
            SelectionEnvironment {
                hdr_mode: HdrMode::Integer,
                ..SelectionEnvironment::default()
            },
        );
        let output = water_material_output(&surface).unwrap().unwrap();
        assert_eq!(output.shader, WaterShaderVariant::Dx9Hdr);
        assert!(output.textures.base.is_none());
        assert!(output.textures.flow.is_none());
        assert_eq!(
            output
                .textures
                .normal
                .as_ref()
                .unwrap()
                .logical_path
                .as_deref(),
            Some("materials/water/tfwater001_normal.vtf")
        );
        assert_eq!(
            output
                .textures
                .environment
                .as_ref()
                .unwrap()
                .logical_path
                .as_deref(),
            Some("materials/maps/jump_beef/c-4787_3137_-2159.vtf")
        );
        assert_eq!(
            output.textures.reflection.as_ref().unwrap().disposition,
            TextureDisposition::BuiltInRenderTarget
        );
        assert_eq!(output.reflect_amount.value, 0.25);
        assert_eq!(output.reflect_amount.origin, ParameterOrigin::Authored);
        assert_eq!(output.opacity, WaterSurfaceOpacity::Opaque);
        assert!(output.fog.enabled.as_ref().unwrap().value);
        assert!(output.normal_transform.proxy_mutated);
        assert!(
            output
                .required_inputs
                .contains(&WaterInputRequirement::ReflectionFramebuffer)
        );
        assert!(
            output
                .required_inputs
                .contains(&WaterInputRequirement::RefractionFramebuffer)
        );
        assert!(
            output
                .required_inputs
                .contains(&WaterInputRequirement::PresentationTime)
        );
        assert!(
            output
                .required_inputs
                .contains(&WaterInputRequirement::WaterLodController)
        );
        assert_eq!(surface.proxy_program.entries.len(), 7);

        let context = ProxyEvaluationContext {
            time: 1.0,
            frame_time: 0.05,
            water_lod: Some([1000.0, 2000.0]),
            texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), 60)]),
            ..ProxyEvaluationContext::default()
        };
        let evaluated = evaluate_water_material(&surface, &context).unwrap();
        assert_eq!(evaluated.normal_frame, 30);
        assert_eq!(evaluated.cheap_start, 1000.0);
        assert_eq!(evaluated.cheap_end, 2000.0);
        assert_eq!(evaluated.proxy.trace.len(), 7);
        assert_eq!(evaluated.normal_transform[0], 1.0);
        assert_eq!(evaluated.normal_transform[5], 1.0);
        assert_ne!(evaluated.normal_transform[3], 0.0);
        assert_ne!(evaluated.normal_transform[7], 0.0);

        let beneath = material(
            br#"Water {
                "$abovewater" "0"
                "$normalmap" "water/tfwater001_normal"
                "$refracttexture" "_rt_WaterRefraction"
                "$underwateroverlay" "effects/water_warp_2fort"
                "$refractamount" ".5"
                "$refracttint" "[0.95 1.0 0.97]"
                "$blurrefract" "1"
                "$fogenable" "1" "$fogcolor" "{92 100 80}"
                "$fogstart" "-350" "$fogend" "1050"
                Proxies {
                    AnimatedTexture {
                        "animatedtexturevar" "$normalmap"
                        "animatedtextureframenumvar" "$bumpframe"
                        "animatedtextureframerate" "30"
                    }
                    TextureScroll {
                        "texturescrollvar" "$bumptransform"
                        "texturescrollrate" ".05"
                        "texturescrollangle" "45"
                    }
                    WaterLOD {}
                }
            }"#,
            SelectionEnvironment::default(),
        );
        let output = water_material_output(&beneath).unwrap().unwrap();
        assert!(!output.above_water.value);
        assert!(output.textures.environment.is_none());
        assert!(output.textures.reflection.is_none());
        assert!(output.textures.refraction.is_some());
        assert_eq!(output.opacity, WaterSurfaceOpacity::Opaque);
        assert_eq!(output.refract_tint.value, [0.95, 1.0, 0.97]);
        assert!(output.blur_refraction.value);
        assert_eq!(beneath.proxy_program.entries.len(), 3);
    }

    #[test]
    fn refraction_does_not_change_authored_material_translucency() {
        let refractive = material(
            br#"Water {
                "$normalmap" "water/tfwater001_normal"
                "$refracttexture" "_rt_WaterRefraction"
            }"#,
            SelectionEnvironment::default(),
        );
        let state = water_material_output(&refractive).unwrap().unwrap();
        assert_eq!(state.opacity, WaterSurfaceOpacity::Opaque);
        assert!(state.textures.refraction.is_some());

        let translucent = material(
            br#"Water {
                "$normalmap" "water/tfwater001_normal"
                "$refracttexture" "_rt_WaterRefraction"
                "$translucent" "1"
            }"#,
            SelectionEnvironment::default(),
        );
        assert_eq!(
            water_material_output(&translucent)
                .unwrap()
                .unwrap()
                .opacity,
            WaterSurfaceOpacity::Translucent,
        );
    }

    #[test]
    fn force_cheap_wins_authored_force_conflict() {
        let selected = material(
            br#"Water { "$forcecheap" "1" "$forceexpensive" "1" }"#,
            SelectionEnvironment::default(),
        );
        let output = water_material_output(&selected).unwrap().unwrap();
        assert!(output.force_cheap.value);
        assert!(!output.force_expensive.value);
        assert_eq!(
            output.force_expensive.origin,
            ParameterOrigin::ShaderInitializer
        );
    }
}
