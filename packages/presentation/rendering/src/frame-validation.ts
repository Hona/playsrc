import type { Frame } from "./index"
import { PARTICLE_RENDER_OUTPUT_LIMITS } from "@playsrc/particle"

export type FrameEnvelopeFailure = Readonly<{ field: string; value: number | string; requirement: string }>

/** The frame envelope gate, shared by real submission and deterministic tests.
 * Failure values are bounded scalars; NaN/Infinity must not turn into JSON null. */
const CAMERA_FIELDS = ["yawDegrees","pitchDegrees","verticalFovDegrees","near","far"] as const
const failure = (field: string, value: number, requirement: string): FrameEnvelopeFailure => ({field,value:Number.isFinite(value)?value:String(value),requirement})

export function invalidFrameEnvelope(frame: Frame, maximumItems: number): FrameEnvelopeFailure | undefined {
  // A Source particle collection alone may contain 5000 particles (SDK
  // particles.h). PSPR has its own bounded aggregate admission. Sprites are
  // not scene objects: combining both populations rejects valid combat output.
  const count=frame.effects.length+(frame.shadows?.length??0)+(frame.models?.length??0)+(frame.brushModels?.models.length??0)
  if(count>maximumItems)return failure("sceneItems.total",count,`<=${maximumItems}`)
  const particleCount=frame.particles?.length??0
  if(particleCount>PARTICLE_RENDER_OUTPUT_LIMITS.maxRenderItems)return failure("particles.length",particleCount,`<=${PARTICLE_RENDER_OUTPUT_LIMITS.maxRenderItems}`)
  for(let i=0;i<frame.camera.position.length;i++)if(!Number.isFinite(frame.camera.position[i]))return failure(`camera.position[${i}]`,frame.camera.position[i]!,"finite")
  for(const field of CAMERA_FIELDS)if(!Number.isFinite(frame.camera[field]))return failure(`camera.${field}`,frame.camera[field],"finite")
  const delta=frame.deltaSeconds??0
  if(!Number.isFinite(delta))return failure("deltaSeconds",delta,"finite")
  if(frame.camera.verticalFovDegrees<=0||frame.camera.verticalFovDegrees>=180)return failure("camera.verticalFovDegrees",frame.camera.verticalFovDegrees,"0<value<180")
  if(frame.camera.near<=0)return failure("camera.near",frame.camera.near,">0")
  if(frame.camera.far<=frame.camera.near)return failure("camera.far",frame.camera.far,`>${frame.camera.near}`)
  if(delta<0)return failure("deltaSeconds",delta,">=0")
  if(frame.exposureHistogram&&frame.exposureHistogram.length!==16)return failure("exposureHistogram.length",frame.exposureHistogram.length,"=16")
}
