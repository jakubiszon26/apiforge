# Stage II P7b: project settings and session secrets

Branch: `feat/stage-ii-p7-settings`. Base: unmerged P7a `375ac2b`
(`feat/stage-ii-p7-projects`), whose Project menu and protected switching are used
by the settings flows.

The Project menu opens project-name/data-contract settings and the test
environment. Project-only settings work without a scenario or hidden placeholder.
Apply changes memory and marks public configuration dirty; Save all persists the
manifest independently of dirty source and works with no source tab. Legacy JSON
settings retain their existing actions. Read-only sessions cannot apply public
configuration, but can enter values for already-declared session secrets.

Configuration edits validate actual immutable published source against the
proposed context before changing memory. This includes contract references by
human-readable alias, which canonical printing alone could hide. Broken published
dependencies leave the prior configuration intact. Explicitly discarding dirty
settings before following an external registry is a separate operation: it
retains the registry guard until the complete saved project is reloaded, including
when the external change has not yet been polled.

Secret values stay in the folder window's session dictionary and runner request.
Changing only those values invalidates analysis/run consent/report inputs without
making the public manifest dirty. They do not survive restart or enter public
settings/cache. New recent-project metadata also passes the session's known-secret
check, preventing an accidental known secret in a display name from being saved
there while preserving the earlier metadata bytes.

Nine new regressions cover project-only dialog validation, empty-project Apply
and explicit Save all, independent configuration/source saving, secret
restart/consent behaviour, read-only secret values, known-secret recent metadata,
immutable dependency validation (including aliases), and explicit discard/reload
before a watcher observes an external manifest change. The header regression also
checks actual painted geometry for long project names beside the scenario
toolbar in writable/read-only windows at both supported review sizes.

Native Linux/Xvfb/Mesa review at 960 × 540 used XTest input:

- [Project settings without a scenario](project-settings-960.png).
- [Applied name with Save all enabled](unsaved-settings-960.png). The probe checked
  that disk still had the earlier name, then pressed Ctrl+Shift+S and verified the
  renamed manifest.
- [Environment and session-only secrets](environment-960.png).

[Long project name at 960 × 540](project-header-960.png) and
[1600 × 1000](project-header-1600.png) show the revised header. The name uses
the shared 13 px regular UI font, aligns with the Project menu, and truncates
before the action toolbar. The full project path remains available on hover.

The review found footer hints compressing action buttons. Hints now have their own
row, and variable/secret addition reserves enough width for the complete action.
The updated native capture shows reachable, untruncated Apply/Cancel controls.

Ordinary configuration validation/saving and recent metadata still run on the UI
thread until P7d. Startup activation, transaction recovery integration and broader
keyboard/accessibility/Windows review remain later packages. Windows directory
durability remains an open release gate.

Verification at code commit `319fa51` passed with Rust 1.99:

- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Serial all-target workspace tests, with isolated `XDG_CONFIG_HOME` and
  `XDG_DATA_HOME`: **858 passed, 0 failed, 2 helper tests ignored** across 45
  test binaries.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`.
- `CARGO_TARGET_DIR=/tmp/apiforge-gate tools/qa/ui-quality/run.sh HEAD`: **10/10**.
- Release layout benchmark with `--strict`: layered layout 0.34/20 ms, full
  routing 26.29/30 ms, dragging 0.22/8 ms, smoothing 0.30/8 ms, and placing eight
  cards 0.07/8 ms. Random routing took 1003 ms and has no defined budget.

No language implementation changed in P7b; language fuzzers were not required.
