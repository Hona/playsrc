//! Soundscape command interpretation and client timelines.
//!
//! Source SDK 2013 `game/client/c_soundscape.cpp`. Resource lookup and the
//! installed client random stream are supplied by their owners. This module
//! neither opens files nor owns a clock, decoder, browser node, or random seed.

use playsrc_keyvalues::{Node, NumericValue, Value};
use std::collections::BTreeSet;

pub const MANIFEST: &str = "scripts/soundscapes_manifest.txt";
pub type Position = [f32; 3];
pub use playsrc_entity::soundscape::Selection;

#[derive(Clone, Debug, Default)]
pub struct ZoneIndex {
    clusters: Vec<Vec<usize>>,
}
impl ZoneIndex {
    pub fn compile(
        world: &playsrc_visibility::World,
        zones: &[(Position, f32)],
    ) -> Result<Self, playsrc_visibility::Error> {
        let mut bounds = vec![([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]); world.cluster_count];
        for leaf in &world.leaves {
            if leaf.cluster < 0 {
                continue;
            }
            let (minimum, maximum) = &mut bounds[leaf.cluster as usize];
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(leaf.mins[axis] as f32);
                maximum[axis] = maximum[axis].max(leaf.maxs[axis] as f32);
            }
        }
        let mut clusters = vec![Vec::new(); world.cluster_count];
        for (index, &(position, radius)) in zones.iter().enumerate() {
            let source_cluster = world.leaves[world.locate_leaf(position)?].cluster;
            if source_cluster < 0 {
                continue;
            }
            for (cluster, &(minimum, maximum)) in bounds.iter().enumerate() {
                let visible = world.pvs
                    [source_cluster as usize * world.words_per_row + cluster / 32]
                    & (1 << (cluster % 32))
                    != 0;
                if !visible {
                    continue;
                }
                let distance_squared = (0..3)
                    .map(|axis| {
                        let distance = if position[axis] < minimum[axis] {
                            minimum[axis] - position[axis]
                        } else if position[axis] > maximum[axis] {
                            position[axis] - maximum[axis]
                        } else {
                            0.0
                        };
                        distance * distance
                    })
                    .sum::<f32>();
                if radius < 0.0 || distance_squared < radius * radius {
                    clusters[cluster].push(index);
                }
            }
        }
        Ok(Self { clusters })
    }
    pub fn candidates(&self, cluster: i16) -> &[usize] {
        if cluster < 0 {
            &[]
        } else {
            self.clusters
                .get(cluster as usize)
                .map_or(&[], Vec::as_slice)
        }
    }
}

pub trait Random {
    fn float(&mut self, low: f32, high: f32) -> f32;
    fn integer(&mut self, low: i32, high: i32) -> i32;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interval {
    pub start: f32,
    pub range: f32,
}

impl Interval {
    pub fn read(bytes: &[u8]) -> Self {
        // ReadInterval uses a 128-byte temporary and strtok (empty fields skip).
        let bytes = &bytes[..bytes.len().min(127)];
        let mut parts = bytes
            .split(|byte| *byte == b',')
            .filter(|part| !part.is_empty());
        let start = number(parts.next().unwrap_or_default());
        Self {
            start,
            range: parts.next().map_or(0.0, |end| {
                (playsrc_keyvalues::decimal_float_prefix(end) - f64::from(start)) as f32
            }),
        }
    }

    fn sample(self, random: &mut impl Random) -> f32 {
        if self.range == 0.0 {
            self.start
        } else {
            self.start + random.float(0.0, self.range)
        }
    }
}

fn number(bytes: &[u8]) -> f32 {
    NumericValue::Bytes(bytes).get_float()
}
fn integer(bytes: &[u8]) -> i32 {
    NumericValue::Bytes(bytes).get_int()
}
fn text(node: &Node) -> &[u8] {
    match &node.value {
        Value::Scalar(value) => &value.token.bytes,
        Value::Object(_) => b"",
    }
}
fn children(node: &Node) -> &[Node] {
    match &node.value {
        Value::Object(value) => value,
        Value::Scalar(_) => &[],
    }
}
fn named(node: &Node, name: &[u8]) -> bool {
    node.key.bytes.eq_ignore_ascii_case(name)
}
fn attenuation(value: f32) -> i32 {
    if value > 0.0 {
        (50.0 + 20.0 / value) as i32
    } else {
        0
    }
}

fn level(bytes: &[u8]) -> Option<i32> {
    if bytes.len() < 7 || !bytes[..7].eq_ignore_ascii_case(b"SNDLVL_") {
        return None;
    }
    let suffix = &bytes[7..];
    Some(if suffix.eq_ignore_ascii_case(b"NONE") {
        0
    } else if suffix.eq_ignore_ascii_case(b"IDLE") {
        60
    } else if suffix.eq_ignore_ascii_case(b"STATIC") {
        66
    } else if suffix.eq_ignore_ascii_case(b"NORM") {
        75
    } else if suffix.eq_ignore_ascii_case(b"TALKING") {
        80
    } else if suffix.eq_ignore_ascii_case(b"GUNFIRE") {
        140
    } else {
        integer(suffix)
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
    definitions: Vec<Node>,
}

impl Registry {
    pub fn load(
        map: &str,
        mut resolve: impl FnMut(&str) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<Self, String> {
        let parse = |bytes: &[u8]| -> Result<Vec<Node>, String> {
            let document = playsrc_keyvalues::parse_text(
                bytes,
                playsrc_keyvalues::EscapeMode::LiteralBackslash,
                playsrc_keyvalues::Limits::default(),
            )
            .map_err(|error| format!("soundscape KeyValues: {error:?}"))?;
            if !document.directives.is_empty() {
                return Err("soundscape document requires KeyValues directive composition".into());
            }
            Ok(document
                .evaluated(&playsrc_keyvalues::ConditionEnvironment::new([
                    (b"$WIN32".to_vec(), true),
                    (b"$X360".to_vec(), false),
                ]))
                .roots)
        };
        let manifest =
            resolve(MANIFEST)?.ok_or_else(|| format!("missing soundscape manifest {MANIFEST}"))?;
        let mut registry = Self::default();
        for path in document_paths(&parse(&manifest)?, map) {
            let path =
                std::str::from_utf8(&path).map_err(|_| "soundscape script path is not UTF-8")?;
            if let Some(bytes) = resolve(path)? {
                registry.append(&parse(&bytes)?);
            }
        }
        Ok(registry)
    }

    pub fn append(&mut self, roots: &[Node]) {
        self.definitions.extend(
            roots
                .iter()
                .filter(|node| !children(node).is_empty())
                .cloned(),
        );
    }
    pub fn find(&self, name: &[u8]) -> Option<usize> {
        self.definitions
            .iter()
            .rposition(|node| node.key.bytes.eq_ignore_ascii_case(name))
    }
    pub fn name(&self, index: usize) -> Option<&[u8]> {
        self.definitions
            .get(index)
            .map(|node| node.key.bytes.as_slice())
    }
    pub fn len(&self) -> usize {
        self.definitions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// Complete wave closure for the supplied map roots. Nested commands follow
    /// the same depth boundary and last-definition-wins lookup as playback.
    pub fn resources(&self, roots: &[usize]) -> BTreeSet<Vec<u8>> {
        let mut result = BTreeSet::new();
        for &root in roots {
            self.collect_resources(root, 0, &mut result);
        }
        result
    }

    pub fn dsp_requirements(&self, roots: &[usize]) -> BTreeSet<i32> {
        roots
            .iter()
            .filter_map(|index| self.definitions.get(*index))
            .flat_map(children)
            .filter(|command| named(command, b"dsp") || named(command, b"dsp_player"))
            .map(|command| integer(text(command)))
            .collect()
    }

    fn collect_resources(&self, index: usize, depth: usize, out: &mut BTreeSet<Vec<u8>>) {
        if depth > 8 {
            return;
        }
        let Some(definition) = self.definitions.get(index) else {
            return;
        };
        for command in children(definition) {
            if named(command, b"playlooping") {
                for field in children(command)
                    .iter()
                    .filter(|field| named(field, b"wave"))
                {
                    out.insert(resource_path(text(field)));
                }
            } else if named(command, b"playrandom") {
                for waves in children(command)
                    .iter()
                    .filter(|field| named(field, b"rndwave"))
                {
                    for wave in children(waves) {
                        out.insert(resource_path(text(wave)));
                    }
                }
            } else if named(command, b"playsoundscape")
                && let Some(name) = children(command)
                    .iter()
                    .rev()
                    .find(|field| named(field, b"name"))
                && let Some(child) = self.find(text(name))
            {
                self.collect_resources(child, depth + 1, out);
            }
        }
    }
}

pub fn resource_path(token: &[u8]) -> Vec<u8> {
    let offset = token
        .iter()
        .take_while(|byte| b"*?!#><^@)}".contains(byte))
        .count();
    let mut path = b"sound/".to_vec();
    path.extend(token[offset..].iter().map(|byte| {
        if *byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    }));
    path
}

/// The caller resolves every path through the active Content provider plan,
/// including the BSP pack. No fallback lookup or filesystem discovery occurs.
pub fn document_paths(manifest_roots: &[Node], map: &str) -> Vec<Vec<u8>> {
    let mut paths = Vec::new();
    let map_path = format!("scripts/soundscapes_{map}.txt").into_bytes();
    let mut already_loaded = false;
    if let Some(root) = manifest_roots.first() {
        for field in children(root).iter().filter(|field| named(field, b"file")) {
            already_loaded |= text(field) == map_path;
            paths.push(text(field).to_vec());
        }
    }
    if !already_loaded {
        paths.push(map_path);
    }
    paths
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub origin: Position,
    pub forward: Position,
    pub right: Position,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub wave: Vec<u8>,
    /// None is EmitAmbientSound, not a positioned sound at the origin.
    pub position: Option<Position>,
    pub volume: f32,
    pub pitch: i32,
    pub sound_level: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Start(Voice),
    Volume(Voice),
    Stop { wave: Vec<u8>, ambient: bool },
    RoomDsp(i32),
    PlayerDsp(i32),
    Mixer(Option<Vec<u8>>),
    DspVolume(Option<f32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    pub voice: Voice,
    pub target: f32,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RandomLayer {
    pub next_time: f32,
    pub time: Interval,
    pub volume: Interval,
    pub pitch: Interval,
    pub sound_level: Interval,
    pub master_volume: f32,
    pub waves: Vec<Vec<u8>>,
    pub position: Option<Position>,
    pub random_position: bool,
}

#[derive(Clone, Copy)]
struct Scope {
    volume: f32,
    offset: i32,
    position: i32,
    ambient_position: i32,
    depth: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Activation {
    pub time: f32,
    pub restoring: bool,
    pub can_set_mixer: bool,
}

struct Expansion<'a, R> {
    activation: Activation,
    wrote: [bool; 2],
    random: &'a mut R,
    out: &'a mut Vec<Action>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Soundscape {
    selection: Selection,
    loops: Vec<Loop>,
    random: Vec<RandomLayer>,
    generation: u64,
    next_random: f32,
    pub fade_seconds: f32,
}
impl Default for Soundscape {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            loops: Vec::new(),
            random: Vec::new(),
            generation: 0,
            next_random: 0.0,
            fade_seconds: 3.0,
        }
    }
}

impl Soundscape {
    pub fn loops(&self) -> &[Loop] {
        &self.loops
    }
    pub fn random_layers(&self) -> &[RandomLayer] {
        &self.random
    }
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn select(
        &mut self,
        registry: &Registry,
        selection: Selection,
        activation: Activation,
        random: &mut impl Random,
        out: &mut Vec<Action>,
    ) {
        // Source ignores local-position-only updates for the same entity/index.
        if self.selection.entity == selection.entity
            && self.selection.soundscape == selection.soundscape
        {
            return;
        }
        self.selection = selection;
        if selection.entity > 0
            && selection.soundscape >= 0
            && (selection.soundscape as usize) < registry.len()
        {
            self.start(
                registry,
                Some(selection.soundscape as usize),
                activation,
                random,
                out,
            );
        }
    }

    pub fn start(
        &mut self,
        registry: &Registry,
        index: Option<usize>,
        activation: Activation,
        random: &mut impl Random,
        out: &mut Vec<Action>,
    ) {
        for layer in &mut self.loops {
            layer.target = 0.0;
            if index.is_none() {
                layer.voice.volume = 0.0;
            }
        }
        self.generation = self.generation.wrapping_add(1);
        self.random.clear();
        self.next_random = activation.time;
        if let Some(index) = index {
            let mut expansion = Expansion {
                activation,
                wrote: [false; 2],
                random,
                out,
            };
            self.expand(
                registry,
                index,
                Scope {
                    volume: 1.0,
                    offset: 0,
                    position: -1,
                    ambient_position: -1,
                    depth: 0,
                },
                &mut expansion,
            );
            if !expansion.wrote[1] {
                expansion.out.push(Action::DspVolume(None));
            }
            if !expansion.wrote[0] {
                expansion.out.push(Action::Mixer(None));
            }
        }
    }

    fn expand(
        &mut self,
        registry: &Registry,
        index: usize,
        scope: Scope,
        expansion: &mut Expansion<'_, impl Random>,
    ) {
        let Some(definition) = registry.definitions.get(index) else {
            return;
        };
        if scope.depth > 8 {
            return;
        }
        for command in children(definition) {
            if named(command, b"playlooping") {
                self.loop_command(
                    command,
                    scope,
                    expansion.activation.restoring,
                    expansion.random,
                    expansion.out,
                );
            } else if named(command, b"playrandom") {
                self.random_command(
                    command,
                    scope,
                    expansion.activation.time,
                    expansion.activation.restoring,
                    expansion.random,
                );
            } else if named(command, b"playsoundscape") {
                if scope.depth == 8 {
                    continue;
                }
                let mut sub = Scope {
                    depth: scope.depth + 1,
                    ..scope
                };
                let mut child = None;
                for field in children(command) {
                    if named(field, b"volume") {
                        sub.volume =
                            scope.volume * Interval::read(text(field)).sample(expansion.random);
                    } else if named(field, b"position") {
                        sub.offset = scope.offset.wrapping_add(integer(text(field)));
                    } else if named(field, b"positionoverride") && scope.position < 0 {
                        sub.position = scope.offset.wrapping_add(integer(text(field)));
                        sub.ambient_position = sub.position;
                    } else if named(field, b"ambientpositionoverride") && scope.ambient_position < 0
                    {
                        sub.ambient_position = scope.offset.wrapping_add(integer(text(field)));
                    } else if named(field, b"name") {
                        child = registry.find(text(field));
                    }
                }
                if let Some(child) = child {
                    self.expand(registry, child, sub, expansion);
                }
            } else if scope.depth == 0 {
                if named(command, b"dsp") {
                    expansion.out.push(Action::RoomDsp(integer(text(command))));
                } else if named(command, b"dsp_player") {
                    expansion
                        .out
                        .push(Action::PlayerDsp(integer(text(command))));
                } else if named(command, b"soundmixer") && expansion.activation.can_set_mixer {
                    expansion.wrote[0] = true;
                    expansion
                        .out
                        .push(Action::Mixer(Some(text(command).to_vec())));
                } else if named(command, b"dsp_volume") {
                    expansion.wrote[1] = true;
                    expansion
                        .out
                        .push(Action::DspVolume(Some(number(text(command)))));
                }
            }
        }
    }

    fn position(&self, index: i32) -> Option<Option<Position>> {
        if index < 0 {
            return Some(None);
        }
        if index >= 8 || self.selection.position_bits & (1 << index) == 0 {
            return None;
        }
        Some(Some(self.selection.positions[index as usize]))
    }

    fn loop_command(
        &mut self,
        command: &Node,
        scope: Scope,
        restoring: bool,
        random: &mut impl Random,
        out: &mut Vec<Action>,
    ) {
        let mut voice = Voice {
            wave: Vec::new(),
            position: None,
            volume: 0.0,
            pitch: 100,
            sound_level: 75,
        };
        let mut position = -1;
        let mut suppress = false;
        let mut wave_seen = false;
        for field in children(command) {
            if named(field, b"volume") {
                voice.volume = scope.volume * Interval::read(text(field)).sample(random);
            } else if named(field, b"pitch") {
                voice.pitch = Interval::read(text(field)).sample(random) as i32;
            } else if named(field, b"wave") {
                voice.wave = text(field).to_vec();
                wave_seen = true;
            } else if named(field, b"position") {
                position = scope.offset.wrapping_add(integer(text(field)));
            } else if named(field, b"attenuation") {
                voice.sound_level = attenuation(Interval::read(text(field)).sample(random));
            } else if named(field, b"soundlevel") {
                voice.sound_level = level(text(field))
                    .unwrap_or_else(|| Interval::read(text(field)).sample(random) as i32);
            } else if named(field, b"suppress_on_restore") {
                suppress = integer(text(field)) != 0;
            }
        }
        if position < 0 {
            position = scope.ambient_position;
        } else if scope.position >= 0 {
            position = scope.position;
        }
        if (restoring && suppress) || voice.volume == 0.0 || !wave_seen {
            return;
        }
        let Some(location) = self.position(position) else {
            return;
        };
        voice.position = location;
        if location.is_none() {
            voice.sound_level = 75;
        }
        let target = voice.volume;
        // Reverse lookup and immediate stop on a moved positioned loop are
        // intentional: Source's static-world channel is keyed by wave, not slot.
        if let Some(layer) = self.loops.iter_mut().rev().find(|layer| {
            layer.generation != self.generation
                && layer.voice.pitch == voice.pitch
                && layer.voice.wave.eq_ignore_ascii_case(&voice.wave)
                && layer.voice.position.is_some() == location.is_some()
        }) {
            let moved = match (layer.voice.position, location) {
                (Some(old), Some(new)) => (0..3).any(|axis| (old[axis] - new[axis]).abs() > 0.1),
                _ => false,
            };
            if moved {
                out.push(Action::Stop {
                    wave: layer.voice.wave.clone(),
                    ambient: false,
                });
            }
            voice.volume = layer.voice.volume;
            *layer = Loop {
                voice,
                target,
                generation: self.generation,
            };
            if moved {
                out.push(Action::Volume(layer.voice.clone()));
            }
        } else {
            voice.volume = if location.is_none() { 0.0 } else { 0.05 };
            out.push(Action::Start(voice.clone()));
            self.loops.push(Loop {
                voice,
                target,
                generation: self.generation,
            });
        }
    }

    fn random_command(
        &mut self,
        command: &Node,
        scope: Scope,
        now: f32,
        restoring: bool,
        random: &mut impl Random,
    ) {
        let mut layer = RandomLayer {
            next_time: now,
            time: Interval::default(),
            volume: Interval::default(),
            pitch: Interval::default(),
            sound_level: Interval::default(),
            master_volume: scope.volume,
            waves: Vec::new(),
            position: None,
            random_position: false,
        };
        let mut position = -1;
        let mut suppress = false;
        for field in children(command) {
            if named(field, b"volume") {
                layer.volume = Interval::read(text(field));
            } else if named(field, b"pitch") {
                layer.pitch = Interval::read(text(field));
            } else if named(field, b"time") {
                layer.time = Interval::read(text(field));
            } else if named(field, b"rndwave") {
                layer.waves = children(field)
                    .iter()
                    .map(|wave| text(wave).to_vec())
                    .collect();
            } else if named(field, b"attenuation") {
                let interval = Interval::read(text(field));
                let start = attenuation(interval.start) as f32;
                layer.sound_level = Interval {
                    start,
                    range: attenuation(interval.start + interval.range) as f32 - start,
                };
            } else if named(field, b"soundlevel") {
                layer.sound_level = level(text(field)).map_or_else(
                    || Interval::read(text(field)),
                    |value| Interval {
                        start: value as f32,
                        range: 0.0,
                    },
                );
            } else if named(field, b"position") {
                if text(field).eq_ignore_ascii_case(b"random") {
                    layer.random_position = true;
                } else {
                    position = scope.offset.wrapping_add(integer(text(field)));
                }
            } else if named(field, b"suppress_on_restore") {
                suppress = integer(text(field)) != 0;
            }
        }
        if position < 0 {
            position = scope.ambient_position;
        } else if scope.position >= 0 {
            position = scope.position;
            layer.random_position = false;
        }
        if (restoring && suppress) || layer.waves.is_empty() {
            return;
        }
        if !layer.random_position {
            let Some(location) = self.position(position) else {
                return;
            };
            layer.position = location;
        }
        layer.next_time = now + 0.5 * layer.time.sample(random);
        self.random.push(layer);
    }

    pub fn update(
        &mut self,
        elapsed: f32,
        now: f32,
        listener: Listener,
        random: &mut impl Random,
        out: &mut Vec<Action>,
    ) {
        let amount = if self.fade_seconds > 0.0 {
            (f64::from(elapsed) * (1.0 / f64::from(self.fade_seconds))) as f32
        } else {
            elapsed
        };
        for index in (0..self.loops.len()).rev() {
            let layer = &mut self.loops[index];
            if layer.voice.volume == layer.target {
                continue;
            }
            layer.voice.volume = if layer.target > layer.voice.volume {
                (layer.voice.volume + amount).min(layer.target)
            } else {
                (layer.voice.volume - amount).max(layer.target)
            };
            if layer.target == 0.0 && layer.voice.volume == 0.0 {
                out.push(Action::Stop {
                    wave: layer.voice.wave.clone(),
                    ambient: layer.voice.position.is_none(),
                });
                self.loops.swap_remove(index);
            } else {
                out.push(Action::Volume(layer.voice.clone()));
            }
        }
        if now < self.next_random {
            return;
        }
        self.next_random = now + 3600.0;
        for layer in self.random.iter_mut().rev() {
            if now >= layer.next_time {
                let wave = random.integer(0, layer.waves.len() as i32 - 1) as usize;
                let volume = layer.master_volume * layer.volume.sample(random);
                let positioned = layer.position.is_some() || layer.random_position;
                let sound_level = if positioned {
                    layer.sound_level.sample(random) as i32
                } else {
                    0
                };
                let pitch = layer.pitch.sample(random) as i32;
                if layer.random_position {
                    // SDK passes this draw directly to SinCos (radians).
                    let angle = random.float(-180.0, 180.0);
                    let (sin, cos) = angle.sin_cos();
                    layer.position = Some(std::array::from_fn(|axis| {
                        listener.origin[axis]
                            + 36.0 * (cos * listener.right[axis] + sin * listener.forward[axis])
                    }));
                }
                out.push(Action::Start(Voice {
                    wave: layer.waves[wave].clone(),
                    position: layer.position,
                    volume,
                    pitch,
                    sound_level,
                }));
                // No catch-up burst. Reschedule from this frame's game clock.
                layer.next_time = now + layer.time.sample(random);
            }
            self.next_random = self.next_random.min(layer.next_time);
        }
    }

    pub fn reset(&mut self, out: &mut Vec<Action>) {
        for layer in self.loops.iter().rev() {
            out.push(Action::Stop {
                wave: layer.voice.wave.clone(),
                ambient: layer.voice.position.is_none(),
            });
        }
        let fade_seconds = self.fade_seconds;
        *self = Self {
            fade_seconds,
            ..Self::default()
        };
    }
}
