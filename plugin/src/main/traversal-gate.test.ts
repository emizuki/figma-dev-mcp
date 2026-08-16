import { describe, expect, test } from "bun:test"

import { LocalCancellationController } from "./cancellation"
import { createTraversalGate } from "./traversal-gate"

async function expectAbortedLease(running: Promise<unknown>): Promise<void> {
  const outcome = await Promise.race([
    running.then(
      () => "resolved" as const,
      (error: unknown) => error,
    ),
    new Promise<"pending">((resolve) => {
      setTimeout(() => resolve("pending"), 50)
    }),
  ])
  expect(outcome).not.toBe("pending")
  expect(outcome).not.toBe("resolved")
  expect(outcome).toMatchObject({ message: "Operation cancelled" })
}

describe("traversal gate", () => {
  test("shares readers and queues an include-hidden writer before later readers", async () => {
    const api = { skipInvisibleInstanceChildren: true }
    const gate = createTraversalGate(api)
    const events: string[] = []
    let releaseFirst: (() => void) | undefined
    const firstBlocked = new Promise<void>((resolve) => {
      releaseFirst = resolve
    })

    const first = gate.read(async () => {
      events.push("reader-1:start")
      await firstBlocked
      events.push("reader-1:end")
    })
    const second = gate.read(async () => {
      events.push("reader-2:start")
      events.push("reader-2:end")
    })
    const writer = gate.includeHidden(async () => {
      events.push(`writer:${api.skipInvisibleInstanceChildren}`)
    })
    const lateReader = gate.read(async () => {
      events.push("reader-3:start")
    })

    await second
    expect(events).toEqual(["reader-1:start", "reader-2:start", "reader-2:end"])
    releaseFirst?.()
    await Promise.all([first, writer, lateReader])
    expect(events).toEqual([
      "reader-1:start",
      "reader-2:start",
      "reader-2:end",
      "reader-1:end",
      "writer:false",
      "reader-3:start",
    ])
    expect(api.skipInvisibleInstanceChildren).toBe(true)
  })

  test("restores skipInvisibleInstanceChildren after a thrown error", async () => {
    const api = { skipInvisibleInstanceChildren: true }
    const gate = createTraversalGate(api)

    await expect(
      gate.includeHidden(async () => {
        expect(api.skipInvisibleInstanceChildren).toBe(false)
        throw new Error("hidden traversal failed")
      }),
    ).rejects.toThrow("hidden traversal failed")
    expect(api.skipInvisibleInstanceChildren).toBe(true)
  })

  test("restores skipInvisibleInstanceChildren after a timeout signal", async () => {
    const api = { skipInvisibleInstanceChildren: true }
    const gate = createTraversalGate(api)
    const timeout = new LocalCancellationController()
    let release: (() => void) | undefined
    const blocked = new Promise<void>((resolve) => {
      release = resolve
    })

    const running = gate.includeHidden(async () => {
      expect(api.skipInvisibleInstanceChildren).toBe(false)
      await blocked
    }, timeout.signal)

    timeout.abort()
    await expectAbortedLease(running)
    expect(api.skipInvisibleInstanceChildren).toBe(true)
    release?.()
  })

  test("restores skipInvisibleInstanceChildren after a cancellation signal", async () => {
    const api = { skipInvisibleInstanceChildren: true }
    const gate = createTraversalGate(api)
    const cancellation = new LocalCancellationController()
    let release: (() => void) | undefined
    const blocked = new Promise<void>((resolve) => {
      release = resolve
    })

    const running = gate.includeHidden(async () => {
      expect(api.skipInvisibleInstanceChildren).toBe(false)
      await blocked
    }, cancellation.signal)

    cancellation.abort()
    await expectAbortedLease(running)
    expect(api.skipInvisibleInstanceChildren).toBe(true)
    release?.()
  })

  test("removes an aborted queued request without acquiring a lease", async () => {
    const api = { skipInvisibleInstanceChildren: true }
    const gate = createTraversalGate(api)
    const events: string[] = []
    let releaseFirst: (() => void) | undefined
    const firstBlocked = new Promise<void>((resolve) => {
      releaseFirst = resolve
    })
    const queuedAbort = new LocalCancellationController()

    const first = gate.read(async () => {
      events.push("reader-1:start")
      await firstBlocked
      events.push("reader-1:end")
    })
    const abortedWriter = gate.includeHidden(async () => {
      events.push("writer:acquired")
    }, queuedAbort.signal)
    const lateReader = gate.read(async () => {
      events.push("reader-2:start")
    })

    queuedAbort.abort()
    await expectAbortedLease(abortedWriter)
    expect(api.skipInvisibleInstanceChildren).toBe(true)
    expect(events).not.toContain("writer:acquired")
    await lateReader
    expect(events).toEqual(["reader-1:start", "reader-2:start"])

    releaseFirst?.()
    await first
    expect(events).toEqual(["reader-1:start", "reader-2:start", "reader-1:end"])
    expect(api.skipInvisibleInstanceChildren).toBe(true)
  })

  test("drains a hung lease so later includeHidden work can run", async () => {
    const api = { skipInvisibleInstanceChildren: true }
    const gate = createTraversalGate(api, { leaseDrainMs: 20 })
    const timeout = new LocalCancellationController()
    const events: string[] = []

    const hung = gate.read(async () => {
      events.push("reader:start")
      await new Promise<void>(() => undefined)
    }, timeout.signal)

    timeout.abort()
    await expectAbortedLease(hung)

    await gate.includeHidden(async () => {
      events.push(`writer:${api.skipInvisibleInstanceChildren}`)
    })

    expect(events).toEqual(["reader:start", "writer:false"])
    expect(api.skipInvisibleInstanceChildren).toBe(true)
  })
})
