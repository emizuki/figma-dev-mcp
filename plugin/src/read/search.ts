import type {
  SearchNodesInput,
  SearchQuery,
  SearchScope,
} from "../shared/protocol"
import type {
  NodeMatch,
  NodeSummary,
  SearchNodesResult,
} from "../shared/results"
import type { CancellationSignal } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { loadPageIfNeeded, type FigmaReadApi } from "./common"
import {
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

function walkOptions(
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): ForestWalkOptions {
  if (signal !== undefined && limits !== undefined) return { signal, limits }
  if (signal !== undefined) return { signal }
  if (limits !== undefined) return { limits }
  return {}
}

export type NodeType = string
export type MatchReason = "name" | "nodeType" | "text"

export interface SearchPredicate {
  name?: { value: string; mode: "exact" | "contains"; caseSensitive?: boolean }
  nodeTypes?: readonly NodeType[]
  text?: { value: string; mode: "exact" | "contains"; caseSensitive?: boolean }
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
  if (figma.getNodeByIdAsync === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  return figma.getNodeByIdAsync(id)
}

async function resolveSearchRoot(
  scope: SearchScope,
  signal?: CancellationSignal,
): Promise<unknown> {
  signal?.throwIfAborted()
  if ("pageId" in scope) {
    const node = await lookupNode(scope.pageId)
    if (
      node === null ||
      node === undefined ||
      !isRecord(node) ||
      node.type !== "PAGE"
    ) {
      throw new PluginReadError("PAGE_NOT_FOUND", false)
    }
    await loadPageIfNeeded(node)
    return node
  }
  const node = await lookupNode(scope.nodeId)
  if (node === null || node === undefined) {
    throw new PluginReadError("NODE_NOT_FOUND", false)
  }
  return loadPageIfNeeded(node)
}

function matchesTerm(
  haystack: string,
  term: { value: string; mode: "exact" | "contains"; caseSensitive?: boolean },
): boolean {
  const caseSensitive = term.caseSensitive === true
  const left = caseSensitive ? haystack : haystack.toLowerCase()
  const right = caseSensitive ? term.value : term.value.toLowerCase()
  return term.mode === "exact" ? left === right : left.includes(right)
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

function compileTerm(
  term: { value: string; mode: "exact" | "contains"; caseSensitive?: boolean },
  label: string,
): { value: string; mode: "exact" | "contains"; caseSensitive?: boolean } {
  const value = term.value.trim()
  if (value.length === 0) {
    throw new TypeError(`${label} must be non-empty after trimming`)
  }
  return term.caseSensitive === undefined
    ? { value, mode: term.mode }
    : { value, mode: term.mode, caseSensitive: term.caseSensitive }
}

export function compilePredicate(query: SearchQuery): SearchPredicate {
  const predicate: SearchPredicate = {}
  if (query.name !== undefined) {
    predicate.name = compileTerm(query.name, "name")
  }
  if (query.text !== undefined) {
    predicate.text = compileTerm(query.text, "text")
  }
  if (query.nodeTypes !== undefined) {
    const nodeTypes: string[] = []
    const seen = new Set<string>()
    for (const type of query.nodeTypes) {
      const trimmed = type.trim()
      if (trimmed.length === 0) {
        throw new TypeError("nodeTypes must be non-empty after trimming")
      }
      if (!seen.has(trimmed)) {
        seen.add(trimmed)
        nodeTypes.push(trimmed)
      }
    }
    if (nodeTypes.length > 0) predicate.nodeTypes = nodeTypes
  }
  if (
    predicate.name === undefined &&
    predicate.nodeTypes === undefined &&
    predicate.text === undefined
  ) {
    throw new TypeError("search query must include name, nodeTypes, or text")
  }
  return predicate
}

export function matchReasons(
  raw: unknown,
  predicate: SearchPredicate,
): MatchReason[] {
  const node = record(raw)
  const reasons: MatchReason[] = []
  if (predicate.name !== undefined) {
    if (!matchesTerm(string(node.name), predicate.name)) return []
    reasons.push("name")
  }
  if (predicate.nodeTypes !== undefined) {
    if (!predicate.nodeTypes.includes(string(node.type))) return []
    reasons.push("nodeType")
  }
  if (predicate.text !== undefined) {
    const characters = textCharacters(raw)
    if (characters === undefined || !matchesTerm(characters, predicate.text)) {
      return []
    }
    reasons.push("text")
  }
  return reasons
}

function summarizeMatch(raw: unknown): NodeSummary {
  const node = record(raw)
  const parent = record(node.parent)
  const bounds = record(node.absoluteBoundingBox)
  const children = Array.isArray(node.children) ? node.children : []
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

export async function searchNodes(
  input: SearchNodesInput,
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<SearchNodesResult> {
  const startedAt = new Date().toISOString()
  const predicate = compilePredicate(input.query)
  const root = await resolveSearchRoot(input.scope, signal)
  const matches: NodeMatch[] = []
  const walked = walkNodeForest(
    [root],
    walkOptions(signal, limits),
    (node, context) => {
      const reasons = matchReasons(node, predicate)
      if (reasons.length === 0) return
      const match = { node: summarizeMatch(node), reasons }
      if (context.tryReturn(match)) matches.push(match)
    },
  )
  const result: SearchNodesResult = {
    matches,
    truncated: walked.truncated,
    observation: observation(startedAt),
  }
  if (walked.truncation !== undefined) result.truncation = walked.truncation
  return result
}
