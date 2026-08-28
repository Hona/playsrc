import { expect, test } from "bun:test"
import { instrumentLightmapSceneSource } from "../profile/lightmap-scene-route"
import { repositoryRoot } from "../src/config"

test("the local reference oracle changes only borrowing in the actual transpiled scene owner", async () => {
  const source = new Bun.Transpiler({ loader: "ts" }).transformSync(await Bun.file(`${repositoryRoot}/packages/presentation/rendering/src/index.ts`).text())
  const candidate = instrumentLightmapSceneSource(source, false), reference = instrumentLightmapSceneSource(source, true)
  const register = "\nglobalThis.__playsrcLightmapEvidence?.register(lightmapTextures);"
  expect(candidate.split(register)).toHaveLength(2)
  expect(candidate.replace(register, "")).toBe(source)
  expect(reference.replace("const borrowedLightmap = undefined;", "const borrowedLightmap = borrowWorldLightmapTextures(lightmap, retained?.lightmapTextures);")).toBe(candidate)
  expect(() => instrumentLightmapSceneSource("changed", false)).toThrow("checked scene owner")
  expect(() => instrumentLightmapSceneSource(source + source, true)).toThrow("checked owner")
})
