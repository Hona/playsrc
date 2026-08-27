import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "./config"
import { startDevelopment } from "./dev"
import { generatedProfileIdentity } from "./profile-identity"

type OwnerLease = Readonly<{ schema: "playsrc-profile-owner-lease-v1"; token: string; expiresAt: number }>

const token = process.env.PLAYSRC_PROFILE_OWNER_TOKEN
const identity = process.env.PLAYSRC_PROFILE_SOURCE_IDENTITY
const metadataPath = process.env.PLAYSRC_PROFILE_OWNER_PATH
const target = process.argv[2]

if (!token || !identity || !metadataPath || !target || !path.isAbsolute(metadataPath)) {
  throw new Error("headed profile development owner requires an exact identity, target, token, and metadata path")
}

const leasePath = `${metadataPath}.lease`
const config = await loadLocalConfig()
let owner: Awaited<ReturnType<typeof startDevelopment>> | undefined
let finished = false
let monitor: ReturnType<typeof setInterval> | undefined

const stop = async (): Promise<void> => {
  if (finished) return
  finished = true
  if (monitor) clearInterval(monitor)
  // A leased service must not keep sockets/watchers alive when Vite teardown
  // stalls. This exits only this owner itself; no PID/group discovery or signal.
  const deadline = setTimeout(() => {
    console.error("headed profile development owner exceeded its 2000 ms cleanup budget")
    process.exit(1)
  }, 2_000)
  try {
    await owner?.close()
    try {
      const value = JSON.parse(await readFile(metadataPath, "utf8")) as { token?: string }
      if (value.token === token) await rm(metadataPath, { force: true })
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
    }
    try {
      const value = JSON.parse(await readFile(leasePath, "utf8")) as OwnerLease
      if (value.token === token) await rm(leasePath, { force: true })
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
    }
  } finally { clearTimeout(deadline) }
}

const fail = (error: unknown): void => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error))
  void stop().finally(() => { process.exit(1) })
}

process.once("SIGINT", () => { void stop().catch(fail) })
process.once("SIGTERM", () => { void stop().catch(fail) })

try {
  owner = await startDevelopment(config, target)
  if (finished) await owner.close()
  else {
    await mkdir(path.dirname(metadataPath), { recursive: true })
    const metadata = Object.freeze({
      schema: "playsrc-profile-owner-v1",
      token,
      identity,
      target,
      repository: repositoryRoot,
      pid: process.pid,
      url: owner.url,
      readyAt: new Date().toISOString(),
      startup: owner.startup,
      generatedIdentity: await generatedProfileIdentity(),
    })
    const temporary = `${metadataPath}.${process.pid}.tmp`
    try {
      await writeFile(temporary, `${JSON.stringify(metadata)}\n`)
      await rename(temporary, metadataPath)
    } finally {
      await rm(temporary, { force: true })
    }
    monitor = setInterval(() => {
      void (async () => {
        try {
          const lease = JSON.parse(await readFile(leasePath, "utf8")) as OwnerLease
          if (lease.schema !== "playsrc-profile-owner-lease-v1" || lease.token !== token || Date.now() >= lease.expiresAt) {
            await stop()
          }
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code === "ENOENT") await stop()
          else throw error
        }
      })().catch(fail)
    }, 500)
    console.error(`playsrc headed profile owner ready target=${target} milliseconds=${owner.startup.totalMilliseconds}`)
  }
} catch (error) {
  await stop().catch((cleanup) => console.error(cleanup instanceof Error ? cleanup.message : String(cleanup)))
  throw error
}
