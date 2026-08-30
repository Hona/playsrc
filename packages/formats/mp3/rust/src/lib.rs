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
        let start = output.samples.len();
        output.samples.resize(start + count, 0);
        quantize(
            &pcm[..count],
            &mut output.samples[start..],
            usize::from(output.channels),
        );
    }
    if output.samples.is_empty() {
        return Err(Error::InvalidStream);
    }
    Ok(output)
}

fn quantize(input: &[f32], output: &mut [i16], channels: usize) {
    assert_eq!(input.len(), output.len());
    assert!(channels == 1 || channels == 2);
    // The first interleaved frame of each synthesis group uses scalar-pair
    // rounding. The remaining samples are independent nearest-even lanes.
    for (input, output) in input
        .chunks(16 * channels)
        .zip(output.chunks_mut(16 * channels))
    {
        let pair = input.len().min(channels);
        for (sample, value) in input[..pair].iter().zip(&mut output[..pair]) {
            *value = pcm_sample(*sample, true);
        }
        quantize_nearest(&input[pair..], &mut output[pair..]);
    }
}

fn quantize_nearest(input: &[f32], output: &mut [i16]) {
    assert_eq!(input.len(), output.len());
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    let at = {
        use core::arch::wasm32::*;
        let mut at = 0;
        while input.len() - at >= 4 {
            // Four readable f32 inputs and four writable i16 outputs. Neither
            // load nor store requires vector alignment or touches a tail lane.
            unsafe {
                let sample = f32x4_mul(
                    v128_load(input.as_ptr().add(at).cast()),
                    f32x4_splat(32768.0),
                );
                let rounded = i32x4_trunc_sat_f32x4(f32x4_nearest(sample));
                let packed = i16x8_narrow_i32x4(rounded, rounded);
                v128_store64_lane::<0>(packed, output.as_mut_ptr().add(at).cast());
            }
            at += 4;
        }
        at
    };
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    let at = 0;
    for (sample, value) in input[at..].iter().zip(&mut output[at..]) {
        *value = pcm_sample(*sample, false);
    }
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

    #[test]
    fn synthesis_groups_preserve_every_lane_and_short_tail() {
        check_synthesis_groups();
    }

    #[cfg(target_arch = "wasm32")]
    #[unsafe(no_mangle)]
    pub extern "C" fn check_wasm_synthesis_groups() {
        check_synthesis_groups();
    }

    #[cfg(target_arch = "wasm32")]
    mod wasm_decode {
        use std::sync::Mutex;
        static PCM: Mutex<Vec<i16>> = Mutex::new(Vec::new());

        #[unsafe(no_mangle)]
        pub extern "C" fn test_input_alloc(length: usize) -> *mut u8 {
            assert!(length <= 32 * 1024 * 1024);
            Box::into_raw(vec![0_u8; length].into_boxed_slice()) as *mut u8
        }

        #[unsafe(no_mangle)]
        /// # Safety
        /// Pass one allocation returned by test_input_alloc with its exact length.
        pub unsafe extern "C" fn test_decode(pointer: *mut u8, length: usize) -> usize {
            let input =
                unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, length)) };
            let decoded = super::decode(&input, 32 * 1024 * 1024, 32 * 1024 * 1024).unwrap();
            let mut pcm = PCM.lock().unwrap();
            *pcm = decoded.samples;
            pcm.len()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn test_pcm_pointer() -> *const i16 {
            PCM.lock().unwrap().as_ptr()
        }
    }

    fn check_synthesis_groups() {
        let values = [
            0.0,
            -0.0,
            f32::from_bits(1),
            -f32::from_bits(1),
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::from_bits(0xffc12345),
            -32767.5 / 32768.0,
            32766.5 / 32768.0,
            -1.0 / 32768.0,
            -2.5 / 32768.0,
            2.5 / 32768.0,
            1.0,
            -1.0,
        ];
        for channels in [1, 2] {
            for offset in 0..values.len() {
                let input: Vec<_> = (0..97)
                    .map(|index| values[(index + offset) % values.len()])
                    .collect();
                for length in 0..input.len() {
                    let mut output = vec![12345; length + 2];
                    quantize(&input[..length], &mut output[1..length + 1], channels);
                    assert_eq!(output[0], 12345);
                    assert_eq!(output[length + 1], 12345);
                    for index in 0..length {
                        assert_eq!(
                            output[index + 1],
                            pcm_sample(input[index], (index / channels) % 16 == 0)
                        );
                    }
                }
            }
        }
    }
}
