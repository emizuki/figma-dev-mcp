# Manual acceptance

Spec §19.6 checks against a representative Figma file in Dev Mode. This file is the checklist and the record. It does not embed node text, design names, or image payloads.

The automated suite in [testing.md](testing.md) does **not** replace this gate. If a representative Figma file or an interactive Figma session is unavailable, mark the gate **pending**. Do not claim the MVP complete and do not fabricate results.

## Record

| Field | Value |
| --- | --- |
| Date | 2026-08-16 |
| Figma desktop version | 126.7.10 observed from `/Applications/Figma.app`; no interactive Dev Mode session for these checks |
| plugin build hash | workspace `plugin/dist/code.js` SHA-256 `190ab2cf9299c52e0ceb35740339bad49a6eb0461daec75d11ab72a77528b594` (verification build, not a live-session artifact) |
| binary version | `figma-dev-mcp 0.1.0` (`target/debug/figma-dev-mcp --version`) |
| Overall gate | **pending** |

This gate is **pending**. Figma desktop was running, but this task had no interactive Dev Mode session, no imported development plugin, and no representative file in which to run the nine checks. Do not treat the workspace hash or binary version as live-session proof. The MVP is not complete while this gate is pending.

## Scenarios

Expected results describe observables only (tool names, error codes, counts, presence of `viewBox`). Leave boxes unchecked while the gate is pending.

- [ ] **Two simultaneous files.** Open two Figma files in Dev Mode, run the development plugin in each, then call `list_files`. Expected: two distinct `connectionId` values, distinguishable by file name and current page. Status: pending.

- [ ] **Omitted `connectionId`.** With exactly one connection, omit `connectionId` on a file-scoped tool (for example `get_metadata`). Expected: success. Connect the second file and omit `connectionId` again. Expected: `AMBIGUOUS_CONNECTION`. Status: pending.

- [ ] **Read categories.** Against a bounded selection or node, call `get_selection`, `get_nodes`, `search_nodes`, `get_variables`, `get_styles`, `get_components`, `get_dev_mode_data` (annotations), `get_reactions`, `get_motion` (seconds fields when the Motion API is present, otherwise `CAPABILITY_UNAVAILABLE`), `get_screenshot` raster, and `get_screenshot` SVG source. Expected: structured results useful for implementation; empty selection is a success with an empty list. Status: pending.

- [ ] **Predictable truncation.** Request a deep or large `get_design_context` (high depth or a large frame). Expected: the response sets truncation metadata and returns without freezing the plugin. Status: pending.

- [ ] **Disconnect cleanup.** Start a long read, then stop the plugin. Expected: the in-flight call fails (`CONNECTION_LOST` or equivalent) and `list_files` no longer lists that session. Status: pending.

- [ ] **Document unchanged.** After the reads above, confirm in Figma that document content, current page, selection, plugin data, and relaunch data are unchanged. Expected: no mutation. Status: pending.

- [ ] **No local export.** After `get_screenshot` PNG/JPEG/SVG, inspect the working directory and usual download locations used by the operator. Expected: no new local export file created by the server or plugin. Status: pending.

- [ ] **Prompts.** From the MCP client, list and get `read_design_strategy`, `prototype_flow_strategy`, and `style_audit_strategy`. Expected: each prompt is discovered and returns one user-role workflow that names only the 14 read tools. Status: pending.

- [ ] **SVG vector source.** Export an icon with `get_screenshot` `format: "svg"`. Expected: structured **SVG source** remains vector, keeps its `viewBox`, and keeps internal fragment references; clients may also see `image/svg+xml`. Unsafe SVG would be `UNSAFE_SVG`, not a rewritten file. Status: pending.

- [ ] **Instance property payload growth.** On a page with many instances, call `get_design_context` with the same `selector` and `detail: "compact"` before and after this change, then compare response size. Expected: instances carry component property values; record the percentage byte growth and whether `truncation` appears earlier than before. Also call `get_components` for the same selection and confirm each instance's property names match a `propertyDefinitions[].name` from its component, including the `#…` suffix. Status: exercised 2026-08-18 against a live Dev Mode session (file `[VBA] 26.1_MAJESTRA`); the overall gate stays pending because the other scenarios have not been run. `get_design_context` on one frame at `detail: "compact"`, `depth: 4` returned 42,836 bytes over 23 instances, 20 of which carried properties; the 43 property entries account for 2,782 bytes, i.e. **+6.9%** over the same payload without them. That figure is single-run attribution — the property bytes measured inside one response — not a before/after comparison of two plugin builds, and the effect on truncation was not measured. Join confirmed: instance `8063:18114` reported `Label#11069:169 = "Today"` and `Size = "Medium"`, and `get_components` on its component set `7437:23219` returned the same `Label#11069:169` (default `"Label"`) and `Size` (default `"Large"`) — suffixes matched exactly and the differing values are visible as overrides of the defaults.

- [ ] **Text style units.** On a page containing text with a percentage line height and text left on Auto line height, call `get_nodes` with `detail: "full"` on those text nodes. Expected: `lineHeight` reports `{unit: "percent", value}` and `{unit: "auto"}` respectively, matching what Figma shows in the Inspect panel; no text node reports a line height of `0`. Repeat for a text node with mixed font sizes: expect `fontSize` to be absent rather than `0`. Status: exercised 2026-08-19 against a live Dev Mode session (file `[VBA] 26.1_MAJESTRA`, page `↳ Calendar`); the overall gate stays pending because the other scenarios have not been run. 120 TEXT nodes read at `detail: "full"`: `lineHeight` reported 118 `pixels` and 2 `percent`, `letterSpacing` reported 36 `pixels` and 84 `percent`, and all 120 `styledRanges` entries carried units. Node `I8005:216931;6507:158988;1451:61290` reports `fontSize: 16` with `lineHeight: {unit: "percent", value: 150}`; the previous code path emitted `finite(lineHeight.value)`, i.e. a bare `150`, which reads as `150px` on 16px text instead of the real 24px. No node had `lineHeight` or `letterSpacing` absent, and none reported `auto` — note the scan tallied units, not values, so it does not speak to whether any node has a zero line height. The `AUTO` branch was not observed in this file and remains unverified against live data; a text node left on Auto line height is still needed to close it. Requires the plugin bundle and the server binary to be built from the same revision — an older binary rejects the new shape and drops the session.

## How to run when a session is available

1. Follow [setup.md](setup.md): `cargo build --release`, import `plugin/manifest.json` as a development plugin in Dev Mode, configure the MCP client over stdio, start the plugin in each file.
2. Fill the record table (date, Figma desktop version from Figma → About, SHA-256 of the built `plugin/dist/code.js`, `figma-dev-mcp --version`).
3. Check each box only after observing the expected result. Do not paste design content into this file.
