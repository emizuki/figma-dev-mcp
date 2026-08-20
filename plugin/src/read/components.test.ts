import { beforeEach, describe, expect, test } from "bun:test"

import { LocalCancellationController } from "../main/cancellation"
import { PluginReadError } from "./navigation"
import { getComponents } from "./components"

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

function installFigma(options: {
  currentPage: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
}): {
  currentPage: Record<string, unknown>
  lookedUp: string[]
  loadedPages: string[]
} {
  const lookedUp: string[] = []
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
  const api = {
    root: { name: "Checkout flow", children: pages },
    currentPage: current,
    editorType: "dev",
    loadAllPagesAsync: async () => {
      throw new Error("components must not load every page")
    },
    importComponentByKeyAsync: async () => {
      throw new Error("components must not import remotes")
    },
    getNodeByIdAsync: async (id: string) => {
      lookedUp.push(id)
      if (nodes.has(id)) return nodes.get(id)
      return pages.find((item) => item.id === id) ?? null
    },
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
  return { currentPage: current, lookedUp, loadedPages }
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
})
