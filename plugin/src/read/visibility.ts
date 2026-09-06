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
/// unreadably deep tree must not hang a read. Exhausting this bound means the
/// walk genuinely cannot establish an answer on a pathological tree, so it
/// fails closed to "does not render" — the server's contract is that what it
/// returns is what Figma draws, and an answer it cannot establish is one it
/// must not give.
///
/// A single throwing getter partway up the chain is a different failure and
/// is deliberately not treated the same way: `hostGet` below swallows it and
/// `rendersVisibly` continues as though that property were absent, which for
/// `visible` reads as switched on. That node is overwhelmingly likely to be
/// visible in practice — a hostile getter is a local, transient failure under
/// `documentAccess: dynamic-page`, not evidence the tree is unreadable — and
/// two of this predicate's callers, `get_selection` and `get_design_context`,
/// have no error slot to explain a node quietly missing. Failing open there
/// avoids deleting real content over a getter that misbehaved once; failing
/// closed on bound exhaustion avoids asserting an answer about a chain that
/// was never actually walked.
///
/// The bound is set far above any depth a real document reaches, because
/// failing closed here is the one way this predicate can drop a node that
/// genuinely renders — and its callers `get_selection` and `get_design_context`
/// have no error slot to say so, making that node silently absent rather than
/// refused. When this walk lived in `render.ts` the same exhaustion only
/// suppressed an `EMPTY_NODE_BOUNDS` guess and the node still exported, so the
/// cost of a low bound rose when the predicate became the rule for every read.
/// A cycle still terminates, just after more steps; the extra iterations are
/// paid only on a tree already pathological enough to have no right answer.
const MAX_ANCESTOR_WALK = 1024

/// Depth past which the walk starts recording what it has seen so a cycle is
/// caught at its own length rather than at the bound. Set above any real
/// document's nesting so the common path allocates nothing.
const CYCLE_WATCH_AFTER = 64

/// What the ancestor walk could establish about a node.
///
/// `hidden` and `undetermined` both mean "do not return this node", but they
/// are not the same fact and must not be reported as one: a caller told
/// `NODE_NOT_VISIBLE` about a chain the walk simply could not follow has been
/// told something false about a node that may well render.
export type VisibilityVerdict = "renders" | "hidden" | "undetermined"

/// A node's own `visible` is not enough: a switched-on node inside a
/// switched-off frame draws nothing, and reporting it would be the same
/// confidently-wrong shape as reporting a disabled drop shadow.
export function visibilityOf(node: unknown): VisibilityVerdict {
  let current: unknown = node
  // Allocated only if a chain runs long enough to be suspicious. Every real
  // chain finishes in the low tens, so the common path never builds a set;
  // a cycle is caught at its own length instead of walking the full bound,
  // which matters because this runs once per child on the read hot path and
  // `search_nodes` visits up to 10,000 nodes with no cancellation check
  // inside this loop.
  let seen: Set<unknown> | undefined
  for (let step = 0; step < MAX_ANCESTOR_WALK; step += 1) {
    // Past the root: nothing in the chain was switched off.
    if (!isRecord(current)) return "renders"
    if (hostGet(current, "visible") === false) return "hidden"
    if (step >= CYCLE_WATCH_AFTER) {
      seen ??= new Set()
      // Revisiting a node means the chain loops and has no root, so no
      // answer about it can be established. Same verdict as exhausting the
      // bound, reached in a fraction of the steps.
      if (seen.has(current)) return "undetermined"
      seen.add(current)
    }
    current = hostGet(current, "parent")
  }
  return "undetermined"
}

/// Whether this node and every ancestor are switched on.
///
/// The answer the child filters want: they have no per-node channel at any
/// depth, so `hidden` and `undetermined` are both simply excluded.
export function rendersVisibly(node: unknown): boolean {
  return visibilityOf(node) === "renders"
}
