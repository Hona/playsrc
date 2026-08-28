//! Material references used by Source's legacy map visual entities.
use std::collections::BTreeSet;
use crate::{Entity, Graph};

/// Non-video sprite model names resolve to VMTs. Explicit VMT paths keep their
/// directory; legacy SPR names select the basename in the sprites directory.
pub fn sprite_material(model: &str) -> Option<String> {
    if model.is_empty() { return None; }
    let model = model.replace('\\', "/").to_ascii_lowercase();
    let explicit = model.ends_with(".vmt") && model.contains('/');
    let name = if explicit { model.strip_prefix("materials/").unwrap_or(&model) }
        else { model.rsplit('/').next().unwrap_or(&model) };
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    Some(if explicit { format!("materials/{stem}.vmt") } else { format!("materials/sprites/{stem}.vmt") })
}

#[derive(Default)]
pub struct References {
    pub materials: BTreeSet<String>,
    pub smoke_series: BTreeSet<String>,
    pub optional_materials: BTreeSet<String>,
}

pub fn sun_materials(entity:&Entity)->Result<[String;2],std::str::Utf8Error>{
    let mut result=[String::new(),String::new()];
    for (index,key) in [b"material".as_slice(),b"overlaymaterial"].into_iter().enumerate(){
        let mut model=text(entity,key)?.to_owned();
        if model.is_empty(){model="sprites/light_glow02_add_noz.vmt".into();}
        else if !model.rsplit(['/', '\\']).next().unwrap_or_default().contains('.') {model.push_str(".vmt");}
        result[index]=sprite_material(&model).expect("nonempty sun material");
    }
    Ok(result)
}

pub fn from_graph(graph: &Graph) -> Result<References, std::str::Utf8Error> {
    from_entities(&graph.entities)
}

pub fn from_entities<'a>(entities: impl IntoIterator<Item = &'a Entity>) -> Result<References, std::str::Utf8Error> {
    let mut output = References::default();
    for entity in entities {
        let class = entity.classname.as_deref().unwrap_or_default();
        if class.eq_ignore_ascii_case(b"env_sprite") || class.eq_ignore_ascii_case(b"env_sprite_oriented") || class.eq_ignore_ascii_case(b"env_glow") {
            if let Some(path) = sprite_material(text(entity, b"model")?) { output.materials.insert(path); }
        } else if class.eq_ignore_ascii_case(b"env_lightglow") {
            output.materials.insert("materials/sprites/light_glow02_add_noz.vmt".into());
        } else if class.eq_ignore_ascii_case(b"point_spotlight") {
            output.materials.extend(["materials/sprites/light_glow03.vmt".into(), "materials/sprites/glow_test02.vmt".into()]);
        } else if class.eq_ignore_ascii_case(b"env_sun") {
            output.materials.extend(sun_materials(entity)?);
        } else if crate::rope::is_rope(class) {
            let material=crate::rope::material(entity)?;
            output.optional_materials.insert(format!("{}_back.vmt",material.strip_suffix(".vmt").expect("rope material")));
            output.materials.insert(material);
        } else if class.eq_ignore_ascii_case(b"env_smokestack") {
            let mut model = text(entity, b"SmokeMaterial")?.to_owned();
            let explicit = !model.is_empty();
            if model.is_empty() { model = "particle/SmokeStack.vmt".into(); }
            else if !model.to_ascii_lowercase().contains(".vmt") { model.push_str(".vmt"); }
            if let Some(path) = sprite_material(&model) {
                if explicit && let Some(stem) = path.strip_suffix(".vmt") {
                    if let Some((last, _)) = stem.char_indices().last() {
                        output.smoke_series.insert(stem[..last].to_owned());
                    }
                }
                output.materials.insert(path);
            }
        }
    }
    Ok(output)
}

fn text<'a>(entity: &'a Entity, key: &[u8]) -> Result<&'a str, std::str::Utf8Error> {
    std::str::from_utf8(entity.pairs.iter().find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map_or(&[], |pair| pair.value.as_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sprite_model_names_follow_explicit_vmt_and_legacy_spr_resolution() {
        assert_eq!(sprite_material("materials\\Sprites/light_glow03.vmt").as_deref(), Some("materials/sprites/light_glow03.vmt"));
        assert_eq!(sprite_material("sprites/glow01.spr").as_deref(), Some("materials/sprites/glow01.vmt"));
        assert_eq!(sprite_material("old/subdir/glow02.spr").as_deref(), Some("materials/sprites/glow02.vmt"));
        assert_eq!(sprite_material("glow01.vmt").as_deref(), Some("materials/sprites/glow01.vmt"));
        assert_eq!(sprite_material(""), None);
    }
}
