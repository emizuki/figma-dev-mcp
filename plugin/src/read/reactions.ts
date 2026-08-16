import type { GetReactionsInput } from "../shared/protocol"
import type {
  GetReactionsResult,
  NodeReactions,
  OverlayBackground,
  OverlayBackgroundInteraction,
  MediaRuntimeAction,
  OverlayPositionType,
  Reaction,
  ReactionAction,
  ReactionOverlay,
  ReactionTrigger,
  Truncation,
} from "../shared/results"
import {
  CANCEL_CHECK_BATCH,
  MAX_RETURNED_NODES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
} from "../shared/limits"
import {
  throwIfAbortedAtBatch,
  type CancellationSignal,
} from "../main/cancellation"
import { resolveDesignRoots } from "./navigation"
import { hasHostField, type FigmaReadApi } from "./common"
import {
  byteLength,
  walkNodeForest,
  type ForestWalkOptions,
  type SerializerLimits,
} from "./serialize"

declare const figma: FigmaReadApi

type UnknownRecord = Record<string, unknown>

function observation(startedAt: string) {
  return { startedAt, completedAt: new Date().toISOString() }
}

function isRecord(value: unknown): value is UnknownRecord {
  return value !== null && typeof value === "object"
}

function record(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {}
}

function string(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback
}

function finite(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

function array(value: unknown): readonly unknown[] {
  return Array.isArray(value) ? value : []
}

function walkOptions(
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): ForestWalkOptions {
  if (signal !== undefined && limits !== undefined) return { signal, limits }
  if (signal !== undefined) return { signal }
  if (limits !== undefined) return { limits }
  return {}
}

class ItemEmission {
  readonly items: GetReactionsResult["items"] = []
  encoded = 0
  considered = 0
  walkTruncation?: Truncation
  emitTruncation?: Truncation

  constructor(readonly limits: SerializerLimits) {}

  get truncation(): Truncation | undefined {
    return this.walkTruncation ?? this.emitTruncation
  }

  mark(truncation: Truncation): void {
    if (this.walkTruncation === undefined) this.walkTruncation = truncation
  }

  push(value: NodeReactions): boolean {
    this.considered += 1
    if (this.emitTruncation !== undefined) return false
    if (this.items.length >= this.limits.returnedNodes) {
      this.emitTruncation = {
        reason: "nodeLimit",
        visitedNodes: this.considered,
      }
      return false
    }
    const encoded = this.encoded + byteLength(value)
    if (encoded > this.limits.encodedBytes) {
      this.emitTruncation = { reason: "byteLimit", encodedBytes: encoded }
      return false
    }
    this.encoded = encoded
    this.items.push({ status: "success", value })
    return true
  }
}

function triggerOf(raw: unknown): ReactionTrigger | undefined {
  const trigger = record(raw).type
  switch (trigger) {
    case "ON_CLICK":
      return "click"
    case "ON_DRAG":
      return "drag"
    case "ON_HOVER":
      return "hover"
    case "ON_PRESS":
      return "press"
    case "ON_KEY_DOWN":
      return "keyDown"
    case "AFTER_TIMEOUT":
      return "afterDelay"
    case "MOUSE_ENTER":
      return "mouseEnter"
    case "MOUSE_LEAVE":
      return "mouseLeave"
    case "MOUSE_UP":
      return "mouseUp"
    case "MOUSE_DOWN":
      return "mouseDown"
    case "ON_MEDIA_HIT":
      return "mediaHit"
    case "ON_MEDIA_END":
      return "mediaEnd"
    default:
      return undefined
  }
}

function nodeAction(
  navigation: unknown,
):
  | "navigate"
  | "openOverlay"
  | "swapOverlay"
  | "changeTo"
  | "scrollTo"
  | undefined {
  switch (navigation) {
    case "NAVIGATE":
      return "navigate"
    case "OVERLAY":
      return "openOverlay"
    case "SWAP":
      return "swapOverlay"
    case "CHANGE_TO":
      return "changeTo"
    case "SCROLL_TO":
      return "scrollTo"
    default:
      return undefined
  }
}

function mediaActionOf(raw: unknown): MediaRuntimeAction | undefined {
  switch (raw) {
    case "PLAY":
      return "play"
    case "PAUSE":
      return "pause"
    case "TOGGLE_PLAY_PAUSE":
      return "togglePlayPause"
    case "MUTE":
      return "mute"
    case "UNMUTE":
      return "unmute"
    case "TOGGLE_MUTE_UNMUTE":
      return "toggleMuteUnmute"
    case "SKIP_FORWARD":
      return "skipForward"
    case "SKIP_BACKWARD":
      return "skipBackward"
    case "SKIP_TO":
      return "skipTo"
    default:
      return undefined
  }
}

function destinationAction(
  type: "navigate" | "openOverlay" | "swapOverlay" | "changeTo" | "scrollTo",
  destinationId: unknown,
): ReactionAction {
  const action: {
    type: "navigate" | "openOverlay" | "swapOverlay" | "changeTo" | "scrollTo"
    destinationId?: string
  } = { type }
  if (typeof destinationId === "string" && destinationId.length > 0) {
    action.destinationId = destinationId
  }
  return action
}

function actionOf(raw: unknown): ReactionAction | undefined {
  const action = record(raw)
  switch (action.type) {
    case "BACK":
      return { type: "back" }
    case "CLOSE":
      return { type: "closeOverlay" }
    case "SET_VARIABLE": {
      const variableId = string(action.variableId)
      return variableId.length > 0
        ? { type: "setVariable", variableId }
        : { type: "setVariable" }
    }
    case "SET_VARIABLE_MODE": {
      const collectionId = string(record(raw).variableCollectionId)
      const modeId = string(record(raw).variableModeId)
      const result: ReactionAction = { type: "setVariableMode" }
      if (collectionId.length > 0) result.variableCollectionId = collectionId
      if (modeId.length > 0) result.variableModeId = modeId
      return result
    }
    case "CONDITIONAL":
      return { type: "conditional" }
    case "UPDATE_MEDIA_RUNTIME": {
      const mediaAction = mediaActionOf(record(raw).mediaAction)
      if (mediaAction === undefined) return undefined
      const result: ReactionAction = { type: "updateMediaRuntime", mediaAction }
      const destinationId = record(raw).destinationId
      if (typeof destinationId === "string" && destinationId.length > 0) {
        result.destinationId = destinationId
      }
      const amount = finite(record(raw).amountToSkip)
      if (amount !== undefined) result.amountToSkip = amount
      const timestamp = finite(record(raw).newTimestamp)
      if (timestamp !== undefined) result.newTimestamp = timestamp
      return result
    }
    case "URL": {
      const uri = string(action.url)
      return uri.length === 0 ? undefined : { type: "openLink", uri }
    }
    case "NODE": {
      const type = nodeAction(action.navigation)
      return type === undefined
        ? undefined
        : destinationAction(type, action.destinationId)
    }
    default:
      return undefined
  }
}

async function lookupDestination(id: string): Promise<unknown> {
  if (figma.getNodeByIdAsync === undefined) return undefined
  const node = await figma.getNodeByIdAsync(id)
  return node === null ? undefined : node
}

async function destinationAccessible(action: ReactionAction): Promise<{
  accessible: boolean
  node: unknown
}> {
  switch (action.type) {
    case "closeOverlay":
    case "back":
    case "openLink":
      return { accessible: true, node: undefined }
    case "setVariable":
    case "setVariableMode":
    case "conditional":
      return { accessible: false, node: undefined }
    default: {
      if (action.destinationId === undefined) {
        return { accessible: false, node: undefined }
      }
      const node = await lookupDestination(action.destinationId)
      return { accessible: node !== undefined, node }
    }
  }
}

function overlayPositionType(raw: unknown): OverlayPositionType | undefined {
  switch (raw) {
    case "CENTER":
      return "center"
    case "TOP_LEFT":
      return "topLeft"
    case "TOP_CENTER":
      return "topCenter"
    case "TOP_RIGHT":
      return "topRight"
    case "BOTTOM_LEFT":
      return "bottomLeft"
    case "BOTTOM_CENTER":
      return "bottomCenter"
    case "BOTTOM_RIGHT":
      return "bottomRight"
    case "MANUAL":
      return "manual"
    default:
      return undefined
  }
}

function overlayBackground(raw: unknown): OverlayBackground | undefined {
  const value = record(raw)
  switch (value.type) {
    case "NONE":
      return { type: "none" }
    case "SOLID_COLOR": {
      const color = record(value.color)
      const r = finite(color.r)
      const g = finite(color.g)
      const b = finite(color.b)
      const a = finite(color.a)
      return r === undefined ||
        g === undefined ||
        b === undefined ||
        a === undefined
        ? undefined
        : { type: "solidColor", color: { r, g, b, a } }
    }
    default:
      return undefined
  }
}

function overlayBackgroundInteraction(
  raw: unknown,
): OverlayBackgroundInteraction | undefined {
  switch (raw) {
    case "NONE":
      return "none"
    case "CLOSE_ON_CLICK_OUTSIDE":
      return "closeOnClickOutside"
    default:
      return undefined
  }
}

function relativePosition(raw: unknown): { x: number; y: number } | undefined {
  const point = record(raw)
  const x = finite(point.x)
  const y = finite(point.y)
  return x === undefined || y === undefined ? undefined : { x, y }
}

function overlaySettings(
  action: ReactionAction,
  actionRaw: unknown,
  destination: unknown,
): ReactionOverlay | undefined {
  const overlay: ReactionOverlay = {}
  const position = relativePosition(record(actionRaw).overlayRelativePosition)
  if (position !== undefined) overlay.relativePosition = position
  if (action.type === "openOverlay" || action.type === "swapOverlay") {
    const dest = record(destination)
    const positionType = overlayPositionType(dest.overlayPositionType)
    if (positionType !== undefined) overlay.positionType = positionType
    const background = overlayBackground(dest.overlayBackground)
    if (background !== undefined) overlay.background = background
    const interaction = overlayBackgroundInteraction(
      dest.overlayBackgroundInteraction,
    )
    if (interaction !== undefined) overlay.backgroundInteraction = interaction
  }
  return overlay.relativePosition === undefined &&
    overlay.positionType === undefined &&
    overlay.background === undefined &&
    overlay.backgroundInteraction === undefined
    ? undefined
    : overlay
}

function reactionActions(raw: unknown): unknown[] {
  const reaction = record(raw)
  if (Array.isArray(reaction.actions)) return [...reaction.actions]
  if (reaction.action !== undefined) return [reaction.action]
  return []
}

async function serializeReactions(raw: unknown): Promise<Reaction[]> {
  const reactions: Reaction[] = []
  for (const item of array(raw)) {
    const trigger = triggerOf(record(item).trigger)
    if (trigger === undefined) continue
    for (const candidate of reactionActions(item)) {
      const action = actionOf(candidate)
      if (action === undefined) continue
      const destination = await destinationAccessible(action)
      const reaction: Reaction = {
        trigger,
        action,
        destinationAccessible: destination.accessible,
      }
      const triggerRaw = record(record(item).trigger)
      if (trigger === "afterDelay") {
        const timeout = finite(triggerRaw.timeout)
        if (timeout !== undefined) reaction.timeout = timeout
      }
      if (
        trigger === "mouseEnter" ||
        trigger === "mouseLeave" ||
        trigger === "mouseUp" ||
        trigger === "mouseDown"
      ) {
        const delay = finite(triggerRaw.delay)
        if (delay !== undefined) reaction.delay = delay
      }
      if (trigger === "keyDown") {
        if (Array.isArray(triggerRaw.keyCodes)) {
          reaction.keyCodes = triggerRaw.keyCodes.filter(
            (code): code is number =>
              typeof code === "number" && Number.isFinite(code),
          )
        }
        if (
          typeof triggerRaw.device === "string" &&
          triggerRaw.device.length > 0
        ) {
          reaction.device = triggerRaw.device
        }
      }
      if (trigger === "mediaHit") {
        const mediaHitTime = finite(triggerRaw.mediaHitTime)
        const timestamp = finite(triggerRaw.timestamp)
        if (mediaHitTime !== undefined) reaction.mediaHitTime = mediaHitTime
        else if (timestamp !== undefined) reaction.mediaHitTime = timestamp
      }
      const transition = record(record(candidate).transition)
      if (typeof transition.type === "string" && transition.type.length > 0) {
        reaction.transitionId = transition.type
      }
      const duration = finite(transition.duration)
      if (duration !== undefined) reaction.transitionDuration = duration
      const overlay = overlaySettings(action, candidate, destination.node)
      if (overlay !== undefined) reaction.overlay = overlay
      reactions.push(reaction)
    }
  }
  return reactions
}

async function serializeNode(raw: unknown): Promise<NodeReactions | undefined> {
  const node = record(raw)
  const nodeId = string(node.id)
  if (nodeId.length === 0) return undefined
  return {
    nodeId,
    reactions: hasHostField(node, "reactions")
      ? await serializeReactions(node.reactions)
      : [],
  }
}

export async function getReactions(
  input: Partial<GetReactionsInput> = {},
  signal?: CancellationSignal,
  limits?: Partial<SerializerLimits>,
): Promise<GetReactionsResult> {
  const startedAt = new Date().toISOString()
  const emission = new ItemEmission({
    returnedNodes: limits?.returnedNodes ?? MAX_RETURNED_NODES,
    visitedNodes: limits?.visitedNodes ?? MAX_VISITED_NODES,
    encodedBytes: limits?.encodedBytes ?? MAX_TEXT_BYTES,
  })
  const roots = await resolveDesignRoots(input.selector, signal)
  const pending: unknown[] = []
  const walked = walkNodeForest(roots, walkOptions(signal, limits), (raw) => {
    pending.push(raw)
  })
  if (walked.truncation !== undefined) emission.mark(walked.truncation)
  for (let index = 0; index < pending.length; index += 1) {
    throwIfAbortedAtBatch(signal, index, CANCEL_CHECK_BATCH)
    signal?.throwIfAborted()
    const value = await serializeNode(pending[index])
    if (value !== undefined && !emission.push(value)) break
  }
  const result: GetReactionsResult = {
    items: emission.items,
    truncated: emission.truncation !== undefined,
    observation: observation(startedAt),
  }
  if (emission.truncation !== undefined) result.truncation = emission.truncation
  return result
}
