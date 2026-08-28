import { expect, test } from "bun:test"
import path from "node:path"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { parseResourceSet } from "../../../asset-store/src/graph"
import { parsePresentationArtifacts } from "../../../../games/tf2/browser/src/artifacts"
import { loadOfflineTextureOwner, offlinePipelineDevice } from "./offline-texture-owner"

test.skipIf(process.env.PLAYSRC_OFFLINE_PARTICLE_ALIAS !== "1")("exact particle alias reuse preserves programs/materials and never increases live or peak backing", async () => {
  const config = await loadLocalConfig(), root = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement")
  const directory = path.join(root, "offline-scene"), manifest = await Bun.file(path.join(directory, "manifest.json")).json()
  const read = async (name: string) => {
    const expected = manifest.files.find((file: any) => file.name === name), data = await Bun.file(path.join(directory, name)).bytes()
    expect(new Bun.CryptoHasher("sha256").update(data).digest("hex")).toBe(expected.sha256)
    return data
  }
  const payload = await read("map.psmp"), resources = parseResourceSet(await read("resources.psdb"))
  const artifacts = await parsePresentationArtifacts(await read("presentation.pspr"), resources)
  const request = { resourceIdentity: manifest.graphSha256, payload, payloadSha256: manifest.files[0].sha256,
    directionalTextures: artifacts.directionalTextures, environment: artifacts.environment, materialStates: artifacts.materialStates,
    particleTextures: artifacts.particleTextures, modelOccurrences: artifacts.modelOccurrences,
    modelDrawInputs: artifacts.modelOccurrences.map(value => ({ entity: value.entity, lighting: value.lighting, eyes: value.eyes })),
    modelFacing: new Map([...artifacts.models].map(([identity, artifact]) => [identity.toLowerCase(), { frontFace: artifact.descriptor.frontFace, cullFace: artifact.descriptor.cullFace }])),
    modelMaterials: artifacts.modelMaterials, authoredTextures: artifacts.authoredTextures, brushModels: artifacts.brushModels, staticProps: artifacts.staticProps, diagnostic: true }
  const modules = [await loadOfflineTextureOwner(true), await loadOfflineTextureOwner(false)]
  const results: any[] = []
  for (const [index, loaded] of modules.entries()) {
    const m = loaded.module, device = await offlinePipelineDevice(m), map = m.parseRuntimeMap(payload)
    const owner = new m.RendererOwner({ canvas: { width: 1280, height: 720 }, configuration: m.SOURCE_LDR, sampleCount: 1,
      textureQuality: { mipOffset: 1, trilinear: false, anisotropy: 1 } })
    const scene = owner.offlineBuildScene(device.renderer, map, payload, request.payloadSha256, request); owner.offlineAdmitScene(scene)
    const values = [...scene.particleTextures.values()] as any[], unique = [...new Set(values)]
    const materials = [...scene.particleBatchMaterials].map(([key, material]: any) => ({ key, side: material.side, blendSrc: material.blendSrc, blendDst: material.blendDst,
      alphaTest: material.alphaTest, depthTest: material.depthTest, depthWrite: material.depthWrite, forceSinglePass: material.forceSinglePass }))
    const aliases = [...scene.particleTextures].map(([material, texture]: any) => ({ material, name: texture.name, textureId: texture.id,
      mips: texture.mipmaps.map((mip: any) => ({ width: mip.width, height: mip.height, sha256: new Bun.CryptoHasher("sha256").update(mip.data).digest("hex") })) }))
    const camera = { position: [0,0,0], yawDegrees: 0, pitchDegrees: 0, verticalFovDegrees: 75, near: 1, far: 32768 }
    device.phase("cold-preparation"); await owner.prepareParticlePipelines(camera)
    const cold = device.allocations.filter((record: any) => record.label?.startsWith("authored:")), firstWrites = device.writes.filter((write: any) => cold.some((allocation: any) => allocation.id === write.id))
    expect(cold.every((record: any) => record.destroyed)).toBe(true)
    expect(unique.every(texture => texture.version === 2)).toBe(true)
    const programs = device.programs.map((program: any) => program.sha256).sort()
    const firstUse = values.find(texture => texture.name.includes("effects/softglow.vtf"))
    device.phase("first-effect-binding"); device.renderer.initTexture(firstUse)
    const live = () => device.allocations.filter((record: any) => record.label?.startsWith("authored:") && !record.destroyed)
    expect(live()).toHaveLength(1)
    device.phase("mixed-warm-preparation"); await owner.prepareParticlePipelines(camera)
    expect(live()).toHaveLength(1)
    const compile = device.renderer.compileAsync.bind(device.renderer)
    device.renderer.compileAsync = async (...args: any[]) => { await compile(...args); owner.offlineInvalidate() }
    device.phase("cancelled-preparation"); await expect(owner.prepareParticlePipelines(camera)).rejects.toThrow()
    expect(live()).toHaveLength(1)
    device.phase("effect-stop"); owner.offlineClearEffects(); expect(live()).toHaveLength(1)
    // New map owner always constructs independent textures, even on this device.
    const replacement = owner.offlineBuildScene(device.renderer, map, payload, request.payloadSha256, request)
    expect([...replacement.particleTextures.values()].every(texture => !unique.includes(texture))).toBe(true)
    replacement.disposables.dispose(); scene.disposables.dispose(); expect(live()).toHaveLength(0)
    device.renderer.dispose()
    const imageBytes = new Map(unique.map(texture => [texture.name, texture.mipmaps.reduce((sum: number, mip: any) => sum + mip.data.byteLength, 0)]))
    const phases: Record<string, { peakBytes: number; endBytes: number }> = {}
    let liveBytes = 0
    for (const event of device.textureEvents) {
      const bytes = imageBytes.get(event.label)
      if (bytes === undefined) continue
      const state = phases[event.phase] ??= { peakBytes: liveBytes, endBytes: liveBytes }
      liveBytes += event.kind === "create" ? bytes : -bytes
      expect(liveBytes).toBeGreaterThanOrEqual(0)
      state.peakBytes = Math.max(state.peakBytes, liveBytes); state.endBytes = liveBytes
    }
    expect(liveBytes).toBe(0)
    results.push({ mode: index ? "candidate" : "reference", sourceSha256: loaded.sourceSha256, logicalMaterials: values.length,
      uniqueImages: unique.length, coldCreated: cold.length, coldUploadBytes: firstWrites.reduce((sum: number, write: any) => sum + write.bytes, 0),
      coldLiveAfterPreparation: 0, liveAfterFirstBinding: 1, liveAfterCancellation: 1, terminalLive: 0, phases, materials, aliases, programs, allocations: device.allocations })
  }
  const [reference, candidate] = results
  expect(reference.logicalMaterials).toBe(42); expect(candidate.logicalMaterials).toBe(42)
  expect(reference.uniqueImages).toBe(42); expect(candidate.uniqueImages).toBe(34)
  expect(reference.coldCreated).toBe(42); expect(candidate.coldCreated).toBe(34)
  expect(reference.coldUploadBytes).toBe(3336920); expect(candidate.coldUploadBytes).toBe(2916208)
  expect(candidate.materials).toEqual(reference.materials)
  expect(candidate.aliases.map(({ textureId, ...value }: any) => value)).toEqual(reference.aliases.map(({ textureId, ...value }: any) => value))
  expect(candidate.programs).toEqual(reference.programs)
  expect(reference.phases["cold-preparation"].peakBytes).toBe(3336920)
  expect(candidate.phases["cold-preparation"].peakBytes).toBe(2916208)
  for (const [phase, state] of Object.entries(candidate.phases) as [string, any][]) {
    expect(state.peakBytes).toBeLessThanOrEqual(reference.phases[phase].peakBytes)
    expect(state.endBytes).toBeLessThanOrEqual(reference.phases[phase].endBytes)
  }
  await Bun.write(path.join(root, "alias-investigation/lifecycle-comparison.json"), JSON.stringify({ nativeBoundary: "recorded only, no GPU execution/pixels/performance acceptance", results }, null, 2) + "\n")
}, 20_000)
