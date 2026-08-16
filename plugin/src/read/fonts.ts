import type { GetFontsInput } from "../shared/protocol"
import type {
  FontAvailability,
  FontName,
  FontUsage,
  GetFontsResult,
  Truncation,
} from "../shared/results"
import {
  CANCEL_CHECK_BATCH,
  MAX_INPUT_IDS,
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import {
  throwIfAbortedAtBatch,
  type CancellationSignal,
} from "../main/cancellation"
import { resolveDesignRoots } from "./navigation"
import type { FigmaReadApi } from "./common"
import {
  byteLength,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

export const FONT_SEGMENT_RANGE = 2048

declare const figma: FigmaReadApi

type UnknownRecord = Record<string, unknown>

interface CollectedFont {
  font: FontName
  nodeIds: string[]
  seen: Set<string>
}

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

function walkOptions(
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): ForestWalkOptions {
  if (signal !== undefined && limits !== undefined) return { signal, limits }
  if (signal !== undefined) return { signal }
  if (limits !== undefined) return { limits }
  return {}
}

function fontKey(font: FontName): string {
  return `${font.family}\0${font.style}`
}

function readFontName(raw: unknown): FontName | undefined {
  if (!isRecord(raw)) return undefined
  if (typeof raw.family !== "string" || typeof raw.style !== "string") {
    return undefined
  }
  if (raw.family.length === 0 && raw.style.length === 0) return undefined
  return { family: raw.family, style: raw.style }
}

function isMixed(value: unknown): boolean {
  const mixed = figma.mixed
  return mixed !== undefined && value === mixed
}

function addUsage(
  collected: Map<string, CollectedFont>,
  raw: unknown,
  nodeId: string,
): void {
  const font = readFontName(raw)
  if (font === undefined || nodeId.length === 0) return
  const key = fontKey(font)
  const existing = collected.get(key)
  if (existing === undefined) {
    collected.set(key, {
      font,
      nodeIds: [nodeId],
      seen: new Set([nodeId]),
    })
    return
  }
  if (existing.seen.has(nodeId) || existing.nodeIds.length >= MAX_INPUT_IDS) {
    return
  }
  existing.seen.add(nodeId)
  existing.nodeIds.push(nodeId)
}

function collectMixed(
  node: UnknownRecord,
  nodeId: string,
  collected: Map<string, CollectedFont>,
): void {
  const reader = node.getStyledTextSegments
  if (typeof reader !== "function") return
  const length = string(node.characters).length
  if (length === 0) return
  for (let start = 0; start < length; start += FONT_SEGMENT_RANGE) {
    const end = Math.min(start + FONT_SEGMENT_RANGE, length)
    let segments: unknown
    try {
      segments = reader.call(node, ["fontName"], start, end)
    } catch {
      continue
    }
    for (const segment of array(segments)) {
      addUsage(collected, record(segment).fontName, nodeId)
    }
  }
}

function collectNode(
  raw: unknown,
  collected: Map<string, CollectedFont>,
): void {
  const node = record(raw)
  if (node.type !== "TEXT") return
  const id = string(node.id)
  if (isMixed(node.fontName)) {
    collectMixed(node, id, collected)
    return
  }
  addUsage(collected, node.fontName, id)
}

async function observedCatalog(
  signal?: CancellationSignal,
): Promise<Set<string> | undefined> {
  const list = figma.listAvailableFontsAsync
  if (typeof list !== "function") return undefined
  signal?.throwIfAborted()
  try {
    const fonts = await list.call(figma)
    const catalog = new Set<string>()
    for (const item of array(fonts)) {
      const font = readFontName(record(item).fontName)
      if (font !== undefined) catalog.add(fontKey(font))
    }
    return catalog
  } catch {
    return undefined
  }
}

function availability(
  key: string,
  catalog: Set<string> | undefined,
): FontAvailability {
  if (catalog === undefined) return "unknown"
  return catalog.has(key) ? "available" : "unavailable"
}

class FontEmission {
  readonly fonts: FontUsage[] = []
  encoded = 0
  considered = 0
  walkTruncation?: Truncation
  emitTruncation?: Truncation

  constructor(readonly limits: SerializerLimits) {}

  get truncation(): Truncation | undefined {
    return this.walkTruncation ?? this.emitTruncation
  }

  mark(truncation: Truncation): void {
    if (this.walkTruncation === undefined) this.walkTruncation = truncation
  }

  push(usage: FontUsage): boolean {
    this.considered += 1
    if (this.emitTruncation !== undefined) return false
    if (this.fonts.length >= this.limits.returnedNodes) {
      this.emitTruncation = {
        reason: "nodeLimit",
        visitedNodes: this.considered,
      }
      return false
    }
    const encoded = this.encoded + byteLength(usage)
    if (encoded > this.limits.encodedBytes) {
      this.emitTruncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    this.fonts.push(usage)
    return true
  }
}

export async function getFonts(
  input: Partial<GetFontsInput> = {},
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<GetFontsResult> {
  const startedAt = new Date().toISOString()
  const emission = new FontEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })
  const roots = await resolveDesignRoots(input.selector, signal)
  const collected = new Map<string, CollectedFont>()
  let index = 0
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    index += 1
    collectNode(raw, collected)
  })
  if (walked.truncation !== undefined) emission.mark(walked.truncation)

  const catalog = await observedCatalog(signal)
  for (const [key, usage] of collected) {
    signal?.throwIfAborted()
    if (
      !emission.push({
        font: usage.font,
        availability: availability(key, catalog),
        nodeIds: usage.nodeIds,
      })
    ) {
      break
    }
  }

  const result: GetFontsResult = {
    fonts: emission.fonts,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
