import { fileURLToPath } from "node:url"
import type { HtmlTagDescriptor, Plugin } from "vite"

/** Keep the original entry's requests on the HTML critical path even though
 * its evaluation is isolated behind the recoverable bootstrap import. */
export function bootstrapPreloadPlugin(): Plugin {
  let base = "/"
  const main = fileURLToPath(new URL("./src/main.tsx", import.meta.url)).replaceAll("\\", "/")
  return {
    name: "playsrc-bootstrap-preload",
    configResolved(configuration) { base = configuration.base },
    transformIndexHtml: {
      order: "post",
      handler(_html, context) {
        if (!context.bundle) return
        const entry = Object.values(context.bundle).find(chunk => chunk.type === "chunk" && chunk.facadeModuleId?.replaceAll("\\", "/") === main)
        if (!entry) throw new Error("TF2 bootstrap main entry is missing from the built module graph")
        const visited = new Set<string>(), styles = new Set<string>(), tags: HtmlTagDescriptor[] = []
        const visit = (fileName: string) => {
          if (visited.has(fileName)) return
          visited.add(fileName)
          const chunk = context.bundle![fileName]
          if (!chunk || chunk.type !== "chunk") throw new Error("TF2 bootstrap dependency is missing from the built module graph")
          tags.push({ tag: "link", attrs: { rel: "modulepreload", crossorigin: "", href: `${base}${fileName}` }, injectTo: "head" })
          for (const css of (chunk as typeof chunk & { viteMetadata?: { importedCss: Set<string> } }).viteMetadata?.importedCss ?? []) styles.add(css)
          for (const dependency of chunk.imports) visit(dependency)
        }
        visit(entry.fileName)
        for (const fileName of styles) tags.push({ tag: "link", attrs: { rel: "stylesheet", crossorigin: "", href: `${base}${fileName}` }, injectTo: "head" })
        return tags
      },
    },
  }
}
