# Setup

This is the documented macOS operator workflow. Launching the MCP client starts or discovers the local broker **without a separate daemon**. There is no daemon process to install or keep running.

Default endpoints, baked into the binary and the plugin manifest:

- plugin WebSocket: `127.0.0.1:3056`
- frontend RPC: `127.0.0.1:3057`

There is no bind-address or port flag. Changing `3056` would require a matching `plugin/manifest.json` change.

## Prerequisites

- Rust 1.95.0 (see `rust-toolchain.toml`)
- Bun 1.3.14 (`plugin/package.json` `packageManager`)
- Figma desktop, with files opened in **Dev Mode**
- An MCP-capable client that can spawn a local stdio server

## 1. Build the release binary

From the repository root:

```bash
cargo build --release
```

The executable is `target/release/figma-dev-mcp`. The CLI has no bind, port, remote, or limit-raising flags; `--help` and `--version` only.

## 2. Build the Dev Mode companion

```bash
(cd plugin && bun install --frozen-lockfile && bun run build)
```

This writes `plugin/dist/code.js` and `plugin/dist/index.html`, which `plugin/manifest.json` names as `main` and `ui`.

## 3. Import the plugin in Figma Dev Mode

1. Open Figma desktop and open each design file you want to inspect.
2. Switch that file to **Dev Mode**.
3. Use **Plugins → Development → Import plugin from manifest…** (wording may vary slightly by Figma desktop version).
4. Select this repository's `plugin/manifest.json`.
5. Confirm the import as a **development plugin**. The manifest requests only `inspect` capability and allows `ws://localhost:3056`. Figma rejects `127.0.0.1` in `allowedDomains`. The iframe connects to `ws://localhost:3056`; the broker still binds `127.0.0.1:3056`.

The companion is not a Figma Community publish in the MVP. Re-import or refresh after rebuilding `plugin/dist`.

## 4. Configure an MCP client over stdio

Point the client at the release binary with **stdio** transport and no extra arguments. Client-specific snippets for Claude Code, Claude Desktop, Codex, OpenCode, and Grok Build are in the [README](../README.md#install-in-mcp-clients). Generic `mcpServers` JSON:

```json
{
  "mcpServers": {
    "figma-dev-mcp": {
      "command": "/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"
    }
  }
}
```

Cursor, VS Code Copilot, and other stdio MCP clients use the same command. Do not add an HTTP URL: the production binary has no HTTP listener.

When the client starts this command:

1. The process tries `127.0.0.1:3057`.
2. If nothing is listening, it binds `127.0.0.1:3056` and `127.0.0.1:3057` and becomes the leader.
3. Later stdio processes connect to the leader as frontends.

That is the entire startup path. You do not start a broker yourself.

## 5. Start the plugin in each file

In every relevant Dev Mode file, run the imported **Figma Dev MCP** plugin. Each instance opens a WebSocket to `ws://localhost:3056` and registers an ephemeral `connectionId`. If the broker is briefly down, the plugin reconnects with bounded backoff and receives a **new** `connectionId`.

Keep the plugin running while you inspect that file. Closing it drops the session.

## 6. Discover files with `list_files`

Call `list_files` first when more than one file may be connected. It returns `connectionId`, file name, current page, connection time, and last-seen time.

### Connection-selection rule

- If **exactly one** plugin session is live, omit `connectionId` on the other tools.
- If several sessions are live, pass `connectionId` on every file-scoped tool. Omitting it returns `AMBIGUOUS_CONNECTION`.
- An unknown or stale id returns `CONNECTION_NOT_FOUND`. After reconnect, call `list_files` again.

`list_files` is the only tool that is not scoped to one file.

## Logs

The server logs to stderr so MCP stdio framing stays clean. See [testing.md](testing.md) for `FIGMA_DEV_MCP_LOG`. Default logs do not include node text, design names, screenshots, variable values, or payloads.

## Limitations to expect

The server is **read-only**. It does not change document content, page, selection, plugin data, or relaunch data, and it does not create a local export file.

`Origin: null` on the plugin socket is origin filtering, **not authentication**. The MVP trusts the same local operating-system user. See the README threat-model caveat.
