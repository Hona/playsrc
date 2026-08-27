import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import path from "node:path"
import { type LocalConfig, repositoryRoot } from "./config"
import { fileFingerprint } from "./file-fingerprint"
import type { Tf2ContentBuildContract } from "@playsrc/game-tf2-browser/content-build"
import { resolveMapTarget } from "./targets"

export async function configuredProfileIdentity(config: LocalConfig, target: string): Promise<string> {
  const content = JSON.parse(await readFile(path.join(repositoryRoot, "games/tf2/content-build.json"), "utf8")) as Tf2ContentBuildContract
  const hash = createHash("sha256").update(JSON.stringify(config)).update(target)
  const inputs = [
    ["gameinfo.txt", content.gameinfoSha256],
    ["tf2_misc_dir.vpk", content.archiveIndexes.tf2Misc],
    ["tf2_textures_dir.vpk", content.archiveIndexes.tf2Textures],
    ["tf2_sound_misc_dir.vpk", content.archiveIndexes.tf2SoundMisc],
    ["tf2_sound_vo_english_dir.vpk", content.archiveIndexes.tf2SoundVoEnglish],
  ] as const
  for (const [name, expected] of inputs) {
    const digest = await fileFingerprint(path.join(config.tf2Dir, name))
    if (digest !== expected) throw new Error(`Configured TF2 ${name} differs from its pinned content contract`)
    hash.update(name).update(digest)
  }
  if (resolveMapTarget(target).navigation === "local") hash.update(await fileFingerprint(path.join(config.tf2Dir, "maps", `${target}.nav`)))
  return hash.digest("hex")
}

export async function generatedProfileIdentity(root = repositoryRoot): Promise<string> {
  const directory = path.join(root, "games/tf2/browser/src/wasm-generated")
  const manifest = JSON.parse(await readFile(path.join(directory, ".playsrc-build.json"), "utf8"))
  if (manifest.schema !== "playsrc-threaded-wasm-build-v2" || !Array.isArray(manifest.files) || !manifest.files.length) throw new Error("Generated WASM has no verified build manifest")
  const hash = createHash("sha256").update(JSON.stringify(manifest))
  for (const file of manifest.files) {
    if (typeof file.name !== "string" || path.isAbsolute(file.name) || file.name.split(/[\\/]/u).includes("..")) throw new Error("Generated WASM manifest path is invalid")
    const digest = await fileFingerprint(path.join(directory, file.name))
    if (digest !== file.sha256) throw new Error(`Generated WASM differs from its exact build: ${file.name}`)
    hash.update(file.name).update(digest)
  }
  return hash.digest("hex")
}
