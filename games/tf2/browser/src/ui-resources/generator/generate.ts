import path from "node:path"
import { fileURLToPath } from "node:url"
import toolchains from "../../../../../../tools/playsrc/toolchains.json"
import { TF2_CLASS_IMAGES, TF2_HUD_DYNAMIC_IMAGES, TF2_SCOREBOARD_IMAGES, TF2_CONTROL_POINT_IMAGES } from "../../hud/inventory"

const root = fileURLToPath(new URL("../../../../../..", import.meta.url))
const manifest = path.join(root, "games", "tf2", "browser", "src", "ui-resources", "generator", "Cargo.toml")
const classImages = [...new Set([
  ...Object.values(TF2_CLASS_IMAGES).flatMap((images) => Object.values(images)),
  ...TF2_HUD_DYNAMIC_IMAGES.filter((image) => image.startsWith("../hud/objectives_flagpanel_") || image.startsWith("../hud/objectives_timepanel_")),
  ...TF2_SCOREBOARD_IMAGES,
  ...TF2_CONTROL_POINT_IMAGES,
])]
const child = Bun.spawn(["cargo", `+${toolchains.rust.toolchain}`, "run", "--quiet", "--manifest-path", manifest, "--", JSON.stringify(classImages)], {
  cwd: root,
  stdout: "pipe",
  stderr: "inherit",
})
const output = await new Response(child.stdout).text()
if (await child.exited !== 0) throw new Error("TF2 UI resource generation failed")
if (!/^generated [1-9]\d* bytes sha256 [0-9a-f]{64}\n$/u.test(output)) throw new Error("TF2 UI resource generator report is malformed")

const bundle = Bun.spawn(["bun", "tools/source-bundle/scripts/generate-tf2-ui-manifest.ts"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
})
if (await bundle.exited !== 0) throw new Error("TF2 UI bundle manifest generation failed")
