/// Host properties can throw under `documentAccess: dynamic-page`, so every
/// read of node content goes through here: one hostile getter costs a field,
/// not the whole walk.
export function hostGet(node: Record<string, unknown>, key: string): unknown {
  try {
    return node[key]
  } catch {
    return undefined
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object"
}

/// A chain deeper than this is treated as not rendering. Bounded because the
/// walk reads host properties that can throw, and because a cycle or an
/// unreadably deep tree must not hang a read. Failing to "does not render" is
/// the safe direction: the server's contract is that what it returns is what
/// Figma draws, so an answer it cannot establish is one it must not give.
const MAX_ANCESTOR_WALK = 128

/// Whether this node and every ancestor are switched on.
///
/// A node's own `visible` is not enough: a switched-on node inside a
/// switched-off frame draws nothing, and reporting it would be the same
/// confidently-wrong shape as reporting a disabled drop shadow.
export function rendersVisibly(node: unknown): boolean {
  let current: unknown = node
  for (let step = 0; step < MAX_ANCESTOR_WALK; step += 1) {
    // Past the root: nothing in the chain was switched off.
    if (!isRecord(current)) return true
    if (hostGet(current, "visible") === false) return false
    current = hostGet(current, "parent")
  }
  return false
}
