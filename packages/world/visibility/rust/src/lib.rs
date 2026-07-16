use playsrc_bsp::{Bsp, Leaf, LumpData, Model, Node, Visibility as BspVisibility};
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub distance: f32,
}
#[derive(Clone, Debug)]
pub struct World {
    pub cluster_count: usize,
    pub words_per_row: usize,
    pub pvs: Vec<u32>,
    pub pas: Vec<u32>,
    pub planes: Vec<Plane>,
    pub nodes: Vec<Node>,
    pub leaves: Vec<Leaf>,
    pub leaf_faces: Vec<u16>,
    pub models: Vec<Model>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidCount,
    InvalidOffset,
    TruncatedRow,
    InvalidRun,
    InvalidReference,
    NonFinite,
    DepthLimit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub item: Option<usize>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for Error {}
pub fn compile(bsp: &Bsp) -> Result<World, Error> {
    let visibility = match &bsp.lumps[4].records {
        LumpData::Visibility(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let nodes = match &bsp.lumps[5].records {
        LumpData::Nodes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaves = match &bsp.lumps[10].records {
        LumpData::Leaves(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaf_faces = match &bsp.lumps[16].records {
        LumpData::LeafFaces(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let models = match &bsp.lumps[14].records {
        LumpData::Models(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let cluster_count = usize::try_from(visibility.cluster_count)
        .map_err(|_| error(ErrorCode::InvalidCount, None))?;
    let words = cluster_count.div_ceil(32);
    let pvs = decode_rows(visibility, 0, cluster_count, words)?;
    let pas = decode_rows(visibility, 1, cluster_count, words)?;
    let mut output_planes = Vec::with_capacity(planes.len());
    for (i, p) in planes.iter().enumerate() {
        let normal = [p.normal.x.value(), p.normal.y.value(), p.normal.z.value()];
        let distance = p.distance.value();
        if normal.iter().any(|v| !v.is_finite()) || !distance.is_finite() {
            return Err(error(ErrorCode::NonFinite, Some(i)));
        }
        output_planes.push(Plane { normal, distance });
    }
    for (i, node) in nodes.iter().enumerate() {
        if node.plane_index < 0 || node.plane_index as usize >= output_planes.len() {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
        for child in node.children {
            if child >= 0 && child as usize >= nodes.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
            if child < 0 && (-1_i64 - child as i64) as usize >= leaves.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
        }
    }
    for (i, leaf) in leaves.iter().enumerate() {
        if leaf.cluster >= 0 && leaf.cluster as usize >= cluster_count {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
        let start = leaf.first_leaf_face as usize;
        let end = start
            .checked_add(leaf.leaf_face_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(i)))?;
        if end > leaf_faces.len() {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
    }
    Ok(World {
        cluster_count,
        words_per_row: words,
        pvs,
        pas,
        planes: output_planes,
        nodes,
        leaves,
        leaf_faces,
        models,
    })
}
fn decode_rows(
    v: &BspVisibility,
    kind: usize,
    clusters: usize,
    words: usize,
) -> Result<Vec<u32>, Error> {
    let mut result = Vec::with_capacity(clusters * words);
    let row_bytes = clusters.div_ceil(8);
    for cluster in 0..clusters {
        let raw = v
            .offsets
            .get(cluster)
            .ok_or_else(|| error(ErrorCode::InvalidOffset, Some(cluster)))?[kind];
        let offset =
            usize::try_from(raw).map_err(|_| error(ErrorCode::InvalidOffset, Some(cluster)))?;
        if offset < v.compressed_range.start || offset > v.compressed_range.end {
            return Err(error(ErrorCode::InvalidOffset, Some(cluster)));
        }
        let mut at = offset - v.compressed_range.start;
        let mut decoded = Vec::with_capacity(row_bytes);
        while decoded.len() < row_bytes {
            let Some(&byte) = v.compressed_bytes.get(at) else {
                return Err(error(ErrorCode::TruncatedRow, Some(cluster)));
            };
            at += 1;
            if byte != 0 {
                decoded.push(byte)
            } else {
                let Some(&run) = v.compressed_bytes.get(at) else {
                    return Err(error(ErrorCode::TruncatedRow, Some(cluster)));
                };
                at += 1;
                if run == 0 || decoded.len() + run as usize > row_bytes {
                    return Err(error(ErrorCode::InvalidRun, Some(cluster)));
                }
                decoded.resize(decoded.len() + run as usize, 0);
            }
        }
        let mut row = vec![0_u32; words];
        for bit in 0..clusters {
            if decoded[bit / 8] & (1 << (bit % 8)) != 0 {
                row[bit / 32] |= 1 << (bit % 32);
            }
        }
        result.extend(row);
    }
    Ok(result)
}
impl World {
    pub fn visible(&self, from: usize, to: usize) -> bool {
        from < self.cluster_count
            && to < self.cluster_count
            && self.pvs[from * self.words_per_row + to / 32] & (1 << (to % 32)) != 0
    }
    pub fn audible(&self, from: usize, to: usize) -> bool {
        from < self.cluster_count
            && to < self.cluster_count
            && self.pas[from * self.words_per_row + to / 32] & (1 << (to % 32)) != 0
    }
    pub fn locate_leaf(&self, point: [f32; 3]) -> Result<usize, Error> {
        if point.iter().any(|v| !v.is_finite()) {
            return Err(error(ErrorCode::NonFinite, None));
        }
        let head = self
            .models
            .first()
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?
            .head_node;
        let mut child = head;
        for depth in 0..4096 {
            if child < 0 {
                return Ok((-1_i64 - child as i64) as usize);
            }
            let node = self
                .nodes
                .get(child as usize)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
            let plane = self.planes[node.plane_index as usize];
            let d = point[0] * plane.normal[0]
                + point[1] * plane.normal[1]
                + point[2] * plane.normal[2]
                - plane.distance;
            child = node.children[usize::from(d < 0.)];
            if depth == 4095 {
                return Err(error(ErrorCode::DepthLimit, None));
            }
        }
        unreachable!()
    }
}
fn error(code: ErrorCode, item: Option<usize>) -> Error {
    Error { code, item }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expands_rows_and_queries_bits() {
        let v = BspVisibility {
            cluster_count: 10,
            offsets: vec![[84, 84]; 10],
            compressed_bytes: vec![0b0000_0011, 0b0000_0010],
            compressed_range: 84..86,
        };
        let rows = decode_rows(&v, 0, 10, 1).unwrap();
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0] & 3, 3);
        let w = World {
            cluster_count: 10,
            words_per_row: 1,
            pvs: rows.clone(),
            pas: rows,
            planes: vec![],
            nodes: vec![],
            leaves: vec![],
            leaf_faces: vec![],
            models: vec![],
        };
        assert!(w.visible(0, 1));
        assert!(!w.visible(0, 2));
    }
    #[test]
    fn traverses_nodes_to_leaf() {
        let w = World {
            cluster_count: 0,
            words_per_row: 0,
            pvs: vec![],
            pas: vec![],
            planes: vec![Plane {
                normal: [1., 0., 0.],
                distance: 0.,
            }],
            nodes: vec![Node {
                plane_index: 0,
                children: [-1, -2],
                mins: [0; 3],
                maxs: [0; 3],
                first_face: 0,
                face_count: 0,
                area: 0,
                padding: 0,
            }],
            leaves: vec![],
            leaf_faces: vec![],
            models: vec![Model {
                mins: playsrc_bsp::Vector3 {
                    x: playsrc_bsp::Float32(0),
                    y: playsrc_bsp::Float32(0),
                    z: playsrc_bsp::Float32(0),
                },
                maxs: playsrc_bsp::Vector3 {
                    x: playsrc_bsp::Float32(0),
                    y: playsrc_bsp::Float32(0),
                    z: playsrc_bsp::Float32(0),
                },
                origin: playsrc_bsp::Vector3 {
                    x: playsrc_bsp::Float32(0),
                    y: playsrc_bsp::Float32(0),
                    z: playsrc_bsp::Float32(0),
                },
                head_node: 0,
                first_face: 0,
                face_count: 0,
            }],
        };
        assert_eq!(w.locate_leaf([1., 0., 0.]).unwrap(), 0);
        assert_eq!(w.locate_leaf([-1., 0., 0.]).unwrap(), 1);
    }
}
