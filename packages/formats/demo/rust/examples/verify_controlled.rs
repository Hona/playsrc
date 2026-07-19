use std::{env, fs, process::ExitCode};

use playsrc_demo::{CommandBody, Limits, Profile, SourceIdentity, TerminalTick, parse};

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("missing internal capture path")?;
    let name = args.next().ok_or("missing capture identity")?;
    let hash = parse_hash(&args.next().ok_or("missing capture sha256")?)?;
    if args.next().is_some() {
        return Err("unexpected verifier argument".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("cannot read capture: {error}"))?;
    let limits = Limits {
        max_input_bytes: 64 * 1024 * 1024,
        max_command_records: 2_000_000,
        max_encoded_bytes_per_command: 4 * 1024 * 1024,
        max_total_encoded_command_bytes: 64 * 1024 * 1024,
        max_retained_diagnostic_bytes: 256,
        max_streaming_buffer_bytes: 64 * 1024 * 1024,
    };
    let demo = parse(
        &bytes,
        SourceIdentity {
            name,
            byte_length: bytes.len() as u64,
            sha256: hash,
        },
        Profile::Tf2Demo3Net24,
        limits,
    )
    .map_err(|error| error.to_string())?;
    let terminal_bytes = match &demo.commands.last().ok_or("capture has no command")?.body {
        CommandBody::Stop {
            tick: TerminalTick::SourceTvStreamFlush { .. },
        } => 4,
        CommandBody::Stop {
            tick: TerminalTick::Complete(_),
        } => 5,
        _ => return Err("capture does not end in stop".into()),
    };
    Ok(format!(
        concat!(
            "{{\"profile\":\"{}\",\"server\":\"{}\",\"client\":\"{}\",",
            "\"map\":\"{}\",\"gameDirectory\":\"{}\",\"playbackTimeBits\":{},",
            "\"playbackTicks\":{},\"playbackFrames\":{},\"signonLength\":{},",
            "\"commandCounts\":{:?},\"sourceTvTerminalBytes\":{}}}"
        ),
        demo.profile.identity(),
        json_ascii(demo.header.server_name.prefix())?,
        json_ascii(demo.header.client_name.prefix())?,
        json_ascii(demo.header.map_name.prefix())?,
        json_ascii(demo.header.game_directory.prefix())?,
        demo.header.playback_time_bits,
        demo.header.playback_ticks,
        demo.header.playback_frames,
        demo.header.signon_length,
        demo.summary.command_counts,
        terminal_bytes,
    ))
}

fn parse_hash(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("sha256 must contain 64 lowercase hexadecimal digits".into());
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "sha256 must contain lowercase hexadecimal digits")?;
    }
    Ok(output)
}

fn json_ascii(value: &[u8]) -> Result<String, String> {
    if !value.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
        return Err("capture header field is not printable ASCII".into());
    }
    Ok(value
        .iter()
        .flat_map(|byte| match byte {
            b'"' => vec!['\\', '"'],
            b'\\' => vec!['\\', '\\'],
            byte => vec![char::from(*byte)],
        })
        .collect())
}
