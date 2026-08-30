import path from "node:path"
import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import { execFileSync } from "node:child_process"
import { test, expect, guardStartupInput } from "./application-test"
import { profileArtifact } from "./profile-artifacts"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { loadLocalConfig } from "../src/config"

// Module initialization is silent, before the application fixture requests its
// native browser stage. Build these fixtures with simd-configured.test.ts first.
const config = await loadLocalConfig(process.cwd())
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")
const index = JSON.parse(await readFile(path.join(config.sourceCacheDir, "simd-tests", hash(Buffer.from(process.cwd())).slice(0, 8), "comparison.json"), "utf8"))
const recordBytes = await readFile(index.path)
if (hash(recordBytes) !== index.sha256) throw Error("SIMD comparison record changed")
const inputRecord = JSON.parse(recordBytes.toString())
if (inputRecord.records.length !== 2 || inputRecord.records[0].variant !== "scalar" || inputRecord.records[1].variant !== "simd") throw Error("Expected exactly the scalar and SIMD comparison")
const commit = execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim()
if (inputRecord.commit !== commit) throw Error("Prepare SIMD fixtures at this exact source commit")
const input = await readFile(inputRecord.input.path)
if (inputRecord.input.sha256 !== "6d5029641d1a058b5316d4fd49b7ee923ec6490bb5ce93e40fa25ccaa169aad5" || hash(input) !== inputRecord.input.sha256 || input.length !== inputRecord.input.bytes) throw Error("Configured MP3 changed")
const modules = await Promise.all(inputRecord.records.map(async (record: any) => {
  const bytes = await readFile(record.file)
  if (hash(bytes) !== record.sha256 || bytes.length !== record.bytes) throw Error("SIMD module changed")
  return { variant: record.variant, bytes: bytes.toString("base64"), sha256: record.sha256 }
}))

test("headed browser executes exact scalar and SIMD decoder kernels", async ({ page }, testInfo) => {
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  const native = await startupNativeReader(page, config.sourceCacheDir)
  guardStartupInput(page, async () => requireStartupNative(await native.read()))
  let result: any, pixels: Buffer
  const admissions: unknown[] = []
  try {
    const admission = await native.read(); requireStartupNative(admission); admissions.push(admission)
    await page.goto("/")
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 30_000 })
    await page.setContent('<!doctype html><title>Source MP3 SIMD comparison</title><style>body{background:#18232d;color:#fff;font:20px monospace;margin:32px}canvas{background:#0c141b}p{max-width:950px}</style><h1>Source MP3 decoder: exact SIMD comparison</h1><p>Configured input; real browser WebAssembly execution. This is a decoder diagnostic, not gameplay FPS or a freeze fix.</p><canvas width="1000" height="280"></canvas><pre id="result">Preparing scalar and SIMD modules…</pre>')
    result = await page.evaluate(async ({ modules, encoded }) => {
      const bytes = (value: string) => Uint8Array.from(atob(value), c => c.charCodeAt(0))
      const input = bytes(encoded), records = []
      const digest = async (value: Uint8Array) => [...new Uint8Array(await crypto.subtle.digest("SHA-256", value))].map(value => value.toString(16).padStart(2, "0")).join("")
      const states = []
      for (const entry of modules) {
        const binary = bytes(entry.bytes), valid = WebAssembly.validate(binary), began = performance.now()
        if (!valid) throw Error(`${entry.variant} module unsupported`)
        const module = await WebAssembly.compile(binary), compiled = performance.now()
        const e = (await WebAssembly.instantiate(module)).exports as any, instantiated = performance.now()
        e.check_wasm_synthesis_groups()
        const invoke = () => {
          const pointer = e.test_input_alloc(input.length)
          new Uint8Array(e.memory.buffer, pointer, input.length).set(input)
          const started = performance.now(), count = e.test_decode(pointer, input.length)
          return { milliseconds: performance.now() - started, count }
        }
        const first = invoke()
        const pcm = new Uint8Array(e.memory.buffer, e.test_pcm_pointer(), first.count * 2).slice()
        const pcmSha256 = await digest(pcm)
        const record = { variant: entry.variant, moduleSha256: await digest(binary), valid, compileMilliseconds: compiled-began, instantiateMilliseconds: instantiated-compiled,
          firstDecodeMilliseconds: first.milliseconds, samples: first.count, pcmSha256, linearBytes: e.memory.buffer.byteLength, batches: [] as number[] }
        records.push(record); states.push({ invoke, record, memory: e.memory as WebAssembly.Memory })
        if (entry.variant === "simd") {
          const samples = new Int16Array(pcm.buffer), canvas = document.querySelector("canvas")!, context = canvas.getContext("2d")!
          context.strokeStyle="#66dfb7"; context.beginPath()
          for(let x=0;x<1000;x++){const value=samples[Math.floor(x*samples.length/1000)]!/32768; const y=140-value*125; if(x===0)context.moveTo(x,y);else context.lineTo(x,y)}context.stroke()
        }
        for (let i=0;i<8;i++) invoke()
      }
      // Alternate ABBA order. Each measurement spans 32 real decoder calls,
      // reducing timer granularity without changing the per-call sample/input.
      const sampleStart=performance.now(); let round=0
      while(performance.now()-sampleStart<5000) {
        for(const i of round++%2===0?[0,1,1,0]:[1,0,0,1]) {
          let elapsed=0;for(let n=0;n<32;n++)elapsed+=states[i]!.invoke().milliseconds
          states[i]!.record.batches.push(elapsed/32)
        }
        await new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()))
      }
      for(const state of states){const record=state.record,values=[...record.batches].sort((a,b)=>a-b);Object.assign(record,{median:values[Math.floor(values.length/2)],p95:values[Math.floor(values.length*.95)],maximum:values.at(-1),finalLinearBytes:state.memory.buffer.byteLength})}
      document.querySelector("#result")!.textContent=records.map((r:any)=>`${r.variant}: median ${r.median.toFixed(4)} ms, p95 ${r.p95.toFixed(4)} ms; ${r.samples} exact PCM samples`).join("\n")
      return { records, sampleMilliseconds:performance.now()-sampleStart, userAgent:navigator.userAgent, platform:navigator.platform, hardwareConcurrency:navigator.hardwareConcurrency,
        browserEvidence:true, sustainedGameplayEvidence:false, scope:"Configured decoder kernel; host input allocation/copy excluded identically, decoder allocations and output replacement/free included. No gameplay or freeze claim." }
    }, { modules, encoded: input.toString("base64") })
    const after = await native.read(); requireStartupNative(after); admissions.push(after)
    for (const record of result.records) {
      expect(record.samples).toBe(73728)
      expect(record.pcmSha256).toBe("b1e43ccf681c3529aad850231599216cfd55778a27bb559b8859917be486ee42")
    }
    pixels = await page.screenshot()
  } finally { await native.close() }
  await profileArtifact(async () => {
    await writeFile(path.join(directory, "simd-decoder.json"), JSON.stringify({ commit, inputRecord, result, admissions }, null, 2))
    await writeFile(path.join(directory, "simd-decoder.png"), pixels)
    await testInfo.attach("simd-decoder", { body: pixels, contentType: "image/png" })
  })
})
