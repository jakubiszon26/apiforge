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
`--check` validates a document without a window: exit code 0 for a valid document, 1
for a document or read error and 2 for invalid arguments. Diagnostics never print
field or secret values. Imports are limited to 16 MiB and perform no HTTP requests.

Two commands exercise the real native window:

```bash
cargo run -p apiforge-desktop --locked -- --smoke-test fixtures/workspace.json
cargo run -p apiforge-desktop --locked -- --screenshot out.png --project fixtures/workspace.json [--select <step>]
```

The smoke test draws three frames and exits; the screenshot command saves the
viewport as PNG. Both use a session workspace and send no requests.

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

Changes to the scenario language also run both fuzzers for a sustained period. They
are dependency-free mutational smoke fuzzers, not coverage-guided fuzzing:

```bash
cargo run -p apiforge-script --example fuzz_syntax --release --locked -- 60 1592639710
cargo run -p apiforge-script --example fuzz_semantics --release --locked -- 60 2718281828
```

`fuzz_syntax` checks panic freedom, token coverage, UTF-8 ranges, fixes and
formatting idempotence. `fuzz_semantics` mutates generated scenario graphs and checks
that every successful lowering prints, round-trips, prints identically and reconciles.

The egui tests lay out the interface without a GPU; they do not test the native
backend. Changes to the interface are also reviewed in the running application,
including the minimum supported window of 960 × 540.

## Fixtures

See [fixtures/README.md](../fixtures/README.md). Project and run fixtures are generated
by `tools/convert-v1` from frozen version-one sources; never edit them by hand. The
semantic corpus is frozen; a deliberate change of expression semantics is recorded in
an ADR.

## Continuous integration

The workflow runs on Linux and Windows: formatting, clippy, tests, the scenario
examples and workspace check, a release build and `--check` with the release binary.
It then opens the real application window twice per platform (`--smoke-test` and
`--screenshot`) and uploads the capture and the binary as artifacts.

On Linux each probe gets a fresh 1920 × 1200 Xvfb display and Mesa's Lavapipe Vulkan
driver; a missing driver fails the job instead of selecting another backend. On Windows
the DX12 backend is used. Each probe is limited to 45 seconds and must exit
successfully; the probe step verifies that its capture exists and is not empty. The
probes detect startup, rendering, capture and exit failures; they are not pixel-diff or
end-to-end editing tests.

## Running a scenario from the command line

```bash
cargo run -p apiforge-cli --locked -- run --workspace fixtures/workspace.json \
  --scenario "Promocja i cena w koszyku" [--live] [--secrets secrets.json] [--json]
```

Exit code 0 means the scenario passed, 1 that it did not or could not run, and 2
that the arguments were wrong. `--live` opens the workspace exclusively, because a
live run records what it sends in the run journal; a simulation only reads.

## The code panel

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

A buffer above 256 KiB is laid out without colouring, which keeps a very large
body responsive; the text is unaffected.

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

## Conventions

- Stage II's [plan](stage-ii-code-workspace.md) and ADRs are published on `main`
  before implementation. Develop that stage on `feat/stage-ii-code-workspace`
  starting at the planning commit; a planning merge does not enable folder mode
  or automatic migration. Follow P1/P2 and its restart round-trip gate before P3.
- Production code, comments, documentation and the interface are in English.
- Work happens on short-lived `feat/…`, `fix/…` or `docs/…` branches with small
  [Conventional Commits](https://www.conventionalcommits.org/), merged into `main`
  after verification.
- Decisions that change the document format, semantics or architecture are recorded as
  ADRs in [docs/decisions](decisions). Deliberate behaviour changes visible in the
  fixtures are explained in the commit that makes them.
- Diagnostics and reports never contain secret values or raw source literals.
