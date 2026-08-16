import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getVariables } from "./variables"

const page = (id: string, name: string) => ({
  id,
  name,
  type: "PAGE",
  children: [],
})

function collection(options: {
  id: string
  name: string
  modes: { modeId: string; name: string }[]
  variableIds: string[]
  defaultModeId?: string
}) {
  return {
    id: options.id,
    name: options.name,
    modes: options.modes,
    variableIds: options.variableIds,
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

function installFigma(options: {
  collections?: unknown[]
  variables?: unknown[]
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
  const api = {
    root: { name: "Checkout flow", children: [page("0:1", "Page 1")] },
    currentPage: page("0:1", "Page 1"),
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("variables must not call loadAllPagesAsync")
    },
    variables: {
      getLocalVariableCollectionsAsync: async () => options.collections ?? [],
      getLocalVariablesAsync: async () => options.variables ?? [],
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

describe("get_variables", () => {
  beforeEach(() => {
    installFigma({})
  })

  test("preserves collection and mode order, scopes, code syntax, and raw aliases", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [
        { modeId: "M:light", name: "Light" },
        { modeId: "M:dark", name: "Dark" },
      ],
      variableIds: ["V:bg", "V:enabled"],
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

    const { variableLookups } = installFigma({
      collections: [theme],
      variables: [enabled, bg],
    })
    const live = await getVariables({})

    expect(variableLookups).toEqual([])
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
      variableIds: ["V:alias"],
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
      collections: [theme],
      variables: [alias, target],
    })

    const omitted = await getVariables({})
    const explicit = await getVariables({ resolveAliases: false })

    expect(omitted.collections[0]?.variables[0]?.values[0]).toEqual({
      modeId: "M:default",
      source: { kind: "alias", value: "V:target" },
    })
    expect(explicit.collections[0]?.variables[0]?.values[0]?.resolved).toBe(
      undefined,
    )
  })

  test("resolveAliases retains the source alias and the terminal value", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [{ modeId: "M:default", name: "Default" }],
      variableIds: ["V:mid", "V:root"],
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
      collections: [theme],
      variables: [mid, root],
      byId: new Map<string, unknown>([[leaf.id, leaf]]),
    })

    const result = await getVariables({ resolveAliases: true })
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
    expect(variableLookups).toEqual(["V:leaf"])
  })

  test("missing aliases stay in source and become item-level NODE_NOT_FOUND errors", async () => {
    const theme = collection({
      id: "C:theme",
      name: "Theme",
      modes: [{ modeId: "M:default", name: "Default" }],
      variableIds: ["V:broken"],
    })
    const broken = variable({
      id: "V:broken",
      name: "broken",
      collectionId: "C:theme",
      valuesByMode: {
        "M:default": { type: "VARIABLE_ALIAS", id: "V:missing" },
      },
    })
    installFigma({ collections: [theme], variables: [broken] })

    const result = await getVariables({ resolveAliases: true })
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
      variableIds: ["V:a", "V:b"],
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
    installFigma({ collections: [theme], variables: [a, b] })

    const result = await getVariables({ resolveAliases: true })
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

  test("fails when the variables API is unavailable", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: page("0:1", "Page 1"),
      editorType: "dev",
    }

    await expect(getVariables({})).rejects.toMatchObject({
      code: "CAPABILITY_UNAVAILABLE",
    })
    expect(PluginReadError).toBeDefined()
  })

  test("checks cancellation between collection batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const collections = Array.from({ length: 101 }, (_, index) =>
      collection({
        id: `C:${index + 1}`,
        name: `Collection ${index + 1}`,
        modes: [{ modeId: `M:${index + 1}`, name: "Default" }],
        variableIds: [],
      }),
    )
    Object.defineProperty(collections, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return collection({
          id: "C:51",
          name: "Collection 51",
          modes: [{ modeId: "M:51", name: "Default" }],
          variableIds: [],
        })
      },
    })
    installFigma({ collections, variables: [] })

    await expect(getVariables({}, cancellation.signal)).rejects.toThrow(
      "Operation cancelled",
    )
  })
})
