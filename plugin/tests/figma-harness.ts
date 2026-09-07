/**
 * The one fake Figma host the `plugin/src/read` tests install.
 *
 * Every option is optional and every channel is always present, so a test can
 * destructure whichever channel it asserts on without the builder having to
 * know which tool is under test. The shape is the union of the nine
 * hand-written `installFigma` builders this replaces — components, dev-mode,
 * fonts, motion, reactions, render, search, styles and variables — so the
 * capabilities one of them installed are now installed for all of them.
 *
 * `plugin/src/read/common.test.ts` is deliberately not part of that union: it
 * installs incomplete hosts on purpose, because what it tests is what the read
 * tools do when a capability is missing. It keeps its own raw setter.
 */

/** The sentinel `figma.mixed` compares against. Fixtures use it directly. */
export const FIGMA_MIXED: unique symbol = Symbol("figma.mixed")

export interface FigmaHarnessOptions {
  currentPage?: Record<string, unknown>
  pages?: Record<string, unknown>[]
  nodes?: Map<string, unknown>
  pageChildren?: unknown[]
  selection?: { id: string }[]
  ui?: { postMessage(message: unknown): void }
  available?: { family: string; style: string }[]
  forbidCatalog?: boolean
  categories?: unknown[]
  forbidCategories?: boolean
  styles?: Map<string, unknown>
  local?: {
    paint?: unknown[]
    text?: unknown[]
    effect?: unknown[]
    grid?: unknown[]
  }
  forbidLocal?: boolean
  forbidGetStyle?: boolean
  motion?: unknown
  getNodeByIdAsync?: (id: string) => Promise<unknown>
  variables?: unknown[]
  collections?: unknown[]
  byId?: Map<string, unknown>
  collectionsById?: Map<string, unknown>
  lookupDelayMs?: Record<string, number>
}

export interface FigmaHarness {
  currentPage: Record<string, unknown>
  loadedPages: string[]
  lookedUp: string[]
  styleLookups: string[]
  variableLookups: string[]
  collectionLookups: string[]
  exports: Record<string, unknown>[]
  catalogCalls: { count: number }
  categoryLoads: { count: number }
  localCalls: string[]
}

/**
 * A write API that must never be reached. Installing it as a thrower rather
 * than leaving it off means a regression that starts calling it fails loudly
 * instead of falling into the "capability unavailable" path and looking fine.
 */
function forbidden(name: string): () => Promise<never> {
  return async () => {
    throw new Error(`the read tools must not call ${name}`)
  }
}

/**
 * Depth-first lookup by id, starting at the node itself. `undefined` means not
 * found, which keeps a legitimately stored `null` distinguishable from a miss.
 */
function findNode(raw: unknown, id: string): unknown {
  if (raw === null || typeof raw !== "object") return undefined
  const node = raw as { id?: unknown; children?: unknown }
  if (node.id === id) return raw
  const children = Array.isArray(node.children) ? node.children : []
  for (const child of children) {
    const match = findNode(child, id)
    if (match !== undefined) return match
  }
  return undefined
}

function defaultCategories(): unknown[] {
  return [
    { id: "cat-note", label: "Note", color: "yellow", leftover: true },
    { id: "cat-todo", label: "Todo", color: "blue" },
  ]
}

export function installFigma(options: FigmaHarnessOptions = {}): FigmaHarness {
  const loadedPages: string[] = []
  const lookedUp: string[] = []
  const styleLookups: string[] = []
  const variableLookups: string[] = []
  const collectionLookups: string[] = []
  const exports: Record<string, unknown>[] = []
  const catalogCalls = { count: 0 }
  const categoryLoads = { count: 0 }
  const localCalls: string[] = []

  const requested: Record<string, unknown> = options.currentPage ?? {
    id: "0:1",
    name: "Page 1",
    type: "PAGE",
    children: [],
  }

  // Recording `loadAsync` is what proves a tool paged in only what it needed.
  // The wrapper is memoised on the caller's own object so that a page reached
  // through the `nodes` map is the same object as the one in `pages`: both
  // paths record the load. Wrapping only the array would leave a map hit
  // returning the unwrapped original, and a `loadedPages` assertion would then
  // silently miss the load it was written to catch.
  //
  // The wrap is a `Proxy`, not a spread. A spread reads every enumerable own
  // property up front, which forces any getter on the page — including one a
  // test installs specifically to prove some reader does *not* touch it
  // eagerly — to fire the moment the harness wraps the page, before the code
  // under test ever runs. A real Figma page does not do that: reading
  // `page.loadAsync` does not force `page.children` to evaluate. The proxy
  // intercepts only `loadAsync` and forwards every other property lazily via
  // `Reflect.get`, so a page's own getters keep their laziness.
  const wrappers = new Map<Record<string, unknown>, Record<string, unknown>>()
  const wrapPage = (item: Record<string, unknown>): Record<string, unknown> => {
    const cached = wrappers.get(item)
    if (cached !== undefined) return cached
    const load = item.loadAsync
    if (typeof load !== "function") return item
    const wrapped: Record<string, unknown> = new Proxy(item, {
      get(target, prop, receiver) {
        if (prop === "loadAsync") {
          return async () => {
            loadedPages.push(String(item.id))
            await load.call(item)
          }
        }
        return Reflect.get(target, prop, receiver)
      },
    })
    wrappers.set(item, wrapped)
    return wrapped
  }

  const pages = (options.pages ?? [requested]).map(wrapPage)

  let current = pages.find((item) => item.id === requested.id) ?? requested
  if (options.pageChildren !== undefined || options.selection !== undefined) {
    const overridden: Record<string, unknown> = { ...current }
    if (options.pageChildren !== undefined) {
      overridden.children = options.pageChildren
    }
    if (options.selection !== undefined) {
      overridden.selection = options.selection
    }
    const index = pages.indexOf(current)
    if (index >= 0) pages[index] = overridden
    // So a `nodes` hit on the caller's own current page yields the page the
    // host actually installed, overrides included, rather than the pre-override
    // one.
    wrappers.set(requested, overridden)
    current = overridden
  }

  const nodes = options.nodes ?? new Map<string, unknown>()
  const styles = options.styles ?? new Map<string, unknown>()
  const byId = options.byId ?? new Map<string, unknown>()
  const collectionsById = options.collectionsById ?? new Map<string, unknown>()
  for (const item of options.variables ?? []) {
    const record = item as { id: string }
    if (!byId.has(record.id)) byId.set(record.id, item)
  }
  for (const item of options.collections ?? []) {
    const record = item as { id: string }
    if (!collectionsById.has(record.id)) collectionsById.set(record.id, item)
  }

  const localReader = (
    kind: "paint" | "text" | "effect" | "grid",
  ): (() => Promise<unknown[]>) => {
    return async () => {
      localCalls.push(kind)
      return options.local?.[kind] ?? []
    }
  }

  const api: Record<string, unknown> = {
    root: { name: "Checkout flow", children: pages },
    currentPage: current,
    editorType: "dev",
    mixed: FIGMA_MIXED,
    loadAllPagesAsync: forbidden("loadAllPagesAsync"),
    loadFontAsync: forbidden("loadFontAsync"),
    importComponentByKeyAsync: forbidden("importComponentByKeyAsync"),
    getNodeByIdAsync:
      options.getNodeByIdAsync ??
      (async (id: string) => {
        lookedUp.push(id)
        if (nodes.has(id)) {
          const hit = nodes.get(id)
          if (hit === null || typeof hit !== "object") return hit
          return wrapPage(hit as Record<string, unknown>)
        }
        for (const page of pages) {
          const match = findNode(page, id)
          if (match !== undefined) return match
        }
        return null
      }),
    variables: {
      // Local enumeration answers a different question than the caller asked;
      // these throw so a regression back to it fails loudly rather than
      // silently returning a set the scope does not bind.
      getLocalVariableCollectionsAsync: forbidden(
        "getLocalVariableCollectionsAsync",
      ),
      getLocalVariablesAsync: forbidden("getLocalVariablesAsync"),
      getVariableByIdAsync: async (id: string) => {
        variableLookups.push(id)
        const delay = options.lookupDelayMs?.[id]
        if (delay !== undefined) {
          if (Number.isFinite(delay)) {
            await new Promise((resolve) => setTimeout(resolve, delay))
          } else {
            // A library that never answers. settleOrSkip is what must end this.
            await new Promise(() => {})
          }
        }
        return byId.get(id) ?? null
      },
      getVariableCollectionByIdAsync: async (id: string) => {
        collectionLookups.push(id)
        return collectionsById.get(id) ?? null
      },
    },
  }

  // Every `forbid*` flag leaves the capability off the object rather than
  // installing a thrower. The four flags did not agree before this harness:
  // `dev-mode`'s `forbidCategories` omitted the key and `fonts`' `forbidCatalog`
  // set it to `undefined` — both already meant "absent" — while `styles`'
  // `forbidLocal` and `forbidGetStyle` installed functions that *threw*, meaning
  // "this reader must not be called". Absence is the only rule that unifies all
  // four, so it is the rule here.
  //
  // Be clear about what that costs, because it is not free: it retires those two
  // tripwires. A read tool that touches a forbidden styles API no longer blows
  // the test up — it sees `undefined` and takes the CAPABILITY_UNAVAILABLE path
  // instead, which is a much quieter failure. No styles test asserts on that
  // path through these two flags, and every site that sets one pairs it with a
  // `source` that never reaches the reader, so nothing changes today. Do not
  // read this comment as saying the tripwire is still there. It is not.
  if (!options.forbidCatalog) {
    api.listAvailableFontsAsync = async () =>
      (options.available ?? [{ family: "Inter", style: "Regular" }]).map(
        (fontName) => ({ fontName }),
      )
  }
  if (!options.forbidCategories) {
    api.annotations = {
      getAnnotationCategoriesAsync: async () => {
        categoryLoads.count += 1
        return options.categories ?? defaultCategories()
      },
    }
  }
  if (!options.forbidLocal) {
    api.getLocalPaintStylesAsync = localReader("paint")
    api.getLocalTextStylesAsync = localReader("text")
    api.getLocalEffectStylesAsync = localReader("effect")
    api.getLocalGridStylesAsync = localReader("grid")
  }
  if (!options.forbidGetStyle) {
    api.getStyleByIdAsync = async (id: string) => {
      styleLookups.push(id)
      return styles.get(id) ?? null
    }
  }

  // `motion: false` is how a test asks for a host with no Motion API at all.
  const motion =
    options.motion === undefined
      ? {
          figmaAnimationStyles: () => {
            catalogCalls.count += 1
            return [
              {
                styleId: "S:fade",
                name: "Fade in",
                description: "Catalog fade",
                leftover: true,
                props: {
                  direction: "string",
                  distance: "number",
                },
              },
            ]
          },
        }
      : options.motion
  if (motion !== false) api.motion = motion

  if (options.ui !== undefined) api.ui = options.ui
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api

  return {
    currentPage: current,
    loadedPages,
    lookedUp,
    styleLookups,
    variableLookups,
    collectionLookups,
    exports,
    catalogCalls,
    categoryLoads,
    localCalls,
  }
}
