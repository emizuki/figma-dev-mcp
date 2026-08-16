import { dispatchControllerMessage } from "./dispatch"
import { completeScreenshotValidation } from "../read/render"
import type { ControllerReady } from "../shared/protocol"
import { detectCapabilities, PLUGIN_VERSION } from "../read/common"
import {
  parseControllerBoundMessage,
  parseControllerMetadataRequest,
} from "../shared/validation"

function controllerReady(metadataRequestId: string): ControllerReady {
  return {
    type: "controllerReady",
    metadataRequestId,
    fileName: figma.root.name,
    currentPage: { id: figma.currentPage.id, name: figma.currentPage.name },
    editorType: figma.editorType,
    pluginVersion: PLUGIN_VERSION,
    capabilities: detectCapabilities(),
  }
}

function postReady(metadataRequestId: string): void {
  figma.ui.postMessage(controllerReady(metadataRequestId))
}

figma.showUI(__html__, { width: 240, height: 96, themeColors: true })

function inbound(input: unknown): unknown {
  if (typeof input === "string") {
    try {
      return JSON.parse(input) as unknown
    } catch {
      return input
    }
  }
  if (input !== null && typeof input === "object" && "pluginMessage" in input) {
    return (input as { pluginMessage: unknown }).pluginMessage
  }
  return input
}

figma.ui.onmessage = async (input: unknown): Promise<void> => {
  const message = inbound(input)
  if (completeScreenshotValidation(message)) return

  try {
    const request = parseControllerMetadataRequest(message)
    postReady(request.metadataRequestId)
    return
  } catch {
    // Non-control messages continue through the closed wire parser.
  }

  try {
    const bound = parseControllerBoundMessage(message)
    const output = await dispatchControllerMessage(bound)
    if (output !== null) figma.ui.postMessage(output)
  } catch {
    // Invalid iframe messages are rejected before dispatch.
  }
}
