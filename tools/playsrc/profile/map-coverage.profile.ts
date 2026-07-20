import { spawn } from "node:child_process"
import { expect, test } from "@playwright/test"

const TARGET = "jump_beef"

test("profile whole-map noclip gameplay coverage", async ({ page, browser }, testInfo) => {
  const cdp = await browser.newBrowserCDPSession()
  const systemInfo = await cdp.send("SystemInfo.getInfo") as { gpu?: { devices?: unknown; featureStatus?: unknown; auxAttributes?: Record<string, unknown> } }
  const sampler = process.platform === "win32" ? spawn("typeperf", [
    "\\Process(msedge*)\\ID Process", "\\Process(msedge*)\\% Processor Time",
    "\\Process(msedge*)\\Working Set - Private", "\\Process(msedge*)\\Private Bytes",
    "\\GPU Engine(*)\\Utilization Percentage", "\\GPU Process Memory(*)\\Dedicated Usage",
    "\\GPU Process Memory(*)\\Shared Usage", "-si", "1",
  ], { stdio: ["ignore", "pipe", "pipe"] }) : null
  let counters = "", counterErrors = ""
  sampler?.stdout.setEncoding("utf8").on("data", (chunk) => { counters += chunk })
  sampler?.stderr.setEncoding("utf8").on("data", (chunk) => { counterErrors += chunk })
  const samplerExit = sampler ? new Promise<void>((resolve) => sampler.once("exit", () => resolve())) : Promise.resolve()

  await page.addInitScript(() => {
    let pointerLockElement: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => pointerLockElement })
    Object.defineProperty(Element.prototype, "requestPointerLock", { configurable: true, value(this: Element) { pointerLockElement = this; queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange"))); return Promise.resolve() } })
    Object.defineProperty(document, "exitPointerLock", { configurable: true, value() { pointerLockElement = null; queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange"))); return Promise.resolve() } })
    const state: { coverageSamples?: readonly unknown[]; longTasks: { at: number; duration: number }[] } = { longTasks: [] }
    ;(window as any).__playsrcProfile = state
    new PerformanceObserver((list) => { for (const entry of list.getEntries()) state.longTasks.push({ at: entry.startTime, duration: entry.duration }) }).observe({ entryTypes: ["longtask"] })
  })

  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 180_000, polling: 50 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await expect(entry).toBeVisible()
  await entry.fill(`map ${TARGET}`)
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => { const main = document.querySelector<HTMLElement>("main"); return (main?.dataset.phase === "Ready" && main.dataset.gameui === "in-game") || main?.dataset.phase === "Failed" }, undefined, { timeout: 600_000, polling: 50 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
  await entry.fill("noclip")
  await page.keyboard.press("Enter")
  await page.keyboard.press("Backquote")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.movementMode === "1", undefined, { timeout: 30_000, polling: 20 })
  await page.locator("canvas.world-canvas").click()
  await page.waitForFunction(() => document.pointerLockElement?.classList.contains("world-canvas"), undefined, { timeout: 5_000 })

  const result = await page.evaluate(async () => {
    type Sample = { leaf: number; cluster: number; area: number; position: readonly [number, number, number] }
    const state = (window as any).__playsrcProfile as { coverageSamples: readonly Sample[]; longTasks: { at: number; duration: number }[] }
    const main = document.querySelector("main") as HTMLElement
    const canvas = document.querySelector("canvas.world-canvas") as HTMLCanvasElement
    if (!state.coverageSamples?.length) throw new Error("coverage goals unavailable")
    const position = () => (main.dataset.cameraPosition ?? "").split(",").map(Number) as [number, number, number]
    const distance = (left: readonly number[], right: readonly number[]) => Math.hypot(left[0]! - right[0]!, left[1]! - right[1]!, left[2]! - right[2]!)
    const key = (kind: "keydown" | "keyup", code: string, value: string, shiftKey = false) => dispatchEvent(new KeyboardEvent(kind, { code, key: value, shiftKey, bubbles: true }))
    const turn = (target: readonly number[]) => {
      const current = position(), dx = target[0]! - current[0], dy = target[1]! - current[1], dz = target[2]! - current[2], horizontal = Math.hypot(dx, dy)
      const desiredYaw = Math.atan2(dy, dx) * 180 / Math.PI, desiredPitch = -Math.atan2(dz, horizontal) * 180 / Math.PI
      const currentYaw = Number(main.dataset.cameraYaw ?? 0), currentPitch = Number(main.dataset.cameraPitch ?? 0)
      const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(event, { movementX: { value: wrap(currentYaw - desiredYaw) / 0.066 }, movementY: { value: (desiredPitch - currentPitch) / 0.066 } })
      dispatchEvent(event)
    }
    const pending = new Set(state.coverageSamples.map((_, index) => index)), route: number[] = []
    const frameRecords: Record<string, unknown>[] = [], reached: number[] = [], unreachable: Record<string, unknown>[] = []
    let activeGoal = -1
    const observer = new MutationObserver(() => { const detail = main.dataset.performanceDetail; if (detail) frameRecords.push({ at: performance.now(), goal: activeGoal, camera: position(), frame: Number(canvas.dataset.displayFrame ?? 0), detail: JSON.parse(detail) }) })
    observer.observe(main, { attributes: true, attributeFilter: ["data-performance-detail"] })
    const runStarted=performance.now(),runDeadline=runStarted+300_000
    let lastTick=main.dataset.snapshotTick,lastFrame=canvas.dataset.displayFrame,lastTickAt=performance.now(),lastFrameAt=performance.now()
    key("keydown", "KeyW", "w")
    try{
      for (let ordinal = 0; pending.size; ordinal += 1) {
        if(performance.now()>runDeadline)throw new Error(`coverage run exceeded 300000 ms after ${route.length}/${state.coverageSamples.length} goals`)
        let index=-1,best=Number.POSITIVE_INFINITY;const current=position();for(const candidate of pending){const value=distance(current,state.coverageSamples[candidate]!.position);if(value<best){best=value;index=candidate}}pending.delete(index);route.push(index)
        const sample = state.coverageSamples[index]!, started = performance.now()
        activeGoal = index
        const strafe = ordinal % 4 === 1, crouch = ordinal % 3 === 1, fire = ordinal % 16 === 0
        if (strafe) key("keydown", "KeyA", "a")
        if (crouch) key("keydown", "ShiftLeft", "Shift", true)
        if (fire){dispatchEvent(new MouseEvent("mousedown",{button:0,bubbles:true}));setTimeout(()=>dispatchEvent(new MouseEvent("mouseup",{button:0,bubbles:true})),35)}
        let lastTurn = 0, arrived = false,lastDistance=distance(position(),sample.position),lastProgressAt=performance.now()
        while (performance.now() - started < 1_500) {
          await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
          const now=performance.now(),phase=main.dataset.phase,tick=main.dataset.snapshotTick,frame=canvas.dataset.displayFrame
          if(phase!=="Ready")throw new Error(`coverage authority entered ${phase}: ${main.dataset.detail}; goal=${index}; tick=${tick}; camera=${main.dataset.cameraPosition}`)
          if(tick!==lastTick){lastTick=tick;lastTickAt=now}else if(now-lastTickAt>1000)throw new Error(`coverage Simulation stalled at tick ${tick}, goal ${index}, camera ${main.dataset.cameraPosition}`)
          if(frame!==lastFrame){lastFrame=frame;lastFrameAt=now}else if(now-lastFrameAt>1000)throw new Error(`coverage display stalled at frame ${frame}, goal ${index}, camera ${main.dataset.cameraPosition}`)
          const remaining=distance(position(),sample.position);if(remaining<96){arrived=true;break}if(remaining<lastDistance-4){lastDistance=remaining;lastProgressAt=now}else if(now-lastProgressAt>750)break
          if(now-lastTurn>80){turn(sample.position);lastTurn=now}
        }
        dispatchEvent(new MouseEvent("mouseup",{button:0,bubbles:true}))
        if (strafe) key("keyup", "KeyA", "a")
        if (crouch) key("keyup", "ShiftLeft", "Shift")
        if (arrived)reached.push(index)
        else unreachable.push({index,leaf:sample.leaf,target:sample.position,current:position(),elapsed:performance.now()-started})
      }
    }finally{
      key("keyup", "KeyW", "w");key("keyup", "KeyA", "a");key("keyup", "ShiftLeft", "Shift");dispatchEvent(new MouseEvent("mouseup",{button:0,bubbles:true}));observer.disconnect()
    }
    return { samples: state.coverageSamples, route, reached, unreachable, frameRecords, longTasks: state.longTasks }
  })

  if (sampler) { sampler.kill(); await samplerExit }
  await cdp.detach()
  const durations = result.frameRecords.map((record) => Number((record.detail as Record<string, unknown>).total ?? 0)).sort((a, b) => a - b)
  const percentile = (fraction: number) => durations[Math.min(durations.length - 1, Math.floor(durations.length * fraction))] ?? 0
  const worst = result.frameRecords.toSorted((left, right) => Number((right.detail as Record<string, unknown>).total ?? 0) - Number((left.detail as Record<string, unknown>).total ?? 0)).slice(0, 100)
  const report = {
    target: TARGET, samples: result.samples.length, reached: result.reached.length, unreachable: result.unreachable.length,
    frames: result.frameRecords.length, p50Milliseconds: percentile(.5), p95Milliseconds: percentile(.95),
    p99Milliseconds: percentile(.99), maximumMilliseconds: durations.at(-1) ?? 0, worst,
    unreachableSamples: result.unreachable, longTasks: result.longTasks,
    gpu: { devices: systemInfo.gpu?.devices ?? [], featureStatus: systemInfo.gpu?.featureStatus ?? {}, renderer: systemInfo.gpu?.auxAttributes?.glRenderer ?? null },
    counterErrors: counterErrors.trim() || null,
  }
  await testInfo.attach("map-coverage", { body: Buffer.from(JSON.stringify({ ...report, route: result.route, records: result.frameRecords }, null, 2)), contentType: "application/json" })
  await testInfo.attach("map-coverage-typeperf", { body: Buffer.from(counters), contentType: "text/csv" })
  console.log(`PLAYSRCMAPCOVERAGE ${JSON.stringify(report)}`)
  expect(result.frameRecords.length).toBeGreaterThan(0)
  expect(result.reached.length).toBeGreaterThan(0)
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
})
