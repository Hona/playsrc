const MAX_DEFINITIONS = 4096
const MAX_EFFECTS = 4096
const IDENTITY = /^[\x21-\x7e]{1,256}$/

export type SpriteDefinition = Readonly<{
  identity: string
  lifetimeTicks: number
  startRadius: number
  endRadius: number
  color: number
  startOpacity: number
  endOpacity: number
}>

export type EmitRequest = Readonly<{
  identity: number
  definition: string
  tick: bigint
  position: readonly [number, number, number]
}>

export type ParticleRenderItem = Readonly<{
  identity: number
  position: readonly [number, number, number]
  radius: number
  color: number
  opacity: number
}>

type Effect = Readonly<{
  identity: number
  definition: SpriteDefinition
  tick: bigint
  position: readonly [number, number, number]
}>

export class ParticleError extends Error {
  constructor(
    readonly code: "MalformedDefinition" | "MissingDefinition" | "MalformedEvent" | "BoundExceeded" | "TimeReversed",
    message: string,
  ) {
    super(message)
    this.name = "ParticleError"
  }
}

function canonical(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => letter.toLowerCase())
}

function finite(values: readonly number[]): boolean {
  return values.every(Number.isFinite)
}

export function createParticleSystem(definitions: readonly SpriteDefinition[]): Readonly<{
  emit(request: EmitRequest): void
  advance(tick: bigint): readonly ParticleRenderItem[]
  reset(tick: bigint): void
  dispose(): void
}> {
  if (definitions.length > MAX_DEFINITIONS) {
    throw new ParticleError("BoundExceeded", "particle definition count exceeds its limit")
  }
  const registry = new Map<string, SpriteDefinition>()
  for (const definition of definitions) {
    if (
      !IDENTITY.test(definition.identity)
      || !Number.isSafeInteger(definition.lifetimeTicks)
      || definition.lifetimeTicks < 1
      || !finite([
        definition.startRadius,
        definition.endRadius,
        definition.startOpacity,
        definition.endOpacity,
      ])
      || definition.startRadius <= 0
      || definition.endRadius <= 0
      || definition.startOpacity < 0
      || definition.startOpacity > 1
      || definition.endOpacity < 0
      || definition.endOpacity > 1
      || !Number.isSafeInteger(definition.color)
      || definition.color < 0
      || definition.color > 0xff_ffff
    ) {
      throw new ParticleError("MalformedDefinition", "particle sprite definition is invalid")
    }
    const identity = canonical(definition.identity)
    if (registry.has(identity)) {
      throw new ParticleError("MalformedDefinition", "particle definition identity is duplicated")
    }
    registry.set(identity, Object.freeze({ ...definition }))
  }
  let tick = 0n
  let effects: Effect[] = []
  let disposed = false
  return Object.freeze({
    emit(request: EmitRequest): void {
      if (
        disposed
        || !Number.isSafeInteger(request.identity)
        || request.identity < 1
        || !IDENTITY.test(request.definition)
        || typeof request.tick !== "bigint"
        || !finite(request.position)
        || effects.some((effect) => effect.identity === request.identity)
      ) {
        throw new ParticleError("MalformedEvent", "particle event is invalid")
      }
      if (request.tick < tick) throw new ParticleError("TimeReversed", "particle event precedes current time")
      if (effects.length >= MAX_EFFECTS) {
        throw new ParticleError("BoundExceeded", "particle effect count exceeds its limit")
      }
      const definition = registry.get(canonical(request.definition))
      if (!definition) throw new ParticleError("MissingDefinition", `particle definition ${request.definition} is missing`)
      effects.push(Object.freeze({
        identity: request.identity,
        definition,
        tick: request.tick,
        position: Object.freeze([...request.position]) as readonly [number, number, number],
      }))
    },
    advance(nextTick: bigint): readonly ParticleRenderItem[] {
      if (disposed || typeof nextTick !== "bigint" || nextTick < tick) {
        throw new ParticleError("TimeReversed", "particle time moved backward")
      }
      tick = nextTick
      effects = effects.filter((effect) => tick - effect.tick < BigInt(effect.definition.lifetimeTicks))
      return Object.freeze(effects.map((effect) => {
        const age = Number(tick - effect.tick)
        const fraction = age / effect.definition.lifetimeTicks
        return Object.freeze({
          identity: effect.identity,
          position: effect.position,
          radius: effect.definition.startRadius
            + (effect.definition.endRadius - effect.definition.startRadius) * fraction,
          color: effect.definition.color,
          opacity: effect.definition.startOpacity
            + (effect.definition.endOpacity - effect.definition.startOpacity) * fraction,
        })
      }))
    },
    reset(nextTick: bigint): void {
      if (disposed || typeof nextTick !== "bigint" || nextTick < 0n) {
        throw new ParticleError("MalformedEvent", "particle reset tick is invalid")
      }
      tick = nextTick
      effects = []
    },
    dispose(): void {
      disposed = true
      effects = []
    },
  })
}
