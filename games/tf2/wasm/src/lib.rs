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

#[derive(Clone, Copy)]
struct Spawn {
    entity: u32,
    hammer_id: u32,
    position: [f32; 3],
    angles: [f32; 3],
}

struct Slot {
    generation: u16,
    payload: Option<Vec<u8>>,
    hash: [u8; 32],
    derived_hash: [u8; 32],
    error: u32,
    spawn: Option<Spawn>,
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
        let canonical = playsrc_map::compile(&bsp, profile).map_err(|error| {
            if error.code == playsrc_map::ErrorCode::IncompleteLightingProfile {
                6_u32
            } else {
                3_u32
            }
        })?;
        let resolved_materials =
            resolve_materials(&canonical, configuration, profile).map_err(|_| 7_u32)?;
        let entity_graph =
            playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
                .map_err(|_| 3_u32)?;
        let (runtime_models, model_occurrences) =
            resolve_models(&entity_graph, configuration, profile).map_err(|_| 8_u32)?;
        let profile_materials =
            resolve_profile_materials(&entity_graph, configuration, profile).map_err(|_| 7_u32)?;
        let inputs = runtime_inputs(configuration).map_err(|_| 7_u32)?;
        let runtime = playsrc_map::compile_runtime(
            &bsp,
            bsp_sha,
            profile,
            playsrc_map::RuntimeAssembly {
                compiler_identity: if profile == playsrc_map::LightingProfile::Hdr {
                    "playsrc-map-runtime-hdr-1"
                } else {
                    "playsrc-map-runtime-2"
                },
                configuration,
                materials: &resolved_materials,
                profile_materials: &profile_materials,
                inputs: &inputs,
                output_role: if profile == playsrc_map::LightingProfile::Hdr {
                    "map-runtime-hdr"
                } else {
                    "map-runtime"
                },
                models: &runtime_models,
                model_occurrences: &model_occurrences,
            },
        )
        .map_err(|error| {
            if error.code == playsrc_map::ErrorCode::IncompleteLightingProfile {
                6_u32
            } else {
                3_u32
            }
        })?;
        let spawn = spawn(&runtime.entities).ok_or(4_u32)?;
        let collision = Arc::new(runtime.collision);
        let entities = playsrc_entity::Runtime::compile(&runtime.entities, collision.clone())
            .map_err(|_| 5_u32)?;
        Ok((
            runtime.descriptor.payload,
            runtime.descriptor.payload_sha256,
            runtime.descriptor.derived_sha256,
            spawn,
            playsrc_tf2::Session::new(SharedWorld(collision), spawn.position, entities),
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
        Ok((payload, hash, derived_hash, spawn, session)) => Slot {
            generation,
            payload: Some(payload),
            hash,
            derived_hash,
            error: 0,
            spawn: Some(spawn),
            session: Some(session),
            snapshot: Vec::new(),
        },
        Err(error) => Slot {
            generation,
            payload: Some(Vec::new()),
            hash: [0; 32],
            derived_hash: [0; 32],
            error,
            spawn: None,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledArtifact {
    pub payload: Vec<u8>,
    pub payload_sha256: [u8; 32],
    pub derived_sha256: [u8; 32],
}
pub fn compile_artifact(
    bsp: &[u8],
    profile: u32,
    configuration: &[u8],
) -> Result<CompiledArtifact, u32> {
    let handle = unsafe {
        playsrc_compile_map(
            bsp.as_ptr(),
            bsp.len(),
            profile,
            configuration.as_ptr(),
            configuration.len(),
        )
    };
    let failure = playsrc_result_error(handle);
    if failure != 0 {
        playsrc_dispose(handle);
        return Err(failure);
    }
    let mut payload = vec![0; playsrc_result_length(handle)];
    if unsafe { playsrc_result_copy(handle, payload.as_mut_ptr(), payload.len()) } != payload.len()
    {
        playsrc_dispose(handle);
        return Err(u32::MAX);
    }
    let mut payload_sha256 = [0; 32];
    let mut derived_sha256 = [0; 32];
    if unsafe { playsrc_result_hash(handle, payload_sha256.as_mut_ptr()) } != 1
        || unsafe { playsrc_result_derived_hash(handle, derived_sha256.as_mut_ptr()) } != 1
    {
        playsrc_dispose(handle);
        return Err(u32::MAX);
    }
    playsrc_dispose(handle);
    Ok(CompiledArtifact {
        payload,
        payload_sha256,
        derived_sha256,
    })
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
/// # Safety
/// `pointer` must identify at least 32 writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_result_derived_hash(handle: u32, pointer: *mut u8) -> u32 {
    with(handle, |slot| {
        unsafe { std::ptr::copy_nonoverlapping(slot.derived_hash.as_ptr(), pointer, 32) };
        1
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_spawn_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    const LENGTH: usize = 40;
    with(handle, |slot| {
        let Some(spawn) = slot.spawn else {
            return 0;
        };
        if capacity < LENGTH {
            return 0;
        }
        let mut bytes = [0_u8; LENGTH];
        bytes[0..4].copy_from_slice(b"PSIV");
        bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&spawn.entity.to_le_bytes());
        bytes[12..16].copy_from_slice(&spawn.hammer_id.to_le_bytes());
        for (index, value) in spawn.position.into_iter().chain(spawn.angles).enumerate() {
            let start = 16 + index * 4;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, LENGTH) };
        LENGTH
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
    slot.derived_hash = [0; 32];
    slot.error = 0;
    slot.spawn = None;
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
                kind,
                position,
            } => (
                4,
                projectile_code(*kind),
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
    profile: playsrc_map::LightingProfile,
) -> Result<Vec<playsrc_map::RuntimeMaterial>, ()> {
    let bundle = bundle(configuration)?;
    if bundle.is_empty() {
        return Ok(Vec::new());
    }
    map.materials
        .iter()
        .map(|reference| {
            resolve_material(
                &reference.logical_path.to_ascii_lowercase(),
                &bundle,
                true,
                material_environment(profile, false),
            )
        })
        .collect()
}

fn resolve_material(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    include_texture: bool,
    environment: playsrc_material::SelectionEnvironment,
) -> Result<playsrc_map::RuntimeMaterial, ()> {
    let material = resolve_material_semantics(identity, bundle, environment)?;
    let selected_role = material.selected_textures.first().copied();
    let base = if include_texture
        && let Some(texture) = material.textures.iter().find(|texture| {
            Some(texture.role) == selected_role
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
    Ok(playsrc_map::RuntimeMaterial {
        logical_path: identity.to_owned(),
        shader: shader_code(material.shader),
        features: feature_bits(material.features),
        texture_role: selected_role.map_or(0, texture_role),
        base_texture: base,
    })
}

fn resolve_material_semantics(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    environment: playsrc_material::SelectionEnvironment,
) -> Result<playsrc_material::Material, ()> {
    let root = *bundle.get(identity).ok_or(())?;
    let mut responses = Vec::new();
    let material = loop {
        match playsrc_vmt::compose(
            root,
            identity.to_owned(),
            &responses,
            &playsrc_keyvalues::ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|_| ())?
        {
            playsrc_vmt::Composition::Complete(document) => {
                break playsrc_material::resolve_for_environment(&document, environment)
                    .map_err(|_| ())?;
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
    Ok(material)
}

fn shader_code(shader: playsrc_material::Shader) -> u8 {
    match shader {
        playsrc_material::Shader::LightmappedGeneric => 1,
        playsrc_material::Shader::VertexLitGeneric => 2,
        playsrc_material::Shader::UnlitGeneric => 3,
        playsrc_material::Shader::WorldVertexTransition => 4,
        playsrc_material::Shader::Water => 5,
        playsrc_material::Shader::Refract => 6,
        playsrc_material::Shader::Sprite => 7,
        playsrc_material::Shader::SkyHdr => 8,
        playsrc_material::Shader::SkyLdr => 9,
        playsrc_material::Shader::Unsupported => 255,
    }
}

fn feature_bits(features: playsrc_material::Features) -> u8 {
    u8::from(features.translucent)
        | (u8::from(features.additive) << 1)
        | (u8::from(features.alpha_test) << 2)
        | (u8::from(features.no_cull) << 3)
        | (u8::from(features.self_illum) << 4)
        | (u8::from(features.ss_bump) << 5)
}

fn material_environment(
    profile: playsrc_map::LightingProfile,
    model: bool,
) -> playsrc_material::SelectionEnvironment {
    playsrc_material::SelectionEnvironment {
        hdr_mode: if profile == playsrc_map::LightingProfile::Hdr {
            playsrc_material::HdrMode::Integer
        } else {
            playsrc_material::HdrMode::None
        },
        model,
        ..playsrc_material::SelectionEnvironment::default()
    }
}

fn texture_role(role: playsrc_material::TextureRole) -> u8 {
    match role {
        playsrc_material::TextureRole::Base => 0,
        playsrc_material::TextureRole::HdrBase => 1,
        playsrc_material::TextureRole::HdrCompressed => 2,
        playsrc_material::TextureRole::HdrCompressed0 => 3,
        playsrc_material::TextureRole::HdrCompressed1 => 4,
        playsrc_material::TextureRole::HdrCompressed2 => 5,
        _ => 255,
    }
}

fn resolve_profile_materials(
    graph: &playsrc_entity::Graph,
    configuration: &[u8],
    profile: playsrc_map::LightingProfile,
) -> Result<Vec<playsrc_map::RuntimeProfileMaterial>, ()> {
    if profile != playsrc_map::LightingProfile::Hdr {
        return Ok(Vec::new());
    }
    let bundle = bundle(configuration)?;
    let world = graph
        .entities
        .iter()
        .find(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"worldspawn"))
        })
        .ok_or(())?;
    let sky = world
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(b"skyname"))
        .ok_or(())?;
    let sky = std::str::from_utf8(&sky.value).map_err(|_| ())?;
    if sky.is_empty()
        || !sky
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(());
    }
    ["rt", "lf", "bk", "ft", "up", "dn"]
        .into_iter()
        .map(|suffix| {
            let identity = format!("materials/skybox/{sky}_hdr{suffix}.vmt").to_ascii_lowercase();
            let material = resolve_material_semantics(
                &identity,
                &bundle,
                material_environment(profile, false),
            )
            .map_err(|_| ())?;
            let role = *material.selected_textures.first().ok_or(())?;
            let texture = material
                .textures
                .iter()
                .find(|texture| texture.role == role)
                .ok_or(())?;
            let path = texture
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase();
            let bytes = *bundle.get(&path).ok_or(())?;
            let metadata = playsrc_vtf::inspect(
                bytes,
                playsrc_vtf::Dialect::Source2013Pc,
                playsrc_vtf::Limits::default(),
            )
            .map_err(|_| ())?;
            if metadata.frame_count != 1
                || metadata.depth != 1
                || metadata.width == 0
                || metadata.height == 0
                || metadata.width > 4_096
                || metadata.height > 4_096
            {
                return Err(());
            }
            Ok(playsrc_map::RuntimeProfileMaterial {
                logical_path: identity,
                shader: shader_code(material.shader),
                features: feature_bits(material.features),
                texture_role: texture_role(role),
                texture: playsrc_map::RuntimeProfileTexture {
                    logical_path: path,
                    width: metadata.width,
                    height: metadata.height,
                    format: metadata.high_format.code(),
                    source_sha256: Sha256::digest(bytes).into(),
                    source_bytes: bytes.to_vec(),
                },
            })
        })
        .collect()
}

fn runtime_inputs(configuration: &[u8]) -> Result<Vec<playsrc_map::RuntimeInput>, ()> {
    Ok(bundle(configuration)?
        .into_iter()
        .map(|(logical_path, bytes)| playsrc_map::RuntimeInput {
            role: 1,
            logical_path,
            sha256: Sha256::digest(bytes).into(),
        })
        .collect())
}

fn model_profile(bytes: &[u8]) -> Result<playsrc_studio_model::Profile, ()> {
    let version = i32::from_le_bytes(bytes.get(4..8).ok_or(())?.try_into().map_err(|_| ())?);
    match version {
        44 => Ok(playsrc_studio_model::Profile::SourcePcMdl44),
        45 => Ok(playsrc_studio_model::Profile::SourcePcMdl45),
        46 => Ok(playsrc_studio_model::Profile::SourcePcMdl46),
        47 => Ok(playsrc_studio_model::Profile::SourcePcMdl47),
        48 => Ok(playsrc_studio_model::Profile::SourcePcMdl48),
        _ => Err(()),
    }
}

fn load_model(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
) -> Result<Box<playsrc_studio_model::Document>, ()> {
    let mdl = *bundle.get(identity).ok_or(())?;
    let mut responses = Vec::new();
    loop {
        match playsrc_studio_model::load(
            identity,
            model_profile(mdl)?,
            playsrc_studio_model::VtxVariant::Dx90,
            mdl,
            &responses,
            playsrc_studio_model::Limits::default(),
        )
        .map_err(|_| ())?
        {
            playsrc_studio_model::Load::Complete(document) => return Ok(document),
            playsrc_studio_model::Load::Needs(requests) => {
                for request in requests {
                    let path = request.logical_path.to_ascii_lowercase();
                    let bytes = bundle.get(&path).map(|bytes| bytes.to_vec());
                    if request.role != playsrc_studio_model::DependencyRole::Physics
                        && bytes.is_none()
                    {
                        return Err(());
                    }
                    responses.push(playsrc_studio_model::DependencyResponse {
                        requester: request.requester,
                        role: request.role,
                        logical_path: path,
                        bytes,
                    });
                }
            }
        }
    }
}

fn entity_vector(entity: &playsrc_entity::Entity, key: &[u8]) -> Result<[f32; 3], ()> {
    let Some(pair) = entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
    else {
        return Ok([0.; 3]);
    };
    let values = std::str::from_utf8(&pair.value)
        .map_err(|_| ())?
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        return Err(());
    }
    Ok([values[0], values[1], values[2]])
}

fn resolve_models(
    graph: &playsrc_entity::Graph,
    configuration: &[u8],
    profile: playsrc_map::LightingProfile,
) -> Result<
    (
        Vec<playsrc_map::RuntimeModel>,
        Vec<playsrc_map::RuntimeModelOccurrence>,
    ),
    (),
> {
    let bundle = bundle(configuration)?;
    if bundle.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut roots = std::collections::BTreeSet::from([
        "models/weapons/w_models/w_rocket.mdl".to_owned(),
        "models/weapons/w_models/w_stickybomb.mdl".to_owned(),
        "models/weapons/v_models/v_rocketlauncher_soldier.mdl".to_owned(),
        "models/weapons/v_models/v_stickybomb_launcher_demo.mdl".to_owned(),
    ]);
    for entity in &graph.entities {
        if entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
            && let Some(model) = &entity.model
        {
            roots.insert(
                std::str::from_utf8(model)
                    .map_err(|_| ())?
                    .to_ascii_lowercase(),
            );
        }
    }
    let mut models = Vec::new();
    let mut indexes = BTreeMap::new();
    for identity in roots {
        let document = load_model(&identity, &bundle)?;
        let mut materials = Vec::new();
        for material in &document.materials {
            let path = material
                .candidates
                .iter()
                .find(|candidate| bundle.contains_key(&candidate.to_ascii_lowercase()))
                .ok_or(())?
                .to_ascii_lowercase();
            let mut material =
                resolve_material(&path, &bundle, false, material_environment(profile, true))?;
            material.base_texture = None;
            materials.push(material);
        }
        let skin = document.skins.first();
        let mut primitives = Vec::new();
        for primitive in document
            .geometry
            .iter()
            .filter(|primitive| primitive.lod == 0 && primitive.model == 0)
        {
            let material = skin
                .and_then(|family| family.texture_indices.get(primitive.material_slot))
                .map_or(primitive.material_slot, |index| *index as usize);
            primitives.push(playsrc_map::RuntimeModelPrimitive {
                material,
                positions: primitive
                    .vertices
                    .iter()
                    .map(|vertex| vertex.position.0.map(|value| f32::from_bits(value.0)))
                    .collect(),
                normals: primitive
                    .vertices
                    .iter()
                    .map(|vertex| vertex.normal.0.map(|value| f32::from_bits(value.0)))
                    .collect(),
                uv: primitive
                    .vertices
                    .iter()
                    .map(|vertex| vertex.uv.map(|value| f32::from_bits(value.0)))
                    .collect(),
                triangles: primitive.triangles.clone(),
            });
        }
        indexes.insert(identity.clone(), models.len());
        models.push(playsrc_map::RuntimeModel {
            logical_path: identity,
            materials,
            primitives,
        });
    }
    let mut occurrences = Vec::new();
    for entity in &graph.entities {
        if !entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
        {
            continue;
        }
        let identity = std::str::from_utf8(entity.model.as_deref().ok_or(())?)
            .map_err(|_| ())?
            .to_ascii_lowercase();
        occurrences.push(playsrc_map::RuntimeModelOccurrence {
            entity: entity.index,
            model: *indexes.get(&identity).ok_or(())?,
            position: entity_vector(entity, b"origin")?,
            angles: entity_vector(entity, b"angles")?,
        });
    }
    Ok((models, occurrences))
}

fn spawn(graph: &playsrc_entity::Graph) -> Option<Spawn> {
    let entity = graph.entities.iter().find(|entity| {
        entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"info_player_teamspawn"))
    })?;
    let hammer_id = match entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(b"hammerid"))
    {
        Some(pair) => std::str::from_utf8(&pair.value).ok()?.parse().ok()?,
        None => u32::MAX,
    };
    if !entity
        .pairs
        .iter()
        .any(|pair| pair.key.eq_ignore_ascii_case(b"origin"))
    {
        return None;
    }
    Some(Spawn {
        entity: entity.index.try_into().ok()?,
        hammer_id,
        position: entity_vector(entity, b"origin").ok()?,
        angles: entity_vector(entity, b"angles").ok()?,
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
            derived_hash: [6; 32],
            error: 0,
            spawn: None,
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
            derived_hash: [7; 32],
            error: 0,
            spawn: None,
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
                    kind: playsrc_tf2::ProjectileKind::Rocket,
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
        assert_eq!(&encoded[92..104], &[4, 1, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&encoded[120..132], &[8, 1, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0]);
    }
}
