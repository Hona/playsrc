import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"

const SAMPLE_MILLISECONDS = 6_000
const CONSTRAINT_BLOCKER = "Payload cart visible model requires its authored breakable constraint and VPhysics rigid-body authority"

test("headed Upward track movers preserve real world movement and expose the authored cart constraint blocker", async ({ page }, testInfo) => {
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 180_000, polling: 25 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")

  const submit = async (command: string): Promise<void> => {
    if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await expect(entry).toBeVisible()
    await entry.fill(command)
    await page.keyboard.press("Enter")
  }
  await submit("map pl_upward")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Failed" || main?.dataset.phase === "Ready" || main?.dataset.teamSelectionVisible === "true"
  }, undefined, { timeout: 600_000, polling: 25 })
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "blue")
  }
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
  await page.waitForFunction((blocker) => {
    const value = document.querySelector<HTMLElement>("main")?.dataset
    return value?.phase === "Failed" || (value?.blockers ?? "").includes(blocker)
  }, CONSTRAINT_BLOCKER, { timeout: 30_000, polling: 20 })

  const initial = await page.locator("main").evaluate((main) => ({
    phase: (main as HTMLElement).dataset.phase,
    blockers: JSON.parse((main as HTMLElement).dataset.blockers ?? "[]") as string[],
    collision: (main as HTMLElement).dataset.collisionMoverProbe ?? "",
    tick: Number((main as HTMLElement).dataset.snapshotTick),
    position: ((main as HTMLElement).dataset.cameraPosition ?? "").split(",").map(Number),
  }))
  expect(initial.phase).toBe("Ready")
  expect(initial.blockers.filter((blocker) => blocker.includes(CONSTRAINT_BLOCKER))).toHaveLength(1)
  expect(initial.position).toHaveLength(3)
  expect(Number(initial.collision.split(":")[1])).toBeGreaterThan(0)

  const canvas = page.locator("canvas.world-canvas")
  await expect(canvas).toBeVisible()
  const before = await canvas.screenshot()
  await submit("noclip")
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.waitForFunction(() => Number(document.querySelector<HTMLElement>("main")?.dataset.movementMode) === 1, undefined, { timeout: 30_000, polling: 20 })
  const started = await page.evaluate(() => performance.now())
  const startTick = Number((await page.locator("main").getAttribute("data-snapshot-tick")) ?? "0")
  await page.evaluate(() => window.dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true })))
  await page.waitForFunction(() => Number(document.querySelector<HTMLElement>("main")?.dataset.wishSpeed) > 0, undefined, { timeout: 10_000, polling: 20 })
  await page.waitForTimeout(350)
  await page.evaluate(() => window.dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true })))
  const after = await canvas.screenshot()
  const remaining = SAMPLE_MILLISECONDS - await page.evaluate((begin) => performance.now() - begin, started)
  if (remaining > 0) await page.waitForTimeout(remaining)

  const final = await page.locator("main").evaluate((main) => ({
    phase: (main as HTMLElement).dataset.phase,
    blockers: JSON.parse((main as HTMLElement).dataset.blockers ?? "[]") as string[],
    collision: (main as HTMLElement).dataset.collisionMoverProbe ?? "",
    tick: Number((main as HTMLElement).dataset.snapshotTick),
    position: ((main as HTMLElement).dataset.cameraPosition ?? "").split(",").map(Number),
    performance: (main as HTMLElement).dataset.performance,
  }))
  const elapsed = await page.evaluate((begin) => performance.now() - begin, started)
  const ticksPerSecond = (final.tick - startTick) / elapsed * 1_000
  expect(final.phase).toBe("Ready")
  expect(ticksPerSecond).toBeGreaterThan(55)
  expect(Math.hypot(...final.position.map((value, axis) => value - initial.position[axis]!))).toBeGreaterThan(1)
  expect(createHash("sha256").update(after).digest("hex")).not.toBe(createHash("sha256").update(before).digest("hex"))
  expect(final.blockers.filter((blocker) => blocker.includes(CONSTRAINT_BLOCKER))).toHaveLength(1)

  const report = {
    schema: "playsrc-tf2-upward-tracktrain-profile-v1",
    elapsedMilliseconds: Number(elapsed.toFixed(2)),
    ticksPerSecond: Number(ticksPerSecond.toFixed(2)),
    initial,
    final,
    beforeSha256: createHash("sha256").update(before).digest("hex"),
    afterSha256: createHash("sha256").update(after).digest("hex"),
  }
  const configuration = await loadLocalConfig(process.cwd())
  const directory = path.join(configuration.sourceCacheDir, "evidence", "tf2-browser-performance")
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "upward-tracktrain-profile.json"), `${JSON.stringify(report, null, 2)}\n`)
  await testInfo.attach("headed-upward-tracktrain-performance", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(JSON.stringify({ elapsedMilliseconds: report.elapsedMilliseconds, ticksPerSecond: report.ticksPerSecond, collision: final.collision, constraintBlocked: true }))
})
