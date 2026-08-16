import { describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { createProgressReporter, type ProgressFrame } from "../main/progress"
import {
  collectInstanceIdentities,
  serializeNodeForest,
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
})
