import path from "node:path"
import { repositoryRoot } from "./config"

const manifests = [
  "Cargo.toml",
  "packages/runtime/simulation/rust/Cargo.toml",
  "packages/runtime/replay/rust/Cargo.toml",
  "packages/runtime/networking/rust/Cargo.toml",
  "packages/formats/demo/rust/Cargo.toml",
  "games/tf2/rulesets/jump/rust/Cargo.toml",
  "games/tf2/browser/src/ui-resources/generator/Cargo.toml",
  "games/tf2/rust/inventory-generator/Cargo.toml",
  "packages/formats/studio-model/scripts/evidence-runner/Cargo.toml",
] as const

for (const manifest of manifests) {
  const child = Bun.spawn([
    "cargo",
    "+1.97.1",
    "test",
    "--locked",
    "--manifest-path",
    path.join(repositoryRoot, manifest),
  ], {
    cwd: repositoryRoot,
    stdout: "inherit",
    stderr: "inherit",
  })
  if (await child.exited !== 0) throw new Error(`Rust tests failed for ${manifest}`)
}
