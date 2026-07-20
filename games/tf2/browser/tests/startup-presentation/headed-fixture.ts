import type { Tf2StartupDescriptor } from "../../src/startup-presentation"

export const TF2_STARTUP_HEADED_VIEWPORTS = Object.freeze([
  Object.freeze({ width: 1_280, height: 720 }),
  Object.freeze({ width: 390, height: 844 }),
] as const)

export const TF2_STARTUP_CAPTURE_POINTS = Object.freeze([
  Object.freeze({ identity: "first", mediaTimeMicroseconds: 0 }),
  Object.freeze({ identity: "middle", mediaTimeMicroseconds: 5_004_170 }),
  Object.freeze({ identity: "final", mediaTimeMicroseconds: 10_008_340 }),
] as const)

export type Tf2StartupHeadedHost = Readonly<{
  mount(descriptor: Tf2StartupDescriptor, viewport: Readonly<{ width: number; height: number }>): Promise<void>
  seek(mediaTimeMicroseconds: number): Promise<void>
  assertMenuExposure(expected: Readonly<{ visible: boolean; focusable: boolean; pointerEnabled: boolean; keyboardEnabled: boolean; accessibilityHidden: boolean }>): Promise<void>
  capture(identity: string): Promise<Readonly<{ width: number; height: number; sha256: string }>>
  destroy(): Promise<void>
}>

export async function runTf2StartupHeadedFixture(descriptor: Tf2StartupDescriptor, host: Tf2StartupHeadedHost) {
  const captures: Readonly<{ identity: string; width: number; height: number; sha256: string }>[] = []
  try {
    for (const viewport of TF2_STARTUP_HEADED_VIEWPORTS) {
      await host.mount(descriptor, viewport)
      await host.assertMenuExposure({ visible: false, focusable: false, pointerEnabled: false, keyboardEnabled: false, accessibilityHidden: true })
      for (const point of TF2_STARTUP_CAPTURE_POINTS) {
        await host.seek(point.mediaTimeMicroseconds)
        const capture = await host.capture(`${viewport.width}x${viewport.height}-${point.identity}`)
        if (capture.width !== viewport.width || capture.height !== viewport.height || !/^[0-9a-f]{64}$/u.test(capture.sha256)) throw new Error(`Invalid startup capture ${point.identity}`)
        captures.push(Object.freeze({ identity: `${viewport.width}x${viewport.height}-${point.identity}`, ...capture }))
      }
      await host.destroy()
    }
    return Object.freeze(captures)
  } finally {
    await host.destroy()
  }
}
