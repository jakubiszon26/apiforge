# MCP-12: Window holder discovery and hand-over — QA report

- Branch: `docs/mcp-12-analysis`, based on `main` (`adb0a80`).
- Approved analysis: [MCP-12](../../../tickets/MCP-12.md), D1–D3; M / 40 review minutes.
- Analysis model: GPT-6 Astra. Implementation: GPT-6.1 SOL, reasoning high.
- Implementation: `83515ad`; Windows test compilation follow-ups: `0e4ddf9`, `07fd1e9`.
- Native evidence: release binaries built from `83515ad` with
  `cargo build --release --locked`, Rust 1.99.0. The follow-ups change only
  test compilation guards.
- Compared against MCP-00's approved M1 observer state; no layout change.

## Verification

Linux, 2026-10-28, all exit 0:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`: 1,321 passed,
  0 failed, 35 ignored. Configuration/data were isolated under a scratch
  directory. Ignored MCP-12 helpers are launched by their parent tests.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`
- `cargo build --release --locked`
- After `0e4ddf9`, the focused actual-CLI MCP-12 tests: 2 passed, 0 failed,
  1 spawned helper ignored; formatting passed again.
- After `07fd1e9`, the Linux mount diagnostic test and formatting passed.
- The complete Linux CI job on `83515ad` passed, including release/native
  probes and folder/managed checks:
  [job 123040751784](https://github.com/jakubiszon26/apiforge/actions/runs/40751967995/job/123040751784).

Acceptance tests precede implementation in `01d93bf`. On the old code,
MCP overview and real stdio/desktop tests failed on `another_process` instead
of `window` and obsolete guidance; channel/storage tests exposed the missing
discovery/uid API. The implementation preserves their assertions. One desktop
live-test fixture was corrected afterwards: the illustrative shop scenario
intentionally fails, so the successful-live case now uses a local GET and a
status assertion. The expected successful run and live-consent assertions did
not change.

The suite includes the existing MCP-08b hand-over regressions: dirty-window
refusal, second-agent refusal, take-back, saves/run cleanup in flight, killed
server recovery, notifications and EOF. MCP-09 is not implemented on this
base; the existing `--allow-live` permission is preserved and tested.

## Linux

### Scenario 1: overview describes the holder without taking it

With a live window holder, call overview before any write.

Expected: `holder: window`, role/client/since in `holder_details`, and
`hands_over_on_write: true` for a valid advertised endpoint. Reading does
not connect, advertise a server, write files or interpret a foreign journal.

Observed: the real release CLI returned those fields. The acceptance matrix
also passed for no writer, stale metadata, legacy/invalid endpoints, commands,
foreign agents and missing/malformed/unsupported descriptions. No bytes
changed in its complete before/after snapshot. This server's active session
is `agent_session`; another agent remains `another_process`. The stdio test
checks the output schema, required nullable description, readable JSON and
tool guidance.

### Scenario 2: a first write reaches the window without XDG

1. Start the release window with isolated configuration/data and
   `XDG_RUNTIME_DIR=/run/user/1000`; managed startup prepares the demo shop.
2. Start the actual release `apiforge mcp --folder <shop>` beside it with
   XDG removed and a distinct scratch `TMPDIR`.
3. Initialize a client named `MCP-12 native QA`, read the scenario/hash and
   write a comment marker using that hash.

Expected: hand-over succeeds without closing the window, exact supplied bytes
are on disk, and the window follows the save in the observer state.

Observed: pass on isolated Wayland and X11/Xwayland sessions. The final X11
run's first write, hand-over included, took 0.416 s, returned `wrote: true`,
and left exactly the supplied source on disk. Overview then reported
`agent_session`. Both the window's and MCP's stderr were empty.

The same running release window was resized and captured at
[960 × 540](observing-960x540.png) and
[1600 × 1000](observing-1600x1000.png). These are native window captures,
not harness images. Both show the saved marker, *Observing*, disabled save
buttons, the agent description, the saved notification and *Take back*.
The bar and button retain M1's placement and fit both sizes. The marker's
unattached-comment formatting warning is expected; the source remains valid.

Host: private D-Bus session, KWin virtual output 1920 × 1200, Xwayland,
Vulkan/Lavapipe for the application. The compositor and screenshots use only
that isolated display. Capture waits for a rendered frame containing the
marker and *Take back* after resize. A separate Wayland run also passed the
write/finish/EOF sequence.

### Scenario 3: finish and EOF return the project

After scenario 2, call `finish_editing`; wait for the holder to become the
window. Write again, then close the server's stdin and wait for the window.

Observed: `finished: true`; both returns happened within the asserted
two-second bound. The second write succeeded, the server exited with code 0
after EOF, and the holder file again named the window. Test processes ended
and the private compositor stopped.

### Scenario 4: reverse connection and live permission

The desktop acceptance test starts a server without XDG, opens a window with
XDG later and verifies its channel connection and a subsequent notified save
with normal polling postponed for ten minutes. Pass. Its first-call live
cases against the local demo API also pass: allowed requests hand over and
complete; disallowed requests return `live_disabled` while the window keeps
ownership. These are process/interaction tests, not additional native clicks.

### Scenario 5: privacy, refusal and Windows/WSL diagnostics

Passed automated cases cover candidate order/deduplication/path bounds,
missing uid, wrong expected uid, non-sockets, symlink endpoint nodes,
nonprivate modes, and a dead first safe socket with no fallback connection.
Discovery does not alter candidates. Storage reads uid from the opened
holder file and keeps the original JSON byte shape.

An unreachable/legacy window retains `held_by_window`, `wrote: false`, holder
details and working reads, with no close-window advice. The diagnostic matrix
qualifies a possible Windows/WSL mismatch only for Linux mounted-drive paths;
ordinary paths and `/mnt/cat` do not match. Production uses the store's checked
absolute root, including when `--folder` was relative. A stale description
alone still permits a normal write under a free OS lock.

## Windows

CI is in progress for `07fd1e9`:
[run 40753685913](https://github.com/jakubiszon26/apiforge/actions/runs/40753685913).
Windows formatting and Clippy passed; workspace tests are running as of this
report. Windows acceptance is pending that result. The actual-CLI test clears the MCP
environment except PATH/SystemRoot; the desktop subprocess test adds only its
explicit test-control inputs.

The first run on `83515ad` found an unused Unix-only temporary-directory
variable under Windows `-D warnings`. `0e4ddf9` scopes that declaration to
Unix. The next run exposed an import in a Linux-only diagnostic test module;
`07fd1e9` scopes the entire module to Linux. Neither changes assertions or
product behaviour. No native Windows session was run locally.

## Delivery

The owner will test the branch before a pull request or merge. All product
code is on `docs/mcp-12-analysis`; no merge was performed. After fetching
origin, `origin/main` remains an ancestor of this branch, so it has no
conflict with the current main at this check. CI completion remains visible
on the linked run; it is not claimed as completed here.

## Not verified

- Windows completion and native Windows captures: pending CI; no local Windows host.
- Actual mixed Windows/WSL locks/pipes: unsupported; the qualified diagnostic
  is tested with controlled paths, not presented as cross-OS prevention.
- Arbitrary custom XDG/TMPDIR values missing from the other process cannot be
  discovered outside the approved fixed candidate list.
- Real Claude Code/Codex client reasoning: the CLI is driven over genuine MCP
  stdio by an automated client; no claim about a particular model's choices.
- Keyboard-only interaction, screen reader, and the complete native regression
  smoke list below. Existing automated coverage passed.

## Regression smoke list

Linux release `83515ad`, isolated native sessions, 2026-10-28:

| # | Flow | Observed |
| --- | --- | --- |
| 1 | Managed startup | Pass: a fresh data directory produced/opened the demo shop; application stderr empty. |
| 2 | Folder project | Pass: explorer and scenario Code view visible; folder captures also opened beside the active agent. |
| 3 | Add a block | Not run natively; workspace interaction tests pass. |
| 4 | Wire blocks | Not run natively; workspace tests pass. |
| 5 | Source follows the board | Not run natively; workspace source/board tests pass. The agent's source save is visible in the native captures. |
| 6 | Simulation | Not run through the native Run button; runner/MCP tests pass. |
| 7 | Save/reopen | Agent-save subset passes: exact bytes and reopened folder view; Ctrl+S/layout identity not separately exercised natively. |
| 8 | Dirty close/crash recovery | Not run natively; existing acceptance tests pass. |
| 9 | Legacy migration | Fixture verification and automated migration tests pass; not repeated through a native migration dialog. |
| 10 | Minimum window | Pass for Code observer at 960 × 540, including source, problems, holder bar and Take back; Diagram/Split not separately captured here. |
