use playsrc_vtf::{
    ChannelLayout, ColorEncoding, Decoder, Dialect, ErrorCode, Face, ImageFormat, Limits, RowOrder,
    SamplingEnvironment, SubresourceIdentity, sampling_state,
};
use std::{fs, path::Path};

const SHARED_NORMAL: &str = "7b5de49340bfe1ec2f1e37d771289d42773414f130767b5632ca29467494c017";
const UPWARD_NORMAL: &str = "f763f3afc234f3ad6e9468dc9a98cca0e289f67810d8b6669f4cefd61cc5aea5";
const UPWARD_BASE: &str = "f035cc70dfd265564ed6ed33f322eef7a025ab42f616349b92ee85d514281429";

#[test]
#[ignore = "requires bun packages/world/material/scripts/water-content.ts"]
fn configured_water_normals_and_stock_base_retain_every_authored_plane() {
    let shared_bytes = source(SHARED_NORMAL);
    let shared = Decoder::new(&shared_bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
    let metadata = shared.metadata();
    assert_eq!(metadata.version, (7, 3));
    assert_eq!((metadata.width, metadata.height), (256, 256));
    assert_eq!((metadata.frame_count, metadata.mip_count), (60, 9));
    assert_eq!(metadata.high_format, ImageFormat::Bgr888);
    assert_eq!(metadata.effective_flags, 0x80);
    let planes = shared.decode_high_resolution().unwrap();
    assert_eq!(planes.len(), 540);
    assert_eq!(
        planes.first().unwrap().identity,
        SubresourceIdentity::HighResolution {
            mip: 8,
            frame: 0,
            face: Face::Right,
            slice: 0,
        },
    );
    assert_eq!(
        planes.last().unwrap().identity,
        SubresourceIdentity::HighResolution {
            mip: 0,
            frame: 59,
            face: Face::Right,
            slice: 0,
        },
    );
    assert!(planes.iter().all(|plane| {
        plane.channel_layout == ChannelLayout::Rgb
            && plane.color_encoding == ColorEncoding::NotColor
            && plane.row_order == RowOrder::TopToBottom
    }));
    assert_eq!(
        planes
            .iter()
            .map(|plane| plane.samples.len())
            .sum::<usize>(),
        15_728_580,
    );
    for frame in [0, 30] {
        let plane = shared
            .decode(SubresourceIdentity::HighResolution {
                mip: 0,
                frame,
                face: Face::Right,
                slice: 0,
            })
            .unwrap();
        fs::write(
            evidence_directory().join(format!("normal-frame-{frame}.rgb")),
            &plane.samples,
        )
        .unwrap();
    }

    let upward_bytes = source(UPWARD_NORMAL);
    let upward = Decoder::new(&upward_bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
    let metadata = upward.metadata();
    assert_eq!((metadata.frame_count, metadata.mip_count), (30, 9));
    assert_eq!(metadata.high_format, ImageFormat::Dxt1);
    assert_eq!(metadata.effective_flags, 0x280);
    let sampling = sampling_state(
        metadata,
        SamplingEnvironment {
            shader_model: 90,
            force_anisotropy: 1,
            maximum_anisotropy: 1,
            force_trilinear: false,
        },
    );
    assert!(sampling.mipmapped);
    assert!(sampling.no_lod);
    let planes = upward.decode_high_resolution().unwrap();
    assert_eq!(planes.len(), 270);
    assert!(planes.iter().all(|plane| {
        plane.channel_layout == ChannelLayout::Rgba
            && plane.color_encoding == ColorEncoding::NotColor
            && plane.row_order == RowOrder::TopToBottom
    }));
    assert_eq!(
        planes
            .iter()
            .map(|plane| plane.samples.len())
            .sum::<usize>(),
        10_485_720,
    );

    let base_bytes = source(UPWARD_BASE);
    let base = Decoder::new(&base_bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
    let metadata = base.metadata();
    assert_eq!(metadata.version, (7, 4));
    assert_eq!((metadata.frame_count, metadata.mip_count), (1, 9));
    assert_eq!(metadata.high_format, ImageFormat::Dxt5);
    assert!(metadata.alpha_flags.eight_bit);
    let planes = base.decode_high_resolution().unwrap();
    assert_eq!(planes.len(), 9);
    assert!(planes.iter().all(|plane| {
        plane.channel_layout == ChannelLayout::Rgba
            && plane.color_encoding == ColorEncoding::Srgb
            && plane.row_order == RowOrder::TopToBottom
    }));
    assert_eq!(
        planes
            .iter()
            .map(|plane| plane.samples.len())
            .sum::<usize>(),
        349_524,
    );

    let rejected = Decoder::new(
        &shared_bytes,
        Dialect::Source2013Pc,
        Limits {
            max_decoded_bytes: 15_728_579,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        rejected.decode_high_resolution().unwrap_err().code,
        ErrorCode::AllocationLimit,
    );
}

fn source(hash: &str) -> Vec<u8> {
    fs::read(evidence_directory().join("content").join(hash)).unwrap()
}

fn evidence_directory() -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let cache = config
        .split_once("\"sourceCacheDir\"")
        .unwrap()
        .1
        .split_once(':')
        .unwrap()
        .1
        .trim_start()
        .strip_prefix('"')
        .unwrap();
    let cache = &cache[..cache.find('"').unwrap()];
    Path::new(cache).join("evidence/tf2-water-rendering")
}
