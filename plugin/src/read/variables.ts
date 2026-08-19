import type { ErrorCode, ScopedInput } from "../shared/protocol"
import type {
  CodeSyntax,
  GetVariablesResult,
  Truncation,
  VariableCollection,
  VariableDefinition,
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

class VariableSession {
  readonly variables = new Map<string, unknown | null>()
  readonly collections = new Map<string, unknown | null>()
  readonly memo = new Map<string, AliasResolution>()
  readonly api: FigmaVariablesApi

  constructor() {
    this.api = requireVariablesApi()
  }

  // Every host lookup below is wrapped in settleOrSkip: a bound id can name a
  // variable in a library that is slow or unreachable, and one of those must
  // not hang the read. A skipped lookup is cached as null like a genuine miss,
  // so a stalled library costs the timeout once, not once per reference.
  async lookupVariable(
    id: string,
    signal?: CancellationSignal,
  ): Promise<unknown | null> {
    signal?.throwIfAborted()
    if (this.variables.has(id)) return this.variables.get(id) ?? null
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
  const modes = array(collection.modes)
  const values: VariableModeValue[] = []
  for (const mode of modes) {
    const modeRecord = record(mode)
    const modeId = string(modeRecord.modeId)
    if (modeId.length === 0 || !Object.hasOwn(valuesByMode, modeId)) continue
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
  const session = new VariableSession()

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

  const budgetMs =
    limits?.variableLookupBudgetMs ?? DEFAULT_VARIABLE_LOOKUP_BUDGET_MS
  const budgetStarted = Date.now()
  let budgetTruncation: Truncation | undefined

  // One lookup per unique id, grouped by the collection each variable declares.
  const grouped = new Map<string, unknown[]>()
  const collectionOrder: string[] = []
  for (let index = 0; index < pending.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    if (Date.now() - budgetStarted >= budgetMs) {
      budgetTruncation = { reason: "nodeLimit", visitedNodes: index }
      break
    }
    const raw = await session.lookupVariable(pending[index] as string, signal)
    if (raw === null) continue
    const collectionId = string(record(raw).variableCollectionId)
    const group = grouped.get(collectionId)
    if (group === undefined) {
      grouped.set(collectionId, [raw])
      collectionOrder.push(collectionId)
    } else group.push(raw)
  }

  for (let index = 0; index < collectionOrder.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    if (!emission.consider()) break
    if (
      budgetTruncation === undefined &&
      Date.now() - budgetStarted >= budgetMs
    ) {
      budgetTruncation = { reason: "nodeLimit", visitedNodes: index }
      break
    }
    const id = collectionOrder[index] as string
    if (id.length === 0) continue
    // Resolved once per distinct collection id, and memoised on the session, so
    // readDefinition's own lookup of the same id costs nothing.
    const collection = record(await session.lookupCollection(id, signal))
    const definition: VariableCollection = {
      id,
      name: string(collection.name),
      modes: array(collection.modes).flatMap((mode) => {
        const item = record(mode)
        const modeId = string(item.modeId)
        if (modeId.length === 0) return []
        return [{ id: modeId, name: string(item.name) }]
      }),
      variables: [],
    }
    for (const variable of grouped.get(id) ?? []) {
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
  // them this way: it is the earlier, larger loss.
  const truncation =
    walked.truncation ?? budgetTruncation ?? emission.truncation
  const result: GetVariablesResult = {
    collections: emission.collections,
    truncated: truncation !== undefined,
    observation: observation(startedAt),
  }
  if (truncation !== undefined) result.truncation = truncation
  return result
}
