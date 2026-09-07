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
 *
 * That exclusion is not a courtesy, it is the consequence of a deliberate
 * decision here: **the default host is richer than a real one.**
 * `installFigma({})` installs seventeen keys — `annotations`, `motion`,
 * `listAvailableFontsAsync`, `getStyleByIdAsync`, all four
 * `getLocal*StylesAsync`, `mixed`, `variables`, three forbidden-write
 * throwers, plus `root` / `currentPage` / `editorType` / `getNodeByIdAsync` —
 * which is more than eight of the nine builders it replaces installed and
 * more than a real `documentAccess: dynamic-page` host offers. The spec asked
 * for that on purpose ("every one gets the page model, whether or not its old
 * builder had one"), and it is what makes one oracle possible at all.
 *
 * The cost is precise, so state it precisely: **this harness must never be
 * used to test capability detection.** A test that asserts a
 * capability-*present* path against this host is asserting against a host
 * more complete than any real one, and would pass whether or not production
 * detects the capability correctly. Two things keep that inert today, and
 * both are load-bearing rather than accidental: `detectCapabilities()` — the
 * only place `"annotations" in figma` / `"motion" in figma` is evaluated — is
 * reached from `navigation.ts` and `main/code.ts`, neither of which any of
 * the nine migrated files exercises; and both `figma.mixed` comparison sites
 * (`fonts.ts`, `styles.ts`) guard on `mixed !== undefined`, so installing
 * `figma.mixed` for the eight files that previously lacked it cannot flip a
 * branch. If either of those stops holding, capability tests belong in
 * `common.test.ts` with its own raw setter — which is exactly why that file
 * is excluded from this migration and keeps one.
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
  forbidCategories?: boolean
  styles?: Map<string, unknown>
  local?: {
    paint?: unknown[]
    text?: unknown[]
    effect?: unknown[]
    grid?: unknown[]
  }
  /** Installs the four `getLocal*StylesAsync` readers as throwers. */
  forbidLocal?: boolean
  /** Installs `getStyleByIdAsync` as a thrower. */
  forbidGetStyle?: boolean
  /** Leaves the four `getLocal*StylesAsync` readers off the host entirely. */
  omitLocal?: boolean
  /** Leaves `getStyleByIdAsync` off the host entirely. */
  omitGetStyle?: boolean
  motion?: unknown
  getNodeByIdAsync?: (id: string) => Promise<unknown>
  variables?: unknown[]
  collections?: unknown[]
  byId?: Map<string, unknown>
  lookupDelayMs?: Record<string, number>
}

export interface FigmaHarness {
  currentPage: Record<string, unknown>
  loadedPages: string[]
  lookedUp: string[]
  styleLookups: string[]
  variableLookups: string[]
  collectionLookups: string[]
  catalogCalls: { count: number }
  categoryLoads: { count: number }
  localCalls: string[]
}

/**
 * An API that must never be reached on this host — the forbidden writes, and
 * the styles readers a `forbidLocal` / `forbidGetStyle` test rules out.
 * Installing it as a thrower rather than leaving it off means a regression
 * that starts calling it fails loudly instead of falling into the "capability
 * unavailable" path and looking fine.
 */
function forbidden(name: string): () => Promise<never> {
  return async () => {
    throw new Error(`the read tools must not call ${name}`)
  }
}

/**
 * The children a walk is allowed to see, read without ever firing a getter.
 *
 * A fixture that writes `children: [card]` has handed the harness an inert
 * array and means it to be walked. A fixture that installs a `children`
 * *accessor* is doing the opposite: under `documentAccess: dynamic-page` a
 * page's children are not there until the plugin pages the page in, and tests
 * model that with a getter that throws if it is touched too early
 * (`search.test.ts`'s "checks a matching node before reading its dynamic
 * children"). Reading through such a property to satisfy a lookup would fire
 * exactly the getter the test installed to prove nothing fires it — the
 * defect Task 4 removed from `wrapPage`, which then survived one call site
 * later in the lookup fallback.
 *
 * So the descriptor, not the value, decides: a data property is walked, an
 * accessor is treated as "not paged in yet" and contributes nothing. That is
 * also what a real host does on a miss — it does not walk unloaded pages — and
 * it matches the shallow `pages.find(...)` fallback six of the nine builders
 * this harness replaces used.
 */
function inertChildren(raw: object): unknown[] {
  const descriptor = Object.getOwnPropertyDescriptor(raw, "children")
  if (descriptor === undefined || descriptor.get !== undefined) return []
  return Array.isArray(descriptor.value) ? descriptor.value : []
}

/**
 * Depth-first lookup by id, starting at the node itself. `undefined` means not
 * found, which keeps a legitimately stored `null` distinguishable from a miss.
 */
function findNode(raw: unknown, id: string): unknown {
  if (raw === null || typeof raw !== "object") return undefined
  const node = raw as { id?: unknown }
  if (node.id === id) return raw
  for (const child of inertChildren(raw)) {
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
  //
  // The test for "is this a page" is production's own: `loadPageIfNeeded`
  // checks `type === "PAGE"` *before* it looks for a `loadAsync`. Keying on
  // `loadAsync` alone would wrap a FRAME that happens to carry one, break its
  // identity against the caller's object, and record its load into
  // `loadedPages` — the harness would agree with a plugin that pages in a
  // non-page rather than catch it. Real Figma only gives `PageNode` a
  // `loadAsync`, so the two predicates coincide on real hosts; they diverge
  // exactly on the bug shape, which is where an oracle has to be right.
  const wrappers = new Map<Record<string, unknown>, Record<string, unknown>>()
  const wrapPage = (item: Record<string, unknown>): Record<string, unknown> => {
    const cached = wrappers.get(item)
    if (cached !== undefined) return cached
    if (item.type !== "PAGE") return item
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
    // A Proxy layered on `current`, not a spread: `current` may itself be a
    // `wrapPage` proxy over a page with lazy getters, and spreading it here
    // would read every one of its properties eagerly — the same defect just
    // removed from `wrapPage`, reintroduced one call site later. Only
    // `children` / `selection` are actually overridden; every other property
    // (`loadAsync` included) forwards through `Reflect.get`, so nesting on
    // top of an already-wrapped page keeps that page's own laziness and its
    // `loadAsync` recording intact.
    const base = current
    const overridden: Record<string, unknown> = new Proxy(base, {
      get(target, prop, receiver) {
        if (prop === "children" && options.pageChildren !== undefined) {
          return options.pageChildren
        }
        if (prop === "selection" && options.selection !== undefined) {
          return options.selection
        }
        return Reflect.get(target, prop, receiver)
      },
    })
    const index = pages.indexOf(current)
    if (index >= 0) pages[index] = overridden
    // Registers the override under the caller's own `currentPage` object, so
    // a `nodes` hit keyed on *that* object yields the overridden view rather
    // than the pre-override one. What this does and does not guarantee:
    //
    //   holds — when `options.currentPage` is the same object the `pages`
    //     entry is (the default, and every migrated test today), `requested`
    //     and the wrapped `pages` entry share one memo key, so every route to
    //     that page — `figma.currentPage`, `pages`, a `nodes` hit — is the
    //     same overridden object.
    //   does not hold — when `options.currentPage` is a *distinct* object
    //     that merely shares an id with a `pages` entry. `current` then
    //     resolved to the `pages` entry's wrapper, so this line files the
    //     override under a key nothing looks up, and a `nodes` hit storing
    //     the `pages` entry hands back a view without the overrides. The
    //     harness would then serve two different views of one page id. No
    //     migrated test builds that shape; do not build one without fixing
    //     this first.
    wrappers.set(requested, overridden)
    current = overridden
  }

  const nodes = options.nodes ?? new Map<string, unknown>()
  const styles = options.styles ?? new Map<string, unknown>()
  const byId = options.byId ?? new Map<string, unknown>()
  const collectionsById = new Map<string, unknown>()
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
        // The miss path must stay as lazy as `wrapPage` is: compare page ids
        // first, and descend only through children the test actually supplied
        // as data — `pageChildren`, or an inert `children` array on the page
        // itself. See `inertChildren`: a `children` *getter* is never fired
        // here, so a lookup miss over a page with lazy children resolves to
        // `null` instead of blowing the test up.
        for (const page of pages) {
          if (page.id === id) return page
        }
        for (const page of pages) {
          const children =
            page === current && options.pageChildren !== undefined
              ? options.pageChildren
              : inertChildren(page)
          for (const child of children) {
            const match = findNode(child, id)
            if (match !== undefined) return match
          }
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

  // "Absent" and "must not be called" are two different claims, and the flags
  // here say which one a test is making rather than collapsing both into
  // absence. The distinction is not cosmetic — it is the difference between a
  // tripwire and no tripwire:
  //
  //   forbidCatalog / forbidCategories — **absent**. `fonts` and `dev-mode`
  //     set these to test the capability-unavailable path itself, so the key
  //     genuinely has to be missing for the code under test to take it.
  //   omitLocal / omitGetStyle — **absent**, same reason: `styles`' "fails
  //     when required style APIs are unavailable" needs `paint === undefined`
  //     and `lookup === undefined` to reach CAPABILITY_UNAVAILABLE.
  //   forbidLocal / forbidGetStyle — **installed as throwers**. These say
  //     "the reader exists on a real host, and this tool must not touch it".
  //
  // Absence cannot express that last claim. Under absence a reader that
  // wrongly calls a forbidden API just gets `undefined` and slides into the
  // CAPABILITY_UNAVAILABLE branch, which is a quiet, plausible-looking wrong
  // answer rather than a failure. Measured: injecting a stray
  // `figma.getStyleByIdAsync?.("MUTANT")` into `emitLocal` fails 6 tests with
  // these installed as throwers and only 2 with them absent; the same
  // injection into `emitReferenced` fails 2 versus 1. Five detections, all in
  // `styles.test.ts`. The throwers are what make `expect(styleLookups)
  // .toEqual([])` and `expect(localCalls).toEqual([])` mean anything — with
  // the API absent, nothing could ever push to those recorders and both
  // assertions hold vacuously.
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
        return defaultCategories()
      },
    }
  }
  if (options.forbidLocal) {
    api.getLocalPaintStylesAsync = forbidden("getLocalPaintStylesAsync")
    api.getLocalTextStylesAsync = forbidden("getLocalTextStylesAsync")
    api.getLocalEffectStylesAsync = forbidden("getLocalEffectStylesAsync")
    api.getLocalGridStylesAsync = forbidden("getLocalGridStylesAsync")
  } else if (!options.omitLocal) {
    api.getLocalPaintStylesAsync = localReader("paint")
    api.getLocalTextStylesAsync = localReader("text")
    api.getLocalEffectStylesAsync = localReader("effect")
    api.getLocalGridStylesAsync = localReader("grid")
  }
  if (options.forbidGetStyle) {
    api.getStyleByIdAsync = forbidden("getStyleByIdAsync")
  } else if (!options.omitGetStyle) {
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
    catalogCalls,
    categoryLoads,
    localCalls,
  }
}
