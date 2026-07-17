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
    presentation: Vec<u8>,
    presentation_hash: [u8; 32],
    particles: Option<playsrc_particle::ParticleWorld>,
    particle_materials: Vec<String>,
    particle_output: Vec<u8>,
    visibility: Option<playsrc_visibility::World>,
    area_state: Option<playsrc_visibility::AreaState>,
    visibility_output: Vec<u8>,
    collision: Option<Arc<playsrc_collision::World>>,
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
        let presentation =
            compile_presentation(&canonical, &bsp, &entity_graph, configuration, profile)
                .map_err(|_| 9_u32)?;
        let presentation_hash: [u8; 32] = Sha256::digest(&presentation).into();
        let (particles, particle_materials) =
            compile_particles(configuration).map_err(|_| 10_u32)?;
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
        let visibility = runtime.visibility;
        let area_state = playsrc_visibility::AreaState::new(&visibility);
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
        let mut session =
            playsrc_tf2::Session::new(SharedWorld(collision.clone()), spawn.position, map);
        session.set_movement_modifiers(playsrc_tf2::MovementModifiers {
            noclip_allowed: true,
            ..playsrc_tf2::MovementModifiers::default()
        });
        Ok((
            runtime.descriptor.payload,
            runtime.descriptor.payload_sha256,
            runtime.descriptor.derived_sha256,
            presentation,
            presentation_hash,
            particles,
            particle_materials,
            visibility,
            area_state,
            collision,
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
        Ok((
            payload,
            hash,
            derived_hash,
            presentation,
            presentation_hash,
            particles,
            particle_materials,
            visibility,
            area_state,
            collision,
            bsp_hash,
            spawn,
            session,
        )) => Slot {
            generation,
            payload: Some(payload),
            presentation,
            presentation_hash,
            particles: Some(particles),
            particle_materials,
            particle_output: Vec::new(),
            visibility: Some(visibility),
            area_state: Some(area_state),
            visibility_output: Vec::new(),
            collision: Some(collision),
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
            presentation: Vec::new(),
            presentation_hash: [0; 32],
            particles: None,
            particle_materials: Vec::new(),
            particle_output: Vec::new(),
            visibility: None,
            area_state: None,
            visibility_output: Vec::new(),
            collision: None,
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
pub extern "C" fn playsrc_presentation_length(handle: u32) -> usize {
    with(handle, |slot| slot.presentation.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_presentation_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.presentation.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.presentation.as_ptr(),
                pointer,
                slot.presentation.len(),
            )
        };
        slot.presentation.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify at least 32 writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_presentation_hash(handle: u32, pointer: *mut u8) -> u32 {
    with(handle, |slot| {
        unsafe { std::ptr::copy_nonoverlapping(slot.presentation_hash.as_ptr(), pointer, 32) };
        1
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify `length` readable transaction bytes.
pub unsafe extern "C" fn playsrc_particle_transact(
    handle: u32,
    pointer: *const u8,
    length: usize,
) -> u32 {
    if length == 0 || length > 4 * 1024 * 1024 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Ok((events, request)) = decode_particle_transaction(bytes) else {
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
    let Some(collision_world) = slot.collision.clone() else {
        return 0;
    };
    let Some(world) = slot.particles.as_mut() else {
        return 0;
    };
    let mut collision = ParticleCollision(collision_world);
    let Ok((items, _)) = world.advance(&events, request, &mut collision) else {
        return 0;
    };
    let Ok(output) =
        playsrc_particle::encode_render_output(&items, &slot.particle_materials, 64 * 1024 * 1024)
    else {
        return 0;
    };
    slot.particle_output = output;
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_particle_output_length(handle: u32) -> usize {
    with(handle, |slot| slot.particle_output.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable bytes of at least `capacity`.
pub unsafe extern "C" fn playsrc_particle_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.particle_output.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.particle_output.as_ptr(),
                pointer,
                slot.particle_output.len(),
            )
        };
        slot.particle_output.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify three readable finite f32 values.
pub unsafe extern "C" fn playsrc_visibility_query(handle: u32, pointer: *const f32) -> u32 {
    if pointer.is_null() {
        return 0;
    }
    let position = unsafe { std::slice::from_raw_parts(pointer, 3) };
    if position.iter().any(|v| !v.is_finite()) {
        return 0;
    }
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
    let (Some(world), Some(area)) = (slot.visibility.as_ref(), slot.area_state.as_ref()) else {
        return 0;
    };
    let Ok(candidates) = playsrc_visibility::CandidateSet::compile(world, 0, &[]) else {
        return 0;
    };
    let Ok(view) = world.view(
        area,
        &candidates,
        &playsrc_visibility::ViewQuery {
            origins: vec![[position[0], position[1], position[2]]],
            bypass_pvs: false,
        },
    ) else {
        return 0;
    };
    let mut output = b"PVIS".to_vec();
    output.extend_from_slice(&1u32.to_le_bytes());
    output.extend_from_slice(&view.cache_identity);
    output.extend_from_slice(&world.identity);
    output.extend_from_slice(&[u8::from(view.outside_world), view.sky as u8, 0, 0]);
    output.extend_from_slice(&(view.world_surfaces.len() as u32).to_le_bytes());
    for face in view.world_surfaces {
        output.extend_from_slice(&u32::from(face).to_le_bytes());
    }
    slot.visibility_output = output;
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_visibility_output_length(handle: u32) -> usize {
    with(handle, |slot| slot.visibility_output.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable bytes of at least `capacity`.
pub unsafe extern "C" fn playsrc_visibility_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.visibility_output.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.visibility_output.as_ptr(),
                pointer,
                slot.visibility_output.len(),
            )
        };
        slot.visibility_output.len()
    })
    .unwrap_or(0)
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
    slot.presentation.clear();
    slot.presentation_hash = [0; 32];
    slot.particles = None;
    slot.particle_materials.clear();
    slot.particle_output.clear();
    slot.visibility = None;
    slot.area_state = None;
    slot.visibility_output.clear();
    slot.collision = None;
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
        "models/player/soldier.mdl".to_owned(),
        "models/player/demo.mdl".to_owned(),
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl".to_owned(),
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl".to_owned(),
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
        if document.skins.is_empty() {
            return Err(());
        }
        indexes.insert(identity.clone(), models.len());
        let retained_skins =
            if identity.ends_with("w_rocket.mdl") || identity.ends_with("w_stickybomb.mdl") {
                2
            } else {
                1
            };
        for (skin_index, skin) in document.skins.iter().take(retained_skins).enumerate() {
            let mut primitives = Vec::new();
            for primitive in document
                .geometry
                .iter()
                .filter(|primitive| primitive.lod == 0 && primitive.model == 0)
            {
                let material = skin
                    .texture_indices
                    .get(primitive.material_slot)
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
            models.push(playsrc_map::RuntimeModel {
                logical_path: if skin_index == 0 {
                    identity.clone()
                } else {
                    format!("{identity}#skin={skin_index}")
                },
                materials: materials.clone(),
                primitives,
            });
        }
        if identity.contains("/v_models/") {
            append_viewmodel_activity_models(&identity, &bundle, profile, &materials, &mut models)?;
        }
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

fn studio_texture_role(
    role: playsrc_material::TextureRole,
) -> Option<playsrc_studio_model::TextureRole> {
    use playsrc_material::TextureRole as S;
    use playsrc_studio_model::TextureRole as T;
    Some(match role {
        S::Base => T::Base,
        S::HdrBase => T::HdrBase,
        S::HdrCompressed => T::HdrCompressed,
        S::HdrCompressed0 => T::HdrCompressed0,
        S::HdrCompressed1 => T::HdrCompressed1,
        S::HdrCompressed2 => T::HdrCompressed2,
        S::Base2 => T::Base2,
        S::Bump => T::Bump,
        S::Normal => T::Normal,
        S::Bump2 => T::Bump2,
        S::Detail => T::Detail,
        S::BlendModulate => T::BlendModulate,
        S::Environment => T::Environment,
        S::EnvironmentMask => T::EnvironmentMask,
        S::SelfIllumMask => T::SelfIllumMask,
        S::Flow => T::Flow,
        S::Reflection | S::Refraction => return None,
    })
}
fn studio_manifest(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    profile: playsrc_map::LightingProfile,
) -> Result<playsrc_studio_model::MaterialResolutionManifest, ()> {
    let identity = identity.to_ascii_lowercase();
    let root = *bundle.get(&identity).ok_or(())?;
    let mut responses = Vec::new();
    let mut include_sources = Vec::new();
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
            playsrc_vmt::Composition::Complete(doc) => {
                break playsrc_material::resolve_for_environment(
                    &doc,
                    material_environment(profile, true),
                )
                .map_err(|_| ())?;
            }
            playsrc_vmt::Composition::Needs(requests) => {
                for request in requests {
                    let path = dependency_path(&request.target_token)?;
                    include_sources.push(playsrc_studio_model::MaterialSourceManifest {
                        requester: request.parent_identity.clone(),
                        logical_path: path.clone(),
                    });
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
    let textures = material
        .textures
        .iter()
        .filter_map(|t| {
            Some(playsrc_studio_model::MaterialTextureManifest {
                role: studio_texture_role(t.role)?,
                parameter: t.parameter.clone(),
                logical_path: t.logical_path.as_ref().map(|v| v.to_ascii_lowercase()),
                disposition: match t.disposition {
                    playsrc_material::TextureDisposition::Source => {
                        playsrc_studio_model::TextureDisposition::Source
                    }
                    playsrc_material::TextureDisposition::BuiltInEnvironment => {
                        playsrc_studio_model::TextureDisposition::BuiltInEnvironment
                    }
                    playsrc_material::TextureDisposition::BuiltInRenderTarget => {
                        playsrc_studio_model::TextureDisposition::BuiltInRenderTarget
                    }
                },
                selected: material.selected_textures.contains(&t.role),
            })
        })
        .collect();
    Ok(playsrc_studio_model::MaterialResolutionManifest {
        root_identity: identity,
        include_sources,
        textures,
    })
}
fn build_model_presentation(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    profile: playsrc_map::LightingProfile,
) -> Result<Box<playsrc_studio_model::PresentationArtifact>, ()> {
    let document = load_model(identity, bundle)?;
    let mut responses = Vec::new();
    loop {
        match playsrc_studio_model::build_presentation(
            &document,
            if identity.contains("/v_models/") {
                playsrc_studio_model::PresentationProfile::ViewModel
            } else {
                playsrc_studio_model::PresentationProfile::World
            },
            &responses,
            playsrc_studio_model::PresentationLimits::default(),
            &playsrc_studio_model::CancellationToken::default(),
        )
        .map_err(|_| ())?
        {
            playsrc_studio_model::PresentationBuild::Complete(a) => return Ok(a),
            playsrc_studio_model::PresentationBuild::Needs(requests) => {
                for r in requests {
                    let bytes = bundle
                        .get(&r.logical_path.to_ascii_lowercase())
                        .map(|b| b.to_vec());
                    let material = if r.role
                        == playsrc_studio_model::PresentationDependencyRole::MaterialCandidate
                        && bytes.is_some()
                    {
                        Some(studio_manifest(&r.logical_path, bundle, profile)?)
                    } else {
                        None
                    };
                    let sha256 = bytes.as_deref().map(|b| Sha256::digest(b).into());
                    responses.push(playsrc_studio_model::PresentationDependencyResponse {
                        requester: r.requester,
                        role: r.role,
                        logical_path: r.logical_path,
                        material_slot: r.material_slot,
                        texture_role: r.texture_role,
                        bytes,
                        sha256,
                        material,
                    });
                }
            }
        }
    }
}

fn transform_point(matrix: &playsrc_studio_model::Matrix3x4, point: [f32; 3]) -> [f32; 3] {
    let m = matrix.0.map(|value| f32::from_bits(value.0));
    [
        m[0].mul_add(
            point[0],
            m[1].mul_add(point[1], m[2].mul_add(point[2], m[3])),
        ),
        m[4].mul_add(
            point[0],
            m[5].mul_add(point[1], m[6].mul_add(point[2], m[7])),
        ),
        m[8].mul_add(
            point[0],
            m[9].mul_add(point[1], m[10].mul_add(point[2], m[11])),
        ),
    ]
}
fn transform_normal(matrix: &playsrc_studio_model::Matrix3x4, normal: [f32; 3]) -> [f32; 3] {
    let m = matrix.0.map(|value| f32::from_bits(value.0));
    [
        m[0].mul_add(normal[0], m[1].mul_add(normal[1], m[2] * normal[2])),
        m[4].mul_add(normal[0], m[5].mul_add(normal[1], m[6] * normal[2])),
        m[8].mul_add(normal[0], m[9].mul_add(normal[1], m[10] * normal[2])),
    ]
}
fn append_viewmodel_activity_models(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    profile: playsrc_map::LightingProfile,
    materials: &[playsrc_map::RuntimeMaterial],
    output: &mut Vec<playsrc_map::RuntimeModel>,
) -> Result<(), ()> {
    let artifact = build_model_presentation(identity, bundle, profile)?;
    let bodygroups = artifact
        .model
        .body_parts
        .iter()
        .map(|_| 0usize)
        .collect::<Vec<_>>();
    for activity in [
        "ACT_VM_DRAW",
        "ACT_VM_IDLE",
        "ACT_VM_PRIMARYATTACK",
        "ACT_VM_RELOAD",
    ] {
        let Some(sequence) =
            playsrc_studio_model::sequences_for_activity_name(&artifact.model, activity.as_bytes())
                .first()
                .copied()
        else {
            continue;
        };
        for (sample, cycle) in [0.0f32, 0.25, 0.5, 0.75, 1.0].into_iter().enumerate() {
            let pose = playsrc_studio_model::sample_pose(
                &artifact.model,
                &playsrc_studio_model::AnimationState {
                    base_sequence: sequence,
                    cycle: playsrc_studio_model::Float32(cycle.to_bits()),
                    pose_parameters: artifact
                        .model
                        .pose_parameters
                        .iter()
                        .map(|_| playsrc_studio_model::Float32(0))
                        .collect(),
                    layers: Vec::new(),
                },
            )
            .map_err(|_| ())?;
            let selected =
                playsrc_studio_model::select_primitives(&artifact.model, &bodygroups, 0, 0)
                    .map_err(|_| ())?;
            let mut primitives = Vec::new();
            for selected in selected {
                let source = &artifact.model.geometry[selected.primitive];
                let material = artifact.model.materials[selected.material].source_slot;
                let mut positions = Vec::with_capacity(source.vertices.len());
                let mut normals = Vec::with_capacity(source.vertices.len());
                for vertex in &source.vertices {
                    let point = vertex.position.0.map(|v| f32::from_bits(v.0));
                    let normal = vertex.normal.0.map(|v| f32::from_bits(v.0));
                    let mut p = [0.0; 3];
                    let mut n = [0.0; 3];
                    for (index, weight) in vertex.weights.iter().enumerate() {
                        let weight = f32::from_bits(weight.0);
                        if weight == 0.0 {
                            continue;
                        }
                        let bone = *vertex.bones.get(index).ok_or(())? as usize;
                        let tp =
                            transform_point(pose.skinning_matrices.get(bone).ok_or(())?, point);
                        let tn =
                            transform_normal(pose.skinning_matrices.get(bone).ok_or(())?, normal);
                        for axis in 0..3 {
                            p[axis] = tp[axis].mul_add(weight, p[axis]);
                            n[axis] = tn[axis].mul_add(weight, n[axis])
                        }
                    }
                    positions.push(p);
                    normals.push(n)
                }
                primitives.push(playsrc_map::RuntimeModelPrimitive {
                    material,
                    positions,
                    normals,
                    uv: source
                        .vertices
                        .iter()
                        .map(|v| v.uv.map(|x| f32::from_bits(x.0)))
                        .collect(),
                    triangles: source.triangles.clone(),
                })
            }
            output.push(playsrc_map::RuntimeModel {
                logical_path: format!("{identity}#activity={activity}#sample={sample}"),
                materials: materials.to_vec(),
                primitives,
            })
        }
    }
    Ok(())
}
fn pbytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ()> {
    out.extend_from_slice(&u32::try_from(bytes.len()).map_err(|_| ())?.to_le_bytes());
    out.extend_from_slice(bytes);
    (out.len() <= 512 * 1024 * 1024).then_some(()).ok_or(())
}
fn decoded_texture(
    path: &str,
    bundle: &BTreeMap<String, &[u8]>,
) -> Result<playsrc_map::RuntimeTexture, ()> {
    let plane = playsrc_vtf::decode(
        *bundle.get(path).ok_or(())?,
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
            let mut output = Vec::with_capacity(plane.width as usize * plane.height as usize * 4);
            for pixel in plane.samples.chunks_exact(3) {
                output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255])
            }
            output
        }
    };
    Ok(playsrc_map::RuntimeTexture {
        logical_path: path.to_owned(),
        width: plane.width,
        height: plane.height,
        rgba,
    })
}
fn compile_presentation(
    canonical: &playsrc_map::CanonicalMap,
    bsp: &playsrc_bsp::Bsp,
    graph: &playsrc_entity::Graph,
    configuration: &[u8],
    profile: playsrc_map::LightingProfile,
) -> Result<Vec<u8>, ()> {
    let bundle = bundle(configuration)?;
    let mut roots = std::collections::BTreeSet::from([
        "models/weapons/w_models/w_rocket.mdl".to_owned(),
        "models/weapons/w_models/w_stickybomb.mdl".to_owned(),
        "models/weapons/v_models/v_rocketlauncher_soldier.mdl".to_owned(),
        "models/weapons/v_models/v_stickybomb_launcher_demo.mdl".to_owned(),
        "models/player/soldier.mdl".to_owned(),
        "models/player/demo.mdl".to_owned(),
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl".to_owned(),
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl".to_owned(),
    ]);
    for e in &graph.entities {
        if e.classname
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case(b"prop_dynamic"))
            && let Some(m) = &e.model
        {
            roots.insert(std::str::from_utf8(m).map_err(|_| ())?.to_ascii_lowercase());
        }
    }
    let mut models = Vec::new();
    for id in roots {
        models.push((id.clone(), build_model_presentation(&id, &bundle, profile)?));
    }
    let mut textures = BTreeMap::new();
    for (_, a) in &models {
        for m in &a.model.materials {
            let path = a
                .model
                .dependencies
                .get(m.material_dependency)
                .ok_or(())?
                .logical_path
                .to_ascii_lowercase();
            let resolved =
                resolve_material(&path, &bundle, true, material_environment(profile, true))?;
            if let Some(texture) = resolved.base_texture {
                textures.insert(path, texture);
            }
        }
    }
    let mut directional = Vec::new();
    for reference in &canonical.materials {
        let identity = reference.logical_path.to_ascii_lowercase();
        let material =
            resolve_material_semantics(&identity, &bundle, material_environment(profile, false))?;
        let Some(texture) = material.textures.iter().find(|texture| {
            matches!(
                texture.role,
                playsrc_material::TextureRole::Bump | playsrc_material::TextureRole::Normal
            ) && texture.disposition == playsrc_material::TextureDisposition::Source
        }) else {
            continue;
        };
        let path = texture
            .logical_path
            .as_ref()
            .ok_or(())?
            .to_ascii_lowercase();
        directional.push((
            identity,
            u8::from(material.features.ss_bump),
            decoded_texture(&path, &bundle)?,
        ));
    }
    let particle_paths = [
        "particles/rockettrail.pcf",
        "particles/rocketbackblast.pcf",
        "particles/stickybomb.pcf",
        "particles/muzzle_flash.pcf",
        "particles/explosion.pcf",
    ];
    let particle_sources = particle_paths
        .iter()
        .map(|path| {
            Ok(playsrc_particle::PcfSource {
                logical_path: path,
                bytes: bundle.get(*path).ok_or(())?,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let particle_registry = playsrc_particle::Registry::from_pcf(
        &particle_sources,
        playsrc_particle::RegistryLimits::default(),
    )
    .map_err(|_| ())?;
    let particle_roots = [
        "rockettrail",
        "rocketbackblast",
        "stickybombtrail_red",
        "stickybombtrail_blue",
        "stickybomb_pulse_red",
        "stickybomb_pulse_blue",
        "muzzle_pipelauncher",
        "ExplosionCore_Wall",
        "ExplosionCore_MidAir",
    ]
    .map(playsrc_particle::DefinitionLookup::Name);
    let particle_materials = particle_registry
        .target_closure(&particle_roots)
        .map_err(|_| ())?
        .materials;
    let environment = compile_environment_artifact(canonical, bsp, graph, &bundle, profile)?;
    let mut out = b"PTF2".to_vec();
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(models.len()).map_err(|_| ())?.to_le_bytes());
    out.extend_from_slice(&u32::try_from(textures.len()).map_err(|_| ())?.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(directional.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u32::try_from(particle_materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (id, a) in models {
        pbytes(&mut out, id.as_bytes())?;
        out.extend_from_slice(&[
            if a.model.profile == playsrc_studio_model::PresentationProfile::World {
                0
            } else {
                1
            },
            0,
            0,
            0,
        ]);
        out.extend_from_slice(&a.sha256);
        out.extend_from_slice(
            &u32::try_from(a.model.skins.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(a.model.body_parts.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for p in &a.model.body_parts {
            out.extend_from_slice(
                &u32::try_from(p.model_names.len())
                    .map_err(|_| ())?
                    .to_le_bytes(),
            );
        }
        out.extend_from_slice(
            &u32::try_from(a.model.attachments.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for v in &a.model.attachments {
            pbytes(&mut out, &v.name)?;
        }
        out.extend_from_slice(
            &u32::try_from(a.model.sequences.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for v in &a.model.sequences {
            pbytes(&mut out, &v.label)?;
            pbytes(&mut out, &v.activity_name)?;
        }
        pbytes(&mut out, &a.bytes)?;
    }
    for (material, texture) in textures {
        pbytes(&mut out, material.as_bytes())?;
        pbytes(&mut out, texture.logical_path.as_bytes())?;
        out.extend_from_slice(&texture.width.to_le_bytes());
        out.extend_from_slice(&texture.height.to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(&texture.rgba).into();
        out.extend_from_slice(&hash);
        pbytes(&mut out, &texture.rgba)?;
    }
    for (material, kind, texture) in directional {
        pbytes(&mut out, material.as_bytes())?;
        out.extend_from_slice(&[kind, 0, 0, 0]);
        pbytes(&mut out, texture.logical_path.as_bytes())?;
        out.extend_from_slice(&texture.width.to_le_bytes());
        out.extend_from_slice(&texture.height.to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(&texture.rgba).into();
        out.extend_from_slice(&hash);
        pbytes(&mut out, &texture.rgba)?;
        for value in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0] {
            out.extend_from_slice(&value.to_le_bytes())
        }
    }
    for material in particle_materials {
        pbytes(&mut out, material.as_bytes())?;
    }
    pbytes(&mut out, &environment)?;
    Ok(out)
}

fn compile_environment_artifact(
    canonical: &playsrc_map::CanonicalMap,
    bsp: &playsrc_bsp::Bsp,
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    profile: playsrc_map::LightingProfile,
) -> Result<Vec<u8>, ()> {
    let materials = canonical
        .materials
        .iter()
        .map(|r| {
            resolve_material_semantics(
                &r.logical_path.to_ascii_lowercase(),
                bundle,
                material_environment(profile, false),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bindings = canonical
        .materials
        .iter()
        .zip(&materials)
        .map(|(reference, material)| playsrc_map::MaterialBinding {
            material_index: reference.index,
            material,
        })
        .collect::<Vec<_>>();
    let world = graph
        .entities
        .iter()
        .find(|e| {
            e.classname
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case(b"worldspawn"))
        })
        .ok_or(())?;
    let sky = std::str::from_utf8(
        &world
            .pairs
            .iter()
            .find(|p| p.key.eq_ignore_ascii_case(b"skyname"))
            .ok_or(())?
            .value,
    )
    .map_err(|_| ())?;
    let mut dependencies = Vec::new();
    for (face, face_suffix) in [
        (playsrc_map::CubeFace::Right, "rt"),
        (playsrc_map::CubeFace::Left, "lf"),
        (playsrc_map::CubeFace::Back, "bk"),
        (playsrc_map::CubeFace::Front, "ft"),
        (playsrc_map::CubeFace::Up, "up"),
        (playsrc_map::CubeFace::Down, "dn"),
    ] {
        let suffix = if profile == playsrc_map::LightingProfile::Hdr {
            "_hdr"
        } else {
            ""
        };
        let path = format!("materials/skybox/{sky}{suffix}{face_suffix}.vmt");
        let source = *bundle.get(&path).ok_or(())?;
        let m = resolve_material_semantics(&path, bundle, material_environment(profile, false))?;
        let selected_textures = m
            .textures
            .iter()
            .filter(|t| {
                m.selected_textures.contains(&t.role)
                    && t.disposition == playsrc_material::TextureDisposition::Source
            })
            .map(|t| {
                let logical_path = t.logical_path.as_ref().ok_or(())?.to_ascii_lowercase();
                Ok(playsrc_map::ResolvedTexture {
                    sha256: Sha256::digest(*bundle.get(&logical_path).ok_or(())?).into(),
                    logical_path,
                })
            })
            .collect::<Result<Vec<_>, ()>>()?;
        dependencies.push(playsrc_map::DependencyResponse {
            request: playsrc_map::DependencyRequest {
                role: playsrc_map::DependencyRole::SkyMaterial(face),
                profile,
                logical_path: path,
            },
            metadata: playsrc_map::DependencyMetadata::SkyMaterial {
                source_sha256: Sha256::digest(source).into(),
                selected_textures,
            },
        })
    }
    let records = match &bsp.lumps[42].records {
        playsrc_bsp::LumpData::Cubemaps(v) => v.as_slice(),
        _ => return Err(()),
    };
    let first = records.first().ok_or(())?;
    let profile_suffix = if profile == playsrc_map::LightingProfile::Hdr {
        ".hdr"
    } else {
        ""
    };
    let leaf = format!(
        "/c{}_{}_{}{profile_suffix}.vtf",
        first.origin[0], first.origin[1], first.origin[2]
    );
    let mut map_names = bundle
        .keys()
        .filter_map(|path| path.strip_prefix("materials/maps/")?.strip_suffix(&leaf))
        .collect::<std::collections::BTreeSet<_>>();
    if map_names.len() != 1 {
        return Err(());
    }
    let map_name = map_names.pop_first().ok_or(())?.to_owned();
    let logical_map_path = format!("maps/{map_name}.bsp");
    for (index, r) in records.iter().enumerate() {
        let suffix = if profile == playsrc_map::LightingProfile::Hdr {
            ".hdr"
        } else {
            ""
        };
        let path = format!(
            "materials/maps/{map_name}/c{}_{}_{}{suffix}.vtf",
            r.origin[0], r.origin[1], r.origin[2]
        );
        let bytes = *bundle.get(&path).ok_or(())?;
        let m = playsrc_vtf::inspect(
            bytes,
            playsrc_vtf::Dialect::Source2013Pc,
            playsrc_vtf::Limits::default(),
        )
        .map_err(|_| ())?;
        dependencies.push(playsrc_map::DependencyResponse {
            request: playsrc_map::DependencyRequest {
                role: playsrc_map::DependencyRole::CubemapTexture { sample: index },
                profile,
                logical_path: path,
            },
            metadata: playsrc_map::DependencyMetadata::CubemapTexture {
                source_sha256: Sha256::digest(bytes).into(),
                width: m.width,
                height: m.height,
                mip_count: m.mip_count,
                source_face_count: m.faces.len() as u8,
            },
        })
    }
    let mut marks = BTreeMap::new();
    for e in &graph.entities {
        if !e
            .classname
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case(b"infodecal"))
        {
            continue;
        }
        let Some(reference) = e
            .pairs
            .iter()
            .find(|p| p.key.eq_ignore_ascii_case(b"texture"))
            .map(|p| p.value.clone())
        else {
            continue;
        };
        let path = dependency_path(&reference)?;
        let Some(source) = bundle.get(&path).copied() else {
            continue;
        };
        let m = resolve_material_semantics(&path, bundle, material_environment(profile, false))?;
        let texture = m
            .textures
            .iter()
            .find(|t| {
                m.selected_textures.contains(&t.role)
                    && t.disposition == playsrc_material::TextureDisposition::Source
            })
            .ok_or(())?;
        let tm = playsrc_vtf::inspect(
            bundle
                .get(
                    &texture
                        .logical_path
                        .as_ref()
                        .ok_or(())?
                        .to_ascii_lowercase(),
                )
                .ok_or(())?,
            playsrc_vtf::Dialect::Source2013Pc,
            playsrc_vtf::Limits::default(),
        )
        .map_err(|_| ())?;
        marks
            .entry(reference.to_ascii_lowercase())
            .or_insert(playsrc_map::MarkMaterial {
                reference,
                logical_path: path,
                source_sha256: Sha256::digest(source).into(),
                width: tm.width,
                height: tm.height,
                state: m.decal,
            });
    }
    let marks = marks.into_values().collect::<Vec<_>>();
    let visibility = playsrc_visibility::compile(bsp).map_err(|_| ())?;
    let env = playsrc_map::compile_environment(
        canonical,
        bsp,
        playsrc_map::EnvironmentInputs {
            logical_map_path: &logical_map_path,
            entities: graph,
            visibility: &visibility,
            materials: &bindings,
            mark_materials: &marks,
            dependencies: &dependencies,
            limits: playsrc_map::EnvironmentLimits::default(),
        },
    )
    .map_err(|_| ())?;
    let mut out = b"PENV".to_vec();
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&[
        if profile == playsrc_map::LightingProfile::Hdr {
            1
        } else {
            0
        },
        0,
        0,
        0,
    ]);
    out.extend_from_slice(&visibility.identity);
    for value in [
        visibility.cluster_count,
        visibility.nodes.len(),
        visibility.leaves.len(),
        env.sky.as_ref().map_or(0, |s| s.surface_faces.len()),
        env.cubemaps.len(),
        env.water.surfaces.len(),
        env.water.volumes.len(),
        env.marks.records.len(),
        env.marks.fragment_count,
        env.controllers.len(),
    ] {
        out.extend_from_slice(&(value as u32).to_le_bytes())
    }
    for mark in &env.marks.records {
        out.extend_from_slice(&[
            mark.status as u8,
            mark.kind as u8,
            u8::from(mark.initially_enabled),
            u8::from(mark.dynamic),
        ]);
        pbytes(&mut out, mark.material_path.as_bytes())?;
        out.extend_from_slice(&(mark.fragments.len() as u32).to_le_bytes());
        for fragment in &mark.fragments {
            out.extend_from_slice(&(fragment.model as u32).to_le_bytes());
            out.extend_from_slice(&(fragment.face as u32).to_le_bytes());
            out.extend_from_slice(&(fragment.positions.len() as u32).to_le_bytes());
            for position in &fragment.positions {
                for value in position {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
            for normal in &fragment.normals {
                for value in normal {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
            for uv in &fragment.uv {
                for value in uv {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
            out.extend_from_slice(&(fragment.triangles.len() as u32).to_le_bytes());
            for triangle in &fragment.triangles {
                for value in triangle {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
        }
    }
    let mut decal_textures = BTreeMap::new();
    for mark in &marks {
        if let Some(texture) = resolve_material(
            &mark.logical_path,
            bundle,
            true,
            material_environment(profile, false),
        )?
        .base_texture
        {
            decal_textures.insert(mark.logical_path.clone(), texture);
        }
    }
    out.extend_from_slice(&(decal_textures.len() as u32).to_le_bytes());
    for (material, texture) in decal_textures {
        pbytes(&mut out, material.as_bytes())?;
        pbytes(&mut out, texture.logical_path.as_bytes())?;
        out.extend_from_slice(&texture.width.to_le_bytes());
        out.extend_from_slice(&texture.height.to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(&texture.rgba).into();
        out.extend_from_slice(&hash);
        pbytes(&mut out, &texture.rgba)?
    }
    if let Some(sky) = &env.sky {
        out.push(1);
        pbytes(&mut out, &sky.name)?;
        out.extend_from_slice(&(sky.faces.len() as u32).to_le_bytes());
        for face in &sky.faces {
            out.push(face.face as u8);
            pbytes(&mut out, face.material_path.as_bytes())?;
            out.extend_from_slice(&face.material_sha256)
        }
    } else {
        out.push(0)
    }
    for sample in &env.cubemaps {
        out.extend_from_slice(&(sample.index as u32).to_le_bytes());
        for value in sample.origin {
            out.extend_from_slice(&value.to_le_bytes())
        }
        pbytes(&mut out, sample.logical_path.as_bytes())?;
        out.extend_from_slice(&sample.source_sha256);
        out.extend_from_slice(&sample.width.to_le_bytes());
        out.extend_from_slice(&sample.height.to_le_bytes())
    }
    let cubemap_code = |selection: &playsrc_map::CubemapSelection| -> (u8, u32) {
        match selection {
            playsrc_map::CubemapSelection::Nearest { sample } => (0, *sample as u32),
            playsrc_map::CubemapSelection::Declared { sample } => (1, *sample as u32),
            playsrc_map::CubemapSelection::External { .. } => (2, u32::MAX),
        }
    };
    for surface in &env.water.surfaces {
        out.extend_from_slice(&[
            if surface.profile == playsrc_map::LightingProfile::Hdr {
                1
            } else {
                0
            },
            u8::from(surface.selected),
            0,
            0,
        ]);
        for value in [surface.face, surface.model, surface.material] {
            out.extend_from_slice(&(value as u32).to_le_bytes())
        }
        for bound in surface.bounds {
            for value in bound {
                out.extend_from_slice(&value.to_le_bytes())
            }
        }
        let (kind, sample) = cubemap_code(&surface.cubemap);
        out.extend_from_slice(&[kind, 0, 0, 0]);
        out.extend_from_slice(&sample.to_le_bytes())
    }
    for volume in &env.water.volumes {
        out.extend_from_slice(&(volume.index as u32).to_le_bytes());
        out.extend_from_slice(&volume.surface_z.to_le_bytes());
        out.extend_from_slice(&volume.minimum_z.to_le_bytes());
        for bound in volume.bounds {
            for value in bound {
                out.extend_from_slice(&value.to_le_bytes())
            }
        }
        out.extend_from_slice(&(volume.leaves.len() as u32).to_le_bytes());
        for leaf in &volume.leaves {
            out.extend_from_slice(&(*leaf as u32).to_le_bytes())
        }
        let (kind, sample) = cubemap_code(&volume.cubemap);
        out.extend_from_slice(&[kind, 0, 0, 0]);
        out.extend_from_slice(&sample.to_le_bytes())
    }
    for controller in &env.controllers {
        out.extend_from_slice(&(controller.entity as u32).to_le_bytes());
        pbytes(&mut out, &controller.classname)?;
        out.push(match &controller.state {
            playsrc_map::ControllerState::Fog(_) => 0,
            playsrc_map::ControllerState::SkyCamera { .. } => 1,
            playsrc_map::ControllerState::WaterLod { .. } => 2,
            playsrc_map::ControllerState::EnvironmentLight { .. } => 3,
            playsrc_map::ControllerState::Shadow { .. } => 4,
            playsrc_map::ControllerState::ToneMap => 5,
        });
    }
    Ok(out)
}

fn compile_particles(
    configuration: &[u8],
) -> Result<(playsrc_particle::ParticleWorld, Vec<String>), ()> {
    let b = bundle(configuration)?;
    let paths = [
        "particles/rockettrail.pcf",
        "particles/rocketbackblast.pcf",
        "particles/stickybomb.pcf",
        "particles/muzzle_flash.pcf",
        "particles/explosion.pcf",
    ];
    let sources = paths
        .iter()
        .map(|path| {
            Ok(playsrc_particle::PcfSource {
                logical_path: path,
                bytes: b.get(*path).ok_or(())?,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let registry =
        playsrc_particle::Registry::from_pcf(&sources, playsrc_particle::RegistryLimits::default())
            .map_err(|_| ())?;
    let roots = [
        "rockettrail",
        "rocketbackblast",
        "stickybombtrail_red",
        "stickybombtrail_blue",
        "stickybomb_pulse_red",
        "stickybomb_pulse_blue",
        "muzzle_pipelauncher",
        "ExplosionCore_Wall",
        "ExplosionCore_MidAir",
    ]
    .map(playsrc_particle::DefinitionLookup::Name);
    let materials = registry.target_closure(&roots).map_err(|_| ())?.materials;
    Ok((
        playsrc_particle::ParticleWorld::new(&registry, playsrc_particle::WorldLimits::default())
            .map_err(|_| ())?,
        materials,
    ))
}
struct ParticleCollision(Arc<playsrc_collision::World>);
impl playsrc_particle::CollisionQuery for ParticleCollision {
    fn trace_batch(
        &mut self,
        requests: &[playsrc_particle::TraceRequest],
    ) -> Result<Vec<playsrc_particle::CollisionResult>, playsrc_particle::Error> {
        Ok(requests
            .iter()
            .map(|r| {
                let radius = r.radius.max(0.0);
                let trace = self
                    .0
                    .trace_hull(
                        r.start,
                        r.end,
                        playsrc_collision::Hull {
                            mins: [-radius; 3],
                            maxs: [radius; 3],
                        },
                        u32::MAX,
                    )
                    .expect("validated particle collision query");
                playsrc_particle::CollisionResult {
                    identity: r.identity,
                    fraction: trace.fraction,
                    start_solid: trace.start_solid,
                    normal: trace.plane.map_or([0.0, 0.0, 1.0], |plane| plane.normal),
                }
            })
            .collect())
    }
}
struct ParticleReader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> ParticleReader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ()> {
        let end = self.at.checked_add(n).ok_or(())?;
        let v = self.bytes.get(self.at..end).ok_or(())?;
        self.at = end;
        Ok(v)
    }
    fn u8(&mut self) -> Result<u8, ()> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, ()> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| ())?,
        ))
    }
    fn u64(&mut self) -> Result<u64, ()> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| ())?,
        ))
    }
    fn f32(&mut self) -> Result<f32, ()> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().map_err(|_| ())?);
        v.is_finite().then_some(v).ok_or(())
    }
    fn text(&mut self) -> Result<String, ()> {
        let n = self.u32()? as usize;
        if n > 1024 {
            return Err(());
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| ())
    }
    fn cp(&mut self) -> Result<playsrc_particle::ControlPoint, ()> {
        let position = [self.f32()?, self.f32()?, self.f32()?];
        let orientation = [self.f32()?, self.f32()?, self.f32()?, self.f32()?];
        Ok(playsrc_particle::ControlPoint {
            index: 0,
            position,
            previous_position: position,
            orientation,
            velocity: [0.0; 3],
            parent: None,
            object_identity: match self.u32()? {
                u32::MAX => None,
                v => Some(v),
            },
        })
    }
}
fn decode_particle_transaction(
    bytes: &[u8],
) -> Result<
    (
        Vec<playsrc_particle::Event>,
        playsrc_particle::AdvanceRequest,
    ),
    (),
> {
    let mut r = ParticleReader { bytes, at: 0 };
    if r.take(4)? != b"PPTX" || r.u32()? != 1 {
        return Err(());
    }
    let from = r.f32()?;
    let to = r.f32()?;
    let camera_position = [r.f32()?, r.f32()?, r.f32()?];
    let count = r.u32()? as usize;
    if count > 4096 || to < from {
        return Err(());
    }
    let mut events = Vec::with_capacity(count);
    for order in 0..count {
        let kind = r.u8()?;
        if r.take(3)? != [0, 0, 0] {
            return Err(());
        }
        let identity = r.u64()?;
        let timestamp_seconds = r.f32()?;
        let effect_identity = r.u32()?;
        let command = match kind {
            1 => {
                let seed = r.u64()?;
                let owner = match r.u32()? {
                    u32::MAX => None,
                    v => Some(v),
                };
                let definition = r.text()?;
                let cp = r.cp()?;
                playsrc_particle::EventCommand::Create {
                    effect_identity,
                    definition,
                    seed,
                    owner_identity: owner,
                    control_points: vec![cp],
                }
            }
            2 => playsrc_particle::EventCommand::SetControlPoint {
                effect_identity,
                control_point: r.cp()?,
            },
            3 => playsrc_particle::EventCommand::StopEmission {
                effect_identity,
                mode: playsrc_particle::StopMode::Immediate,
            },
            4 => playsrc_particle::EventCommand::Destroy { effect_identity },
            _ => return Err(()),
        };
        events.push(playsrc_particle::Event {
            identity,
            timestamp_seconds,
            source_order: order as u32,
            command,
        });
    }
    if r.at != bytes.len() {
        return Err(());
    }
    Ok((
        events,
        playsrc_particle::AdvanceRequest {
            from_seconds: from,
            to_seconds: to,
            maximum_step_seconds: 1.0 / 60.0,
            camera_position,
        },
    ))
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
            presentation: Vec::new(),
            presentation_hash: [0; 32],
            particles: None,
            particle_materials: Vec::new(),
            particle_output: Vec::new(),
            visibility: None,
            area_state: None,
            visibility_output: Vec::new(),
            collision: None,
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
            presentation: Vec::new(),
            presentation_hash: [0; 32],
            particles: None,
            particle_materials: Vec::new(),
            particle_output: Vec::new(),
            visibility: None,
            area_state: None,
            visibility_output: Vec::new(),
            collision: None,
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
