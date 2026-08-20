export const OPERATION_NAMES: readonly [
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
] = [
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
]

export type OperationName = (typeof OPERATION_NAMES)[number]
export type DetailLevel = "minimal" | "compact" | "full"

export interface CapabilitySet {
  annotations: boolean
  devResources: boolean
  motion: boolean
  svgStringExport: boolean
  variableCodeSyntax: boolean
}

export type Selector =
  | { selection: true }
  | { pageId: string }
  | { pageIds: string[] }
  | { nodeId: string }
  | { nodeIds: string[] }

export interface FileScopedInput {
  connectionId?: string
}

export interface ScopedInput extends FileScopedInput {
  selector?: Selector
}

export type GetMetadataInput = FileScopedInput

export interface GetSelectionInput extends FileScopedInput {
  detail?: DetailLevel
  depth?: number
}

export interface GetNodesInput extends FileScopedInput {
  nodeIds: string[]
  detail?: DetailLevel
  depth?: number
}

export type SearchScope = { pageId: string } | { nodeId: string }

export type SearchMatchMode = "exact" | "contains"

export interface SearchNodesInput extends FileScopedInput {
  scope: SearchScope
  query?: string
  types?: string[]
  match: SearchMatchMode
  limit: number
  cursor?: string
}

export interface GetDesignContextInput extends ScopedInput {
  depth?: number
  detail?: DetailLevel
  includeHidden: boolean
  dedupeComponents: boolean
}

export type StyleSource = "local" | "referenced" | "both"

export interface GetStylesInput extends ScopedInput {
  source: StyleSource
}

export interface GetVariablesInput extends ScopedInput {
  resolveAliases: boolean
}

export type GetComponentsInput = ScopedInput
export type GetFontsInput = ScopedInput
export type GetDevModeDataInput = ScopedInput
export type GetReactionsInput = ScopedInput

export interface GetMotionInput extends ScopedInput {
  includeAvailableStyles: boolean
}

export type ScreenshotSelector =
  | { selection: true }
  | { nodeId: string }
  | { nodeIds: string[] }

export type GetScreenshotInput =
  | {
      format: "png" | "jpeg"
      connectionId?: string
      selector: ScreenshotSelector
      scale?: number
    }
  | {
      format: "svg"
      connectionId?: string
      selector: ScreenshotSelector
      svgOutlineText: boolean
      svgIdAttribute: boolean
      svgSimplifyStroke: boolean
    }

export type ReadOperation =
  | { operation: "get_metadata"; input: GetMetadataInput }
  | { operation: "get_selection"; input: GetSelectionInput }
  | { operation: "get_nodes"; input: GetNodesInput }
  | { operation: "search_nodes"; input: SearchNodesInput }
  | { operation: "get_design_context"; input: GetDesignContextInput }
  | { operation: "get_styles"; input: GetStylesInput }
  | { operation: "get_variables"; input: GetVariablesInput }
  | { operation: "get_components"; input: GetComponentsInput }
  | { operation: "get_fonts"; input: GetFontsInput }
  | { operation: "get_dev_mode_data"; input: GetDevModeDataInput }
  | { operation: "get_reactions"; input: GetReactionsInput }
  | { operation: "get_motion"; input: GetMotionInput }
  | { operation: "get_screenshot"; input: GetScreenshotInput }

import type {
  GetComponentsResult,
  GetDesignContextResult,
  GetDevModeDataResult,
  GetFontsResult,
  GetMetadataResult,
  GetMotionResult,
  GetNodesResult,
  GetReactionsResult,
  GetScreenshotResult,
  GetSelectionResult,
  GetStylesResult,
  GetVariablesResult,
  SearchNodesResult,
} from "./results"

export type * from "./results"

export type ReadResult =
  | { operation: "get_metadata"; result: GetMetadataResult }
  | { operation: "get_selection"; result: GetSelectionResult }
  | { operation: "get_nodes"; result: GetNodesResult }
  | { operation: "search_nodes"; result: SearchNodesResult }
  | { operation: "get_design_context"; result: GetDesignContextResult }
  | { operation: "get_styles"; result: GetStylesResult }
  | { operation: "get_variables"; result: GetVariablesResult }
  | { operation: "get_components"; result: GetComponentsResult }
  | { operation: "get_fonts"; result: GetFontsResult }
  | { operation: "get_dev_mode_data"; result: GetDevModeDataResult }
  | { operation: "get_reactions"; result: GetReactionsResult }
  | { operation: "get_motion"; result: GetMotionResult }
  | { operation: "get_screenshot"; result: GetScreenshotResult }

/** `UNSAFE_SVG` is reserved and no longer emitted: SVG safety reports a verdict
 * on the asset rather than failing the item. The member stays because removing
 * a member of a closed enum is itself a wire change. */
export const ERROR_CODES: readonly [
  "NO_FIGMA_CONNECTION",
  "AMBIGUOUS_CONNECTION",
  "CONNECTION_NOT_FOUND",
  "CONNECTION_LOST",
  "PROTOCOL_MISMATCH",
  "NODE_NOT_FOUND",
  "PAGE_NOT_FOUND",
  "UNSUPPORTED_NODE",
  "CAPABILITY_UNAVAILABLE",
  "UNSAFE_SVG",
  "INVALID_CURSOR",
  "LIMIT_EXCEEDED",
  "TIMEOUT",
  "CANCELLED",
  "INTERNAL_ERROR",
] = [
  "NO_FIGMA_CONNECTION",
  "AMBIGUOUS_CONNECTION",
  "CONNECTION_NOT_FOUND",
  "CONNECTION_LOST",
  "PROTOCOL_MISMATCH",
  "NODE_NOT_FOUND",
  "PAGE_NOT_FOUND",
  "UNSUPPORTED_NODE",
  "CAPABILITY_UNAVAILABLE",
  "UNSAFE_SVG",
  "INVALID_CURSOR",
  "LIMIT_EXCEEDED",
  "TIMEOUT",
  "CANCELLED",
  "INTERNAL_ERROR",
]

export type ErrorCode = (typeof ERROR_CODES)[number]

export interface PluginItemFailure {
  index: number
  id?: string
  code: ErrorCode
  retryable: boolean
}

export interface PluginFailure {
  code: ErrorCode
  retryable: boolean
  items?: PluginItemFailure[]
}

export type BrokerToPlugin =
  | {
      type: "request"
      requestId: string
      deadlineMs: number
      target: { fileKey?: string }
      operation: ReadOperation
    }
  | { type: "cancel"; requestId: string }
  | { type: "ping"; nonce: number }

export type ControllerBoundMessage =
  | {
      type: "request"
      controllerRequestId: string
      requestId: string
      deadlineMs: number
      target: { fileKey?: string }
      operation: ReadOperation
    }
  | {
      type: "cancel"
      controllerRequestId: string
      requestId: string
    }

export type PluginToBroker =
  | {
      type: "hello"
      protocolVersion: string
      connectionId: string
      displayName: string
      fileKey?: string
      fileName: string
      currentPage: { id: string; name: string }
      editorType: string
      pluginVersion: string
      capabilities: CapabilitySet
    }
  | {
      type: "progress"
      requestId: string
      completed: number
      total?: number
      message?: string
    }
  | { type: "response"; requestId: string; result: ReadResult }
  | { type: "error"; requestId: string; error: PluginFailure }
  | { type: "pong"; nonce: number }

export interface ControllerReady {
  type: "controllerReady"
  metadataRequestId: string
  fileName: string
  currentPage: { id: string; name: string }
  editorType: string
  pluginVersion: string
  capabilities: CapabilitySet
}

export type ControllerOutboundMessage =
  | ControllerReady
  | {
      type: "progress"
      controllerRequestId: string
      requestId: string
      completed: number
      total?: number
      message?: string
    }
  | {
      type: "response"
      controllerRequestId: string
      requestId: string
      result: ReadResult
    }
  | {
      type: "error"
      controllerRequestId: string
      requestId: string
      error: PluginFailure
    }

export interface ControllerMetadataRequest {
  type: "requestControllerReady"
  metadataRequestId: string
}

export function assertNever(value: never): never {
  throw new Error(`unreachable protocol variant: ${JSON.stringify(value)}`)
}
