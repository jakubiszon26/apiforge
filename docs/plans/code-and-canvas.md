# Plan: scenario code, Export, variables and wire routing

- Status: approved for implementation (grill session, 23 October 2026)
- Scope: `apiforge-core`, `apiforge-runner`, `apiforge-storage`, `apiforge-desktop`,
  new crates `apiforge-script`, `apiforge-layout` and `apiforge-cli`

## Goals

1. **Export block** — a component publishes values to its caller by placing Export
   blocks in its graph, e.g. a component that fetches products exports the list.
2. **Scenario code** — every scenario and component can be edited as text in a
   built-in editor. Code and diagram are equivalent: editing one updates the other
   live, and new blocks from code are laid out automatically.
3. **Large editor for bodies** — request bodies are edited in the same editor, not
   in a one-line field.
4. **Completion** — environment variables, scenario parameters, blocks, variables,
   response paths and operators are suggested everywhere text is typed.
5. **Variables anywhere** — a variable can be placed as a proxy chip anywhere on the
   board, so no wire crosses the whole scenario.
6. **Routed wires** — wires avoid blocks, minimise bends and crossings, and are drawn
   orthogonally (default) or as smooth curves (setting).

The text format is a foundation for a future MCP server; this plan delivers the
GUI-free API and a CLI, not the server.

## Decisions

| Topic | Decision |
| --- | --- |
| Text format | Own small language (working name *ApiForge Script*, `.afs`), not JSON/YAML |
| Source of truth | The document model; code is a projection printed by a canonical formatter. Comments are stored as block notes |
| Code scope | Scenarios (graph and cleanup) and component definitions. Environment and contracts stay in the UI |
| Compatibility | Format version 2; no backward compatibility. Version 1 is rejected with a clear message. Fixtures and demos are converted once by a development tool |
| Pure blocks | Variable, Select, Formula and Build merge into one **Value** block: a named expression. The card adapts to the expression's shape |
| Layout | Positions are not in code. Unpositioned blocks are placed by auto-layout; a block moved by hand is pinned. *Tidy up* re-lays out the selection or board |
| Export | Defines a component output inside the component graph. Not used in scenarios (yet) |
| Variables | A Value block can be marked as a variable (`var` in code). Variables, environment variables and parameters can be placed as proxies anywhere |
| UI layout | Resizable code panel beside the board with tabs (Scenario, Cleanup, Body: …); selection synchronised both ways |
| Bodies | Two modes: object literal with expressions (default) and raw text template for XML, forms and text |
| Completion | Code editor, body editor, small fields (URL, headers, query) and response paths |
| Wires | Orthogonal with bridges at crossings by default; *spline* mode smooths the same route |
| Ordering | Explicit `after`. Line order in code is a canonical topological order without meaning |
| Delivery | Foundation → wires → proxies → language → editor → live sync, each a merged, tested branch |
| MCP | Pure library API and `apiforge fmt/check/run` CLI; the server comes later |

## Architecture

```
apiforge-core      document model v2, validation, graph semantics (no GUI)
apiforge-script    lexer, lossless syntax tree, parser, lowering, printer,
                   diagnostics, completion engine            (depends on core)
apiforge-layout    layered auto-layout and wire router, pure geometry
                   (no core, no egui; plain rectangles, ports and edges)
apiforge-runner    execution of v2 graphs
apiforge-storage   v2 workspace persistence
apiforge-cli       fmt, check, run on .afs files              (script, runner)
apiforge-desktop   code panel, editor widget, proxies, wire rendering
```

`apiforge-layout` knows nothing about blocks, which keeps it testable with
generated geometry and reusable by any future renderer.

## Format version 2

Changes to the version-one document, decided in ADR 0004:

- **Handles.** Every node has a `handle`: an identifier `[A-Za-z_][A-Za-z0-9_]*`,
  unique within its graph (graph and cleanup share one namespace, so cleanup can
  reference the main flow). It is the block's name in code. `name` becomes an
  optional display `label`. Internal `id`s stay and are never shown in code.
- **Node kinds.** `start`, `request`, `value`, `assert`, `condition`, `if`,
  `component`, `export`. `trigger` is renamed `start`; `variable`, `select`,
  `formula` and `build` are removed.
- **Value.** `inputs.value` holds the expression; `config.variable: bool` marks a
  variable. The card shows an operator layout for a top-level operator, a path
  layout for a reference with a path, a literal field for a literal and a field
  list for an object or array.
- **Bindings.** New kinds `object { fields }`, `array { items }` and
  `template { parts: [text | binding] }`. Templates embed bindings directly, so the
  `templateInputs` alias mechanism is removed. Dependencies and wires are still
  derived from reference leaves, now including template parts.
- **Export.** `kind: export`, `config.name`, `config.type`, optional contract, and
  `inputs.value`. Allowed only in component graphs; names are unique. A
  component's outputs are derived from its Export blocks, so `Component.outputs`
  is removed. Inputs remain declared on the component.
- **Notes.** `node.note: Option<String>` carries comments from code; the graph has
  its own `note` for leading comments.
- **Layout.** Semantics and layout are separated: `graph.layout` holds
  `nodes: { id: { x, y, pinned } }` and `proxies: [{ id, variable, x, y }]`.
  A node without a layout entry is placed by auto-layout on load.

The run report, inspector and library adapt to the new kinds. Operator entries in
the library keep their names and simply create Value blocks.

A development-only converter (`tools/convert-v1`, not shipped) rewrites the
compatibility projects, run fixtures and demos. The semantic expression corpus
(ADR 0001) is unaffected and stays tied to the TypeScript reference. The project and
run fixtures are frozen at their converted form and no longer regenerated from
TypeScript.

## The language

A sketch of what ADR 0005 specifies in full:

```
// Places an order for the first product.
scenario "Checkout" {
  param quantity: number = 2

  products = GET "{{env.baseUrl}}/products" {
    headers { Accept: "application/json" }
    query { limit: 10 }
    timeout 5s
  }
  status_ok = assert products.status == 200

  var first = products.body.items[0]
  total = first.price * param.quantity

  cart = AddToCart(product: first, qty: param.quantity)

  order = POST "/orders" after status_ok {
    body {
      productId: first.id,
      cartId: cart.id,
      total: total
    }
  }

  created = if order.status == 201 {
    receipt = GET "/orders/{{order.body.id}}"
  } else {
    failed = assert false
  }

  cleanup {
    removed = DELETE "/orders/{{order.body.id}}"
  }
}

component AddToCart(product: object, qty: number = 1) {
  response = POST "/cart" {
    body { id: product.id, qty: qty }
  }
  export id: string = response.body.cartId
}
```

Rules that make code and diagram equivalent:

- Every statement is `handle = <form>`. The form determines the block kind:
  an HTTP method is a request, `assert` / `condition all|any { … }` / `if` are
  checks and branches, a component name with arguments is an instance, `export`
  declares an output, and anything else is a Value. `var` marks a variable.
- Anonymous statements are accepted while typing and receive a generated handle
  (`assert_1`) that the formatter writes back.
- `after a, b` writes ordering edges. Statements inside `if` blocks receive the
  branch edge from the `if`. Start is implicit: top-level statements are
  connected to it. Blocks not reachable from Start appear in a `detached { }`
  section, so drafts on the board are never lost.
- Strings with `{{expr}}` are templates; `"""…"""` is a multi-line raw template.
  A `body { … }` is an object body; `body """…"""` is a text body.
- `env.name`, `param.name` and `handle.port.path` are references. A bare
  `handle.path` reads the block's main output (`body` for requests).
- `//` comments directly above a statement become its note; others attach to the
  enclosing block. The formatter states this in the language reference.

The equivalence contract, enforced by property tests:

1. `lower(parse(print(g))) == g` for every valid graph, ignoring layout.
2. `print(lower(parse(t)))` is idempotent under a second round.
3. The parser never panics and always returns a tree plus diagnostics (fuzzed).

**Identity across edits.** Lowering reconciles with the current graph: statements
match nodes by handle first. An unmatched new handle that replaces an unmatched
old one of the same kind at the same statement index is treated as a rename. The
node therefore keeps its `id`, pin and position. The editor's rename action (F2)
renames a handle and all its uses explicitly.

## Completion engine

Lives in `apiforge-script` and serves three modes: `script`, `object-body` and
`template` (small fields and raw bodies inside `{{ }}`). Sources:

- keywords, HTTP methods, operators and predicates with signatures;
- handles with their output ports and types; variables first;
- `env.*` (secret values never shown) and `param.*`;
- component names with argument snippets;
- response paths after `handle.`, taken from a host-supplied `ShapeProvider`. The
  desktop implements it with the contract, the sample response and the last run's
  output, in that priority order, and shows each path's type.

Results are ranked by scope and prefix match and have insertion snippets. The same
semantic index powers hover (type and source of a name) and go to definition.

## Editor widget

`apiforge-desktop` builds a `CodeEditor` widget on `egui::TextEdit` with:

- a custom layouter that colours tokens from the `apiforge-script` lexer;
- diagnostics shown as wavy underlines with hover messages and a gutter marker;
- a completion popup (Ctrl+Space or automatic after `.`, `{{` and identifiers),
  keyboard-navigable;
- hover, Ctrl+click to select the block on the board, bracket matching, auto-indent
  and F2 rename;
- line numbers and monospace typography matching the theme.

The widget sits behind its own module boundary so that a custom text engine can
replace `TextEdit` later if performance or features require it.

**Code panel.** A resizable panel beside the board with tabs: *Scenario*,
*Cleanup*, and *Body: <request>* opened from the inspector or a card. Inside a
component, the tab shows the component. The panel width and state persist in
settings.

**Small fields.** URL, header and query fields use the completion engine in
template mode. Their expand button opens the field as a panel tab.

## Live synchronisation

- **Code → diagram.** 250 ms after the last keystroke the text is parsed, lowered
  and validated. On success the reconciled graph replaces the board's graph as one
  coalesced undo step, and unpositioned blocks are placed by auto-layout. On
  failure the board keeps the last valid graph, shows a "code has errors" badge, and
  the editor marks the problems.
- **Diagram → code.** A board edit reprints the code. The cursor and scroll are
  anchored to the statement under the cursor (by handle) and restored after the
  reprint. While the code has errors, board editing is disabled with an
  explanation. This avoids silently discarding the user's unfinished text.
- **Selection.** The cursor's statement highlights its block; selecting a block
  highlights and reveals its statement.
- **History.** One undo stack for both views, using the existing coalescing in
  `editor.rs`.

## Auto-layout and wire routing

ADR 0006 records the algorithms; all live in `apiforge-layout`.

**Layered layout** (Sugiyama): layers by longest path along data and ordering
edges, crossing reduction by barycentre sweeps, and coordinates by Brandes–Köpf.
Flow runs left to right. Pinned blocks are fixed obstacles. A new block goes into
its layer at the free slot nearest its neighbours, and nothing else moves. *Tidy up*
lays out the selection or the whole board as one undo step and pins nothing.

**Router.** An orthogonal visibility graph over block rectangles inflated by a
margin, plus port stubs leaving each port in its natural direction. Routes come
from A\* with cost = length + bend penalty + crossing penalty. Then nudging
separates parallel segments that share a channel. Bridges are drawn where wires
cross. *Spline* mode turns the same route into a smooth curve with rounded
corners, so both styles avoid blocks equally well.

**Performance budgets** (release build, reference machine):

| Operation | Budget |
| --- | --- |
| Full route, 200 blocks / 400 wires | ≤ 30 ms |
| Reroute during drag (wires of moved blocks) | ≤ 8 ms per frame |
| Layered layout, 200 blocks | ≤ 20 ms |
| Parse + lower + validate, 1000 lines | ≤ 5 ms |

Untouched routes are cached by the geometry they depend on. When a drag exceeds
its budget, it falls back to direct curves until the next idle frame.

Settings: wire style (orthogonal / spline), bridges (on / off).

## Variable proxies

- The *Variables* panel lists variables (Value blocks marked `var`), parameters and
  environment variables. Dragging one onto the board creates a proxy chip, which is
  stored in `graph.layout.proxies`.
- A consumer's wire comes from the nearest proxy of that variable, or from the
  block itself when it is nearer. Proxies are layout only: the binding is unchanged,
  and code shows the plain reference.
- Dropping a proxy's output onto a field binds the variable, exactly as dragging
  from the block does.
- Clicking a proxy highlights its source and all uses; a button jumps to the source.
- Deleting a variable, or clearing its `var` mark, removes its proxies, as one
  undo step with the change.

## Milestones

Each milestone is one `feat/…` branch, merged into local `main` after fmt,
Clippy `-D warnings`, the full test suite and the compatibility check pass. GUI
changes are verified with screenshots. Nothing is pushed.

| # | Branch | Content | Depends on |
| --- | --- | --- | --- |
| M0 | `docs/code-canvas-plan` | This plan; ADR 0004 (format v2), 0005 (language), 0006 (layout and routing) | — |
| M1 | `feat/format-v2` | Model v2, validation, runner, storage, desktop adapted to Value/Export/templates/layout; v1 rejection; converter and converted fixtures; Export editing on the component board; the component builder creates Export blocks | M0 |
| M2 | `feat/layout-routing` | `apiforge-layout`; auto-placement of unpositioned blocks, pinning, *Tidy up*; routed wires, bridges, spline mode, settings; benchmarks | M1 |
| M3 | `feat/variable-proxies` | Variable mark in the inspector, Variables panel, proxies, nearest-proxy wiring, navigation | M1, M2 |
| M4 | `feat/script-language` | `apiforge-script` lexer/parser/lowering/printer/diagnostics, property tests and fuzz target; `apiforge-cli fmt/check` | M1 |
| M5 | `feat/code-editor` | Completion engine, `CodeEditor` widget, body tabs and modes, completion in small fields | M4 |
| M6 | `feat/live-sync` | Code panel, both sync directions, reconciliation, selection sync, error state, unified history; `apiforge-cli run` | M2, M5 |
| M7 | `feat/code-canvas-hardening` | Performance budgets checked, accessibility labels for the editor, language reference for users and MCP, screenshot QA | M6 |

M2 and M4 are independent after M1 and can be developed in parallel worktrees.
M3 can run alongside M4 and M5.

### Acceptance per milestone

- **M1:** every converted run fixture replays to the same report, apart from kind
  renames. Demos load and run. Importing v1 gives a clear error. Exports appear as
  instance outputs and run correctly, including nested components.
- **M2:** routing tests on generated boards confirm that no wire crosses a block
  rectangle. Tests also cover deterministic output and bridges at every crossing.
  Budgets hold. Pinned blocks never move without *Tidy up*. Screenshots cover both
  styles.
- **M3:** proxies survive save/load and do not appear in code. Deleting a variable
  removes its proxies in one undo step. A wire is never drawn twice for one consumer.
- **M4:** the equivalence properties hold over generated graphs and every fixture.
  Fuzzing finds no panics in a sustained run. Diagnostics include ranges and fixes
  where obvious. `fmt` is idempotent.
- **M5:** completion covers each listed source in each mode. Secret values never
  appear. Keyboard-only use works. Large bodies (1 MB) stay responsive.
- **M6:** a scripted session (type in code, edit on board, undo across both,
  introduce and fix an error) leaves code and graph equivalent at every step.
  Renames keep positions.
- **M7:** budgets are measured and recorded. The language reference is complete
  enough that a model can write a valid scenario from it alone.

## Risks

- **`egui::TextEdit` limits** (no multi-cursor, cost of relayout on large text).
  Mitigation: the widget boundary, the 1 MB check in M5, and a custom engine if
  needed.
- **Orthogonal routing quality.** Nudging and port ordering are where routers look
  amateurish. Mitigation: dedicated test boards with screenshot review, and the
  spline style as a fallback.
- **Identity on edits.** Heuristic renames can misfire on large pastes.
  Mitigation: match only on kind and index, provide explicit F2 rename, and keep
  one undo step per sync.
- **Notes from comments.** Comments not attached to a statement move to the
  enclosing block. This is documented, and the formatter makes it visible
  immediately.
- **Model change size.** M1 touches every crate. Mitigation: land it as small,
  individually green commits (model, validation, runner, storage, desktop,
  fixtures).

## Open, non-blocking

- Final language name and file extension.
- Whether scenarios can later live as `.afs` files in a repository (git-friendly
  projects).
- Scenario-level Export (publishing run results for CI or other scenarios).
