export type CssToken =
  | { kind: "comment"; value: string }
  | { kind: "whitespace" }
  | { kind: "string"; value: string }
  | { kind: "url"; value: string }
  | { kind: "function"; value: string }
  | { kind: "at-keyword"; value: string }
  | { kind: "ident"; value: string }
  | { kind: "hash"; value: string }
  | { kind: "number"; value: string }
  | { kind: "delim"; value: string }

const TAB = 0x09
const LF = 0x0a
const FF = 0x0c
const CR = 0x0d
const SPACE = 0x20
const QUOTE = 0x22
const HASH = 0x23
const APOSTROPHE = 0x27
const LPAREN = 0x28
const RPAREN = 0x29
const PLUS = 0x2b
const COMMA = 0x2c
const MINUS = 0x2d
const DOT = 0x2e
const SLASH = 0x2f
const COLON = 0x3a
const SEMICOLON = 0x3b
const AT = 0x40
const BACKSLASH = 0x5c
const UNDERSCORE = 0x5f
const LBRACKET = 0x5b
const RBRACKET = 0x5d
const LBRACE = 0x7b
const RBRACE = 0x7d
const DEL = 0x7f

function isWhitespace(code: number): boolean {
  return (
    code === TAB || code === LF || code === FF || code === CR || code === SPACE
  )
}

function isDigit(code: number): boolean {
  return code >= 0x30 && code <= 0x39
}

function isHex(code: number): boolean {
  return (
    isDigit(code) ||
    (code >= 0x41 && code <= 0x46) ||
    (code >= 0x61 && code <= 0x66)
  )
}

function isNameStart(code: number): boolean {
  return (
    (code >= 0x41 && code <= 0x5a) ||
    (code >= 0x61 && code <= 0x7a) ||
    code === UNDERSCORE ||
    code >= 0x80
  )
}

function isName(code: number): boolean {
  return isNameStart(code) || isDigit(code) || code === MINUS
}

function isNewline(code: number): boolean {
  return code === LF || code === FF || code === CR
}

export function tokenizeCss(input: string): CssToken[] {
  const tokens: CssToken[] = []
  let index = 0

  const codeAt = (offset: number): number => {
    if (offset >= input.length) return -1
    return input.charCodeAt(offset)
  }

  const isValidEscape = (offset: number): boolean => {
    return codeAt(offset) === BACKSLASH && !isNewline(codeAt(offset + 1))
  }

  const startsIdent = (offset: number): boolean => {
    const first = codeAt(offset)
    if (first === MINUS) {
      const second = codeAt(offset + 1)
      return (
        isNameStart(second) || second === MINUS || isValidEscape(offset + 1)
      )
    }
    return isNameStart(first) || isValidEscape(offset)
  }

  const startsNumber = (offset: number): boolean => {
    const first = codeAt(offset)
    if (isDigit(first)) return true
    if (first === DOT) return isDigit(codeAt(offset + 1))
    if (first === PLUS || first === MINUS) {
      const second = codeAt(offset + 1)
      if (isDigit(second)) return true
      return second === DOT && isDigit(codeAt(offset + 2))
    }
    return false
  }

  const consumeEscape = (): string => {
    index += 1
    const first = codeAt(index)
    if (first < 0) return "\uFFFD"
    if (isHex(first)) {
      let hex = ""
      for (let count = 0; count < 6 && isHex(codeAt(index)); count += 1) {
        hex += input[index]
        index += 1
      }
      if (isWhitespace(codeAt(index))) index += 1
      const value = Number.parseInt(hex, 16)
      if (
        value === 0 ||
        value > 0x10ffff ||
        (value >= 0xd800 && value <= 0xdfff)
      ) {
        return "\uFFFD"
      }
      return String.fromCodePoint(value)
    }
    if (isNewline(first)) {
      if (first === CR && codeAt(index + 1) === LF) index += 1
      index += 1
      return ""
    }
    index += 1
    return input[index - 1] ?? "\uFFFD"
  }

  const consumeIdent = (): string => {
    let value = ""
    while (index < input.length) {
      const code = codeAt(index)
      if (isName(code)) {
        value += input[index]
        index += 1
        continue
      }
      if (isValidEscape(index)) {
        value += consumeEscape()
        continue
      }
      break
    }
    return value
  }

  const consumeString = (ending: number): string => {
    index += 1
    let value = ""
    while (index < input.length) {
      const code = codeAt(index)
      if (code === ending) {
        index += 1
        break
      }
      if (isNewline(code)) break
      if (code === BACKSLASH) {
        if (index + 1 >= input.length) {
          index += 1
          break
        }
        if (isNewline(codeAt(index + 1))) {
          if (codeAt(index + 1) === CR && codeAt(index + 2) === LF) index += 1
          index += 2
          continue
        }
        value += consumeEscape()
        continue
      }
      value += input[index]
      index += 1
    }
    return value
  }

  const consumeWhitespace = (): void => {
    while (isWhitespace(codeAt(index))) index += 1
  }

  const consumeUrl = (): CssToken => {
    consumeWhitespace()
    const next = codeAt(index)
    if (next === QUOTE || next === APOSTROPHE) {
      const value = consumeString(next)
      consumeWhitespace()
      if (codeAt(index) === RPAREN) index += 1
      return { kind: "url", value }
    }
    let value = ""
    while (index < input.length) {
      const code = codeAt(index)
      if (code === RPAREN) {
        index += 1
        break
      }
      if (isWhitespace(code)) {
        consumeWhitespace()
        if (codeAt(index) === RPAREN) index += 1
        break
      }
      if (
        code === QUOTE ||
        code === APOSTROPHE ||
        code === LPAREN ||
        code === DEL ||
        (code >= 0 && code < SPACE)
      ) {
        value += input[index]
        index += 1
        continue
      }
      if (isValidEscape(index)) {
        value += consumeEscape()
        continue
      }
      value += input[index]
      index += 1
    }
    return { kind: "url", value }
  }

  const consumeNumber = (): string => {
    const start = index
    const first = codeAt(index)
    if (first === PLUS || first === MINUS) index += 1
    while (isDigit(codeAt(index))) index += 1
    if (codeAt(index) === DOT && isDigit(codeAt(index + 1))) {
      index += 1
      while (isDigit(codeAt(index))) index += 1
    }
    const exponent = codeAt(index)
    if (
      (exponent === 0x45 || exponent === 0x65) &&
      (isDigit(codeAt(index + 1)) ||
        ((codeAt(index + 1) === PLUS || codeAt(index + 1) === MINUS) &&
          isDigit(codeAt(index + 2))))
    ) {
      index += 1
      if (codeAt(index) === PLUS || codeAt(index) === MINUS) index += 1
      while (isDigit(codeAt(index))) index += 1
    }
    return input.slice(start, index)
  }

  while (index < input.length) {
    const code = codeAt(index)
    if (code === SLASH && codeAt(index + 1) === 0x2a) {
      index += 2
      const start = index
      while (
        index + 1 < input.length &&
        !(codeAt(index) === 0x2a && codeAt(index + 1) === SLASH)
      ) {
        index += 1
      }
      const value = input.slice(start, index)
      if (index + 1 < input.length) index += 2
      tokens.push({ kind: "comment", value })
      continue
    }
    if (isWhitespace(code)) {
      consumeWhitespace()
      tokens.push({ kind: "whitespace" })
      continue
    }
    if (code === QUOTE || code === APOSTROPHE) {
      tokens.push({ kind: "string", value: consumeString(code) })
      continue
    }
    if (code === HASH) {
      index += 1
      if (isName(codeAt(index)) || isValidEscape(index)) {
        tokens.push({ kind: "hash", value: consumeIdent() })
      } else {
        tokens.push({ kind: "delim", value: "#" })
      }
      continue
    }
    if (code === AT) {
      index += 1
      if (startsIdent(index)) {
        tokens.push({ kind: "at-keyword", value: consumeIdent() })
      } else {
        tokens.push({ kind: "delim", value: "@" })
      }
      continue
    }
    if (startsIdent(index)) {
      const name = consumeIdent()
      if (codeAt(index) === LPAREN) {
        index += 1
        if (name.length === 3) {
          const a = name.charCodeAt(0) | 32
          const b = name.charCodeAt(1) | 32
          const c = name.charCodeAt(2) | 32
          if (a === 0x75 && b === 0x72 && c === 0x6c) {
            tokens.push(consumeUrl())
            continue
          }
        }
        tokens.push({ kind: "function", value: name })
        continue
      }
      tokens.push({ kind: "ident", value: name })
      continue
    }
    if (startsNumber(index)) {
      tokens.push({ kind: "number", value: consumeNumber() })
      continue
    }
    index += 1
    if (
      code === LPAREN ||
      code === RPAREN ||
      code === LBRACKET ||
      code === RBRACKET ||
      code === LBRACE ||
      code === RBRACE ||
      code === COLON ||
      code === SEMICOLON ||
      code === COMMA
    ) {
      tokens.push({ kind: "delim", value: input[index - 1] ?? "" })
      continue
    }
    tokens.push({ kind: "delim", value: input[index - 1] ?? "" })
  }

  return tokens
}
