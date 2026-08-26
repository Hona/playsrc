import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { headedProfileConfiguration } from "../profile/profile-config"
import { headedProfileTarget } from "../profile/profile-target"
import { buildCacheDirectory, rustBuildIdentity } from "../src/build-identity"
import { acquireHeadedProfileLock, parseHeadedProfile, profileSourceIdentity, releaseHeadedProfileLock } from "../src/profile-runner"

const directories: string[] = []

afterEach(async () => {
  await Promise.all(directories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("bounded headed profile orchestration", () => {
  test("accepts exact profile identities and rejects hidden browser execution", () => {
    expect(parseHeadedProfile(["gameplay", "--headed"])).toEqual({ profile: "gameplay", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["map-memory", "--headed"])).toEqual({ profile: "map-memory", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["2fort", "--fresh"])).toEqual({ profile: "2fort", fresh: true, playwright: [] })
    expect(parseHeadedProfile(["2fort-bots", "--fresh", "--output", "/evidence"])).toEqual({
      profile: "2fort-bots",
      fresh: true,
      playwright: ["--output", "/evidence"],
    })
    expect(parseHeadedProfile(["2fort-visual", "--headed"])).toEqual({ profile: "2fort-visual", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["sky-coherence", "--headed"])).toEqual({ profile: "sky-coherence", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["upward-training-bots", "--headed"])).toEqual({ profile: "upward-training-bots", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["2fort-match"])).toEqual({
      profile: "2fort-match",
      fresh: false,
      playwright: [],
    })
    expect(() => parseHeadedProfile([])).toThrow("Usage:")
    expect(() => parseHeadedProfile(["unknown"])).toThrow("Usage:")
    expect(() => parseHeadedProfile(["gameplay", "--headless"])).toThrow("never accept headless")
  })

  test("keeps reusable development servers visibly headed and below three minutes", () => {
    const config = headedProfileConfiguration({ match: "cold-map.profile.ts", target: "jump_beef" })
    expect(config.timeout).toBe(175_000)
    expect(config.globalTimeout).toBe(175_000)
    expect(config.use?.headless).toBe(false)
    expect(config.workers).toBe(1)
    expect(Array.isArray(config.webServer)).toBe(false)
    expect((config.webServer as { reuseExistingServer: boolean; timeout: number }).reuseExistingServer).toBe(true)
    expect((config.webServer as { timeout: number }).timeout).toBe(175_000)
  })

  test("selects each exact authored profile map without a duplicate fallback authority", () => {
    expect(headedProfileTarget({})).toBe("jump_beef")
    expect(headedProfileTarget({ PROFILE_CTF_BOTS: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_2FORT_VISUAL: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_2FORT_MEMORY: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_COMBAT_IMPACTS: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "local-practice" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "2fort-match" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_MEDIC_WEAPONS: "1" })).toBe("pl_upward")
    expect(headedProfileTarget({ PROFILE_SKY_COHERENCE: "1" })).toBe("pl_upward")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "demoman" })).toBe("pl_upward")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "upward-training-bots" })).toBe("pl_upward")
    expect(headedProfileTarget({}, "pl_upward")).toBe("pl_upward")
  })

  test("reclaims dead lock owners and rejects changed release ownership", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-profile-lock-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    await writeFile(pathname, `${JSON.stringify({ token: "stale", pid: 999_999_999 })}\n`)
    const acquired = await acquireHeadedProfileLock(pathname, "gameplay")
    const owner = JSON.parse(await readFile(pathname, "utf8")) as { token: string; pid: number; profile: string }
    expect(owner).toMatchObject({ token: acquired.token, pid: process.pid, profile: "gameplay" })
    await expect(releaseHeadedProfileLock(pathname, "not-the-owner")).rejects.toThrow("ownership changed")
    await releaseHeadedProfileLock(pathname, acquired.token)
    await expect(readFile(pathname)).rejects.toMatchObject({ code: "ENOENT" })
    await expect(acquireHeadedProfileLock(pathname, "gameplay", 180_001)).rejects.toThrow("three-minute bound")
  })

  test("hands a live machine-wide lock directly to the next waiter without a polling delay", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-profile-handoff-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const first = await acquireHeadedProfileLock(pathname, "hud")
    const waiting = acquireHeadedProfileLock(pathname, "gameplay", 2_000)
    await new Promise((resolve) => setTimeout(resolve, 20))
    const released = performance.now()
    await releaseHeadedProfileLock(pathname, first.token)
    const second = await waiting
    expect(performance.now() - released).toBeLessThan(100)
    expect(second.token).not.toBe(first.token)
    await releaseHeadedProfileLock(pathname, second.token)
  })

  test("binds shared build and source snapshots to exact checked repository inputs", async () => {
    const [build, source] = await Promise.all([rustBuildIdentity(), profileSourceIdentity()])
    expect(build).toMatch(/^[0-9a-f]{64}$/)
    expect(source).toMatch(/^[0-9a-f]{64}$/)
    expect(await rustBuildIdentity()).toBe(build)
    expect(buildCacheDirectory("/configured/cache", build)).toBe(path.join("/configured/cache", "prepared-builds", `${process.platform}-${process.arch}`, build))
    expect(() => buildCacheDirectory("/configured/cache", "invalid")).toThrow("identity is malformed")
  })
})
