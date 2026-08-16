function hexByte(value: number): string {
  return value.toString(16).padStart(2, "0")
}

function fromBytes(bytes: Uint8Array): string {
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80
  const hex = Array.from(bytes, hexByte).join("")
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

export function randomUuid(): string {
  const webCrypto = globalThis.crypto
  try {
    if (typeof webCrypto?.randomUUID === "function") {
      return webCrypto.randomUUID()
    }
  } catch {
    // Figma's iframe may expose a non-callable randomUUID.
  }
  if (typeof webCrypto?.getRandomValues === "function") {
    return fromBytes(webCrypto.getRandomValues(new Uint8Array(16)))
  }
  const bytes = new Uint8Array(16)
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Math.floor(Math.random() * 256)
  }
  return fromBytes(bytes)
}
