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
| Delivery | Format → storage safety → language → wires → editor → live sync → proxies, each a merged, tested branch |
| MCP | Pure library API and a CLI that works against a workspace document (`fmt`, `check`, `print`, `apply`, `run`); the server comes later |
| Unfinished code | Buffers, drafts and two histories as in ADR 0007 |

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

A summary of [ADR 0004](../decisions/0004-document-format-v2.md):

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
- **Export.** `kind: export`, `config.type`, optional contract, and `inputs.value`;
  its handle is the output name. Allowed only in component graphs. A
  component's outputs are derived from its Export blocks, so `Component.outputs`
  is removed. Inputs remain declared on the component.
- **Notes.** `node.note: Option<String>` carries comments from code.
- **Layout.** `node.position` is required so every document can be drawn.
  `node.autoPlaced` marks blocks that automatic layout may move; moving one by hand
  clears the flag. Proxies will live in `graph.proxies` from M3.
  Layout never appears in code.
- **Requests.** URL, headers and query become bindings (templates and objects),
  replacing `templateInputs`, `inputPorts` and `/:name` path parameters. Values
  interpolated into a URL are percent-encoded.

The run report, inspector and library adapt to the new kinds. Operator entries in
the library keep their names and simply create Value blocks.

A development-only converter (`tools/convert-v1`, not shipped) rewrites the
compatibility projects, run fixtures and demos. The semantic expression corpus
(ADR 0001) is unaffected and stays tied to the TypeScript reference. The project and
run fixtures are frozen at their converted form and no longer regenerated from
TypeScript.

## The language

[ADR 0005](../decisions/0005-scenario-language.md) holds the grammar. An example:

```
// Places an order for the first product.
scenario "Checkout" {
  param quantity = 2

  begin = start
  @after(begin)
  products = GET "/products" {
    headers { Accept: "application/json" }
    query { limit: 10 }
    timeout 5s
  }
  status_ok = assert products.status == 200

  var first = products.body.items[0]
  total = first.price * param.quantity

  cart = AddToCart(product: first, qty: param.quantity)

  @after(status_ok)
  order = POST "/orders" {
    body { productId: first.id, cartId: cart.id, total: total }
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
```

The form of each `handle = …` statement decides the block kind. Ordering edges
use `@after(…)` or `if` blocks, and Start is an ordinary statement, so everything
the board can express has a text form. `//` comments above a statement become the
block's note.

The equivalence contract, enforced by property tests:

1. `semantic(lower(parse(print(g)))) == semantic(g)` for every valid graph, where
   `semantic` drops identifiers, order and layout as defined in ADR 0005.
2. `print(lower(parse(t)))` is idempotent under a second round.
3. The parser never panics and always returns a tree plus diagnostics (fuzzed).

**Identity across edits** is a separate contract: a block keeps its identity when
its handle and kind stay, through an explicit rename (F2 or the inspector), or
through an unambiguous rename (one block replaced by one equal block under a new
name). Anything ambiguous creates a new block. See ADR 0005.

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

**Code panel.** A resizable panel beside the board with tabs: *Scenario*
(including its `cleanup { }` block) and *Body: <request>* opened from the inspector
or a card. A body tab is a view of the scenario's code, not a separate document.
Inside a component, the tab shows the component. The panel width and state persist
in settings.

**Small fields.** URL, header and query fields use the completion engine in
template mode. Their expand button opens the field as a panel tab.

## Live synchronisation

[ADR 0007](../decisions/0007-code-editing-state.md) holds the rules. In short:

- Each scenario and component draft has one code buffer: Clean, Applied or
  Invalid. Valid text is applied 250 ms after the last keystroke as one document
  edit, and unpositioned blocks are placed by auto-layout.
- An Invalid buffer is never reprinted. Its board is read-only until the code is
  fixed or discarded, so unfinished text is never overwritten.
- Keystrokes have their own text history, and applied changes are document
  history. The focused panel decides which one Ctrl+Z uses.
- Unfinished code is a draft. Drafts follow the user across scenarios, are saved
  atomically beside the workspace, and are restored after closing or a crash.
- Run executes the document. With invalid code it first asks whether to run the
  last valid version.
- **Selection.** The cursor's statement highlights its block; selecting a block
  highlights and reveals its statement.

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
  stored in `graph.proxies`.
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
| M0 | `docs/code-canvas-plan` | This plan; [ADR 0004](../decisions/0004-document-format-v2.md), [0005](../decisions/0005-scenario-language.md), [0006](../decisions/0006-layout-and-wire-routing.md), [0007](../decisions/0007-code-editing-state.md) | — |
| M1 | `feat/format-v2` | Model v2, validation, runner, storage, desktop adapted to Value/Export/templates/layout; v1 rejection; converter and converted fixtures; Export editing on the component board; the component builder creates Export blocks | M0 |
| S1 | `feat/storage-safety` | STOR-01 from the [migration next steps](../migration/next-steps.md): single-writer lock, detection of external changes with a reload prompt, deliberate backup recovery; the drafts file of ADR 0007 under the same rules | M1 |
| M4 | `feat/script-language` | `apiforge-script` lexer/parser/lowering/reconciliation/printer/diagnostics, property tests and fuzz target; `apiforge fmt/check/print/apply` | M1 |
| M2 | `feat/layout-routing` | `apiforge-layout`; auto-placement of unpositioned blocks, pinning, *Tidy up*; routed wires with the ADR 0006 guarantees, bridges, spline mode, settings; benchmarks | M1 |
| M5 | `feat/code-editor` | Completion engine, `CodeEditor` widget, body views and modes, completion in small fields | M4 |
| M6 | `feat/live-sync` | Code panel, buffers, drafts and both histories (ADR 0007), both sync directions, selection sync; `apiforge run` | S1, M2, M5 |
| M3 | `feat/variable-proxies` | Variable mark in the inspector, Variables panel, proxies, nearest-proxy wiring, navigation | M2 |
| M7 | `feat/code-canvas-hardening` | Performance budgets checked, accessibility labels for the editor, language reference for users and MCP, screenshot QA | M6, M3 |

S1 lands before anything extends what is stored, because M1 already changes the
file format and ADR 0007 adds a second file. M4 and M2 are independent and can run
in parallel worktrees after S1. Proxies (M3) and visual polish wait until the code
path works end to end.

M1 is verified locally on Linux, including the current-board Export revision
correction, nested Export execution and native GUI editing/save evidence. See
[M1 acceptance](../migration/format-v2-acceptance.md).
S1 is implemented and verified on Linux: directory locking, external-change
fingerprints, deliberate recovery and draft storage are recorded in
[ADR 0008](../decisions/0008-storage-sessions.md). Native Windows execution remains
a release gate. M4 is in progress on `feat/script-language`: the lossless lexer,
recovering parser, syntax diagnostics, canonical syntax formatter and `fmt` CLI
are implemented. Lowering, reconciliation, the document printer, graph equivalence
tests and `check/print/apply` remain before M4 can merge. See the
[implementation and validation notes](../migration/scenario-language.md).
M2 can follow independently.

**Open release work outside this plan** keeps its place in the
[migration next steps](../migration/next-steps.md) and is scheduled between
milestones, not dropped: the interrupted Live run journal (RUN-02) after M4,
Windows execution evidence and assistive-technology acceptance (A11Y-01) as release
gates before M7 completes. A milestone that touches these areas updates their
evidence.

### Acceptance per milestone

- **M1:** every converted run fixture replays to the same report, apart from the
  mechanical conversion and divergences listed with their ADR in `run_corpus.rs`. Demos load and run. Importing v1 gives a clear error. Exports appear as
  instance outputs and run correctly, including nested components.
- **S1:** a second instance cannot write the workspace and says why. An external
  change is detected before the next save and offered for reload. A corrupt main
  file is recovered from the backup only after the user confirms. Tests cover
  each case and the drafts file.
- **M2:** on generated boards no final route intersects a card other than its own
  ends, after nudging and smoothing (overlapping cards excepted, ADR 0006). Tests
  also cover deterministic output and bridges at every crossing. Interactive frames
  meet the drag budget. Pinned blocks never move without *Tidy up*. Screenshots
  cover both styles.
- **M3:** proxies survive save/load and do not appear in code. Deleting a variable
  removes its proxies in one undo step. A wire is never drawn twice for one consumer.
- **M4:** the equivalence properties hold over generated graphs and every fixture.
  Reconciliation keeps identities only by the ADR 0005 rules: tests include
  insert-in-place, swapping two statements and replacing a request by a different
  one, none of which may transfer an identity. Fuzzing finds no panics in a
  sustained run. Diagnostics include ranges and fixes where obvious. `fmt` is
  idempotent. The CLI follows its contract, including secrets only from the
  environment or a file.
- **M5:** completion covers each listed source in each mode. Secret values never
  appear. Keyboard-only use works. Large bodies (1 MB) stay responsive.
- **M6:** a scripted session (type in code, edit on board, undo in both histories,
  introduce an error, switch scenario and back, close and reopen, fix the error)
  never loses text and leaves code and document equivalent whenever the buffer is
  valid. Explicit renames keep positions. Run with invalid code asks first.
- **M7:** budgets are measured and recorded. The language reference is complete
  enough that a model can write a valid scenario from it alone.

## Risks

- **`egui::TextEdit` limits** (no multi-cursor, cost of relayout on large text).
  Mitigation: the widget boundary, the 1 MB check in M5, and a custom engine if
  needed.
- **Orthogonal routing quality.** Nudging and port ordering are where routers look
  amateurish. Mitigation: dedicated test boards with screenshot review, and the
  spline style as a fallback.
- **Identity on edits.** A rename typed by hand that is not unambiguous creates a
  new block, which loses its position and pin. Mitigation: explicit rename (F2),
  and *Tidy up* places the new block next to its neighbours.
- **Read-only board while code is invalid.** It can surprise users. Mitigation: a
  clear banner with *Show errors* and *Discard code changes*.
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
