/** Preserve the browser's normal audible autoplay policy. The startup owner
 * already has a gesture-required state; do not silently replace sound with a
 * muted second attempt or turn a policy denial into a fatal media error. */
export async function playStartupVideo(video: Pick<HTMLVideoElement, "play">): Promise<"started" | "gesture-required"> {
  try { await video.play(); return "started" }
  catch (error) {
    if (error instanceof DOMException && error.name === "NotAllowedError") return "gesture-required"
    throw error
  }
}
