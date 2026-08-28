import { closeSync, openSync, writeSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { execFile } from "node:child_process"
import path from "node:path"
import { promisify } from "node:util"
import { expect, test, guardStartupInput } from "./application-test"
import { decodeScreenshot } from "./screenshot-pixels"
import { divideProfileWindow, profileSampleSeconds } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { macosProcessMemorySampler } from "./process-memory-macos"
import { activeGameplayTraceWindow, summarizeCompositorTruth, type ChromiumTraceEvent } from "./compositor-truth"
import { mapMemoryReply } from "./map-memory-reply"
import { installGpuTextureAccounting } from "./gpu-texture-accounting"
import { installLightmapAllocationProbe } from "./lightmap-allocation-probe"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { instrumentLightmapSceneSource } from "./lightmap-scene-route"
import { instrumentParticleAliasSource } from "./particle-alias-route"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { windowsProcessMemory } from "./windows-process-memory"
import { fixedInputPulses } from "./fixed-input-pulses"
import { instrumentWaterTargetSceneSource } from "./water-target-scene-route"

const TARGETS = ["jump_beef", "ctf_2fort", "pl_upward"] as const
const executeFile = promisify(execFile)
const helperProcesses: { pid?: number; startedEpoch: number; endedEpoch?: number; command: string }[] = []

type BrowserProcess = Readonly<{ type: string; id: number; cpuTime: number }>
type ProcessMemory = Readonly<{ id: number; type: string; residentBytes: number; privateBytes: number | null }>
type MemorySample = Readonly<{
  at: number
  target: string
  phase: string
  processes: readonly ProcessMemory[]
  residentBytes: number
  privateBytes: number | null
  browser?: Record<string, unknown>
  js?: Readonly<{ usedSize: number; backingStorageSize?: number }>
}>

async function residentProcesses(processes: readonly BrowserProcess[], macosSampler?: string, windowsSampler?: ReturnType<typeof windowsProcessMemory>, hostSamples?: any[]): Promise<readonly ProcessMemory[]> {
  const identities = [...new Set(processes.map((process) => process.id).filter((id) => Number.isSafeInteger(id) && id > 0))]
  if (identities.length === 0) return []
  if (windowsSampler) {
    const result = await windowsSampler.read(identities), types = new Map(processes.map(entry => [entry.id, entry.type]))
    hostSamples?.push({ ...result.host, helper: result.helper, browserCpu: processes.map(({ id, type, cpuTime }) => ({ id, type, cpuTime })) })
    return result.processes.map((entry: any) => ({ id: entry.Id, type: types.get(entry.Id) ?? "unknown", residentBytes: entry.WorkingSet64, privateBytes: entry.PrivateMemorySize64 }))
  }
  if (process.platform === "win32") throw new Error("Windows process telemetry must use its owned persistent reader")
  const command = macosSampler ? [macosSampler, ...identities.map(String)] : ["ps", "-o", "pid=,rss=", "-p", identities.join(",")]
  // This is a non-GUI telemetry helper, not the headed browser. A console
  // window from each sample can itself change native foreground ownership.
  const pending = executeFile(command[0]!, command.slice(1), { timeout: 2_000, windowsHide: true })
  const receipt = { pid: (pending as typeof pending & { child?: { pid?: number } }).child?.pid, startedEpoch: Date.now(), endedEpoch: undefined as number | undefined, command: "process-memory-sampler" }
  if (process.env.PROFILE_MEMORY_INPUT_DIAGNOSTIC === "1" && helperProcesses.length < 512) helperProcesses.push(receipt)
  let output: string
  try { output = (await pending).stdout } finally { receipt.endedEpoch = Date.now() }
  if (!output.trim()) return []
  const types = new Map(processes.map((entry) => [entry.id, entry.type]))
  return output.trim().split("\n").flatMap((line) => {
    const [identity, resident, privateBytes] = line.trim().split(/\s+/u).map(Number)
    return Number.isSafeInteger(identity) && Number.isSafeInteger(resident)
      ? [Object.freeze({ id: identity!, type: types.get(identity!) ?? "unknown", residentBytes: resident! * (macosSampler ? 1 : 1024), privateBytes: macosSampler && Number.isSafeInteger(privateBytes) ? privateBytes! : null })]
      : []
  })
}

test("headed three-map peak browser, Worker, WASM, GPU, transfer, and Ready residency", async ({ page, browser }, testInfo) => {
  const wallStarted = performance.now()
  const requested = process.env.PROFILE_MEMORY_TARGET
  const requestedSequence = process.env.PROFILE_MEMORY_SEQUENCE?.split(",")
  const requestedBots = Number(process.env.PROFILE_MEMORY_BOTS ?? 0)
  if (![0, 15, 23].includes(requestedBots)) throw new Error("PROFILE_MEMORY_BOTS must be 0, 15, or 23")
  if (requested !== undefined && !TARGETS.includes(requested as typeof TARGETS[number])) {
    throw new Error("PROFILE_MEMORY_TARGET must name one configured map")
  }
  if (requestedSequence && (requested !== undefined || requestedSequence.length < 2
    || requestedSequence.some((identity) => !TARGETS.includes(identity as typeof TARGETS[number])))) {
    throw new Error("PROFILE_MEMORY_SEQUENCE must name at least two configured maps without PROFILE_MEMORY_TARGET")
  }
  const targets = requestedSequence
    ? requestedSequence as (typeof TARGETS[number])[]
    : requested === undefined ? TARGETS : [requested as typeof TARGETS[number]]
  const sampleSeconds = profileSampleSeconds()
  const sampleWindows = divideProfileWindow(sampleSeconds, targets.length)
  const output = path.join((await loadLocalConfig()).sourceCacheDir, "profiles", "map-memory")
  const lightmapAudit = process.env.PROFILE_MEMORY_LIGHTMAP_AUDIT === "1"
  const aliasPixels = process.env.PROFILE_MEMORY_ALIAS_PIXELS === "1"
  const aliasCombat = process.env.PROFILE_MEMORY_ALIAS_COMBAT === "1"
  if (aliasCombat) await page.addInitScript({ content: `(${installBrowserFrameProfiler.toString()})();` })
  const inputDiagnostic = process.env.PROFILE_MEMORY_INPUT_DIAGNOSTIC === "1"
  const ownedUiDiagnostic = process.env.PROFILE_MEMORY_OWNED_UI_DIAGNOSTIC === "1"
  const nativeEvidenceDirectory = path.join(output, "native", randomUUID())
  let samplerFailure: string | undefined
  let permissionResolved = false
  const nativeReader = lightmapAudit ? await startupNativeReader(page, (await loadLocalConfig()).sourceCacheDir) : null
  const diagnoseUi = async (stage: string) => {
    if (!nativeReader) throw new Error("Owned UI diagnostic reader is unavailable")
    await mkdir(nativeEvidenceDirectory, { recursive: true })
    const filename = `${stage}-${randomUUID()}`
    const record = await nativeReader.diagnoseOwnedWindow(path.join(nativeEvidenceDirectory, `${filename}.png`))
    await writeFile(path.join(nativeEvidenceDirectory, `${filename}.json`), JSON.stringify({ diagnosticOnly: true, record }, null, 2))
  }
  const native = async () => { if (nativeReader) {
    if (samplerFailure) throw new Error(`Process/host telemetry failed: ${samplerFailure}`)
    const value = await nativeReader.read()
    try { requireStartupNative(value) } catch (error) {
      if (ownedUiDiagnostic || process.env.PROFILE_MEMORY_LOCAL_PERMISSION === "1") await diagnoseUi("guard-failure")
      if (process.env.PROFILE_MEMORY_LOCAL_PERMISSION !== "1" || permissionResolved) throw error
      const action = await nativeReader.allowOwnedLocalPermission(path.join(nativeEvidenceDirectory, "permission-before.png"))
      permissionResolved = true
      await writeFile(path.join(nativeEvidenceDirectory, "permission-action.json"), JSON.stringify(action, null, 2))
      await expect.poll(async () => (await nativeReader.read()).foreground, { timeout: 3_000 }).toBe(true)
      requireStartupNative(await nativeReader.read())
      await diagnoseUi("permission-after")
    }
  } }
  if (inputDiagnostic) guardStartupInput(page, native)
  if (process.env.PROFILE_MEMORY_INPUT_DIAGNOSTIC === "1") await page.addInitScript(() => {
    const state = { events: [] as object[], dropped: 0 }
    ;(globalThis as any).__playsrcInputDiagnosis = state
    for (const kind of ["keydown", "keyup", "pointerdown", "pointerup", "pointermove", "wheel", "focus", "blur", "visibilitychange"]) addEventListener(kind, event => {
      const key = event as KeyboardEvent, pointer = event as PointerEvent
      const record = { kind, epoch: performance.timeOrigin + performance.now(), trusted: event.isTrusted,
        code: key.code, clientX: pointer.clientX, clientY: pointer.clientY, button: pointer.button,
        visible: document.visibilityState, focused: document.hasFocus() }
      if (state.events.length < 512) state.events.push(record); else state.dropped++
    }, { capture: true, passive: true })
  })
  const macosSampler = await macosProcessMemorySampler((await loadLocalConfig()).sourceCacheDir)
  const windowsSampler = process.platform === "win32" ? windowsProcessMemory((await loadLocalConfig()).sourceCacheDir) : undefined
  const hostSamples: any[] = []
  const sourceReceipts: object[] = []
  if (windowsSampler) helperProcesses.push(windowsSampler.receipt)
  await mkdir(output, { recursive: true })
  const browserCdp = await browser.newBrowserCDPSession()
  const pageCdp = await page.context().newCDPSession(page)
  await pageCdp.send("HeapProfiler.startSampling", { samplingInterval: 32_768, includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true })
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
      const processes = await residentProcesses(snapshot.processInfo, macosSampler, windowsSampler, hostSamples).catch(error => {
        samplerFailure ??= String(error); throw error
      })
      const [js, browserState] = await Promise.all([
        pageCdp.send("Runtime.getHeapUsage"),
        page.evaluate(() => {
          const state = (globalThis as any).__playsrcMemoryProfile
          return state ? {
            phase: (globalThis as any).__playsrcProfile?.mapResidencyPhase,
            gpu: { ...state.gpu }, worker: state.worker.at(-1)?.memory,
          } : undefined
        }),
      ])
      timeline.push(Object.freeze({
        at: performance.now(),
        target,
        phase,
        browser: browserState,
        js,
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

  if (lightmapAudit) await page.route(/\/packages\/presentation\/rendering\/src\/index\.ts(?:\?|$)/u, async route => {
    const response = await route.fetch()
    let source = instrumentLightmapSceneSource(await response.text(), process.env.PROFILE_MEMORY_LIGHTMAP_REFERENCE === "1")
    if (process.env.PROFILE_MEMORY_TEXTURE_OWNERS === "1") source = instrumentWaterTargetSceneSource(source, process.env.PROFILE_MEMORY_WATER_TARGET_REFERENCE === "1")
    const canonicalOwnerHash = aliasCombat ? createHash("sha256").update(instrumentParticleAliasSource(source, true)).digest("hex") : undefined
    if (aliasPixels) source = instrumentParticleAliasSource(source, false)
    else if (aliasCombat) source = instrumentParticleAliasSource(source, process.env.PROFILE_MEMORY_ALIAS_REFERENCE === "1")
    else if (process.env.PROFILE_MEMORY_ALIAS_REFERENCE === "1") source = instrumentParticleAliasSource(source, true, false)
    sourceReceipts.push({ canonicalOwnerHash, deliveredHash: createHash("sha256").update(source).digest("hex"), reference: process.env.PROFILE_MEMORY_ALIAS_REFERENCE === "1" })
    await route.fulfill({ response, body: source })
  })

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
         const response = message?.kind === "reply-control" ? message.response : message;
         if (response && typeof response === "object") {
           const owned = globalThis.__playsrcWorkerMemory;
           response.__playsrcProfileMemory = {
            wasmLinearBytes: globalThis.__playsrcProfileWasmMemory?.buffer.byteLength ?? null,
            allocatorLiveBytes: owned?.liveBytes ?? null,
            allocatorHighWaterBytes: owned?.highWaterBytes ?? null,
            borrowedModelSourceBytes: owned?.borrowedModelSourceBytes ?? null,
            copiedModelSourceBytes: owned?.copiedModelSourceBytes ?? null,
            modelSourceSectionBytes: owned?.modelSourceSectionBytes ?? null,
             resourceBytes: owned?.resourceBytes ?? null,
             resourceReferencedBytes: owned?.resourceReferencedBytes ?? null,
             sharedResourceBytes: owned?.sharedResourceBytes ?? null,
              resourceSections: /^(?:resources|loaded|activated|discarded|shutdown|initialized|failure)/.test(response.kind) ? owned?.resourceSections ?? null : undefined,
            heapBytes: globalThis.performance?.memory?.usedJSHeapSize ?? null,
           };
         }
        return __playsrcNativePost(message, transfer);
      };
    `
    await route.fulfill({ response, body: `${prefix}\n${source}` })
  })

  await page.addInitScript(({ memoryReplySource, textureAccountingSource, lightmapProbeSource, textureOwners }) => {
    const memoryReply = new Function(`return (${memoryReplySource})`)() as typeof mapMemoryReply
    const textureAccounting = new Function(`return (${textureAccountingSource})`)() as typeof installGpuTextureAccounting
    const state = {
      transfers: [] as Record<string, unknown>[],
      worker: [] as Record<string, unknown>[],
      requests: [] as Record<string, unknown>[],
      hashes: [] as Record<string, unknown>[],
      inputEvents: [] as Record<string, unknown>[],
      gpu: {
        bufferBytes: 0,
        peakBufferBytes: 0,
        textureAllocation: textureAccounting(globalThis, textureOwners),
        lightmapAllocation: new Function(`return (${lightmapProbeSource})`)()(),
        uploadedBufferBytes: 0,
        stagingBytes: 0,
        peakStagingBytes: 0,
        destroyedBuffers: 0,
        queuedWriteBytes: 0,
        peakQueuedWriteBytes: 0,
        pendingSubmissions: 0,
        peakPendingSubmissions: 0,
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
    ;(globalThis as any).__playsrcProfile = { lightmapStages: [] }
    for (const kind of ["keydown", "keyup"]) addEventListener(kind, (event) => {
      if ((event as KeyboardEvent).code === "KeyW") state.inputEvents.push({ kind, at: performance.now(), trusted: event.isTrusted })
    })
    const originalFetch = globalThis.fetch
    globalThis.fetch = async (...args) => {
      const record: Record<string, unknown> = { url: args[0] instanceof Request ? args[0].url : String(args[0]), started: performance.now() }
      state.requests.push(record)
      try {
        const response = await originalFetch(...args)
        record.bytes = Number(response.headers.get("content-length") ?? 0)
        record.status = response.status
        return response
      } finally { record.finished = performance.now() }
    }
    const originalDigest = SubtleCrypto.prototype.digest
    SubtleCrypto.prototype.digest = async function (algorithm, data) {
      const record = { started: performance.now(), bytes: data.byteLength, finished: 0 }
      state.hashes.push(record)
      try { return await originalDigest.call(this, algorithm, data) }
      finally { record.finished = performance.now() }
    }
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
        let ownership: unknown = null
        const previous = (this as any).__playsrcProfileReply?.bind(this)
        ;(this as any).__playsrcProfileReply = (response: any) => {
          previous?.(response)
          const event = { data: response }
          const entries = buffers(event.data)
          const memory = event.data?.__playsrcProfileMemory
          if (memory?.resourceSections !== undefined) ownership = memory
          const record: Record<string, unknown> = {
            at: performance.now(),
            kind: event.data?.kind ?? "unknown",
            bytes: entries.reduce((total, entry) => total + entry.byteLength, 0),
            memory: memoryReply(event.data, ownership),
            payloadSha256: event.data?.kind === "loaded" ? event.data.payloadSha256 : undefined,
            timings: event.data?.kind === "loaded" ? event.data.timings : undefined,
          }
          state.worker.push(record)
          if (event.data?.kind === "loaded" && event.data.presentation instanceof ArrayBuffer) {
            void originalDigest.call(crypto.subtle, "SHA-256", event.data.presentation).then(digest => {
              record.presentationSha256 = Array.from(new Uint8Array(digest), value => value.toString(16).padStart(2, "0")).join("")
            })
          }
        }
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
      let staging = descriptor.mappedAtCreation === true
      state.gpu.bufferBytes += bytes
      state.gpu.peakBufferBytes = Math.max(state.gpu.peakBufferBytes, state.gpu.bufferBytes)
      if (staging) {
        state.gpu.stagingBytes += bytes
        state.gpu.peakStagingBytes = Math.max(state.gpu.peakStagingBytes, state.gpu.stagingBytes)
      }
      let destroyed = false
      const unmap = result.unmap
      result.unmap = function () {
        const value = unmap.call(this)
        if (staging && !destroyed) { state.gpu.stagingBytes -= bytes; staging = false }
        return value
      }
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
    const unsentWrites = new WeakMap<object, number>()
    const queuedWrite = (queue: object, bytes: number) => {
      unsentWrites.set(queue, (unsentWrites.get(queue) ?? 0) + bytes)
      state.gpu.queuedWriteBytes += bytes
      state.gpu.peakQueuedWriteBytes = Math.max(state.gpu.peakQueuedWriteBytes, state.gpu.queuedWriteBytes)
    }
    instrument((globalThis as any).GPUQueue, "submit", (original, receiver, arguments_) => {
      const value = original.apply(receiver, arguments_)
      const bytes = unsentWrites.get(receiver) ?? 0
      unsentWrites.set(receiver, 0)
      state.gpu.pendingSubmissions += 1
      state.gpu.peakPendingSubmissions = Math.max(state.gpu.peakPendingSubmissions, state.gpu.pendingSubmissions)
      const complete = () => { state.gpu.queuedWriteBytes -= bytes; state.gpu.pendingSubmissions -= 1 }
      void receiver.onSubmittedWorkDone().then(complete, complete)
      return value
    })
    instrument((globalThis as any).GPUQueue, "writeBuffer", (original, receiver, arguments_) => {
      const value = arguments_[2]
      const scalar = value?.BYTES_PER_ELEMENT ?? 1
      const bytes = arguments_[4] === undefined ? value.byteLength - (arguments_[3] ?? 0) * scalar : arguments_[4] * scalar
      const result = original.apply(receiver, arguments_)
      state.gpu.uploadedBufferBytes += bytes
      queuedWrite(receiver, bytes)
      return result
    })
    instrument((globalThis as any).GPUQueue, "writeTexture", (original, receiver, arguments_) => {
      const value = arguments_[1]
      const result = original.apply(receiver, arguments_)
      const bytes = Number(value?.byteLength ?? 0)
      queuedWrite(receiver, bytes)
      return result
    })
  }, { memoryReplySource: mapMemoryReply.toString(), textureAccountingSource: installGpuTextureAccounting.toString(), lightmapProbeSource: installLightmapAllocationProbe.toString(), textureOwners: process.env.PROFILE_MEMORY_TEXTURE_OWNERS === "1" })

  const maps: Record<string, unknown>[] = []
  try {
    if (inputDiagnostic) await native()
    await page.goto("/", { waitUntil: "load", timeout: 30_000 })
    const root = page.locator("main")
    await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
    if (lightmapAudit && !inputDiagnostic) await page.bringToFront()
    await native()
    if (ownedUiDiagnostic) { await diagnoseUi("main-menu"); return }
    if (aliasCombat) await page.evaluate(async url => {
      const module = await import(/* @vite-ignore */ url)
      ;(globalThis as any).__playsrcParticleAliasEvidence = module.installParticleAliasOwnerReceipt()
    }, `/@fs/${repositoryRoot}/packages/presentation/rendering/src/particle-alias-owner-evidence.ts`)
    if (aliasPixels) await page.evaluate(async url => {
      const module = await import(/* @vite-ignore */ url)
      ;(globalThis as any).__playsrcParticleAliasEvidence = module.installParticleAliasEvidence()
    }, `/@fs/${repositoryRoot}/packages/presentation/rendering/src/particle-alias-evidence.ts`)
    if (lightmapAudit) await page.evaluate(async url => {
      const module = await import(/* @vite-ignore */ url)
      ;(globalThis as any).__playsrcLightmapEvidence = module.installLightmapUploadEvidence()
    }, `/@fs/${repositoryRoot}/packages/presentation/rendering/src/lightmap-upload-evidence.ts`)
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
      if (!requestedSequence && targets.length > 1 && index === targets.length - 1) {
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
      const timelineStart = timeline.length
      await sample()
      const before = await pageCdp.send("Runtime.getHeapUsage")
      if (inputDiagnostic) await native()
      else await page.bringToFront()
      await page.keyboard.press("Backquote")
      const command = page.locator("[aria-label='Console command']")
      await expect(command).toBeVisible()
      await command.fill(`map ${identity}`)
      await expect(command).toBeFocused()
      await expect(command).toHaveValue(`map ${identity}`)
      const browserStarted = await page.evaluate(() => {
        const gpu = (globalThis as any).__playsrcMemoryProfile.gpu
        gpu.peakBufferBytes = gpu.bufferBytes
        gpu.textureAllocation.peakKnownBytes = gpu.textureAllocation.live.knownBytes
        gpu.peakStagingBytes = gpu.stagingBytes
        gpu.peakQueuedWriteBytes = gpu.queuedWriteBytes
        gpu.peakPendingSubmissions = gpu.pendingSubmissions
        return performance.now()
      })
      const started = performance.now()
      phase = "loading"
      await native()
      await command.press("Enter")
      if (index === 1 && process.env.PROFILE_MEMORY_CANCEL_REPLACEMENT === "1") {
        await expect(root).toHaveAttribute("data-phase", "Replacing")
        await page.waitForFunction((started) => {
          const active = Number(document.querySelector<HTMLElement>("main")?.dataset.generation)
          return (globalThis as any).__playsrcMemoryProfile.worker.some((record: any) => record.at >= started
            && record.memory?.resourceSections?.some((section: any) => section.generation > active))
        }, browserStarted, { timeout: 15_000 })
        await command.fill(`map ${targets[0]}`)
        await command.press("Enter")
        await command.fill(`map ${identity}`)
        await command.press("Enter")
      }
      await page.waitForFunction((expected) => {
        const main = document.querySelector<HTMLElement>("main")
        return main?.dataset.phase === "Failed"
          || main?.dataset.teamSelectionVisible === "true"
          || main?.dataset.classSelectionVisible === "true"
          || main?.dataset.phase === "Ready" && (main.dataset.detail?.includes(expected) ?? false)
      }, identity, { timeout: 600_000, polling: 25 })
      if (await root.getAttribute("data-team-selection-visible") === "true") {
        if (inputDiagnostic) await native()
        if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
        await chooseTf2Team(page, "red", inputDiagnostic ? native : undefined)
      }
      if (await root.getAttribute("data-class-selection-visible") === "true") {
        if (inputDiagnostic) await native()
        if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
        await page.keyboard.press("Digit2")
      }
      await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
      const readyMilliseconds = performance.now() - started
      phase = "bot-admission"
      const bots = entries.some((entry) => entry.logicalPath === `maps/${identity}.nav`) ? requestedBots : 0
      if (bots > 0) {
        if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
        await command.fill(`tf_bot_add ${bots} normal`)
        await command.press("Enter")
        await expect(root).toHaveAttribute("data-bot-count", String(bots))
        await page.keyboard.press("Backquote")
      }
      const classSwitches: Record<string, unknown>[] = []
      if (process.env.PROFILE_MEMORY_EXERCISE_CLASSES === "1") {
        phase = "class-exercise"
        const classes = [
          { name: "scout", id: 1, weapons: [4, 5, 6] },
          { name: "sniper", id: 2, weapons: [12, 13, 14] },
          { name: "soldier", id: 3, weapons: [1, 7, 8] },
          { name: "demoman", id: 4, weapons: [18, 3, 17] },
          { name: "medic", id: 5, weapons: [19, 20, 21] },
          { name: "heavyweapons", id: 6, weapons: [9, 10, 11] },
          { name: "pyro", id: 7, weapons: [15, 7, 16] },
          { name: "spy", id: 8, weapons: [50, 52, 51] },
          { name: "engineer", id: 9, weapons: [40, 41, 42] },
        ]
        for (const playerClass of classes) {
          if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
          await command.fill(`joinclass ${playerClass.name}`)
          const classStarted = performance.now()
          await command.press("Enter")
          await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe(String(playerClass.id))
          await page.keyboard.press("Backquote")
          for (const [slot, weapon] of playerClass.weapons.entries()) {
            await page.keyboard.press(`Digit${slot + 1}`)
            await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe(String(weapon))
          }
          classSwitches.push({ name: playerClass.name, weapons: playerClass.weapons, milliseconds: performance.now() - classStarted })
        }
        await page.keyboard.press("Backquote")
        await command.fill("joinclass soldier")
        await command.press("Enter")
        await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("3")
        await page.keyboard.press("Backquote")
      }
      phase = "ready"
      await native()
      await sample()
      const heap = await pageCdp.send("Runtime.getHeapUsage")
      if (aliasPixels) {
        const records: unknown[] = []
        try {
          for (const phase of [0, 1]) {
            await native()
            const record: any = await page.evaluate(async phase => Promise.race([
              (globalThis as any).__playsrcParticleAliasEvidence.capture(phase),
              new Promise((_, reject) => setTimeout(() => reject(new Error("Alias pixel capture exceeded 15 seconds")), 15_000)),
            ]), phase)
            records.push(record)
            await writeFile(path.join(output, "particle-alias-pixels.json"), JSON.stringify(records, null, 2))
            for (const plane of record.result.planes) {
              expect(plane.mismatches).toBe(0)
              expect(plane.identicalDrawOrder).toBe(true)
            }
            expect(record.result.planes.find((plane: any) => plane.plane === "color").actorPixels).toBeGreaterThan(100)
            await page.locator("canvas.world-canvas").screenshot({ path: path.join(output, `particle-alias-pixels-${phase}.png`) })
            await native()
          }
          await native()
          await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
          await expect(root).toHaveAttribute("data-pointer-locked", "true")
          await native()
          const fires = Number(await root.getAttribute("data-fire-events"))
          await page.evaluate(() => {
            ;(globalThis as any).__playsrcAliasGameplayCapture = (globalThis as any).__playsrcParticleAliasEvidence.captureGameplay()
          })
          await native()
          await page.mouse.down({ button: "left" })
          try {
            await expect.poll(async () => Number(await root.getAttribute("data-fire-events")), { timeout: 3_000 }).toBeGreaterThan(fires)
            const gameplay: any = await page.evaluate(async () => Promise.race([
              (globalThis as any).__playsrcAliasGameplayCapture,
              new Promise((_, reject) => setTimeout(() => reject(new Error("Actual alias gameplay effect was not captured")), 8_000)),
            ]))
            records.push(gameplay)
            await writeFile(path.join(output, "particle-alias-pixels.json"), JSON.stringify(records, null, 2))
            expect(gameplay.visible.length).toBeGreaterThan(0)
            for (const plane of gameplay.result.planes) { expect(plane.mismatches).toBe(0); expect(plane.identicalDrawOrder).toBe(true) }
            await page.locator("canvas.world-canvas").screenshot({ path: path.join(output, "particle-alias-gameplay.png") })
          } finally { await page.mouse.up({ button: "left" }) }
          await native()
        } finally { await page.evaluate(() => (globalThis as any).__playsrcParticleAliasEvidence.dispose()) }
        return
      }
      const revision = index + 1
      await page.evaluate((value) => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = value }, revision)
      await page.waitForFunction(({ revision, identity }) => {
        const evidence = (globalThis as any).__playsrcProfile?.geometryEvidence
        return evidence?.revision === revision && evidence.target === identity && evidence.finalReady === true
      }, { revision, identity }, { timeout: 30_000 })
      const firstPlayableMilliseconds = performance.now() - started
      const capture = await page.locator("canvas.world-canvas").screenshot()
      const image = decodeScreenshot(capture)
      let visible = 0
      for (let offset = 0; offset < image.pixels.length; offset += image.channels) {
        if (image.pixels[offset]! + image.pixels[offset + 1]! + image.pixels[offset + 2]! > 16) visible += 1
      }
      expect(visible).toBeGreaterThan(image.width * image.height / 8)
      const capturePath = path.join(output, `${identity}-${index + 1}-${process.env.PROFILE_MEMORY_LABEL ?? "current"}.png`)
      await writeFile(capturePath, capture)
      const observed = await page.evaluate((started) => {
        const main = document.querySelector<HTMLElement>("main")!
        const profile = (globalThis as any).__playsrcMemoryProfile
        const textureOwners = (globalThis as any).__playsrcTextureOwners
        if (textureOwners) textureOwners.records.push({ kind: "snapshot", at: performance.now(), generation: Number(main.dataset.generation),
          counters: structuredClone(profile.gpu.textureAllocation) })
        return {
          generation: Number(main.dataset.generation),
          tick: Number(main.dataset.snapshotTick),
          playerClass: Number((main.dataset.hudProbe ?? "").split(":")[1]),
          load: JSON.parse(main.dataset.loadPerformance ?? "null"),
          staticProps: JSON.parse(document.querySelector<HTMLElement>("canvas.world-canvas")?.dataset.staticProps ?? "null"),
          gpu: profile.gpu,
          lightmapStages: (globalThis as any).__playsrcProfile.lightmapStages,
           transfers: profile.transfers.filter((record: any) => record.at >= started),
           worker: profile.worker.filter((record: any) => record.at >= started),
          indexedDb: profile.indexedDb,
          requests: profile.requests.filter((record: any) => record.started >= started),
          hashes: profile.hashes.filter((record: any) => record.started >= started),
          startupSpans: ((globalThis as any).__playsrcProfile.startupSpans ?? []).filter((record: any) => record.started >= started),
          longTasks: profile.longTasks,
           garbageCollections: profile.garbageCollections,
           mountedFontFaces: document.fonts.size,
          assets: (globalThis as any).__playsrcProfile.memoryAssets,
          geometry: (globalThis as any).__playsrcProfile.geometryEvidence,
          owners: ((globalThis as any).__playsrcProfile.mapResidency ?? []).filter((entry: any) => entry.at >= started),
        }
      }, browserStarted)
      const admitted = new Set(observed.geometry.visibility.drawSurfaces)
      const worldDepth = observed.geometry.geometry.samples.filter((entry: any) => entry.disposition === "main-world"
        && entry.depth !== null && entry.depth > observed.geometry.camera.near
        && entry.primitive !== null && admitted.has(entry.primitive)
        && Number.isSafeInteger(entry.object) && typeof entry.material === "string")
      expect(worldDepth.length).toBeGreaterThan(0)
      expect(observed.assets.target).toBe(identity)
      expect(observed.assets.compressedTextures).toBeGreaterThan(0)
      expect(observed.playerClass).toBe(3)
      const currentResources = observed.worker.at(-1)?.memory?.resourceSections
      expect(currentResources).toHaveLength(1)
      expect(currentResources[0].generation).toBe(observed.generation)
      if (index === 1 && process.env.PROFILE_MEMORY_CANCEL_REPLACEMENT === "1") expect(observed.generation).toBeGreaterThan(2)
      if (index === 1 && targets.length > 1) expect(observed.load.client.modelCacheHits).toBeGreaterThan(0)
      expect(Math.max(0, ...observed.worker.map((record: any) => record.memory?.resourceSections?.length ?? 0))).toBeLessThanOrEqual(2)
      const gpuAdmission = observed.owners.find((entry: any) => entry.phase === "gpu-admitted")
      if (lightmapAudit) {
        const allocation = observed.gpu.textureAllocation
        for (const total of [allocation.live, allocation.created]) {
          const formats = Object.values(total.formats) as { textures: number; knownBytes: number; unknownByteTextures: number }[]
          expect(formats.reduce((sum, value) => sum + value.knownBytes, 0)).toBe(total.knownBytes)
          expect(formats.reduce((sum, value) => sum + value.textures, 0)).toBe(total.textures)
          expect(formats.reduce((sum, value) => sum + value.unknownByteTextures, 0)).toBe(total.unknownByteTextures)
          expect(total.compressedBytes).toBeLessThanOrEqual(total.knownBytes)
        }
        expect(allocation.created.textures - allocation.destroyedTextures).toBe(allocation.live.textures)
        expect(observed.gpu.lightmapAllocation.liveBytes).toBe(allocation.live.formats.rgba32float.knownBytes)
        if (index === 1) {
          const stages = observed.lightmapStages
          expect(stages).toHaveLength(2)
          expect(stages[1]).toMatchObject({ retainedSource: true, samePlane: true, borrowed: process.env.PROFILE_MEMORY_LIGHTMAP_REFERENCE !== "1" })
          const bytes = stages[1].bytes
          const multiplier = process.env.PROFILE_MEMORY_LIGHTMAP_REFERENCE === "1" ? 2 : 1
          expect(observed.gpu.lightmapAllocation).toMatchObject({ liveBytes: bytes, peakBytes: bytes * multiplier, createdBytes: bytes * multiplier, uploadBytes: bytes * multiplier })
        }
      }
      if (gpuAdmission) {
        const persisted = observed.owners.find((entry: any) => entry.phase === "cache-write-complete")
        expect(persisted).toBeDefined()
        expect(persisted.at).toBeLessThanOrEqual(gpuAdmission.at)
      }
      if (index > 0 && targets[index - 1] === identity) {
        expect(observed.worker.filter((record: any) => record.kind === "resources")).toHaveLength(0)
        expect(observed.load.mapCache).toBe("hit")
        expect(observed.load.presentationCache).toBe("hit")
      }
      const traceEvents: ChromiumTraceEvent[] = []
      if (aliasCombat) {
        await native()
        await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
        await expect(root).toHaveAttribute("data-pointer-locked", "true")
        expect(Number(await root.getAttribute("data-fire-events"))).toBe(0)
        await native()
      }
      const collectTrace = ({ value }: { value: ChromiumTraceEvent[] }) => traceEvents.push(...value)
      pageCdp.on("Tracing.dataCollected", collectTrace)
      await pageCdp.send("Tracing.start", { categories: "benchmark,viz,gpu,devtools.timeline,blink.user_timing,v8,disabled-by-default-v8.gc", options: "record-as-much-as-possible" })
      const frameSample = page.evaluate(async ({ minimumMilliseconds, combat }) => {
        const root = document.querySelector<HTMLElement>("main")!
        if (combat) await new Promise<void>(resolve => {
          addEventListener("pointerdown", () => resolve(), { capture: true, once: true })
          ;(globalThis as any).__playsrcCombatArmed = true
        })
        const first = Number(root.dataset.snapshotTick)
        const profiler = (globalThis as any).__playsrcFrameProfiler
        if (profiler) profiler.active = true
        const start = performance.now()
        performance.mark("playsrc-active-gameplay-start")
        const initialActor = { fireEvents: Number(root.dataset.fireEvents), hud: root.dataset.hudProbe, position: root.dataset.cameraPosition,
          yaw: root.dataset.cameraYaw, pitch: root.dataset.cameraPitch, pointerLocked: root.dataset.pointerLocked, cache: root.dataset.cache }
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
        const end = performance.now()
        performance.mark("playsrc-active-gameplay-end")
        if (profiler) profiler.active = false
        return {
          initialActor, start, end,
          ...(profiler ? { actualFrames: profiler.completedFrames, gpuTimestamps: profiler.gpuTimestamps, counters: profiler.counters, losses: profiler.losses, shaders: profiler.shaders, devices: profiler.devices, adapters: profiler.adapters, gpuOperations: profiler.gpuOperations } : {}),
          milliseconds: end - start,
          count,
          firstTick: first,
          lastTick: Number(root.dataset.snapshotTick),
          displayedFramesPerSecond: count * 1_000 / (performance.now() - start),
          frameP95Milliseconds: ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * 0.95))] ?? 0,
          frameP99Milliseconds: ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * 0.99))] ?? 0,
          maximumFrameMilliseconds: ordered.at(-1) ?? 0,
        }
      }, { minimumMilliseconds: sampleWindows[index]! * 1_000, combat: aliasCombat })
      const input: Record<string, unknown>[] = []
      let inputDelivery: unknown
      if (aliasCombat) {
        await page.waitForFunction(() => (globalThis as any).__playsrcCombatArmed === true)
        await native()
        await page.mouse.down({ button: "left" })
        const pulses = await fixedInputPulses({ now: () => performance.now(), wait: duration => new Promise(resolve => setTimeout(resolve, duration)), admit: native,
          send: down => down ? page.keyboard.down("w") : page.keyboard.up("w"),
          observe: async down => {
            const event = await page.evaluate(kind => (globalThis as any).__playsrcMemoryProfile.inputEvents.findLast((value: any) => value.kind === kind), down ? "keydown" : "keyup")
            await page.waitForFunction(down => {
              const speed = Number(document.querySelector<HTMLElement>("main")?.dataset.wishSpeed)
              return down ? speed > 0 : speed === 0
            }, down, { timeout: 5_000, polling: 5 })
            return page.evaluate(event => ({ ...event, observed: performance.now(), milliseconds: performance.now() - event.at }), event)
          },
        })
        inputDelivery = pulses.delivery; input.push(...pulses.observations)
      } else if (process.env.PROFILE_MEMORY_INPUT === "1") {
        await page.locator("canvas.world-canvas").focus()
        for (let trial = 0; trial < 3; trial++) {
          try {
            for (const kind of ["keydown", "keyup"] as const) {
              if (inputDiagnostic) await native()
              if (kind === "keydown") await page.keyboard.down("w")
              else await page.keyboard.up("w")
              await page.waitForFunction((down) => {
                const speed = Number(document.querySelector<HTMLElement>("main")?.dataset.wishSpeed)
                return down ? speed > 0 : speed === 0
              }, kind === "keydown", { timeout: 5_000, polling: 5 })
              input.push(await page.evaluate((kind) => {
                const event = (globalThis as any).__playsrcMemoryProfile.inputEvents.findLast((entry: any) => entry.kind === kind)
                return { ...event, observed: performance.now(), milliseconds: performance.now() - event.at }
              }, kind))
            }
          } finally { await page.keyboard.up("w") }
        }
      }
       const frames = await frameSample.finally(async () => { if (aliasCombat) await page.mouse.up({ button: "left" }) })
       await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}-active-${index}.json`), JSON.stringify({ frames, input, inputDelivery, timeline, hostSamples, sourceReceipts, nativeAdmission: nativeReader?.records, samplerFailure }, null, 2))
       if (aliasCombat) {
         expect(frames.actualFrames?.some((frame: any) => frame.detail.particleItems > 0)).toBe(true)
         expect(frames.gpuTimestamps?.length).toBeGreaterThan(0)
         expect(frames.losses).toEqual([])
         await native()
       }
      const traced = new Promise<void>((resolve) => pageCdp.once("Tracing.tracingComplete", () => resolve()))
      await pageCdp.send("Tracing.end")
      await traced
      pageCdp.off("Tracing.dataCollected", collectTrace)
      const traceWindow = activeGameplayTraceWindow(traceEvents)
      const compositor = summarizeCompositorTruth(traceEvents, frames.milliseconds, traceWindow)
      const particleOwnerReceipt = aliasCombat ? await page.evaluate(() => (globalThis as any).__playsrcParticleAliasEvidence.snapshot()) : undefined
      const simulationHz = (frames.lastTick - frames.firstTick) * 1_000 / frames.milliseconds
      expect(simulationHz).toBeGreaterThan(55)
      const own = timeline.slice(timelineStart)
      const peak = own.reduce((maximum, entry) => entry.residentBytes > maximum.residentBytes ? entry : maximum, own[0]!)
      const transition = own.filter((entry) => entry.phase === "loading")
      maps.push({
        inputDelivery,
        particleOwnerReceipt,
        compositorIncludingControlOverhead: summarizeCompositorTruth(traceEvents, frames.milliseconds),
        traceWindow,
        presentationEvents: traceEvents.filter(event => event.name === "Display::FrameDisplayed" || event.name === "PresentationFeedback" || event.name === "FramePresented"),
        configuredTarget: selected,
        target: identity,
        readyMilliseconds: Number(readyMilliseconds.toFixed(3)),
        firstPlayableMilliseconds,
        owners: observed.owners,
        compileAllocatorPhases: ["inputs", "bsp-parsed", "canonical-collision-pvs", "materials", "presentation", "runtime-models", "canonical-serialized", "collision-decals", "game-session", "allocator-high-water", "bsp-released", "static-models-released"]
          .map((phase, index) => ({ phase, liveBytes: observed.load?.client?.wasmCompileOwnerBytes?.[index] ?? null })),
        serializedMapRetainedBytes: observed.load?.client?.wasmCompileOwnerBytes?.[12] ?? null,
        generation: observed.generation,
        playerClass: observed.playerClass,
        bots,
        botAdmission: bots === 0 && requestedBots > 0 ? "configured map has no authored NAV" : "admitted",
        classSwitches,
        source: {
          bspBytes: Number(selected.objects.bsp.byteLength),
          chunks: chunks.length,
          encodedBytes: chunks.reduce((total, chunk) => total + Number(chunk.encodedByteLength), 0),
          decodedBytes: chunks.reduce((total, chunk) => total + Number(chunk.decodedByteLength), 0),
          entries: entries.length,
          vtfBytes: entries.filter((entry) => entry.logicalPath.endsWith(".vtf")).reduce((total, entry) => total + Number(entry.byteLength), 0),
          modelBytes: entries.filter((entry) => /\.(?:mdl|vvd|vtx|ani|phy)$/u.test(entry.logicalPath)).reduce((total, entry) => total + Number(entry.byteLength), 0),
          sourceSections: Object.fromEntries(
            ["vtf", "mdl", "vvd", "vtx", "ani", "phy", "nav", "wav", "mp3", "vmt", "txt", "other"].map((extension) => [
              extension,
              entries.filter((entry) => {
                const actual = entry.logicalPath.split(".").at(-1)?.toLowerCase() ?? ""
                return extension === "other"
                  ? !["vtf", "mdl", "vvd", "vtx", "ani", "phy", "nav", "wav", "mp3", "vmt", "txt"].includes(actual)
                  : actual === extension
              }).reduce((total, entry) => total + Number(entry.byteLength), 0),
            ]),
          ),
        },
        memory: {
          sampleCount: own.length,
          peakResidentBytes: peak.residentBytes,
          transitionPeakResidentBytes: Math.max(0, ...transition.map((entry) => entry.residentBytes)),
          transitionPeakPrivateBytes: Math.max(0, ...transition.map((entry) => entry.privateBytes ?? 0)),
          residentReadyBytes: own.at(-1)!.residentBytes,
          peakPrivateBytes: peak.privateBytes,
          maximumPrivateBytes: Math.max(0, ...own.map((entry) => entry.privateBytes ?? 0)),
          privateReadyBytes: own.at(-1)!.privateBytes,
          privateMemoryMetric: process.platform === "darwin" ? "proc-region-private-resident-pages" : process.platform === "win32" ? "private-committed-bytes" : "unavailable",
          processesAtPeak: peak.processes,
          mainHeapBeforeBytes: before.usedSize,
          mainHeapBeforeBackingBytes: before.backingStorageSize,
          mainHeapReadyBytes: heap.usedSize,
          mainHeapReadyBackingBytes: heap.backingStorageSize,
          mainHeapPeakBytes: Math.max(before.usedSize, heap.usedSize, ...own.map((entry) => entry.js?.usedSize ?? 0)),
          mainBackingPeakBytes: Math.max(before.backingStorageSize ?? 0, heap.backingStorageSize ?? 0, ...own.map((entry) => entry.js?.backingStorageSize ?? 0)),
          wasmLinearBytes: Math.max(0, ...observed.worker.map((record: any) => Number(record.memory?.wasmLinearBytes ?? 0))),
          wasmAllocatorLiveBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.allocatorLiveBytes ?? 0))),
          wasmAllocatorHighWaterBytes: Math.max(0, ...observed.worker.map((record: any) => Number(record.memory?.allocatorHighWaterBytes ?? 0))),
          wasmResourceBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.resourceBytes ?? 0))),
          wasmReferencedResourceBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.resourceReferencedBytes ?? 0))),
          wasmSharedResourceBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.sharedResourceBytes ?? 0))),
          borrowedModelSourceBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.borrowedModelSourceBytes ?? 0))),
          copiedModelSourceBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.copiedModelSourceBytes ?? 0))),
          modelSourceSectionBytes: Math.max(0, ...observed.worker.filter((record: any) => record.kind === "loaded").map((record: any) => Number(record.memory?.modelSourceSectionBytes ?? 0))),
        },
        gpu: observed.gpu,
        lightmapStages: observed.lightmapStages,
        indexedDb: observed.indexedDb,
        requests: observed.requests,
        hashes: observed.hashes,
        startupSpans: observed.startupSpans,
         responsiveness: {
          longTasks: observed.longTasks.length,
          maximumLongTaskMilliseconds: Math.max(0, ...observed.longTasks),
          garbageCollections: observed.garbageCollections.length,
          maximumGarbageCollectionMilliseconds: Math.max(0, ...observed.garbageCollections),
         },
         mountedFontFaces: observed.mountedFontFaces,
        transfers: observed.transfers,
        worker: observed.worker.filter((record: any) => ["resources", "resources-retained", "loaded", "activated"].includes(record.kind)),
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
        screenshot: { path: capturePath, byteLength: capture.byteLength, sha256: createHash("sha256").update(capture).digest("hex") },
        input,
        simulation: { ...frames, hz: Number(simulationHz.toFixed(2)) },
        compositor,
        tracedGarbageCollection: traceEvents.filter((event) => /(?:MajorGC|MinorGC|V8\.GC)/u.test(event.name ?? "") && event.dur !== undefined)
          .map((event) => ({ name: event.name, milliseconds: event.dur! / 1_000 })),
      })
      await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}-partial.json`), `${JSON.stringify({ maps, timeline }, null, 2)}\n`)
      if (process.env.PROFILE_MEMORY_TEXTURE_OWNERS === "1") await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL}-texture-owners.json`),
        JSON.stringify(await page.evaluate(() => (globalThis as any).__playsrcTextureOwners), null, 2))
      await native()
      console.log(`PLAYSRC_MAP_MEMORY ${JSON.stringify({ target: identity, readyMilliseconds, peakResidentBytes: peak.residentBytes, wasmLinearBytes: (maps.at(-1) as any).memory.wasmLinearBytes })}`)
    }
    clearInterval(sampler)
    let lightmapEvidence: unknown = null
    let lightmapTeardown: unknown = null
    if (lightmapAudit) {
      await native()
      try {
        lightmapEvidence = await page.evaluate(async () => Promise.race([
          (globalThis as any).__playsrcLightmapEvidence.capture(),
          new Promise((_, reject) => setTimeout(() => reject(new Error("Lightmap GPU correctness capture exceeded 15 seconds")), 15_000)),
        ]))
        for (const plane of (lightmapEvidence as any).planes) { expect(plane.mismatches).toBe(0); expect(plane.sha256).toBe(plane.canonicalSha256) }
        for (const plane of (lightmapEvidence as any).parity.planes) { expect(plane.mismatches).toBe(0); expect(plane.sha256).toBe(plane.referenceSha256) }
        expect((lightmapEvidence as any).referenceUploadBytes).toBeGreaterThanOrEqual((lightmapEvidence as any).planes[0].bytes)
        expect((lightmapEvidence as any).parity.planes.find((plane: any) => plane.plane === "depth").channels[0]).toBeGreaterThan(1)
        expect((lightmapEvidence as any).parity.planes.find((plane: any) => plane.plane === "color").channels.some((channel: number) => channel > 3)).toBe(true)
        expect((lightmapEvidence as any).planes).toHaveLength(1)
        expect((lightmapEvidence as any).parity.planes).toHaveLength(3)
        await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}-lightmap-parity.json`), JSON.stringify(lightmapEvidence, null, 2))
      } finally {
        await page.evaluate(() => { (globalThis as any).__playsrcLightmapEvidence.dispose(); delete (globalThis as any).__playsrcLightmapEvidence })
      }
      await native()
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await page.keyboard.press("Escape")
      await expect(root).toHaveAttribute("data-gameui", "pause")
      await page.locator("[data-vgui-name='DisconnectButton']").click()
      await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 10_000 })
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcMemoryProfile.gpu.lightmapAllocation.liveBytes)).toBe(0)
      lightmapTeardown = await page.evaluate(() => (globalThis as any).__playsrcMemoryProfile.gpu.textureAllocation)
      expect((lightmapTeardown as any).live.formats.rgba32float.knownBytes).toBe(0)
      expect((lightmapTeardown as any).created.textures - (lightmapTeardown as any).destroyedTextures).toBe((lightmapTeardown as any).live.textures)
      if (process.env.PROFILE_MEMORY_TEXTURE_OWNERS === "1") {
        const teardown = await page.evaluate(() => {
          const owners = (globalThis as any).__playsrcTextureOwners
          owners.records.push({ kind: "snapshot", at: performance.now(), generation: -1,
            counters: structuredClone((globalThis as any).__playsrcMemoryProfile.gpu.textureAllocation) })
          return owners
        })
        const water = new Set<number>()
        for (const record of teardown.records) if (record.owner?.startsWith("playsrc-water-")) {
          if (record.kind === "create") water.add(record.id)
          if (record.kind === "destroy") water.delete(record.id)
        }
        expect(teardown.dropped).toBe(0); expect(water.size).toBe(0)
        await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL}-texture-teardown.json`), JSON.stringify(teardown, null, 2))
      }
      await native()
    }
    const sampled = await pageCdp.send("HeapProfiler.stopSampling")
    const allocations = new Map<string, number>()
    const visit = (node: any): void => {
      const identity = `${node.callFrame.functionName || "(anonymous)"}:${node.callFrame.url || "(native)"}`
      allocations.set(identity, (allocations.get(identity) ?? 0) + Number(node.selfSize ?? 0))
      for (const child of node.children ?? []) visit(child)
    }
    visit(sampled.profile.head)
    let heapSnapshot: Record<string, unknown> | null = null
    if (process.env.PROFILE_MEMORY_HEAP_SNAPSHOT === "1") {
      const location = path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}.heapsnapshot`)
      const file = openSync(location, "w")
      const append = ({ chunk }: { chunk: string }): void => { writeSync(file, chunk) }
      pageCdp.on("HeapProfiler.addHeapSnapshotChunk", append)
      try {
        await pageCdp.send("HeapProfiler.takeHeapSnapshot", { exposeInternals: true })
      } finally {
        pageCdp.off("HeapProfiler.addHeapSnapshotChunk", append)
        closeSync(file)
      }
      const snapshot = JSON.parse(await readFile(location, "utf8")) as {
        snapshot: { meta: { node_fields: string[]; node_types: unknown[][] }; node_count: number }
        nodes: number[]
        strings: string[]
      }
      const fields = snapshot.snapshot.meta.node_fields
      const typeOffset = fields.indexOf("type")
      const nameOffset = fields.indexOf("name")
      const sizeOffset = fields.indexOf("self_size")
      const names = snapshot.snapshot.meta.node_types[typeOffset] as string[]
      const retained = new Map<string, { bytes: number; count: number }>()
      for (let index = 0; index < snapshot.nodes.length; index += fields.length) {
        const type = names[snapshot.nodes[index + typeOffset]!] ?? "unknown"
        const name = (snapshot.strings[snapshot.nodes[index + nameOffset]!] ?? "unknown").slice(0, 192)
        const identity = `${type}:${name}`
        const entry = retained.get(identity) ?? { bytes: 0, count: 0 }
        entry.bytes += snapshot.nodes[index + sizeOffset]!
        entry.count += 1
        retained.set(identity, entry)
      }
      heapSnapshot = {
        path: location,
        nodes: snapshot.snapshot.node_count,
        retained: [...retained].sort((left, right) => right[1].bytes - left[1].bytes).slice(0, 30)
          .map(([identity, value]) => ({ identity, ...value })),
      }
    }
    const report = {
      schema: "playsrc-headed-three-map-memory-v2",
      headed: true,
      startupMovie: "skipped",
      requestedActiveSeconds: sampleSeconds,
      totalWallMilliseconds: performance.now() - wallStarted,
      memoryAttribution: "OS counters are per process and resident sums may double-count shared mappings. Windows private commit differs from macOS private resident pages. Owner ranges, allocator totals, GPU API object sizes, mapped staging and outstanding write input spans are separate counters, not additive physical residency estimates. Texture unknownByteTextures has no inferred byte size; created and writeTexture counters are cumulative.",
      platform: process.platform,
      architecture: process.arch,
      gpu: system.gpu ?? null,
      browserVersion: await browserCdp.send("Browser.getVersion"),
      configuration,
      sourceReceipts,
      maps,
      lightmapEvidence,
      lightmapTeardown,
      lightmapReference: process.env.PROFILE_MEMORY_LIGHTMAP_REFERENCE === "1",
      nativeAdmission: nativeReader?.records,
      inputDiagnosis: process.env.PROFILE_MEMORY_INPUT_DIAGNOSTIC === "1" ? { helpers: helperProcesses,
        page: await page.evaluate(() => (globalThis as any).__playsrcInputDiagnosis) } : undefined,
      timeline,
      hostSamples,
      allocations: [...allocations].sort((left, right) => right[1] - left[1]).slice(0, 30)
        .map(([identity, bytes]) => ({ identity, bytes })),
      heapSnapshot,
    }
    const serialized = `${JSON.stringify(report, null, 2)}\n`
    const label = process.env.PROFILE_MEMORY_LABEL ?? "current"
    await writeFile(path.join(output, `${label}.json`), serialized)
    await writeFile(path.join(output, "report.json"), serialized)
    await testInfo.attach("headed-three-map-memory", { body: Buffer.from(serialized), contentType: "application/json" })
  } catch (error) {
    const failure = await page.evaluate(() => {
      const main = document.querySelector<HTMLElement>("main")
      const command = document.querySelector<HTMLInputElement>("[aria-label='Console command']")
      const state = (globalThis as any).__playsrcMemoryProfile
      return {
        application: { ...main?.dataset },
        console: document.querySelector<HTMLElement>("[aria-label='Console output']")?.textContent,
        command: { value: command?.value, focused: document.activeElement === command, documentFocused: document.hasFocus() },
        worker: state?.worker.slice(-20),
        owners: (globalThis as any).__playsrcProfile?.mapResidency,
      }
    }).catch(() => null)
    const inputDiagnosis = process.env.PROFILE_MEMORY_INPUT_DIAGNOSTIC === "1" ? { helpers: helperProcesses,
      page: await page.evaluate(() => (globalThis as any).__playsrcInputDiagnosis).catch(() => null) } : undefined
    await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}-failure.json`), `${JSON.stringify({ error: String(error), failure, timeline, hostSamples, samplerFailure, nativeAdmission: nativeReader?.records, inputDiagnosis }, null, 2)}\n`)
    console.error(`PLAYSRC_MAP_MEMORY_FAILURE ${JSON.stringify(failure && { phase: failure.application.phase, generation: failure.application.generation, detail: failure.application.detail, console: failure.console, command: failure.command })}`)
    throw error
  } finally {
    clearInterval(sampler)
    windowsSampler?.close()
    await writeFile(path.join(output, `${process.env.PROFILE_MEMORY_LABEL ?? "current"}-helpers.json`), JSON.stringify({ helpers: helperProcesses, hostSamples }, null, 2))
    await nativeReader?.close()
    await browserCdp.detach().catch(() => {})
  }
})
