import { expect, test } from "bun:test"
import path from "node:path"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { parseResourceSet } from "../../../asset-store/src/graph"
import { parsePresentationArtifacts } from "../../../../games/tf2/browser/src/artifacts"
import { loadOfflineTextureOwner, offlinePipelineDevice } from "./offline-texture-owner"

test.skipIf(process.env.PLAYSRC_OFFLINE_PARTICLE_ALIAS !== "1")("same-input actual particle owners isolate construction, first binding and warm update cost", async () => {
  const config = await loadLocalConfig(), root = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement")
  const directory = path.join(root, "offline-scene"), manifest = await Bun.file(path.join(directory, "manifest.json")).json()
  const read = async (name: string) => {
    const bytes = await Bun.file(path.join(directory, name)).bytes()
    expect(new Bun.CryptoHasher("sha256").update(bytes).digest("hex")).toBe(manifest.files.find((file: any) => file.name === name).sha256)
    return bytes
  }
  const resources = parseResourceSet(await read("resources.psdb"))
  const artifacts = await parsePresentationArtifacts(await read("presentation.pspr"), resources)
  const results: any[] = []
  const modules = [await loadOfflineTextureOwner(true), await loadOfflineTextureOwner(false)]
  for (let round = 0; round < 8; round++) for (const index of round % 2 ? [1, 0] : [0, 1]) {
    const m = modules[index]!.module, device = await offlinePipelineDevice(m, true)
    const owner = new m.RendererOwner({ canvas: { width: 1280, height: 720 }, configuration: m.SOURCE_LDR, sampleCount: 1,
      textureQuality: { mipOffset: 1, trilinear: false, anisotropy: 1 } })
    const started = performance.now(), scene = owner.offlineParticleScene(device.renderer, artifacts.particleTextures, artifacts.materialStates)
    const constructMilliseconds = performance.now() - started
    const aliasMilliseconds = owner.offlineAliasMilliseconds, aliasCalls = owner.offlineAliasCalls
    const camera = { position: [0, 0, 0], yawDegrees: 0, pitchDegrees: 0, verticalFovDegrees: 75, near: 1, far: 32768 }
    const prepareStarted = performance.now(); await owner.prepareParticlePipelines(camera)
    const prepareMilliseconds = performance.now() - prepareStarted
    const view = new m.OfflineThree.PerspectiveCamera(75, 16 / 9, 1, 32768)
    const bindStarted = performance.now(); device.renderer.render(scene.particlePipelineMeshes, view)
    const firstBindMilliseconds = performance.now() - bindStarted
    const before = { writes: device.writes.length, allocations: device.allocations.length, programs: device.programs.length }
    const warm: number[] = []
    for (let frame = 0; frame < 100; frame++) {
      const start = performance.now(); device.renderer.render(scene.particlePipelineMeshes, view); warm.push(performance.now() - start)
    }
    expect(device.writes.length).toBe(before.writes)
    expect(device.allocations.length).toBe(before.allocations)
    expect(device.programs.length).toBe(before.programs)
    expect(owner.offlineAliasCalls).toBe(aliasCalls)
    const drawCalls = device.renderer.info.render.drawCalls
    const aliases = [...scene.particleTextures].map(([material, texture]: any) => ({ material, name: texture.name,
      mips: texture.mipmaps.map((mip: any) => new Bun.CryptoHasher("sha256").update(mip.data).digest("hex")) }))
    owner.offlineDispose(); device.renderer.dispose()
    expect(device.allocations.filter((value: any) => value.label?.startsWith("authored:")).every((value: any) => value.destroyed)).toBe(true)
    results.push({ round, mode: index ? "candidate" : "reference", constructMilliseconds, aliasMilliseconds, aliasCalls, prepareMilliseconds, firstBindMilliseconds, warm,
      drawCalls, aliases, programs: device.programs.map((value: any) => value.sha256).sort(),
      created: device.allocations.length, uploadedBytes: device.writes.reduce((sum: number, value: any) => sum + value.bytes, 0) })
  }
  for (let round = 0; round < 8; round++) {
    const pair = results.filter(value => value.round === round), a = pair.find(value => value.mode === "reference"), b = pair.find(value => value.mode === "candidate")
    expect(b.aliases).toEqual(a.aliases); expect(b.programs).toEqual(a.programs); expect(b.drawCalls).toBe(a.drawCalls)
    expect(b.created).toBeLessThan(a.created); expect(b.uploadedBytes).toBeLessThan(a.uploadedBytes)
  }
  await Bun.write(path.join(root, "alias-investigation/owner-cost.json"), JSON.stringify({ boundary: "actual owner and Three bindings; GPU API recorded, not GPU performance", results }, null, 2))
}, 20_000)
