import type { GetComponentsInput } from "../shared/protocol"
import type {
  ComponentDefinition,
  ComponentPropertyDefinition,
  ComponentPropertyValue,
  DocumentationReference,
  GetComponentsResult,
  InstanceRelationship,
  NamedVariantProperty,
  Truncation,
} from "../shared/results"
import {
  CANCEL_CHECK_BATCH,
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import {
  LocalCancellationError,
  throwIfAbortedAtBatch,
  type CancellationSignal,
} from "../main/cancellation"
import { settleOrSkip } from "./common"
import { PluginReadError, resolveDesignRoots } from "./navigation"
import {
  byteLength,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

type UnknownRecord = Record<string, unknown>

function observation(startedAt: string) {
  return { startedAt, completedAt: new Date().toISOString() }
}

function isRecord(value: unknown): value is UnknownRecord {
  return value !== null && typeof value === "object"
}

function record(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {}
}

function string(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback
}

function hostGet(value: UnknownRecord, key: string): unknown {
  try {
    return value[key]
  } catch {
    return undefined
  }
}

function array(value: unknown): readonly unknown[] {
  return Array.isArray(value) ? value : []
}

function walkOptions(
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): ForestWalkOptions {
  if (signal !== undefined && limits !== undefined) return { signal, limits }
  if (signal !== undefined) return { signal }
  if (limits !== undefined) return { limits }
  return {}
}

function documentation(raw: unknown): DocumentationReference[] {
  const links: DocumentationReference[] = []
  for (const item of array(raw)) {
    const link = record(item)
    const uri = string(link.uri)
    if (uri.length === 0) continue
    const entry: DocumentationReference = { uri }
    if (typeof link.label === "string") entry.label = link.label
    links.push(entry)
  }
  return links
}

function variantProperties(raw: unknown): NamedVariantProperty[] {
  if (!isRecord(raw)) return []
  try {
    const properties: NamedVariantProperty[] = []
    for (const [name, value] of Object.entries(raw)) {
      if (typeof value === "string") properties.push({ name, value })
    }
    return properties
  } catch {
    return []
  }
}

function propertyValue(
  type: string,
  value: unknown,
): ComponentPropertyValue | undefined {
  switch (type) {
    case "TEXT":
      return typeof value === "string" ? { kind: "text", value } : undefined
    case "BOOLEAN":
      return typeof value === "boolean" ? { kind: "boolean", value } : undefined
    case "INSTANCE_SWAP":
      return typeof value === "string"
        ? { kind: "instanceSwap", value }
        : undefined
    case "VARIANT":
      return typeof value === "string" ? { kind: "variant", value } : undefined
    default:
      return undefined
  }
}

function propertyDefinitions(raw: unknown): ComponentPropertyDefinition[] {
  if (!isRecord(raw)) return []
  const definitions: ComponentPropertyDefinition[] = []
  let entries: [string, unknown][]
  try {
    entries = Object.entries(raw)
  } catch {
    return []
  }
  for (const [name, value] of entries) {
    const definition = record(value)
    const type = string(definition.type)
    const defaultValue = propertyValue(type, definition.defaultValue)
    if (defaultValue === undefined) continue
    const entry: ComponentPropertyDefinition = { name, defaultValue }
    if (type === "VARIANT") {
      const options = array(definition.variantOptions).flatMap((option) =>
        typeof option === "string"
          ? [{ kind: "variant" as const, value: option }]
          : [],
      )
      if (options.length > 0) entry.preferredValues = options
    }
    if (type === "INSTANCE_SWAP") {
      const preferred = array(definition.preferredValues).flatMap((item) => {
        const key = string(record(item).key)
        return key.length > 0
          ? [{ kind: "instanceSwap" as const, value: key }]
          : []
      })
      if (preferred.length > 0) entry.preferredValues = preferred
    }
    definitions.push(entry)
  }
  return definitions
}

function serializeComponent(raw: unknown): ComponentDefinition | undefined {
  const node = record(raw)
  const id = string(node.id)
  if (
    id.length === 0 ||
    (node.type !== "COMPONENT" && node.type !== "COMPONENT_SET")
  ) {
    return undefined
  }
  const result: ComponentDefinition = {
    id,
    name: string(node.name),
    documentation: documentation(hostGet(node, "documentationLinks")),
    variantProperties: variantProperties(hostGet(node, "variantProperties")),
    propertyDefinitions: propertyDefinitions(
      hostGet(node, "componentPropertyDefinitions"),
    ),
  }
  const parent = record(hostGet(node, "parent"))
  if (parent.type === "COMPONENT_SET") {
    const componentSetId = string(parent.id)
    if (componentSetId.length > 0) result.componentSetId = componentSetId
  }
  const description = hostGet(node, "description")
  if (typeof description === "string" && description.length > 0) {
    result.description = description
  }
  return result
}

class ComponentEmission {
  readonly components: ComponentDefinition[] = []
  readonly instances: InstanceRelationship[] = []
  encoded = 0
  considered = 0
  walkTruncation?: Truncation
  emitTruncation?: Truncation

  constructor(readonly limits: SerializerLimits) {}

  get truncation(): Truncation | undefined {
    return this.walkTruncation ?? this.emitTruncation
  }

  mark(truncation: Truncation): void {
    if (this.walkTruncation === undefined) this.walkTruncation = truncation
  }

  private accept(payload: unknown): boolean {
    this.considered += 1
    if (this.emitTruncation !== undefined) return false
    const returned = this.components.length + this.instances.length
    if (returned >= this.limits.returnedNodes) {
      this.emitTruncation = {
        reason: "nodeLimit",
        visitedNodes: this.considered,
      }
      return false
    }
    const encoded = this.encoded + byteLength(payload)
    if (encoded > this.limits.encodedBytes) {
      this.emitTruncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    return true
  }

  pushComponent(component: ComponentDefinition): boolean {
    if (!this.accept(component)) return false
    this.components.push(component)
    return true
  }

  pushInstance(relationship: InstanceRelationship): boolean {
    if (!this.accept(relationship)) return false
    this.instances.push(relationship)
    return true
  }
}

export interface GetComponentsLimits extends Partial<SerializerLimits> {
  readonly mainComponentBudgetMs?: number
}

const DEFAULT_MAIN_COMPONENT_BUDGET_MS = 8_000

export async function getComponents(
  input: Partial<GetComponentsInput> = {},
  signal?: CancellationSignal,
  limits?: GetComponentsLimits,
): Promise<GetComponentsResult> {
  const startedAt = new Date().toISOString()
  const emission = new ComponentEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })
  const roots = await resolveDesignRoots(input.selector, signal)
  const components = new Map<string, unknown>()
  const componentOrder: string[] = []
  const instances = new Map<string, unknown>()
  const instanceOrder: string[] = []
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    const node = record(raw)
    const id = string(node.id)
    if (id.length === 0) return
    if (node.type === "COMPONENT" || node.type === "COMPONENT_SET") {
      if (components.has(id)) return
      components.set(id, raw)
      componentOrder.push(id)
      return
    }
    if (node.type === "INSTANCE") {
      if (instances.has(id)) return
      instances.set(id, raw)
      instanceOrder.push(id)
    }
  })
  if (walked.truncation !== undefined) emission.mark(walked.truncation)

  for (const id of componentOrder) {
    signal?.throwIfAborted()
    try {
      const component = serializeComponent(components.get(id))
      if (component !== undefined && !emission.pushComponent(component)) break
    } catch (error: unknown) {
      if (
        error instanceof PluginReadError ||
        error instanceof LocalCancellationError
      ) {
        throw error
      }
    }
  }

  const lookupBatch = 16
  const budgetMs = limits?.mainComponentBudgetMs ?? DEFAULT_MAIN_COMPONENT_BUDGET_MS
  const budgetStarted = Date.now()
  for (let start = 0; start < instanceOrder.length; start += lookupBatch) {
    throwIfAbortedAtBatch(signal, start, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    if (emission.emitTruncation !== undefined) break
    if (Date.now() - budgetStarted >= budgetMs) {
      emission.mark({
        reason: "nodeLimit",
        visitedNodes: emission.considered,
      })
      break
    }
    const slice = instanceOrder.slice(start, start + lookupBatch)
    const resolved = await Promise.all(
      slice.map(async (instanceId) => {
        const instance = record(instances.get(instanceId))
        const lookup = instance.getMainComponentAsync
        if (typeof lookup !== "function") return undefined
        try {
          const main = await settleOrSkip(
            lookup.call(instance) as Promise<unknown>,
          )
          const componentId = string(record(main).id)
          if (componentId.length === 0) return undefined
          return { instanceId, componentId }
        } catch (error: unknown) {
          if (
            error instanceof PluginReadError ||
            error instanceof LocalCancellationError
          ) {
            throw error
          }
          return undefined
        }
      }),
    )
    for (const item of resolved) {
      if (item === undefined) continue
      if (!emission.pushInstance(item)) break
    }
    if (Date.now() - budgetStarted >= budgetMs) {
      emission.mark({
        reason: "nodeLimit",
        visitedNodes: emission.considered,
      })
      break
    }
  }

  const result: GetComponentsResult = {
    components: emission.components,
    instances: emission.instances,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
