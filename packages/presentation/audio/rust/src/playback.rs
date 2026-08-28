//! Source-owned PCM voices, soundscape commands and room/player effect buses.
use crate::{
    dsp::{Error, Preset, Presets, SAMPLE_RATE},
    mixers::{Membership, Mixers},
    output::{self, MonoEffect},
    room::{DEFAULT_TEMPLATES, Room},
    soundscape::{Action, Activation, Listener, Random, Registry, Selection, Soundscape, Voice},
    spatial::{self, RoomSend},
};
use std::{collections::BTreeMap, sync::Arc};

const QUANTUM: usize = 1020;
const FRACTION: u64 = 1 << 28;

#[derive(Clone, Debug)]
pub struct Clip {
    pub samples: Vec<i16>,
    pub rate: u32,
    pub channels: u8,
    pub bits: u8,
    pub loop_frame: Option<u32>,
}
impl Clip {
    pub fn decode(path: &[u8], bytes: &[u8]) -> Result<Self, Error> {
        let clip = if path.ends_with(b".mp3") {
            let pcm = playsrc_mp3::decode(bytes, 32 * 1024 * 1024, 32 * 1024 * 1024)
                .map_err(|_| Error::Malformed("MP3 decode"))?;
            Self {
                samples: pcm.samples,
                rate: pcm.sample_rate,
                channels: pcm.channels,
                bits: 16,
                loop_frame: None,
            }
        } else {
            let wav = playsrc_wav::parse_pcm(bytes).map_err(|_| Error::Malformed("PCM wave"))?;
            let samples = if wav.bits == 8 {
                wav.data
                    .iter()
                    .map(|value| (i16::from(*value) - 128) << 8)
                    .collect()
            } else {
                wav.data
                    .chunks_exact(2)
                    .map(|value| i16::from_le_bytes([value[0], value[1]]))
                    .collect()
            };
            Self {
                samples,
                rate: wav.metadata.sample_rate,
                channels: wav.channels as u8,
                bits: wav.bits as u8,
                loop_frame: wav.metadata.cue_frame,
            }
        };
        if ![11025, 22050, 44100].contains(&clip.rate) {
            return Err(Error::Malformed("unsupported source sampling rate"));
        }
        Ok(clip)
    }
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }
    fn value(&self, frame: usize, channel: usize) -> i16 {
        let frame = if frame < self.frames() {
            frame
        } else if let Some(first) = self.loop_frame {
            first as usize + (frame - self.frames()) % (self.frames() - first as usize)
        } else {
            return 0;
        };
        self.samples
            [frame * usize::from(self.channels) + channel.min(usize::from(self.channels) - 1)]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Envelope {
    pub from: f32,
    pub to: f32,
    pub seconds: f32,
}
#[derive(Clone, Debug)]
pub struct Start {
    pub identity: u32,
    pub wave: Vec<u8>,
    pub volume: f32,
    pub pitch: i32,
    pub level: i32,
    pub origin: Option<[f32; 3]>,
    pub local: bool,
    pub radius: f32,
    pub channel: i32,
    pub source_class: Vec<u8>,
    pub offset_seconds: f32,
    pub delay_seconds: f32,
    pub envelope: Option<Envelope>,
    pub entity: Option<(u32, u32)>,
}
#[derive(Clone, Copy, Debug)]
pub struct ObstructionRequest {
    pub voice: u32,
    pub origin: [f32; 3],
    pub level: i32,
    pub radius: f32,
}

pub struct Frame {
    pub selection: Selection,
    pub listener: Listener,
    pub elapsed: f32,
    pub game_time: f32,
    pub host_time: f64,
    pub master_volume: f32,
    pub can_set_mixer: bool,
    pub entities: Vec<EntityOrigin>,
}
pub struct EntityOrigin {
    pub domain: u32,
    pub identity: u32,
    pub origin: [f32; 3],
}

#[derive(Clone, Debug)]
struct Playing {
    external: Option<u32>,
    wave: Vec<u8>,
    clip: Arc<Clip>,
    source: Voice,
    entity: Option<(u32, u32)>,
    available: bool,
    local: bool,
    radius: f32,
    channel: i32,
    dry: bool,
    streaming: bool,
    fast_pitch: bool,
    omni: bool,
    mp3: bool,
    cursor: u64,
    delay: usize,
    elapsed: f32,
    envelope: Option<Envelope>,
    membership: Membership,
    send: RoomSend,
    first: bool,
    traced: bool,
    obscured: f32,
    obscured_target: f32,
    volumes: [f32; 2],
    targets: [f32; 2],
    last_volume: f32,
    source_distance: f32,
}

#[derive(Clone, Copy, Debug)]
struct RenderControls {
    room: i32,
    automatic: i32,
    player: i32,
    dsp_volume: f32,
    master_volume: f32,
    host_time: f64,
}

impl Playing {
    fn sample(&mut self) -> [i16; 2] {
        if self.delay > 0 {
            self.delay -= 1;
            return [0; 2];
        }
        let total = self.clip.frames() as u64 * FRACTION;
        if self.cursor >= total
            && let Some(first) = self.clip.loop_frame
        {
            let first = u64::from(first) * FRACTION;
            self.cursor = first + (self.cursor - total) % (total - first);
        }
        let index = (self.cursor >> 28) as usize;
        let pitch = self.source.pitch as f32 * 0.01;
        let interpolate = !self.fast_pitch && pitch != pitch.floor();
        let fraction = ((self.cursor & (FRACTION - 1)) >> 14) as i32;
        let result = std::array::from_fn(|channel| {
            let first = i32::from(self.clip.value(index, channel));
            if interpolate {
                let second = i32::from(self.clip.value(index + 1, channel));
                (first + (((second - first) * fraction) >> 14)) as i16
            } else {
                first as i16
            }
        });
        let precision = if interpolate { 14 } else { 28 };
        let input = f64::from(pitch * self.clip.rate as f32) / f64::from(self.clip.rate);
        self.cursor += ((input * f64::from(1_u32 << precision)) as u64) << (28 - precision);
        result
    }
    fn envelope_gain(&self) -> f32 {
        self.envelope.map_or(1.0, |envelope| {
            if envelope.seconds == 0.0 {
                return envelope.to;
            }
            let progress = (f64::from(self.elapsed) / f64::from(envelope.seconds)).clamp(0.0, 1.0);
            (f64::from(envelope.from) + f64::from(envelope.to - envelope.from) * progress) as f32
        })
    }
}

#[derive(Debug)]
pub struct Playback {
    registry: Registry,
    soundscape: Soundscape,
    presets: Presets,
    mixers: Mixers,
    clips: BTreeMap<Vec<u8>, Arc<Clip>>,
    voices: Vec<Option<Playing>>,
    regular: MonoEffect,
    automatic: MonoEffect,
    player: MonoEffect,
    water: MonoEffect,
    templates: [Option<Preset>; 40],
    rooms: [Option<Room>; 40],
    send_definition: Preset,
    room_request: i32,
    automatic_request: i32,
    player_request: i32,
    dsp_volume: f32,
    mixer_request: Option<Option<Vec<u8>>>,
    render_controls: RenderControls,
    render_mixer: Option<Option<Vec<u8>>>,
    listener: Listener,
    elapsed: f32,
    host_time: f64,
    underwater: bool,
    master_volume: f32,
    requests: Vec<ObstructionRequest>,
    trace_count: usize,
    filters: [[output::Frame; 2]; 3],
    buses: [Vec<output::Frame>; 3],
    output: Vec<f32>,
    environment_starts: u32,
    mp3_frames: u64,
}

impl Playback {
    pub fn new(registry: Registry, presets: Presets, mixers: Mixers) -> Result<Self, Error> {
        let zero = presets
            .0
            .first()
            .ok_or(Error::Malformed("missing zero preset"))?
            .clone();
        let water = presets
            .0
            .get(14)
            .ok_or(Error::Malformed("missing water preset"))?
            .clone();
        let mut send_definition = zero.clone();
        send_definition.mix = [0.2, 0.8];
        send_definition.minimum_level = 80.0;
        send_definition.quiet_mix_drop = 0.5;
        Ok(Self {
            registry,
            soundscape: Default::default(),
            regular: MonoEffect::new(0, zero.clone(), 0.2)?,
            automatic: MonoEffect::new(0, zero.clone(), 0.2)?,
            player: MonoEffect::new(0, zero, 0.1)?,
            water: MonoEffect::new(14, water, 0.1)?,
            presets,
            mixers,
            clips: BTreeMap::new(),
            voices: vec![],
            templates: std::array::from_fn(|_| None),
            rooms: [None; 40],
            send_definition,
            room_request: 0,
            automatic_request: 0,
            player_request: 0,
            dsp_volume: 1.0,
            mixer_request: None,
            render_controls: RenderControls {
                room: 0,
                automatic: 0,
                player: 0,
                dsp_volume: 1.0,
                master_volume: 1.0,
                host_time: 0.0,
            },
            render_mixer: None,
            listener: Listener {
                origin: [0.0; 3],
                forward: [1.0, 0.0, 0.0],
                right: [0.0, -1.0, 0.0],
            },
            elapsed: 0.0,
            host_time: 0.0,
            underwater: false,
            master_volume: 1.0,
            requests: vec![],
            trace_count: 0,
            filters: [[[0; 2]; 2]; 3],
            buses: std::array::from_fn(|_| vec![[0; 2]; QUANTUM]),
            output: Vec::with_capacity(16384),
            environment_starts: 0,
            mp3_frames: 0,
        })
    }
    pub fn register(&mut self, path: Vec<u8>, clip: impl Into<Arc<Clip>>) -> Result<(), Error> {
        let clip = clip.into();
        if self.clips.len() >= 4096 || self.clips.contains_key(&path) {
            return Err(Error::Malformed("audio resource identity"));
        }
        if ![1, 2].contains(&clip.channels)
            || ![8, 16].contains(&clip.bits)
            || ![11025, 22050, 44100].contains(&clip.rate)
            || clip.samples.is_empty()
            || clip.samples.len() > 32 * 1024 * 1024
            || clip.samples.len() % usize::from(clip.channels) != 0
            || clip
                .loop_frame
                .is_some_and(|frame| frame as usize >= clip.frames())
        {
            return Err(Error::Malformed("audio resource format"));
        }
        self.clips.insert(path, clip);
        Ok(())
    }
    pub fn clip(&self, path: &[u8]) -> Option<&Clip> {
        self.clips.get(path).map(Arc::as_ref)
    }
    pub fn automatic_enabled(&self) -> bool {
        self.room_request == 1
    }
    pub fn obstruction_requests(&self) -> &[ObstructionRequest] {
        &self.requests
    }
    pub fn active_external(&self) -> impl Iterator<Item = u32> + '_ {
        self.voices
            .iter()
            .flatten()
            .filter_map(|voice| voice.external)
    }
    pub fn active_count(&self) -> usize {
        self.voices.iter().flatten().count()
    }
    pub fn painted(&self) -> &[f32] {
        &self.output
    }
    pub fn environment_starts(&self) -> u32 {
        self.environment_starts
    }
    pub fn mp3_frames(&self) -> u64 {
        self.mp3_frames
    }
    pub fn soundscape(&self) -> i32 {
        self.soundscape.selection().soundscape
    }
    pub fn room_observation(&self) -> Option<Room> {
        if self.render_controls.room == 1 && self.automatic.identity() >= 60 {
            self.rooms
                .get((self.automatic.identity() - 60) as usize)
                .copied()
                .flatten()
        } else {
            None
        }
    }
    pub fn underwater(&self) -> bool {
        self.underwater
    }
    pub fn selected_room(&self) -> i32 {
        if self.render_controls.room == 1 {
            self.automatic.identity()
        } else {
            self.regular.identity()
        }
    }

    pub fn start(&mut self, input: Start) -> Result<(), Error> {
        if input.identity == 0 {
            return Err(Error::Malformed("external voice identity"));
        }
        let voice = Voice {
            wave: input.wave,
            position: input.origin,
            volume: input.volume,
            pitch: input.pitch,
            sound_level: input.level,
        };
        self.insert(
            voice,
            Some(input.identity),
            input.local,
            input.radius,
            input.channel,
            &input.source_class,
            input.offset_seconds,
            input.delay_seconds,
            input.envelope,
            input.entity,
        )
    }
    pub fn stop(&mut self, identity: u32) {
        for voice in &mut self.voices {
            if voice
                .as_ref()
                .is_some_and(|voice| voice.external == Some(identity))
            {
                *voice = None;
            }
        }
    }
    pub fn reset_voices(&mut self) {
        self.voices.clear();
        let mut stopped = vec![];
        self.soundscape.reset(&mut stopped);
        self.requests.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn insert(
        &mut self,
        mut source: Voice,
        external: Option<u32>,
        local: bool,
        radius: f32,
        channel: i32,
        class: &[u8],
        offset: f32,
        delay: f32,
        envelope: Option<Envelope>,
        entity: Option<(u32, u32)>,
    ) -> Result<(), Error> {
        if !source.volume.is_finite()
            || source.volume < 0.0
            || !(1..=255).contains(&source.pitch)
            || ![radius, offset, delay]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0)
            || source
                .position
                .is_some_and(|position| position.iter().any(|value| !value.is_finite()))
            || envelope.is_some_and(|value| {
                ![value.from, value.to, value.seconds]
                    .iter()
                    .all(|value| value.is_finite() && *value >= 0.0)
            })
        {
            return Err(Error::Malformed("voice parameters"));
        }
        let path = crate::soundscape::resource_path(&source.wave);
        let clip = Arc::clone(
            self.clips
                .get(&path)
                .ok_or(Error::Malformed("missing audio resource"))?,
        );
        let decorations = source
            .wave
            .iter()
            .take_while(|byte| b"*?!#><^@)}".contains(byte))
            .copied()
            .collect::<Vec<_>>();
        if decorations.iter().any(|byte| b"?!><^".contains(byte)) {
            return Err(Error::Malformed("unsupported decorated voice"));
        }
        if source.position.is_none() && !local {
            source.sound_level = 0;
        }
        let membership = self
            .mixers
            .membership(&path, class, channel, source.sound_level);
        let send = RoomSend::new(source.sound_level, &self.send_definition);
        let cursor = (f64::from(offset) * f64::from(clip.rate) * FRACTION as f64) as u64;
        let delay = (delay * clip.rate as f32) as usize;
        let voice = Playing {
            external,
            wave: source.wave.to_ascii_lowercase(),
            clip,
            source,
            entity,
            available: true,
            local,
            radius,
            channel,
            dry: decorations.contains(&b'#'),
            streaming: decorations.contains(&b'*'),
            fast_pitch: decorations.contains(&b'}'),
            omni: decorations.contains(&b'@'),
            mp3: path.ends_with(b".mp3"),
            cursor,
            delay,
            elapsed: 0.0,
            envelope,
            membership,
            send,
            first: true,
            traced: false,
            obscured: 1.0,
            obscured_target: 1.0,
            volumes: [0.0; 2],
            targets: [0.0; 2],
            last_volume: 0.0,
            source_distance: 12.0,
        };
        if let Some(slot) = self.voices.iter_mut().find(|voice| {
            voice
                .as_ref()
                .is_some_and(|voice| external.is_some() && voice.external == external)
        }) {
            *slot = Some(voice);
        } else if let Some(slot) = self.voices.iter_mut().find(|voice| voice.is_none()) {
            *slot = Some(voice);
        } else if self.voices.len() < 128 {
            self.voices.push(Some(voice));
        } else {
            return Err(Error::Malformed("audio voice capacity"));
        }
        if external.is_none() {
            self.environment_starts = self.environment_starts.saturating_add(1);
        }
        Ok(())
    }

    pub fn frame(&mut self, frame: Frame, random: &mut impl Random) -> Result<(), Error> {
        let Frame {
            selection,
            listener,
            elapsed,
            game_time,
            host_time,
            master_volume: master,
            can_set_mixer,
            entities,
        } = frame;
        if !elapsed.is_finite()
            || elapsed < 0.0
            || !game_time.is_finite()
            || !host_time.is_finite()
            || !master.is_finite()
            || !(0.0..=1.0).contains(&master)
            || listener
                .origin
                .into_iter()
                .chain(listener.forward)
                .chain(listener.right)
                .any(|value| !value.is_finite())
        {
            return Err(Error::Malformed("audio frame parameters"));
        }
        self.listener = listener;
        self.elapsed = elapsed;
        self.host_time = host_time;
        self.master_volume = master;
        let mut actions = vec![];
        self.soundscape.select(
            &self.registry,
            selection,
            Activation {
                time: game_time,
                restoring: false,
                can_set_mixer,
            },
            random,
            &mut actions,
        );
        self.soundscape
            .update(elapsed, game_time, listener, random, &mut actions);
        for action in actions {
            match action {
                Action::RoomDsp(value) => self.room_request = value,
                Action::PlayerDsp(value) => self.player_request = value,
                Action::DspVolume(value) => self.dsp_volume = value.unwrap_or(1.0),
                Action::Mixer(value) => self.mixer_request = Some(value),
                Action::Start(voice) => {
                    self.insert(voice, None, false, 0.0, 6, b"", 0.0, 0.0, None, None)?
                }
                Action::Volume(voice) => {
                    if let Some(active) = self.voices.iter_mut().flatten().find(|active| {
                        active.external.is_none() && active.wave.eq_ignore_ascii_case(&voice.wave)
                    }) {
                        active.source.volume = voice.volume;
                    } else {
                        self.insert(voice, None, false, 0.0, 6, b"", 0.0, 0.0, None, None)?;
                    }
                }
                Action::Stop { wave, .. } => {
                    if let Some(active) = self.voices.iter_mut().find(|active| {
                        active.as_ref().is_some_and(|active| {
                            active.external.is_none() && active.wave.eq_ignore_ascii_case(&wave)
                        })
                    }) {
                        *active = None;
                    }
                }
            }
        }
        self.requests.clear();
        self.trace_count = 0;
        for (index, voice) in self
            .voices
            .iter_mut()
            .enumerate()
            .filter_map(|(index, voice)| voice.as_mut().map(|voice| (index, voice)))
        {
            if !voice.first {
                voice.elapsed += elapsed;
            }
            if let Some((domain, identity)) = voice.entity {
                let entity = entities
                    .iter()
                    .find(|entity| entity.domain == domain && entity.identity == identity);
                voice.available = entity.is_some();
                if let Some(entity) = entity {
                    voice.source.position = Some(entity.origin);
                }
            }
            if !voice.available {
                continue;
            }
            let Some(origin) = voice.source.position else {
                continue;
            };
            if voice.local
                || voice.omni
                || voice.source.sound_level == 0
                || !voice.first && voice.clip.loop_frame.is_none() && !voice.streaming
            {
                continue;
            }
            if !voice.first && (voice.traced || self.trace_count >= 2) {
                continue;
            }
            if !voice.first {
                self.trace_count += 1;
            }
            voice.traced = true;
            self.requests.push(ObstructionRequest {
                voice: index as u32 + 1,
                origin,
                level: voice.source.sound_level,
                radius: voice.radius,
            });
        }
        Ok(())
    }

    pub fn room(&mut self, index: usize, measured: Option<Room>) -> Result<(), Error> {
        if index >= 40 {
            return Err(Error::Malformed("room node"));
        }
        if let Some(room) = measured {
            self.templates[index] = Some(self.presets.automatic(room, &DEFAULT_TEMPLATES)?.1);
            self.rooms[index] = Some(room);
        }
        if self.templates[index].is_none() {
            return Err(Error::Malformed("room node has no preset"));
        }
        self.automatic_request = index as i32 + 60;
        Ok(())
    }

    pub fn spatialize(
        &mut self,
        obstruction: &[(u32, f32)],
        underwater: bool,
    ) -> Result<(), Error> {
        self.validate_obstruction(obstruction)?;
        self.apply_spatialization(obstruction, underwater);
        Ok(())
    }

    pub fn scene(
        &mut self,
        room: Option<crate::acoustics::RoomChange>,
        obstruction: &[(u32, f32)],
        underwater: bool,
    ) -> Result<(), Error> {
        // Validate the whole reply before mutating a room node or any channel.
        // An extra paint may retire a queried voice; it cannot replace its slot
        // until the next control frame, so that expired result is safe to ignore.
        self.validate_obstruction(obstruction)?;
        if let Some(change) = room {
            self.room(change.node, change.created)?;
        }
        self.apply_spatialization(obstruction, underwater);
        Ok(())
    }

    fn validate_obstruction(&self, obstruction: &[(u32, f32)]) -> Result<(), Error> {
        if obstruction.len() != self.requests.len()
            || obstruction
                .iter()
                .zip(&self.requests)
                .any(|((id, gain), request)| {
                    *id != request.voice || !gain.is_finite() || !(0.0..=1.0).contains(gain)
                })
        {
            return Err(Error::Malformed("obstruction response"));
        }
        Ok(())
    }

    fn apply_spatialization(&mut self, obstruction: &[(u32, f32)], underwater: bool) {
        self.underwater = underwater;
        for &(id, gain) in obstruction {
            if let Some(voice) = self.voices[id as usize - 1].as_mut() {
                voice.obscured_target = gain;
            }
        }
        for voice in self.voices.iter_mut().flatten() {
            if !voice.available {
                voice.volumes = [0.0; 2];
                voice.targets = [0.0; 2];
                voice.last_volume = 0.0;
                voice.first = false;
                continue;
            }
            let difference = voice.obscured_target - voice.obscured;
            if voice.first || difference.abs() < 0.01 {
                voice.obscured = voice.obscured_target;
            } else {
                voice.obscured += (f64::from(self.elapsed) / 0.25 * f64::from(difference)) as f32;
            }
            let mixer = self.mixers.gain(voice.membership);
            let origin = if voice.local {
                None
            } else {
                voice.source.position
            };
            let level = if voice.local {
                0
            } else {
                voice.source.sound_level
            };
            let local_gain = if voice.local {
                spatial::distance_gain(voice.source.sound_level, 12.0)
                    * if voice.channel == 1 {
                        10.0_f64.powf(0.1) as f32
                    } else {
                        1.0
                    }
            } else {
                1.0
            };
            let volume = voice.source.volume * voice.envelope_gain();
            let spatial = spatial::stereo(
                volume,
                level,
                origin,
                self.listener,
                voice.radius,
                voice.obscured * mixer * local_gain,
                voice.omni,
            );
            voice.targets = spatial.volume.map(f32::from);
            for (value, target) in voice.volumes.iter_mut().zip(voice.targets) {
                let difference = target - *value;
                if voice.first || difference.abs() < 5.0 {
                    *value = target;
                } else {
                    *value += ((f64::from(self.elapsed) / 0.070 * f64::from(difference)) as f32)
                        .clamp(-20.0, 20.0);
                }
            }
            voice.last_volume =
                spatial.distance_gain * ((volume * 255.0) as i32 as f64 / 255.0) as f32;
            voice.source_distance = spatial.source_distance;
            voice.first = false;
        }
        if self.trace_count == 0 {
            for voice in self.voices.iter_mut().flatten() {
                voice.traced = false;
            }
        }
        self.render_controls = RenderControls {
            room: self.room_request,
            automatic: self.automatic_request,
            player: self.player_request,
            dsp_volume: self.dsp_volume,
            master_volume: self.master_volume,
            host_time: self.host_time,
        };
        if self.mixer_request.is_some() {
            self.render_mixer = self.mixer_request.take();
        }
    }

    pub fn paint(&mut self, frames: usize, random: &mut impl Random) -> Result<&[f32], Error> {
        if frames == 0 || frames > 8192 || !frames.is_multiple_of(4) {
            return Err(Error::Malformed("paint frame count"));
        }
        if let Some(mixer) = self.render_mixer.take() {
            self.mixers.select(mixer.as_deref());
        }
        let controls = self.render_controls;
        if let Some(preset) = self.presets.0.get(controls.room as usize)
            && self.regular.identity() != controls.room
        {
            self.regular.select(controls.room, preset.clone())?;
            self.send_definition = preset.clone();
        }
        if self.automatic.identity() != controls.automatic {
            let preset = if controls.automatic == 0 {
                &self.presets.0[0]
            } else {
                self.templates[(controls.automatic - 60) as usize]
                    .as_ref()
                    .ok_or(Error::Malformed("automatic preset"))?
            };
            self.automatic.select(controls.automatic, preset.clone())?;
            self.send_definition = preset.clone();
        }
        if let Some(preset) = self.presets.0.get(controls.player as usize) {
            self.player.select(controls.player, preset.clone())?;
        }
        self.output.clear();
        self.output.resize(frames * 2, 0.0);
        let mut mixable = [false; 128];
        for (index, slot) in self.voices.iter_mut().enumerate() {
            let Some(voice) = slot else {
                continue;
            };
            if voice.first {
                continue;
            }
            let low = voice
                .volumes
                .into_iter()
                .chain(voice.targets)
                .all(|volume| volume as i32 <= 1);
            if low && voice.clip.loop_frame.is_none() && !voice.dry {
                *slot = None;
            } else {
                mixable[index] = !low;
            }
        }
        let has_11 = self.voices.iter().enumerate().any(|(index, voice)| {
            mixable[index] && voice.as_ref().is_some_and(|voice| voice.clip.rate == 11025)
        });
        let has_22 = self.voices.iter().enumerate().any(|(index, voice)| {
            mixable[index] && voice.as_ref().is_some_and(|voice| voice.clip.rate == 22050)
        });
        for base in (0..frames).step_by(QUANTUM) {
            let count = (frames - base).min(QUANTUM);
            let room_off = if controls.room == 1 {
                self.automatic.is_off()
            } else {
                self.regular.is_off()
            };
            for bus in &mut self.buses {
                bus[..count].fill([0; 2]);
            }
            for (stage, rate) in [11025, 22050, 44100].into_iter().enumerate() {
                let count_at_rate = count / (SAMPLE_RATE / rate) as usize;
                for (index, slot) in self.voices.iter_mut().enumerate() {
                    if !mixable[index] {
                        continue;
                    }
                    let Some(voice) = slot else {
                        continue;
                    };
                    if voice.clip.rate != rate {
                        continue;
                    }
                    if voice.delay == 0
                        && voice.clip.loop_frame.is_none()
                        && voice.cursor >> 28 >= voice.clip.frames() as u64
                    {
                        *slot = None;
                        continue;
                    }
                    let distance = voice.source_distance;
                    for frame in 0..count_at_rate {
                        let sample = voice.sample();
                        let volume = voice.volumes;
                        let (wet, facing) = output::split_volume(
                            volume,
                            voice.send.gain(distance),
                            controls.dsp_volume,
                            room_off,
                        );
                        let mut contributed = false;
                        for (bus, volumes) in if voice.dry {
                            [
                                (2, volume.map(|value| value.clamp(0.0, 255.0))),
                                (2, [0.0; 2]),
                            ]
                        } else {
                            [(0, wet), (1, facing)]
                        } {
                            for channel in 0..2 {
                                let value = if voice.clip.bits == 8 {
                                    output::sample8((sample[channel] >> 8) as i8, volumes[channel])
                                } else {
                                    output::sample16(sample[channel], volumes[channel])
                                };
                                contributed |= value != 0;
                                self.buses[bus][frame][channel] =
                                    self.buses[bus][frame][channel].wrapping_add(value);
                            }
                        }
                        if voice.external.is_none() && voice.mp3 && contributed {
                            self.mp3_frames = self.mp3_frames.saturating_add(1);
                        }
                    }
                }
                if stage < 2 {
                    for (bus, values) in self.buses.iter_mut().enumerate() {
                        if bus == 2 && !(has_11 || stage == 1 && has_22) {
                            continue;
                        }
                        let previous = self.filters[bus][stage];
                        let last = values[count_at_rate - 1];
                        for frame in (0..count_at_rate).rev() {
                            let current = values[frame];
                            let preceding = if frame == 0 {
                                previous
                            } else {
                                values[frame - 1]
                            };
                            values[frame * 2] = std::array::from_fn(|axis| {
                                preceding[axis].wrapping_add(current[axis]) >> 1
                            });
                            values[frame * 2 + 1] = current;
                        }
                        self.filters[bus][stage] = last;
                    }
                }
            }
            let room = if controls.room == 1 {
                &mut self.automatic
            } else {
                &mut self.regular
            };
            room.process(&mut self.buses[0][..count], random);
            for frame in 0..count {
                for channel in 0..2 {
                    self.buses[1][frame][channel] =
                        self.buses[1][frame][channel].wrapping_add(self.buses[0][frame][channel]);
                }
            }
            if self.underwater {
                self.water.process(&mut self.buses[1][..count], random);
            }
            self.player.process(&mut self.buses[1][..count], random);
            for frame in 0..count {
                for channel in 0..2 {
                    self.output[(base + frame) * 2 + channel] = output::device_sample(
                        self.buses[1][frame][channel].wrapping_add(self.buses[2][frame][channel]),
                        controls.master_volume,
                    );
                }
            }
        }
        self.mixers.update_ducking(
            controls.host_time,
            self.voices
                .iter()
                .flatten()
                .map(|voice| (voice.membership, voice.last_volume)),
        );
        Ok(&self.output)
    }
}
