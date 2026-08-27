use playsrc_collision::{Hull, World};
use playsrc_entity::{ModelBounds, Transform};
use playsrc_movement::{Error, Trace, Tracer};
use playsrc_tf2::{
    ActorContact, GameplayWorld, MapRuntime, MoverResult, MoverResultKind, PlayerContactFacts,
    PresentationRevision, Session,
};
use std::{fs, sync::Arc};

#[derive(Clone)]
struct ContactWorld(Arc<World>);
impl Tracer for ContactWorld {
    fn trace(&self, _: [f32; 3], _: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
        panic!("this contact/publication test does not replace the movement solver")
    }
}
impl GameplayWorld for ContactWorld {
    fn collision_snapshot_revision(&self) -> Option<u64> {
        Some(1)
    }
    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, Error> {
        Ok(self
            .0
            .overlaps_model_hull(model, origin, position, hull)
            .unwrap())
    }
}

#[test]
#[ignore = "requires PLAYSRC_TEST_BSP pointing to an exact configured BSP"]
fn configured_authored_player_bot_contacts_and_button_outputs_publish_mover_children() {
    let bytes = fs::read(std::env::var("PLAYSRC_TEST_BSP").unwrap()).unwrap();
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    let graph =
        playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default()).unwrap();
    let playsrc_bsp::LumpData::Models(models) = &bsp.lumps[14].records else {
        panic!("model lump")
    };
    let bounds: Vec<_> = models
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
    let world = ContactWorld(Arc::new(playsrc_collision::compile(&bsp).unwrap()));
    let named = |name: &[u8]| {
        graph
            .entities
            .iter()
            .find(|entity| entity.targetname.as_deref() == Some(name))
    };
    let (door, position, button) = if let Some(door) = named(b"door_red1_2_up") {
        (door, [1020.0, 1440.0, 256.0], None)
    } else if let Some(door) = named(b"door_red_exitA1") {
        (door, [720.0, 544.0, 592.0], None)
    } else {
        (
            named(b"door1").unwrap(),
            [4608.0, 3184.0, -3104.0],
            Some(named(b"button1").unwrap().index as u32),
        )
    };
    let hull = Hull {
        mins: [-24.0, -24.0, 0.0],
        maxs: [24.0, 24.0, 82.0],
    };
    for &bot in if button.is_some() {
        &[false][..]
    } else {
        &[false, true][..]
    } {
        let mut map = MapRuntime::compile(&graph, 0.015, 1, bounds.clone()).unwrap();
        let facts = PlayerContactFacts {
            team: 2,
            class: 3,
            ..PlayerContactFacts::default()
        };
        let snapshot = |map: &MapRuntime| {
            let session = Session::new(world.clone(), position, map.clone());
            session
                .entity_presentation(PresentationRevision {
                    entity: session.entity_revision(),
                    collision: 1,
                })
                .unwrap()
        };
        let closed = snapshot(&map);
        let phase = if let Some(button) = button {
            // jump_beef authors damage buttons, not an approach trigger. Do not
            // invent proximity opening for these doors.
            map.damage(1, button).unwrap()
        } else {
            let actor = ActorContact {
                identity: 1000,
                position,
                hull,
                facts,
                alive: true,
            };
            map.contact_phase(
                &world,
                1,
                if bot { [0.0, 0.0, 8192.0] } else { position },
                hull,
                facts,
                if bot {
                    std::slice::from_ref(&actor)
                } else {
                    &[]
                },
            )
            .unwrap()
        };
        let request = *phase
            .mover_requests
            .iter()
            .find(|request| request.entity == door.index as u32)
            .expect("authored contact/button opens door");
        let parent = closed
            .entities
            .models
            .iter()
            .find(|model| model.source_index == door.index)
            .unwrap();
        let children: Vec<_> = graph
            .entities
            .iter()
            .filter(|entity| {
                entity.pairs.iter().any(|pair| {
                    pair.key.eq_ignore_ascii_case(b"parentname")
                        && Some(pair.value.as_slice()) == door.targetname.as_deref()
                })
            })
            .map(|entity| entity.index)
            .collect();
        for (tick, progress) in [(2, 0.5), (3, 1.0)] {
            let origin = std::array::from_fn(|axis| {
                request.start[axis] + progress * (request.destination[axis] - request.start[axis])
            });
            map.apply_mover_results(
                tick,
                &[MoverResult {
                    request_id: request.request_id,
                    entity: request.entity,
                    kind: if progress == 1.0 {
                        MoverResultKind::Completed
                    } else {
                        MoverResultKind::Progress
                    },
                    transform: Transform {
                        origin,
                        angles: request.start_angles,
                    },
                    carry: [0.0; 3],
                }],
            )
            .unwrap();
            let current = snapshot(&map);
            assert_eq!(
                current
                    .entities
                    .models
                    .iter()
                    .find(|model| model.source_index == door.index)
                    .unwrap()
                    .world_transform
                    .origin,
                origin
            );
            for child in &children {
                let before = closed
                    .studio_models
                    .iter()
                    .find(|model| model.source_index == *child)
                    .unwrap();
                let after = current
                    .studio_models
                    .iter()
                    .find(|model| model.source_index == *child)
                    .unwrap();
                for axis in 0..3 {
                    assert!(
                        (after.world_transform.origin[axis]
                            - before.world_transform.origin[axis]
                            - (origin[axis] - parent.world_transform.origin[axis]))
                            .abs()
                            < 0.001
                    );
                }
            }
        }
        eprintln!(
            "configured door={} bot={} children={} open={:?}",
            door.index,
            bot,
            children.len(),
            request.destination
        );
    }
}
