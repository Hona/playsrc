import { ConfigurationError, loadLocalConfig } from "./config"

async function main(): Promise<number> {
  try {
    await loadLocalConfig()
  } catch (error) {
    if (error instanceof ConfigurationError) {
      console.error(`${error.code}: ${error.message}`)
      return 2
    }
    throw error
  }

  const [command] = process.argv.slice(2)
  console.error(`OwnerUnavailable: ${command ?? "command"} is not implemented`)
  return 3
}

process.exitCode = await main()
