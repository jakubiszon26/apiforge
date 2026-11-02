# REL-02: The application icon on Windows — the window and the .exe — QA report

- Branch and commit: `feat/rel-02-windows-icon` at `9677e14`. The Windows
  evidence comes from that commit's
  [pull_request CI run 41437797866](https://github.com/jakubiszon26/apiforge/actions/runs/41437797866)
  (`windows-2022`, artifact `windows-icon`). The Linux evidence comes from a
  local release build of the same commit.
- Built with: `cargo build --release --locked -p apiforge-desktop -p
  apiforge-cli`, on `windows-2022` by CI (MSVC, the Windows SDK's `rc.exe`)
  and on Ubuntu 24.04 locally, with Rust 1.99.0.
- Ticket: [docs/tickets/REL-02.md](../../../tickets/REL-02.md). Analysis:
  Fable. Implementation: Claude Opus 5.5.
- Compared against: the icon the owner approved in REL-01
  (`packaging/icon/apiforge.svg` and its PNGs). There is no other mock-up.

## Acceptance criteria

| # | Criterion | How it is verified | Result |
| --- | --- | --- | --- |
| 1 | `apiforge.ico` = the 16–256 px PNGs, PNG entries, in order, built by `packaging/icon.py` (standard library) | `test_release.py`, `IconFile`: rebuild and byte equality; the format read back on its own; stdlib-only imports. Scenario 1 | Pass |
| 2 | Both `.exe` carry the icon and version strings; every `windows-2022` run extracts the icon of each | `WindowsResource` and `IconWorkflows` in `test_release.py`; CI step *Show the application icon (Windows)*. Scenario 2 | Pass: both 32 × 32 icons are pixel-identical to `apiforge-32.png` |
| 3 | No Linux gate on `with_icon`; the running window is captured with its icon | `WindowIconSource`; unit test `the_window_carries_the_application_icon_on_every_platform`, which passes on both CI platforms; the CI capture. Scenario 3 | Pass: the icon is in the title bar and on the taskbar |
| 4 | The release ZIP smoke asserts `ProductVersion` for both binaries | `IconWorkflows.test_the_release_checks_the_product_version_of_both_exes` | Pass by test and inspection; only a pushed tag runs it |
| 5 | The existing icon test is unchanged; Linux is unaffected; no Linux dependency or `rc.exe` | `the_window_icon_is_the_256_px_application_icon` untouched; `WindowsResource.test_winresource_is_a_windows_only_build_dependency_pinned_by_the_lock`. Scenario 4 | Pass |
| 6 | *Releases* says where the `.ico` comes from and how to rebuild it | `IconDocs` | Pass |

The acceptance commit `60f7fb7` came before the implementation. All 11 tests
failed there for the right reason. Four raised `FileNotFoundError`
(`packaging/icon.py`, the `.ico`, `build.rs`). Seven assertions failed:
the missing workflow steps, the missing `ProductVersion` check, the missing
lock entry, the guide paragraph, and the `cfg(target_os = "linux")` gate,
which was still present.

## Linux

### Scenario 1: the `.ico` is the six PNGs

1. Run `python3 -I packaging/icon.py`, then `--check`.
2. Open the `.ico` with Pillow 10.2, an independent reader, and compare
   each entry with its PNG.

Expected: six entries (16, 32, 48, 64, 128 and 256 px), each decoding to the
pixels of its PNG.

Observed: as expected. The file is 51 907 bytes, and every entry is
pixel-identical to its source. [The six entries](ico-entries.png) are shown
on a checkerboard so that the transparency is visible.

### Scenario 4: the Linux window icon is unaffected

1. Start the release `apiforge-desktop --project fixtures/workspace.json` on
   Xvfb (1920 × 1200, Lavapipe, X11).
2. After ten seconds, read `_NET_WM_ICON` of the top-level window through
   libX11, then end the process.

Expected: the window *API Forge* carries one 256 × 256 image, equal to
`apiforge-256.png`.

Observed: window `API Forge` carries `_NET_WM_ICON` images `[(256, 256)]`,
pixel-identical to `packaging/icon/apiforge-256.png`. This machine has no
window manager, so the X property stands in for REL-01's Openbox title-bar
capture. The pixels are the same, from the same function.

## Windows

All from CI run 41437797866 on `windows-2022`, at 1024 × 768 and 96 DPI.

### Scenario 2: both `.exe` files carry the icon and the version

The CI step reads each release binary's `VersionInfo`, then
`[System.Drawing.Icon]::ExtractAssociatedIcon` (the icon Explorer shows for
the file), saves it, and checks it.

Expected: *API Forge* as the product name; *API Forge* or *API Forge command
line* as the description; `0.1.0` as the file and product version; a 32 × 32
icon with a transparent corner and an opaque centre, equal to
`apiforge-32.png`.

Observed, from the step log:

```text
apiforge.exe: ProductName 'API Forge', FileDescription 'API Forge command line', ProductVersion '0.1.0', FileVersion '0.1.0'
apiforge.exe: icon 32 x 32, largest channel difference from apiforge-32.png 0
apiforge-desktop.exe: ProductName 'API Forge', FileDescription 'API Forge', ProductVersion '0.1.0', FileVersion '0.1.0'
apiforge-desktop.exe: icon 32 x 32, largest channel difference from apiforge-32.png 0
```

Re-read locally, the downloaded PNG is byte-for-byte the same pixels as
`apiforge-32.png`. The extracted icons are
[apiforge.exe](windows-apiforge-exe-icon.png) and
[apiforge-desktop.exe](windows-apiforge-desktop-exe-icon.png), with the same
icons enlarged 8× ([apiforge.exe](windows-apiforge-exe-icon-x8.png),
[apiforge-desktop.exe](windows-apiforge-desktop-exe-icon-x8.png)).

### Scenario 3: the window shows the icon

The CI step starts `apiforge-desktop.exe --folder <the migrated demo shop>`,
waits for its `MainWindowHandle`, waits five more seconds, captures the
desktop with `CopyFromScreen`, and ends the process.

Expected: the dog in the window's title bar and on its taskbar button, in
place of egui's default logo.

Observed: as expected. [Desktop capture](windows-desktop-window.png):
*Sklep demonstracyjny* is open in Code. The active window's title bar
begins with the dog ([enlarged 4×](windows-titlebar-x4.png)), and the
taskbar button of the running application is the dog
([enlarged 3×](windows-taskbar-x3.png)). The window is 1600 px wide on a
1024 px display, so its right edge is off screen; the title bar and the
taskbar button are visible. The console window behind it belongs to the CI
runner.

## Not verified

- **Seen by eye on a real Windows desktop**: Explorer's icon view of both
  `.exe` files, the *Details* tab of *Properties*, Alt+Tab, the Start menu,
  pinned taskbar shortcuts. The evidence above is programmatic: the icon and
  version that Windows APIs return, plus one CI desktop capture. Explorer
  reads the same resources, but nobody looked at Explorer.
- **Other displays and versions**: only 96 DPI and 1024 × 768 on Windows
  Server 2022. Not verified at 125–200 % scaling, where Windows picks the 48,
  64 or 256 px entry, and not on Windows 10 or 11 desktops.
- **The release workflow**: the new `ProductVersion` assertion in
  `release.yml` runs only on a pushed tag. It is covered by the static test
  and by the identical `VersionInfo` read in `rust.yml`, which passed.
- **Cross-compiling** for Windows from Linux embeds no icon: the build script
  runs `winresource` on Windows hosts only, as the ticket says. No release
  is built that way.
- **The Linux title bar under a window manager** (REL-01's Openbox capture):
  Openbox is not installed here. `_NET_WM_ICON` was read directly instead
  (Scenario 4).
- **The 16 px entry**, which Windows uses for the small title-bar icon at
  96 DPI: its legibility was judged in REL-01 (the face is not recognisable
  at 16 px). REL-02 does not change it.

## Regression smoke list

2026-11-02. The change adds a resource to both executables and removes a
platform gate on the window icon; nothing else is touched.

| # | Flow | Linux | Windows |
| --- | --- | --- | --- |
| 1 | Start with no arguments | Pass in CI (managed startup creates one project) | Pass in CI, same step |
| 2 | Open a folder project | Pass in CI (folder captures of the migrated shop) | Pass in CI; Scenario 3 opens the shop in Code |
| 3–8 | Library, wiring, editing, simulation, save, dirty close and kill | Not run by hand: the change touches none of these; the workspace test suite passes | Workspace tests pass in CI |
| 9 | Migrate a legacy workspace | Pass in CI (four projects, original byte-identical) | Pass in CI |
| 10 | Window at 960 × 540 | Pass in CI (captures at both sizes) | CI captures taken; the display is smaller than 1600 × 1000 |

## Verification

- `cargo fmt --all -- --check`: pass
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: pass
- `cargo test --workspace --all-targets --locked`: pass
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`: pass
- `python3 -I packaging/test_release.py`: pass (45 tests, 11 of them new)
- `python3 -I packaging/icon.py --check`: pass
- CI on `9677e14`: the pull_request run passed on both platforms. The push
  run failed once on `windows-2022` in
  `header_tests::a_narrowing_diagram_row_drops_the_kind_then_shortens_the_name_then_the_summary`
  ("hovering the shortened name shows its full text"), a known flake listed
  on #122. Its failed job was rerun once and passed.
