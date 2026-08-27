//! Bounded RIFF PCM metadata. Sample data remains in its original resource.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PcmMetadata {
    pub sample_rate: u32,
    pub frames: u32,
    pub cue_frame: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InvalidWave;

pub fn pcm_metadata(bytes: &[u8]) -> Result<PcmMetadata, InvalidWave> {
    let u32_at = |at: usize| -> Result<u32, InvalidWave> {
        Ok(u32::from_le_bytes(
            bytes
                .get(at..at + 4)
                .ok_or(InvalidWave)?
                .try_into()
                .unwrap(),
        ))
    };
    if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(InvalidWave);
    }
    let end = 8usize.checked_add(u32_at(4)? as usize).ok_or(InvalidWave)?;
    if end != bytes.len() {
        return Err(InvalidWave);
    }
    let mut at = 12;
    let mut format = None;
    let mut data_bytes = None;
    let mut cue_frame = None;
    while at < end {
        let size = u32_at(at + 4)? as usize;
        let start = at + 8;
        let next = start.checked_add(size).ok_or(InvalidWave)?;
        let chunk = bytes.get(start..next).ok_or(InvalidWave)?;
        match bytes.get(at..at + 4).ok_or(InvalidWave)? {
            b"fmt " => {
                if format.is_some() || chunk.len() < 16 {
                    return Err(InvalidWave);
                }
                let short = |offset| u16::from_le_bytes([chunk[offset], chunk[offset + 1]]);
                let channels = short(2);
                let rate = u32_at(start + 4)?;
                let align = short(12);
                if short(0) != 1
                    || !(1..=2).contains(&channels)
                    || rate == 0
                    || ![8, 16].contains(&short(14))
                    || align != channels * (short(14) / 8)
                {
                    return Err(InvalidWave);
                }
                format = Some((rate, u32::from(align)));
            }
            b"data" => {
                if data_bytes.replace(size as u32).is_some() {
                    return Err(InvalidWave);
                }
            }
            b"cue " => {
                if size < 4 {
                    return Err(InvalidWave);
                }
                let count = u32_at(start)? as usize;
                if count > 1024 || size != 4 + count * 24 {
                    return Err(InvalidWave);
                }
                if count > 0 {
                    if cue_frame.is_some() || chunk.get(12..16) != Some(b"data") {
                        return Err(InvalidWave);
                    }
                    cue_frame = Some(u32_at(start + 24)?);
                }
            }
            _ => {}
        }
        at = next.checked_add(size & 1).ok_or(InvalidWave)?;
    }
    let (sample_rate, align) = format.ok_or(InvalidWave)?;
    let data_bytes = data_bytes.ok_or(InvalidWave)?;
    let frames = data_bytes / align;
    if at != end
        || frames == 0
        || data_bytes % align != 0
        || cue_frame.is_some_and(|cue| cue >= frames)
    {
        return Err(InvalidWave);
    }
    Ok(PcmMetadata {
        sample_rate,
        frames,
        cue_frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave(cue: Option<u32>) -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\x10\0\0\0".to_vec();
        bytes.extend_from_slice(&[0; 16]);
        if let Some(frame) = cue {
            bytes.extend_from_slice(
                b"cue \x1c\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0data\0\0\0\0\0\0\0\0",
            );
            bytes.extend_from_slice(&frame.to_le_bytes());
        }
        let size = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        bytes
    }
    #[test]
    fn pcm_cue_is_a_sample_frame_not_a_byte_offset() {
        assert_eq!(
            pcm_metadata(&wave(Some(2))),
            Ok(PcmMetadata {
                sample_rate: 44100,
                frames: 4,
                cue_frame: Some(2)
            })
        );
        assert_eq!(pcm_metadata(&wave(None)).unwrap().cue_frame, None);
        assert!(pcm_metadata(&wave(Some(4))).is_err());
        let valid = wave(Some(0));
        for length in 0..valid.len() {
            assert!(pcm_metadata(&valid[..length]).is_err());
        }
    }
}
