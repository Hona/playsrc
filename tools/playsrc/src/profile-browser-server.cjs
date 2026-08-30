// Keep Playwright's WebSocket upgrade server on its supported Node transport.
// Bun owns scheduling/leases; no page, context, or executable evidence is reused
// by this process. Each client connection owns fresh test contexts.
const { chromium } = require("@playwright/test")
const path = require("node:path")
const { createRequire } = require("node:module")

// Resolve the pinned Playwright registry without starting any browser process.
// This is preparation, including channel existence/installation validation.
if (process.argv[2] === "prepare") {
  const launch = JSON.parse(process.argv[3])
  const core = createRequire(createRequire(require.resolve("@playwright/test")).resolve("playwright"))
  const { registry: { registry } } = require(path.join(path.dirname(core.resolve("playwright-core")), "lib/coreBundle.js"))
  const executable = launch.executablePath || registry.findExecutable(launch.channel || "chromium")?.executablePathOrDie("javascript")
  if (!executable || !path.isAbsolute(executable)) throw new Error("Configured Chromium executable is not an absolute installed path")
  console.log(JSON.stringify({ executable }))
  process.exit(0)
}

let server
let stopping = false
async function stop() {
  stopping = true
  if (!server) return
  const deadline = setTimeout(() => { void server.kill() }, 5_000)
  try { await server.close() }
  finally { clearTimeout(deadline) }
}
process.once("SIGTERM", () => { void stop() })
process.once("SIGINT", () => { void stop() })
process.stdin.once("data", () => { void stop() })
process.stdin.once("end", () => { void stop() })
process.stdin.once("error", () => { void stop() })

void (async () => {
  server = await chromium.launchServer({ ...JSON.parse(process.argv[2]), host: "127.0.0.1", headless: false, timeout: 20_000 })
  server.on("close", () => process.exit(0))
  if (stopping) await stop()
  else console.log(JSON.stringify({ endpoint: server.wsEndpoint(), executable: server.process().spawnfile,
    browserPid: server.process().pid, arguments: server.process().spawnargs.slice(1) }))
})().catch(async error => {
  console.error(error)
  // A failed launch still has the lease owner's open stdin. It must exit, not
  // leave the enclosing delegated job waiting for its full command deadline.
  try { await stop() } finally { process.exit(1) }
})
