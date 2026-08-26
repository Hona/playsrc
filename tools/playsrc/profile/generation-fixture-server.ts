import { createServer } from "node:http"
import { readFile, readdir } from "node:fs/promises"
import path from "node:path"
import type { BrowserConfiguration } from "../../../apps/web/tf2/src/config"
import { objectPath } from "@playsrc/asset-store"
import { digest } from "./release-generations"

export type GenerationFixture = Readonly<{ name: string; output: string; configuration: BrowserConfiguration }>

// Real HTTP caches and independently retained hashed modules, not Playwright routing
// (routing disables Chromium's HTTP cache). All controls stay in the test process.
export async function generationFixtureServer(fixtures: readonly GenerationFixture[], assetDir: string) {
  const assets = new Map<string, { bytes: Buffer; hash: string }>()
  for (const fixture of fixtures) {
    for (const name of await readdir(path.join(fixture.output, "assets"))) {
      if (!/^[a-zA-Z0-9_-]+\.(js|css)$/u.test(name)) continue
      const bytes = await readFile(path.join(fixture.output, "assets", name))
      const hash = digest(bytes)
      if (assets.has(name) && assets.get(name)!.hash !== hash) throw new Error("Hashed fixture module name collision")
      assets.set(name, { bytes, hash })
    }
  }
  let active = fixtures[0]!
  const state = {
    html: active, configuration: active, configurations: [] as GenerationFixture[],
    requests: [] as Array<{ pathname: string; generation: string; at: number }>,
    workerDelayMilliseconds: 0,
    configurationGate: undefined as Promise<void> | undefined,
    activationDelays: [] as number[],
  }
  let origin = ""
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url!, origin)
      const pathname = url.pathname
      response.setHeader("Cross-Origin-Opener-Policy", "same-origin")
      response.setHeader("Cross-Origin-Embedder-Policy", "require-corp")
      response.setHeader("Cache-Control", "no-store")
      state.requests.push({ pathname, generation: state.html.name, at: performance.now() })
      if (pathname === "/tf2/" || pathname === "/tf2") {
        response.setHeader("Content-Type", "text/html")
        response.end(await readFile(path.join(state.html.output, "index.html")))
      } else if (pathname === "/tf2/playsrc-config.json") {
        const fixture = state.configurations.shift() ?? state.configuration
        if (state.configurationGate) await state.configurationGate
        response.setHeader("Content-Type", "application/json")
        response.end(JSON.stringify({ ...fixture.configuration, assetOrigin: origin }))
      } else if (pathname.startsWith("/tf2/assets/")) {
        const asset = assets.get(pathname.slice("/tf2/assets/".length))
        if (!asset) { response.writeHead(404).end(); return }
        if (pathname.includes("gameplay-worker-") && state.workerDelayMilliseconds) {
          const started = performance.now()
          const deadline = started + state.workerDelayMilliseconds
          do {
            await new Promise((resolve) => setTimeout(resolve, Math.max(1, Math.ceil(deadline - performance.now()))))
          } while (performance.now() < deadline)
          state.activationDelays.push(performance.now() - started)
        }
        response.setHeader("Content-Type", pathname.endsWith(".css") ? "text/css" : "text/javascript")
        response.setHeader("Cache-Control", "public, max-age=31536000, immutable")
        response.setHeader("ETag", `"${asset.hash}"`)
        response.end(asset.bytes)
      } else if (/^\/objects\/sha256\/[0-9a-f]{64}$/u.test(pathname)) {
        const hash = pathname.split("/").at(-1)!
        const bytes = await readFile(objectPath(assetDir, hash))
        if (digest(bytes) !== hash) throw new Error("Fixture immutable object corruption")
        response.setHeader("Content-Type", "application/octet-stream")
        response.setHeader("Content-Length", bytes.byteLength)
        response.setHeader("ETag", `"${hash}"`)
        response.setHeader("Cache-Control", "public, max-age=31536000, immutable, no-transform")
        response.end(bytes)
      } else { response.writeHead(404).end() }
    } catch (error) {
      response.writeHead(500, { "content-type": "text/plain" }).end(String(error))
    }
  })
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve))
  const address = server.address()
  if (!address || typeof address === "string") throw new Error("Fixture HTTP listener unavailable")
  origin = `http://127.0.0.1:${address.port}`
  return {
    origin, state,
    select(fixture: GenerationFixture) { active = fixture; state.html = active; state.configuration = active },
    async close() { server.closeAllConnections(); await new Promise<void>((resolve) => server.close(() => resolve())) },
  }
}
