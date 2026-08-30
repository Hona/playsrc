import path from "node:path"
import { writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import { test, expect, guardStartupInput } from "./application-test"
import { profileArtifact } from "./profile-artifacts"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"

test("actual full-roster KOTH preserves Float32 paint-to-device delivery", async ({ page }) => {
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, config = await loadLocalConfig(process.cwd())
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const native = await startupNativeReader(page, config.sourceCacheDir)
  const check = async () => requireStartupNative(await native.read())
  guardStartupInput(page, check)
  let result: any, pixels: Buffer | undefined, failure: string | null = null
  try {
    await check(); await page.goto("/")
    const main = page.locator("main")
    await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 30_000 })
    await check(); await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await check(); await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
    const dialog = page.getByRole("dialog", { name: "CREATE SERVER" })
    await check(); await dialog.locator("[data-vgui-name='MapList']").click()
    await check(); await page.getByRole("option", { name: "koth_viaduct", exact: true }).click()
    await check(); await dialog.getByRole("tab", { name: "GAME" }).click()
    await check(); await dialog.locator("[data-vgui-name='GameplayPage'] [data-vgui-name='NumPlayersTextEntry']").fill("23")
    await check(); await dialog.getByRole("button", { name: "Start", exact: true }).click()
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
    await chooseTf2Team(page, "red", check)
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 20_000 })
    await expect(main).toHaveAttribute("data-bot-count", "23", { timeout: 15_000 })
    const readyAt = await page.evaluate(() => performance.now())
    await check(); await page.locator("canvas.world-canvas").click()
    await page.waitForFunction(() => document.pointerLockElement?.matches("canvas.world-canvas"))
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.audio?.stats().contextState === "running")
    await check()
    result = await page.evaluate(async readyAt => {
      const p=(globalThis as any).__playsrcProfile, audio=p.audio, root=document.querySelector<HTMLElement>("main")!
      const start=performance.now(), before=audio.stats(), tick=Number(root.dataset.snapshotTick)
      const capture=await audio.capture(220500), ended=performance.now(), pcm=new Float32Array(capture.pcm)
      let nonzero=0,stereoDifferences=0,peak=0
      for(let i=0;i<pcm.length;i+=2){nonzero+=Number(pcm[i]!==0||pcm[i+1]!==0);stereoDifferences+=Number(pcm[i]!==pcm[i+1]);peak=Math.max(peak,Math.abs(pcm[i]!),Math.abs(pcm[i+1]!))}
      const sha256=[...new Uint8Array(await crypto.subtle.digest("SHA-256",capture.pcm))].map(v=>v.toString(16).padStart(2,"0")).join("")
      return { scope:"Short actual gameplay audio correctness, not sustained freeze or FPS acceptance",timeOrigin:performance.timeOrigin,readyAt,start,ended,sinceReadyMilliseconds:start-readyAt,
        before,after:audio.stats(),tickBefore:tick,tickAfter:Number(root.dataset.snapshotTick),bots:p.bots.length,phase:root.dataset.phase,
        capture:{frames:capture.frames,sampleFormat:capture.sampleFormat,sampleRate:capture.sampleRate,differingSamples:capture.differingSamples,uncoveredSamples:capture.uncoveredSamples,underruns:capture.underruns,bytes:capture.pcm.byteLength,sha256,nonzero,stereoDifferences,peak} }
    }, readyAt)
    await check()
    expect(result.phase).toBe("Ready"); expect(result.bots).toBe(23)
    expect(result.tickAfter).toBeGreaterThan(result.tickBefore)
    expect(result.capture.sampleFormat).toBe("f32le"); expect(result.capture.sampleRate).toBe(44100)
    expect(result.capture.differingSamples).toBe(0); expect(result.capture.uncoveredSamples).toBe(0)
    expect(result.capture.underruns).toBe(0); expect(result.capture.nonzero).toBeGreaterThan(1000)
    pixels=await page.screenshot()
  } catch(error) { failure=String(error); throw error }
  finally {
    await native.close()
    await profileArtifact(async () => {
      const image=pixels?{file:"simd-audio-playback.png",bytes:pixels.length,sha256:createHash("sha256").update(pixels).digest("hex")}:null
      await writeFile(path.join(directory,"simd-audio-playback.json"),JSON.stringify({result:result??null,failure,image,nativeAdmission:native.records},null,2))
      if(pixels)await writeFile(path.join(directory,"simd-audio-playback.png"),pixels)
    })
  }
})
