import { describe, expect, test } from "bun:test"
import { createTf2StartupController, type Tf2HiddenMenu, type Tf2StartupMediaSession, type Tf2StartupPolicy } from "../../src/startup-presentation"

const tick = async (): Promise<void> => { await Bun.sleep(0) }
const policy = (changed: Partial<Tf2StartupPolicy> = {}): Tf2StartupPolicy => ({ benchmark: false, editMode: false, forceVr: false, developer: false, noVideo: false, allowDebug: false, healthWarningPresent: false, ...changed })

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

function fixture(playResult: "started" | "gesture-required" = "started", selectedPolicy = policy()) {
  const menuReady = deferred<Tf2HiddenMenu>()
  const mediaReady = deferred<Tf2StartupMediaSession>()
  const calls: string[] = []
  let events!: Readonly<{ completed(): void; failed(reason: string): void }>
  const session: Tf2StartupMediaSession = {
    play: async () => { calls.push("play"); return playResult },
    admitGesture: async () => { calls.push("gesture"); return "started" },
    skip: () => calls.push("skip"),
    setVisible: (visible) => calls.push(`visible:${visible}`),
    destroy: () => calls.push("media:destroy"),
  }
  const menu: Tf2HiddenMenu = { reveal: () => calls.push("menu:reveal"), destroy: () => calls.push("menu:destroy") }
  const controller = createTf2StartupController({
    policy: selectedPolicy,
    clock: { nowMicroseconds: () => calls.length * 1_000 },
    media: { prepare: async (_descriptor, nextEvents) => { calls.push("media:prepare"); events = nextEvents; return mediaReady.promise } },
    menu: { prepareHidden: async () => { calls.push("menu:prepare-hidden"); return menuReady.promise } },
  })
  return { controller, calls, events: () => events, mediaReady, menuReady, session, menu }
}

describe("TF2 startup presentation lifecycle", () => {
  test("prepares the menu concurrently and reveals it after normal completion", async () => {
    const f = fixture()
    f.controller.start()
    expect(f.calls).toEqual(["menu:prepare-hidden", "media:prepare"])
    f.mediaReady.resolve(f.session)
    f.menuReady.resolve(f.menu)
    await tick()
    expect(f.controller.state()).toEqual({ kind: "Playing" })
    f.events().completed()
    expect(f.controller.state()).toEqual({ kind: "Completed" })
    expect(f.calls.slice(-2)).toEqual(["media:destroy", "menu:reveal"])
  })

  test("waits without exposing a pending menu after completion or Escape", async () => {
    for (const finish of ["complete", "skip"] as const) {
      const f = fixture()
      f.controller.start(); f.mediaReady.resolve(f.session); await tick()
      if (finish === "complete") f.events().completed()
      else f.controller.key("Escape")
      expect(f.controller.state()).toEqual({ kind: "WaitingForMenu", movieResult: finish === "complete" ? "Completed" : "Skipped" })
      expect(f.calls).not.toContain("menu:reveal")
      f.menuReady.resolve(f.menu); await tick()
      expect(f.controller.state()).toEqual(finish === "complete" ? { kind: "Completed" } : { kind: "Skipped", reason: "Escape" })
    }
  })

  test("requires a gesture without silently completing or muting playback", async () => {
    const f = fixture("gesture-required")
    f.controller.start(); f.mediaReady.resolve(f.session); f.menuReady.resolve(f.menu); await tick()
    expect(f.controller.state()).toEqual({ kind: "AwaitingGesture" })
    f.controller.visibility(false)
    f.controller.key("Enter")
    expect(f.calls).toContain("visible:false")
    expect(f.calls).not.toContain("skip")
    f.controller.gesture(); await tick()
    expect(f.controller.state()).toEqual({ kind: "Playing" })
  })

  test("applies every startup suppression input and health-warning exception", async () => {
    for (const flag of ["benchmark", "editMode", "forceVr", "developer", "noVideo", "allowDebug"] as const) {
      const f = fixture("started", policy({ [flag]: true }))
      f.controller.start(); f.menuReady.resolve(f.menu); await tick()
      expect(f.controller.state()).toEqual({ kind: "Skipped", reason: "Policy" })
      expect(f.calls).not.toContain("media:prepare")
    }
    const admitted = fixture("started", policy({ developer: true, healthWarningPresent: true }))
    admitted.controller.start()
    expect(admitted.calls).toContain("media:prepare")
  })

  test("keeps preparation and playback failures typed", async () => {
    const preparation = fixture()
    preparation.controller.start(); preparation.mediaReady.reject(new Error("decode")); await tick()
    expect(preparation.controller.state()).toEqual({ kind: "Failed", stage: "MediaPreparation", reason: "decode" })

    const playback = fixture()
    playback.controller.start(); playback.mediaReady.resolve(playback.session); await tick(); playback.events().failed("device")
    expect(playback.controller.state()).toEqual({ kind: "Failed", stage: "Playback", reason: "device" })
  })

  test("destroys every acquired owner once and rejects stale completions", async () => {
    const f = fixture()
    f.controller.start(); f.mediaReady.resolve(f.session); f.menuReady.resolve(f.menu); await tick()
    f.controller.destroy(); f.controller.destroy(); f.events().completed()
    expect(f.controller.state()).toEqual({ kind: "Destroyed" })
    expect(f.calls.filter((value) => value === "media:destroy")).toHaveLength(1)
    expect(f.calls.filter((value) => value === "menu:destroy")).toHaveLength(1)

    const stale = fixture()
    stale.controller.start(); stale.controller.destroy(); stale.mediaReady.resolve(stale.session); stale.menuReady.resolve(stale.menu); await tick()
    expect(stale.calls.filter((value) => value === "media:destroy")).toHaveLength(1)
    expect(stale.calls.filter((value) => value === "menu:destroy")).toHaveLength(1)
  })
})
