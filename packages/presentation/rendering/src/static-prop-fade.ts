export function distanceFadeOpacity(distanceSquared: number, minimumDistance: number, maximumDistance: number): number {
  const minimum = minimumDistance * minimumDistance
  const maximum = maximumDistance * maximumDistance
  if (distanceSquared >= maximum) return 0
  if (minimum >= 0 && distanceSquared > minimum) {
    return Math.max(0, Math.min(1, (maximum - distanceSquared) / (maximum - minimum)))
  }
  return 1
}

export function screenFadeOpacity(pixelWidth: number, minimumWidth: number, maximumWidth: number): number {
  if (pixelWidth <= minimumWidth) return 0
  if (maximumWidth >= 0 && pixelWidth < maximumWidth) {
    return Math.max(0, Math.min(1, (pixelWidth - minimumWidth) / (maximumWidth - minimumWidth)))
  }
  return 1
}

export function quantizeStaticPropOpacity(opacity: number): number {
  return Math.trunc(Math.max(0, Math.min(1, opacity)) * 255) / 255
}
