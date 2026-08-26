import type { Plugin } from "vite"

export type BundleGeneration = Readonly<{ applicationBuild: string; wasmSha256: string; resourceRoots: Readonly<Record<string, string>> }>

export function generationPlugin(current: () => BundleGeneration | Promise<BundleGeneration>): Plugin {
  const name = "virtual:playsrc-generation"
  const resolved = `\0${name}`
  return {
    name: "playsrc-generation",
    resolveId(id) { if (id === name) return resolved },
    async load(id) {
      if (id !== resolved) return
      const generation = await current()
      const hash = /^[0-9a-f]{64}$/
      if (!hash.test(generation.applicationBuild) || !hash.test(generation.wasmSha256)
        || Object.keys(generation.resourceRoots).length === 0 || Object.values(generation.resourceRoots).some((root) => !hash.test(root))) {
        throw new Error("Browser bundle generation is malformed")
      }
      return `export const APPLICATION_BUILD=${JSON.stringify(generation.applicationBuild)};\nexport const WASM_SHA256=${JSON.stringify(generation.wasmSha256)};\nexport const RESOURCE_ROOTS=Object.freeze(${JSON.stringify(generation.resourceRoots)});`
    },
  }
}
