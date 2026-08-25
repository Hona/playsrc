import { describe, expect, test } from "bun:test"
import {
  adaptSessionHud,
  bindTf2Hud,
  bindTf2HudAction,
  resolveTf2CrosshairGeometry,
  tf2CrosshairHudValues,
  tf2CrosshairSettings,
  TF2_GROUPED_CONDITION_PANELS,
  TF2_INDEPENDENT_CONDITION_PANELS,
  tf2HudAvailable,
  tf2HudUnavailable,
  type SessionHudContext,
  type SessionSimulationPublication,
  type Tf2Class,
  type Tf2ConditionWords,
  type Tf2HudAvailability,
  type Tf2HudCrosshair,
  type Tf2HudEvent,
  type Tf2HudHealth,
  type Tf2HudPanelValue,
  type Tf2HudPlayer,
  type Tf2HudScoreboard,
  type Tf2HudSnapshot,
  type Tf2HudWeapon,
  type Tf2ScoreboardCounters,
} from "../../src/hud"

const unavailable = <T>(reason: "initial" | "not-produced" | "not-applicable" | "missing-source-fact" = "not-produced") =>
  tf2HudUnavailable<T>(reason)

function crosshair(overrides: Partial<Tf2HudCrosshair> = {}): Tf2HudCrosshair {
  return Object.freeze({
    configured: true,
    weaponAllows: true,
    loadingImage: false,
    paused: false,
    clientModeAllows: true,
    frozen: false,
    localViewEntity: true,
    vguiInput: false,
    observerMode: "none",
    observerCrosshair: true,
    tfSuppressed: false,
    countdownHidden: false,
    texture: "crosshair_default",
    color: Object.freeze([200, 200, 200, 255]),
    scale: 32,
    weaponScale: 1,
    ...overrides,
  })
}

function weapon(overrides: Partial<Tf2HudWeapon> = {}): Tf2HudWeapon {
  return Object.freeze({
    identity: 1,
    itemDefinition: unavailable(),
    displayName: "Rocket Launcher",
    slot: 0,
    position: 0,
    selectable: true,
    ammoDisplay: "clip-and-reserve",
    clip: tf2HudAvailable(4),
    reserve: tf2HudAvailable(20),
    maximumClip: tf2HudAvailable(4),
    maximumReserve: tf2HudAvailable(20),
    reload: "ready",
    drawsCrosshair: true,
    ...overrides,
  })
}

function health(current = 200, maximum = 200, maximumBuffed = 300): Tf2HudHealth {
  return Object.freeze({ current, maximum, maximumBuffed })
}

function words(...conditions: number[]): Tf2ConditionWords {
  const output = [0, 0, 0, 0, 0]
  for (const condition of conditions) {
    const index = Math.floor(condition / 32)
    output[index] = (output[index]! | (1 << (condition % 32))) >>> 0
  }
  return Object.freeze(output) as Tf2ConditionWords
}

function player(overrides: Partial<Tf2HudPlayer> = {}): Tf2HudPlayer {
  return Object.freeze({
    identity: 1,
    lifecycle: "active",
    class: tf2HudAvailable(3),
    team: tf2HudAvailable(2),
    playerClassUsePlayerModel: false,
    classModel: tf2HudAvailable(Object.freeze({ identity: "models/player/soldier.mdl", skin: 0 })),
    health: tf2HudAvailable(health()),
    conditions: words(),
    weapons: Object.freeze([weapon()]),
    activeWeapon: tf2HudAvailable(1),
    weaponSelection: Object.freeze({ open: false, selectedWeapon: unavailable("not-applicable") }),
    crosshair: tf2HudAvailable(crosshair()),
    liveHudSuppressed: false,
    respawnAllowed: false,
    ...overrides,
  })
}

function snapshot(tick: bigint, overrides: Partial<Tf2HudSnapshot> = {}): Tf2HudSnapshot {
  return Object.freeze({
    tick,
    player: tf2HudAvailable(player()),
    scoreboard: unavailable(),
    freezePanel: unavailable(),
    ...overrides,
  })
}

function availablePrevious(value: Tf2HudSnapshot): Tf2HudAvailability<Tf2HudSnapshot> {
  return tf2HudAvailable(value)
}

function value(
  values: readonly Tf2HudPanelValue[],
  kind: Tf2HudPanelValue["kind"],
  panel: string,
  name?: string,
): Tf2HudPanelValue {
  const match = values.find((item) => item.kind === kind && item.panel === panel && (
    name === undefined
    || (item.kind === "dialog-variable" && item.variable === name)
    || (item.kind === "scalar" && item.property === name)
  ))
  if (!match) throw new Error(`missing ${kind}:${panel}:${name ?? ""}`)
  return match
}

function healthSnapshot(prior: Tf2HudSnapshot, tick: bigint, next: Tf2HudHealth, lifecycle: Tf2HudPlayer["lifecycle"] = "active") {
  const priorPlayer = (prior.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
  return snapshot(tick, {
    player: tf2HudAvailable(player({ ...priorPlayer, lifecycle, health: tf2HudAvailable(next) })),
  })
}

describe("immutable TF2 HUD binding", () => {
  test("binds exact health boundaries and ordered animation/damage/lifecycle transcripts", () => {
    const base = snapshot(1n)
    const initial = bindTf2Hud({ previous: unavailable("initial"), snapshot: base, events: Object.freeze([]) })
    expect(initial.animations).toEqual([])
    expect(value(initial.values, "dialog-variable", "HudPlayerHealth", "Health")).toMatchObject({ value: { kind: "available", value: 200 } })
    expect(value(initial.values, "dialog-variable", "HudPlayerHealth", "MaxHealth")).toMatchObject({ value: { kind: "available", value: "" } })

    const initialBoost = healthSnapshot(base, 2n, health(250))
    expect(bindTf2Hud({ previous: unavailable("initial"), snapshot: initialBoost, events: [] }).animations.map((item) => item.sequence))
      .toEqual(["HudHealthDyingPulseStop", "HudHealthBonusPulse"])

    const boosted = healthSnapshot(base, 2n, health(300))
    const bonus = bindTf2Hud({
      previous: availablePrevious(base),
      snapshot: boosted,
      events: Object.freeze([{ tick: 2n, ordinal: 0, kind: "health", health: health(300), cause: "heal" }]),
    })
    expect(bonus.animations.map((item) => item.sequence)).toEqual(["HudHealthDyingPulseStop", "HudHealthBonusPulse"])
    expect(value(bonus.values, "scalar", "PlayerStatusHealthBonusImage", "boundsAdjustment")).toMatchObject({ value: { value: 35 } })

    const critical = healthSnapshot(boosted, 3n, health(97))
    const damaged = bindTf2Hud({
      previous: availablePrevious(boosted),
      snapshot: critical,
      events: Object.freeze([{
        tick: 3n,
        ordinal: 0,
        kind: "damage",
        amount: 203,
        health: health(97),
        direction: tf2HudAvailable(Object.freeze([1, 0, 0]) as readonly [number, number, number]),
      }]),
    })
    expect(damaged.animations.map((item) => item.sequence)).toEqual(["HudHealthBonusPulseStop", "HudHealthDyingPulse"])
    expect(value(damaged.values, "color", "PlayerStatusHealthImage")).toMatchObject({ value: { value: [255, 0, 0, 255] } })
    expect(damaged.commands).toEqual([{
      kind: "damage-indicator",
      tick: 3n,
      ordinal: 0,
      scale: 100,
      lifetimeSeconds: 2,
      direction: [1, 0, 0],
    }])

    const threshold = healthSnapshot(critical, 4n, health(98))
    const recovered = bindTf2Hud({
      previous: availablePrevious(critical),
      snapshot: threshold,
      events: Object.freeze([{ tick: 4n, ordinal: 0, kind: "health", health: health(98), cause: "heal" }]),
    })
    expect(recovered.animations.map((item) => item.sequence)).toEqual(["HudHealthBonusPulseStop", "HudHealthDyingPulseStop"])
    expect(value(recovered.values, "visible", "PlayerStatusHealthBonusImage")).toMatchObject({ value: false })

    const dead = healthSnapshot(threshold, 5n, health(0), "dying")
    const death = bindTf2Hud({
      previous: availablePrevious(threshold),
      snapshot: dead,
      events: Object.freeze([
        { tick: 5n, ordinal: 0, kind: "damage", amount: 98, health: health(0), direction: unavailable("missing-source-fact") },
        { tick: 5n, ordinal: 1, kind: "lifecycle", lifecycle: "dying" },
      ]),
    })
    expect(death.commands).toEqual([{ kind: "lifecycle", tick: 5n, ordinal: 1, lifecycle: "dying" }])
    expect(value(death.values, "visible", "HudPlayerStatus")).toMatchObject({ value: false })
    expect(value(death.values, "visible", "HudCrosshair")).toMatchObject({ value: false })
    expect(value(death.values, "dialog-variable", "HudPlayerHealth", "Health")).toMatchObject({ value: { value: "" } })

    const respawned = healthSnapshot(dead, 6n, health(), "active")
    const respawn = bindTf2Hud({
      previous: availablePrevious(dead),
      snapshot: respawned,
      events: Object.freeze([
        { tick: 6n, ordinal: 0, kind: "health", health: health(), cause: "respawn" },
        { tick: 6n, ordinal: 1, kind: "ammo", weapon: 1, clip: tf2HudAvailable(4), reserve: tf2HudAvailable(20), reload: "ready", cause: "respawn" },
        { tick: 6n, ordinal: 2, kind: "lifecycle", lifecycle: "active" },
      ]),
    })
    expect(respawn.commands).toEqual([{ kind: "lifecycle", tick: 6n, ordinal: 2, lifecycle: "active" }])
    expect(value(respawn.values, "visible", "HudPlayerStatus")).toMatchObject({ value: true })
  })

  test("preserves strict low-ammo, reload and coalesced publication edges", () => {
    const base = snapshot(10n, { player: tf2HudAvailable(player({ weapons: Object.freeze([weapon({ clip: tf2HudAvailable(0), reserve: tf2HudAvailable(10) })]) })) })
    const low = snapshot(11n, { player: tf2HudAvailable(player({ weapons: Object.freeze([weapon({ clip: tf2HudAvailable(0), reserve: tf2HudAvailable(9), reload: "start" })]) })) })
    const lowBinding = bindTf2Hud({
      previous: availablePrevious(base),
      snapshot: low,
      events: Object.freeze([{ tick: 11n, ordinal: 0, kind: "ammo", weapon: 1, clip: tf2HudAvailable(0), reserve: tf2HudAvailable(9), reload: "start", cause: "reload" }]),
    })
    expect(lowBinding.animations.map((item) => item.sequence)).toEqual(["HudLowAmmoPulse"])
    expect(value(lowBinding.values, "visible", "HudWeaponLowAmmoImage")).toMatchObject({ value: true })
    expect(value(lowBinding.values, "color", "HudWeaponLowAmmoImage")).toMatchObject({ value: { value: [255, 0, 0, 255] } })
    expect(value(lowBinding.values, "scalar", "HudWeaponLowAmmoImage", "boundsAdjustment")).toMatchObject({ value: { value: 0 } })
    expect(value(lowBinding.values, "scalar", "HudWeaponAmmo", "reloadPhase")).toMatchObject({ value: { value: 1 } })

    const recovered = snapshot(12n, { player: tf2HudAvailable(player({ weapons: Object.freeze([weapon({ clip: tf2HudAvailable(1), reserve: tf2HudAvailable(9), reload: "insert" })]) })) })
    const coalesced = bindTf2Hud({
      previous: availablePrevious(base),
      snapshot: recovered,
      events: Object.freeze([
        { tick: 11n, ordinal: 0, kind: "ammo", weapon: 1, clip: tf2HudAvailable(0), reserve: tf2HudAvailable(9), reload: "start", cause: "reload" },
        { tick: 12n, ordinal: 0, kind: "ammo", weapon: 1, clip: tf2HudAvailable(1), reserve: tf2HudAvailable(9), reload: "insert", cause: "reload" },
      ]),
    })
    expect(coalesced.animations.map((item) => `${item.tick}:${item.sequence}`)).toEqual([
      "11:HudLowAmmoPulse",
      "12:HudLowAmmoPulseStop",
    ])
    expect(value(coalesced.values, "visible", "HudWeaponLowAmmoImage")).toMatchObject({ value: false })

    const alternate = weapon({ identity: 2, displayName: "Original", position: 1 })
    const lowPrimary = weapon({ clip: tf2HudAvailable(0), reserve: tf2HudAvailable(9) })
    const beforeSwitch = snapshot(13n, { player: tf2HudAvailable(player({ weapons: Object.freeze([lowPrimary, alternate]) })) })
    const afterSwitch = snapshot(14n, { player: tf2HudAvailable(player({ weapons: Object.freeze([lowPrimary, alternate]), activeWeapon: tf2HudAvailable(2) })) })
    const switched = bindTf2Hud({
      previous: availablePrevious(beforeSwitch),
      snapshot: afterSwitch,
      events: [{ tick: 14n, ordinal: 0, kind: "weapon-selected", weapon: 2 }],
    })
    expect(switched.animations.map((item) => item.sequence)).toEqual(["HudLowAmmoPulseStop"])
  })

  test("publishes regenerate after exact health, ammo and condition replacement", () => {
    const depletedWeapon = weapon({ clip: tf2HudAvailable(1), reserve: tf2HudAvailable(5), reload: "insert" })
    const prior = snapshot(15n, {
      player: tf2HudAvailable(player({
        health: tf2HudAvailable(health(50)),
        conditions: words(25),
        weapons: Object.freeze([depletedWeapon]),
      })),
    })
    const fullWeapon = weapon()
    const restored = snapshot(16n)
    const binding = bindTf2Hud({
      previous: availablePrevious(prior),
      snapshot: restored,
      events: Object.freeze([{
        tick: 16n,
        ordinal: 0,
        kind: "regenerate",
        zone: tf2HudAvailable(85),
        health: health(),
        weapons: Object.freeze([fullWeapon]),
        conditions: words(),
      }]),
    })
    expect(binding.animations.map((item) => item.sequence)).toEqual([
      "HudHealthBonusPulseStop",
      "HudHealthDyingPulseStop",
      "HudLowAmmoPulseStop",
    ])
    expect(binding.commands).toEqual([{
      kind: "regenerate-notification",
      tick: 16n,
      ordinal: 0,
      zone: { kind: "available", value: 85 },
    }])
    expect(value(binding.values, "dialog-variable", "HudWeaponAmmo", "Ammo")).toMatchObject({ value: { value: 4 } })
    expect(value(binding.values, "visible", "PlayerStatusBleedImage")).toMatchObject({ value: false })
  })

  test("lets pickup weapon facts supersede earlier fire ammo while rejecting stale final state", () => {
    const prior = snapshot(17n)
    const recovered = snapshot(18n)
    const fired: Tf2HudEvent = {
      tick: 18n,
      ordinal: 0,
      kind: "ammo",
      weapon: 1,
      clip: tf2HudAvailable(3),
      reserve: tf2HudAvailable(20),
      reload: "ready",
      cause: "fire",
    }
    const pickup: Tf2HudEvent = {
      tick: 18n,
      ordinal: 1,
      kind: "pickup",
      notification: {
        pickupIdentity: 42,
        pickup: "ammo",
        itemIdentity: tf2HudAvailable("item_ammopack_small"),
        amount: tf2HudAvailable(1),
      },
      health: unavailable("not-applicable"),
      weapon: tf2HudAvailable(weapon()),
    }
    const result = bindTf2Hud({ previous: availablePrevious(prior), snapshot: recovered, events: [fired, pickup] })
    expect(value(result.values, "dialog-variable", "HudWeaponAmmo", "Ammo")).toMatchObject({ value: { value: 4 } })
    const incorrect = snapshot(18n, {
      player: tf2HudAvailable(player({ weapons: Object.freeze([weapon({ clip: tf2HudAvailable(3) })]) })),
    })
    expect(() => bindTf2Hud({ previous: availablePrevious(prior), snapshot: incorrect, events: [fired, pickup] }))
      .toThrow("final HUD ammo differs from the event stream")
  })

  test("maps class/team, grouped conditions and exact crosshair eligibility", () => {
    const conditioned = snapshot(20n, {
      player: tf2HudAvailable(player({
        class: tf2HudAvailable(4),
        team: tf2HudAvailable(3),
        conditions: words(58, 61, 25, 48, 119),
      })),
    })
    const binding = bindTf2Hud({ previous: unavailable("initial"), snapshot: conditioned, events: Object.freeze([]) })
    expect(value(binding.values, "image", "PlayerStatusClassImage")).toMatchObject({ value: { value: "../hud/class_demoblue" } })
    expect(value(binding.values, "visible", "PlayerStatus_MedicUberBulletResistImage")).toMatchObject({ value: true })
    expect(value(binding.values, "visible", "PlayerStatus_MedicSmallBulletResistImage")).toMatchObject({ value: false })
    expect(value(binding.values, "visible", "PlayerStatusBleedImage")).toMatchObject({ value: true })
    expect(value(binding.values, "visible", "PlayerStatusMarkedForDeathSilentImage")).toMatchObject({ value: true })
    expect(value(binding.values, "visible", "HudCrosshair")).toMatchObject({ value: true })

    const paused = snapshot(21n, { player: tf2HudAvailable(player({ crosshair: tf2HudAvailable(crosshair({ paused: true })) })) })
    expect(value(bindTf2Hud({ previous: unavailable("initial"), snapshot: paused, events: [] }).values, "visible", "HudCrosshair"))
      .toMatchObject({ value: false })

    const expectedImages = {
      2: ["class_scoutred", "class_sniperred", "class_soldierred", "class_demored", "class_medicred", "class_heavyred", "class_pyrored", "class_spyred", "class_engired"],
      3: ["class_scoutblue", "class_sniperblue", "class_soldierblue", "class_demoblue", "class_medicblue", "class_heavyblue", "class_pyroblue", "class_spyblue", "class_engiblue"],
    } as const
    for (const team of [2, 3] as const) {
      for (let index = 0; index < expectedImages[team].length; index += 1) {
        const current = snapshot(22n, { player: tf2HudAvailable(player({ class: tf2HudAvailable((index + 1) as Tf2Class), team: tf2HudAvailable(team) })) })
        expect(value(bindTf2Hud({ previous: unavailable("initial"), snapshot: current, events: [] }).values, "image", "PlayerStatusClassImage"))
          .toMatchObject({ value: { value: `../hud/${expectedImages[team][index]}` } })
      }
    }
    for (const overrides of [
      { class: unavailable("not-produced") },
      { team: unavailable("not-produced") },
      { team: tf2HudAvailable(1 as const) },
    ]) {
      const current = snapshot(22n, { player: tf2HudAvailable(player(overrides)) })
      const values = bindTf2Hud({ previous: unavailable("initial"), snapshot: current, events: [] }).values
      expect(value(values, "visible", "PlayerStatusClassImage")).toMatchObject({ value: false })
      expect(value(values, "visible", "PlayerStatusClassImageBG")).toMatchObject({ value: false })
      expect(value(values, "visible", "classmodelpanel")).toMatchObject({ value: false })
      expect(value(values, "visible", "classmodelpanelBG")).toMatchObject({ value: false })
    }

    const everyReviewedCondition = words(
      ...TF2_GROUPED_CONDITION_PANELS.map((item) => item.condition),
      ...TF2_INDEPENDENT_CONDITION_PANELS.flatMap((item) => item.conditions),
    )
    const everyConditionBinding = bindTf2Hud({
      previous: unavailable("initial"),
      snapshot: snapshot(23n, { player: tf2HudAvailable(player({ conditions: everyReviewedCondition })) }),
      events: [],
    })
    const seenGroups = new Set<string>()
    for (const item of TF2_GROUPED_CONDITION_PANELS) {
      expect(value(everyConditionBinding.values, "visible", item.panel)).toMatchObject({ value: !seenGroups.has(item.group) })
      seenGroups.add(item.group)
    }
    for (const item of TF2_INDEPENDENT_CONDITION_PANELS) {
      expect(value(everyConditionBinding.values, "visible", item.panel)).toMatchObject({ value: true })
    }
  })

  test("applies every SDK crosshair suppression and custom-weapon override", () => {
    const visibleFor = (crosshairOverrides: Partial<Tf2HudCrosshair>, overrides: Partial<Tf2HudPlayer> = {}) => {
      const facts = snapshot(24n, {
        player: tf2HudAvailable(player({ ...overrides, crosshair: tf2HudAvailable(crosshair(crosshairOverrides)) })),
      })
      return value(bindTf2Hud({ previous: unavailable("initial"), snapshot: facts, events: [] }).values, "visible", "HudCrosshair")
    }
    for (const suppressed of [
      { configured: false }, { weaponAllows: false }, { loadingImage: true }, { paused: true },
      { clientModeAllows: false }, { frozen: true }, { localViewEntity: false }, { vguiInput: true },
      { tfSuppressed: true }, { countdownHidden: true },
    ]) expect(visibleFor(suppressed), JSON.stringify(suppressed)).toMatchObject({ value: false })
    expect(visibleFor({}, { weapons: Object.freeze([weapon({ drawsCrosshair: false })]) })).toMatchObject({ value: false })
    expect(visibleFor({ texture: "vgui/crosshairs/crosshair7", weaponAllows: false }, {
      weapons: Object.freeze([weapon({ drawsCrosshair: false })]),
    })).toMatchObject({ value: true })
    expect(visibleFor({ observerMode: "in-eye" }, { lifecycle: "observer" })).toMatchObject({ value: true })
    expect(visibleFor({ observerMode: "roaming", observerCrosshair: false }, { lifecycle: "observer" })).toMatchObject({ value: false })
    expect(visibleFor({ observerMode: "roaming", observerCrosshair: true }, { lifecycle: "observer" })).toMatchObject({ value: true })
    expect(visibleFor({ observerMode: "other", observerCrosshair: true }, { lifecycle: "observer" })).toMatchObject({ value: false })
  })

  test("uses exact weapon-selected versus custom dimensions, centering, tint, and wrapping", () => {
    const settings = tf2CrosshairSettings({
      "multiplayer.crosshair-red": 300.75,
      "multiplayer.crosshair-green": -1,
      "multiplayer.crosshair-blue": 511,
      "multiplayer.crosshair-scale": 47,
      "multiplayer.crosshair-file": "crosshair4",
    })
    expect(settings).toMatchObject({ red: 44, green: 255, blue: 255, scale: 47, file: "crosshair4" })
    expect(tf2CrosshairHudValues(settings)).toEqual({
      texture: "vgui/crosshairs/crosshair4",
      color: [44, 255, 255, 255],
      scale: 47,
    })
    const stock = bindTf2Hud({ previous: unavailable("initial"), snapshot: snapshot(24n, {
      player: tf2HudAvailable(player({ crosshair: tf2HudAvailable(crosshair({ scale: 31, weaponScale: 1.5 })) })),
    }), events: [] })
    expect(resolveTf2CrosshairGeometry(stock, { width: 1025, height: 769 })).toMatchObject({
      kind: "stock", left: 490, top: 362, width: 47, height: 47,
      asset: { crop: { x: 32, y: 32, width: 32, height: 32 } },
    })
    const custom = bindTf2Hud({ previous: unavailable("initial"), snapshot: snapshot(25n, {
      player: tf2HudAvailable(player({ crosshair: tf2HudAvailable(crosshair({
        texture: "vgui/crosshairs/crosshair4", scale: 31, weaponScale: 1.5,
      })) })),
    }), events: [] })
    expect(resolveTf2CrosshairGeometry(custom, { width: 1025, height: 769 })).toMatchObject({
      kind: "custom", left: 466, top: 338, width: 94, height: 94,
      asset: { file: "crosshair4" },
    })
    const absent = bindTf2Hud({ previous: unavailable("initial"), snapshot: snapshot(26n, {
      player: tf2HudAvailable(player({ crosshair: tf2HudAvailable(crosshair({ texture: "vgui/crosshairs/not_installed" })) })),
    }), events: [] })
    expect(resolveTf2CrosshairGeometry(absent, { width: 1025, height: 769 })).toBeNull()
  })

  test("hides the complete zero-condition baseline and removes every prior condition panel", () => {
    const zero = bindTf2Hud({ previous: unavailable("initial"), snapshot: snapshot(24n), events: [] })
    for (const item of TF2_GROUPED_CONDITION_PANELS) {
      expect(value(zero.values, "visible", item.panel), item.panel).toMatchObject({ value: false })
    }
    for (const item of TF2_INDEPENDENT_CONDITION_PANELS) {
      expect(value(zero.values, "visible", item.panel), item.panel).toMatchObject({ value: false })
    }
    expect(value(zero.values, "visible", "PlayerStatus_WheelOfDoom")).toMatchObject({ value: false })

    for (const selected of [...TF2_GROUPED_CONDITION_PANELS, ...TF2_INDEPENDENT_CONDITION_PANELS.map((item) => ({ ...item, condition: item.conditions[0]! }))]) {
      const active = bindTf2Hud({
        previous: unavailable("initial"),
        snapshot: snapshot(25n, { player: tf2HudAvailable(player({ conditions: words(selected.condition) })) }),
        events: [],
      })
      expect(value(active.values, "visible", selected.panel), selected.panel).toMatchObject({ value: true })
    }

    const conditioned = snapshot(26n, { player: tf2HudAvailable(player({ conditions: words(25, 58, 90) })) })
    const cleared = snapshot(27n)
    const removed = bindTf2Hud({
      previous: availablePrevious(conditioned),
      snapshot: cleared,
      events: [{ tick: 27n, ordinal: 0, kind: "conditions", conditions: words() }],
    })
    for (const panel of ["PlayerStatusBleedImage", "PlayerStatus_MedicUberBulletResistImage", "PlayerStatus_RuneStrength"]) {
      expect(value(removed.values, "visible", panel), panel).toMatchObject({ value: false })
    }
  })

  test("retains scoreboard, killfeed, pickup and regenerate facts without mutating inputs", () => {
    const counters: Tf2ScoreboardCounters = Object.freeze({
      kills: 3, deaths: 1, assists: 2, destruction: 0, captures: 1, defenses: 0, dominations: 1,
      revenge: 0, healing: 0, invulns: 0, teleports: 0, headshots: 0, backstabs: 0, bonus: 2,
      support: 2, damage: 450,
    })
    const scoreboard: Tf2HudScoreboard = Object.freeze({
      visible: true,
      red: Object.freeze({ team: 2, localizedName: "RED", score: 2, playerCount: 1 }),
      blue: Object.freeze({ team: 3, localizedName: "BLU", score: 1, playerCount: 0 }),
      players: Object.freeze([Object.freeze({
        identity: 1, name: "Soldier", team: 2, connection: "connected", score: 5, alive: true,
        class: tf2HudAvailable(3), ping: tf2HudAvailable(24), killstreak: 3, activeDominations: 1,
        relationship: "none", counters: tf2HudAvailable(counters),
      })]),
      spectators: Object.freeze(["Watcher"]),
      waitingToPlay: Object.freeze([]),
      selectedPlayer: tf2HudAvailable(1),
    })
    const base = snapshot(30n, { scoreboard: tf2HudAvailable(scoreboard) })
    const next = snapshot(31n, { scoreboard: tf2HudAvailable(scoreboard) })
    const input = Object.freeze({
      previous: availablePrevious(base),
      snapshot: next,
      events: Object.freeze([
        Object.freeze({
          tick: 31n, ordinal: 0, kind: "pickup", notification: Object.freeze({
            pickupIdentity: 9, pickup: "ammo", itemIdentity: tf2HudAvailable("item_ammopack_small"), amount: tf2HudAvailable(5),
          }), health: unavailable("not-applicable"), weapon: unavailable("not-applicable"),
        }),
        Object.freeze({
          tick: 31n, ordinal: 1, kind: "killfeed", notice: Object.freeze({
            killer: Object.freeze({ identity: tf2HudAvailable(1), name: "Soldier", team: 2 }),
            victim: Object.freeze({ identity: tf2HudAvailable(2), name: "Demoman", team: 3 }),
            assister: unavailable("not-applicable"), weaponIcon: tf2HudAvailable("d_rocketlauncher"),
            weaponIdentity: tf2HudAvailable(18), customKill: 0, critical: false, selfInflicted: false,
            localPlayerInvolved: true, domination: false, revenge: false, silent: false,
          }),
        }),
      ] as readonly Tf2HudEvent[]),
    })
    const before = JSON.stringify(input, (_key, item) => typeof item === "bigint" ? `${item}n` : item)
    const binding = bindTf2Hud(input)
    expect(JSON.stringify(input, (_key, item) => typeof item === "bigint" ? `${item}n` : item)).toBe(before)
    expect(binding.commands.map((item) => item.kind)).toEqual(["pickup-notification", "killfeed-notice"])
    expect(binding.scoreboard).toMatchObject({ kind: "available", value: { players: [{ counters: { value: { damage: 450 } } }] } })
    expect(value(binding.values, "dialog-variable", "scoreinfo", "redteamscore")).toMatchObject({ value: { value: 2 } })
    expect(value(binding.values, "dialog-variable", "scoreinfo", "redteamplayercount")).toMatchObject({
      value: { value: { kind: "localized", token: "#TF_ScoreBoard_Player", parameters: [1] } },
    })
    expect(Object.isFrozen(binding)).toBe(true)
    expect(Object.isFrozen((binding.scoreboard as Extract<typeof binding.scoreboard, { kind: "available" }>).value.players)).toBe(true)
  })

  test("returns typed actions without calling gameplay transitions", () => {
    let transitions = 0
    const dead = snapshot(40n, { player: tf2HudAvailable(player({ lifecycle: "dying", respawnAllowed: true })) })
    const respawn = bindTf2HudAction(dead, { kind: "respawn", transition: () => { transitions += 1 } } as never)
    expect(respawn).toEqual({ kind: "available", value: { kind: "respawn", player: 1 } })
    expect(transitions).toBe(0)
    expect(bindTf2HudAction(snapshot(41n), { kind: "select-weapon", weapon: 1 }))
      .toEqual({ kind: "available", value: { kind: "select-weapon", player: 1, weapon: 1 } })
    expect(bindTf2HudAction(snapshot(41n), { kind: "scoreboard", visible: true }))
      .toEqual({ kind: "available", value: { kind: "scoreboard", visible: true } })
  })

  test("rejects event-order and scoreboard bounds instead of dropping facts", () => {
    expect(() => bindTf2Hud({
      previous: availablePrevious(snapshot(50n)),
      snapshot: snapshot(51n),
      events: [{ tick: 51n, ordinal: 1, kind: "health", health: health(), cause: "state" }],
    })).toThrow("ordinals are not contiguous")

    const players = Array.from({ length: 65 }, (_, index) => Object.freeze({
      identity: index + 1, name: `P${index}`, team: 2 as const, connection: "connected" as const, score: 0,
      alive: true, class: tf2HudAvailable(3 as const), ping: unavailable<number>(), killstreak: 0,
      activeDominations: 0, relationship: "none" as const, counters: unavailable<Tf2ScoreboardCounters>(),
    }))
    const oversized = snapshot(51n, { scoreboard: tf2HudAvailable({
      visible: true,
      red: { team: 2, localizedName: "RED", score: 0, playerCount: 64 },
      blue: { team: 3, localizedName: "BLU", score: 0, playerCount: 0 },
      players,
      spectators: [],
      waitingToPlay: [],
      selectedPlayer: unavailable("not-applicable"),
    }) })
    expect(() => bindTf2Hud({ previous: unavailable("initial"), snapshot: oversized, events: [] }))
      .toThrow("scoreboard player list exceeds its bound")
  })
})

function compactSnapshot(
  tick: bigint,
  overrides: Partial<SessionSimulationPublication["snapshot"]> = {},
): SessionSimulationPublication["snapshot"] {
  return Object.freeze({
    tick,
    class: 3,
    team: 2,
    weapon: 1,
    health: 200,
    maximumHealth: 200,
    lifecycle: 1,
    conditions: Object.freeze([0, 0, 0, 0, 0]),
    loadout: Object.freeze([Object.freeze({ weapon: 1, reload: 0, clip: 4, reserve: 20, maximumClip: 4, maximumReserve: 20 })]),
    events: Object.freeze([]),
    lifecycleEvents: Object.freeze([]),
    projectileEvents: Object.freeze([]),
    ...overrides,
  })
}

function compactPublication(...snapshots: SessionSimulationPublication["snapshot"][]): SessionSimulationPublication {
  return Object.freeze({
    eventBatches: Object.freeze(snapshots.map((item) => Object.freeze({ snapshot: item }))),
    snapshot: snapshots.at(-1)!,
  })
}

describe("canonical all-class TF2 session HUD adapter", () => {
  const context: SessionHudContext = Object.freeze({
    playerIdentity: 1,
    liveHudSuppressed: false,
    respawnAllowed: true,
    weaponSelection: Object.freeze({ open: false, selectedWeapon: unavailable("not-applicable") }),
    crosshair: crosshair(),
    scoreboard: unavailable(),
    freezePanel: unavailable(),
    playerClassUsePlayerModel: false,
  })

  test("publishes exact Soldier stock slots, item identities, and hidden shovel ammunition", () => {
    for (const active of [1, 7, 8] as const) {
      const source = compactSnapshot(1n, {
        weapon: active,
        loadout: Object.freeze([
          Object.freeze({ weapon: 1 as const, reload: 0 as const, clip: 4, reserve: 20, maximumClip: 4, maximumReserve: 20 }),
          Object.freeze({ weapon: 7 as const, reload: 0 as const, clip: 6, reserve: 32, maximumClip: 6, maximumReserve: 32 }),
          Object.freeze({ weapon: 8 as const, reload: 0 as const, clip: 0, reserve: 0, maximumClip: 0, maximumReserve: 0 }),
        ]),
      })
      const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(source), context))
      const player = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
      expect(player.weapons.map((weapon) => [weapon.identity, weapon.slot, weapon.itemDefinition])).toEqual([
        [1, 0, { kind: "available", value: 18 }],
        [7, 1, { kind: "available", value: 10 }],
        [8, 2, { kind: "available", value: 6 }],
      ])
      const shovel = player.weapons[2]!
      expect(shovel.ammoDisplay).toBe("hidden")
      expect(shovel.clip).toEqual({ kind: "unavailable", reason: "not-applicable" })
    }
  })

  test("suppresses observer HUD and carries the authoritative spectator team without a weapon", () => {
    const observer = compactSnapshot(1n, {
      team: 1,
      lifecycle: 4,
      health: 0,
      weapon: null,
      loadout: Object.freeze([]),
      events: Object.freeze([Object.freeze({ kind: 2, detail: 1, subject: 0, auxiliary: 0,
        values: Object.freeze([0, 0, 0, 0]) as readonly [number, number, number, number] })]),
    })
    const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(observer), context))
    const player = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
    expect(player.lifecycle).toBe("observer")
    expect(player.team).toEqual({ kind: "available", value: 1 })
    expect(player.activeWeapon.kind).toBe("unavailable")
    expect(player.liveHudSuppressed).toBeTrue()
  })

  test("publishes all nine canonical class models and both team images without inventing weapons", () => {
    const classes = [
      [1, "scout", 125],
      [2, "sniper", 125],
      [3, "soldier", 200],
      [4, "demo", 175],
      [5, "medic", 150],
      [6, "heavy", 300],
      [7, "pyro", 175],
      [8, "spy", 125],
      [9, "engineer", 125],
    ] as const
    for (const [identity, model, maximumHealth] of classes) {
      for (const team of [2, 3] as const) {
        const armed = identity === 1 || identity === 2 || identity === 3 || identity === 4 || identity === 6 || identity === 9
        const active = identity === 1 ? 4 : identity === 2 ? 12 : identity === 3 ? 1 : identity === 4 ? 3 : identity === 6 ? 9 : identity === 9 ? 40 : null
        const source = compactSnapshot(1n, {
          class: identity,
          team,
          weapon: active,
          health: maximumHealth,
          maximumHealth,
          loadout: armed
            ? Object.freeze([Object.freeze({
              weapon: active!, reload: 0 as const, clip: 4, reserve: 20, maximumClip: 8, maximumReserve: 24,
            })])
            : Object.freeze([]),
        })
        const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(source), context))
        const player = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
        expect(player.class).toEqual({ kind: "available", value: identity })
        expect(player.team).toEqual({ kind: "available", value: team })
        expect(player.classModel).toEqual({
          kind: "available",
          value: { identity: `models/player/${model}.mdl`, skin: team === 2 ? 0 : 1 },
        })
        expect(player.health).toMatchObject({ kind: "available", value: { maximum: maximumHealth } })
        expect(player.activeWeapon).toEqual(active === null
          ? { kind: "unavailable", reason: "not-applicable" }
          : { kind: "available", value: active })
        const image = model === "engineer" ? "engi" : model
        expect(value(binding.values, "image", "PlayerStatusClassImage"))
          .toMatchObject({ value: { value: `../hud/class_${image}${team === 2 ? "red" : "blue"}` } })
        expect(value(binding.values, "visible", "HudWeaponAmmo"))
          .toMatchObject({ value: armed })
      }
    }
  })

  test("publishes Scout stock item identities, authored slots, and hides Bat ammunition", () => {
    const loadout = Object.freeze([
      Object.freeze({ weapon: 4 as const, reload: 0 as const, clip: 6, reserve: 32, maximumClip: 6, maximumReserve: 32 }),
      Object.freeze({ weapon: 5 as const, reload: 0 as const, clip: 12, reserve: 36, maximumClip: 12, maximumReserve: 36 }),
      Object.freeze({ weapon: 6 as const, reload: 0 as const, clip: 0, reserve: 0, maximumClip: 0, maximumReserve: 0 }),
    ])
    for (const active of [4, 5, 6] as const) {
      const source = compactSnapshot(1n, { class: 1, weapon: active, health: 125, maximumHealth: 125, loadout })
      const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(source), context))
      const player = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
      expect(player.weapons.map((item) => ({ identity: item.identity, item: item.itemDefinition, slot: item.slot, name: item.displayName, ammo: item.ammoDisplay }))).toEqual([
        { identity: 4, item: { kind: "available", value: 13 }, slot: 0, name: "Scattergun", ammo: "clip-and-reserve" },
        { identity: 5, item: { kind: "available", value: 23 }, slot: 1, name: "Pistol", ammo: "clip-and-reserve" },
        { identity: 6, item: { kind: "available", value: 0 }, slot: 2, name: "Bat", ammo: "hidden" },
      ])
      expect(value(binding.values, "visible", "HudWeaponAmmo")).toMatchObject({ value: active !== 6 })
    }
  })

  test("publishes distinct Heavy stock item identities, total Minigun ammo, and hidden Fists ammo", () => {
    const loadout = Object.freeze([
      Object.freeze({ weapon: 9 as const, reload: 0 as const, clip: 0, reserve: 200, maximumClip: 0, maximumReserve: 200 }),
      Object.freeze({ weapon: 10 as const, reload: 0 as const, clip: 6, reserve: 32, maximumClip: 6, maximumReserve: 32 }),
      Object.freeze({ weapon: 11 as const, reload: 0 as const, clip: 0, reserve: 0, maximumClip: 0, maximumReserve: 0 }),
    ])
    for (const active of [9, 10, 11] as const) {
      const source = compactSnapshot(1n, { class: 6, weapon: active, health: 300, maximumHealth: 300, loadout })
      const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(source), context))
      const player = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
      expect(player.weapons.map((item) => ({ identity: item.identity, item: item.itemDefinition, slot: item.slot, name: item.displayName, ammo: item.ammoDisplay }))).toEqual([
        { identity: 9, item: { kind: "available", value: 15 }, slot: 0, name: "Minigun", ammo: "total" },
        { identity: 10, item: { kind: "available", value: 11 }, slot: 1, name: "Shotgun", ammo: "clip-and-reserve" },
        { identity: 11, item: { kind: "available", value: 5 }, slot: 2, name: "Fists", ammo: "hidden" },
      ])
      expect(value(binding.values, "visible", "HudWeaponAmmo")).toMatchObject({ value: active !== 11 })
      expect(value(binding.values, "visible", "AmmoNoClip")).toMatchObject({ value: active === 9 })
      expect(value(binding.values, "visible", "AmmoInClip")).toMatchObject({ value: active === 10 })
      expect(value(binding.values, "dialog-variable", "HudWeaponAmmo", "Ammo")).toMatchObject({
        value: active === 11 ? { kind: "unavailable" } : { kind: "available", value: active === 9 ? 200 : 6 },
      })
    }
  })

  test("publishes Engineer stock item identities, authored slots, and hides Wrench ammunition", () => {
    const loadout = Object.freeze([
      Object.freeze({ weapon: 40 as const, reload: 0 as const, clip: 6, reserve: 32, maximumClip: 6, maximumReserve: 32 }),
      Object.freeze({ weapon: 41 as const, reload: 0 as const, clip: 12, reserve: 200, maximumClip: 12, maximumReserve: 200 }),
      Object.freeze({ weapon: 42 as const, reload: 0 as const, clip: 0, reserve: 0, maximumClip: 0, maximumReserve: 0 }),
    ])
    for (const active of [40, 41, 42] as const) {
      const source = compactSnapshot(1n, { class: 9, weapon: active, health: 125, maximumHealth: 125, loadout })
      const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(source), context))
      const player = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
      expect(player.weapons.map((item) => ({ identity: item.identity, item: item.itemDefinition, slot: item.slot, name: item.displayName, ammo: item.ammoDisplay }))).toEqual([
        { identity: 40, item: { kind: "available", value: 9 }, slot: 0, name: "Shotgun", ammo: "clip-and-reserve" },
        { identity: 41, item: { kind: "available", value: 22 }, slot: 1, name: "Pistol", ammo: "clip-and-reserve" },
        { identity: 42, item: { kind: "available", value: 7 }, slot: 2, name: "Wrench", ammo: "hidden" },
      ])
      expect(value(binding.values, "visible", "HudWeaponAmmo")).toMatchObject({ value: active !== 42 })
    }
  })

  test("retains unavailable Minigun clip and Fists ammunition through locker regeneration", () => {
    for (const active of [9, 11] as const) {
      const loadout = Object.freeze([
        Object.freeze({ weapon: 9 as const, reload: 0 as const, clip: 0, reserve: 200, maximumClip: 0, maximumReserve: 200 }),
        Object.freeze({ weapon: 11 as const, reload: 0 as const, clip: 0, reserve: 0, maximumClip: 0, maximumReserve: 0 }),
      ])
      const initial = compactSnapshot(1n, { class: 6, weapon: active, health: 300, maximumHealth: 300, loadout })
      const previous = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(initial), context)).facts
      const regenerated = compactSnapshot(2n, {
        class: 6,
        weapon: active,
        health: 300,
        maximumHealth: 300,
        loadout,
        events: Object.freeze([Object.freeze({ kind: 5, detail: active, subject: 85, auxiliary: 0, values: Object.freeze([300, 0, active === 9 ? 200 : 0, 0]) })]),
      })
      const publication = adaptSessionHud(availablePrevious(previous), compactPublication(regenerated), context)
      expect(() => bindTf2Hud(publication)).not.toThrow()
      const event = publication.events.find((value) => value.kind === "regenerate") as Extract<Tf2HudEvent, { kind: "regenerate" }>
      const state = event.weapons.find((value) => value.identity === active)!
      expect(state.clip).toEqual({ kind: "unavailable", reason: "not-applicable" })
      expect(state.reserve.kind).toBe(active === 9 ? "available" : "unavailable")
    }
  })

  test("retains fire/reload ticks across one coalesced host publication", () => {
    const initial = adaptSessionHud(unavailable("initial"), compactPublication(compactSnapshot(1n)), context)
    const prior = bindTf2Hud(initial).facts
    const fired = compactSnapshot(2n, {
      loadout: Object.freeze([Object.freeze({ weapon: 1, reload: 0, clip: 3, reserve: 20, maximumClip: 4, maximumReserve: 20 })]),
      projectileEvents: Object.freeze([Object.freeze({ type: "fire", launcherIdentity: 1 })]),
    })
    const reloaded = compactSnapshot(3n, {
      loadout: Object.freeze([Object.freeze({ weapon: 1, reload: 3, clip: 4, reserve: 19, maximumClip: 4, maximumReserve: 20 })]),
      events: Object.freeze([Object.freeze({ kind: 4, detail: 1, subject: 0, auxiliary: 0, values: Object.freeze([4, 19, 0, 0]) })]),
    })
    const publication = adaptSessionHud(availablePrevious(prior), compactPublication(fired, reloaded), context)
    expect(publication.events.map((event) => [event.tick, event.ordinal, event.kind, "cause" in event ? event.cause : null])).toEqual([
      [2n, 0, "ammo", "fire"],
      [3n, 0, "ammo", "reload"],
    ])
    const binding = bindTf2Hud(publication)
    expect(value(binding.values, "dialog-variable", "HudWeaponAmmo", "Ammo")).toMatchObject({ value: { value: 4 } })
    expect(value(binding.values, "scalar", "HudWeaponAmmo", "reloadPhase")).toMatchObject({ value: { value: 3 } })
  })

  test("preserves canonical Demoman/BLU stock class, team and overheal identities", () => {
    const demo = compactSnapshot(1n, {
      class: 4,
      team: 3,
      weapon: 3,
      health: 175,
      maximumHealth: 175,
      loadout: Object.freeze([Object.freeze({ weapon: 3, reload: 0, clip: 8, reserve: 24, maximumClip: 8, maximumReserve: 24 })]),
    })
    const binding = bindTf2Hud(adaptSessionHud(unavailable("initial"), compactPublication(demo), context))
    const mappedPlayer = (binding.facts.player as Extract<Tf2HudSnapshot["player"], { kind: "available" }>).value
    expect(mappedPlayer).toMatchObject({
      class: { kind: "available", value: 4 },
      team: { kind: "available", value: 3 },
      health: { kind: "available", value: { current: 175, maximum: 175, maximumBuffed: 260 } },
      activeWeapon: { kind: "available", value: 3 },
      playerClassUsePlayerModel: false,
      classModel: { kind: "available", value: { identity: "models/player/demo.mdl", skin: 1 } },
    })
    expect(value(binding.values, "image", "PlayerStatusClassImage")).toMatchObject({ value: { value: "../hud/class_demoblue" } })
    expect(value(binding.values, "dialog-variable", "classmodelpanel", "weaponName")).toMatchObject({ value: { value: "Stickybomb Launcher" } })
    expect(value(binding.values, "scalar", "classmodelpanel", "itemDefinition")).toMatchObject({ value: { kind: "unavailable" } })
  })

  test("retains regenerate-before-fire ammo within one compact tick", () => {
    const initial = adaptSessionHud(unavailable("initial"), compactPublication(compactSnapshot(1n)), context)
    const prior = bindTf2Hud(initial).facts
    const regeneratedAndFired = compactSnapshot(2n, {
      loadout: Object.freeze([Object.freeze({ weapon: 1, reload: 0, clip: 3, reserve: 20, maximumClip: 4, maximumReserve: 20 })]),
      events: Object.freeze([Object.freeze({ kind: 5, detail: 1, subject: 85, auxiliary: 0, values: Object.freeze([200, 4, 20, 0]) })]),
      projectileEvents: Object.freeze([Object.freeze({ type: "fire", launcherIdentity: 1 })]),
    })
    const publication = adaptSessionHud(availablePrevious(prior), compactPublication(regeneratedAndFired), context)
    expect(publication.events.map((event) => event.kind)).toEqual(["regenerate", "ammo"])
    const regenerate = publication.events[0] as Extract<Tf2HudEvent, { kind: "regenerate" }>
    const fired = publication.events[1] as Extract<Tf2HudEvent, { kind: "ammo" }>
    expect(regenerate.weapons[0]).toMatchObject({ clip: { value: 4 }, reserve: { value: 20 }, reload: "ready" })
    expect(fired).toMatchObject({ clip: { value: 3 }, reserve: { value: 20 }, cause: "fire" })
    expect(bindTf2Hud(publication).commands.map((command) => command.kind)).toEqual(["regenerate-notification"])
  })

  test("retains fire-before-regenerate ammo within one coalesced publication", () => {
    const initial = adaptSessionHud(unavailable("initial"), compactPublication(compactSnapshot(1n)), context)
    const prior = bindTf2Hud(initial).facts
    const fired = compactSnapshot(2n, {
      loadout: Object.freeze([Object.freeze({ weapon: 1, reload: 0, clip: 3, reserve: 20, maximumClip: 4, maximumReserve: 20 })]),
      projectileEvents: Object.freeze([Object.freeze({ type: "fire", launcherIdentity: 1 })]),
    })
    const regenerated = compactSnapshot(3n, {
      events: Object.freeze([Object.freeze({ kind: 5, detail: 1, subject: 85, auxiliary: 0, values: Object.freeze([200, 4, 20, 0]) })]),
    })
    const publication = adaptSessionHud(availablePrevious(prior), compactPublication(fired, regenerated), context)
    expect(publication.events.map((event) => event.kind)).toEqual(["ammo", "regenerate"])
    expect(() => bindTf2Hud(publication)).not.toThrow()
    expect(value(bindTf2Hud(publication).values, "dialog-variable", "HudWeaponAmmo", "Ammo"))
      .toMatchObject({ value: { value: 4 } })
  })

  test("marks unavailable compact damage direction and preserves death ordering", () => {
    const initial = adaptSessionHud(unavailable("initial"), compactPublication(compactSnapshot(1n)), context)
    const prior = bindTf2Hud(initial).facts
    const dead = compactSnapshot(2n, {
      health: 0,
      lifecycle: 2,
      events: Object.freeze([Object.freeze({ kind: 6, detail: 0, subject: 0, auxiliary: 0, values: Object.freeze([200, 0, 0, 0]) })]),
      lifecycleEvents: Object.freeze([Object.freeze({ tick: 2n, kind: 1, class: 3, team: 2 })]),
    })
    const publication = adaptSessionHud(availablePrevious(prior), compactPublication(dead), context)
    expect(publication.events.map((event) => [event.ordinal, event.kind])).toEqual([[0, "damage"], [1, "lifecycle"]])
    expect((publication.events[0] as Extract<Tf2HudEvent, { kind: "damage" }>).direction)
      .toEqual({ kind: "unavailable", reason: "missing-source-fact" })
    const binding = bindTf2Hud(publication)
    expect(binding.commands).toEqual([{ kind: "lifecycle", tick: 2n, ordinal: 1, lifecycle: "dying" }])
  })
})
