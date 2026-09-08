import { describe, expect, test } from "bun:test"

import {
  awaitWithSignal,
  CancellationRegistry,
  ignoreSettlement,
  LocalCancellationController,
  LocalCancellationError,
  throwIfAbortedAtBatch,
} from "./cancellation"
import { dispatchControllerMessage, requestBoundaryFailure } from "./dispatch"
import { PluginReadError } from "../read/navigation"
import { progressFor } from "./progress"
import { installFigma } from "../../tests/figma-harness"
import {
  OPERATION_NAMES,
  parseControllerBoundMessage,
} from "../shared/validation"

const controllerRequestId = (index: number): string =>
  `123e4567-e89b-42d3-a456-${String(index).padStart(12, "0")}`

// A host `exportAsync` that never settles, standing in for a large export the
// operator cancels partway through. Read code has no obligation to poll
// `signal` mid-await, so this is the case that can only be cancelled at the
// request boundary, not inside the read.
function installHangingScreenshot(): void {
  const node = {
    id: "4:1",
    type: "FRAME",
    exportAsync: () => new Promise(() => {}),
  }
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = {
    root: { name: "Checkout flow", children: [{ id: "0:1", name: "Page 1" }] },
    currentPage: { id: "0:1", name: "Page 1" },
    editorType: "dev",
    getNodeByIdAsync: async (id: string) => (id === "4:1" ? node : null),
  }
}

// Settles a promise or a bounded timeout, whichever comes first, so a test
// can assert "resolved promptly" without hanging the suite when it does not.
function raceAgainstTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
): Promise<{ settled: true; value: T } | { settled: false }> {
  return Promise.race([
    promise.then((value) => ({ settled: true as const, value })),
    new Promise<{ settled: false }>((resolve) =>
      setTimeout(() => resolve({ settled: false }), timeoutMs),
    ),
  ])
}

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

  test("cancelling a read that never resolves settles the dispatch as CANCELLED promptly", async () => {
    installHangingScreenshot()
    const registry = new CancellationRegistry()
    const correlationId = controllerRequestId(200)
    const request = parseControllerBoundMessage({
      type: "request",
      controllerRequestId: correlationId,
      requestId: "plugin-hang",
      deadlineMs: 1,
      target: {},
      operation: {
        operation: "get_screenshot",
        input: { format: "png", selector: { nodeId: "4:1" } },
      },
    })
    if (request.type !== "request")
      throw new Error("test request did not decode")

    const dispatched = dispatchControllerMessage(request, registry)
    // Give dispatchRead a turn to actually call the hanging exportAsync
    // before cancelling, so this exercises cancelling mid-export rather than
    // cancelling before the read has started.
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()
    registry.cancel(correlationId)

    const settled = await raceAgainstTimeout(dispatched, 200)

    // Without the fix this never settles within the timeout: dispatchRead
    // only awaits the hanging exportAsync call directly, and nothing in that
    // path polls the signal, so the dispatch promise hangs until the export
    // itself resolves — which, for this test's exporter, is never.
    expect(settled).toEqual({
      settled: true,
      value: {
        type: "error",
        controllerRequestId: correlationId,
        requestId: "plugin-hang",
        error: { code: "CANCELLED", retryable: false },
      },
    })
  })

  // Every operation below is a separate `case` in `dispatchRead`. Deleting any
  // one of them drops the request through to `assertNever`, which throws and
  // comes back as INTERNAL_ERROR — a request the plugin can serve answered as
  // if the plugin were broken. Eleven of the thirteen cases had nothing
  // holding them.
  test("every read operation reaches the reader named in the request", async () => {
    installFigma({})
    let routed = 0
    for (const [index, operation] of OPERATION_NAMES.entries()) {
      const request = parseControllerBoundMessage({
        type: "request",
        controllerRequestId: controllerRequestId(400 + index),
        requestId: `plugin-route-${operation}`,
        deadlineMs: 1,
        target: {},
        operation: { operation, input: EMPTY_INPUTS[operation] },
      })
      if (request.type !== "request")
        throw new Error("test request did not decode")

      const answer = await dispatchControllerMessage(request)
      expect({ operation, answer }).toMatchObject({
        operation,
        answer: {
          type: "response",
          requestId: `plugin-route-${operation}`,
          result: { operation },
        },
      })
      routed += 1
    }
    // Positive control: the loop above asserts nothing if it never runs, and
    // the one pre-existing test over `OPERATION_NAMES` skips every entry.
    expect(routed).toBe(OPERATION_NAMES.length)
    expect(routed).toBe(13)
  })

  test("a cancel message aborts the request it names and answers nothing", async () => {
    installHangingScreenshot()
    const registry = new CancellationRegistry()
    const correlationId = controllerRequestId(500)
    const request = parseControllerBoundMessage({
      type: "request",
      controllerRequestId: correlationId,
      requestId: "plugin-cancel-message",
      deadlineMs: 1,
      target: {},
      operation: {
        operation: "get_screenshot",
        input: { format: "png", selector: { nodeId: "4:1" } },
      },
    })
    const cancel = parseControllerBoundMessage({
      type: "cancel",
      controllerRequestId: correlationId,
      requestId: "plugin-cancel-message",
    })
    if (request.type !== "request" || cancel.type !== "cancel")
      throw new Error("test messages did not decode")

    const dispatched = dispatchControllerMessage(request, registry)
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()

    // The cancel travels the same way a request does — through
    // `dispatchControllerMessage` — rather than by reaching into the registry.
    expect(await dispatchControllerMessage(cancel, registry)).toBeNull()

    expect(await raceAgainstTimeout(dispatched, 200)).toEqual({
      settled: true,
      value: {
        type: "error",
        controllerRequestId: correlationId,
        requestId: "plugin-cancel-message",
        error: { code: "CANCELLED", retryable: false },
      },
    })
  })

  test("a settled request frees its correlation ID for the next one", async () => {
    installFigma({})
    const registry = new CancellationRegistry()
    const correlationId = controllerRequestId(501)
    const build = (requestId: string) => {
      const message = parseControllerBoundMessage({
        type: "request",
        controllerRequestId: correlationId,
        requestId,
        deadlineMs: 1,
        target: {},
        operation: { operation: "get_metadata", input: {} },
      })
      if (message.type !== "request")
        throw new Error("test request did not decode")
      return message
    }

    const first = await dispatchControllerMessage(
      build("plugin-reuse-1"),
      registry,
    )
    const second = await dispatchControllerMessage(
      build("plugin-reuse-2"),
      registry,
    )

    // Both answers are responses: the second would be INTERNAL_ERROR if the
    // first had left its identifier in the registry.
    expect(first).toMatchObject({
      type: "response",
      controllerRequestId: correlationId,
      requestId: "plugin-reuse-1",
    })
    expect(second).toMatchObject({
      type: "response",
      controllerRequestId: correlationId,
      requestId: "plugin-reuse-2",
    })
    expect(registry.size).toBe(0)
  })

  test("a read failure keeps its own code and retryable flag at the boundary", () => {
    expect(
      requestBoundaryFailure(new PluginReadError("NODE_NOT_FOUND", false)),
    ).toEqual({ code: "NODE_NOT_FOUND", retryable: false })
    expect(
      requestBoundaryFailure(new PluginReadError("CONNECTION_LOST", true)),
    ).toEqual({ code: "CONNECTION_LOST", retryable: true })
  })

  test("read code finds the reporter bound to its signal, and its totals reach the controller", async () => {
    const posted: unknown[] = []
    const harness = installFigma({
      ui: {
        postMessage(message: unknown) {
          posted.push(message)
        },
      },
    })
    void harness
    const correlationId = controllerRequestId(502)
    const request = parseControllerBoundMessage({
      type: "request",
      controllerRequestId: correlationId,
      requestId: "plugin-bound-progress",
      deadlineMs: 1,
      target: {},
      operation: { operation: "get_metadata", input: {} },
    })
    if (request.type !== "request")
      throw new Error("test request did not decode")

    const controller = new LocalCancellationController()
    const registry = new CancellationRegistry()
    // Stand in for the registry's controller so the test holds the very signal
    // the dispatcher binds the reporter to.
    const begin = registry.begin.bind(registry)
    registry.begin = () => {
      const real = begin(correlationId)
      void real
      return controller
    }

    await dispatchControllerMessage(request, registry)

    const reporter = progressFor(controller.signal)
    expect(reporter).toBeDefined()
    reporter?.tick("encoding", 3, 7)

    expect(posted).toContainEqual({
      type: "progress",
      controllerRequestId: correlationId,
      requestId: "plugin-bound-progress",
      completed: 3,
      total: 7,
      message: "encoding",
    })
  })

  test("the batch cancellation gate throws on a batch boundary and only there", () => {
    const controller = new LocalCancellationController()
    controller.abort()

    expect(() => throwIfAbortedAtBatch(controller.signal, 0, 100)).toThrow()
    expect(() => throwIfAbortedAtBatch(controller.signal, 100, 100)).toThrow()
    expect(() => throwIfAbortedAtBatch(controller.signal, 250, 50)).toThrow()
    expect(() => throwIfAbortedAtBatch(controller.signal, 1, 100)).not.toThrow()
    expect(() =>
      throwIfAbortedAtBatch(controller.signal, 99, 100),
    ).not.toThrow()
    expect(() =>
      throwIfAbortedAtBatch(controller.signal, 101, 100),
    ).not.toThrow()
    // The default batch size is what every call site in `plugin/src/read`
    // relies on when it passes its own constant, so it is pinned here too.
    expect(() => throwIfAbortedAtBatch(controller.signal, 200)).toThrow()
    expect(() => throwIfAbortedAtBatch(controller.signal, 201)).not.toThrow()
  })

  test("awaitWithSignal passes work through, refuses an aborted signal, and unhooks itself", async () => {
    await expect(
      awaitWithSignal(Promise.resolve("no signal"), undefined),
    ).resolves.toBe("no signal")

    const aborted = new LocalCancellationController()
    aborted.abort()
    await expect(
      awaitWithSignal(Promise.resolve("ignored"), aborted.signal),
    ).rejects.toBeInstanceOf(LocalCancellationError)

    const live = new LocalCancellationController()
    await expect(
      awaitWithSignal(Promise.resolve("carried"), live.signal),
    ).resolves.toBe("carried")
    // Nothing is left listening once the work has settled, so a later abort
    // has no rejection to raise.
    expect(() => live.abort()).not.toThrow()
    expect(live.signal.aborted).toBe(true)
  })

  test("cancelAll aborts every active request and reports how many it stopped", () => {
    const registry = new CancellationRegistry()
    const first = registry.begin(controllerRequestId(600))
    const second = registry.begin(controllerRequestId(601))
    registry.cancel(controllerRequestId(601))
    const third = registry.begin(controllerRequestId(602))

    expect(registry.size).toBe(3)
    // The already-cancelled one is not counted twice.
    expect(registry.cancelAll()).toBe(2)
    expect([first, second, third].map((one) => one.signal.aborted)).toEqual([
      true,
      true,
      true,
    ])
    expect(registry.cancelAll()).toBe(0)
  })

  test("a removed abort listener is not called", () => {
    const controller = new LocalCancellationController()
    let kept = 0
    let removed = 0
    const keptListener = (): void => {
      kept += 1
    }
    const removedListener = (): void => {
      removed += 1
    }
    controller.signal.addEventListener("abort", keptListener)
    controller.signal.addEventListener("abort", removedListener)
    controller.signal.removeEventListener("abort", removedListener)

    controller.abort()

    expect(kept).toBe(1)
    expect(removed).toBe(0)
  })

  test("an abandoned promise's rejection is swallowed rather than left unhandled", async () => {
    let reject: ((reason: unknown) => void) | undefined
    const work = new Promise<never>((_, reject_) => {
      reject = reject_
    })
    ignoreSettlement(work)
    reject?.(new Error("late failure"))

    // Nothing here can observe the swallowing directly; what it proves is that
    // the rejection never reaches the runtime as unhandled, which bun reports
    // as a failure of whichever test is running when it fires.
    await Promise.resolve()
    await Promise.resolve()
    expect(true).toBe(true)
  })
})
