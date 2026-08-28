use playsrc_audio::{
    acoustics::{Detector, Geometry, Hit, TraceKind},
    obstruction,
    wire::{SceneReply, SceneRequest},
};
use std::{collections::BTreeMap, sync::Arc};

pub struct Materials {
    identity: [u8; 32],
    values: Vec<f32>,
    textures: Vec<f32>,
    default: f32,
    props: BTreeMap<u64, f32>,
}
impl Materials {
    pub fn compile(
        map: &playsrc_map::CanonicalMap,
        bsp: &playsrc_bsp::Bsp,
        materials: &[playsrc_material::Material],
        resources: &BTreeMap<String, &[u8]>,
        registry: &playsrc_material::SurfacePropertyRegistry,
    ) -> Result<Self, ()> {
        let default = f32::from_bits(registry.records.first().ok_or(())?.audio_reflectivity_bits);
        let resolve = |name: Option<&[u8]>| {
            registry.resolve(name).map_or(default, |record| {
                f32::from_bits(record.audio_reflectivity_bits)
            })
        };
        let playsrc_bsp::LumpData::TextureInfo(infos) = &bsp.lumps[6].records else {
            return Err(());
        };
        let textures = infos
            .iter()
            .map(|info| {
                let material = materials
                    .get(usize::try_from(info.texture_data_index).map_err(|_| ())?)
                    .ok_or(())?;
                Ok(resolve(material.surface_property.as_deref()))
            })
            .collect::<Result<Vec<_>, ()>>()?;
        let mut model_properties = BTreeMap::new();
        let mut props = BTreeMap::new();
        for prop in map
            .static_props
            .occurrences
            .iter()
            .filter(|prop| prop.solidity != 0)
        {
            let model = &map
                .static_props
                .models
                .get(prop.model)
                .ok_or(())?
                .logical_path;
            let value = if let Some(value) = model_properties.get(model) {
                *value
            } else {
                let name = playsrc_studio_model::read_model_surface_property(
                    model,
                    resources.get(model).ok_or(())?,
                    Default::default(),
                )
                .map_err(|_| ())?;
                let value = resolve(Some(&name));
                model_properties.insert(model.clone(), value);
                value
            };
            props.insert(super::static_prop_collision_identity(prop.source)?, value);
        }
        Ok(Self {
            identity: registry.identity,
            values: registry
                .records
                .iter()
                .map(|record| f32::from_bits(record.audio_reflectivity_bits))
                .collect(),
            textures,
            default,
            props,
        })
    }
    fn reflectivity(
        &self,
        world: &playsrc_collision::World,
        trace: playsrc_collision::Trace,
    ) -> Result<f32, ()> {
        if let Some(surface) = trace.surface {
            if surface.registry != self.identity {
                return Err(());
            }
            return self.values.get(surface.index as usize).copied().ok_or(());
        }
        if let Some(playsrc_collision::Hit::Object {
            identity,
            role: playsrc_collision::ObjectRole::StaticProp,
            ..
        }) = trace.hit
        {
            return self.props.get(&identity).copied().ok_or(());
        }
        if let (Some(brush), Some(plane)) = (trace.brush, trace.plane) {
            let brush = world.brushes.get(brush).ok_or(())?;
            let side = world.sides[brush.first_side..brush.first_side + brush.side_count]
                .iter()
                .find(|side| world.planes.get(side.plane) == Some(&plane))
                .ok_or(())?;
            if side.texture_info >= 0 {
                return self
                    .textures
                    .get(side.texture_info as usize)
                    .copied()
                    .ok_or(());
            }
        }
        Ok(self.default)
    }
}

pub struct Scene {
    detector: Detector,
    materials: Arc<Materials>,
}
impl Scene {
    pub fn new(materials: Materials) -> Self {
        Self {
            detector: Detector::default(),
            materials: Arc::new(materials),
        }
    }
    pub fn update(
        &mut self,
        world: &super::SharedWorld,
        request: SceneRequest,
    ) -> Result<Vec<u8>, ()> {
        let snapshot = world.snapshot();
        let mut geometry = Queries {
            world,
            snapshot,
            materials: &self.materials,
            failed: false,
        };
        let mut detector = self.detector.clone();
        let room = detector.update(
            request.automatic,
            request.host_time,
            request.eyes,
            &mut geometry,
        );
        let mut obstruction = Vec::with_capacity(request.obstruction.len());
        for voice in request.obstruction {
            let value = obstruction::gain(request.eyes, voice, |start, end| {
                let Some(trace) =
                    geometry.ray(start, end, 0x4003, playsrc_collision::TraceScope::WorldOnly)
                else {
                    return obstruction::Hit {
                        hit: false,
                        fraction: 1.0,
                        start_solid: false,
                    };
                };
                obstruction::Hit {
                    hit: trace.did_hit(),
                    fraction: trace.fraction,
                    start_solid: trace.start_solid,
                }
            });
            obstruction.push((voice.voice, value));
        }
        if geometry.failed {
            return Err(());
        }
        let contents = world
            .world
            .point_contents_snapshot_value(&geometry.snapshot, request.eyes)
            .map_err(|_| ())?;
        let output = playsrc_audio::wire::reply_bytes(&SceneReply {
            world: request.world,
            sequence: request.sequence,
            underwater: contents & 0x30 != 0,
            room,
            obstruction,
        });
        self.detector = detector;
        Ok(output)
    }
}

struct Queries<'a> {
    world: &'a super::SharedWorld,
    snapshot: Arc<playsrc_collision::Snapshot>,
    materials: &'a Materials,
    failed: bool,
}
impl Queries<'_> {
    fn ray(
        &mut self,
        start: [f32; 3],
        end: [f32; 3],
        mask: u32,
        scope: playsrc_collision::TraceScope,
    ) -> Option<playsrc_collision::Trace> {
        let result = self.world.world.trace_snapshot_hull_with_scratch(
            &self.snapshot,
            playsrc_collision::SnapshotTraceRequest {
                start,
                end,
                mask,
                scope,
                hull: playsrc_collision::Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                ignored: &[],
            },
            &mut self
                .world
                .movement_queries
                .lock()
                .expect("audio scene queries"),
            |candidate| candidate.role == playsrc_collision::ObjectRole::StaticProp,
        );
        match result {
            Ok(trace) => Some(trace),
            Err(_) => {
                self.failed = true;
                None
            }
        }
    }
}
impl Geometry for Queries<'_> {
    fn trace(&mut self, start: [f32; 3], end: [f32; 3], kind: TraceKind) -> Hit {
        let (mask, scope) = match kind {
            TraceKind::WorldSolid => (1, playsrc_collision::TraceScope::WorldOnly),
            TraceKind::WorldAndStaticProps => {
                (0x4003, playsrc_collision::TraceScope::EverythingFilterProps)
            }
        };
        let Some(trace) = self.ray(start, end, mask, scope) else {
            return Hit {
                start,
                end,
                hit: false,
                sky: false,
                reflectivity: None,
            };
        };
        let reflectivity = match self.materials.reflectivity(&self.world.world, trace) {
            Ok(value) => Some(value),
            Err(()) => {
                self.failed = true;
                None
            }
        };
        Hit {
            start,
            end: trace.end,
            hit: trace.did_hit(),
            sky: trace.is_sky(),
            reflectivity,
        }
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// The input must be a readable module allocation of `length` bytes.
pub unsafe extern "C" fn playsrc_acoustic_update(
    handle: u32,
    pointer: *const u8,
    length: usize,
) -> u32 {
    if pointer.is_null() || length > 4096 {
        return 0;
    }
    let Some(request) =
        playsrc_audio::wire::read_request(unsafe { std::slice::from_raw_parts(pointer, length) })
    else {
        return 0;
    };
    let Some((index, generation)) = super::decode(handle) else {
        return 0;
    };
    let mut slots = super::slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation || slot.bsp_hash != request.world {
        return 0;
    }
    let (Some(scene), Some(world)) = (&mut slot.acoustic_scene, &slot.gameplay_world) else {
        return 0;
    };
    match scene.update(world, request) {
        Ok(bytes) => {
            slot.acoustic_output = bytes;
            1
        }
        Err(()) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_acoustic_output_length(handle: u32) -> usize {
    super::with(handle, |slot| slot.acoustic_output.len()).unwrap_or(0)
}
