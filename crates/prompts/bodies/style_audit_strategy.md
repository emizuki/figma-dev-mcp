# Audit raw values against styles and variables (report only)

Compare painted and typed values in a bounded scope with the file's styles and variables. Produce a designer-facing report. Do not apply styles, bind variables, or call any mutation tool.

## Scope

1. Unless the user named a page or node, start from the current selection via `get_selection`.
2. If that selection is empty, ask the user for an explicit page or node scope. Never widen to the current page or the rest of the file automatically.
3. Stay inside the agreed scope for every later call. Use `list_files` only when the connection is ambiguous.

## Sequence

1. Load the design system once with `get_styles` and `get_variables` for that same bounded scope.
2. Call `get_design_context` with `detail: "full"` only inside that scope and with a bounded depth. Do not request full detail beyond the agreed selector.
3. Compare each node's raw or partially linked fills, strokes, text, effects, and layout values with the returned styles and variables.
4. Use `get_nodes` only for a targeted follow-up ID the first pass did not explain. Do not walk sibling pages.

## Required findings

For every issue, report:

- node ID and name
- the raw or partially linked value
- issue category (unlinked fill, unlinked text, unlinked effect, mixed binding, other)
- matching style or variable when one exists
- match confidence and a short rationale
- design-system gaps: values with no candidate style or variable

## Recommendations

You may recommend that a designer link a matching style or variable, or add a missing token in the design system. Do not apply, bind, or otherwise change the document. This prompt never mutates Figma and never creates local files.
