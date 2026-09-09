import { describe, expect, test } from "bun:test"

import {
  MAX_RASTER_BASE64_BYTES,
  MAX_RASTER_DECODED_BYTES,
  MAX_RASTER_PIXELS,
  MAX_RASTER_SIDE,
} from "../shared/limits"
import {
  decodeBase64,
  encodeBase64,
  encodeValidatedRaster,
  validateEmbeddedImageData,
} from "./raster"

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit += 1) {
      const carry = crc & 1
      crc >>>= 1
      if (carry !== 0) crc ^= 0xedb88320
    }
  }
  return (crc ^ 0xffffffff) >>> 0
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const body = new Uint8Array(4 + type.length + data.length + 4)
  const view = new DataView(body.buffer)
  view.setUint32(0, data.length)
  for (let index = 0; index < type.length; index += 1) {
    body[4 + index] = type.charCodeAt(index)
  }
  body.set(data, 8)
  const crcSource = body.subarray(4, 8 + data.length)
  view.setUint32(8 + data.length, crc32(crcSource))
  return body
}

function pngWithSize(width: number, height: number, extra = 0): Uint8Array {
  const magic = Uint8Array.of(0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)
  const ihdr = new Uint8Array(13)
  const view = new DataView(ihdr.buffer)
  view.setUint32(0, width)
  view.setUint32(4, height)
  ihdr[8] = 8
  ihdr[9] = 2
  const parts = [
    magic,
    chunk("IHDR", ihdr),
    chunk("IDAT", Uint8Array.of(0)),
    chunk("IEND", new Uint8Array()),
  ]
  const total = parts.reduce((sum, part) => sum + part.length, 0)
  const bytes = new Uint8Array(total + extra)
  let offset = 0
  for (const part of parts) {
    bytes.set(part, offset)
    offset += part.length
  }
  return bytes
}

function jpegWithSize(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(20)
  bytes.set([0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0b, 0x08], 0)
  const view = new DataView(bytes.buffer)
  view.setUint16(7, height)
  view.setUint16(9, width)
  bytes[11] = 1
  bytes[12] = 1
  bytes[13] = 0x11
  bytes[14] = 0
  bytes[15] = 0xff
  bytes[16] = 0xd9
  return bytes
}

describe("raster validation", () => {
  test("never creates a Blob URL, download, path, or SVG rasterization", async () => {
    const source = await Bun.file(
      new URL("./raster.ts", import.meta.url),
    ).text()
    for (const forbidden of [
      "createObjectURL",
      "revokeObjectURL",
      "new Blob",
      "download",
      "showSaveFilePicker",
      "HTMLCanvasElement",
      "OffscreenCanvas",
    ]) {
      expect(source).not.toContain(forbidden)
    }
  })

  test("accepts PNG and JPEG magic with encoded dimensions and encodes once", () => {
    const png = pngWithSize(8, 4)
    const jpeg = jpegWithSize(8, 4)
    const pngResult = encodeValidatedRaster(png, "png")
    const jpegResult = encodeValidatedRaster(jpeg, "jpeg")
    expect(pngResult).toMatchObject({
      ok: true,
      format: "png",
      width: 8,
      height: 4,
      decodedBytes: png.byteLength,
    })
    expect(jpegResult).toMatchObject({
      ok: true,
      format: "jpeg",
      width: 8,
      height: 4,
      decodedBytes: jpeg.byteLength,
    })
    if (!pngResult.ok || !jpegResult.ok) throw new Error("expected success")
    expect(Buffer.from(pngResult.dataBase64, "base64")).toEqual(
      Buffer.from(png),
    )
    expect(pngResult.base64Bytes).toBe(pngResult.dataBase64.length)
    expect(jpegResult.base64Bytes).toBe(jpegResult.dataBase64.length)
  })

  test("rejects magic that does not match the declared format", () => {
    expect(encodeValidatedRaster(pngWithSize(1, 1), "jpeg").ok).toBe(false)
    expect(encodeValidatedRaster(jpegWithSize(1, 1), "png").ok).toBe(false)
    expect(encodeValidatedRaster(Uint8Array.of(1, 2, 3, 4), "png").ok).toBe(
      false,
    )
  })

  test("rejects sides above 4096 and more than 16 megapixels", () => {
    expect(
      encodeValidatedRaster(pngWithSize(MAX_RASTER_SIDE + 1, 1), "png"),
    ).toMatchObject({
      ok: false,
      code: "LIMIT_EXCEEDED",
    })
    expect(encodeValidatedRaster(pngWithSize(4001, 4000), "png")).toMatchObject(
      {
        ok: false,
        code: "LIMIT_EXCEEDED",
      },
    )
    expect(4001 * 4000).toBeGreaterThan(MAX_RASTER_PIXELS)
    expect(encodeValidatedRaster(pngWithSize(4000, 4000), "png").ok).toBe(true)
    expect(
      encodeValidatedRaster(jpegWithSize(MAX_RASTER_SIDE, 1), "jpeg").ok,
    ).toBe(true)
  })

  test("rejects decoded and base64 sizes above the fixed ceilings", () => {
    const oversized = pngWithSize(1, 1, MAX_RASTER_DECODED_BYTES)
    expect(oversized.byteLength).toBeGreaterThan(MAX_RASTER_DECODED_BYTES)
    expect(encodeValidatedRaster(oversized, "png")).toMatchObject({
      ok: false,
      code: "LIMIT_EXCEEDED",
    })

    const atDecodedLimit = pngWithSize(
      1,
      1,
      MAX_RASTER_DECODED_BYTES - pngWithSize(1, 1).byteLength,
    )
    expect(atDecodedLimit.byteLength).toBe(MAX_RASTER_DECODED_BYTES)
    const encoded = encodeValidatedRaster(atDecodedLimit, "png")
    expect(encoded.ok).toBe(true)
    if (!encoded.ok) throw new Error("expected success")
    expect(encoded.base64Bytes).toBeLessThanOrEqual(MAX_RASTER_BASE64_BYTES)
    expect(encoded.decodedBytes).toBe(MAX_RASTER_DECODED_BYTES)
  })

  test("embedded SVG data URLs verify declared raster MIME against magic", () => {
    expect(validateEmbeddedImageData(pngWithSize(1, 1), "image/png")).toBe(true)
    expect(validateEmbeddedImageData(jpegWithSize(1, 1), "image/jpeg")).toBe(
      true,
    )
    expect(validateEmbeddedImageData(pngWithSize(1, 1), "image/jpeg")).toBe(
      false,
    )
    const webp = new Uint8Array(12)
    webp.set([0x52, 0x49, 0x46, 0x46], 0)
    webp.set([0x57, 0x45, 0x42, 0x50], 8)
    expect(validateEmbeddedImageData(webp, "image/webp")).toBe(true)
    expect(validateEmbeddedImageData(pngWithSize(1, 1), "image/webp")).toBe(
      false,
    )
  })
})

describe("raster acceptance", () => {
  test("base64 encoding uses the whole alphabet and both padding lengths", () => {
    // 62 and 63 are the two characters a hand-written table gets wrong, and
    // nothing encoded them before: 0xfb 0xef 0xbe is four sextets of 62, and
    // 0xff 0xff 0xff is four of 63.
    expect(encodeBase64(Uint8Array.of(0xfb, 0xef, 0xbe))).toBe("++++")
    expect(encodeBase64(Uint8Array.of(0xff, 0xff, 0xff))).toBe("////")
    expect(encodeBase64(Uint8Array.of(0x4d, 0x61, 0x6e))).toBe("TWFu")
    // One trailing byte pads with two "=", two trailing bytes with one.
    expect(encodeBase64(Uint8Array.of(0x4d))).toBe("TQ==")
    expect(encodeBase64(Uint8Array.of(0x4d, 0x61))).toBe("TWE=")
    expect(encodeBase64(new Uint8Array())).toBe("")
  })

  test("base64 decoding reverses that, skipping whitespace and honouring padding", () => {
    expect(decodeBase64("++++")).toEqual(Uint8Array.of(0xfb, 0xef, 0xbe))
    expect(decodeBase64("////")).toEqual(Uint8Array.of(0xff, 0xff, 0xff))
    expect(decodeBase64("TQ==")).toEqual(Uint8Array.of(0x4d))
    expect(decodeBase64("TWE=")).toEqual(Uint8Array.of(0x4d, 0x61))
    // Whitespace is dropped before the quartets are counted, so a payload
    // wrapped across lines still decodes to the same bytes.
    expect(decodeBase64("TW\nFu\t TWFu\r")).toEqual(
      Uint8Array.of(0x4d, 0x61, 0x6e, 0x4d, 0x61, 0x6e),
    )
    for (const sample of [
      Uint8Array.of(0x00),
      Uint8Array.of(0x00, 0xff),
      Uint8Array.of(0x01, 0x02, 0x03, 0x04, 0x05),
    ]) {
      expect(decodeBase64(encodeBase64(sample))).toEqual(sample)
    }
  })

  test("a side exactly at the ceiling is accepted, in both dimensions", () => {
    expect(
      encodeValidatedRaster(pngWithSize(MAX_RASTER_SIDE, 1), "png"),
    ).toMatchObject({ ok: true, width: MAX_RASTER_SIDE, height: 1 })
    expect(
      encodeValidatedRaster(pngWithSize(1, MAX_RASTER_SIDE), "png"),
    ).toMatchObject({ ok: true, width: 1, height: MAX_RASTER_SIDE })
  })

  test("an embedded image exactly at the decoded-byte ceiling is still accepted", () => {
    const atLimit = pngWithSize(
      1,
      1,
      MAX_RASTER_DECODED_BYTES - pngWithSize(1, 1).byteLength,
    )
    expect(atLimit.byteLength).toBe(MAX_RASTER_DECODED_BYTES)
    expect(validateEmbeddedImageData(atLimit, "image/png")).toBe(true)
    expect(validateEmbeddedImageData(new Uint8Array(), "image/png")).toBe(false)
  })

  test("image/jpg is accepted as its own spelling of the JPEG media type", () => {
    expect(validateEmbeddedImageData(jpegWithSize(2, 2), "image/jpg")).toBe(
      true,
    )
    expect(validateEmbeddedImageData(pngWithSize(2, 2), "image/jpg")).toBe(
      false,
    )
  })

  test("JPEG dimensions are read past skippable segments, from every SOF marker", () => {
    // Restart and TEM markers carry no length; an APP0 segment does and must
    // be stepped over by exactly its own length. Every start-of-frame marker
    // in the four accepted ranges names the same 10x6 frame.
    const markers = [
      0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce,
      0xcf,
    ]
    let checked = 0
    for (const marker of markers) {
      const bytes = Uint8Array.from([
        0xff,
        0xd8,
        0xff,
        0x01,
        0xff,
        0xd0,
        0xff,
        0xe0,
        0x00,
        0x06,
        0x4a,
        0x46,
        0x49,
        0x46,
        0xff,
        marker,
        0x00,
        0x0b,
        0x08,
        0x00,
        0x06,
        0x00,
        0x0a,
        0x01,
        0x01,
        0x11,
        0x00,
        0xff,
        0xd9,
      ])
      expect({
        marker,
        result: encodeValidatedRaster(bytes, "jpeg"),
      }).toMatchObject({
        marker,
        result: { ok: true, format: "jpeg", width: 10, height: 6 },
      })
      checked += 1
    }
    expect(checked).toBe(markers.length)
    expect(checked).toBe(13)
  })

  test("a PNG carrying nothing but its header is still measured", () => {
    // 24 bytes exactly: magic, chunk length, "IHDR", width, height. The
    // dimension reader needs no more than that.
    const header = new Uint8Array(24)
    header.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 0)
    header.set([0x00, 0x00, 0x00, 0x0d], 8)
    header.set([0x49, 0x48, 0x44, 0x52], 12)
    header.set([0x00, 0x00, 0x00, 0x20], 16)
    header.set([0x00, 0x00, 0x00, 0x10], 20)

    expect(encodeValidatedRaster(header, "png")).toMatchObject({
      ok: true,
      width: 32,
      height: 16,
    })
    expect(encodeValidatedRaster(header.subarray(0, 23), "png").ok).toBe(false)
  })

  test("a four-byte JPEG is enough to recognise, though not to measure", () => {
    const soi = Uint8Array.of(0xff, 0xd8, 0xff, 0xd9)
    expect(validateEmbeddedImageData(soi, "image/jpeg")).toBe(true)
    expect(validateEmbeddedImageData(soi.subarray(0, 3), "image/jpeg")).toBe(
      false,
    )
  })
})
