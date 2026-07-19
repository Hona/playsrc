export const SOURCE_CONVAR_FLAGS = Object.freeze({
  NONE: 0x0000_0000,
  UNREGISTERED: 0x0000_0001,
  DEVELOPMENT_ONLY: 0x0000_0002,
  GAME_DLL: 0x0000_0004,
  CLIENT_DLL: 0x0000_0008,
  HIDDEN: 0x0000_0010,
  PROTECTED: 0x0000_0020,
  SINGLE_PLAYER_ONLY: 0x0000_0040,
  ARCHIVE: 0x0000_0080,
  NOTIFY: 0x0000_0100,
  USER_INFO: 0x0000_0200,
  PRINTABLE_ONLY: 0x0000_0400,
  UNLOGGED: 0x0000_0800,
  NEVER_AS_STRING: 0x0000_1000,
  REPLICATED: 0x0000_2000,
  CHEAT: 0x0000_4000,
  INTERNAL_USE: 0x0000_8000,
  DEMO: 0x0001_0000,
  DO_NOT_RECORD: 0x0002_0000,
  ALLOWED_IN_COMPETITIVE: 0x0004_0000,
  RELOAD_MATERIALS: 0x0010_0000,
  RELOAD_TEXTURES: 0x0020_0000,
  NOT_CONNECTED: 0x0040_0000,
  MATERIAL_SYSTEM_THREAD: 0x0080_0000,
  ARCHIVE_XBOX: 0x0100_0000,
  ACCESSIBLE_FROM_THREADS: 0x0200_0000,
  SERVER_CAN_EXECUTE: 0x1000_0000,
  SERVER_CANNOT_QUERY: 0x2000_0000,
  CLIENT_COMMAND_CAN_EXECUTE: 0x4000_0000,
  EXECUTE_DESPITE_DEFAULT: 0x8000_0000,
} as const)

export const SOURCE_KEY_MODIFIERS = Object.freeze({
  NONE: 0,
  SHIFT: 1,
  CONTROL: 2,
  ALT: 4,
  ALL: 7,
} as const)

export const SOURCE_COMMAND_LIMITS = Object.freeze({
  maximumArguments: 64,
  maximumUtf8Bytes: 511,
} as const)

export const SETTING_OWNERS = Object.freeze([
  "renderer",
  "audio",
  "input",
  "game",
  "application",
] as const)

export type SettingOwner = (typeof SETTING_OWNERS)[number]
export type SettingVisibility = "visible" | "hidden" | "development"
export type RestartDisposition = "live" | "owner-restart" | "application-restart"
export type PersistenceDisposition = "persistent" | "session"
export type OwnerAvailability = "available" | "unavailable" | "unknown"
export type BindingConflictPolicy = "replace" | "reject"
export type EnumValue = string | number

export type BindingValue = Readonly<{
  code: string
  modifiers: number
}>

export type SettingValue = boolean | number | string | BindingValue | null

export type SourceConVarContract = Readonly<{
  name: string
  defaultValue: string
  help: string
  flags: number
  visibility: SettingVisibility
  minimum?: number
  maximum?: number
}>

export type SourceConCommandContract = Readonly<{
  name: string
  help: string
  flags: number
  visibility: SettingVisibility
  completion: "none" | "owner"
}>

type SettingSchemaCommon = Readonly<{
  id: string
  page: string
  owner: SettingOwner
  flags: number
  visibility: SettingVisibility
  restart: RestartDisposition
  persistence: PersistenceDisposition
  consoleNames: readonly string[]
}>

export type BooleanSettingSchema = SettingSchemaCommon & Readonly<{
  kind: "boolean"
  defaultValue: boolean
}>

export type IntegerSettingSchema = SettingSchemaCommon & Readonly<{
  kind: "integer"
  defaultValue: number
  minimum: number
  maximum: number
}>

export type FloatSettingSchema = SettingSchemaCommon & Readonly<{
  kind: "float"
  defaultValue: number
  minimum: number
  maximum: number
}>

export type EnumSettingOption = Readonly<{
  value: EnumValue
  label: string
}>

export type EnumSettingSchema = SettingSchemaCommon & Readonly<{
  kind: "enum"
  defaultValue: EnumValue
  options: readonly EnumSettingOption[]
}>

export type StringSettingSchema = SettingSchemaCommon & Readonly<{
  kind: "string"
  defaultValue: string
  minimumUtf8Bytes: number
  maximumUtf8Bytes: number
}>

export type BindingSettingSchema = SettingSchemaCommon & Readonly<{
  kind: "binding"
  defaultValue: BindingValue | null
  action: string
}>

export type SettingSchema =
  | BooleanSettingSchema
  | IntegerSettingSchema
  | FloatSettingSchema
  | EnumSettingSchema
  | StringSettingSchema
  | BindingSettingSchema

export type BindingProfileInput = Readonly<{
  code: string
  aliases?: readonly string[]
}>

export type BindingProfileDefinition = Readonly<{
  identity: string
  inputs: readonly BindingProfileInput[]
  modifierMask: number
  conflictPolicy: BindingConflictPolicy
  reserved: readonly BindingValue[]
}>

export type BindingProfile = Readonly<{
  identity: string
  inputs: readonly Readonly<{ code: string; aliases: readonly string[] }>[]
  modifierMask: number
  conflictPolicy: BindingConflictPolicy
  reserved: readonly BindingValue[]
}>

export type SettingsLimits = Readonly<{
  maximumSettings: number
  maximumConVars: number
  maximumConCommands: number
  maximumPhysicalInputs: number
  maximumAliasesPerInput: number
  maximumIdentifierUtf8Bytes: number
  maximumStringUtf8Bytes: number
  maximumTransactionChanges: number
  maximumJournalEntries: number
  maximumPersistenceBytes: number
}>

export const DEFAULT_SETTINGS_LIMITS: SettingsLimits = Object.freeze({
  maximumSettings: 512,
  maximumConVars: 512,
  maximumConCommands: 128,
  maximumPhysicalInputs: 256,
  maximumAliasesPerInput: 8,
  maximumIdentifierUtf8Bytes: 127,
  maximumStringUtf8Bytes: 4_096,
  maximumTransactionChanges: 512,
  maximumJournalEntries: 512,
  maximumPersistenceBytes: 256 * 1_024,
})

export type SettingsCatalogDefinition = Readonly<{
  identity: string
  limits?: Partial<SettingsLimits>
  convars?: readonly SourceConVarContract[]
  commands?: readonly SourceConCommandContract[]
  settings: readonly SettingSchema[]
  bindingProfile?: BindingProfileDefinition
}>

export type SettingsCatalog = Readonly<{
  identity: string
  limits: SettingsLimits
  convars: readonly SourceConVarContract[]
  commands: readonly SourceConCommandContract[]
  settings: readonly SettingSchema[]
  bindingProfile: BindingProfile | null
}>

export type SettingsDiagnosticCode =
  | "InvalidCatalog"
  | "LimitExceeded"
  | "UnknownSetting"
  | "InvalidValue"
  | "NoActiveTransaction"
  | "TransactionActive"
  | "StaleTransaction"
  | "ApplyInFlight"
  | "StaleApplyPlan"
  | "OwnerUnknown"
  | "OwnerUnavailable"
  | "BindingProfileMissing"
  | "UnknownPhysicalInput"
  | "InvalidModifiers"
  | "ReservedBinding"
  | "BindingConflict"
  | "MalformedApplyResults"
  | "MalformedPersistence"
  | "WrongCatalog"
  | "UnknownPersistedSetting"
  | "MissingPersistedSetting"
  | "DuplicatePersistedSetting"

export type SettingsDiagnostic = Readonly<{
  code: SettingsDiagnosticCode
  operation: string
  settingId?: string
  owner?: SettingOwner
  transactionId?: number
  planId?: number
  conflictingSettingId?: string
  unknownIds?: readonly string[]
  message: string
}>

export type SettingsSuccess<T extends object = Record<never, never>> = Readonly<{
  ok: true
}> & Readonly<T>

export type SettingsFailure = Readonly<{
  ok: false
  diagnostic: SettingsDiagnostic
}>

export type SettingsResult<T extends object = Record<never, never>> = SettingsSuccess<T> | SettingsFailure

export type BindingCapture = Readonly<{
  code: string
  shift?: boolean
  control?: boolean
  alt?: boolean
}>

export type SettingChange = Readonly<{
  settingId: string
  kind: SettingSchema["kind"]
  previousValue: SettingValue
  nextValue: SettingValue
  consoleNames: readonly string[]
  restart: RestartDisposition
}>

export type SettingsAdapterRequest = Readonly<{
  kind: `${SettingOwner}-settings`
  owner: SettingOwner
  requestId: number
  planId: number
  transactionId: number
  changes: readonly SettingChange[]
}>

export type ApplyPlan = Readonly<{
  planId: number
  transactionId: number
  baseRevision: number
  requests: readonly SettingsAdapterRequest[]
}>

export type AdapterRequestResult =
  | Readonly<{ requestId: number; status: "applied" }>
  | Readonly<{ requestId: number; status: "rejected"; reason: string }>

export type ChangeJournalKind =
  | "transaction-began"
  | "value-staged"
  | "binding-replaced"
  | "binding-unbound"
  | "defaults-staged"
  | "persistence-staged"
  | "transaction-cancelled"
  | "apply-prepared"
  | "apply-settled"
  | "owner-availability-changed"
  | "current-synchronized"

export type ChangeJournalEntry = Readonly<{
  sequence: number
  revision: number
  kind: ChangeJournalKind
  settingIds: readonly string[]
  transactionId?: number
  planId?: number
  requestIds?: readonly number[]
}>

export type SettingsSnapshot = Readonly<{
  catalogIdentity: string
  revision: number
  current: Readonly<Record<string, SettingValue>>
  applied: Readonly<Record<string, SettingValue>>
  pending: Readonly<Record<string, SettingValue>> | null
  dirtySettingIds: readonly string[]
  ownerAvailability: Readonly<Record<SettingOwner, OwnerAvailability>>
  activeTransactionId: number | null
  inFlightPlanId: number | null
  journalStartSequence: number
  journal: readonly ChangeJournalEntry[]
}>

export type SettingsStateConfiguration = Readonly<{
  catalog: SettingsCatalog
  initial?: Readonly<Record<string, SettingValue>>
  owners?: Partial<Readonly<Record<SettingOwner, OwnerAvailability>>>
}>

export interface SettingsState {
  snapshot(): SettingsSnapshot
  beginTransaction(): SettingsResult<{ transactionId: number; revision: number }>
  setValue(transactionId: number, settingId: string, value: SettingValue): SettingsResult<{ revision: number }>
  captureBinding(
    transactionId: number,
    settingId: string,
    capture: BindingCapture,
  ): SettingsResult<{ revision: number; binding: BindingValue; displacedSettingId?: string }>
  unbind(transactionId: number, settingId: string): SettingsResult<{ revision: number }>
  reset(transactionId: number, settingIds?: readonly string[]): SettingsResult<{ revision: number }>
  stagePersistence(
    transactionId: number,
    values: Readonly<Record<string, SettingValue>>,
  ): SettingsResult<{ revision: number }>
  cancel(transactionId: number): SettingsResult<{ revision: number }>
  prepareApply(transactionId: number): SettingsResult<{ plan: ApplyPlan }>
  settleApply(
    planId: number,
    results: readonly AdapterRequestResult[],
  ): SettingsResult<{
    revision: number
    complete: boolean
    rejectedOwners: readonly SettingOwner[]
    rejections: readonly Readonly<{ owner: SettingOwner; reason: string }>[]
    restart: readonly RestartDisposition[]
  }>
  setOwnerAvailability(owner: SettingOwner, availability: OwnerAvailability): SettingsResult<{ revision: number }>
  synchronize(values: Readonly<Record<string, SettingValue>>): SettingsResult<{ revision: number }>
}

export type DecodedSettingsPersistence = Readonly<{
  catalogIdentity: string
  values: Readonly<Record<string, SettingValue>>
}>

export type PersistenceDecodeFailure = Readonly<{
  ok: false
  diagnostic: SettingsDiagnostic
}>

export type PersistenceDecodeResult =
  | Readonly<{ ok: true; decoded: DecodedSettingsPersistence }>
  | PersistenceDecodeFailure
