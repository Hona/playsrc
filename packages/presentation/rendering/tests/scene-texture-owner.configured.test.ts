import { expect, test } from "bun:test"
import path from "node:path"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { parseResourceSet } from "../../../asset-store/src/graph"
import { parsePresentationArtifacts } from "../../../../games/tf2/browser/src/artifacts"
import { decodeModelPoseOutput, projectileModelPath } from "../../../../games/tf2/browser/src/presentation"
import { decodeSnapshot } from "../../../../games/tf2/browser/src/codec"
import { particleTextureAlias } from "../src/particle-texture-alias"
import { loadOfflineTextureOwner, offlineTextureDevice, offlinePipelineDevice } from "./offline-texture-owner"

test.skipIf(process.env.PLAYSRC_OFFLINE_SCENE_OWNER !== "1")("actual configured scene replacement distinguishes retained sample objects from fresh owners", async () => {
  const config = await loadLocalConfig(), root = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement")
  const directory = path.join(root, "offline-scene"), manifest = await Bun.file(path.join(directory, "manifest.json")).json()
  const read = async (name: string) => {
    const record = manifest.files.find((entry: any) => entry.name === name), bytes = await Bun.file(path.join(directory, name)).bytes()
    expect(bytes.length).toBe(record.bytes); expect(new Bun.CryptoHasher("sha256").update(bytes).digest("hex")).toBe(record.sha256)
    return bytes
  }
  const payload = await read("map.psmp"), resources = await read("resources.psdb"), presentation = await read("presentation.pspr")
  const artifacts = await parsePresentationArtifacts(presentation, parseResourceSet(resources))
  const loaded = await loadOfflineTextureOwner(), m = loaded.module
  const compile = process.env.PLAYSRC_OFFLINE_COMPILE_OWNER === "1"
  const device = compile ? await offlinePipelineDevice(m, true) : offlineTextureDevice(m), map = m.parseRuntimeMap(payload)
  const request = { resourceIdentity: manifest.graphSha256, payload, payloadSha256: manifest.files[0].sha256,
    directionalTextures: artifacts.directionalTextures, environment: artifacts.environment, materialStates: artifacts.materialStates,
    particleTextures: artifacts.particleTextures, modelOccurrences: artifacts.modelOccurrences,
    modelDrawInputs: artifacts.modelOccurrences.map(value => ({ entity: value.entity, lighting: value.lighting, eyes: value.eyes })),
    modelFacing: new Map([...artifacts.models].map(([identity, artifact]) => [identity.toLowerCase(), { frontFace: artifact.descriptor.frontFace, cullFace: artifact.descriptor.cullFace }])),
    modelMaterials: artifacts.modelMaterials, authoredTextures: artifacts.authoredTextures, brushModels: artifacts.brushModels,
    staticProps: artifacts.staticProps, diagnostic: true }
  const owner = new m.RendererOwner({ canvas: { width: 1280, height: 720 }, configuration: m.SOURCE_LDR, sampleCount: 1,
    textureQuality: { mipOffset: 1, trilinear: false, anisotropy: 1 } })
  const first = owner.offlineBuildScene(device.renderer, map, payload, request.payloadSha256, request)
  owner.offlineAdmitScene(first)
  if (compile) owner.resize(1280, 720, 1)
  // Native records are demands, not assumed construction order or aliases.
  const native = await Bun.file(path.join(root, "diagnostic-rejected/owners.json")).json()
  for (const record of native.records) if (record.kind === "create" && record.owner.startsWith("cubemap:materials/maps/")) {
    const identity = /^cubemap:(.*):frame=/.exec(record.owner)![1]!
    first.modelCubemap(identity)
  }
  const entries = (scene: any) => scene.textureResidency.offlineEntries().map(([identity, record]: any) => ({ identity, value: record.value, pinned: record.pinned }))
  const before = entries(first), beforeObjects = new Set(before.map((entry: any) => entry.value))
  const initializedBefore = new Set<any>()
  if (compile) {
    device.phase("seed-observed-prior-textures")
    const retired = new Set(native.records.filter((record: any) => record.kind === "destroy").map((record: any) => record.id))
    const sourceInputs = new Map([...artifacts.authoredTextures, ...artifacts.environment.authoredTextures])
    for (const record of native.records) if (record.kind === "create" && !retired.has(record.id)) {
      const match = /^(authored|cubemap):(.*):frame=(\d+)\/\d+:(.*)$/.exec(record.owner)
      if (!match) continue
      const [, kind, path, frameText, space] = match, frame = Number(frameText), input = sourceInputs.get(path!)
      if (!input) throw new Error(`Observed input not in configured scene: ${path}`)
      const key = kind === "cubemap" ? `cubemap:${input.sourceSha256}:${space}` : `${input.sourceSha256}:${input.sourceFormat ?? "rgba"}:${input.scalarEncoding}:${space}:${frame}`
      let value = entries(first).find((entry: any) => entry.identity === key)?.value
      if (!value && path!.includes("dx80_tfwater001_normal")) {
        const resource: any = [...first.worldMaterials.values()].find((value: any) => value.normalFrames.frameCount === 30)
        value = resource.normalFrames.select(frame, resource.normalConsumer); resource.normalNode.value = value
      }
      if (!value) throw new Error(`Observed logical texture not constructed: ${record.owner}`)
      device.renderer.initTexture(value)
    }
    device.renderer.initTexture(first.lightmapTextures[0])
    if (first.refractionTarget) device.renderer.initRenderTarget(first.refractionTarget)
    device.renderer.initRenderTarget(device.renderer._getFrameBufferTarget())
    device.backend.textureUtils.getDepthBuffer(true, false)
    for (const entry of entries(first)) if (device.backend.has(entry.value) && device.backend.get(entry.value).texture) initializedBefore.add(entry.value)
  }
  // Reproduce prepared model identities through the same authored request/pose
  // contract; do not mark unseen templates prepared merely to improve reuse.
  const poseRequest = await Bun.file(path.join(directory, "models-request.json")).json()
  const poses = decodeModelPoseOutput(await Bun.file(path.join(directory, "models-output.bin")).bytes())
  const requests = new Map(poseRequest.requests.map((value: any) => [value.request.identity, value]))
  for (const pose of poses) {
    const request: any = requests.get(pose.identity), artifact = artifacts.models.get(pose.model)!
    first.modelPipelineKeys.add(m.offlineModelKey(pose.model, request.request.skin < artifact.skinCount ? request.request.skin : 0))
  }
  for (const model of new Set([1, 2, 3, 4].map(kind => projectileModelPath(kind as 1 | 2 | 3 | 4, false)).concat(projectileModelPath(1, true)))) {
    const artifact = artifacts.models.get(model)!
    for (let skin = 0; skin < artifact.skinCount; skin++) first.modelPipelineKeys.add(m.offlineModelKey(model, skin))
  }
  const second = owner.offlineBuildScene(device.renderer, map, payload, request.payloadSha256, request)
  const after = entries(second), newSamples = after.filter((entry: any) => !beforeObjects.has(entry.value))
  const transferred = after.filter((entry: any) => beforeObjects.has(entry.value))
  const firstParticles = new Set(first.particleTextures.values())
  const unique: any[] = [], aliases: any[] = []
  for (const [material, value] of first.particleTextures) {
    const alias = particleTextureAlias(value, unique)
    if (alias) aliases.push({ material, owner: value.name, originalId: value.id, sharedId: alias.id,
      bytes: value.mipmaps.reduce((sum: number, mip: any) => sum + mip.data.byteLength, 0) })
    else unique.push(value)
  }
  expect(aliases).toHaveLength(8)
  expect(aliases.reduce((sum, alias) => sum + alias.bytes, 0)).toBe(420712)
  expect([...second.particleTextures.values()].every(texture => !firstParticles.has(texture))).toBe(true)
  expect(second.refractionTarget).not.toBe(first.refractionTarget)
  expect(second.lightmapTextures[0]).toBe(first.lightmapTextures[0])
  expect(transferred.length).toBeGreaterThan(0)
  let compilation: any = null
  if (compile) {
    const start = device.allocations.length
    device.phase("candidate-world-pipeline-warmup")
    await owner.offlineWarmScene(second, { position: [464,1456,644.03125], yawDegrees: 295, pitchDegrees: 0, verticalFovDegrees: 59.84044400898544, near: 7, far: 28377.919921875 })
    const worldEnd = device.allocations.length
    owner.offlineAdmitScene(second)
    device.phase("candidate-particle-pipeline-warmup")
    await owner.prepareParticlePipelines(poseRequest.camera)
    const particleEnd = device.allocations.length
    const prepared = poses.map(pose => {
      const preparation: any = requests.get(pose.identity), artifact = artifacts.models.get(pose.model)!, request = preparation.request
      return { pass: preparation.pass, unposedPanel: preparation.pass === "panel" && pose.role === "single", item: {
        identity: pose.identity, model: pose.model, skin: request.skin < artifact.skinCount ? request.skin : 0,
        position: request.lighting.origin, angles: request.lighting.angles, scale: 1, pose, modelLighting: pose.lighting, eyeStates: pose.eyes } }
    })
    device.phase("candidate-class-and-prop-pipeline-warmup")
    await owner.prepareModelPipelines(prepared, poseRequest.camera)
    const modelsEnd = device.allocations.length
    const visibility = m.offlineDecodeVisibility((await Bun.file(path.join(directory, "visibility-output.bin")).bytes()).buffer)
    const snapshot = decodeSnapshot(await Bun.file(path.join(directory, "initial-snapshot.bin")).bytes())
    device.phase("recorded-initial-unassigned-world-binding")
    const frame = await owner.render({ camera: poseRequest.camera, effects: [], particles: [], visibility,
      studioModels: snapshot.entityPresentation.studioModels, brushModels: snapshot.entityPresentation,
      models: prepared.filter((value: any) => value.pass === "world" && value.item.identity < 0xfffc0000).map((value: any) => value.item),
      collisionWorldIdentity: second.result.environment.collisionWorldIdentity })
    const newlyResident = [...device.allocations.slice(particleEnd, modelsEnd), ...device.allocations.slice(modelsEnd)]
      .filter((record: any) => /^(authored|cubemap):/.test(record.label))
      .map((record: any) => {
        const entry = entries(second).find((entry: any) => entry.value.name === record.label)
        expect(entry).toBeDefined(); expect(initializedBefore.has(entry.value)).toBe(false)
        return { ...record, logicalTextureId: entry.value.id, residencyIdentity: entry.identity,
          constructedBeforeReplacement: beforeObjects.has(entry.value), initializedBeforeReplacement: false,
          uploadInputs: device.writes.filter((write: any) => write.id === record.id),
          sampler: Object.fromEntries(["format", "type", "colorSpace", "channel", "internalFormat", "wrapS", "wrapT", "minFilter", "magFilter", "anisotropy", "generateMipmaps", "flipY", "premultiplyAlpha", "unpackAlignment"].map(key => [key, entry.value[key]])) }
      })
    expect(newlyResident).toHaveLength(35)
    expect(newlyResident.filter((record: any) => record.constructedBeforeReplacement)).toHaveLength(34)
    expect(newlyResident.find((record: any) => !record.constructedBeforeReplacement)?.label).toBe("cubemap:materials/maps/pl_upward/c205_520_617.vtf:frame=0/1:")
    expect(newlyResident.reduce((sum: number, record: any) => sum + record.uploadInputs.reduce((bytes: number, write: any) => bytes + write.bytes, 0), 0)).toBe(4144484)
    compilation = { newAllocations: device.allocations.slice(start), worldAllocations: device.allocations.slice(start, worldEnd),
      particleAllocations: device.allocations.slice(worldEnd, particleEnd), modelAllocations: device.allocations.slice(particleEnd, modelsEnd),
      initialBindingAllocations: device.allocations.slice(modelsEnd), initialEyeLeaf: visibility.eyeLeaf,
      newlyResident, worldMaterialInputs: [...artifacts.environment.worldMaterials],
      visibleStaticProps: frame.visibleMainStaticPropSources, visibleProjectedMarks: frame.visibleProjectedMarks,
      programs: (device as any).programs, recordedSubmissions: (device as any).recordedSubmissions(),
      executedGpuWork: false, producedPixels: false }
  }
  const record = (entry: any) => ({ identity: entry.identity, textureId: entry.value.id, owner: entry.value.name, pinned: entry.pinned,
    version: entry.value.version, format: entry.value.format, colorSpace: entry.value.colorSpace, mipCount: entry.value.mipmaps.length })
  // Without compilation this is rollback; with compilation it is terminal
  // retirement after handoff. Both use the production generation owner.
  second.disposables.dispose(); second.textureResidency.clear()
  expect(first.disposables.snapshot().state).toBe("Active")
  first.disposables.dispose(); first.textureResidency.clear()
  if (compilation) {
    compilation.terminalNewlyResidentLive = compilation.newlyResident.filter((entry: any) => !device.allocations.find((allocation: any) => allocation.id === entry.id)!.destroyed).length
    expect(compilation.terminalNewlyResidentLive).toBe(0)
    device.renderer.dispose()
  }
  await Bun.write(path.join(root, "alias-investigation/scene-construction.json"), JSON.stringify({ sourceSha256: loaded.sourceSha256, manifest,
    diagnostics: { first: first.diagnostics, second: second.diagnostics }, compilation, first: before.map(record), second: after.map(record), newSamples: newSamples.map(record),
    transferredSamples: transferred.length, aliases, particleInputs: artifacts.particleTextures.map(input => ({ material: input.material, logicalPath: input.logicalPath, sourceSha256: input.sourceSha256 })),
    interpretation: "Actual configured production scene construction, Three pipeline traversal and initial unassigned-world command-encoding path. GPU commands recorded, not executed; no browser, pixels, physical residency or FPS evidence. Historical native and newly compiled fixture identities remain separate." }, null, 2) + "\n")
}, 20_000)
