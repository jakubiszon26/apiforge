# REL-01: A GitHub release with an AppImage, a Windows ZIP and Linux tar.gz — QA report

- Branch and commit: `feat/rel-01-release` at `34c4fe1` (review round 1).
  The evidence was taken on `c608b9c`; the AppImage scenario was repeated
  on `34c4fe1`.
- Built with: `cargo build --release --locked -p apiforge-desktop -p
  apiforge-cli` on Ubuntu 24.04 (glibc 2.39), Rust 1.99.0. The release
  workflow builds on `ubuntu-22.04` instead; that build belongs to the dry
  run.
- Ticket: [docs/tickets/REL-01.md](../../../tickets/REL-01.md), decisions
  1–4 taken 2026-10-28. Analysis: Fable. Implementation: Opus 5.5.
- Owner decisions folded in on 2026-10-28: the owner's own icon in place of
  the placeholder, the README rewrite, and the licence GPL-3.0-or-later with
  the DCO.

## Acceptance criteria

| # | Criterion | How it is verified | Result |
| --- | --- | --- | --- |
| 1 | One pre-release, four assets, generated notes ending with the sums | The dry run | **Pending** |
| 2 | Asset names carry version and target, asserted before upload | `release.py check-assets` in publish; `test_release.py` (`Names`) | Tests pass; asset list **pending** |
| 3 | Archives hold exactly the manifest's entries | `release.py check-archive` in both build jobs; `test_release.py`; Scenario 1 | Pass on Linux; Windows in the dry run |
| 4 | `apiforge mcp` from the extracted archive: `initialize` 2025-11-25, ten tools, exit 0 at EOF | `packaging/mcp-smoke.py` in both build jobs | Pass locally from the extracted tar.gz; both runners in the dry run |
| 5 | `--version` prints `<name> <version>`; a mismatching tag stops the run before building | CLI acceptance test, desktop argument test, `tests/version.rs`; `release.py version` (`test_release.py`) | Pass |
| 6 | AppImage starts on a clean system; `desktop-file-validate` passes | Job `smoke-appimage` | Local equivalent passes (Scenarios 2–3); job **pending** |
| 7 | Tag-only trigger; `contents: read`, write only in publish; tag must be on main | Blocks quoted below; actionlint clean | Pass by inspection; the refusal is not exercised |
| 8 | A failed step publishes nothing; tests run before packaging | Job graph below | Pass by inspection |
| 9 | Dry run `v0.1.0-rc.1` end to end | Coordinating session | **Pending** |
| 10 | Verification passes; `rust.yml` unchanged; Releases section in the guide; README points at Releases | Below | Pass |

The acceptance commit failed for the right reasons before the
implementation:

- the CLI test with `Usage: apiforge fmt <file.afs> [--check]`;
- the desktop test with `Err("Unknown argument; use --help")`;
- `test_release.py` because `release.py` didn't exist yet.

`mcp-smoke.py` already passed against `main`'s binary, as expected: it
guards the packaged binary, not a new feature.

## The workflow (criteria 7 and 8)

```yaml
on:
  push:
    tags: ['v*']

permissions:
  contents: read
```

Only `publish` declares `permissions: contents: write`.

```text
version ──┬── build-linux ──── smoke-appimage ──┐
          └── build-windows ────────────────────┴── publish
```

- `publish` needs all four other jobs.
- `publish` is the only job that calls the Releases API (`gh api
  …/releases/generate-notes`, `gh release create --prerelease
  --verify-tag`).
- Both build jobs run `cargo test --workspace --all-targets --locked` before
  building.
- Two downloads are pinned by release URL and SHA-256 and checked before
  use: appimagetool 1.9.1 (`ed4ce84f…384eb0`) and the type2 runtime 20251108
  (`2fca8b44…ec260d`).

## Linux, on this machine

### Scenario 1: the tar.gz

Steps:

1. Stage and pack as the workflow does.
2. Compare with the manifest.
3. Extract, then run both `--version`s and `mcp-smoke.py` from the extracted
   copy.
4. Add a stray file, pack again and compare again.

Observed:

- The packed archive matches `manifest-linux.txt`: eight files below one
  top-level directory, executables at 0755.
- `apiforge 0.1.0: initialize 2025-11-25 and 10 tools, exit 0 at EOF`
- The stray file fails the comparison with `…/extra.txt is not in
  manifest-linux.txt` and exit 1.
- Sizes: `apiforge` 11.3 MB, `apiforge-desktop` 36.6 MB, tar.gz 18.6 MB.

### Scenario 2: the AppImage

Steps:

1. Build the AppDir as the workflow does: patchelf sets RUNPATH
   `$ORIGIN/../lib` on the binary and `$ORIGIN` on each library, with
   LICENSE and the font licences included.
2. Pack it with the pinned appimagetool and runtime.
3. Run `desktop-file-validate`, `--version`, `--check
   fixtures/workspace.json` and `appimage-window.sh` (an Xvfb and Lavapipe
   `--screenshot`). Without /dev/fuse here, the AppImage runs with
   `APPIMAGE_EXTRACT_AND_RUN=1`.

Observed:

- 14.3 MB.
- `desktop-file-validate` prints nothing.
- `--version` prints `apiforge-desktop 0.1.0`.
- `--check` prints `Valid v2 document: 4 project(s)`.
- [capture](appimage-window-linux.png) at 1600 × 1000.
- An `LD_DEBUG=files` trace shows all six bundled libraries loaded from the
  AppImage's `usr/lib` through RUNPATH, with no `LD_LIBRARY_PATH` set.
- appimagetool's only warning: gpg is missing, so the AppImage is unsigned,
  as decided.

### Scenario 3: the libraries the AppImage carries

Steps:

1. Trace the window's `--screenshot` with `LD_DEBUG=files`.
2. Map each library to its package and compare with the dependency closure
   of Ubuntu 24.04 plus `libfuse2t64`, `xvfb`, `mesa-vulkan-drivers` and
   `libvulkan1`.

Observed:

- Six libraries fall outside that closure: `libxkbcommon.so.0`,
  `libxkbcommon-x11.so.0`, `libxcb-xkb.so.1`, `libXcursor.so.1`,
  `libXfixes.so.3`, `libXi.so.6`.
- The ticket expected two; all six are now in
  `packaging/appimage-libraries.txt`.
- The closure is computed, not run: there is no container runtime here, so
  the bare-container run is the CI job's.

### Scenario 4: the window icon

Steps:

1. Render the owner's SVG with resvg 0.48.1 and the bundled Adwaita Mono as
   the only font.
2. Capture the window under Openbox on Xvfb before and after the change,
   title bar included.

Observed:

- [Before](titlebar-before-x3.png): Openbox's generic icon.
  [After](titlebar-after-x3.png): the dog icon (both enlarged 3×). Also
  [window top](window-top-after.png) and [icon sizes](icon-sizes.png).
- 256 px: every detail renders, braces included.
- 32 px: the head, eye patch, ears, collar and tag are recognisable; the
  eyes become dots and the braces and grid vanish.
- 16 px: a light head on a dark tile with orange and gold hints; the face is
  not recognisable.

### Scenario 5: `--version` and `--help`

- `apiforge --version` prints `apiforge 0.1.0`; `apiforge-desktop
  --version` prints `apiforge-desktop 0.1.0`.
- Both exit 0 and open no window.
- With a further argument, both exit 2.
- Each `--help` lists `--version`.

## Windows

Not run by the agent. In the dry run, `build-windows` tests, builds and
packs the ZIP, then smokes the extracted binaries: both `--version`s,
`--check` and the MCP smoke. The owner's start of `apiforge-desktop.exe`
from the ZIP gets recorded there, or stated as not run. The Windows window
icon is unchanged.

## Dry run

**Pending.** After acceptance, the coordinating session pushes
`v0.1.0-rc.1` and records here:

- the run URL and each job's result;
- `gh release view v0.1.0-rc.1 --json assets,body,isPrerelease`;
- each asset's size and SHA-256;
- the `appimage-window` captures from the runner and the container;
- the `version` job's check line;
- the AppImage run on the agent's Linux machine;
- the ZIP's CI smoke.

Then one quiz: delete the rc release and tag, and tag `v0.1.0`.

## Regression smoke list

Linux, 2026-10-28, release binaries of this branch, Xvfb with Lavapipe.

| # | Flow | Result |
| --- | --- | --- |
| 1 | Start with no arguments | Pass: an empty data directory gets the demo shop project; stderr has only the X server's keymap warnings |
| 2 | Open a folder project | Pass: `--folder` captures of the migrated shop in Code, Diagram and Split |
| 3–8 | Library, wiring, editing, simulation, save, dirty close and kill | Not run: the change touches only argument parsing and the window icon; the workspace tests cover these flows |
| 9 | Migrate a legacy workspace | Pass: four projects, `apiforge check --folder` passes for each, the original is byte-identical |
| 10 | Window at 960 × 540 | Pass: captures at 960 × 540 and 1600 × 1000 in all three modes have exactly those sizes |

Windows: not run.

## Verification

- `cargo fmt --all -- --check`: pass
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: pass
- `cargo test --workspace --all-targets --locked`: pass (1272 passed, 0
  failed, 31 ignored)
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: pass
- `python3 -I packaging/test_release.py`: pass (12 tests)
- actionlint 1.7.12: no findings
- PR #118 CI: green on `34c4fe1`

## Not verified

- Everything only a pushed tag runs: the workflow as a whole, the 22.04
  build, the Windows ZIP, the bare-container AppImage start, note generation
  and publishing (criteria 1, 2, 6 and 9).
- The refusal of a tag that is not on main (criterion 7).
- The AppImage mounted through FUSE: there is no /dev/fuse on this machine;
  the CI runner step covers it.
- The icon on Wayland, GNOME and KDE, and through AppImage integration
  tools.
- The window icon on Windows: not set.
- Regression smoke items 3–8 and every Windows item.
