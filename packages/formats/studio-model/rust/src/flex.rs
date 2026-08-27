use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct ModelFlex {
    pub controllers: Vec<(String, f32, f32)>,
    descriptors: usize,
    rules: Vec<(usize, Vec<(u32, u32)>)>,
    shapes: Vec<Shape>,
}

#[derive(Clone, Debug)]
struct Shape {
    descriptor: usize,
    pair: usize,
    targets: [f32; 4],
    vertices: Vec<Delta>,
}
#[derive(Clone, Debug)]
struct Delta {
    vertex: usize,
    side: f32,
    position: [f32; 3],
    normal: [f32; 3],
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    let end = at.checked_add(4).ok_or("model flex offset overflow")?;
    Ok(u32::from_le_bytes(
        b.get(at..end)
            .ok_or("truncated model flex")?
            .try_into()
            .unwrap(),
    ))
}
fn count(b: &[u8], at: usize, maximum: usize) -> Result<usize, String> {
    let n = u32_at(b, at)? as usize;
    if n > maximum {
        Err("model flex count limit".into())
    } else {
        Ok(n)
    }
}
fn float(b: &[u8], at: usize) -> Result<f32, String> {
    let f = f32::from_bits(u32_at(b, at)?);
    if f.is_finite() {
        Ok(f)
    } else {
        Err("non-finite model flex".into())
    }
}
fn relative(b: &[u8], base: usize, at: usize) -> Result<usize, String> {
    base.checked_add_signed(u32_at(b, at)? as i32 as isize)
        .filter(|value| *value <= b.len())
        .ok_or("model flex offset overflow".into())
}
fn string(b: &[u8], at: usize) -> Result<String, String> {
    let bytes = b.get(at..).ok_or("model flex string offset")?;
    let length = bytes
        .iter()
        .take(4096)
        .position(|v| *v == 0)
        .ok_or("model flex string limit")?;
    String::from_utf8(bytes[..length].to_vec()).map_err(|_| "model flex string encoding".into())
}

pub fn read_model_flex(bytes: &[u8]) -> Result<ModelFlex, String> {
    if bytes.get(..4) != Some(b"IDST") || bytes.len() < 408 || bytes.len() > 128 * 1024 * 1024 {
        return Err("invalid model flex header".into());
    }
    let descriptors = count(bytes, 260, 1024)?;
    let mut result = ModelFlex {
        descriptors,
        ..Default::default()
    };
    let table = count(bytes, 272, bytes.len())?;
    for i in 0..count(bytes, 268, 96)? {
        let at = table + i * 20;
        result.controllers.push((
            string(bytes, relative(bytes, at, at + 4)?)?,
            float(bytes, at + 12)?,
            float(bytes, at + 16)?,
        ));
    }
    let table = count(bytes, 280, bytes.len())?;
    for i in 0..count(bytes, 276, 4096)? {
        let at = table + i * 12;
        let descriptor = count(bytes, at, descriptors.saturating_sub(1))?;
        let ops = count(bytes, at + 4, 4096)?;
        let start = relative(bytes, at, at + 8)?;
        let operations = (0..ops)
            .map(|i| {
                Ok((
                    u32_at(bytes, start + i * 8)?,
                    u32_at(bytes, start + i * 8 + 4)?,
                ))
            })
            .collect::<Result<_, String>>()?;
        result.rules.push((descriptor, operations));
    }
    let flags = u32_at(bytes, 152)?;
    let scale = if flags & 0x0020_0000 != 0 {
        float(bytes, 392)?
    } else {
        1.0 / 4096.0
    };
    if scale <= 0.0 {
        return Err("invalid model flex scale".into());
    }
    let bodies = count(bytes, 236, bytes.len())?;
    for body in 0..count(bytes, 232, 256)? {
        let at = bodies + body * 16;
        let models = relative(bytes, at, at + 12)?;
        for model in 0..count(bytes, at + 4, 256)? {
            let at = models + model * 148;
            let vertex_base = u32_at(bytes, at + 84)? as usize / 48;
            let meshes = relative(bytes, at, at + 76)?;
            for mesh in 0..count(bytes, at + 72, 4096)? {
                let at = meshes + mesh * 116;
                let mesh_base = vertex_base + u32_at(bytes, at + 12)? as usize;
                let flexes = relative(bytes, at, at + 20)?;
                for flex in 0..count(bytes, at + 16, 4096)? {
                    let at = flexes + flex * 60;
                    let descriptor = count(bytes, at, descriptors.saturating_sub(1))?;
                    let targets = [
                        float(bytes, at + 4)?,
                        float(bytes, at + 8)?,
                        float(bytes, at + 12)?,
                        float(bytes, at + 16)?,
                    ];
                    let pair = count(bytes, at + 28, descriptors.saturating_sub(1))?;
                    let kind = *bytes.get(at + 32).ok_or("truncated flex kind")?;
                    if kind > 1 {
                        return Err("invalid flex vertex kind".into());
                    }
                    let vertices = relative(bytes, at, at + 24)?;
                    let mut deltas = Vec::new();
                    for i in 0..count(bytes, at + 20, 65536)? {
                        let at = vertices + i * if kind == 0 { 16 } else { 18 };
                        let data = bytes.get(at..at + 16).ok_or("truncated flex delta")?;
                        let mut values = [0.0; 6];
                        for (axis, value) in values.iter_mut().enumerate() {
                            let v = u16::from_le_bytes([data[4 + axis * 2], data[5 + axis * 2]]);
                            let fixed = if flags & 0x4000 != 0 {
                                v as i16
                            } else {
                                let value = (super::half_to_f32(v) / scale).trunc();
                                if !value.is_finite() || !(-32768.0..=32767.0).contains(&value) {
                                    return Err("invalid flex delta value".into());
                                }
                                value as i16
                            };
                            *value = fixed as f32 * scale;
                        }
                        deltas.push(Delta {
                            vertex: mesh_base + u16::from_le_bytes([data[0], data[1]]) as usize,
                            side: data[3] as f32 / 255.0,
                            position: [values[0], values[1], values[2]],
                            normal: [values[3], values[4], values[5]],
                        });
                    }
                    result.shapes.push(Shape {
                        descriptor,
                        pair,
                        targets,
                        vertices: deltas,
                    });
                }
            }
        }
    }
    Ok(result)
}

fn remap(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    if a == b {
        if v >= b { d } else { c }
    } else {
        c + (d - c) * ((v - a) / (b - a)).clamp(0.0, 1.0)
    }
}

impl ModelFlex {
    pub fn weights(&self, input: &BTreeMap<&str, f32>) -> Result<Vec<f32>, String> {
        let src = self
            .controllers
            .iter()
            .map(|(name, min, _)| input.get(name.as_str()).copied().unwrap_or(*min))
            .collect::<Vec<_>>();
        let mut dest = vec![0.0; self.descriptors];
        for (target, ops) in &self.rules {
            let mut stack: Vec<f32> = Vec::new();
            for &(op, operand) in ops {
                let index = operand as usize;
                let get = |i: usize| src.get(i).copied().ok_or("invalid flex controller");
                let pop = |stack: &mut Vec<f32>| stack.pop().ok_or("flex stack underflow");
                match op {
                    1 => stack.push(f32::from_bits(operand)),
                    2 => stack.push(get(index)?),
                    3 => stack.push(*dest.get(index).ok_or("invalid flex descriptor")?),
                    4..=7 | 13 | 14 => {
                        let b = pop(&mut stack)?;
                        let a = pop(&mut stack)?;
                        stack.push(match op {
                            4 => a + b,
                            5 => a - b,
                            6 => a * b,
                            7 => {
                                if b > 0.0001 {
                                    a / b
                                } else {
                                    0.0
                                }
                            }
                            13 => a.max(b),
                            _ => a.min(b),
                        });
                    }
                    8 => {
                        let v = pop(&mut stack)?;
                        stack.push(-v);
                    }
                    15 => stack.push(remap(get(index)?, -1.0, 0.0, 1.0, 0.0)),
                    16 => stack.push(remap(get(index)?, 0.0, 1.0, 0.0, 1.0)),
                    17 => {
                        let control = pop(&mut stack)? as usize;
                        let d = pop(&mut stack)?;
                        let c = pop(&mut stack)?;
                        let b = pop(&mut stack)?;
                        let a = pop(&mut stack)?;
                        let value = get(control)?;
                        let scale = if value <= a || value >= d {
                            0.0
                        } else if value < b {
                            remap(value, a, b, 0.0, 1.0)
                        } else if value > c {
                            remap(value, c, d, 1.0, 0.0)
                        } else {
                            1.0
                        };
                        stack.push(scale * get(index)?);
                    }
                    18 | 19 => {
                        if index > stack.len() {
                            return Err("flex combo underflow".into());
                        }
                        let start = stack.len() - index;
                        let product = stack[start..].iter().product::<f32>();
                        stack.truncate(start);
                        if op == 18 {
                            stack.push(product);
                        } else {
                            let value = pop(&mut stack)?;
                            stack.push(value * (1.0 - product));
                        }
                    }
                    20 | 21 => {
                        let close = pop(&mut stack)? as usize;
                        pop(&mut stack)?;
                        let eye = pop(&mut stack)? as i32;
                        let normalized = |i: usize| {
                            let c = self.controllers.get(i).ok_or("invalid eyelid controller")?;
                            Ok::<_, &str>(remap(get(i)?, c.1, c.2, 0.0, 1.0))
                        };
                        let vertical = normalized(index)?;
                        let close = normalized(close)?;
                        let eye = if eye >= 0 {
                            normalized(eye as usize)? * 2.0 - 1.0
                        } else {
                            0.0
                        };
                        stack.push(if op == 20 {
                            (1.0 - eye.max(0.0)) * (1.0 - vertical) * close
                        } else {
                            (1.0 + eye.min(0.0)) * vertical * close
                        });
                    }
                    _ => return Err(format!("unsupported flex operator {op}")),
                }
                if stack.len() > 32 || stack.iter().any(|v| !v.is_finite()) {
                    return Err("invalid flex stack".into());
                }
            }
            dest[*target] = stack.first().copied().unwrap_or(0.0);
        }
        Ok(dest)
    }

    pub fn deltas(&self, weights: &[f32]) -> BTreeMap<usize, ([f32; 3], [f32; 3])> {
        let mut output = BTreeMap::new();
        for shape in &self.shapes {
            let [a, b, c, d] = shape.targets;
            let weight = |index: usize| {
                let v = weights.get(index).copied().unwrap_or(0.0);
                if v <= a || v >= d {
                    0.0
                } else if v < b {
                    (v - a) / (b - a)
                } else if v > c {
                    (d - v) / (d - c)
                } else {
                    1.0
                }
            };
            let left = weight(shape.descriptor);
            let right = if shape.pair == 0 {
                left
            } else {
                weight(shape.pair)
            };
            for vertex in &shape.vertices {
                let (position, normal) =
                    output.entry(vertex.vertex).or_insert(([0.0; 3], [0.0; 3]));
                let w = left + (right - left) * vertex.side;
                for axis in 0..3 {
                    position[axis] += vertex.position[axis] * w;
                    normal[axis] += vertex.normal[axis] * w;
                }
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_preserve_controller_ranges_and_guarded_division() {
        let flex = ModelFlex {
            controllers: vec![("a".into(), -1.0, 1.0), ("b".into(), 0.0, 1.0)],
            descriptors: 3,
            rules: vec![
                (0, vec![(2, 0), (15, 0), (4, 0)]),
                (1, vec![(2, 1), (1, (-0.5f32).to_bits()), (7, 0)]),
                (2, vec![(2, 0), (2, 1), (18, 2)]),
            ],
            shapes: Vec::new(),
        };
        let input = BTreeMap::from([("a", -0.5), ("b", 0.5)]);
        assert_eq!(flex.weights(&input).unwrap(), vec![0.0, 0.0, -0.25]);
    }
    #[test]
    fn paired_flexes_apply_sides_in_bind_space_and_retain_zero_resets() {
        let flex = ModelFlex {
            descriptors: 2,
            shapes: vec![Shape {
                descriptor: 0,
                pair: 1,
                targets: [0.0, 1.0, 10.0, 11.0],
                vertices: vec![Delta {
                    vertex: 3,
                    side: 0.25,
                    position: [2.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                }],
            }],
            ..Default::default()
        };
        assert_eq!(
            flex.deltas(&[1.0, 0.0])[&3],
            ([1.5, 0.0, 0.0], [0.0, 0.75, 0.0])
        );
        assert_eq!(flex.deltas(&[0.0, 0.0])[&3], ([0.0; 3], [0.0; 3]));
        assert!(read_model_flex(&[0; 408]).is_err());
    }
}
