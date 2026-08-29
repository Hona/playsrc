use playsrc_asset_graph::{ResourceGraph, decode, encode_resource_set, hex_hash};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};
mod payload_retention;
mod sustained_bots;
mod native_allocations;
#[global_allocator]
static ALLOCATOR: native_allocations::Allocator = native_allocations::Allocator;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    tf2_dir: PathBuf,
    source_cache_dir: PathBuf,
    asset_dir: PathBuf,
}

fn digest_identity(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn main() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if !(arguments.len() == 2 || arguments.len() == 3 && ["--control-point-match", "--payload-retention", "--texture-owner-scene", "--texture-owner-models", "--particle-owner-output"].contains(&arguments[2].as_str()) || [4, 5].contains(&arguments.len()) && arguments[2] == "--sustained-bots" || arguments.len() == 5 && arguments[2] == "--control-point-crossing" || arguments.len() == 6 && arguments[2] == "--view") || !digest_identity(&arguments[1])
    {
        return Err(
            "usage: playsrc-verify-map-runtime <target> <retained-graph-sha256> [--view x y z | --control-point-match | --control-point-crossing from to | --payload-retention | --particle-owner-output | --sustained-bots label [replay-sha256]]".to_owned(),
        );
    }
    let target = &arguments[0];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config: Config = serde_json::from_slice(
        &fs::read(root.join("playsrc.local.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let _ = (&config.tf2_dir, &config.asset_dir);
    let maps: BTreeMap<String, serde_json::Value> = serde_json::from_slice(
        &fs::read(root.join("games/tf2/maps.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let map = maps.get(target).ok_or("undeclared target")?;
    let (hash, length) = if let Some(installed) = map.get("installed") {
        (
            installed["sha256"].as_str(),
            installed["byteLength"].as_u64(),
        )
    } else {
        (
            map["download"]["decodedSha256"].as_str(),
            map["download"]["decodedByteLength"].as_u64(),
        )
    };
    let hash = hash.ok_or("missing BSP identity")?;
    if !digest_identity(hash) { return Err("malformed BSP identity".to_owned()); }
    let bsp = fs::read(
        config
            .source_cache_dir
            .join("objects/sha256")
            .join(&hash[..2])
            .join(hash),
    )
    .map_err(|error| error.to_string())?;
    if hex_hash(&bsp) != hash || Some(bsp.len() as u64) != length {
        return Err("BSP identity differs".to_owned());
    }
    let graph_bytes = fs::read(
        config
            .source_cache_dir
            .join("browser-bundles/immutable-roots")
            .join(&arguments[1]),
    )
    .map_err(|error| error.to_string())?;
    if hex_hash(&graph_bytes) != arguments[1] {
        return Err("retained graph identity differs".to_owned());
    }
    let graph: ResourceGraph =
        serde_json::from_slice(&graph_bytes).map_err(|error| error.to_string())?;
    if graph.schema != "playsrc-resource-graph-v1" || graph.target != *target || graph.content_build != "24245096" || graph.game != "tf2" {
        return Err("retained graph target differs".to_owned());
    }
    let mut entries = Vec::new();
    for chunk in graph
        .chunks
        .iter()
        .filter(|chunk| chunk.roles.iter().any(|role| role == "gameplay"))
    {
        if !digest_identity(&chunk.encoded_sha256) { return Err("malformed chunk identity".to_owned()); }
        let bytes = fs::read(
            config
                .source_cache_dir
                .join(format!("browser-bundles/{target}.graph/objects"))
                .join(&chunk.encoded_sha256),
        )
        .map_err(|error| error.to_string())?;
        entries.extend(
            decode(chunk, &bytes).map_err(|error| format!("resource integrity: {error:?}"))?,
        );
    }
    entries.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
    let resources =
        encode_resource_set(&entries).map_err(|error| format!("resource set: {error:?}"))?;
    drop(entries);
    if arguments.get(2).is_some_and(|option| option == "--sustained-bots") {
        if !["koth_harvest_final", "koth_viaduct"].contains(&target.as_str()) { return Err("sustained bot workload requires Harvest or Viaduct".into()); }
        let result = sustained_bots::run(&bsp, &resources, &config.source_cache_dir.join("evidence/sustained-bots"), &arguments[3], arguments.get(4).map(String::as_str))?;
        println!("{result}");
        return Ok(());
    }
    if arguments.get(2).is_some_and(|option| option == "--payload-retention") {
        let profiles = payload_retention::verify(&bsp, &resources)?;
        println!("{}", serde_json::json!({"target": target, "graphSha256": arguments[1], "bspSha256": hash, "profiles": profiles}));
        return Ok(());
    }
    let owner_models = arguments.get(2).is_some_and(|option| option == "--texture-owner-models");
    let owner_scene = owner_models || arguments.get(2).is_some_and(|option| option == "--texture-owner-scene");
    let particle_owner = arguments.get(2).is_some_and(|option| option == "--particle-owner-output");
    if particle_owner && !["koth_sawmill", "koth_lakeside_final"].contains(&target.as_str()) { return Err("particle owner diagnostic requires an admitted KOTH target".into()); }
    let particle_directory = config.source_cache_dir.join("evidence/koth-sustained-offline").join(target);
    let mut particle_records = Vec::new();
    if particle_owner { fs::create_dir_all(&particle_directory).map_err(|e|e.to_string())?; }
    if arguments.len() == 3 && !owner_scene && !particle_owner || arguments.get(2).is_some_and(|value|value=="--control-point-crossing") {
        let crossing=if arguments.len()==5{Some((arguments[3].parse::<u32>().map_err(|_|"invalid from area")?,arguments[4].parse::<u32>().map_err(|_|"invalid to area")?))}else{None};
        let mut frames = Vec::new();
        let result = playsrc_tf2_wasm::verify_control_point_match(&bsp, &resources, crossing, |snapshot| {
            frames.push(serde_json::json!({"tick":snapshot.tick,"state":snapshot.round.state as u8,"winner":snapshot.round.winning_team.map(|team|team as u8),"points":snapshot.control_points.as_ref().map(|points|points.points.iter().map(|p|p.owner as u8).collect::<Vec<_>>()),"bots":snapshot.bots.iter().map(|bot|serde_json::json!({"identity":bot.identity,"position":bot.position,"velocity":bot.velocity,"yaw":bot.yaw_degrees,"area":bot.area,"path":bot.remaining_path_areas,"captures":bot.captures})).collect::<Vec<_>>() }));
        });
        let output = config.source_cache_dir.join("evidence/map-runtime");
        fs::create_dir_all(&output).map_err(|error|error.to_string())?;
        let suffix=crossing.map_or_else(||"match".to_owned(),|(from,to)|format!("crossing-{from}-{to}"));
        fs::write(output.join(format!("{target}-control-point-{suffix}.json")),serde_json::to_vec(&frames).unwrap()).map_err(|error|error.to_string())?;
        return result;
    }
    let section = playsrc_tf2_wasm::ResourceSection { pointer: resources.as_ptr(), length: resources.len() };
    let resource_hash: [u8; 32] = Sha256::digest(&resources).into();
    let handle = unsafe { playsrc_tf2_wasm::playsrc_compile_map(bsp.as_ptr(), bsp.len(), u32::from(!owner_scene && !particle_owner), &section, 1, resource_hash.as_ptr(), 1) };
    struct OwnedHandle(u32);
    impl Drop for OwnedHandle { fn drop(&mut self) { playsrc_tf2_wasm::playsrc_dispose(self.0); } }
    let _owner = OwnedHandle(handle);
    let result = (|| {
        let error = playsrc_tf2_wasm::playsrc_result_error(handle);
        if error != 0 { return Err(format!("native HDR compilation failed: {error}")); }
        if arguments.len() == 6 {
            let position = arguments[3..].iter().map(|v| v.parse::<f32>()).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
            let view = [position[0], position[1], position[2], position[0], position[1], position[2], 0.0, 0.0, 75.0, 16.0 / 9.0, 1.0, 30_000.0, 0.0, -1.0];
            if unsafe { playsrc_tf2_wasm::playsrc_visibility_query(handle, view.as_ptr()) } != 1 {
                let length = playsrc_tf2_wasm::playsrc_visibility_output_length(handle);
                let pointer = playsrc_tf2_wasm::playsrc_visibility_output_pointer(handle);
                let reason = if !pointer.is_null() && length <= 4096 { String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(pointer, length) }).into_owned() } else { String::new() };
                return Err(format!("native visibility failed at {position:?}: {reason}"));
            }
        }
        let mut payload = vec![0; playsrc_tf2_wasm::playsrc_result_length(handle)];
        if unsafe { playsrc_tf2_wasm::playsrc_result_copy(handle, payload.as_mut_ptr(), payload.len()) } != payload.len() { return Err("native artifact copy failed".to_owned()); }
        Ok(payload)
    })();
    let payload = result?;
    if owner_scene {
        let output = config.source_cache_dir.join("evidence/tf2-browser-performance/texture-replacement/offline-scene");
        fs::create_dir_all(&output).map_err(|error| error.to_string())?;
        let command = fs::read(output.join("initial-command.bin")).map_err(|error|error.to_string())?;
        // Ordinary baseline then one exact Source tick, not a performance sample.
        for now in [0.0, 0.015] {
            if unsafe { playsrc_tf2_wasm::playsrc_simulation_observe(handle, now, command.as_ptr(), command.len(), 0, 0) } != 1 { return Err("offline initial publication failed".into()); }
        }
        if owner_models {
            let requests = fs::read(output.join("models-request.bin")).map_err(|error|error.to_string())?;
            if unsafe { playsrc_tf2_wasm::playsrc_model_transact(handle, requests.as_ptr(), requests.len()) } != 1 { return Err("offline model request failed".into()); }
            let mut poses = vec![0; playsrc_tf2_wasm::playsrc_model_output_length(handle)];
            if unsafe { playsrc_tf2_wasm::playsrc_model_output_copy(handle, poses.as_mut_ptr(), poses.len()) } != poses.len() { return Err("offline model output failed".into()); }
            fs::write(output.join("models-output.bin"), &poses).map_err(|error|error.to_string())?;
            let request = fs::read(output.join("visibility-request.bin")).map_err(|error|error.to_string())?;
            if request.len() != 56 { return Err("offline visibility input size".into()); }
            let values = request.chunks_exact(4).map(|bytes|f32::from_le_bytes(bytes.try_into().unwrap())).collect::<Vec<_>>();
            if unsafe { playsrc_tf2_wasm::playsrc_visibility_query(handle, values.as_ptr()) } != 1 { return Err("offline visibility query failed".into()); }
            let visibility = unsafe { std::slice::from_raw_parts(playsrc_tf2_wasm::playsrc_visibility_output_pointer(handle), playsrc_tf2_wasm::playsrc_visibility_output_length(handle)) };
            fs::write(output.join("visibility-output.bin"), visibility).map_err(|error|error.to_string())?;
            println!("{}", serde_json::json!({"posesBytes":poses.len(),"posesSha256":hex_hash(&poses),"visibilityBytes":visibility.len(),"visibilitySha256":hex_hash(visibility)}));
            return Ok(());
        }
        let mut snapshot = vec![0; playsrc_tf2_wasm::playsrc_snapshot_length(handle)];
        if snapshot.is_empty() || unsafe { playsrc_tf2_wasm::playsrc_snapshot_copy(handle, snapshot.as_mut_ptr(), snapshot.len()) } != snapshot.len() { return Err("offline snapshot unavailable".into()); }
        let mut equipment = vec![0; 65536];
        let equipment_length = unsafe { playsrc_tf2_wasm::playsrc_equipment_state_copy(handle, equipment.as_mut_ptr(), equipment.len()) };
        if equipment_length == 0 { return Err("offline equipment unavailable".into()); }
        equipment.truncate(equipment_length);
        let mut spawn = vec![0; 40];
        if unsafe { playsrc_tf2_wasm::playsrc_spawn_copy(handle, spawn.as_mut_ptr(), spawn.len()) } != spawn.len() { return Err("offline spawn unavailable".into()); }
        let mut presentation = vec![0; playsrc_tf2_wasm::playsrc_presentation_length(handle)];
        if unsafe { playsrc_tf2_wasm::playsrc_presentation_copy(handle, presentation.as_mut_ptr(), presentation.len()) } != presentation.len() { return Err("presentation copy failed".into()); }
        let mut records = Vec::new();
        for (name, data) in [("map.psmp", &payload), ("presentation.pspr", &presentation), ("resources.psdb", &resources), ("initial-snapshot.bin", &snapshot), ("equipment.bin", &equipment), ("spawn.bin", &spawn)] {
            fs::write(output.join(name), data).map_err(|error| error.to_string())?;
            records.push(serde_json::json!({"name":name,"bytes":data.len(),"sha256":hex_hash(data)}));
        }
        let record = serde_json::json!({"target":target,"graphSha256":arguments[1],"files":records});
        fs::write(output.join("manifest.json"),serde_json::to_vec_pretty(&record).unwrap()).map_err(|error|error.to_string())?;
        println!("{record}");
        return Ok(());
    }
    let smoke_occlusion = playsrc_tf2_wasm::smokestack_occlusion_probe(handle)
        .map(|(identity, position, angles, fraction)| serde_json::json!({"identity":identity,"position":position,"yawDegrees":angles[0],"pitchDegrees":angles[1],"worldFraction":fraction}));
    let parsed = playsrc_bsp::parse(&bsp, playsrc_bsp::Profile::Source2013V20, playsrc_bsp::Limits::default()).map_err(|error| error.to_string())?;
    let entities = playsrc_entity::parse(parsed.lumps[0].bytes(&parsed), playsrc_entity::Limits::default()).map_err(|error| error.to_string())?;
    let mut smoke_camera = None;
    if let Some(stack) = entities.entities.iter().find(|entity| entity.classname.as_deref() == Some(b"env_smokestack")) {
        let origin = stack.pairs.iter().find(|pair| pair.key.eq_ignore_ascii_case(b"origin")).ok_or("smokestack origin")?;
        let origin = std::str::from_utf8(&origin.value).map_err(|e| e.to_string())?.split_whitespace().map(str::parse::<f32>).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        let view = [origin[0], origin[1] - 80.0, origin[2] + 20.0, origin[0], origin[1] - 80.0, origin[2] + 20.0, 90.0, 5.0, 75.0, 16.0 / 9.0, 1.0, 30_000.0, 0.0, -1.0];
        smoke_camera = Some([view[3], view[4], view[5]]);
        if unsafe { playsrc_tf2_wasm::playsrc_visibility_query(handle, view.as_ptr()) } != 1 { return Err("native smokestack view failed".into()); }
    }
    // Exercise real map-authored systems through the same single-call particle
    // boundary as gameplay. This catches operators that fail only on emission.
    let mut particle_request = [0_u8; 40];
    particle_request[..4].copy_from_slice(b"PPTX");
    particle_request[4..8].copy_from_slice(&5_u32.to_le_bytes());
    particle_request[12..16].copy_from_slice(&1.0_f32.to_le_bytes());
    if let Some(camera) = smoke_camera {
        for (axis, value) in camera.into_iter().enumerate() { particle_request[16 + axis * 4..20 + axis * 4].copy_from_slice(&value.to_le_bytes()); }
    }
    if unsafe { playsrc_tf2_wasm::playsrc_particle_transact(handle, particle_request.as_ptr(), particle_request.len()) } != 1 {
        let mut detail = vec![0; playsrc_tf2_wasm::playsrc_simulation_error_length()];
        unsafe { playsrc_tf2_wasm::playsrc_simulation_error_copy(detail.as_mut_ptr(), detail.len()); }
        return Err(format!("native map particle simulation failed: {}", String::from_utf8_lossy(&detail)));
    }
    if particle_owner {
        for index in 0..60_u32 {
            particle_request[8..12].copy_from_slice(&(1.0 + index as f32 / 60.0).to_le_bytes());
            particle_request[12..16].copy_from_slice(&(1.0 + (index + 1) as f32 / 60.0).to_le_bytes());
            if unsafe { playsrc_tf2_wasm::playsrc_particle_transact(handle, particle_request.as_ptr(), particle_request.len()) } != 1 { return Err("native authored particle frame failed".into()); }
            if [0, 1, 15, 30, 59].contains(&index) {
                let mut bytes = vec![0; playsrc_tf2_wasm::playsrc_particle_output_length(handle)];
                if bytes.len() > 8 * 1024 * 1024 || unsafe { playsrc_tf2_wasm::playsrc_particle_output_copy(handle, bytes.as_mut_ptr(), bytes.len()) } != bytes.len() { return Err("particle diagnostic output exceeds bound or copy failed".into()); }
                let name = format!("frame-{index}.bin");
                fs::write(particle_directory.join(&name), &bytes).map_err(|e|e.to_string())?;
                particle_records.push(serde_json::json!({"name":name,"bytes":bytes.len(),"sha256":hex_hash(&bytes),"request":particle_request.to_vec()}));
            }
        }
    }
    if !particle_owner && playsrc_tf2_wasm::playsrc_legacy_particle_frames(handle) == 1 {
        let mut visual=b"PLVQ".to_vec();
        for value in [3_u32,1,1280,1,0] {visual.extend_from_slice(&value.to_le_bytes());}
        visual.extend_from_slice(&0.0_f32.to_le_bytes());
        for value in [720_u32,0] {visual.extend_from_slice(&value.to_le_bytes());}
        let position=smoke_camera.unwrap_or([0.0;3]);
        for value in position.into_iter().chain(position).chain([90.0,5.0,75.0,16.0/9.0,1.0,30_000.0]) {visual.extend_from_slice(&value.to_le_bytes());}
        let mut frame = vec![0_u8; 64];
        frame[..32].copy_from_slice(&particle_request[..32]);
        frame[28..32].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
        frame[32..36].copy_from_slice(&(1.0_f32 / 60.0).to_le_bytes());
        frame[60..64].copy_from_slice(&(visual.len() as u32).to_le_bytes());frame.extend_from_slice(&visual);
        for (axis, value) in [90.0_f32, 5.0, 75.0, 16.0 / 9.0].into_iter().enumerate() { frame[44 + axis * 4..48 + axis * 4].copy_from_slice(&value.to_le_bytes()); }
        for index in 0..60_u32 {
            let now = index as f32 / 60.0;
            frame[8..12].copy_from_slice(&now.to_le_bytes()); frame[12..16].copy_from_slice(&now.to_le_bytes());
            frame[88..92].copy_from_slice(&now.to_le_bytes());
            frame[36..40].copy_from_slice(&index.to_le_bytes()); frame[40..44].copy_from_slice(&(index + 1).to_le_bytes());
            if unsafe { playsrc_tf2_wasm::playsrc_particle_transact(handle, frame.as_ptr(), frame.len()) } != 1 { return Err("native legacy particle frame failed".into()); }
        }
    }
    let particle_bytes = playsrc_tf2_wasm::playsrc_particle_output_length(handle);
    let mut particle_output = vec![0; particle_bytes];
    unsafe { playsrc_tf2_wasm::playsrc_particle_output_copy(handle, particle_output.as_mut_ptr(), particle_bytes); }
    if particle_output.len()<40||&particle_output[..4]!=b"PSPR"||u32::from_le_bytes(particle_output[4..8].try_into().unwrap())!=5{return Err("native particle output header differs".into());}
    let records=u32::from_le_bytes(particle_output[8..12].try_into().unwrap()) as usize;
    let record_end=records.checked_mul(436).and_then(|bytes|bytes.checked_add(40)).filter(|end|*end<=particle_output.len()).ok_or("native particle output record range")?;
    let sky_particles = particle_output[40..record_end].chunks_exact(436).filter(|record| record[15] == 1).count();
    if particle_owner {
        let mut presentation = vec![0; playsrc_tf2_wasm::playsrc_presentation_length(handle)];
        if unsafe { playsrc_tf2_wasm::playsrc_presentation_copy(handle, presentation.as_mut_ptr(), presentation.len()) } != presentation.len() { return Err("particle presentation copy failed".into()); }
        for (name, bytes) in [("final.bin", &particle_output), ("presentation.pspr", &presentation), ("resources.psdb", &resources)] {
            fs::write(particle_directory.join(name), bytes).map_err(|e|e.to_string())?;
            particle_records.push(serde_json::json!({"name":name,"bytes":bytes.len(),"sha256":hex_hash(bytes)}));
        }
        let record = serde_json::json!({"target":target,"bspSha256":hash,"graphSha256":arguments[1],"files":particle_records,"scope":"Native fixed-input map emitter diagnostic; not recorded browser combat, real elapsed soak, GPU execution or pixels"});
        fs::write(particle_directory.join("manifest.json"),serde_json::to_vec(&record).unwrap()).map_err(|e|e.to_string())?;
        println!("{record}"); return Ok(());
    }
    let output = config.source_cache_dir.join("evidence/map-runtime");
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    fs::write(output.join(format!("{target}.psmp")), &payload)
        .map_err(|error| error.to_string())?;
    let mut spawns = Vec::new();
    let mut particle_systems = Vec::new();
    let mut sky_cameras = Vec::new();
    let mut smokestacks = Vec::new();
    let mut legacy_visuals = Vec::new();
    for entity in &entities.entities {
        if ![b"info_player_teamspawn".as_slice(), b"info_particle_system", b"sky_camera", b"env_lightglow", b"env_sprite", b"env_glow", b"env_sprite_oriented", b"point_spotlight", b"env_sun", b"env_smokestack"].contains(&entity.classname.as_deref().unwrap_or_default()) { continue; }
        let value = |name: &[u8]| entity.pairs.iter().find(|pair| pair.key.eq_ignore_ascii_case(name)).map(|pair| String::from_utf8_lossy(&pair.value).into_owned());
        let vector = |name: &[u8]| -> Result<[f32; 3], String> {
            let values = value(name).unwrap_or_else(|| "0 0 0".to_owned()).split_whitespace().map(str::parse::<f32>).collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
            values.try_into().map_err(|_| "spawn vector is invalid".to_owned())
        };
        match entity.classname.as_deref().unwrap_or_default() {
            b"point_spotlight"=>{
                if let Some((beam,color))=playsrc_tf2_wasm::map_spotlight_presentation(handle,entity.index as u32){
                    legacy_visuals.push(serde_json::json!({"identity":entity.index,"classname":"point_spotlight","position":beam.start,"end":beam.end,"width":beam.width,"endWidth":beam.end_width,"color":color.color,"hdrScale":beam.hdr_scale}));
                }
            }
            b"env_sun"=>{
                let sun=playsrc_tf2_wasm::map_sun_presentation(handle,entity.index as u32).ok_or("native sun state missing")?;
                legacy_visuals.push(serde_json::json!({"identity":entity.index,"classname":"env_sun","name":value(b"targetname"),"position":vector(b"origin")?,"direction":sun.direction,"size":sun.size,"overlaySize":sun.overlay_size,"hdrScale":sun.hdr_scale}));
            }
            b"env_smokestack" => smokestacks.push(serde_json::json!({"identity":entity.index,"name":value(b"targetname"),"material":value(b"SmokeMaterial"),"position":vector(b"origin")?,"active":value(b"InitialState"),"rate":value(b"Rate"),"speed":value(b"Speed"),"jetLength":value(b"JetLength"),"color":value(b"rendercolor")})),
            b"info_particle_system" => particle_systems.push(serde_json::json!({"identity":entity.index,"name":value(b"targetname"),"effect":value(b"effect_name"),"position":vector(b"origin")?,"active":value(b"start_active")})),
            b"sky_camera" => sky_cameras.push(serde_json::json!({"position":vector(b"origin")?,"scale":value(b"scale")})),
            b"info_player_teamspawn" => spawns.push(serde_json::json!({ "identity": entity.index, "team": value(b"TeamNum"), "position": vector(b"origin")?, "angles": vector(b"angles")?, "disabled": value(b"StartDisabled"), "classFlags": value(b"spawnflags") })),
            _ => legacy_visuals.push(serde_json::json!({"identity":entity.index,"classname":value(b"classname"),"name":value(b"targetname"),"position":vector(b"origin")?,"angles":vector(b"angles")?,"minimumDistance":value(b"MinDist"),"maximumDistance":value(b"MaxDist"),"outerMaximumDistance":value(b"OuterMaxDist"),"color":value(b"rendercolor"),"spawnflags":value(b"spawnflags"),"model":value(b"model"),"renderMode":value(b"rendermode"),"renderFx":value(b"renderfx"),"scale":value(b"scale")})),
        }
    }
    let ropes=playsrc_tf2_wasm::verified_rope_facts(handle).into_iter().map(|rope|{
        let name=entities.entities.iter().find(|entity|entity.index==rope.source as usize).and_then(|entity|entity.targetname.as_deref()).map(|name|String::from_utf8_lossy(name).into_owned());
        serde_json::json!({"source":rope.source,"nodes":rope.nodes,"noWind":rope.no_wind,"material":rope.material,"name":name,"cameras":rope.cameras})
    }).collect::<Vec<_>>();
    fs::write(output.join(format!("{target}.facts.json")), serde_json::to_vec(&serde_json::json!({"target": target, "bspSha256": hash, "spawns": spawns, "particleSystems":particle_systems,"skyCameras":sky_cameras,"smokestacks":smokestacks,"smokeOcclusion":smoke_occlusion,"legacyVisuals":legacy_visuals,"ropes":ropes})).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::json!({"target": target, "graphSha256": arguments[1], "byteLength": payload.len(), "sha256": hex_hash(&payload), "particleOutputBytes": particle_bytes,"skyParticles":sky_particles})
    );
    Ok(())
}
