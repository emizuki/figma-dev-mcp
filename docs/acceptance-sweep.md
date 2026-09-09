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

**One rule, everywhere.** `not applicable` means only what the line above says.
A mutation that compiles, runs and leaves the suite green is a **gap**, whatever
the reason it stayed green; a gap for which no test was added stays **open**, and
the reason is given in that section's prose as a fact about the *production
code*, never as a claim about what a test could do (Ruling 21). Fourteen rows
once carried `not applicable` under a second, unstated rule — "no input can tell
this mutation apart" — and have been reclassified: **three to `covered`**, in
`tests/policy`, once a mutation was found on the production constant rather than
on the test's own literal; and **eleven to `gap`, open**, in `plugin ui and main`.
**No row in this file carries `not applicable`**, so the verdict survives as a
definition and not as a bucket.

**Tense.** A sentence here saying a mutation "leaves the suite green" records
what that mutation measured, against the baseline its own section states. Where
the row was a **gap**, the test named in its last column now catches it, and the
sentence is a description of the tree before this sweep — not a claim about the
tree it leaves behind. Three passages that read the other way were corrected in
the final round; every remaining production-change recommendation was re-probed
against the finished tree.

Citing code: name the test and quote its assertion message, or name the
production function — a `file.rs:NNN` is the one claim in this record no
mutation checks, and a line number into a test file is worse still, because
nothing says whether it means the `assert!` or the message five lines inside
it. Keep a number only where it points into production code and adds something
the name does not; every one that survives has been checked against the tree.

## Scope

Enumerated by the Step-1 commands in
`docs/superpowers/plans/2026-09-07-acceptance-sweep.md`, **measured at
`85960df`** — the commit this branch starts from, before any of its tests
existed. Each figure counts *test-function names*, not accept paths: a Rust
`fn` line or a plugin `test(`/`it(` line whose name matches the refusal regex,
minus (**refusal-only**) or intersected with (**already paired**) the
acceptance regex. Counts are regex-dependent and are a starting point, not an
authority — the row counts in the sections below come from the production code,
not from here.

Re-running the same commands on the **finished tree** gives higher figures in
five of the eight rows, because the sweep's own ~170 added tests match the
refusal regex too. Both columns are given so the table is checkable rather than
merely stated; every figure below was re-derived by running those commands
against each tree.

| Area | refusal-only at `85960df` | already paired at `85960df` | refusal-only, finished tree |
|---|---|---|---|
| tests/contracts | 15 | 11 | 16 |
| tests/integration | 16 | 4 | 16 |
| tests/policy | 8 | 0 | 9 |
| crates | 5 | 1 | 5 |
| plugin/src/read | 72 | — | 93 |
| plugin/src/shared | 11 | — | 11 |
| plugin/src/main | 5 | — | 10 |
| plugin/src/ui | 24 | — | 28 |

(`already paired` is unchanged in all four Rust areas: 11, 4, 0, 1.)

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
  Re-probed against the finished tree: turning `insert`'s
  `next > MAX_ENVELOPE_BYTES` into `>=` still leaves the workspace at
  **279 / 0**.

### A real bug: `deferred.rs`'s two implementations of one ceiling are a byte apart

Raised, **deliberately not fixed** — this sweep adds tests and does not change
production code. It is written down here because the coverage gap above is the
reason nobody would notice it, and because a bug recorded only outside the
repository is a bug nobody finds after merge.

`DeferredObject` counts the same object twice, and the two counts disagree by
exactly one byte.

- **`insert` estimates.** It starts at `2` (for `{}`) and adds
  `name.len() + 4 + value.len()` per field. For *n* fields that is
  `2 + 4n + Σname + Σvalue`.
- **`decode` encodes.** It writes `{`, then per field `"` + name + `":` + value,
  a `,` before every field but the first, then `}` — which is
  `2 + 4n − 1 + Σname + Σvalue`.

**The estimate over-counts the real encoding by exactly 1 byte for every field
count ≥ 1** (0 for the empty object). Measured rather than reasoned: simulating
both formulas for n = 1…5 gives a delta of 1 every time — estimate
17/32/47/62/77 against actual 16/31/46/61/76.

Two consequences, both live:

1. **The inclusive ceiling behaves exclusively.** `insert` refuses when
   `estimate > MAX_ENVELOPE_BYTES`, which is exactly `actual ≥
   MAX_ENVELOPE_BYTES`. A deferred object encoding to *precisely*
   `MAX_ENVELOPE_BYTES` is refused. That is this record's single most common
   finding class, in production rather than in a test.
2. **`decode`'s own guard is unreachable.** Having passed `insert`, the buffer
   `decode` builds is at most `MAX_ENVELOPE_BYTES − 1`, so
   `encoded.len() > MAX_ENVELOPE_BYTES` never fires. Its `>` / `>=` distinction
   is invisible from outside for that reason, not because the ceiling is
   untested.

Nothing in the suite observes any of this: mutating all three guards to `>=`
leaves the workspace green, which is the row above. A fix belongs in its own
change — either make `insert` account `name.len() + 3` for the first field and
`+ 4` thereafter, or drop the estimate and measure the encoded buffer once —
and it needs the accept-side test the row above already names as missing.

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
- `Broker::shutdown` is two statements, and at the branch point only the
  first was pinned: deleting the second left the suite green, because the
  first causes socket teardown to resolve the same pendings with the same
  error code. This one was **not** reachable from this section's enumeration.
  It is pinned now — re-probed against the finished tree, deleting
  `self.state.pending.lock().await.shutdown()` gives **278 / 1**, the one red
  being `ws_origin::broker_shutdown_resolves_pending_calls_itself_not_by_socket_teardown`,
  the test this row added.

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
| `Broker::shutdown` fails every outstanding call itself, rather than leaving them to socket teardown | delete `self.state.pending.lock().await.shutdown();` from `Broker::shutdown` (`lib.rs`) | 267 / 0 | **gap** — a whole production statement that was removable with the suite green when measured; see below | `ws_origin::broker_shutdown_resolves_pending_calls_itself_not_by_socket_teardown` |

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

Rows: 42. Verdicts: 18 gap, 24 covered, 0 not applicable (18 + 24 = 42).
Tests added: 10 — nine answering the 18 gaps, plus one new scan that answers a
question Task 4 left open rather than a row in this table.

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| `TOOL_NAMES` is exactly the fourteen, in order | rename two entries in the production constant (`protocol/src/lib.rs`), breaking both the equality and the sort | 270 / 9 | covered — `allowlists::mvp_tool_allowlist_is_sorted_closed_and_exact` among them | — |
| `PROMPT_NAMES` is exactly the three, in order | the same, on the three-element constant | 265 / 14 | covered — `allowlists::mvp_prompt_allowlist_is_sorted_closed_and_exact` among them | — |
| `extracted_tool_references` picks backticked snake-case verbs out of a sample | `extracted_tool_references` never inserts, so it always returns the empty set (`prompts/src/validation.rs`) | 276 / 3 | covered — `prompts::tool_reference_extraction_is_limited_to_backticked_snake_case_verbs` among them | — |
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

**Three rows were recorded `not applicable` and are now `covered`.** The two
allowlist tests and the extraction test assert equality against a literal — a
fourteen-element array, a three-element array, a five-element set built from a
literal sample — so there is no *input* to empty, and the first pass concluded
from that there was no mutation to run. That conclusion was aimed at the wrong
side. The mutation belongs on the **production constant**, not on the test's
own literal: renaming two entries of `TOOL_NAMES` in `protocol/src/lib.rs`
takes the workspace to 270 / 9 with
`allowlists::mvp_tool_allowlist_is_sorted_closed_and_exact` among the red;
the same on `PROMPT_NAMES` gives 265 / 14 with its allowlist test red; and
making `extracted_tool_references` never insert gives 276 / 3 with
`prompts::tool_reference_extraction_is_limited_to_backticked_snake_case_verbs`
red. All three are covered, and the observation the old note was reaching for
survives without the verdict: an assertion that carries its own expected value
is the one shape in this suite that cannot pass vacuously, which is why these
three needed no new test.

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

**136 covered, 174 gaps, 0 not applicable.** 136 + 174 = 310. By half:
`main` 94 rows — 35 covered, 59 gaps; `ui` 216 rows — 101 covered, 115 gaps;
59 + 115 = 174. (Eleven of those gaps — 7 in `main`, 4 in `ui` — were recorded
`not applicable` until the final round; see *Eleven mutations nothing here
distinguishes* below.) The new tests close **152**
of the 174 gaps and **22** stay open; 152 + 22 = 174. Eleven of the open
twenty-two are the reclassified rows; the other eleven were open already.

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

`ui/relay.ts` has 23 rows: **21 gaps**, 2 covered. The two
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

What was *not* covered is the cancel arriving as a message. When measured,
`registry.cancel` in `dispatchControllerMessage`'s `cancel` case could be
deleted, and the arm could answer with an error envelope instead of `null`,
with the suite green both times: the covering test reached into the registry
directly. The new `a cancel message aborts the request it names and answers
nothing` sends the cancel the way the socket does, and it holds — re-probed
against the finished tree, deleting `registry.cancel` gives **644 / 1**, the
one red being that test.

### `throwIfAbortedAtBatch` was pinned by nothing

**It is pinned now, and it must not be deleted.** An earlier draft of this
section carried the heading "`throwIfAbortedAtBatch` can be deleted outright".
That recommendation is **disproven** — see *What is left open* in the
`plugin read` section, where `fonts.ts:230` turns out to be load-bearing: with
that one call gone, `getFonts` returns a result to a caller who cancelled.
Re-probed against the finished tree, emptying the helper's body gives
**643 / 2**, the two red being this section's
`the batch cancellation gate throws on a batch boundary and only there` and
Task 7's `get_fonts > the walk visitor checks cancellation before it collects a
node`. A reader acting on the old heading would delete a live cancellation gate
and silently retire two tests. **No production change is recommended here.**

`throwIfAbortedAtBatch(signal, index, batchSize)` is called from **fifteen
sites** across `plugin/src/read` — `dev-mode.ts` ×1, `components.ts` ×2,
`fonts.ts` ×1, `styles.ts` ×2, `motion.ts` ×1, `reactions.ts` ×1,
`variables.ts` ×2, `serialize.ts` ×5. When measured, emptying its body — so it
never threw at any index — left the suite at **413 / 0**. Neither half was
pinned, nor was the complementary property that it does *not* throw between
boundaries.

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

### Eleven mutations nothing here distinguishes

**These eleven rows are `gap`s that stay open.** They were recorded
`not applicable` for four rounds, and none is the "does not compile" case the
vocabulary was written for — `bun run build` strips types, so a type-invalid
mutation still bundles. They were filed under a second, unstated rule: *no input
can tell this mutation apart*. That rule is retired. It is a negative claim of
exactly the class the `plugin read` section examined **fourteen times and found
wrong fourteen times**, and it was applied here to eleven rows that compile,
run, and leave the suite green — which is the definition of a gap. Under
Ruling 21 each is now a gap with no test, marked open, and what follows states
what the *production code* does and claims nothing about what a test could
reach.

Where a branch is described as unreached, that was measured rather than read:
the branch was replaced with a `throw` and the suite, including the new tests,
stayed at 476 / 0, so nothing **in the suite as it then stood** reaches it.

- `createProgressReporter` has **three defensive lines no caller in this tree
  reaches**, each measured with a planted `throw` rather than read. `due()`'s
  `!Number.isFinite(lastEmitAt)` clause never decides anything, because on the
  only tick where `lastEmitAt` is `NaN` the `lastPhase !== phase` clause is
  already true. The scheduled heartbeat's
  `if (lastPhase === undefined) lastPhase = phase` and `emitLatest`'s
  `lastPhase ?? "reading"` are unreached for the same reason:
  `startHeartbeat` ticks before it schedules, and that tick always assigns
  `lastPhase`. Reported to whoever owns `progress.ts`; the three rows stay open
  here.
- `due()`'s `if (intervalMs <= 0) return true` is reached but decides nothing:
  the comparison after it, `now() - lastEmitAt >= intervalMs`, is true whenever
  `intervalMs <= 0` and the clock does not run backwards. Deleting the clause
  leaves the suite at 476 / 0 with the new `a non-positive interval emits every
  tick of the same phase` test in place.
- `LocalCancellationSignal.abort()`'s `if (this.#aborted) return` guards a
  second pass that has nothing to do: the first abort clears the listener set,
  and `addEventListener` refuses to add to an aborted signal.
- `code.ts`'s `inbound` returning the original string when `JSON.parse` throws,
  and the `return` after `completeScreenshotValidation`, hand on a value that
  both `parseControllerMetadataRequest` and `parseControllerBoundMessage`
  refuse, so the handler falls out silently either way.
- `asBytes`'s `value instanceof Uint8Array` arm is subsumed by the
  `ArrayBuffer.isView(value)` arm two lines below, which handles a `Uint8Array`
  identically. Deleting the first arm alone leaves the suite green; deleting
  both turns three of the new relay tests red, which is what establishes that
  the view arm is doing the work.
- `readUint32`'s big-endian assembly agrees with the mutated one across the
  range its caller admits. Shifting the first byte by 16 instead of 24 changes the
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

Twenty-two gaps stay open, marked `— (open)` in the table: the eleven below,
plus the eleven reclassified out of `not applicable` in the final round (see
*Eleven mutations nothing here distinguishes*). All twenty-two are **not
probed**. Each reason below is a fact about the production code; none of them
claims a test could not reach the row — this area's sibling section examined
fourteen such claims and found fourteen of them wrong.

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
  every DOM a real parser produces, and no fixture here separates them.
- **A default whose observable is three seconds away.** `createProgressReporter`
  falling back to `Date.now`: the mutation replaces it with `() => 0`, and the
  difference shows only in a comparison against a 3,000 ms interval. A test was
  attempted and abandoned over the cost of a real three-second wait, not
  because none was found. (`P24`.)

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
| a second `abort()` does not notify again | delete the `if (this.#aborted) return` guard | 413 / 0 | **gap** | — *(open)* |
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
| the very first tick emits even inside the interval | drop `!Number.isFinite(lastEmitAt)` from `due()` | 413 / 0 | **gap** | — *(open)* |
| a tick after the interval has elapsed emits | return `false` from the elapsed-interval comparison | 412 / 1 | covered | — |
| with a non-positive interval every tick emits | delete the `intervalMs <= 0` short-circuit | 413 / 0 | **gap** | — *(open)* |
| a repeat tick inside the interval updates counts without emitting | delete `if (!due(phase)) return` | 411 / 2 | covered | — |
| a frame carries `total` when the tick supplied one | delete `if (total !== undefined) frame.total = toU32(total)` | 410 / 3 | covered | — |
| a tick without a total drops the previous one | keep the previous total when `nextTotal` is undefined | 413 / 0 | **gap** | `a tick without a total drops the total the previous tick carried` |
| an emit records when it happened, so the interval can run | delete `lastEmitAt = now()` | 411 / 2 | covered | — |
| an `emit` that throws does not fail the read | call `options.emit(frame)` outside the `try` | 413 / 0 | **gap** | `an emit that throws does not fail the read it is reporting on` |
| `startHeartbeat` emits a frame at once | delete the immediate `tick(phase, completed, total)` | 412 / 1 | covered | — |
| `startHeartbeat` stops the timer a previous one left | delete `if (stopTimer !== undefined) stopTimer()` | 413 / 0 | **gap** | `restarting the heartbeat stops the timer the previous one left running` |
| `startHeartbeat` schedules the repeat | never call `schedule(...)` | 412 / 1 | covered | — |
| each heartbeat tick emits the latest frame | delete `emitLatest()` from the scheduled callback | 412 / 1 | covered | — |
| a heartbeat with no phase yet adopts the one it was started with | delete `if (lastPhase === undefined) lastPhase = phase` | 413 / 0 | **gap** | — *(open)* |
| `stopHeartbeat` cancels the timer | delete `stopTimer?.()` | 412 / 1 | covered | — |
| a reporter with no interval uses `PROGRESS_INTERVAL_MS` | default `intervalMs` to `1` | 413 / 0 | **gap** | `the default interval is the inactivity-safe one, not something shorter` |
| the default heartbeat repeats rather than firing once | `setTimeout`/`clearTimeout` in place of `setInterval`/`clearInterval` | 413 / 0 | **gap** | `the default heartbeat repeats instead of firing once` |
| a frame with no phase yet is labelled `reading` | default the frame message to `encoding` | 413 / 0 | **gap** | — *(open)* |
| a reporter with no clock uses `Date.now` | default `now` to `() => 0` | 413 / 0 | **gap** | — *(open)* |

**`plugin/src/main/code.ts`**

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
| a message delivered as a JSON string is parsed | return the string unparsed from `inbound` | 413 / 0 | **gap** | `accepts a readiness request in each transport shape the host uses` |
| a string that is not JSON is passed on unchanged | return `null` from `inbound`'s catch | 413 / 0 | **gap** | — *(open)* |
| a `{ pluginMessage }` envelope is unwrapped | return the envelope itself | 413 / 0 | **gap** | `accepts a readiness request in each transport shape the host uses` |
| a plain object is passed through untouched | return `null` from `inbound`'s fall-through | 413 / 0 | **gap** | `answers a readiness request with the file's own identity`, `accepts a readiness request in each transport shape the host uses`, `dispatches a well-formed request and posts the response back` |
| a settled screenshot validation ends the handler | drop the `return` after `completeScreenshotValidation` | 413 / 0 | **gap** | — *(open)* |
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
| export bytes arriving as a `Uint8Array` are taken | delete the `Uint8Array` arm of `asBytes` | 413 / 0 | **gap** | — *(open)* |
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
| a PNG dimension is assembled big-endian | shift the first byte by 16 instead of 24 | 413 / 0 | **gap** | — *(open)* |
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
| a buffer exactly as long as the magic is long enough | `<` becomes `<=` | 413 / 0 | **gap** | — *(open)* |

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
| the data: payload starts at the first comma after `data:` | start the search one character later | 413 / 0 | **gap** | — *(open)* |
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
serializer and one shared visibility predicate, **5,910 lines of production
code** against 314 tests. (`figma-api-conformance.ts`'s 180 lines are excluded
from that denominator for the same reason they are excluded from the
enumeration: nothing imports the file. See the maintenance edges below.)

It groups into **1,380 accept paths**, one row per mutation that answers
exactly one row. Every mutation below was applied to production code, built,
measured and reverted; the committed diff adds tests only. Baseline before this
task: **476 pass / 0 fail / 2,100 expect() calls / 28 files**. After: **645 pass
/ 0 fail / 2,495 expect() calls / 28 files**.

**950 covered, 430 gaps, 0 not applicable.** 950 + 430 = 1,380. Of the 430
gaps, **377 are closed** by the new tests and **53 stay open**; 377 + 53 = 430.
The diff adds 170 test names, one of which replaces a test that asserted
nothing, so the suite grows by 169.

Twenty-seven of those rows were found after the baseline runs — see *How this
enumeration went wrong* — and their Suite result is measured against the suite
as it stood when they were found (600 / 0, 615 / 0 or 632 / 0) rather than
against 476 / 0. The split file says which.

**On `not applicable`.** This section uses the file's stated definition and
nothing wider, and that rule now governs the whole document (see *One rule,
everywhere* at the top). **No row here meets it.** An earlier draft used it for
eleven rows of live-but-unread code — dead fields, a set member consulted after
an earlier `return`, a context field no walker reads — and that was a second,
unstated rule: structurally identical code elsewhere (the batch cancellation
checks, `PASS_THROUGH` and `NORMAL` in `BLEND_MODES`) was recorded as a gap. A
row no test was found for is uniformly a **gap that stays open**, marked
`not probed`, so the three-way split follows from one rule a reader can apply —
and, as `fonts.ts:230` shows, from a rule that does not quietly retire a row
before someone has aimed at it. The same repair was applied to the fourteen
rows in `tests/policy` and `plugin ui and main` that still carried the second
rule; that was the last thing this branch fixed, and it is the same lesson one
level up — **a correction is only as complete as the sweep that applies it.**

By file — rows, then covered / gap, then of the gaps closed / open:

| File | Rows | Covered | Gap | Closed | Open |
|---|---|---|---|---|---|
| `serialize.ts` | 387 | 251 | 136 | 125 | 11 |
| `motion.ts` | 154 | 111 | 43 | 39 | 4 |
| `reactions.ts` | 124 | 91 | 33 | 30 | 3 |
| `variables.ts` | 106 | 74 | 32 | 22 | 10 |
| `render.ts` | 100 | 64 | 36 | 31 | 5 |
| `navigation.ts` | 104 | 78 | 26 | 26 | 0 |
| `search.ts` | 91 | 62 | 29 | 28 | 1 |
| `components.ts` | 87 | 59 | 28 | 24 | 4 |
| `dev-mode.ts` | 74 | 52 | 22 | 17 | 5 |
| `styles.ts` | 69 | 47 | 22 | 16 | 6 |
| `fonts.ts` | 60 | 40 | 20 | 16 | 4 |
| `common.ts` | 14 | 13 | 1 | 1 | 0 |
| `visibility.ts` | 10 | 8 | 2 | 2 | 0 |

387 + 154 + 124 + 106 + 100 + 104 + 91 + 87 + 74 + 69 + 60 + 14 + 10 = 1,380.

The Scope table's **72** is right, and worth pinning down because the brief's
own Step-1 command disagrees with it: run against the tree at `f65c29e`, the
plan's command — which subtracts an acceptance-word filter — finds exactly 72
refusal-named tests in `plugin/src/read`, and the brief's, which does not, finds
84. Both count the same 314 tests, and neither is the row count: the rows come
from the production code.

**The rule this area cost the branch, next to the existing one.** The record's
standing rule is *"an enumeration must expand its indirections before anything is
counted."* It did not prevent the defect below, because nothing here was
indirected — the sites simply were not all found. The sharper rule: **an
enumeration must be driven from the production side and reconciled against the
table, never assembled from the table's own coverage.** All three enumeration
defects on this branch — Task 2's ceiling constants, Task 3's shared field list,
and this area's cancellation checks — were found by someone counting the code,
never by anyone reading the record. What that reconciliation now looks like here
is in *How this enumeration went wrong*.

The Mutation column shows the replacement text rather than a prose description
of it; `⏎` stands for a newline. It is not self-sufficient: 54 mutation texts
repeat within a single file, covering 197 rows — the largest class being the
bare `delete it` used for lookup-table members — and for those the Accept path
is what identifies the site. No Accept path is duplicated within a file, so the
two columns together identify every row.

The per-file tables live in
[`acceptance-sweep-plugin-read.md`](./acceptance-sweep-plugin-read.md); the
prose and the summary above stay here.

### The cancellation question, answered for the read side

Task 6 measured that the ten `checks cancellation …` read tests stay green when
`throwIfAbortedAtBatch` is emptied at all fifteen call sites, and left the
question here. Deleting each of those fifteen calls one at a time is `B01`–`B15`:
all fifteen stay green, confirming that measurement site by site.

Nine of the fifteen are immediately followed by an unconditional
`signal?.throwIfAborted()` on the same signal at the same index, and five by a
call whose first statement is one — the 9 + 5 = 14 partition set out below.
`fonts.ts:230` is the fifteenth: it sits inside a walk visitor, with
`walkNode`'s poll *before* the visitor rather than after, which is not the same
thing as being subsumed by it, as the rest of this section works out.
`components.ts:339`'s twin is `B32`, found later.

Deleting fifteen unconditional
checks one at a time is `B16`–`B30`: **fourteen of the fifteen stay green**
against the suite as it stood. The single exception is `serializeNode`'s own
check, `B20`, which goes red against the pre-existing `bounded node serializer >
checks cancellation between child batches`.

(`B16`–`B30` is not exactly "the twins of the fifteen batch calls", and saying so
was part of how the census defect below stayed hidden: `variables.ts:503` and
`components.ts:259` are in that set and sit beside no batch call, while
`components.ts:339`, which does, was missing from it.)

**That measurement was right and the conclusion first drawn from it was
wrong.** An earlier draft of this section said none of the twenty-nine could be
closed from the test side. It is false for the fourteen twins, because
`throwIfAbortedAtBatch` is **index-gated** —
`if (index % batchSize === 0)` with `batchSize` 100 (`CANCEL_CHECK_BATCH`). A
fixture that aborts at an index *not* divisible by 100, inside a loop shorter
than 100 iterations, leaves the batched check looking at nothing. The twin is
then the only guard, and deleting it lets the read run to completion.

All fourteen are now closed by a test that aborts between batch boundaries, and
so are nine of the thirteen further checks the census below turned up. Three of
the first fifteen needed a second, sharper fixture, and what they needed is the
useful part of the finding:

- **`B27`, the variable lookup loop.** The next statement is
  `session.lookupVariable(...)`, whose own first line polls the same signal, so
  an ordinary two-id fixture is caught either way. The test now has the host
  answer `null` for both ids — so no collection is grouped and no later loop
  runs — and delays the first lookup past the read's budget, so without the twin
  the second pass `break`s before reaching any other check.
- **`B30`, the definition loop.** `readDefinition` calls `lookupCollection`,
  which polls first. The test now makes the second variable id-less, because
  `readDefinition` returns on an empty id *before* the collection lookup.
- **`B29`, the component loop.** The first probe aborted from a `parent` getter
  and passed — for the wrong reason: `visibilityOf` reads `parent` during the
  walk, so the read rejected before the loop under test was ever entered. It
  aborts from `variantProperties` now, which only `serializeComponent` reads.

**The whole construct, counted from the production side.** `plugin/src/read`
has **35** unconditional `signal?.throwIfAborted()` sites and **15**
`throwIfAbortedAtBatch` calls. Every one of the 50 now has a row. Of the 35
unconditional checks: **4 were already covered** — `serializeNode`'s
(`B20`), the search walk's (`B41`), `lookupCollection`'s (`B43`) and the one
after each screenshot encode (`R88`) — and **31 are gaps this sweep closed**.
4 + 31 = 35; none stays open. Of the **15** batch calls, one — `fonts.ts:230`
— is a gap this sweep closed, and 14 stay open.

So the read area polls cancellation at 50 places, of which **36 are
individually load-bearing and individually pinned** and 14 stay open.

**The recommendation this section used to carry is disproven.** Through three
rounds it said `throwIfAbortedAtBatch` was subsumed at all fifteen of its call
sites, and that the helper and its fifteen calls could therefore be deleted
"without losing a single detection". `fonts.ts:230` is load-bearing.
`walkNode` polls the signal, then reports progress, then calls the visitor
(`serialize.ts:1144`, `:1156`, `:1157`), so an abort raised from a progress
reporter lands between the poll and the gate; and on a host with no
`listAvailableFontsAsync` and a childless page, nothing polls afterwards.
Delete the gate and `getFonts` **returns a `GetFontsResult` to a caller who
cancelled**, with the whole committed suite green. A reader acting on the old
recommendation would have deleted a live check. **No production change is
recommended here.**

The other fourteen have a different shape from `fonts.ts:230`, read at their
sites: each is immediately followed by an unconditional
`signal?.throwIfAborted()` on the same signal at the same index — `styles.ts`
`:256`, `:288`; `components.ts` `:278`, `:338`; `dev-mode.ts:290`;
`motion.ts:674`; `reactions.ts:484`; `variables.ts` `:456`, `:477` — or by a
call whose first statement is one: `serialize.ts:1043` → `serializeNode`,
`:1165` → `walkNode`, and `:1210`, `:1261`, `:1318` → each pre-pass's own
`visit`. `fonts.ts:230` was the one call site with neither, and it is the one
that turned out to matter. The fourteen are **not probed**; 9 + 5 = 14.

**Four resisted a first probe. All four were the probe's fault.**
That correction is the most useful thing in this section, so it is written out
rather than summarised:

- **`B40` was closable.** The first probe fed a pre-aborted signal to a scope
  that *resolves*, so the search walk's own poll answered. Point it at a scope
  that does **not** resolve and nothing else is reached: without this poll the
  read answers `PAGE_NOT_FOUND` to a caller who cancelled. Closed by
  `resolving a search scope checks cancellation before it can fail`.
- **`B42` was closable.** The first probe aborted before `getVariables` even
  called `resolveDesignRoots`, whose own poll answered. Nothing polls between
  `readDefinition`'s collection lookup and the alias hop's variable lookup, so
  an abort raised on the mode entry in between is seen by that lookup alone:
  without it the read returns a *resolved alias* to a caller who cancelled.
  Closed by `a variable lookup checks cancellation before it starts`.

- **`B34` and `B35` were closable, and the reason they looked otherwise is the
  most useful thing on this branch.** Both are the `loadDocumentPages` polls.
  Three probes were written for them and all three measured the same thing:
  delete either poll and the read *still cancels*, because
  `serializeNodeForest` runs on every path and `serializeNode`'s first
  statement is a poll (`serialize.ts:936`). That measurement is correct and it
  is not the sweep's question. **The question a mutation answers is "would a
  test go red?", not "does the read still cancel?"** The two diverge exactly
  when a poll's effect is *how much host work happens after the abort* — which
  is what a cancellation poll is for. Asked the right question both rows close
  at once:

  - `navigation.ts:134` deleted → a **second page is loaded** from the host
    after the caller cancelled. Fixture: one `DOCUMENT` root via a `nodeIds`
    scope with two `PAGE` children, the first page's `loadAsync` raising the
    abort; the read rejects either way, and `loads` is `["0:p1"]` intact and
    `["0:p1", "0:p2"]` mutated. Closed by
    `loading document pages checks cancellation before each page is loaded`.
  - `navigation.ts:130` deleted → a **second document's `children` is read**
    after the caller cancelled. Fixture: two `DOCUMENT` roots with counting
    `children` getters, the first root's only page raising the abort;
    `loadDocumentPages` reads the getter twice per root, so the assertion
    filters to the second document, which is read `[]` times intact and twice
    mutated. Closed by
    `loading document pages checks cancellation before each document root`.

  Each fails alone under its own deletion — 644 pass / 1 fail, the one failure
  being its own test.

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
the iframe — were reached by nothing. Twenty-five of `render.ts`'s thirty-six
gaps are in that path: the message type, the correlation id, the item payload,
both codec halves in both directions, the reply handler, and every field that
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
  `PASS_THROUGH` and `NORMAL`: `blendMode()` returns `undefined` both for a
  mode missing from the table and for those two. **Not probed** (`Z070`,
  `Z071`).
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

The `visitedNodes` field of those limits is a seven-times-repeated dead field:
all seven emission classes accept it in their `SerializerLimits` and none reads
it. Those seven rows are open gaps, alongside four other dead fields — the two
byte counters `createUiCodec` fills in that `encodeAsset` never reads,
`VARIABLE_ALIAS` in motion's easing set, which `motionEasing` returns before
consulting, and the forest walk context's `depth`, which `walkNode` never reads.

### What is left open

Fifty-three rows, in six kinds.

**The rule this section is written under.** Nine negative claims about this
area had been examined by the third review and **nine were wrong**: four
impossibility claims disproved by runs; one more, `R98`, reasoned from a
pre-flight and never probed; and the four probes the evidence rule actually
produced — `B40`, `B42`, `B34`, `B35` — every one of them aimed wrong. (An
earlier draft of this paragraph called `R98` a fifth probe. It was not; the
rule produced four, and all four failed. Counting an unprobed claim as a probe
flatters the rule, which is the one thing the tally exists to prevent.) This
round examined five more claims and all five were wrong too:

1. *`throwIfAbortedAtBatch` is subsumed at all fifteen of its call sites* —
   `fonts.ts:230` is load-bearing.
2. *`visibility.ts`'s cycle watch and walk bound change step count and nothing
   else* — step count is host reads, and host reads are assertable.
3. *The six `break`/`continue` rows return byte-identical output* — true of the
   payload, and beside the point for `components.ts:262`, which reads a third
   component from the host after the ceiling.
4. and 5. *`M76` and `M88` are shapes no real host produces* — both close
   through the ordinary `get_motion` entry point and harness, on a
   deliberately discriminating payload (an indexed item carrying `tracks` but
   no `timelineDuration`; a collection map carrying a keyframe binding's own
   fields) that is not claimed to be realistic host output.

**Fourteen examined, fourteen wrong.** At that rate the honest posture is not a
better probe standard; it is abstention. So: **every row here that could not be
closed is recorded `not probed`. No row asserts, implies or hedges that a test
could not reach it.** What each kind still says is what the *production code*
does — the next reader needs that to aim a probe — and it stops there.

The diagnosis behind rounds three and four is worth carrying out of this
section, because it is not about diligence: **the question a mutation answers
is "would a test go red?", not "does the read still cancel?"** Three careful
probes measured the second question for `B34` and `B35` and got a correct
answer to the wrong question. The two diverge whenever a check's effect is *how
much host work happens after it* — a cancellation poll, a `break` at a ceiling,
a classification that stops a scan. That class is assertable with a counting
host getter, and this record had already used the technique (`visitedNodes` for
`dev-mode`/`motion`/`reactions`, `harness.lookedUp` in `render.test.ts`)
without recognising it as the general answer.

- **Batch cancellation calls (14).** Fourteen of the fifteen
  `throwIfAbortedAtBatch` calls. Each is immediately followed by an
  unconditional `signal?.throwIfAborted()` on the same signal at the same index
  (`styles.ts` `:256`, `:288`; `components.ts` `:278`, `:338`;
  `dev-mode.ts:290`; `motion.ts:674`; `reactions.ts:484`; `variables.ts`
  `:456`, `:477`), or by a call whose first statement is one (`serialize.ts`
  `:1043` → `serializeNode`, `:1165` → `walkNode`, `:1210`, `:1261`, `:1318` →
  each pre-pass's own `visit`). The fifteenth, `fonts.ts:230`, had neither and
  is closed. **Not probed.**
- **Eleven dead fields (11).** The seven `visitedNodes` limits, `decodedBytes`
  and `base64Bytes`, motion's `VARIABLE_ALIAS` easing entry, and the walk
  context's `depth` — each written, and no read of it found anywhere in the
  call graph. **Not probed.**
- **A guard whose twin fires first (5).** `resolve`'s
  `if (this.budget.exhausted) return EXHAUSTED` runs only if the deadline
  passes between two consecutive synchronous clock reads with no `await`
  between them (`V37`); `lookupVariable`'s budget gate is read again by every
  caller before it (`V24`); a second `depthLimit` is identical to the first,
  since `appliedDepth` is a constant of the read (`Z459`); and `blendMode()`
  answers `undefined` both for a mode missing from the table and for
  `PASS_THROUGH` / `NORMAL` (`Z070`, `Z071`). **Not probed.** This kind lost
  three members: `R98` — a `getScreenshot` row in `render.ts`, whose per-item
  capability arm the pre-flight was said to make unreachable, when in fact the
  host is read *twice* — and `Z699` / `Z700`, the two visibility cycle guards,
  closed together by counting `parent` reads: 65 for a self-cycle, not the
  1024 the bound allows.
- **A `break` whose `continue` leaves the payload byte-identical (5).**
  `the instance batch stops at the emission ceiling`, `the font loop stops at
  the emission ceiling`, `the local pass stops at the node ceiling rather than
  considering the rest`, and `the collection loop stops at both ceilings rather
  than considering the rest` (two rows). In each, the truncation latches its
  count at the first refusal, so the emitted result is byte-identical under the
  mutation; the probes are committed. Four of the original nine were closable
  and are closed: `dev-mode`, `motion` and `reactions` return `visitedNodes` in
  the result, so `continue` keeps counting where `break` stops, and
  `components.ts:262` reads a third component from the host with the `break`
  turned into a `continue`.
- **The first-truncation-wins guard, called once (5).** `mark()` keeps the
  first walk truncation, and in five readers it is called exactly once per
  read. `components.ts` is the exception — its budget marks a second time — and
  that row is closed. **Not probed.**
- **A shape no fixture here has produced, or bookkeeping the next line
  discards (13).** `styles.ts`'s `isMixedStyleId` compares a style id against
  `figma.mixed` behind a `typeof value !== "string"` gate, and the harness's
  `figma.mixed` is a symbol (`ST22`). `byteLength`'s lone-surrogate branch needs
  a lone surrogate to survive `JSON.stringify` — **measured, not reasoned**:
  `JSON.stringify("\ud800")` on this runtime returns the eight ASCII characters
  `"\ud800"` (`Z463`). A settled screenshot validation clears its timeout and
  unhooks its abort listener (`R22`, `R23`) — both act on an id already removed
  from the pending map — and a cancelled validation's `CANCELLED` item (`R35`)
  is produced and then discarded by the `signal?.throwIfAborted()` on the next
  line. `SE58`, `D20`, `D48`, `V30`, `V36`, `V43`, `Z270` and `F56` are the same
  shape. **Not probed** apart from `Z463`. Two members left this kind this
  round: `M76`, where an indexed item declaring `tracks` is classified as a
  keyframe binding and the classification is what stops the scan of its
  `properties`; and `M88`, where `fills`, `strokes` and `effects` are read by
  the indexed pass and so skipped by the plain property scan. Both show up as
  an extra binding in the emitted list. Their fixtures go through the ordinary
  `get_motion` entry point and the standard harness, but the payload shapes are
  chosen to discriminate rather than to model a host — the claim they pin is
  about the classification, not about what Figma emits.

14 + 11 + 5 + 5 + 5 + 13 = 53.

The narrowing of `not applicable` above rests on this partition: under a wider
rule of "unobservable by construction", kinds 1, 2, 3 and 5 — **35 of the 53** —
would move, and the verdict would stop distinguishing much. The wider rule would
also have been wrong on the merits, in every kind it touches and one it does
not: four of kind 4's original nine rows were observable all along, and so were
three of kind 1's seventeen, three of kind 3's eight and two of kind 6's
fifteen.

### Two maintenance edges

- **`plugin/src/read/figma-api-conformance.ts` is imported by nothing.** It
  appears only in the `exclude` lists of `tsconfig.tests.json` and
  `tsconfig.controller-tests.json`. It is 180 lines and was not enumerated,
  because a file no build or test reaches has no accept path to mutate. Whoever
  owns it should know it is unreferenced.
- **Seven `expect(PluginReadError).toBeDefined()` lines.** They appear in
  `components`, `motion`, `search`, `dev-mode`, `reactions`, `styles` and
  `variables` — five at the end of a cancellation test, and two (`search`,
  `styles`) at the end of a selector-refusal test. Each asserts that an imported
  class is defined, which cannot fail; they keep the import used and add seven
  to the `expect()` count without pinning anything. They were left alone — the
  tests they sit in do work, through the `rejects.toThrow` above them.

### How this enumeration went wrong, and what it cost

Three times. Two were found by reading; the third — much the largest — by
counting the production side, which is where the rule at the top of this section
comes from.

**Four mislabelled rows and two duplicates.** The table generator matched each
lookup-table member by its own source line. `  CENTER: "center",` appears in
five tables in `serialize.ts`, so one call for `STROKE_ALIGNS` produced five
rows: one correct and four carrying the stroke-align label while mutating
`TEXT_ALIGN_HORIZONTALS`, `TEXT_ALIGN_VERTICALS`, `AXIS_ALIGNS` and
`CONSTRAINT_AXES`. Two of the four duplicated rows generated later for the same
members. Every mutation was distinct and correct; only the Accept path column
lied. Two rows were relabelled, two dropped, and a check that no Accept path
repeats within one file now runs over the whole enumeration — it also caught
`N46` and `N74`, two different unresolved-node sites in `navigation.ts` given one
label.

**Sixteen missing cancellation rows.** The enumeration of cancellation checks was
assembled from the *batch call sites* and their neighbours, which is the wrong
direction: a check that stands alone has no batch call to be found from.
`fonts.ts:238` surfaced first, as `B31`, by asking the question of one more
reader. Counting instead — every `signal?.throwIfAborted()` in the area, matched
against the rows — found **15 more**: `navigation.ts` ×6, `search.ts` ×2,
`variables.ts` ×2, `serialize.ts` ×3, `components.ts` ×1, `fonts.ts` ×1. Of the
sixteen, **fourteen were gaps and two were already covered** (`search.ts:401`
and `variables.ts:212`) — so both the gap count and the covered count were
understated. `search.ts` had 88 rows and not one
cancellation row, which is the shape of absence a table cannot show you.

**The reconciliation that found it, run over every countable construct.** Each
construct is counted in production, then every occurrence is checked to fall
inside the line span of at least one enumerated row:

| Construct | Sites | Rows short, before | After |
|---|---|---|---|
| unconditional `signal?.throwIfAborted()` | 35 | 16 | 0 |
| `throwIfAbortedAtBatch` calls | 15 | 0 | 0 |
| `switch` case arms | 94 | 3 | 0 |
| `try` / `catch` blocks | 27 | 8 | 0 |
| lookup-table members | 46 | 0 | 0 |
| `settleOrSkip` calls | 6 | 0 | 0 |
| `hostGet` / `hasHostField` calls | 70 | 0 | 0 |
| `loadPageIfNeeded` calls | 8 | 0 | 0 |
| `resolveDesignRoots` calls | 9 | 0 | 0 |
| `byteLength` calls | 12 | 0 | 0 |
| progress `tick` calls | 5 | 0 | 0 |

Two more constructs were under-enumerated and are now complete: three
`reactions.ts` action arms had rows for what they *carry* but none for the arm
itself (`UPDATE_MEDIA_RUNTIME`, `URL`, `NODE`), and eight `try`/`catch` blocks
whose accept path is "a hostile host costs this field and no more" had no row at
all — `hostGet` in both `components.ts` and `serialize.ts`, the two
`Object.entries` guards, `getStyledTextSegments`, `textCharacters`,
`valueModeIds` and `VariableSession.settle`. Six of those eleven turned out
covered; five were gaps and all five are closed.

Four residuals in the reconciliation are not missing rows and are named so nobody
re-derives them: the `} catch` lines of `components.ts:301`, `navigation.ts:316`,
`render.ts:351` and `search.ts:308`, whose behaviour is rowed by the rethrow
guard *inside* them (`C76`, `N65`, `R77`) or by the parse conditions above them
(`SE86`–`SE88`).

### Verification

`cd plugin && bun run build && bun run test` — **645 pass / 0 fail / 2,495
expect() calls / 28 files**, from a baseline of 476 / 0 / 2,100 / 28.
`bun run format:check` and `bun run typecheck` clean. No Rust file was touched.

Every mutation was applied by a driver that copies the file first, applies one
exact string replacement, builds, runs the whole suite, restores from the copy
and compares SHA-256 against the pre-mutation value; a run that does not restore
byte for byte aborts the batch. `git status --porcelain --untracked-files=all`
was empty before every commit.

Each of the 377 closed rows was re-measured with its mutation re-applied after
the test that closes it was in place, and the test named in the last column is
the one the run reported failing — the attribution is measured, not asserted.
The 53 open rows were re-measured against the finished suite and are green
there.
