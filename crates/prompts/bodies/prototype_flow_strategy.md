# Analyze a prototype flow (read-only)

Map how a bounded set of frames interact. Use only read tools. Do not create connector nodes, edit reactions, or change Motion data.

## Sequence

1. Confirm the file with `list_files` when more than one connection is live, then choose an explicit scope (selection, page, or named frames).
2. Call `get_reactions` on that scope. Keep every reaction kind that is present: navigation, overlay open or swap, close/back, component state-change, timeline, and keyframe actions. Do not drop a transition because it is not page navigation. Dangling or inaccessible destinations stay in the result as explicit references.
3. Optionally call `get_motion` on the same node IDs when timing or keyframes would explain the journey.
   - `includeAvailableStyles` defaults to false. Enable it only when the document animation-style catalog is needed.
   - Motion times are seconds (`duration`, `timelineOffset`, `timelineDuration`, `timelinePosition`).
   - If `get_motion` fails with `CAPABILITY_UNAVAILABLE`, continue with reactions only and record that motion data was unavailable. Do not stop the analysis.
4. Resolve names and structure with targeted `get_nodes` and a bounded `get_design_context` call (`detail: "minimal"` or `"compact"`, small depth).
5. Take selective `get_screenshot` calls for a few key screens if a visual check is needed.

## Required output

1. A concise primary-journey summary of the happy path through the inspected scope.
2. A table with columns: source, trigger, action, destination. Keep dangling or missing destinations visible in the destination column.
3. A Mermaid `flowchart` only when the graph can be represented safely: every node id and label must be free of characters that would break the diagram (unescaped quotes, backticks, or angle brackets). Otherwise omit the graph and say why.
4. Separate lists for:
   - unresolved transitions (missing destination metadata)
   - dangling transitions (destination id present but the node is missing or inaccessible)
   - non-navigation transitions (close/back, state change, timeline, keyframe, overlay close, and similar)
5. The inspected scope (connection, page or node IDs) and any truncation warnings from the tool results.

## Rules

- Never invent destinations or rewrite reaction types.
- Never create, delete, or restyle connector nodes.
- Never edit prototype reactions or Motion, including style application or timeline edits.
- If motion is unavailable, the journey is still valid; state the capability gap next to the inspected scope.
- Respect truncation flags. Do not treat an incomplete graph as complete.
