import { test, expect } from "bun:test"
import { createHash } from "node:crypto"
import { execFile } from "node:child_process"
import { mkdtemp, writeFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { promisify } from "node:util"
import { applicationBuildIdentity } from "../src/build-identity"

test("authenticates clean commits and dirty or untracked executable producer changes without hashing notes", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "playsrc-application-identity-"))
  const git = async (...args: string[]) => (await promisify(execFile)("git", args, { cwd: root })).stdout.trim()
  try {
    await git("init", "--quiet", "--initial-branch=fixture")
    await writeFile(path.join(root, "main.ts"), "export const generation = 1\n")
    await git("add", "main.ts")
    await git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture")
    const clean = await applicationBuildIdentity(root)
    expect(clean).toBe(createHash("sha256").update(await git("rev-parse", "HEAD")).digest("hex"))
    await writeFile(path.join(root, "note.md"), "No executable change\n")
    expect(await applicationBuildIdentity(root)).toBe(clean)
    await writeFile(path.join(root, "main.ts"), "export const generation = 2\n")
    const changed = await applicationBuildIdentity(root)
    expect(changed).not.toBe(clean)
    await git("add", "main.ts")
    expect(await applicationBuildIdentity(root)).toBe(changed)
    await writeFile(path.join(root, "worker.ts"), "export const generation = 3\n")
    expect(await applicationBuildIdentity(root)).not.toBe(changed)
  } finally { await rm(root, { recursive: true, force: true }) }
})
