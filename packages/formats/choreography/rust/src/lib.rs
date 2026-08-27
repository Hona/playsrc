//! Bounded compiled Source choreography reader. Scene image and BVCD v4 fields
//! follow the public Source SDK SceneImageFile and choreography serializers.

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub kind: u8,
    pub name: String,
    pub start: f32,
    pub end: f32,
    pub parameters: [String; 3],
    pub ramp: Vec<(f32, f32)>,
    pub active: bool,
    pub actor: bool,
    pub loops: i8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub events: Vec<Event>,
    pub ramp: Vec<(f32, f32)>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    strings: &'a [String],
}
impl Reader<'_> {
    fn take(&mut self, length: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(length).ok_or("scene offset overflow")?;
        let value = self.bytes.get(self.at..end).ok_or("truncated scene")?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn short(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn float(&mut self) -> Result<f32, String> {
        let value = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        if !value.is_finite() {
            return Err("non-finite scene time".into());
        }
        Ok(value)
    }
    fn string(&mut self) -> Result<String, String> {
        let index = self.short()? as usize;
        self.strings
            .get(index)
            .cloned()
            .ok_or("invalid scene string".into())
    }
    fn curve(&mut self) -> Result<Vec<(f32, f32)>, String> {
        (0..self.byte()?)
            .map(|_| Ok((self.float()?, self.byte()? as f32 / 255.0)))
            .collect()
    }
    fn event(&mut self, actor: bool) -> Result<Event, String> {
        let kind = self.byte()?;
        let name = self.string()?;
        let start = self.float()?;
        let end = self.float()?;
        let parameters = [self.string()?, self.string()?, self.string()?];
        let ramp = self.curve()?;
        let flags = self.byte()?;
        self.float()?; // distance to movement target
        for tag_type in 0..4 {
            for _ in 0..self.byte()? {
                self.string()?;
                if tag_type < 2 {
                    self.byte()?;
                } else {
                    self.short()?;
                }
            }
        }
        if kind == 6 {
            self.float()?;
        }
        if self.byte()? == 1 {
            self.string()?;
            self.string()?;
        }
        let flex_tracks = self.byte()?;
        // Do not silently discard authored flex tracks. TF2 class-select scenes
        // use expression events; callers must explicitly support other scenes.
        if flex_tracks != 0 {
            return Err("scene flex-animation tracks require a track consumer".into());
        }
        let loops = if kind == 12 { self.byte()? as i8 } else { 0 };
        if kind == 5 {
            self.byte()?;
            self.string()?;
            self.byte()?;
        }
        Ok(Event {
            kind,
            name,
            start,
            end,
            parameters,
            ramp,
            active: flags & 8 != 0,
            actor,
            loops,
        })
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset.checked_add(4).ok_or("scene offset overflow")?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or("truncated scene image")?
            .try_into()
            .unwrap(),
    ))
}

/// Resolve a named compiled expression into its named global flex controllers.
pub fn read_expression(bytes: &[u8], name: &str) -> Result<Vec<(String, f32, f32)>, String> {
    if bytes.len() < 108
        || bytes.len() > 8 * 1024 * 1024
        || word(bytes, 72)? as usize != bytes.len()
    {
        return Err("invalid flex expression length".into());
    }
    let string = |offset: usize| -> Result<String, String> {
        let tail = bytes
            .get(offset..)
            .ok_or("invalid flex expression string")?;
        let length = tail
            .iter()
            .position(|v| *v == 0)
            .ok_or("unterminated flex expression string")?;
        if length > 4095 {
            return Err("flex expression string limit".into());
        }
        String::from_utf8(tail[..length].to_vec())
            .map_err(|_| "invalid flex expression encoding".into())
    };
    let count = word(bytes, 76)? as usize;
    let table = word(bytes, 80)? as usize;
    let keys = word(bytes, 96)? as usize;
    let names = word(bytes, 100)? as usize;
    if count > 4096 || keys > 1024 || table > bytes.len() || names > bytes.len() {
        return Err("flex expression count limit".into());
    }
    for index in 0..count {
        let at = table + index * 24;
        if !string(
            at.checked_add(word(bytes, at)? as usize)
                .ok_or("expression offset overflow")?,
        )?
        .eq_ignore_ascii_case(name)
        {
            continue;
        }
        let count = word(bytes, at + 8)? as usize;
        let start = at
            .checked_add(word(bytes, at + 20)? as usize)
            .ok_or("expression offset overflow")?;
        if count > 1024 || start > bytes.len() {
            return Err("flex expression weight limit".into());
        }
        return (0..count)
            .map(|index| {
                let at = start + index * 12;
                let key = word(bytes, at)? as usize;
                if key >= keys {
                    return Err("flex expression controller index".into());
                }
                let weight = f32::from_bits(word(bytes, at + 4)?);
                let influence = f32::from_bits(word(bytes, at + 8)?);
                if !weight.is_finite() || !influence.is_finite() {
                    return Err("non-finite flex expression weight".into());
                }
                Ok((
                    string(word(bytes, names + key * 4)? as usize)?,
                    weight,
                    influence,
                ))
            })
            .collect();
    }
    // Named settings not found in an existing expression file are ignored by TF2.
    Ok(Vec::new())
}

pub fn filename_crc(path: &str) -> u32 {
    let mut crc = u32::MAX;
    for byte in path.bytes().map(|v| {
        if v == b'/' {
            b'\\'
        } else {
            v.to_ascii_lowercase()
        }
    }) {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

pub fn read_scene_image(bytes: &[u8], path: &str) -> Result<Scene, String> {
    if bytes.len() > 64 * 1024 * 1024 || bytes.get(..4) != Some(b"VSIF") || word(bytes, 4)? != 2 {
        return Err("invalid scene image".into());
    }
    let scene_count = word(bytes, 8)? as usize;
    let string_count = word(bytes, 12)? as usize;
    let entry_offset = word(bytes, 16)? as usize;
    if scene_count > 65536 || string_count > 32768 || entry_offset > bytes.len() {
        return Err("scene image count limit".into());
    }
    let strings = (0..string_count)
        .map(|index| {
            let offset = word(bytes, 20 + index * 4)? as usize;
            let tail = bytes.get(offset..).ok_or("invalid scene string offset")?;
            let length = tail
                .iter()
                .position(|v| *v == 0)
                .ok_or("unterminated scene string")?;
            if length > 4095 {
                return Err("scene string limit".into());
            }
            String::from_utf8(tail[..length].to_vec())
                .map_err(|_| "invalid scene string encoding".into())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let crc = filename_crc(path);
    let mut selected = None;
    for index in 0..scene_count {
        let offset = entry_offset + index * 16;
        if word(bytes, offset)? == crc {
            let start = word(bytes, offset + 4)? as usize;
            let length = word(bytes, offset + 8)? as usize;
            selected = Some(
                bytes
                    .get(start..start.checked_add(length).ok_or("scene size overflow")?)
                    .ok_or("invalid scene range")?,
            );
            break;
        }
    }
    let bytes = selected.ok_or_else(|| format!("scene image has no entry for {path}"))?;
    let mut reader = Reader {
        bytes,
        at: 0,
        strings: &strings,
    };
    if reader.take(4)? != b"bvcd" || reader.byte()? != 4 {
        return Err("unsupported compiled scene version".into());
    }
    reader.take(4)?; // source CRC
    let mut events = Vec::new();
    for _ in 0..reader.byte()? {
        events.push(reader.event(false)?);
    }
    for _ in 0..reader.byte()? {
        reader.string()?;
        let first_actor_event = events.len();
        for _ in 0..reader.byte()? {
            reader.string()?;
            let first_channel_event = events.len();
            for _ in 0..reader.byte()? {
                events.push(reader.event(true)?);
            }
            if reader.byte()? == 0 {
                for event in &mut events[first_channel_event..] {
                    event.active = false;
                }
            }
        }
        if reader.byte()? == 0 {
            for event in &mut events[first_actor_event..] {
                event.active = false;
            }
        }
    }
    let ramp = reader.curve()?;
    reader.byte()?; // ignore phonemes
    if reader.at != bytes.len() {
        return Err("trailing compiled scene bytes".into());
    }
    Ok(Scene { events, ramp })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn class_scene_filenames_use_normalized_crc() {
        assert_eq!(
            filename_crc("scenes/player/scout/low/class_select.vcd"),
            2375828162
        );
        assert_eq!(
            filename_crc("SCENES\\PLAYER\\SCOUT\\LOW\\CLASS_SELECT.VCD"),
            2375828162
        );
    }
    #[test]
    fn truncated_images_are_rejected() {
        for length in 0..20 {
            assert!(read_scene_image(&vec![0; length], "scenes/a.vcd").is_err());
        }
    }
}
