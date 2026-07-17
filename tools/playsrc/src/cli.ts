import { ConfigurationError, loadLocalConfig } from "./config"
import { setup, SetupError } from "./setup"
import { ContentCacheError } from "@playsrc/content"
import { acquireMap, TargetError } from "./targets"
import { verifyTf2Wasm, WasmVerificationError } from "./verify-tf2-wasm"
import { DevelopmentError, runDevelopment } from "./dev"
import { BrowserEvidenceError, runBrowserAcceptance } from "./verify-browser"

async function main(): Promise<number> {
  const [command, target, argument] = process.argv.slice(2)
  try {
    if (command === "setup") {
      await setup()
      return 0
    }
    const config = await loadLocalConfig()
    if (command === "verify") {
      if (target === "tf2-wasm") {
        console.log(JSON.stringify(await verifyTf2Wasm(config, argument)))
        return 0
      }
      if (target === "browser") {
        await runBrowserAcceptance(config, argument)
        return 0
      }
    }
    if (command === "dev") {
      await runDevelopment(config, target)
      return 0
    }
    if (command === "compile") await acquireMap(config, target)
  } catch (error) {
    if (error instanceof ConfigurationError) {
      console.error(`${error.code}: ${error.message}`)
      return 2
    }
    if (error instanceof SetupError) {
      console.error(`OwnerUnavailable: ${error.message}`)
      return 3
    }
    if (error instanceof TargetError) {
      console.error(`${error.code}: ${error.message}`)
      return 2
    }
    if (error instanceof ContentCacheError) {
      console.error(`${error.code}: ${error.message}`)
      return 4
    }
    if (error instanceof WasmVerificationError) {
      console.error(`VerificationFailed: ${error.message}`)
      return 5
    }
    if (error instanceof DevelopmentError) {
      console.error(`${error.code}: ${error.message}`)
      return error.code === "CleanupFailure" ? 5 : 4
    }
    if (error instanceof BrowserEvidenceError) {
      console.error(`VerificationFailed: ${error.message}`)
      return 5
    }
    throw error
  }

  console.error(`OwnerUnavailable: ${command ?? "command"} is not implemented`)
  return 3
}

process.exitCode = await main()
