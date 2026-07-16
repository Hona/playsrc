import { readFile, stat } from "node:fs/promises"
import path from "node:path"
import { AssetStoreError, objectPath, readChannel } from "@playsrc/asset-store"

const HASH = /^[0-9a-f]{64}$/
const IMMUTABLE_CACHE = "public, max-age=31536000, immutable, no-transform"
const CHANNEL_CACHE = "public, no-cache, must-revalidate, no-transform"

function problem(status: number, title: string): Response {
  return Response.json({ type: "about:blank", title, status }, { status, headers: { "cache-control": "no-store" } })
}

function parseRange(value: string | null, length: number): { start: number; end: number } | null | "malformed" | "unsatisfied" {
  if (!value) return null
  const match = /^bytes=(\d*)-(\d*)$/.exec(value)
  if (!match || (match[1] === "" && match[2] === "")) return "malformed"
  if (length === 0) return "unsatisfied"
  let start: number
  let end: number
  if (match[1] === "") {
    const suffix = Number(match[2])
    if (!Number.isSafeInteger(suffix) || suffix <= 0) return "malformed"
    start = Math.max(0, length - suffix)
    end = length - 1
  } else {
    start = Number(match[1])
    end = match[2] === "" ? length - 1 : Number(match[2])
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start) return "malformed"
    if (start >= length) return "unsatisfied"
    end = Math.min(end, length - 1)
  }
  return { start, end }
}

const digestField = (hash: string): string => `sha-256=:${Buffer.from(hash, "hex").toString("base64")}:`

export async function assetResponse(request: Request, assetDir: string): Promise<Response> {
  const url = new URL(request.url)
  if (url.search || url.hash) return problem(400, "Malformed route")
  if (url.pathname === "/readyz") return new Response(request.method === "HEAD" ? null : "ready", { headers: { "cache-control": "no-store" } })
  if (!new Set(["GET", "HEAD", "OPTIONS"]).has(request.method)) return new Response(null, { status: 405, headers: { allow: "GET, HEAD, OPTIONS", "cache-control": "no-store" } })
  if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: { allow: "GET, HEAD, OPTIONS", "cache-control": "no-store", "access-control-allow-methods": "GET, HEAD, OPTIONS", "access-control-allow-origin": "*" } })

  const objectMatch = /^\/objects\/sha256\/([^/]+)$/.exec(url.pathname)
  if (objectMatch) {
    const hash = objectMatch[1]
    if (!HASH.test(hash)) return problem(400, "Malformed object identity")
    try {
      const pathname = objectPath(assetDir, hash)
      const metadata = await stat(pathname)
      if (!metadata.isFile()) return problem(404, "Object not found")
      const bytes = new Uint8Array(await readFile(pathname))
      const actual = new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
      if (actual !== hash) return problem(500, "Object integrity failure")
      const etag = `"${hash}"`
      if (request.headers.get("if-none-match") === etag) return new Response(null, { status: 304, headers: { etag, "cache-control": IMMUTABLE_CACHE } })
      const range = request.method === "GET" ? parseRange(request.headers.get("range"), bytes.byteLength) : null
      if (range === "malformed") return problem(400, "Malformed range")
      if (range === "unsatisfied") return new Response(null, { status: 416, headers: { "content-range": `bytes */${bytes.byteLength}`, "cache-control": "no-store" } })
      const selected = range ? bytes.slice(range.start, range.end + 1) : bytes
      const headers: Record<string, string> = { "content-type": "application/octet-stream", "content-length": String(selected.byteLength), "accept-ranges": "bytes", etag, "repr-digest": digestField(hash), "cache-control": IMMUTABLE_CACHE, "access-control-allow-origin": "*" }
      if (range) headers["content-range"] = `bytes ${range.start}-${range.end}/${bytes.byteLength}`
      if (request.method === "GET") headers["content-digest"] = digestField(new Bun.CryptoHasher("sha256").update(selected).digest("hex"))
      return new Response(request.method === "HEAD" ? null : selected, { status: range ? 206 : 200, headers })
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return problem(404, "Object not found")
      return problem(500, "Object read failure")
    }
  }

  const channelMatch = /^\/channels\/([^/]+)$/.exec(url.pathname)
  if (channelMatch) {
    try {
      const channel = await readChannel(assetDir, channelMatch[1])
      const headers = { "content-type": "application/vnd.playsrc.asset-channel+json", "content-length": String(channel.bytes.byteLength), etag: `"${channel.revision}"`, "cache-control": CHANNEL_CACHE, "access-control-allow-origin": "*" }
      return new Response(request.method === "HEAD" ? null : channel.bytes, { headers })
    } catch (error) {
      if (error instanceof AssetStoreError && error.code === "MalformedIdentity") return problem(400, "Malformed channel identity")
      if (error instanceof AssetStoreError && error.code === "MissingObject") return problem(404, "Channel not found")
      return problem(500, "Channel read failure")
    }
  }
  return problem(404, "Resource not found")
}

export function startAssetService(assetDir: string, port = 4173): ReturnType<typeof Bun.serve> {
  return Bun.serve({ hostname: "127.0.0.1", port, fetch: (request) => assetResponse(request, assetDir) })
}
