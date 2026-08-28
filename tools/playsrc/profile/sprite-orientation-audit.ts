import { writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, type Page } from "@playwright/test"
import { repositoryRoot } from "../src/config"

export async function auditSpriteOrientation(page: Page, directory: string, label: string, checkNative: (file: string) => Promise<void>) {
  const url = new URL("/particle-orientation-audit", page.url()).href
  await page.route(url, route => route.fulfill({ contentType: "text/html", body: `<!doctype html><title>SpriteCard Z-alignment pixel verification</title><style>body{margin:0;background:#111;color:white}</style><h3>Production SpriteCard: yaw and near-eye fade</h3><script type="module">import {createSpriteOrientationProbe} from '/@fs/${repositoryRoot}/packages/presentation/rendering/tests/fixtures/sprite-orientation-probe.ts';window.probe=await createSpriteOrientationProbe();</script>` }))
  await page.goto(url)
  await page.waitForFunction(() => (window as any).probe)
  const results = []
  for (const [distance, yaw] of [[3, 0], [0.75, 0], [0.25, 0], [3, Math.PI / 2]]) {
    const { pixels, ...result } = await page.evaluate(async ([distance, yaw]) => (window as any).probe.draw(distance, yaw), [distance, yaw])
    results.push(result)
    await writeFile(path.join(directory, `${label}-sprite-orientation-${results.length}.png`), Buffer.from(pixels.split(",")[1], "base64"))
    await checkNative(path.join(directory, `${label}-sprite-orientation-${results.length}.desktop.png`))
  }
  await writeFile(path.join(directory, `${label}-sprite-orientation.json`), JSON.stringify(results))
  expect(results[0]!.center[0]).toBeGreaterThan(240)
  expect(results[1]!.center[0]).toBeGreaterThan(55)
  expect(results[1]!.center[0]).toBeLessThan(75)
  expect(results[2]!.center.slice(0, 3)).toEqual([0, 0, 0])
  expect(results[3]!.center.slice(0, 3)).toEqual([0, 0, 0])
  await page.evaluate(() => (window as any).probe.destroy())
}
