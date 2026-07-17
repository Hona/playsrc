use super::*;
use std::cell::Cell;

#[derive(Clone, Copy)]
struct SolidBox {
    minimum: [f32; 3],
    maximum: [f32; 3],
    id: u64,
}

struct TestWorld {
    solids: Vec<SolidBox>,
    ceiling_enabled: Cell<bool>,
    ceiling: Option<SolidBox>,
    calls: Cell<usize>,
}

impl TestWorld {
    fn floor(height: f32) -> Self {
        Self {
            solids: vec![SolidBox {
                minimum: [-20_000.0, -20_000.0, -20_000.0],
                maximum: [20_000.0, 20_000.0, height],
                id: 1,
            }],
            ceiling_enabled: Cell::new(false),
            ceiling: None,
            calls: Cell::new(0),
        }
    }

    fn empty() -> Self {
        Self {
            solids: Vec::new(),
            ceiling_enabled: Cell::new(false),
            ceiling: None,
            calls: Cell::new(0),
        }
    }

    fn with_box(mut self, minimum: [f32; 3], maximum: [f32; 3], id: u64) -> Self {
        self.solids.push(SolidBox {
            minimum,
            maximum,
            id,
        });
        self
    }

    fn with_ceiling(mut self, height: f32) -> Self {
        self.ceiling = Some(SolidBox {
            minimum: [-20_000.0, -20_000.0, height],
            maximum: [20_000.0; 3],
            id: 99,
        });
        self.ceiling_enabled.set(true);
        self
    }
}

impl Tracer for TestWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, _: u32) -> Result<Trace, Error> {
        self.calls.set(self.calls.get() + 1);
        let mut solids = self.solids.clone();
        if self.ceiling_enabled.get()
            && let Some(ceiling) = self.ceiling
        {
            solids.push(ceiling);
        }
        Ok(trace_boxes(start, end, hull, &solids))
    }
}

fn trace_boxes(start: [f32; 3], end: [f32; 3], hull: Hull, solids: &[SolidBox]) -> Trace {
    let delta = subtract(end, start);
    let mut output = Trace {
        fraction: 1.0,
        start_solid: false,
        all_solid: false,
        end,
        normal: None,
        hit: None,
        contents: 0,
    };
    for solid in solids {
        let minimum = [
            solid.minimum[0] - hull.maxs[0],
            solid.minimum[1] - hull.maxs[1],
            solid.minimum[2] - hull.maxs[2],
        ];
        let maximum = [
            solid.maximum[0] - hull.mins[0],
            solid.maximum[1] - hull.mins[1],
            solid.maximum[2] - hull.mins[2],
        ];
        let inside = point_inside(start, minimum, maximum);
        let end_inside = point_inside(end, minimum, maximum);
        if inside {
            output.start_solid = true;
            output.contents |= 1;
            if end_inside {
                output.all_solid = true;
                output.hit = Some(solid.id);
            }
            continue;
        }
        if delta == [0.0; 3] {
            continue;
        }

        let mut enter = 0.0_f32;
        let mut leave = 1.0_f32;
        let mut enter_normal = None;
        let mut rejected = false;
        for axis in 0..3 {
            if delta[axis] == 0.0 {
                if start[axis] < minimum[axis] || start[axis] > maximum[axis] {
                    rejected = true;
                    break;
                }
                continue;
            }
            let first = (minimum[axis] - start[axis]) / delta[axis];
            let second = (maximum[axis] - start[axis]) / delta[axis];
            let (axis_enter, axis_leave, sign) = if first <= second {
                (first, second, -1.0)
            } else {
                (second, first, 1.0)
            };
            if axis_enter > enter || (axis_enter == enter && enter_normal.is_none()) {
                enter = axis_enter;
                let mut normal = [0.0; 3];
                normal[axis] = sign;
                enter_normal = Some(normal);
            }
            leave = leave.min(axis_leave);
            if enter > leave {
                rejected = true;
                break;
            }
        }
        if leave <= 0.0 {
            rejected = true;
        }
        if !rejected
            && enter_normal.is_some()
            && (0.0..=1.0).contains(&enter)
            && enter < output.fraction
        {
            output.fraction = enter;
            output.normal = enter_normal;
            output.hit = Some(solid.id);
            output.contents = 1;
        }
    }
    output.end = [
        start[0] + delta[0] * output.fraction,
        start[1] + delta[1] * output.fraction,
        start[2] + delta[2] * output.fraction,
    ];
    output
}

fn point_inside(point: [f32; 3], minimum: [f32; 3], maximum: [f32; 3]) -> bool {
    (0..3).all(|axis| point[axis] > minimum[axis] && point[axis] < maximum[axis])
}

fn subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn tf2_policy() -> Policy {
    Policy {
        maximum_speed: 240.0,
        air_speed_cap: 30.0,
        bunnyhop_speed_cap: Some(288.0),
        backward_speed_factor: 0.9,
        backward_speed_minimum: 100.0,
        ground_detach_speed: 250.0,
        jump_impulse: 289.0,
        surface_friction: 1.0,
        surface_jump_factor: 1.0,
        standing_hull: Hull {
            mins: [-24.0, -24.0, 0.0],
            maxs: [24.0, 24.0, 82.0],
        },
        crouched_hull: Hull {
            mins: [-24.0, -24.0, 0.0],
            maxs: [24.0, 24.0, 62.0],
        },
        standing_view: [0.0, 0.0, 68.0],
        crouched_view: [0.0, 0.0, 45.0],
        duck_duration: 0.2,
        unduck_duration: 0.2,
        crouched_command_factor: 0.333_333_34,
        allow_jump: true,
        allow_duck: true,
        allow_noclip: true,
        allow_crouched_jump: false,
        replace_vertical_while_ducking: true,
        step_strategy: StepStrategy::HighFirst,
    }
}

fn player(position: [f32; 3], grounded: bool, policy: Policy) -> State {
    State::from_player(
        Player {
            position,
            velocity: [0.0; 3],
            grounded,
            crouched: false,
            jump_latched: false,
        },
        policy,
    )
}

fn command(forward: f32, side: f32, yaw: f32) -> StepInput {
    StepInput {
        command: Command {
            forward,
            side,
            yaw_degrees: yaw,
            ..Command::default()
        },
        ..StepInput::default()
    }
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.0001,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn ground_command_basis_diagonal_normalization_and_release_friction() {
    let world = TestWorld::floor(0.0);
    let policy = tf2_policy();
    let configuration = Configuration::default();

    let forward = step(
        &world,
        player([0.0; 3], true, policy),
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(forward.state.velocity[0], 36.0);
    close(forward.state.position[0], 0.54);

    let side = step(
        &world,
        player([0.0; 3], true, policy),
        command(0.0, 450.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(side.state.velocity[1], -36.0);

    let diagonal = step(
        &world,
        player([0.0; 3], true, policy),
        command(450.0, 450.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(
        diagonal.state.velocity[0].hypot(diagonal.state.velocity[1]),
        36.0,
    );
    close(diagonal.state.velocity[0], -diagonal.state.velocity[1]);

    let released = step(
        &world,
        forward.state,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    close(released.state.velocity[0], 30.0);
}

#[test]
fn air_acceleration_uses_capped_projection_and_uncapped_magnitude() {
    let world = TestWorld::floor(0.0);
    let policy = tf2_policy();
    let configuration = Configuration::default();
    let mut initial = player([0.0, 0.0, 100.0], false, policy);
    initial.velocity = [30.0, 0.0, 0.0];

    let aligned = step(
        &world,
        initial,
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(aligned.state.velocity[0], 30.0);

    let perpendicular = step(
        &world,
        initial,
        command(450.0, 0.0, 90.0),
        configuration,
        policy,
    )
    .unwrap();
    close(perpendicular.state.velocity[0], 30.0);
    close(perpendicular.state.velocity[1], 30.0);

    let opposite = step(
        &world,
        initial,
        command(450.0, 0.0, 180.0),
        configuration,
        policy,
    )
    .unwrap();
    close(opposite.state.velocity[0], -6.0);

    let diagonal = step(
        &world,
        initial,
        command(450.0, 0.0, 45.0),
        configuration,
        policy,
    )
    .unwrap();
    close(diagonal.state.velocity[0], 36.213203);
    close(diagonal.state.velocity[1], 6.2132034);

    let rear_diagonal = step(
        &world,
        initial,
        command(450.0, 0.0, 135.0),
        configuration,
        policy,
    )
    .unwrap();
    close(rear_diagonal.state.velocity[0], 4.544155);
    close(rear_diagonal.state.velocity[1], 25.455845);
}

#[test]
fn ground_caps_surface_friction_and_tf2_backward_policy_are_explicit() {
    let world = TestWorld::floor(0.0);
    let mut policy = tf2_policy();
    let configuration = Configuration::default();
    let mut near_cap = player([0.0; 3], true, policy);
    near_cap.velocity = [230.0, 0.0, 0.0];
    let capped = step(
        &world,
        near_cap,
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(capped.state.velocity[0], 240.0);

    let mut backward = player([0.0; 3], true, policy);
    backward.velocity = [-240.0, 0.0, 0.0];
    let clamped = step(
        &world,
        backward,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    close(clamped.state.velocity[0], -216.0);

    policy.surface_friction = 0.5;
    let reduced = step(
        &world,
        player([0.0; 3], true, policy),
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(reduced.state.velocity[0], 18.0);
    assert_eq!(reduced.state.ground.unwrap().surface_friction, 0.5);

    policy.surface_friction = 1.0;
    let mut detached = player([0.0, 0.0, 100.0], true, policy);
    detached.velocity[2] = 251.0;
    let detached = step(
        &TestWorld::empty(),
        detached,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert!(detached.state.ground.is_none());
    assert_eq!(
        detached
            .queries
            .iter()
            .filter(|query| query.purpose == QueryPurpose::GroundFull)
            .count(),
        1
    );
}

#[test]
fn jump_latch_split_gravity_landing_and_repeat_hop_are_exact() {
    let world = TestWorld::floor(0.0);
    let policy = tf2_policy();
    let configuration = Configuration::default();
    let accelerated = step(
        &world,
        player([0.0; 3], true, policy),
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    let mut jump = command(450.0, 0.0, 0.0);
    jump.command.jump = true;
    let jumped = step(&world, accelerated.state, jump, configuration, policy).unwrap();
    close(jumped.state.velocity[0], 36.0);
    close(jumped.state.velocity[2], 271.0);
    close(jumped.state.position[2], 4.155);
    assert!(jumped.events.contains(&Event::Jumped));

    let held = step(&world, jumped.state, jump, configuration, policy).unwrap();
    assert!(!held.events.contains(&Event::Jumped));
    let mut state = step(
        &world,
        held.state,
        command(0.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap()
    .state;
    let mut landed = false;
    for _ in 0..80 {
        let next = step(&world, state, StepInput::default(), configuration, policy).unwrap();
        landed |= next
            .events
            .iter()
            .any(|event| matches!(event, Event::Landed { fall_speed } if *fall_speed > 0.0));
        state = next.state;
        if state.ground.is_some() {
            break;
        }
    }
    assert!(landed && state.ground.is_some() && !state.jump_latched);
    let hopped_again = step(&world, state, jump, configuration, policy).unwrap();
    assert!(hopped_again.events.contains(&Event::Jumped));
}

#[test]
fn walls_corners_steps_and_overheight_routes_are_bounded() {
    let policy = tf2_policy();
    let configuration = Configuration::default();
    let wall = TestWorld::empty().with_box([50.0, -20_000.0, -20_000.0], [20_000.0; 3], 2);
    let mut airborne = player([25.0, 0.0, 100.0], false, policy);
    airborne.velocity = [240.0, 100.0, 0.0];
    let slid = step(&wall, airborne, StepInput::default(), configuration, policy).unwrap();
    close(slid.state.position[0], 26.0);
    close(slid.state.velocity[0], 0.0);
    assert!(slid.state.position[1] > 0.0 && !slid.contacts.is_empty());

    let corner = TestWorld::empty()
        .with_box([50.0, -20_000.0, -20_000.0], [20_000.0; 3], 2)
        .with_box([-20_000.0, 50.0, -20_000.0], [20_000.0; 3], 3);
    let mut corner_state = player([25.0, 25.0, 100.0], false, policy);
    corner_state.velocity = [240.0, 240.0, 0.0];
    let creased = step(
        &corner,
        corner_state,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    close(creased.state.velocity[0], 0.0);
    close(creased.state.velocity[1], 0.0);
    assert!(creased.contacts.len() >= 2);

    let low_step =
        TestWorld::floor(0.0).with_box([50.0, -20_000.0, 0.0], [20_000.0, 20_000.0, 18.0], 4);
    let stepped = step(
        &low_step,
        player([25.9, 0.0, 0.0], true, policy),
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(stepped.state.position[2], 18.0);
    close(stepped.climbed_step, 18.0);

    let high_step =
        TestWorld::floor(0.0).with_box([50.0, -20_000.0, 0.0], [20_000.0, 20_000.0, 19.0], 5);
    let blocked = step(
        &high_step,
        player([25.9, 0.0, 0.0], true, policy),
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    assert!(blocked.state.position[0] <= 26.0001);
    close(blocked.climbed_step, 0.0);
}

struct QuadrantGround;

impl Tracer for QuadrantGround {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, _: u32) -> Result<Trace, Error> {
        if start == end {
            return Ok(Trace {
                fraction: 1.0,
                start_solid: false,
                all_solid: false,
                end,
                normal: None,
                hit: None,
                contents: 0,
            });
        }
        let full = hull.mins[0] < 0.0 && hull.maxs[0] > 0.0;
        Ok(Trace {
            fraction: 0.5,
            start_solid: false,
            all_solid: false,
            end: [start[0], start[1], start[2] + (end[2] - start[2]) * 0.5],
            normal: Some(if full {
                [0.8, 0.0, 0.6]
            } else {
                [0.0, 0.0, 1.0]
            }),
            hit: Some(if full { 7 } else { 8 }),
            contents: 1,
        })
    }
}

struct Ramp;

impl Tracer for Ramp {
    fn trace(&self, start: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
        let normal = [-0.447_213_6, 0.0, 0.894_427_2];
        let start_distance = start[0] * normal[0] + start[2] * normal[2];
        let end_distance = end[0] * normal[0] + end[2] * normal[2];
        let start_solid = start_distance < 0.0;
        let all_solid = start_solid && end_distance < 0.0;
        if !start_solid && end_distance < 0.0 {
            let fraction = start_distance / (start_distance - end_distance);
            Ok(Trace {
                fraction,
                start_solid: false,
                all_solid: false,
                end: [
                    start[0] + (end[0] - start[0]) * fraction,
                    start[1] + (end[1] - start[1]) * fraction,
                    start[2] + (end[2] - start[2]) * fraction,
                ],
                normal: Some(normal),
                hit: Some(12),
                contents: 1,
            })
        } else {
            Ok(Trace {
                fraction: 1.0,
                start_solid,
                all_solid,
                end,
                normal: None,
                hit: all_solid.then_some(12),
                contents: u32::from(start_solid),
            })
        }
    }
}

#[test]
fn steep_full_hull_uses_standable_quadrant_support() {
    let policy = tf2_policy();
    let result = step(
        &QuadrantGround,
        player([0.0; 3], false, policy),
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(result.state.ground.unwrap().support, Some(8));
    assert!(
        result
            .queries
            .iter()
            .any(|query| query.purpose == QueryPurpose::GroundQuadrant)
    );
}

#[test]
fn standable_ramp_clips_without_tunnelling_and_becomes_support() {
    let mut policy = tf2_policy();
    policy.standing_hull = Hull {
        mins: [0.0; 3],
        maxs: [0.0; 3],
    };
    policy.crouched_hull = policy.standing_hull;
    let mut state = player([0.0, 0.0, 3.0], false, policy);
    state.velocity = [100.0, 0.0, -300.0];
    let result = step(
        &Ramp,
        state,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert!(
        result
            .contacts
            .iter()
            .any(|contact| { contact.hit == Some(12) && contact.normal[2] >= 0.7 })
    );
    assert!(result.state.position[2] >= result.state.position[0] * 0.5 - 0.0001);
    assert_eq!(result.state.ground.unwrap().support, Some(12));
}

#[test]
fn crouch_transitions_reverse_switch_air_hulls_and_retry_blocked_unduck() {
    let world = TestWorld::floor(0.0);
    let policy = tf2_policy();
    let configuration = Configuration::default();
    let mut crouch = command(0.0, 0.0, 0.0);
    crouch.command.crouch = true;
    let pressed = step(
        &world,
        player([0.0; 3], true, policy),
        crouch,
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(pressed.state.crouch.phase, CrouchPhase::Ducking);
    close(pressed.state.crouch.fraction, 0.0);

    let mut state = pressed.state;
    for _ in 0..5 {
        state = step(&world, state, crouch, configuration, policy)
            .unwrap()
            .state;
    }
    assert!(state.crouch.fraction > 0.0 && state.crouch.fraction < 1.0);
    let partial = state.crouch.fraction;
    let released = step(&world, state, StepInput::default(), configuration, policy).unwrap();
    assert_eq!(released.state.crouch.phase, CrouchPhase::Unducking);
    close(released.state.crouch.fraction, partial);
    let reversed = step(&world, released.state, crouch, configuration, policy).unwrap();
    assert_eq!(reversed.state.crouch.phase, CrouchPhase::Ducking);
    close(reversed.state.crouch.fraction, partial);

    state = reversed.state;
    for _ in 0..20 {
        state = step(&world, state, crouch, configuration, policy)
            .unwrap()
            .state;
        if state.crouch.phase == CrouchPhase::Crouched {
            break;
        }
    }
    assert_eq!(state.crouch.phase, CrouchPhase::Crouched);
    close(state.view_offset[2], 45.0);
    let cropped = step(
        &world,
        state,
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(cropped.state.velocity[0], 12.0);

    let mut duck_jump = command(0.0, 0.0, 0.0);
    duck_jump.command.crouch = true;
    duck_jump.command.jump = true;
    let duck_jumped = step(
        &world,
        player([0.0; 3], true, policy),
        duck_jump,
        configuration,
        policy,
    )
    .unwrap();
    assert!(duck_jumped.events.contains(&Event::Jumped));
    close(duck_jumped.state.velocity[2], 277.0);

    let mut fully_crouched = cropped.state;
    fully_crouched.velocity = [0.0; 3];
    fully_crouched.crouch = CrouchState::CROUCHED;
    fully_crouched.view_offset = policy.crouched_view;
    fully_crouched.previous_crouch = true;
    fully_crouched.jump_latched = false;
    let blocked_jump = step(&world, fully_crouched, duck_jump, configuration, policy).unwrap();
    assert!(!blocked_jump.events.contains(&Event::Jumped));
    assert!(!blocked_jump.state.jump_latched);

    let air = TestWorld::empty();
    let air_duck = step(
        &air,
        player([0.0, 0.0, 100.0], false, policy),
        crouch,
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(air_duck.state.crouch.phase, CrouchPhase::Crouched);
    close(air_duck.state.position[2], 120.0 - 0.09);
    let air_stand = step(
        &air,
        air_duck.state,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(air_stand.state.crouch.phase, CrouchPhase::Standing);

    let ceiling = TestWorld::floor(0.0).with_ceiling(70.0);
    let mut blocked_state = player([0.0; 3], true, policy);
    blocked_state.crouch = CrouchState::CROUCHED;
    blocked_state.view_offset = policy.crouched_view;
    let blocked = step(
        &ceiling,
        blocked_state,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(blocked.state.crouch.phase, CrouchPhase::Blocked);
    ceiling.ceiling_enabled.set(false);
    let retry = step(
        &ceiling,
        blocked.state,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(retry.state.crouch.phase, CrouchPhase::Unducking);
}

#[test]
fn noclip_modes_cover_pitch_speed_button_direct_motion_and_no_queries() {
    let world = TestWorld::empty();
    let policy = tf2_policy();
    let configuration = Configuration::default();
    let request = ModeRequest {
        mode: Mode::Noclip,
        disposition: TransitionDisposition::RESET_ENVIRONMENT,
    };
    let mut input = command(450.0, 0.0, 0.0);
    input.mode_request = Some(request);
    let accelerated = step(
        &world,
        player([0.0; 3], false, policy),
        input,
        configuration,
        policy,
    )
    .unwrap();
    close(accelerated.state.velocity[0], 96.0);
    close(accelerated.state.position[0], 1.44);
    assert!(accelerated.queries.is_empty() && world.calls.get() == 0);

    input.speed_button = true;
    let speed_button = step(
        &world,
        player([0.0; 3], false, policy),
        input,
        configuration,
        policy,
    )
    .unwrap();
    close(speed_button.state.velocity[0], 60.375);
    close(speed_button.state.position[0], 0.905625);

    let mut pitched = input;
    pitched.speed_button = false;
    pitched.pitch_degrees = -45.0;
    let pitched = step(
        &world,
        player([0.0; 3], false, policy),
        pitched,
        configuration,
        policy,
    )
    .unwrap();
    assert!(pitched.state.position[0] > 0.0 && pitched.state.position[2] > 0.0);

    let direct_configuration = Configuration {
        noclip_acceleration: 0.0,
        ..configuration
    };
    input.speed_button = false;
    let direct = step(
        &world,
        player([0.0; 3], false, policy),
        input,
        direct_configuration,
        policy,
    )
    .unwrap();
    close(direct.state.velocity[0], 1600.0);
    close(direct.state.position[0], 24.0);

    let negative_configuration = Configuration {
        noclip_acceleration: -1.0,
        ..configuration
    };
    let negative = step(
        &world,
        player([0.0; 3], false, policy),
        input,
        negative_configuration,
        policy,
    )
    .unwrap();
    close(negative.state.position[0], 24.0);
    assert_eq!(negative.state.velocity, [0.0; 3]);

    let exit = StepInput {
        mode_request: Some(ModeRequest {
            mode: Mode::Walk,
            disposition: TransitionDisposition {
                velocity: StateDisposition::Reset,
                ground: StateDisposition::Reset,
                water: StateDisposition::Preserve,
            },
        }),
        ..StepInput::default()
    };
    let exited = step(&world, accelerated.state, exit, configuration, policy).unwrap();
    assert_eq!(exited.state.mode, Mode::Walk);
    assert_eq!(exited.state.velocity, [0.0, 0.0, -12.0]);
    assert!(exited.events.iter().any(|event| matches!(
        event,
        Event::ModeChanged {
            from: Mode::Noclip,
            to: Mode::Walk,
            ..
        }
    )));
}

#[test]
fn initial_overlap_recovers_in_declared_order_and_all_solid_traps() {
    let mut point_policy = tf2_policy();
    point_policy.standing_hull = Hull {
        mins: [0.0; 3],
        maxs: [0.0; 3],
    };
    point_policy.crouched_hull = point_policy.standing_hull;
    let recoverable = TestWorld::empty().with_box([-0.2; 3], [0.2; 3], 10);
    let recovered = step(
        &recoverable,
        player([0.0; 3], false, point_policy),
        StepInput::default(),
        Configuration::default(),
        point_policy,
    )
    .unwrap();
    assert!(recovered.events.contains(&Event::Recovered { offset: 18 }));
    close(recovered.state.position[2], 0.2);

    let trapped_world = TestWorld::empty().with_box([-100.0; 3], [100.0; 3], 11);
    let trapped = step(
        &trapped_world,
        player([0.0; 3], false, point_policy),
        StepInput::default(),
        Configuration::default(),
        point_policy,
    )
    .unwrap();
    assert!(trapped.events.contains(&Event::Trapped));
    assert_eq!(trapped.state.velocity, [0.0; 3]);
}

#[test]
fn snapshots_authority_speculation_repeats_and_failures_are_deterministic() {
    let world = TestWorld::floor(0.0);
    let policy = tf2_policy();
    let configuration = Configuration::default();
    let initial = player([0.0; 3], true, policy);
    let input = command(450.0, 450.0, 37.0);
    let authoritative = step(&world, initial, input, configuration, policy).unwrap();
    let speculative = step(&world, initial, input, configuration, policy).unwrap();
    assert_eq!(authoritative, speculative);
    assert_eq!(
        authoritative.state.snapshot_bytes(),
        speculative.state.snapshot_bytes()
    );
    assert_eq!(
        &authoritative.state.snapshot_bytes()[..8],
        b"PMOV\x01\0\0\0"
    );
    for _ in 0..1_024 {
        let repeated = step(&world, initial, input, configuration, policy).unwrap();
        assert_eq!(
            repeated.state.snapshot_bytes(),
            authoritative.state.snapshot_bytes()
        );
    }

    let malformed = StepInput {
        command_number: 7,
        command: Command {
            forward: f32::NAN,
            ..Command::default()
        },
        ..StepInput::default()
    };
    assert_eq!(
        step(&world, initial, malformed, configuration, policy),
        Err(
            Error::new(Operation::Validate, FailureKind::Malformed, "forward")
                .with_value(f32::NAN, Some(MAX_COMMAND_MAGNITUDE))
                .with_command(7)
        )
    );
}
