# Testing

Pinned runtimes: Rust 1.95.0 (`rust-toolchain.toml`) and Bun 1.3.14 (`packageManager` in `plugin/` and `conformance/`). Use frozen lockfiles.

## Seven local verification commands

Run these from the workspace root, in a clean process state (no leftover `figma-dev-mcp` bound to `127.0.0.1:3056` / `127.0.0.1:3057`, and no leftover test adapter on `127.0.0.1:3060`):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
(cd plugin && bun install --frozen-lockfile)
(cd plugin && bun run format:check && bun run typecheck && bun run build && bun run test)
(cd conformance && bun install --frozen-lockfile)
./scripts/run-conformance.sh
```

`cargo test --workspace --all-features` stays green from a clean source checkout. Rust policy tests scan `plugin/src` and do not require `plugin/dist`. After `bun run build`, plugin bundle policy tests scan `plugin/dist`.

## Evidence split

Product evidence and official runner evidence are not the same gate.

- **Production stdio eras.** `crates/figma-dev-mcp/tests/stdio_eras.rs` spawns the real `figma-dev-mcp` binary and exercises protocol paths over stdio for `2026-07-28` (including `server/discover`) and legacy `2025-11-25` (`initialize`, `notifications/initialized`, tool and prompt lists). `tests/integration/all_tools.rs` drives the exact 14-tool / three-prompt catalog through production `McpService` with a scripted fake plugin.
- **Official lifecycle smoke.** The upstream conformance CLI accepts `--url`, not a stdio command. `scripts/run-conformance.sh` therefore starts the **test-only HTTP adapter** (`tests/src/bin/conformance-server.rs`) and runs the two pinned lifecycle smoke scenarios: modern `server-stateless` (`2026-07-28`) and legacy `server-initialize`. Those two scenarios are **not** a substitute for the production stdio matrix.

Do not treat the two official lifecycle smoke runs as complete product proof. They check lifecycle and handler behavior through the adapter. The production transport, exact catalog, and Figma-facing behavior are proven by the stdio and all-tools tests.

The adapter is a tests-crate binary only. The production CLI has no HTTP option, and `crates/figma-dev-mcp` does not enable Streamable HTTP features.

Human ruling A: the advertised catalog stays the 14 tools and three prompts. The adapter may intercept three unadvertised diagnostic `tools/call` names (`test_missing_capability`, `test_streaming_elicitation`, `test_logging_tool`) so those runner checks are not marked "not testable". They are **not** product tools. They must not appear in `tools/list`, `get_tool`, or `server/discover`, they are not compiled into the production binary, and they must not appear in the README tool tables.

CI runs the same split: `rust-static`, `rust-tests`, `plugin`, `policy` (production plugin build plus the Rust policy target), and `conformance` (adapter plus both pinned official scenarios). CI never publishes an artifact or package.

## Log controls

Logs go to **stderr** so MCP stdout stays protocol frames only.

| Setting | Effect |
| --- | --- |
| unset | `info` for the process |
| `FIGMA_DEV_MCP_LOG=off` | silence this family's logs |
| `FIGMA_DEV_MCP_LOG=debug` | schema-safe debug for this crate family; `rmcp` stays at `info` |

Default and debug logs may include request IDs, tool names, connection IDs, durations, counts, byte sizes, and stable error codes. They do **not** contain node text, design names, screenshot bytes or SVG source, variable values, or full payloads. There is no telemetry.

Example:

```bash
FIGMA_DEV_MCP_LOG=debug target/release/figma-dev-mcp
```

## Related checks

Policy tests in `tests/policy/` snapshot the tool and prompt allowlists, reject mutation APIs, and require these operator documents to keep ports, catalogs, SVG-source behavior, read-only limitations, and the `Origin: null` threat-model caveat accurate.

Manual Figma checks live in [manual-acceptance.md](manual-acceptance.md). They are a separate external gate from the commands above.
