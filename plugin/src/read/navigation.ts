import type {
  ErrorCode,
  GetDesignContextInput,
  GetDesignContextResult,
  GetMetadataResult,
  GetNodesInput,
  GetNodesResult,
  GetSelectionInput,
  GetSelectionResult,
  Selector,
} from "../shared/protocol"
import { assertNever } from "../shared/protocol"
import { MAX_DEPTH, MAX_RETURNED_NODES } from "../shared/limits"
import {
  LocalCancellationError,
  type CancellationSignal,
} from "../main/cancellation"
import {
  detectCapabilities,
  loadPageIfNeeded,
  PLUGIN_VERSION,
  type FigmaReadApi,
} from "./common"
import {
  collectInstanceIdentities,
  collectStyleNames,
  serializeNodeForest,
  type SerializeNodeForestOptions,
} from "./serialize"

function serializeOptions(
  options: Omit<SerializeNodeForestOptions, "signal">,
  signal?: CancellationSignal,
): SerializeNodeForestOptions {
  return signal === undefined ? options : { ...options, signal }
}

declare const figma: FigmaReadApi

export class PluginReadError extends Error {
  constructor(
    readonly code: ErrorCode,
    readonly retryable: boolean,
  ) {
    super("Plugin read failed")
    this.name = "PluginReadError"
  }
}

function observation(startedAt: string) {
  return { startedAt, completedAt: new Date().toISOString() }
}

function defaultDetail<T extends { detail?: "minimal" | "compact" | "full" }>(
  input: T,
): "minimal" | "compact" | "full" {
  return input.detail ?? "compact"
}

function defaultDepth(depth: number | undefined): number {
  return Math.min(depth ?? 2, MAX_DEPTH)
}

export function readMetadata(): GetMetadataResult {
  const startedAt = new Date().toISOString()
  const pageCount = figma.root.children.length
  const truncated = pageCount > MAX_RETURNED_NODES
  const result: GetMetadataResult = {
    file: { name: figma.root.name, editorType: figma.editorType },
    pages: figma.root.children.slice(0, MAX_RETURNED_NODES).map((page) => ({
      id: page.id,
      name: page.name,
    })),
    currentPageId: figma.currentPage.id,
    pluginVersion: PLUGIN_VERSION,
    capabilities: detectCapabilities(),
    truncated,
    observation: { startedAt, completedAt: new Date().toISOString() },
  }
  if (truncated)
    result.truncation = { reason: "nodeLimit", visitedNodes: pageCount }
  return result
}

function capturedSelectionIds(): string[] {
  return (figma.currentPage.selection ?? []).map((node) => node.id)
}

async function lookupNode(id: string): Promise<unknown> {
  if (figma.getNodeByIdAsync === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  try {
    return await figma.getNodeByIdAsync(id)
  } catch {
    return null
  }
}

async function loadDocumentPages(
  roots: readonly unknown[],
  signal?: CancellationSignal,
): Promise<void> {
  for (const root of roots) {
    signal?.throwIfAborted()
    if (!isRecord(root) || root.type !== "DOCUMENT") continue
    const children = Array.isArray(root.children) ? root.children : []
    for (const child of children) {
      signal?.throwIfAborted()
      await loadPageIfNeeded(child)
    }
  }
}

async function serializePreparedForest(
  roots: readonly unknown[],
  options: Omit<
    SerializeNodeForestOptions,
    "signal" | "instanceIdentities" | "styleNames"
  >,
  signal?: CancellationSignal,
) {
  await loadDocumentPages(roots, signal)
  if (options.detail === "minimal") {
    return serializeNodeForest(roots, serializeOptions(options, signal))
  }
  const instanceIdentities = await collectInstanceIdentities(
    roots,
    signal,
    options.depth,
  )
  // Style names only matter at `full`; resolving them at `compact` would spend
  // async lookups on a level that is not allowed to carry them.
  const lookup = figma.getStyleByIdAsync
  const styleNames =
    options.detail === "full" && lookup !== undefined
      ? await collectStyleNames(
          roots,
          (id) => lookup.call(figma, id),
          signal,
          options.depth,
        )
      : undefined
  return serializeNodeForest(
    roots,
    serializeOptions(
      {
        ...options,
        instanceIdentities,
        ...(styleNames === undefined ? {} : { styleNames }),
      },
      signal,
    ),
  )
}

export async function readSelection(
  input: GetSelectionInput = {},
  signal?: CancellationSignal,
): Promise<GetSelectionResult> {
  const startedAt = new Date().toISOString()
  // Snapshot IDs synchronously: selection can change while node lookups await.
  const ids = capturedSelectionIds()
  const detail = defaultDetail(input)
  const depth = defaultDepth(input.depth)
  const roots: unknown[] = []
  if (ids.length > 0 && figma.getNodeByIdAsync === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  if (figma.getNodeByIdAsync !== undefined) {
    for (const id of ids) {
      signal?.throwIfAborted()
      const node = await lookupNode(id)
      if (node !== null && node !== undefined) roots.push(node)
    }
  }
  const serialized = await serializePreparedForest(
    roots,
    { detail, depth, dedupeComponents: false },
    signal,
  )
  const result = {
    detail,
    nodes: serialized.nodes,
    truncated: serialized.truncated,
    ...(serialized.truncation === undefined
      ? {}
      : { truncation: serialized.truncation }),
    observation: observation(startedAt),
  }
  return result as GetSelectionResult
}

function nodeError(
  code: "NODE_NOT_FOUND" | "CAPABILITY_UNAVAILABLE" | "INTERNAL_ERROR",
) {
  switch (code) {
    case "NODE_NOT_FOUND":
      return {
        code,
        message: "The requested node was not found.",
        retryable: false,
      }
    case "CAPABILITY_UNAVAILABLE":
      return {
        code,
        message: "The required Figma capability is unavailable.",
        retryable: false,
      }
    case "INTERNAL_ERROR":
      return { code, message: "The operation failed.", retryable: false }
  }
}

export async function readNodes(
  input: GetNodesInput,
  signal?: CancellationSignal,
): Promise<GetNodesResult> {
  const startedAt = new Date().toISOString()
  const detail = defaultDetail(input)
  const depth = defaultDepth(input.depth)
  const lookup = figma.getNodeByIdAsync
  const items: unknown[] = []
  let truncated = false
  let truncation: unknown

  for (const id of input.nodeIds) {
    signal?.throwIfAborted()
    if (lookup === undefined) {
      items.push({
        status: "error",
        error: nodeError("CAPABILITY_UNAVAILABLE"),
      })
      continue
    }
    try {
      const node = await lookupNode(id)
      if (node === null || node === undefined) {
        items.push({ status: "error", error: nodeError("NODE_NOT_FOUND") })
        continue
      }
      const serialized = await serializePreparedForest(
        [await loadPageIfNeeded(node)],
        { detail, depth, dedupeComponents: false },
        signal,
      )
      const value = serialized.nodes[0]
      if (value === undefined) {
        items.push({ status: "error", error: nodeError("INTERNAL_ERROR") })
        continue
      }
      items.push({ status: "success", value })
      if (serialized.truncated && !truncated) {
        truncated = true
        truncation = serialized.truncation
      }
    } catch (error: unknown) {
      if (
        error instanceof PluginReadError ||
        error instanceof LocalCancellationError
      ) {
        throw error
      }
      items.push({ status: "error", error: nodeError("INTERNAL_ERROR") })
    }
  }

  const result = {
    detail,
    items,
    truncated,
    ...(truncation === undefined ? {} : { truncation }),
    observation: observation(startedAt),
  }
  return result as GetNodesResult
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object"
}

async function loadExplicitPage(
  id: string,
  signal?: CancellationSignal,
): Promise<unknown> {
  signal?.throwIfAborted()
  const node = await lookupNode(id)
  if (
    node === null ||
    node === undefined ||
    !isRecord(node) ||
    node.type !== "PAGE"
  ) {
    throw new PluginReadError("PAGE_NOT_FOUND", false)
  }
  return loadPageIfNeeded(node)
}

export async function resolveDesignRoots(
  selector: Selector | undefined,
  signal?: CancellationSignal,
): Promise<unknown[]> {
  if (selector === undefined) return [figma.currentPage]
  if ("selection" in selector) {
    const ids = capturedSelectionIds()
    const roots: unknown[] = []
    for (const id of ids) {
      signal?.throwIfAborted()
      const node = await lookupNode(id)
      if (node !== null && node !== undefined) roots.push(node)
    }
    return roots
  }
  if ("nodeId" in selector) {
    signal?.throwIfAborted()
    const node = await lookupNode(selector.nodeId)
    if (node === null || node === undefined) {
      throw new PluginReadError("NODE_NOT_FOUND", false)
    }
    return [await loadPageIfNeeded(node)]
  }
  if ("nodeIds" in selector) {
    const roots: unknown[] = []
    for (const id of selector.nodeIds) {
      signal?.throwIfAborted()
      const node = await lookupNode(id)
      if (node === null || node === undefined) {
        throw new PluginReadError("NODE_NOT_FOUND", false)
      }
      roots.push(await loadPageIfNeeded(node))
    }
    return roots
  }
  if ("pageId" in selector)
    return [await loadExplicitPage(selector.pageId, signal)]
  if ("pageIds" in selector) {
    const roots: unknown[] = []
    for (const id of selector.pageIds) {
      roots.push(await loadExplicitPage(id, signal))
    }
    return roots
  }
  return assertNever(selector)
}

export async function readDesignContext(
  input: GetDesignContextInput,
  signal?: CancellationSignal,
): Promise<GetDesignContextResult> {
  const startedAt = new Date().toISOString()
  const detail = defaultDetail(input)
  const depth = defaultDepth(input.depth)
  const roots = await resolveDesignRoots(input.selector, signal)
  const serialized = await serializePreparedForest(
    roots,
    {
      detail,
      depth,
      dedupeComponents: input.dedupeComponents,
      includeHidden: input.includeHidden,
    },
    signal,
  )
  const result = {
    detail,
    roots: serialized.nodes,
    truncated: serialized.truncated,
    ...(serialized.truncation === undefined
      ? {}
      : { truncation: serialized.truncation }),
    observation: observation(startedAt),
  }
  return result as GetDesignContextResult
}
