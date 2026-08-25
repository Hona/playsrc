const HEADER_BYTES: usize = 48;
const PHYSICS_RESULT_BYTES: usize = 80;
const MAX_RESULTS: usize = 64;

pub struct AdvanceInput {
    pub command: playsrc_tf2::Command,
    pub physics_results: Vec<playsrc_tf2::ProjectilePhysicsResult>,
}

pub fn decode(bytes: &[u8]) -> Option<AdvanceInput> {
    if bytes.len() < HEADER_BYTES
        || bytes.len() > 64 * 1024
        || &bytes[..4] != b"PCMD"
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) != 5
    {
        return None;
    }
    let physics_count = usize::from(u16::from_le_bytes(bytes[40..42].try_into().ok()?));
    if physics_count > MAX_RESULTS
        || u16::from_le_bytes(bytes[42..44].try_into().ok()?) != 0
        || u32::from_le_bytes(bytes[44..48].try_into().ok()?) as usize != bytes.len()
    {
        return None;
    }
    let expected = HEADER_BYTES.checked_add(physics_count.checked_mul(PHYSICS_RESULT_BYTES)?)?;
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
    let select_class = match (select & 0xff) as u8 {
        0 => None,
        value => Some(playsrc_tf2::PlayerClass::try_from(value).ok()?),
    };
    let select_weapon = match (select >> 8) & 0xff {
        0 => None,
        1 => Some(playsrc_tf2::Weapon::RocketLauncher),
        2 => Some(playsrc_tf2::Weapon::Original),
        3 => Some(playsrc_tf2::Weapon::StickybombLauncher),
        4 => Some(playsrc_tf2::Weapon::Minigun),
        5 => Some(playsrc_tf2::Weapon::Shotgun),
        6 => Some(playsrc_tf2::Weapon::Fists),
        _ => return None,
    };
    let select_team = match (select >> 16) & 0xff {
        0 => None,
        2 => Some(playsrc_tf2::PlayerTeam::Red),
        3 => Some(playsrc_tf2::PlayerTeam::Blue),
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

    (reader.offset == bytes.len()).then_some(AdvanceInput {
        command,
        physics_results,
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
