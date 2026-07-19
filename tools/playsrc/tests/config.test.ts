import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { ConfigurationError, loadLocalConfig, repositoryRoot } from "../src/config"

const testRoots: string[] = []

afterEach(async () => {
  await Promise.all(testRoots.splice(0).map((root) => rm(root, { force: true, recursive: true })))
})

async function fixture(config?: unknown): Promise<string> {
  const root = await mkdtemp(path.join(os.tmpdir(), "playsrc-config-"))
  testRoots.push(root)
  const roots = {
    tf2Dir: path.join(root, "tf2"),
    sourceCacheDir: path.join(root, "source-cache"),
    assetDir: path.join(root, "assets"),
  }
  await Promise.all(Object.values(roots).map((directory) => mkdir(directory)))
  if (config !== undefined) {
    await writeFile(path.join(root, "playsrc.local.json"), JSON.stringify(config === true ? roots : config))
  }
  return root
}

async function expectConfigurationError(
  root: string,
  code: ConfigurationError["code"],
  context = "configuration",
): Promise<void> {
  try {
    await loadLocalConfig(root)
    throw new Error("configuration unexpectedly loaded")
  } catch (error) {
    expect(error).toBeInstanceOf(ConfigurationError)
    const actual = (error as ConfigurationError).code
    if (actual !== code) throw new Error(`${context}: expected ${code}, received ${actual}`)
  }
}

describe("local configuration", () => {
  test("resolves the checked repository configuration independently of cwd", async () => {
    const previous = process.cwd()
    process.chdir(os.tmpdir())
    try {
      const config = await loadLocalConfig()
      expect(repositoryRoot).toBe(path.resolve(import.meta.dir, "../../..") + path.sep)
      expect(path.basename(config.tf2Dir).toLowerCase()).toBe("tf")
      expect(Object.isFrozen(config)).toBe(true)
    } finally {
      process.chdir(previous)
    }
  })

  test("accepts exactly three accessible absolute non-nested roots", async () => {
    const root = await fixture(true)
    const config = await loadLocalConfig(root)
    expect(Object.keys(config)).toEqual(["tf2Dir", "sourceCacheDir", "assetDir"])
  })

  test("rejects missing configuration", async () => {
    await expectConfigurationError(await fixture(), "ConfigurationMissing")
  })

  test("rejects malformed UTF-8 and JSON", async () => {
    const root = await fixture()
    await writeFile(path.join(root, "playsrc.local.json"), Uint8Array.of(0xff))
    await expectConfigurationError(root, "ConfigurationMalformed")
  })

  test("rejects missing, extra, non-string, empty, and relative fields", async () => {
    const root = await fixture()
    const invalid = [
      {},
      { tf2Dir: "/a", sourceCacheDir: "/b", assetDir: "/c", extra: "/d" },
      { tf2Dir: 1, sourceCacheDir: "/b", assetDir: "/c" },
      { tf2Dir: "", sourceCacheDir: "/b", assetDir: "/c" },
      { tf2Dir: "relative", sourceCacheDir: "/b", assetDir: "/c" },
    ]
    for (const [index, value] of invalid.entries()) {
      await writeFile(path.join(root, "playsrc.local.json"), JSON.stringify(value))
      await expectConfigurationError(root, "ConfigurationMalformed", `invalid case ${index}`)
    }
  })

  test("rejects missing and non-directory roots", async () => {
    const root = await fixture()
    const file = path.join(root, "not-a-directory")
    await writeFile(file, "x")
    await writeFile(
      path.join(root, "playsrc.local.json"),
      JSON.stringify({
        tf2Dir: file,
        sourceCacheDir: path.join(root, "missing"),
        assetDir: path.join(root, "assets"),
      }),
    )
    await expectConfigurationError(root, "ConfiguredRootUnavailable")
  })

  test("rejects equal and nested roots after canonicalization", async () => {
    const root = await fixture()
    const outer = path.join(root, "outer")
    const inner = path.join(outer, "inner")
    const other = path.join(root, "other")
    await mkdir(inner, { recursive: true })
    await mkdir(other)

    await writeFile(
      path.join(root, "playsrc.local.json"),
      JSON.stringify({ tf2Dir: outer, sourceCacheDir: outer, assetDir: other }),
    )
    await expectConfigurationError(root, "ConfigurationMalformed")

    await writeFile(
      path.join(root, "playsrc.local.json"),
      JSON.stringify({ tf2Dir: outer, sourceCacheDir: inner, assetDir: other }),
    )
    await expectConfigurationError(root, "ConfigurationMalformed")
  })

  test("setup creates only the two configured writable roots", async () => {
    const root = await fixture()
    const tf2Dir = path.join(root, "tf2")
    const sourceCacheDir = path.join(root, "new", "source-cache")
    const assetDir = path.join(root, "new", "assets")
    await writeFile(
      path.join(root, "playsrc.local.json"),
      JSON.stringify({ tf2Dir, sourceCacheDir, assetDir }),
    )

    const config = await loadLocalConfig(root, "setup")
    expect(config.sourceCacheDir).toBe(await realpath(sourceCacheDir))
    expect(config.assetDir).toBe(await realpath(assetDir))
    await loadLocalConfig(root, "setup")
  })
})
