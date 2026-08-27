import { expect, test } from "bun:test"
import { parseAudioArtifact } from "../src/artifacts"
import { MAX_GRAPH_ENTRIES } from "@playsrc/asset-store/graph"

function audio(patches: number, includeRows = true) {
  const bytes: number[] = [], encoder = new TextEncoder()
  const u32 = (value: number) => { const word = new Uint8Array(4); new DataView(word.buffer).setUint32(0, value, true); bytes.push(...word) }
  const text = (value: string) => { const encoded = encoder.encode(value); u32(encoded.length); bytes.push(...encoded) }
  bytes.push(...encoder.encode("PAUD")); u32(3); bytes.push(...new Uint8Array(32)); u32(0x3f800000); u32(1)
  text("scripts/game_sounds_weapons.txt"); bytes.push(...new Uint8Array(32)); u32(0); u32(patches)
  if (includeRows) for (let index = 0; index < patches; index++) { text(`sound/weapons/fixture-${index}.wav`); u32(44100); u32(1000); u32(index % 2 ? 0xffffffff : 441) }
  return new Uint8Array(bytes)
}

test("PAUD retains every admitted WAV cue beyond the former stock-only ceiling", () => {
  for (const count of [129, 512, MAX_GRAPH_ENTRIES]) {
    const parsed = parseAudioArtifact(audio(count))
    expect(parsed.patches.size).toBe(count)
    expect(parsed.patches.get("sound/weapons/fixture-0.wav")?.loopStartSeconds).toBe(0.01)
    expect(parsed.patches.get("sound/weapons/fixture-1.wav")?.loopStartSeconds).toBeNull()
  }
  expect(() => parseAudioArtifact(audio(MAX_GRAPH_ENTRIES + 1, false))).toThrow("sound patch count")
  expect(() => parseAudioArtifact(audio(129).slice(0, -1))).toThrow("truncated")
})
