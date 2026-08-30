import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
export default headedProfileConfiguration({ match: process.env.PROFILE_SIMD_PLAYBACK === "1" ? "simd-audio-playback.profile.ts" : "simd-decoder.profile.ts" })
