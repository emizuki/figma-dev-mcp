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

| Accept path | Mutation | Suite result | Verdict | Test added |
|---|---|---|---|---|

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
