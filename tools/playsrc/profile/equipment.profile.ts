import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })

test("authored backpack native equip and browser restart persistence", async ({ page }) => {
  test.setTimeout(100_000)
  const local = await loadLocalConfig(), directory = path.join(local.sourceCacheDir, "profiles/equipment")
  await mkdir(directory, { recursive: true })
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 15_000 })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.locator("[data-vgui-name='CharacterSetupButton']").click()
  const equipment = page.locator(".equipment-layer")
  await expect(equipment.locator("[data-vgui-name='Class3']")).toBeVisible({ timeout: 20_000 })
  await equipment.locator("[data-vgui-name='BackpackButton']").click()
  await expect(equipment.locator("[data-vgui-name='Itemitem-378']")).toBeVisible()
  const capture = await page.screenshot({ path: path.join(directory, "backpack.png") })
  const pixels = decodeScreenshot(capture)
  const hat = await equipment.locator("[data-vgui-name='Itemitem-378']").boundingBox()
  expect(hat).not.toBeNull()
  let purple = 0
  for (let y = Math.floor(hat!.y); y < hat!.y + hat!.height; y++) for (let x = Math.floor(hat!.x); x < hat!.x + hat!.width; x++) {
    const at = (y * pixels.width + x) * pixels.channels
    const r = pixels.pixels[at]!, g = pixels.pixels[at + 1]!, b = pixels.pixels[at + 2]!
    if (r > 60 && b > 60 && r > g * 1.3 && b > g * 1.3) purple++
  }
  expect(purple, "Unusual quality must produce actual purple border pixels").toBeGreaterThan(20)
  await equipment.locator("[data-vgui-name='Itemitem-378']").click()
  await equipment.locator("[data-vgui-name='Itemitem-378']").click()
  await expect(equipment.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible()
  const saved = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
  expect(saved?.length).toBe(924)
  await page.screenshot({ path: path.join(directory, "equipped.png") })
  await page.reload({ waitUntil: "domcontentloaded" })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.locator("[data-vgui-name='CharacterSetupButton']").click()
  await equipment.locator("[data-vgui-name='Class3']").click()
  expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
  await expect(equipment.locator("[data-vgui-name='Itemslot-7'] [data-vgui-name='ItemIcon']")).toBeVisible()
  expect(errors).toEqual([])
  await writeFile(path.join(directory, "native-summary.json"), JSON.stringify({ purplePixels: purple, storageBytes: 692, errors }))
})
