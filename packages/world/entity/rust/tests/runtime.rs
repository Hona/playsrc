use playsrc_entity::{
    BehaviorState, BrushSolidity, ClassFieldBinding, ContactKind, ContactRecord, EntityHandle,
    EntityWorld, EntityWorldConfig, EventTarget, FieldBinding, FieldType, InitialAttachmentBinding,
    InputRecord, Lifecycle, ModelBounds, MoverClass, MoverPosition, ParentRequest, PushMode,
    RuntimeFailureCode, RuntimeLimits, RuntimeRequest, Transform, Transition, TriggerEffectData,
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
fn cloned_entity_world_mutates_only_its_transactional_entities() {
    let original = compile(
        b"{\"classname\"\"worldspawn\"}{\"classname\"\"func_brush\"\"targetname\"\"shared\"\"StartDisabled\"\"1\"}",
        |_| {},
    );
    let handle = original.resolve(b"shared", None, None, None)[0];
    let mut changed = original.clone();
    changed
        .phase(1, &[input(handle, b"Enable", Variant::Void, 1)])
        .unwrap();

    assert!(matches!(
        original.entity(handle).unwrap().behavior,
        BehaviorState::Brush(ref brush) if !brush.enabled
    ));
    assert!(matches!(
        changed.entity(handle).unwrap().behavior,
        BehaviorState::Brush(ref brush) if brush.enabled
    ));
    assert_eq!(original.current_tick(), 0);
    assert_eq!(changed.current_tick(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &original.entity(handle).unwrap().definition,
        &changed.entity(handle).unwrap().definition,
    ));
}

#[test]
fn installed_definition_survives_checkpoint_rollback_but_not_last_owner_removal() {
    let graph = parse(b"{\"classname\"\"func_brush\"\"targetname\"\"authored\"\"StartDisabled\"\"1\"}", playsrc_entity::Limits::default()).unwrap();
    let mut config = EntityWorldConfig::default();
    config.limits.max_entities = 1;
    let mut world = EntityWorld::compile(&graph, config).unwrap().0;
    let handle = world.resolve(b"authored", None, None, None)[0];
    let definition = std::sync::Arc::downgrade(&world.entity(handle).unwrap().definition);
    let checkpoint = world.snapshot().unwrap();
    let failure = world.phase(1, &[
        input(handle, b"Enable", Variant::Void, 1),
        WorldCommand::SetTargetname { entity: handle, targetname: Some(b"runtime".to_vec()) },
        WorldCommand::Spawn(graph.entities[0].clone()),
    ]).unwrap_err();
    assert_eq!(failure.code, RuntimeFailureCode::EntityLimit);
    assert_eq!(world.snapshot().unwrap().bytes(), checkpoint.bytes());
    world.phase(1, &[
        input(handle, b"Enable", Variant::Void, 1),
        WorldCommand::SetTargetname { entity: handle, targetname: Some(b"runtime".to_vec()) },
    ]).unwrap();
    let entity = world.entity(handle).unwrap();
    assert_eq!(entity.targetname.as_deref(), Some(b"runtime".as_slice()));
    assert_eq!(entity.definition.targetname.as_deref(), Some(b"authored".as_slice()));
    assert!(std::sync::Arc::ptr_eq(&definition.upgrade().unwrap(), &entity.definition));
    // Mutating an exported owned clone must still be private to that clone.
    let mut exported = entity.clone();
    std::sync::Arc::make_mut(&mut exported.definition).targetname = None;
    assert_eq!(world.entity(handle).unwrap().definition.targetname.as_deref(), Some(b"authored".as_slice()));
    drop(exported);
    world.phase(2, &[WorldCommand::Remove(handle)]).unwrap();
    assert!(definition.upgrade().is_some(), "retained checkpoint owns authored data");
    world.restore(&checkpoint).unwrap();
    assert!(std::sync::Arc::ptr_eq(&definition.upgrade().unwrap(), &world.entity(handle).unwrap().definition));
    world.phase(2, &[WorldCommand::Remove(handle)]).unwrap();
    drop(checkpoint);
    assert!(definition.upgrade().is_none(), "no immortal definition cache");
    world.phase(3, &[WorldCommand::Spawn(graph.entities[0].clone())]).unwrap();
    let replacement = world.resolve(b"authored", None, None, None)[0];
    assert_eq!(replacement.slot, handle.slot);
    assert_ne!(replacement.generation, handle.generation);
    assert!(world.entity(handle).is_none());
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
            ..
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
    assert_eq!(&snapshot.bytes()[..8], b"PSEN\x05\0\0\0");
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
fn kill_hierarchy_marks_children_before_parent_and_repeated_kill_is_inert() {
    let mut world = compile(
        br#"
{"classname" "func_brush" "targetname" "parent"}
{"classname" "func_brush" "targetname" "child" "parentname" "parent"}
"#,
        |_| {},
    );
    let parent = world.resolve(b"parent", None, None, None)[0];
    let child = world.resolve(b"child", None, None, None)[0];
    let removed = world
        .phase(0, &[input(parent, b"KillHierarchy", Variant::Void, 1)])
        .unwrap();
    let order = removed
        .records
        .iter()
        .filter_map(|record| match record.transition {
            Transition::Lifecycle {
                entity,
                state: Lifecycle::PendingRemoval,
            } => Some(entity),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(order, [child, parent]);
    assert!(world.entity(parent).is_none() && world.entity(child).is_none());
    assert!(
        world
            .phase(1, &[WorldCommand::Remove(parent)])
            .unwrap()
            .records
            .is_empty()
    );
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
fn button_damage_accepts_legacy_health_and_rejects_a_missing_attacker_after_on_damaged() {
    let bytes = br#"
{"classname""player""targetname""attacker"}
{"classname""func_button""targetname""button""model""*1""health""1"
"OnDamaged""missing,Use,,0,-1""OnPressed""missing,Use,,0,-1"}
"#;
    let mut world = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [32.0, 8.0, 8.0],
        });
    });
    let attacker = world.resolve(b"attacker", None, None, None)[0];
    let button = world.resolve(b"button", None, None, None)[0];
    let missing = world
        .phase(
            0,
            &[WorldCommand::Damage {
                entity: button,
                attacker: None,
            }],
        )
        .unwrap();
    assert!(missing.records.iter().any(|record| matches!(
        &record.transition,
        Transition::Output { output, .. } if output == b"OnDamaged"
    )));
    assert!(!missing.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover { .. })
    )));
    assert!(matches!(
        world.entity(button).unwrap().behavior,
        BehaviorState::Mover(ref state)
            if state.damage_activates && state.position == MoverPosition::Closed
                && state.activator.is_none()
    ));

    let accepted = world
        .phase(
            1,
            &[WorldCommand::Damage {
                entity: button,
                attacker: Some(attacker),
            }],
        )
        .unwrap();
    let outputs = accepted
        .records
        .iter()
        .filter_map(|record| match &record.transition {
            Transition::Output { output, .. } => Some(output.as_slice()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(outputs, [b"OnDamaged".as_slice(), b"OnPressed"]);
    assert!(accepted.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover { entity, .. }) if entity == button
    )));
}

#[test]
fn mover_completion_outputs_retain_class_specific_activator_and_caller_context() {
    let bytes = br#"
{"classname""player""targetname""player"}
{"classname""game_volume""targetname""sink"}
{"classname""func_button""targetname""button""model""*1""OnIn""sink,Enable,,0,-1"}
{"classname""func_door""targetname""door""model""*2""spawnflags""32"
"OnFullyOpen""sink,Enable,,0,-1""OnFullyClosed""sink,Enable,,0,-1"}
{"classname""func_movelinear""targetname""linear""model""*3""MoveDistance""10"
"OnFullyOpen""sink,Enable,,0,-1"}
"#;
    let mut world = compile(bytes, |config| {
        config
            .external_classes
            .push(playsrc_entity::ExternalClassBinding {
                classname: b"game_volume".to_vec(),
                inputs: vec![b"Enable".to_vec()],
            });
        for model in 1..=3 {
            config.model_bounds.push(ModelBounds {
                model,
                mins: [0.0; 3],
                maxs: [12.0, 2.0, 2.0],
            });
        }
    });
    let player = world.resolve(b"player", None, None, None)[0];
    let button = world.resolve(b"button", None, None, None)[0];
    let door = world.resolve(b"door", None, None, None)[0];
    let linear = world.resolve(b"linear", None, None, None)[0];

    for (tick, mover, expected_activator) in
        [(0, button, player), (2, door, door), (4, linear, linear)]
    {
        let started = world
            .phase(
                tick,
                &[WorldCommand::Input(InputRecord {
                    target: EventTarget::Direct(mover),
                    input: if mover == button || mover == door {
                        b"Use".as_slice()
                    } else {
                        b"Open".as_slice()
                    }
                    .to_vec(),
                    value: Variant::Void,
                    activator: Some(player),
                    caller: Some(player),
                    output_action: None,
                    producer_sequence: tick + 1,
                })],
            )
            .unwrap();
        let request_id = started
            .records
            .iter()
            .find_map(|record| match record.transition {
                Transition::Request(RuntimeRequest::Mover { request_id, .. }) => Some(request_id),
                _ => None,
            })
            .unwrap();
        let completed = world
            .phase(
                tick + 1,
                &[WorldCommand::MoverCompleted {
                    entity: mover,
                    request_id,
                }],
            )
            .unwrap();
        assert!(completed.records.iter().any(|record| matches!(
            record.transition,
            Transition::Request(RuntimeRequest::ExternalInput {
                activator: Some(activator),
                caller: Some(caller),
                ..
            }) if activator == expected_activator && caller == mover
        )));
    }

    let closing = world
        .phase(
            6,
            &[WorldCommand::Input(InputRecord {
                target: EventTarget::Direct(door),
                input: b"Close".to_vec(),
                value: Variant::Void,
                activator: Some(player),
                caller: Some(player),
                output_action: None,
                producer_sequence: 7,
            })],
        )
        .unwrap();
    let request_id = closing
        .records
        .iter()
        .find_map(|record| match record.transition {
            Transition::Request(RuntimeRequest::Mover { request_id, .. }) => Some(request_id),
            _ => None,
        })
        .unwrap();
    let closed = world
        .phase(
            7,
            &[WorldCommand::MoverCompleted {
                entity: door,
                request_id,
            }],
        )
        .unwrap();
    assert!(closed.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::ExternalInput {
            activator: Some(activator),
            caller: Some(caller),
            ..
        }) if activator == player && caller == door
    )));
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

#[test]
fn typed_fields_variant_conversion_and_attachment_parenting_are_atomic() {
    let bytes = br#"
{"classname" "func_brush" "targetname" "parent" "origin" "10 0 0"}
{"classname" "typed" "targetname" "child" "parentname" "parent,muzzle" "origin" "15 0 0"
 "count" "7tail" "ratio" "1.5" "enabled" "1" "vector" "[1 2 3]"
 "position" "4 5 6" "color" "9 8 7" "reference" "parent"}
"#;
    let mut world = compile(bytes, |config| {
        config.field_bindings.push(ClassFieldBinding {
            classname: b"typed".to_vec(),
            fields: vec![
                FieldBinding {
                    key: b"count".to_vec(),
                    field_type: FieldType::Integer,
                    writable_input: Some(b"SetCount".to_vec()),
                },
                FieldBinding {
                    key: b"ratio".to_vec(),
                    field_type: FieldType::Float,
                    writable_input: None,
                },
                FieldBinding {
                    key: b"enabled".to_vec(),
                    field_type: FieldType::Boolean,
                    writable_input: None,
                },
                FieldBinding {
                    key: b"vector".to_vec(),
                    field_type: FieldType::Vector,
                    writable_input: None,
                },
                FieldBinding {
                    key: b"position".to_vec(),
                    field_type: FieldType::PositionVector,
                    writable_input: None,
                },
                FieldBinding {
                    key: b"color".to_vec(),
                    field_type: FieldType::Color,
                    writable_input: None,
                },
                FieldBinding {
                    key: b"reference".to_vec(),
                    field_type: FieldType::Handle,
                    writable_input: None,
                },
                FieldBinding {
                    key: b"missing".to_vec(),
                    field_type: FieldType::String,
                    writable_input: None,
                },
            ],
        });
        config.initial_attachments.push(InitialAttachmentBinding {
            parent_source_index: 0,
            attachment: b"muzzle".to_vec(),
            parent_space_transform: Transform {
                origin: [2.0, 0.0, 0.0],
                angles: [0.0; 3],
            },
        });
    });
    let parent = world.resolve(b"parent", None, None, None)[0];
    let child = world.resolve(b"child", None, None, None)[0];
    let state = world.entity(child).unwrap();
    assert_eq!(state.parent, Some(parent));
    assert_eq!(state.local_transform.origin, [3.0, 0.0, 0.0]);
    assert_eq!(state.fields[0].value, Some(Variant::Integer(7)));
    assert_eq!(state.fields[5].value, Some(Variant::Color([9, 8, 7, 255])));
    assert_eq!(state.fields[6].value, Some(Variant::Handle(parent)));
    assert_eq!(state.fields[7].coverage, playsrc_entity::Coverage::Missing);

    world
        .phase(
            1,
            &[input(
                child,
                b"SetCount",
                Variant::String(b"9tail".to_vec()),
                1,
            )],
        )
        .unwrap();
    assert_eq!(
        world.entity(child).unwrap().fields[0].value,
        Some(Variant::Integer(9))
    );
    world
        .phase(
            2,
            &[
                input(
                    child,
                    b"AddOutput",
                    Variant::String(b"count 12".to_vec()),
                    2,
                ),
                input(
                    child,
                    b"AddOutput",
                    Variant::String(b"OnUser1 parent:Kill::0:1".to_vec()),
                    3,
                ),
            ],
        )
        .unwrap();
    assert_eq!(
        world.entity(child).unwrap().fields[0].value,
        Some(Variant::Integer(12))
    );
    assert!(world.entity(child).unwrap().outputs.iter().any(|action| {
        action.output == b"OnUser1" && action.target == b"parent" && action.input == b"Kill"
    }));
    let before = world.snapshot().unwrap().bytes().to_vec();
    world
        .phase(3, &[input(child, b"SetCount", Variant::Bool(true), 4)])
        .unwrap();
    assert_eq!(
        world.entity(child).unwrap().fields[0].value,
        Some(Variant::Integer(12))
    );
    assert_ne!(world.snapshot().unwrap().bytes(), before);

    world
        .phase(
            4,
            &[input(
                child,
                b"SetParentAttachment",
                Variant::String(b"muzzle".to_vec()),
                5,
            )],
        )
        .unwrap();
    assert_eq!(
        world.entity(child).unwrap().local_transform,
        Transform::IDENTITY
    );
    assert_eq!(
        world.entity(child).unwrap().world_transform.origin,
        [12.0, 0.0, 0.0]
    );
}

#[test]
fn point_template_removes_prototype_fixes_internal_names_and_restores_instance_sequence() {
    let bytes = br#"
{"classname" "point_template" "targetname" "maker" "origin" "100 0 0" "Template01" "piece"
 "OnEntitySpawned" "piece,FireUser1,,0,-1"}
{"classname" "logic_relay" "targetname" "piece" "origin" "110 0 0"
 "OnUser1" "piece,Trigger,,0,-1"}
"#;
    let mut world = compile(bytes, |_| {});
    let maker = world.resolve(b"maker", None, None, None)[0];
    assert!(world.resolve(b"piece", None, None, None).is_empty());
    world
        .phase(0, &[input(maker, b"ForceSpawn", Variant::Void, 1)])
        .unwrap();
    let first = world.resolve(b"piece&0001", None, None, None)[0];
    assert_eq!(
        world.entity(first).unwrap().world_transform.origin,
        [110.0, 0.0, 0.0]
    );
    assert!(
        world
            .entity(first)
            .unwrap()
            .outputs
            .iter()
            .all(|action| action.target == b"piece&0001")
    );

    let snapshot = world.snapshot().unwrap();
    world
        .phase(1, &[input(maker, b"ForceSpawn", Variant::Void, 2)])
        .unwrap();
    assert_eq!(world.resolve(b"piece&0002", None, None, None).len(), 1);
    let second = world.resolve(b"piece&0002", None, None, None)[0];
    assert_eq!(world.entity(first).unwrap().definition.targetname.as_deref(), Some(b"piece&0001".as_slice()));
    assert_eq!(world.entity(second).unwrap().definition.targetname.as_deref(), Some(b"piece&0002".as_slice()));
    assert!(!std::sync::Arc::ptr_eq(&world.entity(first).unwrap().definition, &world.entity(second).unwrap().definition));
    let mut restored = compile(bytes, |_| {});
    restored.restore(&snapshot).unwrap();
    let restored_maker = restored.resolve(b"maker", None, None, None)[0];
    restored
        .phase(1, &[input(restored_maker, b"ForceSpawn", Variant::Void, 2)])
        .unwrap();
    assert_eq!(restored.resolve(b"piece&0002", None, None, None).len(), 1);
    assert_eq!(restored.snapshot().unwrap().bytes(), world.snapshot().unwrap().bytes());
}

#[test]
fn template_member_bound_rejects_compile_without_a_partial_world() {
    let graph = parse(
        br#"
{"classname" "point_template" "Template01" "piece"}
{"classname" "logic_relay" "targetname" "piece"}
{"classname" "logic_relay" "targetname" "piece"}
"#,
        playsrc_entity::Limits::default(),
    )
    .unwrap();
    let error = EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            limits: RuntimeLimits {
                max_template_members: 1,
                ..RuntimeLimits::default()
            },
            ..EntityWorldConfig::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code, RuntimeFailureCode::TemplateLimit);
}

#[test]
fn logic_timer_fixed_alternating_adjustment_and_snapshot_continuation_match() {
    let bytes = br#"
{"classname" "func_brush" "targetname" "sink" "StartDisabled" "1"}
{"classname" "logic_timer" "targetname" "timer" "RefireTime" "0.02" "spawnflags" "1"
 "OnTimerLow" "sink,Enable,,0,-1" "OnTimerHigh" "sink,Disable,,0,-1"}
"#;
    let mut world = compile(bytes, |_| {});
    let timer = world.resolve(b"timer", None, None, None)[0];
    let sink = world.resolve(b"sink", None, None, None)[0];
    world.phase(1, &[]).unwrap();
    assert!(
        matches!(world.entity(sink).unwrap().behavior, BehaviorState::Brush(ref state) if !state.enabled)
    );
    world.phase(2, &[]).unwrap();
    assert!(
        matches!(world.entity(sink).unwrap().behavior, BehaviorState::Brush(ref state) if state.enabled)
    );
    world
        .phase(
            3,
            &[input(timer, b"SubtractFromTimer", Variant::float(1.0), 1)],
        )
        .unwrap();
    assert!(
        matches!(world.entity(sink).unwrap().behavior, BehaviorState::Brush(ref state) if !state.enabled)
    );
    let snapshot = world.snapshot().unwrap();
    let mut restored = compile(bytes, |_| {});
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        world.phase(5, &[]).unwrap(),
        restored.phase(5, &[]).unwrap()
    );
    assert_eq!(
        world.snapshot().unwrap().bytes(),
        restored.snapshot().unwrap().bytes()
    );
}

#[test]
fn rotating_door_platform_and_train_publish_exact_transform_requests() {
    let bytes = br#"
{"classname" "func_door_rotating" "targetname" "door" "model" "*1" "distance" "90" "speed" "90"}
{"classname" "func_platrot" "targetname" "plat" "model" "*2" "height" "16" "rotation" "45" "speed" "80"}
{"classname" "path_corner" "targetname" "a" "target" "b" "origin" "0 0 0"}
{"classname" "path_corner" "targetname" "b" "origin" "100 0 0"}
{"classname" "func_train" "model" "*3" "target" "a" "speed" "50"}
"#;
    let mut world = compile(bytes, |config| {
        for model in 1..=3 {
            config.model_bounds.push(ModelBounds {
                model,
                mins: [-2.0; 3],
                maxs: [2.0; 3],
            });
        }
    });
    let door = world.resolve(b"door", None, None, None)[0];
    let plat = world.resolve(b"plat", None, None, None)[0];
    let train = world
        .live_handles()
        .into_iter()
        .find(|handle| {
            world
                .entity(*handle)
                .is_some_and(|entity| entity.classname == b"func_train")
        })
        .unwrap();
    let opened = world
        .phase(0, &[input(door, b"Open", Variant::Void, 1)])
        .unwrap();
    assert!(opened.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            world_angles_destination: [0.0, 90.0, 0.0],
            angular_velocity: [0.0, 90.0, 0.0],
            ..
        }) if entity == door
    )));
    let down = world.entity(plat).unwrap().world_transform.origin;
    assert_eq!(down[2], 0.0, "named platform starts at its authored top");
    let platform = world
        .phase(1, &[input(plat, b"GoDown", Variant::Void, 2)])
        .unwrap();
    assert!(platform.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            world_destination: [0.0, 0.0, -16.0],
            world_angles_destination: [0.0, 0.0, 0.0],
            ..
        }) if entity == plat
    )));
    let train_phase = world.phase(11, &[]).unwrap();
    assert!(train_phase.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            world_destination: [100.0, 0.0, 0.0],
            speed: 50.0,
            ..
        }) if entity == train
    )));
    assert!(
        matches!(world.entity(door).unwrap().behavior, BehaviorState::Mover(ref state) if state.class == MoverClass::RotatingDoor)
    );
}

#[test]
fn rotating_button_continuous_rotator_momentary_button_and_tracktrain_inputs_are_distinct() {
    let bytes = br#"
{"classname" "func_rot_button" "targetname" "rot_button" "model" "*1" "distance" "90" "speed" "45"}
{"classname" "momentary_rot_button" "targetname" "momentary" "model" "*2" "distance" "180" "StartPosition" "0.25" "speed" "90"}
{"classname" "func_rotating" "targetname" "fan" "model" "*3" "maxspeed" "120" "spawnflags" "4"}
{"classname" "func_rotating" "targetname" "fan_accel" "model" "*5" "maxspeed" "100" "fanfriction" "100" "spawnflags" "20"}
{"classname" "path_track" "targetname" "track_a" "target" "track_b" "origin" "0 0 0"}
{"classname" "path_track" "targetname" "track_b" "origin" "100 0 0"}
{"classname" "func_tracktrain" "targetname" "tracktrain" "target" "track_a" "model" "*4" "speed" "100" "startspeed" "100"}
"#;
    let mut world = compile(bytes, |config| {
        for model in 1..=5 {
            config.model_bounds.push(ModelBounds {
                model,
                mins: [-1.0; 3],
                maxs: [1.0; 3],
            });
        }
    });
    let rot_button = world.resolve(b"rot_button", None, None, None)[0];
    let momentary = world.resolve(b"momentary", None, None, None)[0];
    let fan = world.resolve(b"fan", None, None, None)[0];
    let fan_accel = world.resolve(b"fan_accel", None, None, None)[0];
    let tracktrain = world.resolve(b"tracktrain", None, None, None)[0];

    let button = world
        .phase(0, &[input(rot_button, b"Press", Variant::Void, 1)])
        .unwrap();
    assert!(button.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            world_angles_destination: [0.0, 90.0, 0.0],
            angular_velocity: [0.0, 45.0, 0.0],
            ..
        }) if entity == rot_button
    )));
    world
        .phase(
            1,
            &[input(
                momentary,
                b"SetPositionImmediately",
                Variant::float(0.75),
                2,
            )],
        )
        .unwrap();
    assert_eq!(
        world.entity(momentary).unwrap().world_transform.angles,
        [0.0, 90.0, 0.0]
    );
    let rotating = world
        .phase(2, &[input(fan, b"Start", Variant::Void, 3)])
        .unwrap();
    assert!(rotating.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            angular_velocity: [0.0, 0.0, 120.0],
            continuous: true,
            ..
        }) if entity == fan
    )));
    let reversed = world
        .phase(3, &[input(fan, b"Reverse", Variant::Void, 4)])
        .unwrap();
    assert!(reversed.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            angular_velocity: [0.0, 0.0, -120.0],
            ..
        }) if entity == fan
    )));
    let acceleration_start = world
        .phase(4, &[input(fan_accel, b"Start", Variant::Void, 5)])
        .unwrap();
    assert!(!acceleration_start.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover { entity, .. }) if entity == fan_accel
    )));
    let acceleration_step = world.phase(15, &[]).unwrap();
    assert!(acceleration_step.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            angular_velocity: [0.0, 0.0, 20.0],
            ..
        }) if entity == fan_accel
    )));
    assert!(acceleration_step.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::Mover {
            entity,
            world_destination: [10.0, 0.0, 0.0],
            speed: 100.0,
            ..
        }) if entity == tracktrain
    )));
    assert!(
        matches!(world.entity(tracktrain).unwrap().behavior, BehaviorState::Mover(ref state) if state.class == MoverClass::TrackTrain)
    );
}

#[test]
fn timer_random_fields_and_catapult_mutable_inputs_preserve_due_state() {
    let bytes = br#"
{"classname" "subject" "targetname" "player"}
{"classname" "info_target" "targetname" "launch"}
{"classname" "logic_timer" "targetname" "timer" "UseRandomTime" "1" "LowerRandomBound" "0.02" "UpperRandomBound" "0.02"}
{"classname" "trigger_catapult" "targetname" "cat" "launchDirection" "-90 0 0" "playerSpeed" "300" "physicsSpeed" "200" "launchTarget" "launch"}
"#;
    let mut world = compile(bytes, |_| {});
    let timer = world.resolve(b"timer", None, None, None)[0];
    let player = world.resolve(b"player", None, None, None)[0];
    let catapult = world.resolve(b"cat", None, None, None)[0];
    world
        .phase(
            0,
            &[
                input(timer, b"UseRandomTime", Variant::Integer(0), 1),
                input(timer, b"RefireTime", Variant::float(0.001), 2),
                input(timer, b"ResetTimer", Variant::Void, 3),
                input(timer, b"AddToTimer", Variant::float(0.01), 4),
                input(catapult, b"SetPlayerSpeed", Variant::float(450.0), 5),
                input(catapult, b"SetPhysicsSpeed", Variant::float(250.0), 6),
                input(
                    catapult,
                    b"SetExactVelocityChoiceType",
                    Variant::Integer(2),
                    7,
                ),
            ],
        )
        .unwrap();
    assert!(
        matches!(world.entity(timer).unwrap().behavior, BehaviorState::Timer(ref state)
        if !state.use_random && f32::from_bits(state.interval_bits) == 0.01 && state.next_fire_tick.is_some())
    );
    let entered = world
        .phase(
            1,
            &[WorldCommand::Contact(ContactRecord {
                trigger: catapult,
                subject: player,
                kind: ContactKind::Enter,
                external_filter_result: None,
                producer_sequence: 8,
            })],
        )
        .unwrap();
    assert!(entered.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::TriggerEffect {
            effect: TriggerEffectData::Catapult {
                player_speed_bits,
                physics_speed_bits,
                exact_choice: 2,
                target: Some(_),
                ..
            },
            ..
        }) if f32::from_bits(player_speed_bits) == 450.0 && f32::from_bits(physics_speed_bits) == 250.0
    )));
    let cooldown = world
        .phase(
            2,
            &[WorldCommand::Contact(ContactRecord {
                trigger: catapult,
                subject: player,
                kind: ContactKind::Stay,
                external_filter_result: None,
                producer_sequence: 9,
            })],
        )
        .unwrap();
    assert!(!cooldown.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::TriggerEffect {
            kind: TriggerKind::Catapult,
            ..
        })
    )));
}

#[test]
fn trigger_payloads_hurt_cadence_and_push_once_lifecycle_are_entity_owned() {
    let bytes = br#"
{"classname" "subject" "targetname" "player"}
{"classname" "info_teleport_destination" "targetname" "dest"}
{"classname" "trigger_push" "targetname" "push" "pushdir" "0 0 0" "speed" "200" "spawnflags" "128"}
{"classname" "trigger_hurt" "targetname" "hurt" "damage" "10" "damagecap" "40" "damagemodel" "1"}
{"classname" "trigger_teleport" "targetname" "tele" "target" "dest" "spawnflags" "32"}
"#;
    let mut world = compile(bytes, |_| {});
    let subject = world.resolve(b"player", None, None, None)[0];
    let push = world.resolve(b"push", None, None, None)[0];
    let hurt = world.resolve(b"hurt", None, None, None)[0];
    let tele = world.resolve(b"tele", None, None, None)[0];
    let enter = |trigger, sequence| {
        WorldCommand::Contact(ContactRecord {
            trigger,
            subject,
            kind: ContactKind::Enter,
            external_filter_result: None,
            producer_sequence: sequence,
        })
    };
    let pushed = world.phase(0, &[enter(push, 1)]).unwrap();
    assert!(pushed.records.iter().any(|record| matches!(
        &record.transition,
        Transition::Request(RuntimeRequest::TriggerEffect {
            effect: TriggerEffectData::Push {
                velocity: [200.0, 0.0, 0.0],
                mode: PushMode::ImpulseAndRemove
            },
            ..
        })
    )));
    assert!(world.entity(push).is_none());
    let hurt_phase = world.phase(1, &[enter(hurt, 2)]).unwrap();
    assert!(hurt_phase.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::TriggerEffect {
            effect: TriggerEffectData::Hurt { damage_bits, .. },
            ..
        }) if f32::from_bits(damage_bits) == 5.0
    )));
    let teleported = world.phase(2, &[enter(tele, 3)]).unwrap();
    assert!(teleported.records.iter().any(|record| matches!(
        record.transition,
        Transition::Request(RuntimeRequest::TriggerEffect {
            effect: TriggerEffectData::Teleport {
                destination: Some(_),
                preserve_angles: true,
                ..
            },
            ..
        })
    )));
}

#[test]
fn breakable_dynamic_prop_and_pickup_lifecycle_continue_through_snapshots() {
    let bytes = br#"
{"classname" "subject" "targetname" "player"}
{"classname" "func_breakable" "targetname" "glass" "model" "*1" "health" "10"
 "OnHealthChanged" "prop,TurnOff,,0,-1"}
{"classname" "prop_dynamic" "targetname" "prop" "DefaultAnim" "idle"}
{"classname" "item_test" "targetname" "item"}
"#;
    let mut world = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [8.0; 3],
        });
        config.pickup_classes.push(b"item_test".to_vec());
    });
    let player = world.resolve(b"player", None, None, None)[0];
    let glass = world.resolve(b"glass", None, None, None)[0];
    let prop = world.resolve(b"prop", None, None, None)[0];
    let item = world.resolve(b"item", None, None, None)[0];
    world
        .phase(
            0,
            &[WorldCommand::DamageValue {
                entity: glass,
                attacker: Some(player),
                damage: 4,
            }],
        )
        .unwrap();
    assert!(
        matches!(world.entity(glass).unwrap().behavior, BehaviorState::Breakable(ref state) if state.health == 6)
    );
    assert!(
        matches!(world.entity(prop).unwrap().behavior, BehaviorState::DynamicProp(ref state) if !state.visible)
    );
    let animation = world
        .phase(
            1,
            &[input(
                prop,
                b"SetAnimation",
                Variant::String(b"open".to_vec()),
                2,
            )],
        )
        .unwrap();
    assert!(animation.records.iter().any(|record| matches!(
        &record.transition,
        Transition::Request(RuntimeRequest::ExternalInput { input, .. }) if input == b"SetAnimation"
    )));
    world
        .phase(
            2,
            &[WorldCommand::DynamicPropAnimationStarted {
                entity: prop,
                accepted: true,
            }],
        )
        .unwrap();
    world
        .phase(
            3,
            &[WorldCommand::PickupContact {
                entity: item,
                subject: player,
                unobstructed: true,
            }],
        )
        .unwrap();
    world
        .phase(
            4,
            &[WorldCommand::PickupResult {
                entity: item,
                subject: player,
                accepted: true,
                respawn_ticks: Some(3),
                respawn_transform: None,
            }],
        )
        .unwrap();
    assert!(
        matches!(world.entity(item).unwrap().behavior, BehaviorState::Pickup(ref state) if !state.visible && !state.touchable)
    );
    let snapshot = world.snapshot().unwrap();
    let mut restored = compile(bytes, |config| {
        config.model_bounds.push(ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [8.0; 3],
        });
        config.pickup_classes.push(b"item_test".to_vec());
    });
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        world.phase(7, &[]).unwrap(),
        restored.phase(7, &[]).unwrap()
    );
    assert!(
        matches!(world.entity(item).unwrap().behavior, BehaviorState::Pickup(ref state) if state.visible && state.touchable)
    );
}
