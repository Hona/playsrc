import { constants as fsConstants } from "node:fs"
import { copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { repositoryRoot, type LocalConfig } from "./config"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import { TF2_BROWSER_SETTINGS_STORAGE_KEY } from "@playsrc/game-tf2-browser/settings-integration"
import { chunksForRole, parseResourceGraphBytes } from "@playsrc/asset-store/graph"
import checkedRelease from "../../../apps/web/tf2/releases/current.json"
import { parseTf2Release } from "../../../apps/web/tf2/src/deployment"
import { Tf2BrowserAutomation } from "../../../apps/web/tf2/src/browser-automation"
import { buildSourceBundle } from "./source-bundle"

const MAX_OUTPUT_BYTES = 1024 * 1024
const PROCESS_READY_TIMEOUT_MS = 300_000
const PROCESS_EXIT_TIMEOUT_MS = 30_000
const APPLICATION_URL = "http://127.0.0.1:4173/"
const TF2_BROWSER_AUTOMATION_INIT = path.join(repositoryRoot, "apps/web/tf2/src/browser-automation-init.js")
const VIEWPORT_WIDTH = 1280
const VIEWPORT_HEIGHT = 720
const BACKGROUND_RGB = [17, 24, 32] as const
const CURRENT_TF2_RELEASE=parseTf2Release(checkedRelease)
const EXPECTED_RESOURCE_GRAPH_SHA256 = CURRENT_TF2_RELEASE.targets.find((target) => target.target === "jump_beef")!.objects.resources.sha256
const EXPECTED_RESOURCE_ROLES = Object.freeze({
  startup: Object.freeze({ entries: 2, encodedBytes: 1_323_980 }),
  menu: Object.freeze({ entries: 966, encodedBytes: 63_600_378 }),
  gameplay: Object.freeze({ entries: 725, encodedBytes: 168_705_971 }),
})

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

async function admitInitialClassSelection(session: string): Promise<void> {
  const state = parseJson<{ visible: boolean; consoleVisible: boolean }>(await agent([
    "--session", session, "eval",
    "(()=>{const value=document.querySelector('main').dataset;return{visible:value.classSelectionVisible==='true',consoleVisible:value.consoleVisible==='true'}})()",
  ]))
  if (!state.visible) return
  if (state.consoleVisible) await agent(["--session", session, "press", "Backquote"])
  await agent(["--session", session, "press", "2"])
  await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.classSelectionVisible==='false'", "--timeout", "30000"])
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
  const artifact = await buildSourceBundle(config, "jump_beef")
  const bytes = await readFile(artifact.ledgerPath)
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
    ledger.resolvedEntries === 1396 && ledger.authoritativeAbsences === 104 &&
    Array.isArray(ledger.requests) && ledger.requests.length === 1500,
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
  requireSource("scripts/chapterbackgrounds.txt", 230, "9d24d5870425b7a793583e95db933bd66aec51495840c5a97d3278566048cc58", "game-04-tf2_misc_dir.vpk")
  requireSource("materials/console/background_2fort.vmt", 176, "4fa05e0ddbf5da835ea7e1d70872775d91e56e918ad2f36ae4b24c4cb62afcc3", "game-04-tf2_misc_dir.vpk")
  requireSource("materials/console/background_2fort.vtf", 2_796_448, "c6311965e57125bec0a98de320c4cd4ef7b297c874f47acb40107f0d31d911d9", "game-01-tf2_textures_dir.vpk")
  requireSource("materials/console/background_2fort_widescreen.vmt", 187, "62ea66916136838dec8b843437c21bb3c24cbc5811b00af4f253043156d7ba65", "game-04-tf2_misc_dir.vpk")
  requireSource("materials/console/background_2fort_widescreen.vtf", 2_796_448, "da391abcb6d121dea3786c16014f216cdbbcaf0d5810aa3ef395341f601ddcec", "game-01-tf2_textures_dir.vpk")
  const graph = parseResourceGraphBytes(await readFile(artifact.graphPath))
  for (const [role, expected] of Object.entries(EXPECTED_RESOURCE_ROLES)) {
    const chunks = chunksForRole(graph, role)
    require(chunks.reduce((total, chunk) => total + chunk.entries.length, 0) === expected.entries
      && chunks.reduce((total, chunk) => total + Number(chunk.encodedByteLength), 0) === expected.encodedBytes,
    `${role} resource graph selection differs`)
  }
  for (const logicalPath of [
    "scripts/chapterbackgrounds.txt",
    "materials/console/background_2fort.vmt",
    "materials/console/background_2fort.vtf",
    "materials/console/background_2fort_widescreen.vmt",
    "materials/console/background_2fort_widescreen.vtf",
    "playsrc/tf2-gameui-background.json",
  ]) {
    const owners = graph.chunks.filter((chunk) => chunk.entries.some((entry) => entry.logicalPath === logicalPath))
    require(owners.length === 1 && JSON.stringify(owners[0]!.roles) === JSON.stringify(["menu"]),
      `GameUI base-background graph role differs: ${logicalPath}`)
  }
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
  warmParticlePixels: number
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
type GameUiBackgroundEvidence = InterfaceEvidence & Readonly<{ nonBlackSamples: number; sampleSha256: string }>

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

type VisualRegion = Readonly<{ name: string; x: number; y: number; width: number; height: number }>
type CrouchTrajectory = Readonly<{ fractions: readonly number[]; offsets: readonly number[] }>
type RocketPixelPlane = Readonly<{
  name: string
  changedPixels: number
  brightenedPixels: number
  darkenedPixels: number
  warmFlashPixels: number
  smokePixels: number
  debrisPixels: number
  samples: readonly Readonly<{ x: number; y: number; before: readonly number[]; after: readonly number[]; classes: readonly string[] }>[]
}>
type RocketFrameState = Readonly<{ tick: number; displayFrame: number; fireEvents: number; explosionEvents: number; projectiles: number; particleItems: number; materials: string }>
type RocketWorldSample = Readonly<{ x: number; y: number; disposition: string; depth: number | null; primitive: number | null; object: number | null; material: string | null }>

const VISUAL_REGIONS = Object.freeze([
  Object.freeze({ name: "ceiling", x: 400, y: 120, width: 320, height: 100 }),
  Object.freeze({ name: "forward-wall", x: 400, y: 270, width: 320, height: 180 }),
  Object.freeze({ name: "floor", x: 180, y: 500, width: 160, height: 130 }),
])
const ROCKET_IMPACT_REGION: VisualRegion = Object.freeze({
  name: "rocket-impact-wall",
  x: 480,
  y: 240,
  width: 272,
  height: 216,
})
const ROCKET_IMPACT_SURFACE_REGION: VisualRegion = Object.freeze({
  name: "rocket-impacted-opaque-surface",
  x: 640,
  y: 320,
  width: 64,
  height: 80,
})
const ROCKET_VISUAL_REGIONS = Object.freeze([...VISUAL_REGIONS, ROCKET_IMPACT_REGION, ROCKET_IMPACT_SURFACE_REGION])

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

function measureRegion(image: DecodedPng, region: VisualRegion): RegionMetric {
  require(region.x + region.width <= image.width &&
    region.y + region.height <= image.height, `${region.name} sample region is outside the canvas`)
  let nonBackground = 0
  let luma = 0
  let warmParticlePixels = 0
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
      if (red >= 220 && green >= 140 && blue <= 100) warmParticlePixels += 1
      luma += red * 0.2126 + green * 0.7152 + blue * 0.0722
    }
  }
  return Object.freeze({
    ...region,
    nonBackgroundRatio: Number((nonBackground / pixels).toFixed(6)),
    meanLuma: Number((luma / pixels).toFixed(3)),
    warmParticlePixels,
    sha256: new Bun.CryptoHasher("sha256").update(samples).digest("hex"),
  })
}

async function captureCanvas(session: string, config: LocalConfig, target = "jump_beef", regions: readonly VisualRegion[] = VISUAL_REGIONS): Promise<CanvasEvidence> {
  const evidenceDirectory = path.join(config.sourceCacheDir, "evidence", "browser", target)
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
    regions: Object.freeze(regions.map((region) => measureRegion(image, region))),
  })
}

async function rocketFrameState(session: string): Promise<RocketFrameState> {
  const state = parseJson<RocketFrameState>(await agent([
    "--session", session, "eval",
    "(()=>{const m=document.querySelector('main'),c=document.querySelector('.world-canvas'),d=m.dataset;return{tick:Number(d.snapshotTick),displayFrame:Number(c.dataset.displayFrame),fireEvents:Number(d.fireEvents),explosionEvents:Number(d.explosionEvents),projectiles:Number(d.projectiles),particleItems:Number(d.particleItems),materials:d.particleProbe??''}})()",
  ]))
  require([state.tick, state.displayFrame, state.fireEvents, state.explosionEvents, state.projectiles, state.particleItems]
    .every((value) => Number.isSafeInteger(value) && value >= 0), `rocket frame observation is malformed: ${JSON.stringify(state)}`)
  return Object.freeze(state)
}

async function retainedCanvas(config: LocalConfig, capture: CanvasEvidence): Promise<DecodedPng> {
  return decodePng(new Uint8Array(await readFile(path.join(
    config.sourceCacheDir, "evidence", "browser", "jump_beef", `${capture.sha256}.png`,
  ))))
}

function rocketPixelPlane(before: DecodedPng, after: DecodedPng, region: VisualRegion): RocketPixelPlane {
  require(before.width === after.width && before.height === after.height,
    `${region.name} color planes do not share one image geometry`)
  require(region.x >= 0 && region.y >= 0 && region.x + region.width <= before.width && region.y + region.height <= before.height,
    `${region.name} color plane escapes the fixed canvas`)
  let changedPixels = 0, brightenedPixels = 0, darkenedPixels = 0, warmFlashPixels = 0, smokePixels = 0, debrisPixels = 0
  const samples: { x: number; y: number; before: readonly number[]; after: readonly number[]; classes: readonly string[] }[] = []
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      const at = (y * before.width + x) * 3
      const previous = [before.rgb[at]!, before.rgb[at + 1]!, before.rgb[at + 2]!] as const
      const current = [after.rgb[at]!, after.rgb[at + 1]!, after.rgb[at + 2]!] as const
      const red = current[0] - previous[0], green = current[1] - previous[1], blue = current[2] - previous[2]
      if (Math.abs(red) + Math.abs(green) + Math.abs(blue) < 24) continue
      changedPixels += 1
      const luma = red * 0.2126 + green * 0.7152 + blue * 0.0722
      const classes: string[] = []
      if (luma >= 12) { brightenedPixels += 1; classes.push("brightened") }
      if (luma <= -12) { darkenedPixels += 1; classes.push("darkened") }
      if (red >= 24 && green >= 12 && current[0] >= current[1] && current[1] > current[2]) {
        warmFlashPixels += 1
        classes.push("warm-flash")
      }
      if (luma <= -12 && Math.max(...current) - Math.min(...current) <= 72) {
        smokePixels += 1
        classes.push("smoke")
      }
      if (luma <= -12 && current[0] >= current[1] && current[1] >= current[2] && current[0] - current[2] >= 12) {
        debrisPixels += 1
        classes.push("debris")
      }
      if (samples.length < 24 && classes.length > 0) samples.push(Object.freeze({
        x, y, before: Object.freeze([...previous]), after: Object.freeze([...current]), classes: Object.freeze(classes),
      }))
    }
  }
  return Object.freeze({ name: region.name, changedPixels, brightenedPixels, darkenedPixels, warmFlashPixels, smokePixels, debrisPixels, samples: Object.freeze(samples) })
}

async function installRocketGpuEvidence(session: string): Promise<void> {
  const installed = parseJson<boolean>(await agent([
    "--session", session, "eval",
    `(() => {
      if (typeof GPUDevice === 'undefined' || typeof GPURenderPassEncoder === 'undefined') return false
      const evidence = {
        armed: false, pipelines: [], draws: [], transactions: [], outputs: [], passes: [], impact: null,
        pipelineByObject: new WeakMap(), passByObject: new WeakMap(), renderPassByObject: new WeakMap(), observedWorkers: new WeakSet(),
      }
      globalThis.__playsrcRocketGpuEvidence = evidence
      const summary = descriptor => ({
        label: descriptor.label ?? '',
        depthTest: descriptor.depthStencil?.depthCompare ?? null,
        depthWrite: descriptor.depthStencil?.depthWriteEnabled ?? false,
        blend: (descriptor.fragment?.targets ?? []).map(target => target?.blend
          ? { color: target.blend.color, alpha: target.blend.alpha } : null),
        vertexStrides: (descriptor.vertex?.buffers ?? []).map(buffer => buffer?.arrayStride ?? null),
        topology: descriptor.primitive?.topology ?? 'triangle-list',
        cullMode: descriptor.primitive?.cullMode ?? 'none',
      })
      for (const name of ['createRenderPipeline', 'createRenderPipelineAsync']) {
        const original = GPUDevice.prototype[name]
        if (typeof original !== 'function') continue
        GPUDevice.prototype[name] = function (descriptor) {
          const state = summary(descriptor), pipeline = original.call(this, descriptor)
          const remember = result => {
            evidence.pipelineByObject.set(result, state)
            if (evidence.pipelines.length < 512) evidence.pipelines.push(state)
            return result
          }
          return name === 'createRenderPipelineAsync' ? pipeline.then(remember) : remember(pipeline)
        }
      }
      const beginRenderPass = GPUCommandEncoder.prototype.beginRenderPass
      GPUCommandEncoder.prototype.beginRenderPass = function (descriptor) {
        const pass = beginRenderPass.call(this, descriptor)
        if (evidence.armed) {
          const state = {
            colorLoad: (descriptor.colorAttachments ?? []).map(attachment => attachment?.loadOp ?? null),
            depthLoad: descriptor.depthStencilAttachment?.depthLoadOp ?? null,
            operations: [],
          }
          if (evidence.passes.length === 64) evidence.passes.shift()
          evidence.passes.push(state)
          evidence.renderPassByObject.set(pass, state)
        }
        return pass
      }
      const executeBundles = GPURenderPassEncoder.prototype.executeBundles
      GPURenderPassEncoder.prototype.executeBundles = function (bundles) {
        const state = evidence.renderPassByObject.get(this)
        if (state && state.operations.length < 24) state.operations.push({ kind: 'world-bundles', count: bundles.length })
        return executeBundles.call(this, bundles)
      }
      const setPipeline = GPURenderPassEncoder.prototype.setPipeline
      GPURenderPassEncoder.prototype.setPipeline = function (pipeline) {
        evidence.passByObject.set(this, evidence.pipelineByObject.get(pipeline) ?? null)
        return setPipeline.call(this, pipeline)
      }
      const drawIndexed = GPURenderPassEncoder.prototype.drawIndexed
      GPURenderPassEncoder.prototype.drawIndexed = function (...args) {
        const pipeline = evidence.passByObject.get(this) ?? null
        if (evidence.armed && evidence.draws.length < 2048)
          evidence.draws.push({ indices: args[0], instances: args[1] ?? 1, pipeline })
        const state = evidence.renderPassByObject.get(this)
        if (state && state.operations.length < 24) {
          const particle = pipeline?.vertexStrides.join(',') === '8,4,8,16,12'
          if (particle || state.operations.filter(operation => operation.kind === 'indexed').length < 2) {
            state.operations.push({ kind: particle ? 'particle' : 'indexed', indices: args[0], depthTest: pipeline?.depthTest ?? null })
          }
        }
        return drawIndexed.apply(this, args)
      }
      const rotateForward = quaternion => [
        1 - 2 * (quaternion[1] ** 2 + quaternion[2] ** 2),
        2 * (quaternion[0] * quaternion[1] + quaternion[2] * quaternion[3]),
        2 * (quaternion[0] * quaternion[2] - quaternion[1] * quaternion[3]),
      ]
      const vector = (view, offset) => [0, 1, 2].map(axis => view.getFloat32(offset + axis * 4, true))
      const captureOutput = event => {
        if (!evidence.armed || event.data?.kind !== 'particles' || !(event.data.output instanceof ArrayBuffer)) return
        const view = new DataView(event.data.output)
        if (view.byteLength < 40 || view.getUint32(4, true) !== 3) return
        const count = view.getUint32(8, true)
        const materials = new Map()
        for (let index = 0; index < count; index++) {
          const at = 40 + index * 436
          if (at + 436 > view.byteLength) break
          const identity = view.getUint32(at + 32, true), position = vector(view, at + 36)
          const group = materials.get(identity) ?? { materialIndex: identity, count: 0, front: 0, behind: 0, minimumPlaneDistance: null, maximumPlaneDistance: null, samples: [] }
          group.count++
          if (evidence.impact) {
            const distance = position.reduce((total, value, axis) => total
              + (value - evidence.impact.wall[axis]) * evidence.impact.normal[axis], 0)
            if (distance >= 0) group.front++; else group.behind++
            group.minimumPlaneDistance = group.minimumPlaneDistance === null ? distance : Math.min(group.minimumPlaneDistance, distance)
            group.maximumPlaneDistance = group.maximumPlaneDistance === null ? distance : Math.max(group.maximumPlaneDistance, distance)
            if (group.samples.length < 3) group.samples.push({ position, planeDistance: distance })
          }
          materials.set(identity, group)
        }
        if (evidence.outputs.length === 96) evidence.outputs.shift()
        evidence.outputs.push({ tick: Number(document.querySelector('main')?.dataset.snapshotTick ?? 0), count, materials: [...materials.values()] })
      }
      const originalPostMessage = Worker.prototype.postMessage
      Worker.prototype.postMessage = function (message, ...rest) {
        if (message?.kind === 'particles' && !evidence.observedWorkers.has(this)) {
          evidence.observedWorkers.add(this)
          this.addEventListener('message', captureOutput)
        }
        if (evidence.armed && message?.kind === 'particles' && message.batch instanceof ArrayBuffer) {
          const view = new DataView(message.batch)
          if (view.byteLength >= 32 && view.getUint32(4, true) === 2) {
            const events = [], count = view.getUint32(28, true)
            let offset = 32
            for (let index = 0; index < count && offset + 20 <= view.byteLength; index++) {
              const kind = view.getUint8(offset), time = view.getFloat32(offset + 12, true), effectIdentity = view.getUint32(offset + 16, true)
              offset += 20
              if (kind === 1) {
                const length = view.getUint32(offset + 12, true)
                const system = new TextDecoder().decode(new Uint8Array(message.batch, offset + 16, length))
                offset += 16 + length
                const position = vector(view, offset), orientation = [0, 1, 2, 3].map(axis => view.getFloat32(offset + 12 + axis * 4, true))
                events.push({ kind: 'start', time, effectIdentity, system, position, orientation })
                if (system.toLowerCase() === 'explosioncore_wall') {
                  const normal = rotateForward(orientation)
                  evidence.impact = { origin: position, normal, wall: position.map((value, axis) => value - normal[axis]) }
                }
                offset += 32
              } else if (kind === 2) {
                events.push({ kind: 'set-control-point', time, effectIdentity, position: vector(view, offset) })
                offset += 32
              } else events.push({ kind: kind === 3 ? 'graceful-stop' : 'immediate-stop', time, effectIdentity })
            }
            if (events.length && evidence.transactions.length < 96) evidence.transactions.push({
              from: view.getFloat32(8, true), to: view.getFloat32(12, true), events,
            })
          }
        }
        return originalPostMessage.call(this, message, ...rest)
      }
      return true
    })()`,
  ]))
  require(installed, "headed Chromium did not expose the WebGPU pipeline and render-pass evidence seam")
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

async function captureGameUiBackground(session: string, config: LocalConfig, identity: string): Promise<GameUiBackgroundEvidence> {
  const evidenceDirectory = path.join(config.sourceCacheDir, "evidence", "browser", "tf2-interface")
  await mkdir(evidenceDirectory, { recursive: true })
  const temporaryPath = path.join(evidenceDirectory, `${identity}-${process.pid}.png`)
  await agent(["--session", session, "eval", "(()=>{const prior=document.getElementById('playsrc-background-evidence-style');if(prior)prior.remove();const style=document.createElement('style');style.id='playsrc-background-evidence-style';style.textContent='[data-vgui-name=MainMenuOverride],[data-vgui-name=MMDashboard]{opacity:0!important}';document.head.append(style);return true})()"])
  try {
    await agent(["--session", session, "screenshot", ".gameui-layer", temporaryPath])
  } finally {
    await agent(["--session", session, "eval", "document.getElementById('playsrc-background-evidence-style')?.remove();true"])
  }
  const bytes = new Uint8Array(await readFile(temporaryPath))
  require(bytes.byteLength <= 16 * 1024 * 1024, "GameUI background PNG exceeds the evidence byte bound")
  const image = await decodePng(bytes)
  const samples = new Uint8Array(27)
  let nonBlackSamples = 0
  let offset = 0
  for (const yFraction of [0.1, 0.5, 0.9]) for (const xFraction of [0.1, 0.5, 0.9]) {
    const x = Math.min(image.width - 1, Math.floor(image.width * xFraction))
    const y = Math.min(image.height - 1, Math.floor(image.height * yFraction))
    const source = (y * image.width + x) * 3
    const red = image.rgb[source]!, green = image.rgb[source + 1]!, blue = image.rgb[source + 2]!
    samples[offset++] = red; samples[offset++] = green; samples[offset++] = blue
    if (red + green + blue >= 48) nonBlackSamples += 1
  }
  const sha256 = new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
  const sampleSha256 = new Bun.CryptoHasher("sha256").update(samples).digest("hex")
  const retainedPath = path.join(evidenceDirectory, `${identity}-${sha256}.png`)
  try { await copyFile(temporaryPath, retainedPath, fsConstants.COPYFILE_EXCL) }
  catch (error) { if (!(error instanceof Error && "code" in error && error.code === "EEXIST")) throw error }
  finally { await rm(temporaryPath, { force: true }) }
  return Object.freeze({ sha256, byteLength: bytes.byteLength, width: image.width, height: image.height, nonBlackSamples, sampleSha256 })
}

async function completeStartup(
  session: string,
  config: LocalConfig,
  identity: string,
  disposition: "complete" | "skip",
): Promise<Readonly<{ first: InterfaceEvidence; middle?: InterfaceEvidence; final?: InterfaceEvidence }>> {
  try {
    await agent(["--session", session, "wait", "--fn", "['AwaitingGesture','Playing'].includes(document.querySelector('main')?.dataset.startupState)", "--timeout", "300000"])
  } catch (error) {
    const state = await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main');return{body:document.body.innerText.slice(0,500),main:!!main,startupState:main?.dataset.startupState??null,phase:main?.dataset.phase??null,detail:main?.dataset.detail??null}})()"]).catch((probe) => `probe failed: ${String(probe)}`)
    throw new BrowserEvidenceError(`${String(error)}; startup state: ${state}`)
  }
  const hidden = parseJson<{ state: string; hidden: boolean; inert: boolean; ariaHidden: string | null; focused: boolean; time: number; muted: boolean; mutedFallback: boolean; movie: number[]; viewport: number[] }>(await agent([
    "--session", session, "eval",
    "(()=>{const main=document.querySelector('main'),root=document.querySelector('.gameui-layer'),video=document.querySelector('.startup-movie'),r=video.getBoundingClientRect();return{state:main.dataset.startupState,hidden:root.hidden,inert:root.inert,ariaHidden:root.getAttribute('aria-hidden'),focused:root.contains(document.activeElement),time:video.currentTime,muted:video.muted,mutedFallback:main.dataset.startupMutedFallback==='true',movie:[r.x,r.y,r.width,r.height],viewport:[innerWidth,innerHeight]}})()",
  ]))
  const startupAdmission = (hidden.state === "AwaitingGesture" && hidden.time === 0 && !hidden.muted && !hidden.mutedFallback)
    || (hidden.state === "Playing" && hidden.time >= 0 && hidden.time < 1 && hidden.muted && hidden.mutedFallback)
  require(hidden.hidden && hidden.inert && hidden.ariaHidden === "true" && !hidden.focused && startupAdmission
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

function projectSourcePoint(camera: CameraObservation, point: readonly number[]): Readonly<{ x: number; y: number; depth: number }> {
  require(point.length === 3 && point.every(Number.isFinite), "projected Source point is malformed")
  const yaw = camera.yaw * Math.PI / 180
  const forward = cameraForward(camera)
  const right = [Math.sin(yaw), -Math.cos(yaw), 0] as const
  const up = [
    right[1] * forward[2] - right[2] * forward[1],
    right[2] * forward[0] - right[0] * forward[2],
    right[0] * forward[1] - right[1] * forward[0],
  ] as const
  const delta = point.map((value, index) => value - camera.position[index]!)
  const depth = delta.reduce((total, value, index) => total + value * forward[index]!, 0)
  require(depth > camera.near, `projected Source point is behind the camera: ${JSON.stringify(point)}`)
  const verticalTangent = Math.tan(camera.verticalFov * Math.PI / 360)
  const horizontalTangent = verticalTangent * VIEWPORT_WIDTH / VIEWPORT_HEIGHT
  return Object.freeze({
    x: (delta.reduce((total, value, index) => total + value * right[index]!, 0) / (depth * horizontalTangent) * 0.5 + 0.5) * VIEWPORT_WIDTH,
    y: (0.5 - delta.reduce((total, value, index) => total + value * up[index]!, 0) / (depth * verticalTangent) * 0.5) * VIEWPORT_HEIGHT,
    depth,
  })
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
  return value.trim().replaceAll(/\s+/gu, " ").slice(-500)
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

function tf2BrowserAutomation(session: string): Tf2BrowserAutomation {
  return new Tf2BrowserAutomation({
    evaluate: async <T>(expression: string): Promise<T> => parseJson<T>(
      await agent(["--session", session, "eval", expression]),
    ),
    press: async (key: string): Promise<void> => { await agent(["--session", session, "press", key]) },
    click: async (selector: string): Promise<void> => { await agent(["--session", session, "click", selector]) },
    focus: async (selector: string): Promise<void> => { await agent(["--session", session, "focus", selector]) },
    fill: async (selector: string, value: string): Promise<void> => {
      await agent(["--session", session, "fill", selector, value])
    },
    waitFor: async (expression: string, timeoutMilliseconds: number): Promise<void> => {
      await agent(["--session", session, "wait", "--fn", expression, "--timeout", String(timeoutMilliseconds)])
    },
    activateCurrentTab: async (): Promise<void> => {
      const tabs = await agent(["--session", session, "tab"])
      const tab = /\[(t\d+)\]/.exec(tabs)?.[1]
      if (tab) await agent(["--session", session, "tab", tab])
    },
  })
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

type CacheInventory = Readonly<{
  count: number
  bytes: number
  largest: readonly Readonly<{ key: string; byteLength: number }>[]
}>

async function cacheInventory(session: string): Promise<CacheInventory> {
  return parseJson<CacheInventory>(await agent([
    "--session", session, "eval",
    "(async()=>{const database=await new Promise((resolve,reject)=>{const request=indexedDB.open('playsrc-derived-v3',1);request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error)}),records=await new Promise((resolve,reject)=>{const request=database.transaction('objects').objectStore('objects').getAll();request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error)});database.close();return{count:records.length,bytes:records.reduce((total,record)=>total+record.byteLength,0),largest:records.map(record=>({key:record.key,byteLength:record.byteLength})).sort((a,b)=>b.byteLength-a.byteLength||a.key.localeCompare(b.key)).slice(0,12)}})()",
  ]))
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

async function verifyPlUpwardBrowser(config: LocalConfig): Promise<Record<string, unknown>> {
  const version = await agent(["--version"])
  const session = `playsrc-upward-${process.pid}`
  const automation = tf2BrowserAutomation(session)
  const owner = await startDevelopmentProcess("pl_upward")
  let browserOpen = false
  let primaryError: unknown
  try {
    await agent(["--session", session, "--headed", "--webgpu", "--init-script", TF2_BROWSER_AUTOMATION_INIT, "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    const startup = await completeStartup(session, config, "pl-upward-cold", "complete")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'MainMenu'", "--timeout", "300000"])
    await automation.maps.load("pl_upward")
    try {
      await agent(["--session", session, "wait", "--fn", "['Ready','Failed'].includes(document.querySelector('main').dataset.phase)", "--timeout", "600000"])
      const terminal = parseJson<{ phase: string; gameui: string }>(await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,gameui:m.dataset.gameui}})()"] ))
      if (terminal.phase !== "Ready" || terminal.gameui !== "in-game") throw new Error("loading entered failure")
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,detail:m.dataset.detail,gameui:m.dataset.gameui,loading:m.dataset.loadingStatus,body:document.body.innerText.slice(-2000)}})()"])
      throw new BrowserEvidenceError(`${String(error)}; pl_upward state: ${state}`)
    }
    await admitInitialClassSelection(session)
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.cameraPosition?.split(',').length===3", "--timeout", "30000"])
    await agent(["--session",session,"wait","--fn","document.querySelector('.world-canvas').dataset.staticProps?.length>2","--timeout","30000"])
    const staticEvidence=parseJson<{staticProps:{total:number;main:number;sky3d:number;runtimeLit:number};visibleMain:number[];sky:null|{phases:string[];skySurfaces:number;skyProps:number;visibleSkyPropSources:number[];fog:{start:number;end:number;primary:number[]};stateRestored:boolean};visibility:string;detail:string}>(await agent(["--session",session,"eval","(()=>{const m=document.querySelector('main'),c=document.querySelector('.world-canvas');return{staticProps:JSON.parse(c.dataset.staticProps),visibleMain:JSON.parse(c.dataset.visibleMainStaticProps),sky:c.dataset.sky3dPass?JSON.parse(c.dataset.sky3dPass):null,visibility:m.dataset.environmentSky??'',detail:m.dataset.detail??''}})()"] ))
    require(staticEvidence.staticProps.total===1244&&staticEvidence.staticProps.main===1184&&staticEvidence.staticProps.sky3d===60&&staticEvidence.staticProps.runtimeLit===4&&staticEvidence.visibleMain.length>0&&staticEvidence.sky!==null&&staticEvidence.sky.skySurfaces>0&&staticEvidence.sky.skyProps>0&&staticEvidence.sky.visibleSkyPropSources.length===staticEvidence.sky.skyProps&&staticEvidence.sky.phases.join(",")==="sky3d,depth-reset,main,restore"&&staticEvidence.sky.fog.start===100&&staticEvidence.sky.fog.end===20000&&staticEvidence.sky.fog.primary.join(",")==="174,193,205,255"&&staticEvidence.sky.stateRestored,`pl_upward static-prop/3D-sky evidence differs: ${JSON.stringify(staticEvidence)}`)
    const firstCamera = await cameraObservation(session)
    const firstCapture = await captureCanvas(session, config, "pl_upward")
    require(firstCapture.width === VIEWPORT_WIDTH && firstCapture.height === VIEWPORT_HEIGHT
      && firstCapture.regions.every((region) => region.nonBackgroundRatio > 0.95 && region.meanLuma > 1),
    `pl_upward first terrain capture differs: ${JSON.stringify(firstCapture)}`)
    const runtimeProps=parseJson<readonly{source:number;origin:readonly[number,number,number];lightingOrigin:readonly[number,number,number];radius:number}[]>(await agent(["--session",session,"eval","JSON.parse(document.querySelector('.world-canvas').dataset.runtimeStaticProps)"]))
    require(runtimeProps.length===4&&runtimeProps.map(prop=>prop.source).join(",")==="650,882,888,1105","runtime-lit static-prop inventory differs")
    const runtimePropCaptures=[] as Array<{source:number;screen:{x:number;y:number;width:number;height:number};capture:Awaited<ReturnType<typeof captureCanvas>>}>
    for(const prop of runtimeProps){const position:[number,number,number]=[prop.origin[0],prop.origin[1],prop.origin[2]+Math.max(16,prop.radius*0.25)],horizontal=Math.hypot(prop.origin[0]-position[0],prop.origin[1]-position[1]),yaw=0,pitch=Math.min(89,Math.atan2(position[2]-prop.origin[2],horizontal)*180/Math.PI)
      await agent(["--session",session,"eval",`globalThis.__playsrcProfile??={};globalThis.__playsrcProfile.displacementCameraOverride={position:${JSON.stringify(position)},yawDegrees:${yaw},pitchDegrees:${pitch}};true`])
      await Bun.sleep(1000)
      const projection=parseJson<{screens:readonly{source:number;x:number;y:number;width:number;height:number}[];visible:number[];camera:string}>(await agent(["--session",session,"eval","(()=>{const c=document.querySelector('.world-canvas'),m=document.querySelector('main');return{screens:JSON.parse(c.dataset.runtimeStaticPropScreen||'[]'),visible:JSON.parse(c.dataset.visibleMainStaticProps||'[]'),camera:m.dataset.cameraPosition}})()"])),screen=projection.screens.find(value=>value.source===prop.source)
      require(screen!==undefined&&screen.width>1&&screen.height>1,`runtime-lit static prop ${prop.source} is not projected: ${JSON.stringify({prop,projection})}`)
      const x=Math.max(0,Math.floor(screen.x)),y=Math.max(0,Math.floor(screen.y)),width=Math.max(1,Math.min(VIEWPORT_WIDTH-x,Math.ceil(screen.width))),height=Math.max(1,Math.min(VIEWPORT_HEIGHT-y,Math.ceil(screen.height))),capture=await captureCanvas(session,config,"pl_upward",[{name:`runtime-static-prop-${prop.source}`,x,y,width,height}])
      require(capture.regions[0]!.nonBackgroundRatio>0.05&&capture.regions[0]!.meanLuma>1,`runtime-lit static prop ${prop.source} capture differs`);runtimePropCaptures.push({source:prop.source,screen,capture})
    }
    await agent(["--session",session,"eval","delete globalThis.__playsrcProfile.displacementCameraOverride;true"])

    await automation.console.submitCommand("noclip")
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.movementMode) === 1", "--timeout", "30000"])
    await agent(["--session", session, "eval", "window.dispatchEvent(new KeyboardEvent('keydown',{code:'KeyW',key:'w',bubbles:true}));true"])
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.wishSpeed)>0", "--timeout", "30000"])
    await Bun.sleep(1000)
    await agent(["--session", session, "eval", "window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW',key:'w',bubbles:true}));true"])
    const secondCamera = await cameraObservation(session)
    require(secondCamera.position.some((value, index) => Math.abs(value - firstCamera.position[index]!) > 1),
      `pl_upward noclip did not move the camera: ${JSON.stringify({ firstCamera, secondCamera })}`)
    const secondCapture = await captureCanvas(session, config, "pl_upward")
    require(secondCapture.regions.every((region) => region.nonBackgroundRatio > 0.95 && region.meanLuma > 1),
      `pl_upward second terrain capture differs: ${JSON.stringify(secondCapture)}`)

    await automation.maps.load("pl_upward")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded pl_upward; generation 2')", "--timeout", "600000"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'Ready'", "--timeout", "300000"])
    const publication = parseJson<{ body: string; claims: string[]; displacement: number; materials: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const main=document.querySelector('main'),claims=Object.entries(main.dataset).filter(([key])=>/payload|cart|checkpoint|overtime|score|winning|winner/i.test(key)).map(([key,value])=>`${key}:${value}`);return{body:document.body.innerText,claims,displacement:Number(main.dataset.displacementSurfaces),materials:main.dataset.modelMaterialProbe||''}})()",
    ]))
    require(publication.body.includes("pl_upward") && !publication.body.includes("jump_beef"), "active stock-map product identity differs")
    require(publication.claims.length === 0, `Payload gameplay claims were published: ${JSON.stringify(publication.claims)}`)
    return {
      target: "pl_upward",
      browser: version,
      startup,
      firstCamera,
      secondCamera,
      captures: [firstCapture, secondCapture],
      staticProps:staticEvidence,
      runtimePropCaptures,
      noclip: "mode-1-movement-admitted",
      replacementGeneration: 2,
      payloadGameplayClaims: 0,
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

const DISPLACEMENT_VISUAL_PROBES = Object.freeze([
  Object.freeze({ source: 147, face: 14859, expectedOutward: Object.freeze([0, 0, 1] as const) }),
  Object.freeze({ source: 381, face: 15093, expectedOutward: Object.freeze([0, 0, 1] as const) }),
  Object.freeze({ source: 138, face: 14850, expectedOutward: Object.freeze([0.123091474, 0.123091474, 0.9847318] as const) }),
])

export async function runDisplacementVisualEvidence(config: LocalConfig): Promise<void> {
  const session = `playsrc-displacement-${process.pid}`
  const owner = await startDevelopmentProcess("pl_upward")
  const evidenceDirectory = path.join(config.sourceCacheDir, "evidence", "browser", "pl_upward-displacements", "red")
  await mkdir(evidenceDirectory, { recursive: true })
  let browserOpen = false
  let primaryError: unknown
  const observations: Record<string, unknown>[] = []
  try {
    await agent(["--session", session, "--headed", "--webgpu", "--init-script", TF2_BROWSER_AUTOMATION_INIT, "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    await completeStartup(session, config, "pl-upward-displacement-red", "complete")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'MainMenu'", "--timeout", "300000"])
    await agent(["--session", session, "eval", `globalThis.__playsrcProfile={displacementSources:${JSON.stringify(DISPLACEMENT_VISUAL_PROBES.map(probe => probe.source))}};true`])
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map pl_upward"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--fn", "['Ready','Failed'].includes(document.querySelector('main').dataset.phase)", "--timeout", "600000"])
    const phase = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.phase"]))
    if(phase!=="Ready"){
      const state=await agent(["--session",session,"eval","(()=>{const main=document.querySelector('main');return{phase:main.dataset.phase,detail:main.dataset.detail,body:document.body.innerText.slice(-1000)}})()"])
      throw new BrowserEvidenceError(`displacement visual target entered ${phase}: ${state}`)
    }
    const consoleVisible = parseJson<boolean>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.consoleVisible==='true'"]))
    if (consoleVisible) await agent(["--session", session, "press", "Backquote"])
    await admitInitialClassSelection(session)
    await agent(["--session", session, "wait", "--fn", "globalThis.__playsrcProfile?.displacements?.length===3&&globalThis.__playsrcProfile?.displacementCamera", "--timeout", "30000"])
    for (const probe of DISPLACEMENT_VISUAL_PROBES) {
      const record = parseJson<any>(await agent(["--session", session, "eval", `globalThis.__playsrcProfile.displacements.find(value=>value.source===${probe.source})`]))
      require(record?.face === probe.face && record.indices.length > 0 && record.positions.length > 0,
        `displacement ${probe.source} renderer record is unavailable`)
      const center = [0, 1, 2].map(axis => (record.bounds[0][axis] + record.bounds[1][axis]) * 0.5) as [number, number, number]
      require(center.length === 3 && center.every(Number.isFinite), `displacement ${probe.source} bounds are invalid: ${JSON.stringify(record.bounds)}`)
      const span = Math.max(...[0, 1, 2].map(axis => record.bounds[1][axis] - record.bounds[0][axis]))
      const distance = span * 0.25 + 64
      const axis = Math.abs(probe.expectedOutward[2]) < 0.9 ? [0, 0, 1] : [0, 1, 0]
      const tangentRaw = [
        probe.expectedOutward[1] * axis[2]! - probe.expectedOutward[2] * axis[1]!,
        probe.expectedOutward[2] * axis[0]! - probe.expectedOutward[0] * axis[2]!,
        probe.expectedOutward[0] * axis[1]! - probe.expectedOutward[1] * axis[0]!,
      ]
      const tangentLength = Math.hypot(...tangentRaw), tangent = tangentRaw.map(value => value / tangentLength)
      const position = center.map((value, index) => value + probe.expectedOutward[index]! * distance + tangent[index]! * distance * 0.2) as [number, number, number]
      const dx = center[0] - position[0], dy = center[1] - position[1], dz = center[2] - position[2]
      const yaw = Math.atan2(dy, dx) * 180 / Math.PI
      const pitch = -Math.atan2(dz, Math.hypot(dx, dy)) * 180 / Math.PI
      const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
      await agent(["--session", session, "eval", `globalThis.__playsrcProfile.displacementCameraOverride={position:${JSON.stringify(position)},yawDegrees:${yaw},pitchDegrees:${pitch}};true`])
      await agent(["--session", session, "wait", "--fn", `globalThis.__playsrcProfile.displacementCamera?.position?.every((value,index)=>Math.abs(value-[${position.join(",")}][index])<0.01)`, "--timeout", "30000"])
      const camera = parseJson<CameraObservation>(await agent(["--session", session, "eval", "(()=>{const value=globalThis.__playsrcProfile.displacementCamera;return{position:value.position,yaw:value.yawDegrees,pitch:value.pitchDegrees,verticalFov:value.verticalFovDegrees,near:value.near,far:value.far}})()"] ))
      require(Math.abs(wrap(camera.yaw - yaw)) < 1 && Math.abs(camera.pitch - pitch) < 1,
        `displacement ${probe.source} camera differs: ${JSON.stringify({ desired: { yaw, pitch }, camera })}`)
      const visibility = parseJson<{surfaces:number[];drawSurfaces:number[];outsideWorld:boolean;eyeLeaf:number|null;leaves:number[];areas:number[]}>(await agent(["--session", session, "eval", "globalThis.__playsrcProfile.displacementVisibility"]))
      const yawRadians = camera.yaw * Math.PI / 180, pitchRadians = camera.pitch * Math.PI / 180
      const forward = [Math.cos(pitchRadians) * Math.cos(yawRadians), Math.cos(pitchRadians) * Math.sin(yawRadians), -Math.sin(pitchRadians)]
      const right = [Math.sin(yawRadians), -Math.cos(yawRadians), 0]
      const up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
      ]
      let frontTriangles = 0, outwardTriangles = 0, minimumDepth = Number.POSITIVE_INFINITY, maximumDepth = 0
      const projected: [number, number][] = []
      const verticalTangent = Math.tan(camera.verticalFov * Math.PI / 360), horizontalTangent = verticalTangent * VIEWPORT_WIDTH / VIEWPORT_HEIGHT
      for (let offset = 0; offset < record.indices.length; offset += 3) {
        const triangle = [record.indices[offset], record.indices[offset + 1], record.indices[offset + 2]] as const
        const points = triangle.map(index => record.positions.slice(index * 3, index * 3 + 3))
        const ab = points[1].map((value: number, axis: number) => value - points[0][axis]), ac = points[2].map((value: number, axis: number) => value - points[0][axis])
        const normal = [ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2], ab[0] * ac[1] - ab[1] * ac[0]]
        if (normal.reduce((sum: number, value: number, axis: number) => sum + value * probe.expectedOutward[axis]!, 0) > 0) outwardTriangles += 1
        const toCamera = points[0].map((value: number, axis: number) => camera.position[axis]! - value)
        if (normal.reduce((sum: number, value: number, axis: number) => sum + value * toCamera[axis], 0) > 0) frontTriangles += 1
      }
      for (let offset = 0; offset < record.positions.length; offset += 3) {
        const delta = [record.positions[offset] - camera.position[0], record.positions[offset + 1] - camera.position[1], record.positions[offset + 2] - camera.position[2]]
        const depth = delta.reduce((sum, value, axis) => sum + value * forward[axis]!, 0)
        minimumDepth = Math.min(minimumDepth, depth); maximumDepth = Math.max(maximumDepth, depth)
        if (depth > camera.near) {
          const x = delta.reduce((sum, value, axis) => sum + value * right[axis]!, 0) / (depth * horizontalTangent)
          const y = delta.reduce((sum, value, axis) => sum + value * up[axis]!, 0) / (depth * verticalTangent)
          projected.push([(x * 0.5 + 0.5) * VIEWPORT_WIDTH, (0.5 - y * 0.5) * VIEWPORT_HEIGHT])
        }
      }
      require(projected.length > 0, `displacement ${probe.source} has no projected vertices`)
      const xs = projected.map(value => value[0]), ys = projected.map(value => value[1])
      const x = Math.max(0, Math.floor(Math.min(...xs))), y = Math.max(0, Math.floor(Math.min(...ys)))
      const width = Math.max(1, Math.min(VIEWPORT_WIDTH - x, Math.ceil(Math.max(...xs)) - x))
      const height = Math.max(1, Math.min(VIEWPORT_HEIGHT - y, Math.ceil(Math.max(...ys)) - y))
      const capture = await captureCanvas(session, config, "pl_upward-displacements/red", [{ name: `displacement-${probe.source}`, x, y, width, height }])
      observations.push({ source: probe.source, face: probe.face, expectedOutward: probe.expectedOutward, center, camera, bounds: record.bounds,
        cpu: { triangles: record.indices.length / 3, frontTriangles, outwardTriangles, minimumDepth, maximumDepth },
        visibility: { ...visibility, admitted: visibility.drawSurfaces.includes(probe.face) },
        draw: { submittedTriangles: record.submittedTriangles, cull: record.cull, depthTest: record.depthTest, depthWrite: record.depthWrite, blend: record.blend },
        material: record.material, lighting: record.lighting, region: { x, y, width, height }, capture })
    }
    const report = { target: "pl_upward", probes: observations }
    await writeFile(path.join(evidenceDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
    const failures = observations.flatMap((value: any) => [
      value.cpu.outwardTriangles === value.cpu.triangles ? null : `${value.source}:outward=${value.cpu.outwardTriangles}/${value.cpu.triangles}`,
      value.cpu.frontTriangles > 0 ? null : `${value.source}:front=0`,
      value.visibility.admitted ? null : `${value.source}:not-visible`,
      value.draw.submittedTriangles === value.cpu.triangles ? null : `${value.source}:submitted=${value.draw.submittedTriangles}`,
      value.draw.cull === "back" && value.draw.depthTest && value.draw.depthWrite && !value.draw.blend ? null : `${value.source}:draw-state`,
      value.lighting.kind !== "unlit" && value.lighting.samplesPerLayer > 0 ? null : `${value.source}:lighting`,
      value.capture.regions[0].nonBackgroundRatio > 0.05 && value.capture.regions[0].meanLuma > 1 ? null : `${value.source}:pixels`,
    ].filter(Boolean))
    require(failures.length === 0, `displacement visual evidence failed: ${failures.join(",")}`)
    console.log(JSON.stringify(report))
  } catch (error) {
    primaryError = error
    if (observations.length > 0) await writeFile(path.join(evidenceDirectory, "report.json"), `${JSON.stringify({ target: "pl_upward", probes: observations, error: String(error) }, null, 2)}\n`).catch(() => {})
    throw error
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    try { await owner.interrupt() } catch (error) { if (primaryError === undefined) throw error }
  }
}

async function verifyCtf2fortBrowser(config: LocalConfig): Promise<Record<string, unknown>> {
  const version = await agent(["--version"])
  const session = `playsrc-2fort-${process.pid}`
  const automation = tf2BrowserAutomation(session)
  const owner = await startDevelopmentProcess("ctf_2fort")
  let browserOpen = false
  let primaryError: unknown
  try {
    await agent(["--session", session, "--headed", "--webgpu", "--init-script", TF2_BROWSER_AUTOMATION_INIT, "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    const startup = await completeStartup(session, config, "ctf-2fort-cold", "skip")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase === 'MainMenu'", "--timeout", "300000"])
    await automation.maps.load("ctf_2fort")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='loading'&&document.querySelector('main').dataset.loadingBackground==='map-photo'", "--timeout", "30000"])
    const loadingPhoto = parseJson<{ background: { width: number; height: number }; image: { x: number; y: number; width: number; height: number; display: string; source: string } }>(await agent([
      "--session", session, "eval",
      "(()=>{const backing=document.querySelector('.loading-layer [data-vgui-name=Background]').getBoundingClientRect(),panel=document.querySelector('.loading-layer [data-vgui-name=MapImage]'),rect=panel.getBoundingClientRect(),style=getComputedStyle(panel);return{background:{width:backing.width,height:backing.height},image:{x:rect.x,y:rect.y,width:rect.width,height:rect.height,display:style.display,source:style.backgroundImage}}})()",
    ]))
    require(loadingPhoto.background.width === 960 && loadingPhoto.background.height === 720
      && loadingPhoto.image.width === 450 && loadingPhoto.image.height === 450
      && loadingPhoto.image.display !== "none" && loadingPhoto.image.source.startsWith("url("),
    `ctf_2fort loading photo presentation differs: ${JSON.stringify(loadingPhoto)}`)
    const loadingCapture = await captureInterface(session, config, "loading-ctf-2fort-1280x720")
    await agent(["--session", session, "wait", "--fn", "['Ready','Failed'].includes(document.querySelector('main').dataset.phase)", "--timeout", "600000"])
    const terminal = parseJson<{ phase: string; detail: string; gameui: string }>(await agent([
      "--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,detail:m.dataset.detail,gameui:m.dataset.gameui}})()",
    ]))
    require(terminal.phase === "Ready" && terminal.gameui === "in-game", `ctf_2fort failed to activate: ${JSON.stringify(terminal)}`)
    await admitInitialClassSelection(session)
    await agent(["--session", session, "wait", "--fn", "JSON.parse(document.querySelector('.world-canvas').dataset.staticProps||'null')?.total===2265", "--timeout", "30000"])
    const geometry = await captureFinalReadyGeometry(session, config, "ctf_2fort", 1, 1)
    const facts = parseJson<{
      staticProps: { total: number; main: number; sky3d: number; runtimeLit: number }
      visibleMain: number[]
      sky: { skySurfaces: number; skyProps: number; stateRestored: boolean } | null
      load: { totalMilliseconds: number; mapBytes: number; presentationBytes: number }
      target: string
    }>(await agent([
      "--session", session, "eval",
      "(()=>{const m=document.querySelector('main'),c=document.querySelector('.world-canvas');return{staticProps:JSON.parse(c.dataset.staticProps),visibleMain:JSON.parse(c.dataset.visibleMainStaticProps||'[]'),sky:c.dataset.sky3dPass?JSON.parse(c.dataset.sky3dPass):null,load:JSON.parse(m.dataset.loadPerformance),target:m.dataset.detail}})()",
    ]))
    require(facts.staticProps.total === 2265 && facts.staticProps.runtimeLit === 24
      && facts.staticProps.main === 2227 && facts.staticProps.sky3d === 38 && facts.visibleMain.length > 0
      && (facts.sky === null || (facts.sky.skySurfaces > 0 && facts.sky.skyProps > 0 && facts.sky.stateRestored))
      && facts.load.mapBytes < 128 * 1024 * 1024,
    `ctf_2fort configured world facts differ: ${JSON.stringify(facts)}`)
    const gameplay = await exerciseSwitchedGameplay(session, "ctf_2fort")
    const performance = parseJson<{ milliseconds: number; frames: number; frameHz: number; simulationHz: number; p95FrameMilliseconds: number }>(await agent([
      "--session", session, "eval",
      "(async()=>{const main=document.querySelector('main'),firstTick=Number(main.dataset.snapshotTick),started=performance.now(),frames=[];let previous=started;await new Promise(resolve=>{const next=now=>{frames.push(now-previous);previous=now;if(now-started>=5000)resolve();else requestAnimationFrame(next)};requestAnimationFrame(next)});const milliseconds=previous-started,sorted=[...frames].sort((a,b)=>a-b);return{milliseconds,frames:frames.length,frameHz:frames.length*1000/milliseconds,simulationHz:(Number(main.dataset.snapshotTick)-firstTick)*1000/milliseconds,p95FrameMilliseconds:sorted[Math.min(sorted.length-1,Math.floor(sorted.length*.95))]}})()",
    ]))
    require(performance.milliseconds >= 5000 && performance.frameHz >= 30
      && performance.simulationHz >= 60 && performance.simulationHz <= 75,
    `ctf_2fort headed frame or fixed-tick cadence differs: ${JSON.stringify(performance)}`)
    return Object.freeze({ target: "ctf_2fort", browser: version, startup, loadingPhoto, loadingCapture, facts, geometry, gameplay, performance })
  } catch (error) {
    primaryError = error
    const state = browserOpen
      ? await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main'),c=document.querySelector('.world-canvas');return{phase:m?.dataset.phase,detail:m?.dataset.detail,gameui:m?.dataset.gameui,tick:m?.dataset.snapshotTick,performance:m?.dataset.performance,simulation:m?.dataset.simulationProbe,static:c?.dataset.staticProps,visible:c?.dataset.visibleMainStaticProps,sky:c?.dataset.sky3dPass,console:document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.slice(-2000)}})()"]).catch(() => "unavailable")
      : "unavailable"
    const errors = browserOpen ? await agent(["--session", session, "errors"]).catch(() => "unavailable") : "unavailable"
    const console = browserOpen ? await agent(["--session", session, "console"]).catch(() => "unavailable") : "unavailable"
    throw new BrowserEvidenceError(`${error instanceof Error ? error.message : String(error)}; ctf_2fort state=${state}; errors=${errors.slice(-2000)}; console=${console.slice(-2000)}`)
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    try { await owner.interrupt() } catch (error) { if (primaryError === undefined) throw error }
  }
}

export async function verifyBrowserAcceptance(
  config: LocalConfig,
  target: string | undefined,
): Promise<Record<string, unknown>> {
  if (target === "ctf_2fort") return verifyCtf2fortBrowser(config)
  if (target === "pl_upward") return verifyPlUpwardBrowser(config)
  const version = await agent(["--version"])
  const session = `playsrc-acceptance-${process.pid}`
  const automation = tf2BrowserAutomation(session)
  const owner = await startDevelopmentProcess(target)
  let browserOpen = false
  let primaryError: unknown
  try {
    await agent(["--session", session, "--headed", "--webgpu", "--init-script", TF2_BROWSER_AUTOMATION_INIT, "open", owner.url])
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
    const menuPresentation = parseJson<{ random: { seed: number; draws: number }; character: string; consoleControls: number; characterVisible: boolean; characterBounds: number[]; characterCanvas: number[] }>(await agent([
      "--session", session, "eval",
      "(()=>{const m=document.querySelector('main'),character=document.querySelector('.gameui-layer [data-vgui-name=TFCharacterImage]'),canvas=character?.querySelector('canvas'),r=character?.getBoundingClientRect();return {random:JSON.parse(m.dataset.presentationRandomState),character:m.dataset.presentationCharacter,consoleControls:Array.from(document.querySelectorAll('.gameui-layer [data-vgui-name]')).filter(x=>/console/i.test(x.dataset.vguiName||'')).length,characterVisible:!!character&&getComputedStyle(character).display!=='none'&&getComputedStyle(character).visibility!=='hidden',characterBounds:r?[r.x,r.y,r.width,r.height]:[],characterCanvas:canvas?[canvas.width,canvas.height]:[]}})()",
    ]))
    require(menuPresentation.random.seed === 0 && menuPresentation.random.draws === 1
      && menuPresentation.character !== "unavailable" && menuPresentation.consoleControls === 0
      && menuPresentation.characterVisible && menuPresentation.characterBounds[2]! > 0 && menuPresentation.characterBounds[3]! > 0
      && menuPresentation.characterCanvas[0]! > 0 && menuPresentation.characterCanvas[1]! > 0,
    `Main Menu presentation selection differs: ${JSON.stringify(menuPresentation)}`)
    const gameUiBaseBackground = parseJson<{ count: number; identity: string | null; material: string | null; materialSha256: string | null; texture: string | null; textureSha256: string | null; source: string | null; sourceSha256: string | null; backgroundName: string | null; bounds: number[]; rasterBounds: number[]; opacity: string; baseZ: number; overrideZ: number }>(await agent([
      "--session", session, "eval",
      "(()=>{const layers=[...document.querySelectorAll('[data-tf2-gameui-base-background]')],base=layers[0],raster=base?.querySelector('canvas[data-vgui-raster=image-raster]'),menu=document.querySelector('[data-vgui-name=MainMenuOverride]'),rect=base?base.getBoundingClientRect():null,rasterRect=raster?.getBoundingClientRect(),style=base?getComputedStyle(base):null;return{count:layers.length,identity:base?.getAttribute('data-source-image')??null,material:base?.getAttribute('data-source-material')??null,materialSha256:base?.getAttribute('data-source-material-sha256')??null,texture:base?.getAttribute('data-source-texture')??null,textureSha256:base?.getAttribute('data-source-texture-sha256')??null,source:base?.getAttribute('data-source-list')??null,sourceSha256:base?.getAttribute('data-source-list-sha256')??null,backgroundName:base?.getAttribute('data-background-name')??null,bounds:rect?[rect.x,rect.y,rect.width,rect.height]:[],rasterBounds:rasterRect?[rasterRect.x,rasterRect.y,rasterRect.width,rasterRect.height]:[],opacity:style?.opacity??'',baseZ:base?Number(style.zIndex):NaN,overrideZ:Number(getComputedStyle(menu).zIndex)}})()",
    ]))
    require(gameUiBaseBackground.count === 1 && gameUiBaseBackground.identity === "../console/background_2fort_widescreen"
      && gameUiBaseBackground.material === "materials/console/background_2fort_widescreen.vmt"
      && gameUiBaseBackground.texture === "materials/console/background_2fort_widescreen.vtf"
      && gameUiBaseBackground.source === "scripts/chapterbackgrounds.txt" && gameUiBaseBackground.backgroundName === "background_2fort"
      && gameUiBaseBackground.materialSha256 === "62ea66916136838dec8b843437c21bb3c24cbc5811b00af4f253043156d7ba65"
      && gameUiBaseBackground.textureSha256 === "da391abcb6d121dea3786c16014f216cdbbcaf0d5810aa3ef395341f601ddcec"
      && gameUiBaseBackground.sourceSha256 === "9d24d5870425b7a793583e95db933bd66aec51495840c5a97d3278566048cc58"
      && JSON.stringify(gameUiBaseBackground.bounds) === JSON.stringify([0, 0, 1280, 720])
      && JSON.stringify(gameUiBaseBackground.rasterBounds) === JSON.stringify([0, 0, 1280, 720]) && gameUiBaseBackground.opacity === "1"
      && Number.isFinite(gameUiBaseBackground.baseZ) && gameUiBaseBackground.baseZ < gameUiBaseBackground.overrideZ,
    `ordinary GameUI base background is absent: ${JSON.stringify(gameUiBaseBackground)}`)
    const desktopGameUiBackground = await captureGameUiBackground(session, config, "gameui-background-1280x720")
    require(desktopGameUiBackground.nonBlackSamples >= 7, `ordinary GameUI background pixels are blank: ${JSON.stringify(desktopGameUiBackground)}`)

    await agent(["--session", session, "set", "viewport", "1192", "1339"])
    await agent(["--session", session, "wait", "--fn", "(()=>{const r=document.querySelector('[data-vgui-name=MainMenuOverride]').getBoundingClientRect();return r.width===1192&&r.height===1339})()", "--timeout", "30000"])
    const tallMenu = parseJson<{ root: number[]; menu: number[]; character: number[]; bottom: number; overflow: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const rect=e=>{const r=e.getBoundingClientRect();return[r.x,r.y,r.width,r.height]},root=document.querySelector('.gameui-layer'),menu=document.querySelector('[data-vgui-name=MainMenuOverride]'),character=document.querySelector('[data-vgui-name=TFCharacterImage]'),bottom=Math.max(...[...document.querySelectorAll('[data-vgui-name=SettingsButton],[data-vgui-name=TF2SettingsButton],[data-vgui-name=QuitButton]')].map(e=>e.getBoundingClientRect().bottom));return{root:rect(root),menu:rect(menu),character:rect(character),bottom,overflow:getComputedStyle(menu).overflow}})()",
    ]))
    require(JSON.stringify(tallMenu.root) === JSON.stringify([0, 0, 1192, 1339])
      && JSON.stringify(tallMenu.menu) === JSON.stringify([0, 0, 1192, 1339])
      && tallMenu.character[0]! < 1192 && tallMenu.character[1]! < 1339 && tallMenu.character[0]! + tallMenu.character[2]! > 0 && tallMenu.character[1]! + tallMenu.character[3]! > 0
      && tallMenu.bottom > 0 && tallMenu.bottom <= 1339 && tallMenu.overflow === "hidden",
    `1192x1339 proportional Main Menu differs: ${JSON.stringify(tallMenu)}`)
    const tallMenuCapture = await captureInterface(session, config, "main-menu-1192x1339")
    const tallGameUiBackground = await captureGameUiBackground(session, config, "gameui-background-1192x1339")
    require(tallGameUiBackground.nonBlackSamples >= 7, `tall GameUI background pixels are blank: ${JSON.stringify(tallGameUiBackground)}`)
    const menuViewportMatrix: Array<{ width: number; height: number; devicePixelRatio: number; menu: number[]; bottom: number; characterImage: string | null; backgroundImage: string | null; backgroundMaterialSha256: string | null; backgroundTextureSha256: string | null }> = []
    for (const [width, height, scale] of [[1280, 720, 1], [1024, 768, 1], [2560, 1080, 1], [390, 844, 1], [844, 390, 1], [1280, 720, 2], [1280, 720, 1]] as const) {
      await agent(["--session", session, "set", "viewport", String(width), String(height), String(scale)])
      await agent(["--session", session, "wait", "--fn", `(()=>{const r=document.querySelector('[data-vgui-name=MainMenuOverride]').getBoundingClientRect();return r.width===${width}&&r.height===${height}&&devicePixelRatio===${scale}})()`, "--timeout", "30000"])
      const observation = parseJson<{ devicePixelRatio: number; menu: number[]; bottom: number; characterImage: string | null; backgroundImage: string | null; backgroundMaterialSha256: string | null; backgroundTextureSha256: string | null }>(await agent([
        "--session", session, "eval",
        "(()=>{const r=document.querySelector('[data-vgui-name=MainMenuOverride]').getBoundingClientRect(),bottom=Math.max(...[...document.querySelectorAll('[data-vgui-name=SettingsButton],[data-vgui-name=TF2SettingsButton],[data-vgui-name=QuitButton]')].map(e=>e.getBoundingClientRect().bottom)),background=document.querySelector('[data-tf2-gameui-base-background]');return{devicePixelRatio,menu:[r.x,r.y,r.width,r.height],bottom,characterImage:document.querySelector('[data-vgui-name=TFCharacterImage]')?.getAttribute('data-vgui-image')??document.querySelector('main').dataset.presentationCharacter??null,backgroundImage:background?.getAttribute('data-source-image')??null,backgroundMaterialSha256:background?.getAttribute('data-source-material-sha256')??null,backgroundTextureSha256:background?.getAttribute('data-source-texture-sha256')??null}})()",
      ]))
      require(JSON.stringify(observation.menu) === JSON.stringify([0, 0, width, height])
        && observation.bottom > 0 && observation.bottom <= height && observation.devicePixelRatio === scale
        && observation.characterImage === menuPresentation.character
        && observation.backgroundImage === `../console/background_2fort${width / height >= 1.5999 ? "_widescreen" : ""}`
        && observation.backgroundMaterialSha256 === (width / height >= 1.5999 ? "62ea66916136838dec8b843437c21bb3c24cbc5811b00af4f253043156d7ba65" : "4fa05e0ddbf5da835ea7e1d70872775d91e56e918ad2f36ae4b24c4cb62afcc3")
        && observation.backgroundTextureSha256 === (width / height >= 1.5999 ? "da391abcb6d121dea3786c16014f216cdbbcaf0d5810aa3ef395341f601ddcec" : "c6311965e57125bec0a98de320c4cd4ef7b297c874f47acb40107f0d31d911d9"),
      `proportional Main Menu resize differs at ${width}x${height}@${scale}: ${JSON.stringify(observation)}`)
      menuViewportMatrix.push({ width, height, ...observation })
    }


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
    await installRocketGpuEvidence(session)
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
    await automation.maps.load("jump_beef")
    try {
      await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.gameui==='loading'&&document.querySelector('.loading-layer [data-vgui-name=LoadingDialog]')", "--timeout", "30000"])
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,gameui:m.dataset.gameui,detail:m.dataset.detail,loading:m.dataset.loadingStatus,body:document.body.innerText.slice(0,500)}})()"])
      throw new BrowserEvidenceError(`${String(error)}; loading state: ${state}`)
    }
    const loadingPresentation = parseJson<{ background: number[]; dialog: number[]; progress: number; status: string; disposition: string; gameUiBaseDisplay: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const rect=n=>{const r=document.querySelector(`.loading-layer [data-vgui-name=${n}]`).getBoundingClientRect();return[r.x,r.y,r.width,r.height]},m=document.querySelector('main');return{background:rect('Background'),dialog:rect('LoadingDialog'),progress:Number(m.dataset.loadingProgress),status:m.dataset.loadingStatus,disposition:m.dataset.loadingBackground,gameUiBaseDisplay:getComputedStyle(document.querySelector('[data-tf2-gameui-base-background]')).display}})()",
    ]))
    require(Math.abs(loadingPresentation.background[0]!) < 0.001 && Math.abs(loadingPresentation.background[1]!) < 0.001
      && loadingPresentation.background[2] === 1125 && Math.abs(loadingPresentation.background[3]! - 844) < 0.001
      && JSON.stringify(loadingPresentation.dialog) === JSON.stringify([0, 722, 380, 112])
      && loadingPresentation.progress >= 0 && loadingPresentation.progress < 1
      && loadingPresentation.status.length > 0 && loadingPresentation.disposition === "configured-generic" && loadingPresentation.gameUiBaseDisplay === "none",
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
    await admitInitialClassSelection(session)
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
    require(modelMatrices.length === 36 && new Set(modelMatrices.map((value) => value.entity)).size === 36
      && modelMatrices.filter((value) => value.model === "models/items/ammopack_large.mdl").length === 3,
      "exact dynamic-prop and authored ammo-pack occurrence matrix set differs")
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

    const pointerLockEvidence = await automation.pointer.capture("direction")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.detail === 'Audio running'", "--timeout", "10000"])
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    const pointerMovement=parseJson<string>(await agent(["--session",session,"eval","document.querySelector('main').dataset.pointerMovement"]));require(pointerMovement==="raw"||pointerMovement==="adjusted",`pointer movement mode is unavailable: ${pointerMovement}`)
    const beforePointer = await cameraObservation(session)
    const afterHorizontal = await automation.player.lookBy({ x: 64, y: 0 })
    const afterVertical = await automation.player.lookBy({ x: 0, y: 32 })
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
      try {
        await agent([
          "--session",
          session,
          "wait",
          "--fn",
          `document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded jump_beef; generation ${generation}')`,
          "--timeout",
          "300000",
        ])
      } catch (error) {
        const state = await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main'),console=document.querySelector('.developer-layer [data-vgui-service=developer-console]');return{phase:main.dataset.phase,detail:main.dataset.detail,console:console?.textContent.slice(-1000)}})()"])
        throw new BrowserEvidenceError(`${String(error)}; HDR replacement state: ${state}`)
      }
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
    const movingSpeed = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('main').dataset.wishSpeed)",
      ]),
    )
    await agent([
      "--session",
      session,
      "eval",
      "window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW',key:'w',bubbles:true})); true",
    ])
    require(movingSpeed > 0, "movement binding did not advance the player")
    await automation.player.jump()
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

    await automation.player.settle(3)
    const initialFireEvents = parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.fireEvents)"]),
    )
    const initialExplosionEvents = parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.explosionEvents)"]),
    )
    await automation.pointer.capture("soldier")
    await automation.player.lookBy({ x: -182, y: 0 })
    try {
      await agent(["--session", session, "wait", "--fn", "Math.abs(((Number(document.querySelector('main').dataset.cameraYaw)-192+540)%360)-180)<2", "--timeout", "10000"])
    } catch (error) {
      const camera = await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main');return{yaw:m.dataset.cameraYaw,pitch:m.dataset.cameraPitch,pointer:m.dataset.pointerLocked,focus:document.hasFocus()}})()"])
      throw new BrowserEvidenceError(`${String(error)}; uninterrupted rocket-wall camera state: ${camera}`)
    }
    const stockCamera = await cameraObservation(session)
    await agent(["--session", session, "eval", "globalThis.__playsrcProfile??={};globalThis.__playsrcProfile.geometryEvidenceRevision=157001;true"])
    await agent(["--session", session, "wait", "--fn", "globalThis.__playsrcProfile?.geometryEvidence?.revision===157001", "--timeout", "30000"])
    const rocketWorld = parseJson<{ camera: { position: readonly number[] }; geometry: { samples: readonly RocketWorldSample[] } }>(
      await agent(["--session", session, "eval", "({camera:globalThis.__playsrcProfile.geometryEvidence.camera,geometry:globalThis.__playsrcProfile.geometryEvidence.geometry})"]),
    )
    const impactWall = rocketWorld.geometry.samples.find((sample) => sample.x === 0 && sample.y === 0)
    require(impactWall?.disposition === "main-world" && impactWall.depth !== null && impactWall.material !== null,
      `fixed rocket target does not expose one depth-tested center world surface: ${JSON.stringify(impactWall)}`)
    const beforeRocketState = await rocketFrameState(session)
    const beforeRocketCanvas = await captureCanvas(session, config, "jump_beef", ROCKET_VISUAL_REGIONS)
    await agent(["--session", session, "eval", "(()=>{const e=globalThis.__playsrcRocketGpuEvidence;e.draws=[];e.transactions=[];e.outputs=[];e.passes=[];e.impact=null;e.armed=true;return true})()"])
    let farFlightCanvas: CanvasEvidence | null = null
    let impactCanvas: CanvasEvidence | null = null
    await automation.player.pressPrimaryFire()
    await agent(["--session", session, "wait", "--fn", `document.querySelector('main').dataset.phase==='Failed'||Number(document.querySelector('main').dataset.fireEvents)>${initialFireEvents}`, "--timeout", "30000"])
    const fireAdmission=parseJson<{phase:string;detail:string}>(await agent(["--session",session,"eval","(()=>{const m=document.querySelector('main');return{phase:m.dataset.phase,detail:m.dataset.detail??m.textContent??''}})()"]))
    require(fireAdmission.phase!=="Failed",`rocket fire presentation failed: ${fireAdmission.detail}`)
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.projectiles)>0&&Number(document.querySelector('main').dataset.particleItems)>0", "--timeout", "30000"])
    const farFlightState = await rocketFrameState(session)
    farFlightCanvas = await captureCanvas(session, config, "jump_beef", ROCKET_VISUAL_REGIONS)
    await agent(["--session", session, "wait", "--fn", `Number(document.querySelector('main').dataset.explosionEvents)>${initialExplosionEvents}&&Number(document.querySelector('main').dataset.particleItems)>0`, "--timeout", "30000"])
    const impactState = await rocketFrameState(session)
    impactCanvas = await captureCanvas(session, config, "jump_beef", ROCKET_VISUAL_REGIONS)
    const impactParticleProbe = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.particleProbe??''"]))
    await automation.player.releasePrimaryFire()
    await agent(["--session", session, "wait", "--fn", `Number(document.querySelector('main').dataset.snapshotTick)>=${impactState.tick + 6}&&Number(document.querySelector('main').dataset.projectiles)===0&&Number(document.querySelector('main').dataset.particleItems)>0`, "--timeout", "30000"])
    const lateSmokeState = await rocketFrameState(session)
    const lateSmokeCanvas = await captureCanvas(session, config, "jump_beef", ROCKET_VISUAL_REGIONS)
    const rocketGpu = parseJson<{
      pipelines: readonly Record<string, unknown>[]
      draws: readonly Readonly<{ count: number; indices: number; pipeline: Record<string, unknown> | null }>[]
      totalDraws: number
      impact: Readonly<{ origin: readonly number[]; normal: readonly number[]; wall: readonly number[] }> | null
      transactions: readonly Readonly<{ from: number; to: number; events: readonly Record<string, unknown>[] }>[]
      outputs: readonly Readonly<{ tick: number; count: number; materials: readonly Record<string, unknown>[] }>[]
      passes: readonly Readonly<{ colorLoad: readonly string[]; depthLoad: string | null; operations: readonly Record<string, unknown>[] }>[]
    }>(await agent([
      "--session", session, "eval",
      "(()=>{const e=globalThis.__playsrcRocketGpuEvidence;e.armed=false;const grouped=new Map();for(const draw of e.draws){const key=JSON.stringify({indices:draw.indices,pipeline:draw.pipeline});const prior=grouped.get(key);if(prior)prior.count++;else grouped.set(key,{count:1,indices:draw.indices,pipeline:draw.pipeline})}return{pipelines:e.pipelines.filter(pipeline=>pipeline.blend.some(Boolean)).slice(0,16),draws:[...grouped.values()].slice(0,48),totalDraws:e.draws.length,impact:e.impact,transactions:e.transactions,outputs:e.outputs.filter(output=>output.count>0).slice(-16),passes:e.passes.filter(pass=>pass.operations.some(operation=>operation.kind==='particle')).slice(-8)}})()",
    ]))
    const beforeRocketImage = await retainedCanvas(config, beforeRocketCanvas)
    const flightPlane = rocketPixelPlane(beforeRocketImage, await retainedCanvas(config, farFlightCanvas), ROCKET_IMPACT_REGION)
    const impactImage = await retainedCanvas(config, impactCanvas)
    const impactPlane = rocketPixelPlane(beforeRocketImage, impactImage, ROCKET_IMPACT_REGION)
    const impactSurfacePlane = rocketPixelPlane(beforeRocketImage, impactImage, ROCKET_IMPACT_SURFACE_REGION)
    const lateSmokeImage = await retainedCanvas(config, lateSmokeCanvas)
    const lateSmokePlane = rocketPixelPlane(beforeRocketImage, lateSmokeImage, ROCKET_IMPACT_REGION)
    const lateSmokeSurfacePlane = rocketPixelPlane(beforeRocketImage, lateSmokeImage, ROCKET_IMPACT_SURFACE_REGION)
    const foregroundSamples = rocketWorld.geometry.samples.filter((sample) => {
      if (sample.disposition !== "main-world" || sample.depth === null || sample.depth + 8 >= impactWall.depth!) return false
      const x = (sample.x + 1) * VIEWPORT_WIDTH / 2, y = (1 - sample.y) * VIEWPORT_HEIGHT / 2
      return x <= VIEWPORT_WIDTH / 2 && y < 540 && Math.hypot(x - VIEWPORT_WIDTH / 2, y - VIEWPORT_HEIGHT / 2) >= 200
    })
    const occlusionPlanes = foregroundSamples.map((sample, index) => {
      const x = Math.round((sample.x + 1) * VIEWPORT_WIDTH / 2), y = Math.round((1 - sample.y) * VIEWPORT_HEIGHT / 2)
      const region = { name: `intervening-wall-${index}`, x: x - 6, y: y - 6, width: 12, height: 12 }
      return Object.freeze({ surface: sample, ...rocketPixelPlane(beforeRocketImage, impactImage, region) })
    })
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.projectiles)===0&&Number(document.querySelector('main').dataset.particleItems)===0", "--timeout", "30000"])
    const extinctionState = await rocketFrameState(session)
    const impactProjection = rocketGpu.impact ? projectSourcePoint(stockCamera, rocketGpu.impact.origin) : null
    const rocketVisibilityEvidence = Object.freeze({
      contentBuild: TF2_CONTENT_BUILD.contentBuild,
      viewport: Object.freeze([VIEWPORT_WIDTH, VIEWPORT_HEIGHT]),
      camera: stockCamera,
      impactedWall: impactWall,
      impactProjection,
      states: Object.freeze({ before: beforeRocketState, flight: farFlightState, impact: impactState, lateSmoke: lateSmokeState, extinction: extinctionState }),
      captures: Object.freeze({ before: beforeRocketCanvas, flight: farFlightCanvas, impact: impactCanvas, lateSmoke: lateSmokeCanvas }),
      colorPlanes: Object.freeze({
        flight: flightPlane, impact: impactPlane, impactedSurface: impactSurfacePlane,
        lateSmoke: lateSmokePlane, lateSmokeSurface: lateSmokeSurfacePlane,
        interveningWalls: Object.freeze(occlusionPlanes),
      }),
      gpu: rocketGpu,
    })
    const rocketEvidenceDirectory = path.join(config.sourceCacheDir, "evidence", "browser", "jump_beef", "rocket-visibility")
    await mkdir(rocketEvidenceDirectory, { recursive: true })
    const rocketEvidencePath = path.join(rocketEvidenceDirectory, "report.json")
    await writeFile(rocketEvidencePath, `${JSON.stringify(rocketVisibilityEvidence, null, 2)}\n`)
    const rocketFailures: string[] = []
    if (farFlightState.fireEvents <= beforeRocketState.fireEvents || impactState.explosionEvents <= beforeRocketState.explosionEvents)
      rocketFailures.push("fire/explosion Source event order")
    if (!rocketGpu.impact || Math.abs(Math.hypot(...rocketGpu.impact.normal) - 1) > 0.0001
      || Math.abs(Math.hypot(...rocketGpu.impact.origin.map((value, index) => value - rocketGpu.impact!.wall[index]!)) - 1) > 0.0001)
      rocketFailures.push(`Source one-unit wall-normal producer placement=${JSON.stringify(rocketGpu.impact)}`)
    if (!impactProjection || impactProjection.x < ROCKET_IMPACT_SURFACE_REGION.x - 1
      || impactProjection.x > ROCKET_IMPACT_SURFACE_REGION.x + ROCKET_IMPACT_SURFACE_REGION.width + 1
      || impactProjection.y < ROCKET_IMPACT_SURFACE_REGION.y - 1
      || impactProjection.y > ROCKET_IMPACT_SURFACE_REGION.y + ROCKET_IMPACT_SURFACE_REGION.height + 1
      || Math.abs(impactWall.depth! - impactProjection.depth) > 2)
      rocketFailures.push(`actual rocket contact does not project onto the sampled opaque wall=${JSON.stringify(impactProjection)}`)
    const producerEvents = rocketGpu.transactions.flatMap((transaction) => transaction.events)
    const trailStart = producerEvents.findIndex((event) => event.kind === "start" && event.system === "rockettrail")
    const gracefulStop = producerEvents.findIndex((event) => event.kind === "graceful-stop")
    const wallStart = producerEvents.findIndex((event) => event.kind === "start" && event.system === "ExplosionCore_Wall")
    const trailControls = producerEvents.filter((event) => event.kind === "set-control-point")
    if (trailStart < 0 || gracefulStop <= trailStart || wallStart <= gracefulStop || trailControls.length < 2)
      rocketFailures.push(`rocket attachment/trail/explosion lifecycle=${JSON.stringify(producerEvents)}`)
    if (!["effects/debris/debris_chunk.vmt", "effects/smokelit2/smoke2lit.vmt", "effects/sc_brightglow_y_nomodel.vmt"]
      .every((material) => impactState.materials.includes(material)))
      rocketFailures.push(`authored impact flash/debris/smoke materials=${impactState.materials}`)
    if (!rocketGpu.outputs.some((output) => output.materials.some((material) => Number(material.front) > 0)))
      rocketFailures.push("no authored particle producer position reached the front of the impacted wall")
    const particleDraws = rocketGpu.draws.filter((draw) =>
      JSON.stringify(draw.pipeline?.vertexStrides) === JSON.stringify([8, 4, 8, 16, 12]))
    if (particleDraws.length === 0 || particleDraws.some((draw) =>
      draw.pipeline?.depthTest !== "less-equal" || draw.pipeline.depthWrite !== false))
      rocketFailures.push(`particle pipelines weakened Source opaque-world depth state=${JSON.stringify(particleDraws)}`)
    if (foregroundSamples.length === 0) rocketFailures.push("no nearer opaque-world negative occlusion control")
    if (!rocketGpu.passes.some((pass) => {
      const world = pass.operations.findIndex((operation) => operation.kind === "world-bundles")
      const particle = pass.operations.findIndex((operation) => operation.kind === "particle")
      return world >= 0 && particle > world
    })) rocketFailures.push(`opaque world GPU bundle was not submitted before translucent particles=${JSON.stringify(rocketGpu.passes)}`)
    if (lateSmokeState.tick <= impactState.tick || lateSmokeState.particleItems === 0 || extinctionState.particleItems !== 0)
      rocketFailures.push("graceful post-impact smoke lifetime")
    if (impactPlane.warmFlashPixels < 8) rocketFailures.push(`front-of-wall warm flash pixels=${impactPlane.warmFlashPixels}`)
    if (impactPlane.brightenedPixels < 24) rocketFailures.push(`front-of-wall bright impact pixels=${impactPlane.brightenedPixels}`)
    if (impactSurfacePlane.changedPixels < 8)
      rocketFailures.push(`impact pixels on the actual opaque hit surface=${impactSurfacePlane.changedPixels}; nearby doorway pixels=${impactPlane.changedPixels}`)
    if (lateSmokeSurfacePlane.changedPixels < 8)
      rocketFailures.push(`late smoke/debris pixels on the actual opaque hit surface=${lateSmokeSurfacePlane.changedPixels}`)
    if (lateSmokePlane.smokePixels < 12 && lateSmokePlane.debrisPixels < 12)
      rocketFailures.push(`front-of-wall late smoke/debris pixels=${lateSmokePlane.smokePixels}/${lateSmokePlane.debrisPixels}`)
    if (occlusionPlanes.some((plane) => plane.warmFlashPixels > 0))
      rocketFailures.push(`warm particles leaked through nearer opaque world geometry=${JSON.stringify(occlusionPlanes.map((plane) => plane.warmFlashPixels))}`)
    require(rocketFailures.length === 0,
      `headed rocket wall-pixel/depth regression failed: ${rocketFailures.join("; ")}; evidence ${rocketEvidencePath}`)
    const firePhase=parseJson<string>(await agent(["--session",session,"eval","document.querySelector('main').dataset.phase"]));if(firePhase==="Failed"){const state=await agent(["--session",session,"eval","({text:document.body.innerText,dataset:{...document.querySelector('main').dataset}})"]);throw new BrowserEvidenceError(`Soldier held fire failed: ${state}`)}
    await automation.player.lookBy({ x: 0, y: 2000 })
    await agent(["--session", session, "wait", "--fn", "Number(document.querySelector('main').dataset.cameraPitch)>=80", "--timeout", "30000"])
    const downwardViewmodelCanvas = await captureCanvas(session, config)
    const downwardDepthIsolated = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.viewmodelWorldDepthIsolated??''"]))
    require(downwardDepthIsolated === "true" && downwardViewmodelCanvas.sha256 !== "559b1ae7a0e1749253214494256a65b010cb012172944660391c68667dbd7f49",
      `downward viewmodel remains world-depth occluded: ${downwardViewmodelCanvas.sha256}`)
    let hudAnimationTrace = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudAnimationTrace??''"]))
    let animationFireEvents = parseJson<number>(await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.fireEvents)"]))
    for (let attempt = 0; attempt < 4 && !hudAnimationTrace.includes("HudHealthDyingPulse"); attempt += 1) {
      await automation.player.pressPrimaryFire()
      await agent(["--session", session, "wait", "--fn", `Number(document.querySelector('main').dataset.fireEvents)>${animationFireEvents}`, "--timeout", "60000"])
      await automation.player.releasePrimaryFire()
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
    require(farFlightCanvas !== null && impactCanvas !== null, "Soldier PCF render data was not observed")
    require(farFlightCanvas.regions.find((region) => region.name === "forward-wall")!.warmParticlePixels >= 4,
      `far-flight rocket-trail pixels are absent: ${JSON.stringify(farFlightCanvas.regions)}`)
    require(impactParticleProbe.includes("effects/debris/debris_chunk.vmt") && impactParticleProbe.includes("effects/smokelit2/smoke2lit.vmt"),
      `wall-impact Particle children are absent from the displayed frame: ${impactParticleProbe}`)
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
    await automation.player.selectClass("demoman")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Class selection queued: demoman')", "--timeout", "30000"])
    await agent(["--session", session, "press", "Backquote"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='Failed'||document.querySelector('main').dataset.hudProbe?.split(':')[1]==='4'", "--timeout", "120000"])
    const classPhase = parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.phase"]))
    if (classPhase === "Failed") throw new BrowserEvidenceError(`Demoman transition failed: ${await agent(["--session", session, "eval", "({text:document.body.innerText,dataset:{...document.querySelector('main').dataset}})"])}`)
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.hudProbe"]))
      .split(":")[1] === "4", "Demoman HUD class binding failed")
    await agent(["--session", session, "press", "2"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.hudProbe?.split(':')[2]==='3'", "--timeout", "30000"])
    await automation.pointer.capture("sticky")
    await automation.player.pressPrimaryFire()
    await agent(["--session",session,"wait","--fn","document.querySelector('main').dataset.unsupportedState==='StickyPhysicsSolverUnavailable'","--timeout","30000"])
    await automation.player.releasePrimaryFire()
    const stickyLaunch = parseJson<{ fire: number; projectiles:number; unsupported:string; phase: string }>(await agent([
      "--session", session, "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {fire:Number(d.fireEvents),projectiles:Number(d.projectiles),unsupported:d.unsupportedState,phase:d.phase}})()",
    ]))
    require(stickyLaunch.projectiles===0&&stickyLaunch.unsupported==="StickyPhysicsSolverUnavailable"&&stickyLaunch.phase==="Ready",`unsupported sticky Physics state was not atomic: ${JSON.stringify(stickyLaunch)}`)
    const supportBlockerItems = parseJson<string[]>(await agent([
      "--session", session, "eval", "JSON.parse(document.querySelector('main').dataset.blockers)",
    ]))
    blockerCount = supportBlockerItems.length
    await automation.pointer.release()
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
      blockerPartition.authorityBehavior.some((blocker) => blocker.includes("Tempus core and configured Jump course contract unavailable")),
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
    require(parseJson<string>(await agent(["--session", session, "eval", "getComputedStyle(document.querySelector('[data-tf2-gameui-base-background]')).display"])) === "none",
      "ordinary GameUI base background remained visible in pause")
    await mouseClickVguiPanel(session, "DisconnectButton")
    try {
      await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='MainMenu'&&document.querySelector('main').dataset.gameplayInitialized==='false'", "--timeout", "300000"])
    } catch (error) {
      const state = await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main');return{url:location.href,body:document.body.innerText.slice(0,300),phase:main?.dataset.phase??null,gameui:main?.dataset.gameui??null,gameplay:main?.dataset.gameplayInitialized??null,detail:main?.dataset.detail??null,options:main?.dataset.optionsVisible??null}})()"])
      throw new BrowserEvidenceError(`${String(error)}; disconnect state=${state}`)
    }
    const returnStartup = parseJson<{ state: string; display: string; background: string; backgroundDisplay: string }>(await agent(["--session", session, "eval", "(()=>({state:document.querySelector('main').dataset.startupState,display:getComputedStyle(document.querySelector('.startup-layer')).display,background:document.querySelector('[data-tf2-gameui-base-background]').getAttribute('data-source-image'),backgroundDisplay:getComputedStyle(document.querySelector('[data-tf2-gameui-base-background]')).display}))()"] ))
    require(returnStartup.state === "Skipped" && returnStartup.display === "none"
      && returnStartup.background === "../console/background_2fort_widescreen" && returnStartup.backgroundDisplay !== "none",
    `startup or GameUI background return differs after disconnect: ${JSON.stringify(returnStartup)}`)

    const coldCacheInventory = await cacheInventory(session)
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
    await admitInitialClassSelection(session)
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('main').dataset.cameraPosition && document.querySelector('main').dataset.hudProbe",
      "--timeout",
      "30000",
    ])
    const warmCache = parseJson<{ cache: string; performance: string | null }>(await agent(["--session", session, "eval", "(()=>{const main=document.querySelector('main');return{cache:main.dataset.cache,performance:main.dataset.loadPerformance??null}})()"] ))
    const warmCacheInventory = await cacheInventory(session)
    require(warmCache.cache === "hit", `warm browser run did not reuse the derived payload: ${JSON.stringify({ ...warmCache, coldCacheInventory, warmCacheInventory })}`)
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
      "(()=>{const s={done:false,error:null,result:null};globalThis.__playsrcIdbEvidence=s;s.open=indexedDB.open('playsrc-derived-v3',1);s.open.onerror=()=>{s.error=String(s.open.error);s.done=true};s.open.onsuccess=()=>{try{s.database=s.open.result;s.transaction=s.database.transaction('objects');s.request=s.transaction.objectStore('objects').getAll();s.request.onerror=()=>{s.error=String(s.request.error);s.done=true};s.request.onsuccess=()=>{s.result=s.request.result.map(x=>({key:x.key,byteLength:x.byteLength,sha256:x.sha256}));s.done=true}}catch(error){s.error=String(error);s.done=true}};return true})()",
    ])
    await agent(["--session", session, "wait", "--fn", "globalThis.__playsrcIdbEvidence?.done===true", "--timeout", "30000"])
    const records = parseJson<Array<{ key: string; byteLength: number; sha256: string }>>(
      await agent(["--session", session, "eval", "(()=>{const s=globalThis.__playsrcIdbEvidence;if(s.error)throw new Error(s.error);return s.result})()"]),
    )
    const mapRecords = records.filter(
      (record) =>
        record.sha256 === "735995d68920adcb971fe4c5e773986f438c2a95c07c935882dc7fd081ce1e3a" ||
        record.sha256 === "56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156",
    )
    require(mapRecords.length >= 1 && mapRecords.length <= 2 && new Set(mapRecords.map((record) => record.sha256)).size === mapRecords.length &&
      mapRecords.some(
        (record) =>
          record.byteLength === 78_255_714 &&
          record.sha256 === "735995d68920adcb971fe4c5e773986f438c2a95c07c935882dc7fd081ce1e3a",
      ) && mapRecords.every((record) =>
        record.sha256 === "735995d68920adcb971fe4c5e773986f438c2a95c07c935882dc7fd081ce1e3a"
          ? record.byteLength === 78_255_714
          : record.byteLength === 42_082_929), `warm active IndexedDB record identity differs: ${JSON.stringify(mapRecords)}`)
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
      pointerLock: `acquired-${pointerMovement}-and-released-for-console`,
      pointerLockAutomation: pointerLockEvidence,
      console: platformFontSupported
        ? "history-completion-focus-repeated-visibility-replacement-close-passed"
        : "unsupported-platform-fonts-suppressed-paint-and-input",
      gameUi: { menuPresentation, gameUiBaseBackground, desktopGameUiBackground, tallMenu, tallMenuCapture, tallGameUiBackground, menuViewportMatrix, mobileInterface },
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
      farFlightCanvas,
      impactCanvas,
      impactParticleProbe,
      rocketVisibilityEvidence: Object.freeze({
        report: rocketEvidencePath,
        impactSurfacePixels: impactSurfacePlane.changedPixels,
        lateSmokeSurfacePixels: lateSmokeSurfacePlane.changedPixels,
        impactOrigin: rocketGpu.impact?.origin,
        impactNormal: rocketGpu.impact?.normal,
        impactProjection,
      }),
      downwardViewmodelCanvas,
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

async function exerciseSwitchedGameplay(session:string,target:string):Promise<Readonly<{target:string;firstTick:number;lastTick:number;distance:number}>>{
  const automation=tf2BrowserAutomation(session)
  await automation.console.submitCommand("noclip")
  await agent(["--session",session,"press","Backquote"])
  await agent(["--session",session,"wait","--fn","Number(document.querySelector('main').dataset.movementMode)===1","--timeout","30000"])
  let movement=await automation.player.walkForward(8)
  if(movement.distance<=1)movement=await automation.player.walkForward(8)
  require(movement.lastTick>=movement.firstTick+8&&movement.distance>1,`${target} post-activation gameplay did not advance: ${JSON.stringify(movement)}`)
  await agent(["--session",session,"press","Backquote"])
  return Object.freeze({target,firstTick:movement.firstTick,lastTick:movement.lastTick,distance:movement.distance})
}

async function captureFinalReadyGeometry(session:string,config:LocalConfig,target:string,generation:number,revision:number):Promise<Readonly<Record<string,unknown>>>{
  await agent(["--session",session,"eval",`globalThis.__playsrcProfile??={};globalThis.__playsrcProfile.geometryEvidenceRevision=${revision};true`])
  await agent(["--session",session,"wait","--fn",`globalThis.__playsrcProfile?.geometryEvidence?.revision===${revision}`,"--timeout","30000"])
  const evidence=parseJson<{revision:number;generation:number;target:string;finalReady:boolean;identities:{bsp:string;resourceRoot:string;contentBuild:string;graphTarget:string;wasm:string;simulationTick:string};camera:{near:number};visibility:{outsideWorld:boolean;eyeLeaf:number|null;leaves:number[];areas:number[];pvsSurfaces:number[];drawSurfaces:number[]};geometry:{sceneGeneration:number;samples:{x:number;y:number;disposition:string;depth:number|null;primitive:number|null;object:number|null;material:string|null}[]}}>(await agent(["--session",session,"eval","globalThis.__playsrcProfile.geometryEvidence"])),expected=CURRENT_TF2_RELEASE.targets.find(candidate=>candidate.target===target);require(expected!==undefined&&evidence.revision===revision&&evidence.generation===generation&&evidence.target===target&&evidence.finalReady&&evidence.geometry.sceneGeneration===generation&&evidence.identities.bsp===expected.objects.bsp.sha256&&evidence.identities.resourceRoot===expected.objects.resources.sha256&&evidence.identities.contentBuild===expected.contentBuild&&evidence.identities.graphTarget===target&&evidence.identities.wasm===CURRENT_TF2_RELEASE.objects.wasm.sha256&&/^\d+$/.test(evidence.identities.simulationTick),`${target} final-Ready generation identity differs: ${JSON.stringify(evidence)}`);require(!evidence.visibility.outsideWorld&&evidence.visibility.eyeLeaf!==null&&evidence.visibility.leaves.length>0&&evidence.visibility.areas.length>0&&evidence.visibility.pvsSurfaces.length>0&&evidence.visibility.drawSurfaces.length>0,`${target} final-Ready PVS is empty: ${JSON.stringify(evidence.visibility)}`)
  const admitted=new Set(evidence.visibility.drawSurfaces),hits=evidence.geometry.samples.filter(sample=>sample.disposition==="main-world");require(evidence.geometry.samples.length===25&&hits.length>0&&hits.every(sample=>sample.depth!==null&&sample.depth>evidence.camera.near&&sample.primitive!==null&&admitted.has(sample.primitive)&&Number.isSafeInteger(sample.object)&&sample.object!>=0&&Boolean(sample.material)),`${target} final-Ready depth/primitive/object evidence differs: ${JSON.stringify(evidence.geometry.samples)}`)
  const regions=evidence.geometry.samples.map((sample,index)=>{const cx=(sample.x+1)*VIEWPORT_WIDTH/2,cy=(1-sample.y)*VIEWPORT_HEIGHT/2;return{name:`geometry-${index}`,x:Math.max(0,Math.floor(cx)-4),y:Math.max(0,Math.floor(cy)-4),width:8,height:8}}),color=await captureCanvas(session,config,target,regions),failed=hits.map(hit=>evidence.geometry.samples.indexOf(hit)).filter(index=>color.regions[index]!.nonBackgroundRatio<0.25||color.regions[index]!.meanLuma<=1);require(failed.length===0,`${target} final-Ready main-world primitives expose background/sky color: ${JSON.stringify(failed.map(index=>({sample:evidence.geometry.samples[index],color:color.regions[index]})))}`)
  return Object.freeze({target,generation,identities:evidence.identities,sceneGeneration:evidence.geometry.sceneGeneration,eyeLeaf:evidence.visibility.eyeLeaf,leaves:evidence.visibility.leaves.length,areas:evidence.visibility.areas,pvsSurfaces:evidence.visibility.pvsSurfaces.length,drawSurfaces:evidence.visibility.drawSurfaces.length,mainWorldSamples:hits.length,depthRange:[Math.min(...hits.map(hit=>hit.depth!)),Math.max(...hits.map(hit=>hit.depth!))],primitiveSha256:new Bun.CryptoHasher("sha256").update(JSON.stringify(hits.map(hit=>hit.primitive))).digest("hex"),objectSha256:new Bun.CryptoHasher("sha256").update(JSON.stringify(hits.map(hit=>[hit.object,hit.material]))).digest("hex"),colorSha256:color.sha256,colorRegions:Object.freeze(color.regions.map(region=>Object.freeze({name:region.name,nonBackgroundRatio:region.nonBackgroundRatio,meanLuma:region.meanLuma,sha256:region.sha256})))})
}

export async function runDualMapAcceptance(config: LocalConfig, target: string | undefined): Promise<void> {
  if (target !== undefined) throw new BrowserEvidenceError("dual-map acceptance does not accept a target argument")
  const owner = await startDevelopmentProcess("jump_beef")
  const session = `playsrc-dual-map-${process.pid}`
  const automation = tf2BrowserAutomation(session)
  let browserOpen = false
  let primaryError: unknown
  let report: Record<string, unknown> | undefined
  try {
    await agent(["--session", session, "--headed", "--webgpu", "--init-script", TF2_BROWSER_AUTOMATION_INIT, "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    await completeStartup(session, config, "dual-map-1280x720", "skip")
    await automation.maps.load("jump_beef")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='Ready'&&document.querySelector('main').dataset.environmentSky==='sky_day01_01'", "--timeout", "600000"])
    await admitInitialClassSelection(session)
    const geometry=[await captureFinalReadyGeometry(session,config,"jump_beef",1,1)]
    const gameplay=[await exerciseSwitchedGameplay(session,"jump_beef")]
    const unknownBefore = parseJson<{ detail: string; resources: number }>(await agent(["--session", session, "eval", "(()=>({detail:document.querySelector('main').dataset.detail,resources:performance.getEntriesByType('resource').length}))()"] ))
    await automation.console.submitCommand("map upward")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Usage: map jump_beef|pl_upward|ctf_2fort')", "--timeout", "30000"])
    await Bun.sleep(500)
    const unknownAfter = parseJson<{ detail: string; resources: number }>(await agent(["--session", session, "eval", "(()=>({detail:document.querySelector('main').dataset.detail,resources:performance.getEntriesByType('resource').length}))()"] ))
    require(JSON.stringify(unknownAfter) === JSON.stringify(unknownBefore), `unknown map mutated state or fetched resources: ${JSON.stringify({ unknownBefore, unknownAfter })}`)
    await automation.maps.load("ctf_2fort")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded ctf_2fort; generation 2')&&document.querySelector('main').dataset.phase==='Ready'", "--timeout", "600000"])
    await agent(["--session", session, "wait", "--fn", "(()=>{const p=JSON.parse(document.querySelector('.world-canvas').dataset.staticProps||'null');return p?.total===2265&&p.runtimeLit===24})()", "--timeout", "30000"])
    geometry.push(await captureFinalReadyGeometry(session,config,"ctf_2fort",2,2))
    gameplay.push(await exerciseSwitchedGameplay(session,"ctf_2fort"))
    await automation.maps.load("pl_upward")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('main').dataset.phase==='Replacing'&&document.querySelector('main').dataset.detail.includes('pl_upward')&&document.querySelector('main').dataset.loadingBackground==='map-photo'", "--timeout", "30000"])
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded pl_upward; generation 3')&&document.querySelector('main').dataset.phase==='Ready'", "--timeout", "600000"])
    await agent(["--session", session, "wait", "--fn", "JSON.parse(document.querySelector('.world-canvas').dataset.staticProps||'null')?.total===1244&&JSON.parse(document.querySelector('.world-canvas').dataset.sky3dPass||'null')?.skyProps>0", "--timeout", "30000"])
    require(parseJson<string>(await agent(["--session", session, "eval", "document.querySelector('main').dataset.detail"] )) === "Playing pl_upward", "pl_upward gameplay publication is unavailable")
    geometry.push(await captureFinalReadyGeometry(session,config,"pl_upward",3,3))
    gameplay.push(await exerciseSwitchedGameplay(session,"pl_upward"))
    await automation.maps.load("jump_beef")
    await agent(["--session", session, "wait", "--fn", "document.querySelector('.developer-layer [data-vgui-service=developer-console]')?.textContent.includes('Loaded jump_beef; generation 4')&&document.querySelector('main').dataset.phase==='Ready'", "--timeout", "600000"])
    require(parseJson<number>(await agent(["--session", session, "eval", "JSON.parse(document.querySelector('.world-canvas').dataset.staticProps).total"] )) === 0, "jump_beef retained stock-map static props")
    geometry.push(await captureFinalReadyGeometry(session,config,"jump_beef",4,4))
    gameplay.push(await exerciseSwitchedGameplay(session,"jump_beef"))
    report = { schema: "playsrc-tf2-configured-map-browser-evidence-v3", sequence: ["jump_beef", "ctf_2fort", "pl_upward", "jump_beef"], generations: [1, 2, 3, 4], unknownRejectedWithoutFetch: true, loadingDescriptorsSelected: true, stockMapStaticPropsAndSky: true, gameplay,geometry,replacementResourcesReleased: true }
  } catch (error) {
    let state = "unavailable"
    if (browserOpen) state = await agent(["--session", session, "eval", "(()=>{const m=document.querySelector('main'),c=document.querySelector('.developer-layer [data-vgui-service=developer-console]');return{phase:m?.dataset.phase,detail:m?.dataset.detail,gameui:m?.dataset.gameui,console:c?.textContent.slice(-2000)}})()"] ).catch(() => "unavailable")
    primaryError = error
    throw new BrowserEvidenceError(`${error instanceof Error ? error.message : String(error)}; dual-map state=${state}`)
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    try { await owner.interrupt() } catch (error) { if (primaryError === undefined) throw error }
  }
  require((await unavailable("http://127.0.0.1:4173/readyz")) && (await unavailable("http://127.0.0.1:4174/readyz")), "dual-map listeners remained available after shutdown")
  console.log(JSON.stringify({ ...report, shutdown: "sigint-child-and-listeners-released" }))
}

export async function runBrowserAcceptance(config: LocalConfig, target: string | undefined): Promise<void> {
  const report = await verifyBrowserAcceptance(config, target)
  require((await unavailable("http://127.0.0.1:4173/readyz")) &&
    (await unavailable("http://127.0.0.1:4174/readyz")), "owned listeners remained available after shutdown")
  console.log(JSON.stringify({ ...report, shutdown: "sigint-child-and-listeners-released" }))
}
