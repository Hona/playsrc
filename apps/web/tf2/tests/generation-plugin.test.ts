import { test, expect } from "bun:test"
import { createServer } from "vite"
import { generationPlugin } from "../generation-plugin"

test("invalidating Vite replaces the complete generation module instead of reusing cached define transforms", async () => {
  let generation = { applicationBuild: "a".repeat(64), wasmSha256: "b".repeat(64), resourceRoots: { jump_beef: "c".repeat(64) } }
  const server = await createServer({ configFile: false, server: { middlewareMode: true, watch: null }, plugins: [generationPlugin(() => generation)], optimizeDeps: { noDiscovery: true, include: [] } })
  try {
    const before = await server.transformRequest("virtual:playsrc-generation")
    expect(before?.code).toContain(generation.applicationBuild)
    expect(before?.code).toContain(generation.wasmSha256)
    generation = { applicationBuild: "d".repeat(64), wasmSha256: "e".repeat(64), resourceRoots: { jump_beef: "f".repeat(64) } }
    server.moduleGraph.invalidateAll()
    const after = await server.transformRequest("virtual:playsrc-generation")
    expect(after?.code).toContain(generation.applicationBuild)
    expect(after?.code).toContain(generation.wasmSha256)
    expect(after?.code).toContain(generation.resourceRoots.jump_beef)
    expect(after?.code).not.toContain("a".repeat(64))
  } finally { await server.close() }
})
