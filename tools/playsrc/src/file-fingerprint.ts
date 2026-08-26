import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"

// Cache bytes, never the decision that an input is unchanged. ctime catches
// same-size edits even when an editor restores mtime; inode catches replacement.
const digests = new Map<string, { stamp: string; digest: string }>()
export async function fileFingerprint(filename: string): Promise<string> {
  const stamp = async () => {
    const value = await stat(filename, { bigint: true })
    if (!value.isFile()) throw new Error(`Fingerprint input is not a file: ${filename}`)
    return `${value.dev}:${value.ino}:${value.size}:${value.mtimeNs}:${value.ctimeNs}`
  }
  const before = await stamp()
  const cached = digests.get(filename)
  if (cached?.stamp === before) return cached.digest
  const digest = createHash("sha256").update(await readFile(filename)).digest("hex")
  if (await stamp() !== before) throw new Error(`Fingerprint input changed while being read: ${filename}`)
  digests.set(filename, { stamp: before, digest })
  return digest
}
