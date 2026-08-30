import { runHeadedProfile } from "../../../../tools/playsrc/src/profile-runner"

process.exitCode = await runHeadedProfile(["vgui-raster-parity"])
