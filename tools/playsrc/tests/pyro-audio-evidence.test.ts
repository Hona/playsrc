import { expect, test } from "bun:test"
import { verifyPyroAudioRelease, type PyroAudioEvidence } from "../profile/pyro-audio-evidence"

function evidence() {
  const flameDuration = 160064 / 44100, tailDuration = 36096 / 44100
  return {
    voices: [0, 2].flatMap(started => [
      { started, stopped: started + 1, ended: started + 1, disconnected: started + 1, duration: flameDuration, loop: false },
      { started, stopped: started + 1, ended: started + 1, disconnected: started + 1, duration: flameDuration, loop: true },
      { started: started + 1, stopped: null as number | null, ended: started + 1 + tailDuration - 0.0025, disconnected: started + 1 + tailDuration, duration: tailDuration, loop: false },
    ]),
    edges: [1, 3].map(audio => ({ type: "mouseup", locked: true, trusted: true, audio })),
    state: { audioStarts: "Weapon_Shotgun.Single" },
  } satisfies PyroAudioEvidence
}

test("natural EOS tolerates one control-thread quantum, never a forced stop or shortened tail", () => {
  expect(() => verifyPyroAudioRelease(evidence(), 48000)).not.toThrow()
  for (const change of [
    (value: ReturnType<typeof evidence>) => { value.voices[2]!.stopped = 1.8 },
    (value: ReturnType<typeof evidence>) => { value.voices[2]!.ended -= 0.01 },
    (value: ReturnType<typeof evidence>) => { value.voices[1]!.stopped = null },
    (value: ReturnType<typeof evidence>) => { value.edges[0]!.trusted = false },
  ]) {
    const value = evidence(); change(value)
    expect(() => verifyPyroAudioRelease(value, 48000)).toThrow()
  }
})
