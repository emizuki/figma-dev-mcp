import { describe, expect, test } from "bun:test"

import {
  OPERATION_NAMES,
  parseBrokerToPlugin,
  parseControllerBoundMessage,
  parseControllerOutboundMessage,
  parsePluginToBroker,
  parseUuid,
} from "./validation"

const fixture = async (name: string): Promise<unknown> =>
  Bun.file(
    new URL(`../../../tests/contracts/fixtures/${name}`, import.meta.url),
  ).json()

const observation = { startedAt: "s", completedAt: "e" }
const controllerRequestId = "123e4567-e89b-42d3-a456-426614174000"

const resultMessage = (operation: string, result: unknown): unknown => ({
  type: "response",
  controllerRequestId,
  requestId: "plugin-1",
  result: { operation, result },
})

const designNode = (
  id: string,
  data: Record<string, unknown>,
  children: unknown[] = [],
): Record<string, unknown> => ({
  summary: { id, name: "Node", nodeType: "FRAME", visible: true },
  data,
  children,
  childrenTruncated: false,
})

describe("closed plugin transport validation", () => {
  test("Rust fixtures decode and re-encode without shape drift", async () => {
    for (const name of ["get_metadata_request.json", "cancel.json"]) {
      const source = await fixture(name)
      const parsed: unknown = parseBrokerToPlugin(source)
      expect(parsed).toEqual(source)
    }

    for (const name of [
      "hello.json",
      "get_metadata_response.json",
      "error.json",
    ]) {
      const source = await fixture(name)
      const parsed: unknown = parsePluginToBroker(source)
      expect(parsed).toEqual(source)
    }

    expect(parsePluginToBroker({ type: "pong", nonce: 7 })).toEqual({
      type: "pong",
      nonce: 7,
    })
    expect(
      parseControllerOutboundMessage({
        type: "progress",
        controllerRequestId,
        requestId: "plugin-1",
        completed: 1,
        total: 2,
        message: "Reading",
      }),
    ).toEqual({
      type: "progress",
      controllerRequestId,
      requestId: "plugin-1",
      completed: 1,
      total: 2,
      message: "Reading",
    })
  })

  test("normalizes Rust-defaulted capability fields", () => {
    expect(
      parseControllerOutboundMessage({
        type: "controllerReady",
        metadataRequestId: "123e4567-e89b-42d3-a456-426614174000",
        fileName: "File",
        currentPage: { id: "0:1", name: "Page" },
        editorType: "dev",
        pluginVersion: "0.1.0",
        capabilities: {},
      }),
    ).toEqual({
      type: "controllerReady",
      metadataRequestId: "123e4567-e89b-42d3-a456-426614174000",
      fileName: "File",
      currentPage: { id: "0:1", name: "Page" },
      editorType: "dev",
      pluginVersion: "0.1.0",
      capabilities: {
        annotations: false,
        devResources: false,
        motion: false,
        svgStringExport: false,
        variableCodeSyntax: false,
      },
    })

    const message = resultMessage("get_metadata", {
      file: { key: "file", name: "File", editorType: "dev" },
      pages: [],
      currentPageId: "0:1",
      pluginVersion: "0.1.0",
      capabilities: {},
      truncated: false,
      observation,
    })
    const parsed: unknown = parseControllerOutboundMessage(message)
    expect(parsed).toEqual(
      resultMessage("get_metadata", {
        file: { key: "file", name: "File", editorType: "dev" },
        pages: [],
        currentPageId: "0:1",
        pluginVersion: "0.1.0",
        capabilities: {
          annotations: false,
          devResources: false,
          motion: false,
          svgStringExport: false,
          variableCodeSyntax: false,
        },
        truncated: false,
        observation,
      }),
    )
  })

  test("accepts production metadata without the private file key", () => {
    const message = resultMessage("get_metadata", {
      file: { name: "File", editorType: "dev" },
      pages: [],
      currentPageId: "0:1",
      pluginVersion: "0.1.0",
      capabilities: {
        annotations: false,
        devResources: false,
        motion: false,
        svgStringExport: false,
        variableCodeSyntax: false,
      },
      truncated: false,
      observation,
    })

    const parsed: unknown = parseControllerOutboundMessage(message)
    expect(parsed).toEqual(message)
  })

  test("operation catalog is exact, closed, and excludes removed operations", () => {
    expect(OPERATION_NAMES).toEqual([
      "get_metadata",
      "get_selection",
      "get_nodes",
      "search_nodes",
      "get_design_context",
      "get_styles",
      "get_variables",
      "get_components",
      "get_fonts",
      "get_dev_mode_data",
      "get_reactions",
      "get_motion",
      "get_screenshot",
    ])
    expect(OPERATION_NAMES).toHaveLength(13)
    expect(OPERATION_NAMES).not.toContain("get_css")
    expect(OPERATION_NAMES).not.toContain("get_tokens")
  })

  test("rejects unknown tags, operations, write shapes, and arbitrary methods", () => {
    expect(() => parseBrokerToPlugin({ type: "other" })).toThrow()
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: { operation: "get_css", input: {} },
      }),
    ).toThrow()
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: { operation: "get_tokens", input: {} },
      }),
    ).toThrow()
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: { operation: "set_fills", input: {} },
      }),
    ).toThrow()
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "get_metadata",
          input: {},
          method: "createRectangle",
        },
      }),
    ).toThrow()
  })

  test("rejects blank request IDs, malformed UUIDs, and non-finite numbers", () => {
    expect(() =>
      parseBrokerToPlugin({ type: "cancel", requestId: "" }),
    ).toThrow()
    expect(() => parseUuid("not-a-uuid")).toThrow()
    expect(parseUuid("123e4567-e89b-42d3-a456-426614174000")).toBe(
      "123e4567-e89b-42d3-a456-426614174000",
    )
    expect(() =>
      parseBrokerToPlugin({ type: "ping", nonce: Number.NaN }),
    ).toThrow()
    expect(() =>
      parseControllerBoundMessage({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: { operation: "get_metadata", input: {} },
      }),
    ).toThrow()
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "get_screenshot",
          input: {
            format: "png",
            selector: { nodeId: "1:2" },
            scale: Number.POSITIVE_INFINITY,
          },
        },
      }),
    ).toThrow()
  })

  test("normalizes skipped empty search node types like Rust serialization", () => {
    expect(
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "search_nodes",
          input: { scope: { pageId: "0:1" }, query: { nodeTypes: [] } },
        },
      }),
    ).toEqual({
      type: "request",
      requestId: "plugin-1",
      deadlineMs: 1,
      target: {},
      operation: {
        operation: "search_nodes",
        input: { scope: { pageId: "0:1" }, query: {} },
      },
    })
  })

  test("decodes object search terms and rejects bare string predicates", () => {
    expect(
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "search_nodes",
          input: {
            scope: { pageId: "0:1" },
            query: {
              name: { value: "Card", mode: "exact", caseSensitive: true },
              text: { value: "Pay", mode: "contains" },
            },
          },
        },
      }),
    ).toEqual({
      type: "request",
      requestId: "plugin-1",
      deadlineMs: 1,
      target: {},
      operation: {
        operation: "search_nodes",
        input: {
          scope: { pageId: "0:1" },
          query: {
            name: { value: "Card", mode: "exact", caseSensitive: true },
            text: { value: "Pay", mode: "contains" },
          },
        },
      },
    })
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "search_nodes",
          input: { scope: { pageId: "0:1" }, query: { name: "Card" } },
        },
      }),
    ).toThrow()
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "search_nodes",
          input: {
            scope: { pageId: "0:1" },
            query: { text: { value: "Pay", mode: "fuzzy" } },
          },
        },
      }),
    ).toThrow()
  })

  test("get_variables mode values accept item-level alias errors", () => {
    const common = {
      truncated: false,
      observation: { startedAt: "s", completedAt: "e" },
    }
    const mode = (
      modeId: string,
      code: "NODE_NOT_FOUND" | "LIMIT_EXCEEDED",
    ) => ({
      modeId,
      source: { kind: "alias", value: "V:other" },
      error: { code, retryable: false },
    })
    expect(
      parseControllerOutboundMessage(
        resultMessage("get_variables", {
          collections: [
            {
              id: "C:theme",
              name: "Theme",
              modes: [
                { id: "M:missing", name: "Missing" },
                { id: "M:cycle", name: "Cycle" },
              ],
              variables: [
                {
                  id: "V:broken",
                  name: "broken",
                  collectionId: "C:theme",
                  scopes: [],
                  values: [
                    mode("M:missing", "NODE_NOT_FOUND"),
                    mode("M:cycle", "LIMIT_EXCEEDED"),
                  ],
                  codeSyntax: [],
                },
              ],
            },
          ],
          ...common,
        }),
      ),
    ).toMatchObject({
      result: {
        operation: "get_variables",
        result: {
          collections: [
            {
              variables: [
                {
                  values: [
                    {
                      modeId: "M:missing",
                      error: { code: "NODE_NOT_FOUND", retryable: false },
                    },
                    {
                      modeId: "M:cycle",
                      error: { code: "LIMIT_EXCEEDED", retryable: false },
                    },
                  ],
                },
              ],
            },
          ],
        },
      },
    })
    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_variables", {
          collections: [
            {
              id: "C:theme",
              name: "Theme",
              modes: [],
              variables: [
                {
                  id: "V:broken",
                  name: "broken",
                  collectionId: "C:theme",
                  scopes: [],
                  values: [
                    {
                      modeId: "M:default",
                      source: { kind: "alias", value: "V:other" },
                      leftover: true,
                    },
                  ],
                  codeSyntax: [],
                },
              ],
            },
          ],
          ...common,
        }),
      ),
    ).toThrow(/unknown field leftover/)
  })

  test("get_styles source defaults to both and accepts local or referenced", () => {
    const request = (input: Record<string, unknown>) => ({
      type: "request" as const,
      requestId: "plugin-1",
      deadlineMs: 1,
      target: {},
      operation: { operation: "get_styles" as const, input },
    })

    expect(parseBrokerToPlugin(request({}))).toMatchObject({
      operation: { operation: "get_styles", input: { source: "both" } },
    })
    expect(parseBrokerToPlugin(request({ source: "local" }))).toMatchObject({
      operation: { operation: "get_styles", input: { source: "local" } },
    })
    expect(
      parseBrokerToPlugin(request({ source: "referenced" })),
    ).toMatchObject({
      operation: { operation: "get_styles", input: { source: "referenced" } },
    })
    expect(() => parseBrokerToPlugin(request({ source: "all" }))).toThrow(
      /source/,
    )
  })

  test("trims search node types and rejects whitespace-only types", () => {
    expect(
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "search_nodes",
          input: {
            scope: { pageId: "0:1" },
            query: { nodeTypes: ["FRAME "] },
          },
        },
      }),
    ).toEqual({
      type: "request",
      requestId: "plugin-1",
      deadlineMs: 1,
      target: {},
      operation: {
        operation: "search_nodes",
        input: {
          scope: { pageId: "0:1" },
          query: { nodeTypes: ["FRAME"] },
        },
      },
    })
    expect(() =>
      parseBrokerToPlugin({
        type: "request",
        requestId: "plugin-1",
        deadlineMs: 1,
        target: {},
        operation: {
          operation: "search_nodes",
          input: {
            scope: { pageId: "0:1" },
            query: { nodeTypes: ["   "] },
          },
        },
      }),
    ).toThrow(/non-empty/)
  })

  test("rejects arbitrary, nested-unknown, and mixed-detail result payloads", () => {
    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_metadata", { anything: "accepted" }),
      ),
    ).toThrow()

    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_metadata", {
          file: { key: "file", name: "File", editorType: "dev" },
          pages: [{ id: "0:1", name: "Page", unexpected: true }],
          currentPageId: "0:1",
          capabilities: {
            annotations: false,
            devResources: false,
            motion: false,
            svgStringExport: true,
            variableCodeSyntax: false,
          },
          truncated: false,
          observation: { startedAt: "s", completedAt: "e" },
        }),
      ),
    ).toThrow()

    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_selection", {
          detail: "minimal",
          nodes: [
            {
              summary: {
                id: "1:2",
                name: "Frame",
                nodeType: "FRAME",
                visible: true,
              },
              data: { geometry: {} },
              children: [],
              childrenTruncated: false,
            },
          ],
          truncated: false,
          observation: { startedAt: "s", completedAt: "e" },
        }),
      ),
    ).toThrow()

    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_styles", {
          styles: [
            {
              styleType: "paint",
              id: "style-1",
              name: "Paint",
              paints: [],
              effects: [],
            },
          ],
          truncated: false,
          observation,
        }),
      ),
    ).toThrow()

    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_screenshot", {
          assets: [
            {
              status: "success",
              value: {
                format: "svg",
                nodeId: "1:2",
                source: "<svg/>",
                width: 1,
              },
            },
          ],
          truncated: false,
          observation,
        }),
      ),
    ).toThrow()
  })

  test("rejects oversized screenshot bytes and impossible raster dimensions", () => {
    const screenshot = (value: Record<string, unknown>): unknown =>
      resultMessage("get_screenshot", {
        assets: [{ status: "success", value }],
        truncated: false,
        observation,
      })

    expect(() =>
      parseControllerOutboundMessage(
        screenshot({
          format: "png",
          nodeId: "7:1",
          dataBase64: "A".repeat(16 * 1_024 * 1_024 + 1),
          width: 1,
          height: 1,
        }),
      ),
    ).toThrow()
    expect(() =>
      parseControllerOutboundMessage(
        screenshot({
          format: "svg",
          nodeId: "7:1",
          source: "x".repeat(4 * 1_024 * 1_024 + 1),
        }),
      ),
    ).toThrow()
    expect(() =>
      parseControllerOutboundMessage(
        screenshot({
          format: "png",
          nodeId: "7:1",
          dataBase64: "AA==",
          width: 4_097,
          height: 1,
        }),
      ),
    ).toThrow()
    expect(() =>
      parseControllerOutboundMessage(
        screenshot({
          format: "png",
          nodeId: "7:1",
          dataBase64: "AA==",
          width: 4_001,
          height: 4_000,
        }),
      ),
    ).toThrow()
  })

  test("accepts exact concrete payloads for all 13 Rust result families", () => {
    const common = { truncated: false, observation }
    const validResults: ReadonlyArray<readonly [string, unknown]> = [
      [
        "get_metadata",
        {
          file: { key: "file", name: "File", editorType: "dev" },
          pages: [{ id: "0:1", name: "Page" }],
          currentPageId: "0:1",
          pluginVersion: "0.1.0",
          capabilities: {
            annotations: false,
            devResources: false,
            motion: false,
            svgStringExport: true,
            variableCodeSyntax: false,
          },
          ...common,
        },
      ],
      [
        "get_selection",
        {
          detail: "minimal",
          nodes: [designNode("1:1", {})],
          ...common,
        },
      ],
      [
        "get_nodes",
        {
          detail: "compact",
          items: [
            {
              status: "success",
              value: designNode("1:2", {
                styleReferences: [],
                variableReferences: [],
              }),
            },
          ],
          ...common,
        },
      ],
      ["search_nodes", { matches: [], ...common }],
      [
        "get_design_context",
        {
          detail: "full",
          roots: [
            designNode("1:3", {
              paints: [],
              effects: [],
              styleReferences: [],
              variableReferences: [],
            }),
          ],
          ...common,
        },
      ],
      [
        "get_styles",
        {
          styles: [
            {
              styleType: "paint",
              id: "style-1",
              name: "Brand",
              description: "Brand fill",
              remote: false,
              key: "paint-key",
              paints: [
                {
                  type: "solid",
                  color: { r: 1, g: 0, b: 0, a: 1 },
                  opacity: 1,
                },
              ],
            },
          ],
          ...common,
        },
      ],
      [
        "get_variables",
        {
          collections: [
            {
              id: "collection-1",
              name: "Theme",
              modes: [{ id: "mode-1", name: "Default" }],
              variables: [
                {
                  id: "variable-1",
                  name: "Enabled",
                  collectionId: "collection-1",
                  scopes: [],
                  values: [
                    {
                      modeId: "mode-1",
                      source: { kind: "boolean", value: true },
                    },
                  ],
                  codeSyntax: [],
                },
              ],
            },
          ],
          ...common,
        },
      ],
      [
        "get_components",
        {
          components: [
            {
              id: "2:1",
              name: "Button",
              documentation: [],
              variantProperties: [],
              propertyDefinitions: [],
            },
          ],
          instances: [{ instanceId: "2:2", componentId: "2:1" }],
          ...common,
        },
      ],
      [
        "get_fonts",
        {
          fonts: [
            {
              font: { family: "Inter", style: "Regular" },
              availability: "available",
              nodeIds: ["3:1"],
            },
          ],
          ...common,
        },
      ],
      [
        "get_dev_mode_data",
        {
          items: [
            {
              status: "success",
              value: {
                nodeId: "4:1",
                annotations: [],
                annotationCategories: [],
                documentation: [],
                devResources: [],
              },
            },
          ],
          ...common,
        },
      ],
      [
        "get_reactions",
        {
          items: [
            {
              status: "success",
              value: {
                nodeId: "5:1",
                reactions: [
                  {
                    trigger: "click",
                    action: { type: "openLink", uri: "https://example.com" },
                    destinationAccessible: true,
                  },
                ],
              },
            },
          ],
          ...common,
        },
      ],
      [
        "get_motion",
        {
          items: [
            {
              status: "success",
              value: {
                nodeId: "6:1",
                animationStyles: [],
                animations: [],
                manualKeyframeTracks: [],
                timelines: [],
              },
            },
          ],
          ...common,
        },
      ],
      [
        "get_screenshot",
        {
          assets: [
            {
              status: "success",
              value: { format: "svg", nodeId: "7:1", source: "<svg/>" },
            },
          ],
          ...common,
        },
      ],
    ]

    expect(validResults.map(([operation]) => operation)).toEqual([
      ...OPERATION_NAMES,
    ])
    for (const [operation, result] of validResults) {
      const message = resultMessage(operation, result)
      const parsed: unknown = parseControllerOutboundMessage(message)
      expect(parsed).toEqual(message)
    }
  })

  test("rejects recursive results beyond depth and global node budgets", () => {
    let node: Record<string, unknown> = {
      summary: {
        id: "1:0",
        name: "Node",
        nodeType: "FRAME",
        visible: true,
      },
      data: {},
      children: [],
      childrenTruncated: false,
    }
    for (let depth = 1; depth <= 7; depth += 1) {
      node = {
        summary: {
          id: `1:${depth}`,
          name: "Node",
          nodeType: "FRAME",
          visible: true,
        },
        data: {},
        children: [node],
        childrenTruncated: false,
      }
    }
    expect(() =>
      parseControllerOutboundMessage({
        type: "response",
        controllerRequestId,
        requestId: "plugin-1",
        result: {
          operation: "get_selection",
          result: {
            detail: "minimal",
            nodes: [node],
            truncated: false,
            observation: { startedAt: "s", completedAt: "e" },
          },
        },
      }),
    ).toThrow()

    const wideRoots = [
      designNode(
        "2:1",
        {},
        Array.from({ length: 1_000 }, (_, index) =>
          designNode(`2:${index + 2}`, {}),
        ),
      ),
      designNode(
        "3:1",
        {},
        Array.from({ length: 1_000 }, (_, index) =>
          designNode(`3:${index + 2}`, {}),
        ),
      ),
    ]
    expect(() =>
      parseControllerOutboundMessage(
        resultMessage("get_selection", {
          detail: "minimal",
          nodes: wideRoots,
          truncated: false,
          observation,
        }),
      ),
    ).toThrow()
  })

  test("progress counters are bounded to Rust u32", () => {
    expect(
      parseControllerOutboundMessage({
        type: "progress",
        controllerRequestId,
        requestId: "plugin-1",
        completed: 4_294_967_295,
        total: 4_294_967_295,
      }),
    ).toEqual({
      type: "progress",
      controllerRequestId,
      requestId: "plugin-1",
      completed: 4_294_967_295,
      total: 4_294_967_295,
    })
    expect(() =>
      parseControllerOutboundMessage({
        type: "progress",
        controllerRequestId,
        requestId: "plugin-1",
        completed: 4_294_967_296,
      }),
    ).toThrow()
    expect(() =>
      parseControllerOutboundMessage({
        type: "progress",
        controllerRequestId,
        requestId: "plugin-1",
        completed: 0,
        total: 4_294_967_296,
      }),
    ).toThrow()
  })
})
