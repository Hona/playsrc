use playsrc_bsp::{
    Brush as BspBrush, BrushSide as BspSide, Bsp, Leaf, LumpData, Model, Node, Plane as BspPlane,
};
use std::{fmt, ops::Range};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub distance: f32,
    pub kind: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Brush {
    pub first_side: usize,
    pub side_count: usize,
    pub contents: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Side {
    pub plane: usize,
    pub texture_info: i16,
    pub displacement: i16,
    pub bevel: i16,
}
#[derive(Clone, Debug)]
pub struct World {
    pub planes: Vec<Plane>,
    pub sides: Vec<Side>,
    pub brushes: Vec<Brush>,
    pub leaves: Vec<Leaf>,
    pub leaf_brushes: Vec<u16>,
    pub nodes: Vec<Node>,
    pub models: Vec<Model>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub fraction: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub brush: Option<usize>,
    pub contents: u32,
    pub plane: Option<Plane>,
    pub end: [f32; 3],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidRange,
    InvalidReference,
    NonFinite,
    InvalidHull,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub item: Option<usize>,
    pub range: Option<Range<usize>>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for Error {}

pub fn compile(bsp: &Bsp) -> Result<World, Error> {
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let sides = match &bsp.lumps[19].records {
        LumpData::BrushSides(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let brushes = match &bsp.lumps[18].records {
        LumpData::Brushes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaves = match &bsp.lumps[10].records {
        LumpData::Leaves(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaf_brushes = match &bsp.lumps[17].records {
        LumpData::LeafBrushes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let nodes = match &bsp.lumps[5].records {
        LumpData::Nodes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let models = match &bsp.lumps[14].records {
        LumpData::Models(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let planes = planes
        .iter()
        .enumerate()
        .map(|(i, p)| plane(p, i))
        .collect::<Result<Vec<_>, _>>()?;
    let sides = sides.iter().map(side).collect::<Vec<_>>();
    let mut output = Vec::with_capacity(brushes.len());
    for (i, b) in brushes.iter().enumerate() {
        let brush = brush(b, i, sides.len())?;
        for side in &sides[brush.first_side..brush.first_side + brush.side_count] {
            if side.plane >= planes.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
        }
        output.push(brush);
    }
    for (i, leaf) in leaves.iter().enumerate() {
        let start = leaf.first_leaf_brush as usize;
        let end = start
            .checked_add(leaf.leaf_brush_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(i)))?;
        if end > leaf_brushes.len()
            || leaf_brushes[start..end]
                .iter()
                .any(|v| *v as usize >= output.len())
        {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
    }
    Ok(World {
        planes,
        sides,
        brushes: output,
        leaves,
        leaf_brushes,
        nodes,
        models,
    })
}
fn plane(p: &BspPlane, i: usize) -> Result<Plane, Error> {
    let normal = [p.normal.x.value(), p.normal.y.value(), p.normal.z.value()];
    let distance = p.distance.value();
    if normal.iter().any(|v| !v.is_finite()) || !distance.is_finite() {
        return Err(error(ErrorCode::NonFinite, Some(i)));
    }
    Ok(Plane {
        normal,
        distance,
        kind: p.kind,
    })
}
fn side(v: &BspSide) -> Side {
    Side {
        plane: v.plane_index as usize,
        texture_info: v.texture_info_index,
        displacement: v.displacement_info_index,
        bevel: v.bevel,
    }
}
fn brush(v: &BspBrush, i: usize, total: usize) -> Result<Brush, Error> {
    let first =
        usize::try_from(v.first_side).map_err(|_| error(ErrorCode::InvalidRange, Some(i)))?;
    let count =
        usize::try_from(v.side_count).map_err(|_| error(ErrorCode::InvalidRange, Some(i)))?;
    if first.checked_add(count).is_none_or(|x| x > total) {
        return Err(error(ErrorCode::InvalidRange, Some(i)));
    }
    Ok(Brush {
        first_side: first,
        side_count: count,
        contents: v.contents as u32,
    })
}

impl World {
    pub fn trace_hull(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, Error> {
        if start
            .iter()
            .chain(end.iter())
            .chain(hull.mins.iter())
            .chain(hull.maxs.iter())
            .any(|v| !v.is_finite())
            || hull.mins.iter().zip(hull.maxs).any(|(a, b)| *a > b)
        {
            return Err(error(ErrorCode::InvalidHull, None));
        }
        let center = [
            (hull.mins[0] + hull.maxs[0]) * 0.5,
            (hull.mins[1] + hull.maxs[1]) * 0.5,
            (hull.mins[2] + hull.maxs[2]) * 0.5,
        ];
        let extents = [
            (hull.maxs[0] - hull.mins[0]) * 0.5,
            (hull.maxs[1] - hull.mins[1]) * 0.5,
            (hull.maxs[2] - hull.mins[2]) * 0.5,
        ];
        let trace_start = [
            start[0] + center[0],
            start[1] + center[1],
            start[2] + center[2],
        ];
        let trace_end = [end[0] + center[0], end[1] + center[1], end[2] + center[2]];
        let mut result = Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            brush: None,
            contents: 0,
            plane: None,
            end,
        };
        for (index, brush) in self.brushes.iter().enumerate() {
            if brush.contents & mask == 0 {
                continue;
            }
            let mut enter = -1.0_f32;
            let mut leave = 1.0_f32;
            let mut starts_out = false;
            let mut ends_out = false;
            let mut clip = None;
            let mut rejected = false;
            for side in &self.sides[brush.first_side..brush.first_side + brush.side_count] {
                let p = self.planes[side.plane];
                let offset = extents[0] * p.normal[0].abs()
                    + extents[1] * p.normal[1].abs()
                    + extents[2] * p.normal[2].abs();
                let d1 = dot(trace_start, p.normal) - p.distance - offset;
                let d2 = dot(trace_end, p.normal) - p.distance - offset;
                if d1 > 0.0 {
                    starts_out = true
                }
                if d2 > 0.0 {
                    ends_out = true
                }
                if d1 > 0.0 && d2 >= d1 {
                    rejected = true;
                    break;
                }
                if d1 <= 0.0 && d2 <= 0.0 {
                    continue;
                }
                if d1 > d2 {
                    let f = (d1 - 0.03125) / (d1 - d2);
                    if f > enter {
                        enter = f;
                        clip = Some(p)
                    }
                } else {
                    leave = leave.min((d1 + 0.03125) / (d1 - d2));
                }
            }
            if rejected {
                continue;
            }
            if !starts_out {
                result.start_solid = true;
                result.contents |= brush.contents;
                if !ends_out {
                    result.all_solid = true;
                    result.brush = Some(index);
                }
                continue;
            }
            if enter < leave && enter >= 0.0 && enter < result.fraction {
                result.fraction = enter.max(0.0);
                result.brush = Some(index);
                result.contents = brush.contents;
                result.plane = clip;
            }
        }
        result.end = [
            start[0] + (end[0] - start[0]) * result.fraction,
            start[1] + (end[1] - start[1]) * result.fraction,
            start[2] + (end[2] - start[2]) * result.fraction,
        ];
        Ok(result)
    }
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn error(code: ErrorCode, item: Option<usize>) -> Error {
    Error {
        code,
        item,
        range: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_bsp::{Limits, Profile, parse};
    fn set(b: &mut [u8], i: usize, o: usize, l: usize, v: i32) {
        let h = 8 + i * 16;
        b[h..h + 4].copy_from_slice(&(o as i32).to_le_bytes());
        b[h + 4..h + 8].copy_from_slice(&(l as i32).to_le_bytes());
        b[h + 8..h + 12].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture() -> Bsp {
        let mut b = vec![0; 1036];
        b[..4].copy_from_slice(b"VBSP");
        b[4..8].copy_from_slice(&20_i32.to_le_bytes());
        let mut add = |i: usize, v: i32, x: &[u8]| {
            let o = b.len();
            b.extend_from_slice(x);
            set(&mut b, i, o, x.len(), v);
        };
        let mut p = Vec::new();
        for (n, d) in [
            ([1_f32, 0., 0.], 16_f32),
            ([-1., 0., 0.], 16.),
            ([0., 1., 0.], 16.),
            ([0., -1., 0.], 16.),
            ([0., 0., 1.], 16.),
            ([0., 0., -1.], 16.),
        ] {
            for x in n {
                p.extend_from_slice(&x.to_le_bytes())
            }
            p.extend_from_slice(&d.to_le_bytes());
            p.extend_from_slice(&0_i32.to_le_bytes());
        }
        add(1, 0, &p);
        let mut sides = Vec::new();
        for i in 0..6_u16 {
            sides.extend_from_slice(&i.to_le_bytes());
            sides.extend_from_slice(&(-1_i16).to_le_bytes());
            sides.extend_from_slice(&(-1_i16).to_le_bytes());
            sides.extend_from_slice(&0_i16.to_le_bytes());
        }
        add(19, 0, &sides);
        let mut brush = 0_i32.to_le_bytes().to_vec();
        brush.extend_from_slice(&6_i32.to_le_bytes());
        brush.extend_from_slice(&1_i32.to_le_bytes());
        add(18, 0, &brush);
        let mut leaf = vec![0; 32];
        leaf[24..26].copy_from_slice(&0_u16.to_le_bytes());
        leaf[26..28].copy_from_slice(&1_u16.to_le_bytes());
        add(10, 1, &leaf);
        add(17, 0, &0_u16.to_le_bytes());
        add(5, 0, &[0; 32]);
        add(14, 0, &[0; 48]);
        parse(&b, Profile::Source2013V20, Limits::default()).unwrap()
    }
    #[test]
    fn traces_point_and_hull_against_brush() {
        let w = compile(&fixture()).unwrap();
        let point = Hull {
            mins: [0.; 3],
            maxs: [0.; 3],
        };
        let t = w
            .trace_hull([-32., 0., 0.], [32., 0., 0.], point, 1)
            .unwrap();
        assert_eq!(t.brush, Some(0));
        assert!(t.fraction > 0.24 && t.fraction < 0.26);
        assert_eq!(t.plane.unwrap().normal, [-1., 0., 0.]);
        let hull = Hull {
            mins: [-2.; 3],
            maxs: [2.; 3],
        };
        let h = w
            .trace_hull([-32., 0., 0.], [32., 0., 0.], hull, 1)
            .unwrap();
        assert!(h.fraction < t.fraction);
        let inside = w.trace_hull([0.; 3], [1., 0., 0.], point, 1).unwrap();
        assert!(inside.start_solid && inside.all_solid);
    }
}
