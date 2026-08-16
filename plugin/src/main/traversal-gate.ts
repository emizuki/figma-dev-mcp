import {
  LocalCancellationError,
  type CancellationListener,
  type CancellationSignal,
} from "./cancellation"

export const TRAVERSAL_LEASE_DRAIN_MS = 5_000

export interface TraversalGateApi {
  skipInvisibleInstanceChildren: boolean
}

export interface TraversalGateOptions {
  leaseDrainMs?: number
}

export interface TraversalGate {
  read<T>(run: () => Promise<T>, signal?: CancellationSignal): Promise<T>
  includeHidden<T>(
    run: () => Promise<T>,
    signal?: CancellationSignal,
  ): Promise<T>
}

type Request = {
  mode: "read" | "includeHidden"
  run: () => Promise<unknown>
  resolve: (value: unknown) => void
  reject: (error: unknown) => void
  signal?: CancellationSignal
  settled: boolean
}

function cancelled(): LocalCancellationError {
  return new LocalCancellationError()
}

async function awaitWithSignal<T>(
  work: Promise<T>,
  signal: CancellationSignal | undefined,
): Promise<T> {
  if (signal === undefined) return work
  if (signal.aborted) throw cancelled()
  let listener: CancellationListener | undefined
  const aborted = new Promise<never>((_, reject) => {
    listener = () => {
      reject(cancelled())
    }
    signal.addEventListener("abort", listener)
  })
  try {
    return await Promise.race([work, aborted])
  } finally {
    if (listener !== undefined) signal.removeEventListener("abort", listener)
  }
}

function ignoreSettlement(work: Promise<unknown>): Promise<void> {
  return work.then(
    () => undefined,
    () => undefined,
  )
}

function delay(ms: number): { promise: Promise<void>; cancel: () => void } {
  let handle: ReturnType<typeof setTimeout> | undefined
  const promise = new Promise<void>((resolve) => {
    handle = setTimeout(resolve, ms)
  })
  return {
    promise,
    cancel: () => {
      if (handle !== undefined) clearTimeout(handle)
    },
  }
}

async function drainLease(
  work: Promise<unknown>,
  leaseDrainMs: number,
): Promise<void> {
  const settled = ignoreSettlement(work)
  if (leaseDrainMs <= 0) return
  const wait = delay(leaseDrainMs)
  try {
    await Promise.race([settled, wait.promise])
  } finally {
    wait.cancel()
  }
}

export function createTraversalGate(
  api: TraversalGateApi,
  options: TraversalGateOptions = {},
): TraversalGate {
  const leaseDrainMs = options.leaseDrainMs ?? TRAVERSAL_LEASE_DRAIN_MS
  const queue: Request[] = []
  let readers = 0
  let writerActive = false
  let pumping = false

  const settleResolve = (request: Request, value: unknown): void => {
    if (request.settled) return
    request.settled = true
    request.resolve(value)
  }

  const settleReject = (request: Request, error: unknown): void => {
    if (request.settled) return
    request.settled = true
    request.reject(error)
  }

  const pump = (): void => {
    if (pumping) return
    pumping = true
    try {
      if (writerActive) return
      while (queue[0]?.signal?.aborted === true) {
        const aborted = queue.shift()
        if (aborted !== undefined) settleReject(aborted, cancelled())
      }
      const first = queue[0]
      if (first === undefined) return
      if (first.mode === "includeHidden") {
        if (readers !== 0) return
        queue.shift()
        writerActive = true
        void runExclusive(first).finally(() => {
          writerActive = false
          pump()
        })
        return
      }
      while (queue[0]?.mode === "read" && !writerActive) {
        const request = queue.shift()
        if (request === undefined) break
        if (request.signal?.aborted) {
          settleReject(request, cancelled())
          continue
        }
        readers += 1
        void runShared(request).finally(() => {
          readers -= 1
          pump()
        })
      }
    } finally {
      pumping = false
    }
  }

  const enqueue = <T>(
    mode: Request["mode"],
    run: () => Promise<T>,
    signal?: CancellationSignal,
  ): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      if (signal?.aborted) {
        reject(cancelled())
        return
      }
      const request: Request = {
        mode,
        run,
        resolve: resolve as (value: unknown) => void,
        reject,
        settled: false,
        ...(signal === undefined ? {} : { signal }),
      }
      if (signal !== undefined) {
        signal.addEventListener("abort", () => {
          const index = queue.indexOf(request)
          if (index < 0) return
          queue.splice(index, 1)
          settleReject(request, cancelled())
          pump()
        })
      }
      queue.push(request)
      pump()
    })

  const runShared = async (request: Request): Promise<void> => {
    const work = request.run()
    try {
      settleResolve(request, await awaitWithSignal(work, request.signal))
    } catch (error: unknown) {
      settleReject(request, error)
    }
    await drainLease(work, leaseDrainMs)
  }

  const runExclusive = async (request: Request): Promise<void> => {
    if (request.signal?.aborted) {
      settleReject(request, cancelled())
      return
    }
    const previous = api.skipInvisibleInstanceChildren
    api.skipInvisibleInstanceChildren = false
    const work = request.run()
    try {
      settleResolve(request, await awaitWithSignal(work, request.signal))
    } catch (error: unknown) {
      settleReject(request, error)
    } finally {
      api.skipInvisibleInstanceChildren = previous
    }
    await drainLease(work, leaseDrainMs)
  }

  return {
    read: <T>(run: () => Promise<T>, signal?: CancellationSignal) =>
      enqueue("read", run, signal),
    includeHidden: <T>(run: () => Promise<T>, signal?: CancellationSignal) =>
      enqueue("includeHidden", run, signal),
  }
}
