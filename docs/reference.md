# Behaviour reference

Detail that would clutter the [README](../README.md) but that you will want the first time something surprises you. Everything here describes behaviour that is tested; where a limit is unproven or unobserved, it says so.

## Architecture

One Rust executable serves MCP over stdio. The first process to start binds the broker; later processes become frontends and share the same Figma sessions, so several MCP clients can read the same Figma file without a daemon or a second install step.

```text
MCP client A ──stdio──┐
                      │
MCP client B ──stdio──┼── Rust frontend/broker ──loopback WebSocket── Figma plugin: file A
                      │                        └─loopback WebSocket── Figma plugin: file B
MCP client N ──stdio──┘
```

Loopback endpoints, fixed in this version:

- plugin WebSocket: `127.0.0.1:3056`. The plugin dials `ws://localhost:3056`, because Figma rejects `127.0.0.1` in `allowedDomains`.
- frontend RPC: `127.0.0.1:3057`, raw TCP rather than HTTP.

The TypeScript companion runs only in Figma Dev Mode. Its hidden UI opens the WebSocket with `Origin: null`.

## Version pairing

The plugin bundle and the binary must come from the same revision, and the handshake enforces it: the plugin announces a wire version and the broker refuses a mismatch before registering the session. That refusal is deliberate and it is a clean failure — you learn at connect time instead of losing a session mid-request.

Both ends reject unknown fields. Without the version check, a newer plugin talking to an older binary drops the whole session rather than failing one request, which is the confusing failure the check exists to replace.

The plugin ships **two** bundles: `plugin/dist/code.js` (the sandbox half) and `plugin/dist/index.html` (the UI half). A change to one leaves the other's hash untouched, so a hash is only evidence about the half it came from.

## Connections

`connectionId` is an opaque, ephemeral handle from `list_files`, valid only while that plugin socket stays live. A reconnect issues a new one.

- With exactly one file connected, file-scoped tools may omit it.
- With several connected, every file-scoped call must supply it. The server returns `AMBIGUOUS_CONNECTION` rather than choosing for you.
- `list_files` is the one tool that is not file-scoped.

Sessions are isolated per document: a node id from one file, asked of another, returns `NODE_NOT_FOUND` rather than crossing over.

## Reading design data

At `detail: "full"` a node's `paints` field carries **fills only** — border colour, width, alignment, and dash pattern live under `strokes`. Optional style fields are omitted when they sit at their Figma default, so an absent field means "default", not "unknown".

`get_styles` is two lists in one response: styles **referenced** by your scope, and the document's **local** styles. `selector` constrains only the referenced half. The default, `both`, therefore mixes a document-wide list with a scoped one.

`get_variables` works the other way round: it collects the variables actually bound in your scope, including ones stored in a shared library, and returns the collections that contain them. Asking about a different scope gives a different answer.

Results are bounded by depth, node count, serialised size, and deadlines. A large read truncates and says so, naming the budget that stopped it — `nodeLimit` and `byteLimit` are global and mean more depth will not help, while `depthLimit` on an individual node's `childrenTruncation` is local to that subtree.

`get_dev_mode_data`, `get_reactions`, and `get_motion` return an entry only for nodes that have something to report, alongside a `visitedNodes` count of everything walked. A node absent from the list was inspected and had nothing, not skipped.

## Units

Motion fields are **seconds**: `duration`, `timelineOffset`, `timelineDuration`, `timelinePosition`. They are not converted to milliseconds.

Reaction `timeout` is also **seconds** on the live Plugin API — Figma's UI showing "After delay 800ms" arrives as `0.8`. Official Trigger documentation that says milliseconds is wrong. `delay`, `transitionDuration`, and `mediaHitTime` are copied from the host without conversion.

Text `lineHeight` and `letterSpacing` carry their unit (`pixels`, `percent`, `auto`) rather than a bare number, because a bare `150` on 16px text is ambiguous between 150px and 24px.

## Screenshots and SVG

`get_screenshot` takes `format: "png" | "jpeg" | "svg"`.

PNG and JPEG return MCP image content, and `scale` (0.25–4.0) applies only to them. SVG uses Figma's `exportAsync({ format: "SVG_STRING" })` and returns the UTF-8 **source** in structured content, so you can use the vector directly. `scale` is invalid for SVG; `svgOutlineText` (default `true`), `svgIdAttribute` (default `false`), and `svgSimplifyStroke` (default `true`) map to Figma's own options.

Vector structure survives, including `viewBox` and internal fragment references such as `url(#gradient)`.

### SVG safety verdicts

Before returning, the plugin's iframe parses the SVG as XML and judges it. **The source is never rewritten and never withheld.** Every SVG asset carries a `safe` boolean, and an unsafe one also carries a `rejection` naming which rule fired — `parserError`, `unsafeElement`, `unsafeAttribute`, `unsafeCss`, or `unsafeProcessingInstruction` — plus the offending element or attribute **name**. Never a value: an attribute value can contain design content, which does not belong in a response or a log.

The caller decides what a verdict is worth, which is why the reason is precise. The risks are not equal: a remote `@font-face` URL at worst leaks a fetch when the file is opened, while a `<script>` element executes if the source is written to disk and later opened in a browser.

References are deny-by-default. A same-document `#fragment` passes, and so does a `data:` URL carrying a font media type or a PNG, JPEG, or WebP whose bytes actually validate. Every other scheme and media type is refused, as is a relative path and an `xml:base` naming an origin. Embedded fonts are deliberately allowed, which is what keeps `svgOutlineText: false` usable at all.

Scheme detection strips ASCII tab, line feed, and carriage return from anywhere in a value before testing it, matching what a browser's URL parser does, so `jav&#9;ascript:` does not slip past. The space character is deliberately **not** stripped, because browsers keep it too — `jav ascript:` genuinely is not a scheme, and flagging it would be a false alarm rather than a fix.

**One limitation worth knowing.** The `safe` verdict travels in structured content only. The `image/svg+xml` content block is emitted for every SVG asset, safe or not, and carries no marker. A client that auto-previews that block renders an unsafe SVG exactly like a safe one. Read `safe` from structured content before trusting a preview.

`UNSAFE_SVG` is reserved and no longer emitted; safety produces a verdict, not an error.

### Empty renders

A node that puts no ink on the page fails with a per-asset `EMPTY_NODE_BOUNDS`, in every format. An empty SVG or a 1×1 transparent pixel would be a success carrying nothing, and `INTERNAL_ERROR` is reserved for failures whose cause is unknown — here it is known.

The test is the host's own `absoluteRenderBounds`, measured after strokes and effects rather than from width and height. That distinction is load-bearing: a `LINE` is exactly zero pixels high by API contract, so every divider and underline in every file would fail a geometric test while rendering perfectly well. Measured against a real file, geometry got it backwards — the nodes that genuinely rendered nothing were 20×20 and 50×24.

Nodes that are switched off, whether by their own `visible` or an ancestor's, are handed to the exporter anyway, because the host reports no render bounds for anything invisible and that says nothing about whether the node is empty. So is a node whose bounds the host will not report at all.

## Guarantees

- The plugin targets Dev Mode (`editorType: ["dev"]`) with `capabilities: ["inspect"]` only.
- No tool creates, deletes, moves, restyles, or otherwise mutates document content, the current page, the selection, plugin data, or relaunch data.
- No operation creates a local file. There is no filesystem-path argument anywhere.
- `get_motion` fails with `CAPABILITY_UNAVAILABLE` when the Motion surface is absent.
- `search_nodes` is single-scope: document-wide and multi-page searches are rejected rather than silently widened.

## Threat model

The MVP trusts processes running as the **same local** operating-system user, and the browser context hosting the Figma plugin.

Loopback binding blocks direct remote connections, but `Origin: null` is **not authentication** — intentionally sandboxed browser content can reproduce it. Such content may register a fake session or cause denial of service. It cannot read or complete work belonging to an existing Figma connection: socket-bound correlation and the closed plugin-role protocol prevent that.

Protecting against a malicious local process would need per-install pairing, which is a separate design and not present here.
