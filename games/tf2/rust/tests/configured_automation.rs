use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph, ModelBounds};
use playsrc_movement::{Error as MoveError, Trace, Tracer};
use playsrc_tf2::{
    Command, Event, GameplayWorld, MapRuntime, MoverResult, MoverResultKind, ProjectileEventKind,
    RegenerateModelAnimation, RocketTraceResult, Session,
};
use std::{collections::BTreeSet, fs, path::PathBuf};

const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const BSP_BYTES: usize = 33_379_388;
const SOURCE_IDENTITY: u64 = 0x33a0_6ab5_1020_e2b2;

#[derive(Clone)]
struct TestWorld {
    overlap_model: Option<usize>,
}

impl Tracer for TestWorld {
    fn trace(
        &self,
        _start: [f32; 3],
        end: [f32; 3],
        _hull: Hull,
        _mask: u32,
    ) -> Result<Trace, MoveError> {
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
}

impl GameplayWorld for TestWorld {
    fn overlaps_model_hull(
        &self,
        model: usize,
        _origin: [f32; 3],
        _position: [f32; 3],
        _hull: Hull,
    ) -> Result<bool, MoveError> {
        Ok(self.overlap_model == Some(model))
    }
}

#[test]
#[ignore = "requires playsrc.local.json and the configured jump_beef BSP"]
fn configured_rockets_drive_every_linked_door_platform_cycle_and_locker_state() {
    let (graph, bounds, source_identity) = configured_graph();
    let mut links = Vec::new();
    for button in graph
        .entities
        .iter()
        .filter(|entity| class(entity, b"func_button"))
    {
        for pair in &button.pairs {
            if !pair.key.eq_ignore_ascii_case(b"OnDamaged") {
                continue;
            }
            let fields = pair.value.split(|byte| *byte == b',').collect::<Vec<_>>();
            if fields
                .get(1)
                .is_none_or(|input| !input.eq_ignore_ascii_case(b"Open"))
            {
                continue;
            }
            let target = fields[0];
            let door = graph
                .entities
                .iter()
                .find(|entity| {
                    class(entity, b"func_door")
                        && entity
                            .targetname
                            .as_deref()
                            .is_some_and(|name| name.eq_ignore_ascii_case(target))
                })
                .unwrap();
            links.push((button.index as u32, door.index as u32));
        }
    }
    links.sort_unstable();
    links.dedup();
    assert_eq!(links.len(), 5);
    assert_eq!(
        links
            .iter()
            .map(|(_, door)| *door)
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );

    for (button, door) in links {
        let map = MapRuntime::compile(&graph, 0.015, source_identity, bounds.clone()).unwrap();
        let mut session = Session::new(
            TestWorld {
                overlap_model: None,
            },
            [0.0; 3],
            map,
        );
        for _ in 0..34 {
            session.advance(Command::default()).unwrap();
        }
        let fired = session
            .advance(Command {
                fire: true,
                ..Command::default()
            })
            .unwrap();
        assert!(
            fired
                .projectile_events
                .iter()
                .any(|event| event.kind == ProjectileEventKind::Fire)
        );
        let request = session.rocket_trace_requests()[0];
        let damaged = session
            .advance_with_external(
                Command::default(),
                &[],
                &[RocketTraceResult {
                    projectile: request.projectile,
                    tick: session.producer_snapshot().tick,
                    end: request.end,
                    solid: true,
                    sky: false,
                    normal: Some([1.0, 0.0, 0.0]),
                    direct_target: Some(button),
                }],
                None,
            )
            .unwrap();
        assert!(damaged.entity_events.iter().any(|event| {
            event.entity == door && event.name.eq_ignore_ascii_case(b"Open") && event.accepted
        }));
        assert!(
            session
                .mover_requests()
                .iter()
                .any(|request| request.entity == door && request.opening)
        );
    }

    let logic_auto = graph
        .entities
        .iter()
        .find(|entity| class(entity, b"logic_auto"))
        .unwrap();
    let platform_name = logic_auto
        .pairs
        .iter()
        .find(|pair| {
            pair.key.eq_ignore_ascii_case(b"OnMapSpawn")
                && pair
                    .value
                    .windows(5)
                    .any(|window| window.eq_ignore_ascii_case(b",Open"))
        })
        .unwrap()
        .value
        .split(|byte| *byte == b',')
        .next()
        .unwrap();
    let platform = graph
        .entities
        .iter()
        .find(|entity| {
            class(entity, b"func_movelinear")
                && entity
                    .targetname
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(platform_name))
        })
        .unwrap();
    assert_eq!(field(platform, b"speed"), Some(b"75".as_slice()));
    assert_eq!(field(platform, b"MoveDistance"), Some(b"650".as_slice()));
    assert_eq!(field(platform, b"movedir"), Some(b"0 90 0".as_slice()));
    let map = MapRuntime::compile(&graph, 0.015, source_identity, bounds.clone()).unwrap();
    let mut session = Session::new(
        TestWorld {
            overlap_model: None,
        },
        [0.0; 3],
        map,
    );
    let opening = loop {
        session.advance(Command::default()).unwrap();
        if let Some(request) = session
            .mover_requests()
            .iter()
            .find(|request| request.entity == platform.index as u32)
            .copied()
        {
            break request;
        }
        assert!(session.producer_snapshot().tick <= 15);
    };
    assert!(opening.opening);
    session
        .apply_mover_results(&[MoverResult {
            request_id: opening.request_id,
            entity: opening.entity,
            kind: MoverResultKind::Completed,
            transform: playsrc_entity::Transform {
                origin: opening.destination,
                angles: [0.0; 3],
            },
            carry: [0.0, 9.0, 0.0],
        }])
        .unwrap();
    let closing = session
        .mover_requests()
        .iter()
        .find(|request| request.entity == platform.index as u32)
        .copied()
        .unwrap();
    assert!(!closing.opening);
    assert_eq!(session.movement_state().position[1], 9.0);
    session.advance(Command::default()).unwrap();
    assert!(
        session
            .mover_requests()
            .iter()
            .any(|request| request.request_id == closing.request_id)
    );

    let regenerate = graph
        .entities
        .iter()
        .filter(|entity| class(entity, b"func_regenerate"))
        .collect::<Vec<_>>();
    assert_eq!(regenerate.len(), 22);
    assert!(regenerate.iter().all(|entity| {
        field(entity, b"TeamNum") == Some(b"0".as_slice())
            && field(entity, b"StartDisabled") == Some(b"0".as_slice())
    }));
    let associated_name = field(regenerate[0], b"associatedmodel").unwrap();
    assert!(
        regenerate
            .iter()
            .all(|entity| field(entity, b"associatedmodel") == Some(associated_name))
    );
    let locker = graph
        .entities
        .iter()
        .find(|entity| {
            entity
                .targetname
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(associated_name))
        })
        .unwrap();
    assert!(class(locker, b"prop_dynamic"));
    assert_eq!(
        field(locker, b"model"),
        Some(b"models/props_gameplay/resupply_locker.mdl".as_slice())
    );
    assert_eq!(
        field(locker, b"origin"),
        Some(b"5512 3440 -2800".as_slice())
    );
    assert_eq!(field(locker, b"angles"), Some(b"0 179.5 0".as_slice()));
    assert_eq!(field(locker, b"SetBodyGroup"), Some(b"0".as_slice()));
    let overlap_model = regenerate[0].bsp_model_index.unwrap();
    let map = MapRuntime::compile(&graph, 0.015, source_identity, bounds).unwrap();
    let mut session = Session::new(
        TestWorld {
            overlap_model: Some(overlap_model),
        },
        [0.0; 3],
        map,
    );
    let supplied = session.advance(Command::default()).unwrap();
    assert!(
        supplied
            .events
            .iter()
            .any(|event| matches!(event, Event::Resupplied { .. }))
    );
    let model = session.regenerate_model_events()[0];
    assert_eq!(model.associated_model, locker.index as u32);
    assert_eq!(model.animation, RegenerateModelAnimation::Open);
    assert_eq!(model.body, 0);
}

fn configured_graph() -> (Graph, Vec<ModelBounds>, u64) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read(root.join("playsrc.local.json")).unwrap();
    let cache = PathBuf::from(configured_string(&config, "sourceCacheDir"));
    assert!(cache.is_absolute());
    let bytes = fs::read(
        cache
            .join("objects/sha256")
            .join(&BSP_SHA256[..2])
            .join(BSP_SHA256),
    )
    .unwrap();
    assert_eq!(bytes.len(), BSP_BYTES);
    assert_eq!(&bytes[..4], b"VBSP");
    assert_eq!(i32_at(&bytes, 4), 20);
    let graph = playsrc_entity::parse(lump(&bytes, 0), playsrc_entity::Limits::default()).unwrap();
    let models = lump(&bytes, 14);
    assert_eq!(models.len() % 48, 0);
    let bounds = models
        .chunks_exact(48)
        .enumerate()
        .map(|(model, bytes)| ModelBounds {
            model,
            mins: [f32_at(bytes, 0), f32_at(bytes, 4), f32_at(bytes, 8)],
            maxs: [f32_at(bytes, 12), f32_at(bytes, 16), f32_at(bytes, 20)],
        })
        .collect();
    (graph, bounds, SOURCE_IDENTITY)
}

fn class(entity: &Entity, expected: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn field<'a>(entity: &'a Entity, name: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.as_slice())
}

fn lump(bytes: &[u8], index: usize) -> &[u8] {
    let header = 8 + index * 16;
    let offset = usize::try_from(i32_at(bytes, header)).unwrap();
    let length = usize::try_from(i32_at(bytes, header + 4)).unwrap();
    &bytes[offset..offset + length]
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_bits(u32::from_le_bytes(
        bytes[offset..offset + 4].try_into().unwrap(),
    ))
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
