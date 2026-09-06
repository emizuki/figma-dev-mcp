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
  collectVariableNames,
  serializeNodeForest,
  type SerializeNodeForestOptions,
} from "./serialize"
import { rendersVisibly } from "./visibility"
import { CANONICAL_MESSAGES } from "../shared/result-validation"

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
    "signal" | "instanceIdentities" | "styleNames" | "variableNames"
  >,
  signal?: CancellationSignal,
) {
  await loadDocumentPages(roots, signal)
  if (options.detail === "minimal") {
    return serializeNodeForest(roots, serializeOptions(options, signal))
  }
  // Names only matter at `full`; resolving them at `compact` would spend
  // async lookups on a level that is not allowed to carry them.
  // The three pre-passes below run concurrently: the two name collectors
  // budget themselves in wall-clock time (2s each), while
  // getMainComponentAsync calls (up to 1500ms apiece) now run alongside
  // them instead of before them. Call counts are unchanged, but under host
  // contention on an instance-heavy page, fewer names may resolve than
  // under the old sequential ordering — expected, not a regression.
  const isFull = options.detail === "full"
  const styleLookup = figma.getStyleByIdAsync
  // getVariableByIdAsync lives on figma.variables, not on figma itself.
  const variablesApi = figma.variables
  const variableLookup = variablesApi?.getVariableByIdAsync
  const [instanceIdentities, styleNames, variableNames] = await Promise.all([
    collectInstanceIdentities(roots, signal, options.depth),
    isFull && styleLookup !== undefined
      ? collectStyleNames(
          roots,
          (id) => styleLookup.call(figma, id),
          signal,
          options.depth,
        )
      : Promise.resolve(undefined),
    isFull && variablesApi !== undefined && variableLookup !== undefined
      ? collectVariableNames(
          roots,
          (id) => variableLookup.call(variablesApi, id),
          signal,
          options.depth,
        )
      : Promise.resolve(undefined),
  ])
  return serializeNodeForest(
    roots,
    serializeOptions(
      {
        ...options,
        instanceIdentities,
        ...(styleNames === undefined ? {} : { styleNames }),
        ...(variableNames === undefined ? {} : { variableNames }),
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
      // GetSelectionResult carries no per-item error slot, so a switched-off
      // selected root is excluded rather than reported — same treatment as a
      // lookup miss, and it must not sink the rest of the selection either.
      if (node !== null && node !== undefined && rendersVisibly(node)) {
        roots.push(node)
      }
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

// Sourced from `CANONICAL_MESSAGES` rather than hand-copied here: that map is
// what `parseReadResult` checks every message against
// (`shared/result-validation.ts`), and Rust refuses a non-canonical message
// for a code at decode time. A literal here could drift from it silently —
// nothing short of a live round trip would catch the mismatch — so there is
// exactly one place in the plugin allowed to spell these strings.
function nodeError(
  code:
    | "NODE_NOT_FOUND"
    | "NODE_NOT_VISIBLE"
    | "CAPABILITY_UNAVAILABLE"
    | "INTERNAL_ERROR",
) {
  return { code, message: CANONICAL_MESSAGES[code], retryable: false }
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
      if (!rendersVisibly(node)) {
        items.push({ status: "error", error: nodeError("NODE_NOT_VISIBLE") })
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
      // GetDesignContextResult carries no per-item error slot (it's a bare
      // NodeForest, like GetSelectionResult), so a switched-off root is
      // excluded rather than reported — same treatment as a lookup miss.
      if (node !== null && node !== undefined && rendersVisibly(node)) {
        roots.push(node)
      }
    }
    return roots
  }
  if ("nodeId" in selector) {
    signal?.throwIfAborted()
    const node = await lookupNode(selector.nodeId)
    if (node === null || node === undefined) {
      throw new PluginReadError("NODE_NOT_FOUND", false)
    }
    // Excluded, not refused: NODE_NOT_FOUND would be wrong (the id resolved),
    // and there is no per-item slot to carry a NODE_NOT_VISIBLE error in.
    if (!rendersVisibly(node)) return []
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
      if (rendersVisibly(node)) roots.push(await loadPageIfNeeded(node))
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
