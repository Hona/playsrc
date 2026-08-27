import { expect, test } from "bun:test"
import { summarizeBotAdmissions, type AdmissionEvent } from "../profile/bot-admission-timeline"

function fixture() {
  const events = [[9, 0], [1, 0], [2, 2], [3, 2], [4, 2], [5, 2], [6, 0], [8, 0]].map(([stage, actor], index): AdmissionEvent => ({ stage: stage!, actor: actor!, tick: 17, at: index, allocations: index * 2, allocatedBytes: index * 100, heapBytes: 1000 + index, value: stage === 8 ? 4096 : 0 }))
  const actors = [{ actor: 2, class: 5, team: 2 }]
  return { events, workerTimeOrigin: 1000, pageTimeOrigin: 900, dropped: 0, browserDropped: 0,
    browser: [{ stage: "publication", at: 110, tick: "18", detail: { actors } }, { stage: "model-request", at: 500, tick: "42", detail: { actors: [{ actor: 2, model: "medic", skin: 0 }] } }, { stage: "frame-submitted", at: 650, tick: "42", detail: { actors } }],
    frames: [{ at: 95, tick: 17 }, { at: 115, tick: 18 }, { at: 650, tick: 42 }] }
}

test("joins exact actor ticks and clock origins without calling later model appearance a spawn", () => {
  const result = summarizeBotAdmissions(fixture())
  expect(result.admissions[0]).toMatchObject({ actor: 2, tick: 17, at: 102, committed: true, publicationAt: 110, rosterFrameAt: 115, construction: { milliseconds: 3, allocations: 6, allocatedBytes: 300 }, firstModelRequest: { at: 500 }, firstModelSubmissionAt: 650, firstVisibleFrameAt: null, enclosingFrameGap: { milliseconds: 20 } })
  expect(result.spawnTickTimes.maximumMilliseconds).toBe(7)
  expect(result.quotaTicks).toEqual([17])
})

test("failed construction cannot be attributed to a later actor reusing the same identity", () => {
  const input = fixture()
  input.events.push({ ...input.events.at(-1)!, at: 8, stage: 15 })
  const row = summarizeBotAdmissions(input).admissions[0]!
  expect(row.committed).toBe(false)
  expect(row.publicationAt).toBeNull()
  expect(row.firstModelRequest).toBeNull()
})

test("rejects incomplete evidence and counter reversal rather than guessing missing costs", () => {
  expect(() => summarizeBotAdmissions({ ...fixture(), dropped: 1 })).toThrow("truncated")
  const input = fixture()
  input.events[input.events.length - 1] = { ...input.events.at(-1)!, allocatedBytes: -1 }
  expect(() => summarizeBotAdmissions(input)).toThrow("invalid")
  const reversed = fixture()
  reversed.frames.reverse()
  expect(() => summarizeBotAdmissions(reversed)).toThrow("timeline")
})
