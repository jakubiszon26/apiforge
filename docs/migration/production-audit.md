# Release requirements audit for Rust / egui

Status: a requirements and acceptance plan, not a claim of production readiness. The
audit covers the code in the repository on 19 October 2026. The target is a single-user
desktop application. The user confirmed Linux and Windows for the first release; both
platforms need native CI, artefacts and installation tests. Proposed Windows scope: 10/11
x86_64; the exact supported Windows releases and the Linux distribution and minimum glibc
need approval before packages are built. macOS is out of the first release.

## What exists and must be kept

| Area            | Evidence in the current code                                                                                                           | Consequence for the migration                                                                                                                                                  |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Project storage | `app/features/workspace/storage.ts`, `Workspace.tsx`: JSON v1 in localStorage, validation, export, 350 ms debounce, flush on page exit | Import v1 without losing configuration; browser and desktop share no storage. Explicit export from the old application and import in the desktop are required.                 |
| Migrations      | The current migration changes the old local demo address; an unknown version is rejected                                               | Explicit, tested format migrations, a copy of the source and safe behaviour for a newer version are needed.                                                                    |
| Secrets         | `publicProject` removes values of declared secrets; session values stay in memory                                                      | Do not move tokens into egui settings, the project, the block clipboard or reports.                                                                                            |
| Report          | The runner masks sensitive names and known values; the frontend masks export additionally                                              | Keep central redaction before events are published. Name heuristics and value search do not guarantee removing every transformation of a secret or personal data.              |
| HTTP            | `api/runner/index.ts`: HTTP/HTTPS, no credentials in URLs, no redirects or retries; 4xx/5xx stay data                                  | Keep the semantics and expose the supported transport configuration explicitly.                                                                                                |
| Limits          | Request timeout 10 s by default, 60 s maximum; 2 MiB response; main budget 120 s and cleanup 10 s; graph/report budgets                | Move limits into a typed execution policy and enforce them while streaming and decoding too. A Content-Length limit alone is not sufficient.                                   |
| Ordering and IF | `api/runner/control.ts`, workflow tests: Start, serial DAG, branch choice, data dependencies                                           | The execution contract is a product feature. Rewriting in async Rust must not implicitly introduce parallel requests.                                                          |
| Cancellation    | Main honours the signal; cleanup has an independent budget. A process crash cannot guarantee cleanup                                   | Keep separate cancellation and cleanup states; do not promise undoing a request's effects.                                                                                     |
| History         | `api/routes/runs.ts`: session memory, 4 active runs, up to 100 finished, 32 MiB threshold                                              | Define an explicit desktop limit/retention. The current memory threshold does not hard-limit the size of all active runs.                                                      |
| Tests           | Model, runner with a local server, workflow, secrets, components, Playwright for the UI                                                | Keep the fixture set as an oracle; do not claim desktop coverage from browser tests.                                                                                           |
| Releasing       | typecheck/test/build scripts; the existing Dockerfile is for the Node application                                                      | No proven desktop release pipeline. The Dockerfile does not copy `api`/`shared`, and `npm start` needs `tsx` from devDependencies; it is not a working Rust distribution path. |

## Trust boundary and explicit security scope

A project imported from a file is untrusted. Importing, previewing and migrating must
never send requests, resolve secrets or replay cleanup. Validation covers file size,
element counts, JSON and expression depth, duplicate IDs, number ranges, ports,
references, revisions and cycles. The editor never executes arbitrary JS/Rust or system
commands from a project.

An API testing runner deliberately reaches localhost and private networks; blocking them
globally would prevent the basic use case. A local release trusts the user who runs a
scenario. The interface shows Live mode, the environment and the target host; running
needs an explicit action. A project may contain a full URL bypassing the base URL, which
must be visible when reviewing configuration. A public or remote multi-user runner needs a
separate authorisation and network isolation model and is out of this release.

The native application should run the runner without an open HTTP port: in-process
messages and channels, separating the UI from the runtime. If a local daemon turns out to
be necessary, its authentication, IPC, permissions and lifecycle become a separate
blocking task; do not carry over the current public localhost contract without analysis.

## Release gates

Each gate needs evidence from CI or a recorded test protocol on the target platform. An
unmet gate stops the stable release; finishing the code alone does not close a task.

### G1 — data compatibility and recoverable storage

1. Define the project document version and the application settings version separately.
   Read the version before deserialising the actual model. Open a newer version with a
   clear error without modifying the original.
2. Save projects as portable, versioned JSON documents. Keep UI state and the recent-files
   index separately; never mix secrets with `eframe` state.
3. File save: validate the model → temporary file in the same directory → flush/sync →
   atomic replace → sync the directory where the platform allows. Keep the last valid copy
   and a recovery procedure. Handle missing space, permissions and interruption; after an
   error the UI still shows unsaved changes.
4. Keep autosave/recovery under the document identifier, with user permissions. After a
   crash offer recovery, show the date and source, and never overwrite the file without
   the user knowing.
5. Two instances editing a file must detect the conflict through a lock or a
   revision/hash; no silent loss of the last save. External file changes must be detected
   too.
6. The v1 migration imports every block kind, nested bindings, template inputs,
   revisions, contracts, Start/IF, cleanup and positions. Secrets declared in the old file
   are removed from the model meant for saving and never printed in validation errors.
   Keep the source file; migrations never contact the demo API.

Evidence: golden fixtures from current examples and boundary projects; import → save →
reopen preserves semantics; tests for corrupted/truncated JSON, a future version,
interruption at each save step, conflict and disk full (via fault injection). Recovery
checked on a real Linux and Windows filesystem, accounting for differences in atomic file
replacement and locking.

### G2 — secrets, privacy and reporting

1. Secrets are session-only by default. A durable credential store is an optional later
   scope; if it enters the release it needs the system keyring, explicit consent to save,
   handling of a locked/missing keyring and no silent plaintext fallback.
2. `SecretRef`/`SecretValue` types and one resolver separate the project model from
   credentials. A missing secret gives a diagnostic naming it, without its value. `Debug`,
   `Display`, errors and tracing must not reveal values.
3. UI events, export and durable history go through a shared redaction mechanism. Raw
   request/response is needed for the runner's current calculations but must never end up
   by accident in a log, a task's `Debug` or a panic payload. Sensitive headers are masked
   regardless of secret declarations.
4. Handle known values and their URL-encoded forms; test a secret echoed in any field,
   error, URL, query, headers, body and inside a component. Support extra response field
   paths to mask and a report mode with metadata but no payloads.
5. Session history and, by default, no telemetry or persisted payloads. Any history
   storage needs an explicit retention setting, a space limit, deletion and a warning about
   sensitive response data. The technical log contains identifiers and error codes by
   default, not bodies or full URLs with query.
6. Document limitations: an arbitrarily encoded/encrypted/combined secret and PII under an
   arbitrary field cannot be detected automatically. Memory dumps and logs of the tested
   API are outside the application's redaction guarantee. Do not promise cryptographic
   erasure of all in-memory copies.

Evidence: canary secrets across the matrix of locations and transformations; inspection
of exports, autosave files, logs and events; no values in the block clipboard. A separate
test prevents confusing redacted history with raw data for executing a single step.

### G3 — predictable HTTP in the user's environment

1. Keep: HTTP statuses are data, no automatic retries or redirects, no cookie jar shared
   between independent runs. A POST must never be retried automatically after a timeout.
2. TLS verifies the certificate and host name. Do not add a global "accept invalid
   certificates". The release needs a defined trust store policy: system CAs plus an
   explicit extra CA per environment, without mutating the system store. Local CA tests
   cover an expired certificate and a wrong host name.
3. Proxy must be explicit and visible: direct, or configured per environment, including
   `no_proxy`; never inherit a process proxy by accident or leak its credentials in logs.
   If proxies are initially unsupported, release that as a documented unsupported
   configuration, without a fake UI field.
4. Separate connect and whole-request limits; cancellation must also interrupt reading a
   slow body. The size limit covers the decoded response and data passed to the parser,
   plus the request body, header count/size, JSON depth and the report budget. A limit
   error keeps metadata without publishing a partial sensitive body.
5. Visibly distinguish DNS, connect, TLS, timeout, cancellation, limit exceeded, invalid
   JSON and configuration errors. Transport error text goes through redaction.
6. Define and show the rules for charset, compression, non-JSON responses, duplicate query
   keys and empty bodies. The migration keeps the current meaning of the base URL as a
   prefix and typed body interpolation.

mTLS clients, OAuth flows, multipart upload, WebSocket and redirect/retry support may stay
out of the first release. An extra CA is required for the declared profile of working
with internal APIs; if the profile is narrowed to public CAs, the criterion must be changed
explicitly before implementation, not skipped at acceptance.

Evidence: only controlled fixture servers for HTTP/TLS/proxy, chunked/slow/compressed
responses, invalid JSON, certificates and interruption after a POST was accepted. A
connection smoke test on the acceptance machine; no test depends on a public API.

### G4 — execution, cancellation and cleanup

1. The UI never blocks on network, parsing or the filesystem. The runtime communicates
   through bounded queues; a large/active report must not cause unbounded buffering or
   freeze the Cancel button.
2. A run works on an immutable snapshot of the graph, environment and component
   revisions. Every event has the run ID and a stable step identity with the component
   instance path. Editor changes do not change the active run.
3. Within a scenario execution is serial and deterministic. An unchosen branch sends no
   requests; missing data from a skipped branch does not rerun it. Older scenarios without
   Start keep an explicit compatibility mode.
4. The number of concurrent runs has a hard limit (initially keep at most 4), and queues
   are bounded. Data and report buffering needs an overall budget, not only per response.
5. A state machine defines exactly one final state and the status of each step.
   Cancellation stops starting further main steps and starts cleanup with an independent
   budget. Repeated Cancel is idempotent; the UI shows that cleanup is still running.
6. Cleanup also runs after a component error and a timeout; it operates only on available
   current data. A cleanup failure does not mask the original error and is shown
   separately. Closing the window during a run offers finishing cleanup or explicitly
   terminating the process.
7. No mechanism guarantees that a request cancelled after sending was not executed by the
   server. After a hard crash, a journal marks the run as interrupted with unknown side
   effects; it never resumes requests or cleanup automatically. The user gets context for
   manual verification.

Evidence: TS/Rust conformance fixtures for workflows and all operators; cancellation at
every stage (including TLS, body and cleanup); closing the application, large reports, a
conflict between editing and a run, repeated Cancel. Cart test: prices → IF 200 → create
an empty cart → add products → compare the total → delete the cart, while the 404 branch
creates no cart.

### G5 — tests and the release pipeline

1. The Rust workspace has a pinned toolchain, MSRV, lockfile, `fmt --check`, `clippy`
   with a chosen warning limit, unit/integration/doc tests, dependency and licence analysis
   and a release build in CI. Lockfile updates go through a controlled process, never by
   accidentally fetching the newest versions in a release.
2. Domain/runner are tested without egui. Fixture parity covers null/missing/false/zero,
   numbers and Unicode, seed-controlled random operators, template aliases, snapshots,
   revisions, IF merge, cleanup and importing the older format. Rust/JS semantic
   differences must be documented and approved by a test.
3. Property/fuzz tests cover the parser/import, references and cycles, JSON paths, ID
   remapping on copy, migrations and the redaction policy. Fuzzing does not replace manual
   security analysis.
4. Automated desktop UI scenarios cover creating a graph, dragging a variable, choosing a
   response field, IF, copying a group with configuration, undo/redo, report resize,
   save/reopen and cancellation. External browser tests remain tests of the old UI.
5. The release artefact is built by CI, has checksums, a version/toolchain manifest and a
   licence list; it runs on a clean machine without Node, npm or the repository. Linux and
   Windows have separate native CI jobs and installation, start, upgrade and uninstall tests
   on a clean machine/VM. For Linux, define the minimum glibc version, base distribution,
   architecture and required graphics libraries; verify Wayland and X11. Proposed baseline
   to confirm: Ubuntu 22.04 x86_64 / glibc 2.35. For Windows, define the supported 10/11
   x86_64 releases, a signed installer, per-user permissions and running without an
   installed toolchain; check Unicode paths and spaces.
6. Artefacts/manifest carry a verifiable publisher signature and a documented procedure
   for storing and rotating the CI key. Windows needs signed installers and binaries and a
   signature check in the smoke test; separate native tests are mandatory. macOS is not
   part of this release; adding it later needs codesign/notarisation and its own test
   matrix. Cross-compilation does not replace start-up tests.
7. The first stable release may be updated manually from a signed artefact. Auto-update
   is not required; if enabled it needs a signed manifest, artefact verification, downgrade
   protection and data preservation on a failed update.
8. The release checklist covers restoring a project from backup, upgrading from a
   supported version, rolling back the binary without corrupting data, known limitations
   and instructions for collecting diagnostics without secrets.

### G6 — usability, accessibility and measurable performance

Until reference hardware is agreed, the numbers below are proposed acceptance targets,
not measurements: Linux and Windows x86_64, 4 cores, 8 GiB RAM, integrated GPU,
1920×1080, 100% and 200% scale. Measure separately on both platforms.

- 200 nodes / 1000 connections: p95 frame time during pan/zoom/drag ≤ 16.7 ms; p99 ≤
  33.3 ms. An extra synthetic profile of 500 nodes / 1000 wires: p95 ≤ 33 ms; this load
  profile does not automatically extend the v1 import limit. Measure with normal labels
  and an open inspector, not empty rectangles.
- Registering Cancel in the UI ≤ 100 ms; stopping a local fixture request and moving to
  cleanup ≤ 1 s. This does not include undoing the operation on the tested server.
- Start of an empty project to an interactive window ≤ 2 s; stabilised memory for the test
  graph ≤ 500 MiB outside an explicitly declared budget of active payloads. A series of
  100 runs shows no monotonic memory growth after reports finish and are removed.
- A report with large JSON renders incrementally/virtually; no expanding the whole tree
  and no serialising the full report every frame.
- All key operations are available from the keyboard: focusing and selecting nodes,
  creating a connection through a form/list, editing an argument, choosing a field,
  Start/Cancel, copy/paste/undo and report resize. Dragging is never the only method.
- Visible focus, readable errors and states without relying on colour alone, font
  scaling, contrast and correct non-ASCII text. Dialogs have predictable focus and Escape;
  graph shortcuts do not capture text editing.
- egui accessibility through the available backend must be checked with a screen reader
  on a supported platform. A custom canvas needs a semantic alternative: a step list and
  connection forms. The mere presence of an accessibility library does not close
  acceptance.

Evidence: a load profile and raw measurements with a build/hardware identifier; a manual
keyboard/screen reader protocol; scaling tests and screenshots. If a target is missed,
profile first, then change the implementation or explicitly correct the supported limit.

## Task order and ownership

P0–P7 below are the audit's local breakdown. The overall backlog, owners and stages
R0–R10 are in [RUST_MIGRATION.md](../../RUST_MIGRATION.md). Atomic file save requirements
apply to the document store, exports and auxiliary files.

| Task                                          | Dependencies                                       | Module owner                     | Closing condition                                                                         |
| --------------------------------------------- | -------------------------------------------------- | -------------------------------- | ----------------------------------------------------------------------------------------- |
| P0: compatibility contract and fixture corpus | Audit of the current model and runner              | Domain / coordinator             | Recorded rules and fixtures covering existing product features; no silent meaning change. |
| P1: import/migrations and storage recovery    | P0, platform decision                              | Persistence                      | G1 and fault injection tests.                                                             |
| P2: secret types, redaction, log policy       | P0                                                 | Security / runner                | G2, before the first runner event reaches the UI or a file.                               |
| P3: HTTP client, budgets, runtime             | P0 + P2, TLS/proxy decisions                       | Runner                           | G3 plus backpressure and the memory limit.                                                |
| P4: scheduler, cancellation, cleanup          | P0 + P3                                            | Runner                           | G4 and the cart fixture, compatibility with the old engine.                               |
| P5: desktop application and interactions      | P0; integration of P1/P4                           | UI                               | The full feature list kept; G6 and automated UI scenarios.                                |
| P6: CI/artefacts                              | Workspace and platform contract; parallel to P1–P5 | Release / coordinator            | A reproducible build procedure, signing, a clean smoke test.                              |
| P7: stable release candidate                  | P1–P6                                              | Coordinator + independent review | All of G1–G6 with evidence; open defects classified and none blocking.                    |

Further features that do not block a compatible first release: team collaboration,
cloud, a public runner, automatic OAuth, keyring, macOS and other releases outside the
declared Linux/Windows matrix, automatic updates, loops and parallel graph execution. This
deferral must not be used to skip data correctness, local execution safety or existing
editor features.
