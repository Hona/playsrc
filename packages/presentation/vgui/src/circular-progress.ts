/** CircularProgressBar::DrawCircleSegment's clockwise, top-origin sweep. */
export function circularProgressClip(progress: number, width: number, height: number): string {
  const corners = [[0.5,0],[1,0],[1,0.5],[1,1],[0.5,1],[0,1],[0,0.5],[0,0],[0.5,0]] as const
  const directions = [[1,0],[0,1],[0,1],[-1,0],[-1,0],[0,-1],[0,-1],[1,0]] as const
  if (progress >= 1) return "none"
  if (progress <= 0 || width <= 0 || height <= 0) return "polygon(0 0, 0 0, 0 0)"
  const radians = progress * Math.PI * 2, segment = Math.min(7, Math.floor(radians / (Math.PI / 4)))
  let internal = radians - segment * Math.PI / 4
  if (segment % 2 === 1) internal = Math.PI / 4 - internal
  const tangent = Math.tan(internal), direction = directions[segment]!, corner = corners[segment]!
  const x = corner[0] * width + (segment % 2 ? width / 2 - height / 2 * tangent : height / 2 * tangent) * direction[0]
  const y = corner[1] * height + (segment % 2 ? height / 2 - width / 2 * tangent : width / 2 * tangent) * direction[1]
  const points = [[width / 2,height / 2], ...corners.slice(0,segment+1).map(([x,y])=>[x*width,y*height]), [x,y]]
  return `polygon(${points.map(([x,y])=>`${x}px ${y}px`).join(", ")})`
}
