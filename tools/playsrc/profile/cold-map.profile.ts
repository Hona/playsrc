import { mkdir, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"

const MAX_THREADS = 64

test("profiles BSP-prefetched cold map loading", async ({ page }, testInfo) => {
  const requestedThreads = process.env.PROFILE_THREADS === undefined
    ? null
    : Number(process.env.PROFILE_THREADS)
  if (requestedThreads !== null
    && (!Number.isSafeInteger(requestedThreads) || requestedThreads < 1 || requestedThreads > MAX_THREADS)) {
    throw new Error(`PROFILE_THREADS must be an integer from 1 through ${MAX_THREADS}`)
  }
  if (requestedThreads !== null) {
    await page.addInitScript((threads) => {
      Object.defineProperty(navigator, "hardwareConcurrency", { configurable: true, value: threads })
    }, requestedThreads)
  }
  await page.addInitScript(() => {
    const records: Array<{ kind: string; timings?: Record<string, number> }> = []
    const NativeWorker = window.Worker
    class ProfiledWorker extends NativeWorker {
      constructor(url: string | URL, options?: WorkerOptions) {
        super(url, options)
        this.addEventListener("message", (event) => {
          if (event.data?.kind === "loaded" && event.data?.timings) records.push({ kind: "load", timings: event.data.timings })
        })
      }
    }
    Object.defineProperty(window, "Worker", { configurable: true, value: ProfiledWorker })
    ;(window as typeof window & { __playsrcColdWorkers: typeof records }).__playsrcColdWorkers = records
  })
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, {
    timeout: 180_000,
    polling: 50,
  })
  const main = page.locator("main")
  expect(await main.getAttribute("data-phase")).toBe("MainMenu")
  const configurationResponse = await page.request.get("/playsrc-config.json")
  expect(configurationResponse.status()).toBe(200)
  const configuration = await configurationResponse.json() as {
    assetOrigin: string
    defaultTarget: string
    targets: readonly { target: string; objects: { bsp: { sha256: string; byteLength: string } } }[]
  }
  const target = configuration.targets.find((candidate) => candidate.target === configuration.defaultTarget)
  if (!target) throw new Error("configured default map is absent from the current target catalog")
  const bspDescriptor = target.objects.bsp
  if (!/^[0-9a-f]{64}$/.test(bspDescriptor.sha256)
    || !/^[1-9]\d*$/.test(bspDescriptor.byteLength)) throw new Error("configured BSP descriptor is malformed")
  const bspUrl = `${configuration.assetOrigin}/objects/sha256/${bspDescriptor.sha256}`
  const bspResponse = await page.request.get(bspUrl)
  expect(bspResponse.status()).toBe(200)
  const bsp = await bspResponse.body()
  expect(bsp.byteLength).toBe(Number(bspDescriptor.byteLength))
  let fulfilledBspRequests = 0
  await page.route(bspUrl, async (route) => {
    fulfilledBspRequests += 1
    await route.fulfill({
      status: 200,
      body: bsp,
      headers: {
        "access-control-allow-origin": "*",
        "cache-control": "no-store",
        "content-length": String(bsp.byteLength),
        "content-type": "application/octet-stream",
        "etag": `"${bspDescriptor.sha256}"`,
      },
    })
  })
  const storageBefore = await page.evaluate(async () => ({
    estimate: await navigator.storage.estimate(),
    databases: typeof indexedDB.databases === "function" ? await indexedDB.databases() : [],
    threads: navigator.hardwareConcurrency,
  }))
  await page.keyboard.press("Backquote")
  const consoleEntry = page.locator("[aria-label='Console command']")
  await expect(consoleEntry).toBeVisible()
  await consoleEntry.fill(`map ${target.target}`)
  const started = await page.evaluate(() => performance.now())
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return (root?.dataset.phase === "Ready" && root.dataset.gameui === "in-game") || root?.dataset.phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 20 })
  const finished = await page.evaluate(() => performance.now())
  expect(await main.getAttribute("data-phase")).toBe("Ready")
  expect(fulfilledBspRequests).toBe(1)
  const loadPerformance = JSON.parse((await main.getAttribute("data-load-performance")) ?? "null")
  const workerRecords = await page.evaluate(() => (window as typeof window & { __playsrcColdWorkers: Array<{ kind: string; timings?: Record<string, number> }> }).__playsrcColdWorkers)
  const report = {
    schema: "playsrc-cold-map-profile-v1",
    target: target.target,
    browserThreads: storageBefore.threads,
    requestedThreads,
    bsp: {
      sha256: bspDescriptor.sha256,
      byteLength: bsp.byteLength,
      prefetchedBeforeMeasurement: true,
      networkRequestsInsideMeasurement: 0,
      fulfilledMemoryRequestsInsideMeasurement: fulfilledBspRequests,
    },
    derivedCacheBeforeMeasurement: {
      usage: storageBefore.estimate.usage ?? null,
      databases: storageBefore.databases.map((database) => ({ name: database.name ?? null, version: database.version ?? null })),
    },
    consoleToReadyMilliseconds: Number((finished - started).toFixed(3)),
    targetMilliseconds: 2_000,
    targetMet: finished - started <= 2_000,
    loadPerformance,
    workerLoad: workerRecords.find((record) => record.kind === "load")?.timings ?? null,
  }
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "cold-map", target.target)
  await rm(output, { recursive: true, force: true })
  await mkdir(output, { recursive: true })
  await writeFile(path.join(output, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  await testInfo.attach("cold-map-profile", { body: Buffer.from(JSON.stringify(report, null, 2)), contentType: "application/json" })
  console.log(`PLAYSRC_COLD_MAP ${JSON.stringify(report)}`)
})
