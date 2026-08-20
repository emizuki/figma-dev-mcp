import { describe, expect, test } from "bun:test"
import { DOMParser, onErrorStopParsing } from "@xmldom/xmldom"

import { MAX_SVG_BYTES } from "../shared/limits"
import { validateSvgSource } from "./svg"

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
    expect(validate(script)).toEqual({ ok: false, code: "UNSAFE_SVG" })
    expect(validate(external)).toEqual({ ok: false, code: "UNSAFE_SVG" })
    expect(validate(nested)).toEqual({ ok: false, code: "UNSAFE_SVG" })

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
      expect(validate(source)).toEqual({ ok: false, code: "UNSAFE_SVG" })
    }
  })

  test("rejects invalid UTF-8 transfer and oversized source", () => {
    expect(validate(Uint8Array.of(0xff, 0xfe, 0xfd))).toEqual({
      ok: false,
      code: "UNSAFE_SVG",
    })
    expect(validate("\uD800")).toEqual({ ok: false, code: "UNSAFE_SVG" })
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
      expect(validate(source)).toEqual({ ok: false, code: "UNSAFE_SVG" })
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
      svgWithCss(`@font-face{src:url(data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)}`),
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
})
