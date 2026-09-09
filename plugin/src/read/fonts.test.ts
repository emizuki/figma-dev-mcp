import { beforeEach, describe, expect, test } from "bun:test"

import { FIGMA_MIXED, installFigma } from "../../tests/figma-harness"
import { LocalCancellationController } from "../main/cancellation"
import { MAX_INPUT_IDS } from "../shared/limits"
import { FONT_SEGMENT_RANGE, getFonts } from "./fonts"
import { byteLength } from "./serialize"

const MIXED = FIGMA_MIXED

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
      omitCatalog: true,
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

  test("a font needs both halves, and one empty half is still a font", async () => {
    const text = (id: string, fontName: unknown) => ({
      id,
      name: id,
      type: "TEXT",
      visible: true,
      children: [],
      characters: "Hi",
      fontName,
    })
    installFigma({
      currentPage: page("0:2", "Current", [
        text("1:1", { family: "Inter" }),
        text("1:2", { style: "Bold" }),
        text("1:3", { family: "Inter", style: "" }),
        text("1:4", { family: "", style: "Bold" }),
        text("1:5", { family: "", style: "" }),
      ]),
      available: [{ family: "Inter", style: "" }],
    })

    const result = await getFonts({})

    expect(result.fonts).toEqual([
      {
        font: { family: "Inter", style: "" },
        availability: "available",
        nodeIds: ["1:3"],
      },
      {
        font: { family: "", style: "Bold" },
        availability: "unavailable",
        nodeIds: ["1:4"],
      },
    ])
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("a segment window the host refuses costs only that window", async () => {
    const characters = "x".repeat(FONT_SEGMENT_RANGE + 10)
    const mixed = {
      id: "1:1",
      name: "1:1",
      type: "TEXT",
      visible: true,
      children: [],
      characters,
      fontName: FIGMA_MIXED,
      getStyledTextSegments: (_fields: string[], start: number) => {
        if (start === 0) throw new Error("segment read failed")
        return [{ fontName: { family: "Tail", style: "Regular" } }]
      },
    }
    installFigma({
      currentPage: page("0:2", "Current", [mixed]),
      available: [],
    })

    const result = await getFonts({})

    expect(result.fonts).toEqual([
      {
        font: { family: "Tail", style: "Regular" },
        availability: "unavailable",
        nodeIds: ["1:1"],
      },
    ])
  })

  test("the catalogue is read off figma itself, and a catalogue that throws leaves availability unknown", async () => {
    const text = {
      id: "1:1",
      name: "1:1",
      type: "TEXT",
      visible: true,
      children: [],
      characters: "Hi",
      fontName: { family: "Inter", style: "Regular" },
    }
    installFigma({ currentPage: page("0:2", "Current", [text]) })
    const host = (
      globalThis as typeof globalThis & { figma: Record<string, unknown> }
    ).figma
    host.badge = { family: "Inter", style: "Regular" }
    host.listAvailableFontsAsync = async function (this: {
      badge: { family: string; style: string }
    }) {
      return [{ fontName: this.badge }]
    }

    const observed = await getFonts({})
    expect(observed.fonts[0]?.availability).toBe("available")

    host.listAvailableFontsAsync = async () => {
      throw new Error("catalogue unavailable")
    }
    const unknown = await getFonts({})
    expect(unknown.fonts[0]?.availability).toBe("unknown")
  })

  test("the font ceiling, the walk cut and the byte ceiling each report their own total", async () => {
    const text = (id: string, family: string) => ({
      id,
      name: id,
      type: "TEXT",
      visible: true,
      children: [],
      characters: "Hi",
      fontName: { family, style: "Regular" },
    })
    installFigma({
      currentPage: page("0:2", "Current", [
        text("1:1", "Inter"),
        text("1:2", "Roboto"),
        text("1:3", "Georgia"),
      ]),
      available: [],
    })

    const capped = await getFonts({}, undefined, { returnedNodes: 1 })
    expect(capped.fonts).toHaveLength(1)
    expect(capped.truncated).toBe(true)
    expect(capped.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })

    const walked = await getFonts({}, undefined, {
      visitedNodes: 3,
      returnedNodes: 1,
    })
    expect(walked.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })

    const usage = (family: string, id: string) => ({
      font: { family, style: "Regular" },
      availability: "unavailable" as const,
      nodeIds: [id],
    })
    const budget =
      byteLength(usage("Inter", "1:1")) + byteLength(usage("Roboto", "1:2")) - 1
    const bytes = await getFonts({}, undefined, { encodedBytes: budget })
    expect(bytes.fonts).toEqual([usage("Inter", "1:1")])
    expect(bytes.truncation).toEqual({
      reason: "byteLimit",
      encodedBytes:
        byteLength(usage("Inter", "1:1")) + byteLength(usage("Roboto", "1:2")),
    })
  })

  test("a font's node list stops at MAX_INPUT_IDS", async () => {
    const children = Array.from({ length: MAX_INPUT_IDS + 1 }, (_, index) => ({
      id: `1:${index + 1}`,
      name: `1:${index + 1}`,
      type: "TEXT",
      visible: true,
      children: [],
      characters: "Hi",
      fontName: { family: "Inter", style: "Regular" },
    }))
    installFigma({
      currentPage: page("0:2", "Current", children),
      available: [],
    })

    const result = await getFonts({})

    expect(result.fonts[0]?.nodeIds).toHaveLength(MAX_INPUT_IDS)
    expect(result.fonts[0]?.nodeIds.at(-1)).toBe(`1:${MAX_INPUT_IDS}`)
  })

  // The font loop's own check: nothing batched guards it, and the catalogue
  // read that precedes it polls the signal before the await, not after.
  test("the font loop checks cancellation on every font", async () => {
    const cancellation = new LocalCancellationController()
    const text = {
      id: "1:1",
      name: "1:1",
      type: "TEXT",
      visible: true,
      children: [],
      characters: "Hi",
      fontName: { family: "Inter", style: "Regular" },
    }
    installFigma({ currentPage: page("0:2", "Current", [text]) })
    const host = (
      globalThis as typeof globalThis & { figma: Record<string, unknown> }
    ).figma
    host.listAvailableFontsAsync = async () => {
      cancellation.abort()
      return []
    }

    await expect(getFonts({}, cancellation.signal)).rejects.toThrow(
      "Operation cancelled",
    )
  })

  test("the font catalogue read checks cancellation before it starts", async () => {
    const cancellation = new LocalCancellationController()
    const child: Record<string, unknown> = {
      id: "1:1",
      name: "1:1",
      type: "FRAME",
      visible: true,
    }
    // No TEXT node anywhere, so no font is collected and the emission loop —
    // whose own check would otherwise catch this — has nothing to iterate.
    Object.defineProperty(child, "children", {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return []
      },
    })
    installFigma({ currentPage: page("0:2", "Current", [child]) })

    await expect(getFonts({}, cancellation.signal)).rejects.toThrow(
      "Operation cancelled",
    )
  })

  test("the font loop stops at the emission ceiling", async () => {
    const text = (id: string, family: string) => ({
      id,
      name: id,
      type: "TEXT",
      visible: true,
      children: [],
      characters: "Hi",
      fontName: { family, style: "Regular" },
    })
    installFigma({
      currentPage: page("0:2", "Current", [
        text("1:1", "Inter"),
        text("1:2", "Roboto"),
        text("1:3", "Georgia"),
        text("1:4", "Menlo"),
      ]),
      available: [],
    })

    const result = await getFonts({}, undefined, { returnedNodes: 2 })

    expect(result.fonts.map((usage) => usage.font.family)).toEqual([
      "Inter",
      "Roboto",
    ])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })
  })
})
