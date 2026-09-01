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

#[derive(Clone)]
struct ConfiguredWaterWorld {
    bounds: [[f32; 2]; 3],
    contents: u32,
}

impl Tracer for ConfiguredWaterWorld {
    fn trace(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        _mask: u32,
    ) -> Result<Trace, MoveError> {
        let floor = self.bounds[2][0] - hull.mins[2];
        if end[2] < floor {
            let fraction = ((start[2] - floor) / (start[2] - end[2])).clamp(0.0, 1.0);
            Ok(Trace {
                fraction,
                start_solid: false,
                all_solid: false,
                end: [
                    start[0] + (end[0] - start[0]) * fraction,
                    start[1] + (end[1] - start[1]) * fraction,
                    floor,
                ],
                normal: Some([0.0, 0.0, 1.0]),
                hit: Some(0),
                contents: 1,
            })
        } else {
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

    fn point_contents(&self, point: [f32; 3]) -> Result<u32, MoveError> {
        Ok(
            if point
                .into_iter()
                .zip(self.bounds)
                .all(|(coordinate, [minimum, maximum])| {
                    coordinate > minimum && coordinate < maximum
                })
            {
                self.contents
            } else {
                0
            },
        )
    }
}

impl GameplayWorld for ConfiguredWaterWorld {
    fn overlaps_model_hull(
        &self,
        _model: usize,
        _origin: [f32; 3],
        _position: [f32; 3],
        _hull: Hull,
    ) -> Result<bool, MoveError> {
        Ok(false)
    }
}

#[test]
#[ignore = "requires playsrc.local.json and the configured jump_beef BSP"]
fn configured_water_rocket_jump_preserves_source_hull_force_and_swim_order() {
    let bytes = configured_bsp_bytes();
    let brushes = lump(&bytes, 18);
    let sides = lump(&bytes, 19);
    let planes = lump(&bytes, 1);
    let water = brushes
        .chunks_exact(12)
        .enumerate()
        .filter(|(_, brush)| u32_at(brush, 8) & playsrc_movement::CONTENTS_WATER != 0)
        .collect::<Vec<_>>();
    assert_eq!(water.len(), 1);
    let (index, brush) = water[0];
    assert_eq!(index, 60);
    let contents = u32_at(brush, 8);
    assert_eq!(contents, 0x1000_0020);
    let first = usize::try_from(i32_at(brush, 0)).unwrap();
    let count = usize::try_from(i32_at(brush, 4)).unwrap();
    let mut bounds = [[f32::NEG_INFINITY, f32::INFINITY]; 3];
    for side in sides[first * 8..(first + count) * 8].chunks_exact(8) {
        let plane = usize::from(u16::from_le_bytes(side[..2].try_into().unwrap())) * 20;
        let normal = [
            f32_at(planes, plane),
            f32_at(planes, plane + 4),
            f32_at(planes, plane + 8),
        ];
        let distance = f32_at(planes, plane + 12);
        for axis in 0..3 {
            if (0..3).all(|other| other == axis || normal[other] == 0.0) {
                if normal[axis] == 1.0 {
                    bounds[axis][1] = bounds[axis][1].min(distance);
                } else if normal[axis] == -1.0 {
                    bounds[axis][0] = bounds[axis][0].max(-distance);
                }
            }
        }
    }
    assert_eq!(
        bounds,
        [[-5216.0, -4448.0], [2304.0, 3792.0], [-2416.0, -2160.0]]
    );

    let world = ConfiguredWaterWorld { bounds, contents };
    let mut session = Session::new(world, [-4832.0, 3000.0, -2215.0], MapRuntime::empty(0.015));
    let held_crouch = playsrc_movement::Command {
        crouch: true,
        ..playsrc_movement::Command::default()
    };
    for _ in 0..64 {
        let was_wet = session.movement_state().water_level != 0;
        let snapshot = session
            .advance(Command {
                movement: held_crouch,
                ..Command::default()
            })
            .unwrap();
        if was_wet {
            assert!(!snapshot.movement.crouch.uses_crouched_hull());
        }
        if snapshot.movement.water_level != 0 {
            assert_eq!(
                snapshot.movement.water_type,
                playsrc_movement::CONTENTS_WATER
            );
        }
    }
    assert_eq!(session.movement_state().water_level, 3);
    let fired = session
        .advance(Command {
            movement: held_crouch,
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
    let before = session.movement_state();
    let impacted = session
        .advance_with_external(
            Command {
                movement: held_crouch,
                ..Command::default()
            },
            &[RocketTraceResult {
                projectile: request.projectile,
                tick: session.producer_snapshot().tick,
                end: [
                    before.position[0],
                    before.position[1],
                    before.position[2] - 1.0,
                ],
                solid: true,
                sky: false,
                normal: Some([0.0, 0.0, 1.0]),
                direct_target: None,
            }],
            None,
        )
        .unwrap();
    let impulse = impacted
        .events
        .iter()
        .find_map(|event| match event {
            Event::BlastImpulse { velocity } => Some(velocity[2]),
            _ => None,
        })
        .unwrap();
    assert!((impulse - before.velocity[2] - 900.0).abs() < 0.001);
    assert!((impacted.movement.velocity[2] - impulse * 0.94).abs() < 0.001);
    assert_eq!(impacted.health, 110.0);
    assert!(!impacted.movement.crouch.uses_crouched_hull());

    let jumped = session
        .advance(Command {
            movement: playsrc_movement::Command {
                jump: true,
                crouch: true,
                ..playsrc_movement::Command::default()
            },
            ..Command::default()
        })
        .unwrap();
    assert!(
        (jumped.movement.velocity[2] - 122.8).abs() < 0.001,
        "velocity={}, water_level={}, position={:?}",
        jumped.movement.velocity[2],
        jumped.movement.water_level,
        jumped.movement.position
    );
    assert!(!jumped.movement.crouch.uses_crouched_hull());
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

fn configured_bsp_bytes() -> Vec<u8> {
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
    bytes
}

fn configured_graph() -> (Graph, Vec<ModelBounds>, u64) {
    let bytes = configured_bsp_bytes();
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

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
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
