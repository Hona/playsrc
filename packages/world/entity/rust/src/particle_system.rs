//! Server-side info_particle_system activation and follow-origin control points.
use crate::{Entity, EntityHandle, EntityWorld, ExternalClassBinding, Transform, source_integer};

#[derive(Clone, Debug, PartialEq)]
pub struct ControlPoint {
    pub index: u8,
    pub parent: u8,
    pub entity: EntityHandle,
    pub transform: Transform,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Presentation {
    pub source: usize,
    pub entity: EntityHandle,
    pub definition: String,
    pub active: bool,
    pub started_seconds: f32,
    pub weather: bool,
    pub controls: Vec<ControlPoint>,
}

#[derive(Clone, Debug)]
struct System {
    entity: EntityHandle,
    definition: String,
    active: bool,
    started_seconds: f32,
    weather: bool,
    controls: Vec<(u8, u8, EntityHandle)>,
}

#[derive(Clone, Debug, Default)]
pub struct Systems(Vec<System>);

pub fn binding() -> ExternalClassBinding {
    ExternalClassBinding { classname: b"info_particle_system".to_vec(), inputs: vec![b"Start".to_vec(), b"Stop".to_vec()] }
}

impl Systems {
    pub fn from_world(world: &EntityWorld, now: f32) -> Self {
        let mut systems = Self::default();
        for entity in world.live_handles() {
            let state = world.entity(entity).expect("live entity");
            if !state.definition.classname.as_deref().is_some_and(|class| class.eq_ignore_ascii_case(b"info_particle_system")) { continue; }
            let mut system = System { entity, definition: String::from_utf8_lossy(field(&state.definition, b"effect_name")).into_owned(),
                active: false, started_seconds: now, weather: source_integer(field(&state.definition, b"flag_as_weather")) != 0, controls: Vec::new() };
            if source_integer(field(&state.definition, b"start_active")) != 0 { system.start(world, now); }
            systems.0.push(system);
        }
        systems
    }

    pub fn input(&mut self, world: &EntityWorld, entity: EntityHandle, input: &[u8], now: f32) {
        let Some(system) = self.0.iter_mut().find(|system| system.entity == entity) else { return; };
        if input.eq_ignore_ascii_case(b"Start") { system.start(world, now); }
        else if input.eq_ignore_ascii_case(b"Stop") { system.active = false; }
    }

    pub fn presentation(&self, world: &EntityWorld) -> Vec<Presentation> {
        self.0.iter().filter_map(|system| {
            let state = world.entity(system.entity)?;
            let mut controls = vec![ControlPoint { index: 0, parent: 0, entity: system.entity, transform: state.world_transform }];
            controls.extend(system.controls.iter().filter_map(|&(index, parent, entity)| {
                Some(ControlPoint { index, parent, entity, transform: world.entity(entity)?.world_transform })
            }));
            Some(Presentation { source: state.source_index, entity: system.entity, definition: system.definition.clone(),
                active: system.active, started_seconds: system.started_seconds, weather: system.weather, controls })
        }).collect()
    }
}

impl System {
    fn start(&mut self, world: &EntityWorld, now: f32) {
        if self.active { return; }
        self.active = true;
        self.started_seconds = now;
        let Some(state) = world.entity(self.entity) else { return; };
        // Resolve at Start, not at map parse: the target may have been spawned later.
        for index in 1..=63 {
            let name = field(&state.definition, format!("cpoint{index}").as_bytes());
            if name.is_empty() { continue; }
            if let Some(entity) = world.resolve(name, Some(self.entity), None, None).first().copied() {
                let parent = if index <= 7 { source_integer(field(&state.definition, format!("cpoint{index}_parent").as_bytes())) as u8 } else { 0 };
                if let Some(control) = self.controls.iter_mut().find(|control| control.0 == index) { *control = (index, parent, entity); }
                else { self.controls.push((index, parent, entity)); }
            }
        }
    }
}

fn field<'a>(entity: &'a Entity, name: &[u8]) -> &'a [u8] {
    entity.pairs.iter().find(|pair| pair.key.eq_ignore_ascii_case(name)).map_or(&[], |pair| pair.value.as_slice())
}
