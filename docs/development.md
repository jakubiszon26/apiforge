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

## MCP server

`apiforge mcp` serves a folder project to a model over the Model Context
Protocol on stdio, protocol revisions `2026-07-28` (`server/discover`) and
`2025-11-25` (`initialize`):

```bash
cargo run -p apiforge-cli --locked -- mcp [--folder /path/to/project] \
  [--allow-live] [--secrets secrets.json]
```

Without `--folder` it serves the directory it was started in. Live requests
and secrets are described under [Live requests](#live-requests). Stdout carries
protocol messages only; the server's own lines go to stderr and name the tool
and the error code, never an argument. EOF on stdin ends the process with exit
code 0.

To use it from Claude Code, register the release binary in the project folder:

```bash
claude mcp add apiforge -- /path/to/apiforge mcp
```

or check in a `.mcp.json` at the project root:

```json
{
  "mcpServers": {
    "apiforge": { "command": "/path/to/apiforge", "args": ["mcp"] }
  }
}
```

Claude Code shows the tools as `mcp__apiforge__<tool>` in `/mcp`.

### Tools

Each tool names a document by its registered path; `check_text` and
`create_document` also take the path a new document would have. A read tool
opens the saved project read-only for each call and writes nothing, not even
`.apiforge/local`; the four write tools work through the agent session below.

| Tool | Input | Result |
| --- | --- | --- |
| `project_overview` | — | name, base URL, environment variable names, secret names, contracts, documents (kind, path, handle, base revision, `source_sha256`, `unavailable`), published revisions with `source_sha256`, `generation`, `holder`, `live_requests` (`allowed` and its `source`), `interrupted_runs`, server version, `reference_version` |
| `language_reference` | — | the `.afs` reference for models (`docs/language/reference.md`, embedded in the binary) and its `version` |
| `read_source` | `path` | the saved text, `source_sha256`, `generation`, `valid`, `unavailable`, diagnostics, `omitted` |
| `check` | `path`? | per document: path, `source_sha256`, `valid`, `unavailable`, diagnostics, `omitted`; a project with errors is reported, not refused |
| `check_text` | `path`, `text` | the diagnostics `text` would have as that document, `valid`, `generation`, the saved `source_sha256`, `used` (the published revisions analysed against); for an unregistered path also `registered: false`, `kind` and `handle` from the header and `problems` (`path_exists`, `handle_exists`, `invalid_text`); a path that is not a working source is the error `path_rejected` |
| `print` | `path` | the canonical text, byte for byte what `apiforge print` prints |
| `run` | `path`, `inputs`?, `full`?, `live`?, `request_id`? | a simulation of the saved document, or with `live: true` a live run: `live`, `status` (`passed`, `failed`, `cancelled`), `ended_by` (`completed`, `cancelled`, `timed_out`), `step_count`, the steps (handle, label, status, phase, message, `expected`/`actual` on a failed comparison, `may_have_reached_api`, a component's steps nested), `generation`, `source_sha256`, `used`, `redacted: true`, `interrupted`, `replayed` for a repeated `request_id`; with `full: true` the runner's JSON report instead of the steps |
| `write_source` | `path`, `text`, `expected_source_sha256`, `expected_generation`? | the text saved exactly, with its checkpoint: `wrote`, `changed` (the paths the commit wrote), the new `source_sha256`, `generation`, `valid`, diagnostics (warnings), `omitted` |
| `create_document` | `kind`, `path`, `text`, `expected_generation`? | a new scenario or component registered: `path`, `kind`, `handle`, `wrote`, `changed`, `source_sha256`, `generation`, diagnostics, `omitted` |
| `publish` | `path`, `expected_source_sha256`, `accept_comment_loss`?, `expected_generation`?, `request_id`? | the saved working component's next revision, as *Publish* in the window: `path`, `handle`, `revision`, `revision_path`, the revision's `source_sha256`, `generation`, `wrote`, `changed`, `replayed` |
| `finish_editing` | — | `finished`: whether an agent session ended and released the project |

`check_text` checks a proposed text without saving it, while `run` executes
the saved source, so a model that wants to run its draft writes it first.

`source_sha256` is the SHA-256 of the saved bytes. `generation` is the
project's head generation, which advances on every commit; it is 0 in a fresh
clone, before the first commit on that machine. A registered source over
1 MiB does not stop the server: that document is `unavailable: "too_large"`
and the others are served.

A simulation sends nothing; `live: true` sends real requests only when the
person allowed them ([Live requests](#live-requests)). A failed verdict is a
result, not an error. A component runs alone with `inputs`, a JSON
object of values by input name read by the rules of the Run inputs dialog
(a value of the input's type, or text the dialog would read as one, such as
`"7"` for a number); the defaults written in code fill the rest. A missing
input without a default, an unknown name or a value of another type is
`invalid_inputs` with `details.missing`, `details.unknown` and
`details.invalid`; a scenario takes no inputs.

One run at a time per server: a second `run` meanwhile is
`run_in_progress`, while the other tools stay answered. The client cancels a
run with `notifications/cancelled`; the main phase stops at the next step,
the cleanup still runs, the run's slot frees, and the request gets no
response, as the protocol says, while stderr says `apiforge mcp: run:
cancelled`. A run still going after ten minutes is cancelled the same way
and answered with `status: cancelled`, `ended_by: timed_out`; a step whose
request was on its way is marked `may_have_reached_api`. At the end of stdin
a simulation in progress has five seconds to finish and be answered (rmcp's
drain of in-flight requests); after that it is cancelled and its cleanup
waited for up to ten seconds. A live run is cancelled at once
([the four ends](#the-four-ends-of-a-live-run)).
Three environment variables are test seams for this and are not set in use:
`APIFORGE_MCP_TEST_DEADLINE_MS` shortens the deadline,
`APIFORGE_MCP_TEST_STEP_DELAY_MS` pauses after every step of a run and
`APIFORGE_MCP_TEST_CLEANUP_DELAY_MS` after every step once a run is
stopping, so that its cleanup outlasts the end of stdin. Each is whole
milliseconds, capped at ten minutes; any other value stops the server with
exit code 2.

### The agent session

The first call of `write_source`, `create_document` or `publish` takes the
project's writer lock for an agent session
([ADR 0014](decisions/0014-agent-sessions.md)): the holder file names role
`agent` and the client's name from `initialize` or the request metadata, and
a window opened meanwhile observes. The server keeps the lock across calls,
however long the pause, until `finish_editing`, the end of stdin or the end of
the process; a later write starts a new session. `project_overview` reports
this server's own session as `holder: "agent_session"`. Each write reads the
saved project afresh under that lock.

- `write_source` saves only a text without errors (`invalid_text` with the
  diagnostics otherwise; warnings are saved) and only over the version the
  model names: a different hash on disk is `stale_source` with the current
  `details.source_sha256`, and a different `expected_generation` is
  `stale_project` with the current `details.generation`; a checkpoint or
  registry another program changed without the lock is `stale_project`
  too. The bytes on disk
  equal `text` and the checkpoint is written: the same files, the same bytes
  as a save in the window. When the disk already holds exactly `text`, the
  call is the same write repeated and changes nothing.
- `create_document` writes nothing unless the text passes `check_text` as a
  new document of `kind`: `path_exists` (also for a folder of the path that
  exists in another letter case; with the existing file's
  `details.source_sha256`, so a retry after a lost answer can tell),
  `handle_exists`, `path_rejected` or `invalid_text` (a header of another
  kind, or errors, with the diagnostics). The source, its registry entry and
  its checkpoint commit in one transaction, one generation.
- `publish` publishes the saved working component, never a text in the
  call: the same transaction as *Publish* in the window writes
  `.apiforge/revisions/<component id>/<n>.afs` with the next unused `n`, its
  registry entry and checkpoint, and advances the working document's base
  revision; nothing existing is overwritten. A different hash on disk is
  `stale_source` with the current `details.source_sha256`; a scenario is
  `not_a_component`; a component with errors `invalid_text` with the
  diagnostics; an unreadable checkpoint `invalid_checkpoint`; a saved
  component holding a known secret value `secret_in_source`. Canonical
  printing drops a comment that belongs to no statement, so such a
  component is `comments_would_be_lost` with `details.comments` (line,
  column, text) until the model calls again with `accept_comment_loss: true`
  and the same hash; the consent holds for that call and that hash only.
- A `publish` with a `request_id` this server has answered returns the
  recorded result with `replayed: true` and publishes nothing. The server
  keeps the last 64 in memory: the same id for another path or hash is
  `invalid_inputs`, an id whose result was dropped is `unknown_outcome`.
  Unlike a live run's, a publication's id is not written to
  `.apiforge/local/agent-requests.json`, so a restarted server knows none of
  them — `project_overview` lists every revision with its `source_sha256`,
  which tells whether a lost publication happened. As for a live run, an id
  is at most 256 bytes (`too_large`) and never a known secret value
  (`secret_in_text`).
- A text containing a known secret value (`APIFORGE_SECRET_<NAME>`, read at
  each call) is `secret_in_text`, the value never repeated.
- While a window holds the project a write is `held_by_window`; another
  process, an agent or `apiforge apply`, is `held_by_other`. The message and
  `details.holder` name the holder and since when; the read tools still
  answer.
- A commit that fails after its activation is `recovery_required` with
  `wrote: "unknown"`; every later write of the session is refused with the
  same code (`wrote: false`). The server does not repair: after
  `finish_editing`, the window's recovery view finishes the operation when
  it opens the project.

### Live requests

**The flag.** `run(path, live: true)` sends the requests of the saved
scenario or component to the project's base URL only while live requests are
allowed, and the model cannot allow them: the person starts the server with
`--allow-live`, in `.mcp.json` (`"args": ["mcp", "--allow-live"]`) or on
`claude mcp add apiforge -- /path/to/apiforge mcp --allow-live`. Without it a
live run is `live_disabled` (*live requests are not enabled on this server*)
and sends nothing. The server keeps the current permission in one slot:
the flag while no window is connected; MCP-09 lets a connected window's
per-project setting decide instead (*live requests are off in this project's
settings*), and the flag applies again when the window goes.
`project_overview.live_requests` says `allowed` and its `source`
(`--allow-live`, `no --allow-live`, `window setting`, `off in the project's
settings`). The permission is checked immediately before the run starts,
under the writer lock; a change after that does not stop the run, whose
cleanup must finish.

**The environment.** Secrets come from `APIFORGE_SECRET_<NAME>` in the
server's environment (the `env` of the entry in `.mcp.json`) and from the
`--secrets` file, a JSON object of names and text values, as for `apiforge
run`; a name in both takes the environment's value. The file is read at start
— one that cannot be used stops the server with exit code 2 and a message that
never quotes it — and again at each call. No known value leaves the server: a
run's report is redacted by the runner as the window's is (`[REDACTED]`); a
`text` or `request_id` argument that carries one is `secret_in_text`; a saved
source that contains one is withheld by `read_source`, `print` and
`resources/read` as `secret_in_source` with `details.line`, never as a modified
text; errors and stderr never quote one.

**The journal.** A live run is a write: it takes the project for the agent
session, or uses the session's lock, exactly as `write_source` does, and is
refused with `held_by_window` or `held_by_other` and the holder while another
process holds the project; a simulation still runs then. Each request is
recorded in the project's run journal, `.apiforge/local/journal.json`, before
it is sent, as the window records its own (ADR 0009). The entry exists while
the run is in progress and is removed when it reaches its end, so an entry
still on disk once its process has stopped describes an interruption; this
server's own run in progress is never reported as one.
The next `run`, simulated or live, lists what an earlier process left there
under `interrupted`: per run the scenario, its start and whether its cleanup
had begun, and the requests that may have changed the API with method,
redacted URL and state (`Sending`, `Performed`, `Possibly performed`), as
`apiforge run` prints them; `project_overview` counts them in
`interrupted_runs`. They stay until the window marks them handled. While
another process holds the project its journal is its own and is not read. A
live run repeated with a `request_id` this server has answered returns the
recorded result with `replayed: true` and sends nothing; the same id for
another document or other inputs is `invalid_inputs`. The server keeps the
last 64 results; an older id it answered is `unknown_outcome`, never run
again. Exactly-once is not promised.

**The record of request ids.** Before a live run with a `request_id` sends
its first request, the id, the path and the time are written atomically to
`.apiforge/local/agent-requests.json`, under the writer lock, as the journal
is: machine-local, untracked, never part of the project's format. A new
server that finds an id there answers `unknown_outcome` with the record's
`path` and `started_at` and the journal's `interrupted` runs, and sends
nothing; the model uses a new id to run again. The record keeps the 1 000
most recent ids, the oldest dropped first, and an id is at most 256 bytes
(`too_large`). A record that cannot be read (damaged, or written by a newer
version) is never overwritten: every live run with a `request_id` is refused
with `project_unreadable` naming the file until it is moved aside, because
the server could not tell whether the id already ran; a live run without a
`request_id` still works.

#### The four ends of a live run

- **Completed or failed**: the main phase and the cleanup ran; the journal's
  entry is removed; the result is `passed` or `failed`.
- **Cancelled or timed out**: a `notifications/cancelled` for the call, or the
  ten-minute deadline, stops the main phase at the next step; the cleanup runs
  and its steps are in the report; the entry is removed at the end; the result
  is `status: cancelled` with `ended_by: cancelled` or `timed_out`, and the
  request that was on its way is `may_have_reached_api`. A cancelled call gets
  no response, as the protocol says; a timeout is not evidence that nothing
  was sent.
- **Client lost**: at the end of stdin a live run is cancelled at once, its
  cleanup gets up to ten seconds from the end of stdin, then the lock is
  released and the process exits. When the cleanup finished, the journal is
  empty; when it did not, its entry stays for the next `run`.
- **Process killed**: nothing runs; the operating system releases the lock and
  the entry stays. The next `run` of a new server, or the window when it opens
  the project, reports it as interrupted; the same `request_id` on a new
  server is `unknown_outcome`.

### Resources

`resources/list` returns `apiforge://reference`, the language reference, and
`apiforge://source/<path>` for every registered document; `resources/read`
returns the reference or the saved text as `text/plain`. In Claude Code,
`@apiforge:apiforge://reference` in a prompt attaches the reference. A source
URI's path is percent-decoded and must then pass the path rule below, so
`apiforge://source/%2e%2e/x.afs` or `apiforge://source/.apiforge/…` is
refused with a JSON-RPC error whose `data.code` is `path_rejected`; an
unregistered path is `resource_not_found` with `data.code`
`unknown_document`.

### The contract

- Every tool has an input and an output schema; `tools/list` returns them in
  the order above. The read tools carry `readOnlyHint: true`; `run` does
  not, since a live run sends requests and writes the journal: it is
  `destructiveHint: true`, `openWorldHint: true` and not idempotent;
  `write_source` is `destructiveHint: true` and `idempotentHint: true`,
  `create_document` and `publish` neither, `finish_editing` only
  idempotent. A result carries
  `structuredContent` and the same JSON as readable text in `content`.
- A call that could not do what was asked is `isError: true` with
  `structuredContent` `{ code, message, wrote, details }`. The codes form a
  fixed list: `not_a_project`, `project_unreadable`, `unknown_document`,
  `invalid_text`, `invalid_inputs`, `not_a_component`,
  `invalid_checkpoint`, `stale_source`, `stale_project`, `held_by_window`,
  `handover_pending`, `handover_refused`, `held_by_other`, `live_disabled`,
  `comments_would_be_lost`, `secret_in_text`, `secret_in_source`,
  `path_rejected`, `path_exists`, `handle_exists`, `recovery_required`,
  `too_large`, `result_too_large`, `run_in_progress`, `cancelled`,
  `timed_out`, `unknown_outcome`. The read tools use `not_a_project`,
  `project_unreadable` (any other storage failure, with its message),
  `recovery_required` (an interrupted transaction), `unknown_document`
  (`details.registered_paths`), `invalid_text` (`details.diagnostics`),
  `invalid_inputs`, `live_disabled`, `run_in_progress`, `too_large`,
  `path_rejected`, `result_too_large`, `secret_in_text` (`check_text`, a
  `request_id`) and `secret_in_source`; `wrote` is always `false` for them.
  A live run adds `held_by_window`, `held_by_other` and `unknown_outcome`.
  The write tools add `stale_source`, `stale_project`, `held_by_window`,
  `held_by_other`, `secret_in_text`, `path_exists`, `handle_exists` and
  `invalid_checkpoint`; `publish` also `not_a_component`,
  `comments_would_be_lost`, `secret_in_source` and, for a `request_id`,
  `invalid_inputs`, `too_large` and `unknown_outcome`; `wrote` is `false` for a refusal
  and `"unknown"` only for a commit that failed after its activation.
- A path argument must pass `ProjectPath::working_source`: relative, portable,
  outside `.apiforge`, ending in `.afs`. `../x.afs`, an absolute path,
  `.apiforge/diagram.json` or a Windows reserved name is `path_rejected`.
- Positions: `line` and `column` are 1-based and counted in Unicode scalar
  values; `end_line` and `end_column` are exclusive; a line break is `\n`, and
  a `\r` before it belongs to the break. A fix carries its replacement and its
  own range.
- Limits: the analysis keeps 1 000 diagnostics per document and `omitted`
  counts the rest, an error taking a warning's place at the limit so a
  project-level error is never the one dropped; a result or an error over
  2 MiB is `result_too_large` with how to narrow the request (`full: false`,
  one path), never cut; a `text` argument over 1 MiB is `too_large` before
  anything else.

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
- One ticket, one branch, one pull request, from the analysis to the QA
  report ([one result, one pull request](process/planning.md#one-acceptable-result-one-pull-request)).
  The pull request states its changed lines; a ticket whose
  [size](process/estimation.md#size) proves wrong by a class is re-approved
  or split at that point, not explained at the end.
- Small [Conventional Commits](https://www.conventionalcommits.org/).
  Acceptance tests are committed before the implementation
  ([testing](process/testing.md#1-acceptance-tests-first)).
- The pull request follows `.github/PULL_REQUEST_TEMPLATE.md` and links its
  QA report. At most four pull requests await owner acceptance at a time, and
  at most 60 estimated review minutes among them
  ([limits](process/README.md#limits-that-keep-review-possible)).
- Only the owner merges to `main`. Passing the verification commands above is
  necessary and never sufficient.
- `#[ignore = "<ticket>: <reason>"]` is the accepted form of an ignored test;
  the two exceptions are a design-review view and a spawned helper, named in
  [testing](process/testing.md#1-acceptance-tests-first). A change to a
  default, a format or a migration path cites the ticket in which the owner
  decided it.

## Conventions

- Production code, comments, documentation and the interface are in English.
- Decisions that change the document format, semantics or architecture are recorded as
  ADRs in [docs/decisions](decisions). Deliberate behaviour changes visible in the
  fixtures are explained in the commit that makes them.
- Diagnostics and reports never contain secret values or raw source literals.
