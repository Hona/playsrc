//! TF2 model-preview choreography. Presentation time is independent of gameplay ticks.

#[derive(Debug)]
pub struct SceneEvent {
    pub kind: u8,
    pub start: f32,
    pub end: f32,
    pub parameters: [&'static str; 3],
    pub ramp: &'static [(f32, f32)],
    pub expression: &'static [(&'static str, f32, f32)],
}

pub struct SceneDefinition {
    pub model: &'static str,
    pub path: &'static str,
    pub held_model: &'static str,
    pub events: &'static [SceneEvent],
}

include!("class_selection.generated.rs");

pub fn scene_for_model(model: &str) -> Option<&'static SceneDefinition> {
    CLASS_SELECTION_SCENES
        .iter()
        .find(|scene| scene.model.eq_ignore_ascii_case(model))
}

#[derive(Debug, Default)]
pub struct ScenePlayer {
    elapsed: f32,
    current: f32,
    time: f32,
    processing: Vec<bool>,
    sequence: Option<usize>,
    sequence_began: f32,
    event_sequence: Option<usize>,
    event_cycle: f32,
}

#[derive(Debug)]
pub struct SceneSample {
    pub sequence: Option<&'static str>,
    pub sequence_elapsed: f32,
    pub scene_time: f32,
    pub controllers: std::collections::BTreeMap<&'static str, f32>,
    pub event_sequence: Option<&'static str>,
    pub event_elapsed: f32,
}

impl ScenePlayer {
    pub fn advance(
        &mut self,
        scene: &'static SceneDefinition,
        elapsed: f32,
    ) -> Result<SceneSample, &'static str> {
        if !elapsed.is_finite() || elapsed < self.elapsed {
            return Err("class-select model clock reversed");
        }
        let event_sequence = self.sequence.map(|index| scene.events[index].parameters[0]);
        let event_elapsed = elapsed - self.sequence_began;
        if self.processing.len() != scene.events.len() {
            self.processing = vec![false; scene.events.len()];
        }
        // SetupFlexWeights performs two Think calls at the current scene time,
        // then advances by the model-panel clock. Sequence starts use that clock.
        for _ in 0..2 {
            let mut target = self.time;
            for (index, event) in scene
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind == 12)
            {
                if !self.processing[index] && event.start >= self.current && event.start <= target {
                    let back = event.parameters[0]
                        .parse::<f32>()
                        .expect("validated class-select loop target");
                    self.sequence_began += self.time - back;
                    self.current = back;
                    self.time = back;
                    target = back;
                    self.processing[index] = true;
                } else if self.processing[index] && self.current != event.start {
                    self.processing[index] = false;
                }
            }
            let mut starts = Vec::new();
            for (index, event) in scene
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| e.kind != 12)
            {
                let inside = self.current >= event.start && self.current <= event.end;
                if inside && !self.processing[index] {
                    starts.push(index);
                }
                self.processing[index] = inside;
            }
            starts.sort_by(|a, b| {
                scene.events[*a]
                    .start
                    .total_cmp(&scene.events[*b].start)
                    .then(a.cmp(b))
            });
            for index in starts {
                if scene.events[index].kind == 7 {
                    self.sequence = Some(index);
                    self.sequence_began = elapsed;
                }
            }
            self.current = target;
        }
        let scene_time = self.time;
        self.time += if self.elapsed < f32::EPSILON {
            0.1
        } else {
            elapsed - self.elapsed
        };
        self.time = self.time.max(-0.1);
        self.elapsed = elapsed;
        Ok(SceneSample {
            sequence: self.sequence.map(|index| scene.events[index].parameters[0]),
            sequence_elapsed: elapsed - self.sequence_began,
            scene_time,
            controllers: expression_controllers(scene, scene_time),
            event_sequence,
            event_elapsed,
        })
    }

    pub fn event_range(&mut self, sequence: usize, elapsed_cycle: f32) -> (f32, f32) {
        let (previous, current) = if self.event_sequence == Some(sequence) {
            (self.event_cycle, elapsed_cycle.min(1.0))
        } else {
            (-0.01, 0.0)
        };
        self.event_sequence = Some(sequence);
        self.event_cycle = current;
        (previous, current)
    }
}

fn expression_controllers(
    scene: &SceneDefinition,
    time: f32,
) -> std::collections::BTreeMap<&'static str, f32> {
    let mut result = std::collections::BTreeMap::new();
    let mut events = scene
        .events
        .iter()
        .filter(|event| event.kind == 2 && time >= event.start && time <= event.end)
        .collect::<Vec<_>>();
    events.sort_by(|left, right| left.start.total_cmp(&right.start));
    for event in events {
        let intensity = ramp_intensity(event.ramp, time - event.start, event.end - event.start);
        for &(name, weight, influence) in event.expression {
            let value = result.entry(name).or_insert(0.0);
            let scale = (intensity * influence).clamp(0.0, 1.0);
            *value = *value * (1.0 - scale) + weight * scale;
        }
    }
    result
}

fn ramp_intensity(samples: &[(f32, f32)], time: f32, duration: f32) -> f32 {
    if samples.is_empty() {
        return 1.0;
    }
    let sample = |index: isize| {
        if index < 0 {
            (0.0, 0.0)
        } else {
            samples
                .get(index as usize)
                .copied()
                .unwrap_or((duration, 0.0))
        }
    };
    let index = samples
        .iter()
        .position(|s| s.0 >= time)
        .unwrap_or(samples.len()) as isize
        - 1;
    let (start, p1) = sample(index);
    let (end, p2) = sample(index + 1);
    let p0 = sample(index - 1).1;
    let p3 = sample(index + 2).1;
    let t = if end > start {
        ((time - start) / (end - start)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t))
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_clock_and_event_delivery_follow_the_two_scene_thinks() {
        let scene = scene_for_model("models/player/scout.mdl").unwrap();
        let mut player = ScenePlayer::default();
        let first = player.advance(scene, 1.0).unwrap();
        assert_eq!(first.sequence, Some("selectionMenu_Anim01"));
        assert_eq!(first.sequence_elapsed, 0.0);
        assert_eq!(first.scene_time, 0.0);
        assert_eq!(first.event_sequence, None);
        assert!(!first.controllers.is_empty());
        let second = player.advance(scene, 1.015).unwrap();
        assert_eq!(second.scene_time, 0.1);
        assert_eq!(second.event_sequence, first.sequence);
        assert!((second.event_elapsed - 0.015).abs() < 0.000001);
        assert_eq!(player.event_range(5, 0.25), (-0.01, 0.0));
        assert_eq!(player.event_range(5, 0.5), (0.0, 0.5));
        assert_eq!(player.event_range(6, 0.5), (-0.01, 0.0));
        assert!(player.advance(scene, 0.0).is_err());
    }
    #[test]
    fn configured_loop_retains_idle_layer_and_does_not_replay_the_intro() {
        let scene = scene_for_model("models/player/scout.mdl").unwrap();
        let mut player = ScenePlayer::default();
        let mut prior = 0.0;
        let mut loops = 0;
        for frame in 0..1000 {
            let sample = player.advance(scene, frame as f32 * 0.015).unwrap();
            if sample.scene_time < prior {
                loops += 1;
                assert_eq!(sample.scene_time, 2.66);
                assert_eq!(sample.sequence, Some("selectionMenu_Idle"));
            }
            prior = sample.scene_time;
        }
        assert_eq!(loops, 4);
    }
    #[test]
    fn authored_ramps_use_catmull_rom_and_zero_valued_endpoints() {
        assert_eq!(ramp_intensity(&[], 0.3, 1.0), 1.0);
        assert_eq!(ramp_intensity(&[(0.5, 1.0)], 0.0, 1.0), 0.0);
        assert_eq!(ramp_intensity(&[(0.5, 1.0)], 0.25, 1.0), 0.5625);
        assert_eq!(ramp_intensity(&[(0.5, 1.0)], 0.5, 1.0), 1.0);
    }
}
