use std::collections::BTreeMap;
use playsrc_entity::{EntityHandle, particle_system::Presentation};
use playsrc_particle::{AdvanceRequest, ControlPoint, Event, EventCommand, ParticleWorld, StopMode};
use std::sync::Arc;

#[derive(Clone)]
struct Entropy {
    entity: EntityHandle,
    definition: String,
    seed: u64,
}

#[derive(Clone, Default)]
pub struct MapParticles {
    active: BTreeMap<EntityHandle, (u32, Vec<ControlPoint>)>,
    next_effect: u32,
    next_event: u64,
    sky: BTreeMap<u32, bool>,
    entropy: Option<Vec<Entropy>>,
    entropy_inputs: Option<Arc<[Entropy]>>,
    entropy_cursor: usize,
}

impl MapParticles {
    pub fn prepare(&mut self, states: &[Presentation], world: &ParticleWorld, request: AdvanceRequest, is_sky: impl Fn([f32; 3]) -> bool)
        -> Result<(Vec<Event>, Vec<(u32, ControlPoint)>), ()> {
        let mut events = Vec::new();
        let mut attachments = Vec::new();
        self.sky.retain(|identity, _| world.has_effect(*identity));
        let removed: Vec<_> = self.active.keys().filter(|entity| !states.iter().any(|state| state.entity == **entity && state.active)).copied().collect();
        for entity in removed {
            let (identity, _) = self.active.remove(&entity).expect("active map effect");
            if world.has_effect(identity) {
                self.event(&mut events, request.to_seconds, EventCommand::StopEmission { effect_identity: identity, mode: StopMode::Graceful });
            }
        }
        for state in states.iter().filter(|state| state.active && !state.definition.is_empty()) {
            let mut controls: Vec<_> = state.controls.iter().map(|control| ControlPoint {
                index: control.index, position: control.transform.origin, previous_position: control.transform.origin,
                orientation: quaternion(control.transform.angles), velocity: [0.0; 3], radius: 0.0, density: 1.0, duration: 0.0,
                parent: (control.parent != 0).then_some(control.parent), object_identity: None,
            }).collect();
            if let Some((identity, previous)) = self.active.get_mut(&state.entity) {
                if world.has_effect(*identity) {
                    for control in &mut controls {
                        if let Some(prior) = previous.iter().find(|prior| prior.index == control.index) { control.previous_position = prior.position; }
                        attachments.push((*identity, control.clone()));
                    }
                    *previous = controls;
                }
            } else {
                self.next_effect += 1;
                let identity = self.next_effect;
                self.sky.insert(identity, controls.first().is_some_and(|control| is_sky(control.position)));
                let now = state.started_seconds.clamp(request.from_seconds, request.to_seconds);
                // Particle randomness is presentation-local, independent from gameplay prediction.
                let clock = self.next_seed(state.entity, &state.definition)?;
                self.event(&mut events, now, EventCommand::Create { effect_identity: identity, definition: state.definition.clone(),
                    seed: clock, owner_identity: None, control_points: controls.clone() });
                self.active.insert(state.entity, (identity, controls));
            }
        }
        events.sort_by(|left, right| left.timestamp_seconds.total_cmp(&right.timestamp_seconds).then(left.source_order.cmp(&right.source_order)));
        Ok((events, attachments))
    }

    pub fn record_entropy(&mut self) { self.entropy = Some(Vec::new()); }

    fn next_seed(&mut self, entity: EntityHandle, definition: &str) -> Result<u64, ()> {
        let seed = if let Some(inputs) = &self.entropy_inputs {
            let input = inputs.get(self.entropy_cursor).ok_or(())?;
            if input.entity != entity || input.definition != definition { return Err(()); }
            self.entropy_cursor += 1;
            input.seed
        } else {
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut super::RuntimeMetricsClock::new())
        };
        if let Some(records) = &mut self.entropy {
            if records.len() >= 4096 { return Err(()); }
            records.push(Entropy { entity, definition: definition.to_owned(), seed });
        }
        Ok(seed)
    }

    pub fn entropy_bytes(&self) -> Vec<u8> {
        let mut bytes = b"MPER\x01\0\0\0".to_vec();
        let records = self.entropy.as_deref().unwrap_or(&[]);
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for record in records {
            bytes.extend_from_slice(&record.entity.slot.to_le_bytes()); bytes.extend_from_slice(&[0, 0]);
            bytes.extend_from_slice(&record.entity.generation.to_le_bytes()); bytes.extend_from_slice(&record.seed.to_le_bytes());
            bytes.extend_from_slice(&(record.definition.len() as u32).to_le_bytes()); bytes.extend_from_slice(record.definition.as_bytes());
        }
        bytes
    }

    pub fn restore_entropy(&mut self, bytes: &[u8]) -> Result<(), ()> {
        if self.next_effect != 0 || !self.active.is_empty() || self.entropy_inputs.is_some() || bytes.len() > 4 * 1024 * 1024 { return Err(()); }
        let mut reader = super::ParticleReader { bytes, at: 0 };
        if reader.take(8)? != b"MPER\x01\0\0\0" { return Err(()); }
        let count = reader.u32()? as usize;
        if count > 4096 { return Err(()); }
        let mut inputs = Vec::with_capacity(count);
        for _ in 0..count {
            let slot = u16::from_le_bytes(reader.take(2)?.try_into().map_err(|_| ())?);
            if reader.take(2)? != [0, 0] { return Err(()); }
            let generation = reader.u32()?;
            let seed = reader.u64()?;
            let definition = reader.text()?;
            if definition.is_empty() || definition.len() > 1024 { return Err(()); }
            inputs.push(Entropy { entity: EntityHandle { slot, generation }, definition, seed });
        }
        if reader.at != bytes.len() { return Err(()); }
        self.entropy_inputs = Some(inputs.into());
        Ok(())
    }

    pub fn classify(&self, items: &mut [playsrc_particle::RenderItem]) {
        for item in items { item.sky = self.is_sky(item.effect_identity); }
    }

    pub fn is_sky(&self, identity: u32) -> bool { *self.sky.get(&identity).expect("map effect render ownership") }

    fn event(&mut self, events: &mut Vec<Event>, timestamp_seconds: f32, command: EventCommand) {
        self.next_event += 1;
        events.push(Event { identity: self.next_event, timestamp_seconds, source_order: events.len() as u32, command });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entropy_is_exact_ordered_and_rolls_back_with_its_transaction_owner() {
        let entity = EntityHandle { slot: 17, generation: 2 };
        let mut authored = MapParticles::default();
        authored.entropy = Some(vec![Entropy { entity, definition: "smoke".into(), seed: u64::MAX - 13 }]);
        let bytes = authored.entropy_bytes();
        let mut replay = MapParticles::default();
        replay.record_entropy(); replay.restore_entropy(&bytes).unwrap();
        assert!(replay.next_seed(entity, "another").is_err());
        assert_eq!(replay.entropy_cursor, 0);
        let mut rejected = replay.clone();
        assert_eq!(rejected.next_seed(entity, "smoke").unwrap(), u64::MAX - 13);
        assert_eq!(replay.entropy_cursor, 0);
        assert_eq!(replay.next_seed(entity, "smoke").unwrap(), u64::MAX - 13);
        assert_eq!(replay.entropy_bytes(), bytes);
        assert!(replay.next_seed(entity, "smoke").is_err());
        assert!(replay.restore_entropy(&bytes).is_err());
        for length in 0..bytes.len() { assert!(MapParticles::default().restore_entropy(&bytes[..length]).is_err()); }
    }
}

fn quaternion(angles: [f32; 3]) -> [f32; 4] {
    let (pitch, yaw, roll) = (angles[0].to_radians() * 0.5, angles[1].to_radians() * 0.5, angles[2].to_radians() * 0.5);
    let (sp, cp) = pitch.sin_cos(); let (sy, cy) = yaw.sin_cos(); let (sr, cr) = roll.sin_cos();
    [sr * cp * cy - cr * sp * sy, cr * sp * cy + sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy]
}
