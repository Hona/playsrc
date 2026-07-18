use playsrc_bsp::{Limits as BspLimits, LumpData, Profile as BspProfile, parse as parse_bsp};
use playsrc_entity::{
    EntityWorld, EntityWorldConfig, ExternalBrushModelBinding, ExternalBrushModelVisibility,
    ExternalClassBinding, ModelBounds,
};
use playsrc_map::{BrushModelIdentity, LightingProfile, compile};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const COLLISION_WORLD_SHA256: &str =
    "66d42c750648487669e1b9d7a1b36fc81e213624030f812667fb728ee61aa6ed";

struct Config {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[test]
#[ignore = "requires playsrc.local.json and the configured jump_beef BSP"]
fn configured_jump_beef_enumerates_every_brush_model_and_entity_draw_fact() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = parse_config(&fs::read(root.join("playsrc.local.json")).unwrap()).unwrap();
    assert!(PathBuf::from(&config.tf2_dir).is_absolute());
    assert!(PathBuf::from(&config.asset_dir).is_absolute());
    let cache = PathBuf::from(config.source_cache_dir);
    assert!(cache.is_absolute());
    let bytes = fs::read(
        cache
            .join("objects/sha256")
            .join(&BSP_SHA256[..2])
            .join(BSP_SHA256),
    )
    .unwrap();
    assert_eq!(hex(&Sha256::digest(&bytes)), BSP_SHA256);
    let bsp = parse_bsp(&bytes, BspProfile::Source2013V20, BspLimits::default()).unwrap();
    let map = compile(&bsp, LightingProfile::Hdr).unwrap();
    assert_eq!(map.brush_models.len(), 123);
    assert_eq!(map.surfaces.len(), 3_793);
    assert_eq!(map.materials.len(), 14);
    assert_eq!(hex(&map.collision_world_identity), COLLISION_WORLD_SHA256);
    assert_eq!(map.brush_model_occurrences.len(), 122);
    assert_eq!(map.brush_models[0].identity, BrushModelIdentity::World);

    for model in &map.brush_models {
        assert_eq!(
            model.identity,
            if model.index == 0 {
                BrushModelIdentity::World
            } else {
                BrushModelIdentity::Inline(model.index)
            }
        );
        let surfaces = &map.surfaces[model.surface_range.clone()];
        assert!(surfaces.iter().all(|surface| surface.model == model.index));
        assert_eq!(
            model.vertex_count,
            surfaces
                .iter()
                .map(|surface| surface.positions.len())
                .sum::<usize>()
        );
        assert_eq!(
            model.triangle_count,
            surfaces
                .iter()
                .map(|surface| surface.triangles.len())
                .sum::<usize>()
        );
        let mut materials = Vec::new();
        for surface in surfaces {
            if !materials.contains(&surface.material) {
                materials.push(surface.material);
            }
        }
        assert_eq!(model.materials, materials);
        assert!(!model.collision_brushes.is_empty());
        println!(
            "model={} faces={}..{} materials={:?} entities={:?} origin={:?} bounds={:?}",
            model.index,
            model.surface_range.start,
            model.surface_range.end,
            model.materials,
            model.entities,
            model.origin,
            model.bounds,
        );
    }

    let graph =
        playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default()).unwrap();
    let bounds = map
        .brush_models
        .iter()
        .map(|model| ModelBounds {
            model: model.index,
            mins: model.bounds[0],
            maxs: model.bounds[1],
        })
        .collect();
    let (world, _) = EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            source_identity: u64::from_le_bytes(Sha256::digest(&bytes)[..8].try_into().unwrap()),
            registry_identity: 0x4252_5553_485f_4d31,
            model_bounds: bounds,
            external_classes: vec![
                ExternalClassBinding {
                    classname: b"func_regenerate".to_vec(),
                    inputs: vec![b"Enable".to_vec(), b"Disable".to_vec(), b"Toggle".to_vec()],
                },
                ExternalClassBinding {
                    classname: b"func_respawnroom".to_vec(),
                    inputs: vec![b"SetActive".to_vec(), b"SetInactive".to_vec()],
                },
            ],
            external_brush_models: [b"func_regenerate".as_slice(), b"func_respawnroom"]
                .into_iter()
                .map(|classname| ExternalBrushModelBinding {
                    classname: classname.to_vec(),
                    initial_visibility: ExternalBrushModelVisibility::Hidden,
                })
                .collect(),
            ..EntityWorldConfig::default()
        },
    )
    .unwrap();
    let presentation = world.brush_model_presentation(world.revision()).unwrap();
    let expected_entities = graph
        .entities
        .iter()
        .filter(|entity| entity.bsp_model_index.is_some_and(|model| model != 0))
        .count();
    assert_eq!(presentation.models.len(), expected_entities);
    assert_eq!(
        presentation
            .models
            .iter()
            .filter(|state| state.mover.is_some())
            .count(),
        10
    );
    assert_eq!(
        presentation
            .models
            .iter()
            .filter(|state| state.draw)
            .count(),
        17
    );
    let fixed_mover = presentation
        .models
        .iter()
        .find(|state| state.source_index == 67)
        .unwrap();
    assert_eq!(fixed_mover.model, 26);
    assert_eq!(
        fixed_mover.world_transform.origin,
        [11_545.3, 328.0, -3_308.0]
    );
    for source_index in [20, 21, 85, 151, 237, 314, 324] {
        assert!(
            !presentation
                .models
                .iter()
                .find(|state| state.source_index == source_index)
                .unwrap()
                .draw
        );
    }
    for state in &presentation.models {
        assert!(state.model < map.brush_models.len());
        assert!(
            state
                .local_transform
                .origin
                .into_iter()
                .chain(state.local_transform.angles)
                .chain(state.world_transform.origin)
                .chain(state.world_transform.angles)
                .all(f32::is_finite)
        );
        println!(
            "entity={} class={} model={} draw={} mode={} color={:?} fx={} effects={} local={:?} world={:?} parent={:?} mover={:?}",
            state.source_index,
            String::from_utf8_lossy(
                graph.entities[state.source_index]
                    .classname
                    .as_deref()
                    .unwrap_or_default(),
            ),
            state.model,
            state.draw,
            state.render_mode,
            state.color,
            state.render_fx,
            state.effects,
            state.local_transform,
            state.world_transform,
            state.parent,
            state.mover,
        );
    }

    let LumpData::Models(source_models) = &bsp.lumps[14].records else {
        panic!("missing source models")
    };
    assert_eq!(source_models.len(), map.brush_models.len());
    assert_eq!(map.brush_models[109].collision_brushes, [454]);
    assert_eq!(map.brush_models[109].collision_contents, 1);
    assert_eq!(map.brush_models[122].surface_range, 3793..3793);
    assert_eq!(map.brush_models[122].collision_brushes, [475]);
    assert_eq!(map.brush_models[122].collision_contents, 1);
    for (entity, model, solidity, contents) in [
        (294, 109, b"0".as_slice(), 1),
        (295, 110, b"0".as_slice(), 1),
        (296, 111, b"0".as_slice(), 1),
        (297, 112, b"0".as_slice(), 1),
        (307, 113, b"1".as_slice(), 0x1000_0008),
        (322, 117, b"1".as_slice(), 0x1000_0008),
        (323, 118, b"1".as_slice(), 0x1000_0008),
    ] {
        let occurrence = map
            .brush_model_occurrences
            .iter()
            .find(|occurrence| occurrence.entity == entity)
            .unwrap();
        assert_eq!(occurrence.model, model);
        assert_eq!(occurrence.classname, b"func_brush");
        assert_eq!(occurrence.start_disabled.as_deref(), Some(b"0".as_slice()));
        assert_eq!(occurrence.solidity.as_deref(), Some(solidity));
        assert_eq!(map.brush_models[model].collision_contents, contents);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_config(bytes: &[u8]) -> Result<Config, &'static str> {
    let mut parser = JsonParser { bytes, offset: 0 };
    parser.space();
    parser.byte(b'{')?;
    let mut values = BTreeMap::new();
    loop {
        parser.space();
        if parser.take(b'}') {
            break;
        }
        let key = parser.string()?;
        parser.space();
        parser.byte(b':')?;
        parser.space();
        let value = parser.string()?;
        if values.insert(key, value).is_some() {
            return Err("duplicate configuration key");
        }
        parser.space();
        if parser.take(b'}') {
            break;
        }
        parser.byte(b',')?;
    }
    parser.space();
    if parser.offset != bytes.len()
        || values.keys().map(String::as_str).collect::<Vec<_>>()
            != ["assetDir", "sourceCacheDir", "tf2Dir"]
    {
        return Err("invalid configuration shape");
    }
    Ok(Config {
        tf2_dir: values.remove("tf2Dir").unwrap(),
        source_cache_dir: values.remove("sourceCacheDir").unwrap(),
        asset_dir: values.remove("assetDir").unwrap(),
    })
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl JsonParser<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn take(&mut self, expected: u8) -> bool {
        if self.bytes.get(self.offset) == Some(&expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn byte(&mut self, expected: u8) -> Result<(), &'static str> {
        self.take(expected)
            .then_some(())
            .ok_or("invalid JSON token")
    }

    fn string(&mut self) -> Result<String, &'static str> {
        self.byte(b'"')?;
        let mut output = String::new();
        loop {
            let byte = *self
                .bytes
                .get(self.offset)
                .ok_or("unterminated JSON string")?;
            self.offset += 1;
            match byte {
                b'"' => return Ok(output),
                b'\\' => {
                    let escaped = *self
                        .bytes
                        .get(self.offset)
                        .ok_or("unterminated JSON escape")?;
                    self.offset += 1;
                    output.push(match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode_escape()?,
                        _ => return Err("unsupported configuration escape"),
                    });
                }
                0x00..=0x1f => return Err("control byte in JSON string"),
                _ if byte.is_ascii() => output.push(char::from(byte)),
                _ => {
                    let start = self.offset - 1;
                    let tail = std::str::from_utf8(&self.bytes[start..])
                        .map_err(|_| "configuration is not UTF-8")?;
                    let character = tail.chars().next().ok_or("invalid UTF-8")?;
                    self.offset = start + character.len_utf8();
                    output.push(character);
                }
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, &'static str> {
        let first = self.hex_quad()?;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            self.byte(b'\\')?;
            self.byte(b'u')?;
            let second = self.hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err("invalid JSON surrogate pair");
            }
            0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err("unpaired JSON surrogate");
        } else {
            u32::from(first)
        };
        char::from_u32(scalar).ok_or("invalid JSON scalar")
    }

    fn hex_quad(&mut self) -> Result<u16, &'static str> {
        let bytes = self
            .bytes
            .get(self.offset..self.offset + 4)
            .ok_or("truncated JSON Unicode escape")?;
        self.offset += 4;
        bytes.iter().try_fold(0_u16, |value, byte| {
            let digit = match byte {
                b'0'..=b'9' => u16::from(*byte - b'0'),
                b'a'..=b'f' => u16::from(*byte - b'a' + 10),
                b'A'..=b'F' => u16::from(*byte - b'A' + 10),
                _ => return Err("invalid JSON Unicode escape"),
            };
            Ok((value << 4) | digit)
        })
    }
}
