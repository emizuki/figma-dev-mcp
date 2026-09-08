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

Citing code: name the test and quote its assertion message, or name the
production function — a `file.rs:NNN` is the one claim in this record no
mutation checks, and a line number into a test file is worse still, because
nothing says whether it means the `assert!` or the message five lines inside
it. Keep a number only where it points into production code and adds something
the name does not; every one that survives has been checked against the tree.

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
| A `NodeIdList` / `PageIdList` / `NodeTypeList` **built** at exactly its ceiling is accepted | `bounded_list_newtype!`'s `TryFrom` (`domain/common.rs:446`): `values.len() > $maximum` → `>=` | 260 / 0 | **gap** — the decode half is in the main table above, the builder half is in neither | none — reported, not fixed |

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
refusal-only. Those 20 tests, with every indirection expanded, are **57 accept
paths** — 58 rows including one found outside the enumeration, see below:
`TOOL_NAMES` is fourteen names and `PROMPT_NAMES` three, the screenshot format
rule pairs four fields with two format families, `search must include query or
types` is a disjunction with two satisfying halves, and `handle_incoming` both
refreshes session liveness from three separate match arms — in two separable
statements each — and dispatches four separate accepted plugin frame kinds.
Grouping any of those by their shared constant would have hidden the findings:
every one of the fourteen gaps is one member of a group whose siblings are
pinned.

Every mutation below was applied to production code, measured, and reverted;
the committed diff adds tests only. Baseline before this task: **260 passed /
0 failed**. After: **269 passed / 0 failed**.

**Reading the `Suite result` column:** this section was measured over three
rounds and each round's rows are baselined on the tree that existed when they
were run, so the totals per row differ by design. Rows measured in round one
sum to **260**, rows added in round two to **264**, and rows added in round
three to **267** or **268** depending on which of that round's two tests already
existed when they ran; the tree now stands at 269. A `gap` row therefore reads
`260 / 0`, `264 / 0`, `267 / 0` or `268 / 0` depending on when it was
measured, and all of them mean the same thing: nothing went red.

**44 covered, 14 gaps, 0 not applicable.**

Only one of Task 2's two shapes appears, and it accounts for every gap. There
is no inclusive-ceiling row here at all — this seam holds no ceilings of its
own — but all fourteen gaps are the untested member of a rule written in more
than one place:

- `scale` is refused for SVG and accepted for raster in **two** arms, `Png`
  and `Jpeg`. The png half is pinned; the jpeg half was not.
- The three `svg*` options are three separate fields in one arm, and the
  refusal `reject_svg_fields` is called from both raster arms. Not one of the
  three acceptances was pinned.
- `search must include query or types` has two satisfying halves. `query`
  alone is what every other search test sends; `types` alone was unpinned.
- `handle_incoming` refreshes liveness from three arms — text, pong, ping.
  Only the text arm is on the path the suite drives, and each arm turned out
  to hold **three** separable properties, not one: is the frame accepted, does
  it refresh the socket-local clock that decides reaping, and does it refresh
  the registry's `last_seen_at` that MCP clients read. That is nine cells and
  eight rows; **seven of the fourteen gaps** are in it. (Eight rows, not nine,
  because the text arm's acceptance has no mutation that does not break the
  whole suite; seven gaps, not eight, because the text arm's socket-local
  refresh is the one row in the table that was already pinned.)
- `handle_incoming`'s text arm dispatches four accepted plugin frame kinds —
  progress, response, error, pong. Three are pinned. The **error** frame is
  not, and the test that looks like it pins it passes for the wrong reason.
- `Broker::shutdown` is two statements, and only the first is pinned — the
  second can be deleted outright with the suite green, because the first
  causes socket teardown to resolve the same pendings with the same error
  code. This one was **not** reachable from this section's enumeration.

Turning either control-frame arm into `NonTextProtocolFrame` unregisters the
plugin session on the next ping or pong a real client sends, and all 260 tests
stayed green. Keeping the frame accepted and deleting only its liveness refresh
is quieter still and equally fatal; deleting only the `touch_socket` half of
that refresh is quieter again and freezes what clients are told rather than
killing the session. Three properties and one mutation each, for exactly that
reason — three rows for each control arm and two for the text arm, eight in
all.

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

### Mutations that stall the run, and the reading rule that cost four rows

Four mutations here refuse a whole class of connection — the frontend protocol
check, the frontend lease, the plugin hello, the plugin lease. Every one of
them leaves roughly fifteen integration tests spinning on an unbounded
`while broker.live_file_count().await != 1 { yield }` loop with no timeout
anywhere, so the `integration` binary never prints its `test result:` line and
a plain `cargo test` run never terminates. **That part is a real finding about
this suite and it stands: `tests/integration` has no watchdog on session
establishment, so a broker that stops accepting connections does not fail the
suite, it stalls it.**

What does *not* follow is that the rows are unmeasurable, and the first pass of
this section wrongly recorded all four as `not applicable` — two killed at 600s
and two never attempted at all, on the inference that they were "the same
family". All four are `covered`, by a wide margin, and the evidence was already
on screen in the killed runs:

> **When a run is killed, the stream is still evidence; only the summary is
> lost.** `cargo test` prints `test … FAILED` per test as it goes and the
> per-binary `test result:` line only at the end. Reading for the summary and
> finding none is not the same as measuring nothing.

Re-running the plugin-lease mutation under plain `cargo test` and killing it at
150s reproduces exactly that: **10 `test … FAILED` lines streamed, 0
`test result:` lines**. Ten genuine assertion failures were sitting in the
output the first pass discarded.

The four rows below were then re-measured under `cargo nextest`, whose
per-test watchdog (`slow-timeout = { period = "15s", terminate-after = 2 }`)
turns the stall into a bounded `TIMEOUT` verdict per test and still reports the
rest, so a whole-workspace answer takes about a minute. Their `Suite result`
cells carry three numbers — passed / failed / timed out — and name the harness,
because a timed-out test is neither a pass nor an assertion failure and
flattening it into either would misreport what was seen. Every other row in
this table is a plain `cargo test` run.

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
| A frontend announcing the current `FRONTEND_PROTOCOL_VERSION` is answered `Ready` | `rpc::serve_frontend`: `!=` → `==` | 246 passed / 15 failed / 6 timed out (nextest, on the 267-test tree; 243 / 15 / 6 on the 264-test tree — same 21 non-passing) | covered — the ten `multi_client::*`, three `failover::*` and two `stdio_eras::*` failures, plus six timeouts; see the note on the two port-sensitive tests below | — |
| A frontend lease is granted while the broker is not closing | `Activity::frontend_lease`: `if state.closing` → `if !state.closing` | 233 passed / 21 failed / 10 timed out (nextest) | covered — all three `idle_lifetime::*`, `runtime::the_service_is_up_before_any_election_has_happened` and 17 more | — |
| A well-formed `hello` first frame registers a session | `handle_socket`: refuse any hello carrying a non-empty `connectionId`, i.e. every real one | 213 passed / 36 failed / 15 timed out (nextest) | covered — `all_tools::every_tool_and_prompt_round_trips_through_mcp_service` and 35 more | — |
| A plugin lease is granted while the broker is not closing | `Activity::plugin_lease`: `if state.closing` → `if !state.closing` | 213 passed / 36 failed / 15 timed out (nextest) | covered — the same 36 | — |
| An omitted `match` on a `search_nodes` call defaults to `contains` | `SearchMatchMode`: move `#[default]` from `Contains` to `Exact` | 262 / 2 (nextest) | covered — `search::search_nodes_public_contract_trims_defaults_and_rejects_invalid_values`, `contracts::tools_catalog::schema_snapshots_are_stable` | — |
| An omitted `limit` on a `search_nodes` call defaults to 50 | `default_search_limit()`: `50` → `51` | 262 / 2 (nextest) | covered — the same two | — |
| A plugin **progress** frame reaches its pending call | `handle_incoming`'s `Progress` arm: short-circuit the `note_progress` call | 262 / 2 (nextest) | covered — `resource_control::progress_resets_inactivity_but_not_the_total_deadline`, `resource_control::bounded_progress_is_forwarded_without_design_content` | — |
| A plugin **response** frame completes its pending call | the `Response` arm: short-circuit the `complete` call | 241 / 23 (nextest) | covered — 23 tests | — |
| A plugin **error** frame completes its pending call — the third of four accepted frame kinds | the `Error` arm: short-circuit the `complete` call | 264 / 0 (confirmed under both nextest and `cargo test`) | **gap** — and the test that looks like it covers it passes for the wrong reason; see below | `ws_origin::a_plugin_error_frame_completes_the_call_with_the_code_the_plugin_sent` |
| A plugin **pong** frame (the JSON heartbeat reply, not the WebSocket control frame) is accepted | the `Pong` arm: `{}` → `return Err(BrokerError::SecondHello)` | 250 / 14 (nextest) | covered — 14 tests | — |
| An accepted WebSocket **pong** refreshes the session's liveness clock — the *other* property of that arm | `Message::Pong` arm keeps accepting and returning `Ok`; only the `last_seen`/`touch_socket` pair is deleted | 264 / 0 | **gap** | `ws_origin::control_pongs_alone_hold_a_session_across_staleness_windows` |
| An accepted WebSocket **ping** refreshes the session's liveness clock, pong reply unchanged | the same deletion in the `Message::Ping` arm, `send_with_shutdown` left in place | 264 / 0 | **gap** | `ws_origin::control_pings_alone_hold_a_session_across_staleness_windows` |
| A **text** frame advances the last-seen the registry reports — the *third* property of an arm, and the one an MCP client reads | delete only `touch_socket` from the `Message::Text` arm, keeping `*last_seen = Instant::now()` | 268 / 0 | **gap** | `ws_origin::every_frame_kind_advances_the_last_seen_the_registry_reports` |
| A **control pong** advances the last-seen the registry reports | the same `touch_socket`-only deletion in the `Message::Pong` arm | 268 / 0 | **gap** | the same test |
| A **control ping** advances the last-seen the registry reports | the same `touch_socket`-only deletion in the `Message::Ping` arm | 268 / 0 | **gap** | the same test |
| `Broker::shutdown` fails every outstanding call itself, rather than leaving them to socket teardown | delete `self.state.pending.lock().await.shutdown();` from `Broker::shutdown` (`lib.rs`) | 267 / 0 | **gap** — a whole production statement removable with the suite green; see below | `ws_origin::broker_shutdown_resolves_pending_calls_itself_not_by_socket_teardown` |

### The three-armed liveness refresh, and the three properties per arm

`handle_incoming` refreshes `last_seen` and calls `touch_socket` from three
arms of one match — `Message::Text`, `Message::Pong`, `Message::Ping` — and
`Message::Ping` additionally owes the sender a pong. The three are the same
rule written three times, and the suite reached exactly one of them. Worse,
"the same rule" is itself three separable properties, and each needed its own
mutation before the table stopped being wrong:

| Arm | Reached by | Accepted at all? | `*last_seen` — reaping | `touch_socket` — reported last-seen |
|---|---|---|---|---|
| `Message::Text` | every plugin response and progress frame | covered (the arm cannot be refused without breaking everything) | covered, by one test | **gap** |
| `Message::Pong` | nothing — the broker's heartbeat is a JSON `{"type":"ping"}` text frame, so no WebSocket pong ever arrives from a fake plugin | **gap** | **gap** | **gap** |
| `Message::Ping` | nothing — no test client sends a WebSocket ping | **gap** | **gap** | **gap** |

Nine cells, eight rows (the text arm's acceptance has no mutation that does not
break the whole suite), and the table got there in three passes — each pass
finding that what it had just called "covered" was a coarser grouping hiding a
finer one. That is the enumeration rule biting at a level the rule's own
wording does not reach: it warns about a *constant* standing for many items,
and says nothing about a two-statement body standing for two properties.

- Pass one wrote **accepted** only. Deleting only the two-line refresh from
  both control arms — frames still accepted, `Ok` still returned, the ping
  still answered — left the workspace at 264/0 with both acceptance tests
  green.
- Pass two added **refreshes the clock**, measured with the pair deleted.
  Deleting only `touch_socket` from any one arm, keeping
  `*last_seen = Instant::now()`, left the workspace at 268/0 with the new
  liveness tests green.
- Pass three splits the pair. `*last_seen` is the socket-local value that
  drives `HeartbeatExpired`; `touch_socket` maintains `Session::last_seen_at`,
  which reaches MCP clients as `LiveFile.lastSeenAt`. The failure modes are
  opposite in kind: the first reaps a live plugin, the second keeps it alive
  while reporting a last-seen frozen at the moment it connected.

The production shape each half misses is different, and both are real. Refusal
falls through to `cleanup_socket`, so a real client's next control frame
*unregisters the session immediately*. A dropped refresh is quieter: a plugin
idle except for browser-level ping/pong keeps the socket open and is reaped by
`HeartbeatExpired` once `stale_after` elapses, with the suite green throughout.

The two liveness tests are written in virtual time (`#[tokio::test(start_paused = true)]`)
against a broker configured with a 2s staleness window and a 200ms heartbeat,
both far under the production ceilings. That is deliberate and it is the whole
design: a wall-clock version of this test asserts "the session is still alive
after N milliseconds", which a loaded machine can falsify while the code is
correct — and it falsifies it *looking exactly like the bug*, which is how a
liveness test gets muted and the row ends up worse than uncovered. Under paused
time the clock moves only where the test moves it: each round opens a 500ms gap
inside a 2s window, six rounds carry the session 3s past registration, and the
smallest margin the passing path ever runs is 1.5s. The failing path is exact —
under the dropped refresh the assertion fires at round 3, the first round past
2.0s. Ten consecutive runs of the `ws_origin` tests and five consecutive runs
of the whole 109-test integration binary were green.

### The error frame: a covering test that passes for the wrong reason

`handle_incoming`'s text arm dispatches four accepted plugin frame kinds, and
they are four accept paths, not one. Progress, response and pong are pinned.
The **error** frame is not, and it is the most interesting row in this section
because it does not look unpinned.
`ws_origin::wrong_socket_response_cannot_complete_a_real_pending_request` sends
an error frame on the owning socket and ends with

```rust
assert!((&mut call.result).await.unwrap().is_err());
```

Short-circuiting the `complete` call in that arm leaves the workspace at
**264 / 0** and that test green. Its broker runs on `Limits::reduced_for_test`,
whose staleness window is 100ms; the test takes 114ms. The socket goes stale,
`cleanup_socket` fails the pending call with `CONNECTION_LOST`, and `is_err()`
is satisfied by *the session dying* rather than by the frame arriving. It is
Lesson 2 with the assertion inverted: not "the value was accepted, therefore
fine", but "an error came back, therefore the right error came back".

The new test asserts the code the plugin actually sent — `NODE_NOT_FOUND`,
which no connection-failure path can produce — on a broker whose staleness
window the test cannot outlive.

### The same shape again, in `Broker::shutdown`, and what it says about the enumeration

`Broker::shutdown` is two statements: cancel the shutdown token, then call
`PendingMap::shutdown()` to fail every outstanding call. Delete the second and
the workspace stays at **267 / 0**, with
`broker_shutdown_and_deadlines_resolve_pending_requests` — a test whose name is
literally *resolve pending requests* — passing in 10ms. Cancelling the token
breaks `ws::serve`'s socket loop, `cleanup_socket` calls
`PendingMap::remove_socket`, and the pendings resolve anyway.

**The old test keeps its assertion, deliberately.** Rewriting an existing test
is out of scope for this sweep — it adds counterparts rather than editing what
is there — and the property is now pinned by the new test beside it. So
`broker_shutdown_and_deadlines_resolve_pending_requests` still asserts nothing
but `is_err()` twice, and that assertion is **undiscriminating**: it is
satisfied by socket teardown as readily as by the shutdown path, and it stays
green under the deletion above. What actually pins
`Broker::shutdown`'s pending-resolution is
`ws_origin::broker_shutdown_resolves_pending_calls_itself_not_by_socket_teardown`.
A reader who deletes that test loses the property with the old test still
green; a pointer comment on the old test says so in place.

Asserting the error code does not help here, and that is what makes this row
worth reading: `PendingMap::shutdown` and `remove_socket` both fail with
`CONNECTION_LOST`, so the fix that worked one row up would produce a third
test passing for the wrong reason. What discriminates is removing socket
teardown from the picture. Aborting the task that owns `ws::serve` drops its
`JoinSet`, so every socket future is dropped at its await point and the
`cleanup_socket` after the loop never runs; the session stays registered and
the pending stays in the map. The new test asserts *that this is what
happened* before it asserts anything else — without that first assertion the
test could silently decay into the shape it replaces.

**This row is the one that says something about the method, not just the
code.** It is not reachable from this section's enumeration at all. Every other
row here descends from one of the 20 refusal-named tests, and
`broker_shutdown_and_deadlines_resolve_pending_requests` is not one of them —
it is accept-named. So the refusal-name regex is a **proxy** for the accept
surface, not a cover of it, and this is a measured instance of the blind spot
rather than a suspected one: a whole production statement, removable with the
suite green, that no amount of care inside the enumeration would have reached.
Fixed here because it is one test of a class this section had just fixed one
row over; the general question — what else lives outside the proxy — is left to
the whole-branch review rather than chased here.

### The two port-sensitive tests, and a cell that took three runs to settle

The frontend-version row's cell was recorded as `243 / 15 / 6` on the 264-test
tree and independently measured elsewhere as `241 / 17 / 6` — two more
non-passing tests. Re-measuring twice on the 267-test tree gives
`246 / 15 / 6` both times, with an identical list of 21 non-passing tests, so
the discrepancy is stable on each side rather than noise.

The two tests that differ are `stdio_eras::legacy_2025_11_25_initialize_and_lists_over_real_stdio`
and `stdio_eras::modern_2026_07_28_discover_and_stateless_lists_over_real_stdio`.
Both bind the **fixed** production ports 3056 and 3057. A controlled check
settles it: holding those two ports from an unrelated process and running those
two tests **at HEAD, with no mutation applied at all** fails both, and
releasing the ports makes both pass again. Which production-port assertion
fires depends on how far each test gets before the squatted port stops it, so
expect either — `"SIGTERM/EOF cleanup must release production listeners"`, the
cleanup assertion, or `"production binary must bind the plugin and frontend
listeners"` in `assert_ports_bound`, the earlier one a squatted port stops the
spawn at. Both live in helpers in `crates/figma-dev-mcp/tests/stdio_eras.rs`;
grep the message rather than a line number, which is why this note names no
line. Their outcome under any mutation therefore says more about what
else is on the machine than about the mutation. The recorded cell is the
mutation-attributable set — the 21 that reproduce — and the difference is named
here rather than split between two numbers with no explanation.

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

The Scope row above says 8 refusal-only and 0 already paired, and re-running
the plan's command confirms it: 8 names match, none of them also matches the
accept regex. The row is correct and is also beside the point here, and the
reason it is beside the point is decision 3 of the plan.

A `tests/policy` scan runs against the real repository, so the repository is
the fixture. If a scan broadened, it would fire on compliant code and go red on
the spot — which makes the spec's "no meaningful opposite" exemption look
applicable. It is not. **The failure mode of a policy scan is not firing on
compliant code; it is passing on nothing.** A walk that opens zero files
returns `""`, and `!"".contains(anything)` is true of every forbidden string
there is. From the outside, a scan that found nothing wrong and a scan that
looked at nothing are the same green tick.

So the accept path here is *"the scan reaches its inputs and its predicate
still fires"*, the mutation is *"empty the scan"*, and the enumeration is over
**every** test in the suite rather than over the eight refusal-named ones. The
suite is 31 tests in six modules; they yield 42 accept paths at the granularity
where one mutation answers one row, because several tests own more than one
emptyable input — a walk *and* the constant it iterates, a corpus *and* the
predicate applied to it.

Baseline before this task: **269 passed / 0 failed** across the workspace,
**31 passed / 0 failed** in the policy binary. After: **279 / 0** and
**41 / 0**.

Rows: 42. Verdicts: 18 gap, 21 covered, 3 not applicable (18 + 21 + 3 = 42).
Tests added: 10 — nine answering the 18 gaps, plus one new scan that answers a
question Task 4 left open rather than a row in this table.

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| `TOOL_NAMES` is exactly the fourteen, in order | none available: the assertion is an equality against a fourteen-element literal, which no emptying satisfies | — | not applicable | — |
| `PROMPT_NAMES` is exactly the three, in order | the same shape | — | not applicable | — |
| `extracted_tool_references` picks backticked snake-case verbs out of a sample | the same shape: an equality against a five-element literal set built from a literal string | — | not applicable | — |
| The plugin manifest is read and compared as JSON | replace the parsed value with `json!({})` | 30 / 1 | covered — `manifest_is_the_exact_read_only_loopback_surface` | — |
| The Dev-Mode/inspect/dynamic-page/loopback fields are read from that manifest | the same replacement in the second reader | 30 / 1 | covered — `manifest_is_dev_mode_inspect_dynamic_page_loopback` | — |
| `read_plugin_bundles` returns both artifacts when both exist | make it always return `Err` | 30 / 1 | covered — `bundle_policy_requires_both_artifacts` already ends in an `expect` on the Ok case, which is a positive control | — |
| The controller and transport walks read real source (`plugin_contexts_keep_network_and_figma_apis_separate`) | point the walk root at an empty tree — see the note below on why this needed a code change to be catchable | 31 / 0 | **gap** | `manifest::the_plugin_context_walks_reach_real_source_and_the_separation_predicate_fires` |
| The `plugin/src` walk behind the dispatch-closure scan reads real source, and reads the dispatcher | point the walk root at an empty tree; separately, reduce `dispatch.ts` to `export {}` | 31 / 0; 39 / 2, the two red being this control and the dispatcher control | **gap** | the same test |
| The `plugin/src` walk behind `plugin_source_rejects_unbounded_page_font_and_mutation_surfaces` reads real source | the same, in `plugin_source.rs` | 31 / 0 | **gap** | `plugin_source::the_plugin_source_walk_reaches_real_files_and_the_assignment_predicate_fires` |
| `has_property_assignment` can report an assignment | `&& false` in front of its `any` predicate, so it never reports one | 40 / 1, and the one red is the new control | **gap** | the same test |
| The prompt-body scan opens a distinct body per name | `prompt_body` ignores `name` and returns `read_design_strategy.md` | 31 / 0 | **gap** | `prompts::the_prompt_body_scan_reads_a_distinct_body_per_name_and_its_predicates_fire` |
| `prompts.rs`'s `instructs` can report an instruction | `&& false` in front of its predicate | 40 / 1, and the one red is the new control | **gap** | the same test |
| `contracts/common.rs` is read and its wrapper shape asserted | replace that read with `String::new()` | 30 / 1 | covered — `public_contract_wrappers_keep_protocol_types_behind_crate_private_conversions` asserts positive shapes as well as negative ones | — |
| `contracts/visual.rs` is read | the same, for that file only | 30 / 1 | covered — same test | — |
| `dispatch.rs` is read | the same, for that file only | 30 / 1 | covered — same test | — |
| The tools catalog is enumerated for names and annotations | replace `catalog.tools` with an empty vec | 30 / 1 | covered — `snapshots_lock_tools_annotations_prompts_and_wire_variants` | — |
| The generated error catalog yields its canonical messages | none needed: the test already floors the parse at 17, and 17 is exactly what the catalog yields | — | covered — the floor is a real positive control, and this sweep re-measured the number behind it | — |
| The `plugin/src` walk that looks for those messages reads real source | point the walk root at an empty tree | 31 / 0 | **gap** | `read_only::the_canonical_message_scan_reaches_the_catalog_and_every_production_file` |
| The `plugin/src` walk behind `plugin_source_denies_mutation_private_and_motion_write_apis` reads real source | the same, in `read_only.rs` | 31 / 0 | **gap** | `read_only::the_mutation_denylist_scan_reaches_real_plugin_source_and_its_list_is_not_empty` |
| `MUTATION_DENYLIST` enumerates the 23 surfaces its two consumers scan for | replace the constant with `&[]` | 31 / 0 | **gap** | the same test |
| The read-test walk reaches `plugin/src/read` | point it at an empty directory | 30 / 1 | covered — `read_tests_share_one_figma_harness`, the template this task copies, and the only scan in the suite that already had this control | — |
| `collect_property_names` collects property names from the real schemas | keep the call and clear the result | 31 / 0 | **gap** | `read_only::the_input_schema_scan_collects_names_from_every_level_it_claims_to_reach` |
| `FORBIDDEN_INPUT_KEYS` enumerates the 19 keys that scan rejects | replace the constant with `&[]` | 31 / 0 | **gap** | the same test |
| A well-formed broker request carrying an allowlisted operation decodes | in the test's own envelope, `requestId` → `requestIdTypo`, so every decode below fails on the envelope instead | 31 / 0 | **gap** — and see below: the same row measured against production is worse | `read_only::a_broker_request_carrying_an_allowlisted_operation_still_decodes_intact` |
| `crates/broker/src/ws.rs` is read and its origin handling asserted | replace that read with `String::new()` | 30 / 1 | covered — `origin_socket_and_rpc_boundaries_stay_raw_tcp_and_null_origin` asserts `Origin` and `null` are present, not only that other things are absent | — |
| `crates/broker/src/rpc.rs` is read | the same, for that file only | 30 / 1 | covered — same test, via `encode_frame`/`read_frame` | — |
| `plugin/src/main/dispatch.ts` is read and is the dispatcher | point the read at an empty file | 31 / 0 | **gap** | `read_only::the_read_dispatcher_source_still_names_every_operation_it_dispatches` |
| `OPERATOR_DOCS` enumerates the four documents `documentation_required_operator_files_exist` checks | `OPERATOR_DOCS.iter().take(0)` | 31 / 0 | **gap** | `read_only::the_operator_documentation_scan_reaches_all_four_files_and_instructs_still_fires` |
| The operator-doc corpus is read for ports, tools, prompts and the README tables | `require_file` keeps reading and returns `String::new()` | 21 / 10 | covered — `documentation_states_exact_ports_tools_and_prompts` and 9 more | — |
| The same corpus is read for the strategy resource URIs | the same | 21 / 10 | covered — `documentation_states_the_exact_strategy_resource_uris` | — |
| `crates/tools/src/service.rs` is read and its resource handlers located | the same | 21 / 10 | covered — `serving_a_strategy_resource_never_reaches_the_broker` already floors `handler_body` at non-empty; the real bodies are 222 and 410 bytes | — |
| The setup and README text is read for the Dev-Mode import story | the same | 21 / 10 | covered — `documentation_covers_dev_mode_import_connection_selection_and_no_daemon` | — |
| `docs/testing.md` is read for the verification commands | the same | 21 / 10 | covered — `documentation_lists_all_seven_local_verification_commands` | — |
| `SEVEN_LOCAL_VERIFICATION_COMMANDS` enumerates seven | replace the constant with `&[]` | 31 / 0 | **gap** — the test is named for the count and never checked it | `read_only::the_operator_documentation_scan_reaches_all_four_files_and_instructs_still_fires` |
| The corpus is read for the SVG, read-only and origin caveats | `require_file` returns empty | 21 / 10 | covered — `documentation_states_svg_source_readonly_limits_and_origin_threat_model` | — |
| The corpus `documentation_forbids_local_export_instructions_and_unadvertised_product_tools` scans is the four documents | replace that test's corpus with `String::new()` | 31 / 0 | **gap** | the same operator-documentation control |
| `read_only.rs`'s `instructs` can report an instruction | `&& false` in front of its predicate | 40 / 1, and the one red is the new control | **gap** — measured: all eight forbidden phrases occur **zero** times in 59903 bytes of operator docs, so nothing real ever asks it a question it could answer wrong | the same control |
| `DIAGNOSTIC_CALL_NAMES` enumerates the three unadvertised calls | replace the constant with `&[]` | 31 / 0 | **gap** | the same control |
| `docs/testing.md` is read for the evidence split | `require_file` returns empty | 21 / 10 | covered — `documentation_splits_stdio_evidence_from_official_lifecycle_smoke` | — |
| `docs/manual-acceptance.md` is read for the nine scenarios | the same | 21 / 10 | covered — `documentation_manual_acceptance_has_nine_spec_scenarios`, which also floors the checkbox count at nine | — |
| `.gitignore` is read | the same | 21 / 10 | covered — `documentation_gitignore_keeps_lockfiles_and_snapshots_tracked` | — |
| `.github/workflows/ci.yml` is read | the same | 21 / 10 | covered — `documentation_ci_pins_runtimes_and_never_publishes` | — |

### The template, and what copying it actually required

`read_tests_share_one_figma_harness` is the one scan in this suite that already
had this control, and its six assertions are the shape everything above copies.
Its assertion 5 is the lesson: the test originally carried only the *derived*
rule — a file must import the harness **if** it textually installs a host —
and after the migration seven of the nine migrated files install no host
textually, so the gate never evaluated true and moving a builder into a sibling
module defeated the test with the suite green.

**A rule gated on a condition is only as good as how often that condition
holds.** So every control added here was written against a measurement of what
its gate actually evaluates to on the real inputs, and three of those
measurements changed what got written:

| Gate | Evaluations on real input | How often the discriminating branch fires | Consequence for the control |
|---|---|---|---|
| `instructs` in `read_only.rs` | 8 phrases × 59903 bytes of operator docs | **0** — no phrase occurs at all, so the negation-prefix branch never runs | the predicate is proved on planted text, in both directions; nothing real can prove it |
| `instructs` in `prompts.rs` | 12 phrases × 3 bodies = 36 | **1** — `prototype_flow_strategy` says "Do not create connector nodes" | that one occurrence is pinned by name, because it is the only thing standing between the phrase list and a red suite; the accepting direction is planted |
| `contains_ident` in `prompts.rs` | 23 denied names × 3 bodies = 69 | **3** — every body contains `get_node` inside `get_nodes`, and the boundary check correctly answers no | the rejecting direction is pinned on the real bodies; the accepting direction is planted |
| `has_property_assignment` in `plugin_source.rs` | 4 properties × 32 production files | **0** — no file assigns any of them, so neither the `==` exemption nor `code_lines`'s comment filter ever runs | all four directions planted |
| `collect_property_names` recursion | 14 schemas | `properties` 58, `oneOf`/`anyOf`/`allOf` 37, `$defs` 13, `items` 19 — all four fire | the control asserts the names only the deep branches reach, and plants a forbidden key four levels down |
| The citation scan's three rules | 54 distinct citations | **0** for all three: every citation resolves, is in range, and points at code | all three planted, and one of them found a real defect (below) |

The `collect_property_names` row is the one where measuring changed the
control rather than just justifying it. The first draft asserted `nodeIds` and
`selection` on every tool but three, and went red: `search_nodes` reaches only
`nodeId` and `pageId` below its top level, and `get_screenshot` has no page
selector at all. The rewritten control asserts the *union* of names no schema
exposes at top level — `nodeId`, `nodeIds`, `pageId`, `pageIds`, `selection` —
together with the counts of tools that reach them (10 of 14 reach at least one,
9 reach `selection`). Two of the fourteen schemas were miscounted by reading
and correct only after measuring.

One vacuity is recorded rather than fixed: `list_files` has no input properties
at all, so the forbidden-key check over it is empty by construction. That is a
property of the schema, not of the scan, and the control asserts it stays true
so that it stops being invisible.

### The wrong-reason defect, and why the test-side mutation understates it

`write_shaped_mcp_and_wire_requests_are_rejected` asserts that seven
write-shaped operations fail to decode as a `BrokerToPlugin`, inside an
envelope the test writes by hand. Misspelling one envelope key in that literal
— `requestId` → `requestIdTypo` — makes all seven decodes fail on the envelope
instead, and the policy suite stayed at **31 / 0**.

That is a test-code mutation, and on its own it would be a weak row. The
production-side version is not weak. Adding `#[serde(rename = "deadline_ms")]`
to `wire::Request`'s `deadline_ms` field — so the broker stops accepting the
camelCase key every real request carries, and therefore stops accepting *any*
plugin request at all — leaves the existing test **green**. Only the new
control goes red (**40 / 1**). Every refusal that test makes was being
satisfied by an envelope that no longer decodes, not by an operation that is
not allowlisted.

The same shape appears one row over. Reducing `plugin/src/main/dispatch.ts` to
`export {}` — the dispatcher gone entirely — leaves
`the_read_dispatcher_mutates_no_process_global_host_state` green, because 23
forbidden surfaces are all absent from an empty file. **39 / 2**, and both red
are new controls: the dispatcher control, and the manifest control that reads
`dispatch.ts` directly. That second one is a correction — it first asserted
`get_metadata` and `get_screenshot` were somewhere in the whole `plugin/src`
tree, under a message about the dispatch table, and those names are in the read
handlers too, so it was green with the dispatcher deleted. A message that
overclaims what its assertion checks is the wrong-reason class in miniature,
and it appeared inside the section that names the class.

Both are Task 4's wrong-reason class, and both are sharper here than they were
there: in `tests/integration` the wrong reason produced a plausible-looking
error; here it produces a *pass*.

### Two implementations of `instructs` that disagree, reported not fixed

`prompts.rs` and `read_only.rs` each define a private `instructs(haystack,
phrase)`, and they exempt different negation prefixes. `prompts.rs` exempts
three — `do not`, `never`, `without`. `read_only.rs` exempts six — those three
plus `does not`, `no`, `not`. Four phrases appear in **both** modules' lists
(`save screenshots`, `export frames`, `write to disk`, `save to a path`), so on
those four the two implementations answer differently: `does not save to a
path` is an instruction to one and not to the other.

Probed rather than read: on `does not save to a path`, `no save to a path` and
`not save to a path`, `prompts.rs`'s version returns true and `read_only.rs`'s
returns false. **No live input distinguishes them today** — the prompt bodies'
one hit is "Do not create connector nodes", which both exempt, and the operator
docs contain none of the eight phrases at all.

Raised rather than fixed, under the ruling that a test written today would
freeze the disagreement — **and the first draft of these controls froze it
anyway, one-directionally, which is the more useful half of this note.**
`read_only.rs`'s control asserted `!instructs("does not save to a path", …)`,
and `does not` is exactly one of the three prefixes the two implementations
disagree about. Measured at the time: reconciling toward the shorter list cost
a test edit (40 / 1) while reconciling toward the longer one was free (41 / 0).
That is worse than freezing symmetrically — it puts a thumb on the scale of a
decision this sweep has no standing to make, in a test whose stated subject is
something else entirely.

That assertion is gone. Both controls now pin `instructs` only on `do not` and
the plain accepting direction — prefixes **both** implementations agree about —
which is exactly Task 5's property and nothing more. Re-measured after the
deletion: **both** reconciliation directions are now free at 41 / 0. The
disagreement is as open to being fixed either way as it was before this task
existed, which is what "raised, not fixed" is supposed to mean.

The general lesson, since it cost a review round: a positive control asserts a
predicate *fires*, and the sample it fires on carries opinions. Choosing that
sample from the region two implementations disagree about turns a control into
a vote.

### The record's own citations, and the one that did not resolve

Task 4 established this file's citation convention and left it with a known
hole: 53 `result-validation.ts` line citations survive on the basis that they
"have been checked against the tree", and nothing runs that check. This section
adds a scan that does — `every_file_line_citation_in_the_acceptance_record_resolves_to_a_real_line`
— and on its first run **one of the 54 citations in this record did not resolve
under its rule.**

The `bounded_list_newtype!` row cited a bare `common.rs`, and two tracked files
carry that basename — `crates/protocol/src/domain/common.rs`, which is the one
meant, and `crates/tools/src/contracts/common.rs`, which the row two sections
above is about. The citation is qualified to `domain/common.rs` now. That is
the only change this task makes to another task's row, it changes no verdict,
and it is recorded here rather than made quietly.

**Two rules, both correct, and the difference is the interesting part.** Task
4's round-4 review had already examined this same citation *by reading*, cited
the same two files and the same evidence — the contracts file is 77 lines, so
it has no line 446 — and concluded the citation was unambiguous. That is true
under a resolution rule that consults the line number. This scan resolves by
path suffix *before* consulting the line number, and under that rule the
citation is ambiguous. Neither reading is wrong on the facts; they answer
different questions, and the qualified path satisfies both. The scan's value
here is not that it found something reading could not — reading found it first
— but that it will keep finding it, on every run, without anyone remembering to
look.

What the scan checks, and only this: the cited path resolves to exactly one
file in the tree, the line number is in range, and the line is neither blank
nor a lone bracket. It cannot check that a line *means* what a row says it
means; the record's cells mix prose, quoted markdown and source identifiers,
and an attempt to anchor each citation to a token near it needed a ±10-line
window on a 2884-line file before it matched everything — a window that wide
discriminates almost nothing, so it was not built.

Its sensitivity is measured, not assumed — **and the direction matters, which
this section got backwards the first time.** Simulating a uniform shift of the
53 `result-validation.ts` citations and counting how many land on a blank line
or a lone bracket:

| lines | **deletion** above (cited `n` now shows old `n+k`) | **insertion** above (cited `n` now shows old `n−k`) |
|---|---|---|
| 1 | 0 of 53 — **silent** | 2 of 53 — fires |
| 2 | 0 of 53 — **silent** | 33 of 53 — fires |
| 3 | 2 of 53 — fires | 30 of 53 — fires |
| 5 | 41 of 53 — fires | 18 of 53 — fires |

So the hole is on **deletion** at one and two lines; insertions are caught from
a single line up. Both directions confirmed against the real tree, not only
simulated: inserting one blank line at the top of `result-validation.ts` turns
the scan red and nothing else, and deleting one line from the top leaves the
suite green at 41 / 0.

The first version of this paragraph claimed the opposite — silent at one- and
two-line *insertions* — because the simulation shifted the citation numbers
upward, which models a deletion, and the confirming experiment inserted five
lines, which fires in **both** columns and so distinguished nothing. A
five-line change cannot confirm either direction; that is why it did not catch
the error.

It is a tripwire with a measured hole, not a verifier. Saying exactly which
hole is the point: "these have been checked against the tree" was a claim with
no verification story, and replacing it with a second unchecked claim would be
no better.

Like every scan in this suite it has its own positive controls, and they were
necessary: all three of its rules return "fine" on all 54 real citations, so
none of them is exercised by the repository. The parser is proved on a planted
string carrying both citation forms and on a string carrying none; the
resolution rule is proved by asserting that two files named `common.rs` really
do exist, so that "resolves to exactly one" is a rule that can say no rather
than a decoration.

### What this section did not close

Three limits, each stated in the test that carries it rather than only here.

**The controls used to walk the tree a second time, and that made four rows
above false until a fix round corrected it.** Five production-source scans walk
`plugin/src`, and each originally computed its own root; so did each control.
With two roots, redirecting a *scan's* walk at an empty tree left its control
walking the real one and reporting success over a scan that had read nothing —
measured at **41 / 0** on every one of those rows, while the rows claimed the
mutation was caught. That is the template's own original defect wearing a new
coat, inside the task written to prevent it, and it is worth stating plainly
because the disclosure at the time pointed the opposite way from the table, and
a table is what a reader trusts.

Closed by extraction rather than by rewriting anything: one
`plugin_source_root()` that every scan and every control now reads. It moves no
assertion and changes no scan's behaviour, and after it the walk-root mutation
turns all four controls red (**37 / 4**). The row-level verification for each is
in the fix report.

**And the first attempt at that fix left a residual that was not real.** The
canonical-message scan kept its own inline copy of the root, the recursion and
the filter, and this section said the tie was impossible there — no concatenated
text to compare against, so nothing to tie — and gave it a gap row of its own. A
reviewer closed it in three commands. The inline skip condition was De Morgan-identical
to the `is_production_typescript` the controls already used, so the walk could
simply *be* the shared one; the exempted catalog is now named by one constant
both the scan and its control read. Behaviour-preserving, and verified as such
rather than asserted: instrumented at both versions, the scan opens the same 31
files, lists identical. Every way of making it see less is now measured and
caught — root redirected 37 / 4, filter narrowed 37 / 4, recursion removed
37 / 4, exemption broadened 39 / 2, exemption redirected 40 / 1 (that last one
red on the scan itself, which then reads the catalog and finds all 17 messages
in it). The gap row is gone, which is why this section counts 42 rows and not 43.

The lesson is this area's own, one level up. **An impossibility claim needs a
demonstration exactly as much as a coverage claim does.** "No tie exists here"
was reasoning from the one tie the neighbouring scan happens to use, and it was
wrong. Where this record says something cannot be pinned, it now says what was
tried.

What actually remains is duplication without vacuity: `production_typescript` is
still written out three times, in `manifest.rs`, `plugin_source.rs` and
`read_only.rs`. Each copy's narrowing is caught — the mutation-denylist control
asserts the scan's own concatenated text is exactly as long as the sum of the
files the control enumerated, so two disagreeing filters make the totals
disagree, and the two module-local filters empty the text the `WebSocket`,
`figma.` and `has_property_assignment` assertions read. Deduplicating them is a
change to three scan bodies rather than an extraction of a shared expression,
which is past what this sweep does.

**Floors, not equalities.** The production-file count (32 measured), the
property-name total (89 measured) and the citation count (123 occurrences
measured) are all floors. The failure being pinned is a scan that *shrinks*; a
tree that grows is not a defect, and an equality there would be churn that
teaches a reader to update the number without thinking about it.

**Three rows are `not applicable` and the reason is uniform.** The two
allowlist tests and the extraction test assert equality against a literal — a
fourteen-element array, a three-element array, a five-element set built from a
literal sample. There is no input to empty: the assertion carries its own
expected value, so a scan that saw nothing would compare nothing against
fourteen names and go red. That is the one shape in this suite that cannot pass
vacuously, and it is worth naming as the shape the rest of the suite does not
have.

## plugin ui and main

A gap here means the relay or the controller refuses, drops or garbles a
well-formed message. This is the last hop before Figma on one side and the
broker on the other, so a refusal here is a session that stops working with no
error anyone can read.

The area is `plugin/src/ui` (`relay.ts`, `socket.ts`, `hello.ts`, `uuid.ts`,
`raster.ts`, `css-syntax.ts`, `svg.ts`, `index.ts`) and `plugin/src/main`
(`dispatch.ts`, `cancellation.ts`, `progress.ts`, `code.ts`) — 2,312 lines of
production code against 58 tests. It groups into **310 accept paths**: `main`
94 and `ui` 216, one row per mutation that answers exactly one row. Every
mutation below was applied to production code, built, measured and reverted;
the committed diff adds tests only. Baseline before this task: **413 pass / 0
fail / 1854 expect() calls / 26 files**. After: **476 pass / 0 fail / 2100
expect() calls / 28 files**.

**136 covered, 163 gaps, 11 not applicable.** 136 + 163 + 11 = 310. By half:
`main` 94 rows — 35 covered, 52 gaps, 7 not applicable; `ui` 216 rows — 101
covered, 111 gaps, 4 not applicable; 52 + 111 = 163. The new tests close **152**
of the 163 gaps and 11 stay open; 152 + 11 = 163.

The Scope table's `plugin/src/ui` 24 and `plugin/src/main` 5 are right as the
plan defines them, and the brief's own Step-1 command disagrees with them for a
reason worth writing down: the plan's command subtracts an acceptance-word
filter (`accept|round-trip|preserv|surviv|still |keeps|returns`) and the
brief's does not. Re-run against the tree at `57f1474`, the brief's command
finds **27** refusal-named tests in `ui` and **5** in `main`; the plan's finds
**24** and **5**. Both are counts of the same 58 tests, and neither is the row
count: the rows come from the production code, not from the test names.

### The largest finding: `main/code.ts` was never imported by anything

`plugin/src/main/code.ts` is the controller entry point. It is what Figma
loads; it installs the `figma.ui.onmessage` every message from the iframe
arrives at, it unwraps the three transport shapes that message can take, it
answers the readiness handshake, and it posts whatever the dispatcher returns.
**Not one of its sixteen mutations moved the suite** — including deleting the
`figma.showUI` call, never dispatching a request at all, and answering
readiness with an empty file name. Fourteen are gaps; the other two are
mutations nothing outside the module can observe, and are argued below.

Nothing imported it, and the reason is mechanical rather than accidental:
`plugin/tests/figma-harness.ts` says so in its own header — "`detectCapabilities()`
… is reached from `navigation.ts` and `main/code.ts`, neither of which any of
the nine migrated files exercises". The module also runs `figma.showUI(__html__, …)`
at import time, so a test has to install the host *before* importing it, which
is why the new `code.test.ts` builds its fake host at module scope rather than
in a `beforeEach`. It also needs the `figma` global's real typings, which the
tests project deliberately does not load, so it gets its own tsconfig project
(`tsconfig.controller-tests.json`) rather than loosening the one that keeps
`main`'s environment assertions honest.

### The relay's screenshot path had no acceptance coverage at all

`ui/relay.ts` has 23 rows: **20 gaps**, 2 covered, 1 not applicable. The two
that are covered — `sendToController` posting inside a `pluginMessage`
envelope, and `onControllerMessage` handing a parsed message to `receive` — are
covered only incidentally, by `socket.test.ts` driving the socket through the
relay.

Everything the relay does for a screenshot — recognising a `validateScreenshot`
request, encoding the raster, attaching the SVG verdict, and replying
`screenshotValidated` — was unpinned end to end. The whole branch could be
deleted and the suite stayed at 413 / 0. That is the plugin half of the
screenshot pipeline: the controller sends the export across, and this is what
sends the validated asset back.

Two details of that block are where a silent break would land. `asBytes`
accepts four transfer shapes (`Uint8Array`, `ArrayBuffer`, any typed-array
view, a plain number array) and none was pinned; Figma's `exportAsync` returns
a `Uint8Array`, but the value crosses a `postMessage` boundary, where the shape
depends on the host. And the relay's listener registers for `"message"`:
renaming that event to `"messageX"` left the suite green, because
`socket.test.ts`'s fake `window.addEventListener` takes `(_type, listener)` and
ignores the type.

### Eleven of thirteen read operations are not routed anywhere in particular

`dispatchRead`'s `switch` has thirteen `case` arms, one per operation in
`OPERATION_NAMES`. Deleting an arm drops the request through to `assertNever`,
which throws, which the request boundary turns into `INTERNAL_ERROR` — a
request the plugin can serve, answered as if the plugin were broken. **Eleven
of the thirteen deletions left the suite green.** Only `get_metadata` (held by
`get_metadata returns bounded file and page metadata`) and `get_screenshot`
(held by `cancelling a read that never resolves settles the dispatch as
CANCELLED promptly`) were pinned, and the second only because that test happens
to route a screenshot.

The `plugin/src/read` tests do not close this: they call the readers directly.
Nothing between `dispatchControllerMessage` and the reader was checked.

And the one test in this repository that walks `OPERATION_NAMES` — `every named
milestone operation returns a typed unavailable error` — **skips all thirteen
of them.** Its loop body opens with an `if` chain that `continue`s on
`get_metadata`, `get_selection`, `get_nodes`, `search_nodes`,
`get_design_context`, `get_styles`, `get_variables`, `get_components`,
`get_fonts`, `get_dev_mode_data`, `get_reactions`, `get_motion` and
`get_screenshot`, which is every name the list holds, so the body never runs
and the test asserts nothing. This is the vacuous-scan shape Task 4 named,
found here in a suite rather than in a scan. It is left in place — the sweep
does not rewrite existing tests — and its acceptance counterpart, `every read
operation reaches the reader named in the request`, carries the positive
control the old one lacks: it counts its iterations and asserts the count is
thirteen.

That test asserts more than the response's label. Each reader returns a
differently shaped result, so the shape is what says which reader ran: the test
checks the exact set of top-level fields for all thirteen, and for the three
whose field sets coincide — dev-mode, reactions and motion — a value only that
reader produces, on a host whose one page child carries a reaction. Without
that, an arm that keeps its label and calls the wrong reader, or none at all,
would pass. **The other half of that property is held by `tsc`, not by this
suite:** a `case` arm wired to the wrong reader fails `bun run typecheck` with
`TS2322`, because each `ReadResult` variant fixes its own result type. Both
gates run in the same command chain, and a reader deleting the shape assertions
above should know that the type checker is what remains behind them.

### The cancellation question, answered

The brief asked it directly: is there an accept-side test that
`dispatchControllerMessage` **honours** an abort, rather than only refusing a
malformed one? **Yes — this one is covered**, by `cancelling a read that never
resolves settles the dispatch as CANCELLED promptly` in `dispatch.test.ts`. It
installs a host `exportAsync` that never settles, cancels mid-export, and
asserts the dispatch settles as `CANCELLED` inside 200 ms.

It took two mutations to establish that, and the second is the point. The
surgical one — `await work` in place of `await Promise.race([work, aborted])`
inside `awaitWithSignal` — turned the test red in 2.5 ms, which is *not* the
test working: with the race gone, the `aborted` promise still rejects and
nothing consumes it, so bun reports an unhandled `LocalCancellationError`
against whichever test is running, and the test never reaches its assertion.
The mutation that reproduces the historical regression — replacing
`awaitWithSignal`'s whole body with `return await work`, which is what deleting
`traversal-gate.ts` did — turns the same test red at 202 ms with
`expect(settled).toEqual(…)` receiving `{ settled: false }`. That is the
dispatch failing to settle, which is exactly the property. The row is `covered`
on the strength of the second, not the first.

What was *not* covered is the cancel arriving as a message. `registry.cancel`
in `dispatchControllerMessage`'s `cancel` case could be deleted, and the arm
could answer with an error envelope instead of `null`, with the suite green
both times: the covering test reaches into the registry directly. The new
`a cancel message aborts the request it names and answers nothing` sends the
cancel the way the socket does.

### `throwIfAbortedAtBatch` can be deleted outright

`throwIfAbortedAtBatch(signal, index, batchSize)` is called from **fifteen
sites** across `plugin/src/read` — `dev-mode.ts` ×1, `components.ts` ×2,
`fonts.ts` ×1, `styles.ts` ×2, `motion.ts` ×1, `reactions.ts` ×1,
`variables.ts` ×2, `serialize.ts` ×5. Emptying its body, so it never throws at
any index, leaves the suite at **413 / 0**. Neither half is pinned, nor is the
complementary property that it does *not* throw between boundaries.

The ten tests whose names begin `checks cancellation` — seven of them named
`checks cancellation between child batches of 100` — all stay green under that
mutation, and all go red when `throwIfAborted()` itself is emptied. What they
pin is a direct `signal.throwIfAborted()` elsewhere in the read path, not the
batch gate their names describe. Reported, not fixed: those tests are in Task
7's area and the sweep does not rewrite existing tests. The acceptance
counterpart added here, `the batch cancellation gate throws on a batch boundary
and only there`, tests the gate directly, including its default batch size of
100.

### The inclusive-ceiling class, again — and one duplicated guard

Task 1's dominant shape recurs. Nine `>` comparisons in this area guard a
ceiling; the refusal above each was pinned, and four of the nine acceptances
*at* the ceiling were not:

| Ceiling | Site | Verdict |
|---|---|---|
| `MAX_RASTER_DECODED_BYTES` | `encodeValidatedRaster` | covered |
| `MAX_RASTER_SIDE` (width) | `encodeValidatedRaster` | covered |
| `MAX_RASTER_SIDE` (height) | `encodeValidatedRaster` | **gap** |
| `MAX_RASTER_PIXELS` | `encodeValidatedRaster` | covered |
| `MAX_RASTER_BASE64_BYTES` | `encodeValidatedRaster` | covered |
| `MAX_RASTER_DECODED_BYTES` | `validateEmbeddedImageData` | **gap** |
| `MAX_RASTER_DECODED_BYTES` | `validateDataUrl` | covered |
| `MAX_SVG_BYTES` | `validateSvgSource` | **gap** |
| `MAX_IDENTIFIER_BYTES` | `rejected` | **gap** |

Nine rows, four gaps. The `MAX_RASTER_SIDE` pair is the sharper one, and it is
Task 1's duplicated-guard class compressed onto a single line:
`if (width > MAX_RASTER_SIDE || height > MAX_RASTER_SIDE)`. Moving the width
comparison turns `rejects sides above 4096 and more than 16 megapixels` red —
it asserts `encodeValidatedRaster(jpegWithSize(MAX_RASTER_SIDE, 1)).ok` — and
moving the height comparison leaves the suite green, because nothing ever
offers a 4096-tall image. One line, two operands, one pinned.

The `MAX_SVG_BYTES` gap has the largest blast radius: a one-character change to
`validateSvgSource`'s size gate would refuse a 4 MiB SVG outright, and the test
covering the refusal above that ceiling (`fails a transfer that never decoded,
and an oversized source`) cannot see it.

### Allow-lists with one member pinned and the rest not

Three media-type allow-lists in this area are each held by a single member:

- `allowedDataMime` names `image/png`, `image/jpeg`, `image/jpg`,
  `image/webp`. Removing `image/png` turns two tests red; removing any of the
  other three leaves the suite green. 1 covered, 3 gaps.
- `isFontDataMime` names seven types. Removing `font/woff2` turns five tests
  red; removing any of the other six leaves the suite green. 1 covered, 6 gaps.
- `validateEmbeddedImageData` maps four spellings onto three magic checks.
  `image/png`, `image/jpeg` and `image/webp` are covered; `image/jpg` — the
  second spelling on a shared line — is a gap.

Ten of those fifteen entries were gaps (3 + 6 + 1), and the shape is Task 2's
`truncation` finding at smaller scale: a list where every entry is a separate
accept path, and one fixture happens to name one of them.

### The socket's outbound direction

`ui/socket.ts` is 45 rows with 17 gaps, and the gaps are not scattered. The
broker→controller direction is well covered — the handshake, the backoff table,
the generation ownership, the duplicate suppression, the status ladder are all
red under mutation — while **the controller→broker direction is almost entirely
unpinned**. Forwarding a progress frame, its `total`, its phase message,
keeping the request open across progress frames, forwarding a response, the
response's result, and the broker request ID both frames travel under: seven
consecutive rows, all green. Only the `error` arm is covered, by `drops stale
controller output when a replacement socket reuses a broker request ID`, which
uses error frames to make its point about ownership.

`ping`/`pong` is the other one worth naming. `status announces each handshake
stage truthfully, never claiming connection before acceptance` *sends* a ping —
it is how that test proves acceptance — but asserts only the status text, so
deleting the pong send entirely leaves the suite green. The broker's liveness
check would go unanswered and nothing would notice.

### Eleven mutations no input can distinguish

Eleven rows are `not applicable`, and none is the "does not compile" case the
vocabulary was written for: `bun run build` strips types, so a type-invalid
mutation still bundles. They are mutations that no input can tell apart. Where
the claim is that a branch is unreachable, it was checked by measurement rather
than by reading — the branch was replaced with a `throw`, and the suite,
including the new tests, stayed at 476 / 0, so nothing in it reaches the
branch.

- `createProgressReporter` has **three defensive lines no caller can reach**,
  each confirmed by a planted `throw`. `due()`'s
  `!Number.isFinite(lastEmitAt)` clause never decides anything, because on the
  only tick where `lastEmitAt` is `NaN` the `lastPhase !== phase` clause is
  already true. The scheduled heartbeat's
  `if (lastPhase === undefined) lastPhase = phase` and `emitLatest`'s
  `lastPhase ?? "reading"` are unreachable for the same reason:
  `startHeartbeat` ticks before it schedules, and that tick always assigns
  `lastPhase`. Worth reporting as dead code, not as coverage gaps.
- `due()`'s `if (intervalMs <= 0) return true` is reached but decides nothing:
  the comparison after it, `now() - lastEmitAt >= intervalMs`, is true whenever
  `intervalMs <= 0` and the clock does not run backwards. Deleting the clause
  leaves the suite at 476 / 0 with the new `a non-positive interval emits every
  tick of the same phase` test in place.
- `LocalCancellationSignal.abort()`'s `if (this.#aborted) return` is
  unobservable: the first abort clears the listener set, and `addEventListener`
  refuses to add to an aborted signal, so a second pass finds nothing to call.
- `code.ts`'s `inbound` returning the original string when `JSON.parse` throws,
  and the `return` after `completeScreenshotValidation`, are unobservable from
  outside: whatever those paths hand on, both `parseControllerMetadataRequest`
  and `parseControllerBoundMessage` refuse it and the handler falls out
  silently either way.
- `asBytes`'s `value instanceof Uint8Array` arm is subsumed by the
  `ArrayBuffer.isView(value)` arm two lines below, which handles a `Uint8Array`
  identically. Deleting the first arm alone leaves the suite green; deleting
  both turns three of the new relay tests red, which is what establishes that
  the view arm is doing the work.
- `readUint32`'s big-endian assembly cannot be distinguished within the range
  its caller admits. Shifting the first byte by 16 instead of 24 changes the
  value only when that byte is non-zero, and any such width or height is at
  least 65,536 — refused by `MAX_RASTER_SIDE` (4,096) with or without the
  mutation.
- `matches`'s `bytes.length < offset + expected.length` bound is only ever
  called with `offset` 0 and the 8-byte PNG magic, and `isPng` requires 24
  bytes before the value is used, so `<` and `<=` agree on every input that
  reaches it.
- `parseDataUrl`'s comma search starting at index 5 rather than 6 matters only
  for `data:,payload`, whose media type defaults to `text/plain` and is refused
  either way.

### Five rows whose first measurement did not stand

Five rows — `SV24`, `CS24`, the two the `+`/`/` row split into, and `P19` — are
not the straightforward "mutate, run, record" case, and each is worth naming
because each is a way this method can lie.

- **A mutation that changed nothing.** `SV24`'s first mutation loosened the
  length test in `validateCssText`'s `@import` check (`name.length === 6` →
  `name.length >= 0`) and left the six character comparisons standing, so it
  refused exactly what it refused before. It was recorded green and would have
  read as a gap. Re-measured with a mutation that refuses *every* at-keyword,
  it turns four tests red, three of them pre-existing (`an embedded font data
  URL is accepted`, `… with mixed-case scheme and mime`, `benign values stay
  clean`), so the row is **covered**. Its suite result in the table is from the
  476-test suite, because that is when it was measured.
- **A mutation that did not terminate.** `CS24`'s deletion of the fractional
  part of `consumeNumber` makes the tokenizer return an empty number token
  without advancing `index`, so `tokenizeCss(".5")` loops forever — which only
  showed up once the new number test fed it `.5`. Re-measured with a mutation
  that consumes the decimal point but not its digits: one red, the new test.
  The same shape cost one earlier run too: `CS28`'s first mutation looped on a
  bare backslash and had to be replaced with one that consumes the escape and
  drops the character.
- **A row that was one row for two constants.** `` `+` decodes to 62 and `/`
  decodes to 63 `` was recorded as a single row whose mutation swapped the two,
  which changes both at once and so cannot say which member is unpinned. It is
  now two rows with two mutations — return 61 for `+`, return 61 for `/` — and
  they answer differently: the first turns one new test red, the second turns
  two. Both were measured on the 476-test suite, and both read **gap** because
  every red is a test this task added; their `Suite result` cells therefore
  carry the post-test numbers, as `SV24`'s does.
- **A red that did not reproduce.** `P19` — deleting the heartbeat callback's
  phase default — turned `get_motion > keeps applied styles distinct from the
  catalog and copies seconds unchanged` red on its first run, in a different
  file from anything the mutation touches. Re-running it against
  `motion.test.ts` alone passed, and two further full-suite runs passed. The
  red was a real timer firing across a file boundary, not the property. It is
  the only red in this task that did not reproduce, and the rule that caught it
  is the wrong-reason rule: a `covered` verdict has to say *why* the test
  fired.

### Three maintenance edges this section leaves behind

Recorded because a later reader will meet them and nothing else names them.

- **`plugin/src/main/code.test.ts` exercises `detectCapabilities()` and asserts
  a capability-*present* path.** `plugin/tests/figma-harness.ts`'s header names
  exactly that as the condition under which its "inert" argument stops holding
  — its point being that `detectCapabilities()` is reached from `navigation.ts`
  and `main/code.ts`, neither of which the nine migrated files exercise. The
  argument still holds literally: `code.test.ts` builds its own host rather
  than calling `installFigma`, so the harness's over-complete default is not
  what that assertion runs against. But whoever owns that header should know a
  second caller now exists.
- **A new `plugin/src/main/*.test.ts` lands in two tsconfig projects at once.**
  `tsconfig.controller-tests.json` includes `src/main/**/*.ts` by glob and
  `tsconfig.tests.json` includes `src/**/*.test.ts` by glob, and the two load
  different `types`, which is why `dispatch.test.ts` and `progress.test.ts`
  carry explicit `exclude` entries. A new main test file will most likely fail
  typecheck until someone excludes it — loudly, which is the right direction,
  and the opposite of `tsconfig.ui-tests.json`, whose file-by-file `include`
  leaves a new UI test file silently unchecked.
- **The Mutation column is not self-identifying.** Seventeen mutation texts
  repeat across rows (`` `>` becomes `>=` `` five times, `drop that range`
  three times, and so on). The Accept path disambiguates every one, and no
  Accept path is duplicated within a section, so no row is answered twice — but
  a reader scanning the Mutation column alone cannot reconstruct which site was
  cut.

### What is left open

Eleven gaps stay open, marked `— (open)` in the table. They are open for three
kinds of reason, and the reason is the point rather than the count:

- **Doubly covered in the harness, so the mutation is invisible to it.** The
  socket's teardown calls `cancelGenerationRequests` and then `close()`, and
  the close handler cancels again, so deleting the first call changes nothing a
  fake socket can show — though on a real socket, whose `close` event is
  asynchronous, the cancel would be late. Deleting `stopListening()` is
  invisible for the same reason: by the time a later controller message
  arrives, the owner map the close already emptied has nothing to forward.
  (`S40`, `S42`.)
- **Guards whose accept side no fixture reaches.** `isActiveOpen`'s
  `active === generation` clause and the owner check on the controller path
  (`S44`, `S45`); the reconnect after a `new WebSocket` that throws (`S39`);
  `awaitWithSignal` unhooking its listener, whose only observable is an
  unhandled rejection that `Promise.race` has already claimed (`C18`);
  `ignoreSettlement` being called from the dispatcher rather than in isolation
  (`D06`); the dispatcher stopping its heartbeat once a request settles
  (`D12`). And two in `svg.ts` — preferring a DOM's own `localName` over
  `tagName`, and reading attribute names through `getAttributeNames()` rather
  than the `attributes` map (`SV28`, `SV29`) — where the two paths agree for
  every DOM a real parser produces, so separating them would mean asserting
  against a document no parser emits.
- **A default only a slow test could distinguish.** `createProgressReporter`
  falling back to `Date.now`: the mutation replaces it with `() => 0`, and the
  only observable difference is a comparison against a 3,000 ms interval, so
  separating them needs a real three-second wait. Attempted and abandoned
  rather than declared impossible — the shape of the test is obvious and the
  cost is a three-second suite. (`P24`.)

### The table

**`plugin/src/main/dispatch.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a `cancel` message aborts the request it names | drop the `registry.cancel(...)` call from the `cancel` arm of `dispatchControllerMessage` | 413 / 0 | **gap** | `a cancel message aborts the request it names and answers nothing` |
| a `cancel` message is answered with `null`, so nothing is posted back | return an `error` envelope from the `cancel` arm instead of `null` | 413 / 0 | **gap** | `a cancel message aborts the request it names and answers nothing`, `a cancel is dispatched and answered with no message at all` |
| a request reserves its correlation ID under `controllerRequestId` | `registry.begin(message.controllerRequestId)` keyed on `message.requestId` instead | 411 / 2 | covered | — |
| the request's reporter is bound to its signal, so read code can find it | delete `bindProgress(controller.signal, progress)` | 413 / 0 | **gap** | `read code finds the reporter bound to its signal, and its totals reach the controller` |
| a request emits its first `reading` frame at once | delete `progress.startHeartbeat("reading")` | 412 / 1 | covered | — |
| the abandoned read's later settlement is swallowed | delete `ignoreSettlement(work)` | 413 / 0 | **gap** | — *(open)* |
| a response echoes the controller correlation ID | `controllerRequestId: message.requestId` in the response envelope | 413 / 0 | **gap** | `a settled request frees its correlation ID for the next one`, `dispatches a well-formed request and posts the response back` |
| a response echoes the broker request ID | `requestId: message.controllerRequestId` in the response envelope | 411 / 2 | covered | — |
| a response carries the read result | `result: undefined` in the response envelope | 412 / 1 | covered | — |
| an error envelope carries the mapped boundary failure | hard-code `{ code: "INTERNAL_ERROR", retryable: false }` in the error envelope | 412 / 1 | covered | — |
| a settled request stops its heartbeat | delete `progress?.stopHeartbeat()` from the `finally` | 413 / 0 | **gap** | — *(open)* |
| a settled request frees its correlation ID | delete `registry.finish(...)` from the `finally` | 413 / 0 | **gap** | `a settled request frees its correlation ID for the next one` |
| a request whose signal is live proceeds to the reader | invert `dispatchRead`'s pre-flight to `if (!signal.aborted) throw` | 411 / 2 | covered | — |
| `requestBoundaryFailure` keeps a `PluginReadError`'s own code | return `INTERNAL_ERROR` instead of `error.code` | 413 / 0 | **gap** | `a read failure keeps its own code and retryable flag at the boundary` |
| `requestBoundaryFailure` keeps a `PluginReadError`'s retryable flag | return `!error.retryable` | 413 / 0 | **gap** | `a read failure keeps its own code and retryable flag at the boundary` |
| `requestBoundaryFailure` maps a local cancellation to `CANCELLED` | return `INTERNAL_ERROR` from the `LocalCancellationError` arm | 411 / 2 | covered | — |
| `requestBoundaryFailure` maps an unrecognised error to `INTERNAL_ERROR` | return `CANCELLED` from the fall-through arm | 411 / 2 | covered | — |
| a progress frame's `total` reaches the controller when it has one | delete `if (frame.total !== undefined) message.total = frame.total` | 413 / 0 | **gap** | `read code finds the reporter bound to its signal, and its totals reach the controller` |
| a progress frame is posted through `figma.ui.postMessage` | replace `ui.postMessage(message)` with `void message` | 412 / 1 | covered | — |
| a `get_metadata` request reaches `readMetadata` | delete the `case "get_metadata"` arm of `dispatchRead` | 411 / 2 | covered | — |
| a `get_selection` request reaches `readSelection` | delete the `case "get_selection"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_nodes` request reaches `readNodes` | delete the `case "get_nodes"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `search_nodes` request reaches `searchNodes` | delete the `case "search_nodes"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_design_context` request reaches `readDesignContext` | delete the `case "get_design_context"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_styles` request reaches `getStyles` | delete the `case "get_styles"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_variables` request reaches `getVariables` | delete the `case "get_variables"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_components` request reaches `getComponents` | delete the `case "get_components"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_fonts` request reaches `getFonts` | delete the `case "get_fonts"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_dev_mode_data` request reaches `getDevModeData` | delete the `case "get_dev_mode_data"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_reactions` request reaches `getReactions` | delete the `case "get_reactions"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_motion` request reaches `getMotion` | delete the `case "get_motion"` arm of `dispatchRead` | 413 / 0 | **gap** | `every read operation reaches the reader named in the request` |
| a `get_screenshot` request reaches `getScreenshot` | delete the `case "get_screenshot"` arm of `dispatchRead` | 412 / 1 | covered | — |

**`plugin/src/main/cancellation.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a cancellation fired mid-read settles the dispatch at once | `await work` in place of `await Promise.race([work, aborted])` in `awaitWithSignal` | 412 / 1 | covered | — |
| an abort listener registered on a live signal is kept | make `addEventListener` never add to the listener set | 410 / 3 | covered | — |
| a removed abort listener is not called | make `removeEventListener` never delete from the listener set | 413 / 0 | **gap** | `a removed abort listener is not called` |
| `abort()` marks the signal aborted | delete `this.#aborted = true` | 397 / 16 | covered | — |
| `abort()` calls every registered listener | replace `listener()` with `void listener` | 410 / 3 | covered | — |
| a second `abort()` does not notify again | delete the `if (this.#aborted) return` guard | 413 / 0 | not applicable | — |
| a listener that throws does not stop the ones after it | call `listener()` outside the `try` | 412 / 1 | covered | — |
| `throwIfAborted()` throws once the signal is aborted | empty the `throwIfAborted` body | 399 / 14 | covered | — |
| `throwIfAborted()` is silent while the signal is live | invert to `if (!this.#aborted) throw` | 409 / 4 | covered | — |
| `begin()` registers the controller under its request ID | delete `this.#active.set(requestId, controller)` | 411 / 2 | covered | — |
| `cancel()` aborts the controller it found | delete `controller.abort()` | 411 / 2 | covered | — |
| `cancel()` reports that it cancelled something | return `false` from the successful arm | 412 / 1 | covered | — |
| `finish()` removes the request from the registry | make `finish` a no-op | 412 / 1 | covered | — |
| `cancelAll()` cancels every active request | count without calling `this.cancel(requestId)` | 413 / 0 | **gap** | `cancelAll aborts every active request and reports how many it stopped` |
| `cancelAll()` reports how many it stopped | return `0` | 413 / 0 | **gap** | `cancelAll aborts every active request and reports how many it stopped` |
| `throwIfAbortedAtBatch` throws on a batch boundary | empty the body of `throwIfAbortedAtBatch` | 413 / 0 | **gap** | `the batch cancellation gate throws on a batch boundary and only there` |
| `throwIfAbortedAtBatch` does not throw between boundaries | drop the `index % batchSize === 0` condition, so it always throws | 413 / 0 | **gap** | `the batch cancellation gate throws on a batch boundary and only there` |
| `awaitWithSignal` with no signal returns the work unchanged | throw `LocalCancellationError` on the no-signal arm | 413 / 0 | **gap** | `awaitWithSignal passes work through, refuses an aborted signal, and unhooks itself` |
| `awaitWithSignal` refuses an already-aborted signal without awaiting | delete the `if (signal.aborted) throw` pre-check | 413 / 0 | **gap** | `awaitWithSignal passes work through, refuses an aborted signal, and unhooks itself` |
| `awaitWithSignal` unhooks its listener once the work settles | replace the `finally` `removeEventListener` with `void listener` | 413 / 0 | **gap** | — *(open)* |
| `ignoreSettlement` swallows the abandoned promise's rejection | replace the body with `void work` | 413 / 0 | **gap** | `an abandoned promise's rejection is swallowed rather than left unhandled` |
| `size` reports how many requests are active | return `0` | 413 / 0 | **gap** | `cancelAll aborts every active request and reports how many it stopped` |

**`plugin/src/main/progress.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| `bindProgress` stores the reporter against the signal | make `bindProgress` a no-op | 411 / 2 | covered | — |
| `progressFor` returns the reporter bound to a signal | return `undefined` always | 411 / 2 | covered | — |
| a fractional count is floored | return `value` instead of `Math.floor(value)` | 413 / 0 | **gap** | `a fractional count is floored rather than sent as a fraction` |
| a count at or above `U32_MAX` is clamped to it | delete the `value >= U32_MAX` clamp | 412 / 1 | covered | — |
| a non-finite or non-positive count becomes `0` | delete the `!Number.isFinite(value) \|\| value <= 0` guard | 412 / 1 | covered | — |
| a change of phase emits immediately | drop `lastPhase !== phase` from `due()` | 412 / 1 | covered | — |
| the very first tick emits even inside the interval | drop `!Number.isFinite(lastEmitAt)` from `due()` | 413 / 0 | not applicable | — |
| a tick after the interval has elapsed emits | return `false` from the elapsed-interval comparison | 412 / 1 | covered | — |
| with a non-positive interval every tick emits | delete the `intervalMs <= 0` short-circuit | 413 / 0 | not applicable | — |
| a repeat tick inside the interval updates counts without emitting | delete `if (!due(phase)) return` | 411 / 2 | covered | — |
| a frame carries `total` when the tick supplied one | delete `if (total !== undefined) frame.total = toU32(total)` | 410 / 3 | covered | — |
| a tick without a total drops the previous one | keep the previous total when `nextTotal` is undefined | 413 / 0 | **gap** | `a tick without a total drops the total the previous tick carried` |
| an emit records when it happened, so the interval can run | delete `lastEmitAt = now()` | 411 / 2 | covered | — |
| an `emit` that throws does not fail the read | call `options.emit(frame)` outside the `try` | 413 / 0 | **gap** | `an emit that throws does not fail the read it is reporting on` |
| `startHeartbeat` emits a frame at once | delete the immediate `tick(phase, completed, total)` | 412 / 1 | covered | — |
| `startHeartbeat` stops the timer a previous one left | delete `if (stopTimer !== undefined) stopTimer()` | 413 / 0 | **gap** | `restarting the heartbeat stops the timer the previous one left running` |
| `startHeartbeat` schedules the repeat | never call `schedule(...)` | 412 / 1 | covered | — |
| each heartbeat tick emits the latest frame | delete `emitLatest()` from the scheduled callback | 412 / 1 | covered | — |
| a heartbeat with no phase yet adopts the one it was started with | delete `if (lastPhase === undefined) lastPhase = phase` | 413 / 0 | not applicable | — |
| `stopHeartbeat` cancels the timer | delete `stopTimer?.()` | 412 / 1 | covered | — |
| a reporter with no interval uses `PROGRESS_INTERVAL_MS` | default `intervalMs` to `1` | 413 / 0 | **gap** | `the default interval is the inactivity-safe one, not something shorter` |
| the default heartbeat repeats rather than firing once | `setTimeout`/`clearTimeout` in place of `setInterval`/`clearInterval` | 413 / 0 | **gap** | `the default heartbeat repeats instead of firing once` |
| a frame with no phase yet is labelled `reading` | default the frame message to `encoding` | 413 / 0 | not applicable | — |
| a reporter with no clock uses `Date.now` | default `now` to `() => 0` | 413 / 0 | **gap** | — *(open)* |

**`plugin/src/main/code.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a message delivered as a JSON string is parsed | return the string unparsed from `inbound` | 413 / 0 | **gap** | `accepts a readiness request in each transport shape the host uses` |
| a string that is not JSON is passed on unchanged | return `null` from `inbound`'s catch | 413 / 0 | not applicable | — |
| a `{ pluginMessage }` envelope is unwrapped | return the envelope itself | 413 / 0 | **gap** | `accepts a readiness request in each transport shape the host uses` |
| a plain object is passed through untouched | return `null` from `inbound`'s fall-through | 413 / 0 | **gap** | `answers a readiness request with the file's own identity`, `accepts a readiness request in each transport shape the host uses`, `dispatches a well-formed request and posts the response back` |
| a settled screenshot validation ends the handler | drop the `return` after `completeScreenshotValidation` | 413 / 0 | not applicable | — |
| a readiness request is answered with `controllerReady` | drop the `postReady(request.metadataRequestId)` call | 413 / 0 | **gap** | `answers a readiness request with the file's own identity`, `accepts a readiness request in each transport shape the host uses` |
| a controller-bound message is dispatched | never call `dispatchControllerMessage` | 413 / 0 | **gap** | `dispatches a well-formed request and posts the response back` |
| the dispatcher's answer is posted to the iframe | replace `figma.ui.postMessage(output)` with `void output` | 413 / 0 | **gap** | `dispatches a well-formed request and posts the response back` |
| a `null` answer posts nothing | post unconditionally, dropping the `output !== null` guard | 413 / 0 | **gap** | `a cancel is dispatched and answered with no message at all` |
| `controllerReady` carries the file name | send an empty `fileName` | 413 / 0 | **gap** | `answers a readiness request with the file's own identity` |
| `controllerReady` carries the current page's id and name | send an empty `currentPage` | 413 / 0 | **gap** | `answers a readiness request with the file's own identity` |
| `controllerReady` carries the editor type | hard-code `editorType: "figma"` | 413 / 0 | **gap** | `answers a readiness request with the file's own identity` |
| `controllerReady` carries `PLUGIN_VERSION` | hard-code `pluginVersion: "9.9.9"` | 413 / 0 | **gap** | `answers a readiness request with the file's own identity` |
| `controllerReady` carries the detected capabilities | send an empty capability set | 413 / 0 | **gap** | `answers a readiness request with the file's own identity` |
| `controllerReady` echoes the readiness request's ID | send an empty `metadataRequestId` | 413 / 0 | **gap** | `answers a readiness request with the file's own identity`, `accepts a readiness request in each transport shape the host uses` |
| the controller shows the bundled UI at the declared panel size | replace `figma.showUI(__html__, …)` with `void __html__` | 413 / 0 | **gap** | `shows the bundled UI at the declared panel size` |

**`plugin/src/ui/relay.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a controller-bound message is posted inside a `pluginMessage` envelope | post the message itself, without the envelope | 407 / 6 | covered | — |
| the relay subscribes to `message` events | subscribe to `messageX` instead | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over`, `returns an SVG screenshot with its own source and safety verdict`, `subscribes to message events and unsubscribes from the same channel` |
| a `validateScreenshot` request is recognised | match `validateScreenshotX` instead | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over`, `returns an SVG screenshot with its own source and safety verdict`, `subscribes to message events and unsubscribes from the same channel` |
| the reply is typed `screenshotValidated` | type the reply `screenshotValidatedX` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `returns an SVG screenshot with its own source and safety verdict` |
| the reply echoes the validation ID | send an empty `validationId` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `returns an SVG screenshot with its own source and safety verdict`, `subscribes to message events and unsubscribes from the same channel` |
| the reply carries the finalized asset | send `asset: null` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over`, `returns an SVG screenshot with its own source and safety verdict` |
| an item declared `svg` takes the SVG path | match `svgX` instead | 413 / 0 | **gap** | `returns an SVG screenshot with its own source and safety verdict` |
| an SVG asset carries its source | send an empty `source` | 413 / 0 | **gap** | `returns an SVG screenshot with its own source and safety verdict` |
| an SVG asset carries the safety verdict | hard-code `safe: true` | 413 / 0 | **gap** | `returns an SVG screenshot with its own source and safety verdict` |
| an unsafe SVG asset carries the rule that fired | delete the `value.rejection = result.rejection` assignment | 413 / 0 | **gap** | `returns an SVG screenshot with its own source and safety verdict` |
| an item declared `png` is encoded | narrow the format guard to `!== "jpeg"` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `accepts the export bytes in every shape the host can hand over` |
| an item declared `jpeg` is encoded | narrow the format guard to `!== "png"` | 413 / 0 | **gap** | `validates a JPEG screenshot, which the PNG path cannot stand in for` |
| export bytes arriving as a `Uint8Array` are taken | delete the `Uint8Array` arm of `asBytes` | 413 / 0 | not applicable | — |
| export bytes arriving as an `ArrayBuffer` are taken | delete the `ArrayBuffer` arm of `asBytes` | 413 / 0 | **gap** | `accepts the export bytes in every shape the host can hand over` |
| export bytes arriving as a typed-array view are taken | delete the `ArrayBuffer.isView` arm of `asBytes` | 413 / 0 | **gap** | `accepts the export bytes in every shape the host can hand over` |
| export bytes arriving as a plain number array are taken | return `null` before the array arm of `asBytes` | 413 / 0 | **gap** | `accepts the export bytes in every shape the host can hand over` |
| a raster asset carries the encoded format | hard-code `format: "png"` | 413 / 0 | **gap** | `validates a JPEG screenshot, which the PNG path cannot stand in for` |
| a raster asset carries the base64 payload | send an empty `dataBase64` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over` |
| a raster asset carries its width | send `width: 0` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over` |
| a raster asset carries its height | send `height: 0` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over` |
| an asset carries the node it was captured from | always send an empty `nodeId` | 413 / 0 | **gap** | `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `returns an SVG screenshot with its own source and safety verdict` |
| a well-formed controller message reaches the receiver | parse without calling `receive(...)` | 409 / 4 | covered | — |
| the returned unsubscribe removes the listener | return a no-op unsubscribe | 413 / 0 | **gap** | `subscribes to message events and unsubscribes from the same channel` |

**`plugin/src/ui/socket.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| an opened socket asks the controller for readiness | delete the `requestControllerReady` send | 407 / 6 | covered | — |
| the readiness request carries this generation's ID | send a fresh `randomUuid()` instead | 409 / 4 | covered | — |
| the open stage is announced as `Socket open, waiting for Figma…` | shorten the status text | 412 / 1 | covered | — |
| a matching `controllerReady` sends the hello | delete the `sendJson(... buildHello ...)` call | 411 / 2 | covered | — |
| the hello is marked sent, so it is sent once | delete `generation.helloSent = true` | 409 / 4 | covered | — |
| the hello stage is announced as `Hello sent, waiting for broker…` | shorten the status text | 412 / 1 | covered | — |
| broker frames are read once the hello has gone out | invert the `!generation.helloSent` guard | 409 / 4 | covered | — |
| a string frame is read | invert the `typeof event.data !== "string"` guard | 409 / 4 | covered | — |
| acceptance is latched, so it is announced once per socket | delete `generation.acceptedSinceOpen = true` | 413 / 0 | **gap** | `the connected latch fires once, so a later frame does not re-announce it` |
| an accepted frame resets the reconnect backoff | delete `reconnectAttempt = 0` | 412 / 1 | covered | — |
| acceptance is announced as `Connected to local broker` | shorten the status text | 412 / 1 | covered | — |
| a broker request with an unseen ID is not treated as a duplicate | return unconditionally at the duplicate check | 411 / 2 | covered | — |
| each broker request gets a fresh controller correlation ID | hard-code one UUID | 412 / 1 | covered | — |
| the request's owner is recorded against that ID | delete the `requestOwners.set(...)` call | 411 / 2 | covered | — |
| the decoded request is forwarded to the controller | delete the `sendToController(controllerMessage)` call | 411 / 2 | covered | — |
| a broker cancel drops the owner entry | delete the `requestOwners.delete(...)` call | 412 / 1 | covered | — |
| a broker cancel is forwarded to the controller | delete the `sendToController(... type: "cancel" ...)` call | 412 / 1 | covered | — |
| a broker ping is answered with a pong | delete the pong send | 413 / 0 | **gap** | `a broker ping is answered with a pong carrying the same nonce` |
| the pong echoes the ping's nonce | send `nonce: "0"` | 413 / 0 | **gap** | `a broker ping is answered with a pong carrying the same nonce` |
| a forwarded progress frame uses the broker's request ID | use the controller correlation ID instead | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID` |
| a forwarded progress frame keeps its `total` | delete the `total` copy | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID` |
| a forwarded progress frame keeps its phase message | delete the `message` copy | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID` |
| a progress frame leaves the request open for the next one | delete the owner on every controller message | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID` |
| a controller response is forwarded to the broker | delete the response send | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID`, `a response ends the request, so nothing more is forwarded for it` |
| a forwarded response carries the read result | send `result: undefined` | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID` |
| a forwarded response uses the broker's request ID | use the controller correlation ID instead | 413 / 0 | **gap** | `controller progress and the response reach the broker under the broker's request ID` |
| a controller error is forwarded to the broker | delete the error send | 412 / 1 | covered | — |
| a forwarded error carries the failure it was given | hard-code an `INTERNAL_ERROR` failure | 412 / 1 | covered | — |
| a closed socket cancels the requests it was carrying | delete `cancelGenerationRequests(generation)` from the close handler | 411 / 2 | covered | — |
| a closed socket schedules a reconnect | delete `scheduleReconnect()` from the close handler | 408 / 5 | covered | — |
| successive reconnects walk the delay table | pin the table index at `0` | 410 / 3 | covered | — |
| the reconnect delay clamps at the table's last entry | clamp one entry earlier | 412 / 1 | covered | — |
| each scheduled reconnect counts as an attempt | delete `reconnectAttempt += 1` | 410 / 3 | covered | — |
| the reconnect timer is cleared before reconnecting | delete `reconnectTimer = undefined` from the callback | 409 / 4 | covered | — |
| a generation cancel names the broker's request ID | send the controller correlation ID instead | 411 / 2 | covered | — |
| an outbound frame is serialised as its own JSON | send `"{}"` instead | 411 / 2 | covered | — |
| a status update reaches the status node | make `setStatus` a no-op | 412 / 1 | covered | — |
| the socket opens against `BROKER_URL` | open against a different URL | 413 / 0 | **gap** | `a broker ping is answered with a pong carrying the same nonce` |
| a socket that could not be constructed schedules a reconnect | delete `scheduleReconnect()` from the constructor catch | 413 / 0 | **gap** | — *(open)* |
| stopping the transport unsubscribes from controller messages | delete `stopListening()` | 413 / 0 | **gap** | — *(open)* |
| stopping the transport closes the socket | delete `generation.socket.close()` | 413 / 0 | **gap** | `stopping the transport closes the socket and cancels what it was carrying` |
| stopping the transport cancels the requests it was carrying | delete `cancelGenerationRequests(generation)` from the teardown | 413 / 0 | **gap** | — *(open)* |
| an open socket passes the liveness check | invert the `readyState === WebSocket.OPEN` comparison | 407 / 6 | covered | — |
| the current generation passes the liveness check | invert `active === generation` (first run: drop the clause) | 413 / 0 | **gap** | — *(open)* |
| a controller message with a known owner is forwarded | return early when the owner *is* known (first run: drop the openness clause) | 413 / 0 | **gap** | — *(open)* |

**`plugin/src/ui/hello.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| the hello declares protocol version `4` | declare `5` | 413 / 0 | **gap** | `carries the whole readiness announcement, at the protocol version the broker expects` |
| the hello's connection ID comes from the UUID factory | hard-code one UUID | 411 / 2 | covered | — |
| the hello carries a display name | send an empty `displayName` | 412 / 1 | covered | — |
| the hello carries the file name | send an empty `fileName` | 413 / 0 | **gap** | `carries the whole readiness announcement, at the protocol version the broker expects` |
| the hello carries the current page's id and name | send an empty `currentPage` | 412 / 1 | covered | — |
| the hello carries the editor type | hard-code `editorType: "figma"` | 413 / 0 | **gap** | `carries the whole readiness announcement, at the protocol version the broker expects` |
| the hello carries the plugin version | hard-code `"9.9.9"` | 412 / 1 | covered | — |
| the hello carries a copy of the capability set | send an empty capability set | 413 / 0 | **gap** | `carries the whole readiness announcement, at the protocol version the broker expects` |

**`plugin/src/ui/uuid.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| the host's `crypto.randomUUID` is used when it can be called | skip the `randomUUID` branch | 413 / 0 | **gap** | `uses the host's randomUUID whenever it can be called` |
| `crypto.getRandomValues` is used when `randomUUID` is missing | delete the `getRandomValues` branch | 413 / 0 | **gap** | `falls back to getRandomValues, stamping the version and variant itself`, `a randomUUID that throws when called is not fatal` |
| `Math.random` fills the bytes when there is no web crypto | fill every byte with `255` instead | 413 / 0 | **gap** | `with no web crypto at all, Math.random still produces a valid v4 UUID` |
| a hand-built UUID carries version 4 | stamp `0x90` instead of `0x40` | 412 / 1 | covered | — |
| a hand-built UUID carries the RFC variant | stamp `0xc0` instead of `0x80` | 412 / 1 | covered | — |
| a hand-built UUID is grouped 8-4-4-4-12 | move the first hyphen one character left | 412 / 1 | covered | — |
| a `randomUUID` that throws when called falls through to the next source | remove the `try`/`catch` around it | 413 / 0 | **gap** | `a randomUUID that throws when called is not fatal` |

**`plugin/src/ui/raster.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a PNG's magic is recognised | change the last byte of `PNG_MAGIC` | 407 / 6 | covered | — |
| a PNG's IHDR chunk name is recognised | change the `I` of the IHDR check | 407 / 6 | covered | — |
| a 24-byte PNG header is long enough to measure | require 25 bytes | 413 / 0 | **gap** | `a PNG carrying nothing but its header is still measured` |
| a JPEG's SOI marker is recognised | expect `0xd9` instead of `0xd8` | 410 / 3 | covered | — |
| a four-byte JPEG is long enough to recognise | require 5 bytes | 413 / 0 | **gap** | `a four-byte JPEG is enough to recognise, though not to measure` |
| a WebP's `WEBP` tag is recognised | change the `W` of the tag check | 412 / 1 | covered | — |
| a raster exactly at `MAX_RASTER_DECODED_BYTES` is accepted | `>` becomes `>=` | 412 / 1 | covered | — |
| a width exactly at `MAX_RASTER_SIDE` is accepted | `width >` becomes `width >=` | 412 / 1 | covered | — |
| a height exactly at `MAX_RASTER_SIDE` is accepted | `height >` becomes `height >=` | 413 / 0 | **gap** | `a side exactly at the ceiling is accepted, in both dimensions` |
| an area exactly at `MAX_RASTER_PIXELS` is accepted | `>` becomes `>=` | 412 / 1 | covered | — |
| a payload exactly at `MAX_RASTER_BASE64_BYTES` is accepted | `>` becomes `>=` | 412 / 1 | covered | — |
| an embedded image exactly at `MAX_RASTER_DECODED_BYTES` is accepted | `>` becomes `>=` | 413 / 0 | **gap** | `an embedded image exactly at the decoded-byte ceiling is still accepted` |
| a PNG's width is read from IHDR offset 16 | read from offset 17 | 411 / 2 | covered | — |
| a PNG's height is read from IHDR offset 20 | read from offset 21 | 411 / 2 | covered | — |
| a PNG dimension is assembled big-endian | shift the first byte by 16 instead of 24 | 413 / 0 | not applicable | — |
| TEM and restart markers are stepped over | delete the standalone-marker `continue` | 413 / 0 | **gap** | `JPEG dimensions are read past skippable segments, from every SOF marker` |
| start-of-frame markers 0xC0–0xC3 are measured | drop that range | 411 / 2 | covered | — |
| start-of-frame markers 0xC5–0xC7 are measured | drop that range | 413 / 0 | **gap** | `JPEG dimensions are read past skippable segments, from every SOF marker` |
| start-of-frame markers 0xC9–0xCB are measured | drop that range | 413 / 0 | **gap** | `JPEG dimensions are read past skippable segments, from every SOF marker` |
| start-of-frame markers 0xCD–0xCF are measured | drop that range | 413 / 0 | **gap** | `JPEG dimensions are read past skippable segments, from every SOF marker` |
| a JPEG's height precedes its width in the SOF segment | swap the two reads | 412 / 1 | covered | — |
| a length-carrying segment is stepped over by exactly its length | step one byte too far | 413 / 0 | **gap** | `JPEG dimensions are read past skippable segments, from every SOF marker` |
| the declared format picks the matching dimension reader | swap the two readers | 410 / 3 | covered | — |
| the declared format picks the matching magic check | swap the two checks | 410 / 3 | covered | — |
| base64 index 63 encodes as `/` | encode it as `_` | 413 / 0 | **gap** | `base64 encoding uses the whole alphabet and both padding lengths`, `validates a JPEG screenshot, which the PNG path cannot stand in for` |
| a one-byte tail pads with `=` | pad with `*` | 413 / 0 | **gap** | `base64 encoding uses the whole alphabet and both padding lengths`, `base64 decoding reverses that, skipping whitespace and honouring padding`, `validates a PNG screenshot and answers with the encoded asset`, `validates a JPEG screenshot, which the PNG path cannot stand in for`, `accepts the export bytes in every shape the host can hand over` |
| a two-byte tail pads with `=` | pad with `*` | 413 / 0 | **gap** | `base64 encoding uses the whole alphabet and both padding lengths`, `base64 decoding reverses that, skipping whitespace and honouring padding`, `validates a PNG screenshot and answers with the encoded asset`, `accepts the export bytes in every shape the host can hand over` |
| whitespace in a base64 payload is skipped | count whitespace as payload | 413 / 0 | **gap** | `base64 decoding reverses that, skipping whitespace and honouring padding` |
| two `=` characters mean two dropped bytes | count at most one | 413 / 0 | **gap** | `base64 decoding reverses that, skipping whitespace and honouring padding` |
| `A`–`Z` decode to 0–25 | shift the range by one | 408 / 5 | covered | — |
| `a`–`z` decode to 26–51 | shift the range by one | 411 / 2 | covered | — |
| `0`–`9` decode to 52–61 | shift the range by one | 411 / 2 | covered | — |
| `+` decodes to 62 | return 61 for `+` | 475 / 1 | **gap** | `base64 decoding reverses that, skipping whitespace and honouring padding` |
| `/` decodes to 63 | return 61 for `/` | 474 / 2 | **gap** | `base64 decoding reverses that, skipping whitespace and honouring padding`, `every media type the data: allow-list names is accepted` |
| `image/png` is checked against PNG magic | always refuse it | 410 / 3 | covered | — |
| `image/jpeg` is checked against JPEG magic | drop that spelling | 412 / 1 | covered | — |
| `image/jpg` is checked against JPEG magic | drop that spelling | 413 / 0 | **gap** | `image/jpg is accepted as its own spelling of the JPEG media type` |
| `image/webp` is checked against WebP magic | always refuse it | 412 / 1 | covered | — |
| the encoded result carries the measured dimensions | report `0` for both | 412 / 1 | covered | — |
| the encoded result carries both byte counts | report `0` for both | 411 / 2 | covered | — |
| the encoded result carries the base64 payload | report an empty payload | 412 / 1 | covered | — |
| a buffer exactly as long as the magic is long enough | `<` becomes `<=` | 413 / 0 | not applicable | — |

**`plugin/src/ui/css-syntax.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a comment becomes a `comment` token | emit a `delim` instead | 412 / 1 | covered | — |
| a whitespace run becomes a `whitespace` token | emit a `delim` instead | 413 / 0 | **gap** | `produces every token kind it declares`, `a bare # or @ is a delimiter carrying its own character`, `identifiers start with a hyphen, a double hyphen or a non-ASCII letter`, `escapes resolve to the character they name` |
| a quoted run becomes a `string` token | emit a `delim` instead | 411 / 2 | covered | — |
| an apostrophe opens a string | only accept a double quote | 413 / 0 | **gap** | `produces every token kind it declares` |
| `#name` becomes a `hash` token | emit a `delim` instead | 413 / 0 | **gap** | `produces every token kind it declares`, `URL tokens come from url( in any casing, quoted or bare` |
| `@name` becomes an `at-keyword` token | emit a `delim` instead | 412 / 1 | covered | — |
| `url(` opens a `url` token | emit a `function` token instead | 407 / 6 | covered | — |
| `URL(` is recognised in any casing | compare the first letter without folding | 413 / 0 | **gap** | `URL tokens come from url( in any casing, quoted or bare` |
| `name(` becomes a `function` token | emit an `ident` instead | 412 / 1 | covered | — |
| a bare name becomes an `ident` token | emit a `delim` instead | 413 / 0 | **gap** | `produces every token kind it declares`, `identifiers start with a hyphen, a double hyphen or a non-ASCII letter`, `escapes resolve to the character they name` |
| a numeric run becomes a `number` token | emit a `delim` instead | 413 / 0 | **gap** | `produces every token kind it declares`, `numbers are recognised with a sign, a leading dot, a fraction and an exponent` |
| a hexadecimal escape resolves to its code point | return the replacement character | 413 / 0 | **gap** | `escapes resolve to the character they name` |
| a hexadecimal escape takes up to six digits | take at most five | 413 / 0 | **gap** | `escapes resolve to the character they name` |
| one space after a hexadecimal escape is its terminator | leave the space in place | 413 / 0 | **gap** | `escapes resolve to the character they name` |
| a non-hexadecimal escape is the literal character | return the replacement character | 412 / 1 | covered | — |
| an escaped newline inside a string contributes nothing | contribute a replacement character | 413 / 0 | **gap** | `escapes resolve to the character they name` |
| the closing quote is not part of the string | append it to the value | 411 / 2 | covered | — |
| an apostrophe-quoted URL value is read as a string | only accept a double quote | 413 / 0 | **gap** | `URL tokens come from url( in any casing, quoted or bare` |
| a bare URL value ends at whitespace without keeping it | append the whitespace | 413 / 0 | **gap** | `URL tokens come from url( in any casing, quoted or bare` |
| an identifier may start with `-` or `--` | refuse a leading hyphen | 413 / 0 | **gap** | `identifiers start with a hyphen, a double hyphen or a non-ASCII letter` |
| an identifier may start with a non-ASCII letter | drop the `code >= 0x80` clause | 413 / 0 | **gap** | `identifiers start with a hyphen, a double hyphen or a non-ASCII letter` |
| a number may start with a decimal point | refuse a leading dot | 413 / 0 | **gap** | `numbers are recognised with a sign, a leading dot, a fraction and an exponent` |
| a number may start with `+` or `-` | refuse a leading sign | 413 / 0 | **gap** | `numbers are recognised with a sign, a leading dot, a fraction and an exponent` |
| a number's fractional part is consumed | consume the decimal point without its digits | 413 / 0 | **gap** | `numbers are recognised with a sign, a leading dot, a fraction and an exponent` |
| a number's exponent is consumed | stop before the exponent | 413 / 0 | **gap** | `numbers are recognised with a sign, a leading dot, a fraction and an exponent` |
| a comment ends at `*/` | look for `+/` instead | 412 / 1 | covered | — |
| a backslash before a newline is not a valid escape | treat every backslash as one | 413 / 0 | **gap** | `escapes resolve to the character they name` |
| an identifier keeps the characters its escapes name | consume the escape and drop the character | 412 / 1 | covered | — |
| a delimiter token carries its own character | emit an empty value | 413 / 0 | **gap** | `produces every token kind it declares`, `identifiers start with a hyphen, a double hyphen or a non-ASCII letter`, `escapes resolve to the character they name` |
| a `#` that names nothing is a delimiter | emit a `hash` token | 413 / 0 | **gap** | `a bare # or @ is a delimiter carrying its own character` |
| an `@` that names nothing is a delimiter | emit an `at-keyword` token | 413 / 0 | **gap** | `a bare # or @ is a delimiter carrying its own character` |

**`plugin/src/ui/svg.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a safe document comes back `ok` and `safe` | return a `parserError` verdict instead | 400 / 13 | covered | — |
| a document exactly at `MAX_SVG_BYTES` is accepted | `>` becomes `>=` | 413 / 0 | **gap** | `a document exactly at the byte ceiling is accepted, counted in UTF-8` |
| an offender name exactly at `MAX_IDENTIFIER_BYTES` is still reported | `>` becomes `>=` | 413 / 0 | **gap** | `an offender name exactly at the identifier ceiling is still reported` |
| a data: payload exactly at `MAX_RASTER_DECODED_BYTES` is accepted | `>` becomes `>=` | 412 / 1 | covered | — |
| `image/png` is an allowed data: media type | drop it from `allowedDataMime` | 411 / 2 | covered | — |
| `image/jpeg` is an allowed data: media type | drop that spelling from `allowedDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `image/jpg` is an allowed data: media type | drop that spelling from `allowedDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `image/webp` is an allowed data: media type | drop it from `allowedDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `font/woff` is an allowed font media type | drop it from `isFontDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `font/woff2` is an allowed font media type | drop it from `isFontDataMime` | 408 / 5 | covered | — |
| `font/ttf` is an allowed font media type | drop it from `isFontDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `font/otf` is an allowed font media type | drop it from `isFontDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `application/font-woff` is an allowed font media type | drop it from `isFontDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `application/x-font-ttf` is an allowed font media type | drop it from `isFontDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| `application/x-font-opentype` is an allowed font media type | drop it from `isFontDataMime` | 413 / 0 | **gap** | `every media type the data: allow-list names is accepted` |
| a same-document fragment reference is accepted | refuse it in `validateReference` | 406 / 7 | covered | — |
| an allowed data: URL is accepted as a reference | refuse every data: reference | 408 / 5 | covered | — |
| a namespace declaration is exempt from the attribute rules | drop the `isXmlnsName` exemption | 412 / 1 | covered | — |
| `xmlns:prefix` is exempt too, not only bare `xmlns` | accept only the bare five-character form | 412 / 1 | covered | — |
| an attribute starting with `o` but not `on` is not an event handler | match on the first letter alone | 411 / 2 | covered | — |
| `href` is held to the reference rule | drop it from `isReferenceAttribute` | 406 / 7 | covered | — |
| an ordinary value in a resource attribute is left alone | drop the `looksLikeActiveUrl` precondition | 409 / 4 | covered | — |
| a semicolon is a separator only in `values` | split every resource attribute on `;` | 412 / 1 | covered | — |
| an at-rule other than `@import` passes the CSS check | refuse every at-keyword, not just the six-character `import` | 472 / 4 | covered | — |
| a `url()` that resolves is accepted | refuse every `url()` token | 408 / 5 | covered | — |
| a local name is compared case-insensitively | drop the `toLowerCase()` | 413 / 0 | **gap** | `element and attribute names are matched without their prefix or casing` |
| a namespace prefix is stripped before comparison | never find the separator | 408 / 5 | covered | — |
| a DOM's own `localName` is preferred when it has one | always fall back to `tagName` | 413 / 0 | **gap** | — *(open)* |
| a DOM offering `getAttributeNames()` is read through it | ignore `getAttributeNames` | 413 / 0 | **gap** | — *(open)* |
| a DOM offering only `attributes` is read through the map | collect no names from the map | 401 / 12 | covered | — |
| a child list offering `item()` is read through it | ignore `childNodes.item` | 413 / 0 | **gap** | `both DOM shapes are walked: modern accessors and a bare attribute map`, `element and attribute names are matched without their prefix or casing` |
| a child list offering only index access is read through it | return `null` from the index arm | 413 / 0 | **gap** | `both DOM shapes are walked: modern accessors and a bare attribute map` |
| the walk descends into child nodes | never recurse | 399 / 14 | covered | — |
| a `<style>` element's text is put through the CSS check | match `styleX` instead | 409 / 4 | covered | — |
| a valid UTF-8 byte transfer decodes | make the decoder throw | 413 / 0 | **gap** | `a UTF-8 byte transfer decodes to the same verdict as the string` |
| a string transfer with no lone surrogate is taken as-is | return `null` instead | 388 / 25 | covered | — |
| a paired surrogate is not treated as a lone one | treat every high surrogate as lone | 413 / 0 | **gap** | `a document exactly at the byte ceiling is accepted, counted in UTF-8`, `a UTF-8 byte transfer decodes to the same verdict as the string` |
| a two-byte character counts as two bytes | count it as three | 413 / 0 | **gap** | `a document exactly at the byte ceiling is accepted, counted in UTF-8` |
| a surrogate pair counts as four bytes | count it as six | 413 / 0 | **gap** | `a document exactly at the byte ceiling is accepted, counted in UTF-8` |
| leading ASCII whitespace is trimmed before a rule runs | stop trimming the front | 413 / 0 | **gap** | `a value padded with ASCII whitespace is trimmed before the rule runs` |
| trailing ASCII whitespace is trimmed before a rule runs | stop trimming the back | 413 / 0 | **gap** | `a value padded with ASCII whitespace is trimmed before the rule runs` |
| a prefix test folds case | compare the left side unfolded | 412 / 1 | covered | — |
| a space is not stripped when normalising a URL | strip spaces too | 412 / 1 | covered | — |
| a `//host` value counts as an active URL | drop the protocol-relative check | 413 / 0 | **gap** | `a scheme is a scheme only when it starts with a letter` |
| a scheme must start with a letter, so `1:2` is not one | drop the leading-letter requirement | 413 / 0 | **gap** | `a scheme is a scheme only when it starts with a letter` |
| the data: payload starts at the first comma after `data:` | start the search one character later | 413 / 0 | not applicable | — |
| the `;base64` parameter is detected | never set the base64 flag | 410 / 3 | covered | — |
| the media type is compared in lower case | keep the original casing | 412 / 1 | covered | — |
| a `%XX` sequence decodes to its byte | keep the literal `%` | 413 / 0 | **gap** | `a percent-encoded data: URL is read as the same bytes as its base64 twin` |
| the data: metadata splits on `;` | treat the whole metadata as one part | 407 / 6 | covered | — |
| a `<?…?>` processing instruction is found in the source | never match the opening `<?` | 411 / 2 | covered | — |
| a processing instruction's pseudo-attributes are classified | skip the classification | 411 / 2 | covered | — |
| an unsafe document still comes back with its source | return an empty source | 410 / 3 | covered | — |
| a rejection names the offender when the name fits | drop the name | 408 / 5 | covered | — |
| a `<script>` element is refused | drop it from the element check | 408 / 5 | covered | — |
| a `<foreignObject>` element is refused | drop it from the element check | 411 / 2 | covered | — |
| a parser error found by `getElementsByTagName` is reported | ignore that signal | 413 / 0 | **gap** | `a parser error is caught however the parser reports it` |
| a parser error named by the root element is reported | ignore that signal | 413 / 0 | **gap** | `a parser error is caught however the parser reports it` |
| a parser that throws yields a `parserError` verdict | return an `INTERNAL_ERROR` failure instead | 411 / 2 | covered | — |
| the verdict carries the rule the walk found | substitute a `parserError` rejection | 406 / 7 | covered | — |

## plugin read

A gap here means a field comes back wrong or a read returns less than it
should. Nothing is dropped and no session stops, which is why the spec puts
this area last — and why it is the largest: thirteen readers, one shared
serializer and one shared visibility predicate, **6,090 lines of production
code** against 314 tests.

It groups into **1,353 accept paths**, one row per mutation that answers
exactly one row. Every mutation below was applied to production code, built,
measured and reverted; the committed diff adds tests only. Baseline before this
task: **476 pass / 0 fail / 2,100 expect() calls / 28 files**. After: **600 pass
/ 0 fail / 2,415 expect() calls / 28 files**.

**942 covered, 400 gaps, 11 not applicable.** 942 + 400 + 11 = 1,353. Of the
400 gaps, **334 are closed** by the new tests and **66 stay open**; 334 + 66 =
400. The diff adds 125 test names, one of which replaces a test that asserted
nothing, so the suite grows by 124.

By file — rows, then covered / gap / not applicable, then of the gaps closed /
open:

| File | Rows | Covered | Gap | N/A | Closed | Open |
|---|---|---|---|---|---|---|
| `serialize.ts` | 381 | 250 | 130 | 1 | 116 | 14 |
| `motion.ts` | 154 | 111 | 41 | 2 | 35 | 6 |
| `reactions.ts` | 121 | 88 | 32 | 1 | 28 | 4 |
| `variables.ts` | 102 | 73 | 28 | 1 | 16 | 12 |
| `render.ts` | 100 | 64 | 34 | 2 | 30 | 4 |
| `navigation.ts` | 98 | 78 | 20 | 0 | 20 | 0 |
| `search.ts` | 88 | 60 | 28 | 0 | 27 | 1 |
| `components.ts` | 84 | 58 | 25 | 1 | 19 | 6 |
| `dev-mode.ts` | 74 | 52 | 21 | 1 | 15 | 6 |
| `styles.ts` | 69 | 47 | 21 | 1 | 14 | 7 |
| `fonts.ts` | 58 | 40 | 17 | 1 | 13 | 4 |
| `common.ts` | 14 | 13 | 1 | 0 | 1 | 0 |
| `visibility.ts` | 10 | 8 | 2 | 0 | 0 | 2 |

381 + 154 + 121 + 102 + 100 + 98 + 88 + 84 + 74 + 69 + 58 + 14 + 10 = 1,353.

The Scope table's **72** is right, and worth pinning down because the brief's
own Step-1 command disagrees with it: run against the tree at `f65c29e`, the
plan's command — which subtracts an acceptance-word filter — finds exactly 72
refusal-named tests in `plugin/src/read`, and the brief's, which does not, finds
84. Both count the same 314 tests, and neither is the row count: the rows come
from the production code.

The Mutation column shows the replacement text rather than a prose description
of it, because at this volume the text is the only unambiguous record of what
was cut; `⏎` stands for a newline. The Accept path names the site.

### The cancellation question, answered for the read side

Task 6 measured that the ten `checks cancellation …` read tests stay green when
`throwIfAbortedAtBatch` is emptied at all fifteen call sites, and left the
question here. Deleting each of those fifteen calls one at a time is `B01`–`B15`
below: all fifteen stay green, confirming that measurement site by site.

The reason is not that the batched check is uniquely dead. Each of the fifteen
sits beside an unconditional `signal?.throwIfAborted()` on the same signal, and
deleting *those* one at a time is `B16`–`B30`: **fourteen of the fifteen also
stay green.** The single exception is `serializeNode`'s own check, `B20`, which
goes red — and now goes red against a test written for it, `the cancellation
signal reaches the forest serializer`.

So the honest statement is not "the batched check is dead" but "every read path
notices cancellation at two or more independent points, and only one of those
points is individually load-bearing". Twenty-nine of the sixty-six open rows
are these, and none of them can be closed from the test side: a test can only
observe that the read was cancelled, and every one of these deletions leaves
another check standing that cancels it. Closing them would mean removing the
duplication in production, which this sweep does not do.

### The milestone test that asserted nothing

`plugin/src/main/dispatch.test.ts`'s `every named milestone operation returns a
typed unavailable error` looped over `OPERATION_NAMES` and `continue`d on all
thirteen names. Its body never ran and it contributed no `expect()` call: the
list of exceptions had grown, one implemented operation at a time, until it
covered the list itself.

The claim still worth holding is the complement — that no name in
`OPERATION_NAMES` falls through to the unavailable answer — so the test now
asserts that once per name, and that it ran thirteen times. It is worth fifteen
`expect()` calls where it was worth none. Measured: making `get_fonts` throw
`CAPABILITY_UNAVAILABLE` takes the suite from 600/0 to 598/2, and under the old
test that same change moved nothing in that file.

This is the one change outside `plugin/src/read`. It is in scope because a test
that asserts nothing is not a working refusal test.

### The largest finding: get_screenshot's UI round-trip had no coverage at all

Every screenshot test at the branch point passed its own `codec`, so
`createUiCodec`, `requestUiValidation`, `completeScreenshotValidation` and
`settlePendingValidation` — the whole conversation between the controller and
the iframe — were reached by nothing. Twenty-five of `render.ts`'s thirty-four gaps
are in that path: the message type, the correlation id, the item payload, both
codec halves in both directions, the reply handler, and every field that
crosses back.

Two of those mutations were not merely green: they hung the suite. Deleting
`pending.resolve(result)` (`R24`) or `pendingValidations.set(...)` (`R37`) leaves
two existing tests waiting until bun's five-second per-test timeout, and the run
was killed at the driver's 180-second ceiling with no summary. The streamed
`(fail)` lines are the measurement — both rows are **covered**, by
`keeps the SVG verdict across the UI validation round trip` and
`times out a hung screenshot validation as TIMEOUT` — and the table says so
rather than recording them as unmeasurable.

### Vocabularies nothing checked

The single largest class of gaps is the lookup table with one member pinned and
the rest not. Expanded per member, as the enumeration rule requires:

- **19 blend modes.** Seventeen were unpinned. The two that stay open are
  `PASS_THROUGH` and `NORMAL`, and they are open for a reason rather than by
  omission: `blendMode()` returns `undefined` both for a mode missing from the
  table and for those two, so deleting either entry cannot change an answer.
- **15 motion easing types**, of which five were unpinned; **9 keyframe value
  shapes**, all nine unpinned; **3 keyframe operations**, two unpinned.
- **9 media runtime actions and 8 overlay positions** in `reactions.ts`: every
  one unpinned, plus the `none` background and the `none` background
  interaction.
- **4 image scale modes** (three unpinned), **4 effect types** (two unpinned),
  **3 stroke aligns** (one unpinned), the text aligns and auto-resize modes,
  **5 axis aligns**, **5 constraint axes**, **3 sizing modes**, and all **5
  style-id fields** with their kinds.

None of these is a subtle path. Each is a case arm whose absence turns a real
design value into `undefined` on the wire, and each is now a row in a table a
test compares whole.

### The same four questions, thirteen times

Seven of the thirteen readers carry their own emission class with the same
shape: a returned-node ceiling, a byte ceiling, a `visitedNodes` count, and a
precedence rule between a truncated walk and the emission cut. The sweep asked
the same four questions of each, and the answers were remarkably uniform: the
ceilings' *existence* was usually covered, and their *edges* — whether the
comparison is inclusive, whether bytes accumulate across items or are measured
per item, what number the truncation reports — almost never were.

Every one of those edges is now pinned by a test that derives its budget from
`byteLength` applied to an expected payload the test builds from the protocol,
rather than from what the reader returned. That distinction is the difference
between a test that would notice an off-by-one and one that agrees with it.

The `visitedNodes` field of those limits is the seven-times-repeated dead
field: all seven emission classes accept it in their `SerializerLimits` and none
of the seven reads it. Those seven rows, plus four other dead fields — the two
byte counters `createUiCodec` fills in that `encodeAsset` never reads,
`VARIABLE_ALIAS` in motion's easing set, which `motionEasing` returns before
consulting, and the forest walk context's `depth`, which `walkNode` never
reads — are the eleven `not applicable` entries.

### What is left open

Sixty-six rows. Twenty-nine are the cancellation duplication above. The other
thirty-seven fall into five kinds, and the kind is the point rather than the
count:

- **A guard whose twin always fires first (16).** `resolve`'s
  `if (this.budget.exhausted) return EXHAUSTED` in `variables.ts` can only run
  if the deadline passes between two consecutive synchronous clock reads with no
  `await` between them (`V37`); `lookupVariable`'s own budget gate is checked
  again by every caller before it (`V24`); `readNodes`'s per-item
  `lookup === undefined` arm cannot be reached because the pre-flight throws
  first (`R98`). `visibility.ts`'s cycle watch is the clearest case: a revisited
  node and an exhausted walk bound both return `undetermined`, so removing the
  `seen` set or raising `CYCLE_WATCH_AFTER` changes how many steps the walk
  takes and nothing else (`Z699`, `Z700`). Also `Z459` — a second `depthLimit`
  is always identical to the first, since `appliedDepth` is a constant of the
  read.
- **A loop that stops rather than continues, once the answer is latched (9).**
  Nine readers write `break` where `continue` would do; by the time the ceiling
  is hit the truncation is already recorded and every later iteration is a
  no-op, so the two spellings produce identical results. The `break` is right,
  and nothing outside the module can see it.
- **The first-truncation-wins guard, called once (5).** `mark()` keeps the first
  walk truncation, but in five readers it is called exactly once per read, so
  the guard has nothing to guard against. `components.ts` is the exception —
  its budget marks a second time — and that row is closed.
- **A shape no real host produces (2).** `styles.ts`'s `isMixedStyleId` compares
  a style id against `figma.mixed`, but the comparison sits behind
  `typeof value !== "string"`, and every real host's `figma.mixed` is a symbol
  (`ST22`). `byteLength`'s lone-surrogate branch needs a lone surrogate to
  survive `JSON.stringify`, which well-formed stringification prevents
  (`Z463`).
- **Bookkeeping the next line discards (5).** A settled screenshot validation
  clears its timeout and unhooks its abort listener (`R22`, `R23`) — both act on
  an id already removed from the pending map. A cancelled validation's
  `CANCELLED` item (`R35`) is produced and then thrown away by the
  `signal?.throwIfAborted()` on the very next line. `SE58`, `D20`, `D48`,
  `M76`, `M88`, `V30`, `V36`, `V43`, `Z270` and `F56` are the same shape: work
  that is correct, and that a second mechanism makes invisible.

### Two maintenance edges

- **`plugin/src/read/figma-api-conformance.ts` is imported by nothing.** It
  appears only in the `exclude` lists of `tsconfig.tests.json` and
  `tsconfig.controller-tests.json`. It is 180 lines and was not enumerated,
  because a file no build or test reaches has no accept path to mutate. Whoever
  owns it should know it is unreferenced.
- **Seven `expect(PluginReadError).toBeDefined()` lines.** They appear at the
  end of the cancellation test in `components`, `motion`, `search`, `dev-mode`,
  `reactions`, `styles` and `variables`. Each asserts that an imported class is
  defined, which cannot fail; they keep the import used and add seven to the
  `expect()` count without pinning anything. They were left alone — the tests
  they sit in do work, through the `rejects.toThrow` above them.

### How this enumeration went wrong, and what it cost

The table generator matched each lookup-table member by its own source line.
`  CENTER: "center",` appears in five tables in `serialize.ts`, so one call for
`STROKE_ALIGNS` produced five rows: one correct and four carrying the
stroke-align label while mutating `TEXT_ALIGN_HORIZONTALS`,
`TEXT_ALIGN_VERTICALS`, `AXIS_ALIGNS` and `CONSTRAINT_AXES`. Two of the four
duplicated rows generated later for the same members.

Found by reading the generated labels against the mutations, not by a run: the
mutations were all distinct and all correct, and only the Accept path column
lied. Two rows were relabelled, two were dropped as duplicates, and a check that
no Accept path repeats within one file now runs over the whole enumeration — it
also caught `N46` and `N74`, two different unresolved-node sites in
`navigation.ts` that had been given the same label. The row count fell from
1,355 to 1,353.

### Verification

`cd plugin && bun run build && bun run test` — **600 pass / 0 fail / 2,415
expect() calls / 28 files**, from a baseline of 476 / 0 / 2,100 / 28.
`bun run format:check` and `bun run typecheck` clean. No Rust file was touched.

Every mutation was applied by a driver that copies the file first, applies one
exact string replacement, builds, runs the whole suite, restores from the copy
and compares SHA-256 against the pre-mutation value; a run that does not restore
byte for byte aborts the batch. `git status --porcelain --untracked-files=all`
was empty before each of the ten commits.

Each of the 334 closed rows was re-measured with its mutation re-applied after
the test that closes it was in place, and the test named in the last column is
the one the run reported failing — the attribution is measured, not asserted.
The 66 open rows were re-measured together against the finished suite and are
green there.

### The table

**`plugin/src/read/navigation.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| the cancellation signal reaches the forest serializer | `return options` | 476 / 0 | **gap** | `the cancellation signal reaches the forest serializer` |
| a plugin read error carries a message | `super("")` | 476 / 0 | **gap** | `a plugin read error names itself and carries a message` |
| a plugin read error names itself | `this.name = "Error"` | 476 / 0 | **gap** | `a plugin read error names itself and carries a message` |
| detail defaults to compact | `return input.detail ?? "minimal"` | 476 / 0 | **gap** | `detail defaults to compact and depth to two, capped at MAX_DEPTH` |
| a caller-supplied detail is honoured | `return "compact"` | 469 / 7 | covered | — |
| depth defaults to two | `return Math.min(depth ?? 1, MAX_DEPTH)` | 476 / 0 | **gap** | `detail defaults to compact and depth to two, capped at MAX_DEPTH` |
| a caller-supplied depth is capped at MAX_DEPTH | `return depth ?? 2` | 476 / 0 | **gap** | `detail defaults to compact and depth to two, capped at MAX_DEPTH` |
| a caller-supplied depth below the cap is honoured | `return MAX_DEPTH ⏎ }` | 473 / 3 | covered | — |
| the page ceiling is exclusive at the boundary | `const truncated = pageCount > MAX_RETURNED_NODES + 1` | 475 / 1 | covered | — |
| metadata carries the file name | `file: { name: "", editorType: figma.editorType },` | 472 / 4 | covered | — |
| metadata carries the editor type | `file: { name: figma.root.name, editorType: "" },` | 473 / 3 | covered | — |
| each listed page carries its name | `pages: figma.root.children.slice(0, MAX_RETURNED_NODES).map((page) => ({ ⏎ id: page.id, ⏎ name: "", ⏎ })),` | 473 / 3 | covered | — |
| each listed page carries its id | `id: "", ⏎ name: page.name,` | 473 / 3 | covered | — |
| the page list is capped at MAX_RETURNED_NODES | `pages: figma.root.children.slice(0, 1).map` | 474 / 2 | covered | — |
| metadata names the current page | `currentPageId: "",` | 473 / 3 | covered | — |
| metadata carries the plugin version | `pluginVersion: "",` | 473 / 3 | covered | — |
| metadata carries the detected capabilities | `capabilities: {} as ReturnType<typeof detectCapabilities>,` | 476 / 0 | **gap** | `metadata reports the capabilities of the host it is given` |
| metadata's truncated flag reflects the page count | `truncated: false, ⏎ observation: { startedAt, completedAt: new Date().toISOString() },` | 475 / 1 | covered | — |
| metadata's observation.startedAt is the time the read began | `observation: { startedAt: "", completedAt: new Date().toISOString() },` | 475 / 1 | covered | — |
| metadata's observation.completedAt is emitted | `observation: { startedAt } as { startedAt: string; completedAt: string },` | 475 / 1 | covered | — |
| a truncated page list carries its truncation | `if (false) ⏎ result.truncation = { reason: "nodeLimit", visitedNodes: pageCount }` | 475 / 1 | covered | — |
| the current selection's node ids are captured | `return []` | 466 / 10 | covered | — |
| an unwalkable node is recorded as unresolved | `if (unresolved.length >= 0) return` | 472 / 4 | covered | — |
| an unresolved entry names the node | `unresolved.push({ id: "", error: nodeError("LIMIT_EXCEEDED") })` | 472 / 4 | covered | — |
| an unresolved entry reports LIMIT_EXCEEDED | `unresolved.push({ id, error: nodeError("INTERNAL_ERROR") })` | 473 / 3 | covered | — |
| a node is fetched by the id asked for | `return await figma.getNodeByIdAsync("")` | 393 / 83 | covered | — |
| a lookup that throws is treated as a miss | `} catch (e) { ⏎ throw e ⏎ }` | 475 / 1 | covered | — |
| a DOCUMENT root has its pages loaded | `if (!isRecord(root) \|\| root.type !== "DOCUMENT_X") continue` | 475 / 1 | covered | — |
| each page under a DOCUMENT root is loaded | `void child` | 475 / 1 | covered | — |
| document pages are loaded before serializing | `void roots` | 475 / 1 | covered | — |
| a minimal read skips the three name pre-passes | `if (false) { ⏎ return serializeNodeForest(roots, serializeOptions(options, signal)) ⏎ }` | 476 / 0 | **gap** | `a minimal read resolves no names and no instance identities` |
| a full read resolves style and variable names | `const isFull = false` | 474 / 2 | covered | — |
| instance identities are collected for every non-minimal read | `Promise.resolve(undefined),` | 475 / 1 | covered | — |
| the instance pre-pass is bounded by the requested depth | `collectInstanceIdentities(roots, signal, 0),` | 476 / 0 | **gap** | `a minimal read resolves no names and no instance identities`, `the instance pre-pass stops at the requested depth` |
| a full read with a style lookup collects style names | `false && styleLookup !== undefined` | 475 / 1 | covered | — |
| the style name pre-pass calls the host lookup | `() => Promise.resolve(undefined),` | 475 / 1 | covered | — |
| a full read with a variable lookup collects variable names | `false && variablesApi !== undefined && variableLookup !== undefined` | 475 / 1 | covered | — |
| the variable name pre-pass calls the host lookup | `() => Promise.resolve(undefined),` | 475 / 1 | covered | — |
| collected instance identities reach the serializer | `instanceIdentities: undefined,` | 475 / 1 | covered | — |
| collected style names reach the serializer | `...{},` | 475 / 1 | covered | — |
| collected variable names reach the serializer | `...{},` | 475 / 1 | covered | — |
| a selection read starts from the captured ids | `const ids: string[] = [] ⏎ const detail = defaultDetail(input)` | 470 / 6 | covered | — |
| a host with an id lookup resolves the selection | `if (ids.length > 0 && figma.getNodeByIdAsync === undefined) { ⏎ throw new PluginReadError("CAPABILITY_UNAVAILABLE",…` | 471 / 5 | covered | — |
| a selected node the host cannot find does not end the read | `if (node === null \|\| node === undefined) return {} as never ⏎ const verdict = visibilityOf(node) ⏎ // A switc…` | 476 / 0 | **gap** | `a selected id the host cannot find does not end the read` |
| a rendering selected node is serialized | `if (verdict === "renders") void node ⏎ else if (verdict === "undetermined") recordUnresolved(unresolved, id) ⏎ …` | 473 / 3 | covered | — |
| an unwalkable selected node is reported as unresolved by get_selection | `else if (false) recordUnresolved(unresolved, id) ⏎ } ⏎ }` | 474 / 2 | covered | — |
| a selection read does not dedupe components | `{ detail, depth, dedupeComponents: true }, ⏎ signal, ⏎ ) ⏎ const result = { ⏎ detail, ⏎ nodes: serialized.nodes,` | 476 / 0 | **gap** | `a selection read serializes a repeated component in full both times` |
| a selection result echoes the detail level used | `detail: "minimal", ⏎ nodes: serialized.nodes,` | 475 / 1 | covered | — |
| a selection result carries the serialized nodes | `nodes: [],` | 473 / 3 | covered | — |
| a selection result's truncated flag comes from the walk | `truncated: false, ⏎ ...(serialized.truncation === undefined ⏎ ? {} ⏎ : { truncation: serialized.truncation …` | 475 / 1 | covered | — |
| a selection result carries the walk's truncation | `: {}), ⏎ ...(unresolved.length > 0 ? { unresolved } : {}), ⏎ observation: observation(startedAt), ⏎ } ⏎ return re…` | 475 / 1 | covered | — |
| a selection result carries its unresolved nodes | `...{}, ⏎ observation: observation(startedAt), ⏎ } ⏎ return result as GetSelectionResult` | 474 / 2 | covered | — |
| a node error carries the canonical message for its code | `return { code, message: "", retryable: false }` | 468 / 8 | covered | — |
| a node error carries the code it was given | `return { code: "INTERNAL_ERROR", message: CANONICAL_MESSAGES[code], retryable: false }` | 466 / 10 | covered | — |
| a node error is not retryable | `return { code, message: CANONICAL_MESSAGES[code], retryable: true } ⏎ }` | 467 / 9 | covered | — |
| a read with no id lookup fails each item as CAPABILITY_UNAVAILABLE | `error: nodeError("INTERNAL_ERROR"),` | 476 / 0 | **gap** | `every item fails as CAPABILITY_UNAVAILABLE when the host has no id lookup` |
| a node the host does not have fails that item as NODE_NOT_FOUND | `items.push({ status: "error", error: nodeError("INTERNAL_ERROR") })` | 474 / 2 | covered | — |
| a rendering node named by id is serialized | `const verdict = visibilityOf(node) ⏎ if (verdict === "renders") {` | 464 / 12 | covered | — |
| a hidden node and an unwalkable chain are told apart | `verdict === "hidden" ? "LIMIT_EXCEEDED" : "NODE_NOT_VISIBLE",` | 473 / 3 | covered | — |
| a PAGE named by id is loaded before serializing | `[node],` | 476 / 0 | **gap** | `an explicitly named page is paged in before it is serialized` |
| the serialized node reaches its item | `const value = serialized.nodes[1]` | 466 / 10 | covered | — |
| a resolved node is a success item | `items.push({ status: "error", value } as never)` | 469 / 7 | covered | — |
| the first truncated item's truncation is reported | `if (serialized.truncated && !truncated) { ⏎ truncated = true ⏎ }` | 475 / 1 | covered | — |
| a truncated item truncates the whole read | `if (false) {` | 475 / 1 | covered | — |
| a read error raised while serializing an item ends the call | `if (false) { ⏎ throw error ⏎ } ⏎ items.push({ status: "error", error: nodeError("INTERNAL_ERROR") })` | 476 / 0 | **gap** | `a read error raised while serializing an item ends the whole call` |
| a get_nodes result echoes the detail level used | `detail: "minimal", ⏎ items, ⏎ truncated,` | 473 / 3 | covered | — |
| a get_nodes result carries its items | `items: [], ⏎ truncated,` | 463 / 13 | covered | — |
| a get_nodes result carries its truncation | `...{},` | 475 / 1 | covered | — |
| an explicit pageId naming a PAGE is accepted | `node.type !== "PAGE_X" ⏎ ) {` | 460 / 16 | covered | — |
| an explicitly named page is loaded | `return node ⏎ }` | 468 / 8 | covered | — |
| no selector means the current page | `if (selector === undefined) ⏎ return { roots: [], unresolved: [] }` | 453 / 23 | covered | — |
| a selection selector resolves the current selection | `if ("selection_x" in selector) {` | 472 / 4 | covered | — |
| a rendering selected node becomes a scope root | `if (verdict === "renders") void node ⏎ else if (verdict === "undetermined") recordUnresolved(unresolved, id) ⏎ …` | 473 / 3 | covered | — |
| an unwalkable selected node is reported as unresolved by a selection scope | `else if (false) recordUnresolved(unresolved, id) ⏎ } ⏎ return { roots, unresolved } ⏎ } ⏎ if ("nodeId" in selecto…` | 475 / 1 | covered | — |
| a nodeId selector resolves that one node | `if ("nodeId_x" in selector) {` | 422 / 54 | covered | — |
| a hidden node named by nodeId yields an empty scope | `if (verdict === "hidden_x") return { roots: [], unresolved: [] }` | 475 / 1 | covered | — |
| an unwalkable node named by nodeId is reported as unresolved | `if (false) ⏎ return { ⏎ roots: [],` | 475 / 1 | covered | — |
| the unresolved entry names the node asked for | `{ id: "", error: nodeError("LIMIT_EXCEEDED") },` | 475 / 1 | covered | — |
| a PAGE passed as nodeId is loaded | `return { roots: [node], unresolved: [] }` | 475 / 1 | covered | — |
| a nodeIds selector resolves each id | `if ("nodeIds_x" in selector) {` | 472 / 4 | covered | — |
| a PAGE among nodeIds is loaded | `if (verdict === "renders") roots.push(node)` | 476 / 0 | **gap** | `an explicitly named page is paged in before it is serialized` |
| a rendering node among nodeIds becomes a scope root | `if (verdict === "renders") void node ⏎ else if (verdict === "undetermined") recordUnresolved(unresolved, id)` | 473 / 3 | covered | — |
| an unwalkable node among nodeIds is reported as unresolved | `else if (false) recordUnresolved(unresolved, id) ⏎ } ⏎ return { roots, unresolved } ⏎ } ⏎ if ("pageId" in selector)` | 475 / 1 | covered | — |
| a pageId selector loads that page | `if ("pageId_x" in selector) ⏎ return { ⏎ roots: [await loadExplicitPage(selector.pageId, signal)],` | 461 / 15 | covered | — |
| a pageIds selector loads every page it names | `if ("pageIds_x" in selector) {` | 473 / 3 | covered | — |
| each explicitly named page becomes a scope root | `await loadExplicitPage(id, signal)` | 474 / 2 | covered | — |
| a design-context read honours the caller's selector | `const { roots, unresolved } = await resolveDesignRoots(undefined, signal)` | 466 / 10 | covered | — |
| a design-context read honours dedupeComponents | `dedupeComponents: false,` | 476 / 0 | **gap** | `a design-context read honours dedupeComponents and echoes its detail` |
| a design-context result echoes the detail level used | `detail: "minimal", ⏎ roots: serialized.nodes,` | 476 / 0 | **gap** | `detail defaults to compact and depth to two, capped at MAX_DEPTH` |
| a design-context result carries the serialized roots | `roots: [],` | 470 / 6 | covered | — |
| a design-context result's truncated flag comes from the walk | `truncated: false, ⏎ ...(serialized.truncation === undefined ⏎ ? {} ⏎ : { truncation: serialized.truncation …` | 475 / 1 | covered | — |
| a design-context result carries the walk's truncation | `: {}), ⏎ ...(unresolved.length > 0 ? { unresolved } : {}), ⏎ observation: observation(startedAt), ⏎ } ⏎ return re…` | 475 / 1 | covered | — |
| a design-context result carries its unresolved nodes | `...{}, ⏎ observation: observation(startedAt), ⏎ } ⏎ return result as GetDesignContextResult` | 473 / 3 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 476 / 0 | **gap** | `a design-context read honours dedupeComponents and echoes its detail` |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 474 / 2 | covered | — |
| cancellation is checked before a nodeId lookup | `const node = await lookupNode(selector.nodeId)` | 476 / 0 | **gap** | `cancellation is checked before a nodeId lookup and before an explicit page load` |
| cancellation is checked before an explicit page lookup | `const node = await lookupNode(id)` | 476 / 0 | **gap** | `cancellation is checked before a nodeId lookup and before an explicit page load` |
| a missing id among nodeIds fails the whole call | `if (node === null \|\| node === undefined) { ⏎ continue ⏎ } ⏎ const verdict = visibilityOf(node) ⏎ if…` | 475 / 1 | covered | — |

**`plugin/src/read/serialize.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a host symbol sentinel is recognised as mixed | `function isMixed(value: unknown): boolean { ⏎ return value === "mixed"` | 474 / 2 | covered | — |
| the literal string mixed is recognised as mixed | `return typeof value === "symbol" ⏎ }` | 476 / 0 | **gap** | `a mixed paint list, a mixed stroke list and a mixed corner radius are each reported` |
| a colour carries its r channel | `r: 0,` | 472 / 4 | covered | — |
| a colour carries its g channel | `g: 0,` | 473 / 3 | covered | — |
| a colour carries its b channel | `b: 0,` | 469 / 7 | covered | — |
| a colour with no alpha defaults to opaque | `a: finite(color.a, 0),` | 473 / 3 | covered | — |
| a colour carries its alpha | `a: 1,` | 474 / 2 | covered | — |
| a transform carries m00 | `m00: 0,` | 474 / 2 | covered | — |
| a transform carries m01 | `m01: 0,` | 475 / 1 | covered | — |
| a transform carries m02 | `m02: 0,` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| a transform carries m10 | `m10: 0,` | 475 / 1 | covered | — |
| a transform carries m11 | `m11: 0,` | 474 / 2 | covered | — |
| a transform carries m12 | `m12: 0,` | 475 / 1 | covered | — |
| a transform's first row is the host's first row | `const row0 = array(rows[1])` | 474 / 2 | covered | — |
| a transform's second row is the host's second row | `const row1 = array(rows[0])` | 474 / 2 | covered | — |
| a mixed paint value is reported as the mixed marker | `if (false) return { type: "mixed" } ⏎ const paint = record(value)` | 476 / 0 | **gap** | `a mixed paint list, a mixed stroke list and a mixed corner radius are each reported` |
| a paint with no visible flag is treated as painting | `if (!boolean(paint.visible, false)) return undefined` | 458 / 18 | covered | — |
| a paint switched off in Figma is dropped | `if (false) return undefined` | 471 / 5 | covered | — |
| a SOLID paint is reported as solid | `case "SOLID_X": ⏎ return { ⏎ type: "solid",` | 471 / 5 | covered | — |
| a solid paint carries its colour | `type: "solid", ⏎ color: toColor({}),` | 472 / 4 | covered | — |
| a solid paint carries its opacity | `color: toColor(paint.color), ⏎ opacity: 1,` | 475 / 1 | covered | — |
| GRADIENT_LINEAR maps to the linearGradient wire tag | delete it | 473 / 3 | covered | — |
| GRADIENT_RADIAL maps to the radialGradient wire tag | delete it | 475 / 1 | covered | — |
| GRADIENT_ANGULAR maps to the angularGradient wire tag | delete it | 474 / 2 | covered | — |
| GRADIENT_DIAMOND maps to the diamondGradient wire tag | delete it | 474 / 2 | covered | — |
| a GRADIENT_LINEAR paint is serialized as a gradient | delete it | 472 / 4 | covered | — |
| a GRADIENT_RADIAL paint is serialized as a gradient | delete it | 475 / 1 | covered | — |
| a GRADIENT_ANGULAR paint is serialized as a gradient | delete it | 474 / 2 | covered | — |
| a GRADIENT_DIAMOND paint is serialized as a gradient | `case "GRADIENT_DIAMOND_X": {` | 474 / 2 | covered | — |
| a gradient stop carries its position | `position: 0,` | 475 / 1 | covered | — |
| a gradient stop carries its colour | `color: toColor({}), ⏎ } ⏎ })` | 474 / 2 | covered | — |
| a gradient carries its transform | `gradientTransform: toTransform(undefined),` | 475 / 1 | covered | — |
| a gradient carries its stops | `stops: [], ⏎ gradientTransform:` | 474 / 2 | covered | — |
| a gradient carries its opacity | `gradientTransform: toTransform(paint.gradientTransform), ⏎ opacity: 1,` | 473 / 3 | covered | — |
| an IMAGE paint is reported as image | `case "IMAGE_X": ⏎ return { ⏎ type: "image",` | 473 / 3 | covered | — |
| an image paint carries the host's imageHash | `imageRef: "",` | 474 / 2 | covered | — |
| an image paint carries its scale mode | `scaleMode: "fill",` | 474 / 2 | covered | — |
| an image paint carries its opacity | `scaleMode: imageScaleMode(paint.scaleMode), ⏎ opacity: 1,` | 474 / 2 | covered | — |
| an unsupported paint keeps Figma's own type name | `return figmaType === "" ? undefined : { type: "unsupported", figmaType: "" }` | 472 / 4 | covered | — |
| a mixed paints list is reported as one mixed marker | `if (false) return [{ type: "mixed" }]` | 475 / 1 | covered | — |
| a parsed paint reaches the paints list | `return []` | 456 / 20 | covered | — |
| the FIT scale mode is reported as fit | `case "FIT": ⏎ return "fill"` | 474 / 2 | covered | — |
| the CROP scale mode is reported as crop | `case "CROP": ⏎ return "fill"` | 476 / 0 | **gap** | `every image scale mode is reported, and an unknown one falls back to fill` |
| the TILE scale mode is reported as tile | `case "TILE": ⏎ return "fill"` | 476 / 0 | **gap** | `every image scale mode is reported, and an unknown one falls back to fill` |
| an unrecognised scale mode defaults to fill | `default: ⏎ return "tile" ⏎ } ⏎ }` | 476 / 0 | **gap** | `every image scale mode is reported, and an unknown one falls back to fill` |
| an effect with no visible flag is treated as rendering | `if (!boolean(source.visible, false)) continue` | 472 / 4 | covered | — |
| an effect switched off in Figma is dropped | `if (false) continue` | 473 / 3 | covered | — |
| a DROP_SHADOW effect is reported as dropShadow | `type: "innerShadow",` | 473 / 3 | covered | — |
| an INNER_SHADOW effect is reported as innerShadow | `type: "dropShadow",` | 476 / 0 | **gap** | `every effect the serializer models is reported under its own tag` |
| a DROP_SHADOW effect is serialized | delete it | 473 / 3 | covered | — |
| an INNER_SHADOW effect is serialized | delete it | 476 / 0 | **gap** | `every effect the serializer models is reported under its own tag` |
| a shadow carries its colour | `color: toColor({}), ⏎ offsetX:` | 474 / 2 | covered | — |
| a shadow carries its x offset | `offsetX: 0,` | 474 / 2 | covered | — |
| a shadow carries its y offset | `offsetY: 0,` | 473 / 3 | covered | — |
| a shadow carries its radius | `radius: 0, ⏎ spread:` | 473 / 3 | covered | — |
| a shadow carries its spread | `spread: 0,` | 473 / 3 | covered | — |
| a LAYER_BLUR effect is reported as layerBlur | `type: "backgroundBlur",` | 475 / 1 | covered | — |
| a BACKGROUND_BLUR effect is reported as backgroundBlur | `type: "layerBlur",` | 476 / 0 | **gap** | `every effect the serializer models is reported under its own tag` |
| a LAYER_BLUR effect is serialized | delete it | 475 / 1 | covered | — |
| a BACKGROUND_BLUR effect is serialized | delete it | 476 / 0 | **gap** | `every effect the serializer models is reported under its own tag` |
| a blur carries its radius | `radius: 0, ⏎ }) ⏎ break` | 475 / 1 | covered | — |
| an unsupported effect keeps Figma's own type name | `if (figmaType !== "") result.push({ type: "unsupported", figmaType: "" })` | 474 / 2 | covered | — |
| the INSIDE stroke align is reported as inside | delete it | 475 / 1 | covered | — |
| the OUTSIDE stroke align is reported as outside | delete it | 472 / 4 | covered | — |
| the CENTER stroke align is reported as center | `OUTSIDE: "outside",` | 476 / 0 | **gap** | `every stroke align, text align and auto-resize is reported` |
| the CENTER horizontal text align is reported as center | `const TEXT_ALIGN_HORIZONTALS: Record<string, TextAlignHorizontal> = {` | 475 / 1 | covered | — |
| the CENTER vertical text align is reported as center | `const TEXT_ALIGN_VERTICALS: Record<string, TextAlignVertical> = {` | 476 / 0 | **gap** | `every stroke align, text align and auto-resize is reported` |
| the PASS_THROUGH blend mode is reported as passThrough | delete it | 476 / 0 | **gap** | — *(open)* |
| the NORMAL blend mode is reported as normal | delete it | 476 / 0 | **gap** | — *(open)* |
| the DARKEN blend mode is reported as darken | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the MULTIPLY blend mode is reported as multiply | delete it | 475 / 1 | covered | — |
| the LINEAR_BURN blend mode is reported as linearBurn | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the COLOR_BURN blend mode is reported as colorBurn | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the LIGHTEN blend mode is reported as lighten | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the SCREEN blend mode is reported as screen | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the LINEAR_DODGE blend mode is reported as linearDodge | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the COLOR_DODGE blend mode is reported as colorDodge | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the OVERLAY blend mode is reported as overlay | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the SOFT_LIGHT blend mode is reported as softLight | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the HARD_LIGHT blend mode is reported as hardLight | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the DIFFERENCE blend mode is reported as difference | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the EXCLUSION blend mode is reported as exclusion | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the HUE blend mode is reported as hue | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the SATURATION blend mode is reported as saturation | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the COLOR blend mode is reported as color | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the LUMINOSITY blend mode is reported as luminosity | delete it | 476 / 0 | **gap** | `every blend mode is reported, and the two Figma defaults are omitted` |
| the UNDERLINE text decoration is reported as underline | delete it | 475 / 1 | covered | — |
| the STRIKETHROUGH text decoration is reported as strikethrough | delete it | 475 / 1 | covered | — |
| the RIGHT horizontal text align is reported as right | delete it | 476 / 0 | **gap** | `every stroke align, text align and auto-resize is reported` |
| the JUSTIFIED horizontal text align is reported as justified | delete it | 476 / 0 | **gap** | `every stroke align, text align and auto-resize is reported` |
| the BOTTOM vertical text align is reported as bottom | delete it | 475 / 1 | covered | — |
| the WIDTH_AND_HEIGHT text auto-resize is reported as widthAndHeight | delete it | 475 / 1 | covered | — |
| the HEIGHT text auto-resize is reported as height | delete it | 476 / 0 | **gap** | `every stroke align, text align and auto-resize is reported` |
| the TRUNCATE text auto-resize is reported as truncate | delete it | 476 / 0 | **gap** | `every stroke align, text align and auto-resize is reported` |
| the SPACE_BETWEEN axis align is reported as spaceBetween | delete it | 475 / 1 | covered | — |
| the BASELINE axis align is reported as baseline | delete it | 475 / 1 | covered | — |
| the STRETCH layout constraint is reported as stretch | delete it | 475 / 1 | covered | — |
| the SCALE layout constraint is reported as scale | delete it | 475 / 1 | covered | — |
| a node whose strokes are all switched off reports no stroke value | `const liveStrokes = array(rawStrokes)` | 475 / 1 | covered | — |
| a mixed stroke list still reports weight and align | `if (liveStrokes.length === 0) return undefined` | 476 / 0 | **gap** | `a mixed paint list, a mixed stroke list and a mixed corner radius are each reported` |
| a stroke value carries its paints | `const value: StrokeValue = { paints: [] }` | 473 / 3 | covered | — |
| a stroke value carries its weight | `if (false) value.weight = weight as number` | 472 / 4 | covered | — |
| a stroke value carries its align | `if (false) value.align = align` | 471 / 5 | covered | — |
| a stroke value carries its dash pattern | `if (false) value.dashPattern = dashPattern` | 475 / 1 | covered | — |
| a numeric dash pattern survives the filter | `const dashPattern: number[] = []` | 475 / 1 | covered | — |
| a uniform corner radius carries its value | `return radius === undefined \|\| radius === 0 ⏎ ? undefined ⏎ : { kind: "uniform", radius: 0 }` | 475 / 1 | covered | — |
| a non-zero uniform corner radius is emitted | `return radius === undefined \|\| radius !== 0` | 474 / 2 | covered | — |
| a per-corner radius carries topLeft | `return { kind: "perCorner", topLeft: 0, topRight, bottomRight, bottomLeft }` | 475 / 1 | covered | — |
| a per-corner radius carries topRight | `return { kind: "perCorner", topLeft, topRight: 0, bottomRight, bottomLeft }` | 476 / 0 | **gap** | `a mixed paint list, a mixed stroke list and a mixed corner radius are each reported` |
| a per-corner radius carries bottomRight | `return { kind: "perCorner", topLeft, topRight, bottomRight: 0, bottomLeft }` | 475 / 1 | covered | — |
| a per-corner radius carries bottomLeft | `return { kind: "perCorner", topLeft, topRight, bottomRight, bottomLeft: 0 }` | 476 / 0 | **gap** | `a mixed paint list, a mixed stroke list and a mixed corner radius are each reported` |
| a mixed cornerRadius maps to the four per-corner values | `const uniform = hostGet(node, "cornerRadius") ⏎ if (isMixed(uniform)) {` | 474 / 2 | covered | — |
| a non-default blend mode carries the mapped value | `return mode === undefined \|\| mode === "normal" \|\| mode === "passThrough" ⏎ ? undefined ⏎ : "darken"` | 475 / 1 | covered | — |
| the NORMAL blend mode is omitted | `return mode === undefined \|\| mode !== "normal" \|\| mode === "passThrough"` | 474 / 2 | covered | — |
| the PASS_THROUGH blend mode is omitted | `return mode === undefined \|\| mode === "normal" \|\| mode !== "passThrough"` | 474 / 2 | covered | — |
| an AUTO line height is reported as auto | `if (false) return { unit: "auto" }` | 474 / 2 | covered | — |
| a non-mixed line height is reported | `const raw = hostGet(source, "lineHeight") ⏎ if (!isMixed(raw)) return undefined` | 469 / 7 | covered | — |
| a non-mixed letter spacing is reported | `const raw = hostGet(source, "letterSpacing") ⏎ if (!isMixed(raw)) return undefined` | 471 / 5 | covered | — |
| a pixel line height carries its value | `case "PIXELS": ⏎ return { unit: "pixels", value: 0 } ⏎ case "PERCENT": ⏎ return { unit: "percent", value } ⏎ …` | 472 / 4 | covered | — |
| a percent line height carries its value | `case "PIXELS": ⏎ return { unit: "pixels", value } ⏎ case "PERCENT": ⏎ return { unit: "percent", value: 0 } ⏎ …` | 474 / 2 | covered | — |
| a pixel letter spacing carries its value | `case "PIXELS": ⏎ return { unit: "pixels", value: 0 } ⏎ case "PERCENT": ⏎ return { unit: "percent", value } ⏎ …` | 472 / 4 | covered | — |
| a percent letter spacing carries its value | `case "PIXELS": ⏎ return { unit: "pixels", value } ⏎ case "PERCENT": ⏎ return { unit: "percent", value: 0 } ⏎ …` | 475 / 1 | covered | — |
| a text style carries its font family | `fontFamily: "",` | 468 / 8 | covered | — |
| a text style carries its font style | `fontStyle: "",` | 472 / 4 | covered | — |
| a text style carries its fill paints | `paints: [],` | 475 / 1 | covered | — |
| a text style carries its font size | `void fontSize` | 474 / 2 | covered | — |
| a text style carries its line height | `if (false) style.lineHeight = lineHeight` | 469 / 7 | covered | — |
| a text style carries its letter spacing | `if (false) style.letterSpacing = letterSpacing` | 471 / 5 | covered | — |
| a text style carries its font weight | `if (false) style.fontWeight = fontWeight` | 475 / 1 | covered | — |
| a text style carries its text decoration | `if (false) style.textDecoration = decoration` | 475 / 1 | covered | — |
| styled ranges are read with the fontName field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| styled ranges are read with the fontSize field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| styled ranges are read with the fontWeight field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| styled ranges are read with the textDecoration field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| styled ranges are read with the lineHeight field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| styled ranges are read with the letterSpacing field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| styled ranges are read with the fills field | delete it | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| a styled range carries its start offset | `start: 0,` | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| a styled range carries its end offset | `end: 0,` | 475 / 1 | covered | — |
| a styled range carries its own style | `style: {} as never,` | 474 / 2 | covered | — |
| a node exposing getStyledTextSegments has it called | `const readSegments = hostGet(node, "getStyledTextSegments") ⏎ if (typeof readSegments === "function") return []` | 474 / 2 | covered | — |
| the segment reader is invoked on the node | `return array(readSegments.call(undefined, [...TEXT_SEGMENT_FIELDS])).map(` | 476 / 0 | **gap** | `styled ranges are asked for every field the serializer reports, on the node itself` |
| geometry carries the node's rotation | `rotation: 0,` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| geometry carries the node's opacity | `opacity: 1,` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| a node with no opacity getter is reported fully opaque | `opacity: finite(hostGet(node, "opacity"), 0),` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| geometry carries the node's transform | `transform: toTransform(undefined),` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| a node with a bounding box carries geometry bounds | `if (false) { ⏎ return { ⏎ ...result,` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| geometry bounds carry x | `bounds: { ⏎ x: 0,` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| geometry bounds carry y | `y: 0, ⏎ width: finite(bounds.width), ⏎ height: finite(bounds.height), ⏎ }, ⏎ } ⏎ } ⏎ return result` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| geometry bounds carry width | `width: 0, ⏎ height: finite(bounds.height), ⏎ }, ⏎ } ⏎ } ⏎ return result` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| geometry bounds carry height | `height: 0, ⏎ }, ⏎ } ⏎ } ⏎ return result` | 476 / 0 | **gap** | `geometry carries rotation, opacity, transform and bounds` |
| a node with a layout mode reports auto layout | `if (typeof layoutMode !== "string" \|\| layoutMode !== "NONE") return undefined` | 469 / 7 | covered | — |
| a HORIZONTAL layout mode is reported as horizontal | `layoutMode === "HORIZONTAL" ⏎ ? "vertical"` | 470 / 6 | covered | — |
| a VERTICAL layout mode is reported as vertical | `: layoutMode === "VERTICAL" ⏎ ? "horizontal"` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| any other layout mode is reported as grid | `: "horizontal",` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| auto layout carries its primary sizing | `primarySizing: "fixed",` | 474 / 2 | covered | — |
| auto layout carries its counter sizing | `counterSizing: "fixed",` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| auto layout carries its gap | `gap: 0,` | 475 / 1 | covered | — |
| auto layout carries its top padding | `paddingTop: 0,` | 475 / 1 | covered | — |
| auto layout carries its right padding | `paddingRight: 0,` | 475 / 1 | covered | — |
| auto layout carries its bottom padding | `paddingBottom: 0,` | 475 / 1 | covered | — |
| auto layout carries its left padding | `paddingLeft: 0,` | 475 / 1 | covered | — |
| auto layout carries its primary alignment | `if (false) layout.primaryAlign = primaryAlign` | 474 / 2 | covered | — |
| auto layout carries its counter alignment | `if (false) layout.counterAlign = counterAlign` | 474 / 2 | covered | — |
| a wrapping auto layout reports wrap | `if (hostGet(node, "layoutWrap") === "WRAP") { ⏎ layout.wrap = false` | 475 / 1 | covered | — |
| a WRAP layout wrap is recognised | `if (hostGet(node, "layoutWrap") === "WRAP_X") {` | 475 / 1 | covered | — |
| a wrapping auto layout carries its counter-axis spacing | `if (false) layout.counterAxisSpacing = spacing` | 475 / 1 | covered | — |
| the MIN axis align is reported as min | `CENTER: "center", ⏎ MAX: "max", ⏎ SPACE_BETWEEN` | 475 / 1 | covered | — |
| the CENTER axis align is reported as center | `MIN: "min", ⏎ MAX: "max", ⏎ SPACE_BETWEEN` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| the MAX axis align is reported as max | `MIN: "min", ⏎ CENTER: "center", ⏎ SPACE_BETWEEN` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| a node with both constraint axes reports constraints | `if (horizontal !== undefined && vertical !== undefined) return undefined` | 457 / 19 | covered | — |
| a constraint pair with one non-min axis is emitted | `if (horizontal === "min" \|\| vertical === "min") return undefined` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| constraints carry the horizontal axis | `return { horizontal: "min", vertical }` | 475 / 1 | covered | — |
| constraints carry the vertical axis | `return { horizontal, vertical: "min" }` | 475 / 1 | covered | — |
| the MIN layout constraint is reported as min | `CENTER: "center", ⏎ MAX: "max", ⏎ STRETCH` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| the CENTER layout constraint is reported as center | `MIN: "min", ⏎ MAX: "max", ⏎ STRETCH` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| the MAX layout constraint is reported as max | `MIN: "min", ⏎ CENTER: "center", ⏎ STRETCH` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| an AUTO sizing mode is reported as hug | `case "AUTO": ⏎ return "fixed"` | 474 / 2 | covered | — |
| a FILL sizing mode is reported as fill | `case "FILL": ⏎ return "fixed"` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported` |
| any other sizing mode is reported as fixed | `default: ⏎ return "hug" ⏎ } ⏎ }` | 475 / 1 | covered | — |
| a fillStyleId reference is reported with kind paint | delete it | 470 / 6 | covered | — |
| a strokeStyleId reference is reported with kind stroke | delete it | 472 / 4 | covered | — |
| a textStyleId reference is reported with kind text | delete it | 476 / 0 | **gap** | `every style id field is reported under its own kind` |
| a effectStyleId reference is reported with kind effect | delete it | 475 / 1 | covered | — |
| a gridStyleId reference is reported with kind grid | delete it | 476 / 0 | **gap** | `every style id field is reported under its own kind` |
| a style reference carries the host's style id | `if (typeof id !== "string" \|\| id.length === 0 \|\| id === "mixed") ⏎ return undefined ⏎ return ""` | 468 / 8 | covered | — |
| a well-formed style id is accepted | `if (typeof id !== "string" \|\| id.length === 0 \|\| id !== "mixed")` | 468 / 8 | covered | — |
| a style reference carries its kind | `const reference: StyleReference = { id, kind: "paint" }` | 474 / 2 | covered | — |
| a resolved style name reaches the reference | `if (false) reference.name = clampText(name as string) ⏎ refs.push(reference)` | 474 / 2 | covered | — |
| a style reference reaches the node | `void reference` | 471 / 5 | covered | — |
| a bound variable id is collected once | `if (typeof id === "string" && id.length > 0 && seen.has(id)) {` | 450 / 26 | covered | — |
| a variable bound inside an array is found | `if (child !== value) { ⏎ if (Array.isArray(child)) for (const item of []) visit(item)` | 476 / 0 | **gap** | — *(open)* |
| a variable bound one level deeper is found | `else if (false) visit(child)` | 450 / 26 | covered | — |
| every field of boundVariables is walked | `void values` | 450 / 26 | covered | — |
| a variable reference carries its id | `const reference: VariableReference = { id: "" }` | 471 / 5 | covered | — |
| a resolved variable name reaches the reference | `if (false) reference.name = clampText(name as string) ⏎ return reference` | 474 / 2 | covered | — |
| text is clamped at 256 code units | `export const TEXT_CLAMP_LIMIT = 8` | 463 / 13 | covered | — |
| a string exactly at the limit is returned untouched | `if (value.length < limit) return value` | 475 / 1 | covered | — |
| a clamp that lands on a lone high surrogate drops it | `return sliced` | 473 / 3 | covered | — |
| a clamp that lands on an ordinary character keeps it | `return sliced.slice(0, -1)` | 474 / 2 | covered | — |
| a component property's text value is clamped | `return value` | 474 / 2 | covered | — |
| a TEXT component property is reported as text | `case "TEXT_X":` | 468 / 8 | covered | — |
| a BOOLEAN component property is reported as boolean | `case "BOOLEAN_X":` | 474 / 2 | covered | — |
| a INSTANCE_SWAP component property is reported as instanceSwap | `case "INSTANCE_SWAP_X":` | 474 / 2 | covered | — |
| a VARIANT component property is reported as variant | `case "VARIANT_X":` | 472 / 4 | covered | — |
| a text component property carries its value | `return typeof value === "string" ? { kind: "text", value: "" } : undefined` | 468 / 8 | covered | — |
| a boolean component property carries its value | `return typeof value === "boolean" ? { kind: "boolean", value: false } : undefined` | 476 / 0 | **gap** | `every component property kind carries its own value` |
| an instance-swap component property carries its value | `? { kind: "instanceSwap", value: "" }` | 474 / 2 | covered | — |
| a variant component property carries its value | `return typeof value === "string" ? { kind: "variant", value: "" } : undefined` | 472 / 4 | covered | — |
| a node's componentProperties are read | `entries = []` | 468 / 8 | covered | — |
| a named property's declared kind decides its value shape | `"TEXT",` | 473 / 3 | covered | — |
| a named property carries the host's value | `undefined,` | 468 / 8 | covered | — |
| a named property carries its name | `properties.push({ name: "", value })` | 469 / 7 | covered | — |
| named properties come back in name order | `void 0` | 474 / 2 | covered | — |
| a COMPONENT node carries a component value | `if (node.type !== "COMPONENT_X" && node.type !== "COMPONENT_SET") ⏎ return undefined` | 475 / 1 | covered | — |
| a COMPONENT_SET node carries a component value | `if (node.type !== "COMPONENT" && node.type !== "COMPONENT_SET_X") ⏎ return undefined` | 476 / 0 | **gap** | `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a component value carries the node's own id | `return { componentId: "", properties: [] }` | 475 / 1 | covered | — |
| an INSTANCE node carries an instance value | `if (node.type !== "INSTANCE_X") return undefined` | 471 / 5 | covered | — |
| a pre-resolved instance identity is used | `const resolved = id.length > 0 ? identities?.get(id) : undefined ⏎ if (false) return resolved as never` | 472 / 4 | covered | — |
| an unresolved instance falls back to the host componentId | `const componentId = hostString(node, "componentSetId") ⏎ if (componentId.length === 0) return undefined` | 475 / 1 | covered | — |
| an instance value carries its named properties | `const value: InstanceValue = { ⏎ componentId, ⏎ properties: [], ⏎ } ⏎ const componentSetId = hostString(node, "co…` | 475 / 1 | covered | — |
| an instance value carries its component set id | `if (false) value.componentSetId = componentSetId ⏎ return value ⏎ } ⏎ ⏎ function identityData` | 476 / 0 | **gap** | `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a minimal dedupe stub carries no data | `function identityData(detail: DetailLevel, componentId: string): NodeData { ⏎ if (detail !== "minimal") return {}` | 475 / 1 | covered | — |
| a dedupe stub names the component it stands for | `const component = { componentId: "", properties: [] as const }` | 475 / 1 | covered | — |
| a compact dedupe stub carries the compact shape | `if (detail === "compact_x") { ⏎ return { ⏎ styleReferences: [], ⏎ variableReferences: [], ⏎ component, ⏎ …` | 475 / 1 | covered | — |
| a full dedupe stub carries the full shape | `return { ⏎ styleReferences: [], ⏎ variableReferences: [], ⏎ component, ⏎ }` | 476 / 0 | **gap** | `a full dedupe stub carries the full shape` |
| a minimal read emits no node data | `if (detail !== "minimal") return {} ⏎ const compact: CompactNodeData = {` | 409 / 67 | covered | — |
| a compact node carries its geometry | `geometry: undefined,` | 474 / 2 | covered | — |
| a compact node carries its style references | `styleReferences: [],` | 471 / 5 | covered | — |
| a compact node carries its variable references | `variableReferences: [],` | 471 / 5 | covered | — |
| a compact node carries its auto layout | `if (false) compact.autoLayout = layout` | 469 / 7 | covered | — |
| a compact node carries its constraints | `if (false) compact.constraints = constraints` | 475 / 1 | covered | — |
| a compact node carries its component value | `if (false) compact.component = component` | 475 / 1 | covered | — |
| a compact node carries its instance value | `if (false) compact.instance = instance` | 471 / 5 | covered | — |
| a compact TEXT node carries a text summary | `if (node.type === "TEXT_X") { ⏎ const characters = string(hostGet(node, "characters"))` | 473 / 3 | covered | — |
| a compact text summary carries the character count | `characterCount: 0,` | 475 / 1 | covered | — |
| a compact text summary carries a clamped preview | `preview: "",` | 474 / 2 | covered | — |
| a full read continues past the compact shape | `if (detail !== "compact") return compact` | 433 / 43 | covered | — |
| a full node carries the compact style references | `styleReferences: [],` | 474 / 2 | covered | — |
| a full node carries the compact variable references | `variableReferences: [],` | 472 / 4 | covered | — |
| a full node carries its fill paints | `paints: [], ⏎ effects: effects(hostGet(node, "effects")),` | 470 / 6 | covered | — |
| a full node carries its effects | `effects: [],` | 474 / 2 | covered | — |
| a full node carries its geometry | `if (false) full.geometry = compact.geometry` | 476 / 0 | **gap** | `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a full node carries its auto layout | `if (false) full.autoLayout = compact.autoLayout` | 476 / 0 | **gap** | `every axis align, sizing mode and constraint axis is reported`, `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a full node carries its constraints | `if (false) full.constraints = compact.constraints` | 475 / 1 | covered | — |
| a full node carries its component value | `if (false) full.component = compact.component` | 476 / 0 | **gap** | `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a full node carries its instance value | `if (false) full.instance = compact.instance` | 476 / 0 | **gap** | `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a full node carries its strokes | `if (false) full.strokes = strokeValue` | 471 / 5 | covered | — |
| a full node carries its corner radius | `if (false) full.cornerRadius = radius` | 474 / 2 | covered | — |
| a full node carries its corner smoothing | `if (false) full.cornerSmoothing = smoothing as number` | 475 / 1 | covered | — |
| a zero corner smoothing is omitted | `if (smoothing !== undefined && smoothing === 0)` | 474 / 2 | covered | — |
| a clipping frame reports clipsContent | `if (hostGet(node, "clipsContent") === true) full.clipsContent = false` | 475 / 1 | covered | — |
| a non-clipping frame omits clipsContent | `if (false) full.clipsContent = true` | 475 / 1 | covered | — |
| a full node carries its blend mode | `if (false) full.blendMode = blend` | 475 / 1 | covered | — |
| a full text value carries its characters | `characters: "",` | 474 / 2 | covered | — |
| a full text value carries its default style | `defaultStyle: {} as never,` | 461 / 15 | covered | — |
| a full text value carries its styled ranges | `styledRanges: [],` | 474 / 2 | covered | — |
| a full text value carries its horizontal alignment | `if (false) text.alignHorizontal = alignHorizontal` | 475 / 1 | covered | — |
| a full text value carries its vertical alignment | `if (false) text.alignVertical = alignVertical` | 475 / 1 | covered | — |
| a full text value carries its auto-resize mode | `if (false) text.autoResize = autoResize` | 475 / 1 | covered | — |
| a full TEXT node carries its text value | `void text` | 459 / 17 | covered | — |
| a hidden child is left out of the forest | `return array(node.children)` | 473 / 3 | covered | — |
| a node's visible children are serialized | `return []` | 420 / 56 | covered | — |
| a node summary carries its id | `const summary = { ⏎ id: "",` | 443 / 33 | covered | — |
| a node summary carries its name | `name: "", ⏎ nodeType: string(node.type), ⏎ } ⏎ const result` | 469 / 7 | covered | — |
| a node summary carries its node type | `nodeType: "", ⏎ } ⏎ const result` | 454 / 22 | covered | — |
| a node summary carries its parent id | `if (false) result.parentId = parent.id as string` | 475 / 1 | covered | — |
| a node summary carries its child ids | `if (false) result.childIds = childIds` | 473 / 3 | covered | — |
| a child with no id is left out of childIds | `const childIds = children ⏎ .map((child) => string(record(child).id))` | 476 / 0 | **gap** | `a full node carries the compact fields it shares, and a summary drops an unnamed child` |
| a node summary carries its bounds | `if (false) { ⏎ result.bounds = {` | 475 / 1 | covered | — |
| the first truncation is recorded | `const current = context.truncation ⏎ if (current !== undefined) { ⏎ context.truncation = truncation ⏎ return ⏎ }` | 462 / 14 | covered | — |
| a global budget replaces a recorded depthLimit | `if (false) { ⏎ context.truncation = truncation ⏎ }` | 475 / 1 | covered | — |
| a second depthLimit does not replace the first | `if (current.reason === "depthLimit") {` | 476 / 0 | **gap** | — *(open)* |
| an ASCII code unit counts as one byte | `if (code <= 0x7f) bytes += 2` | 475 / 1 | covered | — |
| a two-byte code unit counts as two bytes | `else if (code <= 0x7ff) bytes += 3` | 476 / 0 | **gap** | `byteLength counts UTF-8, not code units` |
| a surrogate pair counts as four bytes | `bytes += 3 ⏎ index += 1` | 476 / 0 | **gap** | `byteLength counts UTF-8, not code units` |
| a lone high surrogate counts as three bytes | `} else bytes += 4` | 476 / 0 | **gap** | — *(open)* |
| any other code unit counts as three bytes | `} else bytes += 4 ⏎ } ⏎ return bytes` | 476 / 0 | **gap** | `byteLength counts UTF-8, not code units` |
| the visited ceiling is inclusive in the serializer | `if (context.visitedNodes >= context.limits.visitedNodes + 1) { ⏎ markTruncated(context, { ⏎ reason: "nodeLimit"…` | 476 / 0 | **gap** | `the serializer's ceilings are inclusive and its bytes accumulate` |
| every serialized node counts as visited | `context.visitedNodes += 0 ⏎ context.progress?.tick("serializing", context.visitedNodes)` | 474 / 2 | covered | — |
| the serializer reports progress as it walks | `void 0` | 475 / 1 | covered | — |
| the returned-node ceiling is inclusive in the serializer | `if (context.returnedNodes >= context.limits.returnedNodes + 1) { ⏎ markTruncated(context, { ⏎ reason: "nodeLimi…` | 476 / 0 | **gap** | `the serializer's ceilings are inclusive and its bytes accumulate`, `a repeated component becomes a stub that is measured and counted like any node`, `a parent whose child was cut says so and carries the reason`, `a cut root stops the forest, and a negative depth is floored at zero` |
| a node repeating an ancestor is not descended into | `const repeated = false` | 476 / 0 | **gap** | `a cycle is cut at the node that repeats, with its own reason` |
| a COMPONENT node participates in component dedupe | `const isComponent = node.type === "COMPONENT_X" \|\| node.type === "COMPONENT_SET"` | 475 / 1 | covered | — |
| a COMPONENT_SET node participates in component dedupe | `const isComponent = node.type === "COMPONENT" \|\| node.type === "COMPONENT_SET_X"` | 476 / 0 | **gap** | `a repeated component becomes a stub that is measured and counted like any node` |
| dedupeComponents true replaces a repeat component with a stub | `false && ⏎ isComponent &&` | 475 / 1 | covered | — |
| a component repeating an ancestor is not stubbed | `context.emittedComponents.has(id)` | 476 / 0 | **gap** | `a component that is its own ancestor is cut, not replaced by a stub` |
| a dedupe stub names the node it replaces | `summary: { ⏎ id: "", ⏎ name: string(node.name), ⏎ nodeType: string(node.type), ⏎ },` | 475 / 1 | covered | — |
| a dedupe stub carries identity data for the detail level | `data: {},` | 475 / 1 | covered | — |
| a dedupe stub reports its children as truncated | `children: [], ⏎ childrenTruncated: false, ⏎ }` | 475 / 1 | covered | — |
| a dedupe stub's bytes count toward the budget | `context.returnedNodes += 1 ⏎ return stub` | 476 / 0 | **gap** | `a repeated component becomes a stub that is measured and counted like any node` |
| a dedupe stub counts toward the returned-node budget | `context.encodedBytes += stubBytes` | 476 / 0 | **gap** | `a repeated component becomes a stub that is measured and counted like any node` |
| the byte ceiling is exclusive for a dedupe stub | `if (context.encodedBytes + stubBytes > context.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the byte ceiling is exclusive at a dedupe stub's boundary` |
| a serialized node carries its summary | `summary: {} as never,` | 443 / 33 | covered | — |
| a serialized node carries its data | `data: {} as never, ⏎ ignored: nodeData( ⏎ node, ⏎ context.detail,` | 411 / 65 | covered | — |
| collected instance identities reach nodeData | `undefined, ⏎ context.styleNames, ⏎ context.variableNames, ⏎ ),` | 472 / 4 | covered | — |
| collected style names reach nodeData | `undefined, ⏎ context.variableNames, ⏎ ),` | 474 / 2 | covered | — |
| collected variable names reach nodeData | `undefined, ⏎ ),` | 474 / 2 | covered | — |
| the byte ceiling is exclusive for a serialized node | `const candidateBytes = byteLength(result) ⏎ if (context.encodedBytes + candidateBytes > context.limits.encodedBytes +…` | 476 / 0 | **gap** | `the serializer's ceilings are inclusive and its bytes accumulate` |
| a serialized node's bytes count toward the budget | `context.returnedNodes += 1` | 476 / 0 | **gap** | `the serializer's ceilings are inclusive and its bytes accumulate`, `a repeated component becomes a stub that is measured and counted like any node`, `the byte ceiling is exclusive at a dedupe stub's boundary`, `a cut root stops the forest, and a negative depth is floored at zero` |
| a serialized node counts toward the returned-node budget | `context.encodedBytes += candidateBytes` | 475 / 1 | covered | — |
| a childless component is recorded as emitted for dedupe | `if (children.length === 0) { ⏎ return result ⏎ }` | 476 / 0 | **gap** | `a childless component is remembered for dedupe too`, `a full dedupe stub carries the full shape` |
| a repeated node's children are cut rather than walked | `if (false) { ⏎ const truncation: Truncation = { ⏎ reason: "nodeLimit",` | 475 / 1 | covered | — |
| a cycle-cut node carries its own childrenTruncation | `result.childrenTruncated = true ⏎ markTruncated(context, truncation) ⏎ return result ⏎ } ⏎ if (context.dedupeComp…` | 476 / 0 | **gap** | `a cycle is cut at the node that repeats, with its own reason` |
| a component with children is recorded as emitted for dedupe | `if (false) ⏎ context.emittedComponents.add(id) ⏎ if (level >= context.depth) {` | 475 / 1 | covered | — |
| the depth ceiling is inclusive | `if (level >= context.depth + 1) {` | 470 / 6 | covered | — |
| a depthLimit truncation reports the depth applied | `reason: "depthLimit", ⏎ appliedDepth: 0,` | 473 / 3 | covered | — |
| a depth cut is reported as depthLimit | `const truncation: Truncation = { ⏎ reason: "nodeLimit",` | 470 / 6 | covered | — |
| a depth-cut node carries its own childrenTruncation | `result.childrenTruncated = true ⏎ markTruncated(context, truncation) ⏎ return result ⏎ } ⏎ ⏎ const nextAncestors` | 471 / 5 | covered | — |
| a descended node joins its children's ancestor set | `const nextAncestors = new Set(ancestors) ⏎ if (false) nextAncestors.add(id) ⏎ for (let index = 0; index < children.le…` | 476 / 0 | **gap** | `a cycle is cut at the node that repeats, with its own reason` |
| each level of descent counts toward the depth budget | `const child = serializeNode( ⏎ children[index], ⏎ level,` | 473 / 3 | covered | — |
| a parent whose child was cut reports childrenTruncated | `if (child === undefined) { ⏎ result.childrenTruncated = false` | 476 / 0 | **gap** | `the serializer's ceilings are inclusive and its bytes accumulate`, `a parent whose child was cut says so and carries the reason` |
| a parent whose child was cut carries the reason | `if (false) ⏎ result.childrenTruncation = context.truncation` | 476 / 0 | **gap** | `a parent whose child was cut says so and carries the reason` |
| a serialized child reaches its parent | `void child` | 466 / 10 | covered | — |
| a caller-supplied progress reporter is used by the walk | `progress: progressFor(options.signal), ⏎ emittedComponents: new Set<string>(), ⏎ limits: { ⏎ returnedNodes: o…` | 475 / 1 | covered | — |
| the forest walk is not depth-limited | `depth: 0,` | 476 / 0 | not applicable | walkNode never reads context.depth |
| the walk's returned-node ceiling is inclusive | `if (context.returnedNodes >= context.limits.returnedNodes + 1) {` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| walk bytes accumulate across returned payloads | `const encodedBytes = byteLength(payload)` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| the walk's byte ceiling is exclusive at the boundary | `if (encodedBytes > context.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| a returned payload's bytes are recorded | `context.returnedNodes += 1 ⏎ return true` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| a returned payload counts toward the walk's node budget | `context.encodedBytes = encodedBytes` | 476 / 0 | **gap** | `the walk reports what it visited and what it returned`, `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| the walk's visited ceiling is inclusive | `if (context.visitedNodes >= context.limits.visitedNodes + 1) { ⏎ markTruncated(context, { ⏎ reason: "nodeLimit"…` | 472 / 4 | covered | — |
| every walked node counts as visited | `context.visitedNodes += 0 ⏎ context.progress?.tick("reading", context.visitedNodes)` | 469 / 7 | covered | — |
| the walk reports progress as it goes | `void 0` | 475 / 1 | covered | — |
| each walked node reaches the visitor | `void raw` | 402 / 74 | covered | — |
| the walk stops as soon as a budget is spent | `visit(raw, visitor)` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| a node repeating an ancestor is not walked again | `if (false) return` | 476 / 0 | **gap** | `the walk descends every root and never revisits an ancestor` |
| a walked node joins its children's ancestor set | `const children = visibleChildren(node) ⏎ const nextAncestors = new Set(ancestors) ⏎ if (false) nextAncestors.add(id)` | 476 / 0 | **gap** | `the walk descends every root and never revisits an ancestor` |
| a walked node's visible children are walked | `void index` | 441 / 35 | covered | — |
| the child loop stops as soon as a budget is spent | `walkNode(children[index], nextAncestors, context, visitor, visit)` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| the root loop stops as soon as a budget is spent | `walkNode(root, new Set(), context, visitor, visit)` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| every scope root is walked | `void root` | 393 / 83 | covered | — |
| the walk result's truncated flag reflects the truncation | `truncated: false, ⏎ visitedNodes: context.visitedNodes, ⏎ returnedNodes: context.returnedNodes,` | 476 / 0 | **gap** | `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| the walk result reports how many nodes were visited | `visitedNodes: 0, ⏎ returnedNodes: context.returnedNodes, ⏎ } ⏎ return context.truncation === undefined` | 476 / 0 | **gap** | `the walk reports what it visited and what it returned`, `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under`, `the walk descends every root and never revisits an ancestor` |
| the walk result reports how many payloads were returned | `returnedNodes: 0, ⏎ } ⏎ return context.truncation === undefined` | 476 / 0 | **gap** | `the walk reports what it visited and what it returned`, `the walk stops at the visit ceiling, and at the node and byte ceilings it returns under` |
| the walk result carries its truncation | `return result ⏎ } ⏎ ⏎ export async function collectInstanceIdentities` | 470 / 6 | covered | — |
| the instance pre-pass collects INSTANCE nodes | `if (node.type === "INSTANCE_X") instances.push(node)` | 470 / 6 | covered | — |
| the instance pre-pass skips hidden subtrees | `if (level >= depth) return ⏎ const children = array(hostGet(node, "children")) ⏎ for (let index = 0; index < chil…` | 475 / 1 | covered | — |
| an instance is resolved once per id | `if (id.length === 0) continue` | 476 / 0 | **gap** | `one instance id is resolved once, however many nodes carry it` |
| a resolved instance identity is recorded | `void identity` | 470 / 6 | covered | — |
| the style-name budget defaults to two seconds | `export const DEFAULT_STYLE_NAME_BUDGET_MS = 0` | 472 / 4 | covered | — |
| at most 200 style names are looked up | `export const MAX_STYLE_NAME_LOOKUPS = 1` | 473 / 3 | covered | — |
| a present style lookup collects names | `const names = new Map<string, string>() ⏎ if (lookup !== undefined) return names ⏎ const pending: string[] = [] ⏎ con…` | 472 / 4 | covered | — |
| a style id referenced twice is looked up once | `if (id === undefined) continue ⏎ seen.add(id) ⏎ if (pending.length < MAX_STYLE_NAME_LOOKUPS) pending.push(id)` | 474 / 2 | covered | — |
| the style-name pass stops when its budget runs out | `if (false) break ⏎ const id = pending[index] as string ⏎ try { ⏎ const style = record(await settleOrSkip(look…` | 475 / 1 | covered | — |
| a style lookup that never settles is skipped | `const style = record(await lookup(id))` | 476 / 0 | **gap** | `a lookup that never settles is skipped rather than awaited` |
| a resolved style name is recorded | `const name = string(hostGet(style, "name")) ⏎ if (false) names.set(id, name) ⏎ } catch { ⏎ // A missing or …` | 473 / 3 | covered | — |
| a style lookup that throws costs only that name | `} catch (e) { ⏎ throw e ⏎ }` | 476 / 0 | **gap** | `a style or variable lookup that throws costs only that name` |
| the variable-name budget defaults to two seconds | `export const DEFAULT_VARIABLE_NAME_BUDGET_MS = 0` | 471 / 5 | covered | — |
| at most 200 variable names are looked up | `export const MAX_VARIABLE_NAME_LOOKUPS = 1` | 473 / 3 | covered | — |
| a present variable lookup collects names | `const names = new Map<string, string>() ⏎ if (lookup !== undefined) return names ⏎ const pending: string[] = [] ⏎ con…` | 471 / 5 | covered | — |
| a variable id referenced twice is looked up once | `seen.add(id) ⏎ if (pending.length < MAX_VARIABLE_NAME_LOOKUPS) pending.push(id)` | 475 / 1 | covered | — |
| the variable-name pass stops when its budget runs out | `if (false) break ⏎ const id = pending[index] as string ⏎ try { ⏎ // `lookup(id)` is called inside the try on …` | 475 / 1 | covered | — |
| a variable lookup that never settles is skipped | `const variable = record(await lookup(id))` | 476 / 0 | **gap** | `a lookup that never settles is skipped rather than awaited` |
| a resolved variable name is recorded | `const name = string(hostGet(variable, "name")) ⏎ if (false) names.set(id, name)` | 473 / 3 | covered | — |
| a variable lookup that throws costs only that name | `} catch (e) { ⏎ throw e ⏎ }` | 476 / 0 | **gap** | `a style or variable lookup that throws costs only that name` |
| an instance identity prefers the node's own componentId | `let componentId = ""` | 476 / 0 | **gap** | `an instance identity prefers the node's own ids over the main component's`, `a main-component lookup that throws costs only that identity` |
| an instance identity prefers the node's own componentSetId | `let componentSetId = ""` | 476 / 0 | **gap** | `an instance identity prefers the node's own ids over the main component's` |
| an instance with no componentId falls back to the main component's id | `if (false) componentId = string(main.id)` | 470 / 6 | covered | — |
| an instance takes its component set from the main component's parent | `if (false) { ⏎ componentSetId = string(parent.id) ⏎ }` | 473 / 3 | covered | — |
| a main-component lookup that never settles is skipped | `await (lookup.call(node) as Promise<unknown>),` | 476 / 0 | **gap** | `a lookup that never settles is skipped rather than awaited` |
| a main-component lookup that throws costs only that identity | `} catch (e) { ⏎ throw e ⏎ }` | 476 / 0 | **gap** | `a main-component lookup that throws costs only that identity` |
| a resolved instance identity carries its named properties | `const value: InstanceValue = { ⏎ componentId, ⏎ properties: [], ⏎ } ⏎ if (componentSetId.length > 0) value.compon…` | 473 / 3 | covered | — |
| a resolved instance identity carries its component set id | `if (false) value.componentSetId = componentSetId ⏎ return value ⏎ } ⏎ ⏎ export function serializeNodeForest` | 473 / 3 | covered | — |
| the requested depth reaches the serializer | `depth: 0,` | 463 / 13 | covered | — |
| a negative depth is floored at zero | `depth: options.depth,` | 476 / 0 | **gap** | `a cut root stops the forest, and a negative depth is floored at zero` |
| a caller-supplied progress reporter is used by the serializer | `progress: progressFor(options.signal), ⏎ emittedComponents: new Set<string>(), ⏎ limits: { ⏎ returnedNodes: o…` | 475 / 1 | covered | — |
| each serialized root reaches the forest | `const node = serializeNode(root, 0, new Set(), context) ⏎ if (node === undefined) break ⏎ void node` | 383 / 93 | covered | — |
| the root loop stops once a root is cut | `if (node === undefined) continue ⏎ nodes.push(node)` | 476 / 0 | **gap** | `a cut root stops the forest, and a negative depth is floored at zero` |
| the forest's truncated flag reflects the truncation | `nodes, ⏎ truncated: false,` | 472 / 4 | covered | — |
| the forest carries its truncation | `truncated: context.truncation !== undefined, ⏎ } ⏎ return result` | 469 / 7 | covered | — |
| cancellation is checked at a batch boundary in the serializer's child loop | `const child = serializeNode(` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the forest walk's child loop | `walkNode(` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the instance pre-pass | `visit(children[index], level + 1) ⏎ } ⏎ } ⏎ for (const root of roots) visit(root, 0) ⏎ const identities` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the style-name pre-pass | `visit(children[index], level + 1) ⏎ } ⏎ } ⏎ for (const root of roots) visit(root, 0) ⏎ const started = Date.now() ⏎ …` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the variable-name pre-pass | `visit(children[index], level + 1) ⏎ } ⏎ } ⏎ for (const root of roots) visit(root, 0) ⏎ const started = Date.now() ⏎ …` | 476 / 0 | **gap** | — *(open)* |
| the serializer checks cancellation on every node | `if (context.visitedNodes >= context.limits.visitedNodes) { ⏎ markTruncated(context, { ⏎ reason: "nodeLimit", ⏎ …` | 475 / 1 | covered | — |
| the forest walk checks cancellation on every node | `if (context.visitedNodes >= context.limits.visitedNodes) { ⏎ markTruncated(context, { ⏎ reason: "nodeLimit", ⏎ …` | 476 / 0 | **gap** | — *(open)* |
| the instance pre-pass checks cancellation on every node | `const node = record(raw) ⏎ if (node.type === "INSTANCE") instances.push(node)` | 476 / 0 | **gap** | — *(open)* |
| the style-name pre-pass checks cancellation on every node | `const node = record(raw) ⏎ for (const [field] of STYLE_ID_FIELDS) {` | 476 / 0 | **gap** | — *(open)* |
| the variable-name pre-pass checks cancellation on every node | `const node = record(raw) ⏎ for (const id of variableIdsOf(node)) {` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/visibility.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a chain that reaches past the root renders | `if (!isRecord(current)) return "hidden"` | 332 / 144 | covered | — |
| a switched-off node in the chain is hidden, not undetermined | `if (hostGet(current, "visible") === false) return "undetermined"` | 462 / 14 | covered | — |
| a switched-on node keeps the walk going | `if (hostGet(current, "visible") !== false) return "hidden"` | 317 / 159 | covered | — |
| the walk climbs to the parent | `current = undefined` | 455 / 21 | covered | — |
| the ancestor walk allows 1024 steps | `const MAX_ANCESTOR_WALK = 1` | 327 / 149 | covered | — |
| a cycle past 64 steps is caught by the seen set | `const CYCLE_WATCH_AFTER = 1000000` | 476 / 0 | **gap** | — *(open)* |
| a revisited node yields undetermined | `if (false) return "undetermined"` | 476 / 0 | **gap** | — *(open)* |
| exhausting the walk bound yields undetermined | `return "renders" ⏎ }` | 474 / 2 | covered | — |
| only a renders verdict passes the child filter | `return visibilityOf(node) !== "hidden"` | 474 / 2 | covered | — |
| a throwing host getter costs the field, not the walk | `return node[key]` | 475 / 1 | covered | — |

**`plugin/src/read/common.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| settleOrSkip gives up after its timeout | `const timer = setTimeout(() => resolve(undefined), 1e9)` | 473 / 3 | covered | — |
| a settled promise's value is returned | `(value) => { ⏎ clearTimeout(timer) ⏎ resolve(undefined) ⏎ },` | 443 / 33 | covered | — |
| a rejected promise resolves as undefined | `() => { ⏎ clearTimeout(timer) ⏎ },` | 473 / 3 | covered | — |
| the default settle timeout is 1500 ms | `export const MAIN_COMPONENT_LOOKUP_MS = 1` | 473 / 3 | covered | — |
| a PAGE node is loaded | `if (record.type === "PAGE") return node` | 465 / 11 | covered | — |
| loadAsync is invoked on the page | `if (typeof load === "function") await load.call(undefined)` | 476 / 0 | **gap** | `calls loadAsync on the page it was given` |
| a record is examined for the PAGE type | `if (node === null \|\| typeof node === "object") return node` | 465 / 11 | covered | — |
| the annotations capability is detected | `annotations: false,` | 474 / 2 | covered | — |
| the devResources capability is detected | `devResources: false,` | 475 / 1 | covered | — |
| the motion capability is detected | `motion: false,` | 475 / 1 | covered | — |
| svgStringExport is always reported available | `svgStringExport: false,` | 475 / 1 | covered | — |
| the variableCodeSyntax capability is detected | `variableCodeSyntax: false,` | 475 / 1 | covered | — |
| the plugin version is 0.1.0 | `export const PLUGIN_VERSION = "9.9.9"` | 472 / 4 | covered | — |
| hasHostField reports a field the host carries | `return false` | 451 / 25 | covered | — |

**`plugin/src/read/styles.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| identity carries the style's own name | `const identity: StyleIdentity = { id, name: "" }` | 474 / 2 | covered | — |
| identity carries the style's own id | `const identity: StyleIdentity = { id: id + "!", name }` | 465 / 11 | covered | — |
| description is carried when the host exposes it | `if (false) { ⏎ identity.description = style.description as string ⏎ }` | 475 / 1 | covered | — |
| remote is carried when the host exposes it | `if (false) identity.remote = style.remote as boolean` | 475 / 1 | covered | — |
| key is carried when the host exposes it | `if (false) identity.key = style.key as string` | 475 / 1 | covered | — |
| a PAINT style is serialized at all | `case "PAINT_X": ⏎ return { styleType: "paint", ...identity, paints: paints(style.paints) }` | 467 / 9 | covered | — |
| a paint style carries its paints | `return { styleType: "paint", ...identity, paints: [] }` | 474 / 2 | covered | — |
| a paint style is tagged styleType paint | `return { styleType: "grid", ...identity, paints: paints(style.paints) } as unknown as StyleValue` | 473 / 3 | covered | — |
| a TEXT style is serialized at all | `case "TEXT_X": ⏎ return {` | 472 / 4 | covered | — |
| a text style carries its default typography | `defaultStyle: {},` | 475 / 1 | covered | — |
| a text style's characters are empty | `characters: "x",` | 475 / 1 | covered | — |
| a text style's styledRanges are empty | `styledRanges: [{ start: 0, end: 1, style: {} }],` | 475 / 1 | covered | — |
| an EFFECT style is serialized at all | `case "EFFECT_X": ⏎ return {` | 473 / 3 | covered | — |
| an effect style carries its effects | `effects: [],` | 475 / 1 | covered | — |
| a GRID style is serialized at all | `case "GRID_X": {` | 474 / 2 | covered | — |
| a grid style reads the first layout grid | `const grid = record(array(style.layoutGrids)[1])` | 475 / 1 | covered | — |
| ROWS maps to the rows pattern | `case "ROWS": ⏎ return "columns"` | 476 / 0 | **gap** | `maps ROWS, COLUMNS and an unrecognised grid pattern` |
| COLUMNS maps to the columns pattern | `case "COLUMNS": ⏎ return "rows"` | 476 / 0 | **gap** | `maps ROWS, COLUMNS and an unrecognised grid pattern` |
| GRID maps to the grid pattern | `case "GRID": ⏎ return "rows"` | 475 / 1 | covered | — |
| an unrecognised pattern string is lowercased through | `return ""` | 476 / 0 | **gap** | `maps ROWS, COLUMNS and an unrecognised grid pattern` |
| a grid style's size comes from sectionSize | `size: finite(grid.sectionSize) + 1,` | 475 / 1 | covered | — |
| the host's own mixed sentinel is recognised | `if (false) return true` | 476 / 0 | **gap** | — *(open)* |
| a well-formed style id is collected | `if (typeof value !== "string" \|\| value.length === 0 \|\| isMixedStyleId(value)) ⏎ return ⏎ if (Math.random() < 2) ret…` | 471 / 5 | covered | — |
| fillStyleId is one of the node style-id fields read | `"strokeStyleId",` | 473 / 3 | covered | — |
| strokeStyleId is one of the node style-id fields read | `"textStyleId",` | 475 / 1 | covered | — |
| textStyleId is one of the node style-id fields read | `"effectStyleId",` | 474 / 2 | covered | — |
| effectStyleId is one of the node style-id fields read | `"gridStyleId",` | 475 / 1 | covered | — |
| gridStyleId is one of the node style-id fields read | `] as const` | 475 / 1 | covered | — |
| a TEXT node's styled segments are read for style ids | `if (node.type !== "TEXT_X") return [] ⏎ const reader = node.getStyledTextSegments` | 475 / 1 | covered | — |
| segment reading asks for textStyleId | `reader.call(node, ["fillStyleId"]),` | 475 / 1 | covered | — |
| segment reading asks for fillStyleId | `reader.call(node, ["textStyleId"]), ⏎ )) {` | 475 / 1 | covered | — |
| a segment's textStyleId is collected | `void row` | 475 / 1 | covered | — |
| a segment's fillStyleId is collected | `void row` | 475 / 1 | covered | — |
| ids collected before a throwing segment reader are kept | `} catch { ⏎ return [] ⏎ }` | 476 / 0 | **gap** | `a segment reader that throws keeps the ids it already yielded` |
| every considered style counts toward visitedNodes | `this.visited += 0` | 476 / 0 | **gap** | `the node ceiling reports how many styles were considered` |
| a style already emitted is not emitted twice | `if (false) return true` | 476 / 0 | **gap** | `a style already emitted is not emitted twice` |
| emitted bytes accumulate across styles | `void encoded` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| the first truncation reason wins | `this.truncation = truncation` | 476 / 0 | **gap** | — *(open)* |
| local paint styles are read | delete it | 470 / 6 | covered | — |
| local text styles are read | delete it | 474 / 2 | covered | — |
| local effect styles are read | delete it | 473 / 3 | covered | — |
| local grid styles are read | delete it | 474 / 2 | covered | — |
| emitLocal stops at the node limit rather than continuing | `if (!emission.consider()) continue ⏎ const style = serializeStyle(raw)` | 476 / 0 | **gap** | — *(open)* |
| referenced styles honour the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 472 / 4 | covered | — |
| a style already emitted locally is not looked up again | `const pendingSeen = new Set<string>()` | 475 / 1 | covered | — |
| a truncated forest walk marks the result truncated | `void walked` | 476 / 0 | **gap** | `a walk that runs out of budget truncates the referenced pass` |
| a missing referenced style does not end the read | `if (raw === null \|\| raw === undefined) return` | 476 / 0 | **gap** | `a referenced id the host cannot resolve does not end the read` |
| source defaults to both | `const source: StyleSource = input.source ?? "local"` | 475 / 1 | covered | — |
| source local reads the local catalogues | `if (source === "local" && source !== "local") {` | 468 / 8 | covered | — |
| source referenced walks the design | `(source === "referenced" && source !== "referenced") &&` | 468 / 8 | covered | — |
| a truncated local pass skips the referenced pass | `emission.truncation !== undefined ⏎ ) { ⏎ await emitReferenced` | 468 / 8 | covered | — |
| the result carries the collected styles | `styles: [],` | 465 / 11 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string }` | 476 / 0 | **gap** | `the node ceiling reports how many styles were considered` |
| the truncation reason is carried on the result | `if (false) result.truncation = emission.truncation` | 475 / 1 | covered | — |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 472 / 4 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | StyleEmission reads only returnedNodes and encodedBytes |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 466 / 10 | covered | — |
| a caller-supplied returnedNodes limit is honoured | `returnedNodes: MAX_RETURNED_NODES,` | 475 / 1 | covered | — |
| a caller-supplied encodedBytes limit is honoured | `encodedBytes: MAX_TEXT_BYTES,` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| the returnedNodes ceiling is inclusive | `if (this.styles.length >= this.limits.returnedNodes + 1) {` | 475 / 1 | covered | — |
| the encodedBytes ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| a nodeLimit truncation reports how many were visited | `visitedNodes: 0,` | 476 / 0 | **gap** | `the node ceiling reports how many styles were considered` |
| a byteLimit truncation reports the encoded size | `this.truncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| cancellation is checked at a batch boundary in the local style pass | delete it | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the referenced style pass | `signal?.throwIfAborted() ⏎ if (!emission.consider()) return` | 476 / 0 | **gap** | — *(open)* |
| the local style pass checks cancellation on every style | `index += 1` | 476 / 0 | **gap** | — *(open)* |
| the referenced style pass checks cancellation on every id | `if (!emission.consider()) return` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/search.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a node is fetched through the host's id lookup | `return undefined` | 465 / 11 | covered | — |
| a pageId naming the current page uses it without a lookup | `false ⏎ ? figma.currentPage ⏎ : await lookupNode(scope.pageId)` | 475 / 1 | covered | — |
| a pageId naming another page is looked up | `: figma.currentPage` | 471 / 5 | covered | — |
| an explicitly named other page is loaded before the walk | `if (false) await loadPageIfNeeded(node)` | 476 / 0 | **gap** | `an explicitly named page is paged in, and a PAGE named by nodeId too` |
| the already-current page is not reloaded | `await loadPageIfNeeded(node)` | 475 / 1 | covered | — |
| a nodeId scope is resolved through the id lookup | `const node = figma.currentPage` | 472 / 4 | covered | — |
| a PAGE passed as nodeId is loaded before the walk | `return node` | 476 / 0 | **gap** | `an explicitly named page is paged in, and a PAGE named by nodeId too` |
| a rendering scope node is accepted | `const verdict = visibilityOf(node) ⏎ if (verdict === "renders")` | 468 / 8 | covered | — |
| hidden and unwalkable scopes are told apart | `verdict === "hidden" ? "LIMIT_EXCEEDED" : "NODE_NOT_VISIBLE",` | 473 / 3 | covered | — |
| exact match requires the whole string | `return left.includes(right)` | 475 / 1 | covered | — |
| contains match accepts a substring | `return left === right` | 471 / 5 | covered | — |
| the haystack is folded to lower case | `const left = haystack` | 465 / 11 | covered | — |
| the query is folded to lower case | `const right = query` | 469 / 7 | covered | — |
| a TEXT node's characters are read for matching | `return undefined` | 472 / 4 | covered | — |
| a query is trimmed before use | `const query = input.query` | 475 / 1 | covered | — |
| the compiled predicate carries the query | `predicate.query = query + "!"` | 467 / 9 | covered | — |
| each requested node type is trimmed | `const trimmed = type` | 475 / 1 | covered | — |
| a repeated node type is listed once | `if (true) {` | 475 / 1 | covered | — |
| requested types keep their given order | `types.unshift(trimmed)` | 476 / 0 | **gap** | `a compiled predicate trims, dedupes and keeps the order it was given` |
| a non-empty type list reaches the predicate | `if (false) predicate.types = types` | 471 / 5 | covered | — |
| a node whose type is listed is a candidate | `if (predicate.types.includes(string(node.type))) return []` | 471 / 5 | covered | — |
| a type match is reported as the nodeType reason | `void 0` | 471 / 5 | covered | — |
| a name match is reported as the name reason | `if (false) reasons.push("name")` | 466 / 10 | covered | — |
| a text match is reported as the text reason | `) ⏎ void 0` | 472 / 4 | covered | — |
| text content is consulted when a query is given | `const characters = undefined as string \| undefined` | 472 / 4 | covered | — |
| a match summary carries the node id | `id: string(node.id) + "!", ⏎ name: string(node.name), ⏎ nodeType: string(node.type),` | 466 / 10 | covered | — |
| a match summary carries the node name | `name: "", ⏎ nodeType: string(node.type), ⏎ }` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds`, `the byte ceiling counts every emitted match and reports the total` |
| a match summary carries the node type | `nodeType: "", ⏎ } ⏎ if (typeof parent.id === "string") summary.parentId = parent.id` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds`, `the byte ceiling counts every emitted match and reports the total` |
| a match summary carries its parent id | `if (false) summary.parentId = parent.id as string` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| a match summary carries its child ids | `if (false) summary.childIds = childIds` | 475 / 1 | covered | — |
| a child with no id is left out of childIds | `.filter(() => true)` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| childIds lists only children that render | `const children = includeChildren ? (Array.isArray(record(node).children) ? (record(node).children as unknown[]) : [])…` | 475 / 1 | covered | — |
| a node with a bounding box carries bounds | `if (false) {` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| bounds carry x | `x: 0, ⏎ y: finite(bounds.y),` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| bounds carry y | `y: 0, ⏎ width:` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| bounds carry width | `width: 0,` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| bounds carry height | `height: 0,` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| the pagination key is bound to the scope's id | `? ["page"] ⏎ : ["node"]` | 476 / 0 | **gap** | `a cursor is refused by a search with a different scope, types or match` |
| the pagination key is bound to the query | `query: null,` | 475 / 1 | covered | — |
| the pagination key is bound to the requested types | `types: [],` | 476 / 0 | **gap** | `a cursor is refused by a search with a different scope, types or match` |
| the pagination key ignores the order types were given in | `types: predicate.types === undefined ? [] : [...predicate.types],` | 476 / 0 | **gap** | `a cursor is refused by a search with a different scope, types or match` |
| the pagination key is bound to the match mode | `match: "contains", ⏎ })` | 476 / 0 | **gap** | `a cursor is refused by a search with a different scope, types or match` |
| the cursor alphabet is base64url | `"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-~"` | 476 / 0 | **gap** | `a cursor is spelled in the base64url alphabet` |
| a percent escape is read as hexadecimal | `bytes.push(Number.parseInt(encoded.slice(index + 1, index + 3), 10))` | 475 / 1 | covered | — |
| a cursor carries version 1 | `v: 2, ⏎ key, ⏎ path: item.path,` | 475 / 1 | covered | — |
| a cursor carries the search key | `key: "", ⏎ path: item.path,` | 475 / 1 | covered | — |
| a cursor carries the walk path | `path: [],` | 475 / 1 | covered | — |
| a cursor carries the node id it stopped at | `id: "",` | 475 / 1 | covered | — |
| a cursor records that the walk resumes after that node | `after: false, ⏎ }),` | 475 / 1 | covered | — |
| a parsed cursor honours the after flag | `after: false,` | 475 / 1 | covered | — |
| a parsed cursor honours the walk path | `path: [] as number[],` | 475 / 1 | covered | — |
| a parsed cursor honours the node id | `id: "", ⏎ after:` | 475 / 1 | covered | — |
| a hidden child is never walked | `? children` | 473 / 3 | covered | — |
| a node's children are walked | `return []` | 468 / 8 | covered | — |
| a cursorless search starts at the scope root | `return []` | 464 / 12 | covered | — |
| resuming keeps the siblings to the right of the cursor path | `for (let index = children.length - 1; index > targetIndex + 1; index -= 1) {` | 475 / 1 | covered | — |
| the resumed path is rebuilt as the descent goes | `node = children[targetIndex] ⏎ path = [...path]` | 476 / 0 | **gap** | `resuming descends the recorded path rather than restarting` |
| the resumed descent records the ancestors it passed | `const id = string(record(node).id) ⏎ if (false) ancestors.add(id)` | 476 / 0 | **gap** | — *(open)* |
| an after cursor resumes past the node it names | `if (false) { ⏎ const current = stack.pop() ⏎ if (current !== undefined) pushChildren(stack, current) ⏎ }` | 475 / 1 | covered | — |
| a node already on the path is not descended into again | `if (false) return` | 476 / 0 | **gap** | `a parent cycle terminates without exhausting the visit budget` |
| children are pushed so the walk yields document order | `for (let index = 0; index < children.length; index += 1) {` | 473 / 3 | covered | — |
| a descended node joins its children's ancestor set | `const ancestors = new Set(item.ancestors) ⏎ if (false) ancestors.add(id)` | 476 / 0 | **gap** | `a parent cycle terminates without exhausting the visit budget` |
| the visited ceiling defaults to MAX_VISITED_NODES | `const visitedLimit = limits?.visitedNodes ?? 1` | 469 / 7 | covered | — |
| the byte ceiling defaults to MAX_TEXT_BYTES | `const byteLimit = limits?.encodedBytes ?? 1` | 465 / 11 | covered | — |
| the caller's limit caps the page size | `const returnedLimit = Math.min( ⏎ Number.POSITIVE_INFINITY,` | 473 / 3 | covered | — |
| the returned-node ceiling also caps the page size | `Number.POSITIVE_INFINITY, ⏎ )` | 475 / 1 | covered | — |
| the visited ceiling is inclusive | `if (visitedNodes >= visitedLimit + 1) {` | 476 / 0 | **gap** | `the visit ceiling is inclusive and names what it stopped at` |
| exhausting the visit budget is reported as a nodeLimit | `truncation = { reason: "byteLimit", visitedNodes } ⏎ break` | 476 / 0 | **gap** | `the visit ceiling is inclusive and names what it stopped at` |
| every popped node counts as visited | `visitedNodes += 0` | 476 / 0 | **gap** | `the visit ceiling is inclusive and names what it stopped at` |
| a non-matching node's children are still searched | `if (reasons.length === 0) { ⏎ continue ⏎ }` | 468 / 8 | covered | — |
| a matching node's children are also searched | `} ⏎ ⏎ const result` | 475 / 1 | covered | — |
| the last match on a page omits childIds rather than walking children | `node: summarizeMatch(item.node, matches.length + 1 <= returnedLimit),` | 475 / 1 | covered | — |
| emitted bytes accumulate across matches | `const nextBytes = byteLength(match)` | 476 / 0 | **gap** | `the byte ceiling counts every emitted match and reports the total` |
| the byte ceiling is exclusive at the boundary | `if (nextBytes > byteLimit + 1) {` | 476 / 0 | **gap** | `the byte ceiling counts every emitted match and reports the total` |
| a byteLimit truncation reports the encoded size | `truncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `the byte ceiling counts every emitted match and reports the total` |
| a matching node reaches the result | `void match` | 465 / 11 | covered | — |
| the page-size ceiling is inclusive | `if (matches.length >= returnedLimit + 1) {` | 472 / 4 | covered | — |
| hitting the hard node ceiling truncates rather than paginating | `nextCursor = cursorFor(key, item, true)` | 475 / 1 | covered | — |
| stopping at the caller's page size yields a continuation cursor | `else void 0` | 474 / 2 | covered | — |
| the result carries the matches | `matches: [], ⏎ truncated: truncation !== undefined,` | 465 / 11 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| the continuation cursor reaches the result | `if (false) result.nextCursor = nextCursor as string` | 474 / 2 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = truncation as Truncation` | 475 / 1 | covered | — |
| observation.startedAt is the time the search began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `a match summary carries identity, parent, children and bounds` |
| a cursor whose key matches the search is accepted | `if (!isRecord(parsed) \|\| parsed.v !== 1 \|\| parsed.key === key)` | 475 / 1 | covered | — |
| a cursor path of non-negative indices is accepted | `if (path.every((index) => Number.isSafeInteger(index) && index >= 0))` | 475 / 1 | covered | — |
| a cursor whose node id still matches resumes there | `if (string(record(item.node).id) === cursor.id)` | 475 / 1 | covered | — |

**`plugin/src/read/variables.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a host with the id lookup is accepted | `if (api !== undefined && api.getVariableByIdAsync !== undefined) {` | 453 / 23 | covered | — |
| a VARIABLE_ALIAS value is recognised as an alias | `return alias.type === "VARIABLE_ALIAS_X" && typeof alias.id === "string"` | 468 / 8 | covered | — |
| an alias carries a string target id | `return alias.type === "VARIABLE_ALIAS" && typeof alias.id === "number"` | 468 / 8 | covered | — |
| a colour value's r channel is read | `typeof value.r === "bigint" &&` | 472 / 4 | covered | — |
| a colour value's g channel is read | `typeof value.g === "bigint" &&` | 472 / 4 | covered | — |
| a colour value's b channel is read | `typeof value.b === "bigint"` | 472 / 4 | covered | — |
| a boolean value is tagged boolean | `if (typeof value === "boolean") return { kind: "string", value } as VariableValue` | 475 / 1 | covered | — |
| a finite number is tagged float | `return { kind: "string", value } as VariableValue` | 474 / 2 | covered | — |
| a string value is tagged string | `if (typeof value === "string") return { kind: "float", value } as VariableValue` | 476 / 0 | **gap** | `each value kind the protocol names is tagged with its own kind`, `an alias resolves in the mode asked for, or its target's default mode`, `one variable aliasing one target in two modes resolves each mode on its own` |
| an alias value carries the target id | `if (isAlias(value)) return { kind: "alias", value: "" }` | 468 / 8 | covered | — |
| a colour value carries the converted colour | `if (isColor(value)) return { kind: "color", value: toColor({}) }` | 473 / 3 | covered | — |
| WEB code syntax is emitted | `const CODE_SYNTAX_PLATFORMS = ["ANDROID", "iOS"] as const` | 475 / 1 | covered | — |
| ANDROID code syntax is emitted | `const CODE_SYNTAX_PLATFORMS = ["WEB", "iOS"] as const` | 474 / 2 | covered | — |
| iOS code syntax is emitted | `const CODE_SYNTAX_PLATFORMS = ["WEB", "ANDROID"] as const` | 475 / 1 | covered | — |
| a code syntax entry carries the host's snippet | `if (typeof code === "string") result.push({ platform, code: "" })` | 475 / 1 | covered | — |
| a code syntax entry names its platform | `if (typeof code === "string") result.push({ platform: "WEB", code })` | 475 / 1 | covered | — |
| a variable's declared scopes are carried | `typeof scope === "string" ? [] : [],` | 474 / 2 | covered | — |
| the alias memo key includes the mode id | `return `${variableId}`` | 476 / 0 | **gap** | `an alias resolves in the mode asked for, or its target's default mode`, `one variable aliasing one target in two modes resolves each mode on its own` |
| the alias memo key includes the variable id | `return `${modeId}`` | 475 / 1 | covered | — |
| mode ids fall back to the keys of valuesByMode | `return []` | 474 / 2 | covered | — |
| the lookup budget runs from the start of the read | `this.deadline = Date.now() - 1` | 459 / 17 | covered | — |
| the budget trips once the deadline passes | `if (!this.tripped && Date.now() > this.deadline + 1e9) this.tripped = true` | 474 / 2 | covered | — |
| a variable id is looked up once | `if (false) return this.variables.get(id) ?? null` | 475 / 1 | covered | — |
| no variable lookup is made past the deadline | `if (false) return null ⏎ const lookup = this.api.getVariableByIdAsync` | 476 / 0 | **gap** | — *(open)* |
| a resolved variable is remembered for the rest of the read | `void value` | 475 / 1 | covered | — |
| a collection id is looked up once | `if (false) return this.collections.get(id) ?? null` | 475 / 1 | covered | — |
| no collection lookup is made past the deadline | `if (false) return null ⏎ const lookup = this.api.getVariableCollectionByIdAsync` | 476 / 0 | **gap** | `a budget spent mid-read stops the collection lookup and marks its aliases retryable` |
| a resolved collection is remembered for the rest of the read | `void value` | 475 / 1 | covered | — |
| a host lookup that never settles is skipped rather than awaited | `return (await call()) ?? null` | 475 / 1 | covered | — |
| a repeated alias resolution is answered from the memo | `const cached = this.memo.get(key) ⏎ if (false) return cached as AliasResolution` | 476 / 0 | **gap** | — *(open)* |
| alias resolution stops at the deadline | `if (false) return EXHAUSTED` | 476 / 0 | **gap** | `a budget spent mid-read stops the collection lookup and marks its aliases retryable` |
| an exhausted budget is reported as retryable | `error: { code: "LIMIT_EXCEEDED", retryable: false },` | 475 / 1 | covered | — |
| an alias cycle is reported as not retryable | `error: { code: "LIMIT_EXCEEDED", retryable: true },` | 475 / 1 | covered | — |
| an alias cycle is caught by the resolution stack | `if (false) {` | 475 / 1 | covered | — |
| a variable joins the resolution stack while its alias is followed | `const raw` | 475 / 1 | covered | — |
| a settled variable leaves the resolution stack so a sibling alias can reuse it | `this.memo.set(key, result)` | 476 / 0 | **gap** | — *(open)* |
| a deadline that passed during the lookup is reported as exhausted | `if (false) return EXHAUSTED` | 476 / 0 | **gap** | — *(open)* |
| an alias target the host does not return is NODE_NOT_FOUND | `error: { code: "LIMIT_EXCEEDED", retryable: false }, ⏎ } ⏎ this.memo.set(key, missing)` | 475 / 1 | covered | — |
| an alias target's value is read from the requested mode | `let source = undefined as unknown` | 476 / 0 | **gap** | `an alias resolves in the mode asked for, or its target's default mode`, `one variable aliasing one target in two modes resolves each mode on its own` |
| a mode the target does not declare falls back to its collection's default | `resolvedModeId = modeId` | 476 / 0 | **gap** | `an alias resolves in the mode asked for, or its target's default mode` |
| an alias chain follows the mode the previous hop resolved to | `result = await this.resolve(source.id, modeId, stack, signal)` | 476 / 0 | **gap** | `an alias resolves in the mode asked for, or its target's default mode` |
| a resolved alias carries the target's value | `: { status: "success", value: { kind: "string", value: "" } }` | 473 / 3 | covered | — |
| a settled alias resolution is memoised | `return result` | 476 / 0 | **gap** | — *(open)* |
| every considered collection counts as visited | `this.visited += 0 ⏎ if (this.truncation !== undefined) return false ⏎ if (this.returned >= this.limits.returnedNo…` | 475 / 1 | covered | — |
| the returned-collection ceiling is inclusive | `if (this.returned >= this.limits.returnedNodes + 1) {` | 476 / 0 | **gap** | `the collection ceiling, the walk cut and the byte ceiling each report their own total`, `a truncated walk outranks an exhausted budget, and the emission cut outranks an unresolved collection` |
| emitted bytes accumulate across collections | `const encoded = byteLength(collection)` | 476 / 0 | **gap** | `the collection ceiling, the walk cut and the byte ceiling each report their own total` |
| the encoded-byte ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the collection ceiling, the walk cut and the byte ceiling each report their own total` |
| an emitted collection counts toward the returned ceiling | `this.collections.push(collection)` | 476 / 0 | **gap** | `the collection ceiling, the walk cut and the byte ceiling each report their own total`, `a truncated walk outranks an exhausted budget, and the emission cut outranks an unresolved collection` |
| a variable's mode ids come from its collection's declared modes | `const declared: string[] = []` | 476 / 0 | **gap** | `only the modes the collection declares are emitted` |
| an unreachable collection falls back to the variable's own mode ids | `const modeIds = declared` | 474 / 2 | covered | — |
| a declared mode the variable carries a value for is emitted | `if (Object.hasOwn(valuesByMode, modeId)) continue` | 467 / 9 | covered | — |
| a mode value names its mode | `const entry: VariableModeValue = { modeId: "", source }` | 469 / 7 | covered | — |
| a mode value carries the raw source value | `const entry: VariableModeValue = { modeId, source: { kind: "string", value: "" } }` | 468 / 8 | covered | — |
| resolveAliases follows an alias to its value | `if (false && source.kind === "alias") {` | 470 / 6 | covered | — |
| a successfully resolved alias carries its resolved value | `if (false) entry.resolved = (resolved as { value: VariableValue }).value` | 473 / 3 | covered | — |
| an alias that could not be resolved carries the reason | `else void resolved` | 473 / 3 | covered | — |
| a variable's mode values reach the definition | `void entry` | 467 / 9 | covered | — |
| a variable definition carries its id | `id: id + "!", ⏎ name: string(variable.name),` | 469 / 7 | covered | — |
| a variable definition carries its name | `name: "", ⏎ collectionId:` | 474 / 2 | covered | — |
| a variable definition names its collection | `collectionId: "",` | 474 / 2 | covered | — |
| a variable definition carries its scopes | `scopes: [],` | 474 / 2 | covered | — |
| a variable definition carries its code syntax | `codeSyntax: [],` | 475 / 1 | covered | — |
| a fallback mode id appears once across the group | `seen.add(modeId) ⏎ modes.push({ id: modeId, name: "" })` | 476 / 0 | **gap** | `a budget spent mid-read stops the collection lookup and marks its aliases retryable` |
| a fallback mode leaves its name empty rather than guessing | `modes.push({ id: modeId, name: modeId })` | 475 / 1 | covered | — |
| the lookup budget defaults to eight seconds | `const DEFAULT_VARIABLE_LOOKUP_BUDGET_MS = 0` | 461 / 15 | covered | — |
| a caller-supplied lookup budget is honoured | `DEFAULT_VARIABLE_LOOKUP_BUDGET_MS,` | 474 / 2 | covered | — |
| resolveAliases true reaches the definition reader | `const resolveAliases = false` | 470 / 6 | covered | — |
| the read honours the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 474 / 2 | covered | — |
| an id bound by two nodes is looked up once | `for (const id of variableIdsOf(record(raw))) { ⏎ if (false) continue` | 475 / 1 | covered | — |
| the id list stops being walked once the budget is out | `attempted += 1` | 475 / 1 | covered | — |
| the attempted-lookup count feeds the budget truncation | `attempted += 0` | 474 / 2 | covered | — |
| a variable the host will not return does not end the read | `if (raw === null) break` | 475 / 1 | covered | — |
| a newly seen collection joins the emission order | `grouped.set(collectionId, [raw])` | 460 / 16 | covered | — |
| a second variable in a known collection joins its group | `} else void raw` | 470 / 6 | covered | — |
| the collection loop stops at the emission ceiling | `if (!emission.consider()) continue` | 476 / 0 | **gap** | — *(open)* |
| a collection with an id is emitted | `if (id.length >= 0) continue` | 460 / 16 | covered | — |
| a collection the host will not return is counted as unresolved | `if (raw === null) unresolvedCollections += 0` | 475 / 1 | covered | — |
| a declared mode carries its name | `return [{ id: modeId, name: "" }]` | 475 / 1 | covered | — |
| a declared mode carries its id | `return [{ id: "", name: string(item.name) }]` | 475 / 1 | covered | — |
| a collection envelope carries its id | `id: id + "!", ⏎ name: string(collection.name),` | 473 / 3 | covered | — |
| a collection envelope carries its name | `name: "",` | 474 / 2 | covered | — |
| an unresolved collection falls back to the modes its variables carry | `modes: declared,` | 475 / 1 | covered | — |
| a serialized variable reaches its collection | `if (false) definition.variables.push(serialized as VariableDefinition)` | 462 / 14 | covered | — |
| the collection loop stops once the byte ceiling is hit | `if (!emission.pushCollection(definition)) continue` | 476 / 0 | **gap** | — *(open)* |
| an exhausted budget is reported as a nodeLimit truncation | `? { reason: "byteLimit", visitedNodes: attempted }` | 474 / 2 | covered | — |
| an unresolved collection is reported as a nodeLimit truncation | `? { reason: "byteLimit", visitedNodes: emission.visited }` | 475 / 1 | covered | — |
| a truncated walk outranks an exhausted budget | `budgetTruncation ?? ⏎ walked.truncation ??` | 476 / 0 | **gap** | `a truncated walk outranks an exhausted budget, and the emission cut outranks an unresolved collection` |
| an exhausted budget outranks the emission cut | `emission.truncation ?? ⏎ budgetTruncation ??` | 475 / 1 | covered | — |
| the emission cut outranks an unresolved collection | `unresolvedTruncation ?? ⏎ emission.truncation` | 476 / 0 | **gap** | `a truncated walk outranks an exhausted budget, and the emission cut outranks an unresolved collection` |
| the result carries the collected collections | `collections: [],` | 462 / 14 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 472 / 4 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = truncation as Truncation` | 471 / 5 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `each value kind the protocol names is tagged with its own kind` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 475 / 1 | covered | — |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 462 / 14 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | VariableEmission reads only returnedNodes and encodedBytes |
| cancellation is checked at a batch boundary in the variable lookup loop | `signal?.throwIfAborted() ⏎ // The lookups themselves` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the collection loop | `signal?.throwIfAborted() ⏎ if (!emission.consider()) break` | 476 / 0 | **gap** | — *(open)* |
| the variable lookup loop checks cancellation on every id | `// The lookups themselves` | 476 / 0 | **gap** | — *(open)* |
| the collection loop checks cancellation on every collection | `if (!emission.consider()) break` | 476 / 0 | **gap** | — *(open)* |
| the definition loop checks cancellation on every variable | `const serialized = await readDefinition(` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/render.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a failed item carries the code it was given | `const error: ToolError = { ⏎ code: "INTERNAL_ERROR",` | 467 / 9 | covered | — |
| a failed item carries the canonical message for its code | `message: "",` | 476 / 0 | **gap** | `a raster answer to an svg request, and an svg answer to a raster one, are refused`, `an error asset from the UI keeps its own code`, `a validation the UI never answers fails that item as TIMEOUT`, `a UI that refuses the message fails that item as INTERNAL_ERROR` |
| a failed screenshot item is not retryable | `message: CANONICAL_MESSAGES[code], ⏎ retryable: true,` | 468 / 8 | covered | — |
| a selection selector captures the selected node ids | `return []` | 475 / 1 | covered | — |
| a selection selector reaches capturedSelectionIds | `if ("selection" in selector) return []` | 475 / 1 | covered | — |
| a single nodeId selector captures that one id | `if ("nodeId" in selector) return []` | 458 / 18 | covered | — |
| a nodeIds selector captures every id it names | `return []` | 470 / 6 | covered | — |
| svg exports ask the host for SVG_STRING | `format: "PNG",` | 475 / 1 | covered | — |
| svgOutlineText reaches the host export settings | `svgOutlineText: undefined,` | 475 / 1 | covered | — |
| svgIdAttribute reaches the host export settings | `svgIdAttribute: undefined,` | 475 / 1 | covered | — |
| svgSimplifyStroke reaches the host export settings | `svgSimplifyStroke: undefined,` | 475 / 1 | covered | — |
| raster scale defaults to 1 | `const scale = input.scale ?? 2` | 475 / 1 | covered | — |
| a scale of exactly 0.25 is accepted | `if (!Number.isFinite(scale) \|\| scale <= 0.25 \|\| scale > 4) {` | 475 / 1 | covered | — |
| a scale of exactly 4 is accepted | `if (!Number.isFinite(scale) \|\| scale < 0.25 \|\| scale >= 4) {` | 475 / 1 | covered | — |
| a jpeg request asks the host for JPG | `format: "PNG",` | 475 / 1 | covered | — |
| a png request asks the host for PNG | `format: input.format === "jpeg" ? "JPG" : "JPG",` | 475 / 1 | covered | — |
| the requested scale reaches the host constraint | `constraint: { type: "SCALE", value: 1 },` | 475 / 1 | covered | — |
| an absent render-bounds property is not an empty one | `return hostGet(node, "absoluteRenderBounds") == null` | 459 / 17 | covered | — |
| the host exporter is invoked on the node itself | `).call(undefined, settings)` | 476 / 0 | **gap** | `the host exporter is called on the node it belongs to` |
| the export settings reach the host exporter | `).call(node, {})` | 473 / 3 | covered | — |
| a settled validation is dropped from the pending map | `void validationId` | 475 / 1 | covered | — |
| a settled validation cancels its timeout | `void pending` | 476 / 0 | **gap** | — *(open)* |
| a settled validation unhooks its abort listener | `void pending` | 476 / 0 | **gap** | — *(open)* |
| a settled validation answers the waiting caller | `void result` | killed at 180 s; 2 tests timed out | covered | — |
| a screenshotValidated message is accepted | `if (!isRecord(input) \|\| input.type === "screenshotValidated") return false` | 475 / 1 | covered | — |
| a validation reply naming a string id is accepted | `if (typeof id === "string") return false` | 475 / 1 | covered | — |
| a success asset passes the reply's shape check | `(asset.status !== "success_x" && asset.status !== "error")` | 475 / 1 | covered | — |
| an error asset passes the reply's shape check | `(asset.status !== "success" && asset.status !== "error_x")` | 476 / 0 | **gap** | `an error asset from the UI keeps its own code` |
| the validated asset itself is handed back to the waiting caller | `return settlePendingValidation(id, itemError("INTERNAL_ERROR"))` | 475 / 1 | covered | — |
| a present plugin UI is used for validation | `if (ui !== undefined) return itemError("INTERNAL_ERROR")` | 473 / 3 | covered | — |
| a live signal proceeds to validation | `if (signal?.aborted === false) return itemError("CANCELLED")` | 475 / 1 | covered | — |
| each validation takes a fresh id | `const validationId = `screenshot-${validationSeq}`` | 476 / 0 | **gap** | `each item takes a validation id of its own` |
| a validation that never answers fails as TIMEOUT | `settlePendingValidation(validationId, itemError("INTERNAL_ERROR")) ⏎ }, timeoutMs),` | 475 / 1 | covered | — |
| the validation timeout is the one the caller passed | `}, 1e9),` | 475 / 1 | covered | — |
| a cancelled validation fails as CANCELLED | `settlePendingValidation(validationId, itemError("INTERNAL_ERROR"))` | 476 / 0 | **gap** | — *(open)* |
| a validation in flight listens for cancellation | `void abort` | 476 / 0 | **gap** | `a cancelled validation fails that item as CANCELLED` |
| a validation in flight is recorded so the reply can find it | `void pending` | killed at 180 s; 2 tests timed out | covered | — |
| the UI is asked with a validateScreenshot message | `type: "validateScreenshot_x",` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| the validation request carries its correlation id | `type: "validateScreenshot", ⏎ validationId: "",` | 475 / 1 | covered | — |
| the validation request carries the item to validate | `item: {},` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields`, `an svg export is validated by the UI and keeps its unsafe verdict` |
| a UI that refuses the message fails as INTERNAL_ERROR | `} catch { ⏎ settlePendingValidation(validationId, itemError("TIMEOUT")) ⏎ }` | 476 / 0 | **gap** | `a UI that refuses the message fails that item as INTERNAL_ERROR` |
| the raster codec forwards the requested format | `{ ⏎ format: "png", ⏎ nodeId: "", ⏎ bytes, ⏎ },` | 476 / 0 | **gap** | `a jpeg export asks the UI to validate a jpeg and is tagged jpeg` |
| the raster codec forwards the exported bytes | `bytes: new Uint8Array(), ⏎ },` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| a failed raster validation keeps its own code | `if (result.status === "error") ⏎ return { ok: false, code: "INTERNAL_ERROR" } ⏎ if (result.value.format ===…` | 475 / 1 | covered | — |
| an svg answer to a raster request is rejected | `if (false) return { ok: false, code: "INTERNAL_ERROR" }` | 476 / 0 | **gap** | `a raster answer to an svg request, and an svg answer to a raster one, are refused` |
| the raster codec carries the validated base64 | `dataBase64: "",` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| the raster codec carries the validated width | `width: 0,` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| the raster codec carries the validated height | `height: 0,` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| the raster codec reports the encoded length | `base64Bytes: 0,` | 476 / 0 | not applicable | encodeAsset never reads base64Bytes |
| the raster codec reports no decoded byte count | `decodedBytes: 1,` | 476 / 0 | not applicable | encodeAsset never reads decodedBytes |
| the svg codec asks for an svg validation | `format: "png", ⏎ nodeId: "", ⏎ source,` | 476 / 0 | **gap** | `an svg export is validated by the UI and keeps its unsafe verdict` |
| the svg codec forwards the exported source | `source: "", ⏎ },` | 476 / 0 | **gap** | `an svg export is validated by the UI and keeps its unsafe verdict` |
| a failed svg validation keeps its own code | `if (result.status === "error") ⏎ return { ok: false, code: "INTERNAL_ERROR" } ⏎ if (result.value.format !==…` | 476 / 0 | **gap** | `an error asset from the UI keeps its own code` |
| a raster answer to an svg request is rejected | `if (false) return { ok: false, code: "INTERNAL_ERROR" }` | 476 / 0 | **gap** | `a raster answer to an svg request, and an svg answer to a raster one, are refused` |
| the svg codec carries the validated source | `source: "",` | 475 / 1 | covered | — |
| the svg codec carries the safety verdict | `safe: true,` | 475 / 1 | covered | — |
| the svg codec carries the rejection when there is one | `if (false) { ⏎ encoded.rejection = result.value.rejection ⏎ }` | 475 / 1 | covered | — |
| a string svg export is accepted | `if (typeof exported === "string") return itemError("INTERNAL_ERROR")` | 473 / 3 | covered | — |
| a failed svg encode keeps its own code | `if (!encoded.ok) return itemError("INTERNAL_ERROR") ⏎ const value:` | 476 / 0 | **gap** | `an error asset from the UI keeps its own code` |
| an svg asset is tagged svg | `format: "png" as "svg", ⏎ nodeId,` | 473 / 3 | covered | — |
| an svg asset names the node it came from | `nodeId: "", ⏎ source: encoded.source,` | 473 / 3 | covered | — |
| an svg asset carries its source | `source: "",` | 473 / 3 | covered | — |
| an svg asset carries the safety verdict | `safe: true,` | 474 / 2 | covered | — |
| an unsafe svg asset carries its rejection | `if (false) value.rejection = encoded.rejection` | 474 / 2 | covered | — |
| a Uint8Array raster export is accepted | `if (exported instanceof Uint8Array) return itemError("INTERNAL_ERROR")` | 468 / 8 | covered | — |
| a failed raster encode keeps its own code | `if (!encoded.ok) return itemError("INTERNAL_ERROR") ⏎ return {` | 475 / 1 | covered | — |
| a raster asset is tagged with the requested format | `format: "png", ⏎ nodeId,` | 476 / 0 | **gap** | `a jpeg export asks the UI to validate a jpeg and is tagged jpeg` |
| a raster asset names the node it came from | `nodeId: "", ⏎ dataBase64: encoded.dataBase64,` | 473 / 3 | covered | — |
| a raster asset carries its base64 payload | `dataBase64: "",` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields`, `the host exporter is called on the node it belongs to` |
| a raster asset carries its width | `width: 0,` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields`, `the host exporter is called on the node it belongs to` |
| a raster asset carries its height | `height: 0,` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields`, `the host exporter is called on the node it belongs to` |
| the validation timeout defaults to ten seconds | `export const SCREENSHOT_VALIDATION_TIMEOUT_MS = 1` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields`, `an svg export is validated by the UI and keeps its unsafe verdict`, `a raster answer to an svg request, and an svg answer to a raster one, are refused`, `an error asset from the UI keeps its own code`, `a validation answered well inside the default window still succeeds`, `a cancelled validation fails that item as CANCELLED`, `each item takes a validation id of its own` |
| a caller-supplied codec is used | `const activeCodec = createUiCodec(signal, validationTimeoutMs)` | 468 / 8 | covered | — |
| a screenshot read reports its total before it starts | `void ids` | 476 / 0 | **gap** | `progress is reported once before the loop and around every export` |
| a host with an id lookup proceeds to export | `if (ids.length > 0 && figma.getNodeByIdAsync !== undefined) {` | 450 / 26 | covered | — |
| each captured id is the one looked up | `node = await lookup("")` | 452 / 24 | covered | — |
| a read error thrown by the lookup ends the whole call | `if (false) { ⏎ throw error ⏎ } ⏎ assets.push(itemError("NODE_NOT_FOUND"))` | 476 / 0 | **gap** | `a read error raised by the node lookup ends the whole call` |
| a lookup that throws fails that item as NODE_NOT_FOUND | `assets.push(itemError("INTERNAL_ERROR")) ⏎ continue ⏎ } ⏎ if (node === null \|\| node === undefined) {` | 475 / 1 | covered | — |
| a node the host does not have fails that item as NODE_NOT_FOUND | `if (node === null \|\| node === undefined) { ⏎ assets.push(itemError("INTERNAL_ERROR"))` | 475 / 1 | covered | — |
| a PAGE named for export is loaded first | `node = node` | 475 / 1 | covered | — |
| a node with no exporter fails that item as UNSUPPORTED_NODE | `assets.push(itemError("INTERNAL_ERROR"))` | 475 / 1 | covered | — |
| a rendering node proceeds to export | `const verdict = visibilityOf(node) ⏎ if (verdict === "renders") {` | 453 / 23 | covered | — |
| a hidden node and an unwalkable chain are told apart | `itemError(verdict === "hidden" ? "LIMIT_EXCEEDED" : "NODE_NOT_VISIBLE"),` | 473 / 3 | covered | — |
| a node with null render bounds fails as EMPTY_NODE_BOUNDS | `if (rendersNothing(node)) { ⏎ assets.push(itemError("INTERNAL_ERROR"))` | 474 / 2 | covered | — |
| a node that puts ink on the page is exported | `if (false) {` | 474 / 2 | covered | — |
| the computed export settings reach the exporter | `const exported = await exporter({})` | 473 / 3 | covered | — |
| the encoded asset is tagged with the id it was asked for | `const asset = await encodeAsset(input, "", exported, activeCodec)` | 470 / 6 | covered | — |
| a cancellation during encoding ends the call | `assets.push(asset)` | 475 / 1 | covered | — |
| a successfully encoded asset reaches the result | `void asset` | 465 / 11 | covered | — |
| progress advances as each asset is finished | `void 0 ⏎ } catch` | 475 / 1 | covered | — |
| progress is reported before each export begins | `void 0 ⏎ const exported` | 476 / 0 | **gap** | `progress is reported once before the loop and around every export` |
| an export that throws fails that item as INTERNAL_ERROR | `assets.push(itemError("NODE_NOT_FOUND")) ⏎ } ⏎ }` | 475 / 1 | covered | — |
| a read error thrown during export ends the whole call | `false ⏎ ) { ⏎ throw error ⏎ } ⏎ assets.push(itemError("INTERNAL_ERROR"))` | 475 / 1 | covered | — |
| the result carries the encoded assets | `assets: [], ⏎ truncated: false,` | 459 / 17 | covered | — |
| a screenshot result is never truncated | `truncated: true, ⏎ observation: observation(startedAt),` | 474 / 2 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `a raster export is validated by the UI and carries the validated fields` |
| an id lookup that vanishes mid-loop fails that item as CAPABILITY_UNAVAILABLE | `assets.push(itemError("INTERNAL_ERROR"))` | 476 / 0 | **gap** | — *(open)* |
| export settings are computed before any node is looked up | `const settings: Record<string, unknown> = {}` | 472 / 4 | covered | — |
| cancellation is checked before each item | `const lookup = figma.getNodeByIdAsync` | 476 / 0 | **gap** | `a signal already aborted stops before the first lookup` |

**`plugin/src/read/motion.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a LINEAR easing type is accepted | delete it | 475 / 1 | covered | — |
| a EASE_IN easing type is accepted | delete it | 476 / 0 | **gap** | `every easing type the protocol names survives a keyframe` |
| a EASE_OUT easing type is accepted | delete it | 475 / 1 | covered | — |
| a EASE_IN_AND_OUT easing type is accepted | delete it | 476 / 0 | **gap** | `every easing type the protocol names survives a keyframe` |
| a EASE_IN_BACK easing type is accepted | delete it | 474 / 2 | covered | — |
| a EASE_OUT_BACK easing type is accepted | delete it | 476 / 0 | **gap** | `every easing type the protocol names survives a keyframe` |
| a EASE_IN_AND_OUT_BACK easing type is accepted | delete it | 476 / 0 | **gap** | `every easing type the protocol names survives a keyframe` |
| a CUSTOM_CUBIC_BEZIER easing type is accepted | delete it | 475 / 1 | covered | — |
| a GENTLE easing type is accepted | delete it | 475 / 1 | covered | — |
| a QUICK easing type is accepted | delete it | 475 / 1 | covered | — |
| a BOUNCY easing type is accepted | delete it | 475 / 1 | covered | — |
| a SLOW easing type is accepted | delete it | 475 / 1 | covered | — |
| a CUSTOM_SPRING easing type is accepted | delete it | 475 / 1 | covered | — |
| a HOLD easing type is accepted | delete it | 475 / 1 | covered | — |
| a VARIABLE_ALIAS easing type is accepted | delete it | 476 / 0 | not applicable | motionEasing returns from the VARIABLE_ALIAS branch before the set is consulted |
| the fills collection is flattened | delete it | 475 / 1 | covered | — |
| the strokes collection is flattened | delete it | 475 / 1 | covered | — |
| the effects collection is flattened | delete it | 475 / 1 | covered | — |
| a node with no motion surface fails as UNSUPPORTED_NODE | `code: "INTERNAL_ERROR",` | 475 / 1 | covered | — |
| an unsupported item carries the canonical message | `message: "",` | 475 / 1 | covered | — |
| an unsupported item is not retryable | `message: CANONICAL_MESSAGES.UNSUPPORTED_NODE, ⏎ retryable: true,` | 475 / 1 | covered | — |
| a truncated walk outranks the emission cut | `return this.emitTruncation ?? this.walkTruncation` | 476 / 0 | **gap** | `a truncated walk outranks the emission cut and keeps its own reason` |
| the first walk truncation wins | `this.walkTruncation = truncation` | 476 / 0 | **gap** | — *(open)* |
| the returned-item ceiling is inclusive | `if (this.items.length >= this.limits.returnedNodes + 1) {` | 476 / 0 | **gap** | `the item ceiling is inclusive and reports how many nodes were inspected` |
| the item ceiling is reported as a nodeLimit truncation | `this.emitTruncation = { reason: "byteLimit", visitedNodes }` | 476 / 0 | **gap** | `the item ceiling is inclusive and reports how many nodes were inspected` |
| emitted bytes accumulate across items | `const encoded = byteLength(item)` | 476 / 0 | **gap** | `the byte ceiling counts every emitted item and reports the total` |
| the encoded-byte ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the byte ceiling counts every emitted item and reports the total` |
| a byteLimit truncation reports the encoded size | `this.emitTruncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `the byte ceiling counts every emitted item and reports the total` |
| an item with content reaches the result | `void item` | 467 / 9 | covered | — |
| the catalogue reader is invoked on figma.motion | `const styles = reader.call(undefined)` | 476 / 0 | **gap** | `the catalogue reader is called on figma.motion` |
| the host's catalogue array is returned | `return []` | 475 / 1 | covered | — |
| a motion colour carries its r channel | `const r = undefined` | 474 / 2 | covered | — |
| a motion colour carries its g channel | `const g = undefined` | 474 / 2 | covered | — |
| a motion colour carries its b channel | `const b = undefined` | 474 / 2 | covered | — |
| a motion colour carries its a channel | `const a = undefined` | 474 / 2 | covered | — |
| a motion colour's channels are the host's own | `return { r: 0, g, b, a }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload`, `manual tracks carry id and base value, on properties and inside collections`, `a motion colour keeps the host's own channels` |
| an alias easing carries the variable id | `return id.length === 0 ? undefined : { type: "VARIABLE_ALIAS", id: "" }` | 475 / 1 | covered | — |
| an easing carries the host's easing type | `type: "LINEAR" as Exclude<MotionEasingType, "VARIABLE_ALIAS">,` | 474 / 2 | covered | — |
| a cubic-bezier easing carries x1 | `const x1 = undefined` | 475 / 1 | covered | — |
| a cubic-bezier easing carries y1 | `const y1 = undefined` | 475 / 1 | covered | — |
| a cubic-bezier easing carries x2 | `const x2 = undefined` | 475 / 1 | covered | — |
| a cubic-bezier easing carries y2 | `const y2 = undefined` | 475 / 1 | covered | — |
| a cubic-bezier easing carries its control points | `void 0` | 475 / 1 | covered | — |
| a spring easing carries its bounce | `if (false) easing.easingFunctionSpring = { bounce: 0 }` | 476 / 0 | **gap** | `every easing type the protocol names survives a keyframe` |
| a FLOAT keyframe value carries its payload | `return number === undefined ? undefined : { type: "FLOAT", value: 0 }` | 475 / 1 | covered | — |
| a COLOR keyframe value carries its payload | `return parsed === undefined ? undefined : { type: "COLOR", value: { r: 0, g: 0, b: 0, a: 0 } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload`, `manual tracks carry id and base value, on properties and inside collections`, `a motion colour keeps the host's own channels` |
| a TEXT_DATA keyframe value carries its text | `? { type: "TEXT_DATA", value: "" }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a VECTOR keyframe value carries its point | `: { type: "VECTOR", value: { x: 0, y: 0 } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a BOOL keyframe value carries its flag | `? { type: "BOOL", value: true }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a CIRCLE keyframe value carries centre and radius | `: { type: "CIRCLE", value: { x: 0, y: 0, radius: 0 } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a LINE keyframe value carries both endpoints | `: { type: "LINE", value: { x: 0, y: 0, x2: 0, y2: 0 } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a CIRCLE_POINT keyframe value carries its angle and radius | `: { type: "CIRCLE_POINT", value: { x: 0, y: 0, radius: 0, angle: 0 } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a COLOR_POINT keyframe value carries its position | `: { type: "COLOR_POINT", value: { x: 0, y: 0, color: parsed } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| a COLOR_POINT keyframe value carries its colour | `: { type: "COLOR_POINT", value: { x, y, color: { r: 0, g: 0, b: 0, a: 0 } } }` | 476 / 0 | **gap** | `every keyframe value shape the protocol names carries its payload` |
| an unrecognised keyframe value keeps the host's tag | `? { type: "unsupported", tag: "" }` | 475 / 1 | covered | — |
| a FLOAT keyframe value is recognised rather than marked unsupported | `case "FLOAT_X":` | 474 / 2 | covered | — |
| a COLOR keyframe value is recognised rather than marked unsupported | `case "COLOR_X":` | 475 / 1 | covered | — |
| a TEXT_DATA keyframe value is recognised rather than marked unsupported | `case "TEXT_DATA_X":` | 475 / 1 | covered | — |
| a VECTOR keyframe value is recognised rather than marked unsupported | `case "VECTOR_X":` | 475 / 1 | covered | — |
| a BOOL keyframe value is recognised rather than marked unsupported | `case "BOOL_X":` | 475 / 1 | covered | — |
| a CIRCLE keyframe value is recognised rather than marked unsupported | `case "CIRCLE_X":` | 475 / 1 | covered | — |
| a LINE keyframe value is recognised rather than marked unsupported | `case "LINE_X":` | 475 / 1 | covered | — |
| a CIRCLE_POINT keyframe value is recognised rather than marked unsupported | `case "CIRCLE_POINT_X":` | 475 / 1 | covered | — |
| a COLOR_POINT keyframe value is recognised rather than marked unsupported | `case "COLOR_POINT_X":` | 475 / 1 | covered | — |
| a keyframe carries its id | `return { id: "", timelinePosition, value: parsed, easing }` | 474 / 2 | covered | — |
| a keyframe carries its timeline position | `return { id, timelinePosition: 0, value: parsed, easing }` | 474 / 2 | covered | — |
| a keyframe carries its value | `return { id, timelinePosition, value: { type: "BOOL", value: true }, easing } ⏎ }` | 474 / 2 | covered | — |
| a keyframe carries its easing | `return { id, timelinePosition, value: parsed, easing: { type: "LINEAR" } } ⏎ }` | 474 / 2 | covered | — |
| a well-formed keyframe joins the track | `void parsed` | 474 / 2 | covered | — |
| the SET keyframe operation is accepted | delete it | 475 / 1 | covered | — |
| the OFFSET keyframe operation is accepted | delete it | 476 / 0 | **gap** | `all three keyframe operations survive, and a track keeps the one it has` |
| the SCALE keyframe operation is accepted | delete it | 476 / 0 | **gap** | `all three keyframe operations survive, and a track keeps the one it has` |
| a track carries its id | `result.push({ ⏎ id: "", ⏎ keyframeOperation, ⏎ keyframes: keyframes(track.keyframes), ⏎ })` | 475 / 1 | covered | — |
| a track carries its keyframe operation | `keyframeOperation: "SET", ⏎ keyframes: keyframes(track.keyframes),` | 476 / 0 | **gap** | `all three keyframe operations survive, and a track keeps the one it has` |
| a track carries its keyframes | `keyframes: [],` | 475 / 1 | covered | — |
| a binding with a tracks field is a keyframe binding | `Object.hasOwn(value, "tracks_x") \|\| Object.hasOwn(value, "timelineDuration")` | 476 / 0 | **gap** | — *(open)* |
| a binding with a timelineDuration is a keyframe binding | `Object.hasOwn(value, "tracks") \|\| Object.hasOwn(value, "timelineDuration_x")` | 476 / 0 | **gap** | `a binding is recognised by tracks or by timelineDuration alone` |
| a manual binding is recognised by its keyframes field | `return Object.hasOwn(value, "keyframes_x") && Object.hasOwn(value, "id")` | 474 / 2 | covered | — |
| a manual binding is recognised by its id field | `return Object.hasOwn(value, "keyframes") && Object.hasOwn(value, "id_x")` | 474 / 2 | covered | — |
| an animation binding names the field it animates | `return { ⏎ field: { type: "property", name: "" }, ⏎ baseValue, ⏎ timelineDuration, ⏎ tracks: tracks(value.tra…` | 474 / 2 | covered | — |
| an animation binding carries its base value | `baseValue: { type: "BOOL", value: true }, ⏎ timelineDuration,` | 475 / 1 | covered | — |
| an animation binding carries its timeline duration | `timelineDuration: 0, ⏎ tracks: tracks(value.tracks),` | 475 / 1 | covered | — |
| an animation binding carries its tracks | `tracks: [],` | 475 / 1 | covered | — |
| a manual binding names the field it animates | `return { field: { type: "property", name: "" }, id, baseValue, keyframes: keyframes(value.keyframes) }` | 475 / 1 | covered | — |
| a manual binding carries its id | `return { field, id: "", baseValue, keyframes: keyframes(value.keyframes) }` | 476 / 0 | **gap** | `manual tracks carry id and base value, on properties and inside collections` |
| a manual binding carries its base value | `return { field, id, baseValue: { type: "BOOL", value: true }, keyframes: keyframes(value.keyframes) } ⏎ }` | 476 / 0 | **gap** | `manual tracks carry id and base value, on properties and inside collections` |
| a manual binding carries its keyframes | `return { field, id, baseValue, keyframes: [] } ⏎ }` | 475 / 1 | covered | — |
| the three indexed collections are excluded from the plain property scan | `.filter(() => true)` | 476 / 0 | **gap** | — *(open)* |
| a node's plain animated properties are scanned | `return [] ⏎ .filter` | 473 / 3 | covered | — |
| a numeric index in an indexed collection is walked | `.filter(() => false)` | 475 / 1 | covered | — |
| indexed-collection entries come back in ascending index order | `.sort((left, right) => right - left)` | 475 / 1 | covered | — |
| property entries come back in name order | `return ownKeys(raw) ⏎ .map((name)` | 475 / 1 | covered | — |
| a property entry carries its value | `.map((name) => [name, undefined] as [string, unknown])` | 475 / 1 | covered | — |
| a plain animated property becomes a binding | `const binding = animationBinding({ type: "property", name }, undefined)` | 474 / 2 | covered | — |
| an effect's own animated fields become bindings | `const group = map[collection] ⏎ for (const index of numericIndices(group)) { ⏎ const item = record(group)[Strin…` | 475 / 1 | covered | — |
| an effect's animated sub-properties become bindings | `for (const [propertyId, value] of []) { ⏎ const binding = animationBinding(` | 475 / 1 | covered | — |
| a fill or stroke animated as a whole becomes one binding | `if (false) { ⏎ const binding = animationBinding(` | 475 / 1 | covered | — |
| a fill or stroke's animated sub-properties become bindings | `for (const [propertyId, value] of []) { ⏎ const binding = animationBinding(` | 475 / 1 | covered | — |
| a plain manual track becomes a binding | `const binding = manualBinding({ type: "property", name }, undefined)` | 474 / 2 | covered | — |
| an effect's own manual tracks become bindings | `for (const [field, value] of []) { ⏎ const binding = manualBinding(` | 476 / 0 | **gap** | `manual tracks carry id and base value, on properties and inside collections` |
| an effect's manual sub-property tracks become bindings | `for (const [propertyId, value] of []) { ⏎ const binding = manualBinding(` | 476 / 0 | **gap** | `manual tracks carry id and base value, on properties and inside collections` |
| a fill or stroke manually tracked as a whole becomes one binding | `if (false) { ⏎ const binding = manualBinding(` | 475 / 1 | covered | — |
| a fill or stroke's manual sub-property tracks become bindings | `for (const [propertyId, value] of []) { ⏎ const binding = manualBinding(` | 476 / 0 | **gap** | `manual tracks carry id and base value, on properties and inside collections` |
| an applied style prop keeps a string value | `if (typeof raw === "boolean") return raw` | 475 / 1 | covered | — |
| an applied style prop keeps a boolean value | `if (typeof raw === "string") return raw` | 475 / 1 | covered | — |
| an applied style prop keeps a finite number value | `if (false) return raw as number` | 475 / 1 | covered | — |
| an applied style prop keeps an easing value | `return undefined` | 475 / 1 | covered | — |
| an applied style prop carries its name | `if (value !== undefined) props.push({ name: "", value })` | 475 / 1 | covered | — |
| applied style props come back in name order | `for (const name of ownKeys(raw)) { ⏎ const value = appliedPropValue(raw[name])` | 475 / 1 | covered | — |
| an applied animation style carries its instance id | `id: "", ⏎ styleId, ⏎ name: string(style.name),` | 474 / 2 | covered | — |
| an applied animation style names the style it applies | `styleId: "", ⏎ name: string(style.name), ⏎ } ⏎ const duration` | 473 / 3 | covered | — |
| an applied animation style carries its name | `name: "", ⏎ } ⏎ const duration` | 474 / 2 | covered | — |
| an applied animation style carries its duration | `if (false) result.duration = duration as number` | 475 / 1 | covered | — |
| an applied animation style carries its timeline offset | `if (false) result.timelineOffset = timelineOffset as number` | 475 / 1 | covered | — |
| an applied animation style carries its props | `if (false) result.props = props as AppliedStyleProp[]` | 475 / 1 | covered | — |
| a timeline carries its id | `result.push({ id: "", duration })` | 473 / 3 | covered | — |
| a timeline carries its duration | `result.push({ id, duration: 0 })` | 473 / 3 | covered | — |
| an available animation style carries its style id | `styleId: "", ⏎ name: string(style.name), ⏎ } ⏎ if (typeof style.description` | 475 / 1 | covered | — |
| an available animation style carries its name | `name: "", ⏎ } ⏎ if (typeof style.description` | 475 / 1 | covered | — |
| an available animation style carries its description | `if (false) { ⏎ result.description = style.description as string ⏎ }` | 475 / 1 | covered | — |
| an available style prop carries its value | `if (typeof value === "string") props.push({ name, value: "" })` | 475 / 1 | covered | — |
| an available style prop carries its name | `if (typeof value === "string") props.push({ name: "", value })` | 475 / 1 | covered | — |
| an available animation style carries its prop list | `void props` | 475 / 1 | covered | — |
| a node is motion-capable only if it carries animationStyles | `hasHostField(node, "animationStyles_x") &&` | 467 / 9 | covered | — |
| a node is motion-capable only if it carries animations | `hasHostField(node, "animations_x") &&` | 467 / 9 | covered | — |
| a node is motion-capable only if it carries manualKeyframeTracks | `hasHostField(node, "manualKeyframeTracks_x") &&` | 467 / 9 | covered | — |
| a node is motion-capable only if it carries timelines | `hasHostField(node, "timelines_x")` | 467 / 9 | covered | — |
| a motion record names its node | `nodeId: "", ⏎ animationStyles:` | 469 / 7 | covered | — |
| a motion record carries its applied styles | `animationStyles: [],` | 469 / 7 | covered | — |
| a motion record carries its animations | `animations: [],` | 474 / 2 | covered | — |
| a motion record carries its manual tracks | `manualKeyframeTracks: [],` | 474 / 2 | covered | — |
| a motion record carries its timelines | `timelines: [],` | 473 / 3 | covered | — |
| a node with animations is content | `false \|\|` | 476 / 0 | **gap** | `every easing type the protocol names survives a keyframe`, `every keyframe value shape the protocol names carries its payload`, `all three keyframe operations survive, and a track keeps the one it has`, `a binding is recognised by tracks or by timelineDuration alone`, `a motion colour keeps the host's own channels`, `a node whose only content is an animation is emitted` |
| a node with an applied animation style is content | `false \|\|` | 470 / 6 | covered | — |
| a node with a manual keyframe track is content | `false` | 475 / 1 | covered | — |
| an unreadable node is reported rather than dropped | `return hasContent((item as { value: NodeMotion }).value)` | 471 / 5 | covered | — |
| includeAvailableStyles true fetches the catalogue | `false ⏎ ? availableStyles(motion.catalog()) ⏎ : []` | 475 / 1 | covered | — |
| the read honours the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 470 / 6 | covered | — |
| a truncated forest walk marks the result truncated | `void walked` | 475 / 1 | covered | — |
| every walked node counts toward visitedNodes | `visitedNodes += 0` | 475 / 1 | covered | — |
| a node with nothing to report is left out of items | `if (false) continue` | 475 / 1 | covered | — |
| the item loop stops once the emission ceiling is hit | `if (!emission.push(item, visitedNodes)) continue` | 476 / 0 | **gap** | — *(open)* |
| the result carries the emitted items | `items: [],` | 467 / 9 | covered | — |
| the result reports how many nodes were inspected | `visitedNodes: 0, ⏎ truncated:` | 475 / 1 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| a non-empty catalogue reaches the result | `if (false) result.availableStyles = catalog` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 476 / 0 | **gap** | `the item ceiling is inclusive and reports how many nodes were inspected`, `a truncated walk outranks the emission cut and keeps its own reason`, `the byte ceiling counts every emitted item and reports the total` |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 476 / 0 | **gap** | `a node whose only content is an animation is emitted` |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `a node whose only content is an animation is emitted` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 472 / 4 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | motion's ItemEmission reads only returnedNodes and encodedBytes |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 468 / 8 | covered | — |
| cancellation is checked at a batch boundary in the motion item loop | delete it | 476 / 0 | **gap** | — *(open)* |
| the motion item loop checks cancellation on every item | `visitedNodes += 1` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/components.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a documentation link carries its uri | `const entry: DocumentationReference = { uri: "" }` | 475 / 1 | covered | — |
| a documentation link carries its label when the host has one | `if (false) entry.label = link.label as string` | 475 / 1 | covered | — |
| a documentation link reaches the component | `void entry` | 475 / 1 | covered | — |
| a variant property carries its value | `if (typeof value === "string") properties.push({ name, value: "" })` | 474 / 2 | covered | — |
| a variant property carries its name | `if (typeof value === "string") properties.push({ name: "", value })` | 474 / 2 | covered | — |
| a component set's variant properties are read | `const properties: NamedVariantProperty[] = [] ⏎ for (const [name, value] of []) {` | 474 / 2 | covered | — |
| a component's property definitions are read | `entries = []` | 475 / 1 | covered | — |
| a property definition carries the host's default value | `const defaultValue = componentPropertyValue(type, undefined)` | 475 / 1 | covered | — |
| a property definition carries its name | `const entry: ComponentPropertyDefinition = { name: "", defaultValue }` | 475 / 1 | covered | — |
| a VARIANT property's options are collected | `if (type === "VARIANT_X") {` | 475 / 1 | covered | — |
| a variant option carries its value | `? [{ kind: "variant" as const, value: "" }]` | 475 / 1 | covered | — |
| a VARIANT property's options reach preferredValues | `if (false) entry.preferredValues = options` | 475 / 1 | covered | — |
| an INSTANCE_SWAP property's preferred values are collected | `if (type === "INSTANCE_SWAP_X") {` | 475 / 1 | covered | — |
| an instance-swap preferred value carries its key | `? [{ kind: "instanceSwap" as const, value: "" }]` | 475 / 1 | covered | — |
| an INSTANCE_SWAP property's keys reach preferredValues | `if (false) entry.preferredValues = preferred` | 475 / 1 | covered | — |
| a property definition reaches the component | `void entry` | 475 / 1 | covered | — |
| a COMPONENT node is serialized | `(node.type !== "COMPONENT_X" && node.type !== "COMPONENT_SET") ⏎ ) {` | 466 / 10 | covered | — |
| a COMPONENT_SET node is serialized | `(node.type !== "COMPONENT" && node.type !== "COMPONENT_SET_X")` | 474 / 2 | covered | — |
| a component definition carries its id | `id: id + "!", ⏎ name: string(node.name),` | 465 / 11 | covered | — |
| a component definition carries its name | `name: "", ⏎ documentation:` | 473 / 3 | covered | — |
| a component definition carries its documentation links | `documentation: [],` | 475 / 1 | covered | — |
| a component definition carries its variant properties | `variantProperties: [],` | 474 / 2 | covered | — |
| a component definition carries its property definitions | `propertyDefinitions: [],` | 475 / 1 | covered | — |
| a variant names the component set it belongs to | `if (parent.type === "COMPONENT_SET_X") {` | 475 / 1 | covered | — |
| the component-set id is the parent's own id | `if (componentSetId.length > 0) result.componentSetId = ""` | 475 / 1 | covered | — |
| a component definition carries its description | `if (false) { ⏎ result.description = description as string ⏎ }` | 475 / 1 | covered | — |
| a truncated walk outranks the emission cut | `return this.emitTruncation ?? this.walkTruncation` | 476 / 0 | **gap** | `a truncated walk outranks the emission cut` |
| the first walk truncation wins | `this.walkTruncation = truncation` | 476 / 0 | **gap** | `an exhausted main-component budget stops before the first batch` |
| every considered payload counts toward visitedNodes | `this.considered += 0` | 476 / 0 | **gap** | `components and instances share one returned-node ceiling` |
| components and instances share one returned-node ceiling | `const returned = this.components.length` | 476 / 0 | **gap** | `components and instances share one returned-node ceiling`, `main components are looked up sixteen at a time, and the pass stops at the ceiling` |
| the returned-node ceiling is inclusive | `if (returned >= this.limits.returnedNodes + 1) {` | 475 / 1 | covered | — |
| emitted bytes accumulate across payloads | `const encoded = byteLength(payload)` | 476 / 0 | **gap** | `the byte ceiling counts every emitted payload and reports the total` |
| the encoded-byte ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the byte ceiling counts every emitted payload and reports the total` |
| an accepted component reaches the result | `void component` | 465 / 11 | covered | — |
| an accepted instance relationship reaches the result | `void relationship` | 471 / 5 | covered | — |
| the main-component budget defaults to eight seconds | `const DEFAULT_MAIN_COMPONENT_BUDGET_MS = 0` | 471 / 5 | covered | — |
| a caller-supplied main-component budget is honoured | `DEFAULT_MAIN_COMPONENT_BUDGET_MS` | 475 / 1 | covered | — |
| the read honours the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 469 / 7 | covered | — |
| the walk collects COMPONENT nodes | `if (node.type === "COMPONENT_X" \|\| node.type === "COMPONENT_SET") {` | 470 / 6 | covered | — |
| the walk collects COMPONENT_SET nodes | `if (node.type === "COMPONENT" \|\| node.type === "COMPONENT_SET_X") {` | 474 / 2 | covered | — |
| the walk collects INSTANCE nodes | `if (node.type === "INSTANCE_X") {` | 470 / 6 | covered | — |
| a component seen twice in the walk is collected once | `if (false) return` | 476 / 0 | **gap** | `a component reached twice in one walk is emitted once` |
| an instance seen twice in the walk is collected once | `if (false) return` | 475 / 1 | covered | — |
| a collected component keeps its walk order | `void id` | 469 / 7 | covered | — |
| a collected instance keeps its walk order | `void id` | 470 / 6 | covered | — |
| a truncated forest walk marks the result truncated | `void walked` | 475 / 1 | covered | — |
| the component loop stops once the emission ceiling is hit | `if (component !== undefined && !emission.pushComponent(component)) continue ⏎ } catch` | 476 / 0 | **gap** | — *(open)* |
| a read error raised while serializing a component ends the call | `if (false) { ⏎ throw error ⏎ } ⏎ } ⏎ }` | 476 / 0 | **gap** | `a read error raised while serializing a component ends the whole call` |
| main components are looked up sixteen at a time | `const lookupBatch = 1` | 476 / 0 | **gap** | `main components are looked up sixteen at a time, and the pass stops at the ceiling` |
| the instance pass stops once the emission ceiling is hit | `if (false) break ⏎ if (Date.now() - budgetStarted >= budgetMs) { ⏎ emission.mark({ ⏎ reason: "nodeLimit",` | 476 / 0 | **gap** | `main components are looked up sixteen at a time, and the pass stops at the ceiling` |
| an instance with a main-component lookup is resolved | `const lookup = instance.getMainComponentAsync ⏎ if (typeof lookup === "function") return undefined` | 470 / 6 | covered | — |
| a main-component lookup that never settles is skipped | `const main = await (lookup.call(instance) as Promise<unknown>)` | 474 / 2 | covered | — |
| an instance relationship names the main component's id | `const componentId = "x"` | 470 / 6 | covered | — |
| an instance relationship names the instance | `return { instanceId: "", componentId }` | 471 / 5 | covered | — |
| the instance batch stops once the emission ceiling is hit | `if (!emission.pushInstance(item)) continue` | 476 / 0 | **gap** | — *(open)* |
| a main component already found in the walk is not looked up again | `const seen = new Set<string>()` | 473 / 3 | covered | — |
| a main component referenced twice is looked up once | `if (id.length === 0) continue ⏎ seen.add(id)` | 473 / 3 | covered | — |
| an off-page main component is looked up by id | `void id` | 475 / 1 | covered | — |
| an off-page lookup that never settles is skipped | `const node = await lookup.call(figma, id)` | 476 / 0 | **gap** | `an off-page main component that never resolves is skipped, not awaited` |
| an off-page main component is serialized into components | `const component = undefined as ComponentDefinition \| undefined ⏎ void raw ⏎ if (component !== undefined && !e…` | 475 / 1 | covered | — |
| the result carries the collected components | `components: [],` | 465 / 11 | covered | — |
| the result carries the instance relationships | `instances: [],` | 471 / 5 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 473 / 3 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 474 / 2 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `components and instances share one returned-node ceiling` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 469 / 7 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | ComponentEmission reads only returnedNodes and encodedBytes |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 467 / 9 | covered | — |
| the instance pass stops before a batch once the budget is out | `if (false) { ⏎ emission.mark({ ⏎ reason: "nodeLimit", ⏎ visitedNodes: emission.considered, ⏎ }) ⏎ …` | 476 / 0 | **gap** | `an exhausted main-component budget stops before the first batch` |
| the instance pass stops after a batch once the budget is out | `if (false) { ⏎ emission.mark({ ⏎ reason: "nodeLimit", ⏎ visitedNodes: emission.considered, ⏎ }) ⏎ …` | 475 / 1 | covered | — |
| the off-page pass stops before a batch once the budget is out | `if (false) { ⏎ emission.mark({ reason: "nodeLimit", visitedNodes: emission.considered }) ⏎ break ⏎ } ⏎ co…` | 476 / 0 | **gap** | `the off-page pass stops before its batch when the budget is already out` |
| the off-page pass stops after a batch once the budget is out | `if (false) { ⏎ emission.mark({ reason: "nodeLimit", visitedNodes: emission.considered }) ⏎ break ⏎ } ⏎ } ⏎ ⏎ …` | 476 / 0 | **gap** | `the off-page pass stops after a batch that ran the budget out` |
| the main-component budget runs from the start of the instance pass | `const budgetStarted = Date.now() + 1e9` | 475 / 1 | covered | — |
| the off-page pass stops once the emission ceiling is hit | `if (false) break ⏎ if (Date.now() - budgetStarted >= budgetMs) { ⏎ emission.mark({ reason: "nodeLimit", visited…` | 476 / 0 | **gap** | `the off-page pass stops once the emission ceiling is hit` |
| a read error raised by a main-component lookup ends the call | `if (false) { ⏎ throw error ⏎ }` | 476 / 0 | **gap** | `a read error thrown synchronously by a main-component lookup ends the whole call` |
| an off-page lookup that throws costs only that component | `} catch (e) { ⏎ throw e ⏎ }` | 476 / 0 | **gap** | `an off-page lookup that throws costs only that component` |
| a component set whose variantProperties throws still yields a component | `} catch (e) { ⏎ throw e ⏎ } ⏎ } ⏎ ⏎ function propertyDefinitions` | 476 / 0 | **gap** | `a component set whose variantProperties cannot be enumerated is still emitted` |
| a property definition with a readable default is emitted | `if (defaultValue !== undefined) continue` | 475 / 1 | covered | — |
| an off-page component the host returns is used | `return undefined` | 475 / 1 | covered | — |
| cancellation is checked at a batch boundary in the instance pass | `signal?.throwIfAborted() ⏎ if (emission.emitTruncation !== undefined) break ⏎ if (Date.now() - budgetStarted >= b…` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the off-page pass | `signal?.throwIfAborted() ⏎ if (emission.emitTruncation !== undefined) break ⏎ if (Date.now() - budgetStarted >= b…` | 476 / 0 | **gap** | — *(open)* |
| the instance pass checks cancellation on every batch | `if (emission.emitTruncation !== undefined) break ⏎ if (Date.now() - budgetStarted >= budgetMs) { ⏎ emission.mar…` | 476 / 0 | **gap** | — *(open)* |
| the component loop checks cancellation on every component | `try { ⏎ const component = serializeComponent(components.get(id))` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/dev-mode.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a truncated walk outranks the emission cut | `return this.emitTruncation ?? this.walkTruncation` | 476 / 0 | **gap** | `the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total` |
| the first walk truncation wins | `this.walkTruncation = truncation` | 476 / 0 | **gap** | — *(open)* |
| the returned-item ceiling is inclusive | `if (this.items.length >= this.limits.returnedNodes + 1) {` | 476 / 0 | **gap** | `the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total` |
| the item ceiling is reported as a nodeLimit truncation | `this.emitTruncation = { reason: "byteLimit", visitedNodes }` | 476 / 0 | **gap** | `the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total` |
| emitted bytes accumulate across items | `const encoded = byteLength(value)` | 476 / 0 | **gap** | `the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total` |
| the encoded-byte ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total` |
| a byteLimit truncation reports the encoded size | `this.emitTruncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `the item ceiling is inclusive, the walk outranks it, and the byte ceiling reports its total` |
| a node with content reaches the result | `void value` | 466 / 10 | covered | — |
| an emitted dev-mode record is a success item | `this.items.push({ status: "error", value } as never)` | 466 / 10 | covered | — |
| the annotation catalogue is read through figma.annotations | `const reader = undefined` | 474 / 2 | covered | — |
| the catalogue reader is invoked on figma.annotations | `for (const raw of await reader.call(undefined)) {` | 476 / 0 | **gap** | `the category reader is called on figma.annotations` |
| an annotation category carries its label | `categories.push({ id, label: "" })` | 475 / 1 | covered | — |
| an annotation category carries its id | `categories.push({ id: id + "!", label })` | 475 / 1 | covered | — |
| an annotation's label becomes its text | `typeof annotation.label === "string" ⏎ ? ""` | 474 / 2 | covered | — |
| an annotation with only markdown falls back to it | `: typeof annotation.labelMarkdown === "string" ⏎ ? ""` | 475 / 1 | covered | — |
| an annotation keeps the host's own id | `typeof annotation.id === "string" && annotation.id.length > 0 ⏎ ? annotation.id + "!"` | 476 / 0 | **gap** | `an annotation keeps the host's own id when it has one` |
| an annotation with no id is numbered by position | `: `${nodeId}:annotation`` | 474 / 2 | covered | — |
| a generated annotation id is scoped to its node | `: `annotation:${index}`` | 474 / 2 | covered | — |
| an annotation carries its category id | `void annotation` | 475 / 1 | covered | — |
| a category referenced twice is listed once | `if (true) {` | 476 / 0 | **gap** | — *(open)* |
| a referenced category id is collected | `void annotation` | 475 / 1 | covered | — |
| an annotation reaches the node record | `void value` | 473 / 3 | covered | — |
| a documentation link carries its label as its name | `name: "",` | 475 / 1 | covered | — |
| a documentation link carries its uri | `name: typeof link.label === "string" ? link.label : "", ⏎ uri: "", ⏎ })` | 475 / 1 | covered | — |
| a node exposing getDevResourcesAsync has it called | `if (typeof raw === "function") return { resources: [] }` | 474 / 2 | covered | — |
| a dev resource's uri comes from the host's url field | `const uri = string(resource.uri)` | 474 / 2 | covered | — |
| a dev resource carries its name | `name: "", ⏎ uri,` | 475 / 1 | covered | — |
| a dev resource reaches the node record | `void resource` | 475 / 1 | covered | — |
| a dev resource's inherited node id is reported | `void resource` | 475 / 1 | covered | — |
| the first inherited node id wins | `inheritedFromNodeId !== undefined &&` | 475 / 1 | covered | — |
| an inherited node id leaves the dev-resource pass | `return { resources }` | 475 / 1 | covered | — |
| a node referencing a category gets that category | `if (categoryIds.length >= 0) return []` | 475 / 1 | covered | — |
| only the categories a node references are attached | `return catalog` | 476 / 0 | **gap** | `only the categories a node references are attached, once each` |
| a node carrying an annotations field has it read | `const annotated = hasHostField(node, "annotations_x")` | 472 / 4 | covered | — |
| a node carrying documentationLinks has it read | `const docs = hasHostField(node, "documentationLinks_x")` | 475 / 1 | covered | — |
| a node's dev resources are fetched | `const linked = await devResources(undefined)` | 474 / 2 | covered | — |
| a dev-mode record names its node | `nodeId: nodeId + "!", ⏎ annotations: annotated.annotations,` | 467 / 9 | covered | — |
| a dev-mode record carries its annotations | `annotations: [],` | 473 / 3 | covered | — |
| a dev-mode record carries the categories it references | `annotationCategories: [],` | 475 / 1 | covered | — |
| a dev-mode record carries its documentation links | `documentation: [],` | 475 / 1 | covered | — |
| a dev-mode record carries its dev resources | `devResources: [],` | 475 / 1 | covered | — |
| a dev-mode record carries its description | `if (false) value.description = node.description as string` | 471 / 5 | covered | — |
| a dev-mode record carries its markdown description | `if (false) { ⏎ value.descriptionMarkdown = node.descriptionMarkdown as string ⏎ }` | 475 / 1 | covered | — |
| a dev-mode record carries its owner node id | `if (false) { ⏎ value.ownerNodeId = node.ownerNodeId as string ⏎ }` | 475 / 1 | covered | — |
| a node's own inheritedFromNodeId is carried | `void node ⏎ } else if` | 475 / 1 | covered | — |
| a dev resource's inherited node id fills in when the node has none | `} else if (false) { ⏎ value.inheritedFromNodeId = linked.inheritedFromNodeId as string ⏎ }` | 475 / 1 | covered | — |
| a node with an annotation is content | `false \|\|` | 474 / 2 | covered | — |
| a node with an annotation category is content | `false \|\|` | 476 / 0 | **gap** | — *(open)* |
| a node with a dev resource is content | `false \|\|` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| a node with a documentation link is content | `false \|\|` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| a node with a non-empty description is content | `false \|\|` | 472 / 4 | covered | — |
| a node with a non-empty markdown description is content | `false \|\|` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| a node with an owner node id is content | `false \|\|` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| a node with an inherited node id is content | `false` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| the annotation catalogue is loaded before the walk | `const catalog: AnnotationCategory[] = []` | 474 / 2 | covered | — |
| the read honours the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 474 / 2 | covered | — |
| a truncated forest walk marks the result truncated | `void walked` | 475 / 1 | covered | — |
| every walked node counts toward visitedNodes | `visitedNodes += 0` | 473 / 3 | covered | — |
| a node with nothing to report is left out of items | `if (value === undefined) continue` | 472 / 4 | covered | — |
| the item loop stops once the emission ceiling is hit | `if (!emission.push(value, visitedNodes)) continue` | 476 / 0 | **gap** | — *(open)* |
| the result carries the emitted items | `items: [],` | 466 / 10 | covered | — |
| the result reports how many nodes were inspected | `visitedNodes: 0, ⏎ truncated:` | 473 / 3 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 475 / 1 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 475 / 1 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | dev-mode's ItemEmission reads only returnedNodes and encodedBytes |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 467 / 9 | covered | — |
| a catalogue entry with an id is kept | `if (id.length >= 0) continue ⏎ categories.push` | 475 / 1 | covered | — |
| a catalogue reader that throws costs only the catalogue | `} catch (e) { ⏎ throw e ⏎ } ⏎ } ⏎ ⏎ function annotations` | 475 / 1 | covered | — |
| a dev-resource reader that throws costs only that node's resources | `} catch (e) { ⏎ throw e ⏎ }` | 475 / 1 | covered | — |
| cancellation is checked at a batch boundary in the dev-mode item loop | delete it | 476 / 0 | **gap** | — *(open)* |
| the dev-mode item loop checks cancellation on every item | `visitedNodes += 1` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/reactions.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| the ON_CLICK trigger is reported as click | `case "ON_CLICK": ⏎ return undefined` | 466 / 10 | covered | — |
| the ON_DRAG trigger is reported as drag | `case "ON_DRAG": ⏎ return undefined` | 475 / 1 | covered | — |
| the ON_HOVER trigger is reported as hover | `case "ON_HOVER": ⏎ return undefined` | 474 / 2 | covered | — |
| the ON_PRESS trigger is reported as press | `case "ON_PRESS": ⏎ return undefined` | 475 / 1 | covered | — |
| the ON_KEY_DOWN trigger is reported as keyDown | `case "ON_KEY_DOWN": ⏎ return undefined` | 474 / 2 | covered | — |
| the AFTER_TIMEOUT trigger is reported as afterDelay | `case "AFTER_TIMEOUT": ⏎ return undefined` | 473 / 3 | covered | — |
| the MOUSE_ENTER trigger is reported as mouseEnter | `case "MOUSE_ENTER": ⏎ return undefined` | 475 / 1 | covered | — |
| the MOUSE_LEAVE trigger is reported as mouseLeave | `case "MOUSE_LEAVE": ⏎ return undefined` | 476 / 0 | **gap** | `a mouseLeave trigger carries its delay and a URL action its link` |
| the MOUSE_UP trigger is reported as mouseUp | `case "MOUSE_UP": ⏎ return undefined` | 475 / 1 | covered | — |
| the MOUSE_DOWN trigger is reported as mouseDown | `case "MOUSE_DOWN": ⏎ return undefined` | 475 / 1 | covered | — |
| the ON_MEDIA_HIT trigger is reported as mediaHit | `case "ON_MEDIA_HIT": ⏎ return undefined` | 474 / 2 | covered | — |
| the ON_MEDIA_END trigger is reported as mediaEnd | `case "ON_MEDIA_END": ⏎ return undefined` | 475 / 1 | covered | — |
| the NAVIGATE navigation is reported as navigate | `case "NAVIGATE": ⏎ return undefined` | 472 / 4 | covered | — |
| the OVERLAY navigation is reported as openOverlay | `case "OVERLAY": ⏎ return undefined` | 473 / 3 | covered | — |
| the SWAP navigation is reported as swapOverlay | `case "SWAP": ⏎ return undefined` | 474 / 2 | covered | — |
| the CHANGE_TO navigation is reported as changeTo | `case "CHANGE_TO": ⏎ return undefined` | 475 / 1 | covered | — |
| the SCROLL_TO navigation is reported as scrollTo | `case "SCROLL_TO": ⏎ return undefined` | 475 / 1 | covered | — |
| the PLAY media action is reported as play | `case "PLAY": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the PAUSE media action is reported as pause | `case "PAUSE": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the TOGGLE_PLAY_PAUSE media action is reported as togglePlayPause | `case "TOGGLE_PLAY_PAUSE": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the MUTE media action is reported as mute | `case "MUTE": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the UNMUTE media action is reported as unmute | `case "UNMUTE": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the TOGGLE_MUTE_UNMUTE media action is reported as toggleMuteUnmute | `case "TOGGLE_MUTE_UNMUTE": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the SKIP_FORWARD media action is reported as skipForward | `case "SKIP_FORWARD": ⏎ return undefined` | 475 / 1 | covered | — |
| the SKIP_BACKWARD media action is reported as skipBackward | `case "SKIP_BACKWARD": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the SKIP_TO media action is reported as skipTo | `case "SKIP_TO": ⏎ return undefined` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| the CENTER overlay position is reported as center | `case "CENTER": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| the TOP_LEFT overlay position is reported as topLeft | `case "TOP_LEFT": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| the TOP_CENTER overlay position is reported as topCenter | `case "TOP_CENTER": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| the TOP_RIGHT overlay position is reported as topRight | `case "TOP_RIGHT": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| the BOTTOM_LEFT overlay position is reported as bottomLeft | `case "BOTTOM_LEFT": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| the BOTTOM_CENTER overlay position is reported as bottomCenter | `case "BOTTOM_CENTER": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| the BOTTOM_RIGHT overlay position is reported as bottomRight | `case "BOTTOM_RIGHT": ⏎ return undefined` | 475 / 1 | covered | — |
| the MANUAL overlay position is reported as manual | `case "MANUAL": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| a NONE overlay background is reported as none | `case "NONE": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| a SOLID_COLOR overlay background is reported | `case "SOLID_COLOR_X": {` | 475 / 1 | covered | — |
| a solid overlay background carries its colour | `: { type: "solidColor", color: { r: 0, g: 0, b: 0, a: 0 } }` | 475 / 1 | covered | — |
| a NONE background interaction is reported as none | `case "NONE": ⏎ return undefined` | 476 / 0 | **gap** | `every overlay position, background and interaction the protocol names is reported` |
| a CLOSE_ON_CLICK_OUTSIDE background interaction is reported | `case "CLOSE_ON_CLICK_OUTSIDE": ⏎ return undefined` | 475 / 1 | covered | — |
| a truncated walk outranks the emission cut | `return this.emitTruncation ?? this.walkTruncation` | 476 / 0 | **gap** | `a truncated walk outranks the emission cut, and the byte ceiling reports its total` |
| the first walk truncation wins | `this.walkTruncation = truncation` | 476 / 0 | **gap** | — *(open)* |
| the returned-item ceiling is inclusive | `if (this.items.length >= this.limits.returnedNodes + 1) {` | 475 / 1 | covered | — |
| the item ceiling is reported as a nodeLimit truncation | `this.emitTruncation = { reason: "byteLimit", visitedNodes }` | 475 / 1 | covered | — |
| emitted bytes accumulate across items | `const encoded = byteLength(value)` | 476 / 0 | **gap** | `a truncated walk outranks the emission cut, and the byte ceiling reports its total` |
| the encoded-byte ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `a truncated walk outranks the emission cut, and the byte ceiling reports its total` |
| a byteLimit truncation reports the encoded size | `this.emitTruncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `a truncated walk outranks the emission cut, and the byte ceiling reports its total` |
| a node with reactions reaches the result | `void value` | 463 / 13 | covered | — |
| a trigger is read from the host trigger's type field | `const trigger = undefined` | 463 / 13 | covered | — |
| a destination action keeps its own action type | `} = { type: "navigate" }` | 472 / 4 | covered | — |
| a destination action carries the destination id | `if (false) { ⏎ action.destinationId = destinationId as string ⏎ }` | 471 / 5 | covered | — |
| a BACK action is reported as back | `case "BACK": ⏎ return undefined` | 469 / 7 | covered | — |
| a CLOSE action is reported as closeOverlay | `case "CLOSE": ⏎ return undefined` | 473 / 3 | covered | — |
| a SET_VARIABLE action carries its variable id | `? { type: "setVariable" }` | 475 / 1 | covered | — |
| a SET_VARIABLE action is reported as setVariable | `case "SET_VARIABLE_XX": {` | 475 / 1 | covered | — |
| a SET_VARIABLE_MODE action carries its collection id | `if (false) result.variableCollectionId = collectionId` | 475 / 1 | covered | — |
| a SET_VARIABLE_MODE action carries its mode id | `if (false) result.variableModeId = modeId` | 475 / 1 | covered | — |
| a SET_VARIABLE_MODE action is reported as setVariableMode | `case "SET_VARIABLE_MODE_X": {` | 475 / 1 | covered | — |
| a CONDITIONAL action is reported as conditional | `case "CONDITIONAL": ⏎ return undefined` | 475 / 1 | covered | — |
| a media-runtime action carries the media action it performs | `const result: ReactionAction = { type: "updateMediaRuntime", mediaAction: "play" }` | 474 / 2 | covered | — |
| a media-runtime action carries the media node it targets | `const destinationId = record(raw).destinationId ⏎ if (false) { ⏎ result.destinationId = destinationId as st…` | 475 / 1 | covered | — |
| a media-runtime action carries its skip amount | `if (false) result.amountToSkip = amount as number` | 475 / 1 | covered | — |
| a media-runtime action carries its new timestamp | `if (false) result.newTimestamp = timestamp as number` | 476 / 0 | **gap** | `every media runtime action the protocol names is reported` |
| a URL action carries the link it opens | `return uri.length === 0 ? undefined : { type: "openLink", uri: "" }` | 476 / 0 | **gap** | `a mouseLeave trigger carries its delay and a URL action its link` |
| a URL action reads the host's url field | `const uri = string(action.uri)` | 475 / 1 | covered | — |
| a NODE action carries the destination the host named | `: destinationAction(type, undefined)` | 471 / 5 | covered | — |
| a destination is looked up by the id the action names | `if (figma.getNodeByIdAsync === undefined) return undefined ⏎ const node = await figma.getNodeByIdAsync("")` | 472 / 4 | covered | — |
| a destination the host returns is reported accessible | `return undefined` | 472 / 4 | covered | — |
| an action with no destination node is still accessible | `case "closeOverlay": ⏎ case "back": ⏎ case "openLink": ⏎ return { accessible: false, node: undefined }` | 474 / 2 | covered | — |
| a variable or conditional action is reported not accessible | `case "setVariable": ⏎ case "setVariableMode": ⏎ case "conditional": ⏎ return { accessible: true, node: undefi…` | 475 / 1 | covered | — |
| the resolved destination node reaches the overlay reader | `return { accessible: node !== undefined, node: undefined }` | 475 / 1 | covered | — |
| a relative overlay position carries its point | `return x === undefined \|\| y === undefined ? undefined : { x: 0, y: 0 }` | 474 / 2 | covered | — |
| an overlay carries its relative position | `const position = relativePosition(record(actionRaw).overlayRelativePosition) ⏎ if (false) overlay.relativePosition = …` | 474 / 2 | covered | — |
| an openOverlay action reads the destination's overlay settings | `if (action.type === "openOverlay_x" \|\| action.type === "swapOverlay") {` | 475 / 1 | covered | — |
| a swapOverlay action reads the destination's overlay settings | `if (action.type === "openOverlay" \|\| action.type === "swapOverlay_x") {` | 475 / 1 | covered | — |
| an overlay carries its position type | `if (false) overlay.positionType = positionType` | 475 / 1 | covered | — |
| an overlay carries its background | `if (false) overlay.background = background` | 475 / 1 | covered | — |
| an overlay carries its background interaction | `if (false) overlay.backgroundInteraction = interaction` | 475 / 1 | covered | — |
| a reaction's actions array is read | `if (false) return [...(reaction.actions as unknown[])]` | 463 / 13 | covered | — |
| a reaction carrying a single action field is read | `if (false) return [reaction.action]` | 475 / 1 | covered | — |
| a recognised trigger yields a reaction | `if (trigger !== undefined) continue` | 463 / 13 | covered | — |
| a recognised action yields a reaction | `if (action !== undefined) continue` | 463 / 13 | covered | — |
| a reaction carries its trigger | `trigger: "click", ⏎ action, ⏎ destinationAccessible: destination.accessible,` | 471 / 5 | covered | — |
| a reaction carries its action | `action: { type: "back" }, ⏎ destinationAccessible: destination.accessible,` | 468 / 8 | covered | — |
| a reaction reports whether its destination was reachable | `destinationAccessible: true,` | 473 / 3 | covered | — |
| an afterDelay trigger carries its timeout | `if (false) {` | 473 / 3 | covered | — |
| the timeout is the host's own value | `if (false) reaction.timeout = timeout as number` | 473 / 3 | covered | — |
| a mouseEnter trigger carries its delay | `false \|\|` | 475 / 1 | covered | — |
| a mouseLeave trigger carries its delay | `false \|\|` | 476 / 0 | **gap** | `a mouseLeave trigger carries its delay and a URL action its link` |
| a mouseUp trigger carries its delay | `false \|\|` | 475 / 1 | covered | — |
| a mouseDown trigger carries its delay | `false` | 475 / 1 | covered | — |
| the delay is the host's own value | `if (false) reaction.delay = delay as number` | 473 / 3 | covered | — |
| a keyDown trigger carries its key codes and device | `if (false) {` | 474 / 2 | covered | — |
| a keyDown trigger carries its key codes | `if (false) { ⏎ reaction.keyCodes = (triggerRaw.keyCodes as number[]).filter(` | 474 / 2 | covered | — |
| a keyDown trigger carries its device | `void triggerRaw` | 474 / 2 | covered | — |
| a mediaHit trigger carries its hit time | `if (false) {` | 474 / 2 | covered | — |
| a mediaHit trigger prefers the host's mediaHitTime | `if (false) reaction.mediaHitTime = mediaHitTime as number` | 475 / 1 | covered | — |
| a mediaHit trigger falls back to the host's timestamp | `else if (false) reaction.mediaHitTime = timestamp as number` | 475 / 1 | covered | — |
| a reaction reads its action's transition | `const transition = record(undefined)` | 473 / 3 | covered | — |
| a reaction carries its transition type | `void transition` | 473 / 3 | covered | — |
| a reaction carries its transition duration | `if (false) reaction.transitionDuration = duration as number` | 474 / 2 | covered | — |
| a reaction carries its overlay settings | `if (false) reaction.overlay = overlay as never` | 474 / 2 | covered | — |
| a serialized reaction reaches the node record | `void reaction` | 463 / 13 | covered | — |
| a reactions record names its node | `nodeId: nodeId + "!", ⏎ reactions:` | 466 / 10 | covered | — |
| a node carrying a reactions field has it read | `reactions: hasHostField(node, "reactions_x")` | 463 / 13 | covered | — |
| a node's reactions are serialized | `? []` | 463 / 13 | covered | — |
| a node with no reactions is left out of items | `return true` | 472 / 4 | covered | — |
| the read honours the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 471 / 5 | covered | — |
| a truncated forest walk marks the result truncated | `void walked` | 475 / 1 | covered | — |
| every walked node counts toward visitedNodes | `visitedNodes += 0` | 473 / 3 | covered | — |
| the item loop stops once the emission ceiling is hit | `if (!emission.push(value, visitedNodes)) continue` | 476 / 0 | **gap** | — *(open)* |
| the result carries the emitted items | `items: [],` | 463 / 13 | covered | — |
| the result reports how many nodes were inspected | `visitedNodes: 0, ⏎ truncated:` | 473 / 3 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 475 / 1 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 476 / 0 | **gap** | `two reacting nodes are both emitted under the default ceilings` |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `two reacting nodes are both emitted under the default ceilings` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 476 / 0 | **gap** | `two reacting nodes are both emitted under the default ceilings`, `a truncated walk outranks the emission cut, and the byte ceiling reports its total` |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | reactions' ItemEmission reads only returnedNodes and encodedBytes |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 465 / 11 | covered | — |
| cancellation is checked at a batch boundary in the reactions item loop | delete it | 476 / 0 | **gap** | — *(open)* |
| the reactions item loop checks cancellation on every item | `visitedNodes += 1` | 476 / 0 | **gap** | — *(open)* |

**`plugin/src/read/fonts.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| mixed text is scanned 2048 characters at a time | `export const FONT_SEGMENT_RANGE = 1` | 475 / 1 | covered | — |
| the font key distinguishes two styles of one family | `return `${font.family}`` | 473 / 3 | covered | — |
| the font key distinguishes one style across families | `return `${font.style}`` | 475 / 1 | covered | — |
| a font name needs both family and style | `if (typeof raw.family !== "string" && typeof raw.style !== "string") {` | 476 / 0 | **gap** | `a font needs both halves, and one empty half is still a font` |
| a font with an empty style but a real family is kept | `if (raw.family.length === 0 \|\| raw.style.length === 0) return undefined` | 476 / 0 | **gap** | `a font needs both halves, and one empty half is still a font` |
| a font name carries its family | `return { family: "", style: raw.style }` | 469 / 7 | covered | — |
| a font name carries its style | `return { family: raw.family, style: "" }` | 469 / 7 | covered | — |
| the host's mixed sentinel routes a node to the segment scan | `return false` | 474 / 2 | covered | — |
| a newly seen font records the node that uses it | `nodeIds: [],` | 468 / 8 | covered | — |
| a newly seen font remembers that node so it is not listed twice | `seen: new Set<string>(),` | 475 / 1 | covered | — |
| a node using one font twice is listed once | `if (existing.nodeIds.length >= MAX_INPUT_IDS) { ⏎ return ⏎ }` | 475 / 1 | covered | — |
| the per-font node-id ceiling is inclusive | `if (existing.seen.has(nodeId) \|\| existing.nodeIds.length >= MAX_INPUT_IDS + 1) {` | 476 / 0 | **gap** | `a font's node list stops at MAX_INPUT_IDS` |
| a second node using a known font joins its node list | `void nodeId` | 475 / 1 | covered | — |
| a usage with a node id is recorded | `const font = readFontName(raw) ⏎ if (font === undefined \|\| nodeId.length >= 0) return` | 468 / 8 | covered | — |
| the segment scan spans the node's characters | `const length = 0` | 474 / 2 | covered | — |
| the last segment stops at the end of the text | `const end = start + FONT_SEGMENT_RANGE` | 475 / 1 | covered | — |
| each segment window is passed to the host reader | `segments = reader.call(node, ["fontName"], 0, 0)` | 474 / 2 | covered | — |
| the segment reader is asked for fontName | `segments = reader.call(node, [], start, end)` | 475 / 1 | covered | — |
| a window the host refuses costs only that window | `} catch (e) { ⏎ throw e ⏎ }` | 476 / 0 | **gap** | `a segment window the host refuses costs only that window` |
| each segment's font is recorded as a usage | `addUsage(collected, undefined, nodeId)` | 474 / 2 | covered | — |
| a TEXT node's fonts are collected | `if (node.type !== "TEXT_X") return` | 468 / 8 | covered | — |
| a mixed-font node is scanned segment by segment | `if (isMixed(node.fontName)) { ⏎ return ⏎ }` | 474 / 2 | covered | — |
| a single-font node's fontName is recorded | `addUsage(collected, undefined, id)` | 470 / 6 | covered | — |
| a host exposing the font catalogue has it read | `const list = figma.listAvailableFontsAsync ⏎ if (typeof list === "function") return undefined` | 470 / 6 | covered | — |
| the catalogue reader is invoked on figma | `const fonts = await list.call(undefined)` | 476 / 0 | **gap** | `the catalogue is read off figma itself, and a catalogue that throws leaves availability unknown` |
| a catalogue entry's font is read from its fontName field | `const font = readFontName(item)` | 470 / 6 | covered | — |
| a catalogue entry joins the observed set | `if (false) catalog.add(fontKey(font as FontName))` | 470 / 6 | covered | — |
| a catalogue reader that throws leaves availability unknown | `} catch (e) { ⏎ throw e ⏎ }` | 476 / 0 | **gap** | `the catalogue is read off figma itself, and a catalogue that throws leaves availability unknown` |
| an unobservable catalogue yields unknown availability | `if (catalog === undefined) return "unavailable"` | 475 / 1 | covered | — |
| a font in the catalogue is reported available | `return "unavailable"` | 470 / 6 | covered | — |
| a font absent from the catalogue is reported unavailable | `return "available"` | 475 / 1 | covered | — |
| a truncated walk outranks the emission cut | `return this.emitTruncation ?? this.walkTruncation` | 476 / 0 | **gap** | `the font ceiling, the walk cut and the byte ceiling each report their own total` |
| the first walk truncation wins | `this.walkTruncation = truncation` | 476 / 0 | **gap** | — *(open)* |
| every considered font counts toward visitedNodes | `this.considered += 0` | 476 / 0 | **gap** | `the font ceiling, the walk cut and the byte ceiling each report their own total` |
| the returned-font ceiling is inclusive | `if (this.fonts.length >= this.limits.returnedNodes + 1) {` | 476 / 0 | **gap** | `the font ceiling, the walk cut and the byte ceiling each report their own total` |
| emitted bytes accumulate across fonts | `const encoded = byteLength(usage)` | 476 / 0 | **gap** | `the font ceiling, the walk cut and the byte ceiling each report their own total` |
| the encoded-byte ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the font ceiling, the walk cut and the byte ceiling each report their own total` |
| a byteLimit truncation reports the encoded size | `this.emitTruncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `the font ceiling, the walk cut and the byte ceiling each report their own total` |
| an accepted font usage reaches the result | `void usage` | 468 / 8 | covered | — |
| the read honours the caller's selector | `const { roots } = await resolveDesignRoots(undefined, signal)` | 474 / 2 | covered | — |
| each walked node is scanned for fonts | `void raw` | 468 / 8 | covered | — |
| a truncated forest walk marks the result truncated | `void walked` | 475 / 1 | covered | — |
| the observed catalogue decides availability | `const catalog = undefined as Set<string> \| undefined` | 470 / 6 | covered | — |
| a font usage carries the font name | `font: { family: "", style: "" },` | 469 / 7 | covered | — |
| a font usage carries its availability | `availability: "unknown",` | 470 / 6 | covered | — |
| a font usage lists the nodes that use it | `nodeIds: [],` | 468 / 8 | covered | — |
| the font loop stops once the emission ceiling is hit | `) { ⏎ continue ⏎ }` | 476 / 0 | **gap** | — *(open)* |
| the result carries the collected fonts | `fonts: [],` | 468 / 8 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 475 / 1 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `a font needs both halves, and one empty half is still a font` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 472 / 4 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | not applicable | FontEmission reads only returnedNodes and encodedBytes |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 469 / 7 | covered | — |
| the cancellation batch counter advances with the walk | `index += 0` | 476 / 0 | **gap** | — *(open)* |
| fonts are emitted in first-seen order | `for (const [key, usage] of [...collected].reverse()) {` | 472 / 4 | covered | — |
| cancellation is checked at a batch boundary in the font walk | delete it | 476 / 0 | **gap** | — *(open)* |
