import { readFile } from "node:fs/promises"
import { runInNewContext } from "node:vm"
import { expect, test } from "bun:test"
import { Tf2BrowserAutomation, type Tf2BrowserAutomationTransport } from "../src/browser-automation"

type RecordedCall = readonly [string, ...unknown[]]

function transport(
  evaluate: (expression: string) => unknown,
): Readonly<{ calls: RecordedCall[]; driver: Tf2BrowserAutomationTransport }> {
  const calls: RecordedCall[] = []
  return {
    calls,
    driver: {
      evaluate: async <T>(expression: string): Promise<T> => {
        calls.push(["evaluate", expression])
        return evaluate(expression) as T
      },
      press: async (key) => { calls.push(["press", key]) },
      click: async (selector) => { calls.push(["click", selector]) },
      focus: async (selector) => { calls.push(["focus", selector]) },
      fill: async (selector, value) => { calls.push(["fill", selector, value]) },
      waitFor: async (expression, timeout) => { calls.push(["wait", expression, timeout]) },
      activateCurrentTab: async () => { calls.push(["activate-tab"]) },
    },
  }
}

test("semantic TF2 console and map operations preserve the actual VGUI submission path", async () => {
  const value = transport(() => false)
  const automation = new Tf2BrowserAutomation(value.driver)
  await automation.maps.load("jump_beef")
  expect(value.calls.map((call) => call[0])).toEqual(["evaluate", "press", "click", "wait", "fill", "press"])
  expect(value.calls[1]).toEqual(["press", "Backquote"])
  expect(value.calls[4]).toEqual(["fill", "[aria-label='Console command']", "map jump_beef"])
  expect(value.calls[5]).toEqual(["press", "Enter"])

  value.calls.length = 0
  await automation.player.selectClass("demoman")
  expect(value.calls.find((call) => call[0] === "fill")).toEqual([
    "fill", "[aria-label='Console command']", "class demoman",
  ])
})

test("semantic TF2 commands reject malformed identities and the exact console-byte ceiling", async () => {
  const value = transport(() => false)
  const automation = new Tf2BrowserAutomation(value.driver)
  await expect(automation.console.submitCommand("")).rejects.toThrow("console command is invalid")
  await expect(automation.console.submitCommand("status\nmap jump_beef")).rejects.toThrow("console command is invalid")
  await expect(automation.console.submitCommand("a".repeat(256))).rejects.toThrow("console command is invalid")
  await expect(automation.maps.load("../jump_beef")).rejects.toThrow("map identity is invalid")
  await expect(automation.player.lookBy({ x: Number.NaN, y: 0 })).rejects.toThrow("pointer movement is invalid")
  await expect(automation.player.settle(0)).rejects.toThrow("stationary tick bound is invalid")
  await expect(automation.player.walkForward(0)).rejects.toThrow("movement tick bound is invalid")
  expect(value.calls).toEqual([])
})

test("headed TF2 automation preserves and reports an available native pointer lock", async () => {
  const value = transport((expression) => expression.includes("lockOwnerMatches")
    ? { locked: true, lockOwnerMatches: true, focused: true, detail: "Audio running", gameUi: "in-game", mode: "native" }
    : true)
  const automation = new Tf2BrowserAutomation(value.driver)
  expect(await automation.pointer.capture("direction")).toEqual({ mode: "native", native: "available" })
  expect(value.calls.some((call) => call[0] === "evaluate" && String(call[1]).includes("__playsrcBrowserTestPointer?.mode===\"emulated\""))).toBe(false)
})

test("headed TF2 automation installs a labeled page-only adapter only for native root-focus failure", async () => {
  const reason = "Pointer lock failed: WrongDocumentError:The root document of this element is not valid for pointer lock.:activation=true"
  let installed = false
  const value = transport((expression) => {
    if (expression.includes("__playsrcBrowserTestPointer?.mode===\"emulated\"")) {
      installed = true
      return true
    }
    if (expression.includes("lockOwnerMatches")) {
      return installed
        ? { locked: true, lockOwnerMatches: true, focused: true, detail: "Audio running", gameUi: "in-game", mode: "emulated" }
        : { locked: false, lockOwnerMatches: false, focused: true, detail: reason, gameUi: "in-game", mode: "native" }
    }
    return true
  })
  const automation = new Tf2BrowserAutomation(value.driver)
  expect(await automation.pointer.capture("direction")).toEqual({
    mode: "emulated",
    native: "unavailable",
    unavailableReason: reason,
  })
  expect(installed).toBe(true)
  expect(value.calls.filter((call) => call[0] === "click" && call[1] === ".world-canvas")).toHaveLength(2)
  await automation.pointer.release()
  expect(value.calls.at(-1)).toEqual(["wait", "document.pointerLockElement===null", 10_000])
})

test("semantic TF2 stationary capture waits for exact repeated authoritative positions", async () => {
  const value = transport(() => ({ tick: 84, position: [100, 200, 300] }))
  const automation = new Tf2BrowserAutomation(value.driver)
  expect(await automation.player.settle(3)).toEqual({ tick: 84, position: [100, 200, 300] })
  const expression = String(value.calls[0]?.[1])
  expect(expression).toContain("position===previousPosition")
  expect(expression).toContain("stableTicks>=3")
  expect(expression).toContain('root.dataset.grounded==="true"')
})

test("semantic TF2 player movement retains signed physical events and bounded authoritative ticks", async () => {
  let observations = 0
  const value = transport((expression) => {
    if (expression.includes("lockOwnerMatches")) {
      return { locked: true, lockOwnerMatches: true, focused: true, detail: "Audio running", gameUi: "in-game", mode: "native" }
    }
    if (expression.includes("displayMouseRevision")) {
      return { position: [10, 20, 30], yaw: 175.776, pitch: -3.112, verticalFov: 60, near: 7, far: 1024 }
    }
    if (expression.includes("position:value.cameraPosition")) {
      observations += 1
      return observations === 1
        ? { tick: 40, position: [10, 20, 30] }
        : { tick: 43, position: [13, 24, 30] }
    }
    return true
  })
  const automation = new Tf2BrowserAutomation(value.driver)
  expect(await automation.player.lookBy({ x: 64, y: -32 })).toEqual({
    position: [10, 20, 30], yaw: 175.776, pitch: -3.112, verticalFov: 60, near: 7, far: 1024,
  })
  const look = String(value.calls.at(-1)?.[1])
  expect(look).toContain("movementX:{value:64}")
  expect(look).toContain("movementY:{value:-32}")
  expect(await automation.player.walkForward(3)).toEqual({
    firstTick: 40,
    lastTick: 43,
    before: [10, 20, 30],
    after: [13, 24, 30],
    distance: 5,
  })
  expect(value.calls.some((call) => String(call[1]).includes("dataset.snapshotTick)>=43"))).toBe(true)
  expect(value.calls.some((call) => String(call[1]).includes("KeyboardEvent('keyup'"))).toBe(true)
  await automation.player.pressPrimaryFire()
  await automation.player.releasePrimaryFire()
  expect(String(value.calls.at(-2)?.[1])).toContain("MouseEvent('mousedown',{button:0,buttons:1")
  expect(String(value.calls.at(-1)?.[1])).toContain("MouseEvent('mouseup',{button:0,buttons:0")
  await automation.player.jump()
  expect(String(value.calls.at(-1)?.[1])).toContain("code:'Space'")
})

test("headed automation ignores real user gameplay input without blocking scripted commands or UI keys", async () => {
  const handlers = new Map<string, (event: MockInputEvent) => void>()
  const canvas = {}
  const root = { dataset: { phase: "Ready", gameui: "in-game" } }
  const document = {
    pointerLockElement: canvas as object | null,
    querySelector: (selector: string) => selector === "main" ? root : selector === "canvas.world-canvas" ? canvas : null,
  }
  class MockInputEvent {
    prevented = false
    stopped = false
    constructor(readonly isTrusted: boolean, readonly code = "") {}
    preventDefault(): void { this.prevented = true }
    stopImmediatePropagation(): void { this.stopped = true }
  }
  class MockKeyboardEvent extends MockInputEvent {}
  const context = {
    window: { addEventListener: (type: string, handler: (event: MockInputEvent) => void) => handlers.set(type, handler) },
    document,
    KeyboardEvent: MockKeyboardEvent,
  }
  runInNewContext(await readFile(new URL("../src/browser-automation-init.js", import.meta.url), "utf8"), context)

  const physicalMouse = new MockInputEvent(true)
  handlers.get("mousemove")!(physicalMouse)
  expect(physicalMouse.prevented).toBe(true)
  expect(physicalMouse.stopped).toBe(true)

  const scriptedMouse = new MockInputEvent(false)
  handlers.get("mousemove")!(scriptedMouse)
  expect(scriptedMouse.stopped).toBe(false)

  const physicalForward = new MockKeyboardEvent(true, "KeyW")
  handlers.get("keydown")!(physicalForward)
  expect(physicalForward.stopped).toBe(true)

  const consoleToggle = new MockKeyboardEvent(true, "Backquote")
  handlers.get("keydown")!(consoleToggle)
  expect(consoleToggle.stopped).toBe(false)

  document.pointerLockElement = null
  const unlockedMouse = new MockInputEvent(true)
  handlers.get("mousemove")!(unlockedMouse)
  expect(unlockedMouse.stopped).toBe(false)

  root.dataset.gameui = "main-menu"
  const menuTyping = new MockKeyboardEvent(true, "KeyW")
  handlers.get("keydown")!(menuTyping)
  expect(menuTyping.stopped).toBe(false)
})
