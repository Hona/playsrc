export const TF2_STARTUP_LOADING_PLAQUE = Object.freeze({
  material: "materials/console/startup_loading.vmt",
  texture: "materials/console/startup_loading.vtf",
  textureSize: Object.freeze({ width: 128, height: 64 }),
  panel: Object.freeze({ x: 2, y: 4, width: 110, height: 44, radius: 6 }),
  text: Object.freeze({ x: 18, y: 4, height: 44 }),
})

export function tf2StartupLoadingLabel(percentage: number): string {
  if (!Number.isSafeInteger(percentage) || percentage < 0 || percentage > 100) {
    throw new Error("TF2 startup loading percentage is invalid")
  }
  return `Loading ${percentage}%...`
}
