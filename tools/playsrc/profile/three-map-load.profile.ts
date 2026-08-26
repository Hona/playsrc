import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"

const TARGETS = ["jump_beef", "pl_upward", "ctf_2fort"] as const
const SAMPLE_MILLISECONDS = 2_000
const INITIALIZATION_TARGET_MILLISECONDS = 10_000

type Descriptor = { sha256: string; byteLength: string }
type BrowserProfile = {
  requests: Array<{ url: string; started: number; resolved?: number; bytes?: number }>
  workers: Array<{
    kind: string
    started: number
    finished?: number
    postMilliseconds?: number
    requestBytes: number
    transferredBytes: number
    responseBytes?: number
    payloadSha256?: string
    presentationSha256?: string
    timings?: Record<string, number>
  }>
  hashes: Array<{ started: number; finished?: number; bytes: number }>
  indexedDb: Array<{ operation: string; started: number; finished?: number; bytes: number }>
  gpu: { uploadBytes: number; uploadCalls: number; uploadMilliseconds: number; pipelines: number; pipelineMilliseconds: number }
  phases: Array<{ at: number; phase: string; detail: string; team: boolean; frame: number }>
  frames: number[]
  longTasks: Array<{ at: number; milliseconds: number }>
}

function intervalUnion(intervals: readonly (readonly [number, number])[]): number {
  let total = 0
  let start = Number.NaN
  let end = Number.NaN
  for (const [nextStart, nextEnd] of [...intervals].sort((left, right) => left[0] - right[0])) {
    if (!(nextEnd > nextStart)) continue
    if (!Number.isFinite(start)) { start = nextStart; end = nextEnd; continue }
    if (nextStart <= end) end = Math.max(end, nextEnd)
    else { total += end - start; start = nextStart; end = nextEnd }
  }
  return Number((total + (Number.isFinite(start) ? end - start : 0)).toFixed(3))
}

function percentile(values: readonly number[], fraction: number): number | null {
  if (values.length === 0) return null
  const sorted = [...values].sort((left, right) => left - right)
  return Number(sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))]!.toFixed(3))
}

test("profiles exact headed cold initialization for all three configured TF2 maps", async ({ page, browser }, testInfo) => {
  const browserCdp = await browser.newBrowserCDPSession()
  const pageCdp = await page.context().newCDPSession(page)
  await pageCdp.send("Performance.enable")
  await page.addInitScript(() => {
    performance.setResourceTimingBufferSize(4_096)
    const state: BrowserProfile = {
      requests: [], workers: [], hashes: [], indexedDb: [], phases: [], frames: [], longTasks: [],
      gpu: { uploadBytes: 0, uploadCalls: 0, uploadMilliseconds: 0, pipelines: 0, pipelineMilliseconds: 0 },
    }
    ;(globalThis as typeof globalThis & { __playsrcThreeMapProfile: BrowserProfile }).__playsrcThreeMapProfile = state

    const originalFetch = globalThis.fetch
    globalThis.fetch = async (...args) => {
      const url = args[0] instanceof Request ? args[0].url : String(args[0])
      const record = { url, started: performance.now() } as BrowserProfile["requests"][number]
      state.requests.push(record)
      const response = await originalFetch(...args)
      record.resolved = performance.now()
      record.bytes = Number(response.headers.get("content-length") ?? 0)
      return response
    }

    const originalDigest = SubtleCrypto.prototype.digest
    SubtleCrypto.prototype.digest = async function (algorithm, data) {
      const bytes = data instanceof ArrayBuffer ? data.byteLength : data.byteLength
      const record = { started: performance.now(), bytes } as BrowserProfile["hashes"][number]
      state.hashes.push(record)
      try { return await originalDigest.call(this, algorithm, data) }
      finally { record.finished = performance.now() }
    }

    for (const operation of ["get", "getAll", "put"] as const) {
      const original = IDBObjectStore.prototype[operation]
      Object.defineProperty(IDBObjectStore.prototype, operation, {
        configurable: true,
        value(this: IDBObjectStore, ...args: unknown[]) {
          const value = args[0] as { bytes?: Blob } | undefined
          const record = { operation, started: performance.now(), bytes: value?.bytes instanceof Blob ? value.bytes.size : 0 } as BrowserProfile["indexedDb"][number]
          state.indexedDb.push(record)
          const request = Reflect.apply(original, this, args) as IDBRequest
          const finish = () => { record.finished = performance.now() }
          request.addEventListener("success", finish, { once: true })
          request.addEventListener("error", finish, { once: true })
          return request
        },
      })
    }

    const byteLength = (value: unknown): number => value instanceof ArrayBuffer ? value.byteLength : ArrayBuffer.isView(value) ? value.byteLength : 0
    const NativeWorker = globalThis.Worker
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, BrowserProfile["workers"][number]>()
      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event: MessageEvent) => {
          const record = this.records.get(event.data?.id)
          if (!record) return
          record.finished = performance.now()
          record.responseBytes = byteLength(event.data?.payload) + byteLength(event.data?.presentation) + byteLength(event.data?.bytes)
          if (event.data?.timings) record.timings = event.data.timings
          if (typeof event.data?.payloadSha256 === "string") record.payloadSha256 = event.data.payloadSha256
          if (event.data?.presentation instanceof ArrayBuffer) {
            void originalDigest.call(crypto.subtle, "SHA-256", event.data.presentation).then((digest) => {
              record.presentationSha256 = Array.from(new Uint8Array(digest), value => value.toString(16).padStart(2, "0")).join("")
            })
          }
        })
      }
      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (!Number.isSafeInteger(message?.id) || typeof message.kind !== "string") {
          super.postMessage(message, transferOrOptions as any)
          return
        }
        const transfer = Array.isArray(transferOrOptions) ? transferOrOptions : transferOrOptions?.transfer ?? []
        const record = {
          kind: message.kind,
          started: performance.now(),
          requestBytes: byteLength(message.wasm) + byteLength(message.bsp)
            + (Array.isArray(message.configuration) ? message.configuration.reduce((total: number, section: unknown) => total + byteLength(section), 0) : byteLength(message.configuration))
            + byteLength(message.presentation) + byteLength(message.batch) + byteLength(message.command)
            + (Array.isArray(message.chunks) ? message.chunks.reduce((total: number, chunk: { descriptor?: unknown; bytes?: unknown }) => total + byteLength(chunk.descriptor) + byteLength(chunk.bytes), 0) : 0),
          transferredBytes: transfer.reduce((total, value) => total + byteLength(value), 0),
        } as BrowserProfile["workers"][number]
        this.records.set(message.id, record)
        state.workers.push(record)
        const started = performance.now()
        super.postMessage(message, transferOrOptions as any)
        record.postMilliseconds = performance.now() - started
      }
    }
    Object.defineProperty(globalThis, "Worker", { configurable: true, value: ProfiledWorker })

    const queuePrototype = (globalThis as typeof globalThis & { GPUQueue?: { prototype: GPUQueue } }).GPUQueue?.prototype
    if (queuePrototype) for (const operation of ["writeBuffer", "writeTexture"] as const) {
      const original = queuePrototype[operation]
      Object.defineProperty(queuePrototype, operation, {
        configurable: true,
        value(this: GPUQueue, ...args: unknown[]) {
          const started = performance.now()
          try { return Reflect.apply(original, this, args) }
          finally {
            state.gpu.uploadCalls += 1
            state.gpu.uploadBytes += byteLength(args[operation === "writeBuffer" ? 2 : 1])
            state.gpu.uploadMilliseconds += performance.now() - started
          }
        },
      })
    }
    const devicePrototype = (globalThis as typeof globalThis & { GPUDevice?: { prototype: GPUDevice } }).GPUDevice?.prototype
    if (devicePrototype) for (const operation of ["createRenderPipeline", "createRenderPipelineAsync"] as const) {
      const original = devicePrototype[operation]
      Object.defineProperty(devicePrototype, operation, {
        configurable: true,
        value(this: GPUDevice, ...args: unknown[]) {
          const started = performance.now()
          state.gpu.pipelines += 1
          const result = Reflect.apply(original, this, args)
          if (result instanceof Promise) return result.finally(() => { state.gpu.pipelineMilliseconds += performance.now() - started })
          state.gpu.pipelineMilliseconds += performance.now() - started
          return result
        },
      })
    }

    try {
      new PerformanceObserver(entries => {
        for (const entry of entries.getEntries()) state.longTasks.push({ at: entry.startTime, milliseconds: entry.duration })
      }).observe({ entryTypes: ["longtask"] })
    } catch {}
    const frame = (now: number) => { state.frames.push(now); requestAnimationFrame(frame) }
    requestAnimationFrame(frame)
    addEventListener("DOMContentLoaded", () => {
      const main = document.querySelector<HTMLElement>("main")
      if (!main) return
      let prior = ""
      const capture = () => {
        const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")
        const next = `${main.dataset.phase}\0${main.dataset.detail}\0${main.dataset.teamSelectionVisible}\0${canvas?.dataset.displayFrame}`
        if (next === prior) return
        prior = next
        state.phases.push({ at: performance.now(), phase: main.dataset.phase ?? "", detail: main.dataset.detail ?? "", team: main.dataset.teamSelectionVisible === "true", frame: Number(canvas?.dataset.displayFrame ?? 0) })
      }
      capture()
      new MutationObserver(capture).observe(main, { attributes: true, attributeFilter: ["data-phase", "data-detail", "data-team-selection-visible", "data-display-frame"] })
    })
  })

  const enterMainMenu = async () => {
    await page.goto("/", { waitUntil: "load", timeout: 30_000 })
    await page.waitForFunction(() => {
      const main = document.querySelector<HTMLElement>("main")
      return main?.dataset.phase === "MainMenu" || main?.dataset.phase === "Failed"
    }, undefined, { timeout: 300_000, polling: 25 })
    expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")
  }
  await enterMainMenu()
  const response = await page.request.get("/playsrc-config.json")
  expect(response.status()).toBe(200)
  const configuration = await response.json() as { assetOrigin: string; wasm: Descriptor; targets: Array<{ target: string; objects: { bsp: Descriptor; resources: Descriptor } }> }
  expect(configuration.targets.map(target => target.target)).toEqual(TARGETS)

  const snapshot = async () => {
    const [browserProcesses, metrics, heap, value] = await Promise.all([
      browserCdp.send("SystemInfo.getProcessInfo") as Promise<{ processInfo: Array<{ type: string; id: number; cpuTime: number }> }>,
      pageCdp.send("Performance.getMetrics") as Promise<{ metrics: Array<{ name: string; value: number }> }>,
      pageCdp.send("Runtime.getHeapUsage") as Promise<{ usedSize: number; totalSize: number; embedderHeapUsedSize?: number; backingStorageSize?: number }>,
      page.evaluate(async () => {
        const profile = (globalThis as typeof globalThis & { __playsrcThreeMapProfile: BrowserProfile }).__playsrcThreeMapProfile
        return {
          now: performance.now(),
          storage: await navigator.storage.estimate(),
          databases: typeof indexedDB.databases === "function" ? await indexedDB.databases() : [],
          indices: { requests: profile.requests.length, workers: profile.workers.length, hashes: profile.hashes.length, indexedDb: profile.indexedDb.length, phases: profile.phases.length, frames: profile.frames.length, longTasks: profile.longTasks.length },
          gpu: { ...profile.gpu },
          tick: Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick ?? 0),
          displayFrame: Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayFrame ?? 0),
        }
      }),
    ])
    return { browserProcesses: browserProcesses.processInfo, metrics: Object.fromEntries(metrics.metrics.map(metric => [metric.name, metric.value])), heap, ...value }
  }

  const startup = await snapshot()
  const startupProfile = await page.evaluate(() => {
    const profile = (globalThis as typeof globalThis & { __playsrcThreeMapProfile: BrowserProfile }).__playsrcThreeMapProfile
    return { workers: profile.workers.filter(worker => ["initialize", "decode-resources"].includes(worker.kind)), requests: profile.requests }
  })
  const maps: Array<Record<string, unknown>> = []
  for (const target of configuration.targets) {
    await page.keyboard.press("Backquote")
    const consoleEntry = page.locator("[aria-label='Console command']")
    await expect(consoleEntry).toBeVisible()
    await consoleEntry.fill(`map ${target.target}`)
    const before = await snapshot()
    const started = await page.evaluate(() => performance.now())
    await page.keyboard.press("Enter")
    await page.waitForFunction((identity) => {
      const main = document.querySelector<HTMLElement>("main")
      const console = document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText ?? ""
      return main?.dataset.phase === "Failed"
         || main?.dataset.teamSelectionVisible === "true"
         || main?.dataset.phase === "Ready" && main.dataset.detail === `Playing ${identity}`
         || console.includes("ERROR: Map replacement failed")
    }, target.target, { timeout: 120_000, polling: 25 })
    if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
      if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
      await page.waitForFunction(() => ["Ready", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 120_000, polling: 20 })
      if (await page.locator("main").getAttribute("data-class-selection-visible") === "true") await page.keyboard.press("Digit2")
    }
    const ready = await page.evaluate(() => performance.now())
    const main = page.locator("main")
    expect(await main.getAttribute("data-phase"), await main.getAttribute("data-detail") ?? "").toBe("Ready")
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.waitForFunction(previous => Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.dataset.displayFrame ?? 0) > previous, before.displayFrame, { timeout: 120_000, polling: 20 })
    const visible = await page.evaluate(() => performance.now())
    const load = JSON.parse((await main.getAttribute("data-load-performance")) ?? "null") as { application: Record<string, number>; client: Record<string, number>; mapBytes: number; presentationBytes: number; mapCache: string; presentationCache: string }
    expect(load).not.toBeNull()
    const sample = await page.evaluate(async milliseconds => {
      const main = document.querySelector<HTMLElement>("main")!
      const started = performance.now()
      const tick = Number(main.dataset.snapshotTick ?? 0)
      const frames: number[] = []
      let previous = performance.now()
      while (performance.now() - started < milliseconds) {
        const now = await new Promise<number>(resolve => requestAnimationFrame(resolve))
        frames.push(now - previous)
        previous = now
      }
      return { milliseconds: performance.now() - started, ticks: Number(main.dataset.snapshotTick ?? 0) - tick, frames }
    }, SAMPLE_MILLISECONDS)
    const screenshot = decodeScreenshot(await page.locator("canvas.world-canvas").screenshot())
    let visiblePixels = 0
    for (let index = 0; index < screenshot.pixels.length; index += screenshot.channels) {
      if (screenshot.pixels[index]! > 8 || screenshot.pixels[index + 1]! > 8 || screenshot.pixels[index + 2]! > 8) visiblePixels += 1
    }
    expect(visiblePixels).toBeGreaterThan(20_000)
    const after = await snapshot()
    const profile = await page.evaluate(({ indices, started, ready }) => {
      const profile = (globalThis as typeof globalThis & { __playsrcThreeMapProfile: BrowserProfile }).__playsrcThreeMapProfile
      const resources = performance.getEntriesByType("resource").filter(entry => entry.startTime >= started && entry.startTime <= ready).map(entry => {
        const resource = entry as PerformanceResourceTiming
        return { url: resource.name, started: resource.startTime, finished: resource.responseEnd, duration: resource.duration, transferBytes: resource.transferSize, encodedBytes: resource.encodedBodySize, decodedBytes: resource.decodedBodySize }
      })
      return {
        requests: profile.requests.slice(indices.requests).filter(record => record.started <= ready),
        workers: profile.workers.slice(indices.workers).filter(record => record.started <= ready),
        hashes: profile.hashes.slice(indices.hashes).filter(record => record.started <= ready),
        indexedDb: profile.indexedDb.slice(indices.indexedDb).filter(record => record.started <= ready),
        phases: profile.phases.slice(indices.phases).filter(record => record.at <= ready),
        longTasks: profile.longTasks.slice(indices.longTasks).filter(record => record.at <= ready),
        resources,
      }
    }, { indices: before.indices, started, ready })
    const worker = profile.workers.find(value => value.kind === "load")
    expect(worker?.payloadSha256).toMatch(/^[0-9a-f]{64}$/)
    expect(worker?.presentationSha256).toMatch(/^[0-9a-f]{64}$/)
    const networkMilliseconds = intervalUnion(profile.resources.map(resource => [Math.max(started, resource.started), Math.min(ready, resource.finished)] as const))
    const downloadBytes = profile.resources.reduce((sum, resource) => sum + resource.transferBytes, 0)
    const hashMilliseconds = profile.hashes.reduce((sum, hash) => sum + Math.max(0, (hash.finished ?? ready) - hash.started), 0)
    const idbMilliseconds = profile.indexedDb.reduce((sum, entry) => sum + Math.max(0, (entry.finished ?? ready) - entry.started), 0)
    const cpuBefore = before.browserProcesses.reduce((sum, process) => sum + process.cpuTime, 0)
    const cpuAfter = after.browserProcesses.reduce((sum, process) => sum + process.cpuTime, 0)
    const row = {
      target: target.target,
      identities: { bsp: target.objects.bsp, resources: target.objects.resources, wasm: configuration.wasm, payloadSha256: worker!.payloadSha256, presentationSha256: worker!.presentationSha256 },
      bytes: { bsp: Number(target.objects.bsp.byteLength), resources: Number(target.objects.resources.byteLength), payload: load.mapBytes, presentation: load.presentationBytes, downloaded: downloadBytes, workerRequest: worker!.requestBytes, workerTransferred: worker!.transferredBytes, workerResponse: worker!.responseBytes ?? 0, hashed: profile.hashes.reduce((sum, value) => sum + value.bytes, 0), gpuUploaded: after.gpu.uploadBytes - before.gpu.uploadBytes },
      cache: { derivedBeforeBytes: before.storage.usage ?? null, derivedAfterBytes: after.storage.usage ?? null, databasesBefore: before.databases, map: load.mapCache, presentation: load.presentationCache, httpTransfers: profile.resources.filter(resource => resource.transferBytes > 0).length, httpHits: profile.resources.filter(resource => resource.transferBytes === 0).length },
      milliseconds: {
        totalWall: Number((ready - started).toFixed(3)),
        networkDownload: networkMilliseconds,
        initializationExcludingDownload: Number((ready - started - networkMilliseconds).toFixed(3)),
        firstVisibleFrame: Number((visible - started).toFixed(3)),
        assetCacheResolution: Number(((load.application.fetch ?? 0) + (load.application.cacheOpen ?? 0) + (load.application.derivedKey ?? 0) + (load.client.mapCacheReadMilliseconds ?? 0) + (load.client.presentationCacheReadMilliseconds ?? 0)).toFixed(3)),
        resourceGraphAndDecode: Number(profile.workers.filter(value => value.kind === "decode-resources").reduce((sum, value) => sum + (value.finished ?? ready) - value.started, 0).toFixed(3)),
        workerScheduling: Number((worker!.timings?.queueMilliseconds ?? 0).toFixed(3)),
        workerStructuredCloneAndTransfer: Number(((worker!.postMilliseconds ?? 0) + (load.client.inputCloneMilliseconds ?? 0) + (worker!.timings?.inputCopyMilliseconds ?? 0) + (worker!.timings?.mapCopyMilliseconds ?? 0) + (worker!.timings?.presentationCopyMilliseconds ?? 0)).toFixed(3)),
        wasmInitialization: Number(profile.workers.filter(value => value.kind === "initialize").reduce((sum, value) => sum + (value.finished ?? ready) - value.started, 0).toFixed(3)),
        bspParse: Number((worker!.timings?.bspParseMilliseconds ?? 0).toFixed(3)),
        visibilityCollisionEntityPlayerBots: Number(((worker!.timings?.entityParseMilliseconds ?? 0) + (worker!.timings?.runtimeMapMilliseconds ?? 0) + (worker!.timings?.collisionSetupMilliseconds ?? 0) + (worker!.timings?.gameSetupMilliseconds ?? 0)).toFixed(3)),
        mapCompilation: Number(((worker!.timings?.canonicalMapMilliseconds ?? 0) + (worker!.timings?.materialResolutionMilliseconds ?? 0)).toFixed(3)),
        modelDecode: Number(((worker!.timings?.modelResolutionMilliseconds ?? 0) + (worker!.timings?.presentationModelsMilliseconds ?? 0) + (load.application.modelCache ?? 0)).toFixed(3)),
        vtfDecode: Number((worker!.timings?.presentationTexturesMilliseconds ?? 0).toFixed(3)),
        presentationCompilation: Number((worker!.timings?.presentationCompileMilliseconds ?? 0).toFixed(3)),
        presentationParse: Number((load.application.presentationParse ?? 0).toFixed(3)),
        hashing: Number(hashMilliseconds.toFixed(3)),
        indexedDb: Number(idbMilliseconds.toFixed(3)),
        indexedDbPersistence: Number(((load.application.persistence ?? 0) + (load.client.mapCacheWriteMilliseconds ?? 0) + (load.client.presentationCacheWriteMilliseconds ?? 0)).toFixed(3)),
        threeWebGpuStaging: Number(((load.application.rendererCreate ?? 0) + (load.application.rendererLoadMap ?? 0)).toFixed(3)),
        gpuUploads: Number((after.gpu.uploadMilliseconds - before.gpu.uploadMilliseconds).toFixed(3)),
        gpuPipelinePreparation: Number((after.gpu.pipelineMilliseconds - before.gpu.pipelineMilliseconds).toFixed(3)),
        teamClassUi: Number((load.application.initialPublication ?? 0).toFixed(3)),
        initialProbes: Number(((load.application.initialProbes ?? 0) + (load.application.initialization ?? 0)).toFixed(3)),
      },
      gpu: { uploadCalls: after.gpu.uploadCalls - before.gpu.uploadCalls, pipelines: after.gpu.pipelines - before.gpu.pipelines },
      cpu: { browserProcessSeconds: Number((cpuAfter - cpuBefore).toFixed(3)), mainThreadTaskSeconds: Number(((after.metrics.TaskDuration ?? 0) - (before.metrics.TaskDuration ?? 0)).toFixed(3)) },
      memory: { heapBeforeBytes: before.heap.usedSize, heapAfterBytes: after.heap.usedSize, backingBeforeBytes: before.heap.backingStorageSize ?? null, backingAfterBytes: after.heap.backingStorageSize ?? null, storageBeforeBytes: before.storage.usage ?? null, storageAfterBytes: after.storage.usage ?? null },
      simulation: { sampleMilliseconds: Number(sample.milliseconds.toFixed(3)), ticks: sample.ticks, hertz: Number((sample.ticks * 1_000 / sample.milliseconds).toFixed(3)), frames: sample.frames.length, frameP95Milliseconds: percentile(sample.frames, 0.95), visiblePixels },
      applicationPhases: load.application,
      clientPhases: load.client,
      workerPhases: worker!.timings,
      network: profile.resources,
      phaseTransitions: profile.phases,
      longTasks: profile.longTasks,
    }
    expect(row.simulation.ticks).toBeGreaterThan(0)
    maps.push(row)
    console.log(`PLAYSRC_THREE_MAP_ROW ${JSON.stringify({ target: row.target, wallSeconds: row.milliseconds.totalWall / 1_000, networkSeconds: row.milliseconds.networkDownload / 1_000, initializationSeconds: row.milliseconds.initializationExcludingDownload / 1_000, phases: row.milliseconds, frameP95Milliseconds: row.simulation.frameP95Milliseconds, tickHertz: row.simulation.hertz, mapCache: row.cache.map, presentationCache: row.cache.presentation })}`)
  }

  const report = {
    schema: "playsrc-three-map-load-profile-v1",
    capturedAt: new Date().toISOString(),
    startup: { milliseconds: startup.now, wasmInitializationMilliseconds: startupProfile.workers.filter(worker => worker.kind === "initialize").reduce((sum, worker) => sum + (worker.finished ?? startup.now) - worker.started, 0), workers: startupProfile.workers, requests: startupProfile.requests },
    browserSessions: 1,
    pageReloads: 0,
    initializationTargetMilliseconds: INITIALIZATION_TARGET_MILLISECONDS,
    sampleMillisecondsPerMap: SAMPLE_MILLISECONDS,
    maps,
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "three-map-load")
  await mkdir(directory, { recursive: true })
  const filename = process.env.PLAYSRC_THREE_MAP_CAPTURE === "before" ? "before.json" : "after.json"
  const reportPath = path.join(directory, filename)
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)
  await testInfo.attach("three-map-load", { body: Buffer.from(JSON.stringify(report, null, 2)), contentType: "application/json" })
  console.log(`PLAYSRC_THREE_MAP_REPORT ${JSON.stringify({ path: reportPath, maps: maps.map(map => ({ target: map.target, milliseconds: (map.milliseconds as { initializationExcludingDownload: number }).initializationExcludingDownload })) })}`)
  await pageCdp.detach()
  await browserCdp.detach()
  if (process.env.PLAYSRC_THREE_MAP_CAPTURE !== "before") {
    for (const row of maps) {
      expect(
        (row.milliseconds as { initializationExcludingDownload: number }).initializationExcludingDownload,
        `${row.target} exceeded the exact 10-second cold initialization budget`,
      ).toBeLessThan(INITIALIZATION_TARGET_MILLISECONDS)
    }
  }
})
