# figma-dev-mcp

Give your coding agent a way to read Figma designs — layout, styles, variables, components, prototypes, images — straight from Dev Mode, without copying screenshots into a chat.

It reads and nothing else. No tool in it can change a Figma document, and none writes a file to your disk.

Everything runs on your machine. There is no account, no cloud service, and no daemon to install: your MCP client launches the binary, and the first one to start also becomes the broker that other clients share.

## Requirements

- Figma **desktop**, with the file open in **Dev Mode**
- Rust 1.95.0 and Bun 1.3.14
- An MCP client that can launch a local command

## Build

```bash
cargo build --release
(cd plugin && bun install --frozen-lockfile && bun run build)
```

That produces the server at `target/release/figma-dev-mcp` and the Figma plugin in `plugin/dist/`.

Rebuild **both** together. The two halves check that they come from the same version and refuse to connect if they do not — better than the confusing mid-session failures that mismatch used to cause.

## Connect an MCP client

The server speaks **stdio** only. It takes no flags, no port, and no URL. Replace `/absolute/path/to/figma-dev-mcp` with your clone's path, then restart the client.

**Claude Code**

```bash
claude mcp add --scope user figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp
```

Use `--scope project` instead to write a `.mcp.json` into the current project.

**Claude Desktop** — edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS), `~/.config/Claude/claude_desktop_config.json` (Linux), or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "figma-dev-mcp": {
      "command": "/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"
    }
  }
}
```

**Codex** — `codex mcp add figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp`

**Grok Build** — `grok mcp add figma-dev-mcp -- /absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp`

**OpenCode** — in `~/.config/opencode/opencode.json`:

```json
{
  "mcp": {
    "figma-dev-mcp": {
      "type": "local",
      "command": ["/absolute/path/to/figma-dev-mcp/target/release/figma-dev-mcp"],
      "enabled": true
    }
  }
}
```

## Use it

1. In Figma desktop, switch the file to **Dev Mode**.
2. **Plugins → Development → Import plugin from manifest…**, and pick this repository's `plugin/manifest.json`. You only do this once.
3. Run the **Figma Dev MCP** plugin in each file you want to read. Keep it running while you work — closing it ends that file's session.
4. Ask your agent for something. It will usually start with `list_files`.

You can connect several Figma files at once. Each gets its own `connectionId`, which `list_files` reports. With exactly one file connected the agent can leave `connectionId` out; with more than one it must say which file it means, and the server refuses to guess.

Connection ids are temporary. If a plugin reconnects it gets a new one, so call `list_files` again rather than reusing an old id.

Step-by-step operator instructions live in [docs/setup.md](docs/setup.md).

## Tools

Fourteen tools, all read-only.

| Tool | What it gives you |
| --- | --- |
| `list_files` | The Figma files currently connected, and their connection ids. |
| `get_metadata` | File name, pages, editor type, and what this Figma build supports. |
| `get_selection` | Whatever is selected in Figma right now. |
| `get_nodes` | Specific nodes by id, in the order you asked for them. |
| `get_design_context` | A bounded design tree shaped for implementing from. |
| `search_nodes` | Find nodes by name, text, or type within one page or subtree. |
| `get_styles` | Styles used in a scope, plus the document's local styles. |
| `get_variables` | Variables bound in a scope, with their collections, modes, and values. |
| `get_components` | Components and component sets behind the instances in a scope. |
| `get_dev_mode_data` | Annotations, documentation, dev resources, and ownership. |
| `get_reactions` | Prototype interactions and where they lead. |
| `get_motion` | Motion and timeline data, when this Figma build has the Motion API. |
| `get_fonts` | Fonts a scope uses, and whether they are available in the editor. |
| `get_screenshot` | PNG, JPEG, or SVG. SVG comes back as source you can use directly. |

There are also three prompts, which suggest an efficient order to call the tools in. They only advise; they do not run anything.

| Prompt | When to reach for it |
| --- | --- |
| `read_design_strategy` | Inspecting a file without burning tokens on the whole document. |
| `prototype_flow_strategy` | Tracing what happens when someone clicks through a prototype. |
| `style_audit_strategy` | Finding raw values that should have been styles or variables. |

Prompts are user-invoked: your client shows them as slash commands, and the model cannot reach one on its own. The same three bodies are therefore also served as resources, which a model can fetch itself. Same text, two ways in.

| Resource | Serves |
| --- | --- |
| `figma://strategy/read_design_strategy` | The `read_design_strategy` body. |
| `figma://strategy/prototype_flow_strategy` | The `prototype_flow_strategy` body. |
| `figma://strategy/style_audit_strategy` | The `style_audit_strategy` body. |

Details that will surprise you eventually, such as how SVG safety verdicts work and which fields are omitted at their Figma defaults, are in [docs/reference.md](docs/reference.md).

## Non-goals

- Changing anything in Figma. No creating, deleting, moving, restyling, or editing text, variables, styles, components, prototypes, or annotations.
- Writing exports to your filesystem. There is no path argument anywhere.
- Replacing the Figma editor or a designer's workflow.
- Cloud hosting, shared team state, accounts, or any remote network access.
- Being a general Figma REST API client.

It also assumes anything running as your operating-system user is trustworthy. See the threat-model note in [docs/reference.md](docs/reference.md#threat-model) before running it somewhere that is not your own machine.

## Development

Pinned toolchain: Rust 1.95.0, Bun 1.3.14.

```bash
cargo test --workspace --all-features
(cd plugin && bun test)
```

Free ports `3056` and `3057` before running the Rust suite — a broker left running from an earlier session will fail tests in a way that looks like a code defect.

[docs/testing.md](docs/testing.md) has the full verification commands, and [docs/manual-acceptance.md](docs/manual-acceptance.md) is the record of what has actually been checked against a live Figma session, including what has not.
