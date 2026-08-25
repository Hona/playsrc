import { spawn } from "node:child_process"
import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { inflateSync } from "node:zlib"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"

type RpcRecord = { kind: string; started: number; finished?: number; bytes?: number; workerTimings?: Record<string, number> }
type PresentationRecord = { at: number; detail: string; performance: string | undefined }
type BrowserProcessInfo={type:string;id:number;cpuTime:number}
type BrowserProcessSnapshot={at:number;processes:readonly BrowserProcessInfo[]}

const percentile = (values: number[], fraction: number): number =>
  values.length === 0 ? 0 : values[Math.min(values.length - 1, Math.floor(values.length * fraction))]!

type DecodedScreenshot = Readonly<{ width: number; height: number; channels: number; pixels: Uint8Array }>

function decodeScreenshot(bytes: Buffer): DecodedScreenshot {
  if (!bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
    throw new Error("headed browser screen sample is not PNG")
  }
  let width = 0
  let height = 0
  let colorType = -1
  const chunks: Buffer[] = []
  for (let offset = 8; offset + 12 <= bytes.length;) {
    const length = bytes.readUInt32BE(offset)
    if (length > bytes.length - offset - 12) throw new Error("headed browser PNG chunk is truncated")
    const kind = bytes.subarray(offset + 4, offset + 8).toString("ascii")
    const value = bytes.subarray(offset + 8, offset + 8 + length)
    if (kind === "IHDR") {
      if (value.length !== 13 || value[8] !== 8 || value[10] !== 0 || value[11] !== 0 || value[12] !== 0) {
        throw new Error("headed browser PNG format is unsupported")
      }
      width = value.readUInt32BE(0)
      height = value.readUInt32BE(4)
      colorType = value[9]!
    } else if (kind === "IDAT") chunks.push(value)
    offset += length + 12
  }
  if (width < 1 || height < 1 || ![2, 6].includes(colorType)) {
    throw new Error("headed browser PNG dimensions or color type differ")
  }
  const channels = colorType === 2 ? 3 : 4
  const stride = width * channels
  const values = inflateSync(Buffer.concat(chunks))
  if (values.length !== (stride + 1) * height) throw new Error("headed browser PNG scanline length differs")
  const pixels = new Uint8Array(stride * height)
  for (let row = 0; row < height; row += 1) {
    const source = row * (stride + 1)
    const filter = values[source]!
    if (filter > 4) throw new Error("headed browser PNG scanline filter is invalid")
    for (let column = 0; column < stride; column += 1) {
      const destination = row * stride + column
      const left = column >= channels ? pixels[destination - channels]! : 0
      const above = row > 0 ? pixels[destination - stride]! : 0
      const upperLeft = row > 0 && column >= channels ? pixels[destination - stride - channels]! : 0
      let predictor = 0
      if (filter === 1) predictor = left
      else if (filter === 2) predictor = above
      else if (filter === 3) predictor = Math.floor((left + above) / 2)
      else if (filter === 4) {
        const estimate = left + above - upperLeft
        const leftDistance = Math.abs(estimate - left)
        const aboveDistance = Math.abs(estimate - above)
        const upperLeftDistance = Math.abs(estimate - upperLeft)
        predictor = leftDistance <= aboveDistance && leftDistance <= upperLeftDistance
          ? left
          : aboveDistance <= upperLeftDistance ? above : upperLeft
      }
      pixels[destination] = (values[source + column + 1]! + predictor) & 255
    }
  }
  return Object.freeze({ width, height, channels, pixels })
}

function screenshotPixel(bytes: Buffer): readonly [number, number, number] {
  const image = decodeScreenshot(bytes)
  if (image.width !== 1 || image.height !== 1) throw new Error("crosshair screen sample dimensions differ")
  return Object.freeze([image.pixels[0]!, image.pixels[1]!, image.pixels[2]!])
}

function screenshotRegionMotion(
  before: DecodedScreenshot,
  after: DecodedScreenshot,
  region: Readonly<{ x: number; y: number; width: number; height: number }>,
): Readonly<{ changedPixels: number; totalPixels: number; changedFraction: number; maximumChannelDelta: number }> {
  if (
    before.width !== after.width || before.height !== after.height || before.channels !== after.channels
    || region.x < 0 || region.y < 0 || region.width < 1 || region.height < 1
    || region.x + region.width > before.width || region.y + region.height > before.height
  ) throw new Error("headed browser screen comparison region is invalid")
  let changedPixels = 0
  let maximumChannelDelta = 0
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      const offset = (y * before.width + x) * before.channels
      const channelDelta = Math.max(
        Math.abs(before.pixels[offset]! - after.pixels[offset]!),
        Math.abs(before.pixels[offset + 1]! - after.pixels[offset + 1]!),
        Math.abs(before.pixels[offset + 2]! - after.pixels[offset + 2]!),
      )
      maximumChannelDelta = Math.max(maximumChannelDelta, channelDelta)
      if (channelDelta > 2) changedPixels += 1
    }
  }
  const totalPixels = region.width * region.height
  return Object.freeze({ changedPixels, totalPixels, changedFraction: changedPixels / totalPixels, maximumChannelDelta })
}

function parseCsvLine(line:string):string[]{
  const values:string[]=[]
  for(const match of line.matchAll(/"((?:[^"]|"")*)"(?:,|$)/gu))values.push(match[1]!.replaceAll('""','"'))
  return values
}

function summarizeBrowserMetrics(output:string,processSnapshots:readonly BrowserProcessSnapshot[]){
  const lines=output.split(/\r?\n/u).filter(line=>line.startsWith('"'))
  if(lines.length<2)return {sampleCount:0,timeline:[],error:"typeperf produced no samples"}
  const headers=parseCsvLine(lines[0]!)
  const rows=lines.slice(1).map(parseCsvLine).filter(row=>row.length===headers.length)
  const roleByPid=new Map<number,string>()
  for(const snapshot of processSnapshots)for(const process of snapshot.processes)roleByPid.set(process.id,process.type)
  const processIds=new Set(roleByPid.keys())
  const logicalProcessors=os.cpus().length
  const timeline=rows.map(row=>{
    const instancePids=new Map<string,number>()
    headers.forEach((header,index)=>{const match=header.match(/\\Process\((msedge(?:#\d+)?)\)\\ID Process$/u);if(match)instancePids.set(match[1]!,Number(row[index]))})
    const processes=[...instancePids.entries()].flatMap(([instance,processId])=>{
      if(!processIds.has(processId))return []
      const read=(counter:string)=>{const index=headers.findIndex(header=>header.endsWith(`\\Process(${instance})\\${counter}`));return index<0?0:Number(row[index])||0}
      return [{processId,role:roleByPid.get(processId)??"unknown",cpuCorePercent:read("% Processor Time"),workingSetBytes:read("Working Set - Private"),privateBytes:read("Private Bytes")}]
    })
    const engines=headers.flatMap((header,index)=>{const match=header.match(/\\GPU Engine\(pid_(\d+)_luid_(.+?)_phys_(\d+)_eng_\d+_engtype_(.+)\)\\Utilization Percentage$/u);if(!match||!processIds.has(Number(match[1])))return [];return [{processId:Number(match[1]),luid:match[2]!,physicalAdapter:Number(match[3]),engine:match[4]!,percent:Number(row[index])||0}]})
    const gpuMemory=headers.flatMap((header,index)=>{const match=header.match(/\\GPU Process Memory\(pid_(\d+)_luid_(.+?)_phys_(\d+)\)\\(Dedicated|Shared) Usage$/u);if(!match||!processIds.has(Number(match[1])))return [];return [{processId:Number(match[1]),luid:match[2]!,physicalAdapter:Number(match[3]),kind:match[4]!.toLowerCase(),bytes:Number(row[index])||0}]})
    return {at:row[0],cpuCorePercent:processes.reduce((sum,process)=>sum+process.cpuCorePercent,0),workingSetBytes:processes.reduce((sum,process)=>sum+process.workingSetBytes,0),privateBytes:processes.reduce((sum,process)=>sum+process.privateBytes,0),processes,engines,gpuMemory}
  })
  const processRoles=new Map<string,{maximumCpuCorePercent:number;maximumWorkingSetBytes:number;maximumPrivateBytes:number}>()
  const gpuEngines=new Map<string,number>()
  const gpuMemory=new Map<string,{maximumDedicatedBytes:number;maximumSharedBytes:number}>()
  for(const sample of timeline){
    for(const process of sample.processes){
      const value=processRoles.get(process.role)??{maximumCpuCorePercent:0,maximumWorkingSetBytes:0,maximumPrivateBytes:0}
      value.maximumWorkingSetBytes=Math.max(value.maximumWorkingSetBytes,process.workingSetBytes)
      value.maximumPrivateBytes=Math.max(value.maximumPrivateBytes,process.privateBytes)
      processRoles.set(process.role,value)
    }
    for(const engine of sample.engines){
      const key=`${engine.luid}:${engine.physicalAdapter}:${engine.engine}`
      gpuEngines.set(key,Math.max(gpuEngines.get(key)??0,engine.percent))
    }
    for(const memory of sample.gpuMemory){
      const key=`${memory.luid}:${memory.physicalAdapter}`
      const value=gpuMemory.get(key)??{maximumDedicatedBytes:0,maximumSharedBytes:0}
      if(memory.kind==="dedicated")value.maximumDedicatedBytes=Math.max(value.maximumDedicatedBytes,memory.bytes)
      else value.maximumSharedBytes=Math.max(value.maximumSharedBytes,memory.bytes)
      gpuMemory.set(key,value)
    }
  }
  const cpuTimeline=processSnapshots.slice(1).map((snapshot,index)=>{
    const previous=processSnapshots[index]!,elapsedSeconds=Math.max(0.001,(snapshot.at-previous.at)/1000),prior=new Map(previous.processes.map(process=>[process.id,process.cpuTime]))
    const roles:Record<string,number>={}
    for(const process of snapshot.processes){const delta=Math.max(0,process.cpuTime-(prior.get(process.id)??process.cpuTime))*100/elapsedSeconds;roles[process.type]=(roles[process.type]??0)+delta}
    for(const [role,value] of Object.entries(roles)){const peak=processRoles.get(role)??{maximumCpuCorePercent:0,maximumWorkingSetBytes:0,maximumPrivateBytes:0};peak.maximumCpuCorePercent=Math.max(peak.maximumCpuCorePercent,value);processRoles.set(role,peak)}
    return {atUnixMilliseconds:snapshot.at,cpuCorePercent:Object.values(roles).reduce((sum,value)=>sum+value,0),roles}
  })
  return {
    logicalProcessors,
    sampleCount:timeline.length,
    maximumCpuCorePercent:Math.max(0,...cpuTimeline.map(sample=>sample.cpuCorePercent)),
    maximumCpuMachinePercent:Math.max(0,...cpuTimeline.map(sample=>sample.cpuCorePercent/logicalProcessors)),
    maximumWorkingSetBytes:Math.max(0,...timeline.map(sample=>sample.workingSetBytes)),
    maximumPrivateBytes:Math.max(0,...timeline.map(sample=>sample.privateBytes)),
    processRoles:Object.fromEntries([...processRoles.entries()].sort(([left],[right])=>left.localeCompare(right))),
    gpuEngines:Object.fromEntries([...gpuEngines.entries()].sort(([left],[right])=>left.localeCompare(right))),
    gpuMemory:Object.fromEntries([...gpuMemory.entries()].sort(([left],[right])=>left.localeCompare(right))),
    cpuTimeline,
    timeline,
  }
}

test("profile startup and input latency", async ({ page,browser },testInfo) => {
  const cdp=await browser.newBrowserCDPSession()
  const pageCdp=await page.context().newCDPSession(page)
  const processInfo=await cdp.send("SystemInfo.getProcessInfo") as {processInfo:readonly BrowserProcessInfo[]}
  const systemInfo=await cdp.send("SystemInfo.getInfo") as {gpu?:{devices?:unknown;featureStatus?:unknown;auxAttributes?:Record<string,unknown>}}
  const gpuInfo={devices:systemInfo.gpu?.devices??[],featureStatus:systemInfo.gpu?.featureStatus??{},adapter:systemInfo.gpu?.auxAttributes?{displayType:systemInfo.gpu.auxAttributes.displayType,glImplementationParts:systemInfo.gpu.auxAttributes.glImplementationParts,glRenderer:systemInfo.gpu.auxAttributes.glRenderer,glVendor:systemInfo.gpu.auxAttributes.glVendor,supportsDx12:systemInfo.gpu.auxAttributes.supportsDx12,supportsVulkan:systemInfo.gpu.auxAttributes.supportsVulkan}:null}
  const processSnapshots:BrowserProcessSnapshot[]=[{at:Date.now(),processes:processInfo.processInfo}]
  let processSampleBusy=false
  const processSampleTimer=setInterval(async()=>{if(processSampleBusy)return;processSampleBusy=true;try{const value=await cdp.send("SystemInfo.getProcessInfo") as {processInfo:readonly BrowserProcessInfo[]};processSnapshots.push({at:Date.now(),processes:value.processInfo})}finally{processSampleBusy=false}},1000)
  const systemSampler=process.platform==="win32"?spawn("typeperf",[
    "\\Process(msedge*)\\ID Process","\\Process(msedge*)\\% Processor Time","\\Process(msedge*)\\Working Set - Private","\\Process(msedge*)\\Private Bytes",
    "\\GPU Engine(*)\\Utilization Percentage","\\GPU Process Memory(*)\\Dedicated Usage","\\GPU Process Memory(*)\\Shared Usage","-si","1",
  ],{stdio:["ignore","pipe","pipe"]}):null
  let systemSamplerOutput="",systemSamplerError=""
  systemSampler?.stdout.setEncoding("utf8").on("data",chunk=>{systemSamplerOutput+=chunk})
  systemSampler?.stderr.setEncoding("utf8").on("data",chunk=>{systemSamplerError+=chunk})
  const systemSamplerExit=systemSampler?new Promise<void>(resolve=>systemSampler.once("exit",()=>resolve())):Promise.resolve()
  const rafHz = Number(process.env.PROFILE_RAF_HZ ?? 0)
  const scenarioMode = process.env.PROFILE_SCENARIOS ?? (process.env.npm_lifecycle_event === "profile:gameplay" ? "1" : "")
  const mapOnly = process.env.PROFILE_MAP_ONLY === "1" || process.env.npm_lifecycle_event === "profile:map-load"
  const runScenarios = scenarioMode !== ""
  const shouldRunScenario = (name: string) => scenarioMode === "1" || scenarioMode === name
  await page.addInitScript(({ rafHz }) => {
    let pointerLockElement: Element | null = null
    Object.defineProperty(document, "pointerLockElement", {
      configurable: true,
      get: () => pointerLockElement,
    })
    Object.defineProperty(Element.prototype, "requestPointerLock", {
      configurable: true,
      value(this: Element): Promise<void> {
        pointerLockElement = this
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    Object.defineProperty(document, "exitPointerLock", {
      configurable: true,
      value(): Promise<void> {
        pointerLockElement = null
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    if (rafHz > 0) {
      const interval = 1_000 / rafHz
      Object.defineProperty(window, "requestAnimationFrame", {
        configurable: true,
        value: (callback: FrameRequestCallback) => window.setTimeout(() => callback(performance.now()), interval),
      })
      Object.defineProperty(window, "cancelAnimationFrame", {
        configurable: true,
        value: (handle: number) => clearTimeout(handle),
      })
    }
    const state = {
      created: performance.now(),
      syntheticPointerLock: true,
      rpcs: [] as RpcRecord[],
      phases: [] as { at: number; phase: string; detail: string; gameUi: string; frames: number }[],
      longTasks: [] as { at: number; duration: number }[],
      rafGaps: [] as { at: number; duration: number }[],
      presentations: [] as PresentationRecord[],
      frames: 0,
    }
    ;(window as any).__playsrcProfile = state

    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      readonly records = new Map<number, RpcRecord>()

      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event: MessageEvent) => {
          const id = event.data?.id
          if (Number.isSafeInteger(id)) {
            const record = this.records.get(id)
            if (record) {
              record.finished = performance.now()
              if (event.data?.timings) record.workerTimings = event.data.timings
            }
          }
        })
      }

      override postMessage(message: any, transferOrOptions?: Transferable[] | StructuredSerializeOptions): void {
        if (Number.isSafeInteger(message?.id) && typeof message?.kind === "string") {
          const record: RpcRecord = {
            kind: message.kind,
            started: performance.now(),
            bytes: message.command?.byteLength ?? message.batch?.byteLength ?? message.bsp?.byteLength,
          }
          this.records.set(message.id, record)
          state.rpcs.push(record)
        }
        super.postMessage(message, transferOrOptions as any)
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })

    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) state.longTasks.push({ at: entry.startTime, duration: entry.duration })
      }).observe({ entryTypes: ["longtask"] })
    } catch {}

    let previous = performance.now()
    const frame = (now: number) => {
      const gap = now - previous
      if (gap > 20) state.rafGaps.push({ at: now, duration: gap })
      previous = now
      state.frames += 1
      requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)

    addEventListener("DOMContentLoaded", () => {
      const main = document.querySelector("main")
      if (!main) return
      let previous = ""
      const capture = () => {
        const dataset = (main as HTMLElement).dataset
        const current = `${dataset.phase ?? ""}\0${dataset.detail ?? ""}\0${dataset.gameui ?? ""}`
        if (current === previous) return
        previous = current
        state.phases.push({
          at: performance.now(),
          phase: dataset.phase ?? "",
          detail: dataset.detail ?? "",
          gameUi: dataset.gameui ?? "",
          frames: state.frames,
        })
      }
      const capturePresentation = () => {
        const detail = (main as HTMLElement).dataset.performanceDetail
        if (detail) state.presentations.push({
          at: performance.now(),
          detail,
          performance: (main as HTMLElement).dataset.performance,
        })
      }
      capture()
      new MutationObserver((records) => {
        if (records.some((record) => record.attributeName === "data-phase" || record.attributeName === "data-detail" || record.attributeName === "data-gameui")) capture()
        if (records.some((record) => record.attributeName === "data-performance-detail")) capturePresentation()
      }).observe(main, { attributes: true, attributeFilter: ["data-phase", "data-detail", "data-gameui", "data-performance-detail"] })
    })
  }, { rafHz })

  const wallStarted = Date.now()
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => {
    const phase = document.querySelector("main")?.getAttribute("data-phase")
    return phase === "MainMenu" || phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 50 })
  const mainMenuMilliseconds = await page.evaluate(() => performance.now())
  const configurationResponse = await page.request.get("/playsrc-config.json")
  expect(configurationResponse.status()).toBe(200)
  const configuration = await configurationResponse.json() as { defaultTarget: string; targets: readonly { target: string }[] }
  if (!configuration.targets.some((candidate) => candidate.target === configuration.defaultTarget)) {
    throw new Error("configured default map is absent from the current target catalog")
  }
  const target = configuration.defaultTarget
  const storageSnapshot = () => page.evaluate(async () => {
    const estimate = await navigator.storage.estimate()
    return {
      usage: estimate.usage ?? null,
      quota: estimate.quota ?? null,
      persisted: await navigator.storage.persisted(),
      databases: typeof indexedDB.databases === "function"
        ? (await indexedDB.databases()).map((database) => ({ name: database.name ?? null, version: database.version ?? null }))
        : [],
    }
  })
  const storageBeforeMap = await storageSnapshot()
  let mapSubmittedMilliseconds: number | undefined
  let gameplayReadyMilliseconds: number | undefined
  let repeatedMapSubmittedMilliseconds: number | undefined
  let repeatedGameplayReadyMilliseconds: number | undefined
  let pageReloadMapSubmittedMilliseconds: number | undefined
  let pageReloadGameplayReadyMilliseconds: number | undefined
  let firstLoadPerformance: unknown = null
  let repeatedLoadPerformance: unknown = null
  let storageAfterFirstMap: Awaited<ReturnType<typeof storageSnapshot>> | null = null
  let storageAfterRepeatedMap: Awaited<ReturnType<typeof storageSnapshot>> | null = null
  let pageReloadLoadPerformance:unknown=null
  let storageAfterPageReload:Awaited<ReturnType<typeof storageSnapshot>>|null=null
  if (await page.locator("main").getAttribute("data-phase") === "MainMenu") {
    await page.keyboard.press("Backquote")
    const consoleEntry = page.locator("[aria-label='Console command']")
    await expect(consoleEntry).toBeVisible()
    await consoleEntry.fill(`map ${target}`)
    mapSubmittedMilliseconds = await page.evaluate(() => performance.now())
    await page.keyboard.press("Enter")
    await page.waitForFunction(() => {
      const main = document.querySelector<HTMLElement>("main")
      return (main?.dataset.phase === "Ready" && main.dataset.gameui === "in-game") || main?.dataset.phase === "Failed"
    }, undefined, { timeout: 600_000, polling: 50 })
    if (await page.locator("main").getAttribute("data-phase") === "Ready") {
      gameplayReadyMilliseconds = await page.evaluate(() => performance.now())
      firstLoadPerformance = JSON.parse((await page.locator("main").getAttribute("data-load-performance")) ?? "null")
      storageAfterFirstMap = await storageSnapshot()
      await page.keyboard.press("Backquote")
    }
  }
  if (mapOnly && gameplayReadyMilliseconds !== undefined) {
    await page.keyboard.press("Backquote")
    const consoleEntry = page.locator("[aria-label='Console command']")
    await expect(consoleEntry).toBeVisible()
    await consoleEntry.fill(`map ${target}`)
    repeatedMapSubmittedMilliseconds = await page.evaluate(() => performance.now())
    await page.keyboard.press("Enter")
    await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.phase !== "Ready", undefined, { timeout: 30_000, polling: 10 })
    await page.waitForFunction(() => {
      const phase = document.querySelector<HTMLElement>("main")?.dataset.phase
      return phase === "Ready" || phase === "Failed"
    }, undefined, { timeout: 600_000, polling: 50 })
    if (await page.locator("main").getAttribute("data-phase") === "Ready") {
      repeatedGameplayReadyMilliseconds = await page.evaluate(() => performance.now())
      expect(await page.locator("main").getAttribute("data-detail")).toBe(`Playing ${target}`)
      repeatedLoadPerformance = JSON.parse((await page.locator("main").getAttribute("data-load-performance")) ?? "null")
      storageAfterRepeatedMap = await storageSnapshot()
      await page.keyboard.press("Backquote")
    }
  }
  if(mapOnly&&repeatedGameplayReadyMilliseconds!==undefined){
    await page.reload({waitUntil:"load",timeout:30_000})
    await page.waitForFunction(()=>["MainMenu","Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase??""),undefined,{timeout:180_000,polling:50})
    if(await page.locator("main").getAttribute("data-phase")==="MainMenu"){
      await page.keyboard.press("Backquote");const consoleEntry=page.locator("[aria-label='Console command']");await expect(consoleEntry).toBeVisible();await consoleEntry.fill(`map ${target}`);pageReloadMapSubmittedMilliseconds=await page.evaluate(()=>performance.now());await page.keyboard.press("Enter")
      await page.waitForFunction(()=>{const main=document.querySelector<HTMLElement>("main");return(main?.dataset.phase==="Ready"&&main.dataset.gameui==="in-game")||main?.dataset.phase==="Failed"},undefined,{timeout:600_000,polling:50})
      if(await page.locator("main").getAttribute("data-phase")==="Ready"){expect(await page.locator("main").getAttribute("data-detail")).toBe("Click the field to capture the mouse");pageReloadGameplayReadyMilliseconds=await page.evaluate(()=>performance.now());pageReloadLoadPerformance=JSON.parse((await page.locator("main").getAttribute("data-load-performance"))??"null");storageAfterPageReload=await storageSnapshot();await page.keyboard.press("Backquote")}
    }
  }
  const startupMilliseconds = Date.now() - wallStarted

  const initial = await page.locator("main").evaluate((main) => ({ ...((main as HTMLElement).dataset) }))
  const input: Record<string, unknown> = {}
  const verifySimultaneousBindings = async () => {
    const main = page.locator("main")
    await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keydown", { code: "ShiftLeft", key: "Shift", shiftKey: true, bubbles: true }))
      dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", shiftKey: true, bubbles: true }))
    })
    await expect.poll(async () => Number(await main.getAttribute("data-wish-speed"))).toBeGreaterThan(0)
    await expect.poll(async () => Number(await main.getAttribute("data-crouch-fraction"))).toBeGreaterThan(0)
    await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keydown", { code: "KeyA", key: "a", shiftKey: true, bubbles: true }))
      dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", shiftKey: true, bubbles: true }))
    })
    await expect.poll(async () => Number(await main.getAttribute("data-wish-speed"))).toBeGreaterThan(0)
    await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keyup", { code: "KeyA", key: "a", shiftKey: true, bubbles: true }))
    })
    await expect.poll(async () => Number(await main.getAttribute("data-wish-speed"))).toBe(0)
    await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keyup", { code: "ShiftLeft", key: "Shift", bubbles: true }))
    })
    await expect.poll(async () => Number(await main.getAttribute("data-crouch-fraction"))).toBe(0)
  }
  if (!mapOnly && initial.phase === "Ready") {
    await page.waitForTimeout(5_000)
    const settledPhase = await page.locator("main").getAttribute("data-phase")
    if (settledPhase !== "Ready") {
      input.skipped = `phase became ${settledPhase} during settle`
    } else {
    const keyDownAt = await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
      return performance.now()
    })
    try {
      await page.waitForFunction(() => Number(document.querySelector("main")?.getAttribute("data-wish-speed")) > 0, undefined, {
        timeout: 30_000,
        polling: 10,
      })
      input.keyDownMilliseconds = await page.evaluate((started) => performance.now() - started, keyDownAt)
    } catch {
      input.keyDownMilliseconds = ">30000"
    }

    const keyUpAt = await page.evaluate(() => {
      dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
      return performance.now()
    })
    try {
      await page.waitForFunction(() => Number(document.querySelector("main")?.getAttribute("data-wish-speed")) === 0, undefined, {
        timeout: 30_000,
        polling: 10,
      })
      input.keyUpMilliseconds = await page.evaluate((started) => performance.now() - started, keyUpAt)
    } catch {
      input.keyUpMilliseconds = ">30000"
    }

    const fireEvents = Number(await page.locator("main").getAttribute("data-fire-events"))
    try {
      await page.locator(".world-canvas").click()
      await page.waitForFunction(() => document.pointerLockElement?.classList.contains("world-canvas"), undefined, {
        timeout: 5_000,
      })
      const look = await page.evaluate(async () => {
        const main = document.querySelector("main") as HTMLElement | null
        if (!main) throw new Error("application root is unavailable")
        const canvas = document.querySelector(".world-canvas") as HTMLCanvasElement | null
        if (!canvas) throw new Error("world canvas is unavailable")
        const normalize = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
        const records: { frame: number; prepared: number; view: number; yaw: number; pitch: number; viewmodelYaw: number; viewmodelPitch: number; localYaw: number; localPitch: number; mouse: number; snap: number }[] = []
        const observer = new MutationObserver(() => records.push({
          frame: Number(canvas.dataset.displayFrame),
          prepared: Number(canvas.dataset.displayPreparedRevision),
          view: Number(canvas.dataset.displayViewRevision),
          yaw: Number(canvas.dataset.displayCameraYaw),
          pitch: Number(canvas.dataset.displayCameraPitch),
          viewmodelYaw: Number(canvas.dataset.displayViewmodelYaw),
          viewmodelPitch: Number(canvas.dataset.displayViewmodelPitch),
          localYaw: Number(canvas.dataset.displayViewmodelLocalYaw),
          localPitch: Number(canvas.dataset.displayViewmodelLocalPitch),
          mouse: Number(canvas.dataset.displayMouseRevision),
          snap: Number(canvas.dataset.displaySnapRevision),
        }))
        observer.observe(canvas, { attributes: true, attributeFilter: ["data-display-frame"] })
        const started = {
          frame: Number(canvas.dataset.displayFrame),
          prepared: Number(canvas.dataset.displayPreparedRevision),
          view: Number(canvas.dataset.displayViewRevision),
          yaw: Number(canvas.dataset.displayCameraYaw),
          pitch: Number(canvas.dataset.displayCameraPitch),
          viewmodelYaw: Number(canvas.dataset.displayViewmodelYaw),
          viewmodelPitch: Number(canvas.dataset.displayViewmodelPitch),
          localYaw: Number(canvas.dataset.displayViewmodelLocalYaw),
          localPitch: Number(canvas.dataset.displayViewmodelLocalPitch),
          mouse: Number(canvas.dataset.displayMouseRevision),
          snap: Number(canvas.dataset.displaySnapRevision),
        }
        let events = 0
        const interval = setInterval(() => {
          const event = new MouseEvent("mousemove", { bubbles: true })
          Object.defineProperties(event, { movementX: { value: 2 }, movementY: { value: events % 2 === 0 ? 1 : -1 } })
          dispatchEvent(event)
          events += 1
        }, 4)
        await new Promise((resolve) => setTimeout(resolve, 600))
        clearInterval(interval)
        await new Promise((resolve) => setTimeout(resolve, 200))
        observer.disconnect()
        const finished = {
          frame: Number(canvas.dataset.displayFrame),
          prepared: Number(canvas.dataset.displayPreparedRevision),
          view: Number(canvas.dataset.displayViewRevision),
          yaw: Number(canvas.dataset.displayCameraYaw),
          mouse: Number(canvas.dataset.displayMouseRevision),
          snap: Number(canvas.dataset.displaySnapRevision),
        }
        return {
          pointerMovement: main.dataset.pointerMovement,
          events,
          displayFrames: finished.frame - started.frame,
          preparedRevisions: new Set(records.map((record) => record.prepared)).size,
          repeatedPreparedFrames: records.filter((record, index) => index > 0 && record.prepared === records[index - 1]!.prepared).length,
          viewRevisions: finished.view - started.view,
          mouseRevisions: finished.mouse - started.mouse,
          snapRevisions: finished.snap - started.snap,
          yawDegrees: finished.yaw - started.yaw,
          maximumViewmodelYawError: Math.max(0, ...records.map((record) => Math.abs(normalize(record.viewmodelYaw - record.yaw - record.localYaw)))),
          maximumViewmodelPitchError: Math.max(0, ...records.map((record) => Math.abs(record.viewmodelPitch - record.pitch - record.localPitch))),
          viewmodelAlignmentSamples: records.slice(0, 256),
          samples: records.length,
        }
      })
      Object.assign(input, Object.fromEntries(Object.entries(look).map(([key, value]) => [`look${key[0]!.toUpperCase()}${key.slice(1)}`, value])))
      const heapBefore = await pageCdp.send("Runtime.getHeapUsage")
      await pageCdp.send("HeapProfiler.startSampling", {
        samplingInterval: 32_768,
        includeObjectsCollectedByMajorGC: true,
        includeObjectsCollectedByMinorGC: true,
      })
      const motion = await page.evaluate(async () => {
        const root = document.querySelector("main") as HTMLElement | null
        const canvas = document.querySelector(".world-canvas") as HTMLCanvasElement | null
        if (!root || !canvas) throw new Error("movement presentation roots are unavailable")
        const profile = (window as any).__playsrcProfile as { rpcs: RpcRecord[] }
        const records: { at: number; tick: number; frame: number; position: [number, number, number] }[] = []
        const capture = () => {
          const position = (canvas.dataset.displayCameraPosition ?? "").split(",").map(Number)
          if (position.length !== 3 || !position.every(Number.isFinite)) return
          records.push({
            at: performance.now(),
            tick: Number(root.dataset.snapshotTick ?? 0),
            frame: Number(canvas.dataset.displayFrame ?? 0),
            position: position as [number, number, number],
          })
        }
        await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
        const observer = new MutationObserver(capture)
        observer.observe(canvas, { attributes: true, attributeFilter: ["data-display-frame"] })
        const before = profile.rpcs.length
        capture()
        dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
        await new Promise((resolve) => setTimeout(resolve, 700))
        dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
        await new Promise((resolve) => setTimeout(resolve, 50))
        observer.disconnect()
        const movements = records.slice(1).map((sample, index) => Math.hypot(
          sample.position[0] - records[index]!.position[0],
          sample.position[1] - records[index]!.position[1],
          sample.position[2] - records[index]!.position[2],
        ))
        const positive = movements.filter((value) => value > 0).sort((left, right) => left - right)
        const gaps = records.slice(1).map((sample, index) => sample.at - records[index]!.at).sort((left, right) => left - right)
        const visibility = profile.rpcs.slice(before).filter((record) => record.kind === "visibility" && record.finished !== undefined)
        const durations = visibility.map((record) => record.finished! - record.started).sort((left, right) => left - right)
        const iterations = 100_000
        let accumulator = 0
        const adapterStarted = performance.now()
        for (let index = 0; index < iterations; index += 1) {
          const fraction = (index % 15) / 15
          accumulator += (1 + (4 - 1) * fraction) + (2 + (5 - 2) * fraction) + (3 + (6 - 3) * fraction)
        }
        const adapterMilliseconds = performance.now() - adapterStarted
        return {
          samples: records.slice(0, 256),
          displayFrames: records.length - 1,
          movingFrames: positive.length,
          duplicatePositionFrames: movements.length - positive.length,
          authoritativeTicks: (records.at(-1)?.tick ?? 0) - (records[0]?.tick ?? 0),
          medianPositionStep: positive[Math.floor(positive.length / 2)] ?? 0,
          maximumPositionStep: positive.at(-1) ?? 0,
          frameGapP95Milliseconds: gaps[Math.min(gaps.length - 1, Math.floor(gaps.length * 0.95))] ?? 0,
          standaloneVisibilityCalls: visibility.length,
          standaloneVisibilityP50Milliseconds: durations[Math.floor(durations.length / 2)] ?? 0,
          adapterIterations: iterations,
          adapterTotalMilliseconds: adapterMilliseconds,
          adapterMeanNanoseconds: adapterMilliseconds * 1_000_000 / iterations,
          adapterChecksum: accumulator,
        }
      })
      const allocationProfile = await pageCdp.send("HeapProfiler.stopSampling")
      const heapAfter = await pageCdp.send("Runtime.getHeapUsage")
      input.movementContinuity = {
        ...motion,
        allocationSamplingIntervalBytes: 32_768,
        allocationSamples: allocationProfile.profile.samples.length,
        sampledAllocationBytes: allocationProfile.profile.samples.reduce((total, sample) => total + sample.size, 0),
        heapBeforeBytes: heapBefore.usedSize,
        heapAfterBytes: heapAfter.usedSize,
        heapBeforeBackingStorageBytes: heapBefore.backingStorageSize,
        heapAfterBackingStorageBytes: heapAfter.backingStorageSize,
      }
      const fireAt = await page.evaluate(() => performance.now())
      await page.mouse.down({ button: "left" })
      await page.waitForFunction((baseline) => Number(document.querySelector("main")?.getAttribute("data-fire-events")) > baseline, fireEvents, {
        timeout: 30_000,
        polling: 10,
      })
      input.fireMilliseconds = await page.evaluate((started) => performance.now() - started, fireAt)
      await page.mouse.up({ button: "left" })
      await verifySimultaneousBindings()
      if (!runScenarios) await page.keyboard.press("Escape")
    } catch {
      input.fireMilliseconds = ">30000"
      await page.mouse.up({ button: "left" }).catch(() => {})
    }
    }
  }

  const steadyState: {
    elapsedMilliseconds: number
    phase: string | undefined
    performance: string | undefined
    tick: string | undefined
    frames: number
    rpcStarted: Record<string, number>
    rpcCompleted: Record<string, number>
  }[] = []
  const captureRuntime = () => page.locator("main").evaluate((main) => {
    const profile = (window as any).__playsrcProfile as { frames: number; rpcs: RpcRecord[] }
    const rpcCompleted: Record<string, number> = {}
    for (const rpc of profile.rpcs) {
      if (rpc.finished !== undefined) rpcCompleted[rpc.kind] = (rpcCompleted[rpc.kind] ?? 0) + 1
    }
    const dataset = (main as HTMLElement).dataset
    return {
      at: performance.now(),
      frames: profile.frames,
      rpcCompleted,
      tick: Number(dataset.snapshotTick ?? 0),
      cameraPosition: dataset.cameraPosition,
      wishSpeed: Number(dataset.wishSpeed ?? 0),
      grounded: dataset.grounded,
      sweepQueries: Number(dataset.sweepQueries ?? 0),
      pointQueries: Number(dataset.pointQueries ?? 0),
      movementContacts: Number(dataset.movementContacts ?? 0),
      movementEvents: Number(dataset.movementEvents ?? 0),
      fireEvents: Number(dataset.fireEvents ?? 0),
      particleItems: Number(dataset.particleItems ?? 0),
      performance: dataset.performance,
      performanceDetail: dataset.performanceDetail,
      phase: dataset.phase,
    }
  })
  const steadyStarted = Date.now()
  for (let second = 0; second < (mapOnly ? 0 : 10); second += 1) {
    await page.waitForTimeout(1_000)
    const sample = await page.locator("main").evaluate((main) => {
      const profile = (window as any).__playsrcProfile as { frames: number; rpcs: RpcRecord[] }
      const rpcStarted: Record<string, number> = {}
      const rpcCompleted: Record<string, number> = {}
      for (const rpc of profile.rpcs) {
        rpcStarted[rpc.kind] = (rpcStarted[rpc.kind] ?? 0) + 1
        if (rpc.finished !== undefined) rpcCompleted[rpc.kind] = (rpcCompleted[rpc.kind] ?? 0) + 1
      }
      return { ...((main as HTMLElement).dataset), frames: profile.frames, rpcStarted, rpcCompleted }
    })
    steadyState.push({
      elapsedMilliseconds: Date.now() - steadyStarted,
      phase: sample.phase,
      performance: sample.performance,
      tick: sample.snapshotTick,
      frames: sample.frames,
      rpcStarted: sample.rpcStarted,
      rpcCompleted: sample.rpcCompleted,
    })
    if (sample.phase === "Failed") break
  }

  const scenarios: {
    name: string
    samples: Awaited<ReturnType<typeof captureRuntime>>[]
  }[] = []
  const runScenario = async (name: string, seconds: number, start: () => Promise<void>, stop: () => Promise<void>) => {
    await start()
    const samples = [await captureRuntime()]
    for (let second = 0; second < seconds; second += 1) {
      await page.waitForTimeout(1_000)
      samples.push(await captureRuntime())
      if (samples.at(-1)?.phase === "Failed") break
    }
    await stop()
    scenarios.push({ name, samples })
  }
  if (runScenarios && initial.phase === "Ready") {
    if (shouldRunScenario("jump")) await runScenario("repeated-jump", 8, async () => {
      await page.evaluate(() => {
        const press = () => {
          dispatchEvent(new KeyboardEvent("keydown", { code: "Space", key: " ", bubbles: true }))
          setTimeout(() => dispatchEvent(new KeyboardEvent("keyup", { code: "Space", key: " ", bubbles: true })), 35)
        }
        press()
        ;(window as any).__playsrcJumpInterval = setInterval(press, 100)
      })
    }, async () => {
      await page.evaluate(() => {
        clearInterval((window as any).__playsrcJumpInterval)
        dispatchEvent(new KeyboardEvent("keyup", { code: "Space", key: " ", bubbles: true }))
      })
    })
    if (shouldRunScenario("wall")) await page.waitForTimeout(1_000)
    if (shouldRunScenario("wall")) await runScenario("held-forward", 15, async () => {
      await page.evaluate(() => dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true })))
    }, async () => {
      await page.evaluate(() => dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true })))
    })
    if (shouldRunScenario("fire")) await page.waitForTimeout(1_000)
    if (shouldRunScenario("fire")) await runScenario("held-primary-fire", 10, async () => {
      await page.evaluate(async () => {
        const canvas = document.querySelector(".world-canvas")
        if (!canvas) throw new Error("world canvas is unavailable")
        if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
        dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
      })
    }, async () => {
      await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))
    })
  }

  const raw = await page.evaluate(() => {
    const state = (window as any).__playsrcProfile
    return {
      now: performance.now(),
      state,
      dataset: { ...((document.querySelector("main") as HTMLElement | null)?.dataset ?? {}) },
      navigation: performance.getEntriesByType("navigation").map((entry) => ({
        duration: entry.duration,
        responseEnd: (entry as PerformanceNavigationTiming).responseEnd,
        domInteractive: (entry as PerformanceNavigationTiming).domInteractive,
        loadEventEnd: (entry as PerformanceNavigationTiming).loadEventEnd,
      })),
    }
  })
  clearInterval(processSampleTimer)
  const finalProcessInfo=await cdp.send("SystemInfo.getProcessInfo") as {processInfo:readonly BrowserProcessInfo[]}
  processSnapshots.push({at:Date.now(),processes:finalProcessInfo.processInfo})
  if(systemSampler){systemSampler.kill();await systemSamplerExit}
  const systemMetrics=summarizeBrowserMetrics(systemSamplerOutput,processSnapshots)
  await cdp.detach()

  const completed = (raw.state.rpcs as RpcRecord[]).filter((record) => record.finished !== undefined)
  const kinds = [...new Set(completed.map((record) => record.kind))].sort()
  const rpcSummary = Object.fromEntries(kinds.map((kind) => {
    const records = completed.filter((record) => record.kind === kind)
    const durations = records.map((record) => record.finished! - record.started).sort((a, b) => a - b)
    return [kind, {
      count: durations.length,
      totalMilliseconds: Number(durations.reduce((sum, value) => sum + value, 0).toFixed(3)),
      meanMilliseconds: Number((durations.reduce((sum, value) => sum + value, 0) / durations.length).toFixed(3)),
      p50Milliseconds: Number(percentile(durations, 0.5).toFixed(3)),
      p95Milliseconds: Number(percentile(durations, 0.95).toFixed(3)),
      p99Milliseconds: Number(percentile(durations, 0.99).toFixed(3)),
      maxMilliseconds: Number(durations.at(-1)!.toFixed(3)),
      maxBytes: Math.max(0, ...records.map((record) => record.bytes ?? 0)),
      workerTimings: Object.fromEntries(
        [...new Set(records.flatMap((record) => Object.keys(record.workerTimings ?? {})))].map((timing) => {
          const values = records.flatMap((record) => record.workerTimings?.[timing] ?? []).sort((a, b) => a - b)
          return [timing, {
            meanMilliseconds: Number((values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(3)),
            p50Milliseconds: Number(percentile(values, 0.5).toFixed(3)),
            p95Milliseconds: Number(percentile(values, 0.95).toFixed(3)),
            maxMilliseconds: Number((values.at(-1) ?? 0).toFixed(3)),
          }]
        }),
      ),
    }]
  }))
  const rafGapRecords = raw.state.rafGaps as { at: number; duration: number }[]
  const rafGaps = rafGapRecords.map((entry) => entry.duration).sort((a, b) => a - b)
  const longTasks = (raw.state.longTasks as { duration: number }[]).map((entry) => entry.duration).sort((a, b) => a - b)
  const presentationRecords = raw.state.presentations as PresentationRecord[]
  const timingKeys = ["models", "projectiles", "visibility", "particleWorker", "particleDecode", "audio", "dynamicItems", "world", "viewmodel", "render", "total"] as const
  const presentationSummary = (started: number, finished: number) => {
    const entries = presentationRecords
      .filter((record) => record.at >= started && record.at <= finished)
      .map((record) => ({ record, detail: JSON.parse(record.detail) as Record<string, number | string> }))
    const distributions = Object.fromEntries(timingKeys.map((key) => {
      const values = entries.map((entry) => Number(entry.detail[key] ?? 0)).sort((a, b) => a - b)
      return [key, {
        p50Milliseconds: Number(percentile(values, 0.5).toFixed(3)),
        p95Milliseconds: Number(percentile(values, 0.95).toFixed(3)),
        p99Milliseconds: Number(percentile(values, 0.99).toFixed(3)),
        maxMilliseconds: Number((values.at(-1) ?? 0).toFixed(3)),
      }]
    }))
    const worst = entries
      .toSorted((left, right) => Number(right.detail.total) - Number(left.detail.total))
      .slice(0, 10)
      .map(({ record, detail }) => {
        const frameStarted = record.at - Number(detail.total)
        return {
          at: Number(record.at.toFixed(3)),
          detail,
          overlappingWorkerRpc: completed
            .filter((rpc) => rpc.started <= record.at && rpc.finished! >= frameStarted)
            .map((rpc) => ({
              kind: rpc.kind,
              started: Number(rpc.started.toFixed(3)),
              duration: Number((rpc.finished! - rpc.started).toFixed(3)),
              workerTimings: rpc.workerTimings,
            })),
          overlappingLongTasks: (raw.state.longTasks as { at: number; duration: number }[])
            .filter((task) => task.at <= record.at && task.at + task.duration >= frameStarted),
          overlappingRafGaps: rafGapRecords
            .filter((gap) => gap.at >= frameStarted && gap.at - gap.duration <= record.at),
        }
      })
    return { count: entries.length, distributions, worst }
  }
  const firstSteady = steadyState[Math.floor((steadyState.length - 1) / 2)]
  const lastSteady = steadyState.at(-1)
  const steadySeconds = firstSteady && lastSteady
    ? (lastSteady.elapsedMilliseconds - firstSteady.elapsedMilliseconds) / 1_000
    : 0
  const rate = (last: number, first: number): number =>
    steadySeconds > 0 ? Number(((last - first) / steadySeconds).toFixed(3)) : 0
  const completedRate = (kind: string): number => rate(
    lastSteady?.rpcCompleted[kind] ?? 0,
    firstSteady?.rpcCompleted[kind] ?? 0,
  )
  const report = {
    requestedRafHz: rafHz || "native",
    mapOnly,
    startupMilliseconds,
    mapLoad: {
      mainMenuMilliseconds: Number(mainMenuMilliseconds.toFixed(3)),
      submittedMilliseconds: mapSubmittedMilliseconds === undefined ? null : Number(mapSubmittedMilliseconds.toFixed(3)),
      readyMilliseconds: gameplayReadyMilliseconds === undefined ? null : Number(gameplayReadyMilliseconds.toFixed(3)),
      durationMilliseconds: mapSubmittedMilliseconds === undefined || gameplayReadyMilliseconds === undefined
        ? null
        : Number((gameplayReadyMilliseconds - mapSubmittedMilliseconds).toFixed(3)),
      repeatedSubmittedMilliseconds: repeatedMapSubmittedMilliseconds === undefined ? null : Number(repeatedMapSubmittedMilliseconds.toFixed(3)),
      repeatedReadyMilliseconds: repeatedGameplayReadyMilliseconds === undefined ? null : Number(repeatedGameplayReadyMilliseconds.toFixed(3)),
      repeatedDurationMilliseconds: repeatedMapSubmittedMilliseconds === undefined || repeatedGameplayReadyMilliseconds === undefined
        ? null
        : Number((repeatedGameplayReadyMilliseconds - repeatedMapSubmittedMilliseconds).toFixed(3)),
      pageReloadSubmittedMilliseconds:pageReloadMapSubmittedMilliseconds===undefined?null:Number(pageReloadMapSubmittedMilliseconds.toFixed(3)),
      pageReloadReadyMilliseconds:pageReloadGameplayReadyMilliseconds===undefined?null:Number(pageReloadGameplayReadyMilliseconds.toFixed(3)),
      pageReloadDurationMilliseconds:pageReloadMapSubmittedMilliseconds===undefined||pageReloadGameplayReadyMilliseconds===undefined?null:Number((pageReloadGameplayReadyMilliseconds-pageReloadMapSubmittedMilliseconds).toFixed(3)),
      firstLoadPerformance,
      repeatedLoadPerformance,
      storageBeforeMap,
      storageAfterFirstMap,
      storageAfterRepeatedMap,
      pageReloadLoadPerformance,
      storageAfterPageReload,
    },
    terminalPhase: raw.dataset.phase,
    terminalDetail: raw.dataset.detail,
    input,
    steadyState,
    scenarios: scenarios.map((scenario) => ({
      name: scenario.name,
      presentationTrace: presentationSummary(scenario.samples[0]!.at, scenario.samples.at(-1)!.at),
      intervals: scenario.samples.slice(1).map((sample, index) => {
        const previous = scenario.samples[index]!
        const seconds = (sample.at - previous.at) / 1_000
        const rate = (kind: string) => Number((((sample.rpcCompleted[kind] ?? 0) - (previous.rpcCompleted[kind] ?? 0)) / seconds).toFixed(3))
        return {
          seconds: Number(seconds.toFixed(3)),
          rafCallbacksPerSecond: Number(((sample.frames - previous.frames) / seconds).toFixed(3)),
          observeCompletedPerSecond: rate("observe"),
          presentationsCompletedPerSecond: rate("models"),
          simulationTicksPerSecond: Number(((sample.tick - previous.tick) / seconds).toFixed(3)),
          tick: sample.tick,
          cameraPosition: sample.cameraPosition,
          wishSpeed: sample.wishSpeed,
          grounded: sample.grounded,
          sweepQueries: sample.sweepQueries,
          pointQueries: sample.pointQueries,
          movementContacts: sample.movementContacts,
          movementEvents: sample.movementEvents,
          fireEvents: sample.fireEvents,
          particleItems: sample.particleItems,
          performance: sample.performance,
          performanceDetail: sample.performanceDetail,
          phase: sample.phase,
        }
      }),
    })),
    runtimePerformanceProbe: raw.dataset.performance,
    snapshotTick: raw.dataset.snapshotTick,
    elapsedBrowserMilliseconds: raw.now,
    phases: raw.state.phases,
    workerRpc: rpcSummary,
    browserSystem:{
      initialProcesses:processInfo.processInfo,
      gpuInfo,
      samplerError:systemSamplerError.trim()||null,
      ...systemMetrics,
    },
    steadyRates: {
      measurementSeconds: Number(steadySeconds.toFixed(3)),
      rafCallbacksPerSecond: rate(lastSteady?.frames ?? 0, firstSteady?.frames ?? 0),
      observeStartedPerSecond: rate(
        lastSteady?.rpcStarted.observe ?? 0,
        firstSteady?.rpcStarted.observe ?? 0,
      ),
      observeCompletedPerSecond: completedRate("observe"),
      presentationsCompletedPerSecond: completedRate("models"),
      visibilityCompletedPerSecond: completedRate("visibility"),
      particlesCompletedPerSecond: completedRate("particles"),
      simulationTicksPerSecond: rate(Number(lastSteady?.tick ?? 0), Number(firstSteady?.tick ?? 0)),
    },
    mainThread: {
      frames: raw.state.frames,
      rafGapCount: rafGaps.length,
      rafGapP95Milliseconds: Number(percentile(rafGaps, 0.95).toFixed(3)),
      rafGapMaxMilliseconds: Number((rafGaps.at(-1) ?? 0).toFixed(3)),
      longTaskCount: longTasks.length,
      longTaskTotalMilliseconds: Number(longTasks.reduce((sum, value) => sum + value, 0).toFixed(3)),
      longTaskP95Milliseconds: Number(percentile(longTasks, 0.95).toFixed(3)),
      longTaskMaxMilliseconds: Number((longTasks.at(-1) ?? 0).toFixed(3)),
    },
    navigation: raw.navigation,
  }
  await testInfo.attach("playsrc-profile",{body:Buffer.from(JSON.stringify(report,null,2)),contentType:"application/json"})
  const performanceLocal=await loadLocalConfig()
  const performanceDirectory=path.join(performanceLocal.sourceCacheDir,"profiles","application-lifecycle","performance")
  await mkdir(performanceDirectory,{recursive:true})
  const comparisonLabel=process.env.PROFILE_COMPARISON_LABEL??"candidate"
  if(!/^(latest-main|candidate)$/u.test(comparisonLabel))throw new Error("application performance comparison label is invalid")
  await writeFile(path.join(performanceDirectory,`${comparisonLabel}-${process.pid}.json`),`${JSON.stringify(report,null,2)}\n`)
  console.log(`PLAYSRCPROFILE ${JSON.stringify({...report,browserSystem:{...report.browserSystem,timeline:`${report.browserSystem.timeline.length} samples retained in playsrc-profile attachment`}})}`)
  expect(raw.dataset.phase).toBe("Ready")
  if (typeof input.lookDisplayFrames === "number") {
    expect(input.lookDisplayFrames).toBeGreaterThan(1)
    expect(input.lookViewRevisions).toBeGreaterThan(1)
    expect(input.lookMouseRevisions).toBeGreaterThanOrEqual(input.lookEvents as number)
    expect(input.lookViewRevisions).toBe((input.lookMouseRevisions as number)+(input.lookSnapRevisions as number))
    expect(input.lookRepeatedPreparedFrames).toBeGreaterThan(0)
    expect(input.lookMaximumViewmodelYawError).toBeLessThanOrEqual(0.001)
    expect(input.lookMaximumViewmodelPitchError).toBeLessThanOrEqual(0.001)
    const motion = input.movementContinuity as {
      displayFrames: number
      movingFrames: number
      authoritativeTicks: number
      adapterMeanNanoseconds: number
      standaloneVisibilityCalls: number
      standaloneVisibilityP50Milliseconds: number
      allocationSamples: number
      sampledAllocationBytes: number
      heapBeforeBytes: number
      heapAfterBytes: number
    }
    expect(motion.displayFrames).toBeGreaterThan(20)
    expect(motion.movingFrames).toBeGreaterThan(10)
    expect(motion.authoritativeTicks).toBeGreaterThan(30)
    expect(motion.standaloneVisibilityCalls).toBe(0)
    expect(motion.adapterMeanNanoseconds).toBeGreaterThan(0)
    expect(motion.allocationSamples).toBeGreaterThan(0)
    expect(motion.sampledAllocationBytes).toBeGreaterThan(0)
    expect(motion.heapBeforeBytes).toBeGreaterThan(0)
    expect(motion.heapAfterBytes).toBeGreaterThan(0)
    if (motion.standaloneVisibilityP50Milliseconds > 0) {
      expect(motion.adapterMeanNanoseconds / 1_000_000).toBeLessThan(motion.standaloneVisibilityP50Milliseconds)
    }
  }
})

test.describe("TF2 application generation lifecycle", () => {
  test.use({ allowRecoverableApplicationFailure: true })

  test("loads both catalog maps, preserves sky ownership, routes Escape, rolls back, and recovers", async ({ page }, testInfo) => {
    test.skip(process.env.PROFILE_BASELINE_ONLY === "1", "baseline-only run measures the unchanged gameplay execution path")
    await page.addInitScript(() => {
      ;(window as any).__playsrcProfile = { controllerFreeSkyViews: 0 }
      let locked: Element | null = null
      Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
      Object.defineProperty(Element.prototype, "requestPointerLock", {
        configurable: true,
        value(this: Element) {
          locked = this
          queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
          return Promise.resolve()
        },
      })
      Object.defineProperty(document, "exitPointerLock", {
        configurable: true,
        value() {
          locked = null
          queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
          return Promise.resolve()
        },
      })
    })

    const local = await loadLocalConfig()
    const evidenceDirectory = path.join(local.sourceCacheDir, "profiles", "application-lifecycle")
    await mkdir(evidenceDirectory, { recursive: true })
    const root = page.locator("main")
    const canvas = page.locator("canvas.world-canvas")
    const entry = page.locator("[aria-label='Console command']")
    const transitions: Array<{ target: string; outcome: string; generation: number | null; skySurfaces: number }> = []
    const open = async () => {
      await page.goto("/", { waitUntil: "load", timeout: 30_000 })
      await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 180_000, polling: 50 })
      expect(await root.getAttribute("data-phase")).toBe("MainMenu")
    }
    const command = async (text: string) => {
      if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
      await expect(entry).toBeVisible()
      await entry.fill(text)
      await page.keyboard.press("Enter")
    }
    const load = async (target: string, expected: "Ready" | "Failed" = "Ready") => {
      await command(`map ${target}`)
      await page.waitForFunction(() => {
        const main = document.querySelector<HTMLElement>("main")
        return main?.dataset.phase === "Loading" || main?.dataset.phase === "Replacing" || main?.dataset.phase === "Failed"
          || main?.dataset.detail?.startsWith("Prior map retained:")
      }, undefined, { timeout: 30_000, polling: 10 })
      await page.waitForFunction(() => {
        const main = document.querySelector<HTMLElement>("main")
        return main?.dataset.phase === "Failed" || (main?.dataset.phase === "Ready" && main.dataset.gameui === "in-game")
      }, undefined, { timeout: 600_000, polling: 20 })
      expect(await root.getAttribute("data-phase")).toBe(expected)
      if (expected === "Ready") {
        expect(await root.getAttribute("data-detail")).not.toMatch(/^Prior map retained:/u)
        await expect.poll(async () => {
          const value = await canvas.getAttribute("data-sky3d-pass")
          return value ? (JSON.parse(value) as { skySurfaces: number }).skySurfaces : 0
        }, { timeout: 60_000 })[target === "pl_upward" ? "toBeGreaterThan" : "toBe"](0)
        const sky = await canvas.getAttribute("data-sky3d-pass")
        transitions.push({
          target,
          outcome: "Ready",
          generation: Number(await root.getAttribute("data-generation")),
          skySurfaces: sky ? (JSON.parse(sky) as { skySurfaces: number }).skySurfaces : 0,
        })
      } else {
        transitions.push({ target, outcome: "Failed", generation: null, skySurfaces: 0 })
      }
    }

    await open()
    const response = await page.request.get("/playsrc-config.json")
    expect(response.status()).toBe(200)
    const configuration = await response.json() as {
      assetOrigin: string
      defaultTarget: string
      targets: readonly { target: string; objects: { bsp: { sha256: string } } }[]
    }
    expect(configuration.targets.map((target) => target.target)).toEqual(["jump_beef", "pl_upward"])
    const jump = configuration.targets.find((target) => target.target === "jump_beef")!
    const upward = configuration.targets.find((target) => target.target === "pl_upward")!
    const crosshair = page.locator("[data-tf2-crosshair='authored']")
    const captureCrosshair = async (identity: "stock" | "custom" | "replaced") => {
      await expect(crosshair).toBeVisible()
      const sample = await crosshair.evaluate(async (element, mode) => {
        const style = getComputedStyle(element)
        const encoded = style.backgroundImage.match(/^url\(["']?data:image\/svg\+xml,([^"']+)["']?\)$/u)?.[1]
        if (!encoded) throw new Error("authored crosshair SVG is unavailable")
        const documentText = decodeURIComponent(encoded)
        const source = documentText.match(/href="(data:image\/png;base64,[^"]+)"/u)?.[1]
        if (!source) throw new Error("authored crosshair source PNG is unavailable")
        const image = new Image()
        image.src = source
        await image.decode()
        const surface = document.createElement("canvas")
        surface.width = image.width
        surface.height = image.height
        const context = surface.getContext("2d")
        if (!context) throw new Error("authored crosshair source decoder is unavailable")
        context.drawImage(image, 0, 0)
        const pixels = context.getImageData(0, 0, image.width, image.height).data
        let selected = -1
        for (let offset = 0; offset < pixels.length; offset += 4) {
          if (pixels[offset + 3] !== 255) continue
          if (mode !== "stock" && pixels[offset] === 255 && pixels[offset + 1] === 255 && pixels[offset + 2] === 255) continue
          selected = offset
          break
        }
        if (selected < 0) throw new Error("authored crosshair has no exact opaque source pixel")
        const bounds = element.getBoundingClientRect()
        const color = (element as HTMLElement).dataset.crosshairColor?.split(" ").map(Number)
        if (!color || color.length !== 4) throw new Error("authored crosshair tint is unavailable")
        const offset = selected / 4
        const point = { x: offset % image.width, y: Math.floor(offset / image.width) }
        return {
          style: (element as HTMLElement).dataset.crosshairStyle ?? "",
          material: (element as HTMLElement).dataset.sourceMaterialSha256 ?? "",
          texture: (element as HTMLElement).dataset.sourceTextureSha256 ?? "",
          frame: (element as HTMLElement).dataset.sourceFrameSha256 ?? "",
          tint: color,
          bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height },
          point,
          expected: [pixels[selected]!, pixels[selected + 1]!, pixels[selected + 2]!].map((value, index) => Math.round(value * color[index]! / 255)),
        }
      }, identity)
      await page.screenshot({ path: path.join(evidenceDirectory, `${identity}-crosshair.png`) })
      const actual = screenshotPixel(await page.screenshot({
        clip: { x: sample.bounds.x + sample.point.x, y: sample.bounds.y + sample.point.y, width: 1, height: 1 },
      }))
      for (let channel = 0; channel < 3; channel += 1) {
        expect(Math.abs(actual[channel]! - sample.expected[channel]!)).toBeLessThanOrEqual(2)
      }
      return { ...sample, actual }
    }

    await load(upward.target)
    await expect.poll(async () => {
      const value = await canvas.getAttribute("data-sky3d-pass")
      return value ? (JSON.parse(value) as { skySurfaces: number }).skySurfaces : 0
    }, { timeout: 60_000 }).toBeGreaterThan(0)
    expect(JSON.parse((await canvas.getAttribute("data-static-props"))!).total).toBe(1244)

    await page.keyboard.press("Backquote")
    await canvas.click()
    await expect.poll(async () => await root.getAttribute("data-pointer-locked")).toBe("true")
    const escapeCycles: Array<{ opened: string | null; closed: string | null; pointerRestored: string | null }> = []
    for (let cycle = 0; cycle < 3; cycle += 1) {
      await page.keyboard.press("Escape")
      await expect.poll(async () => await root.getAttribute("data-gameui")).toBe("pause")
      expect(await root.getAttribute("data-pointer-locked")).toBe("false")
      const opened = await root.getAttribute("data-gameui")
      await page.keyboard.press("Escape")
      await expect.poll(async () => await root.getAttribute("data-gameui")).toBe("in-game")
      await expect.poll(async () => await root.getAttribute("data-pointer-locked")).toBe("true")
      escapeCycles.push({ opened, closed: await root.getAttribute("data-gameui"), pointerRestored: await root.getAttribute("data-pointer-locked") })
    }
    const stockCrosshair = await captureCrosshair("stock")
    expect(stockCrosshair.style).toBe("stock")
    expect(stockCrosshair.bounds).toEqual({ x: 624, y: 344, width: 32, height: 32 })

    for (const [name, value] of [
      ["cl_crosshair_red", "48"],
      ["cl_crosshair_green", "120"],
      ["cl_crosshair_blue", "240"],
      ["cl_crosshair_scale", "32"],
      ["cl_crosshair_file", "crosshair5"],
    ] as const) {
      await command(`${name} ${value}`)
      await expect.poll(async () => await page.locator("[aria-label='Console output']").innerText()).toContain(`\"${name}\" = \"${value}\"`)
    }
    await page.keyboard.press("Backquote")
    const customCrosshair = await captureCrosshair("custom")
    expect(customCrosshair.style).toBe("crosshair5")
    expect(customCrosshair.tint).toEqual([48, 120, 240, 255])
    expect(customCrosshair.bounds).toEqual({ x: 608, y: 328, width: 64, height: 64 })
    await canvas.click()
    await expect.poll(async () => await root.getAttribute("data-pointer-locked")).toBe("true")
    const beforeLookPixels = await page.screenshot({ path: path.join(evidenceDirectory, "rapid-look-before.png") })
    const rapidLook = page.evaluate(async () => {
      const display = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")
      if (!display) throw new Error("rapid-look display canvas is unavailable")
      const records: Array<{ yaw: number; pitch: number; modelYaw: number; modelPitch: number; localYaw: number; localPitch: number }> = []
      const observer = new MutationObserver(() => records.push({
        yaw: Number(display.dataset.displayCameraYaw),
        pitch: Number(display.dataset.displayCameraPitch),
        modelYaw: Number(display.dataset.displayViewmodelYaw),
        modelPitch: Number(display.dataset.displayViewmodelPitch),
        localYaw: Number(display.dataset.displayViewmodelLocalYaw),
        localPitch: Number(display.dataset.displayViewmodelLocalPitch),
      }))
      observer.observe(display, { attributes: true, attributeFilter: ["data-display-frame"] })
      let events = 0
      const interval = setInterval(() => {
        const event = new MouseEvent("mousemove", { bubbles: true })
        Object.defineProperties(event, { movementX: { value: 4 }, movementY: { value: events % 2 === 0 ? 2 : -2 } })
        dispatchEvent(event)
        events += 1
      }, 4)
      await new Promise((resolve) => setTimeout(resolve, 350))
      clearInterval(interval)
      await new Promise((resolve) => setTimeout(resolve, 50))
      observer.disconnect()
      return {
        events,
        frames: records.length,
        maximumYawError: Math.max(0, ...records.map((record) => Math.abs(record.modelYaw - record.yaw - record.localYaw))),
        maximumPitchError: Math.max(0, ...records.map((record) => Math.abs(record.modelPitch - record.pitch - record.localPitch))),
      }
    })
    await page.waitForTimeout(140)
    const duringLookPixels = await page.screenshot({ path: path.join(evidenceDirectory, "rapid-look-during.png") })
    const beforeLookImage = decodeScreenshot(beforeLookPixels)
    const duringLookImage = decodeScreenshot(duringLookPixels)
    const rapidLookEvidence = {
      ...await rapidLook,
      beforeSha256: createHash("sha256").update(beforeLookPixels).digest("hex"),
      duringSha256: createHash("sha256").update(duringLookPixels).digest("hex"),
      weaponPixels: screenshotRegionMotion(beforeLookImage, duringLookImage, { x: 1030, y: 500, width: 160, height: 90 }),
      worldPixels: screenshotRegionMotion(beforeLookImage, duringLookImage, { x: 400, y: 350, width: 120, height: 80 }),
      crosshairPixels: screenshotRegionMotion(beforeLookImage, duringLookImage, { x: 640, y: 359, width: 1, height: 1 }),
    }
    expect(rapidLookEvidence.events).toBeGreaterThan(30)
    expect(rapidLookEvidence.frames).toBeGreaterThan(10)
    expect(rapidLookEvidence.maximumYawError).toBeLessThanOrEqual(0.001)
    expect(rapidLookEvidence.maximumPitchError).toBeLessThanOrEqual(0.001)
    expect(rapidLookEvidence.beforeSha256).not.toBe(rapidLookEvidence.duringSha256)
    expect(rapidLookEvidence.worldPixels.changedFraction).toBeGreaterThan(0.25)
    expect(rapidLookEvidence.weaponPixels.changedFraction).toBeLessThanOrEqual(0.02)
    expect(rapidLookEvidence.weaponPixels.maximumChannelDelta).toBeLessThanOrEqual(12)
    expect(rapidLookEvidence.crosshairPixels.maximumChannelDelta).toBe(0)

    await load(jump.target)
    await expect.poll(async () => await canvas.getAttribute("data-sky3d-pass"), { timeout: 60_000 }).toBe("")
    expect(JSON.parse((await canvas.getAttribute("data-static-props"))!).total).toBe(0)
    await page.keyboard.press("Backquote")
    const replacedCrosshair = await captureCrosshair("replaced")
    expect(replacedCrosshair.style).toBe("crosshair5")
    expect(replacedCrosshair.tint).toEqual([48, 120, 240, 255])
    const controllerFreeLeaf = await page.evaluate(() => {
      const main = document.querySelector<HTMLElement>("main")
      const profile = (window as any).__playsrcProfile as {
        coverageSamples?: readonly { leaf: number; position: readonly [number, number, number] }[]
        displacementCameraOverride?: { position: readonly [number, number, number]; yawDegrees: number; pitchDegrees: number }
      }
      if (!main) throw new Error("controller-free sky application root is unavailable")
      const authoredSkyLeaves = new Set([129, 130, 132, 139, 143, 149, 153, 157, 161, 166, 170, 171, 173, 174, 178])
      const sample = profile.coverageSamples?.find((candidate) => authoredSkyLeaves.has(candidate.leaf))
      if (!sample) throw new Error("exact jump_beef LEAF_FLAGS_SKY coverage sample is unavailable")
      profile.displacementCameraOverride = {
        position: sample.position,
        yawDegrees: Number(main.dataset.cameraYaw ?? 0),
        pitchDegrees: Number(main.dataset.cameraPitch ?? 0),
      }
      return sample.leaf
    })
    await expect.poll(async () => await canvas.getAttribute("data-sky-visibility-disposition"), { timeout: 60_000 }).toBe("controller-absent")
    expect(await canvas.getAttribute("data-sky3d-pass")).toBe("")
    expect(await root.getAttribute("data-phase")).toBe("Ready")
    const controllerFreeSkyViews = await page.evaluate(() => (window as any).__playsrcProfile.controllerFreeSkyViews as number)
    expect(controllerFreeSkyViews).toBeGreaterThan(0)
    await page.evaluate(() => { delete (window as any).__playsrcProfile.displacementCameraOverride })
    await load(jump.target)

    const upwardBsp = `${configuration.assetOrigin}/objects/sha256/${upward.objects.bsp.sha256}`
    await page.route(upwardBsp, async (route) => route.fulfill({ status: 503, body: "fixed replacement failure" }), { times: 1 })
    await command(`map ${upward.target}`)
    await expect.poll(async () => await root.getAttribute("data-detail"), { timeout: 60_000 }).toMatch(/^Prior map retained:/u)
    expect(await root.getAttribute("data-phase")).toBe("Ready")
    expect(JSON.parse((await canvas.getAttribute("data-static-props"))!).total).toBe(0)
    transitions.push({ target: upward.target, outcome: "prior-retained", generation: Number(await root.getAttribute("data-generation")), skySurfaces: 0 })
    await page.unroute(upwardBsp)

    await load(jump.target)

    await page.reload({ waitUntil: "load", timeout: 30_000 })
    await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.phase === "MainMenu", undefined, { timeout: 180_000, polling: 50 })
    const jumpBsp = `${configuration.assetOrigin}/objects/sha256/${jump.objects.bsp.sha256}`
    await page.route(jumpBsp, async (route) => route.fulfill({ status: 503, body: "fixed initial failure" }), { times: 1 })
    await load(jump.target, "Failed")
    const failureDetail = await root.getAttribute("data-detail")
    expect(failureDetail).toBeTruthy()
    await page.unroute(jumpBsp)
    await load(jump.target)
    expect(await root.getAttribute("data-gameui")).toBe("in-game")

    const report = {
      schema: "playsrc-tf2-application-lifecycle-v1",
      maps: configuration.targets.map((target) => target.target),
      defaultTarget: configuration.defaultTarget,
      transitions,
      escapeCycles,
      crosshair: { stock: stockCrosshair, custom: customCrosshair, replaced: replacedCrosshair },
      rapidLook: rapidLookEvidence,
      controllerFreeLeaf,
      controllerFreeSkyViews,
      failedInitialLoad: failureDetail,
      terminalPhase: await root.getAttribute("data-phase"),
      terminalGameUi: await root.getAttribute("data-gameui"),
    }
    await writeFile(path.join(evidenceDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
    await testInfo.attach("tf2-application-lifecycle", { body: Buffer.from(JSON.stringify(report, null, 2)), contentType: "application/json" })
    console.log(`PLAYSRC_APPLICATION_LIFECYCLE ${JSON.stringify(report)}`)
  })
})
