import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getVariables } from "./variables"

function collection(options: {
  id: string
  name: string
  modes: { modeId: string; name: string }[]
  defaultModeId?: string
}) {
  return {
    id: options.id,
    name: options.name,
    modes: options.modes,
    defaultModeId: options.defaultModeId ?? options.modes[0]?.modeId,
  }
}

function variable(options: {
  id: string
  name: string
  collectionId: string
  valuesByMode: Record<string, unknown>
  scopes?: string[]
  codeSyntax?: Record<string, string>
}) {
  return {
    id: options.id,
    name: options.name,
    variableCollectionId: options.collectionId,
    valuesByMode: options.valuesByMode,
    scopes: options.scopes ?? ["ALL_SCOPES"],
    codeSyntax: options.codeSyntax ?? {},
  }
}

// A node that binds the given variable ids the way the host reports them: an
// array of aliases under `boundVariables.fills`.
function bound(id: string, variableIds: string[], children: unknown[] = []) {
  return {
    id,
    name: `Node ${id}`,
    type: "FRAME",
    children,
    boundVariables: {
      fills: variableIds.map((variableId) => ({
        type: "VARIABLE_ALIAS",
        id: variableId,
      })),
    },
  }
}

function findNode(raw: unknown, id: string): unknown {
  if (raw === null || typeof raw !== "object") return null
  const node = raw as { id?: unknown; children?: unknown }
  if (node.id === id) return raw
  const children = Array.isArray(node.children) ? node.children : []
  for (const child of children) {
    const found = findNode(child, id)
    if (found !== null) return found
  }
  return null
}

function installFigma(options: {
  /** Roots on the current page; these are what a scope walk can see. */
  nodes?: unknown[]
  /** Variables reachable by id — local or library, the caller cannot tell. */
  variables?: unknown[]
  /** Collections reachable by id. */
  collections?: unknown[]
  byId?: Map<string, unknown>
  collectionsById?: Map<string, unknown>
}): {
  variableLookups: string[]
  collectionLookups: string[]
} {
  const variableLookups: string[] = []
  const collectionLookups: string[] = []
  const byId = options.byId ?? new Map()
  const collectionsById = options.collectionsById ?? new Map()
  for (const item of options.variables ?? []) {
    const record = item as { id: string }
    if (!byId.has(record.id)) byId.set(record.id, item)
  }
  for (const item of options.collections ?? []) {
    const record = item as { id: string }
    if (!collectionsById.has(record.id)) collectionsById.set(record.id, item)
  }
  const currentPage = {
    id: "0:1",
    name: "Page 1",
    type: "PAGE",
    children: options.nodes ?? [],
  }
  const api = {
    root: { name: "Checkout flow", children: [currentPage] },
    currentPage,
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("variables must not call loadAllPagesAsync")
    },
    getNodeByIdAsync: async (id: string) => findNode(currentPage, id),
    variables: {
      // Local enumeration answers a different question than the caller asked;
      // these throw so a regression back to it fails loudly rather than
      // silently returning a set the scope does not bind.
      getLocalVariableCollectionsAsync: async () => {
        throw new Error("variables must not enumerate local collections")
      },
      getLocalVariablesAsync: async () => {
        throw new Error("variables must not enumerate local variables")
      },
      getVariableByIdAsync: async (id: string) => {
        variableLookups.push(id)
        return byId.get(id) ?? null
      },
      getVariableCollectionByIdAsync: async (id: string) => {
        collectionLookups.push(id)
        return collectionsById.get(id) ?? null
      },
    },
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { variableLookups, collectionLookups }
}

const idsOf = (result: { collections: { variables: { id: string }[] }[] }) =>
  result.collections.flatMap((item) => item.variables.map((v) => v.id))

describe("get_variables", () => {
  beforeEach(() => {
    installFigma({})
  })

  test("returns the variables bound in scope, including ones stored in a library", async () => {
    // The node binds a library variable. Local enumeration cannot see it, so
    // before this the tool returned nothing about the node the caller asked for.
    const libraryId = "VariableID:abc123/9:9"
    const libraryCollectionId = "VariableCollectionId:abc123/9:1"
    installFigma({
      nodes: [bound("1:1", [libraryId])],
      collections: [
        collection({
          id: libraryCollectionId,
          name: "Brand",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: libraryId,
          name: "brand/primary",
          collectionId: libraryCollectionId,
          valuesByMode: { "M:default": { r: 0, g: 0, b: 1 } },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(idsOf(result)).toContain(libraryId)
    expect(result.collections[0]?.name).toBe("Brand")
  })

  test("a different scope returns a different set", async () => {
    // Proves selector is live: before this every scope returned the same list.
    installFigma({
      nodes: [bound("1:1", ["V:a"]), bound("2:2", ["V:b"])],
      collections: [
        collection({
          id: "C:theme",
          name: "Theme",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:a",
          name: "a",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 1 },
        }),
        variable({
          id: "V:b",
          name: "b",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 2 },
        }),
      ],
    })

    const a = await getVariables({ selector: { nodeId: "1:1" } })
    const b = await getVariables({ selector: { nodeId: "2:2" } })

    expect(idsOf(a)).toEqual(["V:a"])
    expect(idsOf(b)).toEqual(["V:b"])
    expect(idsOf(a)).not.toEqual(idsOf(b))
  })

  test("a scope that binds nothing returns nothing", async () => {
    installFigma({
      nodes: [bound("1:1", []), bound("2:2", ["V:a"])],
      collections: [
        collection({
          id: "C:theme",
          name: "Theme",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:a",
          name: "a",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 1 },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(result.collections).toEqual([])
    expect(result.truncated).toBe(false)
  })

  test("collects ids from descendants and from nested bindings, once each", async () => {
    const child = {
      id: "1:2",
      name: "Child",
      type: "TEXT",
      children: [],
      boundVariables: {
        // Nested one level below boundVariables, the way componentProperties
        // reports them, and repeating an id the parent already bound.
        componentProperties: { Label: { type: "VARIABLE_ALIAS", id: "V:a" } },
        characters: { type: "VARIABLE_ALIAS", id: "V:c" },
      },
    }
    const { variableLookups } = installFigma({
      nodes: [bound("1:1", ["V:a"], [child])],
      collections: [
        collection({
          id: "C:theme",
          name: "Theme",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:a",
          name: "a",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 1 },
        }),
        variable({
          id: "V:c",
          name: "c",
          collectionId: "C:theme",
          valuesByMode: { "M:default": "text" },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(variableLookups).toEqual(["V:a", "V:c"])
    expect(idsOf(result)).toEqual(["V:a", "V:c"])
  })

  test("groups the bound variables by their own collection, resolved once per id", async () => {
    const { collectionLookups } = installFigma({
      nodes: [bound("1:1", ["V:a", "V:b", "V:z"])],
      collections: [
        collection({
          id: "C:theme",
          name: "Theme",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
        collection({
          id: "C:space",
          name: "Space",
          modes: [{ modeId: "M:base", name: "Base" }],
        }),
      ],
      variables: [
        variable({
          id: "V:a",
          name: "a",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 1 },
        }),
        variable({
          id: "V:z",
          name: "z",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 3 },
        }),
        variable({
          id: "V:b",
          name: "b",
          collectionId: "C:space",
          valuesByMode: { "M:base": 2 },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(result.collections.map((item) => item.id)).toEqual([
      "C:theme",
      "C:space",
    ])
    expect(result.collections[0]?.variables.map((v) => v.id)).toEqual([
      "V:a",
      "V:z",
    ])
    expect(collectionLookups).toEqual(["C:theme", "C:space"])
  })

  test("preserves collection and mode order, scopes, code syntax, and raw aliases", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [
        { modeId: "M:light", name: "Light" },
        { modeId: "M:dark", name: "Dark" },
      ],
    })
    const bg = variable({
      id: "V:bg",
      name: "color/bg",
      collectionId: "C:theme",
      scopes: ["ALL_FILLS", "FRAME_FILL"],
      codeSyntax: {
        WEB: "var(--bg)",
        ANDROID: "bg",
        iOS: "bgColor",
      },
      valuesByMode: {
        "M:light": { r: 1, g: 1, b: 1, a: 1 },
        "M:dark": { type: "VARIABLE_ALIAS", id: "V:surface" },
      },
    })
    const enabled = variable({
      id: "V:enabled",
      name: "enabled",
      collectionId: "C:theme",
      scopes: ["ALL_SCOPES"],
      valuesByMode: {
        "M:light": true,
        "M:dark": false,
      },
    })

    installFigma({
      nodes: [bound("1:1", ["V:bg", "V:enabled"])],
      collections: [theme],
      variables: [enabled, bg],
    })
    const live = await getVariables({ selector: { nodeId: "1:1" } })

    expect(live.truncated).toBe(false)
    expect(live.observation.startedAt).toMatch(/Z$/)
    expect(live.collections).toEqual([
      {
        id: "C:theme",
        name: "Theme",
        modes: [
          { id: "M:light", name: "Light" },
          { id: "M:dark", name: "Dark" },
        ],
        variables: [
          {
            id: "V:bg",
            name: "color/bg",
            collectionId: "C:theme",
            scopes: ["ALL_FILLS", "FRAME_FILL"],
            values: [
              {
                modeId: "M:light",
                source: {
                  kind: "color",
                  value: { r: 1, g: 1, b: 1, a: 1 },
                },
              },
              {
                modeId: "M:dark",
                source: { kind: "alias", value: "V:surface" },
              },
            ],
            codeSyntax: [
              { platform: "WEB", code: "var(--bg)" },
              { platform: "ANDROID", code: "bg" },
              { platform: "iOS", code: "bgColor" },
            ],
          },
          {
            id: "V:enabled",
            name: "enabled",
            collectionId: "C:theme",
            scopes: ["ALL_SCOPES"],
            values: [
              { modeId: "M:light", source: { kind: "boolean", value: true } },
              { modeId: "M:dark", source: { kind: "boolean", value: false } },
            ],
            codeSyntax: [],
          },
        ],
      },
    ])
  })

  test("resolveAliases defaults to false and omits resolved values", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [{ modeId: "M:default", name: "Default" }],
    })
    const target = variable({
      id: "V:target",
      name: "target",
      collectionId: "C:theme",
      valuesByMode: { "M:default": 12 },
    })
    const alias = variable({
      id: "V:alias",
      name: "alias",
      collectionId: "C:theme",
      valuesByMode: {
        "M:default": { type: "VARIABLE_ALIAS", id: "V:target" },
      },
    })
    installFigma({
      nodes: [bound("1:1", ["V:alias"])],
      collections: [theme],
      variables: [alias, target],
    })

    const omitted = await getVariables({ selector: { nodeId: "1:1" } })
    const explicit = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: false,
    })

    expect(omitted.collections[0]?.variables[0]?.values[0]).toEqual({
      modeId: "M:default",
      source: { kind: "alias", value: "V:target" },
    })
    expect(explicit.collections[0]?.variables[0]?.values[0]?.resolved).toBe(
      undefined,
    )
  })

  test("resolveAliases resolves an alias-valued mode to a concrete value", async () => {
    // Recorded gap: no live document tested so far had an alias-valued variable
    // in scope, so this path has never been seen against real data. The alias
    // target is deliberately reachable only by id, which is how a library
    // variable behaves.
    const libraryId = "VariableID:abc123/9:9"
    installFigma({
      nodes: [bound("1:1", [libraryId])],
      collections: [
        collection({
          id: "VariableCollectionId:abc123/9:1",
          name: "Brand",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: libraryId,
          name: "brand/primary",
          collectionId: "VariableCollectionId:abc123/9:1",
          valuesByMode: {
            "M:default": { type: "VARIABLE_ALIAS", id: "V:ink" },
          },
        }),
        variable({
          id: "V:ink",
          name: "ink",
          collectionId: "VariableCollectionId:abc123/9:1",
          valuesByMode: { "M:default": { r: 0, g: 0, b: 0 } },
        }),
      ],
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })
    const value = result.collections[0]?.variables[0]?.values[0]

    expect(value?.source.kind).toBe("alias")
    expect(value?.resolved).toEqual({
      kind: "color",
      value: { r: 0, g: 0, b: 0, a: 1 },
    })
  })

  test("resolveAliases retains the source alias and the terminal value", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [{ modeId: "M:default", name: "Default" }],
    })
    const leaf = variable({
      id: "V:leaf",
      name: "leaf",
      collectionId: "C:theme",
      valuesByMode: { "M:default": { r: 0, g: 0, b: 1 } },
    })
    const mid = variable({
      id: "V:mid",
      name: "mid",
      collectionId: "C:theme",
      valuesByMode: {
        "M:default": { type: "VARIABLE_ALIAS", id: "V:leaf" },
      },
    })
    const root = variable({
      id: "V:root",
      name: "root",
      collectionId: "C:theme",
      valuesByMode: {
        "M:default": { type: "VARIABLE_ALIAS", id: "V:mid" },
      },
    })
    const { variableLookups } = installFigma({
      nodes: [bound("1:1", ["V:mid", "V:root"])],
      collections: [theme],
      variables: [mid, root],
      byId: new Map<string, unknown>([[leaf.id, leaf]]),
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })
    const values = result.collections[0]?.variables ?? []

    expect(values[0]?.values[0]).toEqual({
      modeId: "M:default",
      source: { kind: "alias", value: "V:leaf" },
      resolved: {
        kind: "color",
        value: { r: 0, g: 0, b: 1, a: 1 },
      },
    })
    expect(values[1]?.values[0]).toEqual({
      modeId: "M:default",
      source: { kind: "alias", value: "V:mid" },
      resolved: {
        kind: "color",
        value: { r: 0, g: 0, b: 1, a: 1 },
      },
    })
    // The two bound ids, then the alias target — each looked up exactly once.
    expect(variableLookups).toEqual(["V:mid", "V:root", "V:leaf"])
  })

  test("missing aliases stay in source and become item-level NODE_NOT_FOUND errors", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [{ modeId: "M:default", name: "Default" }],
    })
    const broken = variable({
      id: "V:broken",
      name: "broken",
      collectionId: "C:theme",
      valuesByMode: {
        "M:default": { type: "VARIABLE_ALIAS", id: "V:missing" },
      },
    })
    installFigma({
      nodes: [bound("1:1", ["V:broken"])],
      collections: [theme],
      variables: [broken],
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })
    expect(result.collections[0]?.variables[0]?.values[0]).toEqual({
      modeId: "M:default",
      source: { kind: "alias", value: "V:missing" },
      error: { code: "NODE_NOT_FOUND", retryable: false },
    })
  })

  test("cycles stay in source and become item-level LIMIT_EXCEEDED errors", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [
        { modeId: "M:light", name: "Light" },
        { modeId: "M:dark", name: "Dark" },
      ],
    })
    const a = variable({
      id: "V:a",
      name: "a",
      collectionId: "C:theme",
      valuesByMode: {
        "M:light": { type: "VARIABLE_ALIAS", id: "V:b" },
        "M:dark": 1,
      },
    })
    const b = variable({
      id: "V:b",
      name: "b",
      collectionId: "C:theme",
      valuesByMode: {
        "M:light": { type: "VARIABLE_ALIAS", id: "V:a" },
        "M:dark": 2,
      },
    })
    installFigma({
      nodes: [bound("1:1", ["V:a", "V:b"])],
      collections: [theme],
      variables: [a, b],
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })
    const variables = result.collections[0]?.variables ?? []

    expect(variables[0]?.values).toEqual([
      {
        modeId: "M:light",
        source: { kind: "alias", value: "V:b" },
        error: { code: "LIMIT_EXCEEDED", retryable: false },
      },
      { modeId: "M:dark", source: { kind: "float", value: 1 } },
    ])
    expect(variables[1]?.values).toEqual([
      {
        modeId: "M:light",
        source: { kind: "alias", value: "V:a" },
        error: { code: "LIMIT_EXCEEDED", retryable: false },
      },
      { modeId: "M:dark", source: { kind: "float", value: 2 } },
    ])
  })

  test("an unreachable variable is skipped rather than failing the read", async () => {
    installFigma({
      nodes: [bound("1:1", ["V:gone", "V:a"])],
      collections: [
        collection({
          id: "C:theme",
          name: "Theme",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:a",
          name: "a",
          collectionId: "C:theme",
          valuesByMode: { "M:default": 1 },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(idsOf(result)).toEqual(["V:a"])
  })

  test("keeps a bound variable whose collection cannot be resolved", async () => {
    // A library collection the host will not hand back leaves no mode names to
    // report, so the values list is empty — but the caller still learns which
    // variable the design binds, which is strictly more than nothing.
    installFigma({
      nodes: [bound("1:1", ["VariableID:abc123/9:9"])],
      variables: [
        variable({
          id: "VariableID:abc123/9:9",
          name: "brand/primary",
          collectionId: "VariableCollectionId:abc123/9:1",
          valuesByMode: { "M:default": { r: 0, g: 0, b: 1 } },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(result.collections).toEqual([
      {
        id: "VariableCollectionId:abc123/9:1",
        name: "",
        modes: [],
        variables: [
          {
            id: "VariableID:abc123/9:9",
            name: "brand/primary",
            collectionId: "VariableCollectionId:abc123/9:1",
            scopes: ["ALL_SCOPES"],
            values: [],
            codeSyntax: [],
          },
        ],
      },
    ])
  })

  test("fails when the variables API is unavailable", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Page 1", type: "PAGE", children: [] },
      editorType: "dev",
    }

    await expect(getVariables({})).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
    expect(PluginReadError).toBeDefined()
  })

  test("fails when the id lookup this tool depends on is absent", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Page 1", type: "PAGE", children: [] },
      editorType: "dev",
      variables: { getVariableCollectionByIdAsync: async () => null },
    }

    await expect(getVariables({})).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
  })

  test("checks cancellation between variable lookups", async () => {
    const cancellation = new LocalCancellationController()
    const byId = new Map<string, unknown>()
    installFigma({
      nodes: [bound("1:1", ["V:a", "V:b"])],
      byId,
    })
    byId.set("V:a", {
      get id() {
        cancellation.abort()
        return "V:a"
      },
      name: "a",
      variableCollectionId: "C:theme",
      valuesByMode: {},
      scopes: [],
      codeSyntax: {},
    })

    await expect(
      getVariables({ selector: { nodeId: "1:1" } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })

  test("checks cancellation between node batches of 100 while walking the scope", async () => {
    const cancellation = new LocalCancellationController()
    const children: unknown[] = Array.from({ length: 101 }, (_, index) =>
      bound(`1:${index + 2}`, []),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return bound("1:52", [])
      },
    })
    installFigma({ nodes: [bound("1:1", [], children)] })

    await expect(
      getVariables({ selector: { nodeId: "1:1" } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })
})
