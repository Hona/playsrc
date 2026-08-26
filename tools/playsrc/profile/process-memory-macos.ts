import { execFile } from "node:child_process"
import { access, mkdir, readFile } from "node:fs/promises"
import path from "node:path"
import { promisify } from "node:util"
import { createHash } from "node:crypto"
import { fileURLToPath } from "node:url"

const execute = promisify(execFile)

export async function macosProcessMemorySampler(cacheDir: string): Promise<string | undefined> {
  if (process.platform !== "darwin") return undefined
  const source = fileURLToPath(new URL("./process-memory-macos.c", import.meta.url))
  const hash = createHash("sha256").update(await readFile(source)).digest("hex")
  const directory = path.join(cacheDir, "profile-tools")
  const executable = path.join(directory, `process-memory-${hash}`)
  try { await access(executable); return executable } catch {}
  await mkdir(directory, { recursive: true })
  await execute("xcrun", ["cc", "-O2", "-Wall", "-Wextra", "-Werror", source, "-o", executable], { timeout: 15_000 })
  return executable
}
