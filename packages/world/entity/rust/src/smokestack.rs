//! Source SDK env_smokestack server state. TF2 renders the first material only;
//! the numbered material series is a server precache dependency.
use crate::{Entity, EntityHandle, EntityWorld, ExternalClassBinding, Transform, Variant};
use crate::value::{convert_value, source_float, source_integer, source_vector};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Light {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameters {
    pub start_size: f32,
    pub end_size: f32,
    pub base_spread: f32,
    pub spread_speed: f32,
    pub speed: f32,
    pub rate: f32,
    pub jet_length: f32,
    pub twist: f32,
    pub roll: f32,
    pub wind: [f32; 3],
    pub material: String,
    pub ambient: Light,
    pub directional: Light,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Presentation {
    pub entity: EntityHandle,
    pub source: usize,
    pub transform: Transform,
    pub color: [u8; 4],
    pub emit: bool,
    pub parameters: Parameters,
}

#[derive(Clone, Debug)]
struct Stack {
    entity: EntityHandle,
    emit: bool,
    parameters: Parameters,
}

#[derive(Clone, Debug, Default)]
pub struct Systems(Vec<Stack>);

pub fn binding() -> ExternalClassBinding {
    ExternalClassBinding {
        classname: b"env_smokestack".to_vec(),
        inputs: ["TurnOn", "TurnOff", "Toggle", "JetLength", "SpreadSpeed", "Speed", "Rate"]
            .map(|name| name.as_bytes().to_vec()).to_vec(),
    }
}

impl Systems {
    /// Includes template-created entities and generation changes on restart.
    pub fn synchronize(&mut self, world: &EntityWorld) -> Result<(), std::str::Utf8Error> {
        self.0.retain(|stack| world.entity(stack.entity).is_some());
        for handle in world.live_handles() {
            let entity = world.entity(handle).expect("live entity");
            if !class(&entity.definition, b"env_smokestack") || self.0.iter().any(|stack| stack.entity == handle) { continue; }
            let definition = &entity.definition;
            let mut wind = [0.0; 3];
            let (mut wind_angle, mut wind_speed) = (0, 0);
            // Wind/WindAngle/WindSpeed are order-dependent KeyValue handlers.
            for pair in &definition.pairs {
                if pair.key.eq_ignore_ascii_case(b"Wind") { wind = source_vector(&pair.value); }
                else if pair.key.eq_ignore_ascii_case(b"WindAngle") || pair.key.eq_ignore_ascii_case(b"WindSpeed") {
                    if pair.key.eq_ignore_ascii_case(b"WindAngle") { wind_angle = source_integer(&pair.value); }
                    else { wind_speed = source_integer(&pair.value); }
                    let (sin, cos) = (wind_angle as f32).to_radians().sin_cos();
                    wind = [cos * wind_speed as f32, sin * wind_speed as f32, 0.0];
                }
            }
            let mut material = std::str::from_utf8(field(definition, b"SmokeMaterial"))?.replace('\\', "/").to_ascii_lowercase();
            if material.is_empty() { material = "particle/smokestack.vmt".into(); }
            else if !material.contains(".vmt") { material.push_str(".vmt"); }
            let stem = material.split(".vmt").next().unwrap();
            let material = stem.strip_prefix("materials/").unwrap_or(stem).to_owned();
            let mut parameters = Parameters {
                start_size: number(definition, b"StartSize"), end_size: number(definition, b"EndSize"),
                base_spread: number(definition, b"BaseSpread"), spread_speed: number(definition, b"SpreadSpeed"),
                speed: number(definition, b"Speed"), rate: number(definition, b"Rate"),
                jet_length: number(definition, b"JetLength"), twist: number(definition, b"Twist"),
                roll: number(definition, b"Roll"), wind, material, ambient: Light::default(), directional: Light::default(),
            };
            for light in world.live_handles().into_iter().filter_map(|handle| world.entity(handle)) {
                if !class(&light.definition, b"env_particlelight") || field(&light.definition, b"PSName") != entity.targetname.as_deref().unwrap_or_default() { continue; }
                let info = if source_integer(field(&light.definition, b"Directional")) != 0 { &mut parameters.directional } else { &mut parameters.ambient };
                let color = field(&light.definition, b"Color");
                let intensity = field(&light.definition, b"Intensity");
                *info = Light { position: light.world_transform.origin,
                    color: if color.is_empty() { [1.0 / 255.0, 0.0, 0.0] } else { source_vector(color).map(|v| v / 255.0) },
                    intensity: if intensity.is_empty() { 5000.0 } else { source_float(intensity) } };
            }
            self.0.push(Stack { entity: handle, emit: source_integer(field(definition, b"InitialState")) != 0, parameters });
        }
        Ok(())
    }

    pub fn input(&mut self, entity: EntityHandle, input: &[u8], value: &Variant) {
        let Some(stack) = self.0.iter_mut().find(|stack| stack.entity == entity) else { return; };
        if input.eq_ignore_ascii_case(b"TurnOn") { stack.emit = true; }
        else if input.eq_ignore_ascii_case(b"TurnOff") { stack.emit = false; }
        else if input.eq_ignore_ascii_case(b"Toggle") { stack.emit = !stack.emit; }
        else if let Ok(Variant::Float(bits)) = convert_value(value, crate::FieldType::Float, |_| None, |_| None) {
            let target = if input.eq_ignore_ascii_case(b"JetLength") { &mut stack.parameters.jet_length }
                else if input.eq_ignore_ascii_case(b"SpreadSpeed") { &mut stack.parameters.spread_speed }
                else if input.eq_ignore_ascii_case(b"Speed") { &mut stack.parameters.speed }
                else if input.eq_ignore_ascii_case(b"Rate") { &mut stack.parameters.rate }
                else { return; };
            *target = f32::from_bits(bits);
        }
    }

    pub fn presentation(&self, world: &EntityWorld) -> Vec<Presentation> {
        self.0.iter().filter_map(|stack| {
            let entity = world.entity(stack.entity)?;
            Some(Presentation { entity: stack.entity, source: entity.source_index, transform: entity.world_transform,
                color: entity.render.color, emit: stack.emit, parameters: stack.parameters.clone() })
        }).collect()
    }
}

fn class(entity: &Entity, name: &[u8]) -> bool { entity.classname.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(name)) }
fn field<'a>(entity: &'a Entity, name: &[u8]) -> &'a [u8] {
    entity.pairs.iter().rev().find(|pair| pair.key.eq_ignore_ascii_case(name)).map_or(&[], |pair| pair.value.as_slice())
}
fn number(entity: &Entity, name: &[u8]) -> f32 { source_float(field(entity, name)) }
