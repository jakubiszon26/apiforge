# Stage II P7g acceptance — startup activation and managed projects

- Date: 2026-10-25
- Branch: `feat/stage-ii-p7-activation`
- Base: unmerged P7 integration `09387b0`
- Scope: managed startup, automatic migration through `migrate_workspace`, and
  creating a managed folder project. No change to the default start
- Result: implemented and verified on Linux, and exercised on both platforms by
  continuous integration; Windows QA and Windows directory durability remain
  release gates, and the default start still opens the legacy workspace

## Implemented contracts

| Area | Evidence |
| --- | --- |
| Resolving the data directory | A catalogue opens a migrated project; a legacy workspace is migrated; a directory with neither yields one created project. Each path is an interaction test and a native run |
| Migration integration | `migrate_workspace` is called from the application for the first time. Originals byte-identical, catalogue activated with its migration ID and input hashes, previously active project opened |
| Restart | A completed migration is read, never repeated: same migration ID, same projects, no re-preparation. A created project is reopened rather than duplicated |
| Resuming and failure | An occupied destination stops before activation, preserves the foreign bytes, writes no catalogue and names the project; clearing it lets the same migration finish. No rollback, because the originals were never changed |
| Which window opens | A workspace another window owns, or one awaiting a backup decision, opens the workspace window instead; both are decided before a window exists and neither migrates anything |
| Creating a project | `projects/<project-id>/`, the same layout migration produces, with manifest, `.gitignore` and `.gitattributes`, holding the demo so a first run opens a document |
| Remembering projects | Every migrated project is offered, not only the one that opened, by reading each manifest without taking its lock. The catalogue is never rewritten after activation |
| Reporting | Creating and migrating report what happened and where; reopening a managed project reports nothing |
| Git rules | Preparation writes both rules, so a migrated project excludes `.apiforge/local/` — its lock, recovery and run journal — from the repository |

Managed startup does no heavy work before the window exists. The pre-window
decision reads at most the workspace and its drafts; migration, creation and
opening run on the storage worker, so the window appears first and reports
afterwards.

The catalogue keeps its meaning as evidence that one migration completed,
carrying that migration's identifier and the hashes of the files it read. A
project created later was not part of any migration, so it is remembered the way
an opened folder is remembered. This needed no format change and no revision of
ADRs 0010–0012.

## Verification

All final commands passed:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p apiforge-desktop --all-targets --locked --features inspection -- -D warnings
cargo test --workspace --all-targets --locked
cargo run -p apiforge-convert-v1 --locked -- --fixtures --check
cargo test -p apiforge-desktop --release --locked frame_budgets -- --ignored
tools/qa/ui-quality/run.sh HEAD
```

949 workspace tests, eighteen ignored (two helpers, fourteen review views, the
inspection host, the release frame test). UI gate 10/10. Release frame budgets
held at 1000 lines and 1 MiB. Native evidence is in
[the P7g verification](qa/2026-10-25/p7g-activation/verification.md).

## Continuous integration

`.github/workflows/rust.yml` exercises folder projects and managed startup on
`ubuntu-24.04` and `windows-2022` on every push, against the release binary and
a real filesystem. Both runners assert that:

- an application data directory holding nothing yields exactly one project under
  `projects/`, with its manifest, `.gitignore` and `.gitattributes`, and that
  neither `workspace.json` nor `projects.json` was written;
- a legacy `workspace.json` is migrated into `projects.json` and the four
  projects of `fixtures/workspace.json`, with the original workspace
  byte-identical by SHA-256 before and after;
- a second `--managed` run keeps the same migration ID and adds no project
  directory;
- `--screenshot --folder` renders a migrated project that no run had opened, to
  a non-empty PNG.

`--managed` opens a window and never exits by itself, so each run is stopped
once what it owes is on disk; a run that writes nothing fails on its own
deadline. Linux gives each phase its own `XDG_DATA_HOME`. Windows cannot:
`dirs::data_dir()` resolves `FOLDERID_RoamingAppData` through
`SHGetKnownFolderPath`, which ignores `%APPDATA%`, so the job uses the roaming
directory of the runner account and clears it between phases, refusing to start
if it already holds a workspace or projects.

## Open gates

- **Windows.** The directory writes above now run per push on `windows-2022`,
  which is evidence that migration and project creation produce the expected
  layout there and that the original workspace is left untouched. That is not
  the Windows gate. Interactive native QA — keyboard use, file dialogs, restart
  and recovery, read-only locks, both window sizes — has still never been run on
  Windows. Nor does a CI job observe directory durability: the runner does not
  lose power, so a missing directory sync would pass every run. Both remain
  release gates before the default start can change.
- **The default start.** `apiforge-desktop` with no arguments opens the legacy
  workspace, unchanged. Managed startup is reachable through `--managed` only.
  Flipping it is a change to the `Arguments::Window(None)` arm in
  `crates/apiforge-desktop/src/main.rs`, and belongs to the release that closes
  the Windows gates, not to this package.
- No screen-reader session has been run against the managed start.
