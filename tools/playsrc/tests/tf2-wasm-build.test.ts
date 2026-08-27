import { expect, test } from "bun:test"
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { threadedWasmRustFlags } from "../src/tf2-wasm-build"

test("threaded WASM flags retain memory contracts and encode paths with spaces as single arguments", () => {
  const flags = threadedWasmRustFlags("/build one/app", "/build one/cargo", "/build one/rust")
  expect(flags).toContain("-Ctarget-feature=+atomics,+bulk-memory")
  expect(flags).toContain("-Clink-arg=--shared-memory")
  expect(flags).toContain("-Clink-arg=--max-memory=4294967296")
  expect(flags).toContain("--remap-path-prefix=/build one/app=/playsrc")
  expect(flags).toContain("--remap-path-prefix=/build one/cargo=/cargo")
  expect(flags).toContain("--remap-path-prefix=/build one/rust=/rust")
  expect(flags.join("\x1f").split("\x1f")).toEqual(flags)
})

test("source-location constants compile identically from distinct absolute roots", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-remap-"))
  try {
    const outputs: Buffer[] = []
    for (const name of ["short", "long checkout with spaces"]) {
      const root = path.join(directory, name)
      await mkdir(root)
      const source = path.join(root, "fixture.rs")
      await writeFile(source, '#![no_std]\n#[no_mangle]\npub static SOURCE: &str = file!();\n')
      const output = path.join(root, "fixture.ll")
      const remaps = threadedWasmRustFlags(root, path.join(root, "cargo"), path.join(root, "rust"))
        .filter(flag => flag.startsWith("--remap-path-prefix="))
      const child = Bun.spawn(["rustc", "--crate-name", "fixture", "--crate-type", "lib", "--emit=llvm-ir", ...remaps, source, "-o", output], { stdout: "pipe", stderr: "pipe" })
      const errors = await new Response(child.stderr).text()
      expect(await child.exited, errors).toBe(0)
      const bytes = await readFile(output)
      expect(bytes.toString()).toContain("/playsrc/fixture.rs")
      expect(bytes.toString()).not.toContain(root)
      outputs.push(bytes)
    }
    expect(outputs[0]).toEqual(outputs[1])
  } finally { await rm(directory, { recursive: true, force: true }) }
})
