import { afterEach, describe, expect, test } from "bun:test"
import { DOMParser, onErrorStopParsing } from "@xmldom/xmldom"

import { onControllerMessage, sendToController } from "./relay"

// The relay is the iframe half of the screenshot pipeline: the controller
// hands it a raw export, it validates and encodes it, and hands back a
// `screenshotValidated` reply. Every refusal in that path was pinned and no
// acceptance was, so the relay could stop answering `validateScreenshot`
// altogether without a single test noticing.

type Listener = (event: { data?: unknown }) => void

interface Harness {
  readonly sent: unknown[]
  deliver(pluginMessage: unknown): void
  deliverRaw(data: unknown): void
  listenerCount(type: string): number
  restore(): void
}

function installRelayHarness(): Harness {
  // An untyped view of `globalThis`, because the fake `DOMParser` installed
  // below returns the structural document `svg.ts` asks for rather than a full
  // `Document`.
  const hostGlobals = globalThis as unknown as Record<string, unknown>
  const original = {
    window: global.window,
    parent: global.parent,
    DOMParser: hostGlobals.DOMParser,
  }
  const listeners = new Map<string, Set<Listener>>()
  const sent: unknown[] = []

  global.window = {
    addEventListener: (type: string, listener: Listener) => {
      const bucket = listeners.get(type) ?? new Set<Listener>()
      bucket.add(listener)
      listeners.set(type, bucket)
    },
    removeEventListener: (type: string, listener: Listener) => {
      listeners.get(type)?.delete(listener)
    },
  } as unknown as Window & typeof globalThis
  global.parent = {
    postMessage: (message: unknown) => {
      sent.push((message as { pluginMessage?: unknown }).pluginMessage)
    },
  } as unknown as Window
  hostGlobals.DOMParser = class {
    readonly #parser = new DOMParser({ onError: onErrorStopParsing })
    parseFromString(source: string, type: string): unknown {
      return this.#parser.parseFromString(source, type as "image/svg+xml")
    }
  }

  return {
    sent,
    deliver(pluginMessage: unknown) {
      this.deliverRaw({ pluginMessage })
    },
    deliverRaw(data: unknown) {
      for (const listener of listeners.get("message") ?? []) listener({ data })
    },
    listenerCount(type: string) {
      return listeners.get(type)?.size ?? 0
    },
    restore() {
      global.window = original.window
      global.parent = original.parent
      hostGlobals.DOMParser = original.DOMParser
    },
  }
}

// 8x4 PNG: magic, then an IHDR chunk whose width and height sit at the fixed
// offsets the validator reads. `encodeValidatedRaster` never decodes pixels,
// so no image data is needed past the 24-byte header.
const PNG_8x4 = new Uint8Array(25)
PNG_8x4.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 0)
PNG_8x4.set([0x00, 0x00, 0x00, 0x0d], 8)
PNG_8x4.set([0x49, 0x48, 0x44, 0x52], 12)
PNG_8x4.set([0x00, 0x00, 0x00, 0x08], 16)
PNG_8x4.set([0x00, 0x00, 0x00, 0x04], 20)
PNG_8x4[24] = 0x08
const PNG_8x4_BASE64 = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAECA=="

// 10x6 JPEG: SOI, then an SOF0 segment carrying the two dimensions, then EOI.
const JPEG_10x6 = new Uint8Array(17)
JPEG_10x6.set([0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0b, 0x08], 0)
JPEG_10x6.set([0x00, 0x06], 7)
JPEG_10x6.set([0x00, 0x0a], 9)
JPEG_10x6.set([0x01, 0x01, 0x11, 0x00, 0xff, 0xd9], 11)
const JPEG_10x6_BASE64 = "/9j/wAALCAAGAAoBAREA/9k="

const SAFE_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg"><rect fill="#0af" width="4" height="4"/></svg>'
const UNSAFE_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="go()"/></svg>'

let harness: Harness | undefined

afterEach(() => {
  harness?.restore()
  harness = undefined
})

function start(): { harness: Harness; stop: () => void } {
  const installed = installRelayHarness()
  harness = installed
  const received: unknown[] = []
  const stop = onControllerMessage((message) => {
    received.push(message)
  })
  return { harness: installed, stop }
}

describe("controller relay", () => {
  test("validates a PNG screenshot and answers with the encoded asset", () => {
    const { harness, stop } = start()
    harness.deliver({
      type: "validateScreenshot",
      validationId: "screenshot-1",
      item: { format: "png", nodeId: "4:1", bytes: PNG_8x4 },
    })
    stop()

    expect(harness.sent).toEqual([
      {
        type: "screenshotValidated",
        validationId: "screenshot-1",
        asset: {
          status: "success",
          value: {
            format: "png",
            nodeId: "4:1",
            dataBase64: PNG_8x4_BASE64,
            width: 8,
            height: 4,
          },
        },
      },
    ])
  })

  test("validates a JPEG screenshot, which the PNG path cannot stand in for", () => {
    const { harness, stop } = start()
    harness.deliver({
      type: "validateScreenshot",
      validationId: "screenshot-2",
      item: { format: "jpeg", nodeId: "4:2", bytes: JPEG_10x6 },
    })
    stop()

    expect(harness.sent).toEqual([
      {
        type: "screenshotValidated",
        validationId: "screenshot-2",
        asset: {
          status: "success",
          value: {
            format: "jpeg",
            nodeId: "4:2",
            dataBase64: JPEG_10x6_BASE64,
            width: 10,
            height: 6,
          },
        },
      },
    ])
  })

  test("accepts the export bytes in every shape the host can hand over", () => {
    const buffer = PNG_8x4.buffer.slice(0) as ArrayBuffer
    const shapes: Record<string, unknown> = {
      Uint8Array: PNG_8x4,
      ArrayBuffer: buffer,
      DataView: new DataView(buffer),
      "number[]": Array.from(PNG_8x4),
    }
    const { harness, stop } = start()
    for (const [name, bytes] of Object.entries(shapes)) {
      harness.deliver({
        type: "validateScreenshot",
        validationId: name,
        item: { format: "png", nodeId: "4:3", bytes },
      })
    }
    stop()

    expect(harness.sent).toHaveLength(Object.keys(shapes).length)
    for (const reply of harness.sent) {
      expect(reply).toMatchObject({
        asset: {
          status: "success",
          value: { dataBase64: PNG_8x4_BASE64, width: 8, height: 4 },
        },
      })
    }
  })

  test("returns an SVG screenshot with its own source and safety verdict", () => {
    const { harness, stop } = start()
    harness.deliver({
      type: "validateScreenshot",
      validationId: "screenshot-svg",
      item: { format: "svg", nodeId: "4:4", source: SAFE_SVG },
    })
    harness.deliver({
      type: "validateScreenshot",
      validationId: "screenshot-svg-unsafe",
      item: { format: "svg", nodeId: "4:5", source: UNSAFE_SVG },
    })
    stop()

    expect(harness.sent).toEqual([
      {
        type: "screenshotValidated",
        validationId: "screenshot-svg",
        asset: {
          status: "success",
          value: {
            format: "svg",
            nodeId: "4:4",
            source: SAFE_SVG,
            safe: true,
          },
        },
      },
      {
        type: "screenshotValidated",
        validationId: "screenshot-svg-unsafe",
        asset: {
          status: "success",
          value: {
            format: "svg",
            nodeId: "4:5",
            source: UNSAFE_SVG,
            safe: false,
            rejection: { kind: "unsafeAttribute", name: "onclick" },
          },
        },
      },
    ])
  })

  test("subscribes to message events and unsubscribes from the same channel", () => {
    const { harness, stop } = start()
    expect(harness.listenerCount("message")).toBe(1)

    harness.deliver({
      type: "validateScreenshot",
      validationId: "before-stop",
      item: { format: "png", nodeId: "4:6", bytes: PNG_8x4 },
    })
    stop()
    expect(harness.listenerCount("message")).toBe(0)
    harness.deliver({
      type: "validateScreenshot",
      validationId: "after-stop",
      item: { format: "png", nodeId: "4:6", bytes: PNG_8x4 },
    })

    expect(harness.sent).toHaveLength(1)
    expect(harness.sent[0]).toMatchObject({ validationId: "before-stop" })
  })

  test("posts controller-bound messages inside a pluginMessage envelope", () => {
    const { harness, stop } = start()
    sendToController({
      type: "requestControllerReady",
      metadataRequestId: "123e4567-e89b-42d3-a456-426614174000",
    })
    stop()

    expect(harness.sent).toEqual([
      {
        type: "requestControllerReady",
        metadataRequestId: "123e4567-e89b-42d3-a456-426614174000",
      },
    ])
  })
})
