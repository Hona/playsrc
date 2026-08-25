use crate::{
    CONTENTS_CURRENT_0, CONTENTS_CURRENT_90, CONTENTS_CURRENT_180, CONTENTS_CURRENT_270,
    CONTENTS_CURRENT_DOWN, CONTENTS_CURRENT_UP, CONTENTS_LADDER, CONTENTS_SLIME, CONTENTS_WATER,
    Configuration, Contact, CrouchPhase, Error, Event, FailureKind, GroundState, MASK_CURRENT,
    MAX_COMMAND_MAGNITUDE, MAX_COORDINATE, Mode, MoveCollision, MoverResult, MoverStatus,
    ObserverMode, Operation, PointQueryPurpose, PointQueryRecord, Policy, QueryPurpose,
    QueryRecord, State, StateDisposition, StepInput, StepResult, StepStrategy, StuckRecoveryMode,
    Trace, Tracer, WaterSampling, WishState,
};
use playsrc_collision::Hull;

const MAX_BUMPS: usize = 4;
const MAX_CLIP_PLANES: usize = 5;
const STUCK_OFFSETS: usize = 54;

#[derive(Clone, Copy)]
struct PreparedCommand {
    forward_move: f32,
    side_move: f32,
    up_move: f32,
    maximum_speed: f32,
    forward: [f32; 3],
    right: [f32; 3],
}

pub(super) fn step(
    tracer: &impl Tracer,
    mut state: State,
    mut input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> Result<StepResult, Error> {
    validate(state, input, configuration, policy)?;
    let tick = configuration.tick_interval * configuration.lagged_movement_scale;
    let mut context = QueryContext::new(tracer);
    let mut events = Vec::new();

    apply_mode_request(&mut state, input.mode_request, policy, &mut events);
    if matches!(state.mode, Mode::Fly | Mode::FlyGravity)
        && matches!(
            state.move_collision,
            MoveCollision::Custom | MoveCollision::Slide
        )
    {
        return Err(Error::new(
            Operation::Move,
            FailureKind::Unsupported,
            "fly-collision-response",
        ));
    }

    prepare_base_velocity(tracer, &mut state, tick)?;
    let mut command = prepare_command(&mut state, input, configuration, policy);
    let selected_hull = selected_hull(state, configuration, policy)?;
    let mover_result = apply_mover(&mut context, &mut state, selected_hull, configuration)?;

    let mut result = StepResult {
        state,
        selected_hull,
        wish_state: WishState::default(),
        wish_velocity: [0.0; 3],
        jump_velocity: [0.0; 3],
        climbed_step: 0.0,
        contacts: Vec::new(),
        events,
        queries: Vec::new(),
        point_queries: Vec::new(),
        mover_result,
    };

    if mode_checks_stuck(result.state.mode)
        && !check_stuck(
            &mut context,
            &mut result.state,
            input.command_number,
            selected_hull,
            configuration,
            &mut result.contacts,
            &mut result.events,
        )?
    {
        finish_step(&mut result, input, configuration, policy)?;
        result.queries = context.sweeps;
        result.point_queries = context.points;
        return Ok(result);
    }

    if result.state.mode != Mode::Walk || !configuration.optimized_movement {
        categorize_position(
            &mut context,
            &mut result.state,
            configuration,
            policy,
            &mut result.contacts,
            &mut result.events,
        )?;
    } else if result.state.velocity[2] > 250.0 {
        set_ground(
            &context,
            &mut result.state,
            None,
            &mut result.contacts,
            &mut result.events,
        )?;
    }

    let old_water_level = result.state.water_level;
    let old_water_type = result.state.water_type;
    if result.state.ground.is_none() && result.state.velocity[2] < 0.0 {
        result.state.fall_speed = result.state.fall_speed.max(-result.state.velocity[2]);
    }

    if policy.water.suppress_airborne_duck
        && result.state.water_level >= 1
        && result.state.ground.is_none()
        || policy.water.suppress_submerged_duck && result.state.water_level >= 3
    {
        input.command.crouch = false;
    }

    update_crouch(
        &mut context,
        &mut result.state,
        input.command.crouch,
        tick,
        configuration,
        policy,
        &mut result.events,
    )?;
    crop_crouched_command(&result.state, &mut command, policy);

    update_ladder(
        &mut context,
        &mut result.state,
        input,
        command,
        configuration,
        policy,
        &mut result.events,
    )?;

    match result.state.mode {
        Mode::None => {}
        Mode::Noclip => noclip(&mut result, input, command, configuration, policy, tick),
        Mode::Walk | Mode::Isometric => full_walk(
            &mut context,
            &mut result,
            input,
            command,
            configuration,
            policy,
            tick,
        )?,
        Mode::Ladder => full_ladder(&mut context, &mut result, configuration, policy, tick)?,
        Mode::Observer => observer_move(
            &mut context,
            &mut result,
            input,
            command,
            configuration,
            policy,
            tick,
        )?,
        Mode::Fly | Mode::FlyGravity => toss_move(
            &mut context,
            &mut result,
            command,
            configuration,
            policy,
            tick,
        )?,
    }

    if old_water_level == 0 && result.state.water_level != 0 {
        result.events.push(Event::WaterEntered {
            level: result.state.water_level,
            contents: result.state.water_type,
        });
    } else if old_water_level != 0 && result.state.water_level == 0 {
        result.events.push(Event::WaterExited {
            previous_level: old_water_level,
            contents: old_water_type,
        });
    }

    finish_step(&mut result, input, configuration, policy)?;
    result.queries = context.sweeps;
    result.point_queries = context.points;
    Ok(result)
}

fn finish_step(
    result: &mut StepResult,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> Result<(), Error> {
    result.state.previous_jump = input.command.jump;
    result.state.previous_crouch = input.command.crouch;
    result.state.previous_forward = input.command.forward;
    result.selected_hull = selected_hull(result.state, configuration, policy)?;
    validate_output(result.state, configuration)
}

fn apply_mode_request(
    state: &mut State,
    request: Option<crate::ModeRequest>,
    policy: Policy,
    events: &mut Vec<Event>,
) {
    let Some(request) = request else { return };
    if request.mode == state.mode {
        return;
    }
    if request.mode == Mode::Noclip && !policy.allow_noclip {
        events.push(Event::ModeDenied {
            requested: request.mode,
        });
        return;
    }
    let from = state.mode;
    if request.disposition.velocity == StateDisposition::Reset {
        state.velocity = [0.0; 3];
        state.base_velocity = [0.0; 3];
        state.base_velocity_applied = false;
    }
    if request.disposition.ground == StateDisposition::Reset {
        state.ground = None;
        state.fall_speed = 0.0;
    }
    if request.disposition.water == StateDisposition::Reset {
        clear_water(state);
    }
    state.mode = request.mode;
    events.push(Event::ModeChanged {
        from,
        to: request.mode,
        disposition: request.disposition,
    });
}

fn prepare_base_velocity(tracer: &impl Tracer, state: &mut State, tick: f32) -> Result<(), Error> {
    let mut applied = state.base_velocity_applied;
    if let Some(support) = state.ground.and_then(|ground| ground.support)
        && let Some(mut conveyor) = tracer.conveyor_velocity(support)?
    {
        if applied {
            conveyor = add(conveyor, state.base_velocity);
        }
        state.base_velocity = conveyor;
        applied = true;
    }
    if !applied && state.base_velocity != [0.0; 3] {
        state.velocity = add(state.velocity, scale(state.base_velocity, 1.0 + tick * 0.5));
        state.base_velocity = [0.0; 3];
    }
    state.base_velocity_applied = false;
    Ok(())
}

fn prepare_command(
    state: &mut State,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> PreparedCommand {
    let (forward, right, up) = angle_basis(input.pitch_degrees, input.command.yaw_degrees, 0.0);
    let mut maximum_speed = policy.maximum_speed;
    let mut forward_move = input.command.forward;
    let mut side_move = input.command.side;
    let mut up_move = input.up;

    if !matches!(state.mode, Mode::Isometric | Mode::Noclip | Mode::Observer) {
        if configuration.client_max_speed != 0.0 {
            maximum_speed = maximum_speed.min(configuration.client_max_speed);
        }
        let constraint_factor = constraint_speed_factor(
            state.position,
            [forward_move, side_move, up_move],
            [forward, right, up],
            configuration,
        );
        maximum_speed *= configuration
            .surface_max_speed_factor
            .min(constraint_factor);
        let command_speed = length([forward_move, side_move, up_move]);
        if command_speed > maximum_speed && command_speed > 0.0 {
            let factor = maximum_speed / command_speed;
            forward_move *= factor;
            side_move *= factor;
            up_move *= factor;
        }
    }

    state.local_angles = [
        input.pitch_degrees,
        if input.command.yaw_degrees > 180.0 {
            input.command.yaw_degrees - 360.0
        } else {
            input.command.yaw_degrees
        },
        if matches!(state.mode, Mode::Isometric | Mode::Noclip) {
            0.0
        } else {
            movement_roll(
                input.pitch_degrees,
                input.command.yaw_degrees,
                state.velocity,
                configuration.roll_angle,
                configuration.roll_speed,
            )
        },
    ];
    state.absolute_view_angles = [input.pitch_degrees, input.command.yaw_degrees, 0.0];

    PreparedCommand {
        forward_move,
        side_move,
        up_move,
        maximum_speed,
        forward,
        right,
    }
}

fn constraint_speed_factor(
    position: [f32; 3],
    command: [f32; 3],
    basis: [[f32; 3]; 3],
    configuration: Configuration,
) -> f32 {
    let Some(constraint) = configuration.movement_constraint else {
        return 1.0;
    };
    if constraint.radius == 0.0 || constraint.width == 0.0 {
        return 1.0;
    }
    let delta = sub(position, constraint.center);
    let distance_squared = length_squared(delta);
    let inner = constraint.radius - constraint.width;
    if distance_squared <= inner * inner
        || distance_squared >= constraint.radius * constraint.radius
    {
        return 1.0;
    }
    let desired = add(
        add(scale(basis[0], command[0]), scale(basis[1], command[1])),
        scale(basis[2], command[2]),
    );
    if dot(normalized(delta), normalized(desired)) < 0.0 {
        return 1.0;
    }
    let fraction = (distance_squared.sqrt() - inner) / constraint.width;
    1.0 + (constraint.outward_speed_factor - 1.0) * fraction
}

fn selected_hull(
    state: State,
    configuration: Configuration,
    policy: Policy,
) -> Result<Hull, Error> {
    if state.mode == Mode::Observer {
        configuration
            .observer_hull
            .ok_or_else(|| Error::new(Operation::Validate, FailureKind::Missing, "observer-hull"))
    } else {
        Ok(state.active_hull(policy))
    }
}

fn apply_mover(
    context: &mut QueryContext<'_, impl Tracer>,
    state: &mut State,
    hull: Hull,
    configuration: Configuration,
) -> Result<Option<MoverResult>, Error> {
    let support = state.ground.and_then(|ground| ground.support);
    let Some(motion) = context.tracer.mover_motion(state.position, hull, support)? else {
        return Ok(None);
    };
    let grounded = support == Some(motion.identity);
    if !grounded && !motion.swept_contact {
        return Ok(Some(MoverResult {
            identity: motion.identity,
            status: MoverStatus::Moved,
            displacement: [0.0; 3],
            support_velocity: [0.0; 3],
            blocker: None,
        }));
    }

    let destination = add(state.position, motion.displacement);
    let trace = context.trace_without(
        motion.identity,
        QueryPurpose::MoverDisplacement,
        state.position,
        destination,
        hull,
        configuration.solid_mask,
    )?;
    let overlaps = context
        .tracer
        .overlaps_mover(motion.identity, destination, hull)?;
    let blocked = trace.fraction < 1.0 || trace.all_solid || overlaps;
    if blocked && !motion.unblockable {
        return Ok(Some(MoverResult {
            identity: motion.identity,
            status: if trace.all_solid || overlaps {
                MoverStatus::Crushed
            } else {
                MoverStatus::Blocked
            },
            displacement: [0.0; 3],
            support_velocity: [0.0; 3],
            blocker: trace.hit,
        }));
    }

    state.position = if motion.unblockable {
        destination
    } else {
        trace.end
    };
    if grounded {
        state.base_velocity = motion.linear_velocity;
        state.base_velocity_applied = false;
    }
    state.local_angles = add(
        state.local_angles,
        scale(motion.angular_velocity, configuration.tick_interval),
    );
    state.absolute_view_angles = add(
        state.absolute_view_angles,
        scale(motion.angular_velocity, configuration.tick_interval),
    );
    Ok(Some(MoverResult {
        identity: motion.identity,
        status: MoverStatus::Moved,
        displacement: motion.displacement,
        support_velocity: if grounded {
            motion.linear_velocity
        } else {
            [0.0; 3]
        },
        blocker: None,
    }))
}

fn mode_checks_stuck(mode: Mode) -> bool {
    !matches!(
        mode,
        Mode::Noclip | Mode::None | Mode::Isometric | Mode::Observer
    )
}

fn check_stuck(
    context: &mut QueryContext<'_, impl Tracer>,
    state: &mut State,
    command_number: u32,
    hull: Hull,
    configuration: Configuration,
    contacts: &mut Vec<Contact>,
    events: &mut Vec<Event>,
) -> Result<bool, Error> {
    let interval_seconds = if configuration.single_player {
        configuration.stuck_singleplayer_interval
    } else {
        configuration.stuck_multiplayer_interval
    };
    let interval = if state.stuck_offset != 0 {
        1
    } else {
        ((interval_seconds / configuration.tick_interval) as u32).max(1)
    };
    if configuration.optimized_movement
        && !command_number
            .wrapping_add(state.player_identity)
            .is_multiple_of(interval)
    {
        return Ok(true);
    }

    let initial = context.trace(
        QueryPurpose::InitialPosition,
        state.position,
        state.position,
        hull,
        configuration.solid_mask,
    )?;
    if !is_embedded(initial, configuration.solid_mask) {
        state.stuck_offset = 0;
        return Ok(true);
    }

    if configuration.stuck_recovery == StuckRecoveryMode::ClientWorld
        && initial.hit.is_none_or(|hit| context.tracer.is_world(hit))
    {
        state.stuck_offset = 0;
        for (index, offset) in stuck_offsets().into_iter().enumerate() {
            let candidate = add(state.position, offset);
            let trace = context.trace(
                QueryPurpose::StuckRecovery,
                candidate,
                candidate,
                hull,
                configuration.solid_mask,
            )?;
            if !is_embedded(trace, configuration.solid_mask) {
                state.position = candidate;
                state.stuck_offset = 0;
                events.push(Event::Recovered {
                    offset: index as u8,
                });
                return Ok(true);
            }
        }
    }

    let now = context
        .tracer
        .movement_time()
        .ok_or_else(|| Error::new(Operation::Move, FailureKind::Missing, "stuck-movement-time"))?;
    if !now.is_finite() {
        return Err(Error::new(
            Operation::Move,
            FailureKind::Malformed,
            "stuck-movement-time",
        )
        .with_value(now, None));
    }
    if state.stuck_next_check_time >= now - configuration.stuck_retry_seconds {
        record_contact(contacts, initial, state.velocity);
        events.push(Event::Trapped);
        return Ok(false);
    }
    state.stuck_next_check_time = now;
    record_contact(contacts, initial, state.velocity);
    let index = state.stuck_offset as usize % STUCK_OFFSETS;
    state.stuck_offset = ((index + 1) % STUCK_OFFSETS) as u8;
    let candidate = add(state.position, stuck_offsets()[index]);
    let trace = context.trace(
        QueryPurpose::StuckRecovery,
        candidate,
        candidate,
        hull,
        configuration.solid_mask,
    )?;
    if !is_embedded(trace, configuration.solid_mask) {
        state.position = candidate;
        state.stuck_offset = 0;
        events.push(Event::Recovered {
            offset: index as u8,
        });
        return Ok(true);
    }
    events.push(Event::Trapped);
    Ok(false)
}

fn is_embedded(trace: Trace, mask: u32) -> bool {
    trace.start_solid || trace.all_solid || trace.contents & mask != 0 && trace.hit.is_some()
}

fn update_crouch(
    context: &mut QueryContext<'_, impl Tracer>,
    state: &mut State,
    crouch_command: bool,
    tick: f32,
    configuration: Configuration,
    policy: Policy,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    let before = state.crouch.phase;
    let grounded = state.ground.is_some();
    let desired = crouch_command && policy.allow_duck;

    if !grounded {
        if desired && !state.crouch.uses_crouched_hull() {
            state.position = add(state.position, hull_size_delta(policy));
            state.crouch = crate::CrouchState::CROUCHED;
        } else if !desired && state.crouch.uses_crouched_hull() {
            if can_unduck(context, *state, false, configuration, policy)? {
                state.position = sub(state.position, hull_size_delta(policy));
                state.crouch = crate::CrouchState::STANDING;
            } else {
                state.crouch = crate::CrouchState {
                    phase: CrouchPhase::Blocked,
                    fraction: 1.0,
                    linear_fraction: 1.0,
                    start_fraction: 1.0,
                    elapsed: 0.0,
                    duration: policy.unduck_duration,
                };
            }
        } else if !desired && state.crouch.phase == CrouchPhase::Ducking {
            state.crouch = crate::CrouchState::STANDING;
        }
        update_view(state, policy);
        record_crouch_change(before, state.crouch.phase, events);
        return Ok(());
    }

    let reversing = matches!(state.crouch.phase, CrouchPhase::Ducking) && !desired
        || matches!(state.crouch.phase, CrouchPhase::Unducking) && desired;
    if !reversing {
        advance_crouch(state, tick, policy);
    }
    match state.crouch.phase {
        CrouchPhase::Standing if desired => begin_crouch(state, CrouchPhase::Ducking, policy),
        CrouchPhase::Ducking if !desired => begin_crouch(state, CrouchPhase::Unducking, policy),
        CrouchPhase::Crouched if !desired => {
            if can_unduck(context, *state, true, configuration, policy)? {
                begin_crouch(state, CrouchPhase::Unducking, policy);
            } else {
                state.crouch.phase = CrouchPhase::Blocked;
                state.crouch.fraction = 1.0;
                state.crouch.linear_fraction = 1.0;
            }
        }
        CrouchPhase::Unducking if desired => begin_crouch(state, CrouchPhase::Ducking, policy),
        CrouchPhase::Unducking if !can_unduck(context, *state, true, configuration, policy)? => {
            state.crouch = crate::CrouchState {
                phase: CrouchPhase::Blocked,
                fraction: 1.0,
                linear_fraction: 1.0,
                start_fraction: 1.0,
                elapsed: 0.0,
                duration: policy.unduck_duration,
            };
        }
        CrouchPhase::Blocked if desired => state.crouch = crate::CrouchState::CROUCHED,
        CrouchPhase::Blocked if can_unduck(context, *state, true, configuration, policy)? => {
            begin_crouch(state, CrouchPhase::Unducking, policy);
        }
        CrouchPhase::Blocked => {}
        _ => {}
    }
    update_view(state, policy);
    record_crouch_change(before, state.crouch.phase, events);
    Ok(())
}

fn begin_crouch(state: &mut State, phase: CrouchPhase, policy: Policy) {
    state.crouch.phase = phase;
    state.crouch.start_fraction = state.crouch.fraction;
    state.crouch.elapsed = 0.0;
    state.crouch.duration = if phase == CrouchPhase::Ducking {
        policy.duck_duration * (1.0 - state.crouch.linear_fraction)
    } else {
        policy.unduck_duration * state.crouch.linear_fraction
    };
}

fn advance_crouch(state: &mut State, tick: f32, policy: Policy) {
    match state.crouch.phase {
        CrouchPhase::Ducking => {
            if policy.duck_duration == 0.0 {
                state.crouch.linear_fraction = 1.0;
            } else {
                state.crouch.linear_fraction =
                    (state.crouch.linear_fraction + tick / policy.duck_duration).min(1.0);
            }
            state.crouch.elapsed += tick;
            state.crouch.fraction = simple_spline(state.crouch.linear_fraction);
            if state.crouch.linear_fraction >= 1.0 {
                let origin_delta = sub(policy.crouched_hull.mins, policy.standing_hull.mins);
                state.position = sub(state.position, origin_delta);
                state.crouch = crate::CrouchState::CROUCHED;
            }
        }
        CrouchPhase::Unducking => {
            if policy.unduck_duration == 0.0 {
                state.crouch.linear_fraction = 0.0;
            } else {
                state.crouch.linear_fraction =
                    (state.crouch.linear_fraction - tick / policy.unduck_duration).max(0.0);
            }
            state.crouch.elapsed += tick;
            state.crouch.fraction = simple_spline(state.crouch.linear_fraction);
            if state.crouch.linear_fraction <= 0.0 {
                let origin_delta = sub(policy.crouched_hull.mins, policy.standing_hull.mins);
                state.position = add(state.position, origin_delta);
                state.crouch = crate::CrouchState::STANDING;
            }
        }
        _ => {}
    }
}

fn can_unduck(
    context: &mut QueryContext<'_, impl Tracer>,
    state: State,
    grounded: bool,
    configuration: Configuration,
    policy: Policy,
) -> Result<bool, Error> {
    let destination = if grounded {
        add(
            state.position,
            sub(policy.crouched_hull.mins, policy.standing_hull.mins),
        )
    } else {
        sub(state.position, hull_size_delta(policy))
    };
    let trace = context.trace(
        QueryPurpose::Unduck,
        state.position,
        destination,
        policy.standing_hull,
        configuration.solid_mask,
    )?;
    Ok(!trace.start_solid && !trace.all_solid && trace.fraction == 1.0)
}

fn hull_size_delta(policy: Policy) -> [f32; 3] {
    sub(
        sub(policy.standing_hull.maxs, policy.standing_hull.mins),
        sub(policy.crouched_hull.maxs, policy.crouched_hull.mins),
    )
}

fn update_view(state: &mut State, policy: Policy) {
    for axis in 0..3 {
        state.view_offset[axis] = policy.standing_view[axis]
            + (policy.crouched_view[axis] - policy.standing_view[axis]) * state.crouch.fraction;
    }
}

fn record_crouch_change(before: CrouchPhase, after: CrouchPhase, events: &mut Vec<Event>) {
    if before != after {
        events.push(Event::CrouchChanged {
            from: before,
            to: after,
        });
    }
}

fn crop_crouched_command(state: &State, command: &mut PreparedCommand, policy: Policy) {
    if state.ground.is_some() && state.crouch.uses_crouched_hull() {
        command.forward_move *= policy.crouched_command_factor;
        command.side_move *= policy.crouched_command_factor;
        command.up_move *= policy.crouched_command_factor;
    }
}

fn update_ladder(
    context: &mut QueryContext<'_, impl Tracer>,
    state: &mut State,
    input: StepInput,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    if state.mode == Mode::Noclip {
        return Ok(());
    }
    if !configuration.ladders_enabled {
        if state.mode == Mode::Ladder {
            state.mode = Mode::Walk;
            state.ladder_normal = [0.0; 3];
            events.push(Event::LadderDetached);
        }
        return Ok(());
    }

    let wish_direction = if state.mode == Mode::Ladder {
        scale(state.ladder_normal, -1.0)
    } else {
        let wish = add(
            scale(command.forward, command.forward_move),
            scale(command.right, command.side_move),
        );
        if length_squared(wish) == 0.0 {
            return Ok(());
        }
        normalized(wish)
    };
    let destination = add(
        state.position,
        scale(wish_direction, configuration.ladder_distance),
    );
    let hull = state.active_hull(policy);
    let trace = context.trace(
        QueryPurpose::Ladder,
        state.position,
        destination,
        hull,
        configuration.ladder_mask,
    )?;
    let on_ladder = trace.fraction < 1.0
        && (trace.contents & CONTENTS_LADDER != 0 || context.tracer.surface_climbable(trace.hit));
    if !on_ladder {
        if state.mode == Mode::Ladder {
            state.mode = Mode::Walk;
            state.ladder_normal = [0.0; 3];
            events.push(Event::LadderDetached);
        }
        return Ok(());
    }

    let was_ladder = state.mode == Mode::Ladder;
    state.mode = Mode::Ladder;
    state.move_collision = MoveCollision::Default;
    state.ladder_normal = trace
        .normal
        .ok_or_else(|| Error::new(Operation::Trace, FailureKind::Missing, "ladder-plane"))?;
    if !was_ladder {
        events.push(Event::LadderAttached);
    }

    let floor_point = [
        state.position[0],
        state.position[1],
        state.position[2] + hull.mins[2] - 1.0,
    ];
    let floor_contents = context.point(PointQueryPurpose::LadderFloor, floor_point)?;
    let on_floor = floor_contents == 1 || state.ground.is_some();
    let forward_speed = command.forward_move.signum() * configuration.ladder_speed;
    let right_speed = command.side_move.signum() * configuration.ladder_speed;
    if input.command.jump {
        state.mode = Mode::Walk;
        state.move_collision = MoveCollision::Default;
        state.velocity = scale(state.ladder_normal, configuration.ladder_jump_speed);
        state.ladder_normal = [0.0; 3];
        events.push(Event::LadderDetached);
        return Ok(());
    }
    if forward_speed == 0.0 && right_speed == 0.0 {
        state.velocity = [0.0; 3];
        return Ok(());
    }

    let intended = add(
        scale(command.forward, forward_speed),
        scale(
            command.right,
            right_speed * configuration.ladder_lateral_factor,
        ),
    );
    let horizontal_perpendicular = normalized(cross([0.0, 0.0, 1.0], state.ladder_normal));
    let into_speed = dot(intended, state.ladder_normal);
    let into = scale(state.ladder_normal, into_speed);
    let lateral = sub(intended, into);
    let ladder_up = cross(state.ladder_normal, horizontal_perpendicular);
    state.velocity = add(lateral, scale(ladder_up, -into_speed));
    if on_floor && into_speed > 0.0 {
        state.velocity = add(
            state.velocity,
            scale(state.ladder_normal, configuration.ladder_speed),
        );
    }
    Ok(())
}

fn full_ladder(
    context: &mut QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) -> Result<(), Error> {
    check_water(context, &mut result.state, configuration, policy)?;
    let hull = result.state.active_hull(policy);
    let total_velocity = add(result.state.velocity, result.state.base_velocity);
    let movement = slide(
        context,
        result.state.position,
        total_velocity,
        hull,
        configuration,
        tick,
        false,
        result.state.ground.is_none(),
        QueryPurpose::Displacement,
        surface_friction(result.state, policy),
    )?;
    result.state.position = movement.position;
    result.state.velocity = sub(movement.velocity, result.state.base_velocity);
    result.contacts.extend(movement.contacts);
    if movement.trapped {
        result.state.velocity = [0.0; 3];
        result.events.push(Event::Trapped);
    }
    Ok(())
}

fn full_walk(
    context: &mut QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    input: StepInput,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) -> Result<(), Error> {
    let swimming = if policy.water.refresh_before_walk {
        check_water(context, &mut result.state, configuration, policy)?
    } else {
        result.state.water_level > 1
    };
    if !swimming {
        start_gravity(&mut result.state, configuration, tick);
    }

    if result.state.water_jump_time_ms != 0.0 {
        water_jump(&mut result.state, configuration, tick);
        let hull = result.state.active_hull(policy);
        let movement = slide(
            context,
            result.state.position,
            result.state.velocity,
            hull,
            configuration,
            tick,
            false,
            true,
            QueryPurpose::Displacement,
            surface_friction(result.state, policy),
        )?;
        apply_move_outcome(result, movement);
        check_water(context, &mut result.state, configuration, policy)?;
        return Ok(());
    }

    if result.state.water_level >= 2 {
        if result.state.water_level == 2 {
            check_water_exit(context, result, input, command, configuration, policy)?;
        }
        if result.state.velocity[2] < 0.0 && result.state.water_jump_time_ms != 0.0 {
            result.state.water_jump_time_ms = 0.0;
        }
        check_jump(context, result, input, configuration, policy, tick)?;
        water_move(context, result, input, command, configuration, policy, tick)?;
        categorize_position(
            context,
            &mut result.state,
            configuration,
            policy,
            &mut result.contacts,
            &mut result.events,
        )?;
        if result.state.ground.is_some() {
            result.state.velocity[2] = 0.0;
        }
        return Ok(());
    }

    check_jump(context, result, input, configuration, policy, tick)?;
    let movement_surface_friction = surface_friction(result.state, policy);
    if result.state.ground.is_some() {
        result.state.velocity[2] = 0.0;
        let friction_wish = ground_friction(
            &mut result.state.velocity,
            configuration,
            movement_surface_friction,
            result.state.water_jump_time_ms,
            tick,
        );
        result.wish_velocity = add(result.wish_velocity, friction_wish);
    }
    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);

    let wish = horizontal_wish(command);
    result.wish_state = wish;
    if result.state.ground.is_some() {
        accelerate(
            &mut result.state.velocity,
            wish.direction,
            wish.speed,
            configuration.acceleration,
            tick,
            movement_surface_friction,
        );
        result.wish_velocity = add(result.wish_velocity, scale(wish.direction, wish.speed));
        cap_horizontal_speed(&mut result.state.velocity, command.maximum_speed);
        let flat_forward = normalized([command.forward[0], command.forward[1], 0.0]);
        let flat_right = normalized([command.right[0], command.right[1], 0.0]);
        clamp_backward_speed(
            &mut result.state.velocity,
            flat_forward,
            flat_right,
            command.maximum_speed,
            policy,
        );
    } else {
        let applied = air_accelerate(
            &mut result.state.velocity,
            wish.direction,
            wish.speed,
            configuration.air_acceleration,
            policy.air_speed_cap,
            tick,
            movement_surface_friction,
        );
        result.wish_velocity = add(result.wish_velocity, applied);
    }

    let hull = result.state.active_hull(policy);
    let grounded_before_move = result.state.ground.is_some();
    let total_velocity = add(result.state.velocity, result.state.base_velocity);
    let movement = if grounded_before_move {
        walk_move(
            context,
            result.state.position,
            total_velocity,
            hull,
            configuration,
            policy.step_strategy,
            tick,
            movement_surface_friction,
        )?
    } else {
        slide(
            context,
            result.state.position,
            total_velocity,
            hull,
            configuration,
            tick,
            true,
            true,
            QueryPurpose::Displacement,
            movement_surface_friction,
        )?
    };
    apply_move_outcome(result, movement);
    result.state.velocity = sub(result.state.velocity, result.state.base_velocity);

    let was_grounded = result.state.ground.is_some();
    categorize_position(
        context,
        &mut result.state,
        configuration,
        policy,
        &mut result.contacts,
        &mut result.events,
    )?;
    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);
    let swimming = if policy.water.refresh_before_walk {
        check_water(context, &mut result.state, configuration, policy)?
    } else {
        result.state.water_level > 1
    };
    if !swimming {
        finish_gravity(&mut result.state, configuration, tick);
    }
    if result.state.ground.is_some() && result.state.velocity[2] < 0.0 {
        result.state.velocity[2] = 0.0;
    }
    if result.state.ground.is_none() && result.state.velocity[2] < 0.0 {
        result.state.fall_speed = result.state.fall_speed.max(-result.state.velocity[2]);
    } else if !was_grounded && result.state.ground.is_some() {
        land(context, result, configuration)?;
    }
    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);
    Ok(())
}

fn start_gravity(state: &mut State, configuration: Configuration, tick: f32) {
    state.velocity[2] -= configuration.gravity * configuration.gravity_scale * tick * 0.5;
    state.velocity[2] += state.base_velocity[2] * tick;
    state.base_velocity[2] = 0.0;
    clamp_velocity(&mut state.velocity, configuration.maximum_velocity);
}

fn finish_gravity(state: &mut State, configuration: Configuration, tick: f32) {
    if state.water_jump_time_ms != 0.0 {
        return;
    }
    state.velocity[2] -= configuration.gravity * configuration.gravity_scale * tick * 0.5;
    clamp_velocity(&mut state.velocity, configuration.maximum_velocity);
}

fn add_gravity(state: &mut State, configuration: Configuration, tick: f32) {
    if state.water_jump_time_ms != 0.0 {
        return;
    }
    state.velocity[2] -= configuration.gravity * configuration.gravity_scale * tick;
    state.velocity[2] += state.base_velocity[2] * tick;
    state.base_velocity[2] = 0.0;
    clamp_velocity(&mut state.velocity, configuration.maximum_velocity);
}

fn check_jump(
    context: &QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) -> Result<(), Error> {
    if !input.command.jump {
        result.state.jump_latched = false;
        return Ok(());
    }
    if result.state.water_jump_time_ms != 0.0 {
        return Ok(());
    }
    if result.state.water_level >= 2 {
        set_ground(
            context,
            &mut result.state,
            None,
            &mut result.contacts,
            &mut result.events,
        )?;
        if result.state.water_type == CONTENTS_WATER {
            result.state.velocity[2] = configuration.water_swim_speed;
        } else if result.state.water_type == CONTENTS_SLIME {
            result.state.velocity[2] = configuration.slime_swim_speed;
        }
        return Ok(());
    }
    if result.state.ground.is_none() {
        result.state.jump_latched = true;
        return Ok(());
    }
    if result.state.jump_latched
        || !policy.allow_jump
        || (!policy.allow_crouched_jump && result.state.crouch.uses_crouched_hull())
        || result.state.crouch.phase == CrouchPhase::Unducking
    {
        return Ok(());
    }
    if let Some(cap) = policy.bunnyhop_speed_cap {
        cap_vector_speed(&mut result.state.velocity, cap);
    }
    set_ground(
        context,
        &mut result.state,
        None,
        &mut result.contacts,
        &mut result.events,
    )?;
    let starting_z = result.state.velocity[2];
    let impulse = policy.jump_impulse * policy.surface_jump_factor;
    if policy.replace_vertical_while_ducking && result.state.crouch.phase == CrouchPhase::Ducking {
        result.state.velocity[2] = impulse;
    } else {
        result.state.velocity[2] += impulse;
    }
    finish_gravity(&mut result.state, configuration, tick);
    result.jump_velocity[2] += result.state.velocity[2] - starting_z;
    result.climbed_step += 0.15;
    result.state.jump_latched = true;
    result.events.push(Event::Jumped);
    Ok(())
}

fn water_jump(state: &mut State, configuration: Configuration, tick: f32) {
    state.water_jump_time_ms = state
        .water_jump_time_ms
        .min(configuration.water_exit_maximum_ms);
    if state.water_jump_time_ms == 0.0 {
        return;
    }
    state.water_jump_time_ms -= 1_000.0 * tick;
    if state.water_jump_time_ms <= 0.0 || state.water_level == 0 {
        state.water_jump_time_ms = 0.0;
        state.water_jump_velocity = [0.0; 3];
    }
    state.velocity[0] = state.water_jump_velocity[0];
    state.velocity[1] = state.water_jump_velocity[1];
}

fn check_water_exit(
    context: &mut QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    input: StepInput,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
) -> Result<(), Error> {
    if result.state.water_jump_time_ms != 0.0 || result.state.velocity[2] < -180.0 {
        return Ok(());
    }
    let mut flat_velocity = [result.state.velocity[0], result.state.velocity[1], 0.0];
    let current_speed = normalize_in_place(&mut flat_velocity);
    let mut flat_forward = if policy.water.ledge_uses_command_direction {
        [
            command.forward[0] * command.forward_move + command.right[0] * command.side_move,
            command.forward[1] * command.forward_move + command.right[1] * command.side_move,
            0.0,
        ]
    } else {
        [command.forward[0], command.forward[1], 0.0]
    };
    normalize_in_place(&mut flat_forward);
    if current_speed != 0.0
        && dot(flat_velocity, flat_forward) < 0.0
        && !(policy.water.ledge_jump_overrides_backward && input.command.jump)
    {
        return Ok(());
    }

    let hull = result.state.active_hull(policy);
    let waist_start = add(result.state.position, scale(add(hull.mins, hull.maxs), 0.5));
    let waist_end = add(
        waist_start,
        scale(flat_forward, configuration.water_exit_forward),
    );
    let waist = context.trace(
        QueryPurpose::WaterWaist,
        waist_start,
        waist_end,
        hull,
        configuration.solid_mask,
    )?;
    if waist.fraction >= 1.0 {
        return Ok(());
    }
    let waist_normal = waist
        .normal
        .ok_or_else(|| Error::new(Operation::Trace, FailureKind::Missing, "water-waist-plane"))?;
    let eye_start = [
        waist_start[0],
        waist_start[1],
        result.state.position[2] + result.state.view_offset[2] + configuration.water_exit_height,
    ];
    let eye_end = add(
        eye_start,
        scale(flat_forward, configuration.water_exit_forward),
    );
    let eye = context.trace(
        QueryPurpose::WaterEye,
        eye_start,
        eye_end,
        hull,
        configuration.solid_mask,
    )?;
    if eye.fraction != 1.0 || eye.start_solid || eye.all_solid {
        return Ok(());
    }
    let landing_end = [
        eye_end[0],
        eye_end[1],
        eye_end[2] - configuration.water_exit_down,
    ];
    let landing = context.trace(
        QueryPurpose::WaterLanding,
        eye_end,
        landing_end,
        hull,
        configuration.solid_mask,
    )?;
    if !standable(landing, configuration.standable_normal) {
        return Ok(());
    }
    result.state.water_jump_velocity = scale(waist_normal, -configuration.water_exit_push_speed);
    result.state.velocity[2] = configuration.water_exit_up_speed;
    result.state.water_jump_time_ms = configuration.water_exit_duration_ms;
    result.state.jump_latched = true;
    set_ground(
        context,
        &mut result.state,
        None,
        &mut result.contacts,
        &mut result.events,
    )
}

fn water_move(
    context: &mut QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    input: StepInput,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) -> Result<(), Error> {
    let mut wish_velocity = add(
        scale(command.forward, command.forward_move),
        scale(command.right, command.side_move),
    );
    let client_max_speed = if configuration.client_max_speed != 0.0 {
        configuration.client_max_speed
    } else {
        policy.maximum_speed
    };
    if input.command.jump {
        if policy.water.jump_wish_at_waist || result.state.water_level >= 3 {
            wish_velocity[2] += client_max_speed;
        }
    } else if command.forward_move == 0.0 && command.side_move == 0.0 && command.up_move == 0.0 {
        wish_velocity[2] -= configuration.water_idle_sink_speed;
    } else {
        let pitched_up = if policy.water.amplify_forward_pitch {
            (command.forward_move * command.forward[2] * 2.0).clamp(0.0, client_max_speed)
        } else {
            0.0
        };
        wish_velocity[2] += command.up_move + pitched_up;
    }
    let uncapped = length(wish_velocity);
    let direction = normalized(wish_velocity);
    let clamped = uncapped.min(command.maximum_speed);
    if uncapped > command.maximum_speed && uncapped > 0.0 {
        wish_velocity = scale(wish_velocity, command.maximum_speed / uncapped);
    }
    let wish_speed = clamped * configuration.water_wish_speed_factor;
    result.wish_state = WishState {
        direction,
        speed: wish_speed,
        uncapped_speed: uncapped,
    };

    let speed = length(result.state.velocity);
    let new_speed = if speed != 0.0 {
        let next =
            speed - tick * speed * configuration.friction * surface_friction(result.state, policy);
        let next = if next < 0.1 { 0.0 } else { next };
        result.state.velocity = scale(result.state.velocity, next / speed);
        next
    } else {
        0.0
    };
    if wish_speed >= 0.1 {
        let add_speed = wish_speed - new_speed;
        if add_speed > 0.0 {
            let acceleration = (configuration.acceleration
                * wish_speed
                * tick
                * surface_friction(result.state, policy))
            .min(add_speed);
            let delta = scale(normalized(wish_velocity), acceleration);
            result.state.velocity = add(result.state.velocity, delta);
            result.wish_velocity = add(result.wish_velocity, delta);
        }
    }

    let hull = result.state.active_hull(policy);
    let total_velocity = add(result.state.velocity, result.state.base_velocity);
    let destination = add(result.state.position, scale(total_velocity, tick));
    let direct = context.trace(
        QueryPurpose::Displacement,
        result.state.position,
        destination,
        hull,
        configuration.solid_mask,
    )?;
    let movement = if direct.fraction == 1.0 {
        let mut elevated = destination;
        if configuration.allow_auto_movement {
            elevated[2] += configuration.step_height + 1.0;
        }
        let down = context.trace(
            QueryPurpose::StepDown,
            elevated,
            destination,
            hull,
            configuration.solid_mask,
        )?;
        if !down.start_solid && !down.all_solid {
            MoveOutcome {
                position: down.end,
                velocity: total_velocity,
                contacts: Vec::new(),
                climbed: down.end[2] - result.state.position[2],
                trapped: false,
            }
        } else {
            slide(
                context,
                result.state.position,
                total_velocity,
                hull,
                configuration,
                tick,
                false,
                result.state.ground.is_none(),
                QueryPurpose::Displacement,
                surface_friction(result.state, policy),
            )?
        }
    } else if result.state.ground.is_none() {
        slide(
            context,
            result.state.position,
            total_velocity,
            hull,
            configuration,
            tick,
            false,
            true,
            QueryPurpose::Displacement,
            surface_friction(result.state, policy),
        )?
    } else {
        step_move(
            context,
            result.state.position,
            total_velocity,
            hull,
            configuration,
            policy.step_strategy,
            tick,
            surface_friction(result.state, policy),
        )?
    };
    apply_move_outcome(result, movement);
    result.state.velocity = sub(result.state.velocity, result.state.base_velocity);
    Ok(())
}

fn check_water(
    context: &mut QueryContext<'_, impl Tracer>,
    state: &mut State,
    configuration: Configuration,
    policy: Policy,
) -> Result<bool, Error> {
    let hull = state.active_hull(policy);
    let center_x = state.position[0] + (hull.mins[0] + hull.maxs[0]) * 0.5;
    let center_y = state.position[1] + (hull.mins[1] + hull.maxs[1]) * 0.5;
    let feet = [center_x, center_y, state.position[2] + hull.mins[2] + 1.0];
    state.water_level = 0;
    state.water_type = 0;
    let mut contents = context.point(PointQueryPurpose::WaterFeet, feet)?;
    if contents & configuration.water_mask == 0 {
        return Ok(false);
    }

    state.water_type = contents & (CONTENTS_WATER | CONTENTS_SLIME);
    state.water_level = 1;
    let waist = [
        center_x,
        center_y,
        state.position[2] + (hull.mins[2] + hull.maxs[2]) * 0.5 + policy.water.waist_height_offset,
    ];
    let eyes = [center_x, center_y, state.position[2] + state.view_offset[2]];

    match policy.water.sampling {
        WaterSampling::WaistThenEyes => {
            contents = context.point(PointQueryPurpose::WaterWaist, waist)?;
            if contents & configuration.water_mask != 0 {
                state.water_level = 2;
                contents = context.point(PointQueryPurpose::WaterEyes, eyes)?;
                if contents & configuration.water_mask != 0 {
                    state.water_level = 3;
                }
            }
        }
        WaterSampling::EyesThenWaist => {
            contents = context.point(PointQueryPurpose::WaterEyes, eyes)?;
            if contents & configuration.water_mask != 0 {
                state.water_level = 3;
            } else {
                contents = context.point(PointQueryPurpose::WaterWaist, waist)?;
                if contents & configuration.water_mask != 0 {
                    state.water_level = 2;
                }
            }
        }
    }

    if policy.water.apply_currents && contents & MASK_CURRENT != 0 {
        let mut current = [0.0; 3];
        if contents & CONTENTS_CURRENT_0 != 0 {
            current[0] += 1.0;
        }
        if contents & CONTENTS_CURRENT_90 != 0 {
            current[1] += 1.0;
        }
        if contents & CONTENTS_CURRENT_180 != 0 {
            current[0] -= 1.0;
        }
        if contents & CONTENTS_CURRENT_270 != 0 {
            current[1] -= 1.0;
        }
        if contents & CONTENTS_CURRENT_UP != 0 {
            current[2] += 1.0;
        }
        if contents & CONTENTS_CURRENT_DOWN != 0 {
            current[2] -= 1.0;
        }
        state.base_velocity = add(
            state.base_velocity,
            scale(
                current,
                configuration.current_speed_per_level * f32::from(state.water_level),
            ),
        );
    }
    Ok(state.water_level > 1)
}

fn clear_water(state: &mut State) {
    state.water_level = 0;
    state.water_type = 0;
    state.water_jump_time_ms = 0.0;
    state.water_jump_velocity = [0.0; 3];
}

fn categorize_position(
    context: &mut QueryContext<'_, impl Tracer>,
    state: &mut State,
    configuration: Configuration,
    policy: Policy,
    contacts: &mut Vec<Contact>,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    state.surface_friction = 1.0;
    check_water(context, state, configuration, policy)?;
    if state.mode == Mode::Observer {
        return Ok(());
    }
    if state.mode == Mode::Ladder && state.velocity[2] > 0.0 {
        return set_ground(context, state, None, contacts, events);
    }
    let support_velocity_z = if let Some(support) = state.ground.and_then(|ground| ground.support) {
        context.tracer.support_velocity(support)?[2]
    } else {
        0.0
    };
    if state.velocity[2] - support_velocity_z > policy.ground_detach_speed {
        return set_ground(context, state, None, contacts, events);
    }

    let was_grounded = state.ground.is_some();
    let mut distance = configuration.ground_probe;
    let move_to_end = policy.step_strategy == StepStrategy::HighFirst
        && state.mode == Mode::Walk
        && was_grounded
        && state.water_level < 3;
    if move_to_end {
        distance += configuration.step_height;
    }
    let end = [
        state.position[0],
        state.position[1],
        state.position[2] - distance,
    ];
    let hull = state.active_hull(policy);
    let full = context.trace(
        QueryPurpose::GroundFull,
        state.position,
        end,
        hull,
        configuration.solid_mask,
    )?;
    let mut support_trace = standable(full, configuration.standable_normal).then_some(full);
    if support_trace.is_none() {
        for quadrant in quadrants(hull) {
            let trace = context.trace(
                QueryPurpose::GroundQuadrant,
                state.position,
                end,
                quadrant,
                configuration.solid_mask,
            )?;
            if standable(trace, configuration.standable_normal) {
                support_trace = Some(Trace {
                    fraction: full.fraction,
                    end: full.end,
                    ..trace
                });
                break;
            }
        }
    }
    let Some(trace) = support_trace else {
        if state.velocity[2] > 0.0 && state.mode != Mode::Noclip {
            state.surface_friction = 0.25;
        }
        return set_ground(context, state, None, contacts, events);
    };
    if move_to_end
        && !trace.start_solid
        && trace.fraction > 0.0
        && trace.fraction < 1.0
        && (state.position[2] - trace.end[2]).abs() > configuration.ground_network_snap
    {
        state.position[2] = trace.end[2];
    }
    let ground = GroundState {
        support: trace.hit,
        normal: trace.normal.expect("standable traces have planes"),
        surface_friction: policy.surface_friction,
    };
    state.surface_friction = policy.surface_friction;
    set_ground(context, state, Some(ground), contacts, events)
}

fn set_ground(
    context: &QueryContext<'_, impl Tracer>,
    state: &mut State,
    ground: Option<GroundState>,
    _contacts: &mut Vec<Contact>,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    let old_support = state.ground.and_then(|value| value.support);
    let new_support = ground.and_then(|value| value.support);
    if state.ground.is_none() && ground.is_some() {
        let support_velocity = if let Some(support) = new_support {
            context.tracer.support_velocity(support)?
        } else {
            [0.0; 3]
        };
        state.base_velocity = sub(state.base_velocity, support_velocity);
        state.base_velocity[2] = support_velocity[2];
    } else if state.ground.is_some() && ground.is_none() {
        let support_velocity = if let Some(support) = old_support {
            context.tracer.support_velocity(support)?
        } else {
            [0.0; 3]
        };
        state.base_velocity = add(state.base_velocity, support_velocity);
        state.base_velocity[2] = support_velocity[2];
    }
    let changed = state.ground.is_some() != ground.is_some() || old_support != new_support;
    state.ground = ground;
    if ground.is_some() {
        state.water_jump_time_ms = 0.0;
        state.velocity[2] = 0.0;
    }
    if changed {
        events.push(Event::GroundChanged {
            from: old_support,
            to: new_support,
        });
    }
    Ok(())
}

fn land(
    context: &QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    configuration: Configuration,
) -> Result<(), Error> {
    let mut speed = result.state.fall_speed;
    if result.state.water_level == 0
        && speed >= configuration.fall_punch_threshold
        && let Some(support) = result.state.ground.and_then(|ground| ground.support)
    {
        if context.tracer.support_is_floating(support)? {
            speed -= configuration.floating_fall_reduction;
        }
        let support_z = context.tracer.support_velocity(support)?[2];
        if support_z < 0.0 {
            speed = (speed + support_z).max(0.1);
        }
    }
    result.events.push(Event::Landed { fall_speed: speed });
    result.state.fall_speed = 0.0;
    Ok(())
}

fn horizontal_wish(command: PreparedCommand) -> WishState {
    let forward = normalized([command.forward[0], command.forward[1], 0.0]);
    let right = normalized([command.right[0], command.right[1], 0.0]);
    let wish_velocity = add(
        scale(forward, command.forward_move),
        scale(right, command.side_move),
    );
    let uncapped = length(wish_velocity);
    WishState {
        direction: normalized(wish_velocity),
        speed: uncapped.min(command.maximum_speed),
        uncapped_speed: uncapped,
    }
}

fn ground_friction(
    velocity: &mut [f32; 3],
    configuration: Configuration,
    surface_friction: f32,
    water_jump_time_ms: f32,
    tick: f32,
) -> [f32; 3] {
    if water_jump_time_ms != 0.0 {
        return [0.0; 3];
    }
    let speed = length(*velocity);
    if speed < 0.1 {
        return [0.0; 3];
    }
    let control = speed.max(configuration.stop_speed);
    let drop = control * configuration.friction * surface_friction * tick;
    let ratio = (speed - drop).max(0.0) / speed;
    *velocity = scale(*velocity, ratio);
    scale(*velocity, -(1.0 - ratio))
}

fn accelerate(
    velocity: &mut [f32; 3],
    direction: [f32; 3],
    wish_speed: f32,
    acceleration: f32,
    tick: f32,
    surface_friction: f32,
) -> [f32; 3] {
    let add_speed = wish_speed - dot(*velocity, direction);
    if wish_speed == 0.0 || add_speed <= 0.0 {
        return [0.0; 3];
    }
    let acceleration_speed = (acceleration * tick * wish_speed * surface_friction).min(add_speed);
    let delta = scale(direction, acceleration_speed);
    *velocity = add(*velocity, delta);
    delta
}

fn air_accelerate(
    velocity: &mut [f32; 3],
    direction: [f32; 3],
    uncapped_wish_speed: f32,
    acceleration: f32,
    air_cap: f32,
    tick: f32,
    surface_friction: f32,
) -> [f32; 3] {
    let capped = uncapped_wish_speed.min(air_cap);
    let add_speed = capped - dot(*velocity, direction);
    if uncapped_wish_speed == 0.0 || add_speed <= 0.0 {
        return [0.0; 3];
    }
    let acceleration_speed =
        (acceleration * tick * uncapped_wish_speed * surface_friction).min(add_speed);
    let delta = scale(direction, acceleration_speed);
    *velocity = add(*velocity, delta);
    delta
}

fn surface_friction(state: State, policy: Policy) -> f32 {
    if state.surface_friction.is_finite() {
        state.surface_friction
    } else {
        policy.surface_friction
    }
}

fn clamp_backward_speed(
    velocity: &mut [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    maximum_speed: f32,
    policy: Policy,
) {
    if policy.backward_speed_factor >= 1.0 || length(*velocity) <= policy.backward_speed_minimum {
        return;
    }
    let forward_speed = dot(*velocity, forward);
    if forward_speed >= 0.0 {
        return;
    }
    let mut backward = scale(forward, forward_speed);
    let lateral = scale(right, dot(*velocity, right));
    let speed = length(backward);
    let maximum_backward = maximum_speed * policy.backward_speed_factor;
    if speed > maximum_backward {
        backward = scale(backward, maximum_backward / speed);
    }
    let z = velocity[2];
    *velocity = add(backward, lateral);
    velocity[2] = z;
    cap_horizontal_speed(velocity, maximum_speed);
}

#[derive(Clone, Debug)]
struct MoveOutcome {
    position: [f32; 3],
    velocity: [f32; 3],
    contacts: Vec<Contact>,
    climbed: f32,
    trapped: bool,
}

fn apply_move_outcome(result: &mut StepResult, movement: MoveOutcome) {
    result.state.position = movement.position;
    result.state.velocity = movement.velocity;
    result.contacts.extend(movement.contacts);
    if movement.trapped {
        result.state.velocity = [0.0; 3];
        result.events.push(Event::Trapped);
    }
    if movement.climbed > 0.0 {
        result.climbed_step += movement.climbed;
        result.events.push(Event::Stepped {
            height: movement.climbed,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_move(
    context: &mut QueryContext<'_, impl Tracer>,
    start: [f32; 3],
    velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
    strategy: StepStrategy,
    tick: f32,
    surface_friction: f32,
) -> Result<MoveOutcome, Error> {
    if length(velocity) < 1.0 {
        return Ok(MoveOutcome {
            position: start,
            velocity: [0.0; 3],
            contacts: Vec::new(),
            climbed: 0.0,
            trapped: false,
        });
    }
    let destination = [
        start[0] + velocity[0] * tick,
        start[1] + velocity[1] * tick,
        start[2],
    ];
    let direct = context.trace(
        QueryPurpose::Displacement,
        start,
        destination,
        hull,
        configuration.solid_mask,
    )?;
    if direct.all_solid {
        return Ok(MoveOutcome {
            position: start,
            velocity: [0.0; 3],
            contacts: Vec::new(),
            climbed: 0.0,
            trapped: true,
        });
    }
    if direct.fraction == 1.0 {
        let endpoint = context.trace(
            QueryPurpose::Endpoint,
            direct.end,
            direct.end,
            hull,
            configuration.solid_mask,
        )?;
        if endpoint.start_solid || endpoint.fraction != 1.0 {
            return Ok(MoveOutcome {
                position: start,
                velocity: [0.0; 3],
                contacts: Vec::new(),
                climbed: 0.0,
                trapped: true,
            });
        }
        let grounded = stay_on_ground(context, direct.end, hull, configuration)?;
        return Ok(MoveOutcome {
            position: grounded,
            velocity,
            contacts: Vec::new(),
            climbed: 0.0,
            trapped: false,
        });
    }
    let mut outcome = step_move(
        context,
        start,
        velocity,
        hull,
        configuration,
        strategy,
        tick,
        surface_friction,
    )?;
    outcome.position = stay_on_ground(context, outcome.position, hull, configuration)?;
    Ok(outcome)
}

fn stay_on_ground(
    context: &mut QueryContext<'_, impl Tracer>,
    position: [f32; 3],
    hull: Hull,
    configuration: Configuration,
) -> Result<[f32; 3], Error> {
    let up_end = [
        position[0],
        position[1],
        position[2] + configuration.stay_ground_rise,
    ];
    let up = context.trace(
        QueryPurpose::StayGroundUp,
        position,
        up_end,
        hull,
        configuration.solid_mask,
    )?;
    let down_end = [
        position[0],
        position[1],
        position[2] - configuration.step_height,
    ];
    let down = context.trace(
        QueryPurpose::StayGroundDown,
        up.end,
        down_end,
        hull,
        configuration.solid_mask,
    )?;
    if down.fraction > 0.0
        && down.fraction < 1.0
        && !down.start_solid
        && down
            .normal
            .is_some_and(|normal| normal[2] >= configuration.standable_normal)
        && (position[2] - down.end[2]).abs() > configuration.ground_network_snap
    {
        Ok(down.end)
    } else {
        Ok(position)
    }
}

#[allow(clippy::too_many_arguments)]
fn step_move(
    context: &mut QueryContext<'_, impl Tracer>,
    start: [f32; 3],
    velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
    strategy: StepStrategy,
    tick: f32,
    surface_friction: f32,
) -> Result<MoveOutcome, Error> {
    match strategy {
        StepStrategy::FurthestHorizontal => {
            let low = slide(
                context,
                start,
                velocity,
                hull,
                configuration,
                tick,
                true,
                false,
                QueryPurpose::Displacement,
                surface_friction,
            )?;
            let Some(mut high) = high_step(
                context,
                start,
                velocity,
                hull,
                configuration,
                tick,
                surface_friction,
                true,
            )?
            else {
                return Ok(low);
            };
            if horizontal_distance_squared(start, high.position)
                >= horizontal_distance_squared(start, low.position)
            {
                high.velocity[2] = low.velocity[2];
                Ok(high)
            } else {
                Ok(low)
            }
        }
        StepStrategy::HighFirst => {
            if let Some(high) = high_step(
                context,
                start,
                velocity,
                hull,
                configuration,
                tick,
                surface_friction,
                false,
            )? {
                return Ok(high);
            }
            slide(
                context,
                start,
                velocity,
                hull,
                configuration,
                tick,
                true,
                false,
                QueryPurpose::Displacement,
                surface_friction,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn high_step(
    context: &mut QueryContext<'_, impl Tracer>,
    start: [f32; 3],
    velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
    tick: f32,
    surface_friction: f32,
    require_standable: bool,
) -> Result<Option<MoveOutcome>, Error> {
    if !configuration.allow_auto_movement {
        return Ok(None);
    }
    let distance = configuration.step_height + configuration.step_epsilon;
    let up_end = [start[0], start[1], start[2] + distance];
    let up = context.trace(
        QueryPurpose::StepUp,
        start,
        up_end,
        hull,
        configuration.solid_mask,
    )?;
    if up.start_solid || up.all_solid {
        return Ok(None);
    }
    let mut moved = slide(
        context,
        up.end,
        velocity,
        hull,
        configuration,
        tick,
        true,
        false,
        QueryPurpose::Displacement,
        surface_friction,
    )?;
    if moved.trapped || moved.position == start {
        return Ok(None);
    }
    let down_end = [
        moved.position[0],
        moved.position[1],
        moved.position[2] - distance,
    ];
    let down = context.trace(
        QueryPurpose::StepDown,
        moved.position,
        down_end,
        hull,
        configuration.solid_mask,
    )?;
    if require_standable && !standable(down, configuration.standable_normal) {
        return Ok(None);
    }
    if !require_standable
        && down.fraction != 1.0
        && down
            .normal
            .is_some_and(|normal| normal[2] < configuration.standable_normal)
    {
        return Ok(None);
    }
    if !down.start_solid && !down.all_solid {
        moved.position = down.end;
    }
    moved.climbed = (moved.position[2] - start[2]).max(0.0);
    Ok(Some(moved))
}

#[allow(clippy::too_many_arguments)]
fn slide(
    context: &mut QueryContext<'_, impl Tracer>,
    start: [f32; 3],
    mut velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
    tick: f32,
    walk_mode: bool,
    airborne: bool,
    purpose: QueryPurpose,
    surface_friction: f32,
) -> Result<MoveOutcome, Error> {
    let primal_velocity = velocity;
    let mut original_velocity = velocity;
    let mut position = start;
    let mut remaining = tick;
    let mut fraction_sum = 0.0;
    let mut planes = Vec::with_capacity(MAX_CLIP_PLANES);
    let mut contacts = Vec::new();

    for _ in 0..MAX_BUMPS {
        if length(velocity) == 0.0 || remaining <= 0.0 {
            break;
        }
        let end = add(position, scale(velocity, remaining));
        let trace = context.trace(purpose, position, end, hull, configuration.solid_mask)?;
        fraction_sum += trace.fraction;
        if trace.all_solid {
            return Ok(MoveOutcome {
                position,
                velocity: [0.0; 3],
                contacts,
                climbed: 0.0,
                trapped: true,
            });
        }
        if trace.fraction > 0.0 {
            if trace.fraction == 1.0 {
                let endpoint = context.trace(
                    QueryPurpose::Endpoint,
                    trace.end,
                    trace.end,
                    hull,
                    configuration.solid_mask,
                )?;
                if endpoint.start_solid || endpoint.fraction != 1.0 {
                    velocity = [0.0; 3];
                    break;
                }
            }
            position = trace.end;
            original_velocity = velocity;
            planes.clear();
        }
        if trace.fraction == 1.0 {
            break;
        }
        let Some(normal) = trace.normal else {
            velocity = [0.0; 3];
            break;
        };
        record_contact_from_parts(&mut contacts, trace.hit, normal, velocity);
        remaining -= remaining * trace.fraction;
        if planes.len() >= MAX_CLIP_PLANES {
            velocity = [0.0; 3];
            break;
        }
        planes.push(normal);

        if planes.len() == 1 && walk_mode && airborne {
            let overbounce = if normal[2] > configuration.standable_normal {
                1.0
            } else {
                1.0 + configuration.bounce * (1.0 - surface_friction)
            };
            velocity = clip_velocity(original_velocity, normal, overbounce);
            original_velocity = velocity;
            continue;
        }

        let mut accepted = None;
        let mut last_candidate = velocity;
        for (index, plane) in planes.iter().enumerate() {
            let candidate = clip_velocity(original_velocity, *plane, 1.0);
            last_candidate = candidate;
            if planes
                .iter()
                .enumerate()
                .all(|(other_index, other)| other_index == index || dot(candidate, *other) >= 0.0)
            {
                accepted = Some(candidate);
                break;
            }
        }
        velocity = if let Some(candidate) = accepted {
            candidate
        } else if planes.len() == 2 {
            let direction = normalized(cross(planes[0], planes[1]));
            scale(direction, dot(direction, last_candidate))
        } else {
            [0.0; 3]
        };
        if dot(velocity, primal_velocity) <= 0.0 {
            velocity = [0.0; 3];
            break;
        }
    }
    if fraction_sum == 0.0 {
        velocity = [0.0; 3];
    }
    Ok(MoveOutcome {
        position,
        velocity,
        contacts,
        climbed: 0.0,
        trapped: false,
    })
}

fn noclip(
    result: &mut StepResult,
    input: StepInput,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) {
    let maximum_speed = configuration.server_max_speed * configuration.noclip_speed;
    let factor = if input.speed_button {
        configuration.noclip_speed * 0.5
    } else {
        configuration.noclip_speed
    };
    let mut wish_velocity = add(
        scale(command.forward, command.forward_move * factor),
        scale(command.right, command.side_move * factor),
    );
    wish_velocity[2] += command.up_move * factor;
    let uncapped = length(wish_velocity);
    let direction = normalized(wish_velocity);
    let speed = uncapped.min(maximum_speed);
    wish_velocity = scale(direction, speed);
    result.wish_state = WishState {
        direction,
        speed,
        uncapped_speed: uncapped,
    };
    result.wish_velocity = wish_velocity;

    let movement_surface_friction = surface_friction(result.state, policy);
    if configuration.noclip_acceleration > 0.0 {
        accelerate(
            &mut result.state.velocity,
            direction,
            speed,
            configuration.noclip_acceleration,
            tick,
            movement_surface_friction,
        );
        let current = length(result.state.velocity);
        if current < 1.0 {
            result.state.velocity = [0.0; 3];
            return;
        }
        let control = current.max(maximum_speed * 0.25);
        let drop = control * configuration.friction * movement_surface_friction * tick;
        result.state.velocity = scale(result.state.velocity, (current - drop).max(0.0) / current);
    } else {
        result.state.velocity = wish_velocity;
    }
    result.state.position = add(result.state.position, scale(result.state.velocity, tick));
    if configuration.noclip_acceleration < 0.0 {
        result.state.velocity = [0.0; 3];
    }
}

fn observer_move(
    context: &mut QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    input: StepInput,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) -> Result<(), Error> {
    match result.state.observer_mode {
        ObserverMode::InEye | ObserverMode::Chase | ObserverMode::PointOfInterest => {
            if let Some(target) = result.state.observer_target
                && let Some(target) = context.tracer.observer_target(target)?
            {
                result.state.position = target.position;
                result.state.absolute_view_angles = target.angles;
                result.state.velocity = target.velocity;
            }
            Ok(())
        }
        ObserverMode::Roaming if configuration.spectator_noclip => {
            let noclip_configuration = Configuration {
                noclip_speed: configuration.spectator_speed,
                noclip_acceleration: configuration.spectator_acceleration,
                ..configuration
            };
            noclip(result, input, command, noclip_configuration, policy, tick);
            Ok(())
        }
        ObserverMode::Roaming => {
            let factor = if input.speed_button {
                configuration.spectator_speed * 0.5
            } else {
                configuration.spectator_speed
            };
            let mut wish_velocity = add(
                scale(command.forward, command.forward_move * factor),
                scale(command.right, command.side_move * factor),
            );
            wish_velocity[2] += command.up_move;
            let uncapped = length(wish_velocity);
            let direction = normalized(wish_velocity);
            let speed = uncapped.min(configuration.maximum_velocity);
            if uncapped > configuration.maximum_velocity && uncapped > 0.0 {
                wish_velocity = scale(wish_velocity, command.maximum_speed / uncapped);
            }
            result.wish_state = WishState {
                direction,
                speed,
                uncapped_speed: uncapped,
            };
            result.wish_velocity = wish_velocity;
            let movement_surface_friction = surface_friction(result.state, policy);
            accelerate(
                &mut result.state.velocity,
                direction,
                speed,
                configuration.spectator_acceleration,
                tick,
                movement_surface_friction,
            );
            let current = length(result.state.velocity);
            if current < 1.0 {
                result.state.velocity = [0.0; 3];
                return Ok(());
            }
            let next = (current - current * configuration.friction * tick).max(0.0);
            result.state.velocity = scale(result.state.velocity, next / current);
            clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);
            let hull = configuration.observer_hull.ok_or_else(|| {
                Error::new(Operation::Validate, FailureKind::Missing, "observer-hull")
            })?;
            let movement = slide(
                context,
                result.state.position,
                result.state.velocity,
                hull,
                configuration,
                tick,
                false,
                true,
                QueryPurpose::ObserverDisplacement,
                movement_surface_friction,
            )?;
            apply_move_outcome(result, movement);
            Ok(())
        }
        ObserverMode::None
        | ObserverMode::Deathcam
        | ObserverMode::Freezecam
        | ObserverMode::Fixed => Ok(()),
    }
}

fn toss_move(
    context: &mut QueryContext<'_, impl Tracer>,
    result: &mut StepResult,
    command: PreparedCommand,
    configuration: Configuration,
    policy: Policy,
    tick: f32,
) -> Result<(), Error> {
    check_water(context, &mut result.state, configuration, policy)?;
    if command.forward_move != 0.0 || command.side_move != 0.0 || command.up_move != 0.0 {
        let wish_velocity = add(
            add(
                scale(command.forward, command.forward_move),
                scale(command.right, command.side_move),
            ),
            [0.0, 0.0, command.up_move],
        );
        let uncapped = length(wish_velocity);
        let direction = normalized(wish_velocity);
        let speed = uncapped.min(command.maximum_speed);
        result.wish_state = WishState {
            direction,
            speed,
            uncapped_speed: uncapped,
        };
        let movement_surface_friction = surface_friction(result.state, policy);
        accelerate(
            &mut result.state.velocity,
            direction,
            speed,
            configuration.acceleration,
            tick,
            movement_surface_friction,
        );
    }
    if result.state.velocity[2] > 0.0 {
        set_ground(
            context,
            &mut result.state,
            None,
            &mut result.contacts,
            &mut result.events,
        )?;
    }
    if result.state.ground.is_some()
        && result.state.base_velocity == [0.0; 3]
        && result.state.velocity == [0.0; 3]
    {
        return Ok(());
    }
    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);
    if result.state.mode == Mode::FlyGravity {
        add_gravity(&mut result.state, configuration, tick);
    }
    let total_velocity = add(result.state.velocity, result.state.base_velocity);
    let displacement = scale(total_velocity, tick);
    let hull = result.state.active_hull(policy);
    let trace = context.trace(
        QueryPurpose::TossDisplacement,
        result.state.position,
        add(result.state.position, displacement),
        hull,
        configuration.solid_mask,
    )?;
    result.state.position = trace.end;
    if trace.fraction < 1.0 && !trace.all_solid {
        record_contact(&mut result.contacts, trace, result.state.velocity);
    }
    if trace.all_solid {
        result.state.velocity = [0.0; 3];
        result.events.push(Event::Trapped);
        return Ok(());
    }
    if trace.fraction != 1.0 {
        let normal = trace
            .normal
            .ok_or_else(|| Error::new(Operation::Trace, FailureKind::Missing, "toss-plane"))?;
        let overbounce = if result.state.move_collision == MoveCollision::Bounce {
            2.0 - surface_friction(result.state, policy)
        } else {
            1.0
        };
        result.state.velocity = clip_velocity(result.state.velocity, normal, overbounce);
        if normal[2] > configuration.standable_normal {
            if result.state.velocity[2] < configuration.gravity * tick {
                let ground = GroundState {
                    support: trace.hit,
                    normal,
                    surface_friction: policy.surface_friction,
                };
                set_ground(
                    context,
                    &mut result.state,
                    Some(ground),
                    &mut result.contacts,
                    &mut result.events,
                )?;
                result.state.velocity[2] = 0.0;
            }
            if length_squared(result.state.velocity) < 900.0
                || result.state.move_collision != MoveCollision::Bounce
            {
                let ground = GroundState {
                    support: trace.hit,
                    normal,
                    surface_friction: policy.surface_friction,
                };
                set_ground(
                    context,
                    &mut result.state,
                    Some(ground),
                    &mut result.contacts,
                    &mut result.events,
                )?;
                result.state.velocity = [0.0; 3];
            } else {
                let residual = scale(result.state.velocity, (1.0 - trace.fraction) * tick * 0.9);
                let residual_trace = context.trace(
                    QueryPurpose::TossDisplacement,
                    result.state.position,
                    add(result.state.position, residual),
                    hull,
                    configuration.solid_mask,
                )?;
                result.state.position = residual_trace.end;
            }
        }
    }
    check_water(context, &mut result.state, configuration, policy)?;
    Ok(())
}

struct QueryContext<'a, T: Tracer> {
    tracer: &'a T,
    sweeps: Vec<QueryRecord>,
    points: Vec<PointQueryRecord>,
}

impl<'a, T: Tracer> QueryContext<'a, T> {
    fn new(tracer: &'a T) -> Self {
        Self {
            tracer,
            sweeps: Vec::new(),
            points: Vec::new(),
        }
    }

    fn trace(
        &mut self,
        purpose: QueryPurpose,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, Error> {
        let result = self.tracer.trace(start, end, hull, mask)?;
        validate_trace(result)?;
        self.sweeps.push(QueryRecord {
            purpose,
            start,
            end,
            hull,
            mask,
            result,
        });
        Ok(result)
    }

    fn trace_without(
        &mut self,
        ignored: u64,
        purpose: QueryPurpose,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, Error> {
        let result = self.tracer.trace_without(ignored, start, end, hull, mask)?;
        validate_trace(result)?;
        self.sweeps.push(QueryRecord {
            purpose,
            start,
            end,
            hull,
            mask,
            result,
        });
        Ok(result)
    }

    fn point(&mut self, purpose: PointQueryPurpose, point: [f32; 3]) -> Result<u32, Error> {
        if point
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE)
        {
            return Err(Error::new(
                Operation::PointContents,
                FailureKind::Malformed,
                "point-contents-position",
            ));
        }
        let contents = self.tracer.point_contents(point)?;
        self.points.push(PointQueryRecord {
            purpose,
            point,
            contents,
        });
        Ok(contents)
    }
}

fn validate(
    state: State,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> Result<(), Error> {
    validate_output(state, configuration)?;
    for (field, value, limit) in [
        (
            "forward",
            input.command.forward,
            Some(MAX_COMMAND_MAGNITUDE),
        ),
        ("side", input.command.side, Some(MAX_COMMAND_MAGNITUDE)),
        ("yaw", input.command.yaw_degrees, None),
        ("pitch", input.pitch_degrees, None),
        ("up", input.up, Some(MAX_COMMAND_MAGNITUDE)),
    ] {
        if !value.is_finite() || limit.is_some_and(|limit| value.abs() > limit) {
            return Err(
                Error::new(Operation::Validate, FailureKind::Malformed, field)
                    .with_value(value, limit),
            );
        }
    }
    let finite_nonnegative = [
        configuration.acceleration,
        configuration.air_acceleration,
        configuration.friction,
        configuration.stop_speed,
        configuration.gravity,
        configuration.gravity_scale,
        configuration.lagged_movement_scale,
        configuration.step_height,
        configuration.step_epsilon,
        configuration.ground_probe,
        configuration.stay_ground_rise,
        configuration.ground_network_snap,
        configuration.maximum_velocity,
        configuration.server_max_speed,
        configuration.client_max_speed,
        configuration.surface_max_speed_factor,
        configuration.noclip_speed,
        configuration.spectator_speed,
        configuration.spectator_acceleration,
        configuration.bounce,
        configuration.ladder_distance,
        configuration.ladder_speed,
        configuration.ladder_lateral_factor,
        configuration.ladder_jump_speed,
        configuration.water_idle_sink_speed,
        configuration.water_wish_speed_factor,
        configuration.water_swim_speed,
        configuration.slime_swim_speed,
        configuration.water_exit_forward,
        configuration.water_exit_height,
        configuration.water_exit_down,
        configuration.water_exit_up_speed,
        configuration.water_exit_push_speed,
        configuration.water_exit_duration_ms,
        configuration.water_exit_maximum_ms,
        configuration.current_speed_per_level,
        configuration.roll_speed,
        configuration.fall_punch_threshold,
        configuration.floating_fall_reduction,
        configuration.stuck_retry_seconds,
        configuration.stuck_multiplayer_interval,
        configuration.stuck_singleplayer_interval,
        policy.maximum_speed,
        policy.air_speed_cap,
        policy.backward_speed_factor,
        policy.backward_speed_minimum,
        policy.ground_detach_speed,
        policy.jump_impulse,
        policy.surface_friction,
        policy.surface_jump_factor,
        policy.duck_duration,
        policy.unduck_duration,
        policy.crouched_command_factor,
        policy.water.waist_height_offset,
    ];
    let tick = configuration.tick_interval * configuration.lagged_movement_scale;
    let noclip_maximum = configuration.server_max_speed * configuration.noclip_speed;
    let invalid_constraint = configuration.movement_constraint.is_some_and(|constraint| {
        constraint.center.iter().any(|value| !value.is_finite())
            || !constraint.radius.is_finite()
            || constraint.radius < 0.0
            || !constraint.width.is_finite()
            || constraint.width < 0.0
            || constraint.width > constraint.radius
            || !constraint.outward_speed_factor.is_finite()
            || constraint.outward_speed_factor < 0.0
    });
    if !configuration.tick_interval.is_finite()
        || configuration.tick_interval <= 0.0
        || configuration.tick_interval > 1.0
        || finite_nonnegative
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || !configuration.noclip_acceleration.is_finite()
        || configuration.noclip_acceleration.abs() > 1_000.0
        || !configuration.roll_angle.is_finite()
        || configuration.maximum_velocity <= 0.0
        || policy.maximum_speed > configuration.maximum_velocity
        || policy.air_speed_cap > configuration.maximum_velocity
        || policy.ground_detach_speed > configuration.maximum_velocity
        || !(0.0..=1.0).contains(&policy.backward_speed_factor)
        || !(0.0..=1.0).contains(&policy.crouched_command_factor)
        || !configuration.standable_normal.is_finite()
        || !(0.0..=1.0).contains(&configuration.standable_normal)
        || !tick.is_finite()
        || tick <= 0.0
        || !noclip_maximum.is_finite()
        || noclip_maximum > configuration.maximum_velocity
        || policy.bunnyhop_speed_cap.is_some_and(|value| {
            !value.is_finite() || value < 0.0 || value > configuration.maximum_velocity
        })
        || invalid_constraint
        || !valid_hull(policy.standing_hull)
        || !valid_hull(policy.crouched_hull)
        || configuration
            .observer_hull
            .is_some_and(|hull| !valid_hull(hull))
        || !hull_straddles_origin(policy.standing_hull)
        || !hull_straddles_origin(policy.crouched_hull)
        || policy
            .standing_view
            .into_iter()
            .chain(policy.crouched_view)
            .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE)
    {
        return Err(Error::new(
            Operation::Validate,
            FailureKind::Malformed,
            "configuration",
        ));
    }
    Ok(())
}

fn validate_output(state: State, configuration: Configuration) -> Result<(), Error> {
    let vectors = state
        .position
        .into_iter()
        .chain(state.velocity)
        .chain(state.base_velocity)
        .chain(state.local_angles)
        .chain(state.absolute_view_angles)
        .chain(state.ladder_normal)
        .chain(state.view_offset)
        .chain(state.water_jump_velocity);
    if vectors.into_iter().any(|value| !value.is_finite())
        || state
            .position
            .iter()
            .any(|value| value.abs() > MAX_COORDINATE)
        || state
            .velocity
            .iter()
            .any(|value| value.abs() > configuration.maximum_velocity)
        || state
            .base_velocity
            .iter()
            .any(|value| value.abs() > configuration.maximum_velocity)
        || !(0.0..=1.0).contains(&state.crouch.fraction)
        || !(0.0..=1.0).contains(&state.crouch.linear_fraction)
        || !state.crouch.elapsed.is_finite()
        || state.crouch.elapsed < 0.0
        || !state.crouch.duration.is_finite()
        || state.crouch.duration < 0.0
        || !state.fall_speed.is_finite()
        || state.fall_speed < 0.0
        || state.water_level > 3
        || !state.water_jump_time_ms.is_finite()
        || state.water_jump_time_ms < 0.0
        || !state.previous_forward.is_finite()
        || !state.stuck_next_check_time.is_finite()
        || state.stuck_offset as usize >= STUCK_OFFSETS
        || !state.surface_friction.is_finite()
        || state.surface_friction < 0.0
    {
        return Err(Error::new(
            Operation::Validate,
            FailureKind::Malformed,
            "state",
        ));
    }
    Ok(())
}

fn validate_trace(trace: Trace) -> Result<(), Error> {
    let normal_valid = trace.normal.is_none_or(|normal| {
        normal.iter().all(|value| value.is_finite())
            && (0.9..=1.1).contains(&length_squared(normal))
    });
    if !trace.fraction.is_finite()
        || !(0.0..=1.0).contains(&trace.fraction)
        || trace.end.iter().any(|value| !value.is_finite())
        || !normal_valid
    {
        return Err(Error::new(
            Operation::Trace,
            FailureKind::Malformed,
            "trace-result",
        ));
    }
    Ok(())
}

fn valid_hull(hull: Hull) -> bool {
    hull.mins
        .into_iter()
        .chain(hull.maxs)
        .all(|value| value.is_finite() && value.abs() <= MAX_COORDINATE)
        && hull
            .mins
            .into_iter()
            .zip(hull.maxs)
            .all(|(minimum, maximum)| minimum <= maximum)
}

fn hull_straddles_origin(hull: Hull) -> bool {
    hull.mins[0] <= 0.0 && hull.maxs[0] >= 0.0 && hull.mins[1] <= 0.0 && hull.maxs[1] >= 0.0
}

fn standable(trace: Trace, threshold: f32) -> bool {
    !trace.start_solid
        && !trace.all_solid
        && trace.fraction < 1.0
        && trace.normal.is_some_and(|normal| normal[2] >= threshold)
}

fn quadrants(hull: Hull) -> [Hull; 4] {
    [
        Hull {
            mins: hull.mins,
            maxs: [hull.maxs[0].min(0.0), hull.maxs[1].min(0.0), hull.maxs[2]],
        },
        Hull {
            mins: [hull.mins[0].max(0.0), hull.mins[1].max(0.0), hull.mins[2]],
            maxs: hull.maxs,
        },
        Hull {
            mins: [hull.mins[0], hull.mins[1].max(0.0), hull.mins[2]],
            maxs: [hull.maxs[0].min(0.0), hull.maxs[1], hull.maxs[2]],
        },
        Hull {
            mins: [hull.mins[0].max(0.0), hull.mins[1], hull.mins[2]],
            maxs: [hull.maxs[0], hull.maxs[1].min(0.0), hull.maxs[2]],
        },
    ]
}

fn stuck_offsets() -> [[f32; 3]; STUCK_OFFSETS] {
    let mut offsets = [[0.0; 3]; STUCK_OFFSETS];
    let mut index = 0;
    for z in [-0.125, 0.0, 0.125] {
        offsets[index] = [0.0, 0.0, z];
        index += 1;
    }
    for y in [-0.125, 0.0, 0.125] {
        offsets[index] = [0.0, y, 0.0];
        index += 1;
    }
    for x in [-0.125, 0.0, 0.125] {
        offsets[index] = [x, 0.0, 0.0];
        index += 1;
    }
    for x in [-0.125, 0.125] {
        for y in [-0.125, 0.125] {
            for z in [-0.125, 0.125] {
                offsets[index] = [x, y, z];
                index += 1;
            }
        }
    }
    for z in [0.0, 1.0, 6.0] {
        offsets[index] = [0.0, 0.0, z];
        index += 1;
    }
    for y in [-2.0, 0.0, 2.0] {
        offsets[index] = [0.0, y, 0.0];
        index += 1;
    }
    for x in [-2.0, 0.0, 2.0] {
        offsets[index] = [x, 0.0, 0.0];
        index += 1;
    }
    for z in [0.0, 1.0, 6.0] {
        for x in [-2.0, 0.0, 2.0] {
            for y in [-2.0, 0.0, 2.0] {
                offsets[index] = [x, y, z];
                index += 1;
            }
        }
    }
    debug_assert_eq!(index, 53);
    offsets
}

fn record_contact(contacts: &mut Vec<Contact>, trace: Trace, impact_velocity: [f32; 3]) {
    if let Some(normal) = trace.normal {
        record_contact_from_parts(contacts, trace.hit, normal, impact_velocity);
    }
}

fn record_contact_from_parts(
    contacts: &mut Vec<Contact>,
    hit: Option<u64>,
    normal: [f32; 3],
    impact_velocity: [f32; 3],
) {
    if contacts.iter().any(|contact| contact.hit == hit) {
        return;
    }
    contacts.push(Contact {
        hit,
        normal,
        impact_velocity,
        order: contacts.len() as u8,
    });
}

fn movement_roll(
    pitch: f32,
    yaw: f32,
    velocity: [f32; 3],
    roll_angle: f32,
    roll_speed: f32,
) -> f32 {
    if roll_speed == 0.0 {
        return 0.0;
    }
    let (_, right, _) = angle_basis(pitch, yaw, 0.0);
    let side = dot(velocity, right);
    let sign = if side < 0.0 { -1.0 } else { 1.0 };
    sign * if side.abs() < roll_speed {
        side.abs() * roll_angle / roll_speed
    } else {
        roll_angle
    }
}

fn angle_basis(
    pitch_degrees: f32,
    yaw_degrees: f32,
    roll_degrees: f32,
) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (pitch_sine, pitch_cosine) = pitch_degrees.to_radians().sin_cos();
    let (yaw_sine, yaw_cosine) = yaw_degrees.to_radians().sin_cos();
    let (roll_sine, roll_cosine) = roll_degrees.to_radians().sin_cos();
    let forward = [
        pitch_cosine * yaw_cosine,
        pitch_cosine * yaw_sine,
        -pitch_sine,
    ];
    let right = [
        -roll_sine * pitch_sine * yaw_cosine + roll_cosine * yaw_sine,
        -roll_sine * pitch_sine * yaw_sine - roll_cosine * yaw_cosine,
        -roll_sine * pitch_cosine,
    ];
    let up = [
        roll_cosine * pitch_sine * yaw_cosine + roll_sine * yaw_sine,
        roll_cosine * pitch_sine * yaw_sine - roll_sine * yaw_cosine,
        roll_cosine * pitch_cosine,
    ];
    (forward, right, up)
}

fn clip_velocity(input: [f32; 3], normal: [f32; 3], overbounce: f32) -> [f32; 3] {
    let backoff = dot(input, normal) * overbounce;
    let mut output = sub(input, scale(normal, backoff));
    let adjustment = dot(output, normal);
    if adjustment < 0.0 {
        output = sub(output, scale(normal, adjustment));
    }
    output
}

fn clamp_velocity(velocity: &mut [f32; 3], maximum: f32) {
    for value in velocity {
        if value.is_nan() {
            *value = 0.0;
        }
        *value = value.clamp(-maximum, maximum);
    }
}

fn cap_horizontal_speed(velocity: &mut [f32; 3], maximum: f32) {
    let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();
    if speed > maximum && speed > 0.0 {
        let factor = maximum / speed;
        velocity[0] *= factor;
        velocity[1] *= factor;
    }
}

fn cap_vector_speed(velocity: &mut [f32; 3], maximum: f32) {
    let speed = length(*velocity);
    if speed > maximum && speed > 0.0 {
        *velocity = scale(*velocity, maximum / speed);
    }
}

fn horizontal_distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

fn simple_spline(value: f32) -> f32 {
    let squared = value * value;
    3.0 * squared - 2.0 * squared * value
}

fn normalize_in_place(vector: &mut [f32; 3]) -> f32 {
    let magnitude = length(*vector);
    if magnitude > 0.0 {
        *vector = scale(*vector, 1.0 / magnitude);
    }
    magnitude
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(vector: [f32; 3], factor: f32) -> [f32; 3] {
    [vector[0] * factor, vector[1] * factor, vector[2] * factor]
}

fn length_squared(vector: [f32; 3]) -> f32 {
    dot(vector, vector)
}

fn length(vector: [f32; 3]) -> f32 {
    length_squared(vector).sqrt()
}

fn normalized(vector: [f32; 3]) -> [f32; 3] {
    let magnitude = length(vector);
    if magnitude > 0.0 {
        scale(vector, 1.0 / magnitude)
    } else {
        [0.0; 3]
    }
}
