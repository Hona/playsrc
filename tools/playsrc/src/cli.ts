import { ConfigurationError, loadLocalConfig } from "./config"
import { setup, SetupError } from "./setup"
import { ContentCacheError } from "@playsrc/content"
import { acquireMap, TargetError } from "./targets"

async function main(): Promise<number> {
  const [command, target] = process.argv.slice(2)
  try {
    if (command === "setup") {
      await setup()
      return 0
    }
    const config = await loadLocalConfig()
    if (command === "compile" || command === "dev") await acquireMap(config, target)
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
    throw error
  }

  console.error(`OwnerUnavailable: ${command ?? "command"} is not implemented`)
  return 3
}

process.exitCode = await main()
