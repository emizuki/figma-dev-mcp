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
import { PluginReadError } from "./navigation"
import type { FigmaReadApi, FigmaVariablesApi } from "./common"
import { byteLength, toColor, type SerializerLimits } from "./serialize"

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

function requireVariablesApi(): FigmaVariablesApi {
  const api = figma.variables
  if (
    api === undefined ||
    api.getLocalVariableCollectionsAsync === undefined ||
    api.getLocalVariablesAsync === undefined ||
    api.getVariableByIdAsync === undefined
  ) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  return api
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

  rememberVariable(raw: unknown): void {
    const id = string(record(raw).id)
    if (id.length > 0 && !this.variables.has(id)) this.variables.set(id, raw)
  }

  rememberCollection(raw: unknown): void {
    const id = string(record(raw).id)
    if (id.length > 0 && !this.collections.has(id))
      this.collections.set(id, raw)
  }

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
    const raw = await lookup.call(this.api, id)
    const value = raw ?? null
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
    const raw = await lookup.call(this.api, id)
    const value = raw ?? null
    this.collections.set(id, value)
    return value
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

export async function getVariables(
  input: ReadVariablesInput = {},
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<GetVariablesResult> {
  const startedAt = new Date().toISOString()
  const resolveAliases = input.resolveAliases === true
  const session = new VariableSession()
  const readCollections = session.api.getLocalVariableCollectionsAsync
  const readVariables = session.api.getLocalVariablesAsync
  if (readCollections === undefined || readVariables === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  const collections = await readCollections.call(session.api)
  const variables = await readVariables.call(session.api)
  for (const raw of collections) session.rememberCollection(raw)
  for (const raw of variables) session.rememberVariable(raw)

  const emission = new VariableEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })

  for (let index = 0; index < collections.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    const raw = collections[index]
    signal?.throwIfAborted()
    if (!emission.consider()) break
    const collection = record(raw)
    const id = string(collection.id)
    if (id.length === 0) continue
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
    for (const variableId of array(collection.variableIds)) {
      if (typeof variableId !== "string" || variableId.length === 0) continue
      signal?.throwIfAborted()
      const variable = await session.lookupVariable(variableId, signal)
      if (variable === null) continue
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

  const result: GetVariablesResult = {
    collections: emission.collections,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
