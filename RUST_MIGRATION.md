# API Forge: migration to Rust + egui

Status (19 October 2026): R0/R1 are under way and R2 has started. `apiforge-core` imports
and validates v1 documents and implements expression semantics (JSON paths, the 37
operators) verified against a corpus generated from the TypeScript reference.
`apiforge-desktop` is a native graph viewer. Editing, the runner and persistence remain to
be ported. There is no production release yet. This document supersedes the technology
assumptions of the earlier PoC plans; the TypeScript application remains the reference
implementation.

**Current priority:** feature parity with the TypeScript prototype first, refinement and
release hardening afterwards. Everything in the native application — code, documentation
and the user interface — is in English.

## Goal and agreements

The user chose **Linux and Windows from the first release**. We port the whole current
workflow: building tests from blocks, Start, IF / ELSE, request ordering, variables from
responses, nested operators, custom components and their revisions, copying
configuration, the report and cleanup. The engine stays domain-agnostic; arguments give no
special treatment to `id` names or endpoint names.

A release means a verified desktop package on both platforms, import of existing
projects, durable storage with recovery and documented error behaviour. Opening a window
and a few blocks does not close any functional migration stage.

We assume x86_64. Exact minimum Windows releases and Linux distributions will be recorded
in R0 after checking library and packaging requirements; we do not claim OS support based
on cross-compilation alone. macOS, ARM, cloud sync, plugins, loops, schedules and
auto-update are not part of the first release. Installing a newer version must still keep
data and allow restoring a database copy.

## Team and responsibilities

| Role        | Ownership                                                                          | Output                                                 |
| ----------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Coordinator | decisions, module boundaries, contracts, dependencies, integration, CI, acceptance | this plan and the order of work                        |
| Engine      | model, import, operator and execution behaviour                                    | [core audit](docs/migration/core-audit.md)             |
| Interface   | egui interactions, editor, user operations and report                              | [UI audit](docs/migration/ui-audit.md)                 |
| Reliability | storage, secrets, transport, release tests and distribution                        | [production audit](docs/migration/production-audit.md) |

Each file has one owner; Cargo files, the lockfile and shared contracts are changed by the
coordinator. A behaviour change needs a test and a specification update. We do not create
separate project models for the UI and the runner.

## Target architecture

One native application. egui handles the UI, and an asynchronous runner works outside the
paint thread. Express, a browser, a local control server and Node are not runtime
dependencies of the new release. The TypeScript tooling stays temporarily, only for
compatibility comparisons.

```text
crates/
  apiforge-core/          domain model, validation, graph, operators, v1 format
  apiforge-runner/        scheduling, HTTP, simulation, cancellation, cleanup
  apiforge-storage/       durable workspace storage, backups, import/export
  apiforge-desktop/       eframe/egui, canvas, inspector, dialogs, report
fixtures/compatibility/   v1 projects and expected semantic results
```

Core does not import egui, reqwest, a database or window logic. Runner and storage depend
on core but not on each other. Editing commands and history may start as a module of the
desktop crate and move into an `apiforge-application` crate once their interface is
stable; the desktop crate is where services are wired together. The shared report and
event model belongs to core. Trait organisation follows testing needs; we do not create
layers for abstraction's sake.

The file model is separate from the validated execution model. `NodeConfig` will not be a
single bag of options for every block kind: typed `Node`, `Binding`, `Predicate`,
`Operator`, `StepStatus` variants, newtype identifiers and a separate validation result.
The v1 DTO keeps existing names and layout; conversion to the typed model returns
diagnostics with the error location.

Durable state, editor state and run state are separate. Every editing command goes
through one dispatcher; drawing never mutates the project on the side. Undo and redo
cover atomic operations: pasting a group, changing connections and saving a revision.
Paste uses a frozen fragment, new IDs and remaps internal references, including inside
operator trees.

The UI sends Start/Cancel commands to the runner service. Events carry a RunId, a sequence
number and the full step instance path inside components. Channels are bounded; reports
are not cloned every frame and the final status is never lost under load. HTTP calls,
storage writes, parsing large data and migrations never block drawing. Closing the window
during a test has an explicit cancellation and cleanup path.

## Libraries and version choice

Target components: egui/eframe, Tokio, reqwest, serde/serde_json, typed errors and tracing
without secrets. The collection format must keep its required ordering; we do not swap JS
maps for sorting `BTreeMap` or randomly ordered `HashMap` without analysis.

The checked documentation shows egui/eframe **0.37.0** and a change to the `App`
interface (`ui` is required, next to `logic`). The local rustc is **1.99.0**. See the log
below for the chosen, mutually compatible set.

We use a pinned toolchain, Cargo.lock and limited dependency features. Dependencies and
licences are checked in CI.

## Compatibility contract

1. Import `Project.version = 1` and workspace exports from the current application; keep
   IDs, parameters, references, components/revisions, samples and IF connections. Source
   files stay untouched. The desktop application does not read browser localStorage: the
   existing interface must export the whole workspace and the desktop imports it.
2. Import without running requests. Secrets are not part of migration or a normal export.
   An invalid or newer format gives a message and never overwrites existing data. A format
   change has an explicit version number, a migration, a test and a copy taken before the
   change.
3. `Missing` is a different value from JSON `null`. Paths with fields, arrays and keys
   containing dots, brackets or Unicode must keep working.
4. Every current block kind and the 37 operators have ported tests. JS numbers, rounding,
   modulo, Unicode and field order need boundary tests; Rust's default behaviour is not
   automatically compatible. A contract change needs a deliberate migration, not a hidden
   "fix". (See [ADR 0001](docs/decisions/0001-numeric-semantics.md).)
5. Each request is awaited before the next. Start, reachability, IF false as a valid
   result, skipping the inactive path, required data at a join and the error policy each
   have compatibility cases. An older graph without Start keeps an explicitly described
   legacy mode. This is not a direct copy of n8n rules.
6. Immutable run input snapshots; changing the project does not change a running test.
   Full run, run up to a block and a single diagnostic step each have a defined data
   provenance. Redacted report data never replaces original values in runner
   calculations.
7. Cleanup has its own budget and uses results already obtained, also after a component
   error. A process crash cannot guarantee cleanup; after a restart the run is marked
   interrupted, without automatically repeating business operations. Manual cleanup needs
   available data and an explicit user request.
8. HTTP 4xx/5xx remain responses to check, not transport errors. No implicit redirects,
   retries or type conversion. Proxy/TLS configuration is explicit and tested on both
   platforms.

Existing tests are a base of cases, not proof that every historical implementation detail
is correct. The audits list discrepancies; for each we record a decision: compatibility, a
bug fix with a regression test, or a version change. Comparison snapshots normalise only
non-deterministic metadata (time, RunId); statuses, request order or values are never
removed to make tests pass.

## Corrections required during the migration

The audit does not treat the current engine as a flawless reference. Before implementing
the runner we close the following decisions in contract tests:

- Path activation and argument retrieval are distinct. Attaching a pure data producer
  must not by itself allow HTTP for its other consumers. Legitimate dependencies of
  existing projects need a defined migration mapping, without silently widening or
  narrowing requests.
- A single diagnostic step must not automatically clean up resources inherited from
  history. Each resource output has provenance; standard cleanup applies to the current
  run. Cleaning up a historical resource is a separate, deliberate action.
- History used for diagnostics is pinned to a fingerprint of the project, environment and
  component revisions. Redacted text never becomes a real token of the next request.
- Operator default arguments, observing Missing, formatting objects as text and numbers
  have one specification and one test set. We do not duplicate the inconsistency of the
  two current validators.

These corrections are R1/R2/R4 obligations, with concrete regressions described in the
core audit. They are not a claim that the bugs are fixed in the current version.

## Execution plan and dependencies

| ID  | Owner                     | Scope and dependencies                                                                                    | Acceptance condition                                                                                                                   |
| --- | ------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| R0  | coordinator + UI          | library set, OS support, ADRs, Cargo workspace, fmt/clippy/test/build CI; before further implementation   | the chosen stack builds and a window test runs on Linux/Windows; canvas choice closed; module boundaries documented                    |
| R1  | engine + coordinator      | v1 DTO, domain model, validator, export fixtures; after R0                                                | all current demo projects and negative imports; lossless round-trip; boundary sizes and depths                                         |
| R2  | engine                    | evaluator, paths, graph plan compilation, components; after R1                                            | type/operator/IF/dependency tests, determinism, Missing/null and JS–Rust cases                                                         |
| R3  | reliability               | storage, schema, atomic import, backup/recovery; after R1                                                 | no loss of a confirmed save on kill/restart, full disk and failed migration; no secrets in storage/export                              |
| R4  | engine                    | async runner, HTTP, simulation, cancellation, cleanup, events; after R2                                   | HTTP fixture: prices → cart → products, both branches, errors and timeouts, no parallel requests in one path; bounded resources        |
| R5  | coordinator + UI          | editing commands, undo/redo, document/session, storage integration; after R1, R3 integration              | validated atomic edits, configuration copies without aliasing, dirty/saved/error, transaction rollback                                 |
| R6  | UI                        | egui shell, library, canvas, ports, focus control, keyboard; after R0/R1/R5                               | full support for adding, connecting, selecting, moving, deleting, copy/paste and undo on both OSes                                     |
| R7  | UI                        | inspector, nested arguments, dragging variables, response picker, components; after R2/R5/R6              | today's flows reproduced without writing code; type configuration through Comparison; explicit value sources                           |
| R8  | UI + engine               | report, resize, internal errors, cancellation, history/export; after R3/R4/R6                             | report pinned to the right snapshot, responsive UI during HTTP, correct cleanup and diagnostics, redaction before storage/presentation |
| R9  | reliability + coordinator | native tests, performance, TLS/proxy, import hardening, packaging; prepared from R0, accepted after R3–R8 | all release gates below and CI artefacts on both OSes                                                                                  |
| R10 | coordinator               | user data migration, install/upgrade/recovery trial, acceptance of the whole product                      | Linux and Windows meet the same scope; migration and rollback instructions; only then is Rust the default version                      |

Wave 1: R0 and freezing contracts and fixtures. Wave 2: after R1, engine R2/R4, storage R3
and UI/commands R5/R6 in parallel. Wave 3: R7/R8 and integration. Wave 4: R9/R10. Earlier
stages may produce internal working increments, but they are not a production release
that skips the remaining gates.

Each task ends with code, tests adequate to the behaviour, a short decision record and a
list of remaining problems. A module compiling is never a reason to mark the whole
migration done.

## Production release gates

| Area                 | Required evidence                                                                                                                                                                                                                                                                                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Feature parity       | audit matrix closed; all current user flows ported; every deliberate divergence described                                                                                                                                                                                                                                |
| Engine               | tests of local HTTP, ordering, IF/joins, cancellation during connect/read/component, stop/continue, cleanup, diagnostics and stable step identity                                                                                                                                                                        |
| Data                 | round-trip tests of v1 and workspace, corrupted/new format, crash recovery, backup/restore, failed import/migration and package upgrade                                                                                                                                                                                  |
| Security             | parsing limits before expensive allocation, no code execution from projects, correct TLS verification, dependency/licence checks, test secrets never reach reports/logs/snapshots/storage                                                                                                                                |
| UI                   | egui_kittest for commands and interactions plus native window tests; focus, text vs block clipboard, keyboard, IME/Unicode, drag-and-drop, DPI and resize on Linux/Windows                                                                                                                                               |
| Accessibility        | control names and focus, a keyboard alternative for wires, screen reader evaluation on both OSes; a custom canvas is not automatically accessible thanks to egui                                                                                                                                                         |
| Performance          | measurements on recorded hardware with a release build: proposed 200 blocks/1000 wires p95 <= 16.7 ms and a synthetic 500/1000 profile p95 <= 33 ms while editing (the 500 profile does not change the v1 import limit); no UI stalls from HTTP and bounded report memory growth; thresholds fixed after the R0 baseline |
| Distribution         | native Linux/Windows CI: fmt, clippy with warnings as errors, tests, release build; install, start, upgrade and uninstall keeping data; checksums and a version/dependency manifest; artefact provenance verification                                                                                                    |
| Operations & support | rotated diagnostic logs, readable failure messages without sensitive data, backup/restore and migration instructions; no automatic replay after restart                                                                                                                                                                  |

Storage writes transactionally and confirms "saved" only after the write is durable.
Application shutdown, rollback and a full disk have tests.

Secrets are session-only by default, as today. Saving them in the OS credential manager
may come as a separate, tested feature; we never replace it with plaintext. A fully
encrypted store, cloud and automatic updates are not conditions of this migration.
Installer signing and release verification are part of R9; a publisher certificate is an
external dependency that no code change or CI alone can replace.

reqwest needs deliberate client configuration: whole-response and per-stage timeouts,
stream limits, proxy, TLS, redirect and retry policy. We do not rely on library defaults
in tests that create resources.
[ClientBuilder](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html).
Cancellation is signalled by a token, but the policy for finishing a request and starting
cleanup belongs to our runner.
[CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html).

Widget tests may use [egui_kittest](https://docs.rs/egui_kittest/latest/egui_kittest/),
which provides interaction tools and checks of accessible names. They do not replace
integration tests with a real window, clipboard and system dialogs. Graphic snapshots
complement semantic checks; they are never the only criterion.

## Work log

### First increment (R0/R1)

- Cargo workspace, toolchain 1.99.0, Cargo.lock, core without GUI dependencies and a
  native eframe frontend.
- Chosen compatible set: **eframe/egui 0.36.2**. A graph library (egui-snarl 0.12.0,
  which needs egui 0.36) was evaluated; the viewer uses a custom canvas over egui `Painter`
  instead. Moving to 0.37 is a separate, deliberate upgrade; we never mix two egui
  versions in public interfaces.
- Project and workspace v1 import is limited to 16 MiB. The DTO keeps map order,
  `OptionalJson` distinguishes absent from null, unknown fields and duplicate keys are
  rejected instead of lost. The current format keeps the 200 blocks / 1000 edges per graph
  limits.
- The validator checks bindings (including nested ones), parameters, ports, data/control
  cycles, IF, Start and component revisions/recursion.
- The egui viewer shows blocks, data/control wires and both phases. Positions changed in
  the viewer are view state and are not saved. No request is executed during import. Import
  is transactional from the UI's perspective: a failed file keeps the current document.
- Corpus: four projects and a workspace, a SHA-256 manifest and a TypeScript generator
  with `--check`. Node is only used to regenerate the comparison corpus, not to start the
  Rust binary.
- Google Antigravity (`agy`, model `gemini-4-argon-medium`) got two bounded tasks: CI and
  the fixture generator. The coordinator fixed a wrong type import and the manifest
  sorting and ran the verification.
- CI defines build/test/clippy and artefacts for Linux and Windows; adding the YAML does
  not mean the jobs ran on remote runners.

### Expression semantics (R2)

- `apiforge-core::js`, `path` and `operator`: JavaScript-compatible equality, `String()`
  conversion and number formatting, v1 field paths and the 37 operators with typed errors
  and an injected RNG.
- A differential corpus of ~21k cases generated from the TypeScript sources
  (`fixtures/compatibility/semantics/`) caught a one-ULP number parsing defect
  (`serde_json` without `float_roundtrip`) and 17-digit formatting differences. Accepted
  divergences are recorded in [ADR 0001](docs/decisions/0001-numeric-semantics.md).
- `apiforge-core::template`, `predicate`, `run`, `demos` and `edit`: URL/header/query
  template expansion, assertion/condition predicates, run report DTOs, English demo
  projects and pure board editing operations (adding/deleting/duplicating/pasting blocks,
  connecting wires, component extraction/revisions and response variable assignment).

### Local storage and demo API (R3)

- `apiforge-storage`: durable workspace persistence with atomic writes, backup recovery,
  removal of secret values on save/export (`public_project`) and legacy demo URL migration.
- `apiforge-demo-api`: standalone local HTTP server (axum/tokio) for the shop and ticketing
  demo projects, bound strictly to loopback interfaces.

### Execution engine and differential run corpus (R4)

- `apiforge-runner`: topological execution planner with Start reachability and IF branch
  activation (`plan`), live HTTP transport with redirect/size/nesting limits (`http`),
  multi-form secret redactor (`redact`), serial execution engine (`engine`) and background
  Tokio service (`RunnerService`, `RunHandle`).
- Differential simulation run corpus (`fixtures/compatibility/runs/`, `tests/run_corpus.rs`)
  generated by the TypeScript reference runner (`scripts/run-fixtures.ts`) and verified in
  Rust.

### Native desktop editor, inspector, dialogs and run panel (R5–R8)

- `apiforge-desktop::editor`, `persistence`, `canvas` and `library`: undo/redo history with
  edit merging, background workspace autosave, interactive node/wire canvas with pan/zoom,
  inline literals, drag-and-drop and context/action menus, and sidebar navigation with
  operator/variable palettes and built-in examples.
- `apiforge-desktop::inspector`: step configuration, recursive argument/operator tree
  editor, response field picker, keyboard-accessible ordering connections and output tree.
- `apiforge-desktop::dialogs`: modal editors for environment and session-only secrets,
  project/scenario settings and contracts, response variable assignment and component
  creation/revisions.
- `apiforge-desktop::report` and `RunnerService` integration: full, run-until-selected and
  single-step execution with previous-run input reuse, cancellation, staleness detection on
  graph edits, hierarchical step list with component children and cleanup phase, expected vs
  actual comparison, collapsible output/input/HTTP request trees with drag-and-drop and
  quick assertion/variable actions, and redacted JSON report export.

Instructions for running the current code: [docs/migration/development.md](docs/migration/development.md).
