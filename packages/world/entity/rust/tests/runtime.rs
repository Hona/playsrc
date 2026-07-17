use playsrc_entity::{
    BehaviorState, BrushSolidity, ContactKind, ContactRecord, EntityHandle, EntityWorld,
    EntityWorldConfig, EventTarget, InputRecord, Lifecycle, ModelBounds, MoverPosition,
    ParentRequest, RuntimeFailureCode, RuntimeLimits, RuntimeRequest, Transform, Transition,
    TriggerKind, Variant, WorldCommand, parse,
};

fn compile(text: &[u8], configure: impl FnOnce(&mut EntityWorldConfig)) -> EntityWorld {
    let graph = parse(text, playsrc_entity::Limits::default()).unwrap();
    let mut config = EntityWorldConfig {
        tick_interval: 0.01,
        source_identity: 0xbeef,
        registry_identity: 19,
        ..EntityWorldConfig::default()
    };
    configure(&mut config);
    EntityWorld::compile(&graph, config).unwrap().0
}

fn input(target: EntityHandle, name: &[u8], value: Variant, sequence: u64) -> WorldCommand {
    WorldCommand::Input(InputRecord {
        target: EventTarget::Direct(target),
        input: name.to_vec(),
        value,
        activator: None,
        caller: None,
        output_action: None,
        producer_sequence: sequence,
    })
}

fn request(batch: &playsrc_entity::TransitionBatch) -> RuntimeRequest {
    batch
        .records
        .iter()
        .find_map(|record| match &record.transition {
            Transition::Request(request) => Some(request.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn live_handles_duplicate_resolution_reverse_outputs_and_same_tick_queue_are_deterministic() {
    let bytes = b"\
{\"classname\"\"worldspawn\"}\
{\"classname\"\"func_brush\"\"targetname\"\"sink\"\"StartDisabled\"\"1\"}\
{\"classname\"\"func_brush\"\"targetname\"\"sink\"}\
{\"classname\"\"logic_relay\"\"targetname\"\"relay\"\
\"OnTrigger\"\"sink,Enable,,0,1\"\
\"OnTrigger\"\"sink,Disable,,0,1\"}";
    let mut world = compile(bytes, |_| {});
    let sinks = world.resolve(b"sink", None, None, None);
    let relay = world.resolve(b"relay", None, None, None)[0];
    assert_eq!(sinks.len(), 2);

    let batch = world
        .phase(0, &[input(relay, b"Trigger", Variant::Void, 7)])
        .unwrap();
    let actions: Vec<_> = batch
        .records
        .iter()
        .filter_map(|record| match record.transition {
            Transition::Output { action_id, .. } => Some(action_id),
            _ => None,
        })
        .collect();
    assert_eq!(actions, vec![2, 1]);
    for sink in &sinks {
        assert!(matches!(
            world.entity(*sink).unwrap().behavior,
            BehaviorState::Brush(ref brush) if brush.enabled
        ));
    }
    assert!(world.entity(relay).unwrap().outputs.is_empty());

    world.phase(1, &[WorldCommand::Remove(sinks[0])]).unwrap();
    assert!(world.entity(sinks[0]).is_none());
    assert_eq!(world.resolve(b"sink", None, None, None), vec![sinks[1]]);
    let definition = parse(
        b"{\"classname\"\"func_brush\"\"targetname\"\"sink\"}",
        playsrc_entity::Limits::default(),
    )
    .unwrap()
    .entities
    .remove(0);
    world.phase(2, &[WorldCommand::Spawn(definition)]).unwrap();
    let replacements = world.resolve(b"sink", None, None, None);
    assert_eq!(replacements.len(), 2);
    assert_eq!(replacements[1].slot, sinks[0].slot);
    assert_ne!(replacements[1].generation, sinks[0].generation);
}

#[test]
fn parenting_preserves_world_and_propagates_mover_endpoints() {
    let bytes = b"\
{\"classname\"\"func_brush\"\"targetname\"\"carrier\"\"origin\"\"10 0 0\"}\
{\"classname\"\"func_movelinear\"\"targetname\"\"platform\"\"parentname\"\"carrier\"\
\"origin\"\"15 0 0\"\"model\"\"*1\"\"movedir\"\"0 0 0\"\"speed\"\"50\"}";
    let mut world = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0, 0.0, 0.0],
            maxs: [10.0, 2.0, 2.0],
        });
    });
    let carrier = world.resolve(b"carrier", None, None, None)[0];
    let platform = world.resolve(b"platform", None, None, None)[0];
    assert_eq!(
        world.entity(platform).unwrap().local_transform.origin,
        [5.0, 0.0, 0.0]
    );
    assert_eq!(
        world.entity(platform).unwrap().world_transform.origin,
        [15.0, 0.0, 0.0]
    );

    world
        .phase(
            1,
            &[WorldCommand::SetWorldTransform {
                entity: carrier,
                transform: Transform {
                    origin: [20.0, 0.0, 0.0],
                    angles: [0.0; 3],
                },
            }],
        )
        .unwrap();
    assert_eq!(
        world.entity(platform).unwrap().world_transform.origin,
        [25.0, 0.0, 0.0]
    );

    let batch = world
        .phase(2, &[input(platform, b"Open", Variant::Void, 8)])
        .unwrap();
    assert!(matches!(
        request(&batch),
        RuntimeRequest::Mover {
            entity,
            local_destination: [13.0, 0.0, 0.0],
            world_destination: [33.0, 0.0, 0.0],
            speed: 50.0,
            opening: true,
            ..
        } if entity == platform
    ));
}

#[test]
fn mover_completion_wait_and_blocking_are_owned_without_advancing_trajectory() {
    let bytes = b"\
{\"classname\"\"func_door\"\"targetname\"\"door\"\"model\"\"*1\"\"movedir\"\"0 0 0\"\
\"wait\"\"0.03\"\"dmg\"\"5\"\"OnFullyOpen\"\"missing,Use,,0,-1\"}";
    let mut world = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0, 0.0, 0.0],
            maxs: [12.0, 2.0, 2.0],
        });
    });
    let door = world.resolve(b"door", None, None, None)[0];
    let opened = world
        .phase(0, &[input(door, b"Open", Variant::Void, 1)])
        .unwrap();
    let RuntimeRequest::Mover { request_id, .. } = request(&opened) else {
        panic!("missing mover request")
    };
    assert!(matches!(
        world.entity(door).unwrap().behavior,
        BehaviorState::Mover(ref mover) if mover.position == MoverPosition::Opening
    ));

    world
        .phase(
            1,
            &[WorldCommand::MoverCompleted {
                entity: door,
                request_id,
            }],
        )
        .unwrap();
    assert_eq!(
        world.entity(door).unwrap().world_transform.origin,
        [10.0, 0.0, 0.0]
    );
    let closing = world.phase(4, &[]).unwrap();
    assert!(matches!(
        request(&closing),
        RuntimeRequest::Mover {
            entity,
            opening: false,
            ..
        } if entity == door
    ));
}

#[test]
fn trigger_contacts_filter_unique_entries_disable_and_exit_in_order() {
    let bytes = b"\
{\"classname\"\"player_subject\"\"targetname\"\"hero\"}\
{\"classname\"\"filter_activator_name\"\"targetname\"\"hero_filter\"\"filtername\"\"hero\"}\
{\"classname\"\"trigger_multiple\"\"targetname\"\"zone\"\"filtername\"\"hero_filter\"\
\"OnStartTouch\"\"missing,Use,,10,-1\"\"OnEndTouch\"\"missing,Use,,10,-1\"}";
    let mut world = compile(bytes, |_| {});
    let subject = world.resolve(b"hero", None, None, None)[0];
    let trigger = world.resolve(b"zone", None, None, None)[0];
    let entered = world
        .phase(
            0,
            &[WorldCommand::Contact(ContactRecord {
                trigger,
                subject,
                kind: ContactKind::Enter,
                external_filter_result: None,
                producer_sequence: 11,
            })],
        )
        .unwrap();
    assert!(matches!(
        request(&entered),
        RuntimeRequest::TriggerEffect {
            trigger: found_trigger,
            subject: found_subject,
            kind: TriggerKind::Multiple,
            contact: ContactKind::Enter,
        } if found_trigger == trigger && found_subject == subject
    ));
    assert!(matches!(
        world.entity(trigger).unwrap().behavior,
        BehaviorState::Trigger(ref state) if state.contacts == vec![subject]
    ));

    world
        .phase(1, &[input(trigger, b"Disable", Variant::Void, 12)])
        .unwrap();
    world
        .phase(
            2,
            &[WorldCommand::Contact(ContactRecord {
                trigger,
                subject,
                kind: ContactKind::Exit,
                external_filter_result: None,
                producer_sequence: 13,
            })],
        )
        .unwrap();
    assert!(matches!(
        world.entity(trigger).unwrap().behavior,
        BehaviorState::Trigger(ref state) if !state.enabled && state.contacts.is_empty()
    ));
}

#[test]
fn counter_case_and_canonical_snapshot_restore_continue_identically() {
    let bytes = b"\
{\"classname\"\"func_brush\"\"targetname\"\"sink\"}\
{\"classname\"\"math_counter\"\"targetname\"\"count\"\"startvalue\"\"1\"\"min\"\"0\"\"max\"\"2\"\
\"OnHitMax\"\"sink,Disable,,0,-1\"\"OutValue\"\"sink,Enable,,1,-1\"}\
{\"classname\"\"logic_case\"\"targetname\"\"case\"\"Case01\"\"alpha\"\"OnCase01\"\"sink,Toggle,,0,-1\"}";
    let mut world = compile(bytes, |_| {});
    let counter = world.resolve(b"count", None, None, None)[0];
    let case = world.resolve(b"case", None, None, None)[0];
    world
        .phase(0, &[input(counter, b"Add", Variant::float(4.0), 1)])
        .unwrap();
    assert!(matches!(
        world.entity(counter).unwrap().behavior,
        BehaviorState::Counter(ref state)
            if f32::from_bits(state.value_bits) == 2.0 && state.hit_max
    ));
    world
        .phase(
            1,
            &[input(
                case,
                b"InValue",
                Variant::String(b"ALPHA".to_vec()),
                2,
            )],
        )
        .unwrap();

    let snapshot = world.snapshot().unwrap();
    assert_eq!(&snapshot.bytes()[..8], b"PSEN\x01\0\0\0");
    assert_eq!(snapshot.bytes(), world.snapshot().unwrap().bytes());
    let mut restored = compile(bytes, |_| {});
    restored.restore(&snapshot).unwrap();
    let next_a = world.phase(100, &[]).unwrap();
    let next_b = restored.phase(100, &[]).unwrap();
    assert_eq!(next_a, next_b);
    assert_eq!(
        world.snapshot().unwrap().bytes(),
        restored.snapshot().unwrap().bytes()
    );
}

#[test]
fn queue_bound_fails_before_consuming_finite_output_actions() {
    let bytes = b"\
{\"classname\"\"func_brush\"\"targetname\"\"sink\"}\
{\"classname\"\"logic_relay\"\"targetname\"\"relay\"\
\"OnTrigger\"\"sink,Enable,,1,1\"\"OnTrigger\"\"sink,Disable,,1,1\"}";
    let mut world = compile(bytes, |config| {
        config.limits = RuntimeLimits {
            max_queued_events: 1,
            ..RuntimeLimits::default()
        };
    });
    let relay = world.resolve(b"relay", None, None, None)[0];
    let error = world
        .phase(0, &[input(relay, b"Trigger", Variant::Void, 1)])
        .unwrap_err();
    assert_eq!(error.code, RuntimeFailureCode::QueueLimit);
    assert_eq!(world.entity(relay).unwrap().outputs.len(), 2);
    assert!(matches!(
        world.entity(relay).unwrap().lifecycle,
        Lifecycle::Activated
    ));
}

#[test]
fn parent_cycle_and_missing_attachment_leave_prior_state_unchanged() {
    let bytes = b"\
{\"classname\"\"func_brush\"\"targetname\"\"a\"}\
{\"classname\"\"func_brush\"\"targetname\"\"b\"}";
    let mut world = compile(bytes, |_| {});
    let a = world.resolve(b"a", None, None, None)[0];
    let b = world.resolve(b"b", None, None, None)[0];
    world
        .phase(
            0,
            &[WorldCommand::SetParent(ParentRequest {
                child: b,
                parent: Some(a),
                attachment: None,
                mode: playsrc_entity::ParentMode::MaintainWorld,
            })],
        )
        .unwrap();
    world
        .phase(
            1,
            &[WorldCommand::SetParent(ParentRequest {
                child: a,
                parent: Some(b),
                attachment: None,
                mode: playsrc_entity::ParentMode::MaintainWorld,
            })],
        )
        .unwrap();
    assert_eq!(world.entity(a).unwrap().parent, None);
    assert_eq!(world.entity(b).unwrap().parent, Some(a));
    assert!(matches!(
        world.entity(a).unwrap().behavior,
        BehaviorState::Brush(ref state) if state.solidity == BrushSolidity::Toggle
    ));
}

#[test]
fn generic_class_initial_states_follow_declared_keys_and_spawnflags() {
    let bytes = b"\
{\"classname\"\"func_button\"\"targetname\"\"button\"\"model\"\"*1\"\"spawnflags\"\"2080\"\"wait\"\"-1\"}\
{\"classname\"\"func_door\"\"targetname\"\"door\"\"model\"\"*1\"\"spawnflags\"\"2080\"\"spawnpos\"\"1\"}\
{\"classname\"\"func_brush\"\"targetname\"\"brush\"\"StartDisabled\"\"1\"\"Solidity\"\"2\"}\
{\"classname\"\"logic_relay\"\"targetname\"\"relay\"\"StartDisabled\"\"1\"}\
{\"classname\"\"trigger_teleport\"\"targetname\"\"tele\"\"StartDisabled\"\"1\"}" ;
    let world = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [10.0, 2.0, 2.0],
        });
    });
    let button = world.resolve(b"button", None, None, None)[0];
    let door = world.resolve(b"door", None, None, None)[0];
    let brush = world.resolve(b"brush", None, None, None)[0];
    let relay = world.resolve(b"relay", None, None, None)[0];
    let tele = world.resolve(b"tele", None, None, None)[0];
    assert!(matches!(
        world.entity(button).unwrap().behavior,
        BehaviorState::Mover(ref state)
            if state.locked && state.toggle && state.speed == 40.0
    ));
    assert!(matches!(
        world.entity(door).unwrap().behavior,
        BehaviorState::Mover(ref state)
            if state.locked && state.no_auto_return && state.outputs_reversed
                && state.position == MoverPosition::Open
    ));
    assert!(matches!(
        world.entity(brush).unwrap().behavior,
        BehaviorState::Brush(ref state)
            if !state.enabled && state.solidity == BrushSolidity::Always
    ));
    assert!(matches!(
        world.entity(relay).unwrap().behavior,
        BehaviorState::Relay(ref state) if !state.enabled
    ));
    assert!(matches!(
        world.entity(tele).unwrap().behavior,
        BehaviorState::Trigger(ref state)
            if !state.enabled && state.kind == TriggerKind::Teleport
    ));
}

#[test]
fn caller_and_direct_prefix_cancellation_remove_only_matching_pending_events() {
    let bytes = b"\
{\"classname\"\"func_brush\"\"targetname\"\"sink\"}\
{\"classname\"\"logic_relay\"\"targetname\"\"relay\"\"OnTrigger\"\"sink,Disable,,1,-1\"}";
    let mut world = compile(bytes, |_| {});
    let relay = world.resolve(b"relay", None, None, None)[0];
    let first = world
        .phase(0, &[input(relay, b"Trigger", Variant::Void, 1)])
        .unwrap();
    assert!(world.has_pending(relay, b"Enable"));
    assert!(
        first
            .records
            .iter()
            .any(|record| matches!(record.transition, Transition::Scheduled { .. }))
    );
    let cancelled = world
        .phase(
            1,
            &[WorldCommand::CancelDirectInput {
                target: relay,
                input_prefix: b"Enable".to_vec(),
            }],
        )
        .unwrap();
    assert!(!world.has_pending(relay, b"Enable"));
    assert!(
        cancelled
            .records
            .iter()
            .any(|record| matches!(record.transition, Transition::Cancelled { .. }))
    );

    world
        .phase(
            2,
            &[
                input(relay, b"EnableRefire", Variant::Void, 2),
                input(relay, b"Trigger", Variant::Void, 3),
                WorldCommand::CancelCaller(relay),
            ],
        )
        .unwrap();
    assert!(!world.has_pending(relay, b"Enable"));
}

#[test]
fn output_inheritance_uses_call_delay_while_override_uses_only_action_delay() {
    let bytes = b"\
{\"classname\"\"func_brush\"\"targetname\"\"sink\"}\
{\"classname\"\"logic_relay\"\"targetname\"\"relay\"\
\"OnTrigger\"\"sink,Enable,,0.01,1\"\"OnTrigger\"\"sink,Disable,override,0.01,1\"}";
    let mut world = compile(bytes, |_| {});
    let relay = world.resolve(b"relay", None, None, None)[0];
    let batch = world
        .phase(
            0,
            &[WorldCommand::EmitOutput {
                entity: relay,
                output: b"OnTrigger".to_vec(),
                value: Variant::float(4.0),
                activator: None,
                caller: Some(relay),
                delay: 0.02,
            }],
        )
        .unwrap();
    let due: Vec<_> = batch
        .records
        .iter()
        .filter_map(|record| match record.transition {
            Transition::Scheduled { due_tick, .. } => Some(due_tick),
            _ => None,
        })
        .collect();
    assert_eq!(due, vec![1, 3]);
}

#[test]
fn cyclic_filter_graph_fails_closed_without_recording_contact() {
    let bytes = b"\
{\"classname\"\"player_subject\"\"targetname\"\"subject\"}\
{\"classname\"\"filter_multi\"\"targetname\"\"f1\"\"Filter01\"\"f2\"}\
{\"classname\"\"filter_multi\"\"targetname\"\"f2\"\"Filter01\"\"f1\"}\
{\"classname\"\"trigger_multiple\"\"targetname\"\"zone\"\"filtername\"\"f1\"}";
    let mut world = compile(bytes, |_| {});
    let subject = world.resolve(b"subject", None, None, None)[0];
    let trigger = world.resolve(b"zone", None, None, None)[0];
    world
        .phase(
            0,
            &[WorldCommand::Contact(ContactRecord {
                trigger,
                subject,
                kind: ContactKind::Enter,
                external_filter_result: None,
                producer_sequence: 1,
            })],
        )
        .unwrap();
    assert!(matches!(
        world.entity(trigger).unwrap().behavior,
        BehaviorState::Trigger(ref state) if state.contacts.is_empty()
    ));
}
