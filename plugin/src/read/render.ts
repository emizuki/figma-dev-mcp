import type {
  ErrorCode,
  GetScreenshotInput,
  GetScreenshotResult,
  ScreenshotAsset,
  ScreenshotSelector,
} from "../shared/protocol"
import type { ItemResult, SvgRejection, ToolError } from "../shared/results"
import {
  type CancellationListener,
  type CancellationSignal,
  LocalCancellationError,
} from "../main/cancellation"
import { progressFor } from "../main/progress"
import { PluginReadError } from "./navigation"
import { loadPageIfNeeded, type FigmaReadApi } from "./common"

export const SCREENSHOT_VALIDATION_TIMEOUT_MS = 10_000

/** Figma nests far shallower than this; the cap only stops a malformed parent
 * chain from spinning. */
const MAX_ANCESTOR_WALK = 128

declare const figma: FigmaReadApi

export interface RasterEncodeSuccess {
  ok: true
  dataBase64: string
  width: number
  height: number
  decodedBytes: number
  base64Bytes: number
}

/** The SVG safety policy runs in the UI context, so the verdict it reached has
 * to ride back with the source or it is lost before the result is built. An
 * unsafe verdict is not a failure: `ok` is still `true` and the source is still
 * present. */
export interface SvgEncodeSuccess {
  ok: true
  source: string
  safe: boolean
  rejection?: SvgRejection
}

export type RasterEncodeResult =
  | RasterEncodeSuccess
  | { ok: false; code: ErrorCode }

export type SvgEncodeResult = SvgEncodeSuccess | { ok: false; code: ErrorCode }

export interface ScreenshotCodec {
  encodeRaster(
    format: "png" | "jpeg",
    bytes: Uint8Array,
  ): Promise<RasterEncodeResult>
  encodeSvg(source: string): Promise<SvgEncodeResult>
}

const MESSAGES: Record<ErrorCode, string> = {
  NO_FIGMA_CONNECTION: "No Figma connection is available.",
  AMBIGUOUS_CONNECTION: "More than one Figma connection matches the request.",
  CONNECTION_NOT_FOUND: "The requested Figma connection was not found.",
  CONNECTION_LOST: "The Figma connection was lost.",
  PROTOCOL_MISMATCH: "The plugin protocol version is not supported.",
  NODE_NOT_FOUND: "The requested node was not found.",
  PAGE_NOT_FOUND: "The requested page was not found.",
  UNSUPPORTED_NODE: "The requested node type is not supported.",
  EMPTY_NODE_BOUNDS: "The requested node renders nothing.",
  CAPABILITY_UNAVAILABLE: "The required Figma capability is unavailable.",
  UNSAFE_SVG: "The SVG was rejected by the safety policy.",
  INVALID_CURSOR: "The search cursor is invalid or stale.",
  LIMIT_EXCEEDED: "The operation exceeded a safety limit.",
  TIMEOUT: "The operation timed out.",
  CANCELLED: "The operation was cancelled.",
  INTERNAL_ERROR: "The operation failed.",
}

function observation(startedAt: string) {
  return { startedAt, completedAt: new Date().toISOString() }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object"
}

function itemError(code: ErrorCode): ItemResult<ScreenshotAsset> {
  const error: ToolError = {
    code,
    message: MESSAGES[code],
    retryable: false,
  }
  return { status: "error", error }
}

function capturedSelectionIds(): string[] {
  return (figma.currentPage.selection ?? []).map((node) => node.id)
}

function captureNodeIds(selector: ScreenshotSelector): string[] {
  if ("selection" in selector) return capturedSelectionIds()
  if ("nodeId" in selector) return [selector.nodeId]
  return selector.nodeIds
}

function exportSettings(input: GetScreenshotInput): Record<string, unknown> {
  if (input.format === "svg") {
    return {
      format: "SVG_STRING",
      svgOutlineText: input.svgOutlineText,
      svgIdAttribute: input.svgIdAttribute,
      svgSimplifyStroke: input.svgSimplifyStroke,
    }
  }
  const scale = input.scale ?? 1
  if (!Number.isFinite(scale) || scale < 0.25 || scale > 4) {
    throw new PluginReadError("LIMIT_EXCEEDED", false)
  }
  return {
    format: input.format === "jpeg" ? "JPG" : "PNG",
    constraint: { type: "SCALE", value: scale },
  }
}

// Bounds are node *content*, and under `documentAccess: dynamic-page` some node
// properties are write-only and throw on read. A hostile or unloaded getter
// costs us the measurement, not the export.
function hostGet(node: Record<string, unknown>, key: string): unknown {
  try {
    return node[key]
  } catch {
    return undefined
  }
}

/** A `visible` switch is inherited: the API counts a node as visible only when
 * `visible === true` for itself *and every one of its parents*. So this walks
 * the chain rather than reading one flag.
 *
 * An unfinished walk returns false. Failing to establish visibility is not the
 * same as establishing it, and every caller here treats false as "leave this
 * node alone". */
function isFullyVisible(node: unknown): boolean {
  let current: unknown = node
  for (let step = 0; step < MAX_ANCESTOR_WALK; step += 1) {
    // Past the root: nothing in the chain was switched off.
    if (!isRecord(current)) return true
    if (hostGet(current, "visible") === false) return false
    current = hostGet(current, "parent")
  }
  return false
}

/** The rule: the host reports `absoluteRenderBounds` as exactly `null` for a
 * node we can also see is switched on.
 *
 * `absoluteRenderBounds` is the property that answers the question we are
 * actually asking — "does this node put any ink on the page?" — because it is
 * measured after strokes, shadows and effects. Geometry cannot answer it. A
 * `LINE` is *always* exactly zero pixels high, by API contract, yet every
 * divider and underline in every file renders perfectly through its stroke; a
 * straight `VECTOR` or `CONNECTOR` is the same. Judging on width and height
 * would fire on precisely those nodes and on almost nothing else, since every
 * other type is clamped to at least 0.01.
 *
 * The visibility guard is the price of using it. The same API calls a node
 * invisible when an *ancestor* is switched off, and null-because-hidden says
 * nothing about whether the node has anything in it, so those fall through to
 * the exporter exactly as they do today. The guard can only make this rule fire
 * less often, never more.
 *
 * `undefined` is not `null`: a property the host does not carry at all — a
 * `PAGE`, which has no layout — or one whose getter throws under
 * `documentAccess: dynamic-page` leaves the export alone. An unknown answer is
 * not an empty one. */
function rendersNothing(node: unknown): boolean {
  if (!isRecord(node)) return false
  if (!isFullyVisible(node)) return false
  return hostGet(node, "absoluteRenderBounds") === null
}

function nodeExporter(
  node: unknown,
): ((settings: Record<string, unknown>) => Promise<unknown>) | undefined {
  if (!isRecord(node)) return undefined
  const exporter = node.exportAsync
  if (typeof exporter !== "function") return undefined
  return (settings) =>
    (
      exporter as (
        this: unknown,
        value: Record<string, unknown>,
      ) => Promise<unknown>
    ).call(node, settings)
}

interface PendingValidation {
  resolve: (value: ItemResult<ScreenshotAsset>) => void
  timeout: ReturnType<typeof setTimeout>
  signal?: CancellationSignal
  abort?: CancellationListener
}

const pendingValidations = new Map<string, PendingValidation>()
let validationSeq = 0

function pluginUi(): { postMessage(message: unknown): void } | undefined {
  const api = (
    globalThis as typeof globalThis & {
      figma?: { ui?: { postMessage(message: unknown): void } }
    }
  ).figma
  return api?.ui
}

function settlePendingValidation(
  validationId: string,
  result: ItemResult<ScreenshotAsset>,
): boolean {
  const pending = pendingValidations.get(validationId)
  if (pending === undefined) return false
  pendingValidations.delete(validationId)
  clearTimeout(pending.timeout)
  if (pending.signal !== undefined && pending.abort !== undefined) {
    pending.signal.removeEventListener("abort", pending.abort)
  }
  pending.resolve(result)
  return true
}

export function completeScreenshotValidation(input: unknown): boolean {
  if (!isRecord(input) || input.type !== "screenshotValidated") return false
  const id = input.validationId
  if (typeof id !== "string") return false
  const asset = input.asset
  if (
    !isRecord(asset) ||
    (asset.status !== "success" && asset.status !== "error")
  ) {
    return settlePendingValidation(id, itemError("INTERNAL_ERROR"))
  }
  return settlePendingValidation(id, asset as ItemResult<ScreenshotAsset>)
}

async function requestUiValidation(
  payload:
    | { format: "png" | "jpeg"; nodeId: string; bytes: Uint8Array }
    | { format: "svg"; nodeId: string; source: string },
  signal?: CancellationSignal,
  timeoutMs: number = SCREENSHOT_VALIDATION_TIMEOUT_MS,
): Promise<ItemResult<ScreenshotAsset>> {
  const ui = pluginUi()
  if (ui === undefined) return itemError("INTERNAL_ERROR")
  if (signal?.aborted) return itemError("CANCELLED")
  const validationId = `screenshot-${validationSeq}`
  validationSeq += 1
  return new Promise((resolve) => {
    const abort = (): void => {
      settlePendingValidation(validationId, itemError("CANCELLED"))
    }
    const pending: PendingValidation = {
      resolve,
      timeout: setTimeout(() => {
        settlePendingValidation(validationId, itemError("TIMEOUT"))
      }, timeoutMs),
    }
    if (signal !== undefined) {
      pending.signal = signal
      pending.abort = abort
      signal.addEventListener("abort", abort)
    }
    pendingValidations.set(validationId, pending)
    try {
      ui.postMessage({
        type: "validateScreenshot",
        validationId,
        item: payload,
      })
    } catch {
      settlePendingValidation(validationId, itemError("INTERNAL_ERROR"))
    }
  })
}

function createUiCodec(
  signal?: CancellationSignal,
  timeoutMs: number = SCREENSHOT_VALIDATION_TIMEOUT_MS,
): ScreenshotCodec {
  return {
    async encodeRaster(format, bytes) {
      const result = await requestUiValidation(
        {
          format,
          nodeId: "",
          bytes,
        },
        signal,
        timeoutMs,
      )
      if (result.status === "error")
        return { ok: false, code: result.error.code }
      if (result.value.format === "svg")
        return { ok: false, code: "INTERNAL_ERROR" }
      return {
        ok: true,
        dataBase64: result.value.dataBase64,
        width: result.value.width,
        height: result.value.height,
        decodedBytes: 0,
        base64Bytes: result.value.dataBase64.length,
      }
    },
    async encodeSvg(source) {
      const result = await requestUiValidation(
        {
          format: "svg",
          nodeId: "",
          source,
        },
        signal,
        timeoutMs,
      )
      if (result.status === "error")
        return { ok: false, code: result.error.code }
      if (result.value.format !== "svg")
        return { ok: false, code: "INTERNAL_ERROR" }
      const encoded: SvgEncodeSuccess = {
        ok: true,
        source: result.value.source,
        safe: result.value.safe,
      }
      if (result.value.rejection !== undefined) {
        encoded.rejection = result.value.rejection
      }
      return encoded
    },
  }
}

async function encodeAsset(
  input: GetScreenshotInput,
  nodeId: string,
  exported: unknown,
  codec: ScreenshotCodec,
): Promise<ItemResult<ScreenshotAsset>> {
  if (input.format === "svg") {
    if (typeof exported !== "string") return itemError("INTERNAL_ERROR")
    const encoded = await codec.encodeSvg(exported)
    if (!encoded.ok) return itemError(encoded.code)
    const value: Extract<ScreenshotAsset, { format: "svg" }> = {
      format: "svg",
      nodeId,
      source: encoded.source,
      safe: encoded.safe,
    }
    if (encoded.rejection !== undefined) value.rejection = encoded.rejection
    return { status: "success", value }
  }
  if (!(exported instanceof Uint8Array)) return itemError("INTERNAL_ERROR")
  const encoded = await codec.encodeRaster(input.format, exported)
  if (!encoded.ok) return itemError(encoded.code)
  return {
    status: "success",
    value: {
      format: input.format,
      nodeId,
      dataBase64: encoded.dataBase64,
      width: encoded.width,
      height: encoded.height,
    },
  }
}

export async function getScreenshot(
  input: GetScreenshotInput,
  signal?: CancellationSignal,
  codec?: ScreenshotCodec,
  validationTimeoutMs: number = SCREENSHOT_VALIDATION_TIMEOUT_MS,
): Promise<GetScreenshotResult> {
  const startedAt = new Date().toISOString()
  const ids = captureNodeIds(input.selector)
  const settings = exportSettings(input)
  const assets: ItemResult<ScreenshotAsset>[] = []
  const activeCodec = codec ?? createUiCodec(signal, validationTimeoutMs)
  const progress = progressFor(signal)
  progress?.tick("encoding", 0, ids.length)
  if (ids.length > 0 && figma.getNodeByIdAsync === undefined) {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  for (const id of ids) {
    signal?.throwIfAborted()
    const lookup = figma.getNodeByIdAsync
    if (lookup === undefined) {
      assets.push(itemError("CAPABILITY_UNAVAILABLE"))
      continue
    }
    let node: unknown
    try {
      node = await lookup(id)
    } catch (error: unknown) {
      if (
        error instanceof PluginReadError ||
        error instanceof LocalCancellationError
      ) {
        throw error
      }
      assets.push(itemError("NODE_NOT_FOUND"))
      continue
    }
    if (node === null || node === undefined) {
      assets.push(itemError("NODE_NOT_FOUND"))
      continue
    }
    node = await loadPageIfNeeded(node)
    const exporter = nodeExporter(node)
    if (exporter === undefined) {
      assets.push(itemError("UNSUPPORTED_NODE"))
      continue
    }
    // Asked before the host exporter runs, and for every format: a node that
    // puts no ink on the page is empty as a PNG too, and a 1×1 transparent
    // pixel would be the same silent lie as an empty SVG.
    if (rendersNothing(node)) {
      assets.push(itemError("EMPTY_NODE_BOUNDS"))
      continue
    }
    try {
      progress?.tick("encoding", assets.length, ids.length)
      const exported = await exporter(settings)
      const asset = await encodeAsset(input, id, exported, activeCodec)
      signal?.throwIfAborted()
      assets.push(asset)
      progress?.tick("encoding", assets.length, ids.length)
    } catch (error: unknown) {
      if (
        error instanceof PluginReadError ||
        error instanceof LocalCancellationError
      ) {
        throw error
      }
      assets.push(itemError("INTERNAL_ERROR"))
    }
  }
  return {
    assets,
    truncated: false,
    observation: observation(startedAt),
  }
}
