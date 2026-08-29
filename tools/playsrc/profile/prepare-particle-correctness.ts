import path from "node:path"
import { mkdir, readFile } from "node:fs/promises"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { parseResourceSet } from "../../../packages/asset-store/src/graph"
import { parsePresentationArtifacts } from "../../../games/tf2/browser/src/artifacts"
import { buildParticleCorrectnessBundle } from "../../../packages/presentation/rendering/tests/offline-texture-owner"

const { sourceCacheDir } = await loadLocalConfig()
const base = path.join(sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement"), input = path.join(base, "offline-scene"), output = path.join(base, "current-format/native-particles")
await mkdir(output, { recursive: true })
const manifest = JSON.parse(await readFile(path.join(input, "manifest.json"), "utf8"))
const load = async (name: string) => {
  const bytes = await Bun.file(path.join(input, name)).bytes()
  if (new Bun.CryptoHasher("sha256").update(bytes).digest("hex") !== manifest.files.find((file: any) => file.name === name)?.sha256) throw new Error("Current producer fixture identity differs")
  return bytes
}
const artifacts = await parsePresentationArtifacts(await load("presentation.pspr"), parseResourceSet(await load("resources.psdb")))
const names = new Set(artifacts.particleTextures.map(texture => texture.material.toLowerCase()))
const ids = new Map<object, number>(), views: { type: string; data: string }[] = []
const encoded = JSON.stringify({ textures: artifacts.particleTextures, states: [...artifacts.materialStates].filter(([name]) => names.has(name)) }, (_key, value) => {
  if (!ArrayBuffer.isView(value)) return value
  let id = ids.get(value)
  if (id === undefined) { id = views.length; ids.set(value, id); views.push({ type: value.constructor.name, data: Buffer.from(value.buffer, value.byteOffset, value.byteLength).toString("base64") }) }
  return { $view: id }
})
await Bun.write(path.join(output, "inputs.json"), JSON.stringify({ manifest, views, encoded }))
const built = await buildParticleCorrectnessBundle(path.join(repositoryRoot, "packages/presentation/rendering/tests/particle-correctness.browser.ts"), path.join(output, "fixture.js"))
console.log(JSON.stringify({ directory: output, logicalMaterials: names.size, canonicalViews: views.length, sourceSha256: built.sourceSha256, graphSha256: manifest.graphSha256 }))
