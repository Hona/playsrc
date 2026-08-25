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
import type { CaptureObjectives, RoundSnapshot, Tf2Class, Tf2Team, Tf2Weapon } from "../codec"
import { tf2ClassPresentation } from "../class"
import { Tf2HudBindingError } from "./contract"
import { tf2HudAvailable, tf2HudUnavailable } from "./bindings"

export type SessionHudContext = Readonly<{
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
}>

type SessionSnapshot = Readonly<{
  tick: bigint
  class: Tf2Class
  team: Tf2Team
  weapon: Tf2Weapon | null
  health: number
  maximumHealth: number
  lifecycle: 1 | 2 | 3 | 4
  objectives?: CaptureObjectives | null
  round?: RoundSnapshot
  conditions: readonly [number, number, number, number, number]
  loadout: readonly CompactWeaponState[]
  events: readonly CompactGameplayEvent[]
  lifecycleEvents: readonly Readonly<{ tick: bigint; kind: 1 | 2 | 3 | 4; class: Tf2Class; team: Tf2Team }>[]
  projectileEvents: readonly Readonly<{ type: "fire" | "impact" | "stick" | "arm" | "fizzle" | "explode"; launcherIdentity: number }>[]
  bots?: readonly Readonly<{ identity: number; team: Tf2Team; class: Tf2Class }>[]
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

function weaponName(identity: CompactWeaponState["weapon"]): string {

  return identity === 40 ? "Shotgun" : identity === 41 ? "Pistol" : identity === 42 ? "Wrench"
    : (["", "Rocket Launcher", "Original", "Stickybomb Launcher", "Scattergun", "Pistol", "Bat", "Shotgun", "Shovel", "Minigun", "Shotgun", "Fists", "Sniper Rifle", "SMG", "Kukri", "Flamethrower", "Fire Axe", "Bottle", "Grenade Launcher"] as const)[identity]
}

function weapon(value: CompactWeaponState, playerClass: Tf2Class): Tf2HudWeapon {
  const totalAmmo = value.weapon === 9 || value.weapon === 12 || value.weapon === 15
  const melee = value.weapon === 6 || value.weapon === 8 || value.weapon === 11 || value.weapon === 14 || value.weapon === 16 || value.weapon === 17 || value.weapon === 42
  const definitions = ([undefined, 18, undefined, 20, 13, 23, 0, 10, 6, 15, 11, 5, 14, 16, 3, 21, 2, 1, 19] as const)
  const definition = value.weapon === 7 && playerClass === 7 ? 12 : value.weapon === 15 ? 21 : value.weapon === 16 ? 2
    : value.weapon === 40 ? 9 : value.weapon === 41 ? 22 : value.weapon === 42 ? 7 : definitions[value.weapon]

  return Object.freeze({
    identity: value.weapon,
    itemDefinition: definition === undefined ? tf2HudUnavailable<number>("not-produced") : tf2HudAvailable(definition),
    displayName: weaponName(value.weapon),

    slot: value.weapon === 3 || value.weapon === 5 || value.weapon === 7 || value.weapon === 10 || value.weapon === 13 || value.weapon === 41 ? 1 : melee ? 2 : 0,
    position: value.weapon === 2 ? 1 : 0,
    selectable: true,
    ammoDisplay: melee ? "hidden" as const : totalAmmo ? "total" as const : "clip-and-reserve" as const,
    clip: totalAmmo || melee ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.clip),
    reserve: melee ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.reserve),
    maximumClip: totalAmmo || melee ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.maximumClip),

    maximumReserve: melee ? tf2HudUnavailable<number>("not-applicable") : tf2HudAvailable(value.maximumReserve),
    reload: reload(value.reload),
    drawsCrosshair: true,
  })
}

function classModel(snapshot: SessionSnapshot) {
  return Object.freeze({
    identity: tf2ClassPresentation(snapshot.class).model,
    skin: snapshot.team === 2 ? 0 : 1,
  })
}

function conditions(value: SessionSnapshot["conditions"]): Tf2ConditionWords {
  return Object.freeze([...value]) as Tf2ConditionWords
}

function canonicalSnapshot(snapshot: SessionSnapshot, context: SessionHudContext): Tf2HudSnapshot {
  const words = conditions(snapshot.conditions)
  const tfSuppressed = context.crosshair.tfSuppressed || conditionActive(words, 1) || conditionActive(words, 7) || conditionActive(words, 77)
  const crosshair = Object.freeze({ ...context.crosshair, tfSuppressed })
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
      weapons: Object.freeze(snapshot.loadout.map((item) => weapon(item, snapshot.class))),
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

function eventWeapon(snapshot: SessionSnapshot, identity: number): Tf2HudWeapon {
  const value = snapshot.loadout.find((item) => item.weapon === identity)
  if (!value) throw new Tf2HudBindingError("InconsistentPublication", "compact event weapon is absent from its tick")
  return weapon(value, snapshot.class)
}

function integerEventValue(value: number, subject: string): number {
  if (!Number.isSafeInteger(value) || value < 0) throw new Tf2HudBindingError("MalformedFacts", `${subject} is invalid`)
  return value
}

function mapGameplayEvent(
  source: CompactGameplayEvent,
  snapshot: SessionSnapshot,
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
      eventWeapon(snapshot, source.detail)
      push({ kind: "weapon-selected", weapon: source.detail })
      break
    case 4: {
      const state = eventWeapon(snapshot, source.detail)
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
      const active = eventWeapon(snapshot, source.detail)
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
        weapons: Object.freeze(snapshot.loadout.map((item) => item.weapon === source.detail ? restoredActive : weapon(item, snapshot.class))),
        conditions: conditions(snapshot.conditions),
      })
      break
    }
    case 6:
      push({
        kind: "damage",
        amount: source.values[0],
        health: eventHealth(snapshot, source.values[1]),
        direction: tf2HudUnavailable("missing-source-fact"),
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
      const active = snapshot.weapon === null ? null : eventWeapon(snapshot, snapshot.weapon)
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
        const bot = snapshot.bots?.find(value => value.identity === identity)
        return Object.freeze({ identity: tf2HudAvailable(identity),
          name: bot ? `Bot ${identity}` : "Player", team: bot?.team ?? snapshot.team })
      }
      const killer = source.auxiliary === 0
        ? Object.freeze({ identity: tf2HudUnavailable<number>("not-applicable"), name: "World", team: snapshot.team })
        : participant(source.auxiliary)
      push({ kind: "killfeed", notice: Object.freeze({ killer, victim: participant(source.subject),
        assister: source.values[0] === 0 ? tf2HudUnavailable("not-applicable") : tf2HudAvailable(participant(source.values[0])),
        weaponIcon: source.detail === 0 ? tf2HudUnavailable("not-applicable") : tf2HudAvailable(weaponName(source.detail as Tf2Weapon)),
        weaponIdentity: source.detail === 0 ? tf2HudUnavailable("not-applicable") : tf2HudAvailable(source.detail),
        customKill: source.values[2], critical: source.values[1] === 1,
        selfInflicted: source.auxiliary === source.subject, localPlayerInvolved: source.auxiliary === 1 || source.subject === 1,
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
    for (const event of source.events) mapGameplayEvent(event, source, push)

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

  const snapshot = canonicalSnapshot(publication.snapshot, context)
  if (rolling?.tick !== snapshot.tick) {
    throw new Tf2HudBindingError("InconsistentPublication", "compact final snapshot does not match its final event batch")
  }
  return Object.freeze({ previous, snapshot, events: Object.freeze(events) })
}
