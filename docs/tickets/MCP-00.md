# MCP-00: An MCP server — a model works on a project without the window

- Date: 2026-10-27 · Analysis model: Fable
- Scope: a new `apiforge mcp` command that lets a model (Claude Code or any
  MCP client) read, check, edit and run the scenarios of a folder project
  without the desktop window
- Goal in the owner's words (2026-10-26): analyse an MCP server for the
  project; a task for a whole new sprint.
- State: Approved 2026-10-27 (issue #59)

## Goal

A person opens a folder project in Claude Code, types "add a scenario that
checks the cart total after a promotion", and the model does it: reads the
project and the language reference, writes the scenario, checks it, runs a
simulation, saves — through API Forge's own machinery, so what lands on disk
is exactly what the window would have written, and the window shows it.

## What exists

- **Headless sessions.** `apiforge-project::ProjectSession` already does
  everything the window does without egui: opens a project read-only
  (`open_saved_read_only`, saved files only, writes nothing — not even
  `.apiforge/local`) or as the one writer (`open_saved_exclusive`), edits a
  document's text in memory, settles its analysis, saves exactly, publishes a
  component, registers a new document, and prepares a verified run snapshot.
- **The command line.** `apiforge check|print|apply|run --folder` are thin
  wrappers over that session ([development guide](../development.md#folder-commands)).
  Run against the demo shop today, a model receives: `check` prints
  `Warning 1148..1163: The value's type does not match this input` — byte
  offsets, no line or column, no file name when several documents are
  checked; `print` prints the canonical text; `run` prints one line per step
  and `Passed`/`Failed`, or the full report as JSON with `--json`; `apply`
  refuses a text with errors (`Saved source has no valid definition`, exit 1)
  and otherwise saves the bytes exactly; a second writer gets
  `The folder project is open in another writer session.` A live run needs
  the writer lock and keeps the journal of what it sent.
- **One writer, many readers.** `.apiforge/local/project.lock` is held by
  the process that may write; readers never create it (ADR 0011, KTLO-21).
  The window follows the disk once a second: a clean file is reloaded
  silently, a file with unsaved work gets *Changed outside API Forge while
  unsaved here … Saving them is paused* and the conflict review (KTLO-15).
- **The language has no reference for a model.** ADR 0005 says the reference
  "is generated from this ADR in M7"; M7 has not happened. What exists is the
  ADR's grammar sections (about 290 lines) and two canonical examples in
  `fixtures/scripts/`.
- **The protocol.** MCP's current revision is `2026-07-28` (stateless, every
  request names its version; `2025-11-25` and earlier use an `initialize`
  handshake). Two transports: stdio (the client starts the server as a
  subprocess; stdout carries only protocol messages) and Streamable HTTP. The
  official Rust SDK `rmcp` 3.8.0 (2026-10-26, Rust ≥ 1.88; our toolchain is
  1.99) speaks both revisions and offers `#[tool]` macros, stdio and HTTP.
  Claude Code registers a stdio server with `claude mcp add <name> -- <command>`
  or a `.mcp.json` in the project folder, names its tools
  `mcp__<server>__<tool>`, lets a prompt mention a server resource as
  `@server:uri`, asks the person before each tool call unless they allow it,
  and writes a text result longer than 50 000 characters to a file instead of
  showing it. The repository's own `.mcp.json` registers `egui_mcp` for GUI
  testing; that is tooling for developing API Forge, not part of the product.

## What a model will be able to do

Each tool names a document by its registered path (`scenarios/main.afs`).
Read tools open the saved project read-only and write nothing. Write tools
take the writer lock for the duration of the call and release it.

| Tool | Input | Output | Kind |
| --- | --- | --- | --- |
| `project_overview` | — | name, base URL, environment variable names, secret *names*, contracts, documents (kind, path, handle, revision), published revisions, server and reference version | read |
| `read_source` | path | the saved text, whether it is valid, its diagnostics with line and column | read |
| `check` | path (optional) | diagnostics of one or every document: file, line:column, severity, message, suggested fix | read |
| `check_text` | path, text | the diagnostics a proposed text would have in this project, without writing it | read |
| `print` | path | the canonical text of the document, as the window writes it | read |
| `run` | path | a simulation of the saved scenario: each step with its status and redacted values, the verdict; nothing is sent | read |
| `language_reference` | — | the `.afs` reference written for models, with its version | read |
| `write_source` | path, text | checks, saves the text exactly, returns its diagnostics; refuses text with errors (decision 2) | write |
| `create_document` | kind, path, text | registers a new scenario or component and writes it | write |
| `publish` | path | publishes the working component as its next revision (decision 3) | write |
| `run` with `live: true` | path | real requests, journalled as the window's are; only when the server was started with `--allow-live` (decision 4) | write |

The reference and every source are also MCP resources (`apiforge://reference`,
`apiforge://source/<path>`) so a person can mention them with `@apiforge:…`.

## What it must not do without the owner

- **Write project files** unless the owner allows it (decision 1); then only
  through the session, validated, under the writer lock, never while the
  window holds the project, and never a text with errors (decision 2).
- **Send live requests** unless the person started the server with
  `--allow-live` (decision 4). A simulation sends nothing.
- **Handle secret values.** Values come from the server's environment
  (`APIFORGE_SECRET_<NAME>`) or a `--secrets` file the person names, exactly
  as the CLI; no tool accepts or returns a secret value; reports are redacted
  by the runner as in the window; `project_overview` lists names only.
- **Touch `.apiforge/local`** on a read: read tools use the stable read-only
  load and create no local metadata, so a clean checkout stays clean.

## Where the server lives

A `mcp` subcommand of the existing `apiforge` binary (`apiforge mcp [--folder <dir>]`);
the tools themselves are a new library crate `apiforge-mcp` with no protocol
dependency, so the tests call the tools directly and the protocol layer stays
thin. For the person this means one binary to build: `cargo build -p
apiforge-cli --release`, then `claude mcp add apiforge -- <path>/apiforge mcp`
in any project folder. A separate binary would only add a second thing to
install.

## Transport

stdio first: Claude Code starts the server itself, one per project folder,
nothing listens on a port, and nothing writes to stdout but protocol messages
(diagnostics and logs go to stderr, which the CLI already does). Streamable
HTTP — a server shared by several clients, or hosted by the window — is later
work and changes nothing in the tools.

## Concurrency with an open window

The lock decides, and the server never forces it:

- **The window has the project open** (it holds the lock). Read tools work
  (stable reads). A write tool is refused with "the project is open for
  editing in API Forge; close it there, or edit the file and let the window
  follow it", and nothing changes. A model that edits the `.afs` file with its
  own file tools instead is followed by the window within a second: a clean
  file reloads silently; a file with unsaved work gets the conflict notice and
  review the window already has. A new `.afs` file stays unregistered until
  the person registers it in the window.
- **The server writes first** (a save takes well under a second; a live run
  lasts the run). A window opened at that moment opens read-only and says
  *Another window is editing this project. This window is read-only: changes
  cannot be saved.* — and stays read-only after the server has let go, until
  the project is reopened. MCP-07 names the holder ("the API Forge MCP server
  started 10:42") and lets the window take the lock back when it is free
  (decision 6).

## Versioning of the language reference

The reference is a document in the repository (`docs/language/reference.md`,
decision 7) with a version line, embedded in the binary at build time, so the
text a model receives is the text of the binary it runs. A test parses every
example in it, formats it unchanged and lowers the folder examples against a
fixture project; ADR 0005's rule — a grammar change updates the ADR, the
reference and the round-trip corpus together — gains the reference as a third
member. `project_overview` and `language_reference` return the server's crate
version and the reference's version, so a report of a wrong answer names both.

## Decisions for the owner (taken 2026-10-27: 1 = A, 2 = A, 3 = A, 4 = A, 5 = A, 6 = A, 7 = A, 8 = A, 9 = B)

1. **May the model save project files through the server?** **A (recommended,
   taken):** yes — `write_source` and `create_document`, validated and saved
   under the writer lock exactly as `apply` does, refused while the window
   holds the project. Consequence: a model finishes a scenario on its own and
   Git shows the owner the diff. **B (declined):** read-only server; the model
   edits `.afs` files with its own tools and uses the server to check, print
   and run. Consequence: no registration and no checkpoint from the model —
   new files wait for the owner to register them in the window.
2. **Does the server save a text with errors?** **A (recommended, taken):** no
   — it returns the diagnostics and saves nothing, like `apply`. Consequence:
   the project on disk is never red because of the model; the model fixes and
   retries. **B (declined):** yes, like Ctrl+S in the window. Consequence: an
   unfinished scenario can be committed with errors.
3. **May the model publish a component revision?** **A (recommended, taken):**
   yes, `publish`; a revision is a Git-tracked file the owner reviews like any
   other, and without it a scenario cannot call a component the model wrote
   (folder calls name `Handle@revision`). Consequence: revisions can
   accumulate; none is ever overwritten. **B (declined):** no; the model asks
   the person to publish in the window. Consequence: one round trip per new
   component.
4. **Live requests through the server?** **A (recommended, taken):** only when
   the person starts it with `--allow-live` (a flag in `.mcp.json`),
   journalled, secrets from the server's environment or a `--secrets` file.
   Consequence: off unless the person turns it on per project; a model cannot
   turn it on from inside a session. **B (declined):** never in this sprint.
   Consequence: the model simulates; the person runs live from the window or
   `apiforge run --live`.
5. **The lock while the window is open.** **A (recommended, taken):** the
   server takes the writer lock per write and releases it; a write is refused
   while the window holds the project. Consequence: the owner keeps the window
   open and loses nothing; the model's saves fail with a clear message until
   the window closes the project. **B (declined):** the server holds the lock
   for its lifetime. Consequence: simpler, but every window opened while
   Claude Code runs is read-only.
6. **The window names who holds the project and takes the lock back when it is
   free (MCP-07, mock-up required).** **A (recommended, taken):** yes: the
   notice reads "The API Forge MCP server (started 10:42) is editing this
   project…" and the *Read-only* badge disappears within a second of the
   holder leaving. Consequence: a `default` change (a read-only window becomes
   writable by itself) and a text change; the KTLO-21 probe gets its first
   caller. **B (declined):** leave the text and the behaviour. Consequence:
   "Another window" is wrong whenever the holder is a model or `apply`, and
   the person reopens the project to write again.
7. **A language reference as its own document.** **A (recommended, taken):**
   `docs/language/reference.md`, written for a model, tested, served by the
   tool and the resource. Consequence: one document to review (about 30
   minutes), maintained with the grammar. **B (declined):** the model gets ADR
   0005 as is. Consequence: nothing to write; the model reads reconciliation
   and storage rules it does not need and no complete example of a folder
   call.
8. **`apiforge mcp` without `--folder` uses the current directory** (so
   `claude mcp add apiforge -- apiforge mcp` works in any project folder). **A
   (recommended, taken):** yes (a `default` flag: the other commands require
   `--folder`). Consequence: a server started outside a project answers "the
   folder has no API Forge project". **B (declined):** `--folder` required, as
   for `check`. Consequence: every registration names the folder.
9. **Which sprint follows KTLO.** **A (recommended, declined):** the layout
   sprint (LAY-01..06), as decided on 2026-10-26, then MCP; MCP-01 and MCP-02
   depend on nothing in LAY and can be prepared meanwhile. **B (taken):** MCP
   first. Consequence: the layout decision waits one sprint.

## Sizing

Seven tickets: three M and four S, about 130 minutes of owner review plus
the decisions above (30 minutes). Nothing in MCP-01 to MCP-06 changes the
window; MCP-07 is the only interface ticket and needs a mock-up.

## Tickets, in the order to do them

The MCP sprint follows KTLO, and LAY follows MCP (decision 9 = B, taken
2026-10-27; it revises the order of 2026-10-26 on #32, where LAY followed
KTLO).

| Ticket | Goal | Size |
| --- | --- | --- |
| [MCP-01](MCP-01.md) | `apiforge mcp` answers Claude Code: overview, read, check, print | M · 20 min |
| [MCP-02](MCP-02.md) | A language reference the model reads from the server | M · 30 min |
| [MCP-03](MCP-03.md) | Run a simulation and check a proposed text from the server | S · 15 min · after MCP-01 |
| [MCP-04](MCP-04.md) | Save a scenario or component from the server | M · 30 min · decisions 1, 2, 5 |
| [MCP-05](MCP-05.md) | Publish a component from the server | S · 15 min · decision 3 · after MCP-04 |
| [MCP-06](MCP-06.md) | Live requests from the server behind `--allow-live` | S · 20 min · decision 4 · after MCP-03 |
| [MCP-07](MCP-07.md) | The window names who holds the project and becomes writable when they leave | S · 20 min · `visible UI`, `default` · decision 6 |

MCP-01 and MCP-02 are independent and can run in parallel. MCP-03 to MCP-06
each add tools to the server of MCP-01. MCP-07 touches only the window and
the lock file; it can be scheduled any time after the decision, and matters as
soon as anything but the window writes a project — `apply` does today.

## Technical appendix

- **Session facts** (`crates/apiforge-project/src`): `open_saved_read_only`
  and `open_saved_exclusive` (`session.rs:207–229`) load saved files only, no
  recovery and no last-valid cache, and never fall back from exclusive to
  read-only. `edit_source` (`session.rs:441`) changes text in memory on a
  read-only session too, which is what `check_text` needs; `save`
  (`session.rs:881`) commits the text, the checkpoint and a dirty manifest in
  one transaction. `create_document` (`session.rs:390`) writes the file and
  the manifest. `publishable`/`publish` (`publish.rs:33,73`). `current_snapshot`
  (`session.rs:233`) is a verified closed snapshot of a scenario; a component
  is refused (`Error::ModelLocked`), so running a component waits for
  KTLO-23a's runner target. Diagnostics (`apiforge-script/src/ast.rs:24`) are
  byte ranges with a severity, a static message and an optional `Fix`
  (range + replacement); line and column are computed from the text.
- **CLI facts** (`crates/apiforge-cli/src`): `main.rs` dispatches by the first
  argument with hand-written parsing (no clap); `folder.rs::execute` is the
  pattern for every tool (open, select by path, check, print/apply);
  `run.rs::execute_request` polls a `RunnerService` with a ten-minute
  deadline, reports interrupted journal runs on stderr and prints JSON or
  lines. `secrets()` merges a JSON file and `APIFORGE_SECRET_*`.
- **Lock facts** (`crates/apiforge-storage/src/project/transaction.rs`):
  `open_exclusive` (`:152`) takes `.apiforge/local/project.lock` with a
  confirmed try-lock, creating `.apiforge/local` if needed; `has_live_writer`
  (`:896`) probes without creating the file and waits out the 200 ms window
  of KTLO-21; the lock file is opened read-write and never unlinked, so a
  holder can write its description into it (MCP-07). `Locked` says "The
  folder project is open in another writer session." Measured: a Python
  process holding `flock` on the lock made `apply` and `run --live` fail with
  that message while `check` and a simulation went on working.
- **Window facts** (`crates/apiforge-desktop/src/folder`): `mod.rs:430` sets
  the read-only notice at open and nothing re-checks it; `watcher.rs` runs an
  `ExternalCheck` every second and on focus; `mod.rs:879 follow_external`
  shows the conflict, rename and reload notices quoted above.
  `RegistryState::Changed` (`external.rs`) makes writes wait for a reload
  when the manifest changed under unsaved work.
- **What the CLI gives a model today**, measured on the demo shop written
  with `prepare_project` to a scratch folder: `check` exit 0 with two
  warnings as byte ranges; `run` 18 step lines and `Passed` in 13 ms; after
  `apply` of a scenario expecting `80 * 2` the simulation reports `Failed` on
  *Promotional price* because the sample cart total is 80; `apply` with a
  syntax error reports three errors and saves nothing; `run --live` against
  `apiforge-demo-api` passes and leaves no journal entry behind once the run
  completes.
- **Protocol facts.** MCP revision `2026-07-28` is current
  ([versioning](https://modelcontextprotocol.io/specification/versioning)):
  no `initialize`, `server/discover` mandatory, every request carries
  `io.modelcontextprotocol/protocolVersion` in `_meta`; tools have `name`,
  `description`, `inputSchema` (JSON Schema 2020-12), optional
  `outputSchema` and annotations (`readOnlyHint`, `destructiveHint`), results
  carry `content` and optional `structuredContent`, errors of the tool itself
  go in the result with `isError: true` so the model can correct itself; tool
  names ≤ 128 characters, `[A-Za-z0-9_.-]`; stdio: newline-delimited JSON on
  stdout, logs on stderr. `rmcp` 3.8.0 (crates.io, 2026-10-26, `rust-version`
  1.88) has `ProtocolVersion::LATEST = V_2026_07_28` and
  `LATEST_WITH_INITIALIZE = V_2025_11_25`; features `server`,
  `transport-io` (stdio), `macros` (`#[tool]`, `#[tool_router]`), schemas by
  `schemars`. Pinned as `rmcp = "=3.8.0"` like every other dependency; it
  brings `schemars` (new) and `tokio` (already in the workspace). The
  revision Claude Code speaks was not verified; `rmcp` answers both.
- **Claude Code facts** ([docs](https://code.claude.com/docs/en/mcp)):
  `claude mcp add [--scope local|project|user] <name> -- <command> [args]`;
  project scope writes `.mcp.json` (`{"mcpServers":{"apiforge":{"type":"stdio","command":"…","args":["mcp"]}}}`,
  with `${VAR}` expansion in `command`, `args` and `env`); tools appear as
  `mcp__apiforge__check`; a tool whose full name exceeds 128 characters is
  dropped; text results over 50 000 characters are saved to a file
  (`MAX_MCP_OUTPUT_TOKENS` sets the token limit); resources are mentioned as
  `@apiforge:apiforge://reference`; prompts appear as
  `/apiforge:<prompt> (MCP)`. Project-scoped servers need the person's
  approval once.
- **Size of the reference.** ADR 0005 sections *Lexical structure* to
  *Comments and labels* are lines 58–236; a reference for a model needs the
  same ground plus one complete scenario, one component and one folder call
  with `@revision`, and the list of operators, checks and request fields —
  about 250–350 lines, hence M.
