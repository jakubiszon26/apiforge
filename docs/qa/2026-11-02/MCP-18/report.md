# MCP-18: `project_overview` says who decides live requests while a window holds the project — QA report

- Branch: `docs/mcp-18-overview-live`, with `main` at `109562e` (MCP-17)
  merged in `f73a3c1`. Owner's decision recorded in `0d5f7a5`; acceptance
  tests `176a5ee`; implementation `b6ae1e0` and `e2d97f3` (the schema's
  description of `allowed` without the ticket's name); review round 1 after
  this report's transcripts, which a malformed endpoint alone changes.
- Built with: `cargo build --release --locked -p apiforge-cli`, Rust 1.99.0,
  rmcp 3.8.0. "Before" is the release binary of `176a5ee` (the server code of
  `main` at `109562e`); "after" is the release binary of `e2d97f3`.
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#190](https://github.com/jakubiszon26/apiforge/issues/190). Ticket:
  [`docs/tickets/MCP-18.md`](../../../tickets/MCP-18.md), decision 1 = A
  (`allowed` becomes nullable), taken 2026-11-02.
- Analysis model: Fable. Implementation model: Claude Opus 5.5.

## Criteria and their evidence

| # | Criterion | Tests | Release binary |
| --- | --- | --- | --- |
| 1 | A window holds the project with a valid endpoint, no `--allow-live`, nothing connected: `live_requests` is `{ allowed: null, source: "window setting, asked when the run starts" }`; `holder: window`, `hands_over_on_write: true`; no connection; the folder byte-identical | `mcp18_criterion_1_a_holding_window_decides_at_run_time` | Scenario 1 |
| 2 | The same with `--allow-live` | `mcp18_criterion_2_the_flag_does_not_speak_for_a_window` | Scenario 1 |
| 3 | The flag still speaks with no holder, a window without an endpoint (or with a malformed one, review round 1), a foreign agent | `mcp18_criterion_3_the_flag_speaks_when_no_window_can_be_asked` (all four holders, with and without the flag); `mcp06_criterion_1_without_allow_live_over_stdio` unchanged | Scenario 3 (no holder) |
| 4 | A connected window speaks for itself (`window setting`); after its channel closes, the flag again | `mcp18_criterion_4_a_connected_window_speaks_for_itself` (stdio); `mcp06_criterion_4_the_window_decides_while_connected_the_flag_otherwise` (in process, states unchanged) | Scenario 2 |
| 5 | `tools/list` advertises `allowed: boolean \| null`, still required; every overview result validates | `mcp18_criterion_5_the_schema_admits_the_new_state`; `Server::response` validates every overview of every stdio session (MCP-14) | Scenario 1 |
| 6 | Documentation: development guide, ADR 0014 MCP-18 amendment, the tool description and `INSTRUCTIONS`; no change to the window, the channel messages, `agent-consent.json` or the file format | — | Scenario 1 (description and instructions as served); *Documentation* below |

## Linux

### Scenario 1: a window holds the project and nothing has connected, before and after

A scratch driver (Rust, kept outside the repository) built from the
project's own crates stood in for the window: it prepared a fresh demo-shop
folder project, took its writer lock with
`ProjectStore::open_exclusive_as(root, Holder::window().with_endpoint(…))`
and listened on that endpoint with `apiforge_channel::Listener`, counting
the connections it accepted. It then started the release
`apiforge mcp --folder <it>` without and with `--allow-live`, sent
`initialize` (revision `2025-11-25`), `tools/list` (first server only) and
`project_overview`, closed stdin, waited 200 ms and compared every file
under the folder with its state before the server started. Transcripts:
[before.txt](before.txt) and [after.txt](after.txt); `>>` sent, `<<`
received, `--` the driver's findings. Each overview line prints `holder`,
`holder_details.role`, `hands_over_on_write` and `live_requests`.

Expected before: the overview reports the start flag, which does not decide
in this state.
Observed before: as expected. Without the flag
`{"allowed":false,"source":"no --allow-live"}`, with it
`{"allowed":true,"source":"--allow-live"}`; `holder` `window`,
`hands_over_on_write` `true`. The schema's `allowed` was
`{"type":"boolean"}`; the description said only "whether live requests are
allowed".

Expected after: `{"allowed":null,"source":"window setting, asked when the
run starts"}` with and without the flag; the window accepts no connection;
the folder is unchanged; the schema admits null and keeps `allowed`
required; the description and the instructions say the new state.
Observed after: as expected. Both servers answered
`{"allowed":null,"source":"window setting, asked when the run starts"}`
with `holder` `window` and `hands_over_on_write` `true`; the window
accepted 0 connections in each session and the folder was byte-identical
(lock file and `holder.json` included). `tools/list` advertised
`live_requests` as
`{"properties":{"allowed":{"type":["boolean","null"],…},"source":{…}},"required":["allowed","source"]}`.
The tool description now reads "whether live requests are allowed
(live_requests: allowed true or false with its source; null while a window
holding the project decides, asked when a live run starts, since the
overview never connects)", and the instructions' live sentence ends
"(project_overview.live_requests; allowed is null while an API Forge window
holding the project decides, asked when the live run starts)". Each process
exited 0 within 10 ms of the end of stdin.

### Scenario 2: the window grants a write and says its consent, then closes

The same driver window, this time answering as the desktop's channel thread
does: on the first connection it says `permission { live: true }`, reads the
request, lets the lock go and answers `granted`, then observes until it is
closed. The server, without `--allow-live`, read the overview, wrote a new
scenario with `create_document`, read the overview again, and once more
after the driver closed the window.

Expected: before the write, as scenario 1 (the flag before the change,
`null` after); once the window is connected `{"allowed":true,"source":"window
setting"}` with `holder` `agent_session`; after it closes, the flag.
Observed: as expected, in both binaries apart from the first overview. The
window accepted one connection, heard `Request { client: "mcp18-qa", role:
None }` and granted; `create_document` wrote; the overview said
`window setting` and `true`; the server logged `a window disconnected`, and
the next overview said `{"allowed":false,"source":"no --allow-live"}`.

### Scenario 3: no holder

The server started without and with `--allow-live` on a folder nobody
holds. Expected and observed, in both binaries: `no --allow-live` /
`false` and `--allow-live` / `true`, `holder` `none`.

### Scenario 4: the tests before and after the implementation

At the acceptance commit `176a5ee`, with the server unchanged, criteria 1,
2 and 5 failed for the reason the ticket names: criterion 1 read
`{"allowed": false, "source": "no --allow-live"}`, criterion 2
`{"allowed": true, "source": "--allow-live"}`, and criterion 5 found that
the advertised schema rejects an overview whose `allowed` is `null`.
Criteria 3 and 4 passed: they guard the states the change keeps. After
`b6ae1e0` all five pass without a change to their assertions. The
in-process acceptance tests of `apiforge-mcp` compare `allowed` with
`Some(true)`/`Some(false)` now that it is an `Option`; the states they
assert are unchanged.

Verification on `98f2918` (review round 1, the code this report ends
with): `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --all-targets --locked` (1409 passed, 0 failed,
43 ignored; the malformed endpoint is one more case inside criterion 3, so
the count is that of `b6ae1e0`) and
`cargo run -p apiforge-convert-v1 --locked -- --fixtures --check` pass on
Linux. The CI of `98f2918` passed on `ubuntu-24.04` and `windows-2022`.
The transcripts are of `e2d97f3`'s release binary: round 1 changes the
overview only for a malformed endpoint, which they do not drive.

### Documentation

- `docs/development.md`, *Live requests*: "The overview reports the flag
  until a window is connected" now states the exception and its `null`; the
  list of `source` values gains *window setting, asked when the run starts*;
  the tool table says `allowed` may be `null`.
- `docs/decisions/0014-agent-sessions.md`: an MCP-18 amendment directly
  after the MCP-09 one.
- `crates/apiforge-mcp/src/server.rs`: the `project_overview` description
  and `INSTRUCTIONS`, as served in scenario 1.
- No change to the desktop crate, `apiforge-channel`, `agent-consent.json`
  or the project file format: the diff touches none of them.

## Windows

Not run locally. The acceptance tests run on the CI of this branch
(`windows-2022`), where the scripted window listens on a named pipe; the
pull request's checks are the Windows evidence.

## Not verified

- A real API Forge window holding the project: the window was the driver
  built from the project's own lock and channel crates. The desktop's own
  MCP tests (`mcp09_*`) pass unchanged; none reads the overview before the
  window is connected.
- A real MCP client (Claude Code, Codex) reading `allowed: null`, or a typed
  client generated from the earlier schema (which the owner accepted would
  break on `null`).
- A window whose holder file names a malformed endpoint, over the release
  binary: review round 1 (cubic on `f3039eb`) narrowed the overview to an
  endpoint that parses, as `hands_over_on_write` does, so it reports the
  flag there; criterion 3 asserts it over stdio, and fails without the
  narrowing. `run` is unchanged for that holder (it still defers the flag
  and ends `held_by_window`, as since MCP-09).
- The regression smoke list: not run; nothing in the window changes.
