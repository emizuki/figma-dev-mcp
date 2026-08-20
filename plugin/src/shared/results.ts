import type { CapabilitySet, ErrorCode } from "./protocol"

export type TruncationReason = "depthLimit" | "nodeLimit" | "byteLimit"

export interface Truncation {
  reason: TruncationReason
  appliedDepth?: number
  visitedNodes?: number
  encodedBytes?: number
}

export interface ObservationWindow {
  startedAt: string
  completedAt: string
}

export interface ItemError {
  index: number
  id?: string
  code: ErrorCode
  message: string
  retryable: boolean
}

export interface ToolError {
  code: ErrorCode
  message: string
  retryable: boolean
  items?: ItemError[]
}

export type ItemResult<T> =
  | { status: "success"; value: T }
  | { status: "error"; error: ToolError }

export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

export interface Color {
  r: number
  g: number
  b: number
  a: number
}

export interface GradientStop {
  position: number
  color: Color
}

export type ImageScaleMode = "fill" | "fit" | "crop" | "tile"

export type PaintValue =
  | { type: "solid"; color: Color; opacity: number }
  | { type: "linearGradient"; stops: GradientStop[] }
  | { type: "radialGradient"; stops: GradientStop[] }
  | { type: "image"; imageRef: string; scaleMode: ImageScaleMode }
  | { type: "mixed" }

export type EffectValue =
  | {
      type: "dropShadow" | "innerShadow"
      color: Color
      offsetX: number
      offsetY: number
      radius: number
      spread: number
    }
  | { type: "layerBlur" | "backgroundBlur"; radius: number }

export type StrokeAlign = "inside" | "outside" | "center"

export interface StrokeValue {
  paints: PaintValue[]
  weight?: number
  align?: StrokeAlign
  dashPattern?: number[]
}

export type CornerRadiusValue =
  | { kind: "uniform"; radius: number }
  | {
      kind: "perCorner"
      topLeft: number
      topRight: number
      bottomRight: number
      bottomLeft: number
    }

export type BlendMode =
  | "passThrough"
  | "normal"
  | "darken"
  | "multiply"
  | "linearBurn"
  | "colorBurn"
  | "lighten"
  | "screen"
  | "linearDodge"
  | "colorDodge"
  | "overlay"
  | "softLight"
  | "hardLight"
  | "difference"
  | "exclusion"
  | "hue"
  | "saturation"
  | "color"
  | "luminosity"

export type LayoutMode = "none" | "horizontal" | "vertical" | "grid"
export type LayoutSizing = "fixed" | "hug" | "fill"
export type AxisAlign = "min" | "center" | "max" | "spaceBetween" | "baseline"

export interface LayoutValue {
  mode: LayoutMode
  primarySizing: LayoutSizing
  counterSizing: LayoutSizing
  gap: number
  paddingTop: number
  paddingRight: number
  paddingBottom: number
  paddingLeft: number
  primaryAlign?: AxisAlign
  counterAlign?: AxisAlign
  wrap?: boolean
  counterAxisSpacing?: number
}

export type LineHeightValue =
  | { unit: "pixels"; value: number }
  | { unit: "percent"; value: number }
  | { unit: "auto" }

export type LetterSpacingValue =
  | { unit: "pixels"; value: number }
  | { unit: "percent"; value: number }

// "none" is the Figma default for textDecoration and is never emitted; it stays in
// the union so the schema describes the full domain (absence means "default").
export type TextDecoration = "none" | "underline" | "strikethrough"
// "left" is the Figma default for textAlignHorizontal and is never emitted; it stays
// in the union so the schema describes the full domain (absence means "default").
export type TextAlignHorizontal = "left" | "center" | "right" | "justified"
// "top" is the Figma default for textAlignVertical and is never emitted; it stays in
// the union so the schema describes the full domain (absence means "default").
export type TextAlignVertical = "top" | "center" | "bottom"
// "none" is the Figma default for textAutoResize and is never emitted; it stays in
// the union so the schema describes the full domain (absence means "default").
export type TextAutoResize = "none" | "widthAndHeight" | "height" | "truncate"

export interface TextStyle {
  fontFamily: string
  fontStyle: string
  fontSize?: number
  lineHeight?: LineHeightValue
  letterSpacing?: LetterSpacingValue
  fontWeight?: number
  textDecoration?: TextDecoration
  paints: PaintValue[]
}

export interface StyledTextRange {
  start: number
  end: number
  style: TextStyle
}

export interface TextValue {
  characters: string
  defaultStyle: TextStyle
  styledRanges: StyledTextRange[]
  alignHorizontal?: TextAlignHorizontal
  alignVertical?: TextAlignVertical
  autoResize?: TextAutoResize
}

export interface TextSummary {
  characterCount: number
  preview: string
}

export type ComponentPropertyValue =
  | { kind: "text"; value: string }
  | { kind: "boolean"; value: boolean }
  | { kind: "instanceSwap"; value: string }
  | { kind: "variant"; value: string }

export interface NamedComponentProperty {
  name: string
  value: ComponentPropertyValue
}

export interface ComponentValue {
  componentId: string
  componentSetId?: string
  properties: NamedComponentProperty[]
}

export type VariableValue =
  | { kind: "boolean"; value: boolean }
  | { kind: "float"; value: number }
  | { kind: "string"; value: string }
  | { kind: "color"; value: Color }
  | { kind: "alias"; value: string }

export type StyleKind = "paint" | "stroke" | "text" | "effect" | "grid"

export interface StyleIdentity {
  id: string
  name: string
  description?: string
  remote?: boolean
  key?: string
}

export type StyleValue =
  | ({ styleType: "paint"; paints: PaintValue[] } & StyleIdentity)
  | ({ styleType: "text"; text: TextValue } & StyleIdentity)
  | ({ styleType: "effect"; effects: EffectValue[] } & StyleIdentity)
  | ({
      styleType: "grid"
      pattern: string
      size: number
    } & StyleIdentity)

export interface Transform2D {
  m00: number
  m01: number
  m02: number
  m10: number
  m11: number
  m12: number
}

export interface GeometryValue {
  bounds?: Rect
  rotation: number
  opacity: number
  transform: Transform2D
}

export type ConstraintAxis = "min" | "center" | "max" | "stretch" | "scale"

export interface LayoutConstraints {
  horizontal: ConstraintAxis
  vertical: ConstraintAxis
}

export interface StyleReference {
  id: string
  kind: StyleKind
  name?: string
}

export interface VariableReference {
  id: string
  name?: string
}

export interface InstanceValue {
  componentId: string
  componentSetId?: string
  properties: NamedComponentProperty[]
}

export interface NodeSummary {
  id: string
  name: string
  nodeType: string
  visible: boolean
  parentId?: string
  childIds?: string[]
  bounds?: Rect
}

export interface MinimalNodeDetails {}

export interface CompactNodeData {
  geometry?: GeometryValue
  constraints?: LayoutConstraints
  autoLayout?: LayoutValue
  text?: TextSummary
  component?: ComponentValue
  instance?: InstanceValue
  styleReferences: StyleReference[]
  variableReferences: VariableReference[]
}

export interface FullNodeData {
  geometry?: GeometryValue
  constraints?: LayoutConstraints
  autoLayout?: LayoutValue
  text?: TextValue
  paints: PaintValue[]
  effects: EffectValue[]
  strokes?: StrokeValue
  cornerRadius?: CornerRadiusValue
  cornerSmoothing?: number
  clipsContent?: boolean
  blendMode?: BlendMode
  component?: ComponentValue
  instance?: InstanceValue
  styleReferences: StyleReference[]
  variableReferences: VariableReference[]
}

export interface DesignNode<D> {
  summary: NodeSummary
  data: D
  children: DesignNode<D>[]
  childrenTruncated: boolean
  childrenTruncation?: Truncation
}

export interface FileMetadata {
  key?: string
  name: string
  editorType: string
}

export interface PageSummary {
  id: string
  name: string
}

export interface GetMetadataResult {
  file: FileMetadata
  pages: PageSummary[]
  currentPageId: string
  pluginVersion: string
  capabilities: CapabilitySet
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export type GetSelectionResult =
  | CommonDetailResult<"minimal", "nodes", MinimalNodeDetails>
  | CommonDetailResult<"compact", "nodes", CompactNodeData>
  | CommonDetailResult<"full", "nodes", FullNodeData>

export type GetNodesResult =
  | CommonBatchDetailResult<"minimal", MinimalNodeDetails>
  | CommonBatchDetailResult<"compact", CompactNodeData>
  | CommonBatchDetailResult<"full", FullNodeData>

export type GetDesignContextResult =
  | CommonDetailResult<"minimal", "roots", MinimalNodeDetails>
  | CommonDetailResult<"compact", "roots", CompactNodeData>
  | CommonDetailResult<"full", "roots", FullNodeData>

export type CommonDetailResult<
  Detail extends string,
  Field extends "nodes" | "roots",
  Data,
> = {
  detail: Detail
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
} & { [Key in Field]: DesignNode<Data>[] }

export interface CommonBatchDetailResult<Detail extends string, Data> {
  detail: Detail
  items: ItemResult<DesignNode<Data>>[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export interface NodeMatch {
  node: NodeSummary
  reasons: string[]
}

export interface SearchNodesResult {
  matches: NodeMatch[]
  nextCursor?: string
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export interface GetStylesResult {
  styles: StyleValue[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export interface CodeSyntax {
  platform: string
  code: string
}

export interface VariableModeError {
  code: ErrorCode
  retryable: boolean
}

export interface VariableModeValue {
  modeId: string
  source: VariableValue
  resolved?: VariableValue
  error?: VariableModeError
}

export interface VariableDefinition {
  id: string
  name: string
  collectionId: string
  scopes: string[]
  values: VariableModeValue[]
  codeSyntax: CodeSyntax[]
}

export interface VariableMode {
  id: string
  name: string
}

export interface VariableCollection {
  id: string
  name: string
  modes: VariableMode[]
  variables: VariableDefinition[]
}

export interface GetVariablesResult {
  collections: VariableCollection[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export interface DocumentationReference {
  label?: string
  uri: string
}

export interface ComponentPropertyDefinition {
  name: string
  defaultValue: ComponentPropertyValue
  preferredValues?: ComponentPropertyValue[]
}

export interface NamedVariantProperty {
  name: string
  value: string
}

export interface ComponentDefinition {
  id: string
  name: string
  componentSetId?: string
  description?: string
  documentation: DocumentationReference[]
  variantProperties: NamedVariantProperty[]
  propertyDefinitions: ComponentPropertyDefinition[]
}

export interface InstanceRelationship {
  instanceId: string
  componentId: string
}

export interface GetComponentsResult {
  components: ComponentDefinition[]
  instances: InstanceRelationship[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export interface FontName {
  family: string
  style: string
}

export type FontAvailability = "available" | "unavailable" | "unknown"

export interface FontUsage {
  font: FontName
  availability: FontAvailability
  nodeIds: string[]
}

export interface GetFontsResult {
  fonts: FontUsage[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export interface AnnotationValue {
  id: string
  categoryId?: string
  text: string
}

export interface AnnotationCategory {
  id: string
  label: string
}

export interface DevResource {
  name: string
  uri: string
}

export interface DevModeNodeData {
  nodeId: string
  description?: string
  descriptionMarkdown?: string
  annotations: AnnotationValue[]
  annotationCategories: AnnotationCategory[]
  documentation: DevResource[]
  devResources: DevResource[]
  ownerNodeId?: string
  inheritedFromNodeId?: string
}

export interface GetDevModeDataResult {
  items: ItemResult<DevModeNodeData>[]
  /** Nodes walked, including those that reported nothing and are therefore
   * absent from `items`. Without this a caller cannot tell "scanned and found
   * nothing" from "never reached". */
  visitedNodes: number
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export type ReactionTrigger =
  | "click"
  | "drag"
  | "hover"
  | "press"
  | "keyDown"
  | "afterDelay"
  | "mouseEnter"
  | "mouseLeave"
  | "mouseUp"
  | "mouseDown"
  | "mediaHit"
  | "mediaEnd"

export type MediaRuntimeAction =
  | "play"
  | "pause"
  | "togglePlayPause"
  | "mute"
  | "unmute"
  | "toggleMuteUnmute"
  | "skipForward"
  | "skipBackward"
  | "skipTo"

export type ReactionAction =
  | {
      type: "navigate" | "openOverlay" | "swapOverlay" | "changeTo" | "scrollTo"
      destinationId?: string
    }
  | { type: "closeOverlay" | "back" }
  | { type: "openLink"; uri: string }
  | { type: "setVariable"; variableId?: string }
  | {
      type: "setVariableMode"
      variableCollectionId?: string
      variableModeId?: string
    }
  | { type: "conditional" }
  | {
      type: "updateMediaRuntime"
      mediaAction: MediaRuntimeAction
      destinationId?: string
      amountToSkip?: number
      newTimestamp?: number
    }

export type OverlayPositionType =
  | "center"
  | "topLeft"
  | "topCenter"
  | "topRight"
  | "bottomLeft"
  | "bottomCenter"
  | "bottomRight"
  | "manual"

export type OverlayBackgroundInteraction = "none" | "closeOnClickOutside"

export type OverlayBackground =
  | { type: "none" }
  | { type: "solidColor"; color: Color }

export interface ReactionOverlay {
  relativePosition?: { x: number; y: number }
  positionType?: OverlayPositionType
  background?: OverlayBackground
  backgroundInteraction?: OverlayBackgroundInteraction
}

export interface Reaction {
  trigger: ReactionTrigger
  action: ReactionAction
  transitionId?: string
  /** Host `transition.duration`. Official reactions example: seconds (`0.2`). Not converted. */
  transitionDuration?: number
  destinationAccessible: boolean
  overlay?: ReactionOverlay
  /** Host `AFTER_TIMEOUT.timeout` in seconds (live: UI 800ms → 0.8). Not converted. */
  timeout?: number
  /** Host mouse-trigger `delay`. Same Trigger surface; not live-measured. Not converted. */
  delay?: number
  keyCodes?: number[]
  device?: string
  /** Host `ON_MEDIA_HIT.mediaHitTime` (or docs alias `timestamp`). Official notes: seconds. */
  mediaHitTime?: number
}

export interface NodeReactions {
  nodeId: string
  reactions: Reaction[]
}

export interface GetReactionsResult {
  items: ItemResult<NodeReactions>[]
  /** Nodes walked, including those that reported nothing and are therefore
   * absent from `items`. Without this a caller cannot tell "scanned and found
   * nothing" from "never reached". */
  visitedNodes: number
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export type MotionEasingType =
  | "LINEAR"
  | "EASE_IN"
  | "EASE_OUT"
  | "EASE_IN_AND_OUT"
  | "EASE_IN_BACK"
  | "EASE_OUT_BACK"
  | "EASE_IN_AND_OUT_BACK"
  | "CUSTOM_CUBIC_BEZIER"
  | "GENTLE"
  | "QUICK"
  | "BOUNCY"
  | "SLOW"
  | "CUSTOM_SPRING"
  | "HOLD"
  | "VARIABLE_ALIAS"

export interface CubicBezier {
  x1: number
  y1: number
  x2: number
  y2: number
}

export interface NormalizedSpring {
  bounce: number
}

export type MotionEasing =
  | {
      type: Exclude<MotionEasingType, "VARIABLE_ALIAS">
      easingFunctionCubicBezier?: CubicBezier
      easingFunctionSpring?: NormalizedSpring
    }
  | { type: "VARIABLE_ALIAS"; id: string }

export type MotionKeyframeValue =
  | { type: "FLOAT"; value: number }
  | { type: "COLOR"; value: Color }
  | { type: "TEXT_DATA"; value: string }
  | { type: "VECTOR"; value: { x: number; y: number } }
  | { type: "BOOL"; value: boolean }
  | { type: "CIRCLE"; value: { x: number; y: number; radius: number } }
  | { type: "LINE"; value: { x: number; y: number; x2: number; y2: number } }
  | {
      type: "CIRCLE_POINT"
      value: { x: number; y: number; radius: number; angle: number }
    }
  | { type: "COLOR_POINT"; value: { x: number; y: number; color: Color } }
  | { type: "unsupported"; tag: string }

export type AppliedStylePropValue = string | number | boolean | MotionEasing

export interface AppliedStyleProp {
  name: string
  value: AppliedStylePropValue
}

export interface AvailableStyleProp {
  name: string
  value: string
}

export interface AppliedAnimationStyle {
  id: string
  styleId: string
  name: string
  duration?: number
  timelineOffset?: number
  props?: AppliedStyleProp[]
}

export interface AvailableAnimationStyle {
  styleId: string
  name: string
  description?: string
  props?: AvailableStyleProp[]
}

export type IndexedCollection = "fills" | "strokes" | "effects"

export type KeyframeField =
  | { type: "property"; name: string }
  | {
      type: "indexedItem"
      collection: IndexedCollection
      index: number
      field?: string
      propertyId?: string
    }

export type KeyframeOperation = "SET" | "OFFSET" | "SCALE"

export interface MotionKeyframe {
  id: string
  timelinePosition: number
  value: MotionKeyframeValue
  easing: MotionEasing
}

export interface AnimationTrack {
  id: string
  keyframeOperation: KeyframeOperation
  keyframes: MotionKeyframe[]
}

export interface AnimationBinding {
  field: KeyframeField
  baseValue: MotionKeyframeValue
  timelineDuration: number
  tracks: AnimationTrack[]
}

export interface ManualTrackBinding {
  field: KeyframeField
  id: string
  baseValue: MotionKeyframeValue
  keyframes: MotionKeyframe[]
}

export interface MotionTimeline {
  id: string
  duration: number
}

export interface NodeMotion {
  nodeId: string
  animationStyles: AppliedAnimationStyle[]
  animations: AnimationBinding[]
  manualKeyframeTracks: ManualTrackBinding[]
  timelines: MotionTimeline[]
}

export interface GetMotionResult {
  items: ItemResult<NodeMotion>[]
  /** Nodes walked, including those that reported nothing and are therefore
   * absent from `items`. Without this a caller cannot tell "scanned and found
   * nothing" from "never reached". */
  visitedNodes: number
  availableStyles?: AvailableAnimationStyle[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}

export type ScreenshotAsset =
  | {
      format: "png" | "jpeg"
      nodeId: string
      dataBase64: string
      width: number
      height: number
    }
  | { format: "svg"; nodeId: string; source: string }

export interface GetScreenshotResult {
  assets: ItemResult<ScreenshotAsset>[]
  truncated: boolean
  truncation?: Truncation
  observation: ObservationWindow
}
