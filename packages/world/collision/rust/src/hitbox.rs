use crate::{CONTENTS_HITBOX, Hull, SURF_HITBOX, SurfaceIdentity};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioHitbox<'a> {
    pub identity: usize,
    pub group: i32,
    pub bone: usize,
    pub physics_bone: i32,
    pub bone_contents: u32,
    pub surface: Option<SurfaceIdentity>,
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
    pub bone_to_world: &'a [f32; 12],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioHitboxRequest<'a> {
    pub entity: u64,
    pub origin: [f32; 3],
    pub scale: f32,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
    pub hitboxes: &'a [StudioHitbox<'a>],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioHitboxTrace {
    pub entity: u64,
    pub hitbox: usize,
    pub hitgroup: i32,
    pub physics_bone: i32,
    pub contents: u32,
    pub surface: Option<SurfaceIdentity>,
    pub surface_flags: u16,
    pub fraction: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub end: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StudioHitboxError {
    NonFinite,
    InvalidScale,
    InvalidBounds,
    InvalidBone,
    Limit,
}

pub fn trace_studio_hitboxes(
    request: StudioHitboxRequest<'_>,
) -> Result<Option<StudioHitboxTrace>, StudioHitboxError> {
    if request.hitboxes.len() > 1024 {
        return Err(StudioHitboxError::Limit);
    }
    if !request.scale.is_finite() || request.scale <= 0.0 {
        return Err(StudioHitboxError::InvalidScale);
    }
    if request
        .start
        .into_iter()
        .chain(request.end)
        .chain(request.origin)
        .chain(request.hull.mins)
        .chain(request.hull.maxs)
        .any(|value| !value.is_finite())
    {
        return Err(StudioHitboxError::NonFinite);
    }
    if request
        .hull
        .mins
        .into_iter()
        .zip(request.hull.maxs)
        .any(|(minimum, maximum)| minimum > maximum)
    {
        return Err(StudioHitboxError::InvalidBounds);
    }
    let swept = request.hull.mins != [0.0; 3] || request.hull.maxs != [0.0; 3];
    let mut best: Option<StudioHitboxTrace> = None;
    for hitbox in request.hitboxes {
        if hitbox.bone_contents & request.mask == 0 {
            continue;
        }
        if hitbox.physics_bone < 0 {
            return Err(StudioHitboxError::InvalidBone);
        }
        if hitbox
            .minimum
            .into_iter()
            .chain(hitbox.maximum)
            .chain(hitbox.bone_to_world.iter().copied())
            .any(|value| !value.is_finite())
        {
            return Err(StudioHitboxError::NonFinite);
        }
        if hitbox
            .minimum
            .into_iter()
            .zip(hitbox.maximum)
            .any(|(minimum, maximum)| minimum > maximum)
        {
            return Err(StudioHitboxError::InvalidBounds);
        }
        let scale = if swept { 1.0 } else { request.scale };
        let hull_center: [f32; 3] =
            std::array::from_fn(|axis| (request.hull.mins[axis] + request.hull.maxs[axis]) * 0.5);
        let project = |point: [f32; 3]| {
            std::array::from_fn(|axis| {
                (point[axis] + hull_center[axis] - request.origin[axis]) / scale
                    + request.origin[axis]
            })
        };
        let start = project(request.start);
        let end = project(request.end);
        let translation: [f32; 3] = std::array::from_fn(|axis| {
            (hitbox.bone_to_world[axis * 4 + 3] - request.origin[axis]) / scale
                + request.origin[axis]
        });
        let inverse = |point: [f32; 3]| {
            let offset = [
                point[0] - translation[0],
                point[1] - translation[1],
                point[2] - translation[2],
            ];
            [
                offset[0] * hitbox.bone_to_world[0]
                    + offset[1] * hitbox.bone_to_world[4]
                    + offset[2] * hitbox.bone_to_world[8],
                offset[0] * hitbox.bone_to_world[1]
                    + offset[1] * hitbox.bone_to_world[5]
                    + offset[2] * hitbox.bone_to_world[9],
                offset[0] * hitbox.bone_to_world[2]
                    + offset[1] * hitbox.bone_to_world[6]
                    + offset[2] * hitbox.bone_to_world[10],
            ]
        };
        let local_start = inverse(start);
        let local_end = inverse(end);
        let extents = std::array::from_fn::<_, 3, _>(|axis| {
            (0..3)
                .map(|index| {
                    let half = (request.hull.maxs[index] - request.hull.mins[index]) * 0.5;
                    hitbox.bone_to_world[index * 4 + axis].abs() * half
                })
                .sum::<f32>()
        });
        let mut enter = 0.0_f32;
        let mut leave = 1.0_f32;
        let mut side = None;
        let mut inside = true;
        for axis in 0..3 {
            let minimum = hitbox.minimum[axis] - extents[axis];
            let maximum = hitbox.maximum[axis] + extents[axis];
            let delta = local_end[axis] - local_start[axis];
            if local_start[axis] < minimum || local_start[axis] > maximum {
                inside = false;
            }
            if delta == 0.0 {
                if local_start[axis] < minimum || local_start[axis] > maximum {
                    enter = 2.0;
                    break;
                }
                continue;
            }
            let first = (minimum - local_start[axis]) / delta;
            let second = (maximum - local_start[axis]) / delta;
            let (near, far, sign) = if first <= second {
                (first, second, -1.0)
            } else {
                (second, first, 1.0)
            };
            if near > enter {
                enter = near;
                side = Some((axis, sign));
            }
            leave = leave.min(far);
            if enter > leave {
                break;
            }
        }
        if enter > leave || enter > 1.0 || leave < 0.0 {
            continue;
        }
        let fraction = if inside { 0.0 } else { enter.max(0.0) };
        if best.as_ref().is_some_and(|prior| fraction > prior.fraction) {
            continue;
        }
        let normal = side.map_or([0.0; 3], |(axis, sign)| {
            [
                hitbox.bone_to_world[axis] * sign,
                hitbox.bone_to_world[4 + axis] * sign,
                hitbox.bone_to_world[8 + axis] * sign,
            ]
        });
        best = Some(StudioHitboxTrace {
            entity: request.entity,
            hitbox: hitbox.identity,
            hitgroup: hitbox.group,
            physics_bone: hitbox.physics_bone,
            contents: hitbox.bone_contents | CONTENTS_HITBOX,
            surface: hitbox.surface,
            surface_flags: SURF_HITBOX,
            fraction,
            start_solid: inside,
            all_solid: inside && leave >= 1.0,
            end: std::array::from_fn(|axis| {
                request.start[axis] + (request.end[axis] - request.start[axis]) * fraction
            }),
            normal,
        });
        if inside && swept {
            break;
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATRIX: [f32; 12] = [1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 70.0];

    #[test]
    fn returns_ordered_studio_hitbox_group_bone_contents_and_surface() {
        let hitbox = StudioHitbox {
            identity: 4,
            group: 1,
            bone: 2,
            physics_bone: 3,
            bone_contents: 0x0200_0000,
            surface: None,
            minimum: [-3.0; 3],
            maximum: [3.0; 3],
            bone_to_world: &MATRIX,
        };
        let request = StudioHitboxRequest {
            entity: 9,
            origin: [0.0; 3],
            scale: 1.0,
            start: [0.0, 0.0, 70.0],
            end: [20.0, 0.0, 70.0],
            hull: Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            },
            mask: 0x0200_0000,
            hitboxes: &[hitbox],
        };
        let trace = trace_studio_hitboxes(request).unwrap().unwrap();
        assert_eq!(
            (
                trace.entity,
                trace.hitbox,
                trace.hitgroup,
                trace.physics_bone
            ),
            (9, 4, 1, 3)
        );
        assert_eq!(trace.end, [7.0, 0.0, 70.0]);
        assert_eq!(trace.normal, [-1.0, 0.0, 0.0]);
        assert_eq!(trace.contents, 0x4200_0000);
        assert_eq!(trace.surface_flags, SURF_HITBOX);
        assert!(
            trace_studio_hitboxes(StudioHitboxRequest { mask: 1, ..request })
                .unwrap()
                .is_none()
        );
    }
}
