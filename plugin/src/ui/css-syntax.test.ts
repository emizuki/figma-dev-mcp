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
