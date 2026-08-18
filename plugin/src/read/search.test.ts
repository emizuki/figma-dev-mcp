import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import {
  compilePredicate,
  matchReasons,
  searchNodes,
  type SearchPredicate,
} from "./search"

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
  type: string,
  extras: Record<string, unknown> = {},
) => ({
  id,
  name,
  type,
  visible: true,
  children: [],
  ...extras,
})

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
  getNodeByIdAsync?: (id: string) => Promise<unknown>
}): { currentPage: Record<string, unknown>; lookedUp: string[] } {
  const lookedUp: string[] = []
  const pages = options.pages ?? [options.currentPage]
  const nodes = options.nodes ?? new Map()
  const api = {
    root: { name: "Checkout flow", children: pages },
    currentPage: options.currentPage,
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("search must not call loadAllPagesAsync")
    },
    loadFontAsync: async () => {
      throw new Error("search must not load fonts")
    },
    getNodeByIdAsync:
      options.getNodeByIdAsync ??
      (async (id: string) => {
        lookedUp.push(id)
        if (nodes.has(id)) return nodes.get(id)
        return pages.find((item) => item.id === id) ?? null
      }),
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: options.currentPage, lookedUp }
}

describe("search predicate", () => {
  test("requires query or types, trims them, and keeps match mode", () => {
    expect(() => compilePredicate({ match: "contains" })).toThrow(
      /query or types/,
    )
    expect(() => compilePredicate({ query: "   ", match: "contains" })).toThrow(
      /non-empty/,
    )
    expect(() =>
      compilePredicate({ types: ["   "], match: "contains" }),
    ).toThrow(/non-empty/)
    expect(
      compilePredicate({
        types: ["FRAME ", "FRAME"],
        match: "contains",
      }),
    ).toEqual({ types: ["FRAME"], match: "contains" })
    expect(compilePredicate({ query: "  Card  ", match: "exact" })).toEqual({
      query: "Card",
      match: "exact",
    })
  })

  test("matches exact and contains case-insensitively across name and text", () => {
    const card = node("1:1", "Card", "FRAME")
    const titled = node("1:2", "Card Title", "FRAME")
    const text = node("1:3", "Label", "TEXT", { characters: "CARD" })

    const exact: SearchPredicate = { query: "card", match: "exact" }
    const contains: SearchPredicate = { query: "card", match: "contains" }

    expect(matchReasons(card, exact)).toEqual(["name"])
    expect(matchReasons(titled, exact)).toEqual([])
    expect(matchReasons(card, contains)).toEqual(["name"])
    expect(matchReasons(titled, contains)).toEqual(["name"])
    expect(matchReasons(text, exact)).toEqual(["text"])
  })

  test("emits every applicable reason and ANDs query with types", () => {
    const pay = node("1:3", "Pay now", "TEXT", { characters: "Pay now" })
    const frame = node("1:4", "Pay now", "FRAME")

    const predicate: SearchPredicate = {
      query: "Pay now",
      types: ["TEXT"],
      match: "exact",
    }

    expect(matchReasons(pay, predicate)).toEqual(["nodeType", "name", "text"])
    expect(matchReasons(frame, predicate)).toEqual([])
  })

  test("reads text characters only from text-bearing nodes and never loads fonts", () => {
    const throwingText = node("1:5", "Label", "TEXT")
    Object.defineProperty(throwingText, "characters", {
      get() {
        throw new Error("font not loaded")
      },
    })
    const frame = node("1:6", "Container", "FRAME", { characters: "Pay" })
    const text = node("1:7", "Label", "TEXT", { characters: "Pay now" })
    const predicate: SearchPredicate = { query: "Pay", match: "contains" }

    expect(matchReasons(throwingText, predicate)).toEqual([])
    expect(matchReasons(frame, predicate)).toEqual([])
    expect(matchReasons(text, predicate)).toEqual(["text"])
  })
})

describe("search_nodes handler", () => {
  beforeEach(() => {
    installFigma({
      currentPage: page("0:2", "Current"),
    })
  })

  test("accepts a flat query and searches names plus TEXT characters", async () => {
    const text = node("1:2", "Label", "TEXT", { characters: "Commission" })
    const frame = node("1:3", "Commission card", "FRAME")
    const requested = page("0:1", "Requested", [text, frame])
    installFigma({ currentPage: requested, pages: [requested] })

    const result = await searchNodes({
      scope: { nodeId: requested.id },
      query: "commission",
      match: "contains",
      limit: 50,
    })

    expect(result.matches.map((item) => [item.node.id, item.reasons])).toEqual([
      ["1:2", ["text"]],
      ["1:3", ["name"]],
    ])
  })

  test("paginates without duplicates and rejects a changed query", async () => {
    const requested = page("0:1", "Requested", [
      node("1:1", "Card one", "FRAME"),
      node("1:2", "Card two", "FRAME"),
      node("1:3", "Card three", "FRAME"),
    ])
    installFigma({ currentPage: requested, pages: [requested] })

    const first = await searchNodes({
      scope: { pageId: requested.id },
      query: "Card",
      match: "contains",
      limit: 2,
    })
    expect(first.matches.map((item) => item.node.id)).toEqual(["1:1", "1:2"])
    expect(first.nextCursor).toBeString()
    expect(first.truncated).toBe(false)
    const cursor = first.nextCursor
    if (cursor === undefined) throw new Error("expected next cursor")

    const second = await searchNodes({
      scope: { pageId: requested.id },
      query: "Card",
      match: "contains",
      limit: 2,
      cursor,
    })
    expect(second.matches.map((item) => item.node.id)).toEqual(["1:3"])
    expect(second.nextCursor).toBeUndefined()
    await expect(
      searchNodes({
        scope: { pageId: requested.id },
        query: "Other",
        match: "contains",
        limit: 2,
        cursor,
      }),
    ).rejects.toMatchObject({ code: "INVALID_CURSOR" })
  })

  test("returns a continuation without scanning the whole tree after reaching the limit", async () => {
    const requested = page(
      "0:1",
      "Property",
      Array.from({ length: 10_000 }, (_, index) =>
        node(`1:${index}`, "Unrelated", "FRAME"),
      ),
    )
    installFigma({ currentPage: requested, pages: [requested] })

    const first = await searchNodes({
      scope: { pageId: requested.id },
      query: "Property",
      match: "exact",
      limit: 1,
    })

    expect(first.matches.map((item) => item.node.id)).toEqual(["0:1"])
    expect(first.nextCursor).toBeString()
    expect(first.truncated).toBe(false)
  })

  test("does not reload the already-current page before searching", async () => {
    const requested = page("0:1", "Current", [])
    requested.loadAsync = async () => {
      throw new Error("current page should already be loaded")
    }
    installFigma({ currentPage: requested, pages: [requested] })

    const result = await searchNodes({
      scope: { pageId: requested.id },
      types: ["PAGE"],
      match: "contains",
      limit: 1,
    })

    expect(result.matches.map((item) => item.node.id)).toEqual(["0:1"])
  })

  test("uses the already-current page without looking it up again", async () => {
    const requested = page("0:1", "Current", [])
    installFigma({
      currentPage: requested,
      pages: [requested],
      getNodeByIdAsync: async () => {
        throw new Error("current page should not be looked up")
      },
    })

    const result = await searchNodes({
      scope: { pageId: requested.id },
      types: ["PAGE"],
      match: "contains",
      limit: 1,
    })

    expect(result.matches.map((item) => item.node.id)).toEqual(["0:1"])
  })

  test("checks a matching node before reading its dynamic children", async () => {
    const requested = page("0:1", "Current", [])
    Object.defineProperty(requested, "children", {
      get() {
        throw new Error("children should not be read after the limit is met")
      },
    })
    installFigma({ currentPage: requested, pages: [requested] })

    const result = await searchNodes({
      scope: { pageId: requested.id },
      types: ["PAGE"],
      match: "contains",
      limit: 1,
    })

    expect(result.matches.map((item) => item.node.id)).toEqual(["0:1"])
  })

  test("searches one explicit page in document order without changing the current page", async () => {
    const leaf = node("1:3", "Card Title", "TEXT", { characters: "Pay now" })
    const child = node("1:2", "Card", "FRAME", { children: [leaf] })
    const requested = page("0:1", "Requested", [child])
    const current = page("0:2", "Current", [node("9:9", "Card", "FRAME")])
    Object.assign(leaf, { parent: child })
    Object.assign(child, { parent: requested })
    const { currentPage } = installFigma({
      currentPage: current,
      pages: [requested, current],
    })

    const result = await searchNodes({
      scope: { pageId: requested.id },
      query: "Card",
      match: "contains",
      limit: 50,
    })

    expect(result.matches.map((match) => match.node.id)).toEqual(["1:2", "1:3"])
    expect(result.matches[0]?.reasons).toEqual(["name"])
    expect(result.truncated).toBe(false)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(currentPage).toBe(current)
  })

  test("searches one explicit node scope and loads a page node without widening", async () => {
    const other = node("2:1", "Card", "FRAME")
    const target = node("1:1", "Card", "RECTANGLE")
    const requested = page("0:1", "Requested", [target])
    const current = page("0:2", "Current", [other])
    installFigma({
      currentPage: current,
      pages: [requested, current],
      nodes: new Map<string, unknown>([
        [target.id, target],
        [requested.id, requested],
        [other.id, other],
      ]),
    })

    const nodeResult = await searchNodes({
      scope: { nodeId: target.id },
      types: ["RECTANGLE"],
      match: "contains",
      limit: 50,
    })
    const pageAsNode = await searchNodes({
      scope: { nodeId: requested.id },
      query: "Card",
      match: "exact",
      limit: 50,
    })

    expect(nodeResult.matches.map((match) => match.node.id)).toEqual([
      target.id,
    ])
    expect(pageAsNode.matches.map((match) => match.node.id)).toEqual([
      target.id,
    ])
    expect(pageAsNode.matches.some((match) => match.node.id === other.id)).toBe(
      false,
    )
  })

  test("fails missing explicit roots without falling back to the current page", async () => {
    const current = page("0:2", "Current", [node("1:1", "Card", "FRAME")])
    const rectangle = node("1:9", "Not a page", "RECTANGLE")
    installFigma({
      currentPage: current,
      pages: [current],
      nodes: new Map<string, unknown>([[rectangle.id, rectangle]]),
    })

    await expect(
      searchNodes({
        scope: { pageId: "missing" },
        query: "Card",
        match: "contains",
        limit: 50,
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      searchNodes({
        scope: { pageId: rectangle.id },
        query: "Card",
        match: "contains",
        limit: 50,
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      searchNodes({
        scope: { nodeId: "missing" },
        query: "Card",
        match: "contains",
        limit: 50,
      }),
    ).rejects.toMatchObject({ code: "NODE_NOT_FOUND" })
    expect(PluginReadError).toBeDefined()
  })

  test("fails when node lookup is unavailable", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
    }

    await expect(
      searchNodes({
        scope: { pageId: "0:2" },
        query: "Card",
        match: "contains",
        limit: 50,
      }),
    ).rejects.toMatchObject({ code: "CAPABILITY_UNAVAILABLE" })
  })

  test("bounds visited and returned matches and reports truncation", async () => {
    const children = Array.from({ length: 3 }, (_, index) =>
      node(`1:${index + 1}`, "Card", "FRAME"),
    )
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    const result = await searchNodes(
      {
        scope: { pageId: requested.id },
        query: "Card",
        match: "contains",
        limit: 50,
      },
      undefined,
      { returnedNodes: 1, visitedNodes: 4, encodedBytes: 8 * 1024 * 1024 },
    )

    expect(result.matches).toHaveLength(1)
    expect(result.matches[0]?.node.id).toBe("1:1")
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: expect.any(Number),
    })
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      node(`1:${index + 1}`, "Item", "FRAME"),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return node("1:51", "Item", "FRAME")
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    await expect(
      searchNodes(
        {
          scope: { pageId: requested.id },
          query: "Item",
          match: "contains",
          limit: 50,
        },
        cancellation.signal,
      ),
    ).rejects.toThrow("Operation cancelled")
  })
})
