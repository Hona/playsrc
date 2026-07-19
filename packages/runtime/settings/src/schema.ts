import {
  DEFAULT_SETTINGS_LIMITS,
  SETTING_OWNERS,
  SOURCE_CONVAR_FLAGS,
  SOURCE_KEY_MODIFIERS,
  type BindingCapture,
  type BindingProfile,
  type BindingProfileDefinition,
  type BindingValue,
  type EnumSettingOption,
  type EnumValue,
  type SettingSchema,
  type SettingsCatalog,
  type SettingsCatalogDefinition,
  type SettingsDiagnosticCode,
  type SettingsLimits,
  type SettingValue,
  type SourceConCommandContract,
  type SourceConVarContract,
} from "./contract"

const encoder = new TextEncoder()
const IDENTIFIER = /^[a-z0-9+][a-z0-9._/+:-]*$/u
const CONSOLE_NAME = /^[\x21-\x7e]+$/u

export class SettingsCatalogError extends Error {
  constructor(
    readonly code: "InvalidCatalog" | "LimitExceeded",
    message: string,
  ) {
    super(message)
    this.name = "SettingsCatalogError"
  }
}

function fail(code: "InvalidCatalog" | "LimitExceeded", message: string): never {
  throw new SettingsCatalogError(code, message)
}

export function utf8Length(value: string): number {
  return encoder.encode(value).byteLength
}

export function hasValidUnicode(value: string): boolean {
  if (typeof value !== "string") return false
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index)
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false
      index += 1
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return false
    }
  }
  return true
}

export function asciiFold(value: string): string {
  let result = ""
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    result += code >= 65 && code <= 90 ? String.fromCharCode(code + 32) : value[index]
  }
  return result
}

function validIdentifier(value: unknown, maximumBytes: number): value is string {
  return typeof value === "string"
    && IDENTIFIER.test(value)
    && utf8Length(value) <= maximumBytes
}

function validConsoleName(value: unknown, maximumBytes: number): value is string {
  return typeof value === "string"
    && CONSOLE_NAME.test(value)
    && !/[;"\s]/u.test(value)
    && utf8Length(value) <= maximumBytes
}

function validPhysicalCode(value: unknown, maximumBytes: number): value is string {
  return typeof value === "string"
    && value.length > 0
    && /^[\x21-\x7e]+$/u.test(value)
    && !/[;"]/u.test(value)
    && utf8Length(value) <= maximumBytes
}

function validBindingAction(value: unknown, maximumBytes: number): value is string {
  return typeof value === "string"
    && value.length > 0
    && hasValidUnicode(value)
    && !/[\u0000-\u001f\u007f]/u.test(value)
    && utf8Length(value) <= maximumBytes
}

function validFlags(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0 && (value as number) <= 0xffff_ffff
}

function validFiniteRange(minimum: number, maximum: number): boolean {
  return Number.isFinite(minimum) && Number.isFinite(maximum) && minimum <= maximum
}

function exactKeys(value: object, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort()
  const expected = [...keys].sort()
  return actual.length === expected.length && actual.every((key, index) => key === expected[index])
}

function normalizeLimits(source: Partial<SettingsLimits> | undefined): SettingsLimits {
  const value = { ...DEFAULT_SETTINGS_LIMITS, ...source }
  for (const [name, limit] of Object.entries(value)) {
    const ceiling = DEFAULT_SETTINGS_LIMITS[name as keyof SettingsLimits]
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > ceiling) {
      fail("InvalidCatalog", `${name} must be an integer from 1 through ${ceiling}`)
    }
  }
  if (value.maximumTransactionChanges > value.maximumSettings) {
    fail("InvalidCatalog", "maximumTransactionChanges cannot exceed maximumSettings")
  }
  return Object.freeze(value)
}

function cloneConVar(value: SourceConVarContract, limits: SettingsLimits): SourceConVarContract {
  if (!value || typeof value !== "object") fail("InvalidCatalog", "convar record is missing")
  const allowed = ["name", "defaultValue", "help", "flags", "visibility", "minimum", "maximum"]
  if (Object.keys(value).some((key) => !allowed.includes(key))) {
    fail("InvalidCatalog", `convar ${String(value.name)} has an unknown field`)
  }
  if (!validConsoleName(value.name, limits.maximumIdentifierUtf8Bytes)) {
    fail("InvalidCatalog", "convar name is invalid")
  }
  if (
    typeof value.defaultValue !== "string"
    || !hasValidUnicode(value.defaultValue)
    || utf8Length(value.defaultValue) > limits.maximumStringUtf8Bytes
    || typeof value.help !== "string"
    || !hasValidUnicode(value.help)
    || utf8Length(value.help) > limits.maximumStringUtf8Bytes
    || !validFlags(value.flags)
    || !["visible", "hidden", "development"].includes(value.visibility)
  ) {
    fail("InvalidCatalog", `convar ${value.name} metadata is invalid`)
  }
  if ((value.flags & SOURCE_CONVAR_FLAGS.HIDDEN) !== 0 && value.visibility !== "hidden") {
    fail("InvalidCatalog", `convar ${value.name} must expose hidden visibility`)
  }
  if (
    (value.flags & SOURCE_CONVAR_FLAGS.HIDDEN) === 0
    && (value.flags & SOURCE_CONVAR_FLAGS.DEVELOPMENT_ONLY) !== 0
    && value.visibility !== "development"
  ) {
    fail("InvalidCatalog", `convar ${value.name} must expose development visibility`)
  }
  const hasMinimum = value.minimum !== undefined
  const hasMaximum = value.maximum !== undefined
  if (
    (hasMinimum && !Number.isFinite(value.minimum))
    || (hasMaximum && !Number.isFinite(value.maximum))
    || (hasMinimum && hasMaximum && value.minimum! > value.maximum!)
  ) {
    fail("InvalidCatalog", `convar ${value.name} bounds are invalid`)
  }
  return Object.freeze({
    name: value.name,
    defaultValue: value.defaultValue,
    help: value.help,
    flags: value.flags,
    visibility: value.visibility,
    ...(hasMinimum ? { minimum: value.minimum } : {}),
    ...(hasMaximum ? { maximum: value.maximum } : {}),
  })
}

function cloneCommand(value: SourceConCommandContract, limits: SettingsLimits): SourceConCommandContract {
  if (!value || typeof value !== "object") fail("InvalidCatalog", "command record is missing")
  if (!exactKeys(value, ["name", "help", "flags", "visibility", "completion"])) {
    fail("InvalidCatalog", `command ${String(value.name)} fields are invalid`)
  }
  if (
    !validConsoleName(value.name, limits.maximumIdentifierUtf8Bytes)
    || typeof value.help !== "string"
    || !hasValidUnicode(value.help)
    || utf8Length(value.help) > limits.maximumStringUtf8Bytes
    || !validFlags(value.flags)
    || !["visible", "hidden", "development"].includes(value.visibility)
    || !["none", "owner"].includes(value.completion)
  ) {
    fail("InvalidCatalog", `command ${String(value.name)} metadata is invalid`)
  }
  if ((value.flags & SOURCE_CONVAR_FLAGS.HIDDEN) !== 0 && value.visibility !== "hidden") {
    fail("InvalidCatalog", `command ${value.name} must expose hidden visibility`)
  }
  return Object.freeze({ ...value })
}

function bindingIdentity(value: BindingValue): string {
  return `${value.modifiers}:${asciiFold(value.code)}`
}

function cloneProfile(
  definition: BindingProfileDefinition | undefined,
  limits: SettingsLimits,
): BindingProfile | null {
  if (definition === undefined) return null
  if (!definition || typeof definition !== "object") fail("InvalidCatalog", "binding profile is invalid")
  if (!exactKeys(definition, ["identity", "inputs", "modifierMask", "conflictPolicy", "reserved"])) {
    fail("InvalidCatalog", "binding profile fields are invalid")
  }
  if (!validIdentifier(definition.identity, limits.maximumIdentifierUtf8Bytes)) {
    fail("InvalidCatalog", "binding profile identity is invalid")
  }
  if (!Array.isArray(definition.inputs) || definition.inputs.length > limits.maximumPhysicalInputs) {
    fail("LimitExceeded", "binding profile input count exceeds its limit")
  }
  if (
    !Number.isSafeInteger(definition.modifierMask)
    || definition.modifierMask < 0
    || (definition.modifierMask & ~SOURCE_KEY_MODIFIERS.ALL) !== 0
    || !["replace", "reject"].includes(definition.conflictPolicy)
    || !Array.isArray(definition.reserved)
    || definition.reserved.length > definition.inputs.length * 8
  ) {
    fail("InvalidCatalog", "binding profile policy is invalid")
  }
  const known = new Map<string, string>()
  const inputs = definition.inputs.map((input) => {
    if (!input || typeof input !== "object") fail("InvalidCatalog", "binding input is invalid")
    const aliases = input.aliases ?? []
    const expectedKeys = input.aliases === undefined ? ["code"] : ["code", "aliases"]
    if (
      !exactKeys(input, expectedKeys)
      || !validPhysicalCode(input.code, limits.maximumIdentifierUtf8Bytes)
      || !Array.isArray(aliases)
      || aliases.length > limits.maximumAliasesPerInput
    ) {
      fail("InvalidCatalog", `binding input ${String(input.code)} is invalid`)
    }
    const clonedAliases = aliases.map((alias) => {
      if (!validPhysicalCode(alias, limits.maximumIdentifierUtf8Bytes)) {
        fail("InvalidCatalog", `binding alias ${String(alias)} is invalid`)
      }
      return alias
    })
    for (const candidate of [input.code, ...clonedAliases]) {
      const folded = asciiFold(candidate)
      if (known.has(folded)) fail("InvalidCatalog", `binding input ${candidate} is duplicated`)
      known.set(folded, input.code)
    }
    return Object.freeze({ code: input.code, aliases: Object.freeze(clonedAliases) })
  })
  const reservedIds = new Set<string>()
  const reserved = definition.reserved.map((value) => {
    if (!value || typeof value !== "object" || !exactKeys(value, ["code", "modifiers"])) {
      fail("InvalidCatalog", "reserved binding is invalid")
    }
    const code = known.get(asciiFold(value.code))
    if (
      code === undefined
      || !Number.isSafeInteger(value.modifiers)
      || value.modifiers < 0
      || (value.modifiers & ~definition.modifierMask) !== 0
    ) {
      fail("InvalidCatalog", `reserved binding ${String(value.code)} is invalid`)
    }
    const result = Object.freeze({ code, modifiers: value.modifiers })
    const identity = bindingIdentity(result)
    if (reservedIds.has(identity)) fail("InvalidCatalog", `reserved binding ${value.code} is duplicated`)
    reservedIds.add(identity)
    return result
  })
  return Object.freeze({
    identity: definition.identity,
    inputs: Object.freeze(inputs),
    modifierMask: definition.modifierMask,
    conflictPolicy: definition.conflictPolicy,
    reserved: Object.freeze(reserved),
  })
}

function enumValueEqual(left: EnumValue, right: EnumValue): boolean {
  return typeof left === typeof right && left === right
}

function validEnumValue(value: unknown, limits: SettingsLimits): value is EnumValue {
  return (typeof value === "number" && Number.isFinite(value))
    || (typeof value === "string" && hasValidUnicode(value) && utf8Length(value) <= limits.maximumStringUtf8Bytes)
}

function cloneOption(value: EnumSettingOption, limits: SettingsLimits): EnumSettingOption {
  if (
    !value
    || typeof value !== "object"
    || !exactKeys(value, ["value", "label"])
    || !validEnumValue(value.value, limits)
    || typeof value.label !== "string"
    || !hasValidUnicode(value.label)
    || utf8Length(value.label) > limits.maximumStringUtf8Bytes
  ) {
    fail("InvalidCatalog", "enum option is invalid")
  }
  return Object.freeze({ value: value.value, label: value.label })
}

function profileCode(profile: BindingProfile, candidate: string): string | null {
  const folded = asciiFold(candidate)
  for (const input of profile.inputs) {
    if (asciiFold(input.code) === folded || input.aliases.some((alias) => asciiFold(alias) === folded)) {
      return input.code
    }
  }
  return null
}

function normalizeBindingValue(profile: BindingProfile, value: BindingValue): BindingValue | null {
  if (!value || typeof value !== "object" || !exactKeys(value, ["code", "modifiers"])) return null
  if (typeof value.code !== "string") return null
  const code = profileCode(profile, value.code)
  if (
    code === null
    || !Number.isSafeInteger(value.modifiers)
    || value.modifiers < 0
    || (value.modifiers & ~profile.modifierMask) !== 0
  ) return null
  return Object.freeze({ code, modifiers: value.modifiers })
}

function cloneSetting(
  source: SettingSchema,
  limits: SettingsLimits,
  profile: BindingProfile | null,
  consoleNames: ReadonlySet<string>,
): SettingSchema {
  if (!source || typeof source !== "object") fail("InvalidCatalog", "setting record is missing")
  if (
    !validIdentifier(source.id, limits.maximumIdentifierUtf8Bytes)
    || !validIdentifier(source.page, limits.maximumIdentifierUtf8Bytes)
    || !SETTING_OWNERS.includes(source.owner)
    || !validFlags(source.flags)
    || !["visible", "hidden", "development"].includes(source.visibility)
    || !["live", "owner-restart", "application-restart"].includes(source.restart)
    || !["persistent", "session"].includes(source.persistence)
    || !Array.isArray(source.consoleNames)
  ) {
    fail("InvalidCatalog", `setting ${String(source.id)} metadata is invalid`)
  }
  const names = source.consoleNames.map((name) => {
    if (!validConsoleName(name, limits.maximumIdentifierUtf8Bytes) || !consoleNames.has(asciiFold(name))) {
      fail("InvalidCatalog", `setting ${source.id} references unknown console name ${String(name)}`)
    }
    return name
  })
  if (new Set(names.map(asciiFold)).size !== names.length) {
    fail("InvalidCatalog", `setting ${source.id} repeats a console name`)
  }
  const common = {
    id: source.id,
    page: source.page,
    owner: source.owner,
    flags: source.flags,
    visibility: source.visibility,
    restart: source.restart,
    persistence: source.persistence,
    consoleNames: Object.freeze(names),
  } as const
  switch (source.kind) {
    case "boolean":
      if (!exactKeys(source, [...Object.keys(common), "kind", "defaultValue"]) || typeof source.defaultValue !== "boolean") {
        fail("InvalidCatalog", `boolean setting ${source.id} is invalid`)
      }
      return Object.freeze({ ...common, kind: "boolean", defaultValue: source.defaultValue })
    case "integer":
      if (
        !exactKeys(source, [...Object.keys(common), "kind", "defaultValue", "minimum", "maximum"])
        || !Number.isSafeInteger(source.minimum)
        || !Number.isSafeInteger(source.maximum)
        || source.minimum > source.maximum
        || !Number.isSafeInteger(source.defaultValue)
      ) fail("InvalidCatalog", `integer setting ${source.id} is invalid`)
      return Object.freeze({ ...common, kind: "integer", defaultValue: source.defaultValue, minimum: source.minimum, maximum: source.maximum })
    case "float":
      if (
        !exactKeys(source, [...Object.keys(common), "kind", "defaultValue", "minimum", "maximum"])
        || !validFiniteRange(source.minimum, source.maximum)
        || !Number.isFinite(source.defaultValue)
      ) fail("InvalidCatalog", `float setting ${source.id} is invalid`)
      return Object.freeze({ ...common, kind: "float", defaultValue: source.defaultValue, minimum: source.minimum, maximum: source.maximum })
    case "enum": {
      if (
        !exactKeys(source, [...Object.keys(common), "kind", "defaultValue", "options"])
        || !validEnumValue(source.defaultValue, limits)
        || !Array.isArray(source.options)
        || source.options.length === 0
        || source.options.length > limits.maximumSettings
      ) fail("InvalidCatalog", `enum setting ${source.id} is invalid`)
      const options = source.options.map((option) => cloneOption(option, limits))
      for (let index = 0; index < options.length; index += 1) {
        if (options.slice(index + 1).some((candidate) => enumValueEqual(candidate.value, options[index].value))) {
          fail("InvalidCatalog", `enum setting ${source.id} repeats a value`)
        }
      }
      if (!options.some((option) => enumValueEqual(option.value, source.defaultValue))) {
        fail("InvalidCatalog", `enum setting ${source.id} default is not an option`)
      }
      return Object.freeze({ ...common, kind: "enum", defaultValue: source.defaultValue, options: Object.freeze(options) })
    }
    case "string":
      if (
        !exactKeys(source, [...Object.keys(common), "kind", "defaultValue", "minimumUtf8Bytes", "maximumUtf8Bytes"])
        || !Number.isSafeInteger(source.minimumUtf8Bytes)
        || !Number.isSafeInteger(source.maximumUtf8Bytes)
        || source.minimumUtf8Bytes < 0
        || source.minimumUtf8Bytes > source.maximumUtf8Bytes
        || source.maximumUtf8Bytes > limits.maximumStringUtf8Bytes
        || typeof source.defaultValue !== "string"
        || !hasValidUnicode(source.defaultValue)
        || utf8Length(source.defaultValue) < source.minimumUtf8Bytes
        || utf8Length(source.defaultValue) > source.maximumUtf8Bytes
      ) fail("InvalidCatalog", `string setting ${source.id} is invalid`)
      return Object.freeze({
        ...common,
        kind: "string",
        defaultValue: source.defaultValue,
        minimumUtf8Bytes: source.minimumUtf8Bytes,
        maximumUtf8Bytes: source.maximumUtf8Bytes,
      })
    case "binding": {
      if (
        !exactKeys(source, [...Object.keys(common), "kind", "defaultValue", "action"])
        || profile === null
        || !validBindingAction(source.action, limits.maximumStringUtf8Bytes)
      ) fail("InvalidCatalog", `binding setting ${source.id} is invalid`)
      const defaultValue = source.defaultValue === null ? null : normalizeBindingValue(profile, source.defaultValue)
      if (source.defaultValue !== null && defaultValue === null) {
        fail("InvalidCatalog", `binding setting ${source.id} default is invalid`)
      }
      return Object.freeze({ ...common, kind: "binding", defaultValue, action: source.action })
    }
    default:
      fail("InvalidCatalog", `setting ${String((source as { id?: unknown }).id)} kind is invalid`)
  }
}

export function defineSettingsCatalog(definition: SettingsCatalogDefinition): SettingsCatalog {
  if (!definition || typeof definition !== "object") fail("InvalidCatalog", "catalog definition is missing")
  const allowed = ["identity", "limits", "convars", "commands", "settings", "bindingProfile"]
  if (Object.keys(definition).some((key) => !allowed.includes(key))) {
    fail("InvalidCatalog", "catalog definition has an unknown field")
  }
  const limits = normalizeLimits(definition.limits)
  if (!validIdentifier(definition.identity, limits.maximumIdentifierUtf8Bytes)) {
    fail("InvalidCatalog", "catalog identity is invalid")
  }
  if (!Array.isArray(definition.settings)) fail("InvalidCatalog", "catalog settings are missing")
  const rawConVars = definition.convars ?? []
  const rawCommands = definition.commands ?? []
  if (!Array.isArray(rawConVars) || !Array.isArray(rawCommands)) fail("InvalidCatalog", "console catalog is invalid")
  if (definition.settings.length > limits.maximumSettings) fail("LimitExceeded", "setting count exceeds its limit")
  if (rawConVars.length > limits.maximumConVars) fail("LimitExceeded", "convar count exceeds its limit")
  if (rawCommands.length > limits.maximumConCommands) fail("LimitExceeded", "command count exceeds its limit")
  const convars = rawConVars.map((value) => cloneConVar(value, limits))
  const commands = rawCommands.map((value) => cloneCommand(value, limits))
  const consoleNames = new Set<string>()
  for (const item of [...convars, ...commands]) {
    const folded = asciiFold(item.name)
    if (consoleNames.has(folded)) fail("InvalidCatalog", `console name ${item.name} is duplicated`)
    consoleNames.add(folded)
  }
  const profile = cloneProfile(definition.bindingProfile, limits)
  const settings = definition.settings.map((value) => cloneSetting(value, limits, profile, consoleNames))
  const settingIds = new Set<string>()
  const actions = new Set<string>()
  const defaultBindings = new Set<string>()
  for (const setting of settings) {
    const folded = asciiFold(setting.id)
    if (settingIds.has(folded)) fail("InvalidCatalog", `setting id ${setting.id} is duplicated`)
    settingIds.add(folded)
    if (setting.kind === "binding") {
      const action = asciiFold(setting.action)
      if (actions.has(action)) fail("InvalidCatalog", `binding action ${setting.action} is duplicated`)
      actions.add(action)
      if (setting.defaultValue !== null) {
        const identity = bindingIdentity(setting.defaultValue)
        if (defaultBindings.has(identity)) fail("InvalidCatalog", `default binding ${setting.defaultValue.code} conflicts`)
        defaultBindings.add(identity)
      }
    }
  }
  return Object.freeze({
    identity: definition.identity,
    limits,
    convars: Object.freeze(convars),
    commands: Object.freeze(commands),
    settings: Object.freeze(settings),
    bindingProfile: profile,
  })
}

export function settingById(catalog: SettingsCatalog, settingId: string): SettingSchema | null {
  if (typeof settingId !== "string") return null
  const folded = asciiFold(settingId)
  return catalog.settings.find((setting) => asciiFold(setting.id) === folded) ?? null
}

export function normalizeBindingCapture(
  profile: BindingProfile,
  capture: BindingCapture,
): Readonly<{ ok: true; binding: BindingValue }> | Readonly<{
  ok: false
  code: Extract<SettingsDiagnosticCode, "UnknownPhysicalInput" | "InvalidModifiers" | "ReservedBinding">
  message: string
}> {
  if (!capture || typeof capture !== "object" || typeof capture.code !== "string") {
    return Object.freeze({ ok: false, code: "UnknownPhysicalInput", message: "physical input is malformed" })
  }
  for (const field of [capture.shift, capture.control, capture.alt]) {
    if (field !== undefined && typeof field !== "boolean") {
      return Object.freeze({ ok: false, code: "InvalidModifiers", message: "modifier state must be boolean" })
    }
  }
  const code = profileCode(profile, capture.code)
  if (code === null) {
    return Object.freeze({ ok: false, code: "UnknownPhysicalInput", message: `physical input ${capture.code} is unknown` })
  }
  const modifiers = (capture.shift ? SOURCE_KEY_MODIFIERS.SHIFT : 0)
    | (capture.control ? SOURCE_KEY_MODIFIERS.CONTROL : 0)
    | (capture.alt ? SOURCE_KEY_MODIFIERS.ALT : 0)
  if ((modifiers & ~profile.modifierMask) !== 0) {
    return Object.freeze({ ok: false, code: "InvalidModifiers", message: "modifier is outside the physical profile" })
  }
  const binding = Object.freeze({ code, modifiers })
  if (profile.reserved.some((candidate) => bindingIdentity(candidate) === bindingIdentity(binding))) {
    return Object.freeze({ ok: false, code: "ReservedBinding", message: `binding ${code} is reserved` })
  }
  return Object.freeze({ ok: true, binding })
}

export function cloneSettingValue(value: SettingValue): SettingValue {
  if (value !== null && typeof value === "object") {
    return Object.freeze({ code: value.code, modifiers: value.modifiers })
  }
  return value
}

export function settingValuesEqual(left: SettingValue, right: SettingValue): boolean {
  if (left === right) return true
  if (left === null || right === null || typeof left !== "object" || typeof right !== "object") return false
  return left.code === right.code && left.modifiers === right.modifiers
}

export function validateSettingValue(
  catalog: SettingsCatalog,
  schema: SettingSchema,
  value: SettingValue,
): Readonly<{ ok: true; value: SettingValue }> | Readonly<{ ok: false; message: string }> {
  switch (schema.kind) {
    case "boolean":
      return typeof value === "boolean"
        ? Object.freeze({ ok: true, value })
        : Object.freeze({ ok: false, message: "value must be boolean" })
    case "integer":
      return typeof value === "number"
        && Number.isSafeInteger(value)
        && value >= schema.minimum
        && value <= schema.maximum
        ? Object.freeze({ ok: true, value })
        : Object.freeze({ ok: false, message: `value must be an integer from ${schema.minimum} through ${schema.maximum}` })
    case "float":
      return typeof value === "number"
        && Number.isFinite(value)
        && value >= schema.minimum
        && value <= schema.maximum
        ? Object.freeze({ ok: true, value })
        : Object.freeze({ ok: false, message: `value must be finite and from ${schema.minimum} through ${schema.maximum}` })
    case "enum":
      return (typeof value === "string" || typeof value === "number")
        && schema.options.some((option) => enumValueEqual(option.value, value))
        ? Object.freeze({ ok: true, value })
        : Object.freeze({ ok: false, message: "value is not a declared enum option" })
    case "string": {
      if (typeof value !== "string" || !hasValidUnicode(value)) {
        return Object.freeze({ ok: false, message: "value must be valid Unicode text" })
      }
      const length = utf8Length(value)
      return length >= schema.minimumUtf8Bytes && length <= schema.maximumUtf8Bytes
        ? Object.freeze({ ok: true, value })
        : Object.freeze({ ok: false, message: `UTF-8 value length must be from ${schema.minimumUtf8Bytes} through ${schema.maximumUtf8Bytes}` })
    }
    case "binding": {
      if (value === null) return Object.freeze({ ok: true, value })
      if (typeof value !== "object") return Object.freeze({ ok: false, message: "binding must be a physical chord or null" })
      if (catalog.bindingProfile === null) return Object.freeze({ ok: false, message: "binding profile is missing" })
      const normalized = normalizeBindingValue(catalog.bindingProfile, value)
      return normalized === null
        ? Object.freeze({ ok: false, message: "binding is outside the physical profile" })
        : Object.freeze({ ok: true, value: normalized })
    }
  }
}

export function validateStoredSettingValue(
  catalog: SettingsCatalog,
  schema: SettingSchema,
  value: SettingValue,
): Readonly<{ ok: true; value: SettingValue }> | Readonly<{ ok: false; message: string }> {
  if (schema.kind === "integer") {
    return typeof value === "number" && Number.isSafeInteger(value)
      ? Object.freeze({ ok: true, value })
      : Object.freeze({ ok: false, message: "stored value must be a safe integer" })
  }
  if (schema.kind === "float") {
    return typeof value === "number" && Number.isFinite(value)
      ? Object.freeze({ ok: true, value })
      : Object.freeze({ ok: false, message: "stored value must be finite" })
  }
  return validateSettingValue(catalog, schema, value)
}

export function defaultSettingValues(catalog: SettingsCatalog): Readonly<Record<string, SettingValue>> {
  return freezeSettingValues(Object.fromEntries(
    catalog.settings.map((setting) => [setting.id, cloneSettingValue(setting.defaultValue)]),
  ))
}

export function freezeSettingValues(
  values: Readonly<Record<string, SettingValue>>,
): Readonly<Record<string, SettingValue>> {
  const clone: Record<string, SettingValue> = Object.create(null)
  for (const [key, value] of Object.entries(values)) clone[key] = cloneSettingValue(value)
  return Object.freeze(clone)
}

export function bindingValuesEqual(left: BindingValue, right: BindingValue): boolean {
  return bindingIdentity(left) === bindingIdentity(right)
}
