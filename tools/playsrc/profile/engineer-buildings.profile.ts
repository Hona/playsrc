import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"

test("authored Engineer build menus, stock objects and headed building pixels", async ({ page }, testInfo) => {
  await page.addInitScript(() => { ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {} })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map pl_upward")
  await entry.press("Enter")
  await page.waitForFunction(()=>{const main=document.querySelector<HTMLElement>("main");return main?.dataset.teamSelectionVisible==="true"||main?.dataset.phase==="Ready"||main?.dataset.phase==="Failed"},undefined,{timeout:600_000,polling:50})
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    if(await page.locator("main").getAttribute("data-console-visible")==="true")await page.keyboard.press("Backquote")
    await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
    await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "false")
  }
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  if (await page.locator("main").getAttribute("data-class-selection-visible") === "true") {
    await page.keyboard.press("Digit6")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  } else {
    await page.keyboard.press("Backquote")
    await entry.fill("joinclass engineer")
    await entry.press("Enter")
    await page.keyboard.press("Backquote")
  }
  await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "200", { timeout: 60_000 })
  await page.keyboard.press("Digit4")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "build")
  await expect(page.locator(".engineer-layer [data-vgui-name='HudMenuEngyBuild']")).toBeVisible()
  await expect(page.locator(".engineer-layer [data-vgui-name='AccountValue']")).toHaveText("200")
  const authoredIcons=await page.locator(".engineer-layer [data-vgui-name='BuildingIcon']").count()
  expect(authoredIcons).toBeGreaterThanOrEqual(4)
  const menu = await page.screenshot()
  await testInfo.attach("headed-authored-engineer-build-menu", { body: menu, contentType: "image/png" })
  await page.keyboard.press("Digit1")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "none")
  await page.waitForFunction(() => (document.querySelector<HTMLElement>("main")?.dataset.placement ?? "").startsWith("2:0:"), undefined, { timeout: 30_000 })
  if((await page.locator("main").getAttribute("data-placement"))?.startsWith("2:0:0:")){
    await page.keyboard.press("Backquote")
    await entry.fill("noclip")
    await entry.press("Enter")
    await page.keyboard.press("Backquote")
    await expect(page.locator("main")).toHaveAttribute("data-movement-mode","1")
    const departure=await page.evaluate(async()=>{
      const root=document.querySelector<HTMLElement>("main")!
      const position=()=>(root.dataset.cameraPosition??"").split(",").map(Number)
      const initial=position()
      const target=initial[0]!<0?[-1780,-1536]:initial[1]!>1000?[512,1230]:[810,490]
      const order=initial[0]!>=0&&initial[1]!>1000?[0,1]:[1,0]
      for(const axis of order){
        const current=position()[axis]!,increasing=target[axis]!>current
        const keys=axis===0?(increasing?["KeyW","KeyA"]:["KeyS","KeyD"]):(increasing?["KeyS","KeyA"]:["KeyW","KeyD"])
        for(const code of keys)dispatchEvent(new KeyboardEvent("keydown",{code,key:code.slice(3).toLowerCase(),bubbles:true}))
        const began=performance.now()
        try{while(increasing?position()[axis]!<target[axis]!-8:position()[axis]!>target[axis]!+8){if(performance.now()-began>5000)throw new Error(`axis=${axis};position=${position().join(",")}`);await new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()))}}
        finally{for(const code of keys)dispatchEvent(new KeyboardEvent("keyup",{code,key:code.slice(3).toLowerCase(),bubbles:true}))}
        for(let frame=0;frame<3;frame+=1)await new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()))
      }
      return{position:position(),placement:root.dataset.placement,target}
    })
    if(!departure.placement?.startsWith("2:0:1:"))throw new Error(`Engineer reached an invalid authored build surface: ${JSON.stringify(departure)}`)
  }
  await page.waitForFunction(() => (document.querySelector<HTMLElement>("main")?.dataset.placement ?? "").startsWith("2:0:1:"), undefined, { timeout: 30_000 })
  const blueprint = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-sentry-blueprint", { body: blueprint, contentType: "image/png" })
  await page.keyboard.press("Backquote")
  await entry.fill("+attack");await entry.press("Enter")
  try{await expect(page.locator("main")).toHaveAttribute("data-building-count", "1",{timeout:5000})}catch(error){const state=await page.evaluate(()=>{const root=document.querySelector<HTMLElement>("main")!;return{position:root.dataset.cameraPosition,placement:root.dataset.placement,weapon:root.dataset.weaponTrace,metal:root.dataset.metal,console:document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText,tick:root.dataset.snapshotTick}});throw new Error(`Engineer primary attack did not place its valid blueprint: ${JSON.stringify(state)}`,{cause:error})}
  await entry.fill("-attack");await entry.press("Enter")
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "70")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.buildings?.length === 1)
  const constructing = await page.evaluate(() => (globalThis as any).__playsrcProfile.buildings[0])
  expect(constructing.object).toEqual({ kind: 2, mode: 0 })
  expect(constructing.owner).toBe(1)
  expect(constructing.team).toBe(2)
  const built = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-constructed-sentry", { body: built, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    let previous = started
    const frames: number[] = []
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - started >= duration * 1000) resolve(); else requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - started) / 1000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames }
  }, seconds)
  expect(measurement.lastTick - measurement.firstTick).toBeGreaterThan(seconds * 60)

  await page.keyboard.press("Digit5")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "destroy")
  await page.keyboard.press("Digit1")
  await expect(page.locator("main")).toHaveAttribute("data-building-count", "0")
  const removed = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-sentry-destroyed", { body: removed, contentType: "image/png" })
  const presentPixels=decodeScreenshot(built),removedPixels=decodeScreenshot(removed)
  let changedBuildingPixels=0,redBuildingPixels=0
  for(let y=120;y<Math.min(presentPixels.height-80,600);y+=1)for(let x=280;x<Math.min(presentPixels.width-280,1000);x+=1){const offset=(y*presentPixels.width+x)*presentPixels.channels;const delta=Math.abs(presentPixels.pixels[offset]!-removedPixels.pixels[offset]!)+Math.abs(presentPixels.pixels[offset+1]!-removedPixels.pixels[offset+1]!)+Math.abs(presentPixels.pixels[offset+2]!-removedPixels.pixels[offset+2]!);if(delta>36){changedBuildingPixels+=1;if(presentPixels.pixels[offset]!>presentPixels.pixels[offset+2]!+8)redBuildingPixels+=1}}
  expect(changedBuildingPixels).toBeGreaterThan(100)
  const report = { schema: "playsrc-tf2-headed-engineer-buildings-v1", headed: true, target: "pl_upward", building: constructing, pixels:{changedBuildingPixels,redBuildingPixels,authoredIcons},
    simulation: { seconds: Number(measurement.seconds.toFixed(3)), ticksPerSecond: Number(((measurement.lastTick - measurement.firstTick) / measurement.seconds).toFixed(2)) },
    frames: summarizeFrameTimes(measurement.frames), screenshots: ["authored-build-menu", "sentry-blueprint", "constructed-sentry", "destroyed-sentry"] }
  const local = await loadLocalConfig(), directory = path.join(local.sourceCacheDir, "evidence", "tf2-engineer-buildings")
  await mkdir(directory, { recursive: true })
  await Promise.all([writeFile(path.join(directory, "pl_upward-engineer.json"), `${JSON.stringify(report, null, 2)}\n`), writeFile(path.join(directory, "build-menu.png"), menu), writeFile(path.join(directory, "sentry-blueprint.png"), blueprint), writeFile(path.join(directory, "sentry-built.png"), built), writeFile(path.join(directory, "sentry-destroyed.png"), removed)])
  console.log(`[engineer-buildings] ${JSON.stringify(report)}`)
})
