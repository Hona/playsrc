// Use callback-arrival time consistently, including the first interval. A rAF
// timestamp can precede performance.now() at admission within the same frame.
export async function sampleSetupFrames(
  clock = () => performance.now(),
  schedule = (callback: FrameRequestCallback) => requestAnimationFrame(callback),
) {
  const at = clock(), frames: number[] = []
  let previous = at
  await new Promise<void>(resolve => {
    const frame = () => {
      const now = clock()
      frames.push(now - previous)
      previous = now
      if (now - at >= 5000) resolve()
      else schedule(frame)
    }
    schedule(frame)
  })
  return { seconds: (clock() - at) / 1000, frames }
}
