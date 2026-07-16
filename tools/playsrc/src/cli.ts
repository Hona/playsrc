import { ConfigurationError, loadLocalConfig } from "./config"
import { setup, SetupError } from "./setup"

async function main(): Promise<number> {
  const [command] = process.argv.slice(2)
  try {
    if (command === "setup") {
      await setup()
      return 0
    }
    await loadLocalConfig()
  } catch (error) {
    if (error instanceof ConfigurationError) {
      console.error(`${error.code}: ${error.message}`)
      return 2
    }
    if (error instanceof SetupError) {
      console.error(`OwnerUnavailable: ${error.message}`)
      return 3
    }
    throw error
  }

  console.error(`OwnerUnavailable: ${command ?? "command"} is not implemented`)
  return 3
}

process.exitCode = await main()
