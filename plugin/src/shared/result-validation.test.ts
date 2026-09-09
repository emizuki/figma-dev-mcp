import { describe, expect, test } from "bun:test"

import { parseReadResult } from "./result-validation"

// `result-validation.ts` is the wall the plugin holds up against its own
// output: `exact()` refuses any field it was not told to expect, so a name
// missing from an `optional` list turns a valid result into a refusal and the
// session drops. Every test here feeds one result family a payload carrying
// every optional field that family can carry, and compares the whole parsed
// tree against the payload.
//
// The comparison is the point. `expect(parseReadResult(...)).toBeDefined()`
// proves only that the wall did not refuse; it passes unchanged when the
// parser accepts a field and then quietly drops it on the way out. `toEqual`
// against the payload fails in both directions.

const observation = {
  startedAt: "2026-09-07T00:00:00.000Z",
  completedAt: "2026-09-07T00:00:01.000Z",
}

/** A truncation carrying all three of its optional counters. */
const truncation = {
  reason: "nodeLimit",
  appliedDepth: 3,
  visitedNodes: 42,
  encodedBytes: 4096,
}

/** The metadata trailer every result family carries, `truncation` included. */
const trailer = { truncated: true, observation, truncation }

/** The `unresolved` list only the two forest results carry. */
const unresolved = [
  {
    id: "9:9",
    error: {
      code: "LIMIT_EXCEEDED",
      message: "The operation exceeded a safety limit.",
      retryable: false,
    },
  },
]

const solidPaint = {
  type: "solid",
  color: { r: 1, g: 0, b: 0, a: 1 },
  opacity: 1,
}

/** Every optional field `parseTextStyle` allows. */
const textStyle = {
  fontFamily: "Inter",
  fontStyle: "Regular",
  paints: [solidPaint],
  fontSize: 14,
  lineHeight: { unit: "pixels", value: 20 },
  letterSpacing: { unit: "percent", value: 2 },
  fontWeight: 300,
  textDecoration: "underline",
}

/** Every optional field `parseTextValue` allows. */
const textValue = {
  characters: "Save",
  defaultStyle: textStyle,
  styledRanges: [{ start: 0, end: 4, style: textStyle }],
  alignHorizontal: "justified",
  alignVertical: "center",
  autoResize: "height",
}

/** `componentSetId` is the optional here, and instances reuse the same shape. */
const componentValue = {
  componentId: "2:1",
  properties: [{ name: "Label", value: { kind: "text", value: "Save" } }],
  componentSetId: "2:0",
}

/** Every optional field `parseNodeSummary` allows. `childIds` must be
 * non-empty: an empty list is dropped rather than returned. */
const summary = (id: string): Record<string, unknown> => ({
  id,
  name: "Card",
  nodeType: "FRAME",
  parentId: "0:1",
  childIds: ["1:2"],
  bounds: { x: 0, y: 0, width: 10, height: 10 },
})

/** Every optional field `parseCompactData` allows. */
const compactData = {
  styleReferences: [{ id: "S:1", kind: "paint", name: "Brand" }],
  variableReferences: [{ id: "V:1", name: "Enabled" }],
  geometry: {
    rotation: 0,
    opacity: 1,
    transform: { m00: 1, m01: 0, m02: 0, m10: 0, m11: 1, m12: 0 },
    bounds: { x: 1, y: 2, width: 3, height: 4 },
  },
  constraints: { horizontal: "center", vertical: "stretch" },
  autoLayout: {
    mode: "horizontal",
    primarySizing: "fixed",
    counterSizing: "hug",
    gap: 8,
    paddingTop: 1,
    paddingRight: 2,
    paddingBottom: 3,
    paddingLeft: 4,
    primaryAlign: "spaceBetween",
    counterAlign: "baseline",
    wrap: true,
    counterAxisSpacing: 6,
  },
  // Compact detail carries a TextSummary here, not the TextValue full detail
  // carries. The two node-data shapes share six optional names and disagree on
  // what one of them holds.
  text: { characterCount: 4, preview: "Save" },
  component: componentValue,
  instance: componentValue,
}

/** `parseCompactData`'s optionals plus the five only full detail carries. */
const fullData = {
  ...compactData,
  text: textValue,
  paints: [solidPaint],
  effects: [
    {
      type: "dropShadow",
      color: { r: 0, g: 0, b: 0, a: 0.4 },
      offsetX: 1,
      offsetY: 2,
      radius: 3,
      spread: 0,
    },
  ],
  strokes: {
    paints: [solidPaint],
    weight: 2,
    align: "center",
    dashPattern: [4, 2],
  },
  cornerRadius: {
    kind: "perCorner",
    topLeft: 1,
    topRight: 2,
    bottomRight: 3,
    bottomLeft: 4,
  },
  cornerSmoothing: 0.6,
  clipsContent: true,
  blendMode: "multiply",
}

/** A node carrying `childrenTruncation`, the one optional on a design node. */
const node = (id: string, data: Record<string, unknown>) => ({
  summary: summary(id),
  data,
  children: [],
  childrenTruncated: true,
  childrenTruncation: truncation,
})

/** A batch failure carrying `items`, and an item carrying `id`. */
const failedItem = {
  status: "error",
  error: {
    code: "LIMIT_EXCEEDED",
    message: "The operation exceeded a safety limit.",
    retryable: false,
    items: [
      {
        index: 0,
        code: "NODE_NOT_FOUND",
        message: "The requested node was not found.",
        retryable: false,
        id: "8:1",
      },
    ],
  },
}

const survives = (operation: string, result: Record<string, unknown>): void => {
  const payload = { operation, result }
  const parsed: unknown = parseReadResult(payload)
  expect(parsed).toEqual(payload)
}

describe("the plugin's own result wall keeps every optional field it accepts", () => {
  test("a get_metadata result carrying every optional field survives parseReadResult", () => {
    survives("get_metadata", {
      file: { name: "File", editorType: "figma", key: "file-key" },
      pages: [{ id: "0:1", name: "Page" }],
      currentPageId: "0:1",
      pluginVersion: "0.1.0",
      // Every capability is `true`, so the wall dropping one is visible: the
      // parser defaults a missing capability to `false` rather than omitting
      // it, and an all-`false` set would slip past a laxer comparison.
      capabilities: {
        annotations: true,
        devResources: true,
        motion: true,
        svgStringExport: true,
        variableCodeSyntax: true,
      },
      ...trailer,
    })
  })

  test("a get_selection result carrying every optional field survives parseReadResult", () => {
    survives("get_selection", {
      detail: "full",
      nodes: [node("1:1", fullData)],
      unresolved,
      ...trailer,
    })
  })

  test("a get_nodes result carrying every optional field survives parseReadResult", () => {
    survives("get_nodes", {
      detail: "compact",
      items: [
        { status: "success", value: node("1:2", compactData) },
        failedItem,
      ],
      ...trailer,
    })
  })

  test("a search_nodes result carrying every optional field survives parseReadResult", () => {
    survives("search_nodes", {
      matches: [{ node: summary("1:3"), reasons: ["name"] }],
      nextCursor: "opaque-cursor",
      ...trailer,
    })
  })

  test("a get_design_context result carrying every optional field survives parseReadResult", () => {
    survives("get_design_context", {
      detail: "minimal",
      roots: [node("1:4", {})],
      unresolved,
      ...trailer,
    })
  })

  test("a get_styles result carrying every optional field survives parseReadResult", () => {
    // The identity fields are one shared list read at four separate call
    // sites, so all four style types carry them.
    survives("get_styles", {
      styles: [
        {
          styleType: "paint",
          id: "S:1",
          name: "Brand",
          description: "Brand fill",
          remote: true,
          key: "paint-key",
          paints: [solidPaint],
        },
        {
          styleType: "text",
          id: "S:2",
          name: "Body",
          description: "Body copy",
          remote: false,
          key: "text-key",
          text: textValue,
        },
        {
          styleType: "effect",
          id: "S:3",
          name: "Card shadow",
          description: "Elevation 1",
          remote: true,
          key: "effect-key",
          effects: [{ type: "layerBlur", radius: 4 }],
        },
        {
          styleType: "grid",
          id: "S:4",
          name: "Columns",
          description: "Twelve columns",
          remote: false,
          key: "grid-key",
          pattern: "columns",
          size: 12,
        },
      ],
      ...trailer,
    })
  })

  test("a get_variables result carrying every optional field survives parseReadResult", () => {
    survives("get_variables", {
      collections: [
        {
          id: "C:1",
          name: "Theme",
          modes: [{ id: "M:1", name: "Default" }],
          variables: [
            {
              id: "V:1",
              name: "Enabled",
              collectionId: "C:1",
              scopes: ["ALL_SCOPES"],
              values: [
                {
                  modeId: "M:1",
                  source: { kind: "alias", value: "V:2" },
                  resolved: { kind: "boolean", value: true },
                },
                {
                  modeId: "M:2",
                  source: { kind: "alias", value: "V:3" },
                  error: { code: "LIMIT_EXCEEDED", retryable: false },
                },
              ],
              codeSyntax: [{ platform: "WEB", code: "--enabled" }],
            },
          ],
        },
      ],
      ...trailer,
    })
  })

  test("a get_components result carrying every optional field survives parseReadResult", () => {
    survives("get_components", {
      components: [
        {
          id: "2:1",
          name: "Button",
          documentation: [{ uri: "https://example.com", label: "Docs" }],
          variantProperties: [{ name: "Size", value: "Large" }],
          propertyDefinitions: [
            {
              name: "Label",
              defaultValue: { kind: "text", value: "Save" },
              // A non-empty list: an empty one is dropped rather than kept.
              preferredValues: [{ kind: "instanceSwap", value: "2:9" }],
            },
          ],
          componentSetId: "2:0",
          description: "Primary button",
        },
      ],
      instances: [{ instanceId: "2:2", componentId: "2:1" }],
      ...trailer,
    })
  })

  test("a get_fonts result carrying every optional field survives parseReadResult", () => {
    survives("get_fonts", {
      fonts: [
        {
          font: { family: "Inter", style: "Regular" },
          availability: "available",
          nodeIds: ["3:1"],
        },
      ],
      ...trailer,
    })
  })

  test("a get_dev_mode_data result carrying every optional field survives parseReadResult", () => {
    survives("get_dev_mode_data", {
      items: [
        {
          status: "success",
          value: {
            nodeId: "4:1",
            annotations: [{ id: "A:1", text: "Note", categoryId: "AC:1" }],
            annotationCategories: [{ id: "AC:1", label: "Spec" }],
            documentation: [{ name: "Guide", uri: "https://example.com" }],
            devResources: [{ name: "Repo", uri: "https://example.com/repo" }],
            description: "Card",
            descriptionMarkdown: "**Card**",
            ownerNodeId: "4:0",
            inheritedFromNodeId: "4:2",
          },
        },
      ],
      visitedNodes: 1,
      ...trailer,
    })
  })

  test("a get_reactions result carrying every optional field survives parseReadResult", () => {
    survives("get_reactions", {
      items: [
        {
          status: "success",
          value: {
            nodeId: "5:1",
            reactions: [
              {
                trigger: "click",
                action: { type: "navigate", destinationId: "5:2" },
                destinationAccessible: true,
                transitionId: "T:1",
                transitionDuration: 300,
                overlay: {
                  relativePosition: { x: 1, y: 2 },
                  positionType: "manual",
                  background: {
                    type: "solidColor",
                    color: { r: 0, g: 0, b: 0, a: 0.5 },
                  },
                  backgroundInteraction: "closeOnClickOutside",
                },
                timeout: 1000,
                delay: 250,
                keyCodes: [13],
                device: "iPhone 15",
                mediaHitTime: 12.5,
              },
              {
                trigger: "press",
                action: { type: "setVariable", variableId: "V:1" },
                destinationAccessible: false,
              },
              {
                trigger: "hover",
                action: {
                  type: "setVariableMode",
                  variableCollectionId: "C:1",
                  variableModeId: "M:1",
                },
                destinationAccessible: false,
              },
              {
                trigger: "mediaHit",
                action: {
                  type: "updateMediaRuntime",
                  mediaAction: "skipTo",
                  destinationId: "5:3",
                  amountToSkip: 5,
                  newTimestamp: 10,
                },
                destinationAccessible: false,
              },
            ],
          },
        },
      ],
      visitedNodes: 1,
      ...trailer,
    })
  })

  test("a get_motion result carrying every optional field survives parseReadResult", () => {
    survives("get_motion", {
      items: [
        {
          status: "success",
          value: {
            nodeId: "6:1",
            animationStyles: [
              {
                id: "AS:1",
                styleId: "S:9",
                name: "Fade",
                duration: 0.4,
                timelineOffset: 0.1,
                props: [
                  { name: "opacity", value: 1 },
                  {
                    name: "easing",
                    value: {
                      type: "CUSTOM_SPRING",
                      easingFunctionSpring: { bounce: 0.3 },
                    },
                  },
                ],
              },
            ],
            animations: [
              {
                field: {
                  type: "indexedItem",
                  collection: "fills",
                  index: 0,
                  field: "color",
                  propertyId: "P:1",
                },
                baseValue: { type: "FLOAT", value: 0 },
                timelineDuration: 1,
                tracks: [
                  {
                    id: "TR:1",
                    keyframeOperation: "SET",
                    keyframes: [
                      {
                        id: "KF:1",
                        timelinePosition: 0.5,
                        value: {
                          type: "COLOR",
                          value: { r: 1, g: 1, b: 1, a: 1 },
                        },
                        easing: {
                          type: "CUSTOM_CUBIC_BEZIER",
                          easingFunctionCubicBezier: {
                            x1: 0,
                            y1: 0,
                            x2: 1,
                            y2: 1,
                          },
                        },
                      },
                    ],
                  },
                ],
              },
            ],
            manualKeyframeTracks: [
              {
                field: { type: "property", name: "opacity" },
                id: "MT:1",
                baseValue: { type: "BOOL", value: true },
                keyframes: [
                  {
                    id: "KF:2",
                    timelinePosition: 0,
                    value: { type: "TEXT_DATA", value: "hi" },
                    easing: { type: "LINEAR" },
                  },
                ],
              },
            ],
            timelines: [{ id: "TL:1", duration: 2 }],
          },
        },
      ],
      visitedNodes: 1,
      // A non-empty list: an empty one is dropped rather than kept.
      availableStyles: [
        {
          styleId: "S:9",
          name: "Fade",
          description: "Fade in",
          props: [{ name: "duration", value: "0.4" }],
        },
      ],
      ...trailer,
    })
  })

  test("a get_screenshot result carrying every optional field survives parseReadResult", () => {
    survives("get_screenshot", {
      assets: [
        {
          status: "success",
          value: {
            format: "svg",
            nodeId: "7:1",
            source: "<svg/>",
            safe: true,
          },
        },
        {
          status: "success",
          value: {
            format: "svg",
            nodeId: "7:2",
            source: "<svg><script/></svg>",
            safe: false,
            rejection: { kind: "unsafeElement", name: "script" },
          },
        },
      ],
      ...trailer,
    })
  })
})
