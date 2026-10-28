# REL-03: Third-party licence notices in every package

- Size: S · Owner review: 10 min · Risk: none (nothing on screen; the
  archives gain files)
- State: Analysis approved — the owner's decision of 2026-10-28 on #120
  (REL-01 acceptance quiz, recorded on #115) fixed the goal and the estimate;
  this file is the analysis it asked for. Implementation starts after REL-01
  (#118) merges: `release.yml`, `packaging/release.py` and the manifests are
  that pull request's. `v0.1.0-rc.1` may go out without it; `v0.1.0` may not.
- Analysis model: Fable · Implementation model: —
- After REL-01 (#118)

## Goal

Whoever downloads a package finds, next to the GPL-3.0 `LICENSE`, the notices
of everything else it contains: every Rust crate linked into the two
binaries, the fonts egui embeds, and on Linux the six X libraries the
AppImage carries.

## What the owner will see

- New: `licenses/THIRD-PARTY-NOTICES.md` in the tar.gz, the ZIP and the
  AppImage: one section per licence (MIT, Apache-2.0, BSD, ISC, Zlib,
  Unicode-3.0, BSL-1.0, MPL-2.0, CDLA-Permissive-2.0, OFL-1.1,
  Ubuntu-font-1.0, …) with the licence text once and, under it, each crate
  with version and copyright holders; the egui default fonts (Hack,
  Ubuntu-Light, NotoEmoji, emoji-icon-font) listed with their texts. In the
  AppImage also `licenses/appimage-libraries/<library>.copyright`, Ubuntu's
  copyright file of each library in `packaging/appimage-libraries.txt`.
- Changed: the manifests list the file, so a release without it fails; the
  README's licence paragraph names it in one sentence.
- Removed or hidden, migrated, default behaviour: nothing. No owner-owned
  category is touched.

## Mock-up

Not applicable: no interface change.

## Acceptance criteria

1. `about.toml` (repository root) accepts exactly the licences in the
   workspace today, ordered MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception,
   BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, Unlicense, Unicode-3.0,
   BSL-1.0, MPL-2.0, CDLA-Permissive-2.0, OFL-1.1, Ubuntu-font-1.0, so a crate
   offering a choice is listed under the first accepted one and never under
   GPL-2.0-only (`self_cell`) or LGPL-2.1 (`r-efi`); `targets` names
   `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`; workspace crates
   are ignored. `cargo about generate --fail` with `packaging/about.hbs`
   produces the file; an unaccepted or unresolved licence fails the command.
   This runs on the Linux job of `rust.yml` on every push, so a new dependency
   with an unaccepted licence fails CI, not the release.
2. The file names every crate of both binaries: `python3 -I
   packaging/release.py check-notices <file>` reads on stdin the output of
   `cargo tree -e normal --locked --prefix none --format "{p}" --target
   <triple> -p apiforge-desktop -p apiforge-cli` for each triple and fails
   when a non-workspace crate is missing from the file. Unit tests in
   `packaging/test_release.py` cover a complete, a missing and a workspace
   crate; `release.yml` runs the check where the file is generated.
3. The egui default fonts appear with their licence texts through a
   `clarify` entry for `epaint_default_fonts` naming its `fonts/*.txt`
   files; test: `check-notices` also requires the four font names.
4. Pinned generator: cargo-about's prebuilt `x86_64-unknown-linux-musl`
   release, URL and SHA-256 in `release.yml` and `rust.yml` as appimagetool
   is, verified with `sha256sum -c` before use. The file is generated once in
   the `version` job, uploaded as artifact `notices`, and downloaded by
   `build-linux` and `build-windows` into `licenses/` of their stages and of
   the AppDir; the same bytes are in every package.
5. `packaging/manifest-linux.txt` and `packaging/manifest-windows.txt` list
   `licenses/THIRD-PARTY-NOTICES.md`; the existing `check-archive` fails
   without it. The AppImage smoke job's `--appimage-extract` lists
   `licenses/THIRD-PARTY-NOTICES.md` and the six `.copyright` files and fails
   when one is missing.
6. `README.md` (one sentence in the licence section) and
   `docs/development.md` *Releases* (one paragraph: the generator, its pinned
   version, how to regenerate locally, how to add a licence) are updated;
   `crates/apiforge-desktop/assets/fonts/FONT-SOURCES.md` points to the
   notices for the egui fonts.
7. `python3 -I packaging/test_release.py` and the verification commands of
   the development guide pass.

## Not in scope

Notices for the development tools (CI actions, appimagetool, cargo-about
itself); an SBOM; licence compatibility review beyond the accepted list
(every licence above is compatible with distribution under
GPL-3.0-or-later); Liberation Sans, which is in `assets/fonts` but not
embedded.

## Regression potential and what covers it

A release could fail on a crate whose licence file cargo-about cannot find:
criterion 1 runs the generation on every push, so the failure surfaces in a
pull request. The manifest check (criterion 5) is REL-01's, extended by one
line.

## Technical appendix

- Files: `about.toml`, `packaging/about.hbs`, `packaging/release.py`,
  `packaging/test_release.py`, `packaging/manifest-linux.txt`,
  `packaging/manifest-windows.txt`, `.github/workflows/release.yml`,
  `.github/workflows/rust.yml`, `README.md`, `docs/development.md`,
  `crates/apiforge-desktop/assets/fonts/FONT-SOURCES.md`.
- Today's dependency graph (487 non-workspace packages across all targets
  and dev-dependencies) has a licence field on every crate; the unusual ones
  are `clipboard-win`/`error-code` (BSL-1.0), `option-ext` (MPL-2.0),
  `webpki-roots` (CDLA-Permissive-2.0), the `icu_*`/`zerovec` family
  (Unicode-3.0), `epaint_default_fonts` (fonts).
- The AppImage copyright files come from
  `/usr/share/doc/{libxkbcommon0,libxkbcommon-x11-0,libxcb-xkb1,libxcursor1,libxfixes3,libxi6}/copyright`
  on the `ubuntu-22.04` builder, in the *Build the AppImage* step.
