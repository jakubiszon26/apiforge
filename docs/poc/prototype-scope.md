# API Forge — prototype v1 scope

Status: historical. This scope was accepted for the TypeScript prototype, which is now
the behavioural reference for the native Rust port ([RUST_MIGRATION.md](../../RUST_MIGRATION.md)).
The mock-up in `design-preview/` was an earlier interface experiment.

## 1. Goal and boundaries

A manual tester builds and runs REST API scenarios on a graph. The team defines its own
requests, data types, components and business actions. Product, cart and promotion are
only an example configuration of one project; neither the engine nor the interface
handles them specially.

Main flow: define a request → save it as a component with arguments → place it on the
graph → choose a matching action → connect data → add a check → run → inspect why the
result is what it is.

v1 design assumptions: a local single-user application, one open project, durable
local storage and JSON export/import. No JavaScript authoring required. (The prototype
UI was in Polish; the native application is in English.)

## 2. Two prototype stages

**P1 — validate the authoring model.** A working editor: drag and drop, connections,
request configuration, action menu, custom components, response preview and assertion
building. Results come from explicitly labelled sample data. This is the first review
point with the user.

**P2 — a useful API test.** The same saved graph performs real requests through a local
backend. Adds the report, partial runs, error handling and cleanup. Only P2 meets the
complete v1 acceptance criteria. A simulation never pretends to be a real run.

## 3. Editor

- Left library: built-in operations and components of the current project.
- Centre: a pannable, zoomable graph. Dragging blocks from the library, moving,
  connecting, deleting, duplicating, multi-selection and graph undo/redo.
- On a block: name, a short description, the arguments the author marked as important,
  named ports and execution state. For a request: method and path pattern. An `id`
  argument is an ordinary author-defined argument.
- Right panel: full configuration of the selected block or the inside of a component.
- Bottom panel: run progress, inputs, outputs, request, response and errors.
- A visible "Add action" button offers the same options as the context menu. A basic
  scenario can be built by clicking alone, without dragging.
- In v1 the user can choose one explicit set of scenario arguments. Case tables and
  loops are a later extension.

### Context menu and creating connections

Right-clicking a component, or dragging from a port into empty space, shows actions
compatible with the chosen output. With several outputs the user picks a specific one.

An action can be dragged from the menu onto the graph or clicked to add it next to the
source. Both create the block and the connection. A compatible input is bound
automatically; if several inputs match, the user picks one. Remaining required
arguments are visibly marked.

The menu combines universal operations with project actions. A component author decides
for which output type the component is offered and which of its inputs is fed. There is
no built-in knowledge of endpoint names or field meanings.

## 4. Custom components and data contracts

A component can wrap a single request or a graph of several operations. Selecting part
of the graph and choosing "Save as component" opens an editor for public arguments,
outputs and fields shown on the block. Connections crossing the boundary must be
explicitly mapped to public ports.

A definition contains a stable ID, name, description, revision, typed arguments with
defaults, the inner graph, output mapping and optional action-menu bindings. The editor
can change the inside and save a new revision. An instance pins a specific revision;
updates are applied explicitly to chosen instances. A saved test never changes
behaviour because the library was silently updated.

Base types: text, number, boolean, object, list, any JSON. A team can name an object
contract, e.g. `User` or `Product`, and describe its fields. In v1 action compatibility
follows from the base type or the project contract ID; there is no full JSON Schema
compatibility system.

Response structure comes from a manual description or a saved sample response. Structure
inferred from a single response is marked as a suggestion; an observed field never
automatically becomes a test requirement. OpenAPI import is deferred.

Nested components may use other saved components. Cyclic and recursive dependencies are
not allowed.

## 5. Built-in blocks

| Block                | v1 scope                                                                                                                                      |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| HTTP request         | Method, URL or base URL + path, path arguments, query, headers, JSON body, timeout. Outputs: status, headers, JSON or text body, duration.    |
| Select data          | Choose from a JSON tree or a simple path: field, nesting, list index. Distinguishes a missing field, `null`, empty text and zero.             |
| Build data           | An object or list assembled from literals and earlier blocks' values, to prepare bodies and arguments.                                        |
| Formula              | Addition, subtraction, multiplication, division, rounding and text joining. Arguments as values or bindings. Operations can be chained.       |
| Comparison/assertion | Equality, inequality, numeric comparison, numeric tolerance, existence, type, non-empty, containment. The result carries expected and actual. |
| Entry condition      | The same simple predicates, combined with "all" or "any". Not being met means invalid data and blocks dependent steps.                        |

Formulas are a record of permitted operations: no `eval` and no arbitrary JavaScript.
For price calculations the tester explicitly states rounding and tolerance; the engine
assumes no currency or business rules.

## 6. Bindings and execution

- An argument is a literal, a scenario parameter, an environment variable or a
  reference `nodeId + outputPort + path`.
- A connection carries the selected data and creates an execution dependency. A bound
  argument shows its source instead of an editable literal.
- Picking data from an earlier block creates an explicit binding and dependency. There
  is no invisible, overwritten global "current response".
- "Next" connections exist when ordering is needed without passing data. A manual Start
  marks the entry point; IF / ELSE chooses the Yes or No branch. Inactive steps are
  skipped. Older graphs without Start run every root.
- The graph is acyclic. A block waits for all required inputs. One input has exactly one
  source; joining two branches needs separate named inputs.
- v1 executes ready steps sequentially in a stable order; parallelism is a later
  extension.
- Every run uses a frozen copy of the graph, component revisions and parameters. Editing
  the graph does not affect an active run.

### Errors and continuation

We distinguish a failed assertion, an execution error, invalid input data, a blocked
step, a skipped step and a cancelled step. The report never marks blocked checks as
passing.

An HTTP 4xx/5xx response is still a response to test; its status can be asserted
explicitly. A timeout, connection failure or impossible data mapping is an execution
error. A missing field can be asserted, but it is never silently turned into zero in a
formula.

Scenario setting: stop after an error, or continue with the steps that can still run. In
the second mode a failed assertion keeps its data and later checks can still run. Steps
that need non-existent outputs stay blocked; independent branches may run. An entry
condition always blocks its dependants.

### Partial runs and cleanup

- "Run test" and "run up to the block" perform a fresh run; the latter runs only the
  ancestors and the chosen block.
- "Run this step" uses current literals or explicitly chosen inputs saved from a previous
  run. The UI shows their origin and time. Such a result is diagnostic and never passes
  the full test.
- HTTP operations are never retried automatically. Repeating a step may change data
  again; clicking run is a deliberate user choice.
- A separate "After test" section holds an ordinary cleanup graph, which runs also after
  an error or a controlled cancellation. A missing created ID explicitly skips the
  related step. A process crash cannot guarantee cleanup.
- A cleanup error is shown separately and never overwrites the original test error.
  Cancellation does not undo requests the external server already processed.

## 7. Assertions from responses and the report

Clicking a field in the response tree offers "Check…", "Use as argument" and "Expose as
component output". Adding an assertion fills in the path and the source. For a numeric
field, the expectation can be a value, another field or a formula result.

The result shows each step's status, inputs, response, duration and assertion details.
An error in a custom component points to the inner step. Full request/response is
available, with secret fields and authorisation headers masked.

Comparing two endpoints shows that they are consistent; an independent value or
formula is a separate reference. The UI never presents agreement between endpoints as
proof that a business rule is correct.

## 8. Environment and storage

A project stores scenarios, components, contracts and sample responses. Import and
export carry a format version and are validated. An unknown version or a missing
component definition gives a clear error. Project data survives a page reload; run
history may be session-only in v1, with manual report export.

An environment has a base URL, ordinary variables and secret values. Headers are
supported, including Bearer/API key. Secrets entered for the session never reach the
project export or reports; full responses may contain sensitive data, so exporting a
report is an explicit action. OAuth and automatic login flows are out of scope; a project
can contain an ordinary login request and bind its result.

Requests were performed by a local Express backend so that the client did not depend on
the tested API's CORS. The runner was reachable only locally, with request validation,
time and size limits and origin checks. Localhost and private networks remain valid test
targets. Public multi-user hosting needs a separate security design and is out of v1.

## 9. Deferred scope

Loops and variant tables, parallel execution, OpenAPI import, AI-generated tests,
arbitrary JS scripts, browser visual tests, CI and schedules, multi-user collaboration,
cloud environments, a marketplace, automatic retries and waiting for eventual
consistency. The format allows these to be added later without implementing them now.

## 10. v1 acceptance

1. A tester defines a request to any endpoint, any named arguments and a response
   structure without editing application code.
2. They save a component and place it on the graph, editing chosen arguments directly on
   the block.
3. They add an assertion from a response field by clicking; the data source and the
   connection are created automatically.
4. They save several steps as a custom action with arguments and outputs; the action is
   offered for a compatible data type.
5. They drag the action onto the graph and perform the equivalent operation by clicking
   alone.
6. They run a real scenario: read → create data → read again → compare → clean up.
7. They can tell an assertion failure, a network error and invalid data apart; the
   continuation policy behaves as described.
8. They save, reload, export and import the project without losing bindings or
   revisions.
9. Two demo projects from different domains use the same operations and differ only in
   project definitions. No engine or UI condition checks the name "Product" or a specific
   endpoint path.
10. Simulations, earlier responses and real runs are clearly distinguished.
