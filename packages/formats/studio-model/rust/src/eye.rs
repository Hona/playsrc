use crate::{
    Document, Float32, Matrix3x4, PresentationError, PresentationErrorCode, Vector3,
    presentation::{transform_point, values3},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EyeConfiguration {
    pub move_eyes: bool,
    pub shift: Vector3,
    pub size: Float32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EyeDrawRequest<'a> {
    pub body_part: usize,
    pub submodel: usize,
    pub bone_to_world: &'a [Matrix3x4],
    pub world_target: Vector3,
    pub view_right: Vector3,
    pub view_up: Vector3,
    pub configuration: EyeConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EyeDrawState {
    pub mesh: usize,
    pub eyeball: usize,
    pub texture: usize,
    pub world_origin: Vector3,
    pub authored_up: Vector3,
    pub iris_u: [Float32; 4],
    pub iris_v: [Float32; 4],
    pub glint_u: [Float32; 4],
    pub glint_v: [Float32; 4],
}

pub fn eye_draw_states(
    document: &Document,
    request: &EyeDrawRequest<'_>,
) -> Result<Vec<EyeDrawState>, PresentationError> {
    let submodel = document
        .body_parts
        .get(request.body_part)
        .and_then(|part| part.models.get(request.submodel))
        .ok_or_else(|| invalid_eye(&document.identity))?;
    if request.bone_to_world.len() != document.bones.len()
        || !finite_vector(request.world_target)
        || !finite_vector(request.view_right)
        || !finite_vector(request.view_up)
        || !finite_vector(request.configuration.shift)
        || !finite(request.configuration.size)
    {
        return Err(invalid_eye(&document.identity));
    }
    let view_right =
        normalized(values3(request.view_right)).ok_or_else(|| invalid_eye(&document.identity))?;
    let view_up =
        normalized(values3(request.view_up)).ok_or_else(|| invalid_eye(&document.identity))?;
    let mut output = Vec::new();
    for mesh in &submodel.meshes {
        if mesh.material_type != 1 {
            continue;
        }
        let eye_index = usize::try_from(mesh.material_parameter)
            .ok()
            .filter(|index| *index < submodel.eyeballs.len())
            .ok_or_else(|| invalid_eye(&document.identity))?;
        let eye = &submodel.eyeballs[eye_index];
        let bone = request
            .bone_to_world
            .get(eye.bone as usize)
            .ok_or_else(|| invalid_eye(&document.identity))?;
        let mut local_origin = values3(eye.origin);
        for (axis, value) in local_origin.iter_mut().enumerate() {
            *value += f32::from_bits(request.configuration.shift.0[axis].0) * sign(*value);
        }
        let origin = transform_point(bone, local_origin);
        let mut up = rotate_vector(bone, values3(eye.up));
        let mut forward = if request.configuration.move_eyes {
            subtract(values3(request.world_target), origin)
        } else {
            scale(rotate_vector(bone, values3(eye.forward)), -1.0)
        };
        forward = normalized(forward).ok_or_else(|| invalid_eye(&document.identity))?;
        up = normalized(up).ok_or_else(|| invalid_eye(&document.identity))?;
        let mut right =
            normalized(cross(forward, up)).ok_or_else(|| invalid_eye(&document.identity))?;
        forward = normalized(add(
            forward,
            scale(right, f32::from_bits(eye.z_offset.0) * 2.0),
        ))
        .ok_or_else(|| invalid_eye(&document.identity))?;
        right = normalized(cross(forward, up)).ok_or_else(|| invalid_eye(&document.identity))?;
        up = normalized(cross(right, forward)).ok_or_else(|| invalid_eye(&document.identity))?;

        let mut iris_scale =
            1.0 / f32::from_bits(eye.iris_scale.0) + f32::from_bits(request.configuration.size.0);
        if iris_scale > 0.0 {
            iris_scale = 1.0 / iris_scale;
        }
        if !iris_scale.is_finite() {
            return Err(invalid_eye(&document.identity));
        }
        let iris_u = projection_row(scale(right, -iris_scale), origin);
        let iris_v = projection_row(scale(up, -iris_scale), origin);
        let glint_scale = 1.0 / (f32::from_bits(eye.radius.0) * 2.0);
        let glint_u = projection_row(scale(view_right, glint_scale), origin);
        let glint_v = projection_row(scale(view_up, glint_scale), origin);
        output.push(EyeDrawState {
            mesh: mesh.index,
            eyeball: eye.index,
            texture: eye.texture as usize,
            world_origin: vector(origin),
            authored_up: eye.up,
            iris_u,
            iris_v,
            glint_u,
            glint_v,
        });
    }
    Ok(output)
}

fn projection_row(direction: [f32; 3], origin: [f32; 3]) -> [Float32; 4] {
    [
        Float32(direction[0].to_bits()),
        Float32(direction[1].to_bits()),
        Float32(direction[2].to_bits()),
        Float32((0.5 - dot(direction, origin)).to_bits()),
    ]
}

fn rotate_vector(matrix: &Matrix3x4, vector: [f32; 3]) -> [f32; 3] {
    let value = matrix.0.map(|value| f32::from_bits(value.0));
    [
        value[0] * vector[0] + value[1] * vector[1] + value[2] * vector[2],
        value[4] * vector[0] + value[5] * vector[1] + value[6] * vector[2],
        value[8] * vector[0] + value[9] * vector[1] + value[10] * vector[2],
    ]
}

fn normalized(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(value, value).sqrt();
    (length.is_finite() && length > 0.0).then(|| scale(value, 1.0 / length))
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f32; 3], scale: f32) -> [f32; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn sign(value: f32) -> f32 {
    if value < 0.0 {
        -1.0
    } else if value > 0.0 {
        1.0
    } else {
        0.0
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

fn invalid_eye(identity: &str) -> PresentationError {
    PresentationError {
        code: PresentationErrorCode::InvalidState,
        identity: identity.to_owned(),
    }
}
