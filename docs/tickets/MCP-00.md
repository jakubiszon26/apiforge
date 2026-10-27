# MCP-00: An MCP server — a model works on a project while the owner watches

- Date: 2026-10-27 · Revised 2026-10-27 after the owner's audit (revision 2)
  · Analysis model: Fable
- Scope: a `mcp` subcommand of the `apiforge` binary over stdio, so that
  Claude Code or another MCP client reads, checks, edits, publishes and runs
  the scenarios of a folder project through API Forge's own session — and
  the owner sees every committed change in the window that is already open.
- Goal in the owner's words (2026-10-26, amended 2026-10-27): analyse an MCP
  server for the project, a sprint of its own; **the agent works through MCP
  and the owner watches its committed changes live in the already open
  application.**
- State: revision 1 approved 2026-10-27 (issue #59, decisions 1–9);
  revision 2 decisions 10–18 taken 2026-10-27 (all A); mock-ups M2 and M3
  approved 2026-10-27, M1 and M4 approved the same day in round 4 (the
  bottom status bar)

## What changed in revision 2

The owner's audit of 2026-10-27 kept the foundation — `ProjectSession`, the
storage transactions, the runner, stdio, a tested `.afs` reference, live
requests only when the person allows them — and corrected eleven points,
then added a twelfth: a per-project toggle for live requests. In short:

| # | Revision 1 said | Revision 2 says |
| --- | --- | --- |
| 1 | The window had to close the project before the agent could write; the window followed `apply`-style writes by its own file watcher | The window stays open, **hands the project to the agent and observes**: every committed change is on screen within two seconds, editing is paused there, navigation is not ([live preview](#live-preview-in-the-open-window)) |
| 2 | The server took the lock per write (decision 5); MCP-07 let the window take it back whenever it was free | One writer for one **agent session**: the server holds the lock from the hand-over until the agent finishes; the window never takes it from a living process ([lock ownership](#one-writer-the-agent-session)); decision 5 is superseded by decision 10 |
| 3 | A write while the window held the project: "close it there, or edit the file directly" | A hand-over request to the window; "edit the file directly" is no longer advice ([hand-over](#hand-over-between-the-window-and-the-agent)) |
| 4 | `create_document` = the session's `create_document` | Validate the new document in the project's context first; source, registry entry and checkpoint land in **one** transaction or not at all ([validation](#validation-before-the-first-write)) |
| 5 | A write replaced the text | Every read returns the source hash and the project generation; a write names the version it was based on and is refused when stale ([versions](#versions-and-stale-writes)) |
| 6 | A cancelled run "left its entries in the journal" | Cancellation, timeout, client disconnect and process death are four cases with four outcomes; the runner's record is the report, the journal describes only an interruption ([runs](#runs-cancellation-and-the-journal)) |
| 7 | `publish` = the session's `publish` | Comment loss asks first; lost responses have a way to learn the outcome; nothing is retried on its own ([publishing](#publishing-and-lost-responses)) |
| 8 | "No tool returns a secret" | One rule for every tool, resource, report, diagnostic, error and log; a source that contains a secret is withheld, never rewritten ([secrets](#secrets-and-the-project-boundary)) |
| 9 | Tool shapes in prose | Output schemas, stable error codes, `wrote` on every write result, columns in Unicode scalar values, explicit limits, a real stdio test ([contract](#the-contract)) |
| 10 | Two demo warnings; components cannot run | KTLO-25 removed the warnings and KTLO-23 made components run; the plan follows ([refresh](#what-exists)) |
| 11 | Seven tickets, 130 minutes | Thirteen tickets, dependencies corrected, 150 → 305 minutes, the window scope split into acceptable results ([tickets](#tickets)) |
| 12 | Live consent = `--allow-live` at start | A toggle in the project settings the person can flip while the agent is connected; the flag serves the headless case ([live consent](#live-consent-the-toggle)) |

## Goal

A person has the demo shop open in API Forge and Claude Code beside it. They
type "add a component that fetches a product and a scenario that checks the
cart total after a promotion". The model reads the project and the language
reference, writes the component, publishes it, writes the scenario that calls
it, checks and simulates, fixes what the diagnostics say — and the person
watches each saved step appear in the window they already had open: the file
in the explorer, the code, the diagram, the problems list, the new revision.
When the model is done the window is theirs again, with nothing lost.

## What exists

- **Headless sessions.** `apiforge-project::ProjectSession` does everything
  the window does without egui: opens a project read-only
  (`open_saved_read_only`: saved files only, nothing written, not even
  `.apiforge/local`) or as the one writer (`open_saved_exclusive`), edits a
  document's text in memory, settles its analysis, saves exactly, publishes,
  registers a new document, prepares a verified run snapshot — for a scenario
  **or a component** since KTLO-23 (`current_snapshot`, `prepare_run`;
  `RunTarget::Component { revision, inputs }`).
- **The command line.** `check|print|apply|run --folder` wrap that session.
  `run` has a ten-minute deadline and prints lines or JSON; `apply` refuses
  text with errors and saves bytes exactly; a second writer gets `The folder
  project is open in another writer session.` The demo shop analyses
  **without a warning** since KTLO-25 (#76); tests that need a diagnostic
  inject one.
- **One writer, many readers** (ADR 0011). `.apiforge/local/project.lock` is
  an OS lock on an open file, never unlinked, never taken over; a reader
  probes it without creating it (`has_live_writer`, KTLO-21). A read-only
  store writes nothing under `.apiforge/local`. The holder has no
  description yet; MCP-07 gives it one.
- **The window follows the disk** once a second and on focus (`watcher.rs`):
  a clean document follows (`follow_disk`: text and saved hash, new source
  version, analysis pending — **not** the checkpoint `diagram.json`, which the
  session reads only at load and reload); a document with unsaved work gets a
  conflict and is never overwritten (KTLO-15); a changed registry reloads the
  whole project when nothing is unsaved and otherwise waits for an explicit
  *Reload project* (`RegistryState::Changed`). A reload starts a new epoch, so
  a late analysis result is rejected (ADR 0012), and today drops the active
  document (`follow_external` sets `shown = None`).
- **Read-only today** means: *Save*, *Save all*, *Publish* and the settings
  dialog are disabled, but typing in the code view and editing the board
  still change memory; nothing can save them. The notice says *Another
  window is editing this project*, whoever the holder is.
- **Runs.** `RunnerService::start` returns a handle; `cancel` stops the main
  phase and cleanup still runs; a run that reaches its end — passed, failed
  or cancelled — removes its journal entry, so a journal on disk always
  describes an interruption (ADR 0009). `shutdown(timeout)` cancels every
  run and waits for cleanup. Known secret values set on the store refuse any
  staged text that contains one (`set_known_secrets`).
- **Settings.** Project name and contracts are edited in *Project settings*
  (disabled when read-only); the environment and the session's secret
  values in *Environment*. Person-level preferences live outside the project
  in the user's configuration directory (`preferences.rs`), by design away
  from the writer lock.
- **The language has no reference for a model** (decision 7 writes one).
- **The protocol.** MCP `2026-07-28` (stateless, `server/discover`) and
  `2025-11-25` (`initialize`); `rmcp` 3.8.0 speaks both; Claude Code starts a
  stdio server per project, asks before each tool call unless allowed, and
  writes a text result over 50 000 characters to a file.

## The sprint's key acceptance criterion

**The agent works through MCP and the owner watches its committed changes
live in the application that was already open.** MCP-11 is that test: the
window is open first; the agent obtains the right to edit, creates a
component, publishes it, creates a scenario that calls it, fixes it twice and
simulates; the person sees each committed change in the same window within
two seconds; the pauses between the agent's saves are longer than any
interval the window polls at; nothing is closed, reloaded by hand or edited
by hand. Separate cases: unsaved work in the window, an invalid proposal, a
stale write, a lost client, a dead server, and the window getting its editing
back.

## What a model will be able to do

Each tool names a document by its registered path. Read tools open the
saved project read-only and write nothing. Write tools need the **agent
session** (below) and say in their result whether they wrote.

| Tool | Input | Output | Kind |
| --- | --- | --- | --- |
| `project_overview` | — | name, base URL, environment variable names, secret *names*, contracts, documents (kind, path, handle, base revision, `source_sha256`), published revisions (handle, number, `source_sha256`), `generation`, who holds the project, `live_requests` (allowed and why), server and reference version | read |
| `read_source` | path | the saved text, `source_sha256`, `generation`, `valid`, diagnostics with line and column | read |
| `check` | path? | diagnostics of one or every document, each with file, line, column, severity, message, fix | read |
| `check_text` | path, text | the diagnostics the text would have as that document — or, for a path that is not registered yet, as a new document: kind from the header, handle, collisions | read |
| `print` | path | the canonical text, as the window writes it | read |
| `run` | path, `inputs`?, `full`?, `request_id`? | a simulation of the **saved** scenario or component: status, steps, verdict, `generation`, `source_sha256`; nothing is sent | read |
| `language_reference` | — | the `.afs` reference for models, with its version | read |
| `write_source` | path, text, `expected_source_sha256`, `expected_generation`? | checks and saves the text exactly when it has no error; refused when stale; returns diagnostics, `wrote`, new `source_sha256`, `generation` | write |
| `create_document` | kind, path, text, `expected_generation`?, `request_id`? | validates in the project's context, then registers and writes source, registry entry and checkpoint in one transaction | write |
| `publish` | path, `expected_source_sha256`, `accept_comment_loss`?, `request_id`? | the working component's next revision; asks before dropping comments | write |
| `run` with `live: true` | as `run`, `request_id` | real requests, journalled, only while live is allowed ([live consent](#live-consent-the-toggle)) | write |
| `finish_editing` | — | ends the agent session: releases the project to the window | write |

Resources: `apiforge://reference` and `apiforge://source/<path>`.
`check_text` checks a **proposal**; `run` executes the **saved** source — a
model that wants to run its draft writes it first. This is said in both
tools' descriptions.

## One writer: the agent session

Revision 1 took the lock per write (decision 5) and let the window take it
back whenever it was free (MCP-07). Together they let the window take the
project away from the agent after its first save. Revision 2 keeps one
owner of writes at a time and defines who it is and for how long:

- The window holds the project while the person edits.
- An **agent session** begins when the server first needs to write (or when
  the model calls a write tool while the project is free) and lasts until
  the model calls `finish_editing`, the client disconnects (stdin EOF), or
  the server process ends. For its whole duration the server holds the
  writer lock; the pauses between tool calls do not matter, however long.
- **The window never takes the project from a living process.** It becomes
  the writer again only when the lock is free — released, or the holder is
  dead — which the existing probe tells it within a second (MCP-07). No
  timeout ever decides.
- A second agent (two Claude Code sessions, or `apiforge apply`) is refused
  with the holder's description; there is no hand-over between agents.

Two designs were compared for the window's side (details in the
[appendix](#the-two-designs-for-live-preview)). **Recommended: the window
is an observer for the agent session** — it hands the lock over, follows
the committed state from disk, and gets the lock back when the agent is
done. The alternative, routing every tool call to the window's own session
over a local channel, would give an instant preview but needs a second
execution path for the headless case, a far larger protocol, and the same
pause on editing; it does not fit the architecture better. Neither needs
Streamable HTTP.

## Hand-over between the window and the agent

A small **control channel** between the two processes replaces "close the
project in the window": the lock holder listens on a local endpoint (a Unix
socket under the user's runtime directory; a named pipe on Windows) and
writes the endpoint's name into the holder file beside its description
(MCP-07); the other side connects. The messages are few:

| From | To | Message | Effect |
| --- | --- | --- | --- |
| server | window | `request` (who the agent is) | the window hands over or answers why not |
| window | server | `granted` / `refused { reason }` | the server takes the lock, or reports the reason, nothing written |
| server | window | `saved { generation, paths }`, `run { path, mode, state }`, `released` | the window rescans at once; the status line; take-back |
| window | server | `permission { live }` | the live consent, at hand-over and on every change |
| window | server | `take_back` | the server finishes the operation in flight, releases, answers `released` |

Every situation the owner asked for:

| Situation | What happens |
| --- | --- |
| Window first, then the agent writes | The server finds the lock held by a window, asks over the channel. **Clean window:** it saves nothing, releases the lock, becomes an observer, says who has the project; the server takes the lock, the write proceeds. **Unsaved work:** MCP-08b refuses with *The window has unsaved work; save it there first* on both sides; MCP-08c asks the person instead: *Save all and hand over* or *Not now* (decision 13). The answer reaches the server within the request; a prompt the person has not answered in 30 s returns `handover_pending`, nothing written, and the prompt stays until answered; the next request is answered at once |
| Server first, then the window opens | The window opens read-only, reads the holder's description from the holder file (*Claude Code via apiforge mcp, since 10:42*), connects to the server's endpoint, and is an observer from the start: editing paused, live consent pushed, following every commit. When the agent finishes, the window takes the project back within two seconds |
| Two clients on one project | The second server's write is refused with the first's description; reads and simulations work for both; no hand-over between servers |
| The client disconnects (EOF) | The server cancels a run in progress, waits up to ten seconds for its cleanup, releases the lock, exits; the window takes the project back within two seconds |
| The server process dies | The OS releases the lock; the window takes the project back; an activated transaction is finished by the window's existing recovery view, an interrupted live run is reported from the journal at that moment, as at start |
| A run is active and the person presses *Take back* | The server finishes the run (cleanup included), then releases; the window says *waiting for the agent's run to finish*. Cancelling the run from the window is not in this sprint |
| A write is in flight when `take_back` arrives | The transaction (milliseconds) commits or fails as a whole; the release follows; `wrote` in the tool result is true or false, never half |
| The window is closed while observing | The channel drops; the server keeps the lock and continues; live consent falls back to the start flag (below) |

"Edit the `.afs` file directly" is no longer offered as a way around a
refusal: a refusal names the holder and, when it is a window, what the
person can do there.

## Live preview in the open window

**Definition.** The window shows the project's **committed** state: what
the agent has saved, registered or published through a finished
transaction. An unsaved proposal, a text being generated, a failed write
are not shown and are not saved for the preview's sake.

**Target.** Within **2 seconds** of the end of a commit on the demo shop,
the window shows the new state. Mechanism: the server's `saved` message
makes the window rescan at once (`watcher.rescan()`), and the one-second
poll remains the fallback when the channel is absent.

**What the preview covers:** the source in the code view, the diagram with
the identities and positions the agent's checkpoint sealed, the Problems
panel, a new document in the explorer, a new published revision in the
component's tab and in the calls that may use it.

**Consistency.** In observer mode the window follows a commit by **reloading
the project from one stable read** (ADR 0011: one generation, no active
transaction), not by following files one by one: a commit writes source,
checkpoint and registry together, and the stable read sees all of them or
none. The reload starts a new session epoch, so an analysis result that was
in flight for the previous text is rejected and can never put an older
state back. The window keeps what is still valid across the reload: the open
tabs whose documents still exist, the active document, its camera, the
selected block when its identity survived, the panel layout; a tab whose
document is gone closes with a notice. This also closes a gap that exists
today beside `apply`: the per-file follow leaves the window's checkpoint
baseline stale, and its next save would meet a conflict on `diagram.json`.

**Agent runs in the window.** The owner asked whether the agent's runs are
visible. Recommended for this sprint: a status line in the bottom bar — *Agent:
running main.afs (simulation)…*, then *main.afs passed, 18 steps, 12:58* —
fed by the server's `run` messages (MCP-10). The full report stays in the
agent's result, which the model shows the person; showing it in *Run results*
means transferring the report over the channel and is later work.

## Unsaved work and the observer

The window hands the project over only from a **consistent** state: nothing
unsaved in code, inspector fields, project configuration (name, contracts,
environment) or diagram layout, and no held recovery draft. `has_unsaved_work`
already answers exactly that question. With unsaved work the window refuses
(MCP-08b) or asks (MCP-08c): *Save all and hand over* saves everything,
invalid text included, as Ctrl+S does, then hands over; *Not now* keeps the
person editing and the agent gets the refusal. Nothing is overwritten,
discarded or declared saved on the person's behalf.

While observing, editing is **paused** (the owner allowed this): the code
view and the board do not accept changes, the explorer's create, rename and
delete are disabled, *Environment* opens for viewing, *Project settings*
opens with everything disabled but the live toggle. Navigation stays:
tabs, modes, the explorer, scrolling, selection, zoom, *Run* for a
simulation, the panels. Because the observer holds no unsaved work, the
registry-changed case that blocks a reload today cannot arise, and the
reload above is always possible.

Decision 12 asks whether the same pause applies when the holder is another
window or `apply`; recommended yes, one state for every holder, because an
edit that cannot be saved is a trap and the take-back of MCP-07 would turn
it into a conflict.

## Validation before the first write

The session's `create_document` writes the source and the registry entry
first and analyses afterwards; the checkpoint follows in a later
transaction. It is not a "validate then write" operation and MCP will not
use it as one. Instead:

1. `check_new_document(kind, path, text)` on the session validates without
   touching anything: the path (`ProjectPath::working_source`, not existing
   on disk, not registered, no case-insensitive collision), the header's kind
   against `kind`, the handle against the registered handles of that kind,
   and the text analysed in the project's context as if it were registered —
   missing dependencies, unknown revisions, type errors, everything
   `check_text` reports. `check_text` on an unregistered path is this call.
2. `create_document` runs the check and refuses on any error: no source, no
   registry entry, nothing. On success it commits **source, registry entry
   and checkpoint in one transaction**, the checkpoint computed from the
   validated model before the commit, so a new document never exists without
   its sealed identities.
3. Transaction boundaries follow ADR 0011: a failure before activation leaves
   nothing; a failure after activation (a sync error while installing the
   checkpoint, for example) leaves recoverable intent, the tool answers
   `recovery_required` with `wrote: "unknown"`, every later write of the
   agent session is refused with the same code, and the window's existing
   recovery view finishes or reports the operation once it is the writer
   again. The server does not repair on its own.

`write_source` already validates first (the text is edited in memory,
settled, and saved only without errors); it gains the version check below.

## Versions and stale writes

Every result that describes a document carries `source_sha256` (the saved
bytes' hash the storage already keeps) and `generation` (the project's head
generation, which advances on every commit: source, registry, publication,
configuration). A write names what it was based on:

- `write_source(expected_source_sha256)` is required; a different hash on
  disk is `stale_source`, nothing written, the current hash returned.
- `expected_generation` is optional on every write; when given and
  different, `stale_project`: the configuration, a dependency or another
  document changed since the model checked.
- `publish(expected_source_sha256)` is required: the model publishes what it
  checked.
- `check_text` and `run` results carry the `generation` they were evaluated
  at and the `source_sha256` of every document they used, so a model can tell
  what exactly was checked.

The check happens under the writer lock, after the stable read, immediately
before the transaction's expected-state check — the lock does not replace
it, because the model's picture of the project can be older than the lock.

## Runs, cancellation and the journal

The shared executor (today `run.rs::execute_request`, which prints) becomes a
function that **returns data**: the report, what the journal held before the
run, and how the run ended. The CLI prints it; the server returns it. Four
ends, four results:

| End | Main phase | Cleanup | Journal | Result |
| --- | --- | --- | --- | --- |
| Completed | ran | ran | entry removed at the end (ADR 0009) | `passed` / `failed` with the report |
| Cancelled (MCP `notifications/cancelled`, or the server's own deadline of ten minutes — `timed_out`) | stopped at the next step | ran | entry removed at the end | `cancelled` with the steps done and the in-flight request marked *may have reached the API*; a timeout is **not** evidence that nothing was sent |
| Client disconnected | cancelled as above | waited for, up to ten seconds | removed when cleanup finished in time; otherwise stays and is reported as interrupted | no result (nobody to answer); the window takes the project back |
| Process killed | — | — | stays: the next writer reports it (the window at take-back or start, the server's next `run`) | `interrupted` in the next `run` result, as the CLI prints it |

The agent session keeps the lock through all of it. The server stays
responsive during a run: the run is a task; reads, `project_overview`,
`ping` and cancellation are answered meanwhile; one run at a time per server,
a second `run` is `run_in_progress`. A component runs with `inputs`
(KTLO-23b's dialog by name); a missing input without a default is an error
that lists the missing names.

## Publishing and lost responses

`publish` reprints the component, and canonical printing drops a comment
that belongs to no statement (ADR 0012, `CommentsWouldBeLost`). The tool
reports the comments it would drop and writes nothing until the model calls
again with `accept_comment_loss: true` **and** the same
`expected_source_sha256`; the consent is for that text only, as in the
window. The server never accepts on the model's behalf.

A response can be lost between the server and the client. For each write
that cannot be undone the model has a way to learn what happened, and the
server never retries by itself:

- `create_document`: a retry meets `path_exists` with the file's current hash.
- `publish`: `project_overview` lists every revision with its
  `source_sha256`; a retry with a `request_id` the server has seen returns the
  recorded outcome instead of a new revision.
- `run` with `live: true`: the same `request_id` rule; after a server restart
  the answer is `unknown_outcome` with the journal's interrupted entries, if
  any — and the honest statement that an external API may have received the
  requests once. Exactly-once is not promised anywhere.

## Secrets and the project boundary

One rule for every tool, resource, report, diagnostic, error and log line:
**no known secret value leaves the server.** Known values are those from
`APIFORGE_SECRET_<NAME>` and the `--secrets` file (reread at each call), set
on the store with `set_known_secrets` at start and at each reread.

- Reports are redacted by the runner as the window's are; the result says so.
- A **saved source that contains a known secret value** is withheld:
  `read_source`, `print` and the resource answer `secret_in_source` with the
  line, never a modified text presented as the file.
- Writes: `write_source`, `create_document` and `publish` go through the
  storage check that already refuses a staged text containing a known secret
  (`secret_in_text`, the value not echoed); a test puts the value inside the
  `text` argument.
- Diagnostics are static messages with ranges; a fix's replacement comes from
  the grammar, not from the text. Errors never quote the argument; stderr
  logs name the tool and the code, never the text.
- Paths: every path argument and every resource URI (percent-decoded first)
  passes `ProjectPath::working_source`; tests cover `../`, an absolute path,
  `.apiforge/…`, `%2e%2e`, a symlink inside the folder, a Windows reserved
  name and a case-insensitive duplicate.

## The contract

- Every tool has an `outputSchema`; results carry `structuredContent` and a
  readable `content`.
- A call that could not do what was asked is `isError: true` with
  `{ code, message, wrote, details }`. A run that completed with the verdict
  `failed` is **not** an error. Codes are a fixed list (`not_a_project`,
  `project_unreadable`, `unknown_document`, `invalid_text`, `invalid_inputs`, `not_a_component`,
  `invalid_checkpoint`, `stale_source`, `stale_project`,
  `held_by_window`, `handover_pending`, `handover_refused`, `held_by_other`,
  `live_disabled`, `comments_would_be_lost`, `secret_in_text`,
  `secret_in_source`, `path_rejected`, `path_exists`, `handle_exists`,
  `recovery_required`, `too_large`, `result_too_large`, `run_in_progress`,
  `cancelled`, `timed_out`, `unknown_outcome`).
- `wrote` is on every write result: `true`, `false`, or `"unknown"` for
  `recovery_required`; `changed` lists the paths a commit wrote.
- Positions: `line` and `column` 1-based, counted in **Unicode scalar
  values**; `end_line`/`end_column` exclusive; a line break is `\n`, and a
  `\r` before it belongs to the break. Tests use multi-byte characters and a
  CRLF file.
- Limits: a `text` over 1 MiB is `too_large` before anything else; a result
  over 2 MiB is `result_too_large` with how to narrow it (`full: false`, one
  path), never cut; diagnostics beyond 1 000 per document are counted in
  `omitted`, never dropped silently.
- Transport: a test spawns the binary and drives real stdio with both
  protocol revisions, a malformed line, an unknown method, and EOF, asserting
  that stdout holds nothing but protocol lines and that EOF ends the process
  with the lock released and runs cancelled.
- QA records the client's name and version (`claude --version`) and the
  workflow actually run, tool by tool, on each platform — not the presence
  of tools in a list.

## Live consent: the toggle

The owner's addition of 2026-10-27 updates decision 4: live requests
through the server are governed by a per-project setting the person can
change in the open window, and `--allow-live` remains for the headless case.

- **Where:** *Project settings*, a new section *Agent*: ☐ *Allow the MCP agent
  to send live requests* (mock-up M3). The dialog opens in observer mode with
  everything else disabled. While the box is ticked the header shows a *Live
  allowed* badge so the state is never hidden.
- **Scope:** live runs through MCP only; simulations are never affected; the
  window's own *Run live requests* is unchanged.
- **Default:** off.
- **Remembered:** per project (by its manifest id), per person, in the
  application's own preferences directory beside `preferences.json` — not in
  the manifest (Git-tracked: consent must not travel with the project) and
  not under `.apiforge/local` (needs the writer lock the observer does not
  hold). It persists until changed (decision 15 offers "for this window only"
  instead).
- **Enforcement:** the window pushes `permission { live }` at hand-over and on
  every change; the server keeps the latest and checks it immediately before
  a live run starts, under the lock. No tool argument can set it; a model
  reads it in `project_overview.live_requests` and a refusal says *live
  requests are off in this project's settings*.
- **Relation to `--allow-live`:** while a window is connected over the
  channel, **the window decides and the flag is neither necessary nor
  enough**; with no window connected, the flag decides as before. So a
  person who turns live off in the open window is obeyed even when the
  server was started with the flag, and a headless server keeps working. When
  the window closes, its consent goes with it and the flag applies.
- **A run already in progress** when the box is unticked finishes, cleanup
  included; the dialog says so (*A live run is in progress; it will finish.
  No new one starts.*). Stopping it mid-way would leave uncertain effects
  (ADR 0009) and is not what the toggle means (decision 16 offers the
  alternative).

## Decisions for the owner

Decisions 1–4 and 6–9 of revision 1 stand (taken 2026-10-27 on #59: 1 = A,
2 = A, 3 = A, 4 = A, 6 = A, 7 = A, 8 = A, 9 = B). Decision 5 (the lock per
write) is **superseded by decision 10**. New decisions, A recommended;
**10–18 taken 2026-10-27: all A**:

10. **Who holds the project while the agent works.** **A:** the server holds
    the lock for the whole agent session, the window observes and never takes
    it from a living process. Consequence: the person's editing pauses for
    the session, the preview follows every commit, the agent is never cut
    off mid-task. **B:** route every tool call to the window's session over
    the channel. Consequence: instant preview, but a second execution path
    for headless use, a protocol that carries every tool, the same pause on
    editing.
11. **When a clean window hands over.** **A:** at once, without a prompt — the
    person approved the tool call in Claude Code and sees the hand-over in
    the header. **B:** always ask in the window. Consequence of B: one more
    click per agent session, in the other application.
12. **Editing paused while read-only, for every holder.** **A:** yes — the
    window blocks edits whenever it is not the writer, whoever holds the
    project; the pause is one state with one wording. Consequence: typing in
    a read-only window, which could never be saved, is gone (a `removal`).
    **B:** only when the holder is an agent; another window or `apply` keeps
    today's behaviour.
13. **Unsaved work at a hand-over request** (MCP-08c). **A:** a prompt with
    *Save all and hand over* and *Not now*. **B:** add *Discard and hand
    over*. Consequence of B: a one-click loss of work, which nothing else in
    the window offers. **C:** no prompt; the agent is refused until the person
    saves (MCP-08b alone).
14. **Where the live toggle lives.** **A:** *Project settings*, section
    *Agent*, reachable while observing (mock-up M3). **B:** the *Run options*
    menu in the header. Consequence of B: one click away, but beside the
    window's own run mode, which it does not change.
15. **How long the consent lasts.** **A:** remembered per project and person
    until changed, with the *Live allowed* badge always visible. **B:** for
    this window only; off again at the next start.
16. **A live run in progress when live is turned off.** **A:** it finishes,
    cleanup included; no new one starts. **B:** it is cancelled; cleanup
    still runs; the agent's result says `cancelled`.
17. **Agent run status in the bottom bar** (MCP-10). **A:** yes, a status line;
    the report stays in the agent's answer. **B:** not in this sprint.
18. **Capacity.** The thirteen MCP tickets below sum to 305 minutes of owner
    review, plus 115 for the five interface findings (UX-00 to UX-05) and 5
    for KTLO-31. **A:** MCP tickets first, UX after MCP-11, in the owner's
    priority order, each UX ticket starting only when the WIP limit allows.
    **B:** defer UX-01 to UX-05 to the sprint after MCP. **C:** name the cuts.

## Mock-ups required before the window tickets are approved

Form 1 of [planning](../process/planning.md#mock-ups): review captures from
a throwaway branch, at 960 × 540 and 1600 × 1000, posted to the issue.

| Mock-up | Shows | Ticket |
| --- | --- | --- |
| M1 | The *Observing* badge in the header and a **status bar at the bottom of the window** (36 px, shown only while observing): the holder's name for an agent, another window and `apply`; the last-change line; *Take back* at the right end | MCP-07, MCP-08a, MCP-08b |
| M2 | The hand-over prompt with unsaved work: the files, *Save all and hand over*, *Not now* | MCP-08c |
| M3 | *Project settings* in observer mode with the *Agent* section and the live toggle; the *Live allowed* badge in the header | MCP-09 |
| M4 | The agent's run status in the same bar, in place of the last-change line: running, then passed | MCP-10 |

The captures are under `docs/qa/2026-10-27/MCP-00/` (`m1-*`, `m2-*`, `m3-*`,
`m4-*`, rendered 2026-10-27 from the throwaway branch `mock/mcp-00-m1-m4`)
and posted on #66 and #90–#94. Two points the captures raise for the
owner: the settings dialog's footer *Apply changes, then use Save all…* reads
oddly while observing and needs its wording; at 960 × 540 on a component tab
(which has *Publish*) the two badges *Observing · Live allowed* may collide
with the mode tabs — MCP-09 solves it (a shorter badge or a tooltip-only
state) and its QA proves it. Owner decisions of 2026-10-27 on the captures:
M2 and M3 approved as a whole (the settings footer and *Apply settings*
hidden while observing, MCP-09). M1 and M4 went through four rounds: the
notice under the tabs with a plain button (round 1, button rejected), the
window's own button style (round 2), everything moved to a bottom status bar
at the owner's suggestion (round 3), and the bar's margins corrected (round
4, **approved**): the committed `m1-*` and `m4-*` captures are round 4; the
`m3-header-*` capture still shows the round-1 notice, which M3 does not
decide. At 960 px with a run status the bar shortens the holder text, never
the status (MCP-10).

## Tickets

Sized by [complexity and risk](../process/estimation.md). Dependencies
("after") are on merged tickets: one ticket, one branch from `main`.

| Ticket | Goal | Size · review · risk | After |
| --- | --- | --- | --- |
| [MCP-01](MCP-01.md) | `apiforge mcp` answers Claude Code: overview, read, check, print — with versions | M · 20 min · `default` | — |
| [MCP-02](MCP-02.md) | The language reference, tested; the document only | M (docs) · 30 min | — |
| [MCP-07](MCP-07.md) | The lock names its holder; the window says who and takes the project back when it is free | S · 20 min · `visible UI`, `default` | — |
| [MCP-03](MCP-03.md) | Run a simulation (scenario or component), check a proposal, serve the reference and resources; the executor returns data; cancellation and timeout | M · 20 min | MCP-01, MCP-02 |
| [MCP-04](MCP-04.md) | Save and create from the server: the agent session, validation before the first write, versions, secrets in text, paths | M · 30 min | MCP-03 |
| [MCP-08a](MCP-08a.md) | Observer mode: editing paused while another holds the project; the window follows every commit by reload within two seconds, keeping its navigation | M · 30 min · `visible UI`, `default`, `removal` | MCP-07 |
| [MCP-05](MCP-05.md) | Publish from the server: comment-loss consent, expected hash, request ids | S · 15 min | MCP-04 |
| [MCP-06](MCP-06.md) | Live requests: the four ends of a run, the journal, `--allow-live` and the permission slot | M · 25 min | MCP-04 |
| [MCP-08b](MCP-08b.md) | Hand-over on request: the control channel, a clean window hands over and observes, refusal with unsaved work, `saved` rescans, *Take back*, release | M · 30 min · `visible UI`, `default` | MCP-08a, MCP-04 |
| [MCP-08c](MCP-08c.md) | The hand-over prompt when the window has unsaved work | S · 15 min · `visible UI` | MCP-08b |
| [MCP-09](MCP-09.md) | The live toggle in the project settings; the server obeys it while a window is connected | M · 25 min · `visible UI`, `default` | MCP-08b, MCP-06 |
| [MCP-10](MCP-10.md) | The agent's run status in the bottom bar | S · 15 min · `visible UI` | MCP-08b |
| [MCP-11](MCP-11.md) | The whole workflow, end to end, with a real client; the sprint's acceptance | S · 30 min | all above |
| KTLO-31 | The `timing; run in release` ignore names its ticket | S · 5 min | — |

Sum: 305 minutes of owner review for MCP, 5 for KTLO-31. Three tickets can
start at once (MCP-01, MCP-02, MCP-07: 70 minutes, which the WIP cap of 60
limits to two until one is accepted). The UX line (UX-00 to UX-05, 115
minutes) is in the backlog with its own capacity.

## Technical appendix

### The two designs for live preview

**A — observer (recommended).** Reuses: the watcher, `reload`, the stable
read, `has_live_writer`, the read-only chrome, the recovery view, the
journal's interrupted report. Adds: the holder description with an endpoint
(MCP-07), the channel with seven message kinds, observer mode (edits
blocked, reload keeping navigation), the agent session in the server (one
`ProjectStore` held for the session, a fresh `ProjectSession` per call from
the same store — `load_saved` with a cloned store, a new public
constructor), the hand-over prompt, the permission slot. The server's
execution path is the same with and without a window, so every tool is
tested once.

**B — route to the window.** Reuses the window's session for execution.
Adds: a protocol carrying every tool's arguments and results, run progress
and cancellation over the channel, a window-side executor per tool, a
headless executor anyway (the server must work without a window), and the
same pause on editing (the agent's `write_source` lands in the same
`DocumentSession` as the person's unsaved text). Twice the tests, and the
window becomes a dependency of the agent's work.

### Channel facts

Tokio is already in the workspace with `net`; `tokio::net::UnixListener`
and `tokio::net::windows::named_pipe` need no new dependency. The window
runs the listener on a background thread with a current-thread runtime, as
the watcher runs its scans. Endpoint names are random and carry no project
path; a Unix socket lives under `$XDG_RUNTIME_DIR` (or the temp directory)
because a socket path is limited to 108 bytes; a pipe is
`\\.\pipe\apiforge-<random>`. Same-user access only, which is the stance of
ADR 0011 for a local single-user application. Messages are newline-delimited
JSON with a version field; an unknown version or message is ignored and
logged. Windows pipes are not verified in this analysis: the first ticket
that ships the channel (MCP-08b) records the Windows evidence or its
absence explicitly.

### Session facts

- `create_document` (`session.rs:393`): `ProjectPath::working_source`,
  `require_current_registry`, size check, commits source + manifest, then
  `analyse_now` seals the checkpoint in a second transaction. The new
  `check_new_document` builds the analysis input from a synthetic entry
  without pushing a `DocumentSession`; the MCP create commits
  `DiagramRecord::capture(key, text, identity_base(model), context)` with
  the source and the manifest.
- `save` (`:900`): settles, writes source, checkpoint and dirty manifest in one
  commit with `expected: document.saved` — the storage's own stale check,
  which the `expected_source_sha256` check precedes with a clearer message.
- `has_unsaved_work` (`external.rs:412`): dirty manifest, held drafts,
  documents needing recovery, unsaved texts. `reload_project` (`:334`) carries
  drafts across; the observer never has any, so plain `reload` suffices.
- `follow_external` (`folder/mod.rs:923`) sets `shown = None` on a reload;
  MCP-08a keeps it when the document still exists, and restores the camera
  from `self.canvases` by document id.
- Comments: `require_comment_decision` and `accept_comment_loss` bind the
  decision to `source_version` (`comments.rs`); through MCP the binding is the
  source hash the model names.
- Runs: `RunHandle::cancel`, `is_finished`, `latest`; `RunnerService::shutdown`
  waits for cleanup; `RunStatus::{Running, Passed, Failed, Cancelled}`; the
  deadline and `report_interrupted` are the CLI's (`run.rs:25,209`).

### Lock and storage facts

`open_exclusive` (`transaction.rs:152`) opens the lock read-write and never
truncates it; MCP-07 writes `{"program","role","since","endpoint"}` to
`.apiforge/local/holder.json` after the confirmed lock — not into the lock
file, which Windows' mandatory lock makes unreadable while held.
`has_live_writer` (`:896`) probes with a shared try-lock over the KTLO-21
window. Head generation (`head.json`) advances on every commit and is what
`generation` reports. `set_known_secrets` (`:238`) feeds the staging check.
`ProjectPath::working_source` (`path.rs:50`) rejects `..`, absolute paths
and technical directories; symlinks are refused at operation time
(`:109`).

### What the window does today that the observer changes

`folder/mod.rs:455` sets the notice once at open; `chrome.rs` disables
*Save*, *Save all*, *Publish* and the settings dialog when read-only, and
nothing disables the code view, the board, the explorer's commands or the
environment dialog. The watcher applies `apply_external` every second: a
changed registry reloads only when `has_unsaved_work()` is false.

### Protocol and client facts

Unchanged from revision 1: MCP `2026-07-28` and `2025-11-25`, `rmcp =
"=3.8.0"` (`server`, `transport-io`, `macros`), `schemars` for schemas;
Claude Code names tools `mcp__apiforge__<tool>`, asks before each call
unless allowed, writes results over 50 000 characters to a file, mentions
resources as `@apiforge:apiforge://reference`. MCP's `notifications/cancelled`
is the client's way to cancel a `tools/call` in progress; `rmcp` surfaces it
as a cancellation token the run adapter watches.

### Demo shop after KTLO

Four documents (one scenario, three components at revision 1), no warning
since KTLO-25. Tests that need a diagnostic use a fixture with an injected
type mismatch (the `75c55f5` shape of `components/product.afs`), not the
demo as shipped.
