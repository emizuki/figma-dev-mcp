import { readdirSync, readFileSync, statSync } from "node:fs"
import { join, relative } from "node:path"
import { describe, expect, test } from "bun:test"
import * as ts from "typescript"

const pluginRoot = join(import.meta.dir, "../..")
const sourceRoot = join(pluginRoot, "src")

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

function isProductionSource(path: string): boolean {
  return (
    path.endsWith(".ts") &&
    !path.endsWith(".test.ts") &&
    !path.endsWith("environment.typecheck.ts")
  )
}

function collectFiles(directory: string): string[] {
  const files: string[] = []
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry)
    if (statSync(path).isDirectory()) {
      files.push(...collectFiles(path))
    } else if (isProductionSource(path)) {
      files.push(path)
    }
  }
  return files
}

function productionSource(): { path: string; text: string }[] {
  return collectFiles(sourceRoot).map((path) => ({
    path,
    text: readFileSync(path, "utf8"),
  }))
}

describe("plugin source policy", () => {
  test("rejects mutation, private, and Motion write APIs", () => {
    const files = productionSource()
    expect(files.length).toBeGreaterThan(0)
    for (const file of files) {
      for (const forbidden of MUTATION_DENYLIST) {
        expect(
          file.text.includes(forbidden),
          `${relative(pluginRoot, file.path)} contains ${forbidden}`,
        ).toBe(false)
      }
    }
  })

  test("rejects AnyKeyword so untrusted values stay unknown until guarded", () => {
    const files = productionSource()
    const hits: string[] = []
    for (const file of files) {
      const source = ts.createSourceFile(
        file.path,
        file.text,
        ts.ScriptTarget.ES2022,
        true,
        ts.ScriptKind.TS,
      )
      const visit = (node: ts.Node): void => {
        if (node.kind === ts.SyntaxKind.AnyKeyword) {
          const { line, character } = source.getLineAndCharacterOfPosition(
            node.getStart(source),
          )
          hits.push(
            `${relative(pluginRoot, file.path)}:${line + 1}:${character + 1}`,
          )
        }
        ts.forEachChild(node, visit)
      }
      visit(source)
    }
    expect(hits).toEqual([])
  })
})
