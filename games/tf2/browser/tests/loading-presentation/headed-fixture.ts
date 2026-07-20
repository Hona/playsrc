import type { Tf2LoadingPresentationSnapshot } from "../../src/loading-presentation"

export const TF2_LOADING_HEADED_VIEWPORTS = Object.freeze([
  Object.freeze({ width: 1_280, height: 720 }),
  Object.freeze({ width: 390, height: 844 }),
] as const)

export type Tf2LoadingHeadedHost = Readonly<{
  mount(snapshot: Tf2LoadingPresentationSnapshot, viewport: Readonly<{ width: number; height: number }>): Promise<void>
  assertModalDialog(): Promise<void>
  capture(identity: string): Promise<Readonly<{ width: number; height: number; sha256: string }>>
  destroy(): Promise<void>
}>

export async function runTf2LoadingHeadedFixture(
  snapshots: Readonly<Record<"desktop" | "tall", Tf2LoadingPresentationSnapshot>>,
  host: Tf2LoadingHeadedHost,
) {
  const captures: Readonly<{ identity: string; width: number; height: number; sha256: string }>[] = []
  try {
    for (const [index, viewport] of TF2_LOADING_HEADED_VIEWPORTS.entries()) {
      const identity = index === 0 ? "desktop" : "tall"
      await host.mount(snapshots[identity], viewport)
      await host.assertModalDialog()
      const capture = await host.capture(`${viewport.width}x${viewport.height}-loading`)
      if (capture.width !== viewport.width || capture.height !== viewport.height || !/^[0-9a-f]{64}$/u.test(capture.sha256)) throw new Error(`Invalid loading capture ${identity}`)
      captures.push(Object.freeze({ identity: `${viewport.width}x${viewport.height}-loading`, ...capture }))
      await host.destroy()
    }
    return Object.freeze(captures)
  } finally {
    await host.destroy()
  }
}
