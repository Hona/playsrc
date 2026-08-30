import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"

type Node = { name: string; value: string | null; children: Node[] }
type Source = { logicalPath: string; sha256: string; document: Node[] | null }

test("configured CEx borders retain authored square edges and state colors", async ({ page }, testInfo) => {
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-vgui-button-parity", `states-${Date.now()}`)
  await mkdir(directory, { recursive: true })
  const generated = await readFile(new URL("../../../games/tf2/browser/src/ui-resources/configured.generated.ts", import.meta.url), "utf8")
  const input = JSON.parse(generated.split("export const configuredTf2UiResourceInput: unknown = ")[1]!) as { contentBuild: string; resources: Source[] }
  const client = input.resources.find(source => source.logicalPath === "resource/clientscheme.res")!
  const root = client.document!.find(node => node.name.toLowerCase() === "scheme")!
  const colors = root.children.find(node => node.name === "Colors")!.children
  const color = (name: string) => colors.find(node => node.name === name)!.value!.split(/\s+/u).map(Number).slice(0, 3)
  const fill = color("TanDark")
  const armed = color("TFOrange")
  const edge = color("TanDarker")
  const records: unknown[] = []
  const capture = async (name: string, selector: string) => {
    const control = page.locator(selector)
    await expect(control).toBeVisible()
    const bytes = await control.screenshot()
    const destination = path.join(directory, `${name}.png`)
    await writeFile(destination, bytes)
    const image = decodeScreenshot(bytes)
    const dpr = await page.evaluate(() => devicePixelRatio)
    const pixel = (x: number, y: number) => {
      const offset = (Math.floor(y * dpr) * image.width + Math.floor(x * dpr)) * image.channels
      return Array.from(image.pixels.subarray(offset, offset + 3))
    }
    records.push({ name, path: destination, sha256: createHash("sha256").update(bytes).digest("hex"), bytes: bytes.length,
      width: image.width, height: image.height, dpr, bounds: await control.boundingBox(),
      state: await control.evaluate(element => ({ armed: (element as HTMLElement).dataset.armed,
        depressed: (element as HTMLElement).dataset.depressed, selected: (element as HTMLElement).dataset.selected,
        focused: (element as HTMLElement).dataset.focused, disabled: element.getAttribute("aria-disabled") })),
      pixelNearCorner: pixel(2, 2), pixelEdge: pixel(0, 2) })
    await writeFile(path.join(directory, "captures.json"), JSON.stringify(records, null, 2))
    return { pixel }
  }
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
  const selector = '[data-vgui-name="NewUserForumsButton"]'
  const button = page.locator(selector)
  await page.mouse.move(600, 300)
  const normal = await capture("normal", selector)
  expect(normal.pixel(2, 2)).toEqual(fill)
  expect(normal.pixel(0, 2)).toEqual(edge)
  await button.hover()
  expect((await capture("armed", selector)).pixel(2, 2)).toEqual(armed)
  await page.mouse.down()
  expect((await capture("depressed", selector)).pixel(2, 2)).toEqual(armed)
  await page.mouse.move(600, 300)
  await page.mouse.up() // Cancel rather than launch the external forum command.
  // Button::OnCursorExited deliberately retains armed while selected. Exercise
  // the subsequent unselected enter/exit rather than inventing a release reset.
  await button.hover()
  await page.mouse.move(600, 300)
  await expect(button).toHaveAttribute("data-armed", "false")
  await expect(button).toHaveAttribute("data-focused", "true")
  expect((await capture("restored-authored-focus", selector)).pixel(2, 2)).toEqual(fill)
  await capture("disabled", '[data-vgui-name="AchievementsButton"]')
  await page.locator('[data-vgui-name="SettingsButton"]').click()
  await capture("options", '[data-vgui-runtime="tf2-options"]')
  await page.keyboard.press("Escape")
  const screenshot = await page.screenshot()
  await writeFile(path.join(directory, "main-menu.png"), screenshot)
  records.push({ name: "main-menu", path: path.join(directory, "main-menu.png"), bytes: screenshot.length,
    sha256: createHash("sha256").update(screenshot).digest("hex") })
  await page.locator('[data-vgui-name="FindAGameButton"]').click()
  await page.locator('[data-vgui-name="TrainingEntry"] [data-vgui-name="ModeButton"]').click()
  const practice = '.local-match-layer [data-vgui-name="OfflinePracticePanel"] [data-vgui-name="StartButton"]'
  await capture("practice-normal", practice)
  await page.locator(practice).hover()
  await capture("practice-armed", practice)
  await page.keyboard.press("Escape")
  await expect(page.locator("main")).toHaveAttribute("data-local-match-visible", "false")
  await page.locator('[data-vgui-name="FindAGameButton"]').click()
  await page.locator('[data-vgui-name="CreateServerEntry"] [data-vgui-name="ModeButton"]').click()
  await capture("create-server", '.local-match-layer [data-vgui-name="CreateMultiplayerGameDialog"]')
  await page.keyboard.press("Escape")
  const evidence = { platform: process.platform, contentBuild: input.contentBuild, scheme: { path: client.logicalPath, sha256: client.sha256 },
    resource: input.resources.filter(source => ["resource/ui/mainmenuoverride.res", "resource/ui/training/modeselection/modepanel.res"].includes(source.logicalPath)).map(({ logicalPath, sha256 }) => ({ logicalPath, sha256 })), records }
  await writeFile(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2))
  await testInfo.attach("authored-button-pixels", { body: JSON.stringify(evidence), contentType: "application/json" })
  console.log(`PLAYSRCBUTTONPARITY ${JSON.stringify(evidence)}`)
})
