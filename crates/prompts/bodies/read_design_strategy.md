# Read a Figma design efficiently

Use only this server's 14 read-only tools. Do not mutate the Figma document, change the current page or selection, or create local files.

## Sequence

1. Resolve the target file. When more than one plugin connection may be live, call `list_files` and pass that `connectionId` on later calls. Omit `connectionId` only when exactly one file is connected.
2. Start with `get_metadata` plus one bounded `get_design_context` call.
   - `get_metadata` returns file, page, editor, and capability information. It does not serialize page contents.
   - Prefer the current selection or an explicit node/page selector the user named.
   - Use `detail: "compact"` and a small `depth` (2 is typical) so the first tree stays token-efficient.
   - Set `dedupeComponents: true` when the screen repeats component instances.
3. Prefer targeted `search_nodes` inside exactly one `pageId` or `nodeId` scope, using a simple string `query` plus optional `types`, `match`, and `limit`. When `nextCursor` is present, repeat the same scope/query/filters with that cursor before widening the scope. Then batch `get_nodes` for the IDs you still need; do not issue one-id-at-a-time reads.
4. Request specialized handoff data only when it is relevant to the implementation task:
   - `get_styles` for paint, text, effect, and grid styles
   - `get_variables` for collections, modes, aliases, and code syntax
   - `get_components` for component sets, variants, and documentation
   - `get_fonts` for families used in the same bounded scope
   - `get_dev_mode_data` for annotations, documentation links, and dev resources
   - `get_reactions` for prototype interactions on the same screen
5. Call `get_screenshot` last, and only for visual confirmation of a specific node, an ordered node list, or the captured selection.

If you need to confirm what is selected, `get_selection` is a successful empty list when nothing is selected. Ask the user for a node or page instead of loading every page.

## Detail levels

- `minimal`: identity, hierarchy references, type, name, visibility, and bounds.
- `compact`: minimal plus layout, text summary, style references, and component metadata. Use this for the first structural read.
- `full`: compact plus resolved paint, stroke, effect, corner, blend, and text values, plus tool-specific developer metadata. Use only on a small, explicit scope.

Absent optional fields mean the Figma default, not missing data. `cornerRadius`, `strokes`, `clipsContent`, `blendMode`, `cornerSmoothing`, `wrap`, and the text alignment fields are omitted at their default values. `paints` on a node is fills only; border colour is `strokes.paints`. A `styleReferences` entry carries `name` when the style resolved; an absent `name` means it could not be resolved, not that the style is unnamed.

No detail level is an unbounded recursive walk. Honor depth, node-count, and truncation metadata.

## Component instances

Every `INSTANCE` node carries its own component property values: variant selections, text property content, boolean toggles, and instance-swap targets. Read them from the instance node in the tree you already fetched instead of issuing another call. Property names keep their `#` suffix so they match the definitions returned by `get_components`; join on that name to see which values differ from the component default.

When `dedupeComponents` is true, repeated component definitions are serialized once and later instances reference that definition. Use this for lists, tables, and repeated cards so you do not reserialize the same component tree. Each instance still reports its own property values.

## Rules

- Stay inside the selector you chose. Do not widen to other pages automatically.
- `search_nodes` is single-scope only; do not request document-wide or multi-page search.
- `search_nodes.query` is a case-insensitive substring over node names and TEXT characters by default; use `match: "exact"` only when needed.
- Treat `connectionId` as an ephemeral handle from `list_files`. Rediscover it after a reconnect.
- This catalog has no document dump, viewport read, text scan, or local export tool. Do not invent those calls.
