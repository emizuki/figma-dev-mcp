import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { FONT_SEGMENT_RANGE, getFonts } from "./fonts"

const MIXED = Symbol("figma.mixed")

const page = (id: string, name: string, children: unknown[] = []) => ({
  id,
  name,
  type: "PAGE",
  visible: true,
  children,
  loadAsync: async () => {},
})

const text = (
  id: string,
  extras: Record<string, unknown> = {},
): Record<string, unknown> => ({
  id,
  name: id,
  type: "TEXT",
  visible: true,
  children: [],
  characters: "Hello",
  fontName: { family: "Inter", style: "Regular" },
  ...extras,
})

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
  available?: { family: string; style: string }[]
  forbidCatalog?: boolean
}): {
  currentPage: Record<string, unknown>
  loadedPages: string[]
} {
  const loadedPages: string[] = []
  const pages = (options.pages ?? [options.currentPage]).map((item) => {
    const load = item.loadAsync
    if (typeof load !== "function") return item
    return {
      ...item,
      loadAsync: async () => {
        loadedPages.push(String(item.id))
        await load.call(item)
      },
    }
  })
  const nodes = options.nodes ?? new Map()
  const current =
    pages.find((item) => item.id === options.currentPage.id) ??
    options.currentPage
  const api = {
    root: { name: "Checkout flow", children: pages },
    currentPage: current,
    editorType: "dev",
    mixed: MIXED,
    loadAllPagesAsync: async () => {
      throw new Error("fonts must not load every page")
    },
    loadFontAsync: async () => {
      throw new Error("fonts must not load or substitute fonts")
    },
    listAvailableFontsAsync: options.forbidCatalog
      ? undefined
      : async () =>
          (options.available ?? [{ family: "Inter", style: "Regular" }]).map(
            (fontName) => ({ fontName }),
          ),
    getNodeByIdAsync: async (id: string) => {
      if (nodes.has(id)) return nodes.get(id)
      return pages.find((item) => item.id === id) ?? null
    },
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: current, loadedPages }
}

describe("get_fonts", () => {
  beforeEach(() => {
    installFigma({ currentPage: page("0:2", "Current") })
  })

  test("collects plain text fonts and reports observed availability", async () => {
    const heading = text("5:1", {
      characters: "Title",
      fontName: { family: "Inter", style: "Bold" },
    })
    const body = text("5:2")
    const current = page("0:2", "Current", [heading, body])
    installFigma({
      currentPage: current,
      available: [
        { family: "Inter", style: "Regular" },
        { family: "Inter", style: "Bold" },
      ],
    })

    const result = await getFonts({})
    expect(result.truncated).toBe(false)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "Bold" },
        availability: "available",
        nodeIds: ["5:1"],
      },
      {
        font: { family: "Inter", style: "Regular" },
        availability: "available",
        nodeIds: ["5:2"],
      },
    ])
  })

  test("reads mixed styled ranges without loading fonts", async () => {
    const segments = [
      {
        characters: "Hello",
        start: 0,
        end: 5,
        fontName: { family: "Inter", style: "Bold" },
      },
      {
        characters: " world",
        start: 5,
        end: 11,
        fontName: { family: "Inter", style: "Regular" },
      },
    ]
    const calls: {
      fields: unknown
      start: number | undefined
      end: number | undefined
    }[] = []
    const mixed = text("5:1", {
      characters: "Hello world",
      fontName: MIXED,
      getStyledTextSegments: (
        fields: unknown,
        start?: number,
        end?: number,
      ) => {
        calls.push({ fields, start, end })
        return segments.filter(
          (segment) =>
            start !== undefined &&
            end !== undefined &&
            segment.start < end &&
            segment.end > start,
        )
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [mixed]),
      available: [
        { family: "Inter", style: "Regular" },
        { family: "Inter", style: "Bold" },
      ],
    })

    const result = await getFonts({})
    expect(calls.length).toBeGreaterThan(0)
    for (const call of calls) {
      expect(call.fields).toEqual(["fontName"])
      expect(call.start).toBeTypeOf("number")
      expect(call.end).toBeTypeOf("number")
    }
    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "Bold" },
        availability: "available",
        nodeIds: ["5:1"],
      },
      {
        font: { family: "Inter", style: "Regular" },
        availability: "available",
        nodeIds: ["5:1"],
      },
    ])
  })

  test("dedupes duplicate family and style pairs across nodes", async () => {
    const first = text("5:1")
    const second = text("5:2", {
      characters: "Again",
      fontName: { family: "Inter", style: "Regular" },
    })
    const third = text("5:3", {
      characters: "Bold",
      fontName: { family: "Inter", style: "Bold" },
    })
    installFigma({
      currentPage: page("0:2", "Current", [first, second, third]),
      available: [
        { family: "Inter", style: "Regular" },
        { family: "Inter", style: "Bold" },
      ],
    })

    const result = await getFonts({})
    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "Regular" },
        availability: "available",
        nodeIds: ["5:1", "5:2"],
      },
      {
        font: { family: "Inter", style: "Bold" },
        availability: "available",
        nodeIds: ["5:3"],
      },
    ])
  })

  test("marks fonts missing from the catalog as unavailable", async () => {
    const missing = text("5:1", {
      fontName: { family: "Comic Sans", style: "Regular" },
    })
    const present = text("5:2")
    installFigma({
      currentPage: page("0:2", "Current", [missing, present]),
      available: [{ family: "Inter", style: "Regular" }],
    })

    const result = await getFonts({})
    expect(result.fonts).toEqual([
      {
        font: { family: "Comic Sans", style: "Regular" },
        availability: "unavailable",
        nodeIds: ["5:1"],
      },
      {
        font: { family: "Inter", style: "Regular" },
        availability: "available",
        nodeIds: ["5:2"],
      },
    ])
  })

  test("returns unknown availability when the catalog cannot be observed", async () => {
    installFigma({
      currentPage: page("0:2", "Current", [text("5:1")]),
      forbidCatalog: true,
    })

    const result = await getFonts({})
    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "Regular" },
        availability: "unknown",
        nodeIds: ["5:1"],
      },
    ])
  })

  test("scans mixed text in bounded character ranges", async () => {
    const length = FONT_SEGMENT_RANGE * 2 + 10
    const calls: { start: number | undefined; end: number | undefined }[] = []
    const mixed = text("5:1", {
      characters: "x".repeat(length),
      fontName: MIXED,
      getStyledTextSegments: (
        _fields: unknown,
        start?: number,
        end?: number,
      ) => {
        calls.push({ start, end })
        return [
          {
            start: start ?? 0,
            end: end ?? length,
            fontName: { family: "Inter", style: "Regular" },
          },
        ]
      },
    })
    installFigma({ currentPage: page("0:2", "Current", [mixed]) })

    const result = await getFonts({})
    expect(calls).toEqual([
      { start: 0, end: FONT_SEGMENT_RANGE },
      { start: FONT_SEGMENT_RANGE, end: FONT_SEGMENT_RANGE * 2 },
      { start: FONT_SEGMENT_RANGE * 2, end: length },
    ])
    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "Regular" },
        availability: "available",
        nodeIds: ["5:1"],
      },
    ])
  })

  test("loads an explicit page without changing the current page", async () => {
    const requested = page("0:1", "Requested", [text("5:1")])
    const current = page("0:2", "Current", [
      text("5:9", { fontName: { family: "Other", style: "Regular" } }),
    ])
    const { currentPage, loadedPages } = installFigma({
      currentPage: current,
      pages: [requested, current],
    })

    const result = await getFonts({ selector: { pageId: requested.id } })
    expect(loadedPages).toEqual(["0:1"])
    expect(
      (globalThis as typeof globalThis & { figma: { currentPage: unknown } })
        .figma.currentPage,
    ).toBe(currentPage)
    expect(result.fonts.map((item) => item.nodeIds)).toEqual([["5:1"]])
  })

  test("keeps fonts already indexed when the visit ceiling is hit", async () => {
    const first = text("5:1")
    const extras = Array.from({ length: 4 }, (_, index) => ({
      id: `1:${index + 1}`,
      name: "Padding",
      type: "FRAME",
      visible: true,
      children: [],
    }))
    const later = text("5:2", {
      fontName: { family: "Inter", style: "Bold" },
    })
    installFigma({
      currentPage: page("0:2", "Current", [first, ...extras, later]),
      available: [
        { family: "Inter", style: "Regular" },
        { family: "Inter", style: "Bold" },
      ],
    })

    const result = await getFonts({}, undefined, {
      returnedNodes: 10,
      visitedNodes: 2,
      encodedBytes: 8 * 1024 * 1024,
    })

    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "Regular" },
        availability: "available",
        nodeIds: ["5:1"],
      },
    ])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: 2,
    })
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      text(`5:${index + 1}`),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return text("5:51")
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    await expect(
      getFonts({ selector: { pageId: requested.id } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })
})
