import { describe, expect, test } from "bun:test"

import {
  CancellationRegistry,
  LocalCancellationController,
} from "./cancellation"
import { dispatchControllerMessage, requestBoundaryFailure } from "./dispatch"
import type { TraversalGate } from "./traversal-gate"
import {
  OPERATION_NAMES,
  parseControllerBoundMessage,
} from "../shared/validation"

const controllerRequestId = (index: number): string =>
  `123e4567-e89b-42d3-a456-${String(index).padStart(12, "0")}`

const EMPTY_INPUTS: Record<
  (typeof OPERATION_NAMES)[number],
  Record<string, unknown>
> = {
  get_metadata: {},
  get_selection: {},
  get_nodes: { nodeIds: [] },
  search_nodes: {
    scope: { pageId: "0:1" },
    query: "Card",
    match: "contains",
    limit: 50,
  },
  get_design_context: {},
  get_styles: {},
  get_variables: {},
  get_components: {},
  get_fonts: {},
  get_dev_mode_data: {},
  get_reactions: {},
  get_motion: {},
  get_screenshot: { format: "png", selector: { nodeId: "1:2" } },
}

describe("closed read dispatcher", () => {
  test("every named milestone operation returns a typed unavailable error", async () => {
    for (const [index, operation] of OPERATION_NAMES.entries()) {
      if (
        operation === "get_metadata" ||
        operation === "get_selection" ||
        operation === "get_nodes" ||
        operation === "search_nodes" ||
        operation === "get_design_context" ||
        operation === "get_styles" ||
        operation === "get_variables" ||
        operation === "get_components" ||
        operation === "get_fonts" ||
        operation === "get_dev_mode_data" ||
        operation === "get_reactions" ||
        operation === "get_motion" ||
        operation === "get_screenshot"
      )
        continue
      const correlationId = controllerRequestId(index)
      const request = parseControllerBoundMessage({
        type: "request",
        controllerRequestId: correlationId,
        requestId: `plugin-${index}`,
        deadlineMs: 1,
        target: {},
        operation: { operation, input: EMPTY_INPUTS[operation] },
      })
      expect(request.type).toBe("request")
      if (request.type !== "request")
        throw new Error("test request did not decode")

      expect(await dispatchControllerMessage(request)).toEqual({
        type: "error",
        controllerRequestId: correlationId,
        requestId: `plugin-${index}`,
        error: { code: "CAPABILITY_UNAVAILABLE", retryable: false },
      })
    }
  })

  test("routes every operation through its closed traversal policy", async () => {
    const leases: string[] = []
    const gate: TraversalGate = {
      read: async <T>(run: () => Promise<T>): Promise<T> => {
        leases.push("read")
        return run()
      },
      includeHidden: async <T>(run: () => Promise<T>): Promise<T> => {
        leases.push("includeHidden")
        return run()
      },
    }
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: { name: "Checkout flow", children: [] },
      currentPage: { id: "0:1", name: "Page 1", selection: [] },
      editorType: "dev",
      getNodeByIdAsync: async () => null,
    }

    for (const [index, operation] of OPERATION_NAMES.entries()) {
      const input =
        operation === "get_design_context"
          ? { ...EMPTY_INPUTS[operation], includeHidden: true }
          : EMPTY_INPUTS[operation]
      const request = parseControllerBoundMessage({
        type: "request",
        controllerRequestId: controllerRequestId(200 + index),
        requestId: `policy-${index}`,
        deadlineMs: 1,
        target: {},
        operation: { operation, input },
      })
      if (request.type !== "request") throw new Error("request did not decode")
      await dispatchControllerMessage(request, new CancellationRegistry(), gate)
    }

    expect(leases).toEqual([
      "read",
      "read",
      "read",
      "includeHidden",
      "read",
      "read",
      "read",
      "read",
      "read",
      "read",
      "read",
      "read",
    ])
  })

  test("get_metadata returns bounded file and page metadata", async () => {
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: {
        name: "Checkout flow",
        children: [{ id: "0:1", name: "Page 1" }],
      },
      currentPage: { id: "0:1", name: "Page 1" },
      editorType: "dev",
    }
    const request = parseControllerBoundMessage({
      type: "request",
      controllerRequestId: controllerRequestId(100),
      requestId: "plugin-metadata",
      deadlineMs: 1,
      target: {},
      operation: { operation: "get_metadata", input: {} },
    })
    if (request.type !== "request") throw new Error("request did not decode")
    await expect(dispatchControllerMessage(request)).resolves.toMatchObject({
      type: "response",
      requestId: "plugin-metadata",
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

  test("duplicate active correlation IDs are rejected and cancellation is idempotent", async () => {
    const registry = new CancellationRegistry()
    const correlationId = controllerRequestId(99)
    registry.begin(correlationId)
    const request = parseControllerBoundMessage({
      type: "request",
      controllerRequestId: correlationId,
      requestId: "plugin-1",
      deadlineMs: 1,
      target: {},
      operation: { operation: "get_metadata", input: {} },
    })
    if (request.type !== "request")
      throw new Error("test request did not decode")

    expect(await dispatchControllerMessage(request, registry)).toEqual({
      type: "error",
      controllerRequestId: correlationId,
      requestId: "plugin-1",
      error: { code: "INTERNAL_ERROR", retryable: false },
    })
    expect(registry.cancel(correlationId)).toBe(true)
    expect(registry.cancel(correlationId)).toBe(false)
    registry.finish(correlationId)
    expect(registry.size).toBe(0)
  })

  test("local cancellation signal works without ambient AbortController", () => {
    const controller = new LocalCancellationController()
    let notifications = 0
    const listener = (): void => {
      notifications += 1
    }
    controller.signal.addEventListener("abort", listener)
    expect(controller.signal.aborted).toBe(false)

    controller.abort()
    controller.abort()

    expect(controller.signal.aborted).toBe(true)
    expect(notifications).toBe(1)
    expect(() => controller.signal.throwIfAborted()).toThrow()
    controller.signal.removeEventListener("abort", listener)
  })

  test("one failing cancellation listener cannot block later listeners", () => {
    const controller = new LocalCancellationController()
    let notifications = 0
    controller.signal.addEventListener("abort", () => {
      throw new Error("listener failure")
    })
    controller.signal.addEventListener("abort", () => {
      notifications += 1
    })

    expect(() => controller.abort()).not.toThrow()
    expect(controller.signal.aborted).toBe(true)
    expect(notifications).toBe(1)
  })

  test("emits a schema-safe reading progress frame for a plugin-routed request", async () => {
    const posted: unknown[] = []
    ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
      root: {
        name: "Checkout flow",
        children: [{ id: "0:1", name: "Page 1" }],
      },
      currentPage: { id: "0:1", name: "Page 1" },
      editorType: "dev",
      ui: {
        postMessage(message: unknown) {
          posted.push(message)
        },
      },
    }
    const request = parseControllerBoundMessage({
      type: "request",
      controllerRequestId: controllerRequestId(101),
      requestId: "plugin-progress",
      deadlineMs: 1,
      target: {},
      operation: { operation: "get_metadata", input: {} },
    })
    if (request.type !== "request") throw new Error("request did not decode")

    await expect(dispatchControllerMessage(request)).resolves.toMatchObject({
      type: "response",
      requestId: "plugin-progress",
    })
    expect(posted).toContainEqual({
      type: "progress",
      controllerRequestId: controllerRequestId(101),
      requestId: "plugin-progress",
      completed: 0,
      message: "reading",
    })
  })

  test("request boundary maps local cancellation without exposing exception text", () => {
    const controller = new LocalCancellationController()
    controller.abort()
    let cancellation: unknown
    try {
      controller.signal.throwIfAborted()
    } catch (error: unknown) {
      cancellation = error
    }

    expect(requestBoundaryFailure(cancellation)).toEqual({
      code: "CANCELLED",
      retryable: false,
    })
    expect(requestBoundaryFailure(new Error("private details"))).toEqual({
      code: "INTERNAL_ERROR",
      retryable: false,
    })
  })
})
