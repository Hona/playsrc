import { expect, test } from "bun:test"
import { fixedInputPulses } from "../profile/fixed-input-pulses"

test("slower observations cannot lengthen movement or change scheduled pulse order", async () => {
  let clock = 0
  const releases: (() => void)[] = [], sent: number[] = []
  const result = fixedInputPulses({ now: () => clock, wait: async duration => { clock += duration }, admit: async () => { clock += 5 },
    send: async () => { sent.push(clock); if (sent.length === 6) for (const resolve of releases) resolve() },
    observe: async down => { if (sent.length < 6) await new Promise<void>(resolve => releases.push(resolve)); return down },
  })
  expect((await result).observations).toEqual([true, false, true, false, true, false])
  expect(sent).toEqual([200, 300, 400, 500, 600, 700])
})
