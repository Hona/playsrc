import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { descriptor } from "@playsrc/asset-store"
import { canonicalGraphBytes, resourceChunkObject } from "@playsrc/asset-store/graph"
import releaseJson from "../../../apps/web/tf2/releases/current.json"

test("clean artifact delivery authenticates its remote closure without any compiler tree and fails immediately on deterministic errors", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "playsrc-clean-artifact-"))
  try {
    const objects: Record<string, string> = {}
    const add = (template: { kind: any; mediaType: string }, bytes: Uint8Array) => {
      const value = descriptor(template.kind, template.mediaType, bytes)
      objects[value.sha256] = Buffer.from(bytes).toString("base64")
      return value
    }
    const source = releaseJson.targets[0]!
    const chunkBytes = Buffer.alloc(64, 1), chunkHash = new Bun.CryptoHasher("sha256").update(chunkBytes).digest("hex")
    const chunk = { codec: "identity" as const, encodedByteLength: "64", encodedSha256: chunkHash, decodedByteLength: "64", decodedSha256: chunkHash,
      roles: ["menu"], entries: [{ logicalPath: "materials/a.vmt", offset: "60", byteLength: "4", sha256: "1".repeat(64) }] }
    add(resourceChunkObject(chunk), chunkBytes)
    const resources = add(source.objects.resources, canonicalGraphBytes({ schema: "playsrc-resource-graph-v1", game: "tf2", contentBuild: source.contentBuild, target: source.target, chunks: [chunk] }))
    const release = { ...releaseJson, targets: [{ ...source, objects: { resources,
      bsp: add(source.objects.bsp, Buffer.from("bsp")), dependencyLedger: add(source.objects.dependencyLedger, Buffer.from("ledger")) } }], objects: {
      wasm: add(releaseJson.objects.wasm, Buffer.from([0,97,115,109,1,0,0,0])),
      catalog: add(releaseJson.objects.catalog, canonicalGraphBytes({ schema: "playsrc-resource-catalog-v1", application: "tf2", entries: [{ target: source.target, resources }] })),
    } }
    // Isolate the repository-root override in a child so other tests cannot see it.
    // The prepared descriptor is the only local input; games/ and wasm-generated/ do not exist.
    const config = path.resolve(import.meta.dir, "../src/config.ts"), deploy = path.resolve(import.meta.dir, "../src/deploy.ts")
    const maps = path.resolve(import.meta.dir, "../../../games/tf2/browser/src/maps.ts")
    const script = path.join(root, "verify.ts")
    await writeFile(script, `
      import { mock, expect } from "bun:test";
      import { existsSync } from "node:fs";
      const original = await import(${JSON.stringify(config)});
      mock.module(${JSON.stringify(config)}, () => ({ ...original, repositoryRoot: ${JSON.stringify(root)} }));
      const { release, objects } = JSON.parse(await Bun.stdin.text());
      const maps = await import(${JSON.stringify(maps)});
      mock.module(${JSON.stringify(maps)}, () => ({ ...maps, tf2MapBsp: () => release.targets[0].objects.bsp }));
      const { verifyRemoteObjects } = await import(${JSON.stringify(deploy)});
      expect(existsSync(${JSON.stringify(path.join(root, "games"))})).toBe(false);
      let calls = 0, mode = "good";
      const localError = Object.assign(new Error("missing deterministic fixture"), { code: "ENOENT" });
      const fetcher = async (url, init) => {
        calls++;
        if (mode === "local-error") throw localError;
        const sha = new URL(url).pathname.split("/").at(-1);
        if (!objects[sha]) throw new Error("unexpected object request");
        const bytes = Buffer.from(objects[sha], "base64");
        if (mode === "corrupt") bytes[0] ^= 1;
        const headers = { "content-length": String(bytes.length), etag: sha, "access-control-allow-origin": "https://playsrc.online" };
        if (mode === "metadata" && init.method === "HEAD") delete headers.etag;
        return new Response(init.method === "HEAD" ? null : bytes, { status: mode === "unavailable" ? 503 : 200, headers });
      };
      await verifyRemoteObjects(release, fetcher);
      expect(calls).toBe(11);
      for (const failure of ["local-error", "corrupt", "unavailable"]) {
        calls = 0; mode = failure;
        await expect(verifyRemoteObjects(release, fetcher)).rejects.toThrow(failure === "local-error" ? "missing deterministic fixture" : failure === "corrupt" ? "bytes differ" : "HTTP 503");
        expect(calls).toBe(1);
      }
      calls = 0; mode = "metadata";
      await expect(verifyRemoteObjects(release, fetcher)).rejects.toThrow("metadata differs");
      calls = 0;
      await expect(verifyRemoteObjects({ ...release, schema: "invalid" }, fetcher)).rejects.toThrow();
      expect(calls).toBe(0);
    `)
    const child = Bun.spawn([process.execPath, script], { cwd: root, stdin: new Blob([JSON.stringify({ release, objects })]), stdout: "pipe", stderr: "pipe" })
    const timer = setTimeout(() => child.kill(), 3_000)
    try {
      const [status, error] = await Promise.all([child.exited, new Response(child.stderr).text()])
      expect(error).toBe("")
      expect(status).toBe(0)
    } finally { clearTimeout(timer) }
  } finally { await rm(root, { recursive: true, force: true }) }
})
