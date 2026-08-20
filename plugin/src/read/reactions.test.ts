import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getReactions } from "./reactions"

const page = (id: string, name: string, children: unknown[] = []) => ({
  id,
  name,
  type: "PAGE",
  visible: true,
  children,
  loadAsync: async () => {},
})

const frame = (
  id: string,
  extras: Record<string, unknown> = {},
): Record<string, unknown> => ({
  id,
  name: id,
  type: "FRAME",
  visible: true,
  children: [],
  leftover: "must-not-leak",
  ...extras,
})

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
}): {
  currentPage: Record<string, unknown>
  loadedPages: string[]
  lookedUp: string[]
} {
  const loadedPages: string[] = []
  const lookedUp: string[] = []
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
    loadAllPagesAsync: async () => {
      throw new Error("reactions must not load every page")
    },
    getNodeByIdAsync: async (id: string) => {
      lookedUp.push(id)
      if (nodes.has(id)) return nodes.get(id)
      return pages.find((item) => item.id === id) ?? null
    },
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: current, loadedPages, lookedUp }
}

describe("get_reactions", () => {
  beforeEach(() => {
    installFigma({ currentPage: page("0:2", "Current") })
  })

  test("serializes navigation, overlay, close, back, and state-change reactions", async () => {
    const checkout = frame("5:9")
    const overlay = frame("5:8")
    const variant = frame("5:7")
    const source = frame("5:1", {
      reactions: [
        {
          leftover: true,
          trigger: { type: "ON_CLICK" },
          actions: [
            {
              type: "NODE",
              destinationId: "5:9",
              navigation: "NAVIGATE",
              transition: { type: "SMART_ANIMATE", duration: 0.2 },
              leftover: true,
            },
          ],
        },
        {
          trigger: { type: "ON_HOVER" },
          action: {
            type: "NODE",
            destinationId: "5:8",
            navigation: "OVERLAY",
            transition: { type: "MOVE_IN", direction: "BOTTOM" },
            overlayRelativePosition: { x: 8, y: 12 },
          },
        },
        {
          trigger: { type: "ON_PRESS" },
          actions: [{ type: "CLOSE" }],
        },
        {
          trigger: { type: "ON_DRAG" },
          actions: [{ type: "BACK" }],
        },
        {
          trigger: { type: "AFTER_TIMEOUT", timeout: 0.4 },
          actions: [
            {
              type: "NODE",
              destinationId: "5:7",
              navigation: "CHANGE_TO",
              transition: null,
            },
          ],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source]),
      nodes: new Map<string, unknown>([
        [String(source.id), source],
        [String(checkout.id), checkout],
        [String(overlay.id), overlay],
        [String(variant.id), variant],
      ]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.truncated).toBe(false)
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "5:1",
          reactions: [
            {
              trigger: "click",
              action: { type: "navigate", destinationId: "5:9" },
              transitionId: "SMART_ANIMATE",
              transitionDuration: 0.2,
              destinationAccessible: true,
            },
            {
              trigger: "hover",
              action: { type: "openOverlay", destinationId: "5:8" },
              transitionId: "MOVE_IN",
              destinationAccessible: true,
              overlay: { relativePosition: { x: 8, y: 12 } },
            },
            {
              trigger: "press",
              action: { type: "closeOverlay" },
              destinationAccessible: true,
            },
            {
              trigger: "drag",
              action: { type: "back" },
              destinationAccessible: true,
            },
            {
              trigger: "afterDelay",
              timeout: 0.4,
              action: { type: "changeTo", destinationId: "5:7" },
              destinationAccessible: true,
            },
          ],
        },
      },
    ])
    expect(JSON.stringify(result)).not.toContain("leftover")
  })

  test("keeps dangling destinations as typed references", async () => {
    const source = frame("5:1", {
      reactions: [
        {
          trigger: { type: "ON_CLICK" },
          actions: [
            {
              type: "NODE",
              destinationId: null,
              navigation: "NAVIGATE",
              transition: null,
            },
          ],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source]),
      nodes: new Map<string, unknown>([[String(source.id), source]]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "5:1",
          reactions: [
            {
              trigger: "click",
              action: { type: "navigate" },
              destinationAccessible: false,
            },
          ],
        },
      },
    ])
  })

  test("serializes overlay position, background, and interaction from the destination", async () => {
    const overlay = frame("5:8", {
      leftover: "must-not-leak",
      overlayPositionType: "BOTTOM_RIGHT",
      overlayBackground: {
        type: "SOLID_COLOR",
        color: { r: 0, g: 0, b: 0, a: 0.4 },
        leftover: true,
      },
      overlayBackgroundInteraction: "CLOSE_ON_CLICK_OUTSIDE",
    })
    const source = frame("5:1", {
      reactions: [
        {
          trigger: { type: "ON_CLICK" },
          actions: [
            {
              type: "NODE",
              destinationId: "5:8",
              navigation: "OVERLAY",
              transition: null,
              overlayRelativePosition: { x: 4, y: 6 },
            },
          ],
        },
        {
          trigger: { type: "ON_HOVER" },
          actions: [
            {
              type: "NODE",
              destinationId: "5:8",
              navigation: "SWAP",
              transition: null,
            },
          ],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source, overlay]),
      nodes: new Map<string, unknown>([
        [String(source.id), source],
        [String(overlay.id), overlay],
      ]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "5:1",
          reactions: [
            {
              trigger: "click",
              action: { type: "openOverlay", destinationId: "5:8" },
              destinationAccessible: true,
              overlay: {
                relativePosition: { x: 4, y: 6 },
                positionType: "bottomRight",
                background: {
                  type: "solidColor",
                  color: { r: 0, g: 0, b: 0, a: 0.4 },
                },
                backgroundInteraction: "closeOnClickOutside",
              },
            },
            {
              trigger: "hover",
              action: { type: "swapOverlay", destinationId: "5:8" },
              destinationAccessible: true,
              overlay: {
                positionType: "bottomRight",
                background: {
                  type: "solidColor",
                  color: { r: 0, g: 0, b: 0, a: 0.4 },
                },
                backgroundInteraction: "closeOnClickOutside",
              },
            },
          ],
        },
      },
    ])
    expect(JSON.stringify(result)).not.toContain("leftover")
    expect(JSON.stringify(result)).not.toContain("BOTTOM_RIGHT")
    expect(JSON.stringify(result)).not.toContain("SOLID_COLOR")
  })

  test("omits overlay when position and background settings are absent", async () => {
    const overlay = frame("5:8")
    const source = frame("5:1", {
      reactions: [
        {
          trigger: { type: "ON_CLICK" },
          actions: [
            {
              type: "NODE",
              destinationId: "5:8",
              navigation: "OVERLAY",
              transition: null,
            },
          ],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source, overlay]),
      nodes: new Map<string, unknown>([
        [String(source.id), source],
        [String(overlay.id), overlay],
      ]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        nodeId: "5:1",
        reactions: [
          {
            trigger: "click",
            action: { type: "openOverlay", destinationId: "5:8" },
            destinationAccessible: true,
          },
        ],
      },
    })
    const item = result.items[0]
    expect(item?.status).toBe("success")
    if (item?.status === "success") {
      expect(item.value.reactions[0]).not.toHaveProperty("overlay")
    }
  })

  test("marks inaccessible destinations without dropping the reaction", async () => {
    const source = frame("5:1", {
      reactions: [
        {
          trigger: { type: "MOUSE_ENTER", delay: 0, deprecatedVersion: false },
          actions: [
            {
              type: "NODE",
              destinationId: "9:9",
              navigation: "SCROLL_TO",
              transition: { type: "SCROLL_ANIMATE" },
            },
          ],
        },
        {
          trigger: { type: "ON_KEY_DOWN", device: "KEYBOARD", keyCodes: [13] },
          actions: [
            {
              type: "NODE",
              destinationId: "9:8",
              navigation: "SWAP",
              transition: null,
            },
          ],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source]),
      nodes: new Map<string, unknown>([[String(source.id), source]]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "5:1",
          reactions: [
            {
              trigger: "mouseEnter",
              delay: 0,
              action: { type: "scrollTo", destinationId: "9:9" },
              transitionId: "SCROLL_ANIMATE",
              destinationAccessible: false,
            },
            {
              trigger: "keyDown",
              device: "KEYBOARD",
              keyCodes: [13],
              action: { type: "swapOverlay", destinationId: "9:8" },
              destinationAccessible: false,
            },
          ],
        },
      },
    ])
  })

  test("keeps host trigger payloads, extra triggers, and non-navigation actions", async () => {
    const source = frame("5:1", {
      reactions: [
        {
          trigger: { type: "AFTER_TIMEOUT", timeout: 0.4 },
          actions: [
            {
              type: "NODE",
              destinationId: "5:9",
              navigation: "NAVIGATE",
              transition: { type: "SMART_ANIMATE", duration: 0.2 },
            },
          ],
        },
        {
          trigger: { type: "MOUSE_UP", delay: 0.1 },
          actions: [
            { type: "SET_VARIABLE", variableId: "V:1" },
            {
              type: "SET_VARIABLE_MODE",
              variableCollectionId: "C:1",
              variableModeId: "M:1",
            },
            { type: "CONDITIONAL", conditionalBlocks: [{}, {}] },
            {
              type: "UPDATE_MEDIA_RUNTIME",
              mediaAction: "SKIP_FORWARD",
              amountToSkip: 1.5,
              destinationId: "5:8",
            },
          ],
        },
        {
          trigger: {
            type: "ON_KEY_DOWN",
            device: "KEYBOARD",
            keyCodes: [13],
          },
          actions: [{ type: "BACK" }],
        },
        {
          trigger: { type: "ON_MEDIA_HIT", mediaHitTime: 1.2 },
          actions: [{ type: "BACK" }],
        },
        {
          trigger: { type: "ON_MEDIA_END" },
          actions: [{ type: "CLOSE" }],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source, frame("5:9"), frame("5:8")]),
      nodes: new Map<string, unknown>([
        ["5:1", source],
        ["5:9", frame("5:9")],
        ["5:8", frame("5:8")],
      ]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        nodeId: "5:1",
        reactions: [
          {
            trigger: "afterDelay",
            timeout: 0.4,
            action: { type: "navigate", destinationId: "5:9" },
            transitionId: "SMART_ANIMATE",
            transitionDuration: 0.2,
            destinationAccessible: true,
          },
          {
            trigger: "mouseUp",
            delay: 0.1,
            action: { type: "setVariable", variableId: "V:1" },
            destinationAccessible: false,
          },
          {
            trigger: "mouseUp",
            delay: 0.1,
            action: {
              type: "setVariableMode",
              variableCollectionId: "C:1",
              variableModeId: "M:1",
            },
            destinationAccessible: false,
          },
          {
            trigger: "mouseUp",
            delay: 0.1,
            action: { type: "conditional" },
            destinationAccessible: false,
          },
          {
            trigger: "mouseUp",
            delay: 0.1,
            action: {
              type: "updateMediaRuntime",
              mediaAction: "skipForward",
              amountToSkip: 1.5,
              destinationId: "5:8",
            },
            destinationAccessible: true,
          },
          {
            trigger: "keyDown",
            device: "KEYBOARD",
            keyCodes: [13],
            action: { type: "back" },
            destinationAccessible: true,
          },
          {
            trigger: "mediaHit",
            mediaHitTime: 1.2,
            action: { type: "back" },
            destinationAccessible: true,
          },
          {
            trigger: "mediaEnd",
            action: { type: "closeOverlay" },
            destinationAccessible: true,
          },
        ],
      },
    })
  })

  test("copies trigger timeout and delay without converting units", async () => {
    const source = frame("5:1", {
      reactions: [
        {
          trigger: { type: "AFTER_TIMEOUT", timeout: 400 },
          actions: [{ type: "BACK" }],
        },
        {
          trigger: { type: "MOUSE_DOWN", delay: 800 },
          actions: [{ type: "CLOSE" }],
        },
        {
          trigger: { type: "ON_MEDIA_HIT", timestamp: 2.5 },
          actions: [{ type: "BACK" }],
        },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [source]),
      nodes: new Map<string, unknown>([[String(source.id), source]]),
    })

    const result = await getReactions({ selector: { nodeId: "5:1" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        nodeId: "5:1",
        reactions: [
          {
            trigger: "afterDelay",
            timeout: 400,
            action: { type: "back" },
          },
          {
            trigger: "mouseDown",
            delay: 800,
            action: { type: "closeOverlay" },
          },
          {
            trigger: "mediaHit",
            mediaHitTime: 2.5,
            action: { type: "back" },
          },
        ],
      },
    })
  })

  test("reads reactions from mixin fields that are not own properties", async () => {
    const proto = {
      reactions: [
        { trigger: { type: "ON_CLICK" }, actions: [{ type: "BACK" }] },
      ],
    }
    const hosted = Object.assign(Object.create(proto), {
      id: "5:3",
      name: "Hosted",
      type: "FRAME",
      visible: true,
      children: [],
    })
    installFigma({
      currentPage: page("0:2", "Current", [hosted]),
      nodes: new Map([["5:3", hosted]]),
    })
    const result = await getReactions({ selector: { nodeId: "5:3" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        nodeId: "5:3",
        reactions: [{ trigger: "click", action: { type: "back" } }],
      },
    })
  })

  test("loads an explicit page without changing the current page", async () => {
    const requested = page("0:1", "Requested", [
      frame("5:1", {
        reactions: [
          {
            trigger: { type: "ON_CLICK" },
            actions: [{ type: "URL", url: "https://example.com" }],
          },
        ],
      }),
    ])
    const current = page("0:2", "Current", [
      frame("5:9", {
        reactions: [
          {
            trigger: { type: "ON_CLICK" },
            actions: [{ type: "BACK" }],
          },
        ],
      }),
    ])
    const { currentPage, loadedPages } = installFigma({
      currentPage: current,
      pages: [requested, current],
    })

    const result = await getReactions({ selector: { pageId: requested.id } })
    expect(loadedPages).toEqual(["0:1"])
    expect(
      (globalThis as typeof globalThis & { figma: { currentPage: unknown } })
        .figma.currentPage,
    ).toBe(currentPage)
    const actions = result.items.flatMap((item) =>
      item.status === "success"
        ? item.value.reactions.map((reaction) => reaction.action.type)
        : [],
    )
    expect(actions).toEqual(["openLink"])
  })

  test("nodes with nothing to report are not emitted, and are still counted", async () => {
    // 3 nodes, 1 with a reaction. Before this, all 3 were emitted; on a real
    // page that cost 356,668 bytes to report 11 reactions and exhausted the
    // node budget, so reactions past the cap went missing.
    const wired = frame("5:1", {
      reactions: [
        { trigger: { type: "ON_CLICK" }, actions: [{ type: "BACK" }] },
      ],
    })
    const silent = frame("5:2")
    const root = frame("root", { children: [wired, silent] })
    installFigma({
      currentPage: page("0:2", "Current", [root]),
      nodes: new Map<string, unknown>([["root", root]]),
    })

    const result = await getReactions({ selector: { nodeId: "root" } })
    expect(result.items).toHaveLength(1)
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: { nodeId: "5:1" },
    })
    expect(result.visitedNodes).toBe(3)
    expect(result.truncated).toBe(false)
  })

  test("separates scanned-and-found-nothing from never-reached", async () => {
    const root = frame("root", { children: [frame("5:1"), frame("5:2")] })
    installFigma({
      currentPage: page("0:2", "Current", [root]),
      nodes: new Map<string, unknown>([["root", root]]),
    })

    const result = await getReactions({ selector: { nodeId: "root" } })
    expect(result.items).toEqual([])
    expect(result.visitedNodes).toBe(3)
    expect(result.truncated).toBe(false)
  })

  test("counts the node that trips the emit ceiling", async () => {
    const wired = (id: string) =>
      frame(id, {
        reactions: [
          { trigger: { type: "ON_CLICK" }, actions: [{ type: "BACK" }] },
        ],
      })
    const root = frame("root", {
      children: [frame("5:0"), wired("5:1"), wired("5:2")],
    })
    installFigma({
      currentPage: page("0:2", "Current", [root]),
      nodes: new Map<string, unknown>([["root", root]]),
    })

    const result = await getReactions(
      { selector: { nodeId: "root" } },
      undefined,
      {
        returnedNodes: 1,
        visitedNodes: 100,
        encodedBytes: 8 * 1024 * 1024,
      },
    )
    expect(result.items).toHaveLength(1)
    // Four nodes were inspected before the ceiling stopped emission.
    expect(result.visitedNodes).toBe(4)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 4 })
  })

  test("keeps nodes already indexed when the visit ceiling is hit", async () => {
    const first = frame("5:1", {
      reactions: [
        { trigger: { type: "ON_CLICK" }, actions: [{ type: "BACK" }] },
      ],
    })
    const extras = Array.from({ length: 4 }, (_, index) =>
      frame(`1:${index + 1}`),
    )
    const later = frame("5:2", {
      reactions: [
        { trigger: { type: "ON_CLICK" }, actions: [{ type: "CLOSE" }] },
      ],
    })
    installFigma({
      currentPage: page("0:2", "Current", [first, ...extras, later]),
    })

    const result = await getReactions({}, undefined, {
      returnedNodes: 10,
      visitedNodes: 2,
      encodedBytes: 8 * 1024 * 1024,
    })
    const ids = result.items.flatMap((item) =>
      item.status === "success" ? [item.value.nodeId] : [],
    )
    expect(ids).toContain("5:1")
    expect(ids).not.toContain("5:2")
    expect(result.truncated).toBe(true)
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      frame(`5:${index + 1}`),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return frame("5:51")
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    await expect(
      getReactions({ selector: { pageId: requested.id } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
    expect(PluginReadError).toBeDefined()
  })
})
