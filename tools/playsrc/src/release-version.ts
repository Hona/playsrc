/** Minor releases require an explicitly selected workflow increment. */
export function validateReleaseVersion(version: string, latest: string, tags: readonly string[], increment = "patch"): void {
  const pattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/
  if (!pattern.test(version) || !pattern.test(latest)) throw new Error("Expected numeric versions without a v prefix")
  if (increment !== "patch" && increment !== "minor") throw new Error("Expected patch or minor increment")
  const [major, minor, patch] = latest.split(".").map(BigInt)
  const expected = increment === "minor" ? `${major}.${minor + 1n}.0` : `${major}.${minor}.${patch + 1n}`
  if (version !== expected) throw new Error(`next ${increment} release must be ${expected}`)
  if (tags.includes(`v${version}`)) throw new Error(`v${version} already exists`)
  for (const tag of tags) {
    if (!tag.startsWith("v") || !pattern.test(tag.slice(1))) continue
    const parts = tag.slice(1).split(".").map(BigInt)
    const base = [major, minor, patch]
    const difference = parts.findIndex((part, index) => part !== base[index])
    if (difference >= 0 && parts[difference] > base[difference]) throw new Error("A version tag is newer than the latest published release")
  }
}

if (import.meta.main) validateReleaseVersion(process.argv[2], process.argv[3], (await Bun.stdin.text()).trim().split(/\s+/), process.argv[4])
