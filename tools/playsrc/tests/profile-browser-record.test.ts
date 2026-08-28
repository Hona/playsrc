import { expect, test } from "bun:test"
import { publishBrowserRecord } from "../src/profile-browser"

test("browser owner replacement retries only bounded sharing failures without deleting authority", async () => {
  const pauses: number[] = []
  let calls = 0
  await publishBrowserRecord("new", "old", async (source, destination) => {
    expect([source, destination]).toEqual(["new", "old"])
    if (++calls < 3) throw Object.assign(new Error("sharing"), { code: "EPERM" })
  }, async milliseconds => { pauses.push(milliseconds) })
  expect(calls).toBe(3)
  expect(pauses).toEqual([50, 50])
  calls = 0
  await expect(publishBrowserRecord("new", "old", async () => {
    calls++; throw Object.assign(new Error("persistent sharing"), { code: "EBUSY" })
  }, async () => {})).rejects.toThrow("persistent sharing")
  expect(calls).toBe(20)
  calls = 0
  await expect(publishBrowserRecord("new", "old", async () => {
    calls++; throw Object.assign(new Error("not sharing"), { code: "EROFS" })
  }, async () => { throw new Error("must not retry") })).rejects.toThrow("not sharing")
  expect(calls).toBe(1)
})
