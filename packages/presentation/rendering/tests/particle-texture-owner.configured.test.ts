import { expect, test } from "bun:test"
import path from "node:path"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { loadOfflineTextureOwner, offlineTextureDevice, offlinePipelineDevice } from "./offline-texture-owner"

const configured = test.skipIf(process.env.PLAYSRC_OFFLINE_TEXTURE_OWNER !== "1")
const camera = { position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0, verticalFovDegrees: 75, near: 1, far: 32768 }

configured("offline compiler executes real Three binding/texture ownership without a canvas or submitted frame", async () => {
  const { module: m } = await loadOfflineTextureOwner(), device = await offlinePipelineDevice(m), T = device.T
  const source = new T.DataTexture(new Uint8Array(4 * 4 * 4), 4, 4)
  source.name = "offline-control"; source.needsUpdate = true
  const material = new T.MeshBasicNodeMaterial({ map: source }), scene = new T.Scene()
  const geometry = new T.PlaneGeometry(2, 2), mesh = new T.Mesh(geometry, material), camera = new T.PerspectiveCamera()
  mesh.frustumCulled = false; scene.add(mesh)
  await device.renderer.compileAsync(scene, camera)
  expect(device.allocations.some((record: any) => record.label === "offline-control")).toBe(true)
  expect(device.programs.length).toBeGreaterThan(0); expect(device.recordedSubmissions()).toBe(0)
  source.dispose(); material.dispose(); geometry.dispose(); device.renderer.dispose()
})

configured("actual particle preparation retires cold flame mips, first required binding reuploads, warm preparation preserves live backing", async () => {
  const config = await loadLocalConfig(), directory = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement/offline")
  const inputs = await Bun.file(path.join(directory, "texture-inputs.json")).json(), records = inputs.records
  const record = records.find((entry: any) => entry.logicalPath === "materials/particle/flamethrowerfire/flamethrowerfire128.vtf")
  expect(record.sourceSha256).toBe("0f121335470533adaab1ee96f3bfd7da65ab2de8026d90288d551eb7dc6ab61d")
  const effect = inputs.particleDefinitions.find((entry: any) => entry.definition === "manmelter_vacuum_flames")
  expect(effect).toMatchObject({ material: "materials/particle/flamethrowerfire/flamethrowerfire128.vmt",
    materialSha256: "160182c6e71d5cc26190f740ffc4ff30538eedbcb50b3ac343712b9c76e6e15d", pcf: "particles/drg_pyro.pcf", renderers: ["render_animated_sprites"] })
  expect(effect.textures).toEqual([{ role: "Base", path: "materials/particle/flameThrowerFire/flamethrowerfire128.vtf", colorRead: "Srgb" }])
  expect(inputs.particleDefinitions.find((entry: any) => entry.definition === "drg_manmelter_vacuum_flames").children.some((entry: any) => entry.definition === effect.definition)).toBe(true)
  const bytes = new Uint8Array(await Bun.file(record.dataPath).arrayBuffer())
  expect(new Bun.CryptoHasher("sha256").update(bytes).digest("hex")).toBe(record.dataSha256)
  const input = { ...record, planes: record.planes.map((plane: any) => ({ ...plane, rgba: bytes.subarray(plane.offset, plane.offset + plane.length) })) }
  const loaded = await loadOfflineTextureOwner(), m = loaded.module, device = offlineTextureDevice(m)
  const quality = { mipOffset: 1, trilinear: false, anisotropy: 1 }
  const texture = m.textureFromAuthored(input, device.T.SRGBColorSpace, 0, quality)
  expect(texture.mipmaps).toHaveLength(11)
  expect(texture.mipmaps.reduce((n: number, mip: any) => n + mip.data.byteLength, 0)).toBe(1398128)
  const owner = new m.RendererOwner({ canvas: { width: 1280, height: 720 }, configuration: m.SOURCE_LDR, sampleCount: 1, textureQuality: quality })
  const values = new Map([[record.logicalPath, texture]])
  owner.offlineAttach(device.renderer, values); device.compile(values)
  const stages: any[] = []
  const snapshot = (phase: string) => stages.push({ phase, logicalTextureId: texture.id, version: texture.version,
    created: device.allocations.length, live: device.allocations.filter((record: any) => !record.destroyed).length,
    uploadBytes: device.writes.reduce((n: number, record: any) => n + record.bytes, 0), writes: device.writes.length })
  device.phase("cold-preparation"); await owner.prepareParticlePipelines(camera); snapshot("cold-prepared")
  expect(device.allocations).toHaveLength(1); expect(device.allocations[0].destroyed).toBe(true)
  expect(device.backend.has(texture)).toBe(false)
  expect(texture.version).toBe(2)
  device.phase("repeated-cold-preparation"); await owner.prepareParticlePipelines(camera); snapshot("cold-reprepared")
  expect(device.allocations).toHaveLength(2); expect(device.allocations.every((record: any) => record.destroyed)).toBe(true)
  device.phase("first-required-effect-binding"); device.textures.updateTexture(texture); snapshot("first-required-binding")
  expect(device.allocations).toHaveLength(3); expect(device.allocations[2].destroyed).toBe(false)
  expect(device.writes.slice(0, 11).map((write: any) => write.sourceHash)).toEqual(device.writes.slice(22, 33).map((write: any) => write.sourceHash))
  device.phase("class-equipment-repreparation"); await owner.prepareParticlePipelines(camera); snapshot("warm-reprepared")
  expect(device.allocations).toHaveLength(3); expect(device.allocations[2].destroyed).toBe(false)
  expect(device.writes).toHaveLength(33)
  device.phase("effect-stop-or-holster"); owner.offlineClearEffects(); snapshot("effects-cleared")
  expect(device.allocations[2].destroyed).toBe(false)
  device.phase("subsequent-binding"); device.textures.updateTexture(texture); snapshot("subsequent-binding")
  expect(device.writes).toHaveLength(33)
  device.phase("map-teardown"); owner.offlineDispose(); snapshot("disposed")
  expect(device.allocations.every((record: any) => record.destroyed)).toBe(true)
  await Bun.write(path.join(directory, "flame-lifecycle.json"), JSON.stringify({ schema: "playsrc-offline-owner-lifecycle-v1",
    interpretation: "Production JS texture factory/preparation/disposal and Three texture manager/queue writer; native device/compiler boundary recorded, not executed. No browser, pixels, physical residency or FPS claim.",
    sourceSha256: loaded.sourceSha256, effect, input: { path: record.logicalPath, sourceSha256: record.sourceSha256, bytes: record.sourceBytes }, stages,
    allocations: device.allocations, writes: device.writes }, null, 2) + "\n")
})

configured("all captured cold particle owners reconcile exact formats/mips/queue spans and release only their cold backings", async () => {
  const config = await loadLocalConfig(), root = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement")
  const captured = await Bun.file(path.join(root, "diagnostic-rejected/owners.json")).json()
  const inputRecords = (await Bun.file(path.join(root, "offline/texture-inputs.json")).json()).records
  const destroyed = new Set(captured.records.filter((event: any) => event.kind === "destroy").map((event: any) => event.id))
  const cold = captured.records.filter((event: any) => event.kind === "create" && event.owner.startsWith("authored:") && destroyed.has(event.id))
  expect(cold).toHaveLength(42)
  const inputs = new Map<string, any>()
  for (const record of inputRecords) {
    const bytes = new Uint8Array(await Bun.file(record.dataPath).arrayBuffer())
    expect(new Bun.CryptoHasher("sha256").update(bytes).digest("hex")).toBe(record.dataSha256)
    inputs.set(record.logicalPath, { ...record, planes: record.planes.map((plane: any) => ({ ...plane, rgba: bytes.subarray(plane.offset, plane.offset + plane.length) })) })
  }
  const loaded = await loadOfflineTextureOwner(), m = loaded.module, device = offlineTextureDevice(m), values = new Map<string, any>()
  for (const native of cold) {
    const match = /^authored:(.*):frame=(\d+)\/\d+:(.*)$/.exec(native.owner)!
    const input = inputs.get(match[1]!)!
    const texture = m.textureFromAuthored(input, match[3], Number(match[2]), { mipOffset: 1, trilinear: false, anisotropy: 1 })
    values.set(`recorded-native-owner:${native.id}`, texture)
  }
  const owner = new m.RendererOwner({ canvas: { width: 1280, height: 720 }, configuration: m.SOURCE_LDR, sampleCount: 1 })
  owner.offlineAttach(device.renderer, values); device.compile(values); device.phase("all-cold-preparation")
  await owner.prepareParticlePipelines(camera)
  expect(device.allocations).toHaveLength(42)
  expect(device.allocations.every((record: any) => record.destroyed)).toBe(true)
  expect(device.writes.reduce((sum: number, write: any) => sum + write.bytes, 0)).toBe(3336920)
  const attributed = cold.map((native: any, index: number) => {
    const allocation = device.allocations[index], writes = device.writes.filter((write: any) => write.id === allocation.id)
    expect(allocation).toMatchObject({ label: native.owner, format: native.format, levels: native.mips, samples: native.samples,
      size: { width: native.width, height: native.height, depthOrArrayLayers: native.depth } })
    expect(writes.reduce((sum: number, write: any) => sum + write.bytes, 0)).toBe(native.bytes)
    return { nativeId: native.id, owner: native.owner, format: native.format, bytes: native.bytes, mips: native.mips,
      lifecycle: "cold pipeline preparation only; backing explicitly retired before first gameplay snapshot", allocation, writes }
  })
  // Only the selected effect's original texture is admitted again. No global
  // retention, eager admission of other effects, or change to authored samples.
  const flame = [...values.values()].find(texture => texture.name.includes("flamethrowerfire128"))
  device.phase("required-flame-binding"); device.textures.updateTexture(flame)
  expect(device.allocations.filter((record: any) => !record.destroyed)).toHaveLength(1)
  device.phase("warm-plus-cold-repreparation"); await owner.prepareParticlePipelines(camera)
  expect(device.allocations.filter((record: any) => !record.destroyed)).toHaveLength(1)
  const warm = device.allocations.find((record: any) => !record.destroyed)!
  expect(warm.id).toBe(43)
  device.phase("failed-repreparation"); device.compile(values, () => owner.offlineInvalidate())
  await expect(owner.prepareParticlePipelines(camera)).rejects.toThrow()
  expect(device.allocations.filter((record: any) => !record.destroyed)).toHaveLength(1)
  expect(warm.destroyed).toBe(false)
  device.phase("terminal-map-teardown"); owner.offlineDispose()
  expect(device.allocations.every((record: any) => record.destroyed)).toBe(true)
  await Bun.write(path.join(root, "offline/particle-owner-attribution.json"), JSON.stringify({ sourceSha256: loaded.sourceSha256,
    interpretation: "Exact configured source ranges and unchanged production factory/preparation/disposal/Three queue writer. Native GPU/compiler boundary recorded only; no pixel or physical residency claim.",
    owners: attributed, creationEvents: device.allocations.length, uploadInputBytes: device.writes.reduce((n: number, write: any) => n + write.bytes, 0), terminalLive: 0 }, null, 2) + "\n")
})

configured("configured raw cubemap owner uploads all six faces/mips once; exact handoff does not suppress a newly required cube", async () => {
  const config = await loadLocalConfig(), root = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement")
  const records = (await Bun.file(path.join(root, "offline/texture-inputs.json")).json()).records
  const loaded = await loadOfflineTextureOwner(), m = loaded.module, device = offlineTextureDevice(m)
  const input = async (path: string) => {
    const record = records.find((entry: any) => entry.logicalPath === path), data = new Uint8Array(await Bun.file(record.dataPath).arrayBuffer())
    expect(new Bun.CryptoHasher("sha256").update(data).digest("hex")).toBe(record.dataSha256)
    return { ...record, planes: record.planes.map((plane: any) => ({ ...plane, rgba: data.subarray(plane.offset, plane.offset + plane.length) })) }
  }
  const first = await input("materials/maps/pl_upward/c7168_-2048_128.vtf"), second = await input("materials/maps/pl_upward/c1678_-610_57.vtf")
  const old = new m.OwnedResourceGeneration(1, 1), a = new m.SharedTextureResidency(old)
  const key = (input: any) => `cubemap:${input.sourceSha256}:${device.T.NoColorSpace}`
  const cube = a.retain(key(first), () => m.textureFromAuthoredCubemap(first, device.T.NoColorSpace))
  old.activate(); device.phase("first-cube-binding"); device.textures.updateTexture(cube)
  expect(device.allocations[0]).toMatchObject({ format: "rgba8unorm", size: { width: 32, height: 32, depthOrArrayLayers: 6 }, levels: 6 })
  expect(device.writes).toHaveLength(36)
  expect(device.writes.reduce((sum: number, write: any) => sum + write.bytes, 0)).toBe(32760)
  const next = new m.OwnedResourceGeneration(1, 2), b = new m.SharedTextureResidency(next, 4, undefined, a)
  const retained = b.retain(key(first), () => { throw new Error("duplicate cubemap factory") })
  device.phase("same-cube-handoff"); device.textures.updateTexture(retained)
  expect(retained).toBe(cube); expect(device.allocations).toHaveLength(1)
  b.commitTransfers(); next.activate(); await old.retire(Promise.resolve()); a.clear()
  const required = b.retain(key(second), () => m.textureFromAuthoredCubemap(second, device.T.NoColorSpace))
  device.phase("new-cube-binding"); device.textures.updateTexture(required)
  expect(device.allocations).toHaveLength(2); expect(device.writes).toHaveLength(72)
  expect(device.writes.reduce((sum: number, write: any) => sum + write.bytes, 0)).toBe(65520)
  device.phase("cube-teardown"); next.dispose()
  expect(device.allocations.every((record: any) => record.destroyed)).toBe(true)
  await Bun.write(path.join(root, "offline/cubemap-lifecycle.json"), JSON.stringify({ sourceSha256: loaded.sourceSha256,
    interpretation: "Configured cube owner regression, not identification of the unrecorded historical replacement cubemap path or physical GPU residency.",
    allocations: device.allocations, writes: device.writes }, null, 2) + "\n")
})

configured("real animation residency resets consumers and releases unused old frames rather than increasing settled storage", async () => {
  const config = await loadLocalConfig(), root = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement")
  const records = (await Bun.file(path.join(root, "offline/texture-inputs.json")).json()).records
  const record = records.find((entry: any) => entry.logicalPath === "materials/water/dx80_tfwater001_normal.vtf")
  expect(record.sourceSha256).toBe("f763f3afc234f3ad6e9468dc9a98cca0e289f67810d8b6669f4cefd61cc5aea5")
  const bytes = await Bun.file(record.dataPath).bytes(), input = { ...record, planes: record.planes.map((plane: any) => ({ ...plane, rgba: bytes.subarray(plane.offset, plane.offset + plane.length) })) }
  const { module: m } = await loadOfflineTextureOwner(), device = offlineTextureDevice(m)
  const sequence = `${input.sourceSha256}:${input.sourceFormat}:${input.scalarEncoding}:`
  const create = (frame: number) => m.textureFromAuthored(input, device.T.NoColorSpace, frame, { mipOffset: 1, trilinear: false, anisotropy: 1 })
  const old = new m.OwnedResourceGeneration(1, 1), a = new m.SharedTextureResidency(old)
  const base = a.retain(`${sequence}:0`, () => create(0)); old.activate()
  device.phase("old-complete-authored-cycle")
  for (let frame = 0; frame < 30; frame++) device.textures.updateTexture(a.select(sequence, frame, "old-water", () => create(frame), 30))
  expect(device.allocations).toHaveLength(30)
  expect(device.writes.reduce((n: number, write: any) => n + write.bytes, 0)).toBe(1311120)
  const next = new m.OwnedResourceGeneration(1, 2), b = new m.SharedTextureResidency(next, 4, undefined, a)
  expect(b.selected("old-water")).toBeUndefined()
  expect(b.retain(`${sequence}:0`, () => create(0))).toBe(base)
  b.commitTransfers(); next.activate(); device.phase("retire-old-animation-owner"); await old.retire(Promise.resolve()); a.clear()
  expect(device.allocations.filter((record: any) => !record.destroyed)).toHaveLength(1)
  device.phase("new-selected-authored-frames")
  for (let frame = 1; frame <= 19; frame++) device.textures.updateTexture(b.select(sequence, frame, "new-water", () => create(frame), 30))
  expect(device.allocations).toHaveLength(49)
  expect(device.allocations.filter((record: any) => !record.destroyed)).toHaveLength(20)
  for (let frame = 1; frame <= 19; frame++) {
    const prior = device.writes.filter((write: any) => write.id === frame + 1).map((write: any) => write.sourceHash)
    const current = device.writes.filter((write: any) => write.id === frame + 30).map((write: any) => write.sourceHash)
    expect(current).toEqual(prior)
  }
  device.phase("terminal-animation-teardown"); next.dispose()
  expect(device.allocations.every((record: any) => record.destroyed)).toBe(true)
  await Bun.write(path.join(root, "offline/animation-lifecycle.json"), JSON.stringify({ source: record.logicalPath, sourceSha256: record.sourceSha256,
    frameCount: 30, mipCount: 9, bytesPerFrame: 43704, pinnedBaseFrames: 1, selectedNewNonzeroFrames: 19,
    currentSettledBytesAfterSelection: 874080, retainingAllOldFramesBytes: 1311120, indiscriminateRetentionIncrease: 437040,
    interpretation: "Representative valid19-frame selection, not invented historical frame indices. Actual production factory/residency/queue writer, native boundary recorded; no physical residency or pixel claim.",
    allocations: device.allocations }, null, 2) + "\n")
})
