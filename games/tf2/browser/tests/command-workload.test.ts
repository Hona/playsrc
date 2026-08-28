import { expect, test } from "bun:test"
import { CommandWorkloadPlayer, validateWorkload, type CommandWorkload } from "../src/command-workload"
import { encodeCommand } from "../src/codec"

function plan(): CommandWorkload {
  const command = Buffer.from(encodeCommand({ forward: 10, side: 0, yawDegrees: 45, pitchDegrees: 5 })).toString("hex")
  return { schema: 1, journalSha256: "a".repeat(64), bspSha256: "b".repeat(64), configurationSha256: "c".repeat(64), configurationBytes: 12,
    profile: 1, generation: 1, sampleStarted: 101000, sampleEnded: 106000,
    observes: [100, 101, 106, 107].map((nowSeconds, i) => ({ nowSeconds, suspended: false, snapshotTick: String(i), command,
      mutations: i === 1 ? [{ kind: 4, hex: "02000000" }] : [] })) }
}
test("preserves every recorded input and clock delta at real deadlines, never catches up by rewriting clocks", async () => {
  let now = 5000
  const events: unknown[] = []
  const player = new CommandWorkloadPlayer(plan(), 3, async request => { events.push([now, request]); return {} as any }, () => now, async ms => { now += ms })
  expect(await player.next(0n)).toMatchObject({ nowSeconds: 100, due: 5000 })
  expect(await player.next(1n)).toMatchObject({ nowSeconds: 101, due: 6000 })
  expect(events).toEqual([[6000, { kind: "team-selection", generation: 3, choice: "red" }]])
  now = 12000 // A late caller must remain late, with original timestamps intact.
  expect(await player.next(2n)).toMatchObject({ nowSeconds: 106, due: 11000 })
  expect(player.receipt).toMatchObject({ lateMilliseconds: 1000, yaw: 45, pitch: 5, cursor: 3 })
  expect(Buffer.from((await player.next(3n)).command).toString("hex")).toBe(plan().observes[3]!.command)
  expect(player.ended).toBe(true)
  await expect(player.next(4n)).rejects.toThrow("exhausted")
})
test("rejects wrong acknowledgement, malformed phase and cancellation without publishing partial input", async () => {
  expect(() => validateWorkload({ ...plan(), sampleEnded: 109000 })).toThrow("cover")
  const player = new CommandWorkloadPlayer(plan(), 1, async () => ({} as any))
  await expect(player.next(1n)).rejects.toThrow("acknowledgement")
  player.close()
  await expect(player.next(0n)).rejects.toThrow("closed")
})

test("startup mutation uses its real acknowledgement exactly once before the first observe", async () => {
  const input = plan(), events: unknown[] = []
  const mutation = { kind: 4 as const, hex: "02000000" }
  const source = { ...input, observes: input.observes.map((entry, i) => ({ ...entry, mutations: i === 0 ? [mutation] : [] })) }
  const response = { kind: "team-selection", generation: 3, id: 17 } as any
  const player = new CommandWorkloadPlayer(source, 3, async request => { events.push(request); return response }, () => 5000)
  expect(await player.redirect({ kind: "team-selection", generation: 3, choice: "red" })).toBe(response)
  await player.next(0n)
  expect(events).toEqual([{ kind: "team-selection", generation: 3, choice: "red" }])
  expect(() => player.redirect({ kind: "team-selection", generation: 3, choice: "red" })).toThrow("differs")
  const foreign = { kind: "team-selection" as const, generation: 4, choice: "blue" as const }
  expect(player.redirect(foreign)).toBe(foreign)
})

test("closing during a deadline wait releases no additional command or mutation", async () => {
  let now = 5000, sends = 0
  const player = new CommandWorkloadPlayer(plan(), 1, async () => { sends++; return {} as any }, () => now,
    async ms => { now += ms; player.close() })
  await player.next(0n)
  await expect(player.next(1n)).rejects.toThrow("cancelled")
  expect(player.receipt.cursor).toBe(1)
  expect(sends).toBe(0)
})

test("presentation groups cannot omit or repeat selected host ticks", () => {
  const group = { atSeconds: 100, firstHostTick: "1", lastHostTick: "2", selectedTicks: 2 }
  expect(() => validateWorkload({ ...plan(), presentations: [group] })).not.toThrow()
  expect(() => validateWorkload({ ...plan(), presentations: [group, { ...group, atSeconds: 101 }] })).toThrow("coverage")
  expect(() => validateWorkload({ ...plan(), presentations: [{ ...group, selectedTicks: 1 }] })).toThrow("coverage")
  expect(() => validateWorkload({ ...plan(), presentations: [] })).toThrow("count")
})
