import { expect, test } from "bun:test"
import path from "node:path"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { parseResourceSet } from "../../../asset-store/src/graph"
import { parsePresentationArtifacts } from "../../../../games/tf2/browser/src/artifacts"
import { decodeParticleRenderOutput } from "../../particle/src"
import { tf2MapBsp } from "../../../../games/tf2/browser/src/maps"
import { loadOfflineTextureOwner, offlinePipelineDevice } from "./offline-texture-owner"

for (const target of ["koth_sawmill", "koth_lakeside_final"] as const) test.skipIf(process.env.PLAYSRC_OFFLINE_KOTH !== "1")(`${target} authored particle values, compiler layouts and create/retire/reentry ownership`, async () => {
  const referenceMode = process.env.PLAYSRC_OFFLINE_KOTH_REFERENCE === "1"
  const config = await loadLocalConfig(), directory = path.join(config.sourceCacheDir, "evidence/koth-sustained-offline", target)
  const manifest = await Bun.file(path.join(directory, "manifest.json")).json()
  expect(manifest.target).toBe(target); expect(manifest.bspSha256).toBe(tf2MapBsp(target).sha256)
  const read = async (name: string) => {
    const expected = manifest.files.find((file: any) => file.name === name), bytes = await Bun.file(path.join(directory, name)).bytes()
    expect(bytes.byteLength).toBe(expected.bytes)
    expect(new Bun.CryptoHasher("sha256").update(bytes).digest("hex")).toBe(expected.sha256)
    return bytes
  }
  const resources = parseResourceSet(await read("resources.psdb"))
  const artifacts = await parsePresentationArtifacts(await read("presentation.pspr"), resources)
  const frames = await Promise.all(manifest.files.filter((file: any) => /^frame-\d+\.bin$/.test(file.name)).map(async (file: any) => ({
    name: file.name, items: decodeParticleRenderOutput(await read(file.name), artifacts.particleMaterials).items,
  })))
  expect(frames).toHaveLength(5)
  if (target === "koth_sawmill") expect(frames.some(frame => frame.items.length > 0)).toBe(true)
  for (const sampleCount of [1, 4] as const) {
  const loaded = await loadOfflineTextureOwner(), m = loaded.module, device = await offlinePipelineDevice(m, false, sampleCount)
  const owner = new m.RendererOwner({ canvas: { width: 1280, height: 720 }, configuration: m.SOURCE_LDR, sampleCount,
    textureQuality: { mipOffset: 0, trilinear: true, anisotropy: 1 } })
  const scene = owner.offlineParticleScene(device.renderer, artifacts.particleTextures, artifacts.materialStates)
  const camera = { position: [0, 0, 0], yawDegrees: 90, pitchDegrees: 5, verticalFovDegrees: 75, near: 1, far: 30000 }
  await owner.prepareParticlePipelines(camera)
  const geometryState = (entries: any[]) => entries.map(({ key, capacity, mesh }) => ({ key, capacity, visible: mesh.visible,
    drawRange: { ...mesh.geometry.drawRange }, renderOrder: mesh.renderOrder,
    attributes: Object.entries(mesh.geometry.attributes).map(([name, attribute]: any) => ({ name, size: attribute.itemSize,
      sha256: new Bun.CryptoHasher("sha256").update(new Uint8Array(attribute.array.buffer, attribute.array.byteOffset, attribute.array.byteLength)).digest("hex") })),
    indices: new Bun.CryptoHasher("sha256").update(new Uint8Array(mesh.geometry.index.array.buffer)).digest("hex") }))
  const materials = [...scene.particleBatchMaterials].map(([key, material]: any) => ({ key, side: material.side, depthTest: material.depthTest,
    depthWrite: material.depthWrite, depthFunc: material.depthFunc, blendSrc: material.blendSrc, blendDst: material.blendDst, transparent: material.transparent }))
  const samples: any[] = []
  let created = 0, destroyed = 0
  const seen = new Set<object>()
  const observe = (entries: any[]) => { for (const { mesh } of entries) if (!seen.has(mesh.geometry)) {
    seen.add(mesh.geometry); created++; mesh.geometry.addEventListener("dispose", () => destroyed++)
  } }
  for (const frame of frames) {
    const start = performance.now(), entries = owner.offlineStageParticles(frame.items, camera)
    const firstMilliseconds = performance.now() - start
    observe(entries)
    const first = geometryState(entries), createdBefore = created
    const warm: number[] = []
    for (let iteration = 0; iteration < 40; iteration++) {
      const started = performance.now(); const current = owner.offlineStageParticles(frame.items, camera); warm.push(performance.now() - started)
      observe(current)
    }
    expect(created).toBe(createdBefore)
    expect(geometryState(entries)).toEqual(first)
    owner.offlineStageParticles([], camera)
    expect(entries.every((entry: any) => !entry.mesh.visible)).toBe(true)
    const reentry = owner.offlineStageParticles(frame.items, camera); observe(reentry)
    expect(geometryState(reentry)).toEqual(first)
    expect(created).toBe(createdBefore)
    samples.push({ name: frame.name, particles: frame.items.length, materials: [...new Set(frame.items.map((item: any) => item.material))],
      batches: entries.length, firstMilliseconds, warm, created, destroyed, geometry: first })
  }
  const programs = device.programs.map((value: any) => value.sha256).sort()
  owner.offlineClearEffects(); expect(destroyed).toBe(created)
  // No retained geometry or image from the prior generation may survive final disposal.
  owner.offlineDispose()
  if (referenceMode) device.renderer.dispose()
  else m.disposeWebGpuBackend(device.renderer)
  const outstanding = device.allocations.filter((value: any) => !value.destroyed)
  const gpuApi = { created: device.allocations.length, uploadCalls: device.writes.length,
    uploadSourceBytes: device.writes.reduce((sum: number, write: any) => sum + write.bytes, 0),
    outstandingColorBytes: outstanding.filter((value: any) => value.format === "rgba16float")
      .reduce((sum: number, value: any) => sum + value.size.width * value.size.height * value.size.depthOrArrayLayers * value.samples * 8, 0),
    outstandingOpaqueDepthTextures: outstanding.filter((value: any) => value.format === "depth24plus").length }
  await Bun.write(path.join(directory, `owner-${referenceMode ? "reference" : "candidate"}-msaa${sampleCount}.json`), JSON.stringify({ target, sampleCount, sourceSha256: loaded.sourceSha256, graphSha256: manifest.graphSha256,
    samples, created, destroyed, programs, materials, outstanding, gpuApi, scope: "Bun CPU microcost and recorded API/compiler ownership only. Fixed native inputs are not recorded combat. No browser, GPU execution, pixels, physical residency, GC or sustained frame/input acceptance." }))
  if (referenceMode) {
    expect(outstanding.some((value: any) => value.format === "rgba16float")).toBe(true)
    expect(outstanding.some((value: any) => value.format === "depth24plus")).toBe(true)
  } else {
    expect(outstanding).toEqual([])
    // Separate processes preserve the same Three global declaration sequence,
    // rather than normalizing away differences in generated shader programs.
    const referenceFile = Bun.file(path.join(directory, `owner-reference-msaa${sampleCount}.json`))
    const reference = await referenceFile.json()
    expect(reference.graphSha256).toBe(manifest.graphSha256)
    expect(programs).toEqual(reference.programs)
    expect(materials).toEqual(reference.materials)
    expect(samples.map(({ geometry }) => geometry)).toEqual(reference.samples.map(({ geometry }: any) => geometry))
    expect([created, destroyed]).toEqual([reference.created, reference.destroyed])
    expect([gpuApi.created, gpuApi.uploadCalls, gpuApi.uploadSourceBytes]).toEqual([reference.gpuApi.created, reference.gpuApi.uploadCalls, reference.gpuApi.uploadSourceBytes])
  }
  }
}, 30_000)
