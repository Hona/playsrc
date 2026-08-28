import { test, expect } from "bun:test"
import { mkdtemp, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { createHash } from "node:crypto"
import { readRetainedWasmManifest } from "../profile/exact-wasm-replay"

test("retained closures authenticate historical files without current audio requirements", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "playsrc-retained-wasm-"))
  try {
    const files = []
    for (const name of ["tf2_wasm.js", "tf2_wasm_bg.wasm"]) {
      const bytes = Buffer.from(name)
      await writeFile(path.join(directory, name), bytes)
      files.push({ name, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") })
    }
    const manifest = { schema: "playsrc-threaded-wasm-build-v2", identity: "a".repeat(64), files }
    const save = () => writeFile(path.join(directory, ".playsrc-build.json"), JSON.stringify(manifest))
    await save()
    expect(await readRetainedWasmManifest(directory)).toEqual(manifest)
    await writeFile(path.join(directory, "tf2_wasm.js"), "changed")
    await expect(readRetainedWasmManifest(directory)).rejects.toThrow("bytes changed")
    await writeFile(path.join(directory, "tf2_wasm.js"), "tf2_wasm.js")
    files.push({ ...files[0]! })
    await save()
    await expect(readRetainedWasmManifest(directory)).rejects.toThrow("descriptor")
    files.pop()
    for (const name of ["../escape", "C:\\escape", "/escape", "a//b"]) {
      files.push({ ...files[0]!, name }); await save()
      await expect(readRetainedWasmManifest(directory)).rejects.toThrow("descriptor")
      files.pop()
    }
  } finally { await rm(directory, { recursive: true, force: true }) }
})
