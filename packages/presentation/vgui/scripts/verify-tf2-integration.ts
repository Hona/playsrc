import { mkdir, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"

const MAX_OUTPUT_BYTES = 1024 * 1024
const READY_TIMEOUT_MS = 180_000
const EXIT_TIMEOUT_MS = 30_000
const packageRoot = fileURLToPath(new URL("../", import.meta.url))
const repositoryRoot = path.resolve(packageRoot, "../../..")
const evidenceRoot = path.join(packageRoot, "evidence", "client-diagnostics")
const session = `playsrc-tf2-diagnostics-${process.pid}`

async function agent(args: string[]): Promise<string> {
  const child = Bun.spawn(["agent-browser", "--session", session, ...args], { stdout: "pipe", stderr: "pipe" })
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).arrayBuffer(),
    new Response(child.stderr).arrayBuffer(),
    child.exited,
  ])
  if (stdout.byteLength > MAX_OUTPUT_BYTES || stderr.byteLength > MAX_OUTPUT_BYTES) throw new Error("agent-browser output exceeded bound")
  const output = new TextDecoder().decode(stdout).trim()
  if (code !== 0) throw new Error(new TextDecoder().decode(stderr).trim() || output)
  return output
}

async function evaluate<T>(expression: string): Promise<T> {
  return JSON.parse(await agent(["eval", expression])) as T
}

async function waitFor(label: string, expression: string, timeout: string): Promise<void> {
  try {
    await agent(["wait", "--fn", expression, "--timeout", timeout])
  } catch (error) {
    const observation = await agent(["eval", `({body:document.body.innerText.slice(0,1000),camera:document.querySelector('main')?.dataset.cameraPosition ?? null,speed:document.querySelector('.speed-readout strong')?.textContent ?? null})`]).catch(() => "unavailable")
    throw new Error(`${label}: ${error instanceof Error ? error.message : "wait failed"}; observation=${observation}`)
  }
}

function require(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

function startDevelopmentProcess(): Readonly<{ ready: Promise<void>; interrupt(): Promise<void> }> {
  const child = Bun.spawn([process.execPath, "run", "dev", "jump_beef"], {
    cwd: repositoryRoot,
    stdout: "pipe",
    stderr: "pipe",
  })
  let outputBytes = 0
  let stdout = ""
  let stderr = ""
  let resolveReady: (() => void) | undefined
  let rejectReady: ((error: Error) => void) | undefined
  const ready = new Promise<void>((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })
  const consume = async (stream: ReadableStream<Uint8Array>, channel: "stdout" | "stderr") => {
    const reader = stream.getReader()
    const decoder = new TextDecoder()
    while (true) {
      const next = await reader.read()
      if (next.done) break
      outputBytes += next.value.byteLength
      if (outputBytes > MAX_OUTPUT_BYTES) {
        child.kill("SIGKILL")
        rejectReady?.(new Error("development output exceeded bound"))
        return
      }
      const text = decoder.decode(next.value, { stream: true })
      if (channel === "stdout") {
        stdout += text
        if (stdout.split(/\r?\n/u).includes("http://127.0.0.1:4173/")) resolveReady?.()
      } else stderr += text
    }
  }
  const readers = Promise.allSettled([consume(child.stdout, "stdout"), consume(child.stderr, "stderr")])
  const exited = child.exited
  const timeout = setTimeout(() => rejectReady?.(new Error("development readiness timed out")), READY_TIMEOUT_MS)
  void ready.finally(() => clearTimeout(timeout))
  void exited.then((code) => {
    if (!stdout.includes("http://127.0.0.1:4173/")) rejectReady?.(new Error(`development exited ${code}: ${stderr || stdout}`))
  })
  return Object.freeze({
    ready,
    async interrupt() {
      child.kill("SIGINT")
      let exitTimer: ReturnType<typeof setTimeout> | undefined
      const code = await Promise.race([
        exited,
        new Promise<never>((_, reject) => {
          exitTimer = setTimeout(() => reject(new Error("development interrupt timed out")), EXIT_TIMEOUT_MS)
        }),
      ]).finally(() => { if (exitTimer) clearTimeout(exitTimer) })
      await readers
      require(code === 0, `development exited ${code}: ${stderr || stdout}`)
    },
  })
}

async function command(value: string): Promise<void> {
  await agent(["fill", "[aria-label='Console command']", value])
  await agent(["press", "Enter"])
}

const processOwner = startDevelopmentProcess()
let browserOpen = false
try {
  await processOwner.ready
  await mkdir(evidenceRoot, { recursive: true })
  await agent(["--headed", "--webgpu", "open", "http://127.0.0.1:4173/"])
  browserOpen = true
  await agent(["set", "viewport", "1280", "720", "1"])
  await agent(["wait", "--text", "Ready", "--timeout", "120000"])
  const platformFontCapability = await evaluate<string | null>("document.querySelector('[data-vgui-service=developer-console]')?.dataset.platformFontCapability ?? null")
  await waitFor("initial camera", "document.querySelector('main').dataset.cameraPosition && Number(document.querySelector('.speed-readout strong').textContent) === 0 && Math.abs(Number(document.querySelector('main').dataset.cameraPosition.split(',')[2]) + 3067.96875) < 0.01", "30000")
  const initial = await evaluate<number[]>("document.querySelector('main').dataset.cameraPosition.split(',').map(Number)")

  await agent(["eval", "window.dispatchEvent(new KeyboardEvent('keydown',{code:'ControlLeft',key:'Control',bubbles:true})); true"])
  await agent(["wait", "100"])
  const control = await evaluate<number[]>("document.querySelector('main').dataset.cameraPosition.split(',').map(Number)")
  await agent(["eval", "window.dispatchEvent(new KeyboardEvent('keyup',{code:'ControlLeft',key:'Control',bubbles:true})); true"])
  require(Math.abs(control[2] - initial[2]) < 2, "Control changed crouch state")

  await agent(["eval", "window.dispatchEvent(new KeyboardEvent('keydown',{code:'ShiftLeft',key:'Shift',bubbles:true})); true"])
  await waitFor("left shift crouch", `Number(document.querySelector('main').dataset.cameraPosition.split(',')[2]) < ${initial[2] - 15}`, "10000")
  const shiftLeft = await evaluate<number[]>("document.querySelector('main').dataset.cameraPosition.split(',').map(Number)")
  await agent(["eval", "window.dispatchEvent(new Event('blur')); true"])
  await waitFor("blur neutralization", `Number(document.querySelector('main').dataset.cameraPosition.split(',')[2]) > ${shiftLeft[2] + 15}`, "10000")
  const neutral = await evaluate<number[]>("document.querySelector('main').dataset.cameraPosition.split(',').map(Number)")

  await agent(["eval", "window.dispatchEvent(new KeyboardEvent('keydown',{code:'ShiftRight',key:'Shift',bubbles:true})); true"])
  await waitFor("right shift crouch", `Number(document.querySelector('main').dataset.cameraPosition.split(',')[2]) < ${neutral[2] - 15}`, "10000")
  const shiftRight = await evaluate<number[]>("document.querySelector('main').dataset.cameraPosition.split(',').map(Number)")
  await agent(["eval", "window.dispatchEvent(new KeyboardEvent('keyup',{code:'ShiftRight',key:'Shift',bubbles:true})); true"])

  await agent(["press", "Backquote"])
  await agent(["wait", "--fn", "getComputedStyle(document.querySelector('[role=dialog]')).display !== 'none'"])
  await command("cl_showfps")
  await agent(["wait", "--text", "\"cl_showfps\" = \"0\"", "--timeout", "10000"])
  await command("cl_showfps 2")
  await agent(["wait", "--fn", "document.querySelector('[data-diagnostic-kind=fps]')?.textContent.includes('on jump_beef')", "--timeout", "10000"])
  await command("cl_showpos 2")
  await agent(["wait", "--text", "unavailable (player absolute angles)", "--timeout", "10000"])
  await command("cl_showfps 3")
  await agent(["wait", "--text", "cl_showfps accepts exactly 0, 1, or 2", "--timeout", "10000"])
  await agent(["fill", "[aria-label='Console command']", "cl_show"])
  await agent(["wait", "--fn", "document.querySelector('[role=listbox]').textContent.includes('cl_showfps 2') && document.querySelector('[role=listbox]').textContent.includes('cl_showpos 2')"])
  const observation = await evaluate(`(()=>{
    const rect=x=>Object.fromEntries(['x','y','width','height'].map(k=>[k,x.getBoundingClientRect()[k]]));
    const frame=document.querySelector('[role=dialog]'); const diagnostic=document.querySelector('[data-vgui-name=ClientDiagnostics]');
    const history=document.querySelector('[data-vgui-name=ConsoleHistory]'),entry=document.querySelector('[data-vgui-name=ConsoleEntry]');
    return {frame:rect(frame),diagnostic:rect(diagnostic),fonts:{history:{family:getComputedStyle(history).fontFamily,size:getComputedStyle(history).fontSize,lineHeight:getComputedStyle(history).lineHeight},entry:{family:getComputedStyle(entry).fontFamily,size:getComputedStyle(entry).fontSize,lineHeight:getComputedStyle(entry).lineHeight}},
      fps:document.querySelector('[data-diagnostic-kind=fps]').textContent,
      position:[...document.querySelectorAll('[data-vgui-name=ClientDiagnostics] span')].map(x=>x.textContent),
      completion:document.querySelector('[role=listbox]').textContent,
      active:document.activeElement?.getAttribute('aria-label')};})()`)
  require((observation as { frame: { y: number }; diagnostic: { y: number; height: number } }).diagnostic.y
    + (observation as { diagnostic: { height: number } }).diagnostic.height
    <= (observation as { frame: { y: number } }).frame.y, "diagnostics overlap console")
  if (platformFontCapability === "supported") {
    await agent(["screenshot", path.join(evidenceRoot, "tf2-app-720p.png")])
  } else {
    await rm(path.join(evidenceRoot, "tf2-app-720p.png"), { force: true })
  }

  await command("cl_showfps 0")
  await command("cl_showpos 0")
  await agent(["wait", "--fn", "getComputedStyle(document.querySelector('[data-vgui-name=ClientDiagnostics]')).display === 'none'"])
  const result = {
    capturedAt: new Date().toISOString(),
    operatingSystem: `${process.platform} ${os.release()}`,
    architecture: process.arch,
    bun: Bun.version,
    browser: await agent(["--version"]),
    browserRuntime: await evaluate("({userAgent:navigator.userAgent,platform:navigator.platform,language:navigator.language})"),
    gpu: await evaluate("navigator.gpu?.requestAdapter().then(adapter=>adapter ? {vendor:adapter.info.vendor,architecture:adapter.info.architecture,device:adapter.info.device,description:adapter.info.description} : null)"),
    sourceBuild: "TF2 24207079 / patch 10822003",
    platformFonts: platformFontCapability === "supported"
      ? "browser source admitted; native target raster comparison remains required"
      : "source unavailable; glyph paint suppressed while focus, input, submission, diagnostics, and accessibility remain active",
    crouch: { initial, control, shiftLeft, neutralAfterBlur: neutral, shiftRight },
    consoleAndDiagnostics: observation,
    shutdown: "pending",
  }
  await agent(["close"])
  browserOpen = false
  await processOwner.interrupt()
  result.shutdown = "browser closed; SIGINT child exit zero"
  await writeFile(path.join(evidenceRoot, "tf2-integration.json"), `${JSON.stringify(result, null, 2)}\n`)
  console.log(JSON.stringify(result))
} finally {
  if (browserOpen) await agent(["close"]).catch(() => {})
  await processOwner.interrupt().catch(() => {})
}
