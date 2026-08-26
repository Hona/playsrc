//! Lossless local snapshot transport. The game snapshot remains the authority;
//! this layer only replaces equal byte runs with a reference to the acknowledged tick.
use playsrc_simulation::Publication;
use std::sync::Arc;

const MAX_SNAPSHOT: usize = 64 * 1024 * 1024;
const MAX_OUTPUT: usize = 512 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct Encoder {
    tick: u64,
    bytes: Arc<[u8]>,
}

impl Encoder {
    pub fn encode(&mut self, publications: &[Publication], acknowledged: u64) -> Option<Vec<u8>> {
        let mut candidate = self.clone();
        // A missing/different acknowledgement requests an authoritative full restore,
        // not a replay or a coalescing of any intervening event batches.
        if candidate.tick != acknowledged {
            candidate.bytes = Arc::from([]);
        }
        let mut output = b"PSIM".to_vec();
        output.extend_from_slice(&3_u32.to_le_bytes());
        output.extend_from_slice(&u32::try_from(publications.len()).ok()?.to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        for publication in publications {
            if publication.events.last().is_none_or(|event| {
                !Arc::ptr_eq(&event.bytes, &publication.snapshot)
                    && event.bytes != publication.snapshot
            }) {
                return None;
            }
            output.extend_from_slice(&publication.host_frame.to_le_bytes());
            output.extend_from_slice(&publication.first_host_tick.to_le_bytes());
            output.extend_from_slice(&publication.last_host_tick.to_le_bytes());
            output.extend_from_slice(&publication.selected_ticks.to_le_bytes());
            output.extend_from_slice(&publication.interpolation.to_le_bytes());
            output.extend_from_slice(
                &u32::try_from(publication.snapshot.len())
                    .ok()?
                    .to_le_bytes(),
            );
            output.extend_from_slice(&u32::try_from(publication.events.len()).ok()?.to_le_bytes());
            for event in &publication.events {
                if event.host_tick != candidate.tick.checked_add(1)?
                    || event.bytes.is_empty()
                    || event.bytes.len() > MAX_SNAPSHOT
                {
                    return None;
                }
                let delta = delta(&candidate.bytes, &event.bytes);
                let (base, wire) = match &delta {
                    Some(bytes) => (candidate.tick, bytes.as_slice()),
                    None => (0, event.bytes.as_ref()),
                };
                if output.len().checked_add(24)?.checked_add(wire.len())? > MAX_OUTPUT {
                    return None;
                }
                output.extend_from_slice(&event.host_tick.to_le_bytes());
                output.extend_from_slice(&(event.bytes.len() as u32).to_le_bytes());
                output.extend_from_slice(&(wire.len() as u32).to_le_bytes());
                output.extend_from_slice(&base.to_le_bytes());
                output.extend_from_slice(wire);
                candidate.tick = event.host_tick;
                candidate.bytes = event.bytes.clone();
            }
        }
        *self = candidate;
        Some(output)
    }
}

// Sorted non-overlapping replacement runs. Comparing bytes rather than floats
// preserves signed zero and every serialized bit. Short equal gaps are included
// in a run when doing so is cheaper than another eight-byte run header.
fn delta(previous: &[u8], current: &[u8]) -> Option<Vec<u8>> {
    if previous.is_empty() || previous.len() != current.len() {
        return None;
    }
    let mut output = Vec::new();
    let mut at = 0;
    while at < current.len() {
        while at < current.len() && current[at] == previous[at] {
            at += 1;
        }
        if at == current.len() {
            break;
        }
        let start = at;
        let mut end = at + 1;
        at += 1;
        while at < current.len() && at - end < 8 {
            if current[at] != previous[at] {
                end = at + 1;
            }
            at += 1;
        }
        if output.len() + 8 + end - start >= current.len() {
            return None;
        }
        output.extend_from_slice(&(start as u32).to_le_bytes());
        output.extend_from_slice(&((end - start) as u32).to_le_bytes());
        output.extend_from_slice(&current[start..end]);
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_simulation::EventBatch;

    fn restore(previous: &[u8], wire: &[u8]) -> Vec<u8> {
        let mut output = previous.to_vec();
        let mut at = 0;
        while at < wire.len() {
            let start = u32::from_le_bytes(wire[at..at + 4].try_into().unwrap()) as usize;
            let length = u32::from_le_bytes(wire[at + 4..at + 8].try_into().unwrap()) as usize;
            at += 8;
            output[start..start + length].copy_from_slice(&wire[at..at + length]);
            at += length;
        }
        output
    }

    #[test]
    fn randomized_lossless_runs_and_signed_zero() {
        let mut seed = 0xa74e_f127_u32;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for size in [1, 8, 16, 127, 8192, 72_000] {
            let mut prior: Vec<u8> = (0..size).map(|_| random() as u8).collect();
            for _ in 0..100 {
                let mut current = prior.clone();
                for _ in 0..(random() as usize % size).min(256) {
                    current[random() as usize % size] = random() as u8;
                }
                if let Some(wire) = delta(&prior, &current) {
                    assert!(wire.len() < current.len());
                    assert_eq!(restore(&prior, &wire), current);
                }
                prior = current;
            }
        }
        let positive = [0_u8; 64];
        let mut negative = positive;
        negative[..4].copy_from_slice(&(-0.0_f32).to_le_bytes());
        assert_eq!(
            restore(&positive, &delta(&positive, &negative).unwrap()),
            negative
        );
        assert!(delta(&positive, &[0; 63]).is_none());
    }

    fn publication(tick: u64, bytes: Arc<[u8]>) -> Publication {
        Publication {
            host_frame: tick,
            first_host_tick: tick,
            last_host_tick: tick,
            selected_ticks: 1,
            interpolation: 0.0,
            snapshot: bytes.clone(),
            events: vec![EventBatch {
                host_tick: tick,
                bytes,
            }],
        }
    }

    #[test]
    fn acknowledgements_full_restore_order_and_transactional_failure() {
        let mut encoder = Encoder::default();
        let bytes: Arc<[u8]> = Arc::from([7_u8; 72_000]);
        let first = encoder.encode(&[publication(1, bytes.clone())], 0).unwrap();
        assert_eq!(first.len(), 80 + bytes.len());
        let second = encoder.encode(&[publication(2, bytes.clone())], 1).unwrap();
        assert_eq!(second.len(), 80);
        assert_eq!(u64::from_le_bytes(second[72..80].try_into().unwrap()), 1);
        assert!(
            encoder
                .encode(&[publication(4, bytes.clone())], 2)
                .is_none()
        );
        assert_eq!(encoder.tick, 2);
        let restored = encoder.encode(&[publication(3, bytes)], 0).unwrap();
        assert_eq!(restored.len(), first.len());
        assert_eq!(&restored[80..], &first[80..]);
        assert!(encoder.encode(&[], 3).is_some());
        assert_eq!(encoder.tick, 3);
    }
}
