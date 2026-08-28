import { describe, expect, test } from "bun:test"
import { CloudflareError, createR2Adapter, publishImmutableObject, publicationObjectByteLimit, sortPublicationDescriptors, type RemoteObjectAdapter } from "../src/cloudflare"
import type { ObjectDescriptor } from "@playsrc/asset-store"

describe("Cloudflare publication output", () => {
  const bytes = new TextEncoder().encode("object")
  const descriptor: ObjectDescriptor = Object.freeze({
    kind: "derived-object",
    mediaType: "application/octet-stream",
    byteLength: "6",
    sha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
  })

  function adapter(initial: Uint8Array | "Missing", create: "Created" | "PreconditionFailed" = "Created", persistCreate = true) {
    let remote = initial
    const calls: string[] = []
    const value: RemoteObjectAdapter = {
      async read() { calls.push("read"); return remote },
      async create(_key, _expected, input) { calls.push("create"); if (persistCreate) remote = input; return create },
      close() { calls.push("close") },
    }
    return { value, calls }
  }

  test("uploads only a missing object and verifies exact readback", async () => {
    const remote = adapter("Missing")
    expect(await publishImmutableObject(descriptor, bytes, remote.value)).toBe("Uploaded")
    expect(remote.calls).toEqual(["read", "create", "read"])
  })

  test("performs zero writes for a warm exact object", async () => {
    const remote = adapter(bytes)
    expect(await publishImmutableObject(descriptor, bytes, remote.value)).toBe("AlreadyPresent")
    expect(remote.calls).toEqual(["read"])
  })

  test("accepts a concurrent conditional-create winner only after readback", async () => {
    const remote = adapter("Missing", "PreconditionFailed")
    expect(await publishImmutableObject(descriptor, bytes, remote.value)).toBe("AlreadyPresent")
    expect(remote.calls).toEqual(["read", "create", "read"])
  })

  test("rejects conflicts, missing readback, and absent credentials", async () => {
    const corrupt = adapter(new TextEncoder().encode("Object"))
    await expect(publishImmutableObject(descriptor, bytes, corrupt.value)).rejects.toBeInstanceOf(CloudflareError)
    const missing = adapter("Missing", "Created", false)
    await expect(publishImmutableObject(descriptor, bytes, missing.value)).rejects.toBeInstanceOf(CloudflareError)
    expect(() => createR2Adapter({})).toThrow("R2 conditional publication credentials are unavailable")
  })

  test("orders leaves before roots and catalogs deterministically", () => {
    const value = (kind: ObjectDescriptor["kind"], digit: string): ObjectDescriptor => ({ kind, mediaType: "x", byteLength: "1", sha256: digit.repeat(64) })
    expect(sortPublicationDescriptors([
      value("catalog", "1"),
      value("source-root", "2"),
      value("derived-object", "4"),
      value("source-object", "3"),
    ]).map((item) => item.kind)).toEqual(["source-object", "derived-object", "source-root", "catalog"])
  })

  for (const corrupt of [false, true]) test(`publication progress counts verified bytes and clears its timer on ${corrupt ? "failure" : "success"}`, async () => {
    // Confine transport/filesystem mocks to a subprocess; no real R2 requests.
    const script = `
      import { mock } from "bun:test";
      import assert from "node:assert/strict";
      import { mkdtemp, rm } from "node:fs/promises";
      import { tmpdir } from "node:os";
      import path from "node:path";
      const payloads = [Buffer.alloc(1048576, 1), Buffer.alloc(1048576, 2)];
      const descriptors = payloads.map(bytes => ({ kind: "derived-object", mediaType: "application/octet-stream", byteLength: String(bytes.length), sha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex") })).sort((a,b) => a.sha256.localeCompare(b.sha256));
      const bytes = new Map(payloads.map(value => [new Bun.CryptoHasher("sha256").update(value).digest("hex"), value]));
      const directory = await mkdtemp(path.join(tmpdir(), "playsrc-publication-progress-"));
      for (const [hash, value] of bytes) await Bun.write(path.join(directory, hash), value);
      const stored = new Map([[descriptors[1].sha256, bytes.get(descriptors[1].sha256)]]);
      let tick, cleared = false, closed = false, writes = 0;
      globalThis.setInterval = (callback, milliseconds) => { assert.equal(milliseconds, 1000); tick = callback; return 1; };
      globalThis.clearInterval = timer => { assert.equal(timer, 1); cleared = true; };
      Bun.spawn = () => ({ stdout: new Blob(["{}"]).stream(), stderr: new Blob([]).stream(), exited: Promise.resolve(0) });
      mock.module(${JSON.stringify(import.meta.resolve("../src/tf2-release"))}, () => ({
        prepareTf2Release: async () => ({ release: { defaultTarget: "fixture", targets: [{ target: "fixture" }] }, files: new Map(descriptors.map(descriptor => [descriptor.sha256, { descriptor }])) }),
        releaseObjectPath: (_config, descriptor) => path.join(directory, descriptor.sha256), verifyFile: async () => {},
      }));
      class S3ServiceException extends Error { constructor() { super("missing"); this.name = "NoSuchKey"; this.$metadata = { httpStatusCode: 404 }; } }
      class GetObjectCommand { constructor(input) { this.input = input; } }
      class PutObjectCommand { constructor(input) { this.input = input; this.middlewareStack = { add() {} }; } }
      mock.module("@aws-sdk/client-s3", () => ({ S3ServiceException, GetObjectCommand, PutObjectCommand, S3Client: class {
        async send(command) {
          tick();
          const key = command.input.Key.split("/").at(-1);
          if (command instanceof PutObjectCommand) { writes++; stored.set(key, command.input.Body); return {}; }
          if (!stored.has(key)) throw new S3ServiceException();
          return { Body: { transformToByteArray: async () => ${corrupt} && key === descriptors[1].sha256 ? Buffer.from("corrupt") : stored.get(key) } };
        }
        destroy() { closed = true; }
      } }));
      const { publishTf2Release } = await import(${JSON.stringify(import.meta.resolve("../src/cloudflare"))});
      let failed = false;
      try { await publishTf2Release({}, undefined); } catch(error) { failed = true; assert.match(error.message, /differs/); }
      finally { await rm(directory, { recursive: true, force: true }); }
      assert.equal(failed, ${corrupt}); assert.equal(cleared, true); assert.equal(closed, true); assert.equal(writes, 1);
    `
    const child = Bun.spawn([process.execPath, "-e", script], {
      env: { ...process.env, CLOUDFLARE_ACCOUNT_ID: "a".repeat(32), AWS_ACCESS_KEY_ID: "fixture", AWS_SECRET_ACCESS_KEY: "fixture" },
      stdout: "pipe", stderr: "pipe",
    })
    const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
    if (exit !== 0) throw new Error(stderr)
    expect(exit).toBe(0)
    const lines = stderr.trim().split("\n")
    expect(lines[0]).toContain("publishing 0.0% | verified 0.00/2.00 MiB | objects 0/2")
    expect(lines.filter(line => line.includes("publishing 0.0%"))).toHaveLength(4)
    expect(lines).toContainEqual(expect.stringContaining("publishing 50.0% | verified 1.00/2.00 MiB | objects 1/2 | uploaded 1 | already present 0"))
    if (corrupt) {
      expect(lines.at(-1)).toContain("failed 50.0%")
      expect(stderr).not.toContain("100.0%")
      expect(stdout).toBe("")
    } else {
      expect(lines.at(-1)).toContain("complete 100.0% | verified 2.00/2.00 MiB | objects 2/2 | uploaded 1 | already present 1")
      expect(JSON.parse(stdout).totals).toEqual({ objects: 2, bytes: 2097152, uploaded: 1, alreadyPresent: 1 })
    }
  })

  test("large exact source objects do not relax the derived-object or root guards",async()=>{
    expect(publicationObjectByteLimit("source-object")).toBe(128*1024*1024)
    for(const kind of ["derived-object","source-root","derived-root","catalog"] as const)expect(publicationObjectByteLimit(kind)).toBe(64*1024*1024)
    const bytes=new Uint8Array(64*1024*1024+1)
    bytes[0]=86;bytes[bytes.length-1]=80
    const source:ObjectDescriptor={kind:"source-object",mediaType:"application/octet-stream",byteLength:String(bytes.length),sha256:new Bun.CryptoHasher("sha256").update(bytes).digest("hex")}
    const local=adapter("Missing")
    expect(await publishImmutableObject(source,bytes,local.value)).toBe("Uploaded")
    expect(local.calls).toEqual(["read","create","read"])
    const refused=adapter("Missing")
    await expect(publishImmutableObject({...source,kind:"derived-object"},bytes,refused.value)).rejects.toThrow("67108864-byte")
    await expect(publishImmutableObject(source,new Uint8Array(128*1024*1024+1),refused.value)).rejects.toThrow("134217728-byte")
    expect(refused.calls).toEqual([])
  })
})
