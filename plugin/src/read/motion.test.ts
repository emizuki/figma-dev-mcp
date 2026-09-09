import { beforeEach, describe, expect, test } from "bun:test"

import { installFigma } from "../../tests/figma-harness"
import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getMotion } from "./motion"
import { byteLength } from "./serialize"

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
    const { motionCatalogCalls } = installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({
      selector: { nodeId: "6:1" },
      includeAvailableStyles: true,
    })
    expect(motionCatalogCalls.count).toBe(1)
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
    const { motionCatalogCalls } = installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({
      selector: { nodeId: "6:1" },
      includeAvailableStyles: false,
    })
    expect(motionCatalogCalls.count).toBe(0)
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

  test("every easing type the protocol names survives a keyframe", async () => {
    const named = [
      "LINEAR",
      "EASE_IN",
      "EASE_OUT",
      "EASE_IN_AND_OUT",
      "EASE_IN_BACK",
      "EASE_OUT_BACK",
      "EASE_IN_AND_OUT_BACK",
      "CUSTOM_CUBIC_BEZIER",
      "GENTLE",
      "QUICK",
      "BOUNCY",
      "SLOW",
      "CUSTOM_SPRING",
      "HOLD",
    ] as const
    const node = motionNode("6:1", {
      animations: {
        TRANSLATION_X: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 1,
          tracks: [
            {
              id: "track-1",
              keyframeOperation: "SET",
              keyframes: [
                ...named.map((type, index) =>
                  floatKeyframe(`kf-${type}`, index, index, { type }),
                ),
                floatKeyframe("kf-alias", 99, 99, {
                  type: "VARIABLE_ALIAS",
                  id: "VariableID:1:2",
                }),
                floatKeyframe("kf-bezier", 100, 100, {
                  type: "CUSTOM_CUBIC_BEZIER",
                  easingFunctionCubicBezier: {
                    x1: 0.1,
                    y1: 0.2,
                    x2: 0.3,
                    y2: 0.4,
                  },
                }),
                floatKeyframe("kf-spring", 101, 101, {
                  type: "CUSTOM_SPRING",
                  easingFunctionSpring: { bounce: 0.55 },
                }),
              ],
            },
          ],
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const [item] = result.items
    expect(item?.status).toBe("success")
    const binding =
      item?.status === "success" ? item.value.animations[0] : undefined
    expect(binding?.tracks[0]?.keyframes.map((frame) => frame.easing)).toEqual([
      ...named.map((type) => ({ type })),
      { type: "VARIABLE_ALIAS", id: "VariableID:1:2" },
      {
        type: "CUSTOM_CUBIC_BEZIER",
        easingFunctionCubicBezier: { x1: 0.1, y1: 0.2, x2: 0.3, y2: 0.4 },
      },
      { type: "CUSTOM_SPRING", easingFunctionSpring: { bounce: 0.55 } },
    ])
  })

  test("every keyframe value shape the protocol names carries its payload", async () => {
    const values = [
      { type: "FLOAT", value: 1.5 },
      { type: "COLOR", value: { r: 0.1, g: 0.2, b: 0.3, a: 0.4 } },
      { type: "TEXT_DATA", value: "Buy now" },
      { type: "VECTOR", value: { x: 3, y: 4 } },
      { type: "BOOL", value: false },
      { type: "CIRCLE", value: { x: 1, y: 2, radius: 3 } },
      { type: "LINE", value: { x: 1, y: 2, x2: 3, y2: 4 } },
      { type: "CIRCLE_POINT", value: { x: 1, y: 2, radius: 3, angle: 4 } },
      {
        type: "COLOR_POINT",
        value: { x: 5, y: 6, color: { r: 0.5, g: 0.6, b: 0.7, a: 0.8 } },
      },
      { type: "NOISE", value: 1 },
    ]
    const node = motionNode("6:1", {
      animations: {
        TRANSLATION_X: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 1,
          tracks: [
            {
              id: "track-1",
              keyframeOperation: "SET",
              keyframes: values.map((value, index) => ({
                id: `kf-${index}`,
                timelinePosition: index,
                value,
                easing: { type: "LINEAR" },
              })),
            },
          ],
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const item = result.items[0]
    const binding =
      item?.status === "success" ? item.value.animations[0] : undefined
    expect(binding?.tracks[0]?.keyframes.map((frame) => frame.value)).toEqual([
      { type: "FLOAT", value: 1.5 },
      { type: "COLOR", value: { r: 0.1, g: 0.2, b: 0.3, a: 0.4 } },
      { type: "TEXT_DATA", value: "Buy now" },
      { type: "VECTOR", value: { x: 3, y: 4 } },
      { type: "BOOL", value: false },
      { type: "CIRCLE", value: { x: 1, y: 2, radius: 3 } },
      { type: "LINE", value: { x: 1, y: 2, x2: 3, y2: 4 } },
      { type: "CIRCLE_POINT", value: { x: 1, y: 2, radius: 3, angle: 4 } },
      {
        type: "COLOR_POINT",
        value: { x: 5, y: 6, color: { r: 0.5, g: 0.6, b: 0.7, a: 0.8 } },
      },
      { type: "unsupported", tag: "NOISE" },
    ])
  })

  test("all three keyframe operations survive, and a track keeps the one it has", async () => {
    const node = motionNode("6:1", {
      animations: {
        A_SET: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 1,
          tracks: [{ id: "t-set", keyframeOperation: "SET", keyframes: [] }],
        },
        B_OFFSET: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 1,
          tracks: [
            { id: "t-offset", keyframeOperation: "OFFSET", keyframes: [] },
          ],
        },
        C_SCALE: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 1,
          tracks: [
            { id: "t-scale", keyframeOperation: "SCALE", keyframes: [] },
          ],
        },
        D_UNKNOWN: {
          baseValue: { type: "FLOAT", value: 0 },
          timelineDuration: 1,
          tracks: [
            { id: "t-none", keyframeOperation: "WOBBLE", keyframes: [] },
          ],
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const item = result.items[0]
    const animations = item?.status === "success" ? item.value.animations : []
    expect(
      animations.map((binding) =>
        binding.tracks.map((track) => [track.id, track.keyframeOperation]),
      ),
    ).toEqual([
      [["t-set", "SET"]],
      [["t-offset", "OFFSET"]],
      [["t-scale", "SCALE"]],
      [],
    ])
  })

  test("a binding is recognised by tracks or by timelineDuration alone", async () => {
    const node = motionNode("6:1", {
      animations: {
        A_TRACKS: {
          baseValue: { type: "FLOAT", value: 1 },
          timelineDuration: 2,
          tracks: [],
        },
        B_DURATION_ONLY: {
          baseValue: { type: "FLOAT", value: 3 },
          timelineDuration: 4,
        },
        C_NEITHER: { baseValue: { type: "FLOAT", value: 5 } },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const item = result.items[0]
    expect(item?.status === "success" ? item.value.animations : []).toEqual([
      {
        field: { type: "property", name: "A_TRACKS" },
        baseValue: { type: "FLOAT", value: 1 },
        timelineDuration: 2,
        tracks: [],
      },
      {
        field: { type: "property", name: "B_DURATION_ONLY" },
        baseValue: { type: "FLOAT", value: 3 },
        timelineDuration: 4,
        tracks: [],
      },
    ])
  })

  test("manual tracks carry id and base value, on properties and inside collections", async () => {
    const node = motionNode("6:1", {
      manualKeyframeTracks: {
        OPACITY: {
          id: "mt-opacity",
          baseValue: { type: "FLOAT", value: 1 },
          keyframes: [],
        },
        fills: {
          "0": {
            properties: {
              color: {
                id: "mt-fill-colour",
                baseValue: { type: "COLOR", value: { r: 1, g: 0, b: 0, a: 1 } },
                keyframes: [],
              },
            },
          },
        },
        effects: {
          "0": {
            radius: {
              id: "mt-effect-radius",
              baseValue: { type: "FLOAT", value: 4 },
              keyframes: [],
            },
            properties: {
              spread: {
                id: "mt-effect-spread",
                baseValue: { type: "FLOAT", value: 2 },
                keyframes: [],
              },
            },
          },
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const item = result.items[0]
    expect(
      item?.status === "success" ? item.value.manualKeyframeTracks : [],
    ).toEqual([
      {
        field: { type: "property", name: "OPACITY" },
        id: "mt-opacity",
        baseValue: { type: "FLOAT", value: 1 },
        keyframes: [],
      },
      {
        field: {
          type: "indexedItem",
          collection: "fills",
          index: 0,
          propertyId: "color",
        },
        id: "mt-fill-colour",
        baseValue: { type: "COLOR", value: { r: 1, g: 0, b: 0, a: 1 } },
        keyframes: [],
      },
      {
        field: {
          type: "indexedItem",
          collection: "effects",
          index: 0,
          field: "radius",
        },
        id: "mt-effect-radius",
        baseValue: { type: "FLOAT", value: 4 },
        keyframes: [],
      },
      {
        field: {
          type: "indexedItem",
          collection: "effects",
          index: 0,
          propertyId: "spread",
        },
        id: "mt-effect-spread",
        baseValue: { type: "FLOAT", value: 2 },
        keyframes: [],
      },
    ])
  })

  test("the catalogue reader is called on figma.motion", async () => {
    const surface = {
      badge: "S:from-this",
      figmaAnimationStyles(this: { badge: string }) {
        return [{ styleId: this.badge, name: "Fade in" }]
      },
    }
    installFigma({ currentPage: page("0:2", "Current"), motion: surface })

    const result = await getMotion({ includeAvailableStyles: true })

    expect(result.availableStyles).toEqual([
      { styleId: "S:from-this", name: "Fade in" },
    ])
  })

  test("a motion colour keeps the host's own channels", async () => {
    const node = motionNode("6:1", {
      animations: {
        FILL: {
          baseValue: {
            type: "COLOR",
            value: { r: 0.25, g: 0.5, b: 0.75, a: 0.125 },
          },
          timelineDuration: 1,
          tracks: [],
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const item = result.items[0]
    expect(
      item?.status === "success"
        ? item.value.animations[0]?.baseValue
        : undefined,
    ).toEqual({
      type: "COLOR",
      value: { r: 0.25, g: 0.5, b: 0.75, a: 0.125 },
    })
  })

  test("a property named like an indexed collection is not scanned as a plain property", async () => {
    const node = motionNode("6:1", {
      animationStyles: [appliedStyle("a-ok")],
      animations: {
        fills: {
          "0": {
            properties: {
              color: {
                baseValue: { type: "FLOAT", value: 1 },
                timelineDuration: 1,
                tracks: [],
              },
            },
          },
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    const item = result.items[0]
    expect(
      item?.status === "success"
        ? item.value.animations.map((binding) => binding.field)
        : [],
    ).toEqual([
      {
        type: "indexedItem",
        collection: "fills",
        index: 0,
        propertyId: "color",
      },
    ])
  })

  test("a node whose only content is an animation is emitted", async () => {
    const node = motionNode("6:1", {
      animations: {
        OPACITY: {
          baseValue: { type: "FLOAT", value: 1 },
          timelineDuration: 1,
          tracks: [],
        },
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [node]),
      nodes: new Map<string, unknown>([[String(node.id), node]]),
    })

    const result = await getMotion({ selector: { nodeId: "6:1" } })
    expect(result.items).toHaveLength(1)
    expect(result.visitedNodes).toBe(1)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("the item ceiling is inclusive and reports how many nodes were inspected", async () => {
    const nodes = [1, 2, 3].map((index) =>
      motionNode(`6:${index}`, {
        animationStyles: [appliedStyle(`a-${index}`)],
      }),
    )
    installFigma({
      currentPage: page("0:2", "Current", nodes),
      nodes: new Map<string, unknown>(
        nodes.map((node) => [String(node.id), node]),
      ),
    })

    const result = await getMotion(
      { selector: { nodeIds: ["6:1", "6:2", "6:3"] } },
      undefined,
      { returnedNodes: 1 },
    )

    expect(result.items).toHaveLength(1)
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })
  })

  test("a truncated walk outranks the emission cut and keeps its own reason", async () => {
    const nodes = [1, 2, 3, 4].map((index) =>
      motionNode(`6:${index}`, {
        animationStyles: [appliedStyle(`a-${index}`)],
      }),
    )
    installFigma({
      currentPage: page("0:2", "Current", nodes),
      nodes: new Map<string, unknown>(
        nodes.map((node) => [String(node.id), node]),
      ),
    })

    // The walk stops after three nodes; the emission ceiling stops after one,
    // having considered two. The walk is the earlier, larger loss, so its
    // count — three, not two — is what the result reports.
    const result = await getMotion(
      { selector: { nodeIds: ["6:1", "6:2", "6:3", "6:4"] } },
      undefined,
      { visitedNodes: 3, returnedNodes: 1 },
    )

    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })
  })

  test("the byte ceiling counts every emitted item and reports the total", async () => {
    const nodes = [1, 2].map((index) =>
      motionNode(`6:${index}`, {
        animationStyles: [appliedStyle(`a-${index}`)],
      }),
    )
    installFigma({
      currentPage: page("0:2", "Current", nodes),
      nodes: new Map<string, unknown>(
        nodes.map((node) => [String(node.id), node]),
      ),
    })
    const item = (index: number) => ({
      status: "success" as const,
      value: {
        nodeId: `6:${index}`,
        animationStyles: [
          { id: `a-${index}`, styleId: `S:a-${index}`, name: "Fade" },
        ],
        animations: [],
        manualKeyframeTracks: [],
        timelines: [],
      },
    })
    const budget = byteLength(item(1)) + byteLength(item(2)) - 1

    const result = await getMotion(
      { selector: { nodeIds: ["6:1", "6:2"] } },
      undefined,
      { encodedBytes: budget },
    )

    expect(result.items).toEqual([item(1)])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "byteLimit",
      encodedBytes: byteLength(item(1)) + byteLength(item(2)),
    })
  })

  // `throwIfAbortedAtBatch` polls only when `index % 100 === 0`, so an abort
  // raised between batch boundaries is seen by nothing but the unconditional
  // check beside it.
  test("the motion item loop checks cancellation on every item", async () => {
    const cancellation = new LocalCancellationController()
    const first = motionNode("6:1")
    Object.defineProperty(first, "animationStyles", {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return []
      },
    })
    const second = motionNode("6:2", {
      animationStyles: [appliedStyle("a-2")],
    })
    installFigma({
      currentPage: page("0:2", "Current", [first, second]),
      nodes: new Map<string, unknown>([
        ["6:1", first],
        ["6:2", second],
      ]),
    })

    await expect(
      getMotion({ selector: { nodeIds: ["6:1", "6:2"] } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })

  test("the motion item loop stops at the ceiling, and stops counting too", async () => {
    const nodes = [1, 2, 3].map((index) =>
      motionNode(`6:${index}`, {
        animationStyles: [appliedStyle(`a-${index}`)],
      }),
    )
    installFigma({
      currentPage: page("0:2", "Current", nodes),
      nodes: new Map<string, unknown>(
        nodes.map((node) => [String(node.id), node]),
      ),
    })

    const result = await getMotion(
      { selector: { nodeIds: ["6:1", "6:2", "6:3"] } },
      undefined,
      { returnedNodes: 1 },
    )

    expect(result.items).toHaveLength(1)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })
    expect(result.visitedNodes).toBe(2)
  })
})
