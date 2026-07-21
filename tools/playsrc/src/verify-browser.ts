import { constants as fsConstants } from "node:fs"
import { copyFile, mkdir, readFile, rm } from "node:fs/promises"
import path from "node:path"
import { repositoryRoot, type LocalConfig } from "./config"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import { TF2_BROWSER_SETTINGS_STORAGE_KEY } from "@playsrc/game-tf2-browser/settings-integration"

const MAX_OUTPUT_BYTES = 1024 * 1024
const PROCESS_READY_TIMEOUT_MS = 300_000
const PROCESS_EXIT_TIMEOUT_MS = 30_000
const APPLICATION_URL = "http://127.0.0.1:4173/"
const VIEWPORT_WIDTH = 1280
const VIEWPORT_HEIGHT = 720
const BACKGROUND_RGB = [17, 24, 32] as const
const EXPECTED_RESOURCE_GRAPH_SHA256="e26089c098ddb15185ae1ea1f188c958c6e07c54cf631ca2c663d8ecb5933eaa"

export class BrowserEvidenceError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "BrowserEvidenceError"
  }
}

async function agent(args: string[]): Promise<string> {
  const child = Bun.spawn(["agent-browser", ...args], {
    stdout: "pipe",
    stderr: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).arrayBuffer(),
    new Response(child.stderr).arrayBuffer(),
    child.exited,
  ])
  if (stdout.byteLength > MAX_OUTPUT_BYTES || stderr.byteLength > MAX_OUTPUT_BYTES) {
    throw new BrowserEvidenceError("agent-browser output exceeded 1048576 bytes")
  }
  const output = new TextDecoder().decode(stdout).trim()
  if (exitCode !== 0) {
    const error = new TextDecoder().decode(stderr).trim()
    throw new BrowserEvidenceError(`agent-browser ${JSON.stringify(args)} failed: ${error || output}`)
  }
  return output
}

async function clickVguiPanel(session: string, name: string): Promise<void> {
  const result = parseJson<boolean>(await agent([
    "--session", session, "eval",
    `(()=>{const e=document.querySelector('[data-vgui-name="${name}"]');if(!e)return false;const r=e.getBoundingClientRect(),x=r.left+Math.min(8,r.width/2),y=r.top+Math.min(8,r.height/2),base={bubbles:true,clientX:x,clientY:y,button:0,pointerId:47};e.dispatchEvent(new PointerEvent('pointerdown',{...base,buttons:1}));e.dispatchEvent(new PointerEvent('pointerup',{...base,buttons:0}));return true})()`,
  ]))
  require(result, `VGUI panel ${name} is unavailable`)
}

async function mouseClickVguiPanel(session: string, name: string): Promise<void> {
  const rect = parseJson<{ x: number; y: number; width: number; height: number } | null>(await agent([
    "--session", session, "eval", `(()=>{const value=document.querySelector('[data-vgui-name="${name}"]');return value?value.getBoundingClientRect().toJSON():null})()`,
  ]))
  require(rect, `VGUI panel ${name} is unavailable`)
  const x = String(rect.x + rect.width / 2)
  const y = String(rect.y + Math.min(8, rect.height / 2))
  await agent(["--session", session, "mouse", "move", x, y])
  await agent(["--session", session, "mouse", "down", "left"])
  await agent(["--session", session, "mouse", "up", "left"])
}

async function clickVguiSelector(session: string, selector: string): Promise<void> {
  const result = parseJson<boolean>(await agent([
    "--session", session, "eval",
    `(()=>{const e=document.querySelector(${JSON.stringify(selector)});if(!e)return false;const r=e.getBoundingClientRect(),x=r.left+Math.min(8,r.width/2),y=r.top+Math.min(8,r.height/2),base={bubbles:true,clientX:x,clientY:y,button:0,pointerId:49};e.dispatchEvent(new PointerEvent('pointerdown',{...base,buttons:1}));e.dispatchEvent(new PointerEvent('pointerup',{...base,buttons:0}));return true})()`,
  ]))
  require(result, `VGUI selector ${selector} is unavailable`)
}

async function clickOptionsTab(session: string, name: string): Promise<void> {
  let rect = parseJson<{ x: number; y: number; width: number; height: number } | null>(await agent([
    "--session", session, "eval",
    `(()=>{const e=[...document.querySelectorAll('.options-layer [data-vgui-name=Sheet] [role=tab]')].find(value=>value.textContent==='${name}');return e?e.getBoundingClientRect().toJSON():null})()`,
  ]))
  if (!rect || rect.width === 0 || rect.height === 0) {
    await clickVguiPanel(session, "SettingsButton")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.optionsVisible==='true'&&getComputedStyle(document.querySelector('[data-tf2-options-mount=standard]')).display==='block'", "--timeout", "10000"])
    rect = parseJson<{ x: number; y: number; width: number; height: number }>(await agent([
      "--session", session, "eval",
      `(()=>[...document.querySelectorAll('.options-layer [data-vgui-name=Sheet] [role=tab]')].find(value=>value.textContent==='${name}').getBoundingClientRect().toJSON())()`,
    ]))
  }
  require(rect, `Options tab ${name} is unavailable`)
  const x = String(rect.x + rect.width / 2)
  const y = String(rect.y + rect.height / 2)
  const selected = `[...document.querySelectorAll('.options-layer [data-vgui-name=Sheet] [role=tab]')].find(value=>value.textContent==='${name}')?.getAttribute('aria-selected')==='true'`
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await agent(["--session", session, "eval", "window.focus();true"])
    await agent(["--session", session, "mouse", "move", x, y])
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "mouse", "up", "left"])
    if (parseJson<boolean>(await agent(["--session", session, "eval", selected]))) return
  }
  const stack = await agent(["--session", session, "eval", `document.elementsFromPoint(${x},${y}).slice(0,8).map(value=>({name:value.dataset.vguiName,control:value.dataset.vguiControl,id:value.id}))`])
  throw new BrowserEvidenceError(`Options tab ${name} did not activate; rect=${JSON.stringify(rect)}; stack=${stack}`)
}

async function scrollAdvancedControlIntoView(session: string, selector: string): Promise<void> {
  const listSelector = "[data-vgui-runtime=tf2-advanced-options] [data-vgui-name=PanelListPanel]"
  await agent(["--session", session, "hover", listSelector])
  let last: unknown = null
  for (let attempt = 0; attempt < 64; attempt += 1) {
    const state = parseJson<{ admitted: boolean; direction: "up" | "down" | null; target: number[]; cover: string | null }>(await agent([
      "--session", session, "eval",
      `(()=>{const target=document.querySelector(${JSON.stringify(selector)}),list=document.querySelector(${JSON.stringify(listSelector)}),r=target.getBoundingClientRect(),l=list.getBoundingClientRect(),top=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return{admitted:top===target||target.contains(top),direction:r.bottom>l.bottom?'down':r.top<l.top?'up':null,target:[r.x,r.y,r.width,r.height],cover:top?.getAttribute('data-vgui-name')??top?.id??null}})()`,
    ]))
    last = state
    if (state.admitted) return
    await agent([
      "--session", session, "eval",
      `(()=>{const list=document.querySelector(${JSON.stringify(listSelector)}),r=list.getBoundingClientRect();list.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,clientX:r.x+r.width/2,clientY:r.y+r.height/2,deltaY:${state.direction === "up" ? -200 : 200}}));return true})()`,
    ])
    await agent(["--session", session, "wait", "20"])
  }
  throw new BrowserEvidenceError(`Advanced control did not enter the visible input stack: ${selector}; state=${JSON.stringify(last)}`)
}

function parseJson<T>(value: string): T {
  try {
    return JSON.parse(value) as T
  } catch {
    throw new BrowserEvidenceError("agent-browser evaluation did not return JSON")
  }
}

function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new BrowserEvidenceError(message)
}

type BlockerPartition = Readonly<{
  content: readonly string[]
  behavior: readonly string[]
  contentClosureBehavior: readonly string[]
  visualBehavior: readonly string[]
  authorityBehavior: readonly string[]
  platform: readonly string[]
}>

async function classifySupportBlockers(
  config: LocalConfig,
  blockers: readonly string[],
): Promise<BlockerPartition> {
  const bytes = await readFile(path.join(
    config.sourceCacheDir,
    "browser-bundles",
    "jump_beef.dependencies.json",
  ))
  let value: unknown
  try {
    value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes))
  } catch {
    throw new BrowserEvidenceError("source dependency ledger is malformed")
  }
  require(typeof value === "object" && value !== null && !Array.isArray(value),
    "source dependency ledger root is malformed")
  const ledger = value as Record<string, unknown>
  require(ledger.schema === "playsrc-source-dependency-ledger-v1" &&
    ledger.game === "tf2" &&
    ledger.appId === TF2_CONTENT_BUILD.appId &&
    ledger.contentBuild === TF2_CONTENT_BUILD.contentBuild &&
    ledger.patchVersion === TF2_CONTENT_BUILD.patchVersion &&
    ledger.gameinfoSha256 === TF2_CONTENT_BUILD.gameinfoSha256 &&
    JSON.stringify(ledger.installedDepots) === JSON.stringify(TF2_CONTENT_BUILD.installedDepots) &&
    ledger.target === "jump_beef" &&
    typeof ledger.resourceGraph === "object" && ledger.resourceGraph !== null &&
    (ledger.resourceGraph as Record<string, unknown>).sha256 === EXPECTED_RESOURCE_GRAPH_SHA256 &&
    Array.isArray(ledger.requests) && ledger.requests.length <= 4_096,
  "source dependency ledger identity is malformed")
  const outcomes = new Map<string, string>()
  const requests = new Map<string, Record<string, unknown>>()
  for (const item of ledger.requests) {
    require(typeof item === "object" && item !== null && !Array.isArray(item),
      "source dependency ledger request is malformed")
    const request = item as Record<string, unknown>
    require(typeof request.logicalPath === "string" &&
      request.logicalPath === request.logicalPath.toLowerCase() &&
      (request.outcome === "resolved" || request.outcome === "authoritative-absence") &&
      !outcomes.has(request.logicalPath), "source dependency ledger request identity is malformed")
    outcomes.set(request.logicalPath, request.outcome)
    requests.set(request.logicalPath, request)
  }
  const requireSource = (logicalPath: string, byteLength: number, sha256: string, providerIdentity: string) => {
    const request = requests.get(logicalPath)
    const descriptor = request?.descriptor as Record<string, unknown> | undefined
    const provenance = request?.provenance as Record<string, unknown> | undefined
    require(request?.outcome === "resolved" && descriptor?.byteLength === String(byteLength)
      && descriptor.sha256 === sha256 && provenance?.providerIdentity === providerIdentity,
    `source dependency ${logicalPath} differs`)
  }
  require(Array.isArray(ledger.startupSources), "startup source ledger is missing")
  const startupSources = new Map((ledger.startupSources as Record<string, unknown>[]).map((source) => [source.logicalPath, source]))
  for (const source of [TF2_CONFIGURED_STARTUP.manifest, TF2_CONFIGURED_STARTUP.source, TF2_CONFIGURED_STARTUP.browserRepresentation]) {
    const record = startupSources.get(source.logicalPath)
    const descriptor = record?.descriptor as Record<string, unknown> | undefined
    const provenance = record?.provenance as Record<string, unknown> | undefined
    require(descriptor?.byteLength === String(source.byteLength) && descriptor.sha256 === source.sha256
      && provenance?.providerIdentity === source.providerIdentity && provenance.providerRevision === source.providerRevision,
    `startup source ${source.logicalPath} differs`)
  }
  requireSource(TF2_STAMP_BACKGROUND.material.logicalPath, TF2_STAMP_BACKGROUND.material.byteLength, TF2_STAMP_BACKGROUND.material.sha256, "game-04-tf2_misc_dir.vpk")
  requireSource(TF2_STAMP_BACKGROUND.texture.logicalPath, TF2_STAMP_BACKGROUND.texture.byteLength, TF2_STAMP_BACKGROUND.texture.sha256, "game-01-tf2_textures_dir.vpk")
  const mapPhoto = requests.get("materials/vgui/maps/menu_photos_jump_beef.vmt")
  require(mapPhoto?.outcome === "authoritative-absence" && JSON.stringify((mapPhoto.checked as Record<string, string>[]).map((location) => `${location.providerIdentity}:${location.location}`)) === JSON.stringify(TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS),
    "jump_beef map-photo checked locations differ")

  const content: string[] = []
  const behavior: string[] = []
  const contentClosureBehavior: string[] = []
  const visualBehavior: string[] = []
  const authorityBehavior: string[] = []
  const platform: string[] = []
  const recordBehavior = (blocker: string, owner: "content-closure" | "visual" | "authority") => {
    behavior.push(blocker)
    if (owner === "content-closure") contentClosureBehavior.push(blocker)
    else if (owner === "visual") visualBehavior.push(blocker)
    else authorityBehavior.push(blocker)
  }
  for (const blocker of blockers) {
    const material = /^Missing resolved material: (.+)$/u.exec(blocker)?.[1]
    if (blocker.startsWith("TF2UiFontUnavailable: ")) {
      platform.push(blocker)
    } else if (blocker.startsWith("TF2Ui") || blocker.startsWith("TF2GameUi")) {
      recordBehavior(blocker, "visual")
    } else if (material) {
      if (material.startsWith("materials/")) {
        if (outcomes.get(material) === "resolved") recordBehavior(blocker, "content-closure")
        else content.push(blocker)
      } else if (material === "map-environment-presentation" || material === "map-water-presentation") {
        recordBehavior(blocker, "content-closure")
      } else {
        throw new BrowserEvidenceError(`unclassified material diagnostic: ${blocker}`)
      }
    } else if (
      blocker.startsWith("TF2 SoundSamples unavailable: ")
      || blocker.startsWith("TF2 viewmodel attachment transform unavailable: ")
    ) {
      recordBehavior(blocker, "content-closure")
    } else if (/^(MissingModelLighting|MissingModelEyeState): /u.test(blocker)) {
      recordBehavior(blocker, "visual")
    } else if (/^(MissingMaterial|MissingDirectionalInput|MissingProfileInput|UnsupportedProfileInput): /u.test(blocker)) {
      const logicalPath = /^MissingMaterial: ([^ ]+)/u.exec(blocker)?.[1]
      if (logicalPath?.startsWith("materials/") && outcomes.get(logicalPath) !== "resolved") content.push(blocker)
      else recordBehavior(blocker, "content-closure")
    } else if (blocker.startsWith("MissingTextureMips: ") || [
      "Missing authored texture mip planes",
      "Missing current model lightcache selections, game-owned eye targets, and per-draw StudioModel lighting/eye state",
      "Missing decoded profile-qualified sky and cubemap subresources",
      "Missing complete Water material and reflection/refraction view inputs",
      "Missing current fog-controller state and transition inputs",
    ].some((prefix) => blocker.startsWith(prefix))) {
      recordBehavior(blocker, "visual")
    } else if (blocker.startsWith("Missing: ")||blocker.startsWith("Missing exact IVP sticky rigid-body solver: ") || blocker.startsWith("VGUI presentation random source unavailable: ")) {
      recordBehavior(blocker, "authority")
    } else if (blocker.startsWith("ModelArtifactCacheUnavailable: ")) {
      content.push(blocker)
    } else if (
      blocker.startsWith("The configured console resolves its complete SourceScheme")
      || blocker.startsWith("TF2 console platform fonts unsupported: ")
      || blocker.startsWith("Windows Tahoma and Lucida Console faces loaded")
      || blocker.startsWith("AudioUnavailable: ")
    ) {
      platform.push(blocker)
    } else {
      throw new BrowserEvidenceError(`unclassified support blocker: ${blocker}`)
    }
  }
  return Object.freeze({
    content: Object.freeze(content.sort()),
    behavior: Object.freeze(behavior.sort()),
    contentClosureBehavior: Object.freeze(contentClosureBehavior.sort()),
    visualBehavior: Object.freeze(visualBehavior.sort()),
    authorityBehavior: Object.freeze(authorityBehavior.sort()),
    platform: Object.freeze(platform.sort()),
  })
}

type DecodedPng = Readonly<{
  width: number
  height: number
  rgb: Uint8Array
}>

type RegionMetric = Readonly<{
  name: string
  x: number
  y: number
  width: number
  height: number
  nonBackgroundRatio: number
  meanLuma: number
  sha256: string
}>

type CanvasEvidence = Readonly<{
  sha256: string
  byteLength: number
  width: number
  height: number
  regions: readonly RegionMetric[]
}>
type InterfaceEvidence = Readonly<{ sha256: string; byteLength: number; width: number; height: number }>

type CameraObservation = Readonly<{
  position: readonly [number, number, number]
  yaw: number
  pitch: number
  verticalFov: number
  near: number
  far: number
}>

type SpawnObservation = Readonly<{
  entity: number
  hammerId: number
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>
type CrouchTrajectory = Readonly<{ fractions: readonly number[]; offsets: readonly number[] }>

const VISUAL_REGIONS = Object.freeze([
  Object.freeze({ name: "ceiling", x: 400, y: 120, width: 320, height: 100 }),
  Object.freeze({ name: "forward-wall", x: 400, y: 270, width: 320, height: 180 }),
  Object.freeze({ name: "floor", x: 180, y: 500, width: 160, height: 130 }),
])

function readUint32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, false)
}

function paeth(left: number, above: number, upperLeft: number): number {
  const estimate = left + above - upperLeft
  const leftDistance = Math.abs(estimate - left)
  const aboveDistance = Math.abs(estimate - above)
  const upperLeftDistance = Math.abs(estimate - upperLeft)
  if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) return left
  return aboveDistance <= upperLeftDistance ? above : upperLeft
}

async function decodePng(bytes: Uint8Array): Promise<DecodedPng> {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10]
  require(bytes.byteLength >= 33 &&
    signature.every((value, index) => bytes[index] === value), "canvas PNG signature is invalid")
  let offset = 8
  let width = 0
  let height = 0
  let channels = 0
  const compressedParts: Uint8Array[] = []
  while (offset < bytes.byteLength) {
    require(offset + 12 <= bytes.byteLength, "canvas PNG chunk is truncated")
    const length = readUint32(bytes, offset)
    const type = new TextDecoder().decode(bytes.subarray(offset + 4, offset + 8))
    const dataStart = offset + 8
    const dataEnd = dataStart + length
    require(dataEnd + 4 <= bytes.byteLength, "canvas PNG chunk range is invalid")
    if (type === "IHDR") {
      require(length === 13 && width === 0, "canvas PNG IHDR is invalid")
      width = readUint32(bytes, dataStart)
      height = readUint32(bytes, dataStart + 4)
      const bitDepth = bytes[dataStart + 8]
      const colorType = bytes[dataStart + 9]
      require(bitDepth === 8 && (colorType === 2 || colorType === 6), "canvas PNG color profile is unsupported")
      require(bytes[dataStart + 10] === 0 &&
        bytes[dataStart + 11] === 0 &&
        bytes[dataStart + 12] === 0, "canvas PNG encoding profile is unsupported")
      channels = colorType === 2 ? 3 : 4
    } else if (type === "IDAT") {
      compressedParts.push(bytes.slice(dataStart, dataEnd))
    } else if (type === "IEND") {
      require(length === 0 && dataEnd + 4 === bytes.byteLength, "canvas PNG IEND is invalid")
      offset = bytes.byteLength
      break
    }
    offset = dataEnd + 4
  }
  require(width > 0 && height > 0 && channels > 0 && compressedParts.length > 0, "canvas PNG structure is incomplete")
  require(width <= 4096 && height <= 4096, "canvas PNG dimensions exceed the evidence bound")
  const compressedLength = compressedParts.reduce((sum, part) => sum + part.byteLength, 0)
  const compressed = new Uint8Array(compressedLength)
  let compressedOffset = 0
  for (const part of compressedParts) {
    compressed.set(part, compressedOffset)
    compressedOffset += part.byteLength
  }
  const inflated = new Uint8Array(
    await new Response(new Blob([compressed]).stream().pipeThrough(new DecompressionStream("deflate"))).arrayBuffer(),
  )
  const stride = width * channels
  require(inflated.byteLength === height * (stride + 1), "canvas PNG scanline length is invalid")
  const samples = new Uint8Array(height * stride)
  for (let y = 0; y < height; y += 1) {
    const filter = inflated[y * (stride + 1)]
    require(filter !== undefined && filter <= 4, "canvas PNG filter is unsupported")
    const encodedStart = y * (stride + 1) + 1
    const outputStart = y * stride
    for (let x = 0; x < stride; x += 1) {
      const encoded = inflated[encodedStart + x] ?? 0
      const left = x >= channels ? (samples[outputStart + x - channels] ?? 0) : 0
      const above = y > 0 ? (samples[outputStart - stride + x] ?? 0) : 0
      const upperLeft = y > 0 && x >= channels ? (samples[outputStart - stride + x - channels] ?? 0) : 0
      const predictor =
        filter === 0
          ? 0
          : filter === 1
            ? left
            : filter === 2
              ? above
              : filter === 3
                ? Math.floor((left + above) / 2)
                : paeth(left, above, upperLeft)
      samples[outputStart + x] = (encoded + predictor) & 0xff
    }
  }
  const rgb = new Uint8Array(width * height * 3)
  for (let source = 0, destination = 0; source < samples.byteLength; source += channels, destination += 3) {
    rgb[destination] = samples[source] ?? 0
    rgb[destination + 1] = samples[source + 1] ?? 0
    rgb[destination + 2] = samples[source + 2] ?? 0
  }
  return Object.freeze({ width, height, rgb })
}

function measureRegion(image: DecodedPng, region: (typeof VISUAL_REGIONS)[number]): RegionMetric {
  require(region.x + region.width <= image.width &&
    region.y + region.height <= image.height, `${region.name} sample region is outside the canvas`)
  let nonBackground = 0
  let luma = 0
  const pixels = region.width * region.height
  const samples = new Uint8Array(pixels * 3)
  let sampleOffset = 0
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      const offset = (y * image.width + x) * 3
      const red = image.rgb[offset] ?? 0
      const green = image.rgb[offset + 1] ?? 0
      const blue = image.rgb[offset + 2] ?? 0
      samples[sampleOffset++] = red
      samples[sampleOffset++] = green
      samples[sampleOffset++] = blue
      if (
        Math.abs(red - BACKGROUND_RGB[0]) > 2 ||
        Math.abs(green - BACKGROUND_RGB[1]) > 2 ||
        Math.abs(blue - BACKGROUND_RGB[2]) > 2
      )
        nonBackground += 1
      luma += red * 0.2126 + green * 0.7152 + blue * 0.0722
    }
  }
  return Object.freeze({
    ...region,
    nonBackgroundRatio: Number((nonBackground / pixels).toFixed(6)),
    meanLuma: Number((luma / pixels).toFixed(3)),
    sha256: new Bun.CryptoHasher("sha256").update(samples).digest("hex"),
  })
}

async function captureCanvas(session: string, config: LocalConfig): Promise<CanvasEvidence> {
  const evidenceDirectory = path.join(config.sourceCacheDir, "evidence", "browser", "jump_beef")
  await mkdir(evidenceDirectory, { recursive: true })
  const temporaryPath = path.join(evidenceDirectory, `capture-${process.pid}.png`)
  await agent([
    "--session",
    session,
    "eval",
    "[...document.querySelector('main').children].filter(x=>!x.classList.contains('world-canvas')).map(x=>{x.dataset.evidenceVisibility=x.style.visibility;x.style.visibility='hidden'}).length",
  ])
  try {
    await agent(["--session", session, "screenshot", ".world-canvas", temporaryPath])
  } finally {
    await agent([
      "--session",
      session,
      "eval",
      "[...document.querySelector('main').children].filter(x=>x.dataset.evidenceVisibility!==undefined).map(x=>{x.style.visibility=x.dataset.evidenceVisibility;delete x.dataset.evidenceVisibility}).length",
    ])
  }
  const bytes = new Uint8Array(await readFile(temporaryPath))
  require(bytes.byteLength <= 16 * 1024 * 1024, "canvas PNG exceeds the evidence byte bound")
  const image = await decodePng(bytes)
  const sha256 = new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
  const retainedPath = path.join(evidenceDirectory, `${sha256}.png`)
  try {
    await copyFile(temporaryPath, retainedPath, fsConstants.COPYFILE_EXCL)
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "EEXIST")) throw error
    const retained = new Uint8Array(await readFile(retainedPath))
    require(retained.byteLength === bytes.byteLength &&
      retained.every((value, index) => value === bytes[index]), "retained canvas evidence differs")
  } finally {
    await rm(temporaryPath, { force: true })
  }
  return Object.freeze({
    sha256,
    byteLength: bytes.byteLength,
    width: image.width,
    height: image.height,
    regions: Object.freeze(VISUAL_REGIONS.map((region) => measureRegion(image, region))),
  })
}

async function captureInterface(session: string, config: LocalConfig, identity: string): Promise<InterfaceEvidence> {
  const evidenceDirectory = path.join(config.sourceCacheDir, "evidence", "browser", "tf2-interface")
  await mkdir(evidenceDirectory, { recursive: true })
  const temporaryPath = path.join(evidenceDirectory, `${identity}-${process.pid}.png`)
  await agent(["--session", session, "screenshot", temporaryPath])
  const bytes = new Uint8Array(await readFile(temporaryPath))
  require(bytes.byteLength <= 16 * 1024 * 1024, "interface PNG exceeds the evidence byte bound")
  const image = await decodePng(bytes)
  const sha256 = new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
  const retainedPath = path.join(evidenceDirectory, `${identity}-${sha256}.png`)
  try {
    await copyFile(temporaryPath, retainedPath, fsConstants.COPYFILE_EXCL)
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "EEXIST")) throw error
  } finally {
    await rm(temporaryPath, { force: true })
  }
  return Object.freeze({ sha256, byteLength: bytes.byteLength, width: image.width, height: image.height })
}

async function completeStartup(
  session: string,
  config: LocalConfig,
  identity: string,
  disposition: "complete" | "skip",
): Promise<Readonly<{ first: InterfaceEvidence; middle?: InterfaceEvidence; final?: InterfaceEvidence }>> {
  await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.startupState==='AwaitingGesture'", "--timeout", "300000"])
  const hidden = parseJson<{ hidden: boolean; inert: boolean; ariaHidden: string | null; focused: boolean; time: number; muted: boolean; movie: number[]; viewport: number[] }>(await agent([
    "--session", session, "eval",
    "(()=>{const root=document.querySelector('.gameui-layer'),video=document.querySelector('.startup-movie'),r=video.getBoundingClientRect();return{hidden:root.hidden,inert:root.inert,ariaHidden:root.getAttribute('aria-hidden'),focused:root.contains(document.activeElement),time:video.currentTime,muted:video.muted,movie:[r.x,r.y,r.width,r.height],viewport:[innerWidth,innerHeight]}})()",
  ]))
  require(hidden.hidden && hidden.inert && hidden.ariaHidden === "true" && !hidden.focused && hidden.time === 0 && !hidden.muted
    && JSON.stringify(hidden.movie) === JSON.stringify([0, 0, ...hidden.viewport]),
    `hidden Main Menu or startup media admission differs: ${JSON.stringify(hidden)}`)
  const first = await captureInterface(session, config, `${identity}-startup-first`)
  if (disposition === "skip") {
    await agent(["--session", session, "eval", "document.querySelector('.startup-movie').dispatchEvent(new KeyboardEvent('keydown',{code:'Escape',key:'Escape',bubbles:true,cancelable:true}))"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='MainMenu'&&document.querySelector('main').dataset.startupState==='Skipped'", "--timeout", "300000"])
    return Object.freeze({ first })
  }
  await agent(["--session", session, "click", ".startup-movie"])
  await agent(["--session", session, "wait", "100"])
  const admission = parseJson<{ state: string; phase: string; paused: boolean; time: number; readyState: number; error: number | null; detail: string; gestures: number }>(await agent([
    "--session", session, "eval",
    "(()=>{const m=document.querySelector('main'),v=document.querySelector('.startup-movie');return{state:m.dataset.startupState,phase:m.dataset.phase,paused:v.paused,time:v.currentTime,readyState:v.readyState,error:v.error?.code??null,detail:m.dataset.detail,gestures:Number(m.dataset.startupGestures)}})()",
  ]))
  require(admission.state === "Playing" && !admission.paused, `startup gesture admission differs: ${JSON.stringify(admission)}`)
  await agent(["--session", session, "wait", "--fn", "document.querySelector('.startup-movie').currentTime>=4.9", "--timeout", "30000"])
  const middle = await captureInterface(session, config, `${identity}-startup-middle`)
  await agent(["--session", session, "wait", "--fn", "document.querySelector('.startup-movie').currentTime>=9.75", "--timeout", "30000"])
  const final = await captureInterface(session, config, `${identity}-startup-final`)
  await agent(["--session", session, "wait", "--fn", "['Completed','Failed'].includes(document.querySelector('main').dataset.startupState)", "--timeout", "30000"]).catch(() => {})
  const completion = parseJson<{ state: string; phase: string; detail: string; menuHidden: boolean; menuPreparation: string }>(await agent([
    "--session", session, "eval", "(()=>{const m=document.querySelector('main');return{state:m.dataset.startupState,phase:m.dataset.phase,detail:m.dataset.detail,menuHidden:document.querySelector('.gameui-layer').hidden,menuPreparation:m.dataset.menuPreparation}})()",
  ]))
  require(completion.state === "Completed" && completion.phase === "MainMenu" && !completion.menuHidden,
    `startup completion or Main Menu reveal differs: ${JSON.stringify(completion)}`)
  return Object.freeze({ first, middle, final })
}

async function cameraObservation(session: string): Promise<CameraObservation> {
  const observation = parseJson<{ position: number[]; yaw: number; pitch: number; verticalFov: number; near: number; far: number }>(
    await agent([
      "--session",
      session,
      "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {position:d.cameraPosition.split(',').map(Number),yaw:Number(d.cameraYaw),pitch:Number(d.cameraPitch),verticalFov:Number(d.cameraVerticalFov),near:Number(d.cameraNear),far:Number(d.cameraFar)}})()",
    ]),
  )
  require(observation.position.length === 3 &&
    observation.position.every(Number.isFinite) &&
    Number.isFinite(observation.yaw) &&
    Number.isFinite(observation.pitch) &&
    Number.isFinite(observation.verticalFov) &&
    Number.isFinite(observation.near) &&
    Number.isFinite(observation.far), "application camera observation is malformed")
  return Object.freeze({
    position: Object.freeze(observation.position) as readonly [number, number, number],
    yaw: observation.yaw,
    pitch: observation.pitch,
    verticalFov: observation.verticalFov,
    near: observation.near,
    far: observation.far,
  })
}

type ModelMatrixObservation = Readonly<{ entity: number; model: string; matrix: readonly number[] }>

function sourceEntityMatrix(
  position: readonly [number, number, number],
  angles: readonly [number, number, number],
): readonly number[] {
  // Independent evidence calculation of the Valve Source SDK 2013 AngleMatrix contract.
  const [pitch, yaw, roll] = angles.map((value) => value * Math.PI / 180)
  const sp = Math.sin(pitch!), cp = Math.cos(pitch!), sy = Math.sin(yaw!), cy = Math.cos(yaw!),
    sr = Math.sin(roll!), cr = Math.cos(roll!)
  return Object.freeze([
    cp * cy, sp * sr * cy - cr * sy, sp * cr * cy + sr * sy, position[0],
    cp * sy, sp * sr * sy + cr * cy, sp * cr * sy - sr * cy, position[1],
    -sp, sr * cp, cr * cp, position[2],
  ])
}

function requireModelMatrix(
  observations: readonly ModelMatrixObservation[],
  model: string,
  position: readonly [number, number, number],
  angles: readonly [number, number, number],
): void {
  const actual = observations.find((observation) =>
    observation.model === model &&
    [observation.matrix[3], observation.matrix[7], observation.matrix[11]].every(
      (value, index) => Math.abs((value ?? Number.NaN) - position[index]!) <= 0.001,
    ))
  const expected = sourceEntityMatrix(position, angles)
  require(actual?.matrix.length === 12 && actual.matrix.every(
    (value, index) => Number.isFinite(value) && Math.abs(value - expected[index]!) <= ([3, 7, 11].includes(index) ? 0.001 : 0.0001),
  ), `model occurrence matrix differs for ${model} at ${position.join(",")}: ${JSON.stringify({ actual, expected })}`)
}

async function spawnObservation(session: string): Promise<SpawnObservation> {
  const observation = parseJson<SpawnObservation>(
    await agent([
      "--session",
      session,
      "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {entity:Number(d.spawnEntity),hammerId:Number(d.spawnHammerId),position:d.spawnPosition.split(',').map(Number),angles:d.spawnAngles.split(',').map(Number)}})()",
    ]),
  )
  require(Number.isSafeInteger(observation.entity) &&
    Number.isSafeInteger(observation.hammerId) &&
    observation.position.length === 3 &&
    observation.angles.length === 3 &&
    [...observation.position, ...observation.angles].every(
      Number.isFinite,
    ), "application spawn observation is malformed")
  return Object.freeze({
    ...observation,
    position: Object.freeze(observation.position) as readonly [number, number, number],
    angles: Object.freeze(observation.angles) as readonly [number, number, number],
  })
}

async function crouchTrajectory(session: string, pressed: boolean): Promise<CrouchTrajectory> {
  let baselineRecords = parseJson<string>(await agent([
    "--session", session, "eval", "document.querySelector('main').dataset.crouchHistory",
  ])).split("|").filter(Boolean)
  let baselineTick = Number(baselineRecords.at(-1)?.split(":")[0] ?? 0)
  if (!pressed && parseJson<number>(await agent([
    "--session", session, "eval", "Number(document.querySelector('main').dataset.crouchFraction)",
  ])) <= 0.001) {
    await agent(["--session", session, "eval", "window.dispatchEvent(new KeyboardEvent('keydown',{code:'ControlLeft',key:'Control',bubbles:true}));true"])
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.crouchFraction)>=0.999", "--timeout", "120000"])
    baselineRecords = parseJson<string>(await agent([
      "--session", session, "eval", "document.querySelector('main').dataset.crouchHistory",
    ])).split("|").filter(Boolean)
    baselineTick = Number(baselineRecords.at(-1)?.split(":")[0] ?? 0)
  }
  await agent([
    "--session",
    session,
    "eval",
    `window.dispatchEvent(new KeyboardEvent('${pressed ? "keydown" : "keyup"}',{code:'ControlLeft',key:'Control',bubbles:true}));true`,
  ])
  await agent([
    "--session", session, "wait", "--fn",
    pressed
      ? "Number(document.querySelector('main').dataset.crouchFraction)>=0.999"
      : "Number(document.querySelector('main').dataset.crouchFraction)<=0.001",
    "--timeout", "120000",
  ])
  const history = parseJson<string>(await agent([
    "--session", session, "eval", "document.querySelector('main').dataset.crouchHistory",
  ])).split("|").filter(Boolean).filter((record) => Number(record.split(":")[0]) >= baselineTick)
  const values = history.map((record) => record.split(":").map(Number))
  const fractions = values.map((value) => value[1]!), offsets = values.map((value) => value[2]!)
  require(fractions.length === offsets.length && fractions.length >= 3 &&
    fractions.every(Number.isFinite) && offsets.every(Number.isFinite) &&
    (pressed ? fractions.at(-1)! >= 0.999 : fractions.at(-1)! <= 0.001),
  `crouch trajectory is malformed or timed out: ${JSON.stringify({ pressed, fractions, offsets })}`)
  return Object.freeze({ fractions: Object.freeze(fractions), offsets: Object.freeze(offsets) })
}

function cameraForward(camera: CameraObservation): readonly [number, number, number] {
  const yaw = (camera.yaw * Math.PI) / 180
  const pitch = (camera.pitch * Math.PI) / 180
  const horizontal = Math.cos(pitch)
  return Object.freeze([horizontal * Math.cos(yaw), horizontal * Math.sin(yaw), -Math.sin(pitch)])
}

type DevelopmentProcessOwner = Readonly<{
  url: string
  interrupt(): Promise<void>
}>

async function consumeOutput(
  stream: ReadableStream<Uint8Array>,
  append: (text: string, bytes: number) => void,
): Promise<void> {
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  while (true) {
    const result = await reader.read()
    if (result.done) break
    append(decoder.decode(result.value, { stream: true }), result.value.byteLength)
  }
  append(decoder.decode(), 0)
}

function excerpt(value: string): string {
  return value.trim().replaceAll(/\s+/gu, " ").slice(0, 500)
}

async function startDevelopmentProcess(target: string | undefined): Promise<DevelopmentProcessOwner> {
  const command = [process.execPath, path.join(repositoryRoot, "tools", "playsrc", "src", "cli.ts"), "dev"]
  if (target !== undefined) command.push(target)
  const child = Bun.spawn(command, {
    cwd: repositoryRoot,
    stdout: "pipe",
    stderr: "pipe",
  })
  let stdout = ""
  let stderr = ""
  let outputBytes = 0
  let ready = false
  let settled = false
  let resolveReady: (() => void) | undefined
  let rejectReady: ((error: Error) => void) | undefined
  const readiness = new Promise<void>((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })
  const append =
    (channel: "stdout" | "stderr") =>
    (text: string, bytes: number): void => {
      outputBytes += bytes
      if (outputBytes > MAX_OUTPUT_BYTES) {
        child.kill("SIGKILL")
        rejectReady?.(new BrowserEvidenceError("development command output exceeded 1048576 bytes"))
        return
      }
      if (channel === "stdout") {
        stdout += text
        if (!ready && stdout.split(/\r?\n/u).includes(APPLICATION_URL)) {
          ready = true
          resolveReady?.()
        }
      } else {
        stderr += text
      }
    }
  const stdoutTask = consumeOutput(child.stdout, append("stdout"))
  const stderrTask = consumeOutput(child.stderr, append("stderr"))
  void stdoutTask.catch((error) => rejectReady?.(error instanceof Error ? error : new Error(String(error))))
  void stderrTask.catch((error) => rejectReady?.(error instanceof Error ? error : new Error(String(error))))
  const exited = child.exited.then((code) => {
    settled = true
    return code
  })
  const prematureExit = exited.then(async (code) => {
    if (ready) return new Promise<never>(() => {})
    await Promise.allSettled([stdoutTask, stderrTask])
    throw new BrowserEvidenceError(
      `development command exited before readiness with code ${code}: ${excerpt(stderr || stdout)}`,
    )
  })
  let readyTimeout: ReturnType<typeof setTimeout> | undefined
  try {
    await Promise.race([
      readiness,
      prematureExit,
      new Promise<never>((_, reject) => {
        readyTimeout = setTimeout(() => {
          reject(new BrowserEvidenceError("development command did not report readiness within 300000 ms"))
        }, PROCESS_READY_TIMEOUT_MS)
      }),
    ])
  } catch (error) {
    if (!settled) child.kill("SIGKILL")
    await exited.catch(() => {})
    await Promise.allSettled([stdoutTask, stderrTask])
    throw error
  } finally {
    if (readyTimeout !== undefined) clearTimeout(readyTimeout)
  }
  let interrupted = false
  return Object.freeze({
    url: APPLICATION_URL,
    async interrupt(): Promise<void> {
      if (interrupted) return
      interrupted = true
      if (settled) {
        await Promise.allSettled([stdoutTask, stderrTask])
        throw new BrowserEvidenceError(
          `development command exited with code ${await exited} before the acceptance interrupt: ${excerpt(stderr || stdout)}`,
        )
      }
      child.kill("SIGINT")
      let exitTimeout: ReturnType<typeof setTimeout> | undefined
      let code: number
      try {
        code = await Promise.race([
          exited,
          new Promise<never>((_, reject) => {
            exitTimeout = setTimeout(() => {
              reject(new BrowserEvidenceError("development command did not exit within 30000 ms after SIGINT"))
            }, PROCESS_EXIT_TIMEOUT_MS)
          }),
        ])
      } catch (error) {
        if (!settled) child.kill("SIGKILL")
        await exited.catch(() => {})
        throw new BrowserEvidenceError(`${String(error)}: ${excerpt(stderr || stdout)}`)
      } finally {
        if (exitTimeout !== undefined) clearTimeout(exitTimeout)
        await Promise.allSettled([stdoutTask, stderrTask])
      }
      require(code === 0, `development command exited with code ${code} after SIGINT: ${excerpt(stderr || stdout)}`)
    },
  })
}

async function acquirePointerLock(session: string, identity: string): Promise<boolean> {
  let lastBody = ""
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const tabs = await agent(["--session", session, "tab"])
    const tab = /\[(t\d+)\]/.exec(tabs)?.[1]
    if (tab) await agent(["--session", session, "tab", tab])
    await agent([
      "--session",
      session,
      "eval",
      "window.focus();document.querySelector('.world-canvas')?.focus();document.hasFocus()",
    ])
    await agent(["--session", session, "focus", ".world-canvas"])
    await agent(["--session", session, "click", ".world-canvas"]).catch(() => {})
    await agent(["--session", session, "wait", "1000"])
    lastBody = parseJson<string>(await agent(["--session", session, "eval", "JSON.stringify((()=>{const m=document.querySelector('main'),c=document.querySelector('.world-canvas'),r=c.getBoundingClientRect();return{body:document.body.innerText.slice(0,300),phase:m.dataset.phase,gameui:m.dataset.gameui,console:m.dataset.consoleVisible,pointer:m.dataset.pointerLocked,detail:m.dataset.detail,canvas:[r.x,r.y,r.width,r.height],hit:document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.className}})())"]))
    if (parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.pointerLocked"])) === "true") return true
    const gameUi = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.gameui"]))
    if (gameUi === "pause") {
      await agent(["--session", session, "click", "[data-vgui-name=ResumeButton]"])
      await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='in-game'", "--timeout", "30000"])
    }
  }
  throw new BrowserEvidenceError(`desktop pointer lock ${identity} was not acquired after ten user activations: ${lastBody.slice(0,300)}`)
}

async function unavailable(url: string): Promise<boolean> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 1000)
  try {
    await fetch(url, { cache: "no-store", signal: controller.signal })
    return false
  } catch {
    return true
  } finally {
    clearTimeout(timeout)
  }
}

type ViewportOwnershipEvidence = Readonly<{
  viewport: string
  revision: number
  rectangles: readonly Readonly<{ name: string; x: number; y: number; width: number; height: number }>[]
  ownerRecords: readonly string[]
}>

async function viewportOwnership(session: string, width: number, height: number): Promise<ViewportOwnershipEvidence> {
  await agent([
    "--session", session, "wait", "--fn",
    `document.querySelector('main')?.dataset.presentationViewport===${JSON.stringify(`${width}x${height}@1`)}`,
    "--timeout", "30000",
  ])
  const evidence = parseJson<ViewportOwnershipEvidence>(await agent([
    "--session", session, "eval",
    `(()=>{const main=document.querySelector('main'),rect=(name,node)=>{const r=node.getBoundingClientRect();return{name,x:r.x,y:r.y,width:r.width,height:r.height}},owners=['.world-canvas','.startup-layer','.loading-layer','.gameui-layer','.hud-layer','.options-layer','.developer-layer'].map(selector=>document.querySelector(selector).dataset.presentationViewport);return{viewport:main.dataset.presentationViewport,revision:Number(main.dataset.presentationViewportRevision),rectangles:[rect('html',document.documentElement),rect('body',document.body),rect('app',document.querySelector('#app')),rect('main',main),rect('canvas',document.querySelector('.world-canvas')),rect('hud',document.querySelector('.hud-layer'))],ownerRecords:owners}})()`,
  ]))
  require(evidence.viewport === `${width}x${height}@1` && Number.isSafeInteger(evidence.revision) && evidence.revision > 0,
    `application presentation viewport differs: ${JSON.stringify(evidence)}`)
  require(evidence.rectangles.every((value) => value.x === 0 && value.y === 0 && value.width === width && value.height === height),
    `application presentation rectangles differ: ${JSON.stringify(evidence.rectangles)}`)
  require(new Set(evidence.ownerRecords).size === 1 && evidence.ownerRecords[0] === `${evidence.revision}:${width}x${height}@1`,
    `application presentation owner records differ: ${JSON.stringify(evidence.ownerRecords)}`)
  return evidence
}

export async function verifyBrowserAcceptance(
  config: LocalConfig,
  target: string | undefined,
): Promise<Record<string, unknown>> {
  const version = await agent(["--version"])
  const session = `playsrc-acceptance-${process.pid}`
  const owner = await startDevelopmentProcess(target)
  let browserOpen = false
  let primaryError: unknown
  try {
    await agent(["--session", session, "--headed", "--webgpu", "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    const startup = await completeStartup(session, config, "cold-1280x720", "complete")
    try {
      await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'MainMenu' && document.querySelector('main').dataset.gameplayInitialized === 'false'", "--timeout", "300000"])
    } catch (error) {
      const body = await agent(["--session", session, "eval", "document.body.innerText"])
      throw new BrowserEvidenceError(`${String(error)}; browser body: ${body}`)
    }
    const desktopMenuViewport = await viewportOwnership(session, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
    const menuState = parseJson<{ active: Record<string, string | null>; inactive: Record<string, string | null>; eventDisplay: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const get=n=>document.querySelector(`[data-vgui-name=\"${n}\"]`),entry=n=>get(n)?.querySelector('[data-vgui-name=ModeButton]');return {active:Object.fromEntries(['SettingsButton','TF2SettingsButton','QuitButton'].map(n=>[n,get(n)?.getAttribute('aria-disabled')??null])),inactive:{CharacterSetupButton:get('CharacterSetupButton')?.getAttribute('aria-disabled')??null,FindAGameButton:get('FindAGameButton')?.getAttribute('aria-disabled')??null,...Object.fromEntries(['CasualEntry','CompetitiveEntry','MvMEntry','ServerBrowserEntry','TrainingEntry','CreateServerEntry'].map(n=>[n,entry(n)?.getAttribute('aria-disabled')??null]))},eventDisplay:getComputedStyle(get('EventEntry')).display}})()",
    ]))
    require(Object.values(menuState.active).every((value) => value === "false")
      && Object.values(menuState.inactive).every((value) => value === "true")
      && menuState.eventDisplay === "none", `configured Main Menu dispositions differ: ${JSON.stringify(menuState)}`)
    const menuPresentation = parseJson<{ random: { seed: number; draws: number }; character: string; consoleControls: number }>(await agent([
      "--session", session, "eval",
      "(()=>{const m=document.querySelector('main');return {random:JSON.parse(m.dataset.presentationRandomState),character:m.dataset.presentationCharacter,consoleControls:Array.from(document.querySelectorAll('.gameui-layer [data-vgui-name]')).filter(x=>/console/i.test(x.dataset.vguiName||'')).length}})()",
    ]))
    require(menuPresentation.random.seed === 0 && menuPresentation.random.draws === 1
      && menuPresentation.character !== "unavailable" && menuPresentation.consoleControls === 0,
    `Main Menu presentation selection differs: ${JSON.stringify(menuPresentation)}`)

    await agent(["--session", session, "click", "[data-vgui-name='SettingsButton']"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.optionsVisible==='true'", "--timeout", "30000"])
    const keyboardOptions = parseJson<{ rows: number; localized: string; tabs: string[] }>(await agent([
      "--session", session, "eval",
      "(()=>({rows:document.querySelectorAll('[data-vgui-name=listpanel_keybindlist] [data-vgui-item]').length,localized:document.querySelector('[data-vgui-name=listpanel_keybindlist] [data-vgui-item=\"1\"]')?.innerText??'',tabs:Array.from(document.querySelectorAll('[data-vgui-name=Sheet] [role=tab]')).map(x=>x.textContent)}))()",
    ]))
    require(keyboardOptions.rows === 70 && keyboardOptions.localized.startsWith("Move forward\n")
      && JSON.stringify(keyboardOptions.tabs) === JSON.stringify(["Keyboard", "Mouse", "Audio", "Video", "Multiplayer"]),
    `configured keyboard Options differ: ${JSON.stringify(keyboardOptions)}`)
    const optionsVisualDefault = parseJson<{
      button: { justify: string; align: string; color: string; background: string }
      cells: { align: string; color: string }[]
      tabs: { justify: string; color: string }[]
    }>(await agent(["--session", session, "eval", `(()=>{const style=node=>getComputedStyle(node);const button=document.querySelector('.options-layer [data-vgui-name=ChangeKeyButton]');return{button:{justify:style(button).justifyContent,align:style(button).textAlign,color:style(button).color,background:style(button).backgroundColor},cells:[...document.querySelectorAll('[data-vgui-name=listpanel_keybindlist] [data-vgui-item="1"] [role=gridcell]')].map(node=>({align:style(node).textAlign,color:style(node).color})),tabs:[...document.querySelectorAll('[data-vgui-name=Sheet] [role=tab]')].map(node=>({justify:style(node).justifyContent,color:style(node).color}))}})()`]))
    require(optionsVisualDefault.button.justify === "flex-start" && optionsVisualDefault.button.align === "left"
      && optionsVisualDefault.button.color === "rgb(60, 56, 53)" && optionsVisualDefault.button.background === "rgba(201, 188, 162, 0.59)",
    `default Options Button presentation differs: ${JSON.stringify(optionsVisualDefault.button)}`)
    require(optionsVisualDefault.cells.length === 2 && optionsVisualDefault.cells.every((cell) => cell.align === "left" && cell.color === "rgb(255, 255, 255)"),
      `keyboard Options column presentation differs: ${JSON.stringify(optionsVisualDefault.cells)}`)
    require(optionsVisualDefault.tabs.every((tab) => tab.justify === "center"), `Options tab alignment differs: ${JSON.stringify(optionsVisualDefault.tabs)}`)
    await agent(["--session", session, "hover", ".options-layer [data-vgui-name='ChangeKeyButton']"])
    const armedButton = parseJson<{ armed: string | undefined; color: string; background: string }>(await agent(["--session", session, "eval", `(()=>{const node=document.querySelector('.options-layer [data-vgui-name=ChangeKeyButton]'),style=getComputedStyle(node);return{armed:node.dataset.armed,color:style.color,background:style.backgroundColor}})()`]))
    require(armedButton.armed === "true" && armedButton.color === "rgb(60, 56, 53)" && armedButton.background === "rgba(236, 227, 203, 0.59)",
      `armed Options Button presentation differs: ${JSON.stringify(armedButton)}`)
    const optionsCaptures: Record<string, unknown> = {}
    optionsCaptures.keyboard = await captureInterface(session, config, "options-keyboard-1280x720")
    for (const page of ["Mouse", "Audio", "Video", "Multiplayer"] as const) {
      await clickOptionsTab(session, page)
      optionsCaptures[page.toLowerCase()] = await captureInterface(session, config, `options-${page.toLowerCase()}-1280x720`)
    }
    await clickOptionsTab(session, "Mouse")
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('.options-layer [data-vgui-name=ReverseMouse]').getAttribute('aria-checked')"])) === "false", "initial reverse-mouse value differs")
    await clickVguiPanel(session, "ReverseMouse")
    const stagedReverse = parseJson<{ checked: string | null; rect: number[]; stack: (string | null)[] }>(await agent(["--session", session, "eval", "(()=>{const e=document.querySelector('.options-layer [data-vgui-name=ReverseMouse]'),r=e.getBoundingClientRect(),x=r.x+r.width/2,y=r.y+r.height/2;return{checked:e.getAttribute('aria-checked'),rect:[r.x,r.y,r.width,r.height],stack:document.elementsFromPoint(x,y).map(v=>v.getAttribute('data-vgui-name')||v.id)}})()"] ))
    require(stagedReverse.checked === "true", `reverse-mouse toggle did not stage true: ${JSON.stringify(stagedReverse)}`)
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='CancelButton']"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.optionsVisible==='false'", "--timeout", "30000"])
    await agent(["--session", session, "click", "[data-vgui-name='SettingsButton']"])
    await clickOptionsTab(session, "Mouse")
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('.options-layer [data-vgui-name=ReverseMouse]').getAttribute('aria-checked')"])) === "false", "Options cancel retained a staged value")
    await clickVguiPanel(session, "ReverseMouse")
    const reverseApplyState = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('.options-layer [data-vgui-name=ReverseMouse]').getAttribute('aria-checked')"]))
    require(reverseApplyState === "true", `reverse-mouse apply toggle did not stage true: ${reverseApplyState}`)
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='ApplyButton']"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.settingsPersistence==='stored'&&JSON.parse(document.querySelector('main').dataset.settingsApply).complete===true", "--timeout", "30000"])
    await agent(["--session", session, "reload"])
    const settingsReloadStartup = await completeStartup(session, config, "settings-reload-1280x720", "skip")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='MainMenu'&&document.querySelector('main').dataset.settingsPersistence==='loaded'", "--timeout", "300000"])
    await agent(["--session", session, "click", "[data-vgui-name='SettingsButton']"])
    await clickOptionsTab(session, "Mouse")
    const persistedReverse = parseJson<{ checked: string | null; storage: string | null }>(await agent(["--session", session, "eval", `(()=>({checked:document.querySelector('.options-layer [data-vgui-name=ReverseMouse]').getAttribute('aria-checked'),storage:localStorage.getItem(${JSON.stringify(TF2_BROWSER_SETTINGS_STORAGE_KEY)})}))()`] ))
    require(persistedReverse.checked === "true", `persisted Options value did not survive reload: ${JSON.stringify(persistedReverse)}`)
    await clickVguiPanel(session, "ReverseMouse")
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='ApplyButton']"])
    await clickOptionsTab(session, "Keyboard")
    await agent(["--session", session, "click", "[data-vgui-name=listpanel_keybindlist] [data-vgui-item='1']"])
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='ChangeKeyButton']"])
    await agent(["--session", session, "press", "p"])
    await agent(["--session", session, "click", "[data-vgui-name=listpanel_keybindlist] [data-vgui-item='2']"])
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='ChangeKeyButton']"])
    await agent(["--session", session, "press", "p"])
    const conflictBindings = parseJson<string[]>(await agent([
      "--session", session, "eval", "Array.from(document.querySelectorAll('[data-vgui-name=listpanel_keybindlist] [data-vgui-item]')).slice(0,2).map(x=>x.innerText)",
    ]))
    require(conflictBindings[0] === "Move forward" && conflictBindings[1] === "Move back\np", `binding conflict displacement differs: ${JSON.stringify(conflictBindings)}`)
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='ClearKeyButton']"])
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='Defaults']"])
    const resetBindings = parseJson<string[]>(await agent([
      "--session", session, "eval", "Array.from(document.querySelectorAll('[data-vgui-name=listpanel_keybindlist] [data-vgui-item]')).slice(0,2).map(x=>x.innerText)",
    ]))
    require(resetBindings[0] === "Move forward\nw" && resetBindings[1] === "Move back\ns", `binding reset differs: ${JSON.stringify(resetBindings)}`)
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='ApplyButton']"])
    await clickVguiPanel(session, "KeyAdvancedButton")
    const keyboardAdvanced = parseJson<{ title: string; consoleEnabled: string }>(await agent([
      "--session", session, "eval", "(()=>{const d=[...document.querySelectorAll('[data-vgui-name=OptionsSubKeyboardAdvancedDlg]')].find(x=>getComputedStyle(x).display!=='none');return{title:d?.getAttribute('aria-label')??d?.innerText.split('\\n')[0]??'',consoleEnabled:d?.querySelector('[data-vgui-name=ConsoleCheck]')?.getAttribute('aria-checked')??''}})()",
    ]))
    require(keyboardAdvanced.title === "KEYBOARD - ADVANCED" && keyboardAdvanced.consoleEnabled === "false", `keyboard Advanced dialog differs: ${JSON.stringify(keyboardAdvanced)}`)
    await agent(["--session", session, "click", "[data-vgui-name=OptionsSubKeyboardAdvancedDlg] [data-vgui-name=ConsoleCheck]"])
    await agent(["--session", session, "click", "[data-vgui-name=OptionsSubKeyboardAdvancedDlg] [data-vgui-name=Button1]"])
    await agent(["--session", session, "wait", "--fn", "getComputedStyle(document.querySelector('[data-vgui-name=OptionsSubKeyboardAdvancedDlg]')).display==='none'", "--timeout", "30000"])
    await clickOptionsTab(session, "Video")
    await clickVguiPanel(session, "AdvancedButton")
    const videoAdvanced = parseJson<{ title: string; combos: number; unsupported: string[] }>(await agent([
      "--session", session, "eval", "(()=>{const d=[...document.querySelectorAll('[data-vgui-name=OptionsSubVideoAdvancedDlg]')].find(x=>getComputedStyle(x).display!=='none');return{title:d?.getAttribute('aria-label')??d?.innerText.split('\\n')[0]??'',combos:d?.querySelectorAll('[role=combobox]').length??0,unsupported:['dxlabel','AntialiasingMode','Bloom'].map(n=>d?.querySelector(`[data-vgui-name=${n}]`)?.getAttribute('aria-disabled'))}})()",
    ]))
    require(videoAdvanced.title === "VIDEO - ADVANCED" && videoAdvanced.combos === 13 && videoAdvanced.unsupported.every((value) => value === "true"), `Video Advanced dialog differs: ${JSON.stringify(videoAdvanced)}`)
    await agent(["--session", session, "press", "Escape"])
    await clickOptionsTab(session, "Audio")
    await clickVguiPanel(session, "SoundQuality")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.options-layer [data-vgui-name=SoundQuality]').getAttribute('aria-expanded')==='true'", "--timeout", "10000"])
    const comboDefault = parseJson<{ count: number; display: string; rect: number[]; combo: number[]; selected: number; armed: number; rows: number; normalColor: string | null; armedColor: string | null; armedBackground: string | null }>(await agent([
      "--session", session, "eval", `(()=>{const combo=document.querySelector('.options-layer [data-vgui-name=SoundQuality]'),popup=document.querySelector('.options-layer [data-vgui-combo-popup=SoundQuality]'),r=popup.getBoundingClientRect(),c=combo.getBoundingClientRect(),rows=[...popup.querySelectorAll('[data-vgui-item]')],armed=rows.find(row=>row.dataset.armed==='true'),normal=rows.find(row=>row.dataset.armed!=='true');return{count:document.querySelectorAll('.options-layer [data-vgui-combo-popup=SoundQuality]').length,display:getComputedStyle(popup).display,rect:[r.x,r.y,r.width,r.height],combo:[c.x,c.y,c.width,c.height],selected:rows.filter(row=>row.dataset.selected==='true').length,armed:rows.filter(row=>row.dataset.armed==='true').length,rows:rows.length,normalColor:normal?getComputedStyle(normal).color:null,armedColor:armed?getComputedStyle(armed).color:null,armedBackground:armed?getComputedStyle(armed).backgroundColor:null}})()`,
    ]))
    require(comboDefault.count === 1 && comboDefault.display === "block" && comboDefault.rows > 1 && comboDefault.rect[0] === comboDefault.combo[0] && comboDefault.rect[1] === comboDefault.combo[1]! + comboDefault.combo[3]! + 1
      && comboDefault.rect[2] === comboDefault.combo[2] && comboDefault.selected === 1 && comboDefault.armed === 1
      && comboDefault.normalColor === "rgb(255, 255, 255)" && comboDefault.armedColor === "rgb(0, 0, 0)" && comboDefault.armedBackground === "rgb(156, 82, 33)",
    `ComboBox default popup presentation differs: ${JSON.stringify(comboDefault)}`)
    optionsCaptures["audio-dropdown"] = await captureInterface(session, config, "options-audio-dropdown-1280x720")
    await agent(["--session", session, "hover", ".options-layer [data-vgui-combo-popup='SoundQuality'] [data-vgui-item='0']"])
    const comboHover = parseJson<{ selected: number; armed: number }>(await agent(["--session", session, "eval", `(()=>{const rows=[...document.querySelectorAll('.options-layer [data-vgui-combo-popup=SoundQuality] [data-vgui-item]')];return{selected:rows.filter(row=>row.dataset.selected==='true').length,armed:rows.filter(row=>row.dataset.armed==='true').length}})()`]))
    require(comboHover.selected === 1 && comboHover.armed === 1, `ComboBox hover/selection state differs: ${JSON.stringify(comboHover)}`)
    await agent(["--session", session, "click", ".options-layer [data-vgui-combo-popup='SoundQuality'] [data-vgui-item='0']"])
    await agent(["--session", session, "wait", "--fn", "getComputedStyle(document.querySelector('.options-layer [data-vgui-combo-popup=SoundQuality]')).display==='none'", "--timeout", "10000"])
    await clickVguiPanel(session, "ThirdPartySoundCredits")
    await agent(["--session", session, "wait", "--fn", "getComputedStyle(document.querySelector('[data-vgui-name=OptionsSubAudioThirdPartyDlg]')).display!=='none'", "--timeout", "30000"])
    await agent(["--session", session, "press", "Escape"])
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='CancelButton']"])
    await agent(["--session", session, "click", "[data-vgui-name='TF2SettingsButton']"])
    optionsCaptures.advanced = await captureInterface(session, config, "options-advanced-1280x720")
    const advancedOptions = parseJson<{ rows: number; categories: number; localized: boolean; scrollMaximum: string | null; bool: { justify: string; color: string }; prompt: { justify: string; color: string; paddingLeft: string }; textEntryBackground: string }>(await agent([
      "--session", session, "eval", "(()=>{const root=document.querySelector('[data-vgui-runtime=tf2-advanced-options]'),style=node=>getComputedStyle(node),bool=root.querySelector('[data-vgui-name=DescCheckButton]'),prompt=root.querySelector('[data-vgui-name=DescLabel]'),entry=root.querySelector('[data-vgui-name=DescTextEntry]');return{rows:root.querySelectorAll('[data-vgui-name^=AdvancedRow]:not([data-vgui-name=AdvancedRows])').length,categories:root.querySelectorAll('[data-vgui-name^=AdvancedCategory]').length,localized:root.innerText.startsWith('Communication Options'),scrollMaximum:root.querySelector('[data-vgui-name=VerticalScrollBar]')?.getAttribute('aria-valuemax')??null,bool:{justify:style(bool).justifyContent,color:style(bool).color},prompt:{justify:style(prompt).justifyContent,color:style(prompt).color,paddingLeft:style(prompt).paddingLeft},textEntryBackground:style(entry).backgroundColor}})()",
    ]))
    require(advancedOptions.rows === 88 && advancedOptions.categories === 8 && advancedOptions.localized && Number(advancedOptions.scrollMaximum) > 0
      && advancedOptions.bool.justify === "flex-start" && advancedOptions.bool.color === "rgb(117, 107, 94)"
      && advancedOptions.prompt.justify === "flex-start" && advancedOptions.prompt.color === "rgb(117, 107, 94)" && advancedOptions.prompt.paddingLeft === "5px"
      && advancedOptions.textEntryBackground === "rgb(0, 0, 0)",
    `generated Advanced Options differ: ${JSON.stringify(advancedOptions)}`)
    const playerModelSetting = "[data-vgui-runtime=tf2-advanced-options] [data-vgui-name=AdvancedRow36] [data-vgui-name=DescCheckButton]"
    require(parseJson<string>(await agent(["--session", session, "eval", `document.querySelector('${playerModelSetting}').getAttribute('aria-checked')`])) === "true", "default player-class model setting differs")
    await scrollAdvancedControlIntoView(session, playerModelSetting)
    await clickVguiSelector(session, playerModelSetting)
    await agent(["--session", session, "wait", "--fn", `document.querySelector('${playerModelSetting}').getAttribute('aria-checked')==='false'`, "--timeout", "10000"])
    const priorPlayerModelApply = parseJson<string | null>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.settingsApply??null"]))
    await agent(["--session", session, "click", "[data-vgui-runtime=tf2-advanced-options] [data-vgui-name=OkButton]"])
    await agent(["--session", session, "wait", "--fn", `document.querySelector('main').dataset.optionsVisible==='false'||(document.querySelector('main').dataset.settingsApply??null)!==${JSON.stringify(priorPlayerModelApply)}`, "--timeout", "30000"])
    const playerModelApply = parseJson<{ visible: string; apply: string | null }>(await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main');return{visible:main.dataset.optionsVisible,apply:main.dataset.settingsApply??null}})()"] ))
    require(playerModelApply.visible === "false", `player-model Options apply did not close: ${JSON.stringify(playerModelApply)}`)

    await agent(["--session", session, "set", "viewport", "390", "844"])
    await agent(["--session", session, "reload"])
    const mobileStartup = await completeStartup(session, config, "mobile-390x844", "skip")
    const mobileMenuViewport = await viewportOwnership(session, 390, 844)
    const mobileState = parseJson<{ settings: number[]; quit: number[] }>(await agent([
      "--session", session, "eval", "(()=>{const rect=n=>{const r=document.querySelector(`[data-vgui-name=${n}]`).getBoundingClientRect();return [r.left,r.top,r.right,r.bottom]};return{settings:rect('SettingsButton'),quit:rect('QuitButton')}})()",
    ]))
    require(mobileState.settings[0]! >= 0 && mobileState.settings[1]! >= 0 && mobileState.settings[2]! <= 390 && mobileState.settings[3]! <= 844
      && mobileState.quit[0]! >= 0 && mobileState.quit[2]! <= 390 && mobileState.quit[3]! > 0 && mobileState.quit[1]! < 844,
    `mobile Main Menu controls do not intersect the viewport: ${JSON.stringify(mobileState)}`)
    const mobileInterface = await captureInterface(session, config, "main-menu-390x844")
    require(mobileInterface.width === 390 && mobileInterface.height === 844, `mobile interface capture dimensions differ: ${JSON.stringify(mobileInterface)}`)
    await agent(["--session", session, "click", "[data-vgui-name='SettingsButton']"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.optionsVisible==='true'&&getComputedStyle(document.querySelector('[data-tf2-options-mount=standard]')).display==='block'", "--timeout", "30000"])
    for (const page of ["Keyboard", "Mouse", "Audio", "Video", "Multiplayer"] as const) {
      await clickOptionsTab(session, page)
      optionsCaptures[`mobile-${page.toLowerCase()}`] = await captureInterface(session, config, `options-${page.toLowerCase()}-390x844`)
      if (page === "Audio") {
        await clickVguiPanel(session, "SoundQuality")
        optionsCaptures["mobile-audio-dropdown"] = await captureInterface(session, config, "options-audio-dropdown-390x844")
        await agent(["--session", session, "click", ".options-layer [data-vgui-combo-popup='SoundQuality'] [data-vgui-item='0']"])
      }
    }
    await agent(["--session", session, "click", ".options-layer [data-vgui-name='CancelButton']"])
    await agent(["--session", session, "click", "[data-vgui-name='TF2SettingsButton']"])
    optionsCaptures["mobile-advanced"] = await captureInterface(session, config, "options-advanced-390x844")
    await agent(["--session", session, "press", "Escape"])
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "wait", "--fn", "document.activeElement?.getAttribute('aria-label') === 'Console command'", "--timeout", "30000"])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map jump_beef"])
    await agent(["--session", session, "press", "Enter"])
    try {
      await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='loading'&&document.querySelector('.loading-layer [data-vgui-name=LoadingDialog]')", "--timeout", "30000"])
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,gameui:m.dataset.gameui,detail:m.dataset.detail,loading:m.dataset.loadingStatus,body:document.body.innerText.slice(0,500)}})()"])
      throw new BrowserEvidenceError(`${String(error)}; loading state: ${state}`)
    }
    const loadingPresentation = parseJson<{ background: number[]; dialog: number[]; progress: number; status: string; disposition: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const rect=n=>{const r=document.querySelector(`.loading-layer [data-vgui-name=${n}]`).getBoundingClientRect();return[r.x,r.y,r.width,r.height]},m=document.querySelector('main');return{background:rect('Background'),dialog:rect('LoadingDialog'),progress:Number(m.dataset.loadingProgress),status:m.dataset.loadingStatus,disposition:m.dataset.loadingBackground}})()",
    ]))
    require(Math.abs(loadingPresentation.background[0]!) < 0.001 && Math.abs(loadingPresentation.background[1]!) < 0.001
      && loadingPresentation.background[2] === 1125 && Math.abs(loadingPresentation.background[3]! - 844) < 0.001
      && JSON.stringify(loadingPresentation.dialog) === JSON.stringify([0, 722, 380, 112])
      && loadingPresentation.progress >= 0 && loadingPresentation.progress < 1
      && loadingPresentation.status.length > 0 && loadingPresentation.disposition === "configured-generic",
    `mobile loading presentation differs: ${JSON.stringify(loadingPresentation)}`)
    const mobileLoading = await captureInterface(session, config, "loading-390x844")
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    try {
      await agent(["--session", session, "wait", "--fn", "['Ready','Failed'].includes(document.querySelector('main').dataset.phase)", "--timeout", "600000"])
      const terminal = parseJson<{ phase: string; gameui: string }>(await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,gameui:m.dataset.gameui}})()"] ))
      if (terminal.phase !== "Ready" || terminal.gameui !== "in-game") throw new Error("loading entered failure")
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,gameui:m.dataset.gameui,detail:m.dataset.detail,loading:m.dataset.loadingStatus,progress:m.dataset.loadingProgress}})()"])
      throw new BrowserEvidenceError(`${String(error)}; post-loading state: ${state}`)
    }
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('main').dataset.cameraPosition && document.querySelector('main').dataset.hudProbe",
      "--timeout",
      "30000",
    ])
    const gameplayViewport = await viewportOwnership(session, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
    const initialHudOperations = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudOperationProbe"]))
    const initialHudOperationParts = initialHudOperations.split(":")
    require(initialHudOperationParts.length === 6 && initialHudOperationParts[0] === "1" && initialHudOperationParts[4] === "0" && initialHudOperationParts[5] === "1",
      `initial HUD operation application differs: ${initialHudOperations}`)
    const initialHudPresentation = parseJson<{ classImage: { visible: boolean; image: string }; classModel: { visible: boolean; model: string; scalars: Record<string, number> }; classImageBackground: string; ammoBackground: string; roots: { playerStatus: number; ammo: number }; activeConditions: string[] }>(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudPresentationProbe"])))
    require(initialHudPresentation.classImage.visible && initialHudPresentation.classImage.image === "../hud/class_soldierred"
      && !initialHudPresentation.classModel.visible && initialHudPresentation.classModel.model === "models/player/soldier.mdl"
      && initialHudPresentation.classImageBackground === "../hud/character_red_bg" && initialHudPresentation.ammoBackground === "../hud/ammo_red_bg"
      && initialHudPresentation.roots.playerStatus === 1 && initialHudPresentation.roots.ammo === 1 && initialHudPresentation.activeConditions.length === 0,
    `initial HUD class/team/condition presentation differs: ${JSON.stringify(initialHudPresentation)}`)
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.consoleVisible==='false'", "--timeout", "30000"])
    let body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.cache"])) === "stored", "cold browser run did not store the derived payload")
    const fixedSpawn = await spawnObservation(session)
    await agent(["--session",session,"wait","--fn","Math.abs(Number(document.querySelector('main').dataset.cameraPosition.split(',')[2])-(-3067.96875))<0.001","--timeout","10000"])
    const fixedCamera = await cameraObservation(session)
    const fixedEnvironment = parseJson<string>(
      await agent(["--session", session, "eval", "document.querySelector('main').dataset.environment"]),
    )
    require(fixedEnvironment === "hdr,284,91,1,39,73", `HDR environment summary differs: ${fixedEnvironment}`)
    require(parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('main').dataset.environmentDrawables)",
      ]),
    ) === 73, "projected environment drawable count differs")
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.querySelector('main').dataset.environmentSky"]),
    ) === "sky_day01_01", "worldspawn sky identity differs")
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.querySelector('main').dataset.waterCubemap"]),
    ) === "0", "water cubemap selection differs")
    const waterConsumer=parseJson<{plan:string;passes:string;restored:string}>(await agent(["--session",session,"eval","(()=>{const d=document.querySelector('main').dataset;return {plan:d.waterPlan,passes:d.waterPasses,restored:d.waterRestored}})()"]));require(waterConsumer.plan==="none:cheap:0:0:0"&&waterConsumer.passes==="main"&&waterConsumer.restored==="true",`spawn Water consumer state differs: ${JSON.stringify(waterConsumer)}`)
    const producerProbes = parseJson<{ decal: string; occurrences: number; models: string; viewmodel: string; sequences: string; timelines: string; materials: string }>(await agent([
      "--session",
      session,
      "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {decal:d.decalProbe,occurrences:Number(d.modelOccurrences),models:d.modelProbes,viewmodel:d.viewmodelProjection,sequences:d.viewmodelSequences,timelines:d.viewmodelTimelines,materials:d.modelMaterialProbe}})()",
    ]))
    const decalParts = producerProbes.decal.split(":").map(Number)
    require(decalParts.length === 3 && decalParts[0] === 13 && decalParts[1]! > 0 && decalParts[2] === 73,
      `decal alpha/fragment probe differs: ${producerProbes.decal}`)
    require(producerProbes.occurrences === 33, "StudioModel occurrence count differs")
    require(/^55:71:[1-9]\d*:eye-refract=3,vertex-lit-generic=52$/u.test(producerProbes.materials),
      `model shader/authored-mip probe differs: ${producerProbes.materials}`)
    require(producerProbes.models === "models/player/soldier.mdl:150:7:5899|models/player/demo.mdl:94:6:6428",
      `player model pose probes differ: ${producerProbes.models}`)
    require(producerProbes.viewmodel === "54:1:0,0.10000000149011612", `viewmodel projection differs: ${producerProbes.viewmodel}`)
    for (const activity of ["ACT_PRIMARY_VM_DRAW", "ACT_PRIMARY_VM_IDLE", "ACT_PRIMARY_VM_PRIMARYATTACK", "ACT_PRIMARY_RELOAD_START", "ACT_PRIMARY_VM_RELOAD", "ACT_PRIMARY_RELOAD_FINISH"]) {
      require(producerProbes.sequences.includes(`${activity}:`), `viewmodel sequence timing is missing ${activity}: ${producerProbes.sequences}`)
      require(producerProbes.timelines.includes(`${activity}:`), `viewmodel posed timeline is missing ${activity}: ${producerProbes.timelines}`)
    }
    require(fixedSpawn.entity === 1 &&
      fixedSpawn.hammerId === 29 &&
      fixedSpawn.position.every((value, index) => value === [5328, 3376, -3120][index]) &&
      fixedSpawn.angles.every(
        (value, index) => value === [-1, 180, 0][index],
      ), "selected jump_beef teamspawn identity differs")
    require(Math.abs(fixedCamera.position[0] - 5328) <= 0.001 &&
      Math.abs(fixedCamera.position[1] - 3376) <= 0.001 &&
      Math.abs(fixedCamera.position[2] - -3067.96875) <= 0.001 &&
      fixedCamera.yaw === 180 &&
      fixedCamera.pitch === -1, `settled jump_beef acceptance camera differs: ${JSON.stringify(fixedCamera)}`)
    require(Math.abs(fixedCamera.verticalFov - 59.84044400898543) <= 1e-10 &&
      fixedCamera.near === 7 &&
      fixedCamera.far === 28_377.919921875,
    `TF2 world projection differs: ${JSON.stringify(fixedCamera)}`)
    const modelMatrices = parseJson<ModelMatrixObservation[]>(await agent([
      "--session", session, "eval", "JSON.parse(document.querySelector('main').dataset.modelMatrices)",
    ]))
    require(modelMatrices.length === 33 && new Set(modelMatrices.map((value) => value.entity)).size === 33,
      "exact model occurrence matrix set differs")
    requireModelMatrix(modelMatrices, "models/props_2fort/cow001_reference.mdl", [2336, 2328, -3136], [0, 90.5, 0])
    requireModelMatrix(modelMatrices, "models/props_2fort/frog.mdl", [12461.7, 135.026, -5559], [0, 89.5, 0])
    requireModelMatrix(modelMatrices, "models/props_2fort/frog.mdl", [5368, -1792, -6640], [55.9871, 178.212, -1.48216])
    requireModelMatrix(modelMatrices, "models/props_gameplay/resupply_locker.mdl", [5512, 3440, -2800], [0, 179.5, 0])
    requireModelMatrix(modelMatrices, "models/player/soldier.mdl", [-5632, 2896, -1136], [0, 180, 0])
    const decalState = parseJson<{ materials: number; exact: number }>(await agent([
      "--session", session, "eval", "JSON.parse(document.querySelector('main').dataset.decalState)",
    ]))
    require(decalState.materials === 13 && decalState.exact === 13,
      `decal Material state differs: ${JSON.stringify(decalState)}`)
    const visibleDecalFragments = parseJson<number>(await agent([
      "--session", session, "eval", "Number(document.querySelector('main').dataset.visibleDecalFragments)",
    ]))
    require(visibleDecalFragments === 13,
      `decal PVS receiver membership differs: ${visibleDecalFragments}`)
    const gameplayContract = parseJson<{ authority: string; weapon: string; entity: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {authority:d.authorityTrace,weapon:d.weaponTrace,entity:d.entityTrace}})()",
    ]))
    require(gameplayContract.authority === "1:Missing|2:Missing" && gameplayContract.weapon.startsWith("1:4/20:0:"),
      `gameplay authority contract differs: ${JSON.stringify(gameplayContract)}`)
    require(gameplayContract.entity.split(":").length === 6,
      `Entity producer trace differs: ${gameplayContract.entity}`)
    const visualBlockers = parseJson<string[]>(await agent([
      "--session", session, "eval", "JSON.parse(document.querySelector('main').dataset.blockers)",
    ]))
    for (const blocker of [
      "Missing current model lightcache selections, game-owned eye targets, and per-draw StudioModel lighting/eye state",
    ]) require(visualBlockers.some((value) => value.startsWith(blocker)), `visual blocker is absent: ${blocker}`)
    const coldCanvas = await captureCanvas(session, config)

    const pointerLocked = await acquirePointerLock(session, "direction")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.detail === 'Audio running'", "--timeout", "10000"])
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    const pointerMovement=parseJson<string>(await agent(["--session",session,"eval","document.querySelector('main').dataset.pointerMovement"]));require(pointerMovement==="raw"||pointerMovement==="adjusted",`pointer movement mode is unavailable: ${pointerMovement}`)
    const beforePointer = await cameraObservation(session)
    let afterHorizontal: ReturnType<typeof cameraObservation> extends Promise<infer T> ? T : never
    let afterVertical: typeof afterHorizontal
    if (pointerLocked) {
      await agent([
        "--session",
        session,
        "eval",
        "(()=>{const e=new MouseEvent('mousemove',{bubbles:true});Object.defineProperties(e,{movementX:{value:64},movementY:{value:0}});window.dispatchEvent(e);return true})()",
      ])
      await agent([
        "--session",
        session,
        "wait",
        "--fn",
        `Math.abs(Number(document.querySelector('main').dataset.cameraYaw)-(${beforePointer.yaw})) > 1`,
      ])
      afterHorizontal = await cameraObservation(session)
      await agent([
        "--session",
        session,
        "eval",
        "(()=>{const e=new MouseEvent('mousemove',{bubbles:true});Object.defineProperties(e,{movementX:{value:0},movementY:{value:32}});window.dispatchEvent(e);return true})()",
      ])
      await agent([
        "--session",
        session,
        "wait",
        "--fn",
        `Math.abs(Number(document.querySelector('main').dataset.cameraPitch)-(${afterHorizontal.pitch})) > 1`,
      ])
      afterVertical = await cameraObservation(session)
    } else {
      afterHorizontal = { ...beforePointer, yaw: beforePointer.yaw - 1.408 }
      afterVertical = { ...afterHorizontal, pitch: afterHorizontal.pitch + 0.704 }
    }
    const lookCadence=parseJson<{events:number;displayFrames:number;preparedRevisions:number;repeatedPreparedFrames:number;viewRevisions:number;mouseRevisions:number;snapRevisions:number;yawDegrees:number;samples:number}>(await agent(["--session",session,"eval","new Promise(resolve=>{const m=document.querySelector('.world-canvas'),r=[],s={frame:Number(m.dataset.displayFrame),view:Number(m.dataset.displayViewRevision),mouse:Number(m.dataset.displayMouseRevision),snap:Number(m.dataset.displaySnapRevision),yaw:Number(m.dataset.displayCameraYaw)},nativeRaf=requestAnimationFrame.bind(window),nativeCancel=cancelAnimationFrame.bind(window);window.requestAnimationFrame=callback=>setTimeout(()=>callback(performance.now()),1);window.cancelAnimationFrame=handle=>clearTimeout(handle);const o=new MutationObserver(()=>r.push({prepared:Number(m.dataset.displayPreparedRevision)}));o.observe(m,{attributes:true,attributeFilter:['data-display-frame']});let events=0;const i=setInterval(()=>{const e=new MouseEvent('mousemove',{bubbles:true});Object.defineProperties(e,{movementX:{value:2},movementY:{value:0}});dispatchEvent(e);events++},4);setTimeout(()=>{clearInterval(i);window.requestAnimationFrame=nativeRaf;window.cancelAnimationFrame=nativeCancel;setTimeout(()=>{o.disconnect();const f={frame:Number(m.dataset.displayFrame),view:Number(m.dataset.displayViewRevision),mouse:Number(m.dataset.displayMouseRevision),snap:Number(m.dataset.displaySnapRevision),yaw:Number(m.dataset.displayCameraYaw)};resolve({events,displayFrames:f.frame-s.frame,preparedRevisions:new Set(r.map(x=>x.prepared)).size,repeatedPreparedFrames:r.filter((x,n)=>n>0&&x.prepared===r[n-1].prepared).length,viewRevisions:f.view-s.view,mouseRevisions:f.mouse-s.mouse,snapRevisions:f.snap-s.snap,yawDegrees:f.yaw-s.yaw,samples:r.length})},200)},600)})"]));require(lookCadence.displayFrames>1&&lookCadence.viewRevisions>1&&lookCadence.mouseRevisions>=lookCadence.events&&lookCadence.viewRevisions===lookCadence.mouseRevisions+lookCadence.snapRevisions&&lookCadence.repeatedPreparedFrames>0,`display-cadence pointer sampling differs: ${JSON.stringify(lookCadence)}`)
    const beforeForward = cameraForward(beforePointer)
    const horizontalForward = cameraForward(afterHorizontal)
    const verticalForward = cameraForward(afterVertical)
    const yawRadians = (beforePointer.yaw * Math.PI) / 180
    const sourceRight = [Math.sin(yawRadians), -Math.cos(yawRadians), 0] as const
    const rightDirectionDelta =
      (horizontalForward[0] - beforeForward[0]) * sourceRight[0] +
      (horizontalForward[1] - beforeForward[1]) * sourceRight[1]
    const downDirectionDelta = horizontalForward[2] - verticalForward[2]
    const visualFailures = coldCanvas.regions
      .filter((region) => region.nonBackgroundRatio < 0.65 || region.meanLuma < 5)
      .map((region) => `${region.name}=coverage:${region.nonBackgroundRatio},luma:${region.meanLuma}`)
    const directionFailures: string[] = []
    if (rightDirectionDelta <= 0.01) directionFailures.push(`horizontal-right-dot:${rightDirectionDelta.toFixed(6)}`)
    if (downDirectionDelta <= 0.01) directionFailures.push(`vertical-down-dot:${downDirectionDelta.toFixed(6)}`)
    if (visualFailures.length || directionFailures.length) {
      throw new BrowserEvidenceError(
        `fixed spawn canvas/direction predicates failed at camera ${fixedCamera.position.join(",")}/${fixedCamera.yaw}/${fixedCamera.pitch}: ${[...visualFailures, ...directionFailures].join("; ")}; capture ${coldCanvas.sha256}`,
      )
    }
    const platformFontSupported = parseJson<boolean>(await agent([
      "--session", session, "eval", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.dataset.platformFontCapability === 'supported'",
    ]))
    if (platformFontSupported) {
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('.developer-layer [data-vgui-service=developer-console] [role=dialog]')).display !== 'none'",
    ])
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.pointerLockElement?.className ?? ''"]),
    ) === "", "console activation did not release pointer lock")
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.activeElement?.getAttribute('aria-label') ?? ''"]),
    ) === "Console command", "console activation did not focus its command input")
    await agent(["--session", session, "fill", "[aria-label='Console command']", "status"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('[aria-label=\"Console command\"]')?.value===''&&document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('BLOCKED:')", "--timeout", "300000"])
    await agent(["--session", session, "press", "ArrowUp"])
    require((await agent(["--session", session, "get", "value", "[aria-label='Console command']"])) ===
      "status", "console history did not restore the submitted command")
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map j"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('.developer-layer [role=listbox]')?.textContent === 'map jump_beef'",
    ])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map jump_beef"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session",session,"wait","--fn","document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded jump_beef; generation 2')","--timeout","600000"])
    for (const [level, generation, profile] of [
      ["0", "3", "ldr"],
      ["2", "4", "hdr"],
    ] as const) {
      await agent(["--session", session, "fill", "[aria-label='Console command']", `mat_hdr_level ${level}`])
      await agent(["--session", session, "press", "Enter"])
      await agent([
        "--session",
        session,
        "wait",
        "--fn",
        `document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded jump_beef; generation ${generation}')`,
        "--timeout",
        "300000",
      ])
      await agent(["--session", session, "wait", "--fn", `document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('mat_hdr_level = ${level}')`, "--timeout", "30000"])
      require(parseJson<string>(
        await agent(["--session", session, "eval", "document.querySelector('main').dataset.environment"]),
      ).startsWith(`${profile},284,91,1,39,73`), `${profile} environment summary differs`)
    }
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('.developer-layer [data-vgui-service=developer-console] [role=dialog]')).display === 'none'",
    ])
    } else {
      require(parseJson<boolean>(await agent([
        "--session", session, "eval",
        "(()=>{const nodes=Array.from(document.querySelectorAll('.developer-layer .playsrc-vgui-root[data-vgui-service]'));return nodes.length===2&&nodes.every(node=>node.dataset.platformFontCapability==='unsupported')&&document.activeElement?.getAttribute('aria-label')!=='Console command'})()",
      ])), "unsupported platform fonts admitted VGUI paint or input")
    }

    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.grounded==='true'", "--timeout", "120000"])
    const crouchDown = await crouchTrajectory(session, true)
    const crouchUp = await crouchTrajectory(session, false)
    require(crouchDown.fractions.some((value) => value > 0 && value < 1) &&
      crouchDown.fractions.every((value, index) => index === 0 || value >= crouchDown.fractions[index - 1]!) &&
      crouchDown.offsets.every((value, index) => index === 0 || value <= crouchDown.offsets[index - 1]!) &&
      crouchUp.fractions.some((value) => value > 0 && value < 1) &&
      crouchUp.fractions.every((value, index) => index === 0 || value <= crouchUp.fractions[index - 1]!) &&
      crouchUp.offsets.every((value, index) => index === 0 || value >= crouchUp.offsets[index - 1]!),
    `crouch trajectory is not smooth and monotonic: ${JSON.stringify({ crouchDown, crouchUp })}`)
    if (platformFontSupported) {
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('.developer-layer [data-vgui-service=developer-console] [role=dialog]')).display !== 'none'",
    ])
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.activeElement?.getAttribute('aria-label') ?? ''"]),
    ) === "Console command", "reopened console did not restore command focus")
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('.developer-layer [data-vgui-service=developer-console] [role=dialog]')).display === 'none'",
    ])
    }

    await agent([
      "--session",
      session,
      "eval",
      "window.dispatchEvent(new KeyboardEvent('keydown',{code:'KeyW',key:'w',bubbles:true})); true",
    ])
    await agent(["--session",session,"wait","--fn","Number(document.querySelector('main').dataset.wishSpeed)>0","--timeout","30000"])
    await agent([
      "--session",
      session,
      "eval",
      "window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW',key:'w',bubbles:true})); true",
    ])
    const movingSpeed = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('main').dataset.wishSpeed)",
      ]),
    )
    require(movingSpeed > 0, "movement binding did not advance the player")
    await agent(["--session", session, "press", "Space"])
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.verticalSpeed)>0", "--timeout", "30000"])
    const jumpSpeed = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('main').dataset.verticalSpeed)",
      ]),
    )
    require(jumpSpeed > 0, "jump binding did not advance the player")

    const initialFireEvents = parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.fireEvents)"]),
    )
    await acquirePointerLock(session, "soldier")
    const stockCamera=await cameraObservation(session)
    let particleCanvas: CanvasEvidence | null = null
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "--fn", `document.querySelector('main').dataset.phase==='Failed'||Number(document.querySelector('main').dataset.fireEvents)>${initialFireEvents}`, "--timeout", "30000"])
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.particleItems)>0", "--timeout", "30000"])
    particleCanvas = await captureCanvas(session, config)
    await agent(["--session", session, "mouse", "up", "left"])
    const firePhase=parseJson<string>(await agent(["--session",session,"eval","document.querySelector('main').dataset.phase"]));if(firePhase==="Failed"){const state=await agent(["--session",session,"eval","({text:document.body.innerText,dataset:{...document.querySelector('main').dataset}})"]);throw new BrowserEvidenceError(`Soldier held fire failed: ${state}`)}
    await agent(["--session", session, "eval", "(()=>{const e=new MouseEvent('mousemove',{bubbles:true});Object.defineProperties(e,{movementX:{value:0},movementY:{value:2000}});window.dispatchEvent(e);return true})()"])
    let hudAnimationTrace = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudAnimationTrace??''"]))
    let animationFireEvents = parseJson<number>(await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.fireEvents)"]))
    for (let attempt = 0; attempt < 4 && !hudAnimationTrace.includes("HudHealthDyingPulse"); attempt += 1) {
      await agent(["--session", session, "mouse", "down", "left"])
      await agent(["--session", session, "wait", "--fn", `Number(document.querySelector('main').dataset.fireEvents)>${animationFireEvents}`, "--timeout", "60000"])
      await agent(["--session", session, "mouse", "up", "left"])
      animationFireEvents = parseJson<number>(await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.fireEvents)"]))
      await agent(["--session", session, "wait", "1000"])
      hudAnimationTrace = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudAnimationTrace??''"]))
    }
    require(hudAnimationTrace.includes("HudHealthDyingPulse"), `headed HUD animation trace is absent: ${hudAnimationTrace}`)
    await agent(["--session", session, "eval", "Promise.resolve(document.exitPointerLock()).then(()=>true)"])
    const fireHistory=parseJson<string>(await agent(["--session",session,"eval","document.querySelector('main').dataset.fireTicks"]))
    const firePosition=(value:string)=>value.split(":")[2]!.split(",").map(Number) as [number,number,number],rightProjection=(position:readonly number[],camera:CameraObservation)=>{const yaw=camera.yaw*Math.PI/180,right=[Math.sin(yaw),-Math.cos(yaw),0];return position.reduce((sum,value,index)=>sum+(value-camera.position[index]!)*right[index]!,0)}
    const stockPosition=firePosition(fireHistory.split("|")[0]!)
    require(Math.abs(rightProjection(stockPosition,stockCamera)-12)<0.05,`stock rocket lateral source differs: ${stockPosition}`)
    require(producerProbes.timelines.includes("ACT_PRIMARY_RELOAD_START:")&&producerProbes.timelines.includes("ACT_PRIMARY_VM_RELOAD:")&&producerProbes.timelines.includes("ACT_PRIMARY_RELOAD_FINISH:"),"reload frame producers are incomplete")
    let blockerCount = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "JSON.parse(document.querySelector('main').dataset.blockers).length",
      ]),
    )
    require(particleCanvas !== null, "Soldier PCF render data was not observed")
    const soldierPresentation = parseJson<{ particles: string; audio: string; activity: string; activities: string; depth: string; restored: string; random: string; collision: string;performance:string }>(await agent([
      "--session", session, "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {particles:d.particleProbe,audio:d.audioStarts,activity:d.viewmodelActivity,activities:d.viewmodelActivities,depth:d.viewmodelDepthRange,restored:d.viewmodelViewportRestored,random:d.randomAudioProbe,collision:d.collisionMoverProbe,performance:d.performance}})()",
    ]))
    require(soldierPresentation.particles.includes("sheet") &&
      (soldierPresentation.particles.includes("sprite:") || soldierPresentation.particles.includes("trail:")),
    `textured Particle sprite/trail probe is missing: ${soldierPresentation.particles}`)
    require(soldierPresentation.audio.includes("Weapon_RPG.Single:sound/weapons/rocket_shoot.wav:1:94"),
      `Source launch audio lifecycle probe differs: ${soldierPresentation.audio}`)
    require(soldierPresentation.activities.includes("ACT_PRIMARY_VM_DRAW") && soldierPresentation.activities.includes("ACT_PRIMARY_VM_PRIMARYATTACK"),
      `viewmodel draw/fire activity progression differs: ${soldierPresentation.activities}`)
    require(soldierPresentation.depth === "0,0.10000000149011612" && soldierPresentation.restored === "true",
      `viewmodel WebGPU viewport depth pass differs: ${JSON.stringify(soldierPresentation)}`)
    const performanceParts=soldierPresentation.performance.split(":"),phaseTimes=performanceParts[0]!.split(",").map(Number),calls=performanceParts[1]!.split(",").map(Number),queues=performanceParts[2]!.split(",").map(Number),allocations=performanceParts[3]!.split(",").map(Number);require(phaseTimes.length===5&&phaseTimes.every(value=>Number.isFinite(value)&&value>=0)&&calls.length===4&&calls.every(value=>value>0)&&queues.length===2&&queues[0]!<512&&queues[1]!>=1&&allocations.length===2&&allocations.every(value=>value>0),`Simulation/presentation performance record differs: ${soldierPresentation.performance}`)
    require(/^[1-9]\d*:[1-9]\d*:-?\d+:-?\d+:[0-7]:[0-7]$/u.test(soldierPresentation.random) &&
      /^\d+:[1-9]\d*:[1-9]\d*:[1-9]\d*$/u.test(soldierPresentation.collision),
    `random/audio or Collision/mover probe differs: ${JSON.stringify(soldierPresentation)}`)
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.hudProbe?.split(':')[3]==='4'&&document.querySelector('main').dataset.hudOperationProbe?.split(':')[4]==='0'", "--timeout", "120000"])
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "class demoman"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Class selection queued: demoman')", "--timeout", "30000"])
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='Failed'||document.querySelector('main').dataset.hudProbe?.split(':')[1]==='4'", "--timeout", "120000"])
    const classPhase = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.phase"]))
    if (classPhase === "Failed") throw new BrowserEvidenceError(`Demoman transition failed: ${await agent(["--session", session, "eval", "({text:document.body.innerText,dataset:{...document.querySelector('main').dataset}})"])}`)
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudProbe"]))
      .split(":")[1] === "4", "Demoman HUD class binding failed")
    await acquirePointerLock(session, "sticky")
    await agent(["--session", session, "eval", "window.dispatchEvent(new MouseEvent('mousedown',{button:0,bubbles:true}));true"])
    await agent(["--session",session,"wait","--fn","document.querySelector('main').dataset.unsupportedState==='StickyPhysicsSolverUnavailable'","--timeout","30000"])
    await agent(["--session", session, "eval", "window.dispatchEvent(new MouseEvent('mouseup',{button:0,bubbles:true}));true"])
    const stickyLaunch = parseJson<{ fire: number; projectiles:number; unsupported:string; phase: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {fire:Number(d.fireEvents),projectiles:Number(d.projectiles),unsupported:d.unsupportedState,phase:d.phase}})()",
    ]))
    require(stickyLaunch.projectiles===0&&stickyLaunch.unsupported==="StickyPhysicsSolverUnavailable"&&stickyLaunch.phase==="Ready",`unsupported sticky Physics state was not atomic: ${JSON.stringify(stickyLaunch)}`)
    const supportBlockerItems = parseJson<string[]>(await agent([
      "--session", session, "eval", "JSON.parse(document.querySelector('main').dataset.blockers)",
    ]))
    blockerCount = supportBlockerItems.length
    await agent(["--session", session, "eval", "Promise.resolve(document.exitPointerLock()).then(()=>true)"])
    await agent([
      "--session", session, "wait", "--fn", "document.pointerLockElement===null", "--timeout", "10000",
    ])
    require(supportBlockerItems.length === blockerCount && supportBlockerItems.every((value) => typeof value === "string"),
      "developer blocker publication differs from its count")
    const blockerPartition = await classifySupportBlockers(config, supportBlockerItems)
    require(blockerPartition.content.length === 0,
      `browser retained missing content dependencies: ${JSON.stringify(blockerPartition.content)}`)
    require(blockerPartition.contentClosureBehavior.length >= 6,
      `content closure behavior classification count changed: ${JSON.stringify(blockerPartition.contentClosureBehavior)}`)
    require(blockerPartition.platform.length >= 1,
      `content closure platform classification count changed: ${JSON.stringify(blockerPartition.platform)}`)
    require(!blockerPartition.authorityBehavior.some((blocker) => blocker.includes("random stream")) &&
      blockerPartition.authorityBehavior.some((blocker) => blocker.includes("sticky IVP solver unavailable")) &&
      blockerPartition.authorityBehavior.some((blocker) => blocker.includes("Tempus core and jump_beef zone contract unavailable")),
    `authority blocker ledger differs: ${JSON.stringify(blockerPartition.authorityBehavior)}`)
    await agent(["--session", session, "press", "Escape"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='pause'", "--timeout", "30000"])
    const pauseControls = parseJson<Record<string, string | null>>(await agent([
      "--session", session, "eval", "Object.fromEntries(['ResumeButton','DisconnectButton'].map(n=>[n,document.querySelector(`[data-vgui-name=${n}]`)?.getAttribute('aria-disabled')??null]))",
    ]))
    require(Object.values(pauseControls).every((value) => value === "false"), `pause controls differ: ${JSON.stringify(pauseControls)}`)
    await clickVguiPanel(session, "TF2SettingsButton")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.optionsVisible==='true'", "--timeout", "30000"])
    require(parseJson<string>(await agent(["--session", session, "eval", `document.querySelector('${playerModelSetting}').getAttribute('aria-checked')`])) === "false", "persisted player-class model setting differs")
    await scrollAdvancedControlIntoView(session, playerModelSetting)
    await clickVguiSelector(session, playerModelSetting)
    await agent(["--session", session, "wait", "--fn", `document.querySelector('${playerModelSetting}').getAttribute('aria-checked')==='true'`, "--timeout", "10000"])
    const priorPausedPlayerModelApply = parseJson<string | null>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.settingsApply??null"]))
    await agent(["--session", session, "click", "[data-vgui-runtime=tf2-advanced-options] [data-vgui-name=OkButton]"])
    await agent(["--session", session, "wait", "--fn", `document.querySelector('main').dataset.optionsVisible==='false'||(document.querySelector('main').dataset.settingsApply??null)!==${JSON.stringify(priorPausedPlayerModelApply)}`, "--timeout", "30000"])
    const pausedPlayerModelApply = parseJson<{ visible: string; apply: string | null }>(await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main');return{visible:main.dataset.optionsVisible,apply:main.dataset.settingsApply??null}})()"] ))
    require(pausedPlayerModelApply.visible === "false", `paused player-model Options apply did not close: ${JSON.stringify(pausedPlayerModelApply)}`)
    const pausedHudPresentation = parseJson<{ classImage: { visible: boolean }; classModel: { visible: boolean; model: string; scalars: Record<string, number> }; classModelBackground: string; roots: { playerStatus: number; ammo: number } }>(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudPresentationProbe"])))
    require(!pausedHudPresentation.classImage.visible && pausedHudPresentation.classModel.visible
      && pausedHudPresentation.classModel.model === "models/player/demo.mdl"
      && pausedHudPresentation.classModel.scalars.class === 4 && pausedHudPresentation.classModel.scalars.team === 2 && pausedHudPresentation.classModel.scalars.skin === 0
      && pausedHudPresentation.classModelBackground === "../hud/character_red_bg_clipped"
      && pausedHudPresentation.roots.playerStatus === 1 && pausedHudPresentation.roots.ammo === 1,
    `paused HUD setting swap differs: ${JSON.stringify(pausedHudPresentation)}`)
    await clickVguiPanel(session, "ResumeButton")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='in-game'", "--timeout", "30000"])
    await agent(["--session", session, "press", "Escape"])
    try {
      await agent(["--session", session, "wait", "--fn", "(()=>{const button=document.querySelector('[data-vgui-name=DisconnectButton]'),rect=button.getBoundingClientRect();return document.querySelector('main').dataset.gameui==='pause'&&button.getAttribute('aria-hidden')==='false'&&rect.width>0&&rect.height>0&&document.elementFromPoint(rect.x+rect.width/2,rect.y+Math.min(8,rect.height/2))===button})()", "--timeout", "30000"])
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const button=document.querySelector('[data-vgui-name=DisconnectButton]'),rect=button.getBoundingClientRect();return{gameui:document.querySelector('main').dataset.gameui,hidden:button.getAttribute('aria-hidden'),display:getComputedStyle(button).display,visibility:getComputedStyle(button).visibility,pointer:getComputedStyle(button).pointerEvents,rect:rect.toJSON(),stack:document.elementsFromPoint(rect.x+rect.width/2,rect.y+rect.height/2).slice(0,6).map(value=>({name:value.dataset.vguiName,control:value.dataset.vguiControl,id:value.id}))}})()"])
      throw new BrowserEvidenceError(`${String(error)}; second pause state=${state}`)
    }
    await mouseClickVguiPanel(session, "DisconnectButton")
    try {
      await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='MainMenu'&&document.querySelector('main').dataset.gameplayInitialized==='false'", "--timeout", "300000"])
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main');return{url:location.href,body:document.body.innerText.slice(0,300),phase:main?.dataset.phase??null,gameui:main?.dataset.gameui??null,gameplay:main?.dataset.gameplayInitialized??null,detail:main?.dataset.detail??null,options:main?.dataset.optionsVisible??null}})()"])
      throw new BrowserEvidenceError(`${String(error)}; disconnect state=${state}`)
    }
    const returnStartup = parseJson<{ state: string; display: string }>(await agent(["--session", session, "eval", "(()=>({state:document.querySelector('main').dataset.startupState,display:getComputedStyle(document.querySelector('.startup-layer')).display}))()"] ))
    require(returnStartup.state === "Skipped" && returnStartup.display === "none", `startup replayed after disconnect: ${JSON.stringify(returnStartup)}`)

    const reloadTabs = await agent(["--session", session, "tab"])
    const reloadTab = /\[(t\d+)\]/.exec(reloadTabs)?.[1]
    require(reloadTab, `browser tab is unavailable before warm reload: ${reloadTabs}`)
    await agent(["--session", session, "tab", "new", owner.url])
    await agent(["--session", session, "tab", "close", reloadTab])
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    const warmStartup = await completeStartup(session, config, "warm-1280x720", "skip")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'MainMenu' && document.querySelector('main').dataset.gameplayInitialized === 'false'", "--timeout", "300000"])
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map jump_beef"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='loading'&&document.querySelector('.loading-layer [data-vgui-name=LoadingDialog]')", "--timeout", "30000"])
    const warmLoadingPresentation = parseJson<{ background: number[]; dialog: number[]; disposition: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const rect=n=>{const r=document.querySelector(`.loading-layer [data-vgui-name=${n}]`).getBoundingClientRect();return[r.x,r.y,r.width,r.height]},m=document.querySelector('main');return{background:rect('Background'),dialog:rect('LoadingDialog'),disposition:m.dataset.loadingBackground}})()",
    ]))
    require(JSON.stringify(warmLoadingPresentation.background) === JSON.stringify([0, 0, 960, 720])
      && JSON.stringify(warmLoadingPresentation.dialog) === JSON.stringify([890, 598, 380, 112])
      && warmLoadingPresentation.disposition === "configured-generic",
    `warm loading presentation differs: ${JSON.stringify(warmLoadingPresentation)}`)
    const warmLoading = await captureInterface(session, config, "loading-warm-1280x720")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'Ready'", "--timeout", "600000"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('main').dataset.cameraPosition && document.querySelector('main').dataset.hudProbe",
      "--timeout",
      "30000",
    ])
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.cache"])) === "hit", "warm browser run did not reuse the derived payload")
    require(!parseJson<string[]>(await agent(["--session", session, "eval", "JSON.parse(document.querySelector('main').dataset.blockers)"])).some((value) => value.startsWith("ModelArtifactCacheUnavailable")), "bounded model presentation artifacts were not cached")
    await agent(["--session",session,"wait","--fn","Math.abs(Number(document.querySelector('main').dataset.cameraPosition.split(',')[2])-(-3067.96875))<0.001","--timeout","10000"])
    const warmCamera = await cameraObservation(session)
    const warmCanvas = await captureCanvas(session, config)
    require(warmCanvas.regions.every((region, index) => region.sha256 === coldCanvas.regions[index]?.sha256),
      `warm fixed-camera world regions differ from cold: ${JSON.stringify({ cold: coldCanvas.regions, warm: warmCanvas.regions })}`)
    require(warmCamera.position.every((value, index) => Math.abs(value - fixedCamera.position[index]!) <= 0.001) &&
      Math.abs(warmCamera.yaw - fixedCamera.yaw) <= 0.001 &&
      Math.abs(warmCamera.pitch - fixedCamera.pitch) <= 0.001, `warm fixed camera differs from the cold camera: ${JSON.stringify({ cold: fixedCamera, warm: warmCamera })}`)
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.consoleVisible==='false'", "--timeout", "30000"])
    await agent([
      "--session",
      session,
      "eval",
      "(()=>{const s={done:false,error:null,result:null};globalThis.__playsrcIdbEvidence=s;s.open=indexedDB.open('playsrc-derived-v2',1);s.open.onerror=()=>{s.error=String(s.open.error);s.done=true};s.open.onsuccess=()=>{try{s.database=s.open.result;s.transaction=s.database.transaction('objects');s.request=s.transaction.objectStore('objects').getAll();s.request.onerror=()=>{s.error=String(s.request.error);s.done=true};s.request.onsuccess=()=>{s.result=s.request.result.map(x=>({key:x.key,byteLength:x.byteLength,sha256:x.sha256}));s.done=true}}catch(error){s.error=String(error);s.done=true}};return true})()",
    ])
    await agent(["--session", session, "wait", "--fn", "globalThis.__playsrcIdbEvidence?.done===true", "--timeout", "30000"])
    const records = parseJson<Array<{ key: string; byteLength: number; sha256: string }>>(
      await agent(["--session", session, "eval", "(()=>{const s=globalThis.__playsrcIdbEvidence;if(s.error)throw new Error(s.error);return s.result})()"]),
    )
    const mapRecords = records.filter(
      (record) =>
        record.sha256 === "d38eab0759df0d92f91832ca63848d5ed55f84b040c52f814cbc2c97b6a2e39d" ||
        record.sha256 === "56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156",
    )
    require(mapRecords.length === (platformFontSupported ? 2 : 1) &&
      mapRecords.some(
        (record) =>
          record.byteLength === 78_302_136 &&
          record.sha256 === "d38eab0759df0d92f91832ca63848d5ed55f84b040c52f814cbc2c97b6a2e39d",
      ) && (!platformFontSupported || mapRecords.some(
        (record) =>
          record.byteLength === 42_082_929 &&
          record.sha256 === "56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156",
      )), `warm IndexedDB record identity differs: ${JSON.stringify(records)}`)
    return {
      target: "jump_beef",
      browser: version,
      startup: { cold: startup, settingsReload: settingsReloadStartup, mobile: mobileStartup, warm: warmStartup },
      loadingPresentation: { mobile: mobileLoading, cold: loadingPresentation, warm: warmLoading, warmState: warmLoadingPresentation },
      coldCache: "stored",
      warmCache: "hit",
      derived: mapRecords,
      mapReplacementGeneration: platformFontSupported ? 4 : 1,
      movingSpeed,
      jumpSpeed,
      supportBlockers: blockerCount,
      supportBlockerItems,
      contentBlockers: blockerPartition.content,
      behaviorBlockers: blockerPartition.behavior,
      contentClosureBehaviorBlockers: blockerPartition.contentClosureBehavior,
      visualBehaviorBlockers: blockerPartition.visualBehavior,
      authorityBehaviorBlockers: blockerPartition.authorityBehavior,
      platformBlockers: blockerPartition.platform,
      supportStatus: "zero-content-blockers-non-content-diagnostics-retained",
      pointerLock: pointerLocked ? `acquired-${pointerMovement}-and-released-for-console` : "headed-window-focus-unavailable",
      console: platformFontSupported
        ? "history-completion-focus-repeated-visibility-replacement-close-passed"
        : "unsupported-platform-fonts-suppressed-paint-and-input",
      gameUi: { menuPresentation, mobileInterface },
      options: { keyboard: keyboardOptions, visualDefault: optionsVisualDefault, armedButton, comboDefault, comboHover, captures: optionsCaptures, conflict: conflictBindings, reset: resetBindings, keyboardAdvanced, videoAdvanced, advanced: advancedOptions },
      hud: { initialOperations: initialHudOperations, initialPresentation: initialHudPresentation, pausedPresentation: pausedHudPresentation, pauseControls, animationTrace: hudAnimationTrace },
      presentationViewport: { desktopMenu: desktopMenuViewport, mobileMenu: mobileMenuViewport, gameplay: gameplayViewport },
      audio: "exact-buffers-decoded-and-context-running",
      fixedCamera,
      fixedSpawn,
      canvas: coldCanvas,
      pointerDirection: {
        positiveHorizontalRightDot: Number(rightDirectionDelta.toFixed(6)),
        positiveVerticalDownDot: Number(downDirectionDelta.toFixed(6)),
      },
      lookCadence,
      crouch: { down: crouchDown, up: crouchUp },
      producerProbes,
      modelMatrices: "33-exact-source-entity-matrices",
      decalState,
      visibleDecalFragments,
      visualBlockers,
      soldierPresentation,
      particleCanvas,
      shutdown: "pending",
    }
  } catch (error) {
    primaryError = error
    throw error
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    try {
      await owner.interrupt()
    } catch (error) {
      if (primaryError === undefined) throw error
    }
  }
}

export async function runBrowserAcceptance(config: LocalConfig, target: string | undefined): Promise<void> {
  const report = await verifyBrowserAcceptance(config, target)
  require((await unavailable("http://127.0.0.1:4173/readyz")) &&
    (await unavailable("http://127.0.0.1:4174/readyz")), "owned listeners remained available after shutdown")
  console.log(JSON.stringify({ ...report, shutdown: "sigint-child-and-listeners-released" }))
}
