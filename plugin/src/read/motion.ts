import type { GetMotionInput } from "../shared/protocol"
import type {
  AnimationBinding,
  AnimationTrack,
  AppliedAnimationStyle,
  AppliedStyleProp,
  AppliedStylePropValue,
  AvailableAnimationStyle,
  AvailableStyleProp,
  Color,
  GetMotionResult,
  IndexedCollection,
  ItemResult,
  KeyframeField,
  KeyframeOperation,
  ManualTrackBinding,
  MotionEasing,
  MotionEasingType,
  MotionKeyframe,
  MotionKeyframeValue,
  MotionTimeline,
  NodeMotion,
  Truncation,
} from "../shared/results"
import {
  CANCEL_CHECK_BATCH,
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import {
  throwIfAbortedAtBatch,
  type CancellationSignal,
} from "../main/cancellation"
import { hasHostField } from "./common"
import { PluginReadError, resolveDesignRoots } from "./navigation"
import type { FigmaReadApi } from "./common"
import {
  byteLength,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

declare const figma: FigmaReadApi

type UnknownRecord = Record<string, unknown>

const EASING_TYPES = new Set<MotionEasingType>([
  "LINEAR",
  "EASE_IN",
  "EASE_OUT",
  "EASE_IN_AND_OUT",
  "EASE_IN_BACK",
  "EASE_OUT_BACK",
  "EASE_IN_AND_OUT_BACK",
  "CUSTOM_CUBIC_BEZIER",
  "GENTLE",
  "QUICK",
  "BOUNCY",
  "SLOW",
  "CUSTOM_SPRING",
  "HOLD",
  "VARIABLE_ALIAS",
])

const INDEXED_COLLECTIONS: readonly IndexedCollection[] = [
  "fills",
  "strokes",
  "effects",
]

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

function finite(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

function array(value: unknown): readonly unknown[] {
  return Array.isArray(value) ? value : []
}

function ownKeys(value: unknown): string[] {
  return isRecord(value) ? Object.keys(value) : []
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

function unsupportedItem(): ItemResult<NodeMotion> {
  return {
    status: "error",
    error: {
      code: "UNSUPPORTED_NODE",
      message: "The requested node type is not supported.",
      retryable: false,
    },
  }
}

class ItemEmission {
  readonly items: GetMotionResult["items"] = []
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

  push(item: ItemResult<NodeMotion>): boolean {
    this.considered += 1
    if (this.emitTruncation !== undefined) return false
    if (this.items.length >= this.limits.returnedNodes) {
      this.emitTruncation = {
        reason: "nodeLimit",
        visitedNodes: this.considered,
      }
      return false
    }
    const encoded = this.encoded + byteLength(item)
    if (encoded > this.limits.encodedBytes) {
      this.emitTruncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    this.items.push(item)
    return true
  }
}

function requireMotionSurface(): { catalog(): unknown[] } {
  const motion = figma.motion
  if (!isRecord(motion) || typeof motion.figmaAnimationStyles !== "function") {
    throw new PluginReadError("CAPABILITY_UNAVAILABLE", false)
  }
  const reader = motion.figmaAnimationStyles
  return {
    catalog: () => {
      const styles = reader.call(motion)
      return Array.isArray(styles) ? styles : []
    },
  }
}

function color(raw: unknown): Color | undefined {
  const value = record(raw)
  const r = finite(value.r)
  const g = finite(value.g)
  const b = finite(value.b)
  const a = finite(value.a)
  if (
    r === undefined ||
    g === undefined ||
    b === undefined ||
    a === undefined
  ) {
    return undefined
  }
  return { r, g, b, a }
}

function motionEasing(raw: unknown): MotionEasing | undefined {
  const value = record(raw)
  if (value.type === "VARIABLE_ALIAS") {
    const id = string(value.id)
    return id.length === 0 ? undefined : { type: "VARIABLE_ALIAS", id }
  }
  if (
    typeof value.type !== "string" ||
    !EASING_TYPES.has(value.type as MotionEasingType)
  ) {
    return undefined
  }
  const easing: MotionEasing = {
    type: value.type as Exclude<MotionEasingType, "VARIABLE_ALIAS">,
  }
  const bezier = record(value.easingFunctionCubicBezier)
  const x1 = finite(bezier.x1)
  const y1 = finite(bezier.y1)
  const x2 = finite(bezier.x2)
  const y2 = finite(bezier.y2)
  if (
    x1 !== undefined &&
    y1 !== undefined &&
    x2 !== undefined &&
    y2 !== undefined
  ) {
    easing.easingFunctionCubicBezier = { x1, y1, x2, y2 }
  }
  const bounce = finite(record(value.easingFunctionSpring).bounce)
  if (bounce !== undefined) easing.easingFunctionSpring = { bounce }
  return easing
}

function keyframeValue(raw: unknown): MotionKeyframeValue | undefined {
  const value = record(raw)
  switch (value.type) {
    case "FLOAT": {
      const number = finite(value.value)
      return number === undefined ? undefined : { type: "FLOAT", value: number }
    }
    case "COLOR": {
      const parsed = color(value.value)
      return parsed === undefined ? undefined : { type: "COLOR", value: parsed }
    }
    case "TEXT_DATA":
      return typeof value.value === "string"
        ? { type: "TEXT_DATA", value: value.value }
        : undefined
    case "VECTOR": {
      const point = record(value.value)
      const x = finite(point.x)
      const y = finite(point.y)
      return x === undefined || y === undefined
        ? undefined
        : { type: "VECTOR", value: { x, y } }
    }
    case "BOOL":
      return typeof value.value === "boolean"
        ? { type: "BOOL", value: value.value }
        : undefined
    case "CIRCLE": {
      const circle = record(value.value)
      const x = finite(circle.x)
      const y = finite(circle.y)
      const radius = finite(circle.radius)
      return x === undefined || y === undefined || radius === undefined
        ? undefined
        : { type: "CIRCLE", value: { x, y, radius } }
    }
    case "LINE": {
      const line = record(value.value)
      const x = finite(line.x)
      const y = finite(line.y)
      const x2 = finite(line.x2)
      const y2 = finite(line.y2)
      return x === undefined ||
        y === undefined ||
        x2 === undefined ||
        y2 === undefined
        ? undefined
        : { type: "LINE", value: { x, y, x2, y2 } }
    }
    case "CIRCLE_POINT": {
      const point = record(value.value)
      const x = finite(point.x)
      const y = finite(point.y)
      const radius = finite(point.radius)
      const angle = finite(point.angle)
      return x === undefined ||
        y === undefined ||
        radius === undefined ||
        angle === undefined
        ? undefined
        : { type: "CIRCLE_POINT", value: { x, y, radius, angle } }
    }
    case "COLOR_POINT": {
      const point = record(value.value)
      const x = finite(point.x)
      const y = finite(point.y)
      const parsed = color(point.color)
      return x === undefined || y === undefined || parsed === undefined
        ? undefined
        : { type: "COLOR_POINT", value: { x, y, color: parsed } }
    }
    default:
      return typeof value.type === "string"
        ? { type: "unsupported", tag: value.type }
        : undefined
  }
}

function keyframe(raw: unknown): MotionKeyframe | undefined {
  const value = record(raw)
  const id = string(value.id)
  const timelinePosition = finite(value.timelinePosition)
  const parsed = keyframeValue(value.value)
  const easing = motionEasing(value.easing)
  if (
    id.length === 0 ||
    timelinePosition === undefined ||
    parsed === undefined ||
    easing === undefined
  ) {
    return undefined
  }
  return { id, timelinePosition, value: parsed, easing }
}

function keyframes(raw: unknown): MotionKeyframe[] {
  const frames: MotionKeyframe[] = []
  for (const item of array(raw)) {
    const parsed = keyframe(item)
    if (parsed !== undefined) frames.push(parsed)
  }
  return frames
}

function operation(raw: unknown): KeyframeOperation | undefined {
  switch (raw) {
    case "SET":
    case "OFFSET":
    case "SCALE":
      return raw
    default:
      return undefined
  }
}

function tracks(raw: unknown): AnimationTrack[] {
  const result: AnimationTrack[] = []
  for (const item of array(raw)) {
    const track = record(item)
    const id = string(track.id)
    const keyframeOperation = operation(track.keyframeOperation)
    if (id.length === 0 || keyframeOperation === undefined) continue
    result.push({
      id,
      keyframeOperation,
      keyframes: keyframes(track.keyframes),
    })
  }
  return result
}

function isKeyframeBinding(raw: unknown): boolean {
  const value = record(raw)
  return (
    Object.hasOwn(value, "tracks") || Object.hasOwn(value, "timelineDuration")
  )
}

function isManualBinding(raw: unknown): boolean {
  const value = record(raw)
  return Object.hasOwn(value, "keyframes") && Object.hasOwn(value, "id")
}

function animationBinding(
  field: KeyframeField,
  raw: unknown,
): AnimationBinding | undefined {
  if (!isKeyframeBinding(raw)) return undefined
  const value = record(raw)
  const baseValue = keyframeValue(value.baseValue)
  const timelineDuration = finite(value.timelineDuration)
  if (baseValue === undefined || timelineDuration === undefined)
    return undefined
  return {
    field,
    baseValue,
    timelineDuration,
    tracks: tracks(value.tracks),
  }
}

function manualBinding(
  field: KeyframeField,
  raw: unknown,
): ManualTrackBinding | undefined {
  if (!isManualBinding(raw)) return undefined
  const value = record(raw)
  const id = string(value.id)
  const baseValue = keyframeValue(value.baseValue)
  if (id.length === 0 || baseValue === undefined) return undefined
  return { field, id, baseValue, keyframes: keyframes(value.keyframes) }
}

function sortedPropertyNames(map: UnknownRecord): string[] {
  return ownKeys(map)
    .filter((key) => key !== "fills" && key !== "strokes" && key !== "effects")
    .sort()
}

function numericIndices(raw: unknown): number[] {
  return ownKeys(raw)
    .map((key) => Number(key))
    .filter((index) => Number.isInteger(index) && index >= 0)
    .sort((left, right) => left - right)
}

function propertyEntries(raw: unknown): [string, unknown][] {
  return ownKeys(raw)
    .sort()
    .map((name) => [name, record(raw)[name]] as [string, unknown])
}

function flattenAnimations(raw: unknown): AnimationBinding[] {
  const map = record(raw)
  const bindings: AnimationBinding[] = []
  for (const name of sortedPropertyNames(map)) {
    const binding = animationBinding({ type: "property", name }, map[name])
    if (binding !== undefined) bindings.push(binding)
  }
  for (const collection of INDEXED_COLLECTIONS) {
    const group = map[collection]
    for (const index of numericIndices(group)) {
      const item = record(group)[String(index)]
      if (collection === "effects") {
        const effect = record(item)
        for (const [field, value] of propertyEntries(effect).filter(
          ([key]) => key !== "properties",
        )) {
          const binding = animationBinding(
            { type: "indexedItem", collection, index, field },
            value,
          )
          if (binding !== undefined) bindings.push(binding)
        }
        for (const [propertyId, value] of propertyEntries(effect.properties)) {
          const binding = animationBinding(
            { type: "indexedItem", collection, index, propertyId },
            value,
          )
          if (binding !== undefined) bindings.push(binding)
        }
        continue
      }
      if (isKeyframeBinding(item)) {
        const binding = animationBinding(
          { type: "indexedItem", collection, index },
          item,
        )
        if (binding !== undefined) bindings.push(binding)
        continue
      }
      for (const [propertyId, value] of propertyEntries(
        record(item).properties,
      )) {
        const binding = animationBinding(
          { type: "indexedItem", collection, index, propertyId },
          value,
        )
        if (binding !== undefined) bindings.push(binding)
      }
    }
  }
  return bindings
}

function flattenManual(raw: unknown): ManualTrackBinding[] {
  const map = record(raw)
  const bindings: ManualTrackBinding[] = []
  for (const name of sortedPropertyNames(map)) {
    const binding = manualBinding({ type: "property", name }, map[name])
    if (binding !== undefined) bindings.push(binding)
  }
  for (const collection of INDEXED_COLLECTIONS) {
    const group = map[collection]
    for (const index of numericIndices(group)) {
      const item = record(group)[String(index)]
      if (collection === "effects") {
        const effect = record(item)
        for (const [field, value] of propertyEntries(effect).filter(
          ([key]) => key !== "properties",
        )) {
          const binding = manualBinding(
            { type: "indexedItem", collection, index, field },
            value,
          )
          if (binding !== undefined) bindings.push(binding)
        }
        for (const [propertyId, value] of propertyEntries(effect.properties)) {
          const binding = manualBinding(
            { type: "indexedItem", collection, index, propertyId },
            value,
          )
          if (binding !== undefined) bindings.push(binding)
        }
        continue
      }
      if (isManualBinding(item)) {
        const binding = manualBinding(
          { type: "indexedItem", collection, index },
          item,
        )
        if (binding !== undefined) bindings.push(binding)
        continue
      }
      for (const [propertyId, value] of propertyEntries(
        record(item).properties,
      )) {
        const binding = manualBinding(
          { type: "indexedItem", collection, index, propertyId },
          value,
        )
        if (binding !== undefined) bindings.push(binding)
      }
    }
  }
  return bindings
}

function appliedPropValue(raw: unknown): AppliedStylePropValue | undefined {
  if (typeof raw === "string" || typeof raw === "boolean") return raw
  if (typeof raw === "number" && Number.isFinite(raw)) return raw
  return motionEasing(raw)
}

function appliedProps(raw: unknown): AppliedStyleProp[] | undefined {
  if (!isRecord(raw)) return undefined
  const props: AppliedStyleProp[] = []
  for (const name of ownKeys(raw).sort()) {
    const value = appliedPropValue(raw[name])
    if (value !== undefined) props.push({ name, value })
  }
  return props
}

function appliedStyles(raw: unknown): AppliedAnimationStyle[] {
  const styles: AppliedAnimationStyle[] = []
  for (const item of array(raw)) {
    const style = record(item)
    const id = string(style.id)
    const styleId = string(style.styleId)
    if (id.length === 0 || styleId.length === 0) continue
    const result: AppliedAnimationStyle = {
      id,
      styleId,
      name: string(style.name),
    }
    const duration = finite(style.duration)
    if (duration !== undefined) result.duration = duration
    const timelineOffset = finite(style.timelineOffset)
    if (timelineOffset !== undefined) result.timelineOffset = timelineOffset
    const props = appliedProps(style.props)
    if (props !== undefined) result.props = props
    styles.push(result)
  }
  return styles
}

function timelines(raw: unknown): MotionTimeline[] {
  const result: MotionTimeline[] = []
  for (const item of array(raw)) {
    const timeline = record(item)
    const id = string(timeline.id)
    const duration = finite(timeline.duration)
    if (id.length === 0 || duration === undefined) continue
    result.push({ id, duration })
  }
  return result
}

function availableStyles(raw: unknown): AvailableAnimationStyle[] {
  const styles: AvailableAnimationStyle[] = []
  for (const item of array(raw)) {
    const style = record(item)
    const styleId = string(style.styleId)
    if (styleId.length === 0) continue
    const result: AvailableAnimationStyle = {
      styleId,
      name: string(style.name),
    }
    if (typeof style.description === "string") {
      result.description = style.description
    }
    if (isRecord(style.props)) {
      const props: AvailableStyleProp[] = []
      for (const name of ownKeys(style.props).sort()) {
        const value = style.props[name]
        if (typeof value === "string") props.push({ name, value })
      }
      result.props = props
    }
    styles.push(result)
  }
  return styles
}

function supportsMotion(raw: unknown): boolean {
  const node = record(raw)
  return (
    hasHostField(node, "animationStyles") &&
    hasHostField(node, "animations") &&
    hasHostField(node, "manualKeyframeTracks") &&
    hasHostField(node, "timelines")
  )
}

function serializeMotion(raw: unknown): ItemResult<NodeMotion> {
  const node = record(raw)
  const nodeId = string(node.id)
  if (nodeId.length === 0 || !supportsMotion(node)) return unsupportedItem()
  return {
    status: "success",
    value: {
      nodeId,
      animationStyles: appliedStyles(node.animationStyles),
      animations: flattenAnimations(node.animations),
      manualKeyframeTracks: flattenManual(node.manualKeyframeTracks),
      timelines: timelines(node.timelines),
    },
  }
}

export async function getMotion(
  input: Partial<GetMotionInput> = {},
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<GetMotionResult> {
  const startedAt = new Date().toISOString()
  const motion = requireMotionSurface()
  const emission = new ItemEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })
  const catalog =
    input.includeAvailableStyles === true
      ? availableStyles(motion.catalog())
      : []
  const roots = await resolveDesignRoots(input.selector, signal)
  const pending: unknown[] = []
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    pending.push(raw)
  })
  if (walked.truncation !== undefined) emission.mark(walked.truncation)
  for (let index = 0; index < pending.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    if (!emission.push(serializeMotion(pending[index]))) break
  }
  const result: GetMotionResult = {
    items: emission.items,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (catalog.length > 0) result.availableStyles = catalog
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
