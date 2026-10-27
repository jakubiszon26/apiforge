# Development guide

## Toolchain

Rust 1.99.0 with rustfmt and clippy, pinned by `rust-toolchain.toml`; dependencies are
locked by `Cargo.lock` and every command uses `--locked`. On Linux the desktop
application needs an X11 or Wayland session, a driver compatible with the WGPU backend
and a file portal for the Open dialog. The Ubuntu package list is in the
[CI workflow](../.github/workflows/rust.yml).

## Running

```bash
cargo run -p apiforge-desktop --locked
cargo run -p apiforge-desktop --locked -- --project fixtures/workspace.json
cargo run -p apiforge-desktop --locked -- --check fixtures/workspace.json
```

`--project` opens a document in a session that does not touch the saved workspace.
`--folder <path>` opens one folder project by path: Code, Diagram
and Split over the project's `.afs` sources, with an explorer of the folder and
external changes followed once a second. A folder without a project is written
only after choosing Initialise, which can register its existing sources. `--screenshot OUT.png --folder <path>` captures it.
Starting with no arguments works from the application data directory: it opens
the project recorded there, migrates a legacy workspace the first time, or
creates one project if the directory holds neither. `--managed` asks for the
same thing explicitly. A migration that is refused activates nothing and stops in
a *Migration stopped* window that names the project and the scenario and offers
*Open the previous workspace* (what `--legacy` opens on request) or *Quit*.
`--check` validates a document without a window: exit code 0 for a valid document, 1
for a document or read error and 2 for invalid arguments. Diagnostics never print
field or secret values. Imports are limited to 16 MiB and perform no HTTP requests.

Two commands exercise the real native window:

```bash
cargo run -p apiforge-desktop --locked -- --smoke-test fixtures/workspace.json
cargo run -p apiforge-desktop --locked -- --screenshot out.png --project fixtures/workspace.json [--select <step>]
cargo run -p apiforge-desktop --locked -- --screenshot out.png --folder <path> [--mode code|diagram|split] [--size 960x540]
```

The smoke test draws three frames and exits; the screenshot command saves the
viewport as PNG. None of them sends requests. The smoke test and the
`--project` capture use a session workspace; the `--folder` capture opens the
project itself, as `--folder` does, so it is added to the recent projects and
the project's view may be written (never with the requested mode).
`--mode` shows a folder capture in Code, Diagram or Split instead of the mode
the folder restores, without recording it as the folder's view; it is rejected
without `--folder`, because the workspace window has no modes. `--size WxH`
sets the window's logical size for either window (default 1600x1000, at least
960x540); the PNG is that size times the display scale.

### Demo API

The demo projects send requests to `http://127.0.0.1:4318/demo`:

```bash
cargo run -p apiforge-demo-api --locked [-- --port 4400]
```

It serves `GET /demo/shop/products/{id}` (products 123 and 124), `POST /demo/shop/carts`,
`POST /demo/shop/carts/{id}/items`, `GET`/`DELETE /demo/shop/carts/{id}` and
`GET /demo/support/tickets/{id}` (ticket 7). State lives in memory: at most 1000 carts,
each expiring after an hour with at most 100 item lines. The server listens on loopback
only and rejects foreign hosts and origins. Tests can run it in-process with
`apiforge_demo_api::spawn(addr)`; dropping the handle stops it.

## Verification

Every change passes these commands before it is merged:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo run -p apiforge-convert-v1 --locked -- --fixtures --check
```

Changes to the layout or the wire router also run the budgets of
[ADR 0006](decisions/0006-layout-and-wire-routing.md), in release mode:

```bash
cargo run -p apiforge-layout --example bench --release --locked [-- --strict]
```

It prints each measurement against its budget. `--strict` makes an exceeded budget
an error, which only makes sense on a machine whose speed is known.

Changes to analysis, the code view or folder sessions also check the source
budgets of the [Stage II plan](stage-ii-code-workspace.md#delivery-and-acceptance)
in release mode: parse, lower and validate a 1000-line scenario in 5 ms, and keep
code and split frames within their budgets at 1000 lines and at the 1 MiB limit.

```bash
cargo run -p apiforge-project --example analysis --release --locked [-- --strict]
cargo test -p apiforge-desktop --release --locked frame_budgets -- --ignored --nocapture
```

The frame test is ignored in the ordinary suite because a debug build says nothing
about time; it fails when a budget is exceeded.

Changes to the scenario language also run both fuzzers for a sustained period. They
are dependency-free mutational smoke fuzzers, not coverage-guided fuzzing:

```bash
cargo run -p apiforge-script --example fuzz_syntax --release --locked -- 60 1592639710
cargo run -p apiforge-script --example fuzz_semantics --release --locked -- 60 2718281828
```

`fuzz_syntax` checks panic freedom, token coverage, UTF-8 ranges, fixes and
formatting idempotence. `fuzz_semantics` mutates generated scenario graphs and checks
that every successful lowering prints, round-trips, prints identically and reconciles.

Changes to the interface follow [GUI testing](gui-testing.md): interaction tests
through named widgets, measurable layout assertions and a design review of the
views the change touches, at the minimum supported window of 960 × 540 and larger.
These run without a display or GPU and do not test the native backend; native
checks remain where a change or its plan requires them.

## Fixtures

See [fixtures/README.md](../fixtures/README.md). Project and run fixtures are generated
by `tools/convert-v1` from frozen version-one sources; never edit them by hand. The
semantic corpus is frozen; a deliberate change of expression semantics is recorded in
an ADR.

## Continuous integration

The workflow runs on Linux and Windows: formatting, clippy, tests, the scenario
examples and workspace check, a release build and `--check` with the release binary.
It then opens the real application window twice per platform (`--smoke-test` and
`--screenshot`) and uploads the capture and the binary as artifacts. The folder
project step captures the migrated demo shop in Code, Diagram and Split at
960 × 540 and 1600 × 1000 and uploads them as `qa-captures-<os>` for QA reports
([testing](process/testing.md#how-to-fetch-windows-captures-for-a-qa-report)).

On Linux each probe gets a fresh 1920 × 1200 Xvfb display and Mesa's Lavapipe Vulkan
driver; a missing driver fails the job instead of selecting another backend. On Windows
the DX12 backend is used. Each probe is limited to 45 seconds and must exit
successfully; the probe step verifies that its capture exists and is not empty. The
probes detect startup, rendering, capture and exit failures; they are not pixel-diff or
end-to-end editing tests.

The Linux probe needs the runtime library `libxkbcommon-x11-0`, which winit loads
under X11 and `libxkbcommon-dev` does not pull in; the root `.gitattributes` keeps
every `.json` and `.afs` file at LF on checkout, so the byte-for-byte fixture check
also passes on Windows.

## Running a scenario from the command line

```bash
cargo run -p apiforge-cli --locked -- run --workspace fixtures/workspace.json \
  --scenario "Promocja i cena w koszyku" [--live] [--secrets secrets.json] [--json]
```

Exit code 0 means the scenario passed, 1 that it did not or could not run, and 2
that the arguments were wrong. `--live` opens the workspace exclusively, because a
live run records what it sends in the run journal; a simulation only reads.

## Folder commands

The CLI and desktop both select a folder project with `--folder`:

```bash
cargo run -p apiforge-cli --locked -- check --folder /path/to/project
cargo run -p apiforge-cli --locked -- print --folder /path/to/project --file scenarios/main.afs
cargo run -p apiforge-cli --locked -- apply --folder /path/to/project --file scenarios/main.afs incoming.afs
cargo run -p apiforge-cli --locked -- run --folder /path/to/project --scenario scenarios/main.afs --json
```

`check` optionally selects one registered source with `--file`. `print` emits its
canonical definition; `apply` validates and saves the supplied source exactly under
the exclusive project lock. Neither changes published component revisions. Folder
calls must name `Handle@revision(...)`. `run` selects a scenario by registered path
or entity ID, and supports `--live`, `--secrets` and `--json` as in JSON mode.
Only the selected scenario and its exact dependencies enter its snapshot; another
invalid scenario does not block it. Live runs require the writer lock and keep
their journal under `.apiforge/local/`. Check, print and simulations use stable
read-only loads without creating local metadata. All folder CLI commands ignore
local recovery and last-valid cache, and fail on invalid saved definitions.

The folder window's Project menu opens folders, recent projects, project settings
and the test environment. Apply keeps public configuration dirty until Save all
(or a source save that includes it). Save all also works with no source tab and
does not save another tab merely because configuration changed. Secret values are
session-only, including in a read-only window; only their names belong to public
configuration. A proposed configuration that breaks immutable published
revisions is rejected before replacing the current settings.

Source/text conflicts open a modal review with exact exports of both candidates.
A newer local or foreign edit requires a fresh preview. An interrupted transaction
opens a separate recovery review before definitions are loaded; Finish first
preserves changed files, then applies the original intent. Read-only windows can
inspect/export, while the writer owns completion. Corrupt intent supports an
opaque transaction-evidence export for manual recovery. Cancel preserves the
unresolved state. Public settings require explicit discard before recovery reload;
the latest unsaved source and text are retained as drafts.

Explicit folder creation/migration appends narrow LF rules to `.gitattributes`
for `.afs`, the root project manifest and the shared diagram JSON. Existing bytes,
sources and index entries are preserved. Opening a project does not repair Git
attributes, and ordinary Save preserves exact LF/CRLF editor text. Review nested
overrides before checkout: a converted immutable revision fails its exact-byte
integrity check. See the [Git portability evidence](qa/2026-10-25/p7-portability/verification.md).

## The legacy JSON code panel

A scenario or an open component can be edited as text beside its board (Ctrl+E).
Typing pauses for 250 ms, then the text becomes one document edit: blocks keep
their identity, and a block the code introduces is placed by the board's layout.
Text that does not parse keeps the document as it was and holds the board until
it is fixed or discarded. Unfinished code is written to `drafts.json` beside the
workspace and comes back at the next start, including unfinished inspector URL,
header, query and body fields. Drafts from inactive projects are retained too.
Conflicting restored text waits for a keep/discard choice. Run settles pending
code first. Ctrl+Z in the panel walks the text;
anywhere else it walks the document. [ADR 0007](decisions/0007-code-editing-state.md)
has the rules.

## The code editor

Scenario text, request bodies and the small template fields share one completion
engine in `apiforge-script`, which also answers what a name means. The desktop
adds the shapes behind a name — the contract a request declares, the sample a
simulation would use, then the output of the last run — so a path offered after
`handle.` says where its shape came from. Environment variables are offered by
name; no value, secret or not, reaches a completion list.

A buffer above 256 KiB is large: it is laid out without colouring, completion
opens only on Ctrl+Space, and a folder code view looks up the statement under the
caret only once the edited source has been analysed. Views share the session's
model rather than copying it, and keep derived names until the model is replaced,
so an idle frame does no whole-source work. The text is unaffected.

Every folder control has an accessible name: fields and choices are named by
`widgets::named`, menus by `widgets::name_menu`, and canvas hit areas by their
widget info. `folder/accessibility_tests.rs` audits each Stage II screen, dialog,
menu and demo block for unnamed focusable nodes and checks keyboard order; extend
it with each new control.

## Storage behaviour

Only one window can save a workspace directory; others open read-only and say why.
`workspace.lock` is released by the operating system when the writer exits and must not
be deleted by hand. External changes stop autosave until the user reloads or continues
without saving. A workspace that cannot be loaded is recovered from its backup only
after confirmation. A version-one workspace is preserved as `workspace.v1.json` before a
fresh workspace opens. Details are in [ADR 0002](decisions/0002-workspace-storage.md)
and [ADR 0008](decisions/0008-storage-sessions.md).

A live run records every request in `journal.json` beside the workspace before it is
sent, and removes the run when it reports its own outcome. A journal that is still
there at the next start belonged to a process that died mid-run: the window lists the
requests that may have changed the tested API and clears them only when the user
marks them as handled. A session without a writable workspace (a read-only second
window, `--project`, a temporary workspace) cannot keep a journal and says so before
a live run. Details are in [ADR 0009](decisions/0009-run-journal.md).

To convert a version-one document during development:

```bash
cargo run -p apiforge-convert-v1 --locked -- workspace.v1.json workspace-v2.json
```

Folder windows perform project, configuration, recovery and journal I/O on storage
workers. UI commands accept a queued request; only its completion acknowledges
success. The serial queue takes session snapshots at dequeue, preserves edits
entered during ordinary saves and binds results to the current session/epoch.
Destructive operations display a working dialog and temporarily disable editing.
Close/project switch waits for storage and Run cleanup before releasing ownership.
Use the controlled slow-I/O tests in `folder/storage_tests.rs` and the
[P7d native audit](qa/2026-10-25/p7-storage-worker/verification.md) when changing
these boundaries. The UI quality probe waits for asynchronous opening while
retaining the 960 × 540 assertions.

## Branches and pull requests

The [development process](process/README.md) governs what is built and in what
order; these are the mechanics.

- Every branch starts at `main`: `feat/<ticket>-…`, `fix/<ticket>-…`,
  `docs/…`. A branch is never based on an unmerged branch, and no integration
  branch collects several tickets before review. Stage II's chain of fifteen
  branches is the counter-example that produced this rule.
- One ticket, one branch, one pull request, within the ticket's
  [size class](process/estimation.md#size). A pull request that outgrows its
  class is split, not explained.
- Small [Conventional Commits](https://www.conventionalcommits.org/).
  Acceptance tests are committed before the implementation
  ([testing](process/testing.md#1-acceptance-tests-first)).
- The pull request follows `.github/PULL_REQUEST_TEMPLATE.md` and links its
  QA report. At most three pull requests await owner acceptance at a time.
- Only the owner merges to `main`. Passing the verification commands above is
  necessary and never sufficient.
- `#[ignore = "<ticket>: <reason>"]` is the only accepted form of an ignored
  test. A change to a default, a format or a migration path cites the ticket in
  which the owner decided it.

## Conventions

- Production code, comments, documentation and the interface are in English.
- Decisions that change the document format, semantics or architecture are recorded as
  ADRs in [docs/decisions](decisions). Deliberate behaviour changes visible in the
  fixtures are explained in the commit that makes them.
- Diagnostics and reports never contain secret values or raw source literals.
