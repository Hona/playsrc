import {
  type DecodedSettingsPersistence,
  type PersistenceDecodeResult,
  type SettingSchema,
  type SettingsCatalog,
  type SettingsDiagnostic,
  type SettingsDiagnosticCode,
  type SettingValue,
} from "./contract"
import {
  cloneSettingValue,
  freezeSettingValues,
  hasValidUnicode,
  settingById,
  validateStoredSettingValue,
} from "./schema"

const FORMAT = "playsrc-settings"
const REVISION = 1
const encoder = new TextEncoder()
const decoder = new TextDecoder("utf-8", { fatal: true })

export class SettingsPersistenceError extends Error {
  constructor(
    readonly code: "MalformedPersistence" | "UnknownSetting" | "MissingPersistedSetting" | "LimitExceeded",
    message: string,
  ) {
    super(message)
    this.name = "SettingsPersistenceError"
  }
}

function decodeFailure(
  code: SettingsDiagnosticCode,
  message: string,
  details: Omit<SettingsDiagnostic, "code" | "operation" | "message"> = {},
): PersistenceDecodeResult {
  const cloned = { ...details } as Record<string, unknown>
  if (details.unknownIds) cloned.unknownIds = Object.freeze([...details.unknownIds])
  return Object.freeze({
    ok: false,
    diagnostic: Object.freeze({ code, operation: "decode-persistence", message, ...cloned }) as SettingsDiagnostic,
  })
}

function exactKeys(value: object, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort()
  const expected = [...keys].sort()
  return actual.length === expected.length && actual.every((key, index) => key === expected[index])
}

function persistedSettings(catalog: SettingsCatalog): readonly SettingSchema[] {
  return catalog.settings.filter((schema) => schema.persistence === "persistent")
}

function encodedValue(value: SettingValue): unknown {
  if (value !== null && typeof value === "object") {
    return { code: value.code, modifiers: value.modifiers }
  }
  return value
}

export function encodeSettingsPersistence(
  catalog: SettingsCatalog,
  values: Readonly<Record<string, SettingValue>>,
): Uint8Array {
  if (!values || typeof values !== "object" || Array.isArray(values)) {
    throw new SettingsPersistenceError("MalformedPersistence", "setting values must be a record")
  }
  for (const id of Object.keys(values)) {
    if (!settingById(catalog, id)) {
      throw new SettingsPersistenceError("UnknownSetting", `setting ${id} is unknown`)
    }
  }
  const entries: unknown[] = []
  for (const schema of persistedSettings(catalog)) {
    if (!Object.hasOwn(values, schema.id)) {
      throw new SettingsPersistenceError("MissingPersistedSetting", `setting ${schema.id} is missing`)
    }
    const validated = validateStoredSettingValue(catalog, schema, values[schema.id])
    if (!validated.ok) {
      throw new SettingsPersistenceError("MalformedPersistence", `setting ${schema.id}: ${validated.message}`)
    }
    entries.push([schema.id, schema.kind, encodedValue(validated.value)])
  }
  const document = {
    format: FORMAT,
    revision: REVISION,
    catalog: catalog.identity,
    values: entries,
  }
  const bytes = encoder.encode(JSON.stringify(document))
  if (bytes.byteLength > catalog.limits.maximumPersistenceBytes) {
    throw new SettingsPersistenceError("LimitExceeded", "persistence bytes exceed the catalog limit")
  }
  return bytes
}

function decodeValue(
  catalog: SettingsCatalog,
  schema: SettingSchema,
  value: unknown,
): Readonly<{ ok: true; value: SettingValue }> | Readonly<{ ok: false; message: string }> {
  let candidate = value as SettingValue
  if (schema.kind === "binding" && value !== null) {
    if (!value || typeof value !== "object" || Array.isArray(value) || !exactKeys(value, ["code", "modifiers"])) {
      return Object.freeze({ ok: false, message: "binding persistence value is malformed" })
    }
    const record = value as Record<string, unknown>
    if (typeof record.code !== "string" || !Number.isSafeInteger(record.modifiers)) {
      return Object.freeze({ ok: false, message: "binding persistence value is malformed" })
    }
    candidate = Object.freeze({ code: record.code, modifiers: record.modifiers as number })
  } else if (value !== null && typeof value === "object") {
    return Object.freeze({ ok: false, message: "scalar persistence value is malformed" })
  }
  const validated = validateStoredSettingValue(catalog, schema, candidate)
  return validated.ok
    ? Object.freeze({ ok: true, value: cloneSettingValue(validated.value) })
    : Object.freeze({ ok: false, message: validated.message })
}

export function decodeSettingsPersistence(
  catalog: SettingsCatalog,
  bytes: Uint8Array,
): PersistenceDecodeResult {
  if (!(bytes instanceof Uint8Array)) {
    return decodeFailure("MalformedPersistence", "persistence input must be bytes")
  }
  if (bytes.byteLength > catalog.limits.maximumPersistenceBytes) {
    return decodeFailure("LimitExceeded", "persistence bytes exceed the catalog limit")
  }
  let text: string
  try {
    text = decoder.decode(bytes)
  } catch {
    return decodeFailure("MalformedPersistence", "persistence input is not valid UTF-8")
  }
  if (!hasValidUnicode(text)) return decodeFailure("MalformedPersistence", "persistence text is not valid Unicode")
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return decodeFailure("MalformedPersistence", "persistence input is not valid JSON")
  }
  if (!value || typeof value !== "object" || Array.isArray(value) || !exactKeys(value, ["format", "revision", "catalog", "values"])) {
    return decodeFailure("MalformedPersistence", "persistence document fields are invalid")
  }
  const document = value as Record<string, unknown>
  if (document.format !== FORMAT || document.revision !== REVISION) {
    return decodeFailure("MalformedPersistence", "persistence format or revision is unsupported")
  }
  if (document.catalog !== catalog.identity) {
    return decodeFailure("WrongCatalog", `persistence catalog ${String(document.catalog)} does not match ${catalog.identity}`)
  }
  if (!Array.isArray(document.values)) {
    return decodeFailure("MalformedPersistence", "persistence values must be an array")
  }
  if (document.values.length > catalog.limits.maximumSettings) {
    return decodeFailure("LimitExceeded", "persistence value count exceeds the catalog limit")
  }
  const expected = persistedSettings(catalog)
  const expectedById = new Map(expected.map((schema) => [schema.id, schema]))
  const decoded: Record<string, SettingValue> = Object.create(null)
  const unknown: string[] = []
  for (const entry of document.values) {
    if (!Array.isArray(entry) || entry.length !== 3 || typeof entry[0] !== "string" || typeof entry[1] !== "string") {
      return decodeFailure("MalformedPersistence", "persistence value entry is malformed")
    }
    const [id, kind, raw] = entry
    const schema = expectedById.get(id)
    if (!schema) {
      unknown.push(id)
      continue
    }
    if (Object.hasOwn(decoded, id)) {
      return decodeFailure("DuplicatePersistedSetting", `persisted setting ${id} is duplicated`, { settingId: id })
    }
    if (kind !== schema.kind) {
      return decodeFailure("MalformedPersistence", `persisted setting ${id} kind does not match its schema`, { settingId: id })
    }
    const result = decodeValue(catalog, schema, raw)
    if (!result.ok) {
      return decodeFailure("MalformedPersistence", `persisted setting ${id}: ${result.message}`, { settingId: id })
    }
    decoded[id] = result.value
  }
  if (unknown.length > 0) {
    return decodeFailure("UnknownPersistedSetting", `unknown persisted settings: ${unknown.join(", ")}`, {
      unknownIds: Object.freeze(unknown),
    })
  }
  const missing = expected.filter((schema) => !Object.hasOwn(decoded, schema.id)).map((schema) => schema.id)
  if (missing.length > 0) {
    return decodeFailure("MissingPersistedSetting", `missing persisted settings: ${missing.join(", ")}`, {
      unknownIds: Object.freeze(missing),
    })
  }
  const result: DecodedSettingsPersistence = Object.freeze({
    catalogIdentity: catalog.identity,
    values: freezeSettingValues(decoded),
  })
  return Object.freeze({ ok: true, decoded: result })
}
