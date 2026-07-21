import { ConfigurationError, loadLocalConfig } from "./config"

async function main(): Promise<number> {
  const [command, target, argument] = process.argv.slice(2)
  try {
    if (command === "setup") {
      const { setup } = await import("./setup")
      await setup()
      return 0
    }
    if (command === "infra") {
      const { applyCloudflareInfrastructure, bootstrapCloudflareState, planCloudflareInfrastructure } = await import("./cloudflare-infra")
      if (target === "bootstrap") await bootstrapCloudflareState()
      else if (target === "plan") await planCloudflareInfrastructure()
      else if (target === "apply") await applyCloudflareInfrastructure()
      else throw new Error("InfrastructureError: action must be bootstrap, plan, or apply")
      return 0
    }
    if (command === "deploy") {
      const { deployCloudflare } = await import("./deploy")
      await deployCloudflare(target)
      return 0
    }
    if (command === "verify" && target === "deploy") {
      const { verifyCloudflareDeployment } = await import("./deploy")
      await verifyCloudflareDeployment(argument)
      return 0
    }
    const config = await loadLocalConfig()
    if (command === "verify") {
      if (target === "tf2-wasm") {
        const { verifyTf2Wasm } = await import("./verify-tf2-wasm")
        console.log(JSON.stringify(await verifyTf2Wasm(config, argument)))
        return 0
      }
      if (target === "browser") {
        const { runBrowserAcceptance } = await import("./verify-browser")
        await runBrowserAcceptance(config, argument)
        return 0
      }
    }
    if (command === "dev") {
      const { runDevelopment } = await import("./dev")
      await runDevelopment(config, target)
      return 0
    }
    if (command === "release") {
      const { prepareTf2Release } = await import("./tf2-release")
      const artifact = await prepareTf2Release(config, target)
      console.log(JSON.stringify(artifact.release))
      return 0
    }
    if (command === "publish") {
      const { publishTf2Release } = await import("./cloudflare")
      await publishTf2Release(config, target)
      return 0
    }
    if (command === "compile") {
      const { acquireMap } = await import("./targets")
      await acquireMap(config, target)
    }
  } catch (error) {
    if (error instanceof ConfigurationError) {
      console.error(`${error.code}: ${error.message}`)
      return 2
    }
    if (error instanceof Error && error.name === "SetupError") {
      console.error(`OwnerUnavailable: ${error.message}`)
      return 3
    }
    if (error instanceof Error && error.name === "TargetError") {
      console.error(`${(error as Error & { code: string }).code}: ${error.message}`)
      return 2
    }
    if (error instanceof Error && error.name === "ContentCacheError") {
      console.error(`${(error as Error & { code: string }).code}: ${error.message}`)
      return 4
    }
    if (error instanceof Error && error.name === "WasmVerificationError") {
      console.error(`VerificationFailed: ${error.message}`)
      return 5
    }
    if (error instanceof Error && error.name === "Tf2WasmBuildError") {
      console.error(`BuildFailed: ${error.message}`)
      return 4
    }
    if (error instanceof Error && error.name === "DevelopmentError") {
      const code = (error as Error & { code: string }).code
      console.error(`${code}: ${error.message}`)
      return code === "CleanupFailure" ? 5 : 4
    }
    if (error instanceof Error && error.name === "BrowserEvidenceError") {
      console.error(`VerificationFailed: ${error.message}`)
      return 5
    }
    if (error instanceof Error && ["CloudflareError", "DeploymentError", "Tf2ReleaseError"].includes(error.name)) {
      console.error(`${error.name}: ${error.message}`)
      return 6
    }
    throw error
  }

  console.error(`OwnerUnavailable: ${command ?? "command"} is not implemented`)
  return 3
}

process.exitCode = await main()
