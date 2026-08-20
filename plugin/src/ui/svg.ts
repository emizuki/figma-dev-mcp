import {
  MAX_IDENTIFIER_BYTES,
  MAX_RASTER_DECODED_BYTES,
  MAX_SVG_BYTES,
} from "../shared/limits"
import type { SvgRejection, SvgRejectionKind } from "../shared/results"
import { tokenizeCss } from "./css-syntax"
import {
  decodeBase64,
  validateEmbeddedImageData,
  type EmbeddedImageMime,
} from "./raster"

interface NamedNode {
  readonly name: string
}

interface NamedNodeMapLike {
  readonly length: number
  item(index: number): NamedNode | null
}

interface NodeListLike {
  readonly length: number
  item?(index: number): SvgNode | null
  readonly [index: number]: SvgNode | undefined
}

export interface SvgNode {
  readonly nodeType: number
  readonly childNodes: NodeListLike
  readonly data?: string
  readonly nodeValue?: string | null
}

export interface SvgElement extends SvgNode {
  readonly localName?: string | null
  readonly tagName: string
  readonly textContent?: string | null
  readonly attributes: NamedNodeMapLike
  getAttributeNames?(): string[]
  getAttribute(name: string): string | null
}

export interface SvgDocument extends SvgNode {
  readonly documentElement: SvgElement | null
  getElementsByTagName?(name: string): { readonly length: number }
}

export interface InjectedDomParser {
  parseFromString(source: string, type: string): SvgDocument
}

/** Safety declares a verdict; it does not withhold the source. An SVG that
 * trips a rule still comes back with its source attached, carrying the rule
 * that fired, and the caller decides what the verdict is worth. The consumer is
 * an agent reading JSON, not a renderer, so withholding only destroyed
 * information it could have used.
 *
 * A failure is left only where there is no source to return at all: a transfer
 * that never decoded to a string, and a document over the byte ceiling. Neither
 * is a safety finding, so neither carries a verdict.
 *
 * `safe` and `rejection` are declared absent on the failure arm, and `rejection`
 * on the safe arm, so a caller can read either off the union without narrowing
 * first. */
export type SvgValidation =
  | { ok: true; source: string; safe: true; rejection?: undefined }
  | { ok: true; source: string; safe: false; rejection: SvgRejection }
  | {
      ok: false
      code: "LIMIT_EXCEEDED" | "INTERNAL_ERROR"
      source?: undefined
      safe?: undefined
      rejection?: undefined
    }

const ELEMENT_NODE = 1
const PROCESSING_INSTRUCTION_NODE = 7

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

/** Names are structural (element and attribute local names), so they carry no
 * design content; values are never reported. A name that does not fit the
 * identifier bound is dropped rather than truncated, because a truncated name
 * is a name that reads as a different, real one. */
function rejected(kind: SvgRejectionKind, name?: string): SvgRejection {
  if (
    name === undefined ||
    name.length === 0 ||
    utf8ByteLength(name) > MAX_IDENTIFIER_BYTES
  ) {
    return { kind }
  }
  return { kind, name }
}

function unsafe(source: string, rejection: SvgRejection): SvgValidation {
  return { ok: true, source, safe: false, rejection }
}

function hasLoneSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code >= 0xd800 && code <= 0xdbff) {
      // A high surrogate in the final position has no `charCodeAt` to compare
      // against — the comparisons below are false for NaN — so it is checked
      // for explicitly. It matters more now that safety returns the source:
      // an unpaired surrogate that reached the wire would be escaped as
      // `\ud800` by JSON.stringify and refused by the Rust decoder, dropping
      // the session rather than the asset.
      if (index + 1 >= value.length) return true
      const next = value.charCodeAt(index + 1)
      if (next < 0xdc00 || next > 0xdfff) return true
      index += 1
      continue
    }
    if (code >= 0xdc00 && code <= 0xdfff) return true
  }
  return false
}

function decodeTransfer(input: string | Uint8Array): string | null {
  if (typeof input !== "string") {
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(input)
    } catch {
      return null
    }
  }
  if (hasLoneSurrogate(input)) return null
  return input
}

function isAsciiWhitespace(code: number): boolean {
  return (
    code === 0x09 ||
    code === 0x0a ||
    code === 0x0c ||
    code === 0x0d ||
    code === 0x20
  )
}

function trimAscii(value: string): string {
  let start = 0
  let end = value.length
  while (start < end && isAsciiWhitespace(value.charCodeAt(start))) start += 1
  while (end > start && isAsciiWhitespace(value.charCodeAt(end - 1))) end -= 1
  return value.slice(start, end)
}

function startsWithIgnoreCase(value: string, prefix: string): boolean {
  if (value.length < prefix.length) return false
  for (let index = 0; index < prefix.length; index += 1) {
    const left = value.charCodeAt(index)
    const right = prefix.charCodeAt(index)
    const foldedLeft = left >= 0x41 && left <= 0x5a ? left + 32 : left
    const foldedRight = right >= 0x41 && right <= 0x5a ? right + 32 : right
    if (foldedLeft !== foldedRight) return false
  }
  return true
}

function isHexChar(code: number): boolean {
  return (
    (code >= 0x30 && code <= 0x39) ||
    (code >= 0x41 && code <= 0x46) ||
    (code >= 0x61 && code <= 0x66)
  )
}

function decodePercentBytes(payload: string): Uint8Array {
  const bytes: number[] = []
  for (let index = 0; index < payload.length; index += 1) {
    const code = payload.charCodeAt(index)
    if (
      code === 0x25 &&
      index + 2 < payload.length &&
      isHexChar(payload.charCodeAt(index + 1)) &&
      isHexChar(payload.charCodeAt(index + 2))
    ) {
      bytes.push(Number.parseInt(payload[index + 1]! + payload[index + 2]!, 16))
      index += 2
      continue
    }
    bytes.push(code & 0xff)
  }
  return Uint8Array.from(bytes)
}

function indexOfChar(value: string, needle: number, start = 0): number {
  for (let index = start; index < value.length; index += 1) {
    if (value.charCodeAt(index) === needle) return index
  }
  return -1
}

function splitMeta(meta: string): string[] {
  const parts: string[] = []
  let start = 0
  for (let index = 0; index <= meta.length; index += 1) {
    if (index === meta.length || meta.charCodeAt(index) === 0x3b) {
      parts.push(meta.slice(start, index))
      start = index + 1
    }
  }
  return parts
}

function parseDataUrl(
  value: string,
): { mime: string; bytes: Uint8Array } | null {
  if (!startsWithIgnoreCase(value, "data:")) return null
  const comma = indexOfChar(value, 0x2c, 5)
  if (comma < 0) return null
  const meta = value.slice(5, comma)
  const payload = value.slice(comma + 1)
  const parts = splitMeta(meta)
  const mimePart = trimAscii(parts[0] ?? "")
  const mime = mimePart.length === 0 ? "text/plain" : mimePart.toLowerCase()
  let base64 = false
  for (let index = 1; index < parts.length; index += 1) {
    if (trimAscii(parts[index] ?? "").toLowerCase() === "base64") base64 = true
  }
  const bytes = base64 ? decodeBase64(payload) : decodePercentBytes(payload)
  if (bytes === null) return null
  return { mime, bytes }
}

function isFragmentReference(value: string): boolean {
  return value.length > 0 && value.charCodeAt(0) === 0x23
}

/** The WHATWG URL parser removes ASCII tab, line feed and carriage return from
 * anywhere in a URL before parsing, so a browser sees "javascript:alert(1)"
 * where a prefix test on the raw value sees an ordinary string. Strip exactly
 * those three, and only for detection — nothing about the returned source
 * changes. NOT the space character: WHATWG keeps it, so "jav ascript:" really
 * is not a scheme, and treating it as one would invent a false positive. */
function stripUrlWhitespace(value: string): string {
  let out = ""
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code === 0x09 || code === 0x0a || code === 0x0d) continue
    out += value.charAt(index)
  }
  return out
}

function isJavascriptUrl(value: string): boolean {
  return startsWithIgnoreCase(
    stripUrlWhitespace(trimAscii(value)),
    "javascript:",
  )
}

function allowedDataMime(mime: string): EmbeddedImageMime | null {
  if (mime === "image/png") return "image/png"
  if (mime === "image/jpeg" || mime === "image/jpg") return "image/jpeg"
  if (mime === "image/webp") return "image/webp"
  return null
}

// Figma embeds fonts as data URLs when text is not outlined. A data: font URL
// fetches nothing, so rejecting it was a false positive that made
// svgOutlineText:false unusable. Only font media types are added; every other
// scheme and media type keeps failing, which the rejection tests assert.
function isFontDataMime(mime: string): boolean {
  return (
    mime === "font/woff" ||
    mime === "font/woff2" ||
    mime === "font/ttf" ||
    mime === "font/otf" ||
    mime === "application/font-woff" ||
    mime === "application/x-font-ttf" ||
    mime === "application/x-font-opentype"
  )
}

// Exported so the byte-ceiling-vs-font-mime ordering can be pinned directly:
// MAX_SVG_BYTES (whole-document cap) is smaller than MAX_RASTER_DECODED_BYTES
// (per-data-url decoded cap), so no data: URL embedded in an SVG that passes
// validateSvgSource's document-size gate can ever carry decoded bytes large
// enough to exercise this function's own ceiling check — a black-box test
// through validateSvgSource cannot discriminate the check order below.
export function validateDataUrl(value: string): boolean {
  const parsed = parseDataUrl(value)
  if (parsed === null) return false
  if (parsed.bytes.byteLength > MAX_RASTER_DECODED_BYTES) return false
  if (isFontDataMime(parsed.mime)) return true
  const mime = allowedDataMime(parsed.mime)
  if (mime === null) return false
  return validateEmbeddedImageData(parsed.bytes, mime)
}

function validateReference(value: string): boolean {
  const trimmed = trimAscii(value)
  if (trimmed.length === 0) return false
  if (isFragmentReference(trimmed)) return true
  if (isJavascriptUrl(trimmed)) return false
  if (startsWithIgnoreCase(trimmed, "data:")) return validateDataUrl(trimmed)
  return false
}

function isAlpha(code: number): boolean {
  return (code >= 0x41 && code <= 0x5a) || (code >= 0x61 && code <= 0x7a)
}

function hasUriScheme(value: string): boolean {
  if (value.length < 2 || !isAlpha(value.charCodeAt(0))) return false
  for (let index = 1; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code === 0x3a) return true
    if (
      !isAlpha(code) &&
      !isHexChar(code) &&
      code !== 0x2b &&
      code !== 0x2d &&
      code !== 0x2e
    ) {
      return false
    }
  }
  return false
}

function looksLikeActiveUrl(value: string): boolean {
  // Normalised at the head so the data: and protocol-relative "//" checks see
  // what a browser would see, not what the raw bytes spell.
  const trimmed = stripUrlWhitespace(trimAscii(value))
  if (trimmed.length === 0) return false
  if (isJavascriptUrl(trimmed) || startsWithIgnoreCase(trimmed, "data:")) {
    return true
  }
  if (
    trimmed.length >= 2 &&
    trimmed.charCodeAt(0) === 0x2f &&
    trimmed.charCodeAt(1) === 0x2f
  ) {
    return true
  }
  return hasUriScheme(trimmed)
}

function isXmlnsName(name: string): boolean {
  if (name.length < 5) return false
  const first = name.charCodeAt(0) | 32
  const second = name.charCodeAt(1) | 32
  const third = name.charCodeAt(2) | 32
  const fourth = name.charCodeAt(3) | 32
  const fifth = name.charCodeAt(4) | 32
  if (
    first !== 0x78 ||
    second !== 0x6d ||
    third !== 0x6c ||
    fourth !== 0x6e ||
    fifth !== 0x73
  ) {
    return false
  }
  return name.length === 5 || name.charCodeAt(5) === 0x3a
}

/** Attributes whose whole value addresses a resource. A value here is a URL or
 * it is nothing, so it must resolve to a same-document fragment or an allowed
 * data: payload. Deny by default: anything that is not one of those two is
 * refused, including a relative path.
 *
 * Matched on the local name, so a prefix cannot dodge the rule — this is what
 * covers `xlink:href`, and it covers `xml:base` even if a document binds the
 * XML namespace to some other prefix. The cost is that a bare, unprefixed
 * `base` attribute is held to the same rule; no SVG defines one, so there is
 * nothing legitimate to lose. `xmlns:base` is not affected, because a namespace
 * declaration is exempted before this runs.
 *
 * `xml:base` belongs here rather than in the conditional tier because it names
 * the origin that relative and fragment references are resolved against. A
 * renderer honouring XML Base turns `href="#a"` under
 * `xml:base="https://evil.example/"` into a remote fetch, so even a relative
 * base has to be refused. */
function isReferenceAttribute(localAttr: string): boolean {
  return localAttr === "href" || localAttr === "src" || localAttr === "base"
}

/** Attributes that may address a resource but usually hold a plain value.
 *
 * The animation attributes can retarget a reference while the document is
 * running (`<set attributeName="href" to="javascript:…"/>`), and the funciri
 * paints take a URL alongside colours and keywords. Both are held to the
 * reference rule only when the value already looks like an active URL, because
 * `to="1"` and `fill="red"` are the ordinary case. */
function isResourceValueAttribute(localAttr: string): boolean {
  return (
    localAttr === "to" ||
    localAttr === "from" ||
    localAttr === "by" ||
    localAttr === "values" ||
    localAttr === "fill" ||
    localAttr === "stroke" ||
    localAttr === "clip-path" ||
    localAttr === "mask" ||
    localAttr === "filter" ||
    localAttr === "marker" ||
    localAttr === "marker-start" ||
    localAttr === "marker-mid" ||
    localAttr === "marker-end"
  )
}

/** Returns the rule that rejected the value, or `undefined` when it is safe.
 *
 * The scheme check is applied by attribute, never to every attribute. A colon
 * makes a value look like a URI scheme to `hasUriScheme`, and attributes that
 * carry data rather than references hold colons all the time: a Figma layer
 * named `Icon: Search` exports as `id="Icon: Search"` under `svgIdAttribute`,
 * and `font-family="Inter:Bold"` and `style="fill:red"` are equally ordinary.
 * Scheme-checking those rejected benign documents and blamed the wrong rule.
 *
 * What still guards every attribute is `validateCssText`, which rejects a
 * `url(…)` that does not resolve and an `@import` wherever either appears —
 * including inside an `id`. Separating `unsafeCss` from `unsafeAttribute` is
 * what tells a reader which of the two paths fired. */
function attributeValueUnsafe(
  name: string,
  value: string,
): SvgRejection | undefined {
  if (isXmlnsName(name)) return undefined
  const localAttr = localNameOf(name)
  if (
    localAttr.length >= 2 &&
    localAttr.charCodeAt(0) === 0x6f &&
    localAttr.charCodeAt(1) === 0x6e
  ) {
    return rejected("unsafeAttribute", localAttr)
  }
  if (isReferenceAttribute(localAttr)) {
    if (validateReference(value)) return undefined
    return rejected("unsafeAttribute", localAttr)
  }
  if (isResourceValueAttribute(localAttr)) {
    // `values` on an animation element is a semicolon-separated list, and this
    // check used to read the whole attribute as one string — so a harmless
    // leading "#a" carried an active URL through in the tail. Splitting can
    // only widen what is considered, never narrow it, so it cannot reintroduce
    // a bypass; a single-valued attribute yields one segment and behaves as
    // before.
    for (const segment of value.split(";")) {
      if (looksLikeActiveUrl(segment) && !validateReference(segment)) {
        return rejected("unsafeAttribute", localAttr)
      }
    }
  }
  if (validateCssText(value)) return undefined
  return rejected("unsafeCss", localAttr)
}

function readPiIdent(
  data: string,
  start: number,
): { name: string; next: number } {
  let index = start
  while (index < data.length && isAsciiWhitespace(data.charCodeAt(index))) {
    index += 1
  }
  const begin = index
  while (index < data.length) {
    const code = data.charCodeAt(index)
    if (
      isAsciiWhitespace(code) ||
      code === 0x3d ||
      code === 0x3f ||
      code === 0x3e
    ) {
      break
    }
    index += 1
  }
  return { name: data.slice(begin, index), next: index }
}

function readPiValue(
  data: string,
  start: number,
): { value: string; next: number } | null {
  let index = start
  while (index < data.length && isAsciiWhitespace(data.charCodeAt(index))) {
    index += 1
  }
  if (index >= data.length) return null
  const quote = data.charCodeAt(index)
  if (quote === 0x22 || quote === 0x27) {
    index += 1
    const begin = index
    while (index < data.length && data.charCodeAt(index) !== quote) index += 1
    const value = data.slice(begin, index)
    if (index < data.length) index += 1
    return { value, next: index }
  }
  const begin = index
  while (index < data.length && !isAsciiWhitespace(data.charCodeAt(index))) {
    index += 1
  }
  return { value: data.slice(begin, index), next: index }
}

/** A pseudo-attribute failure is reported as `unsafeProcessingInstruction`,
 * named by the pseudo-attribute, rather than as the attribute rule: where the
 * value sits is the fact a reader needs. */
function piDataUnsafe(data: string): SvgRejection | undefined {
  let index = 0
  const target = readPiIdent(data, index)
  index = target.next
  while (index < data.length) {
    const attribute = readPiIdent(data, index)
    if (attribute.name.length === 0) break
    index = attribute.next
    while (index < data.length && isAsciiWhitespace(data.charCodeAt(index))) {
      index += 1
    }
    if (index < data.length && data.charCodeAt(index) === 0x3d) {
      index += 1
      const parsed = readPiValue(data, index)
      if (parsed === null) break
      index = parsed.next
      if (attributeValueUnsafe(attribute.name, parsed.value) !== undefined) {
        return rejected(
          "unsafeProcessingInstruction",
          localNameOf(attribute.name),
        )
      }
    }
  }
  return undefined
}

function sourceProcessingInstructionsUnsafe(
  source: string,
): SvgRejection | undefined {
  let index = 0
  while (index + 1 < source.length) {
    if (
      source.charCodeAt(index) === 0x3c &&
      source.charCodeAt(index + 1) === 0x3f
    ) {
      index += 2
      const begin = index
      while (
        index + 1 < source.length &&
        !(
          source.charCodeAt(index) === 0x3f &&
          source.charCodeAt(index + 1) === 0x3e
        )
      ) {
        index += 1
      }
      const reason = piDataUnsafe(source.slice(begin, index))
      if (reason !== undefined) return reason
      if (index + 1 < source.length) index += 2
      continue
    }
    index += 1
  }
  return undefined
}

function validateCssText(value: string): boolean {
  for (const token of tokenizeCss(value)) {
    if (token.kind === "at-keyword") {
      const name = token.value
      if (
        name.length === 6 &&
        (name.charCodeAt(0) | 32) === 0x69 &&
        (name.charCodeAt(1) | 32) === 0x6d &&
        (name.charCodeAt(2) | 32) === 0x70 &&
        (name.charCodeAt(3) | 32) === 0x6f &&
        (name.charCodeAt(4) | 32) === 0x72 &&
        (name.charCodeAt(5) | 32) === 0x74
      ) {
        return false
      }
    }
    if (token.kind === "url" && !validateReference(token.value)) return false
  }
  return true
}

function localNameOf(name: string): string {
  const separator = name.lastIndexOf(":")
  return (separator >= 0 ? name.slice(separator + 1) : name).toLowerCase()
}

function attributeNames(element: SvgElement): readonly string[] {
  if (typeof element.getAttributeNames === "function") {
    return element.getAttributeNames()
  }
  const attributes = element.attributes
  const names: string[] = []
  for (let index = 0; index < attributes.length; index += 1) {
    const attribute = attributes.item(index)
    if (attribute !== null) names.push(attribute.name)
  }
  return names
}

function childAt(node: SvgNode, index: number): SvgNode | null {
  const children = node.childNodes
  if (typeof children.item === "function") return children.item(index) ?? null
  return children[index] ?? null
}

function elementLocalName(element: SvgElement): string {
  const local = element.localName
  if (typeof local === "string" && local.length > 0) return local.toLowerCase()
  return localNameOf(element.tagName)
}

function walkUnsafe(node: SvgNode): SvgRejection | undefined {
  if (node.nodeType === PROCESSING_INSTRUCTION_NODE) {
    return piDataUnsafe(node.data ?? node.nodeValue ?? "")
  }
  if (node.nodeType === ELEMENT_NODE) {
    const element = node as SvgElement
    const local = elementLocalName(element)
    if (local === "script" || local === "foreignobject") {
      return rejected("unsafeElement", local)
    }
    if (local === "style" && !validateCssText(element.textContent ?? "")) {
      return rejected("unsafeCss", local)
    }
    for (const name of attributeNames(element)) {
      const reason = attributeValueUnsafe(
        name,
        element.getAttribute(name) ?? "",
      )
      if (reason !== undefined) return reason
    }
  }
  const children = node.childNodes
  for (let index = 0; index < children.length; index += 1) {
    const child = childAt(node, index)
    if (child === null) continue
    const reason = walkUnsafe(child)
    if (reason !== undefined) return reason
  }
  return undefined
}

function hasParserError(document: SvgDocument): boolean {
  if (typeof document.getElementsByTagName === "function") {
    if (document.getElementsByTagName("parsererror").length > 0) return true
  }
  const root = document.documentElement
  if (root === null) return true
  return elementLocalName(root) === "parsererror"
}

export function validateSvgSource(
  input: string | Uint8Array,
  parser: InjectedDomParser,
): SvgValidation {
  const source = decodeTransfer(input)
  // A transfer that never decoded left no string to return and no tree to
  // judge, so there is no verdict to give. Nothing about it concerns safety.
  if (source === null) return { ok: false, code: "INTERNAL_ERROR" }
  if (utf8ByteLength(source) > MAX_SVG_BYTES) {
    return { ok: false, code: "LIMIT_EXCEEDED" }
  }
  let document: SvgDocument
  try {
    document = parser.parseFromString(source, "image/svg+xml")
  } catch {
    return unsafe(source, rejected("parserError"))
  }
  if (hasParserError(document)) return unsafe(source, rejected("parserError"))
  const piReason = sourceProcessingInstructionsUnsafe(source)
  if (piReason !== undefined) return unsafe(source, piReason)
  const reason = walkUnsafe(document)
  if (reason !== undefined) return unsafe(source, reason)
  return { ok: true, source, safe: true }
}
