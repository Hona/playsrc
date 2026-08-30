import { expect, test } from "bun:test"
import { readTicket, retryExclusiveOpenDenial } from "../src/profile-lock"

const denied = Object.assign(new Error("open denied"), { code: "EPERM" })
test("exclusive-open retries are bounded Windows denials, never acquisition", () => {
  expect(retryExclusiveOpenDenial("EPERM", "win32", undefined, 0)).toBe(true)
  expect(retryExclusiveOpenDenial("EPERM", "win32", 0, 99)).toBe(true)
  expect(retryExclusiveOpenDenial("EPERM", "win32", 0, 100)).toBe(false)
  expect(retryExclusiveOpenDenial("EPERM", "darwin", undefined, 0)).toBe(false)
  expect(retryExclusiveOpenDenial("EACCES", "win32", undefined, 0)).toBe(false)
})
test("Windows ticket retirement races retry reads without skipping ownership", async () => {
  let reads = 0
  const owner = { pid: 42, token: "unchanged-owner" }
  expect(await readTicket("ticket", async () => { if (++reads < 3) throw denied; return JSON.stringify(owner) }, "win32")).toEqual(owner)
  expect(reads).toBe(3)
  reads = 0
  expect(await readTicket("ticket", async () => { if (++reads === 1) throw denied; throw Object.assign(new Error("retired"), { code: "ENOENT" }) }, "win32")).toBeNull()
  expect(reads).toBe(2)
})
test("persistent denial, non-Windows denial and malformed ownership remain faults", async () => {
  let reads = 0
  await expect(readTicket("ticket", async () => { reads++; throw denied }, "win32")).rejects.toBe(denied)
  expect(reads).toBe(4)
  reads = 0
  await expect(readTicket("ticket", async () => { reads++; throw denied }, "darwin")).rejects.toBe(denied)
  expect(reads).toBe(1)
  await expect(readTicket("ticket", async () => JSON.stringify({ pid: 0, token: "bad" }), "win32")).rejects.toThrow("Malformed")
})
