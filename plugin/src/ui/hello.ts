import type { ControllerReady, PluginToBroker } from "../shared/protocol"
import { parseUuid } from "../shared/validation"

type UuidFactory = () => string

export function buildHello(
  ready: ControllerReady,
  uuidFactory: UuidFactory,
): Extract<PluginToBroker, { type: "hello" }> {
  return {
    type: "hello",
    protocolVersion: "1",
    connectionId: parseUuid(uuidFactory()),
    displayName: ready.fileName,
    fileName: ready.fileName,
    currentPage: { id: ready.currentPage.id, name: ready.currentPage.name },
    editorType: ready.editorType,
    pluginVersion: ready.pluginVersion,
    capabilities: { ...ready.capabilities },
  }
}
