use std::collections::{BTreeMap, BTreeSet, VecDeque};

const SOURCE_NEAREST_AREA_DISTANCE: f32 = 10_000.0;

fn entity_value<'a>(entity: &'a playsrc_entity::Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}

fn entity_origin(entity: &playsrc_entity::Entity) -> Result<[f32; 3], String> {
    let text = std::str::from_utf8(
        entity_value(entity, b"origin").ok_or("navigation entity has no origin")?,
    )
    .map_err(|_| "navigation entity origin is not UTF-8")?;
    let values: Vec<f32> = text
        .split_whitespace()
        .map(|part| part.parse::<f32>())
        .collect::<Result<_, _>>()
        .map_err(|_| "navigation entity origin is malformed")?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        return Err("navigation entity origin is malformed".to_owned());
    }
    Ok([values[0], values[1], values[2]])
}

fn nearest(
    mesh: &playsrc_nav::Mesh,
    indexes: &BTreeMap<u32, usize>,
    position: [f32; 3],
) -> Result<usize, String> {
    let area = mesh.nearest_area(position).ok_or("NAV mesh has no areas")?;
    let point = area.closest_point(position);
    let squared: f32 = (0..3)
        .map(|axis| (position[axis] - point[axis]).powi(2))
        .sum();
    if squared > SOURCE_NEAREST_AREA_DISTANCE.powi(2) {
        return Err(format!(
            "navigation entity at {position:?} has no Source-reachable NAV area"
        ));
    }
    Ok(indexes[&area.identity])
}

pub fn authenticate(
    bytes: &[u8],
    bsp_length: usize,
    bounds: [[f32; 3]; 2],
    entities: &playsrc_entity::Graph,
    target: &str,
) -> Result<(), String> {
    let bsp_size =
        u32::try_from(bsp_length).map_err(|_| "authenticated BSP exceeds Source NAV size")?;
    let mesh = playsrc_nav::parse(
        bytes,
        playsrc_nav::Profile::TeamFortress2,
        Some(bsp_size),
        playsrc_nav::Limits::default(),
    )
    .map_err(|error| format!("invalid Source NAV: {error}"))?;
    if mesh.version != playsrc_nav::CURRENT_VERSION
        || mesh.subversion != playsrc_nav::TF2_SUBVERSION
    {
        return Err("NAV format does not match the current TF2 Source generator".to_owned());
    }
    if !mesh.analyzed {
        return Err("NAV mesh has not completed Source analysis".to_owned());
    }
    let indexes: BTreeMap<u32, usize> = mesh
        .areas
        .iter()
        .enumerate()
        .map(|(index, area)| (area.identity, index))
        .collect();
    let mut topology = vec![Vec::new(); mesh.areas.len()];
    for (index, area) in mesh.areas.iter().enumerate() {
        let minimum = [
            area.northwest[0],
            area.northwest[1],
            area.northwest[2]
                .min(area.southeast[2])
                .min(area.northeast_z)
                .min(area.southwest_z),
        ];
        let maximum = [
            area.southeast[0],
            area.southeast[1],
            area.northwest[2]
                .max(area.southeast[2])
                .max(area.northeast_z)
                .max(area.southwest_z),
        ];
        if (0..3).any(|axis| minimum[axis] < bounds[0][axis] || maximum[axis] > bounds[1][axis]) {
            return Err("NAV area extends outside authenticated BSP world bounds".to_owned());
        }
        for connection in area.connections.iter().flatten() {
            let linked = indexes[connection];
            topology[index].push(linked);
            topology[linked].push(index);
        }
    }
    for ladder in &mesh.ladders {
        let links: Vec<usize> = [
            ladder.top_forward,
            ladder.top_left,
            ladder.top_right,
            ladder.top_behind,
            ladder.bottom_area,
        ]
        .into_iter()
        .flatten()
        .map(|identity| indexes[&identity])
        .collect();
        for pair in links.windows(2) {
            topology[pair[0]].push(pair[1]);
            topology[pair[1]].push(pair[0]);
        }
    }
    let mut red = Vec::new();
    let mut blue = Vec::new();
    let mut objectives = Vec::new();
    let mut payload_route = BTreeSet::new();
    if target == "pl_upward" {
        let watcher = entities
            .entities
            .iter()
            .find(|entity| {
                entity
                    .classname
                    .as_deref()
                    .is_some_and(|classname| classname.eq_ignore_ascii_case(b"team_train_watcher"))
            })
            .ok_or("payload watcher is absent from authenticated BSP")?;
        let goal = entity_value(watcher, b"goal_node").ok_or("payload goal node is absent")?;
        let mut current =
            entity_value(watcher, b"start_node").ok_or("payload start node is absent")?;
        loop {
            if !payload_route.insert(current.to_vec()) {
                return Err("payload path contains a cycle before its goal".to_owned());
            }
            let track = entities
                .entities
                .iter()
                .find(|entity| {
                    entity
                        .classname
                        .as_deref()
                        .is_some_and(|classname| classname.eq_ignore_ascii_case(b"path_track"))
                        && entity_value(entity, b"targetname")
                            .is_some_and(|name| name.eq_ignore_ascii_case(current))
                })
                .ok_or("payload path references an absent track")?;
            if current.eq_ignore_ascii_case(goal) {
                break;
            }
            current = entity_value(track, b"target").ok_or("payload path ends before its goal")?;
        }
    }
    for entity in &entities.entities {
        let Some(classname) = entity.classname.as_deref() else {
            continue;
        };
        if classname.eq_ignore_ascii_case(b"info_player_teamspawn") {
            let team = entity_value(entity, b"TeamNum").unwrap_or_default();
            if team != b"2" && team != b"3" {
                continue;
            }
            let area = nearest(&mesh, &indexes, entity_origin(entity)?)?;
            if team == b"2" {
                red.push(area);
            }
            if team == b"3" {
                blue.push(area);
            }
        } else if (target == "ctf_2fort" && classname.eq_ignore_ascii_case(b"item_teamflag"))
            || (target == "pl_upward"
                && (classname.eq_ignore_ascii_case(b"team_control_point")
                    || (classname.eq_ignore_ascii_case(b"path_track")
                        && entity_value(entity, b"targetname")
                            .is_some_and(|name| payload_route.contains(name)))))
        {
            objectives.push(nearest(&mesh, &indexes, entity_origin(entity)?)?);
        }
    }
    if red.is_empty() || blue.is_empty() || objectives.is_empty() {
        return Err("NAV mesh lacks both team spawns or authenticated map objectives".to_owned());
    }
    let mut reachable = vec![false; mesh.areas.len()];
    let mut pending: VecDeque<usize> = blue.into_iter().collect();
    while let Some(index) = pending.pop_front() {
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        pending.extend(
            topology[index]
                .iter()
                .copied()
                .filter(|next| !reachable[*next]),
        );
    }
    if !red.into_iter().any(|index| reachable[index])
        || objectives.into_iter().any(|index| !reachable[index])
    {
        return Err(
            "NAV topology does not connect both teams to the complete map objective route"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(attributes: u32, connected: bool) -> (Vec<u8>, playsrc_entity::Graph) {
        let mut bytes = Vec::new();
        for value in [
            playsrc_nav::MAGIC,
            playsrc_nav::CURRENT_VERSION,
            playsrc_nav::TF2_SUBVERSION,
            128,
        ] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.push(1);
        bytes.extend(0_u16.to_le_bytes());
        bytes.push(1);
        bytes.extend(2_u32.to_le_bytes());
        for index in 0..2_u32 {
            bytes.extend((index + 1).to_le_bytes());
            bytes.extend(attributes.to_le_bytes());
            let minimum = [index as f32 * 50.0, 0.0, 0.0];
            let maximum = [(index + 1) as f32 * 50.0, 50.0, 0.0];
            for value in minimum.into_iter().chain(maximum) {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend([0; 8]);
            for direction in 0..4 {
                let linked =
                    connected && ((index == 0 && direction == 1) || (index == 1 && direction == 3));
                bytes.extend(u32::from(linked).to_le_bytes());
                if linked {
                    bytes.extend((2 - index).to_le_bytes());
                }
            }
            bytes.push(0);
            bytes.extend([0; 4]);
            bytes.extend([0; 2]);
            bytes.extend([0; 8]);
            bytes.extend([0; 8]);
            bytes.extend([0; 16]);
            bytes.extend([0; 8]);
            bytes.extend(attributes.to_le_bytes());
        }
        bytes.extend([0; 4]);
        let entities = playsrc_entity::parse(
            br#"{
"classname" "info_player_teamspawn"
"TeamNum" "2"
"origin" "25 25 0"
}
{
"classname" "info_player_teamspawn"
"TeamNum" "3"
"origin" "75 25 0"
}
{
"classname" "item_teamflag"
"origin" "25 25 0"
}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        (bytes, entities)
    }

    #[test]
    fn accepts_distinct_source_mesh_identities_for_the_same_authenticated_bsp() {
        for attributes in [0, playsrc_nav::NAV_MESH_CROUCH] {
            let (bytes, entities) = fixture(attributes, true);
            authenticate(
                &bytes,
                128,
                [[0.0, 0.0, -1.0], [100.0, 50.0, 1.0]],
                &entities,
                "ctf_2fort",
            )
            .unwrap();
        }
    }

    #[test]
    fn rejects_wrong_bsp_bounds_disconnected_objectives_and_unauthenticated_trailing_data() {
        let (mut bytes, entities) = fixture(0, true);
        let bounds = [[0.0, 0.0, -1.0], [100.0, 50.0, 1.0]];
        assert!(
            authenticate(&bytes, 129, bounds, &entities, "ctf_2fort")
                .unwrap_err()
                .contains("BspSizeMismatch")
        );
        assert!(
            authenticate(
                &bytes,
                128,
                [[0.0; 3], [49.0, 50.0, 1.0]],
                &entities,
                "ctf_2fort"
            )
            .unwrap_err()
            .contains("world bounds")
        );
        let (disconnected, entities) = fixture(0, false);
        assert!(
            authenticate(&disconnected, 128, bounds, &entities, "ctf_2fort")
                .unwrap_err()
                .contains("topology")
        );
        bytes.push(0);
        assert!(
            authenticate(&bytes, 128, bounds, &entities, "ctf_2fort")
                .unwrap_err()
                .contains("TrailingBytes")
        );
    }

    #[test]
    fn rejects_foreign_headers_before_entity_or_topology_admission() {
        let entities = playsrc_entity::parse(b"\0", playsrc_entity::Limits::default()).unwrap();
        let mut bytes = Vec::new();
        for value in [
            playsrc_nav::MAGIC,
            playsrc_nav::CURRENT_VERSION,
            playsrc_nav::TF2_SUBVERSION,
            42,
        ] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.push(1);
        assert!(
            authenticate(&bytes, 41, [[-1.0; 3], [1.0; 3]], &entities, "ctf_2fort")
                .unwrap_err()
                .contains("BspSizeMismatch")
        );
        bytes[0] = 0;
        assert!(
            authenticate(&bytes, 42, [[-1.0; 3], [1.0; 3]], &entities, "ctf_2fort")
                .unwrap_err()
                .contains("InvalidMagic")
        );
    }
}
