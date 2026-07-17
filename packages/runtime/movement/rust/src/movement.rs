use crate::{
    Configuration, Contact, CrouchPhase, Error, Event, FailureKind, GroundState,
    MAX_COMMAND_MAGNITUDE, MAX_COORDINATE, Mode, Operation, Policy, QueryPurpose, QueryRecord,
    State, StateDisposition, StepInput, StepResult, StepStrategy, Trace, Tracer,
};
use playsrc_collision::Hull;

const MAX_BUMPS: usize = 4;
const MAX_CLIP_PLANES: usize = 5;

pub(super) fn step(
    tracer: &impl Tracer,
    mut state: State,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> Result<StepResult, Error> {
    validate(state, input, configuration, policy)?;
    let mut context = TraceContext {
        tracer,
        queries: Vec::new(),
    };
    let mut events = Vec::new();
    apply_mode_request(&mut state, input.mode_request, policy, &mut events);

    let mut result = StepResult {
        state,
        wish_velocity: [0.0; 3],
        jump_velocity: [0.0; 3],
        climbed_step: 0.0,
        contacts: Vec::new(),
        events,
        queries: Vec::new(),
    };

    match result.state.mode {
        Mode::Noclip => noclip(&mut result.state, input, configuration, policy),
        Mode::Walk => walk(&mut context, &mut result, input, configuration, policy)?,
    }

    result.state.previous_jump = input.command.jump;
    result.state.previous_crouch = input.command.crouch;
    validate_output(result.state, configuration)?;
    result.queries = context.queries;
    Ok(result)
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
    }
    if request.disposition.ground == StateDisposition::Reset {
        state.ground = None;
        state.fall_speed = 0.0;
    }
    if request.disposition.water == StateDisposition::Reset {
        state.water_level = 0;
    }
    state.mode = request.mode;
    events.push(Event::ModeChanged {
        from,
        to: request.mode,
        disposition: request.disposition,
    });
}

fn walk(
    context: &mut TraceContext<'_, impl Tracer>,
    result: &mut StepResult,
    input: StepInput,
    configuration: Configuration,
    policy: Policy,
) -> Result<(), Error> {
    let initial_hull = result.state.active_hull(policy);
    if !recover_initial_overlap(
        context,
        &mut result.state,
        initial_hull,
        configuration,
        &mut result.events,
    )? {
        result.state.velocity = [0.0; 3];
        return Ok(());
    }

    categorize_ground(
        context,
        &mut result.state,
        configuration,
        policy,
        &mut result.events,
    )?;
    let started_grounded = result.state.ground.is_some();
    if !started_grounded && result.state.velocity[2] < 0.0 {
        result.state.fall_speed = result.state.fall_speed.max(-result.state.velocity[2]);
    }

    update_crouch(
        context,
        &mut result.state,
        input.command.crouch,
        configuration,
        policy,
        &mut result.events,
    )?;

    let half_gravity = configuration.gravity * configuration.tick_interval * 0.5;
    result.state.velocity[2] -= half_gravity;
    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);

    if !input.command.jump {
        result.state.jump_latched = false;
    } else if !result.state.jump_latched {
        if result.state.ground.is_none() {
            result.state.jump_latched = true;
        } else if policy.allow_jump
            && (policy.allow_crouched_jump || !result.state.crouch.uses_crouched_hull())
        {
            if let Some(cap) = policy.bunnyhop_speed_cap {
                cap_vector_speed(&mut result.state.velocity, cap);
            }
            set_ground(&mut result.state, None, &mut result.events);
            let start_z = result.state.velocity[2];
            let impulse = policy.jump_impulse * policy.surface_jump_factor;
            if policy.replace_vertical_while_ducking
                && result.state.crouch.phase == CrouchPhase::Ducking
            {
                result.state.velocity[2] = impulse;
            } else {
                result.state.velocity[2] += impulse;
            }
            result.state.velocity[2] -= half_gravity;
            result.jump_velocity[2] = result.state.velocity[2] - start_z;
            result.state.jump_latched = true;
            result.events.push(Event::Jumped);
        }
    }

    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);
    let mut command = input.command;
    let mut up = input.up;
    let command_speed =
        (command.forward * command.forward + command.side * command.side + up * up).sqrt();
    if command_speed > policy.maximum_speed && command_speed > 0.0 {
        let factor = policy.maximum_speed / command_speed;
        command.forward *= factor;
        command.side *= factor;
        up *= factor;
    }
    if result.state.ground.is_some() && result.state.crouch.uses_crouched_hull() {
        command.forward *= policy.crouched_command_factor;
        command.side *= policy.crouched_command_factor;
        up *= policy.crouched_command_factor;
    }
    let _ = up;

    let (forward, right, _) = angle_basis(input.pitch_degrees, command.yaw_degrees, 0.0);
    let flat_forward = normalized([forward[0], forward[1], 0.0]);
    let flat_right = normalized([right[0], right[1], 0.0]);
    let mut wish = add(
        scale(flat_forward, command.forward),
        scale(flat_right, command.side),
    );
    wish[2] = 0.0;
    let wish_speed_raw = length(wish);
    let wish_direction = normalized(wish);
    let wish_speed = wish_speed_raw.min(policy.maximum_speed);
    result.wish_velocity = scale(wish_direction, wish_speed);

    if result.state.ground.is_some() {
        result.state.velocity[2] = 0.0;
        ground_friction(&mut result.state.velocity, configuration, policy);
        accelerate(
            &mut result.state.velocity,
            wish_direction,
            wish_speed,
            configuration.acceleration,
            configuration.tick_interval,
            policy.surface_friction,
        );
        cap_horizontal_speed(&mut result.state.velocity, policy.maximum_speed);
        clamp_backward_speed(&mut result.state.velocity, flat_forward, flat_right, policy);
    } else {
        air_accelerate(
            &mut result.state.velocity,
            wish_direction,
            wish_speed,
            configuration,
            policy,
        );
    }

    let hull = result.state.active_hull(policy);
    let movement = if result.state.ground.is_some() && length(result.state.velocity) < 1.0 {
        result.state.velocity = [0.0; 3];
        MoveOutcome {
            position: result.state.position,
            velocity: result.state.velocity,
            contacts: Vec::new(),
            climbed: 0.0,
            trapped: false,
        }
    } else if result.state.ground.is_some() {
        ground_move(
            context,
            result.state.position,
            result.state.velocity,
            hull,
            configuration,
            policy.step_strategy,
        )?
    } else {
        slide(
            context,
            result.state.position,
            result.state.velocity,
            hull,
            configuration,
        )?
    };
    result.state.position = movement.position;
    result.state.velocity = movement.velocity;
    result.contacts.extend(movement.contacts);
    if movement.trapped {
        result.state.velocity = [0.0; 3];
        result.events.push(Event::Trapped);
    }
    if movement.climbed > 0.0 {
        result.climbed_step = movement.climbed;
        result.events.push(Event::Stepped {
            height: movement.climbed,
        });
    }

    let ground_before_categorize = result.state.ground.is_some();
    categorize_ground(
        context,
        &mut result.state,
        configuration,
        policy,
        &mut result.events,
    )?;
    result.state.velocity[2] -= half_gravity;
    if result.state.ground.is_some() && result.state.velocity[2] < 0.0 {
        result.state.velocity[2] = 0.0;
    }
    if result.state.ground.is_none() && result.state.velocity[2] < 0.0 {
        result.state.fall_speed = result.state.fall_speed.max(-result.state.velocity[2]);
    } else if !ground_before_categorize && result.state.ground.is_some() {
        result.events.push(Event::Landed {
            fall_speed: result.state.fall_speed,
        });
        result.state.fall_speed = 0.0;
    }
    clamp_velocity(&mut result.state.velocity, configuration.maximum_velocity);
    Ok(())
}

fn noclip(state: &mut State, input: StepInput, configuration: Configuration, policy: Policy) {
    let (forward, right, _) = angle_basis(input.pitch_degrees, input.command.yaw_degrees, 0.0);
    let maximum_speed = configuration.server_max_speed * configuration.noclip_speed;
    let command_factor = if input.speed_button {
        configuration.noclip_speed * 0.5
    } else {
        configuration.noclip_speed
    };
    let mut wish = add(
        scale(forward, input.command.forward * command_factor),
        scale(right, input.command.side * command_factor),
    );
    wish[2] += input.up * command_factor;
    let raw_speed = length(wish);
    let direction = normalized(wish);
    let wish_speed = raw_speed.min(maximum_speed);
    wish = scale(direction, wish_speed);

    if configuration.noclip_acceleration > 0.0 {
        accelerate(
            &mut state.velocity,
            direction,
            wish_speed,
            configuration.noclip_acceleration,
            configuration.tick_interval,
            policy.surface_friction,
        );
        let speed = length(state.velocity);
        if speed < 1.0 {
            state.velocity = [0.0; 3];
            return;
        }
        let control = speed.max(maximum_speed * 0.25);
        let drop = control
            * configuration.friction
            * policy.surface_friction
            * configuration.tick_interval;
        state.velocity = scale(state.velocity, (speed - drop).max(0.0) / speed);
    } else {
        state.velocity = wish;
    }

    state.position = add(
        state.position,
        scale(state.velocity, configuration.tick_interval),
    );
    clamp_velocity(&mut state.velocity, configuration.maximum_velocity);
    if configuration.noclip_acceleration < 0.0 {
        state.velocity = [0.0; 3];
    }
}

fn update_crouch(
    context: &mut TraceContext<'_, impl Tracer>,
    state: &mut State,
    crouch_command: bool,
    configuration: Configuration,
    policy: Policy,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    let before = state.crouch.phase;
    let grounded = state.ground.is_some();
    if !policy.allow_duck {
        if before != CrouchPhase::Standing {
            if try_finish_unduck(context, state, grounded, configuration, policy)? {
                state.crouch = crate::CrouchState::STANDING;
                update_view(state, policy);
            } else {
                state.crouch.phase = CrouchPhase::Blocked;
            }
        }
        record_crouch_change(before, state.crouch.phase, events);
        return Ok(());
    }

    if !grounded {
        if crouch_command {
            if !state.crouch.uses_crouched_hull() {
                let delta = hull_size_delta(policy);
                state.position = add(state.position, delta);
            }
            state.crouch = crate::CrouchState::CROUCHED;
        } else if !crouch_command && state.crouch.uses_crouched_hull() {
            if try_finish_unduck(context, state, false, configuration, policy)? {
                state.crouch = crate::CrouchState::STANDING;
            } else {
                state.crouch = crate::CrouchState {
                    phase: CrouchPhase::Blocked,
                    fraction: 1.0,
                    start_fraction: 1.0,
                    elapsed: 0.0,
                    duration: policy.unduck_duration,
                };
            }
        } else if !crouch_command && state.crouch.phase == CrouchPhase::Ducking {
            state.crouch = crate::CrouchState::STANDING;
        }
        update_view(state, policy);
        record_crouch_change(before, state.crouch.phase, events);
        return Ok(());
    }

    let pressed = crouch_command && !state.previous_crouch;
    let released = !crouch_command && state.previous_crouch;
    let mut started = false;
    match state.crouch.phase {
        CrouchPhase::Standing => {
            if pressed || crouch_command {
                begin_crouch_transition(state, CrouchPhase::Ducking, policy.duck_duration);
                started = true;
            }
        }
        CrouchPhase::Ducking => {
            if released || !crouch_command {
                begin_crouch_transition(
                    state,
                    CrouchPhase::Unducking,
                    policy.unduck_duration * state.crouch.fraction,
                );
                started = true;
            }
        }
        CrouchPhase::Crouched => {
            if released || !crouch_command {
                if can_unduck(context, *state, true, configuration, policy)? {
                    begin_crouch_transition(state, CrouchPhase::Unducking, policy.unduck_duration);
                } else {
                    state.crouch.phase = CrouchPhase::Blocked;
                }
                started = true;
            }
        }
        CrouchPhase::Unducking => {
            if pressed || crouch_command {
                begin_crouch_transition(
                    state,
                    CrouchPhase::Ducking,
                    policy.duck_duration * (1.0 - state.crouch.fraction),
                );
                started = true;
            } else if !can_unduck(context, *state, true, configuration, policy)? {
                state.crouch = crate::CrouchState {
                    phase: CrouchPhase::Blocked,
                    fraction: 1.0,
                    start_fraction: 1.0,
                    elapsed: 0.0,
                    duration: policy.unduck_duration,
                };
                started = true;
            }
        }
        CrouchPhase::Blocked => {
            if crouch_command {
                state.crouch = crate::CrouchState::CROUCHED;
            } else if can_unduck(context, *state, true, configuration, policy)? {
                begin_crouch_transition(state, CrouchPhase::Unducking, policy.unduck_duration);
            }
            started = true;
        }
    }

    if !started {
        advance_crouch_transition(state, configuration.tick_interval, policy);
    }
    update_view(state, policy);
    record_crouch_change(before, state.crouch.phase, events);
    Ok(())
}

fn begin_crouch_transition(state: &mut State, phase: CrouchPhase, duration: f32) {
    state.crouch.phase = phase;
    state.crouch.start_fraction = state.crouch.fraction;
    state.crouch.elapsed = 0.0;
    state.crouch.duration = duration.max(0.0);
}

fn advance_crouch_transition(state: &mut State, tick: f32, policy: Policy) {
    let target = match state.crouch.phase {
        CrouchPhase::Ducking => 1.0,
        CrouchPhase::Unducking => 0.0,
        _ => return,
    };
    if state.crouch.duration == 0.0 {
        state.crouch.fraction = target;
    } else {
        state.crouch.elapsed += tick;
        let progress = (state.crouch.elapsed / state.crouch.duration).clamp(0.0, 1.0);
        let spline = simple_spline(progress);
        state.crouch.fraction =
            state.crouch.start_fraction + (target - state.crouch.start_fraction) * spline;
    }
    if state.crouch.elapsed >= state.crouch.duration {
        state.crouch.fraction = target;
        state.crouch.start_fraction = target;
        state.crouch.elapsed = 0.0;
        state.crouch.duration = 0.0;
        if target == 1.0 {
            let delta = sub(policy.crouched_hull.mins, policy.standing_hull.mins);
            state.position = sub(state.position, delta);
            state.crouch.phase = CrouchPhase::Crouched;
        } else {
            let delta = sub(policy.crouched_hull.mins, policy.standing_hull.mins);
            state.position = add(state.position, delta);
            state.crouch.phase = CrouchPhase::Standing;
        }
    }
}

fn try_finish_unduck(
    context: &mut TraceContext<'_, impl Tracer>,
    state: &mut State,
    grounded: bool,
    configuration: Configuration,
    policy: Policy,
) -> Result<bool, Error> {
    if !can_unduck(context, *state, grounded, configuration, policy)? {
        return Ok(false);
    }
    if grounded {
        state.position = add(
            state.position,
            sub(policy.crouched_hull.mins, policy.standing_hull.mins),
        );
    } else {
        state.position = sub(state.position, hull_size_delta(policy));
    }
    Ok(true)
}

fn can_unduck(
    context: &mut TraceContext<'_, impl Tracer>,
    state: State,
    grounded: bool,
    configuration: Configuration,
    policy: Policy,
) -> Result<bool, Error> {
    let end = if grounded {
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
        end,
        policy.standing_hull,
        configuration.solid_mask,
    )?;
    Ok(!trace.start_solid && !trace.all_solid && trace.fraction == 1.0)
}

fn hull_size_delta(policy: Policy) -> [f32; 3] {
    let standing = sub(policy.standing_hull.maxs, policy.standing_hull.mins);
    let crouched = sub(policy.crouched_hull.maxs, policy.crouched_hull.mins);
    sub(standing, crouched)
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

fn categorize_ground(
    context: &mut TraceContext<'_, impl Tracer>,
    state: &mut State,
    configuration: Configuration,
    policy: Policy,
    events: &mut Vec<Event>,
) -> Result<(), Error> {
    if state.velocity[2] > policy.ground_detach_speed {
        set_ground(state, None, events);
        return Ok(());
    }
    let was_grounded = state.ground.is_some();
    let hull = state.active_hull(policy);
    let distance = configuration.ground_probe
        + if was_grounded {
            configuration.step_height
        } else {
            0.0
        };
    let end = [
        state.position[0],
        state.position[1],
        state.position[2] - distance,
    ];
    let full = context.trace(
        QueryPurpose::GroundFull,
        state.position,
        end,
        hull,
        configuration.solid_mask,
    )?;
    if standable(full, configuration.standable_normal) {
        if was_grounded && full.fraction > 0.0 && full.fraction < 1.0 {
            state.position = full.end;
        }
        set_ground(
            state,
            Some(GroundState {
                support: full.hit,
                normal: full.normal.expect("standable trace has a plane"),
                surface_friction: policy.surface_friction,
            }),
            events,
        );
        return Ok(());
    }

    for quadrant in quadrants(hull) {
        let trace = context.trace(
            QueryPurpose::GroundQuadrant,
            state.position,
            end,
            quadrant,
            configuration.solid_mask,
        )?;
        if standable(trace, configuration.standable_normal) {
            set_ground(
                state,
                Some(GroundState {
                    support: trace.hit,
                    normal: trace.normal.expect("standable trace has a plane"),
                    surface_friction: policy.surface_friction,
                }),
                events,
            );
            return Ok(());
        }
    }
    set_ground(state, None, events);
    Ok(())
}

fn set_ground(state: &mut State, ground: Option<GroundState>, events: &mut Vec<Event>) {
    let before = state.ground.map(|value| value.support.unwrap_or(u64::MAX));
    let after = ground.map(|value| value.support.unwrap_or(u64::MAX));
    let changed = state.ground.is_some() != ground.is_some() || before != after;
    state.ground = ground;
    if changed {
        events.push(Event::GroundChanged {
            from: before,
            to: after,
        });
    }
}

fn recover_initial_overlap(
    context: &mut TraceContext<'_, impl Tracer>,
    state: &mut State,
    hull: Hull,
    configuration: Configuration,
    events: &mut Vec<Event>,
) -> Result<bool, Error> {
    let initial = context.trace(
        QueryPurpose::InitialPosition,
        state.position,
        state.position,
        hull,
        configuration.solid_mask,
    )?;
    if !initial.start_solid && !initial.all_solid {
        return Ok(true);
    }

    let offsets = stuck_offsets();
    for step in 0..offsets.len() {
        let index = (state.stuck_offset as usize + step) % offsets.len();
        let candidate = add(state.position, offsets[index]);
        let trace = context.trace(
            QueryPurpose::StuckRecovery,
            candidate,
            candidate,
            hull,
            configuration.solid_mask,
        )?;
        if !trace.start_solid && !trace.all_solid && trace.fraction == 1.0 {
            state.position = candidate;
            state.stuck_offset = ((index + 1) % offsets.len()) as u8;
            events.push(Event::Recovered {
                offset: index as u8,
            });
            return Ok(true);
        }
    }
    events.push(Event::Trapped);
    Ok(false)
}

fn ground_friction(velocity: &mut [f32; 3], configuration: Configuration, policy: Policy) {
    let speed = length(*velocity);
    if speed < 0.1 {
        return;
    }
    let control = speed.max(configuration.stop_speed);
    let drop =
        control * configuration.friction * policy.surface_friction * configuration.tick_interval;
    *velocity = scale(*velocity, (speed - drop).max(0.0) / speed);
}

fn accelerate(
    velocity: &mut [f32; 3],
    direction: [f32; 3],
    wish_speed: f32,
    acceleration: f32,
    tick: f32,
    surface_friction: f32,
) {
    if wish_speed == 0.0 {
        return;
    }
    let add_speed = wish_speed - dot(*velocity, direction);
    if add_speed <= 0.0 {
        return;
    }
    let acceleration_speed = (acceleration * tick * wish_speed * surface_friction).min(add_speed);
    *velocity = add(*velocity, scale(direction, acceleration_speed));
}

fn air_accelerate(
    velocity: &mut [f32; 3],
    direction: [f32; 3],
    uncapped_wish_speed: f32,
    configuration: Configuration,
    policy: Policy,
) {
    if uncapped_wish_speed == 0.0 {
        return;
    }
    let capped_wish_speed = uncapped_wish_speed.min(policy.air_speed_cap);
    let add_speed = capped_wish_speed - dot(*velocity, direction);
    if add_speed <= 0.0 {
        return;
    }
    let acceleration_speed = (configuration.air_acceleration
        * configuration.tick_interval
        * uncapped_wish_speed
        * policy.surface_friction)
        .min(add_speed);
    *velocity = add(*velocity, scale(direction, acceleration_speed));
}

fn clamp_backward_speed(
    velocity: &mut [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
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
    let backward_speed = length(backward);
    let maximum_backward = policy.maximum_speed * policy.backward_speed_factor;
    if backward_speed > maximum_backward {
        backward = scale(backward, maximum_backward / backward_speed);
    }
    let vertical = velocity[2];
    *velocity = add(backward, lateral);
    velocity[2] = vertical;
    cap_horizontal_speed(velocity, policy.maximum_speed);
}

#[derive(Clone, Debug)]
struct MoveOutcome {
    position: [f32; 3],
    velocity: [f32; 3],
    contacts: Vec<Contact>,
    climbed: f32,
    trapped: bool,
}

fn ground_move(
    context: &mut TraceContext<'_, impl Tracer>,
    start: [f32; 3],
    velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
    strategy: StepStrategy,
) -> Result<MoveOutcome, Error> {
    let destination = [
        start[0] + velocity[0] * configuration.tick_interval,
        start[1] + velocity[1] * configuration.tick_interval,
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
        return Ok(MoveOutcome {
            position: if endpoint.start_solid || endpoint.all_solid {
                start
            } else {
                direct.end
            },
            velocity: if endpoint.start_solid || endpoint.all_solid {
                [0.0; 3]
            } else {
                velocity
            },
            contacts: Vec::new(),
            climbed: 0.0,
            trapped: endpoint.start_solid || endpoint.all_solid,
        });
    }
    step_move(context, start, velocity, hull, configuration, strategy)
}

fn step_move(
    context: &mut TraceContext<'_, impl Tracer>,
    start: [f32; 3],
    velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
    strategy: StepStrategy,
) -> Result<MoveOutcome, Error> {
    match strategy {
        StepStrategy::HighFirst => {
            if let Some(high) = high_step(context, start, velocity, hull, configuration)? {
                return Ok(high);
            }
            slide(context, start, velocity, hull, configuration)
        }
        StepStrategy::FurthestHorizontal => {
            let low = slide(context, start, velocity, hull, configuration)?;
            let Some(mut high) = high_step(context, start, velocity, hull, configuration)? else {
                return Ok(low);
            };
            let low_distance = horizontal_distance_squared(start, low.position);
            let high_distance = horizontal_distance_squared(start, high.position);
            if high_distance >= low_distance {
                high.velocity[2] = low.velocity[2];
                Ok(high)
            } else {
                Ok(low)
            }
        }
    }
}

fn high_step(
    context: &mut TraceContext<'_, impl Tracer>,
    start: [f32; 3],
    velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
) -> Result<Option<MoveOutcome>, Error> {
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
    let mut moved = slide(context, up.end, velocity, hull, configuration)?;
    if moved.trapped || horizontal_distance_squared(start, moved.position) == 0.0 {
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
    if !standable(down, configuration.standable_normal) {
        return Ok(None);
    }
    moved.position = down.end;
    moved.climbed = (moved.position[2] - start[2]).max(0.0);
    Ok(Some(moved))
}

fn slide(
    context: &mut TraceContext<'_, impl Tracer>,
    start: [f32; 3],
    mut velocity: [f32; 3],
    hull: Hull,
    configuration: Configuration,
) -> Result<MoveOutcome, Error> {
    let primal_velocity = velocity;
    let mut original_velocity = velocity;
    let mut position = start;
    let mut remaining = configuration.tick_interval;
    let mut fraction_sum = 0.0;
    let mut planes = Vec::with_capacity(MAX_CLIP_PLANES);
    let mut contacts = Vec::new();

    for _ in 0..MAX_BUMPS {
        if length_squared(velocity) == 0.0 || remaining <= 0.0 {
            break;
        }
        let end = add(position, scale(velocity, remaining));
        let trace = context.trace(
            QueryPurpose::Displacement,
            position,
            end,
            hull,
            configuration.solid_mask,
        )?;
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
            position = trace.end;
            original_velocity = velocity;
            planes.clear();
        }
        if trace.fraction == 1.0 {
            let endpoint = context.trace(
                QueryPurpose::Endpoint,
                position,
                position,
                hull,
                configuration.solid_mask,
            )?;
            if endpoint.start_solid || endpoint.all_solid {
                velocity = [0.0; 3];
            }
            break;
        }
        let Some(normal) = trace.normal else {
            velocity = [0.0; 3];
            break;
        };
        contacts.push(Contact {
            hit: trace.hit,
            normal,
            impact_velocity: velocity,
            order: contacts.len() as u8,
        });
        remaining -= remaining * trace.fraction;
        if planes.len() >= MAX_CLIP_PLANES {
            velocity = [0.0; 3];
            break;
        }
        planes.push(normal);

        let mut accepted = None;
        for plane in &planes {
            let candidate = clip_velocity(original_velocity, *plane);
            if planes.iter().all(|other| dot(candidate, *other) >= 0.0) {
                accepted = Some(candidate);
                break;
            }
        }
        velocity = if let Some(candidate) = accepted {
            candidate
        } else if planes.len() == 2 {
            let direction = normalized(cross(planes[0], planes[1]));
            scale(direction, dot(direction, original_velocity))
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

struct TraceContext<'a, T: Tracer> {
    tracer: &'a T,
    queries: Vec<QueryRecord>,
}

impl<T: Tracer> TraceContext<'_, T> {
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
        self.queries.push(QueryRecord {
            purpose,
            start,
            end,
            hull,
            mask,
            result,
        });
        Ok(result)
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
    let nonnegative = [
        configuration.acceleration,
        configuration.air_acceleration,
        configuration.friction,
        configuration.stop_speed,
        configuration.gravity,
        configuration.step_height,
        configuration.step_epsilon,
        configuration.ground_probe,
        configuration.maximum_velocity,
        configuration.server_max_speed,
        configuration.noclip_speed,
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
    ];
    let noclip_maximum = configuration.server_max_speed * configuration.noclip_speed;
    let jump_speed = policy.jump_impulse * policy.surface_jump_factor;
    let derived_per_tick = [
        configuration.gravity * configuration.tick_interval,
        configuration.acceleration
            * configuration.tick_interval
            * policy.maximum_speed
            * policy.surface_friction,
        configuration.air_acceleration
            * configuration.tick_interval
            * policy.maximum_speed
            * policy.surface_friction,
        configuration.noclip_acceleration.abs()
            * configuration.tick_interval
            * noclip_maximum
            * policy.surface_friction,
    ];
    if !configuration.tick_interval.is_finite()
        || configuration.tick_interval <= 0.0
        || configuration.tick_interval > 1.0
        || nonnegative
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        || !configuration.noclip_acceleration.is_finite()
        || configuration.noclip_acceleration.abs() > 1_000.0
        || configuration.maximum_velocity <= 0.0
        || policy.maximum_speed > configuration.maximum_velocity
        || policy.air_speed_cap > configuration.maximum_velocity
        || policy.ground_detach_speed > configuration.maximum_velocity
        || !(0.0..=1.0).contains(&policy.backward_speed_factor)
        || !(0.0..=1.0).contains(&policy.crouched_command_factor)
        || !noclip_maximum.is_finite()
        || noclip_maximum > configuration.maximum_velocity
        || !jump_speed.is_finite()
        || jump_speed > configuration.maximum_velocity
        || derived_per_tick
            .iter()
            .any(|value| !value.is_finite() || *value > configuration.maximum_velocity)
        || !(configuration.step_height + configuration.step_epsilon + configuration.ground_probe)
            .is_finite()
        || configuration.step_height + configuration.step_epsilon + configuration.ground_probe
            > MAX_COORDINATE
        || !configuration.standable_normal.is_finite()
        || !(0.0..=1.0).contains(&configuration.standable_normal)
        || policy.bunnyhop_speed_cap.is_some_and(|value| {
            !value.is_finite() || value < 0.0 || value > configuration.maximum_velocity
        })
        || !valid_hull(policy.standing_hull)
        || !valid_hull(policy.crouched_hull)
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
    if state
        .position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE)
        || state
            .velocity
            .iter()
            .any(|value| !value.is_finite() || value.abs() > configuration.maximum_velocity)
        || state.view_offset.iter().any(|value| !value.is_finite())
        || !state.crouch.fraction.is_finite()
        || !(0.0..=1.0).contains(&state.crouch.fraction)
        || !state.fall_speed.is_finite()
        || state.fall_speed < 0.0
        || state.water_level > 3
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

fn stuck_offsets() -> Vec<[f32; 3]> {
    let little = [-0.125, 0.0, 0.125];
    let edges = [-0.125, 0.125];
    let big_xy = [-2.0, 0.0, 2.0];
    let big_z = [0.0, 1.0, 6.0];
    let mut offsets = Vec::with_capacity(54);
    for z in little {
        offsets.push([0.0, 0.0, z]);
    }
    for y in little {
        offsets.push([0.0, y, 0.0]);
    }
    for x in little {
        offsets.push([x, 0.0, 0.0]);
    }
    for x in edges {
        for y in edges {
            for z in edges {
                offsets.push([x, y, z]);
            }
        }
    }
    for z in big_z {
        offsets.push([0.0, 0.0, z]);
    }
    for y in big_xy {
        offsets.push([0.0, y, 0.0]);
    }
    for x in big_xy {
        offsets.push([x, 0.0, 0.0]);
    }
    for z in big_z {
        for x in big_xy {
            for y in big_xy {
                offsets.push([x, y, z]);
            }
        }
    }
    offsets.push([0.0; 3]);
    debug_assert_eq!(offsets.len(), 54);
    offsets
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

fn clip_velocity(input: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let backoff = dot(input, normal);
    let mut output = sub(input, scale(normal, backoff));
    let adjustment = dot(output, normal);
    if adjustment < 0.0 {
        output = sub(output, scale(normal, adjustment));
    }
    output
}

fn clamp_velocity(velocity: &mut [f32; 3], maximum: f32) {
    for value in velocity {
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
    let length = length(vector);
    if length > 0.0 {
        scale(vector, 1.0 / length)
    } else {
        [0.0; 3]
    }
}
