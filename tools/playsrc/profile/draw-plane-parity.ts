import { writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, type Page, type Locator } from "@playwright/test"

export function drawPlaneEvidenceUrl(applicationRoot: string, lightingParity: boolean): string {
  // The diagnostic must patch the application's actual module/prototype, not
  // a second copy imported from an isolated controller checkout.
  const root = applicationRoot.replaceAll("\\", "/").replace(/\/$/, "")
  return encodeURI(`/@fs/${root}/packages/presentation/rendering/src/${lightingParity ? "draw-lighting-evidence" : "skinning-evidence"}.ts`)
}

/** Post-sample (or correctness-only) GPU evidence. No simulation pause, bot
 * changes or sampled-boundary changes. An aligned camera is local to each
 * correctness capture and is always removed on success/failure. */
export async function auditDrawPlaneParity(page: Page, canvas: Locator, directory: string, label: string,
  lightingParity: boolean, native: (label: string) => Promise<void>) {
  await page.evaluate(async ({ url, lightingParity }) => {
    const module = await import(/* @vite-ignore */ url)
    ;(globalThis as any).__skinningEvidence = lightingParity ? module.installDrawLightingEvidence() : module.installSkinningEvidence()
  }, { url: drawPlaneEvidenceUrl(process.cwd(), lightingParity), lightingParity })
  const records = []
  try {
    for (const pass of ["main", "viewmodel"]) {
      if (pass === "main") {
        await page.evaluate(() => {
          const profile = (globalThis as any).__playsrcProfile
          const bot = profile.bots.find((bot: any) => bot.lifecycle === 1)
          if (!bot) throw new Error("No live bot for visible draw parity")
          const yaw = bot.yawDegrees * Math.PI / 180
          profile.displacementCameraOverride = {
            position: [bot.position[0] + Math.cos(yaw) * 64, bot.position[1] + Math.sin(yaw) * 64, bot.position[2] + 48],
            yawDegrees: bot.yawDegrees + 180, pitchDegrees: 0,
          }
        })
        await page.waitForFunction(() => document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition
          === (globalThis as any).__playsrcProfile.displacementCameraOverride.position.join(","), undefined, { timeout: 5_000 })
      }
      await native(`${pass}-before`)
      const record = await page.evaluate(async ({ label, pass }) => Promise.race([
        (globalThis as any).__skinningEvidence.capture(label, pass),
        new Promise((_, reject) => setTimeout(() => reject(new Error(`no skinned ${pass} pass`)), 8_000)),
      ]), { label, pass }) as any
      records.push({ ...record, performanceSample: false })
      await writeFile(path.join(directory, `${label}-skinning-parity.json`), JSON.stringify(records, null, 2))
      if (lightingParity) {
        expect(record.lightingDraws).toBeGreaterThan(0)
        expect(record.lightingValues).toBe(record.lightingDraws * 44)
      }
      expect(record.planes).toHaveLength(3)
      for (const plane of record.planes) {
        expect(plane.mismatches, `${pass}/${plane.plane}`).toBe(0)
        expect(plane.referenceSha256).toBe(plane.sha256)
        if (lightingParity) expect(plane.identicalDrawOrder).toBe(true)
        if (plane.plane === "color") expect(plane.actorPixels).toBeGreaterThan(40)
        if (plane.plane === "depth") expect(plane.channels[0]).toBeGreaterThan(1)
      }
      await writeFile(path.join(directory, `${label}-skinning-${pass}.png`), await canvas.screenshot({ timeout: 5_000 }))
      await native(`${pass}-after`)
      await page.evaluate(() => { delete (globalThis as any).__playsrcProfile.displacementCameraOverride })
    }
  } finally {
    await page.evaluate(() => {
      delete (globalThis as any).__playsrcProfile.displacementCameraOverride
      ;(globalThis as any).__skinningEvidence.dispose()
      delete (globalThis as any).__skinningEvidence
    })
  }
}
