type Voice = Readonly<{
  started: number
  stopped: number | null
  ended: number | null
  disconnected: number | null
  duration: number
  loop: boolean
}>

export type PyroAudioEvidence = Readonly<{
  voices: readonly Voice[]
  edges: readonly Readonly<{ type: string; locked: boolean; audio?: number; trusted: boolean }>[]
  state: Readonly<{ audioStarts?: string }>
}>

/** Also accepts retained headed captures, so a probe correction needs no new benchmark. */
export function verifyPyroAudioRelease(report: PyroAudioEvidence, sampleRate: number): void {
  const require = (condition: boolean, detail: string) => { if (!condition) throw new Error(`Pyro audio evidence: ${detail}`) }
  require(Number.isFinite(sampleRate) && sampleRate > 0, "missing actual context sample rate")
  const flame = report.voices.filter(voice => Math.abs(voice.duration - 160064 / 44100) < 0.001)
  require(flame.length >= 4, "missing repeated firing patches")
  require(flame.every(voice => (voice.stopped !== null || voice.ended !== null) && voice.disconnected !== null), "firing graph survived release")
  const loops = flame.filter(voice => voice.loop)
  require(loops.length === 2, "expected one loop for each press")
  const releases = report.edges.filter(edge => edge.type === "mouseup" && edge.locked)
  for (let index = 0; index < loops.length; index++) {
    const release = releases[index]!, loop = loops[index]!
    require(release?.trusted === true && release.audio !== undefined, "release was not real pointer input")
    require(loop.stopped !== null && loop.stopped - release.audio! >= 0 && loop.stopped - release.audio! < 0.15, "loop destruction missed release")
  }
  const tails = report.voices.filter(voice => Math.abs(voice.duration - 36096 / 44100) < 0.001)
  require(tails.length === 2, "missing authored winddowns")
  // currentTime is sampled on the control thread, at render-quantum boundaries.
  // An ended callback may observe the preceding quantum, not the exact EOS frame.
  const quantum = 128 / sampleRate
  require(tails.every(voice => voice.stopped === null && voice.ended !== null
    && voice.ended - voice.started + quantum >= voice.duration), "winddown was cut short")
  require(report.state.audioStarts?.includes("Weapon_Shotgun.Single") === true, "other gameplay audio did not continue")
  require(!report.voices.some(voice => voice.loop && voice.stopped === null && voice.ended === null), "active looping voice remained")
}
