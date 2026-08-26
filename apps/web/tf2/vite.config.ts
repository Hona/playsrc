import preact from "@preact/preset-vite"
import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { defineConfig, type Plugin, type UserConfig } from "vite"
import { generationPlugin } from "./generation-plugin"
import { applicationBuildIdentity } from "../../../tools/playsrc/src/build-identity"

function localRuntime(ensureCoherentBuild?: () => Promise<void>): Plugin {
  return {
    name: "playsrc-local-runtime",
    configureServer(server) {
      server.middlewares.use("/playsrc-config.json", async (_request, response) => {
        try {
          await ensureCoherentBuild?.()
        } catch (error) {
          response.statusCode = 503
          response.setHeader("content-type", "application/problem+json")
          response.setHeader("cache-control", "no-store")
          response.end(JSON.stringify({ title: error instanceof Error ? error.message : "Browser build replacement failed", status: 503 }))
          return
        }
        const value = process.env.PLAYSRC_BROWSER_CONFIG
        if (!value) {
          response.statusCode = 503
          response.setHeader("content-type", "application/problem+json")
          response.end(JSON.stringify({ title: "Browser configuration unavailable", status: 503 }))
          return
        }
        response.statusCode = 200
        response.setHeader("content-type", "application/json; charset=utf-8")
        response.setHeader("cache-control", "no-store")
        response.end(value)
      })
    },
  }
}

export function tf2ViteConfiguration(
  assetOrigin = process.env.PLAYSRC_ASSET_ORIGIN,
  deployment = false,
  ensureCoherentBuild?: () => Promise<void>,
): UserConfig {
  const generation = async () => {
    await ensureCoherentBuild?.()
    const configuration = process.env.PLAYSRC_BROWSER_CONFIG ? JSON.parse(process.env.PLAYSRC_BROWSER_CONFIG) : undefined
    const applicationBuild = process.env.PLAYSRC_APPLICATION_BUILD
      ?? configuration?.applicationBuild
      ?? (deployment ? await applicationBuildIdentity() : undefined)
    if (!applicationBuild || !/^[0-9a-f]{64}$/.test(applicationBuild)) {
      throw new Error("TF2 application bundle build identity is unavailable")
    }
    const wasmSha256 = createHash("sha256").update(readFileSync(new URL("../../../games/tf2/browser/src/wasm-generated/tf2_wasm_bg.wasm", import.meta.url))).digest("hex")
    if (configuration && configuration.wasm.sha256 !== wasmSha256) throw new Error("TF2 configured WASM differs from its browser bindings producer")
    const targets = configuration?.targets ?? JSON.parse(readFileSync(new URL("./releases/current.json", import.meta.url), "utf8")).targets
    const resourceRoots = Object.fromEntries(targets.map((target: { target: string; objects: { resources: { sha256: string } } }) => [target.target, target.objects.resources.sha256]))
    return { applicationBuild, wasmSha256, resourceRoots }
  }
  return {
    base: deployment ? "/tf2/" : "/",
    plugins: [preact(), localRuntime(ensureCoherentBuild), generationPlugin(generation)],
    worker: { plugins: () => [generationPlugin(generation)] },
    resolve: {
      alias: {
        playsrc_metrics: fileURLToPath(new URL("../../../games/tf2/browser/src/wasm-metrics.ts", import.meta.url)),
      },
    },
    server: {
      host: "127.0.0.1",
      port: Number(process.env.PLAYSRC_DEV_PORT ?? "4173"),
      strictPort: true,
      headers: {
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
      },
      proxy: assetOrigin ? {
        "/objects": { target: assetOrigin, changeOrigin: false },
      } : undefined,
      fs: { allow: [fileURLToPath(new URL("../../../..", import.meta.url))] },
    },
    preview: {
      host: "127.0.0.1",
      port: Number(process.env.PLAYSRC_DEV_PORT ?? "4173"),
      strictPort: true,
      headers: {
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
      },
    },
    build: {
      target: "es2022",
      sourcemap: true,
      ...(deployment ? { outDir: "dist/cloudflare/tf2", emptyOutDir: true } : {}),
    },
  }
}

export default defineConfig(({ command }) => tf2ViteConfiguration(undefined, command === "build"))
