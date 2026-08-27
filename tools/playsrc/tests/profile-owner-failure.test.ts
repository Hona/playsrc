import { expect, test } from "bun:test"
import { createHash } from "node:crypto"
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { acquireHeadedProfileLock, releaseHeadedProfileLock, processIsAlive } from "../src/profile-lock"
import { stopOwner } from "../src/profile-runner"

async function exitWithin(child: { exited: Promise<number> }, milliseconds: number): Promise<number> {
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([child.exited, new Promise<never>((_resolve, reject) => {
      timer = setTimeout(() => reject(new Error("fixture process exceeded its exit deadline")), milliseconds)
    })])
  } finally { clearTimeout(timer) }
}

// Real child/listeners and checked FIFO admission; no browser, build, or configured assets.
for (const failure of ["import", "configuration"]) test(`prelaunch ${failure} failure retires a stuck leased service before FIFO handoff`, async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-owner-failure-"))
  const evidence = path.join(directory, "evidence/tf2-browser-performance")
  await mkdir(evidence, { recursive: true })
  const metadataPath = path.join(evidence, "development-owner.json")
  const lockPath = path.join(evidence, "chromium-profile.lock")
  const identity = createHash("sha256").update("source").update("configured").update("4173").digest("hex")
  const service = Bun.spawn([process.execPath, "-e", `
    import { writeFile } from "node:fs/promises";
    const metadata = { schema: "playsrc-profile-owner-v1", token: "service", identity: ${JSON.stringify(identity)}, target: "jump_beef", repository: ${JSON.stringify(directory)}, pid: process.pid, startup: {}, generatedIdentity: "generated" };
    process.on("SIGTERM", () => {});
    const assets = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response("assets") });
    const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => Response.json(metadata) });
    metadata.url = server.url.toString();
    await writeFile(${JSON.stringify(metadataPath)}, JSON.stringify(metadata));
    await writeFile(${JSON.stringify(`${metadataPath}.lease`)}, JSON.stringify({schema: "playsrc-profile-owner-lease-v1", token: "service", expiresAt: Date.now() + 60000}));
    console.log(JSON.stringify([metadata.url, assets.url.toString()]));
  `], { stdout: "pipe", stderr: "pipe" })
  let runner: ReturnType<typeof Bun.spawn> | undefined
  let next: Awaited<ReturnType<typeof acquireHeadedProfileLock>> | undefined
  try {
    const reader = service.stdout.getReader()
    const urls = JSON.parse(new TextDecoder().decode((await reader.read()).value)) as string[]
    reader.releaseLock()
    await writeFile(path.join(directory, "playwright.profile.config.ts"), failure === "import"
      ? 'import "./missing-controller-module.ts"; export default {};'
      : 'export default { get use() { throw new Error("injected configuration failure") } };')
    // Keep module mocks confined to this subprocess, including the temporary repository.
    const script = `
      import { mock } from "bun:test";
      mock.module(${JSON.stringify(import.meta.resolve("../src/config"))}, () => ({ repositoryRoot: ${JSON.stringify(directory)}, loadLocalConfig: async () => ({ sourceCacheDir: ${JSON.stringify(directory)} }) }));
      mock.module(${JSON.stringify(import.meta.resolve("../src/build-identity"))}, () => ({ applicationBuildIdentity: async () => "source" }));
      mock.module(${JSON.stringify(import.meta.resolve("../src/profile-identity"))}, () => ({ configuredProfileIdentity: async () => "configured", generatedProfileIdentity: async () => "generated" }));
      mock.module(${JSON.stringify(import.meta.resolve("../src/profile-browser"))}, () => ({ browserLease: async () => {}, profileNodeExecutable: () => "node", prepareProfileBrowser: async () => { throw new Error("BROWSER MUST NOT LAUNCH") } }));
      const { runHeadedProfile } = await import(${JSON.stringify(import.meta.resolve("../src/profile-runner"))});
      process.exitCode = await runHeadedProfile(["gameplay"]);
    `
    const holder = await acquireHeadedProfileLock(lockPath, "fixture-admission")
    runner = Bun.spawn([process.execPath, "-e", script], {
      env: { ...Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("PLAYSRC_PROFILE_") && !key.startsWith("PROFILE_"))), PLAYSRC_DEV_PORT: "4173" },
      stdout: "pipe", stderr: "pipe",
    })
    const queuedDeadline = Date.now() + 2_000
    while ((await readdir(`${lockPath}.queue`)).filter(name => name.endsWith(".json")).length === 0 && Date.now() < queuedDeadline) await Bun.sleep(10)
    expect((await readdir(`${lockPath}.queue`)).filter(name => name.endsWith(".json"))).toHaveLength(1)
    let serviceAliveAtHandoff: boolean | undefined
    const waiting = acquireHeadedProfileLock(lockPath, "next-checked-run", 8_000).then(lock => {
      serviceAliveAtHandoff = processIsAlive(service.pid)
      return lock
    })
    const began = Date.now()
    await releaseHeadedProfileLock(lockPath, holder.token)
    const errors = new Response(runner.stderr).text()
    expect(await exitWithin(runner, 6_000)).toBe(1)
    next = await waiting
    expect(serviceAliveAtHandoff).toBe(false)
    expect(Date.now() - began).toBeLessThan(6_000)
    expect(await errors).not.toContain("BROWSER MUST NOT LAUNCH")
    expect(service.signalCode).toBe("SIGKILL")
    for (const url of urls) await expect(fetch(url, { signal: AbortSignal.timeout(200) })).rejects.toThrow()
    await expect(readFile(metadataPath)).rejects.toMatchObject({ code: "ENOENT" })
    await expect(readFile(`${metadataPath}.lease`)).rejects.toMatchObject({ code: "ENOENT" })
    const [run] = await readdir(path.join(evidence, "runs"))
    const reportPath = path.join(evidence, "runs", run!, "command.json")
    const bytes = await readFile(reportPath, "utf8")
    const report = JSON.parse(bytes)
    expect(report).toMatchObject({ outcome: "failed", exitCode: 1, timedOut: false, cleanupFailure: null, phases: { headedBrowserMilliseconds: 0, ownerReused: true } })
    expect(report.failure).toContain(failure === "import" ? "missing-controller-module" : "injected configuration failure")
    expect(report.attempts).toContainEqual(expect.objectContaining({ phase: "controller-preflight", complete: false }))
    expect(report.phases.cleanupMilliseconds).toBeGreaterThan(0)
    expect(report.elapsedMilliseconds).toBe(Date.parse(report.finishedAt) - Date.parse(report.startedAt))
    expect(report.elapsedMilliseconds).toBeGreaterThanOrEqual(report.phases.cleanupMilliseconds)
    await releaseHeadedProfileLock(lockPath, next.token)
    next = undefined
    expect(await readFile(reportPath, "utf8")).toBe(bytes)
  } finally {
    if (next) await releaseHeadedProfileLock(lockPath, next.token)
    runner?.kill("SIGKILL")
    if (runner) await runner.exited
    service.kill("SIGKILL")
    await service.exited
    await rm(directory, { recursive: true, force: true })
  }
}, 12_000)

test("the leased owner exits boundedly when Vite close stalls, releasing both listeners and its pipe-owned child", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-owner-close-"))
  const filename = path.join(directory, "owner.json")
  await writeFile(`${filename}.lease`, JSON.stringify({ schema: "playsrc-profile-owner-lease-v1", token: "close-test", expiresAt: Date.now() + 60_000 }))
  const script = `
    import { mock } from "bun:test";
    import { createServer } from ${JSON.stringify(import.meta.resolve("vite"))};
    const { closeDevelopmentListeners } = await import(${JSON.stringify(import.meta.resolve("../src/dev"))});
    mock.module(${JSON.stringify(import.meta.resolve("../src/config"))}, () => ({ repositoryRoot: ${JSON.stringify(directory)}, loadLocalConfig: async () => ({}) }));
    mock.module(${JSON.stringify(import.meta.resolve("../src/profile-identity"))}, () => ({ generatedProfileIdentity: async () => "generated" }));
    mock.module(${JSON.stringify(import.meta.resolve("../src/dev"))}, () => ({ startDevelopment: async () => {
      const child = Bun.spawn([process.execPath, "-e", 'await new Response(Bun.stdin.stream()).text()'], { stdin: "pipe", stdout: "ignore", stderr: "ignore" });
      const assets = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response("assets") });
      const application = await createServer({ configFile: false, root: ${JSON.stringify(directory)}, server: { host: "127.0.0.1", port: 0 }, plugins: [{ name: "stuck-close", closeBundle: () => new Promise(() => {}) }] });
      await application.listen();
      const port = application.httpServer.address().port;
      const url = "http://127.0.0.1:" + port;
      console.log(JSON.stringify({ urls: [url, assets.url.toString()], child: child.pid }));
      return { url, startup: {}, close: () => closeDevelopmentListeners(application, assets) };
    } }));
    process.argv[2] = "jump_beef";
    await import(${JSON.stringify(import.meta.resolve("../src/profile-owner"))});
  `
  const owner = Bun.spawn([process.execPath, "-e", script], { env: { ...process.env, PLAYSRC_PROFILE_OWNER_TOKEN: "close-test", PLAYSRC_PROFILE_SOURCE_IDENTITY: "source", PLAYSRC_PROFILE_OWNER_PATH: filename }, stdout: "pipe", stderr: "pipe" })
  try {
    const reader = owner.stdout.getReader()
    const { urls, child } = JSON.parse(new TextDecoder().decode((await reader.read()).value)) as { urls: string[]; child: number }
    reader.releaseLock()
    const readyDeadline = Date.now() + 2_000
    while (!await Bun.file(filename).exists() && Date.now() < readyDeadline) await Bun.sleep(10)
    expect(JSON.parse(await readFile(filename, "utf8")).pid).toBe(owner.pid)
    expect(processIsAlive(child)).toBe(true)
    const began = Date.now()
    await writeFile(`${filename}.lease`, JSON.stringify({ schema: "playsrc-profile-owner-lease-v1", token: "close-test", expiresAt: 0 }))
    let assetsClosed = false
    while (!assetsClosed && Date.now() - began < 1_500) {
      try { await (await fetch(urls[1]!, { signal: AbortSignal.timeout(100) })).text() }
      catch { assetsClosed = true }
      if (!assetsClosed) await Bun.sleep(25)
    }
    expect(assetsClosed).toBe(true)
    expect(processIsAlive(owner.pid)).toBe(true) // Vite is still stuck; asset cleanup is independent.
    expect(await exitWithin(owner, 3_500)).toBe(1)
    expect(Date.now() - began).toBeLessThan(3_000)
    expect(await new Response(owner.stderr).text()).toContain("2000 ms cleanup budget")
    const childDeadline = Date.now() + 500
    while (processIsAlive(child) && Date.now() < childDeadline) await Bun.sleep(10)
    expect(processIsAlive(child)).toBe(false)
    for (const url of urls) {
      await expect(fetch(url, { signal: AbortSignal.timeout(200) })).rejects.toThrow()
      const rebound = Bun.serve({ hostname: "127.0.0.1", port: Number(new URL(url).port), fetch: () => new Response("successor") })
      rebound.stop(true)
    }
  } finally { owner.kill("SIGKILL"); await owner.exited; await rm(directory, { recursive: true, force: true }) }
}, 8_000)

test("retirement rejects changed ownership and live lock holders, and never signals an endpoint with unproven identity", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-owner-proof-"))
  const filename = path.join(directory, "owner.json")
  const lockPath = path.join(directory, "chromium-profile.lock")
  const child = Bun.spawn([process.execPath, "-e", 'console.log("ready");setInterval(()=>{},1000)'], { stdout: "pipe", stderr: "pipe" })
  const metadata = { schema: "playsrc-profile-owner-v1" as const, pid: child.pid, token: "proof", identity: "source", target: "jump_beef", repository: directory, url: "", startup: {} }
  let endpoint: object = metadata
  const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => Response.json(endpoint) })
  metadata.url = server.url.toString()
  const lock = await acquireHeadedProfileLock(lockPath, "proof-test")
  try {
    const reader = child.stdout.getReader()
    await reader.read()
    reader.releaseLock()
    await writeFile(filename, JSON.stringify(metadata))
    const lease = JSON.stringify({ schema: "playsrc-profile-owner-lease-v1", token: metadata.token, expiresAt: Date.now() + 60_000 })
    await writeFile(`${filename}.lease`, lease)
    const held = await readFile(lockPath, "utf8")
    await writeFile(lockPath, JSON.stringify({ token: "live-holder", pid: child.pid }))
    await expect(stopOwner(filename, metadata, 10)).rejects.toThrow("never signal its holder")
    expect(await readFile(`${filename}.lease`, "utf8")).toBe(lease)
    await writeFile(lockPath, held)
    await writeFile(filename, JSON.stringify({ ...metadata, identity: "replacement" }))
    await expect(stopOwner(filename, metadata, 10)).rejects.toThrow("changed during retirement")
    await writeFile(filename, JSON.stringify(metadata))
    await writeFile(`${filename}.lease`, JSON.stringify({ token: "replacement" }))
    await expect(stopOwner(filename, metadata, 10)).rejects.toThrow("lease changed")
    await writeFile(`${filename}.lease`, lease)
    for (const field of ["pid", "token", "identity", "repository", "target"]) {
      endpoint = { ...metadata, [field]: field === "pid" ? process.pid : "foreign" }
      await expect(stopOwner(filename, metadata, 1_100)).rejects.toThrow("remained live")
      expect(processIsAlive(child.pid)).toBe(true)
      expect(await readFile(lockPath, "utf8")).toBe(held)
    }
    // An endpoint that no longer answers cannot authorize escalation either.
    server.stop(true)
    await expect(stopOwner(filename, metadata, 1_100)).rejects.toThrow("remained live")
    expect(processIsAlive(child.pid)).toBe(true)
  } finally {
    await releaseHeadedProfileLock(lockPath, lock.token)
    server.stop(true); child.kill("SIGKILL"); await child.exited
    await rm(directory, { recursive: true, force: true })
  }
}, 10_000)
