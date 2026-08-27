import { execFile } from "node:child_process"
import { access, mkdir, readFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { promisify } from "node:util"

type Bounds = Readonly<{ X: number; Y: number; Width: number; Height: number }>
type Window = Readonly<{ id: number; pid: number; owner: string; layer: number; alpha: number; bounds: Bounds }>
export type MacWindowSnapshot = Readonly<{ windows: readonly Window[]; screens: readonly Bounds[]; cursorLayer: number }>

/** CGWindowList is front-to-back. Nonzero layers include system permission
 * alerts; page focus/visibility alone cannot admit an unoccluded capture. */
export function admitMacWindow(snapshot: MacWindowSnapshot, browserPid: number): Readonly<{ window: Window; occluders: readonly Window[]; cursors: readonly Window[] }> {
  const target = snapshot.windows.findIndex(window => window.pid === browserPid && window.layer === 0 && window.alpha > 0)
  if (target < 0) throw new Error("Native browser window is not on screen")
  const window = snapshot.windows[target]!, b = window.bounds
  if (!(b.Width > 0 && b.Height > 0) || !snapshot.screens.some(s => b.X >= s.X && b.Y >= s.Y && b.X + b.Width <= s.X + s.Width && b.Y + b.Height <= s.Y + s.Height)) {
    throw new Error("Native browser window is not fully contained by a visible display")
  }
  // The system's cursor plane is expected native input, not a foreign alert.
  // Retain it separately; never move/hide it or ignore arbitrary overlay layers.
  const cursors = snapshot.windows.filter(w => w.layer === snapshot.cursorLayer && w.owner === "Window Server")
  const occluders = snapshot.windows.slice(0, target).filter(w => !cursors.includes(w) && w.alpha > 0 && w.bounds.Width > 0 && w.bounds.Height > 0
    && w.bounds.X < b.X + b.Width && w.bounds.X + w.bounds.Width > b.X && w.bounds.Y < b.Y + b.Height && w.bounds.Y + w.bounds.Height > b.Y)
  return { window, occluders, cursors }
}

export async function macWindowReader(cacheDir: string): Promise<(() => Promise<MacWindowSnapshot>) | undefined> {
  if (process.platform !== "darwin") return undefined
  const execute = promisify(execFile), source = fileURLToPath(new URL("./macos-visible-windows.m", import.meta.url))
  const hash = createHash("sha256").update(await readFile(source)).digest("hex")
  const directory = path.join(cacheDir, "profile-tools"), executable = path.join(directory, `visible-windows-${hash}`)
  try { await access(executable) } catch {
    await mkdir(directory, { recursive: true })
    await execute("xcrun", ["clang", "-fobjc-arc", "-O2", "-Wall", "-Wextra", "-Werror", source, "-framework", "AppKit", "-framework", "CoreGraphics", "-o", executable], { timeout: 15_000 })
  }
  return async () => JSON.parse((await execute(executable, [], { timeout: 2_000 })).stdout)
}
