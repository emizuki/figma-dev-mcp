import { describe, expect, test } from "bun:test"

import { buildHello } from "./hello"
import { startSocketTransport } from "./socket"
import { BROKER_URL } from "../shared/limits"
import type { ControllerReady } from "../shared/protocol"

const ready: ControllerReady = {
  type: "controllerReady",
  metadataRequestId: "123e4567-e89b-42d3-a456-426614174000",
  fileName: "Checkout",
  currentPage: { id: "0:1", name: "Checkout flow" },
  editorType: "dev",
  pluginVersion: "0.1.0",
  capabilities: {
    annotations: true,
    devResources: true,
    motion: false,
    svgStringExport: true,
    variableCodeSyntax: true,
  },
}

describe("WebSocket hello identity", () => {
  test("every successful socket hello receives a fresh explicit UUID", () => {
    const identities = [
      "123e4567-e89b-42d3-a456-426614174000",
      "123e4567-e89b-42d3-a456-426614174001",
    ]
    const first = buildHello(ready, () => identities.shift() ?? "")
    const second = buildHello(ready, () => identities.shift() ?? "")

    expect(first.connectionId).not.toBe(second.connectionId)
    expect(first.displayName).toBe("Checkout")
    expect(first.currentPage).toEqual(ready.currentPage)
    expect(first.pluginVersion).toBe("0.1.0")
  })

  test("malformed random UUID output is rejected before hello", () => {
    expect(() => buildHello(ready, () => "not-a-uuid")).toThrow()
  })
})

type TestListener = (event: { data?: unknown; source?: unknown }) => void

class FakeWebSocket {
  static readonly OPEN = 1
  static readonly instances: FakeWebSocket[] = []

  readonly sent: string[] = []
  readyState = 0
  #listeners = new Map<string, TestListener[]>()

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this)
  }

  addEventListener(type: string, listener: TestListener): void {
    const listeners = this.#listeners.get(type) ?? []
    listeners.push(listener)
    this.#listeners.set(type, listeners)
  }

  send(data: string): void {
    this.sent.push(data)
  }

  close(): void {
    this.readyState = 3
    this.emit("close")
  }

  open(): void {
    this.readyState = FakeWebSocket.OPEN
    this.emit("open")
  }

  message(data: unknown): void {
    this.emit("message", { data })
  }

  emit(type: string, event: { data?: unknown } = {}): void {
    for (const listener of this.#listeners.get(type) ?? []) listener(event)
  }
}

function installSocketHarness(): {
  controllerMessages: unknown[]
  emitController: (message: unknown) => void
  runNextTimer: () => void
  restore: () => void
  timerDelays: number[]
  statuses: string[]
} {
  FakeWebSocket.instances.length = 0
  const global = globalThis as unknown as {
    WebSocket: unknown
    window: unknown
    parent: unknown
    document: unknown
  }
  const original = {
    WebSocket: global.WebSocket,
    window: global.window,
    parent: global.parent,
    document: global.document,
    setTimeout: globalThis.setTimeout,
    clearTimeout: globalThis.clearTimeout,
  }
  const listeners = new Set<TestListener>()
  const controllerMessages: unknown[] = []
  const timers = new Map<number, () => void>()
  const timerDelays: number[] = []
  const statuses: string[] = []
  let nextTimer = 1

  global.WebSocket = FakeWebSocket
  global.window = {
    addEventListener: (_type: string, listener: TestListener) =>
      listeners.add(listener),
    removeEventListener: (_type: string, listener: TestListener) =>
      listeners.delete(listener),
  }
  const controllerParent = {
    postMessage: (message: unknown) => {
      const envelope = message as { pluginMessage?: unknown }
      controllerMessages.push(envelope.pluginMessage)
    },
  }
  global.parent = controllerParent
  // Real code renders status via `document.getElementById("status").textContent`.
  // Stub just enough of `document` to observe every status the code sets.
  const statusNode = {
    set textContent(value: string) {
      statuses.push(value)
    },
  }
  global.document = {
    getElementById: (id: string) => (id === "status" ? statusNode : null),
  }
  globalThis.setTimeout = ((callback: () => void, delay?: number) => {
    const id = nextTimer
    nextTimer += 1
    timers.set(id, callback)
    timerDelays.push(delay ?? 0)
    return id
  }) as typeof setTimeout
  globalThis.clearTimeout = ((id: number) => {
    // Keep cleared callbacks so the final assertion can exercise a stale timer.
    void id
  }) as typeof clearTimeout

  return {
    controllerMessages,
    timerDelays,
    statuses,
    emitController: (message) => {
      for (const listener of listeners) {
        // Figma's inspect iframe delivers pluginMessage with a null event.source.
        listener({ source: null, data: { pluginMessage: message } })
      }
    },
    runNextTimer: () => {
      const entry = timers.entries().next().value
      if (entry === undefined) throw new Error("no pending timer")
      const [id, callback] = entry
      timers.delete(id)
      callback()
    },
    restore: () => {
      global.WebSocket = original.WebSocket
      global.window = original.window
      global.parent = original.parent
      global.document = original.document
      globalThis.setTimeout = original.setTimeout
      globalThis.clearTimeout = original.clearTimeout
      FakeWebSocket.instances.length = 0
    },
  }
}

function metadataRequestId(message: unknown): string {
  const parsed = message as { metadataRequestId?: unknown }
  if (typeof parsed.metadataRequestId !== "string") {
    throw new Error("controller metadata request must carry an identifier")
  }
  return parsed.metadataRequestId
}

function controllerRequestId(message: unknown): string {
  const parsed = message as { controllerRequestId?: unknown }
  if (typeof parsed.controllerRequestId !== "string") {
    throw new Error("controller request must carry an internal correlation ID")
  }
  return parsed.controllerRequestId
}

function readyFor(metadataRequestId: string): unknown {
  return { ...ready, metadataRequestId }
}

describe("WebSocket reconnect ownership", () => {
  test("drops stale controller output when a replacement socket reuses a broker request ID", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const first = FakeWebSocket.instances[0]
      first?.open()
      const firstMetadata = metadataRequestId(harness.controllerMessages[0])
      harness.emitController(readyFor(firstMetadata))

      first?.message(
        JSON.stringify({
          type: "request",
          requestId: "request-1",
          deadlineMs: 1_000,
          target: {},
          operation: { operation: "get_metadata", input: {} },
        }),
      )
      const firstCorrelationId = controllerRequestId(
        harness.controllerMessages.at(-1),
      )

      first?.close()
      expect(harness.controllerMessages).toContainEqual({
        type: "cancel",
        requestId: "request-1",
        controllerRequestId: firstCorrelationId,
      })
      harness.runNextTimer()
      const second = FakeWebSocket.instances[1]
      second?.open()
      const secondMetadata = metadataRequestId(
        harness.controllerMessages.at(-1),
      )
      harness.emitController(readyFor(secondMetadata))
      expect(second?.sent).toHaveLength(1)

      second?.message(
        JSON.stringify({
          type: "request",
          requestId: "request-1",
          deadlineMs: 1_000,
          target: {},
          operation: { operation: "get_metadata", input: {} },
        }),
      )
      const secondCorrelationId = controllerRequestId(
        harness.controllerMessages.at(-1),
      )
      expect(secondCorrelationId).not.toBe(firstCorrelationId)

      harness.emitController({
        type: "progress",
        requestId: "request-1",
        controllerRequestId: firstCorrelationId,
        completed: 1,
      })
      expect(second?.sent).toHaveLength(1)

      second?.message(
        JSON.stringify({ type: "cancel", requestId: "request-1" }),
      )
      expect(harness.controllerMessages).toContainEqual({
        type: "cancel",
        requestId: "request-1",
        controllerRequestId: secondCorrelationId,
      })
      harness.emitController({
        type: "error",
        requestId: "request-1",
        controllerRequestId: secondCorrelationId,
        error: { code: "CANCELLED", retryable: false },
      })
      expect(second?.sent).toHaveLength(1)

      second?.message(
        JSON.stringify({
          type: "request",
          requestId: "request-1",
          deadlineMs: 1_000,
          target: {},
          operation: { operation: "get_metadata", input: {} },
        }),
      )
      const finalCorrelationId = controllerRequestId(
        harness.controllerMessages.at(-1),
      )
      harness.emitController({
        type: "error",
        requestId: "request-1",
        controllerRequestId: finalCorrelationId,
        error: { code: "CAPABILITY_UNAVAILABLE", retryable: false },
      })
      expect(second?.sent).toHaveLength(2)
      expect(JSON.parse(second?.sent[1] ?? "{}")).toEqual({
        type: "error",
        requestId: "request-1",
        error: { code: "CAPABILITY_UNAVAILABLE", retryable: false },
      })

      stop()
    } finally {
      harness.restore()
    }
  })

  test("keeps metadata and controller results bound to their receiving socket generation", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const first = FakeWebSocket.instances[0]
      expect(first).toBeDefined()
      first?.open()
      const firstMetadata = metadataRequestId(harness.controllerMessages[0])

      first?.close()
      first?.emit("close")
      harness.runNextTimer()
      expect(FakeWebSocket.instances).toHaveLength(2)
      const second = FakeWebSocket.instances[1]
      expect(second).toBeDefined()
      second?.open()
      const secondMetadata = metadataRequestId(harness.controllerMessages[1])

      harness.emitController(readyFor(firstMetadata))
      expect(second?.sent).toEqual([])

      harness.emitController(readyFor(secondMetadata))
      expect(second?.sent).toHaveLength(1)
      expect(JSON.parse(second?.sent[0] ?? "{}").type).toBe("hello")

      second?.message(
        JSON.stringify({
          type: "request",
          requestId: "request-1",
          deadlineMs: 1_000,
          target: {},
          operation: { operation: "get_metadata", input: {} },
        }),
      )
      const secondCorrelationId = controllerRequestId(
        harness.controllerMessages.at(-1),
      )
      expect(harness.controllerMessages).toContainEqual({
        type: "request",
        controllerRequestId: secondCorrelationId,
        requestId: "request-1",
        deadlineMs: 1_000,
        target: {},
        operation: { operation: "get_metadata", input: {} },
      })

      second?.close()
      expect(harness.controllerMessages).toContainEqual({
        type: "cancel",
        controllerRequestId: secondCorrelationId,
        requestId: "request-1",
      })
      harness.runNextTimer()
      expect(FakeWebSocket.instances).toHaveLength(3)
      const third = FakeWebSocket.instances[2]
      expect(third).toBeDefined()
      third?.open()
      const thirdMetadata = metadataRequestId(harness.controllerMessages.at(-1))
      harness.emitController(readyFor(thirdMetadata))
      expect(third?.sent).toHaveLength(1)

      harness.emitController({
        type: "error",
        controllerRequestId: secondCorrelationId,
        requestId: "request-1",
        error: { code: "CANCELLED", retryable: false },
      })
      expect(third?.sent).toHaveLength(1)

      third?.close()
      stop()
      harness.runNextTimer()
      expect(FakeWebSocket.instances).toHaveLength(3)
    } finally {
      harness.restore()
    }
  })
})

describe("connection is reported on acceptance, not on send", () => {
  const driveHandshake = (
    socket: FakeWebSocket,
    harness: ReturnType<typeof installSocketHarness>,
  ) => {
    socket.open()
    const metadata = metadataRequestId(
      harness.controllerMessages[harness.controllerMessages.length - 1],
    )
    harness.emitController(readyFor(metadata))
  }

  test("a handshake the broker rejects backs off along the delay table", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      // Six attempts the broker refuses: each opens, sends hello, and is
      // closed without the broker ever sending a frame back — exactly what a
      // protocol-version mismatch does. Six, not four, so the run reaches
      // past the delay table's last entry (index 4, value 5000) and exercises
      // the `Math.min(reconnectAttempt, RECONNECT_DELAYS_MS.length - 1)` clamp
      // in `scheduleReconnect`: without it, the 6th consecutive rejection
      // would index past the table's end, read `undefined`, and throw
      // "reconnect delay table is empty" from inside the `close` listener —
      // which leaves `reconnectTimer` unset and the plugin never reconnecting
      // again.
      for (let attempt = 0; attempt < 6; attempt += 1) {
        const socket =
          FakeWebSocket.instances[FakeWebSocket.instances.length - 1]
        if (socket === undefined) throw new Error("no socket was opened")
        driveHandshake(socket, harness)
        socket.close()
        harness.runNextTimer()
      }

      expect(harness.timerDelays).toEqual([
        250, 500, 1_000, 2_000, 5_000, 5_000,
      ])
      stop()
    } finally {
      harness.restore()
    }
  })

  test("an accepted handshake resets a backoff that had already advanced", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()

      // Accepted, then the broker itself drops the socket (e.g. a restart).
      // Acceptance should reset the table before this happens, so the very
      // next delay is the table's first entry.
      const first = FakeWebSocket.instances[0]
      if (first === undefined) throw new Error("no socket was opened")
      driveHandshake(first, harness)
      first.message(JSON.stringify({ type: "ping", nonce: 1 }))
      first.close()
      harness.runNextTimer()

      // Rejected — advances the table past that first entry.
      const second = FakeWebSocket.instances[FakeWebSocket.instances.length - 1]
      if (second === undefined) throw new Error("no socket was opened")
      driveHandshake(second, harness)
      second.close()
      harness.runNextTimer()

      // Accepted again — must fall back to the table's first entry, not
      // continue climbing from the rejected attempt in between. If either
      // the acceptance reset or its per-generation freshness were missing,
      // this delay would carry on from the rejected attempt instead.
      const third = FakeWebSocket.instances[FakeWebSocket.instances.length - 1]
      if (third === undefined) throw new Error("no socket was opened")
      driveHandshake(third, harness)
      third.message(JSON.stringify({ type: "ping", nonce: 1 }))
      third.close()
      harness.runNextTimer()

      expect(harness.timerDelays).toEqual([250, 500, 250])
      stop()
    } finally {
      harness.restore()
    }
  })

  test("a frame the plugin cannot parse is not proof of acceptance: no backoff reset, no Connected status", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()

      // First attempt: rejected outright, advancing the table past its
      // first entry.
      const first = FakeWebSocket.instances[0]
      if (first === undefined) throw new Error("no socket was opened")
      driveHandshake(first, harness)
      first.close()
      harness.runNextTimer()

      // Second attempt: the broker sends a frame, but one this plugin
      // cannot parse — `ping` with an unknown field, which `exact` in
      // `parseBrokerToPlugin` rejects. If any text frame counted as
      // acceptance rather than only one the plugin could actually parse,
      // this would reset the backoff and announce "Connected to local
      // broker" even though the plugin can never understand this broker.
      const second = FakeWebSocket.instances[FakeWebSocket.instances.length - 1]
      if (second === undefined) throw new Error("no socket was opened")
      driveHandshake(second, harness)
      second.message(JSON.stringify({ type: "ping", nonce: 1, extra: true }))
      second.close()
      harness.runNextTimer()

      // The table must keep climbing rather than resetting: 250 then 500,
      // not 250 then 250 again.
      expect(harness.timerDelays).toEqual([250, 500])
      expect(harness.statuses).not.toContain("Connected to local broker")

      stop()
    } finally {
      harness.restore()
    }
  })

  test("status announces each handshake stage truthfully, never claiming connection before acceptance", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const socket = FakeWebSocket.instances[0]
      if (socket === undefined) throw new Error("no socket was opened")
      driveHandshake(socket, harness)

      expect(harness.statuses).toEqual([
        "Opening socket…",
        "Socket open, waiting for Figma…",
        "Hello sent, waiting for broker…",
      ])
      expect(harness.statuses).not.toContain("Connected to local broker")

      // The broker's first frame is the only proof of acceptance available.
      socket.message(JSON.stringify({ type: "ping", nonce: 1 }))

      expect(harness.statuses.at(-1)).toBe("Connected to local broker")
      stop()
    } finally {
      harness.restore()
    }
  })
})

// The socket carries traffic in both directions, and only the broker→controller
// direction and the error arm of the controller→broker direction were pinned.
// A gap here means the plugin silently stops answering the broker's liveness
// check, or stops relaying the results it just produced.
describe("frames the plugin sends back to the broker", () => {
  const handshake = (
    socket: FakeWebSocket,
    harness: ReturnType<typeof installSocketHarness>,
  ) => {
    socket.open()
    const metadata = metadataRequestId(harness.controllerMessages.at(-1))
    harness.emitController(readyFor(metadata))
  }

  const startRequest = (
    socket: FakeWebSocket,
    harness: ReturnType<typeof installSocketHarness>,
    brokerRequestId: string,
  ): string => {
    socket.message(
      JSON.stringify({
        type: "request",
        requestId: brokerRequestId,
        deadlineMs: 1_000,
        target: {},
        operation: { operation: "get_metadata", input: {} },
      }),
    )
    return controllerRequestId(harness.controllerMessages.at(-1))
  }

  test("a broker ping is answered with a pong carrying the same nonce", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const socket = FakeWebSocket.instances[0]
      if (socket === undefined) throw new Error("no socket was opened")
      expect(socket.url).toBe(BROKER_URL)
      handshake(socket, harness)
      const afterHello = socket.sent.length

      socket.message(JSON.stringify({ type: "ping", nonce: 7 }))
      socket.message(JSON.stringify({ type: "ping", nonce: 8 }))

      expect(
        socket.sent.slice(afterHello).map((frame) => JSON.parse(frame)),
      ).toEqual([
        { type: "pong", nonce: 7 },
        { type: "pong", nonce: 8 },
      ])
      stop()
    } finally {
      harness.restore()
    }
  })

  test("controller progress and the response reach the broker under the broker's request ID", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const socket = FakeWebSocket.instances[0]
      if (socket === undefined) throw new Error("no socket was opened")
      handshake(socket, harness)
      const correlationId = startRequest(socket, harness, "request-forward")
      const afterHello = socket.sent.length

      harness.emitController({
        type: "progress",
        controllerRequestId: correlationId,
        requestId: "ignored-plugin-id",
        completed: 4,
        total: 9,
        message: "serializing",
      })
      // A second progress frame proves the owner survives the first one.
      harness.emitController({
        type: "progress",
        controllerRequestId: correlationId,
        requestId: "ignored-plugin-id",
        completed: 6,
      })
      harness.emitController({
        type: "response",
        controllerRequestId: correlationId,
        requestId: "ignored-plugin-id",
        result: {
          operation: "get_metadata",
          result: {
            file: { name: "Checkout", editorType: "dev" },
            pages: [{ id: "0:1", name: "Checkout flow" }],
            currentPageId: "0:1",
            pluginVersion: "0.1.0",
            capabilities: {
              annotations: true,
              devResources: true,
              motion: false,
              svgStringExport: true,
              variableCodeSyntax: true,
            },
            truncated: false,
            observation: {
              startedAt: "2026-09-08T00:00:00.000Z",
              completedAt: "2026-09-08T00:00:01.000Z",
            },
          },
        },
      })

      expect(
        socket.sent.slice(afterHello).map((frame) => JSON.parse(frame)),
      ).toEqual([
        {
          type: "progress",
          requestId: "request-forward",
          completed: 4,
          total: 9,
          message: "serializing",
        },
        { type: "progress", requestId: "request-forward", completed: 6 },
        {
          type: "response",
          requestId: "request-forward",
          result: {
            operation: "get_metadata",
            result: {
              file: { name: "Checkout", editorType: "dev" },
              pages: [{ id: "0:1", name: "Checkout flow" }],
              currentPageId: "0:1",
              pluginVersion: "0.1.0",
              capabilities: {
                annotations: true,
                devResources: true,
                motion: false,
                svgStringExport: true,
                variableCodeSyntax: true,
              },
              truncated: false,
              observation: {
                startedAt: "2026-09-08T00:00:00.000Z",
                completedAt: "2026-09-08T00:00:01.000Z",
              },
            },
          },
        },
      ])
      stop()
    } finally {
      harness.restore()
    }
  })

  test("a response ends the request, so nothing more is forwarded for it", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const socket = FakeWebSocket.instances[0]
      if (socket === undefined) throw new Error("no socket was opened")
      handshake(socket, harness)
      const correlationId = startRequest(socket, harness, "request-once")
      const afterHello = socket.sent.length

      const response = {
        type: "response" as const,
        controllerRequestId: correlationId,
        requestId: "ignored-plugin-id",
        result: {
          operation: "get_metadata" as const,
          result: {
            file: { name: "Checkout", editorType: "dev" as const },
            pages: [{ id: "0:1", name: "Checkout flow" }],
            currentPageId: "0:1",
            pluginVersion: "0.1.0",
            capabilities: {
              annotations: true,
              devResources: true,
              motion: false,
              svgStringExport: true,
              variableCodeSyntax: true,
            },
            truncated: false,
            observation: {
              startedAt: "2026-09-08T00:00:00.000Z",
              completedAt: "2026-09-08T00:00:01.000Z",
            },
          },
        },
      }
      harness.emitController(response)
      harness.emitController(response)

      expect(socket.sent).toHaveLength(afterHello + 1)
      stop()
    } finally {
      harness.restore()
    }
  })

  test("the connected latch fires once, so a later frame does not re-announce it", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const socket = FakeWebSocket.instances[0]
      if (socket === undefined) throw new Error("no socket was opened")
      handshake(socket, harness)

      socket.message(JSON.stringify({ type: "ping", nonce: 1 }))
      socket.message(JSON.stringify({ type: "ping", nonce: 2 }))

      expect(
        harness.statuses.filter(
          (status) => status === "Connected to local broker",
        ),
      ).toHaveLength(1)
      stop()
    } finally {
      harness.restore()
    }
  })

  test("stopping the transport closes the socket and cancels what it was carrying", () => {
    const harness = installSocketHarness()
    try {
      const stop = startSocketTransport()
      const socket = FakeWebSocket.instances[0]
      if (socket === undefined) throw new Error("no socket was opened")
      handshake(socket, harness)
      const correlationId = startRequest(socket, harness, "request-teardown")

      stop()

      expect(socket.readyState).toBe(3)
      expect(harness.controllerMessages).toContainEqual({
        type: "cancel",
        controllerRequestId: correlationId,
        requestId: "request-teardown",
      })

      // The relay listener is gone too: a controller message arriving after the
      // stop reaches nothing, so no further frame is sent.
      const afterStop = socket.sent.length
      harness.emitController({
        type: "progress",
        controllerRequestId: correlationId,
        requestId: "ignored-plugin-id",
        completed: 1,
      })
      expect(socket.sent).toHaveLength(afterStop)
    } finally {
      harness.restore()
    }
  })
})

describe("the hello the plugin introduces itself with", () => {
  test("carries the whole readiness announcement, at the protocol version the broker expects", () => {
    const connectionId = "123e4567-e89b-42d3-a456-426614174321"
    const hello = buildHello(ready, () => connectionId)

    expect(hello).toEqual({
      type: "hello",
      protocolVersion: "4",
      connectionId,
      displayName: "Checkout",
      fileName: "Checkout",
      currentPage: { id: "0:1", name: "Checkout flow" },
      editorType: "dev",
      pluginVersion: "0.1.0",
      capabilities: {
        annotations: true,
        devResources: true,
        motion: false,
        svgStringExport: true,
        variableCodeSyntax: true,
      },
    })
    // The capability set is copied, not aliased: mutating the hello's copy
    // must not reach back into the readiness message it came from.
    hello.capabilities.motion = true
    expect(ready.capabilities.motion).toBe(false)
  })
})
