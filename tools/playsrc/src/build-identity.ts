import { createHash } from "node:crypto"
import { execFile } from "node:child_process"
import { readFile } from "node:fs/promises"
import { promisify } from "node:util"
import path from "node:path"
import { repositoryRoot } from "./config"
import toolchains from "../toolchains.json" with { type: "json" }

const BUILD_INPUTS = Object.freeze([
  "Cargo.lock",
  "Cargo.toml",
  "rust-toolchain.toml",
  "tools/playsrc/toolchains.json",
  ":(glob)**/*.rs",
  ":(glob)**/Cargo.toml",
])

let pendingIdentity: Promise<string> | undefined

export async function applicationBuildIdentity(root = repositoryRoot): Promise<string> {
  const git = async (args: string[]) => (await promisify(execFile)("git", args, { cwd: root, maxBuffer: 64 * 1024 * 1024 })).stdout
  const [head, diff, untracked] = await Promise.all([
    git(["rev-parse", "HEAD"]),
    git(["diff", "--binary", "HEAD", "--", ".", ":(exclude)**/*.md", ":(exclude)*.md"]),
    git(["ls-files", "--others", "--exclude-standard", "-z"]),
  ])
  if (!/^[0-9a-f]{40}$/.test(head.trim())) throw new Error("Application source identity is unavailable")
  const names = untracked.split("\0").filter((name) => name && !name.endsWith(".md")).sort()
  const hash = createHash("sha256").update(head.trim())
  if (diff || names.length) {
    hash.update("\0playsrc-working-source-v1\0").update(diff)
    for (const name of names) hash.update("\0").update(name).update("\0").update(await readFile(path.join(root, name)))
  }
  return hash.digest("hex")
}

export function invalidateRustBuildIdentity(): void {
  pendingIdentity = undefined
}

export async function rustBuildIdentity(root = repositoryRoot): Promise<string> {
  if (root === repositoryRoot && pendingIdentity) return pendingIdentity
  const operation = (async () => {
    const child = Bun.spawn(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", ...BUILD_INPUTS], {
      cwd: root,
      stdout: "pipe",
      stderr: "pipe",
    })
    const [output, errors, status] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ])
    if (status !== 0) throw new Error(`Rust build identity failed: ${errors.trim()}`)
    const files = [...new Set(output.split("\0").filter(Boolean))].sort()
    if (files.length === 0) throw new Error("Rust build identity has no tracked source inputs")
    const contents = await Promise.all(files.map(async (file) => ({ file, bytes: await readFile(path.join(root, file)) })))
    const hash = createHash("sha256")
      .update("playsrc-rust-build-v1\0")
      .update(`${process.platform}\0${process.arch}\0${toolchains.rust.toolchain}\0${toolchains.rust.threadedToolchain}\0${toolchains.wasmBindgen.version}\0`)
    for (const { file, bytes } of contents) hash.update(file).update("\0").update(String(bytes.byteLength)).update("\0").update(bytes)
    return hash.digest("hex")
  })()
  if (root === repositoryRoot) pendingIdentity = operation.catch((error) => {
    pendingIdentity = undefined
    throw error
  })
  return operation
}

export function buildCacheDirectory(sourceCacheDir: string, identity: string): string {
  if (!/^[0-9a-f]{64}$/.test(identity)) throw new Error("Rust build cache identity is malformed")
  return path.join(sourceCacheDir, "prepared-builds", `${process.platform}-${process.arch}`, identity)
}
