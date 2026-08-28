import { expect, test } from "bun:test"
import { playStartupVideo } from "../src/startup-playback"

test("normal autoplay denial asks for a gesture without muting or retrying", async () => {
  let calls = 0
  expect(await playStartupVideo({ play: async () => { calls++; throw new DOMException("User activation required", "NotAllowedError") } })).toBe("gesture-required")
  expect(calls).toBe(1)
})

test("playback success and actual decoder errors retain their distinct outcomes", async () => {
  expect(await playStartupVideo({ play: async () => {} })).toBe("started")
  const failure = new DOMException("Unsupported media", "NotSupportedError")
  await expect(playStartupVideo({ play: async () => { throw failure } })).rejects.toBe(failure)
})
