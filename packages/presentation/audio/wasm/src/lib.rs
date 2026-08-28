//! Main-thread audio computation. The browser supplies its existing random
//! stream synchronously; geometry is returned by the map Worker's transaction.
use playsrc_audio::{
    dsp::{Error, Presets},
    mixers::Mixers,
    playback::{Clip, Envelope, Playback, Start},
    soundscape::{Listener, Random, Registry, Selection},
    wire::{self, Reader, SceneRequest},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "playsrc_audio")]
unsafe extern "C" {
    fn random_float(low: f32, high: f32) -> f32;
    fn random_integer(low: i32, high: i32) -> i32;
}
struct InstalledRandom;
impl Random for InstalledRandom {
    fn float(&mut self, low: f32, high: f32) -> f32 {
        #[cfg(target_arch = "wasm32")]
        {
            unsafe { random_float(low, high) }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (low, high);
            panic!("audio host random stream is required")
        }
    }
    fn integer(&mut self, low: i32, high: i32) -> i32 {
        #[cfg(target_arch = "wasm32")]
        {
            unsafe { random_integer(low, high) }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (low, high);
            panic!("audio host random stream is required")
        }
    }
}

#[derive(Clone)]
struct Resource {
    hash: [u8; 32],
    clip: Arc<Clip>,
}
#[derive(Default)]
struct State {
    playback: Option<Playback>,
    resources: BTreeMap<Vec<u8>, Resource>,
    staged: BTreeMap<Vec<u8>, Resource>,
    documents: BTreeMap<String, Vec<u8>>,
    world: [u8; 32],
    sequence: u64,
    pending: bool,
    output: Vec<u8>,
    active: Vec<u32>,
    error: String,
    room_info: [f32; 12],
}
fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}
fn run(operation: impl FnOnce(&mut State) -> Result<(), Error>) -> u32 {
    let mut state = state().lock().expect("audio instance");
    match operation(&mut state) {
        Ok(()) => {
            state.error.clear();
            1
        }
        Err(error) => {
            state.error = error.to_string();
            0
        }
    }
}
unsafe fn input<'a>(pointer: *const u8, length: usize, maximum: usize) -> Option<&'a [u8]> {
    if pointer.is_null() || length > maximum {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(pointer, length) })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_alloc(length: usize) -> *mut u8 {
    if length > 32 * 1024 * 1024 {
        return std::ptr::null_mut();
    }
    let mut bytes = vec![0_u8; length].into_boxed_slice();
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}
#[unsafe(no_mangle)]
/// # Safety
/// Return the exact allocation and length from `playsrc_audio_alloc` once.
pub unsafe extern "C" fn playsrc_audio_free(pointer: *mut u8, length: usize) {
    if !pointer.is_null() {
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, length)) });
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_stage() -> u32 {
    run(|state| {
        state.staged.clear();
        state.documents.clear();
        Ok(())
    })
}
#[unsafe(no_mangle)]
/// # Safety
/// Both pairs must identify readable module allocations.
pub unsafe extern "C" fn playsrc_audio_resource(
    name: *const u8,
    name_length: usize,
    bytes: *const u8,
    length: usize,
) -> u32 {
    let Some(name) = (unsafe { input(name, name_length, 1024) }) else {
        return 0;
    };
    let Some(bytes) = (unsafe { input(bytes, length, 32 * 1024 * 1024) }) else {
        return 0;
    };
    run(|state| {
        let name = std::str::from_utf8(name).map_err(|_| Error::Malformed("resource path"))?;
        if name.starts_with("sound/") {
            if state.staged.contains_key(name.as_bytes()) {
                return Err(Error::Malformed("duplicate audio resource"));
            }
            let hash = Sha256::digest(bytes).into();
            let resource = match state
                .resources
                .get(name.as_bytes())
                .filter(|resource| resource.hash == hash)
            {
                Some(resource) => resource.clone(),
                None => Resource {
                    hash,
                    clip: Arc::new(Clip::decode(name.as_bytes(), bytes)?),
                },
            };
            state.staged.insert(name.as_bytes().to_vec(), resource);
        } else {
            if state.documents.len() >= 256
                || state
                    .documents
                    .insert(name.to_owned(), bytes.to_vec())
                    .is_some()
            {
                return Err(Error::Malformed("audio document limit or duplicate"));
            }
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_needed_documents() -> u32 {
    run(|state| {
        let binding = state
            .documents
            .get(playsrc_audio::soundscape::MAP_BINDING)
            .ok_or(Error::Malformed("soundscape map binding"))?;
        let world: [u8; 32] = binding
            .get(8..40)
            .ok_or(Error::Malformed("soundscape map binding"))?
            .try_into()
            .unwrap();
        let name = playsrc_audio::soundscape::read_map_binding(binding, world)
            .ok_or(Error::Malformed("soundscape map binding"))?;
        let mut paths = vec![
            "scripts/dsp_presets.txt".to_owned(),
            "scripts/soundmixers.txt".to_owned(),
        ];
        Registry::load(name, |path| {
            paths.push(path.to_owned());
            Ok(state.documents.get(path).cloned())
        })
        .map_err(|_| Error::Malformed("soundscape manifest"))?;
        state.output.clear();
        state
            .output
            .extend_from_slice(&(paths.len() as u32).to_le_bytes());
        for path in paths {
            state
                .output
                .extend_from_slice(&(path.len() as u32).to_le_bytes());
            state.output.extend_from_slice(path.as_bytes());
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_commit() -> u32 {
    run(|state| {
        let binding = state
            .documents
            .get(playsrc_audio::soundscape::MAP_BINDING)
            .ok_or(Error::Malformed("soundscape map binding"))?;
        let world: [u8; 32] = binding
            .get(8..40)
            .ok_or(Error::Malformed("soundscape map binding"))?
            .try_into()
            .unwrap();
        let name = playsrc_audio::soundscape::read_map_binding(binding, world)
            .ok_or(Error::Malformed("soundscape map binding"))?;
        let registry = Registry::load(name, |path| Ok(state.documents.get(path).cloned()))
            .map_err(|_| Error::Malformed("soundscape registry"))?;
        let presets = Presets::parse(
            state
                .documents
                .get("scripts/dsp_presets.txt")
                .ok_or(Error::Malformed("DSP document"))?,
        )?;
        let mixers = Mixers::parse(
            state
                .documents
                .get("scripts/soundmixers.txt")
                .ok_or(Error::Malformed("mixer document"))?,
        )?;
        let mut playback = Playback::new(registry, presets, mixers)?;
        for (name, resource) in &state.staged {
            playback.register(name.clone(), resource.clip.clone())?;
        }
        state.playback = Some(playback);
        state.world = world;
        state.pending = false;
        state.resources = std::mem::take(&mut state.staged);
        state.documents.clear();
        state.output.clear();
        state
            .output
            .extend_from_slice(&(state.resources.len() as u32).to_le_bytes());
        for (name, resource) in &state.resources {
            state
                .output
                .extend_from_slice(&(name.len() as u32).to_le_bytes());
            state.output.extend_from_slice(name);
            for value in [
                resource.clip.rate,
                u32::from(resource.clip.channels),
                u32::from(resource.clip.bits),
                resource.clip.frames() as u32,
                resource.clip.loop_frame.unwrap_or(u32::MAX),
            ] {
                state.output.extend_from_slice(&value.to_le_bytes());
            }
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// Input is one complete readable audio-frame packet in module memory.
pub unsafe extern "C" fn playsrc_audio_frame(pointer: *const u8, length: usize) -> u32 {
    let Some(bytes) = (unsafe { input(pointer, length, 16384) }) else {
        return 0;
    };
    run(|state| {
        if state.pending {
            return Err(Error::Malformed("audio scene reply pending"));
        }
        let mut read = Reader::new(bytes);
        let decode = (|| {
            let entity = read.i32()?;
            let soundscape = read.i32()?;
            let bits = read.u32()?;
            if bits > 255 {
                return None;
            }
            let mut positions = [[0.0; 3]; 8];
            for position in &mut positions {
                *position = read.vector()?;
            }
            let listener = Listener {
                origin: read.vector()?,
                forward: read.vector()?,
                right: read.vector()?,
            };
            let elapsed = read.f32()?;
            let game_time = read.f32()?;
            let host_time = read.f64()?;
            let volume = read.f32()?;
            let count = read.u32()?;
            if count > 512 {
                return None;
            }
            let mut entities = Vec::with_capacity(count as usize);
            let mut seen = std::collections::BTreeSet::new();
            for _ in 0..count {
                let domain = read.u32()?;
                let identity = read.u32()?;
                let origin = read.vector()?;
                if !(1..=2).contains(&domain) || identity == 0 || !seen.insert((domain, identity)) {
                    return None;
                }
                entities.push(playsrc_audio::playback::EntityOrigin {
                    domain,
                    identity,
                    origin,
                });
            }
            read.done().then_some((
                Selection {
                    entity,
                    soundscape,
                    position_bits: bits as u8,
                    positions,
                },
                listener,
                elapsed,
                game_time,
                host_time,
                volume,
                entities,
            ))
        })()
        .ok_or(Error::Malformed("audio frame packet"))?;
        let playback = state
            .playback
            .as_mut()
            .ok_or(Error::Malformed("audio is not initialized"))?;
        playback.frame(
            playsrc_audio::playback::Frame {
                selection: decode.0,
                listener: decode.1,
                elapsed: decode.2,
                game_time: decode.3,
                host_time: decode.4,
                master_volume: decode.5,
                can_set_mixer: true,
                entities: decode.6,
            },
            &mut InstalledRandom,
        )?;
        state.sequence = state
            .sequence
            .checked_add(1)
            .ok_or(Error::Malformed("audio sequence overflow"))?;
        state.output = wire::request_bytes(&SceneRequest {
            world: state.world,
            sequence: state.sequence,
            eyes: decode.1.origin,
            host_time: decode.4,
            automatic: playback.automatic_enabled(),
            obstruction: playback.obstruction_requests().to_vec(),
        });
        state.pending = true;
        Ok(())
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// Input is one complete readable scene reply in module memory.
pub unsafe extern "C" fn playsrc_audio_scene(pointer: *const u8, length: usize) -> u32 {
    let Some(bytes) = (unsafe { input(pointer, length, 2048) }) else {
        return 0;
    };
    run(|state| {
        let reply = wire::read_reply(bytes).ok_or(Error::Malformed("scene reply"))?;
        if !state.pending || reply.world != state.world || reply.sequence != state.sequence {
            return Err(Error::Malformed("stale audio scene reply"));
        }
        let playback = state
            .playback
            .as_mut()
            .ok_or(Error::Malformed("audio is not initialized"))?;
        playback.scene(reply.room, &reply.obstruction, reply.underwater)?;
        state.pending = false;
        Ok(())
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// Input is one complete readable voice packet in module memory.
pub unsafe extern "C" fn playsrc_audio_start(pointer: *const u8, length: usize) -> u32 {
    let Some(bytes) = (unsafe { input(pointer, length, 2048) }) else {
        return 0;
    };
    run(|state| {
        let mut read = Reader::new(bytes);
        let start = (|| {
            let identity = read.u32()?;
            let path_length = read.u32()? as usize;
            let wave = read.bytes(path_length)?.to_vec();
            let volume = read.f32()?;
            let pitch = read.i32()?;
            let level = read.i32()?;
            let local = read.flag()?;
            let domain = read.u32()?;
            let entity_id = read.u32()?;
            if domain > 2 || domain != 0 && entity_id == 0 {
                return None;
            }
            let entity = (domain != 0).then_some((domain, entity_id));
            let has_position = read.flag()?;
            let position = read.vector()?;
            let radius = read.f32()?;
            let channel = read.i32()?;
            let offset_seconds = read.f32()?;
            let delay_seconds = read.f32()?;
            let envelope = if read.flag()? {
                Some(Envelope {
                    from: read.f32()?,
                    to: read.f32()?,
                    seconds: read.f32()?,
                })
            } else {
                None
            };
            let class_length = read.u32()? as usize;
            let source_class = read.bytes(class_length)?.to_vec();
            read.done().then_some(Start {
                identity,
                wave,
                volume,
                pitch,
                level,
                local,
                origin: has_position.then_some(position),
                radius,
                channel,
                offset_seconds,
                delay_seconds,
                envelope,
                source_class,
                entity,
            })
        })()
        .ok_or(Error::Malformed("voice packet"))?;
        state
            .playback
            .as_mut()
            .ok_or(Error::Malformed("audio is not initialized"))?
            .start(start)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_stop(identity: u32) {
    if let Some(playback) = &mut state().lock().expect("audio instance").playback {
        playback.stop(identity);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_reset() {
    let mut state = state().lock().expect("audio instance");
    if let Some(playback) = &mut state.playback {
        playback.reset_voices();
    }
    state.pending = false;
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_dispose() {
    *state().lock().expect("audio instance") = State::default();
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_paint(frames: usize) -> u32 {
    run(|state| {
        let playback = state
            .playback
            .as_mut()
            .ok_or(Error::Malformed("audio is not initialized"))?;
        playback.paint(frames, &mut InstalledRandom)?;
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_active_count() -> usize {
    let mut state = state().lock().expect("audio instance");
    let State {
        playback, active, ..
    } = &mut *state;
    active.clear();
    if let Some(playback) = playback {
        active.extend(playback.active_external());
    }
    active.len()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_pcm_data() -> *const i16 {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(std::ptr::null(), |playback| playback.painted().as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_pcm_count() -> usize {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(0, |playback| playback.painted().len())
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_active_data() -> *const u32 {
    state().lock().expect("audio instance").active.as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_voice_count() -> usize {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(0, Playback::active_count)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_room() -> i32 {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(0, Playback::selected_room)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_environment_starts() -> u32 {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(0, Playback::environment_starts)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_mp3_frames() -> u64 {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(0, Playback::mp3_frames)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_soundscape() -> i32 {
    state()
        .lock()
        .expect("audio instance")
        .playback
        .as_ref()
        .map_or(-1, Playback::soundscape)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_underwater() -> u32 {
    u32::from(
        state()
            .lock()
            .expect("audio instance")
            .playback
            .as_ref()
            .is_some_and(Playback::underwater),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_room_observation() -> *const f32 {
    let mut state = state().lock().expect("audio instance");
    let Some(room) = state.playback.as_ref().and_then(Playback::room_observation) else {
        return std::ptr::null();
    };
    state.room_info = [
        u32::from(room.outside) as f32,
        room.width as f32,
        room.length as f32,
        room.height as f32,
        room.diffusion,
        room.reflectivity,
        room.surfaces[0],
        room.surfaces[1],
        room.surfaces[2],
        room.surfaces[3],
        room.surfaces[4],
        room.surfaces[5],
    ];
    state.room_info.as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_output_length() -> usize {
    state().lock().expect("audio instance").output.len()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_output_data() -> *const u8 {
    state().lock().expect("audio instance").output.as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_error_length() -> usize {
    state().lock().expect("audio instance").error.len()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_audio_error_data() -> *const u8 {
    state().lock().expect("audio instance").error.as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(name: &str, bytes: &[u8]) {
        assert_eq!(
            unsafe {
                playsrc_audio_resource(name.as_ptr(), name.len(), bytes.as_ptr(), bytes.len())
            },
            1
        );
    }
    fn stage(world: [u8; 32]) {
        assert_eq!(playsrc_audio_stage(), 1);
        resource(
            playsrc_audio::soundscape::MAP_BINDING,
            &playsrc_audio::soundscape::encode_map_binding("fixture", world).unwrap(),
        );
        resource(
            playsrc_audio::soundscape::MANIFEST,
            b"soundscapes_manifest {}",
        );
        let presets = (0..15)
            .map(|id| format!("{{ {id} LINEAR .2 .7 0 0 80 .5 {{ NULL }} }}\n"))
            .collect::<String>();
        resource("scripts/dsp_presets.txt", presets.as_bytes());
        resource(
            "scripts/soundmixers.txt",
            br#"GROUPRULES { All "" "" "" "" "" 50 0 0 100 40 } Default_Mix { All 1 }"#,
        );
        let mut wav = b"RIFF\x2a\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x06\0\0\0".to_vec();
        for sample in [1000_i16, 2000, 3000] {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        resource("sound/test.wav", &wav);
    }
    fn frame() -> Vec<u8> {
        let mut bytes = vec![0; 168];
        bytes[4..8].copy_from_slice(&(-1_i32).to_le_bytes());
        for (at, value) in [
            (120, 1.0_f32),
            (136, -1.0),
            (144, 0.015),
            (148, 1.0),
            (160, 1.0),
        ] {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[152..160].copy_from_slice(&1.0_f64.to_le_bytes());
        bytes
    }
    fn start() -> Vec<u8> {
        let mut bytes = vec![];
        for word in [17_u32, 8] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(b"test.wav");
        // volume, pitch, level, local, entity domain/id, has origin, xyz,
        // radius, channel, offsets, no envelope, empty source class.
        for word in [
            1.0_f32.to_bits(),
            100,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    fn update(bytes: &[u8]) -> u32 {
        unsafe { playsrc_audio_frame(bytes.as_ptr(), bytes.len()) }
    }
    fn scene(reply: &wire::SceneReply) -> u32 {
        let bytes = wire::reply_bytes(reply);
        unsafe { playsrc_audio_scene(bytes.as_ptr(), bytes.len()) }
    }
    #[test]
    fn host_binding_owns_transaction_rollback_replacement_reuse_and_disposal() {
        playsrc_audio_dispose();
        stage([7; 32]);
        assert_eq!(playsrc_audio_commit(), 1);
        let clip = state().lock().unwrap().resources[b"sound/test.wav".as_slice()]
            .clip
            .clone();
        let voice = start();
        assert_eq!(
            unsafe { playsrc_audio_start(voice.as_ptr(), voice.len()) },
            1
        );
        assert_eq!(playsrc_audio_active_count(), 1);
        let frame = frame();
        let mut malformed = frame.clone();
        malformed[160..164].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(update(&malformed), 0);
        assert!(!state().lock().unwrap().pending);
        assert_eq!(update(&frame), 1);
        let request = wire::read_request(&state().lock().unwrap().output).unwrap();
        assert_eq!(update(&frame), 0);
        assert_eq!(playsrc_audio_paint(4), 1);
        assert_eq!(
            state().lock().unwrap().playback.as_ref().unwrap().painted(),
            &[0; 8]
        );
        let mut reply = wire::SceneReply {
            world: [8; 32],
            sequence: request.sequence,
            room: None,
            underwater: false,
            obstruction: vec![],
        };
        assert_eq!(scene(&reply), 0);
        reply.world = request.world;
        reply.obstruction.push((1, 1.0));
        reply.room = Some(playsrc_audio::acoustics::RoomChange {
            node: 0,
            created: Some(playsrc_audio::room::Room {
                outside: false,
                width: 256,
                length: 512,
                height: 128,
                diffusion: 0.0,
                reflectivity: 0.5,
                surfaces: [0.5; 6],
            }),
        });
        assert_eq!(scene(&reply), 0);
        assert!(
            state()
                .lock()
                .unwrap()
                .playback
                .as_mut()
                .unwrap()
                .room(0, None)
                .is_err()
        );
        reply.obstruction.clear();
        reply.room = None;
        assert_eq!(scene(&reply), 1);
        assert_eq!(scene(&reply), 0);
        assert_eq!(playsrc_audio_paint(4), 1);
        assert_eq!(
            state().lock().unwrap().playback.as_ref().unwrap().painted(),
            &[992, 992, 1984, 1984, 2976, 2976, 0, 0]
        );

        assert_eq!(playsrc_audio_stage(), 1);
        assert_eq!(playsrc_audio_commit(), 0);
        assert_eq!(playsrc_audio_active_count(), 1);
        assert!(Arc::ptr_eq(
            &clip,
            &state().lock().unwrap().resources[b"sound/test.wav".as_slice()].clip
        ));
        stage([8; 32]);
        assert_eq!(playsrc_audio_commit(), 1);
        assert!(Arc::ptr_eq(
            &clip,
            &state().lock().unwrap().resources[b"sound/test.wav".as_slice()].clip
        ));
        assert_eq!(playsrc_audio_active_count(), 0);
        assert_eq!(update(&frame), 1);
        assert_eq!(scene(&reply), 0);
        let next = wire::read_request(&state().lock().unwrap().output).unwrap();
        assert!(next.sequence > request.sequence);
        reply.world = next.world;
        reply.sequence = next.sequence;
        assert_eq!(scene(&reply), 1);
        playsrc_audio_reset();
        assert_eq!(playsrc_audio_voice_count(), 0);
        let weak = Arc::downgrade(&clip);
        drop(clip);
        playsrc_audio_dispose();
        assert!(weak.upgrade().is_none());
        assert_eq!(playsrc_audio_active_count(), 0);
        assert_eq!(playsrc_audio_paint(4), 0);
    }
}
