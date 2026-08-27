/** Read-only admission for a fixed-view native movement capture. No input is
 * cancelled, viewport changed, or profiler boundary silently moved on failure.
 * The controller must end and retain the invalid attempt on notification. */
export function installNativeFrameGuard(host: any, expected: {
  width: number; height: number; dpr: number; canvasWidth: number; canvasHeight: number;
  bots: number; yaw: number; pitch: number; quality: Record<string, unknown>;
  quotaRamp?: boolean;
}, notify: (failure: any) => void = () => {}) {
  const profile = host.__playsrcFrameProfiler
  if (!profile || !Array.isArray(profile.completedFrames)) throw new Error("Native frame profiler is unavailable")
  if (![expected.width, expected.height, expected.dpr, expected.canvasWidth, expected.canvasHeight, expected.bots, expected.yaw, expected.pitch].every(Number.isFinite)
    || expected.width <= 0 || expected.height <= 0 || expected.dpr <= 0 || expected.bots < 1) throw new Error("Native frame expectations are invalid")
  const qualityKeys = ["rootLod", "picmip", "shadowRenderToTexture", "flashlightDepthTexture", "reduceFillRate", "hdrLevel", "antialias", "aaQuality", "trilinear", "anisotropy", "waterForceExpensive", "waterReflectEntities", "vsync", "queueMode", "colorCorrection", "motionBlur", "sampleCount", "lightingProfile"]
  if (qualityKeys.some(key => expected.quality?.[key] === undefined)) throw new Error("Native quality expectations are incomplete")
  const baseline = { ...expected, quality: { ...expected.quality } }
  const frames = profile.completedFrames
  const descriptor = Object.getOwnPropertyDescriptor(frames, "push")
  const push = frames.push
  const state = { checkedFrames: 0, failure: null as any, notificationError: null as string | null }
  let priorBots = 0
  const fail = (reason: string, actual?: unknown) => {
    if (state.failure) return
    state.failure = { at: host.performance.now(), reason, actual }
    try { notify(state.failure) } catch (error) { state.notificationError = String(error) }
  }
  const snapshot = () => {
    const canvas = host.document.querySelector("canvas.world-canvas")
    const source = host.__playsrcProfile?.videoQuality
    return {
      width: host.innerWidth, height: host.innerHeight, dpr: host.devicePixelRatio,
      canvasWidth: canvas?.width ?? null, canvasHeight: canvas?.height ?? null,
      visible: host.document.visibilityState === "visible", focused: host.document.hasFocus(),
      pointerLocked: Boolean(canvas) && host.document.pointerLockElement === canvas,
      quality: Object.fromEntries(qualityKeys.map(key => [key, source?.[key] ?? null])),
    }
  }
  Object.defineProperty(frames, "push", { configurable: true, writable: true, value(this: any[], ...values: any[]) {
    if (profile.active) for (const frame of values) {
      const actual = snapshot()
      frame.nativeAdmission = actual
      state.checkedFrames += 1
      if (!actual.visible || !actual.focused || !actual.pointerLocked) fail("native window lost visible focused gameplay capture", actual)
      else if (["width", "height", "dpr", "canvasWidth", "canvasHeight"].some(key => actual[key] !== baseline[key])) fail("native viewport changed", actual)
      else if (qualityKeys.some(key => actual.quality[key] !== baseline.quality[key])) fail("native quality changed", actual.quality)
       else if (baseline.quotaRamp
         ? !Number.isSafeInteger(frame.detail?.bots) || frame.detail.bots < priorBots || frame.detail.bots > baseline.bots
         : frame.detail?.bots !== baseline.bots) fail("native rendered roster changed", frame.detail?.bots)
       else if (frame.yaw !== baseline.yaw || frame.pitch !== baseline.pitch) fail("native rendered view changed", { yaw: frame.yaw, pitch: frame.pitch })
       priorBots = frame.detail?.bots
    }
    return push.apply(this, values)
  } })
  const mouse = (event: any) => {
    if (profile.active && host.document.pointerLockElement && (event.movementX !== 0 || event.movementY !== 0)) {
      fail("unexpected native look input", { x: event.movementX, y: event.movementY, trusted: event.isTrusted })
    }
  }
  const changed = () => { if (profile.active) fail("native capture lifecycle changed", snapshot()) }
  host.document.addEventListener("mousemove", mouse, { passive: true })
  host.document.addEventListener("visibilitychange", changed, { passive: true })
  host.document.addEventListener("pointerlockchange", changed, { passive: true })
  host.addEventListener("resize", changed, { passive: true })
  host.addEventListener("blur", changed, { passive: true })
  return { state, restore() {
    if (descriptor) Object.defineProperty(frames, "push", descriptor)
    else delete frames.push
    host.document.removeEventListener("mousemove", mouse)
    host.document.removeEventListener("visibilitychange", changed)
    host.document.removeEventListener("pointerlockchange", changed)
    host.removeEventListener("resize", changed)
    host.removeEventListener("blur", changed)
  } }
}
