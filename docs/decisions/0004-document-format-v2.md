# ADR 0004: Document format version 2

- Status: accepted
- Date: 2026-10-23
- Scope: `apiforge-core`, `apiforge-runner`, `apiforge-storage`, `apiforge-desktop`
- Plan: [scenario code and canvas](../plans/code-and-canvas.md)

## Context

Scenario code ([ADR 0005](0005-scenario-language.md)) must be equivalent to the
board: printing a graph and parsing the text must give the same graph back. Several
parts of the version-one format prevent that or duplicate concepts:

- Node names are free text and not unique, so code cannot refer to blocks by name.
- Variable, Select, Formula and Build are four kinds of one idea: a named value.
  `x = a.price * 2` would otherwise be ambiguous.
- Request URLs, headers and query values are configuration strings whose
  `{{alias}}` tokens resolve through `templateInputs` aliases, declared
  `inputPorts`, `/:name` path parameters and an implicit environment fallback. These
  are four lookup mechanisms for one concept: "insert this value here".
- Bodies embed `{{alias}}` strings in JSON, where a whole-string token keeps its
  type and a partial one becomes text.
- Component outputs are configured in a dialog, not in the component's graph.
- Positions are required, so blocks created from code would have no place.

The user decided that backward compatibility is not needed at this stage.

## Decision

Documents carry `"version": 2`. Version 1 is rejected with the message *"This
document uses format version 1, which is no longer supported"*. There is no runtime
migration. The development converter `tools/convert-v1` rewrites fixtures and demos
once. It is neither shipped nor part of the import path.

### Nodes

```text
Node {
  id: String                 // internal, stable, never shown in code
  handle: String             // [A-Za-z_][A-Za-z0-9_]{0,63}, unique, the name in code
  label: Option<String>      // display text; the handle is shown when absent
  note: Option<String>       // comment attached in code
  kind: NodeKind
  inputs: IndexMap<String, Binding>
  config: NodeConfig
  position: Option<Position> // None: placed by auto-layout
  pinned: bool               // placed by the user; omitted when false
}
```

Handles are unique across a scenario's graph and cleanup graph, because cleanup
steps refer to main-phase results. A component graph has its own namespace. Handles
must not be reserved words of the language.

`NodeKind` is `start`, `request`, `value`, `assert`, `condition`, `if`,
`component` or `export`.

| Kind | Inputs | Config | Outputs |
| --- | --- | --- | --- |
| `start` | — | — | — |
| `request` | `url` (required), `headers`, `query`, `body` | `method`, `timeoutMs`, `bodyFormat` (`json` \| `text`), `sampleResponse`, `sampleStatus`, `contractId` | `body`, `status`, `headers`, `duration` |
| `value` | `value` | `variable: bool` | `value` |
| `assert`, `if` | `actual`, `expected` | `predicate`, `tolerance` | `value`, `passed` |
| `condition` | `actual1`, `expected1`, … | `predicates`, `match`, `tolerance` | `value`, `passed` |
| `component` | the definition's inputs | `componentId`, `revision` | the definition's exports |
| `export` | `value` | `type`, `contractId` | — |

- **Value** replaces Variable, Select, Formula and Build. The expression in
  `inputs.value` may be any binding. A missing result is not an error: the step
  passes with no output, and dependants report their missing input as before.
  The card's layout follows the expression's top level. An operator shows operand
  rows, a reference with a path shows the selection, a literal shows an editable
  field, and an object or array shows its fields. `variable: true` lists the value
  in the Variables panel and allows proxies.
- **Request.** `headers` and `query` are object bindings whose values are converted
  to text with `String(value)` (JSON for objects and arrays). `bodyFormat: json`
  sends the body value as JSON (default `Content-Type: application/json`).
  `bodyFormat: text` sends `String(value)` with default `Content-Type: text/plain`.
  `inputPorts`, `templateInputs` and `/:name` path parameters are removed.
- **Condition** predicates refer to their inputs by position. Predicate *n* (from 1)
  compares `actual<n>` with `expected<n>`. A field path is part of the actual
  value's reference binding, not of the predicate.
- **Export** is valid only in a component graph. Its handle is the output name.
  Its value type and contract describe the output port. A skipped or failed Export
  leaves the output missing.

### Bindings

Version-one bindings stay (`expression`, `literal`, `reference`, `parameter`,
`environment`) and three are added:

```text
object   { fields: IndexMap<String, Binding> }
array    { items: Vec<Binding> }
template { parts: Vec<TemplatePart> }   // TemplatePart = text { text } | value { binding }
```

- An object or array evaluates every child. A missing child is an error that names
  its field path, as a missing build field or body argument was before.
- A template concatenates its parts and converts values with `String(value)`
  (JSON for objects and arrays). A missing value is an error. In a request `url`,
  value parts are percent-encoded like `encodeURIComponent`, which replaces `/:name`
  parameters safely. Text parts and the environment's base URL are not encoded.
- Every child of `object`, `array` and `template` is traversed by the binding
  helpers. Data wires and dependencies therefore include references at any depth.
  Wire identifiers extend the version-one form with field names and indexes, e.g.
  `data:n:body:fields/items/0`.
- **Canonical form**, enforced by validation and kept by editing helpers:
  - a `literal` holds a scalar (string, number, boolean or null); objects and
    arrays are always `object` and `array` bindings, even when every leaf is a
    literal (JSON samples, contracts and port defaults are plain JSON, not
    bindings);
  - a template has at least one value part; text alone is a `literal` string;
  - adjacent text parts are merged and empty text parts are dropped;
  - reference paths are in the canonical `JsonPath` form (`a.b[0]["x y"]`).

  Each value therefore has exactly one representation. This is what makes
  printing and parsing ([ADR 0005](0005-scenario-language.md)) a bijection.

Parameter bindings name scenario parameters, or component inputs inside a
component. Environment bindings name environment variables. Nothing falls back to
the environment implicitly any more.

### Components

```text
Component {
  id, revision, handle, name, description,
  inputs: Vec<Port>,
  graph: Graph,
  action: Option<ComponentAction>,
}
```

`handle` is unique within the project and names the component in code. `outputs`
is removed: a component's output ports are derived from its Export nodes, in graph
order. The component builder creates Export nodes instead of output rows.

### Layout

Layout is the only part of the document without meaning for a run:

- `position: None` marks a node for automatic placement; the editor places it
  when the board is opened or the graph changes.
- `pinned: true` marks a node the user placed. Automatic placement treats pinned
  nodes as fixed obstacles and moves only unpinned ones. *Tidy up* moves both and
  clears the pin.
- `Graph::proxies: Vec<Proxy>`, with `Proxy { id, source, position }` and
  `source` being `variable { nodeId }`, `environment { name }` or
  `parameter { name }`, holds variable proxies ([plan](../plans/code-and-canvas.md#variable-proxies)).

Scenario code never contains layout. Equivalence is defined on the document
without `position`, `pinned` and `proxies`.

### Run reports

Steps carry the node's `handle` and display `label` instead of `name`, and the new
kinds. Report JSON is therefore not comparable with version-one reports.

## Consequences

- The TypeScript prototype can no longer open documents of the native application.
  The expression corpus of [ADR 0001](0001-numeric-semantics.md) is unaffected,
  because operator semantics do not change.
- The project and run fixtures under `fixtures/compatibility/` are converted once
  and then maintained as native fixtures. `scripts/export-compatibility-fixtures.ts
  --check` covers only the semantic corpus afterwards. Each changed expectation in
  the run fixtures is reviewed and attributed to one of these decisions:
  kind names, handles, Value step messages, or encoded URL values.
- A Value's expression is evaluated like any nested expression. An operand that
  resolves to a missing value is therefore an error, where a version-one formula
  block substituted the operand's default. The run corpus lists this divergence.
- Parameter bindings gain an optional field `path`, so a field of a component input
  (`param.product.id`) needs no extra selection block.
- [ADR 0002](0002-workspace-storage.md) stays valid. A version-one workspace file
  and its backup are renamed to `workspace.v1.json` and `workspace.v1.json.bak`
  before the first save, so the first save never overwrites them. The application
  starts with the demo projects and tells the user that the old file needs the
  converter.
- Every crate changes. The work lands as one branch of small, individually green
  commits.
