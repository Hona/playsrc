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

impl playsrc_tf2::GameplayWorld for SharedWorld {
    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: playsrc_collision::Hull,
    ) -> Result<bool, playsrc_movement::Error> {
        self.0
            .overlaps_model_hull(model, origin, position, hull)
            .map_err(|_| {
                playsrc_movement::Error::new(
                    playsrc_movement::Operation::Trace,
                    playsrc_movement::FailureKind::Malformed,
                    "model overlap",
                )
            })
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
    bsp_hash: [u8; 32],
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
        let model_bounds = collision
            .models
            .iter()
            .enumerate()
            .map(|(model, value)| playsrc_entity::ModelBounds {
                model,
                mins: [
                    value.mins.x.value(),
                    value.mins.y.value(),
                    value.mins.z.value(),
                ],
                maxs: [
                    value.maxs.x.value(),
                    value.maxs.y.value(),
                    value.maxs.z.value(),
                ],
            })
            .collect();
        let map = playsrc_tf2::MapRuntime::compile(
            &entity_graph,
            playsrc_movement::Configuration::default().tick_interval,
            u64::from_le_bytes(bsp_sha[..8].try_into().expect("BSP identity prefix")),
            model_bounds,
        )
        .map_err(|_| 5_u32)?;
        let mut session = playsrc_tf2::Session::new(SharedWorld(collision), spawn.position, map);
        session.set_movement_modifiers(playsrc_tf2::MovementModifiers {
            noclip_allowed: true,
            ..playsrc_tf2::MovementModifiers::default()
        });
        Ok((
            runtime.descriptor.payload,
            runtime.descriptor.payload_sha256,
            runtime.descriptor.derived_sha256,
            bsp_sha,
            spawn,
            session,
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
        Ok((payload, hash, derived_hash, bsp_hash, spawn, session)) => Slot {
            generation,
            payload: Some(payload),
            hash,
            derived_hash,
            bsp_hash,
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
            bsp_hash: [0; 32],
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
    slot.bsp_hash = [0; 32];
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
    if length != 40 || tick_count == 0 || tick_count > 64 {
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
    let mut snapshot: Option<playsrc_tf2::Snapshot> = None;
    for _ in 0..tick_count {
        match candidate.advance(command) {
            Ok(mut value) => {
                if let Some(previous) = snapshot.as_mut() {
                    value
                        .events
                        .splice(0..0, std::mem::take(&mut previous.events));
                    value
                        .projectile_events
                        .splice(0..0, std::mem::take(&mut previous.projectile_events));
                    value
                        .entity_events
                        .splice(0..0, std::mem::take(&mut previous.entity_events));
                    if let (Some(prior), Some(current)) =
                        (previous.jump.as_mut(), value.jump.as_mut())
                    {
                        current
                            .events
                            .splice(0..0, std::mem::take(&mut prior.events));
                        if current.result.is_none() {
                            current.result = prior.result.take();
                        }
                    }
                }
                snapshot = Some(value);
            }
            Err(_) => return 0,
        }
    }
    let snapshot = snapshot.expect("positive tick count");
    let Some(encoded) = encode_snapshot(&snapshot) else {
        return 0;
    };
    *session = candidate;
    slot.snapshot = encoded;
    1
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify exactly `length` readable course-definition bytes.
pub unsafe extern "C" fn playsrc_jump_configure(
    handle: u32,
    pointer: *const u8,
    length: usize,
) -> u32 {
    if !(52..=64 * 1024).contains(&length) {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Ok(definition) =
        playsrc_tf2::jump::decode_course(bytes, playsrc_tf2::jump::Limits::default())
    else {
        return 0;
    };
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation || definition.map_identity != slot.bsp_hash {
        return 0;
    }
    let Some(session) = slot.session.as_mut() else {
        return 0;
    };
    let mut candidate = session.clone();
    if candidate.configure_jump(definition).is_err() {
        return 0;
    }
    *session = candidate;
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
            .map_or(0, |session| session.map_counts().teleport_triggers as usize)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_teleport_destination_count(handle: u32) -> usize {
    with(handle, |slot| {
        slot.session.as_ref().map_or(0, |session| {
            session.map_counts().teleport_destinations as usize
        })
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_runtime_count(handle: u32, kind: u32) -> usize {
    with(handle, |slot| {
        let Some(session) = slot.session.as_ref() else {
            return 0;
        };
        let counts = session.map_counts();
        match kind {
            1 => counts.buttons as usize,
            2 => counts.doors as usize,
            3 => counts.linear_movers as usize,
            4 => counts.multiple_triggers as usize,
            5 => counts.hurt_triggers as usize,
            6 => counts.push_triggers as usize,
            7 => counts.catapult_triggers as usize,
            8 => counts.teleport_triggers as usize,
            9 => counts.regenerate_zones as usize,
            10 => counts.teleport_destinations as usize,
            _ => 0,
        }
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
    if bytes.len() != 40
        || &bytes[..4] != b"PCMD"
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) != 2
    {
        return None;
    }
    let f = |offset| {
        f32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("fixed command scalar"),
        )
    };
    let flags = u32::from_le_bytes(bytes[28..32].try_into().ok()?);
    let select = u32::from_le_bytes(bytes[32..36].try_into().ok()?);
    if flags & !0xff != 0 {
        return None;
    }
    let optional_class = match select & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Class::Soldier),
        2 => Some(playsrc_tf2::Class::Demoman),
        _ => return None,
    };
    let optional_weapon = match (select >> 8) & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Weapon::RocketLauncher),
        2 => Some(playsrc_tf2::Weapon::Original),
        3 => Some(playsrc_tf2::Weapon::StickybombLauncher),
        _ => return None,
    };
    let optional_team = match (select >> 16) & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Team::Red),
        2 => Some(playsrc_tf2::Team::Blue),
        _ => return None,
    };
    let mode_request = match (select >> 24) & 0xff {
        0 => None,
        1 => Some(playsrc_movement::Mode::Walk),
        2 => Some(playsrc_movement::Mode::Noclip),
        _ => return None,
    };
    let target = u32::from_le_bytes(bytes[36..40].try_into().ok()?);
    let command = playsrc_tf2::Command {
        movement: playsrc_movement::Command {
            forward: f(8),
            side: f(12),
            yaw_degrees: f(20),
            jump: flags & 1 != 0,
            crouch: flags & 2 != 0,
        },
        pitch_degrees: f(24),
        up: f(16),
        speed_button: flags & 4 != 0,
        fire: flags & 8 != 0,
        detonate: flags & 16 != 0,
        reload: flags & 32 != 0,
        reset: flags & 64 != 0,
        respawn: flags & 128 != 0,
        select_class: optional_class,
        select_team: optional_team,
        select_weapon: optional_weapon,
        mode_request,
        activate_entity: (target != u32::MAX).then_some(target),
    };
    [
        command.movement.forward,
        command.movement.side,
        command.up,
        command.movement.yaw_degrees,
        command.pitch_degrees,
    ]
    .into_iter()
    .all(f32::is_finite)
    .then_some(command)
}

fn encode_snapshot(snapshot: &playsrc_tf2::Snapshot) -> Option<Vec<u8>> {
    const MAX: usize = 64 * 1024 * 1024;
    let movement = snapshot.movement.snapshot_bytes();
    let jump = match snapshot.jump.as_ref() {
        Some(value) => encode_jump(value)?,
        None => Vec::new(),
    };
    let mut out = Vec::new();
    extend(&mut out, b"PSSN", MAX)?;
    u32_field(&mut out, 3, MAX)?;
    u64_field(&mut out, snapshot.tick, MAX)?;
    extend(
        &mut out,
        &[
            class_code(snapshot.class),
            team_code(snapshot.team),
            weapon_code(snapshot.weapon),
            snapshot.movement.mode as u8,
        ],
        MAX,
    )?;
    f32_field(&mut out, snapshot.health, MAX)?;
    f32_field(&mut out, snapshot.maximum_health, MAX)?;
    u32_field(&mut out, snapshot.conditions, MAX)?;
    u32_field(&mut out, u32::try_from(snapshot.loadout.len()).ok()?, MAX)?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.projectiles.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.projectile_events.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.entity_transforms.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.entity_events.len()).ok()?,
        MAX,
    )?;
    u32_field(&mut out, u32::try_from(snapshot.events.len()).ok()?, MAX)?;
    u32_field(&mut out, u32::try_from(jump.len()).ok()?, MAX)?;
    u32_field(&mut out, u32::try_from(movement.len()).ok()?, MAX)?;
    extend(&mut out, &movement, MAX)?;
    for state in &snapshot.loadout {
        extend(
            &mut out,
            &[weapon_code(state.weapon), state.reload as u8, 0, 0],
            MAX,
        )?;
        u16_field(&mut out, state.clip, MAX)?;
        u16_field(&mut out, state.reserve, MAX)?;
        u16_field(&mut out, state.maximum_clip, MAX)?;
        u16_field(&mut out, state.maximum_reserve, MAX)?;
        u64_field(&mut out, state.next_primary_tick, MAX)?;
        u64_field(&mut out, state.next_reload_tick, MAX)?;
        u32_field(&mut out, 0, MAX)?;
    }
    for projectile in &snapshot.projectiles {
        u32_field(&mut out, projectile.identity, MAX)?;
        extend(
            &mut out,
            &[
                projectile_code(projectile.kind),
                team_code(projectile.team),
                projectile.state as u8,
                u8::from(projectile.contact_normal.is_some()),
            ],
            MAX,
        )?;
        u32_field(&mut out, projectile.owner_identity, MAX)?;
        u32_field(&mut out, projectile.launcher_identity, MAX)?;
        floats(
            &mut out,
            projectile
                .position
                .into_iter()
                .chain(projectile.velocity)
                .chain(projectile.orientation)
                .chain(projectile.angular_velocity)
                .chain(projectile.contact_normal.unwrap_or([0.0; 3]))
                .chain([projectile.age_seconds]),
            MAX,
        )?;
    }
    for event in &snapshot.projectile_events {
        extend(
            &mut out,
            &[
                event.kind as u8,
                projectile_code(event.projectile_kind),
                team_code(event.team),
                u8::from(event.contact_normal.is_some()),
            ],
            MAX,
        )?;
        u32_field(&mut out, event.projectile, MAX)?;
        u32_field(&mut out, event.owner_identity, MAX)?;
        u32_field(&mut out, event.launcher_identity, MAX)?;
        u64_field(&mut out, event.tick, MAX)?;
        floats(
            &mut out,
            event
                .position
                .into_iter()
                .chain(event.orientation)
                .chain(event.contact_normal.unwrap_or([0.0; 3])),
            MAX,
        )?;
    }
    for transform in &snapshot.entity_transforms {
        u32_field(&mut out, transform.identity, MAX)?;
        u32_field(&mut out, transform.model, MAX)?;
        floats(
            &mut out,
            transform.position.into_iter().chain(transform.angles),
            MAX,
        )?;
    }
    for event in &snapshot.entity_events {
        u64_field(&mut out, event.sequence, MAX)?;
        extend(
            &mut out,
            &[
                event.kind as u8,
                u8::from(event.accepted),
                event.contact.map_or(0, |value| match value {
                    playsrc_entity::ContactKind::Enter => 1,
                    playsrc_entity::ContactKind::Stay => 2,
                    playsrc_entity::ContactKind::Exit => 3,
                }),
                0,
            ],
            MAX,
        )?;
        u32_field(&mut out, event.entity, MAX)?;
        u32_field(&mut out, event.subject.unwrap_or(u32::MAX), MAX)?;
        u32_field(&mut out, u32::try_from(event.name.len()).ok()?, MAX)?;
        extend(&mut out, &event.name, MAX)?;
    }
    for event in &snapshot.events {
        encode_game_event(&mut out, event, MAX)?;
    }
    extend(&mut out, &jump, MAX)?;
    Some(out)
}

fn class_code(class: playsrc_tf2::Class) -> u8 {
    match class {
        playsrc_tf2::Class::Soldier => 1,
        playsrc_tf2::Class::Demoman => 2,
    }
}
fn team_code(team: playsrc_tf2::Team) -> u8 {
    match team {
        playsrc_tf2::Team::Red => 1,
        playsrc_tf2::Team::Blue => 2,
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

fn extend(output: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Option<()> {
    (output.len().checked_add(bytes.len())? <= limit).then(|| output.extend_from_slice(bytes))
}

fn u16_field(output: &mut Vec<u8>, value: u16, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn u32_field(output: &mut Vec<u8>, value: u32, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn u64_field(output: &mut Vec<u8>, value: u64, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn f32_field(output: &mut Vec<u8>, value: f32, limit: usize) -> Option<()> {
    value
        .is_finite()
        .then_some(())
        .and_then(|()| extend(output, &value.to_le_bytes(), limit))
}

fn floats(output: &mut Vec<u8>, values: impl IntoIterator<Item = f32>, limit: usize) -> Option<()> {
    for value in values {
        f32_field(output, value, limit)?;
    }
    Some(())
}

fn encode_game_event(output: &mut Vec<u8>, event: &playsrc_tf2::Event, limit: usize) -> Option<()> {
    let (kind, detail, subject, auxiliary, values) = match event {
        playsrc_tf2::Event::ClassChanged(value) => (1, class_code(*value), 0, 0, [0.0; 4]),
        playsrc_tf2::Event::TeamChanged(value) => (2, team_code(*value), 0, 0, [0.0; 4]),
        playsrc_tf2::Event::WeaponChanged(value) => (3, weapon_code(*value), 0, 0, [0.0; 4]),
        playsrc_tf2::Event::Reloaded {
            weapon,
            clip,
            reserve,
        } => (
            4,
            weapon_code(*weapon),
            0,
            0,
            [*clip as f32, *reserve as f32, 0.0, 0.0],
        ),
        playsrc_tf2::Event::Resupplied {
            entity,
            health,
            weapon,
            clip,
            reserve,
        } => (
            5,
            weapon_code(*weapon),
            *entity,
            0,
            [*health, *clip as f32, *reserve as f32, 0.0],
        ),
        playsrc_tf2::Event::Damaged { amount, health } => {
            (6, 0, 0, 0, [*amount, *health, 0.0, 0.0])
        }
        playsrc_tf2::Event::Healed {
            amount,
            health,
            trigger,
        } => (7, 0, *trigger, 0, [*amount, *health, 0.0, 0.0]),
        playsrc_tf2::Event::BlastImpulse { velocity } => {
            (8, 0, 0, 0, [velocity[0], velocity[1], velocity[2], 0.0])
        }
        playsrc_tf2::Event::Teleported {
            trigger,
            destination,
            position,
            yaw_degrees,
        } => (
            9,
            u8::from(yaw_degrees.is_some()),
            *trigger,
            *destination,
            [
                position[0],
                position[1],
                position[2],
                yaw_degrees.unwrap_or(0.0),
            ],
        ),
        playsrc_tf2::Event::TriggerVelocity { trigger, velocity } => (
            10,
            0,
            *trigger,
            0,
            [velocity[0], velocity[1], velocity[2], 0.0],
        ),
        playsrc_tf2::Event::Respawned => (11, 0, 0, 0, [0.0; 4]),
    };
    extend(output, &[kind, detail, 0, 0], limit)?;
    u32_field(output, subject, limit)?;
    u32_field(output, auxiliary, limit)?;
    floats(output, values, limit)
}

fn encode_jump(output: &playsrc_tf2::jump::TickOutput) -> Option<Vec<u8>> {
    const MAX: usize = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    extend(&mut bytes, b"PJOF", MAX)?;
    u32_field(&mut bytes, 1, MAX)?;
    match &output.run {
        None => extend(&mut bytes, &[0; 4], MAX)?,
        Some(run) => {
            extend(
                &mut bytes,
                &[1, run.disposition as u8, run.class as u8, run.team as u8],
                MAX,
            )?;
            u32_field(&mut bytes, run.instance, MAX)?;
            u32_field(&mut bytes, run.player_identity, MAX)?;
            extend(
                &mut bytes,
                &[run.invalidation.map_or(0, |value| value as u8), 0, 0, 0],
                MAX,
            )?;
            u64_field(&mut bytes, run.start_tick, MAX)?;
            u64_field(&mut bytes, run.end_tick.unwrap_or(u64::MAX), MAX)?;
            u32_field(&mut bytes, u32::try_from(run.checkpoints.len()).ok()?, MAX)?;
            for visit in &run.checkpoints {
                u32_field(&mut bytes, visit.zone_identity, MAX)?;
                u32_field(&mut bytes, visit.index, MAX)?;
                u64_field(&mut bytes, visit.tick, MAX)?;
            }
        }
    }
    u32_field(&mut bytes, u32::try_from(output.events.len()).ok()?, MAX)?;
    for event in &output.events {
        let (kind, detail) = match event.kind {
            playsrc_tf2::jump::EventKind::Started => (1, 0),
            playsrc_tf2::jump::EventKind::CheckpointAccepted => (2, 0),
            playsrc_tf2::jump::EventKind::Rejected(reason) => (3, reason as u8),
            playsrc_tf2::jump::EventKind::Completed => (4, 0),
            playsrc_tf2::jump::EventKind::Invalidated(reason) => (5, reason as u8),
            playsrc_tf2::jump::EventKind::ResetRequested => (6, 0),
            playsrc_tf2::jump::EventKind::RespawnObserved => (7, 0),
            playsrc_tf2::jump::EventKind::TeleportObserved => (8, 0),
        };
        u64_field(&mut bytes, event.sequence, MAX)?;
        u64_field(&mut bytes, event.tick, MAX)?;
        extend(&mut bytes, &[kind, detail, 0, 0], MAX)?;
        u32_field(&mut bytes, event.run_instance, MAX)?;
        u32_field(&mut bytes, event.zone_identity.unwrap_or(u32::MAX), MAX)?;
        u32_field(&mut bytes, event.zone_index.unwrap_or(u32::MAX), MAX)?;
    }
    match &output.result {
        None => extend(&mut bytes, &[0, 0, 0, 0], MAX)?,
        Some(result) => {
            extend(
                &mut bytes,
                &[
                    1,
                    result.class as u8,
                    result.team as u8,
                    result.run_kind as u8,
                ],
                MAX,
            )?;
            u64_field(&mut bytes, result.course_identity, MAX)?;
            extend(&mut bytes, &result.map_identity, MAX)?;
            u32_field(&mut bytes, result.run_instance, MAX)?;
            u32_field(&mut bytes, result.player_identity, MAX)?;
            u32_field(&mut bytes, result.zone_index, MAX)?;
            extend(&mut bytes, &[result.disposition as u8, 0, 0, 0], MAX)?;
            u64_field(&mut bytes, result.start_tick, MAX)?;
            u64_field(&mut bytes, result.end_tick, MAX)?;
            u64_field(&mut bytes, result.elapsed_ticks, MAX)?;
            u32_field(&mut bytes, result.tick_interval_bits, MAX)?;
            u32_field(
                &mut bytes,
                u32::try_from(result.checkpoints.len()).ok()?,
                MAX,
            )?;
            for visit in &result.checkpoints {
                u32_field(&mut bytes, visit.zone_identity, MAX)?;
                u32_field(&mut bytes, visit.index, MAX)?;
                u64_field(&mut bytes, visit.tick, MAX)?;
            }
        }
    }
    Some(bytes)
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
            bsp_hash: [8; 32],
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
            bsp_hash: [9; 32],
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
        let mut bytes = [0; 40];
        bytes[..4].copy_from_slice(b"PCMD");
        bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&240_f32.to_le_bytes());
        bytes[16..20].copy_from_slice(&100_f32.to_le_bytes());
        bytes[24..28].copy_from_slice(&(-30_f32).to_le_bytes());
        bytes[28..32].copy_from_slice(&0xad_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x0201_0302_u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&77_u32.to_le_bytes());
        let command = decode_command(&bytes).unwrap();
        assert_eq!(command.movement.forward, 240.);
        assert_eq!(command.up, 100.);
        assert_eq!(command.pitch_degrees, -30.);
        assert!(command.movement.jump && command.fire && command.reload && command.respawn);
        assert_eq!(command.mode_request, Some(playsrc_movement::Mode::Noclip));
        assert_eq!(command.activate_entity, Some(77));
        assert_eq!(command.select_class, Some(playsrc_tf2::Class::Demoman));
        assert_eq!(command.select_team, Some(playsrc_tf2::Team::Red));
        assert_eq!(
            command.select_weapon,
            Some(playsrc_tf2::Weapon::StickybombLauncher)
        );
        bytes[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(decode_command(&bytes).is_none());
        let movement = playsrc_movement::State::from_player(
            playsrc_movement::Player {
                position: [1., 2., 3.],
                velocity: [4., 5., 6.],
                grounded: true,
                crouched: false,
                jump_latched: true,
            },
            playsrc_movement::Policy::default(),
        );
        let projectile = playsrc_tf2::Projectile {
            identity: 12,
            kind: playsrc_tf2::ProjectileKind::Rocket,
            team: playsrc_tf2::Team::Blue,
            owner_identity: 1,
            launcher_identity: 2,
            state: playsrc_tf2::ProjectileState::Flying,
            position: [7., 8., 9.],
            velocity: [10., 11., 12.],
            orientation: [0., 0., 0., 1.],
            angular_velocity: [0.; 3],
            contact_normal: None,
            age_seconds: 0.5,
        };
        let snapshot = playsrc_tf2::Snapshot {
            tick: 9,
            class: playsrc_tf2::Class::Soldier,
            team: playsrc_tf2::Team::Blue,
            weapon: playsrc_tf2::Weapon::Original,
            movement,
            health: 175.,
            maximum_health: 200.,
            loadout: vec![playsrc_tf2::WeaponState {
                weapon: playsrc_tf2::Weapon::Original,
                clip: 3,
                reserve: 20,
                maximum_clip: 4,
                maximum_reserve: 20,
                reload: playsrc_tf2::ReloadState::Idle,
                next_primary_tick: 20,
                next_reload_tick: 0,
            }],
            conditions: 0,
            projectiles: vec![projectile.clone()],
            projectile_events: vec![playsrc_tf2::ProjectileEvent {
                kind: playsrc_tf2::ProjectileEventKind::Explode,
                projectile: 12,
                projectile_kind: playsrc_tf2::ProjectileKind::Rocket,
                owner_identity: 1,
                launcher_identity: 2,
                team: playsrc_tf2::Team::Blue,
                tick: 9,
                position: projectile.position,
                orientation: projectile.orientation,
                contact_normal: None,
            }],
            entity_transforms: Vec::new(),
            entity_events: Vec::new(),
            jump: None,
            events: vec![playsrc_tf2::Event::Teleported {
                trigger: 20,
                destination: 21,
                position: [13., 14., 15.],
                yaw_degrees: Some(90.),
            }],
        };
        let encoded = encode_snapshot(&snapshot).unwrap();
        assert_eq!(&encoded[..8], b"PSSN\x03\0\0\0");
        assert_eq!(encoded.len(), 368);
        assert_eq!(u32::from_le_bytes(encoded[32..36].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[36..40].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[40..44].try_into().unwrap()), 1);
        assert_eq!(&encoded[192..196], &[12, 0, 0, 0]);
        assert_eq!(&encoded[276..280], &[6, 1, 2, 0]);
    }
}
