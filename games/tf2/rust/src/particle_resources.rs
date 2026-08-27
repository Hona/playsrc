//! Game precaches and map-authored particle roots share one dependency contract.
pub const GAME_FILES: &[&str] = &[
    "particles/rockettrail.pcf", "particles/rocketbackblast.pcf", "particles/stickybomb.pcf",
    "particles/muzzle_flash.pcf", "particles/explosion.pcf", "particles/flamethrower.pcf",
    "particles/nailtrails.pcf", "particles/medicgun_beam.pcf", "particles/blood_impact.pcf",
    "particles/bullet_tracers.pcf", "particles/impact_fx.pcf", "particles/crit.pcf",
    "particles/item_fx.pcf",
];

pub const GAME_SYSTEMS: &[&str] = &[
    "rockettrail", "rocketbackblast", "stickybombtrail_red", "stickybombtrail_blue",
    "stickybomb_pulse_red", "stickybomb_pulse_blue", "muzzle_pipelauncher", "muzzle_scattergun",
    "muzzle_pistol", "muzzle_shotgun", "blood_impact_red_01", "water_blood_impact_red_01",
    "blood_spray_red_01", "blood_spray_red_01_far", "bullet_scattergun_tracer01_red",
    "bullet_scattergun_tracer01_blue", "bullet_scattergun_tracer01_red_crit",
    "bullet_scattergun_tracer01_blue_crit", "bullet_pistol_tracer01_red",
    "bullet_pistol_tracer01_blue", "bullet_pistol_tracer01_red_crit", "bullet_pistol_tracer01_blue_crit",
    "bullet_shotgun_tracer01_red", "bullet_shotgun_tracer01_blue", "bullet_shotgun_tracer01_red_crit",
    "bullet_shotgun_tracer01_blue_crit", "bullet_tracer01_red", "bullet_tracer01_blue",
    "bullet_tracer01_red_crit", "bullet_tracer01_blue_crit", "impact_concrete", "impact_wood",
    "impact_metal", "impact_dirt", "impact_glass", "crit_text", "minicrit_text", "mark_for_death", "muzzle_revolver",
    "ExplosionCore_Wall", "ExplosionCore_MidAir", "new_flame", "new_flame_crit_red",
    "new_flame_crit_blue", "flamethrower_underwater", "pyro_blast", "nailtrails_medic_red",
    "nailtrails_medic_blue", "muzzle_syringe", "medicgun_beam_red", "medicgun_beam_blue",
    "medicgun_beam_red_invun", "medicgun_beam_blue_invun",
    "superrare_burning1",
];

pub const SOURCE_LIST: &str = "derived/particle-sources.txt";

pub fn roots(graph: &playsrc_entity::Graph) -> Vec<&str> {
    let mut roots = GAME_SYSTEMS.to_vec();
    for entity in &graph.entities {
        if entity.classname.as_deref().is_some_and(|class| class.eq_ignore_ascii_case(b"info_particle_system"))
            && let Some(name) = entity.pairs.iter().find(|pair| pair.key.eq_ignore_ascii_case(b"effect_name"))
            && let Ok(name) = std::str::from_utf8(&name.value)
            && !name.is_empty()
        {
            roots.push(name);
        }
    }
    roots
}

/// Source's manifest order is significant: later name definitions replace earlier ones.
pub fn manifest_files(bytes: &[u8]) -> Result<Vec<String>, String> {
    let doc = playsrc_keyvalues::parse_text(bytes, playsrc_keyvalues::EscapeMode::LiteralBackslash,
        playsrc_keyvalues::Limits::default()).map_err(|error| error.to_string())?;
    let Some(root) = doc.roots.first() else { return Err("empty particle manifest".into()); };
    let playsrc_keyvalues::Value::Object(entries) = &root.value else { return Err("invalid particle manifest".into()); };
    entries.iter().filter(|entry| entry.key.bytes.eq_ignore_ascii_case(b"file")).map(|entry| {
        let playsrc_keyvalues::Value::Scalar(value) = &entry.value else { return Err("invalid particle file".into()); };
        let text = std::str::from_utf8(&value.token.bytes).map_err(|_| "non-UTF8 particle path")?;
        Ok(text.trim_start_matches('!').replace('\\', "/").to_ascii_lowercase())
    }).collect()
}
