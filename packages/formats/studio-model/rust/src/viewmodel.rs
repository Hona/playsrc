use crate::{
    AnimationLayer, AnimationState, Float32, GeometryFacing, GeometryOrientation, Matrix3x4,
    PresentationDescriptor, PresentationError, PresentationErrorCode, PresentationModel,
    SampledAttachment, SampledPose, SelectedPrimitive, SequenceEvent, SequenceTiming,
    TransformOrientation, Vector3, combine_transform_orientations,
    presentation::{identity_quaternion, matrix_translation, multiply_matrix, quaternion_matrix},
    sample_pose, select_primitives, sequence_events_between, sequence_timing,
    transformed_geometry_facing,
};

const STUDIO_HEADER_FORCE_OPAQUE: i32 = 0x0000_0004;

/// Model panels deliver all authored sequence events, with end-of-loop events
/// before the following loop's zero-frame events (CMDLPanel::DoAnimationEvents).
pub fn model_panel_events(events: &[SequenceEvent], previous: f32, current: f32) -> Vec<&SequenceEvent> {
    if previous == current { return Vec::new(); }
    let mut output = Vec::new();
    let start = if current < previous {
        output.extend(events.iter().filter(|event| f32::from_bits(event.cycle.0) > previous));
        -0.01
    } else { previous };
    output.extend(events.iter().filter(|event| { let cycle = f32::from_bits(event.cycle.0); cycle > start && cycle <= current }));
    output
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewModelMaterialOpacity {
    Opaque,
    Translucent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewModelPart {
    Hand,
    Item,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewModelPartDrawPlan {
    pub part: ViewModelPart,
    pub identity: String,
    pub opaque_primitives: Vec<SelectedPrimitive>,
    pub translucent_primitives: Vec<SelectedPrimitive>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewModelDrawPlan {
    pub item_entity_translucent: bool,
    pub parts: Vec<ViewModelPartDrawPlan>,
}

pub fn viewmodel_draw_plan(
    hand: &PresentationModel,
    item: &PresentationModel,
    composition: &ViewModelComposition,
    hand_material_opacity: &[ViewModelMaterialOpacity],
    item_material_opacity: &[ViewModelMaterialOpacity],
) -> Result<ViewModelDrawPlan, PresentationError> {
    if composition.hand.identity != hand.identity
        || composition.item.identity != item.identity
        || hand_material_opacity.len() != hand.materials.len()
        || item_material_opacity.len() != item.materials.len()
    {
        return Err(invalid_composition(&hand.identity));
    }
    let hand_plan = partition_part(
        ViewModelPart::Hand,
        &composition.hand,
        hand_material_opacity,
    )?;
    let item_plan = partition_part(
        ViewModelPart::Item,
        &composition.item,
        item_material_opacity,
    )?;
    let item_entity_translucent = item.flags & STUDIO_HEADER_FORCE_OPAQUE == 0
        && item_material_opacity.contains(&ViewModelMaterialOpacity::Translucent);
    let parts = if item_entity_translucent {
        vec![hand_plan, item_plan]
    } else {
        vec![item_plan, hand_plan]
    };
    Ok(ViewModelDrawPlan {
        item_entity_translucent,
        parts,
    })
}

fn partition_part(
    part: ViewModelPart,
    composition: &ComposedViewModelPart,
    material_opacity: &[ViewModelMaterialOpacity],
) -> Result<ViewModelPartDrawPlan, PresentationError> {
    let mut opaque_primitives = Vec::new();
    let mut translucent_primitives = Vec::new();
    for primitive in &composition.primitives {
        match material_opacity
            .get(primitive.material)
            .ok_or_else(|| invalid_composition(&composition.identity))?
        {
            ViewModelMaterialOpacity::Opaque => opaque_primitives.push(*primitive),
            ViewModelMaterialOpacity::Translucent => translucent_primitives.push(*primitive),
        }
    }
    Ok(ViewModelPartDrawPlan {
        part,
        identity: composition.identity.clone(),
        opaque_primitives,
        translucent_primitives,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewModelDrawSuppression {
    ClientMode,
    RenderRequest,
    RenderViewModels,
    LocalPlayerVisible,
    EntitiesDisabled,
    NonPlayerViewEntity,
    BaseShouldDraw,
    FullyLowered,
    ObserverOwnerMismatch,
    OwnerDead,
    NotReady,
    ZeroBlend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewModelDrawDisposition {
    Draw,
    SuppressedSuccess(ViewModelDrawSuppression),
    Suppressed(ViewModelDrawSuppression),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelDrawEligibility {
    pub client_mode: bool,
    pub render_request: bool,
    pub render_viewmodels: bool,
    pub local_player_visible: bool,
    pub draw_entities: bool,
    pub player_view_entity: bool,
    pub base_should_draw: bool,
    pub fully_lowered: bool,
    pub observer_owner_matches: bool,
    pub owner_alive: bool,
    pub ready: bool,
    pub fx_blend: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewModelPhase {
    Draw,
    PrimaryFire,
    ReloadStart,
    ReloadInsertOrLoop,
    ReloadFinish,
    Idle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewModelFrameRequest {
    pub phase: ViewModelPhase,
    pub previous_cycle: Float32,
    pub composition: ViewModelCompositionRequest,
    pub hand_material_opacity: Vec<ViewModelMaterialOpacity>,
    pub item_material_opacity: Vec<ViewModelMaterialOpacity>,
    pub draw_eligibility: ViewModelDrawEligibility,
    pub occurrence_orientation: TransformOrientation,
    pub reflected_viewmodel: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducedViewModelFrame {
    pub phase: ViewModelPhase,
    pub timing: SequenceTiming,
    pub crossed_events: Vec<SequenceEvent>,
    pub item_bodygroup_mutations: Vec<ViewModelBodygroupMutation>,
    pub hand_bodygroups: Vec<usize>,
    pub item_bodygroups: Vec<usize>,
    pub composition: ViewModelComposition,
    pub draw_disposition: ViewModelDrawDisposition,
    pub draw_plan: ViewModelDrawPlan,
    pub hand_facing: GeometryFacing,
    pub item_facing: GeometryFacing,
}

pub fn viewmodel_draw_disposition(state: ViewModelDrawEligibility) -> ViewModelDrawDisposition {
    let suppressed = if !state.client_mode {
        Some(ViewModelDrawSuppression::ClientMode)
    } else if !state.render_request {
        Some(ViewModelDrawSuppression::RenderRequest)
    } else if !state.render_viewmodels {
        Some(ViewModelDrawSuppression::RenderViewModels)
    } else if state.local_player_visible {
        Some(ViewModelDrawSuppression::LocalPlayerVisible)
    } else if !state.draw_entities {
        Some(ViewModelDrawSuppression::EntitiesDisabled)
    } else if !state.player_view_entity {
        Some(ViewModelDrawSuppression::NonPlayerViewEntity)
    } else if !state.base_should_draw {
        Some(ViewModelDrawSuppression::BaseShouldDraw)
    } else {
        None
    };
    if let Some(reason) = suppressed {
        return ViewModelDrawDisposition::Suppressed(reason);
    }
    if state.fully_lowered {
        return ViewModelDrawDisposition::SuppressedSuccess(ViewModelDrawSuppression::FullyLowered);
    }
    let suppressed = if !state.observer_owner_matches {
        Some(ViewModelDrawSuppression::ObserverOwnerMismatch)
    } else if !state.owner_alive {
        Some(ViewModelDrawSuppression::OwnerDead)
    } else if !state.ready {
        Some(ViewModelDrawSuppression::NotReady)
    } else if state.fx_blend == 0 {
        Some(ViewModelDrawSuppression::ZeroBlend)
    } else {
        None
    };
    suppressed.map_or(ViewModelDrawDisposition::Draw, |reason| {
        ViewModelDrawDisposition::Suppressed(reason)
    })
}

pub fn produce_viewmodel_frame(
    hand: &PresentationModel,
    item: &PresentationModel,
    request: &ViewModelFrameRequest,
) -> Result<ProducedViewModelFrame, PresentationError> {
    let hand_geometry = viewmodel_geometry(hand)?;
    let item_geometry = viewmodel_geometry(item)?;
    let crossed_events = sequence_events_between(
        hand,
        request.composition.hand_sequence,
        request.previous_cycle,
        request.composition.cycle,
    )?
    .into_iter()
    .cloned()
    .collect::<Vec<_>>();
    let item_bodygroup_mutations = viewmodel_item_bodygroup_events(
        hand,
        item,
        request.composition.hand_sequence,
        request.previous_cycle,
        request.composition.cycle,
    )?;
    let mut composition_request = request.composition.clone();
    apply_viewmodel_bodygroup_events(
        item,
        &mut composition_request.item_bodygroups,
        &item_bodygroup_mutations,
    )?;
    let hand_bodygroups = composition_request.hand_bodygroups.clone();
    let item_bodygroups = composition_request.item_bodygroups.clone();
    let timing = sequence_timing(
        hand,
        composition_request.hand_sequence,
        &composition_request.hand_pose_parameters,
    )?;
    let composition = compose_viewmodel(hand, item, &composition_request)?;
    let draw_plan = viewmodel_draw_plan(
        hand,
        item,
        &composition,
        &request.hand_material_opacity,
        &request.item_material_opacity,
    )?;
    let orientation = combine_transform_orientations([
        request.occurrence_orientation,
        if request.reflected_viewmodel {
            TransformOrientation::Reversing
        } else {
            TransformOrientation::Preserving
        },
    ]);
    Ok(ProducedViewModelFrame {
        phase: request.phase,
        timing,
        crossed_events,
        item_bodygroup_mutations,
        hand_bodygroups,
        item_bodygroups,
        composition,
        draw_disposition: viewmodel_draw_disposition(request.draw_eligibility),
        draw_plan,
        hand_facing: transformed_geometry_facing(hand_geometry.facing, orientation),
        item_facing: transformed_geometry_facing(item_geometry.facing, orientation),
    })
}

fn viewmodel_geometry(model: &PresentationModel) -> Result<GeometryOrientation, PresentationError> {
    match model.descriptor {
        PresentationDescriptor::ViewModel { geometry, .. } => Ok(geometry),
        PresentationDescriptor::World { .. } => Err(invalid_composition(&model.identity)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelProjectionRequest {
    pub configured_horizontal_fov_4_by_3: Float32,
    pub default_world_fov: Float32,
    pub current_world_fov: Float32,
    pub screen_aspect_ratio: Float32,
    pub world_far_plane: Float32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelProjection {
    pub unscaled_horizontal_fov_4_by_3: Float32,
    pub horizontal_fov: Float32,
    pub aspect_ratio: Float32,
    pub near_plane: Float32,
    pub far_plane: Float32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewModelCullMode {
    CounterClockwise,
    Clockwise,
}

pub fn viewmodel_cull_mode(flipped: bool) -> ViewModelCullMode {
    if flipped {
        ViewModelCullMode::Clockwise
    } else {
        ViewModelCullMode::CounterClockwise
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelPassState {
    pub projection: ViewModelProjection,
    pub view_depth_range: [Float32; 2],
    pub restored_depth_range: [Float32; 2],
    pub initial_color_modulation: [Float32; 3],
    pub initial_alpha_modulation: Float32,
    pub draws_after_world: bool,
    pub opaque_before_translucent: bool,
    pub projection_pushed: bool,
    pub projection_restored: bool,
    pub view_pushed: bool,
    pub view_restored: bool,
    pub restored_cull_mode: ViewModelCullMode,
}

pub fn viewmodel_pass_state(
    request: ViewModelProjectionRequest,
) -> Result<ViewModelPassState, PresentationError> {
    let configured = finite(request.configured_horizontal_fov_4_by_3)?;
    let default_world = finite(request.default_world_fov)?;
    let current_world = finite(request.current_world_fov)?;
    let aspect = finite(request.screen_aspect_ratio)?;
    let far = finite(request.world_far_plane)?;
    if !(configured > 0.0
        && configured < 180.0
        && default_world > 0.0
        && default_world < 180.0
        && current_world > 0.0
        && current_world < 180.0
        && aspect > 0.0
        && far > 0.0)
    {
        return Err(invalid_viewmodel_state());
    }
    let unscaled = configured - (default_world - current_world);
    let ratio = aspect * 0.75;
    let half_angle = unscaled * (0.5 * std::f32::consts::PI / 180.0);
    let mut tangent = half_angle.tan();
    tangent *= ratio;
    let ret_degrees = (180.0 / std::f32::consts::PI) * tangent.atan();
    let horizontal = ret_degrees * 2.0;
    if !unscaled.is_finite() || !horizontal.is_finite() {
        return Err(invalid_viewmodel_state());
    }
    Ok(ViewModelPassState {
        projection: ViewModelProjection {
            unscaled_horizontal_fov_4_by_3: float(unscaled),
            horizontal_fov: float(horizontal),
            aspect_ratio: request.screen_aspect_ratio,
            near_plane: float(1.0),
            far_plane: request.world_far_plane,
        },
        view_depth_range: [float(0.0), float(0.1)],
        restored_depth_range: [float(0.0), float(1.0)],
        initial_color_modulation: [float(1.0); 3],
        initial_alpha_modulation: float(1.0),
        draws_after_world: true,
        opaque_before_translucent: true,
        projection_pushed: true,
        projection_restored: true,
        view_pushed: true,
        view_restored: true,
        restored_cull_mode: ViewModelCullMode::CounterClockwise,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelOffsetRequest {
    pub eye_origin: Vector3,
    pub eye_angles: Vector3,
    pub lowered_pitch: Float32,
    pub inspecting: bool,
    pub inspect_interpolation: Float32,
    pub inspect_offset: Option<Vector3>,
    pub minimized_offset: Option<Vector3>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelTransform {
    pub origin: Vector3,
    pub angles: Vector3,
}

pub fn viewmodel_offset_transform(
    request: ViewModelOffsetRequest,
) -> Result<ViewModelTransform, PresentationError> {
    let mut origin = vector_values(request.eye_origin)?;
    let mut angles = vector_values(request.eye_angles)?;
    let lowered = finite(request.lowered_pitch)?;
    let interpolation = finite(request.inspect_interpolation)?;
    if !(0.0..=1.0).contains(&interpolation) {
        return Err(invalid_viewmodel_state());
    }
    angles[0] += lowered;
    if request.inspecting
        && let Some(offset) = request.inspect_offset
    {
        add_local_offset(
            &mut origin,
            vector_values(request.eye_angles)?,
            vector_values(offset)?,
            gain(interpolation, 0.5),
        );
    }
    if let Some(offset) = request.minimized_offset {
        add_local_offset(
            &mut origin,
            vector_values(request.eye_angles)?,
            vector_values(offset)?,
            gain(1.0 - interpolation, 0.5),
        );
    }
    if origin.iter().chain(&angles).any(|value| !value.is_finite()) {
        return Err(invalid_viewmodel_state());
    }
    Ok(ViewModelTransform {
        origin: vector(origin),
        angles: vector(angles),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelBobState {
    pub bob_time: Float32,
    pub last_bob_time: Float32,
    pub last_speed: Float32,
    pub vertical_bob: Float32,
    pub lateral_bob: Float32,
}

impl Default for ViewModelBobState {
    fn default() -> Self {
        Self {
            bob_time: float(0.0),
            last_bob_time: float(0.0),
            last_speed: float(0.0),
            vertical_bob: float(0.0),
            lateral_bob: float(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelBobRequest {
    pub current_time: Float32,
    pub frame_time: Float32,
    pub planar_speed: Float32,
    pub cycle: Float32,
    pub up_fraction: Float32,
}

pub fn update_viewmodel_bob(
    state: ViewModelBobState,
    request: ViewModelBobRequest,
) -> Result<ViewModelBobState, PresentationError> {
    let current_time = finite(request.current_time)?;
    let frame_time = finite(request.frame_time)?;
    let mut speed = finite(request.planar_speed)?;
    let mut cycle_length = finite(request.cycle)?;
    let mut up_fraction = finite(request.up_fraction)?;
    let mut bob_time = finite(state.bob_time)?;
    let last_bob_time = finite(state.last_bob_time)?;
    let last_speed = finite(state.last_speed)?;
    let vertical_bob = finite(state.vertical_bob)?;
    let lateral_bob = finite(state.lateral_bob)?;
    if speed < 0.0 {
        return Err(invalid_viewmodel_state());
    }
    if frame_time == 0.0 {
        return Ok(state);
    }
    if up_fraction <= 0.0 {
        up_fraction = 0.01;
    }
    if cycle_length <= 0.0 {
        cycle_length = 0.01;
    }
    let speed_delta = ((current_time - last_bob_time) * 320.0).max(0.0);
    speed = speed
        .clamp(last_speed - speed_delta, last_speed + speed_delta)
        .clamp(-320.0, 320.0);
    let bob_offset = speed / 320.0;
    bob_time += (current_time - last_bob_time) * bob_offset;
    let vertical_cycle = bob_cycle(bob_time, cycle_length, up_fraction, false);
    let mut next_vertical = speed * 0.005;
    next_vertical = next_vertical * 0.3 + next_vertical * 0.7 * vertical_cycle.sin();
    next_vertical = next_vertical.clamp(-7.0, 4.0);
    let lateral_cycle = bob_cycle(bob_time, cycle_length, up_fraction, true);
    let mut next_lateral = speed * 0.005;
    next_lateral = next_lateral * 0.3 + next_lateral * 0.7 * lateral_cycle.sin();
    next_lateral = next_lateral.clamp(-7.0, 4.0);
    if [
        bob_time,
        speed,
        next_vertical,
        next_lateral,
        vertical_bob,
        lateral_bob,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(invalid_viewmodel_state());
    }
    Ok(ViewModelBobState {
        bob_time: float(bob_time),
        last_bob_time: float(current_time),
        last_speed: float(speed),
        vertical_bob: float(next_vertical),
        lateral_bob: float(next_lateral),
    })
}

pub fn apply_viewmodel_bob(
    transform: ViewModelTransform,
    state: ViewModelBobState,
) -> Result<ViewModelTransform, PresentationError> {
    let mut origin = vector_values(transform.origin)?;
    let mut angles = vector_values(transform.angles)?;
    let vertical = finite(state.vertical_bob)?;
    let lateral = finite(state.lateral_bob)?;
    let (forward, right, _) = angle_vectors(angles);
    add_scaled(&mut origin, forward, vertical * 0.4);
    origin[2] += vertical * 0.1;
    angles[2] += vertical * 0.5;
    angles[0] -= vertical * 0.4;
    angles[1] -= lateral * 0.3;
    add_scaled(&mut origin, right, lateral * 0.2);
    if origin.iter().chain(&angles).any(|value| !value.is_finite()) {
        return Err(invalid_viewmodel_state());
    }
    Ok(ViewModelTransform {
        origin: vector(origin),
        angles: vector(angles),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewModelAttachmentFormatRequest {
    pub view_origin: Vector3,
    pub view_right: Vector3,
    pub view_up: Vector3,
    pub view_forward: Vector3,
    pub world_horizontal_fov: Float32,
    pub viewmodel_horizontal_fov: Float32,
    pub inverse: bool,
}

pub fn position_viewmodel_attachment(
    attachment: Matrix3x4,
    view_origin: Vector3,
    view_orientation: [Float32; 4],
    viewmodel_transform: ViewModelTransform,
    world_horizontal_fov: Float32,
    viewmodel_horizontal_fov: Float32,
) -> Result<Matrix3x4, PresentationError> {
    let mut magnitude = 0.0;
    for component in view_orientation {
        let value = finite(component)?;
        magnitude += value * value;
    }
    if (magnitude - 1.0).abs() > 1.0e-4 {
        return Err(invalid_viewmodel_state());
    }
    let camera = quaternion_matrix(view_orientation, view_origin);
    let origin = vector_values(viewmodel_transform.origin)?;
    let angles = vector_values(viewmodel_transform.angles)?;
    let (forward, right, up) = angle_vectors(angles);
    let local = Matrix3x4(
        [
            forward[0], -right[0], up[0], origin[0], forward[1], -right[1], up[1], origin[1],
            forward[2], -right[2], up[2], origin[2],
        ]
        .map(float),
    );
    let positioned = multiply_matrix(&multiply_matrix(&camera, &local), &attachment);
    let matrix = camera.0.map(|value| f32::from_bits(value.0));
    format_viewmodel_attachment(
        positioned,
        ViewModelAttachmentFormatRequest {
            view_origin,
            view_right: vector([-matrix[1], -matrix[5], -matrix[9]]),
            view_up: vector([matrix[2], matrix[6], matrix[10]]),
            view_forward: vector([matrix[0], matrix[4], matrix[8]]),
            world_horizontal_fov,
            viewmodel_horizontal_fov,
            inverse: false,
        },
    )
}

pub fn format_viewmodel_attachment(
    mut transform: Matrix3x4,
    request: ViewModelAttachmentFormatRequest,
) -> Result<Matrix3x4, PresentationError> {
    let view_origin = vector_values(request.view_origin)?;
    let right = vector_values(request.view_right)?;
    let up = vector_values(request.view_up)?;
    let forward = vector_values(request.view_forward)?;
    let world_fov = finite(request.world_horizontal_fov)?;
    let viewmodel_fov = finite(request.viewmodel_horizontal_fov)?;
    if !(0.0..180.0).contains(&world_fov) || !(0.0..180.0).contains(&viewmodel_fov) {
        return Err(invalid_viewmodel_state());
    }
    let origin = matrix_translation(&transform);
    let origin = vector_values(origin)?;
    let delta = [
        origin[0] - view_origin[0],
        origin[1] - view_origin[1],
        origin[2] - view_origin[2],
    ];
    let mut transformed = [dot(right, delta), dot(up, delta), dot(forward, delta)];
    let view = (viewmodel_fov * std::f32::consts::PI / 360.0).tan();
    let factor = if view == 0.0 {
        0.0
    } else {
        (world_fov * std::f32::consts::PI / 360.0).tan() / view
    };
    if request.inverse {
        if factor != 0.0 {
            transformed[0] /= factor;
            transformed[1] /= factor;
        } else {
            transformed[0] = 0.0;
            transformed[1] = 0.0;
        }
    } else {
        transformed[0] *= factor;
        transformed[1] *= factor;
    }
    let mut output = view_origin;
    add_scaled(&mut output, right, transformed[0]);
    add_scaled(&mut output, up, transformed[1]);
    add_scaled(&mut output, forward, transformed[2]);
    if output.iter().any(|value| !value.is_finite()) {
        return Err(invalid_viewmodel_state());
    }
    transform.0[3] = float(output[0]);
    transform.0[7] = float(output[1]);
    transform.0[11] = float(output[2]);
    Ok(transform)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewModelBodygroupMutation {
    pub event: usize,
    pub bodygroup: usize,
    pub name: Vec<u8>,
    pub value: i32,
}

pub fn viewmodel_item_bodygroup_events(
    hand: &PresentationModel,
    item: &PresentationModel,
    sequence: usize,
    previous_cycle: Float32,
    current_cycle: Float32,
) -> Result<Vec<ViewModelBodygroupMutation>, PresentationError> {
    sequence_events_between(hand, sequence, previous_cycle, current_cycle)?
        .into_iter()
        .filter(|event| {
            event
                .name
                .eq_ignore_ascii_case(b"AE_CL_BODYGROUP_SET_VALUE_CMODEL_WPN")
        })
        .filter_map(|event| bodygroup_mutation(item, event).transpose())
        .collect()
}

pub fn apply_viewmodel_bodygroup_events(
    item: &PresentationModel,
    bodygroups: &mut [usize],
    mutations: &[ViewModelBodygroupMutation],
) -> Result<(), PresentationError> {
    if bodygroups.len() != item.body_parts.len() {
        return Err(invalid_composition(&item.identity));
    }
    for mutation in mutations {
        let part = item
            .body_parts
            .get(mutation.bodygroup)
            .filter(|part| part.name.eq_ignore_ascii_case(&mutation.name))
            .ok_or_else(|| invalid_composition(&item.identity))?;
        let value =
            usize::try_from(mutation.value).map_err(|_| unsupported_composition(&item.identity))?;
        if value >= part.model_names.len() {
            continue;
        }
        bodygroups[mutation.bodygroup] = value;
    }
    Ok(())
}

fn bodygroup_mutation(
    item: &PresentationModel,
    event: &SequenceEvent,
) -> Result<Option<ViewModelBodygroupMutation>, PresentationError> {
    let options = event
        .options
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or_default();
    let separator = options.iter().position(|byte| *byte == b' ');
    let (name, remaining) = separator.map_or((options, &[][..]), |separator| {
        (&options[..separator], &options[separator + 1..])
    });
    let value = remaining
        .iter()
        .position(|byte| *byte == b' ')
        .map_or(remaining, |separator| &remaining[..separator]);
    let Some(bodygroup) = item
        .body_parts
        .iter()
        .position(|part| part.name.eq_ignore_ascii_case(name))
    else {
        return Ok(None);
    };
    let value = source_integer(value);
    Ok(Some(ViewModelBodygroupMutation {
        event: event.index,
        bodygroup,
        name: name.to_vec(),
        value,
    }))
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
    let item_skin = if request.skin < item.skins.len() {
        request.skin
    } else {
        0
    };
    let item_primitives =
        select_primitives(item, &request.item_bodygroups, item_skin, request.lod)?;
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

pub fn merge_model_pose(
    parent: &PresentationModel,
    parent_pose: &SampledPose,
    child: &PresentationModel,
    child_pose: &SampledPose,
) -> Result<SampledPose, PresentationError> {
    let bone_map = child.bones.iter().map(|bone| parent.bones.iter().position(|p| p.name.eq_ignore_ascii_case(&bone.name))).collect::<Vec<_>>();
    merged_item_pose(child, child_pose, parent, parent_pose, &bone_map)
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

fn add_local_offset(origin: &mut [f32; 3], angles: [f32; 3], offset: [f32; 3], scale: f32) {
    let (forward, right, up) = angle_vectors(angles);
    add_scaled(origin, forward, offset[0] * scale);
    add_scaled(origin, right, offset[1] * scale);
    add_scaled(origin, up, offset[2] * scale);
}

fn angle_vectors(angles: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (sp, cp) = angles[0].to_radians().sin_cos();
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

fn gain(value: f32, bias_amount: f32) -> f32 {
    if value < 0.5 {
        0.5 * bias(2.0 * value, 1.0 - bias_amount)
    } else {
        1.0 - 0.5 * bias(2.0 - 2.0 * value, 1.0 - bias_amount)
    }
}

#[allow(clippy::approx_constant)]
fn bias(value: f32, amount: f32) -> f32 {
    value.powf(amount.ln() * -1.4427)
}

fn bob_cycle(bob_time: f32, cycle: f32, up: f32, doubled: bool) -> f32 {
    let mut value = if doubled {
        bob_time - (bob_time / cycle * 2.0) as i32 as f32 * cycle * 2.0
    } else {
        bob_time - (bob_time / cycle) as i32 as f32 * cycle
    };
    value /= if doubled { cycle * 2.0 } else { cycle };
    if value < up {
        std::f32::consts::PI * value / up
    } else {
        std::f32::consts::PI + std::f32::consts::PI * (value - up) / (1.0 - up)
    }
}

fn add_scaled(output: &mut [f32; 3], direction: [f32; 3], scale: f32) {
    for axis in 0..3 {
        output[axis] += direction[axis] * scale;
    }
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn finite(value: Float32) -> Result<f32, PresentationError> {
    let value = f32::from_bits(value.0);
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(invalid_viewmodel_state)
}

fn vector_values(value: Vector3) -> Result<[f32; 3], PresentationError> {
    let values = value.0.map(|value| f32::from_bits(value.0));
    values
        .iter()
        .all(|value| value.is_finite())
        .then_some(values)
        .ok_or_else(invalid_viewmodel_state)
}

fn float(value: f32) -> Float32 {
    Float32(value.to_bits())
}

fn vector(values: [f32; 3]) -> Vector3 {
    Vector3(values.map(float))
}

fn source_integer(value: &[u8]) -> i32 {
    let Ok(value) = std::str::from_utf8(value) else {
        return 0;
    };
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

fn invalid_viewmodel_state() -> PresentationError {
    PresentationError {
        code: PresentationErrorCode::InvalidState,
        identity: "viewmodel".to_owned(),
    }
}

fn invalid_composition(identity: &str) -> PresentationError {
    PresentationError {
        code: PresentationErrorCode::InvalidState,
        identity: identity.to_owned(),
    }
}

fn unsupported_composition(identity: &str) -> PresentationError {
    PresentationError {
        code: PresentationErrorCode::UnsupportedState,
        identity: identity.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EntityAngleConvention, FarPlaneContract, FeatureSupport, GeometryOrientation, ModelBasis,
        PhysicsStatus, PresentationBodyPart, PresentationMaterial, PresentationProfile, SkinFamily,
        TangentHandednessConvention, TextureCoordinateConvention, VertexAttributeTransform,
        ViewmodelHandednessContract,
    };

    fn values(value: Vector3) -> [f32; 3] {
        value.0.map(|value| f32::from_bits(value.0))
    }

    fn identity(translation: [f32; 3]) -> Matrix3x4 {
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
            .map(float),
        )
    }

    fn viewmodel_descriptor() -> PresentationDescriptor {
        PresentationDescriptor::ViewModel {
            geometry: GeometryOrientation {
                positions: VertexAttributeTransform::AuthoredSourceValues,
                normals: VertexAttributeTransform::AuthoredSourceValues,
                tangents: VertexAttributeTransform::AuthoredSourceValues,
                texture_coordinates: TextureCoordinateConvention::AuthoredUTowardRightVDown,
                tangent_handedness: TangentHandednessConvention::TangentSWComponent,
                deformation: crate::VertexDeformationContract::FlexBeforeLinearBoneSkinningWithoutTopologyChanges,
                facing: GeometryFacing {
                    front_face: crate::TriangleWinding::Clockwise,
                    cull_face: crate::CullFace::Back,
                },
            },
            entity_angles: EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns,
            default_horizontal_fov_4_by_3: float(54.0),
            minimum_fov: float(0.1),
            maximum_fov: float(179.9),
            near_plane: float(1.0),
            far_plane: FarPlaneContract::SuppliedWorldFarPlane,
            depth_range: [float(0.0), float(0.1)],
            draws_after_world: true,
            opaque_before_translucent: true,
            handedness: ViewmodelHandednessContract::OptionalViewSpaceYReflection,
        }
    }

    fn empty_model(identity: &str, flags: i32, material_count: usize) -> PresentationModel {
        PresentationModel {
            profile: PresentationProfile::ViewModel,
            descriptor: viewmodel_descriptor(),
            identity: identity.to_owned(),
            checksum: 0,
            flags,
            basis: ModelBasis {
                forward: vector([1.0, 0.0, 0.0]),
                left: vector([0.0, 1.0, 0.0]),
                up: vector([0.0, 0.0, 1.0]),
            },
            dependencies: Vec::new(),
            base_material_count: material_count,
            materials: (0..material_count)
                .map(|slot| PresentationMaterial {
                    slot,
                    source_slot: slot,
                    lod: None,
                    authored_name: format!("material{slot}").into_bytes(),
                    material_dependency: 0,
                    include_dependencies: Vec::new(),
                    textures: Vec::new(),
                })
                .collect(),
            bones: Vec::new(),
            animations: Vec::new(),
            sequences: Vec::new(),
            pose_parameters: Vec::new(),
            attachments: Vec::new(),
            hitbox_sets: Vec::new(),
            skins: Vec::<SkinFamily>::new(),
            body_parts: Vec::new(),
            geometry: Vec::new(),
            physics_status: PhysicsStatus::Missing,
            features: Vec::<FeatureSupport>::new(),
        }
    }

    #[test]
    fn projection_pass_preserves_source_fov_and_restoration_contract() {
        let pass = viewmodel_pass_state(ViewModelProjectionRequest {
            configured_horizontal_fov_4_by_3: float(54.0),
            default_world_fov: float(90.0),
            current_world_fov: float(75.0),
            screen_aspect_ratio: float(4.0 / 3.0),
            world_far_plane: float(28_000.0),
        })
        .unwrap();
        assert_eq!(
            f32::from_bits(pass.projection.unscaled_horizontal_fov_4_by_3.0),
            39.0
        );
        assert_eq!(f32::from_bits(pass.projection.horizontal_fov.0), 39.0);
        assert_eq!(pass.view_depth_range, [float(0.0), float(0.1)]);
        assert_eq!(pass.restored_depth_range, [float(0.0), float(1.0)]);
        assert!(pass.draws_after_world && pass.opaque_before_translucent);
        assert!(pass.projection_pushed && pass.projection_restored);
        assert!(pass.view_pushed && pass.view_restored);
        assert_eq!(pass.restored_cull_mode, ViewModelCullMode::CounterClockwise);
        assert_eq!(
            viewmodel_cull_mode(false),
            ViewModelCullMode::CounterClockwise
        );
        assert_eq!(viewmodel_cull_mode(true), ViewModelCullMode::Clockwise);
    }

    #[test]
    fn eligibility_retains_ordered_suppression_and_lowered_success() {
        let ready = ViewModelDrawEligibility {
            client_mode: true,
            render_request: true,
            render_viewmodels: true,
            local_player_visible: false,
            draw_entities: true,
            player_view_entity: true,
            base_should_draw: true,
            fully_lowered: false,
            observer_owner_matches: true,
            owner_alive: true,
            ready: true,
            fx_blend: 255,
        };
        assert_eq!(
            viewmodel_draw_disposition(ready),
            ViewModelDrawDisposition::Draw
        );
        assert_eq!(
            viewmodel_draw_disposition(ViewModelDrawEligibility {
                render_request: false,
                fully_lowered: true,
                ..ready
            }),
            ViewModelDrawDisposition::Suppressed(ViewModelDrawSuppression::RenderRequest)
        );
        assert_eq!(
            viewmodel_draw_disposition(ViewModelDrawEligibility {
                fully_lowered: true,
                ready: false,
                ..ready
            }),
            ViewModelDrawDisposition::SuppressedSuccess(ViewModelDrawSuppression::FullyLowered)
        );
    }

    #[test]
    fn minimized_offset_bob_and_attachment_format_use_view_space_axes() {
        let offset = viewmodel_offset_transform(ViewModelOffsetRequest {
            eye_origin: vector([0.0; 3]),
            eye_angles: vector([0.0; 3]),
            lowered_pitch: float(10.0),
            inspecting: false,
            inspect_interpolation: float(0.0),
            inspect_offset: None,
            minimized_offset: Some(vector([1.0, 2.0, 3.0])),
        })
        .unwrap();
        assert_eq!(values(offset.origin), [1.0, -2.0, 3.0]);
        assert_eq!(values(offset.angles), [10.0, 0.0, 0.0]);

        let initial = ViewModelBobState::default();
        assert_eq!(
            update_viewmodel_bob(
                initial,
                ViewModelBobRequest {
                    current_time: float(0.1),
                    frame_time: float(0.0),
                    planar_speed: float(320.0),
                    cycle: float(0.8),
                    up_fraction: float(0.5),
                },
            )
            .unwrap(),
            initial
        );
        let bob = update_viewmodel_bob(
            initial,
            ViewModelBobRequest {
                current_time: float(0.1),
                frame_time: float(0.1),
                planar_speed: float(320.0),
                cycle: float(0.8),
                up_fraction: float(0.5),
            },
        )
        .unwrap();
        assert_eq!(f32::from_bits(bob.last_speed.0), 32.0);
        assert!((f32::from_bits(bob.bob_time.0) - 0.01).abs() < 1.0e-8);
        assert_ne!(apply_viewmodel_bob(offset, bob).unwrap(), offset);

        let positioned = position_viewmodel_attachment(
            identity([1.0, 2.0, 3.0]),
            vector([10.0, 20.0, 30.0]),
            [float(0.0), float(0.0), float(0.0), float(1.0)],
            ViewModelTransform {
                origin: vector([4.0, 0.0, 0.0]),
                angles: vector([0.0; 3]),
            },
            float(90.0),
            float(90.0),
        )
        .unwrap();
        assert_eq!(values(matrix_translation(&positioned)), [15.0, 22.0, 33.0]);

        let formatted = format_viewmodel_attachment(
            identity([1.0, 2.0, 3.0]),
            ViewModelAttachmentFormatRequest {
                view_origin: vector([0.0; 3]),
                view_right: vector([1.0, 0.0, 0.0]),
                view_up: vector([0.0, 1.0, 0.0]),
                view_forward: vector([0.0, 0.0, 1.0]),
                world_horizontal_fov: float(90.0),
                viewmodel_horizontal_fov: float(54.0),
                inverse: false,
            },
        )
        .unwrap();
        let restored = format_viewmodel_attachment(
            formatted,
            ViewModelAttachmentFormatRequest {
                view_origin: vector([0.0; 3]),
                view_right: vector([1.0, 0.0, 0.0]),
                view_up: vector([0.0, 1.0, 0.0]),
                view_forward: vector([0.0, 0.0, 1.0]),
                world_horizontal_fov: float(90.0),
                viewmodel_horizontal_fov: float(54.0),
                inverse: true,
            },
        )
        .unwrap();
        let restored = values(matrix_translation(&restored));
        for (actual, expected) in restored.into_iter().zip([1.0, 2.0, 3.0]) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
        let zero_fov = format_viewmodel_attachment(
            identity([1.0, 2.0, 3.0]),
            ViewModelAttachmentFormatRequest {
                view_origin: vector([0.0; 3]),
                view_right: vector([1.0, 0.0, 0.0]),
                view_up: vector([0.0, 1.0, 0.0]),
                view_forward: vector([0.0, 0.0, 1.0]),
                world_horizontal_fov: float(90.0),
                viewmodel_horizontal_fov: float(0.0),
                inverse: true,
            },
        )
        .unwrap();
        assert_eq!(values(matrix_translation(&zero_fov)), [0.0, 0.0, 3.0]);
    }

    #[test]
    fn draw_plan_partitions_materials_and_keeps_force_opaque_overlap_separate() {
        let hand = empty_model("models/hand.mdl", 0, 2);
        let mut item = empty_model("models/item.mdl", STUDIO_HEADER_FORCE_OPAQUE, 1);
        let part = |identity: &str, primitives| ComposedViewModelPart {
            identity: identity.to_owned(),
            pose: SampledPose {
                local_translations: Vec::new(),
                local_rotations: Vec::new(),
                model_matrices: Vec::new(),
                skinning_matrices: Vec::new(),
                attachments: Vec::new(),
            },
            primitives,
        };
        let composition = ViewModelComposition {
            translated_activity: b"ACT_TEST".to_vec(),
            skin: 0,
            hand: part(
                &hand.identity,
                vec![
                    SelectedPrimitive {
                        primitive: 0,
                        material: 0,
                    },
                    SelectedPrimitive {
                        primitive: 1,
                        material: 1,
                    },
                ],
            ),
            item: part(
                &item.identity,
                vec![SelectedPrimitive {
                    primitive: 0,
                    material: 0,
                }],
            ),
            item_to_hand_bones: Vec::new(),
        };
        let forced = viewmodel_draw_plan(
            &hand,
            &item,
            &composition,
            &[
                ViewModelMaterialOpacity::Opaque,
                ViewModelMaterialOpacity::Translucent,
            ],
            &[ViewModelMaterialOpacity::Translucent],
        )
        .unwrap();
        assert!(!forced.item_entity_translucent);
        assert_eq!(forced.parts[0].part, ViewModelPart::Item);
        assert_eq!(forced.parts[0].translucent_primitives.len(), 1);
        assert_eq!(forced.parts[1].opaque_primitives.len(), 1);
        assert_eq!(forced.parts[1].translucent_primitives.len(), 1);

        item.flags = 0;
        let translucent = viewmodel_draw_plan(
            &hand,
            &item,
            &composition,
            &[
                ViewModelMaterialOpacity::Opaque,
                ViewModelMaterialOpacity::Translucent,
            ],
            &[ViewModelMaterialOpacity::Translucent],
        )
        .unwrap();
        assert!(translucent.item_entity_translucent);
        assert_eq!(
            translucent
                .parts
                .iter()
                .map(|part| part.part)
                .collect::<Vec<_>>(),
            [ViewModelPart::Hand, ViewModelPart::Item]
        );
    }

    #[test]
    fn cmodel_bodygroup_events_are_ordered_matched_and_explicit() {
        let mut hand = empty_model("models/hand.mdl", 0, 0);
        let mut item = empty_model("models/item.mdl", 0, 0);
        item.body_parts = vec![PresentationBodyPart {
            index: 0,
            name: b"body".to_vec(),
            base: 1,
            model_names: vec![b"visible".to_vec(), b"hidden".to_vec()],
        }];
        let options = |value: &[u8]| {
            let mut options = [0_u8; 64];
            options[..value.len()].copy_from_slice(value);
            options
        };
        hand.sequences.push(crate::Sequence {
            index: 0,
            label: b"test".to_vec(),
            activity_name: b"ACT_TEST".to_vec(),
            flags: 0,
            activity: 0,
            activity_weight: 1,
            event_count: 4,
            bounds_min: vector([0.0; 3]),
            bounds_max: vector([0.0; 3]),
            blend_count: 1,
            blend_size: [1, 1],
            animation_indices: vec![0],
            pose_parameter_indices: [-1, -1],
            pose_parameter_start: [float(0.0); 2],
            pose_parameter_end: [float(0.0); 2],
            fade_in: float(0.0),
            fade_out: float(0.0),
            entry_node: 0,
            exit_node: 0,
            node_flags: 0,
            entry_phase: float(0.0),
            exit_phase: float(0.0),
            last_frame: float(1.0),
            next_sequence: -1,
            pose: 0,
            auto_layer_count: 0,
            bone_weights: Vec::new(),
            pose_keys: [Vec::new(), Vec::new()],
            ik_lock_count: 0,
            cycle_pose_parameter: -1,
            activity_modifier_count: 0,
            events: vec![
                SequenceEvent {
                    index: 0,
                    cycle: float(0.1),
                    event: 0,
                    event_type: 1 << 10,
                    options: options(b"body 1"),
                    name: b"AE_CL_BODYGROUP_SET_VALUE_CMODEL_WPN".to_vec(),
                },
                SequenceEvent {
                    index: 1,
                    cycle: float(0.2),
                    event: 5000,
                    event_type: 0,
                    options: options(b"unmatched 1"),
                    name: b"AE_CL_BODYGROUP_SET_VALUE_CMODEL_WPN".to_vec(),
                },
                SequenceEvent {
                    index: 2,
                    cycle: float(0.3),
                    event: 5000,
                    event_type: 0,
                    options: options(b"BODY"),
                    name: b"AE_CL_BODYGROUP_SET_VALUE_CMODEL_WPN".to_vec(),
                },
                SequenceEvent {
                    index: 3,
                    cycle: float(0.4),
                    event: 5000,
                    event_type: 0,
                    options: options(b"body  1"),
                    name: b"AE_CL_BODYGROUP_SET_VALUE_CMODEL_WPN".to_vec(),
                },
            ],
            auto_layers: Vec::new(),
            source_identity: hand.identity.clone(),
        });
        let mutations =
            viewmodel_item_bodygroup_events(&hand, &item, 0, float(0.0), float(0.5)).unwrap();
        assert_eq!(
            mutations
                .iter()
                .map(|mutation| (mutation.event, mutation.bodygroup, mutation.value))
                .collect::<Vec<_>>(),
            [(0, 0, 1), (2, 0, 0), (3, 0, 0)]
        );
        let mut bodygroups = vec![0];
        apply_viewmodel_bodygroup_events(&item, &mut bodygroups, &mutations).unwrap();
        assert_eq!(bodygroups, [0]);
    }
}
