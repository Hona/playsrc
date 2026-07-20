import {
  TF2_SELECTED_OPTIONS,
  createSettingsState,
  decodeSettingsPersistence,
  encodeSettingsPersistence,
  type AdapterRequestResult,
  type BindingCapture,
  type OwnerAvailability,
  type RestartDisposition,
  type SettingOwner,
  type SettingsAdapterRequest,
  type SettingsDiagnostic,
  type SettingsSnapshot,
  type SettingsState,
  type SettingValue,
} from "@playsrc/settings"
import { TF2_CONTENT_BUILD } from "../content-build"

export const TF2_BROWSER_SETTINGS_STORAGE_KEY = `playsrc.tf2.options.build-${TF2_CONTENT_BUILD.contentBuild}.patch-${TF2_CONTENT_BUILD.patchVersion}`

export type Tf2BrowserSettingsSnapshot = Readonly<{
  settings: SettingsSnapshot
  persistenceDiagnostic: SettingsDiagnostic | null
  lastApply: Readonly<{
    complete: boolean
    rejectedOwners: readonly SettingOwner[]
    rejections: readonly Readonly<{ owner: SettingOwner; reason: string }>[]
    restart: readonly RestartDisposition[]
  }> | null
}>

export type Tf2BrowserSettings = Readonly<{
  snapshot(): Tf2BrowserSettingsSnapshot
  begin(): number
  set(settingId: string, value: SettingValue): void
  capture(settingId: string, capture: BindingCapture): Readonly<{ displacedSettingId?: string }>
  unbind(settingId: string): void
  defaults(settingIds?: readonly string[]): void
  cancel(): void
  apply(): Promise<Tf2BrowserSettingsSnapshot>
  synchronize(values: Readonly<Record<string, SettingValue>>): void
  persistence(): Uint8Array
}>

export type Tf2BrowserSettingsRequest = Readonly<{
  persistence: Uint8Array | null
  current?: Readonly<Record<string, SettingValue>>
  owners: Readonly<Record<SettingOwner, OwnerAvailability>>
  apply(request: SettingsAdapterRequest): Promise<AdapterRequestResult>
}>

function expect<T extends object>(result: Readonly<{ ok: true }> & T | Readonly<{ ok: false; diagnostic: SettingsDiagnostic }>): T {
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.message}`)
  return result
}

class BrowserSettings implements Tf2BrowserSettings {
  readonly #state: SettingsState
  readonly #applyOwner: Tf2BrowserSettingsRequest["apply"]
  readonly #persistenceDiagnostic: SettingsDiagnostic | null
  #transaction: number | null = null
  #lastApply: Tf2BrowserSettingsSnapshot["lastApply"] = null

  constructor(request: Tf2BrowserSettingsRequest) {
    let initial: Readonly<Record<string, SettingValue>> | undefined = request.current
    let persistenceDiagnostic: SettingsDiagnostic | null = null
    if (request.persistence !== null) {
      const decoded = decodeSettingsPersistence(TF2_SELECTED_OPTIONS, request.persistence)
      if (decoded.ok) initial = Object.freeze({ ...(request.current ?? {}), ...decoded.decoded.values })
      else persistenceDiagnostic = decoded.diagnostic
    }
    this.#persistenceDiagnostic = persistenceDiagnostic
    this.#state = createSettingsState({ catalog: TF2_SELECTED_OPTIONS, initial, owners: request.owners })
    this.#applyOwner = request.apply
  }

  snapshot(): Tf2BrowserSettingsSnapshot {
    return Object.freeze({
      settings: this.#state.snapshot(),
      persistenceDiagnostic: this.#persistenceDiagnostic,
      lastApply: this.#lastApply,
    })
  }

  begin(): number {
    if (this.#transaction !== null) return this.#transaction
    const result = expect(this.#state.beginTransaction())
    this.#transaction = result.transactionId
    return result.transactionId
  }

  #active(): number { return this.#transaction ?? this.begin() }
  set(settingId: string, value: SettingValue): void { expect(this.#state.setValue(this.#active(), settingId, value)) }
  capture(settingId: string, capture: BindingCapture): Readonly<{ displacedSettingId?: string }> {
    const result = expect(this.#state.captureBinding(this.#active(), settingId, capture))
    return Object.freeze(result.displacedSettingId ? { displacedSettingId: result.displacedSettingId } : {})
  }
  unbind(settingId: string): void { expect(this.#state.unbind(this.#active(), settingId)) }
  defaults(settingIds?: readonly string[]): void { expect(this.#state.reset(this.#active(), settingIds)) }
  cancel(): void {
    if (this.#transaction === null) return
    expect(this.#state.cancel(this.#transaction))
    this.#transaction = null
    this.#lastApply = null
  }

  async apply(): Promise<Tf2BrowserSettingsSnapshot> {
    const transaction = this.#active()
    const prepared = expect(this.#state.prepareApply(transaction))
    const results = await Promise.all(prepared.plan.requests.map((request) => this.#applyOwner(request)))
    const settled = expect(this.#state.settleApply(prepared.plan.planId, results))
    if (settled.complete) this.#transaction = null
    this.#lastApply = Object.freeze({
      complete: settled.complete,
      rejectedOwners: settled.rejectedOwners,
      rejections: settled.rejections,
      restart: settled.restart,
    })
    return this.snapshot()
  }

  synchronize(values: Readonly<Record<string, SettingValue>>): void { expect(this.#state.synchronize(values)) }

  persistence(): Uint8Array {
    return encodeSettingsPersistence(TF2_SELECTED_OPTIONS, this.#state.snapshot().current)
  }
}

export function initializeTf2BrowserSettings(request: Tf2BrowserSettingsRequest): Tf2BrowserSettings {
  if (!request || !(request.persistence === null || request.persistence instanceof Uint8Array) || typeof request.apply !== "function") {
    throw new Error("TF2 browser settings request is invalid")
  }
  return Object.freeze(new BrowserSettings(request))
}
