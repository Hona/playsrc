use crate::{
    EnvironmentMapState, Error, EvaluatedProxyState, Material, ProxyEvaluationContext,
    ProxyEvaluationError, ProxyValue, Shader, TextureFrameSelection, TextureRequest, TextureRole,
    evaluate_proxy_program, float_or,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct WorldTextureBinding {
    pub texture: TextureRequest,
    pub initial_frame: Option<i32>,
    pub frame_parameter: Option<Vec<u8>>,
    pub transform: Option<[f32; 16]>,
    pub transform_parameter: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldMaterialOutput {
    pub shader: Shader,
    pub textures: Vec<WorldTextureBinding>,
    pub environment_map: Option<EnvironmentMapState>,
    pub fresnel_reflection: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedWorldTexture {
    pub role: TextureRole,
    pub frame: Option<i32>,
    pub transform: Option<[f32; 16]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedWorldMaterialState {
    pub textures: Vec<EvaluatedWorldTexture>,
    pub proxy: EvaluatedProxyState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldEvaluationError {
    Material(Error),
    Proxy(ProxyEvaluationError),
    MissingFrameCount(Vec<u8>),
    InvalidFrame {
        parameter: Vec<u8>,
        frame: i32,
        count: u32,
    },
    InvalidVariable(Vec<u8>),
}

pub fn world_material_output(material: &Material) -> Result<Option<WorldMaterialOutput>, Error> {
    if !matches!(
        material.shader,
        Shader::LightmappedGeneric | Shader::WorldVertexTransition
    ) {
        return Ok(None);
    }

    let textures = material
        .textures
        .iter()
        .map(|texture| {
            let usage = material
                .texture_uses
                .iter()
                .find(|usage| usage.role == texture.role)
                .expect("resolved texture owns one usage state");
            let (initial_frame, frame_parameter) = match &usage.frame {
                TextureFrameSelection::Static {
                    initial, parameter, ..
                } => (Some(*initial), Some(parameter.clone())),
                TextureFrameSelection::Unframed => (None, None),
            };
            WorldTextureBinding {
                texture: texture.clone(),
                initial_frame,
                frame_parameter,
                transform: usage.transform.as_ref().map(|transform| transform.matrix),
                transform_parameter: usage
                    .transform
                    .as_ref()
                    .map(|transform| transform.parameter.clone()),
            }
        })
        .collect();

    Ok(Some(WorldMaterialOutput {
        shader: material.shader,
        textures,
        environment_map: material.environment_map.clone(),
        fresnel_reflection: float_or(&material.first_parameters, b"$fresnelreflection", 1.0)?,
    }))
}

pub fn evaluate_world_material(
    material: &Material,
    context: &ProxyEvaluationContext,
) -> Result<EvaluatedWorldMaterialState, WorldEvaluationError> {
    let output = world_material_output(material)
        .map_err(WorldEvaluationError::Material)?
        .ok_or_else(|| WorldEvaluationError::InvalidVariable(material.shader_token.clone()))?;
    let mut initial = BTreeMap::new();
    for (name, value) in &material.first_parameters {
        if let Some(value) = crate::water::proxy_parameter(value) {
            initial.insert(name.clone(), value);
        }
    }
    for texture in &output.textures {
        if let (Some(parameter), Some(frame)) = (&texture.frame_parameter, texture.initial_frame) {
            initial.insert(parameter.clone(), ProxyValue::Int(frame));
        }
        if let (Some(parameter), Some(transform)) =
            (&texture.transform_parameter, texture.transform)
        {
            initial.insert(parameter.clone(), ProxyValue::Matrix(transform));
        }
    }

    let proxy = evaluate_proxy_program(&material.proxy_program, &initial, context)
        .map_err(WorldEvaluationError::Proxy)?;
    let mut textures = Vec::with_capacity(output.textures.len());
    for texture in &output.textures {
        let frame = texture
            .frame_parameter
            .as_ref()
            .map(|parameter| match proxy.variables.get(parameter) {
                Some(ProxyValue::Int(value)) => Ok(*value),
                Some(ProxyValue::Float(value)) if value.is_finite() => Ok(*value as i32),
                _ => Err(WorldEvaluationError::InvalidVariable(parameter.clone())),
            })
            .transpose()?;
        if texture.texture.disposition == crate::TextureDisposition::Source
            && let Some(frame) = frame
        {
            let count = context
                .texture_frames
                .get(&texture.texture.parameter)
                .copied()
                .ok_or_else(|| {
                    WorldEvaluationError::MissingFrameCount(texture.texture.parameter.clone())
                })?;
            if frame < 0 || u32::try_from(frame).is_err() || frame as u32 >= count {
                return Err(WorldEvaluationError::InvalidFrame {
                    parameter: texture.frame_parameter.clone().expect("framed texture"),
                    frame,
                    count,
                });
            }
        }
        let transform = texture
            .transform_parameter
            .as_ref()
            .map(|parameter| match proxy.variables.get(parameter) {
                Some(ProxyValue::Matrix(value)) => Ok(*value),
                _ => Err(WorldEvaluationError::InvalidVariable(parameter.clone())),
            })
            .transpose()?;
        textures.push(EvaluatedWorldTexture {
            role: texture.texture.role,
            frame,
            transform,
        });
    }

    Ok(EvaluatedWorldMaterialState { textures, proxy })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProxyEvaluationErrorCode, TextureDisposition, resolve};
    use playsrc_keyvalues::ConditionEnvironment;
    use playsrc_vmt::{Composition, Limits, compose};

    fn material() -> Material {
        let source = br#"LightmappedGeneric {
            "$basetexture" "water/base"
            "$bumpmap" "water/normal"
            "$normalmap" "water/normal"
            "$envmap" "maps/test/c0_0_0"
            "$envmaptint" "[.2 .3 .4]"
            "$bumpframe" "0"
            "Proxies" {
                "AnimatedTexture" {
                    "animatedtexturevar" "$bumpmap"
                    "animatedtextureframenumvar" "$bumpframe"
                    "animatedtextureframerate" "30"
                }
            }
        }"#;
        let Composition::Complete(document) = compose(
            source,
            "materials/water/test.vmt".to_owned(),
            &[],
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("world material unexpectedly requested composition dependencies");
        };
        resolve(&document).unwrap()
    }

    fn context(time: f32) -> ProxyEvaluationContext {
        ProxyEvaluationContext {
            time,
            frame_time: 0.015,
            texture_frames: BTreeMap::from([
                (b"$basetexture".to_vec(), 1),
                (b"$bumpmap".to_vec(), 30),
                (b"$normalmap".to_vec(), 30),
                (b"$envmap".to_vec(), 1),
            ]),
            ..ProxyEvaluationContext::default()
        }
    }

    #[test]
    fn authored_world_bindings_preserve_shared_proxy_frames_and_environment_constants() {
        let material = material();
        let output = world_material_output(&material).unwrap().unwrap();
        assert_eq!(output.fresnel_reflection, 1.0);
        assert_eq!(output.environment_map.unwrap().tint, [0.2, 0.3, 0.4]);
        assert_eq!(output.textures.len(), 4);
        assert!(output.textures.iter().all(|texture| {
            texture.texture.disposition == TextureDisposition::Source
                && texture.initial_frame == Some(0)
        }));
        for (time, frame) in [(0.0, 0), (0.5, 15), (1.0, 0), (1.5, 15)] {
            let evaluated = evaluate_world_material(&material, &context(time)).unwrap();
            for role in [TextureRole::Bump, TextureRole::Normal] {
                assert_eq!(
                    evaluated
                        .textures
                        .iter()
                        .find(|texture| texture.role == role)
                        .unwrap()
                        .frame,
                    Some(frame),
                );
            }
            assert_eq!(evaluated.proxy.trace.len(), 1);
        }
    }

    #[test]
    fn world_proxy_refuses_missing_counts_nonfinite_time_and_out_of_range_static_frames() {
        let material = material();
        let mut missing = context(0.5);
        missing.texture_frames.remove(b"$bumpmap".as_slice());
        assert!(matches!(
            evaluate_world_material(&material, &missing),
            Err(WorldEvaluationError::Proxy(error))
                if error.code == ProxyEvaluationErrorCode::MissingVariable
        ));

        let mut invalid = context(0.5);
        invalid.texture_frames.insert(b"$normalmap".to_vec(), 15);
        assert!(matches!(
            evaluate_world_material(&material, &invalid),
            Err(WorldEvaluationError::InvalidFrame {
                frame: 15,
                count: 15,
                ..
            })
        ));
        assert!(matches!(
            evaluate_world_material(&material, &context(f32::NAN)),
            Err(WorldEvaluationError::Proxy(error))
                if error.code == ProxyEvaluationErrorCode::NonFinite
        ));
    }
}
