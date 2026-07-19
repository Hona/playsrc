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
import { Tf2HudBindingError } from "./contract"
import { tf2HudAvailable, tf2HudUnavailable } from "./bindings"

export type CompactSessionHudContext = Readonly<{
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
}>

type CompactWeaponState = Readonly<{
  weapon: 1 | 2 | 3
  reload: 0 | 1 | 2 | 3
  clip: number
  reserve: number
  maximumClip: number
  maximumReserve: number
}>

type CompactGameplayEvent = Readonly<{
  kind: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11
  detail: number
  subject: number
  auxiliary: number
  values: readonly [number, number, number, number]
}>

type CompactSessionSnapshot = Readonly<{
  tick: bigint
  class: 1 | 2
  team: 1 | 2
  weapon: 1 | 2 | 3
  health: number
  maximumHealth: number
  lifecycle: 1 | 2
  conditions: readonly [number, number, number, number, number]
  loadout: readonly CompactWeaponState[]
  events: readonly CompactGameplayEvent[]
  lifecycleEvents: readonly Readonly<{ tick: bigint; kind: 1 | 2 | 3 | 4; class: 1 | 2; team: 1 | 2 }>[]
  projectileEvents: readonly Readonly<{ type: "fire" | "impact" | "stick" | "arm" | "fizzle" | "explode"; launcherIdentity: number }>[]
}>

export type CompactSessionSimulationPublication = Readonly<{
  eventBatches: readonly Readonly<{ snapshot: CompactSessionSnapshot }>[]
  snapshot: CompactSessionSnapshot
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

function health(snapshot: CompactSessionSnapshot, current = snapshot.health): Tf2HudHealth {
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
  return identity === 1 ? "Rocket Launcher" : identity === 2 ? "Original" : "Stickybomb Launcher"
}

function weaponPosition(identity: CompactWeaponState["weapon"]): number {
  return identity === 2 ? 1 : 0
}

function weapon(value: CompactWeaponState): Tf2HudWeapon {
  return Object.freeze({
    identity: value.weapon,
    itemDefinition: tf2HudUnavailable<number>("not-produced"),
    displayName: weaponName(value.weapon),
    slot: 0,
    position: weaponPosition(value.weapon),
    selectable: true,
    ammoDisplay: "clip-and-reserve",
    clip: tf2HudAvailable(value.clip),
    reserve: tf2HudAvailable(value.reserve),
    maximumClip: tf2HudAvailable(value.maximumClip),
    maximumReserve: tf2HudAvailable(value.maximumReserve),
    reload: reload(value.reload),
    drawsCrosshair: true,
  })
}

function classIdentity(value: CompactSessionSnapshot["class"]): 3 | 4 {
  return value === 1 ? 3 : 4
}

function teamIdentity(value: CompactSessionSnapshot["team"]): 2 | 3 {
  return value === 1 ? 2 : 3
}

function conditions(value: CompactSessionSnapshot["conditions"]): Tf2ConditionWords {
  return Object.freeze([...value]) as Tf2ConditionWords
}

function canonicalSnapshot(snapshot: CompactSessionSnapshot, context: CompactSessionHudContext): Tf2HudSnapshot {
  const words = conditions(snapshot.conditions)
  const tfSuppressed = context.crosshair.tfSuppressed || conditionActive(words, 7) || conditionActive(words, 77)
  const crosshair = Object.freeze({ ...context.crosshair, tfSuppressed })
  return Object.freeze({
    tick: snapshot.tick,
    player: tf2HudAvailable(Object.freeze({
      identity: context.playerIdentity,
      lifecycle: snapshot.lifecycle === 1 ? "active" as const : "dying" as const,
      class: tf2HudAvailable(classIdentity(snapshot.class)),
      team: tf2HudAvailable(teamIdentity(snapshot.team)),
      health: tf2HudAvailable(health(snapshot)),
      conditions: words,
      weapons: Object.freeze(snapshot.loadout.map(weapon)),
      activeWeapon: tf2HudAvailable(snapshot.weapon),
      weaponSelection: Object.freeze({
        open: context.weaponSelection.open,
        selectedWeapon: context.weaponSelection.selectedWeapon,
      }),
      crosshair: tf2HudAvailable(crosshair),
      liveHudSuppressed: context.liveHudSuppressed || conditionActive(words, 77),
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

function eventHealth(snapshot: CompactSessionSnapshot, current: number): Tf2HudHealth {
  return health(snapshot, current)
}

function ammoCause(snapshot: CompactSessionSnapshot, identity: number): Extract<Tf2HudEvent, { kind: "ammo" }>["cause"] {
  return snapshot.projectileEvents.some((event) => event.type === "fire" && event.launcherIdentity === identity)
    ? "fire"
    : "state"
}

function compactEventClass(detail: number): 3 | 4 {
  if (detail === 1) return 3
  if (detail === 2) return 4
  throw new Tf2HudBindingError("MalformedFacts", "compact class event is invalid")
}

function compactEventTeam(detail: number): 2 | 3 {
  if (detail === 1) return 2
  if (detail === 2) return 3
  throw new Tf2HudBindingError("MalformedFacts", "compact team event is invalid")
}

function eventWeapon(snapshot: CompactSessionSnapshot, identity: number): Tf2HudWeapon {
  const value = snapshot.loadout.find((item) => item.weapon === identity)
  if (!value) throw new Tf2HudBindingError("InconsistentPublication", "compact event weapon is absent from its tick")
  return weapon(value)
}

function integerEventValue(value: number, subject: string): number {
  if (!Number.isSafeInteger(value) || value < 0) throw new Tf2HudBindingError("MalformedFacts", `${subject} is invalid`)
  return value
}

function mapGameplayEvent(
  source: CompactGameplayEvent,
  snapshot: CompactSessionSnapshot,
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
        clip: tf2HudAvailable(integerEventValue(source.values[0], "compact reload clip")),
        reserve: tf2HudAvailable(integerEventValue(source.values[1], "compact reload reserve")),
        reload: state.reload,
        cause: "reload",
      })
      break
    }
    case 5: {
      const active = eventWeapon(snapshot, source.detail)
      const restoredActive = Object.freeze({
        ...active,
        clip: tf2HudAvailable(integerEventValue(source.values[1], "compact regenerate clip")),
        reserve: tf2HudAvailable(integerEventValue(source.values[2], "compact regenerate reserve")),
        reload: "ready" as const,
      })
      push({
        kind: "regenerate",
        zone: tf2HudAvailable(source.subject),
        health: eventHealth(snapshot, source.values[0]),
        weapons: Object.freeze(snapshot.loadout.map((item) => item.weapon === source.detail ? restoredActive : weapon(item))),
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
  }
}

export function adaptCompactSessionHud(
  previous: Tf2HudAvailability<Tf2HudSnapshot>,
  publication: CompactSessionSimulationPublication,
  context: CompactSessionHudContext,
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
      const healthAlreadyFinal = events.some((event) => event.tick === source.tick && (event.kind === "health" || event.kind === "damage" || event.kind === "regenerate") && currentHealth !== null && (
        sameHealth(event.health, currentHealth)
      ))
      if (currentHealth && !sameHealth(priorHealth, currentHealth) && !healthAlreadyFinal) {
        push({ kind: "health", health: currentHealth, cause: "state" })
      }

      for (const currentWeapon of currentPlayer.weapons) {
        const priorWeapon = priorPlayer.weapons.find((item) => item.identity === currentWeapon.identity)
        const ammoAlreadyFinal = events.some((event) => event.tick === source.tick && (
          event.kind === "ammo" && event.weapon === currentWeapon.identity && sameNumber(event.clip, currentWeapon.clip) && sameNumber(event.reserve, currentWeapon.reserve) && event.reload === currentWeapon.reload
          || event.kind === "regenerate" && event.weapons.some((item) => item.identity === currentWeapon.identity && sameWeapon(item, currentWeapon))
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
