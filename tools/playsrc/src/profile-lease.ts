import { randomUUID } from "node:crypto"
import { readFile, rename, rm, writeFile } from "node:fs/promises"

const filesystem = { readFile, rename, rm, writeFile, pause: () => Bun.sleep(10) }

/** Caller holds the checked profile lock. Windows readers may briefly deny the
 * atomic replacement; retry only this owned temporary file, never its expiry. */
export async function writeProfileLease(metadataPath: string, token: string, milliseconds: number, fs = filesystem): Promise<void> {
  const destination = `${metadataPath}.lease`
  const temporary = `${destination}.${process.pid}.${randomUUID()}.tmp`
  try {
    await fs.writeFile(temporary, `${JSON.stringify({ schema: "playsrc-profile-owner-lease-v1", token, expiresAt: Date.now() + milliseconds })}\n`, { flag: "wx" })
    for (let attempt = 0; ; attempt++) {
      try { await fs.rename(temporary, destination); return }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EPERM" || attempt === 2) throw error
        const current = await fs.readFile(destination, "utf8").catch(error => {
          if ((error as NodeJS.ErrnoException).code === "ENOENT") return null
          throw error
        })
        if (current !== null && JSON.parse(current).token !== token) throw new Error("Shared development lease changed during publication")
        await fs.pause()
      }
    }
  } finally { await fs.rm(temporary, { force: true }) }
}
