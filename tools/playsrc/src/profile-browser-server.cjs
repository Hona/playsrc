// Keep Playwright's WebSocket upgrade server on its supported Node transport.
// Bun owns scheduling/leases; no page, context, or executable evidence is reused
// by this process. Each client connection owns fresh test contexts.
const { chromium } = require("@playwright/test")

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
  else console.log(JSON.stringify({ endpoint: server.wsEndpoint(), executable: server.process().spawnfile }))
})().catch(error => { console.error(error); process.exitCode = 1 })
