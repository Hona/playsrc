use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone)]
struct SharedWorld(Arc<playsrc_collision::World>);
impl playsrc_movement::Tracer for SharedWorld {
    fn trace(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: playsrc_collision::Hull,
        mask: u32,
    ) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        playsrc_movement::Tracer::trace(self.0.as_ref(), start, end, hull, mask)
    }
}

struct Slot {
    generation: u16,
    payload: Option<Vec<u8>>,
    hash: [u8; 32],
    error: u32,
    session: Option<playsrc_tf2::Session<SharedWorld>>,
    snapshot: Vec<u8>,
}
fn slots() -> &'static Mutex<Vec<Slot>> {
    static S: OnceLock<Mutex<Vec<Slot>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}
fn encode(index: usize, generation: u16) -> u32 {
    ((generation as u32) << 16) | (index as u32 + 1)
}
fn decode(handle: u32) -> Option<(usize, u16)> {
    let index = (handle & 0xffff).checked_sub(1)? as usize;
    Some((index, (handle >> 16) as u16))
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_alloc(length: usize) -> *mut u8 {
    let mut bytes = Vec::<u8>::with_capacity(length);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must be a live allocation returned by `playsrc_alloc` with the same capacity.
pub unsafe extern "C" fn playsrc_free(pointer: *mut u8, length: usize) {
    if !pointer.is_null() {
        drop(unsafe { Vec::from_raw_parts(pointer, 0, length) });
    }
}
#[unsafe(no_mangle)]
/// # Safety
/// Each nonempty pointer/length pair must identify readable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_compile_map(
    bsp_pointer: *const u8,
    bsp_length: usize,
    profile: u32,
    configuration_pointer: *const u8,
    configuration_length: usize,
) -> u32 {
    let bsp_bytes = if bsp_length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bsp_pointer, bsp_length) }
    };
    let configuration = if configuration_length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(configuration_pointer, configuration_length) }
    };
    let result = (|| {
        let bsp = playsrc_bsp::parse(
            bsp_bytes,
            playsrc_bsp::Profile::Source2013V20,
            playsrc_bsp::Limits::default(),
        )
        .map_err(|_| 1_u32)?;
        let bsp_sha: [u8; 32] = Sha256::digest(bsp_bytes).into();
        let profile = match profile {
            0 => playsrc_map::LightingProfile::Ldr,
            1 => playsrc_map::LightingProfile::Hdr,
            _ => return Err(2),
        };
        let canonical = playsrc_map::compile(&bsp, profile).map_err(|_| 3_u32)?;
        let resolved_materials = resolve_materials(&canonical, configuration).map_err(|_| 7_u32)?;
        let runtime = playsrc_map::compile_runtime(
            &bsp,
            bsp_sha,
            profile,
            "playsrc-map-runtime-1",
            configuration,
            &resolved_materials,
        )
        .map_err(|_| 3_u32)?;
        let spawn = spawn(&runtime.entities).ok_or(4_u32)?;
        let collision = Arc::new(runtime.collision);
        let entities = playsrc_entity::Runtime::compile(&runtime.entities, collision.clone())
            .map_err(|_| 5_u32)?;
        Ok((
            runtime.descriptor.payload,
            runtime.descriptor.payload_sha256,
            playsrc_tf2::Session::new(SharedWorld(collision), spawn, entities),
        ))
    })();
    let mut slots = slots().lock().expect("TF2 slots");
    let index = slots
        .iter()
        .position(|slot| slot.payload.is_none())
        .unwrap_or(slots.len());
    let generation = if index == slots.len() {
        1
    } else {
        slots[index].generation.wrapping_add(1).max(1)
    };
    let slot = match result {
        Ok((payload, hash, session)) => Slot {
            generation,
            payload: Some(payload),
            hash,
            error: 0,
            session: Some(session),
            snapshot: Vec::new(),
        },
        Err(error) => Slot {
            generation,
            payload: Some(Vec::new()),
            hash: [0; 32],
            error,
            session: None,
            snapshot: Vec::new(),
        },
    };
    if index == slots.len() {
        slots.push(slot)
    } else {
        slots[index] = slot
    }
    encode(index, generation)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_result_length(handle: u32) -> usize {
    with(handle, |slot| slot.payload.as_ref().map_or(0, Vec::len)).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_result_error(handle: u32) -> u32 {
    with(handle, |slot| slot.error).unwrap_or(u32::MAX)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_result_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        let Some(payload) = &slot.payload else {
            return 0;
        };
        if capacity < payload.len() {
            return 0;
        }
        unsafe { std::ptr::copy_nonoverlapping(payload.as_ptr(), pointer, payload.len()) };
        payload.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify at least 32 writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_result_hash(handle: u32, pointer: *mut u8) -> u32 {
    with(handle, |slot| {
        unsafe { std::ptr::copy_nonoverlapping(slot.hash.as_ptr(), pointer, 32) };
        1
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_dispose(handle: u32) -> u32 {
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    slot.payload = None;
    slot.hash = [0; 32];
    slot.error = 0;
    slot.session = None;
    slot.snapshot.clear();
    1
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify the fixed command bytes in this module's memory.
pub unsafe extern "C" fn playsrc_game_advance(
    handle: u32,
    pointer: *const u8,
    length: usize,
    tick_count: u32,
) -> u32 {
    if length != 24 || tick_count == 0 || tick_count > 64 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Some(command) = decode_command(bytes) else {
        return 0;
    };
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    let Some(session) = slot.session.as_mut() else {
        return 0;
    };
    let mut candidate = session.clone();
    let mut snapshot = None;
    let mut events = Vec::new();
    for _ in 0..tick_count {
        match candidate.advance(command) {
            Ok(mut value) => {
                events.append(&mut value.events);
                snapshot = Some(value);
            }
            Err(_) => return 0,
        }
    }
    let mut snapshot = snapshot.expect("positive tick count");
    snapshot.events = events;
    *session = candidate;
    slot.snapshot = encode_snapshot(&snapshot);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_snapshot_length(handle: u32) -> usize {
    with(handle, |slot| slot.snapshot.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_teleport_count(handle: u32) -> usize {
    with(handle, |slot| {
        slot.session
            .as_ref()
            .map_or(0, playsrc_tf2::Session::teleport_count)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_teleport_destination_count(handle: u32) -> usize {
    with(handle, |slot| {
        slot.session
            .as_ref()
            .map_or(0, playsrc_tf2::Session::teleport_destination_count)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_snapshot_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.snapshot.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(slot.snapshot.as_ptr(), pointer, slot.snapshot.len())
        };
        slot.snapshot.len()
    })
    .unwrap_or(0)
}

fn decode_command(bytes: &[u8]) -> Option<playsrc_tf2::Command> {
    let f =
        |offset| f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("command field"));
    let flags = u32::from_le_bytes(bytes[16..20].try_into().expect("command flags"));
    let select = u32::from_le_bytes(bytes[20..24].try_into().expect("command selection"));
    let command = playsrc_tf2::Command {
        movement: playsrc_movement::Command {
            forward: f(0),
            side: f(4),
            yaw_degrees: f(8),
            jump: flags & 1 != 0,
            crouch: flags & 2 != 0,
        },
        pitch_degrees: f(12),
        fire: flags & 4 != 0,
        detonate: flags & 8 != 0,
        select_class: match select & 0xff {
            1 => Some(playsrc_tf2::Class::Soldier),
            2 => Some(playsrc_tf2::Class::Demoman),
            _ => None,
        },
        select_weapon: match (select >> 8) & 0xff {
            1 => Some(playsrc_tf2::Weapon::RocketLauncher),
            2 => Some(playsrc_tf2::Weapon::Original),
            3 => Some(playsrc_tf2::Weapon::StickybombLauncher),
            _ => None,
        },
    };
    [
        command.movement.forward,
        command.movement.side,
        command.movement.yaw_degrees,
        command.pitch_degrees,
    ]
    .into_iter()
    .all(f32::is_finite)
    .then_some(command)
}

fn encode_snapshot(snapshot: &playsrc_tf2::Snapshot) -> Vec<u8> {
    let mut out = b"PSSN".to_vec();
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&snapshot.tick.to_le_bytes());
    out.push(class_code(snapshot.class));
    out.push(weapon_code(snapshot.weapon));
    out.push(u8::from(snapshot.player.grounded));
    out.push(u8::from(snapshot.player.crouched));
    for value in snapshot
        .player
        .position
        .into_iter()
        .chain(snapshot.player.velocity)
    {
        out.extend_from_slice(&value.to_le_bytes())
    }
    out.extend_from_slice(&snapshot.health.to_le_bytes());
    out.extend_from_slice(&(snapshot.projectiles.len() as u32).to_le_bytes());
    for projectile in &snapshot.projectiles {
        out.extend_from_slice(&projectile.id.to_le_bytes());
        out.push(projectile_code(projectile.kind));
        out.push(u8::from(projectile.armed));
        out.push(u8::from(projectile.stuck));
        out.push(0);
        for value in projectile.position {
            out.extend_from_slice(&value.to_le_bytes())
        }
        for value in projectile.velocity {
            out.extend_from_slice(&value.to_le_bytes())
        }
        out.extend_from_slice(&projectile.age.to_le_bytes());
    }
    out.extend_from_slice(&(snapshot.events.len() as u32).to_le_bytes());
    for event in &snapshot.events {
        let (kind, detail, subject, auxiliary, data) = match event {
            playsrc_tf2::Event::ClassChanged(class) => (1, class_code(*class), 0, 0, [0.; 4]),
            playsrc_tf2::Event::WeaponChanged(weapon) => (2, weapon_code(*weapon), 0, 0, [0.; 4]),
            playsrc_tf2::Event::Fired { projectile, kind } => {
                (3, projectile_code(*kind), *projectile, 0, [0.; 4])
            }
            playsrc_tf2::Event::Explosion {
                projectile,
                position,
            } => (
                4,
                0,
                *projectile,
                0,
                [position[0], position[1], position[2], 0.],
            ),
            playsrc_tf2::Event::Damaged { amount, health } => {
                (5, 0, 0, 0, [*amount, *health, 0., 0.])
            }
            playsrc_tf2::Event::BlastImpulse { velocity } => {
                (6, 0, 0, 0, [velocity[0], velocity[1], velocity[2], 0.])
            }
            playsrc_tf2::Event::Respawned => (7, 0, 0, 0, [0.; 4]),
            playsrc_tf2::Event::Teleported {
                trigger,
                destination,
                position,
                yaw_degrees,
            } => (
                8,
                u8::from(yaw_degrees.is_some()),
                *trigger,
                *destination,
                [
                    position[0],
                    position[1],
                    position[2],
                    yaw_degrees.unwrap_or(0.),
                ],
            ),
        };
        out.extend_from_slice(&[kind, detail, 0, 0]);
        out.extend_from_slice(&subject.to_le_bytes());
        out.extend_from_slice(&auxiliary.to_le_bytes());
        for value in data {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

fn class_code(class: playsrc_tf2::Class) -> u8 {
    match class {
        playsrc_tf2::Class::Soldier => 1,
        playsrc_tf2::Class::Demoman => 2,
    }
}
fn weapon_code(weapon: playsrc_tf2::Weapon) -> u8 {
    match weapon {
        playsrc_tf2::Weapon::RocketLauncher => 1,
        playsrc_tf2::Weapon::Original => 2,
        playsrc_tf2::Weapon::StickybombLauncher => 3,
    }
}
fn projectile_code(kind: playsrc_tf2::ProjectileKind) -> u8 {
    match kind {
        playsrc_tf2::ProjectileKind::Rocket => 1,
        playsrc_tf2::ProjectileKind::Sticky => 2,
    }
}

fn bundle(bytes: &[u8]) -> Result<BTreeMap<String, &[u8]>, ()> {
    if bytes.is_empty() {
        return Ok(BTreeMap::new());
    }
    if bytes.len() < 12
        || &bytes[..4] != b"PSDB"
        || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| ())?) != 1
    {
        return Err(());
    }
    let count = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| ())?) as usize;
    if count > 4_096 {
        return Err(());
    }
    let mut offset = 12;
    let mut output = BTreeMap::new();
    for _ in 0..count {
        let path = bundle_field(bytes, &mut offset)?;
        let value = bundle_field(bytes, &mut offset)?;
        let path = std::str::from_utf8(path).map_err(|_| ())?;
        if path.is_empty()
            || path.len() > 1_024
            || path != path.to_ascii_lowercase()
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || output.insert(path.to_owned(), value).is_some()
        {
            return Err(());
        }
    }
    (offset == bytes.len()).then_some(output).ok_or(())
}

fn bundle_field<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a [u8], ()> {
    let end = (*offset).checked_add(4).ok_or(())?;
    let length = u32::from_le_bytes(
        bytes
            .get(*offset..end)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    ) as usize;
    *offset = end;
    let end = (*offset).checked_add(length).ok_or(())?;
    let value = bytes.get(*offset..end).ok_or(())?;
    *offset = end;
    Ok(value)
}

fn dependency_path(token: &[u8]) -> Result<String, ()> {
    let value = std::str::from_utf8(token)
        .map_err(|_| ())?
        .replace('\\', "/");
    let mut path = if value.to_ascii_lowercase().starts_with("materials/") {
        value
    } else {
        format!("materials/{value}")
    };
    if !path.to_ascii_lowercase().ends_with(".vmt") {
        path.push_str(".vmt");
    }
    Ok(path.to_ascii_lowercase())
}

fn resolve_materials(
    map: &playsrc_map::CanonicalMap,
    configuration: &[u8],
) -> Result<Vec<playsrc_map::RuntimeMaterial>, ()> {
    let bundle = bundle(configuration)?;
    if bundle.is_empty() {
        return Ok(Vec::new());
    }
    map.materials
        .iter()
        .map(|reference| {
            let identity = reference.logical_path.to_ascii_lowercase();
            let root = *bundle.get(&identity).ok_or(())?;
            let mut responses = Vec::new();
            let material = loop {
                match playsrc_vmt::compose(
                    root,
                    identity.clone(),
                    &responses,
                    &playsrc_keyvalues::ConditionEnvironment::default(),
                    playsrc_vmt::Limits::default(),
                )
                .map_err(|_| ())?
                {
                    playsrc_vmt::Composition::Complete(document) => {
                        break playsrc_material::resolve(&document).map_err(|_| ())?;
                    }
                    playsrc_vmt::Composition::Needs(requests) => {
                        for request in requests {
                            let path = dependency_path(&request.target_token)?;
                            responses.push(playsrc_vmt::DependencyResponse {
                                parent_identity: request.parent_identity,
                                target_token: request.target_token,
                                canonical_identity: path.clone(),
                                bytes: Some(bundle.get(&path).ok_or(())?.to_vec()),
                            });
                        }
                    }
                }
            };
            let base = if let Some(texture) = material.textures.iter().find(|texture| {
                texture.role == playsrc_material::TextureRole::Base
                    && texture.disposition == playsrc_material::TextureDisposition::Source
            }) {
                let path = texture
                    .logical_path
                    .as_ref()
                    .ok_or(())?
                    .to_ascii_lowercase();
                let bytes = *bundle.get(&path).ok_or(())?;
                let plane = playsrc_vtf::decode(
                    bytes,
                    playsrc_vtf::Dialect::Source2013Pc,
                    playsrc_vtf::SubresourceIdentity::HighResolution {
                        mip: 0,
                        frame: 0,
                        face: playsrc_vtf::Face::Right,
                        slice: 0,
                    },
                    playsrc_vtf::Limits::default(),
                )
                .map_err(|_| ())?;
                let rgba = match plane.channel_layout {
                    playsrc_vtf::ChannelLayout::Rgba => plane.samples,
                    playsrc_vtf::ChannelLayout::Rgb => {
                        let mut output =
                            Vec::with_capacity(plane.width as usize * plane.height as usize * 4);
                        for pixel in plane.samples.chunks_exact(3) {
                            output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
                        }
                        output
                    }
                };
                Some(playsrc_map::RuntimeTexture {
                    logical_path: path,
                    width: plane.width,
                    height: plane.height,
                    rgba,
                })
            } else {
                None
            };
            let shader = match material.shader {
                playsrc_material::Shader::LightmappedGeneric => 1,
                playsrc_material::Shader::VertexLitGeneric => 2,
                playsrc_material::Shader::UnlitGeneric => 3,
                playsrc_material::Shader::WorldVertexTransition => 4,
                playsrc_material::Shader::Water => 5,
                playsrc_material::Shader::Refract => 6,
                playsrc_material::Shader::Sprite => 7,
                playsrc_material::Shader::Unsupported => 255,
            };
            let features = u8::from(material.features.translucent)
                | (u8::from(material.features.additive) << 1)
                | (u8::from(material.features.alpha_test) << 2)
                | (u8::from(material.features.no_cull) << 3)
                | (u8::from(material.features.self_illum) << 4)
                | (u8::from(material.features.ss_bump) << 5);
            Ok(playsrc_map::RuntimeMaterial {
                logical_path: identity,
                shader,
                features,
                base_texture: base,
            })
        })
        .collect()
}

fn spawn(graph: &playsrc_entity::Graph) -> Option<[f32; 3]> {
    graph
        .entities
        .iter()
        .find(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"info_player_teamspawn"))
        })
        .and_then(|entity| {
            entity
                .pairs
                .iter()
                .find(|pair| pair.key.eq_ignore_ascii_case(b"origin"))
        })
        .and_then(|pair| {
            let text = std::str::from_utf8(&pair.value).ok()?;
            let values: Vec<_> = text
                .split_ascii_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<_, _>>()
                .ok()?;
            (values.len() == 3 && values.iter().all(|value| value.is_finite()))
                .then(|| [values[0], values[1], values[2]])
        })
}
fn with<T>(handle: u32, read: impl FnOnce(&Slot) -> T) -> Option<T> {
    let (index, generation) = decode(handle)?;
    let slots = slots().lock().ok()?;
    let slot = slots.get(index)?;
    (slot.generation == generation && slot.payload.is_some()).then(|| read(slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_handles_do_not_read_reused_slots() {
        let mut guard = slots().lock().unwrap();
        guard.clear();
        guard.push(Slot {
            generation: 1,
            payload: Some(vec![1, 2]),
            hash: [3; 32],
            error: 0,
            session: None,
            snapshot: Vec::new(),
        });
        drop(guard);
        let old = encode(0, 1);
        assert_eq!(playsrc_result_length(old), 2);
        assert_eq!(playsrc_dispose(old), 1);
        let mut guard = slots().lock().unwrap();
        guard[0] = Slot {
            generation: 2,
            payload: Some(vec![4]),
            hash: [5; 32],
            error: 0,
            session: None,
            snapshot: Vec::new(),
        };
        drop(guard);
        assert_eq!(playsrc_result_length(old), 0);
        assert_eq!(playsrc_result_length(encode(0, 2)), 1);
    }

    #[test]
    fn command_and_snapshot_binary_contract_is_stable() {
        let mut bytes = [0; 24];
        bytes[0..4].copy_from_slice(&240_f32.to_le_bytes());
        bytes[12..16].copy_from_slice(&(-30_f32).to_le_bytes());
        bytes[16..20].copy_from_slice(&13_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&0x0302_u32.to_le_bytes());
        let command = decode_command(&bytes).unwrap();
        assert_eq!(command.movement.forward, 240.);
        assert_eq!(command.pitch_degrees, -30.);
        assert!(command.movement.jump && command.fire && command.detonate);
        assert_eq!(command.select_class, Some(playsrc_tf2::Class::Demoman));
        assert_eq!(
            command.select_weapon,
            Some(playsrc_tf2::Weapon::StickybombLauncher)
        );
        bytes[0..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(decode_command(&bytes).is_none());
        let snapshot = playsrc_tf2::Snapshot {
            tick: 9,
            class: playsrc_tf2::Class::Soldier,
            weapon: playsrc_tf2::Weapon::Original,
            player: playsrc_movement::Player {
                position: [1., 2., 3.],
                velocity: [4., 5., 6.],
                grounded: true,
                crouched: false,
                jump_latched: true,
            },
            health: 175.,
            projectiles: vec![playsrc_tf2::Projectile {
                id: 12,
                kind: playsrc_tf2::ProjectileKind::Rocket,
                position: [7., 8., 9.],
                velocity: [10., 11., 12.],
                age: 0.5,
                armed: true,
                stuck: false,
            }],
            events: vec![
                playsrc_tf2::Event::Explosion {
                    projectile: 12,
                    position: [7., 8., 9.],
                },
                playsrc_tf2::Event::Teleported {
                    trigger: 20,
                    destination: 21,
                    position: [13., 14., 15.],
                    yaw_degrees: Some(90.),
                },
            ],
        };
        let encoded = encode_snapshot(&snapshot);
        assert_eq!(&encoded[..8], b"PSSN\x02\0\0\0");
        assert_eq!(encoded.len(), 148);
        assert_eq!(u32::from_le_bytes(encoded[48..52].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[88..92].try_into().unwrap()), 2);
        assert_eq!(&encoded[92..104], &[4, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&encoded[120..132], &[8, 1, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0]);
    }
}
