import path from "node:path"
import { writeFile } from "node:fs/promises"
import { loadLocalConfig } from "../src/config"
import { guardStartupInput, test as base, expect } from "./application-test"
import { closeStartupNativeProbe, startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { profileArtifact } from "./profile-artifacts"
import { macWindowClick } from "./macos-window-click"

export const test = base.extend<{
  nativeGameplay: { lockPointer(): Promise<{ x: number; y: number }>; capture(label: string): Promise<void> }
}>({
  nativeGameplay: [async ({ page }, use, testInfo) => {
    const local = await loadLocalConfig()
    const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY
    if (!directory || process.env.PLAYSRC_PROFILE_MANAGED !== "1") throw new Error("Demoman evidence requires the checked machine-wide profile owner")
    let reader: Awaited<ReturnType<typeof startupNativeReader>> | undefined
    let timer: ReturnType<typeof setTimeout> | undefined
    let pending = Promise.resolve()
    let stopped = false
    let pointerAdmission = false
    let fault: unknown
    let establishmentRecords = 0
    const nativeClickRecords: unknown[] = []
    await page.addInitScript(() => {
      document.addEventListener("click", event => {
        const canvas = document.querySelector("canvas.world-canvas")
        if (event.target !== canvas || !canvas) return
        Object.assign(globalThis, { __playsrcPointerClick: { x: event.clientX, y: event.clientY, trusted: event.isTrusted,
          connected: canvas.isConnected, currentDocument: canvas.ownerDocument === document && canvas.getRootNode() === document,
          topLevel: window.top === window } })
      }, true)
    })
    let rejectFailure: (error: unknown) => void = () => {}
    const failure = new Promise<never>((_, reject) => { rejectFailure = reject })
    const check = (label?: string) => {
      pending = pending.then(async () => {
        if (fault) throw fault
        if (!reader) throw new Error("Native gameplay reader is unavailable")
        const capture = label === undefined ? undefined : path.join(directory, `demoman-${label}-native.png`)
        requireStartupNative(await reader.read(capture, "window"))
        if (!await page.evaluate(() => document.hasFocus() && document.visibilityState === "visible")) {
          throw new Error("Demoman evidence lost the foreground document")
        }
      })
      return pending
    }
    try {
      if (await startupConsoleIdle(local.sourceCacheDir) < 2_000) throw new Error("Demoman evidence requires two seconds of genuine physical-console idle")
      reader = await startupNativeReader(page, local.sourceCacheDir)
      const windowDeadline = Date.now() + 2_000
      for (;;) {
        try { requireStartupNative(await reader.read()); requireStartupNative(await reader.read()); break } catch (error) {
          const record = reader.records.at(-1)
          if (process.platform !== "darwin"
            || record?.error !== "Error: No on-screen native window matches the measured page's exact PID/bounds"
            || !record.snapshot?.console?.onConsole || record.snapshot.console.locked
            || !record.document?.focused || Date.now() >= windowDeadline) throw error
          await new Promise(resolve => setTimeout(resolve, 100))
        }
      }
      establishmentRecords = reader.records.length
      guardStartupInput(page, () => check())
      const monitor = () => {
        timer = setTimeout(() => {
          if (pointerAdmission) { monitor(); return }
          void check().then(() => { if (!stopped) monitor() }, error => {
            fault = error
            rejectFailure(error)
          })
        }, 500)
      }
      monitor()
      const lockPointer = async () => {
        pointerAdmission = true
        try {
          await check()
          if (process.platform === "darwin") {
            // DOM focus can be emulated while the omnibox is the native first responder.
            // Let OS hit testing focus the already-foreground canvas through a real click.
            const onCanvas = await page.evaluate(() => document.elementFromPoint(innerWidth / 2, outerHeight / 2 - (outerHeight - innerHeight)) === document.querySelector("canvas.world-canvas"))
            if (!onCanvas) throw new Error("Native window center is outside the game canvas")
            const record = reader!.records.at(-1) as any
            nativeClickRecords.push(await macWindowClick(local.sourceCacheDir, record.page.browserPid, record.window.id, record.window.bounds))
          } else await page.locator("canvas.world-canvas").click()
          await expect.poll(() => page.evaluate(() => document.pointerLockElement?.matches("canvas.world-canvas") ?? false), { timeout: 5_000 }).toBe(true)
          const point = await page.evaluate(() => (globalThis as any).__playsrcPointerClick as { x: number; y: number; trusted: boolean; connected: boolean; currentDocument: boolean; topLevel: boolean })
          if (!point?.trusted || !point.connected || !point.currentDocument || !point.topLevel || !Number.isFinite(point.x) || !Number.isFinite(point.y)) throw new Error("Native pointer click coordinates are unavailable")
          await page.mouse.move(point.x, point.y)
          // A native browser notice can obscure pixels immediately after lock.
          // Retain rejected readbacks and await its own dismissal before firing.
          const deadline = Date.now() + 5_000
          for (;;) {
            try { requireStartupNative(await reader!.read()); break } catch (error) {
              const record = reader!.records.at(-1)
              if (process.platform === "darwin" && record?.error === "Error: No on-screen native window matches the measured page's exact PID/bounds"
                  && Date.now() < deadline) { await new Promise(resolve => setTimeout(resolve, 100)); continue }
              if (process.platform !== "darwin" || record?.error || !record?.occluders?.length
                || record.occluders.some((window: { pid: number }) => window.pid !== record.page?.browserPid)
                || Date.now() >= deadline) throw error
              await new Promise(resolve => setTimeout(resolve, 100))
            }
          }
          await check()
          return point
        } finally { pointerAdmission = false }
      }
      await Promise.race([use({ lockPointer, capture: label => check(label) }), failure])
      await check()
    } catch (error) {
      fault = error
      throw error
    } finally {
      stopped = true
      if (timer) clearTimeout(timer)
      try { await pending } finally {
        try {
          const application = await page.evaluate(() => {
            const data = document.querySelector<HTMLElement>("main")?.dataset
            return { phase: data?.phase, detail: data?.detail, hud: data?.hudProbe, tick: data?.snapshotTick, gameUi: data?.gameui,
              console: document.querySelector("[aria-label='Console output']")?.textContent?.slice(-8_192),
              pointerClick: (globalThis as any).__playsrcPointerClick }
          }).catch(error => ({ error: String(error) }))
          const observation = { status: testInfo.status, fault: fault ? String(fault) : null, application, nativeClickRecords,
            frames: page.frames().map(frame => ({ url: frame.url(), topLevel: frame === page.mainFrame(), detached: frame.isDetached() })),
            establishmentRecords, records: [...(reader?.records ?? [])] }
          await profileArtifact(async () => {
            const record = JSON.stringify(observation)
            await writeFile(path.join(directory, "demoman-native-admission.json"), record)
            await testInfo.attach("demoman-native-admission", { body: record, contentType: "application/json" })
          })
        } finally {
          await reader?.close()
          closeStartupNativeProbe()
        }
      }
    }
  }, { auto: true }],
})

export { expect }
