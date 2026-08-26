import type { Page } from "@playwright/test"
import { assertVisibleGameplayTruth, summarizeCompositorTruth, type ChromiumTraceEvent } from "./compositor-truth"
import { digest } from "./release-generations"

export async function changingGameplayEvidence(page: Page) {
  const cdp = await page.context().newCDPSession(page)
  const events: ChromiumTraceEvent[] = []
  cdp.on("Tracing.dataCollected", ({ value }) => events.push(...value))
  const canvas = page.locator("canvas.world-canvas")
  const beforePixels = await canvas.screenshot()
  await cdp.send("Performance.enable")
  await cdp.send("Tracing.start", { categories: "benchmark,viz,gpu,devtools.timeline", options: "record-as-much-as-possible" })
  const firstClock = (await cdp.send("Performance.getMetrics")).metrics.find((metric) => metric.name === "Timestamp")!.value
  await page.keyboard.down("w")
  const measured = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>("main")!
    const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const profiler = (globalThis as any).__playsrcFrameProfiler
    const start = { tick: Number(root.dataset.snapshotTick), frame: Number(canvas.dataset.displayFrame), submissions: profiler.counters.submissions }
    const started = performance.now()
    profiler.active = true
    await new Promise((resolve) => setTimeout(resolve, 5_000))
    profiler.active = false
    return {
      elapsedMilliseconds: performance.now() - started,
      visible: document.visibilityState === "visible", focused: document.hasFocus(),
      ticks: Number(root.dataset.snapshotTick) - start.tick,
      displayFrames: Number(canvas.dataset.displayFrame) - start.frame,
      submissions: profiler.counters.submissions - start.submissions,
      losses: profiler.losses, validationErrors: profiler.counters.validationErrors,
      phase: root.dataset.phase, bots: Number(root.dataset.botCount),
    }
  })
  await page.keyboard.up("w")
  const lastClock = (await cdp.send("Performance.getMetrics")).metrics.find((metric) => metric.name === "Timestamp")!.value
  const finished = new Promise<void>((resolve) => cdp.once("Tracing.tracingComplete", () => resolve()))
  await cdp.send("Tracing.end")
  await finished
  await cdp.detach()
  const afterPixels = await canvas.screenshot()
  const pixels = { beforeSha256: digest(beforePixels), afterSha256: digest(afterPixels) }
  assertVisibleGameplayTruth({ ...measured, ...pixels })
  const compositor = summarizeCompositorTruth(events, (lastClock - firstClock) * 1_000, { startedMicroseconds: firstClock * 1e6, endedMicroseconds: lastClock * 1e6 })
  if (compositor.evidence !== "chromium-compositor-presentation-trace") throw new Error("Actual compositor presentation evidence is unavailable")
  return { ...measured, ...pixels, compositor, afterPixels }
}

export async function immutableInventory(page: Page) {
  return page.evaluate(async () => {
    const request = indexedDB.open("playsrc-derived-v3")
    const db = await new Promise<IDBDatabase>((resolve, reject) => { request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error) })
    try {
      const read = db.transaction("objects", "readonly").objectStore("objects").getAll()
      const records = await new Promise<any[]>((resolve, reject) => { read.onsuccess = () => resolve(read.result); read.onerror = () => reject(read.error) })
      const result = []
      for (const record of records.filter((record) => record.key === record.sha256)) {
        const actual = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", await record.bytes.arrayBuffer())), (byte) => byte.toString(16).padStart(2, "0")).join("")
        if (actual !== record.key || record.bytes.size !== record.byteLength) throw new Error("Warm immutable CAS corruption")
        result.push({ key: record.key, sha256: actual, byteLength: record.byteLength })
      }
      return result.sort((a, b) => a.key.localeCompare(b.key))
    } finally { db.close() }
  })
}
