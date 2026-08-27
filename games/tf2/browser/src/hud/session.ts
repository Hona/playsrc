import type {
  Tf2ConditionWords,
  Tf2HudAvailability,
  Tf2HudCrosshair,
  Tf2HudEvent,
  Tf2HudFreezePanel,
  Tf2HudHealth,
  Tf2HudPublication,
  Tf2HudScoreboard,
  Tf2HudSnapshot,
  Tf2HudWeapon,
  Tf2ReloadPhase,
} from "./contract"
import type { CaptureObjectives, ControlPoints, RoundSnapshot, Tf2Class, Tf2Team, Tf2Weapon } from "../codec"
import type { Tf2EquippedItem, Tf2SupportedItem } from "../equipment/types"
import { equipmentCatalogIndex } from "../equipment/lookup"
import { tf2ClassPresentation } from "../class"
import { Tf2HudBindingError } from "./contract"
import { tf2HudAvailable, tf2HudUnavailable } from "./bindings"

export type SessionHudContext = Readonly<{
  inventory: readonly Tf2SupportedItem[]
  playerIdentity: number
  liveHudSuppressed: boolean
  respawnAllowed: boolean
  weaponSelection: Readonly<{
    open: boolean
    selectedWeapon: Tf2HudAvailability<number>
  }>
  crosshair: Tf2HudCrosshair
  scoreboard: Tf2HudAvailability<Tf2HudScoreboard>
  freezePanel: Tf2HudAvailability<Tf2HudFreezePanel>
  playerClassUsePlayerModel: boolean
}>

type CompactWeaponState = Readonly<{
  weapon: Tf2Weapon
  reload: 0 | 1 | 2 | 3
  clip: number
  reserve: number
  maximumClip: number
  maximumReserve: number
}>

type CompactGameplayEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19
  detail: number
  subject: number
  auxiliary: number
  values: readonly [number, number, number, number]
  killingWeapon?: string
}>

type SessionSnapshot = Readonly<{
  weaponCrosshairScale: number
  decapitations: number
  revengeCrits: number
  equippedItems: readonly Tf2EquippedItem[]
  tick: bigint
  class: Tf2Class
  team: Tf2Team
  weapon: Tf2Weapon | null
  health: number
  maximumHealth: number
  spy?: Readonly<{ cloakMeter: number; invisibility: number; disguise: Readonly<{ class: Tf2Class; team: Tf2Team }> | null; desiredDisguise: Readonly<{ class: Tf2Class; team: Tf2Team }> | null }> | null
  lifecycle: 1 | 2 | 3 | 4
  objectives?: CaptureObjectives | null
  controlPoints?: ControlPoints | null
  round?: RoundSnapshot
  conditions: readonly [number, number, number, number, number]
  loadout: readonly CompactWeaponState[]
  events: readonly CompactGameplayEvent[]
  lifecycleEvents: readonly Readonly<{ tick: bigint; kind: 1 | 2 | 3 | 4; class: Tf2Class; team: Tf2Team }>[]
  projectileEvents: readonly Readonly<{ type: "fire" | "impact" | "stick" | "arm" | "fizzle" | "explode"; launcherIdentity: number }>[]
  bots?: readonly Readonly<{ identity: number; team: Tf2Team; class: Tf2Class }>[]
  scoreboard?: Readonly<{ players: readonly Readonly<{ identity: number; name: string; team: Tf2Team }>[] }>
}>

export type SessionSimulationPublication = Readonly<{
  eventBatches: readonly Readonly<{ snapshot: SessionSnapshot }>[]
  snapshot: SessionSnapshot
}>

type Tf2HudEventPayload = Tf2HudEvent extends infer Event
  ? Event extends Tf2HudEvent
    ? Omit<Event, "tick" | "ordinal">
    : never
  : never

function conditionActive(words: readonly number[], condition: number): boolean {
  return (words[Math.floor(condition / 32)]! & (1 << (condition % 32))) !== 0
}

function maximumBuffedHealth(current: number, maximum: number): number {
  return Math.max(Math.floor((maximum * 1.5) / 5) * 5, maximum, current)
}

function health(snapshot: SessionSnapshot, current = snapshot.health): Tf2HudHealth {
  return Object.freeze({
    current,
    maximum: snapshot.maximumHealth,
    maximumBuffed: maximumBuffedHealth(current, snapshot.maximumHealth),
  })
}

function reload(value: CompactWeaponState["reload"]): Tf2ReloadPhase {
  return (["ready", "start", "insert", "finish"] as const)[value]
}

function weapon(value: CompactWeaponState, snapshot: SessionSnapshot, inventory: readonly Tf2SupportedItem[]): Tf2HudWeapon {
  const definitions = equipmentCatalogIndex(inventory)
  const matches = snapshot.equippedItems.flatMap(item => {
    const definition = definitions!.get(item.definitionIndex)
    const eligibility = definition?.classSlots.find(slot => slot.class === snapshot.class && slot.slot === item.slot && slot.weapon === value.weapon)
    return definition && eligibility?.hud ? [{ item, definition, eligibility, hud: eligibility.hud }] : []
  })
  if (matches.length !== 1) throw new Error(`TF2 HUD weapon has no unique equipped definition: ${snapshot.class}:${value.weapon}`)
  const { item, definition, eligibility, hud } = matches[0]!
  const noAmmo = hud.ammoDisplay === "hidden", totalAmmo = hud.ammoDisplay === "total"
  return Object.freeze({
    identity: value.weapon,
    itemDefinition: tf2HudAvailable(item.definitionIndex),
    displayName: definition.displayName,
    slot: eligibility.selectionSlot ?? hud.bucket,
    position: hud.position,
    selectable: eligibility.selectionSlot !== null,
    ammoDisplay: hud.ammoDisplay,
    clip: totalAmmo || noAmmo ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.clip),
    reserve: noAmmo ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.reserve),
    maximumClip: totalAmmo || noAmmo ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.maximumClip),

    maximumReserve: noAmmo ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.maximumReserve),
    reload: reload(value.reload),
    crosshairScript: hud.script,
    suppressesCrosshair: hud.suppressCrosshair,
    drawsCrosshair: hud.drawsCrosshair,
  })
}

function classModel(snapshot: SessionSnapshot) {
  const disguise = snapshot.spy?.disguise
  const identity = disguise?.class ?? snapshot.class
  const team = disguise?.team ?? snapshot.team
  return Object.freeze({
    identity: tf2ClassPresentation(identity).model,
    skin: team === 2 ? 0 : 1,
  })
}

function conditions(value: SessionSnapshot["conditions"]): Tf2ConditionWords {
  return Object.freeze([...value]) as Tf2ConditionWords
}

function canonicalSnapshot(snapshot: SessionSnapshot, context: SessionHudContext): Tf2HudSnapshot {
  const words = conditions(snapshot.conditions)
  const tfSuppressed = context.crosshair.tfSuppressed || conditionActive(words, 1) || conditionActive(words, 7) || conditionActive(words, 77)
  const crosshair = Object.freeze({ ...context.crosshair, tfSuppressed, weaponScale: snapshot.weaponCrosshairScale })
  return Object.freeze({
    tick: snapshot.tick,
    player: tf2HudAvailable(Object.freeze({
      identity: context.playerIdentity,
      lifecycle: snapshot.lifecycle === 1 ? "active" as const : snapshot.lifecycle === 2 ? "dying" as const : "observer" as const,
      class: tf2HudAvailable(snapshot.class),
      team: tf2HudAvailable(snapshot.team),
      playerClassUsePlayerModel: context.playerClassUsePlayerModel,
      classModel: tf2HudAvailable(classModel(snapshot)),
      health: tf2HudAvailable(health(snapshot)),
      conditions: words,
      ...(snapshot.spy ? { spy: Object.freeze({
        cloakMeter: snapshot.spy.cloakMeter,
        invisibility: snapshot.spy.invisibility,
        disguise: snapshot.spy.disguise,
        desiredDisguise: snapshot.spy.desiredDisguise,
      }) } : {}),
      weapons: Object.freeze(snapshot.loadout.map((item) => weapon(item, snapshot, context.inventory))),
      activeWeapon: snapshot.weapon === null
        ? tf2HudUnavailable<number>("not-applicable")
        : tf2HudAvailable(snapshot.weapon),
      weaponSelection: Object.freeze({
        open: context.weaponSelection.open,
        selectedWeapon: context.weaponSelection.selectedWeapon,
      }),
      crosshair: tf2HudAvailable(crosshair),
      liveHudSuppressed: context.liveHudSuppressed || snapshot.lifecycle === 3 || snapshot.lifecycle === 4 || conditionActive(words, 77),
      respawnAllowed: context.respawnAllowed,
    })),
    scoreboard: context.scoreboard,
    freezePanel: context.freezePanel,
  })
}

function sameNumber(left: Tf2HudAvailability<number>, right: Tf2HudAvailability<number>): boolean {
  if (left.kind === "unavailable") return right.kind === "unavailable" && left.reason === right.reason
  return right.kind === "available" && left.value === right.value
}

function sameWeapon(left: Tf2HudWeapon | undefined, right: Tf2HudWeapon): boolean {
  return left !== undefined
    && sameNumber(left.clip, right.clip)
    && sameNumber(left.reserve, right.reserve)
    && left.reload === right.reload
}

function sameHealth(left: Tf2HudHealth | null, right: Tf2HudHealth): boolean {
  return left !== null
    && left.current === right.current
    && left.maximum === right.maximum
    && left.maximumBuffed === right.maximumBuffed
}

function finalPlayer(snapshot: Tf2HudSnapshot) {
  if (snapshot.player.kind !== "available") throw new Tf2HudBindingError("MalformedFacts", "compact session player is unavailable")
  return snapshot.player.value
}

function eventHealth(snapshot: SessionSnapshot, current: number): Tf2HudHealth {
  return health(snapshot, current)
}

function ammoCause(snapshot: SessionSnapshot, identity: number): Extract<Tf2HudEvent, { kind: "ammo" }>["cause"] {
  return snapshot.projectileEvents.some((event) => event.type === "fire" && event.launcherIdentity === identity)
    || snapshot.events.some((event) => event.kind === 12 && event.detail === identity)
    ? "fire"
    : "state"
}

function compactEventClass(detail: number): Tf2Class {
  if (Number.isInteger(detail) && detail >= 1 && detail <= 9) return detail as Tf2Class
  throw new Tf2HudBindingError("MalformedFacts", "session class event is invalid")
}

function compactEventTeam(detail: number): Tf2Team {
  if (detail === 0 || detail === 1 || detail === 2 || detail === 3) return detail
  throw new Tf2HudBindingError("MalformedFacts", "session team event is invalid")
}

function eventWeapon(weapons: readonly Tf2HudWeapon[], identity: number): Tf2HudWeapon {
  const value = weapons.find((item) => item.identity === identity)
  if (!value) throw new Tf2HudBindingError("InconsistentPublication", "compact event weapon is absent from its tick")
  return value
}

function integerEventValue(value: number, subject: string): number {
  if (!Number.isSafeInteger(value) || value < 0) throw new Tf2HudBindingError("MalformedFacts", `${subject} is invalid`)
  return value
}

function mapGameplayEvent(
  source: CompactGameplayEvent,
  snapshot: SessionSnapshot,
  weapons: readonly Tf2HudWeapon[],
  push: (event: Tf2HudEventPayload) => void,
): void {
  switch (source.kind) {
    case 1:
      push({ kind: "class-changed", class: compactEventClass(source.detail) })
      break
    case 2:
      push({ kind: "team-changed", team: compactEventTeam(source.detail) })
      break
    case 3:
      eventWeapon(weapons, source.detail)
      push({ kind: "weapon-selected", weapon: source.detail })
      break
    case 4: {
      const state = eventWeapon(weapons, source.detail)
      push({
        kind: "ammo",
        weapon: state.identity,
        clip: state.clip.kind === "available"
          ? tf2HudAvailable(integerEventValue(source.values[0], "compact reload clip"))
          : state.clip,
        reserve: state.reserve.kind === "available"
          ? tf2HudAvailable(integerEventValue(source.values[1], "compact reload reserve"))
          : state.reserve,
        reload: state.reload,
        cause: "reload",
      })
      break
    }
    case 5: {
      const active = eventWeapon(weapons, source.detail)
      const restoredActive = Object.freeze({
        ...active,
        clip: active.clip.kind === "available"
          ? tf2HudAvailable(integerEventValue(source.values[1], "compact regenerate clip"))
          : active.clip,
        reserve: active.reserve.kind === "available"
          ? tf2HudAvailable(integerEventValue(source.values[2], "compact regenerate reserve"))
          : active.reserve,
        reload: "ready" as const,
      })
      push({
        kind: "regenerate",
        zone: tf2HudAvailable(source.subject),
        health: eventHealth(snapshot, source.values[0]),
        weapons: Object.freeze(weapons.map((item) => item.identity === source.detail ? restoredActive : item)),
        conditions: conditions(snapshot.conditions),
      })
      break
    }
    case 6:
      push({
        kind: "damage",
        amount: source.values[0],
        health: eventHealth(snapshot, source.values[1]),
        direction: source.detail === 1
          ? tf2HudAvailable(Object.freeze([
              source.values[2],
              source.values[3],
              new DataView(Uint32Array.of(source.subject).buffer).getFloat32(0, true),
            ]) as readonly [number, number, number])
          : tf2HudUnavailable("missing-source-fact"),
      })
      break
    case 7:
      push({ kind: "health", health: eventHealth(snapshot, source.values[1]), cause: "heal" })
      break
    case 11:
      push({ kind: "health", health: health(snapshot), cause: "respawn" })
      push({ kind: "lifecycle", lifecycle: "active" })
      break
    case 15:
    case 16: {
      if (source.auxiliary !== 1) break
      const pickup = source.kind === 15 ? "health" as const : "ammo" as const
      const suffix = (["small", "medium", "large"] as const)[source.detail]
      if (!suffix) throw new Tf2HudBindingError("MalformedFacts", "compact pickup size is invalid")
      const active = snapshot.weapon === null ? null : eventWeapon(weapons, snapshot.weapon)
      push({
        kind: "pickup",
        notification: Object.freeze({
          pickupIdentity: source.subject,
          pickup,
          itemIdentity: tf2HudAvailable(`${pickup === "health" ? "medkit" : "ammopack"}_${suffix}`),
          amount: tf2HudAvailable(integerEventValue(source.values[0], "compact pickup amount")),
        }),
        health: pickup === "health"
          ? tf2HudAvailable(eventHealth(snapshot, source.values[1]))
          : tf2HudUnavailable("not-applicable"),
        weapon: pickup === "ammo" && active
          ? tf2HudAvailable(Object.freeze({
              ...active,
              clip: active.clip.kind === "available"
                ? tf2HudAvailable(integerEventValue(source.values[2], "compact pickup clip"))
                : active.clip,
              reserve: active.reserve.kind === "available"
                ? tf2HudAvailable(integerEventValue(source.values[3], "compact pickup reserve"))
                : active.reserve,
            }))
          : tf2HudUnavailable("not-applicable"),
      })
      break
    }
    case 18: {
      const participant = (identity: number) => {
        const player = snapshot.scoreboard?.players.find(value => value.identity === identity)
        if (!player) throw new Tf2HudBindingError("MalformedFacts", `death notice participant ${identity} is absent`)
        return Object.freeze({ identity: tf2HudAvailable(identity),
          name: player.name, team: player.team })
      }
      const killer = source.auxiliary === 0
        ? Object.freeze({ identity: tf2HudUnavailable<number>("not-applicable"), name: "", team: 0 as const })
        : participant(source.auxiliary)
      if (!source.killingWeapon) throw new Tf2HudBindingError("MalformedFacts", "death notice killing weapon is absent")
      push({ kind: "killfeed", notice: Object.freeze({ killer, victim: participant(source.subject),
        assister: source.values[0] === 0 ? tf2HudUnavailable("not-applicable") : tf2HudAvailable(participant(source.values[0])),
        weaponIcon: tf2HudAvailable(source.killingWeapon),
        weaponIdentity: source.detail === 0 ? tf2HudUnavailable("not-applicable") : tf2HudAvailable(source.detail),
        customKill: source.values[2], damageBits: source.values[1], critical: (source.values[1] & (1 << 20)) !== 0,
        selfInflicted: source.auxiliary === 0 || source.auxiliary === source.subject,
        localPlayerInvolved: source.auxiliary === 1 || source.subject === 1 || source.values[0] === 1,
        domination: false, revenge: false, silent: false }) })
      break
    }
  }
}

export function adaptSessionHud(
  previous: Tf2HudAvailability<Tf2HudSnapshot>,
  publication: SessionSimulationPublication,
  context: SessionHudContext,
): Tf2HudPublication {
  if (publication.eventBatches.length === 0) {
    throw new Tf2HudBindingError("MalformedFacts", "compact simulation publication has no event batches")
  }
  const events: Tf2HudEvent[] = []
  let rolling = previous.kind === "available" ? previous.value : null
  for (const batch of publication.eventBatches) {
    const source = batch.snapshot
    const current = canonicalSnapshot(source, context)
    let ordinal = 0
    const push = (event: Tf2HudEventPayload): void => {
      events.push(Object.freeze({ tick: source.tick, ordinal, ...event }) as Tf2HudEvent)
      ordinal += 1
    }
    for (const event of source.events) mapGameplayEvent(event, source, finalPlayer(current).weapons, push)

    const priorPlayer = rolling?.player.kind === "available" ? rolling.player.value : null
    const currentPlayer = finalPlayer(current)
    if (priorPlayer) {
      const priorHealth = priorPlayer.health.kind === "available" ? priorPlayer.health.value : null
      const currentHealth = currentPlayer.health.kind === "available" ? currentPlayer.health.value : null
      const healthAlreadyFinal = events.some((event) => event.tick === source.tick && currentHealth !== null && (
        (event.kind === "health" || event.kind === "damage" || event.kind === "regenerate") && sameHealth(event.health, currentHealth)
        || event.kind === "pickup" && event.health.kind === "available" && sameHealth(event.health.value, currentHealth)
      ))
      if (currentHealth && !sameHealth(priorHealth, currentHealth) && !healthAlreadyFinal) {
        push({ kind: "health", health: currentHealth, cause: "state" })
      }

      for (const currentWeapon of currentPlayer.weapons) {
        const priorWeapon = priorPlayer.weapons.find((item) => item.identity === currentWeapon.identity)
        const ammoAlreadyFinal = events.some((event) => event.tick === source.tick && (
          event.kind === "ammo" && event.weapon === currentWeapon.identity && sameNumber(event.clip, currentWeapon.clip) && sameNumber(event.reserve, currentWeapon.reserve) && event.reload === currentWeapon.reload
          || event.kind === "regenerate" && event.weapons.some((item) => item.identity === currentWeapon.identity && sameWeapon(item, currentWeapon))
          || event.kind === "pickup" && event.weapon.kind === "available" && event.weapon.value.identity === currentWeapon.identity && sameWeapon(event.weapon.value, currentWeapon)
        ))
        if (!sameWeapon(priorWeapon, currentWeapon) && !ammoAlreadyFinal) {
          push({ kind: "ammo", weapon: currentWeapon.identity, clip: currentWeapon.clip, reserve: currentWeapon.reserve, reload: currentWeapon.reload, cause: ammoCause(source, currentWeapon.identity) })
        }
      }
      const currentActiveWeapon = currentPlayer.activeWeapon.kind === "available" ? currentPlayer.activeWeapon.value : null
      if (priorPlayer.activeWeapon.kind === "available" && currentActiveWeapon !== null && priorPlayer.activeWeapon.value !== currentActiveWeapon
        && !events.some((event) => event.tick === source.tick && event.kind === "weapon-selected" && event.weapon === currentActiveWeapon)) {
        push({ kind: "weapon-selected", weapon: currentActiveWeapon })
      }
      if (priorPlayer.class.kind === "available" && currentPlayer.class.kind === "available" && priorPlayer.class.value !== currentPlayer.class.value
        && !events.some((event) => event.tick === source.tick && event.kind === "class-changed")) {
        push({ kind: "class-changed", class: currentPlayer.class.value })
      }
      if (priorPlayer.team.kind === "available" && currentPlayer.team.kind === "available" && priorPlayer.team.value !== currentPlayer.team.value
        && !events.some((event) => event.tick === source.tick && event.kind === "team-changed")) {
        push({ kind: "team-changed", team: currentPlayer.team.value })
      }
      if (priorPlayer.conditions.some((word, index) => word !== currentPlayer.conditions[index])
        && !events.some((event) => event.tick === source.tick && (event.kind === "conditions" || event.kind === "regenerate"))) {
        push({ kind: "conditions", conditions: currentPlayer.conditions })
      }
    }

    for (const event of source.lifecycleEvents) {
      if (event.kind === 1 && !events.some((item) => item.tick === source.tick && item.kind === "lifecycle" && item.lifecycle === "dying")) {
        push({ kind: "lifecycle", lifecycle: "dying" })
      }
    }
    rolling = current
  }

  if (rolling?.tick !== publication.snapshot.tick) {
    throw new Tf2HudBindingError("InconsistentPublication", "compact final snapshot does not match its final event batch")
  }
  const snapshot = rolling
  return Object.freeze({ previous, snapshot, events: Object.freeze(events) })
}
