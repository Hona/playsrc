use crate::{Error, ErrorCode, error};
use playsrc_entity::{Entity, Graph};
use playsrc_visibility::{AreaState, World};

pub fn compile_area_portal_state(entities: &Graph, visibility: &World) -> Result<AreaState, Error> {
    let mut state = AreaState::new(visibility);
    let mut updates = Vec::new();

    for entity in &entities.entities {
        let Some(classname) = entity.classname.as_deref() else {
            continue;
        };
        let window = classname.eq_ignore_ascii_case(b"func_areaportalwindow");
        if !window && !classname.eq_ignore_ascii_case(b"func_areaportal") {
            continue;
        }

        let portal = field(entity, b"portalnumber")
            .map(playsrc_entity::source_integer)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|portal| *portal != 0 && state.portal_open(*portal).is_some())
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(entity.index)))?;
        let open = if window {
            true
        } else {
            field(entity, b"StartOpen")
                .map(|value| playsrc_entity::source_integer(value) != 0)
                .unwrap_or(true)
        };
        if let Some((_, current)) = updates.iter_mut().find(|(key, _)| *key == portal) {
            *current = open;
        } else {
            updates.push((portal, open));
        }
    }

    state
        .set_portals(&updates)
        .map_err(|_| error(ErrorCode::InvalidReference, None))?;
    Ok(state)
}

fn field<'a>(entity: &'a Entity, identity: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .rev()
        .find(|pair| pair.key.eq_ignore_ascii_case(identity))
        .map(|pair| pair.value.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_visibility::{Area, AreaPortal, VisibilityMode};

    fn world() -> World {
        World {
            identity: [0; 32],
            visibility_mode: VisibilityMode::NoVis,
            cluster_count: 0,
            words_per_row: 0,
            pvs: Vec::new(),
            pas: Vec::new(),
            planes: Vec::new(),
            nodes: Vec::new(),
            leaves: Vec::new(),
            leaf_faces: Vec::new(),
            models: Vec::new(),
            areas: vec![
                Area {
                    first_portal: 0,
                    portal_count: 0,
                },
                Area {
                    first_portal: 0,
                    portal_count: 2,
                },
                Area {
                    first_portal: 2,
                    portal_count: 2,
                },
            ],
            portals: vec![
                AreaPortal {
                    key: 7,
                    destination_area: 2,
                    first_vertex: 0,
                    vertex_count: 0,
                    plane: 0,
                },
                AreaPortal {
                    key: 9,
                    destination_area: 2,
                    first_vertex: 0,
                    vertex_count: 0,
                    plane: 0,
                },
                AreaPortal {
                    key: 7,
                    destination_area: 1,
                    first_vertex: 0,
                    vertex_count: 0,
                    plane: 0,
                },
                AreaPortal {
                    key: 9,
                    destination_area: 1,
                    first_vertex: 0,
                    vertex_count: 0,
                    plane: 0,
                },
            ],
            portal_vertices: Vec::new(),
            leaf_displacements: Vec::new(),
        }
    }

    fn graph(source: &[u8]) -> Graph {
        playsrc_entity::parse(source, playsrc_entity::Limits::default()).unwrap()
    }

    #[test]
    fn authored_area_portals_default_open_and_preserve_explicit_closed_state() {
        let state = compile_area_portal_state(
            &graph(
                br#"{"classname""func_areaportal""portalnumber""  +7tail"}{"classname""FUNC_AREAPORTAL""PORTALNUMBER""9""startopen""false"}"#,
            ),
            &world(),
        )
        .unwrap();
        assert_eq!(state.portal_open(7), Some(true));
        assert_eq!(state.portal_open(9), Some(false));
        assert_eq!(state.revision, 1);
        assert!(state.connected(&world(), 1, 2).unwrap());
    }

    #[test]
    fn area_portal_windows_open_at_spawn_and_nonzero_start_values_are_open() {
        let state = compile_area_portal_state(
            &graph(
                br#"{"classname""func_areaportalwindow""portalnumber""7""StartOpen""0"}{"classname""func_areaportal""portalnumber""9""StartOpen""  -2tail"}"#,
            ),
            &world(),
        )
        .unwrap();
        assert_eq!(state.portal_open(7), Some(true));
        assert_eq!(state.portal_open(9), Some(true));
    }

    #[test]
    fn area_portals_apply_duplicate_sources_in_authored_spawn_order() {
        let state = compile_area_portal_state(
            &graph(
                br#"{"classname""func_areaportal""portalnumber""7""StartOpen""1"}{"classname""func_areaportal""portalnumber""7""StartOpen""0"}{"classname""func_areaportalwindow""portalnumber""9"}"#,
            ),
            &world(),
        )
        .unwrap();
        assert_eq!(state.portal_open(7), Some(false));
        assert_eq!(state.portal_open(9), Some(true));
    }

    #[test]
    fn area_portals_reject_missing_unknown_and_invalid_authored_keys() {
        for source in [
            br#"{"classname""func_areaportal"}"#.as_slice(),
            br#"{"classname""func_areaportal""portalnumber""0"}"#.as_slice(),
            br#"{"classname""func_areaportal""portalnumber""8"}"#.as_slice(),
            br#"{"classname""func_areaportal""portalnumber""-7"}"#.as_slice(),
            br#"{"classname""func_areaportal""portalnumber""65536"}"#.as_slice(),
        ] {
            assert!(compile_area_portal_state(&graph(source), &world()).is_err());
        }
    }
}
