import type { LocalConfig } from "./config"
import { startDevelopment } from "./dev"

const MAX_OUTPUT_BYTES = 1024 * 1024

export class BrowserEvidenceError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "BrowserEvidenceError"
  }
}

async function agent(args: string[]): Promise<string> {
  const child = Bun.spawn(["agent-browser", ...args], {
    stdout: "pipe",
    stderr: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).arrayBuffer(),
    new Response(child.stderr).arrayBuffer(),
    child.exited,
  ])
  if (stdout.byteLength > MAX_OUTPUT_BYTES || stderr.byteLength > MAX_OUTPUT_BYTES) {
    throw new BrowserEvidenceError("agent-browser output exceeded 1048576 bytes")
  }
  const output = new TextDecoder().decode(stdout).trim()
  if (exitCode !== 0) {
    const error = new TextDecoder().decode(stderr).trim()
    throw new BrowserEvidenceError(`agent-browser ${args.at(-1)} failed: ${error || output}`)
  }
  return output
}

function parseJson<T>(value: string): T {
  try {
    return JSON.parse(value) as T
  } catch {
    throw new BrowserEvidenceError("agent-browser evaluation did not return JSON")
  }
}

function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new BrowserEvidenceError(message)
}

async function acquirePointerLock(session: string): Promise<void> {
  let lastBody = ""
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const tabs = await agent(["--session", session, "tab"])
    const tab = /\[(t\d+)\]/.exec(tabs)?.[1]
    if (tab) await agent(["--session", session, "tab", tab])
    await agent(["--session", session, "focus", ".world-canvas"])
    await agent(["--session", session, "click", ".world-canvas"])
    await agent(["--session", session, "wait", "1000"])
    lastBody = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    if (lastBody.includes("MOUSE CAPTURED")) return
    await agent(["--session", session, "press", "Escape"]).catch(() => {})
    await agent(["--session", session, "wait", "1000"])
  }
  throw new BrowserEvidenceError(`desktop pointer lock was not acquired after three user activations: ${lastBody.slice(0, 300)}`)
}

async function unavailable(url: string): Promise<boolean> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 1000)
  try {
    await fetch(url, { cache: "no-store", signal: controller.signal })
    return false
  } catch {
    return true
  } finally {
    clearTimeout(timeout)
  }
}

export async function verifyBrowserAcceptance(
  config: LocalConfig,
  target: string | undefined,
): Promise<Record<string, unknown>> {
  const version = await agent(["--version"])
  const session = `playsrc-acceptance-${process.pid}`
  const owner = await startDevelopment(config, target)
  let browserOpen = false
  try {
    await agent(["--session", session, "--headed", "--webgpu", "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "wait", "--text", "Ready", "--timeout", "120000"])
    let body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("DERIVED CACHE STORED"), "cold browser run did not store the derived payload")

    await acquirePointerLock(session)
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("MOUSE CAPTURED"), "desktop pointer lock was not acquired")
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('[role=dialog]')).display !== 'none'",
    ])
    require(
      parseJson<string>(await agent(["--session", session, "eval", "document.pointerLockElement?.className ?? ''"])) === "",
      "console activation did not release pointer lock",
    )
    await agent(["--session", session, "fill", "[aria-label='Console command']", "status"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--text", "generation 1", "--timeout", "120000"])
    await agent(["--session", session, "press", "ArrowUp"])
    require(
      await agent(["--session", session, "get", "value", "[aria-label='Console command']"]) === "status",
      "console history did not restore the submitted command",
    )
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map j"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "document.querySelector('[role=listbox]')?.textContent === 'map jump_beef'",
    ])
    await agent(["--session", session, "fill", "[aria-label='Console command']", "map jump_beef"])
    await agent(["--session", session, "press", "Enter"])
    await agent(["--session", session, "wait", "--text", "Loaded jump_beef; generation 2", "--timeout", "120000"])
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('[role=dialog]')).display === 'none'",
    ])

    await agent([
      "--session",
      session,
      "eval",
      "window.dispatchEvent(new KeyboardEvent('keydown',{code:'KeyW',key:'w',bubbles:true})); true",
    ])
    await agent(["--session", session, "wait", "500"])
    await agent([
      "--session",
      session,
      "eval",
      "window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW',key:'w',bubbles:true})); true",
    ])
    const movingSpeed = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number(document.querySelector('.speed-readout strong').textContent)",
    ]))
    require(movingSpeed > 0, "movement binding did not advance the player")
    await agent(["--session", session, "press", "Space"])
    await agent(["--session", session, "wait", "100"])
    const jumpSpeed = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number(document.querySelector('.speed-readout strong').textContent)",
    ]))
    require(jumpSpeed > 0, "jump binding did not advance the player")

    const initialBlockerCount = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
    ]))

    await acquirePointerLock(session)
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "left"])
    await agent(["--session", session, "wait", "1200"])
    let blockerCount = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
    ]))
    require(blockerCount >= initialBlockerCount + 2, "Soldier fire event was not observed")
    const soldierBlockerCount = blockerCount

    await agent(["--session", session, "press", "Escape"])
    await agent(["--session", session, "wait", "1000"])
    await agent(["--session", session, "click", ".class-rail button:nth-child(2)"])
    await agent(["--session", session, "wait", "--text", "STICKYBOMB LAUNCHER", "--timeout", "120000"])
    await acquirePointerLock(session)
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "left"])
    await agent(["--session", session, "wait", "1000"])
    await agent(["--session", session, "mouse", "down", "right"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "right"])
    await agent(["--session", session, "wait", "500"])
    blockerCount = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
    ]))
    require(blockerCount >= soldierBlockerCount + 1, "Demoman sticky fire/detonation events were not observed")

    await agent(["--session", session, "reload"])
    await agent(["--session", session, "wait", "--text", "Ready", "--timeout", "120000"])
    body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("DERIVED CACHE HIT"), "warm browser run did not reuse the derived payload")
    const records = parseJson<Array<{ key: string; byteLength: number; sha256: string }>>(await agent([
      "--session",
      session,
      "eval",
      "new Promise((resolve,reject)=>{const r=indexedDB.open('playsrc-derived-v1',1);r.onerror=()=>reject(r.error);r.onsuccess=()=>{const q=r.result.transaction('objects').objectStore('objects').getAll();q.onerror=()=>reject(q.error);q.onsuccess=()=>resolve(q.result.map(x=>({key:x.key,byteLength:x.byteLength,sha256:x.sha256})))}})",
    ]))
    require(
      records.length === 1
      && records[0]?.key === "eeb42b44f31ef02867844ad71adb4e64917bad493d87997a207e9ccb9e1376e6"
      && records[0]?.byteLength === 38_700_243
      && records[0]?.sha256 === "2c8a6864cb485dc5512649b6543cf2b06623500dce7c7f45bf6302d9fce53d1c",
      "warm IndexedDB record identity differs",
    )
    return {
      target: "jump_beef",
      browser: version,
      coldCache: "stored",
      warmCache: "hit",
      derived: records[0],
      mapReplacementGeneration: 2,
      movingSpeed,
      jumpSpeed,
      supportBlockers: blockerCount,
      supportStatus: "diagnostic-blockers-retained",
      pointerLock: "acquired-and-released-for-console",
      console: "history-completion-replacement-close-passed",
      shutdown: "pending",
    }
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    await owner.close()
  }
}

export async function runBrowserAcceptance(
  config: LocalConfig,
  target: string | undefined,
): Promise<void> {
  const report = await verifyBrowserAcceptance(config, target)
  require(
    await unavailable("http://127.0.0.1:4173/readyz")
    && await unavailable("http://127.0.0.1:4174/readyz"),
    "owned listeners remained available after shutdown",
  )
  console.log(JSON.stringify({ ...report, shutdown: "listeners-released" }))
}
