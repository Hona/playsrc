import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "./config"
import { createCompilerParityOwner, THREE, TSL, swizzleModelTexture, createSourceWaterFogUniforms } from "../../../packages/presentation/rendering/tests/fixtures/model-compiler-parity"
import { sourceShaderGammaToLinear } from "../../../packages/presentation/rendering/src/color-output"
import { sourceTextureLayout } from "../../../packages/presentation/rendering/src/source-texture-layout"

const digest = (bytes: Uint8Array | string) => new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
const require = (value: unknown, message: string) => { if (!value) throw new Error(message) }

export async function verifyModelCompilerParity(file: string) {
  const bytes = await readFile(file), encoded = JSON.parse(bytes.toString())
  require(path.basename(file) === `${digest(bytes)}.json`, "Compiler fixture metadata digest differs")
  require(encoded.fixture.schema === "playsrc-model-compiler-parity-input-v1" && encoded.fixture.contentBuild === "24245096", "Compiler fixture identity differs")
  const arena = await readFile(path.join(path.dirname(file), encoded.arena.file))
  require(arena.length === encoded.arena.byteLength && digest(arena) === encoded.arena.sha256, "Compiler fixture arena digest differs")
  const types = { Float32Array, Uint16Array, Uint32Array, Uint8Array, Int32Array, Float64Array }
  const fixture = JSON.parse(JSON.stringify(encoded.fixture), (_, value) => {
    if (value?.bigInt !== undefined) return BigInt(value.bigInt)
    if (!value?.arrayType) return value
    const Type = types[value.arrayType as keyof typeof types]
    require(Type && Number.isSafeInteger(value.byteOffset) && value.byteOffset >= 0 && Number.isSafeInteger(value.byteLength)
      && value.byteLength >= 0 && value.byteOffset + value.byteLength <= arena.length && value.byteLength % Type.BYTES_PER_ELEMENT === 0, "Invalid compiler input array")
    return new Type(arena.buffer, arena.byteOffset + value.byteOffset, value.byteLength / Type.BYTES_PER_ELEMENT)
  })
  const materials = new Map<string, any>(fixture.materials), states = new Map<string, any>(fixture.materialStates), textures = new Map<string, any>(fixture.textures)
  const geometry = new Map<string, any>(fixture.geometry.map((model: any) => [model.logicalPath, model]))
  const headers = new Map<string, any>(fixture.models), textureObjects = new Map<string, any>(), geometryObjects = new Map<string, any>()
  const texture = (binding: any) => {
    if (!binding) return undefined
    require(binding.colorRead !== "format-dependent", "Unresolved actual texture interpretation")
    const input = textures.get(binding.logicalPath)
    require(input, `Missing texture metadata:${binding.logicalPath}`)
    const key = `${input.sourceSha256}:${binding.colorRead}`
    let object = textureObjects.get(key)
    if (!object) {
      // Device-free compiler inputs keep exact texture interpretation and shape.
      // Plane hashes/lengths are retained, but pixel storage is neither allocated
      // nor invented; this command does not claim GPU/pixel equivalence.
      const layout = sourceTextureLayout(input.sourceFormat, input.scalarEncoding)
      require(layout, `Unsupported actual texture layout:${binding.logicalPath}`)
      object = input.faces.length > 1 ? new THREE.CubeTexture() : layout!.compressed === null ? new THREE.DataTexture(null, input.width, input.height)
        : new THREE.CompressedTexture([], input.width, input.height, layout!.compressed, layout!.type)
      object.type = layout!.type; object.format = layout!.format
      object.wrapS = input.sampling.wrapS === 0 ? THREE.RepeatWrapping : THREE.ClampToEdgeWrapping
      object.wrapT = input.sampling.wrapT === 0 ? THREE.RepeatWrapping : THREE.ClampToEdgeWrapping
      object.minFilter = [THREE.NearestFilter, THREE.LinearFilter, THREE.LinearMipmapNearestFilter, THREE.LinearMipmapLinearFilter, THREE.LinearMipmapLinearFilter][input.sampling.minFilter]
      object.magFilter = input.sampling.magFilter === 0 ? THREE.NearestFilter : THREE.LinearFilter
      object.anisotropy = input.sampling.anisotropyLevel; object.generateMipmaps = false
      object.colorSpace = binding.colorRead === "srgb" ? THREE.SRGBColorSpace : THREE.NoColorSpace
      object.flipY = false
      object.name = binding.logicalPath
      textureObjects.set(key, object)
    }
    return { texture: object, sourceFormat: input.sourceFormat ?? 0 }
  }
  const passes = ["panel", "world", "view"] as const
  const counts: Record<string, number> = {}, excluded: any[] = [], summaries: any[] = []
  const started = performance.now(), deadline = started + 150000
  for (const generation of [0, 1]) {
    const owner = createCompilerParityOwner()
    let current = "initial"
    try {
      for (let team = 0; team < 2; team++) {
        const requests = new Map<number, any>(fixture.requests[team].map((entry: any) => [entry.request.identity, entry]))
        for (const pose of fixture.poses[team]) {
          const entry = requests.get(pose.identity)
          require(entry && passes.includes(entry.pass), `Unknown pose owner:${pose.model}`)
          const skin = entry.request.skin < headers.get(pose.model).skinCount ? entry.request.skin : 0
          const key = `${pose.model}${skin ? `#skin=${skin}` : ""}`, model = geometry.get(key)
          require(model, `Missing real pose geometry:${key}`)
          for (let primitive = 0; primitive < model.primitives.length; primitive++) {
            const source = model.primitives[primitive], material = materials.get(model.materials[source.material].logicalPath.toLowerCase())
            const state = states.get(material.identity)
            const label = `${team + 2}:${entry.pass}:${key}:${primitive}:${material.identity}`
            current = label
            if (!["vertex-lit-generic", "eyes", "eye-refract"].includes(material.shader) || state.noDraw) {
              if (generation === 0) excluded.push({ label, shader: material.shader, reason: state.noDraw ? "authored-no-draw" : "unchanged-unlit/refract-owner" })
              continue
            }
            const eye = pose.eyes.find((eye: any) => eye.primitive === primitive)
            if ((material.shader === "eyes" || material.shader === "eye-refract") && !eye) {
              if (generation === 0) excluded.push({ label, shader: material.shader, reason: "inactive-eye-primitive-not-in-current-pose" })
              continue
            }
            require(performance.now() < deadline, "Compiler acceptance exceeded150seconds; split exact profile cases")
            const getTexture = (kind: string, role: number) => texture(material.bindings.find((binding: any) => binding.kind === kind && binding.role === role))
            const base = getTexture("material", 0), iris = getTexture("model", 8)
            const environment = getTexture("material", 12)
            require(base || iris, `No actual base/iris:${label}`)
            const baseNode = base ? swizzleModelTexture(TSL.texture(base.texture, TSL.uv()), base.sourceFormat) : TSL.vec4(1)
            const input = { shader: material.shader, state: material.state, fragment: state, base: baseNode, baseTexture: base,
              textures: { iris: iris?.texture, warp: getTexture("model", 6)?.texture, exponent: getTexture("model", 5)?.texture, ambientOcclusion: getTexture("model", 10)?.texture },
              environment: environment && material.environmentMap ? { texture: environment.texture, tint: material.environmentMap.tint.map(sourceShaderGammaToLinear), scale: material.shader === "eye-refract" ? 1 : fixture.profile === 1 ? 16 : 1 } : undefined,
              exposure: TSL.float(1), waterFog: entry.pass === "panel" ? undefined : createSourceWaterFogUniforms() }
            const modes = entry.pass === "panel" && pose.role === "single" ? [false, true] : [true]
            for (const skinned of modes) {
              const geometryKey = `${key}:${primitive}:${skinned}`
              let buffer = geometryObjects.get(geometryKey)
              if (!buffer) {
                buffer = new THREE.BufferGeometry()
                buffer.setAttribute("position", new THREE.BufferAttribute(skinned ? source.bindPositions : source.positions, 3))
                buffer.setAttribute("normal", new THREE.BufferAttribute(skinned ? source.bindNormals : source.normals, 3))
                buffer.setAttribute("uv", new THREE.BufferAttribute(source.uv, 2)); buffer.setIndex(new THREE.BufferAttribute(source.indices, 1))
                if (skinned) {
                  buffer.setAttribute("tangent", new THREE.BufferAttribute(source.bindTangents, 4))
                  buffer.setAttribute("skinIndex", new THREE.BufferAttribute(source.boneIndices, 4))
                  buffer.setAttribute("skinWeight", new THREE.BufferAttribute(source.boneWeights, 4))
                }
                geometryObjects.set(geometryKey, buffer)
              }
              const bones = new Float32Array(source.bonePalette.length * 12)
              source.bonePalette.forEach((bone: number, index: number) => bones.set(pose.boneMatrices.subarray(bone * 12, (bone + 1) * 12), index * 12))
              owner.admit(label, buffer, entry.pass, input as any, { lighting: pose.lighting, eye, bones }, skinned)
              if (generation === 0) counts[entry.pass] = (counts[entry.pass] ?? 0) + 1
            }
          }
        }
      }
      const lifetime = owner.verifyLifetime()
      summaries.push({ generation, ...lifetime, records: owner.records.map(record => ({ ...record,
        vertexShader: digest(record.vertexShader), fragmentShader: digest(record.fragmentShader) })) })
    } catch (error) {
      await writeFile(path.join(path.dirname(file), "compiler-failure.json"), JSON.stringify({ input: path.basename(file), current, generation, completed: owner.records.length, failure: String(error), last: owner.records.at(-1) }, null, 2))
      throw error
    }
  }
  for (const value of geometryObjects.values()) value.dispose()
  for (const value of textureObjects.values()) value.dispose()
  if (JSON.stringify(summaries[0].records) !== JSON.stringify(summaries[1].records)) {
    const differences = summaries[0].records.flatMap((record: any, index: number) => JSON.stringify(record) !== JSON.stringify(summaries[1].records[index])
      ? [{ before: record, after: summaries[1].records[index] }] : [])
    await writeFile(path.join(path.dirname(file), "compiler-replacement-failure.json"), JSON.stringify(differences, null, 2))
    throw new Error(`Replacement/device-owner programs differ:${differences.length}`)
  }
  const report = { input: path.basename(file), provenance: fixture.provenance, contentBuild: fixture.contentBuild,
    profile: fixture.profile, counts, excluded, summaries, milliseconds: performance.now() - started, pixelsVerified: false }
  const output = path.join(path.dirname(file), `${digest(JSON.stringify(report))}.compiler-parity.json`)
  await writeFile(output, JSON.stringify(report, null, 2))
  return { output, counts, excluded: excluded.length, summaries: summaries.map(({ records, ...summary }) => summary), milliseconds: report.milliseconds }
}

if (import.meta.main) {
  const [file] = process.argv.slice(2), local = await loadLocalConfig()
  require(file && path.resolve(file).startsWith(path.resolve(local.sourceCacheDir) + path.sep) && process.argv.length === 3, "Usage: verify-model-compiler-parity.ts <configured-cache fixture.json>")
  console.log(JSON.stringify(await verifyModelCompilerParity(file!)))
}
