import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getDevModeData } from "./dev-mode"

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

function findNode(root: unknown, id: string): unknown {
  const node = record(root)
  if (node.id === id) return root
  const children = Array.isArray(node.children) ? node.children : []
  for (const child of children) {
    const match = findNode(child, id)
    if (match !== undefined) return match
  }
  return undefined
}

function record(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {}
}

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
  categories?: unknown[]
  forbidCategories?: boolean
}): {
  currentPage: Record<string, unknown>
  loadedPages: string[]
  categoryLoads: { count: number }
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
  const categoryLoads = { count: 0 }
  const api: Record<string, unknown> = {
    root: { name: "Checkout flow", children: pages },
    currentPage: current,
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("dev-mode must not load every page")
    },
    getNodeByIdAsync: async (id: string) => {
      if (nodes.has(id)) return nodes.get(id)
      for (const pageNode of pages) {
        const match = findNode(pageNode, id)
        if (match !== undefined) return match
      }
      return null
    },
  }
  if (!options.forbidCategories) {
    api.annotations = {
      getAnnotationCategoriesAsync: async () => {
        categoryLoads.count += 1
        return (
          options.categories ?? [
            { id: "cat-note", label: "Note", color: "yellow", leftover: true },
            { id: "cat-todo", label: "Todo", color: "blue" },
          ]
        )
      },
    }
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: current, loadedPages, categoryLoads }
}

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
    const bare = frame("4:2")
    installFigma({
      currentPage: page("0:2", "Current", [bare]),
      forbidCategories: true,
    })

    const result = await getDevModeData({ selector: { nodeId: "4:2" } })
    expect(result.items).toEqual([
      {
        status: "success",
        value: {
          nodeId: "4:2",
          annotations: [],
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
    expect(ids).toContain("0:2")
    expect(ids).toContain("4:1")
    expect(ids).not.toContain("4:2")
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
})
