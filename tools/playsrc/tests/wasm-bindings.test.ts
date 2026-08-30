import { expect, test } from "bun:test"
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { descriptor } from "@playsrc/asset-store"
import { captureWasmBindings, assertWasmBindings } from "../src/wasm-bindings"
import { assertStaticBundleGeneration } from "../../../apps/web/tf2/generation-plugin"
import { tf2ViteConfiguration } from "../../../apps/web/tf2/vite.config"

test("approved binary and generated helper closure stay paired even with identical raw interfaces", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "playsrc-bindings-"))
  try {
    const wasm = new Uint8Array([0,97,115,109,1,0,0,0])
    await mkdir(path.join(directory,"snippets/wasm-bindgen-rayon-abcd/src"),{recursive:true})
    await writeFile(path.join(directory,"tf2_wasm_bg.wasm"),wasm)
    await writeFile(path.join(directory,"tf2_wasm.js"),"export const closureTableIndex=17")
    const helper=path.join(directory,"snippets/wasm-bindgen-rayon-abcd/src/workerHelpers.js")
    await writeFile(helper,"export const ready='ready'")
    const certificate=await captureWasmBindings(directory,descriptor("derived-object","application/octet-stream",wasm))
    await assertWasmBindings(directory,certificate)
    await writeFile(helper,"export const ready='wrong'")
    await expect(assertWasmBindings(directory,certificate)).rejects.toThrow("binding closure differs")
    await expect(captureWasmBindings(directory,{...descriptor("derived-object","application/octet-stream",wasm),sha256:"0".repeat(64)})).rejects.toThrow("producer differs")
  } finally {await rm(directory,{recursive:true,force:true})}
})

test("deployment embeds the selected approved WASM rather than an unrelated local compiler hash", async () => {
  const configuration={applicationBuild:"a".repeat(64),wasm:{sha256:"b".repeat(64)},targets:[{target:"jump_beef",objects:{resources:{sha256:"c".repeat(64)}}}]}
  const previous=process.env.PLAYSRC_BROWSER_CONFIG, build=process.env.PLAYSRC_APPLICATION_BUILD
  try {
    process.env.PLAYSRC_BROWSER_CONFIG=JSON.stringify(configuration);delete process.env.PLAYSRC_APPLICATION_BUILD
    const plugin=tf2ViteConfiguration(undefined,true).plugins!.find((p:any)=>p?.name==='playsrc-generation') as any
    const source=await plugin.load('\0virtual:playsrc-generation')
    expect(source).toContain(`WASM_SHA256=${JSON.stringify(configuration.wasm.sha256)}`)
    const bundle:any={entry:{type:'chunk',modules:{'\0virtual:playsrc-generation':{}},code:source},bootstrap:{type:'chunk',isEntry:true,modules:{},code:'import("./main.js")'}}
    await plugin.generateBundle({},bundle)
    assertStaticBundleGeneration(bundle.entry.code,configuration as any)
    assertStaticBundleGeneration(bundle.bootstrap.code,configuration as any)
    const mixed=bundle.entry.code.replaceAll('b'.repeat(64),'d'.repeat(64))
    expect(()=>assertStaticBundleGeneration(mixed,configuration as any)).toThrow('generation differs')
    expect(()=>assertStaticBundleGeneration(source,configuration as any)).toThrow('seal is absent')
  } finally {
    if(previous===undefined)delete process.env.PLAYSRC_BROWSER_CONFIG;else process.env.PLAYSRC_BROWSER_CONFIG=previous
    if(build===undefined)delete process.env.PLAYSRC_APPLICATION_BUILD;else process.env.PLAYSRC_APPLICATION_BUILD=build
  }
})
