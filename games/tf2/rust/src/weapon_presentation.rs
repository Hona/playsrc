//! Source item activity overrides and weapon-role translation.

use crate::{
    Weapon,
    class::PlayerClass,
    equipment,
    schema::{ItemDefinition, SchemaValue},
};

fn scalar<'a>(definition: &'a ItemDefinition, name: &str) -> Option<&'a str> {
    definition.source.iter().find_map(|node| {
        if !node.key.eq_ignore_ascii_case(name) {
            return None;
        }
        match &node.value {
            SchemaValue::Scalar(value) => Some(value.as_str()),
            _ => None,
        }
    })
}

fn role(definition: &ItemDefinition, weapon: Weapon) -> &str {
    if let Some(slot) = scalar(definition, "anim_slot") {
        return slot;
    }
    match definition.item_class.as_str() {
        "tf_weapon_flaregun" => "ITEM1",
        "tf_weapon_flaregun_revenge" => "SECONDARY",
        _ => match weapon {
            Weapon::Bat
            | Weapon::Bottle
            | Weapon::FireAxe
            | Weapon::Kukri
            | Weapon::Knife
            | Weapon::Fists
            | Weapon::Shovel
            | Weapon::Wrench
            | Weapon::Bonesaw => "MELEE",
            Weapon::Shotgun
            | Weapon::HeavyShotgun
            | Weapon::Pistol
            | Weapon::EngineerPistol
            | Weapon::Revolver
            | Weapon::Smg
            | Weapon::StickybombLauncher
            | Weapon::MediGun => "SECONDARY",
            Weapon::BuildPda | Weapon::DestroyPda | Weapon::DisguiseKit => "PDA",
            Weapon::Toolbox | Weapon::Sapper | Weapon::InvisibilityWatch => "BUILDING",
            _ => "PRIMARY",
        },
    }
}

fn replacement<'a>(overrides: &'a [(&str, &str)], activity: &str) -> Option<&'a str> {
    overrides
        .iter()
        .find_map(|(base, replacement)| base.eq_ignore_ascii_case(activity).then_some(*replacement))
}

fn translate_viewmodel(role: &str, activity: &str, overrides: &[(&str, &str)]) -> String {
    // TranslateViewmodelHandActivityInternal checks item overrides before the
    // role table. In particular QRL and Manmelter activities must not acquire
    // another PRIMARY/ITEM1 prefix after replacement.
    if let Some(replacement) = replacement(overrides, activity) {
        return replacement.to_owned();
    }
    let role = role.to_ascii_uppercase();
    let mapped_role = if role == "PRIMARY2" { "PRIMARY" } else { &role };
    if !matches!(
        mapped_role,
        "PRIMARY" | "SECONDARY" | "MELEE" | "PDA" | "ITEM1" | "ITEM2" | "SECONDARY2"
    ) {
        return activity.to_owned();
    }
    let ordinary = matches!(
        activity,
        "ACT_VM_DRAW"
            | "ACT_VM_HOLSTER"
            | "ACT_VM_IDLE"
            | "ACT_VM_PULLBACK"
            | "ACT_VM_PRIMARYATTACK"
            | "ACT_VM_RELOAD"
            | "ACT_VM_DRYFIRE"
            | "ACT_VM_IDLE_TO_LOWERED"
            | "ACT_VM_IDLE_LOWERED"
            | "ACT_VM_LOWERED_TO_IDLE"
    );
    let secondary =
        activity == "ACT_VM_SECONDARYATTACK" && role != "SECONDARY2" && role != "PRIMARY2";
    let melee =
        matches!(activity, "ACT_VM_HITCENTER" | "ACT_VM_SWINGHARD") && mapped_role == "MELEE";
    let reload = matches!(activity, "ACT_RELOAD_START" | "ACT_RELOAD_FINISH")
        && matches!(
            mapped_role,
            "PRIMARY" | "SECONDARY" | "ITEM1" | "SECONDARY2"
        );
    if ordinary || secondary || melee || reload {
        let suffix = activity.strip_prefix("ACT_").expect("canonical activity");
        let mut result = format!("ACT_{mapped_role}_{suffix}");
        if role == "PRIMARY2"
            && matches!(
                activity,
                "ACT_VM_RELOAD" | "ACT_RELOAD_START" | "ACT_RELOAD_FINISH"
            )
        {
            result.push_str("_3");
        }
        result
    } else {
        activity.to_owned()
    }
}

pub fn viewmodel_activity(
    definition_index: u32,
    class: PlayerClass,
    activity: &str,
) -> Option<String> {
    let supported = *equipment::supported_item(definition_index)?;
    let definition = equipment::schema().definition(definition_index)?;
    if !definition.usable_by.contains(&class) {
        return None;
    }
    let weapon = supported.weapon_for_class(class)?;
    let presentation = equipment::presentation(definition_index)?;
    if !presentation.attach_to_hands {
        return Some(activity.to_owned());
    }
    Some(translate_viewmodel(
        role(definition, weapon),
        activity,
        presentation.animation_replacements,
    ))
}

pub fn world_activity(definition_index: u32, class: PlayerClass, activity: &str) -> Option<String> {
    let presentation = equipment::presentation(definition_index)?;
    let definition = equipment::schema().definition(definition_index)?;
    if !definition.usable_by.contains(&class) {
        return None;
    }
    let weapon = equipment::supported_item(definition_index)?.weapon_for_class(class)?;
    let role = role(definition, weapon).to_ascii_uppercase();
    let role = if role == "PRIMARY2" { "PRIMARY" } else { &role };
    let translated = if activity == "ACT_MP_STAND_IDLE" {
        format!("ACT_MP_STAND_{role}")
    } else {
        activity.to_owned()
    };
    // Player animation state applies the item override to the weapon's already
    // translated third-person activity, unlike the hand-viewmodel path.
    Some(
        replacement(presentation.animation_replacements, &translated)
            .unwrap_or(&translated)
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewmodel_override_precedes_role_and_primary2_only_changes_reload() {
        assert_eq!(
            translate_viewmodel(
                "PRIMARY",
                "ACT_VM_DRAW",
                &[("ACT_VM_DRAW", "ACT_VM_DRAW_QRL")]
            ),
            "ACT_VM_DRAW_QRL"
        );
        assert_eq!(
            translate_viewmodel(
                "ITEM1",
                "ACT_VM_PRIMARYATTACK",
                &[("ACT_VM_PRIMARYATTACK", "ACT_SECONDARY2_VM_PRIMARYATTACK")]
            ),
            "ACT_SECONDARY2_VM_PRIMARYATTACK"
        );
        assert_eq!(
            translate_viewmodel("PRIMARY2", "ACT_VM_DRAW", &[]),
            "ACT_PRIMARY_VM_DRAW"
        );
        assert_eq!(
            translate_viewmodel("PRIMARY2", "ACT_VM_RELOAD", &[]),
            "ACT_PRIMARY_VM_RELOAD_3"
        );
        assert_eq!(
            translate_viewmodel("PRIMARY2", "ACT_RELOAD_START", &[]),
            "ACT_PRIMARY_RELOAD_START_3"
        );
        assert_eq!(
            translate_viewmodel("ITEM1", "ACT_VM_DRAW", &[]),
            "ACT_ITEM1_VM_DRAW"
        );
        assert_eq!(
            translate_viewmodel("MELEE", "ACT_VM_SWINGHARD", &[]),
            "ACT_MELEE_VM_SWINGHARD"
        );
        assert_eq!(
            translate_viewmodel("BUILDING", "ACT_VM_DRAW", &[]),
            "ACT_VM_DRAW"
        );
    }

    #[test]
    fn configured_stock_items_resolve_through_the_single_equipment_schema() {
        assert_eq!(
            viewmodel_activity(18, PlayerClass::Soldier, "ACT_VM_PRIMARYATTACK").as_deref(),
            Some("ACT_PRIMARY_VM_PRIMARYATTACK")
        );
        assert_eq!(
            viewmodel_activity(0, PlayerClass::Scout, "ACT_VM_HITCENTER").as_deref(),
            Some("ACT_MELEE_VM_HITCENTER")
        );
        assert_eq!(
            viewmodel_activity(5, PlayerClass::Heavy, "ACT_VM_DRAW").as_deref(),
            Some("ACT_FISTS_VM_DRAW")
        );
        assert_eq!(
            viewmodel_activity(18, PlayerClass::Pyro, "ACT_VM_DRAW"),
            None
        );
        assert_eq!(
            viewmodel_activity(u32::MAX, PlayerClass::Scout, "ACT_VM_DRAW"),
            None
        );
        assert_eq!(
            world_activity(18, PlayerClass::Soldier, "ACT_MP_STAND_IDLE").as_deref(),
            Some("ACT_MP_STAND_PRIMARY")
        );
        assert_eq!(
            world_activity(0, PlayerClass::Scout, "ACT_MP_STAND_IDLE").as_deref(),
            Some("ACT_MP_STAND_MELEE")
        );
    }
}
