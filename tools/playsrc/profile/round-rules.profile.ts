import { createHash } from "node:crypto"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"

const timerSelector = "[data-vgui-runtime='tf2-hud'] [data-vgui-name='ObjectiveStatusTimePanel']"
const waitingSelector = "[data-vgui-runtime='tf2-hud'] [data-vgui-name='WaitingForPlayersPanel']"

async function visiblePixelEvidence(page: import("@playwright/test").Page, selector: string) {
  const panel = page.locator(selector)
  await expect(panel).toBeVisible()
  const bytes = await panel.screenshot({ animations: "disabled" })
  const pixels = await page.evaluate(async (input) => {
    const image = await createImageBitmap(new Blob([Uint8Array.from(input)], { type: "image/png" }))
    const canvas = document.createElement("canvas")
    canvas.width = image.width
    canvas.height = image.height
    const context = canvas.getContext("2d", { willReadFrequently: true })
    if (!context) throw new Error("Round HUD pixel evidence is unavailable")
    context.drawImage(image, 0, 0)
    const data = context.getImageData(0, 0, image.width, image.height).data
    let visible = 0
    for (let offset = 0; offset < data.length; offset += 4) {
      if (data[offset + 3]! > 0 && data[offset]! + data[offset + 1]! + data[offset + 2]! > 24) visible += 1
    }
    image.close()
    return { width: canvas.width, height: canvas.height, visible }
  }, [...bytes])
  expect(pixels.visible).toBeGreaterThan(100)
  return { ...pixels, screenshotSha256: createHash("sha256").update(bytes).digest("hex"), bytes }
}

test("authored Upward round HUD exposes waiting, setup, visible timer pixels and real-time fixed ticks", async ({ page }, testInfo) => {
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map pl_upward")
  await entry.press("Enter")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.teamSelectionVisible === "true" || root?.dataset.phase === "Ready" || root?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000 })
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "blue")
  }
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.roundProbe?.split(":")[1] === "1", undefined, { timeout: 15_000 })
  const waitingVisible = await page.locator(waitingSelector).isVisible()
  const waiting = await visiblePixelEvidence(page, waitingVisible ? waitingSelector : timerSelector)
  await testInfo.attach("headed-authored-waiting-timer", { body: waiting.bytes, contentType: "image/png" })
  const waitingTime = await page.locator(`${timerSelector} [data-vgui-name='TimePanelValue']`).innerText()
  expect(waitingTime).toMatch(/^0:[0-2]\d$/u)
  await page.waitForFunction(() => {
    const probe = document.querySelector<HTMLElement>("main")?.dataset.roundProbe?.split(":")
    return probe?.[0] === "4" && probe[1] === "0" && probe[2] === "1"
  }, undefined, { timeout: 45_000 })
  await expect(page.locator(waitingSelector)).toBeHidden()
  await expect(page.locator(`${timerSelector} [data-vgui-name='SetupLabel']`)).toBeVisible()
  const timer = await visiblePixelEvidence(page, timerSelector)
  await testInfo.attach("headed-authored-setup-timer", { body: timer.bytes, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const sample = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const firstTick = Number(root.dataset.snapshotTick)
    const firstTimer = Number(root.dataset.roundProbe?.split(":")[7])
    const started = performance.now()
    const frames: number[] = []
    let previous = started
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous)
        previous = now
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return {
      elapsedSeconds: (performance.now() - started) / 1000,
      ticks: Number(root.dataset.snapshotTick) - firstTick,
      timerElapsed: firstTimer - Number(root.dataset.roundProbe?.split(":")[7]),
      frames,
    }
  }, seconds)
  expect(sample.ticks).toBeGreaterThan(seconds * 63)
  expect(sample.timerElapsed).toBeGreaterThan(seconds - 0.25)
  expect(sample.timerElapsed).toBeLessThan(seconds + 0.5)
  const evidence = {
    waiting: { ...waiting, bytes: undefined },
    timer: { ...timer, bytes: undefined },
    elapsedSeconds: Number(sample.elapsedSeconds.toFixed(3)),
    ticks: sample.ticks,
    timerElapsed: Number(sample.timerElapsed.toFixed(3)),
    frameTimes: summarizeFrameTimes(sample.frames),
  }
  await testInfo.attach("headed-round-rules-evidence", { body: JSON.stringify(evidence, null, 2), contentType: "application/json" })
  console.log(JSON.stringify(evidence))
})
