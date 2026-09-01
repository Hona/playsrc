import { mkdir,writeFile } from "node:fs/promises"
import path from "node:path"
import { expect,test } from "./demoman-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"
import { profileArtifact } from "./profile-artifacts"

test.use({video:{mode:"on",size:{width:1280,height:720}}})

test("Demoman launches, sticks and detonates an owned physical projectile",async({page,nativeGameplay},testInfo)=>{
  const propContact=process.env.PROFILE_DEMOMAN_PROP==="1"
  test.setTimeout(150_000)
  await page.addInitScript(()=>{(globalThis as any).__playsrcProfile={captureProjectileGameplay:true}})
  await page.goto("/")
  const root=page.locator("main")
  await page.waitForFunction(()=>["Startup","MainMenu","Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase??""))
  if(await root.getAttribute("data-phase")==="Startup") await page.keyboard.press("Escape")
  await expect(root).toHaveAttribute("data-phase","MainMenu",{timeout:30_000})
  const command=async(value:string)=>{
    if(await root.getAttribute("data-console-visible")!=="true") await page.keyboard.press("Backquote")
    const input=page.locator("[aria-label='Console command']");await expect(input).toBeVisible();await input.fill(value);await input.press("Enter")
    if(await root.getAttribute("data-console-visible")==="true") await page.keyboard.press("Backquote")
  }
  await command("map jump_beef")
  await page.waitForFunction(()=>{const root=document.querySelector<HTMLElement>("main");return root?.dataset.teamSelectionVisible==="true" || root?.dataset.phase==="Ready" || root?.dataset.phase==="Failed"},undefined,{timeout:90_000})
  if(await root.getAttribute("data-phase")==="Failed") throw new Error(await root.getAttribute("data-detail")??"map admission failed")
  if(await root.getAttribute("data-team-selection-visible")==="true") await chooseTf2Team(page,"red")
   await command("joinclass demoman")
   await expect(root).toHaveAttribute("data-phase","Ready",{timeout:30_000})
   await expect.poll(async()=>(await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("4")
  if(!propContact){
    await command("setpos 5328 3376 -3118")
    await expect.poll(async()=>Number((await page.evaluate(()=>(globalThis as any).__playsrcProfile.projectileState))?.position?.[0])).toBe(5328)
  }
  const local=await loadLocalConfig()
  await page.keyboard.press("Digit2")
  await expect.poll(async()=>((await root.getAttribute("data-hud-probe"))??"").split(":")[2]).toBe("3")
  const pipes=page.locator('[data-vgui-name="HudDemomanPipes"]')
  const present=pipes.locator('[data-vgui-name="PipesPresentPanel"]')
  const empty=pipes.locator('[data-vgui-name="NoPipesPresentPanel"]')
  const charge=page.locator('[data-vgui-name="HudDemomanCharge"]')
  await expect(empty.locator('[data-vgui-name="NumPipesLabel"]')).toHaveText("0")
  await expect(charge).toBeVisible()
  const canvas=page.locator("canvas.world-canvas")
   const pointer=await nativeGameplay.lockPointer()
  const selected=Number(await root.getAttribute("data-snapshot-tick"))
  await page.waitForFunction(tick=>Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick)>tick+36,selected)
   let mouseX=pointer.x
   let mouseY=pointer.y
  const aim=async(target:number)=>{const pitch=Number(await root.getAttribute("data-camera-pitch"));mouseY+=(target-pitch)/0.066;await page.mouse.move(mouseX,mouseY)}
  const directory=path.join(local.sourceCacheDir,"evidence","demoman-owned-physics",path.basename(process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!));await mkdir(directory,{recursive:true})
  await aim(0)
  const desiredYaw=propContact?0:180
  const yaw=Number(await root.getAttribute("data-camera-yaw"));mouseX+=(yaw-desiredYaw)/0.066;await page.mouse.move(mouseX,mouseY)
  await expect.poll(async()=>Math.abs(Number(await root.getAttribute("data-camera-yaw"))-desiredYaw)).toBeLessThan(1)
  await expect.poll(async()=>Math.abs(Number(await root.getAttribute("data-camera-pitch")))).toBeLessThan(1)
  await canvas.screenshot({path:path.join(directory,"horizontal-before.png")})
  await page.waitForFunction(()=>(globalThis as any).__playsrcProfile.audio?.stats().contextState==="running")
  const audioCapture=page.evaluate(async()=>{
    const audio=(globalThis as any).__playsrcProfile.audio
    const timeOrigin=performance.timeOrigin, started=performance.now(), before=audio.stats(), captured=await audio.capture(441000), after=audio.stats()
    const bytes=new Uint8Array(captured.pcm);let binary=""
    for(let at=0;at<bytes.length;at+=8192)binary+=String.fromCharCode(...bytes.subarray(at,at+8192))
    return {timeOrigin,started,before,after,captured:{sampleFormat:captured.sampleFormat,frames:captured.frames,sampleRate:captured.sampleRate,
      differingSamples:captured.differingSamples,uncoveredSamples:captured.uncoveredSamples,underruns:captured.underruns},pcm:btoa(binary)}
  })
  await page.evaluate(()=>{
    const profile=(globalThis as any).__playsrcProfile;profile.arcSamples=[];const end=performance.now()+3500
    const capture=()=>{const state=profile.projectileState;const previous=profile.arcSamples.at(-1);if(state&&previous?.tick!==state.tick)profile.arcSamples.push(structuredClone(state));if(performance.now()<end)requestAnimationFrame(capture)};requestAnimationFrame(capture)
  })
  const horizontalStart=Number(await root.getAttribute("data-snapshot-tick"))
  await page.mouse.down({button:"left"})
  await page.waitForFunction(tick=>Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick)>tick+5,horizontalStart)
  await page.waitForFunction(()=>(globalThis as any).__playsrcProfile.projectileState.chargeProgress>0)
  await nativeGameplay.capture("sticky-charging")
  await charge.screenshot({path:path.join(directory,"charge.png")})
  await page.mouse.up({button:"left"})
   await expect(present.locator('[data-vgui-name="NumPipesLabel"]')).toHaveText("1")
   await expect(present).toBeVisible()
   await page.waitForFunction(()=>(globalThis as any).__playsrcProfile.projectileEvents?.some((event:any)=>event.kind===2&&event.type==="fire"))
   const flightStart=await page.evaluate(()=>Number((globalThis as any).__playsrcProfile.projectileEvents.find((event:any)=>event.kind===2&&event.type==="fire").tick))
   for(const [label,offset] of [["rising",12],["crest",22],["falling",34],["landed",75]] as const){
      await page.waitForFunction(tick=>Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick)>=tick,flightStart+offset)
     await nativeGameplay.capture(`sticky-${label}`)
    await canvas.screenshot({path:path.join(directory,`horizontal-${label}.png`)})
  }
  const trajectory=await page.evaluate(()=>(globalThis as any).__playsrcProfile.arcSamples)
   if(propContact) await canvas.screenshot({path:path.join(directory,"locker-contact.png")})
  await page.mouse.down({button:"right"});await page.mouse.up({button:"right"})
  await page.waitForFunction(()=>(globalThis as any).__playsrcProfile?.projectileState?.projectileItems?.length===0,undefined,{timeout:5_000})
  await expect(empty.locator('[data-vgui-name="NumPipesLabel"]')).toHaveText("0")
  await expect(empty).toBeVisible()
  await aim(85)
  await expect.poll(async()=>Number(await root.getAttribute("data-camera-pitch"))).toBeGreaterThan(70)
  await canvas.screenshot({path:path.join(directory,"before.png")})
  const started=Date.now();const begin=Number(await root.getAttribute("data-snapshot-tick"))
  await page.mouse.down({button:"left"})
  await page.waitForFunction(tick=>Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick)>tick+5,begin)
  await page.mouse.up({button:"left"})
  await page.waitForFunction(()=>{const state=(globalThis as any).__playsrcProfile?.projectileState;return state?.projectileItems?.some((item:any)=>item.kind===2 && item.state!==1) || document.querySelector<HTMLElement>("main")?.dataset.phase==="Failed"},undefined,{timeout:8_000})
  await expect(root).toHaveAttribute("data-phase","Ready")
  const stuck=await page.evaluate(()=>(globalThis as any).__playsrcProfile.projectileState)
  expect(stuck.projectileItems.some((item:any)=>item.kind===2 && item.state!==1)).toBe(true)
  await canvas.screenshot({path:path.join(directory,"stuck.png")})
  await page.waitForFunction(tick=>Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick)>tick+60,begin)
  await page.keyboard.down("Space")
  await page.mouse.down({button:"right"});await page.mouse.up({button:"right"})
  await page.keyboard.up("Space")
  await page.waitForFunction(()=>(globalThis as any).__playsrcProfile?.projectileState?.projectileItems?.every((item:any)=>item.kind!==2),undefined,{timeout:5_000})
   const detonated=await page.evaluate(()=>(globalThis as any).__playsrcProfile.projectileState)
   expect(detonated.velocity[2]).toBeGreaterThan(0)
   await page.waitForFunction(z=>(globalThis as any).__playsrcProfile.projectileState.position[2]>z+16,stuck.position[2],{timeout:2000})
   await nativeGameplay.capture("sticky-blast-jump")
   await canvas.screenshot({path:path.join(directory,"blast-jump.png")})
   await expect(empty.locator('[data-vgui-name="NumPipesLabel"]')).toHaveText("0")
   await nativeGameplay.capture("sticky-detonated")
  await canvas.screenshot({path:path.join(directory,"detonated.png")})
  const remaining=5000-(Date.now()-started);if(remaining>0) await page.waitForFunction(time=>performance.now()>=time,await page.evaluate(delay=>performance.now()+delay,remaining))
  const end=Number(await root.getAttribute("data-snapshot-tick"));const seconds=(Date.now()-started)/1000
  const sound=await audioCapture
  const audioStarts=await root.getAttribute("data-audio-starts")??""
  const report={trajectory,stuck,detonated,seconds,ticks:end-begin,ticksPerSecond:(end-begin)/seconds,audioStarts,audio:{...sound,pcm:undefined},pointerLocked:await page.evaluate(()=>document.pointerLockElement?.matches("canvas.world-canvas")??false)}
    await profileArtifact(async()=>{
      const pcm=Buffer.from(sound.pcm,"base64");let nonzero=0
      for(let at=0;at<pcm.length;at+=4)if(pcm.readFloatLE(at)!==0)nonzero++
      const header=Buffer.alloc(44)
      header.write("RIFF");header.writeUInt32LE(pcm.length+36,4);header.write("WAVEfmt ",8);header.writeUInt32LE(16,16)
      header.writeUInt16LE(3,20);header.writeUInt16LE(2,22);header.writeUInt32LE(sound.captured.sampleRate,24)
      header.writeUInt32LE(sound.captured.sampleRate*8,28);header.writeUInt16LE(8,32);header.writeUInt16LE(32,34);header.write("data",36);header.writeUInt32LE(pcm.length,40)
      await writeFile(path.join(directory,"sticky-audio.wav"),Buffer.concat([header,pcm]))
     await writeFile(path.join(directory,"trajectory.json"),JSON.stringify(trajectory,null,2)+"\n")
     await writeFile(path.join(directory,"report.json"),JSON.stringify(report,null,2)+"\n")
      await testInfo.attach("demoman-owned-physics",{body:JSON.stringify(report),contentType:"application/json"})
      expect(audioStarts).toContain("Weapon_StickyBombLauncher.ChargeUp")
      expect(audioStarts).toContain("Weapon_StickyBombLauncher.Single")
      expect(audioStarts).toContain("Weapon_Grenade_Pipebomb.Explode")
      expect(sound.captured.differingSamples).toBe(0)
      expect(sound.captured.uncoveredSamples).toBe(0)
      expect(sound.captured.underruns).toBe(0)
      expect(nonzero).toBeGreaterThan(4410)
     const airborne=trajectory.flatMap((frame:any)=>frame.projectileItems.filter((item:any)=>item.kind===2&&item.state===1).map((item:any)=>({tick:frame.tick,...item})))
     if(propContact){
       const contact=trajectory.flatMap((frame:any)=>frame.projectileItems).find((item:any)=>item.kind===2&&item.state!==1)
       expect(contact).toBeDefined()
       // The configured locker is at [5512,3440,-2800], facing west.
       expect(contact.position[0]).toBeLessThan(5512)
       expect(contact.position[0]).toBeGreaterThan(5460)
       expect(contact.contactNormal[0]).toBeLessThan(-0.9)
     }else{
       expect(airborne.some((item:any)=>item.velocity[2]>0)).toBe(true)
       expect(airborne.some((item:any)=>item.velocity[2]<0)).toBe(true)
       expect(Math.max(...airborne.map((item:any)=>item.position[2]))).toBeGreaterThan(airborne[0].position[2])
     }
     expect(report.pointerLocked).toBe(true)
     expect(report.ticksPerSecond).toBeGreaterThan(60)
   })
})
