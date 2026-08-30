import type { Plugin } from "vite"
import type { BrowserConfiguration } from "./src/config"

export type BundleGeneration = Readonly<{ applicationBuild: string; wasmSha256: string; resourceRoots: Readonly<Record<string, string>> }>

export const STATIC_GENERATION_BUNDLE_PREFIXES = ["index-", "main-", "gameplay-worker-"] as const

export function readBundledGeneration(source: string): BundleGeneration {
  const records = [...source.matchAll(/\/\*playsrc-generation:(.*?)\*\//g)]
  if (records.length !== 1) throw new Error("Static bundle generation seal is absent or ambiguous")
  const value=JSON.parse(records[0]![1]!)
  if(!value||Object.keys(value).sort().join("\0")!=="applicationBuild\0resourceRoots\0wasmSha256"
    ||!/^[0-9a-f]{64}$/.test(value.applicationBuild)||!/^[0-9a-f]{64}$/.test(value.wasmSha256)
    ||!value.resourceRoots||Array.isArray(value.resourceRoots)||typeof value.resourceRoots!=="object"
    ||Object.keys(value.resourceRoots).length===0||Object.entries(value.resourceRoots).some(([key,hash])=>!/^[a-z0-9_]+$/.test(key)||!/^[0-9a-f]{64}$/.test(String(hash))))throw new Error("Static bundle generation seal is malformed")
  return value
}

export function assertStaticBundleGeneration(source: string, configuration: BrowserConfiguration): void {
  const generation = readBundledGeneration(source)
  const expected = Object.fromEntries(configuration.targets.map(target => [target.target, target.objects.resources.sha256]))
  if (generation.applicationBuild !== configuration.applicationBuild || generation.wasmSha256 !== configuration.wasm.sha256
    || JSON.stringify(generation.resourceRoots) !== JSON.stringify(expected)) throw new Error("Static bundle/configuration generation differs")
}

export function generationPlugin(current: () => BundleGeneration | Promise<BundleGeneration>): Plugin {
  const name = "virtual:playsrc-generation"
  const resolved = `\0${name}`
  let generation: Promise<BundleGeneration> | undefined
  return {
    name: "playsrc-generation",
    resolveId(id) { if (id === name) return resolved },
    async load(id) {
      if (id !== resolved) return
      const value = await (generation = Promise.resolve(current()))
      const hash = /^[0-9a-f]{64}$/
      if (!hash.test(value.applicationBuild) || !hash.test(value.wasmSha256)
        || Object.keys(value.resourceRoots).length === 0 || Object.values(value.resourceRoots).some((root) => !hash.test(root))) {
        throw new Error("Browser bundle generation is malformed")
      }
      return `export const APPLICATION_BUILD=${JSON.stringify(value.applicationBuild)};\nexport const WASM_SHA256=${JSON.stringify(value.wasmSha256)};\nexport const RESOURCE_ROOTS=Object.freeze(${JSON.stringify(value.resourceRoots)});`
    },
    async generateBundle(_options, bundle) {
      if (!generation) return
      for (const chunk of Object.values(bundle)) if (chunk.type === "chunk" && (chunk.isEntry || Object.hasOwn(chunk.modules, resolved))) {
        chunk.code += `\n/*playsrc-generation:${JSON.stringify(await generation)}*/\n`
      }
    },
  }
}
