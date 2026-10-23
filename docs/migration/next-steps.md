# Next steps: remaining parity, polish and release readiness

Audit date: 20 October 2026. Scope: the **current working tree** of `main` at `627aa84`
including all uncommitted changes (20 modified files and the untracked `navigator.rs`,
`live_workflow.rs`, `service_startup.rs`, ADR 0003, `integration-review.md`, `native-ci.md`
and `tools/`). Nothing in this document claims production readiness. The machine-readable
campaign is [claude-backlog.json](claude-backlog.json).

## 1. Evidence collected for this audit

| Check | Result | Notes |
| --- | --- | --- |
| `cargo test --workspace --all-targets --locked` (Linux, this tree) | **218 passed, 0 failed** | core 92 + import 10 + semantics 3; demo-api 6 + 1 + 10; desktop 55; runner 22 + live 3 + corpus 1 + startup 2; storage 13 |
| Coordinator baseline: fmt, clippy `-D warnings`, 218 tests, fixture `--check`, build | **all PASSED** | `.agent-environment/campaigns/migration-20261020/baseline` |
| Hosted CI (Linux and Windows jobs in `.github/workflows/rust.yml`) | **no run evidence** | configuration only, see [native-ci.md](native-ci.md):43-46; a GitHub lookup for HEAD PR runs returned none (not proof that CI never ran); agents never push |
| Windows execution of any kind | **none** | external gate EG-01/EG-02 |
| GUI acceptance of the current tree by Gemini QA | **FAIL (valid)** | `baseline-gui-report.json` from `gemini-4-argon-high`, 24 retained screenshots. All functional flows worked at client 1100×680, 1280×800 and 1920×1080. Two real visual defects remain at supported sizes, see §9. 800×600 is UNSUPPORTED (forced client 800×564) |

Sources read: `RUST_MIGRATION.md`, `README.md`, `AGENT_WORKFLOW.md`, every file in
`docs/migration/`, ADR 0001–0003, the React reference (`app/features/**`, `shared/`, `api/`,
`tests/browser/app.spec.ts`) and the Rust crates under `crates/`.

## 2. Parity matrix (React prototype → native)

Status legend: **done** = behaviour present and covered by native tests; **gap** = genuine
remaining difference; **n/a** = reference behaviour that does not apply to an in-process
runner.

| Area | Status | Native evidence | Remaining gap → increment |
| --- | --- | --- | --- |
| Projects, scenarios, new-item dialogs | done | `app.rs`:462-505, 1431-1507; `library.rs`:83-140 | — |
| Board: select, box-select, drag, wires, zoom/fit, menus, empty state | done | `canvas/mod.rs`:269-401, 637-761, 958-1021 | **baseline GUI defect:** floating overlays cover cards and the mode line when drawers are open (`canvas/mod.rs`:382-389, `paint.rs`:380-455) → **M01 Part B1**; minimap is paint-only (`canvas/mod.rs`:385) while React's MiniMap pans/zooms; Delete only when the pointer is over the board (`canvas/mod.rs`:844-847) → backlog |
| Control flow (Start, Next, IF Yes/No, keyboard connection form) | done | `inspector/mod.rs`:514-593; `edit::connect` | — |
| Nested operators, operator drops on arguments | done | `inspector/binding.rs`:128-150, 394-495 | — |
| One Comparison with type-as-argument | done | `inspector/mod.rs`:797-841; `fields.rs`:73-96 | — |
| Variables palette, drops into URL/query/headers/select path | done | `library.rs`:314-399; `fields.rs`:156-266; `inspector/mod.rs`:595-748 | condition-predicate path does not accept drops (`inspector/mod.rs`:903-913 vs React `NodeInspector.tsx`:516-540) → **M03** |
| Response field picker (argument references) | done | `binding.rs`:519-765 (searchable modal) | field list is collected unbounded every frame and rendered without virtualisation (`binding.rs`:660, 691-733; `core/edit/inspect.rs`:81) → **M04** |
| Variable "take value from a response" | gap | `inspector/mod.rs`:378-487 renders an unsearchable flat list | React reuses the searchable `ResponseFieldDialog` (`ResponseBindingPicker.tsx`:50-107) → **M03** |
| Response → variable assignment, field → Comparison | done | `app.rs`:836-898, 990-1013, 1363-1396 | — |
| Components: extract, revisions, pinned instances, update instance | done | `dialogs/component.rs`; `app.rs`:1260-1276, 1397-1427; `inspector/mod.rs`:206-249 | — |
| Copy/paste/duplicate with frozen snapshot, text shortcuts untouched | done | `app.rs`:1111-1205, 1280-1326; tests `app.rs`:1604-1682 | — |
| Undo/redo with merging, per-context history | done | `editor.rs`:281-380, test at 561 | — |
| Runs: full / up to block / single step, cancel, staleness | done | `app.rs`:243-306; `runner/service.rs` | the React runner also served `/demo` on 127.0.0.1:4318 (`api/server.ts`, `api/demo.ts`); natively the user must start a second program → **M01** |
| Report: steps, components, cleanup, expected/actual, HTTP tab, export | done | `report.rs`:276-500, 737-794 | children beyond 100 and depth ≥ 10 are hidden silently (`report.rs`:660-701) → **M04** |
| Report size: drag, collapse keeps height | done | `report.rs`:145-186, test at 1051 | **no keyboard control** (React `ResizableRunPanel.tsx`:61-73: ↑/↓ ±32, Home, End) → **M02** |
| Inspector output tree | partial | `fields.rs`:276-405 | expands every level (100 children × depth 10) every frame; large 2 MiB responses can stall → **M04** |
| Secrets: session-only, never exported, redacted | done | `apiforge-storage`:183; `runner/redact.rs`; `dialogs/environment.rs` | — |
| Durable storage, backup, failed-save close path | done | `apiforge-storage/src/lib.rs`:98-180; `app.rs`:150-204 | no single-writer lock or external-change detection (`production-audit.md` G1.5) → **M05**; no backup-restore UI (G1.4) → backlog |
| HTML / non-JSON reply of the runner HTTP API (browser test 4) | n/a | the runner is in-process (`app.rs`:104-110) | — |
| Window size and scaling | gap | `main.rs`:242 minimum 1100×680 logical | G6 measures 1920×1080 at 200 % = 960×540 logical, which cannot fit (`production-audit.md`:207-210) → **M06** |

Functional parity is high: no React user flow from the 14 browser scenarios is missing
outright. The remaining parity gaps are the in-app demo API, two inspector details, the
keyboard alternative for report resizing and large-response behaviour.

## 3. Risks found outside the parity matrix

1. **Stuck "running" state after a runner panic.** `RunnerService::start` sets `finished`
   only after `run_validated` returns (`runner/src/service.rs`:90-103). A panic in the task
   drops the guard but never sets `finished`, so `App::is_running` (`app.rs`:206-212) stays
   true forever: Run, project switching and closing are blocked. Backlog B1.
2. **No interrupted-run journal.** Contract item 7 and G4.7 require marking a crashed live
   run as interrupted with unknown side effects. Nothing records it. Backlog B2. A GUI agent
   cannot kill the process, so acceptance needs an automated process-kill test.
3. **Silent data loss with two windows.** Both instances autosave the same
   `workspace.json` and the last writer wins. Addressed by **M05**.
4. **Transport error categories.** `HttpError::Transport(String)` (`runner/src/http.rs`:84-85)
   does not distinguish DNS, connect, TLS and timeout as G3.5 requires. Backlog B4.
5. **TLS/proxy profile undecided.** `no_proxy()` is hard-coded (`http.rs`:34) and there is no
   extra-CA option (G3.2/G3.3). This is a product decision (EG-07), then backlog B5.
6. **Accessibility of the custom canvas.** Cards and ports have no AccessKit nodes. The
   navigator (Ctrl+K) and the connection form are the only semantic alternatives. Backlog B7.
   Screen-reader acceptance is an external gate.
7. **egui 0.36.2 pinned** (`RUST_MIGRATION.md`:228-231). Upgrading to 0.37 is a separate,
   deliberate step after this campaign (backlog B10).

## 4. GUI QA harness constraints that every scenario must respect

Observed in `tools/agent-env/lab.py` and `tools/agent-env/staging.py` (read only, not
changed by this plan):

- **Two staged binaries behind one launcher, no arguments.** The coordinator extended
  `staging.py` (:65-70) to stage `apiforge-desktop` and `apiforge-demo-api` behind the same
  allowed `/mnt/app` wrapper. QA still launches exactly `/mnt/app` without arguments, so
  features needed by QA must be reachable through the UI.
- **Demo API lifecycle.** Before M01 the wrapper starts the private demo API itself
  (`staging.py`:28-38). Once `crates/apiforge-desktop/src/demo_server.rs` exists
  (`staging.py`:58) it leaves the lifecycle to the in-app control, initially stopped. It never
  kills unrelated host demo API servers.
- **Live mode needs loopback port 4318 free on the host.** The sandbox shares the host
  network ("not a network sandbox"). The coordinator must make sure `npm run dev` and any
  host `apiforge-demo-api` are not holding the port before a live-mode scenario; nothing
  in the harness or app stops them.
- **No file dialogs.** The filtered D-Bus proxy exposes only Secret Service, so the XDG file
  portal used by `rfd` is unavailable. This is an external/harness gap (EG-09); scenarios
  avoid Import/Export and those flows stay covered by automated tests.
- **Fresh isolated HOME per QA session.** The first launch always shows the default
  workspace (*Demo shop* › *Promotion and cart price*, plus *Ticket system*). Relaunching
  `/mnt/app` within one session keeps that HOME, so restart persistence can be checked.
- **Window sizes.** The supported minimum is 1100×680 logical pixels (`main.rs`:242); after
  M06 it will be 960×540, derived from the G6 requirement of 200 % scaling at 1920×1080.
  800×600 is not a project requirement and no document may claim that it passed. The
  coordinator's QA contract now accepts UNSUPPORTED only for a size below the declared source
  minimum, together with the actual KWin size readback and a PASS at the observed supported
  minimum. The minimum is a **client** size. KWin frames add decorations: the baseline saw a
  36 px title bar, so frame 1100×716 gave client 1100×680. KWin scripting can force a frame
  below the minimum (requested 800×600 read back as client 800×564), so no scenario may
  assume a clamp. Every scenario requests 800×600, records the readback and the UNSUPPORTED
  result, and then needs an actual PASS at the minimum client readback plus 1280×800 and
  1920×1080 (M06 also at 960×540). The controller (`lab.parse_report`) rejects any PASS
  below the minimum or without a supported client readback.
- **API JSON keys are data, not UI.** Demo-data field names such as `inna_nazwa` are kept for
  prototype compatibility. They are not untranslated application text, and no stage renames
  them.
- **egui exposes little accessibility data to KWin MCP.** QA works from pixels; scenarios name
  visible labels exactly as the native UI renders them.

## 5. First campaign: six locally actionable increments

Each increment fits one coding-agent turn, has targeted automated tests and a real GUI
scenario. Claude reviews every implementation before independent Gemini QA. Order follows
dependencies and file ownership; no increment needs Windows.

| ID | Developer | Depends on | Title | Main files |
| --- | --- | --- | --- | --- |
| M01 | claude | — | In-app local demo API (start/stop, safe close), plus Part B fixes for the two baseline GUI defects | desktop `Cargo.toml`, `Cargo.lock`, new `demo_server.rs`, `app.rs`, `main.rs`, `library.rs`; Part B: `canvas/mod.rs`, `canvas/paint.rs`, `canvas/layout.rs`, `dialogs/mod.rs`, `navigator.rs` |
| M02 | codex | — | Keyboard-operable report resize handle | `report.rs` |
| M03 | codex | — | Searchable variable response picker; drops into condition paths | `inspector/mod.rs`, `inspector/binding.rs`, `inspector/response.rs` |
| M04 | claude | M01, M02, M03 | Bounded rendering of large JSON (picker, inspector, report) and a demo catalogue endpoint | `core/edit/inspect.rs`, new `json_view.rs`, `fields.rs`, `binding.rs`, `report.rs`, demo-api |
| M05 | claude | M01 | Single-writer workspace lock and external-change conflict handling | `apiforge-storage/src/lib.rs`, `persistence.rs`, `app.rs`, new ADR 0004 |
| M06 | codex | M02, M04, M05 | Compact layout down to 960×540 logical pixels | `main.rs`, `app.rs`, `library.rs`, `canvas/*`, `dialogs/*`, `navigator.rs`, `report.rs` |

Rationale:

- **M01** first: it closes the most visible parity gap (the React prototype served `/demo`
  itself) and moves demo API lifecycle from the harness wrapper into the app. After the
  baseline GUI FAIL it also carries the two layout fixes in §9. Otherwise every
  GUI-gated stage would fail on them.
- **M02** and **M03** are independent, small parity fixes that run in parallel with M01.
  M03's QA uses Simulation mode and needs no staging.
- **M04** depends on M02/M03 for file ownership (`report.rs`, `binding.rs`) and on M01 because
  its QA needs a large live response from the demo API.
- **M05** closes a data-loss risk (G1.5) that is locally testable on Linux. Windows locking
  semantics are verified later under EG-01.
- **M06** is visual polish and release preparation. It goes last because it touches the
  layout of every panel changed before it.

Acceptance for each increment (details in the JSON):

1. Developer: implementation plus targeted tests; `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets --locked -- -D warnings` and
   `cargo test --workspace --all-targets --locked` pass on Linux.
2. Claude review of the diff against the stage's owned files and acceptance list.
3. Gemini QA via KWin MCP with the stage's `qa_scenario`, at the sizes above, after the
   staging prerequisites in the scenario are met.
4. Documentation updates listed in the stage (behaviour changes need a specification
   update, as `RUST_MIGRATION.md`:44-46 requires).

## 6. External gates (not locally verifiable)

These must not block the Linux increments above, and no document may claim them before
evidence exists.

| Gate | What is missing | Evidence required |
| --- | --- | --- |
| EG-01 Hosted CI | first run of `rust.yml` on ubuntu-24.04 and windows-2022 | run URL with green jobs; uploaded `native-window-*` captures inspected |
| EG-02 Native Windows | real Windows 10/11 x86_64 session | protocol for start, DPI 100/200 %, IME, clipboard, file dialogs, file lock (M05) and close-during-run |
| EG-03 Signing | publisher code-signing certificate, artefact signing key and rotation procedure | signed binaries and installer; signature check in the smoke test |
| EG-04 Installers | Windows installer, Linux package, clean-machine trials | install, start, upgrade and uninstall keeping data on clean VMs; checksums and manifest |
| EG-05 Linux matrix | clean Ubuntu 22.04 (glibc 2.35) Wayland and X11 sessions | smoke and functional protocol on both backends |
| EG-06 Accessibility | screen readers on both platforms | Orca and NVDA/Narrator protocols covering the keyboard-only path |
| EG-07 Product decisions | supported OS versions, TLS extra-CA and proxy profile, history retention | recorded decisions (ADRs) approved by the user |
| EG-08 Reference hardware | agreed machines for G6 measurements | raw frame-time and memory measurements with build and hardware identifiers |
| EG-09 QA file portal | XDG file portal in the QA sandbox | Gemini QA PASS of Import and Export scenarios |

## 7. Later backlog (locally actionable after this campaign)

B1 runner panic → final internal-error report; B2 interrupted live-run journal; B3
backup-restore UI; B4 transport error categories; B5 TLS/proxy after EG-07; B6 Linux
packaging with checksums, manifest and licence list (`cargo-deny`/`cargo-about` in CI); B7
canvas accessibility and keyboard focus traversal of cards, Delete key routing, interactive
minimap; B8 fuzz/property tests for import, paths, ID remapping and redaction; B9 headless
performance profile for 200/1000 and 500/1000 graphs; B10 deliberate egui 0.37 upgrade; B11
harness extension: launch arguments and a file portal for Import/Export scenarios (two
staged binaries and the size policy are already done by the coordinator).

## 8. Campaign process (user policy, 20 October 2026)

- Claude and Codex are the developer providers. When one exhausts a temporary quota, the
  other takes over planning, architecture, implementation and review with the original
  instructions and the partial edits.
- Review is a fresh, read-only agent turn on a separate checkout; the author turn never
  approves itself. With only one developer provider available, a fresh separate reviewing
  turn of that provider is allowed and is recorded as such.
- Gemini `gemini-4-argon-high` does only GUI QA, never development. Without Gemini capacity
  QA waits; no other agent can grant a GUI PASS. The JSON lists each item with its
evidence.

## 9. Plan amendment after the real baseline GUI FAIL (20 October 2026)

**Why the plan changed.** The first real Gemini GUI QA of the current tree
(`baseline-gui-report.json`, `gemini-4-argon-high`, 24 retained screenshots, KWin readbacks)
is a valid **FAIL**, not an infrastructure error. Every main functional flow worked at the
minimum client size 1100×680 (frame 1100×716) and at 1280×800 and 1920×1080. The flows
covered were demo workspaces, Find step, keyboard rename, Undo/Redo, Settings JSON
validation, a Simulation run with a nested component, a live API run against
127.0.0.1:4318 and scrolling. 800×600 was correctly recorded UNSUPPORTED (forced client
800×564). Two real visual defects remain at **supported** sizes:

1. **Canvas overlays cover cards and clip the mode line** (medium). This was seen at
   1100×716 and 1280×800 with the Inspector and/or Run results open. The toolbar
   ('+ Add step | Selection (1)') overlaps the 'Legacy mode: …' line, and the minimap and
   the shortcut hint are drawn over the 'Product Item' and 'Expected price' cards. Cause:
   the overlays sit at constant offsets from the raw viewport (`canvas/mod.rs`:382-389,
   863-995, `paint.rs`:380-455), and the camera's fit and reveal ignore them and do not react
   when drawers shrink the board.
2. **Modals are off-centre after a resize** (low). At client width 1280, after a resize from
   1100, 'Project and scenario' opened at x=214..854 instead of the centred 320..960. At 1100
   both Settings and 'Find a step' measured centred within 2 px, although QA perceived them
   as flush with the sidebar. At 1920 they were centred. The likely cause is the egui 0.36.2
   `Modal` anchoring with stale area state or a missing repaint; M01 must confirm it with a
   failing headless test first
   (`dialogs/mod.rs`:146, `navigator.rs`:36, `app.rs`:1500).

A GUI PASS cannot carry unresolved defects, and the controller rejects one that does
(`lab.parse_report`). Unfixed, these baseline defects would therefore block **every**
strict GUI-gated stage (M01–M06), whatever that stage implements. M01 is complete:
independent Linux fmt, clippy, tests, fixture check and build passed, and its binaries are
pinned. It has not had its code review or its own GUI regression yet, so the fixes are
added to **M01 before its acceptance**, not to a new stage or to M06.

**What changed in [claude-backlog.json](claude-backlog.json).** Only stage M01:

- New owned files: `canvas/mod.rs`, `canvas/paint.rs`, `canvas/layout.rs`, `dialogs/mod.rs`
  and `navigator.rs`. `app.rs` was already owned.
- Implementation Part B1: a pure `board_overlays` layout in `canvas/layout.rs`. It defines
  a header band holding the wrapped mode line and the toolbar, and a footer band holding
  horizontal zoom controls and the inline hint, which becomes a 'Shortcuts' tooltip when it
  does not fit. The minimap only appears above a size threshold. Cards are clipped to the
  board, and fit, reveal and resize recentring target a focus rect.
- Implementation Part B2: one shared `centered_modal` helper used by `Chrome::show`
  (Settings, Environment, Component, Assign variable), the navigator and the new-item modal.
  It positions the modal from the current `content_rect` every frame and repaints after a
  resize or a sizing pass. It comes with a reproduction test that must fail before the fix.
- Headless tests with no GPU and no timing. For layout, five board sizes check that
  overlays are disjoint, the text wraps and the hint and minimap fallbacks work. For the
  app, clients 1100×680, 1280×764 and 1920×1044 with the Inspector and Run results open
  check that no card is under an overlay. For modals, five sizes check ±2 px centring at
  open, after a resize while open, and on reopen after a resize.
- Acceptance and QA: Part A demo API criteria and steps 1–7 are unchanged, apart from the
  clarification that Live is the run-mode option labelled 'API'. The size protocol now
  requires an actual PASS at the minimum client readback plus 1280×800 and 1920×1080, with
  800×600 UNSUPPORTED and its forced readback, and no clamp assumed. Regression steps 8–11
  cover both defects at all three supported sizes, with a ±4 px equal-margin check for the
  dialogs.
- Unchanged: the 1100×680 minimum (the 960×540 compact layout is still M06), `report.rs`,
  `inspector/*`, `theme.rs`, demo data and API JSON keys such as `inna_nazwa`, all other
  stages, their order and dependencies, the external gates and the later backlog.

**Routing.** No product source changed in this architectural turn. The coordinator routes
the amended M01 as a repair turn to the developer, then reruns the build gates, the
independent read-only review and Gemini GUI QA. The campaign controller requires the plan
on resume to match the recorded plan, so the coordinator must re-register the amended plan
deliberately. Nothing here waives a visual defect or claims a GUI PASS.

**Notes for later stages (not part of their criteria).** The inspector modals
(`inspector/binding.rs`, `inspector/mod.rs`, owned by M03) and M06's dialog work should reuse
`dialogs::centered_modal`. M06's scenario text "expected actual >= 960x540 (clamped)" predates
the observation that KWin scripting can force sizes below the minimum. The controller's
readback rules (UNSUPPORTED below the minimum, PASS only with a supported client readback)
take precedence over that wording.
