import { join } from "node:path"
import { simulationWorkflow } from "./config"

const { root, cargo } = await simulationWorkflow()
const manifest = join(import.meta.dir, "../rust/wasm-smoke/Cargo.toml")
const child = Bun.spawn(
  [cargo, "build", "--quiet", "--release", "--target", "wasm32-unknown-unknown", "--manifest-path", manifest],
  { cwd: root, stdin: "inherit", stdout: "inherit", stderr: "inherit" },
)
const status = await child.exited
if (status !== 0) throw new Error(`Bare-WASM simulation build failed with status ${status}`)

const wasm = join(import.meta.dir, "../rust/target/wasm32-unknown-unknown/release/playsrc_simulation_wasm_smoke.wasm")
const module = await WebAssembly.compile(await Bun.file(wasm).arrayBuffer())
const imports = WebAssembly.Module.imports(module)
if (imports.length !== 0) throw new Error(`Bare-WASM simulation unexpectedly imports ${imports.length} values`)
const instance = await WebAssembly.instantiate(module, {})
const smoke = instance.exports.simulation_wasm_smoke
if (typeof smoke !== "function") throw new Error("Bare-WASM simulation smoke export is missing")
const result = smoke()
if (result !== 1) throw new Error(`Bare-WASM simulation smoke returned ${result}`)
