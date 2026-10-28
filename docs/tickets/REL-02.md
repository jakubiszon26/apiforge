# REL-02: The application icon on Windows — the window and the .exe

- Size: S · Owner review: 10 min · Risk: `visible UI` (the icon only)
- State: Analysis approved — the owner's decision of 2026-10-28 on #119
  (REL-01 acceptance quiz, recorded on #115) fixed the goal, the estimate and
  the flag; this file is the analysis it asked for. Implementation starts
  after REL-01 (#118) merges: it builds on `packaging/icon/`, `window_icon()`
  and `release.yml` of that pull request. Before `v0.1.0`.
- Analysis model: Fable · Implementation model: —
- After REL-01 (#118)

## Goal

On Windows the window shows the owner's icon in its title bar, on the taskbar
and in Alt+Tab, and `apiforge-desktop.exe` and `apiforge.exe` show it in
Explorer — the same dog as the Linux window and the AppImage.

## What the owner will see

- New: the icon on the Windows window instead of egui's default logo; the
  icon on both `.exe` files in Explorer. Explorer's *Details* tab also names
  the product (*API Forge*) and the version — the icon travels in a version
  resource, so the version strings come for free (the agent's decision,
  reported here).
- Changed: nothing on Linux; the window icon is set by the same function on
  every platform (Wayland keeps taking it from the desktop file).
- Removed or hidden, migrated, default behaviour: nothing. No owner-owned
  category is touched.

## Mock-up

Not applicable beyond the icon itself, which the owner supplied and approved
in REL-01 (`packaging/icon/apiforge.svg`, PNGs rendered by resvg 0.48.1).
Nothing else visible changes; the QA report shows the Windows title bar,
taskbar and Explorer captures.

## Acceptance criteria

1. `packaging/icon/apiforge.ico` holds exactly the 16, 32, 48, 64, 128 and
   256 px renders `packaging/icon/apiforge-<size>.png`, as PNG-compressed
   entries in that order, assembled by `packaging/icon.py` (standard library
   only) from those files. Test `test_the_ico_is_built_from_the_pngs` in
   `packaging/test_release.py` rebuilds the `.ico` in memory and asserts
   byte equality with the committed file, so the icon cannot drift from its
   source.
2. Both binaries carry the icon as a Windows resource: `build.rs` in
   `apiforge-desktop` and `apiforge-cli` runs `winresource` (a
   `[target.'cfg(windows)'.build-dependencies]` entry, pinned by
   `Cargo.lock`; the script does nothing on other targets) with the `.ico`,
   `ProductName` *API Forge*, `FileDescription` *API Forge* / *API Forge
   command line*, and `FileVersion`/`ProductVersion` from
   `CARGO_PKG_VERSION`. On the `windows-2022` job of `rust.yml`, a step
   extracts the associated icon of each release `.exe`
   (`[System.Drawing.Icon]::ExtractAssociatedIcon`), saves it as PNG, and
   fails unless it is 32 × 32 with a transparent corner and an opaque centre
   (the unit test of `window_icon()` asserts the same of the source).
3. The window: the `#[cfg(target_os = "linux")]` gate on `with_icon` in
   `crates/apiforge-desktop/src/main.rs` goes. The same `windows-2022` step
   starts `apiforge-desktop.exe --folder <shop>`, waits until the process
   has a `MainWindowHandle`, captures the desktop
   (`System.Drawing.Graphics.CopyFromScreen`) and ends the process; the
   capture joins the icon PNGs in the artifact `windows-icon`. QA step: the
   report cites the capture showing the icon in the title bar and taskbar.
4. `release.yml`'s *Smoke the binaries from the extracted ZIP* step asserts
   `(Get-Item <exe>).VersionInfo.ProductVersion` equals the release version
   for both binaries.
5. The existing unit test `the_window_icon_is_the_256_px_application_icon`
   runs unchanged on both platforms; the Linux title-bar capture of REL-01 is
   unaffected (same function, same bytes). The verification commands of the
   development guide pass; a Linux build needs no new dependency and no
   `rc.exe`.
6. `docs/development.md` *Releases* says in one paragraph where the `.ico`
   comes from and how to rebuild it.

## Not in scope

An installer; a Wayland icon beyond the desktop file; a macOS bundle; an
icon theme for the AppImage integration tools.

## Regression potential and what covers it

Low: the icon function is already exercised on Linux; the Windows build gains
a build script that needs the Windows SDK's `rc.exe`, which `windows-2022`
has (criterion 2 proves it on every CI run, not only at a tag).

## Technical appendix

- Files: `crates/apiforge-desktop/build.rs`, `crates/apiforge-cli/build.rs`,
  both `Cargo.toml`, `Cargo.lock`, `crates/apiforge-desktop/src/main.rs`,
  `packaging/icon/apiforge.ico`, `packaging/icon.py`,
  `packaging/test_release.py`, `.github/workflows/rust.yml`,
  `.github/workflows/release.yml`, `docs/development.md`.
- The `.ico` format: a 6-byte header, one 16-byte directory entry per image
  (width and height bytes 0 for 256), then the PNG files verbatim; Windows
  reads PNG entries at every size since Vista.
- `winresource` is the maintained fork of `winres`; it locates `rc.exe`
  through the Windows SDK registry keys and needs no PATH setup on the runner.
