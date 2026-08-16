import { beforeEach, describe, expect, test } from "bun:test"

import {
  LocalCancellationController,
  LocalCancellationError,
} from "../main/cancellation"
import {
  bindProgress,
  createProgressReporter,
  type ProgressFrame,
} from "../main/progress"
import { parseBrokerToPlugin } from "../shared/validation"
import {
  completeScreenshotValidation,
  getScreenshot,
  type ScreenshotCodec,
} from "./render"

const page = (id: string, name: string, selection: { id: string }[] = []) => ({
  id,
  name,
  type: "PAGE",
  selection,
})

const passthrough: ScreenshotCodec = {
  async encodeRaster(_format, bytes) {
    const dataBase64 = Buffer.from(bytes).toString("base64")
    return {
      ok: true,
      dataBase64,
      width: 1,
      height: 1,
      decodedBytes: bytes.byteLength,
      base64Bytes: dataBase64.length,
    }
  },
  async encodeSvg(source) {
    return { ok: true, source }
  },
}

function exportNode(
  id: string,
  exporter: (settings: Record<string, unknown>) => Promise<unknown>,
): Record<string, unknown> {
  return {
    id,
    type: "FRAME",
    exportAsync: exporter,
  }
}

function installFigma(options: {
  selection?: { id: string }[]
  nodes?: Map<string, unknown>
  ui?: { postMessage(message: unknown): void }
}): { exports: Record<string, unknown>[] } {
  const exports: Record<string, unknown>[] = []
  const nodes = options.nodes ?? new Map()
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
    root: { name: "Checkout flow", children: [page("0:1", "Page 1")] },
    currentPage: page("0:1", "Page 1", options.selection ?? []),
    editorType: "dev",
    getNodeByIdAsync: async (id: string) => nodes.get(id) ?? null,
    ...(options.ui === undefined ? {} : { ui: options.ui }),
  }
  return { exports }
}

function parseScreenshotInput(input: Record<string, unknown>): unknown {
  return parseBrokerToPlugin({
    type: "request",
    requestId: "plugin-1",
    deadlineMs: 1,
    target: {},
    operation: { operation: "get_screenshot", input },
  })
}

describe("get_screenshot export selection", () => {
  beforeEach(() => {
    installFigma({})
  })

  test("rejects SVG scale and raster-only fields before dispatch", () => {
    expect(() =>
      parseScreenshotInput({
        format: "svg",
        selector: { nodeId: "1:2" },
        scale: 2,
      }),
    ).toThrow()
    expect(() =>
      parseScreenshotInput({
        format: "svg",
        selector: { nodeId: "1:2" },
        width: 1,
      }),
    ).toThrow()
    expect(() =>
      parseScreenshotInput({
        format: "png",
        selector: { nodeId: "1:2" },
        svgOutlineText: true,
      }),
    ).toThrow()
  })

  test("defaults SVG export options when they are omitted", () => {
    const parsed = parseScreenshotInput({
      format: "svg",
      selector: { nodeId: "1:2" },
    }) as {
      operation: {
        input: {
          svgOutlineText: boolean
          svgIdAttribute: boolean
          svgSimplifyStroke: boolean
        }
      }
    }
    expect(parsed.operation.input).toMatchObject({
      svgOutlineText: true,
      svgIdAttribute: false,
      svgSimplifyStroke: true,
    })
  })

  test("passes raster scale bounds 0.25 through 4.0 to exportAsync", async () => {
    const calls: unknown[] = []
    const node = exportNode("1:2", async (settings) => {
      calls.push(settings)
      return new Uint8Array([1, 2, 3])
    })
    installFigma({ nodes: new Map([["1:2", node]]) })

    for (const scale of [0.25, 1, 4]) {
      await getScreenshot(
        { format: "png", selector: { nodeId: "1:2" }, scale },
        undefined,
        passthrough,
      )
    }

    expect(calls).toEqual([
      { format: "PNG", constraint: { type: "SCALE", value: 0.25 } },
      { format: "PNG", constraint: { type: "SCALE", value: 1 } },
      { format: "PNG", constraint: { type: "SCALE", value: 4 } },
    ])
  })

  test("rejects raster scale outside 0.25 through 4.0", async () => {
    const node = exportNode("1:2", async () => new Uint8Array([1]))
    installFigma({ nodes: new Map([["1:2", node]]) })
    await expect(
      getScreenshot(
        { format: "jpeg", selector: { nodeId: "1:2" }, scale: 4.01 },
        undefined,
        passthrough,
      ),
    ).rejects.toMatchObject({ code: "LIMIT_EXCEEDED" })
    await expect(
      getScreenshot(
        { format: "png", selector: { nodeId: "1:2" }, scale: 0.24 },
        undefined,
        passthrough,
      ),
    ).rejects.toMatchObject({ code: "LIMIT_EXCEEDED" })
  })

  test("defaults omitted raster scale to 1 and uses JPG for jpeg", async () => {
    let settings: Record<string, unknown> | undefined
    const node = exportNode("2:2", async (value) => {
      settings = value
      return new Uint8Array([9])
    })
    installFigma({ nodes: new Map([["2:2", node]]) })
    await getScreenshot(
      { format: "jpeg", selector: { nodeId: "2:2" } },
      undefined,
      passthrough,
    )
    expect(settings).toEqual({
      format: "JPG",
      constraint: { type: "SCALE", value: 1 },
    })
  })

  test("SVG export uses only SVG_STRING options", async () => {
    let settings: Record<string, unknown> | undefined
    const node = exportNode("3:3", async (value) => {
      settings = value
      return "<svg xmlns='http://www.w3.org/2000/svg'/>"
    })
    installFigma({ nodes: new Map([["3:3", node]]) })
    await getScreenshot(
      {
        format: "svg",
        selector: { nodeId: "3:3" },
        svgOutlineText: false,
        svgIdAttribute: true,
        svgSimplifyStroke: false,
      },
      undefined,
      passthrough,
    )
    expect(settings).toEqual({
      format: "SVG_STRING",
      svgOutlineText: false,
      svgIdAttribute: true,
      svgSimplifyStroke: false,
    })
    expect(settings).not.toHaveProperty("scale")
    expect(settings).not.toHaveProperty("constraint")
  })

  test("captures ordered node IDs and keeps successful assets when one item fails", async () => {
    const order: string[] = []
    const nodes = new Map<string, unknown>([
      [
        "1:1",
        exportNode("1:1", async () => {
          order.push("1:1")
          return new Uint8Array([1])
        }),
      ],
      [
        "1:3",
        exportNode("1:3", async () => {
          order.push("1:3")
          return new Uint8Array([3])
        }),
      ],
    ])
    installFigma({ nodes })

    const result = await getScreenshot(
      { format: "png", selector: { nodeIds: ["1:1", "1:2", "1:3"] } },
      undefined,
      passthrough,
    )
    expect(order).toEqual(["1:1", "1:3"])
    expect(result.assets).toHaveLength(3)
    expect(result.assets[0]).toMatchObject({
      status: "success",
      value: { format: "png", nodeId: "1:1" },
    })
    expect(result.assets[1]).toMatchObject({
      status: "error",
      error: { code: "NODE_NOT_FOUND", retryable: false },
    })
    expect(result.assets[2]).toMatchObject({
      status: "success",
      value: { format: "png", nodeId: "1:3" },
    })
    expect(result.truncated).toBe(false)
  })

  test("keeps successful assets when one export throws", async () => {
    const nodes = new Map<string, unknown>([
      ["2:1", exportNode("2:1", async () => new Uint8Array([1]))],
      [
        "2:2",
        exportNode("2:2", async () => {
          throw new Error("export failed")
        }),
      ],
    ])
    installFigma({ nodes })
    const result = await getScreenshot(
      { format: "png", selector: { nodeIds: ["2:1", "2:2"] } },
      undefined,
      passthrough,
    )
    expect(result.assets[0]?.status).toBe("success")
    expect(result.assets[1]).toMatchObject({
      status: "error",
      error: { code: "INTERNAL_ERROR", retryable: false },
    })
  })

  test("captured empty selection succeeds with an empty asset list", async () => {
    installFigma({ selection: [] })
    const result = await getScreenshot(
      { format: "png", selector: { selection: true } },
      undefined,
      passthrough,
    )
    expect(result.assets).toEqual([])
    expect(result.truncated).toBe(false)
  })

  test("snapshots selection IDs before lookups", async () => {
    const current = page("0:1", "Page 1", [{ id: "4:1" }])
    const node = exportNode("4:1", async () => {
      current.selection = []
      return new Uint8Array([4])
    })
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [current] },
      currentPage: current,
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => (id === "4:1" ? node : null),
    }
    const result = await getScreenshot(
      { format: "png", selector: { selection: true } },
      undefined,
      passthrough,
    )
    expect(result.assets).toHaveLength(1)
    expect(result.assets[0]).toMatchObject({
      status: "success",
      value: { nodeId: "4:1" },
    })
  })

  test("loads a PAGE before exportAsync", async () => {
    const events: string[] = []
    const requestedPage = {
      id: "0:1",
      name: "Requested",
      type: "PAGE",
      loadAsync: async () => {
        events.push("load")
      },
      exportAsync: async () => {
        events.push("export")
        return new Uint8Array([9])
      },
    }
    installFigma({ nodes: new Map([["0:1", requestedPage]]) })

    const result = await getScreenshot(
      { format: "png", selector: { nodeId: "0:1" } },
      undefined,
      passthrough,
    )

    expect(events).toEqual(["load", "export"])
    expect(result.assets[0]).toMatchObject({
      status: "success",
      value: { nodeId: "0:1" },
    })
  })

  test("maps a throwing screenshot lookup to NODE_NOT_FOUND", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [page("0:1", "Page 1")] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => {
        throw new Error("invalid node id")
      },
    }
    const result = await getScreenshot(
      { format: "png", selector: { nodeId: "00:00000" } },
      undefined,
      passthrough,
    )
    expect(result.assets[0]).toMatchObject({
      status: "error",
      error: { code: "NODE_NOT_FOUND", retryable: false },
    })
  })

  test("treats a node without exportAsync as unsupported", async () => {
    installFigma({
      nodes: new Map([["5:1", { id: "5:1", type: "DOCUMENT" }]]),
    })
    const result = await getScreenshot(
      { format: "png", selector: { nodeId: "5:1" } },
      undefined,
      passthrough,
    )
    expect(result.assets[0]).toMatchObject({
      status: "error",
      error: { code: "UNSUPPORTED_NODE", retryable: false },
    })
  })

  test("propagates cancellation between captured nodes", async () => {
    const controller = new LocalCancellationController()
    const nodes = new Map<string, unknown>([
      [
        "6:1",
        exportNode("6:1", async () => {
          controller.abort()
          return new Uint8Array([1])
        }),
      ],
      ["6:2", exportNode("6:2", async () => new Uint8Array([2]))],
    ])
    installFigma({ nodes })
    await expect(
      getScreenshot(
        { format: "png", selector: { nodeIds: ["6:1", "6:2"] } },
        controller.signal,
        passthrough,
      ),
    ).rejects.toBeInstanceOf(LocalCancellationError)
  })

  test("emits encoding progress from the export loop", async () => {
    const frames: ProgressFrame[] = []
    const controller = new LocalCancellationController()
    bindProgress(
      controller.signal,
      createProgressReporter({
        emit: (frame) => frames.push(frame),
        intervalMs: 0,
      }),
    )
    const node = exportNode("7:1", async () => new Uint8Array([1]))
    installFigma({ nodes: new Map([["7:1", node]]) })

    await getScreenshot(
      { format: "png", selector: { nodeId: "7:1" } },
      controller.signal,
      passthrough,
    )

    expect(frames.length).toBeGreaterThan(0)
    expect(frames.every((frame) => frame.message === "encoding")).toBe(true)
    expect(
      frames.some((frame) => frame.completed === 1 && frame.total === 1),
    ).toBe(true)
  })

  test("times out a hung screenshot validation as TIMEOUT", async () => {
    const node = exportNode("8:1", async () => new Uint8Array([1, 2, 3]))
    installFigma({
      nodes: new Map([["8:1", node]]),
      ui: { postMessage() {} },
    })

    const result = await getScreenshot(
      { format: "png", selector: { nodeId: "8:1" } },
      undefined,
      undefined,
      20,
    )
    expect(result.assets[0]).toMatchObject({
      status: "error",
      error: { code: "TIMEOUT", retryable: false },
    })
    expect(
      completeScreenshotValidation({
        type: "screenshotValidated",
        validationId: "screenshot-0",
        asset: {
          status: "success",
          value: {
            format: "png",
            nodeId: "8:1",
            dataBase64: "AA==",
            width: 1,
            height: 1,
          },
        },
      }),
    ).toBe(false)
  })

  test("resolves hung screenshot validation on abort as CANCELLED", async () => {
    const controller = new LocalCancellationController()
    const node = exportNode("9:1", async () => new Uint8Array([1]))
    installFigma({
      nodes: new Map([["9:1", node]]),
      ui: {
        postMessage() {
          controller.abort()
        },
      },
    })

    await expect(
      getScreenshot(
        { format: "png", selector: { nodeId: "9:1" } },
        controller.signal,
        undefined,
        1_000,
      ),
    ).rejects.toBeInstanceOf(LocalCancellationError)
  })
})
