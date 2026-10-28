# REL-01: A GitHub release with an AppImage, a Windows ZIP and Linux tar.gz

- Size: M · Owner review: 30 min · Risk: `visible UI` (the application icon only)
- State: Implementation · decisions 1–4 taken 2026-10-28 (all the
  recommended option) · before MCP-11 (#95) · charged to the UX capacity line
  (30 min) · the dry run follows the pull request
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

A person downloads API Forge from the repository's Releases page and runs it
without a Rust toolchain: on Linux an AppImage for the window and a tar.gz
with both binaries, on Windows a ZIP with both; the `apiforge` binary of the
archive registers with Claude Code as the MCP server. The first release is
v0.1.0, a pre-release.

## Decisions taken (2026-10-28)

1. **Linux desktop package: AppImage**, not Flatpak. Flatpak's sandbox would
   break `apiforge mcp` spawned by Claude Code and the control-channel
   socket (MCP-08b).
2. **Windows: one ZIP** with `apiforge-desktop.exe` and `apiforge.exe`,
   unsigned.
3. **Linux tar.gz: x86_64 only**, both binaries, built on `ubuntu-22.04` for
   an older glibc (2.35), with SHA-256 sums.
4. **Trigger: a push of a `v*` tag** builds everything and publishes a GitHub
   pre-release with the SHA-256 sums and notes generated from the merged pull
   requests. The first release is v0.1.0; the coordinating session tags it
   after the owner agrees in a quiz.

## What the owner will see

- New: a *Releases* entry `v0.1.0` (pre-release) with four assets:
  - `apiforge-desktop-v0.1.0-x86_64.AppImage` — the window alone;
  - `apiforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` — `apiforge-desktop`,
    `apiforge`, `README.md`, `CLAUDE-CODE.md`, `licenses/` (the bundled
    fonts' licences; the repository has no LICENSE file of its own);
  - `apiforge-v0.1.0-x86_64-pc-windows-msvc.zip` — the same with `.exe`;
  - `SHA256SUMS.txt` — one line per asset, `sha256sum -c` format; the sums
    are repeated at the end of the release notes.
  `CLAUDE-CODE.md` is a short note: `claude mcp add apiforge -- <path>/apiforge
  mcp`, the `.mcp.json` alternative, and that the Linux CLI comes from the
  tar.gz, not the AppImage.
- New: `apiforge --version` and `apiforge-desktop --version` print
  `<name> 0.1.0`. The AppImage installs a launcher entry *API Forge* with an
  icon, and the window carries the same icon on Linux (today it has none).
- Changed, removed or hidden, migrated, default behaviour: nothing. No
  user-owned category is touched.

## Mock-up

Not applicable for the release itself. The icon is new: the implementation
draws a placeholder (a rounded square in the application's accent colour
with a mark from `icons.rs`), posts it to the issue as an image before the
dry run, and the owner accepts or sends it back there (open point 1).

## Acceptance criteria

1. Pushing a tag `v*` runs `release.yml` once; it builds on `ubuntu-22.04`
   and `windows-2022` and creates **one pre-release** with exactly the four
   assets above, whose notes are generated from the merged pull requests and
   end with the SHA-256 sums. — QA step: the dry run (criterion 9), `gh
   release view --json assets,body,isPrerelease` quoted in the report.
2. Every asset name carries the version and the target as listed; the
   workflow asserts the names before uploading. — QA step: the dry run's
   asset list.
3. The tar.gz and the ZIP contain exactly the entries of
   `packaging/manifest-linux.txt` and `packaging/manifest-windows.txt`; a
   workflow step compares `tar tzf` / the ZIP listing with the manifest and
   fails on any difference. — Test: that step; the manifests are committed
   first.
4. `apiforge mcp`, run from the **extracted archive** on each builder (not
   from `target/`), answers `initialize` (revision `2025-11-25`) and
   `tools/list` with the ten tools of `crates/apiforge-cli/tests/mcp.rs`, and
   exits 0 at EOF. — Test: `packaging/mcp-smoke.py` (`python3 -I`, both
   runners have it) as a workflow step on Linux and Windows.
5. Both binaries answer `--version` with `<name> <Cargo version>`; the
   workflow fails before building when the tag (without `-rc.N`) does not
   equal `v` + the workspace version. — Tests:
   `crates/apiforge-cli/tests/acceptance.rs` and the argument tests in
   `crates/apiforge-desktop/src/main.rs`; QA step: the version check in the
   dry run's log.
6. The AppImage starts on a **clean runner**: a job on `ubuntu-24.04` that
   installs only `libfuse2t64`, `xvfb`, `mesa-vulkan-drivers` and
   `libvulkan1` (display infrastructure, none of the application's
   libraries) runs the downloaded AppImage with `--version`, `--check
   fixtures/workspace.json`, and `--screenshot` under Xvfb producing a
   non-empty PNG that is uploaded and cited in the report;
   `desktop-file-validate` passes on `apiforge.desktop`. — Test: that job.
7. `release.yml` triggers on `push.tags: ['v*']` only — no `pull_request`,
   no untagged push, no `workflow_dispatch`; `permissions: contents: read` at
   the workflow level and `contents: write` only in the publish job; the
   publish job refuses a tagged commit that is not on `origin/main`
   (`git merge-base --is-ancestor`). — QA step: the `on:` and `permissions:`
   blocks quoted; the pull request's Actions list shows only `CI`.
8. A failed step publishes nothing: the publish job `needs` every build and
   smoke job and is the only one that calls the Releases API; both builders
   run `cargo test --workspace --all-targets --locked` before packaging. —
   QA step: the job graph quoted; any failed run of the dry run recorded with
   "no release appeared".
9. Dry run: the tag `v0.1.0-rc.1` produces the pre-release end to end. The
   report records the release URL, each asset's size and SHA-256, the
   AppImage running on the agent's Linux machine, and offers the owner in one
   quiz: delete the rc release and tag, then tag `v0.1.0`. Windows: the ZIP's
   smoke from CI; the owner's own run of `apiforge-desktop.exe` is recorded
   or stated as not run.
10. The verification commands of the development guide pass; `rust.yml` is
    unchanged. The development guide gains a *Releases* section (how to tag,
    what the workflow does); `README.md` points at Releases in *Getting
    started*.

## Why an rc tag and not `workflow_dispatch`

The risky part is publishing — permissions, asset upload, note generation —
and a `workflow_dispatch` that only produces artifacts leaves exactly that
untested (every push already builds the binaries in `rust.yml`). An rc tag
exercises the whole path for the price of a pre-release that is deleted
afterwards.

## Regression potential and what covers it

- The application changes only by `--version` (both parsers) and the
  window icon (`ViewportBuilder::with_icon`). The existing argument tests
  and the CI window probes (`--smoke-test`, `--screenshot`) cover both; a
  bad icon PNG fails at start on every CI run.
- `rust.yml` is not edited; the release workflow is a separate file.
- Release profile (`lto`, `strip = "debuginfo"`) unchanged; binary sizes are
  recorded in the report.

## Not in scope

Code signing (decision 2: unsigned); an aarch64 or musl build (decision 3);
Flatpak; a Windows installer or an `.exe` resource icon; a designed icon
(placeholder now, a ticket if the owner wants one); the CLI inside the
AppImage; AppStream metadata and AppImage update information; a release that
is not a pre-release; a `CHANGELOG`; a project LICENSE file (an owner
decision for its own ticket).

## Open points

1. The icon: closed 2026-10-28. The owner supplied the icon
   (`packaging/icon/apiforge.svg`, a dog on a dark squircle) in place of a
   placeholder. Its PNGs are rendered with resvg 0.48.1 and the bundled
   Adwaita Mono as the only font
   ([development guide](../development.md#releases)).
2. Release notes with no earlier tag cover every merged pull request since
   the repository began (111 of them); acceptable for a first pre-release.

## Taken during implementation (2026-10-28)

- The owner's icon replaces the placeholder (open point 1).
- The README is rewritten for the first release, at the owner's request.
- Licence: GPL-3.0-or-later, copyright The API Forge contributors, external
  contributions under the Developer Certificate of Origin, no CLA; `LICENSE`
  is in every archive and the AppImage.
- The AppImage carries six libraries, not two: the trace in the QA report
  found `libxcb-xkb`, `libXcursor`, `libXfixes` and `libXi` missing from a
  bare Ubuntu 24.04 as well.
- The clean-runner job also starts the AppImage in a bare `ubuntu:24.04`
  container, because the hosted runner image already carries most X
  libraries; the runner step adds `fuse3`, which the pinned static runtime
  mounts through.

## Technical appendix

- `.github/workflows/release.yml`: jobs `build-linux` (ubuntu-22.04: apt
  list of `rust.yml`, `cargo test`, `cargo build --release --locked -p
  apiforge-desktop -p apiforge-cli`, tar.gz + manifest check + MCP smoke,
  AppDir + AppImage), `build-windows` (windows-2022: test, build,
  `Compress-Archive`, manifest check, MCP smoke), `smoke-appimage`
  (ubuntu-24.04, criterion 6), `publish` (needs all three; `gh api
  …/releases/generate-notes` → body + sums → `gh release create --prerelease
  --verify-tag --notes-file`). Actions as in `rust.yml` (`checkout@v4`,
  `upload/download-artifact@v4`, `dtolnay/rust-toolchain`); no third-party
  release action — the runner's `gh` publishes.
- AppImage: a hand-made `AppDir/` (`usr/bin/apiforge-desktop`, `AppRun`
  script exporting `LD_LIBRARY_PATH=$APPDIR/usr/lib`, `apiforge.desktop` with
  `Exec=apiforge-desktop`, `Icon=apiforge`, `Categories=Development;`,
  `StartupWMClass=apiforge` to match `with_app_id("apiforge")`,
  `apiforge.png` and `.DirIcon`), packed by a **tagged** release of
  `AppImage/appimagetool` (1.9.x) downloaded by its release URL with its
  SHA-256 recorded in the workflow and checked before use, run with
  `--appimage-extract-and-run`; the `continuous` build is never used.
  `usr/lib` bundles what winit dlopens and a stock runner lacks —
  expected `libxkbcommon.so.0` and `libxkbcommon-x11.so.0` (the reason
  `rust.yml` installs `libxkbcommon-x11-0`) — and leaves glibc, libX11,
  Wayland, GL and Vulkan to the host as AppImages do; criterion 6 proves the
  list.
- Files: `packaging/{AppRun,apiforge.desktop,CLAUDE-CODE.md,mcp-smoke.py,
  manifest-linux.txt,manifest-windows.txt}`;
  `crates/apiforge-desktop/assets/icon/apiforge.svg` (source) and
  `apiforge-256.png` (committed, so CI needs no SVG renderer; loaded with the
  `image` crate already in the workspace); `--version` in
  `crates/apiforge-cli/src/main.rs` and `crates/apiforge-desktop/src/main.rs`
  from `CARGO_PKG_VERSION` (both crates take `version.workspace`).
- Order: acceptance commit (Rust `--version` tests, `mcp-smoke.py`, the
  manifests) → implementation → rc dry run → QA report → PR ready. The
  backlog row rides on the pull request.
