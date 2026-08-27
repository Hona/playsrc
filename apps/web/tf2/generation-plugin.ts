import type { Plugin } from "vite"

export type BundleGeneration = Readonly<{ applicationBuild: string; wasmSha256: string; resourceRoots: Readonly<Record<string, string>> }>

export function readBundledGeneration(source: string): BundleGeneration {
  const records = [...source.matchAll(/\/\*playsrc-generation:(.*?)\*\//g)]
  if (records.length !== 1) throw new Error("Static bundle generation seal is absent or ambiguous")
  return JSON.parse(records[0]![1]!)
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
      for (const chunk of Object.values(bundle)) if (chunk.type === "chunk" && Object.hasOwn(chunk.modules, resolved)) {
        chunk.code += `\n/*playsrc-generation:${JSON.stringify(await generation)}*/\n`
      }
    },
  }
}
