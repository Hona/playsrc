import { createHash } from "node:crypto"
import { writeFile } from "node:fs/promises"
import path from "node:path"
import { test, expect, guardStartupInput } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative, startupPixelEvidence } from "./static-startup-gate"

test.use({ preserveStartupMovie: true, allowRecoverableApplicationFailure: true })

test("a failed application module shows independent actionable pixels", async ({ page }) => {
  await page.route("**/src/main.tsx*", route => route.abort("failed"))
  await page.goto("/")
  await expect(page.locator("#bootstrap-status")).toHaveAttribute("role", "alert")
  await expect(page.getByRole("heading", { name: "Unable to start TF2" })).toBeVisible()
  await expect(page.getByRole("link", { name: "Reload", exact: true })).toBeVisible()
  const native = await startupNativeReader(page, (await loadLocalConfig()).sourceCacheDir)
  try {
    // A failed module can settle before the new native browser window's outer
    // geometry does. Wait once for that window, never focus or move it.
    try { requireStartupNative(await native.read()) }
    catch (error) {
      if (!String(error).includes("No on-screen native window matches")) throw error
      await page.waitForTimeout(250)
      requireStartupNative(await native.read())
    }
    const bytes = await page.screenshot()
    const output = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
    await writeFile(path.join(output, "module-failure.page.png"), bytes)
    await writeFile(path.join(output, "module-failure.json"), JSON.stringify({ fault: "local main module request aborted", pixels: startupPixelEvidence(bytes), bytes: bytes.length, native: native.records }))
  } finally { await native.close() }
})

test("the authenticated audible movie presents frames and reaches its menu", async ({ page }) => {
  const native = await startupNativeReader(page, (await loadLocalConfig()).sourceCacheDir)
  const admit = async () => requireStartupNative(await native.read())
  guardStartupInput(page, admit)
  const output = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  try {
    await page.goto("/")
    const main = page.locator("main")
    await expect(main).toHaveAttribute("data-startup-state", /Playing|AwaitingGesture|Failed/, { timeout: 60_000 })
    expect(await main.getAttribute("data-startup-state")).not.toBe("Failed")
    if (await main.getAttribute("data-startup-state") === "AwaitingGesture") {
      await admit()
      await page.getByRole("button", { name: "Play intro", exact: true }).click()
    }
    await expect(main).toHaveAttribute("data-startup-state", "Playing")
    const frames = []
    // The authenticated clip has an intentional black transition at 7s.
    for (const time of [2, 8]) {
      await page.waitForFunction(time => document.querySelector<HTMLVideoElement>("video")!.currentTime >= time, time, { timeout: 15_000 })
      await admit()
      const bytes = await page.screenshot()
      const file = `movie-${time}.page.png`
      await writeFile(path.join(output, file), bytes)
      frames.push({ file, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), pixels: startupPixelEvidence(bytes),
        video: await page.locator("video").evaluate((video: HTMLVideoElement) => ({ width: video.videoWidth, height: video.videoHeight, duration: video.duration, currentTime: video.currentTime, muted: video.muted, paused: video.paused })) })
    }
    expect(frames[0]!.sha256).not.toBe(frames[1]!.sha256)
    for (const frame of frames) expect(frame.video).toMatchObject({ width: 1440, height: 1080, duration: 10.051, muted: false, paused: false })
    await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 30_000 })
    const probe = JSON.parse(await main.getAttribute("data-startup-media-probe") ?? "null")
    expect(probe.chronology.some((event: any) => event.event === "first-presented-frame")).toBe(true)
    await writeFile(path.join(output, "startup-diagnostics.json"), JSON.stringify({ scope: "supported desktop browser, not iPhone acceptance", frames, probe, native: native.records }, null, 2))
  } finally { await native.close() }
})
