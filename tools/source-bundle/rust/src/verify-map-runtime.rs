use playsrc_asset_graph::{ResourceGraph, decode, encode_resource_set, hex_hash};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

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
    if !(arguments.len() == 2 || arguments.len() == 3 && arguments[2] == "--control-point-match" || arguments.len() == 6 && arguments[2] == "--view") || !digest_identity(&arguments[1])
    {
        return Err(
            "usage: playsrc-verify-map-runtime <target> <retained-graph-sha256> [--view x y z | --control-point-match]".to_owned(),
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
    if arguments.len() == 3 {
        let mut frames = Vec::new();
        let result = playsrc_tf2_wasm::verify_control_point_match(&bsp, &resources, |snapshot| {
            frames.push(serde_json::json!({"tick":snapshot.tick,"state":snapshot.round.state as u8,"winner":snapshot.round.winning_team.map(|team|team as u8),"points":snapshot.control_points.as_ref().map(|points|points.points.iter().map(|p|p.owner as u8).collect::<Vec<_>>()),"bots":snapshot.bots.iter().map(|bot|serde_json::json!({"identity":bot.identity,"position":bot.position,"area":bot.area,"path":bot.remaining_path_areas,"captures":bot.captures})).collect::<Vec<_>>() }));
        });
        let output = config.source_cache_dir.join("evidence/map-runtime");
        fs::create_dir_all(&output).map_err(|error|error.to_string())?;
        fs::write(output.join(format!("{target}-control-point-match.json")),serde_json::to_vec(&frames).unwrap()).map_err(|error|error.to_string())?;
        return result;
    }
    let section = playsrc_tf2_wasm::ResourceSection { pointer: resources.as_ptr(), length: resources.len() };
    let resource_hash: [u8; 32] = Sha256::digest(&resources).into();
    let handle = unsafe { playsrc_tf2_wasm::playsrc_compile_map(bsp.as_ptr(), bsp.len(), 1, &section, 1, resource_hash.as_ptr()) };
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
    playsrc_tf2_wasm::playsrc_dispose(handle);
    let payload = result?;
    let output = config.source_cache_dir.join("evidence/map-runtime");
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    fs::write(output.join(format!("{target}.psmp")), &payload)
        .map_err(|error| error.to_string())?;
    let parsed = playsrc_bsp::parse(&bsp, playsrc_bsp::Profile::Source2013V20, playsrc_bsp::Limits::default()).map_err(|error| error.to_string())?;
    let entities = playsrc_entity::parse(parsed.lumps[0].bytes(&parsed), playsrc_entity::Limits::default()).map_err(|error| error.to_string())?;
    let mut spawns = Vec::new();
    for entity in &entities.entities {
        if entity.classname.as_deref() != Some(b"info_player_teamspawn") { continue; }
        let value = |name: &[u8]| entity.pairs.iter().find(|pair| pair.key.eq_ignore_ascii_case(name)).map(|pair| String::from_utf8_lossy(&pair.value).into_owned());
        let vector = |name: &[u8]| -> Result<[f32; 3], String> {
            let values = value(name).unwrap_or_else(|| "0 0 0".to_owned()).split_whitespace().map(str::parse::<f32>).collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
            values.try_into().map_err(|_| "spawn vector is invalid".to_owned())
        };
        spawns.push(serde_json::json!({ "identity": entity.index, "team": value(b"TeamNum"), "position": vector(b"origin")?, "angles": vector(b"angles")?, "disabled": value(b"StartDisabled"), "classFlags": value(b"spawnflags") }));
    }
    fs::write(output.join(format!("{target}.facts.json")), serde_json::to_vec(&serde_json::json!({"target": target, "bspSha256": hash, "spawns": spawns})).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::json!({"target": target, "graphSha256": arguments[1], "byteLength": payload.len(), "sha256": hex_hash(&payload)})
    );
    Ok(())
}
