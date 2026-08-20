import type { GetDevModeDataInput } from "../shared/protocol"
import type {
  AnnotationCategory,
  AnnotationValue,
  DevModeNodeData,
  DevResource,
  GetDevModeDataResult,
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
import { resolveDesignRoots } from "./navigation"
import { hasHostField, type FigmaReadApi } from "./common"
import {
  byteLength,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

declare const figma: FigmaReadApi

type UnknownRecord = Record<string, unknown>

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

class ItemEmission {
  readonly items: GetDevModeDataResult["items"] = []
  encoded = 0
  walkTruncation?: Truncation
  emitTruncation?: Truncation

  constructor(readonly limits: SerializerLimits) {}

  get truncation(): Truncation | undefined {
    return this.walkTruncation ?? this.emitTruncation
  }

  mark(truncation: Truncation): void {
    if (this.walkTruncation === undefined) this.walkTruncation = truncation
  }

  push(value: DevModeNodeData, visitedNodes: number): boolean {
    if (this.emitTruncation !== undefined) return false
    if (this.items.length >= this.limits.returnedNodes) {
      this.emitTruncation = { reason: "nodeLimit", visitedNodes }
      return false
    }
    const encoded = this.encoded + byteLength(value)
    if (encoded > this.limits.encodedBytes) {
      this.emitTruncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    this.items.push({ status: "success", value })
    return true
  }
}

async function loadCategories(): Promise<AnnotationCategory[]> {
  const reader = figma.annotations?.getAnnotationCategoriesAsync
  if (typeof reader !== "function") return []
  try {
    const categories: AnnotationCategory[] = []
    for (const raw of await reader.call(figma.annotations)) {
      const item = record(raw)
      const id = string(item.id)
      const label = string(item.label)
      if (id.length === 0) continue
      categories.push({ id, label })
    }
    return categories
  } catch {
    return []
  }
}

function annotations(
  nodeId: string,
  raw: unknown,
): { annotations: AnnotationValue[]; categoryIds: string[] } {
  const values: AnnotationValue[] = []
  const categoryIds: string[] = []
  const seen = new Set<string>()
  array(raw).forEach((item, index) => {
    const annotation = record(item)
    const text =
      typeof annotation.label === "string"
        ? annotation.label
        : typeof annotation.labelMarkdown === "string"
          ? annotation.labelMarkdown
          : ""
    const id =
      typeof annotation.id === "string" && annotation.id.length > 0
        ? annotation.id
        : `${nodeId}:annotation:${index}`
    const value: AnnotationValue = { id, text }
    if (typeof annotation.categoryId === "string") {
      value.categoryId = annotation.categoryId
      if (!seen.has(annotation.categoryId)) {
        seen.add(annotation.categoryId)
        categoryIds.push(annotation.categoryId)
      }
    }
    values.push(value)
  })
  return { annotations: values, categoryIds }
}

function documentation(raw: unknown): DevResource[] {
  const links: DevResource[] = []
  for (const item of array(raw)) {
    const link = record(item)
    const uri = string(link.uri)
    if (uri.length === 0) continue
    links.push({
      name: typeof link.label === "string" ? link.label : "",
      uri,
    })
  }
  return links
}

async function devResources(raw: unknown): Promise<{
  resources: DevResource[]
  inheritedFromNodeId?: string
}> {
  if (typeof raw !== "function") return { resources: [] }
  try {
    const resources: DevResource[] = []
    let inheritedFromNodeId: string | undefined
    for (const item of array(await raw())) {
      const resource = record(item)
      const uri = string(resource.url)
      if (uri.length === 0) continue
      resources.push({
        name: string(resource.name),
        uri,
      })
      if (
        inheritedFromNodeId === undefined &&
        typeof resource.inheritedNodeId === "string" &&
        resource.inheritedNodeId.length > 0
      ) {
        inheritedFromNodeId = resource.inheritedNodeId
      }
    }
    return inheritedFromNodeId === undefined
      ? { resources }
      : { resources, inheritedFromNodeId }
  } catch {
    return { resources: [] }
  }
}

function referencedCategories(
  catalog: readonly AnnotationCategory[],
  categoryIds: readonly string[],
): AnnotationCategory[] {
  if (categoryIds.length === 0) return []
  const wanted = new Set(categoryIds)
  return catalog.filter((category) => wanted.has(category.id))
}

async function serializeDevMode(
  raw: unknown,
  catalog: readonly AnnotationCategory[],
): Promise<DevModeNodeData | undefined> {
  const node = record(raw)
  const nodeId = string(node.id)
  if (nodeId.length === 0) return undefined
  const annotated = hasHostField(node, "annotations")
    ? annotations(nodeId, node.annotations)
    : { annotations: [], categoryIds: [] }
  const docs = hasHostField(node, "documentationLinks")
    ? documentation(node.documentationLinks)
    : []
  const linked = await devResources(node.getDevResourcesAsync)
  const value: DevModeNodeData = {
    nodeId,
    annotations: annotated.annotations,
    annotationCategories: referencedCategories(catalog, annotated.categoryIds),
    documentation: docs,
    devResources: linked.resources,
  }
  if (typeof node.description === "string") value.description = node.description
  if (typeof node.descriptionMarkdown === "string") {
    value.descriptionMarkdown = node.descriptionMarkdown
  }
  if (typeof node.ownerNodeId === "string" && node.ownerNodeId.length > 0) {
    value.ownerNodeId = node.ownerNodeId
  }
  if (
    typeof node.inheritedFromNodeId === "string" &&
    node.inheritedFromNodeId.length > 0
  ) {
    value.inheritedFromNodeId = node.inheritedFromNodeId
  } else if (linked.inheritedFromNodeId !== undefined) {
    value.inheritedFromNodeId = linked.inheritedFromNodeId
  }
  return value
}

// Emit only nodes that have something to say. A record per visited node cost
// 175,226 bytes across 563 items on one measured page to carry content on
// exactly one of them.
//
// Empty here means "carries nothing but its own id". The four capability-backed
// lists are the bulk of the payload, but a node can also carry a description or
// an ownership pointer with all four lists empty, and dropping that would be a
// second data loss in the name of fixing the first.
//
// The description fields are tested for length, not for presence. The Figma
// Plugin API gives every ComponentNode, ComponentSetNode and style node a
// default empty-string `description`, so a presence test would re-emit an empty
// record for every component in the file. `ownerNodeId` and
// `inheritedFromNodeId` need no such guard: the serializer above only assigns
// them when they are non-empty.
function hasContent(value: DevModeNodeData): boolean {
  return (
    value.annotations.length > 0 ||
    value.annotationCategories.length > 0 ||
    value.devResources.length > 0 ||
    value.documentation.length > 0 ||
    (value.description ?? "").length > 0 ||
    (value.descriptionMarkdown ?? "").length > 0 ||
    value.ownerNodeId !== undefined ||
    value.inheritedFromNodeId !== undefined
  )
}

export async function getDevModeData(
  input: Partial<GetDevModeDataInput> = {},
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<GetDevModeDataResult> {
  const startedAt = new Date().toISOString()
  const emission = new ItemEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })
  const catalog = await loadCategories()
  const roots = await resolveDesignRoots(input.selector, signal)
  const pending: unknown[] = []
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    pending.push(raw)
  })
  if (walked.truncation !== undefined) emission.mark(walked.truncation)
  // Counts nodes inspected, not records emitted: a caller reading `items` needs
  // to know how much of the tree was examined and found to have nothing.
  let visitedNodes = 0
  for (let index = 0; index < pending.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    visitedNodes += 1
    const value = await serializeDevMode(pending[index], catalog)
    if (value === undefined || !hasContent(value)) continue
    if (!emission.push(value, visitedNodes)) break
  }
  const result: GetDevModeDataResult = {
    items: emission.items,
    visitedNodes,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
