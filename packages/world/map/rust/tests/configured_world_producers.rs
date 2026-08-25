use playsrc_bsp::{Limits as BspLimits, Profile as BspProfile};
use playsrc_collision::{DisplacementInput, Hull, MASK_PLAYERSOLID, SurfaceIdentity};
use playsrc_entity::{Entity, Limits as EntityLimits};
use playsrc_map::{
    ControllerState, LightingProfile, attach_displacement_visibility, compile_area_portal_state,
    compile_environment_controllers, compile_prepared,
};
use playsrc_visibility::{
    CandidateId, CandidateInput, CandidateKind, CandidateMembership, CandidateSet, SkyVisibility,
    ViewQuery,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf};

const JUMP_BEEF_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const PL_UPWARD_SHA256: &str = "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709";
const CTF_2FORT_SHA256: &str = "cbd191411c0be57099da73458167001ec80d58bf37c71cb3c36b2911b6e80fd7";
const FAILING_JUMP_BEEF_CAMERA: [f32; 3] = [
    f32::from_bits(0xc51c_783d),
    f32::from_bits(0x4622_6b1e),
    f32::from_bits(0xc4d8_0bab),
];
const UPWARD_SKY_ORIGIN: [f32; 3] = [7168.0, -2048.0, 0.0];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalConfig {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[test]
#[ignore = "requires playsrc.local.json and all exact configured TF2 maps"]
fn configured_maps_preserve_controller_independent_sky_and_complete_world_producers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .to_path_buf();
    let configuration: LocalConfig = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).expect("configured local paths"),
    )
    .expect("valid local configuration");
    assert!(PathBuf::from(&configuration.tf2_dir).is_absolute());
    assert!(PathBuf::from(&configuration.source_cache_dir).is_absolute());
    assert!(PathBuf::from(&configuration.asset_dir).is_absolute());

    for (name, expected_sha256, path) in [
        (
            "jump_beef",
            JUMP_BEEF_SHA256,
            PathBuf::from(&configuration.source_cache_dir)
                .join("objects/sha256")
                .join(&JUMP_BEEF_SHA256[..2])
                .join(JUMP_BEEF_SHA256),
        ),
        (
            "pl_upward",
            PL_UPWARD_SHA256,
            PathBuf::from(&configuration.tf2_dir).join("maps/pl_upward.bsp"),
        ),
        (
            "ctf_2fort",
            CTF_2FORT_SHA256,
            PathBuf::from(&configuration.tf2_dir).join("maps/ctf_2fort.bsp"),
        ),
    ] {
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!("exact configured {name} BSP {}: {error}", path.display())
        });
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected_sha256);
        let bsp = playsrc_bsp::parse(&bytes, BspProfile::Source2013V20, BspLimits::default())
            .expect("exact configured BSP");
        let entities = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), EntityLimits::default())
            .expect("decoded entity stream");
        let collision = playsrc_collision::compile(&bsp).expect("brush collision authority");
        let map = compile_prepared(&bsp, LightingProfile::Hdr, &entities, &collision)
            .expect("prepared canonical map authority");
        let visibility = attach_displacement_visibility(
            &map,
            &playsrc_visibility::compile(&bsp).expect("BSP visibility authority"),
        )
        .expect("complete displacement visibility authority");
        let area_state = compile_area_portal_state(&entities, &visibility)
            .expect("authored initial area-portal states");
        let portal_count = match name {
            "jump_beef" => 0,
            "pl_upward" => 59,
            "ctf_2fort" => 29,
            _ => unreachable!("configured map"),
        };
        assert!((1..=portal_count).all(|portal| area_state.portal_open(portal) == Some(true)));
        assert_eq!(area_state.portal_open(0), Some(false));
        let authored_sky_cameras = entities
            .entities
            .iter()
            .filter(|entity| entity.classname.as_deref() == Some(b"sky_camera"))
            .collect::<Vec<_>>();
        let controllers = compile_environment_controllers(&entities, &visibility)
            .expect("source-ordered typed environment controllers");
        let sky_controllers = controllers
            .iter()
            .filter(|controller| matches!(controller.state, ControllerState::SkyCamera { .. }))
            .collect::<Vec<_>>();
        let flags = visibility
            .leaves
            .iter()
            .fold([0usize; 3], |mut counts, leaf| {
                let value = leaf.area_and_flags >> 9;
                counts[0] += usize::from(value & 0x01 != 0);
                counts[1] += usize::from(value & 0x04 != 0);
                counts[2] += usize::from(leaf.cluster < 0);
                counts
            });
        eprintln!(
            "{name}: revision={} entityBytes={} entities={} brushes={} models={} leaves={} clusters={} areas={} sky3dLeaves={} sky2dLeaves={} outsideLeaves={} staticProps={} displacementPatches={}",
            bsp.map_revision,
            bsp.lumps[0].bytes(&bsp).len(),
            entities.entities.len(),
            collision.brushes.len(),
            collision.models.len(),
            visibility.leaves.len(),
            visibility.cluster_count,
            visibility.areas.len(),
            flags[0],
            flags[1],
            flags[2],
            map.static_props.occurrences.len(),
            map.collision_displacements.len()
        );

        if name == "jump_beef" {
            assert_eq!(bytes.len(), 33_379_388);
            assert_eq!(bsp.map_revision, 731);
            assert_eq!(bsp.lumps[0].bytes(&bsp).len(), 81_425);
            assert_eq!(entities.entities.len(), 361);
            assert_eq!(collision.brushes.len(), 476);
            assert_eq!(collision.models.len(), 123);
            assert_eq!(visibility.leaves.len(), 1899);
            assert_eq!(visibility.cluster_count, 284);
            assert_eq!(visibility.areas.len(), 8);
            assert_eq!(flags, [893, 0, 1614]);
            assert!(
                authored_sky_cameras.is_empty() && sky_controllers.is_empty(),
                "jump_beef authors no sky controller"
            );
            assert_eq!(
                controllers
                    .iter()
                    .map(|controller| controller.entity)
                    .collect::<Vec<_>>(),
                [170, 360]
            );
            assert_eq!(map.static_props.source_version, 6);
            assert!(map.static_props.occurrences.is_empty());
            assert!(map.collision_displacements.is_empty());
            assert!(
                flags[0] > 0,
                "authored sky leaf flags require no sky controller"
            );
            let candidates = CandidateSet::compile(&visibility, 0, &[]).unwrap();
            let view = visibility
                .view(
                    &area_state,
                    &candidates,
                    &ViewQuery {
                        origins: vec![FAILING_JUMP_BEEF_CAMERA],
                        bypass_pvs: false,
                    },
                )
                .unwrap();
            eprintln!(
                "jump_beef failing camera: leaf={} cluster={} area={} sky={:?} outside={}",
                view.origin_leaves[0],
                view.origin_clusters[0],
                visibility.leaves[view.origin_leaves[0]].area_and_flags & 0x01ff,
                view.sky,
                view.outside_world
            );
            assert!(view.outside_world);
            assert_eq!(view.sky, SkyVisibility::NotVisible);
            let mut unrelated_entities = entities.clone();
            unrelated_entities.source[0] ^= 1;
            assert!(
                matches!(
                    compile_prepared(&bsp, LightingProfile::Hdr, &unrelated_entities, &collision),
                    Err(playsrc_map::Error {
                        code: playsrc_map::ErrorCode::InvalidReference,
                        ..
                    })
                ),
                "prepared map authority must reject an Entity graph from different source bytes"
            );
            let (flagged_leaf, flagged_origin) = visibility
                .leaves
                .iter()
                .enumerate()
                .filter(|(_, leaf)| leaf.cluster >= 0 && leaf.area_and_flags >> 9 & 0x01 != 0)
                .find_map(|(index, leaf)| {
                    let center = std::array::from_fn(|axis| {
                        (f32::from(leaf.mins[axis]) + f32::from(leaf.maxs[axis])) * 0.5
                    });
                    (visibility.locate_leaf(center).ok() == Some(index)).then_some((index, center))
                })
                .expect("reachable 3D-sky-flagged leaf on a map without sky_camera");
            let flagged_view = visibility
                .view(
                    &area_state,
                    &candidates,
                    &ViewQuery {
                        origins: vec![flagged_origin],
                        bypass_pvs: false,
                    },
                )
                .unwrap();
            eprintln!(
                "jump_beef controllerless 3D-sky leaf={flagged_leaf} origin={flagged_origin:?} sky={:?}",
                flagged_view.sky
            );
            assert!(!flagged_view.outside_world);
            assert_eq!(flagged_view.sky, SkyVisibility::Sky3d);
            continue;
        }

        if name == "ctf_2fort" {
            assert_eq!(bytes.len(), 22_751_863);
            assert_eq!(bsp.map_revision, 4067);
            assert_eq!(bsp.lumps[0].bytes(&bsp).len(), 314_566);
            assert_eq!(entities.entities.len(), 1097);
            assert_eq!(collision.brushes.len(), 5676);
            assert_eq!(collision.models.len(), 148);
            assert_eq!(visibility.leaves.len(), 7971);
            assert_eq!(visibility.cluster_count, 2489);
            assert_eq!(visibility.areas.len(), 20);
            assert_eq!(flags, [4107, 0, 5248]);
            assert_eq!(map.static_props.occurrences.len(), 2265);
            assert_eq!(map.collision_displacements.len(), 232);
            assert!(!authored_sky_cameras.is_empty());
            assert!(!sky_controllers.is_empty());
            assert!(!map.lighting.surfaces.is_empty());
            continue;
        }

        assert_eq!(bytes.len(), 25_446_018);
        assert_eq!(bsp.map_revision, 173);
        assert_eq!(bsp.lumps[0].encoded_bytes(&bsp).len(), 17_434);
        assert_eq!(bsp.lumps[0].bytes(&bsp).len(), 240_747);
        assert_eq!(entities.entities.len(), 1004);
        assert_eq!(collision.brushes.len(), 5085);
        assert_eq!(collision.models.len(), 70);
        assert_eq!(visibility.leaves.len(), 3891);
        assert_eq!(visibility.cluster_count, 1427);
        assert_eq!(visibility.areas.len(), 34);
        assert_eq!(flags, [2135, 0, 2019]);
        assert_eq!(authored_sky_cameras.len(), 1);
        assert_eq!(authored_sky_cameras[0].index, 621);
        assert_eq!(field(authored_sky_cameras[0], b"origin"), b"7168 -2048 0");
        assert_eq!(field(authored_sky_cameras[0], b"scale"), b"16");
        assert_eq!(
            controllers
                .iter()
                .map(|controller| controller.entity)
                .collect::<Vec<_>>(),
            [1, 2, 3, 621, 858, 859]
        );
        assert_eq!(sky_controllers.len(), 1);
        match &sky_controllers[0].state {
            ControllerState::SkyCamera {
                origin,
                scale,
                area,
                fog,
            } => {
                assert_eq!(*origin, UPWARD_SKY_ORIGIN);
                assert_eq!(*scale, 16);
                assert_eq!(*area, 1);
                assert!(fog.enabled);
                assert!(!fog.blend);
                assert_eq!(fog.direction, [1.0, 0.0, 0.0]);
                assert_eq!(fog.primary, [174, 193, 205, 255]);
                assert_eq!(fog.secondary, [255, 255, 255, 255]);
                assert_eq!(fog.start, 100.0);
                assert_eq!(fog.end, 20_000.0);
                assert_eq!(fog.maximum_density, 1.0);
                assert!(!fog.radial);
                assert_eq!(fog.far_z, None);
            }
            _ => unreachable!("selected typed sky controller"),
        }
        assert_eq!(map.static_props.source_version, 10);
        assert_eq!(map.static_props.models.len(), 234);
        assert_eq!(map.static_props.leaf_reference_count, 2756);
        assert_eq!(map.static_props.occurrences.len(), 1244);
        assert_eq!(map.collision_displacements.len(), 558);
        assert_eq!(
            collision
                .displacements
                .iter()
                .fold([0usize; 2], |mut counts, patch| {
                    match patch.power {
                        2 => counts[0] += 1,
                        3 => counts[1] += 1,
                        _ => panic!("unexpected configured displacement power"),
                    }
                    counts
                }),
            [554, 4]
        );
        assert_eq!(
            map.collision_displacements
                .iter()
                .map(|patch| patch.positions.len())
                .sum::<usize>(),
            14_174
        );
        assert_eq!(
            map.collision_displacements
                .iter()
                .map(|patch| patch.triangles.len())
                .sum::<usize>(),
            18_240
        );
        assert_eq!(
            map.static_props
                .occurrences
                .iter()
                .filter(|prop| prop.solidity == 6)
                .count(),
            554
        );
        let runtime_lit_props = map
            .static_props
            .occurrences
            .iter()
            .filter(|prop| prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING != 0)
            .map(|prop| prop.source)
            .collect::<Vec<_>>();
        assert_eq!(runtime_lit_props, [650, 882, 888, 1105]);
        let authored_vhv_paths = bsp
            .lump(40)
            .and_then(|lump| lump.pak.as_ref())
            .expect("indexed embedded BSP PAK")
            .entries
            .iter()
            .filter_map(|entry| {
                let path = std::str::from_utf8(&entry.raw_name)
                    .expect("UTF-8 embedded Source asset identity")
                    .to_ascii_lowercase();
                path.ends_with(".vhv").then_some(path)
            })
            .collect::<BTreeSet<_>>();
        let expected_vhv_paths = map
            .static_props
            .occurrences
            .iter()
            .filter(|prop| prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING == 0)
            .flat_map(|prop| {
                [
                    format!("sp_{}.vhv", prop.source),
                    format!("sp_hdr_{}.vhv", prop.source),
                ]
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(authored_vhv_paths.len(), 2480);
        assert_eq!(authored_vhv_paths, expected_vhv_paths);

        let sky_leaf = visibility.locate_leaf(UPWARD_SKY_ORIGIN).unwrap();
        let sky_area = visibility.leaves[sky_leaf].area_and_flags & 0x01ff;
        let mut main_props = 0;
        let mut sky_props = 0;
        let candidate_inputs = map
            .static_props
            .occurrences
            .iter()
            .map(|prop| {
                let areas = prop
                    .leaves
                    .iter()
                    .map(|leaf| {
                        visibility
                            .leaves
                            .get(usize::from(*leaf))
                            .expect("static prop leaf belongs to this BSP")
                            .area_and_flags
                            & 0x01ff
                    })
                    .collect::<BTreeSet<_>>();
                assert!(
                    !areas.is_empty(),
                    "prop {} has no compiled leaves",
                    prop.source
                );
                if areas.contains(&sky_area) {
                    assert_eq!(
                        areas,
                        BTreeSet::from([sky_area]),
                        "prop {} crosses main and sky areas",
                        prop.source
                    );
                    sky_props += 1;
                } else {
                    main_props += 1;
                }
                CandidateInput {
                    id: CandidateId {
                        kind: CandidateKind::StaticProp,
                        index: prop.source as u32,
                    },
                    membership: CandidateMembership::CompiledLeaves(
                        prop.leaves.iter().map(|leaf| usize::from(*leaf)).collect(),
                    ),
                    bounds: None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!([main_props, sky_props], [1184, 60]);
        let candidates = CandidateSet::compile(&visibility, 0, &candidate_inputs).unwrap();
        let sky_view = visibility
            .view(
                &area_state,
                &candidates,
                &ViewQuery {
                    origins: vec![UPWARD_SKY_ORIGIN],
                    bypass_pvs: false,
                },
            )
            .unwrap();
        let sky_drawable_surfaces = sky_view
            .world_surfaces
            .iter()
            .filter(|face| {
                let surface = &map.surfaces[usize::from(**face)];
                surface.draw && surface.model == 0
            })
            .count();
        eprintln!(
            "pl_upward sky: leaf={sky_leaf} cluster={} area={sky_area} visibleAreas={:?} surfaces={} drawableSurfaces={sky_drawable_surfaces} staticProps={}",
            visibility.leaves[sky_leaf].cluster,
            sky_view.visible_areas,
            sky_view.world_surfaces.len(),
            sky_view.candidates.len()
        );
        assert_eq!(sky_area, 1);
        assert_eq!(sky_view.visible_areas, [sky_area as usize]);
        assert_eq!(sky_view.candidates.len(), 60);
        assert_eq!(sky_drawable_surfaces, 752);
        let spawn_door_view = visibility
            .view(
                &area_state,
                &candidates,
                &ViewQuery {
                    origins: vec![[-1850.0, -1536.0, 132.0]],
                    bypass_pvs: false,
                },
            )
            .unwrap();
        assert!(spawn_door_view.visible_areas.contains(&2));
        assert!(spawn_door_view.visible_areas.contains(&30));
        assert!(spawn_door_view.world_surfaces.contains(&14_755));
        assert_eq!(
            map.surfaces
                .iter()
                .filter(|surface| surface.flags & 0x0006 != 0)
                .count(),
            198,
            "authored sky-mask surfaces are distinct from drawable 3D-sky-area world geometry"
        );

        let collision = collision
            .with_displacement_inputs(
                map.collision_displacements
                    .iter()
                    .map(|patch| DisplacementInput {
                        source: patch.source,
                        parent_face: patch.parent_face,
                        contents: patch.contents,
                        positions: patch.positions.clone(),
                        triangles: patch.triangles.clone(),
                        triangle_tags: patch.triangle_tags.clone(),
                        primary_surface: SurfaceIdentity {
                            registry: [0; 32],
                            index: patch.material as u32,
                        },
                        secondary_surface: None,
                        use_secondary_surface: vec![false; patch.triangles.len()],
                    })
                    .collect(),
            )
            .expect("complete exact full-grid terrain collision producer");
        for source in [147, 381, 138] {
            let patch = &map.collision_displacements[source];
            let [first, second, third] =
                patch.triangles[0].map(|index| patch.positions[index as usize]);
            let center: [f32; 3] =
                std::array::from_fn(|axis| (first[axis] + second[axis] + third[axis]) / 3.0);
            let face = &map.surfaces[patch.parent_face];
            let normal: [f32; 3] = std::array::from_fn(|axis| {
                if face.plane_back {
                    -face.plane[axis]
                } else {
                    face.plane[axis]
                }
            });
            let start = std::array::from_fn(|axis| center[axis] + normal[axis] * 2.0);
            let end = std::array::from_fn(|axis| center[axis] - normal[axis] * 2.0);
            let trace = collision
                .trace_hull(
                    start,
                    end,
                    Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    MASK_PLAYERSOLID,
                )
                .unwrap();
            eprintln!(
                "pl_upward terrain source={source} fraction={} brush={:?} displacement={:?}",
                trace.fraction, trace.brush, trace.displacement
            );
            assert_eq!(trace.displacement.map(|hit| hit.source), Some(source));
            assert_eq!(
                trace.displacement.map(|hit| hit.parent_face),
                Some(patch.parent_face)
            );
        }
    }
}

fn field<'a>(entity: &'a Entity, key: &[u8]) -> &'a [u8] {
    &entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .expect("authored entity field")
        .value
}
