import { simulationWorkflow } from "./config"

const profile = Bun.argv[2] ?? "smoke"
if (profile !== "smoke" && profile !== "full") {
  throw new Error("Simulation benchmark profile must be smoke or full")
}

const { root, cargo, crate } = await simulationWorkflow()
const child = Bun.spawn(
  [cargo, "run", "--quiet", "--release", "--manifest-path", crate, "--bin", "simulation-benchmark", "--", profile],
  { cwd: root, stdin: "inherit", stdout: "inherit", stderr: "inherit" },
)
const status = await child.exited
if (status !== 0) throw new Error(`Simulation benchmark failed with status ${status}`)
