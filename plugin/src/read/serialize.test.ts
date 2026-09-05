import { describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { createProgressReporter, type ProgressFrame } from "../main/progress"
import { parseReadResult } from "../shared/result-validation"
import {
  clampText,
  collectInstanceIdentities,
  collectStyleNames,
  collectVariableNames,
  effects,
  namedComponentProperties,
  paints,
  serializeNodeForest,
  TEXT_CLAMP_LIMIT,
  textStyle,
  walkNodeForest,
} from "./serialize"

const base = (
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  id: "1:1",
  name: "Card",
  type: "FRAME",
  visible: true,
  parent: { id: "0:1" },
  absoluteBoundingBox: { x: 10, y: 20, width: 300, height: 200 },
  children: [],
  ...overrides,
})

describe("bounded node serializer", () => {
  test("minimal detail emits identity and hierarchy while making depth truncation explicit", () => {
    const leaf = base({ id: "1:3", name: "Leaf", type: "RECTANGLE" })
    const child = base({ id: "1:2", name: "Child", children: [leaf] })
    const root = base({ children: [child] })

    const result = serializeNodeForest([root], {
      detail: "minimal",
      depth: 1,
      dedupeComponents: false,
    })

    expect(result.nodes[0]?.summary).toEqual({
      id: "1:1",
      name: "Card",
      nodeType: "FRAME",
      visible: true,
      parentId: "0:1",
      childIds: ["1:2"],
      bounds: { x: 10, y: 20, width: 300, height: 200 },
    })
    expect(result.nodes[0]?.data).toEqual({})
    expect(result.nodes[0]?.children[0]?.children).toEqual([])
    expect(result.nodes[0]?.children[0]?.childrenTruncation).toEqual({
      reason: "depthLimit",
      appliedDepth: 1,
    })
    expect(result.truncated).toBe(true)
    expect(result.truncation?.reason).toBe("depthLimit")
  })

  test("a node-budget stop outranks an earlier depth cut", () => {
    // A forest deep enough to cut on depth early (the first branch visited),
    // and wide enough to exhaust the node budget afterwards. The depth cut
    // must still appear on the node whose children were dropped; only the
    // document-level reason changes.
    const greatGrandchild = base({
      id: "3:4",
      name: "GreatGrandchild",
      type: "RECTANGLE",
    })
    const grandchild = base({
      id: "3:3",
      name: "Grandchild",
      children: [greatGrandchild],
    })
    const deepBranch = base({
      id: "3:2",
      name: "DeepBranch",
      children: [grandchild],
    })
    const wideSiblings = Array.from({ length: 10 }, (_, index) =>
      base({ id: `3:w${index}`, name: `Wide${index}`, type: "RECTANGLE" }),
    )
    const root = base({
      id: "3:1",
      name: "Root",
      children: [deepBranch, ...wideSiblings],
    })

    const result = serializeNodeForest([root], {
      detail: "minimal",
      depth: 2,
      dedupeComponents: false,
      limits: {
        returnedNodes: 10,
        visitedNodes: 10_000,
        encodedBytes: 10_000_000,
      },
    })

    expect(result.truncation?.reason).toBe("nodeLimit")
    expect(result.truncation?.visitedNodes).toBeGreaterThan(0)
    // The local fact stays put: grandchild's own children were cut by depth.
    const deepBranchNode = result.nodes[0]?.children[0]
    expect(deepBranchNode?.children[0]?.childrenTruncation).toEqual({
      reason: "depthLimit",
      appliedDepth: 2,
    })
  })

  test("a depth cut alone is still reported as depthLimit", () => {
    const leaf = base({ id: "2:4", name: "Leaf", type: "RECTANGLE" })
    const grandchild = base({ id: "2:3", name: "Grandchild", children: [leaf] })
    const child = base({ id: "2:2", name: "Child", children: [grandchild] })
    const root = base({ id: "2:1", name: "Root", children: [child] })

    const result = serializeNodeForest([root], {
      detail: "minimal",
      depth: 2,
      dedupeComponents: false,
      limits: {
        returnedNodes: 10_000,
        visitedNodes: 10_000,
        encodedBytes: 10_000_000,
      },
    })

    expect(result.truncation?.reason).toBe("depthLimit")
    expect(result.truncation?.appliedDepth).toBe(2)
  })

  test("omits mixed style ids from styleReferences", () => {
    const node = base({
      fillStyleId: Symbol("figma.mixed"),
      textStyleId: "mixed",
      effectStyleId: "S:effect",
      gridStyleId: "",
    })

    const result = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    })

    expect(result.nodes[0]?.data).toMatchObject({
      styleReferences: [{ id: "S:effect", kind: "effect" }],
    })
  })

  test("compact instance serialization survives write-only dynamic-page instance getters", () => {
    const instance = base({
      id: "4:1",
      name: "Calendar pill",
      type: "INSTANCE",
    })
    for (const [key, message] of [
      ["componentId", "componentId is not readable on InstanceNode 1.130"],
      ["mainComponent", "mainComponent is write-only under dynamic-page"],
      ["componentSetId", "componentSetId is not an InstanceNode field"],
    ] as const) {
      Object.defineProperty(instance, key, {
        configurable: true,
        enumerable: true,
        get() {
          throw new Error(message)
        },
      })
    }

    const result = serializeNodeForest([instance], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    })

    expect(result.nodes[0]?.summary).toMatchObject({
      id: "4:1",
      name: "Calendar pill",
      nodeType: "INSTANCE",
    })
    expect(result.nodes[0]?.data).toMatchObject({
      styleReferences: [],
      variableReferences: [],
    })
    expect(
      (result.nodes[0]?.data as { instance?: unknown }).instance,
    ).toBeUndefined()
  })

  test("resolves instance identity through getMainComponentAsync, not mainComponent", async () => {
    const instance = base({
      id: "4:1",
      name: "Calendar pill",
      type: "INSTANCE",
      getMainComponentAsync: async () => ({
        id: "8055:10274",
        type: "COMPONENT",
        parent: { id: "8055:10286", type: "COMPONENT_SET" },
      }),
    })
    Object.defineProperty(instance, "mainComponent", {
      configurable: true,
      enumerable: true,
      get() {
        throw new Error("mainComponent is write-only under dynamic-page")
      },
    })

    const identities = await collectInstanceIdentities([instance])
    const result = serializeNodeForest([instance], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
      instanceIdentities: identities,
    })

    expect(identities.get("4:1")).toEqual({
      componentId: "8055:10274",
      componentSetId: "8055:10286",
      properties: [],
    })
    expect(result.nodes[0]?.data).toMatchObject({
      instance: {
        componentId: "8055:10274",
        componentSetId: "8055:10286",
        properties: [],
      },
    })
  })

  test("does not resolve nested instances beyond the requested serialize depth", async () => {
    let nestedLookups = 0
    const nested = base({
      id: "4:2",
      name: "Nested",
      type: "INSTANCE",
      getMainComponentAsync: async () => {
        nestedLookups += 1
        return { id: "2:9", type: "COMPONENT" }
      },
    })
    const root = base({
      id: "4:1",
      name: "Outer",
      type: "INSTANCE",
      children: [nested],
      getMainComponentAsync: async () => ({
        id: "2:1",
        type: "COMPONENT",
        parent: { id: "2:0", type: "COMPONENT_SET" },
      }),
    })

    const identities = await collectInstanceIdentities([root], undefined, 0)
    expect(nestedLookups).toBe(0)
    expect([...identities.keys()]).toEqual(["4:1"])
  })

  test("compact and full detail preserve layout, mixed paints, gradients, images, effects, and rich text", () => {
    const text = base({
      type: "TEXT",
      characters: "Pay now",
      fontName: { family: "Inter", style: "Regular" },
      fontSize: 16,
      lineHeight: { unit: "PIXELS", value: 24 },
      letterSpacing: { unit: "PIXELS", value: 0 },
      fills: Symbol.for("figma.mixed"),
      fillStyleId: "S:fill",
      getStyledTextSegments: () => [
        {
          start: 0,
          end: 3,
          fontName: { family: "Inter", style: "Bold" },
          fontSize: 16,
          lineHeight: { unit: "PIXELS", value: 24 },
          letterSpacing: { unit: "PIXELS", value: 0 },
          fills: [
            {
              type: "GRADIENT_LINEAR",
              gradientStops: [
                { position: 0, color: { r: 1, g: 0, b: 0, a: 1 } },
              ],
            },
            { type: "IMAGE", imageHash: "img-1", scaleMode: "FIT" },
          ],
        },
      ],
      effects: [
        {
          type: "DROP_SHADOW",
          color: { r: 0, g: 0, b: 0, a: 0.5 },
          offset: { x: 1, y: 2 },
          radius: 4,
          spread: 1,
          visible: true,
        },
      ],
      layoutMode: "HORIZONTAL",
      primaryAxisSizingMode: "AUTO",
      counterAxisSizingMode: "FIXED",
      itemSpacing: 8,
      paddingTop: 4,
      paddingRight: 6,
      paddingBottom: 4,
      paddingLeft: 6,
    })

    const compact = serializeNodeForest([text], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]
    expect(compact?.data).toMatchObject({
      text: { characterCount: 7, preview: "Pay now" },
      autoLayout: { mode: "horizontal", primarySizing: "hug" },
      styleReferences: [{ id: "S:fill", kind: "paint" }],
    })

    const full = serializeNodeForest([text], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]
    expect(full?.data).toMatchObject({
      paints: [{ type: "mixed" }],
      effects: [
        {
          type: "dropShadow",
          offsetX: 1,
          offsetY: 2,
          radius: 4,
          spread: 1,
        },
      ],
      text: {
        characters: "Pay now",
        styledRanges: [
          {
            start: 0,
            end: 3,
            style: {
              lineHeight: { unit: "pixels", value: 24 },
              letterSpacing: { unit: "pixels", value: 0 },
              paints: [
                { type: "linearGradient" },
                { type: "image", imageRef: "img-1", scaleMode: "fit" },
              ],
            },
          },
        ],
      },
    })
  })

  test("text style carries font weight and decoration on default style and ranges", () => {
    const node = base({
      type: "TEXT",
      characters: "Save",
      fontName: { family: "Inter", style: "Light" },
      fontSize: 12,
      fontWeight: 300,
      textDecoration: "UNDERLINE",
      lineHeight: { unit: "PIXELS", value: 18 },
      letterSpacing: { unit: "PERCENT", value: 0 },
      fills: [],
      getStyledTextSegments: () => [
        {
          start: 0,
          end: 4,
          fontName: { family: "Inter", style: "Bold" },
          fontSize: 12,
          fontWeight: 700,
          textDecoration: "STRIKETHROUGH",
          lineHeight: { unit: "PIXELS", value: 18 },
          letterSpacing: { unit: "PERCENT", value: 0 },
          fills: [],
        },
      ],
    })

    const text = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { text?: Record<string, unknown> }
    ).text

    expect(text?.defaultStyle).toMatchObject({
      fontWeight: 300,
      textDecoration: "underline",
    })
    expect(
      (text?.styledRanges as { style: Record<string, unknown> }[])[0]?.style,
    ).toMatchObject({ fontWeight: 700, textDecoration: "strikethrough" })
  })

  test("mixed font weight and NONE decoration are omitted", () => {
    const node = base({
      type: "TEXT",
      characters: "Save",
      fontName: { family: "Inter", style: "Regular" },
      fontWeight: Symbol("figma.mixed"),
      textDecoration: "NONE",
      fills: [],
    })

    const style = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { text?: { defaultStyle: Record<string, unknown> } }
    ).text?.defaultStyle

    expect(Object.hasOwn(style ?? {}, "fontWeight")).toBe(false)
    expect(Object.hasOwn(style ?? {}, "textDecoration")).toBe(false)
  })

  test("text alignment and auto-resize live on the text value, not on ranges", () => {
    const node = base({
      type: "TEXT",
      characters: "Save",
      fontName: { family: "Inter", style: "Regular" },
      fills: [],
      textAlignHorizontal: "CENTER",
      textAlignVertical: "BOTTOM",
      textAutoResize: "WIDTH_AND_HEIGHT",
      getStyledTextSegments: () => [
        {
          start: 0,
          end: 4,
          fontName: { family: "Inter", style: "Regular" },
          fills: [],
        },
      ],
    })

    const text = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { text?: Record<string, unknown> }
    ).text

    expect(text).toMatchObject({
      alignHorizontal: "center",
      alignVertical: "bottom",
      autoResize: "widthAndHeight",
    })
    const range = (
      text?.styledRanges as { style: Record<string, unknown> }[]
    )[0]
    expect(Object.hasOwn(range?.style ?? {}, "alignHorizontal")).toBe(false)
    expect(Object.hasOwn(range?.style ?? {}, "alignVertical")).toBe(false)
    expect(Object.hasOwn(range?.style ?? {}, "autoResize")).toBe(false)
  })

  test("default text alignment and auto-resize are omitted", () => {
    const node = base({
      type: "TEXT",
      characters: "Save",
      fontName: { family: "Inter", style: "Regular" },
      fills: [],
      textAlignHorizontal: "LEFT",
      textAlignVertical: "TOP",
      textAutoResize: "NONE",
    })

    const text = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { text?: Record<string, unknown> }
    ).text

    expect(Object.hasOwn(text ?? {}, "alignHorizontal")).toBe(false)
    expect(Object.hasOwn(text ?? {}, "alignVertical")).toBe(false)
    expect(Object.hasOwn(text ?? {}, "autoResize")).toBe(false)
  })

  test("text additions survive the wire validator", () => {
    const node = base({
      type: "TEXT",
      characters: "Save",
      fontName: { family: "Inter", style: "Light" },
      fontWeight: 300,
      textDecoration: "UNDERLINE",
      fills: [],
      textAlignHorizontal: "JUSTIFIED",
      textAlignVertical: "CENTER",
      textAutoResize: "HEIGHT",
    })

    const serialized = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    })

    expect(
      parseReadResult({
        operation: "get_nodes",
        result: {
          detail: "full",
          items: [{ status: "success", value: serialized.nodes[0] }],
          truncated: false,
          observation: {
            startedAt: "2026-08-19T00:00:00.000Z",
            completedAt: "2026-08-19T00:00:01.000Z",
          },
        },
      }),
    ).toBeDefined()
  })

  test("compact auto-layout carries alignment, wrap, and counter-axis spacing", () => {
    const node = base({
      layoutMode: "HORIZONTAL",
      primaryAxisSizingMode: "AUTO",
      counterAxisSizingMode: "FIXED",
      itemSpacing: 8,
      paddingTop: 4,
      paddingRight: 12,
      paddingBottom: 4,
      paddingLeft: 12,
      primaryAxisAlignItems: "SPACE_BETWEEN",
      counterAxisAlignItems: "BASELINE",
      layoutWrap: "WRAP",
      counterAxisSpacing: 6,
    })

    const result = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    })

    expect(result.nodes[0]?.data).toMatchObject({
      autoLayout: {
        mode: "horizontal",
        primarySizing: "hug",
        counterSizing: "fixed",
        gap: 8,
        paddingTop: 4,
        paddingRight: 12,
        paddingBottom: 4,
        paddingLeft: 12,
        primaryAlign: "spaceBetween",
        counterAlign: "baseline",
        wrap: true,
        counterAxisSpacing: 6,
      },
    })
  })

  test("auto-layout emits min alignment but omits wrap fields when not wrapping", () => {
    const node = base({
      layoutMode: "VERTICAL",
      primaryAxisAlignItems: "MIN",
      counterAxisAlignItems: "MIN",
      layoutWrap: "NO_WRAP",
      counterAxisSpacing: 6,
    })

    const layout = (
      serializeNodeForest([node], {
        detail: "compact",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { autoLayout?: Record<string, unknown> }
    ).autoLayout

    expect(layout?.primaryAlign).toBe("min")
    expect(layout?.counterAlign).toBe("min")
    expect(Object.hasOwn(layout ?? {}, "wrap")).toBe(false)
    expect(Object.hasOwn(layout ?? {}, "counterAxisSpacing")).toBe(false)
  })

  test("layout constraints are emitted at compact and full", () => {
    const node = base({
      constraints: { horizontal: "STRETCH", vertical: "SCALE" },
    })

    for (const detail of ["compact", "full"] as const) {
      const result = serializeNodeForest([node], {
        detail,
        depth: 0,
        dedupeComponents: false,
      })
      expect(result.nodes[0]?.data).toMatchObject({
        constraints: { horizontal: "stretch", vertical: "scale" },
      })
    }
  })

  test("unknown constraint axes drop the whole constraints field", () => {
    const node = base({
      constraints: { horizontal: "STRETCH", vertical: "WOBBLE" },
    })

    const data = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as Record<string, unknown>

    expect(Object.hasOwn(data, "constraints")).toBe(false)
  })

  test("MIN/MIN constraints are omitted at compact and full, matching the Figma default", () => {
    const node = base({
      constraints: { horizontal: "MIN", vertical: "MIN" },
    })

    for (const detail of ["compact", "full"] as const) {
      const data = serializeNodeForest([node], {
        detail,
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as Record<string, unknown>

      expect(Object.hasOwn(data, "constraints")).toBe(false)
    }
  })

  test("throwing layout getters leave the node serializable", () => {
    const node = base({ layoutMode: "HORIZONTAL" })
    Object.defineProperty(node, "primaryAxisAlignItems", {
      get() {
        throw new Error("write-only under dynamic-page")
      },
      enumerable: true,
    })
    Object.defineProperty(node, "constraints", {
      get() {
        throw new Error("write-only under dynamic-page")
      },
      enumerable: true,
    })

    const data = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as { autoLayout?: Record<string, unknown> }

    expect(data.autoLayout?.mode).toBe("horizontal")
    expect(Object.hasOwn(data.autoLayout ?? {}, "primaryAlign")).toBe(false)
    expect(Object.hasOwn(data, "constraints")).toBe(false)
  })

  test("layout and constraint additions survive the wire validator", () => {
    const node = base({
      layoutMode: "HORIZONTAL",
      primaryAxisAlignItems: "CENTER",
      counterAxisAlignItems: "MAX",
      layoutWrap: "WRAP",
      counterAxisSpacing: 2,
      constraints: { horizontal: "MIN", vertical: "CENTER" },
    })

    const serialized = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    })

    const parsed = parseReadResult({
      operation: "get_nodes",
      result: {
        detail: "compact",
        items: [{ status: "success", value: serialized.nodes[0] }],
        truncated: false,
        observation: {
          startedAt: "2026-08-19T00:00:00.000Z",
          completedAt: "2026-08-19T00:00:01.000Z",
        },
      },
    })

    expect(parsed).toBeDefined()
  })

  test("full detail returns stroke paints, weight, align, and dash pattern", () => {
    const node = base({
      strokes: [
        { type: "SOLID", color: { r: 0, g: 0, b: 0, a: 1 }, opacity: 1 },
      ],
      strokeWeight: 2,
      strokeAlign: "INSIDE",
      dashPattern: [4, 2],
    })

    const result = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    })

    expect(result.nodes[0]?.data).toMatchObject({
      strokes: {
        paints: [
          { type: "solid", color: { r: 0, g: 0, b: 0, a: 1 }, opacity: 1 },
        ],
        weight: 2,
        align: "inside",
        dashPattern: [4, 2],
      },
    })
  })

  test("mixed stroke weight keeps the rest of the stroke", () => {
    const node = base({
      strokes: [
        { type: "SOLID", color: { r: 1, g: 0, b: 0, a: 1 }, opacity: 1 },
      ],
      strokeWeight: Symbol("figma.mixed"),
      strokeAlign: "OUTSIDE",
      dashPattern: [],
    })

    const strokes = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { strokes?: Record<string, unknown> }
    ).strokes

    expect(strokes?.align).toBe("outside")
    expect(Object.hasOwn(strokes ?? {}, "weight")).toBe(false)
    expect(Object.hasOwn(strokes ?? {}, "dashPattern")).toBe(false)
  })

  test("nodes without strokes omit the field entirely", () => {
    const node = base({ strokes: [], strokeWeight: 1, strokeAlign: "CENTER" })

    const data = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as Record<string, unknown>

    expect(Object.hasOwn(data, "strokes")).toBe(false)
  })

  test("an unmodelled stroke paint type still reports weight, align, and an empty paints array", () => {
    const node = base({
      strokes: [{ type: "GRADIENT_ANGULAR", gradientStops: [] }],
      strokeWeight: 3,
      strokeAlign: "OUTSIDE",
    })

    const strokes = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as { strokes?: Record<string, unknown> }
    ).strokes

    expect(strokes).toMatchObject({ paints: [], weight: 3, align: "outside" })
  })

  test("throwing stroke getters leave the node serializable", () => {
    const node = base({})
    Object.defineProperty(node, "strokes", {
      get() {
        throw new Error("write-only under dynamic-page")
      },
      enumerable: true,
    })

    const data = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as Record<string, unknown>

    expect(Object.hasOwn(data, "strokes")).toBe(false)
    expect(data.paints).toEqual([])
  })

  test("strokes survive the wire validator", () => {
    const node = base({
      strokes: [
        { type: "SOLID", color: { r: 0, g: 0, b: 0, a: 1 }, opacity: 1 },
      ],
      strokeWeight: 1.5,
      strokeAlign: "CENTER",
      dashPattern: [1],
    })

    const serialized = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    })

    expect(
      parseReadResult({
        operation: "get_nodes",
        result: {
          detail: "full",
          items: [{ status: "success", value: serialized.nodes[0] }],
          truncated: false,
          observation: {
            startedAt: "2026-08-19T00:00:00.000Z",
            completedAt: "2026-08-19T00:00:01.000Z",
          },
        },
      }),
    ).toBeDefined()
  })

  test("uniform corner radius is reported as a uniform value", () => {
    const node = base({ cornerRadius: 8 })

    expect(
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data,
    ).toMatchObject({ cornerRadius: { kind: "uniform", radius: 8 } })
  })

  test("mixed corner radius becomes the four per-corner values", () => {
    const node = base({
      cornerRadius: Symbol("figma.mixed"),
      topLeftRadius: 8,
      topRightRadius: 0,
      bottomRightRadius: 4,
      bottomLeftRadius: 0,
    })

    expect(
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data,
    ).toMatchObject({
      cornerRadius: {
        kind: "perCorner",
        topLeft: 8,
        topRight: 0,
        bottomRight: 4,
        bottomLeft: 0,
      },
    })
  })

  test("zero radius and zero smoothing are omitted", () => {
    const node = base({ cornerRadius: 0, cornerSmoothing: 0 })

    const data = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as Record<string, unknown>

    expect(Object.hasOwn(data, "cornerRadius")).toBe(false)
    expect(Object.hasOwn(data, "cornerSmoothing")).toBe(false)
  })

  test("corner smoothing is returned when the squircle is on", () => {
    const node = base({ cornerRadius: 12, cornerSmoothing: 0.6 })

    expect(
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data,
    ).toMatchObject({ cornerSmoothing: 0.6 })
  })

  test("throwing corner getters leave the node serializable", () => {
    const node = base({})
    Object.defineProperty(node, "cornerRadius", {
      get() {
        throw new Error("write-only under dynamic-page")
      },
      enumerable: true,
    })

    const data = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as Record<string, unknown>

    expect(Object.hasOwn(data, "cornerRadius")).toBe(false)
  })

  test("corner radius survives the wire validator", () => {
    const node = base({
      cornerRadius: Symbol("figma.mixed"),
      topLeftRadius: 8,
      topRightRadius: 8,
      bottomRightRadius: 0,
      bottomLeftRadius: 0,
      cornerSmoothing: 0.6,
    })

    const serialized = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    })

    expect(
      parseReadResult({
        operation: "get_nodes",
        result: {
          detail: "full",
          items: [{ status: "success", value: serialized.nodes[0] }],
          truncated: false,
          observation: {
            startedAt: "2026-08-19T00:00:00.000Z",
            completedAt: "2026-08-19T00:00:01.000Z",
          },
        },
      }),
    ).toBeDefined()
  })

  test("clipsContent and non-default blend mode are returned", () => {
    const node = base({ clipsContent: true, blendMode: "MULTIPLY" })

    expect(
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data,
    ).toMatchObject({ clipsContent: true, blendMode: "multiply" })
  })

  test("default blend modes and clipsContent false are omitted", () => {
    for (const blendMode of ["NORMAL", "PASS_THROUGH"]) {
      const node = base({ clipsContent: false, blendMode })

      const data = serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      }).nodes[0]?.data as Record<string, unknown>

      expect(Object.hasOwn(data, "clipsContent")).toBe(false)
      expect(Object.hasOwn(data, "blendMode")).toBe(false)
    }
  })

  test("unknown blend modes are dropped rather than passed through", () => {
    const node = base({ blendMode: "PLUS_LIGHTER" })

    const data = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as Record<string, unknown>

    expect(Object.hasOwn(data, "blendMode")).toBe(false)
  })

  test("blend mode survives the wire validator", () => {
    const node = base({ clipsContent: true, blendMode: "SOFT_LIGHT" })

    const serialized = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    })

    expect(
      parseReadResult({
        operation: "get_nodes",
        result: {
          detail: "full",
          items: [{ status: "success", value: serialized.nodes[0] }],
          truncated: false,
          observation: {
            startedAt: "2026-08-19T00:00:00.000Z",
            completedAt: "2026-08-19T00:00:01.000Z",
          },
        },
      }),
    ).toBeDefined()
  })

  test("terminates repeated-node cycles and enforces node and byte budgets", () => {
    const cyclic = base({ id: "C:1", type: "COMPONENT" })
    cyclic.children = [cyclic]
    const cycle = serializeNodeForest([cyclic], {
      detail: "compact",
      depth: 6,
      dedupeComponents: true,
    })
    expect(cycle.nodes[0]?.children[0]?.children).toEqual([])
    expect(cycle.nodes[0]?.children[0]?.childrenTruncated).toBe(true)

    const limited = serializeNodeForest(
      [base({ id: "1:1" }), base({ id: "1:2" })],
      {
        detail: "minimal",
        depth: 0,
        dedupeComponents: false,
        limits: { returnedNodes: 1, visitedNodes: 1, encodedBytes: 256 },
      },
    )
    expect(limited.nodes).toHaveLength(1)
    expect(limited.truncation?.reason).toBe("nodeLimit")
  })

  test("dedupes a later component occurrence as an identity stub, not a second definition", () => {
    const label = base({ id: "C:2", name: "Label", type: "TEXT" })
    const definition = base({
      id: "C:1",
      name: "Button",
      type: "COMPONENT",
      layoutMode: "HORIZONTAL",
      children: [label],
    })
    const sibling = base({
      id: "C:1",
      name: "Button",
      type: "COMPONENT",
      layoutMode: "HORIZONTAL",
      children: [label],
    })
    const root = base({
      id: "1:1",
      name: "Row",
      children: [definition, sibling],
    })

    const result = serializeNodeForest([root], {
      detail: "compact",
      depth: 6,
      dedupeComponents: true,
    })
    const first = result.nodes[0]?.children[0]
    const second = result.nodes[0]?.children[1]

    expect(first?.summary.id).toBe("C:1")
    expect(first?.children).toHaveLength(1)
    expect(first?.childrenTruncated).toBe(false)
    expect(first?.data).toMatchObject({ autoLayout: { mode: "horizontal" } })

    expect(second?.summary).toEqual({
      id: "C:1",
      name: "Button",
      nodeType: "COMPONENT",
      visible: true,
    })
    expect(second?.data).toEqual({
      styleReferences: [],
      variableReferences: [],
      component: { componentId: "C:1", properties: [] },
    })
    expect(second?.children).toEqual([])
    expect(second?.childrenTruncated).toBe(true)
  })

  test("omits hidden children only when includeHidden is false", () => {
    const hidden = base({ id: "1:2", name: "Hidden", visible: false })
    const shown = base({ id: "1:3", name: "Shown", visible: true })
    const root = base({ children: [hidden, shown] })

    const filtered = serializeNodeForest([root], {
      detail: "minimal",
      depth: 2,
      dedupeComponents: false,
      includeHidden: false,
    })
    expect(filtered.nodes[0]?.summary.childIds).toEqual(["1:3"])
    expect(
      filtered.nodes[0]?.children.map((child) => child.summary.id),
    ).toEqual(["1:3"])

    const included = serializeNodeForest([root], {
      detail: "minimal",
      depth: 2,
      dedupeComponents: false,
      includeHidden: true,
    })
    expect(
      included.nodes[0]?.children.map((child) => child.summary.id),
    ).toEqual(["1:2", "1:3"])

    const unspecified = serializeNodeForest([root], {
      detail: "minimal",
      depth: 2,
      dedupeComponents: false,
    })
    expect(
      unspecified.nodes[0]?.children.map((child) => child.summary.id),
    ).toEqual(["1:2", "1:3"])
  })

  test("checks cancellation between child batches", () => {
    const cancellation = new LocalCancellationController()
    cancellation.abort()
    expect(() =>
      serializeNodeForest([base()], {
        detail: "minimal",
        depth: 0,
        dedupeComponents: false,
        signal: cancellation.signal,
      }),
    ).toThrow("Operation cancelled")
  })

  test("walk and serialize loops emit bounded phase counts", () => {
    const child = base({ id: "1:2", name: "Child", type: "RECTANGLE" })
    const root = base({ children: [child] })
    const walkFrames: ProgressFrame[] = []
    const serializeFrames: ProgressFrame[] = []

    walkNodeForest(
      [root],
      {
        progress: createProgressReporter({
          emit: (frame) => walkFrames.push(frame),
          intervalMs: 0,
        }),
      },
      () => undefined,
    )
    serializeNodeForest([root], {
      detail: "minimal",
      depth: 2,
      dedupeComponents: false,
      progress: createProgressReporter({
        emit: (frame) => serializeFrames.push(frame),
        intervalMs: 0,
      }),
    })

    expect(walkFrames.length).toBeGreaterThan(0)
    expect(walkFrames.every((frame) => frame.message === "reading")).toBe(true)
    expect(walkFrames.at(-1)?.completed).toBe(2)
    expect(serializeFrames.length).toBeGreaterThan(0)
    expect(
      serializeFrames.every((frame) => frame.message === "serializing"),
    ).toBe(true)
    expect(serializeFrames.at(-1)?.completed).toBe(2)
  })

  test("a throwing getter on any content property leaves the node serializable", () => {
    const sites = [
      "absoluteTransform",
      "absoluteBoundingBox",
      "rotation",
      "opacity",
      "boundVariables",
      "effects",
    ] as const

    for (const site of sites) {
      const node = base({})
      Object.defineProperty(node, site, {
        get() {
          throw new Error("write-only under dynamic-page")
        },
        enumerable: true,
      })

      const result = serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      })

      expect(result.nodes[0]?.summary.id).toBe("1:1")
      expect(result.nodes[0]?.data).toBeDefined()
      expect(result.truncated).toBe(false)
    }
  })

  test("a throwing characters getter leaves a compact TEXT node serializable", () => {
    const node = base({ type: "TEXT" })
    Object.defineProperty(node, "characters", {
      get() {
        throw new Error("write-only under dynamic-page")
      },
      enumerable: true,
    })

    const data = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as { text?: { characterCount: number; preview: string } }

    expect(data.text).toEqual({ characterCount: 0, preview: "" })
  })

  test("a throwing getStyledTextSegments leaves a full TEXT node serializable", () => {
    const node = base({
      type: "TEXT",
      characters: "Save",
      fontName: { family: "Inter", style: "Regular" },
      fills: [],
    })
    Object.defineProperty(node, "getStyledTextSegments", {
      get() {
        throw new Error("write-only under dynamic-page")
      },
      enumerable: true,
    })

    const data = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as {
      text?: { characters: string; styledRanges: unknown[] }
    }

    expect(data.text?.characters).toBe("Save")
    expect(data.text?.styledRanges).toEqual([])
  })
})

describe("instance component properties", () => {
  test("keeps suffixed names, sorts by name, and drops unsupported kinds", () => {
    const instance = base({
      id: "4:1",
      type: "INSTANCE",
      componentProperties: {
        "ButtonText#0:1": { type: "TEXT", value: "Save" },
        Size: { type: "VARIANT", value: "Large" },
        "IconVisible#0:0": { type: "BOOLEAN", value: false },
        "IconSwap#0:2": { type: "INSTANCE_SWAP", value: "9:9" },
        "Slot#0:3": { type: "SLOT", value: "" },
      },
    })

    expect(namedComponentProperties(instance)).toEqual([
      { name: "ButtonText#0:1", value: { kind: "text", value: "Save" } },
      { name: "IconSwap#0:2", value: { kind: "instanceSwap", value: "9:9" } },
      { name: "IconVisible#0:0", value: { kind: "boolean", value: false } },
      { name: "Size", value: { kind: "variant", value: "Large" } },
    ])
  })

  test("survives a throwing host getter and clamps long string values", () => {
    const throwing = base({ id: "4:2", type: "INSTANCE" })
    Object.defineProperty(throwing, "componentProperties", {
      configurable: true,
      enumerable: true,
      get() {
        throw new Error("componentProperties is not readable")
      },
    })

    expect(namedComponentProperties(throwing)).toEqual([])

    const long = base({
      id: "4:3",
      type: "INSTANCE",
      componentProperties: {
        "Body#1:0": { type: "TEXT", value: "x".repeat(300) },
      },
    })

    expect(namedComponentProperties(long)).toEqual([
      { name: "Body#1:0", value: { kind: "text", value: "x".repeat(256) } },
    ])
  })

  test("ignores properties whose value type does not match the declared kind", () => {
    const mismatched = base({
      id: "4:4",
      type: "INSTANCE",
      componentProperties: {
        "Label#0:1": { type: "TEXT", value: 42 },
        "Toggle#0:2": { type: "BOOLEAN", value: "true" },
        Size: { type: "VARIANT", value: "Large" },
      },
    })

    expect(namedComponentProperties(mismatched)).toEqual([
      { name: "Size", value: { kind: "variant", value: "Large" } },
    ])
  })

  test("identity path carries properties without extra main component lookups", async () => {
    let lookups = 0
    const instance = base({
      id: "4:1",
      name: "Primary button",
      type: "INSTANCE",
      componentProperties: {
        Size: { type: "VARIANT", value: "Large" },
        "ButtonText#0:1": { type: "TEXT", value: "Save" },
      },
      getMainComponentAsync: async () => {
        lookups += 1
        return {
          id: "8055:10274",
          type: "COMPONENT",
          parent: { id: "8055:10286", type: "COMPONENT_SET" },
        }
      },
    })

    const identities = await collectInstanceIdentities([instance])
    const result = serializeNodeForest([instance], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
      instanceIdentities: identities,
    })

    expect(lookups).toBe(1)
    expect((result.nodes[0]?.data as { instance?: unknown }).instance).toEqual({
      componentId: "8055:10274",
      componentSetId: "8055:10286",
      properties: [
        { name: "ButtonText#0:1", value: { kind: "text", value: "Save" } },
        { name: "Size", value: { kind: "variant", value: "Large" } },
      ],
    })
  })

  test("fallback path fills properties while components stay empty", () => {
    const instance = base({
      id: "4:4",
      type: "INSTANCE",
      componentId: "2:1",
      componentProperties: {
        "Label#0:1": { type: "TEXT", value: "Buy" },
      },
    })
    const component = base({
      id: "2:1",
      type: "COMPONENT",
      componentProperties: {
        "Label#0:1": { type: "TEXT", value: "Buy" },
      },
    })

    const result = serializeNodeForest([instance, component], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    })

    expect((result.nodes[0]?.data as { instance?: unknown }).instance).toEqual({
      componentId: "2:1",
      properties: [
        { name: "Label#0:1", value: { kind: "text", value: "Buy" } },
      ],
    })
    expect(
      (result.nodes[1]?.data as { component?: unknown }).component,
    ).toEqual({
      componentId: "2:1",
      properties: [],
    })
  })

  test("dedupeComponents keeps per-instance property values", async () => {
    const main = { id: "8055:10274", type: "COMPONENT", parent: { id: "0:1" } }
    const first = base({
      id: "4:5",
      type: "INSTANCE",
      componentProperties: { "Label#0:1": { type: "TEXT", value: "Save" } },
      getMainComponentAsync: async () => main,
    })
    const second = base({
      id: "4:6",
      type: "INSTANCE",
      componentProperties: { "Label#0:1": { type: "TEXT", value: "Cancel" } },
      getMainComponentAsync: async () => main,
    })
    const root = base({ id: "1:1", children: [first, second] })

    const identities = await collectInstanceIdentities([root])
    const result = serializeNodeForest([root], {
      detail: "compact",
      depth: 2,
      dedupeComponents: true,
      instanceIdentities: identities,
    })

    const children = result.nodes[0]?.children ?? []
    expect(
      (children[0]?.data as { instance?: { properties: unknown } }).instance
        ?.properties,
    ).toEqual([{ name: "Label#0:1", value: { kind: "text", value: "Save" } }])
    expect(
      (children[1]?.data as { instance?: { properties: unknown } }).instance
        ?.properties,
    ).toEqual([{ name: "Label#0:1", value: { kind: "text", value: "Cancel" } }])
  })
})

describe("style name resolution", () => {
  const styled = (id: string, overrides: Record<string, unknown> = {}) =>
    base({ id, fillStyleId: "S:fill", strokeStyleId: "S:stroke", ...overrides })

  test("resolves one lookup per unique style id, not per node", async () => {
    const lookups: string[] = []
    const lookup = async (id: string) => {
      lookups.push(id)
      return { name: `Name/${id}` }
    }
    const roots = [
      styled("1:1", {
        children: [styled("1:2"), styled("1:3", { fillStyleId: "S:other" })],
      }),
    ]

    const names = await collectStyleNames(roots, lookup)

    expect(lookups.sort()).toEqual(["S:fill", "S:other", "S:stroke"])
    expect(names.get("S:fill")).toBe("Name/S:fill")
  })

  test("style references carry resolved names and the stroke kind", () => {
    const node = styled("1:1")

    const result = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
      styleNames: new Map([
        ["S:fill", "Primary/500"],
        ["S:stroke", "Border/Default"],
      ]),
    })

    expect(result.nodes[0]?.data).toMatchObject({
      styleReferences: [
        { id: "S:fill", kind: "paint", name: "Primary/500" },
        { id: "S:stroke", kind: "stroke", name: "Border/Default" },
      ],
    })
  })

  test("unresolved style ids omit name rather than emitting an empty string", () => {
    const node = styled("1:1")

    const refs = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
        styleNames: new Map([["S:fill", ""]]),
      }).nodes[0]?.data as { styleReferences: Record<string, unknown>[] }
    ).styleReferences

    expect(Object.hasOwn(refs[0] ?? {}, "name")).toBe(false)
    expect(Object.hasOwn(refs[1] ?? {}, "name")).toBe(false)
  })

  test("a failing lookup drops that name and leaves the rest resolved", async () => {
    const lookup = async (id: string) => {
      if (id === "S:fill") throw new Error("remote style unavailable")
      return { name: "Border/Default" }
    }

    const names = await collectStyleNames([styled("1:1")], lookup)

    expect(names.has("S:fill")).toBe(false)
    expect(names.get("S:stroke")).toBe("Border/Default")
  })

  test("an exhausted budget leaves remaining names absent without truncating", async () => {
    const lookup = async (id: string) => ({ name: `Name/${id}` })

    const names = await collectStyleNames(
      [styled("1:1")],
      lookup,
      undefined,
      Number.POSITIVE_INFINITY,
      0,
    )

    expect(names.size).toBe(0)

    const result = serializeNodeForest([styled("1:1")], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
      styleNames: names,
    })
    expect(result.truncated).toBe(false)
  })

  test("style names survive the wire validator", () => {
    const serialized = serializeNodeForest([styled("1:1")], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
      styleNames: new Map([["S:stroke", "Border/Default"]]),
    })

    expect(
      parseReadResult({
        operation: "get_nodes",
        result: {
          detail: "full",
          items: [{ status: "success", value: serialized.nodes[0] }],
          truncated: false,
          observation: {
            startedAt: "2026-08-19T00:00:00.000Z",
            completedAt: "2026-08-19T00:00:01.000Z",
          },
        },
      }),
    ).toBeDefined()
  })
})

describe("variable name resolution", () => {
  const bound = (id: string) => ({
    boundVariables: {
      fills: [{ type: "VARIABLE_ALIAS", id }],
    },
  })

  test("resolves one lookup per unique variable id, not per node or per reference", async () => {
    const lookups: string[] = []
    const lookup = async (id: string) => {
      lookups.push(id)
      return { name: `token/${id}` }
    }
    // Three nodes, four references, two unique ids.
    const leaf = base({
      id: "1:3",
      boundVariables: {
        fills: [{ type: "VARIABLE_ALIAS", id: "V:a" }],
        strokes: [{ type: "VARIABLE_ALIAS", id: "V:b" }],
      },
    })
    const child = base({ id: "1:2", ...bound("V:a"), children: [leaf] })
    const root = base({ id: "1:1", ...bound("V:b"), children: [child] })

    const names = await collectVariableNames([root], lookup)

    expect(lookups.sort()).toEqual(["V:a", "V:b"])
    expect(names.get("V:a")).toBe("token/V:a")
    expect(names.get("V:b")).toBe("token/V:b")
  })

  test("variable references carry the resolved name", () => {
    const node = base({ id: "1:1", ...bound("V:a") })

    const result = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
      variableNames: new Map([["V:a", "text/primary"]]),
    })

    expect(result.nodes[0]?.data).toMatchObject({
      variableReferences: [{ id: "V:a", name: "text/primary" }],
    })
  })

  test("an unresolved id omits name rather than emitting an empty string", () => {
    const node = base({ id: "1:1", ...bound("V:a") })

    const refs = (
      serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
        variableNames: new Map([["V:a", ""]]),
      }).nodes[0]?.data as { variableReferences: Record<string, unknown>[] }
    ).variableReferences

    expect(refs[0]).toEqual({ id: "V:a" })
    expect(Object.hasOwn(refs[0] ?? {}, "name")).toBe(false)
  })

  test("a failing lookup drops that name and leaves the rest resolved", async () => {
    const lookup = async (id: string) => {
      if (id === "V:a") throw new Error("remote variable unavailable")
      return { name: "spacing/md" }
    }
    const node = base({
      id: "1:1",
      boundVariables: {
        fills: [{ type: "VARIABLE_ALIAS", id: "V:a" }],
        strokes: [{ type: "VARIABLE_ALIAS", id: "V:b" }],
      },
    })

    const names = await collectVariableNames([node], lookup)

    expect(names.has("V:a")).toBe(false)
    expect(names.get("V:b")).toBe("spacing/md")
  })

  test("an exhausted budget leaves names absent without truncating the forest", async () => {
    const lookup = async (id: string) => ({ name: `token/${id}` })
    const node = base({ id: "1:1", ...bound("V:a") })

    const names = await collectVariableNames(
      [node],
      lookup,
      undefined,
      Number.POSITIVE_INFINITY,
      0,
    )

    expect(names.size).toBe(0)

    const result = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
      variableNames: names,
    })
    expect(result.truncated).toBe(false)
  })

  test("a missing lookup yields an empty map rather than throwing", async () => {
    const names = await collectVariableNames(
      [base({ ...bound("V:a") })],
      undefined,
    )
    expect(names.size).toBe(0)
  })

  test("nested alias objects are all collected", async () => {
    const lookups: string[] = []
    const lookup = async (id: string) => {
      lookups.push(id)
      return { name: id }
    }
    const node = base({
      id: "1:1",
      boundVariables: {
        fills: [{ type: "VARIABLE_ALIAS", id: "V:fill" }],
        componentProperties: {
          "Label#1:1": { type: "VARIABLE_ALIAS", id: "V:label" },
        },
        itemSpacing: { type: "VARIABLE_ALIAS", id: "V:gap" },
      },
    })

    await collectVariableNames([node], lookup)

    expect(lookups.sort()).toEqual(["V:fill", "V:gap", "V:label"])
  })

  test("resolved names survive the wire validator", () => {
    const serialized = serializeNodeForest(
      [base({ id: "1:1", ...bound("V:a") })],
      {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
        variableNames: new Map([["V:a", "text/primary"]]),
      },
    )

    expect(
      parseReadResult({
        operation: "get_nodes",
        result: {
          detail: "full",
          items: [{ status: "success", value: serialized.nodes[0] }],
          truncated: false,
          observation: {
            startedAt: "2026-08-19T00:00:00.000Z",
            completedAt: "2026-08-19T00:00:01.000Z",
          },
        },
      }),
    ).toBeDefined()
  })
})

function endsWithLoneHighSurrogate(value: string): boolean {
  const code = value.charCodeAt(value.length - 1)
  return code >= 0xd800 && code <= 0xdbff
}

describe("text clamp helper", () => {
  test("leaves strings shorter than or equal to the limit untouched", () => {
    const at255 = "a".repeat(255)
    const at256 = "a".repeat(256)
    expect(clampText(at255)).toBe(at255)
    expect(clampText(at255)).toHaveLength(255)
    expect(clampText(at256)).toBe(at256)
    expect(clampText(at256)).toHaveLength(256)
  })

  test("truncates a string one over the limit down to the limit", () => {
    const at257 = "a".repeat(257)
    const clamped = clampText(at257)
    expect(clamped).toHaveLength(TEXT_CLAMP_LIMIT)
    expect(clamped).toBe("a".repeat(256))
  })

  test("drops a trailing high surrogate left dangling by a mid-pair slice", () => {
    // A surrogate pair ("😀", one emoji) straddles the 256/257 boundary:
    // 255 filler units (indices 0-254) + the pair at indices 255-256.
    const filler = "a".repeat(255)
    const pair = "😀"
    const value = filler + pair + "tail"

    const clamped = clampText(value)

    expect(endsWithLoneHighSurrogate(clamped)).toBe(false)
    expect(clamped).toBe(filler)
    expect(clamped).toHaveLength(255)
    // JSON.stringify/parse must round-trip without corrupting the string.
    expect(JSON.parse(JSON.stringify(clamped))).toBe(clamped)
  })

  test("keeps a surrogate pair that lands entirely within the limit", () => {
    const filler = "a".repeat(254)
    const pair = "😀"
    const value = filler + pair

    const clamped = clampText(value)

    expect(clamped).toBe(value)
    expect(endsWithLoneHighSurrogate(clamped)).toBe(false)
  })

  test("returns a within-limit string byte-identical even with a pre-existing lone high surrogate", () => {
    // No truncation occurs here: the input is already shorter than the limit, so a
    // pre-existing (already broken) lone high surrogate must not be touched.
    const value = "\ud800"
    expect(clampText(value)).toBe(value)
    expect(clampText(value)).toHaveLength(1)

    const longer = "a".repeat(254) + "\ud800"
    expect(clampText(longer)).toBe(longer)
    expect(clampText(longer)).toHaveLength(255)
  })

  test("returns a string exactly at the limit byte-identical even with a pre-existing lone high surrogate", () => {
    // No truncation occurs here either: the input is exactly at the limit.
    const value = "a".repeat(255) + "\ud800"
    expect(value).toHaveLength(TEXT_CLAMP_LIMIT)

    expect(clampText(value)).toBe(value)
    expect(clampText(value)).toHaveLength(TEXT_CLAMP_LIMIT)
  })

  test("namedComponentProperties clamps TEXT values without emitting a lone surrogate", () => {
    const filler = "a".repeat(255)
    const pair = "😀"
    const instance = base({
      id: "4:9",
      type: "INSTANCE",
      componentProperties: {
        "Body#1:0": { type: "TEXT", value: filler + pair + "tail" },
      },
    })

    const properties = namedComponentProperties(instance)
    const value = properties[0]?.value

    expect(value).toEqual({ kind: "text", value: filler })
    if (value?.kind === "text") {
      expect(endsWithLoneHighSurrogate(value.value)).toBe(false)
      expect(JSON.parse(JSON.stringify(value))).toEqual(value)
    }
  })

  test("text.preview clamps without emitting a lone surrogate", () => {
    const filler = "a".repeat(255)
    const pair = "😀"
    const node = base({
      id: "1:9",
      type: "TEXT",
      characters: filler + pair + "tail",
    })

    const result = serializeNodeForest([node], {
      detail: "compact",
      depth: 0,
      dedupeComponents: false,
    })

    const text = (result.nodes[0]?.data as { text?: { preview: string } }).text
    expect(text?.preview).toBe(filler)
    expect(endsWithLoneHighSurrogate(text?.preview ?? "")).toBe(false)
    expect(JSON.parse(JSON.stringify(result.nodes[0]))).toBeTruthy()
  })

  describe("text style units", () => {
    const textNode = (overrides: Record<string, unknown> = {}) =>
      base({
        id: "5:1",
        name: "Label",
        type: "TEXT",
        characters: "Today",
        fontName: { family: "Inter", style: "Medium" },
        fontSize: 14,
        lineHeight: { unit: "PIXELS", value: 20 },
        letterSpacing: { unit: "PIXELS", value: 0.5 },
        fills: [],
        ...overrides,
      })

    const defaultStyle = (node: Record<string, unknown>) => {
      const result = serializeNodeForest([node], {
        detail: "full",
        depth: 0,
        dedupeComponents: false,
      })
      return (result.nodes[0]?.data as { text?: { defaultStyle: unknown } })
        .text?.defaultStyle
    }

    test("keeps PIXELS and PERCENT units instead of a bare number", () => {
      expect(defaultStyle(textNode())).toEqual({
        fontFamily: "Inter",
        fontStyle: "Medium",
        fontSize: 14,
        lineHeight: { unit: "pixels", value: 20 },
        letterSpacing: { unit: "pixels", value: 0.5 },
        paints: [],
      })

      expect(
        defaultStyle(
          textNode({
            lineHeight: { unit: "PERCENT", value: 150 },
            letterSpacing: { unit: "PERCENT", value: 2 },
          }),
        ),
      ).toMatchObject({
        lineHeight: { unit: "percent", value: 150 },
        letterSpacing: { unit: "percent", value: 2 },
      })
    })

    test("AUTO line height is reported as auto, never as zero", () => {
      const style = defaultStyle(
        textNode({ lineHeight: { unit: "AUTO" } }),
      ) as {
        lineHeight: unknown
      }
      expect(style.lineHeight).toEqual({ unit: "auto" })
    })

    test("mixed values drop the field instead of collapsing to zero", () => {
      const mixed = Symbol("figma.mixed")
      const style = defaultStyle(
        textNode({ fontSize: mixed, lineHeight: mixed, letterSpacing: mixed }),
      ) as Record<string, unknown>
      expect(style).toEqual({
        fontFamily: "Inter",
        fontStyle: "Medium",
        paints: [],
      })
    })

    test("an unknown unit drops the field rather than emitting a foreign tag", () => {
      const style = defaultStyle(
        textNode({ lineHeight: { unit: "EM", value: 2 } }),
      ) as Record<string, unknown>
      expect(style.lineHeight).toBeUndefined()
      expect(style.letterSpacing).toEqual({ unit: "pixels", value: 0.5 })
    })

    describe("round-trips through the wire validator", () => {
      // parseReadResult is a hard gate: if it rejects a shape that the
      // serializer emits, it throws and destroys the whole response, not
      // one field. Push a serialized TEXT node at detail "full" back
      // through the real validator (not the parser functions directly) so
      // a validator/serializer drift on TextStyle's unit fields is caught.
      const observation = {
        startedAt: "2024-01-01T00:00:00.000Z",
        completedAt: "2024-01-01T00:00:00.000Z",
      }

      const validatedDefaultStyle = (node: Record<string, unknown>) => {
        const forest = serializeNodeForest([node], {
          detail: "full",
          depth: 0,
          dedupeComponents: false,
        })
        const wireResult: Record<string, unknown> = {
          detail: "full",
          nodes: forest.nodes,
          truncated: forest.truncated,
          observation,
        }
        if (forest.truncation !== undefined) {
          wireResult.truncation = forest.truncation
        }

        const validated = parseReadResult({
          operation: "get_selection",
          result: wireResult,
        })
        expect(validated.operation).toBe("get_selection")
        const result = validated.result as {
          nodes: readonly { data: { text?: { defaultStyle: unknown } } }[]
        }
        return result.nodes[0]?.data.text?.defaultStyle
      }

      test("PIXELS line height is accepted with its unit shape intact", () => {
        const style = validatedDefaultStyle(textNode()) as Record<
          string,
          unknown
        >
        expect(style.lineHeight).toEqual({ unit: "pixels", value: 20 })
        expect(style.letterSpacing).toEqual({ unit: "pixels", value: 0.5 })
      })

      test("PERCENT line height is accepted with its unit shape intact", () => {
        const style = validatedDefaultStyle(
          textNode({ lineHeight: { unit: "PERCENT", value: 150 } }),
        ) as Record<string, unknown>
        expect(style.lineHeight).toEqual({ unit: "percent", value: 150 })
      })

      test("AUTO line height is accepted without a value field", () => {
        const style = validatedDefaultStyle(
          textNode({ lineHeight: { unit: "AUTO" } }),
        ) as Record<string, unknown>
        expect(style.lineHeight).toEqual({ unit: "auto" })
      })

      test("the all-omitted case (figma.mixed) is accepted", () => {
        const mixed = Symbol("figma.mixed")
        const style = validatedDefaultStyle(
          textNode({
            fontSize: mixed,
            lineHeight: mixed,
            letterSpacing: mixed,
          }),
        ) as Record<string, unknown>
        expect(style).toEqual({
          fontFamily: "Inter",
          fontStyle: "Medium",
          paints: [],
        })
      })
    })

    test.each([
      [
        "fontName",
        (style: Record<string, unknown>) => {
          expect(style.fontFamily).toBe("")
          expect(style.fontStyle).toBe("")
        },
      ],
      [
        "fontSize",
        (style: Record<string, unknown>) => {
          expect(style.fontSize).toBeUndefined()
          expect(style.fontFamily).toBe("Inter")
        },
      ],
      [
        "letterSpacing",
        (style: Record<string, unknown>) => {
          expect(style.letterSpacing).toBeUndefined()
          expect(style.fontFamily).toBe("Inter")
        },
      ],
      [
        "fills",
        (style: Record<string, unknown>) => {
          expect(style.paints).toEqual([])
          expect(style.fontFamily).toBe("Inter")
        },
      ],
      [
        "lineHeight",
        (style: Record<string, unknown>) => {
          expect(style.lineHeight).toBeUndefined()
          expect(style.fontFamily).toBe("Inter")
        },
      ],
    ] as const)(
      "a throwing %s getter does not break serialization",
      (key, assertStyle) => {
        const node = textNode()
        Object.defineProperty(node, key, {
          configurable: true,
          enumerable: true,
          get() {
            throw new Error(`${key} is not readable`)
          },
        })
        const style = defaultStyle(node) as Record<string, unknown>
        assertStyle(style)
      },
    )
  })
})

describe("hidden effects and paints are not reported", () => {
  test("drops an effect whose visible is false, keeping its live neighbour", () => {
    const result = effects([
      {
        type: "DROP_SHADOW",
        color: { r: 0, g: 0, b: 0, a: 0.5 },
        offset: { x: 1, y: 2 },
        radius: 4,
        spread: 1,
        visible: false,
      },
      {
        type: "DROP_SHADOW",
        color: { r: 0, g: 0, b: 0, a: 0.25 },
        offset: { x: 0, y: 8 },
        radius: 16,
        spread: 2,
        visible: true,
      },
    ])

    expect(result).toEqual([
      {
        type: "dropShadow",
        color: { r: 0, g: 0, b: 0, a: 0.25 },
        offsetX: 0,
        offsetY: 8,
        radius: 16,
        spread: 2,
      },
    ])
  })

  test("treats an absent visible as visible, matching the Figma default", () => {
    const result = effects([{ type: "LAYER_BLUR", radius: 4 }])
    expect(result).toEqual([{ type: "layerBlur", radius: 4 }])
  })

  test("drops a hidden fill and keeps a live one", () => {
    const result = paints([
      { type: "SOLID", color: { r: 1, g: 0, b: 0, a: 1 }, visible: false },
      { type: "SOLID", color: { r: 0, g: 1, b: 0, a: 1 }, visible: true },
    ])

    expect(result).toEqual([
      { type: "solid", color: { r: 0, g: 1, b: 0, a: 1 }, opacity: 1 },
    ])
  })

  test("drops a hidden fill from a text style", () => {
    const style = textStyle({
      fontName: { family: "Inter", style: "Regular" },
      fills: [
        { type: "SOLID", color: { r: 1, g: 0, b: 0, a: 1 }, visible: false },
      ],
    })
    expect(style.paints).toEqual([])
  })
})

describe("stroke reporting follows stroke visibility", () => {
  const nodeWithStrokes = (strokes: unknown[]) =>
    base({
      strokes,
      strokeWeight: 1,
      strokeAlign: "OUTSIDE",
    })

  const fullData = (node: Record<string, unknown>) =>
    serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    }).nodes[0]?.data as { strokes?: Record<string, unknown> } | undefined

  test("omits strokes entirely when every stroke is hidden", () => {
    const data = fullData(
      nodeWithStrokes([
        { type: "SOLID", color: { r: 0, g: 0, b: 0, a: 1 }, visible: false },
      ]),
    )
    expect(data?.strokes).toBeUndefined()
  })

  test("keeps weight and align when only some strokes are hidden", () => {
    const data = fullData(
      nodeWithStrokes([
        { type: "SOLID", color: { r: 1, g: 0, b: 0, a: 1 }, visible: false },
        { type: "SOLID", color: { r: 0, g: 0, b: 1, a: 1 }, visible: true },
      ]),
    )
    expect(data?.strokes).toEqual({
      paints: [
        { type: "solid", color: { r: 0, g: 0, b: 1, a: 1 }, opacity: 1 },
      ],
      weight: 1,
      align: "outside",
    })
  })

  test("still reports a visible stroke whose paint type cannot be modelled", () => {
    const data = fullData(
      nodeWithStrokes([{ type: "GRADIENT_ANGULAR", visible: true }]),
    )
    expect(data?.strokes).toMatchObject({ weight: 1, align: "outside" })
  })
})

describe("non-solid paints keep their opacity and direction", () => {
  const gradientTransform = [
    [0, 1, 0],
    [-1, 0, 1],
  ]

  test("a linear gradient reports opacity and gradientTransform", () => {
    expect(
      paints([
        {
          type: "GRADIENT_LINEAR",
          opacity: 0.4,
          gradientTransform,
          gradientStops: [
            { position: 0, color: { r: 1, g: 0, b: 0, a: 1 } },
            { position: 1, color: { r: 0, g: 0, b: 1, a: 1 } },
          ],
        },
      ]),
    ).toEqual([
      {
        type: "linearGradient",
        opacity: 0.4,
        gradientTransform: { m00: 0, m01: 1, m02: 0, m10: -1, m11: 0, m12: 1 },
        stops: [
          { position: 0, color: { r: 1, g: 0, b: 0, a: 1 } },
          { position: 1, color: { r: 0, g: 0, b: 1, a: 1 } },
        ],
      },
    ])
  })

  test("an image paint reports opacity", () => {
    expect(
      paints([
        { type: "IMAGE", imageHash: "img-1", scaleMode: "FIT", opacity: 0.25 },
      ]),
    ).toEqual([
      { type: "image", imageRef: "img-1", scaleMode: "fit", opacity: 0.25 },
    ])
  })

  test("an absent opacity defaults to 1, as it already does for solid", () => {
    const [gradient] = paints([
      { type: "GRADIENT_RADIAL", gradientTransform, gradientStops: [] },
    ])
    expect(gradient).toMatchObject({ type: "radialGradient", opacity: 1 })
  })

  test("an absent gradientTransform falls back to identity", () => {
    const [gradient] = paints([{ type: "GRADIENT_LINEAR", gradientStops: [] }])
    expect(gradient).toMatchObject({
      gradientTransform: { m00: 1, m01: 0, m02: 0, m10: 0, m11: 1, m12: 0 },
    })
  })
})

describe("new paint and effect shapes survive the wire validator", () => {
  const observation = {
    startedAt: "2024-01-01T00:00:00.000Z",
    completedAt: "2024-01-01T00:00:00.000Z",
  }

  // Exported for reuse by the later paint and effect shape tasks.
  const validatedFullData = (node: Record<string, unknown>) => {
    const forest = serializeNodeForest([node], {
      detail: "full",
      depth: 0,
      dedupeComponents: false,
    })
    const validated = parseReadResult({
      operation: "get_selection",
      result: {
        detail: "full",
        nodes: forest.nodes,
        truncated: forest.truncated,
        observation,
      },
    })
    const result = validated.result as unknown as {
      nodes: readonly { data: Record<string, unknown> }[]
    }
    return result.nodes[0]?.data
  }

  test("a gradient and image fill pass parseReadResult intact", () => {
    const data = validatedFullData(
      base({
        fills: [
          {
            type: "GRADIENT_LINEAR",
            opacity: 0.4,
            gradientTransform: [
              [0, 1, 0],
              [-1, 0, 1],
            ],
            gradientStops: [{ position: 0, color: { r: 1, g: 0, b: 0, a: 1 } }],
          },
          {
            type: "IMAGE",
            imageHash: "img-1",
            scaleMode: "FIT",
            opacity: 0.25,
          },
        ],
      }),
    )
    expect(data?.paints).toMatchObject([
      { type: "linearGradient", opacity: 0.4 },
      { type: "image", opacity: 0.25 },
    ])
  })
})
