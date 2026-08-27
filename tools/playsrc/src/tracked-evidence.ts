import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import path from "node:path"

// Case-insensitive, including formats which are not supported by PR attachments.
export const isMediaPath = (file: string): boolean => /\.(?:png|apng|jpe?g|jpe|jfif|gif|webp|avif|svgz?|bmp|dib|tiff?|ico|icns|heic|heif|tga|psd|exr|pfm|hdr|mp4|webm|mov|m4v|avi|mkv|mpg|mpeg|m2v|wmv|flv|ogv|3gp|bik)$/i.test(file)

type Fixture = { path: string; reason: string; consumers: string[] }

export function checkMediaPaths(files: readonly string[], fixtures: readonly Fixture[]): void {
  const tracked = new Set(files)
  const allowed = new Set<string>()
  for (const fixture of fixtures) {
    if (!isMediaPath(fixture.path) || !tracked.has(fixture.path) || allowed.has(fixture.path)
      || !fixture.reason?.trim() || !fixture.consumers?.length
      || fixture.consumers.some((file) => !tracked.has(file) || isMediaPath(file))) {
      throw new Error(`Invalid authored asset/test fixture declaration: ${fixture.path}`)
    }
    allowed.add(fixture.path)
  }
  const evidence = files.filter((file) => isMediaPath(file) && !allowed.has(file))
  if (evidence.length) throw new Error(`Generated PR evidence must remain transient, not tracked:\n${evidence.join("\n")}\nOnly authored assets or required test inputs may be reviewed in media-fixtures.json with their actual consumers.`)
}

export function checkTrackedEvidence(root: string): void {
  const files = execFileSync("git", ["ls-files", "-z"], { cwd: root, encoding: "utf8" }).split("\0").filter(Boolean)
  const fixtures: Fixture[] = JSON.parse(readFileSync(path.join(root, "media-fixtures.json"), "utf8"))
  checkMediaPaths(files, fixtures)
}

if (import.meta.main) checkTrackedEvidence(path.resolve(process.argv[2] ?? "."))
