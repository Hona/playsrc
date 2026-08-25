import { describe, expect, test } from "bun:test"
import { CanvasFrameDiagnostics } from "../src/frame-diagnostics"
import type { FrameResult } from "@playsrc/rendering"

type DiagnosticFrame = Pick<FrameResult, "visibleMainStaticPropSources" | "sky3dPass" | "runtimeStaticPropScreen">

function target() {
  const values: Record<string, string> = {}
  const writes: string[] = []
  const dataset = new Proxy(values, {
    set(object, key, value: string) {
      object[key as string] = value
      writes.push(`${String(key)}=${value}`)
      return true
    },
  }) as DOMStringMap
  return { target: { dataset }, values, writes }
}

const sky = (): NonNullable<FrameResult["sky3dPass"]> => Object.freeze({
  phases: Object.freeze(["sky3d", "depth-reset", "main", "restore"]) as readonly ["sky3d", "depth-reset", "main", "restore"],
  skySurfaces: 548,
  skyProps: 60,
  mainProps: 1184,
  visibleSkyPropSources: Object.freeze([1, 3, 5]),
  fog: Object.freeze({ start: 32, end: 512, primary: Object.freeze([1, 2, 3, 255]) as readonly [number, number, number, number] }),
  stateRestored: true,
})

const frame = (): DiagnosticFrame => ({
  visibleMainStaticPropSources: Object.freeze([2, 4, 8]),
  sky3dPass: sky(),
  runtimeStaticPropScreen: Object.freeze([Object.freeze({ source: 4, x: 10, y: 20, width: 30, height: 40 })]),
})

describe("application canvas frame observations", () => {
  test("publishes exact main, authored-sky, and runtime-prop values once per changed identity", () => {
    const diagnostics = new CanvasFrameDiagnostics()
    const value = target()
    diagnostics.publish(value.target, frame())
    expect(value.values.visibleMainStaticProps).toBe("[2,4,8]")
    expect(JSON.parse(value.values.sky3dPass!)).toMatchObject({ skySurfaces: 548, skyProps: 60 })
    expect(value.values.runtimeStaticPropScreen).toBe('[{"source":4,"x":10,"y":20,"width":30,"height":40}]')
    expect(value.writes).toHaveLength(3)

    diagnostics.publish(value.target, frame())
    expect(value.writes).toHaveLength(3)
    diagnostics.publish(value.target, { ...frame(), visibleMainStaticPropSources: Object.freeze([2, 8]) })
    expect(value.writes).toHaveLength(4)
    expect(value.values.visibleMainStaticProps).toBe("[2,8]")
  })

  test("retains empty values, invalidates exact projected coordinates, and resets across generations", () => {
    const diagnostics = new CanvasFrameDiagnostics()
    const value = target()
    const empty: DiagnosticFrame = { visibleMainStaticPropSources: [], runtimeStaticPropScreen: [] }
    diagnostics.publish(value.target, empty)
    expect(value.values).toEqual({ visibleMainStaticProps: "[]", sky3dPass: "", runtimeStaticPropScreen: "[]" })
    diagnostics.publish(value.target, empty)
    expect(value.writes).toHaveLength(3)
    diagnostics.publish(value.target, frame())
    const writes = value.writes.length
    diagnostics.publish(value.target, {
      ...frame(),
      runtimeStaticPropScreen: [{ source: 4, x: 10.5, y: 20, width: 30, height: 40 }],
    })
    expect(value.writes).toHaveLength(writes + 1)
    diagnostics.clear()
    diagnostics.publish(value.target, frame())
    expect(value.writes).toHaveLength(writes + 4)
  })
})
