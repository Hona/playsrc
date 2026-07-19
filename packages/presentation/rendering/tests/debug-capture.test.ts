import { describe, expect, test } from "bun:test"
import {
  compareDebugPlanes,
  createDebugCapture,
  validateAlignedCaptureManifest,
  type AlignedCaptureManifest,
  type DebugPlaneInput,
} from "../src/debug-capture"

const bytes = (values: Float32Array | Uint32Array) => new Uint8Array(values.buffer.slice(0))

function inputs(): readonly DebugPlaneInput[] {
  return Object.freeze([
    { kind: "color", components: 4, scalar: "u8", bytes: new Uint8Array([10, 20, 30, 255]) },
    { kind: "depth", components: 1, scalar: "f32", bytes: bytes(new Float32Array([0.5])) },
    { kind: "normal", components: 3, scalar: "f32", bytes: bytes(new Float32Array([0, 0, 1])) },
    { kind: "material-id", components: 1, scalar: "u32", bytes: bytes(new Uint32Array([7])) },
    { kind: "primitive-id", components: 1, scalar: "u32", bytes: bytes(new Uint32Array([8])) },
    { kind: "object-id", components: 1, scalar: "u32", bytes: bytes(new Uint32Array([9])) },
  ])
}

describe("debug plane output", () => {
  test("hashes the complete aligned color/depth/normal/material/primitive/object plane set", async () => {
    const capture = await createDebugCapture({
      identity: "frame:4", frameOrdinal: 4, sceneIdentity: "scene:2", width: 1, height: 1, planes: inputs(),
    })
    expect([...capture.planes.keys()]).toEqual(["color", "depth", "normal", "material-id", "primitive-id", "object-id"])
    expect([...capture.planes.values()].every((plane) => /^[0-9a-f]{64}$/.test(plane.sha256))).toBe(true)
    await expect(createDebugCapture({
      identity: "bad", frameOrdinal: 1, sceneIdentity: "scene", width: 1, height: 1, planes: inputs().slice(0, 5),
    })).rejects.toThrow(/invalid/i)
  })

  test("compares numeric and identity planes through their declared predicates", async () => {
    const target = await createDebugCapture({ identity: "a", frameOrdinal: 1, sceneIdentity: "s", width: 1, height: 1, planes: inputs() })
    const changed = inputs().map((plane) => plane.kind === "color"
      ? { ...plane, bytes: new Uint8Array([11, 20, 30, 255]) }
      : plane.kind === "depth" ? { ...plane, bytes: bytes(new Float32Array([0.55])) }
      : plane.kind === "normal" ? { ...plane, bytes: bytes(new Float32Array([0, 0.1, 0.995])) }
      : plane.kind === "object-id" ? { ...plane, bytes: bytes(new Uint32Array([10])) }
      : plane) as readonly DebugPlaneInput[]
    const browser = await createDebugCapture({ identity: "b", frameOrdinal: 1, sceneIdentity: "s", width: 1, height: 1, planes: changed })
    expect(compareDebugPlanes(target.planes.get("color")!, browser.planes.get("color")!, { kind: "color", maximumAbsolute: 1 / 255, meanAbsolute: 1 / 1020 }).passed).toBe(true)
    expect(compareDebugPlanes(target.planes.get("depth")!, browser.planes.get("depth")!, { kind: "depth", maximumAbsolute: 0.04 }).passed).toBe(false)
    expect(compareDebugPlanes(target.planes.get("normal")!, browser.planes.get("normal")!, { kind: "normal", minimumDot: 0.99 }).passed).toBe(true)
    expect(compareDebugPlanes(target.planes.get("object-id")!, browser.planes.get("object-id")!, { kind: "object-id", exact: true })).toMatchObject({ passed: false, mismatchedValues: 1 })
  })
})

test("aligned manifest validation rejects absent identities, planes, and tolerances", async () => {
  const capture = await createDebugCapture({ identity: "a", frameOrdinal: 1, sceneIdentity: "s", width: 1, height: 1, planes: inputs() })
  const side = Object.freeze({
    runtime: "fixed", operatingSystem: "fixed", gpu: "fixed", driver: "fixed",
    planes: Object.freeze([...capture.planes.values()].map((plane) => Object.freeze({ kind: plane.kind, sha256: plane.sha256 }))),
  })
  const manifest: AlignedCaptureManifest = Object.freeze({
    version: 1, identity: "capture:1", contentBuild: "24207079", mapLogicalPath: "maps/jump_beef.bsp",
    mapSha256: "1".repeat(64), authoritySnapshot: "tick:100", presentationTimestamp: 1.5,
    camera: Object.freeze({ position: [1,2,3], angles: [4,5,6], projection: Object.freeze([1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]) }),
    viewport: [320,180], lightingProfile: "hdr", exposure: 1, assetClosureSha256: "2".repeat(64),
    target: side, browser: side,
    comparisons: Object.freeze([
      { kind: "color", maximumAbsolute: 0.01, meanAbsolute: 0.001 },
      { kind: "depth", maximumAbsolute: 0.001 },
      { kind: "normal", minimumDot: 0.99 },
      { kind: "material-id", exact: true }, { kind: "primitive-id", exact: true }, { kind: "object-id", exact: true },
    ]),
  })
  expect(validateAlignedCaptureManifest(manifest)).toBe(manifest)
  expect(() => validateAlignedCaptureManifest({ ...manifest, mapSha256: "missing" })).toThrow(/invalid/i)
  expect(() => validateAlignedCaptureManifest({ ...manifest, comparisons: manifest.comparisons.slice(0, 5) })).toThrow(/invalid/i)
})
