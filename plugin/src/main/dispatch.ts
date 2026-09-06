import {
  awaitWithSignal,
  CancellationRegistry,
  ignoreSettlement,
  LocalCancellationError,
  type CancellationSignal,
  type LocalCancellationController,
} from "./cancellation"
import {
  assertNever,
  type ControllerBoundMessage,
  type ControllerOutboundMessage,
  type PluginFailure,
  type ReadOperation,
  type ReadResult,
} from "../shared/protocol"
import {
  PluginReadError,
  readDesignContext,
  readMetadata,
  readNodes,
  readSelection,
} from "../read/navigation"
import { searchNodes } from "../read/search"
import { getStyles } from "../read/styles"
import { getVariables } from "../read/variables"
import { getComponents } from "../read/components"
import { getFonts } from "../read/fonts"
import { getDevModeData } from "../read/dev-mode"
import { getReactions } from "../read/reactions"
import { getMotion } from "../read/motion"
import { getScreenshot } from "../read/render"
import {
  bindProgress,
  createProgressReporter,
  type ProgressFrame,
  type ProgressReporter,
} from "./progress"

const sharedRegistry = new CancellationRegistry()

function pluginUi(): { postMessage(message: unknown): void } | undefined {
  return (
    globalThis as typeof globalThis & {
      figma?: { ui?: { postMessage(message: unknown): void } }
    }
  ).figma?.ui
}

function postControllerProgress(
  controllerRequestId: string,
  requestId: string,
  frame: ProgressFrame,
): void {
  const ui = pluginUi()
  if (ui === undefined) return
  const message: ControllerOutboundMessage = {
    type: "progress",
    controllerRequestId,
    requestId,
    completed: frame.completed,
    message: frame.message,
  }
  if (frame.total !== undefined) message.total = frame.total
  try {
    ui.postMessage(message)
  } catch {
    // Progress is best-effort and must not fail the tool.
  }
}

function createControllerProgress(
  controllerRequestId: string,
  requestId: string,
): ProgressReporter {
  return createProgressReporter({
    emit: (frame) =>
      postControllerProgress(controllerRequestId, requestId, frame),
  })
}

async function dispatchRead(
  operation: ReadOperation,
  signal: CancellationSignal,
): Promise<ReadResult> {
  if (signal.aborted) throw new PluginReadError("CANCELLED", false)
  switch (operation.operation) {
    case "get_metadata":
      return { operation: "get_metadata", result: readMetadata() }
    case "get_selection":
      return {
        operation: "get_selection",
        result: await readSelection(operation.input, signal),
      }
    case "get_nodes":
      return {
        operation: "get_nodes",
        result: await readNodes(operation.input, signal),
      }
    case "search_nodes":
      return {
        operation: "search_nodes",
        result: await searchNodes(operation.input, signal),
      }
    case "get_design_context":
      return {
        operation: "get_design_context",
        result: await readDesignContext(operation.input, signal),
      }
    case "get_styles":
      return {
        operation: "get_styles",
        result: await getStyles(operation.input, signal),
      }
    case "get_variables":
      return {
        operation: "get_variables",
        result: await getVariables(operation.input, signal),
      }
    case "get_components":
      return {
        operation: "get_components",
        result: await getComponents(operation.input, signal),
      }
    case "get_fonts":
      return {
        operation: "get_fonts",
        result: await getFonts(operation.input, signal),
      }
    case "get_dev_mode_data":
      return {
        operation: "get_dev_mode_data",
        result: await getDevModeData(operation.input, signal),
      }
    case "get_reactions":
      return {
        operation: "get_reactions",
        result: await getReactions(operation.input, signal),
      }
    case "get_motion":
      return {
        operation: "get_motion",
        result: await getMotion(operation.input, signal),
      }
    case "get_screenshot":
      return {
        operation: "get_screenshot",
        result: await getScreenshot(operation.input, signal),
      }
    default:
      return assertNever(operation)
  }
}

export function requestBoundaryFailure(error: unknown): PluginFailure {
  if (error instanceof PluginReadError) {
    return { code: error.code, retryable: error.retryable }
  }
  if (error instanceof LocalCancellationError) {
    return { code: "CANCELLED", retryable: false }
  }
  return { code: "INTERNAL_ERROR", retryable: false }
}

export async function dispatchControllerMessage(
  message: ControllerBoundMessage,
  registry = sharedRegistry,
): Promise<ControllerOutboundMessage | null> {
  switch (message.type) {
    case "cancel":
      registry.cancel(message.controllerRequestId)
      return null
    case "request": {
      let controller: LocalCancellationController | undefined
      let progress: ProgressReporter | undefined
      try {
        controller = registry.begin(message.controllerRequestId)
        progress = createControllerProgress(
          message.controllerRequestId,
          message.requestId,
        )
        bindProgress(controller.signal, progress)
        progress.startHeartbeat("reading")
        // Raced at the request boundary rather than left to the read code to
        // poll: most read code checks `signal` between steps, but a single
        // `await` on a host API — most importantly `exportAsync` for a large
        // screenshot — does not, and cancelling must still take effect
        // immediately rather than waiting for that call to return on its
        // own. `work` is not stopped by losing the race; it keeps running
        // detached, so its eventual settlement is swallowed rather than left
        // to surface as an unhandled rejection.
        const work = dispatchRead(message.operation, controller.signal)
        ignoreSettlement(work)
        const result = await awaitWithSignal(work, controller.signal)
        return {
          type: "response",
          controllerRequestId: message.controllerRequestId,
          requestId: message.requestId,
          result,
        }
      } catch (error: unknown) {
        return {
          type: "error",
          controllerRequestId: message.controllerRequestId,
          requestId: message.requestId,
          error: requestBoundaryFailure(error),
        }
      } finally {
        progress?.stopHeartbeat()
        if (controller !== undefined)
          registry.finish(message.controllerRequestId)
      }
    }
    default:
      return assertNever(message)
  }
}
