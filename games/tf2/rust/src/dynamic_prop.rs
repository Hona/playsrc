//! Authored dynamic-prop sequence selection, independent of brush pusher motion.
use playsrc_studio_model::{Float32, PresentationModel, sequence_timing};
use std::sync::Arc;

#[derive(Clone, Debug)]
struct Sequence {
    name: Vec<u8>,
    activity: Vec<u8>,
    entry_node: i32,
    timing: Result<(f32, bool), ()>,
    bounds: [[f32; 3]; 2],
}

#[derive(Clone, Debug)]
pub(crate) struct Definition(Vec<Sequence>);

impl Definition {
    pub fn compile(model: &PresentationModel) -> Self {
        let poses = vec![Float32(0); model.pose_parameters.len()];
        let sequences = model
            .sequences
            .iter()
            .map(|sequence| {
                let timing = sequence_timing(model, sequence.index, &poses)
                    .map(|timing| (f32::from_bits(timing.cycles_per_second.0), timing.looping))
                    .map_err(|_| ());
                Sequence {
                    name: sequence.label.clone(),
                    activity: sequence.activity_name.clone(),
                    entry_node: sequence.entry_node,
                    timing,
                    bounds: [
                        sequence.bounds_min.0.map(|value| f32::from_bits(value.0)),
                        sequence.bounds_max.0.map(|value| f32::from_bits(value.0)),
                    ],
                }
            })
            .collect();
        Self(sequences)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Animation {
    definition: Arc<Definition>,
    pub sequence: usize,
    pub started: f32,
    pub next_think: Option<f32>,
    pub last_think: f32,
    pub cycle: f32,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationPresentation {
    pub source_index: u32,
    pub sequence: Vec<u8>,
    pub elapsed_seconds: f32,
    pub bounds: [[f32; 3]; 2],
}

impl Animation {
    pub fn new(definition: Arc<Definition>) -> Self {
        Self {
            definition,
            sequence: 0,
            started: 0.0,
            next_think: None,
            last_think: 0.0,
            cycle: 0.0,
            active: false,
        }
    }

    pub fn set(&mut self, name: &[u8], now: f32) -> Result<bool, ()> {
        let sequence = self
            .definition
            .0
            .iter()
            .position(|seq| seq.name.eq_ignore_ascii_case(name));
        if sequence.is_none()
            && self
                .definition
                .0
                .iter()
                .any(|seq| seq.activity.eq_ignore_ascii_case(name))
        {
            // Activity selection needs the Source weighted-random authority;
            // do not substitute the first matching sequence for that draw.
            return Err(());
        }
        let Some(sequence) = sequence else {
            self.sequence = 0;
            return Ok(false);
        };
        // GotoSequence's node-zero path is the authored contract of these
        // models. Never silently replace a transition graph with a restart.
        if self.definition.0[self.sequence].entry_node != 0
            && self.definition.0[sequence].entry_node != 0
        {
            return Err(());
        }
        // Source asks for the selected sequence's rate when animation runs,
        // not for every unrelated sequence when the prop enters the map.
        // Preserve an unavailable rate as an error when that sequence is used.
        self.definition.0[sequence].timing?;
        self.sequence = sequence;
        self.cycle = 0.0;
        self.started = now;
        self.last_think = now;
        self.active = true;
        if self.next_think.is_none_or(|think| think <= now) {
            self.next_think = Some(now + 0.1);
        }
        Ok(true)
    }

    pub fn think(&mut self, now: f32) -> Result<bool, ()> {
        if self.next_think.is_none_or(|think| think > now) {
            return Ok(false);
        }
        self.next_think = None;
        let timing = &self.definition.0[self.sequence];
        let (cycles_per_second, looping) = timing.timing?;
        let done = self.cycle >= 0.999 && !looping;
        if !done {
            self.next_think = Some(now + 0.1);
        }
        let interval = (now - self.last_think).clamp(0.0, 0.2);
        if interval <= 0.001 {
            return Ok(done);
        }
        self.last_think = now;
        self.cycle += interval * cycles_per_second;
        if looping {
            self.cycle -= self.cycle.floor();
        } else {
            self.cycle = self.cycle.clamp(0.0, 1.0);
        }
        Ok(done)
    }

    pub fn presentation(&self, source_index: u32, now: f32) -> Option<AnimationPresentation> {
        self.active.then(|| AnimationPresentation {
            source_index,
            sequence: self.definition.0[self.sequence].name.clone(),
            elapsed_seconds: (now - self.started).max(0.0),
            bounds: self.definition.0[self.sequence].bounds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn door() -> Animation {
        Animation::new(Arc::new(Definition(
            [b"idle".as_slice(), b"open", b"close"]
                .into_iter()
                .map(|name| Sequence {
                    name: name.to_vec(),
                    activity: Vec::new(),
                    entry_node: 0,
                    timing: Ok((1.0 / 0.3, false)),
                    bounds: [[-128.0, -4.0, -64.0], [128.0, 0.0, 192.0]],
                })
                .collect(),
        )))
    }

    #[test]
    fn authored_sequences_restart_retrigger_reverse_and_complete_on_animthink_not_door_distance() {
        let mut animation = door();
        assert!(animation.presentation(537, 0.0).is_none());
        assert!(animation.set(b"open", 1.0).unwrap());
        assert!(!animation.think(1.09).unwrap());
        assert_eq!(animation.cycle, 0.0);
        assert!(!animation.think(1.11).unwrap());
        assert!((animation.cycle - 0.11 / 0.3).abs() < 0.0001);
        assert!(animation.set(b"close", 1.15).unwrap());
        assert_eq!(animation.sequence, 2);
        assert_eq!(animation.cycle, 0.0);
        assert!(animation.set(b"close", 1.16).unwrap());
        assert_eq!(animation.started, 1.16);
        for now in [1.23, 1.34, 1.45, 1.56] {
            assert!(!animation.think(now).unwrap());
        }
        assert_eq!(animation.cycle, 1.0);
        assert!(animation.think(1.67).unwrap());
        assert!(!animation.think(1.78).unwrap());
        assert_eq!(
            animation.presentation(537, 1.78).unwrap().sequence,
            b"close"
        );
    }

    #[test]
    fn clone_is_independent_and_missing_transition_graphs_are_not_replaced_by_restarts() {
        let mut animation = door();
        animation.set(b"open", 0.0).unwrap();
        let saved = animation.clone();
        animation.set(b"close", 0.1).unwrap();
        assert_eq!(saved.sequence, 1);
        assert_eq!(animation.sequence, 2);
        assert!(!animation.set(b"absent", 0.2).unwrap());
        assert_eq!(animation.sequence, 0);
        let definition = Arc::make_mut(&mut animation.definition);
        definition.0[0].entry_node = 1;
        definition.0[1].entry_node = 2;
        assert!(animation.set(b"open", 0.3).is_err());
    }

    #[test]
    fn unrelated_unavailable_sequence_rate_does_not_activate_or_reject_a_fixed_prop() {
        let mut animation = door();
        Arc::make_mut(&mut animation.definition).0[2].timing = Err(());
        assert!(!animation.think(10.0).unwrap());
        assert!(animation.presentation(1, 10.0).is_none());
        assert!(animation.set(b"open", 10.0).unwrap());
        assert!(animation.set(b"close", 10.1).is_err());
        assert_eq!(animation.sequence, 1);
        assert!(!animation.think(10.11).unwrap());
    }
}
