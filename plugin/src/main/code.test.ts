import { afterAll, describe, expect, test } from "bun:test"

// `code.ts` is the controller entry point: it is what Figma loads, and every
// message the iframe sends arrives at the `figma.ui.onmessage` it installs.
// Nothing imported it before this file, so the whole module — the transport
// unwrapping, the readiness answer, and the dispatch hand-off — was unpinned.
// It runs its side effects at import time, so the host has to be installed
// before the import rather than inside a test.

const posted: unknown[] = []
const shown: { html: unknown; options: unknown }[] = []

const PANEL_HTML = "<!doctype html><title>plugin ui</title>"

interface FigmaUi {
  postMessage(message: unknown): void
  onmessage?: (input: unknown) => Promise<void>
}

const ui: FigmaUi = {
  postMessage(message: unknown) {
    posted.push(message)
  },
}

// Written through an untyped view of `globalThis`: the real declarations are
// `PluginAPI` and a `const` string, and this file's whole point is to stand a
// fake in their place before `code.ts` reads them at import time.
//
// Bun shares one runtime across test files, so a host installed here would
// otherwise outlive the file and become an ordering coupling for whatever runs
// next. The previous values are captured here and put back in `afterAll`;
// `HAD_*` records whether the key existed at all, so a key this file invented
// is deleted rather than set to `undefined`.
const hostGlobals = globalThis as unknown as Record<string, unknown>
const PREVIOUS = { html: hostGlobals.__html__, figma: hostGlobals.figma }
const HAD = {
  html: Object.hasOwn(hostGlobals, "__html__"),
  figma: Object.hasOwn(hostGlobals, "figma"),
}

afterAll(() => {
  if (HAD.html) hostGlobals.__html__ = PREVIOUS.html
  else delete hostGlobals.__html__
  if (HAD.figma) hostGlobals.figma = PREVIOUS.figma
  else delete hostGlobals.figma
})

hostGlobals.__html__ = PANEL_HTML
hostGlobals.figma = {
  root: { name: "Checkout flow", children: [{ id: "0:1", name: "Page 1" }] },
  currentPage: {
    id: "0:1",
    name: "Page 1",
    getDevResourcesAsync: async () => [],
  },
  editorType: "dev",
  annotations: {},
  motion: {},
  variables: {},
  showUI: (html: unknown, options: unknown) => {
    shown.push({ html, options })
  },
  ui,
}

await import("./code")

const metadataRequestId = "123e4567-e89b-42d3-a456-426614174000"
const correlationId = (suffix: string): string =>
  `123e4567-e89b-42d3-a456-${suffix}`

async function deliver(input: unknown): Promise<void> {
  const handler = ui.onmessage
  if (handler === undefined) {
    throw new Error("code.ts did not install figma.ui.onmessage")
  }
  await handler(input)
}

function drain(): unknown[] {
  const taken = [...posted]
  posted.length = 0
  return taken
}

function request(id: string, requestId: string): Record<string, unknown> {
  return {
    type: "request",
    controllerRequestId: id,
    requestId,
    deadlineMs: 1,
    target: {},
    operation: { operation: "get_metadata", input: {} },
  }
}

describe("controller entry point", () => {
  test("shows the bundled UI at the declared panel size", () => {
    expect(shown).toEqual([
      {
        html: PANEL_HTML,
        options: { width: 240, height: 96, themeColors: true },
      },
    ])
  })

  test("answers a readiness request with the file's own identity", async () => {
    drain()
    await deliver({ type: "requestControllerReady", metadataRequestId })

    expect(drain()).toEqual([
      {
        type: "controllerReady",
        metadataRequestId,
        fileName: "Checkout flow",
        currentPage: { id: "0:1", name: "Page 1" },
        editorType: "dev",
        pluginVersion: "0.1.0",
        capabilities: {
          annotations: true,
          devResources: true,
          motion: true,
          svgStringExport: true,
          variableCodeSyntax: true,
        },
      },
    ])
  })

  test("accepts a readiness request in each transport shape the host uses", async () => {
    const shapes: unknown[] = [
      { type: "requestControllerReady", metadataRequestId },
      { pluginMessage: { type: "requestControllerReady", metadataRequestId } },
      JSON.stringify({ type: "requestControllerReady", metadataRequestId }),
    ]
    drain()
    for (const shape of shapes) await deliver(shape)

    const answers = drain()
    expect(answers).toHaveLength(shapes.length)
    for (const answer of answers) {
      expect(answer).toMatchObject({
        type: "controllerReady",
        metadataRequestId,
      })
    }
  })

  test("dispatches a well-formed request and posts the response back", async () => {
    drain()
    await deliver(request(correlationId("000000000301"), "plugin-entry-1"))

    const messages = drain()
    expect(messages).toHaveLength(2)
    expect(messages[0]).toEqual({
      type: "progress",
      controllerRequestId: correlationId("000000000301"),
      requestId: "plugin-entry-1",
      completed: 0,
      message: "reading",
    })
    expect(messages[1]).toMatchObject({
      type: "response",
      controllerRequestId: correlationId("000000000301"),
      requestId: "plugin-entry-1",
      result: {
        operation: "get_metadata",
        result: {
          file: { name: "Checkout flow", editorType: "dev" },
          pages: [{ id: "0:1", name: "Page 1" }],
          currentPageId: "0:1",
          pluginVersion: "0.1.0",
          truncated: false,
        },
      },
    })
  })

  test("a cancel is dispatched and answered with no message at all", async () => {
    drain()
    await deliver({
      type: "cancel",
      controllerRequestId: correlationId("000000000302"),
      requestId: "plugin-entry-2",
    })

    expect(drain()).toEqual([])
  })
})
