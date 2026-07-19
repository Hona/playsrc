import type { Tf2Class, Tf2PlayableTeam } from "./contract"

export const TF2_HUD_RESOURCES = Object.freeze({
  layout: "scripts/HudLayout.res",
  animations: "scripts/HudAnimations_tf.txt",
  clientScheme: "resource/ClientScheme.res",
  playerHealth: "resource/UI/HudPlayerHealth.res",
  playerClass: "resource/UI/HudPlayerClass.res",
  weaponAmmo: "resource/UI/HudAmmoWeapons.res",
  weaponSelection: "resource/UI/HudWeaponSelection.res",
  scoreboard: "resource/UI/Scoreboard.res",
  freezePanel: "resource/UI/FreezePanel_Basic.res",
})

export const TF2_HUD_RESOURCE_REVISIONS = Object.freeze({
  layout: "1f18cb73d9ef54ff79ea208c9996db0655ac731b2ee8e9a82ff63a4b697f400f",
  animations: "cffca69ab872c358d8afb566e78acde022062e87b4fdcfa1779e77dd28ad52ff",
  clientScheme: "1d071b99def0405cbf73d97642a396e6dcbad1a7488f12696ca5dd62893c604c",
  playerHealth: "31fabca97c196eb2cff565b18ff8b3a17aa8806c8e1a980c9376782be7fa4774",
  playerClass: "10181165d10a81821672fd8e104d798e18cf896ca1156cf92df8ce0a07f8c89d",
  weaponAmmo: "a23a98f009dd34ac8c94e7149b1ded56eb9ed66e03d583fcd9c2ab68c3cb7734",
  weaponSelection: "7a6f02c7eab4f0befdac5c69082c9334b0a03975738e1fc6d598ba6c91967138",
  scoreboard: "ed6e7d1619dcfa7423d00cf77c19026d7fe00c6a2bca634bb742d20d870b3e52",
  freezePanel: "aa7eb65149c32af23766daccee2fa6cb45a88ca09e4875008bdf692671f6a15e",
})

export const TF2_CLASS_IMAGES: Readonly<Record<Tf2PlayableTeam, Readonly<Record<Tf2Class, string>>>> =
  Object.freeze({
    2: Object.freeze({
      1: "../hud/class_scoutred",
      2: "../hud/class_sniperred",
      3: "../hud/class_soldierred",
      4: "../hud/class_demored",
      5: "../hud/class_medicred",
      6: "../hud/class_heavyred",
      7: "../hud/class_pyrored",
      8: "../hud/class_spyred",
      9: "../hud/class_engired",
    }),
    3: Object.freeze({
      1: "../hud/class_scoutblue",
      2: "../hud/class_sniperblue",
      3: "../hud/class_soldierblue",
      4: "../hud/class_demoblue",
      5: "../hud/class_medicblue",
      6: "../hud/class_heavyblue",
      7: "../hud/class_pyroblue",
      8: "../hud/class_spyblue",
      9: "../hud/class_engiblue",
    }),
  })

export type Tf2ConditionPanel = Readonly<{
  condition: number
  panel: string
  group: string
  blueImage: string
  redImage: string
}>

export const TF2_GROUPED_CONDITION_PANELS: readonly Tf2ConditionPanel[] = Object.freeze([
  Object.freeze({ condition: 58, panel: "PlayerStatus_MedicUberBulletResistImage", group: "bullet-resist", blueImage: "../HUD/defense_buff_bullet_blue", redImage: "../HUD/defense_buff_bullet_red" }),
  Object.freeze({ condition: 59, panel: "PlayerStatus_MedicUberBlastResistImage", group: "blast-resist", blueImage: "../HUD/defense_buff_explosion_blue", redImage: "../HUD/defense_buff_explosion_red" }),
  Object.freeze({ condition: 60, panel: "PlayerStatus_MedicUberFireResistImage", group: "fire-resist", blueImage: "../HUD/defense_buff_fire_blue", redImage: "../HUD/defense_buff_fire_red" }),
  Object.freeze({ condition: 61, panel: "PlayerStatus_MedicSmallBulletResistImage", group: "bullet-resist", blueImage: "../HUD/defense_buff_bullet_blue", redImage: "../HUD/defense_buff_bullet_red" }),
  Object.freeze({ condition: 62, panel: "PlayerStatus_MedicSmallBlastResistImage", group: "blast-resist", blueImage: "../HUD/defense_buff_explosion_blue", redImage: "../HUD/defense_buff_explosion_red" }),
  Object.freeze({ condition: 63, panel: "PlayerStatus_MedicSmallFireResistImage", group: "fire-resist", blueImage: "../HUD/defense_buff_fire_blue", redImage: "../HUD/defense_buff_fire_red" }),
  Object.freeze({ condition: 16, panel: "PlayerStatus_SoldierOffenseBuff", group: "soldier-offense", blueImage: "../Effects/soldier_buff_offense_blue", redImage: "../Effects/soldier_buff_offense_red" }),
  Object.freeze({ condition: 26, panel: "PlayerStatus_SoldierDefenseBuff", group: "soldier-defense", blueImage: "../Effects/soldier_buff_defense_blue", redImage: "../Effects/soldier_buff_defense_red" }),
  Object.freeze({ condition: 29, panel: "PlayerStatus_SoldierHealOnHitBuff", group: "soldier-heal", blueImage: "../Effects/soldier_buff_healonhit_blue", redImage: "../Effects/soldier_buff_healonhit_red" }),
  Object.freeze({ condition: 90, panel: "PlayerStatus_RuneStrength", group: "rune-strength", blueImage: "../Effects/powerup_strength_hud", redImage: "../Effects/powerup_strength_hud" }),
  Object.freeze({ condition: 91, panel: "PlayerStatus_RuneHaste", group: "rune-haste", blueImage: "../Effects/powerup_haste_hud", redImage: "../Effects/powerup_haste_hud" }),
  Object.freeze({ condition: 92, panel: "PlayerStatus_RuneRegen", group: "rune-regen", blueImage: "../Effects/powerup_regen_hud", redImage: "../Effects/powerup_regen_hud" }),
  Object.freeze({ condition: 93, panel: "PlayerStatus_RuneResist", group: "rune-resist", blueImage: "../Effects/powerup_resist_hud", redImage: "../Effects/powerup_resist_hud" }),
  Object.freeze({ condition: 94, panel: "PlayerStatus_RuneVampire", group: "rune-vampire", blueImage: "../Effects/powerup_vampire_hud", redImage: "../Effects/powerup_vampire_hud" }),
  Object.freeze({ condition: 95, panel: "PlayerStatus_RuneReflect", group: "rune-reflect", blueImage: "../Effects/powerup_reflect_hud", redImage: "../Effects/powerup_reflect_hud" }),
  Object.freeze({ condition: 96, panel: "PlayerStatus_RunePrecision", group: "rune-precision", blueImage: "../Effects/powerup_precision_hud", redImage: "../Effects/powerup_precision_hud" }),
  Object.freeze({ condition: 97, panel: "PlayerStatus_RuneAgility", group: "rune-agility", blueImage: "../Effects/powerup_agility_hud", redImage: "../Effects/powerup_agility_hud" }),
  Object.freeze({ condition: 103, panel: "PlayerStatus_RuneKnockout", group: "rune-knockout", blueImage: "../Effects/powerup_knockout_hud", redImage: "../Effects/powerup_knockout_hud" }),
  Object.freeze({ condition: 109, panel: "PlayerStatus_RuneKing", group: "rune-king", blueImage: "../Effects/powerup_king_hud", redImage: "../Effects/powerup_king_hud" }),
  Object.freeze({ condition: 110, panel: "PlayerStatus_RunePlague", group: "rune-plague", blueImage: "../Effects/powerup_plague_hud", redImage: "../Effects/powerup_plague_hud" }),
  Object.freeze({ condition: 111, panel: "PlayerStatus_RuneSupernova", group: "rune-supernova", blueImage: "../Effects/powerup_supernova_hud", redImage: "../Effects/powerup_supernova_hud" }),
  Object.freeze({ condition: 80, panel: "PlayerStatus_Parachute", group: "parachute", blueImage: "../HUD/hud_parachute_active", redImage: "../HUD/hud_parachute_active" }),
])

export const TF2_INDEPENDENT_CONDITION_PANELS = Object.freeze([
  Object.freeze({ panel: "PlayerStatusBleedImage", conditions: Object.freeze([25]) }),
  Object.freeze({ panel: "PlayerStatusHookBleedImage", conditions: Object.freeze([101]) }),
  Object.freeze({ panel: "PlayerStatusMilkImage", conditions: Object.freeze([27]) }),
  Object.freeze({ panel: "PlayerStatusMarkedForDeathImage", conditions: Object.freeze([30]) }),
  Object.freeze({ panel: "PlayerStatusMarkedForDeathSilentImage", conditions: Object.freeze([48, 119]) }),
  Object.freeze({ panel: "PlayerStatusSlowed", conditions: Object.freeze([15]) }),
  Object.freeze({ panel: "PlayerStatusGasImage", conditions: Object.freeze([123]) }),
])
