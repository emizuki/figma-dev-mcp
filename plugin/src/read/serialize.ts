import {
  CANCEL_CHECK_BATCH,
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import { settleOrSkip } from "./common"
import type {
  AxisAlign,
  BlendMode,
  CompactNodeData,
  ComponentPropertyValue,
  ComponentValue,
  ConstraintAxis,
  CornerRadiusValue,
  DesignNode,
  EffectValue,
  FullNodeData,
  InstanceValue,
  LayoutConstraints,
  LayoutValue,
  LetterSpacingValue,
  LineHeightValue,
  MinimalNodeDetails,
  NamedComponentProperty,
  PaintValue,
  StrokeAlign,
  StrokeValue,
  StyleReference,
  TextAlignHorizontal,
  TextAlignVertical,
  TextAutoResize,
  TextDecoration,
  TextStyle,
  TextValue,
  Truncation,
  VariableReference,
} from "../shared/results"
import type { DetailLevel } from "../shared/protocol"
import {
  throwIfAbortedAtBatch,
  type CancellationSignal,
} from "../main/cancellation"
import { progressFor, type ProgressReporter } from "../main/progress"

type UnknownRecord = Record<string, unknown>
type NodeData = MinimalNodeDetails | CompactNodeData | FullNodeData

export interface SerializerLimits {
  readonly returnedNodes: number
  readonly visitedNodes: number
  readonly encodedBytes: number
}

export interface SerializeNodeForestOptions {
  readonly detail: DetailLevel
  readonly depth: number
  readonly dedupeComponents: boolean
  readonly includeHidden?: boolean
  readonly instanceIdentities?: ReadonlyMap<string, InstanceValue>
  readonly signal?: CancellationSignal
  readonly progress?: ProgressReporter
  readonly limits?: Partial<SerializerLimits>
}

export interface SerializedNodeForest {
  readonly nodes: DesignNode<NodeData>[]
  readonly truncated: boolean
  readonly truncation?: Truncation
}

interface SerializerContext {
  readonly detail: DetailLevel
  readonly depth: number
  readonly dedupeComponents: boolean
  readonly includeHidden: boolean | undefined
  readonly instanceIdentities: ReadonlyMap<string, InstanceValue> | undefined
  readonly signal: CancellationSignal | undefined
  readonly progress: ProgressReporter | undefined
  readonly limits: SerializerLimits
  readonly emittedComponents: Set<string>
  visitedNodes: number
  returnedNodes: number
  encodedBytes: number
  truncation?: Truncation
}

function record(value: unknown): UnknownRecord {
  return value !== null && typeof value === "object"
    ? (value as UnknownRecord)
    : {}
}

function string(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback
}

function finite(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

function optionalFinite(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

function boolean(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback
}

function array(value: unknown): readonly unknown[] {
  return Array.isArray(value) ? value : []
}

function own(value: UnknownRecord, key: string): boolean {
  return Object.hasOwn(value, key)
}

function hostGet(value: UnknownRecord, key: string): unknown {
  try {
    return value[key]
  } catch {
    return undefined
  }
}

function hostString(value: UnknownRecord, key: string, fallback = ""): string {
  return string(hostGet(value, key), fallback)
}

function isMixed(value: unknown): boolean {
  return typeof value === "symbol" || value === "mixed"
}

export function toColor(value: unknown) {
  const color = record(value)
  return {
    r: finite(color.r),
    g: finite(color.g),
    b: finite(color.b),
    a: finite(color.a, 1),
  }
}

function toPaint(value: unknown): PaintValue | undefined {
  if (isMixed(value)) return { type: "mixed" }
  const paint = record(value)
  switch (paint.type) {
    case "SOLID":
      return {
        type: "solid",
        color: toColor(paint.color),
        opacity: finite(paint.opacity, 1),
      }
    case "GRADIENT_LINEAR":
    case "GRADIENT_RADIAL": {
      const stops = array(paint.gradientStops).map((stop) => {
        const source = record(stop)
        return {
          position: finite(source.position),
          color: toColor(source.color),
        }
      })
      return {
        type:
          paint.type === "GRADIENT_LINEAR"
            ? "linearGradient"
            : "radialGradient",
        stops,
      }
    }
    case "IMAGE":
      return {
        type: "image",
        imageRef: string(paint.imageHash),
        scaleMode: imageScaleMode(paint.scaleMode),
      }
    default:
      return undefined
  }
}

export function paints(value: unknown): PaintValue[] {
  if (isMixed(value)) return [{ type: "mixed" }]
  return array(value).flatMap((paint) => {
    const parsed = toPaint(paint)
    return parsed === undefined ? [] : [parsed]
  })
}

function imageScaleMode(value: unknown): "fill" | "fit" | "crop" | "tile" {
  switch (value) {
    case "FIT":
      return "fit"
    case "CROP":
      return "crop"
    case "TILE":
      return "tile"
    default:
      return "fill"
  }
}

export function effects(value: unknown): EffectValue[] {
  const result: EffectValue[] = []
  for (const effect of array(value)) {
    const source = record(effect)
    const offset = record(source.offset)
    switch (source.type) {
      case "DROP_SHADOW":
      case "INNER_SHADOW":
        result.push({
          type: source.type === "DROP_SHADOW" ? "dropShadow" : "innerShadow",
          color: toColor(source.color),
          offsetX: finite(offset.x),
          offsetY: finite(offset.y),
          radius: finite(source.radius),
          spread: finite(source.spread),
        })
        break
      case "LAYER_BLUR":
      case "BACKGROUND_BLUR":
        result.push({
          type: source.type === "LAYER_BLUR" ? "layerBlur" : "backgroundBlur",
          radius: finite(source.radius),
        })
        break
      default:
        break
    }
  }
  return result
}

const STROKE_ALIGNS: Record<string, StrokeAlign> = {
  INSIDE: "inside",
  OUTSIDE: "outside",
  CENTER: "center",
}

// `paints` on FullNodeData is fills only; stroke colours live here.
function strokes(node: UnknownRecord): StrokeValue | undefined {
  const strokePaints = paints(hostGet(node, "strokes"))
  if (strokePaints.length === 0) return undefined
  const value: StrokeValue = { paints: strokePaints }
  const weight = optionalFinite(hostGet(node, "strokeWeight"))
  if (weight !== undefined) value.weight = weight
  const align = STROKE_ALIGNS[hostString(node, "strokeAlign")]
  if (align !== undefined) value.align = align
  const dashPattern = array(hostGet(node, "dashPattern")).filter(
    (entry): entry is number =>
      typeof entry === "number" && Number.isFinite(entry),
  )
  if (dashPattern.length > 0) value.dashPattern = dashPattern
  return value
}

// A mixed cornerRadius means exactly "the four corners differ", and the four
// values are readable — so it maps to perCorner instead of dropping the field.
function cornerRadius(node: UnknownRecord): CornerRadiusValue | undefined {
  const uniform = hostGet(node, "cornerRadius")
  if (!isMixed(uniform)) {
    const radius = optionalFinite(uniform)
    return radius === undefined || radius === 0
      ? undefined
      : { kind: "uniform", radius }
  }
  const topLeft = optionalFinite(hostGet(node, "topLeftRadius"))
  const topRight = optionalFinite(hostGet(node, "topRightRadius"))
  const bottomRight = optionalFinite(hostGet(node, "bottomRightRadius"))
  const bottomLeft = optionalFinite(hostGet(node, "bottomLeftRadius"))
  if (
    topLeft === undefined ||
    topRight === undefined ||
    bottomRight === undefined ||
    bottomLeft === undefined
  ) {
    return undefined
  }
  return { kind: "perCorner", topLeft, topRight, bottomRight, bottomLeft }
}

const BLEND_MODES: Record<string, BlendMode> = {
  PASS_THROUGH: "passThrough",
  NORMAL: "normal",
  DARKEN: "darken",
  MULTIPLY: "multiply",
  LINEAR_BURN: "linearBurn",
  COLOR_BURN: "colorBurn",
  LIGHTEN: "lighten",
  SCREEN: "screen",
  LINEAR_DODGE: "linearDodge",
  COLOR_DODGE: "colorDodge",
  OVERLAY: "overlay",
  SOFT_LIGHT: "softLight",
  HARD_LIGHT: "hardLight",
  DIFFERENCE: "difference",
  EXCLUSION: "exclusion",
  HUE: "hue",
  SATURATION: "saturation",
  COLOR: "color",
  LUMINOSITY: "luminosity",
}

// PASS_THROUGH is the Figma default for frames and groups, NORMAL for everything
// else. Emitting either would add ~26 bytes to nearly every node for no signal.
function blendMode(node: UnknownRecord): BlendMode | undefined {
  const mode = BLEND_MODES[hostString(node, "blendMode")]
  return mode === undefined || mode === "normal" || mode === "passThrough"
    ? undefined
    : mode
}

function lineHeightValue(source: UnknownRecord): LineHeightValue | undefined {
  const raw = hostGet(source, "lineHeight")
  if (isMixed(raw)) return undefined
  const object = record(raw)
  if (string(object.unit) === "AUTO") return { unit: "auto" }
  const value = object.value
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined
  switch (string(object.unit)) {
    case "PIXELS":
      return { unit: "pixels", value }
    case "PERCENT":
      return { unit: "percent", value }
    default:
      return undefined
  }
}

function letterSpacingValue(
  source: UnknownRecord,
): LetterSpacingValue | undefined {
  const raw = hostGet(source, "letterSpacing")
  if (isMixed(raw)) return undefined
  const object = record(raw)
  const value = object.value
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined
  switch (string(object.unit)) {
    case "PIXELS":
      return { unit: "pixels", value }
    case "PERCENT":
      return { unit: "percent", value }
    default:
      return undefined
  }
}

// Figma default values are deliberately absent: an absent key means the field is
// omitted at its default (see the TextDecoration/TextAlign*/TextAutoResize unions).
const TEXT_DECORATIONS: Record<string, TextDecoration> = {
  UNDERLINE: "underline",
  STRIKETHROUGH: "strikethrough",
}

const TEXT_ALIGN_HORIZONTALS: Record<string, TextAlignHorizontal> = {
  CENTER: "center",
  RIGHT: "right",
  JUSTIFIED: "justified",
}

const TEXT_ALIGN_VERTICALS: Record<string, TextAlignVertical> = {
  CENTER: "center",
  BOTTOM: "bottom",
}

const TEXT_AUTO_RESIZES: Record<string, TextAutoResize> = {
  WIDTH_AND_HEIGHT: "widthAndHeight",
  HEIGHT: "height",
  TRUNCATE: "truncate",
}

export function textStyle(source: UnknownRecord): TextStyle {
  const font = record(hostGet(source, "fontName"))
  const style: TextStyle = {
    fontFamily: string(font.family),
    fontStyle: string(font.style),
    paints: paints(hostGet(source, "fills")),
  }
  const fontSize = hostGet(source, "fontSize")
  if (typeof fontSize === "number" && Number.isFinite(fontSize)) {
    style.fontSize = fontSize
  }
  const lineHeight = lineHeightValue(source)
  if (lineHeight !== undefined) style.lineHeight = lineHeight
  const letterSpacing = letterSpacingValue(source)
  if (letterSpacing !== undefined) style.letterSpacing = letterSpacing
  const fontWeight = optionalFinite(hostGet(source, "fontWeight"))
  if (fontWeight !== undefined) style.fontWeight = fontWeight
  const decoration = TEXT_DECORATIONS[hostString(source, "textDecoration")]
  if (decoration !== undefined) style.textDecoration = decoration
  return style
}

const TEXT_SEGMENT_FIELDS = [
  "fontName",
  "fontSize",
  "fontWeight",
  "textDecoration",
  "lineHeight",
  "letterSpacing",
  "fills",
] as const

function getStyledRanges(node: UnknownRecord) {
  const readSegments = node.getStyledTextSegments
  if (typeof readSegments !== "function") return []
  try {
    return array(readSegments.call(node, [...TEXT_SEGMENT_FIELDS])).map(
      (segment) => {
        const source = record(segment)
        return {
          start: finite(source.start),
          end: finite(source.end),
          style: textStyle(source),
        }
      },
    )
  } catch {
    return []
  }
}

function geometry(node: UnknownRecord) {
  const transform = array(node.absoluteTransform)
  const row0 = array(transform[0])
  const row1 = array(transform[1])
  const bounds = record(node.absoluteBoundingBox)
  const result = {
    rotation: finite(node.rotation),
    opacity: finite(node.opacity, 1),
    transform: {
      m00: finite(row0[0], 1),
      m01: finite(row0[1]),
      m02: finite(row0[2]),
      m10: finite(row1[0]),
      m11: finite(row1[1], 1),
      m12: finite(row1[2]),
    },
  }
  if (own(bounds, "x") && own(bounds, "y")) {
    return {
      ...result,
      bounds: {
        x: finite(bounds.x),
        y: finite(bounds.y),
        width: finite(bounds.width),
        height: finite(bounds.height),
      },
    }
  }
  return result
}

const AXIS_ALIGNS: Record<string, AxisAlign> = {
  MIN: "min",
  CENTER: "center",
  MAX: "max",
  SPACE_BETWEEN: "spaceBetween",
  BASELINE: "baseline",
}

function autoLayout(node: UnknownRecord): LayoutValue | undefined {
  const layoutMode = hostGet(node, "layoutMode")
  if (typeof layoutMode !== "string" || layoutMode === "NONE") return undefined
  const layout: LayoutValue = {
    mode:
      layoutMode === "HORIZONTAL"
        ? "horizontal"
        : layoutMode === "VERTICAL"
          ? "vertical"
          : "grid",
    primarySizing: sizing(hostGet(node, "primaryAxisSizingMode")),
    counterSizing: sizing(hostGet(node, "counterAxisSizingMode")),
    gap: finite(hostGet(node, "itemSpacing")),
    paddingTop: finite(hostGet(node, "paddingTop")),
    paddingRight: finite(hostGet(node, "paddingRight")),
    paddingBottom: finite(hostGet(node, "paddingBottom")),
    paddingLeft: finite(hostGet(node, "paddingLeft")),
  }
  // Alignment is justify-content and align-items: always emitted when the node has
  // auto-layout, including the Figma default MIN. Absence means the getter was
  // unreadable, not that the layout is left-aligned.
  const primaryAlign = AXIS_ALIGNS[hostString(node, "primaryAxisAlignItems")]
  if (primaryAlign !== undefined) layout.primaryAlign = primaryAlign
  const counterAlign = AXIS_ALIGNS[hostString(node, "counterAxisAlignItems")]
  if (counterAlign !== undefined) layout.counterAlign = counterAlign
  if (hostGet(node, "layoutWrap") === "WRAP") {
    layout.wrap = true
    const spacing = optionalFinite(hostGet(node, "counterAxisSpacing"))
    if (spacing !== undefined) layout.counterAxisSpacing = spacing
  }
  return layout
}

const CONSTRAINT_AXES: Record<string, ConstraintAxis> = {
  MIN: "min",
  CENTER: "center",
  MAX: "max",
  STRETCH: "stretch",
  SCALE: "scale",
}

function layoutConstraints(node: UnknownRecord): LayoutConstraints | undefined {
  const source = record(hostGet(node, "constraints"))
  const horizontal = CONSTRAINT_AXES[string(hostGet(source, "horizontal"))]
  const vertical = CONSTRAINT_AXES[string(hostGet(source, "vertical"))]
  if (horizontal === undefined || vertical === undefined) return undefined
  return { horizontal, vertical }
}

function sizing(value: unknown): "fixed" | "hug" | "fill" {
  switch (value) {
    case "AUTO":
      return "hug"
    case "FILL":
      return "fill"
    default:
      return "fixed"
  }
}

function styleReferences(node: UnknownRecord): StyleReference[] {
  const refs: StyleReference[] = []
  for (const [field, kind] of [
    ["fillStyleId", "paint"],
    ["textStyleId", "text"],
    ["effectStyleId", "effect"],
    ["gridStyleId", "grid"],
  ] as const) {
    const id = node[field]
    if (typeof id === "string" && id.length > 0 && id !== "mixed") {
      refs.push({ id, kind })
    }
  }
  return refs
}

function variableReferences(node: UnknownRecord): VariableReference[] {
  const values = record(node.boundVariables)
  const seen = new Set<string>()
  const refs: VariableReference[] = []
  const visit = (value: unknown): void => {
    const source = record(value)
    if (typeof source.id === "string" && !seen.has(source.id)) {
      seen.add(source.id)
      const ref: VariableReference = { id: source.id }
      if (typeof source.name === "string") ref.name = source.name
      refs.push(ref)
    }
    for (const child of Object.values(source)) {
      if (child !== value) {
        if (Array.isArray(child)) for (const item of child) visit(item)
        else if (child !== null && typeof child === "object") visit(child)
      }
    }
  }
  for (const value of Object.values(values)) visit(value)
  return refs
}

export const TEXT_CLAMP_LIMIT = 256

// Slicing a string by UTF-16 code unit can land inside a surrogate pair, leaving a
// trailing lone high surrogate. JSON.stringify happily emits it (e.g. "\ud83d"), but
// serde_json rejects the escape as invalid Unicode and fails to decode the whole
// response, not just this value. Only when truncation actually occurred do we drop a
// dangling trailing high surrogate; a string already within the limit — even one that
// ends in a pre-existing unpaired high surrogate — is returned untouched.
export function clampText(
  value: string,
  limit: number = TEXT_CLAMP_LIMIT,
): string {
  if (value.length <= limit) return value
  const sliced = value.slice(0, limit)
  const lastCode = sliced.charCodeAt(sliced.length - 1)
  return lastCode >= 0xd800 && lastCode <= 0xdbff ? sliced.slice(0, -1) : sliced
}

function clampPropertyValue(value: unknown): unknown {
  return typeof value === "string" ? clampText(value) : value
}

// Clamping deliberately lives outside this function: get_components' propertyDefinitions
// call this directly with unclamped defaultValue text, and must stay that way.
export function componentPropertyValue(
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

export function namedComponentProperties(
  node: UnknownRecord,
): NamedComponentProperty[] {
  const source = record(hostGet(node, "componentProperties"))
  let entries: [string, unknown][]
  try {
    entries = Object.entries(source)
  } catch {
    return []
  }
  const properties: NamedComponentProperty[] = []
  for (const [name, raw] of entries) {
    const property = record(raw)
    const value = componentPropertyValue(
      string(hostGet(property, "type")),
      clampPropertyValue(hostGet(property, "value")),
    )
    if (value === undefined) continue
    properties.push({ name, value })
  }
  properties.sort((left, right) => {
    if (left.name < right.name) return -1
    return left.name > right.name ? 1 : 0
  })
  return properties
}

function componentValue(node: UnknownRecord): ComponentValue | undefined {
  if (node.type !== "COMPONENT" && node.type !== "COMPONENT_SET")
    return undefined
  const componentId = string(node.id)
  if (componentId.length === 0) return undefined
  return { componentId, properties: [] }
}

function instanceValue(
  node: UnknownRecord,
  identities?: ReadonlyMap<string, InstanceValue>,
): InstanceValue | undefined {
  if (node.type !== "INSTANCE") return undefined
  const id = string(node.id)
  const resolved = id.length > 0 ? identities?.get(id) : undefined
  if (resolved !== undefined) return resolved
  // Never read mainComponent: it is write-only under documentAccess: dynamic-page.
  const componentId = hostString(node, "componentId")
  if (componentId.length === 0) return undefined
  const value: InstanceValue = {
    componentId,
    properties: namedComponentProperties(node),
  }
  const componentSetId = hostString(node, "componentSetId")
  if (componentSetId.length > 0) value.componentSetId = componentSetId
  return value
}

function identityData(detail: DetailLevel, componentId: string): NodeData {
  if (detail === "minimal") return {}
  const component = { componentId, properties: [] as const }
  if (detail === "compact") {
    return {
      styleReferences: [],
      variableReferences: [],
      component,
    }
  }
  return {
    styleReferences: [],
    variableReferences: [],
    paints: [],
    effects: [],
    component,
  }
}

function nodeData(
  node: UnknownRecord,
  detail: DetailLevel,
  identities?: ReadonlyMap<string, InstanceValue>,
): NodeData {
  if (detail === "minimal") return {}
  const compact: CompactNodeData = {
    geometry: geometry(node),
    styleReferences: styleReferences(node),
    variableReferences: variableReferences(node),
  }
  const layout = autoLayout(node)
  if (layout !== undefined) compact.autoLayout = layout
  const constraints = layoutConstraints(node)
  if (constraints !== undefined) compact.constraints = constraints
  const component = componentValue(node)
  if (component !== undefined) compact.component = component
  const instance = instanceValue(node, identities)
  if (instance !== undefined) compact.instance = instance
  if (node.type === "TEXT") {
    const characters = string(node.characters)
    compact.text = {
      characterCount: characters.length,
      preview: clampText(characters),
    }
  }
  if (detail === "compact") return compact
  const full: FullNodeData = {
    styleReferences: compact.styleReferences,
    variableReferences: compact.variableReferences,
    paints: paints(hostGet(node, "fills")),
    effects: effects(node.effects),
  }
  if (compact.geometry !== undefined) full.geometry = compact.geometry
  if (compact.autoLayout !== undefined) full.autoLayout = compact.autoLayout
  if (compact.constraints !== undefined) full.constraints = compact.constraints
  if (compact.component !== undefined) full.component = compact.component
  if (compact.instance !== undefined) full.instance = compact.instance
  const strokeValue = strokes(node)
  if (strokeValue !== undefined) full.strokes = strokeValue
  const radius = cornerRadius(node)
  if (radius !== undefined) full.cornerRadius = radius
  const smoothing = optionalFinite(hostGet(node, "cornerSmoothing"))
  if (smoothing !== undefined && smoothing !== 0)
    full.cornerSmoothing = smoothing
  if (hostGet(node, "clipsContent") === true) full.clipsContent = true
  const blend = blendMode(node)
  if (blend !== undefined) full.blendMode = blend
  if (node.type === "TEXT") {
    const text: TextValue = {
      characters: string(hostGet(node, "characters")),
      defaultStyle: textStyle(node),
      styledRanges: getStyledRanges(node),
    }
    const alignHorizontal =
      TEXT_ALIGN_HORIZONTALS[hostString(node, "textAlignHorizontal")]
    if (alignHorizontal !== undefined) text.alignHorizontal = alignHorizontal
    const alignVertical =
      TEXT_ALIGN_VERTICALS[hostString(node, "textAlignVertical")]
    if (alignVertical !== undefined) text.alignVertical = alignVertical
    const autoResize = TEXT_AUTO_RESIZES[hostString(node, "textAutoResize")]
    if (autoResize !== undefined) text.autoResize = autoResize
    full.text = text
  }
  return full
}

function visibleChildren(
  node: UnknownRecord,
  includeHidden: boolean | undefined,
): readonly unknown[] {
  const children = array(node.children)
  if (includeHidden !== false) return children
  return children.filter((child) => boolean(record(child).visible, true))
}

function summarize(node: UnknownRecord, children: readonly unknown[]) {
  const parent = record(node.parent)
  const bounds = record(node.absoluteBoundingBox)
  const childIds = children
    .map((child) => string(record(child).id))
    .filter((id) => id.length > 0)
  const summary = {
    id: string(node.id),
    name: string(node.name),
    nodeType: string(node.type),
    visible: boolean(node.visible, true),
  }
  const result: {
    id: string
    name: string
    nodeType: string
    visible: boolean
    parentId?: string
    childIds?: string[]
    bounds?: { x: number; y: number; width: number; height: number }
  } = summary
  if (typeof parent.id === "string") result.parentId = parent.id
  if (childIds.length > 0) result.childIds = childIds
  if (own(bounds, "x") && own(bounds, "y")) {
    result.bounds = {
      x: finite(bounds.x),
      y: finite(bounds.y),
      width: finite(bounds.width),
      height: finite(bounds.height),
    }
  }
  return result
}

function markTruncated(
  context: SerializerContext,
  truncation: Truncation,
): void {
  if (context.truncation === undefined) context.truncation = truncation
}

export function byteLength(value: unknown): number {
  const text = JSON.stringify(value)
  let bytes = 0
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index)
    if (code <= 0x7f) bytes += 1
    else if (code <= 0x7ff) bytes += 2
    else if (code >= 0xd800 && code <= 0xdbff) {
      const next = text.charCodeAt(index + 1)
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4
        index += 1
      } else bytes += 3
    } else bytes += 3
  }
  return bytes
}

function serializeNode(
  raw: unknown,
  level: number,
  ancestors: ReadonlySet<string>,
  context: SerializerContext,
): DesignNode<NodeData> | undefined {
  context.signal?.throwIfAborted()
  if (context.visitedNodes >= context.limits.visitedNodes) {
    markTruncated(context, {
      reason: "nodeLimit",
      visitedNodes: context.visitedNodes,
    })
    return undefined
  }
  context.visitedNodes += 1
  context.progress?.tick("serializing", context.visitedNodes)
  if (context.returnedNodes >= context.limits.returnedNodes) {
    markTruncated(context, {
      reason: "nodeLimit",
      visitedNodes: context.visitedNodes,
    })
    return undefined
  }

  const node = record(raw)
  const id = string(node.id)
  const repeated = id.length > 0 && ancestors.has(id)
  const isComponent = node.type === "COMPONENT" || node.type === "COMPONENT_SET"
  if (
    context.dedupeComponents &&
    isComponent &&
    id.length > 0 &&
    context.emittedComponents.has(id) &&
    !repeated
  ) {
    const stub: DesignNode<NodeData> = {
      summary: {
        id,
        name: string(node.name),
        nodeType: string(node.type),
        visible: boolean(node.visible, true),
      },
      data: identityData(context.detail, id),
      children: [],
      childrenTruncated: true,
    }
    const stubBytes = byteLength(stub)
    if (context.encodedBytes + stubBytes > context.limits.encodedBytes) {
      markTruncated(context, {
        reason: "byteLimit",
        encodedBytes: context.encodedBytes + stubBytes,
      })
      return undefined
    }
    context.encodedBytes += stubBytes
    context.returnedNodes += 1
    return stub
  }

  const children = visibleChildren(node, context.includeHidden)
  const result: DesignNode<NodeData> = {
    summary: summarize(node, children),
    data: nodeData(node, context.detail, context.instanceIdentities),
    children: [],
    childrenTruncated: false,
  }
  const candidateBytes = byteLength(result)
  if (context.encodedBytes + candidateBytes > context.limits.encodedBytes) {
    markTruncated(context, {
      reason: "byteLimit",
      encodedBytes: context.encodedBytes + candidateBytes,
    })
    return undefined
  }
  context.encodedBytes += candidateBytes
  context.returnedNodes += 1

  if (children.length === 0) {
    if (context.dedupeComponents && isComponent && id.length > 0)
      context.emittedComponents.add(id)
    return result
  }
  if (repeated) {
    const truncation: Truncation = {
      reason: "nodeLimit",
      visitedNodes: context.visitedNodes,
    }
    result.childrenTruncated = true
    result.childrenTruncation = truncation
    markTruncated(context, truncation)
    return result
  }
  if (context.dedupeComponents && isComponent && id.length > 0)
    context.emittedComponents.add(id)
  if (level >= context.depth) {
    const truncation: Truncation = {
      reason: "depthLimit",
      appliedDepth: context.depth,
    }
    result.childrenTruncated = true
    result.childrenTruncation = truncation
    markTruncated(context, truncation)
    return result
  }

  const nextAncestors = new Set(ancestors)
  if (id.length > 0) nextAncestors.add(id)
  for (let index = 0; index < children.length; index += 1) {
    throwIfAbortedAtBatch(context.signal, index, CANCEL_CHECK_BATCH)
    const child = serializeNode(
      children[index],
      level + 1,
      nextAncestors,
      context,
    )
    if (child === undefined) {
      result.childrenTruncated = true
      if (context.truncation !== undefined)
        result.childrenTruncation = context.truncation
      break
    }
    result.children.push(child)
  }
  return result
}

export interface ForestWalkOptions {
  readonly includeHidden?: boolean
  readonly signal?: CancellationSignal
  readonly progress?: ProgressReporter
  readonly limits?: Partial<SerializerLimits>
}

export interface ForestWalkContext {
  readonly visitedNodes: number
  readonly returnedNodes: number
  readonly encodedBytes: number
  tryReturn(payload: unknown): boolean
}

export interface ForestWalkResult {
  readonly truncated: boolean
  readonly truncation?: Truncation
  readonly visitedNodes: number
  readonly returnedNodes: number
}

function createWalkContext(options: ForestWalkOptions): SerializerContext {
  return {
    detail: "minimal",
    depth: Number.POSITIVE_INFINITY,
    dedupeComponents: false,
    includeHidden: options.includeHidden,
    instanceIdentities: undefined,
    signal: options.signal,
    progress: options.progress ?? progressFor(options.signal),
    emittedComponents: new Set<string>(),
    limits: {
      returnedNodes: options.limits?.returnedNodes ?? MAX_RETURNED_NODES,
      visitedNodes: options.limits?.visitedNodes ?? MAX_VISITED_NODES,
      encodedBytes: options.limits?.encodedBytes ?? MAX_TEXT_BYTES,
    },
    visitedNodes: 0,
    returnedNodes: 0,
    encodedBytes: 0,
  }
}

function walkVisitor(context: SerializerContext): ForestWalkContext {
  return {
    get visitedNodes() {
      return context.visitedNodes
    },
    get returnedNodes() {
      return context.returnedNodes
    },
    get encodedBytes() {
      return context.encodedBytes
    },
    tryReturn(payload: unknown): boolean {
      if (context.returnedNodes >= context.limits.returnedNodes) {
        markTruncated(context, {
          reason: "nodeLimit",
          visitedNodes: context.visitedNodes,
        })
        return false
      }
      const encodedBytes = context.encodedBytes + byteLength(payload)
      if (encodedBytes > context.limits.encodedBytes) {
        markTruncated(context, {
          reason: "byteLimit",
          encodedBytes,
        })
        return false
      }
      context.encodedBytes = encodedBytes
      context.returnedNodes += 1
      return true
    },
  }
}

function walkNode(
  raw: unknown,
  ancestors: ReadonlySet<string>,
  context: SerializerContext,
  visitor: ForestWalkContext,
  visit: (node: unknown, context: ForestWalkContext) => void,
): void {
  context.signal?.throwIfAborted()
  if (context.visitedNodes >= context.limits.visitedNodes) {
    markTruncated(context, {
      reason: "nodeLimit",
      visitedNodes: context.visitedNodes,
    })
    return
  }
  context.visitedNodes += 1
  context.progress?.tick("reading", context.visitedNodes)
  visit(raw, visitor)
  if (context.truncation !== undefined) return

  const node = record(raw)
  const id = string(node.id)
  if (id.length > 0 && ancestors.has(id)) return

  const children = visibleChildren(node, context.includeHidden)
  const nextAncestors = new Set(ancestors)
  if (id.length > 0) nextAncestors.add(id)
  for (let index = 0; index < children.length; index += 1) {
    throwIfAbortedAtBatch(context.signal, index, CANCEL_CHECK_BATCH)
    walkNode(children[index], nextAncestors, context, visitor, visit)
    if (context.truncation !== undefined) return
  }
}

export function walkNodeForest(
  roots: readonly unknown[],
  options: ForestWalkOptions,
  visit: (node: unknown, context: ForestWalkContext) => void,
): ForestWalkResult {
  const context = createWalkContext(options)
  const visitor = walkVisitor(context)
  for (const root of roots) {
    walkNode(root, new Set(), context, visitor, visit)
    if (context.truncation !== undefined) break
  }
  const result: ForestWalkResult = {
    truncated: context.truncation !== undefined,
    visitedNodes: context.visitedNodes,
    returnedNodes: context.returnedNodes,
  }
  return context.truncation === undefined
    ? result
    : { ...result, truncation: context.truncation }
}

export async function collectInstanceIdentities(
  roots: readonly unknown[],
  signal?: CancellationSignal,
  depth: number = Number.POSITIVE_INFINITY,
): Promise<Map<string, InstanceValue>> {
  const instances: UnknownRecord[] = []
  const visit = (raw: unknown, level: number): void => {
    signal?.throwIfAborted()
    const node = record(raw)
    if (node.type === "INSTANCE") instances.push(node)
    if (level >= depth) return
    const children = array(hostGet(node, "children"))
    for (let index = 0; index < children.length; index += 1) {
      throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
      visit(children[index], level + 1)
    }
  }
  for (const root of roots) visit(root, 0)
  const identities = new Map<string, InstanceValue>()
  for (const node of instances) {
    signal?.throwIfAborted()
    const id = string(node.id)
    if (id.length === 0 || identities.has(id)) continue
    const identity = await resolveInstanceIdentity(node)
    if (identity !== undefined) identities.set(id, identity)
  }
  return identities
}

async function resolveInstanceIdentity(
  node: UnknownRecord,
): Promise<InstanceValue | undefined> {
  let componentId = hostString(node, "componentId")
  let componentSetId = hostString(node, "componentSetId")
  const lookup = hostGet(node, "getMainComponentAsync")
  if (typeof lookup === "function") {
    try {
      const main = record(
        await settleOrSkip(lookup.call(node) as Promise<unknown>),
      )
      if (componentId.length === 0) componentId = string(main.id)
      const parent = record(main.parent)
      if (componentSetId.length === 0 && parent.type === "COMPONENT_SET") {
        componentSetId = string(parent.id)
      }
    } catch {
      // Dynamic-page and missing remotes must not fail the whole forest.
    }
  }
  if (componentId.length === 0) return undefined
  const value: InstanceValue = {
    componentId,
    properties: namedComponentProperties(node),
  }
  if (componentSetId.length > 0) value.componentSetId = componentSetId
  return value
}

export function serializeNodeForest(
  roots: readonly unknown[],
  options: SerializeNodeForestOptions,
): SerializedNodeForest {
  const context: SerializerContext = {
    detail: options.detail,
    depth: Math.max(0, options.depth),
    dedupeComponents: options.dedupeComponents,
    includeHidden: options.includeHidden,
    instanceIdentities: options.instanceIdentities,
    signal: options.signal,
    progress: options.progress ?? progressFor(options.signal),
    emittedComponents: new Set<string>(),
    limits: {
      returnedNodes: options.limits?.returnedNodes ?? MAX_RETURNED_NODES,
      visitedNodes: options.limits?.visitedNodes ?? MAX_VISITED_NODES,
      encodedBytes: options.limits?.encodedBytes ?? MAX_TEXT_BYTES,
    },
    visitedNodes: 0,
    returnedNodes: 0,
    encodedBytes: 0,
  }
  const nodes: DesignNode<NodeData>[] = []
  for (const root of roots) {
    const node = serializeNode(root, 0, new Set(), context)
    if (node === undefined) break
    nodes.push(node)
  }
  const result: SerializedNodeForest = {
    nodes,
    truncated: context.truncation !== undefined,
  }
  return context.truncation === undefined
    ? result
    : { ...result, truncation: context.truncation }
}
