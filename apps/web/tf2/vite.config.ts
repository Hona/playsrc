import preact from "@preact/preset-vite"
import { fileURLToPath } from "node:url"
import { defineConfig, type Plugin, type UserConfig } from "vite"

function localRuntime(): Plugin {
  return {
    name: "playsrc-local-runtime",
    configureServer(server) {
      server.middlewares.use("/playsrc-config.json", (_request, response) => {
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
): UserConfig {
  return {
    base: deployment ? "/tf2/" : "/",
    plugins: [preact(), localRuntime()],
    resolve: {
      alias: {
        playsrc_metrics: fileURLToPath(new URL("../../../games/tf2/browser/src/wasm-metrics.ts", import.meta.url)),
      },
    },
    server: {
      host: "127.0.0.1",
      port: Number(process.env.PLAYSRC_DEV_PORT ?? 4173),
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
      port: Number(process.env.PLAYSRC_DEV_PORT ?? 4173),
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
