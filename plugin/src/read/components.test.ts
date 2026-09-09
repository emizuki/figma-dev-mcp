import { beforeEach, describe, expect, test } from "bun:test"

import { installFigma } from "../../tests/figma-harness"
import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getComponents } from "./components"
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
  name: string,
  extras: Record<string, unknown> = {},
) => ({
  id,
  name,
  type: "FRAME",
  visible: true,
  children: [],
  ...extras,
})

function propertyDefinitions() {
  return {
    Size: {
      type: "VARIANT",
      defaultValue: "Small",
      variantOptions: ["Small", "Medium", "Large"],
    },
    "IconVisible#0:0": {
      type: "BOOLEAN",
      defaultValue: false,
    },
    "ButtonText#0:1": {
      type: "TEXT",
      defaultValue: "submit",
    },
    "IconInstance#0:2": {
      type: "INSTANCE_SWAP",
      defaultValue: "1:1",
      preferredValues: [
        { type: "COMPONENT", key: "ckey1" },
        { type: "COMPONENT_SET", key: "sgkey1" },
      ],
    },
    "Slot#0:3": {
      type: "SLOT",
      defaultValue: "",
    },
  }
}

function componentSet(options: {
  id: string
  name: string
  children: unknown[]
  description?: string
  documentationLinks?: { uri: string; label?: string }[]
}) {
  return {
    id: options.id,
    name: options.name,
    type: "COMPONENT_SET",
    visible: true,
    leftover: "must-not-leak",
    description: options.description ?? "Primary button set",
    documentationLinks: options.documentationLinks ?? [
      { uri: "https://docs.example/button", label: "Button" },
    ],
    variantProperties: null,
    componentPropertyDefinitions: propertyDefinitions(),
    children: options.children,
  }
}

function variant(options: {
  id: string
  name: string
  parentId: string
  variantProperties: Record<string, string>
  description?: string
}) {
  return {
    id: options.id,
    name: options.name,
    type: "COMPONENT",
    visible: true,
    leftover: "must-not-leak",
    description: options.description ?? "Small hover",
    documentationLinks: [{ uri: "https://docs.example/variant" }],
    variantProperties: options.variantProperties,
    componentPropertyDefinitions: {},
    parent: { id: options.parentId, type: "COMPONENT_SET" },
    children: [],
  }
}

function standaloneComponent(id: string, name: string) {
  return {
    id,
    name,
    type: "COMPONENT",
    visible: true,
    description: "Icon",
    documentationLinks: [],
    variantProperties: null,
    componentPropertyDefinitions: {
      "Label#1:0": { type: "TEXT", defaultValue: "icon" },
    },
    children: [],
  }
}

function instance(options: {
  id: string
  name: string
  main?: unknown
  fail?: boolean
  children?: unknown[]
}) {
  return {
    id: options.id,
    name: options.name,
    type: "INSTANCE",
    visible: true,
    children: options.children ?? [],
    getMainComponentAsync: async () => {
      if (options.fail) throw new Error("main component lookup failed")
      return options.main ?? null
    },
  }
}

describe("get_components", () => {
  beforeEach(() => {
    installFigma({ currentPage: page("0:2", "Current") })
  })

  test("serializes components, sets, descriptions, docs, variants, and property definitions", async () => {
    const smallHover = variant({
      id: "2:2",
      name: "Size=Small, State=Hover",
      parentId: "2:1",
      variantProperties: { Size: "Small", State: "Hover" },
    })
    const largeDefault = variant({
      id: "2:3",
      name: "Size=Large, State=Default",
      parentId: "2:1",
      variantProperties: { Size: "Large", State: "Default" },
      description: "Large default",
    })
    const set = componentSet({
      id: "2:1",
      name: "Button",
      children: [smallHover, largeDefault],
    })
    const icon = standaloneComponent("3:1", "Icon")
    const current = page("0:2", "Current", [set, icon])
    installFigma({ currentPage: current })

    const result = await getComponents({})

    expect(result.truncated).toBe(false)
    expect(result.observation.startedAt).toMatch(/Z$/)
    expect(result.instances).toEqual([])
    expect(result.components).toEqual([
      {
        id: "2:1",
        name: "Button",
        description: "Primary button set",
        documentation: [
          { uri: "https://docs.example/button", label: "Button" },
        ],
        variantProperties: [],
        propertyDefinitions: [
          {
            name: "Size",
            defaultValue: { kind: "variant", value: "Small" },
            preferredValues: [
              { kind: "variant", value: "Small" },
              { kind: "variant", value: "Medium" },
              { kind: "variant", value: "Large" },
            ],
          },
          {
            name: "IconVisible#0:0",
            defaultValue: { kind: "boolean", value: false },
          },
          {
            name: "ButtonText#0:1",
            defaultValue: { kind: "text", value: "submit" },
          },
          {
            name: "IconInstance#0:2",
            defaultValue: { kind: "instanceSwap", value: "1:1" },
            preferredValues: [
              { kind: "instanceSwap", value: "ckey1" },
              { kind: "instanceSwap", value: "sgkey1" },
            ],
          },
        ],
      },
      {
        id: "2:2",
        name: "Size=Small, State=Hover",
        componentSetId: "2:1",
        description: "Small hover",
        documentation: [{ uri: "https://docs.example/variant" }],
        variantProperties: [
          { name: "Size", value: "Small" },
          { name: "State", value: "Hover" },
        ],
        propertyDefinitions: [],
      },
      {
        id: "2:3",
        name: "Size=Large, State=Default",
        componentSetId: "2:1",
        description: "Large default",
        documentation: [{ uri: "https://docs.example/variant" }],
        variantProperties: [
          { name: "Size", value: "Large" },
          { name: "State", value: "Default" },
        ],
        propertyDefinitions: [],
      },
      {
        id: "3:1",
        name: "Icon",
        description: "Icon",
        documentation: [],
        variantProperties: [],
        propertyDefinitions: [
          {
            name: "Label#1:0",
            defaultValue: { kind: "text", value: "icon" },
          },
        ],
      },
    ])
    for (const component of result.components) {
      expect(Object.keys(component)).not.toContain("leftover")
    }
  })

  test("returns components already indexed when instance lookups exhaust the time budget", async () => {
    const main = standaloneComponent("3:1", "Icon")
    const hung = Array.from({ length: 8 }, (_, index) => {
      const node = instance({ id: `4:${index + 1}`, name: "Hung" })
      node.getMainComponentAsync = () => new Promise(() => undefined)
      return node
    })
    const current = page("0:2", "Current", [main, ...hung])
    installFigma({ currentPage: current })

    const result = await getComponents({}, undefined, {
      returnedNodes: 20,
      visitedNodes: 40,
      encodedBytes: 8 * 1024 * 1024,
      mainComponentBudgetMs: 40,
    })

    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
    expect(result.instances).toEqual([])
    expect(result.truncated).toBe(true)
  })

  test("skips an instance whose getMainComponentAsync never settles", async () => {
    const main = standaloneComponent("3:1", "Icon")
    const ok = instance({ id: "4:1", name: "Used", main })
    const hung = instance({ id: "4:2", name: "Hung" })
    hung.getMainComponentAsync = () => new Promise(() => undefined)
    const current = page("0:2", "Current", [main, ok, hung])
    installFigma({ currentPage: current })

    const result = await getComponents({})
    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
    expect(result.instances).toEqual([
      { instanceId: "4:1", componentId: "3:1" },
    ])
  })

  test("resolves main components asynchronously and keeps failures on the affected instance", async () => {
    const main = standaloneComponent("3:1", "Icon")
    const ok = instance({ id: "4:1", name: "Used", main })
    const missing = instance({ id: "4:2", name: "Missing" })
    const failed = instance({ id: "4:3", name: "Broken", fail: true })
    const current = page("0:2", "Current", [main, ok, missing, failed])
    installFigma({ currentPage: current })

    const result = await getComponents({})

    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
    expect(result.instances).toEqual([
      { instanceId: "4:1", componentId: "3:1" },
    ])
    expect(result.truncated).toBe(false)
  })

  test("indexes instances once and dedupes relationships by stable instance identity", async () => {
    const main = standaloneComponent("3:1", "Icon")
    const first = instance({ id: "4:1", name: "Used", main })
    const second = instance({ id: "4:2", name: "Also", main })
    const cycle = instance({
      id: "4:1",
      name: "Used",
      main,
      children: [first],
    })
    const current = page("0:2", "Current", [main, cycle, second])
    installFigma({ currentPage: current })

    const result = await getComponents({})

    expect(result.instances).toEqual([
      { instanceId: "4:1", componentId: "3:1" },
      { instanceId: "4:2", componentId: "3:1" },
    ])
  })

  test("loads several explicit pages independently without changing the current page", async () => {
    const alpha = standaloneComponent("3:1", "Alpha")
    const beta = standaloneComponent("3:2", "Beta")
    const current = page("0:2", "Current", [
      standaloneComponent("3:9", "Hidden"),
    ])
    const first = page("0:1", "First", [alpha])
    const second = page("0:3", "Second", [beta])
    const { currentPage, loadedPages, lookedUp } = installFigma({
      currentPage: current,
      pages: [first, current, second],
    })

    const result = await getComponents({
      selector: { pageIds: [first.id, second.id] },
    })

    expect(loadedPages).toEqual(["0:1", "0:3"])
    expect(lookedUp).toEqual(["0:1", "0:3"])
    expect(
      (globalThis as typeof globalThis & { figma: { currentPage: unknown } })
        .figma.currentPage,
    ).toBe(currentPage)
    expect(result.components.map((item) => item.id)).toEqual(["3:1", "3:2"])
  })

  test("fails a missing page among several without falling back to the current page", async () => {
    const current = page("0:2", "Current", [
      standaloneComponent("3:9", "Hidden"),
    ])
    const first = page("0:1", "First", [standaloneComponent("3:1", "Alpha")])
    const { loadedPages } = installFigma({
      currentPage: current,
      pages: [first, current],
    })

    await expect(
      getComponents({ selector: { pageIds: [first.id, "0:9"] } }),
    ).rejects.toMatchObject({ code: "PAGE_NOT_FOUND" })
    expect(loadedPages).toEqual(["0:1"])
    expect(
      (
        globalThis as typeof globalThis & {
          figma: { currentPage: { id: string } }
        }
      ).figma.currentPage.id,
    ).toBe("0:2")
    expect(PluginReadError).toBeDefined()
  })

  test("keeps a component set when variantProperties is not readable", async () => {
    const set = componentSet({
      id: "2:1",
      name: "Button",
      children: [
        variant({
          id: "2:2",
          name: "Size=Small",
          parentId: "2:1",
          variantProperties: { Size: "Small" },
        }),
      ],
    })
    Object.defineProperty(set, "variantProperties", {
      configurable: true,
      enumerable: true,
      get() {
        throw new Error("variantProperties is not on ComponentSetNode")
      },
    })
    installFigma({
      currentPage: page("0:2", "Current"),
      nodes: new Map<string, unknown>([[set.id, set]]),
    })

    const result = await getComponents({ selector: { nodeId: set.id } })
    expect(result.components[0]).toMatchObject({
      id: "2:1",
      name: "Button",
      variantProperties: [],
    })
    expect(result.truncated).toBe(false)
  })

  test("keeps a variant component when componentPropertyDefinitions throws", async () => {
    const child = variant({
      id: "2:2",
      name: "Size=Small",
      parentId: "2:1",
      variantProperties: { Size: "Small" },
    })
    Object.defineProperty(child, "componentPropertyDefinitions", {
      configurable: true,
      enumerable: true,
      get() {
        throw new Error("definitions live on the component set")
      },
    })
    installFigma({
      currentPage: page("0:2", "Current"),
      nodes: new Map<string, unknown>([[child.id, child]]),
    })

    const result = await getComponents({ selector: { nodeId: child.id } })
    expect(result.components[0]).toMatchObject({
      id: "2:2",
      name: "Size=Small",
      variantProperties: [{ name: "Size", value: "Small" }],
      propertyDefinitions: [],
    })
  })

  test("resolves only the explicit node selector", async () => {
    const selected = standaloneComponent("3:1", "Icon")
    const other = standaloneComponent("3:2", "Other")
    const current = page("0:2", "Current", [selected, other])
    installFigma({
      currentPage: current,
      nodes: new Map<string, unknown>([[selected.id, selected]]),
    })

    const result = await getComponents({ selector: { nodeId: selected.id } })
    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
  })

  test("bounds returned components and reports truncation", async () => {
    const current = page("0:2", "Current", [
      standaloneComponent("3:1", "One"),
      standaloneComponent("3:2", "Two"),
      standaloneComponent("3:3", "Three"),
    ])
    installFigma({ currentPage: current })

    const result = await getComponents({}, undefined, {
      returnedNodes: 2,
      visitedNodes: 10,
      encodedBytes: 8 * 1024 * 1024,
    })

    expect(result.components.map((item) => item.id)).toEqual(["3:1", "3:2"])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: expect.any(Number),
    })
  })

  test("keeps components and instances already indexed when the visit ceiling is hit", async () => {
    const main = standaloneComponent("3:1", "Icon")
    const used = instance({ id: "4:1", name: "Used", main })
    const unseen = standaloneComponent("3:2", "Unseen")
    const extras = Array.from({ length: 4 }, (_, index) =>
      frame(`1:${index + 1}`, "Padding"),
    )
    const current = page("0:2", "Current", [main, used, ...extras, unseen])
    installFigma({ currentPage: current })

    const result = await getComponents({}, undefined, {
      returnedNodes: 10,
      visitedNodes: 3,
      encodedBytes: 8 * 1024 * 1024,
    })

    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
    expect(result.instances).toEqual([
      { instanceId: "4:1", componentId: "3:1" },
    ])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({
      reason: "nodeLimit",
      visitedNodes: 3,
    })
  })

  test("components referenced by in-scope instances are resolved from outside the scope", async () => {
    // The instance is inside the selector; its main component lives on
    // another page (or library) that the subtree walk cannot see. Today
    // `components` comes back empty and the componentId cannot be joined.
    const component = standaloneComponent("1:1", "Button")
    const target = instance({
      id: "2:2",
      name: "Button instance",
      main: component,
    })
    const { lookedUp } = installFigma({
      currentPage: page("0:2", "Current"),
      nodes: new Map<string, unknown>([
        ["2:2", target],
        ["1:1", component],
      ]),
    })

    const result = await getComponents({ selector: { nodeId: "2:2" } })

    expect(result.instances).toEqual([
      { instanceId: "2:2", componentId: "1:1" },
    ])
    expect(result.components.map((item) => item.id)).toEqual(["1:1"])
    expect(lookedUp).toContain("1:1")
  })

  test("checks cancellation between child batches of 100", async () => {
    const cancellation = new LocalCancellationController()
    const children = Array.from({ length: 101 }, (_, index) =>
      standaloneComponent(`3:${index + 1}`, "Item"),
    )
    Object.defineProperty(children, 50, {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return standaloneComponent("3:51", "Item")
      },
    })
    const requested = page("0:1", "Requested", children)
    installFigma({
      currentPage: page("0:2", "Current"),
      pages: [requested],
    })

    await expect(
      getComponents(
        { selector: { pageId: requested.id } },
        cancellation.signal,
      ),
    ).rejects.toThrow("Operation cancelled")
  })

  test("components and instances share one returned-node ceiling", async () => {
    const icon = standaloneComponent("3:1", "Icon")
    const first = instance({ id: "5:1", name: "One", main: { id: "3:1" } })
    const second = instance({ id: "5:2", name: "Two", main: { id: "3:1" } })
    installFigma({
      currentPage: page("0:2", "Current", [icon, first, second]),
    })

    const result = await getComponents({}, undefined, { returnedNodes: 2 })

    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
    expect(result.instances).toEqual([
      { instanceId: "5:1", componentId: "3:1" },
    ])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })
    expect(result.observation.completedAt).toMatch(/Z$/)
  })

  test("a truncated walk outranks the emission cut", async () => {
    const icons = [1, 2, 3, 4].map((index) =>
      standaloneComponent(`3:${index}`, `Icon ${index}`),
    )
    installFigma({ currentPage: page("0:2", "Current", icons) })

    const result = await getComponents({}, undefined, {
      visitedNodes: 4,
      returnedNodes: 1,
    })

    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 4 })
  })

  test("the byte ceiling counts every emitted payload and reports the total", async () => {
    const icons = [1, 2].map((index) =>
      standaloneComponent(`3:${index}`, `Icon ${index}`),
    )
    installFigma({ currentPage: page("0:2", "Current", icons) })
    const serialized = (index: number) => ({
      id: `3:${index}`,
      name: `Icon ${index}`,
      documentation: [],
      variantProperties: [],
      propertyDefinitions: [
        {
          name: "Label#1:0",
          defaultValue: { kind: "text" as const, value: "icon" },
        },
      ],
      description: "Icon",
    })
    const budget = byteLength(serialized(1)) + byteLength(serialized(2)) - 1

    const result = await getComponents({}, undefined, { encodedBytes: budget })

    expect(result.components).toEqual([serialized(1)])
    expect(result.truncation).toEqual({
      reason: "byteLimit",
      encodedBytes: byteLength(serialized(1)) + byteLength(serialized(2)),
    })
  })

  test("a component reached twice in one walk is emitted once", async () => {
    const icon = standaloneComponent("3:1", "Icon")
    installFigma({ currentPage: page("0:2", "Current", [icon, icon]) })

    const result = await getComponents({})

    expect(result.components.map((item) => item.id)).toEqual(["3:1"])
  })

  test("main components are looked up sixteen at a time, and the pass stops at the ceiling", async () => {
    const lookups: string[] = []
    const instances = Array.from({ length: 20 }, (_, index) => ({
      id: `5:${index + 1}`,
      name: `Instance ${index + 1}`,
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => {
        lookups.push(`5:${index + 1}`)
        return { id: "3:1" }
      },
    }))
    installFigma({ currentPage: page("0:2", "Current", instances) })

    const result = await getComponents({}, undefined, { returnedNodes: 1 })

    expect(lookups).toHaveLength(16)
    expect(result.instances).toHaveLength(1)
    expect(result.truncated).toBe(true)
  })

  test("an exhausted main-component budget stops before the first batch", async () => {
    const lookups: string[] = []
    const instances = [1, 2, 3].map((index) => ({
      id: `5:${index}`,
      name: `Instance ${index}`,
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => {
        lookups.push(`5:${index}`)
        return { id: "3:1" }
      },
    }))
    installFigma({ currentPage: page("0:2", "Current", instances) })

    const result = await getComponents({}, undefined, {
      mainComponentBudgetMs: 0,
    })

    expect(lookups).toEqual([])
    expect(result.instances).toEqual([])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 0 })

    // With a walk that also ran out, the walk's count is the one kept: the
    // budget marks second and must not overwrite it.
    const walked = await getComponents({}, undefined, {
      mainComponentBudgetMs: 0,
      visitedNodes: 2,
    })
    expect(walked.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 2 })
  })

  test("a budget that runs out during a batch truncates after it", async () => {
    const one = {
      id: "5:1",
      name: "One",
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => {
        await new Promise((resolve) => setTimeout(resolve, 30))
        return { id: "3:1" }
      },
    }
    installFigma({ currentPage: page("0:2", "Current", [one]) })

    const result = await getComponents({}, undefined, {
      mainComponentBudgetMs: 10,
    })

    expect(result.instances).toEqual([
      { instanceId: "5:1", componentId: "3:1" },
    ])
    expect(result.truncated).toBe(true)
  })

  test("an off-page main component that never resolves is skipped, not awaited", async () => {
    const one = instance({ id: "5:1", name: "One", main: { id: "9:9" } })
    installFigma({
      currentPage: page("0:2", "Current", [one]),
      getNodeByIdAsync: async (id: string) =>
        id === "9:9" ? new Promise(() => {}) : one,
    })

    const result = await getComponents({ selector: { nodeId: "5:1" } })

    expect(result.instances).toEqual([
      { instanceId: "5:1", componentId: "9:9" },
    ])
    expect(result.components).toEqual([])
  })

  test("an off-page lookup that throws costs only that component", async () => {
    const one = instance({ id: "5:1", name: "One", main: { id: "9:9" } })
    installFigma({
      currentPage: page("0:2", "Current", [one]),
      // Synchronous, not a rejected promise: settleOrSkip swallows the latter,
      // so only this shape reaches the catch under test.
      getNodeByIdAsync: ((id: string) => {
        if (id === "9:9") throw new Error("library unreachable")
        return Promise.resolve(null)
      }) as (id: string) => Promise<unknown>,
    })

    const result = await getComponents({})

    expect(result.components).toEqual([])
    expect(result.instances).toEqual([
      { instanceId: "5:1", componentId: "9:9" },
    ])
  })

  // A rejected promise is swallowed by settleOrSkip, so only a *synchronous*
  // throw from the host method reaches the guard that rethrows read errors.
  test("the off-page pass stops before its batch when the budget is already out", async () => {
    const one = {
      id: "5:1",
      name: "One",
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => {
        await new Promise((resolve) => setTimeout(resolve, 30))
        return { id: "9:9" }
      },
    }
    const offPage = standaloneComponent("9:9", "Off page")
    const harness = installFigma({
      currentPage: page("0:2", "Current", [one]),
      nodes: new Map<string, unknown>([["9:9", offPage]]),
    })

    const result = await getComponents({}, undefined, {
      mainComponentBudgetMs: 10,
    })

    expect(result.instances).toEqual([
      { instanceId: "5:1", componentId: "9:9" },
    ])
    expect(harness.lookedUp).toEqual([])
    expect(result.components).toEqual([])
  })

  test("the off-page pass stops after a batch that ran the budget out", async () => {
    const one = instance({ id: "5:1", name: "One", main: { id: "9:9" } })
    const offPage = standaloneComponent("9:9", "Off page")
    installFigma({
      currentPage: page("0:2", "Current", [one]),
      getNodeByIdAsync: async (id: string) => {
        if (id !== "9:9") return null
        await new Promise((resolve) => setTimeout(resolve, 30))
        return offPage
      },
    })

    const result = await getComponents({}, undefined, {
      mainComponentBudgetMs: 20,
    })

    expect(result.components.map((item) => item.id)).toEqual(["9:9"])
    expect(result.truncated).toBe(true)
  })

  test("the off-page pass stops once the emission ceiling is hit", async () => {
    const first = instance({ id: "5:1", name: "One", main: { id: "9:9" } })
    const second = instance({ id: "5:2", name: "Two", main: { id: "9:9" } })
    const offPage = standaloneComponent("9:9", "Off page")
    const harness = installFigma({
      currentPage: page("0:2", "Current", [first, second]),
      nodes: new Map<string, unknown>([["9:9", offPage]]),
    })

    const result = await getComponents({}, undefined, { returnedNodes: 1 })

    expect(result.instances).toHaveLength(1)
    expect(harness.lookedUp).toEqual([])
    expect(result.components).toEqual([])
  })

  test("a read error thrown synchronously by a main-component lookup ends the whole call", async () => {
    const one = {
      id: "5:1",
      name: "One",
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: (): Promise<unknown> => {
        throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
      },
    }
    installFigma({ currentPage: page("0:2", "Current", [one]) })

    await expect(getComponents({})).rejects.toBeInstanceOf(PluginReadError)
  })

  test("a read error raised while serializing a component ends the whole call", async () => {
    const hostile = {
      id: "3:1",
      type: "COMPONENT",
      visible: true,
      children: [],
      get name(): string {
        throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
      },
    }
    installFigma({ currentPage: page("0:2", "Current", [hostile]) })

    await expect(getComponents({})).rejects.toBeInstanceOf(PluginReadError)
  })

  test("a component set whose variantProperties cannot be enumerated is still emitted", async () => {
    const hostile = {
      id: "2:1",
      name: "Button",
      type: "COMPONENT_SET",
      visible: true,
      children: [],
      description: "",
      documentationLinks: [],
      componentPropertyDefinitions: {},
      variantProperties: new Proxy(
        {},
        {
          ownKeys() {
            throw new Error("variantProperties is not enumerable")
          },
        },
      ),
    }
    installFigma({ currentPage: page("0:2", "Current", [hostile]) })

    const result = await getComponents({})

    expect(result.components).toEqual([
      {
        id: "2:1",
        name: "Button",
        documentation: [],
        variantProperties: [],
        propertyDefinitions: [],
      },
    ])
  })

  // `throwIfAbortedAtBatch` polls only when `index % 100 === 0`, and the
  // instance pass steps sixteen at a time, so an abort raised during the first
  // batch is seen by nothing but the unconditional check beside it.
  test("the instance pass checks cancellation on every batch", async () => {
    const cancellation = new LocalCancellationController()
    const icon = standaloneComponent("3:1", "Icon")
    const instances = Array.from({ length: 17 }, (_, index) => ({
      id: `5:${index + 1}`,
      name: `Instance ${index + 1}`,
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => {
        if (index === 0) cancellation.abort()
        return { id: "3:1" }
      },
    }))
    installFigma({
      currentPage: page("0:2", "Current", [icon, ...instances]),
    })

    await expect(getComponents({}, cancellation.signal)).rejects.toThrow(
      "Operation cancelled",
    )
  })

  test("the component loop checks cancellation on every component", async () => {
    const cancellation = new LocalCancellationController()
    // Not `parent`: `visibilityOf` reads that during the walk, and the read
    // would then reject before ever reaching the loop under test.
    // `variantProperties` is read only by serializeComponent.
    const first = standaloneComponent("3:1", "First") as Record<string, unknown>
    Object.defineProperty(first, "variantProperties", {
      configurable: true,
      enumerable: true,
      get() {
        cancellation.abort()
        return null
      },
    })
    installFigma({
      currentPage: page("0:2", "Current", [
        first,
        standaloneComponent("3:2", "Second"),
      ]),
    })

    await expect(getComponents({}, cancellation.signal)).rejects.toThrow(
      "Operation cancelled",
    )
  })

  test("the off-page pass checks cancellation on every batch", async () => {
    const cancellation = new LocalCancellationController()
    const instances = Array.from({ length: 17 }, (_, index) => ({
      id: `5:${index + 1}`,
      name: `Instance ${index + 1}`,
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => ({ id: `9:${index + 1}` }),
    }))
    installFigma({
      currentPage: page("0:2", "Current", instances),
      // Seventeen off-page ids, so the pass takes two batches of sixteen; the
      // abort lands inside the first, where the batched poll never looks.
      getNodeByIdAsync: async (id: string) => {
        if (id === "9:1") cancellation.abort()
        return null
      },
    })

    await expect(getComponents({}, cancellation.signal)).rejects.toThrow(
      "Operation cancelled",
    )
  })

  test("the component loop stops at the emission ceiling", async () => {
    const icons = [1, 2, 3, 4].map((index) =>
      standaloneComponent(`3:${index}`, `Icon ${index}`),
    )
    installFigma({ currentPage: page("0:2", "Current", icons) })

    const result = await getComponents({}, undefined, { returnedNodes: 2 })

    expect(result.components.map((item) => item.id)).toEqual(["3:1", "3:2"])
    expect(result.instances).toEqual([])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })
  })

  test("the instance batch stops at the emission ceiling", async () => {
    const instances = [1, 2, 3, 4].map((index) => ({
      id: `5:${index}`,
      name: `Instance ${index}`,
      type: "INSTANCE",
      visible: true,
      children: [],
      getMainComponentAsync: async () => ({ id: "3:1" }),
    }))
    installFigma({ currentPage: page("0:2", "Current", instances) })

    const result = await getComponents({}, undefined, { returnedNodes: 2 })

    expect(result.components).toEqual([])
    expect(result.instances.map((item) => item.instanceId)).toEqual([
      "5:1",
      "5:2",
    ])
    expect(result.truncated).toBe(true)
    expect(result.truncation).toEqual({ reason: "nodeLimit", visitedNodes: 3 })
  })

  test("property definitions that cannot be enumerated cost that field only", async () => {
    const hostile = {
      id: "3:1",
      name: "Icon",
      type: "COMPONENT",
      visible: true,
      children: [],
      description: "",
      documentationLinks: [],
      variantProperties: null,
      componentPropertyDefinitions: new Proxy(
        {},
        {
          ownKeys() {
            throw new Error("componentPropertyDefinitions is not enumerable")
          },
        },
      ),
    }
    installFigma({ currentPage: page("0:2", "Current", [hostile]) })

    const result = await getComponents({})

    expect(result.components).toEqual([
      {
        id: "3:1",
        name: "Icon",
        documentation: [],
        variantProperties: [],
        propertyDefinitions: [],
      },
    ])
  })
})
