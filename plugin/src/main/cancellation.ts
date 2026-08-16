export class DuplicateRequestError extends Error {
  override readonly name = "DuplicateRequestError"
}

export class LocalCancellationError extends Error {
  override readonly name = "LocalCancellationError"

  constructor() {
    super("Operation cancelled")
  }
}

export type CancellationListener = () => void

export interface CancellationSignal {
  readonly aborted: boolean
  addEventListener(type: "abort", listener: CancellationListener): void
  removeEventListener(type: "abort", listener: CancellationListener): void
  throwIfAborted(): void
}

class LocalCancellationSignal implements CancellationSignal {
  #aborted = false
  readonly #listeners = new Set<CancellationListener>()

  get aborted(): boolean {
    return this.#aborted
  }

  addEventListener(type: "abort", listener: CancellationListener): void {
    if (type === "abort" && !this.#aborted) this.#listeners.add(listener)
  }

  removeEventListener(type: "abort", listener: CancellationListener): void {
    if (type === "abort") this.#listeners.delete(listener)
  }

  throwIfAborted(): void {
    if (this.#aborted) throw new LocalCancellationError()
  }

  abort(): void {
    if (this.#aborted) return
    this.#aborted = true
    const listeners = [...this.#listeners]
    this.#listeners.clear()
    for (const listener of listeners) {
      try {
        listener()
      } catch {
        // Cancellation must remain best-effort and notify every registered handler.
      }
    }
  }
}

export class LocalCancellationController {
  readonly #signal = new LocalCancellationSignal()

  get signal(): CancellationSignal {
    return this.#signal
  }

  abort(): void {
    this.#signal.abort()
  }
}

export class CancellationRegistry {
  readonly #active = new Map<string, LocalCancellationController>()

  get size(): number {
    return this.#active.size
  }

  begin(requestId: string): LocalCancellationController {
    if (this.#active.has(requestId)) {
      throw new DuplicateRequestError("duplicate active request identifier")
    }
    const controller = new LocalCancellationController()
    this.#active.set(requestId, controller)
    return controller
  }

  cancel(requestId: string): boolean {
    const controller = this.#active.get(requestId)
    if (controller === undefined || controller.signal.aborted) return false
    controller.abort()
    return true
  }

  finish(requestId: string): void {
    this.#active.delete(requestId)
  }

  cancelAll(): number {
    const ids = [...this.#active.keys()]
    let cancelled = 0
    for (const requestId of ids) {
      if (this.cancel(requestId)) cancelled += 1
    }
    return cancelled
  }
}

export function throwIfAbortedAtBatch(
  signal: CancellationSignal | undefined,
  index: number,
  batchSize = 100,
): void {
  if (index % batchSize === 0) signal?.throwIfAborted()
}
