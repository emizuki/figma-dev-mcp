import { beforeEach, describe, expect, test } from "bun:test"

import { installFigma } from "../../tests/figma-harness"
import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getDevModeData } from "./dev-mode"
import { byteLength } from "./serialize"

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

describe("get_dev_mode_data", () => {
  beforeEach(() => {
    installFigma({ currentPage: page("0:2", "Current") })
  })

  test("serializes annotations, categories, descriptions, docs, resources, and inheritance", async () => {
    const card = frame("4:1", {
      description: "Primary card",
      descriptionMarkdown: "**Primary** card",
      documentationLinks: [
        { uri: "https://docs.example/card", leftover: true },
        { uri: "https://docs.example/labeled", label: "Guide" },
      ],
      annotations: [
        {
          label: "Match padding",
          categoryId: "cat-note",
          leftover: true,
        },
        { labelMarkdown: "Use **todo**", categoryId: "cat-todo" },
      ],
      getDevResourcesAsync: async () => [
        {
          name: "Storybook",
          url: "https://storybook.example/card",
          leftover: true,
        },
        {
          name: "Inherited spec",
          url: "https://docs.example/inherited",
          inheritedNodeId: "2:1",
        },
      ],
      ownerNodeId: "4:1",
      inheritedFromNodeId: "2:1",
    })
    const { categoryLoads } = installFigma({
      currentPage: page("0:2", "Current", [card]),
    })

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    expect(categoryLoads.count).toBe(1)
    expect(result.truncated).toBe(false)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "4:1",
          description: "Primary card",
          descriptionMarkdown: "**Primary** card",
          annotations: [
            {
              id: "4:1:annotation:0",
              categoryId: "cat-note",
              text: "Match padding",
            },
            {
              id: "4:1:annotation:1",
              categoryId: "cat-todo",
              text: "Use **todo**",
            },
          ],
          annotationCategories: [
            { id: "cat-note", label: "Note" },
            { id: "cat-todo", label: "Todo" },
          ],
          documentation: [
            { name: "", uri: "https://docs.example/card" },
            { name: "Guide", uri: "https://docs.example/labeled" },
          ],
          devResources: [
            { name: "Storybook", uri: "https://storybook.example/card" },
            { name: "Inherited spec", uri: "https://docs.example/inherited" },
          ],
          ownerNodeId: "4:1",
          inheritedFromNodeId: "2:1",
        },
      },
    ])
    expect(JSON.stringify(result)).not.toContain("leftover")
    expect(JSON.stringify(result)).not.toContain("must-not-leak")
  })

  test("omits unsupported fields and keeps empty capability-backed lists", async () => {
    // One annotation and nothing else: the node is emitted, so the three lists
    // it cannot fill stay present and empty rather than being dropped.
    const bare = frame("4:2", { annotations: [{ label: "Note" }] })
    installFigma({
      currentPage: page("0:2", "Current", [bare]),
      omitCategories: true,
    })

    const result = await getDevModeData({ selector: { nodeId: "4:2" } })
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "4:2",
          annotations: [{ id: "4:2:annotation:0", text: "Note" }],
          annotationCategories: [],
          documentation: [],
          devResources: [],
        },
      },
    ])
    const value = result.items[0]
    expect(value?.status).toBe("success")
    if (value?.status === "success") {
      expect(value.value).not.toHaveProperty("description")
      expect(value.value).not.toHaveProperty("descriptionMarkdown")
      expect(value.value).not.toHaveProperty("ownerNodeId")
      expect(value.value).not.toHaveProperty("inheritedFromNodeId")
    }
  })

  test("loads annotation categories once per request", async () => {
    const first = frame("4:1", {
      annotations: [{ label: "One", categoryId: "cat-note" }],
    })
    const second = frame("4:2", {
      annotations: [{ label: "Two", categoryId: "cat-todo" }],
    })
    const { categoryLoads } = installFigma({
      currentPage: page("0:2", "Current", [first, second]),
    })

    const result = await getDevModeData({})
    expect(categoryLoads.count).toBe(1)
    expect(
      result.items
        .filter((item) => item.status === "success")
        .map((item) => item.status === "success" && item.value.nodeId),
    ).toContain("4:1")
    expect(
      result.items
        .filter((item) => item.status === "success")
        .map((item) => item.status === "success" && item.value.nodeId),
    ).toContain("4:2")
  })

  test("loads an explicit page without changing the current page", async () => {
    const requested = page("0:1", "Requested", [
      frame("4:1", { description: "On requested page" }),
    ])
    const current = page("0:2", "Current", [
      frame("4:9", { description: "On current page" }),
    ])
    const { currentPage, loadedPages } = installFigma({
      currentPage: current,
      pages: [requested, current],
    })

    const result = await getDevModeData({ selector: { pageId: requested.id } })
    expect(loadedPages).toEqual(["0:1"])
    expect(
      (globalThis as typeof globalThis & { figma: { currentPage: unknown } })
        .figma.currentPage,
    ).toBe(currentPage)
    const descriptions = result.items.flatMap((item) =>
      item.status === "success" && item.value.description !== undefined
        ? [item.value.description]
        : [],
    )
    expect(descriptions).toEqual(["On requested page"])
  })

  test("keeps the call successful when annotation categories throw", async () => {
    const card = frame("4:1", { description: "Still readable" })
    installFigma({ currentPage: page("0:2", "Current", [card]) })
    const api = (
      globalThis as typeof globalThis & {
        figma: { annotations?: { getAnnotationCategoriesAsync?: unknown } }
      }
    ).figma
    api.annotations = {
      getAnnotationCategoriesAsync: async () => {
        throw new Error("annotations unavailable")
      },
    }

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: { nodeId: "4:1", description: "Still readable" },
    })
  })

  test("keeps the node when getDevResourcesAsync throws", async () => {
    const card = frame("4:1", {
      description: "Has a broken resource reader",
      getDevResourcesAsync: async () => {
        throw new Error("dev resources unavailable")
      },
    })
    installFigma({ currentPage: page("0:2", "Current", [card]) })

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        nodeId: "4:1",
        description: "Has a broken resource reader",
        devResources: [],
      },
    })
  })

  test("inherits ownership from inherited dev resources when node fields are absent", async () => {
    const card = frame("4:1", {
      getDevResourcesAsync: async () => [
        {
          name: "Spec",
          url: "https://docs.example/spec",
          inheritedNodeId: "2:8",
        },
      ],
    })
    installFigma({ currentPage: page("0:2", "Current", [card]) })

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: { nodeId: "4:1", inheritedFromNodeId: "2:8" },
    })
  })

  test("nodes with nothing to report are not emitted, and are still counted", async () => {
    // On one measured page this returned 563 items and 175,226 bytes of which
    // exactly one carried anything.
    const documented = frame("4:1", {
      annotations: [{ label: "Ship behind a flag" }],
    })
    const silent = frame("4:2")
    installFigma({
      currentPage: page("0:2", "Current", [documented, silent]),
    })

    const result = await getDevModeData({})
    expect(result.items).toHaveLength(1)
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: { nodeId: "4:1" },
    })
    // Page plus two frames: nothing was skipped, two had nothing to say.
    expect(result.visitedNodes).toBe(3)
    expect(result.truncated).toBe(false)
  })

  test("treats a default empty description as nothing to report", async () => {
    // The Figma Plugin API gives every ComponentNode, ComponentSetNode and
    // style node a default empty-string description. Testing the field for
    // presence rather than length would re-emit an empty record for every
    // component in the file, which is most real files.
    const blank = frame("4:2", { description: "", descriptionMarkdown: "" })
    const described = frame("4:1", {
      description: "Primary card",
      descriptionMarkdown: "**Primary** card",
    })
    installFigma({ currentPage: page("0:2", "Current", [described, blank]) })

    const result = await getDevModeData({})
    const ids = result.items.flatMap((item) =>
      item.status === "success" ? [item.value.nodeId] : [],
    )
    expect(ids).toEqual(["4:1"])
    expect(JSON.stringify(result)).not.toContain('"description":""')
    // Page, described node, blank node: the blank one was looked at.
    expect(result.visitedNodes).toBe(3)
  })

  test("keeps nodes already indexed when the visit ceiling is hit", async () => {
    const first = frame("4:1", { description: "Keep" })
    const extras = Array.from({ length: 4 }, (_, index) =>
      frame(`1:${index + 1}`),
    )
    const later = frame("4:2", { description: "Unseen" })
    installFigma({
      currentPage: page("0:2", "Current", [first, ...extras, later]),
    })

    const result = await getDevModeData({}, undefined, {
      returnedNodes: 10,
      visitedNodes: 2,
      encodedBytes: 8 * 1024 * 1024,
    })

    const ids = result.items.flatMap((item) =>
      item.status === "success" ? [item.value.nodeId] : [],
    )
    // The page itself carries no dev-mode data, so it is counted, not emitted.
    expect(ids).not.toContain("0:2")
    expect(ids).toContain("4:1")
    expect(ids).not.toContain("4:2")
    expect(result.visitedNodes).toBe(2)
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: 2,
    })
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      frame(`4:${index + 1}`),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return frame("4:51")
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    await expect(
      getDevModeData(
        { selector: { pageId: requested.id } },
        cancellation.signal,
      ),
    ).rejects.toThrow("Operation cancelled")
    expect(PluginReadError).toBeDefined()
  })

  test("each kind of content on its own is enough to emit a node", async () => {
    const nodes = [
      frame("4:1", {
        annotations: [{ label: "Note", categoryId: "cat-note" }],
      }),
      frame("4:2", {
        getDevResourcesAsync: async () => [
          { name: "Spec", url: "https://docs.example/spec" },
        ],
      }),
      frame("4:3", {
        documentationLinks: [{ uri: "https://docs.example/only" }],
      }),
      frame("4:4", { descriptionMarkdown: "**only markdown**" }),
      frame("4:5", { ownerNodeId: "2:9" }),
      frame("4:6", { inheritedFromNodeId: "2:8" }),
      frame("4:7", { description: "" }),
    ]
    installFigma({ currentPage: page("0:2", "Current", nodes) })

    const result = await getDevModeData({})

    expect(
      result.items.map((item) =>
        item.status === "success" ? item.value.nodeId : item.error.code,
      ),
    ).toEqual(["4:1", "4:2", "4:3", "4:4", "4:5", "4:6"])
    expect(result.visitedNodes).toBe(8)
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("only the categories a node references are attached, once each", async () => {
    const card = frame("4:1", {
      annotations: [
        { label: "One", categoryId: "cat-note" },
        { label: "Two", categoryId: "cat-note" },
      ],
    })
    installFigma({ currentPage: page("0:2", "Current", [card]) })

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    const item = result.items[0]

    expect(
      item?.status === "success" ? item.value.annotationCategories : [],
    ).toEqual([{ id: "cat-note", label: "Note" }])
  })

  test("an annotation keeps the host's own id when it has one", async () => {
    const card = frame("4:1", {
      annotations: [
        { id: "an-host", label: "Named", categoryId: "cat-note" },
        { label: "Unnamed" },
      ],
    })
    installFigma({ currentPage: page("0:2", "Current", [card]) })

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    const item = result.items[0]

    expect(item?.status === "success" ? item.value.annotations : []).toEqual([
      { id: "an-host", text: "Named", categoryId: "cat-note" },
      { id: "4:1:annotation:1", text: "Unnamed" },
    ])
  })

  test("the category reader is called on figma.annotations", async () => {
    const card = frame("4:1", {
      annotations: [{ label: "Note", categoryId: "cat-self" }],
    })
    installFigma({ currentPage: page("0:2", "Current", [card]) })
    const host = (
      globalThis as typeof globalThis & { figma: Record<string, unknown> }
    ).figma
    host.annotations = {
      badge: "cat-self",
      async getAnnotationCategoriesAsync(this: { badge: string }) {
        return [{ id: this.badge, label: "From this" }]
      },
    }

    const result = await getDevModeData({ selector: { nodeId: "4:1" } })
    const item = result.items[0]

    expect(
      item?.status === "success" ? item.value.annotationCategories : [],
    ).toEqual([{ id: "cat-self", label: "From this" }])
  })

  test("the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total", async () => {
    const nodes = [1, 2, 3, 4].map((index) =>
      frame(`4:${index}`, { description: `Card ${index}` }),
    )
    installFigma({ currentPage: page("0:2", "Current", nodes) })

    // The page carries nothing, so it is inspected and skipped: the second
    // emitted node is the third inspected.
    const capped = await getDevModeData({}, undefined, { returnedNodes: 1 })
    expect(capped.items).toHaveLength(1)
    expect(capped.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })

    // The walk stops after four nodes and the emission after three; the walk
    // is the earlier loss and its count is the one reported.
    const walked = await getDevModeData({}, undefined, {
      visitedNodes: 4,
      returnedNodes: 1,
    })
    expect(walked.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 4 })

    // The budget is spent on the node record, not on the item envelope.
    const value = (index: number) => ({
      nodeId: `4:${index}`,
      annotations: [],
      annotationCategories: [],
      documentation: [],
      devResources: [],
      description: `Card ${index}`,
    })
    const budget = byteLength(value(1)) + byteLength(value(2)) - 1
    const bytes = await getDevModeData({}, undefined, { encodedBytes: budget })
    expect(bytes.items).toEqual([{ status: "success", value: value(1) }])
    expect(bytes.truncation).toEqual({
      reason: "byteLimit",
      encodedBytes: byteLength(value(1)) + byteLength(value(2)),
    })
  })

  // `throwIfAbortedAtBatch` polls only when `index % 100 === 0`, so an abort
  // raised between batch boundaries is seen by nothing but the unconditional
  // check beside it.
  test("the dev-mode item loop checks cancellation on every item", async () => {
    const first = frame("4:1", {
      getDevResourcesAsync: async () => {
        cancellation.abort()
        return []
      },
    })
    const second = frame("4:2", { description: "Second" })
    const cancellation = new LocalCancellationController()
    installFigma({ currentPage: page("0:2", "Current", [first, second]) })

    await expect(
      getDevModeData(
        { selector: { nodeIds: ["4:1", "4:2"] } },
        cancellation.signal,
      ),
    ).rejects.toThrow("Operation cancelled")
  })
})
