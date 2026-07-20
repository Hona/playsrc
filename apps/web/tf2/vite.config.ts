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

export function tf2ViteConfiguration(assetOrigin = process.env.PLAYSRC_ASSET_ORIGIN): UserConfig {
  return {
    plugins: [preact(), localRuntime()],
    server: {
      host: "127.0.0.1",
      port: 4173,
      strictPort: true,
      proxy: assetOrigin ? {
        "/objects": { target: assetOrigin, changeOrigin: false },
      } : undefined,
      fs: { allow: [fileURLToPath(new URL("../../../..", import.meta.url))] },
    },
    preview: { host: "127.0.0.1", port: 4173, strictPort: true },
    build: { target: "es2022", sourcemap: true },
  }
}

export default defineConfig(() => tf2ViteConfiguration())
