//! Immutable geometry and its derived bone membership have one lifetime.
use playsrc_studio_model::{PresentationModel, SelectedPrimitive};

/// Source vertex bone IDs are bytes. Membership never changes authored
/// influences (including zero weights); iteration is ascending transport order.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct BonePalette([u64; 4]);

impl BonePalette {
    pub(super) fn from_bones(bones: impl IntoIterator<Item = u8>) -> Self {
        let mut result = Self::default();
        for bone in bones {
            result.0[usize::from(bone) / 64] |= 1_u64 << (bone % 64);
        }
        result
    }

    pub(super) fn len(self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
    }

    pub(super) fn iter(self) -> impl Iterator<Item = u8> {
        self.0
            .into_iter()
            .enumerate()
            .flat_map(|(index, mut word)| {
                std::iter::from_fn(move || {
                    if word == 0 {
                        return None;
                    }
                    let bone = (index * 64 + word.trailing_zeros() as usize) as u8;
                    word &= word - 1;
                    Some(bone)
                })
            })
    }
}

/// No mutable model access: metadata is indexed by this exact geometry, not a
/// pathname, actor, pose, skin or map generation. The existing dependency-checked
/// weak model cache shares and retires both together. Retention is exactly 32
/// bytes per primitive, independent of the number of actors or selections.
pub(super) struct RetainedPresentationModel {
    model: std::sync::Arc<PresentationModel>,
    palettes: Box<[BonePalette]>,
}

impl RetainedPresentationModel {
    pub(super) fn new(model: PresentationModel) -> Self {
        let palettes = model
            .geometry
            .iter()
            .map(|primitive| {
                BonePalette::from_bones(primitive.vertices.iter().flat_map(|vertex| {
                    #[cfg(test)]
                    VERTEX_VISITS.with(|count| count.set(count.get() + 1));
                    vertex.bones[..usize::from(vertex.bone_count)]
                        .iter()
                        .copied()
                }))
            })
            .collect();
        Self {
            model: std::sync::Arc::new(model),
            palettes,
        }
    }

    pub(super) fn source(&self) -> &std::sync::Arc<PresentationModel> {
        &self.model
    }

    pub(super) fn primitive_palette(&self, primitive: usize) -> Result<BonePalette, ()> {
        self.palettes.get(primitive).copied().ok_or(())
    }

    pub(super) fn selected_palette(
        &self,
        selected: &[SelectedPrimitive],
        attachments_only: bool,
    ) -> Result<BonePalette, ()> {
        let mut palette = BonePalette::default();
        if !attachments_only {
            for selection in selected {
                for (out, word) in palette
                    .0
                    .iter_mut()
                    .zip(self.primitive_palette(selection.primitive)?.0)
                {
                    *out |= word;
                }
            }
        }
        Ok(palette)
    }
}

impl std::ops::Deref for RetainedPresentationModel {
    type Target = PresentationModel;
    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

#[cfg(test)]
thread_local! { static VERTEX_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_studio_model::*;
    use std::sync::Arc;

    fn fixture() -> PresentationModel {
        let zero = Float32(0);
        let vector = Vector3([zero; 3]);
        let geometry = (0..12)
            .map(|index| GeometryPrimitive {
                body_part: index / 6,
                model: index / 3 % 2,
                lod: index % 3,
                mesh: 0,
                strip_group: 0,
                switch_point: zero,
                material_slot: index % 2,
                source_vertex_ids: vec![0, 1],
                encoded_indices: vec![0, 1, 0],
                strips: vec![],
                triangles: vec![[0, 1, 0]],
                vertices: vec![
                    Vertex {
                        source_index: 0,
                        bones: [255, index as u8, 64],
                        bone_count: 3,
                        weights: [
                            Float32(0x8000_0000),
                            Float32(0x3f00_0001),
                            Float32(0x3eff_fffe),
                        ],
                        position: vector,
                        normal: vector,
                        uv: [zero; 2],
                        tangent: [zero; 4],
                    },
                    Vertex {
                        source_index: 1,
                        bones: [0, 200, 199],
                        bone_count: 1,
                        weights: [Float32(1.0f32.to_bits()), zero, zero],
                        position: vector,
                        normal: vector,
                        uv: [zero; 2],
                        tangent: [zero; 4],
                    },
                ],
            })
            .collect();
        PresentationModel {
            identity: "models/palette.mdl".into(), profile: PresentationProfile::World,
            descriptor: PresentationDescriptor::World {
                geometry: GeometryOrientation {
                    positions: VertexAttributeTransform::AuthoredSourceValues,
                    normals: VertexAttributeTransform::AuthoredSourceValues,
                    tangents: VertexAttributeTransform::AuthoredSourceValues,
                    texture_coordinates: TextureCoordinateConvention::AuthoredUTowardRightVDown,
                    tangent_handedness: TangentHandednessConvention::TangentSWComponent,
                    deformation: VertexDeformationContract::FlexBeforeLinearBoneSkinningWithoutTopologyChanges,
                    facing: GeometryFacing { front_face: TriangleWinding::Clockwise, cull_face: CullFace::Back },
                }, entity_angles: EntityAngleConvention::DegreesPitchYawRollForwardLeftUpColumns,
                root_bone: RootBoneContract::AnimatedBelowEntity, depth_range: [zero, Float32(1.0f32.to_bits())],
            }, checksum: 0, flags: 0, basis: ModelBasis { forward: vector, left: vector, up: vector },
            collision_bounds: [vector; 2],
            dependencies: vec![], base_material_count: 2,
            materials: (0..2).map(|slot| PresentationMaterial { slot, source_slot: slot, lod: None,
                authored_name: vec![], material_dependency: 0, include_dependencies: vec![], textures: vec![] }).collect(),
            bones: vec![], animations: vec![], sequences: vec![], pose_parameters: vec![], attachments: vec![], hitbox_sets: vec![],
            skins: vec![SkinFamily { index: 0, texture_indices: vec![0, 1] }, SkinFamily { index: 1, texture_indices: vec![1, 0] }],
            body_parts: (0..2).map(|index| PresentationBodyPart { index, name: vec![], base: 1, model_names: vec![b"a".to_vec(), b"b".to_vec()] }).collect(),
            geometry, physics_status: PhysicsStatus::Missing, features: vec![],
        }
    }

    fn oracle(model: &PresentationModel, selected: &[SelectedPrimitive]) -> Vec<u8> {
        let mut bones = selected
            .iter()
            .flat_map(|selection| {
                model.geometry[selection.primitive]
                    .vertices
                    .iter()
                    .flat_map(|vertex| {
                        vertex.bones[..usize::from(vertex.bone_count)]
                            .iter()
                            .copied()
                    })
            })
            .collect::<Vec<_>>();
        bones.sort_unstable();
        bones.dedup();
        bones
    }

    fn assert_selection(model: &RetainedPresentationModel, selected: &[SelectedPrimitive]) {
        let expected = oracle(model, selected);
        let palette = model.selected_palette(selected, false).unwrap();
        assert_eq!(palette.len(), expected.len());
        assert_eq!(palette.iter().collect::<Vec<_>>(), expected);
        // Matrix bits are transported unchanged, not recomputed or normalized.
        let matrices: Vec<[u32; 12]> = (0..256)
            .map(|bone| {
                std::array::from_fn(|axis| {
                    [0x8000_0000, 0x7fc0_0123, 1, 0x3f80_0001][axis % 4] ^ bone
                })
            })
            .collect();
        let bytes = |bones: Vec<u8>| {
            bones
                .iter()
                .flat_map(|bone| {
                    matrices[usize::from(*bone)]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(bytes(palette.iter().collect()), bytes(expected));
        assert_eq!(model.selected_palette(selected, true).unwrap().len(), 0);
    }

    #[test]
    fn retained_palette_selections_preserve_authored_bits_and_never_rescan_vertices() {
        let source = fixture();
        let expected = source.clone();
        VERTEX_VISITS.with(|count| count.set(0));
        let model = RetainedPresentationModel::new(source);
        assert_eq!(VERTEX_VISITS.with(|count| count.get()), 24);
        let pointer = model.palettes.as_ptr();
        for _ in 0..100 {
            for first in 0..2 {
                for second in 0..2 {
                    for skin in 0..2 {
                        for lod in 0..4 {
                            let mut selected =
                                select_primitives(&model, &[first, second], skin, lod).unwrap();
                            assert_selection(&model, &selected);
                            selected.reverse();
                            selected.extend(selected.clone());
                            assert_selection(&model, &selected);
                        }
                    }
                }
            }
        }
        assert_eq!(
            *model, expected,
            "all authored attributes and selection inputs remain untouched"
        );
        assert_eq!(model.palettes.as_ptr(), pointer);
        assert_eq!(std::mem::size_of_val(&*model.palettes), 12 * 32);
        assert_eq!(VERTEX_VISITS.with(|count| count.get()), 24);
        let invalid = [SelectedPrimitive {
            primitive: 12,
            material: 0,
        }];
        assert!(model.selected_palette(&invalid, false).is_err());
        assert_eq!(model.selected_palette(&invalid, true).unwrap().len(), 0);
    }

    #[test]
    fn retained_palette_weak_cache_replacement_and_retirement_are_generation_local() {
        VERTEX_VISITS.with(|count| count.set(0));
        let old = Arc::new(RetainedPresentationModel::new(fixture()));
        let weak = Arc::downgrade(&old);
        let old_pointer = old.palettes.as_ptr();
        let shared = Arc::clone(&old);
        assert_eq!(shared.palettes.as_ptr(), old_pointer);
        let mut replacement = fixture();
        replacement.geometry[0].vertices[0].bones = [32, 33, 34];
        let replacement = Arc::new(RetainedPresentationModel::new(replacement));
        assert_eq!(old.identity, replacement.identity);
        assert_ne!(old.primitive_palette(0), replacement.primitive_palette(0));
        assert_selection(
            &replacement,
            &[SelectedPrimitive {
                primitive: 0,
                material: 0,
            }],
        );
        let cpu_owner = Arc::clone(old.source());
        drop(old);
        assert!(weak.upgrade().is_some());
        drop(shared);
        assert!(
            weak.upgrade().is_none(),
            "retired rendering metadata is not retained by CPU-only owners"
        );
        assert_eq!(cpu_owner.geometry[0].vertices[0].bones, [255, 0, 64]);
        assert_eq!(VERTEX_VISITS.with(|count| count.get()), 48);
    }

    #[test]
    #[ignore = "requires the exact configured TF2 gameplay resource graph"]
    fn configured_retained_palette_parity_and_cache_lifetime() {
        use super::super::{build_model_presentation, bundle};
        use sha2::{Digest, Sha256};
        let graph = std::env::var("PLAYSRC_PALETTE_GRAPH").expect("configured resource graph");
        let objects=std::path::PathBuf::from(std::env::var("PLAYSRC_RESOURCE_OBJECT_DIRECTORY").expect("configured graph object directory"));
        let bytes =
            playsrc_asset_graph::read_resource_set(std::path::Path::new(&graph), &objects, Some("gameplay"))
                .unwrap();
        let resources = bundle(&bytes).unwrap();
        let hashes = resources
            .iter()
            .map(|(path, bytes)| (path.clone(), <[u8; 32]>::from(Sha256::digest(bytes))))
            .collect();
        let mut models = 0;
        let mut primitives = 0;
        let mut vertices = 0;
        let mut selections = 0;
        for path in resources.keys().filter(|path| {
            path.ends_with(".mdl") && resources.contains_key(&path.replace(".mdl", ".vvd"))
        }) {
            let profile = if path.starts_with("models/weapons/") {
                PresentationProfile::ViewModel
            } else {
                PresentationProfile::World
            };
            let artifact = build_model_presentation(
                path,
                &resources,
                &hashes,
                playsrc_map::LightingProfile::Hdr,
                profile,
            )
            .unwrap_or_else(|_| panic!("build {path}"));
            let model = &artifact.model;
            let visits = VERTEX_VISITS.with(|count| count.get());
            let cached = build_model_presentation(
                path,
                &resources,
                &hashes,
                playsrc_map::LightingProfile::Hdr,
                profile,
            )
            .unwrap();
            assert!(Arc::ptr_eq(model, &cached.model));
            for primitive in 0..model.geometry.len() {
                assert_selection(
                    model,
                    &[SelectedPrimitive {
                        primitive,
                        material: 0,
                    }],
                );
            }
            let mut bodies = vec![vec![0; model.body_parts.len()]];
            for (part, definition) in model.body_parts.iter().enumerate() {
                for body in 1..definition.model_names.len() {
                    let mut selected = vec![0; model.body_parts.len()];
                    selected[part] = body;
                    bodies.push(selected);
                }
            }
            for body in bodies {
                for skin in 0..model.skins.len() {
                    for lod in 0..=model.geometry.iter().map(|p| p.lod).max().unwrap_or(0) {
                        let selected = select_primitives(model, &body, skin, lod).unwrap();
                        assert_selection(model, &selected);
                        selections += 1;
                    }
                }
            }
            assert_eq!(VERTEX_VISITS.with(|count| count.get()), visits);
            models += 1;
            primitives += model.geometry.len();
            vertices += model
                .geometry
                .iter()
                .map(|p| p.vertices.len())
                .sum::<usize>();
            let weak = Arc::downgrade(model);
            drop(cached);
            drop(artifact);
            assert!(weak.upgrade().is_none());
        }
        assert!(models >= 20, "configured roster must be present");
        eprintln!(
            "palette parity: {models} models, {primitives} primitives, {vertices} vertices, {selections} selections, {} retained metadata bytes; zero cache-hit/selection vertex visits",
            primitives * 32
        );
    }
}
