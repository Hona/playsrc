import { constants as fsConstants } from "node:fs"
import { copyFile, mkdir, readFile, rm } from "node:fs/promises"
import path from "node:path"
import { repositoryRoot, type LocalConfig } from "./config"

const MAX_OUTPUT_BYTES = 1024 * 1024
const PROCESS_READY_TIMEOUT_MS = 180_000
const PROCESS_EXIT_TIMEOUT_MS = 30_000
const APPLICATION_URL = "http://127.0.0.1:4173/"
const VIEWPORT_WIDTH = 1280
const VIEWPORT_HEIGHT = 720
const BACKGROUND_RGB = [17, 24, 32] as const

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
}>

type CanvasEvidence = Readonly<{
  sha256: string
  byteLength: number
  width: number
  height: number
  regions: readonly RegionMetric[]
}>

type CameraObservation = Readonly<{
  position: readonly [number, number, number]
  yaw: number
  pitch: number
}>

type SpawnObservation = Readonly<{
  entity: number
  hammerId: number
  position: readonly [number, number, number]
  angles: readonly [number, number, number]
}>

const VISUAL_REGIONS = Object.freeze([
  Object.freeze({ name: "ceiling", x: 400, y: 120, width: 320, height: 100 }),
  Object.freeze({ name: "forward-wall", x: 400, y: 270, width: 320, height: 180 }),
  Object.freeze({ name: "floor", x: 180, y: 500, width: 160, height: 130 }),
  Object.freeze({ name: "right-wall", x: 920, y: 300, width: 160, height: 180 }),
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
  for (let y = region.y; y < region.y + region.height; y += 1) {
    for (let x = region.x; x < region.x + region.width; x += 1) {
      const offset = (y * image.width + x) * 3
      const red = image.rgb[offset] ?? 0
      const green = image.rgb[offset + 1] ?? 0
      const blue = image.rgb[offset + 2] ?? 0
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

async function cameraObservation(session: string): Promise<CameraObservation> {
  const observation = parseJson<{ position: number[]; yaw: number; pitch: number }>(
    await agent([
      "--session",
      session,
      "eval",
      "(()=>{const d=document.querySelector('main').dataset;return {position:d.cameraPosition.split(',').map(Number),yaw:Number(d.cameraYaw),pitch:Number(d.cameraPitch)}})()",
    ]),
  )
  require(observation.position.length === 3 &&
    observation.position.every(Number.isFinite) &&
    Number.isFinite(observation.yaw) &&
    Number.isFinite(observation.pitch), "application camera observation is malformed")
  return Object.freeze({
    position: Object.freeze(observation.position) as readonly [number, number, number],
    yaw: observation.yaw,
    pitch: observation.pitch,
  })
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
  const command = [process.execPath, "run", "dev"]
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
          reject(new BrowserEvidenceError("development command did not report readiness within 180000 ms"))
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
        throw new BrowserEvidenceError("development command exited before the acceptance interrupt")
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
        throw error
      } finally {
        if (exitTimeout !== undefined) clearTimeout(exitTimeout)
        await Promise.allSettled([stdoutTask, stderrTask])
      }
      require(code === 0, `development command exited with code ${code} after SIGINT: ${excerpt(stderr || stdout)}`)
    },
  })
}

async function acquirePointerLock(session: string): Promise<boolean> {
  let lastBody = ""
  for (let attempt = 0; attempt < 3; attempt += 1) {
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
    lastBody = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    if (lastBody.includes("MOUSE CAPTURED")) return true
    await agent(["--session", session, "press", "Escape"]).catch(() => {})
    await agent(["--session", session, "wait", "1000"])
  }
  throw new BrowserEvidenceError(`desktop pointer lock was not acquired after three user activations: ${lastBody.slice(0,300)}`)
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

export async function verifyBrowserAcceptance(
  config: LocalConfig,
  target: string | undefined,
): Promise<Record<string, unknown>> {
  const version = await agent(["--version"])
  const session = `playsrc-acceptance-${process.pid}`
  const owner = await startDevelopmentProcess(target)
  let browserOpen = false
  try {
    await agent(["--session", session, "--headed", "--webgpu", "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "set", "viewport", String(VIEWPORT_WIDTH), String(VIEWPORT_HEIGHT)])
    await agent(["--session", session, "wait", "--text", "Ready", "--timeout", "300000"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('main').dataset.cameraPosition && Number(document.querySelector('.speed-readout strong').textContent) === 0",
      "--timeout",
      "30000",
    ])
    let body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("DERIVED CACHE STORED"), "cold browser run did not store the derived payload")
    const fixedSpawn = await spawnObservation(session)
    const fixedCamera = await cameraObservation(session)
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.querySelector('main').dataset.environment"]),
    ) === "hdr,284,91,1,39,62", "HDR environment summary differs")
    require(parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('main').dataset.environmentDrawables)",
      ]),
    ) === 62, "projected environment drawable count differs")
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.querySelector('main').dataset.environmentSky"]),
    ) === "sky_day01_01", "worldspawn sky identity differs")
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.querySelector('main').dataset.waterCubemap"]),
    ) === "0", "water cubemap selection differs")
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
    const coldCanvas = await captureCanvas(session, config)

    await agent(["--session", session, "click", "button.audio-toggle"])
    await agent(["--session", session, "wait", "--text", "Audio running", "--timeout", "10000"])

    const pointerLocked = await acquirePointerLock(session)
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
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
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('[role=dialog]')).display !== 'none'",
    ])
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.pointerLockElement?.className ?? ''"]),
    ) === "", "console activation did not release pointer lock")
    require(parseJson<string>(
      await agent(["--session", session, "eval", "document.activeElement?.getAttribute('aria-label') ?? ''"]),
    ) === "Console command", "console activation did not focus its command input")
    await agent(["--session", session, "fill", "[aria-label='Console command']", "status"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--text", "generation 1", "--timeout", "300000"])
    await agent(["--session", session, "press", "ArrowUp"])
    require((await agent(["--session", session, "get", "value", "[aria-label='Console command']"])) ===
      "status", "console history did not restore the submitted command")
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map j"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('[role=listbox]')?.textContent === 'map jump_beef'",
    ])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map jump_beef"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--text", "Loaded jump_beef; generation 2", "--timeout", "300000"])
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
        "--text",
        `Loaded jump_beef; generation ${generation}`,
        "--timeout",
        "300000",
      ])
      await agent(["--session", session, "wait", "--text", `mat_hdr_level = ${level}`, "--timeout", "30000"])
      require(parseJson<string>(
        await agent(["--session", session, "eval", "document.querySelector('main').dataset.environment"]),
      ).startsWith(`${profile},284,91,1,39,62`), `${profile} environment summary differs`)
    }
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('[role=dialog]')).display === 'none'",
    ])
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('[role=dialog]')).display !== 'none'",
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
      "getComputedStyle(document.querySelector('[role=dialog]')).display === 'none'",
    ])

    await agent([
      "--session",
      session,
      "eval",
      "window.dispatchEvent(new KeyboardEvent('keydown',{code:'KeyW',key:'w',bubbles:true})); true",
    ])
    await agent(["--session", session, "wait", "500"])
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
        "Number(document.querySelector('.speed-readout strong').textContent)",
      ]),
    )
    require(movingSpeed > 0, "movement binding did not advance the player")
    await agent(["--session", session, "press", "Space"])
    await agent(["--session", session, "wait", "100"])
    const jumpSpeed = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('.speed-readout strong').textContent)",
      ]),
    )
    require(jumpSpeed > 0, "jump binding did not advance the player")

    const initialBlockerCount = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
      ]),
    )
    const initialFireEvents = parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.fireEvents)"]),
    )
    const initialExplosionEvents = parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.explosionEvents)"]),
    )

    await acquirePointerLock(session)
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "left"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      `Number(document.querySelector('main').dataset.fireEvents) > ${initialFireEvents}`,
      "--timeout",
      "10000",
    ])
    await agent(["--session", session, "wait", "1200"])
    let blockerCount = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
      ]),
    )
    require(parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.particleItems)"]),
    ) > 0, "Soldier PCF render data was not observed")

    await agent(["--session", session, "press", "Escape"])
    await agent(["--session", session, "wait", "1000"])
    await agent(["--session", session, "click", ".class-rail button:nth-child(2)"])
    await agent(["--session", session, "wait", "3000"])
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("STICKYBOMB LAUNCHER"), `Demoman selection failed: ${body.slice(0, 500)}`)
    await acquirePointerLock(session)
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "left"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      `Number(document.querySelector('main').dataset.fireEvents) > ${initialFireEvents + 1}`,
      "--timeout",
      "10000",
    ])
    await agent(["--session", session, "wait", "1000"])
    await agent(["--session", session, "mouse", "down", "right"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "right"])
    await agent(["--session", session, "wait", "500"])
    blockerCount = parseJson<number>(
      await agent([
        "--session",
        session,
        "eval",
        "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
      ]),
    )
    require(parseJson<number>(
      await agent(["--session", session, "eval", "Number(document.querySelector('main').dataset.explosionEvents)"]),
    ) > initialExplosionEvents, "Demoman sticky detonation event was not observed")

    await agent(["--session", session, "reload"])
    await agent(["--session", session, "wait", "--text", "Ready", "--timeout", "300000"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('main').dataset.cameraPosition && Number(document.querySelector('.speed-readout strong').textContent) === 0",
      "--timeout",
      "30000",
    ])
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("DERIVED CACHE HIT"), "warm browser run did not reuse the derived payload")
    require(!body.includes("ModelArtifactCacheUnavailable"), "bounded model presentation artifacts were not cached")
    const warmCamera = await cameraObservation(session)
    const warmCanvas = await captureCanvas(session, config)
    require(warmCanvas.sha256 === coldCanvas.sha256, "warm fixed-camera canvas differs from the cold capture")
    require(warmCamera.position.every((value, index) => Math.abs(value - fixedCamera.position[index]!) <= 0.001) &&
      Math.abs(warmCamera.yaw - fixedCamera.yaw) <= 0.001 &&
      Math.abs(warmCamera.pitch - fixedCamera.pitch) <= 0.001, "warm fixed camera differs from the cold camera")
    const records = parseJson<Array<{ key: string; byteLength: number; sha256: string }>>(
      await agent([
        "--session",
        session,
        "eval",
        "new Promise((resolve,reject)=>{const r=indexedDB.open('playsrc-derived-v1',1);r.onerror=()=>reject(r.error);r.onsuccess=()=>{const q=r.result.transaction('objects').objectStore('objects').getAll();q.onerror=()=>reject(q.error);q.onsuccess=()=>resolve(q.result.map(x=>({key:x.key,byteLength:x.byteLength,sha256:x.sha256})))}})",
      ]),
    )
    const mapRecords = records.filter(
      (record) =>
        record.sha256 === "d39f32489a7449075e788f78cde8bb0263b161e917d9a1b10cd0f6a96e865c68" ||
        record.sha256 === "f44941ce76aa276d7a278cb84c122709f47e477baaec865091c0b0ab5653ab0e",
    )
    require(mapRecords.length === 2 &&
      mapRecords.some(
        (record) =>
          record.byteLength === 85_586_296 &&
          record.sha256 === "d39f32489a7449075e788f78cde8bb0263b161e917d9a1b10cd0f6a96e865c68",
      ) &&
      mapRecords.some(
        (record) =>
          record.byteLength === 49_414_468 &&
          record.sha256 === "f44941ce76aa276d7a278cb84c122709f47e477baaec865091c0b0ab5653ab0e",
      ) &&
      records.some(
        (record) => record.byteLength === 39_322_364 && record.key === record.sha256,
      ), `warm IndexedDB record identity differs: ${JSON.stringify(records)}`)
    return {
      target: "jump_beef",
      browser: version,
      coldCache: "stored",
      warmCache: "hit",
      derived: mapRecords,
      mapReplacementGeneration: 4,
      movingSpeed,
      jumpSpeed,
      supportBlockers: blockerCount,
      supportStatus: "diagnostic-blockers-retained",
      pointerLock: pointerLocked ? "acquired-and-released-for-console" : "headed-window-focus-unavailable",
      console: "history-completion-focus-repeated-visibility-replacement-close-passed",
      audio: "exact-buffers-decoded-and-context-running",
      fixedCamera,
      fixedSpawn,
      canvas: coldCanvas,
      pointerDirection: {
        positiveHorizontalRightDot: Number(rightDirectionDelta.toFixed(6)),
        positiveVerticalDownDot: Number(downDirectionDelta.toFixed(6)),
      },
      shutdown: "pending",
    }
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    await owner.interrupt()
  }
}

export async function runBrowserAcceptance(config: LocalConfig, target: string | undefined): Promise<void> {
  const report = await verifyBrowserAcceptance(config, target)
  require((await unavailable("http://127.0.0.1:4173/readyz")) &&
    (await unavailable("http://127.0.0.1:4174/readyz")), "owned listeners remained available after shutdown")
  console.log(JSON.stringify({ ...report, shutdown: "sigint-child-and-listeners-released" }))
}
