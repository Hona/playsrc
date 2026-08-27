use std::io::Write;
fn main() {
    let path = std::env::args().nth(1).expect("MP3 input");
    let bytes = std::fs::read(path).unwrap();
    let result = playsrc_mp3::decode(&bytes, 32 * 1024 * 1024, 32 * 1024 * 1024).unwrap();
    eprintln!(
        "rate={} channels={} samples={}",
        result.sample_rate,
        result.channels,
        result.samples.len()
    );
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for sample in result.samples {
        output.write_all(&sample.to_le_bytes()).unwrap();
    }
}
