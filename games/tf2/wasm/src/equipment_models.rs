//! Bounded incremental model admission. A model-panel cache has no map or player.
use super::*;

#[derive(Default)]
struct Panel {
    models: BTreeMap<String, Arc<playsrc_studio_model::PresentationModel>>,
    metadata: BTreeMap<String, StudioModelLightingMetadata>,
    opacity: BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>,
    scenes: BTreeMap<u32, ClassPreview>,
    bob: BTreeMap<u32, playsrc_studio_model::ViewModelBobState>,
    output: Vec<u8>,
    admission: Vec<u8>,
    particles: Option<playsrc_particle::ParticleWorld>,
    particle_materials: Vec<String>,
    particle_sheets: BTreeMap<String, playsrc_particle::ParticleMaterial>,
    wearable_particles: wearable::ParticleStates,
}

fn panel() -> &'static std::sync::Mutex<Panel> {
    static PANEL: std::sync::OnceLock<std::sync::Mutex<Panel>> = std::sync::OnceLock::new();
    PANEL.get_or_init(Default::default)
}

pub(super) fn roots(definitions: &[u32]) -> Result<BTreeMap<String, playsrc_studio_model::PresentationProfile>, ()> {
    use playsrc_studio_model::PresentationProfile::{ViewModel, World};
    if definitions.is_empty() || definitions.len() > 32 { return Err(()); }
    let mut roots = BTreeMap::new();
    for definition in definitions {
        let item = playsrc_tf2::equipment::supported_item(*definition).ok_or(())?;
        let presentation = playsrc_tf2::equipment::presentation(*definition).ok_or(())?;
        if !presentation.model_player.is_empty() {
            roots.insert(presentation.model_player.to_owned(), if matches!(item.implementation, playsrc_tf2::equipment::Implementation::Weapon(_)) { ViewModel } else { World });
        }
        for (class, _) in presentation.class_slots {
            if let Some(model) = presentation.model_for_class(*class) {
                roots.insert(model.to_owned(), if matches!(item.implementation, playsrc_tf2::equipment::Implementation::Weapon(_)) { ViewModel } else { World });
            }
            roots.insert(class.data().model.to_owned(), World);
            roots.insert(class.data().hand_model.to_owned(), ViewModel);
        }
    }
    Ok(roots)
}

#[unsafe(no_mangle)]
/// # Safety
/// The definitions and resource sections must reference readable module memory.
pub unsafe extern "C" fn playsrc_equipment_models_admit(handle: u32, definitions: *const u32, count: usize,
    sections: *const ResourceSection, section_count: usize, profile: u32) -> u32 {
    if definitions.is_null() || !(1..=32).contains(&count) { return 0; }
    let result = (|| {
        let definitions = unsafe { std::slice::from_raw_parts(definitions, count) };
        let roots = roots(definitions)?;
        let bundle = unsafe { resource_sections(sections, section_count) }?;
        let profile = match profile { 0 => playsrc_map::LightingProfile::Ldr, 1 => playsrc_map::LightingProfile::Hdr, _ => return Err(()) };
        let resource_hashes = bundle.iter().map(|(path, bytes)| (path.clone(), Sha256::digest(bytes).into())).collect();
        let decoders = TextureDecoders::new(&bundle);
        let models = roots.iter().map(|(path, kind)| build_model_presentation(path, &bundle, &resource_hashes, profile, *kind).map(|model| (path.clone(), model))).collect::<Result<Vec<_>, _>>()?;
        let materials = prepare_model_materials(&models, &bundle, &decoders, &resource_hashes, profile)?;
        let opacity = model_material_opacity(&models, &bundle, &decoders, profile, Some(&materials))?;
        let particles = if handle == 0 && definitions.iter().any(|definition| playsrc_tf2::equipment::supported_item(*definition)
            .is_some_and(|item| item.attributes.iter().any(|attribute| attribute.definition == 134 && attribute.value == 13.0))) {
            Some(compile_cosmetic_particles(&bundle, &decoders)?)
        } else { None };
        let mut out = b"PEQM\x02\0\0\0".to_vec();
        out.extend_from_slice(&(models.len() as u32).to_le_bytes());
        encode_headers(&mut out, &models)?;
        out.extend_from_slice(b"PMST\x02\0\0\0");
        out.extend_from_slice(&((materials.materials.len() + particles.as_ref().map_or(0, |value| value.3.len())) as u32).to_le_bytes());
        for (identity, material) in &materials.materials { encode_one_material_state(&mut out, identity, material, &decoders)?; }
        if let Some((_, _, _, presentation)) = &particles {
            for (identity, material) in presentation { encode_resolved_material_state(&mut out, identity, &material.state, Some(&material.metadata))?; }
        }
        encode_model_materials(&mut out, &materials)?;
        let studio_models = models.iter().map(|(path, artifact)| (path.clone(), Arc::clone(&artifact.model))).collect();
        let (geometry, _) = resolve_models(None, &studio_models, &bundle, &decoders, profile, None)?;
        let mut registry = Vec::new();
        playsrc_map::serialize_model_registry(&mut registry, &geometry, true);
        pbytes(&mut out, &registry)?;
        out.extend_from_slice(&(particles.as_ref().map_or(0, |value| value.1.len()) as u32).to_le_bytes());
        if let Some((_, identities, _, presentation)) = &particles {
            for identity in identities { pbytes(&mut out, identity.as_bytes())?; }
            encode_particle_textures(&mut out, presentation)?;
        } else { encode_particle_textures(&mut out, &BTreeMap::new())?; }
        if out.len() > 64 * 1024 * 1024 { return Err(()); }
        let metadata = models.iter().map(|(path, artifact)| (path.clone(), StudioModelLightingMetadata {
            flex: Arc::clone(&artifact.flex), position: artifact.illumination_position,
            attachment: artifact.illumination_attachment, eyes: artifact.eyes.clone(),
        }));
        if handle == 0 {
            let mut panel = panel().lock().unwrap();
            // A panel owns only its latest admitted selection, not every item
            // visited in the backpack. Shared compiled models remain weak cached.
            panel.metadata = metadata.collect();
            panel.models = models.into_iter().map(|(path, artifact)| (path, artifact.model)).collect();
            panel.opacity = opacity;
            panel.scenes.clear();
            panel.wearable_particles = wearable::ParticleStates::default();
            if let Some((world, identities, sheets, _)) = particles {
                panel.particles = Some(world); panel.particle_materials = identities; panel.particle_sheets = sheets;
            } else { panel.particles = None; panel.particle_materials.clear(); panel.particle_sheets.clear(); }
            panel.admission = out;
        } else {
            let (index, generation) = decode(handle).ok_or(())?;
            let mut slots = slots().lock().unwrap();
            let slot = slots.get_mut(index).filter(|slot| slot.generation == generation).ok_or(())?;
            slot.model_lighting_metadata.extend(metadata);
            slot.studio_models.extend(models.into_iter().map(|(path, artifact)| (path, artifact.model)));
            slot.model_material_opacity.extend(opacity);
            panel().lock().unwrap().admission = out;
        }
        Ok::<_, ()>(())
    })();
    u32::from(result.is_ok())
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_equipment_models_length() -> usize { panel().lock().unwrap().admission.len() }

#[unsafe(no_mangle)]
/// # Safety
/// Output must reference `capacity` writable bytes in module memory.
pub unsafe extern "C" fn playsrc_equipment_models_copy(output: *mut u8, capacity: usize) -> usize {
    let panel = panel().lock().unwrap();
    if output.is_null() || capacity < panel.admission.len() { return 0; }
    unsafe { std::ptr::copy_nonoverlapping(panel.admission.as_ptr(), output, panel.admission.len()) };
    panel.admission.len()
}

pub(super) fn transact(requests: &[ModelPoseRequest]) -> Result<(), ()> {
    if requests.iter().any(|request| !(request.model_panel || request.class_selection || request.world_item) || request.cloak.is_some()) { return Err(()); }
    let requests: Vec<_> = requests.iter().cloned().map(|mut request| { request.model_panel = true; request }).collect();
    let mut panel = panel().lock().unwrap();
    let mut bob = panel.bob.clone();
    let mut wearable_particles = panel.wearable_particles.clone();
    if requests.is_empty() { wearable_particles = wearable::ParticleStates::default(); } else { wearable_particles.retain(&requests); }
    let Panel { models, metadata, opacity, scenes, output, particles, particle_materials, particle_sheets, .. } = &mut *panel;
    let mut world = ModelPoseWorld { metadata, lighting: None, visibility: None, collision: None, snapshot: None, gameplay: None, cubemaps: &[],
        particle_inputs: particles.as_ref().map(|template| wearable::ParticleInputs { template, materials: particle_sheets, identities: particle_materials }), wearable_particles: &mut wearable_particles };
    *output = encode_model_poses(models, opacity, &mut bob, scenes, &requests, &mut world, std::mem::take(output))?;
    panel.bob = bob; panel.wearable_particles = wearable_particles;
    Ok(())
}

pub(super) fn output_length() -> usize { panel().lock().unwrap().output.len() }
pub(super) fn output_capacity() -> usize { panel().lock().unwrap().output.capacity() }
pub(super) fn output_take() -> *mut u8 {
    let mut bytes = std::mem::take(&mut panel().lock().unwrap().output);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}
pub(super) fn recycle(bytes: Vec<u8>) { let mut panel = panel().lock().unwrap(); if panel.output.is_empty() && panel.output.capacity() < bytes.capacity() { panel.output = bytes; } }
pub(super) unsafe fn copy_output(pointer: *mut u8, capacity: usize) -> usize {
    let panel = panel().lock().unwrap();
    if pointer.is_null() || capacity < panel.output.len() { return 0; }
    unsafe { std::ptr::copy_nonoverlapping(panel.output.as_ptr(), pointer, panel.output.len()) };
    panel.output.len()
}

pub(super) fn encode_headers(out: &mut Vec<u8>, models: &[(String, Box<CompiledPresentationModel>)]) -> Result<(), ()> {
    use playsrc_studio_model as studio;
    for (id, artifact) in models {
        let model = &artifact.model;
        pbytes(out, id.as_bytes())?;
        out.extend_from_slice(&[u8::from(model.profile == studio::PresentationProfile::ViewModel), 0, 0, 0]);
        out.extend_from_slice(&artifact.identity);
        out.extend_from_slice(&(model.skins.len() as u32).to_le_bytes());
        out.extend_from_slice(&(model.body_parts.len() as u32).to_le_bytes());
        for part in &model.body_parts { out.extend_from_slice(&(part.model_names.len() as u32).to_le_bytes()); }
        let parameters = vec![studio::Float32(0); model.pose_parameters.len()];
        let pose = studio::sample_pose(model, &studio::AnimationState { base_sequence: 0, cycle: studio::Float32(0), pose_parameters: parameters.clone(), layers: Vec::new() }).map_err(|_| ())?;
        out.extend_from_slice(&(model.attachments.len() as u32).to_le_bytes());
        for attachment in &model.attachments {
            pbytes(out, &attachment.name)?;
            let sampled = pose.attachments.iter().find(|sample| sample.index == attachment.index).ok_or(())?;
            for value in sampled.model_transform.0 { out.extend_from_slice(&value.0.to_le_bytes()); }
        }
        out.extend_from_slice(&(model.sequences.len() as u32).to_le_bytes());
        for sequence in &model.sequences {
            pbytes(out, &sequence.label)?;
            pbytes(out, &sequence.activity_name)?;
            out.extend_from_slice(&(sequence.index as u32).to_le_bytes());
            let timing = studio::sequence_timing(model, sequence.index, &parameters).ok();
            out.extend_from_slice(&[u8::from(timing.is_some()), 0, 0, 0]);
            if let Some(timing) = timing {
                for value in [timing.frames_per_second, timing.weighted_frame_count, timing.cycles_per_second, timing.duration_seconds] { out.extend_from_slice(&value.0.to_le_bytes()); }
                out.extend_from_slice(&[u8::from(timing.looping), 0, 0, 0]);
            } else { out.extend_from_slice(&[0; 20]); }
        }
        match model.descriptor {
            studio::PresentationDescriptor::World { geometry, root_bone, depth_range, .. } => {
                out.extend_from_slice(&[0, u8::from(root_bone == studio::RootBoneContract::StaticPropBoneZeroIsEntity), u8::from(geometry.facing.front_face == studio::TriangleWinding::CounterClockwise), 0]);
                for value in depth_range { out.extend_from_slice(&value.0.to_le_bytes()); }
                out.extend_from_slice(&[0; 32]);
            }
            studio::PresentationDescriptor::ViewModel { geometry, default_horizontal_fov_4_by_3, minimum_fov, maximum_fov, near_plane, depth_range, draws_after_world, opaque_before_translucent, .. } => {
                out.extend_from_slice(&[1, 0, u8::from(geometry.facing.front_face == studio::TriangleWinding::CounterClockwise), 0]);
                for value in [default_horizontal_fov_4_by_3, minimum_fov, maximum_fov, near_plane].into_iter().chain(depth_range) { out.extend_from_slice(&value.0.to_le_bytes()); }
                out.extend_from_slice(&[u8::from(draws_after_world), u8::from(opaque_before_translucent), 0, 0]);
                out.extend_from_slice(&[0; 12]);
            }
        }
        pbytes(out, &[])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_model_roots_are_bounded_deduplicated_and_class_complete() {
        assert!(roots(&[]).is_err());
        assert!(roots(&[u32::MAX]).is_err());
        assert!(roots(&[0; 33]).is_err());
        assert_eq!(roots(&[0]).unwrap(), roots(&[0, 0]).unwrap());
        let scout = roots(&[0]).unwrap();
        assert_eq!(scout.len(), 3);
        assert!(scout.contains_key("models/player/scout.mdl"));
        assert!(scout.contains_key("models/weapons/c_models/c_scout_arms.mdl"));
        assert!(scout.contains_key("models/weapons/c_models/c_bat.mdl"));
        assert_eq!(roots(&[5]).unwrap().len(), 2, "Fists have no invented attachment model");
        let captain = roots(&[378]).unwrap();
        for class in ["soldier", "medic", "heavy"] {
            assert!(captain.contains_key(&format!("models/player/items/{class}/{class}_officer.mdl")));
        }
    }

    #[test]
    fn malformed_admission_does_not_install_a_panel_or_replace_publication() {
        let before = playsrc_equipment_models_length();
        assert_eq!(unsafe { playsrc_equipment_models_admit(0, std::ptr::null(), 1, std::ptr::null(), 0, 1) }, 0);
        assert_eq!(unsafe { playsrc_equipment_models_admit(0, &u32::MAX, 1, std::ptr::null(), 0, 1) }, 0);
        assert_eq!(playsrc_equipment_models_length(), before);
    }
}
