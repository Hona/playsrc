use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    ops::Range,
    path::PathBuf,
};

struct Segments(PathBuf);
impl playsrc_vpk::SegmentReader for Segments {
    fn len(&self, index: u32) -> Result<u64, playsrc_vpk::SourceError> {
        Ok(
            fs::metadata(self.0.join(format!("tf2_sound_misc_{index:03}.vpk")))
                .unwrap()
                .len(),
        )
    }
    fn read(&self, index: u32, range: Range<u64>) -> Result<Vec<u8>, playsrc_vpk::SourceError> {
        let mut file =
            fs::File::open(self.0.join(format!("tf2_sound_misc_{index:03}.vpk"))).unwrap();
        file.seek(SeekFrom::Start(range.start)).unwrap();
        let mut bytes = vec![0; usize::try_from(range.end - range.start).unwrap()];
        file.read_exact(&mut bytes).unwrap();
        Ok(bytes)
    }
}

#[test]
#[ignore = "requires the exact configured TF2 sound archive; no game assets are distributed"]
fn configured_mono_matches_public_minimp3_sse_pcm() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("playsrc.local.json")).unwrap()).unwrap();
    let contract: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("games/tf2/content-build.json")).unwrap())
            .unwrap();
    let tf2 = PathBuf::from(config["tf2Dir"].as_str().unwrap());
    let directory = fs::read(tf2.join("tf2_sound_misc_dir.vpk")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&directory)),
        contract["archiveIndexes"]["tf2SoundMisc"].as_str().unwrap()
    );
    let archive = playsrc_vpk::parse(
        &directory,
        "tf2_sound_misc_dir.vpk",
        playsrc_vpk::Layout::Split,
        Default::default(),
    )
    .unwrap();
    let input = archive
        .read_entry("sound/ambient_mp3/cow1.mp3", &Segments(tf2))
        .unwrap()
        .bytes;
    assert_eq!(
        format!("{:x}", Sha256::digest(&input)),
        "6d5029641d1a058b5316d4fd49b7ee923ec6490bb5ce93e40fa25ccaa169aad5"
    );
    let decoded = playsrc_mp3::decode(&input, 32 * 1024 * 1024, 32 * 1024 * 1024).unwrap();
    assert_eq!(
        (decoded.sample_rate, decoded.channels, decoded.samples.len()),
        (44100, 1, 73728)
    );
    assert_eq!(
        decoded.samples.capacity(),
        decoded.samples.len().next_power_of_two()
    );
    let mut digest = Sha256::new();
    for sample in &decoded.samples {
        digest.update(sample.to_le_bytes());
    }
    // minimp3 ea99364f61c14656440e8d77e9c233ccf3124633 SSE2 output.
    // Agreement here is decoder arithmetic evidence, not complete engine mixing evidence.
    assert_eq!(
        format!("{:x}", digest.finalize()),
        "b1e43ccf681c3529aad850231599216cfd55778a27bb559b8859917be486ee42"
    );
    assert_eq!(
        playsrc_mp3::decode(&input, input.len(), decoded.samples.len() - 1).unwrap_err(),
        playsrc_mp3::Error::OutputLimit
    );
    let retained = PathBuf::from(config["sourceCacheDir"].as_str().unwrap())
        .join("evidence/tf2-wasm-simd-performance/configured");
    fs::create_dir_all(&retained).unwrap();
    fs::write(retained.join("cow1.mp3"), &input).unwrap();
}
