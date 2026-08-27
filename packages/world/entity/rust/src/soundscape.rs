//! Server-side soundscape selection, proxy binding and trigger stack.
//! Source SDK 2013 `game/server/soundscape.cpp` and `soundscape_system.cpp`.
use crate::{Entity, EntityHandle, EntityWorld, ExternalClassBinding, source_integer};

pub type Position = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub entity: i32,
    pub soundscape: i32,
    pub positions: [Position; 8],
    pub position_bits: u8,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            entity: 0,
            soundscape: -1,
            positions: [[0.0; 3]; 8],
            position_bits: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Zone {
    pub entity: EntityHandle,
    pub radius: f32,
    pub soundscape: i32,
    pub disabled: bool,
    pub triggerable: bool,
    positions: [Vec<u8>; 8],
    identity: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub selection: Selection,
    triggers: Vec<EntityHandle>,
}

#[derive(Clone, Copy, Debug)]
pub struct Trace {
    pub fraction: f32,
    pub start_solid: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Systems {
    zones: Vec<Zone>,
    triggers: Vec<(EntityHandle, EntityHandle)>,
}

pub fn bindings() -> Vec<ExternalClassBinding> {
    [
        b"env_soundscape".as_slice(),
        b"env_soundscape_proxy",
        b"env_soundscape_triggerable",
    ]
    .into_iter()
    .map(|name| ExternalClassBinding {
        classname: name.to_vec(),
        inputs: [b"Enable".as_slice(), b"Disable", b"ToggleEnabled"]
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect(),
    })
    .collect()
}

impl Systems {
    pub fn from_world(
        world: &EntityWorld,
        mut resolve_soundscape: impl FnMut(&[u8]) -> Option<usize>,
    ) -> Self {
        let mut result = Self::default();
        let live = world.live_handles();
        for &entity in &live {
            let state = world.entity(entity).expect("live entity");
            if ![
                b"env_soundscape".as_slice(),
                b"env_soundscape_proxy",
                b"env_soundscape_triggerable",
            ]
            .iter()
            .any(|class| state.classname.eq_ignore_ascii_case(class))
            {
                continue;
            }
            let definition = &state.definition;
            result.zones.push(Zone {
                entity,
                radius: playsrc_number(field(definition, b"radius")),
                soundscape: resolve_soundscape(field(definition, b"soundscape"))
                    .map_or(-1, |index| index as i32),
                disabled: source_integer(field(definition, b"StartDisabled")) != 0,
                triggerable: state
                    .classname
                    .eq_ignore_ascii_case(b"env_soundscape_triggerable"),
                positions: std::array::from_fn(|index| {
                    field(definition, format!("position{index}").as_bytes()).to_vec()
                }),
                identity: result.zones.len() as i32 + 1,
            });
        }
        // Proxies copy names and index at Activate. They do not follow later
        // enabled/radius/transform changes of the referenced soundscape.
        for index in 0..result.zones.len() {
            let entity = result.zones[index].entity;
            let state = world.entity(entity).expect("live entity");
            if !state
                .classname
                .eq_ignore_ascii_case(b"env_soundscape_proxy")
            {
                continue;
            }
            let target = world
                .resolve(
                    field(&state.definition, b"MainSoundscapeName"),
                    None,
                    None,
                    None,
                )
                .first()
                .copied();
            if let Some(main) = result
                .zones
                .iter()
                .find(|zone| Some(zone.entity) == target)
                .cloned()
            {
                result.zones[index].soundscape = main.soundscape;
                result.zones[index].positions = main.positions;
            }
        }
        for entity in live {
            let state = world.entity(entity).expect("live entity");
            if !state.classname.eq_ignore_ascii_case(b"trigger_soundscape") {
                continue;
            }
            let target = world
                .resolve(field(&state.definition, b"soundscape"), None, None, None)
                .first()
                .copied();
            if let Some(zone) = result
                .zones
                .iter()
                .find(|zone| Some(zone.entity) == target && zone.triggerable)
            {
                result.triggers.push((entity, zone.entity));
            }
        }
        result
    }

    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    pub fn input(&mut self, entity: EntityHandle, input: &[u8]) {
        let Some(zone) = self.zones.iter_mut().find(|zone| zone.entity == entity) else {
            return;
        };
        if input.eq_ignore_ascii_case(b"Enable") {
            zone.disabled = false;
        } else if input.eq_ignore_ascii_case(b"Disable") {
            zone.disabled = true;
        } else if input.eq_ignore_ascii_case(b"ToggleEnabled") {
            zone.disabled = !zone.disabled;
        }
    }

    /// Invoke only for actual Source StartTouch/EndTouch, including the separate
    /// 0.2s dead/spectator intersection pass. Delegate precedes base-trigger I/O
    /// and does not test the soundscape's StartDisabled or the trigger filter.
    pub fn touch(
        &self,
        world: &EntityWorld,
        trigger: EntityHandle,
        enter: bool,
        player: &mut Player,
        on_play: &mut Vec<EntityHandle>,
    ) {
        let Some(&(_, entity)) = self.triggers.iter().find(|(handle, _)| *handle == trigger) else {
            return;
        };
        if world.entity(entity).is_none() {
            return;
        }
        if let Some(index) = player.triggers.iter().position(|handle| *handle == entity) {
            player.triggers.remove(index);
        }
        if enter {
            player.triggers.insert(0, entity);
            if let Some(zone) = self.zones.iter().find(|zone| zone.entity == entity) {
                zone.write(world, &mut player.selection, on_play);
            }
        } else {
            while let Some(&handle) = player.triggers.first() {
                if let Some(zone) = self.zones.iter().find(|zone| zone.entity == handle)
                    && world.entity(handle).is_some()
                {
                    zone.write(world, &mut player.selection, on_play);
                    return;
                }
                player.triggers.remove(0);
            }
            // Retain script and positions. A zero entity does not stop client loops.
            player.selection.entity = 0;
        }
    }

    /// Candidates must be in Source's precomputed per-cluster order (PVS plus
    /// strict radius/AABB intersection). The current zone is checked first even
    /// when absent from that list. The trace owner uses brush-solid plus water,
    /// excludes the listener and reports both fraction and startsolid.
    pub fn update(
        &self,
        world: &EntityWorld,
        player: &mut Player,
        ear: Position,
        candidates: &[usize],
        mut trace: impl FnMut(Position, Position) -> Trace,
        on_play: &mut Vec<EntityHandle>,
    ) -> usize {
        let mut current = self.zones.iter().position(|zone| {
            zone.identity == player.selection.entity && world.entity(zone.entity).is_some()
        });
        let mut in_range = false;
        let mut distance = 0.0;
        let mut traces = 0;
        if let Some(index) = current {
            let zone = &self.zones[index];
            if zone.disabled {
                current = None;
            } else {
                let origin = world.entity(zone.entity).unwrap().world_transform.origin;
                distance = length(ear, origin);
                if zone.radius > distance || zone.radius == -1.0 {
                    traces += 1;
                    let hit = trace(origin, ear);
                    in_range = hit.fraction == 1.0 && !hit.start_solid;
                }
            }
        }
        for &index in candidates {
            if Some(index) == current {
                continue;
            }
            let Some(zone) = self.zones.get(index) else {
                continue;
            };
            let Some(state) = world.entity(zone.entity) else {
                continue;
            };
            if zone.disabled {
                continue;
            }
            let origin = state.world_transform.origin;
            let range = length(ear, origin);
            if (!in_range || range < distance) && (zone.radius > range || zone.radius == -1.0) {
                traces += 1;
                let hit = trace(origin, ear);
                if hit.fraction == 1.0 && !hit.start_solid {
                    zone.write(world, &mut player.selection, on_play);
                    current = Some(index);
                    distance = range;
                    in_range = true;
                }
            }
        }
        traces
    }
}

impl Zone {
    fn write(
        &self,
        world: &EntityWorld,
        selection: &mut Selection,
        on_play: &mut Vec<EntityHandle>,
    ) {
        selection.entity = self.identity;
        selection.soundscape = self.soundscape;
        selection.position_bits = 0;
        for (index, name) in self.positions.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            if let Some(&entity) = world
                .resolve(name, Some(self.entity), Some(self.entity), None)
                .first()
                && let Some(state) = world.entity(entity)
            {
                selection.position_bits |= 1 << index;
                selection.positions[index] = state.world_transform.origin;
            }
        }
        on_play.push(self.entity);
    }
}

fn field<'a>(entity: &'a Entity, name: &[u8]) -> &'a [u8] {
    entity
        .pairs
        .iter()
        .rev()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map_or(&[], |pair| pair.value.as_slice())
}
fn playsrc_number(bytes: &[u8]) -> f32 {
    crate::value::source_float(bytes)
}
fn length(a: Position, b: Position) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
