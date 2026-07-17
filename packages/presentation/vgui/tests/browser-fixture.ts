import { diagnosticResources, consoleResources, consoleLimits } from "../../../../apps/web/tf2/src/console-resources"
import {
  initializeClientDiagnostics,
  initializeDeveloperConsole,
  type ClientDiagnostics,
  type ConsoleCatalog,
  type ConsoleRequest,
  type DeveloperConsole,
} from "../src"

declare global {
  interface Window {
    vguiEvidence: Readonly<{
      reset(): void
      snapshots(): unknown
      hide(): void
      show(): void
      cycles(count: number): unknown
    }>
  }
}

const mount = document.getElementById("mount")
if (!mount) throw new Error("fixture mount is missing")

const catalog: ConsoleCatalog = Object.freeze({
  revision: "target-browser-fixture-1",
  items: Object.freeze([
    Object.freeze({ kind: "command" as const, name: "status", disposition: "visible" as const, acceptsSuggestions: false }),
    Object.freeze({ kind: "convar" as const, name: "cl_showfps", disposition: "visible" as const, displayValue: "2" }),
    Object.freeze({ kind: "convar" as const, name: "cl_showpos", disposition: "visible" as const, displayValue: "1" }),
  ]),
})

let developerConsole: DeveloperConsole | null = null
let diagnostics: ClientDiagnostics | null = null

function viewport() {
  const bounds = mount.getBoundingClientRect()
  return Object.freeze({ width: bounds.width, height: bounds.height, devicePixelRatio: window.devicePixelRatio })
}

function request(request: ConsoleRequest): void {
  if (request.kind === "visibility") developerConsole?.apply({ kind: "hide" })
}

function reset(): void {
  developerConsole?.apply({ kind: "destroy" })
  diagnostics?.apply({ kind: "destroy" })
  mount.replaceChildren()
  const initialized = initializeDeveloperConsole({
    runtimeIdentity: "target-console-evidence",
    limits: consoleLimits,
    resources: consoleResources,
    catalog,
    viewport: viewport(),
    reducedMotion: false,
    onRequest: request,
  })
  if (!initialized.ok) throw new Error(initialized.diagnostic.code)
  developerConsole = initialized.console
  if (!developerConsole.apply({ kind: "mount", root: mount }).ok) throw new Error("console mount failed")
  developerConsole.apply({
    kind: "append-output",
    segments: [
      { kind: "normal", text: "Team Fortress 2\n" },
      { kind: "developer", text: "Developer output\n" },
      { kind: "color", text: "Explicit color output\n", color: [120, 180, 255, 255] },
    ],
  })
  developerConsole.apply({ kind: "activate" })
  const initializedDiagnostics = initializeClientDiagnostics({
    runtimeIdentity: "target-diagnostic-evidence",
    resources: diagnosticResources,
    viewport: viewport(),
  })
  if (!initializedDiagnostics.ok) throw new Error(initializedDiagnostics.code)
  diagnostics = initializedDiagnostics.diagnostics
  if (!diagnostics.apply({ kind: "mount", root: mount }).ok) throw new Error("diagnostic mount failed")
  const frame = (realTimeMilliseconds: number) => ({
    realTimeMilliseconds,
    fpsMode: 2 as const,
    positionMode: 1 as const,
    mapIdentity: "jump_beef",
    view: { position: [5328, 3376, -3052] as const, angles: [-1, 180, 0] as const },
    player: { position: [5328, 3376, -3120] as const, angles: null, velocity: [300, 400, 0] as const },
  })
  diagnostics.apply({ kind: "present", frame: frame(1_000) })
  diagnostics.apply({ kind: "present", frame: frame(1_020) })
  diagnostics.apply({ kind: "present", frame: frame(1_030) })
}

window.addEventListener("resize", () => {
  const next = viewport()
  developerConsole?.apply({ kind: "set-viewport", viewport: next })
  diagnostics?.apply({ kind: "set-viewport", viewport: next })
})

window.vguiEvidence = Object.freeze({
  reset,
  snapshots: () => Object.freeze({ console: developerConsole?.snapshot(), diagnostics: diagnostics?.snapshot() }),
  hide: () => { developerConsole?.apply({ kind: "hide" }) },
  show: () => { developerConsole?.apply({ kind: "activate" }) },
  cycles: (count: number) => {
    for (let index = 0; index < count; index += 1) reset()
    return Object.freeze({ children: mount.childElementCount, snapshots: window.vguiEvidence.snapshots() })
  },
})

reset()
