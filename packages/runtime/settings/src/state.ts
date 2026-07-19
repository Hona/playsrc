import {
  SETTING_OWNERS,
  type AdapterRequestResult,
  type ApplyPlan,
  type BindingCapture,
  type BindingValue,
  type ChangeJournalEntry,
  type ChangeJournalKind,
  type OwnerAvailability,
  type RestartDisposition,
  type SettingChange,
  type SettingOwner,
  type SettingSchema,
  type SettingsAdapterRequest,
  type SettingsDiagnostic,
  type SettingsDiagnosticCode,
  type SettingsFailure,
  type SettingsResult,
  type SettingsState,
  type SettingsStateConfiguration,
  type SettingsSuccess,
  type SettingValue,
} from "./contract"
import {
  asciiFold,
  bindingValuesEqual,
  cloneSettingValue,
  defaultSettingValues,
  freezeSettingValues,
  hasValidUnicode,
  normalizeBindingCapture,
  settingById,
  settingValuesEqual,
  utf8Length,
  validateSettingValue,
  validateStoredSettingValue,
} from "./schema"

export class SettingsStateError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SettingsStateError"
  }
}

function diagnostic(
  code: SettingsDiagnosticCode,
  operation: string,
  message: string,
  details: Omit<SettingsDiagnostic, "code" | "operation" | "message"> = {},
): SettingsDiagnostic {
  const clone = { ...details } as Record<string, unknown>
  if (details.unknownIds) clone.unknownIds = Object.freeze([...details.unknownIds])
  return Object.freeze({ code, operation, message, ...clone }) as SettingsDiagnostic
}

function failure(
  code: SettingsDiagnosticCode,
  operation: string,
  message: string,
  details: Omit<SettingsDiagnostic, "code" | "operation" | "message"> = {},
): SettingsFailure {
  return Object.freeze({ ok: false, diagnostic: diagnostic(code, operation, message, details) })
}

function success<T extends object>(value: T): SettingsSuccess<T> {
  return Object.freeze({ ok: true, ...value })
}

function bindingIdentity(value: BindingValue): string {
  return `${value.modifiers}:${asciiFold(value.code)}`
}

function isBinding(value: SettingValue): value is BindingValue {
  return value !== null && typeof value === "object"
}

class SettingsStateImplementation implements SettingsState {
  private readonly catalog: SettingsStateConfiguration["catalog"]
  private readonly current: Record<string, SettingValue>
  private readonly applied: Record<string, SettingValue>
  private pending: Record<string, SettingValue> | null = null
  private activeTransactionId: number | null = null
  private inFlightPlan: ApplyPlan | null = null
  private nextTransactionId = 1
  private nextPlanId = 1
  private nextRequestId = 1
  private nextJournalSequence = 1
  private revision = 0
  private readonly journal: ChangeJournalEntry[] = []
  private readonly owners: Record<SettingOwner, OwnerAvailability> = {
    renderer: "unknown",
    audio: "unknown",
    input: "unknown",
    game: "unknown",
    application: "unknown",
  }

  constructor(configuration: SettingsStateConfiguration) {
    this.catalog = configuration.catalog
    for (const owner of SETTING_OWNERS) {
      const availability = configuration.owners?.[owner]
      if (availability !== undefined) {
        if (!["available", "unavailable", "unknown"].includes(availability)) {
          throw new SettingsStateError(`owner ${owner} availability is invalid`)
        }
        this.owners[owner] = availability
      }
    }
    const values: Record<string, SettingValue> = { ...defaultSettingValues(this.catalog) }
    const initial = configuration.initial ?? {}
    const normalized = this.validateValueSet(initial, "initialize", "stored")
    if (!normalized.ok) throw new SettingsStateError(normalized.diagnostic.message)
    for (const [id, value] of Object.entries(normalized.values)) values[id] = cloneSettingValue(value)
    const conflict = this.bindingConflict(values)
    if (conflict) throw new SettingsStateError(`initial bindings ${conflict[0]} and ${conflict[1]} conflict`)
    this.current = values
    this.applied = Object.fromEntries(
      Object.entries(values).map(([id, value]) => [id, cloneSettingValue(value)]),
    )
  }

  snapshot() {
    const dirty = this.dirtySettingIds()
    const ownerAvailability = Object.freeze({ ...this.owners })
    const journal = Object.freeze(this.journal.map((entry) => entry))
    return Object.freeze({
      catalogIdentity: this.catalog.identity,
      revision: this.revision,
      current: freezeSettingValues(this.current),
      applied: freezeSettingValues(this.applied),
      pending: this.pending === null ? null : freezeSettingValues(this.pending),
      dirtySettingIds: Object.freeze(dirty),
      ownerAvailability,
      activeTransactionId: this.activeTransactionId,
      inFlightPlanId: this.inFlightPlan?.planId ?? null,
      journalStartSequence: journal[0]?.sequence ?? this.nextJournalSequence,
      journal,
    })
  }

  beginTransaction(): SettingsResult<{ transactionId: number; revision: number }> {
    const operation = "begin-transaction"
    if (this.activeTransactionId !== null) {
      return failure("TransactionActive", operation, "a settings transaction is already active", {
        transactionId: this.activeTransactionId,
      })
    }
    const transactionId = this.nextTransactionId
    this.nextTransactionId += 1
    this.activeTransactionId = transactionId
    this.pending = Object.fromEntries(
      Object.entries(this.current).map(([id, value]) => [id, cloneSettingValue(value)]),
    )
    this.record("transaction-began", [], { transactionId })
    return success({ transactionId, revision: this.revision })
  }

  setValue(
    transactionId: number,
    settingId: string,
    value: SettingValue,
  ): SettingsResult<{ revision: number }> {
    const operation = "set-value"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    const schema = settingById(this.catalog, settingId)
    if (!schema) return failure("UnknownSetting", operation, `setting ${String(settingId)} is unknown`, { settingId })
    const validated = validateSettingValue(this.catalog, schema, value)
    if (!validated.ok) {
      return failure("InvalidValue", operation, validated.message, { settingId: schema.id, transactionId })
    }
    if (schema.kind === "binding" && isBinding(validated.value) && this.isReserved(validated.value)) {
      const unchanged = settingValuesEqual(this.pending![schema.id], validated.value)
      if (!unchanged) {
        return failure("ReservedBinding", operation, `binding ${validated.value.code} is reserved`, {
          settingId: schema.id,
          transactionId,
        })
      }
    }
    const staged = this.stageOne(this.pending!, schema, validated.value, operation)
    if (!staged.ok) return staged
    if (!staged.changed) return success({ revision: this.revision })
    const dirty = this.dirtySettingIds()
    if (dirty.length > this.catalog.limits.maximumTransactionChanges) {
      this.pending = staged.before
      return failure("LimitExceeded", operation, "transaction change count exceeds its limit", { transactionId })
    }
    this.record(staged.displacedSettingId ? "binding-replaced" : "value-staged", staged.settingIds, { transactionId })
    return success({ revision: this.revision })
  }

  captureBinding(
    transactionId: number,
    settingId: string,
    capture: BindingCapture,
  ): SettingsResult<{ revision: number; binding: BindingValue; displacedSettingId?: string }> {
    const operation = "capture-binding"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    const schema = settingById(this.catalog, settingId)
    if (!schema) return failure("UnknownSetting", operation, `setting ${String(settingId)} is unknown`, { settingId })
    if (schema.kind !== "binding") {
      return failure("InvalidValue", operation, `setting ${schema.id} is not a binding`, { settingId: schema.id })
    }
    const profile = this.catalog.bindingProfile
    if (!profile) return failure("BindingProfileMissing", operation, "binding profile is unavailable", { settingId: schema.id })
    const normalized = normalizeBindingCapture(profile, capture)
    if (!normalized.ok) {
      return failure(normalized.code, operation, normalized.message, { settingId: schema.id, transactionId })
    }
    const staged = this.stageOne(this.pending!, schema, normalized.binding, operation)
    if (!staged.ok) return staged
    if (staged.changed) {
      const dirty = this.dirtySettingIds()
      if (dirty.length > this.catalog.limits.maximumTransactionChanges) {
        this.pending = staged.before
        return failure("LimitExceeded", operation, "transaction change count exceeds its limit", { transactionId })
      }
      this.record(staged.displacedSettingId ? "binding-replaced" : "value-staged", staged.settingIds, { transactionId })
    }
    return success({
      revision: this.revision,
      binding: normalized.binding,
      ...(staged.displacedSettingId ? { displacedSettingId: staged.displacedSettingId } : {}),
    })
  }

  unbind(transactionId: number, settingId: string): SettingsResult<{ revision: number }> {
    const operation = "unbind"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    const schema = settingById(this.catalog, settingId)
    if (!schema) return failure("UnknownSetting", operation, `setting ${String(settingId)} is unknown`, { settingId })
    if (schema.kind !== "binding") {
      return failure("InvalidValue", operation, `setting ${schema.id} is not a binding`, { settingId: schema.id })
    }
    if (this.pending![schema.id] === null) return success({ revision: this.revision })
    this.pending![schema.id] = null
    this.record("binding-unbound", [schema.id], { transactionId })
    return success({ revision: this.revision })
  }

  reset(transactionId: number, settingIds?: readonly string[]): SettingsResult<{ revision: number }> {
    const operation = "reset"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    if (settingIds !== undefined && !Array.isArray(settingIds)) {
      return failure("InvalidValue", operation, "reset setting identities must be an array", { transactionId })
    }
    const selected = settingIds === undefined ? this.catalog.settings : [] as SettingSchema[]
    if (settingIds !== undefined) {
      const seen = new Set<string>()
      for (const id of settingIds) {
        const schema = settingById(this.catalog, id)
        if (!schema) return failure("UnknownSetting", operation, `setting ${String(id)} is unknown`, { settingId: id })
        const folded = asciiFold(schema.id)
        if (seen.has(folded)) return failure("InvalidValue", operation, `setting ${schema.id} is duplicated`, { settingId: schema.id })
        seen.add(folded)
        ;(selected as SettingSchema[]).push(schema)
      }
    }
    const before = this.cloneMutableValues(this.pending!)
    const changed = new Set<string>()
    for (const schema of selected) {
      const staged = this.stageOne(this.pending!, schema, schema.defaultValue, operation, true)
      if (!staged.ok) {
        this.pending = before
        return staged
      }
      for (const id of staged.settingIds) changed.add(id)
    }
    if (this.dirtySettingIds().length > this.catalog.limits.maximumTransactionChanges) {
      this.pending = before
      return failure("LimitExceeded", operation, "transaction change count exceeds its limit", { transactionId })
    }
    if (changed.size > 0) this.record("defaults-staged", this.schemaOrdered([...changed]), { transactionId })
    return success({ revision: this.revision })
  }

  stagePersistence(
    transactionId: number,
    values: Readonly<Record<string, SettingValue>>,
  ): SettingsResult<{ revision: number }> {
    const operation = "stage-persistence"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    const validated = this.validateValueSet(values, operation, "stored")
    if (!validated.ok) return validated
    const before = this.cloneMutableValues(this.pending!)
    const draft = this.cloneMutableValues(this.pending!)
    for (const [id, value] of Object.entries(validated.values)) draft[id] = cloneSettingValue(value)
    const conflict = this.bindingConflict(draft)
    if (conflict) {
      return failure("BindingConflict", operation, `persisted bindings ${conflict[0]} and ${conflict[1]} conflict`, {
        settingId: conflict[0],
        conflictingSettingId: conflict[1],
        transactionId,
      })
    }
    const changed = this.catalog.settings
      .filter((schema) => !settingValuesEqual(this.pending![schema.id], draft[schema.id]))
      .map((schema) => schema.id)
    this.pending = draft
    if (this.dirtySettingIds().length > this.catalog.limits.maximumTransactionChanges) {
      this.pending = before
      return failure("LimitExceeded", operation, "transaction change count exceeds its limit", { transactionId })
    }
    if (changed.length > 0) this.record("persistence-staged", changed, { transactionId })
    return success({ revision: this.revision })
  }

  cancel(transactionId: number): SettingsResult<{ revision: number }> {
    const operation = "cancel"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    const settingIds = this.dirtySettingIds()
    this.pending = null
    this.activeTransactionId = null
    this.record("transaction-cancelled", settingIds, { transactionId })
    return success({ revision: this.revision })
  }

  prepareApply(transactionId: number): SettingsResult<{ plan: ApplyPlan }> {
    const operation = "prepare-apply"
    const active = this.requireMutableTransaction(transactionId, operation)
    if (!active.ok) return active
    const dirty = this.dirtySettingIds()
    const dirtySchemas = this.catalog.settings.filter((schema) => dirty.includes(schema.id))
    for (const owner of SETTING_OWNERS) {
      if (!dirtySchemas.some((schema) => schema.owner === owner)) continue
      if (this.owners[owner] === "unknown") {
        return failure("OwnerUnknown", operation, `owner ${owner} availability is unknown`, { owner, transactionId })
      }
      if (this.owners[owner] === "unavailable") {
        return failure("OwnerUnavailable", operation, `owner ${owner} is unavailable`, { owner, transactionId })
      }
    }
    const planId = this.nextPlanId
    this.nextPlanId += 1
    const requests: SettingsAdapterRequest[] = []
    for (const owner of SETTING_OWNERS) {
      const schemas = dirtySchemas.filter((schema) => schema.owner === owner)
      if (schemas.length === 0) continue
      const changes: SettingChange[] = schemas.map((schema) => Object.freeze({
        settingId: schema.id,
        kind: schema.kind,
        previousValue: cloneSettingValue(this.current[schema.id]),
        nextValue: cloneSettingValue(this.pending![schema.id]),
        consoleNames: Object.freeze([...schema.consoleNames]),
        restart: schema.restart,
      }))
      const requestId = this.nextRequestId
      this.nextRequestId += 1
      requests.push(Object.freeze({
        kind: `${owner}-settings`,
        owner,
        requestId,
        planId,
        transactionId,
        changes: Object.freeze(changes),
      }))
    }
    const plan = Object.freeze({
      planId,
      transactionId,
      baseRevision: this.revision,
      requests: Object.freeze(requests),
    })
    this.inFlightPlan = plan
    this.record("apply-prepared", dirty, {
      transactionId,
      planId,
      requestIds: requests.map((request) => request.requestId),
    })
    return success({ plan })
  }

  settleApply(
    planId: number,
    results: readonly AdapterRequestResult[],
  ): SettingsResult<{
    revision: number
    complete: boolean
    rejectedOwners: readonly SettingOwner[]
    rejections: readonly Readonly<{ owner: SettingOwner; reason: string }>[]
    restart: readonly RestartDisposition[]
  }> {
    const operation = "settle-apply"
    const plan = this.inFlightPlan
    if (!plan || plan.planId !== planId) {
      return failure("StaleApplyPlan", operation, `apply plan ${String(planId)} is not active`, { planId })
    }
    if (!Array.isArray(results) || results.length !== plan.requests.length) {
      return failure("MalformedApplyResults", operation, "apply results must cover every request exactly once", { planId })
    }
    const resultByRequest = new Map<number, AdapterRequestResult>()
    const requestIds = new Set(plan.requests.map((request) => request.requestId))
    for (const result of results) {
      if (
        !result
        || typeof result !== "object"
        || !Number.isSafeInteger(result.requestId)
        || !requestIds.has(result.requestId)
        || resultByRequest.has(result.requestId)
        || !["applied", "rejected"].includes(result.status)
        || (result.status === "rejected"
          && (typeof result.reason !== "string"
            || !hasValidUnicode(result.reason)
            || utf8Length(result.reason) > this.catalog.limits.maximumStringUtf8Bytes))
      ) {
        return failure("MalformedApplyResults", operation, "apply result is malformed, duplicated, or unknown", { planId })
      }
      resultByRequest.set(result.requestId, result)
    }
    const rejectedOwners: SettingOwner[] = []
    const rejections: Readonly<{ owner: SettingOwner; reason: string }>[] = []
    const restart = new Set<RestartDisposition>()
    const committed: string[] = []
    for (const request of plan.requests) {
      const result = resultByRequest.get(request.requestId)!
      if (result.status === "rejected") {
        rejectedOwners.push(request.owner)
        rejections.push(Object.freeze({ owner: request.owner, reason: result.reason }))
        continue
      }
      for (const change of request.changes) {
        this.current[change.settingId] = cloneSettingValue(change.nextValue)
        this.applied[change.settingId] = cloneSettingValue(change.nextValue)
        committed.push(change.settingId)
        if (change.restart !== "live") restart.add(change.restart)
      }
    }
    this.inFlightPlan = null
    const complete = this.dirtySettingIds().length === 0
    if (complete) {
      this.pending = null
      this.activeTransactionId = null
    }
    this.record("apply-settled", committed, {
      transactionId: plan.transactionId,
      planId,
      requestIds: plan.requests.map((request) => request.requestId),
    })
    return success({
      revision: this.revision,
      complete,
      rejectedOwners: Object.freeze(rejectedOwners),
      rejections: Object.freeze(rejections),
      restart: Object.freeze([...restart]),
    })
  }

  setOwnerAvailability(
    owner: SettingOwner,
    availability: OwnerAvailability,
  ): SettingsResult<{ revision: number }> {
    const operation = "set-owner-availability"
    if (!SETTING_OWNERS.includes(owner) || !["available", "unavailable", "unknown"].includes(availability)) {
      return failure("InvalidValue", operation, "owner or availability is invalid")
    }
    if (this.inFlightPlan) {
      return failure("ApplyInFlight", operation, "owner availability cannot change while apply is in flight", {
        planId: this.inFlightPlan.planId,
      })
    }
    if (this.owners[owner] === availability) return success({ revision: this.revision })
    this.owners[owner] = availability
    this.record("owner-availability-changed", [], {})
    return success({ revision: this.revision })
  }

  synchronize(values: Readonly<Record<string, SettingValue>>): SettingsResult<{ revision: number }> {
    const operation = "synchronize"
    if (this.activeTransactionId !== null) {
      return failure("TransactionActive", operation, "current values cannot synchronize during a transaction", {
        transactionId: this.activeTransactionId,
      })
    }
    const validated = this.validateValueSet(values, operation, "stored")
    if (!validated.ok) return validated
    const draft = this.cloneMutableValues(this.current)
    for (const [id, value] of Object.entries(validated.values)) draft[id] = cloneSettingValue(value)
    const conflict = this.bindingConflict(draft)
    if (conflict) {
      return failure("BindingConflict", operation, `bindings ${conflict[0]} and ${conflict[1]} conflict`, {
        settingId: conflict[0],
        conflictingSettingId: conflict[1],
      })
    }
    const changed = this.catalog.settings
      .filter((schema) => !settingValuesEqual(this.current[schema.id], draft[schema.id]))
      .map((schema) => schema.id)
    if (changed.length === 0) return success({ revision: this.revision })
    for (const id of changed) {
      this.current[id] = cloneSettingValue(draft[id])
    }
    this.record("current-synchronized", changed, {})
    return success({ revision: this.revision })
  }

  private requireMutableTransaction(transactionId: number, operation: string): SettingsResult {
    if (this.activeTransactionId === null || this.pending === null) {
      return failure("NoActiveTransaction", operation, "no settings transaction is active", { transactionId })
    }
    if (!Number.isSafeInteger(transactionId) || transactionId !== this.activeTransactionId) {
      return failure("StaleTransaction", operation, `transaction ${String(transactionId)} is not active`, { transactionId })
    }
    if (this.inFlightPlan) {
      return failure("ApplyInFlight", operation, "settings cannot mutate while apply is in flight", {
        transactionId,
        planId: this.inFlightPlan.planId,
      })
    }
    return success({})
  }

  private stageOne(
    draft: Record<string, SettingValue>,
    schema: SettingSchema,
    value: SettingValue,
    operation: string,
    stored = false,
  ): SettingsResult<{
    changed: boolean
    before: Record<string, SettingValue>
    settingIds: readonly string[]
    displacedSettingId?: string
  }> {
    const normalized = stored
      ? validateStoredSettingValue(this.catalog, schema, value)
      : validateSettingValue(this.catalog, schema, value)
    if (!normalized.ok) return failure("InvalidValue", operation, normalized.message, { settingId: schema.id })
    const before = this.cloneMutableValues(draft)
    const changedIds = new Set<string>()
    let displacedSettingId: string | undefined
    if (schema.kind === "binding" && isBinding(normalized.value)) {
      const conflict = this.catalog.settings.find((candidate) =>
        candidate.kind === "binding"
        && candidate.id !== schema.id
        && isBinding(draft[candidate.id])
        && bindingValuesEqual(draft[candidate.id] as BindingValue, normalized.value as BindingValue)
      )
      if (conflict) {
        if (this.catalog.bindingProfile?.conflictPolicy === "reject") {
          return failure("BindingConflict", operation, `binding is already assigned to ${conflict.id}`, {
            settingId: schema.id,
            conflictingSettingId: conflict.id,
            transactionId: this.activeTransactionId ?? undefined,
          })
        }
        draft[conflict.id] = null
        displacedSettingId = conflict.id
        changedIds.add(conflict.id)
      }
    }
    if (!settingValuesEqual(draft[schema.id], normalized.value)) {
      draft[schema.id] = cloneSettingValue(normalized.value)
      changedIds.add(schema.id)
    }
    return success({
      changed: changedIds.size > 0,
      before,
      settingIds: Object.freeze(this.schemaOrdered([...changedIds])),
      ...(displacedSettingId ? { displacedSettingId } : {}),
    })
  }

  private validateValueSet(
    values: Readonly<Record<string, SettingValue>>,
    operation: string,
    validation: "edit" | "stored",
  ): SettingsResult<{ values: Readonly<Record<string, SettingValue>> }> {
    if (!values || typeof values !== "object" || Array.isArray(values)) {
      return failure("InvalidValue", operation, "setting values must be a record")
    }
    const entries = Object.entries(values)
    if (entries.length > this.catalog.limits.maximumSettings) {
      return failure("LimitExceeded", operation, "setting value count exceeds its limit")
    }
    const result: Record<string, SettingValue> = Object.create(null)
    const seen = new Set<string>()
    const unknown: string[] = []
    for (const [candidateId, value] of entries) {
      const schema = settingById(this.catalog, candidateId)
      if (!schema) {
        unknown.push(candidateId)
        continue
      }
      const folded = asciiFold(schema.id)
      if (seen.has(folded)) {
        return failure("InvalidValue", operation, `setting ${schema.id} is duplicated`, { settingId: schema.id })
      }
      seen.add(folded)
      const validated = validation === "stored"
        ? validateStoredSettingValue(this.catalog, schema, value)
        : validateSettingValue(this.catalog, schema, value)
      if (!validated.ok) {
        return failure("InvalidValue", operation, validated.message, { settingId: schema.id })
      }
      result[schema.id] = cloneSettingValue(validated.value)
    }
    if (unknown.length > 0) {
      return failure("UnknownSetting", operation, `unknown settings: ${unknown.join(", ")}`, {
        unknownIds: Object.freeze(unknown),
      })
    }
    return success({ values: freezeSettingValues(result) })
  }

  private bindingConflict(values: Readonly<Record<string, SettingValue>>): readonly [string, string] | null {
    const seen = new Map<string, string>()
    for (const schema of this.catalog.settings) {
      if (schema.kind !== "binding") continue
      const value = values[schema.id]
      if (!isBinding(value)) continue
      const identity = bindingIdentity(value)
      const prior = seen.get(identity)
      if (prior) return Object.freeze([prior, schema.id])
      seen.set(identity, schema.id)
    }
    return null
  }

  private isReserved(value: BindingValue): boolean {
    return this.catalog.bindingProfile?.reserved.some((candidate) => bindingValuesEqual(candidate, value)) ?? false
  }

  private dirtySettingIds(): string[] {
    if (this.pending === null) return []
    return this.catalog.settings
      .filter((schema) => !settingValuesEqual(this.current[schema.id], this.pending![schema.id]))
      .map((schema) => schema.id)
  }

  private schemaOrdered(settingIds: readonly string[]): string[] {
    const selected = new Set(settingIds)
    return this.catalog.settings.filter((schema) => selected.has(schema.id)).map((schema) => schema.id)
  }

  private cloneMutableValues(values: Readonly<Record<string, SettingValue>>): Record<string, SettingValue> {
    return Object.fromEntries(Object.entries(values).map(([id, value]) => [id, cloneSettingValue(value)]))
  }

  private record(
    kind: ChangeJournalKind,
    settingIds: readonly string[],
    details: Readonly<{
      transactionId?: number
      planId?: number
      requestIds?: readonly number[]
    }>,
  ): void {
    this.revision += 1
    const entry = Object.freeze({
      sequence: this.nextJournalSequence,
      revision: this.revision,
      kind,
      settingIds: Object.freeze([...settingIds]),
      ...(details.transactionId !== undefined ? { transactionId: details.transactionId } : {}),
      ...(details.planId !== undefined ? { planId: details.planId } : {}),
      ...(details.requestIds !== undefined ? { requestIds: Object.freeze([...details.requestIds]) } : {}),
    }) as ChangeJournalEntry
    this.nextJournalSequence += 1
    this.journal.push(entry)
    while (this.journal.length > this.catalog.limits.maximumJournalEntries) this.journal.shift()
  }
}

export function createSettingsState(configuration: SettingsStateConfiguration): SettingsState {
  if (!configuration || typeof configuration !== "object" || !configuration.catalog) {
    throw new SettingsStateError("settings state configuration is missing")
  }
  if (
    !Object.isFrozen(configuration.catalog)
    || !Array.isArray(configuration.catalog.settings)
    || !Object.isFrozen(configuration.catalog.settings)
  ) {
    throw new SettingsStateError("settings catalog must be created by defineSettingsCatalog")
  }
  const implementation = new SettingsStateImplementation(configuration)
  const facade: SettingsState = {
    snapshot: () => implementation.snapshot(),
    beginTransaction: () => implementation.beginTransaction(),
    setValue: (transactionId, settingId, value) => implementation.setValue(transactionId, settingId, value),
    captureBinding: (transactionId, settingId, capture) => implementation.captureBinding(transactionId, settingId, capture),
    unbind: (transactionId, settingId) => implementation.unbind(transactionId, settingId),
    reset: (transactionId, settingIds) => implementation.reset(transactionId, settingIds),
    stagePersistence: (transactionId, values) => implementation.stagePersistence(transactionId, values),
    cancel: (transactionId) => implementation.cancel(transactionId),
    prepareApply: (transactionId) => implementation.prepareApply(transactionId),
    settleApply: (planId, results) => implementation.settleApply(planId, results),
    setOwnerAvailability: (owner, availability) => implementation.setOwnerAvailability(owner, availability),
    synchronize: (values) => implementation.synchronize(values),
  }
  return Object.freeze(facade)
}
