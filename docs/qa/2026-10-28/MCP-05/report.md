# MCP-05: Publish a component from the server — QA report

- Branch and commit: `feat/mcp-05-publish`. Acceptance tests `73aaab7`,
  criterion 3's fixture corrected in `5017ed6`; implementation `395cc28`;
  development guide `5c567ef`; the window's Publish compared through the
  window `0a5df14`; main with MCP-06 merged and reconciled in `c10f7c2`.
- Built with: `cargo build --release --locked -p apiforge-cli -p
  apiforge-desktop` at `c10f7c2` (release binaries `apiforge` and
  `apiforge-desktop`).
- Mock-up: not applicable; nothing in the window changes. Issue:
  [#64](https://github.com/jakubiszon26/apiforge/issues/64). Ticket:
  [`docs/tickets/MCP-05.md`](../../../tickets/MCP-05.md).
- Machine: Linux 7.0 (Ubuntu 24.04), Rust 1.99.0, rmcp 3.8.0, Python 3.12
  for the stdio client; Xvfb with Lavapipe for the window.

## Linux

The stdio sessions ran the release binary against a fresh demo-shop
checkout: the managed demo shop created by smoke item 1 (a first start on
empty `XDG_DATA_HOME` and `XDG_CONFIG_HOME`), copied and stripped of
`.apiforge/local`, which is what a fresh Git clone holds; every session got
its own copy. A small Python client drove `apiforge mcp --folder <checkout>`
over stdin and stdout with protocol revision `2025-11-25` and client name
`qa`. The whole transcript is [transcript.txt](transcript.txt): `>>` was
sent, `<<` was received (cut to about 220 characters), `--` lines are the
client's summary of the structured result, and `##` lines are actions
outside the protocol (hashing a file, running `apiforge apply`, starting or
capturing the window).

Whether the lock is held is observed from outside with `apiforge apply` of
the saved scenario: it takes the writer lock, changes nothing, and fails
with *The folder project is open in another writer session* while another
process holds the project.

### Scenario 1: publish writes the next revision (criterion 1)

Tests: `mcp05_criterion_1_publish_writes_the_next_revision`,
`mcp05_publish_over_stdio`, and through the window
`the_windows_publish_and_the_servers_write_the_same_bytes`.

Expected: on a valid saved component whose hash matches, the next unused
revision is written and the result names the handle, the revision, its
`source_sha256`, `generation` and `wrote: true`; `project_overview` lists
the revision with that hash; `check_text` accepts a call to it; the files
are those of the window's Publish.

Observed: as expected. In session 1, `publish(components/product.afs,
eba04ff2…)` answered `handle: "product"`, `revision: 2`, `revision_path:
.apiforge/revisions/get-product/2.afs` (the folder is the component's id,
not its handle), `source_sha256` equal to the file's hash, `generation`
0 → 1, `wrote: true`, and `changed` named exactly the three files whose
bytes changed: the revision, `.apiforge/diagram.json` and
`apiforge.project.json`. The working source was not rewritten. The
overview then listed `product` revisions 1 and 2 with their hashes, and
`check_text` of the scenario calling `product@2(…)` was valid with no
diagnostic.

The comparison with the window's Publish: the native window could not be
driven on this machine (no input injection: neither `xdotool` nor the
XTEST library is installed, and the egui inspection server did not
connect), so it was made through the window's own code instead. The desktop
test presses *Publish* in the component's header of a headless window on
one copy of the demo shop, publishes from the agent session on another, and
compares every file outside `.apiforge/local`: the revision file, the
registry and the checkpoint are byte-identical. The library test makes the
same comparison against `ProjectSession::publish`, the function the window's
storage worker calls.

The demo product is already canonical, so its revision's bytes, and hash,
equal the working source's: revisions 1, 2 and 3 of the transcripts all
have `eba04ff2…`. A hash tells which text a revision holds, the revision
numbers how many there are.

### Scenario 2: a stale hash (criterion 2)

Test: `mcp05_criterion_2_a_stale_hash_publishes_nothing` (the component
changed by `apply`'s own code between the read and the publication).

Observed: as expected. In session 1 a publication naming another hash
answered `stale_source` with the current hash in `details.source_sha256`,
`wrote: false`; the files were unchanged.

### Scenario 3: nothing existing is overwritten (criterion 3)

Test: `mcp05_criterion_3_publishing_twice_yields_two_revisions`.

Observed: as expected. Two publications, with a write between them, made
revisions 2 and 3; revisions 1 and 2 kept their bytes. Session 4 shows the
same on the binary: revision 3 beside revision 2.

### Scenario 4: comments printing would drop (criterion 4)

Tests: `mcp05_criterion_4_comment_loss_needs_consent_bound_to_the_hash`,
`mcp05_publish_over_stdio`.

Observed: as expected. In session 2 the model saved the component with a
comment before its header. `publish` answered `comments_would_be_lost`
with `details.comments: [{line: 1, column: 1, text: "// A loose note from
the model."}]` and nothing was written. The consent with the earlier hash
answered `stale_source`; the consent with the current hash published
revision 2. The revision holds no comment; the working source still holds
it, byte for byte as written. The library test also changes the comment
after a refusal: the consent does not carry over to the new text, which is
refused again with its own comment listed.

### Scenario 5: what cannot be published (criterion 5)

Test: `mcp05_criterion_5_what_cannot_be_published_is_refused`.

Observed: as expected. In session 3 a component saved outside API Forge
with a call of an unknown component on line 2 answered `invalid_text` with
the two errors at 2:12–2:27; the scenario answered `not_a_component`
(session 1); a `diagram.json` replaced by text that is not a checkpoint
answered `invalid_checkpoint`. Nothing was written in any case.

### Scenario 6: request ids (criterion 6)

Tests: `mcp05_criterion_6_an_answered_request_id_returns_the_recorded_result`,
`mcp05_publish_over_stdio`, `mcp05_a_request_id_is_bounded_and_never_a_secret`
and two unit tests of the record in `agent.rs`.

Observed: as expected. In session 1 the publication with `request_id:
"qa-1"` repeated answered the same result with `replayed: true`, and no
file changed; the same id with another hash answered `invalid_inputs`
naming the path it answered. In session 4 the server ended after a
publication with `request_id: "qa-lost"`, as if its response had been lost;
a new server did not know the id and published revision 3, and the
overview before and after showed what existed. The record outlives
`finish_editing` and ends with the process. As for a live run (MCP-06), an
id over 256 bytes is `too_large` and one holding a known secret value is
`secret_in_text`, the value not echoed.

The record is in memory, not in MCP-06's
`.apiforge/local/agent-requests.json`: criterion 6 says a fresh server does
not remember a publication's id. The two records are separate, so the same
id may name a live run and a publication.

### Scenario 7: the agent session's rules (criterion 7)

Tests: `mcp05_criterion_7_the_agent_session_rules_apply`,
`mcp05_criterion_7_a_window_holding_the_project_refuses_publish_over_stdio`.

Observed: as expected. In session 5 the release window opened the checkout
(`holder.json` role `window`); `publish` answered `held_by_window` naming
the window and since when, nothing was written, and `read_source` in the
same session still answered. In session 1 the lock stayed with the session
after the publication (`apiforge apply` refused, the overview said
`holder: "agent_session"`) and was free after `finish_editing`.

### Scenario 8: the window beside the server (criterion 8)

Test: `a_read_only_window_lists_the_revision_the_server_published` (headless
window, desktop crate).

Observed: as expected. A window opened while the agent session held the
project lists revisions 1 and 2 of `product`; after the server published
revision 3 and the window's next scan, it lists 1, 2 and 3, the working
document's base revision is 3, no error notice is shown and no document is
unsaved. In session 6 the release window opened beside a session that had
just published: the bar says *qa via apiforge mcp · since 11:19 · editing
is paused here; this window follows every change*
([960 × 540](observing-published-960x540.png),
[1600 × 1000](observing-published-1600x1000.png)).

### Verification (criterion 9)

At `c10f7c2`, Linux:

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: no
  warning.
- `cargo test --workspace --all-targets --locked`: 1 281 passed, 0 failed,
  31 ignored over 59 test binaries. A first run under load failed one
  MCP-08a test, `a_document_the_holder_registers_is_listed_without_a_notice`
  (*two or more nodes* match the new file's name: the explorer entry and
  the bar's last change), which this branch does not touch; it passed five
  times alone and in the full rerun.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: exit 0.

The analysis, the scenario language, the layout and the code view are
untouched, so the analysis budget, the fuzzers, the layout bench and the
frame budgets were not run.

## Windows

Pending: filled from this pull request's CI run (windows-2022).

## Not verified

- **Claude Code with `publish`: not verified by the implementing agent;
  left to the owner.** With the release binary registered as in the guide,
  `/mcp` should list `publish` after `create_document`.
- **The native window's Publish button with the release binary**: no input
  injection on this machine (scenario 1). The bytes were compared through
  the window's code in a headless window.
- A publication that fails after its activation, through the release
  binary or a test: the mapping is the one `write_source` uses
  (`recovery_required`), not exercised for `publish`.
- A component renamed after publication (`HandleChanged`) and an existing
  revision file (`RevisionExists`): mapped to `invalid_text` and
  `path_exists`, not exercised by a test.
- The hand-over from a window (MCP-08b): not in scope. Today a publication
  while the window holds the project is refused.

## Regression smoke list

Linux, 2026-10-28, release binaries of `c10f7c2`, on Xvfb (`xvfb-run`, X11
backend, Lavapipe), with empty scratch `XDG_DATA_HOME` and
`XDG_CONFIG_HOME`.

| # | Result |
| --- | --- |
| 1 | Pass: `apiforge-desktop` with no arguments created and opened the managed demo shop under the scratch data directory (`apiforge/projects/shop`, `recent-projects.json`). Ended by `timeout` after 20 s; nothing on stderr. |
| 2 | Partial: `--screenshot … --folder <checkout> --mode diagram` at 960 × 540 and 1600 × 1000 opened the project with its scenario (scenario 8's captures); Code mode was not captured. |
| 7 | Pass for the bytes of a publication: the server's and the window's are byte-identical (scenario 1). Ctrl+S in the window: not run, no window change. |
| 3–6, 8–10 | Not run: no interface change. The workspace suite that covers them passes. |
