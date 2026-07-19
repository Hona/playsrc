use playsrc_bsp::{Limits as BspLimits, Profile as BspProfile, parse as parse_bsp};
use playsrc_collision::{
    ContactEdgeKind, ContactLimits, ContactSnapshot, ContactSubject, Hull, ObjectInput, ObjectRole,
    Snapshot, SnapshotLimits, SnapshotShape, Transform as CollisionTransform, TriggerVolume,
    compile as compile_collision, produce_trigger_contacts,
};
use playsrc_entity::{EntityWorld, EntityWorldConfig, Limits, ModelBounds, parse};
use std::{fs, path::PathBuf};

const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";

#[test]
#[ignore = "requires playsrc.local.json and the configured TF2 target cache"]
fn configured_jump_beef_inventory_and_selected_generic_runtime_are_fixed() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = fs::read(root.join("playsrc.local.json")).unwrap();
    let cache = PathBuf::from(configured_string(&config, "sourceCacheDir"));
    let bytes = fs::read(
        cache
            .join("objects/sha256")
            .join(&BSP_SHA256[..2])
            .join(BSP_SHA256),
    )
    .unwrap();
    let bsp = parse_bsp(&bytes, BspProfile::Source2013V20, BspLimits::default()).unwrap();
    let graph = parse(bsp.lumps[0].bytes(&bsp), Limits::default()).unwrap();
    assert_eq!(graph.inventory.entity_count, 361);
    assert_eq!(graph.inventory.pair_count, 3_674);
    assert_eq!(graph.inventory.parsed_connections, 66);
    assert_eq!(graph.inventory.malformed_connections, 1);
    let expected_classes = [
        ("func_brush", 7),
        ("func_button", 5),
        ("func_door", 4),
        ("func_movelinear", 1),
        ("func_regenerate", 22),
        ("func_respawnroom", 3),
        ("game_text", 51),
        ("info_observer_point", 1),
        ("info_player_teamspawn", 10),
        ("info_teleport_destination", 25),
        ("infodecal", 39),
        ("item_ammopack_full", 3),
        ("light", 41),
        ("light_environment", 1),
        ("light_spot", 30),
        ("logic_auto", 1),
        ("prop_dynamic", 33),
        ("team_round_timer", 1),
        ("tf_gamerules", 1),
        ("trigger_hurt", 2),
        ("trigger_multiple", 22),
        ("trigger_teleport", 56),
        ("water_lod_control", 1),
        ("worldspawn", 1),
    ];
    assert_eq!(graph.inventory.class_counts.len(), expected_classes.len());
    for (class, count) in expected_classes {
        assert_eq!(
            graph.inventory.class_counts.get(class.as_bytes()),
            Some(&count),
            "configured class {class}"
        );
    }
    let models = match &bsp.lumps[14].records {
        playsrc_bsp::LumpData::Models(models) => models,
        _ => panic!("configured model lump"),
    };
    let model_bounds = models
        .iter()
        .enumerate()
        .map(|(model, bounds)| ModelBounds {
            model,
            mins: [
                bounds.mins.x.value(),
                bounds.mins.y.value(),
                bounds.mins.z.value(),
            ],
            maxs: [
                bounds.maxs.x.value(),
                bounds.maxs.y.value(),
                bounds.maxs.z.value(),
            ],
        })
        .collect();
    let (world, _) = EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            source_identity: 0xbeef,
            registry_identity: 0x534d_4546,
            model_bounds,
            ..EntityWorldConfig::default()
        },
    )
    .unwrap();
    assert_eq!(world.live_handles().len(), 361);
    assert_eq!(
        world
            .brush_model_presentation(world.revision())
            .unwrap()
            .models
            .len(),
        122
    );
    for (class, count) in [
        (b"func_button".as_slice(), 5),
        (b"func_door".as_slice(), 4),
        (b"func_movelinear".as_slice(), 1),
        (b"trigger_multiple".as_slice(), 22),
        (b"trigger_hurt".as_slice(), 2),
        (b"trigger_teleport".as_slice(), 56),
        (b"prop_dynamic".as_slice(), 33),
    ] {
        assert_eq!(
            world
                .live_handles()
                .iter()
                .filter(|handle| world
                    .entity(**handle)
                    .is_some_and(|entity| entity.classname.eq_ignore_ascii_case(class)))
                .count(),
            count
        );
    }

    let trigger = graph
        .entities
        .iter()
        .find(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|class| class.eq_ignore_ascii_case(b"trigger_multiple"))
        })
        .unwrap();
    let model = trigger.bsp_model_index.unwrap();
    let bounds = &models[model];
    let origin = vector_field(trigger, b"origin").unwrap_or([0.0; 3]);
    let angles = vector_field(trigger, b"angles").unwrap_or([0.0; 3]);
    let center = [
        origin[0] + (bounds.mins.x.value() + bounds.maxs.x.value()) * 0.5,
        origin[1] + (bounds.mins.y.value() + bounds.maxs.y.value()) * 0.5,
        origin[2] + (bounds.mins.z.value() + bounds.maxs.z.value()) * 0.5,
    ];
    let collision_world = compile_collision(&bsp).unwrap();
    let collision = Snapshot::compile(
        &collision_world,
        1,
        vec![ObjectInput {
            identity: trigger.index as u64,
            role: ObjectRole::Entity,
            enabled: true,
            transform: CollisionTransform { origin, angles },
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel { model },
        }],
        SnapshotLimits::default(),
    )
    .unwrap();
    let contacts = produce_trigger_contacts(
        &collision_world,
        &collision,
        &ContactSnapshot::empty(1, ContactLimits::default()).unwrap(),
        &[TriggerVolume {
            identity: trigger.index as u64,
            enabled: true,
            mask: u32::MAX,
        }],
        &[ContactSubject {
            identity: 0xfeed,
            enabled: true,
            position: center,
            hull: Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            },
            mask: u32::MAX,
        }],
        |_, _| true,
    )
    .unwrap();
    assert_eq!(contacts.edges.len(), 1);
    assert_eq!(contacts.edges[0].kind, ContactEdgeKind::Enter);
}

fn vector_field(entity: &playsrc_entity::Entity, key: &[u8]) -> Option<[f32; 3]> {
    let value = entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))?
        .value
        .as_slice();
    let values = std::str::from_utf8(value)
        .ok()?
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3).then(|| [values[0], values[1], values[2]])
}

fn configured_string(bytes: &[u8], key: &str) -> String {
    let text = std::str::from_utf8(bytes).unwrap();
    let marker = format!("\"{key}\"");
    let tail = &text[text.find(&marker).unwrap() + marker.len()..];
    let tail = &tail[tail.find(':').unwrap() + 1..];
    let mut chars = tail.trim_start().chars();
    assert_eq!(chars.next(), Some('"'));
    let mut output = String::new();
    while let Some(character) = chars.next() {
        match character {
            '"' => return output,
            '\\' => match chars.next().unwrap() {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                '/' => output.push('/'),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                escape => panic!("unsupported local configuration escape {escape}"),
            },
            character => output.push(character),
        }
    }
    panic!("unterminated local configuration value")
}
