use playsrc_tf2_wasm as wasm;
use sha2::{Digest, Sha256};

struct Handle(u32);
impl Drop for Handle { fn drop(&mut self) { wasm::playsrc_dispose(self.0); } }

fn require(value: bool, message: &str) -> Result<(), String> {
    value.then_some(()).ok_or_else(|| message.to_owned())
}

fn identity(handle: u32) -> Result<serde_json::Value, String> {
    require(wasm::playsrc_result_error(handle) == 0, "payload-retention map compilation failed")?;
    let mut hash = [0; 32];
    let mut derived = [0; 32];
    require(unsafe { wasm::playsrc_result_hash(handle, hash.as_mut_ptr()) } == 1, "payload hash unavailable")?;
    require(unsafe { wasm::playsrc_result_derived_hash(handle, derived.as_mut_ptr()) } == 1, "derived hash unavailable")?;
    let mut spawn = [0; 40];
    require(unsafe { wasm::playsrc_spawn_copy(handle, spawn.as_mut_ptr(), spawn.len()) } == spawn.len(), "spawn copy failed")?;
    let mut coverage = vec![0; wasm::playsrc_coverage_length(handle)];
    require(unsafe { wasm::playsrc_coverage_copy(handle, coverage.as_mut_ptr(), coverage.len()) } == coverage.len(), "coverage copy failed")?;
    Ok(serde_json::json!({"byteLength": wasm::playsrc_result_length(handle), "sha256": hash, "derivedSha256": derived, "spawn": spawn.as_slice(), "coverageSha256": format!("{:x}", Sha256::digest(coverage))}))
}

pub fn verify(bsp: &[u8], resources: &[u8]) -> Result<Vec<serde_json::Value>, String> {
    let section = wasm::ResourceSection { pointer: resources.as_ptr(), length: resources.len() };
    let digest: [u8; 32] = Sha256::digest(resources).into();
    let mut profiles = Vec::new();
    for profile in [0, 1] {
        let full = Handle(unsafe { wasm::playsrc_compile_map(bsp.as_ptr(), bsp.len(), profile, &section, 1, digest.as_ptr(), 1) });
        let expected = identity(full.0)?;
        let mut payload = vec![0; wasm::playsrc_result_length(full.0)];
        require(unsafe { wasm::playsrc_result_copy(full.0, payload.as_mut_ptr(), payload.len()) } == payload.len(), "retained map copy failed")?;
        let payload_sha256: [u8; 32] = Sha256::digest(&payload).into();
        require(expected["sha256"] == serde_json::json!(payload_sha256), "retained bytes differ from declared SHA")?;
        drop(payload);
        let mut presentation = vec![0; wasm::playsrc_presentation_length(full.0)];
        require(unsafe { wasm::playsrc_presentation_copy(full.0, presentation.as_mut_ptr(), presentation.len()) } == presentation.len(), "presentation copy failed")?;
        drop(full);
        let mut variants = Vec::new();
        for (cached, retain) in [(true, 1), (true, 0), (false, 0)] {
            let started = std::time::Instant::now();
            let handle = Handle(unsafe {
                if cached { wasm::playsrc_compile_map_cached(bsp.as_ptr(), bsp.len(), profile, &section, 1, digest.as_ptr(), presentation.as_ptr(), presentation.len(), retain) }
                else { wasm::playsrc_compile_map(bsp.as_ptr(), bsp.len(), profile, &section, 1, digest.as_ptr(), retain) }
            });
            require(identity(handle.0)? == expected, "identity-only serialization changed map bytes, derived identity, spawn or closure")?;
            let retained_bytes = wasm::playsrc_compile_memory_bytes(handle.0, 12);
            require(retained_bytes == if retain == 0 { 0 } else { wasm::playsrc_result_length(handle.0) }, "map payload storage differs from retention request")?;
            if retain == 0 {
                require(wasm::playsrc_result_take(handle.0).is_null(), "identity-only map unexpectedly owns output bytes")?;
                let mut sentinel = [0xa5; 4];
                require(unsafe { wasm::playsrc_result_copy(handle.0, sentinel.as_mut_ptr(), sentinel.len()) } == 0 && sentinel == [0xa5; 4], "identity-only copy modified destination")?;
            }
            require(wasm::playsrc_result_release(handle.0) == 1 && wasm::playsrc_result_length(handle.0) == 0, "map release did not retire length")?;
            variants.push(serde_json::json!({"cachedPresentation": cached, "retainPayload": retain, "retainedBytes": retained_bytes, "runtimeMapMilliseconds": wasm::playsrc_compile_metric_milliseconds(handle.0, 7), "milliseconds": started.elapsed().as_secs_f64() * 1000.0}));
        }
        profiles.push(serde_json::json!({"profile": profile, "identity": expected, "presentationSha256": format!("{:x}", Sha256::digest(presentation)), "variants": variants}));
    }
    Ok(profiles)
}
