use crate::{
    Error, ErrorCode, Feature, Hit, MASK_OPAQUE, ObjectRole, Snapshot, SnapshotRayRequest, Trace,
    TraceScope, World, error,
};
use std::collections::BTreeSet;

pub const LIGHTING_RAY_BATCH_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightingOccluders {
    World,
    WorldAndStaticProps,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightingRay {
    pub identity: u64,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub ignored_static_prop: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightingRayLimits {
    pub max_rays: usize,
    pub max_output_bytes: usize,
}

impl Default for LightingRayLimits {
    fn default() -> Self {
        Self {
            max_rays: 4_096,
            max_output_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightingRayResult {
    pub identity: u64,
    pub trace: Trace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightingRayBatch {
    pub sample_set_identity: u64,
    pub world: [u8; 32],
    pub snapshot: u64,
    pub occluders: LightingOccluders,
    pub rays: Vec<LightingRayResult>,
    bytes: Vec<u8>,
}

impl LightingRayBatch {
    pub fn comparison_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl World {
    pub fn trace_lighting_rays(
        &self,
        snapshot: &Snapshot,
        sample_set_identity: u64,
        occluders: LightingOccluders,
        rays: &[LightingRay],
        limits: LightingRayLimits,
        mut cancelled: impl FnMut(usize) -> bool,
    ) -> Result<LightingRayBatch, Error> {
        if limits.max_rays == 0 || limits.max_output_bytes == 0 || rays.len() > limits.max_rays {
            return Err(error(ErrorCode::Limit, None));
        }
        if snapshot.world_identity() != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let mut identities = BTreeSet::new();
        let mut output = Vec::with_capacity(rays.len());
        for (index, ray) in rays.iter().copied().enumerate() {
            if cancelled(index) {
                return Err(error(ErrorCode::Cancelled, Some(index)));
            }
            if !identities.insert(ray.identity) {
                return Err(error(ErrorCode::DuplicateIdentity, Some(index)));
            }
            let ignored = ray.ignored_static_prop.into_iter().collect::<Vec<_>>();
            let scope = match occluders {
                LightingOccluders::World => TraceScope::WorldOnly,
                LightingOccluders::WorldAndStaticProps => TraceScope::EverythingFilterProps,
            };
            let trace = self.trace_snapshot_ray(
                snapshot,
                SnapshotRayRequest {
                    start: ray.start,
                    end: ray.end,
                    mask: MASK_OPAQUE,
                    scope,
                    ignored: &ignored,
                },
                |candidate| candidate.role == ObjectRole::StaticProp,
            )?;
            output.push(LightingRayResult {
                identity: ray.identity,
                trace,
            });
        }
        let bytes = comparison_bytes(
            self.identity,
            snapshot.identity(),
            sample_set_identity,
            occluders,
            &output,
            limits.max_output_bytes,
        )?;
        Ok(LightingRayBatch {
            sample_set_identity,
            world: self.identity,
            snapshot: snapshot.identity(),
            occluders,
            rays: output,
            bytes,
        })
    }
}

fn comparison_bytes(
    world: [u8; 32],
    snapshot: u64,
    sample_set_identity: u64,
    occluders: LightingOccluders,
    rays: &[LightingRayResult],
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let mut bytes = BatchBytes::new(maximum);
    bytes.add(b"CLRB")?;
    bytes.u32(LIGHTING_RAY_BATCH_VERSION)?;
    bytes.add(&world)?;
    bytes.u64(snapshot)?;
    bytes.u64(sample_set_identity)?;
    bytes.u8(match occluders {
        LightingOccluders::World => 0,
        LightingOccluders::WorldAndStaticProps => 1,
    })?;
    bytes.u32(u32::try_from(rays.len()).map_err(|_| error(ErrorCode::Limit, None))?)?;
    for ray in rays {
        bytes.u64(ray.identity)?;
        bytes.f32(ray.trace.fraction)?;
        bytes.f32(ray.trace.fraction_left_solid)?;
        bytes.u8(u8::from(ray.trace.start_solid))?;
        bytes.u8(u8::from(ray.trace.all_solid))?;
        bytes.u32(ray.trace.contents)?;
        bytes.u16(ray.trace.surface_flags)?;
        for value in ray.trace.end {
            bytes.f32(value)?;
        }
        match ray.trace.plane {
            Some(plane) => {
                bytes.u8(1)?;
                for value in plane.normal {
                    bytes.f32(value)?;
                }
                bytes.f32(plane.distance)?;
                bytes.i32(plane.kind)?;
            }
            None => bytes.u8(0)?,
        }
        hit_bytes(
            &mut bytes,
            ray.trace.hit,
            ray.trace.displacement_flags,
            ray.trace.surface,
            ray.trace.displacement,
        )?;
    }
    Ok(bytes.finish())
}

fn hit_bytes(
    bytes: &mut BatchBytes,
    hit: Option<Hit>,
    displacement_flags: u16,
    surface: Option<crate::SurfaceIdentity>,
    displacement: Option<crate::DisplacementFeature>,
) -> Result<(), Error> {
    if let Some(displacement) = displacement {
        bytes.u8(3)?;
        bytes.usize(displacement.source)?;
        bytes.usize(displacement.parent_face)?;
        bytes.usize(displacement.triangle)?;
        bytes.u16(displacement_flags)?;
        let surface = surface.ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
        bytes.add(&surface.registry)?;
        return bytes.u32(surface.index);
    }
    match hit {
        None => bytes.u8(0),
        Some(Hit::WorldBrush { brush }) => {
            bytes.u8(1)?;
            bytes.usize(brush)
        }
        Some(Hit::Object {
            identity,
            role,
            feature,
        }) => {
            bytes.u8(2)?;
            bytes.u64(identity)?;
            bytes.u8(match role {
                ObjectRole::Entity => 0,
                ObjectRole::StaticProp => 1,
            })?;
            match feature {
                Feature::Brush { model, brush } => {
                    bytes.u8(0)?;
                    bytes.usize(model)?;
                    bytes.usize(brush)
                }
                Feature::Box => bytes.u8(1),
                Feature::Convex {
                    solid,
                    convex,
                    triangle,
                } => {
                    bytes.u8(2)?;
                    bytes.usize(solid)?;
                    bytes.usize(convex)?;
                    match triangle {
                        Some(value) => {
                            bytes.u8(1)?;
                            bytes.usize(value)
                        }
                        None => bytes.u8(0),
                    }
                }
            }
        }
    }
}

struct BatchBytes {
    bytes: Vec<u8>,
    maximum: usize,
}

impl BatchBytes {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn add(&mut self, value: &[u8]) -> Result<(), Error> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(error(ErrorCode::Limit, None));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), Error> {
        self.add(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), Error> {
        self.add(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.add(&value.to_le_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), Error> {
        self.add(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.add(&value.to_le_bytes())
    }

    fn usize(&mut self, value: usize) -> Result<(), Error> {
        self.u64(u64::try_from(value).map_err(|_| error(ErrorCode::Limit, None))?)
    }

    fn f32(&mut self, value: f32) -> Result<(), Error> {
        self.u32(value.to_bits())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hull, ObjectInput, SnapshotLimits, SnapshotShape, Transform};

    fn object(
        identity: u64,
        role: ObjectRole,
        enabled: bool,
        contents: u32,
        origin: [f32; 3],
        angles: [f32; 3],
    ) -> ObjectInput {
        ObjectInput {
            identity,
            role,
            enabled,
            transform: Transform { origin, angles },
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents,
            surface_flags: 0,
            shape: SnapshotShape::OrientedBox {
                bounds: Hull {
                    mins: [-1.0; 3],
                    maxs: [1.0; 3],
                },
            },
        }
    }

    #[test]
    fn batch_selects_only_nearest_eligible_static_props_and_honors_self_exclusion() {
        let world = World::empty();
        let snapshot = Snapshot::compile(
            &world,
            41,
            vec![
                object(
                    10,
                    ObjectRole::StaticProp,
                    true,
                    MASK_OPAQUE,
                    [0.0; 3],
                    [0.0; 3],
                ),
                object(
                    11,
                    ObjectRole::StaticProp,
                    true,
                    MASK_OPAQUE,
                    [4.0, 0.0, 0.0],
                    [0.0, 45.0, 0.0],
                ),
                object(
                    12,
                    ObjectRole::Entity,
                    true,
                    MASK_OPAQUE,
                    [-4.0, 0.0, 0.0],
                    [0.0; 3],
                ),
                object(
                    13,
                    ObjectRole::StaticProp,
                    false,
                    MASK_OPAQUE,
                    [-6.0, 0.0, 0.0],
                    [0.0; 3],
                ),
                object(
                    14,
                    ObjectRole::StaticProp,
                    true,
                    crate::CONTENTS_GRATE,
                    [-8.0, 0.0, 0.0],
                    [0.0; 3],
                ),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let rays = [
            LightingRay {
                identity: 1,
                start: [-10.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
                ignored_static_prop: None,
            },
            LightingRay {
                identity: 2,
                start: [0.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
                ignored_static_prop: Some(10),
            },
            LightingRay {
                identity: 3,
                start: [-10.0, 3.0, 0.0],
                end: [10.0, 3.0, 0.0],
                ignored_static_prop: None,
            },
        ];
        let first = world
            .trace_lighting_rays(
                &snapshot,
                0x4c49_4748_545f_3031,
                LightingOccluders::WorldAndStaticProps,
                &rays,
                LightingRayLimits::default(),
                |_| false,
            )
            .unwrap();
        assert!(matches!(
            first.rays[0].trace.hit,
            Some(Hit::Object {
                identity: 10,
                role: ObjectRole::StaticProp,
                ..
            })
        ));
        assert!(matches!(
            first.rays[1].trace.hit,
            Some(Hit::Object {
                identity: 11,
                role: ObjectRole::StaticProp,
                ..
            })
        ));
        assert!(!first.rays[2].trace.did_hit());
        assert_eq!(&first.comparison_bytes()[..8], b"CLRB\x01\0\0\0");
        for _ in 0..1_024 {
            assert_eq!(
                world
                    .trace_lighting_rays(
                        &snapshot,
                        0x4c49_4748_545f_3031,
                        LightingOccluders::WorldAndStaticProps,
                        &rays,
                        LightingRayLimits::default(),
                        |_| false,
                    )
                    .unwrap(),
                first
            );
        }
    }

    #[test]
    fn batch_preserves_ties_inside_state_and_atomic_failures() {
        let world = World::empty();
        let snapshot = Snapshot::compile(
            &world,
            42,
            vec![
                object(
                    20,
                    ObjectRole::StaticProp,
                    true,
                    MASK_OPAQUE,
                    [0.0; 3],
                    [0.0; 3],
                ),
                object(
                    21,
                    ObjectRole::StaticProp,
                    true,
                    MASK_OPAQUE,
                    [0.0; 3],
                    [0.0; 3],
                ),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let rays = [
            LightingRay {
                identity: 4,
                start: [-2.0, 1.0, 0.0],
                end: [2.0, 1.0, 0.0],
                ignored_static_prop: None,
            },
            LightingRay {
                identity: 5,
                start: [0.0; 3],
                end: [0.0; 3],
                ignored_static_prop: None,
            },
        ];
        let batch = world
            .trace_lighting_rays(
                &snapshot,
                0x4c49_4748_545f_3032,
                LightingOccluders::WorldAndStaticProps,
                &rays,
                LightingRayLimits::default(),
                |_| false,
            )
            .unwrap();
        assert!(matches!(
            batch.rays[0].trace.hit,
            Some(Hit::Object { identity: 20, .. })
        ));
        assert!(batch.rays[1].trace.start_solid && batch.rays[1].trace.all_solid);
        assert_eq!(
            world
                .trace_lighting_rays(
                    &snapshot,
                    0x4c49_4748_545f_3032,
                    LightingOccluders::WorldAndStaticProps,
                    &rays,
                    LightingRayLimits::default(),
                    |index| index == 1,
                )
                .unwrap_err()
                .code,
            ErrorCode::Cancelled
        );
        assert_eq!(
            world
                .trace_lighting_rays(
                    &snapshot,
                    0x4c49_4748_545f_3032,
                    LightingOccluders::WorldAndStaticProps,
                    &rays,
                    LightingRayLimits {
                        max_output_bytes: 8,
                        ..LightingRayLimits::default()
                    },
                    |_| false,
                )
                .unwrap_err()
                .code,
            ErrorCode::Limit
        );
    }
}
