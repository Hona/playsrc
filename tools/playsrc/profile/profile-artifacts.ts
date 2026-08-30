import { readFile, writeFile, rename, mkdir } from "node:fs/promises"
import path from "node:path"

// Worker-local extraction closures run only after its tests/contexts/input have
// ended AND the profile owner has closed the actual browser and released the
// native desktop stage. No browser or sample clock continues in the background.
const artifacts: Array<() => Promise<void>> = []
let workerDirectory: string | undefined
let nativeTeardown: (() => Promise<void>) | undefined
let nativeProbe: { prepare(): Promise<void>; close(): void } | undefined
export function registerProfileNativeProbe(probe: { prepare(): Promise<void>; close(): void }): void { nativeProbe = probe }
async function publish(name: string, value: unknown) {
  const file = path.join(workerDirectory!, name)
  await writeFile(`${file}.tmp`, JSON.stringify(value), { flag: "wx" })
  await rename(`${file}.tmp`, file)
}
async function receive(name: string) {
  for (;;) {
    if (Date.now() >= Number(process.env.PLAYSRC_PROFILE_DEADLINE)) throw new Error("Desktop transition exceeded the profile deadline")
    const text = await readFile(path.join(workerDirectory!, name), "utf8").catch(error => {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
      return null
    })
    if (text) {
      const value = JSON.parse(text)
      if (value.token !== process.env.PLAYSRC_PROFILE_DESKTOP_CHANNEL) throw new Error("Desktop worker handoff differs")
      return value
    }
    await new Promise(resolve => setTimeout(resolve, 25))
  }
}
export async function prepareProfileWorkerBrowser(workerIndex: number, launch: unknown): Promise<string> {
  return beginWorkerBrowser(workerIndex, "playwright", launch)
}
export async function prepareProfileNativeBrowser(workerIndex: number, executablePath: string, teardown: () => Promise<void>): Promise<void> {
  nativeTeardown = teardown
  await beginWorkerBrowser(workerIndex, "native-edge", { executablePath })
}
async function beginWorkerBrowser(workerIndex: number, kind: "playwright" | "native-edge", launch: unknown): Promise<string> {
  if (!Number.isSafeInteger(workerIndex) || workerIndex < 0) throw new Error("Invalid profile worker identity")
  await nativeProbe?.prepare()
  workerDirectory = path.join(process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, `desktop-worker-${workerIndex}`)
  await mkdir(workerDirectory)
  await publish("request.json", { token: process.env.PLAYSRC_PROFILE_DESKTOP_CHANNEL, workerPid: process.pid, kind, launch })
  return (await receive("browser.json")).endpoint
}
export async function profileArtifact(action: () => Promise<void>): Promise<void> {
  if (process.platform !== "win32") return action()
  if (!process.env.PLAYSRC_PROFILE_DESKTOP_CHANNEL) throw new Error("Artifact extraction requires the owned profile lifecycle")
  artifacts.push(action)
}

export async function finishProfileArtifacts(succeeded: boolean): Promise<void> {
  const token = process.env.PLAYSRC_PROFILE_DESKTOP_CHANNEL
  if (!token) return
  if (!workerDirectory) throw new Error("Worker did not enter its owned browser stage")
  await nativeTeardown?.()
  nativeProbe?.close()
  await publish("release.json", { token, succeeded })
  const receipt = await receive("released.json")
  // Authenticated native stage receipt is available to extraction checks too.
  process.env.PLAYSRC_PROFILE_DESKTOP_RECEIPT = path.join(workerDirectory, "released.json")
  for (const action of artifacts.splice(0)) await action()
}
