# MCP-18: `project_overview` says who decides live requests while a window holds the project

- Size: S · Owner review: 10 min · Risk: MCP output compatibility
  (`live_requests` gains a state); no `format`, `migration`, `default`,
  `visible UI` or `removal`
- State: Implementation (issue #190) · decision 1 taken: A (2026-11-02)
- Analysis model: Claude Fable 5.1 · Implementation model: Claude Opus 5.5

## Goal

A model that reads `project_overview.live_requests` before any window is
connected is not told "no" by the start flag when an open window holding the
project would allow its live run, nor "yes" when that window would refuse it.
The overview says that the window decides, at run time.

## Cause

`ApiforgeServer::overview_tool` (`crates/apiforge-mcp/src/server.rs`) reports
`LivePermission::describe()`, the permission slot of `live.rs`, which is
`Flag(--allow-live)` until a window is connected over the control channel.
Since MCP-09, `run_for` defers that flag when `agent.window_decides()` — the
project is held by a window that names an endpoint — and the live run hears
the window's consent before asking for the project, fail-closed (owner
decision on #93, 2026-11-01: a window that is off or silent for two seconds
refuses, whatever the flag). The overview never connects (MCP-12 D1), so in
exactly the state where the flag does not decide it reports the flag:
`no --allow-live` while the box is ticked, so a model may never try; and
`--allow-live` while the box is off, so the run it expects is
`live_disabled`. The holder and the endpoint are already known to the
overview (`holder: window`, `hands_over_on_write: true`); only
`live_requests` ignores them.

## What the owner will see

- Changed: while a window holding the project names an endpoint and no
  window is connected yet, `live_requests` says the window's setting decides
  and is asked when the run starts (shape per decision 1); the
  `project_overview` description and the server's instructions say so. Every
  other state is unchanged: no holder, this server's session, another
  process, a window without an endpoint (the flag), and a connected window
  (`window setting` / `off in the project's settings`).
- The overview still opens no connection, takes no lock and changes no
  setting (MCP-12 D1, ADR 0014 decision 6). Nothing in the window changes.
  Removed, hidden, migrated, defaults: nothing.

## Decisions for the owner

1. **How `live_requests` says "the window decides at run time".** Today it
   is `{ allowed: boolean, source: string }`, `source` one of four texts.
   - **A (recommended): `allowed` becomes nullable.** `allowed: null` means
     the answer is not known until the run asks the window; `source:
     "window setting, asked when the run starts"`. The schema's `allowed`
     is `boolean | null`, still required. A model reading `null` knows to
     try; `true`/`false` keep their meaning. A typed client generated from
     the prerelease schema (`bool`) would break on `null`: the same class of
     prerelease change the owner accepted in MCP-12 D1.
   - **B: a new field.** `allowed` stays `false` (the flag's answer) and a
     new required `asks_window: boolean` is `true` only in this state, with
     the `source` text of A. Additive for every client; a model that reads
     only `allowed` still may not try.
   - **C: text only.** No schema change: `allowed: false` and the `source`
     text explains. Smallest change; a model trusting `allowed` is still
     told "no".

   **Decided 2026-11-02: A.** `allowed` becomes nullable: `allowed: null`
   means the window decides when the run starts, with `source: "window
   setting, asked when the run starts"`. The schema's `allowed` is
   `boolean | null` and stays required. The owner accepts this prerelease
   schema change: a typed client generated from the earlier schema reads a
   `bool` and must be regenerated.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

Each criterion runs through the real `apiforge mcp` over stdio
(`crates/apiforge-cli/tests/mcp.rs`), with a window-role holder as
`mcp12_cli_window_process` builds it: a `apiforge_channel::Listener` and
`ProjectStore::open_exclusive_as(root, &Holder::window().with_endpoint(…))`.
Shapes are written for A; B and C change the asserted object only.

1. A window holds the project with a valid endpoint, the server has no
   `--allow-live`, nothing has connected: `project_overview.live_requests`
   is `{ "allowed": null, "source": "window setting, asked when the run
   starts" }`; `holder` is `window` and `hands_over_on_write` `true` as
   before; the listener accepted no connection during the call and the
   project folder is byte-identical afterwards.
   `mcp18_criterion_1_a_holding_window_decides_at_run_time`.
2. The same with `--allow-live`: the same answer (the window decides,
   fail-closed). `mcp18_criterion_2_the_flag_does_not_speak_for_a_window`.
3. The flag still speaks when no window can be asked: no holder
   (`mcp06_criterion_1_without_allow_live_over_stdio`, unchanged), a window
   holding without an endpoint, and a foreign agent holding
   (`Holder::agent()`): `no --allow-live` / `--allow-live` as today.
   `mcp18_criterion_3_the_flag_speaks_when_no_window_can_be_asked`.
4. Once the window is connected — the first live run or write asked it, it
   granted and said `permission { live: true }` — the overview says
   `{ allowed: true, source: "window setting" }`; after the window's channel
   closes, the flag again. `mcp18_criterion_4_a_connected_window_speaks_for_itself`
   (stdio); the in-process `mcp06_criterion_4_the_window_decides_while_connected_the_flag_otherwise`
   stays as it is.
5. `tools/list` advertises the new shape in `project_overview`'s
   `outputSchema` (A: `allowed` admits `null` and stays required) and every
   overview result of every stdio session validates against it (the MCP-14
   harness). `mcp18_criterion_5_the_schema_admits_the_new_state`.
6. Documentation follows: `docs/development.md` (the sentence "The overview
   reports the flag until a window is connected" and the list of `source`
   values), an MCP-18 amendment of ADR 0014 under the MCP-09 amendment, the
   `project_overview` description and `INSTRUCTIONS` in `server.rs`. No
   change to the window, the channel messages, `agent-consent.json` or the
   project file format. Checked in the QA report.

## Not in scope

- Connecting to the window from the overview to read its consent: MCP-12
  D1 stands (the overview never connects); the run asks.
- A window of a build older than MCP-09, which never says `permission`: it
  refuses by the owner's fail-closed decision on #93, and the overview's
  "asked when the run starts" is right for it too.
- `apiforge run --live` from the command line (MCP-15) and the semantics
  of `hands_over_on_write`.
- Error results of the server: MCP-17 (#188).

## Technical appendix

- **Files.** `crates/apiforge-mcp/src/server.rs::overview_tool`: when
  `permission.current()` is `Permission::Flag(_)` and
  `agent.window_decides()`, report the run-time state instead of
  `describe()` — the same predicate `run_for` uses, so overview and run
  agree by construction (about eight lines). `crates/apiforge-mcp/src/live.rs`:
  a third describing arm. `crates/apiforge-mcp/src/tools.rs::LiveRequests`:
  with A, `allowed: Option<bool>` and a `#[schemars(transform = …)]` that
  keeps it required, as `require_holder_details` already does for
  `holder_details` (schemars 1.2.2 drops an `Option` field from `required`).
  `agent.window_decides()` opens the store read-only and reads
  `.apiforge/local/holder.json`; it is `false` without a hub, and the stdio
  server always sets one (`connect_windows`, `server.rs` line 817).
- **Tests.** `crates/apiforge-cli/tests/mcp.rs`: criteria 1–5 modelled on
  `mcp12_cli_window_process` (a window-role listener; criterion 4 grants and
  sends `Message::Permission { live: true }` like the desktop's channel
  thread); the listener's accept count proves the overview did not connect.
  `crates/apiforge-mcp/tests/acceptance.rs` lines 176, 2763 and 2906 assert
  the flag states and keep passing.
- **Documentation.** `docs/development.md` lines 643–649;
  `docs/decisions/0014-agent-sessions.md`, a four-line MCP-18 amendment
  after the MCP-09 one ("the overview still never connects, so before a
  window is connected it reports the flag" becomes "… it reports the flag
  unless a window holding the project names an endpoint: then it says the
  window decides at run time").
- **Order with MCP-17.** Different cause and code (the result envelope
  there; the overview's content here); not one fix. Both add to
  `crates/apiforge-cli/tests/mcp.rs`: MCP-17 changes the shared helpers, so
  it merges first or this ticket rebases its tests on it.
- **Estimate.** S: the predicate exists, no new state, no protocol change;
  regression potential is the four existing `source` states, each asserted
  by an existing test (criteria 3 and 4). About 200 changed lines: 25 in
  the server, 130 in tests, 45 in documentation. Owner review 10 minutes.
- **`ready:sol`.** Once decision 1 is taken, every item of the autonomy
  checklist holds: files named, criteria with tests, no format, migration
  or removal flag.
- **Probe.** No build was made for this analysis: the cause is read from
  `overview_tool`, `live.rs::describe`, `run_for`'s deferral and the
  existing tests that pin the four flag and window states.
