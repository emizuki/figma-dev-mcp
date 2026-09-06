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

/// Races `work` against `signal` so a cancelled request settles the instant
/// the signal fires, not whenever `work` next happens to poll it. Read code
/// is expected to call `signal.throwIfAborted()` between steps, but it is not
/// required to — and the one call that cannot poll mid-await, a host
/// `exportAsync`, must still be cancellable at the request boundary. `work`
/// itself is not stopped: there is no way to abort a promise already in
/// flight, so it keeps running to whatever it was going to do and its result
/// is discarded. Callers that create `work` are responsible for swallowing
/// that later settlement themselves — see `ignoreSettlement` below — since
/// this function does not retain a reference to `work` once it loses the
/// race.
export async function awaitWithSignal<T>(
  work: Promise<T>,
  signal: CancellationSignal | undefined,
): Promise<T> {
  if (signal === undefined) return work
  if (signal.aborted) throw new LocalCancellationError()
  let listener: CancellationListener | undefined
  const aborted = new Promise<never>((_, reject) => {
    listener = () => {
      reject(new LocalCancellationError())
    }
    signal.addEventListener("abort", listener)
  })
  try {
    return await Promise.race([work, aborted])
  } finally {
    if (listener !== undefined) signal.removeEventListener("abort", listener)
  }
}

/// A promise abandoned by `awaitWithSignal` keeps running detached and will
/// eventually settle on its own. Nothing else is listening for that by then,
/// so an eventual rejection would otherwise surface as an unhandled
/// rejection in the plugin sandbox rather than the cancellation the caller
/// already received. Call this once, right when `work` is created, so it
/// applies whichever way the race in `awaitWithSignal` goes.
export function ignoreSettlement(work: Promise<unknown>): void {
  void work.then(
    () => undefined,
    () => undefined,
  )
}
