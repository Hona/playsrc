import { writeFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { tf2UiResources } from "../../../games/tf2/browser/src/ui-resources"

type DependencyKind = "resource" | "font" | "material" | "texture"
type Dependency = Readonly<{ logicalPath: string; sha256: string; byteLength: number; kinds: readonly DependencyKind[] }>

const root = fileURLToPath(new URL("../../..", import.meta.url))
const output = path.join(root, "tools", "source-bundle", "tf2-ui.generated.json")
const dependencies = new Map<string, { sha256: string; byteLength: number; kinds: Set<DependencyKind> }>()

function admit(logicalPath: string, sha256: string | null, byteLength: number | null, kind: DependencyKind): void {
  if (!sha256 || byteLength === null) throw new Error(`Found TF2 UI dependency ${logicalPath} has no immutable descriptor`)
  const prior = dependencies.get(logicalPath)
  if (prior && (prior.sha256 !== sha256 || prior.byteLength !== byteLength)) {
    throw new Error(`TF2 UI dependency ${logicalPath} has conflicting immutable descriptors`)
  }
  if (prior) prior.kinds.add(kind)
  else dependencies.set(logicalPath, { sha256, byteLength, kinds: new Set([kind]) })
}

for (const source of tf2UiResources.sources) {
  if (source.outcome === "found") admit(source.logicalPath, source.sha256, source.byteLength, "resource")
}
for (const font of tf2UiResources.fonts) {
  if (font.source?.outcome === "found") admit(font.source.logicalPath, font.source.sha256, font.source.byteLength, "font")
}
for (const image of tf2UiResources.images) {
  if (image.material?.outcome === "found") admit(image.material.logicalPath, image.material.sha256, image.material.byteLength, "material")
  for (const texture of image.textures) {
    if (texture.source.outcome === "found") admit(texture.source.logicalPath, texture.source.sha256, texture.source.byteLength, "texture")
  }
}

const orderedDependencies: Dependency[] = [...dependencies]
  .sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
  .map(([logicalPath, value]) => Object.freeze({
    logicalPath,
    sha256: value.sha256,
    byteLength: value.byteLength,
    kinds: Object.freeze([...value.kinds].sort()),
  }))

const manifest = Object.freeze({
  schema: "playsrc-tf2-ui-bundle-v1",
  identity: tf2UiResources.identity,
  contentBuild: tf2UiResources.contentBuild,
  sourceLedger: tf2UiResources.sourceLedger,
  dependencies: Object.freeze(orderedDependencies),
  images: Object.freeze(tf2UiResources.images.map((image) => Object.freeze({
    identity: image.identity,
    configuredValue: image.configuredValue,
    classification: image.classification,
    material: image.material?.outcome === "found" ? image.material.logicalPath : null,
    textures: Object.freeze(image.textures.map((texture) => Object.freeze({
      logicalPath: texture.source.logicalPath,
      sha256: texture.source.sha256,
      width: texture.width,
      height: texture.height,
      frames: texture.frames,
      rawFlags: texture.rawFlags,
    }))),
  }))),
  missingDependencies: tf2UiResources.missingDependencies,
})

await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`)
console.log(JSON.stringify({
  output: "tools/source-bundle/tf2-ui.generated.json",
  descriptor: manifest.identity,
  dependencies: manifest.dependencies.length,
  images: manifest.images.length,
  missingDependencies: manifest.missingDependencies.length,
}))
