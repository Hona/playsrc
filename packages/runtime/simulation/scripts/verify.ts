import { simulationWorkflow } from "./config"

const { root, cargo, crate } = await simulationWorkflow()

const commands = [
  [cargo, "fmt", "--manifest-path", crate, "--", "--check"],
  [cargo, "test", "--manifest-path", crate],
  [cargo, "test", "--release", "--manifest-path", crate],
  [cargo, "+stable", "clippy", "--manifest-path", crate, "--all-targets", "--", "-D", "warnings"],
  [cargo, "run", "--quiet", "--release", "--manifest-path", crate, "--bin", "simulation-benchmark", "--", "smoke"],
]

for (const command of commands) {
  const child = Bun.spawn(command, { cwd: root, stdin: "inherit", stdout: "inherit", stderr: "inherit" })
  const status = await child.exited
  if (status !== 0) throw new Error(`Simulation verification failed with status ${status}: ${command.join(" ")}`)
}
