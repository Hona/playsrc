//! Offline diagnostics: production content resolution and VTF ranges, no GPU or browser.
use playsrc_content::{Content, Resolution};
use playsrc_vtf::{Decoder, Dialect, Limits, SamplingEnvironment, SubresourceIdentity, sampling_state};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn collect(content: &Content, cache: &Path, request: &Path) -> Result<Value, String> {
    let request = fs::canonicalize(request).map_err(|error| error.to_string())?;
    let cache = fs::canonicalize(cache).map_err(|error| error.to_string())?;
    if !request.starts_with(&cache) { return Err("owner request must be in sourceCacheDir".into()); }
    let bytes = fs::read(request).map_err(|error| error.to_string())?;
    if bytes.len() > 128 * 1024 { return Err("owner request exceeds bound".into()); }
    let paths: Vec<String> = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if paths.len() > 512 { return Err("owner input count exceeds bound".into()); }
    let directory = cache.join("evidence/tf2-browser-performance/texture-replacement/offline-inputs");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut records = Vec::new();
    let mut particle_definitions = Vec::new();
    for logical in paths {
        if logical.starts_with("particles/") && logical.ends_with(".pcf") && !logical.contains("..") {
            let Resolution::Found(source) = content.resolve_resource(&logical).map_err(|error| error.to_string())?
                else { return Err(format!("configured PCF unavailable: {logical}")); };
            let registry = playsrc_particle::Registry::from_pcf(&[playsrc_particle::PcfSource { logical_path: &logical, bytes: &source.bytes }], playsrc_particle::RegistryLimits::default()).map_err(|error| error.to_string())?;
            for definition in registry.definitions() {
                if definition.material.is_empty() { continue; }
                let material_path = super::material_path(definition.material.as_bytes())?;
                let Resolution::Found(vmt) = content.resolve_resource(&material_path).map_err(|error| error.to_string())?
                    else { return Err(format!("configured particle VMT unavailable: {material_path}")); };
                let mut responses = Vec::new();
                let material = loop {
                    match playsrc_vmt::compose(&vmt.bytes, material_path.clone(), &responses, &playsrc_keyvalues::ConditionEnvironment::default(), playsrc_vmt::Limits::default()).map_err(|error| error.to_string())? {
                        playsrc_vmt::Composition::Complete(document) => break playsrc_material::resolve_for_environment(&document,
                            playsrc_material::SelectionEnvironment { sprite_card_default_depth_blend: Some(true), ..Default::default() }).map_err(|error| error.to_string())?,
                        playsrc_vmt::Composition::Needs(requests) => for request in requests {
                            let path = super::material_path(&request.target_token)?;
                            let Resolution::Found(value) = content.resolve_resource(&path).map_err(|error| error.to_string())?
                                else { return Err(format!("configured VMT dependency unavailable: {path}")); };
                            responses.push(playsrc_vmt::DependencyResponse { parent_identity: request.parent_identity, target_token: request.target_token, canonical_identity: path, bytes: Some(value.bytes) });
                        },
                    }
                };
                particle_definitions.push(json!({"pcf":logical,"pcfSha256":source.provenance.sha256,"definition":definition.name,"material":material_path,"materialSha256":vmt.provenance.sha256,
                    "children":definition.children.iter().map(|child|json!({"definition":child.definition_name,"delaySeconds":child.delay_seconds})).collect::<Vec<_>>(),
                    "renderers":definition.functions.iter().filter(|function|function.category==playsrc_particle::FunctionCategory::Renderer).map(|function|function.identity.clone()).collect::<Vec<_>>(),
                    "shader":format!("{:?}",material.shader),"textures":material.textures.iter().filter(|texture|material.selected_textures.contains(&texture.role)).map(|texture|json!({"role":format!("{:?}",texture.role),"path":texture.logical_path,"colorRead":format!("{:?}",texture.color_read)})).collect::<Vec<_>>() }));
            }
            continue;
        }
        if !logical.starts_with("materials/") || !logical.ends_with(".vtf") || logical.contains("..") { return Err("owner input must name a material VTF".into()); }
        let Resolution::Found(source) = content.resolve_resource(&logical).map_err(|error| error.to_string())?
            else { return Err(format!("configured owner input unavailable: {logical}")); };
        let decoder = Decoder::new(&source.bytes, Dialect::Source2013Pc, Limits::default()).map_err(|error| error.to_string())?;
        let metadata = decoder.metadata();
        let sampling = sampling_state(metadata, SamplingEnvironment { shader_model: 90, force_anisotropy: 1, maximum_anisotropy: 1, force_trilinear: false });
        let cube = metadata.faces.len() > 1;
        let mut decoded = Vec::new();
        let mut planes = Vec::new();
        for plane in &metadata.subresources {
            if let SubresourceIdentity::HighResolution { mip, frame, face, slice } = plane.identity {
                let (offset, length) = if cube {
                    let value = decoder.decode(plane.identity).map_err(|error| error.to_string())?;
                    if value.channel_layout != playsrc_vtf::ChannelLayout::Rgba { return Err("cube evidence requires RGBA samples".into()); }
                    let offset = decoded.len(); decoded.extend_from_slice(&value.samples); (offset, value.samples.len())
                } else { (plane.encoded_range.start, plane.encoded_range.len()) };
                planes.push(json!({"mip":mip,"frame":frame,"face":face as u8,"slice":slice,"width":plane.width,"height":plane.height,"offset":offset,"length":length}));
            }
        }
        let data = if cube { decoded.as_slice() } else { source.bytes.as_slice() };
        let data_hash = super::digest(data);
        let data_path = directory.join(format!("{data_hash}.bin"));
        fs::write(&data_path, data).map_err(|error| error.to_string())?;
        records.push(json!({"logicalPath":logical,"sourceSha256":source.provenance.sha256,"sourceBytes":source.bytes.len(),"provider":source.provenance.provider_id,
            "dataPath":data_path,"dataSha256":data_hash,"dataBytes":data.len(),"width":metadata.width,"height":metadata.height,"depth":metadata.depth,
            "frameCount":metadata.frame_count,"mipCount":metadata.mip_count,"sourceFormat":if cube {None} else {Some(metadata.high_format.code())},
            "scalarEncoding":if metadata.high_format.code()==24 {"f16"} else {"u8"},"faces":metadata.faces.iter().map(|face|*face as u8).collect::<Vec<_>>(),"planes":planes,
            "sampling":{"wrapS":sampling.wrap_s as u8,"wrapT":sampling.wrap_t as u8,"wrapU":sampling.wrap_u as u8,"minFilter":sampling.min_filter as u8,
                "magFilter":sampling.mag_filter as u8,"mipmapped":sampling.mipmapped,"noLod":sampling.no_lod,"allMips":sampling.all_mips,"anisotropyLevel":1}}));
    }
    Ok(json!({"schema":"playsrc-offline-texture-owner-inputs-v1","records":records,"particleDefinitions":particle_definitions}))
}
