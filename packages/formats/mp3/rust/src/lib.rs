//! Buffered MPEG audio decoding. Resource resolution remains with Content.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InputLimit,
    OutputLimit,
    InvalidStream,
    FormatChange,
}

#[derive(Clone, Debug)]
pub struct Decoded {
    pub sample_rate: u32,
    pub channels: u8,
    pub samples: Vec<i16>,
}

pub fn decode(bytes: &[u8], max_input: usize, max_samples: usize) -> Result<Decoded, Error> {
    if bytes.len() > max_input {
        return Err(Error::InputLimit);
    }
    let mut decoder = nanomp3::Decoder::new();
    let mut pcm = [0.0; nanomp3::MAX_SAMPLES_PER_FRAME];
    let mut output = Decoded {
        sample_rate: 0,
        channels: 0,
        samples: Vec::new(),
    };
    let mut cursor = 0;
    while cursor < bytes.len() {
        let end = cursor.saturating_add(16 * 1024).min(bytes.len());
        let (consumed, info) = decoder.decode(&bytes[cursor..end], &mut pcm);
        if consumed == 0 || consumed > end - cursor {
            break;
        }
        cursor += consumed;
        let Some(info) = info else {
            continue;
        };
        if output.channels == 0 {
            output.channels = info.channels.num();
            output.sample_rate = info.sample_rate;
        } else if info.channels.num() != output.channels || info.sample_rate != output.sample_rate {
            return Err(Error::FormatChange);
        }
        let count = info.samples_produced * usize::from(info.channels.num());
        if output
            .samples
            .len()
            .checked_add(count)
            .is_none_or(|total| total > max_samples)
        {
            return Err(Error::OutputLimit);
        }
        for (index, sample) in pcm[..count].iter().enumerate() {
            output.samples.push(pcm_sample(
                *sample,
                (index / usize::from(output.channels)) % 16 == 0,
            ));
        }
    }
    if output.samples.is_empty() {
        return Err(Error::InvalidStream);
    }
    Ok(output)
}

// minimp3's PC synthesis uses scalar quantization for the pair at positions
// 0/16 and nearest-even SIMD quantization for the other synthesis samples.
fn pcm_sample(normalized: f32, scalar_pair: bool) -> i16 {
    let sample = normalized * 32768.0;
    if scalar_pair {
        if sample >= 32766.5 {
            return i16::MAX;
        }
        if sample <= -32767.5 {
            return i16::MIN;
        }
        let value = (sample + 0.5) as i16;
        value - i16::from(value < 0)
    } else {
        sample.round_ties_even().clamp(-32768.0, 32767.0) as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthesis_pair_quantization_and_simd_lanes_keep_their_distinct_boundaries() {
        assert_eq!(pcm_sample(-1.0 / 32768.0, true), 0);
        assert_eq!(pcm_sample(-1.0 / 32768.0, false), -1);
        assert_eq!(pcm_sample(-2.0 / 32768.0, true), -2);
        assert_eq!(pcm_sample(2.5 / 32768.0, false), 2);
        assert_eq!(pcm_sample(2.5 / 32768.0, true), 3);
        assert_eq!(pcm_sample(10.0, true), i16::MAX);
        assert_eq!(pcm_sample(-10.0, false), i16::MIN);
    }
    #[test]
    fn missing_frames_and_input_capacity_fail_without_pcm() {
        assert_eq!(decode(&[], 1, 1).unwrap_err(), Error::InvalidStream);
        assert_eq!(decode(b"ID3", 2, 100).unwrap_err(), Error::InputLimit);
        assert_eq!(decode(b"ID3", 3, 100).unwrap_err(), Error::InvalidStream);
    }
}
