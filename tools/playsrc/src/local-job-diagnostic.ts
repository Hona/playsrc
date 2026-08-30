import path from "node:path"
import { loadLocalConfig } from "./config"
import { borrowedWindowsJobLock } from "./windows-job-native"

// Harmless workload for real native consent/teardown tests. No browser, game,
// synthetic receipt, simulated input or alternate authorization path.
const [milliseconds, exit] = process.argv.slice(2)
if (!/^\d{1,5}$/.test(milliseconds ?? "") || Number(milliseconds) > 30_000 || !/^[01]$/.test(exit ?? "")) throw new Error("Expected diagnostic milliseconds (0..30000) and exit (0|1)")
if (process.platform === "win32") {
  const config = await loadLocalConfig()
  const ownership = await borrowedWindowsJobLock(path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock"), ["diagnostic", milliseconds!, exit!])
  if (!ownership) throw new Error("Diagnostic workload has no native consent ownership")
  console.log(JSON.stringify({ borrowedOwnership: ownership }))
}
console.log("native diagnostic workload")
await new Promise(resolve => setTimeout(resolve, Number(milliseconds)))
process.exitCode = Number(exit)
