import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getMotion } from "./motion"

const page = (id: string, name: string, children: unknown[] = []) => ({
  id,
  name,
  type: "PAGE",
  visible: true,
  children,
  loadAsync: async () => {},
})

const motionNode = (
  id: string,
  extras: Record<string, unknown> = {},
): Record<string, unknown> => ({
  id,
  name: id,
  type: "FRAME",
  visible: true,
  children: [],
  leftover: "must-not-leak",
  animationStyles: [],
  animations: {},
  manualKeyframeTracks: {},
  timelines: [],
  ...extras,
})

// A node is only emitted when something actually animates on it, so fixtures
// that exist to prove some other point still need one real applied style.
const appliedStyle = (id: string) => ({
  id,
  styleId: `S:${id}`,
  name: "Fade",
})

function floatKeyframe(
  id: string,
  position: number,
  value: number,
  easing: unknown,
) {
  return {
    id,
    timelinePosition: position,
    value: { type: "FLOAT", value },
    easing,
  }
}

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
  motion?: unknown
}): {
  currentPage: Record<string, unknown>
  loadedPages: string[]
  catalogCalls: { count: number }
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
  const catalogCalls = { count: 0 }
  const motion =
    options.motion === undefined
      ? {
          figmaAnimationStyles: () => {
            catalogCalls.count += 1
            return [
              {
                styleId: "S:fade",
                name: "Fade in",
                description: "Catalog fade",
                leftover: true,
                props: {
                  direction: "string",
                  distance: "number",
                },
              },
            ]
          },
        }
      : options.motion
  const api: Record<string, unknown> = {
    root: { name: "Checkout flow", children: pages },
    currentPage: current,
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("motion must not load every page")
    },
    getNodeByIdAsync: async (id: string) => {
      if (nodes.has(id)) return nodes.get(id)
      return pages.find((item) => item.id === id) ?? null
    },
  }
  if (motion !== false) api.motion = motion
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: current, loadedPages, catalogCalls }
}

describe("get_motion", () => {
  beforeEach(() => {
    installFigma({ currentPage: page("0:2", "Current") })
  })

  test("fails the whole call when figma.motion is absent", async () => {
    installFigma({
      currentPage: page("0:2", "Current", [motionNode("6:1")]),
      motion: false,
    })
    await expect(getMotion({})).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
    expect(PluginReadError).toBeDefined()
  })

  test("fails the whole call when figmaAnimationStyles is absent", async () => {
    installFigma({
      currentPage: page("0:2", "Current", [motionNode("6:1")]),
      motion: {},
    })
    await expect(getMotion({})).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
  })

  test("treats mixin fields on the prototype chain as supported", async () => {
    const proto = {
      animationStyles: [appliedStyle("a-proto")],
      animations: {},
      manualKeyframeTracks: {},
      timelines: [{ id: "tl-proto", duration: 0.4 }],
    }
    const hosted = Object.assign(Object.create(proto), {
      id: "6:9",
      name: "Hosted",
      type: "FRAME",
      visible: true,
      children: [],
    })
    installFigma({
      currentPage: page("0:2", "Current", [hosted]),
      nodes: new Map([["6:9", hosted]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:9" } })
    const item = result.items.find(
      (entry) => entry.status === "success" && entry.value.nodeId === "6:9",
    )
    expect(item).toMatchObject({
      status: "success",
      value: {
        nodeId: "6:9",
        timelines: [{ id: "tl-proto", duration: 0.4 }],
      },
    })
  })

  test("marks nodes missing any of the four read properties as UNSUPPORTED_NODE", async () => {
    const supported = motionNode("6:1", {
      animationStyles: [appliedStyle("a-ok")],
      timelines: [{ id: "tl-ok", duration: 0.4 }],
    })
    const missingAnimations = {
      id: "6:2",
      name: "6:2",
      type: "FRAME",
      visible: true,
      children: [],
      animationStyles: [],
      manualKeyframeTracks: {},
      timelines: [],
    }
    installFigma({
      currentPage: page("0:2", "Current", [supported, missingAnimations]),
      nodes: new Map<string, unknown>([
        [String(supported.id), supported],
        [String(missingAnimations.id), missingAnimations],
      ]),
    })

    const result = await getMotion({
      selector: { nodeIds: ["6:1", "6:2"] },
    })
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "6:1",
          animationStyles: [{ id: "a-ok", styleId: "S:a-ok", name: "Fade" }],
          animations: [],
          manualKeyframeTracks: [],
          timelines: [{ id: "tl-ok", duration: 0.4 }],
        },
      },
      {
        status: "error",
        error: {
          code: "UNSUPPORTED_NODE",
          message: "The requested node type is not supported.",
          retryable: false,
        },
      },
    ])
  })

  test("keeps applied styles distinct from the catalog and copies seconds unchanged", async () => {
    const node = motionNode("6:1", {
      animationStyles: [
        {
          id: "applied-1",
          styleId: "S:fade",
          name: "Fade in",
          duration: 0.4,
          timelineOffset: 0.1,
          leftover: true,
          props: {
            direction: "right",
            distance: 120,
            enabled: true,
            easing: { type: "EASE_OUT" },
          },
        },
      ],
      animations: {
        TRANSLATION_X: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 0.4,
          tracks: [
            {
              id: "track-1",
              keyframeOperation: "SET",
              keyframes: [
                floatKeyframe("kf-1", 0.4, 120, { type: "EASE_IN_BACK" }),
              ],
            },
          ],
        },
      },
      timelines: [{ id: "tl-1", duration: 0.4, leftover: true }],
    })
    const { catalogCalls } = installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({
      selector: { nodeId: "6:1" },
      includeAvailableStyles: true,
    })
    expect(catalogCalls.count).toBe(1)
    expect(result.availableStyles).toEqual([
      {
        styleId: "S:fade",
        name: "Fade in",
        description: "Catalog fade",
        props: [
          { name: "direction", value: "string" },
          { name: "distance", value: "number" },
        ],
      },
    ])
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "6:1",
          animationStyles: [
            {
              id: "applied-1",
              styleId: "S:fade",
              name: "Fade in",
              duration: 0.4,
              timelineOffset: 0.1,
              props: [
                { name: "direction", value: "right" },
                { name: "distance", value: 120 },
                { name: "easing", value: { type: "EASE_OUT" } },
                { name: "enabled", value: true },
              ],
            },
          ],
          animations: [
            {
              field: { type: "property", name: "TRANSLATION_X" },
              baseValue: { type: "FLOAT", value: 0 },
              timelineDuration: 0.4,
              tracks: [
                {
                  id: "track-1",
                  keyframeOperation: "SET",
                  keyframes: [
                    {
                      id: "kf-1",
                      timelinePosition: 0.4,
                      value: { type: "FLOAT", value: 120 },
                      easing: { type: "EASE_IN_BACK" },
                    },
                  ],
                },
              ],
            },
          ],
          manualKeyframeTracks: [],
          timelines: [{ id: "tl-1", duration: 0.4 }],
        },
      },
    ])
    expect(JSON.stringify(result)).not.toContain("400")
    expect(JSON.stringify(result)).not.toContain("durationMs")
    expect(JSON.stringify(result)).not.toContain("delayMs")
    expect(JSON.stringify(result)).not.toContain("startsAtMs")
    expect(JSON.stringify(result)).not.toContain("leftover")
    const applied = result.items[0]
    expect(applied?.status).toBe("success")
    if (applied?.status === "success") {
      expect(applied.value.animationStyles[0]).not.toHaveProperty("description")
      expect(result.availableStyles?.[0]).not.toHaveProperty("duration")
      expect(result.availableStyles?.[0]).not.toHaveProperty("id")
    }
  })

  test("flattens keyed animations and manual tracks in sorted field then index order", async () => {
    const node = motionNode("6:1", {
      animations: {
        TRANSLATION_Y: {
          baseValue: { type: "FLOAT", value: 2 },
          timelineDuration: 0.5,
          tracks: [],
        },
        TRANSLATION_X: {
          baseValue: { type: "FLOAT", value: 1 },
          timelineDuration: 0.5,
          tracks: [],
        },
        fills: {
          1: {
            baseValue: { type: "COLOR", value: { r: 0, g: 1, b: 0, a: 1 } },
            timelineDuration: 0.5,
            tracks: [],
          },
          0: {
            properties: {
              "prop-b": {
                baseValue: { type: "FLOAT", value: 9 },
                timelineDuration: 0.5,
                tracks: [],
              },
              "prop-a": {
                baseValue: { type: "FLOAT", value: 8 },
                timelineDuration: 0.5,
                tracks: [],
              },
            },
          },
        },
        strokes: {
          0: {
            baseValue: { type: "COLOR", value: { r: 0, g: 0, b: 1, a: 1 } },
            timelineDuration: 0.5,
            tracks: [],
          },
        },
        effects: {
          0: {
            COLOR: {
              baseValue: { type: "COLOR", value: { r: 1, g: 1, b: 0, a: 1 } },
              timelineDuration: 0.5,
              tracks: [],
            },
            OFFSET_X: {
              baseValue: { type: "FLOAT", value: 4 },
              timelineDuration: 0.5,
              tracks: [],
            },
            properties: {
              "fx-1": {
                baseValue: { type: "FLOAT", value: 3 },
                timelineDuration: 0.5,
                tracks: [],
              },
            },
          },
        },
      },
      manualKeyframeTracks: {
        OPACITY: {
          id: "manual-opacity",
          baseValue: { type: "FLOAT", value: 1 },
          keyframes: [floatKeyframe("kf-op", 0.2, 0, { type: "LINEAR" })],
        },
        fills: {
          0: {
            id: "manual-fill",
            baseValue: { type: "COLOR", value: { r: 1, g: 0, b: 0, a: 1 } },
            keyframes: [],
          },
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    expect(result.items[0]?.status).toBe("success")
    if (result.items[0]?.status !== "success") return
    expect(result.items[0].value.animations.map((item) => item.field)).toEqual([
      { type: "property", name: "TRANSLATION_X" },
      { type: "property", name: "TRANSLATION_Y" },
      {
        type: "indexedItem",
        collection: "fills",
        index: 0,
        propertyId: "prop-a",
      },
      {
        type: "indexedItem",
        collection: "fills",
        index: 0,
        propertyId: "prop-b",
      },
      { type: "indexedItem", collection: "fills", index: 1 },
      { type: "indexedItem", collection: "strokes", index: 0 },
      { type: "indexedItem", collection: "effects", index: 0, field: "COLOR" },
      {
        type: "indexedItem",
        collection: "effects",
        index: 0,
        field: "OFFSET_X",
      },
      {
        type: "indexedItem",
        collection: "effects",
        index: 0,
        propertyId: "fx-1",
      },
    ])
    expect(
      result.items[0].value.manualKeyframeTracks.map((item) => item.field),
    ).toEqual([
      { type: "property", name: "OPACITY" },
      { type: "indexedItem", collection: "fills", index: 0 },
    ])
  })

  test("serializes the full easing set and closed keyframe value tags", async () => {
    const frames = [
      { value: { type: "FLOAT", value: 1 }, easing: { type: "LINEAR" } },
      {
        value: { type: "COLOR", value: { r: 1, g: 0, b: 0, a: 1 } },
        easing: { type: "EASE_IN_BACK" },
      },
      {
        value: { type: "TEXT_DATA", value: "hello" },
        easing: { type: "GENTLE" },
      },
      {
        value: { type: "VECTOR", value: { x: 1, y: 2 } },
        easing: { type: "QUICK" },
      },
      { value: { type: "BOOL", value: true }, easing: { type: "BOUNCY" } },
      {
        value: { type: "CIRCLE", value: { x: 1, y: 2, radius: 3 } },
        easing: { type: "SLOW" },
      },
      {
        value: { type: "LINE", value: { x: 0, y: 0, x2: 1, y2: 1 } },
        easing: {
          type: "CUSTOM_SPRING",
          easingFunctionSpring: { bounce: 0.5 },
        },
      },
      {
        value: {
          type: "CIRCLE_POINT",
          value: { x: 1, y: 2, radius: 3, angle: 0.5 },
        },
        easing: {
          type: "CUSTOM_CUBIC_BEZIER",
          easingFunctionCubicBezier: { x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 },
        },
      },
      {
        value: {
          type: "COLOR_POINT",
          value: { x: 1, y: 2, color: { r: 0, g: 1, b: 0, a: 1 } },
        },
        easing: { type: "HOLD" },
      },
      {
        value: { type: "MESH", value: { leftover: true } },
        easing: { type: "VARIABLE_ALIAS", id: "V:ease" },
      },
    ]
    const node = motionNode("6:1", {
      manualKeyframeTracks: {
        WIDTH: {
          id: "manual-width",
          baseValue: { type: "FLOAT", value: 0 },
          keyframes: frames.map((frame, index) => ({
            id: `kf-${index}`,
            timelinePosition: 0.1 * index,
            value: frame.value,
            easing: frame.easing,
          })),
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    expect(result.items[0]?.status).toBe("success")
    if (result.items[0]?.status !== "success") return
    const keyframes = result.items[0].value.manualKeyframeTracks[0]?.keyframes
    expect(keyframes?.map((frame) => frame.value.type)).toEqual([
      "FLOAT",
      "COLOR",
      "TEXT_DATA",
      "VECTOR",
      "BOOL",
      "CIRCLE",
      "LINE",
      "CIRCLE_POINT",
      "COLOR_POINT",
      "unsupported",
    ])
    expect(keyframes?.[9]).toEqual({
      id: "kf-9",
      timelinePosition: 0.9,
      value: { type: "unsupported", tag: "MESH" },
      easing: { type: "VARIABLE_ALIAS", id: "V:ease" },
    })
    expect(keyframes?.map((frame) => frame.easing.type)).toEqual([
      "LINEAR",
      "EASE_IN_BACK",
      "GENTLE",
      "QUICK",
      "BOUNCY",
      "SLOW",
      "CUSTOM_SPRING",
      "CUSTOM_CUBIC_BEZIER",
      "HOLD",
      "VARIABLE_ALIAS",
    ])
    expect(keyframes?.[7]?.easing).toEqual({
      type: "CUSTOM_CUBIC_BEZIER",
      easingFunctionCubicBezier: { x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 },
    })
  })

  test("does not call figmaAnimationStyles when includeAvailableStyles is false", async () => {
    const node = motionNode("6:1")
    const { catalogCalls } = installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({
      selector: { nodeId: "6:1" },
      includeAvailableStyles: false,
    })
    expect(catalogCalls.count).toBe(0)
    expect(result.availableStyles).toBeUndefined()
  })

  test("loads an explicit page without changing the current page", async () => {
    const requested = page("0:1", "Requested", [
      motionNode("6:1", {
        animationStyles: [appliedStyle("a-1")],
        timelines: [{ id: "tl-1", duration: 1 }],
      }),
    ])
    const current = page("0:2", "Current", [
      motionNode("6:9", {
        animationStyles: [appliedStyle("a-9")],
        timelines: [{ id: "tl-current", duration: 2 }],
      }),
    ])
    const { currentPage, loadedPages } = installFigma({
      currentPage: current,
      pages: [requested, current],
    })

    const result = await getMotion({ selector: { pageId: requested.id } })
    expect(loadedPages).toEqual(["0:1"])
    expect(
      (globalThis as typeof globalThis & { figma: { currentPage: unknown } })
        .figma.currentPage,
    ).toBe(currentPage)
    const ids = result.items.flatMap((item) =>
      item.status === "success" ? [item.value.nodeId] : [],
    )
    expect(ids).toContain("6:1")
    expect(ids).not.toContain("6:9")
  })

  test("a timeline with no animation is not content, and is still counted", async () => {
    // The measured page returned 563 nodes each carrying one timeline of an
    // identical duration and zero animations, for 237,212 bytes. A duration
    // with nothing keyed to it describes no motion, so those nodes are dropped
    // and accounted for in visitedNodes instead.
    const animated = motionNode("6:1", {
      animationStyles: [appliedStyle("a-1")],
      timelines: [{ id: "tl-shared", duration: 0.4 }],
    })
    const ambient = motionNode("6:2", {
      timelines: [{ id: "tl-shared", duration: 0.4 }],
    })
    installFigma({
      currentPage: page("0:2", "Current", [animated, ambient]),
    })

    const result = await getMotion({})
    const ids = result.items.flatMap((item) =>
      item.status === "success" ? [item.value.nodeId] : [],
    )
    expect(ids).toEqual(["6:1"])
    // The page node reports no motion fields at all, so it is an error item.
    expect(result.items.filter((item) => item.status === "error")).toHaveLength(
      1,
    )
    expect(result.visitedNodes).toBe(3)
    expect(result.truncated).toBe(false)
  })

  test("keeps nodes already indexed when the visit ceiling is hit", async () => {
    const first = motionNode("6:1", {
      animationStyles: [appliedStyle("a-1")],
      timelines: [{ id: "tl-1", duration: 0.4 }],
    })
    const extras = Array.from({ length: 4 }, (_, index) =>
      motionNode(`1:${index + 1}`),
    )
    const later = motionNode("6:2", {
      animationStyles: [appliedStyle("a-2")],
      timelines: [{ id: "tl-2", duration: 0.8 }],
    })
    installFigma({
      currentPage: page("0:2", "Current", [first, ...extras, later]),
    })

    const result = await getMotion({}, undefined, {
      returnedNodes: 10,
      visitedNodes: 2,
      encodedBytes: 8 * 1024 * 1024,
    })
    const ids = result.items.flatMap((item) =>
      item.status === "success" ? [item.value.nodeId] : [],
    )
    expect(ids).toContain("6:1")
    expect(ids).not.toContain("6:2")
    expect(result.truncated).toBe(true)
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      motionNode(`6:${index + 1}`),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return motionNode("6:51")
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    await expect(
      getMotion({ selector: { pageId: requested.id } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })
})
