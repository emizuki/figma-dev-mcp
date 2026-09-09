# Acceptance sweep — `plugin/src/read` rows

The per-file mutation tables for the `plugin read` section of
[`acceptance-sweep.md`](./acceptance-sweep.md), moved here so that document
stays readable. The verdict vocabulary, the counts, the findings and the
`not applicable` rule all live there; this file is the raw evidence behind them.

One row per accept path, at the granularity where one mutation answers exactly
one row. **Suite result** is pass / fail of the whole plugin suite with that one
mutation applied, against the baseline 476 / 0 at `f65c29e` — except for the
twenty-seven rows found after those runs (`B31`–`B46`, `X120`–`X122`, `C81`,
`C82`, `SE89`, `Z801`–`Z803`, `V98`, `V99`), which are measured against the
suite as it stood when they were found: 600 / 0, 615 / 0 or 632 / 0. The **Mutation** column is the replacement text, with `⏎` for a newline;
it repeats across rows in 54 places, so the **Accept path** is what identifies
the site.

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
| loading document pages checks cancellation on every root | `if (!isRecord(root) \|\| root.type !== "DOCUMENT") continue` | 615 / 0 | **gap** | — *(open)* |
| loading document pages checks cancellation on every page | `await loadPageIfNeeded(child)` | 615 / 0 | **gap** | — *(open)* |
| get_selection checks cancellation on every selected id | `const node = await lookupNode(id) ⏎ if (node === null \|\| node === undefined) continue ⏎ const verdict = visib…` | 615 / 0 | **gap** | `get_selection checks cancellation on every selected id` |
| get_nodes checks cancellation on every requested id | `if (lookup === undefined) {` | 615 / 0 | **gap** | `get_nodes checks cancellation on every requested id` |
| a selection scope checks cancellation on every selected id | `const node = await lookupNode(id) ⏎ if (node === null \|\| node === undefined) continue ⏎ const verdict = visib…` | 615 / 0 | **gap** | `a selection scope checks cancellation on every selected id` |
| a nodeIds scope checks cancellation on every id | `const node = await lookupNode(id) ⏎ if (node === null \|\| node === undefined) {` | 615 / 0 | **gap** | `a nodeIds scope checks cancellation on every id` |

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
| the forest walk is not depth-limited | `depth: 0,` | 476 / 0 | **gap** | — *(open)* |
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
| the forest walk checks cancellation on every node | `if (context.visitedNodes >= context.limits.visitedNodes) { ⏎ markTruncated(context, { ⏎ reason: "nodeLimit", ⏎ …` | 476 / 0 | **gap** | `the forest walk checks cancellation on every node` |
| the instance pre-pass checks cancellation on every node | `const node = record(raw) ⏎ if (node.type === "INSTANCE") instances.push(node)` | 476 / 0 | **gap** | `the instance pre-pass checks cancellation on every node` |
| the style-name pre-pass checks cancellation on every node | `const node = record(raw) ⏎ for (const [field] of STYLE_ID_FIELDS) {` | 476 / 0 | **gap** | `the style-name pre-pass checks cancellation on every node` |
| the variable-name pre-pass checks cancellation on every node | `const node = record(raw) ⏎ for (const id of variableIdsOf(node)) {` | 476 / 0 | **gap** | `the variable-name pre-pass checks cancellation on every node` |
| the instance identity pass checks cancellation on every instance | `const id = string(node.id)` | 615 / 0 | **gap** | `the instance identity pass checks cancellation on every instance` |
| the style-name pass checks cancellation on every id | `// Running out of budget leaves the remaining names absent; the forest is ⏎ // never marked truncated over a missin…` | 615 / 0 | **gap** | `the style-name pass checks cancellation on every id` |
| the variable-name pass checks cancellation on every id | `// Running out of budget leaves the remaining names absent; the forest is ⏎ // never marked truncated over a missin…` | 615 / 0 | **gap** | `the variable-name pass checks cancellation on every id` |
| a throwing getter costs that field, not the node | `function hostGet(value: UnknownRecord, key: string): unknown { ⏎ return value[key] ⏎ }` | 619 / 13 | covered | — |
| a segment reader that throws costs the styled ranges, not the node | `} catch (e) { ⏎ throw e ⏎ // getStyledTextSegments rejects the whole call if any single field name in` | 632 / 0 | **gap** | `host objects that refuse to be enumerated or read cost one field, not the node` |
| component properties that cannot be enumerated cost that field, not the node | `entries = Object.entries(source) ⏎ } catch (e) { ⏎ throw e ⏎ }` | 632 / 0 | **gap** | `host objects that refuse to be enumerated or read cost one field, not the node` |

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
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 466 / 10 | covered | — |
| a caller-supplied returnedNodes limit is honoured | `returnedNodes: MAX_RETURNED_NODES,` | 475 / 1 | covered | — |
| a caller-supplied encodedBytes limit is honoured | `encodedBytes: MAX_TEXT_BYTES,` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| the returnedNodes ceiling is inclusive | `if (this.styles.length >= this.limits.returnedNodes + 1) {` | 475 / 1 | covered | — |
| the encodedBytes ceiling is exclusive at the boundary | `if (encoded > this.limits.encodedBytes + 1) {` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| a nodeLimit truncation reports how many were visited | `visitedNodes: 0,` | 476 / 0 | **gap** | `the node ceiling reports how many styles were considered` |
| a byteLimit truncation reports the encoded size | `this.truncation = { reason: "byteLimit" }` | 476 / 0 | **gap** | `the byte ceiling counts every emitted style and reports the total` |
| cancellation is checked at a batch boundary in the local style pass | delete it | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the referenced style pass | `signal?.throwIfAborted() ⏎ if (!emission.consider()) return` | 476 / 0 | **gap** | — *(open)* |
| the local style pass checks cancellation on every style | `index += 1` | 476 / 0 | **gap** | `the local style pass checks cancellation on every style` |
| the referenced style pass checks cancellation on every id | `if (!emission.consider()) return` | 476 / 0 | **gap** | `the referenced style pass checks cancellation on every id` |

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
| resolving a search scope checks cancellation first | `if ("pageId" in scope) {` | 615 / 0 | **gap** | — *(open)* |
| the search walk checks cancellation on every node | delete it | 614 / 1 | covered | — |
| a throwing characters getter costs the text match, not the search | `return typeof node.characters === "string" ? node.characters : undefined ⏎ } catch (e) { ⏎ throw e ⏎ }` | 631 / 1 | covered | — |

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
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the variable lookup loop | `signal?.throwIfAborted() ⏎ // The lookups themselves` | 476 / 0 | **gap** | — *(open)* |
| cancellation is checked at a batch boundary in the collection loop | `signal?.throwIfAborted() ⏎ if (!emission.consider()) break` | 476 / 0 | **gap** | — *(open)* |
| the variable lookup loop checks cancellation on every id | `// The lookups themselves` | 476 / 0 | **gap** | `the variable lookup loop checks cancellation on every id` |
| the collection loop checks cancellation on every collection | `if (!emission.consider()) break` | 476 / 0 | **gap** | `the collection loop checks cancellation on every collection` |
| the definition loop checks cancellation on every variable | `const serialized = await readDefinition(` | 476 / 0 | **gap** | `the definition loop checks cancellation on every variable` |
| a variable lookup checks cancellation before it starts | `if (this.variables.has(id)) return this.variables.get(id) ?? null` | 615 / 0 | **gap** | — *(open)* |
| a collection lookup checks cancellation before it starts | `if (this.collections.has(id)) return this.collections.get(id) ?? null` | 614 / 1 | covered | — |
| valuesByMode keys that cannot be read cost the fallback modes, not the read | `return Object.keys(valuesByMode).filter((modeId) => modeId.length > 0) ⏎ } catch (e) { ⏎ throw e ⏎ }` | 632 / 0 | **gap** | `a host that refuses its own keys or throws on lookup costs that much and no more` |
| a host lookup that throws synchronously costs that lookup, not the read | `return (await settleOrSkip(call())) ?? null ⏎ } catch (e) { ⏎ throw e ⏎ }` | 632 / 0 | **gap** | `a host that refuses its own keys or throws on lookup costs that much and no more` |

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
| the raster codec reports the encoded length | `base64Bytes: 0,` | 476 / 0 | **gap** | — *(open)* |
| the raster codec reports no decoded byte count | `decodedBytes: 1,` | 476 / 0 | **gap** | — *(open)* |
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
| a VARIABLE_ALIAS easing type is accepted | delete it | 476 / 0 | **gap** | — *(open)* |
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
| the item loop stops once the emission ceiling is hit | `if (!emission.push(item, visitedNodes)) continue` | 476 / 0 | **gap** | `the motion item loop stops at the ceiling, and stops counting too` |
| the result carries the emitted items | `items: [],` | 467 / 9 | covered | — |
| the result reports how many nodes were inspected | `visitedNodes: 0, ⏎ truncated:` | 475 / 1 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| a non-empty catalogue reaches the result | `if (false) result.availableStyles = catalog` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 476 / 0 | **gap** | `the item ceiling is inclusive and reports how many nodes were inspected`, `a truncated walk outranks the emission cut and keeps its own reason`, `the byte ceiling counts every emitted item and reports the total` |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 476 / 0 | **gap** | `a node whose only content is an animation is emitted` |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `a node whose only content is an animation is emitted` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 472 / 4 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 468 / 8 | covered | — |
| cancellation is checked at a batch boundary in the motion item loop | delete it | 476 / 0 | **gap** | — *(open)* |
| the motion item loop checks cancellation on every item | `visitedNodes += 1` | 476 / 0 | **gap** | `the motion item loop checks cancellation on every item` |

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
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
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
| the instance pass checks cancellation on every batch | `if (emission.emitTruncation !== undefined) break ⏎ if (Date.now() - budgetStarted >= budgetMs) { ⏎ emission.mar…` | 476 / 0 | **gap** | `the instance pass checks cancellation on every batch` |
| the component loop checks cancellation on every component | `try { ⏎ const component = serializeComponent(components.get(id))` | 476 / 0 | **gap** | `the component loop checks cancellation on every component` |
| the off-page pass checks cancellation on every batch | `if (emission.emitTruncation !== undefined) break ⏎ if (Date.now() - budgetStarted >= budgetMs) { ⏎ emission.mar…` | 615 / 0 | **gap** | `the off-page pass checks cancellation on every batch` |
| a throwing getter costs that field, not the component | `function hostGet(value: UnknownRecord, key: string): unknown { ⏎ return value[key] ⏎ }` | 630 / 2 | covered | — |
| property definitions that cannot be enumerated cost that field, not the component | `entries = Object.entries(raw) ⏎ } catch (e) { ⏎ throw e ⏎ }` | 632 / 0 | **gap** | `property definitions that cannot be enumerated cost that field only` |

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
| the item loop stops once the emission ceiling is hit | `if (!emission.push(value, visitedNodes)) continue` | 476 / 0 | **gap** | `the dev-mode item loop stops at the ceiling, and stops counting too` |
| the result carries the emitted items | `items: [],` | 466 / 10 | covered | — |
| the result reports how many nodes were inspected | `visitedNodes: 0, ⏎ truncated:` | 473 / 3 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 475 / 1 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 475 / 1 | covered | — |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `each kind of content on its own is enough to emit a node` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 475 / 1 | covered | — |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 467 / 9 | covered | — |
| a catalogue entry with an id is kept | `if (id.length >= 0) continue ⏎ categories.push` | 475 / 1 | covered | — |
| a catalogue reader that throws costs only the catalogue | `} catch (e) { ⏎ throw e ⏎ } ⏎ } ⏎ ⏎ function annotations` | 475 / 1 | covered | — |
| a dev-resource reader that throws costs only that node's resources | `} catch (e) { ⏎ throw e ⏎ }` | 475 / 1 | covered | — |
| cancellation is checked at a batch boundary in the dev-mode item loop | delete it | 476 / 0 | **gap** | — *(open)* |
| the dev-mode item loop checks cancellation on every item | `visitedNodes += 1` | 476 / 0 | **gap** | `the dev-mode item loop checks cancellation on every item` |

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
| the item loop stops once the emission ceiling is hit | `if (!emission.push(value, visitedNodes)) continue` | 476 / 0 | **gap** | `the reactions item loop stops at the ceiling, and stops counting too` |
| the result carries the emitted items | `items: [],` | 463 / 13 | covered | — |
| the result reports how many nodes were inspected | `visitedNodes: 0, ⏎ truncated:` | 473 / 3 | covered | — |
| the truncated flag reflects the truncation | `truncated: false,` | 475 / 1 | covered | — |
| the truncation reason reaches the result | `if (false) result.truncation = emission.truncation` | 475 / 1 | covered | — |
| observation.startedAt is the time the read began | `return { startedAt: "", completedAt: new Date().toISOString() }` | 476 / 0 | **gap** | `two reacting nodes are both emitted under the default ceilings` |
| observation.completedAt is emitted | `return { startedAt } as { startedAt: string; completedAt: string } ⏎ }` | 476 / 0 | **gap** | `two reacting nodes are both emitted under the default ceilings` |
| returnedNodes defaults to MAX_RETURNED_NODES | `returnedNodes: limits?.returnedNodes ?? 1,` | 476 / 0 | **gap** | `two reacting nodes are both emitted under the default ceilings`, `a truncated walk outranks the emission cut, and the byte ceiling reports its total` |
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 465 / 11 | covered | — |
| cancellation is checked at a batch boundary in the reactions item loop | delete it | 476 / 0 | **gap** | — *(open)* |
| the reactions item loop checks cancellation on every item | `visitedNodes += 1` | 476 / 0 | **gap** | `the reactions item loop checks cancellation on every item` |
| an UPDATE_MEDIA_RUNTIME action is reported as updateMediaRuntime | `case "UPDATE_MEDIA_RUNTIME_X": {` | 630 / 2 | covered | — |
| a URL action is reported as openLink | `case "URL_X": {` | 630 / 2 | covered | — |
| a NODE action is reported as its navigation kind | `case "NODE_X": {` | 624 / 8 | covered | — |

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
| visitedNodes defaults to MAX_VISITED_NODES | `visitedNodes: limits?.visitedNodes ?? 1,` | 476 / 0 | **gap** | — *(open)* |
| encodedBytes defaults to MAX_TEXT_BYTES | `encodedBytes: limits?.encodedBytes ?? 1,` | 469 / 7 | covered | — |
| the cancellation batch counter advances with the walk | `index += 0` | 476 / 0 | **gap** | — *(open)* |
| fonts are emitted in first-seen order | `for (const [key, usage] of [...collected].reverse()) {` | 472 / 4 | covered | — |
| cancellation is checked at a batch boundary in the font walk | delete it | 476 / 0 | **gap** | — *(open)* |
| the font loop checks cancellation on every font | `for (const [key, usage] of collected) {` | 600 / 0 | **gap** | `the font loop checks cancellation on every font` |
| the font catalogue read checks cancellation before it starts | `try {` | 615 / 0 | **gap** | `the font catalogue read checks cancellation before it starts` |
