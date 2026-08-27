import { mkdir, mkdtemp, writeFile } from "node:fs/promises"
import path from "node:path"
import type { LocalConfig } from "./config"

export async function createEvidenceDirectory(config: Pick<LocalConfig, "sourceCacheDir">, label: string): Promise<string> {
  if (!/^[a-z0-9-]+$/.test(label)) throw new Error("Invalid evidence label")
  const root = path.join(config.sourceCacheDir, "evidence")
  await mkdir(root, { recursive: true })
  const directory = await mkdtemp(path.join(root, `${label}-`))
  await writeFile(path.join(directory, ".gitignore"), "*\n")
  return directory
}
