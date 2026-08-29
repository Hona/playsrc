import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { writeFile } from "node:fs/promises"
import path from "node:path"
import type { Page } from "@playwright/test"
import { CLASS_INPUT_SEQUENCE, classInputViolations, prepareClassCapture } from "./class-input-sequence"

export const CLASS_CORRECTNESS_PLAN = Object.freeze({
  schema: "playsrc-class-input-correctness-v1", target: "pl_upward", entry: "training",
  purpose: "functional-only", performanceSample: false, cpuSampling: false, heapSampling: false, nativeTrace: false,
  classes: [...CLASS_INPUT_SEQUENCE, ...CLASS_INPUT_SEQUENCE], windowSizes: [6, 6, 6], maximumWindowMilliseconds: 10_000,
})

/** Existing Rust reply and renderer-completion hooks, with no debugger or input interception. */
export function installClassCorrectnessObserver(host: any = globalThis) {
  const state = {
    active: false, action: "none", events: [] as any[],
    before() {
      const records = host.__playsrcFrameProfiler.simulation
      return { index: records.length, hostTick: records.at(-1)?.replayAttack?.hostTick ?? "0" }
    },
    acknowledgement(before: { index: number; hostTick: string }, identity: number) {
      for (const record of host.__playsrcFrameProfiler.simulation.slice(before.index)) {
        const attack = record.replayAttack
        if (!attack || attack.playerClass !== identity || attack.lifecycle !== 1 || BigInt(attack.hostTick) <= BigInt(before.hostTick)) continue
        for (const publication of record.publications) {
          if (!publication.selectedTicks || publication.player.playerClass !== identity
            || BigInt(attack.hostTick) < BigInt(publication.firstHostTick) || BigInt(attack.hostTick) > BigInt(publication.lastHostTick)) continue
          if (!publication.weapons.some((weapon: any) => weapon.weapon === attack.weapon)) continue
          return { ...attack, requestId: record.requestId, at: record.at, playerTick: publication.player.tick }
        }
      }
      return null
    },
    prefire(before: { index: number }) {
      for (const record of host.__playsrcFrameProfiler.simulation.slice(before.index)) {
        for (const publication of record.publications) {
          const activity = publication.activities.find((activity: any) => activity.weapon === 9 && activity.activity === 12)
          if (publication.player.playerClass === 6 && activity) return { ...activity, requestId: record.requestId }
        }
      }
      return null
    },
  }
  const event = (phase: string, detail: object) => { if (state.active) state.events.push({ at: host.performance.now(), phase, controllerAction: state.action, ...detail }) }
  host.document.addEventListener("keydown", (value: KeyboardEvent) => event("key-down", { key: value.code, trusted: value.isTrusted }), true)
  host.document.addEventListener("pointerdown", (value: PointerEvent) => {
    if (!(value.target as Element)?.matches?.("canvas.world-canvas")) return
    event(value.button !== 0 ? "other-pointer-button" : host.document.pointerLockElement === value.target ? "weapon-fire" : "pointer-capture",
      { button: value.button, trusted: value.isTrusted })
  }, true)
  host.__playsrcClassCorrectness = state
  return state
}

export async function verifyClassInputs(page: Page, directory: string, checkNative: () => Promise<void>) {
  const records: any[] = [], windows: any[] = []
  const root = page.locator("main"), canvas = page.locator("canvas.world-canvas")
  const generation = await root.getAttribute("data-generation")
  const action = (value: string) => page.evaluate(value => { (globalThis as any).__playsrcClassCorrectness.action = value }, value)
  let lastCapture = 0, position = 0
  let error: string | null = null
  try {
    await page.evaluate(() => { (globalThis as any).__playsrcClassCorrectness.active = true })
    for (const count of CLASS_CORRECTNESS_PLAN.windowSizes) {
      const started = performance.now(), deadline = started + CLASS_CORRECTNESS_PLAN.maximumWindowMilliseconds
      const remaining = () => Math.max(1, deadline - performance.now())
      const first = position
      await page.evaluate(() => {
        const profile = (globalThis as any).__playsrcFrameProfiler
        profile.completedFrames = []; profile.simulation = []; profile.simulationDropped = 0; profile.active = true
      })
      try {
        for (let offset = 0; offset < count; offset++, position++) {
          const selected = CLASS_CORRECTNESS_PLAN.classes[position]!
          await checkNative()
          assert.equal(await root.getAttribute("data-generation"), generation, "class input replaced the application generation")
          assert(await prepareClassCapture({
            earliestCapture: position >= 4 && (position - 4) % 5 === 0 ? lastCapture + 2100 : 0,
            deadline, now: () => performance.now(), delay: milliseconds => page.waitForTimeout(milliseconds),
            select: async () => {
              await action("select")
              try {
                await page.keyboard.press("Comma")
                await page.waitForFunction(() => document.querySelector<HTMLElement>("main")!.dataset.classSelectionVisible === "true", null, { timeout: remaining() })
                await page.keyboard.press(`Digit${selected.digit}`)
                await page.waitForFunction(identity => (document.querySelector<HTMLElement>("main")!.dataset.hudProbe ?? "").split(":")[1] === String(identity), selected.identity, { timeout: remaining() })
                const selectedTick = Number(await root.getAttribute("data-snapshot-tick"))
                // The existing authored Bottle path, not an unavailable rigid-body shot.
                if (selected.identity === 4) await page.keyboard.press("Digit3")
                await page.waitForFunction(({ identity, selectedTick }) => {
                  const main = document.querySelector<HTMLElement>("main")!, hud = (main.dataset.hudProbe ?? "").split(":")
                  const frame = (globalThis as any).__playsrcFrameProfiler.completedFrames.at(-1)
                  return main.dataset.phase === "Ready" && main.dataset.classSelectionVisible === "false"
                    && hud[1] === String(identity) && (identity !== 4 || hud[2] === "17")
                    && frame?.playerClass === identity && frame.tick >= selectedTick
                    && (identity !== 4 || frame.weapon === 17)
                }, { identity: selected.identity, selectedTick }, { timeout: remaining() })
                return true
              } finally { await action("none") }
            },
          }), "bounded class selection/capture deadline")
          assert.equal(await page.evaluate(() => document.pointerLockElement !== null), false, "unowned pointer capture")
          await action("capture")
          const bounds = await canvas.boundingBox()
          assert(bounds, "visible class surface")
          await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
          await page.waitForFunction(() => document.pointerLockElement === document.querySelector("canvas.world-canvas"), null, { timeout: remaining() })
          lastCapture = performance.now()
          const before = await page.evaluate(() => (globalThis as any).__playsrcClassCorrectness.before())
          await action("attack")
          let acknowledgement: any, prefire: any
          try {
            await page.mouse.down()
            const admitted = await page.waitForFunction(({ before, identity }) => (globalThis as any).__playsrcClassCorrectness.acknowledgement(before, identity),
              { before, identity: selected.identity }, { timeout: remaining() })
            acknowledgement = await admitted.jsonValue(); await admitted.dispose()
            if (selected.identity === 6) {
              const spun = await page.waitForFunction(before => (globalThis as any).__playsrcClassCorrectness.prefire(before), before, { timeout: remaining() })
              prefire = await spun.jsonValue(); await spun.dispose()
            }
          } finally { await page.mouse.up(); await action("none") }
          const tick = prefire?.tick ?? acknowledgement.playerTick
          const completed = await page.waitForFunction(({ identity, weapon, tick }) => {
            const frame = (globalThis as any).__playsrcFrameProfiler.completedFrames.at(-1)
            return frame?.playerClass === identity && frame.weapon === weapon && BigInt(frame.tick) >= BigInt(tick)
              ? { at: frame.at, displayFrame: frame.displayFrame, tick: frame.tick, playerClass: frame.playerClass,
                weapon: frame.weapon, drawSurfaces: frame.drawSurfaces, preparedRevision: frame.preparedRevision } : null
          }, { identity: selected.identity, weapon: acknowledgement.weapon, tick }, { timeout: remaining() })
          const frame = await completed.jsonValue(); await completed.dispose()
          await checkNative()
          const file = `class-${String(position + 1).padStart(2, "0")}-${selected.identity}.png`
          const pixels = await canvas.screenshot({ timeout: remaining() })
          assert.equal(pixels.subarray(1, 4).toString(), "PNG")
          await writeFile(path.join(directory, file), pixels)
          assert.equal((await root.getAttribute("data-hud-probe"))?.split(":")[1], String(selected.identity))
          records.push({ position, ...selected, generation, acknowledgement, prefire: prefire ?? null, frame,
            image: { file, bytes: pixels.length, sha256: createHash("sha256").update(pixels).digest("hex"),
              width: pixels.readUInt32BE(16), height: pixels.readUInt32BE(20) } })
          assert(performance.now() <= deadline, "correctness window exceeded ten seconds")
        }
      } finally {
        windows.push({ first, requested: count, completed: records.length - first, elapsedMilliseconds: performance.now() - started,
          observations: await page.evaluate(() => {
            const profile = (globalThis as any).__playsrcFrameProfiler
            profile.active = false
            return { frames: profile.completedFrames.map((frame: any) => ({ at: frame.at, displayFrame: frame.displayFrame,
              tick: frame.tick, playerClass: frame.playerClass, weapon: frame.weapon, drawSurfaces: frame.drawSurfaces,
              preparedRevision: frame.preparedRevision })), simulation: profile.simulation, dropped: profile.simulationDropped }
          }) })
      }
    }
    const events = await page.evaluate(() => (globalThis as any).__playsrcClassCorrectness.events)
    assert.deepEqual(classInputViolations(events), [], "unplanned native class input")
    assert.equal(events.filter((value: any) => value.phase === "weapon-fire").length, 18, "trusted primary presses")
    assert.equal(records.length, 18, "authoritative primary acknowledgements and visible class frames")
    assert(records.every((record, index) => index === 0 || BigInt(record.acknowledgement.hostTick) > BigInt(records[index - 1].acknowledgement.hostTick)), "reused authoritative attack acknowledgement")
    assert.equal(records.filter((record, index) => record.identity === 6 && record.prefire && records[index + 1]?.identity === 7).length, 2, "both real Minigun prefire-to-Pyro transitions")
    assert(windows.every(window => window.observations.dropped === 0), "dropped authoritative input evidence")
    assert.equal(await root.getAttribute("data-phase"), "Ready")
    return { plan: CLASS_CORRECTNESS_PLAN, records, windows, events, generation }
  } catch (failure) { error = String(failure); throw failure }
  finally {
    await page.mouse.up().catch(() => undefined)
    await action("none").catch(() => undefined)
    const events = await page.evaluate(() => {
      const observer = (globalThis as any).__playsrcClassCorrectness
      observer.active = false
      return observer.events
    }).catch(() => [])
    await writeFile(path.join(directory, "class-input-correctness.json"), JSON.stringify({ plan: CLASS_CORRECTNESS_PLAN, records, windows, events, generation, error }, null, 2))
  }
}
