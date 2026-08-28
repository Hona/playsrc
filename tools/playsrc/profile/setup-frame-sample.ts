// Use callback-arrival time consistently, including the first interval. A rAF
// timestamp can precede performance.now() at admission within the same frame.
export async function sampleSetupFrames(
  clock = () => performance.now(),
  schedule = (callback: FrameRequestCallback) => requestAnimationFrame(callback),
  seconds = 5,
) {
  if(seconds!==5&&seconds!==10)throw Error("Setup frame sample must be five or ten real seconds")
  const at = clock(), frames: number[] = []
  let previous = at
  await new Promise<void>(resolve => {
    const frame = () => {
      const now = clock()
      frames.push(now - previous)
      previous = now
      if (now - at >= seconds*1000) resolve()
      else schedule(frame)
    }
    schedule(frame)
  })
  return { seconds: (clock() - at) / 1000, frames }
}
