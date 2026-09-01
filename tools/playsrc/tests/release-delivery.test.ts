import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import releaseJson from "../../../apps/web/tf2/releases/current.json"

test("clean-artifact delivery needs no compiler tree or asset rescan, and local failures stop before production", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "playsrc-clean-delivery-"))
  try {
    const source = path.resolve(import.meta.dir, "../src"), profile = path.resolve(import.meta.dir, "../profile")
    const script = path.join(root, "verify.ts")
    // Isolated production-boundary mocks exercise real delivery control flow and static-tree checks.
    // The mocked startup receipt is a unit fixture, never browser evidence or deployment approval.
    await writeFile(script, `
      import { mock, expect } from "bun:test";
      import { mkdir, writeFile, rm } from "node:fs/promises";
      import { existsSync } from "node:fs";
      const source = ${JSON.stringify(source)}, profile = ${JSON.stringify(profile)}, root = ${JSON.stringify(root)};
      const config = await import(source + "/config.ts");
      mock.module(source + "/config.ts", () => ({ ...config, repositoryRoot: root }));
      const { parseTf2Release, createDeployedBrowserConfiguration } = await import(${JSON.stringify(path.resolve(import.meta.dir, "../../../apps/web/tf2/src/deployment.ts"))});
      const release = parseTf2Release(JSON.parse(await Bun.stdin.text()));
      const build = "a".repeat(64), sha = "b".repeat(64), configuration = createDeployedBrowserConfiguration(release, build);
      const releaseModule = await import(source + "/tf2-release.ts");
      mock.module(source + "/tf2-release.ts", () => ({ ...releaseModule, readTf2Release: async () => release }));
      const identity = await import(source + "/build-identity.ts");
      mock.module(source + "/build-identity.ts", () => ({ ...identity, applicationBuildIdentity: async () => build }));
      let packageReads = 0, receiptChecks = 0, operations = [];
      const packageModule = await import(profile + "/static-startup-package.ts");
      mock.module(profile + "/static-startup-package.ts", () => ({ ...packageModule, staticStartupPackage: async () => { packageReads++; return { sha256: sha, release, configuration }; } }));
      const gate = await import(profile + "/static-startup-gate.ts");
      mock.module(profile + "/static-startup-gate.ts", () => ({ ...gate, assertStaticStartupReceipt: (receipt, expected) => {
        receiptChecks++; expect(receipt.unitFixture).toBe(true); expect(expected).toEqual({ packageSha256: sha, wasmSha256: release.objects.wasm.sha256 });
      } }));
      const infra = await import(source + "/cloudflare-infra.ts");
      mock.module(source + "/cloudflare-infra.ts", () => ({ ...infra, applyCloudflareInfrastructure: async () => { operations.push("apply"); } }));
      const cloudflare = await import(source + "/cloudflare.ts");
      mock.module(source + "/cloudflare.ts", () => ({ ...cloudflare, runWrangler: async args => { expect(args[0]).toBe("deploy"); operations.push("deploy"); return { code: 0 }; } }));
      globalThis.fetch = async url => {
        const requested = new URL(url);
        if (requested.origin !== "https://playsrc.online" || !["/", "/tf2", "/tf2/playsrc-config.json"].includes(requested.pathname)) throw new Error("Unexpected asset scan or network request");
        operations.push(requested.pathname);
        return new Response(JSON.stringify(configuration));
      };
      const directory = root + "/apps/web/tf2/dist/cloudflare";
      await mkdir(directory + "/tf2/assets", { recursive: true });
      for (const file of ["index.html", "404.html", "_headers", "release.json", "tf2/index.html", "tf2/playsrc-config.json", "tf2/assets/style.css"]) await writeFile(directory + "/" + file, "unit fixture");
      const generation = { applicationBuild: build, wasmSha256: configuration.wasm.sha256, resourceRoots: Object.fromEntries(configuration.targets.map(t => [t.target, t.objects.resources.sha256])) };
      for (const prefix of ["index", "main", "gameplay-worker"]) await writeFile(directory + "/tf2/assets/" + prefix + "-test.js", "/*playsrc-generation:" + JSON.stringify(generation) + "*/");
      process.env.PLAYSRC_RELEASE_VERSION = "0.1.0";
      process.env.PLAYSRC_STATIC_STARTUP_RECEIPT = JSON.stringify({ unitFixture: true, packageSha256: sha, wasmSha256: release.objects.wasm.sha256 });
      const { deployCloudflare } = await import(source + "/deploy.ts");
      expect(existsSync(root + "/games")).toBe(false);
      await deployCloudflare(undefined);
      expect(packageReads).toBe(3); expect(receiptChecks).toBe(1);
      expect(operations).toEqual(["apply", "deploy", "/", "/tf2", "/tf2/playsrc-config.json"]);
      operations = [];
      await rm(directory + "/_headers");
      await expect(deployCloudflare(undefined)).rejects.toThrow("ENOENT");
      expect(operations).toEqual([]);
    `)
    const child = Bun.spawn([process.execPath, script], { cwd: root, stdin: new Blob([JSON.stringify(releaseJson)]), stdout: "pipe", stderr: "pipe" })
    const timer = setTimeout(() => child.kill(), 3_000)
    try {
      const [status, error] = await Promise.all([child.exited, new Response(child.stderr).text()])
      expect(error).toBe("")
      expect(status).toBe(0)
    } finally { clearTimeout(timer) }
  } finally { await rm(root, { recursive: true, force: true }) }
})
