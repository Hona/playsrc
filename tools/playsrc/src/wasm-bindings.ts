import { createHash } from "node:crypto"
import { readdir, readFile } from "node:fs/promises"
import path from "node:path"
import { parseWasmBindings, type WasmBindingFile } from "../../../apps/web/tf2/src/deployment"
import type { ObjectDescriptor } from "@playsrc/asset-store"

const digest = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")

async function javascriptFiles(directory: string, prefix = ""): Promise<string[]> {
  const files: string[] = []
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) throw new Error("WASM binding closure cannot contain symbolic links")
    const name = prefix + entry.name
    if (entry.isDirectory()) files.push(...await javascriptFiles(path.join(directory, entry.name), name + "/"))
    else if (entry.isFile() && name.endsWith(".js")) files.push(name)
  }
  return files.sort()
}

async function describeBindings(directory: string): Promise<readonly WasmBindingFile[]> {
  return parseWasmBindings(await Promise.all((await javascriptFiles(directory)).map(async name => {
    const bytes = await readFile(path.join(directory, name))
    return { name, byteLength: String(bytes.byteLength), sha256: digest(bytes) }
  })))
}

/** Capture together with the approved binary, never from an unrelated host build. */
export async function captureWasmBindings(directory: string, wasm: ObjectDescriptor): Promise<readonly WasmBindingFile[]> {
  const bytes = await readFile(path.join(directory, "tf2_wasm_bg.wasm"))
  if (String(bytes.byteLength) !== wasm.byteLength || digest(bytes) !== wasm.sha256) throw new Error("WASM binding producer differs from the approved binary")
  return describeBindings(directory)
}

/** Identical generated glue (including every helper) may consume its approved
 * binary across hosts. Matching export names alone cannot establish this pair. */
export async function assertWasmBindings(directory: string, approved: readonly WasmBindingFile[]): Promise<void> {
  if (JSON.stringify(await describeBindings(directory)) !== JSON.stringify(parseWasmBindings(approved))) {
    throw new Error("Generated JavaScript binding closure differs from the approved WASM producer")
  }
}
