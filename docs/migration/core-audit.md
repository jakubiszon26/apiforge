# Engine audit before the Rust migration

Source state: 19 October 2026. The first release covers Linux and Windows equally. The
audit reads the implementation and existing tests; it does not confirm that those tests
were run. Points needing clarification are kept apart from behaviour users already rely
on.

## Sources of truth and the compatibility boundary

The project format is described by `shared/model.ts`, bindings and dependencies by
`shared/bindings.ts` and `shared/graph.ts`, operators by `shared/operators.ts`. Actual
execution lives in `api/runner/index.ts` and `api/runner/control.ts`. Browser import is
handled by `app/features/workspace/storage.ts`.
[implementation-contract.md](../poc/implementation-contract.md) helps with orientation but
is not sufficient as a production specification.

Keep existing projects, component configurations and the meaning of results. Do not copy
bugs, accidental JS dictionary ordering or the limits of browser storage as native
architecture. Every deliberate semantic change needs a decision, a migration and a
comparison test.

## Feature and compatibility matrix

| Area                | Working behaviour to port                                                                                | Rust acceptance criterion                                                                   |
| ------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Document            | `Project.version = 1`; scenarios, components, contracts, environment; node positions                     | Real v1 files import without losing configuration; re-export keeps the document's meaning   |
| Nodes               | `trigger`, `if`, `request`, `select`, `build`, `formula`, `assert`, `condition`, `component`, `variable` | Every variant has an explicit configuration type, validator, ports and executor             |
| Arguments           | JSON literal, node/port/path reference, parameter, environment, nested expression                        | An argument can be any JSON; IDs get no special privileges                                  |
| Data connections    | Derived from all reference leaves, including inside expressions                                          | One source of truth: bindings; no second, diverging list of data edges                      |
| Control connections | `Graph.edges`: source, target, optional `true`/`false` branch                                            | Cycles detected across control and data dependencies together                               |
| Start               | Manual trigger; at most one per graph, no inputs                                                         | Only the active path executes; a disconnected request sends no HTTP                         |
| IF                  | The same basic predicates as Comparison; a `false` result is still `passed`                              | Only the chosen branch executes; other steps get `skipped`                                  |
| Comparison          | One `assert` block, with the type predicate in the same block                                            | Type, value, tolerance and missing fields handled without a separate type block             |
| Legacy condition    | `condition` returns `blocked` when not met; also a predicate list with `all`/`any`                       | Import compatible; the UI distinguishes a blocking condition from IF                        |
| Request             | HTTP/HTTPS; URL, query, header and JSON body arguments; simulation and real execution                    | Local server tests confirm method, encoding, body, ordering and no retries                  |
| Response            | Ports `body`, `status`, `headers`, `duration`; text if the response is not JSON                          | 4xx/5xx stay data for IF/assertions; a transport failure is `error`                         |
| Variable            | `inputs.value` → `outputs.value`, type preserved; assigning a response field is a reference              | The next run reads a fresh field, never a value remembered by the editor                    |
| Field selection     | Object/array paths, quoted keys, Unicode, empty path = whole document                                    | Tests for `null`, `false`, `0`, empty text, empty keys and keys with dots                   |
| Build data          | Named inputs form an object or array                                                                     | Array element order is explicitly defined and does not depend on a Rust map                 |
| Operators           | One catalogue drives ports, form, validation and interpreter                                             | All 37 operators have case and error tables                                                 |
| Components          | Definitions pinned to `(id, revision)`; own parameters and local outputs                                 | Changing a revision does not change existing instances; recursion rejected                  |
| Component boundary  | Typed ports, defaults, output bindings also literal/parameter/env/expression                             | Extraction keeps every nested binding and IF control                                        |
| Cleanup             | A separate graph after the main run, also after error/cancellation                                       | Deletes a created resource even after a later error; never deletes a resource without an ID |
| Diagnostics         | Full run, up to a chosen step, single step with historical outputs                                       | A diagnostic result marks historical data explicitly and does not pose as a full run        |
| Report              | Step statuses, phase, inputs/outputs, request, actual/expected, nested results                           | Events and the final result agree on status; data is redacted before publication            |
| Editing             | Copying configuration, repeated paste, remapping internal references, keeping external ones              | Copies are independent; IF branches kept; no second Start is created                        |
| Data schemas        | A contract is currently an ID + sample JSON, not a full JSON Schema                                      | Import does not promise schema validation the old engine never performed                    |

Operator catalogue: `add`, `subtract`, `multiply`, `divide`, `modulo`, `random`, `round`,
`equals`, `notEquals`, `greater`, `less`, `atLeast`, `atMost`, `and`, `or`, `not`, `concat`,
`letter`, `length`, `contains`, `abs`, `floor`, `ceiling`, `sqrt`, `sin`, `cos`, `tan`,
`asin`, `acos`, `atan`, `ln`, `log`, `exp`, `power10`, `typeOf`, `exists`, `field`.

## Exact execution semantics

### Values and expressions

A missing field is represented by JS `undefined`, while JSON `null` is a real value. Rust
must use a distinct representation; `Option<Value>` correctly distinguishes `None` from
`Some(Value::Null)`. `serde_json::Value` indexing, which returns a substitute `Null`, must
never be used to resolve paths.

`exists` is true for `null`, `false`, `0` and `""`, false only for Missing. `notEmpty`
rejects Missing, null, empty text and empty object/array; it accepts `false` and `0`.
Value comparisons are deep and without type conversion. Type distinguishes null, array
and object. Numeric and logic operators never convert text, null or bool. An infinite
result is an error. Port defaults replace Missing, never null.

Expressions are evaluated eagerly; `and` and `or` evaluate both sides. A reference hidden
in nesting still creates a graph dependency. `field` can return Missing. `letter` is
1-based, path array indices are 0-based; text `length` and `letter` count Unicode code
points, not UTF-8 bytes or graphemes.

### Ordering and branches

Operations execute serially and each is awaited. A node waits for all its data and
control dependencies to finish; when several are ready, `graph.nodes` order decides. This
is not a parallel scheduler. The four concurrent runs allowed by the API are a separate
matter.

A graph with Start computes reachability along dependencies, admitting pure
`variable/select/build/formula` arguments as predecessors. Disconnected requests and
components must not run just because the active path wants their data. A graph without
Start keeps the legacy behaviour: all roots run.

IF produces `passed: boolean` and always has status `passed`, unless evaluating the
condition itself raises an error. An edge leaving IF needs `branch: "true" | "false"`;
other sources must not have that property. The unchosen path propagates `skipped`, not an
error.

Merging alternative control edges acts as an OR of activation, but the scheduler waits
for all predecessors. A skipped alternative does not block the active one. An error of an
active predecessor is not hidden by another one succeeding. A data reference to a skipped
node is still an unavailable dependency; OR applies to control only.

A false `assert` gives `failed` but keeps its outputs and may precede later operations
under `continue`. `condition` gives `blocked` and blocks its descendants.
`error/blocked/skipped/cancelled` of a predecessor normally blocks main-phase descendants.
`errorPolicy=stop` skips the rest of the main graph after failed/error/blocked, without
skipping cleanup.

### Components, cleanup and diagnostics

A component has its own output map and arguments instead of scenario parameters. The
environment stays shared. Status aggregates children with priority cancelled → error →
blocked → failed. Even a component that ended with an error can export IDs it already
created.

Cleanup always starts in `finally`, gets its own 10 s budget independent of main-run
cancellation, and uses main-phase results. A missing referenced resource field skips the
cleanup step. An available exported component resource may be used despite a later
error. An ordering edge in cleanup essentially means "after completion", not "only after
success". IF and Start reachability in cleanup need separate tests, because general
activation and cleanup exceptions overlap.

"Up to step" selects the closure of all dependency ancestors. "Only step" loads main
results of an earlier report and bypasses its own activation gate; it does not bypass
gates of a nested component. Both modes still run cleanup. History is already redacted,
so it cannot guarantee reproducing secret arguments; it must never be treated as a cache
of correct execution data.

User cancellation gives a final `cancelled`. Exceeding the 120 s main budget is a run
error. A failed assertion or cleanup error results in a final `failed`. `skipped` alone is
not a failure. A single-step diagnostic stores the description of history use in
`run.error`, without treating that description as an error — Rust needs a separate
warning/provenance field.

### HTTP and data protection

The base URL is a prefix including its path. A request `:argument` encodes a segment;
`{{argument}}` handles text and falls back to the environment. A whole token in a JSON
body keeps its JSON type; a token embedded in text produces text. Query is encoded by the
URL API. Redirects are rejected, credentials in the URL are forbidden, there are no
automatic retries. GET/HEAD with a body is an error. Rust must never implicitly retry a
POST that creates a cart.

Current limits: request 10 s by default, 60 s maximum; response 2 MiB including
streaming, response JSON at most 100 levels deep; main run 120 s, cleanup 10 s, 1000
executed steps including children, report 16 MiB. These are starting points for product
configuration, not a justified SLA. The current report limit counts nested data several
times; the new counter needs one precise definition.

The engine keeps raw values internally and publishes redacted copies. Redaction knows
session secrets and declared names, and detects conventional keys
(`authorization/cookie/password/secret/token/api-key`), URL credentials, query tokens and
Bearer values. It also masks URL/form encodings. It covers the snapshot, inputs, outputs,
errors and callbacks.

Detecting a secret only in a later response cannot undo a leak in an earlier callback.
Name heuristics do not protect an arbitrary secret. Production redaction needs explicit
secret classification before publication, limited raw logs and negative tests of every
channel, including `tracing`, crashes and disk writes.

## v1 import and persistence

1. Handle a single-project file and the workspace wrapper `{version:1, activeId,
projects}`. A workspace needs a non-empty list, unique project IDs and an existing
   `activeId`.
2. Read existing camelCase and binding tags exactly; keep revisions, node order,
   positions, `templateInputs`, nested references, branches and arbitrary JSON literals.
3. Separate the v1 DTO from the validated domain model. A new enum configuration must not
   force a change of the old wire format.
4. Run structural and semantic validation before replacing the open document. A rejected
   import changes neither the current project nor autosave. Errors include the JSON path
   and node ID.
5. Keep the absence of Start as an explicit compatibility mode. Do not insert a Start
   automatically, which would change the number of requests. Converting to the new flow
   is a separate edit with undo.
6. The current importer only moves the historical demo
   `localhost/127.0.0.1:3000/demo/(shop|support)` to port 4318. In the native application
   the demo endpoint may not exist; use a dedicated demo migration rather than rewriting
   addresses of users' own APIs.
7. `publicProject` removes values only for names in `environment.secretNames`; an ordinary
   project may still contain literals with credentials. Define the production public
   export separately from local storage. Session secrets never reach the clipboard or a
   project file.
8. The native application cannot read the browser's `localStorage`. Provide export from
   the old application and import in the new one; leave the original file untouched and
   keep a copy before a version migration.

Current import limits: up to 100 scenarios, 200 component definitions, 200 contracts, 200
nodes and 1000 edges per graph, 100 inputs/ports, up to 12 levels and 200 elements per
binding, text up to 100000 JS units. The two existing validations have different JSON
depth rules; unify them into one limit validator. `serde` must not silently drop unknown
configuration fields and export an incomplete document; either keep extensions or report a
warning/refusal with the full path.

## Proposed crate boundaries

| Crate / module         | Responsibility                                                                                                                                           | Must not depend on              |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- |
| `apiforge-core`        | Domain: IDs, bindings, typed configurations, ports, statuses, errors, JSON paths, pure operators; v1 DTO, migrations and validation in a `format` module | egui, Tokio, HTTP, files        |
| `apiforge-runner`      | DAG compilation, IF activation, execution, components, cleanup, limits, events, cancellation; HTTP adapter as a separate module                          | egui and global UI state        |
| `apiforge-storage`     | Atomic document saves, backups, history, secret store adapter                                                                                            | egui as a domain format         |
| `apiforge-application` | Editor commands and undo/redo; session, runner and storage coordination, public state for the UI                                                         | egui and renderer details       |
| `apiforge-desktop`     | egui, graph, forms, clipboard, report and Linux/Windows window integration                                                                               | internal network/secret details |

A CLI/headless runner may be a separate binary frontend using the same
application/runner, once it is in release scope. The Express local server need not be
ported: the native frontend talks to the async Tokio runner and reqwest transport over
internal channels. Small adapters may start as modules while keeping the dependency
direction and public contracts. Production readiness does not come from the number of
crates. The engine should take a validated plan snapshot and a `RunContext` with injected
transport, clock and RNG, and emit typed events through a bounded channel. The UI must not
perform blocking I/O in an egui frame.

## Implementation order and concrete acceptance tests

1. **Freeze the contract.** Export fixtures of existing projects: shop, tickets,
   operators, cart workflow, components with revisions and copies. Add expected traces
   with ID, status, active branch and results; omit random IDs and times. Node stays the
   comparison reference until the migration is done.
2. **Domain and import.** Implement DTOs, validation, paths and operators. A shared corpus
   runs TS and Rust. Check Missing/null/false/0/empty keys, boundary numbers, Unicode,
   nested bindings, a missing revision, mixed cycles and recursion. Fuzzing the
   parser/importer must end in a controlled result, without panics or unbounded
   allocation.
3. **Planner and control.** Reversed node order still gives prices → cart → add, with at
   most one request in flight. IF true/false sends only the active branch's requests.
   Test joins, an error in one active alternative, an inactive reference and a
   disconnected HTTP sharing a pure argument. Keep the legacy graph without Start.
4. **HTTP and cleanup.** A local fixture checks typed body and URL, 4xx as data, a text
   response, timeout, cancellation while reading, overstated/understated/missing
   content-length, the stream limit and redirects. The cart scenario must create, add
   after creation finished, compare the total and actually delete the resource even after
   an assertion error.
5. **Components and diagnostics.** Test pinned revisions, an ID output after a later child
   error, the depth limit, partial execution and the provenance of historical values. A
   nested IF also works when the parent runs as a single step.
6. **Security boundary.** Canonical and encoded secrets never appear in events, reports,
   `tracing`, exports or crash files. Debug output of sensitive data hides text by
   default. A redacted report is never a source of real secrets for the next run.
7. **Editor commands.** Copy/paste/duplicate/delete/connect/extract component as atomic
   commands. Test independence of repeated copies, remapping only internal leaves,
   keeping revisions/branches and undo/redo when the document changes.
8. **Persistence and release.** The overall plan chose SQLite: transactions, schema
   migrations and a consistent backup covering an active WAL. File export uses a temporary
   file and a safe replace. Test interrupted saves, corrupted data and backup recovery. An
   integration test runs the same project through the GUI and directly through a headless
   runner. CI and acceptance cover Linux and Windows from the first release: save/recovery,
   session secrets (keyring only if added as a separate feature), clipboard, TLS/proxy,
   cancellation and native installers. Test atomic replace semantics on both systems
   rather than assuming Unix file operations are portable. The pipeline requires
   formatting, clippy, tests, dependency scanning and signed/reproducible artefacts for the
   chosen target platform.

Existing suites to reproduce: `tests/runner.test.ts`, `tests/workflow.test.ts`,
`tests/operators.test.ts`, `tests/variables.test.ts`, `tests/project.test.ts`,
`tests/runner-api.test.ts` and the graph operation, component extraction, JSON path and
redaction tests under `app/features/`. Rewriting test names is not enough: the observed
fixture requests and the absence of forbidden side effects are what matters.

## Decisions and weaknesses to close before release

- **Reachability through data.** The current planner walks successors of data and control
  dependencies and adds pure arguments backwards. A shared pure producer can therefore
  extend reachability to another HTTP consumer. The production model must separate
  control activation from data needs explicitly; add a test before treating "a
  disconnected request never runs" as satisfied.
- **Joins and ordering.** Activation OR does not mean "first finished wins"; today we wait
  for all. Define the alternative join and a possible future AND join explicitly. Keep
  serial execution as the default; parallelism needs a separate explicit block/setting and
  side-effect tests.
- **Numeric values.** JS uses IEEE-754; Rust/serde may distinguish integer and float
  differently from the old equality. Decide `1`/`1.0` compatibility, the safe integer
  range, rounding of negative numbers, `-0`, modulo and exponent notation. Monetary prices
  need an explicit decimal/minor-units decision, without silently changing old formulas.
  (Decided in [ADR 0001](../decisions/0001-numeric-semantics.md).)
- **Strings and arrays.** `concat` uses JS `String` also for objects/arrays, and an array
  `build` uses JS key order. Rust needs compatible formatting or a migration. The 100000
  "character" limit counts UTF-16 in JS, not UTF-8 bytes; name the unit of limits.
- **Inconsistent default arguments.** A formula node fills default ports before execution;
  a direct expression with a missing argument fails. `typeOf` has a required argument, so
  despite the `jsonType(Missing)` helper Missing cannot currently be observed. Define
  deliberate rules and diagnostics instead of changing them accidentally during the port.
- **RNG and repeatability.** `random` uses `Math.random`; tests and replay should take an
  RNG/seed and the report should record the seed. Do not introduce implicit determinism
  into import without defining expectations.
- **History and diagnostic authorisation.** The previous report is validated mostly by
  scenario/mode, without a project fingerprint. Editing a URL or revision can mix data
  from an old plan with a new one. This needs a snapshot hash and explicit provenance.
  Cleanup after a diagnostic run may delete a historical resource — a deliberate policy
  must distinguish resources created in the current run from inherited ones.
- **Redaction is not data validation.** Do not replace secrets with text in the working
  model, because that changes request behaviour and types. Secure storage, debug output
  and export separately; key names are not a complete classification system.
- **Desktop networking.** An API testing application must support localhost and private
  networks. Do not copy a global ban on those addresses as SSRF protection. At the same
  time define a policy for imported untrusted projects, proxy, TLS/custom CA,
  authentication, cookies and credential transfer. There is currently no explicit cookie
  jar and no retry; adding them changes tests.
- **History and process interruption.** The API keeps only a session report (up to 100 /
  32 MiB), with no persistence or recovery. Cleanup cannot run after SIGKILL or an OS
  crash. Production needs a durable journal of created resources and a controlled cleanup
  resume; promising "always cleans up" would be untrue.
- **Validation consistency.** The runner and project validators partly duplicate rules.
  Errors are strings, `NodeConfig` is a loose bag of options with fields possibly unused
  by a given kind. Rust should validate once into a typed model and return stable error
  codes with context, without `unwrap` on untrusted documents.
- **Scale and responsiveness.** The 200 nodes/graph and 1000 step limits do not replace
  measurement. Define the frame budget, memory and time to open a large document; the
  event queue needs backpressure, and report virtualisation must not keep many full copies
  of responses per frame.
