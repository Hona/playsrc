import { execFile } from "node:child_process"
import { promisify } from "node:util"
import type { Page } from "@playwright/test"
import { admitMacWindow, macWindowReader, sameBounds, type MacWindowSnapshot, type PageWindowFacts, type PageWindowLinkage } from "./macos-visible-windows"

export type MacPageAdmission = {
  at: number
  linkage?: PageWindowLinkage
  page?: PageWindowFacts
  pageAfter?: PageWindowFacts
  snapshot?: MacWindowSnapshot
  snapshotAfter?: MacWindowSnapshot
  window?: ReturnType<typeof admitMacWindow>["window"]
  occluders?: ReturnType<typeof admitMacWindow>["occluders"]
  browserOverlays?: ReturnType<typeof admitMacWindow>["browserOverlays"]
  cursors?: ReturnType<typeof admitMacWindow>["cursors"]
  desktopScreenshot?: string
  document?: Readonly<{ url: string; visibility: string; focused: boolean }>
  error?: string
}

export function requireMacPageAdmission(record: MacPageAdmission): void {
  if (record.error) throw new Error(record.error)
  if (!record.linkage || !record.window) throw new Error("Missing measured page/native window linkage")
  if (record.occluders?.length) throw new Error(`Native drawing window is occluded: ${record.occluders.map(w => `${w.owner} pid=${w.pid} window=${w.id} layer=${w.layer}`).join(", ")}`)
  if (record.document && record.document.visibility !== "visible") throw new Error("Measured page is not the visible document in its native window")
}

/** Read-only native admission for one measured Page. Keep the established join
 * across navigation and resize; errors retain all facts for rejected evidence. */
export async function macPageAdmission(page: Page, cacheDir: string) {
  const readWindows = await macWindowReader(cacheDir)
  if (!readWindows) return undefined
  const pageCdp = await page.context().newCDPSession(page)
  const browserCdp = await page.context().browser()!.newBrowserCDPSession()
  const browsers = (await browserCdp.send("SystemInfo.getProcessInfo")).processInfo.filter(p => p.type === "browser")
  if (browsers.length !== 1) throw new Error("Cannot identify the measured page's browser process")
  const browserPid = browsers[0]!.id
  let established: PageWindowLinkage | undefined
  const facts = async (): Promise<PageWindowFacts> => {
    // Ask the session attached to this Page, never enumerate/select a browser's
    // first target (which could be a same-process popup).
    const { targetInfo } = await pageCdp.send("Target.getTargetInfo")
    if (targetInfo.type !== "page") throw new Error("Measured target is not a page")
    const { windowId, bounds } = await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId })
    if ([bounds.left, bounds.top, bounds.width, bounds.height].some(v => typeof v !== "number")) throw new Error("CDP page window lacks exact native bounds")
    return { browserPid, targetId: targetInfo.targetId, cdpWindowId: windowId, windowState: bounds.windowState ?? "unknown",
      bounds: { X: bounds.left!, Y: bounds.top!, Width: bounds.width!, Height: bounds.height! }, url: targetInfo.url }
  }
  return {
    async read(desktopScreenshot?: string): Promise<MacPageAdmission> {
      const record: MacPageAdmission = { at: Date.now(), linkage: established }
      try {
        record.page = await facts()
        // No renderer evaluation in the 500ms active-sample native monitor.
        // Document evidence accompanies establishment and endpoint pixels only;
        // the existing gameplay sampler owns continuous page visibility checks.
        if (!established || desktopScreenshot) record.document = await page.evaluate(() => ({ url: location.href, visibility: document.visibilityState, focused: document.hasFocus() }))
        record.snapshot = await readWindows()
        if (desktopScreenshot) {
          await promisify(execFile)("screencapture", ["-x", desktopScreenshot], { timeout: 5_000 })
          record.desktopScreenshot = desktopScreenshot
          record.snapshotAfter = await readWindows()
        }
        record.pageAfter = await facts()
        if (record.page.targetId !== record.pageAfter.targetId || record.page.cdpWindowId !== record.pageAfter.cdpWindowId
          || record.page.windowState !== record.pageAfter.windowState || !sameBounds(record.page.bounds, record.pageAfter.bounds)) {
          throw new Error("Measured page window changed during native readback")
        }
        const admission = admitMacWindow(record.snapshot, record.page, established)
        established = admission.linkage
        Object.assign(record, admission)
        if (record.snapshotAfter) {
          const after = admitMacWindow(record.snapshotAfter, record.pageAfter, established)
          // An overlay present on either side of desktop capture invalidates
          // admission. Do not let a later clear snapshot erase an obstruction.
          record.occluders = [...admission.occluders, ...after.occluders.filter(w => !admission.occluders.some(previous => previous.id === w.id))]
          record.browserOverlays = record.occluders.filter(w => w.pid === browserPid)
        }
        if (record.document && record.document.visibility !== "visible") throw new Error("Measured page is not the visible document in its native window")
      } catch (error) { record.error = String(error) }
      return record
    },
    async close() { await Promise.all([pageCdp.detach(), browserCdp.detach()]) },
  }
}
