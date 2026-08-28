import { decodeScreenshot } from "./screenshot-pixels"
import { startupDigest } from "./static-startup-package"

export type StartupObservation = {
  phase: string; detail: string; startupState?: string; visible: boolean; focused: boolean
  timeOrigin: number; at: number; frame: number; teamSelection: boolean; classSelection: boolean; unexpectedInput: number
  cache?: string
  consoleVisible?: boolean; gameUi?: string; playerClass?: number; tick?: string
  movie: null | { time: number; paused: boolean; muted: boolean; width: number; height: number }
}
export type StartupNativeAdmission = {
  at: number; physical: boolean; unlocked: boolean; foreground: boolean; visible: boolean; minimized: boolean
  idleMilliseconds: number; browserPid: number; windowId: number; targetId: string
}
export type StaticStartupDriver = {
  navigate(mode: "cold" | "warm-upgrade"): Promise<void>
  read(): Promise<StartupObservation>
  native(): Promise<StartupNativeAdmission>
  screenshot(label: string): Promise<Uint8Array>
  action(action: "play-intro" | "open-map" | "close-console" | "choose-team" | "choose-class", target?: string): Promise<void>
  wait(milliseconds: number): Promise<void>
}

export function startupPixelEvidence(bytes: Uint8Array) {
  const image = decodeScreenshot(Buffer.from(bytes))
  const colors = new Set<number>()
  let nonblack = 0
  for (let offset = 0; offset < image.pixels.length; offset += image.channels) {
    const color = image.pixels[offset]! << 16 | image.pixels[offset + 1]! << 8 | image.pixels[offset + 2]!
    if (color) nonblack++
    if (colors.size < 256) colors.add(color)
  }
  if (nonblack < 1024 || colors.size < 16) throw new Error("Static startup screenshot has no visible movie/menu/game detail")
  return { sha256: startupDigest(bytes), width: image.width, height: image.height, nonblack, colors: colors.size }
}

export function requireStartupNative(value: StartupNativeAdmission) {
  if (value.physical!==true || value.unlocked!==true || value.foreground!==true || value.visible!==true || value.minimized!==false
    || !Number.isSafeInteger(value.browserPid) || value.browserPid < 1 || !Number.isSafeInteger(value.windowId) || value.windowId < 1
    || !value.targetId || !Number.isFinite(value.idleMilliseconds) || value.idleMilliseconds<0) throw new Error("Static startup physical window admission failed")
}

export async function captureStaticStartup(driver: StaticStartupDriver, target: string) {
  const startedAt = Date.now(), deadline = startedAt + 145_000
  const runs: any[] = [], native: StartupNativeAdmission[] = []
  let terminal: StartupObservation | undefined
  const read = async () => {
    if (Date.now() >= deadline) throw new Error("Static startup capture exceeded its bounded wall clock")
    const state = await driver.read()
    terminal = state
    if (!state.visible || !state.focused || state.unexpectedInput) throw new Error("Static startup visibility/input guard failed")
    if (state.phase === "Failed" || state.startupState === "Failed") throw new Error(`Static startup failed: ${state.detail}`)
    return state
  }
  const initial = await driver.native(); requireStartupNative(initial); native.push(initial)
  if (initial.idleMilliseconds < 2000) throw new Error("Static startup requires genuine two-second idle admission")
  try {
    for (const mode of ["cold", "warm-upgrade"] as const) {
      const run: any = { mode, startedAt: Date.now(), states: [], movie: [] }; runs.push(run)
      await driver.navigate(mode)
      const startupDeadline = Math.min(deadline, Date.now() + 50_000)
      let gesture = false, menu: StartupObservation | undefined
      while (Date.now() < startupDeadline) {
        const state = await read(); run.states.push(state)
        if (state.startupState === "AwaitingGesture" && !gesture) { await driver.action("play-intro"); gesture = true }
        // Sample the configured movie's lit segment, not its intentional black
        // tail. Completion still waits for the real ended event and Main Menu.
        if (state.startupState === "Playing" && state.movie && !state.movie.paused && !state.movie.muted
          && state.movie.width > 0 && state.movie.height > 0 && state.movie.time >= 1
          && (run.movie.length === 0 || run.movie.length === 1 && state.movie.time >= run.movie[0].time + 1)) {
          const movieNative=await driver.native();requireStartupNative(movieNative);native.push(movieNative)
          const beforePixels=await read()
          if(beforePixels.startupState!=="Playing"||!beforePixels.movie||beforePixels.movie.paused||beforePixels.movie.muted)throw new Error("Startup movie ended before its pixel observation")
          const image=startupPixelEvidence(await driver.screenshot(`${mode}-movie-${run.movie.length}`))
          const afterPixels=await read()
          if(afterPixels.startupState!=="Playing"||!afterPixels.movie||afterPixels.movie.paused||afterPixels.movie.muted||afterPixels.movie.time<beforePixels.movie.time)throw new Error("Startup movie changed during its pixel observation")
          run.movie.push({ ...beforePixels.movie, finishedTime:afterPixels.movie.time, pixels:image })
        }
        if (state.phase === "MainMenu") { menu = state; break }
        await driver.wait(100)
      }
      if (!menu || menu.startupState !== "Completed" || run.movie.length !== 2
        || run.movie[0].pixels.sha256 === run.movie[1].pixels.sha256) throw new Error("Static package did not show advancing audible movie pixels followed by its menu")
      const menuNative = await driver.native(); requireStartupNative(menuNative); native.push(menuNative)
      run.menu = { state: menu, pixels: startupPixelEvidence(await driver.screenshot(`${mode}-menu`)) }
      await driver.action("open-map", target)
      const mapDeadline = Math.min(deadline, Date.now() + 75_000)
      let team = false, playerClass = false, consoleClosed = false, firstFrame: StartupObservation | undefined, playable: StartupObservation | undefined
      while (Date.now() < mapDeadline) {
        const state = await read(); run.states.push(state)
        // Keep the console's focused input through loading. The authored team
        // selection transition closes it immediately before choosing a team.
        if(state.phase==="Ready"&&!state.teamSelection&&!state.classSelection&&state.consoleVisible&&!consoleClosed){await driver.action("close-console");consoleClosed=true;await driver.wait(100);continue}
        if (state.teamSelection && !team) { await driver.action("choose-team"); team = true }
        if (state.classSelection && !playerClass) { await driver.action("choose-class"); playerClass = true }
        if (state.phase === "Ready" && Number.isSafeInteger(state.frame) && state.frame > 0 && state.gameUi === "in-game" && !state.consoleVisible && !state.teamSelection && !state.classSelection
          && (state.playerClass??0)>0 && /^\d+$/.test(state.tick??"") && BigInt(state.tick!)>0n) {
          firstFrame ??= state
          if (state.frame >= firstFrame.frame + 2) { playable = state; break }
        }
        await driver.wait(100)
      }
      if (!playable) throw new Error("Static package did not complete actual playable game frames")
      if (playable.cache !== (mode === "cold" ? "stored" : "hit")) throw new Error("Static startup did not exercise the declared cold/warm compatible map cache")
      const playableNative = await driver.native(); requireStartupNative(playableNative); native.push(playableNative)
      run.playable = { firstFrame: firstFrame!.frame, state: playable, pixels: startupPixelEvidence(await driver.screenshot(`${mode}-playable`)) }
      run.endedAt = Date.now()
    }
    return { startedAt, endedAt: Date.now(), runs, native }
  } catch (error) {
    throw Object.assign(error instanceof Error ? error : new Error(String(error)), { startupEvidence: { startedAt, endedAt: Date.now(), runs, native, terminal } })
  }
}

export type StaticStartupReceipt = {
  schema: "playsrc-static-startup-v1"; packageSha256: string; wasmSha256: string; previousPackageSha256: string
  previousEntryUsed: boolean; upgradeNavigations: number; capture: { startedAt: number; endedAt: number; native: StartupNativeAdmission[]; runs: any[] }
  bootFailure: { phase: string; visible: boolean; text: string; pixels: ReturnType<typeof startupPixelEvidence>; native: StartupNativeAdmission }
}

export function staticStartupReceipt(identity: Omit<StaticStartupReceipt, "schema" | "capture">, capture: Awaited<ReturnType<typeof captureStaticStartup>>): StaticStartupReceipt {
  const receipt: StaticStartupReceipt = { schema: "playsrc-static-startup-v1", ...identity, capture: {
    startedAt: capture.startedAt, endedAt: capture.endedAt, native: capture.native,
    runs: capture.runs.map(run => ({ mode: run.mode, movie: run.movie, menu: run.menu, playable: run.playable,
      guard: { observations: run.states.length, valid: run.states.length > 0 && run.states.every((s: StartupObservation) => s.visible && s.focused && !s.unexpectedInput && s.phase !== "Failed" && s.startupState !== "Failed" && s.startupState !== "Skipped") } })),
  } }
  assertStaticStartupReceipt(receipt, identity)
  return receipt
}

export function assertStaticStartupReceipt(value: unknown, expected: { packageSha256: string; wasmSha256: string }): asserts value is StaticStartupReceipt {
  const receipt = value as StaticStartupReceipt
  if (!receipt || receipt.schema !== "playsrc-static-startup-v1" || receipt.packageSha256 !== expected.packageSha256
    || receipt.wasmSha256 !== expected.wasmSha256 || !/^[0-9a-f]{64}$/.test(receipt.previousPackageSha256) || receipt.previousPackageSha256===receipt.packageSha256 || receipt.previousEntryUsed !== true || receipt.upgradeNavigations!==2
    || !receipt.capture || receipt.capture.endedAt <= receipt.capture.startedAt || receipt.capture.endedAt - receipt.capture.startedAt > 150_000
    || receipt.capture.native?.length !== 9 || receipt.capture.runs?.length !== 2) throw new Error("Exact static-package startup acceptance is absent or mismatched")
  receipt.capture.native.forEach(requireStartupNative)
  if(!receipt.bootFailure||receipt.bootFailure.phase!=="Failed"||!receipt.bootFailure.visible||!receipt.bootFailure.text
    ||receipt.bootFailure.pixels?.nonblack<1024||receipt.bootFailure.pixels?.colors<16||!/^[0-9a-f]{64}$/.test(receipt.bootFailure.pixels?.sha256??""))throw new Error("Static startup receipt lacks visible independent boot failure evidence")
  requireStartupNative(receipt.bootFailure.native)
  if (receipt.capture.native[0]!.idleMilliseconds < 2000) throw new Error("Static startup receipt has no idle admission")
  if ([...receipt.capture.native,receipt.bootFailure.native].some(value => ["browserPid", "windowId", "targetId"].some(key => value[key as keyof StartupNativeAdmission] !== receipt.capture.native[0]![key as keyof StartupNativeAdmission]))) throw new Error("Static startup native window ownership changed")
  for (const [index, run] of receipt.capture.runs.entries()) {
    if (run.mode !== ["cold", "warm-upgrade"][index] || run.movie?.length !== 2 || !run.menu || !run.playable
      || run.movie[1].time < run.movie[0].time + 1 || run.movie[0].time < 1 || run.movie.some((m: any) => m.muted || m.paused)
      || run.menu.state.phase !== "MainMenu" || run.menu.state.startupState !== "Completed"
      || run.playable.state.phase !== "Ready" || run.playable.state.frame < run.playable.firstFrame + 2
      || !Number.isSafeInteger(run.playable.state.frame) || !Number.isSafeInteger(run.playable.firstFrame) || run.playable.firstFrame<1
      || run.playable.state.gameUi!=="in-game" || run.playable.state.consoleVisible || run.playable.state.teamSelection || run.playable.state.classSelection
      || !(run.playable.state.playerClass>0) || !/^\d+$/.test(run.playable.state.tick??"") || BigInt(run.playable.state.tick)<=0n
      || run.playable.state.cache !== (index === 0 ? "stored" : "hit")
      || run.guard?.valid !== true || !Number.isSafeInteger(run.guard.observations) || run.guard.observations < 1) throw new Error("Static startup receipt lacks complete movie/menu/playable-frame evidence")
    const images = [...run.movie.map((m: any) => m.pixels), run.menu.pixels, run.playable.pixels]
    if (images.some(p => !p || !/^[0-9a-f]{64}$/.test(p.sha256) || p.nonblack < 1024 || p.colors < 16 || !(p.width > 0 && p.height > 0))
      || run.movie[0].pixels.sha256 === run.movie[1].pixels.sha256) throw new Error("Static startup receipt lacks real changing pixels")
  }
}
