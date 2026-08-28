import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { auditSpriteOrientation } from "./sprite-orientation-audit"
import { macPageAdmission, requireMacPageAdmission, type MacPageAdmission } from "./macos-page-admission"

test("visible SpriteCard Z-alignment and post-clamp proximity fading", async ({ page, baseURL }) => {
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory || !baseURL) throw new Error("Run the visible fixture through the checked profile runner")
  await mkdir(directory, { recursive: true })
  const reader = await macPageAdmission(page, (await loadLocalConfig()).sourceCacheDir)
  const records: MacPageAdmission[] = []
  try {
    await auditSpriteOrientation(page, directory, "sprite", async file => {
      if (reader) { const record = await reader.read(file); records.push(record); requireMacPageAdmission(record) }
    }, baseURL)
  } finally {
    await writeFile(path.join(directory, "native-admission.json"), JSON.stringify({ performanceSample: false, records }))
    await reader?.close()
  }
})
