import {
  MAX_RASTER_BASE64_BYTES,
  MAX_RASTER_DECODED_BYTES,
  MAX_RASTER_PIXELS,
  MAX_RASTER_SIDE,
} from "../shared/limits"

export type RasterFormat = "png" | "jpeg"
export type EmbeddedImageMime =
  | "image/png"
  | "image/jpeg"
  | "image/jpg"
  | "image/webp"

export type RasterValidation =
  | {
      ok: true
      format: RasterFormat
      width: number
      height: number
      decodedBytes: number
      base64Bytes: number
      dataBase64: string
    }
  | { ok: false; code: "LIMIT_EXCEEDED" | "INTERNAL_ERROR" }

const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a] as const
const BASE64_TABLE =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"

function byte(bytes: Uint8Array, index: number): number {
  return bytes[index] ?? -1
}

function matches(
  bytes: Uint8Array,
  offset: number,
  expected: readonly number[],
): boolean {
  if (bytes.length < offset + expected.length) return false
  for (let index = 0; index < expected.length; index += 1) {
    if (byte(bytes, offset + index) !== expected[index]) return false
  }
  return true
}

export function encodeBase64(bytes: Uint8Array): string {
  const parts: string[] = []
  const length = bytes.length
  let index = 0
  while (index < length) {
    const remaining = length - index
    const b0 = byte(bytes, index)
    const b1 = remaining > 1 ? byte(bytes, index + 1) : 0
    const b2 = remaining > 2 ? byte(bytes, index + 2) : 0
    parts.push(BASE64_TABLE[(b0 >> 2) & 0x3f] ?? "")
    parts.push(BASE64_TABLE[((b0 & 0x03) << 4) | (b1 >> 4)] ?? "")
    parts.push(
      remaining > 1
        ? (BASE64_TABLE[((b1 & 0x0f) << 2) | (b2 >> 6)] ?? "")
        : "=",
    )
    parts.push(remaining > 2 ? (BASE64_TABLE[b2 & 0x3f] ?? "") : "=")
    index += 3
  }
  return parts.join("")
}

export function decodeBase64(value: string): Uint8Array | null {
  let length = 0
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code === 0x09 || code === 0x0a || code === 0x0d || code === 0x20) {
      continue
    }
    length += 1
  }
  if (length % 4 !== 0) return null
  const cleaned = new Uint8Array(length)
  let write = 0
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code === 0x09 || code === 0x0a || code === 0x0d || code === 0x20) {
      continue
    }
    cleaned[write] = code
    write += 1
  }
  const padding =
    (cleaned[length - 1] === 0x3d ? 1 : 0) +
    (cleaned[length - 2] === 0x3d ? 1 : 0)
  const output = new Uint8Array((length / 4) * 3 - padding)
  let outputIndex = 0
  const sextet = (code: number): number => {
    if (code >= 0x41 && code <= 0x5a) return code - 0x41
    if (code >= 0x61 && code <= 0x7a) return code - 0x61 + 26
    if (code >= 0x30 && code <= 0x39) return code - 0x30 + 52
    if (code === 0x2b) return 62
    if (code === 0x2f) return 63
    if (code === 0x3d) return 0
    return -1
  }
  for (let index = 0; index < length; index += 4) {
    const a = sextet(cleaned[index] ?? 0)
    const b = sextet(cleaned[index + 1] ?? 0)
    const c = sextet(cleaned[index + 2] ?? 0)
    const d = sextet(cleaned[index + 3] ?? 0)
    if (a < 0 || b < 0 || c < 0 || d < 0) return null
    output[outputIndex] = (a << 2) | (b >> 4)
    outputIndex += 1
    if (cleaned[index + 2] !== 0x3d) {
      output[outputIndex] = ((b & 0x0f) << 4) | (c >> 2)
      outputIndex += 1
    }
    if (cleaned[index + 3] !== 0x3d) {
      output[outputIndex] = ((c & 0x03) << 6) | d
      outputIndex += 1
    }
  }
  return output
}

function isPng(bytes: Uint8Array): boolean {
  return (
    matches(bytes, 0, PNG_MAGIC) &&
    bytes.length >= 24 &&
    byte(bytes, 12) === 0x49 &&
    byte(bytes, 13) === 0x48 &&
    byte(bytes, 14) === 0x44 &&
    byte(bytes, 15) === 0x52
  )
}

function isJpeg(bytes: Uint8Array): boolean {
  return bytes.length >= 4 && byte(bytes, 0) === 0xff && byte(bytes, 1) === 0xd8
}

function isWebp(bytes: Uint8Array): boolean {
  return (
    bytes.length >= 12 &&
    byte(bytes, 0) === 0x52 &&
    byte(bytes, 1) === 0x49 &&
    byte(bytes, 2) === 0x46 &&
    byte(bytes, 3) === 0x46 &&
    byte(bytes, 8) === 0x57 &&
    byte(bytes, 9) === 0x45 &&
    byte(bytes, 10) === 0x42 &&
    byte(bytes, 11) === 0x50
  )
}

function readUint32(bytes: Uint8Array, offset: number): number {
  return (
    ((byte(bytes, offset) << 24) |
      (byte(bytes, offset + 1) << 16) |
      (byte(bytes, offset + 2) << 8) |
      byte(bytes, offset + 3)) >>>
    0
  )
}

function pngDimensions(
  bytes: Uint8Array,
): { width: number; height: number } | null {
  if (!isPng(bytes)) return null
  return { width: readUint32(bytes, 16), height: readUint32(bytes, 20) }
}

function jpegDimensions(
  bytes: Uint8Array,
): { width: number; height: number } | null {
  if (!isJpeg(bytes)) return null
  let index = 2
  while (index + 1 < bytes.length) {
    if (byte(bytes, index) !== 0xff) return null
    const marker = byte(bytes, index + 1)
    index += 2
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd9)) continue
    if (index + 1 >= bytes.length) return null
    const length = (byte(bytes, index) << 8) | byte(bytes, index + 1)
    if (length < 2 || index + length > bytes.length) return null
    const sof =
      (marker >= 0xc0 && marker <= 0xc3) ||
      (marker >= 0xc5 && marker <= 0xc7) ||
      (marker >= 0xc9 && marker <= 0xcb) ||
      (marker >= 0xcd && marker <= 0xcf)
    if (sof) {
      if (index + 6 >= bytes.length) return null
      const height = (byte(bytes, index + 3) << 8) | byte(bytes, index + 4)
      const width = (byte(bytes, index + 5) << 8) | byte(bytes, index + 6)
      return { width, height }
    }
    index += length
  }
  return null
}

function dimensionsFor(
  bytes: Uint8Array,
  format: RasterFormat,
): { width: number; height: number } | null {
  return format === "png" ? pngDimensions(bytes) : jpegDimensions(bytes)
}

function formatMatches(bytes: Uint8Array, format: RasterFormat): boolean {
  return format === "png" ? isPng(bytes) : isJpeg(bytes)
}

export function encodeValidatedRaster(
  bytes: Uint8Array,
  format: RasterFormat,
): RasterValidation {
  if (bytes.byteLength > MAX_RASTER_DECODED_BYTES) {
    return { ok: false, code: "LIMIT_EXCEEDED" }
  }
  if (!formatMatches(bytes, format)) {
    return { ok: false, code: "INTERNAL_ERROR" }
  }
  const dimensions = dimensionsFor(bytes, format)
  if (dimensions === null) return { ok: false, code: "INTERNAL_ERROR" }
  const { width, height } = dimensions
  if (width > MAX_RASTER_SIDE || height > MAX_RASTER_SIDE) {
    return { ok: false, code: "LIMIT_EXCEEDED" }
  }
  if (width * height > MAX_RASTER_PIXELS) {
    return { ok: false, code: "LIMIT_EXCEEDED" }
  }
  const dataBase64 = encodeBase64(bytes)
  if (dataBase64.length > MAX_RASTER_BASE64_BYTES) {
    return { ok: false, code: "LIMIT_EXCEEDED" }
  }
  return {
    ok: true,
    format,
    width,
    height,
    decodedBytes: bytes.byteLength,
    base64Bytes: dataBase64.length,
    dataBase64,
  }
}

export function validateEmbeddedImageData(
  bytes: Uint8Array,
  mime: EmbeddedImageMime,
): boolean {
  if (bytes.byteLength === 0 || bytes.byteLength > MAX_RASTER_DECODED_BYTES) {
    return false
  }
  if (mime === "image/png") return isPng(bytes)
  if (mime === "image/jpeg" || mime === "image/jpg") return isJpeg(bytes)
  return isWebp(bytes)
}
