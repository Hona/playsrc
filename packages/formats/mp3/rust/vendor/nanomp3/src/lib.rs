#![no_std]

mod minimp3;

/// The minimum length of the PCM output buffer.
pub const MAX_SAMPLES_PER_FRAME: usize = 1152*2;

/// The core MP3 decoder, with no internal buffering.
pub struct Decoder(minimp3::mp3dec_t);


/// The channel formats that may be encoded in an MP3 frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Channels {
    Mono = 1,
    Stereo
}

impl Channels {
    /// Returns the corresponding number of channels for `self`.
    pub fn num(self) -> u8 {
        self as u8
    }
}

/// Information about the frame decoded by [`Decoder::decode`]
#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    /// The number of PCM samples produced.
    pub samples_produced: usize,
    /// The number of channels in this frame.
    pub channels: Channels,
    /// Sample rate of this frame, in Hz.
    pub sample_rate: u32,
    /// The current MP3 bit rate, in kilobits per second.
    pub bitrate: u32
}

impl Decoder {
    /// Instantiates a `Decoder`.
    pub const fn new() -> Self {
        Self(minimp3::mp3dec_t::new())
    }

    /// Decode MP3 data into a buffer, returning the amount of MP3 data consumed and info about decoded samples.
    /// `mp3` should contain at least several frames worth of data at any given time (16KiB recommended) to avoid artifacting.
    ///
    /// # Panics
    ///
    /// Panics if `pcm` is less than [`MAX_SAMPLES_PER_FRAME`] long.
    pub fn decode(&mut self, mp3: &[u8], pcm: &mut [f32]) -> (usize, Option<FrameInfo>) {
        assert!(pcm.len() >= MAX_SAMPLES_PER_FRAME, "pcm buffer too small");

        let mut info = minimp3::mp3dec_frame_info_t::default();

        let samples = unsafe { minimp3::mp3dec_decode_frame(
            &mut self.0,
            mp3.as_ptr(),
            mp3.len().try_into().unwrap(),
            pcm.as_mut_ptr(),
            &mut info
        ) };

        (
            info.frame_bytes.try_into().unwrap(),
            (samples != 0).then(|| FrameInfo {
                samples_produced: samples.try_into().unwrap(),
                channels: match info.channels {
                    1 => Channels::Mono,
                    2 => Channels::Stereo,
                    _ => unreachable!()
                },
                sample_rate: info.hz.try_into().unwrap(),
                bitrate: info.bitrate_kbps.try_into().unwrap()
            })
        )
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
