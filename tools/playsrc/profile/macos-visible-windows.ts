import { execFile } from "node:child_process"
import { access, mkdir, readFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { promisify } from "node:util"

export type Bounds = Readonly<{ X: number; Y: number; Width: number; Height: number }>
type Window = Readonly<{ id: number; pid: number; owner: string; layer: number; alpha: number; bounds: Bounds }>
export type MacWindowSnapshot = Readonly<{ windows: readonly Window[]; screens: readonly Bounds[]; cursorLayer: number }>
export type PageWindowFacts = Readonly<{ browserPid: number; targetId: string; cdpWindowId: number; bounds: Bounds; windowState: string; url: string }>
export type PageWindowLinkage = Readonly<{ browserPid: number; targetId: string; cdpWindowId: number; nativeWindowId: number }>

export function sameBounds(a: Bounds, b: Bounds): boolean {
  return a.X === b.X && a.Y === b.Y && a.Width === b.Width && a.Height === b.Height
}

/** CGWindowList is front-to-back. Nonzero layers include system permission
 * alerts; page focus/visibility alone cannot admit an unoccluded capture. */
export function admitMacWindow(snapshot: MacWindowSnapshot, facts: PageWindowFacts, established?: PageWindowLinkage) {
  const { browserPid, targetId, cdpWindowId } = facts
  if (!Number.isSafeInteger(browserPid) || browserPid < 1 || !targetId || !Number.isSafeInteger(cdpWindowId)
    || !Object.values(facts.bounds).every(Number.isFinite)) throw new Error("Incomplete page/native window facts")
  if (facts.windowState === "minimized") throw new Error("Measured page window is minimized")
  if (established && (established.browserPid !== browserPid || established.targetId !== targetId || established.cdpWindowId !== cdpWindowId)) {
    throw new Error("Measured page/window identity changed")
  }
  // CDP window IDs are NOT CGWindowIDs. Join once by the browser process and
  // exact outer bounds from Browser.getWindowForTarget, then retain both IDs.
  // Never choose by title, nearest geometry, z-order, focus or same-app alone.
  const candidates = snapshot.windows.filter(w => w.pid === browserPid && w.layer === 0 && w.alpha > 0 && sameBounds(w.bounds, facts.bounds))
  if (candidates.length > 1) throw new Error("Measured page/native window linkage is ambiguous")
  if (candidates.length !== 1) throw new Error("No on-screen native window matches the measured page's exact PID/bounds")
  if (established && candidates[0]!.id !== established.nativeWindowId) throw new Error("Established native drawing window identity changed")
  const target = snapshot.windows.indexOf(candidates[0]!)
  const window = snapshot.windows[target]!, b = window.bounds
  if (!(b.Width > 0 && b.Height > 0) || !snapshot.screens.some(s => b.X >= s.X && b.Y >= s.Y && b.X + b.Width <= s.X + s.Width && b.Y + b.Height <= s.Y + s.Height)) {
    throw new Error("Native browser window is not fully contained by a visible display")
  }
  // The system's cursor plane is expected native input, not a foreign alert.
  // Retain it separately; never move/hide it or ignore arbitrary overlay layers.
  const cursors = snapshot.windows.filter(w => w.layer === snapshot.cursorLayer && w.owner === "Window Server")
  const covering = snapshot.windows.slice(0, target).filter(w => !cursors.includes(w) && w.alpha > 0 && w.bounds.Width > 0 && w.bounds.Height > 0
    && w.bounds.X < b.X + b.Width && w.bounds.X + w.bounds.Width > b.X && w.bounds.Y < b.Y + b.Height && w.bounds.Y + w.bounds.Height > b.Y)
  const linkage: PageWindowLinkage = established ?? { browserPid, targetId, cdpWindowId, nativeWindowId: window.id }
  // Same-browser popups/notices also obscure real pixels. Retain their category,
  // but never exempt them from rejection or dismiss them to obtain admission.
  return { linkage, window, occluders: covering, browserOverlays: covering.filter(w => w.pid === browserPid), cursors }
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
