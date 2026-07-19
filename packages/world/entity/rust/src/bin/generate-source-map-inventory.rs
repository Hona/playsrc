use playsrc_bsp::{Limits as BspLimits, Profile as BspProfile, parse as parse_bsp};
use std::{fmt::Write as _, fs, path::PathBuf};

const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const SDK_REVISION: &str = "88fa198fba3fb85d46d4c95018254693fdc3af0a";

const SELECTED: &[(&str, &str)] = &[
    (
        "field.typed-projection-and-writable-input",
        "SDK datamap key/input fields and variant conversion matrix",
    ),
    (
        "hierarchy.attachments",
        "SetParent, attachment snap/maintain-offset, ClearParent and live attachment transforms",
    ),
    (
        "template.point-template",
        "16 expressions, captured relative transforms, prototypes, fixups and atomic ForceSpawn",
    ),
    (
        "logic.timer",
        "fixed/random interval, alternating output and complete timer inputs",
    ),
    (
        "trigger.contacts",
        "multiple/hurt/push/catapult/teleport accepted-contact state and typed effects",
    ),
    (
        "mover.func-button",
        "linear button endpoints, lock, wait, damage and output context",
    ),
    (
        "mover.func-rot-button",
        "angular button endpoints and block/carry request",
    ),
    (
        "mover.momentary-rot-button",
        "positioned angular endpoint and immediate/current transform",
    ),
    (
        "mover.func-door",
        "linear door endpoints, wait, lock, block and reversal",
    ),
    (
        "mover.func-door-rotating",
        "angular door endpoints, start position, wait, block and reversal",
    ),
    (
        "mover.func-movelinear",
        "position input, speed replacement and endpoint outputs",
    ),
    (
        "mover.func-rotating",
        "continuous angular speed, start/stop/toggle/reverse and current transform",
    ),
    (
        "mover.func-plat",
        "top/bottom endpoints, toggle, auto-return and block reversal",
    ),
    (
        "mover.func-platrot",
        "synchronized top/bottom translation and rotation",
    ),
    (
        "mover.func-train",
        "path-corner progression, wait/retrigger/teleport and block state",
    ),
    (
        "mover.func-tracktrain",
        "path-track links, direction/speed inputs, look-ahead segments and current transform",
    ),
    (
        "collision.pusher-current-transform",
        "linear/angular hierarchy carry, reverse-order block and whole-proposal rollback",
    ),
    (
        "collision.trigger-contact-producer",
        "moving brush/box trigger enter/stay/exit and swept crossing",
    ),
    (
        "lifecycle.func-breakable",
        "health inputs, normalized health output, break, nonsolid state and delayed removal",
    ),
    (
        "lifecycle.prop-dynamic",
        "draw/collision/animation request and completion state",
    ),
    (
        "lifecycle.ordinary-pickup",
        "cache interaction, game admission, touch output, respawn/materialize/remove",
    ),
    (
        "snapshot.selected-state",
        "version-5 Entity, version-2 pusher and version-1 contact continuation state",
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = package.join("../../../..");
    let config = fs::read(root.join("playsrc.local.json"))?;
    let cache = PathBuf::from(configured_string(&config, "sourceCacheDir")?);
    if !cache.is_absolute() {
        return Err("sourceCacheDir must be absolute".into());
    }
    let bytes = fs::read(
        cache
            .join("objects/sha256")
            .join(&BSP_SHA256[..2])
            .join(BSP_SHA256),
    )?;
    if hex(&sha256(&bytes)) != BSP_SHA256 {
        return Err("configured BSP identity differs".into());
    }
    let bsp = parse_bsp(&bytes, BspProfile::Source2013V20, BspLimits::default())?;
    let graph = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())?;
    if (
        graph.inventory.entity_count,
        graph.inventory.pair_count,
        graph.inventory.parsed_connections,
        graph.inventory.malformed_connections,
    ) != (361, 3_674, 66, 1)
    {
        return Err("configured entity inventory differs".into());
    }

    let mut output = String::new();
    writeln!(output, "# Selected Source Map Foundation Inventory\n")?;
    writeln!(output, "Owner: [`../ROADMAP.md`](../ROADMAP.md)\n")?;
    writeln!(
        output,
        "State: Generated; reviewed for the source-map-foundations checkpoint.\n"
    )?;
    writeln!(output, "| Field | Value |")?;
    writeln!(output, "|---|---|")?;
    writeln!(output, "| SDK revision | `{SDK_REVISION}` |")?;
    writeln!(
        output,
        "| Configured BSP | `maps/jump_beef.bsp`, SHA-256 `{BSP_SHA256}` |"
    )?;
    writeln!(
        output,
        "| Generator | `cargo run -p playsrc-entity --bin generate-source-map-inventory` |"
    )?;
    writeln!(output, "| Selected behavior items | {} |", SELECTED.len())?;
    writeln!(
        output,
        "| Configured class identities | {} |\n",
        graph.inventory.class_counts.len()
    )?;
    writeln!(output, "## Selected Generic Behaviors\n")?;
    writeln!(
        output,
        "| Stable identity | Exact selected contract | Evidence status |"
    )?;
    writeln!(output, "|---|---|---|")?;
    for (identity, contract) in SELECTED {
        writeln!(output, "| `{identity}` | {contract}. | Ready |")?;
    }
    writeln!(output, "\n## Configured Class Occurrences\n")?;
    writeln!(output, "| Classname | Count | Owner disposition |")?;
    writeln!(output, "|---|---:|---|")?;
    for (class, count) in &graph.inventory.class_counts {
        let class = std::str::from_utf8(class)?;
        writeln!(output, "| `{class}` | {count} | {} |", disposition(class))?;
    }
    writeln!(
        output,
        "\nConfigured totals: 361 entities, 3,674 ordered key/value pairs, 66 parsed output actions, and one malformed output action. No configured classname is selected by spelling: generic classes are fixed above; every other class remains selected-game, presentation/map, or intentionally inert input."
    )?;
    let destination = package.join("../inventories/source-map-foundations.md");
    let prior = fs::read_to_string(&destination).ok();
    if prior.as_deref() != Some(output.as_str()) {
        fs::write(destination, output)?;
    }
    Ok(())
}

fn disposition(class: &str) -> &'static str {
    match class {
        "worldspawn"
        | "func_brush"
        | "func_button"
        | "func_door"
        | "func_movelinear"
        | "logic_auto"
        | "prop_dynamic"
        | "trigger_hurt"
        | "trigger_multiple"
        | "trigger_teleport"
        | "info_teleport_destination" => "Generic Entity",
        "func_regenerate"
        | "func_respawnroom"
        | "info_player_teamspawn"
        | "item_ammopack_full"
        | "team_round_timer"
        | "tf_gamerules" => "Selected game",
        "game_text" | "info_observer_point" => "Selected game/presentation",
        "infodecal" | "light" | "light_environment" | "light_spot" | "water_lod_control" => {
            "Map/presentation"
        }
        _ => "Unknown",
    }
}

fn configured_string(bytes: &[u8], key: &str) -> Result<String, &'static str> {
    let text = std::str::from_utf8(bytes).map_err(|_| "configuration is not UTF-8")?;
    let marker = format!("\"{key}\"");
    let tail = text
        .split_once(&marker)
        .ok_or("missing configuration key")?
        .1;
    let tail = tail.split_once(':').ok_or("missing configuration value")?.1;
    let mut chars = tail.trim_start().chars();
    if chars.next() != Some('"') {
        return Err("configuration value is not a string");
    }
    let mut output = String::new();
    while let Some(character) = chars.next() {
        match character {
            '"' => return Ok(output),
            '\\' => match chars.next().ok_or("truncated escape")? {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                '/' => output.push('/'),
                'b' => output.push('\u{0008}'),
                'f' => output.push('\u{000c}'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                _ => return Err("unsupported configuration escape"),
            },
            character => output.push(character),
        }
    }
    Err("unterminated configuration string")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const ROUND: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    let mut hash = INITIAL;
    for block in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes(word.try_into().expect("four-byte SHA-256 word"));
        }
        for index in 16..64 {
            let first = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let second = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(first)
                .wrapping_add(words[index - 7])
                .wrapping_add(second);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = hash;
        for index in 0..64 {
            let sigma_one = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let first = h
                .wrapping_add(sigma_one)
                .wrapping_add(choice)
                .wrapping_add(ROUND[index])
                .wrapping_add(words[index]);
            let sigma_zero = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = sigma_zero.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        for (current, value) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *current = current.wrapping_add(value);
        }
    }
    let mut output = [0_u8; 32];
    for (destination, value) in output.chunks_exact_mut(4).zip(hash) {
        destination.copy_from_slice(&value.to_be_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{hex, sha256};

    #[test]
    fn checked_sha256_matches_fixed_vector() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
