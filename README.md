# figma-dev-mcp

A local-first, read-only Model Context Protocol server for developers who inspect Figma designs in Dev Mode and turn that intent into implementation context.

It does not expose, dispatch, or compile any operation that mutates a Figma document. It does not write screenshots, SVG, or other exports to the local filesystem.

Starting an MCP client that launches this binary starts or discovers the shared loopback broker **without a separate daemon**. There is no daemon setup step.

## Architecture

One Rust executable serves MCP over stdio. The first process binds the broker; later processes become frontends and share the same Figma sessions.

```text
MCP client A ──stdio──┐
                     │
MCP client B ──stdio──┼── Rust frontend/broker ──loopback WebSocket── Figma plugin: file A
                     │                    └───────loopback WebSocket── Figma plugin: file B
MCP client N ──stdio──┘
```

Default loopback endpoints (not configurable in the MVP):

- plugin WebSockets: `127.0.0.1:3056` (`ws://localhost:3056` in the plugin; Figma rejects `127.0.0.1` in `allowedDomains`)
- internal frontend RPC (raw TCP, not HTTP): `127.0.0.1:3057`

The TypeScript companion plugin runs only in Figma Dev Mode. Its hidden UI connects to `ws://localhost:3056` with `Origin: null`. That origin check is **not authentication**; see [Limitations](#limitations).

## Quick start

1. Build the release binary: `cargo build --release`.
2. Build the companion: `(cd plugin && bun install --frozen-lockfile && bun run build)`.
3. In Figma desktop, switch the file to **Dev Mode** and import `plugin/manifest.json` as a **development plugin**.
4. Point an MCP client at `target/release/figma-dev-mcp` over **stdio** (no extra flags). Snippets: [Install in MCP clients](#install-in-mcp-clients).
5. Run the plugin in each relevant Figma file.
6. Call `list_files` to see live connections.

Full operator steps: [docs/setup.md](docs/setup.md). Local verification: [docs/testing.md](docs/testing.md). Manual checklist: [docs/manual-acceptance.md](docs/manual-acceptance.md).

## Install in MCP clients

The production binary is **stdio only**. It has no HTTP listener and takes no bind, port, or extra flags. Replace `/absolute/path/to/figma-dev-mcp` with this repository's path. Restart the client or start a new session after you change the config.

Build the release binary first (`cargo build --release`). Import and run the Dev Mode plugin as in [Quick start](#quick-start).

### Claude Code

```bash
claude mcp add --transport stdio --scope user figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp
```

`--scope local` writes `.mcp.json` in the current project. `--scope user` writes `~/.claude.json`. Equivalent project file:

```json
{
  "mcpServers": {
    "figma-dev-mcp": {
      "type": "stdio",
      "command": "/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp",
      "args": []
    }
  }
}
```

### Claude Desktop

Edit the desktop config, then restart Claude Desktop.

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "figma-dev-mcp": {
      "command": "/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"
    }
  }
}
```

### Codex

Add to `~/.codex/config.toml`, or to `.codex/config.toml` in a project:

```toml
[mcp_servers.figma-dev-mcp]
command = "/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"
```

Or:

```bash
codex mcp add figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp
```

### OpenCode

User config: `~/.config/opencode/opencode.json`. Project config: `opencode.json`.

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "figma-dev-mcp": {
      "type": "local",
      "command": ["/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"],
      "enabled": true
    }
  }
}
```

Or: `opencode mcp add`.

### Grok Build

```bash
grok mcp add figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp
```

That writes `~/.grok/config.toml`. For this repository only: `grok mcp add --scope project figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp` (`.grok/config.toml`). Equivalent TOML:

```toml
[mcp_servers.figma-dev-mcp]
command = "/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"
```

In an existing TUI session, run `/mcps` and press `r` to reload.

## Connection selection

`connectionId` is an opaque, ephemeral handle from `list_files`. It is valid only while that plugin socket stays live. A reconnect issues a new id; rediscover it with `list_files`.

- If **exactly one** Figma file is connected, file-scoped tools may omit `connectionId`.
- If several files are connected, every file-scoped call must provide `connectionId`. The server returns `AMBIGUOUS_CONNECTION` instead of choosing a file implicitly.

`list_files` itself is not file-scoped.

## Tools

The advertised catalog is exactly these 14 tools. Each tool is annotated `readOnlyHint: true`, `destructiveHint: false`, and `openWorldHint: false`.

| Tool | Purpose |
| --- | --- |
| `get_components` | Return components and component sets in a bounded node or page scope. |
| `get_design_context` | Return an implementation-oriented bounded design tree. |
| `get_dev_mode_data` | Return annotations, documentation, resources, and ownership metadata. |
| `get_fonts` | Return fonts used by a bounded scope and whether they appear in the editor font picker (`listAvailableFontsAsync`). |
| `get_metadata` | Return file, page, editor, and plugin capability metadata without descendants. |
| `get_motion` | Return bounded motion data when the Figma Motion API is available. Times are seconds. |
| `get_nodes` | Fetch one or more nodes by opaque ID while preserving input order. |
| `get_reactions` | Return prototype reactions and explicit target references. Trigger `timeout` is seconds on the live Plugin API (UI 800ms → `0.8`). `delay`, `transitionDuration`, and `mediaHitTime` are host numbers with no conversion. |
| `get_screenshot` | Render bounded nodes or the captured selection as raster or safe SVG assets. |
| `get_selection` | Return the current selection with a requested detail level and bounded depth. |
| `get_styles` | Return local styles and styles referenced by a bounded scope. |
| `get_variables` | Return variable collections, modes, aliases, scopes, and code syntax. |
| `list_files` | List live Figma connections. Connection IDs expire when plugin sockets reconnect. |
| `search_nodes` | Search exactly one explicit page or node scope with bounded predicates. |

## Prompts

The advertised catalog is exactly these three argumentless prompts. Each returns one user-role text message that guides tool choice. Prompts do not execute tools.

| Prompt | Purpose |
| --- | --- |
| `prototype_flow_strategy` | Read-only prototype journey analysis using reactions, optional motion, and targeted node context. |
| `read_design_strategy` | Token-efficient, read-only sequence for inspecting a Figma file with the 14-tool catalog. |
| `style_audit_strategy` | Bounded, report-only audit of raw values versus linked styles and variables. |

## Visual output and SVG source

`get_screenshot` accepts `format: "png" | "jpeg" | "svg"`.

- PNG and JPEG return MCP image content. `scale` is allowed only on raster variants (0.25–4.0).
- SVG uses Figma `exportAsync({ format: "SVG_STRING" })`. The result includes `image/svg+xml` content for clients that can preview it **and** the UTF-8 **SVG source** in structured content so a developer can use the vector asset directly.
- `scale` is invalid for SVG. Supported SVG options map to Figma's API: `svgOutlineText` defaults to `true`, `svgIdAttribute` to `false`, and `svgSimplifyStroke` to `true`.
- Safe vector structure is preserved, including `viewBox` and internal fragment references such as `url(#gradient)`.
- Before return, the iframe parses the SVG as XML and rejects scripts, `foreignObject`, inline event handlers, `javascript:` URLs, CSS `@import`, and non-fragment network references. Unsafe input fails with `UNSAFE_SVG` rather than being rewritten.

The tool does not choose or write a local path.

Motion fields use seconds (`duration`, `timelineOffset`, `timelineDuration`, `timelinePosition`). They are not converted to milliseconds.

Reaction `timeout` is **seconds** on the live Plugin API (UI After delay 800ms → host `0.8`). Official Trigger docs that say milliseconds are wrong. `delay`, `transitionDuration`, and `mediaHitTime` are copied without conversion; Transition/Motion examples are also seconds.

## Limitations

Read-only in Figma and side-effect-free on the local filesystem:

- The plugin targets Dev Mode (`editorType: ["dev"]`) with `capabilities: ["inspect"]` only.
- No public tool creates, deletes, moves, restyles, or otherwise mutates document content, the current page, the selection, plugin data, or relaunch data.
- No MVP operation creates a local export file. There is no filesystem-path argument.
- Results are bounded (depth, node count, serialized size, deadlines). Large or deep reads truncate instead of walking the whole document.
- `get_motion` fails with `CAPABILITY_UNAVAILABLE` when the live Motion surface is absent.
- `search_nodes` is single-scope only; document-wide and multi-page search are rejected.

Threat-model caveat: the MVP trusts processes running as the **same local** operating-system user and the browser context hosting the Figma plugin. Loopback binding blocks direct remote connections, but `Origin: null` is **not authentication** and can be reproduced by intentionally sandboxed browser content. Such content may register a fake session or cause denial of service; socket-bound correlation and the closed plugin-role protocol prevent it from reading or completing work for an existing Figma connection. Protection against a malicious local process requires per-install pairing in a separate design.

## Non-goals

- Creating, deleting, moving, resizing, renaming, or restyling Figma nodes.
- Editing text, variables, styles, components, prototypes, annotations, or plugin data.
- Persisting connection identifiers in the Figma document.
- Replacing Figma's editor or designer workflow.
- Writing screenshots, PDFs, or other exports to arbitrary local paths.
- Cloud hosting, shared team state, authentication, or remote network access.
- A general Figma REST API client.
- Any prompt whose intended outcome mutates Figma, or client-specific workflow skills.

## Development

Pinned toolchain: Rust 1.95.0 and Bun 1.3.14. See [docs/testing.md](docs/testing.md) for the seven local verification commands and the stdio versus official lifecycle-smoke evidence split.
