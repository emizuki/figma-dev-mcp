import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

const pluginRoot = join(import.meta.dir, "../..")
const dist = join(pluginRoot, "dist")

const MUTATION_DENYLIST = [
  "loadAllPagesAsync",
  "loadFontAsync",
  "setCurrentPageAsync",
  "setRangeFontName",
  "installFont",
  "substituteFont",
  "importComponentByKeyAsync",
  "importComponentSetByKeyAsync",
  "addComponentProperty",
  "editComponentProperty",
  "deleteComponentProperty",
  "setProperties(",
  "setPluginData",
  "setRelaunchData",
  "createRectangle",
  "createFrame",
  "createText",
  "applyAnimationStyle",
  "removeAnimationStyle",
  "applyManualKeyframeTrack",
  "removeManualKeyframeTrack",
  "setTimelineDuration",
] as const

describe("plugin bundle policy", () => {
  test("scans production dist for the reviewed mutation denylist", () => {
    const mainPath = join(dist, "code.js")
    const uiPath = join(dist, "index.html")
    expect(existsSync(mainPath)).toBe(true)
    expect(existsSync(uiPath)).toBe(true)
    const main = readFileSync(mainPath, "utf8")
    const ui = readFileSync(uiPath, "utf8")
    expect(main.length).toBeGreaterThan(0)
    expect(ui.length).toBeGreaterThan(0)
    expect(main.includes("WebSocket")).toBe(false)
    expect(ui.includes("figma.")).toBe(false)
    expect(main.includes("get_css")).toBe(false)
    expect(main.includes("get_tokens")).toBe(false)
    for (const forbidden of MUTATION_DENYLIST) {
      expect(
        main.includes(forbidden),
        `dist/code.js contains ${forbidden}`,
      ).toBe(false)
      expect(
        ui.includes(forbidden),
        `dist/index.html contains ${forbidden}`,
      ).toBe(false)
    }
  })
})
