import type { CancellationSignal } from "./cancellation"
import { U32_MAX } from "../shared/limits"

export const PROGRESS_INTERVAL_MS = 3_000

export type ProgressPhase = "reading" | "serializing" | "encoding"

export interface ProgressFrame {
  completed: number
  total?: number
  message: ProgressPhase
}

export interface ProgressReporter {
  tick(phase: ProgressPhase, completed: number, total?: number): void
  startHeartbeat(phase: ProgressPhase): void
  stopHeartbeat(): void
}

export interface ProgressReporterOptions {
  emit: (frame: ProgressFrame) => void
  intervalMs?: number
  now?: () => number
  schedule?: (callback: () => void, ms: number) => () => void
}

const reporters = new WeakMap<CancellationSignal, ProgressReporter>()

export function bindProgress(
  signal: CancellationSignal,
  reporter: ProgressReporter,
): void {
  reporters.set(signal, reporter)
}

export function progressFor(
  signal: CancellationSignal | undefined,
): ProgressReporter | undefined {
  return signal === undefined ? undefined : reporters.get(signal)
}

function toU32(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return 0
  if (value >= U32_MAX) return U32_MAX
  return Math.floor(value)
}

function defaultSchedule(callback: () => void, ms: number): () => void {
  const handle = setInterval(callback, ms)
  return () => clearInterval(handle)
}

export function createProgressReporter(
  options: ProgressReporterOptions,
): ProgressReporter {
  const intervalMs = options.intervalMs ?? PROGRESS_INTERVAL_MS
  const now = options.now ?? Date.now
  const schedule = options.schedule ?? defaultSchedule
  let lastEmitAt = Number.NaN
  let lastPhase: ProgressPhase | undefined
  let completed = 0
  let total: number | undefined
  let stopTimer: (() => void) | undefined

  const emitLatest = (): void => {
    const frame: ProgressFrame = {
      completed: toU32(completed),
      message: lastPhase ?? "reading",
    }
    if (total !== undefined) frame.total = toU32(total)
    lastEmitAt = now()
    try {
      options.emit(frame)
    } catch {
      // Progress is best-effort and must not fail the tool.
    }
  }

  const due = (phase: ProgressPhase): boolean => {
    if (lastPhase !== phase || !Number.isFinite(lastEmitAt)) return true
    if (intervalMs <= 0) return true
    return now() - lastEmitAt >= intervalMs
  }

  const tick = (
    phase: ProgressPhase,
    nextCompleted: number,
    nextTotal?: number,
  ): void => {
    completed = nextCompleted
    if (nextTotal === undefined) {
      total = undefined
    } else {
      total = nextTotal
    }
    if (!due(phase)) return
    lastPhase = phase
    emitLatest()
  }

  return {
    tick,
    startHeartbeat(phase: ProgressPhase): void {
      if (stopTimer !== undefined) stopTimer()
      tick(phase, completed, total)
      if (intervalMs <= 0) {
        stopTimer = undefined
        return
      }
      stopTimer = schedule(() => {
        if (lastPhase === undefined) lastPhase = phase
        emitLatest()
      }, intervalMs)
    },
    stopHeartbeat(): void {
      stopTimer?.()
      stopTimer = undefined
    },
  }
}
