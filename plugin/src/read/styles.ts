import type { GetStylesInput, StyleSource } from "../shared/protocol"
import type {
  GetStylesResult,
  StyleIdentity,
  StyleValue,
  Truncation,
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
import type { FigmaReadApi } from "./common"
import {
  byteLength,
  effects,
  paints,
  textStyle,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

export type { StyleSource }

declare const figma: FigmaReadApi

type UnknownRecord = Record<string, unknown>

const STYLE_ID_FIELDS = [
  "fillStyleId",
  "strokeStyleId",
  "textStyleId",
  "effectStyleId",
  "gridStyleId",
] as const

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

function finite(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

function array(value: unknown): readonly unknown[] {
  return Array.isArray(value) ? value : []
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

function gridPattern(value: unknown): string {
  switch (value) {
    case "ROWS":
      return "rows"
    case "COLUMNS":
      return "columns"
    case "GRID":
      return "grid"
    default:
      return typeof value === "string" ? value.toLowerCase() : ""
  }
}

function styleIdentity(style: UnknownRecord): StyleIdentity | undefined {
  const id = string(style.id)
  const name = string(style.name)
  if (id.length === 0) return undefined
  const identity: StyleIdentity = { id, name }
  if (typeof style.description === "string") {
    identity.description = style.description
  }
  if (typeof style.remote === "boolean") identity.remote = style.remote
  if (typeof style.key === "string") identity.key = style.key
  return identity
}

function serializeStyle(raw: unknown): StyleValue | undefined {
  const style = record(raw)
  const identity = styleIdentity(style)
  if (identity === undefined) return undefined
  switch (style.type) {
    case "PAINT":
      return { styleType: "paint", ...identity, paints: paints(style.paints) }
    case "TEXT":
      return {
        styleType: "text",
        ...identity,
        text: {
          characters: "",
          defaultStyle: textStyle(style),
          styledRanges: [],
        },
      }
    case "EFFECT":
      return {
        styleType: "effect",
        ...identity,
        effects: effects(style.effects),
      }
    case "GRID": {
      const grid = record(array(style.layoutGrids)[0])
      return {
        styleType: "grid",
        ...identity,
        pattern: gridPattern(grid.pattern),
        size: finite(grid.sectionSize),
      }
    }
    default:
      return undefined
  }
}

function isMixedStyleId(value: unknown): boolean {
  if (figma.mixed !== undefined && value === figma.mixed) return true
  return value === "mixed"
}

function pushStyleId(ids: string[], value: unknown): void {
  if (typeof value !== "string" || value.length === 0 || isMixedStyleId(value))
    return
  ids.push(value)
}

function segmentStyleIds(node: UnknownRecord): string[] {
  if (node.type !== "TEXT") return []
  const reader = node.getStyledTextSegments
  if (typeof reader !== "function") return []
  const ids: string[] = []
  try {
    for (const segment of array(
      reader.call(node, ["textStyleId", "fillStyleId"]),
    )) {
      const row = record(segment)
      pushStyleId(ids, row.textStyleId)
      pushStyleId(ids, row.fillStyleId)
    }
  } catch {
    return ids
  }
  return ids
}

function styleIds(raw: unknown): string[] {
  const node = record(raw)
  const ids: string[] = []
  for (const field of STYLE_ID_FIELDS) {
    pushStyleId(ids, node[field])
  }
  for (const id of segmentStyleIds(node)) pushStyleId(ids, id)
  return ids
}

class StyleEmission {
  readonly styles: StyleValue[] = []
  readonly seen = new Set<string>()
  visited = 0
  encoded = 0
  truncation?: Truncation

  constructor(readonly limits: SerializerLimits) {}

  consider(): boolean {
    this.visited += 1
    if (this.truncation !== undefined) return false
    if (this.styles.length >= this.limits.returnedNodes) {
      this.truncation = {
        reason: "nodeLimit",
        visitedNodes: this.visited,
      }
      return false
    }
    return true
  }

  push(style: StyleValue): boolean {
    if (this.seen.has(style.id)) return true
    const encoded = this.encoded + byteLength(style)
    if (encoded > this.limits.encodedBytes) {
      this.truncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    this.seen.add(style.id)
    this.styles.push(style)
    return true
  }

  mark(truncation: Truncation): void {
    if (this.truncation === undefined) this.truncation = truncation
  }
}

function requireLocalReaders(): {
  paint: () => Promise<unknown[]>
  text: () => Promise<unknown[]>
  effect: () => Promise<unknown[]>
  grid: () => Promise<unknown[]>
} {
  const paint = figma.getLocalPaintStylesAsync
  const text = figma.getLocalTextStylesAsync
  const effect = figma.getLocalEffectStylesAsync
  const grid = figma.getLocalGridStylesAsync
  if (
    paint === undefined ||
    text === undefined ||
    effect === undefined ||
    grid === undefined
  ) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  return { paint, text, effect, grid }
}

async function emitLocal(
  emission: StyleEmission,
  signal?: CancellationSignal,
): Promise<void> {
  const readers = requireLocalReaders()
  const groups = [
    await readers.paint.call(figma),
    await readers.text.call(figma),
    await readers.effect.call(figma),
    await readers.grid.call(figma),
  ]
  let index = 0
  for (const group of groups) {
    for (const raw of group) {
      throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
      signal?.throwIfAborted()
      index += 1
      if (!emission.consider()) return
      const style = serializeStyle(raw)
      if (style !== undefined && !emission.push(style)) return
    }
  }
}

async function emitReferenced(
  emission: StyleEmission,
  input: Partial<GetStylesInput>,
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<void> {
  const lookup = figma.getStyleByIdAsync
  if (lookup === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  const roots = await resolveDesignRoots(input.selector, signal)
  const pending: string[] = []
  const pendingSeen = new Set(emission.seen)
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    for (const id of styleIds(raw)) {
      if (pendingSeen.has(id)) continue
      pendingSeen.add(id)
      pending.push(id)
    }
  })
  if (walked.truncation !== undefined) emission.mark(walked.truncation)
  for (let index = 0; index < pending.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    if (!emission.consider()) return
    const raw = await lookup.call(figma, pending[index] as string)
    if (raw === null || raw === undefined) continue
    const style = serializeStyle(raw)
    if (style !== undefined && !emission.push(style)) return
  }
}

export async function getStyles(
  input: Partial<GetStylesInput> = {},
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<GetStylesResult> {
  const startedAt = new Date().toISOString()
  const source: StyleSource = input.source ?? "both"
  const emission = new StyleEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })
  if (source === "local" || source === "both") {
    await emitLocal(emission, signal)
  }
  if (
    (source === "referenced" || source === "both") &&
    emission.truncation === undefined
  ) {
    await emitReferenced(emission, input, signal, limits)
  }
  const result: GetStylesResult = {
    styles: emission.styles,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
