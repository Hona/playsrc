use crate::{
    Document, Float32, Matrix3x4, PresentationError, PresentationErrorCode, SampledWorldPose,
    Vector3,
    presentation::{matrix_translation, transform_point, values3},
};

pub const MAX_MODEL_LOCAL_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelLocalLightType {
    Point,
    Directional,
    Spot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelLocalLight {
    pub kind: ModelLocalLightType,
    pub color: Vector3,
    pub position: Vector3,
    pub direction: Vector3,
    pub range: Float32,
    pub falloff: Float32,
    pub attenuation: [Float32; 3],
    pub theta: Float32,
    pub phi: Float32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelLightingInput {
    pub lighting_origin: Vector3,
    pub ambient_cube: [Vector3; 6],
    pub local_lights: Vec<ModelLocalLight>,
    pub camera_position: Vector3,
    pub local_environment: Option<String>,
    pub ambient_light: bool,
    pub static_light_vertex: bool,
    pub static_light_texel: bool,
}

impl ModelLightingInput {
    pub fn validate(&self) -> Result<(), PresentationError> {
        if self.local_lights.len() > MAX_MODEL_LOCAL_LIGHTS
            || !finite_vector(self.lighting_origin)
            || !finite_vector(self.camera_position)
            || self.ambient_cube.iter().any(|value| !finite_vector(*value))
            || self.local_lights.iter().any(|light| {
                !finite_vector(light.color)
                    || !finite_vector(light.position)
                    || !finite_vector(light.direction)
                    || !finite(light.range)
                    || !finite(light.falloff)
                    || light.attenuation.iter().any(|value| !finite(*value))
                    || !finite(light.theta)
                    || !finite(light.phi)
            })
            || self.local_environment.as_deref().is_some_and(|identity| {
                identity.is_empty()
                    || !identity.starts_with("materials/")
                    || identity.bytes().any(|byte| byte.is_ascii_uppercase())
                    || identity.contains('\\')
                    || identity
                        .split('/')
                        .any(|component| component.is_empty() || matches!(component, "." | ".."))
            })
        {
            return Err(PresentationError {
                code: PresentationErrorCode::InvalidState,
                identity: "model-lighting".to_owned(),
            });
        }
        Ok(())
    }
}

pub fn ambient_cube_directions() -> [Vector3; 6] {
    [
        vector([1.0, 0.0, 0.0]),
        vector([-1.0, 0.0, 0.0]),
        vector([0.0, 1.0, 0.0]),
        vector([0.0, -1.0, 0.0]),
        vector([0.0, 0.0, 1.0]),
        vector([0.0, 0.0, -1.0]),
    ]
}

pub fn model_lighting_origin(
    document: &Document,
    model_to_world: Matrix3x4,
    world_pose: &SampledWorldPose,
) -> Result<Vector3, PresentationError> {
    source_model_lighting_origin(
        document.bounds.illumination,
        document.illumination_attachment,
        model_to_world,
        Some(world_pose),
        &document.identity,
    )
}

pub fn source_model_lighting_origin(
    illumination: Vector3,
    illumination_attachment: i32,
    model_to_world: Matrix3x4,
    world_pose: Option<&SampledWorldPose>,
    identity: &str,
) -> Result<Vector3, PresentationError> {
    let matrix = if illumination_attachment == 0 {
        model_to_world
    } else {
        let index = usize::try_from(illumination_attachment - 1)
            .map_err(|_| invalid_lighting_origin(identity))?;
        world_pose
            .ok_or_else(|| invalid_lighting_origin(identity))?
            .attachments
            .get(index)
            .filter(|attachment| attachment.index == index)
            .map(|attachment| attachment.model_transform)
            .ok_or_else(|| invalid_lighting_origin(identity))?
    };
    let transformed = transform_point(&matrix, values3(illumination));
    if transformed.iter().any(|value| !value.is_finite()) {
        return Err(invalid_lighting_origin(identity));
    }
    Ok(vector(transformed))
}

pub fn attachment_world_position(pose: &SampledWorldPose, attachment: usize) -> Option<Vector3> {
    pose.attachments
        .get(attachment)
        .filter(|value| value.index == attachment)
        .map(|value| matrix_translation(&value.model_transform))
}

pub fn transform_model_render_bounds(bounds: [Vector3; 2], transform: Matrix3x4) -> Result<[Vector3; 2], PresentationError> {
    let [mins, maxs] = bounds.map(values3);
    if (0..3).any(|axis| !mins[axis].is_finite() || !maxs[axis].is_finite() || mins[axis] > maxs[axis]) {
        return Err(invalid_lighting_origin("model-render-bounds"));
    }
    let center = std::array::from_fn(|axis| (mins[axis] + maxs[axis]) * 0.5);
    let extent: [f32; 3] = std::array::from_fn(|axis| maxs[axis] - center[axis]);
    let world_center = transform_point(&transform, center);
    let world_extent: [f32; 3] = std::array::from_fn(|row| {
        f32::from_bits(transform.0[row * 4].0).abs() * extent[0]
            + f32::from_bits(transform.0[row * 4 + 1].0).abs() * extent[1]
            + f32::from_bits(transform.0[row * 4 + 2].0).abs() * extent[2]
    });
    let result = [
        vector(std::array::from_fn(|axis| world_center[axis] - world_extent[axis])),
        vector(std::array::from_fn(|axis| world_center[axis] + world_extent[axis])),
    ];
    if result.into_iter().any(|value| !finite_vector(value)) { return Err(invalid_lighting_origin("model-render-bounds")); }
    Ok(result)
}

fn invalid_lighting_origin(identity: &str) -> PresentationError {
    PresentationError {
        code: PresentationErrorCode::InvalidState,
        identity: identity.to_owned(),
    }
}

fn finite(value: Float32) -> bool {
    f32::from_bits(value.0).is_finite()
}

fn finite_vector(value: Vector3) -> bool {
    value.0.into_iter().all(finite)
}

fn vector(values: [f32; 3]) -> Vector3 {
    Vector3(values.map(|value| Float32(value.to_bits())))
}

#[cfg(test)]
mod tests {
    #[test]
    fn static_prop_boxes_use_transformed_render_bounds() {
        let bounds = crate::Bounds {
            eye: super::vector([0.0; 3]), illumination: super::vector([0.0; 3]),
            hull_min: super::vector([-8.0; 3]), hull_max: super::vector([8.0; 3]),
            view_min: super::vector([-2.0, -1.0, -3.0]), view_max: super::vector([2.0, 1.0, 3.0]),
        };
        let transform = crate::source_entity_transform(super::vector([10.0, 20.0, 30.0]), super::vector([0.0, 90.0, 0.0])).unwrap();
        let result = super::transform_model_render_bounds(bounds.render_bounds(), transform).unwrap();
        assert_eq!(result.map(|value| value.0.map(|value| f32::from_bits(value.0))), [[9.0, 18.0, 27.0], [11.0, 22.0, 33.0]]);
        let unset = crate::Bounds { view_min: super::vector([0.0; 3]), view_max: super::vector([-0.0; 3]), ..bounds };
        assert_eq!(unset.render_bounds(), [unset.hull_min, unset.hull_max]);
    }

    use super::*;

    #[test]
    fn ambient_cube_and_local_light_contract_is_ordered_and_bounded() {
        assert_eq!(
            ambient_cube_directions().map(values3),
            [
                [1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ]
        );
        let light = ModelLocalLight {
            kind: ModelLocalLightType::Point,
            color: vector([1.0; 3]),
            position: vector([2.0, 3.0, 4.0]),
            direction: vector([0.0, 0.0, -1.0]),
            range: Float32(0.0_f32.to_bits()),
            falloff: Float32(0.0_f32.to_bits()),
            attenuation: [
                Float32(1.0_f32.to_bits()),
                Float32(0.0_f32.to_bits()),
                Float32(0.0_f32.to_bits()),
            ],
            theta: Float32(0.0_f32.to_bits()),
            phi: Float32(0.0_f32.to_bits()),
        };
        let mut input = ModelLightingInput {
            lighting_origin: vector([0.0; 3]),
            ambient_cube: [vector([0.25; 3]); 6],
            local_lights: vec![light; 4],
            camera_position: vector([8.0, 0.0, 0.0]),
            local_environment: Some("materials/maps/test/c0_0_0.vtf".to_owned()),
            ambient_light: true,
            static_light_vertex: false,
            static_light_texel: false,
        };
        input.validate().unwrap();
        input.local_lights.push(light);
        assert_eq!(
            input.validate().unwrap_err().code,
            PresentationErrorCode::InvalidState
        );
    }

    #[test]
    fn illumination_uses_the_model_root_or_one_based_attachment() {
        let matrix = |translation: [f32; 3]| {
            Matrix3x4(
                [
                    1.0,
                    0.0,
                    0.0,
                    translation[0],
                    0.0,
                    1.0,
                    0.0,
                    translation[1],
                    0.0,
                    0.0,
                    1.0,
                    translation[2],
                ]
                .map(|value| Float32(value.to_bits())),
            )
        };
        let pose = SampledWorldPose {
            bone_matrices: Vec::new(),
            skinning_matrices: Vec::new(),
            attachments: vec![crate::SampledAttachment {
                index: 0,
                name: b"illumination".to_vec(),
                world_aligned: false,
                model_transform: matrix([10.0, 20.0, 30.0]),
            }],
        };
        let illumination = vector([1.0, 2.0, 3.0]);
        assert_eq!(
            source_model_lighting_origin(illumination, 0, matrix([4.0, 5.0, 6.0]), None, "model",)
                .map(values3)
                .unwrap(),
            [5.0, 7.0, 9.0]
        );
        assert_eq!(
            source_model_lighting_origin(illumination, 1, matrix([0.0; 3]), Some(&pose), "model")
                .map(values3)
                .unwrap(),
            [11.0, 22.0, 33.0]
        );
    }
}
