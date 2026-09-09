import { describe, expect, test } from "bun:test"

import { tokenizeCss } from "./css-syntax"

function kinds(input: string): string[] {
  return tokenizeCss(input).map((token) => token.kind)
}

describe("CSS finite-state tokenizer", () => {
  test("does not detect CSS constructs with regular expressions", async () => {
    const source = await Bun.file(
      new URL("./css-syntax.ts", import.meta.url),
    ).text()
    expect(source).not.toContain("RegExp")
    expect(source).not.toMatch(/\/(?:import|url|javascript)\//)
  })

  test("understands comments, strings, escapes, at-keywords, functions, and URL tokens", () => {
    const tokens = tokenizeCss(
      '/* @import url(https://hidden.example); */ @import url(https://evil.example); color: url("#ok"); content: "url(https://string.example)"; bg: url("https://quoted.example"); ident\\:fn(1);',
    )
    const interesting = tokens.filter(
      (token) =>
        token.kind === "at-keyword" ||
        token.kind === "url" ||
        token.kind === "function" ||
        token.kind === "string" ||
        token.kind === "comment",
    )
    expect(interesting).toEqual([
      { kind: "comment", value: " @import url(https://hidden.example); " },
      { kind: "at-keyword", value: "import" },
      { kind: "url", value: "https://evil.example" },
      { kind: "url", value: "#ok" },
      { kind: "string", value: "url(https://string.example)" },
      { kind: "url", value: "https://quoted.example" },
      { kind: "function", value: "ident:fn" },
    ])
    expect(kinds("url(#clip)")).toContain("url")
  })

  test("does not treat a URL written only inside a string as a url token", () => {
    const tokens = tokenizeCss('content: "url(https://not-a-token.example)"')
    expect(tokens.some((token) => token.kind === "url")).toBe(false)
    expect(
      tokens.some(
        (token) =>
          token.kind === "string" &&
          token.value === "url(https://not-a-token.example)",
      ),
    ).toBe(true)
  })

  test("does not treat @import inside a comment as an at-keyword", () => {
    const tokens = tokenizeCss(
      "/* @import url(https://hidden.example); */ body { color: red; }",
    )
    expect(tokens.some((token) => token.kind === "at-keyword")).toBe(false)
  })
})

// The tokenizer is what `validateCssText` reads with, so a construct it stops
// recognising is a construct the SVG safety rules stop seeing. Only five of
// its eleven token kinds and none of its number, escape or ident entry rules
// were pinned.
describe("CSS tokenizer acceptance", () => {
  test("produces every token kind it declares", () => {
    const cases: Array<[string, ReturnType<typeof tokenizeCss>]> = [
      ["/*note*/", [{ kind: "comment", value: "note" }]],
      [" \t\n ", [{ kind: "whitespace" }]],
      ['"text"', [{ kind: "string", value: "text" }]],
      ["'text'", [{ kind: "string", value: "text" }]],
      ["url(#a)", [{ kind: "url", value: "#a" }]],
      ["rgb(", [{ kind: "function", value: "rgb" }]],
      ["@media", [{ kind: "at-keyword", value: "media" }]],
      ["color", [{ kind: "ident", value: "color" }]],
      ["#ff0000", [{ kind: "hash", value: "ff0000" }]],
      ["42", [{ kind: "number", value: "42" }]],
      ["*", [{ kind: "delim", value: "*" }]],
    ]
    const seen = new Set<string>()
    for (const [input, expected] of cases) {
      expect({ input, tokens: tokenizeCss(input) }).toEqual({
        input,
        tokens: expected,
      })
      for (const token of expected) seen.add(token.kind)
    }
    // Positive control, and the check that the list above is complete: every
    // kind in the `CssToken` union is produced by one of these inputs.
    expect([...seen].sort()).toEqual([
      "at-keyword",
      "comment",
      "delim",
      "function",
      "hash",
      "ident",
      "number",
      "string",
      "url",
      "whitespace",
    ])
  })

  test("a bare # or @ is a delimiter carrying its own character", () => {
    expect(tokenizeCss("#")).toEqual([{ kind: "delim", value: "#" }])
    expect(tokenizeCss("@")).toEqual([{ kind: "delim", value: "@" }])
    expect(tokenizeCss("# @ ;")).toEqual([
      { kind: "delim", value: "#" },
      { kind: "whitespace" },
      { kind: "delim", value: "@" },
      { kind: "whitespace" },
      { kind: "delim", value: ";" },
    ])
  })

  test("URL tokens come from url( in any casing, quoted or bare", () => {
    expect(tokenizeCss("URL(#a)")).toEqual([{ kind: "url", value: "#a" }])
    expect(tokenizeCss("Url(#a)")).toEqual([{ kind: "url", value: "#a" }])
    expect(tokenizeCss("url('#a')")).toEqual([{ kind: "url", value: "#a" }])
    expect(tokenizeCss('url("#a")')).toEqual([{ kind: "url", value: "#a" }])
    // A bare value ends at whitespace, and the whitespace is not part of it.
    expect(tokenizeCss("url( #a )")).toEqual([{ kind: "url", value: "#a" }])
    // Four letters is not `url`, so it stays an ordinary function.
    expect(tokenizeCss("urls(#a)")).toEqual([
      { kind: "function", value: "urls" },
      { kind: "hash", value: "a" },
      { kind: "delim", value: ")" },
    ])
  })

  test("numbers are recognised with a sign, a leading dot, a fraction and an exponent", () => {
    const numbers = [
      "42",
      "+42",
      "-42",
      ".5",
      "+.5",
      "-.5",
      "1.25",
      "1e3",
      "1E3",
      "1e+3",
      "1e-3",
      "1.25e-3",
    ]
    let checked = 0
    for (const source of numbers) {
      expect({ source, tokens: tokenizeCss(source) }).toEqual({
        source,
        tokens: [{ kind: "number", value: source }],
      })
      checked += 1
    }
    expect(checked).toBe(numbers.length)
    expect(checked).toBe(12)
  })

  test("identifiers start with a hyphen, a double hyphen or a non-ASCII letter", () => {
    expect(tokenizeCss("-webkit-mask")).toEqual([
      { kind: "ident", value: "-webkit-mask" },
    ])
    expect(tokenizeCss("--brand-blue")).toEqual([
      { kind: "ident", value: "--brand-blue" },
    ])
    expect(tokenizeCss("größe")).toEqual([{ kind: "ident", value: "größe" }])
    // A hyphen that starts nothing is a delimiter, not the head of an ident.
    expect(tokenizeCss("- ")).toEqual([
      { kind: "delim", value: "-" },
      { kind: "whitespace" },
    ])
  })

  test("escapes resolve to the character they name", () => {
    // A hexadecimal escape takes up to six digits and swallows one following
    // space as its terminator; the space is the delimiter, not content.
    expect(tokenizeCss("\\41 BC")).toEqual([{ kind: "ident", value: "ABC" }])
    expect(tokenizeCss("\\000041BC")).toEqual([{ kind: "ident", value: "ABC" }])
    expect(tokenizeCss("\\1F600 ")).toEqual([{ kind: "ident", value: "😀" }])
    // A non-hexadecimal escape is the literal character.
    expect(tokenizeCss("a\\:b")).toEqual([{ kind: "ident", value: "a:b" }])
    // Inside a string, a backslash before a newline is a line continuation and
    // contributes nothing at all.
    expect(tokenizeCss('"ab\\\ncd"')).toEqual([
      { kind: "string", value: "abcd" },
    ])
    // Outside a string it is not a valid escape, so it does not start an ident.
    expect(tokenizeCss("\\\na")).toEqual([
      { kind: "delim", value: "\\" },
      { kind: "whitespace" },
      { kind: "ident", value: "a" },
    ])
  })
})
