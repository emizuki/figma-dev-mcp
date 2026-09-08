# Acceptance sweep

For every refusal test in this repository, the question is: *if the accept path
for this same thing broke, would anything go red?* Reading cannot answer it —
the accept path is usually exercised incidentally, and reading cannot tell
incidental coverage from none. Mutation can.

This file records every mutation run, including the ones that went red. The
already-covered cases are the evidence the sweep was run rather than assumed.

Verdicts:

- **covered** — the mutation went red. Something already pins this accept
  path. Nothing to do.
- **gap** — the mutation stayed green. A test was added, named in the last
  column, and verified by re-applying the same mutation.
- **not applicable** — the mutation does not compile (the type system already
  forbids it), or the refusal has no meaningful opposite.

## Scope

Enumerated by the commands in `docs/superpowers/plans/2026-09-07-acceptance-sweep.md`.
Counts are regex-dependent and are a starting point, not an authority.

| Area | refusal-only | already paired |
|---|---|---|
| tests/contracts | 15 | 11 |
| tests/integration | 16 | 4 |
| tests/policy | 8 | 0 |
| crates | 5 | 1 |
| plugin/src/read | 72 | — |
| plugin/src/shared | 11 | — |
| plugin/src/main | 5 | — |
| plugin/src/ui | 24 | — |

## tests/contracts and crates/protocol

The 23 refusal-named tests in `tests/contracts/mod.rs`, plus `allocation.rs` and
`response_accounting.rs`, group into 23 accept paths — a group being what one
mutation answers. (22 at first pass; the 23rd is the `read_frame` row below,
split out once review showed the envelope guard has two readers and only one was
pinned.) Every mutation below was applied to production code, measured, and
reverted; the committed diff adds tests only. Baseline before this task:
**255 passed / 0 failed**. After: **260 passed / 0 failed**.

The worked example the plan cites — renaming `unresolved` so the decoder no
longer recognises the wire key — is row 1, and it comes back **covered**: the
fix for that defect landed with `the_forest_results_decode_an_unresolved_entry_and_keep_it`,
which is exactly the test the brief proposed writing. It is redundant now.

Five gaps, and none is the shape the plan expected — not one is a renamed wire
key.

Four of the five are one class: an **inclusive ceiling turned exclusive**, a
single character each. In every one the refusal at `limit + 1` was pinned and
the acceptance at `limit` was not, so the suite could not tell an inclusive
ceiling from an exclusive one — and `limit` is the one value each ceiling exists
to admit. That is the dominant shape here and the one to look for elsewhere.

The fifth breaks the class, and is the more interesting finding. `read_frame`
is a **second reader of a guard already pinned through `decode_frame`**: the
comparison is not wrong at all, and no character of it changed. What was missing
was any test of the other consumer. It also fails differently — `decode_frame`
works on a slice that already holds the whole frame, so a short read there is
invisible, while `read_frame` pulls from a stream and the bytes it leaves behind
are the next frame's length prefix. Same guard, same value, one reader tested,
and the untested one desynchronises the connection rather than refusing a call.
The duplicated-guard sub-table at the end of this section is what that finding
opened up.

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| A `GetSelectionResult` / `GetDesignContextResult` carrying `unresolved` decodes and keeps the entry | `#[serde(rename)]` on the `Unresolved` arm of the `impl_detail_result_deserialize!` field identifier (`navigation.rs`) | 253 / 2 | covered — `the_forest_results_decode_an_unresolved_entry_and_keep_it`, `both_detail_result_macro_arms_agree_on_every_shared_field` | — |
| An unsafe SVG asset's `rejection` decodes with its kind and offender name | rename the `Rejection` arm of `ScreenshotAssetField` (`visual.rs`) | 253 / 2 | covered — `every_svg_rejection_rule_round_trips_named_and_unnamed`, `an_unsafe_screenshot_asset_carries_its_rule_through_the_result` | — |
| An SVG asset's `source` decodes, escape sequences included | rename the `Source` arm of `ScreenshotAssetField` | 249 / 6 | covered — `a_lone_surrogate_in_svg_source_is_refused_by_the_decoder` and 4 more | — |
| A boundary string of exactly its byte ceiling decodes | `validate_boundary_string`: `value.len() > maximum_bytes` → `>=` | 254 / 1 | covered — `response_accounting::max_svg_source_survives_preview_base64_and_is_charged_once` | — |
| A bounded list of exactly `maximum_items` decodes | `BoundedListVisitor::visit_seq`: `values.len() < self.maximum_items` → `values.len() + 1 < …` | 254 / 1 | covered — `bounded_collection_decoders_stop_at_max_plus_one_and_frame_encoding_is_stream_capped` | — |
| An outbound forest of exactly `MAX_RETURNED_NODES` nodes is built | `validate_node`: `*count > MAX_RETURNED_NODES` → `>=` | 254 / 1 | covered — `outbound_node_collections_reject_wide_roots_and_children_without_auxiliary_growth` | — |
| An outbound tree at exactly `MAX_DEPTH` is built | `validate_node`: `depth > MAX_DEPTH` → `>=` | 255 / 0 | **gap** | `the_outbound_node_builder_accepts_a_tree_at_exactly_the_depth_ceiling` |
| An input asking for `depth: MAX_DEPTH` decodes at the boundary | `deserialize_optional_depth`: `depth > MAX_DEPTH` → `>=` | 255 / 0 | **gap** | `an_input_asking_for_exactly_the_maximum_depth_is_accepted` |
| A frame declaring exactly `MAX_ENVELOPE_BYTES` gets past the length check **in `decode_frame`** | `validate_body_length`: `length > MAX_ENVELOPE_BYTES` → `>=` | 255 / 0 | **gap** | `a_frame_declaring_exactly_the_envelope_ceiling_is_not_refused_for_its_size` |
| The same ceiling **in `read_frame`**, and the frame consumed exactly — the wire path, a second reader of the same guard | the same clamp applied in `read_frame` (`rpc.rs`), which `decode_frame`'s test cannot see | 259 / 0 | **gap** | `the_wire_read_path_takes_a_ceiling_frame_and_consumes_exactly_it` |
| A `FrontendToLeader` frame's `rpcRequestId` decodes | rename the `RpcRequestId` arm of `FrontendMessageField` (`rpc.rs`) | 238 / 17 | covered — `unknown_fields_are_rejected_at_nested_boundaries` and 16 more | — |
| A node's optional `childrenTruncation` decodes and survives | rename the `ChildrenTruncation` arm of `DesignNodeField` | 252 / 3 | covered — `minimal_results_preserve_recursive_depth_and_reject_flat_summaries` and 2 more | — |
| A result's optional `truncation` decodes and survives | rename the `Truncation` arm in both `impl_detail_result_deserialize!` arms | 254 / 1 | covered — `both_detail_result_macro_arms_agree_on_every_shared_field` | — |
| A gradient paint's `gradientTransform` and `opacity` decode | rename the `GradientTransform` arm of `PaintField` | 253 / 2 | covered — `gradient_and_image_paints_carry_opacity_and_direction`, `angular_and_diamond_gradients_are_first_class_paints` | — |
| A `RasterScale` of exactly 4.0 is accepted | `(0.25..=4.0).contains` → `(0.25..4.0).contains` | 254 / 1 | covered — `public_scalar_boundaries_reject_invalid_inbound_and_outbound_values` | — |
| A detail-discriminated result whose `detail` tag arrives last decodes | require `detail` before the payload key in both macro arms | 253 / 2 | covered — `actual_wire_discriminators_accept_tag_last_and_reject_duplicate_or_unknown_fields` | — |
| An `instanceSwap` component / instance property value decodes | rename the `InstanceSwap` tag of `ComponentPropertyTag` | 254 / 1 | covered — `component_property_values_round_trip_and_reject_unknown_kinds` | — |
| A `percent` line-height unit decodes | rename the `Percent` variant of `LineHeightValue` | 253 / 2 | covered — `text_style_units_round_trip_and_reject_unknown_units`, `tools_catalog::schema_snapshots_are_stable` | — |
| A `spaceBetween` layout alignment decodes | rename the `SpaceBetween` variant of `AxisAlign` | 250 / 5 | covered — `layout_alignment_round_trips_and_rejects_unknown_values` and 4 more | — |
| A stroke's optional `dashPattern` decodes | rename `StrokeValue::dash_pattern` | 253 / 2 | covered — `stroke_value_round_trips_and_rejects_unknown_fields`, `tools_catalog::schema_snapshots_are_stable` | — |
| `visitedNodes` decodes on the dev-mode, reactions and motion results | rename `visited_nodes` on all three results in `prototype.rs` | 239 / 16 | covered — the six `get_*_result_*_visited_nodes` tests and 10 more | — |
| `NODE_NOT_VISIBLE` round-trips as a wire code | rename the `NodeNotVisible` serde tag (`error.rs`) | 250 / 5 | covered — `a_hidden_node_is_refused_by_name_rather_than_reported_missing` and 4 more | — |
| A raster asset of exactly `MAX_RASTER_BASE64_BYTES` is returned whole | `content.rs`: `item.image_base64.len() <= MAX_RASTER_BASE64_BYTES` → `<` (both sites) | 255 / 0 | **gap** | `response_accounting::a_raster_asset_at_exactly_the_per_item_ceiling_is_returned_whole` |

Totals: **18 covered, 5 gaps, 0 not applicable.** Each new test was verified by
re-applying its own mutation, and by a second mutation that *accepts* the
boundary value and then drops or alters it — a test that only proves the value
was not refused does not prove it survived. In every case that test, and only
that test, went red.

### Why the mutations are at the comparison, not the constant

The plan suggests narrowing a bound in `crates/protocol/src/limits.rs`.
`fixed_limits_match_the_reviewed_ceiling` asserts all 22 constants by value, so
any edit there goes red for a reason that has nothing to do with the accept path
being measured — it tells you the constant changed, not whether anything reads
it correctly. Every ceiling mutation above therefore narrows the **comparison at
the site that consumes the constant** instead. That is also the mutation that
matches the real failure mode: nobody edits a reviewed constant by accident, and
a `>` becoming a `>=` is one keystroke.

### Duplicated guards: a decode half and a builder half

Two of the five gaps above turned out to be the same structural fault — a bound
enforced in two independent places, with the suite pinning only one of them:

- `MAX_DEPTH` is checked in `DesignNodeListSeed` (decode, pinned) **and** in
  `validate_node` (the outbound `TryFrom`, which was not).
- `MAX_ENVELOPE_BYTES` is read by `decode_frame` (a slice, where a one-byte
  under-read is invisible) **and** by `read_frame` (the wire path, where it
  desynchronises the stream).

Two instances is a pattern, so the protocol crate was swept for the rest. Most
bounded values here enforce their ceiling in more than one place — usually a
`TryFrom` that builds outbound values paired with a `Deserialize` that reads
inbound ones — and the decode half is pinned far more often than the builder
half. Not all of them are pairs: `deferred.rs` carries three checks of the
envelope ceiling and no matching decode counterpart at all, so "twice" is the
common case rather than the rule. The remaining sites, measured but **not
fixed** (reported for a ruling on scope):

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| A `ReturnedList` **built** from exactly `MAX_RETURNED_NODES` values is accepted | `ReturnedList::try_from`: `values.len() > MAX_RETURNED_NODES` → `>=` | 260 / 0 | **gap** — the decode half is pinned, the builder half is not | none — reported, not fixed |
| A `NodeBatch` **built** from exactly `MAX_INPUT_IDS` items is accepted | `NodeBatch::try_from`: `items.len() > MAX_INPUT_IDS` → `>=` | 260 / 0 | **gap** — same split | none — reported, not fixed |
| A `NodeBatch` **decoded** with exactly `MAX_INPUT_IDS` items is accepted | its visitor: `while items.len() < MAX_INPUT_IDS` → `+ 1 <` | 259 / 1 | covered — `recursive_results_enforce_depth_and_global_returned_node_budgets` | — |
| An error item list of exactly `MAX_INPUT_IDS` items is accepted, **built** | `error.rs` `validate_item_count`: `count > MAX_INPUT_IDS` → `>=` | 260 / 0 | **gap** | none — reported, not fixed |
| An error item list of exactly `MAX_INPUT_IDS` items is accepted, **decoded** | `error.rs` `ErrorItemListVisitor`: `while items.len() < MAX_INPUT_IDS` → `+ 1 <` | 260 / 0 | **gap** — this pair has *neither* half pinned | none — reported, not fixed |
| A deferred JSON payload of exactly `MAX_ENVELOPE_BYTES` is accepted | `deferred.rs`: all three `> MAX_ENVELOPE_BYTES` → `>=` (lines 38, 65, 79) | 260 / 0 | **gap** — a third, independent implementation of the envelope ceiling | none — reported, not fixed |
| A node whose children fill the remaining node budget exactly is accepted | `validate_node`: `node.children.len() > MAX_RETURNED_NODES - *count` → `>=` | 259 / 1 | covered — `outbound_node_collections_reject_wide_roots_and_children_without_auxiliary_growth` | — |
| A `NodeIdList` / `PageIdList` / `NodeTypeList` **built** at exactly its ceiling is accepted | `bounded_list_newtype!`'s `TryFrom` (`common.rs:446`): `values.len() > $maximum` → `>=` | 260 / 0 | **gap** — the decode half is in the main table above, the builder half is in neither | none — reported, not fixed |

**Eight groups, six of them unpinned.** Counted by comparison site rather than by
group it is ten, because the `deferred.rs` row bundles three separate checks that
were mutated together.

Two are worth ruling on first:

- **`error.rs`'s item list is the only pair where neither half is pinned.**
  Every other bound in the crate has at least one side held.
- **`deferred.rs` is not a pair at all.** It holds a third, independent
  implementation of the envelope ceiling that `validate_body_length` knows
  nothing about, and nothing in the suite touches any of its three checks.

## plugin shared/validation.ts and shared/result-validation.ts

A gap here means the plugin refuses a result it has just produced itself, so the
session drops. Same consequence as the decoder above, from the other end of the
wire.

`result-validation.ts` is 2,884 lines, exports `parseU32` and `parseReadResult`,
is imported by exactly one production file, and has no dedicated test file of
its own. Its wall is `exact(value, label, required, optional)`, which refuses
any key outside `required ∪ optional`. There are **129 `exact(` call sites**;
**53 of them pass an `optional` list**, and those lists hold **122 (call site,
optional field) pairs**.

Those 122 pairs are the 122 groups, and the grouping is forced rather than
chosen. A group is what one mutation answers, and deleting one name from one
site's `optional` list refuses exactly the payloads carrying that field at that
site — nothing wider, nothing narrower. Grouping by result family, as the plan
suggested, would have hidden the main finding: `truncation` is thirteen separate
entries in thirteen separate lists, and a mutation at one of them says nothing
whatever about the other twelve.

Every mutation below was applied to production code, built, measured, and
reverted; the committed diff adds tests only. Baseline before this task:
**400 pass / 0 fail / 1841 expect() calls / 25 files**. After: **413 pass /
0 fail / 1854 expect() calls / 26 files**.

**48 covered, 74 gaps, 0 not applicable.**

Neither of Task 2's shapes transfers cleanly. The inclusive-ceiling class cannot
arise at all: `exact()` is an allow-list, not a comparison, so there is no
character to move. The duplicated-guard class does arise, and where it does it
is stark — but it is not the majority. **54 of the 122 pairs are a field name
that appears in more than one `optional` list; 36 of those 54 are gaps. The
other 38 gaps are at a name that occurs exactly once.** The bulk of the finding
is therefore simpler than Task 2's: whole result families — reactions, motion,
dev-mode — have no acceptance coverage of their optional fields at all.

### The single largest finding: `truncation`, thirteen times, never once pinned

`truncation` is optional on every one of the thirteen result families, and each
family names it in its own `exact()` call. All thirteen mutations stayed green.
Not one half of this thirteen-way duplication was held by anything in the plugin
suite, and Rust omits the field entirely when it is absent — every one of its
`truncation: Option<Truncation>` fields carries
`skip_serializing_if = "Option::is_none"`, most of them alongside `default`
(`BoundedItems` is the exception) — so the two ends could have drifted silently
in either direction. `parseTruncation`'s own three optional counters
(`appliedDepth`, `visitedNodes`, `encodedBytes`) were likewise unpinned, which
means a truncated response — the response the plugin sends precisely when a
read hit a safety limit — was the least-tested shape in the file.

### What the covered rows do and do not prove

Some covered rows here are weaker than the covered rows in the Rust section
above. Seven `… survive(s) the wire validator` tests in
`plugin/src/read/serialize.test.ts` assert
`expect(parseReadResult(…)).toBeDefined()` and nothing else — the whole test
body is a serializer call and that one line. That is exactly the assertion Task
2's review found insufficient: it goes red when the wall *refuses* the field,
which is why those rows read covered, but it would pass unchanged if the parser
accepted the field and then dropped it. Re-running all 48 covered mutations and
recording which tests went red puts a number on it: **21 of the 48 covered rows
were held by nothing but those seven tests.**

The thirteen new tests therefore carry **all 122 optional fields**, not only the
74 unpinned ones, and compare the whole parsed tree against the payload with
`toEqual`. That re-run confirms it: every one of the 48 covered mutations turned
a new test red as well, so every covered row above now has a survival assertion
behind it too.

### How this enumeration went wrong the first time

The first pass of this section counted 116 pairs, not 122. Four `exact()` calls
pass `[...STYLE_IDENTITY_FIELDS]`, a three-name constant, and the enumerator
printed the spread rather than the names it stands for; transcribing that by
hand kept one representative per site instead of three. All six missing pairs
(`remote` and `key` at the text, effect and grid style sites) were then measured
against the same pre-commit tree, and all six are gaps.

Task 2's survey failed the same way — it grepped for a bound by the constant's
name and missed a site that compared against a macro parameter. The rule both
misses share:

> **An enumeration must expand its indirections before anything is counted: a
> shared constant, a spread, or a macro parameter left standing reads as one
> item where it stands for many, and the count loses the difference silently.**

### The table

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a batch item failure (`parseItemError`) carrying `id` is taken and the field survives | drop `"id"` from the `optional` list at `result-validation.ts:276` | 400 / 0 | **gap** | `a get_nodes result carrying every optional field survives parseReadResult` |
| an unsafe SVG asset's rejection (`parseSvgRejection`) carrying `name` is taken and the field survives | drop `"name"` from the `optional` list at `result-validation.ts:306` | 399 / 1 | covered | — |
| a tool error (`parseToolError`) carrying `items` is taken and the field survives | drop `"items"` from the `optional` list at `result-validation.ts:316` | 400 / 0 | **gap** | `a get_nodes result carrying every optional field survives parseReadResult` |
| a truncation record (`parseTruncation`) carrying `appliedDepth` is taken and the field survives | drop `"appliedDepth"` from the `optional` list at `result-validation.ts:369` | 400 / 0 | **gap** | all thirteen `a get_… result carrying every optional field survives parseReadResult` tests |
| a truncation record (`parseTruncation`) carrying `visitedNodes` is taken and the field survives | drop `"visitedNodes"` from the `optional` list at `result-validation.ts:369` | 400 / 0 | **gap** | all thirteen `a get_… result carrying every optional field survives parseReadResult` tests |
| a truncation record (`parseTruncation`) carrying `encodedBytes` is taken and the field survives | drop `"encodedBytes"` from the `optional` list at `result-validation.ts:369` | 400 / 0 | **gap** | all thirteen `a get_… result carrying every optional field survives parseReadResult` tests |
| the metadata capability set (`parseCapabilitySet`) carrying `annotations` is taken and the field survives | drop `"annotations"` from the `optional` list at `result-validation.ts:414` | 397 / 3 | covered | — |
| the metadata capability set (`parseCapabilitySet`) carrying `devResources` is taken and the field survives | drop `"devResources"` from the `optional` list at `result-validation.ts:414` | 397 / 3 | covered | — |
| the metadata capability set (`parseCapabilitySet`) carrying `motion` is taken and the field survives | drop `"motion"` from the `optional` list at `result-validation.ts:414` | 397 / 3 | covered | — |
| the metadata capability set (`parseCapabilitySet`) carrying `svgStringExport` is taken and the field survives | drop `"svgStringExport"` from the `optional` list at `result-validation.ts:414` | 397 / 3 | covered | — |
| the metadata capability set (`parseCapabilitySet`) carrying `variableCodeSyntax` is taken and the field survives | drop `"variableCodeSyntax"` from the `optional` list at `result-validation.ts:414` | 397 / 3 | covered | — |
| a node's strokes (`parseStrokes`) carrying `weight` is taken and the field survives | drop `"weight"` from the `optional` list at `result-validation.ts:592` | 399 / 1 | covered | — |
| a node's strokes (`parseStrokes`) carrying `align` is taken and the field survives | drop `"align"` from the `optional` list at `result-validation.ts:592` | 399 / 1 | covered | — |
| a node's strokes (`parseStrokes`) carrying `dashPattern` is taken and the field survives | drop `"dashPattern"` from the `optional` list at `result-validation.ts:592` | 399 / 1 | covered | — |
| a node's auto layout (`parseLayout`) carrying `primaryAlign` is taken and the field survives | drop `"primaryAlign"` from the `optional` list at `result-validation.ts:683` | 399 / 1 | covered | — |
| a node's auto layout (`parseLayout`) carrying `counterAlign` is taken and the field survives | drop `"counterAlign"` from the `optional` list at `result-validation.ts:683` | 399 / 1 | covered | — |
| a node's auto layout (`parseLayout`) carrying `wrap` is taken and the field survives | drop `"wrap"` from the `optional` list at `result-validation.ts:683` | 399 / 1 | covered | — |
| a node's auto layout (`parseLayout`) carrying `counterAxisSpacing` is taken and the field survives | drop `"counterAxisSpacing"` from the `optional` list at `result-validation.ts:683` | 399 / 1 | covered | — |
| a text style's line height (`parseLineHeight`) carrying `value` is taken and the field survives | drop `"value"` from the `optional` list at `result-validation.ts:747` | 398 / 2 | covered | — |
| a text style (`parseTextStyle`) carrying `fontSize` is taken and the field survives | drop `"fontSize"` from the `optional` list at `result-validation.ts:777` | 397 / 3 | covered | — |
| a text style (`parseTextStyle`) carrying `lineHeight` is taken and the field survives | drop `"lineHeight"` from the `optional` list at `result-validation.ts:777` | 397 / 3 | covered | — |
| a text style (`parseTextStyle`) carrying `letterSpacing` is taken and the field survives | drop `"letterSpacing"` from the `optional` list at `result-validation.ts:777` | 397 / 3 | covered | — |
| a text style (`parseTextStyle`) carrying `fontWeight` is taken and the field survives | drop `"fontWeight"` from the `optional` list at `result-validation.ts:777` | 399 / 1 | covered | — |
| a text style (`parseTextStyle`) carrying `textDecoration` is taken and the field survives | drop `"textDecoration"` from the `optional` list at `result-validation.ts:777` | 399 / 1 | covered | — |
| a text value (`parseTextValue`) carrying `alignHorizontal` is taken and the field survives | drop `"alignHorizontal"` from the `optional` list at `result-validation.ts:826` | 399 / 1 | covered | — |
| a text value (`parseTextValue`) carrying `alignVertical` is taken and the field survives | drop `"alignVertical"` from the `optional` list at `result-validation.ts:826` | 399 / 1 | covered | — |
| a text value (`parseTextValue`) carrying `autoResize` is taken and the field survives | drop `"autoResize"` from the `optional` list at `result-validation.ts:826` | 399 / 1 | covered | — |
| a component/instance value (`parseComponentValue`) carrying `componentSetId` is taken and the field survives | drop `"componentSetId"` from the `optional` list at `result-validation.ts:907` | 399 / 1 | covered | — |
| a node's geometry (`parseGeometry`) carrying `bounds` is taken and the field survives | drop `"bounds"` from the `optional` list at `result-validation.ts:960` | 384 / 16 | covered | — |
| a style reference (`parseStyleReference`) carrying `name` is taken and the field survives | drop `"name"` from the `optional` list at `result-validation.ts:992` | 399 / 1 | covered | — |
| a variable reference (`parseVariableReference`) carrying `name` is taken and the field survives | drop `"name"` from the `optional` list at `result-validation.ts:1010` | 399 / 1 | covered | — |
| a node summary (`parseNodeSummary`) carrying `parentId` is taken and the field survives | drop `"parentId"` from the `optional` list at `result-validation.ts:1022` | 384 / 16 | covered | — |
| a node summary (`parseNodeSummary`) carrying `childIds` is taken and the field survives | drop `"childIds"` from the `optional` list at `result-validation.ts:1022` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| a node summary (`parseNodeSummary`) carrying `bounds` is taken and the field survives | drop `"bounds"` from the `optional` list at `result-validation.ts:1022` | 384 / 16 | covered | — |
| compact node data (`parseCompactData`) carrying `geometry` is taken and the field survives | drop `"geometry"` from the `optional` list at `result-validation.ts:1050` | 398 / 2 | covered | — |
| compact node data (`parseCompactData`) carrying `constraints` is taken and the field survives | drop `"constraints"` from the `optional` list at `result-validation.ts:1050` | 399 / 1 | covered | — |
| compact node data (`parseCompactData`) carrying `autoLayout` is taken and the field survives | drop `"autoLayout"` from the `optional` list at `result-validation.ts:1050` | 399 / 1 | covered | — |
| compact node data (`parseCompactData`) carrying `text` is taken and the field survives | drop `"text"` from the `optional` list at `result-validation.ts:1050` | 400 / 0 | **gap** | `a get_nodes result carrying every optional field survives parseReadResult` |
| compact node data (`parseCompactData`) carrying `component` is taken and the field survives | drop `"component"` from the `optional` list at `result-validation.ts:1050` | 400 / 0 | **gap** | `a get_nodes result carrying every optional field survives parseReadResult` |
| compact node data (`parseCompactData`) carrying `instance` is taken and the field survives | drop `"instance"` from the `optional` list at `result-validation.ts:1050` | 399 / 1 | covered | — |
| full node data (`parseFullData`) carrying `geometry` is taken and the field survives | drop `"geometry"` from the `optional` list at `result-validation.ts:1090` | 385 / 15 | covered | — |
| full node data (`parseFullData`) carrying `constraints` is taken and the field survives | drop `"constraints"` from the `optional` list at `result-validation.ts:1090` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| full node data (`parseFullData`) carrying `autoLayout` is taken and the field survives | drop `"autoLayout"` from the `optional` list at `result-validation.ts:1090` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| full node data (`parseFullData`) carrying `text` is taken and the field survives | drop `"text"` from the `optional` list at `result-validation.ts:1090` | 395 / 5 | covered | — |
| full node data (`parseFullData`) carrying `component` is taken and the field survives | drop `"component"` from the `optional` list at `result-validation.ts:1090` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| full node data (`parseFullData`) carrying `instance` is taken and the field survives | drop `"instance"` from the `optional` list at `result-validation.ts:1090` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| full node data (`parseFullData`) carrying `strokes` is taken and the field survives | drop `"strokes"` from the `optional` list at `result-validation.ts:1090` | 399 / 1 | covered | — |
| full node data (`parseFullData`) carrying `cornerRadius` is taken and the field survives | drop `"cornerRadius"` from the `optional` list at `result-validation.ts:1090` | 399 / 1 | covered | — |
| full node data (`parseFullData`) carrying `cornerSmoothing` is taken and the field survives | drop `"cornerSmoothing"` from the `optional` list at `result-validation.ts:1090` | 399 / 1 | covered | — |
| full node data (`parseFullData`) carrying `clipsContent` is taken and the field survives | drop `"clipsContent"` from the `optional` list at `result-validation.ts:1090` | 399 / 1 | covered | — |
| full node data (`parseFullData`) carrying `blendMode` is taken and the field survives | drop `"blendMode"` from the `optional` list at `result-validation.ts:1090` | 399 / 1 | covered | — |
| a design node (`parseDesignNode`) carrying `childrenTruncation` is taken and the field survives | drop `"childrenTruncation"` from the `optional` list at `result-validation.ts:1186` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| the get_selection result (`parseSelectionResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1314` | 400 / 0 | **gap** | `a get_selection result carrying every optional field survives parseReadResult` |
| the get_selection result (`parseSelectionResult`) carrying `unresolved` is taken and the field survives | drop `"unresolved"` from the `optional` list at `result-validation.ts:1314` | 398 / 2 | covered | — |
| the get_nodes result (`parseNodesResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1362` | 400 / 0 | **gap** | `a get_nodes result carrying every optional field survives parseReadResult` |
| the get_design_context result (`parseDesignContextResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1413` | 400 / 0 | **gap** | `a get_design_context result carrying every optional field survives parseReadResult` |
| the get_design_context result (`parseDesignContextResult`) carrying `unresolved` is taken and the field survives | drop `"unresolved"` from the `optional` list at `result-validation.ts:1413` | 399 / 1 | covered | — |
| the file metadata (`parseFileMetadata`) carrying `key` is taken and the field survives | drop `"key"` from the `optional` list at `result-validation.ts:1461` | 397 / 3 | covered | — |
| the get_metadata result (`parseMetadataResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1480` | 400 / 0 | **gap** | `a get_metadata result carrying every optional field survives parseReadResult` |
| the search_nodes result (`parseSearchResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1522` | 400 / 0 | **gap** | `a search_nodes result carrying every optional field survives parseReadResult` |
| the search_nodes result (`parseSearchResult`) carrying `nextCursor` is taken and the field survives | drop `"nextCursor"` from the `optional` list at `result-validation.ts:1522` | 399 / 1 | covered | — |
| a **paint** style (`parseStyle`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:1564` | 399 / 1 | covered | — |
| a **paint** style (`parseStyle`) carrying `remote` is taken and the field survives | drop `"remote"` from the `optional` list at `result-validation.ts:1564` | 399 / 1 | covered | — |
| a **paint** style (`parseStyle`) carrying `key` is taken and the field survives | drop `"key"` from the `optional` list at `result-validation.ts:1564` | 399 / 1 | covered | — |
| a **text** style (`parseStyle`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:1577` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| a **text** style (`parseStyle`) carrying `remote` is taken and the field survives | drop `"remote"` from the `optional` list at `result-validation.ts:1577` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| a **text** style (`parseStyle`) carrying `key` is taken and the field survives | drop `"key"` from the `optional` list at `result-validation.ts:1577` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| an **effect** style (`parseStyle`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:1590` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| an **effect** style (`parseStyle`) carrying `remote` is taken and the field survives | drop `"remote"` from the `optional` list at `result-validation.ts:1590` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| an **effect** style (`parseStyle`) carrying `key` is taken and the field survives | drop `"key"` from the `optional` list at `result-validation.ts:1590` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| a **grid** style (`parseStyle`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:1603` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| a **grid** style (`parseStyle`) carrying `remote` is taken and the field survives | drop `"remote"` from the `optional` list at `result-validation.ts:1603` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| a **grid** style (`parseStyle`) carrying `key` is taken and the field survives | drop `"key"` from the `optional` list at `result-validation.ts:1603` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| the get_styles result (`parseStylesResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1622` | 400 / 0 | **gap** | `a get_styles result carrying every optional field survives parseReadResult` |
| a variable mode value (`parseVariableModeValue`) carrying `resolved` is taken and the field survives | drop `"resolved"` from the `optional` list at `result-validation.ts:1658` | 400 / 0 | **gap** | `a get_variables result carrying every optional field survives parseReadResult` |
| a variable mode value (`parseVariableModeValue`) carrying `error` is taken and the field survives | drop `"error"` from the `optional` list at `result-validation.ts:1658` | 399 / 1 | covered | — |
| the get_variables result (`parseVariablesResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1732` | 400 / 0 | **gap** | `a get_variables result carrying every optional field survives parseReadResult` |
| a documentation reference (`parseDocumentation`) carrying `label` is taken and the field survives | drop `"label"` from the `optional` list at `result-validation.ts:1755` | 400 / 0 | **gap** | `a get_components result carrying every optional field survives parseReadResult` |
| a component property definition (`parsePropertyDefinition`) carrying `preferredValues` is taken and the field survives | drop `"preferredValues"` from the `optional` list at `result-validation.ts:1768` | 400 / 0 | **gap** | `a get_components result carrying every optional field survives parseReadResult` |
| a component definition (`parseComponentDefinition`) carrying `componentSetId` is taken and the field survives | drop `"componentSetId"` from the `optional` list at `result-validation.ts:1807` | 400 / 0 | **gap** | `a get_components result carrying every optional field survives parseReadResult` |
| a component definition (`parseComponentDefinition`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:1807` | 400 / 0 | **gap** | `a get_components result carrying every optional field survives parseReadResult` |
| the get_components result (`parseComponentsResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1854` | 400 / 0 | **gap** | `a get_components result carrying every optional field survives parseReadResult` |
| the get_fonts result (`parseFontsResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:1905` | 400 / 0 | **gap** | `a get_fonts result carrying every optional field survives parseReadResult` |
| an annotation (`parseAnnotation`) carrying `categoryId` is taken and the field survives | drop `"categoryId"` from the `optional` list at `result-validation.ts:1919` | 400 / 0 | **gap** | `a get_dev_mode_data result carrying every optional field survives parseReadResult` |
| a dev-mode node (`parseDevModeNode`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:1949` | 400 / 0 | **gap** | `a get_dev_mode_data result carrying every optional field survives parseReadResult` |
| a dev-mode node (`parseDevModeNode`) carrying `descriptionMarkdown` is taken and the field survives | drop `"descriptionMarkdown"` from the `optional` list at `result-validation.ts:1949` | 400 / 0 | **gap** | `a get_dev_mode_data result carrying every optional field survives parseReadResult` |
| a dev-mode node (`parseDevModeNode`) carrying `ownerNodeId` is taken and the field survives | drop `"ownerNodeId"` from the `optional` list at `result-validation.ts:1949` | 400 / 0 | **gap** | `a get_dev_mode_data result carrying every optional field survives parseReadResult` |
| a dev-mode node (`parseDevModeNode`) carrying `inheritedFromNodeId` is taken and the field survives | drop `"inheritedFromNodeId"` from the `optional` list at `result-validation.ts:1949` | 400 / 0 | **gap** | `a get_dev_mode_data result carrying every optional field survives parseReadResult` |
| the get_dev_mode_data result (`parseDevModeResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:2004` | 400 / 0 | **gap** | `a get_dev_mode_data result carrying every optional field survives parseReadResult` |
| a navigate-family reaction action (`parseReactionAction`) carrying `destinationId` is taken and the field survives | drop `"destinationId"` from the `optional` list at `result-validation.ts:2033` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a setVariable reaction action (`parseReactionAction`) carrying `variableId` is taken and the field survives | drop `"variableId"` from the `optional` list at `result-validation.ts:2048` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a setVariableMode reaction action (`parseReactionAction`) carrying `variableCollectionId` is taken and the field survives | drop `"variableCollectionId"` from the `optional` list at `result-validation.ts:2055` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a setVariableMode reaction action (`parseReactionAction`) carrying `variableModeId` is taken and the field survives | drop `"variableModeId"` from the `optional` list at `result-validation.ts:2055` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| an updateMediaRuntime reaction action (`parseReactionAction`) carrying `destinationId` is taken and the field survives | drop `"destinationId"` from the `optional` list at `result-validation.ts:2076` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| an updateMediaRuntime reaction action (`parseReactionAction`) carrying `amountToSkip` is taken and the field survives | drop `"amountToSkip"` from the `optional` list at `result-validation.ts:2076` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| an updateMediaRuntime reaction action (`parseReactionAction`) carrying `newTimestamp` is taken and the field survives | drop `"newTimestamp"` from the `optional` list at `result-validation.ts:2076` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction overlay (`parseReactionOverlay`) carrying `relativePosition` is taken and the field survives | drop `"relativePosition"` from the `optional` list at `result-validation.ts:2143` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction overlay (`parseReactionOverlay`) carrying `positionType` is taken and the field survives | drop `"positionType"` from the `optional` list at `result-validation.ts:2143` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction overlay (`parseReactionOverlay`) carrying `background` is taken and the field survives | drop `"background"` from the `optional` list at `result-validation.ts:2143` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction overlay (`parseReactionOverlay`) carrying `backgroundInteraction` is taken and the field survives | drop `"backgroundInteraction"` from the `optional` list at `result-validation.ts:2143` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `transitionId` is taken and the field survives | drop `"transitionId"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `transitionDuration` is taken and the field survives | drop `"transitionDuration"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `overlay` is taken and the field survives | drop `"overlay"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `timeout` is taken and the field survives | drop `"timeout"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `delay` is taken and the field survives | drop `"delay"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `keyCodes` is taken and the field survives | drop `"keyCodes"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `device` is taken and the field survives | drop `"device"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a reaction (`parseReaction`) carrying `mediaHitTime` is taken and the field survives | drop `"mediaHitTime"` from the `optional` list at `result-validation.ts:2193` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| the get_reactions result (`parseReactionsResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:2278` | 400 / 0 | **gap** | `a get_reactions result carrying every optional field survives parseReadResult` |
| a motion easing (`parseMotionEasing`) carrying `easingFunctionCubicBezier` is taken and the field survives | drop `"easingFunctionCubicBezier"` from the `optional` list at `result-validation.ts:2335` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| a motion easing (`parseMotionEasing`) carrying `easingFunctionSpring` is taken and the field survives | drop `"easingFunctionSpring"` from the `optional` list at `result-validation.ts:2335` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an indexedItem keyframe field (`parseKeyframeField`) carrying `field` is taken and the field survives | drop `"field"` from the `optional` list at `result-validation.ts:2490` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an indexedItem keyframe field (`parseKeyframeField`) carrying `propertyId` is taken and the field survives | drop `"propertyId"` from the `optional` list at `result-validation.ts:2490` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an applied animation style (`parseAppliedStyle`) carrying `duration` is taken and the field survives | drop `"duration"` from the `optional` list at `result-validation.ts:2563` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an applied animation style (`parseAppliedStyle`) carrying `timelineOffset` is taken and the field survives | drop `"timelineOffset"` from the `optional` list at `result-validation.ts:2563` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an applied animation style (`parseAppliedStyle`) carrying `props` is taken and the field survives | drop `"props"` from the `optional` list at `result-validation.ts:2563` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an available animation style (`parseAvailableStyle`) carrying `description` is taken and the field survives | drop `"description"` from the `optional` list at `result-validation.ts:2593` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an available animation style (`parseAvailableStyle`) carrying `props` is taken and the field survives | drop `"props"` from the `optional` list at `result-validation.ts:2593` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| the get_motion result (`parseMotionResult`) carrying `availableStyles` is taken and the field survives | drop `"availableStyles"` from the `optional` list at `result-validation.ts:2684` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| the get_motion result (`parseMotionResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:2684` | 400 / 0 | **gap** | `a get_motion result carrying every optional field survives parseReadResult` |
| an SVG screenshot asset (`parseScreenshotAsset`) carrying `rejection` is taken and the field survives | drop `"rejection"` from the `optional` list at `result-validation.ts:2748` | 399 / 1 | covered | — |
| the get_screenshot result (`parseScreenshotResult`) carrying `truncation` is taken and the field survives | drop `"truncation"` from the `optional` list at `result-validation.ts:2789` | 400 / 0 | **gap** | `a get_screenshot result carrying every optional field survives parseReadResult` |

### Where the duplicated names split

Fifty-four pairs are a name that appears in more than one `optional` list.
Which halves are held, and which are not:

| Optional name | Sites | Verdicts |
|---|---|---|
| `truncation` | 13 result families | **all 13 gaps** — no half pinned anywhere |
| `description` | 4 style types, component definition, dev-mode node, available animation style | paint style covered; **6 gaps** |
| `remote` | 4 style types | paint style covered; **3 gaps** |
| `key` | file metadata, 4 style types | file metadata and paint style covered; **3 gaps** |
| `component` | compact and full node data | **both gaps** |
| `props` | applied and available animation style | **both gaps** |
| `destinationId` | navigate-family and updateMediaRuntime actions | **both gaps** |
| `autoLayout`, `constraints`, `instance` | compact and full node data | compact covered, **full a gap** |
| `text` | compact and full node data | **compact a gap**, full covered |
| `componentSetId` | component value, component definition | component value covered, **definition a gap** |
| `bounds`, `geometry`, `name`, `unresolved` | 2–3 sites each | every half covered |

The three style-identity names are the clearest instance of the shape: one
`STYLE_IDENTITY_FIELDS` constant, read at four `exact()` calls, with the paint
site pinned and the text, effect and grid sites carrying nine unpinned pairs
between them.

The `text` row is the one worth a second look. Compact and full node data share
six optional names, but `text` does not hold the same thing in each: compact
carries a `TextSummary` (`characterCount`, `preview`) and full carries a
`TextValue`. The plugin's two parsers agree with Rust's `CompactNodeData` and
`FullNodeData` on this, so it is not a defect — but it does mean the two lists
are not interchangeable, and the suite happened to pin whichever half the other
one did not.

### Whole families with no acceptance coverage of their optionals

Thirty-eight of the 74 gaps are at a name occurring exactly once in the file,
and the gaps cluster by parser rather than scattering. Every optional field of
`parseReaction` (8),
`parseReactionOverlay` (4), the four `parseReactionAction` arms (7),
`parseDevModeNode` (4), `parseAppliedStyle` (3), `parseAvailableStyle` (2),
`parseMotionEasing` (2) and `parseKeyframeField` (2) came back green. The
reactions, motion and dev-mode families reach `parseReadResult` in no test at
all; their own suites (`reactions.test.ts`, `motion.test.ts`,
`dev-mode.test.ts`) stop at the serializer and never put the result through the
wall that would reject it in production.

### No production defect found

Every mutation here answered a coverage question, not a correctness one. The
`optional` lists at the sites with the most gaps were checked by hand against
their Rust counterparts — `Truncation`, `Reaction`, `ReactionOverlay`,
`AppliedAnimationStyle`, `AvailableAnimationStyle`, `DevModeNodeData`,
`CompactNodeData`, `FullNodeData` — and each names exactly the fields Rust makes
optional (`Option<…>` with `skip_serializing_if = "Option::is_none"`, usually
alongside `default`), with the same types. Nothing to report as a bug.

### Verification

Each of the 74 gaps was verified twice, with the new tests in place:

1. **Its own delete mutation re-applied.** Across all 74, the failing tests were
   new tests and only new tests: one test where the field sits at a single
   family's call site — including each of the six style-identity pairs, every
   one of which turned only the `get_styles` test red — thirteen for the three
   `parseTruncation` counters every family's payload carries, four for
   `childIds` (the four payloads holding a node summary), three for
   `childrenTruncation` (the three node-forest payloads). No pre-existing test
   went red for any of them.
2. **An accept-then-discard mutation**, one per distinct gap field name — 50 of
   them: a clause added to `exact()` that leaves the allow-list untouched, so
   nothing is refused, and strips that key from the record it hands back, so the
   caller copies nothing. This is the mutation a `toBeDefined()` assertion
   cannot see. All 50 turned a new test red. Five of them (`description`,
   `instance`, `text`, `remote`, `key`) also caught a pre-existing test, and
   that union is the whole list of tests in this repository that already assert
   an optional field *survives* rather than merely parses:
   `accepts exact concrete payloads for all 13 Rust result families`,
   `Rust fixtures decode and re-encode without shape drift`,
   `normalizes Rust-defaulted capability fields`,
   `resolves instance compact data via getMainComponentAsync under dynamic-page`,
   and the four `round-trips through the wire validator` line-height tests.

## tests/integration

A gap here means a tool errors on a valid request, or a live plugin session is
dropped, and nothing notices. The seam is a real MCP request travelling through
the production `McpService`, the production broker, and a scripted fake plugin
on a real WebSocket.

The Scope row above is confirmed rather than corrected: re-running Task 1's
command against this tree gives 20 refusal-named tests in `tests/integration`,
of which 4 also name an accept (`still_`, `accepts`, `round_trips`), leaving 16
refusal-only. Those 20 tests, with every indirection expanded, are **46 accept
paths**: `TOOL_NAMES` is fourteen names and `PROMPT_NAMES` three, the screenshot
format rule pairs four fields with two format families, `search must include
query or types` is a disjunction with two satisfying halves, and
`handle_incoming` refreshes session liveness from three separate match arms.
Grouping any of those by their shared constant would have hidden the findings:
every one of the seven gaps is one member of a group whose siblings are pinned.

Every mutation below was applied to production code, measured, and reverted;
the committed diff adds tests only. Baseline before this task: **260 passed /
0 failed**. After: **264 passed / 0 failed**.

**35 covered, 7 gaps, 4 not applicable.**

Only one of Task 2's two shapes appears, and it accounts for every gap. There
is no inclusive-ceiling row here at all — this seam holds no ceilings of its
own — but all seven gaps are the untested member of a rule written in more than
one place:

- `scale` is refused for SVG and accepted for raster in **two** arms, `Png`
  and `Jpeg`. The png half is pinned; the jpeg half was not.
- The three `svg*` options are three separate fields in one arm, and the
  refusal `reject_svg_fields` is called from both raster arms. Not one of the
  three acceptances was pinned.
- `search must include query or types` has two satisfying halves. `query`
  alone is what every other search test sends; `types` alone was unpinned.
- `handle_incoming` refreshes liveness from three arms — text, pong, ping.
  Only the text arm is on the path the suite drives.

The last of those is the one worth reading twice. Turning either control-frame
arm into `NonTextProtocolFrame` unregisters the plugin session on the next ping
or pong a real client sends, and all 260 tests stayed green.

### What the covered rows prove

The fourteen tool rows all rest on `all_tools::every_tool_and_prompt_round_trips_through_mcp_service`
plus one per-family round-trip test, and both use `assert_structured_success`,
which validates the payload against the tool's published output schema and
compares the compatibility text block to the structured content. That is a
survival assertion, not an `is_ok()`, and it was measured rather than read: the
`content::structured` row below empties the payload while keeping the call
successful, and 22 tests go red. Two covered rows are weaker and are marked as
such — `types`/`cursor` survive trimming is held only by
`search_nodes_public_contract_trims_defaults_and_rejects_invalid_values`, a
`serde_json::from_value` unit test that never reaches the seam, and
`SessionRegistry::try_send_to` is held only by a unit test of an API production
does not call at all (see the note after the table).

### Mutations that hang, and why the row is empty

Narrowing `deferred.rs` wedged this binary for Task 2; four mutations here are
in the same family and two were measured doing it. Any mutation that refuses a
whole class of connection — the frontend protocol check, the frontend lease,
the plugin hello, the plugin lease — leaves roughly fifteen integration tests
spinning on an unbounded `while broker.live_file_count().await != 1 { yield }`
loop with no timeout anywhere. Both attempts were killed at 600s with the
`integration` binary still running and the other eleven binaries already green;
nothing is concluded from either. The two remaining mutations of that family
were not attempted. **This is itself a finding: `tests/integration` has no
watchdog on session establishment, so a broker that stops accepting connections
does not fail the suite, it stalls it.**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| A valid `tools/call` for `get_components` reaches its handler | `ToolName::try_from`: `.find(\|name\| name.as_str() == value && *name != Self::GetComponents)` | 258 / 2 | covered — `all_tools::every_tool_and_prompt_round_trips_through_mcp_service`, `components_fonts::components_and_fonts_round_trip_through_server_broker_and_plugin` | — |
| The same for `get_design_context` | the same exclusion, `Self::GetDesignContext` | 258 / 2 | covered — `all_tools::…round_trips…`, `navigation::navigation_tools_round_trip_through_server_broker_and_plugin` | — |
| The same for `get_dev_mode_data` | the same exclusion, `Self::GetDevModeData` | 258 / 2 | covered — `all_tools::…round_trips…`, `dev_mode::dev_mode_reactions_and_motion_round_trip_through_server_broker_and_plugin` | — |
| The same for `get_fonts` | the same exclusion, `Self::GetFonts` | 258 / 2 | covered — `all_tools::…round_trips…`, `components_fonts::…` | — |
| The same for `get_metadata` | the same exclusion, `Self::GetMetadata` | 244 / 16 | covered — `metadata::get_metadata_round_trips_through_server_broker_and_plugin` and 15 more | — |
| The same for `get_motion` | the same exclusion, `Self::GetMotion` | 258 / 2 | covered — `all_tools::…round_trips…`, `dev_mode::…` | — |
| The same for `get_nodes` | the same exclusion, `Self::GetNodes` | 258 / 2 | covered — `all_tools::…round_trips…`, `navigation::…` | — |
| The same for `get_reactions` | the same exclusion, `Self::GetReactions` | 258 / 2 | covered — `all_tools::…round_trips…`, `dev_mode::…` | — |
| The same for `get_screenshot` | the same exclusion, `Self::GetScreenshot` | 256 / 4 | covered — `screenshot::screenshot_round_trips_raster_bytes_and_validated_svg_source` and 3 more | — |
| The same for `get_selection` | the same exclusion, `Self::GetSelection` | 258 / 2 | covered — `all_tools::…round_trips…`, `navigation::…` | — |
| The same for `get_styles` | the same exclusion, `Self::GetStyles` | 258 / 2 | covered — `all_tools::…round_trips…`, `design_system::styles_and_variables_round_trip_through_server_broker_and_plugin` | — |
| The same for `get_variables` | the same exclusion, `Self::GetVariables` | 258 / 2 | covered — `all_tools::…round_trips…`, `design_system::…` | — |
| The same for `list_files` | the same exclusion, `Self::ListFiles` | 251 / 9 | covered — `runtime::the_service_is_up_before_any_election_has_happened` and 8 more | — |
| The same for `search_nodes` | the same exclusion, `Self::SearchNodes` | 256 / 4 | covered — the three `search::search_nodes_without_connection_id_*` tests and 1 more | — |
| Every name `ToolName::ALL` dispatches is also advertised by `tools_catalog()` — two independent enumerations of the same fourteen | delete the `definition::<GetFontsInput, GetFontsResult>` entry from the `tools_catalog()` vec | 249 / 11 | covered — `tools_catalog::tools_catalog_is_complete_sorted_read_only_and_cacheable` and 10 more | — |
| A tool's structured payload reaches the client intact, not merely successfully | `content::structured`: return `CallToolResult::structured({})`, discarding the value | 238 / 22 | covered — every `assert_structured_success` caller; this is what makes the fourteen rows above survival rows | — |
| `prompts/get` resolves `prototype_flow_strategy` with its body | `get_prompt_result`: `prompt_by_name(name).filter(\|p\| p.name != "prototype_flow_strategy")` | 256 / 4 | covered — `prompts::prompts_get_returns_one_user_text_message_and_rejects_unknown_names` and 3 more | — |
| The same for `read_design_strategy` | the same filter, `read_design_strategy` | 256 / 4 | covered — same four | — |
| The same for `style_audit_strategy` | the same filter, `style_audit_strategy` | 256 / 4 | covered — same four | — |
| `resources/read` serves the body at `figma://strategy/prototype_flow_strategy` | `read_resource_result`: `.filter(\|p\| p.name != "prototype_flow_strategy")` after the prefix strip | 257 / 3 | covered — `strategy_resources::resources_read_serves_the_same_text_as_prompts_get` and 2 more | — |
| The same for `read_design_strategy` | the same filter, `read_design_strategy` | 256 / 4 | covered — same, plus `modern_2026_07_28_discover_and_stateless_lists_over_real_stdio` | — |
| The same for `style_audit_strategy` | the same filter, `style_audit_strategy` | 257 / 3 | covered — same three | — |
| A `png` screenshot carrying `scale` is accepted and dispatched with it | `ScreenshotFormatTag::Png` arm: `if scale.is_some() { return Err(…) }` | 258 / 2 | covered — `screenshot::screenshot_round_trips_raster_bytes_and_validated_svg_source`, `all_tools::…round_trips…` | — |
| A `jpeg` screenshot carrying `scale` is accepted and dispatched with it — the second arm of the same rule | the same clamp in the `Jpeg` arm | 260 / 0 | **gap** | `screenshot::jpeg_scale_and_the_three_svg_options_reach_the_plugin_intact` |
| An `svg` screenshot carrying `svgOutlineText` is accepted and the value survives | `Svg` arm: `if svg_outline_text.is_some() { return Err(…) }` | 260 / 0 | **gap** | the same test |
| An `svg` screenshot carrying `svgIdAttribute` is accepted and the value survives | `Svg` arm: `if svg_id_attribute.is_some() { return Err(…) }` | 260 / 0 | **gap** | the same test |
| An `svg` screenshot carrying `svgSimplifyStroke` is accepted and the value survives | `Svg` arm: `if svg_simplify_stroke.is_some() { return Err(…) }` | 260 / 0 | **gap** | the same test |
| A `search_nodes` call carrying `query` and no `types` is accepted | `SearchNodesInput::deserialize`: `query.is_none() && types.is_empty()` → `types.is_empty()` | 256 / 4 | covered — the three `search::search_nodes_without_connection_id_*` tests and 1 more | — |
| A `search_nodes` call carrying `types` and no `query` is accepted — the other half of the same disjunction | the same guard → `input.query.is_none()` | 260 / 0 | **gap** | `search::a_search_carrying_only_node_types_is_dispatched_with_those_types` |
| A `types` list survives trimming into the dispatched input | replace the rebuilt list with `["TEXT"]` | 259 / 1 | covered — `search::search_nodes_public_contract_trims_defaults_and_rejects_invalid_values`, a `from_value` unit test that never reaches the seam | — |
| A `query` survives trimming into the dispatched input | replace the trimmed query with `"Zzz"` | 257 / 3 | covered — the same test plus two that assert it at the plugin | — |
| A `cursor` survives trimming into the dispatched input | `input.cursor = None` after trimming | 259 / 1 | covered — the same `from_value` unit test only | — |
| With exactly one live session and no `connectionId`, the registry selects it | `SessionRegistry::select`: `(Some(session), None) => Selection::Ambiguous` | 257 / 3 | covered — `session_registry::selection_requires_exactly_one_live_session_and_never_falls_back` and 2 more | — |
| An explicit, live `connectionId` selects that session | `select`: `.map_or(Selection::Missing, \|_\| Selection::Missing)` | 226 / 34 | covered — 34 tests, including `multi_client::leader_routes_explicit_calls_and_rejects_ambiguous_omissions` | — |
| Two sessions with the same `fileName` both register | add a `file_name` duplicate check to `SessionRegistry::insert` | 258 / 2 | covered — `session_registry::duplicate_connection_id_is_rejected_but_duplicate_file_name_is_allowed` and 1 more | — |
| `SessionRegistry::try_send_to` delivers to the session's current socket | invert `session.socket_id != expected_socket_id` | 259 / 1 | covered — `session_registry::outbound_queue_reports_full_without_waiting` only, and that API is unreachable from production (see below) | — |
| A WebSocket handshake with `Origin: null` is accepted | `require_null_origin`: `==` → `!=` | 211 / 49 | covered — 49 tests | — |
| A plugin text frame refreshes the session's liveness | delete the `last_seen`/`touch_socket` pair from the `Message::Text` arm | 259 / 1 | covered — `resource_control::progress_resets_inactivity_but_not_the_total_deadline` only | — |
| A WebSocket **pong** frame from the plugin is taken, not treated as a protocol violation — the second of three liveness arms | `Message::Pong(_)` arm → `return Err(BrokerError::NonTextProtocolFrame)` | 260 / 0 | **gap** | `ws_origin::an_unsolicited_control_pong_keeps_the_session_routable` |
| A WebSocket **ping** frame is taken and answered — the third arm | `Message::Ping(_)` arm → `return Err(BrokerError::NonTextProtocolFrame)` | 260 / 0 | **gap** | `ws_origin::a_control_ping_is_answered_and_leaves_the_session_routable` |
| The wire version the shipped plugin announces is the one the broker accepts | `PLUGIN_PROTOCOL_VERSION`: `"4"` → `"5"`, which every Rust fake plugin follows because it imports the constant | 259 / 1 | covered — `contracts::both_ends_declare_the_same_wire_version`, which reads `plugin/src/ui/hello.ts` | — |
| A response arriving while its request is still pending is delivered to the caller | `PendingMap::complete`: drop the result instead of sending it, still returning `true` | 239 / 21 | covered — 21 tests | — |
| A frontend announcing the current `FRONTEND_PROTOCOL_VERSION` is answered `Ready` | `rpc::serve_frontend`: `!=` → `==` | hangs — killed at 600s, `integration` still running | not applicable — not measurable by this mutation | — |
| A frontend lease is granted while the broker is not closing | `Activity::frontend_lease`: `if state.closing` → `if !state.closing` | hangs — killed at 600s, `integration` still running | not applicable — not measurable by this mutation | — |
| A well-formed `hello` first frame registers a session | `handle_socket`: treat a valid `Hello` as `FirstFrameNotHello` | not run | not applicable — same family as the two hangs above; not attempted | — |
| A plugin lease is granted while the broker is not closing | `Activity::plugin_lease`: `if state.closing` → `if !state.closing` | not run | not applicable — same family; not attempted | — |

### The three-armed liveness refresh, and the two arms nothing reaches

`handle_incoming` refreshes `last_seen` and calls `touch_socket` from three
arms of one match — `Message::Text`, `Message::Pong`, `Message::Ping` — and
`Message::Ping` additionally owes the sender a pong. The three are the same
rule written three times, and the suite reaches exactly one of them:

| Arm | Reached by | Verdict |
|---|---|---|
| `Message::Text` | every plugin response and progress frame | covered, by one test |
| `Message::Pong` | nothing — the broker's heartbeat is a JSON `{"type":"ping"}` text frame, so no WebSocket pong ever arrives from a fake plugin | **gap** |
| `Message::Ping` | nothing — no test client sends a WebSocket ping | **gap** |

The consequence is not cosmetic. Both arms fall through to a single
`cleanup_socket`, so turning either into a refusal *unregisters the plugin
session* on the next control frame a real client sends. Nothing in 260 tests
saw it. Both new tests therefore assert more than "no error": each sends its
control frame, then calls `broker.invoke` and reads the request frame back off
the same socket, so the test fails if the session is gone or has stopped
routing.

### `SessionRegistry::try_send` and `try_send_to` are unreachable from production

Inverting the socket-identity comparison in `try_send_to` — which should break
every routed call in the workspace — turned exactly one test red, and that test
is a unit test of the registry. Grepping the callers explains it: production
routes through `Broker::invoke` and `Broker::cancel`, which call
`registry.route_for(connection_id)` and then a free `encoded_plugin_send`
helper in `lib.rs`. Nothing outside `registry.rs` calls `try_send` or
`try_send_to`. The `RouteError::ConnectionChanged` guard inside `try_send_to`
therefore has no production counterpart at all — `route_for` reads the current
socket rather than comparing against an expected one, so there is no second
implementation to disagree with. This is **not** a bug under the ruling (there
are not two implementations that differ); it is dead API carrying a guard, and
a test that pins behaviour the product never executes. Reported, not fixed:
deleting a public API is outside a sweep that adds tests.

### Why this area has no inclusive-ceiling row

Task 2's dominant shape does not appear here. Every bound this seam observes —
envelope size, queue depth, returned-node counts, the raster scale range — is
enforced in `crates/protocol` and was swept in the `tests/contracts` section
above. What `tests/integration` owns is *routing and admission*: which name
dispatches, which session is selected, which frame is accepted on the socket.
Those are membership decisions, not comparisons, and their failure mode is a
missing member rather than an off-by-one — which is why every gap here is one
untested member of a group whose siblings were pinned.

## tests/policy

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

## plugin ui and main

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

## plugin read

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
