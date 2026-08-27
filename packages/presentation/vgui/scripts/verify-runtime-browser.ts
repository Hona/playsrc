import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { createEvidenceDirectory } from "../../../../tools/playsrc/src/evidence-directory"

const MAX_OUTPUT_BYTES = 1024 * 1024
const packageRoot = fileURLToPath(new URL("../", import.meta.url))
const evidenceRoot = await createEvidenceDirectory(await loadLocalConfig(path.resolve(packageRoot, "../../..")), "vgui-runtime")
console.log(`Evidence: ${evidenceRoot}`)
const session = `playsrc-vgui-runtime-${process.pid}`

async function agent(args: string[]): Promise<string> {
  const child = Bun.spawn(["agent-browser", "--session", session, ...args], { stdout: "pipe", stderr: "pipe" })
  const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).arrayBuffer(), child.exited])
  if (stdout.byteLength > MAX_OUTPUT_BYTES || stderr.byteLength > MAX_OUTPUT_BYTES) throw new Error("agent-browser output exceeded bound")
  const output = new TextDecoder().decode(stdout).trim()
  if (code !== 0) throw new Error(new TextDecoder().decode(stderr).trim() || output)
  return output
}

async function evaluate<T>(expression: string): Promise<T> {
  return JSON.parse(await agent(["eval", expression])) as T
}

function require(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const temporary = await mkdtemp(path.join(os.tmpdir(), "playsrc-vgui-runtime-"))
const bundle = await Bun.build({
  entrypoints: [path.join(packageRoot, "tests", "runtime-browser-fixture.ts")],
  outdir: temporary,
  target: "browser",
  minify: false,
})
if (!bundle.success) throw new Error("runtime browser fixture bundle failed")
await writeFile(path.join(temporary, "index.html"), `<!doctype html><html><head><meta charset="utf-8"><title>VGUI runtime evidence</title><style>html,body,#mount{width:100%;height:100%;margin:0;overflow:hidden}body{background:#171a1b}#mount{position:relative}</style></head><body><main id="mount"></main><script type="module" src="/runtime-browser-fixture.js"></script></body></html>`)
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 4175,
  async fetch(request) {
    const url = new URL(request.url)
    const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1)
    if (file !== "index.html" && file !== "runtime-browser-fixture.js") return new Response("not found", { status: 404 })
    return new Response(await readFile(path.join(temporary, file)), { headers: { "content-type": file.endsWith(".js") ? "text/javascript" : "text/html" } })
  },
})

try {
  await mkdir(evidenceRoot, { recursive: true })
  await agent(["--headed", "open", "http://127.0.0.1:4175/"])
  await agent(["set", "viewport", "1280", "720", "1"])
  await agent(["wait", "--fn", "Boolean(window.vguiRuntimeEvidence) && (window.vguiRuntimeEvidence.status().ready || window.vguiRuntimeEvidence.status().error)"])
  const status = await evaluate<{ ready: boolean; error: string | null }>("window.vguiRuntimeEvidence.status()")
  require(status.ready, status.error ?? "runtime fixture failed to initialize")
  const visualAudit = await evaluate<{ justify: string; color: string; background: string }>(`(()=>{const node=document.querySelector('[data-vgui-name=AuditDisabledButton]');const style=getComputedStyle(node);return {justify:style.justifyContent,color:style.color,background:style.backgroundColor}})()`)
  require(visualAudit.justify === "flex-start", "west-aligned Button did not inherit Label alignment")
  require(visualAudit.color === "rgb(35, 38, 40)", "disabled Button text color differs")
  require(visualAudit.background === "rgb(64, 71, 77)", "disabled Button background differs")
  await agent(["click", "[aria-label='Toggle']"])
  await agent(["fill", "[aria-label='Entry']", "12x.5"])
  await agent(["eval", "(()=>{const entry=document.querySelector('[aria-label=Entry]');entry.value='12x.5';entry.dispatchEvent(new Event('input',{bubbles:true}));return entry.value})()"])
  await agent(["click", "[aria-label='Volume']"])
  await agent(["press", "ArrowRight"])
  const sliderInput = await evaluate<{ value: number; keyFocus: number | null; active: string | null }>(`(()=>{const snapshot=window.vguiRuntimeEvidence.snapshot();return {value:snapshot.panels.find(panel=>panel.name==='Volume').state.value,keyFocus:snapshot.input.keyFocus,active:document.activeElement?.dataset?.vguiName??null}})()`)
  require(sliderInput.value === 6, `slider keyboard input differs immediately: ${JSON.stringify(sliderInput)}`)
  const frame = await evaluate<{ x: number; y: number; width: number; height: number }>("document.querySelector('[data-vgui-name=Frame]').getBoundingClientRect().toJSON()")
  await agent(["mouse", "move", String(frame.x + 80), String(frame.y + 10)])
  await agent(["mouse", "down", "left"])
  await agent(["mouse", "move", String(frame.x + 120), String(frame.y + 40)])
  await agent(["mouse", "up", "left"])
  await agent(["eval", "window.vguiRuntimeEvidence.showQuery(); true"])
  const modalAccessibility = JSON.parse(await agent(["snapshot", "--json"]))
  await agent(["press", "Escape"])
  const functional = await evaluate<{
    entry: string
    slider: number
    toggle: boolean
    queryVisible: boolean
    applicationModal: number | null
    capture: number | null
    frame: { x: number; y: number }
  }>(`(()=>{const snapshot=window.vguiRuntimeEvidence.snapshot();const panel=name=>snapshot.panels.find(panel=>panel.name===name);return {entry:panel('Entry').text,slider:panel('Volume').state.value,toggle:panel('Toggle').state.checked,queryVisible:panel('Query').visible,applicationModal:snapshot.input.applicationModal,capture:snapshot.input.capture,frame:panel('Frame').bounds}})()`)
  require(functional.entry === "12.5", "numeric browser input differs")
  require(functional.slider === 6, `slider keyboard input differs: ${functional.slider}`)
  require(functional.toggle, "checkbox pointer input differs")
  require(!functional.queryVisible && functional.applicationModal === null, "query modal did not close")
  require(functional.capture === null, "pointer capture survived frame drag")
  require(functional.frame.x === 60 && functional.frame.y === 310, "frame drag geometry differs")
  await agent(["screenshot", path.join(evidenceRoot, "runtime.png")])
  const observation = await evaluate(`(()=>{
    const controls=[...document.querySelectorAll('[data-vgui-panel]')];
    const pick=name=>{const node=document.querySelector('[data-vgui-name='+name+']');const style=getComputedStyle(node);return {rect:node.getBoundingClientRect().toJSON(),role:node.getAttribute('role'),label:node.getAttribute('aria-label'),disabled:node.getAttribute('aria-disabled'),background:style.backgroundColor,color:style.color,border:style.borderWidth,display:style.display}};
    return {runtime:window.vguiRuntimeEvidence.snapshot(),requests:window.vguiRuntimeEvidence.requests(),dom:{controls:controls.length,names:controls.map(node=>node.dataset.vguiName),types:controls.map(node=>node.dataset.vguiControl),roles:controls.map(node=>node.getAttribute('role'))},computed:{Submit:pick('Submit'),Toggle:pick('Toggle'),Entry:pick('Entry'),Volume:pick('Volume'),Progress:pick('Progress'),Menu:pick('Menu'),Frame:pick('Frame'),Query:pick('Query')}};
  })()`)
  const controlTypes = await evaluate<string[]>("[...document.querySelectorAll('[data-vgui-panel]')].map(node=>node.dataset.vguiControl)")
  for (const control of ["Panel", "EditablePanel", "Label", "ImagePanel", "Button", "TextEntry", "RichText", "Frame", "ScrollBar", "Slider", "ComboBox", "Menu", "MenuItem", "PropertySheet", "PropertyPage", "CheckButton", "RadioButton", "ProgressBar", "ListPanel", "MessageBox", "QueryBox", "URLLabel"]) {
    require(controlTypes.includes(control), `browser fixture omitted ${control}`)
  }
  const cycles = await evaluate<{ mountChildren: number; snapshot: { ownedResources: { nodes: number; listeners: number; observers: number; timers: number } } }>("window.vguiRuntimeEvidence.cycles(25)")
  require(cycles.mountChildren === 2, "repeated runtime mount retained DOM")
  require(cycles.snapshot.ownedResources.listeners === 13 && cycles.snapshot.ownedResources.observers === 0 && cycles.snapshot.ownedResources.timers === 0, "runtime browser resources differ")
  const result = {
    capturedAt: new Date().toISOString(),
    sourceBuild: "TF2 24245096 / patch 10828683",
    sdkRevision: "88fa198fba3fb85d46d4c95018254693fdc3af0a",
    operatingSystem: `${process.platform} ${os.release()}`,
    architecture: process.arch,
    bun: Bun.version,
    browserCli: await agent(["--version"]),
    browserRuntime: await evaluate("({userAgent:navigator.userAgent,platform:navigator.platform,language:navigator.language,viewport:[innerWidth,innerHeight],devicePixelRatio})"),
    visualAudit,
    functional,
    observation,
    modalAccessibility,
    cycles,
  }
  await writeFile(path.join(evidenceRoot, "browser-evidence.json"), `${JSON.stringify(result, null, 2)}\n`)
  console.log(JSON.stringify(result))
} finally {
  await agent(["close"]).catch(() => {})
  server.stop(true)
  await rm(temporary, { recursive: true, force: true })
}
