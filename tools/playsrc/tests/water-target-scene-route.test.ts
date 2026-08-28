import { expect, test } from "bun:test"
import { instrumentWaterTargetSceneSource } from "../profile/water-target-scene-route"

test("matched native reference changes only actual water attachment admission ordering", async () => {
  const source = new Bun.Transpiler({ loader: "ts", target: "browser" }).transformSync(await Bun.file(new URL("../../../packages/presentation/rendering/src/index.ts", import.meta.url)).text())
  const candidate = instrumentWaterTargetSceneSource(source, false), reference = instrumentWaterTargetSceneSource(source, true)
  expect(candidate).toBe(source)
  expect(reference.indexOf("this.#backend.initRenderTarget(scene.refractionTarget)")).toBeGreaterThan(reference.search(/async\s*#prepareWaterPipelines\(/u))
  const statements = (value: string) => value.split("\n").map(line => line.trim()).filter(Boolean).sort()
  expect(statements(reference)).toEqual(statements(candidate))
  expect(() => instrumentWaterTargetSceneSource(reference, true)).toThrow("not before")
})
