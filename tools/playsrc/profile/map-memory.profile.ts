import { mkdir, writeFile } from "node:fs/promises"
import { execFile } from "node:child_process"
import path from "node:path"
import { promisify } from "node:util"
import { expect, test } from "./application-test"
import { decodeScreenshot } from "./screenshot-pixels"
import { divideProfileWindow, profileSampleSeconds } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"

const TARGETS = ["jump_beef", "ctf_2fort", "pl_upward"] as const
const executeFile = promisify(execFile)

type BrowserProcess = Readonly<{ type: string; id: number; cpuTime: number }>
type ProcessMemory = Readonly<{ id: number; type: string; residentBytes: number; privateBytes: number | null }>
type MemorySample = Readonly<{
  at: number
  target: string
  phase: string
  processes: readonly ProcessMemory[]
  residentBytes: number
  privateBytes: number | null
}>

async function residentProcesses(processes: readonly BrowserProcess[]): Promise<readonly ProcessMemory[]> {
  const identities = [...new Set(processes.map((process) => process.id).filter((id) => Number.isSafeInteger(id) && id > 0))]
  if (identities.length === 0) return []
  const command = process.platform === "win32"
    ? ["powershell", "-NoProfile", "-Command", `@(Get-Process -Id ${identities.join(",")} -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64,PrivateMemorySize64) | ConvertTo-Json -Compress`]
    : ["ps", "-o", "pid=,rss=", "-p", identities.join(",")]
  const { stdout: output } = await executeFile(command[0]!, command.slice(1))
  if (!output.trim()) return []
  const types = new Map(processes.map((entry) => [entry.id, entry.type]))
  if (process.platform === "win32") {
    const parsed = JSON.parse(output) as { Id: number; WorkingSet64: number; PrivateMemorySize64: number } | { Id: number; WorkingSet64: number; PrivateMemorySize64: number }[]
    return (Array.isArray(parsed) ? parsed : [parsed]).map((entry) => Object.freeze({
      id: entry.Id,
      type: types.get(entry.Id) ?? "unknown",
      residentBytes: entry.WorkingSet64,
      privateBytes: entry.PrivateMemorySize64,
    }))
  }
  return output.trim().split("\n").flatMap((line) => {
    const [identity, resident] = line.trim().split(/\s+/u).map(Number)
    return Number.isSafeInteger(identity) && Number.isSafeInteger(resident)
      ? [Object.freeze({ id: identity!, type: types.get(identity!) ?? "unknown", residentBytes: resident! * 1024, privateBytes: null })]
      : []
  })
}

test("headed three-map peak browser, Worker, WASM, GPU, transfer, and Ready residency", async ({ page, browser }, testInfo) => {
  const requested = process.env.PROFILE_MEMORY_TARGET
  if (requested !== undefined && !TARGETS.includes(requested as typeof TARGETS[number])) {
    throw new Error("PROFILE_MEMORY_TARGET must name one configured map")
  }
  const targets = requested === undefined ? TARGETS : [requested as typeof TARGETS[number]]
  const sampleSeconds = profileSampleSeconds()
  const sampleWindows = divideProfileWindow(sampleSeconds, targets.length)
  const output = path.join((await loadLocalConfig()).sourceCacheDir, "profiles", "map-memory")
  await mkdir(output, { recursive: true })
  const browserCdp = await browser.newBrowserCDPSession()
  const pageCdp = await page.context().newCDPSession(page)
  const system = await browserCdp.send("SystemInfo.getInfo") as { gpu?: { devices?: unknown; featureStatus?: unknown } }
  const timeline: MemorySample[] = []
  let target = "startup"
  let phase = "startup"
  let busy = false
  const sample = async (): Promise<void> => {
    if (busy) return
    busy = true
    try {
      const snapshot = await browserCdp.send("SystemInfo.getProcessInfo") as { processInfo: BrowserProcess[] }
      const processes = await residentProcesses(snapshot.processInfo)
      timeline.push(Object.freeze({
        at: performance.now(),
        target,
        phase,
        processes,
        residentBytes: processes.reduce((total, entry) => total + entry.residentBytes, 0),
        privateBytes: processes.every((entry) => entry.privateBytes !== null)
          ? processes.reduce((total, entry) => total + entry.privateBytes!, 0) : null,
      }))
    } catch (error) {
      if (timeline.length === 0) console.error(`[map-memory] process sampler: ${String(error)}`)
    } finally {
      busy = false
    }
  }
  const sampler = setInterval(() => { void sample() }, 175)

  await page.route(/gameplay-worker\.ts(?:\?|$)/u, async (route) => {
    const response = await route.fetch()
    const source = await response.text()
    const prefix = `
      const __playsrcNativeInstantiate = WebAssembly.instantiate;
      WebAssembly.instantiate = async function (...args) {
        const result = await __playsrcNativeInstantiate.apply(this, args);
        const instance = result instanceof WebAssembly.Instance ? result : result.instance;
        if (instance?.exports?.memory instanceof WebAssembly.Memory) globalThis.__playsrcProfileWasmMemory = instance.exports.memory;
        return result;
      };
      const __playsrcNativePost = globalThis.postMessage.bind(globalThis);
      globalThis.postMessage = function (message, transfer) {
        if (message && typeof message === "object") message.__playsrcProfileMemory = {
          wasmLinearBytes: globalThis.__playsrcProfileWasmMemory?.buffer.byteLength ?? null,
          heapBytes: globalThis.performance?.memory?.usedJSHeapSize ?? null,
        };
        return __playsrcNativePost(message, transfer);
      };
    `
    await route.fulfill({ response, body: `${prefix}\n${source}` })
  })

  await page.addInitScript(() => {
    const state = {
      transfers: [] as Record<string, unknown>[],
      worker: [] as Record<string, unknown>[],
      gpu: {
        bufferBytes: 0,
        peakBufferBytes: 0,
        textureBytes: 0,
        peakTextureBytes: 0,
        compressedTextureBytes: 0,
        compressedTextures: 0,
        uploadedBufferBytes: 0,
        uploadedTextureBytes: 0,
        stagingBytes: 0,
        peakStagingBytes: 0,
        destroyedBuffers: 0,
        destroyedTextures: 0,
        formats: {} as Record<string, number>,
      },
      indexedDb: {
        objectReads: 0,
        objectWrites: 0,
        objectWriteBytes: 0,
        metadataInventories: 0,
        metadataWrites: 0,
      },
      longTasks: [] as number[],
      garbageCollections: [] as number[],
    }
    ;(globalThis as any).__playsrcMemoryProfile = state
    ;(globalThis as any).__playsrcProfile = {}
    const buffers = (value: unknown, seen = new Set<unknown>()): ArrayBuffer[] => {
      if (!value || typeof value !== "object" || seen.has(value)) return []
      seen.add(value)
      if (value instanceof ArrayBuffer) return [value]
      if (ArrayBuffer.isView(value)) return value.buffer instanceof ArrayBuffer ? [value.buffer] : []
      return Object.values(value).flatMap((entry) => buffers(entry, seen))
    }
    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event) => {
          const entries = buffers(event.data)
          state.worker.push({
            at: performance.now(),
            kind: event.data?.kind ?? "unknown",
            bytes: entries.reduce((total, entry) => total + entry.byteLength, 0),
            memory: event.data?.__playsrcProfileMemory ?? null,
            timings: event.data?.kind === "loaded" ? event.data.timings : undefined,
          })
        })
      }
      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        const transfer = Array.isArray(transferOrOptions) ? transferOrOptions : transferOrOptions?.transfer ?? []
        const entries = buffers(message)
        const transferred = new Set(transfer)
        state.transfers.push({
          at: performance.now(),
          kind: message?.kind ?? "unknown",
          totalBytes: entries.reduce((total, entry) => total + entry.byteLength, 0),
          transferredBytes: entries.filter((entry) => transferred.has(entry)).reduce((total, entry) => total + entry.byteLength, 0),
          clonedBytes: entries.filter((entry) => !transferred.has(entry)).reduce((total, entry) => total + entry.byteLength, 0),
        })
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })

    for (const operation of ["get", "getAll", "add", "put"] as const) {
      const original = IDBObjectStore.prototype[operation]
      Object.defineProperty(IDBObjectStore.prototype, operation, {
        configurable: true,
        writable: true,
        value(this: IDBObjectStore, ...arguments_: any[]) {
          if (this.name === "objects") {
            if (operation === "get") state.indexedDb.objectReads += 1
            if (operation === "add" || operation === "put") {
              state.indexedDb.objectWrites += 1
              state.indexedDb.objectWriteBytes += arguments_[0]?.bytes instanceof Blob ? arguments_[0].bytes.size : 0
            }
          } else if (this.name === "metadata") {
            if (operation === "getAll") state.indexedDb.metadataInventories += 1
            if (operation === "add" || operation === "put") state.indexedDb.metadataWrites += 1
          }
          return Reflect.apply(original, this, arguments_)
        },
      })
    }
    for (const [type, destination] of [["longtask", state.longTasks], ["gc", state.garbageCollections]] as const) {
      if (!PerformanceObserver.supportedEntryTypes.includes(type)) continue
      new PerformanceObserver((entries) => {
        for (const entry of entries.getEntries()) destination.push(entry.duration)
      }).observe({ type, buffered: true })
    }

    const gpuTextureBytes = (descriptor: any): number => {
      const size = typeof descriptor.size === "number" ? [descriptor.size, 1, 1]
        : Array.isArray(descriptor.size) ? descriptor.size
          : [descriptor.size.width, descriptor.size.height ?? 1, descriptor.size.depthOrArrayLayers ?? 1]
      const compressed = descriptor.format.startsWith("bc")
      const block = compressed ? (descriptor.format.startsWith("bc1") || descriptor.format.startsWith("bc4") ? 8 : 16) : 0
      const scalar = descriptor.format.includes("rgba32") ? 16
        : descriptor.format.includes("rgba16") || descriptor.format.includes("rg32") ? 8
          : descriptor.format.includes("r8") && !descriptor.format.includes("rg8") ? 1
            : descriptor.format.includes("rg8") || descriptor.format.includes("r16") ? 2 : 4
      let bytes = 0
      for (let mip = 0; mip < (descriptor.mipLevelCount ?? 1); mip += 1) {
        const width = Math.max(1, Math.floor(Number(size[0] ?? 1) / 2 ** mip))
        const height = Math.max(1, Math.floor(Number(size[1] ?? 1) / 2 ** mip))
        bytes += (compressed ? Math.ceil(width / 4) * Math.ceil(height / 4) * block : width * height * scalar)
          * Number(size[2] ?? 1) * (descriptor.sampleCount ?? 1)
      }
      return bytes
    }
    const instrument = (owner: any, name: string, wrap: (original: Function, receiver: any, arguments_: any[]) => unknown): void => {
      if (!owner?.prototype || typeof owner.prototype[name] !== "function") return
      const original = owner.prototype[name]
      Object.defineProperty(owner.prototype, name, {
        configurable: true,
        writable: true,
        value(this: unknown, ...arguments_: any[]) { return wrap(original, this, arguments_) },
      })
    }
    instrument((globalThis as any).GPUDevice, "createBuffer", (original, receiver, arguments_) => {
      const descriptor = arguments_[0]
      const result = original.apply(receiver, arguments_)
      const bytes = Number(descriptor.size)
      const staging = (Number(descriptor.usage) & 1) !== 0 || descriptor.mappedAtCreation === true
      state.gpu.bufferBytes += bytes
      state.gpu.peakBufferBytes = Math.max(state.gpu.peakBufferBytes, state.gpu.bufferBytes)
      if (staging) {
        state.gpu.stagingBytes += bytes
        state.gpu.peakStagingBytes = Math.max(state.gpu.peakStagingBytes, state.gpu.stagingBytes)
      }
      let destroyed = false
      const destroy = result.destroy
      result.destroy = function () {
        if (!destroyed) {
          destroyed = true
          state.gpu.bufferBytes -= bytes
          state.gpu.destroyedBuffers += 1
          if (staging) state.gpu.stagingBytes -= bytes
        }
        return destroy.call(this)
      }
      return result
    })
    instrument((globalThis as any).GPUDevice, "createTexture", (original, receiver, arguments_) => {
      const descriptor = arguments_[0]
      const result = original.apply(receiver, arguments_)
      const bytes = gpuTextureBytes(descriptor)
      state.gpu.textureBytes += bytes
      state.gpu.peakTextureBytes = Math.max(state.gpu.peakTextureBytes, state.gpu.textureBytes)
      state.gpu.formats[descriptor.format] = (state.gpu.formats[descriptor.format] ?? 0) + 1
      if (descriptor.format.startsWith("bc")) {
        state.gpu.compressedTextures += 1
        state.gpu.compressedTextureBytes += bytes
      }
      let destroyed = false
      const destroy = result.destroy
      result.destroy = function () {
        if (!destroyed) {
          destroyed = true
          state.gpu.textureBytes -= bytes
          state.gpu.destroyedTextures += 1
        }
        return destroy.call(this)
      }
      return result
    })
    instrument((globalThis as any).GPUQueue, "writeBuffer", (original, receiver, arguments_) => {
      const value = arguments_[2]
      state.gpu.uploadedBufferBytes += Number(arguments_[4] ?? (ArrayBuffer.isView(value) ? value.byteLength : value?.byteLength ?? 0))
      return original.apply(receiver, arguments_)
    })
    instrument((globalThis as any).GPUQueue, "writeTexture", (original, receiver, arguments_) => {
      const value = arguments_[1]
      state.gpu.uploadedTextureBytes += Number(value?.byteLength ?? 0)
      return original.apply(receiver, arguments_)
    })
  })

  const maps: Record<string, unknown>[] = []
  try {
    await page.goto("/", { waitUntil: "load", timeout: 30_000 })
    const root = page.locator("main")
    await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
    let configuration = await (await page.request.get("/playsrc-config.json")).json() as {
      assetOrigin: string
      targets: { target: string; objects: { bsp: { byteLength: string }; resources: { sha256: string } } }[]
    }
    for (const [index, identity] of targets.entries()) {
      if (!configuration.targets.some((entry) => entry.target === identity)) {
        const prepared = await page.request.post(`/__playsrc/prepare-target/${identity}`)
        expect(prepared.status()).toBe(200)
        configuration = await prepared.json() as typeof configuration
      }
      if (targets.length > 1 && index === targets.length - 1) {
        target = "page-reload"
        await page.evaluate(async () => {
          await new Promise<void>((resolve, reject) => {
            const open = indexedDB.open("playsrc-derived-v3")
            open.onerror = () => reject(open.error)
            open.onsuccess = () => {
              const database = open.result
              const stores = ["objects", ...(database.objectStoreNames.contains("metadata") ? ["metadata"] : [])]
              const transaction = database.transaction(stores, "readwrite")
              transaction.oncomplete = () => { database.close(); resolve() }
              transaction.onerror = transaction.onabort = () => { database.close(); reject(transaction.error) }
              for (const store of stores) transaction.objectStore(store).clear()
            }
          })
        })
        await page.reload({ waitUntil: "load", timeout: 30_000 })
        await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
      }
      const selected = configuration.targets.find((entry) => entry.target === identity)
      if (!selected) throw new Error(`configured map ${identity} is absent`)
      const graph = await (await page.request.get(`${configuration.assetOrigin}/objects/sha256/${selected.objects.resources.sha256}`)).json() as {
        chunks: { roles: string[]; encodedByteLength: string; decodedByteLength: string; entries: { logicalPath: string; byteLength: string }[] }[]
      }
      const chunks = graph.chunks.filter((chunk) => chunk.roles.includes("gameplay"))
      const entries = chunks.flatMap((chunk) => chunk.entries)
      target = identity
      phase = "before-command"
      await sample()
      const before = await pageCdp.send("Runtime.getHeapUsage")
      await page.keyboard.press("Backquote")
      const command = page.locator("[aria-label='Console command']")
      await expect(command).toBeVisible()
      await command.fill(`map ${identity}`)
      const started = performance.now()
      phase = "loading"
      await command.press("Enter")
      await page.waitForFunction((expected) => {
        const main = document.querySelector<HTMLElement>("main")
        return main?.dataset.phase === "Failed"
          || main?.dataset.teamSelectionVisible === "true"
          || main?.dataset.classSelectionVisible === "true"
          || main?.dataset.phase === "Ready" && (main.dataset.detail?.includes(expected) ?? false)
      }, identity, { timeout: 600_000, polling: 25 })
      if (await root.getAttribute("data-team-selection-visible") === "true") {
        if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
        await chooseTf2Team(page, "red")
      }
      if (await root.getAttribute("data-class-selection-visible") === "true") {
        if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
        await page.keyboard.press("Digit2")
      }
      await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
      const readyMilliseconds = performance.now() - started
      phase = "ready"
      await sample()
      const heap = await pageCdp.send("Runtime.getHeapUsage")
      const revision = index + 1
      await page.evaluate((value) => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = value }, revision)
      await page.waitForFunction(({ revision, identity }) => {
        const evidence = (globalThis as any).__playsrcProfile?.geometryEvidence
        return evidence?.revision === revision && evidence.target === identity && evidence.finalReady === true
      }, { revision, identity }, { timeout: 30_000 })
      const capture = await page.locator("canvas.world-canvas").screenshot()
      const image = decodeScreenshot(capture)
      let visible = 0
      for (let offset = 0; offset < image.pixels.length; offset += image.channels) {
        if (image.pixels[offset]! + image.pixels[offset + 1]! + image.pixels[offset + 2]! > 16) visible += 1
      }
      expect(visible).toBeGreaterThan(image.width * image.height / 8)
      await writeFile(path.join(output, `${identity}-${process.env.PROFILE_MEMORY_LABEL ?? "current"}.png`), capture)
      const observed = await page.evaluate(() => {
        const main = document.querySelector<HTMLElement>("main")!
        const profile = (globalThis as any).__playsrcMemoryProfile
        return {
          generation: Number(main.dataset.generation),
          tick: Number(main.dataset.snapshotTick),
          playerClass: Number((main.dataset.hudProbe ?? "").split(":")[1]),
          load: JSON.parse(main.dataset.loadPerformance ?? "null"),
          staticProps: JSON.parse(document.querySelector<HTMLElement>("canvas.world-canvas")?.dataset.staticProps ?? "null"),
          gpu: profile.gpu,
          transfers: profile.transfers,
          worker: profile.worker,
          indexedDb: profile.indexedDb,
          longTasks: profile.longTasks,
          garbageCollections: profile.garbageCollections,
          assets: (globalThis as any).__playsrcProfile.memoryAssets,
          geometry: (globalThis as any).__playsrcProfile.geometryEvidence,
        }
      })
      const admitted = new Set(observed.geometry.visibility.drawSurfaces)
      const worldDepth = observed.geometry.geometry.samples.filter((entry: any) => entry.disposition === "main-world"
        && entry.depth !== null && entry.depth > observed.geometry.camera.near
        && entry.primitive !== null && admitted.has(entry.primitive)
        && Number.isSafeInteger(entry.object) && typeof entry.material === "string")
      expect(worldDepth.length).toBeGreaterThan(0)
      expect(observed.assets.target).toBe(identity)
      expect(observed.assets.compressedTextures).toBeGreaterThan(0)
      expect(observed.playerClass).toBe(3)
      if (index === 1 && targets.length > 1) expect(observed.load.client.modelCacheHits).toBeGreaterThan(0)
      const frames = await page.evaluate(async (minimumMilliseconds) => {
        const root = document.querySelector<HTMLElement>("main")!
        const first = Number(root.dataset.snapshotTick)
        const start = performance.now()
        let count = 0
        let previous = start
        const gaps: number[] = []
        await new Promise<void>((resolve) => {
          const frame = (now: number): void => {
            count += 1
            if (now > previous) gaps.push(now - previous)
            previous = now
            if (now - start >= minimumMilliseconds) resolve()
            else requestAnimationFrame(frame)
          }
          requestAnimationFrame(frame)
        })
        const ordered = gaps.toSorted((left, right) => left - right)
        return {
          milliseconds: performance.now() - start,
          count,
          firstTick: first,
          lastTick: Number(root.dataset.snapshotTick),
          displayedFramesPerSecond: count * 1_000 / (performance.now() - start),
          frameP95Milliseconds: ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * 0.95))] ?? 0,
          frameP99Milliseconds: ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * 0.99))] ?? 0,
          maximumFrameMilliseconds: ordered.at(-1) ?? 0,
        }
      }, sampleWindows[index]! * 1_000)
      const simulationHz = (frames.lastTick - frames.firstTick) * 1_000 / frames.milliseconds
      expect(simulationHz).toBeGreaterThan(55)
      const own = timeline.filter((entry) => entry.target === identity)
      const peak = own.reduce((maximum, entry) => entry.residentBytes > maximum.residentBytes ? entry : maximum, own[0]!)
      maps.push({
        target: identity,
        readyMilliseconds: Number(readyMilliseconds.toFixed(3)),
        generation: observed.generation,
        playerClass: observed.playerClass,
        source: {
          bspBytes: Number(selected.objects.bsp.byteLength),
          chunks: chunks.length,
          encodedBytes: chunks.reduce((total, chunk) => total + Number(chunk.encodedByteLength), 0),
          decodedBytes: chunks.reduce((total, chunk) => total + Number(chunk.decodedByteLength), 0),
          entries: entries.length,
          vtfBytes: entries.filter((entry) => entry.logicalPath.endsWith(".vtf")).reduce((total, entry) => total + Number(entry.byteLength), 0),
          modelBytes: entries.filter((entry) => /\.(?:mdl|vvd|vtx|ani|phy)$/u.test(entry.logicalPath)).reduce((total, entry) => total + Number(entry.byteLength), 0),
        },
        memory: {
          sampleCount: own.length,
          peakResidentBytes: peak.residentBytes,
          residentReadyBytes: own.at(-1)!.residentBytes,
          peakPrivateBytes: peak.privateBytes,
          processesAtPeak: peak.processes,
          mainHeapBeforeBytes: before.usedSize,
          mainHeapBeforeBackingBytes: before.backingStorageSize,
          mainHeapReadyBytes: heap.usedSize,
          mainHeapReadyBackingBytes: heap.backingStorageSize,
          wasmLinearBytes: Math.max(0, ...observed.worker.map((record: any) => Number(record.memory?.wasmLinearBytes ?? 0))),
        },
        gpu: observed.gpu,
        indexedDb: observed.indexedDb,
        responsiveness: {
          longTasks: observed.longTasks.length,
          maximumLongTaskMilliseconds: Math.max(0, ...observed.longTasks),
          garbageCollections: observed.garbageCollections.length,
          maximumGarbageCollectionMilliseconds: Math.max(0, ...observed.garbageCollections),
        },
        transfers: observed.transfers,
        worker: observed.worker.filter((record: any) => ["resources", "loaded"].includes(record.kind)),
        load: observed.load,
        staticProps: observed.staticProps,
        assets: observed.assets,
        geometry: {
          generation: observed.geometry.generation,
          eyeLeaf: observed.geometry.visibility.eyeLeaf,
          outsideWorld: observed.geometry.visibility.outsideWorld,
          visibleSurfaces: observed.geometry.visibility.drawSurfaces.length,
          worldDepthSamples: worldDepth.length,
          nearestDepth: Math.min(...worldDepth.map((entry: any) => entry.depth)),
        },
        pixels: { width: image.width, height: image.height, visible },
        simulation: { ...frames, hz: Number(simulationHz.toFixed(2)) },
      })
      await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}-partial.json`), `${JSON.stringify({ maps, timeline }, null, 2)}\n`)
      console.log(`PLAYSRC_MAP_MEMORY ${JSON.stringify({ target: identity, readyMilliseconds, peakResidentBytes: peak.residentBytes, wasmLinearBytes: (maps.at(-1) as any).memory.wasmLinearBytes })}`)
    }
    const report = {
      schema: "playsrc-headed-three-map-memory-v1",
      headed: true,
      startupMovie: "skipped",
      requestedActiveSeconds: sampleSeconds,
      platform: process.platform,
      architecture: process.arch,
      gpu: system.gpu ?? null,
      maps,
      timeline,
    }
    const serialized = `${JSON.stringify(report, null, 2)}\n`
    const label = process.env.PROFILE_MEMORY_LABEL ?? "current"
    await writeFile(path.join(output, `${label}.json`), serialized)
    await writeFile(path.join(output, "report.json"), serialized)
    await testInfo.attach("headed-three-map-memory", { body: Buffer.from(serialized), contentType: "application/json" })
  } finally {
    clearInterval(sampler)
    await browserCdp.detach().catch(() => {})
  }
})
