import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"

test("a failed development owner logs its startup cause and exits nonzero after cleanup", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-owner-failure-"))
  try {
    const built = await Bun.build({ entrypoints: [path.resolve(import.meta.dir, "../src/profile-owner.ts")], target: "bun", format: "esm",
      plugins: [{ name: "failed-development-owner", setup(build) {
        build.onResolve({ filter: /^\.\/(config|dev|profile-identity)$/ }, args => ({ path: args.path, namespace: "owner-fixture" }))
        build.onLoad({ filter: /.*/, namespace: "owner-fixture" }, args => ({ loader: "js", contents: args.path === "./config"
          ? `export const repositoryRoot=${JSON.stringify(directory)}; export async function loadLocalConfig(){return {}}`
          : args.path === "./dev" ? 'export async function startDevelopment(){throw new Error("exact-startup-failure-sentinel")}'
          : 'export async function generatedProfileIdentity(){return "fixture"}' }))
      } }],
    })
    expect(built.success).toBe(true)
    const file = path.join(directory, "owner.mjs")
    await writeFile(file, await built.outputs[0]!.text())
    const child = Bun.spawn([process.execPath, file, "fixture"], { stdout: "pipe", stderr: "pipe",
      env: { ...process.env, PLAYSRC_PROFILE_OWNER_TOKEN: "fixture", PLAYSRC_PROFILE_SOURCE_IDENTITY: "fixture", PLAYSRC_PROFILE_OWNER_PATH: path.join(directory, "owner.json") } })
    const timer = setTimeout(() => child.kill(), 5000)
    try {
      expect(await child.exited).toBe(1)
      expect(await new Response(child.stderr).text()).toContain("exact-startup-failure-sentinel")
    } finally { clearTimeout(timer) }
  } finally { await rm(directory, { recursive: true, force: true }) }
})
