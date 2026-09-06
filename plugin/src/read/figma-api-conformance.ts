/// Type-only conformance between the host properties this server reads and the
/// Figma plugin API as `@figma/plugin-typings` declares it.
///
/// The read path deliberately treats every host value as `unknown` and reaches
/// properties through `hostGet`, because `documentAccess: dynamic-page` lets a
/// getter throw and because a narrow hand-written `FigmaReadApi` is what keeps
/// the write surface unreachable by type. The cost of that choice is that a
/// misspelled property name, or one Figma renames, reads as `undefined` at
/// runtime and compiles clean — the field simply goes missing from a result
/// and nothing says so.
///
/// This file buys the missing half back without giving up any of it. Each entry
/// below asserts only that the name is a real key of some Figma type. It does
/// not claim the property belongs to the node the caller happens to hold; that
/// would mean narrowing 62 call sites by node type, and is not what this file
/// is for.
///
/// Two failures it catches, both of which have no other detector today:
///
///   - a typo or an invented property, at compile time rather than as a
///     silently absent field in a live read;
///   - Figma removing or renaming something this server depends on — bump the
///     pinned `@figma/plugin-typings` version and `tsc` reports exactly which
///     properties no longer exist.
///
/// The list is every distinct name passed to `hostGet` across `src/read`. When
/// a read starts reaching for a new property, add it here in the same commit.
/// Nothing imports this module: it erases entirely at build time and exists to
/// be type-checked.

/// `keyof` a union yields only the keys every member shares, which is the wrong
/// question — `characters` lives on `TextNode` alone and would vanish. This
/// distributes first, so the result is every key of every member.
type KeyOfUnion<T> = T extends unknown ? keyof T : never

/// Everything this server reads from, node and non-node alike. The non-node
/// members each earn their place: `Constraints` carries `horizontal` and
/// `vertical`, styles carry `description`, and `ComponentProperties[string]`
/// carries the `value` read off an instance's properties at
/// `serialize.ts:723` — a shape that belongs to no node type, which is why
/// asserting against nodes alone reported it missing.
type FigmaSurface =
  | KeyOfUnion<SceneNode>
  | keyof PageNode
  | keyof DocumentNode
  | keyof Constraints
  | keyof Variable
  | keyof VariableAlias
  | keyof StyledTextSegment
  | KeyOfUnion<BaseStyle>
  | keyof ComponentProperties[string]

type Assert<T extends true> = T
type IsFigmaProperty<K extends string> = K extends FigmaSurface ? true : false

/// One entry per name reached through `hostGet`. A failure names the offending
/// line, so the error points at the property rather than at the file.
export type PropertiesThisServerReads = [
  Assert<IsFigmaProperty<"absoluteBoundingBox">>,
  Assert<IsFigmaProperty<"absoluteRenderBounds">>,
  Assert<IsFigmaProperty<"absoluteTransform">>,
  Assert<IsFigmaProperty<"bottomLeftRadius">>,
  Assert<IsFigmaProperty<"bottomRightRadius">>,
  Assert<IsFigmaProperty<"boundVariables">>,
  Assert<IsFigmaProperty<"characters">>,
  Assert<IsFigmaProperty<"children">>,
  Assert<IsFigmaProperty<"clipsContent">>,
  Assert<IsFigmaProperty<"componentProperties">>,
  Assert<IsFigmaProperty<"componentPropertyDefinitions">>,
  Assert<IsFigmaProperty<"constraints">>,
  Assert<IsFigmaProperty<"cornerRadius">>,
  Assert<IsFigmaProperty<"cornerSmoothing">>,
  Assert<IsFigmaProperty<"counterAxisSizingMode">>,
  Assert<IsFigmaProperty<"counterAxisSpacing">>,
  Assert<IsFigmaProperty<"dashPattern">>,
  Assert<IsFigmaProperty<"description">>,
  Assert<IsFigmaProperty<"documentationLinks">>,
  Assert<IsFigmaProperty<"effects">>,
  Assert<IsFigmaProperty<"fills">>,
  Assert<IsFigmaProperty<"fontName">>,
  Assert<IsFigmaProperty<"fontSize">>,
  Assert<IsFigmaProperty<"fontWeight">>,
  Assert<IsFigmaProperty<"getMainComponentAsync">>,
  Assert<IsFigmaProperty<"getStyledTextSegments">>,
  Assert<IsFigmaProperty<"horizontal">>,
  Assert<IsFigmaProperty<"itemSpacing">>,
  Assert<IsFigmaProperty<"layoutMode">>,
  Assert<IsFigmaProperty<"layoutWrap">>,
  Assert<IsFigmaProperty<"letterSpacing">>,
  Assert<IsFigmaProperty<"lineHeight">>,
  Assert<IsFigmaProperty<"name">>,
  Assert<IsFigmaProperty<"opacity">>,
  Assert<IsFigmaProperty<"paddingBottom">>,
  Assert<IsFigmaProperty<"paddingLeft">>,
  Assert<IsFigmaProperty<"paddingRight">>,
  Assert<IsFigmaProperty<"paddingTop">>,
  Assert<IsFigmaProperty<"parent">>,
  Assert<IsFigmaProperty<"primaryAxisSizingMode">>,
  Assert<IsFigmaProperty<"rotation">>,
  Assert<IsFigmaProperty<"strokeWeight">>,
  Assert<IsFigmaProperty<"strokes">>,
  Assert<IsFigmaProperty<"topLeftRadius">>,
  Assert<IsFigmaProperty<"topRightRadius">>,
  Assert<IsFigmaProperty<"type">>,
  Assert<IsFigmaProperty<"value">>,
  Assert<IsFigmaProperty<"variantProperties">>,
  Assert<IsFigmaProperty<"vertical">>,
  Assert<IsFigmaProperty<"visible">>,
]
