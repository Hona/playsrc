import type { OwnedResourceGeneration } from "./resource-generation"

type Source = { disposables: OwnedResourceGeneration; payloadSha256: string; loadRequest: { resourceIdentity?: string } }

/** Only the currently admitted exact resource closure on this device can lend
 * resources to a candidate. Publication still belongs to transferTo. */
export function retainedSceneSource<T extends Source>(source: T | undefined, deviceGeneration: number,
  payloadSha256: string, resourceIdentity: string | undefined): T | undefined {
  return resourceIdentity && /^[0-9a-f]{64}$/.test(resourceIdentity)
    && source?.disposables.deviceGeneration === deviceGeneration && source.disposables.snapshot().state === "Active"
    && source.payloadSha256 === payloadSha256 && source.loadRequest.resourceIdentity === resourceIdentity ? source : undefined
}
