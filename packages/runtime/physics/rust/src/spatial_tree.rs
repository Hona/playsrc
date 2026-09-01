use crate::{SpatialCell, SpatialCellError};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpatialNode {
    pub parent: Option<SpatialCell>,
    pub children: Vec<SpatialCell>,
    pub members: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Sphere {
    cell: SpatialCell,
    center: [f32; 3],
    radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpatialIndex {
    root: Option<SpatialCell>,
    nodes: BTreeMap<SpatialCell, SpatialNode>,
    spheres: BTreeMap<u64, Sphere>,
    maximum_elements: usize,
    maximum_nodes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpatialInsertion {
    pub cell: SpatialCell,
    pub radius: f64,
    pub partners: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialIndexError {
    InvalidLimit,
    InvalidIdentity,
    DuplicateElement,
    Limit,
    InconsistentCell,
    Cell(SpatialCellError),
}

impl fmt::Display for SpatialIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimit => formatter.write_str("spatial index limits must be positive"),
            Self::InvalidIdentity => formatter.write_str("spatial identity must be nonzero"),
            Self::DuplicateElement => {
                formatter.write_str("spatial element must be removed before reinsertion")
            }
            Self::Limit => formatter.write_str("spatial index exceeds its configured bound"),
            Self::InconsistentCell => {
                formatter.write_str("spatial cell has an inconsistent parent")
            }
            Self::Cell(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for SpatialIndexError {}
impl From<SpatialCellError> for SpatialIndexError {
    fn from(error: SpatialCellError) -> Self {
        Self::Cell(error)
    }
}

impl SpatialIndex {
    pub fn new(maximum_elements: usize, maximum_nodes: usize) -> Result<Self, SpatialIndexError> {
        if maximum_elements == 0 || maximum_nodes == 0 {
            return Err(SpatialIndexError::InvalidLimit);
        }
        Ok(Self {
            root: None,
            nodes: BTreeMap::new(),
            spheres: BTreeMap::new(),
            maximum_elements,
            maximum_nodes,
        })
    }

    pub fn root(&self) -> Option<SpatialCell> {
        self.root
    }
    pub fn nodes(&self) -> impl Iterator<Item = (&SpatialCell, &SpatialNode)> {
        self.nodes.iter()
    }
    pub fn element_count(&self) -> usize {
        self.spheres.len()
    }
    pub(crate) fn contains(&self, identity: u64) -> bool {
        self.spheres.contains_key(&identity)
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert(
        &mut self,
        identity: u64,
        center: [f32; 3],
        minimum_radius: f64,
        maximum_radius: f64,
        collect: bool,
    ) -> Result<SpatialInsertion, SpatialIndexError> {
        if identity == 0 {
            return Err(SpatialIndexError::InvalidIdentity);
        }
        if self.spheres.contains_key(&identity) {
            return Err(SpatialIndexError::DuplicateElement);
        }
        if self.spheres.len() == self.maximum_elements {
            return Err(SpatialIndexError::Limit);
        }
        let fitted = SpatialCell::fit(center, minimum_radius, maximum_radius)?;
        let first_root = self.root.is_none();
        let mut edit = TreeEdit {
            base: self,
            root: self.root,
            nodes: BTreeMap::new(),
            added: 0,
        };
        edit.insert(fitted.cell, identity)?;
        let (root, changed) = edit.finish();
        self.nodes.extend(changed);
        self.root = root;
        let sphere = Sphere {
            cell: fitted.cell,
            center,
            radius: fitted.radius as f32,
        };
        self.spheres.insert(identity, sphere);
        let partners = if collect && !first_root {
            self.collect(sphere)
        } else {
            Vec::new()
        };
        Ok(SpatialInsertion {
            cell: fitted.cell,
            radius: fitted.radius,
            partners,
        })
    }

    pub fn remove(&mut self, identity: u64) -> bool {
        let Some(sphere) = self.spheres.remove(&identity) else {
            return false;
        };
        let node = self
            .nodes
            .get_mut(&sphere.cell)
            .expect("admitted sphere node");
        let position = node
            .members
            .iter()
            .position(|member| *member == identity)
            .expect("admitted sphere membership");
        node.members.remove(position);
        let mut current = sphere.cell;
        loop {
            let node = &self.nodes[&current];
            if !node.members.is_empty() || !node.children.is_empty() {
                break;
            }
            let parent = node.parent;
            self.nodes.remove(&current);
            let Some(parent) = parent else {
                self.root = None;
                break;
            };
            let children = &mut self
                .nodes
                .get_mut(&parent)
                .expect("admitted parent")
                .children;
            let position = children
                .iter()
                .position(|child| *child == current)
                .expect("admitted child");
            children.remove(position);
            current = parent;
        }
        true
    }

    fn collect(&self, sphere: Sphere) -> Vec<u64> {
        let mut output = Vec::new();
        let mut pending = vec![(self.root.expect("inserted sphere root"), false)];
        while let Some((cell, subtree)) = pending.pop() {
            let node = &self.nodes[&cell];
            for identity in node.members.iter().rev() {
                let other = self.spheres[identity];
                let delta: [f64; 3] =
                    std::array::from_fn(|axis| f64::from(other.center[axis] - sphere.center[axis]));
                let squared = (delta[1] * delta[1] + delta[0] * delta[0]) + delta[2] * delta[2];
                let radius = f64::from(other.radius + sphere.radius);
                if squared <= radius * radius {
                    output.push(*identity);
                }
            }
            if subtree {
                pending.extend(node.children.iter().map(|child| (*child, true)));
            } else {
                for child in node.children.iter().rev() {
                    if *child == sphere.cell {
                        pending.push((*child, true));
                    } else if overlaps(*child, sphere.cell) {
                        pending.push((*child, false));
                    }
                }
            }
        }
        output
    }
}

// An insertion prepares only changed ancestors/membership, not a copy of the tree.
struct TreeEdit<'a> {
    base: &'a SpatialIndex,
    root: Option<SpatialCell>,
    nodes: BTreeMap<SpatialCell, SpatialNode>,
    added: usize,
}

impl TreeEdit<'_> {
    fn node(&self, cell: SpatialCell) -> &SpatialNode {
        self.nodes
            .get(&cell)
            .unwrap_or_else(|| &self.base.nodes[&cell])
    }
    fn node_mut(&mut self, cell: SpatialCell) -> &mut SpatialNode {
        self.nodes
            .entry(cell)
            .or_insert_with(|| self.base.nodes[&cell].clone())
    }
    fn add(&mut self, cell: SpatialCell, node: SpatialNode) -> Result<(), SpatialIndexError> {
        if self.nodes.contains_key(&cell) || self.base.nodes.contains_key(&cell) {
            return Err(SpatialIndexError::InconsistentCell);
        }
        if self.base.nodes.len() + self.added == self.base.maximum_nodes {
            return Err(SpatialIndexError::Limit);
        }
        self.nodes.insert(cell, node);
        self.added += 1;
        Ok(())
    }
    fn insert(&mut self, cell: SpatialCell, identity: u64) -> Result<(), SpatialIndexError> {
        if self.base.nodes.contains_key(&cell) {
            self.node_mut(cell).members.push(identity);
            return Ok(());
        }
        let Some(mut root) = self.root else {
            self.add(
                cell,
                SpatialNode {
                    parent: None,
                    children: Vec::new(),
                    members: vec![identity],
                },
            )?;
            self.root = Some(cell);
            return Ok(());
        };
        while !contains(root, cell) {
            let raster = root.raster_exponent + 1;
            let scale = power(raster)?;
            let target_scale = power(cell.raster_exponent)?;
            let origin = std::array::from_fn(|axis| {
                let old = root.origin[axis];
                let remainder = old % 2;
                let half = old / 2;
                if remainder == -1
                    || remainder == 0
                        && f64::from(cell.origin[axis]) * target_scale < f64::from(half) * scale
                {
                    half.wrapping_sub(1)
                } else {
                    half
                }
            });
            let expanded = SpatialCell {
                origin,
                raster_exponent: raster,
                size_exponent: root.size_exponent + 1,
            };
            self.add(
                expanded,
                SpatialNode {
                    parent: None,
                    children: vec![root],
                    members: Vec::new(),
                },
            )?;
            self.node_mut(root).parent = Some(expanded);
            root = expanded;
            self.root = Some(root);
        }
        if root.raster_exponent == cell.raster_exponent {
            if root != cell {
                return Err(SpatialIndexError::InconsistentCell);
            }
            self.node_mut(root).members.push(identity);
            return Ok(());
        }
        let mut parent = root;
        while let Some(child) = self
            .node(parent)
            .children
            .iter()
            .copied()
            .find(|child| contains(*child, cell))
        {
            parent = child;
        }
        while parent.size_exponent - cell.size_exponent > 1 {
            let difference = (parent.size_exponent - cell.size_exponent) as u32;
            let origin = std::array::from_fn(|axis| {
                let twice = parent.origin[axis].wrapping_mul(2);
                if cell.origin[axis] >= parent.origin[axis].wrapping_add(1).wrapping_shl(difference)
                {
                    twice.wrapping_add(2)
                } else if cell.origin[axis] >= twice.wrapping_add(1).wrapping_shl(difference - 1) {
                    twice.wrapping_add(1)
                } else {
                    twice
                }
            });
            let next = SpatialCell {
                origin,
                raster_exponent: parent.raster_exponent - 1,
                size_exponent: parent.size_exponent - 1,
            };
            self.add(
                next,
                SpatialNode {
                    parent: Some(parent),
                    children: Vec::new(),
                    members: Vec::new(),
                },
            )?;
            self.node_mut(parent).children.push(next);
            parent = next;
        }
        if parent.size_exponent - cell.size_exponent != 1 {
            return Err(SpatialIndexError::InconsistentCell);
        }
        self.add(
            cell,
            SpatialNode {
                parent: Some(parent),
                children: Vec::new(),
                members: vec![identity],
            },
        )?;
        self.node_mut(parent).children.push(cell);
        Ok(())
    }
    fn finish(self) -> (Option<SpatialCell>, BTreeMap<SpatialCell, SpatialNode>) {
        (self.root, self.nodes)
    }
}

fn power(exponent: i32) -> Result<f64, SpatialIndexError> {
    if !(-40..=40).contains(&exponent) {
        return Err(SpatialCellError::UnsupportedScale.into());
    }
    Ok(f64::from_bits(((1023 + exponent) as u64) << 52))
}

fn contains(parent: SpatialCell, child: SpatialCell) -> bool {
    if parent.raster_exponent < child.raster_exponent {
        return false;
    }
    let shift = (parent.raster_exponent - child.raster_exponent) as u32;
    let offset = 2_i32.wrapping_shl(shift).wrapping_sub(2);
    (0..3).all(|axis| {
        let lower = parent.origin[axis].wrapping_shl(shift);
        child.origin[axis] >= lower && child.origin[axis] <= lower.wrapping_add(offset)
    })
}

fn overlaps(first: SpatialCell, second: SpatialCell) -> bool {
    let (large, small) = if first.raster_exponent > second.raster_exponent {
        (first, second)
    } else {
        (second, first)
    };
    let shift = (large.raster_exponent - small.raster_exponent) as u32;
    (0..3).all(|axis| {
        small.origin[axis].wrapping_add(2) > large.origin[axis].wrapping_shl(shift)
            && small.origin[axis] < large.origin[axis].wrapping_add(2).wrapping_shl(shift)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_order_noop_removal_and_failed_expansion_are_atomic() {
        let mut tree = SpatialIndex::new(4, 32).unwrap();
        assert!(
            tree.insert(1, [0.0; 3], 1.0, 1.0, true)
                .unwrap()
                .partners
                .is_empty()
        );
        assert_eq!(
            tree.insert(2, [0.0; 3], 1.0, 1.0, true).unwrap().partners,
            [2, 1]
        );
        assert_eq!(
            tree.insert(3, [0.0; 3], 1.0, 1.0, true).unwrap().partners,
            [3, 2, 1]
        );
        tree.remove(2);
        assert_eq!(tree.nodes().next().unwrap().1.members, [1, 3]);
        let before = tree.clone();
        assert_eq!(
            tree.insert(1, [0.0; 3], 1.0, 1.0, true),
            Err(SpatialIndexError::DuplicateElement)
        );
        assert_eq!(tree, before);
        assert!(!tree.remove(99));
        assert_eq!(tree, before);
        tree.remove(1);
        tree.remove(3);
        assert_eq!(tree.root(), None);
        let mut bounded = SpatialIndex::new(2, 1).unwrap();
        bounded.insert(1, [0.0; 3], 1.0, 1.0, false).unwrap();
        let before = bounded.clone();
        assert_eq!(
            bounded.insert(2, [128.0; 3], 1.0, 1.0, true),
            Err(SpatialIndexError::Limit)
        );
        assert_eq!(bounded, before);
    }
}
