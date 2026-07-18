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
    assert_eq!(&snapshot.bytes()[..8], b"PSEN\x03\0\0\0");
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
            if state.locked && state.no_auto_return && !state.outputs_reversed
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

#[test]
fn button_damage_fires_outputs_before_request_and_honors_dont_move() {
    let bytes = b"\
{\"classname\"\"player\"\"targetname\"\"attacker\"}\
{\"classname\"\"func_button\"\"targetname\"\"button\"\"model\"\"*1\"\"spawnflags\"\"513\"\
\"OnDamaged\"\"missing,Use,,0,-1\"\"OnPressed\"\"missing,Use,,0,-1\"}";
    let mut world = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [32.0, 8.0, 8.0],
        });
    });
    let attacker = world.resolve(b"attacker", None, None, None)[0];
    let button = world.resolve(b"button", None, None, None)[0];
    let batch = world
        .phase(
            0,
            &[WorldCommand::Damage {
                entity: button,
                attacker: Some(attacker),
            }],
        )
        .unwrap();
    let ordered: Vec<_> = batch
        .records
        .iter()
        .filter_map(|record| match &record.transition {
            Transition::Output { output, .. } => Some(output.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(ordered, [b"OnDamaged".as_slice(), b"OnPressed"]);
    assert!(batch.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            local_destination: [0.0, 0.0, 0.0],
            ..
        }) if entity == button
    )));
    assert!(matches!(
        world.entity(button).unwrap().behavior,
        BehaviorState::Mover(ref state)
            if state.damage_activates && state.dont_move && state.activator == Some(attacker)
    ));
}

#[test]
fn external_class_binding_accepts_only_declared_inputs() {
    let mut world = compile(
        b"{\"classname\"\"game_volume\"\"targetname\"\"volume\"}",
        |config| {
            config
                .external_classes
                .push(playsrc_entity::ExternalClassBinding {
                    classname: b"game_volume".to_vec(),
                    inputs: vec![b"Enable".to_vec()],
                });
        },
    );
    let volume = world.resolve(b"volume", None, None, None)[0];
    let accepted = world
        .phase(0, &[input(volume, b"Enable", Variant::Void, 1)])
        .unwrap();
    assert!(accepted.records.iter().any(|record| matches!(
        &record.transition,
        Transition::Request(RuntimeRequest::ExternalInput { entity, input, .. })
            if *entity == volume && input.eq_ignore_ascii_case(b"Enable")
    )));
    let rejected = world
        .phase(1, &[input(volume, b"Disable", Variant::Void, 2)])
        .unwrap();
    assert!(rejected.records.iter().any(|record| matches!(
        record.transition,
        Transition::Input { target, accepted: false, .. } if target == volume
    )));
}

#[test]
fn external_brush_visibility_requires_an_explicit_selected_game_binding() {
    let bytes = br#"{"classname" "game_volume" "targetname" "volume" "model" "*1"}"#;
    let configure = |config: &mut EntityWorldConfig, hidden| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [8.0; 3],
        });
        config
            .external_classes
            .push(playsrc_entity::ExternalClassBinding {
                classname: b"game_volume".to_vec(),
                inputs: vec![b"Enable".to_vec()],
            });
        config
            .external_brush_models
            .push(playsrc_entity::ExternalBrushModelBinding {
                classname: b"game_volume".to_vec(),
                initial_visibility: if hidden {
                    playsrc_entity::ExternalBrushModelVisibility::Hidden
                } else {
                    playsrc_entity::ExternalBrushModelVisibility::BaseEntity
                },
            });
    };
    let visible = compile(bytes, |config| configure(config, false));
    assert!(
        visible
            .brush_model_presentation(visible.revision())
            .unwrap()
            .models[0]
            .draw
    );
    let hidden = compile(bytes, |config| configure(config, true));
    assert!(
        !hidden
            .brush_model_presentation(hidden.revision())
            .unwrap()
            .models[0]
            .draw
    );
}

#[test]
fn brush_model_presentation_joins_render_parent_solidity_model_and_revision_state() {
    let bytes = br#"
{"classname" "func_brush" "targetname" "carrier" "model" "*1" "origin" "10 0 0" "Solidity" "1"}
{"classname" "func_movelinear" "targetname" "child" "model" "*2" "parentname" "carrier"
 "origin" "15 0 0" "angles" "0 90 0" "MoveDistance" "10" "StartPosition" "0"
 "rendermode" "2" "rendercolor" "10 20 30 99" "renderamt" "128" "renderfx" "3" "effects" "64"}
{"classname" "func_brush" "targetname" "hidden_solid" "model" "*3" "StartDisabled" "1" "Solidity" "2"}
{"classname" "trigger_multiple" "targetname" "trigger" "model" "*4"}
{"classname" "func_breakable" "targetname" "breakable" "model" "*5"}
"#;
    let mut world = compile(bytes, |config| {
        for model in 1..=5 {
            config.model_bounds.push(ModelBounds {
                model,
                mins: [0.0; 3],
                maxs: [12.0, 4.0, 4.0],
            });
        }
    });
    assert_eq!(world.revision(), 1);
    let initial = world.brush_model_presentation(1).unwrap();
    assert_eq!(initial.revision, 1);
    assert_eq!(initial.models.len(), 5);

    let carrier = world.resolve(b"carrier", None, None, None)[0];
    let child = world.resolve(b"child", None, None, None)[0];
    let hidden = world.resolve(b"hidden_solid", None, None, None)[0];
    let trigger = world.resolve(b"trigger", None, None, None)[0];
    let breakable = world.resolve(b"breakable", None, None, None)[0];
    let carrier_draw = initial
        .models
        .iter()
        .find(|state| state.handle == carrier)
        .unwrap();
    assert!(carrier_draw.draw);
    assert!(matches!(
        world.entity(carrier).unwrap().behavior,
        BehaviorState::Brush(ref state)
            if state.enabled && state.solidity == BrushSolidity::Never
    ));
    let child_draw = initial
        .models
        .iter()
        .find(|state| state.handle == child)
        .unwrap();
    assert_eq!(child_draw.parent, Some(carrier));
    assert_eq!(child_draw.local_transform.origin, [5.0, 0.0, 0.0]);
    assert_eq!(child_draw.world_transform.origin, [15.0, 0.0, 0.0]);
    assert_eq!(child_draw.render_mode, 2);
    assert_eq!(child_draw.color, [10, 20, 30, 128]);
    assert_eq!(child_draw.render_fx, 3);
    assert_eq!(child_draw.effects, 64);
    assert!(
        !initial
            .models
            .iter()
            .find(|state| state.handle == hidden)
            .unwrap()
            .draw
    );
    assert!(matches!(
        world.entity(hidden).unwrap().behavior,
        BehaviorState::Brush(ref state) if state.solidity == BrushSolidity::Always
    ));
    assert!(
        !initial
            .models
            .iter()
            .find(|state| state.handle == trigger)
            .unwrap()
            .draw
    );
    assert!(
        initial
            .models
            .iter()
            .find(|state| state.handle == breakable)
            .unwrap()
            .draw
    );

    let changed = world
        .phase(
            1,
            &[
                input(child, b"Alpha", Variant::Integer(300), 1),
                input(child, b"Color", Variant::Color([1, 2, 3, 4]), 2),
                input(child, b"DisableShadow", Variant::Void, 3),
                input(
                    child,
                    b"AddOutput",
                    Variant::String(b"rendermode 10".to_vec()),
                    4,
                ),
                WorldCommand::SetBrushModel {
                    entity: child,
                    model: Some(3),
                },
            ],
        )
        .unwrap();
    assert!(!changed.records.is_empty());
    assert_eq!(world.revision(), 2);
    assert_eq!(
        world.brush_model_presentation(1).unwrap_err().code,
        RuntimeFailureCode::RevisionMismatch
    );
    let current = world.brush_model_presentation(2).unwrap();
    let child_draw = current
        .models
        .iter()
        .find(|state| state.handle == child)
        .unwrap();
    assert_eq!(child_draw.model, 3);
    assert_eq!(child_draw.color, [1, 2, 3, 255]);
    assert_eq!(child_draw.render_mode, 10);
    assert!(!child_draw.draw);
    assert_eq!(child_draw.effects, 64 | 16);

    for mode in 0_u8..=10 {
        world
            .phase(
                u64::from(mode) + 2,
                &[input(
                    child,
                    b"AddOutput",
                    Variant::String(format!("rendermode {mode}").into_bytes()),
                    u64::from(mode) + 10,
                )],
            )
            .unwrap();
        let presentation = world.brush_model_presentation(world.revision()).unwrap();
        let state = presentation
            .models
            .iter()
            .find(|state| state.handle == child)
            .unwrap();
        assert_eq!(state.render_mode, mode);
        assert_eq!(state.draw, mode != 10);
    }
    let snapshot = world.snapshot().unwrap();
    let mut restored = compile(bytes, |config| {
        for model in 1..=5 {
            config.model_bounds.push(ModelBounds {
                model,
                mins: [0.0; 3],
                maxs: [12.0, 4.0, 4.0],
            });
        }
    });
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.revision(), world.revision());
    assert_eq!(
        restored
            .brush_model_presentation(restored.revision())
            .unwrap(),
        world.brush_model_presentation(world.revision()).unwrap()
    );
}

#[test]
fn mover_presentation_tracks_progress_block_reversal_completion_and_atomic_rollback() {
    let mut world = compile(
        br#"{"classname" "func_door" "targetname" "door" "model" "*1" "movedir" "0 0 0" "wait" "1"}"#,
        |config| {
            config.model_bounds.push(ModelBounds {
                model: 1,
                mins: [0.0; 3],
                maxs: [12.0, 2.0, 2.0],
            });
        },
    );
    let door = world.resolve(b"door", None, None, None)[0];
    let opened = world
        .phase(0, &[input(door, b"Open", Variant::Void, 1)])
        .unwrap();
    let RuntimeRequest::Mover { request_id, .. } = request(&opened) else {
        panic!("missing mover request")
    };
    let progress = world
        .phase(
            1,
            &[WorldCommand::SetWorldTransform {
                entity: door,
                transform: Transform {
                    origin: [5.0, 0.0, 0.0],
                    angles: [0.0; 3],
                },
            }],
        )
        .unwrap();
    assert!(progress.records.iter().any(|record| matches!(
        record.transition,
        Transition::TransformChanged { entity, .. } if entity == door
    )));
    let at_half = world.brush_model_presentation(world.revision()).unwrap();
    let mover = at_half.models[0].mover.as_ref().unwrap();
    assert_eq!(f32::from_bits(mover.progress_bits), 0.5);
    assert_eq!(mover.request_id, Some(request_id));

    let blocked = world
        .phase(
            2,
            &[WorldCommand::MoverBlocked {
                entity: door,
                request_id,
                blocker: door,
                kind: playsrc_entity::BlockContactKind::Stay,
            }],
        )
        .unwrap();
    let reversed = blocked
        .records
        .iter()
        .find_map(|record| match record.transition {
            Transition::Request(RuntimeRequest::Mover {
                request_id,
                opening: false,
                ..
            }) => Some(request_id),
            _ => None,
        })
        .unwrap();
    let reversing = world.brush_model_presentation(world.revision()).unwrap();
    let mover = reversing.models[0].mover.as_ref().unwrap();
    assert_eq!(f32::from_bits(mover.progress_bits), 0.5);
    assert_eq!(mover.request_id, Some(reversed));
    assert_eq!(mover.opening, Some(false));

    world
        .phase(
            3,
            &[
                WorldCommand::SetWorldTransform {
                    entity: door,
                    transform: Transform::IDENTITY,
                },
                WorldCommand::MoverCompleted {
                    entity: door,
                    request_id: reversed,
                },
            ],
        )
        .unwrap();
    let completed = world.brush_model_presentation(world.revision()).unwrap();
    let mover = completed.models[0].mover.as_ref().unwrap();
    assert_eq!(f32::from_bits(mover.progress_bits), 0.0);
    assert_eq!(mover.request_id, None);
    assert_eq!(mover.position, MoverPosition::Closed);

    let revision = world.revision();
    let before = world.brush_model_presentation(revision).unwrap();
    let failure = world
        .phase(
            4,
            &[
                WorldCommand::SetWorldTransform {
                    entity: door,
                    transform: Transform {
                        origin: [7.0, 0.0, 0.0],
                        angles: [0.0; 3],
                    },
                },
                WorldCommand::SetBrushModel {
                    entity: door,
                    model: Some(999),
                },
            ],
        )
        .unwrap_err();
    assert_eq!(failure.code, RuntimeFailureCode::InvalidField);
    assert_eq!(world.revision(), revision);
    assert_eq!(world.brush_model_presentation(revision).unwrap(), before);
}

#[test]
fn breakable_remains_drawn_while_nonsolid_then_removes_at_source_delay() {
    let mut world = compile(
        br#"{"classname" "func_breakable" "targetname" "glass" "model" "*1"}"#,
        |config| {
            config.model_bounds.push(ModelBounds {
                model: 1,
                mins: [0.0; 3],
                maxs: [8.0; 3],
            });
        },
    );
    let glass = world.resolve(b"glass", None, None, None)[0];
    let broken = world
        .phase(0, &[input(glass, b"Break", Variant::Void, 1)])
        .unwrap();
    assert!(broken.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::BrushState {
            entity,
            enabled: true,
            solid: false,
        }) if entity == glass
    )));
    let due_tick = broken
        .records
        .iter()
        .find_map(|record| match record.transition {
            Transition::Scheduled { due_tick, .. } => Some(due_tick),
            _ => None,
        })
        .unwrap();
    assert!(
        world
            .brush_model_presentation(world.revision())
            .unwrap()
            .models[0]
            .draw
    );
    world.phase(due_tick - 1, &[]).unwrap();
    assert_eq!(
        world
            .brush_model_presentation(world.revision())
            .unwrap()
            .models
            .len(),
        1
    );
    world.phase(due_tick, &[]).unwrap();
    assert!(
        world
            .brush_model_presentation(world.revision())
            .unwrap()
            .models
            .is_empty()
    );

    let mut unbreakable = compile(
        br#"{"classname" "func_breakable" "targetname" "glass" "model" "*1" "material" "7"}"#,
        |config| {
            config.model_bounds.push(ModelBounds {
                model: 1,
                mins: [0.0; 3],
                maxs: [8.0; 3],
            });
        },
    );
    let glass = unbreakable.resolve(b"glass", None, None, None)[0];
    let ignored = unbreakable
        .phase(0, &[input(glass, b"Break", Variant::Void, 1)])
        .unwrap();
    assert!(!ignored.records.iter().any(|record| matches!(
        record.transition,
        Transition::Scheduled { .. } | Transition::Request(RuntimeRequest::BrushState { .. })
    )));
    unbreakable.phase(100, &[]).unwrap();
    assert_eq!(
        unbreakable
            .brush_model_presentation(unbreakable.revision())
            .unwrap()
            .models
            .len(),
        1
    );
}
