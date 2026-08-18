import type {
  SearchMatchMode,
  SearchNodesInput,
  SearchScope,
} from "../shared/protocol"
import type {
  NodeMatch,
  NodeSummary,
  SearchNodesResult,
  Truncation,
} from "../shared/results"
import type { CancellationSignal } from "../main/cancellation"
import {
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import { PluginReadError } from "./navigation"
import { loadPageIfNeeded, type FigmaReadApi } from "./common"
import { byteLength, type SerializerLimits } from "./serialize"

export type NodeType = string
export type MatchReason = "name" | "nodeType" | "text"

export interface SearchPredicate {
  query?: string
  types?: readonly NodeType[]
  match: SearchMatchMode
}

interface SearchCursorPayload {
  v: 1
  key: string
  path: number[]
  id: string
  after?: boolean
}

interface WalkItem {
  node: unknown
  path: number[]
  ancestors: ReadonlySet<string>
}

declare const figma: FigmaReadApi

function observation(startedAt: string) {
  return { startedAt, completedAt: new Date().toISOString() }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object"
}

function record(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {}
}

function string(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback
}

function boolean(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback
}

function finite(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

function own(value: Record<string, unknown>, key: string): boolean {
  return Object.hasOwn(value, key)
}

async function lookupNode(id: string): Promise<unknown> {
  if (figma.getNodeByIdAsync === undefined)
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  return figma.getNodeByIdAsync(id)
}

async function resolveSearchRoot(
  scope: SearchScope,
  signal?: CancellationSignal,
): Promise<unknown> {
  signal?.throwIfAborted()
  if ("pageId" in scope) {
    const node =
      figma.currentPage.id === scope.pageId
        ? figma.currentPage
        : await lookupNode(scope.pageId)
    if (!isRecord(node) || node.type !== "PAGE")
      throw new PluginReadError("PAGE_NOT_FOUND", false)
    if (figma.currentPage.id !== scope.pageId) await loadPageIfNeeded(node)
    return node
  }
  const node = await lookupNode(scope.nodeId)
  if (node === null || node === undefined)
    throw new PluginReadError("NODE_NOT_FOUND", false)
  return loadPageIfNeeded(node)
}

function matches(
  haystack: string,
  query: string,
  mode: SearchMatchMode,
): boolean {
  const left = haystack.toLowerCase()
  const right = query.toLowerCase()
  return mode === "exact" ? left === right : left.includes(right)
}

function textCharacters(raw: unknown): string | undefined {
  const node = record(raw)
  if (node.type !== "TEXT") return undefined
  try {
    return typeof node.characters === "string" ? node.characters : undefined
  } catch {
    return undefined
  }
}

export function compilePredicate(
  input: Pick<SearchNodesInput, "query" | "types" | "match">,
): SearchPredicate {
  const predicate: SearchPredicate = { match: input.match }
  if (input.query !== undefined) {
    const query = input.query.trim()
    if (query.length === 0)
      throw new TypeError("query must be non-empty after trimming")
    predicate.query = query
  }
  if (input.types !== undefined) {
    const types: string[] = []
    const seen = new Set<string>()
    for (const type of input.types) {
      const trimmed = type.trim()
      if (trimmed.length === 0)
        throw new TypeError("types must be non-empty after trimming")
      if (!seen.has(trimmed)) {
        seen.add(trimmed)
        types.push(trimmed)
      }
    }
    if (types.length > 0) predicate.types = types
  }
  if (predicate.query === undefined && predicate.types === undefined)
    throw new TypeError("search must include query or types")
  return predicate
}

export function matchReasons(
  raw: unknown,
  predicate: SearchPredicate,
): MatchReason[] {
  const node = record(raw)
  const reasons: MatchReason[] = []
  if (predicate.types !== undefined) {
    if (!predicate.types.includes(string(node.type))) return []
    reasons.push("nodeType")
  }
  if (predicate.query !== undefined) {
    if (matches(string(node.name), predicate.query, predicate.match))
      reasons.push("name")
    const characters = textCharacters(raw)
    if (
      characters !== undefined &&
      matches(characters, predicate.query, predicate.match)
    )
      reasons.push("text")
    if (!reasons.includes("name") && !reasons.includes("text")) return []
  }
  return reasons
}

function summarizeMatch(raw: unknown, includeChildren = true): NodeSummary {
  const node = record(raw)
  const parent = record(node.parent)
  const bounds = record(node.absoluteBoundingBox)
  const children =
    includeChildren && Array.isArray(node.children) ? node.children : []
  const childIds = children
    .map((child) => string(record(child).id))
    .filter((id) => id.length > 0)
  const summary: NodeSummary = {
    id: string(node.id),
    name: string(node.name),
    nodeType: string(node.type),
    visible: boolean(node.visible, true),
  }
  if (typeof parent.id === "string") summary.parentId = parent.id
  if (childIds.length > 0) summary.childIds = childIds
  if (own(bounds, "x") && own(bounds, "y")) {
    summary.bounds = {
      x: finite(bounds.x),
      y: finite(bounds.y),
      width: finite(bounds.width),
      height: finite(bounds.height),
    }
  }
  return summary
}

function searchKey(
  input: SearchNodesInput,
  predicate: SearchPredicate,
): string {
  const scope =
    "pageId" in input.scope
      ? ["page", input.scope.pageId]
      : ["node", input.scope.nodeId]
  return JSON.stringify({
    scope,
    query: predicate.query ?? null,
    types: predicate.types === undefined ? [] : [...predicate.types].sort(),
    match: predicate.match,
  })
}

const BASE64URL =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"

function utf8Bytes(value: string): number[] {
  const encoded = encodeURIComponent(value)
  const bytes: number[] = []
  for (let index = 0; index < encoded.length; index += 1) {
    if (encoded[index] === "%") {
      bytes.push(Number.parseInt(encoded.slice(index + 1, index + 3), 16))
      index += 2
    } else {
      bytes.push(encoded.charCodeAt(index))
    }
  }
  return bytes
}

function encodeBase64Url(value: string): string {
  const bytes = utf8Bytes(value)
  let encoded = ""
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0
    const second = bytes[index + 1]
    const third = bytes[index + 2]
    encoded += BASE64URL[first >> 2]
    encoded += BASE64URL[((first & 3) << 4) | ((second ?? 0) >> 4)]
    if (second !== undefined)
      encoded += BASE64URL[((second & 15) << 2) | ((third ?? 0) >> 6)]
    if (third !== undefined) encoded += BASE64URL[third & 63]
  }
  return encoded
}

function decodeBase64Url(value: string): string {
  if (!/^[A-Za-z0-9_-]+$/u.test(value) || value.length % 4 === 1)
    throw new Error("invalid base64url")
  const bytes: number[] = []
  for (let index = 0; index < value.length; index += 4) {
    const a = BASE64URL.indexOf(value[index] ?? "")
    const b = BASE64URL.indexOf(value[index + 1] ?? "")
    const c =
      value[index + 2] === undefined
        ? -1
        : BASE64URL.indexOf(value[index + 2] ?? "")
    const d =
      value[index + 3] === undefined
        ? -1
        : BASE64URL.indexOf(value[index + 3] ?? "")
    if (a < 0 || b < 0) throw new Error("invalid base64url")
    bytes.push((a << 2) | (b >> 4))
    if (c >= 0) bytes.push(((b & 15) << 4) | (c >> 2))
    if (d >= 0) bytes.push(((c & 3) << 6) | d)
  }
  const percentEncoded = bytes
    .map((byte) => `%${byte.toString(16).padStart(2, "0")}`)
    .join("")
  return decodeURIComponent(percentEncoded)
}

function cursorFor(key: string, item: WalkItem, after = false): string {
  return encodeBase64Url(
    JSON.stringify({
      v: 1,
      key,
      path: item.path,
      id: string(record(item.node).id),
      after,
    }),
  )
}

function parseCursor(value: string, key: string): SearchCursorPayload {
  try {
    const parsed: unknown = JSON.parse(decodeBase64Url(value))
    if (!isRecord(parsed) || parsed.v !== 1 || parsed.key !== key)
      throw new Error()
    if (typeof parsed.id !== "string" || !Array.isArray(parsed.path))
      throw new Error()
    const path = parsed.path
    if (!path.every((index) => Number.isSafeInteger(index) && index >= 0))
      throw new Error()
    return {
      v: 1,
      key,
      path: path as number[],
      id: parsed.id,
      after: parsed.after === true,
    }
  } catch {
    throw new PluginReadError("INVALID_CURSOR", false)
  }
}

function childrenOf(node: unknown): readonly unknown[] {
  const children = record(node).children
  return Array.isArray(children) ? children : []
}

function initialStack(
  root: unknown,
  cursor: SearchCursorPayload | undefined,
): WalkItem[] {
  if (cursor === undefined)
    return [{ node: root, path: [], ancestors: new Set() }]
  const stack: WalkItem[] = []
  let node = root
  let path: number[] = []
  const ancestors = new Set<string>()
  for (const targetIndex of cursor.path) {
    const id = string(record(node).id)
    if (id.length > 0) ancestors.add(id)
    const children = childrenOf(node)
    if (targetIndex >= children.length)
      throw new PluginReadError("INVALID_CURSOR", false)
    for (let index = children.length - 1; index > targetIndex; index -= 1) {
      stack.push({
        node: children[index],
        path: [...path, index],
        ancestors: new Set(ancestors),
      })
    }
    node = children[targetIndex]
    path = [...path, targetIndex]
  }
  const item: WalkItem = { node, path, ancestors }
  if (string(record(item.node).id) !== cursor.id)
    throw new PluginReadError("INVALID_CURSOR", false)
  stack.push(item)
  if (cursor.after) {
    const current = stack.pop()
    if (current !== undefined) pushChildren(stack, current)
  }
  return stack
}

function pushChildren(stack: WalkItem[], item: WalkItem): void {
  const id = string(record(item.node).id)
  if (id.length > 0 && item.ancestors.has(id)) return
  const ancestors = new Set(item.ancestors)
  if (id.length > 0) ancestors.add(id)
  const children = childrenOf(item.node)
  for (let index = children.length - 1; index >= 0; index -= 1) {
    stack.push({
      node: children[index],
      path: [...item.path, index],
      ancestors,
    })
  }
}

export async function searchNodes(
  input: SearchNodesInput,
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<SearchNodesResult> {
  const startedAt = new Date().toISOString()
  const predicate = compilePredicate(input)
  const key = searchKey(input, predicate)
  const root = await resolveSearchRoot(input.scope, signal)
  const cursor =
    input.cursor === undefined ? undefined : parseCursor(input.cursor, key)
  const stack = initialStack(root, cursor)
  const matches: NodeMatch[] = []
  const visitedLimit = limits?.visitedNodes ?? MAX_VISITED_NODES
  const byteLimit = limits?.encodedBytes ?? MAX_TEXT_BYTES
  const returnedLimit = Math.min(
    input.limit,
    limits?.returnedNodes ?? MAX_RETURNED_NODES,
  )
  let visitedNodes = 0
  let encodedBytes = 0
  let truncation: Truncation | undefined
  let nextCursor: string | undefined

  while (stack.length > 0) {
    signal?.throwIfAborted()
    if (visitedNodes >= visitedLimit) {
      truncation = { reason: "nodeLimit", visitedNodes }
      break
    }
    const item = stack.pop()
    if (item === undefined) break
    visitedNodes += 1
    const reasons = matchReasons(item.node, predicate)
    if (reasons.length === 0) {
      pushChildren(stack, item)
      continue
    }
    const match = {
      node: summarizeMatch(item.node, matches.length + 1 < returnedLimit),
      reasons,
    }
    const nextBytes = encodedBytes + byteLength(match)
    if (nextBytes > byteLimit) {
      truncation = { reason: "byteLimit", encodedBytes: nextBytes }
      break
    }
    encodedBytes = nextBytes
    matches.push(match)
    if (matches.length >= returnedLimit) {
      if (matches.length >= (limits?.returnedNodes ?? MAX_RETURNED_NODES))
        truncation = { reason: "nodeLimit", visitedNodes }
      else nextCursor = cursorFor(key, item, true)
      break
    }
    pushChildren(stack, item)
  }

  const result: SearchNodesResult = {
    matches,
    truncated: truncation !== undefined,
    observation: observation(startedAt),
  }
  if (nextCursor !== undefined) result.nextCursor = nextCursor
  if (truncation !== undefined) result.truncation = truncation
  return result
}
