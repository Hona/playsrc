import { expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { createEvidenceDirectory } from "./evidence-directory"

test("captures use unique ignored directories under the configured cache", async () => {
  const sourceCacheDir = await mkdtemp(path.join(os.tmpdir(), "playsrc-evidence-directory-"))
  try {
    const first = await createEvidenceDirectory({ sourceCacheDir }, "vgui-runtime")
    const second = await createEvidenceDirectory({ sourceCacheDir }, "vgui-runtime")
    expect(first).not.toBe(second)
    expect(path.dirname(first)).toBe(path.join(sourceCacheDir, "evidence"))
    expect(await readFile(path.join(first, ".gitignore"), "utf8")).toBe("*\n")
    await expect(createEvidenceDirectory({ sourceCacheDir }, "../escape")).rejects.toThrow("Invalid")
  } finally {
    await rm(sourceCacheDir, { recursive: true, force: true })
  }
})
