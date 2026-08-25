use crate::{
    Error, ErrorCode, EvaluatedProxyState, Material, ProxyEvaluationContext, ProxyEvaluationError,
    ProxyValue, Shader, TextureColorRead, TextureDisposition, TextureFrameSelection,
    TextureRequest, TextureRole, color_or, error, evaluate_proxy_program, float_or, integer_or,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct RefractMaterialOutput {
    pub normal: TextureRequest,
    pub normal_frame: i32,
    pub normal_transform: [f32; 16],
    pub refract_amount: f32,
    pub refract_tint: [f32; 3],
    pub blur_amount: u8,
    pub ignore_depth: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedRefractState {
    pub normal_frame: i32,
    pub normal_transform: [f32; 16],
    pub proxy: EvaluatedProxyState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RefractEvaluationError {
    Material(Error),
    Proxy(ProxyEvaluationError),
    InvalidVariable(Vec<u8>),
}

pub fn refract_material_output(
    material: &Material,
) -> Result<Option<RefractMaterialOutput>, Error> {
    if material.shader != Shader::Refract {
        return Ok(None);
    }
    let normal = material
        .textures
        .iter()
        .find(|texture| texture.role == TextureRole::Normal)
        .filter(|texture| {
            texture.disposition == TextureDisposition::Source
                && texture.color_read == TextureColorRead::Linear
                && texture.logical_path.is_some()
        })
        .cloned()
        .ok_or_else(|| {
            error(
                ErrorCode::MissingProfileTexture,
                Some(b"$normalmap".to_vec()),
            )
        })?;
    let usage = material
        .texture_uses
        .iter()
        .find(|usage| usage.role == TextureRole::Normal)
        .ok_or_else(|| error(ErrorCode::InvalidParameter, Some(b"$normalmap".to_vec())))?;
    let TextureFrameSelection::Static { initial, .. } = usage.frame else {
        return Err(error(
            ErrorCode::InvalidParameter,
            Some(b"$bumpframe".to_vec()),
        ));
    };
    let transform = usage.transform.as_ref().ok_or_else(|| {
        error(
            ErrorCode::InvalidParameter,
            Some(b"$bumptransform".to_vec()),
        )
    })?;

    Ok(Some(RefractMaterialOutput {
        normal,
        normal_frame: initial,
        normal_transform: transform.matrix,
        refract_amount: float_or(&material.first_parameters, b"$refractamount", 2.0)?,
        refract_tint: color_or(&material.first_parameters, b"$refracttint", [1.0; 3])?,
        blur_amount: integer_or(&material.first_parameters, b"$bluramount", 0)?.clamp(0, 1) as u8,
        ignore_depth: material.features.ignore_z,
    }))
}

pub fn evaluate_refract_material(
    material: &Material,
    context: &ProxyEvaluationContext,
) -> Result<EvaluatedRefractState, RefractEvaluationError> {
    let output = refract_material_output(material)
        .map_err(RefractEvaluationError::Material)?
        .ok_or_else(|| {
            RefractEvaluationError::Material(error(ErrorCode::InvalidParameter, None))
        })?;
    let mut initial = BTreeMap::new();
    for (name, value) in &material.first_parameters {
        if let Some(value) = crate::water::proxy_parameter(value) {
            initial.insert(name.clone(), value);
        }
    }
    initial.insert(b"$bumpframe".to_vec(), ProxyValue::Int(output.normal_frame));
    initial.insert(
        b"$bumptransform".to_vec(),
        ProxyValue::Matrix(output.normal_transform),
    );
    let proxy = evaluate_proxy_program(&material.proxy_program, &initial, context)
        .map_err(RefractEvaluationError::Proxy)?;
    let normal_frame = match proxy.variables.get(b"$bumpframe".as_slice()) {
        Some(ProxyValue::Int(value)) => *value,
        Some(ProxyValue::Float(value)) if value.is_finite() => *value as i32,
        _ => {
            return Err(RefractEvaluationError::InvalidVariable(
                b"$bumpframe".to_vec(),
            ));
        }
    };
    let normal_transform = match proxy.variables.get(b"$bumptransform".as_slice()) {
        Some(ProxyValue::Matrix(value)) => *value,
        _ => {
            return Err(RefractEvaluationError::InvalidVariable(
                b"$bumptransform".to_vec(),
            ));
        }
    };
    Ok(EvaluatedRefractState {
        normal_frame,
        normal_transform,
        proxy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_keyvalues::ConditionEnvironment;
    use playsrc_vmt::{Composition, Limits, compose};

    fn authored_refract() -> Material {
        let source = br#"Refract {
            "$refractamount" ".05"
            "$refracttint" "{185 215 245}"
            "$bluramount" "7"
            "$ignorez" "1"
            "$bumpmap" "dev/water_dudv"
            "$normalmap" "water/tfwater001_normal"
            "$bumpframe" "0"
            "Proxies" {
                "AnimatedTexture" {
                    "animatedtexturevar" "$normalmap"
                    "animatedtextureframenumvar" "$bumpframe"
                    "animatedtextureframerate" "30"
                }
                "TextureScroll" {
                    "texturescrollvar" "$bumptransform"
                    "texturescrollrate" ".1"
                    "texturescrollangle" "45"
                }
            }
        }"#;
        let Composition::Complete(document) = compose(
            source,
            "materials/effects/water_warp_2fort.vmt".to_owned(),
            &[],
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("refract material unexpectedly requested dependencies");
        };
        crate::resolve(&document).unwrap()
    }

    #[test]
    fn authored_refract_uses_normal_map_byte_tint_and_clamped_shader_blur() {
        let material = authored_refract();
        let output = refract_material_output(&material).unwrap().unwrap();
        assert_eq!(
            output.normal.logical_path.as_deref(),
            Some("materials/water/tfwater001_normal.vtf")
        );
        assert_eq!(output.refract_amount, 0.05);
        assert_eq!(
            output.refract_tint,
            [
                185.0 * (1.0 / 255.0),
                215.0 * (1.0 / 255.0),
                245.0 * (1.0 / 255.0)
            ]
        );
        assert_eq!(output.blur_amount, 1);
        assert!(output.ignore_depth);
    }

    #[test]
    fn authored_refract_proxies_evaluate_frame_and_independent_scroll_in_rust() {
        let material = authored_refract();
        let context = ProxyEvaluationContext {
            time: 0.5,
            frame_time: 0.015,
            texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), 60)]),
            ..ProxyEvaluationContext::default()
        };
        let output = evaluate_refract_material(&material, &context).unwrap();
        assert_eq!(output.normal_frame, 15);
        assert_eq!(output.proxy.trace.len(), 2);
        assert!(output.normal_transform[3] > 0.0);
        assert!(output.normal_transform[7] > 0.0);
    }
}
