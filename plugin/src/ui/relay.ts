import type {
  ControllerBoundMessage,
  ControllerMetadataRequest,
  ControllerOutboundMessage,
  ErrorCode,
} from "../shared/protocol"
import { parseControllerOutboundMessage } from "../shared/validation"
import { encodeValidatedRaster, type RasterFormat } from "./raster"
import { validateSvgSource } from "./svg"

type ControllerMessage =
  | ControllerBoundMessage
  | ControllerMetadataRequest
  | {
      type: "screenshotValidated"
      validationId: string
      asset: unknown
    }

const ERROR_MESSAGES: Record<
  "UNSAFE_SVG" | "LIMIT_EXCEEDED" | "INTERNAL_ERROR",
  string
> = {
  UNSAFE_SVG: "The SVG was rejected by the safety policy.",
  LIMIT_EXCEEDED: "The operation exceeded a safety limit.",
  INTERNAL_ERROR: "The operation failed.",
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null
}

function asBytes(value: unknown): Uint8Array | null {
  if (value instanceof Uint8Array) return value
  if (value instanceof ArrayBuffer) return new Uint8Array(value)
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength)
  }
  if (!Array.isArray(value)) return null
  const bytes = new Uint8Array(value.length)
  for (let index = 0; index < value.length; index += 1) {
    const entry = value[index]
    if (typeof entry !== "number") return null
    bytes[index] = entry
  }
  return bytes
}

function itemError(code: "UNSAFE_SVG" | "LIMIT_EXCEEDED" | "INTERNAL_ERROR"): {
  status: "error"
  error: { code: ErrorCode; message: string; retryable: false }
} {
  return {
    status: "error",
    error: { code, message: ERROR_MESSAGES[code], retryable: false },
  }
}

function finalizeScreenshotItem(item: Record<string, unknown>): unknown {
  const nodeId = typeof item.nodeId === "string" ? item.nodeId : ""
  if (item.format === "svg") {
    if (typeof item.source !== "string") return itemError("INTERNAL_ERROR")
    const result = validateSvgSource(item.source, new DOMParser())
    if (!result.ok) return itemError(result.code)
    return {
      status: "success",
      value: { format: "svg", nodeId, source: result.source },
    }
  }
  if (item.format !== "png" && item.format !== "jpeg") {
    return itemError("INTERNAL_ERROR")
  }
  const bytes = asBytes(item.bytes)
  if (bytes === null) return itemError("INTERNAL_ERROR")
  const encoded = encodeValidatedRaster(bytes, item.format as RasterFormat)
  if (!encoded.ok) return itemError(encoded.code)
  return {
    status: "success",
    value: {
      format: encoded.format,
      nodeId,
      dataBase64: encoded.dataBase64,
      width: encoded.width,
      height: encoded.height,
    },
  }
}

export function sendToController(message: ControllerMessage): void {
  parent.postMessage({ pluginMessage: message }, "*")
}

export function onControllerMessage(
  receive: (message: ControllerOutboundMessage) => void,
): () => void {
  const listener = (event: MessageEvent<unknown>): void => {
    if (typeof event.data !== "object" || event.data === null) return
    const envelope = event.data as Record<string, unknown>
    if (!Object.hasOwn(envelope, "pluginMessage")) return
    const raw = envelope.pluginMessage
    if (isRecord(raw) && raw.type === "validateScreenshot") {
      if (typeof raw.validationId !== "string" || !isRecord(raw.item)) return
      sendToController({
        type: "screenshotValidated",
        validationId: raw.validationId,
        asset: finalizeScreenshotItem(raw.item),
      })
      return
    }
    try {
      receive(parseControllerOutboundMessage(raw))
    } catch {
      // Invalid cross-context data is rejected at the boundary.
    }
  }
  window.addEventListener("message", listener)
  return () => window.removeEventListener("message", listener)
}
