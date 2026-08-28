import { expect, test } from "bun:test"
import { instrumentParticleAliasSource } from "../profile/particle-alias-route"

test("particle alias evidence registers the real normalized material owner and changes only alias selection in reference mode", async () => {
  const source = await Bun.file(new URL("../../../packages/presentation/rendering/src/index.ts", import.meta.url)).text()
  const compiled = new Bun.Transpiler({ loader: "ts" }).transformSync(source)
  const candidate = instrumentParticleAliasSource(compiled, false)
  expect(candidate).toContain("globalThis.__playsrcParticleAliasEvidence?.register")
  expect(candidate).toContain("particleTextureAlias(candidate, textures.values())")
  expect(instrumentParticleAliasSource(compiled, true)).not.toContain("particleTextureAlias(candidate, textures.values())")
  expect(instrumentParticleAliasSource(compiled, true, false)).not.toContain("__playsrcParticleAliasEvidence")
  expect(() => instrumentParticleAliasSource("", false)).toThrow("owner route differs")
})
