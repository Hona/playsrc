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

    fn movement_time(&self) -> Option<f32> {
        Some(1.0)
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
        water: WaterPolicy::default(),
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
    assert!(
        accelerated
            .queries
            .iter()
            .all(|query| query.purpose != QueryPurpose::Displacement)
    );

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
    close(recovered.state.position[2], 0.91);

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

struct WaterWorld {
    surface: f32,
    current: u32,
}

impl Tracer for WaterWorld {
    fn trace(&self, _start: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }

    fn point_contents(&self, point: [f32; 3]) -> Result<u32, Error> {
        Ok(if point[2] < self.surface {
            CONTENTS_WATER | self.current
        } else {
            0
        })
    }

    fn movement_time(&self) -> Option<f32> {
        Some(1.0)
    }
}

fn source_tf2_water_policy() -> Policy {
    Policy {
        water: WaterPolicy {
            sampling: WaterSampling::EyesThenWaist,
            waist_height_offset: 12.0,
            refresh_before_walk: false,
            apply_currents: false,
            jump_wish_at_waist: false,
            amplify_forward_pitch: false,
            ledge_uses_command_direction: true,
            ledge_jump_overrides_backward: true,
            suppress_airborne_duck: true,
            suppress_submerged_duck: true,
        },
        ..tf2_policy()
    }
}

#[test]
fn game_selected_water_samples_normalize_type_and_preserve_generic_currents() {
    let policy = source_tf2_water_policy();
    let configuration = Configuration {
        optimized_movement: false,
        ..Configuration::default()
    };
    for (surface, expected_level, expected_queries) in [
        (-10.0, 0, vec![PointQueryPurpose::WaterFeet]),
        (
            10.0,
            1,
            vec![
                PointQueryPurpose::WaterFeet,
                PointQueryPurpose::WaterEyes,
                PointQueryPurpose::WaterWaist,
            ],
        ),
        (
            50.0,
            1,
            vec![
                PointQueryPurpose::WaterFeet,
                PointQueryPurpose::WaterEyes,
                PointQueryPurpose::WaterWaist,
            ],
        ),
        (
            60.0,
            2,
            vec![
                PointQueryPurpose::WaterFeet,
                PointQueryPurpose::WaterEyes,
                PointQueryPurpose::WaterWaist,
            ],
        ),
        (
            80.0,
            3,
            vec![PointQueryPurpose::WaterFeet, PointQueryPurpose::WaterEyes],
        ),
    ] {
        let world = WaterWorld {
            surface,
            current: 0x1000_0000 | CONTENTS_CURRENT_0,
        };
        let result = step(
            &world,
            player([0.0; 3], false, policy),
            StepInput::default(),
            configuration,
            policy,
        )
        .unwrap();
        assert_eq!(
            result.state.water_level, expected_level,
            "surface={surface}"
        );
        assert_eq!(
            result
                .point_queries
                .iter()
                .take(expected_queries.len())
                .map(|query| query.purpose)
                .collect::<Vec<_>>(),
            expected_queries,
            "surface={surface}"
        );
        if expected_level != 0 {
            assert_eq!(result.state.water_type, CONTENTS_WATER);
            assert_eq!(result.state.base_velocity, [0.0; 3]);
            let eye = &result.point_queries[1];
            assert_eq!(eye.point[2], 68.0);
            if expected_level != 3 {
                let waist = &result.point_queries[2];
                assert_eq!(waist.point[2], 53.0);
            }
        }
    }

    let generic_policy = tf2_policy();
    let generic = step(
        &WaterWorld {
            surface: 50.0,
            current: CONTENTS_CURRENT_0,
        },
        player([0.0; 3], false, generic_policy),
        StepInput::default(),
        Configuration::default(),
        generic_policy,
    )
    .unwrap();
    assert_eq!(generic.state.water_level, 2);
    assert_eq!(generic.state.water_type, CONTENTS_WATER);
    assert_eq!(
        generic.point_queries[1].purpose,
        PointQueryPurpose::WaterWaist
    );
    assert_eq!(generic.point_queries[1].point[2], 41.0);

    let generic_current = step(
        &WaterWorld {
            surface: 200.0,
            current: CONTENTS_CURRENT_0,
        },
        player([0.0; 3], false, generic_policy),
        StepInput::default(),
        Configuration::default(),
        generic_policy,
    )
    .unwrap();
    assert!(generic_current.state.base_velocity[0] > 0.0);
}

#[test]
fn game_selected_water_jump_and_pitched_swim_use_class_speed_and_exact_levels() {
    let policy = source_tf2_water_policy();
    let mut state = player([0.0, 0.0, 10.0], false, policy);
    state.water_level = 3;
    state.water_type = CONTENTS_WATER;
    state.velocity = [0.0, 0.0, 900.0];
    let jumped = step(
        &WaterWorld {
            surface: 200.0,
            current: 0,
        },
        state,
        StepInput {
            command: Command {
                jump: true,
                ..Command::default()
            },
            ..StepInput::default()
        },
        Configuration::default(),
        policy,
    )
    .unwrap();
    close(jumped.state.velocity[2], 122.8);
    close(jumped.wish_state.speed, 192.0);

    let mut waist = player([0.0; 3], false, policy);
    waist.water_level = 2;
    waist.water_type = CONTENTS_WATER;
    waist.velocity = [0.0, 0.0, 900.0];
    let waist_jump = step(
        &WaterWorld {
            surface: 60.0,
            current: 0,
        },
        waist,
        StepInput {
            command: Command {
                jump: true,
                ..Command::default()
            },
            ..StepInput::default()
        },
        Configuration::default(),
        policy,
    )
    .unwrap();
    close(waist_jump.state.velocity[2], 94.0);
    close(waist_jump.wish_state.speed, 0.0);

    let mut swimming = player([0.0; 3], false, policy);
    swimming.water_level = 3;
    swimming.water_type = CONTENTS_WATER;
    let pitched = step(
        &WaterWorld {
            surface: 200.0,
            current: 0,
        },
        swimming,
        StepInput {
            command: Command {
                forward: 240.0,
                ..Command::default()
            },
            pitch_degrees: -30.0,
            ..StepInput::default()
        },
        Configuration::default(),
        policy,
    )
    .unwrap();
    close(pitched.state.velocity[0], 24.94153);
    close(pitched.state.velocity[2], 14.4);

    let generic_policy = tf2_policy();
    let mut generic_swimming = player([0.0; 3], false, generic_policy);
    generic_swimming.water_level = 3;
    generic_swimming.water_type = CONTENTS_WATER;
    let generic = step(
        &WaterWorld {
            surface: 200.0,
            current: 0,
        },
        generic_swimming,
        StepInput {
            command: Command {
                forward: 240.0,
                ..Command::default()
            },
            pitch_degrees: -30.0,
            ..StepInput::default()
        },
        Configuration::default(),
        generic_policy,
    )
    .unwrap();
    close(generic.state.velocity[0], 14.4);
    close(generic.state.velocity[2], 24.94153);
}

#[test]
fn game_selected_water_transitions_preserve_prior_classification_and_half_gravity() {
    let policy = source_tf2_water_policy();
    let entered = step(
        &WaterWorld {
            surface: 200.0,
            current: 0,
        },
        player([0.0, 0.0, 10.0], false, policy),
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(entered.state.water_level, 3);
    close(entered.state.velocity[2], -6.0);
    close(entered.state.position[2], 9.91);
    assert_eq!(
        entered.point_queries[0].purpose,
        PointQueryPurpose::WaterFeet
    );
    assert_eq!(
        entered.point_queries[1].purpose,
        PointQueryPurpose::WaterEyes
    );

    let mut submerged = player([0.0, 0.0, 10.0], false, policy);
    submerged.water_level = 3;
    submerged.water_type = CONTENTS_WATER;
    submerged.velocity[2] = 100.0;
    let exited = step(
        &WaterWorld {
            surface: -1.0,
            current: 0,
        },
        submerged,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(exited.state.water_level, 0);
    close(exited.state.velocity[2], 94.0);
}

#[test]
fn game_selected_water_rejects_airborne_and_eye_depth_crouch_only() {
    let policy = source_tf2_water_policy();
    for (level, grounded, expected_crouch) in [
        (0, false, true),
        (1, false, false),
        (2, false, false),
        (3, false, false),
        (1, true, true),
        (2, true, true),
        (3, true, false),
    ] {
        let mut state = player([0.0; 3], grounded, policy);
        state.water_level = level;
        state.water_type = if level == 0 { 0 } else { CONTENTS_WATER };
        let result = step(
            &WaterWorld {
                surface: 200.0,
                current: 0,
            },
            state,
            StepInput {
                command: Command {
                    crouch: true,
                    ..Command::default()
                },
                ..StepInput::default()
            },
            Configuration::default(),
            policy,
        )
        .unwrap();
        assert_eq!(
            result.state.previous_crouch, expected_crouch,
            "level={level}, grounded={grounded}"
        );
        if !expected_crouch {
            assert!(!result.state.crouch.uses_crouched_hull());
        }
    }
}

#[test]
fn water_samples_currents_swim_and_transition_trace_are_explicit() {
    let world = WaterWorld {
        surface: 200.0,
        current: CONTENTS_CURRENT_0,
    };
    let policy = tf2_policy();
    let mut state = player([0.0, 0.0, 10.0], false, policy);
    state.velocity = [0.0; 3];
    let result = step(
        &world,
        state,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(result.state.water_level, 3);
    assert_eq!(result.state.water_type & CONTENTS_WATER, CONTENTS_WATER);
    assert!(result.state.base_velocity[0] > 0.0);
    assert!(result.state.velocity[2] < 0.0);
    assert!(result.events.iter().any(|event| matches!(
        event,
        Event::WaterEntered {
            level: 3,
            contents
        } if contents & CONTENTS_WATER != 0
    )));
    assert_eq!(
        result
            .point_queries
            .iter()
            .take(3)
            .map(|query| query.purpose)
            .collect::<Vec<_>>(),
        [
            PointQueryPurpose::WaterFeet,
            PointQueryPurpose::WaterWaist,
            PointQueryPurpose::WaterEyes,
        ]
    );

    let mut jump = StepInput::default();
    jump.command.jump = true;
    let jumped = step(&world, result.state, jump, Configuration::default(), policy).unwrap();
    assert!(jumped.state.velocity[2] > 0.0);
    assert!(jumped.state.ground.is_none());
}

struct WaterExitWorld;

impl Tracer for WaterExitWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
        let delta = subtract(end, start);
        if delta[2] < -500.0 {
            let fraction = 0.1;
            return Ok(Trace {
                fraction,
                start_solid: false,
                all_solid: false,
                end: [
                    start[0] + delta[0] * fraction,
                    start[1] + delta[1] * fraction,
                    start[2] + delta[2] * fraction,
                ],
                normal: Some([0.0, 0.0, 1.0]),
                hit: Some(1),
                contents: 1,
            });
        }
        if delta[2] == 0.0 && delta[0].abs() >= 23.9 && (30.0..60.0).contains(&start[2]) {
            let fraction = 0.5;
            return Ok(Trace {
                fraction,
                start_solid: false,
                all_solid: false,
                end: [start[0] + delta[0] * fraction, start[1], start[2]],
                normal: Some([-1.0, 0.0, 0.0]),
                hit: Some(2),
                contents: 1,
            });
        }
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }

    fn point_contents(&self, point: [f32; 3]) -> Result<u32, Error> {
        Ok(if point[2] < 60.0 { CONTENTS_WATER } else { 0 })
    }

    fn movement_time(&self) -> Option<f32> {
        Some(1.0)
    }
}

#[test]
fn game_selected_water_ledge_uses_command_direction_and_exact_tf2_impulse() {
    let policy = source_tf2_water_policy();
    let configuration = Configuration {
        water_exit_forward: 30.0,
        water_exit_up_speed: 300.0,
        ..Configuration::default()
    };
    let mut state = player([0.0; 3], false, policy);
    state.water_level = 2;
    state.water_type = CONTENTS_WATER;
    state.velocity = [10.0, 0.0, 0.0];
    let result = step(
        &WaterExitWorld,
        state,
        command(240.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    let waist = result
        .queries
        .iter()
        .find(|query| query.purpose == QueryPurpose::WaterWaist)
        .unwrap();
    close(waist.end[0] - waist.start[0], 30.0);
    close(result.state.velocity[2], 282.0);
    assert_eq!(result.state.water_jump_velocity, [50.0, 0.0, 0.0]);
    assert_eq!(result.state.water_jump_time_ms, 2_000.0);

    let without_direction = step(
        &WaterExitWorld,
        state,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(without_direction.state.water_jump_time_ms, 0.0);

    let mut backward = state;
    backward.velocity[0] = -10.0;
    let rejected = step(
        &WaterExitWorld,
        backward,
        command(240.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(rejected.state.water_jump_time_ms, 0.0);

    let admitted = step(
        &WaterExitWorld,
        backward,
        StepInput {
            command: Command {
                forward: 240.0,
                jump: true,
                ..Command::default()
            },
            ..StepInput::default()
        },
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(admitted.state.water_jump_time_ms, 2_000.0);
}

#[test]
fn water_ledge_exit_requires_waist_eye_and_landing_queries() {
    let policy = tf2_policy();
    let mut state = player([0.0; 3], false, policy);
    state.velocity = [10.0, 0.0, 0.0];
    let result = step(
        &WaterExitWorld,
        state,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert!(result.state.water_jump_time_ms > 0.0);
    assert_eq!(result.state.water_jump_velocity, [50.0, 0.0, 0.0]);
    assert!(
        result
            .queries
            .iter()
            .any(|query| query.purpose == QueryPurpose::WaterWaist)
    );
    assert!(
        result
            .queries
            .iter()
            .any(|query| query.purpose == QueryPurpose::WaterEye)
    );
    assert!(
        result
            .queries
            .iter()
            .any(|query| query.purpose == QueryPurpose::WaterLanding)
    );
}

struct LadderWorld;

impl Tracer for LadderWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], _: Hull, mask: u32) -> Result<Trace, Error> {
        let delta = subtract(end, start);
        if mask & CONTENTS_LADDER != 0 && delta[2] == 0.0 && (delta[0].abs() - 2.0).abs() < 0.001 {
            return Ok(Trace {
                fraction: 0.5,
                start_solid: false,
                all_solid: false,
                end: [start[0] + delta[0] * 0.5, start[1], start[2]],
                normal: Some([-1.0, 0.0, 0.0]),
                hit: Some(4),
                contents: CONTENTS_LADDER,
            });
        }
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }

    fn movement_time(&self) -> Option<f32> {
        Some(1.0)
    }
}

#[test]
fn ladder_attach_climb_and_jump_away_follow_one_mode_transition() {
    let policy = tf2_policy();
    let state = player([0.0, 0.0, 100.0], false, policy);
    let climbed = step(
        &LadderWorld,
        state,
        command(450.0, 0.0, 0.0),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(climbed.state.mode, Mode::Ladder);
    assert!(climbed.state.velocity[2] > 0.0);
    assert!(climbed.events.contains(&Event::LadderAttached));

    let mut jump = command(450.0, 0.0, 0.0);
    jump.command.jump = true;
    let detached = step(
        &LadderWorld,
        climbed.state,
        jump,
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(detached.state.mode, Mode::Walk);
    assert!(detached.state.velocity[0] < 0.0);
    assert!(detached.events.contains(&Event::LadderDetached));
}

struct ObserverWorld;

impl Tracer for ObserverWorld {
    fn trace(&self, _: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }

    fn observer_target(&self, target: u64) -> Result<Option<ObserverTarget>, Error> {
        Ok((target == 9).then_some(ObserverTarget {
            position: [10.0, 20.0, 30.0],
            angles: [4.0, 5.0, 6.0],
            velocity: [7.0, 8.0, 9.0],
        }))
    }
}

#[test]
fn observer_fixed_follow_and_roaming_dispositions_are_distinct() {
    let policy = tf2_policy();
    let configuration = Configuration {
        observer_hull: Some(Hull {
            mins: [-10.0; 3],
            maxs: [10.0; 3],
        }),
        ..Configuration::default()
    };
    let mut follow = player([1.0, 2.0, 3.0], false, policy);
    follow.mode = Mode::Observer;
    follow.observer_mode = ObserverMode::InEye;
    follow.observer_target = Some(9);
    let followed = step(
        &ObserverWorld,
        follow,
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(followed.state.position, [10.0, 20.0, 30.0]);
    assert_eq!(followed.state.velocity, [7.0, 8.0, 9.0]);

    let mut fixed = follow;
    fixed.observer_mode = ObserverMode::Fixed;
    let fixed_result = step(
        &ObserverWorld,
        fixed,
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    assert_eq!(fixed_result.state.position, fixed.position);

    let mut roaming = follow;
    roaming.observer_mode = ObserverMode::Roaming;
    let roaming_result = step(
        &ObserverWorld,
        roaming,
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    assert!(roaming_result.state.position[0] > roaming.position[0]);
    assert!(
        roaming_result
            .queries
            .iter()
            .all(|query| query.purpose != QueryPurpose::ObserverDisplacement)
    );
}

struct ConveyorWorld(TestWorld);

impl Tracer for ConveyorWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<Trace, Error> {
        self.0.trace(start, end, hull, mask)
    }

    fn conveyor_velocity(&self, support: u64) -> Result<Option<[f32; 3]>, Error> {
        Ok((support == 5).then_some([100.0, 0.0, 0.0]))
    }

    fn support_velocity(&self, support: u64) -> Result<[f32; 3], Error> {
        Ok(if support == 5 {
            [100.0, 0.0, 0.0]
        } else {
            [0.0; 3]
        })
    }

    fn movement_time(&self) -> Option<f32> {
        Some(1.0)
    }
}

#[test]
fn conveyor_and_retained_base_velocity_are_applied_once() {
    let policy = tf2_policy();
    let mut state = player([0.0; 3], true, policy);
    state.ground.as_mut().unwrap().support = Some(5);
    let carried = step(
        &ConveyorWorld(TestWorld::floor(0.0)),
        state,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    close(carried.state.position[0], 1.5);
    close(carried.state.velocity[0], 0.0);

    let mut retained = player([0.0, 0.0, 100.0], false, policy);
    retained.base_velocity = [100.0, 0.0, 0.0];
    let momentum = step(
        &TestWorld::empty(),
        retained,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    close(momentum.state.base_velocity[0], 0.0);
    assert!(momentum.state.velocity[0] > 100.0);
}

struct MoverWorld {
    blocked: bool,
}

impl Tracer for MoverWorld {
    fn trace(&self, _: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }

    fn support_velocity(&self, support: u64) -> Result<[f32; 3], Error> {
        Ok(if support == 7 {
            [100.0, 0.0, 0.0]
        } else {
            [0.0; 3]
        })
    }

    fn mover_motion(
        &self,
        _: [f32; 3],
        _: Hull,
        _: Option<u64>,
    ) -> Result<Option<MoverMotion>, Error> {
        Ok(Some(MoverMotion {
            identity: 7,
            displacement: [10.0, 0.0, 0.0],
            linear_velocity: [100.0, 0.0, 0.0],
            angular_velocity: [0.0, 10.0, 0.0],
            swept_contact: true,
            unblockable: false,
        }))
    }

    fn trace_without(
        &self,
        _: u64,
        start: [f32; 3],
        end: [f32; 3],
        _: Hull,
        _: u32,
    ) -> Result<Trace, Error> {
        if self.blocked {
            Ok(Trace {
                fraction: 0.5,
                start_solid: false,
                all_solid: false,
                end: [
                    (start[0] + end[0]) * 0.5,
                    (start[1] + end[1]) * 0.5,
                    (start[2] + end[2]) * 0.5,
                ],
                normal: Some([-1.0, 0.0, 0.0]),
                hit: Some(99),
                contents: 1,
            })
        } else {
            self.trace(
                start,
                end,
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                0,
            )
        }
    }

    fn movement_time(&self) -> Option<f32> {
        Some(1.0)
    }
}

#[test]
fn mover_carry_reports_acceptance_or_blocker_without_mutating_mover_state() {
    let policy = tf2_policy();
    let mut state = player([0.0; 3], true, policy);
    state.ground.as_mut().unwrap().support = Some(7);
    let moved = step(
        &MoverWorld { blocked: false },
        state,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert!(moved.state.position[0] >= 10.0);
    assert_eq!(moved.mover_result.unwrap().status, MoverStatus::Moved);
    assert!(moved.state.local_angles[1] > 0.0);

    let blocked = step(
        &MoverWorld { blocked: true },
        state,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    assert_eq!(blocked.mover_result.unwrap().status, MoverStatus::Blocked);
    assert_eq!(blocked.mover_result.unwrap().blocker, Some(99));
}

#[test]
fn constraints_toss_modes_and_tick_trace_have_fixed_outputs() {
    let policy = tf2_policy();
    let configuration = Configuration {
        client_max_speed: 100.0,
        surface_max_speed_factor: 0.5,
        movement_constraint: Some(MovementConstraint {
            center: [-10.0, 0.0, 0.0],
            radius: 20.0,
            width: 20.0,
            outward_speed_factor: 0.5,
        }),
        ..Configuration::default()
    };
    let constrained = step(
        &TestWorld::floor(0.0),
        player([0.0; 3], true, policy),
        command(450.0, 0.0, 0.0),
        configuration,
        policy,
    )
    .unwrap();
    close(constrained.wish_state.speed, 50.0);
    let trace = constrained.tick_trace(77);
    assert_eq!(trace.command_number, 77);
    assert_eq!(trace.position, constrained.state.position);
    assert_eq!(trace.hull, policy.standing_hull);
    assert_eq!(trace.contacts, constrained.contacts);

    let mut flying = player([0.0, 0.0, 100.0], false, policy);
    flying.mode = Mode::FlyGravity;
    let tossed = step(
        &TestWorld::empty(),
        flying,
        StepInput::default(),
        Configuration::default(),
        policy,
    )
    .unwrap();
    close(tossed.state.velocity[2], -12.0);
    close(tossed.state.position[2], 99.82);

    flying.move_collision = MoveCollision::Custom;
    assert_eq!(
        step(
            &TestWorld::empty(),
            flying,
            StepInput::default(),
            Configuration::default(),
            policy,
        ),
        Err(Error::new(
            Operation::Move,
            FailureKind::Unsupported,
            "fly-collision-response"
        )
        .with_command(0))
    );
}

#[test]
fn incremental_stuck_recovery_requires_an_explicit_monotonic_clock() {
    let mut policy = tf2_policy();
    policy.standing_hull = Hull {
        mins: [0.0; 3],
        maxs: [0.0; 3],
    };
    policy.crouched_hull = policy.standing_hull;
    let world = TestWorld::empty().with_box([-100.0; 3], [100.0; 3], 11);
    let configuration = Configuration {
        stuck_recovery: StuckRecoveryMode::Incremental,
        ..Configuration::default()
    };
    let result = step(
        &world,
        player([0.0; 3], false, policy),
        StepInput::default(),
        configuration,
        policy,
    )
    .unwrap();
    assert!(result.events.contains(&Event::Trapped));
    assert_eq!(result.state.stuck_offset, 1);

    struct NoClock(TestWorld);
    impl Tracer for NoClock {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            hull: Hull,
            mask: u32,
        ) -> Result<Trace, Error> {
            self.0.trace(start, end, hull, mask)
        }
    }
    assert!(matches!(
        step(
            &NoClock(TestWorld::empty().with_box([-100.0; 3], [100.0; 3], 11)),
            player([0.0; 3], false, policy),
            StepInput::default(),
            configuration,
            policy,
        ),
        Err(Error {
            kind: FailureKind::Missing,
            field: "stuck-movement-time",
            ..
        })
    ));
}
