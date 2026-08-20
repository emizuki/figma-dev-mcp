import { describe, expect, test } from "bun:test"
import { DOMParser, onErrorStopParsing } from "@xmldom/xmldom"

import {
  MAX_IDENTIFIER_BYTES,
  MAX_RASTER_DECODED_BYTES,
  MAX_SVG_BYTES,
} from "../shared/limits"
import { validateDataUrl, validateSvgSource } from "./svg"

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

describe("SVG safety policy", () => {
  test("preserves viewBox, paths, gradients, masks, clip paths, and fragment references", async () => {
    const source = await fixture("safe.svg")
    const result = validate(source)
    expect(result).toEqual({ ok: true, source })
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
    expect(validate(script)).toMatchObject({ ok: false, code: "UNSAFE_SVG" })
    expect(validate(external)).toMatchObject({ ok: false, code: "UNSAFE_SVG" })
    expect(validate(nested)).toMatchObject({ ok: false, code: "UNSAFE_SVG" })

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
      expect(validate(source)).toMatchObject({ ok: false, code: "UNSAFE_SVG" })
    }
  })

  test("rejects invalid UTF-8 transfer and oversized source", () => {
    expect(validate(Uint8Array.of(0xff, 0xfe, 0xfd))).toMatchObject({
      ok: false,
      code: "UNSAFE_SVG",
    })
    expect(validate("\uD800")).toMatchObject({ ok: false, code: "UNSAFE_SVG" })
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
      expect(validate(source)).toMatchObject({ ok: false, code: "UNSAFE_SVG" })
    }
    const safe = `<svg xmlns="http://www.w3.org/2000/svg"><image href="#ok"/></svg>`
    expect(validate(safe)).toEqual({ ok: true, source: safe })
  })

  test("allows bounded PNG data URLs and does not rewrite safe source", async () => {
    const png =
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
    const source = `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,${png}"/></svg>`
    const result = validate(source)
    expect(result).toEqual({ ok: true, source })
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
      expect(validate(source).ok).toBe(false)
    }
  })

  test("script and external href stay rejected", () => {
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>`,
      ).ok,
    ).toBe(false)
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><image href="https://x/y.png"/></svg>`,
      ).ok,
    ).toBe(false)
  })

  test("an embedded font data URL is accepted", () => {
    const source = svgWithCss(
      `@font-face{font-family:x;src:url(data:font/woff2;base64,d09GMgABAAAAAAAA)}`,
    )
    expect(validate(source).ok).toBe(true)
  })

  test("an embedded font data URL is accepted with mixed-case scheme and mime", () => {
    const source = svgWithCss(
      `@font-face{font-family:x;src:url(Data:FONT/WOFF2;base64,d09GMgABAAAAAAAA)}`,
    )
    expect(validate(source).ok).toBe(true)
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

  test("a rejection says which rule fired", () => {
    const script = validate(
      `<svg xmlns="http://www.w3.org/2000/svg"><script/></svg>`,
    )
    expect(script.ok).toBe(false)
    expect(script.reason).toEqual({ kind: "unsafeElement", name: "script" })

    const external = validate(
      `<svg xmlns="http://www.w3.org/2000/svg"><image href="https://x/y"/></svg>`,
    )
    expect(external.reason?.kind).toBe("unsafeAttribute")
  })

  test("every rejection kind is reachable and names the offender", () => {
    // parserError: unterminated root element.
    expect(validate(`<svg xmlns="http://www.w3.org/2000/svg"`).reason).toEqual({
      kind: "parserError",
    })
    // parserError: the transfer never decoded, so no node ever existed.
    expect(validate(Uint8Array.of(0xff, 0xfe, 0xfd)).reason).toEqual({
      kind: "parserError",
    })
    // unsafeElement: foreignObject, named by its lower-cased local name.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="1" height="1"/></svg>`,
      ).reason,
    ).toEqual({ kind: "unsafeElement", name: "foreignobject" })
    // unsafeAttribute: an event handler, named without its prefix.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="alert(1)"/></svg>`,
      ).reason,
    ).toEqual({ kind: "unsafeAttribute", name: "onclick" })
    // unsafeAttribute: a namespaced href keeps the local name only.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="https://evil.example/x.svg#a"/></svg>`,
      ).reason,
    ).toEqual({ kind: "unsafeAttribute", name: "href" })
    // unsafeCss: a <style> body, named by the element that carried it.
    expect(
      validate(svgWithCss(`@import url(https://evil.example/x.css);`)).reason,
    ).toEqual({ kind: "unsafeCss", name: "style" })
    // unsafeCss: an attribute value that reached the CSS fallback, named by
    // the attribute that carried it.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url(https://evil.example/x)"/></svg>`,
      ).reason,
    ).toEqual({ kind: "unsafeCss", name: "fill" })
    // unsafeCss: a style attribute is CSS, so it is judged as CSS. It is not
    // scheme-checked, or `style="fill:red"` would read as a URI scheme.
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:url(https://evil.example/x)"/></svg>`,
      ).reason,
    ).toEqual({ kind: "unsafeCss", name: "style" })
    // unsafeProcessingInstruction: named by the pseudo-attribute that failed.
    expect(
      validate(
        `<?xml-stylesheet href="https://evil.example/x.css" type="text/css"?><svg xmlns="http://www.w3.org/2000/svg"/>`,
      ).reason,
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
      expect(validate(source)).toEqual({ ok: true, source })
    }

    // The whole shape Figma emits under svgIdAttribute, not just one attribute.
    const exported = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g id="Icon: Search"><path id="Vector: outline" d="M4 4h16v16H4z" fill="url(#gradient)"/></g><defs><linearGradient id="gradient: primary"><stop offset="0" stop-color="#fff"/></linearGradient></defs></svg>`
    expect(validate(exported)).toEqual({ ok: true, source: exported })
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
      expect(result.ok).toBe(false)
      expect(result.reason).toEqual({ kind: "unsafeAttribute", name })
    }

    // A namespaced href is still caught, and a fragment reference still passes.
    const namespaced = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="https://evil.example/x.svg#a"/></svg>`
    expect(validate(namespaced).reason).toEqual({
      kind: "unsafeAttribute",
      name: "href",
    })
    const fragment = `<svg xmlns="http://www.w3.org/2000/svg"><use href="#a"/></svg>`
    expect(validate(fragment)).toEqual({ ok: true, source: fragment })
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
      expect(result.ok).toBe(false)
      expect(result.reason).toEqual({ kind: "unsafeAttribute", name: "base" })
    }

    // A same-document base fetches nothing, so it passes the same rule href does.
    const fragment = `<svg xmlns="http://www.w3.org/2000/svg" xml:base="#a"><use href="#a"/></svg>`
    expect(validate(fragment)).toEqual({ ok: true, source: fragment })

    // A namespace declaration is an identifier, not a fetch, and stays exempt
    // even though its local name is now a reference attribute.
    const declaration = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:base="http://www.w3.org/1999/xlink"><rect/></svg>`
    expect(validate(declaration)).toEqual({ ok: true, source: declaration })
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
      expect(result.ok).toBe(false)
      expect(result.reason?.kind).toBe("unsafeCss")
    }
  })

  test("a size rejection carries no rule, and an accepted document carries none either", () => {
    const oversized = validate(
      `<svg xmlns="http://www.w3.org/2000/svg">${"a".repeat(MAX_SVG_BYTES)}`,
    )
    expect(oversized).toEqual({ ok: false, code: "LIMIT_EXCEEDED" })
    expect(oversized.reason).toBeUndefined()

    const safe = `<svg xmlns="http://www.w3.org/2000/svg"><image href="#ok"/></svg>`
    expect(validate(safe).reason).toBeUndefined()
  })

  test("an unusable offender name is omitted rather than sent oversized", () => {
    const huge = `on${"b".repeat(MAX_IDENTIFIER_BYTES)}`
    expect(huge.length).toBeGreaterThan(MAX_IDENTIFIER_BYTES)
    expect(
      validate(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect ${huge}="alert(1)"/></svg>`,
      ).reason,
    ).toEqual({ kind: "unsafeAttribute" })
  })

  test("validateDataUrl accepts a font payload at the byte ceiling", () => {
    const atLimit = Buffer.alloc(MAX_RASTER_DECODED_BYTES, 0x41).toString(
      "base64",
    )
    expect(validateDataUrl(`data:font/woff2;base64,${atLimit}`)).toBe(true)
  })
})
