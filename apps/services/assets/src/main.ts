import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { startAssetService } from "./server"

const config = await loadLocalConfig()
const server = startAssetService(config.assetDir)
console.log(`http://${server.hostname}:${server.port}`)
