import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"

const MAX_OUTPUT_BYTES = 1024 * 1024
const packageRoot = fileURLToPath(new URL("../", import.meta.url))
const evidenceRoot = path.join(packageRoot, "evidence", "client-diagnostics")
const session = `playsrc-vgui-evidence-${process.pid}`

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

function require(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

async function drag(start: readonly [number, number], end: readonly [number, number]): Promise<unknown> {
  await agent(["mouse", "move", String(start[0]), String(start[1])])
  await agent(["mouse", "down", "left"])
  const captured = await evaluate("window.vguiEvidence.snapshots().console.frame")
  await agent(["mouse", "move", String(end[0]), String(end[1])])
  await agent(["mouse", "up", "left"])
  const final = await evaluate<{ interaction: string | null; capturedPointerId: number | null }>("window.vguiEvidence.snapshots().console.frame")
  require(final.interaction === null && final.capturedPointerId === null, "pointer capture survived drag")
  return { captured, final }
}

async function reset(): Promise<void> {
  await agent(["eval", "window.vguiEvidence.reset(); true"])
  await agent(["wait", "350"])
}

const temporary = await mkdtemp(path.join(os.tmpdir(), "playsrc-vgui-evidence-"))
const bundle = await Bun.build({
  entrypoints: [path.join(packageRoot, "tests", "browser-fixture.ts")],
  outdir: temporary,
  target: "browser",
  minify: false,
})
if (!bundle.success) throw new Error("browser fixture bundle failed")
await writeFile(path.join(temporary, "index.html"), `<!doctype html>
<html><head><meta charset="utf-8"><title>playsrc VGUI evidence</title><style>
html,body,#mount{width:100%;height:100%;margin:0;overflow:hidden}body{background:#171a1b}#mount{position:relative}
</style></head><body><main id="mount"></main><script type="module" src="/browser-fixture.js"></script></body></html>`)
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 4174,
  async fetch(request) {
    const url = new URL(request.url)
    const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1)
    if (file !== "index.html" && file !== "browser-fixture.js") return new Response("not found", { status: 404 })
    return new Response(await readFile(path.join(temporary, file)), {
      headers: { "content-type": file.endsWith(".js") ? "text/javascript" : "text/html" },
    })
  },
})

const result: Record<string, unknown> = {
  capturedAt: new Date().toISOString(),
  operatingSystem: `${process.platform} ${os.release()}`,
  architecture: process.arch,
  bun: Bun.version,
  sourceBuild: "TF2 24207079 / patch 10822003",
  schemeRevision: "e9159a983557dea91b7030b382cce9ee7521c6f4de904107013bdcb47c4a732e",
  browserCli: await agent(["--version"]),
}

try {
  await mkdir(evidenceRoot, { recursive: true })
  await agent(["--headed", "open", "http://127.0.0.1:4174/"])
  await agent(["wait", "--fn", "Boolean(window.vguiEvidence)"])
  result.browserRuntime = await evaluate("({userAgent:navigator.userAgent,platform:navigator.platform,language:navigator.language})")
  const platformFonts = await evaluate<{ kind: "supported" | "unsupported"; reason?: string }>("window.vguiEvidence.platformFonts")
  result.platformFonts = platformFonts
  if (platformFonts.kind === "unsupported") {
    result.result = "unsupported-platform-fonts"
    result.captures = {}
    await Promise.all([
      "480p.png",
      "720p.png",
      "1080p.png",
      "720p-dpr2.png",
      "720p-zoom2.png",
    ].map((name) => rm(path.join(evidenceRoot, name), { force: true })))
    await writeFile(path.join(evidenceRoot, "browser-evidence.json"), `${JSON.stringify(result, null, 2)}\n`)
    console.log(JSON.stringify(result))
  } else {
  const captures: Record<string, unknown> = {}
  for (const [name, width, height, scale] of [
    ["480p", 854, 480, 1],
    ["720p", 1280, 720, 1],
    ["1080p", 1920, 1080, 1],
    ["720p-dpr2", 1280, 720, 2],
  ] as const) {
    await agent(["set", "viewport", String(width), String(height), String(scale)])
    await reset()
    await agent(["fill", "[aria-label='Console command']", "cl_"])
    await agent(["press", "ArrowDown"])
    await agent(["wait", "350"])
    const observation = await evaluate(`(()=>{
      const frame=document.querySelector('[data-vgui-name=GameConsole]');
      const titleBackground=document.querySelector('[data-vgui-name=ConsoleTitleBackground]');
      const titlebar=document.querySelector('[data-vgui-name=ConsoleTitleBar]');
      const title=document.querySelector('[data-vgui-name=ConsoleTitle]');
      const client=document.querySelector('[data-vgui-name=ConsolePage]');
      const close=document.querySelector('[data-vgui-name=ConsoleClose]');
      const history=document.querySelector('[data-vgui-name=ConsoleHistory]');
      const entry=document.querySelector('[data-vgui-name=ConsoleEntry]');
      const submit=document.querySelector('[data-vgui-name=ConsoleSubmit]');
      const popup=document.querySelector('[data-vgui-name=CompletionList]');
      const diagnostic=document.querySelector('[data-vgui-name=ClientDiagnostics]');
      const style=getComputedStyle(history); const rect=x=>Object.fromEntries(['x','y','width','height'].map(k=>[k,x.getBoundingClientRect()[k]]));
      const border=x=>{const s=getComputedStyle(x);return Object.fromEntries(['Top','Right','Bottom','Left'].flatMap(side=>[[side.toLowerCase()+'Color',s['border'+side+'Color']],[side.toLowerCase()+'Width',s['border'+side+'Width']]]))};
      return {inner:[innerWidth,innerHeight],dpr:devicePixelRatio,active:document.activeElement?.getAttribute('aria-label'),
        frame:rect(frame),titleBackground:rect(titleBackground),titlebar:rect(titlebar),title:rect(title),client:rect(client),close:rect(close),history:rect(history),entry:rect(entry),submit:rect(submit),popup:rect(popup),diagnostic:rect(diagnostic),
        fonts:Object.fromEntries(Object.entries({title,history,entry,submit,completion:popup,diagnostic}).map(([name,node])=>{const s=getComputedStyle(node);return [name,{family:s.fontFamily,size:s.fontSize,lineHeight:s.lineHeight,weight:s.fontWeight}]})),
        colors:{frame:getComputedStyle(frame).backgroundColor,titleBackground:getComputedStyle(titleBackground).backgroundColor,title:getComputedStyle(title).color,history:style.backgroundColor,text:style.color,entry:getComputedStyle(entry).color,submit:getComputedStyle(submit).backgroundColor,popup:getComputedStyle(popup).backgroundColor},
        borders:{frame:border(frame),history:border(history),entry:border(entry),submit:border(submit),popup:border(popup)},
        resizeCursors:Object.fromEntries([...document.querySelectorAll('[data-resize-edge]')].map(x=>[x.dataset.resizeEdge,getComputedStyle(x).cursor])),
        snapshots:window.vguiEvidence.snapshots()};})()`)
    await agent(["screenshot", path.join(evidenceRoot, `${name}.png`)])
    captures[name] = observation
  }
  await agent(["set", "viewport", "640", "360", "2"])
  await reset()
  await agent(["fill", "[aria-label='Console command']", "cl_"])
  await agent(["press", "ArrowDown"])
  await agent(["wait", "350"])
  captures["720p-zoom2"] = await evaluate(`(()=>({
    physicalViewport:[1280,720],inner:[innerWidth,innerHeight],dpr:devicePixelRatio,zoom:2,
    frame:Object.fromEntries(['x','y','width','height'].map(k=>[k,document.querySelector('[data-vgui-name=GameConsole]').getBoundingClientRect()[k]])),
    snapshots:window.vguiEvidence.snapshots()
  }))()`)
  await agent(["screenshot", path.join(evidenceRoot, "720p-zoom2.png")])
  result.captures = captures

  await agent(["set", "viewport", "1280", "720", "1"])
  await reset()
  const focused = await evaluate(`(()=>{const f=document.querySelector('[data-vgui-name=GameConsole]'),t=document.querySelector('[data-vgui-name=ConsoleTitle]'),c=document.querySelector('[data-vgui-name=ConsoleClose]');return {frame:getComputedStyle(f).backgroundColor,title:getComputedStyle(t).color,close:getComputedStyle(c).color,focused:f.dataset.focused}})()`)
  await agent(["eval", "window.dispatchEvent(new Event('blur')); true"])
  await agent(["wait", "350"])
  const unfocused = await evaluate(`(()=>{const f=document.querySelector('[data-vgui-name=GameConsole]'),t=document.querySelector('[data-vgui-name=ConsoleTitle]'),c=document.querySelector('[data-vgui-name=ConsoleClose]');return {frame:getComputedStyle(f).backgroundColor,title:getComputedStyle(t).color,close:getComputedStyle(c).color,focused:f.dataset.focused}})()`)
  await agent(["eval", "window.dispatchEvent(new Event('focus')); true"])
  await agent(["wait", "350"])
  const submitBox = await evaluate<{ x: number; y: number; width: number; height: number }>("document.querySelector('[data-vgui-name=ConsoleSubmit]').getBoundingClientRect().toJSON()")
  await agent(["mouse", "move", String(submitBox.x + submitBox.width / 2), String(submitBox.y + submitBox.height / 2)])
  const armed = await evaluate("(()=>{const s=getComputedStyle(document.querySelector('[data-vgui-name=ConsoleSubmit]'));return {background:s.backgroundColor,color:s.color}})()")
  await agent(["mouse", "down", "left"])
  const depressed = await evaluate("(()=>{const s=getComputedStyle(document.querySelector('[data-vgui-name=ConsoleSubmit]'));return {background:s.backgroundColor,color:s.color,borderTop:s.borderTopColor}})()")
  await agent(["mouse", "up", "left"])
  result.controlStates = { focused, unfocused, submit: { armed, depressed } }

  const movement: Record<string, unknown> = {}
  const positions = [
    ["center", 320, 96], ["top", 320, 0], ["top-right", 640, 0], ["right", 640, 96],
    ["bottom-right", 640, 192], ["bottom", 320, 192], ["bottom-left", 0, 192], ["left", 0, 96], ["top-left", 0, 0],
  ] as const
  for (const [name, x, y] of positions) {
    await reset()
    const start = [650, 112] as const
    movement[name] = await drag(start, [start[0] + x - 616, start[1] + y - 96])
  }
  result.movement = movement

  const resize: Record<string, unknown> = {}
  const vectors = [
    ["n", 936, 97, 936, 127], ["ne", 1255, 97, 1295, 127], ["e", 1255, 360, 1295, 390],
    ["se", 1255, 623, 1295, 653], ["s", 936, 623, 976, 653], ["sw", 617, 623, 657, 653],
    ["w", 617, 360, 657, 390], ["nw", 617, 97, 657, 127],
  ] as const
  for (const [name, sx, sy, ex, ey] of vectors) {
    await reset()
    resize[name] = await drag([sx, sy], [ex, ey])
  }
  result.resize = resize

  await reset()
  await agent(["mouse", "move", "650", "112"])
  await agent(["mouse", "down", "left"])
  const beforeHide = await evaluate("window.vguiEvidence.snapshots().console.frame")
  await agent(["eval", "window.vguiEvidence.hide(); true"])
  await agent(["mouse", "up", "left"])
  const afterHide = await evaluate("window.vguiEvidence.snapshots().console.frame")
  require((afterHide as { capturedPointerId: number | null }).capturedPointerId === null, "hide retained pointer capture")
  await agent(["eval", "window.vguiEvidence.show(); true"])
  const cycles = await evaluate("window.vguiEvidence.cycles(25)")
  result.cancellation = { beforeHide, afterHide }
  result.cycles = cycles
  result.accessibility = JSON.parse(await agent(["snapshot", "--json"]))
  result.platformFontMetrics = await evaluate(`(()=>{
    const canvas=document.createElement('canvas'); const context=canvas.getContext('2d');
    const nodes={title:document.querySelector('[data-vgui-name=ConsoleTitle]'),history:document.querySelector('[data-vgui-name=ConsoleHistory]'),entry:document.querySelector('[data-vgui-name=ConsoleEntry]'),completion:document.querySelector('[data-vgui-name=CompletionList]'),diagnostic:document.querySelector('[data-vgui-name=ClientDiagnostics]')};
    return Object.fromEntries(Object.entries(nodes).map(([name,node])=>{const style=getComputedStyle(node);context.font=style.font;const metrics=context.measureText('TF2 Console Hg 0123456789');return [name,{font:context.font,width:metrics.width,actualBoundingBoxLeft:metrics.actualBoundingBoxLeft,actualBoundingBoxRight:metrics.actualBoundingBoxRight,actualBoundingBoxAscent:metrics.actualBoundingBoxAscent,actualBoundingBoxDescent:metrics.actualBoundingBoxDescent,fontBoundingBoxAscent:metrics.fontBoundingBoxAscent,fontBoundingBoxDescent:metrics.fontBoundingBoxDescent}]}));
  })()`)
  await writeFile(path.join(evidenceRoot, "browser-evidence.json"), `${JSON.stringify(result, null, 2)}\n`)
  console.log(JSON.stringify(result))
  }
} finally {
  await agent(["close"]).catch(() => {})
  server.stop(true)
  await rm(temporary, { recursive: true, force: true })
}
