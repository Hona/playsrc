import { expect, test } from "bun:test"
import { replaceProfileLeaseFile } from "../src/profile-lease-rename"

test("both development and browser lease writers retain the bounded atomic replacement contract", async () => {
  for (const name of ["profile-runner.ts", "profile-browser.ts"]) {
    const source = await Bun.file(new URL(`../src/${name}`, import.meta.url)).text()
    expect(source).toContain('from "./profile-lease-rename"')
    expect(source).toContain("await replaceProfileLeaseFile(")
  }
})

test("a Windows reader collision retries only the same atomic lease replacement", async () => {
  let now = 0, calls = 0
  const error = Object.assign(new Error("reader sharing violation"), { code: "EPERM" })
  await replaceProfileLeaseFile("owned.tmp", "owned.lease", { platform: "win32", now: () => now, wait: async milliseconds => { now += milliseconds },
    replace: async (source, destination) => { expect([source, destination]).toEqual(["owned.tmp", "owned.lease"]); if (++calls < 3) throw error } })
  expect(calls).toBe(3); expect(now).toBe(20)
})

test("permanent Windows lease faults retain the original error within250ms", async () => {
  let now = 0
  const error = Object.assign(new Error("permission denied"), { code: "EPERM" })
  await expect(replaceProfileLeaseFile("owned.tmp", "owned.lease", { platform: "win32", now: () => now, wait: async milliseconds => { now += milliseconds },
    replace: async () => { throw error } })).rejects.toBe(error)
  expect(now).toBe(250)
})

test("unrelated errors and non-Windows ownership faults are never retried", async () => {
  for (const [platform, code] of [["darwin", "EPERM"], ["win32", "ENOENT"], ["win32", "EIO"]]) {
    let calls = 0
    const error = Object.assign(new Error(code), { code })
    await expect(replaceProfileLeaseFile("owned.tmp", "owned.lease", { platform, replace: async () => { calls++; throw error },
      wait: async () => { throw new Error("unexpected retry") } })).rejects.toBe(error)
    expect(calls).toBe(1)
  }
})
