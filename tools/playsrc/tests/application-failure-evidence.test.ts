import { expect, test } from "bun:test"
import { retainBeforeApplicationFailure, rejectAfterApplicationFailureEvidence } from "../profile/application-failure-evidence"

test("fatal application error waits for bounded evidence then still rejects the original error", async () => {
  const page = {}, error = new Error("game-advance:17"), events: string[] = []
  const release = retainBeforeApplicationFailure(page, async received => { expect(received).toBe(error); events.push("retained") })
  expect(() => retainBeforeApplicationFailure(page, async () => {})).toThrow("owner")
  await expect(rejectAfterApplicationFailureEvidence(page, error)).rejects.toBe(error)
  expect(events).toEqual(["retained"])
  release()
  await expect(rejectAfterApplicationFailureEvidence(page, error)).rejects.toBe(error)
  expect(events).toHaveLength(1)
})

test("failed or hung evidence never swallows a fatal transition or consumes the whole run", async () => {
  for (const retain of [async () => { throw new Error("drain failed") }, () => new Promise<void>(() => {})]) {
    const page = {}, error = new Error("game-advance:17")
    retainBeforeApplicationFailure(page, retain)
    await expect(rejectAfterApplicationFailureEvidence(page, error, 5)).rejects.toBe(error)
    expect(error.message).toContain("game-advance:17\nEvidence retention:")
  }
})
