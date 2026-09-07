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
`response_accounting.rs`, group into 22 accept paths — a group being what one
mutation answers. Every mutation below was applied to production code, measured,
and reverted; the committed diff adds tests only. Baseline before this task:
**255 passed / 0 failed**. After: **259 passed / 0 failed**.

The worked example the plan cites — renaming `unresolved` so the decoder no
longer recognises the wire key — is row 1, and it comes back **covered**: the
fix for that defect landed with `the_forest_results_decode_an_unresolved_entry_and_keep_it`,
which is exactly the test the brief proposed writing. It is redundant now.

All four gaps are the same shape, and it is not the shape the plan expected: not
a renamed wire key, but an **inclusive ceiling turned exclusive**. Every one is a
single character. In each case the refusal at `limit + 1` was pinned and the
acceptance at `limit` was not, so the suite could not tell an inclusive ceiling
from an exclusive one — and `limit` is the one value each ceiling exists to admit.

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
| A frame declaring exactly `MAX_ENVELOPE_BYTES` gets past the length check | `validate_body_length`: `length > MAX_ENVELOPE_BYTES` → `>=` | 255 / 0 | **gap** | `a_frame_declaring_exactly_the_envelope_ceiling_is_not_refused_for_its_size` |
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

Totals: **18 covered, 4 gaps, 0 not applicable.** Each of the four new tests was
verified by re-applying its own mutation: in every case that test — and only
that test — went red (258 passed / 1 failed).

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
