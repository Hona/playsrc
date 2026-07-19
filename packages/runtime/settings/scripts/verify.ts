import {
  SETTING_OWNERS,
  TF2_SELECTED_OPTIONS,
  createSettingsState,
  decodeSettingsPersistence,
  encodeSettingsPersistence,
} from "../src"

const packageRoot = new URL("../", import.meta.url)

const build = await Bun.build({
  entrypoints: [new URL("../src/index.ts", import.meta.url).pathname],
  target: "browser",
  format: "esm",
  minify: true,
  sourcemap: "none",
  write: false,
})
if (!build.success || build.outputs.length !== 1) {
  throw new Error(`settings browser bundle failed: ${build.logs.map(String).join("\n")}`)
}

const machinePathPattern = /["'`]\/(?:Users|home|private\/var)\//u
const sideEffectPatterns = [
  /\blocalStorage\b/u,
  /\bsessionStorage\b/u,
  /\bindexedDB\b/u,
  /\bglobalThis\s*\.\s*(?:document|window|localStorage|sessionStorage|indexedDB)\b/u,
  /\b(?:HTMLElement|Storage|IDBDatabase|Window|Document)\b/u,
  /\bfetch\s*\(/u,
  /from\s+["'](?:node:)?(?:fs|path|http|https|database)/u,
  /from\s+["']@playsrc\//u,
]
const glob = new Bun.Glob("**/*.{ts,tsx,md,json}")
let scannedFiles = 0
for await (const relativePath of glob.scan({ cwd: packageRoot.pathname, onlyFiles: true })) {
  scannedFiles += 1
  const text = await Bun.file(new URL(relativePath, packageRoot)).text()
  if (machinePathPattern.test(text)) throw new Error(`${relativePath} contains a machine-local path`)
  if (relativePath.startsWith("src/")) {
    for (const pattern of sideEffectPatterns) {
      if (pattern.test(text)) throw new Error(`${relativePath} contains forbidden side-effect dependency ${pattern}`)
    }
  }
}

const owners = Object.fromEntries(SETTING_OWNERS.map((owner) => [owner, "available"]))
const state = createSettingsState({ catalog: TF2_SELECTED_OPTIONS, owners })
const started = Bun.nanoseconds()
for (let iteration = 0; iteration < 1_000; iteration += 1) {
  const begun = state.beginTransaction()
  if (!begun.ok) throw new Error(begun.diagnostic.message)
  const changed = state.setValue(begun.transactionId, "audio.effect-volume", (iteration % 101) / 100)
  if (!changed.ok) throw new Error(changed.diagnostic.message)
  const prepared = state.prepareApply(begun.transactionId)
  if (!prepared.ok) throw new Error(prepared.diagnostic.message)
  const settled = state.settleApply(
    prepared.plan.planId,
    prepared.plan.requests.map((request) => ({ requestId: request.requestId, status: "applied" as const })),
  )
  if (!settled.ok || !settled.complete) throw new Error("bounded transaction cycle did not complete")
}
const elapsedMilliseconds = (Bun.nanoseconds() - started) / 1_000_000
const snapshot = state.snapshot()
if (snapshot.journal.length !== TF2_SELECTED_OPTIONS.limits.maximumJournalEntries) {
  throw new Error("change journal did not retain its exact configured bound")
}
const persistence = encodeSettingsPersistence(TF2_SELECTED_OPTIONS, snapshot.current)
const decoded = decodeSettingsPersistence(TF2_SELECTED_OPTIONS, persistence)
if (!decoded.ok) throw new Error(decoded.diagnostic.message)

console.log(JSON.stringify({
  settings: TF2_SELECTED_OPTIONS.settings.length,
  convars: TF2_SELECTED_OPTIONS.convars.length,
  commands: TF2_SELECTED_OPTIONS.commands.length,
  physicalInputs: TF2_SELECTED_OPTIONS.bindingProfile?.inputs.length ?? 0,
  persistentBytes: persistence.byteLength,
  journalEntries: snapshot.journal.length,
  transactionCycles: 1_000,
  elapsedMilliseconds: Number(elapsedMilliseconds.toFixed(3)),
  bundleBytes: build.outputs[0].size,
  scannedFiles,
}))
