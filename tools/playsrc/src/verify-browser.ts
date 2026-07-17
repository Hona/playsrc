import { repositoryRoot } from "./config"

const MAX_OUTPUT_BYTES = 1024 * 1024
const PROCESS_READY_TIMEOUT_MS = 180_000
const PROCESS_EXIT_TIMEOUT_MS = 30_000
const APPLICATION_URL = "http://127.0.0.1:4173/"

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

type DevelopmentProcessOwner = Readonly<{
  url: string
  interrupt(): Promise<void>
}>

async function consumeOutput(
  stream: ReadableStream<Uint8Array>,
  append: (text: string, bytes: number) => void,
): Promise<void> {
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  while (true) {
    const result = await reader.read()
    if (result.done) break
    append(decoder.decode(result.value, { stream: true }), result.value.byteLength)
  }
  append(decoder.decode(), 0)
}

function excerpt(value: string): string {
  return value.trim().replaceAll(/\s+/gu, " ").slice(0, 500)
}

async function startDevelopmentProcess(target: string | undefined): Promise<DevelopmentProcessOwner> {
  const command = [process.execPath, "run", "dev"]
  if (target !== undefined) command.push(target)
  const child = Bun.spawn(command, {
    cwd: repositoryRoot,
    stdout: "pipe",
    stderr: "pipe",
  })
  let stdout = ""
  let stderr = ""
  let outputBytes = 0
  let ready = false
  let settled = false
  let resolveReady: (() => void) | undefined
  let rejectReady: ((error: Error) => void) | undefined
  const readiness = new Promise<void>((resolve, reject) => {
    resolveReady = resolve
    rejectReady = reject
  })
  const append = (channel: "stdout" | "stderr") => (text: string, bytes: number): void => {
    outputBytes += bytes
    if (outputBytes > MAX_OUTPUT_BYTES) {
      child.kill("SIGKILL")
      rejectReady?.(new BrowserEvidenceError("development command output exceeded 1048576 bytes"))
      return
    }
    if (channel === "stdout") {
      stdout += text
      if (!ready && stdout.split(/\r?\n/u).includes(APPLICATION_URL)) {
        ready = true
        resolveReady?.()
      }
    } else {
      stderr += text
    }
  }
  const stdoutTask = consumeOutput(child.stdout, append("stdout"))
  const stderrTask = consumeOutput(child.stderr, append("stderr"))
  void stdoutTask.catch((error) => rejectReady?.(error instanceof Error ? error : new Error(String(error))))
  void stderrTask.catch((error) => rejectReady?.(error instanceof Error ? error : new Error(String(error))))
  const exited = child.exited.then((code) => {
    settled = true
    return code
  })
  const prematureExit = exited.then(async (code) => {
    if (ready) return new Promise<never>(() => {})
    await Promise.allSettled([stdoutTask, stderrTask])
    throw new BrowserEvidenceError(
      `development command exited before readiness with code ${code}: ${excerpt(stderr || stdout)}`,
    )
  })
  let readyTimeout: ReturnType<typeof setTimeout> | undefined
  try {
    await Promise.race([
      readiness,
      prematureExit,
      new Promise<never>((_, reject) => {
        readyTimeout = setTimeout(() => {
          reject(new BrowserEvidenceError("development command did not report readiness within 180000 ms"))
        }, PROCESS_READY_TIMEOUT_MS)
      }),
    ])
  } catch (error) {
    if (!settled) child.kill("SIGKILL")
    await exited.catch(() => {})
    await Promise.allSettled([stdoutTask, stderrTask])
    throw error
  } finally {
    if (readyTimeout !== undefined) clearTimeout(readyTimeout)
  }
  let interrupted = false
  return Object.freeze({
    url: APPLICATION_URL,
    async interrupt(): Promise<void> {
      if (interrupted) return
      interrupted = true
      if (settled) {
        await Promise.allSettled([stdoutTask, stderrTask])
        throw new BrowserEvidenceError("development command exited before the acceptance interrupt")
      }
      child.kill("SIGINT")
      let exitTimeout: ReturnType<typeof setTimeout> | undefined
      let code: number
      try {
        code = await Promise.race([
          exited,
          new Promise<never>((_, reject) => {
            exitTimeout = setTimeout(() => {
              reject(new BrowserEvidenceError("development command did not exit within 30000 ms after SIGINT"))
            }, PROCESS_EXIT_TIMEOUT_MS)
          }),
        ])
      } catch (error) {
        if (!settled) child.kill("SIGKILL")
        await exited.catch(() => {})
        throw error
      } finally {
        if (exitTimeout !== undefined) clearTimeout(exitTimeout)
        await Promise.allSettled([stdoutTask, stderrTask])
      }
      require(code === 0, `development command exited with code ${code} after SIGINT: ${excerpt(stderr || stdout)}`)
    },
  })
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

export async function verifyBrowserAcceptance(target: string | undefined): Promise<Record<string, unknown>> {
  const version = await agent(["--version"])
  const session = `playsrc-acceptance-${process.pid}`
  const owner = await startDevelopmentProcess(target)
  let browserOpen = false
  try {
    await agent(["--session", session, "--headed", "--webgpu", "open", owner.url])
    browserOpen = true
    await agent(["--session", session, "wait", "--text", "Ready", "--timeout", "120000"])
    let body = parseJson<string>(await agent(["--session", session, "eval", "document.body.innerText"]))
    require(body.includes("DERIVED CACHE STORED"), "cold browser run did not store the derived payload")

    await agent(["--session", session, "click", "button.audio-toggle"])
    await agent(["--session", session, "wait", "--text", "Audio running", "--timeout", "10000"])

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
    require(
      parseJson<string>(await agent(["--session", session, "eval", "document.activeElement?.getAttribute('aria-label') ?? ''"])) === "Console command",
      "console activation did not focus its command input",
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
    await agent(["--session", session, "press", "Backquote"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      "getComputedStyle(document.querySelector('[role=dialog]')).display !== 'none'",
    ])
    require(
      parseJson<string>(await agent(["--session", session, "eval", "document.activeElement?.getAttribute('aria-label') ?? ''"])) === "Console command",
      "reopened console did not restore command focus",
    )
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
    const initialFireEvents = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number(document.querySelector('main').dataset.fireEvents)",
    ]))
    const initialExplosionEvents = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number(document.querySelector('main').dataset.explosionEvents)",
    ]))

    await acquirePointerLock(session)
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "left"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      `Number(document.querySelector('main').dataset.fireEvents) > ${initialFireEvents}`,
      "--timeout",
      "10000",
    ])
    await agent(["--session", session, "wait", "1200"])
    let blockerCount = parseJson<number>(await agent([
      "--session",
      session,
      "eval",
      "Number.parseInt(document.querySelector('.support-card button span').textContent,10)",
    ]))
    require(blockerCount >= initialBlockerCount + 2, "Soldier explosion presentation events were not observed")

    await agent(["--session", session, "press", "Escape"])
    await agent(["--session", session, "wait", "1000"])
    await agent(["--session", session, "click", ".class-rail button:nth-child(2)"])
    await agent(["--session", session, "wait", "--text", "STICKYBOMB LAUNCHER", "--timeout", "120000"])
    await acquirePointerLock(session)
    await agent(["--session", session, "mouse", "down", "left"])
    await agent(["--session", session, "wait", "100"])
    await agent(["--session", session, "mouse", "up", "left"])
    await agent([
      "--session",
      session,
      "wait",
      "--fn",
      `Number(document.querySelector('main').dataset.fireEvents) > ${initialFireEvents + 1}`,
      "--timeout",
      "10000",
    ])
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
    require(
      parseJson<number>(await agent([
        "--session",
        session,
        "eval",
        "Number(document.querySelector('main').dataset.explosionEvents)",
      ])) > initialExplosionEvents,
      "Demoman sticky detonation event was not observed",
    )

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
      && records[0]?.key === "a01cbd3d1a62ee695ff3767e9b5029a78d8fe7cd415e692a5994a1a403867c2c"
      && records[0]?.byteLength === 39_814_462
      && records[0]?.sha256 === "4553bd793f7334df823071f98807151020aae8a2246c4a737daa1d63a0d718bc",
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
      console: "history-completion-focus-repeated-visibility-replacement-close-passed",
      audio: "exact-buffers-decoded-and-context-running",
      shutdown: "pending",
    }
  } finally {
    if (browserOpen) await agent(["--session", session, "close"]).catch(() => {})
    await owner.interrupt()
  }
}

export async function runBrowserAcceptance(target: string | undefined): Promise<void> {
  const report = await verifyBrowserAcceptance(target)
  require(
    await unavailable("http://127.0.0.1:4173/readyz")
    && await unavailable("http://127.0.0.1:4174/readyz"),
    "owned listeners remained available after shutdown",
  )
  console.log(JSON.stringify({ ...report, shutdown: "sigint-child-and-listeners-released" }))
}
