import { beforeEach, describe, expect, test } from "bun:test"

import { FIGMA_MIXED, installFigma } from "../../tests/figma-harness"
import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { byteLength } from "./serialize"
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
    // Restates the claim; it does not enforce it. `forbidGetStyle` installs
    // `getStyleByIdAsync` as a thrower, so a local read that touched it would
    // blow this test up before reaching here — and this recorder, which only
    // the non-forbidding reader ever pushes to, could not have caught it
    // either way. The thrower is the tripwire; this line documents what it
    // guards. (Verified: injecting `figma.getStyleByIdAsync?.("MUTANT")` into
    // `emitLocal` fails this test via the thrower, not via this assertion.)
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
    const { localCalls, styleLookups } = installFigma({
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

    // Was `toBe(current)` against the harness's returned `currentPage`
    // channel. That channel is an install-time snapshot (`installFigma`
    // returns `currentPage: current` once, at setup, before `getStyles` ever
    // runs) — it has no way to observe a later switch, so comparing it
    // cannot fail no matter what the code under test does. Reading the live
    // host's `figma.currentPage` is what actually checks that requesting an
    // explicit page's styles didn't move the current page away from
    // `current`; verified by injecting a current-page switch into
    // `loadPageIfNeeded` and confirming this assertion (and only this
    // form) catches it.
    expect(
      (
        globalThis as typeof globalThis & {
          figma: { currentPage: { id: unknown } }
        }
      ).figma.currentPage.id,
    ).toBe(current.id)
    // Same shape as the note at the `styleLookups` assertion above:
    // `forbidLocal` installs the four `getLocal*StylesAsync` readers as
    // throwers, so a referenced read that enumerated local styles fails here
    // loudly. This line records the intent; the throwers enforce it.
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
    // Was `toBe(current)`: same cause as above, on the installed host's
    // channel rather than the returned one. The harness wraps `current`
    // (it carries `loadAsync`) in a proxy, so `figma.currentPage` is no
    // longer the exact fixture even though it is still the current page.
    // Comparing `.id` checks the thing the test cares about: loading the
    // explicit page did not change which page is current.
    expect(
      (
        globalThis as typeof globalThis & {
          figma: { currentPage: { id: unknown } }
        }
      ).figma.currentPage.id,
    ).toBe(current.id)
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
    // `omit*`, not `forbid*`: this is the one styles test whose subject *is*
    // the missing-capability path, so the readers have to be genuinely absent
    // for production's `=== undefined` checks to reach CAPABILITY_UNAVAILABLE.
    // `forbid*` installs throwers, which are present and would blow up here.
    installFigma({
      currentPage: page("0:1", "Page 1"),
      omitLocal: true,
      omitGetStyle: true,
    })

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
    const card = node("1:1", "Card", {
      fillStyleId: FIGMA_MIXED,
      strokeStyleId: "mixed",
      textStyleId: "S:text",
    })
    const { styleLookups } = installFigma({
      currentPage: page("0:2", "Current", [card]),
      nodes: new Map([[card.id, card]]),
      styles: new Map([["S:text", textStyle("S:text", "Body")]]),
    })

    const result = await getStyles({
      source: "referenced",
      selector: { nodeId: card.id },
    })
    expect(styleLookups).toEqual(["S:text"])
    expect(result.styles.map((style) => style.id)).toEqual(["S:text"])
  })

  test("collects mixed text style ids from styled segments", async () => {
    const text = {
      id: "2:1",
      name: "Label",
      type: "TEXT",
      visible: true,
      children: [],
      fillStyleId: FIGMA_MIXED,
      textStyleId: FIGMA_MIXED,
      getStyledTextSegments: (fields: string[]) => {
        expect(fields).toEqual(["textStyleId", "fillStyleId"])
        return [
          { textStyleId: "S:range", fillStyleId: "S:fill" },
          { textStyleId: FIGMA_MIXED, fillStyleId: "S:fill" },
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

  test("drops a switched-off effect from an effect style", async () => {
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        effect: [
          {
            id: "S:effect",
            name: "Elevation",
            type: "EFFECT",
            description: "Elevation",
            remote: false,
            key: "effect-key",
            effects: [
              {
                type: "DROP_SHADOW",
                color: { r: 0, g: 0, b: 0, a: 0.4 },
                offset: { x: 1, y: 2 },
                radius: 4,
                spread: 1,
                visible: false,
              },
            ],
          },
        ],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" })

    expect(result.styles[0]).toMatchObject({
      styleType: "effect",
      id: "S:effect",
      effects: [],
    })
  })

  test("drops a switched-off paint from a paint style", async () => {
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        paint: [
          {
            id: "S:paint",
            name: "Brand/Fill",
            type: "PAINT",
            description: "Brand fill",
            remote: false,
            key: "paint-key",
            paints: [
              {
                type: "SOLID",
                color: { r: 1, g: 0, b: 0, a: 1 },
                opacity: 0.8,
                visible: false,
              },
            ],
          },
        ],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" })

    expect(result.styles[0]).toMatchObject({
      styleType: "paint",
      id: "S:paint",
      paints: [],
    })
  })

  test("maps ROWS, COLUMNS and an unrecognised grid pattern", async () => {
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        grid: [
          {
            id: "S:rows",
            name: "Rows",
            type: "GRID",
            layoutGrids: [{ pattern: "ROWS", sectionSize: 4 }],
          },
          {
            id: "S:columns",
            name: "Columns",
            type: "GRID",
            layoutGrids: [{ pattern: "COLUMNS", sectionSize: 12 }],
          },
          {
            id: "S:other",
            name: "Other",
            type: "GRID",
            layoutGrids: [{ pattern: "DIAGONAL", sectionSize: 2 }],
          },
        ],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" })

    expect(result.styles).toEqual([
      {
        styleType: "grid",
        id: "S:rows",
        name: "Rows",
        pattern: "rows",
        size: 4,
      },
      {
        styleType: "grid",
        id: "S:columns",
        name: "Columns",
        pattern: "columns",
        size: 12,
      },
      {
        styleType: "grid",
        id: "S:other",
        name: "Other",
        pattern: "diagonal",
        size: 2,
      },
    ])
  })

  test("a segment reader that throws keeps the ids it already yielded", async () => {
    const text = {
      id: "1:10",
      name: "Label",
      type: "TEXT",
      visible: true,
      children: [],
      getStyledTextSegments: () => [
        { textStyleId: "S:early" },
        {
          get textStyleId(): string {
            throw new Error("segment read failed")
          },
        },
      ],
    }
    installFigma({
      currentPage: page("0:2", "Current", [text]),
      styles: new Map<string, unknown>([
        [
          "S:early",
          { id: "S:early", name: "Early", type: "PAINT", paints: [] },
        ],
      ]),
      forbidLocal: true,
    })

    const result = await getStyles({ source: "referenced" })

    expect(result.styles).toEqual([
      { styleType: "paint", id: "S:early", name: "Early", paints: [] },
    ])
  })

  test("the node ceiling reports how many styles were considered", async () => {
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        paint: [
          paintStyle("S:one", "One"),
          paintStyle("S:two", "Two"),
          paintStyle("S:three", "Three"),
        ],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" }, undefined, {
      returnedNodes: 1,
    })

    expect(result.styles.map((style) => style.id)).toEqual(["S:one"])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("a style already emitted is not emitted twice", async () => {
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        paint: [paintStyle("S:dup", "First")],
        text: [{ ...textStyle("S:dup", "Second"), type: "PAINT", paints: [] }],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" })

    expect(
      result.styles.map((style) => [style.styleType, style.id, style.name]),
    ).toEqual([["paint", "S:dup", "First"]])
  })

  test("the byte ceiling counts every emitted style and reports the total", async () => {
    const first = {
      styleType: "paint" as const,
      id: "S:one",
      name: "Brand/Fill",
      description: "Brand fill",
      remote: false,
      key: "paint-key",
      paints: [
        {
          type: "solid" as const,
          color: { r: 1, g: 0, b: 0, a: 1 },
          opacity: 0.8,
        },
      ],
    }
    const second = { ...first, id: "S:two" }
    const budget = byteLength(first) + byteLength(second) - 1
    installFigma({
      currentPage: page("0:2", "Current"),
      local: {
        paint: [
          paintStyle("S:one", "Brand/Fill"),
          paintStyle("S:two", "Brand/Fill"),
        ],
      },
      forbidGetStyle: true,
    })

    const result = await getStyles({ source: "local" }, undefined, {
      encodedBytes: budget,
    })

    expect(result.styles).toEqual([first])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "byteLimit",
      encodedBytes: byteLength(first) + byteLength(second),
    })
  })

  test("a walk that runs out of budget truncates the referenced pass", async () => {
    installFigma({
      currentPage: page("0:2", "Current", [
        node("1:1", "Card", { fillStyleId: "S:card" }),
        node("1:2", "Chip", { fillStyleId: "S:chip" }),
      ]),
      styles: new Map<string, unknown>([
        ["S:card", { id: "S:card", name: "Card", type: "PAINT", paints: [] }],
        ["S:chip", { id: "S:chip", name: "Chip", type: "PAINT", paints: [] }],
      ]),
      forbidLocal: true,
    })

    const result = await getStyles({ source: "referenced" }, undefined, {
      visitedNodes: 1,
    })

    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 1 })
  })

  test("a referenced id the host cannot resolve does not end the read", async () => {
    installFigma({
      currentPage: page("0:2", "Current", [
        node("1:1", "Card", {
          fillStyleId: "S:missing",
          strokeStyleId: "S:found",
        }),
      ]),
      styles: new Map<string, unknown>([
        [
          "S:found",
          { id: "S:found", name: "Found", type: "PAINT", paints: [] },
        ],
      ]),
      forbidLocal: true,
    })

    const result = await getStyles({ source: "referenced" })

    expect(result.styles).toEqual([
      { styleType: "paint", id: "S:found", name: "Found", paints: [] },
    ])
  })

  // `throwIfAbortedAtBatch` polls only when `index % 100 === 0`, so in a
  // catalogue shorter than a hundred entries an abort raised between batch
  // boundaries is seen by nothing but the unconditional check beside it.
  test("the local style pass checks cancellation on every style", async () => {
    const cancellation = new LocalCancellationController()
    const paints = [paintStyle("S:a", "A"), paintStyle("S:b", "B")]
    Object.defineProperty(paints, 1, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return paintStyle("S:b", "B")
      },
    })
    installFigma({
      currentPage: page("0:2", "Current"),
      local: { paint: paints },
      forbidGetStyle: true,
    })

    await expect(
      getStyles({ source: "local" }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })

  test("the referenced style pass checks cancellation on every id", async () => {
    const cancellation = new LocalCancellationController()
    installFigma({
      currentPage: page("0:2", "Current", [
        node("1:1", "Card", { fillStyleId: "S:a", strokeStyleId: "S:b" }),
      ]),
      forbidLocal: true,
    })
    const host = (
      globalThis as typeof globalThis & { figma: Record<string, unknown> }
    ).figma
    host.getStyleByIdAsync = async (id: string) => {
      if (id === "S:a") cancellation.abort()
      return { id, name: id, type: "PAINT", paints: [] }
    }

    await expect(
      getStyles({ source: "referenced" }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })
})
