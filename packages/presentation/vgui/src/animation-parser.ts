/*
 * Source-facing behavior in this file is adapted from Valve Source SDK 2013
 * under the Source 1 SDK License. See ../LICENSE.source-sdk-2013 and the
 * repository's included thirdpartylegalnotices.txt.
 */
import type {
  VguiAnimationCommand,
  VguiAnimationInterpolator,
  VguiAnimationScript,
  VguiRelativeAlignment,
} from "./runtime-contract"

export type VguiAnimationParseLimits = Readonly<{
  maximumSourceBytes: number
  maximumTokenCodeUnits: number
  maximumSequences: number
  maximumCommands: number
}>

export type VguiAnimationParseResult =
  | Readonly<{ ok: true; script: VguiAnimationScript }>
  | Readonly<{
      ok: false
      diagnostic: Readonly<{
        code: "InvalidInput" | "MalformedSource" | "BoundExceeded" | "UnknownCommand"
        logicalIdentity: string
        token: number
        subject: string
      }>
    }>

const IDENTITY = /^[a-z0-9][a-z0-9./_-]{0,511}$/u
const FLOAT_PREFIX = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?/iu
const ALIGNMENTS = new Set([
  "northwest", "north", "northeast", "west", "center", "east", "southwest", "south", "southeast",
  "nw", "n", "ne", "w", "c", "e", "sw", "s", "se",
])

class ParseFault extends Error {
  constructor(
    readonly code: Extract<VguiAnimationParseResult, { ok: false }>["diagnostic"]["code"],
    readonly token: number,
    readonly subject: string,
  ) {
    super(subject)
  }
}

function validLimits(limits: VguiAnimationParseLimits): boolean {
  return !!limits
    && Object.values(limits).every((value) => Number.isSafeInteger(value) && value > 0)
}

function tokenize(source: string, limits: VguiAnimationParseLimits): string[] {
  const tokens: string[] = []
  let offset = 0
  const push = (value: string) => {
    if (value.length > limits.maximumTokenCodeUnits) throw new ParseFault("BoundExceeded", tokens.length, "token")
    tokens.push(value)
  }
  while (offset < source.length) {
    const character = source[offset]!
    if (/\s/u.test(character)) { offset += 1; continue }
    if (character === "/" && source[offset + 1] === "/") {
      offset += 2
      while (offset < source.length && source[offset] !== "\n" && source[offset] !== "\r") offset += 1
      continue
    }
    if (character === "{" || character === "}") { push(character); offset += 1; continue }
    if (character === "[") {
      const end = source.indexOf("]", offset + 1)
      if (end < 0) throw new ParseFault("MalformedSource", tokens.length, "condition")
      push(source.slice(offset, end + 1))
      offset = end + 1
      continue
    }
    if (character === '"') {
      offset += 1
      let value = ""
      let closed = false
      while (offset < source.length) {
        const item = source[offset++]!
        if (item === '"') { closed = true; break }
        if (item === "\\" && offset < source.length) {
          const escaped = source[offset++]!
          value += escaped === "n" ? "\n" : escaped === "t" ? "\t" : escaped
        } else value += item
      }
      if (!closed) throw new ParseFault("MalformedSource", tokens.length, "quoted-token")
      push(value)
      continue
    }
    const start = offset
    while (offset < source.length && !/\s|[{}]/u.test(source[offset]!)) {
      if (source[offset] === "/" && source[offset + 1] === "/") break
      offset += 1
    }
    if (start === offset) throw new ParseFault("MalformedSource", tokens.length, "token")
    push(source.slice(start, offset))
  }
  return tokens
}

function isCondition(value: string | undefined): value is string {
  const folded = value?.toLowerCase() ?? ""
  return folded.includes("[$") || folded.includes("[!$")
}

function number(value: string, token: number, subject: string): number {
  const match = FLOAT_PREFIX.exec(value)
  if (!match) throw new ParseFault("MalformedSource", token, `${subject}:${value}`)
  const parsed = Number(match[0])
  if (!Number.isFinite(parsed)) throw new ParseFault("MalformedSource", token, `${subject}:${value}`)
  return parsed
}

function boolean(value: string, token: number, subject: string): boolean {
  if (!/^[+-]?\d+$/u.test(value)) throw new ParseFault("MalformedSource", token, subject)
  return Number.parseInt(value, 10) !== 0
}

function targetAndRelative(value: string, token: number): Readonly<{
  target: string
  relative?: Readonly<{ panel: string; alignment: VguiRelativeAlignment }>
}> {
  let relative: Readonly<{ panel: string; alignment: VguiRelativeAlignment }> | undefined
  const parts = value.trim().split(/\s+/u).map((part) => {
    if (!part.startsWith("(")) return part
    const match = /^\(([^:()]+):([^()]+)\)(.*)$/u.exec(part)
    if (!match || !ALIGNMENTS.has(match[1]!.toLowerCase()) || !match[2]) {
      throw new ParseFault("MalformedSource", token, "relative-position")
    }
    relative = Object.freeze({ panel: match[2], alignment: match[1].toLowerCase() as VguiRelativeAlignment })
    return match[3] || "0"
  })
  return Object.freeze({ target: parts.join(" "), ...(relative ? { relative } : {}) })
}

export function parseVguiAnimationScript(
  logicalIdentity: string,
  revision: string,
  bytes: Uint8Array,
  limits: VguiAnimationParseLimits,
): VguiAnimationParseResult {
  if (!IDENTITY.test(logicalIdentity) || typeof revision !== "string" || revision.length === 0
    || !(bytes instanceof Uint8Array) || !validLimits(limits)) {
    return Object.freeze({
      ok: false,
      diagnostic: Object.freeze({ code: "InvalidInput", logicalIdentity, token: 0, subject: "input" }),
    })
  }
  if (bytes.byteLength > limits.maximumSourceBytes) {
    return Object.freeze({
      ok: false,
      diagnostic: Object.freeze({ code: "BoundExceeded", logicalIdentity, token: 0, subject: "source-bytes" }),
    })
  }
  try {
    const source = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
    const tokens = tokenize(source, limits)
    let index = 0
    let commandCount = 0
    const sequences: VguiAnimationScript["sequences"][number][] = []
    const next = (subject: string): string => {
      const value = tokens[index]
      if (value === undefined) throw new ParseFault("MalformedSource", index, subject)
      index += 1
      return value
    }
    const delayed = (kind: VguiAnimationCommand["kind"], fields: Record<string, unknown>, delaySeconds: number): VguiAnimationCommand =>
      Object.freeze({ kind, ...fields, delaySeconds, condition: null }) as VguiAnimationCommand
    while (index < tokens.length) {
      if (next("event") .toLowerCase() !== "event") throw new ParseFault("MalformedSource", index - 1, "event")
      const name = next("event-name")
      let sequenceCondition: string | null = null
      if (isCondition(tokens[index])) sequenceCondition = next("event-condition")
      if (next("event-open") !== "{") throw new ParseFault("MalformedSource", index - 1, "event-open")
      const commands: VguiAnimationCommand[] = []
      while (tokens[index] !== "}") {
        const commandToken = index
        const kind = next("command")
        const folded = kind.toLowerCase()
        let command: VguiAnimationCommand
        if (folded === "animate") {
          const panel = next("animate-panel")
          const variable = next("animate-variable")
          const target = targetAndRelative(next("animate-target"), index - 1)
          const interpolationToken = next("animate-interpolator")
          const interpolationFolded = interpolationToken.toLowerCase()
          const interpolationNames: Record<string, VguiAnimationInterpolator> = {
            accel: "Accel", deaccel: "Deaccel", spline: "Spline", pulse: "Pulse", flicker: "Flicker",
            bias: "Bias", gain: "Gain", bounce: "Bounce", linear: "Linear",
          }
          const interpolator = interpolationNames[interpolationFolded] ?? "Linear"
          const parameter = ["pulse", "bias", "gain", "flicker"].includes(interpolationFolded)
            ? number(next("animate-parameter"), index - 1, "animate-parameter")
            : 0
          command = Object.freeze({
            kind: "Animate",
            panel,
            variable,
            target: target.target,
            interpolator,
            parameter,
            delaySeconds: number(next("animate-delay"), index - 1, "animate-delay"),
            durationSeconds: number(next("animate-duration"), index - 1, "animate-duration"),
            ...(target.relative ? { relative: target.relative } : {}),
            condition: null,
          })
        } else if (folded === "runevent") {
          command = delayed("RunEvent", { sequence: next("run-event") }, number(next("run-event-delay"), index - 1, "run-event-delay"))
        } else if (folded === "runeventchild") {
          command = delayed("RunEventChild", { child: next("run-event-child"), sequence: next("run-event-child-sequence") }, number(next("run-event-child-delay"), index - 1, "run-event-child-delay"))
        } else if (folded === "firecommand") {
          const delaySeconds = number(next("fire-command-delay"), index - 1, "fire-command-delay")
          command = delayed("FireCommand", { command: next("fire-command") }, delaySeconds)
        } else if (folded === "playsound") {
          const delaySeconds = number(next("play-sound-delay"), index - 1, "play-sound-delay")
          command = delayed("PlaySound", { sound: next("play-sound") }, delaySeconds)
        } else if (folded === "setvisible") {
          command = delayed("SetVisible", { panel: next("visible-panel"), visible: boolean(next("visible-value"), index - 1, "visible-value") }, number(next("visible-delay"), index - 1, "visible-delay"))
        } else if (folded === "setinputenabled") {
          command = delayed("SetInputEnabled", { panel: next("input-panel"), enabled: boolean(next("input-value"), index - 1, "input-value") }, number(next("input-delay"), index - 1, "input-delay"))
        } else if (folded === "stopevent") {
          command = delayed("StopEvent", { sequence: next("stop-event") }, number(next("stop-event-delay"), index - 1, "stop-event-delay"))
        } else if (folded === "stoppanelanimations") {
          command = delayed("StopPanelAnimations", { panel: next("stop-panel") }, number(next("stop-panel-delay"), index - 1, "stop-panel-delay"))
        } else if (folded === "stopanimation") {
          command = delayed("StopAnimation", { panel: next("stop-animation-panel"), variable: next("stop-animation-variable") }, number(next("stop-animation-delay"), index - 1, "stop-animation-delay"))
        } else if (folded === "setfont") {
          command = delayed("SetFont", { panel: next("font-panel"), variable: next("font-variable"), font: next("font-value") }, number(next("font-delay"), index - 1, "font-delay"))
        } else if (folded === "settexture") {
          command = delayed("SetTexture", { panel: next("texture-panel"), variable: next("texture-variable"), texture: next("texture-value") }, number(next("texture-delay"), index - 1, "texture-delay"))
        } else if (folded === "setstring") {
          command = delayed("SetString", { panel: next("string-panel"), variable: next("string-variable"), value: next("string-value") }, number(next("string-delay"), index - 1, "string-delay"))
        } else throw new ParseFault("UnknownCommand", commandToken, kind)
        if (isCondition(tokens[index])) command = Object.freeze({ ...command, condition: next("command-condition") })
        commands.push(command)
        commandCount += 1
        if (commandCount > limits.maximumCommands) throw new ParseFault("BoundExceeded", index, "commands")
      }
      next("event-close")
      sequences.push(Object.freeze({ name, condition: sequenceCondition, commands: Object.freeze(commands) }))
      if (sequences.length > limits.maximumSequences) throw new ParseFault("BoundExceeded", index, "sequences")
    }
    return Object.freeze({
      ok: true,
      script: Object.freeze({ logicalIdentity, revision, sequences: Object.freeze(sequences) }),
    })
  } catch (error) {
    const fault = error instanceof ParseFault
      ? error
      : new ParseFault("MalformedSource", 0, error instanceof Error ? error.message : "source")
    return Object.freeze({
      ok: false,
      diagnostic: Object.freeze({ code: fault.code, logicalIdentity, token: fault.token, subject: fault.subject }),
    })
  }
}
