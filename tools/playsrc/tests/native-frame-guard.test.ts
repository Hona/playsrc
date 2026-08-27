import { expect, test } from "bun:test"
import { installNativeFrameGuard } from "../profile/native-frame-guard"

function fixture() {
  const events = new Map<string, (event?: any) => void>()
  const target = { addEventListener: (name: string, callback: any) => events.set(name, callback), removeEventListener: (name: string) => events.delete(name) }
  const quality: Record<string, number | string> = Object.fromEntries(["rootLod", "picmip", "shadowRenderToTexture", "flashlightDepthTexture", "reduceFillRate", "hdrLevel", "antialias", "aaQuality", "trilinear", "anisotropy", "waterForceExpensive", "waterReflectEntities", "vsync", "queueMode", "colorCorrection", "motionBlur", "sampleCount"].map(key => [key, 1]))
  quality.lightingProfile = "ldr"
  const canvas = { width: 1689, height: 1277 }
  const host = { ...target, performance: { now: () => 10 }, innerWidth: 1689, innerHeight: 1277, devicePixelRatio: 1,
    document: { ...target, querySelector: () => canvas, pointerLockElement: canvas, visibilityState: "visible", hasFocus: () => true },
    __playsrcFrameProfiler: { active: true, completedFrames: [] as any[] }, __playsrcProfile: { videoQuality: { ...quality } } }
  const expected = { width: 1689, height: 1277, dpr: 1, canvasWidth: 1689, canvasHeight: 1277, bots: 15, yaw: 315, pitch: 0, quality }
  return { host, expected, events, frame: () => ({ yaw: 315, pitch: 0, detail: { bots: 15 } }) }
}

test("records every completed frame and preserves the native push contract", () => {
  const { host, expected, frame } = fixture()
  const guard = installNativeFrameGuard(host, expected)
  expect(host.__playsrcFrameProfiler.completedFrames.push(frame(), frame())).toBe(2)
  expect(guard.state).toEqual({ checkedFrames: 2, failure: null, notificationError: null })
  expect(host.__playsrcFrameProfiler.completedFrames[0].nativeAdmission).toMatchObject({ width: 1689, canvasWidth: 1689, pointerLocked: true })
  guard.restore()
  expect(Object.hasOwn(host.__playsrcFrameProfiler.completedFrames, "push")).toBe(false)
})

test("single unexpected mouse event notifies once without suppressing input or truncating the trace", () => {
  const { host, expected, events, frame } = fixture()
  const failures: any[] = []
  const guard = installNativeFrameGuard(host, expected, failure => failures.push(failure))
  let suppressed = false
  const event = { movementX: 2, movementY: 0, isTrusted: true, preventDefault: () => { suppressed = true } }
  events.get("mousemove")!(event)
  events.get("mousemove")!(event)
  expect(failures).toHaveLength(1)
  expect(guard.state.failure.reason).toBe("unexpected native look input")
  expect(suppressed).toBe(false)
  expect(host.__playsrcFrameProfiler.active).toBe(true)
  host.__playsrcFrameProfiler.completedFrames.push(frame())
  expect(host.__playsrcFrameProfiler.completedFrames).toHaveLength(1)
})

test("rejects transient per-frame viewport, roster, view and quality changes", () => {
  for (const mutate of [
    (f: ReturnType<typeof fixture>) => { f.host.innerWidth = 3440 },
    (f: ReturnType<typeof fixture>) => { f.host.document.pointerLockElement.width = 3440 },
    (f: ReturnType<typeof fixture>) => { f.host.__playsrcProfile.videoQuality.picmip = 2 },
    (f: ReturnType<typeof fixture>) => { f.frame = () => ({ yaw: 315, pitch: 0, detail: { bots: 14 } }) },
    (f: ReturnType<typeof fixture>) => { f.frame = () => ({ yaw: 316, pitch: 0, detail: { bots: 15 } }) },
  ]) {
    const f = fixture(), guard = installNativeFrameGuard(f.host, f.expected)
    mutate(f)
    f.host.__playsrcFrameProfiler.completedFrames.push(f.frame())
    expect(guard.state.failure).not.toBeNull()
  }
})

test("serialized guard is inactive outside sampling and rejects incomplete quality", () => {
  const { host, expected, events, frame } = fixture()
  const install = new Function(`return (${installNativeFrameGuard.toString()})`)()
  host.__playsrcFrameProfiler.active = false
  const guard = install(host, expected)
  events.get("resize")!()
  host.__playsrcFrameProfiler.completedFrames.push(frame())
  expect(guard.state).toEqual({ checkedFrames: 0, failure: null, notificationError: null })
  guard.restore()
  expect(() => install(host, { ...expected, quality: {} })).toThrow("quality expectations")
})

test("notification failure cannot prevent application progress or change pinned expectations", () => {
  const { host, expected, frame } = fixture()
  const guard = installNativeFrameGuard(host, expected, () => { throw new Error("controller disconnected") })
  expected.quality.picmip = 2
  host.__playsrcProfile.videoQuality.picmip = 2
  expect(host.__playsrcFrameProfiler.completedFrames.push(frame())).toBe(1)
  expect(guard.state.failure.reason).toBe("native quality changed")
  expect(guard.state.notificationError).toContain("controller disconnected")
})
