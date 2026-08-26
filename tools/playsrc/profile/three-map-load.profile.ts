import { execFile } from "node:child_process"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { promisify } from "node:util"
import { expect, test } from "@playwright/test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"

const TARGETS = ["jump_beef", "pl_upward", "ctf_2fort"] as const
const SAMPLE_MILLISECONDS = 2_000
const executeFile = promisify(execFile)

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
    wasmLinearBytes?: number
    timings?: Record<string, number>
  }>
  hashes: Array<{ started: number; finished?: number; bytes: number }>
  indexedDb: Array<{ operation: string; started: number; finished?: number; bytes: number }>
  gpu: {
    uploadBytes: number
    uploadCalls: number
    uploadMilliseconds: number
    pipelines: number
    pipelineMilliseconds: number
    residentTextureBytes: number
    peakTextureBytes: number
    residentBufferBytes: number
    peakBufferBytes: number
    stagingBytes: number
    peakStagingBytes: number
    compressedTextureBytes: number
    compressedTextures: number
    textures: number
    destroyedTextures: number
    formats: Record<string, number>
  }
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

type BrowserProcess = Readonly<{ type: string; id: number; cpuTime: number }>
type ResidentProcess = Readonly<{ id: number; type: string; residentBytes: number; privateBytes: number | null }>

async function residentProcesses(processes: readonly BrowserProcess[]): Promise<readonly ResidentProcess[]> {
  const identities = [...new Set(processes.map((entry) => entry.id).filter((identity) => Number.isSafeInteger(identity) && identity > 0))]
  if (identities.length === 0) return []
  const types = new Map(processes.map((entry) => [entry.id, entry.type]))
  if (process.platform === "win32") {
    const { stdout } = await executeFile("powershell", ["-NoProfile", "-Command", `@(Get-Process -Id ${identities.join(",")} -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64,PrivateMemorySize64) | ConvertTo-Json -Compress`])
    if (!stdout.trim()) return []
    const values = [JSON.parse(stdout)].flat() as Array<{ Id: number; WorkingSet64: number; PrivateMemorySize64: number }>
    return values.map((value) => Object.freeze({ id: value.Id, type: types.get(value.Id) ?? "unknown", residentBytes: value.WorkingSet64, privateBytes: value.PrivateMemorySize64 }))
  }
  const { stdout } = await executeFile("ps", ["-o", "pid=,rss=", "-p", identities.join(",")])
  return stdout.trim().split("\n").flatMap((line) => {
    const [identity, resident] = line.trim().split(/\s+/u).map(Number)
    return Number.isSafeInteger(identity) && Number.isSafeInteger(resident)
      ? [Object.freeze({ id: identity!, type: types.get(identity!) ?? "unknown", residentBytes: resident! * 1024, privateBytes: null })]
      : []
  })
}

test("profiles exact headed cold initialization for all three configured TF2 maps", async ({ page, browser }, testInfo) => {
  const browserCdp = await browser.newBrowserCDPSession()
  const pageCdp = await page.context().newCDPSession(page)
  await pageCdp.send("Performance.enable")
  await page.route(/gameplay-worker\.ts(?:\?|$)/u, async (route) => {
    const response = await route.fetch()
    const source = await response.text()
    await route.fulfill({ response, body: `
      const __playsrcInstantiate = WebAssembly.instantiate;
      WebAssembly.instantiate = async function (...args) {
        const result = await __playsrcInstantiate.apply(this, args);
        const instance = result instanceof WebAssembly.Instance ? result : result.instance;
        if (instance?.exports?.memory instanceof WebAssembly.Memory) globalThis.__playsrcProfileWasmMemory = instance.exports.memory;
        return result;
      };
      const __playsrcPostMessage = globalThis.postMessage.bind(globalThis);
      globalThis.postMessage = function (message, transfer) {
        if (message && typeof message === "object") message.__playsrcWasmLinearBytes = globalThis.__playsrcProfileWasmMemory?.buffer.byteLength ?? null;
        return __playsrcPostMessage(message, transfer);
      };
      ${source}` })
  })
  await page.addInitScript(() => {
    const state: BrowserProfile = {
      requests: [], workers: [], hashes: [], indexedDb: [], phases: [], frames: [], longTasks: [],
      gpu: {
        uploadBytes: 0, uploadCalls: 0, uploadMilliseconds: 0, pipelines: 0, pipelineMilliseconds: 0,
        residentTextureBytes: 0, peakTextureBytes: 0, residentBufferBytes: 0, peakBufferBytes: 0,
        stagingBytes: 0, peakStagingBytes: 0, compressedTextureBytes: 0, compressedTextures: 0,
        textures: 0, destroyedTextures: 0, formats: {},
      },
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
          if (Number.isSafeInteger(event.data?.__playsrcWasmLinearBytes)) record.wasmLinearBytes = event.data.__playsrcWasmLinearBytes
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
          requestBytes: byteLength(message.wasm) + byteLength(message.bsp) + byteLength(message.configuration) + byteLength(message.presentation) + byteLength(message.batch) + byteLength(message.command)
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
    if (devicePrototype) {
      const textureBytes = (descriptor: GPUTextureDescriptor): number => {
        const size = typeof descriptor.size === "number" ? [descriptor.size, 1, 1]
          : Array.isArray(descriptor.size) ? descriptor.size
            : [descriptor.size.width, descriptor.size.height ?? 1, descriptor.size.depthOrArrayLayers ?? 1]
        const format = String(descriptor.format)
        const compressed = format.startsWith("bc")
        const block = compressed ? (format.startsWith("bc1") || format.startsWith("bc4") ? 8 : 16) : 0
        const scalar = format.includes("rgba32") ? 16 : format.includes("rgba16") || format.includes("rg32") ? 8
          : format.includes("r8") && !format.includes("rg8") ? 1 : format.includes("rg8") || format.includes("r16") ? 2 : 4
        let bytes = 0
        for (let mip = 0; mip < (descriptor.mipLevelCount ?? 1); mip += 1) {
          const width = Math.max(1, Math.floor(Number(size[0] ?? 1) / 2 ** mip))
          const height = Math.max(1, Math.floor(Number(size[1] ?? 1) / 2 ** mip))
          bytes += (compressed ? Math.ceil(width / 4) * Math.ceil(height / 4) * block : width * height * scalar)
            * Number(size[2] ?? 1) * (descriptor.sampleCount ?? 1)
        }
        return bytes
      }
      const originalTexture = devicePrototype.createTexture
      Object.defineProperty(devicePrototype, "createTexture", {
        configurable: true,
        value(this: GPUDevice, descriptor: GPUTextureDescriptor) {
          const texture = originalTexture.call(this, descriptor)
          const bytes = textureBytes(descriptor)
          state.gpu.textures += 1
          state.gpu.residentTextureBytes += bytes
          state.gpu.peakTextureBytes = Math.max(state.gpu.peakTextureBytes, state.gpu.residentTextureBytes)
          state.gpu.formats[descriptor.format] = (state.gpu.formats[descriptor.format] ?? 0) + 1
          if (descriptor.format.startsWith("bc")) {
            state.gpu.compressedTextures += 1
            state.gpu.compressedTextureBytes += bytes
          }
          const destroy = texture.destroy
          let destroyed = false
          texture.destroy = function () {
            if (!destroyed) {
              destroyed = true
              state.gpu.destroyedTextures += 1
              state.gpu.residentTextureBytes -= bytes
            }
            return destroy.call(this)
          }
          return texture
        },
      })
      const originalBuffer = devicePrototype.createBuffer
      Object.defineProperty(devicePrototype, "createBuffer", {
        configurable: true,
        value(this: GPUDevice, descriptor: GPUBufferDescriptor) {
          const buffer = originalBuffer.call(this, descriptor)
          const bytes = Number(descriptor.size)
          const staging = (Number(descriptor.usage) & 1) !== 0 || descriptor.mappedAtCreation === true
          state.gpu.residentBufferBytes += bytes
          state.gpu.peakBufferBytes = Math.max(state.gpu.peakBufferBytes, state.gpu.residentBufferBytes)
          if (staging) {
            state.gpu.stagingBytes += bytes
            state.gpu.peakStagingBytes = Math.max(state.gpu.peakStagingBytes, state.gpu.stagingBytes)
          }
          const destroy = buffer.destroy
          let destroyed = false
          buffer.destroy = function () {
            if (!destroyed) {
              destroyed = true
              state.gpu.residentBufferBytes -= bytes
              if (staging) state.gpu.stagingBytes -= bytes
            }
            return destroy.call(this)
          }
          return buffer
        },
      })
    }
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

  const enterMainMenu = async (reload = false) => {
    if (reload) await page.reload({ waitUntil: "load", timeout: 30_000 })
    else await page.goto("/", { waitUntil: "load", timeout: 30_000 })
    await page.waitForFunction(() => {
      const main = document.querySelector<HTMLElement>("main")
      return main?.dataset.phase === "MainMenu" || main?.dataset.phase === "Failed"
        || (main?.dataset.phase === "Startup" && ["Playing", "AwaitingGesture"].includes(main.dataset.startupState ?? ""))
    }, undefined, { timeout: 300_000, polling: 25 })
    if (await page.locator("main").getAttribute("data-phase") === "Startup") await page.keyboard.press("Escape")
    await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 300_000, polling: 25 })
    expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")
  }
  await enterMainMenu()
  const response = await page.request.get("/playsrc-config.json")
  expect(response.status()).toBe(200)
  const configuration = await response.json() as { assetOrigin: string; wasm: Descriptor; targets: Array<{ target: string; objects: { bsp: Descriptor; resources: Descriptor } }> }
  expect(configuration.targets.map(target => target.target)).toEqual(TARGETS)
  const selectedTarget = process.env.PLAYSRC_THREE_MAP_TARGET
  if (selectedTarget !== undefined && !TARGETS.includes(selectedTarget as typeof TARGETS[number])) {
    throw new Error("PLAYSRC_THREE_MAP_TARGET must name one configured TF2 map")
  }
  const targets = selectedTarget === undefined
    ? configuration.targets
    : configuration.targets.filter((target) => target.target === selectedTarget)

  const snapshot = async () => {
    const [browserProcesses, metrics, heap, value] = await Promise.all([
      browserCdp.send("SystemInfo.getProcessInfo") as Promise<{ processInfo: BrowserProcess[] }>,
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
    const resident = await residentProcesses(browserProcesses.processInfo)
    return {
      browserProcesses: browserProcesses.processInfo,
      residentProcesses: resident,
      residentBytes: resident.reduce((total, entry) => total + entry.residentBytes, 0),
      metrics: Object.fromEntries(metrics.metrics.map(metric => [metric.name, metric.value])),
      heap,
      ...value,
    }
  }

  const startup = await snapshot()
  const startupProfile = await page.evaluate(() => {
    const profile = (globalThis as typeof globalThis & { __playsrcThreeMapProfile: BrowserProfile }).__playsrcThreeMapProfile
    return { workers: profile.workers.filter(worker => ["initialize", "decode-resources"].includes(worker.kind)), requests: profile.requests }
  })
  const maps: Array<Record<string, unknown>> = []
  for (const [index, target] of targets.entries()) {
    if (index > 0) await enterMainMenu(true)
    await page.keyboard.press("Backquote")
    const consoleEntry = page.locator("[aria-label='Console command']")
    await expect(consoleEntry).toBeVisible()
    await consoleEntry.fill(`map ${target.target}`)
    const before = await snapshot()
    const processSamples: Array<{ residentBytes: number; processes: readonly ResidentProcess[] }> = [
      { residentBytes: before.residentBytes, processes: before.residentProcesses },
    ]
    let sampling = false
    const sampleProcesses = async () => {
      if (sampling) return
      sampling = true
      try {
        const processes = await browserCdp.send("SystemInfo.getProcessInfo") as { processInfo: BrowserProcess[] }
        const resident = await residentProcesses(processes.processInfo)
        processSamples.push({ residentBytes: resident.reduce((total, entry) => total + entry.residentBytes, 0), processes: resident })
      } catch { /* a closing browser invalidates an in-flight bounded process sample */ }
      finally { sampling = false }
    }
    const processSampler = setInterval(() => { void sampleProcesses() }, 250)
    const started = await page.evaluate(() => performance.now())
    await page.keyboard.press("Enter")
    await page.waitForFunction(() => {
      const main = document.querySelector<HTMLElement>("main")
      const console = document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText ?? ""
      return main?.dataset.phase === "Failed"
        || main?.dataset.teamSelectionVisible === "true"
        || main?.dataset.phase === "Ready"
        || console.includes("ERROR: Map replacement failed")
    }, undefined, { timeout: 180_000, polling: 25 })
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
    clearInterval(processSampler)
    processSamples.push({ residentBytes: after.residentBytes, processes: after.residentProcesses })
    const peakResident = processSamples.reduce((maximum, current) => current.residentBytes > maximum.residentBytes ? current : maximum)
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
      gpu: {
        uploadCalls: after.gpu.uploadCalls - before.gpu.uploadCalls,
        pipelines: after.gpu.pipelines - before.gpu.pipelines,
        residentTextureBytes: after.gpu.residentTextureBytes,
        peakTextureBytes: after.gpu.peakTextureBytes,
        residentBufferBytes: after.gpu.residentBufferBytes,
        peakBufferBytes: after.gpu.peakBufferBytes,
        stagingBytes: after.gpu.stagingBytes,
        peakStagingBytes: after.gpu.peakStagingBytes,
        compressedTextureBytes: after.gpu.compressedTextureBytes,
        compressedTextures: after.gpu.compressedTextures,
        textures: after.gpu.textures,
        destroyedTextures: after.gpu.destroyedTextures,
        formats: after.gpu.formats,
      },
      cpu: { browserProcessSeconds: Number((cpuAfter - cpuBefore).toFixed(3)), mainThreadTaskSeconds: Number(((after.metrics.TaskDuration ?? 0) - (before.metrics.TaskDuration ?? 0)).toFixed(3)) },
      memory: {
        heapBeforeBytes: before.heap.usedSize,
        heapAfterBytes: after.heap.usedSize,
        backingBeforeBytes: before.heap.backingStorageSize ?? null,
        backingAfterBytes: after.heap.backingStorageSize ?? null,
        browserResidentBeforeBytes: before.residentBytes,
        browserResidentAfterBytes: after.residentBytes,
        peakBrowserResidentBytes: peakResident.residentBytes,
        processesAtPeak: peakResident.processes,
        processSamples: processSamples.length,
        wasmLinearBytes: Math.max(0, ...profile.workers.map(record => record.wasmLinearBytes ?? 0)),
        storageBeforeBytes: before.storage.usage ?? null,
        storageAfterBytes: after.storage.usage ?? null,
      },
      simulation: { sampleMilliseconds: Number(sample.milliseconds.toFixed(3)), ticks: sample.ticks, hertz: Number((sample.ticks * 1_000 / sample.milliseconds).toFixed(3)), frames: sample.frames.length, frameP95Milliseconds: percentile(sample.frames, 0.95), visiblePixels },
      applicationPhases: load.application,
      clientPhases: load.client,
      workerPhases: worker!.timings,
      network: profile.resources,
      phaseTransitions: profile.phases,
      longTasks: profile.longTasks,
    }
    expect(row.simulation.ticks).toBeGreaterThan(0)
    if (process.env.PLAYSRC_THREE_MAP_CAPTURE !== "before") {
      expect(row.milliseconds.initializationExcludingDownload, `${target.target} exceeded the exact 30-second cold initialization budget`).toBeLessThanOrEqual(30_000)
    }
    maps.push(row)
    console.log(`PLAYSRC_THREE_MAP_ROW ${JSON.stringify({ target: row.target, wallSeconds: row.milliseconds.totalWall / 1_000, networkSeconds: row.milliseconds.networkDownload / 1_000, initializationSeconds: row.milliseconds.initializationExcludingDownload / 1_000, frameP95Milliseconds: row.simulation.frameP95Milliseconds, tickHertz: row.simulation.hertz, peakBrowserResidentBytes: row.memory.peakBrowserResidentBytes, browserResidentBytes: row.memory.browserResidentAfterBytes, wasmLinearBytes: row.memory.wasmLinearBytes, residentTextureBytes: row.gpu.residentTextureBytes, peakTextureBytes: row.gpu.peakTextureBytes, mapCache: row.cache.map, presentationCache: row.cache.presentation })}`)
  }

  const report = {
    schema: "playsrc-three-map-load-profile-v1",
    capturedAt: new Date().toISOString(),
    startup: { milliseconds: startup.now, wasmInitializationMilliseconds: startupProfile.workers.filter(worker => worker.kind === "initialize").reduce((sum, worker) => sum + (worker.finished ?? startup.now) - worker.started, 0), workers: startupProfile.workers, requests: startupProfile.requests },
    initializationTargetMilliseconds: 30_000,
    sampleMillisecondsPerMap: SAMPLE_MILLISECONDS,
    maps,
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "three-map-load")
  await mkdir(directory, { recursive: true })
  const label = process.env.PLAYSRC_THREE_MAP_LABEL
  if (label !== undefined && !/^[a-z0-9][a-z0-9-]*$/u.test(label)) {
    throw new Error("PLAYSRC_THREE_MAP_LABEL must be a lowercase bounded filename label")
  }
  const filename = label === undefined
    ? process.env.PLAYSRC_THREE_MAP_CAPTURE === "before" ? "before.json" : "after.json"
    : `${label}.json`
  const reportPath = path.join(directory, filename)
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)
  await testInfo.attach("three-map-load", { body: Buffer.from(JSON.stringify(report, null, 2)), contentType: "application/json" })
  console.log(`PLAYSRC_THREE_MAP_REPORT ${JSON.stringify({ path: reportPath, maps: maps.map(map => ({ target: map.target, milliseconds: (map.milliseconds as { initializationExcludingDownload: number }).initializationExcludingDownload })) })}`)
  await pageCdp.detach()
  await browserCdp.detach()
})
