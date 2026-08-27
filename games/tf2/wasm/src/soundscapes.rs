//! The server selection boundary uses public SDK soundscape and trace contracts.
//! No DSP candidate is instantiated by map construction or snapshot publication.
use std::collections::BTreeMap;

pub fn prepare(
    resources: &BTreeMap<String, &[u8]>,
    graph: &playsrc_entity::Graph,
    map: &mut playsrc_tf2::MapRuntime,
    visibility: &playsrc_visibility::World,
    bsp_hash: [u8; 32],
) -> Result<playsrc_audio::soundscape::ZoneIndex, ()> {
    let has_soundscapes = graph.entities.iter().any(|entity| {
        entity.classname.as_deref().is_some_and(|class| {
            [
                b"env_soundscape".as_slice(),
                b"env_soundscape_proxy",
                b"env_soundscape_triggerable",
            ]
            .iter()
            .any(|name| class.eq_ignore_ascii_case(name))
        })
    });
    if !has_soundscapes {
        return Ok(Default::default());
    }
    let name = playsrc_audio::soundscape::read_map_binding(
        resources
            .get(playsrc_audio::soundscape::MAP_BINDING)
            .ok_or(())?,
        bsp_hash,
    )
    .ok_or(())?;
    let registry = playsrc_audio::soundscape::Registry::load(name, |path| {
        Ok(resources.get(path).map(|bytes| bytes.to_vec()))
    })
    .map_err(|_| ())?;
    map.initialize_soundscapes(&registry);
    playsrc_audio::soundscape::ZoneIndex::compile(visibility, &map.soundscape_zones())
        .map_err(|_| ())
}

pub fn trace(
    world: &super::SharedWorld,
    start: [f32; 3],
    end: [f32; 3],
) -> Result<playsrc_entity::soundscape::Trace, ()> {
    let snapshot = world.snapshot();
    let trace = world
        .world
        .trace_snapshot_hull_with_scratch(
            &snapshot,
            playsrc_collision::SnapshotTraceRequest {
                start,
                end,
                hull: playsrc_collision::Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                // MASK_SOLID_BRUSHONLY | MASK_WATER (public bspflags.h).
                mask: 0x400b | 0x4030,
                scope: playsrc_collision::TraceScope::Everything,
                ignored: &[],
            },
            &mut world
                .movement_queries
                .lock()
                .expect("soundscape trace scratch"),
            |candidate| {
                // StandardFilterRules rejects non-brush entities when MONSTER is
                // absent. Static props are handled by TraceScope::Everything.
                snapshot
                    .records()
                    .iter()
                    .find(|object| object.identity == candidate.identity)
                    .is_some_and(|object| {
                        matches!(
                            object.shape,
                            playsrc_collision::SnapshotShape::BrushModel { .. }
                        )
                    })
            },
        )
        .map_err(|_| ())?;
    Ok(playsrc_entity::soundscape::Trace {
        fraction: trace.fraction,
        start_solid: trace.start_solid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visibility() -> playsrc_visibility::World {
        let zero = playsrc_bsp::Vector3 {
            x: playsrc_bsp::Float32(0),
            y: playsrc_bsp::Float32(0),
            z: playsrc_bsp::Float32(0),
        };
        playsrc_visibility::World {
            identity: [0; 32],
            visibility_mode: playsrc_visibility::VisibilityMode::Compressed,
            cluster_count: 1,
            words_per_row: 1,
            pvs: vec![1],
            pas: vec![1],
            planes: vec![],
            nodes: vec![],
            leaves: vec![playsrc_bsp::Leaf {
                contents: 0,
                cluster: 0,
                area_and_flags: 0,
                mins: [-512; 3],
                maxs: [512; 3],
                first_leaf_face: 0,
                leaf_face_count: 0,
                first_leaf_brush: 0,
                leaf_brush_count: 0,
                leaf_water_data_id: -1,
                padding: 0,
                ambient_cube: None,
            }],
            leaf_faces: vec![],
            models: vec![playsrc_bsp::Model {
                mins: zero,
                maxs: zero,
                origin: zero,
                head_node: -1,
                first_face: 0,
                face_count: 0,
            }],
            areas: vec![],
            portals: vec![],
            portal_vertices: vec![],
            leaf_displacements: vec![vec![]],
        }
    }

    #[test]
    fn map_preparation_binds_authored_positions_without_constructing_dsp() {
        let graph = playsrc_entity::parse(br#"
            {"classname" "worldspawn"}
            {"classname" "env_soundscape" "soundscape" "inside" "radius" "-1" "position7" "speaker" "OnPlay" "relay,Trigger,,0,-1"}
            {"classname" "info_target" "targetname" "speaker" "origin" "10 20 30"}
            {"classname" "logic_relay" "targetname" "relay"}
        "#, Default::default()).unwrap();
        let mut map = playsrc_tf2::MapRuntime::compile(&graph, 0.015, 1, vec![]).unwrap();
        let binding = playsrc_audio::soundscape::encode_map_binding("small", [0; 32]).unwrap();
        let mut resources = BTreeMap::from([
            (
                playsrc_audio::soundscape::MAP_BINDING.into(),
                binding.as_slice(),
            ),
            (
                "scripts/soundscapes_manifest.txt".into(),
                b"soundscapes_manifest { file scripts/zones.txt }".as_slice(),
            ),
            (
                "scripts/zones.txt".into(),
                b"inside { dsp 1 playlooping { volume 1 position 7 wave ambient/room.wav } }"
                    .as_slice(),
            ),
        ]);
        let index = prepare(&resources, &graph, &mut map, &visibility(), [0; 32]).unwrap();
        let mut no_vis = visibility();
        no_vis.visibility_mode = playsrc_visibility::VisibilityMode::NoVis;
        no_vis.pvs.clear();
        assert_eq!(
            prepare(&resources, &graph, &mut map, &no_vis, [0; 32])
                .unwrap()
                .candidates(0),
            [0]
        );
        assert!(prepare(&resources, &graph, &mut map, &visibility(), [1; 32]).is_err());
        assert_eq!(index.candidates(0), [0]);
        let prior = map.clone();
        let phase = map
            .update_soundscape([0.0; 3], index.candidates(0), |_, _| {
                playsrc_entity::soundscape::Trace {
                    fraction: 1.0,
                    start_solid: false,
                }
            })
            .unwrap();
        assert_eq!(map.soundscape_selection().soundscape, 0);
        assert_eq!(map.soundscape_selection().position_bits, 0x80);
        assert_eq!(map.soundscape_selection().positions[7], [10.0, 20.0, 30.0]);
        assert!(phase.events.iter().any(|event| event.name == b"OnPlay"));
        assert_eq!(prior.soundscape_selection(), Default::default());
        resources.remove("scripts/soundscapes_manifest.txt");
        assert!(prepare(&resources, &graph, &mut map, &visibility(), [0; 32]).is_err());
    }
}
