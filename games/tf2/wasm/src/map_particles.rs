use std::collections::BTreeMap;
use playsrc_entity::{EntityHandle, particle_system::Presentation};
use playsrc_particle::{AdvanceRequest, ControlPoint, Event, EventCommand, ParticleWorld, StopMode};

#[derive(Clone, Default)]
pub struct MapParticles {
    active: BTreeMap<EntityHandle, (u32, Vec<ControlPoint>)>,
    next_effect: u32,
    next_event: u64,
    sky: BTreeMap<u32, bool>,
}

impl MapParticles {
    pub fn prepare(&mut self, states: &[Presentation], world: &ParticleWorld, request: AdvanceRequest, is_sky: impl Fn([f32; 3]) -> bool)
        -> (Vec<Event>, Vec<(u32, ControlPoint)>) {
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
                let clock = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut super::RuntimeMetricsClock::new());
                self.event(&mut events, now, EventCommand::Create { effect_identity: identity, definition: state.definition.clone(),
                    seed: clock, owner_identity: None, control_points: controls.clone() });
                self.active.insert(state.entity, (identity, controls));
            }
        }
        events.sort_by(|left, right| left.timestamp_seconds.total_cmp(&right.timestamp_seconds).then(left.source_order.cmp(&right.source_order)));
        (events, attachments)
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

fn quaternion(angles: [f32; 3]) -> [f32; 4] {
    let (pitch, yaw, roll) = (angles[0].to_radians() * 0.5, angles[1].to_radians() * 0.5, angles[2].to_radians() * 0.5);
    let (sp, cp) = pitch.sin_cos(); let (sy, cy) = yaw.sin_cos(); let (sr, cr) = roll.sin_cos();
    [sr * cp * cy - cr * sp * sy, cr * sp * cy + sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy]
}
