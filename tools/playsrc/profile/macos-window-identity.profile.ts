import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { test, expect } from "./application-test"
import { macPageAdmission, requireMacPageAdmission, type MacPageAdmission } from "./macos-page-admission"

test("native admission retains the actual profile page across popup, navigation and resize", async ({ page, context }) => {
  test.skip(process.platform !== "darwin", "WindowServer admission requires a real Mac desktop")
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Run native admission through the checked headed profile runner")
  await mkdir(directory, { recursive: true })
  const reader = (await macPageAdmission(page, sourceCacheDir))!
  const records: Array<MacPageAdmission & { phase: string }> = []
  const read = async (phase: string) => {
    const record = { ...await reader.read(path.join(directory, `${phase}.desktop.png`)), phase }
    records.push(record)
    await page.screenshot({ path: path.join(directory, `${phase}.page.png`) })
    await writeFile(path.join(directory, "native-admission.json"), JSON.stringify({ performanceSample: false, records }, null, 2))
    return record
  }
  let popup: import("@playwright/test").Page | undefined
  try {
    await page.goto("/", { waitUntil: "domcontentloaded" })
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    await page.bringToFront()
    const established = await read("established")
    requireMacPageAdmission(established)
    await page.reload({ waitUntil: "domcontentloaded" })
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    const application = await read("application")
    requireMacPageAdmission(application)
    expect(application.linkage).toEqual(established.linkage)
    // Five real seconds of visible application activity, no game benchmark or
    // timing/quality override. All native reads stay outside the active interval.
    await page.waitForTimeout(5_000)
    const popupUrl = new URL("/native-admission-owned-popup", page.url()).href
    await context.route(popupUrl, route => route.fulfill({ contentType: "text/html", body: '<!doctype html><title>Owned admission popup</title><body style="background:#e53;color:white;font:24px sans-serif">Owned popup: must reject main-page admission</body>' }))
    const popupReady = context.waitForEvent("page")
    await page.evaluate(url => window.open(url, "admission-popup", "popup,width=320,height=174"), popupUrl)
    popup = await popupReady
    await popup.waitForLoadState()
    const popupCdp = await context.newCDPSession(popup)
    const browserCdp = await context.browser()!.newBrowserCDPSession()
    try {
      const { targetInfo } = await popupCdp.send("Target.getTargetInfo")
      const popupWindow = await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId })
      expect(popupWindow.windowId).not.toBe(established.linkage!.cdpWindowId)
      const b = application.window!.bounds
      await browserCdp.send("Browser.setWindowBounds", { windowId: popupWindow.windowId, bounds: { left: b.X + 100, top: b.Y + 150, width: 320, height: 174 } })
      await popup.bringToFront()
      const covered = await read("popup-covered")
      expect(covered.linkage).toEqual(established.linkage)
      expect(covered.error).toBeUndefined()
      expect(covered.browserOverlays!.length).toBeGreaterThan(0)
      expect(() => requireMacPageAdmission(covered)).toThrow("occluded")
      await writeFile(path.join(directory, "popup-linkage.json"), JSON.stringify({ targetInfo, window: await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId }) }, null, 2))
      const auditUrl = new URL("/native-admission-navigation", page.url()).href
      await page.route(auditUrl, route => route.fulfill({ contentType: "text/html", body: '<!doctype html><title>Native identity after navigation</title><body style="background:#246;color:white;font:32px sans-serif">Same measured page and drawing window after navigation and resize</body>' }))
      await page.goto(auditUrl)
      await page.setViewportSize({ width: 1024, height: 700 })
      const resized = await read("navigation-resized-covered")
      expect(resized.error).toBeUndefined()
      expect(resized.linkage).toEqual(established.linkage)
      expect(resized.window!.bounds).not.toEqual(application.window!.bounds)
      expect(resized.browserOverlays!.length).toBeGreaterThan(0)
      expect(() => requireMacPageAdmission(resized)).toThrow("occluded")
    } finally { await popupCdp.detach(); await browserCdp.detach() }
    // Close only the popup created by this test, after retaining its rejection.
    await popup.close()
    popup = undefined
    const restored = await read("owned-popup-closed")
    expect(restored.linkage).toEqual(established.linkage)
    requireMacPageAdmission(restored)
  } finally { await popup?.close(); await reader.close() }
})
