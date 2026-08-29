//! Native accelerated simulation diagnostics. No browser, frame delivery, or GC claims.
use playsrc_tf2_wasm as wasm;
use wasm::gameplay_replay as replay_api;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Instant};
use crate::native_allocations;

struct Handle(u32);
impl Drop for Handle { fn drop(&mut self) { wasm::playsrc_dispose(self.0); } }
fn require(value: bool, message: &str) -> Result<(), String> { value.then_some(()).ok_or_else(|| message.to_owned()) }
fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn command(initial: bool) -> [u8; 84] {
    let mut bytes = [0; 84];
    bytes[..4].copy_from_slice(b"PCMD");
    bytes[4..8].copy_from_slice(&9_u32.to_le_bytes());
    bytes[48..52].copy_from_slice(&84_u32.to_le_bytes());
    if initial {
        // Soldier/RED, normal-intelligence normal-quota23 in a24-player server.
        bytes[32..36].copy_from_slice(&(3_u32 | (2 << 16)).to_le_bytes());
        bytes[44..48].copy_from_slice(&(0x8000_0000_u32 | 23 | (24 << 6) | (1 << 12)).to_le_bytes());
    }
    bytes
}

fn records(bytes: &[u8]) -> Result<Vec<(u32, &[u8])>, String> {
    require((780..=4 * 1024 * 1024).contains(&bytes.len()) && &bytes[..8] == b"PGRP\x04\0\0\0" && &bytes[88..96] == b"TFEQ\x01\0\0\0", "expected PGRP4 checkpoint")?;
    require(bytes[72..80] == 0_u64.to_le_bytes() && bytes[80..88] == 1_u64.to_le_bytes(), "invalid initial tick/revision")?;
    let mut offset = 780;
    let mut result = Vec::new();
    let (mut observing, mut tick, mut marks, mut last_clock) = (false, 0_u64, 0_u32, 0.0_f64);
    while offset < bytes.len() {
        let header = bytes.get(offset..offset + 8).ok_or("truncated record")?;
        let length = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        require((8..=65596).contains(&length) && result.len() <= 16384, "invalid record length/count")?;
        let kind = u32::from_le_bytes(header[4..].try_into().unwrap());
        let data = bytes.get(offset + 8..offset + length).ok_or("truncated payload")?;
        match kind {
            1 => {
                require(!observing && data.len() >= 108, "invalid observe")?;
                require(u32::from_le_bytes(data[20..24].try_into().unwrap()) as usize + 24 == data.len()
                    && f64::from_le_bytes(data[..8].try_into().unwrap()).is_finite()
                    && u32::from_le_bytes(data[8..12].try_into().unwrap()) <= 1, "invalid observe payload")?;
                observing = true;
            }
            2 => {
                require(observing && data.len() >= 140, "invalid tick")?;
                let next = u64::from_le_bytes(data[..8].try_into().unwrap());
                let command = u32::from_le_bytes(data[48..52].try_into().unwrap()) as usize;
                let clocks = u32::from_le_bytes(data[52..56].try_into().unwrap()) as usize;
                require(next == tick + 1 && command >= 84 && clocks <= 4096 && data.len() == 56 + command + clocks * 8, "invalid tick order/size")?;
                for clock in data[56 + command..].chunks_exact(8) {
                    let value = f64::from_le_bytes(clock.try_into().unwrap());
                    require(value.is_finite() && value >= last_clock, "invalid work clock")?;
                    last_clock = value;
                }
                tick = next;
            }
            3 => { require(observing && data.len() == 32, "invalid publication")?; observing = false; }
            7 => {
                require(!observing && data == marks.to_le_bytes() && marks < 2, "invalid mark")?;
                marks += 1;
            }
            8 => { require(!observing && marks == 2 && data == [1, 0, 0, 0] && offset + length == bytes.len(), "invalid footer")?; }
            _ => return Err("native scripted workload contains an unsupported mutation".into()),
        }
        result.push((kind, data));
        offset += length;
    }
    require(result.last() == Some(&(8, &[1, 0, 0, 0][..])), "incomplete journal")?;
    Ok(result)
}

fn compare(expected: &[u8], actual: &[u8]) -> Result<(), String> {
    let expected_records = records(expected)?;
    let actual_records = records(actual)?;
    require(actual[..780] == expected[..780], "initial state/equipment differs")?;
    require(actual_records.len() == expected_records.len(), "record count differs")?;
    for (index, ((kind, actual), (expected_kind, expected))) in actual_records.iter().zip(expected_records).enumerate() {
        let same = *kind == expected_kind && if *kind == 2 { actual[..8] == expected[..8] && actual[16..] == expected[16..] } else { *actual == expected };
        require(same, &format!("tick/command/state/order transcript differs at record {index}"))?;
    }
    Ok(())
}

pub fn run(bsp: &[u8], resources: &[u8], directory: &Path, label: &str, replay: Option<&str>) -> Result<serde_json::Value, String> {
    require(!label.is_empty() && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'), "invalid evidence label")?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    require(!directory.join(format!("{label}.json")).exists(), "evidence label already exists; never overwrite a comparison")?;
    let expected = replay.map(|identity| {
        require(identity.len() == 64 && identity.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()), "invalid replay identity")?;
        let bytes = fs::read(directory.join(format!("{identity}.replay.bin"))).map_err(|e| e.to_string())?;
        require(hash(&bytes) == identity, "replay identity differs")?;
        Ok::<_, String>(bytes)
    }).transpose()?;
    let section = wasm::ResourceSection { pointer: resources.as_ptr(), length: resources.len() };
    let digest: [u8; 32] = Sha256::digest(resources).into();
    let handle = Handle(unsafe { wasm::playsrc_compile_map(bsp.as_ptr(), bsp.len(), 1, &section, 1, digest.as_ptr(), 1) });
    require(wasm::playsrc_result_error(handle.0) == 0, "native map compilation failed")?;
    wasm::playsrc_result_release(handle.0);
    wasm::playsrc_presentation_release(handle.0);
    let mut inputs = Vec::new();
    if let Some(expected) = &expected {
        for (kind, data) in records(expected)? {
            if kind == 2 {
                require(data.len() >= 56, "truncated tick")?;
                let command_length = u32::from_le_bytes(data[48..52].try_into().unwrap()) as usize;
                let count = u32::from_le_bytes(data[52..56].try_into().unwrap()) as usize;
                require(data.len() == 56 + command_length + count * 8, "invalid clock payload")?;
                inputs.extend_from_slice(&data[56 + command_length..]);
            }
        }
        require(unsafe { replay_api::playsrc_gameplay_replay_clock_input(handle.0, inputs.as_ptr(), inputs.len()) } == 1, "clock input rejected")?;
    }
    require(replay_api::playsrc_gameplay_replay_begin(handle.0) == 1, "capture rejected")?;
    // The phase recorder is separately bounded; complete tick/command/state and
    // work-clock journal capture continues without per-phase admission events.
    replay_api::playsrc_gameplay_replay_stop_admission(handle.0);
    replay_api::playsrc_gameplay_replay_mark(handle.0, 0);
    let started = Instant::now();
    let mut samples = Vec::new();
    let mut active_tick = None;
    let mut last_tick = 0;
    let mut calls = Vec::new();
    let expected_records = expected.as_ref().map(|bytes| records(bytes)).transpose()?;
    let mut observations = expected_records.as_ref().map(|records| records.iter().filter(|(kind, _)| *kind == 1));
    let mut ordinal = 0_u32;
    loop {
        require(started.elapsed().as_secs() < 150, "native simulation deadline exceeded")?;
        let scripted = command(ordinal == 1);
        let (now, command, suspended, acknowledged) = if let Some(observations) = &mut observations {
            let Some((_, data)) = observations.next() else { break; };
            require(data.len() >= 24, "truncated observation")?;
            (f64::from_le_bytes(data[..8].try_into().unwrap()), &data[24..], u32::from_le_bytes(data[8..12].try_into().unwrap()), u64::from_le_bytes(data[12..20].try_into().unwrap()))
        } else { (f64::from(ordinal) * 4.0 * 0.015, scripted.as_slice(), 0, 0) };
        let began = Instant::now();
        let (success, (requests, bytes)) = native_allocations::measure(|| unsafe { wasm::playsrc_simulation_observe(handle.0, now, command.as_ptr(), command.len(), suspended, acknowledged) });
        let milliseconds = began.elapsed().as_secs_f64() * 1000.0;
        require(success == 1, &format!("observe failed: {}", wasm::playsrc_simulation_error()))?;
        let state = wasm::inspect_native_gameplay(handle.0, |s| (s.tick, s.bots.len(), s.round.state == playsrc_tf2::round::State::Running && !s.round.waiting_for_players && !s.round.in_setup));
        let (tick, bots, active) = if ordinal == 0 { state.unwrap_or((0, 0, false)) } else { state.ok_or("missing snapshot")? };
        require(ordinal == 0 || tick > last_tick, "native simulation did not advance")?;
        if active && bots == 23 && active_tick.is_none() {
            active_tick = Some(tick);
            println!("NATIVE_ACTIVE {}", std::process::id());
        }
        if active_tick.is_some() { require(active && bots == 23, "active full-roster interval interrupted")?; }
        calls.push(serde_json::json!({"tick":tick,"now":now,"wallMilliseconds":started.elapsed().as_secs_f64()*1000.0,"milliseconds":milliseconds,"allocationRequests":requests,"requestedBytes":bytes,"movementQueryStorageBytes":wasm::native_movement_query_storage_bytes(handle.0)}));
        if ordinal > 0 && ordinal % 16 == 0 {
            samples.push(wasm::inspect_native_gameplay(handle.0, |s| serde_json::json!({"tick":s.tick,"active":active,"bots":s.bots.iter().map(|b|serde_json::json!({"identity":b.identity,"class":b.class as u8,"team":b.team as u8,"health":b.health,"shots":b.shots,"position":b.position,"objective":b.objective as u8})).collect::<Vec<_>>(),"owners":s.control_points.as_ref().map(|p|p.points.iter().map(|p|p.owner as u8).collect::<Vec<_>>())})).unwrap());
        }
        last_tick = tick;
        ordinal += 1;
        if expected.is_none() && active_tick.is_some_and(|start| tick >= start + 6667) { break; }
        require(tick < 10000, "full-roster sustained interval not reached")?;
    }
    replay_api::playsrc_gameplay_replay_mark(handle.0, 1);
    require(replay_api::playsrc_gameplay_replay_stop(handle.0) == 1, "journal overflow/incomplete capture")?;
    let mut journal = vec![0; replay_api::playsrc_gameplay_replay_length(handle.0)];
    require(unsafe { replay_api::playsrc_gameplay_replay_copy(handle.0, 0, journal.as_mut_ptr(), journal.len()) } == journal.len(), "journal copy failed")?;
    let actual_records = records(&journal)?;
    if let Some(expected) = &expected {
        require(replay_api::playsrc_gameplay_replay_clock_remaining(handle.0) == 0, "work clock consumption differs")?;
        compare(expected, &journal)?;
    }
    let identity = hash(&journal);
    fs::write(directory.join(format!("{identity}.replay.bin")), &journal).map_err(|e| e.to_string())?;
    let report = serde_json::json!({"scope":"accelerated native observation/tick simulation; not browser aging, GC, WASM memory or presented FPS","bspSha256":hash(bsp),"resourcesSha256":hash(resources),"replaySha256":identity,"replayBytes":journal.len(),"verifiedAgainst":replay,"records":actual_records.len(),"ticks":last_tick,"activeStartedTick":active_tick,"activeSimulationSeconds":(last_tick-active_tick.ok_or("no active interval")?) as f64*0.015,"wallMilliseconds":started.elapsed().as_secs_f64()*1000.0,"calls":calls,"samples":samples});
    fs::write(directory.join(format!("{label}.json")), serde_json::to_vec(&report).unwrap()).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"report":directory.join(format!("{label}.json")),"replaySha256":identity,"ticks":last_tick,"wallMilliseconds":started.elapsed().as_secs_f64()*1000.0}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 780];
        bytes[..8].copy_from_slice(b"PGRP\x04\0\0\0");
        bytes[80..88].copy_from_slice(&1_u64.to_le_bytes());
        bytes[88..96].copy_from_slice(b"TFEQ\x01\0\0\0");
        let mut append = |kind: u32, data: &[u8]| { bytes.extend_from_slice(&((data.len()+8) as u32).to_le_bytes()); bytes.extend_from_slice(&kind.to_le_bytes()); bytes.extend_from_slice(data); };
        append(7, &0_u32.to_le_bytes());
        let mut observe = vec![0; 24];
        observe[20..24].copy_from_slice(&84_u32.to_le_bytes()); observe.extend_from_slice(&command(true));
        append(1, &observe);
        let mut tick = vec![0; 56]; tick[..8].copy_from_slice(&1_u64.to_le_bytes());
        tick[48..52].copy_from_slice(&84_u32.to_le_bytes()); tick[52..56].copy_from_slice(&1_u32.to_le_bytes());
        tick.extend_from_slice(&command(true)); tick.extend_from_slice(&1.0_f64.to_le_bytes()); append(2, &tick);
        append(3, &[0; 32]); append(7, &1_u32.to_le_bytes()); append(8, &1_u32.to_le_bytes());
        bytes
    }
    #[test]
    fn transcript_ignores_only_measurement_duration_not_order_commands_state_or_work_clocks() {
        let expected = fixture();
        let tick = 780 + 12 + 116 + 8;
        let mut actual = expected.clone(); actual[tick+8] = 99;
        assert!(compare(&expected, &actual).is_ok());
        for offset in [72, 100, tick, tick+16, tick+56, tick+140] {
            let mut actual = expected.clone(); actual[offset] ^= 1;
            assert!(compare(&expected, &actual).is_err(), "offset {offset}");
        }
    }
    #[test]
    fn partial_malformed_and_incomplete_journals_fail_closed() {
        let expected = fixture();
        for length in 0..expected.len() { assert!(records(&expected[..length]).is_err()); }
        let mut oversized = expected.clone(); oversized[780..784].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(records(&oversized).is_err());
        let mut incomplete = expected; let end = incomplete.len(); incomplete[end-4] = 0;
        assert!(records(&incomplete).is_err());
    }
}
