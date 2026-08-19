import type { ErrorCode, ScopedInput } from "../shared/protocol"
import type {
  CodeSyntax,
  GetVariablesResult,
  Truncation,
  VariableCollection,
  VariableDefinition,
  VariableMode,
  VariableModeValue,
  VariableValue,
} from "../shared/results"
import {
  CANCEL_CHECK_BATCH,
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import {
  throwIfAbortedAtBatch,
  type CancellationSignal,
} from "../main/cancellation"
import { PluginReadError, resolveDesignRoots } from "./navigation"
import {
  settleOrSkip,
  type FigmaReadApi,
  type FigmaVariablesApi,
} from "./common"
import {
  byteLength,
  toColor,
  variableIdsOf,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

export interface ReadVariablesInput extends ScopedInput {
  resolveAliases?: boolean
}

type AliasResolution =
  | { status: "success"; value: VariableValue }
  | { status: "error"; error: { code: ErrorCode; retryable: boolean } }

const EXHAUSTED: AliasResolution = {
  status: "error",
  error: { code: "LIMIT_EXCEEDED", retryable: true },
}

declare const figma: FigmaReadApi

type UnknownRecord = Record<string, unknown>

const CODE_SYNTAX_PLATFORMS = ["WEB", "ANDROID", "iOS"] as const

function observation(startedAt: string) {
  return { startedAt, completedAt: new Date().toISOString() }
}

function isRecord(value: unknown): value is UnknownRecord {
  return value !== null && typeof value === "object"
}

function record(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {}
}

function string(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback
}

function array(value: unknown): readonly unknown[] {
  return Array.isArray(value) ? value : []
}

// The scope walk resolves every bound id through getVariableByIdAsync and never
// enumerates local storage, so that one method — not the local enumerators — is
// what this tool cannot work without. Both live on figma.variables, not figma.
function requireVariablesApi(): FigmaVariablesApi {
  const api = figma.variables
  if (api === undefined || api.getVariableByIdAsync === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  return api
}

function walkOptions(
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): ForestWalkOptions {
  if (signal !== undefined && limits !== undefined) return { signal, limits }
  if (signal !== undefined) return { signal }
  if (limits !== undefined) return { limits }
  return {}
}

function isAlias(value: unknown): value is { id: string } {
  const alias = record(value)
  return alias.type === "VARIABLE_ALIAS" && typeof alias.id === "string"
}

function isColor(value: unknown): boolean {
  if (!isRecord(value) || isAlias(value)) return false
  return (
    typeof value.r === "number" &&
    typeof value.g === "number" &&
    typeof value.b === "number"
  )
}

function normalizeValue(value: unknown): VariableValue | undefined {
  if (typeof value === "boolean") return { kind: "boolean", value }
  if (typeof value === "number" && Number.isFinite(value)) {
    return { kind: "float", value }
  }
  if (typeof value === "string") return { kind: "string", value }
  if (isAlias(value)) return { kind: "alias", value: value.id }
  if (isColor(value)) return { kind: "color", value: toColor(value) }
  return undefined
}

function codeSyntax(value: unknown): CodeSyntax[] {
  const syntax = record(value)
  const result: CodeSyntax[] = []
  for (const platform of CODE_SYNTAX_PLATFORMS) {
    const code = syntax[platform]
    if (typeof code === "string") result.push({ platform, code })
  }
  return result
}

function scopes(value: unknown): string[] {
  return array(value).flatMap((scope) =>
    typeof scope === "string" ? [scope] : [],
  )
}

function memoKey(variableId: string, modeId: string): string {
  return `${variableId}\0${modeId}`
}

// Mode ids are host-supplied identifiers — the keys of valuesByMode — not
// inferences, so reporting them when the collection itself is unreachable states
// what the host said rather than guessing. Mode *names* stay empty, because
// those would be a guess.
function valueModeIds(valuesByMode: UnknownRecord): string[] {
  try {
    return Object.keys(valuesByMode).filter((modeId) => modeId.length > 0)
  } catch {
    return []
  }
}

// One wall-clock deadline shared by every host lookup this read makes, alias
// chains included: an alias target lives in the same library as the variable
// that aliases it, so a budget that stops at the top level stops nothing.
class LookupBudget {
  private readonly deadline: number
  private tripped = false

  constructor(budgetMs: number) {
    this.deadline = Date.now() + budgetMs
  }

  get exhausted(): boolean {
    return this.tripped
  }

  expired(): boolean {
    if (!this.tripped && Date.now() >= this.deadline) this.tripped = true
    return this.tripped
  }
}

class VariableSession {
  readonly variables = new Map<string, unknown | null>()
  readonly collections = new Map<string, unknown | null>()
  readonly memo = new Map<string, AliasResolution>()
  readonly api: FigmaVariablesApi

  constructor(readonly budget: LookupBudget) {
    this.api = requireVariablesApi()
  }

  // Every host lookup below is wrapped in settleOrSkip: a bound id can name a
  // variable in a library that is slow or unreachable, and one of those must
  // not hang the read. A skipped lookup is cached as null like a genuine miss,
  // so a stalled library costs the timeout once, not once per reference.
  // These two methods are also the single gate the budget is enforced at, so no
  // caller — including the recursive alias resolver — can reach the host after
  // the deadline. Worst-case overshoot is therefore one settleOrSkip timeout.
  async lookupVariable(
    id: string,
    signal?: CancellationSignal,
  ): Promise<unknown | null> {
    signal?.throwIfAborted()
    if (this.variables.has(id)) return this.variables.get(id) ?? null
    if (this.budget.expired()) return null
    const lookup = this.api.getVariableByIdAsync
    if (lookup === undefined) {
      throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
    }
    const value = await this.settle(() => lookup.call(this.api, id))
    this.variables.set(id, value)
    return value
  }

  async lookupCollection(
    id: string,
    signal?: CancellationSignal,
  ): Promise<unknown | null> {
    signal?.throwIfAborted()
    if (this.collections.has(id)) return this.collections.get(id) ?? null
    if (this.budget.expired()) return null
    const lookup = this.api.getVariableCollectionByIdAsync
    if (lookup === undefined) return null
    const value = await this.settle(() => lookup.call(this.api, id))
    this.collections.set(id, value)
    return value
  }

  private async settle(call: () => Promise<unknown>): Promise<unknown | null> {
    try {
      // The host call is made inside the try on purpose: settleOrSkip only
      // handles the rejection of an already-constructed promise, so a
      // synchronous throw from the host API would escape it entirely.
      return (await settleOrSkip(call())) ?? null
    } catch {
      return null
    }
  }

  async resolve(
    variableId: string,
    modeId: string,
    stack: Set<string>,
    signal?: CancellationSignal,
  ): Promise<AliasResolution> {
    const key = memoKey(variableId, modeId)
    const cached = this.memo.get(key)
    if (cached !== undefined) return cached
    // Stopping on the clock is not the same as a missing target, and it is
    // retryable where a cycle or a missing target is not. Never memoised: the
    // answer is about when we asked, not about the variable.
    if (this.budget.expired()) return EXHAUSTED
    if (stack.has(key)) {
      return {
        status: "error",
        error: { code: "LIMIT_EXCEEDED", retryable: false },
      }
    }
    stack.add(key)
    const raw = await this.lookupVariable(variableId, signal)
    if (raw === null) {
      stack.delete(key)
      // The deadline can pass while this very lookup is in flight.
      if (this.budget.exhausted) return EXHAUSTED
      const missing: AliasResolution = {
        status: "error",
        error: { code: "NODE_NOT_FOUND", retryable: false },
      }
      this.memo.set(key, missing)
      return missing
    }
    const variable = record(raw)
    const values = record(variable.valuesByMode)
    let resolvedModeId = modeId
    let source = values[resolvedModeId]
    if (source === undefined) {
      const collection = await this.lookupCollection(
        string(variable.variableCollectionId),
        signal,
      )
      resolvedModeId = string(record(collection).defaultModeId, modeId)
      source = values[resolvedModeId]
    }
    let result: AliasResolution
    if (source === undefined) {
      result = {
        status: "error",
        error: { code: "NODE_NOT_FOUND", retryable: false },
      }
    } else if (isAlias(source)) {
      result = await this.resolve(source.id, resolvedModeId, stack, signal)
    } else {
      const value = normalizeValue(source)
      result =
        value === undefined
          ? {
              status: "error",
              error: { code: "NODE_NOT_FOUND", retryable: false },
            }
          : { status: "success", value }
    }
    stack.delete(key)
    this.memo.set(key, result)
    return result
  }
}

class VariableEmission {
  readonly collections: VariableCollection[] = []
  returned = 0
  encoded = 0
  visited = 0
  truncation?: Truncation

  constructor(readonly limits: SerializerLimits) {}

  consider(): boolean {
    this.visited += 1
    if (this.truncation !== undefined) return false
    if (this.returned >= this.limits.returnedNodes) {
      this.truncation = {
        reason: "nodeLimit",
        visitedNodes: this.visited,
      }
      return false
    }
    return true
  }

  pushCollection(collection: VariableCollection): boolean {
    const encoded = this.encoded + byteLength(collection)
    if (encoded > this.limits.encodedBytes) {
      this.truncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    this.returned += 1
    this.collections.push(collection)
    return true
  }
}

async function readDefinition(
  session: VariableSession,
  raw: unknown,
  resolveAliases: boolean,
  signal?: CancellationSignal,
): Promise<VariableDefinition | undefined> {
  const variable = record(raw)
  const id = string(variable.id)
  if (id.length === 0) return undefined
  const valuesByMode = record(variable.valuesByMode)
  const collection = record(
    await session.lookupCollection(
      string(variable.variableCollectionId),
      signal,
    ),
  )
  const declared = array(collection.modes).flatMap((mode) => {
    const modeId = string(record(mode).modeId)
    return modeId.length === 0 ? [] : [modeId]
  })
  // A collection the host will not return by id must not silently reduce every
  // one of its variables to no values at all: fall back to the mode ids the
  // variable itself carries. Same source of truth as the collection envelope.
  const modeIds = declared.length > 0 ? declared : valueModeIds(valuesByMode)
  const values: VariableModeValue[] = []
  for (const modeId of modeIds) {
    if (!Object.hasOwn(valuesByMode, modeId)) continue
    const source = normalizeValue(valuesByMode[modeId])
    if (source === undefined) continue
    const entry: VariableModeValue = { modeId, source }
    if (resolveAliases && source.kind === "alias") {
      const resolved = await session.resolve(
        source.value,
        modeId,
        new Set(),
        signal,
      )
      if (resolved.status === "success") entry.resolved = resolved.value
      else entry.error = resolved.error
    }
    values.push(entry)
  }
  return {
    id,
    name: string(variable.name),
    collectionId: string(variable.variableCollectionId),
    scopes: scopes(variable.scopes),
    values,
    codeSyntax: codeSyntax(variable.codeSyntax),
  }
}

// Union of the mode ids the group's variables actually carry, in first-seen
// order. Names are left empty rather than invented: an id is what the host said,
// a name would be a guess.
function fallbackModes(group: readonly unknown[]): VariableMode[] {
  const seen = new Set<string>()
  const modes: VariableMode[] = []
  for (const raw of group) {
    for (const modeId of valueModeIds(record(record(raw).valuesByMode))) {
      if (seen.has(modeId)) continue
      seen.add(modeId)
      modes.push({ id: modeId, name: "" })
    }
  }
  return modes
}

export interface GetVariablesLimits extends Partial<SerializerLimits> {
  readonly variableLookupBudgetMs?: number
}

// The whole payload of this tool is host lookups, so it gets the same wall-clock
// ceiling as get_components' main-component pass rather than the tighter one the
// name pre-passes use, where a missing name is only a cosmetic loss.
const DEFAULT_VARIABLE_LOOKUP_BUDGET_MS = 8_000

export async function getVariables(
  input: ReadVariablesInput = {},
  signal?: CancellationSignal,
  limits?: GetVariablesLimits,
): Promise<GetVariablesResult> {
  const startedAt = new Date().toISOString()
  const resolveAliases = input.resolveAliases === true
  // The clock starts before the scope is resolved, not after: the ceiling exists
  // to keep the whole operation inside the broker's inactivity timeout, and the
  // node lookups behind a selector are part of the same operation.
  const budget = new LookupBudget(
    limits?.variableLookupBudgetMs ?? DEFAULT_VARIABLE_LOOKUP_BUDGET_MS,
  )
  const session = new VariableSession(budget)

  const emission = new VariableEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })

  // A caller asks get_variables what the design in front of them is built from.
  // Local storage answers a different question: library variables are bound by
  // the thousand and stored locally by none, so enumerating locally returned a
  // set that provably did not intersect what the same scope actually binds.
  const roots = await resolveDesignRoots(input.selector, signal)
  const pending: string[] = []
  const seen = new Set<string>()
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    // variableIdsOf is the single definition of "which variables does this node
    // reference"; a second walker here would be free to drift from it.
    for (const id of variableIdsOf(record(raw))) {
      if (seen.has(id)) continue
      seen.add(id)
      pending.push(id)
    }
  })

  // One lookup per unique id, grouped by the collection each variable declares.
  const grouped = new Map<string, unknown[]>()
  const collectionOrder: string[] = []
  let attempted = 0
  for (let index = 0; index < pending.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    // The lookups themselves are gated too; breaking here just avoids spinning
    // through the remainder of a long id list once the answer cannot change.
    if (budget.expired()) break
    attempted += 1
    const raw = await session.lookupVariable(pending[index] as string, signal)
    if (raw === null) continue
    const collectionId = string(record(raw).variableCollectionId)
    const group = grouped.get(collectionId)
    if (group === undefined) {
      grouped.set(collectionId, [raw])
      collectionOrder.push(collectionId)
    } else group.push(raw)
  }

  // Not a lie by omission: a collection the host would not return by id costs
  // the caller its name and its mode names, and that is a loss worth reporting
  // even though the variables underneath it are still emitted.
  let unresolvedCollections = 0
  for (let index = 0; index < collectionOrder.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    if (!emission.consider()) break
    const id = collectionOrder[index] as string
    if (id.length === 0) continue
    const group = grouped.get(id) ?? []
    // Resolved once per distinct collection id, and memoised on the session, so
    // readDefinition's own lookup of the same id costs nothing. Past the
    // deadline this returns null without touching the host, and the fallback
    // below is what keeps the payload useful rather than empty.
    const raw = await session.lookupCollection(id, signal)
    if (raw === null) unresolvedCollections += 1
    const collection = record(raw)
    const declared = array(collection.modes).flatMap((mode) => {
      const item = record(mode)
      const modeId = string(item.modeId)
      if (modeId.length === 0) return []
      return [{ id: modeId, name: string(item.name) }]
    })
    const definition: VariableCollection = {
      id,
      name: string(collection.name),
      modes: declared.length > 0 ? declared : fallbackModes(group),
      variables: [],
    }
    for (const variable of group) {
      signal?.throwIfAborted()
      const serialized = await readDefinition(
        session,
        variable,
        resolveAliases,
        signal,
      )
      if (serialized !== undefined) definition.variables.push(serialized)
    }
    if (!emission.pushCollection(definition)) break
  }

  // A walk or budget that ran out named the reason the id set is incomplete,
  // which outranks the emission cut for the same reason get_components orders
  // them this way: it is the earlier, larger loss. An unresolved collection is
  // the smallest loss of the four and is reported only when nothing else was.
  // nodeLimit is the only vocabulary the schema offers for "you were given less
  // than exists"; there is no reason code for "the host would not answer".
  const budgetTruncation: Truncation | undefined = budget.exhausted
    ? { reason: "nodeLimit", visitedNodes: attempted }
    : undefined
  const unresolvedTruncation: Truncation | undefined =
    unresolvedCollections > 0
      ? { reason: "nodeLimit", visitedNodes: emission.visited }
      : undefined
  const truncation =
    walked.truncation ??
    budgetTruncation ??
    emission.truncation ??
    unresolvedTruncation
  const result: GetVariablesResult = {
    collections: emission.collections,
    truncated: truncation !== undefined,
    observation: observation(startedAt),
  }
  if (truncation !== undefined) result.truncation = truncation
  return result
}
