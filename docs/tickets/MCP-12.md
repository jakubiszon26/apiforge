# MCP-12: A window holder is discoverable and can hand the project to MCP

- Issue: [#123](https://github.com/jakubiszon26/apiforge/issues/123)
- Size: M · Owner review: 40 min · One ticket and one implementation PR.
- State: Analysis and decisions D1–D3 approved by the owner on 2026-10-28; ready for SOL.
- Risk: MCP output compatibility; connection discovery changes by default (already decided); no project-format migration.
- Analysis model: GPT-6 Astra · Implementation model: GPT-6.1 SOL, reasoning high.

## Goal

An MCP client recognises that an open API Forge window can hand over the
project and tries a write. The request reaches the window even when the MCP
process lacks `XDG_RUNTIME_DIR`; failures explain what is unavailable.

## What the owner will see

- New: `project_overview` distinguishes a window, this server's agent session,
  another process and no holder, with the holder's description.
- Changed: overview/run guidance describes hand-over on the first write or
  permitted live run. An unreachable window's refusal says that connection
  failed and reading remains available; it does not request closing the window.
- Default behaviour: clients search the already approved candidate directories;
  Windows keeps its existing named pipe. A visible possible Windows/WSL mismatch
  receives an explanation in the existing refusal.
- Removed, hidden, migrated: nothing. Desktop controls, hand-over consent,
  live permission and writer-lock rules stay as approved in MCP-00/ADR 0014.

## Mock-up

Not applicable: no desktop layout, control or interaction is redesigned.
The existing observer and hand-over states retain MCP-00's approved M1 design.

## Acceptance criteria

| # | Observable result | Assigned verification |
| --- | --- | --- |
| 1 | Overview reports `none`, `window`, `another_process` or this server's `agent_session`; details and hand-over eligibility follow D1. It writes nothing and never claims a foreign agent is this session. | MCP acceptance matrix T1; stdio/schema T2 |
| 2 | Tool descriptions explain hand-over without promising success or bypassing live consent; unreachable/legacy windows retain `held_by_window`, `wrote: false`, holder details and readable sources. | T2, T3 |
| 3 | Linux: window under `/run/user/<uid>`, MCP without XDG and with a different temporary directory; first write succeeds, window observes, finish/EOF returns ownership. | Real stdio + desktop T4 |
| 4 | Client candidates follow the approved order; only a valid endpoint in a private directory belonging to the holder-file owner is contacted. Listening location and endpoint-file syntax do not change. | Channel/storage T5; reverse-direction T4 |
| 5 | Native Windows: with only PATH/SystemRoot in the MCP environment, the same write and return of ownership succeed. | T4 on windows-2022 |
| 6 | An unreachable window endpoint on a Linux `/mnt/<drive>/` project gets the qualified same-side explanation; reads work and the refusal writes nothing. Stale metadata alone does not block a free lock. Both setup guides state the unsupported mix. | T3, documentation review T6 |

## Not in scope

Cross-OS lock interoperability, Windows-pipe access from Linux, arbitrary socket
directory discovery, new desktop behaviour, new channel messages, release
packaging/version changes, project-file migration and automatic retries of writes.

## Technical appendix

### Owner decisions approved 2026-10-28

**D1 — MCP output contract: approved.** The output shape (new fields are siblings of
the existing string, not a replacement object):

```json
{
  "holder": "window",
  "holder_details": {"role": "window", "client": null, "since": "2026-10-28T18:00:00Z"},
  "hands_over_on_write": true
}
```

Keep the existing `none`, `another_process`, `agent_session` spellings and add
`window`. `holder_details` is null for no holder or an unreadable description;
otherwise it contains the same role/client/since values as write refusals.
Other agents remain `another_process`, with role `agent` in their details.
`hands_over_on_write` is true only for a window advertising a syntactically
valid endpoint; it means that a write can request hand-over, not that the
endpoint was contacted or that unsaved work, a busy window or live permission
will allow it. It is false for all other cases. Overview does not open a
control connection, take the writer lock or inspect another writer's journal.
The endpoint name, filesystem paths and uid are not added to MCP output.

There is **no separate MCP application-contract version today**:
`initialize.serverInfo.version` and overview `server_version` use the workspace
crate version (currently 0.1.0); `reference_version` versions the language
reference, MCP protocol negotiation versions the transport, and the `version: 1`
in holder/channel data versions those local formats. None should be repurposed.
The generated `outputSchema` currently contains a closed three-value holder
enum. Adding `window` is therefore not compatible with every old typed client,
even though the JSON value remains a string.

The owner accepted this enum extension for the current prerelease API. Publish
its exact output schema through `tools/list` and document the change without
introducing a separate contract-version system. The eventual release version
remains a separate release decision. Acceptance tests fix the string-based shape
above, including `window`, `holder_details` and `hands_over_on_write`.

**D2 — scope of the Windows/WSL diagnosis: approved.** When a live
window holder has an endpoint, connection fails, and the Linux project root is
under `/mnt/<single ASCII drive letter>/`, keep `held_by_window` and add wording
such as: “The window could not be reached. This project is on a Windows drive;
if the window and MCP run on different sides of Windows/WSL, run both on the
same side. Nothing was written; reading remains available.” The path and failed
connection are evidence of a possible mismatch, not proof of Windows ownership.
Use the existing error envelope; do not add a new error code.

`ProjectStore::holder()` trusts metadata only when the OS lock probe finds a
live writer. If a Windows lock is invisible to Linux, the existing path may
take the lock before attempting any connection; this diagnosis cannot catch
that unsupported configuration. A stale holder file looks the same. The owner
accepted this diagnostic boundary: do not block writes merely because an old
holder file remains on a mounted drive. Preventing an invisible cross-OS writer
would need a separate ownership design and is outside this ticket.

**D3 — estimate and delivery boundary: approved.** Deliver the complete bounded
scope as **one M ticket, 40 owner review minutes**, on one branch and in one PR.
This replaces the original S / 10 min estimate. D1 and D2 bound the work: no new
ownership mechanism, compatibility negotiation or cross-OS prevention, and reuse
of existing tests. The full regression and platform evidence below remain
required. Implementation is assigned to **GPT-6.1 SOL with reasoning high**;
acceptance tests precede implementation in their own commit. Any additional WSL
prevention, migration or contract-version negotiation reopens the estimate before
code. Four PRs / 60 minutes remains the global WIP limit.

### Owner decisions already recorded on the issue

- Client candidates: XDG when set, `/run/user/<uid>` using the ownership of
  `.apiforge/local/holder.json`, `temp_dir()`, `/tmp`; choose the first existing
  private socket belonging to that user. No `unsafe`. Listener location unchanged.
- Windows uses its current pipe independent of environment. Test windows-2022
  with the server environment cleared except PATH/SystemRoot.
- Mixed Windows window / Linux MCP in WSL is unsupported; both must run on the
  same side. Diagnose an observable mismatch and add one sentence to each of
  README.md and CLAUDE-CODE.md.

### Evidence and reuse

- `apiforge-mcp/src/tools.rs::project_overview` currently maps every live writer
  to `AnotherProcess`. `server.rs::overview_tool` recognises this server's active
  session. Preserve its own-journal handling and the foreign-journal exclusion.
- `apiforge-storage/src/project/transaction.rs::holder` already distinguishes
  no live holder from an unknown live one. `project/holder.rs::Holder` provides
  role/client/since/endpoint, bounds input and ignores malformed/unknown versions.
  Reuse this rather than independently decoding holder JSON in MCP.
- `agent.rs::description` already defines refusal details. `held` has obsolete
  close-window advice. `take_lock` / `ask` and `handover.rs::Hub::ask` already
  implement refusal, timeout, grant and lock reacquisition; do not redesign them.
- `apiforge-channel/src/lib.rs::Endpoint` validates a random endpoint name,
  uses one environment-derived socket path for both bind and connect, and bounds
  Unix socket path length. Listener creates a new 0700 directory and 0600 socket.
- Desktop `folder/handover.rs::connect_to_holder` is also a client. Apply the
  discovery rule there too: MCP started without XDG can listen in temp while
  a window opened later has XDG. This is the same defect in the reverse direction.
- Existing desktop MCP-08b acceptance tests and `apiforge-mcp::test_support`
  already drive a real stdio server alongside the desktop harness; reuse them.
  `apiforge-cli/tests/mcp.rs` verifies the actual CLI binary and protocol output.

### Connection and ownership details

Separate client discovery from listener address selection. Obtain the Unix uid
with safe `std::os::unix::fs::MetadataExt::uid` from the same opened holder file
whose bytes were read, through storage's existing path boundary checks; keep it
in memory, never in the serialized holder. Pass that context to the shared
channel client, including the desktop observer path. Do not derive uid from a
mutable environment variable or trust a new JSON field.

Deduplicate absolute candidate paths, preserve order, reject overlong Unix
addresses, malformed endpoint names, symlink endpoint directories/sockets,
non-socket nodes, wrong owners and group/world-accessible endpoint directories
or sockets. A shared `/tmp` base is allowed: privacy is provided by the endpoint's
own directory. Never create, chmod, remove or replace candidates during discovery.
Continue past absent or unsafe candidates. Choose the first valid private socket;
if connection to that socket fails, report unreachable through the existing path
without replaying a hand-over request against another endpoint. Filesystem checks
narrow ordinary races; they are not a claim to defeat malicious same-user code.
Missing ownership metadata fails conservatively rather than inventing a uid.
Windows bypasses Unix path/ownership discovery and retains its existing pipe ACL.
Document the discovery amendment in ADR 0014 while preserving endpoint-as-name.

An arbitrary custom XDG directory is **not discoverable** after its environment
value is removed unless it coincides with another approved candidate. The fixed
list does not solve different custom TMPDIRs either. Do not claim universal
environment independence or search the filesystem. The Linux acceptance test
must exercise `/run/user/<uid>`, not give both processes a shared temporary XDG
path that accidentally satisfies the old implementation.

### Acceptance tests and QA assignment

Acceptance tests follow this approved analysis in their own commit and fail for the intended
reason before implementation. Existing passing regressions accompany new red tests.

- **T1 — MCP acceptance matrix:** `crates/apiforge-mcp/tests/acceptance.rs`:
  no writer with stale description; window with valid endpoint; window without
  endpoint or malformed endpoint; foreign command; foreign agent; this server's
  active session; live writer with missing/malformed/unsupported holder metadata.
  Assert the complete D1 fields, no bytes created or changed by overview, and
  no foreign-journal interpretation. Endpoint availability is not a read probe.
- **T2 — wire contract:** `crates/apiforge-cli/tests/mcp.rs`: inspect
  `tools/list` output schema and descriptions, call overview before and during
  an agent session, check structured content equals readable JSON and stdout is
  protocol-only. Assert descriptions distinguish clean/dirty/unreachable windows,
  foreign holders and the separate permission needed for live runs.
- **T3 — refusal and diagnostic matrix:** MCP acceptance tests cover no channel,
  unreachable endpoint, dirty-window refusal and a foreign writer. Assert
  existing codes, `wrote: false`, holder details, preserved bytes and working
  reads. Linux path cases include `/mnt/c/project`, `/mnt/C/project`, ordinary
  paths and nonmatches such as `/mnt/cat/project`; a reachable endpoint must
  not produce the mismatch hint. Test a stale description with no live lock:
  normal writes remain possible. Do not claim this is a real cross-OS lock test.
- **T4 — actual desktop/stdio hand-over:** extend the MCP-08b desktop acceptance
  harness using child-process environments, never global environment mutation.
  Linux window: XDG=`/run/user/<uid>`; server: remove XDG and use an isolated
  different TMPDIR. First `create_document` succeeds, saved source appears in
  the observer, `finish_editing` and EOF return ownership. A separate fresh session
  starts with a permitted live run against the local demo API; permission-off
  sends no request. Exercise server-first/window-later with different environments
  and an agent save reaching the observer. On windows-2022 clear server env except
  PATH/SystemRoot and run the same handshake. Keep dirty-work and foreign-agent
  regressions. Also drive the actual CLI binary over stdio with a controlled
  window-role listener to distinguish CLI launch coverage from desktop behaviour.
- **T5 — storage/channel tests:** add channel acceptance cases for every candidate,
  precedence, duplicates, absent XDG, long/relative XDG, temporary fallback,
  invalid names, symlink/non-socket nodes, nonprivate modes, wrong expected uid,
  missing metadata and a dead first valid socket. Check no candidate is altered.
  Storage tests prove uid comes from file metadata without changing holder JSON
  or its version. Use pure candidate inputs and subprocesses instead of unsafe
  environment setters. Linux CI must provision `/run/user/<runner uid>` if absent
  before the subprocess acceptance case; production must not create it. Failure
  to provision is explicit, not a silently skipped test.
- **T6 — report and documentation:** review README.md, CLAUDE-CODE.md,
  docs/development.md and ADR 0014 against the implemented contract. Run all four
  development-guide verification commands and relevant existing MCP-08b/09 tests.
  Record Linux and Windows results, exact tested build/environment and unverified
  cases in the QA report. Capture the release window's existing observer state
  at 960 × 540 and 1600 × 1000 during native hand-over; harness images alone are
  not native QA evidence. Include the process regression smoke list; mark any
  missing Windows evidence explicitly rather than claiming Windows acceptance.

### Workflow at approval

Read-only GitHub checks repeated on 2026-10-28 found no open pull requests and
no open `awaiting-acceptance` issues: no existing acceptance work consumes the
four-PR / 60-minute budget. The owner approved the analysis, D1–D3 and readiness
for SOL. Issue #123 remains in `sprint:mcp`, with `size:M` and `ready:sol`.
Recheck WIP before opening the implementation PR. This analysis is the first
commit on the ticket's branch from main, followed by the acceptance-test commit,
implementation and QA report in the same PR. There is no separate analysis-only
delivery PR; only the owner merges the completed result.
