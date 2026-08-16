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

const nameTerm = (
  value: string,
  mode: "exact" | "contains" = "contains",
  caseSensitive?: boolean,
) =>
  caseSensitive === undefined ? { value, mode } : { value, mode, caseSensitive }

describe("search predicate", () => {
  test("requires at least one non-empty predicate member and keeps caller mode", () => {
    expect(() => compilePredicate({})).toThrow(/name, nodeTypes, or text/)
    expect(() =>
      compilePredicate({ name: nameTerm("   ", "contains") }),
    ).toThrow(/non-empty/)
    expect(() => compilePredicate({ text: nameTerm("", "exact") })).toThrow(
      /non-empty/,
    )
    expect(() => compilePredicate({ nodeTypes: [] })).toThrow(
      /name, nodeTypes, or text/,
    )
    expect(() => compilePredicate({ nodeTypes: ["   "] })).toThrow(/non-empty/)
    expect(compilePredicate({ nodeTypes: ["FRAME "] })).toEqual({
      nodeTypes: ["FRAME"],
    })
    expect(
      compilePredicate({
        name: nameTerm("  Card  ", "exact", true),
      }),
    ).toEqual({
      name: { value: "Card", mode: "exact", caseSensitive: true },
    })
  })

  test("matches exact and contains name predicates with case sensitivity", () => {
    const card = node("1:1", "Card", "FRAME")
    const titled = node("1:2", "Card Title", "FRAME")

    const exact: SearchPredicate = {
      name: { value: "Card", mode: "exact" },
    }
    const contains: SearchPredicate = {
      name: { value: "card", mode: "contains" },
    }
    const sensitive: SearchPredicate = {
      name: { value: "card", mode: "contains", caseSensitive: true },
    }

    expect(matchReasons(card, exact)).toEqual(["name"])
    expect(matchReasons(titled, exact)).toEqual([])
    expect(matchReasons(card, contains)).toEqual(["name"])
    expect(matchReasons(titled, contains)).toEqual(["name"])
    expect(matchReasons(card, sensitive)).toEqual([])
  })

  test("emits every applicable reason once and requires every provided predicate", () => {
    const pay = node("1:3", "Pay now", "TEXT", { characters: "Pay now" })
    const frame = node("1:4", "Pay now", "FRAME")

    const predicate: SearchPredicate = {
      name: { value: "Pay now", mode: "exact" },
      nodeTypes: ["TEXT", "TEXT"],
      text: { value: "Pay", mode: "contains" },
    }

    expect(matchReasons(pay, predicate)).toEqual(["name", "nodeType", "text"])
    expect(matchReasons(frame, predicate)).toEqual([])
  })

  test("reads text characters only from text-bearing nodes and never loads fonts", () => {
    const throwingText = node("1:5", "Label", "TEXT")
    Object.defineProperty(throwingText, "characters", {
      get() {
        throw new Error("font not loaded")
      },
    })
    const frame = node("1:6", "Pay", "FRAME", { characters: "Pay" })
    const text = node("1:7", "Label", "TEXT", { characters: "Pay now" })
    const predicate: SearchPredicate = {
      text: { value: "Pay", mode: "contains" },
    }

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
      query: { name: nameTerm("Card") },
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
      query: { nodeTypes: ["RECTANGLE"] },
    })
    const pageAsNode = await searchNodes({
      scope: { nodeId: requested.id },
      query: { name: nameTerm("Card", "exact") },
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
        scope: { pageId: "0:1" },
        query: { name: nameTerm("Card") },
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      searchNodes({
        scope: { pageId: rectangle.id },
        query: { name: nameTerm("Card") },
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      searchNodes({
        scope: { nodeId: "missing" },
        query: { name: nameTerm("Card") },
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
        scope: { pageId: "0:1" },
        query: { name: nameTerm("Card") },
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
        query: { name: nameTerm("Card") },
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
          query: { name: nameTerm("Item") },
        },
        cancellation.signal,
      ),
    ).rejects.toThrow("Operation cancelled")
  })
})
