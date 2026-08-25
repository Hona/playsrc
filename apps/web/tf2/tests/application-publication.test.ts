import { describe, expect, test } from "bun:test"
import { ApplicationPublication } from "../src/application-publication"
import {
  GAME_UI_FRAME_OWNER,
  HUD_FRAME_OWNER,
  LOADING_FRAME_OWNER,
  OPTIONS_FRAME_OWNER,
  visibleFrameOwners,
} from "../src/frame-owners"
import type { ApplicationView } from "../src/runtime"

class Element {
  readonly attributes = new Map<string, string>()
  readonly writes: string[] = []
  tabIndex = -1

  getAttribute(name: string): string | null { return this.attributes.get(name) ?? null }
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value)
    this.writes.push(`${name}=${value}`)
  }
  removeAttribute(name: string): void {
    this.attributes.delete(name)
    this.writes.push(`-${name}`)
  }
}

function fixture() {
  const root = new Element()
  const canvas = new Element()
  canvas.attributes.set("aria-hidden", "true")
  const loadingLabel = { textContent: "" }
  const publication = new ApplicationPublication({ root, canvas, loadingLabel })
  const startup: ApplicationView = Object.freeze({
    phase: "Startup",
    detail: "Preparing exact startup media",
    gameUi: "main-menu",
    pointerLocked: false,
    consoleVisible: false,
    blockers: Object.freeze([]),
    fireEvents: 0,
    explosionEvents: 0,
    bootstrapLoading: true,
    bootstrapProgress: 0,
  })
  return { publication, root, canvas, loadingLabel, startup }
}

describe("TF2 incremental application publication", () => {
  test("publishes exact initial lifecycle, zero-valued observations, and startup accessibility", () => {
    const value = fixture()
    value.publication.publish(value.startup)
    expect(Object.fromEntries(value.root.attributes)).toEqual({
      "data-phase": "Startup",
      "data-bootstrap-loading": "true",
      "data-bootstrap-progress": "0",
      "data-startup-muted-fallback": "false",
      "data-detail": "Preparing exact startup media",
      "data-gameui": "main-menu",
      "data-gameplay-initialized": "false",
      "data-pointer-locked": "false",
      "data-console-visible": "false",
      "data-class-selection-visible": "false",
      "data-options-visible": "false",
      "data-team-selection-visible": "false",
      "data-fire-events": "0",
      "data-explosion-events": "0",
      "data-particle-items": "0",
      "data-projectiles": "0",
      "data-environment-drawables": "0",
      "data-blockers": "[]",
    })
    expect(value.canvas.tabIndex).toBe(-1)
    expect(value.canvas.getAttribute("aria-hidden")).toBe("true")
    expect(value.loadingLabel.textContent).toBe("Loading 0%...")
  })

  test("preserves an already-mounted initial lifecycle without rewriting exact attributes", () => {
    const value = fixture()
    const initial = fixture()
    initial.publication.publish(initial.startup)
    for (const [name, encoded] of initial.root.attributes) value.root.attributes.set(name, encoded)
    value.loadingLabel.textContent = initial.loadingLabel.textContent
    value.publication.publish(value.startup)
    expect(value.root.writes).toEqual([])
    expect(value.canvas.writes).toEqual([])
    expect(value.root.attributes.get("data-phase")).toBe("Startup")
  })

  test("writes only changed exact values and preserves simulation-to-render mutation order", () => {
    const value = fixture()
    value.publication.publish(value.startup)
    value.root.writes.length = 0
    const ready: ApplicationView = Object.freeze({
      ...value.startup,
      phase: "Ready",
      generation: 4,
      gameUi: "in-game",
      detail: "Playing jump_beef",
      snapshotTick: "41",
      projectileStates: "5:flying,8:flying",
      fireEvents: 2,
      bootstrapProgress: 100,
      bootstrapLoading: false,
      camera: Object.freeze({
        position: Object.freeze([1, 2, 3]) as readonly [number, number, number],
        yawDegrees: 90,
        pitchDegrees: -10,
        verticalFovDegrees: 60,
        near: 7,
        far: 32768,
      }),
    })
    value.publication.publish(ready)
    expect(value.root.attributes.get("data-phase")).toBe("Ready")
    expect(value.root.attributes.get("data-generation")).toBe("4")
    expect(value.root.attributes.get("data-gameplay-initialized")).toBe("true")
    expect(value.root.attributes.get("data-projectiles")).toBe("2")
    expect(value.root.attributes.get("data-camera-position")).toBe("1,2,3")
    expect(value.root.attributes.get("data-camera-yaw")).toBe("90")
    expect(value.canvas.tabIndex).toBe(0)
    expect(value.canvas.getAttribute("aria-hidden")).toBe("false")
    expect(value.loadingLabel.textContent).toBe("Loading 100%...")
    expect(value.root.writes.indexOf("data-phase=Ready")).toBeLessThan(value.root.writes.indexOf("data-gameui=in-game"))
    expect(value.root.writes.some((write) => write.startsWith("data-blockers="))).toBe(false)

    value.root.writes.length = 0
    value.canvas.writes.length = 0
    value.publication.publish(Object.freeze({ ...ready }))
    expect(value.root.writes).toEqual([])
    expect(value.canvas.writes).toEqual([])
  })

  test("publishes independent in-water flags alongside canonical movement fluid state", () => {
    const value = fixture()
    const movement = Object.freeze({
      waterLevel: 3,
      waterType: 0x20,
      viewOffset: Object.freeze([0, 0, 68]),
      velocity: Object.freeze([0, 0, 0]),
    }) as ApplicationView["movement"]
    value.publication.publish(Object.freeze({
      ...value.startup,
      movement,
      playerFlags: 0x100,
      inWater: false,
    }))
    expect(value.root.attributes.get("data-water-level")).toBe("3")
    expect(value.root.attributes.get("data-water-type")).toBe("32")
    expect(value.root.attributes.get("data-player-flags")).toBe("256")
    expect(value.root.attributes.get("data-in-water")).toBe("false")
    value.publication.publish(Object.freeze({
      ...value.startup,
      movement,
      playerFlags: 0x500,
      inWater: true,
    }))
    expect(value.root.attributes.get("data-player-flags")).toBe("1280")
    expect(value.root.attributes.get("data-in-water")).toBe("true")
  })

  test("serializes immutable model and blocker identities once and removes replaced values", () => {
    const value = fixture()
    value.publication.publish(value.startup)
    let modelSerializations = 0
    const modelMatrices = Object.freeze([{
      entity: 7,
      model: "models/example.mdl",
      matrix: Object.freeze([1, 0, 0, 1]),
      toJSON() {
        modelSerializations += 1
        return { entity: this.entity, model: this.model, matrix: this.matrix }
      },
    }])
    const blockers = Object.freeze(["blocked source input"])
    const observed = Object.freeze({
      ...value.startup,
      modelMatrices,
      blockers,
      particleProbe: "sprite:exact:sheet",
    }) satisfies ApplicationView
    value.publication.publish(observed)
    expect(modelSerializations).toBe(1)
    expect(value.root.attributes.get("data-model-matrices")).toBe('[{"entity":7,"model":"models/example.mdl","matrix":[1,0,0,1]}]')
    expect(value.root.attributes.get("data-blockers")).toBe('["blocked source input"]')
    value.publication.publish(Object.freeze({ ...observed, snapshotTick: "42" }))
    expect(modelSerializations).toBe(1)
    value.publication.publish(Object.freeze({ ...observed, snapshotTick: "43", modelMatrices: undefined, particleProbe: undefined }))
    expect(value.root.attributes.has("data-model-matrices")).toBe(false)
    expect(value.root.attributes.has("data-particle-probe")).toBe(false)
  })

  test("preserves false, zero, empty-string, and unavailable attribute boundaries", () => {
    const value = fixture()
    value.publication.publish(Object.freeze({
      ...value.startup,
      waterStateRestored: false,
      waterNormalFrame: 0,
      particleProbe: "",
      audioStarts: Object.freeze([]),
      initialView: Object.freeze({
        entity: 3,
        hammerId: null,
        position: Object.freeze([0, 0, 0]) as readonly [number, number, number],
        angles: Object.freeze([0, 90, 0]) as readonly [number, number, number],
      }),
    }))
    expect(value.root.attributes.get("data-water-restored")).toBe("false")
    expect(value.root.attributes.get("data-water-normal-frame")).toBe("0")
    expect(value.root.attributes.get("data-particle-probe")).toBe("")
    expect(value.root.attributes.get("data-audio-starts")).toBe("")
    expect(value.root.attributes.has("data-spawn-hammer-id")).toBe(false)
  })
})

describe("TF2 visible presentation owners", () => {
  const select = (phase: ApplicationView["phase"], gameUi: ApplicationView["gameUi"], optionsVisible = false, revealed = true) =>
    visibleFrameOwners({ phase, gameUi, optionsVisible }, revealed)

  test("never advances unrevealed GameUI or hidden loading and Options owners", () => {
    expect(select("Startup", "main-menu", false, false)).toBe(0)
    expect(select("MainMenu", "main-menu")).toBe(GAME_UI_FRAME_OWNER)
    expect(select("Ready", "in-game")).toBe(HUD_FRAME_OWNER)
    expect(select("Ready", "in-game", true)).toBe(HUD_FRAME_OWNER | OPTIONS_FRAME_OWNER)
  })

  test("keeps exact loading, failure, pause, and closed owner boundaries", () => {
    expect(select("Loading", "loading")).toBe(GAME_UI_FRAME_OWNER | LOADING_FRAME_OWNER)
    expect(select("Failed", "failure")).toBe(GAME_UI_FRAME_OWNER | LOADING_FRAME_OWNER)
    expect(select("Ready", "pause")).toBe(GAME_UI_FRAME_OWNER | HUD_FRAME_OWNER)
    expect(select("Closed", "pause", true)).toBe(0)
  })
})
