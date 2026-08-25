use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalConfig {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[test]
#[ignore = "requires playsrc.local.json and configured TF2 build 24245096"]
fn configured_pl_upward_displacements_are_complete_and_deterministic() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("repository root")
        .to_path_buf();
    let config: LocalConfig = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).expect("configured local paths"),
    )
    .expect("valid local configuration");
    assert!(!config.source_cache_dir.is_empty() && !config.asset_dir.is_empty());
    let bytes = fs::read(PathBuf::from(config.tf2_dir).join("maps/pl_upward.bsp"))
        .expect("exact configured maps/pl_upward.bsp");
    assert_eq!(bytes.len(), 25_446_018);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709"
    );
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .expect("configured BSP");
    let first =
        playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Hdr).expect("configured HDR map");
    let second = playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Hdr)
        .expect("repeated configured HDR map");
    assert_eq!(first, second);
    assert_eq!(first.static_props.source_version, 10);
    assert_eq!(first.static_props.models.len(), 234);
    assert_eq!(first.static_props.leaf_reference_count, 2_756);
    assert_eq!(first.static_props.occurrences.len(), 1_244);
    assert_eq!(first.collision_displacements.len(), 558);
    assert_eq!(
        first
            .collision_displacements
            .iter()
            .map(|patch| patch.positions.len())
            .sum::<usize>(),
        14_174
    );
    assert_eq!(
        first
            .collision_displacements
            .iter()
            .map(|patch| patch.triangles.len())
            .sum::<usize>(),
        18_240
    );
    assert_eq!(
        first
            .surfaces
            .iter()
            .filter(|surface| surface.displacement.is_some())
            .count(),
        558
    );
    assert_eq!(
        first
            .surfaces
            .iter()
            .filter(|surface| surface.draw && surface.model == 0)
            .count(),
        15_072
    );
    assert_eq!(first.vertex_count, 96_880);
    let collision = playsrc_collision::compile(&bsp).expect("configured collision producer");
    assert_eq!(collision.displacements.len(), 558);
    assert_eq!(
        collision
            .displacements
            .iter()
            .map(|patch| patch.vertices.len())
            .sum::<usize>(),
        14_174
    );
    assert_eq!(
        collision
            .displacements
            .iter()
            .map(|patch| patch.triangle_tags.len())
            .sum::<usize>(),
        18_240
    );
    let probes = first
        .surfaces
        .iter()
        .filter(|surface| surface.model == 0 && surface.draw && surface.displacement.is_some())
        .map(|surface| {
            let bounds = [0, 1, 2].map(|axis| {
                surface
                    .positions
                    .iter()
                    .map(|position| position[axis])
                    .fold(
                        [f32::INFINITY, f32::NEG_INFINITY],
                        |[minimum, maximum], value| [minimum.min(value), maximum.max(value)],
                    )
            });
            let expected = [surface.plane[0], surface.plane[1], surface.plane[2]]
                .map(|value| if surface.plane_back { -value } else { value });
            let dots = surface
                .triangles
                .iter()
                .map(|triangle| {
                    let [a, b, c] = triangle.map(|index| surface.positions[index as usize]);
                    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let normal = [
                        ab[1] * ac[2] - ab[2] * ac[1],
                        ab[2] * ac[0] - ab[0] * ac[2],
                        ab[0] * ac[1] - ab[1] * ac[0],
                    ];
                    normal[0] * expected[0] + normal[1] * expected[1] + normal[2] * expected[2]
                })
                .collect::<Vec<_>>();
            (
                surface.displacement.as_ref().unwrap().source,
                surface.face,
                surface.material,
                expected,
                bounds,
                dots.iter().filter(|dot| **dot > 0.0).count(),
                dots.iter().filter(|dot| **dot < 0.0).count(),
                surface.triangles.len(),
            )
        })
        .collect::<Vec<_>>();
    for (source, face) in [(147, 14_859), (381, 15_093), (138, 14_850)] {
        let probe = probes.iter().find(|probe| probe.0 == source).unwrap();
        assert_eq!(probe.1, face);
        assert_eq!((probe.5, probe.6, probe.7), (32, 0, 32));
    }
    let base_visibility = playsrc_visibility::compile(&bsp).unwrap();
    let visibility = playsrc_map::attach_displacement_visibility(&first, &base_visibility).unwrap();
    let surface_lighting = playsrc_map::SurfaceLightingWorld::compile(
        &first,
        &visibility,
        std::collections::BTreeSet::new(),
    )
    .unwrap();
    for source in [650, 882, 888, 1105] {
        let origin = first.static_props.occurrences[source]
            .lighting_origin
            .expect("runtime-lit static prop lighting origin");
        for direction in [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]] {
            let end = std::array::from_fn(|axis| {
                origin[axis] + direction[axis] * playsrc_map::SOURCE_AMBIENT_RAY_LENGTH
            });
            assert_eq!(
                surface_lighting.trace(origin, end).unwrap(),
                surface_lighting.trace(origin, end).unwrap()
            );
        }
    }
    let entities =
        playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
            .expect("configured map entities");
    let state = playsrc_map::compile_area_portal_state(&entities, &visibility)
        .expect("configured authored area portals");
    assert_eq!(
        state.portal_open(1),
        Some(true),
        "Upward's authored-open spawn area portal must connect its outdoor terrain"
    );
    assert!((1..=59).all(|portal| state.portal_open(portal) == Some(true)));
    assert!(state.connected(&visibility, 33, 2).unwrap());
    assert!(!state.connected(&visibility, 1, 2).unwrap());
    let candidates = playsrc_visibility::CandidateSet::compile(&visibility, 0, &[]).unwrap();
    for (source, face) in [(147, 14_859), (381, 15_093), (138, 14_850)] {
        let probe = probes.iter().find(|probe| probe.0 == source).unwrap();
        let center = probe.4.map(|[minimum, maximum]| (minimum + maximum) * 0.5);
        let span = probe
            .4
            .iter()
            .map(|[minimum, maximum]| maximum - minimum)
            .fold(0.0f32, f32::max);
        let position =
            std::array::from_fn(|axis| center[axis] + probe.3[axis] * (span * 0.25 + 64.0));
        let memberships = visibility
            .leaf_displacements
            .iter()
            .enumerate()
            .filter_map(|(leaf, faces)| {
                faces
                    .contains(&face)
                    .then_some((leaf, visibility.leaves[leaf].cluster))
            })
            .collect::<Vec<_>>();
        let view = visibility
            .view(
                &state,
                &candidates,
                &playsrc_visibility::ViewQuery {
                    origins: vec![position],
                    bypass_pvs: false,
                },
            )
            .unwrap();
        assert!(
            !memberships.is_empty(),
            "source {source} has no visibility leaves"
        );
        assert!(
            view.world_surfaces.contains(&face),
            "source {source} is absent from its above/front PVS"
        );
    }
}
