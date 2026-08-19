// Deliberately no getLocalVariable*Async: reads resolve variables by the ids the
// design actually binds, which local enumeration cannot see for a library.
export interface FigmaVariablesApi {
  getVariableByIdAsync?(id: string): Promise<unknown>
  getVariableCollectionByIdAsync?(id: string): Promise<unknown>
}

export interface FigmaReadApi {
  readonly root: {
    readonly name: string
    readonly children: readonly { readonly id: string; readonly name: string }[]
  }
  readonly currentPage: {
    readonly id: string
    readonly name: string
    readonly type?: string
    readonly selection?: readonly { readonly id: string }[]
    readonly children?: readonly unknown[]
    readonly annotations?: unknown
    readonly getDevResourcesAsync?: unknown
  }
  readonly editorType: string
  skipInvisibleInstanceChildren?: boolean
  readonly mixed?: unknown
  getNodeByIdAsync?(id: string): Promise<unknown>
  listAvailableFontsAsync?(): Promise<unknown[]>
  getStyleByIdAsync?(id: string): Promise<unknown>
  getLocalPaintStylesAsync?(): Promise<unknown[]>
  getLocalTextStylesAsync?(): Promise<unknown[]>
  getLocalEffectStylesAsync?(): Promise<unknown[]>
  getLocalGridStylesAsync?(): Promise<unknown[]>
  readonly motion?: unknown
  readonly variables?: FigmaVariablesApi
  readonly annotations?: {
    getAnnotationCategoriesAsync?(): Promise<unknown[]>
  }
}

declare const figma: FigmaReadApi

export const PLUGIN_VERSION = "0.1.0"

export function hasHostField(value: object, key: string): boolean {
  return key in value
}

export const MAIN_COMPONENT_LOOKUP_MS = 1_500

export function settleOrSkip<T>(
  promise: Promise<T>,
  timeoutMs: number = MAIN_COMPONENT_LOOKUP_MS,
): Promise<T | undefined> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(undefined), timeoutMs)
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      () => {
        clearTimeout(timer)
        resolve(undefined)
      },
    )
  })
}

export async function loadPageIfNeeded<T>(node: T): Promise<T> {
  if (node === null || typeof node !== "object") return node
  const record = node as Record<string, unknown>
  if (record.type !== "PAGE") return node
  const load = record.loadAsync
  if (typeof load === "function") await load.call(node)
  return node
}

export function detectCapabilities() {
  return {
    // AnnotationsMixin is a scene-node mixin; PageNode does not extend it, so
    // "annotations" in figma.currentPage was structurally always false. The
    // annotation reader in dev-mode.ts already calls through figma.annotations.
    annotations: "annotations" in figma,
    devResources: "getDevResourcesAsync" in figma.currentPage,
    motion: "motion" in figma,
    svgStringExport: true,
    variableCodeSyntax: "variables" in figma,
  }
}
