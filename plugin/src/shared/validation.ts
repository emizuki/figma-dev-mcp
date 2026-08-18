import {
  ERROR_CODES,
  OPERATION_NAMES,
  assertNever,
  type BrokerToPlugin,
  type CapabilitySet,
  type ControllerBoundMessage,
  type ControllerMetadataRequest,
  type ControllerOutboundMessage,
  type DetailLevel,
  type ErrorCode,
  type GetDesignContextInput,
  type GetMotionInput,
  type GetNodesInput,
  type GetScreenshotInput,
  type GetSelectionInput,
  type GetStylesInput,
  type GetVariablesInput,
  type StyleSource,
  type PluginFailure,
  type PluginToBroker,
  type ReadOperation,
  type ScopedInput,
  type SearchMatchMode,
  type SearchNodesInput,
  type Selector,
} from "./protocol"
import {
  MAX_DEPTH,
  MAX_DISPLAY_TEXT_BYTES,
  MAX_IDENTIFIER_BYTES,
  MAX_INPUT_IDS,
  MAX_PAGE_IDS,
  MAX_QUERY_BYTES,
  MAX_RETURNED_NODES,
  MAX_SEARCH_CURSOR_BYTES,
} from "./limits"
import { parseReadResult, parseU32 } from "./result-validation"

export { OPERATION_NAMES }

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function utf8ByteLength(value: string): number {
  let bytes = 0
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index)
    if (code <= 0x7f) {
      bytes += 1
    } else if (code <= 0x7ff) {
      bytes += 2
    } else if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1)
      if (next >= 0xdc00 && next <= 0xdfff) index += 1
      bytes += next >= 0xdc00 && next <= 0xdfff ? 4 : 3
    } else {
      bytes += 3
    }
  }
  return bytes
}

function fail(message: string): never {
  throw new TypeError(`Invalid plugin protocol: ${message}`)
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return fail(`${label} must be an object`)
  }
  return value as Record<string, unknown>
}

function exact(
  value: unknown,
  label: string,
  required: readonly string[],
  optional: readonly string[] = [],
): Record<string, unknown> {
  const object = record(value, label)
  const allowed = new Set([...required, ...optional])
  for (const key of Object.keys(object)) {
    if (!allowed.has(key)) fail(`${label} contains unknown field ${key}`)
  }
  for (const key of required) {
    if (!Object.hasOwn(object, key)) fail(`${label} is missing ${key}`)
  }
  return object
}

function boundedString(
  value: unknown,
  label: string,
  maximumBytes: number,
  allowEmpty = false,
): string {
  if (typeof value !== "string") return fail(`${label} must be a string`)
  if (!allowEmpty && value.length === 0)
    return fail(`${label} must not be blank`)
  if (utf8ByteLength(value) > maximumBytes) {
    return fail(`${label} exceeds ${maximumBytes} UTF-8 bytes`)
  }
  return value
}

function identifier(value: unknown, label: string): string {
  return boundedString(value, label, MAX_IDENTIFIER_BYTES)
}

function displayText(value: unknown, label: string): string {
  return boundedString(value, label, MAX_DISPLAY_TEXT_BYTES, true)
}

function boolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") return fail(`${label} must be boolean`)
  return value
}

function finiteNumber(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fail(`${label} must be finite`)
  }
  return value
}

function unsignedInteger(value: unknown, label: string): number {
  const parsed = finiteNumber(value, label)
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    return fail(`${label} must be a non-negative safe integer`)
  }
  return parsed
}

function optionalIdentifier(
  object: Record<string, unknown>,
  key: string,
): string | undefined {
  if (!Object.hasOwn(object, key)) return undefined
  return identifier(object[key], key)
}

function stringList(value: unknown, label: string, maximum: number): string[] {
  if (!Array.isArray(value)) return fail(`${label} must be an array`)
  if (value.length > maximum) return fail(`${label} exceeds ${maximum} items`)
  return value.map((item, index) => identifier(item, `${label}[${index}]`))
}

function detailLevel(value: unknown): DetailLevel {
  switch (value) {
    case "minimal":
    case "compact":
    case "full":
      return value
    default:
      return fail("detail must be minimal, compact, or full")
  }
}

function optionalDepth(object: Record<string, unknown>): number | undefined {
  if (!Object.hasOwn(object, "depth")) return undefined
  const depth = unsignedInteger(object.depth, "depth")
  if (depth > MAX_DEPTH) return fail(`depth exceeds ${MAX_DEPTH}`)
  return depth
}

function parseCapabilitySet(value: unknown): CapabilitySet {
  const fields = [
    "annotations",
    "devResources",
    "motion",
    "svgStringExport",
    "variableCodeSyntax",
  ] as const
  const object = exact(value, "capabilities", [], fields)
  return {
    annotations: Object.hasOwn(object, "annotations")
      ? boolean(object.annotations, "annotations")
      : false,
    devResources: Object.hasOwn(object, "devResources")
      ? boolean(object.devResources, "devResources")
      : false,
    motion: Object.hasOwn(object, "motion")
      ? boolean(object.motion, "motion")
      : false,
    svgStringExport: Object.hasOwn(object, "svgStringExport")
      ? boolean(object.svgStringExport, "svgStringExport")
      : false,
    variableCodeSyntax: Object.hasOwn(object, "variableCodeSyntax")
      ? boolean(object.variableCodeSyntax, "variableCodeSyntax")
      : false,
  }
}

function parseSelector(value: unknown, pageLimit = MAX_PAGE_IDS): Selector {
  const object = record(value, "selector")
  const keys = Object.keys(object)
  if (keys.length !== 1) return fail("selector must contain exactly one field")
  switch (keys[0]) {
    case "selection":
      if (object.selection !== true)
        return fail("selection must be literal true")
      return { selection: true }
    case "pageId":
      return { pageId: identifier(object.pageId, "pageId") }
    case "pageIds":
      return { pageIds: stringList(object.pageIds, "pageIds", pageLimit) }
    case "nodeId":
      return { nodeId: identifier(object.nodeId, "nodeId") }
    case "nodeIds":
      return { nodeIds: stringList(object.nodeIds, "nodeIds", MAX_INPUT_IDS) }
    default:
      return fail("selector contains an unknown field")
  }
}

function parseFileScope(
  value: unknown,
  label: string,
  optional: readonly string[] = [],
): Record<string, unknown> {
  return exact(value, label, [], ["connectionId", ...optional])
}

function copyFileScope(object: Record<string, unknown>): {
  connectionId?: string
} {
  const connectionId = optionalIdentifier(object, "connectionId")
  return connectionId === undefined ? {} : { connectionId }
}

function parseScopedInput(value: unknown, label: string): ScopedInput {
  const object = parseFileScope(value, label, ["selector"])
  const base = copyFileScope(object)
  if (!Object.hasOwn(object, "selector")) return base
  return { ...base, selector: parseSelector(object.selector) }
}

function parseGetSelectionInput(value: unknown): GetSelectionInput {
  const object = parseFileScope(value, "get_selection input", [
    "detail",
    "depth",
  ])
  const result: GetSelectionInput = copyFileScope(object)
  if (Object.hasOwn(object, "detail"))
    result.detail = detailLevel(object.detail)
  const depth = optionalDepth(object)
  if (depth !== undefined) result.depth = depth
  return result
}

function parseGetNodesInput(value: unknown): GetNodesInput {
  const object = exact(
    value,
    "get_nodes input",
    ["nodeIds"],
    ["connectionId", "detail", "depth"],
  )
  const result: GetNodesInput = {
    ...copyFileScope(object),
    nodeIds: stringList(object.nodeIds, "nodeIds", MAX_INPUT_IDS),
  }
  if (Object.hasOwn(object, "detail"))
    result.detail = detailLevel(object.detail)
  const depth = optionalDepth(object)
  if (depth !== undefined) result.depth = depth
  return result
}

function parseSearchMatchMode(value: unknown, label: string): SearchMatchMode {
  if (value === "exact" || value === "contains") return value
  return fail(`${label} must be exact or contains`)
}

function parseSearchNodesInput(value: unknown): SearchNodesInput {
  const object = exact(
    value,
    "search_nodes input",
    ["scope", "match", "limit"],
    ["connectionId", "query", "types", "cursor"],
  )
  const scope = parseSelector(object.scope)
  if (!("pageId" in scope) && !("nodeId" in scope)) {
    return fail("search scope must contain exactly one pageId or nodeId")
  }
  const result: SearchNodesInput = {
    ...copyFileScope(object),
    scope,
    match: parseSearchMatchMode(object.match, "match"),
    limit: unsignedInteger(object.limit, "limit"),
  }
  if (result.limit < 1 || result.limit > MAX_RETURNED_NODES)
    return fail(`limit must be between 1 and ${MAX_RETURNED_NODES}`)
  if (Object.hasOwn(object, "query")) {
    const query = boundedString(
      object.query,
      "query",
      MAX_QUERY_BYTES,
      true,
    ).trim()
    if (query.length === 0)
      return fail("query must be non-empty after trimming")
    result.query = query
  }
  if (Object.hasOwn(object, "types")) {
    const types = stringList(object.types, "types", MAX_INPUT_IDS).map(
      (type, index) => {
        const trimmed = type.trim()
        if (trimmed.length === 0)
          return fail(`types[${index}] must be non-empty after trimming`)
        return trimmed
      },
    )
    if (types.length > 0) result.types = types
  }
  if (result.query === undefined && result.types === undefined)
    return fail("search must include query or types")
  if (Object.hasOwn(object, "cursor"))
    result.cursor = boundedString(
      object.cursor,
      "cursor",
      MAX_SEARCH_CURSOR_BYTES,
    ).trim()
  return result
}

function parseGetDesignContextInput(value: unknown): GetDesignContextInput {
  const object = parseFileScope(value, "get_design_context input", [
    "selector",
    "depth",
    "detail",
    "includeHidden",
    "dedupeComponents",
  ])
  const result: GetDesignContextInput = {
    ...copyFileScope(object),
    includeHidden: Object.hasOwn(object, "includeHidden")
      ? boolean(object.includeHidden, "includeHidden")
      : false,
    dedupeComponents: Object.hasOwn(object, "dedupeComponents")
      ? boolean(object.dedupeComponents, "dedupeComponents")
      : false,
  }
  if (Object.hasOwn(object, "selector"))
    result.selector = parseSelector(object.selector)
  if (Object.hasOwn(object, "detail"))
    result.detail = detailLevel(object.detail)
  const depth = optionalDepth(object)
  if (depth !== undefined) result.depth = depth
  return result
}

function parseStyleSource(value: unknown): StyleSource {
  if (value === "local" || value === "referenced" || value === "both") {
    return value
  }
  return fail("source must be local, referenced, or both")
}

function parseGetStylesInput(value: unknown): GetStylesInput {
  const object = parseFileScope(value, "get_styles input", [
    "selector",
    "source",
  ])
  const scoped = parseScopedInput(
    Object.fromEntries(
      Object.entries(object).filter(([key]) => key !== "source"),
    ),
    "get_styles input",
  )
  return {
    ...scoped,
    source: Object.hasOwn(object, "source")
      ? parseStyleSource(object.source)
      : "both",
  }
}

function parseGetVariablesInput(value: unknown): GetVariablesInput {
  const object = parseFileScope(value, "get_variables input", [
    "selector",
    "resolveAliases",
  ])
  const scoped = parseScopedInput(
    Object.fromEntries(
      Object.entries(object).filter(([key]) => key !== "resolveAliases"),
    ),
    "get_variables input",
  )
  return {
    ...scoped,
    resolveAliases: Object.hasOwn(object, "resolveAliases")
      ? boolean(object.resolveAliases, "resolveAliases")
      : false,
  }
}

function parseGetMotionInput(value: unknown): GetMotionInput {
  const object = parseFileScope(value, "get_motion input", [
    "selector",
    "includeAvailableStyles",
  ])
  const scoped = parseScopedInput(
    Object.fromEntries(
      Object.entries(object).filter(
        ([key]) => key !== "includeAvailableStyles",
      ),
    ),
    "get_motion input",
  )
  return {
    ...scoped,
    includeAvailableStyles: Object.hasOwn(object, "includeAvailableStyles")
      ? boolean(object.includeAvailableStyles, "includeAvailableStyles")
      : false,
  }
}

function parseScreenshotInput(value: unknown): GetScreenshotInput {
  const object = record(value, "get_screenshot input")
  const format = object.format
  if (format === "png" || format === "jpeg") {
    const raster = exact(
      object,
      "raster screenshot input",
      ["format", "selector"],
      ["connectionId", "scale"],
    )
    const result: GetScreenshotInput = {
      format,
      ...copyFileScope(raster),
      selector: parseScreenshotSelector(raster.selector),
    }
    if (Object.hasOwn(raster, "scale")) {
      const scale = finiteNumber(raster.scale, "scale")
      if (scale < 0.25 || scale > 4)
        return fail("scale must be between 0.25 and 4")
      result.scale = scale
    }
    return result
  }
  if (format === "svg") {
    const svg = exact(
      object,
      "SVG screenshot input",
      ["format", "selector"],
      ["connectionId", "svgOutlineText", "svgIdAttribute", "svgSimplifyStroke"],
    )
    return {
      format,
      ...copyFileScope(svg),
      selector: parseScreenshotSelector(svg.selector),
      svgOutlineText: Object.hasOwn(svg, "svgOutlineText")
        ? boolean(svg.svgOutlineText, "svgOutlineText")
        : true,
      svgIdAttribute: Object.hasOwn(svg, "svgIdAttribute")
        ? boolean(svg.svgIdAttribute, "svgIdAttribute")
        : false,
      svgSimplifyStroke: Object.hasOwn(svg, "svgSimplifyStroke")
        ? boolean(svg.svgSimplifyStroke, "svgSimplifyStroke")
        : true,
    }
  }
  return fail("screenshot format must be png, jpeg, or svg")
}

function parseScreenshotSelector(
  value: unknown,
): Selector &
  ({ selection: true } | { nodeId: string } | { nodeIds: string[] }) {
  const selector = parseSelector(value)
  if (
    "selection" in selector ||
    "nodeId" in selector ||
    "nodeIds" in selector
  ) {
    return selector
  }
  return fail("screenshot selector must be selection, nodeId, or nodeIds")
}

function parseReadOperation(value: unknown): ReadOperation {
  const object = exact(value, "read operation", ["operation", "input"])
  switch (object.operation) {
    case "get_metadata":
      return {
        operation: "get_metadata",
        input: copyFileScope(
          parseFileScope(object.input, "get_metadata input"),
        ),
      }
    case "get_selection":
      return {
        operation: "get_selection",
        input: parseGetSelectionInput(object.input),
      }
    case "get_nodes":
      return { operation: "get_nodes", input: parseGetNodesInput(object.input) }
    case "search_nodes":
      return {
        operation: "search_nodes",
        input: parseSearchNodesInput(object.input),
      }
    case "get_design_context":
      return {
        operation: "get_design_context",
        input: parseGetDesignContextInput(object.input),
      }
    case "get_styles":
      return {
        operation: "get_styles",
        input: parseGetStylesInput(object.input),
      }
    case "get_variables":
      return {
        operation: "get_variables",
        input: parseGetVariablesInput(object.input),
      }
    case "get_components":
      return {
        operation: "get_components",
        input: parseScopedInput(object.input, "get_components input"),
      }
    case "get_fonts":
      return {
        operation: "get_fonts",
        input: parseScopedInput(object.input, "get_fonts input"),
      }
    case "get_dev_mode_data":
      return {
        operation: "get_dev_mode_data",
        input: parseScopedInput(object.input, "get_dev_mode_data input"),
      }
    case "get_reactions":
      return {
        operation: "get_reactions",
        input: parseScopedInput(object.input, "get_reactions input"),
      }
    case "get_motion":
      return {
        operation: "get_motion",
        input: parseGetMotionInput(object.input),
      }
    case "get_screenshot":
      return {
        operation: "get_screenshot",
        input: parseScreenshotInput(object.input),
      }
    default:
      return fail("unknown or non-read operation")
  }
}

function errorCode(value: unknown): ErrorCode {
  if (typeof value !== "string") return fail("error code must be a string")
  for (const code of ERROR_CODES) if (value === code) return code
  return fail("unknown error code")
}

function parsePluginFailure(value: unknown): PluginFailure {
  const object = exact(
    value,
    "plugin failure",
    ["code", "retryable"],
    ["items"],
  )
  const result: PluginFailure = {
    code: errorCode(object.code),
    retryable: boolean(object.retryable, "retryable"),
  }
  if (Object.hasOwn(object, "items")) {
    if (!Array.isArray(object.items))
      return fail("error items must be an array")
    if (object.items.length > MAX_INPUT_IDS) return fail("too many error items")
    result.items = object.items.map((value, itemIndex) => {
      const item = exact(
        value,
        `error item ${itemIndex}`,
        ["index", "code", "retryable"],
        ["id"],
      )
      const index = unsignedInteger(item.index, "index")
      const id = optionalIdentifier(item, "id")
      const code = errorCode(item.code)
      const retryable = boolean(item.retryable, "retryable")
      return id === undefined
        ? { index, code, retryable }
        : { index, id, code, retryable }
    })
  }
  return result
}

export function parseBrokerToPlugin(value: unknown): BrokerToPlugin {
  const object = record(value, "broker message")
  switch (object.type) {
    case "request": {
      const request = exact(object, "request", [
        "type",
        "requestId",
        "deadlineMs",
        "target",
        "operation",
      ])
      const target = exact(request.target, "request target", [], ["fileKey"])
      const fileKey = optionalIdentifier(target, "fileKey")
      return {
        type: "request",
        requestId: identifier(request.requestId, "requestId"),
        deadlineMs: unsignedInteger(request.deadlineMs, "deadlineMs"),
        target: fileKey === undefined ? {} : { fileKey },
        operation: parseReadOperation(request.operation),
      }
    }
    case "cancel": {
      const cancel = exact(object, "cancel", ["type", "requestId"])
      return {
        type: "cancel",
        requestId: identifier(cancel.requestId, "requestId"),
      }
    }
    case "ping": {
      const ping = exact(object, "ping", ["type", "nonce"])
      return { type: "ping", nonce: unsignedInteger(ping.nonce, "nonce") }
    }
    default:
      return fail("unknown broker message tag")
  }
}

export function parseControllerBoundMessage(
  value: unknown,
): ControllerBoundMessage {
  const object = record(value, "controller-bound message")
  switch (object.type) {
    case "request": {
      const request = exact(object, "controller request", [
        "type",
        "controllerRequestId",
        "requestId",
        "deadlineMs",
        "target",
        "operation",
      ])
      const target = exact(
        request.target,
        "controller request target",
        [],
        ["fileKey"],
      )
      const fileKey = optionalIdentifier(target, "fileKey")
      return {
        type: "request",
        controllerRequestId: parseUuid(request.controllerRequestId),
        requestId: identifier(request.requestId, "requestId"),
        deadlineMs: unsignedInteger(request.deadlineMs, "deadlineMs"),
        target: fileKey === undefined ? {} : { fileKey },
        operation: parseReadOperation(request.operation),
      }
    }
    case "cancel": {
      const cancel = exact(object, "controller cancel", [
        "type",
        "controllerRequestId",
        "requestId",
      ])
      return {
        type: "cancel",
        controllerRequestId: parseUuid(cancel.controllerRequestId),
        requestId: identifier(cancel.requestId, "requestId"),
      }
    }
    default:
      return fail("unknown controller-bound message tag")
  }
}

export function parseControllerOutboundMessage(
  value: unknown,
): ControllerOutboundMessage {
  const object = record(value, "controller message")
  switch (object.type) {
    case "controllerReady": {
      const ready = exact(object, "controllerReady", [
        "type",
        "metadataRequestId",
        "fileName",
        "currentPage",
        "editorType",
        "pluginVersion",
        "capabilities",
      ])
      const currentPage = exact(ready.currentPage, "currentPage", [
        "id",
        "name",
      ])
      return {
        type: "controllerReady",
        metadataRequestId: parseUuid(ready.metadataRequestId),
        fileName: displayText(ready.fileName, "fileName"),
        currentPage: {
          id: identifier(currentPage.id, "currentPage.id"),
          name: displayText(currentPage.name, "currentPage.name"),
        },
        editorType: displayText(ready.editorType, "editorType"),
        pluginVersion: identifier(ready.pluginVersion, "pluginVersion"),
        capabilities: parseCapabilitySet(ready.capabilities),
      }
    }
    case "progress": {
      const progress = exact(
        object,
        "progress",
        ["type", "controllerRequestId", "requestId", "completed"],
        ["total", "message"],
      )
      const result: Extract<ControllerOutboundMessage, { type: "progress" }> = {
        type: "progress",
        controllerRequestId: parseUuid(progress.controllerRequestId),
        requestId: identifier(progress.requestId, "requestId"),
        completed: parseU32(progress.completed, "completed"),
      }
      if (Object.hasOwn(progress, "total"))
        result.total = parseU32(progress.total, "total")
      if (Object.hasOwn(progress, "message")) {
        result.message = displayText(progress.message, "message")
      }
      return result
    }
    case "response": {
      const response = exact(object, "response", [
        "type",
        "controllerRequestId",
        "requestId",
        "result",
      ])
      return {
        type: "response",
        controllerRequestId: parseUuid(response.controllerRequestId),
        requestId: identifier(response.requestId, "requestId"),
        result: parseReadResult(response.result),
      }
    }
    case "error": {
      const error = exact(object, "error", [
        "type",
        "controllerRequestId",
        "requestId",
        "error",
      ])
      return {
        type: "error",
        controllerRequestId: parseUuid(error.controllerRequestId),
        requestId: identifier(error.requestId, "requestId"),
        error: parsePluginFailure(error.error),
      }
    }
    default:
      return fail("unknown controller message tag")
  }
}

export function parsePluginToBroker(value: unknown): PluginToBroker {
  const object = record(value, "plugin message")
  switch (object.type) {
    case "hello": {
      const hello = exact(
        object,
        "hello",
        [
          "type",
          "protocolVersion",
          "connectionId",
          "displayName",
          "fileName",
          "currentPage",
          "editorType",
          "pluginVersion",
          "capabilities",
        ],
        ["fileKey"],
      )
      const currentPage = exact(hello.currentPage, "currentPage", [
        "id",
        "name",
      ])
      const fileKey = optionalIdentifier(hello, "fileKey")
      const result: Extract<PluginToBroker, { type: "hello" }> = {
        type: "hello",
        protocolVersion: identifier(hello.protocolVersion, "protocolVersion"),
        connectionId: parseUuid(hello.connectionId),
        displayName: displayText(hello.displayName, "displayName"),
        fileName: displayText(hello.fileName, "fileName"),
        currentPage: {
          id: identifier(currentPage.id, "currentPage.id"),
          name: displayText(currentPage.name, "currentPage.name"),
        },
        editorType: displayText(hello.editorType, "editorType"),
        pluginVersion: displayText(hello.pluginVersion, "pluginVersion"),
        capabilities: parseCapabilitySet(hello.capabilities),
      }
      if (fileKey !== undefined) result.fileKey = fileKey
      return result
    }
    case "progress": {
      const progress = exact(
        object,
        "progress",
        ["type", "requestId", "completed"],
        ["total", "message"],
      )
      const result: Extract<PluginToBroker, { type: "progress" }> = {
        type: "progress",
        requestId: identifier(progress.requestId, "requestId"),
        completed: parseU32(progress.completed, "completed"),
      }
      if (Object.hasOwn(progress, "total"))
        result.total = parseU32(progress.total, "total")
      if (Object.hasOwn(progress, "message"))
        result.message = displayText(progress.message, "message")
      return result
    }
    case "response": {
      const response = exact(object, "response", [
        "type",
        "requestId",
        "result",
      ])
      return {
        type: "response",
        requestId: identifier(response.requestId, "requestId"),
        result: parseReadResult(response.result),
      }
    }
    case "error": {
      const error = exact(object, "error", ["type", "requestId", "error"])
      return {
        type: "error",
        requestId: identifier(error.requestId, "requestId"),
        error: parsePluginFailure(error.error),
      }
    }
    case "pong": {
      const pong = exact(object, "pong", ["type", "nonce"])
      return { type: "pong", nonce: unsignedInteger(pong.nonce, "nonce") }
    }
    default:
      return fail("unknown plugin message tag")
  }
}

export function parseControllerMetadataRequest(
  value: unknown,
): ControllerMetadataRequest {
  const object = exact(value, "controller metadata request", [
    "type",
    "metadataRequestId",
  ])
  if (object.type !== "requestControllerReady")
    return fail("unknown controller control message")
  return {
    type: "requestControllerReady",
    metadataRequestId: parseUuid(object.metadataRequestId),
  }
}

export function parseUuid(value: unknown): string {
  const uuid = boundedString(value, "UUID", 36)
  if (!UUID_PATTERN.test(uuid)) return fail("malformed UUID")
  return uuid.toLowerCase()
}
