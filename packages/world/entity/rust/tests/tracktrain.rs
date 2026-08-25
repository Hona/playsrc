use playsrc_entity::{
    BehaviorState, EntityHandle, EntityWorld, EntityWorldConfig, EventTarget, InputRecord,
    ModelBounds, RuntimeRequest, Transform, Transition, Variant, WorldCommand, parse,
};

const TICK_INTERVAL: f32 = 0.015;

fn compile(source: &[u8]) -> EntityWorld {
    let graph = parse(source, playsrc_entity::Limits::default()).unwrap();
    EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            tick_interval: TICK_INTERVAL,
            model_bounds: vec![ModelBounds {
                model: 1,
                mins: [-8.0; 3],
                maxs: [8.0; 3],
            }],
            ..EntityWorldConfig::default()
        },
    )
    .unwrap()
    .0
}

fn handle(world: &EntityWorld, name: &[u8]) -> EntityHandle {
    world.resolve(name, None, None, None)[0]
}

fn input(entity: EntityHandle, name: &[u8], value: Variant) -> WorldCommand {
    WorldCommand::Input(InputRecord {
        target: EventTarget::Direct(entity),
        input: name.to_vec(),
        value,
        activator: None,
        caller: None,
        output_action: None,
        producer_sequence: 1,
    })
}

fn train_path(world: &EntityWorld, train: EntityHandle) -> &playsrc_entity::TrainPathState {
    let BehaviorState::Mover(mover) = &world.entity(train).unwrap().behavior else {
        panic!("track train mover is unavailable")
    };
    mover.path.as_ref().unwrap()
}

fn mover_request(batch: &playsrc_entity::TransitionBatch, train: EntityHandle) -> RuntimeRequest {
    batch
        .records
        .iter()
        .find_map(|record| match &record.transition {
            Transition::Request(request @ RuntimeRequest::Mover { entity, speed, .. })
                if *entity == train && *speed > 0.0 =>
            {
                Some(request.clone())
            }
            _ => None,
        })
        .expect("track train movement request")
}

fn apply_tick(
    world: &mut EntityWorld,
    tick: u64,
    train: EntityHandle,
    request: RuntimeRequest,
) -> playsrc_entity::TransitionBatch {
    let RuntimeRequest::Mover {
        request_id,
        world_destination,
        world_angles_destination,
        angular_velocity,
        speed,
        ..
    } = request
    else {
        panic!("track train request kind")
    };
    let current = world.entity(train).unwrap().world_transform;
    let delta = [
        world_destination[0] - current.origin[0],
        world_destination[1] - current.origin[1],
        world_destination[2] - current.origin[2],
    ];
    let distance = delta.iter().map(|value| value * value).sum::<f32>().sqrt();
    let fraction = if distance == 0.0 {
        1.0
    } else {
        (speed * TICK_INTERVAL / distance).min(1.0)
    };
    let mut commands = vec![WorldCommand::SetWorldTransform {
        entity: train,
        transform: Transform {
            origin: std::array::from_fn(|axis| current.origin[axis] + delta[axis] * fraction),
            angles: if angular_velocity == [0.0; 3] {
                current.angles
            } else {
                world_angles_destination
            },
        },
    }];
    if fraction >= 1.0 {
        commands.push(WorldCommand::MoverCompleted {
            entity: train,
            request_id,
        });
    }
    world.phase(tick, &commands).unwrap()
}

#[test]
fn configured_tracktrain_speed_acceleration_modifier_and_reversal_follow_source_inputs() {
    let mut world = compile(
        br#"
{"classname" "path_track" "targetname" "first" "target" "second" "origin" "0 0 0"}
{"classname" "path_track" "targetname" "second" "target" "third" "origin" "128 0 0"}
{"classname" "path_track" "targetname" "third" "origin" "256 0 0"}
{"classname" "func_tracktrain" "targetname" "train" "target" "first" "model" "*1"
 "origin" "-10 0 2" "height" "1" "wheels" "20" "speed" "0" "startspeed" "90"
 "velocitytype" "1" "orientationtype" "1" "ManualSpeedChanges" "1"
 "ManualAccelSpeed" "70" "ManualDecelSpeed" "150" "spawnflags" "514"}
"#,
    );
    let train = handle(&world, b"train");
    world.phase(0, &[]).unwrap();
    assert_eq!(
        world.entity(train).unwrap().world_transform.origin,
        [0.0, 0.0, 1.0]
    );
    assert_eq!(
        world.entity(train).unwrap().world_transform.angles,
        [0.0, 0.0, 0.0]
    );

    let started = world
        .phase(
            0,
            &[input(train, b"SetSpeedDirAccel", Variant::float(0.55))],
        )
        .unwrap();
    let mut request = mover_request(&started, train);
    let path = train_path(&world, train);
    assert!((f32::from_bits(path.desired_speed_bits) - 49.5).abs() < 0.000_01);
    assert!((f32::from_bits(path.current_speed_bits) - 1.15).abs() < 0.000_01);

    for tick in 1..=3 {
        let batch = apply_tick(&mut world, tick, train, request);
        request = mover_request(&batch, train);
        let expected = 1.15 + 1.05 * tick as f32;
        assert!(
            (f32::from_bits(train_path(&world, train).current_speed_bits) - expected).abs()
                < 0.000_02
        );
    }

    let modified = world
        .phase(
            3,
            &[input(
                train,
                b"SetSpeedForwardModifier",
                Variant::float(-0.5),
            )],
        )
        .unwrap();
    let path = train_path(&world, train);
    assert_eq!(f32::from_bits(path.forward_modifier_bits), 0.5);
    assert!((f32::from_bits(path.desired_speed_bits) - 24.75).abs() < 0.000_01);
    assert!(matches!(
        mover_request(&modified, train),
        RuntimeRequest::Mover { .. }
    ));

    let previous_node = train_path(&world, train).current;
    world
        .phase(
            3,
            &[input(train, b"SetSpeedDirAccel", Variant::float(-0.1))],
        )
        .unwrap();
    let path = train_path(&world, train);
    assert!(!path.forward);
    assert_eq!(f32::from_bits(path.desired_speed_bits), -9.0);
    assert_ne!(path.current, previous_node);
    assert!(f32::from_bits(path.current_speed_bits) < 5.35);
}

#[test]
fn path_track_alternates_have_authored_backlinks_and_disabled_routes_stop_without_guessing() {
    let mut world = compile(
        br#"
{"classname" "path_track" "targetname" "start" "target" "straight" "altpath" "alternate" "origin" "0 0 0"}
{"classname" "path_track" "targetname" "straight" "origin" "100 0 0"}
{"classname" "path_track" "targetname" "alternate" "origin" "0 100 0"}
{"classname" "func_tracktrain" "targetname" "train" "target" "start" "model" "*1" "wheels" "20" "startspeed" "90"}
"#,
    );
    let train = handle(&world, b"train");
    let start = handle(&world, b"start");
    let alternate = handle(&world, b"alternate");
    world.phase(0, &[]).unwrap();
    assert!(matches!(
        &world.entity(alternate).unwrap().behavior,
        BehaviorState::PathNode(node) if node.previous == Some(start) && node.orientation == 1
    ));
    world
        .phase(0, &[input(start, b"EnableAlternatePath", Variant::Void)])
        .unwrap();
    let redirected = world
        .phase(0, &[input(train, b"StartForward", Variant::Void)])
        .unwrap();
    assert!(matches!(
        mover_request(&redirected, train),
        RuntimeRequest::Mover { world_destination: [0.0, y, 0.0], .. } if y > 0.0
    ));

    world
        .phase(0, &[input(alternate, b"DisablePath", Variant::Void)])
        .unwrap();
    world
        .phase(0, &[input(train, b"SetSpeedReal", Variant::float(45.0))])
        .unwrap();
    assert_eq!(
        f32::from_bits(train_path(&world, train).current_speed_bits),
        0.0
    );
    assert!(!train_path(&world, train).running);
}

#[test]
fn tracktrain_initial_wheels_node_pass_and_every_think_output_preserve_source_order() {
    let mut world = compile(
        br#"
{"classname" "math_counter" "targetname" "passes" "startvalue" "0" "min" "0" "max" "100"}
{"classname" "math_counter" "targetname" "thinks" "startvalue" "0" "min" "0" "max" "100"}
{"classname" "path_track" "targetname" "first" "target" "corner" "origin" "0 0 0"
 "OnPass" "passes,Add,1,0,-1"}
{"classname" "path_track" "targetname" "corner" "target" "end" "origin" "5 0 0"
 "OnPass" "passes,Add,1,0,-1"}
{"classname" "path_track" "targetname" "end" "origin" "5 100 0"}
{"classname" "func_tracktrain" "targetname" "train" "target" "first" "model" "*1"
 "wheels" "10" "startspeed" "90" "OnNextPoint" "thinks,Add,1,0,-1"}
"#,
    );
    let train = handle(&world, b"train");
    let passes = handle(&world, b"passes");
    let thinks = handle(&world, b"thinks");
    world.phase(0, &[]).unwrap();
    assert!((world.entity(train).unwrap().world_transform.angles[1] - 45.0).abs() < 0.000_01);
    assert!(
        matches!(&world.entity(passes).unwrap().behavior, BehaviorState::Counter(counter)
        if f32::from_bits(counter.value_bits) == 1.0)
    );

    let started = world
        .phase(0, &[input(train, b"StartForward", Variant::Void)])
        .unwrap();
    let request = mover_request(&started, train);
    assert_eq!(
        train_path(&world, train).current,
        Some(handle(&world, b"corner"))
    );
    assert!(
        matches!(&world.entity(passes).unwrap().behavior, BehaviorState::Counter(counter)
        if f32::from_bits(counter.value_bits) == 2.0)
    );
    assert!(
        matches!(&world.entity(thinks).unwrap().behavior, BehaviorState::Counter(counter)
        if f32::from_bits(counter.value_bits) == 1.0)
    );

    apply_tick(&mut world, 1, train, request);
    assert!(
        matches!(&world.entity(thinks).unwrap().behavior, BehaviorState::Counter(counter)
        if f32::from_bits(counter.value_bits) == 2.0)
    );
}

#[test]
fn tracktrain_fixed_authored_path_angle_linear_and_eased_orientation_are_distinct() {
    for (orientation, manual, expected_yaw) in [
        (0, true, 0.0_f32),
        (1, true, 90.0),
        (2, false, 22.5),
        (3, false, 14.0625),
    ] {
        let source = format!(
            r#"
{{"classname" "path_track" "targetname" "first" "target" "second" "origin" "0 0 0"}}
{{"classname" "path_track" "targetname" "second" "target" "third" "origin" "100 0 0" "orientationtype" "2" "angles" "0 90 0"}}
{{"classname" "path_track" "targetname" "third" "origin" "100 100 0"}}
{{"classname" "func_tracktrain" "targetname" "train" "target" "first" "model" "*1"
 "wheels" "20" "startspeed" "90" "orientationtype" "{orientation}" "ManualSpeedChanges" "{}"}}
"#,
            u8::from(manual)
        );
        let mut world = compile(source.as_bytes());
        let train = handle(&world, b"train");
        world.phase(0, &[]).unwrap();
        world
            .phase(
                0,
                &[WorldCommand::SetWorldTransform {
                    entity: train,
                    transform: Transform {
                        origin: [25.0, 0.0, 0.0],
                        angles: [0.0; 3],
                    },
                }],
            )
            .unwrap();
        let started = world
            .phase(0, &[input(train, b"StartForward", Variant::Void)])
            .unwrap();
        let RuntimeRequest::Mover {
            world_angles_destination,
            angular_velocity,
            ..
        } = mover_request(&started, train)
        else {
            unreachable!()
        };
        assert!(
            (world_angles_destination[1] - expected_yaw).abs() < 0.001,
            "orientation {orientation}: {} != {expected_yaw}",
            world_angles_destination[1]
        );
        assert!((angular_velocity[1] - expected_yaw / TICK_INTERVAL).abs() < 0.01);
    }
}

#[test]
fn tracktrain_teleport_uses_exact_node_origin_and_authored_teleport_output() {
    let mut world = compile(
        br#"
{"classname" "math_counter" "targetname" "teleports" "startvalue" "0" "min" "0" "max" "100"}
{"classname" "path_track" "targetname" "first" "target" "second" "origin" "0 0 10"}
{"classname" "path_track" "targetname" "second" "origin" "50 0 20"
 "OnTeleport" "teleports,Add,1,0,-1"}
{"classname" "func_tracktrain" "targetname" "train" "target" "first" "model" "*1"
 "height" "6" "wheels" "20" "startspeed" "90"}
"#,
    );
    let train = handle(&world, b"train");
    let counter = handle(&world, b"teleports");
    world.phase(0, &[]).unwrap();
    assert_eq!(
        world.entity(train).unwrap().world_transform.origin,
        [0.0, 0.0, 16.0]
    );
    world
        .phase(
            0,
            &[input(
                train,
                b"TeleportToPathTrack",
                Variant::String(b"second".to_vec()),
            )],
        )
        .unwrap();
    assert_eq!(
        world.entity(train).unwrap().world_transform.origin,
        [50.0, 0.0, 20.0]
    );
    assert!(
        matches!(&world.entity(counter).unwrap().behavior, BehaviorState::Counter(state)
        if f32::from_bits(state.value_bits) == 1.0)
    );
}
