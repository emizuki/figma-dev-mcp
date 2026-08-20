import { ERROR_CODES, type ErrorCode, type ReadResult } from "./protocol"
import type {
  AnimationBinding,
  AnimationTrack,
  AnnotationCategory,
  AnnotationValue,
  AppliedAnimationStyle,
  AppliedStyleProp,
  AppliedStylePropValue,
  AvailableAnimationStyle,
  AvailableStyleProp,
  AxisAlign,
  BlendMode,
  CodeSyntax,
  Color,
  CompactNodeData,
  ComponentDefinition,
  ComponentPropertyDefinition,
  ComponentPropertyValue,
  ComponentValue,
  ConstraintAxis,
  CornerRadiusValue,
  CubicBezier,
  DesignNode,
  DevModeNodeData,
  DevResource,
  DocumentationReference,
  EffectValue,
  FileMetadata,
  FontAvailability,
  FontName,
  FontUsage,
  FullNodeData,
  GeometryValue,
  GetComponentsResult,
  GetDesignContextResult,
  GetDevModeDataResult,
  GetFontsResult,
  GetMetadataResult,
  GetMotionResult,
  GetNodesResult,
  GetReactionsResult,
  GetScreenshotResult,
  GetSelectionResult,
  GetStylesResult,
  GetVariablesResult,
  GradientStop,
  InstanceRelationship,
  InstanceValue,
  ItemError,
  ItemResult,
  KeyframeField,
  KeyframeOperation,
  LayoutConstraints,
  LayoutMode,
  LayoutSizing,
  LayoutValue,
  LetterSpacingValue,
  LineHeightValue,
  ManualTrackBinding,
  MinimalNodeDetails,
  MotionEasing,
  MotionEasingType,
  MotionKeyframe,
  MotionKeyframeValue,
  MotionTimeline,
  NamedComponentProperty,
  NamedVariantProperty,
  NodeMatch,
  NodeMotion,
  NodeReactions,
  NodeSummary,
  ObservationWindow,
  OverlayBackground,
  OverlayBackgroundInteraction,
  OverlayPositionType,
  PageSummary,
  PaintValue,
  Reaction,
  ReactionAction,
  ReactionOverlay,
  ReactionTrigger,
  Rect,
  ScreenshotAsset,
  SearchNodesResult,
  StrokeAlign,
  StrokeValue,
  StyledTextRange,
  StyleKind,
  StyleReference,
  StyleIdentity,
  StyleValue,
  SvgRejection,
  SvgRejectionKind,
  TextAlignHorizontal,
  TextAlignVertical,
  TextAutoResize,
  TextDecoration,
  TextStyle,
  TextSummary,
  TextValue,
  ToolError,
  Transform2D,
  Truncation,
  TruncationReason,
  VariableCollection,
  VariableDefinition,
  VariableMode,
  VariableModeError,
  VariableModeValue,
  VariableReference,
  VariableValue,
} from "./results"
import {
  MAX_DEPTH,
  MAX_DISPLAY_TEXT_BYTES,
  MAX_IDENTIFIER_BYTES,
  MAX_INPUT_IDS,
  MAX_RASTER_PIXELS,
  MAX_RASTER_BASE64_BYTES,
  MAX_RASTER_SIDE,
  MAX_RETURNED_NODES,
  MAX_SVG_BYTES,
  U32_MAX,
} from "./limits"

function utf8ByteLength(value: string): number {
  let bytes = 0
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code <= 0x7f) {
      bytes += 1
    } else if (code <= 0x7ff) {
      bytes += 2
    } else if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      const paired = next >= 0xdc00 && next <= 0xdfff
      if (paired) index += 1
      bytes += paired ? 4 : 3
    } else {
      bytes += 3
    }
  }
  return bytes
}

function fail(message: string): never {
  throw new TypeError(`Invalid plugin result: ${message}`)
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return fail(`${label} must be an object`)
  }
  return value as Record<string, unknown>
}

function exact(
  value: unknown,
  label: string,
  required: readonly string[],
  optional: readonly string[] = [],
): Record<string, unknown> {
  const object = record(value, label)
  const allowed = new Set([...required, ...optional])
  for (const key of Object.keys(object)) {
    if (!allowed.has(key)) return fail(`${label} contains unknown field ${key}`)
  }
  for (const key of required) {
    if (!Object.hasOwn(object, key)) return fail(`${label} is missing ${key}`)
  }
  return object
}

function string(value: unknown, label: string): string {
  if (typeof value !== "string") return fail(`${label} must be a string`)
  return value
}

function boundedString(
  value: unknown,
  label: string,
  maximumBytes: number,
  allowEmpty = false,
): string {
  const parsed = string(value, label)
  if (!allowEmpty && parsed.length === 0)
    return fail(`${label} must not be blank`)
  if (utf8ByteLength(parsed) > maximumBytes) {
    return fail(`${label} exceeds ${maximumBytes} UTF-8 bytes`)
  }
  return parsed
}

function identifier(value: unknown, label: string): string {
  return boundedString(value, label, MAX_IDENTIFIER_BYTES)
}

function displayText(value: unknown, label: string): string {
  return boundedString(value, label, MAX_DISPLAY_TEXT_BYTES, true)
}

function boolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") return fail(`${label} must be boolean`)
  return value
}

function finite(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fail(`${label} must be finite`)
  }
  return value
}

function integer(
  value: unknown,
  label: string,
  maximum = Number.MAX_SAFE_INTEGER,
): number {
  const parsed = finite(value, label)
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > maximum) {
    return fail(`${label} must be an integer from 0 through ${maximum}`)
  }
  return parsed
}

export function parseU32(value: unknown, label: string): number {
  return integer(value, label, U32_MAX)
}

function arrayOf<T>(
  value: unknown,
  label: string,
  parser: (item: unknown, label: string) => T,
  maximum = MAX_RETURNED_NODES,
): T[] {
  if (!Array.isArray(value)) return fail(`${label} must be an array`)
  if (value.length > maximum) return fail(`${label} exceeds ${maximum} items`)
  const result: T[] = []
  for (let index = 0; index < value.length; index += 1) {
    result.push(parser(value[index], `${label}[${index}]`))
  }
  return result
}

function optionalString(
  object: Record<string, unknown>,
  key: string,
  parser: (value: unknown, label: string) => string = string,
): string | undefined {
  if (!Object.hasOwn(object, key)) return undefined
  return parser(object[key], key)
}

function oneOf<const Value extends string>(
  value: unknown,
  allowed: readonly Value[],
  label: string,
): Value {
  for (const candidate of allowed) if (value === candidate) return candidate
  return fail(`${label} is not an allowed value`)
}

const CANONICAL_MESSAGES: Record<ErrorCode, string> = {
  NO_FIGMA_CONNECTION: "No Figma connection is available.",
  AMBIGUOUS_CONNECTION: "More than one Figma connection matches the request.",
  CONNECTION_NOT_FOUND: "The requested Figma connection was not found.",
  CONNECTION_LOST: "The Figma connection was lost.",
  PROTOCOL_MISMATCH: "The plugin protocol version is not supported.",
  NODE_NOT_FOUND: "The requested node was not found.",
  PAGE_NOT_FOUND: "The requested page was not found.",
  UNSUPPORTED_NODE: "The requested node type is not supported.",
  CAPABILITY_UNAVAILABLE: "The required Figma capability is unavailable.",
  UNSAFE_SVG: "The SVG was rejected by the safety policy.",
  INVALID_CURSOR: "The search cursor is invalid or stale.",
  LIMIT_EXCEEDED: "The operation exceeded a safety limit.",
  TIMEOUT: "The operation timed out.",
  CANCELLED: "The operation was cancelled.",
  INTERNAL_ERROR: "The operation failed.",
}

function errorCode(value: unknown): ErrorCode {
  return oneOf(value, ERROR_CODES, "error code")
}

function parseItemError(value: unknown, label: string): ItemError {
  const object = exact(
    value,
    label,
    ["index", "code", "message", "retryable"],
    ["id"],
  )
  const code = errorCode(object.code)
  const message = string(object.message, `${label}.message`)
  if (message !== CANONICAL_MESSAGES[code])
    return fail(`${label}.message is not canonical`)
  const result: ItemError = {
    index: integer(object.index, `${label}.index`),
    code,
    message,
    retryable: boolean(object.retryable, `${label}.retryable`),
  }
  const id = optionalString(object, "id", identifier)
  if (id !== undefined) result.id = id
  return result
}

const SVG_REJECTION_KINDS: readonly SvgRejectionKind[] = [
  "parserError",
  "unsafeElement",
  "unsafeAttribute",
  "unsafeCss",
  "unsafeProcessingInstruction",
]

function parseSvgRejection(value: unknown, label: string): SvgRejection {
  const object = exact(value, label, ["kind"], ["name"])
  const result: SvgRejection = {
    kind: oneOf(object.kind, SVG_REJECTION_KINDS, `${label}.kind`),
  }
  const name = optionalString(object, "name", identifier)
  if (name !== undefined) result.name = name
  return result
}

function parseToolError(value: unknown, label: string): ToolError {
  const object = exact(
    value,
    label,
    ["code", "message", "retryable"],
    ["items", "svgRejection"],
  )
  const code = errorCode(object.code)
  const message = string(object.message, `${label}.message`)
  if (message !== CANONICAL_MESSAGES[code])
    return fail(`${label}.message is not canonical`)
  const result: ToolError = {
    code,
    message,
    retryable: boolean(object.retryable, `${label}.retryable`),
  }
  if (Object.hasOwn(object, "items")) {
    result.items = arrayOf(
      object.items,
      `${label}.items`,
      parseItemError,
      MAX_INPUT_IDS,
    )
  }
  if (Object.hasOwn(object, "svgRejection")) {
    result.svgRejection = parseSvgRejection(
      object.svgRejection,
      `${label}.svgRejection`,
    )
  }
  return result
}

function parseItemResult<T>(
  value: unknown,
  label: string,
  parser: (value: unknown, label: string) => T,
): ItemResult<T> {
  const object = record(value, label)
  switch (object.status) {
    case "success": {
      const success = exact(object, label, ["status", "value"])
      return {
        status: "success",
        value: parser(success.value, `${label}.value`),
      }
    }
    case "error": {
      const error = exact(object, label, ["status", "error"])
      return {
        status: "error",
        error: parseToolError(error.error, `${label}.error`),
      }
    }
    default:
      return fail(`${label}.status is not allowed`)
  }
}

function parseTruncation(value: unknown, label: string): Truncation {
  const object = exact(
    value,
    label,
    ["reason"],
    ["appliedDepth", "visitedNodes", "encodedBytes"],
  )
  const result: Truncation = {
    reason: oneOf<TruncationReason>(
      object.reason,
      ["depthLimit", "nodeLimit", "byteLimit"],
      `${label}.reason`,
    ),
  }
  if (Object.hasOwn(object, "appliedDepth")) {
    result.appliedDepth = integer(
      object.appliedDepth,
      `${label}.appliedDepth`,
      255,
    )
  }
  if (Object.hasOwn(object, "visitedNodes")) {
    result.visitedNodes = integer(object.visitedNodes, `${label}.visitedNodes`)
  }
  if (Object.hasOwn(object, "encodedBytes")) {
    result.encodedBytes = integer(object.encodedBytes, `${label}.encodedBytes`)
  }
  return result
}

function parseObservation(value: unknown, label: string): ObservationWindow {
  const object = exact(value, label, ["startedAt", "completedAt"])
  return {
    startedAt: displayText(object.startedAt, `${label}.startedAt`),
    completedAt: displayText(object.completedAt, `${label}.completedAt`),
  }
}

function parseCapabilitySet(value: unknown, label: string) {
  const fields = [
    "annotations",
    "devResources",
    "motion",
    "svgStringExport",
    "variableCodeSyntax",
  ] as const
  const object = exact(value, label, [], fields)
  return {
    annotations: Object.hasOwn(object, "annotations")
      ? boolean(object.annotations, `${label}.annotations`)
      : false,
    devResources: Object.hasOwn(object, "devResources")
      ? boolean(object.devResources, `${label}.devResources`)
      : false,
    motion: Object.hasOwn(object, "motion")
      ? boolean(object.motion, `${label}.motion`)
      : false,
    svgStringExport: Object.hasOwn(object, "svgStringExport")
      ? boolean(object.svgStringExport, `${label}.svgStringExport`)
      : false,
    variableCodeSyntax: Object.hasOwn(object, "variableCodeSyntax")
      ? boolean(object.variableCodeSyntax, `${label}.variableCodeSyntax`)
      : false,
  }
}

function withResultMetadata<T extends object>(
  object: Record<string, unknown>,
  value: T,
  label: string,
): T & {
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
} {
  const result = {
    ...value,
    truncated: boolean(object.truncated, `${label}.truncated`),
    observation: parseObservation(object.observation, `${label}.observation`),
  }
  if (Object.hasOwn(object, "truncation")) {
    return {
      ...result,
      truncation: parseTruncation(object.truncation, `${label}.truncation`),
    }
  }
  return result
}

function parseRect(value: unknown, label: string): Rect {
  const object = exact(value, label, ["x", "y", "width", "height"])
  return {
    x: finite(object.x, `${label}.x`),
    y: finite(object.y, `${label}.y`),
    width: finite(object.width, `${label}.width`),
    height: finite(object.height, `${label}.height`),
  }
}

function parseColor(value: unknown, label: string): Color {
  const object = exact(value, label, ["r", "g", "b", "a"])
  return {
    r: finite(object.r, `${label}.r`),
    g: finite(object.g, `${label}.g`),
    b: finite(object.b, `${label}.b`),
    a: finite(object.a, `${label}.a`),
  }
}

function parseGradientStop(value: unknown, label: string): GradientStop {
  const object = exact(value, label, ["position", "color"])
  return {
    position: finite(object.position, `${label}.position`),
    color: parseColor(object.color, `${label}.color`),
  }
}

function parsePaint(value: unknown, label: string): PaintValue {
  const object = record(value, label)
  switch (object.type) {
    case "solid": {
      const solid = exact(object, label, ["type", "color", "opacity"])
      return {
        type: "solid",
        color: parseColor(solid.color, `${label}.color`),
        opacity: finite(solid.opacity, `${label}.opacity`),
      }
    }
    case "linearGradient":
    case "radialGradient": {
      const gradient = exact(object, label, ["type", "stops"])
      return {
        type: object.type,
        stops: arrayOf(gradient.stops, `${label}.stops`, parseGradientStop),
      }
    }
    case "image": {
      const image = exact(object, label, ["type", "imageRef", "scaleMode"])
      return {
        type: "image",
        imageRef: string(image.imageRef, `${label}.imageRef`),
        scaleMode: oneOf(
          image.scaleMode,
          ["fill", "fit", "crop", "tile"],
          `${label}.scaleMode`,
        ),
      }
    }
    case "mixed":
      exact(object, label, ["type"])
      return { type: "mixed" }
    default:
      return fail(`${label}.type is not allowed`)
  }
}

function parseEffect(value: unknown, label: string): EffectValue {
  const object = record(value, label)
  switch (object.type) {
    case "dropShadow":
    case "innerShadow": {
      const shadow = exact(object, label, [
        "type",
        "color",
        "offsetX",
        "offsetY",
        "radius",
        "spread",
      ])
      return {
        type: object.type,
        color: parseColor(shadow.color, `${label}.color`),
        offsetX: finite(shadow.offsetX, `${label}.offsetX`),
        offsetY: finite(shadow.offsetY, `${label}.offsetY`),
        radius: finite(shadow.radius, `${label}.radius`),
        spread: finite(shadow.spread, `${label}.spread`),
      }
    }
    case "layerBlur":
    case "backgroundBlur": {
      const blur = exact(object, label, ["type", "radius"])
      return {
        type: object.type,
        radius: finite(blur.radius, `${label}.radius`),
      }
    }
    default:
      return fail(`${label}.type is not allowed`)
  }
}

function parseStrokes(value: unknown, label: string): StrokeValue {
  const object = exact(
    value,
    label,
    ["paints"],
    ["weight", "align", "dashPattern"],
  )
  const result: StrokeValue = {
    paints: arrayOf(object.paints, `${label}.paints`, parsePaint),
  }
  if (Object.hasOwn(object, "weight")) {
    result.weight = finite(object.weight, `${label}.weight`)
  }
  if (Object.hasOwn(object, "align")) {
    result.align = oneOf<StrokeAlign>(
      object.align,
      ["inside", "outside", "center"],
      `${label}.align`,
    )
  }
  if (Object.hasOwn(object, "dashPattern")) {
    result.dashPattern = arrayOf(
      object.dashPattern,
      `${label}.dashPattern`,
      finite,
    )
  }
  return result
}

function parseCornerRadius(value: unknown, label: string): CornerRadiusValue {
  const object = record(value, label)
  switch (object.kind) {
    case "uniform": {
      const uniform = exact(object, label, ["kind", "radius"])
      return {
        kind: "uniform",
        radius: finite(uniform.radius, `${label}.radius`),
      }
    }
    case "perCorner": {
      const corners = exact(object, label, [
        "kind",
        "topLeft",
        "topRight",
        "bottomRight",
        "bottomLeft",
      ])
      return {
        kind: "perCorner",
        topLeft: finite(corners.topLeft, `${label}.topLeft`),
        topRight: finite(corners.topRight, `${label}.topRight`),
        bottomRight: finite(corners.bottomRight, `${label}.bottomRight`),
        bottomLeft: finite(corners.bottomLeft, `${label}.bottomLeft`),
      }
    }
    default:
      return fail(`${label}.kind is not allowed`)
  }
}

const AXIS_ALIGN_VALUES: readonly AxisAlign[] = [
  "min",
  "center",
  "max",
  "spaceBetween",
  "baseline",
]

const BLEND_MODE_VALUES: readonly BlendMode[] = [
  "passThrough",
  "normal",
  "darken",
  "multiply",
  "linearBurn",
  "colorBurn",
  "lighten",
  "screen",
  "linearDodge",
  "colorDodge",
  "overlay",
  "softLight",
  "hardLight",
  "difference",
  "exclusion",
  "hue",
  "saturation",
  "color",
  "luminosity",
]

function parseLayout(value: unknown, label: string): LayoutValue {
  const object = exact(
    value,
    label,
    [
      "mode",
      "primarySizing",
      "counterSizing",
      "gap",
      "paddingTop",
      "paddingRight",
      "paddingBottom",
      "paddingLeft",
    ],
    ["primaryAlign", "counterAlign", "wrap", "counterAxisSpacing"],
  )
  const result: LayoutValue = {
    mode: oneOf<LayoutMode>(
      object.mode,
      ["none", "horizontal", "vertical", "grid"],
      `${label}.mode`,
    ),
    primarySizing: oneOf<LayoutSizing>(
      object.primarySizing,
      ["fixed", "hug", "fill"],
      `${label}.primarySizing`,
    ),
    counterSizing: oneOf<LayoutSizing>(
      object.counterSizing,
      ["fixed", "hug", "fill"],
      `${label}.counterSizing`,
    ),
    gap: finite(object.gap, `${label}.gap`),
    paddingTop: finite(object.paddingTop, `${label}.paddingTop`),
    paddingRight: finite(object.paddingRight, `${label}.paddingRight`),
    paddingBottom: finite(object.paddingBottom, `${label}.paddingBottom`),
    paddingLeft: finite(object.paddingLeft, `${label}.paddingLeft`),
  }
  if (Object.hasOwn(object, "primaryAlign")) {
    result.primaryAlign = oneOf<AxisAlign>(
      object.primaryAlign,
      AXIS_ALIGN_VALUES,
      `${label}.primaryAlign`,
    )
  }
  if (Object.hasOwn(object, "counterAlign")) {
    result.counterAlign = oneOf<AxisAlign>(
      object.counterAlign,
      AXIS_ALIGN_VALUES,
      `${label}.counterAlign`,
    )
  }
  if (Object.hasOwn(object, "wrap")) {
    result.wrap = boolean(object.wrap, `${label}.wrap`)
  }
  if (Object.hasOwn(object, "counterAxisSpacing")) {
    result.counterAxisSpacing = finite(
      object.counterAxisSpacing,
      `${label}.counterAxisSpacing`,
    )
  }
  return result
}

function parseLineHeight(value: unknown, label: string): LineHeightValue {
  const object = exact(value, label, ["unit"], ["value"])
  const hasValue = Object.hasOwn(object, "value")
  switch (object.unit) {
    case "pixels":
      if (!hasValue) return fail(`${label} is missing value`)
      return { unit: "pixels", value: finite(object.value, `${label}.value`) }
    case "percent":
      if (!hasValue) return fail(`${label} is missing value`)
      return { unit: "percent", value: finite(object.value, `${label}.value`) }
    case "auto":
      if (hasValue) return fail(`${label} must not carry a value`)
      return { unit: "auto" }
    default:
      return fail(`${label}.unit is not allowed`)
  }
}

function parseLetterSpacing(value: unknown, label: string): LetterSpacingValue {
  const object = exact(value, label, ["unit", "value"])
  switch (object.unit) {
    case "pixels":
      return { unit: "pixels", value: finite(object.value, `${label}.value`) }
    case "percent":
      return { unit: "percent", value: finite(object.value, `${label}.value`) }
    default:
      return fail(`${label}.unit is not allowed`)
  }
}

function parseTextStyle(value: unknown, label: string): TextStyle {
  const object = exact(
    value,
    label,
    ["fontFamily", "fontStyle", "paints"],
    ["fontSize", "lineHeight", "letterSpacing", "fontWeight", "textDecoration"],
  )
  const result: TextStyle = {
    fontFamily: string(object.fontFamily, `${label}.fontFamily`),
    fontStyle: string(object.fontStyle, `${label}.fontStyle`),
    paints: arrayOf(object.paints, `${label}.paints`, parsePaint),
  }
  if (Object.hasOwn(object, "fontSize")) {
    result.fontSize = finite(object.fontSize, `${label}.fontSize`)
  }
  if (Object.hasOwn(object, "lineHeight")) {
    result.lineHeight = parseLineHeight(
      object.lineHeight,
      `${label}.lineHeight`,
    )
  }
  if (Object.hasOwn(object, "letterSpacing")) {
    result.letterSpacing = parseLetterSpacing(
      object.letterSpacing,
      `${label}.letterSpacing`,
    )
  }
  if (Object.hasOwn(object, "fontWeight")) {
    result.fontWeight = finite(object.fontWeight, `${label}.fontWeight`)
  }
  if (Object.hasOwn(object, "textDecoration")) {
    result.textDecoration = oneOf<TextDecoration>(
      object.textDecoration,
      ["none", "underline", "strikethrough"],
      `${label}.textDecoration`,
    )
  }
  return result
}

function parseStyledRange(value: unknown, label: string): StyledTextRange {
  const object = exact(value, label, ["start", "end", "style"])
  return {
    start: integer(object.start, `${label}.start`),
    end: integer(object.end, `${label}.end`),
    style: parseTextStyle(object.style, `${label}.style`),
  }
}

function parseTextValue(value: unknown, label: string): TextValue {
  const object = exact(
    value,
    label,
    ["characters", "defaultStyle", "styledRanges"],
    ["alignHorizontal", "alignVertical", "autoResize"],
  )
  const result: TextValue = {
    characters: string(object.characters, `${label}.characters`),
    defaultStyle: parseTextStyle(object.defaultStyle, `${label}.defaultStyle`),
    styledRanges: arrayOf(
      object.styledRanges,
      `${label}.styledRanges`,
      parseStyledRange,
    ),
  }
  if (Object.hasOwn(object, "alignHorizontal")) {
    result.alignHorizontal = oneOf<TextAlignHorizontal>(
      object.alignHorizontal,
      ["left", "center", "right", "justified"],
      `${label}.alignHorizontal`,
    )
  }
  if (Object.hasOwn(object, "alignVertical")) {
    result.alignVertical = oneOf<TextAlignVertical>(
      object.alignVertical,
      ["top", "center", "bottom"],
      `${label}.alignVertical`,
    )
  }
  if (Object.hasOwn(object, "autoResize")) {
    result.autoResize = oneOf<TextAutoResize>(
      object.autoResize,
      ["none", "widthAndHeight", "height", "truncate"],
      `${label}.autoResize`,
    )
  }
  return result
}

function parseTextSummary(value: unknown, label: string): TextSummary {
  const object = exact(value, label, ["characterCount", "preview"])
  return {
    characterCount: integer(object.characterCount, `${label}.characterCount`),
    preview: string(object.preview, `${label}.preview`),
  }
}

function parseComponentProperty(
  value: unknown,
  label: string,
): ComponentPropertyValue {
  const object = exact(value, label, ["kind", "value"])
  switch (object.kind) {
    case "text":
      return { kind: "text", value: string(object.value, `${label}.value`) }
    case "boolean":
      return { kind: "boolean", value: boolean(object.value, `${label}.value`) }
    case "instanceSwap":
      return {
        kind: "instanceSwap",
        value: string(object.value, `${label}.value`),
      }
    case "variant":
      return { kind: "variant", value: string(object.value, `${label}.value`) }
    default:
      return fail(`${label}.kind is not allowed`)
  }
}

function parseNamedComponentProperty(
  value: unknown,
  label: string,
): NamedComponentProperty {
  const object = exact(value, label, ["name", "value"])
  return {
    name: string(object.name, `${label}.name`),
    value: parseComponentProperty(object.value, `${label}.value`),
  }
}

function parseComponentValue(value: unknown, label: string): ComponentValue {
  const object = exact(
    value,
    label,
    ["componentId", "properties"],
    ["componentSetId"],
  )
  const result: ComponentValue = {
    componentId: identifier(object.componentId, `${label}.componentId`),
    properties: arrayOf(
      object.properties,
      `${label}.properties`,
      parseNamedComponentProperty,
    ),
  }
  const componentSetId = optionalString(object, "componentSetId", identifier)
  if (componentSetId !== undefined) result.componentSetId = componentSetId
  return result
}

function parseVariableValue(value: unknown, label: string): VariableValue {
  const object = exact(value, label, ["kind", "value"])
  switch (object.kind) {
    case "boolean":
      return { kind: "boolean", value: boolean(object.value, `${label}.value`) }
    case "float":
      return { kind: "float", value: finite(object.value, `${label}.value`) }
    case "string":
      return { kind: "string", value: string(object.value, `${label}.value`) }
    case "color":
      return {
        kind: "color",
        value: parseColor(object.value, `${label}.value`),
      }
    case "alias":
      return { kind: "alias", value: string(object.value, `${label}.value`) }
    default:
      return fail(`${label}.kind is not allowed`)
  }
}

function parseTransform(value: unknown, label: string): Transform2D {
  const object = exact(value, label, ["m00", "m01", "m02", "m10", "m11", "m12"])
  return {
    m00: finite(object.m00, `${label}.m00`),
    m01: finite(object.m01, `${label}.m01`),
    m02: finite(object.m02, `${label}.m02`),
    m10: finite(object.m10, `${label}.m10`),
    m11: finite(object.m11, `${label}.m11`),
    m12: finite(object.m12, `${label}.m12`),
  }
}

function parseGeometry(value: unknown, label: string): GeometryValue {
  const object = exact(
    value,
    label,
    ["rotation", "opacity", "transform"],
    ["bounds"],
  )
  const result: GeometryValue = {
    rotation: finite(object.rotation, `${label}.rotation`),
    opacity: finite(object.opacity, `${label}.opacity`),
    transform: parseTransform(object.transform, `${label}.transform`),
  }
  if (Object.hasOwn(object, "bounds"))
    result.bounds = parseRect(object.bounds, `${label}.bounds`)
  return result
}

function parseConstraints(value: unknown, label: string): LayoutConstraints {
  const object = exact(value, label, ["horizontal", "vertical"])
  const allowed: readonly ConstraintAxis[] = [
    "min",
    "center",
    "max",
    "stretch",
    "scale",
  ]
  return {
    horizontal: oneOf(object.horizontal, allowed, `${label}.horizontal`),
    vertical: oneOf(object.vertical, allowed, `${label}.vertical`),
  }
}

function parseStyleReference(value: unknown, label: string): StyleReference {
  const object = exact(value, label, ["id", "kind"], ["name"])
  const result: StyleReference = {
    id: string(object.id, `${label}.id`),
    kind: oneOf<StyleKind>(
      object.kind,
      ["paint", "stroke", "text", "effect", "grid"],
      `${label}.kind`,
    ),
  }
  if (Object.hasOwn(object, "name"))
    result.name = displayText(object.name, `${label}.name`)
  return result
}

function parseVariableReference(
  value: unknown,
  label: string,
): VariableReference {
  const object = exact(value, label, ["id"], ["name"])
  const result: VariableReference = { id: string(object.id, `${label}.id`) }
  const name = optionalString(object, "name")
  if (name !== undefined) result.name = name
  return result
}

function parseInstanceValue(value: unknown, label: string): InstanceValue {
  return parseComponentValue(value, label)
}

function parseNodeSummary(value: unknown, label: string): NodeSummary {
  const object = exact(
    value,
    label,
    ["id", "name", "nodeType", "visible"],
    ["parentId", "childIds", "bounds"],
  )
  const result: NodeSummary = {
    id: identifier(object.id, `${label}.id`),
    name: displayText(object.name, `${label}.name`),
    nodeType: identifier(object.nodeType, `${label}.nodeType`),
    visible: boolean(object.visible, `${label}.visible`),
  }
  const parentId = optionalString(object, "parentId", identifier)
  if (parentId !== undefined) result.parentId = parentId
  if (Object.hasOwn(object, "childIds")) {
    const childIds = arrayOf(
      object.childIds,
      `${label}.childIds`,
      identifier,
      MAX_INPUT_IDS,
    )
    if (childIds.length > 0) result.childIds = childIds
  }
  if (Object.hasOwn(object, "bounds"))
    result.bounds = parseRect(object.bounds, `${label}.bounds`)
  return result
}

function parseCompactData(value: unknown, label: string): CompactNodeData {
  const object = exact(
    value,
    label,
    ["styleReferences", "variableReferences"],
    ["geometry", "constraints", "autoLayout", "text", "component", "instance"],
  )
  const result: CompactNodeData = {
    styleReferences: arrayOf(
      object.styleReferences,
      `${label}.styleReferences`,
      parseStyleReference,
    ),
    variableReferences: arrayOf(
      object.variableReferences,
      `${label}.variableReferences`,
      parseVariableReference,
    ),
  }
  if (Object.hasOwn(object, "geometry"))
    result.geometry = parseGeometry(object.geometry, `${label}.geometry`)
  if (Object.hasOwn(object, "constraints"))
    result.constraints = parseConstraints(
      object.constraints,
      `${label}.constraints`,
    )
  if (Object.hasOwn(object, "autoLayout"))
    result.autoLayout = parseLayout(object.autoLayout, `${label}.autoLayout`)
  if (Object.hasOwn(object, "text"))
    result.text = parseTextSummary(object.text, `${label}.text`)
  if (Object.hasOwn(object, "component"))
    result.component = parseComponentValue(
      object.component,
      `${label}.component`,
    )
  if (Object.hasOwn(object, "instance"))
    result.instance = parseInstanceValue(object.instance, `${label}.instance`)
  return result
}

function parseFullData(value: unknown, label: string): FullNodeData {
  const object = exact(
    value,
    label,
    ["paints", "effects", "styleReferences", "variableReferences"],
    [
      "geometry",
      "constraints",
      "autoLayout",
      "text",
      "component",
      "instance",
      "strokes",
      "cornerRadius",
      "cornerSmoothing",
      "clipsContent",
      "blendMode",
    ],
  )
  const result: FullNodeData = {
    paints: arrayOf(object.paints, `${label}.paints`, parsePaint),
    effects: arrayOf(object.effects, `${label}.effects`, parseEffect),
    styleReferences: arrayOf(
      object.styleReferences,
      `${label}.styleReferences`,
      parseStyleReference,
    ),
    variableReferences: arrayOf(
      object.variableReferences,
      `${label}.variableReferences`,
      parseVariableReference,
    ),
  }
  if (Object.hasOwn(object, "geometry"))
    result.geometry = parseGeometry(object.geometry, `${label}.geometry`)
  if (Object.hasOwn(object, "constraints"))
    result.constraints = parseConstraints(
      object.constraints,
      `${label}.constraints`,
    )
  if (Object.hasOwn(object, "autoLayout"))
    result.autoLayout = parseLayout(object.autoLayout, `${label}.autoLayout`)
  if (Object.hasOwn(object, "text"))
    result.text = parseTextValue(object.text, `${label}.text`)
  if (Object.hasOwn(object, "component"))
    result.component = parseComponentValue(
      object.component,
      `${label}.component`,
    )
  if (Object.hasOwn(object, "instance"))
    result.instance = parseInstanceValue(object.instance, `${label}.instance`)
  if (Object.hasOwn(object, "strokes"))
    result.strokes = parseStrokes(object.strokes, `${label}.strokes`)
  if (Object.hasOwn(object, "cornerRadius"))
    result.cornerRadius = parseCornerRadius(
      object.cornerRadius,
      `${label}.cornerRadius`,
    )
  if (Object.hasOwn(object, "cornerSmoothing"))
    result.cornerSmoothing = finite(
      object.cornerSmoothing,
      `${label}.cornerSmoothing`,
    )
  if (Object.hasOwn(object, "clipsContent"))
    result.clipsContent = boolean(object.clipsContent, `${label}.clipsContent`)
  if (Object.hasOwn(object, "blendMode"))
    result.blendMode = oneOf<BlendMode>(
      object.blendMode,
      BLEND_MODE_VALUES,
      `${label}.blendMode`,
    )
  return result
}

interface NodeBudget {
  count: number
}

type NodeDataParser<Data> = (value: unknown, label: string) => Data

function parseMinimalData(value: unknown, label: string): MinimalNodeDetails {
  exact(value, label, [])
  return {}
}

function parseDesignNode<Data>(
  value: unknown,
  label: string,
  parseData: NodeDataParser<Data>,
  depth: number,
  budget: NodeBudget,
): DesignNode<Data> {
  if (depth > MAX_DEPTH) return fail(`${label} exceeds node depth ${MAX_DEPTH}`)
  budget.count += 1
  if (budget.count > MAX_RETURNED_NODES) {
    return fail(`returned node count exceeds ${MAX_RETURNED_NODES}`)
  }
  const object = exact(
    value,
    label,
    ["summary", "data", "children", "childrenTruncated"],
    ["childrenTruncation"],
  )
  const data = parseData(object.data, `${label}.data`)
  if (!Array.isArray(object.children))
    return fail(`${label}.children must be an array`)
  if (object.children.length > MAX_RETURNED_NODES) {
    return fail(`${label}.children exceeds ${MAX_RETURNED_NODES} items`)
  }
  const children: DesignNode<Data>[] = []
  for (let index = 0; index < object.children.length; index += 1) {
    children.push(
      parseDesignNode(
        object.children[index],
        `${label}.children[${index}]`,
        parseData,
        depth + 1,
        budget,
      ),
    )
  }
  const result: DesignNode<Data> = {
    summary: parseNodeSummary(object.summary, `${label}.summary`),
    data,
    children,
    childrenTruncated: boolean(
      object.childrenTruncated,
      `${label}.childrenTruncated`,
    ),
  }
  if (Object.hasOwn(object, "childrenTruncation")) {
    result.childrenTruncation = parseTruncation(
      object.childrenTruncation,
      `${label}.childrenTruncation`,
    )
  }
  return result
}

function parseNodeForest<Data>(
  value: unknown,
  label: string,
  parseData: NodeDataParser<Data>,
): DesignNode<Data>[] {
  if (!Array.isArray(value)) return fail(`${label} must be an array`)
  if (value.length > MAX_RETURNED_NODES)
    return fail(`${label} exceeds node limit`)
  const budget: NodeBudget = { count: 0 }
  const result: DesignNode<Data>[] = []
  for (let index = 0; index < value.length; index += 1) {
    result.push(
      parseDesignNode(value[index], `${label}[${index}]`, parseData, 0, budget),
    )
  }
  return result
}

function parseNodeBatch<Data>(
  value: unknown,
  label: string,
  parseData: NodeDataParser<Data>,
): ItemResult<DesignNode<Data>>[] {
  if (!Array.isArray(value)) return fail(`${label} must be an array`)
  if (value.length > MAX_INPUT_IDS)
    return fail(`${label} exceeds ${MAX_INPUT_IDS} items`)
  const budget: NodeBudget = { count: 0 }
  const result: ItemResult<DesignNode<Data>>[] = []
  for (let index = 0; index < value.length; index += 1) {
    result.push(
      parseItemResult(value[index], `${label}[${index}]`, (item, itemLabel) =>
        parseDesignNode(item, itemLabel, parseData, 0, budget),
      ),
    )
  }
  return result
}

function parseSelectionResult(
  value: unknown,
  label: string,
): GetSelectionResult {
  const object = exact(
    value,
    label,
    ["detail", "nodes", "truncated", "observation"],
    ["truncation"],
  )
  switch (object.detail) {
    case "minimal":
      return withResultMetadata(
        object,
        {
          detail: "minimal",
          nodes: parseNodeForest(
            object.nodes,
            `${label}.nodes`,
            parseMinimalData,
          ),
        },
        label,
      )
    case "compact":
      return withResultMetadata(
        object,
        {
          detail: "compact",
          nodes: parseNodeForest(
            object.nodes,
            `${label}.nodes`,
            parseCompactData,
          ),
        },
        label,
      )
    case "full":
      return withResultMetadata(
        object,
        {
          detail: "full",
          nodes: parseNodeForest(object.nodes, `${label}.nodes`, parseFullData),
        },
        label,
      )
    default:
      return fail(`${label}.detail is not allowed`)
  }
}

function parseNodesResult(value: unknown, label: string): GetNodesResult {
  const object = exact(
    value,
    label,
    ["detail", "items", "truncated", "observation"],
    ["truncation"],
  )
  switch (object.detail) {
    case "minimal":
      return withResultMetadata(
        object,
        {
          detail: "minimal",
          items: parseNodeBatch(
            object.items,
            `${label}.items`,
            parseMinimalData,
          ),
        },
        label,
      )
    case "compact":
      return withResultMetadata(
        object,
        {
          detail: "compact",
          items: parseNodeBatch(
            object.items,
            `${label}.items`,
            parseCompactData,
          ),
        },
        label,
      )
    case "full":
      return withResultMetadata(
        object,
        {
          detail: "full",
          items: parseNodeBatch(object.items, `${label}.items`, parseFullData),
        },
        label,
      )
    default:
      return fail(`${label}.detail is not allowed`)
  }
}

function parseDesignContextResult(
  value: unknown,
  label: string,
): GetDesignContextResult {
  const object = exact(
    value,
    label,
    ["detail", "roots", "truncated", "observation"],
    ["truncation"],
  )
  switch (object.detail) {
    case "minimal":
      return withResultMetadata(
        object,
        {
          detail: "minimal",
          roots: parseNodeForest(
            object.roots,
            `${label}.roots`,
            parseMinimalData,
          ),
        },
        label,
      )
    case "compact":
      return withResultMetadata(
        object,
        {
          detail: "compact",
          roots: parseNodeForest(
            object.roots,
            `${label}.roots`,
            parseCompactData,
          ),
        },
        label,
      )
    case "full":
      return withResultMetadata(
        object,
        {
          detail: "full",
          roots: parseNodeForest(object.roots, `${label}.roots`, parseFullData),
        },
        label,
      )
    default:
      return fail(`${label}.detail is not allowed`)
  }
}

function parseFileMetadata(value: unknown, label: string): FileMetadata {
  const object = exact(value, label, ["name", "editorType"], ["key"])
  const result: FileMetadata = {
    name: displayText(object.name, `${label}.name`),
    editorType: displayText(object.editorType, `${label}.editorType`),
  }
  const key = optionalString(object, "key", identifier)
  if (key !== undefined) result.key = key
  return result
}

function parsePageSummary(value: unknown, label: string): PageSummary {
  const object = exact(value, label, ["id", "name"])
  return {
    id: identifier(object.id, `${label}.id`),
    name: displayText(object.name, `${label}.name`),
  }
}

function parseMetadataResult(value: unknown, label: string): GetMetadataResult {
  const object = exact(
    value,
    label,
    [
      "file",
      "pages",
      "currentPageId",
      "pluginVersion",
      "capabilities",
      "truncated",
      "observation",
    ],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    {
      file: parseFileMetadata(object.file, `${label}.file`),
      pages: arrayOf(object.pages, `${label}.pages`, parsePageSummary),
      currentPageId: identifier(object.currentPageId, `${label}.currentPageId`),
      pluginVersion: displayText(
        object.pluginVersion,
        `${label}.pluginVersion`,
      ),
      capabilities: parseCapabilitySet(
        object.capabilities,
        `${label}.capabilities`,
      ),
    },
    label,
  )
}

function parseNodeMatch(value: unknown, label: string): NodeMatch {
  const object = exact(value, label, ["node", "reasons"])
  return {
    node: parseNodeSummary(object.node, `${label}.node`),
    reasons: arrayOf(object.reasons, `${label}.reasons`, string),
  }
}

function parseSearchResult(value: unknown, label: string): SearchNodesResult {
  const object = exact(
    value,
    label,
    ["matches", "truncated", "observation"],
    ["truncation", "nextCursor"],
  )
  return withResultMetadata(
    object,
    {
      matches: arrayOf(object.matches, `${label}.matches`, parseNodeMatch),
      ...(Object.hasOwn(object, "nextCursor")
        ? { nextCursor: string(object.nextCursor, `${label}.nextCursor`) }
        : {}),
    },
    label,
  )
}

const STYLE_IDENTITY_FIELDS = ["description", "remote", "key"] as const

function parseStyleIdentity(
  object: Record<string, unknown>,
  label: string,
): StyleIdentity {
  const identity: StyleIdentity = {
    id: string(object.id, `${label}.id`),
    name: string(object.name, `${label}.name`),
  }
  const description = optionalString(object, "description")
  if (description !== undefined) identity.description = description
  if (Object.hasOwn(object, "remote")) {
    identity.remote = boolean(object.remote, `${label}.remote`)
  }
  const key = optionalString(object, "key")
  if (key !== undefined) identity.key = key
  return identity
}

function parseStyle(value: unknown, label: string): StyleValue {
  const object = record(value, label)
  switch (object.styleType) {
    case "paint": {
      const style = exact(
        object,
        label,
        ["styleType", "id", "name", "paints"],
        [...STYLE_IDENTITY_FIELDS],
      )
      return {
        styleType: "paint",
        ...parseStyleIdentity(style, label),
        paints: arrayOf(style.paints, `${label}.paints`, parsePaint),
      }
    }
    case "text": {
      const style = exact(
        object,
        label,
        ["styleType", "id", "name", "text"],
        [...STYLE_IDENTITY_FIELDS],
      )
      return {
        styleType: "text",
        ...parseStyleIdentity(style, label),
        text: parseTextValue(style.text, `${label}.text`),
      }
    }
    case "effect": {
      const style = exact(
        object,
        label,
        ["styleType", "id", "name", "effects"],
        [...STYLE_IDENTITY_FIELDS],
      )
      return {
        styleType: "effect",
        ...parseStyleIdentity(style, label),
        effects: arrayOf(style.effects, `${label}.effects`, parseEffect),
      }
    }
    case "grid": {
      const style = exact(
        object,
        label,
        ["styleType", "id", "name", "pattern", "size"],
        [...STYLE_IDENTITY_FIELDS],
      )
      return {
        styleType: "grid",
        ...parseStyleIdentity(style, label),
        pattern: string(style.pattern, `${label}.pattern`),
        size: finite(style.size, `${label}.size`),
      }
    }
    default:
      return fail(`${label}.styleType is not allowed`)
  }
}

function parseStylesResult(value: unknown, label: string): GetStylesResult {
  const object = exact(
    value,
    label,
    ["styles", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    { styles: arrayOf(object.styles, `${label}.styles`, parseStyle) },
    label,
  )
}

function parseCodeSyntax(value: unknown, label: string): CodeSyntax {
  const object = exact(value, label, ["platform", "code"])
  return {
    platform: string(object.platform, `${label}.platform`),
    code: string(object.code, `${label}.code`),
  }
}

function parseVariableModeError(
  value: unknown,
  label: string,
): VariableModeError {
  const object = exact(value, label, ["code", "retryable"])
  return {
    code: errorCode(object.code),
    retryable: boolean(object.retryable, `${label}.retryable`),
  }
}

function parseVariableModeValue(
  value: unknown,
  label: string,
): VariableModeValue {
  const object = exact(
    value,
    label,
    ["modeId", "source"],
    ["resolved", "error"],
  )
  const result: VariableModeValue = {
    modeId: string(object.modeId, `${label}.modeId`),
    source: parseVariableValue(object.source, `${label}.source`),
  }
  if (Object.hasOwn(object, "resolved")) {
    result.resolved = parseVariableValue(object.resolved, `${label}.resolved`)
  }
  if (Object.hasOwn(object, "error")) {
    result.error = parseVariableModeError(object.error, `${label}.error`)
  }
  return result
}

function parseVariableDefinition(
  value: unknown,
  label: string,
): VariableDefinition {
  const object = exact(value, label, [
    "id",
    "name",
    "collectionId",
    "scopes",
    "values",
    "codeSyntax",
  ])
  return {
    id: string(object.id, `${label}.id`),
    name: string(object.name, `${label}.name`),
    collectionId: string(object.collectionId, `${label}.collectionId`),
    scopes: arrayOf(object.scopes, `${label}.scopes`, string),
    values: arrayOf(object.values, `${label}.values`, parseVariableModeValue),
    codeSyntax: arrayOf(
      object.codeSyntax,
      `${label}.codeSyntax`,
      parseCodeSyntax,
    ),
  }
}

function parseVariableMode(value: unknown, label: string): VariableMode {
  const object = exact(value, label, ["id", "name"])
  return {
    id: string(object.id, `${label}.id`),
    name: string(object.name, `${label}.name`),
  }
}

function parseVariableCollection(
  value: unknown,
  label: string,
): VariableCollection {
  const object = exact(value, label, ["id", "name", "modes", "variables"])
  return {
    id: string(object.id, `${label}.id`),
    name: string(object.name, `${label}.name`),
    modes: arrayOf(object.modes, `${label}.modes`, parseVariableMode),
    variables: arrayOf(
      object.variables,
      `${label}.variables`,
      parseVariableDefinition,
    ),
  }
}

function parseVariablesResult(
  value: unknown,
  label: string,
): GetVariablesResult {
  const object = exact(
    value,
    label,
    ["collections", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    {
      collections: arrayOf(
        object.collections,
        `${label}.collections`,
        parseVariableCollection,
      ),
    },
    label,
  )
}

function parseDocumentation(
  value: unknown,
  label: string,
): DocumentationReference {
  const object = exact(value, label, ["uri"], ["label"])
  const result: DocumentationReference = {
    uri: string(object.uri, `${label}.uri`),
  }
  const optionalLabel = optionalString(object, "label")
  if (optionalLabel !== undefined) result.label = optionalLabel
  return result
}

function parsePropertyDefinition(
  value: unknown,
  label: string,
): ComponentPropertyDefinition {
  const object = exact(
    value,
    label,
    ["name", "defaultValue"],
    ["preferredValues"],
  )
  const result: ComponentPropertyDefinition = {
    name: string(object.name, `${label}.name`),
    defaultValue: parseComponentProperty(
      object.defaultValue,
      `${label}.defaultValue`,
    ),
  }
  if (Object.hasOwn(object, "preferredValues")) {
    const values = arrayOf(
      object.preferredValues,
      `${label}.preferredValues`,
      parseComponentProperty,
    )
    if (values.length > 0) result.preferredValues = values
  }
  return result
}

function parseNamedVariant(
  value: unknown,
  label: string,
): NamedVariantProperty {
  const object = exact(value, label, ["name", "value"])
  return {
    name: string(object.name, `${label}.name`),
    value: string(object.value, `${label}.value`),
  }
}

function parseComponentDefinition(
  value: unknown,
  label: string,
): ComponentDefinition {
  const object = exact(
    value,
    label,
    ["id", "name", "documentation", "variantProperties", "propertyDefinitions"],
    ["componentSetId", "description"],
  )
  const result: ComponentDefinition = {
    id: identifier(object.id, `${label}.id`),
    name: string(object.name, `${label}.name`),
    documentation: arrayOf(
      object.documentation,
      `${label}.documentation`,
      parseDocumentation,
    ),
    variantProperties: arrayOf(
      object.variantProperties,
      `${label}.variantProperties`,
      parseNamedVariant,
    ),
    propertyDefinitions: arrayOf(
      object.propertyDefinitions,
      `${label}.propertyDefinitions`,
      parsePropertyDefinition,
    ),
  }
  const componentSetId = optionalString(object, "componentSetId", identifier)
  if (componentSetId !== undefined) result.componentSetId = componentSetId
  const description = optionalString(object, "description")
  if (description !== undefined) result.description = description
  return result
}

function parseInstanceRelationship(
  value: unknown,
  label: string,
): InstanceRelationship {
  const object = exact(value, label, ["instanceId", "componentId"])
  return {
    instanceId: identifier(object.instanceId, `${label}.instanceId`),
    componentId: identifier(object.componentId, `${label}.componentId`),
  }
}

function parseComponentsResult(
  value: unknown,
  label: string,
): GetComponentsResult {
  const object = exact(
    value,
    label,
    ["components", "instances", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    {
      components: arrayOf(
        object.components,
        `${label}.components`,
        parseComponentDefinition,
      ),
      instances: arrayOf(
        object.instances,
        `${label}.instances`,
        parseInstanceRelationship,
      ),
    },
    label,
  )
}

function parseFontName(value: unknown, label: string): FontName {
  const object = exact(value, label, ["family", "style"])
  return {
    family: string(object.family, `${label}.family`),
    style: string(object.style, `${label}.style`),
  }
}

function parseFontUsage(value: unknown, label: string): FontUsage {
  const object = exact(value, label, ["font", "availability", "nodeIds"])
  return {
    font: parseFontName(object.font, `${label}.font`),
    availability: oneOf<FontAvailability>(
      object.availability,
      ["available", "unavailable", "unknown"],
      `${label}.availability`,
    ),
    nodeIds: arrayOf(
      object.nodeIds,
      `${label}.nodeIds`,
      identifier,
      MAX_INPUT_IDS,
    ),
  }
}

function parseFontsResult(value: unknown, label: string): GetFontsResult {
  const object = exact(
    value,
    label,
    ["fonts", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    { fonts: arrayOf(object.fonts, `${label}.fonts`, parseFontUsage) },
    label,
  )
}

function parseAnnotation(value: unknown, label: string): AnnotationValue {
  const object = exact(value, label, ["id", "text"], ["categoryId"])
  const result: AnnotationValue = {
    id: string(object.id, `${label}.id`),
    text: string(object.text, `${label}.text`),
  }
  const categoryId = optionalString(object, "categoryId")
  if (categoryId !== undefined) result.categoryId = categoryId
  return result
}

function parseAnnotationCategory(
  value: unknown,
  label: string,
): AnnotationCategory {
  const object = exact(value, label, ["id", "label"])
  return {
    id: string(object.id, `${label}.id`),
    label: string(object.label, `${label}.label`),
  }
}

function parseDevResource(value: unknown, label: string): DevResource {
  const object = exact(value, label, ["name", "uri"])
  return {
    name: string(object.name, `${label}.name`),
    uri: string(object.uri, `${label}.uri`),
  }
}

function parseDevModeNode(value: unknown, label: string): DevModeNodeData {
  const object = exact(
    value,
    label,
    [
      "nodeId",
      "annotations",
      "annotationCategories",
      "documentation",
      "devResources",
    ],
    [
      "description",
      "descriptionMarkdown",
      "ownerNodeId",
      "inheritedFromNodeId",
    ],
  )
  const result: DevModeNodeData = {
    nodeId: identifier(object.nodeId, `${label}.nodeId`),
    annotations: arrayOf(
      object.annotations,
      `${label}.annotations`,
      parseAnnotation,
    ),
    annotationCategories: arrayOf(
      object.annotationCategories,
      `${label}.annotationCategories`,
      parseAnnotationCategory,
    ),
    documentation: arrayOf(
      object.documentation,
      `${label}.documentation`,
      parseDevResource,
    ),
    devResources: arrayOf(
      object.devResources,
      `${label}.devResources`,
      parseDevResource,
    ),
  }
  for (const key of ["description", "descriptionMarkdown"] as const) {
    const parsed = optionalString(object, key)
    if (parsed !== undefined) result[key] = parsed
  }
  const ownerNodeId = optionalString(object, "ownerNodeId", identifier)
  if (ownerNodeId !== undefined) result.ownerNodeId = ownerNodeId
  const inherited = optionalString(object, "inheritedFromNodeId", identifier)
  if (inherited !== undefined) result.inheritedFromNodeId = inherited
  return result
}

function parseDevModeResult(
  value: unknown,
  label: string,
): GetDevModeDataResult {
  const object = exact(
    value,
    label,
    ["items", "visitedNodes", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    {
      items: arrayOf(
        object.items,
        `${label}.items`,
        (item, itemLabel) => parseItemResult(item, itemLabel, parseDevModeNode),
        MAX_INPUT_IDS,
      ),
      visitedNodes: integer(object.visitedNodes, `${label}.visitedNodes`),
    },
    label,
  )
}

function parseReactionAction(value: unknown, label: string): ReactionAction {
  const object = record(value, label)
  switch (object.type) {
    case "navigate":
    case "openOverlay":
    case "swapOverlay":
    case "changeTo":
    case "scrollTo": {
      const action = exact(object, label, ["type"], ["destinationId"])
      const result: ReactionAction = { type: object.type }
      const destinationId = optionalString(action, "destinationId", identifier)
      if (destinationId !== undefined) result.destinationId = destinationId
      return result
    }
    case "closeOverlay":
    case "back":
      exact(object, label, ["type"])
      return { type: object.type }
    case "openLink": {
      const action = exact(object, label, ["type", "uri"])
      return { type: "openLink", uri: string(action.uri, `${label}.uri`) }
    }
    case "setVariable": {
      const action = exact(object, label, ["type"], ["variableId"])
      const result: ReactionAction = { type: "setVariable" }
      const variableId = optionalString(action, "variableId", identifier)
      if (variableId !== undefined) result.variableId = variableId
      return result
    }
    case "setVariableMode": {
      const action = exact(
        object,
        label,
        ["type"],
        ["variableCollectionId", "variableModeId"],
      )
      const result: ReactionAction = { type: "setVariableMode" }
      const collectionId = optionalString(
        action,
        "variableCollectionId",
        identifier,
      )
      const modeId = optionalString(action, "variableModeId", identifier)
      if (collectionId !== undefined) result.variableCollectionId = collectionId
      if (modeId !== undefined) result.variableModeId = modeId
      return result
    }
    case "conditional":
      exact(object, label, ["type"])
      return { type: "conditional" }
    case "updateMediaRuntime": {
      const action = exact(
        object,
        label,
        ["type", "mediaAction"],
        ["destinationId", "amountToSkip", "newTimestamp"],
      )
      const result: ReactionAction = {
        type: "updateMediaRuntime",
        mediaAction: oneOf(
          action.mediaAction,
          [
            "play",
            "pause",
            "togglePlayPause",
            "mute",
            "unmute",
            "toggleMuteUnmute",
            "skipForward",
            "skipBackward",
            "skipTo",
          ],
          `${label}.mediaAction`,
        ),
      }
      const destinationId = optionalString(action, "destinationId", identifier)
      if (destinationId !== undefined) result.destinationId = destinationId
      if (Object.hasOwn(action, "amountToSkip")) {
        result.amountToSkip = finite(
          action.amountToSkip,
          `${label}.amountToSkip`,
        )
      }
      if (Object.hasOwn(action, "newTimestamp")) {
        result.newTimestamp = finite(
          action.newTimestamp,
          `${label}.newTimestamp`,
        )
      }
      return result
    }
    default:
      return fail(`${label}.type is not allowed`)
  }
}

function parseOverlayBackground(
  value: unknown,
  label: string,
): OverlayBackground {
  const object = record(value, label)
  switch (object.type) {
    case "none":
      exact(object, label, ["type"])
      return { type: "none" }
    case "solidColor": {
      const background = exact(object, label, ["type", "color"])
      return {
        type: "solidColor",
        color: parseColor(background.color, `${label}.color`),
      }
    }
    default:
      return fail(`${label}.type is not allowed`)
  }
}

function parseReactionOverlay(value: unknown, label: string): ReactionOverlay {
  const object = exact(
    value,
    label,
    [],
    ["relativePosition", "positionType", "background", "backgroundInteraction"],
  )
  const result: ReactionOverlay = {}
  if (Object.hasOwn(object, "relativePosition")) {
    const point = exact(object.relativePosition, `${label}.relativePosition`, [
      "x",
      "y",
    ])
    result.relativePosition = {
      x: finite(point.x, `${label}.relativePosition.x`),
      y: finite(point.y, `${label}.relativePosition.y`),
    }
  }
  if (Object.hasOwn(object, "positionType")) {
    result.positionType = oneOf<OverlayPositionType>(
      object.positionType,
      [
        "center",
        "topLeft",
        "topCenter",
        "topRight",
        "bottomLeft",
        "bottomCenter",
        "bottomRight",
        "manual",
      ],
      `${label}.positionType`,
    )
  }
  if (Object.hasOwn(object, "background")) {
    result.background = parseOverlayBackground(
      object.background,
      `${label}.background`,
    )
  }
  if (Object.hasOwn(object, "backgroundInteraction")) {
    result.backgroundInteraction = oneOf<OverlayBackgroundInteraction>(
      object.backgroundInteraction,
      ["none", "closeOnClickOutside"],
      `${label}.backgroundInteraction`,
    )
  }
  return result
}

function parseReaction(value: unknown, label: string): Reaction {
  const object = exact(
    value,
    label,
    ["trigger", "action", "destinationAccessible"],
    [
      "transitionId",
      "transitionDuration",
      "overlay",
      "timeout",
      "delay",
      "keyCodes",
      "device",
      "mediaHitTime",
    ],
  )
  const result: Reaction = {
    trigger: oneOf<ReactionTrigger>(
      object.trigger,
      [
        "click",
        "drag",
        "hover",
        "press",
        "keyDown",
        "afterDelay",
        "mouseEnter",
        "mouseLeave",
        "mouseUp",
        "mouseDown",
        "mediaHit",
        "mediaEnd",
      ],
      `${label}.trigger`,
    ),
    action: parseReactionAction(object.action, `${label}.action`),
    destinationAccessible: boolean(
      object.destinationAccessible,
      `${label}.destinationAccessible`,
    ),
  }
  const transitionId = optionalString(object, "transitionId")
  if (transitionId !== undefined) result.transitionId = transitionId
  if (Object.hasOwn(object, "transitionDuration")) {
    result.transitionDuration = finite(
      object.transitionDuration,
      `${label}.transitionDuration`,
    )
  }
  if (Object.hasOwn(object, "overlay")) {
    result.overlay = parseReactionOverlay(object.overlay, `${label}.overlay`)
  }
  if (Object.hasOwn(object, "timeout")) {
    result.timeout = finite(object.timeout, `${label}.timeout`)
  }
  if (Object.hasOwn(object, "delay")) {
    result.delay = finite(object.delay, `${label}.delay`)
  }
  if (Object.hasOwn(object, "keyCodes")) {
    result.keyCodes = arrayOf(
      object.keyCodes,
      `${label}.keyCodes`,
      (code, codeLabel) => finite(code, codeLabel),
    )
  }
  if (Object.hasOwn(object, "device")) {
    result.device = string(object.device, `${label}.device`)
  }
  if (Object.hasOwn(object, "mediaHitTime")) {
    result.mediaHitTime = finite(object.mediaHitTime, `${label}.mediaHitTime`)
  }
  return result
}

function parseNodeReactions(value: unknown, label: string): NodeReactions {
  const object = exact(value, label, ["nodeId", "reactions"])
  return {
    nodeId: identifier(object.nodeId, `${label}.nodeId`),
    reactions: arrayOf(object.reactions, `${label}.reactions`, parseReaction),
  }
}

function parseReactionsResult(
  value: unknown,
  label: string,
): GetReactionsResult {
  const object = exact(
    value,
    label,
    ["items", "visitedNodes", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    {
      items: arrayOf(
        object.items,
        `${label}.items`,
        (item, itemLabel) =>
          parseItemResult(item, itemLabel, parseNodeReactions),
        MAX_INPUT_IDS,
      ),
      visitedNodes: integer(object.visitedNodes, `${label}.visitedNodes`),
    },
    label,
  )
}

const MOTION_EASING_TYPES: readonly MotionEasingType[] = [
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
]

function parseCubicBezier(value: unknown, label: string): CubicBezier {
  const object = exact(value, label, ["x1", "y1", "x2", "y2"])
  return {
    x1: finite(object.x1, `${label}.x1`),
    y1: finite(object.y1, `${label}.y1`),
    x2: finite(object.x2, `${label}.x2`),
    y2: finite(object.y2, `${label}.y2`),
  }
}

function parseMotionEasing(value: unknown, label: string): MotionEasing {
  const object = record(value, label)
  const easingType = oneOf(object.type, MOTION_EASING_TYPES, `${label}.type`)
  if (easingType === "VARIABLE_ALIAS") {
    const alias = exact(object, label, ["type", "id"])
    return { type: "VARIABLE_ALIAS", id: string(alias.id, `${label}.id`) }
  }
  const easing = exact(
    object,
    label,
    ["type"],
    ["easingFunctionCubicBezier", "easingFunctionSpring"],
  )
  const result: MotionEasing = { type: easingType }
  if (Object.hasOwn(easing, "easingFunctionCubicBezier")) {
    result.easingFunctionCubicBezier = parseCubicBezier(
      easing.easingFunctionCubicBezier,
      `${label}.easingFunctionCubicBezier`,
    )
  }
  if (Object.hasOwn(easing, "easingFunctionSpring")) {
    const spring = exact(
      easing.easingFunctionSpring,
      `${label}.easingFunctionSpring`,
      ["bounce"],
    )
    result.easingFunctionSpring = {
      bounce: finite(spring.bounce, `${label}.easingFunctionSpring.bounce`),
    }
  }
  return result
}

function parseMotionKeyframeValue(
  value: unknown,
  label: string,
): MotionKeyframeValue {
  const object = record(value, label)
  switch (object.type) {
    case "FLOAT": {
      const parsed = exact(object, label, ["type", "value"])
      return { type: "FLOAT", value: finite(parsed.value, `${label}.value`) }
    }
    case "COLOR": {
      const parsed = exact(object, label, ["type", "value"])
      return {
        type: "COLOR",
        value: parseColor(parsed.value, `${label}.value`),
      }
    }
    case "TEXT_DATA": {
      const parsed = exact(object, label, ["type", "value"])
      return {
        type: "TEXT_DATA",
        value: string(parsed.value, `${label}.value`),
      }
    }
    case "VECTOR": {
      const parsed = exact(object, label, ["type", "value"])
      const point = exact(parsed.value, `${label}.value`, ["x", "y"])
      return {
        type: "VECTOR",
        value: {
          x: finite(point.x, `${label}.value.x`),
          y: finite(point.y, `${label}.value.y`),
        },
      }
    }
    case "BOOL": {
      const parsed = exact(object, label, ["type", "value"])
      return { type: "BOOL", value: boolean(parsed.value, `${label}.value`) }
    }
    case "CIRCLE": {
      const parsed = exact(object, label, ["type", "value"])
      const circle = exact(parsed.value, `${label}.value`, ["x", "y", "radius"])
      return {
        type: "CIRCLE",
        value: {
          x: finite(circle.x, `${label}.value.x`),
          y: finite(circle.y, `${label}.value.y`),
          radius: finite(circle.radius, `${label}.value.radius`),
        },
      }
    }
    case "LINE": {
      const parsed = exact(object, label, ["type", "value"])
      const line = exact(parsed.value, `${label}.value`, ["x", "y", "x2", "y2"])
      return {
        type: "LINE",
        value: {
          x: finite(line.x, `${label}.value.x`),
          y: finite(line.y, `${label}.value.y`),
          x2: finite(line.x2, `${label}.value.x2`),
          y2: finite(line.y2, `${label}.value.y2`),
        },
      }
    }
    case "CIRCLE_POINT": {
      const parsed = exact(object, label, ["type", "value"])
      const point = exact(parsed.value, `${label}.value`, [
        "x",
        "y",
        "radius",
        "angle",
      ])
      return {
        type: "CIRCLE_POINT",
        value: {
          x: finite(point.x, `${label}.value.x`),
          y: finite(point.y, `${label}.value.y`),
          radius: finite(point.radius, `${label}.value.radius`),
          angle: finite(point.angle, `${label}.value.angle`),
        },
      }
    }
    case "COLOR_POINT": {
      const parsed = exact(object, label, ["type", "value"])
      const point = exact(parsed.value, `${label}.value`, ["x", "y", "color"])
      return {
        type: "COLOR_POINT",
        value: {
          x: finite(point.x, `${label}.value.x`),
          y: finite(point.y, `${label}.value.y`),
          color: parseColor(point.color, `${label}.value.color`),
        },
      }
    }
    case "unsupported": {
      const parsed = exact(object, label, ["type", "tag"])
      return { type: "unsupported", tag: string(parsed.tag, `${label}.tag`) }
    }
    default:
      return fail(`${label}.type is not allowed`)
  }
}

function parseMotionKeyframe(value: unknown, label: string): MotionKeyframe {
  const object = exact(value, label, [
    "id",
    "timelinePosition",
    "value",
    "easing",
  ])
  return {
    id: string(object.id, `${label}.id`),
    timelinePosition: finite(
      object.timelinePosition,
      `${label}.timelinePosition`,
    ),
    value: parseMotionKeyframeValue(object.value, `${label}.value`),
    easing: parseMotionEasing(object.easing, `${label}.easing`),
  }
}

function parseKeyframeField(value: unknown, label: string): KeyframeField {
  const object = record(value, label)
  switch (object.type) {
    case "property": {
      const field = exact(object, label, ["type", "name"])
      return { type: "property", name: string(field.name, `${label}.name`) }
    }
    case "indexedItem": {
      const field = exact(
        object,
        label,
        ["type", "collection", "index"],
        ["field", "propertyId"],
      )
      const result: KeyframeField = {
        type: "indexedItem",
        collection: oneOf(
          field.collection,
          ["fills", "strokes", "effects"],
          `${label}.collection`,
        ),
        index: integer(field.index, `${label}.index`),
      }
      const named = optionalString(field, "field")
      if (named !== undefined) result.field = named
      const propertyId = optionalString(field, "propertyId")
      if (propertyId !== undefined) result.propertyId = propertyId
      return result
    }
    default:
      return fail(`${label}.type is not allowed`)
  }
}

function parseAnimationTrack(value: unknown, label: string): AnimationTrack {
  const object = exact(value, label, ["id", "keyframeOperation", "keyframes"])
  return {
    id: string(object.id, `${label}.id`),
    keyframeOperation: oneOf<KeyframeOperation>(
      object.keyframeOperation,
      ["SET", "OFFSET", "SCALE"],
      `${label}.keyframeOperation`,
    ),
    keyframes: arrayOf(
      object.keyframes,
      `${label}.keyframes`,
      parseMotionKeyframe,
    ),
  }
}

function parseAppliedPropValue(
  value: unknown,
  label: string,
): AppliedStylePropValue {
  if (typeof value === "string") return value
  if (typeof value === "boolean") return value
  if (typeof value === "number" && Number.isFinite(value)) return value
  return parseMotionEasing(value, label)
}

function parseAppliedProp(value: unknown, label: string): AppliedStyleProp {
  const object = exact(value, label, ["name", "value"])
  return {
    name: string(object.name, `${label}.name`),
    value: parseAppliedPropValue(object.value, `${label}.value`),
  }
}

function parseAvailableProp(value: unknown, label: string): AvailableStyleProp {
  const object = exact(value, label, ["name", "value"])
  return {
    name: string(object.name, `${label}.name`),
    value: string(object.value, `${label}.value`),
  }
}

function parseAppliedStyle(
  value: unknown,
  label: string,
): AppliedAnimationStyle {
  const object = exact(
    value,
    label,
    ["id", "styleId", "name"],
    ["duration", "timelineOffset", "props"],
  )
  const result: AppliedAnimationStyle = {
    id: string(object.id, `${label}.id`),
    styleId: string(object.styleId, `${label}.styleId`),
    name: string(object.name, `${label}.name`),
  }
  if (Object.hasOwn(object, "duration")) {
    result.duration = finite(object.duration, `${label}.duration`)
  }
  if (Object.hasOwn(object, "timelineOffset")) {
    result.timelineOffset = finite(
      object.timelineOffset,
      `${label}.timelineOffset`,
    )
  }
  if (Object.hasOwn(object, "props")) {
    result.props = arrayOf(object.props, `${label}.props`, parseAppliedProp)
  }
  return result
}

function parseAvailableStyle(
  value: unknown,
  label: string,
): AvailableAnimationStyle {
  const object = exact(
    value,
    label,
    ["styleId", "name"],
    ["description", "props"],
  )
  const result: AvailableAnimationStyle = {
    styleId: string(object.styleId, `${label}.styleId`),
    name: string(object.name, `${label}.name`),
  }
  const description = optionalString(object, "description")
  if (description !== undefined) result.description = description
  if (Object.hasOwn(object, "props")) {
    result.props = arrayOf(object.props, `${label}.props`, parseAvailableProp)
  }
  return result
}

function parseAnimationBinding(
  value: unknown,
  label: string,
): AnimationBinding {
  const object = exact(value, label, [
    "field",
    "baseValue",
    "timelineDuration",
    "tracks",
  ])
  return {
    field: parseKeyframeField(object.field, `${label}.field`),
    baseValue: parseMotionKeyframeValue(object.baseValue, `${label}.baseValue`),
    timelineDuration: finite(
      object.timelineDuration,
      `${label}.timelineDuration`,
    ),
    tracks: arrayOf(object.tracks, `${label}.tracks`, parseAnimationTrack),
  }
}

function parseManualTrack(value: unknown, label: string): ManualTrackBinding {
  const object = exact(value, label, ["field", "id", "baseValue", "keyframes"])
  return {
    field: parseKeyframeField(object.field, `${label}.field`),
    id: string(object.id, `${label}.id`),
    baseValue: parseMotionKeyframeValue(object.baseValue, `${label}.baseValue`),
    keyframes: arrayOf(
      object.keyframes,
      `${label}.keyframes`,
      parseMotionKeyframe,
    ),
  }
}

function parseTimeline(value: unknown, label: string): MotionTimeline {
  const object = exact(value, label, ["id", "duration"])
  return {
    id: string(object.id, `${label}.id`),
    duration: finite(object.duration, `${label}.duration`),
  }
}

function parseNodeMotion(value: unknown, label: string): NodeMotion {
  const object = exact(value, label, [
    "nodeId",
    "animationStyles",
    "animations",
    "manualKeyframeTracks",
    "timelines",
  ])
  return {
    nodeId: identifier(object.nodeId, `${label}.nodeId`),
    animationStyles: arrayOf(
      object.animationStyles,
      `${label}.animationStyles`,
      parseAppliedStyle,
    ),
    animations: arrayOf(
      object.animations,
      `${label}.animations`,
      parseAnimationBinding,
    ),
    manualKeyframeTracks: arrayOf(
      object.manualKeyframeTracks,
      `${label}.manualKeyframeTracks`,
      parseManualTrack,
    ),
    timelines: arrayOf(object.timelines, `${label}.timelines`, parseTimeline),
  }
}

function parseMotionResult(value: unknown, label: string): GetMotionResult {
  const object = exact(
    value,
    label,
    ["items", "visitedNodes", "truncated", "observation"],
    ["availableStyles", "truncation"],
  )
  const result = withResultMetadata(
    object,
    {
      items: arrayOf(
        object.items,
        `${label}.items`,
        (item, itemLabel) => parseItemResult(item, itemLabel, parseNodeMotion),
        MAX_INPUT_IDS,
      ),
      visitedNodes: integer(object.visitedNodes, `${label}.visitedNodes`),
    },
    label,
  )
  if (Object.hasOwn(object, "availableStyles")) {
    const styles = arrayOf(
      object.availableStyles,
      `${label}.availableStyles`,
      parseAvailableStyle,
    )
    if (styles.length > 0) return { ...result, availableStyles: styles }
  }
  return result
}

function parseScreenshotAsset(value: unknown, label: string): ScreenshotAsset {
  const object = record(value, label)
  switch (object.format) {
    case "png":
    case "jpeg": {
      const asset = exact(object, label, [
        "format",
        "nodeId",
        "dataBase64",
        "width",
        "height",
      ])
      const width = parseU32(asset.width, `${label}.width`)
      const height = parseU32(asset.height, `${label}.height`)
      if (width > MAX_RASTER_SIDE || height > MAX_RASTER_SIDE) {
        return fail(`${label} exceeds raster side limit ${MAX_RASTER_SIDE}`)
      }
      if (width * height > MAX_RASTER_PIXELS) {
        return fail(`${label} exceeds raster pixel limit ${MAX_RASTER_PIXELS}`)
      }
      return {
        format: object.format,
        nodeId: identifier(asset.nodeId, `${label}.nodeId`),
        dataBase64: boundedString(
          asset.dataBase64,
          `${label}.dataBase64`,
          MAX_RASTER_BASE64_BYTES,
          true,
        ),
        width,
        height,
      }
    }
    case "svg": {
      const asset = exact(object, label, ["format", "nodeId", "source"])
      return {
        format: "svg",
        nodeId: identifier(asset.nodeId, `${label}.nodeId`),
        source: boundedString(
          asset.source,
          `${label}.source`,
          MAX_SVG_BYTES,
          true,
        ),
      }
    }
    default:
      return fail(`${label}.format is not allowed`)
  }
}

function parseScreenshotResult(
  value: unknown,
  label: string,
): GetScreenshotResult {
  const object = exact(
    value,
    label,
    ["assets", "truncated", "observation"],
    ["truncation"],
  )
  return withResultMetadata(
    object,
    {
      assets: arrayOf(
        object.assets,
        `${label}.assets`,
        (item, itemLabel) =>
          parseItemResult(item, itemLabel, parseScreenshotAsset),
        MAX_INPUT_IDS,
      ),
    },
    label,
  )
}

export function parseReadResult(value: unknown): ReadResult {
  const object = exact(value, "read result", ["operation", "result"])
  switch (object.operation) {
    case "get_metadata":
      return {
        operation: "get_metadata",
        result: parseMetadataResult(object.result, "get_metadata result"),
      }
    case "get_selection":
      return {
        operation: "get_selection",
        result: parseSelectionResult(object.result, "get_selection result"),
      }
    case "get_nodes":
      return {
        operation: "get_nodes",
        result: parseNodesResult(object.result, "get_nodes result"),
      }
    case "search_nodes":
      return {
        operation: "search_nodes",
        result: parseSearchResult(object.result, "search_nodes result"),
      }
    case "get_design_context":
      return {
        operation: "get_design_context",
        result: parseDesignContextResult(
          object.result,
          "get_design_context result",
        ),
      }
    case "get_styles":
      return {
        operation: "get_styles",
        result: parseStylesResult(object.result, "get_styles result"),
      }
    case "get_variables":
      return {
        operation: "get_variables",
        result: parseVariablesResult(object.result, "get_variables result"),
      }
    case "get_components":
      return {
        operation: "get_components",
        result: parseComponentsResult(object.result, "get_components result"),
      }
    case "get_fonts":
      return {
        operation: "get_fonts",
        result: parseFontsResult(object.result, "get_fonts result"),
      }
    case "get_dev_mode_data":
      return {
        operation: "get_dev_mode_data",
        result: parseDevModeResult(object.result, "get_dev_mode_data result"),
      }
    case "get_reactions":
      return {
        operation: "get_reactions",
        result: parseReactionsResult(object.result, "get_reactions result"),
      }
    case "get_motion":
      return {
        operation: "get_motion",
        result: parseMotionResult(object.result, "get_motion result"),
      }
    case "get_screenshot":
      return {
        operation: "get_screenshot",
        result: parseScreenshotResult(object.result, "get_screenshot result"),
      }
    default:
      return fail("unknown read result operation")
  }
}
