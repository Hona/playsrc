use crate::{AnimationLayer, Float32, PresentationError, PresentationErrorCode, PresentationModel, sequence_timing};

#[derive(Clone, Debug)]
struct Entry {
    sequence: usize,
    cycle: f32,
    time: f32,
    fade_out: f32,
    parity: u8,
}

/// C_BaseAnimating sequence transitions, distinct from studio frame interpolation.
#[derive(Clone, Debug, Default)]
pub struct SequenceTransitioner {
    queue: Vec<Entry>,
}

impl SequenceTransitioner {
    pub fn clear(&mut self) { self.queue.clear(); }
    pub fn is_transitioning(&self) -> bool { self.queue.len() > 1 }

    /// `previous_paint_time` also accounts for paints that reused a terminal pose.
    /// Finished sequences still participate in the next authored cross-fade.
    pub fn update(
        &mut self,
        model: &PresentationModel,
        sequence: usize,
        cycle: f32,
        time: f32,
        previous_paint_time: f32,
        parameters: &[Float32],
        revision: u32,
    ) -> Result<Vec<AnimationLayer>, PresentationError> {
        let invalid = || PresentationError { code: PresentationErrorCode::InvalidState, identity: model.identity.clone() };
        if ![cycle, time, previous_paint_time].into_iter().all(f32::is_finite) || previous_paint_time > time {
            return Err(invalid());
        }
        let incoming = model.sequences.get(sequence).ok_or_else(invalid)?;
        let parity = (revision & 7) as u8;
        if let Some(current) = self.queue.last_mut() {
            if current.time < previous_paint_time {
                let timing = sequence_timing(model, current.sequence, parameters)?;
                current.cycle = advance(current.cycle, previous_paint_time - current.time, timing);
                current.time = previous_paint_time;
            }
            if current.time != 0.0 && (current.sequence != sequence || current.parity != parity) {
                if incoming.flags & 2 != 0 {
                    self.queue.clear();
                } else {
                    current.fade_out = f32::from_bits(model.sequences[current.sequence].fade_out.0)
                        .min(f32::from_bits(incoming.fade_in.0));
                }
                self.queue.push(Entry { sequence, cycle, time, fade_out: 0.0, parity });
            } else {
                *current = Entry { sequence, cycle, time, fade_out: 0.0, parity };
            }
        } else {
            self.queue.push(Entry { sequence, cycle, time, fade_out: 0.0, parity });
        }
        let last = self.queue.len() - 1;
        let mut index = 0;
        self.queue.retain(|entry| {
            let keep = index == last || fade(entry, time) > 0.0;
            index += 1;
            keep
        });
        self.queue[..self.queue.len() - 1].iter().rev().map(|entry| {
            let timing = sequence_timing(model, entry.sequence, parameters)?;
            Ok(AnimationLayer {
                sequence: entry.sequence,
                cycle: Float32(advance(entry.cycle, time - entry.time, timing).to_bits()),
                weight: Float32(fade(entry, time).to_bits()),
            })
        }).collect()
    }
}

fn advance(cycle: f32, elapsed: f32, timing: crate::SequenceTiming) -> f32 {
    let value = cycle + elapsed * f32::from_bits(timing.cycles_per_second.0);
    if timing.looping { value - value.floor() } else { value.clamp(0.0, 1.0) }
}

fn fade(entry: &Entry, time: f32) -> f32 {
    if entry.fade_out <= 0.0 { return 0.0; }
    let s = 1.0 - (time - entry.time) / entry.fade_out;
    if s > 0.0 && s <= 1.0 { 3.0 * s * s - 2.0 * s * s * s } else { s.min(1.0) }
}
