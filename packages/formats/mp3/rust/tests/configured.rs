use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires configured cow1.mp3; no game assets are distributed"]
fn configured_mono_matches_public_minimp3_sse_pcm() {
    let root = std::env::var_os("PLAYSRC_AUDIO_EVIDENCE").expect("explicit evidence directory");
    let input = std::fs::read(std::path::PathBuf::from(root).join("cow1.mp3")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&input)),
        "6d5029641d1a058b5316d4fd49b7ee923ec6490bb5ce93e40fa25ccaa169aad5"
    );
    let decoded = playsrc_mp3::decode(&input, 32 * 1024 * 1024, 32 * 1024 * 1024).unwrap();
    assert_eq!(
        (decoded.sample_rate, decoded.channels, decoded.samples.len()),
        (44100, 1, 73728)
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
}
