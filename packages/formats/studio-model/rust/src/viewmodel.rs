use crate::{
    AnimationLayer, AnimationState, Float32, Matrix3x4, PresentationDescriptor, PresentationError,
    PresentationErrorCode, PresentationModel, SampledAttachment, SampledPose, SelectedPrimitive,
    presentation::{identity_quaternion, matrix_translation, multiply_matrix, quaternion_matrix},
    sample_pose, select_primitives,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewModelCompositionRequest {
    pub translated_activity: Vec<u8>,
    pub hand_sequence: usize,
    pub cycle: Float32,
    pub time: Float32,
    pub hand_pose_parameters: Vec<Float32>,
    pub hand_layers: Vec<AnimationLayer>,
    pub skin: usize,
    pub hand_bodygroups: Vec<usize>,
    pub item_bodygroups: Vec<usize>,
    pub lod: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposedViewModelPart {
    pub identity: String,
    pub pose: SampledPose,
    pub primitives: Vec<SelectedPrimitive>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewModelComposition {
    pub translated_activity: Vec<u8>,
    pub skin: usize,
    pub hand: ComposedViewModelPart,
    pub item: ComposedViewModelPart,
    pub item_to_hand_bones: Vec<Option<usize>>,
}

pub fn compose_viewmodel(
    hand: &PresentationModel,
    item: &PresentationModel,
    request: &ViewModelCompositionRequest,
) -> Result<ViewModelComposition, PresentationError> {
    if !matches!(hand.descriptor, PresentationDescriptor::ViewModel { .. })
        || !matches!(item.descriptor, PresentationDescriptor::ViewModel { .. })
        || !f32::from_bits(request.time.0).is_finite()
    {
        return Err(invalid_composition(&hand.identity));
    }
    let sequence = hand
        .sequences
        .get(request.hand_sequence)
        .filter(|sequence| {
            sequence
                .activity_name
                .eq_ignore_ascii_case(&request.translated_activity)
        })
        .ok_or_else(|| invalid_composition(&hand.identity))?;
    if sequence.index != request.hand_sequence {
        return Err(invalid_composition(&hand.identity));
    }
    let hand_pose = crate::sample_pose_at_time(
        hand,
        &AnimationState {
            base_sequence: request.hand_sequence,
            cycle: request.cycle,
            pose_parameters: request.hand_pose_parameters.clone(),
            layers: request.hand_layers.clone(),
        },
        request.time,
    )?;
    let item_pose = sample_pose(
        item,
        &AnimationState {
            base_sequence: 0,
            cycle: Float32(0.0_f32.to_bits()),
            pose_parameters: item
                .pose_parameters
                .iter()
                .map(|_| Float32(0.0_f32.to_bits()))
                .collect(),
            layers: Vec::new(),
        },
    )?;
    let item_to_hand_bones = item
        .bones
        .iter()
        .map(|item_bone| {
            hand.bones
                .iter()
                .position(|hand_bone| hand_bone.name.eq_ignore_ascii_case(&item_bone.name))
        })
        .collect::<Vec<_>>();
    if !item_to_hand_bones.iter().any(Option::is_some) {
        return Err(invalid_composition(&item.identity));
    }
    let item_pose = merged_item_pose(item, &item_pose, hand, &hand_pose, &item_to_hand_bones)?;
    let hand_primitives =
        select_primitives(hand, &request.hand_bodygroups, request.skin, request.lod)?;
    let item_primitives =
        select_primitives(item, &request.item_bodygroups, request.skin, request.lod)?;
    Ok(ViewModelComposition {
        translated_activity: request.translated_activity.clone(),
        skin: request.skin,
        hand: ComposedViewModelPart {
            identity: hand.identity.clone(),
            pose: hand_pose,
            primitives: hand_primitives,
        },
        item: ComposedViewModelPart {
            identity: item.identity.clone(),
            pose: item_pose,
            primitives: item_primitives,
        },
        item_to_hand_bones,
    })
}

fn merged_item_pose(
    item: &PresentationModel,
    sampled_item: &SampledPose,
    hand: &PresentationModel,
    sampled_hand: &SampledPose,
    bone_map: &[Option<usize>],
) -> Result<SampledPose, PresentationError> {
    if sampled_item.local_translations.len() != item.bones.len()
        || sampled_item.local_rotations.len() != item.bones.len()
        || sampled_hand.model_matrices.len() != hand.bones.len()
        || bone_map.len() != item.bones.len()
    {
        return Err(invalid_composition(&item.identity));
    }
    let mut model_matrices = Vec::with_capacity(item.bones.len());
    let mut skinning_matrices = Vec::with_capacity(item.bones.len());
    for (index, bone) in item.bones.iter().enumerate() {
        let matrix = if let Some(hand_bone) = bone_map[index] {
            *sampled_hand
                .model_matrices
                .get(hand_bone)
                .ok_or_else(|| invalid_composition(&item.identity))?
        } else {
            let local = quaternion_matrix(
                sampled_item.local_rotations[index],
                sampled_item.local_translations[index],
            );
            if bone.parent == -1 {
                local
            } else {
                let parent = usize::try_from(bone.parent)
                    .ok()
                    .and_then(|parent| model_matrices.get(parent))
                    .ok_or_else(|| invalid_composition(&item.identity))?;
                multiply_matrix(parent, &local)
            }
        };
        skinning_matrices.push(multiply_matrix(&matrix, &Matrix3x4(bone.pose_to_bone)));
        model_matrices.push(matrix);
    }
    let attachments = item
        .attachments
        .iter()
        .map(|attachment| {
            let bone = usize::try_from(attachment.bone)
                .ok()
                .and_then(|bone| model_matrices.get(bone))
                .ok_or_else(|| invalid_composition(&item.identity))?;
            let local = Matrix3x4(attachment.local);
            let model_transform = multiply_matrix(bone, &local);
            Ok(SampledAttachment {
                index: attachment.index,
                name: attachment.name.clone(),
                world_aligned: attachment.flags & 0x0001_0000 != 0,
                model_transform: if attachment.flags & 0x0001_0000 != 0 {
                    quaternion_matrix(identity_quaternion(), matrix_translation(&model_transform))
                } else {
                    model_transform
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SampledPose {
        local_translations: sampled_item.local_translations.clone(),
        local_rotations: sampled_item.local_rotations.clone(),
        model_matrices,
        skinning_matrices,
        attachments,
    })
}

fn invalid_composition(identity: &str) -> PresentationError {
    PresentationError {
        code: PresentationErrorCode::InvalidState,
        identity: identity.to_owned(),
    }
}
