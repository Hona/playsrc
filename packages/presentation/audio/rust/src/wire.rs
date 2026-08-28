//! Bounded in-process audio/geometry transactions. No network transport.
use crate::{acoustics::RoomChange, playback::ObstructionRequest, room::Room};

pub struct SceneRequest {
    pub world: [u8; 32],
    pub sequence: u64,
    pub eyes: [f32; 3],
    pub host_time: f64,
    pub automatic: bool,
    pub obstruction: Vec<ObstructionRequest>,
}
pub struct SceneReply {
    pub world: [u8; 32],
    pub sequence: u64,
    pub underwater: bool,
    pub room: Option<RoomChange>,
    pub obstruction: Vec<(u32, f32)>,
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub fn bytes(&mut self, size: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(size)?;
        let bytes = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(bytes)
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }
    pub fn i32(&mut self) -> Option<i32> {
        Some(self.u32()? as i32)
    }
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
    }
    pub fn f32(&mut self) -> Option<f32> {
        let value = f32::from_bits(self.u32()?);
        value.is_finite().then_some(value)
    }
    pub fn f64(&mut self) -> Option<f64> {
        let value = f64::from_bits(self.u64()?);
        value.is_finite().then_some(value)
    }
    pub fn vector(&mut self) -> Option<[f32; 3]> {
        Some([self.f32()?, self.f32()?, self.f32()?])
    }
    pub fn flag(&mut self) -> Option<bool> {
        match self.u32()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    pub fn done(&self) -> bool {
        self.at == self.bytes.len()
    }
}
fn integer(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn float(out: &mut Vec<u8>, value: f32) {
    integer(out, value.to_bits());
}

pub fn request_bytes(request: &SceneRequest) -> Vec<u8> {
    let mut out = b"PSAQ\x01\0\0\0".to_vec();
    out.extend_from_slice(&request.world);
    out.extend_from_slice(&request.sequence.to_le_bytes());
    for value in request.eyes {
        float(&mut out, value);
    }
    out.extend_from_slice(&request.host_time.to_le_bytes());
    integer(&mut out, u32::from(request.automatic));
    integer(&mut out, request.obstruction.len() as u32);
    for voice in &request.obstruction {
        integer(&mut out, voice.voice);
        for value in voice.origin {
            float(&mut out, value);
        }
        integer(&mut out, voice.level as u32);
        float(&mut out, voice.radius);
    }
    out
}
pub fn read_request(bytes: &[u8]) -> Option<SceneRequest> {
    if bytes.len() > 4096 {
        return None;
    }
    let mut read = Reader::new(bytes);
    if read.bytes(8)? != b"PSAQ\x01\0\0\0" {
        return None;
    }
    let world = read.bytes(32)?.try_into().ok()?;
    let sequence = read.u64()?;
    let eyes = read.vector()?;
    let host_time = read.f64()?;
    let automatic = read.flag()?;
    let count = read.u32()?;
    if count > 128 {
        return None;
    }
    let mut obstruction = Vec::<ObstructionRequest>::with_capacity(count as usize);
    for _ in 0..count {
        let voice = read.u32()?;
        let origin = read.vector()?;
        let level = read.i32()?;
        let radius = read.f32()?;
        if voice == 0
            || voice > 128
            || radius < 0.0
            || obstruction.iter().any(|prior| prior.voice == voice)
        {
            return None;
        }
        obstruction.push(ObstructionRequest {
            voice,
            origin,
            level,
            radius,
        });
    }
    read.done().then_some(SceneRequest {
        world,
        sequence,
        eyes,
        host_time,
        automatic,
        obstruction,
    })
}
pub fn reply_bytes(reply: &SceneReply) -> Vec<u8> {
    let mut out = b"PSAR\x01\0\0\0".to_vec();
    out.extend_from_slice(&reply.world);
    out.extend_from_slice(&reply.sequence.to_le_bytes());
    integer(&mut out, u32::from(reply.underwater));
    integer(&mut out, u32::from(reply.room.is_some()));
    if let Some(change) = reply.room {
        integer(&mut out, change.node as u32);
        integer(&mut out, u32::from(change.created.is_some()));
        if let Some(room) = change.created {
            integer(&mut out, u32::from(room.outside));
            for value in [room.width, room.length, room.height] {
                integer(&mut out, value as u32);
            }
            for value in [room.diffusion, room.reflectivity]
                .into_iter()
                .chain(room.surfaces)
            {
                float(&mut out, value);
            }
        }
    }
    integer(&mut out, reply.obstruction.len() as u32);
    for (identity, gain) in &reply.obstruction {
        integer(&mut out, *identity);
        float(&mut out, *gain);
    }
    out
}
pub fn read_reply(bytes: &[u8]) -> Option<SceneReply> {
    if bytes.len() > 2048 {
        return None;
    }
    let mut read = Reader::new(bytes);
    if read.bytes(8)? != b"PSAR\x01\0\0\0" {
        return None;
    }
    let world = read.bytes(32)?.try_into().ok()?;
    let sequence = read.u64()?;
    let underwater = read.flag()?;
    let room = if read.flag()? {
        let node = read.u32()? as usize;
        if node >= 40 {
            return None;
        }
        let created = if read.flag()? {
            let outside = read.flag()?;
            let width = read.i32()?;
            let length = read.i32()?;
            let height = read.i32()?;
            if width < 0 || length < 0 || height < 0 {
                return None;
            }
            let diffusion = read.f32()?;
            let reflectivity = read.f32()?;
            let surfaces = [
                read.f32()?,
                read.f32()?,
                read.f32()?,
                read.f32()?,
                read.f32()?,
                read.f32()?,
            ];
            Some(Room {
                outside,
                width,
                length,
                height,
                diffusion,
                reflectivity,
                surfaces,
            })
        } else {
            None
        };
        Some(RoomChange { node, created })
    } else {
        None
    };
    let count = read.u32()?;
    if count > 128 {
        return None;
    }
    let mut obstruction = Vec::with_capacity(count as usize);
    for _ in 0..count {
        obstruction.push((read.u32()?, read.f32()?));
    }
    read.done().then_some(SceneReply {
        world,
        sequence,
        underwater,
        room,
        obstruction,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transactions_bound_world_sequence_geometry_and_all_room_fields() {
        let request = SceneRequest {
            world: [7; 32],
            sequence: 91,
            eyes: [1.0, 2.0, 3.0],
            host_time: 4.5,
            automatic: true,
            obstruction: vec![ObstructionRequest {
                voice: 3,
                origin: [4.0, 5.0, 6.0],
                level: 70,
                radius: 0.0,
            }],
        };
        let bytes = request_bytes(&request);
        assert_eq!(read_request(&bytes).unwrap().sequence, 91);
        for end in 0..bytes.len() {
            assert!(read_request(&bytes[..end]).is_none());
        }
        let reply = SceneReply {
            world: [7; 32],
            sequence: 91,
            underwater: false,
            room: Some(RoomChange {
                node: 39,
                created: Some(Room {
                    outside: true,
                    width: 64,
                    length: 128,
                    height: 4800,
                    diffusion: 0.0,
                    reflectivity: 0.5,
                    surfaces: [0.25; 6],
                }),
            }),
            obstruction: vec![(3, 0.5)],
        };
        let bytes = reply_bytes(&reply);
        assert_eq!(read_reply(&bytes).unwrap().room, reply.room);
        for end in 0..bytes.len() {
            assert!(read_reply(&bytes[..end]).is_none());
        }
    }
}
