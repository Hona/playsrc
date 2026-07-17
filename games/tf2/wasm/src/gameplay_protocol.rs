const HEADER_BYTES: usize = 56;
const STICKY_RANDOM_BYTES: usize = 12;
const PHYSICS_RESULT_BYTES: usize = 80;
const ROCKET_RESULT_BYTES: usize = 44;
const MOVER_RESULT_BYTES: usize = 52;
const MAX_RESULTS: usize = 64;

pub struct AdvanceInput {
    pub command: playsrc_tf2::Command,
    pub sticky_random: Option<playsrc_tf2::StickyLaunchRandom>,
    pub physics_results: Vec<playsrc_tf2::ProjectilePhysicsResult>,
    pub rocket_results: Vec<playsrc_tf2::RocketTraceResult>,
    pub mover_results: Vec<playsrc_tf2::MoverResult>,
}

pub fn decode(bytes: &[u8]) -> Option<AdvanceInput> {
    if bytes.len() < HEADER_BYTES
        || bytes.len() > 64 * 1024
        || &bytes[..4] != b"PCMD"
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) != 3
    {
        return None;
    }
    let physics_count = usize::from(u16::from_le_bytes(bytes[40..42].try_into().ok()?));
    let rocket_count = usize::from(u16::from_le_bytes(bytes[42..44].try_into().ok()?));
    let mover_count = usize::from(u16::from_le_bytes(bytes[44..46].try_into().ok()?));
    let external_flags = u16::from_le_bytes(bytes[46..48].try_into().ok()?);
    if physics_count > MAX_RESULTS
        || rocket_count > MAX_RESULTS
        || mover_count > MAX_RESULTS
        || external_flags & !1 != 0
        || u32::from_le_bytes(bytes[48..52].try_into().ok()?) as usize != bytes.len()
        || u32::from_le_bytes(bytes[52..56].try_into().ok()?) != 0
    {
        return None;
    }
    let expected = HEADER_BYTES
        .checked_add(if external_flags & 1 != 0 {
            STICKY_RANDOM_BYTES
        } else {
            0
        })?
        .checked_add(physics_count.checked_mul(PHYSICS_RESULT_BYTES)?)?
        .checked_add(rocket_count.checked_mul(ROCKET_RESULT_BYTES)?)?
        .checked_add(mover_count.checked_mul(MOVER_RESULT_BYTES)?)?;
    if expected != bytes.len() {
        return None;
    }

    let f = |offset| -> Option<f32> {
        Some(f32::from_le_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let flags = u32::from_le_bytes(bytes[28..32].try_into().ok()?);
    let select = u32::from_le_bytes(bytes[32..36].try_into().ok()?);
    if flags & !0xff != 0 {
        return None;
    }
    let select_class = match select & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Class::Soldier),
        2 => Some(playsrc_tf2::Class::Demoman),
        _ => return None,
    };
    let select_weapon = match (select >> 8) & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Weapon::RocketLauncher),
        2 => Some(playsrc_tf2::Weapon::Original),
        3 => Some(playsrc_tf2::Weapon::StickybombLauncher),
        _ => return None,
    };
    let select_team = match (select >> 16) & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Team::Red),
        2 => Some(playsrc_tf2::Team::Blue),
        _ => return None,
    };
    let mode_request = match (select >> 24) & 0xff {
        0 => None,
        1 => Some(playsrc_movement::Mode::Walk),
        2 => Some(playsrc_movement::Mode::Noclip),
        _ => return None,
    };
    let target = u32::from_le_bytes(bytes[36..40].try_into().ok()?);
    let command = playsrc_tf2::Command {
        movement: playsrc_movement::Command {
            forward: f(8)?,
            side: f(12)?,
            yaw_degrees: f(20)?,
            jump: flags & 1 != 0,
            crouch: flags & 2 != 0,
        },
        pitch_degrees: f(24)?,
        up: f(16)?,
        speed_button: flags & 4 != 0,
        fire: flags & 8 != 0,
        detonate: flags & 16 != 0,
        reload: flags & 32 != 0,
        reset: flags & 64 != 0,
        respawn: flags & 128 != 0,
        select_class,
        select_team,
        select_weapon,
        mode_request,
        activate_entity: (target != u32::MAX).then_some(target),
    };
    if [
        command.movement.forward,
        command.movement.side,
        command.up,
        command.movement.yaw_degrees,
        command.pitch_degrees,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
    {
        return None;
    }

    let mut reader = Reader {
        bytes,
        offset: HEADER_BYTES,
    };
    let sticky_random = if external_flags & 1 != 0 {
        Some(playsrc_tf2::StickyLaunchRandom {
            right_velocity: reader.f32()?,
            up_velocity: reader.f32()?,
            angular_y: reader.i32()?,
        })
    } else {
        None
    };
    if sticky_random.is_some_and(|value| !value.validate()) {
        return None;
    }

    let mut physics_results = Vec::with_capacity(physics_count);
    for _ in 0..physics_count {
        let projectile = reader.u32()?;
        let tick = reader.u64()?;
        let motion_enabled = reader.u8()?;
        let contact_kind = reader.u8()?;
        if motion_enabled > 1 || contact_kind > 3 || reader.u16()? != 0 {
            return None;
        }
        let position = reader.vector()?;
        let velocity = reader.vector()?;
        let orientation = reader.quaternion()?;
        let angular_velocity = reader.vector()?;
        let normal = reader.vector()?;
        let contact = match contact_kind {
            0 if normal == [0.0; 3] => None,
            1 => Some(playsrc_tf2::ProjectileContact {
                kind: playsrc_tf2::ProjectileContactKind::World,
                normal,
            }),
            2 => Some(playsrc_tf2::ProjectileContact {
                kind: playsrc_tf2::ProjectileContactKind::DynamicProp,
                normal,
            }),
            3 => Some(playsrc_tf2::ProjectileContact {
                kind: playsrc_tf2::ProjectileContactKind::Other,
                normal,
            }),
            _ => return None,
        };
        physics_results.push(playsrc_tf2::ProjectilePhysicsResult {
            projectile,
            tick,
            position,
            velocity,
            orientation,
            angular_velocity,
            motion_enabled: motion_enabled != 0,
            contact,
        });
    }

    let mut rocket_results = Vec::with_capacity(rocket_count);
    for _ in 0..rocket_count {
        let projectile = reader.u32()?;
        let tick = reader.u64()?;
        let solid = reader.u8()?;
        let sky = reader.u8()?;
        let has_normal = reader.u8()?;
        let has_target = reader.u8()?;
        if solid > 1 || sky > 1 || has_normal > 1 || has_target > 1 {
            return None;
        }
        let end = reader.vector()?;
        let raw_normal = reader.vector()?;
        let raw_target = reader.u32()?;
        if (has_normal == 0 && raw_normal != [0.0; 3])
            || (has_target == 0 && raw_target != u32::MAX)
        {
            return None;
        }
        rocket_results.push(playsrc_tf2::RocketTraceResult {
            projectile,
            tick,
            end,
            solid: solid != 0,
            sky: sky != 0,
            normal: (has_normal != 0).then_some(raw_normal),
            direct_target: (has_target != 0).then_some(raw_target),
        });
    }

    let mut mover_results = Vec::with_capacity(mover_count);
    for _ in 0..mover_count {
        let request_id = reader.u64()?;
        let entity = reader.u32()?;
        let kind = match reader.u8()? {
            1 => playsrc_tf2::MoverResultKind::Progress,
            2 => playsrc_tf2::MoverResultKind::Completed,
            3 => playsrc_tf2::MoverResultKind::BlockedStart,
            4 => playsrc_tf2::MoverResultKind::BlockedStay,
            5 => playsrc_tf2::MoverResultKind::BlockedEnd,
            _ => return None,
        };
        if reader.take::<3>()? != [0, 0, 0] {
            return None;
        }
        mover_results.push(playsrc_tf2::MoverResult {
            request_id,
            entity,
            kind,
            transform: playsrc_entity::Transform {
                origin: reader.vector()?,
                angles: reader.vector()?,
            },
            carry: reader.vector()?,
        });
    }
    (reader.offset == bytes.len()).then_some(AdvanceInput {
        command,
        sticky_random,
        physics_results,
        rocket_results,
        mover_results,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.offset.checked_add(N)?;
        let value = self.bytes.get(self.offset..end)?.try_into().ok()?;
        self.offset = end;
        Some(value)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take()?))
    }

    fn f32(&mut self) -> Option<f32> {
        let value = f32::from_le_bytes(self.take()?);
        value.is_finite().then_some(value)
    }

    fn vector(&mut self) -> Option<[f32; 3]> {
        Some([self.f32()?, self.f32()?, self.f32()?])
    }

    fn quaternion(&mut self) -> Option<[f32; 4]> {
        Some([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }
}
