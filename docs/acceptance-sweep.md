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

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

## tests/integration

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

## tests/policy

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

## plugin ui and main

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

## plugin read

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|
