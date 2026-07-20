import { mkdir, rm, writeFile } from "node:fs/promises"
import path from "node:path"

const TARGET = "jump_beef"
const processStarted = performance.now()
const startedMemory = process.memoryUsage()
const [{ loadLocalConfig }, { startDevelopment }] = await Promise.all([
  import("../src/config"),
  import("../src/dev"),
])
const config = await loadLocalConfig()
const outputDirectory = path.join(config.sourceCacheDir, "profiles", "startup", TARGET)
await rm(outputDirectory, { recursive: true, force: true })
await mkdir(outputDirectory, { recursive: true })

const peak = { ...startedMemory }
const sample = () => {
  const current = process.memoryUsage()
  for (const name of ["rss", "heapTotal", "heapUsed", "external", "arrayBuffers"] as const) {
    peak[name] = Math.max(peak[name], current[name])
  }
}
const sampler = setInterval(sample, 10)
let owner: Awaited<ReturnType<typeof startDevelopment>> | undefined
try {
  owner = await startDevelopment(config, TARGET)
  clearInterval(sampler)
  sample()
  const report = Object.freeze({
    schema: "playsrc-development-startup-profile-v1",
    target: TARGET,
    readyMilliseconds: Math.round(performance.now() - processStarted),
    stages: owner.startup,
    memory: Object.freeze({
      start: Object.freeze(startedMemory),
      peak: Object.freeze(peak),
    }),
  })
  await writeFile(path.join(outputDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify(report))
} finally {
  clearInterval(sampler)
  await owner?.close()
}
