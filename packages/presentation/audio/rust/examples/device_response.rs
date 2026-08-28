//! Device-facing float PCM fixture, normalized for the browser output range.
use std::io::Write;
fn main() {
    let gain: f32 = std::env::args().nth(1).unwrap().parse().unwrap();
    assert!(gain.is_finite() && (0.0..=1.0).contains(&gain));
    let input = std::fs::read(std::env::var("DSP_INPUT_PCM").unwrap()).unwrap();
    assert_eq!(input.len(), 512 * 8);
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for sample in input.chunks_exact(4) {
        let value = i32::from_le_bytes(sample.try_into().unwrap());
        out.write_all(&playsrc_audio::output::device_sample(value, gain).to_le_bytes())
            .unwrap();
    }
}
