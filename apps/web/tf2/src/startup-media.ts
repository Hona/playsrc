type ExpectedMetadata = Readonly<{ video: Readonly<{ width: number; height: number }>; durationMicroseconds: number }>
type ObservedVideo = Pick<HTMLVideoElement, "videoWidth" | "videoHeight" | "duration" | "readyState" | "networkState">

export function startupMetadataFacts(video: ObservedVideo, expected: ExpectedMetadata, generation: number, sourceMatches: boolean) {
  return {
    generation, sourceMatches,
    expected: { width: expected.video.width, height: expected.video.height, durationMicroseconds: expected.durationMicroseconds },
    observed: { width: video.videoWidth, height: video.videoHeight, durationSeconds: String(video.duration), readyState: video.readyState, networkState: video.networkState },
  }
}

export function validateStartupMetadata(video: ObservedVideo, expected: ExpectedMetadata, generation: number, sourceMatches: boolean): void {
  if (!sourceMatches || !Number.isFinite(video.duration) || video.duration <= 0
    || video.videoWidth !== expected.video.width || video.videoHeight !== expected.video.height
    || Math.abs(video.duration * 1_000_000 - expected.durationMicroseconds) > 1_000) {
    throw new Error(`Configured startup media metadata differs: ${JSON.stringify(startupMetadataFacts(video, expected, generation, sourceMatches))}`)
  }
}

/** Selecting src does not synchronously clear metadata for the old resource.
 * Observe the new resource's event, never its predecessor's HAVE_METADATA. */
export async function loadStartupMetadata(video: HTMLVideoElement, url: string, signal: AbortSignal): Promise<void> {
  if (signal.aborted) throw new DOMException("Startup media preparation cancelled", "AbortError")
  await new Promise<void>((resolve, reject) => {
    const cleanup = () => {
      video.removeEventListener("loadedmetadata", loaded)
      video.removeEventListener("error", failed)
      signal.removeEventListener("abort", aborted)
    }
    const loaded = () => { if (video.currentSrc === url) { cleanup(); resolve() } }
    const failed = () => { cleanup(); reject(new Error(`Startup media decode failed: MediaError:${video.error?.code ?? 0}`)) }
    const aborted = () => { cleanup(); reject(new DOMException("Startup media preparation cancelled", "AbortError")) }
    video.addEventListener("loadedmetadata", loaded)
    video.addEventListener("error", failed)
    signal.addEventListener("abort", aborted, { once: true })
    try { video.src = url; video.load() }
    catch (error) { cleanup(); reject(error) }
  })
}
