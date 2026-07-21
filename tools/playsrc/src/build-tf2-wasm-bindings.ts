import { stat } from "node:fs/promises"
import path from "node:path"
import { buildTf2Wasm, buildThreadedTf2Wasm } from "./tf2-wasm-build"
import { loadLocalConfig, repositoryRoot } from "./config"

async function exists(pathname: string): Promise<boolean> {
  try {
    return (await stat(pathname)).isFile()
  } catch {
    return false
  }
}

const localConfiguration = path.join(repositoryRoot, "playsrc.local.json")
const output = await (await exists(localConfiguration)
  ? buildTf2Wasm(await loadLocalConfig(), true)
  : buildThreadedTf2Wasm("cargo", "wasm-bindgen", process.env))
console.log(output)
