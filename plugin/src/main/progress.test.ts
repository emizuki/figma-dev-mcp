import { describe, expect, test } from "bun:test"

import { LocalCancellationController } from "./cancellation"
import {
  bindProgress,
  createProgressReporter,
  progressFor,
  type ProgressFrame,
} from "./progress"
import { parseControllerOutboundMessage } from "../shared/validation"

const controllerRequestId = "123e4567-e89b-42d3-a456-426614174000"

function recorded(): {
  frames: ProgressFrame[]
  reporter: ReturnType<typeof createProgressReporter>
  advance: (ms: number) => void
} {
  const frames: ProgressFrame[] = []
  let now = 0
  const reporter = createProgressReporter({
    emit: (frame) => frames.push(frame),
    intervalMs: 3_000,
    now: () => now,
  })
  return {
    frames,
    reporter,
    advance: (ms: number) => {
      now += ms
    },
  }
}

describe("bounded plugin progress", () => {
  test("emits on phase change and after the inactivity-safe interval", () => {
    const { frames, reporter, advance } = recorded()
    reporter.tick("reading", 1)
    reporter.tick("reading", 2)
    advance(2_999)
    reporter.tick("reading", 3)
    advance(1)
    reporter.tick("reading", 4)
    reporter.tick("serializing", 4)

    expect(frames).toEqual([
      { completed: 1, message: "reading" },
      { completed: 4, message: "reading" },
      { completed: 4, message: "serializing" },
    ])
  })

  test("heartbeat re-emits counts so inactivity can refresh without new work", () => {
    const frames: ProgressFrame[] = []
    const callbacks: Array<() => void> = []
    let cancelled = false
    const reporter = createProgressReporter({
      emit: (frame) => frames.push(frame),
      intervalMs: 3_000,
      now: () => 0,
      schedule: (callback) => {
        callbacks.push(callback)
        return () => {
          cancelled = true
        }
      },
    })
    reporter.tick("encoding", 2, 8)
    reporter.startHeartbeat("encoding")
    expect(callbacks).toHaveLength(1)
    callbacks[0]?.()
    reporter.stopHeartbeat()
    expect(cancelled).toBe(true)

    expect(frames).toEqual([
      { completed: 2, total: 8, message: "encoding" },
      { completed: 2, total: 8, message: "encoding" },
    ])
  })

  test("clamps counters to u32 and keeps messages as phase names only", () => {
    const frames: ProgressFrame[] = []
    const reporter = createProgressReporter({
      emit: (frame) => frames.push(frame),
      intervalMs: 0,
    })
    reporter.tick("reading", -3, Number.POSITIVE_INFINITY)
    reporter.tick("serializing", 4_294_967_296, 4_294_967_295)

    expect(frames).toEqual([
      { completed: 0, total: 0, message: "reading" },
      {
        completed: 4_294_967_295,
        total: 4_294_967_295,
        message: "serializing",
      },
    ])
    for (const frame of frames) {
      expect(
        parseControllerOutboundMessage({
          type: "progress",
          controllerRequestId,
          requestId: "plugin-1",
          completed: frame.completed,
          ...(frame.total === undefined ? {} : { total: frame.total }),
          message: frame.message,
        }),
      ).toMatchObject({
        type: "progress",
        completed: frame.completed,
        message: frame.message,
      })
    }
  })

  test("bindProgress scopes reporters to a cancellation signal", () => {
    const first = new LocalCancellationController()
    const second = new LocalCancellationController()
    const reporter = createProgressReporter({
      emit: () => undefined,
      intervalMs: 0,
    })
    bindProgress(first.signal, reporter)
    expect(progressFor(first.signal)).toBe(reporter)
    expect(progressFor(second.signal)).toBeUndefined()
    expect(progressFor(undefined)).toBeUndefined()
  })
})
