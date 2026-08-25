use std::{fs, ops::Range, path::PathBuf};

use playsrc_vpk::{SegmentReader, SourceError, SourceErrorCode};
use sha2::{Digest, Sha256};

struct Segments(PathBuf);

impl SegmentReader for Segments {
    fn len(&self, index: u32) -> Result<u64, SourceError> {
        fs::metadata(self.0.join(format!("tf2_misc_{index:03}.vpk")))
            .map(|metadata| metadata.len())
            .map_err(|_| SourceError {
                code: SourceErrorCode::Missing,
                range: 0..0,
            })
    }

    fn read(&self, index: u32, range: Range<u64>) -> Result<Vec<u8>, SourceError> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file =
            fs::File::open(self.0.join(format!("tf2_misc_{index:03}.vpk"))).map_err(|_| {
                SourceError {
                    code: SourceErrorCode::Missing,
                    range: range.clone(),
                }
            })?;
        file.seek(SeekFrom::Start(range.start))
            .map_err(|_| SourceError {
                code: SourceErrorCode::Io,
                range: range.clone(),
            })?;
        let mut bytes = vec![0; usize::try_from(range.end - range.start).unwrap()];
        file.read_exact(&mut bytes).map_err(|_| SourceError {
            code: SourceErrorCode::ShortRead,
            range,
        })?;
        Ok(bytes)
    }
}

#[test]
#[ignore = "requires playsrc.local.json and the exact configured TF2 misc archive"]
fn configured_pl_upward_nav_retains_authored_topology_and_paths() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let marker = "\"tf2Dir\"";
    let tail = &config[config.find(marker).unwrap() + marker.len()..];
    let tail = tail[tail.find(':').unwrap() + 1..].trim_start();
    let value = &tail[1..tail[1..].find('"').unwrap() + 1];
    let content_root = PathBuf::from(value);
    assert!(content_root.is_absolute());
    let index_bytes = fs::read(content_root.join("tf2_misc_dir.vpk")).unwrap();
    let index = playsrc_vpk::parse(
        &index_bytes,
        "tf2_misc_dir.vpk",
        playsrc_vpk::Layout::Split,
        playsrc_vpk::Limits::default(),
    )
    .unwrap();
    let nav = index
        .read_entry("maps/pl_upward.nav", &Segments(content_root.clone()))
        .unwrap();
    assert_eq!(nav.bytes.len(), 2_471_913);
    assert_eq!(
        format!("{:x}", Sha256::digest(&nav.bytes)),
        "13de0c3e2666d2194474d855683cbabb807eead1c24587fd093a5c70a04cd0b4"
    );
    let map_size = fs::metadata(content_root.join("maps/pl_upward.bsp"))
        .unwrap()
        .len();
    assert_eq!(map_size, 25_446_018);
    let mesh = playsrc_nav::parse(
        &nav.bytes,
        playsrc_nav::Profile::TeamFortress2,
        Some(map_size as u32),
        playsrc_nav::Limits::default(),
    )
    .unwrap();
    assert_eq!(mesh.version, 16);
    assert_eq!(mesh.subversion, 2);
    assert_eq!(mesh.areas.len(), 2_617);
    assert_eq!(mesh.ladders.len(), 0);
    assert_eq!(
        mesh.areas
            .iter()
            .flat_map(|area| area.connections.iter())
            .map(Vec::len)
            .sum::<usize>(),
        10_579
    );
    assert_eq!(
        mesh.areas
            .iter()
            .map(|area| area.hiding_spots.len())
            .sum::<usize>(),
        1_305
    );
    assert_eq!(
        mesh.areas
            .iter()
            .map(|area| area.visible_areas.len())
            .sum::<usize>(),
        425_473
    );
    let spawn = mesh.nearest_area([-2528.0, -1744.0, 17.0]).unwrap();
    let payload = mesh.nearest_area([-1674.0, -1536.0, 58.0]).unwrap();
    let path = mesh
        .build_path(spawn.identity, payload.identity, |_, _, _, length| {
            Some(length)
        })
        .unwrap();
    assert!(path.len() > 4);
    assert_eq!(path.first().copied(), Some(spawn.identity));
    assert_eq!(path.last().copied(), Some(payload.identity));
}
