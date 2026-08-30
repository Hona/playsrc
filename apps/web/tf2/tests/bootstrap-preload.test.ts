import { expect, test } from "bun:test"
import { fileURLToPath } from "node:url"
import { bootstrapPreloadPlugin } from "../bootstrap-preload-plugin"

test("preloads the recoverable main entry, its exact static graph and CSS without changing lazy imports", () => {
  const plugin = bootstrapPreloadPlugin() as any
  plugin.configResolved({ base: "/tf2/" })
  const bundle = {
    "main.js": { type: "chunk", fileName: "main.js", facadeModuleId: fileURLToPath(new URL("../src/main.tsx", import.meta.url)), imports: ["shared.js"], dynamicImports: ["lazy.js"], viteMetadata: { importedCss: new Set(["main.css"]) } },
    "shared.js": { type: "chunk", fileName: "shared.js", imports: [], viteMetadata: { importedCss: new Set(["main.css"]) } },
    "lazy.js": { type: "chunk", fileName: "lazy.js", imports: [] },
  }
  expect(plugin.transformIndexHtml.handler("", { bundle }).map((tag: any) => tag.attrs)).toEqual([
    { rel: "modulepreload", crossorigin: "", href: "/tf2/main.js" },
    { rel: "modulepreload", crossorigin: "", href: "/tf2/shared.js" },
    { rel: "stylesheet", crossorigin: "", href: "/tf2/main.css" },
  ])
  expect(plugin.transformIndexHtml.handler("", {})).toBeUndefined()
  expect(() => plugin.transformIndexHtml.handler("", { bundle: {} })).toThrow("main entry is missing")
})
