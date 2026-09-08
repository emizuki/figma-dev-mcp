import { describe, expect, test } from "bun:test"
import { DOMParser, onErrorStopParsing } from "@xmldom/xmldom"

import {
  MAX_IDENTIFIER_BYTES,
  MAX_RASTER_DECODED_BYTES,
  MAX_SVG_BYTES,
} from "../shared/limits"
import {
  validateDataUrl,
  validateSvgSource,
  type SvgDocument,
  type SvgElement,
} from "./svg"

const parser = new DOMParser({ onError: onErrorStopParsing })

const fixture = async (name: string): Promise<string> =>
  Bun.file(
    new URL(`../../../tests/contracts/fixtures/${name}`, import.meta.url),
  ).text()

function validate(source: string | Uint8Array) {
  return validateSvgSource(source, parser)
}

function svgWithCss(css: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg"><style>${css}</style></svg>`
}

// XML attribute-value normalisation replaces a literal tab, line feed or
// carriage return with a space before the value ever reaches the classifier
// (verified against @xmldom/xmldom), which would silently turn a whitespace
// fixture into the space fixture and make the two indistinguishable. Numeric
// character references are exempt from that normalisation, so they are what an
// attacker writes and what these tests write. A space is emitted literally,
// because a space really is a space either way.
function escapeAttributeValue(value: string): string {
  let out = ""
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code === 0x09 || code === 0x0a || code === 0x0d) {
      out += `&#${code};`
      continue
    }
    if (code === 0x26) out += "&amp;"
    else if (code === 0x3c) out += "&lt;"
    else if (code === 0x22) out += "&quot;"
    else out += value.charAt(index)
  }
  return out
}

function svgWithAttr(name: string, value: string): string {
  const escaped = escapeAttributeValue(value)
  return `<svg xmlns="http://www.w3.org/2000/svg"><rect ${name}="${escaped}"/></svg>`
}

describe("SVG safety policy", () => {
  test("preserves viewBox, paths, gradients, masks, clip paths, and fragment references", async () => {
    const source = await fixture("safe.svg")
    const result = validate(source)
    expect(result).toEqual({ ok: true, source, safe: true })
    expect(source).toContain("viewBox")
    expect(source).toContain("<path")
    expect(source).toContain("linearGradient")
    expect(source).toContain("url(#gradient)")
    expect(source).toContain("url(#mask)")
    expect(source).toContain("url(#clip)")
  })

  test("rejects every unsafe construct listed in the spec", async () => {
    const script = await fixture("unsafe-script.svg")
    const external = await fixture("unsafe-external.svg")
    const nested = await fixture("unsafe-nested-data.svg")
    expect(validate(script)).toMatchObject({ safe: false })
    expect(validate(external)).toMatchObject({ safe: false })
    expect(validate(nested)).toMatchObject({ safe: false })

    const cases = [
      `<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="1" height="1"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1" onclick="alert(1)"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><a href="javascript:alert(1)"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url(https://evil.example/x)"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><style>@import url(https://evil.example/x.css);</style></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:url(https://evil.example/x)"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="https://evil.example/x.png"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="https://evil.example/x.svg#a"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:text/html,&lt;script&gt;alert(1)&lt;/script&gt;"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"`,
    ]
    for (const source of cases) {
      // The source comes back either way; only the verdict changes.
      expect(validate(source)).toMatchObject({ ok: true, source, safe: false })
    }
  })

  test("fails a transfer that never decoded, and an oversized source", () => {
    // No string decoded means no source to return and no document to judge,
    // so there is no verdict to give. Nothing about it concerns safety.
    expect(validate(Uint8Array.of(0xff, 0xfe, 0xfd))).toEqual({
      ok: false,
      code: "INTERNAL_ERROR",
    })
    expect(validate("\uD800")).toEqual({ ok: false, code: "INTERNAL_ERROR" })
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg">${"a".repeat(MAX_SVG_BYTES)}`,
      ),
    ).toEqual({
      ok: false,
      code: "LIMIT_EXCEEDED",
    })
  })

  test("rejects javascript and network URLs on processing instructions and any attribute", () => {
    const cases = [
      `<?xml-stylesheet href="https://evil.example/x.css" type="text/css"?><svg xmlns="http://www.w3.org/2000/svg"/>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="#ok"><set attributeName="href" to="javascript:alert(1)"/></image></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><set values="javascript:alert(1)"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><set from="https://evil.example/x"/></svg>`,
      `<svg xmlns="http://www.w3.org/2000/svg"><rect fill="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E"/></svg>`,
    ]
    for (const source of cases) {
      expect(validate(source)).toMatchObject({ ok: true, source, safe: false })
    }
    const safe = `<svg xmlns="http://www.w3.org/2000/svg"><image href="#ok"/></svg>`
    expect(validate(safe)).toEqual({ ok: true, source: safe, safe: true })
  })

  test("allows bounded PNG data URLs and does not rewrite safe source", async () => {
    const png =
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
    const source = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,${png}"/></svg>`
    const result = validate(source)
    expect(result).toEqual({ ok: true, source, safe: true })
  })

  test("widening for fonts does not admit other data URLs", () => {
    // These must keep failing. Written before the predicate is widened.
    for (const source of [
      svgWithCss(`@font-face{src:url(data:text/html;base64,PGh0bWw+)}`),
      svgWithCss(
        `@font-face{src:url(data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)}`,
      ),
      svgWithCss(
        `@font-face{src:url(data:application/javascript;base64,YWxlcnQoMSk=)}`,
      ),
      svgWithCss(`@font-face{src:url(https://example.com/f.woff2)}`),
      svgWithCss(`@font-face{src:url(javascript:alert(1))}`),
      // Additional attacker-plausible schemes/mimes beyond the brief's floor.
      svgWithCss(`@font-face{src:url(vbscript:msgbox(1))}`),
      svgWithCss(`@font-face{src:url(blob:https://evil.example/uuid)}`),
      svgWithCss(`@font-face{src:url(data:font/collection;base64,AAAA)}`),
      svgWithCss(
        `@font-face{src:url(data:application/font-woff2;base64,AAAA)}`,
      ),
      svgWithCss(`@font-face{src:url(//evil.example/f.woff2)}`),
    ]) {
      expect(validate(source).safe).toBe(false)
    }
  })

  test("script and external href stay unsafe", () => {
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>`,
      ).safe,
    ).toBe(false)
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><image href="https://x/y.png"/></svg>`,
      ).safe,
    ).toBe(false)
  })

  test("an embedded font data URL is accepted", () => {
    const source = svgWithCss(
      `@font-face{font-family:x;src:url(data:font/woff2;base64,d09GMgABAAAAAAAA)}`,
    )
    expect(validate(source).safe).toBe(true)
  })

  test("an embedded font data URL is accepted with mixed-case scheme and mime", () => {
    const source = svgWithCss(
      `@font-face{font-family:x;src:url(Data:FONT/WOFF2;base64,d09GMgABAAAAAAAA)}`,
    )
    expect(validate(source).safe).toBe(true)
  })

  // MAX_SVG_BYTES (whole-document cap, 4 MiB) is smaller than
  // MAX_RASTER_DECODED_BYTES (per-data-url decoded cap, 12 MiB), and decoded
  // bytes can never exceed the source bytes they were decoded from. So no
  // data: URL that survives validateSvgSource's document-size gate can ever
  // carry enough decoded bytes to exercise the ceiling inside validateDataUrl
  // — a test built through validateSvgSource cannot discriminate whether that
  // ceiling still runs before the font-mime short-circuit. This calls
  // validateDataUrl directly so the property is actually pinned.
  test("validateDataUrl rejects an oversized font payload even though the mime is allowed", () => {
    const oversized = Buffer.alloc(MAX_RASTER_DECODED_BYTES + 1, 0x41).toString(
      "base64",
    )
    expect(validateDataUrl(`data:font/woff2;base64,${oversized}`)).toBe(false)
  })

  test("an unsafe SVG is returned with its source and a verdict", () => {
    const source = `<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>`
    const result = validate(source)
    expect(result.source).toBe(source)
    expect(result.safe).toBe(false)
    expect(result.rejection).toEqual({ kind: "unsafeElement", name: "script" })
  })

  test("a safe SVG carries no rejection", () => {
    const result = validate(
      `<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0"/></svg>`,
    )
    expect(result.safe).toBe(true)
    expect(result.rejection).toBeUndefined()
  })

  test("a rejection says which rule fired", () => {
    const script = validate(
      `<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>`,
    )
    expect(script.safe).toBe(false)
    expect(script.rejection).toEqual({ kind: "unsafeElement", name: "script" })

    const external = validate(
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="https://x/y"/></svg>`,
    )
    expect(external.rejection?.kind).toBe("unsafeAttribute")
  })

  test("every rejection kind is reachable and names the offender", () => {
    // parserError: unterminated root element.
    expect(
      validate(`<svg xmlns="http://www.w3.org/2000/svg"`).rejection,
    ).toEqual({ kind: "parserError" })
    // unsafeElement: foreignObject, named by its lower-cased local name.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="1" height="1"/></svg>`,
      ).rejection,
    ).toEqual({ kind: "unsafeElement", name: "foreignobject" })
    // unsafeAttribute: an event handler, named without its prefix.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="alert(1)"/></svg>`,
      ).rejection,
    ).toEqual({ kind: "unsafeAttribute", name: "onclick" })
    // unsafeAttribute: a namespaced href keeps the local name only.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="https://evil.example/x.svg#a"/></svg>`,
      ).rejection,
    ).toEqual({ kind: "unsafeAttribute", name: "href" })
    // unsafeCss: a <style> body, named by the element that carried it.
    expect(
      validate(svgWithCss(`@import url(https://evil.example/x.css);`))
        .rejection,
    ).toEqual({ kind: "unsafeCss", name: "style" })
    // unsafeCss: an attribute value that reached the CSS fallback, named by
    // the attribute that carried it.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url(https://evil.example/x)"/></svg>`,
      ).rejection,
    ).toEqual({ kind: "unsafeCss", name: "fill" })
    // unsafeCss: a style attribute is CSS, so it is judged as CSS. It is not
    // scheme-checked, or `style="fill:red"` would read as a URI scheme.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:url(https://evil.example/x)"/></svg>`,
      ).rejection,
    ).toEqual({ kind: "unsafeCss", name: "style" })
    // unsafeProcessingInstruction: named by the pseudo-attribute that failed.
    expect(
      validate(
        `<?xml-stylesheet href="https://evil.example/x.css" type="text/css"?><svg xmlns="http://www.w3.org/2000/svg"/>`,
      ).rejection,
    ).toEqual({ kind: "unsafeProcessingInstruction", name: "href" })
  })

  // A colon in a value is not a URI scheme. Figma writes layer names into `id`
  // when `svgIdAttribute` is set, and a layer called "Icon: Search" exports as
  // `id="Icon: Search"` — which the scheme check read as the scheme `Icon`.
  // Every value below is benign and must survive.
  test("a colon in a data-bearing attribute is not a scheme", () => {
    const attributes = [
      `id="Icon: Search"`,
      `id="a:b"`,
      `id="Frame 1: Copy 2"`,
      `font-family="Inter:Bold"`,
      `style="fill:red"`,
      `style="fill:#ff0000;stroke-width:2"`,
      `aria-label="Step 1: pick a plan"`,
      `data-name="Icon: Search"`,
      `clip-path="url(#clip)"`,
      `fill="red"`,
      `to="1"`,
      `from="0"`,
    ]
    for (const attribute of attributes) {
      const source = `<svg xmlns="http://www.w3.org/2000/svg"><rect ${attribute}/></svg>`
      expect(validate(source)).toEqual({ ok: true, source, safe: true })
    }

    // The whole shape Figma emits under svgIdAttribute, not just one attribute.
    const exported = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g id="Icon: Search"><path id="Vector: outline" d="M4 4h16v16H4z" fill="url(#gradient)"/></g><defs><linearGradient id="gradient: primary"><stop offset="0" stop-color="#fff"/></linearGradient></defs></svg>`
    expect(validate(exported)).toEqual({
      ok: true,
      source: exported,
      safe: true,
    })
  })

  // The other direction. Narrowing the scheme check to attributes that address
  // a resource must not let a live URL through one of them.
  test("an active URL in a resource attribute is still rejected", () => {
    const cases: [string, string][] = [
      [`<a href="javascript:alert(1)"/>`, "href"],
      [`<a href="https://example.com/x"/>`, "href"],
      [`<a href="HTTPS://EXAMPLE.COM/x"/>`, "href"],
      [`<a href="//example.com/x"/>`, "href"],
      [`<a href="ftp://example.com/x"/>`, "href"],
      [`<image href="data:text/html,%3Cscript%3E"/>`, "href"],
      [`<image src="https://example.com/x.png"/>`, "src"],
      [`<set to="javascript:alert(1)"/>`, "to"],
      [`<set from="https://evil.example/x"/>`, "from"],
      [`<set by="https://evil.example/x"/>`, "by"],
      [`<animate values="javascript:alert(1)"/>`, "values"],
      [`<rect fill="data:image/svg+xml,%3Csvg/%3E"/>`, "fill"],
      [`<rect stroke="https://evil.example/x"/>`, "stroke"],
      [`<rect filter="javascript:alert(1)"/>`, "filter"],
      [`<rect mask="https://evil.example/x"/>`, "mask"],
      [`<path marker-end="https://evil.example/x"/>`, "marker-end"],
      [`<rect onclick="alert(1)"/>`, "onclick"],
    ]
    for (const [element, name] of cases) {
      const result = validate(
        `<svg xmlns="http://www.w3.org/2000/svg">${element}</svg>`,
      )
      expect(result.safe).toBe(false)
      expect(result.rejection).toEqual({ kind: "unsafeAttribute", name })
    }

    // A namespaced href is still caught, and a fragment reference still passes.
    const namespaced = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="https://evil.example/x.svg#a"/></svg>`
    expect(validate(namespaced).rejection).toEqual({
      kind: "unsafeAttribute",
      name: "href",
    })
    const fragment = `<svg xmlns="http://www.w3.org/2000/svg"><use href="#a"/></svg>`
    expect(validate(fragment)).toEqual({
      ok: true,
      source: fragment,
      safe: true,
    })
  })

  // xml:base names the origin that every relative and fragment reference in
  // the document is resolved against, so a remote base turns an otherwise safe
  // href="#a" into a remote fetch. Deny by default, like href itself.
  test("a base that names a remote origin is rejected", () => {
    const bases = [
      `xml:base="https://evil.example/"`,
      `xml:base="//evil.example/"`,
      `xml:base="http://evil.example/assets/"`,
      // A relative base re-bases just as effectively.
      `xml:base="../other.svg"`,
      `xml:base="assets/"`,
      // A prefix must not dodge the rule, and neither must its absence.
      `base="https://evil.example/"`,
      `xmlbase:base="https://evil.example/"`,
    ]
    for (const base of bases) {
      const result = validate(
        `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xmlbase="http://www.w3.org/XML/1998/namespace" ${base}><use href="#a"/></svg>`,
      )
      expect(result.safe).toBe(false)
      expect(result.rejection).toEqual({
        kind: "unsafeAttribute",
        name: "base",
      })
    }

    // A same-document base fetches nothing, so it passes the same rule href does.
    const fragment = `<svg xmlns="http://www.w3.org/2000/svg" xml:base="#a"><use href="#a"/></svg>`
    expect(validate(fragment)).toEqual({
      ok: true,
      source: fragment,
      safe: true,
    })

    // A namespace declaration is an identifier, not a fetch, and stays exempt
    // even though its local name is now a reference attribute.
    const declaration = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:base="http://www.w3.org/1999/xlink"><rect/></svg>`
    expect(validate(declaration)).toEqual({
      ok: true,
      source: declaration,
      safe: true,
    })
  })

  // A url() that does not resolve is still refused wherever it appears, which
  // is what still guards attributes that are no longer scheme-checked.
  test("an unresolvable url() is refused in any attribute", () => {
    for (const attribute of [
      `id="url(https://evil.example/x)"`,
      `d="url(https://evil.example/x)"`,
      `aria-label="url(https://evil.example/x)"`,
    ]) {
      const result = validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect ${attribute}/></svg>`,
      )
      expect(result.safe).toBe(false)
      expect(result.rejection?.kind).toBe("unsafeCss")
    }
  })

  test("a size failure carries no rule, and an accepted document carries none either", () => {
    const oversized = validate(
      `<svg xmlns="http://www.w3.org/2000/svg">${"a".repeat(MAX_SVG_BYTES)}`,
    )
    expect(oversized).toEqual({ ok: false, code: "LIMIT_EXCEEDED" })
    expect(oversized.rejection).toBeUndefined()

    const safe = `<svg xmlns="http://www.w3.org/2000/svg"><image href="#ok"/></svg>`
    expect(validate(safe).rejection).toBeUndefined()
  })

  test("an unusable offender name is omitted rather than sent oversized", () => {
    const huge = `on${"b".repeat(MAX_IDENTIFIER_BYTES)}`
    expect(huge.length).toBeGreaterThan(MAX_IDENTIFIER_BYTES)
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect ${huge}="alert(1)"/></svg>`,
      ).rejection,
    ).toEqual({ kind: "unsafeAttribute" })
  })

  test("validateDataUrl accepts a font payload at the byte ceiling", () => {
    const atLimit = Buffer.alloc(MAX_RASTER_DECODED_BYTES, 0x41).toString(
      "base64",
    )
    expect(validateDataUrl(`data:font/woff2;base64,${atLimit}`)).toBe(true)
  })

  test("narrowing for whitespace and lists does not admit anything new", () => {
    // Every one of these must stay rejected. Written and run BEFORE the change.
    for (const source of [
      svgWithAttr("href", "javascript:alert(1)"),
      svgWithAttr("href", "https://example.com/x"),
      svgWithAttr("xml:base", "https://evil.example/"),
      svgWithAttr("to", "javascript:alert(1)"),
      svgWithAttr("fill", "url(https://example.com/x)"),
      `<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>`,
      svgWithCss(`@font-face{src:url(data:text/html;base64,PGh0bWw+)}`),
      svgWithCss(`@font-face{src:url(https://example.com/f.woff2)}`),
    ]) {
      expect(validate(source).safe).toBe(false)
    }
  })

  test("benign values stay clean", () => {
    // These were a real defect once, fixed, and a careless normalisation brings
    // them back. `id="Icon: Search"` is a layer name, not a URI scheme.
    for (const source of [
      svgWithAttr("id", "Icon: Search"),
      svgWithAttr("id", "a:b"),
      svgWithAttr("font-family", "Inter:Bold"),
      svgWithAttr("style", "fill:red"),
      svgWithCss(
        `@font-face{src:url(data:font/woff2;base64,d09GMgABAAAAAAAA)}`,
      ),
    ]) {
      expect(validate(source).safe).toBe(true)
    }
  })

  test("whitespace inside a scheme does not hide it", () => {
    // Browsers strip ASCII tab, LF and CR from anywhere in a URL before
    // parsing, so a browser reads this as javascript: while a prefix test does
    // not. The gap is written as a character reference by svgWithAttr, since a
    // literal one would be normalised to a space by the XML parser first.
    for (const gap of ["\t", "\n", "\r"]) {
      const value = `jav${gap}ascript:alert(1)`
      expect(validate(svgWithAttr("to", value)).safe).toBe(false)
      // Not only javascript:. A split data: scheme is invisible to the scheme
      // test at the head of looksLikeActiveUrl too, and a browser reads it.
      expect(validate(svgWithAttr("to", `dat${gap}a:text/html,x`)).safe).toBe(
        false,
      )
    }
  })

  test("a space is not stripped, so it is not a scheme", () => {
    // WHATWG keeps the space character. Flagging this would be a false
    // positive, and false positives are what make a verdict worthless.
    expect(validate(svgWithAttr("to", "jav ascript:alert(1)")).safe).toBe(true)
  })

  // The other side of the segment split. `values` is the only semicolon list
  // among the resource-value attributes; everywhere else a semicolon is
  // content. Splitting these tore one value into pieces that are not values —
  // a truncated `data:image/png` that parses as nothing, and a fragment name
  // read as though it were a second entry — and rejected what a browser never
  // fetches.
  test("a semicolon in a single-valued attribute is content, not a list", () => {
    const png =
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
    for (const value of [
      `data:image/png;base64,${png}`,
      `data:font/woff2;base64,d09GMgABAAAAAAAA`,
      `#a;javascript:alert(1)`,
    ]) {
      for (const attribute of ["to", "from", "by", "fill", "stroke"]) {
        expect(validate(svgWithAttr(attribute, value)).safe).toBe(true)
      }
      // Tier 1 reads the whole value for the same reason, and always did.
      expect(validate(svgWithAttr("href", value)).safe).toBe(true)
    }
  })

  test("every segment of a list value is inspected", () => {
    // `values` on an animation element is semicolon-separated. The classifier
    // read only the head, so a harmless first segment carried the rest through.
    const source = `<svg xmlns="http://www.w3.org/2000/svg"><animate attributeName="href" values="#a;javascript:alert(1)"/></svg>`
    expect(validate(source).safe).toBe(false)
  })
})

// A hand-built tree, so the two DOM shapes `svg.ts` supports can both be
// exercised: a parser that offers `getAttributeNames()` and `childNodes.item()`
// (what a browser gives) and one that offers only the `attributes` map and
// index access (what a minimal DOM gives). Only the first was ever walked.
interface FakeElementSpec {
  readonly name: string
  /** `null` stands for a DOM that exposes no `localName`, so the classifier
   * has to take the local name off `tagName` itself. */
  readonly localName?: string | null
  readonly attributes?: Record<string, string>
  readonly text?: string
  readonly children?: FakeElementSpec[]
}

function buildElement(spec: FakeElementSpec, modern: boolean): SvgElement {
  const attributes = spec.attributes ?? {}
  const names = Object.keys(attributes)
  const children = (spec.children ?? []).map((child) =>
    buildElement(child, modern),
  )
  const childNodes = modern
    ? {
        length: children.length,
        item: (index: number) => children[index] ?? null,
      }
    : Object.assign({ length: children.length }, children)
  const element: SvgElement = {
    nodeType: 1,
    tagName: spec.name,
    localName: spec.localName === undefined ? spec.name : spec.localName,
    textContent: spec.text ?? "",
    childNodes: childNodes as SvgElement["childNodes"],
    attributes: {
      length: names.length,
      item: (index: number) => {
        const name = names[index]
        return name === undefined ? null : { name }
      },
    },
    getAttribute: (name: string) => attributes[name] ?? null,
  }
  if (!modern) return element
  return Object.assign({}, element, { getAttributeNames: () => names })
}

function fakeParserFor(spec: FakeElementSpec, modern: boolean) {
  const root = buildElement(spec, modern)
  const document: SvgDocument = {
    nodeType: 9,
    documentElement: root,
    childNodes: modern
      ? { length: 1, item: () => root }
      : (Object.assign({ length: 1 }, [root]) as SvgDocument["childNodes"]),
  }
  return { parseFromString: () => document }
}

const encoder = new TextEncoder()
const utf8Bytes = (value: string): number => encoder.encode(value).length

describe("SVG acceptance", () => {
  test("a document exactly at the byte ceiling is accepted, counted in UTF-8", () => {
    const head = '<svg xmlns="http://www.w3.org/2000/svg"><desc>'
    const tail = "</desc></svg>"
    // Two- and four-byte characters, so a byte count is not a character count.
    const filler = "é😀"
    const fillerBytes = utf8Bytes(filler)
    const budget = MAX_SVG_BYTES - utf8Bytes(head) - utf8Bytes(tail)
    const whole = Math.floor(budget / fillerBytes)
    const source =
      head +
      filler.repeat(whole) +
      "a".repeat(budget - whole * fillerBytes) +
      tail

    expect(utf8Bytes(source)).toBe(MAX_SVG_BYTES)
    expect(source.length).toBeLessThan(MAX_SVG_BYTES)
    expect(validate(source)).toEqual({ ok: true, source, safe: true })
    expect(validate(source + "a")).toEqual({
      ok: false,
      code: "LIMIT_EXCEEDED",
    })
  })

  test("an offender name exactly at the identifier ceiling is still reported", () => {
    const name = `on${"a".repeat(MAX_IDENTIFIER_BYTES - 2)}`
    expect(utf8Bytes(name)).toBe(MAX_IDENTIFIER_BYTES)
    const result = validate(svgWithAttr(name, "go()"))

    expect(result).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "unsafeAttribute", name },
    })
  })

  test("every media type the data: allow-list names is accepted", () => {
    const png = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAECA=="
    const jpeg = "/9j/wAALCAAGAAoBAREA/9k="
    const webp = "UklGRgAAAABXRUJQ"
    const font = "AAEAAAAKAIAAAwAgT1MvMg=="
    const cases: Array<[string, string]> = [
      ["image/png", png],
      ["image/jpeg", jpeg],
      ["image/jpg", jpeg],
      ["image/webp", webp],
      ["font/woff", font],
      ["font/woff2", font],
      ["font/ttf", font],
      ["font/otf", font],
      ["application/font-woff", font],
      ["application/x-font-ttf", font],
      ["application/x-font-opentype", font],
    ]
    let accepted = 0
    for (const [mime, payload] of cases) {
      expect({
        mime,
        ok: validateDataUrl(`data:${mime};base64,${payload}`),
      }).toEqual({
        mime,
        ok: true,
      })
      accepted += 1
    }
    expect(accepted).toBe(cases.length)
    expect(accepted).toBe(11)
    // The list is an allow-list, not a prefix match.
    expect(validateDataUrl(`data:font/collection;base64,${font}`)).toBe(false)
    expect(validateDataUrl(`data:image/svg+xml;base64,${png}`)).toBe(false)
  })

  test("an at-rule other than @import passes the CSS check", () => {
    for (const rule of [
      "@media screen { rect { fill: red } }",
      "@font-face { font-family: Inter }",
      "@supports (fill: red) { rect { fill: red } }",
      "@keyframes spin { from { fill: red } }",
    ]) {
      expect({ rule, result: validate(svgWithCss(rule)) }).toMatchObject({
        rule,
        result: { ok: true, safe: true },
      })
    }
    expect(validate(svgWithCss('@import "x"'))).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "unsafeCss" },
    })
  })

  test("a percent-encoded data: URL is read as the same bytes as its base64 twin", () => {
    // PNG magic followed by an IHDR header, written both ways.
    const percent =
      "%89PNG%0D%0A%1A%0A%00%00%00%0DIHDR%00%00%00%08%00%00%00%04%08"
    expect(validateDataUrl(`data:image/png,${percent}`)).toBe(true)
    expect(
      validateDataUrl(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAECA==",
      ),
    ).toBe(true)
    // The media type is read from the first parameter, before the comma that
    // starts the payload, and a comma inside the payload does not end it.
    expect(validateDataUrl(`data:image/png,${percent}%2C`)).toBe(true)
  })

  test("a UTF-8 byte transfer decodes to the same verdict as the string", () => {
    const source =
      '<svg xmlns="http://www.w3.org/2000/svg"><desc>é😀</desc></svg>'
    const bytes = encoder.encode(source)

    expect(validate(bytes)).toEqual({ ok: true, source, safe: true })
    // A paired surrogate is not a lone one, so the string form is accepted too.
    expect(validate(source)).toEqual({ ok: true, source, safe: true })
  })

  test("a value padded with ASCII whitespace is trimmed before the rule runs", () => {
    for (const padded of [" #a", "#a ", "\t#a\n", "  #a  "]) {
      expect({
        padded,
        result: validate(svgWithAttr("href", padded)),
      }).toMatchObject({ padded, result: { ok: true, safe: true } })
    }
    // The media type of a data: URL is trimmed on both sides too, which is
    // where a trailing space actually changes the answer: an untrimmed
    // "image/png " matches no entry in the allow-list.
    const png = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAECA=="
    expect(validateDataUrl(`data: image/png ;base64,${png}`)).toBe(true)
  })

  test("a scheme is a scheme only when it starts with a letter", () => {
    // A leading digit is not a scheme, so `to` keeps its ordinary meaning.
    expect(validate(svgWithAttr("to", "1:2"))).toMatchObject({
      ok: true,
      safe: true,
    })
    expect(validate(svgWithAttr("to", "https://evil.example"))).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "unsafeAttribute", name: "to" },
    })
    // Protocol-relative is active even without a scheme at all.
    expect(validate(svgWithAttr("to", "//evil.example/x"))).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "unsafeAttribute", name: "to" },
    })
  })

  test("both DOM shapes are walked: modern accessors and a bare attribute map", () => {
    const safe: FakeElementSpec = {
      name: "svg",
      children: [
        {
          name: "g",
          children: [{ name: "rect", attributes: { fill: "red" } }],
        },
      ],
    }
    const unsafe: FakeElementSpec = {
      name: "svg",
      children: [
        {
          name: "g",
          children: [{ name: "rect", attributes: { onclick: "go()" } }],
        },
      ],
    }
    let shapes = 0
    for (const modern of [true, false]) {
      expect({
        modern,
        result: validateSvgSource("<svg/>", fakeParserFor(safe, modern)),
      }).toMatchObject({ modern, result: { ok: true, safe: true } })
      expect({
        modern,
        result: validateSvgSource("<svg/>", fakeParserFor(unsafe, modern)),
      }).toMatchObject({
        modern,
        result: {
          ok: true,
          safe: false,
          rejection: { kind: "unsafeAttribute", name: "onclick" },
        },
      })
      shapes += 1
    }
    expect(shapes).toBe(2)
  })

  test("element and attribute names are matched without their prefix or casing", () => {
    const spec: FakeElementSpec = {
      name: "svg",
      children: [{ name: "svg:ForeignObject", localName: null }],
    }
    expect(
      validateSvgSource("<svg/>", fakeParserFor(spec, true)),
    ).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "unsafeElement", name: "foreignobject" },
    })
    expect(
      validateSvgSource(
        "<svg/>",
        fakeParserFor(
          {
            name: "svg",
            children: [
              { name: "rect", attributes: { "XLINK:HREF": "javascript:go()" } },
            ],
          },
          true,
        ),
      ),
    ).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "unsafeAttribute", name: "href" },
    })
  })

  test("a parser error is caught however the parser reports it", () => {
    const root: SvgElement = {
      nodeType: 1,
      tagName: "parsererror",
      localName: "parsererror",
      textContent: "",
      childNodes: { length: 0, item: () => null },
      attributes: { length: 0, item: () => null },
      getAttribute: () => null,
      getAttributeNames: () => [],
    }
    // Signalled by a `parsererror` element the document can look up. The root
    // is an ordinary `<svg>`, so the lookup is the only thing that can find it.
    const ordinaryRoot: SvgElement = {
      ...root,
      tagName: "svg",
      localName: "svg",
    }
    expect(
      validateSvgSource("<svg", {
        parseFromString: () => ({
          nodeType: 9,
          documentElement: ordinaryRoot,
          childNodes: { length: 0, item: () => null },
          getElementsByTagName: () => ({ length: 1 }),
        }),
      }),
    ).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "parserError" },
    })
    // Signalled only by the root element's own name.
    expect(
      validateSvgSource("<svg", {
        parseFromString: () => ({
          nodeType: 9,
          documentElement: root,
          childNodes: { length: 0, item: () => null },
        }),
      }),
    ).toMatchObject({
      ok: true,
      safe: false,
      rejection: { kind: "parserError" },
    })
  })
})
