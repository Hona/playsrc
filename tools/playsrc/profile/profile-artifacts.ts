import { readFile, writeFile, rename } from "node:fs/promises"
import path from "node:path"

// Worker-local extraction closures run only after its tests/contexts/input have
// ended AND the profile owner has closed the actual browser and released the
// native desktop stage. No browser or sample clock continues in the background.
const artifacts: Array<() => Promise<void>> = []
export async function profileArtifact(action: () => Promise<void>): Promise<void> {
  if (process.platform !== "win32") return action()
  if (!process.env.PLAYSRC_PROFILE_DESKTOP_CHANNEL) throw new Error("Artifact extraction requires the owned profile lifecycle")
  artifacts.push(action)
}

export async function finishProfileArtifacts(succeeded: boolean): Promise<void> {
  const token = process.env.PLAYSRC_PROFILE_DESKTOP_CHANNEL
  if (!token) return
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  const file = path.join(directory, "desktop-extraction.json")
  await writeFile(`${file}.tmp`, JSON.stringify({ token, succeeded }), { flag: "wx" })
  await rename(`${file}.tmp`, file)
  const deadline = Number(process.env.PLAYSRC_PROFILE_DEADLINE)
  for (;;) {
    if (Date.now() >= deadline) throw new Error("Desktop teardown exceeded the profile deadline")
    const receipt = await readFile(path.join(directory, "desktop-extraction-released.json"), "utf8").catch(error => {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
      return null
    })
    if (receipt) {
      if (JSON.parse(receipt).token !== token) throw new Error("Desktop extraction handoff differs")
      break
    }
    await new Promise(resolve => setTimeout(resolve, 25))
  }
  for (const action of artifacts.splice(0)) await action()
}
