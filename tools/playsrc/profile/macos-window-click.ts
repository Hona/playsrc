import { execFile } from "node:child_process"
import { access, mkdir, readFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { promisify } from "node:util"
import type { Bounds } from "./macos-visible-windows"

export async function macWindowClick(cacheDir: string, pid: number, windowId: number, bounds: Bounds): Promise<unknown> {
  if (!Number.isSafeInteger(pid) || pid < 1 || !Number.isSafeInteger(windowId) || windowId < 1
      || ![bounds.X, bounds.Y, bounds.Width, bounds.Height].every(Number.isFinite) || bounds.Width <= 0 || bounds.Height <= 0) {
    throw new Error("Native click requires an exact process, window and finite bounds")
  }
  const execute = promisify(execFile), source = fileURLToPath(new URL("./macos-window-click.m", import.meta.url))
  const hash = createHash("sha256").update(await readFile(source)).digest("hex")
  const directory = path.join(cacheDir, "profile-tools"), executable = path.join(directory, `window-click-${hash}`)
  try { await access(executable) } catch {
    await mkdir(directory, { recursive: true })
    await execute("xcrun", ["clang", "-fobjc-arc", "-O2", "-Wall", "-Wextra", "-Werror", source, "-framework", "AppKit", "-framework", "CoreGraphics", "-o", executable], { timeout: 15_000 })
  }
  return JSON.parse((await execute(executable, [pid, windowId, bounds.X, bounds.Y, bounds.Width, bounds.Height].map(String), { timeout: 2_000 })).stdout)
}
