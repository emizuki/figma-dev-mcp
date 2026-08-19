import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getStyles } from "./styles"

const page = (id: string, name: string, children: unknown[] = []) => ({
  id,
  name,
  type: "PAGE",
  visible: true,
  children,
  loadAsync: async () => {},
})

const node = (
  id: string,
  name: string,
  extras: Record<string, unknown> = {},
) => ({
  id,
  name,
  type: "FRAME",
  visible: true,
  children: [],
  ...extras,
})

function paintStyle(id: string, name: string) {
  return {
    id,
    name,
    type: "PAINT",
    description: "Brand fill",
    remote: false,
    key: "paint-key",
    leftover: "must-not-leak",
    paints: [
      {
        type: "SOLID",
        color: { r: 1, g: 0, b: 0, a: 1 },
        opacity: 0.8,
      },
    ],
  }
}

function textStyle(id: string, name: string) {
  return {
    id,
    name,
    type: "TEXT",
    description: "Body",
    remote: true,
    key: "text-key",
    leftover: "must-not-leak",
    fontName: { family: "Inter", style: "Regular" },
    fontSize: 16,
    lineHeight: { unit: "PIXELS", value: 24 },
    letterSpacing: { unit: "PIXELS", value: 0.5 },
  }
}

function effectStyle(id: string, name: string) {
  return {
    id,
    name,
    type: "EFFECT",
    description: "Elevation",
    remote: false,
    key: "effect-key",
    leftover: "must-not-leak",
    effects: [
      {
        type: "DROP_SHADOW",
        color: { r: 0, g: 0, b: 0, a: 0.4 },
        offset: { x: 1, y: 2 },
        radius: 4,
        spread: 1,
      },
    ],
  }
}

function gridStyle(id: string, name: string) {
  return {
    id,
    name,
    type: "GRID",
    description: "8pt",
    remote: false,
    key: "grid-key",
    leftover: "must-not-leak",
    layoutGrids: [{ pattern: "GRID", sectionSize: 8 }],
  }
}

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
  styles?: Map<string, unknown>
  local?: {
    paint?: unknown[]
    text?: unknown[]
    effect?: unknown[]
    grid?: unknown[]
  }
  forbidLocal?: boolean
  forbidGetStyle?: boolean
}): {
  currentPage: Record<string, unknown>
  localCalls: string[]
  styleLookups: string[]
} {
  const localCalls: string[] = []
  const styleLookups: string[] = []
  const pages = options.pages ?? [options.currentPage]
  const nodes = options.nodes ?? new Map()
  const styles = options.styles ?? new Map()
  const api = {
    root: { name: "Checkout flow", children: pages },
    currentPage: options.currentPage,
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("styles must not call loadAllPagesAsync")
    },
    getNodeByIdAsync: async (id: string) => {
      if (nodes.has(id)) return nodes.get(id)
      return pages.find((item) => item.id === id) ?? null
    },
    getLocalPaintStylesAsync: async () => {
      if (options.forbidLocal) throw new Error("local paint styles forbidden")
      localCalls.push("paint")
      return options.local?.paint ?? []
    },
    getLocalTextStylesAsync: async () => {
      if (options.forbidLocal) throw new Error("local text styles forbidden")
      localCalls.push("text")
      return options.local?.text ?? []
    },
    getLocalEffectStylesAsync: async () => {
      if (options.forbidLocal) throw new Error("local effect styles forbidden")
      localCalls.push("effect")
      return options.local?.effect ?? []
    },
    getLocalGridStylesAsync: async () => {
      if (options.forbidLocal) throw new Error("local grid styles forbidden")
      localCalls.push("grid")
      return options.local?.grid ?? []
    },
    getStyleByIdAsync: async (id: string) => {
      if (options.forbidGetStyle) throw new Error("getStyleByIdAsync forbidden")
      styleLookups.push(id)
      return styles.get(id) ?? null
    },
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: options.currentPage, localCalls, styleLookups }
}

describe("get_styles", () => {
  beforeEach(() => {
    installFigma({ currentPage: page("0:2", "Current") })
  })

  test("serializes local paint, text, effect, and grid styles without leaking extra fields", async () => {
    const local = {
      paint: [paintStyle("S:paint", "Brand/Fill")],
      text: [textStyle("S:text", "Body")],
      effect: [effectStyle("S:effect", "Elevation")],
      grid: [gridStyle("S:grid", "8pt")],
    }
    const { localCalls, styleLookups } = installFigma({
      currentPage: page("0:2", "Current"),
      local,
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" })

    expect(localCalls).toEqual(["paint", "text", "effect", "grid"])
    expect(styleLookups).toEqual([])
    expect(result.truncated).toBe(false)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(result.styles).toEqual([
      {
        styleType: "paint",
        id: "S:paint",
        name: "Brand/Fill",
        description: "Brand fill",
        remote: false,
        key: "paint-key",
        paints: [
          {
            type: "solid",
            color: { r: 1, g: 0, b: 0, a: 1 },
            opacity: 0.8,
          },
        ],
      },
      {
        styleType: "text",
        id: "S:text",
        name: "Body",
        description: "Body",
        remote: true,
        key: "text-key",
        text: {
          characters: "",
          defaultStyle: {
            fontFamily: "Inter",
            fontStyle: "Regular",
            fontSize: 16,
            lineHeight: { unit: "pixels", value: 24 },
            letterSpacing: { unit: "pixels", value: 0.5 },
            paints: [],
          },
          styledRanges: [],
        },
      },
      {
        styleType: "effect",
        id: "S:effect",
        name: "Elevation",
        description: "Elevation",
        remote: false,
        key: "effect-key",
        effects: [
          {
            type: "dropShadow",
            color: { r: 0, g: 0, b: 0, a: 0.4 },
            offsetX: 1,
            offsetY: 2,
            radius: 4,
            spread: 1,
          },
        ],
      },
      {
        styleType: "grid",
        id: "S:grid",
        name: "8pt",
        description: "8pt",
        remote: false,
        key: "grid-key",
        pattern: "grid",
        size: 8,
      },
    ])
    for (const style of result.styles) {
      expect(Object.keys(style)).not.toContain("leftover")
    }
  })

  test("omits description, remote, and key when Figma does not expose them", async () => {
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        paint: [
          {
            id: "S:bare",
            name: "Bare",
            type: "PAINT",
            leftover: "must-not-leak",
            paints: [
              {
                type: "SOLID",
                color: { r: 0, g: 0, b: 1, a: 1 },
                opacity: 1,
              },
            ],
          },
        ],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" })
    expect(result.styles).toEqual([
      {
        styleType: "paint",
        id: "S:bare",
        name: "Bare",
        paints: [
          {
            type: "solid",
            color: { r: 0, g: 0, b: 1, a: 1 },
            opacity: 1,
          },
        ],
      },
    ])
    expect(Object.keys(result.styles[0] ?? {})).not.toContain("description")
    expect(Object.keys(result.styles[0] ?? {})).not.toContain("remote")
    expect(Object.keys(result.styles[0] ?? {})).not.toContain("key")
    expect(Object.keys(result.styles[0] ?? {})).not.toContain("leftover")
  })

  test("collects node-referenced styles once and skips local readers", async () => {
    const fill = paintStyle("S:fill", "Fill")
    const stroke = paintStyle("S:stroke", "Stroke")
    const text = textStyle("S:text", "Label")
    const effect = effectStyle("S:effect", "Shadow")
    const grid = gridStyle("S:grid", "Layout")
    const child = node("1:2", "Child", {
      fillStyleId: "S:fill",
      textStyleId: "S:text",
    })
    const root = node("1:1", "Root", {
      children: [child],
      strokeStyleId: "S:stroke",
      effectStyleId: "S:effect",
      gridStyleId: "S:grid",
      fillStyleId: "S:fill",
    })
    const requested = page("0:1", "Requested", [root])
    const current = page("0:2", "Current")
    const { currentPage, localCalls, styleLookups } = installFigma({
      currentPage: current,
      pages: [requested, current],
      nodes: new Map<string, unknown>([[requested.id, requested]]),
      styles: new Map<string, unknown>([
        [fill.id, fill],
        [stroke.id, stroke],
        [text.id, text],
        [effect.id, effect],
        [grid.id, grid],
      ]),
      forbidLocal: true,
    })

    const result = await getStyles({
      source: "referenced",
      selector: { pageId: requested.id },
    })

    expect(currentPage).toBe(current)
    expect(localCalls).toEqual([])
    expect(styleLookups).toEqual([
      "S:fill",
      "S:stroke",
      "S:effect",
      "S:grid",
      "S:text",
    ])
    expect(result.styles.map((style) => style.id)).toEqual([
      "S:fill",
      "S:stroke",
      "S:effect",
      "S:grid",
      "S:text",
    ])
  })

  test("both unions local and referenced styles and defaults when source is omitted", async () => {
    const localPaint = paintStyle("S:local", "Local")
    const referenced = paintStyle("S:ref", "Referenced")
    const selected = node("1:1", "Card", { fillStyleId: "S:ref" })
    const current = {
      ...page("0:2", "Current", [selected]),
      selection: [selected],
    }
    const { localCalls, styleLookups } = installFigma({
      currentPage: current,
      nodes: new Map<string, unknown>([[selected.id, selected]]),
      styles: new Map<string, unknown>([
        [localPaint.id, localPaint],
        [referenced.id, referenced],
      ]),
      local: { paint: [localPaint] },
    })

    const result = await getStyles({ selector: { selection: true } })

    expect(localCalls).toEqual(["paint", "text", "effect", "grid"])
    expect(styleLookups).toEqual(["S:ref"])
    expect(result.styles.map((style) => style.id)).toEqual(["S:local", "S:ref"])
  })

  test("does not re-resolve a style that is already local in both mode", async () => {
    const shared = paintStyle("S:shared", "Shared")
    const selected = node("1:1", "Card", { fillStyleId: "S:shared" })
    const { styleLookups } = installFigma({
      currentPage: { ...page("0:2", "Current", [selected]), selection: [] },
      nodes: new Map<string, unknown>([[selected.id, selected]]),
      styles: new Map<string, unknown>([[shared.id, shared]]),
      local: { paint: [shared] },
    })

    const result = await getStyles({
      source: "both",
      selector: { nodeId: selected.id },
    })

    expect(styleLookups).toEqual([])
    expect(result.styles).toHaveLength(1)
    expect(result.styles[0]?.id).toBe("S:shared")
  })

  test("loads an explicit page without changing the current page", async () => {
    let loaded = 0
    const current = page("0:2", "Current")
    const requested = {
      ...page("0:1", "Requested", [
        node("1:1", "Card", { fillStyleId: "S:fill" }),
      ]),
      loadAsync: async () => {
        loaded += 1
      },
    }
    const fill = paintStyle("S:fill", "Fill")
    installFigma({
      currentPage: current,
      pages: [requested, current],
      nodes: new Map<string, unknown>([[requested.id, requested]]),
      styles: new Map<string, unknown>([[fill.id, fill]]),
    })

    const result = await getStyles({
      source: "referenced",
      selector: { pageId: requested.id },
    })

    expect(loaded).toBe(1)
    expect(result.styles.map((style) => style.id)).toEqual(["S:fill"])
    expect(
      (globalThis as typeof globalThis & { figma: { currentPage: unknown } })
        .figma.currentPage,
    ).toBe(current)
  })

  test("fails missing explicit roots without falling back to the current page", async () => {
    const current = page("0:2", "Current", [
      node("1:1", "Card", { fillStyleId: "S:fill" }),
    ])
    const rectangle = node("1:9", "Not a page")
    installFigma({
      currentPage: current,
      pages: [current],
      nodes: new Map<string, unknown>([[rectangle.id, rectangle]]),
      styles: new Map<string, unknown>([
        ["S:fill", paintStyle("S:fill", "Fill")],
      ]),
    })

    await expect(
      getStyles({ source: "referenced", selector: { pageId: "0:1" } }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      getStyles({
        source: "referenced",
        selector: { pageId: rectangle.id },
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      getStyles({ source: "referenced", selector: { nodeId: "missing" } }),
    ).rejects.toMatchObject({ code: "NODE_NOT_FOUND" })
    expect(PluginReadError).toBeDefined()
  })

  test("fails when required style APIs are unavailable", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
    }

    await expect(getStyles({ source: "local" })).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
    await expect(getStyles({ source: "referenced" })).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
  })

  test("bounds returned styles and reports truncation", async () => {
    const paints = [
      paintStyle("S:1", "One"),
      paintStyle("S:2", "Two"),
      paintStyle("S:3", "Three"),
    ]
    installFigma({
      currentPage: page("0:2", "Current"),
      local: { paint: paints },
    })

    const result = await getStyles({ source: "local" }, undefined, {
      returnedNodes: 2,
      visitedNodes: 10,
      encodedBytes: 8 * 1024 * 1024,
    })

    expect(result.styles.map((style) => style.id)).toEqual(["S:1", "S:2"])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: expect.any(Number),
    })
  })

  test("skips mixed style ids and does not look up the string mixed", async () => {
    const mixed = Symbol("figma.mixed")
    const card = node("1:1", "Card", {
      fillStyleId: mixed,
      strokeStyleId: "mixed",
      textStyleId: "S:text",
    })
    const { styleLookups } = installFigma({
      currentPage: page("0:2", "Current", [card]),
      nodes: new Map([[card.id, card]]),
      styles: new Map([["S:text", textStyle("S:text", "Body")]]),
    })
    ;(
      globalThis as typeof globalThis & { figma: { mixed: unknown } }
    ).figma.mixed = mixed

    const result = await getStyles({
      source: "referenced",
      selector: { nodeId: card.id },
    })
    expect(styleLookups).toEqual(["S:text"])
    expect(result.styles.map((style) => style.id)).toEqual(["S:text"])
  })

  test("collects mixed text style ids from styled segments", async () => {
    const mixed = Symbol("figma.mixed")
    const text = {
      id: "2:1",
      name: "Label",
      type: "TEXT",
      visible: true,
      children: [],
      fillStyleId: mixed,
      textStyleId: mixed,
      getStyledTextSegments: (fields: string[]) => {
        expect(fields).toEqual(["textStyleId", "fillStyleId"])
        return [
          { textStyleId: "S:range", fillStyleId: "S:fill" },
          { textStyleId: mixed, fillStyleId: "S:fill" },
        ]
      },
    }
    const { styleLookups } = installFigma({
      currentPage: page("0:2", "Current", [text]),
      nodes: new Map([[text.id, text]]),
      styles: new Map<string, unknown>([
        ["S:range", textStyle("S:range", "Range")],
        ["S:fill", paintStyle("S:fill", "Fill")],
      ]),
    })
    ;(
      globalThis as typeof globalThis & { figma: { mixed: unknown } }
    ).figma.mixed = mixed

    const result = await getStyles({
      source: "referenced",
      selector: { nodeId: text.id },
    })
    expect(styleLookups.sort()).toEqual(["S:fill", "S:range"])
    expect(result.styles.map((style) => style.id).sort()).toEqual([
      "S:fill",
      "S:range",
    ])
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      node(`1:${index + 1}`, "Item", { fillStyleId: `S:${index + 1}` }),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return node("1:51", "Item", { fillStyleId: "S:51" })
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
      nodes: new Map<string, unknown>([[requested.id, requested]]),
      styles: new Map(),
    })

    await expect(
      getStyles(
        { source: "referenced", selector: { pageId: requested.id } },
        cancellation.signal,
      ),
    ).rejects.toThrow("Operation cancelled")
  })
})
