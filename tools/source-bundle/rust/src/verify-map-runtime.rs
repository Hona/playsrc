use playsrc_asset_graph::{ResourceGraph, decode, encode_resource_set, hex_hash};
use serde::Deserialize;
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
    if arguments.len() != 2 || !digest_identity(&arguments[1])
    {
        return Err(
            "usage: playsrc-verify-map-runtime <target> <retained-graph-sha256>".to_owned(),
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
    let artifact = playsrc_tf2_wasm::compile_artifact(&bsp, 1, &resources)
        .map_err(|error| format!("native HDR compilation failed: {error}"))?;
    let output = config.source_cache_dir.join("evidence/map-runtime");
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    fs::write(output.join(format!("{target}.psmp")), &artifact.payload)
        .map_err(|error| error.to_string())?;
    println!(
        "{}",
        serde_json::json!({"target": target, "graphSha256": arguments[1], "byteLength": artifact.payload.len(), "sha256": hex_hash(&artifact.payload)})
    );
    Ok(())
}
