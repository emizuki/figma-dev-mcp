import { beforeEach, describe, expect, test } from "bun:test"

import { installFigma } from "../../tests/figma-harness"
import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { byteLength } from "./serialize"
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
      pageChildren: [bound("1:1", [libraryId])],
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
      pageChildren: [bound("1:1", ["V:a"]), bound("2:2", ["V:b"])],
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
      pageChildren: [bound("1:1", []), bound("2:2", ["V:a"])],
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
      pageChildren: [bound("1:1", ["V:a"], [child])],
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
      pageChildren: [bound("1:1", ["V:a", "V:b", "V:z"])],
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
      pageChildren: [bound("1:1", ["V:bg", "V:enabled"])],
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
      pageChildren: [bound("1:1", ["V:alias"])],
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
      pageChildren: [bound("1:1", [libraryId])],
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
      pageChildren: [bound("1:1", ["V:mid", "V:root"])],
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
      pageChildren: [bound("1:1", ["V:broken"])],
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
      pageChildren: [bound("1:1", ["V:a", "V:b"])],
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
      pageChildren: [bound("1:1", ["V:gone", "V:a"])],
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

  test("a collection that cannot be resolved still yields real values, and says data was lost", async () => {
    // A library collection the host will not hand back costs the caller its name
    // and its mode names. The mode *ids* are not lost: they are the host-supplied
    // keys of valuesByMode, so the values are reported rather than dropped.
    installFigma({
      pageChildren: [bound("1:1", ["VariableID:abc123/9:9"])],
      variables: [
        variable({
          id: "VariableID:abc123/9:9",
          name: "brand/primary",
          collectionId: "VariableCollectionId:abc123/9:1",
          valuesByMode: {
            "M:light": { r: 0, g: 0, b: 1 },
            "M:dark": { r: 0, g: 0, b: 0 },
          },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(result.collections).toEqual([
      {
        id: "VariableCollectionId:abc123/9:1",
        name: "",
        modes: [
          { id: "M:light", name: "" },
          { id: "M:dark", name: "" },
        ],
        variables: [
          {
            id: "VariableID:abc123/9:9",
            name: "brand/primary",
            collectionId: "VariableCollectionId:abc123/9:1",
            scopes: ["ALL_SCOPES"],
            values: [
              {
                modeId: "M:light",
                source: { kind: "color", value: { r: 0, g: 0, b: 1, a: 1 } },
              },
              {
                modeId: "M:dark",
                source: { kind: "color", value: { r: 0, g: 0, b: 0, a: 1 } },
              },
            ],
            codeSyntax: [],
          },
        ],
      },
    ])
    // Never silently complete: the collection's own name and its mode names are
    // missing from this answer.
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 1 })
  })

  test("an unresolved collection resolves aliases too", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["VariableID:abc123/9:9"])],
      variables: [
        variable({
          id: "VariableID:abc123/9:9",
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
          valuesByMode: { "M:default": 4 },
        }),
      ],
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })

    expect(result.collections[0]?.variables[0]?.values[0]).toEqual({
      modeId: "M:default",
      source: { kind: "alias", value: "V:ink" },
      resolved: { kind: "float", value: 4 },
    })
  })

  test("the wall-clock budget covers alias resolution, not only the first lookup", async () => {
    // Measured against the pre-fix code: three stalling alias targets took
    // 4509 ms and still reported truncated: false, because the budget was only
    // checked in the id loop and once per collection while session.resolve
    // recursed outside it. Eleven such targets — the live file that motivated
    // this task has exactly eleven in-scope ids — would pass the broker's 15 s
    // inactivity ceiling. Each stall costs one settleOrSkip timeout (1.5 s), so
    // the budget's job here is to make sure only the first one is ever paid.
    const aliasTo = (id: string, target: string) =>
      variable({
        id,
        name: id,
        collectionId: "C:theme",
        valuesByMode: { "M:default": { type: "VARIABLE_ALIAS", id: target } },
      })
    const { variableLookups } = installFigma({
      pageChildren: [bound("1:1", ["V:a", "V:b", "V:c"])],
      collections: [
        collection({
          id: "C:theme",
          name: "Theme",
          modes: [{ modeId: "M:default", name: "Default" }],
        }),
      ],
      variables: [
        aliasTo("V:a", "V:x"),
        aliasTo("V:b", "V:y"),
        aliasTo("V:c", "V:z"),
      ],
      lookupDelayMs: {
        "V:x": Number.POSITIVE_INFINITY,
        "V:y": Number.POSITIVE_INFINITY,
        "V:z": Number.POSITIVE_INFINITY,
      },
    })

    const started = Date.now()
    const result = await getVariables(
      { selector: { nodeId: "1:1" }, resolveAliases: true },
      undefined,
      { variableLookupBudgetMs: 50 },
    )
    const elapsed = Date.now() - started

    // Only the first stall is paid for: the deadline passes while it is in
    // flight, and no further host lookup is started after that.
    expect(variableLookups).toEqual(["V:a", "V:b", "V:c", "V:x"])
    expect(elapsed).toBeLessThan(3_000)
    // Stopping on the clock is reported as retryable, unlike a cycle or a
    // genuinely missing target.
    const values = result.collections[0]?.variables ?? []
    expect(values[1]?.values[0]?.error).toEqual({
      code: "LIMIT_EXCEEDED",
      retryable: true,
    })
    expect(values[2]?.values[0]?.error).toEqual({
      code: "LIMIT_EXCEEDED",
      retryable: true,
    })
    // Silent truncation is worse than slow.
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })
  })

  test("a walk that ran out outranks a byte-limit cut", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["V:a"], [bound("1:2", ["V:b"])])],
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

    const result = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      { visitedNodes: 1, encodedBytes: 1 },
    )

    expect(result.collections).toEqual([])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 1 })
  })

  test("a spent budget outranks a byte-limit cut", async () => {
    const { variableLookups } = installFigma({
      pageChildren: [bound("1:1", ["V:a", "V:b"])],
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
      lookupDelayMs: { "V:a": 80 },
    })

    const result = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      { variableLookupBudgetMs: 20, encodedBytes: 1 },
    )

    // V:b was never fetched, and the collection that was grouped hit the byte
    // limit on the way out. The budget is the earlier, larger loss.
    expect(variableLookups).toEqual(["V:a"])
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 1 })
  })

  test("a byte-limit cut is reported when nothing earlier stopped the read", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["V:a"])],
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

    const result = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      { encodedBytes: 1 },
    )

    expect(result.collections).toEqual([])
    expect(result.truncated).toBe(true)
    expect(result.truncation?.reason).toBe("byteLimit")
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
      pageChildren: [bound("1:1", ["V:a", "V:b"])],
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
    installFigma({ pageChildren: [bound("1:1", [], children)] })

    await expect(
      getVariables({ selector: { nodeId: "1:1" } }, cancellation.signal),
    ).rejects.toThrow("Operation cancelled")
  })

  test("each value kind the protocol names is tagged with its own kind", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["V:bool", "V:num", "V:str", "V:color"])],
      collections: [
        collection({
          id: "C:kinds",
          name: "Kinds",
          modes: [{ modeId: "M:1", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:bool",
          name: "bool",
          collectionId: "C:kinds",
          valuesByMode: { "M:1": true },
        }),
        variable({
          id: "V:num",
          name: "num",
          collectionId: "C:kinds",
          valuesByMode: { "M:1": 12.5 },
        }),
        variable({
          id: "V:str",
          name: "str",
          collectionId: "C:kinds",
          valuesByMode: { "M:1": "Buy now" },
        }),
        variable({
          id: "V:color",
          name: "color",
          collectionId: "C:kinds",
          valuesByMode: { "M:1": { r: 0.1, g: 0.2, b: 0.3, a: 0.4 } },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(
      result.collections[0]?.variables.map((item) => item.values[0]?.source),
    ).toEqual([
      { kind: "boolean", value: true },
      { kind: "float", value: 12.5 },
      { kind: "string", value: "Buy now" },
      { kind: "color", value: { r: 0.1, g: 0.2, b: 0.3, a: 0.4 } },
    ])
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("only the modes the collection declares are emitted", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["V:one"])],
      collections: [
        collection({
          id: "C:one",
          name: "One",
          modes: [{ modeId: "M:declared", name: "Declared" }],
        }),
      ],
      variables: [
        variable({
          id: "V:one",
          name: "one",
          collectionId: "C:one",
          valuesByMode: { "M:declared": 1, "M:undeclared": 2 },
        }),
      ],
    })

    const result = await getVariables({ selector: { nodeId: "1:1" } })

    expect(result.collections[0]?.variables[0]?.values).toEqual([
      { modeId: "M:declared", source: { kind: "float", value: 1 } },
    ])
  })

  test("an alias resolves in the mode asked for, or its target's default mode", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["V:same", "V:missing", "V:chain"])],
      collections: [
        collection({
          id: "C:src",
          name: "Source",
          modes: [{ modeId: "M:a", name: "A" }],
          defaultModeId: "M:a",
        }),
        collection({
          id: "C:dst",
          name: "Target",
          modes: [
            { modeId: "M:a", name: "A" },
            { modeId: "M:b", name: "B" },
          ],
          defaultModeId: "M:b",
        }),
      ],
      variables: [
        variable({
          id: "V:same",
          name: "same",
          collectionId: "C:src",
          valuesByMode: { "M:a": { type: "VARIABLE_ALIAS", id: "V:both" } },
        }),
        variable({
          id: "V:missing",
          name: "missing",
          collectionId: "C:src",
          valuesByMode: { "M:a": { type: "VARIABLE_ALIAS", id: "V:onlyB" } },
        }),
        variable({
          id: "V:chain",
          name: "chain",
          collectionId: "C:src",
          valuesByMode: { "M:a": { type: "VARIABLE_ALIAS", id: "V:hop" } },
        }),
        variable({
          id: "V:both",
          name: "both",
          collectionId: "C:dst",
          valuesByMode: { "M:a": "from mode a", "M:b": "from mode b" },
        }),
        variable({
          id: "V:onlyB",
          name: "onlyB",
          collectionId: "C:dst",
          valuesByMode: { "M:b": "only in mode b" },
        }),
        variable({
          id: "V:hop",
          name: "hop",
          collectionId: "C:dst",
          valuesByMode: { "M:b": { type: "VARIABLE_ALIAS", id: "V:both" } },
        }),
      ],
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })
    const resolvedOf = (id: string) =>
      result.collections
        .flatMap((item) => item.variables)
        .find((item) => item.id === id)?.values[0]?.resolved

    // The target declares the mode asked for, so its collection default is
    // not consulted.
    expect(resolvedOf("V:same")).toEqual({
      kind: "string",
      value: "from mode a",
    })
    // The target does not, so the collection's default mode answers instead.
    expect(resolvedOf("V:missing")).toEqual({
      kind: "string",
      value: "only in mode b",
    })
    // The second hop follows the mode the first hop resolved to, not the mode
    // the caller started in.
    expect(resolvedOf("V:chain")).toEqual({
      kind: "string",
      value: "from mode b",
    })
  })

  test("one variable aliasing one target in two modes resolves each mode on its own", async () => {
    installFigma({
      pageChildren: [bound("1:1", ["V:alias"])],
      collections: [
        collection({
          id: "C:two",
          name: "Two",
          modes: [
            { modeId: "M:a", name: "A" },
            { modeId: "M:b", name: "B" },
          ],
        }),
      ],
      variables: [
        variable({
          id: "V:alias",
          name: "alias",
          collectionId: "C:two",
          valuesByMode: {
            "M:a": { type: "VARIABLE_ALIAS", id: "V:target" },
            "M:b": { type: "VARIABLE_ALIAS", id: "V:target" },
          },
        }),
        variable({
          id: "V:target",
          name: "target",
          collectionId: "C:two",
          valuesByMode: { "M:a": "A side", "M:b": "B side" },
        }),
      ],
    })

    const result = await getVariables({
      selector: { nodeId: "1:1" },
      resolveAliases: true,
    })

    expect(
      result.collections[0]?.variables[0]?.values.map((entry) => [
        entry.modeId,
        entry.resolved,
      ]),
    ).toEqual([
      ["M:a", { kind: "string", value: "A side" }],
      ["M:b", { kind: "string", value: "B side" }],
    ])
  })

  test("a budget spent mid-read stops the collection lookup and marks its aliases retryable", async () => {
    const harness = installFigma({
      pageChildren: [bound("1:1", ["V:cached", "V:slow"])],
      collections: [
        collection({
          id: "C:one",
          name: "One",
          modes: [{ modeId: "M:1", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:cached",
          name: "cached",
          collectionId: "C:one",
          valuesByMode: { "M:1": { type: "VARIABLE_ALIAS", id: "V:slow" } },
        }),
        variable({
          id: "V:slow",
          name: "slow",
          collectionId: "C:one",
          valuesByMode: { "M:1": 1 },
        }),
      ],
      lookupDelayMs: { "V:slow": 30 },
    })

    const result = await getVariables(
      { selector: { nodeId: "1:1" }, resolveAliases: true },
      undefined,
      { variableLookupBudgetMs: 10 },
    )

    // Past the deadline the collection is never asked for, so its name and
    // mode names are lost and the mode ids come from the variables themselves.
    expect(harness.collectionLookups).toEqual([])
    expect(result.collections[0]?.name).toBe("")
    expect(result.collections[0]?.modes).toEqual([{ id: "M:1", name: "" }])
    // The alias target is already in the session cache and carries a plain
    // value, so nothing but the clock can stop this resolution — and it does,
    // as a retryable limit rather than a missing target.
    expect(
      result.collections[0]?.variables.map((item) => [
        item.id,
        item.values[0]?.error,
        item.values[0]?.resolved,
      ]),
    ).toEqual([
      ["V:cached", { code: "LIMIT_EXCEEDED", retryable: true }, undefined],
      ["V:slow", undefined, undefined],
    ])
    expect(result.truncated).toBe(true)
  })

  test("the collection ceiling, the walk cut and the byte ceiling each report their own total", async () => {
    const build = (index: number) => ({
      collection: collection({
        id: `C:${index}`,
        name: `C${index}`,
        modes: [{ modeId: "M:1", name: "Default" }],
      }),
      variable: variable({
        id: `V:${index}`,
        name: `v${index}`,
        collectionId: `C:${index}`,
        valuesByMode: { "M:1": index },
      }),
    })
    const built = [1, 2, 3].map(build)
    const bindings = built.map((item) => item.variable.id)
    installFigma({
      pageChildren: [bound("1:1", bindings)],
      collections: built.map((item) => item.collection),
      variables: built.map((item) => item.variable),
    })

    const capped = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      { returnedNodes: 1 },
    )
    expect(capped.collections).toHaveLength(1)
    expect(capped.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })

    const serialized = (index: number) => ({
      id: `C:${index}`,
      name: `C${index}`,
      modes: [{ id: "M:1", name: "Default" }],
      variables: [
        {
          id: `V:${index}`,
          name: `v${index}`,
          collectionId: `C:${index}`,
          scopes: ["ALL_SCOPES"],
          values: [
            { modeId: "M:1", source: { kind: "float" as const, value: index } },
          ],
          codeSyntax: [],
        },
      ],
    })
    const budget = byteLength(serialized(1)) + byteLength(serialized(2)) - 1
    const bytes = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      { encodedBytes: budget },
    )
    expect(bytes.collections).toEqual([serialized(1)])
    expect(bytes.truncation).toEqual({
      reason: "byteLimit",
      encodedBytes: byteLength(serialized(1)) + byteLength(serialized(2)),
    })
  })

  test("a truncated walk outranks an exhausted budget, and the emission cut outranks an unresolved collection", async () => {
    installFigma({
      pageChildren: [
        bound("1:1", ["V:1"], [bound("1:2", ["V:2"]), bound("1:3", ["V:3"])]),
      ],
      collections: [
        collection({
          id: "C:1",
          name: "One",
          modes: [{ modeId: "M:1", name: "Default" }],
        }),
      ],
      variables: [
        variable({
          id: "V:1",
          name: "v1",
          collectionId: "C:1",
          valuesByMode: { "M:1": 1 },
        }),
        variable({
          id: "V:2",
          name: "v2",
          collectionId: "C:2",
          valuesByMode: { "M:1": 2 },
        }),
        variable({
          id: "V:3",
          name: "v3",
          collectionId: "C:3",
          valuesByMode: { "M:1": 3 },
        }),
      ],
    })

    // The walk stops after two nodes; the budget is out before the first
    // lookup. The walk is the earlier, larger loss and is what gets reported.
    const walked = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      {
        visitedNodes: 2,
        variableLookupBudgetMs: 0,
      },
    )
    expect(walked.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })

    // C:2 and C:3 are collections the host will not return, so both are
    // unresolved — the smallest of the four losses. The byte ceiling cuts
    // first and is what gets reported.
    const first = {
      id: "C:1",
      name: "One",
      modes: [{ id: "M:1", name: "Default" }],
      variables: [
        {
          id: "V:1",
          name: "v1",
          collectionId: "C:1",
          scopes: ["ALL_SCOPES"],
          values: [
            { modeId: "M:1", source: { kind: "float" as const, value: 1 } },
          ],
          codeSyntax: [],
        },
      ],
    }
    const capped = await getVariables(
      { selector: { nodeId: "1:1" } },
      undefined,
      { encodedBytes: byteLength(first) },
    )
    expect(capped.collections).toEqual([first])
    expect(capped.truncation?.reason).toBe("byteLimit")
  })
})
