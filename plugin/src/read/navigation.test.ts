import { beforeEach, describe, expect, test } from "bun:test"
import {
  readDesignContext,
  readMetadata,
  readNodes,
  readSelection,
} from "./navigation"
import { MAX_RETURNED_NODES } from "../shared/limits"
import { parseReadResult } from "../shared/result-validation"

const page = (id: string, name: string) => ({ id, name })

beforeEach(() => {
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
    root: {
      name: "Checkout flow",
      children: [page("0:1", "Page 1"), page("0:2", "Page 2")],
    },
    currentPage: page("0:2", "Page 2"),
    editorType: "dev",
    fileKey: undefined,
  }
})

describe("metadata reader", () => {
  test("reads file and page identity without traversing descendants or changing page", () => {
    const result = readMetadata()
    expect(result.file).toEqual({ name: "Checkout flow", editorType: "dev" })
    expect(result.pages).toEqual([
      { id: "0:1", name: "Page 1" },
      { id: "0:2", name: "Page 2" },
    ])
    expect(result.currentPageId).toBe("0:2")
    expect(result.pluginVersion).toBe("0.1.0")
    expect(result.truncated).toBe(false)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("bounds page metadata and reports page truncation", () => {
    const pages = Array.from({ length: MAX_RETURNED_NODES + 1 }, (_, index) =>
      page(`0:${index}`, `Page ${index}`),
    )
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Large file", children: pages },
      currentPage: pages[0],
      editorType: "dev",
    }

    const result = readMetadata()
    expect(result.pages).toHaveLength(MAX_RETURNED_NODES)
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: MAX_RETURNED_NODES + 1,
    })
  })
})

describe("selection reader", () => {
  test("returns an empty successful forest when nothing is selected", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Page 1", selection: [] },
      editorType: "dev",
      getNodeByIdAsync: async () => null,
    }

    await expect(
      readSelection({ detail: "minimal", depth: 0 }),
    ).resolves.toMatchObject({
      detail: "minimal",
      nodes: [],
      truncated: false,
    })
  })

  test("fails the call when a non-empty selection cannot be looked up", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: {
        id: "0:1",
        name: "Page 1",
        selection: [{ id: "1:1", name: "Card", type: "RECTANGLE" }],
      },
      editorType: "dev",
    }

    await expect(
      readSelection({ detail: "minimal", depth: 0 }),
    ).rejects.toMatchObject({ code: "CAPABILITY_UNAVAILABLE" })
  })

  test("captures selected IDs before the first lookup await", async () => {
    let releaseLookup: (() => void) | undefined
    const lookupStarted = new Promise<void>((resolve) => {
      releaseLookup = resolve
    })
    const selected = {
      id: "1:1",
      name: "Original",
      type: "RECTANGLE",
      children: [],
    }
    const replacement = {
      id: "1:2",
      name: "Replacement",
      type: "RECTANGLE",
      children: [],
    }
    const currentPage = { id: "0:1", name: "Page 1", selection: [selected] }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [currentPage] },
      currentPage,
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => {
        await lookupStarted
        return id === selected.id ? selected : null
      },
    }

    const reading = readSelection({ detail: "minimal", depth: 0 })
    currentPage.selection = [replacement]
    releaseLookup?.()

    await expect(reading).resolves.toMatchObject({
      detail: "minimal",
      nodes: [{ summary: { id: "1:1", name: "Original" } }],
      truncated: false,
    })
  })

  test("applies requested detail and depth to the captured selection", async () => {
    const child = {
      id: "1:2",
      name: "Child",
      type: "TEXT",
      characters: "hello",
      children: [],
    }
    const selected = {
      id: "1:1",
      name: "Container",
      type: "FRAME",
      layoutMode: "HORIZONTAL",
      children: [child],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Page 1", selection: [selected] },
      editorType: "dev",
      getNodeByIdAsync: async () => selected,
    }

    await expect(
      readSelection({ detail: "compact", depth: 0 }),
    ).resolves.toMatchObject({
      detail: "compact",
      nodes: [
        {
          data: { autoLayout: { mode: "horizontal" } },
          children: [],
          childrenTruncated: true,
          childrenTruncation: { reason: "depthLimit", appliedDepth: 0 },
        },
      ],
      truncated: true,
      truncation: { reason: "depthLimit", appliedDepth: 0 },
    })
  })

  test("a hidden selected node is refused, and its visible sibling still comes back", async () => {
    // GetSelectionResult has no per-item error slot, so a switched-off
    // selected root is excluded rather than reported — but it must not sink
    // the rest of the selection, same as a lookup miss does not.
    const hidden = {
      id: "1:1",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const shown = {
      id: "1:2",
      name: "Shown",
      type: "FRAME",
      visible: true,
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: {
        id: "0:1",
        name: "Page 1",
        selection: [hidden, shown],
      },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : id === shown.id ? shown : null,
    }

    await expect(
      readSelection({ detail: "minimal", depth: 0 }),
    ).resolves.toMatchObject({
      nodes: [{ summary: { id: shown.id, name: "Shown" } }],
      truncated: false,
    })
  })

  test("get_selection reports an unwalkable root and stays silent about a hidden one", async () => {
    const hidden = {
      id: "1:2",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const looped: Record<string, unknown> = {
      id: "1:3",
      name: "Looped",
      type: "FRAME",
      visible: true,
      children: [],
    }
    looped.parent = looped
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: {
        id: "0:1",
        name: "Page 1",
        selection: [hidden, looped],
      },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : id === looped.id ? looped : null,
    }

    const result = await readSelection({}, undefined)

    expect(result.nodes).toHaveLength(0)
    expect(result.unresolved).toEqual([
      {
        id: "1:3",
        error: {
          code: "LIMIT_EXCEEDED",
          message: "The operation exceeded a safety limit.",
          retryable: false,
        },
      },
    ])
  })

  test("an oversized unresolved list is truncated, not thrown on", async () => {
    // Every node here loops onto itself, so every one is `undetermined` and
    // wants an `unresolved` entry. `parseUnresolved` bounds that list at
    // MAX_RETURNED_NODES, so without a cap on the producing side the whole
    // read — the one this field exists to keep alive — would fail validation.
    const loopedSelection = (count: number): Record<string, unknown>[] =>
      Array.from({ length: count }, (_, index) => {
        const node: Record<string, unknown> = {
          id: `1:${index + 1}`,
          name: "Looped",
          type: "FRAME",
          visible: true,
          children: [],
        }
        node.parent = node
        return node
      })

    const install = (selection: Record<string, unknown>[]): void => {
      const byId = new Map(
        selection.map((node) => [node.id as string, node] as const),
      )
      ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
        root: { name: "Checkout flow", children: [] },
        currentPage: { id: "0:1", name: "Page 1", selection },
        editorType: "dev",
        getNodeByIdAsync: async (id: string) => byId.get(id) ?? null,
      }
    }

    install(loopedSelection(MAX_RETURNED_NODES))
    const atLimit = await readSelection({}, undefined)
    expect(atLimit.unresolved).toHaveLength(MAX_RETURNED_NODES)
    expect(() =>
      parseReadResult({ operation: "get_selection", result: atLimit }),
    ).not.toThrow()

    install(loopedSelection(MAX_RETURNED_NODES + 1))
    const overLimit = await readSelection({}, undefined)
    expect(overLimit.unresolved).toHaveLength(MAX_RETURNED_NODES)
    // The point of the cap: the extra node costs its own entry, not the read.
    expect(() =>
      parseReadResult({ operation: "get_selection", result: overLimit }),
    ).not.toThrow()
  })

  test("a clean selection omits unresolved entirely", async () => {
    const live = {
      id: "1:2",
      name: "Live",
      type: "FRAME",
      visible: true,
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: {
        id: "0:1",
        name: "Page 1",
        selection: [live],
      },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => (id === live.id ? live : null),
    }

    const result = await readSelection({}, undefined)

    expect(Object.hasOwn(result, "unresolved")).toBe(false)
  })
})

describe("node reader", () => {
  test("preserves requested order and represents missing nodes as item errors", async () => {
    const first = { id: "1:1", name: "First", type: "RECTANGLE", children: [] }
    const last = { id: "1:3", name: "Last", type: "TEXT", children: [] }
    const lookedUp: string[] = []
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => {
        lookedUp.push(id)
        return id === first.id ? first : id === last.id ? last : null
      },
    }

    const result = await readNodes({
      nodeIds: [last.id, "1:2", first.id],
      detail: "minimal",
      depth: 0,
    })

    expect(lookedUp).toEqual([last.id, "1:2", first.id])
    expect(result).toMatchObject({
      detail: "minimal",
      items: [
        {
          status: "success",
          value: { summary: { id: last.id, name: "Last" } },
        },
        {
          status: "error",
          error: {
            code: "NODE_NOT_FOUND",
            message: "The requested node was not found.",
            retryable: false,
          },
        },
        {
          status: "success",
          value: { summary: { id: first.id, name: "First" } },
        },
      ],
      truncated: false,
    })
  })

  test("returns an empty successful batch for no node IDs", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => {
        throw new Error("getNodeByIdAsync must not be called")
      },
    }

    await expect(
      readNodes({ nodeIds: [], detail: "full" }),
    ).resolves.toMatchObject({
      detail: "full",
      items: [],
      truncated: false,
    })
  })

  test("passes requested detail and depth to each found node", async () => {
    const child = { id: "1:2", name: "Child", type: "RECTANGLE", children: [] }
    const node = {
      id: "1:1",
      name: "Container",
      type: "FRAME",
      layoutMode: "HORIZONTAL",
      children: [child],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => node,
    }

    await expect(
      readNodes({ nodeIds: [node.id], detail: "compact", depth: 0 }),
    ).resolves.toMatchObject({
      detail: "compact",
      items: [
        {
          status: "success",
          value: {
            data: { autoLayout: { mode: "horizontal" } },
            children: [],
            childrenTruncated: true,
            childrenTruncation: { reason: "depthLimit", appliedDepth: 0 },
          },
        },
      ],
      truncated: true,
      truncation: { reason: "depthLimit", appliedDepth: 0 },
    })
  })

  test("maps a throwing getNodeByIdAsync lookup to NODE_NOT_FOUND", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => {
        throw new Error("invalid node id")
      },
    }

    const result = await readNodes({
      nodeIds: ["00:00000"],
      detail: "minimal",
      depth: 0,
    })
    expect(result.items[0]).toMatchObject({
      status: "error",
      error: { code: "NODE_NOT_FOUND", retryable: false },
    })
  })

  test("loads document pages before serializing get_nodes", async () => {
    let loaded = 0
    const frame = { id: "1:1", name: "Frame", type: "FRAME", children: [] }
    const pageNode = {
      id: "0:1",
      name: "Page",
      type: "PAGE",
      loadAsync: async () => {
        loaded += 1
      },
    }
    Object.defineProperty(pageNode, "children", {
      configurable: true,
      enumerable: true,
      get() {
        if (loaded === 0) throw new Error("unloaded page")
        return [frame]
      },
    })
    const document = {
      id: "0:0",
      name: "Doc",
      type: "DOCUMENT",
      children: [pageNode],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [pageNode] },
      currentPage: page("0:2", "Current"),
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === document.id ? document : null,
    }

    const result = await readNodes({
      nodeIds: [document.id],
      detail: "minimal",
      depth: 1,
    })
    expect(loaded).toBe(1)
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        summary: { id: "0:0", nodeType: "DOCUMENT" },
        children: [{ summary: { id: "0:1", nodeType: "PAGE" } }],
      },
    })
  })

  test("resolves instance compact data via getMainComponentAsync under dynamic-page", async () => {
    const instance = {
      id: "4:1",
      name: "Calendar pill",
      type: "INSTANCE",
      children: [],
      componentProperties: {
        "Label#0:1": { type: "TEXT", value: "Today" },
      },
      getMainComponentAsync: async () => ({
        id: "2:1",
        type: "COMPONENT",
        parent: { id: "2:0", type: "COMPONENT_SET" },
      }),
    }
    Object.defineProperty(instance, "mainComponent", {
      configurable: true,
      enumerable: true,
      get() {
        throw new Error("mainComponent is write-only under dynamic-page")
      },
    })
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => instance,
    }

    const result = await readNodes({
      nodeIds: [instance.id],
      detail: "compact",
      depth: 0,
    })

    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        summary: { id: "4:1", nodeType: "INSTANCE" },
        data: {
          instance: {
            componentId: "2:1",
            componentSetId: "2:0",
            properties: [
              { name: "Label#0:1", value: { kind: "text", value: "Today" } },
            ],
          },
        },
      },
    })

    // Push the result back through the same validator that guards the wire
    // boundary, so a populated properties array is actually exercised by it.
    const validated = parseReadResult({ operation: "get_nodes", result })
    expect(validated.operation).toBe("get_nodes")
    const validatedResult = validated.result as {
      items: readonly {
        status: string
        value?: { data?: { instance?: { properties?: unknown } } }
      }[]
    }
    expect(validatedResult.items[0]?.value?.data?.instance?.properties).toEqual(
      [{ name: "Label#0:1", value: { kind: "text", value: "Today" } }],
    )
  })

  test("resolves style names via getStyleByIdAsync at detail full", async () => {
    const node = {
      id: "1:1",
      name: "Card",
      type: "RECTANGLE",
      children: [],
      fillStyleId: "S:fill",
    }
    const lookups: string[] = []
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => node,
      getStyleByIdAsync: async (id: string) => {
        lookups.push(id)
        return { name: "Primary/500" }
      },
    }

    const result = await readNodes({
      nodeIds: [node.id],
      detail: "full",
      depth: 0,
    })

    // The resolved name must travel the real readNodes path (navigation.ts's
    // detail === "full" gate into collectStyleNames), not a direct
    // serializeNodeForest call.
    expect(lookups).toEqual(["S:fill"])
    expect(result.items[0]).toMatchObject({
      status: "success",
      value: {
        data: {
          styleReferences: [
            { id: "S:fill", kind: "paint", name: "Primary/500" },
          ],
        },
      },
    })
  })

  test("spends zero style lookups and omits name at detail compact", async () => {
    const node = {
      id: "1:1",
      name: "Card",
      type: "RECTANGLE",
      children: [],
      fillStyleId: "S:fill",
    }
    let calls = 0
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => node,
      getStyleByIdAsync: async (_id: string) => {
        calls += 1
        return { name: "Primary/500" }
      },
    }

    const result = await readNodes({
      nodeIds: [node.id],
      detail: "compact",
      depth: 0,
    })

    // Zero calls, not merely an absent name: an empty-but-threaded styleNames
    // map would still leave `name` absent while still spending the lookup.
    expect(calls).toBe(0)
    const item = result.items[0] as {
      status: string
      value?: { data?: { styleReferences?: Record<string, unknown>[] } }
    }
    expect(item.status).toBe("success")
    expect(item.value?.data?.styleReferences).toEqual([
      { id: "S:fill", kind: "paint" },
    ])
    expect(
      Object.hasOwn(item.value?.data?.styleReferences?.[0] ?? {}, "name"),
    ).toBe(false)
  })

  test("resolves variable names via figma.variables at detail full", async () => {
    const node = {
      id: "1:1",
      name: "Card",
      type: "RECTANGLE",
      children: [],
      boundVariables: {
        fills: [{ type: "VARIABLE_ALIAS", id: "V:a" }],
      },
    }
    const lookups: string[] = []
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => node,
      variables: {
        getVariableByIdAsync: async (id: string) => {
          lookups.push(id)
          return { name: "text/primary" }
        },
      },
    }

    const result = await readNodes({
      nodeIds: [node.id],
      detail: "full",
      depth: 0,
    })

    expect(lookups).toEqual(["V:a"])
    const item = result.items[0] as {
      status: string
      value?: { data?: { variableReferences?: Record<string, unknown>[] } }
    }
    expect(item.value?.data?.variableReferences).toEqual([
      { id: "V:a", name: "text/primary" },
    ])
  })

  test("spends zero variable lookups and omits name at detail compact", async () => {
    const node = {
      id: "1:1",
      name: "Card",
      type: "RECTANGLE",
      children: [],
      boundVariables: {
        fills: [{ type: "VARIABLE_ALIAS", id: "V:a" }],
      },
    }
    let calls = 0
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => node,
      variables: {
        getVariableByIdAsync: async (_id: string) => {
          calls += 1
          return { name: "text/primary" }
        },
      },
    }

    const result = await readNodes({
      nodeIds: [node.id],
      detail: "compact",
      depth: 0,
    })

    // Asserting zero calls, not merely that `name` is absent: threading an empty
    // map with the detail gate removed would produce an identical payload while
    // still spending the lookups.
    expect(calls).toBe(0)
    const item = result.items[0] as {
      status: string
      value?: { data?: { variableReferences?: Record<string, unknown>[] } }
    }
    expect(item.value?.data?.variableReferences).toEqual([{ id: "V:a" }])
    expect(
      Object.hasOwn(item.value?.data?.variableReferences?.[0] ?? {}, "name"),
    ).toBe(false)
  })

  test("a missing figma.variables API leaves names absent without failing the read", async () => {
    const node = {
      id: "1:1",
      name: "Card",
      type: "RECTANGLE",
      children: [],
      boundVariables: {
        fills: [{ type: "VARIABLE_ALIAS", id: "V:a" }],
      },
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async () => node,
    }

    const result = await readNodes({
      nodeIds: [node.id],
      detail: "full",
      depth: 0,
    })

    const item = result.items[0] as {
      status: string
      value?: { data?: { variableReferences?: Record<string, unknown>[] } }
    }
    expect(item.value?.data?.variableReferences).toEqual([{ id: "V:a" }])
  })

  test("an explicitly named hidden node is refused, and its siblings still succeed", async () => {
    // Per-item: one switched-off node must not cost the caller the whole batch.
    const hidden = {
      id: "1:2",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const shown = {
      id: "1:3",
      name: "Shown",
      type: "FRAME",
      visible: true,
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : id === shown.id ? shown : null,
    }

    const result = await readNodes({
      nodeIds: [hidden.id, shown.id],
      detail: "minimal",
      depth: 0,
    })

    // The message is asserted literally, not merely the code: `nodeError`
    // sources it from `CANONICAL_MESSAGES` rather than a copy of its own, and
    // this is the test that would catch the two drifting.
    expect(result.items[0]).toMatchObject({
      status: "error",
      error: {
        code: "NODE_NOT_VISIBLE",
        message:
          "The requested node exists but is switched off, so it renders nothing.",
        retryable: false,
      },
    })
    expect(result.items[1]).toMatchObject({ status: "success" })
  })

  test("a switched-on node inside a switched-off parent is refused too", async () => {
    // Its own `visible` is true; only the ancestor walk catches this.
    const hiddenParent = {
      id: "1:4",
      name: "Group",
      type: "FRAME",
      visible: false,
    }
    const child = {
      id: "1:5",
      name: "Deep",
      type: "FRAME",
      visible: true,
      children: [],
      parent: hiddenParent,
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => (id === child.id ? child : null),
    }

    const result = await readNodes({
      nodeIds: [child.id],
      detail: "minimal",
      depth: 0,
    })

    expect(result.items[0]).toMatchObject({
      status: "error",
      error: {
        code: "NODE_NOT_VISIBLE",
        message:
          "The requested node exists but is switched off, so it renders nothing.",
        retryable: false,
      },
    })
  })

  test("get_nodes tells a switched-off node apart from an unwalkable one", async () => {
    const hidden = {
      id: "1:2",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const looped: Record<string, unknown> = {
      id: "1:3",
      name: "Looped",
      type: "FRAME",
      visible: true,
      children: [],
    }
    looped.parent = looped
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === "1:2" ? hidden : id === "1:3" ? looped : null,
    }

    const result = await readNodes({
      nodeIds: ["1:2", "1:3"],
      detail: "minimal",
      depth: 0,
    })

    expect(result.items[0]).toMatchObject({
      status: "error",
      error: { code: "NODE_NOT_VISIBLE", retryable: false },
    })
    expect(result.items[1]).toMatchObject({
      status: "error",
      error: { code: "LIMIT_EXCEEDED", retryable: false },
    })
  })
})

describe("design context reader", () => {
  test("defaults to the current page and applies requested detail, depth, and component dedupe", async () => {
    const child = { id: "1:2", name: "Child", type: "RECTANGLE", children: [] }
    const currentPage = {
      id: "0:1",
      name: "Current page",
      type: "PAGE",
      children: [child],
      selection: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [currentPage] },
      currentPage,
      editorType: "dev",
      getNodeByIdAsync: async () => {
        throw new Error("default current-page reads must not look up the page")
      },
    }

    await expect(
      readDesignContext({
        detail: "minimal",
        depth: 0,
        dedupeComponents: true,
      }),
    ).resolves.toMatchObject({
      detail: "minimal",
      roots: [
        {
          summary: { id: "0:1", name: "Current page", nodeType: "PAGE" },
          children: [],
          childrenTruncated: true,
          childrenTruncation: { reason: "depthLimit", appliedDepth: 0 },
        },
      ],
      truncated: true,
      truncation: { reason: "depthLimit", appliedDepth: 0 },
    })
  })

  test("loads an explicit page without changing the current page", async () => {
    let loaded = 0
    const currentPage = {
      id: "0:2",
      name: "Current",
      type: "PAGE",
      children: [],
    }
    const requestedPage = {
      id: "0:1",
      name: "Requested",
      type: "PAGE",
      children: [],
      loadAsync: async () => {
        loaded += 1
      },
    }
    const api = {
      root: { name: "Checkout flow", children: [requestedPage, currentPage] },
      currentPage,
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === requestedPage.id ? requestedPage : null,
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = api

    await expect(
      readDesignContext({
        selector: { pageId: requestedPage.id },
        detail: "minimal",
        dedupeComponents: false,
      }),
    ).resolves.toMatchObject({
      roots: [
        {
          summary: {
            id: requestedPage.id,
            name: "Requested",
            nodeType: "PAGE",
          },
        },
      ],
      truncated: false,
    })
    expect(loaded).toBe(1)
    expect(api.currentPage).toBe(currentPage)
  })

  test("loads a PAGE passed as nodeId without changing the current page", async () => {
    let loaded = 0
    const currentPage = {
      id: "0:2",
      name: "Current",
      type: "PAGE",
      children: [],
    }
    const requestedPage = {
      id: "0:1",
      name: "Requested",
      type: "PAGE",
      children: [{ id: "1:1", name: "Child", type: "FRAME", children: [] }],
      loadAsync: async () => {
        loaded += 1
      },
    }
    const api = {
      root: { name: "Checkout flow", children: [requestedPage, currentPage] },
      currentPage,
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === requestedPage.id ? requestedPage : null,
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = api

    const result = await readDesignContext({
      selector: { nodeId: requestedPage.id },
      detail: "minimal",
      depth: 1,
      dedupeComponents: false,
    })
    expect(loaded).toBe(1)
    expect(api.currentPage).toBe(currentPage)
    expect(result.roots[0]?.children.map((child) => child.summary.id)).toEqual([
      "1:1",
    ])
  })

  test("preserves each supported selector's requested root order", async () => {
    const selection = {
      id: "1:1",
      name: "Selected",
      type: "RECTANGLE",
      children: [],
    }
    const first = { id: "1:2", name: "First", type: "RECTANGLE", children: [] }
    const last = { id: "1:3", name: "Last", type: "RECTANGLE", children: [] }
    const firstPage = {
      id: "0:1",
      name: "First page",
      type: "PAGE",
      children: [],
      loadAsync: async () => {},
    }
    const lastPage = {
      id: "0:2",
      name: "Last page",
      type: "PAGE",
      children: [],
      loadAsync: async () => {},
    }
    const nodes = new Map([
      [selection.id, selection],
      [first.id, first],
      [last.id, last],
      [firstPage.id, firstPage],
      [lastPage.id, lastPage],
    ])
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [firstPage, lastPage] },
      currentPage: {
        id: "0:3",
        name: "Current",
        type: "PAGE",
        children: [],
        selection: [selection],
      },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => nodes.get(id) ?? null,
    }

    const selectionResult = await readDesignContext({
      selector: { selection: true },
      detail: "minimal",
      dedupeComponents: false,
    })
    const nodeResult = await readDesignContext({
      selector: { nodeIds: [last.id, first.id] },
      detail: "minimal",
      dedupeComponents: false,
    })
    const pageResult = await readDesignContext({
      selector: { pageIds: [lastPage.id, firstPage.id] },
      detail: "minimal",
      dedupeComponents: false,
    })

    expect(selectionResult.roots.map((node) => node.summary.id)).toEqual([
      selection.id,
    ])
    expect(nodeResult.roots.map((node) => node.summary.id)).toEqual([
      last.id,
      first.id,
    ])
    expect(pageResult.roots.map((node) => node.summary.id)).toEqual([
      lastPage.id,
      firstPage.id,
    ])
  })

  test("distinguishes missing and non-page explicit roots", async () => {
    const nonPage = {
      id: "1:1",
      name: "Rectangle",
      type: "RECTANGLE",
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Current", type: "PAGE", children: [] },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === nonPage.id ? nonPage : null,
    }

    await expect(
      readDesignContext({
        selector: { nodeId: "missing" },
        dedupeComponents: false,
      }),
    ).rejects.toMatchObject({ code: "NODE_NOT_FOUND" })
    await expect(
      readDesignContext({
        selector: { pageId: "missing" },
        dedupeComponents: false,
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    await expect(
      readDesignContext({
        selector: { pageId: nonPage.id },
        dedupeComponents: false,
      }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
  })

  test("excludes a hidden root named by nodeId, rather than reporting or refusing it", async () => {
    // GetDesignContextResult is a bare NodeForest with no per-item error
    // slot, so a switched-off root is excluded, same as get_selection.
    const hidden = {
      id: "1:1",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Current", type: "PAGE", children: [] },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : null,
    }

    const result = await readDesignContext({
      selector: { nodeId: hidden.id },
      dedupeComponents: false,
    })

    expect(result.roots).toEqual([])
    // A switched-off root must not slip into `unresolved` — it is excluded,
    // not reported. Collapsing the hidden/undetermined guards on this branch
    // would leave `roots` empty (satisfying the assertion above) while still
    // adding an entry here, so this is the only thing that catches that leak.
    expect(Object.hasOwn(result, "unresolved")).toBe(false)
  })

  test("get_design_context reports an unwalkable selector root", async () => {
    const looped: Record<string, unknown> = {
      id: "1:3",
      name: "Looped",
      type: "FRAME",
      visible: true,
      children: [],
    }
    looped.parent = looped
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Current", type: "PAGE", children: [] },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === looped.id ? looped : null,
    }

    const result = await readDesignContext(
      { selector: { nodeId: "1:3" }, dedupeComponents: false },
      undefined,
    )

    expect(result.roots).toHaveLength(0)
    expect(result.unresolved).toEqual([
      {
        id: "1:3",
        error: {
          code: "LIMIT_EXCEEDED",
          message: "The operation exceeded a safety limit.",
          retryable: false,
        },
      },
    ])
  })

  test("excludes a hidden root from a selection, and its visible sibling still comes back", async () => {
    const hidden = {
      id: "1:2",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const shown = {
      id: "1:3",
      name: "Shown",
      type: "FRAME",
      visible: true,
      children: [],
    }
    const currentPage = {
      id: "0:1",
      name: "Current",
      type: "PAGE",
      children: [],
      selection: [hidden, shown],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [currentPage] },
      currentPage,
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : id === shown.id ? shown : null,
    }

    const result = await readDesignContext({
      selector: { selection: true },
      dedupeComponents: false,
    })

    expect(result.roots.map((node) => node.summary.id)).toEqual([shown.id])
    // Same leak this branch could otherwise let through: a switched-off root
    // must not add an `unresolved` entry alongside its excluded absence.
    expect(Object.hasOwn(result, "unresolved")).toBe(false)
  })

  test("get_design_context reports an unwalkable selection root, and stays silent about a hidden one", async () => {
    const hidden = {
      id: "1:2",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const looped: Record<string, unknown> = {
      id: "1:3",
      name: "Looped",
      type: "FRAME",
      visible: true,
      children: [],
    }
    looped.parent = looped
    const currentPage = {
      id: "0:1",
      name: "Current",
      type: "PAGE",
      children: [],
      selection: [hidden, looped],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [currentPage] },
      currentPage,
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : id === looped.id ? looped : null,
    }

    const result = await readDesignContext({
      selector: { selection: true },
      dedupeComponents: false,
    })

    expect(result.roots).toHaveLength(0)
    expect(result.unresolved).toEqual([
      {
        id: "1:3",
        error: {
          code: "LIMIT_EXCEEDED",
          message: "The operation exceeded a safety limit.",
          retryable: false,
        },
      },
    ])
  })

  test("excludes a root whose ancestor is switched off, and keeps its visible sibling in nodeIds", async () => {
    // Its own `visible` is true; only the ancestor walk catches this.
    const hiddenParent = {
      id: "1:4",
      name: "Group",
      type: "FRAME",
      visible: false,
    }
    const deep = {
      id: "1:5",
      name: "Deep",
      type: "FRAME",
      visible: true,
      children: [],
      parent: hiddenParent,
    }
    const shown = {
      id: "1:6",
      name: "Shown",
      type: "FRAME",
      visible: true,
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Current", type: "PAGE", children: [] },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === deep.id ? deep : id === shown.id ? shown : null,
    }

    const result = await readDesignContext({
      selector: { nodeIds: [deep.id, shown.id] },
      dedupeComponents: false,
    })

    expect(result.roots.map((node) => node.summary.id)).toEqual([shown.id])
    // Same leak this branch could otherwise let through: a switched-off root
    // must not add an `unresolved` entry alongside its excluded absence.
    expect(Object.hasOwn(result, "unresolved")).toBe(false)
  })

  test("get_design_context reports an unwalkable nodeIds root, and stays silent about a hidden one", async () => {
    const hidden = {
      id: "1:2",
      name: "Hidden",
      type: "FRAME",
      visible: false,
      children: [],
    }
    const looped: Record<string, unknown> = {
      id: "1:3",
      name: "Looped",
      type: "FRAME",
      visible: true,
      children: [],
    }
    looped.parent = looped
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Current", type: "PAGE", children: [] },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) =>
        id === hidden.id ? hidden : id === looped.id ? looped : null,
    }

    const result = await readDesignContext({
      selector: { nodeIds: [hidden.id, "1:3"] },
      dedupeComponents: false,
    })

    expect(result.roots).toHaveLength(0)
    expect(result.unresolved).toEqual([
      {
        id: "1:3",
        error: {
          code: "LIMIT_EXCEEDED",
          message: "The operation exceeded a safety limit.",
          retryable: false,
        },
      },
    ])
  })

  test("a clean design context selector omits unresolved entirely", async () => {
    const live = {
      id: "1:2",
      name: "Live",
      type: "FRAME",
      visible: true,
      children: [],
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Current", type: "PAGE", children: [] },
      editorType: "dev",
      getNodeByIdAsync: async (id: string) => (id === live.id ? live : null),
    }

    const result = await readDesignContext({
      selector: { nodeId: live.id },
      dedupeComponents: false,
    })

    // `toEqual` cannot tell an absent key apart from one present with value
    // `undefined`; only `Object.hasOwn` proves a healthy response stays
    // byte-identical to what it was before this field existed.
    expect(Object.hasOwn(result, "unresolved")).toBe(false)
  })
})
