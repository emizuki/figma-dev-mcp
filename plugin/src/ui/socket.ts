import {
  assertNever,
  type ControllerOutboundMessage,
  type PluginToBroker,
} from "../shared/protocol"
import { BROKER_URL, RECONNECT_DELAYS_MS } from "../shared/limits"
import {
  parseBrokerToPlugin,
  parseControllerBoundMessage,
  parseUuid,
} from "../shared/validation"
import { buildHello } from "./hello"
import { onControllerMessage, sendToController } from "./relay"
import { randomUuid } from "./uuid"

function setStatus(text: string): void {
  if (typeof document === "undefined") return
  const node = document.getElementById("status")
  if (node !== null) node.textContent = text
}

function sendJson(socket: WebSocket, message: PluginToBroker): void {
  socket.send(JSON.stringify(message))
}

interface SocketGeneration {
  socket: WebSocket
  metadataRequestId: string
  helloSent: boolean
  acceptedSinceOpen: boolean
}

interface RequestOwner {
  generation: SocketGeneration
  brokerRequestId: string
}

export function startSocketTransport(): () => void {
  let stopped = false
  let reconnectAttempt = 0
  let active: SocketGeneration | undefined
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  const requestOwners = new Map<string, RequestOwner>()

  const isActiveOpen = (generation: SocketGeneration): boolean =>
    !stopped &&
    active === generation &&
    generation.socket.readyState === WebSocket.OPEN

  const findGenerationRequest = (
    generation: SocketGeneration,
    brokerRequestId: string,
  ): readonly [string, RequestOwner] | undefined => {
    for (const entry of requestOwners) {
      const [controllerRequestId, owner] = entry
      if (
        owner.generation === generation &&
        owner.brokerRequestId === brokerRequestId
      ) {
        return entry
      }
    }
    return undefined
  }

  const cancelGenerationRequests = (generation: SocketGeneration): void => {
    for (const [controllerRequestId, owner] of requestOwners) {
      if (owner.generation !== generation) continue
      requestOwners.delete(controllerRequestId)
      sendToController({
        type: "cancel",
        controllerRequestId,
        requestId: owner.brokerRequestId,
      })
    }
  }

  const receiveController = (message: ControllerOutboundMessage): void => {
    switch (message.type) {
      case "controllerReady": {
        const generation = active
        if (
          generation !== undefined &&
          isActiveOpen(generation) &&
          !generation.helloSent &&
          message.metadataRequestId === generation.metadataRequestId
        ) {
          sendJson(generation.socket, buildHello(message, randomUuid))
          generation.helloSent = true
          // Not "Connected" yet: the broker has not accepted anything. It can
          // still refuse this hello — a protocol-version mismatch closes the
          // socket right here — and claiming success now is what let the
          // backoff counter reset on every rejected attempt, pinning the
          // reconnect delay at the table's first entry forever.
          setStatus("Hello sent, waiting for broker…")
        }
        return
      }
      case "progress":
      case "response":
      case "error": {
        const owner = requestOwners.get(message.controllerRequestId)
        if (owner === undefined || !isActiveOpen(owner.generation)) return
        if (message.type !== "progress") {
          requestOwners.delete(message.controllerRequestId)
        }
        switch (message.type) {
          case "progress": {
            const outbound: PluginToBroker = {
              type: "progress",
              requestId: owner.brokerRequestId,
              completed: message.completed,
            }
            if (message.total !== undefined) outbound.total = message.total
            if (message.message !== undefined)
              outbound.message = message.message
            sendJson(owner.generation.socket, outbound)
            return
          }
          case "response":
            sendJson(owner.generation.socket, {
              type: "response",
              requestId: owner.brokerRequestId,
              result: message.result,
            })
            return
          case "error":
            sendJson(owner.generation.socket, {
              type: "error",
              requestId: owner.brokerRequestId,
              error: message.error,
            })
            return
          default:
            return assertNever(message)
        }
      }
      default:
        return assertNever(message)
    }
  }

  const stopListening = onControllerMessage(receiveController)

  const scheduleReconnect = (): void => {
    if (stopped || reconnectTimer !== undefined) return
    const index = Math.min(reconnectAttempt, RECONNECT_DELAYS_MS.length - 1)
    const delay = RECONNECT_DELAYS_MS[index]
    if (delay === undefined) throw new Error("reconnect delay table is empty")
    reconnectAttempt += 1
    reconnectTimer = setTimeout(() => {
      reconnectTimer = undefined
      connect()
    }, delay)
  }

  const connect = (): void => {
    if (stopped || active !== undefined) return
    setStatus("Opening socket…")
    const metadataRequestId = randomUuid()
    let candidate: WebSocket
    try {
      candidate = new WebSocket(BROKER_URL)
    } catch (error: unknown) {
      const detail = error instanceof Error ? error.message : "WebSocket failed"
      setStatus(detail)
      scheduleReconnect()
      return
    }
    const generation: SocketGeneration = {
      socket: candidate,
      metadataRequestId,
      helloSent: false,
      acceptedSinceOpen: false,
    }
    active = generation

    candidate.addEventListener("open", () => {
      if (!isActiveOpen(generation)) return
      setStatus("Socket open, waiting for Figma…")
      sendToController({
        type: "requestControllerReady",
        metadataRequestId: generation.metadataRequestId,
      })
    })
    candidate.addEventListener("message", (event: MessageEvent<unknown>) => {
      if (
        !isActiveOpen(generation) ||
        typeof event.data !== "string" ||
        !generation.helloSent
      )
        return
      // The broker never acknowledges a hello: `BrokerToPlugin` is
      // request | cancel | ping. A frame can only arrive on a socket it chose
      // to keep, so the first one is the only proof of acceptance available.
      if (!generation.acceptedSinceOpen) {
        generation.acceptedSinceOpen = true
        reconnectAttempt = 0
        setStatus("Connected to local broker")
      }
      try {
        const message = parseBrokerToPlugin(JSON.parse(event.data))
        switch (message.type) {
          case "request": {
            if (
              findGenerationRequest(generation, message.requestId) !== undefined
            )
              return
            const controllerRequestId = parseUuid(randomUuid())
            const controllerMessage = parseControllerBoundMessage({
              ...message,
              controllerRequestId,
            })
            if (controllerMessage.type !== "request") return
            requestOwners.set(controllerRequestId, {
              generation,
              brokerRequestId: message.requestId,
            })
            sendToController(controllerMessage)
            return
          }
          case "cancel": {
            const entry = findGenerationRequest(generation, message.requestId)
            if (entry === undefined) return
            const [controllerRequestId, owner] = entry
            requestOwners.delete(controllerRequestId)
            sendToController(
              parseControllerBoundMessage({
                type: "cancel",
                controllerRequestId,
                requestId: owner.brokerRequestId,
              }),
            )
            return
          }
          case "ping":
            sendJson(candidate, { type: "pong", nonce: message.nonce })
            return
          default:
            return assertNever(message)
        }
      } catch {
        // Malformed broker frames do not cross into the controller.
      }
    })
    candidate.addEventListener("close", () => {
      cancelGenerationRequests(generation)
      if (active !== generation) return
      active = undefined
      setStatus("Reconnecting…")
      scheduleReconnect()
    })
    candidate.addEventListener("error", () => {
      setStatus("Cannot reach local broker")
      candidate.close()
    })
  }

  connect()

  return () => {
    stopped = true
    stopListening()
    if (reconnectTimer !== undefined) clearTimeout(reconnectTimer)
    reconnectTimer = undefined
    const generation = active
    active = undefined
    if (generation !== undefined) {
      cancelGenerationRequests(generation)
      generation.socket.close()
    }
  }
}
