# REL-03: Third-party licence notices in every package — QA report

- Branch: `feat/rel-03-third-party-notices`, `main` merged in (REL-01).
  Evidence taken on `97ac083`; the CI run of the pull request's head is
  linked from the pull request.
- Ticket: [docs/tickets/REL-03.md](../../../tickets/REL-03.md). Analysis:
  Fable. Implementation: Claude Opus 5.5, under `ready:sol`.
- Generator: cargo-about 0.9.2. CI downloads the prebuilt
  `x86_64-unknown-linux-musl` release and checks its SHA-256
  (`9099a59e…2566af`, the digest GitHub publishes for the asset). Locally
  the same version was built with `cargo install cargo-about --version
  =0.9.2 --locked --features cli`, because this machine refuses to run a
  downloaded binary.
- Nothing on screen changes. The packages gain files under `licenses/`.

## Acceptance criteria

| # | Criterion | How it is verified | Result |
| --- | --- | --- | --- |
| 1 | `about.toml` accepts exactly today's licences in the given order, for both targets, with workspace crates ignored; `cargo about generate --fail` runs on every push in `rust.yml` | `test_release.py` `AboutConfig`, `NoticeWorkflows.test_every_push_…`; Scenarios 1 and 2; CI | Pass |
| 2 | `release.py check-notices` fails when a non-workspace crate of either binary is missing; unit-tested; `release.yml` runs it where the file is generated | `Notices` (complete, missing, other version, workspace crate, empty tree, stdin); `NoticeWorkflows.test_the_release_generates_…`; Scenario 1 | Pass |
| 3 | The egui default fonts appear with their licence texts through a `clarify` entry | `AboutConfig.test_the_egui_fonts_are_clarified_…`; `Notices.test_each_egui_font_…` (names and texts); Scenario 3 | Pass |
| 4 | Pinned prebuilt cargo-about (URL and SHA-256 checked with `sha256sum -c`) in both workflows; generated once in `version`, uploaded as `notices`, downloaded into every package | `NoticeWorkflows.test_one_pinned_…`, `test_the_release_generates_…`; CI's *Download cargo-about* step | Pass for `rust.yml`; the `release.yml` jobs run only on a tag (Not verified) |
| 5 | Both manifests list `licenses/THIRD-PARTY-NOTICES.md`; `check-archive` fails without it; the AppImage smoke requires the notices and the six `.copyright` files | `NoticeManifestsAndDocs`, `NoticeWorkflows.test_the_appimage_…`; Scenario 4 | Pass for the manifests and `check-archive`; the AppImage steps run only on a tag |
| 6 | README, the guide's *Releases* and `FONT-SOURCES.md` updated | `NoticeManifestsAndDocs.test_the_readme_…` | Pass |
| 7 | `test_release.py` and the guide's verification commands pass | Verification below | Pass |

The acceptance commit (`a5cf0a8`) failed for the right reasons: 7 failures
and 11 errors, all from what did not exist yet (`release.check_notices`,
`about.toml`, the workflow steps, the manifest line, the documentation).
The implementation commits did not change the tests.

## Linux

### Scenario 1: generate the notices and check them against both binaries
1. `cargo about generate --frozen --workspace --fail -o N.md packaging/about.hbs`
   on Ubuntu 24.04, Rust 1.99.0.
2. `cargo tree … --target <triple> -p apiforge-desktop -p apiforge-cli` for
   both triples, piped into `release.py check-notices N.md`.

Expected: the command passes; the file names every crate.
Observed: `N.md names all 332 crates and the egui fonts`. The file has
7 602 lines (317 KB). A second generation is byte-identical, and so is one
without `--frozen`: cargo-about fetches nothing for this configuration. The
overview counts MIT 310, ISC 19, Unicode-3.0 19, Apache-2.0 17,
BSD-3-Clause 2, BSL-1.0 2, CDLA-Permissive-2.0 1, MPL-2.0 1, OFL-1.1 1,
Ubuntu-font-1.0 1 and Zlib 1. Counted per licence text, so one crate may
count more than once. `self_cell` is listed under Apache-2.0, never under a
GNU licence; `r-efi` is not on either target.

The file covers the workspace's normal dependencies on both targets: ten
crates more than the two binaries need (`axum` and four of its crates from
`apiforge-demo-api`, and five that cargo-about's target filter keeps, such
as `windows_x86_64_gnu`). A notice too many is harmless; a missing one
fails the check.

### Scenario 2: a licence that is not accepted fails the generation
1. Remove `"Zlib"` from `accepted`, generate again.

Expected: a non-zero exit and no output.
Observed: `encountered 1 errors resolving licenses, unable to generate
output`, exit 1, after about three minutes of scanning for an alternative.
`about.toml` was restored.

### Scenario 3: the egui fonts and a stale clarification
1. Look for `epaint_default_fonts` in the generated file.
2. Change one checksum of the clarification, generate again.

Expected: the four texts (Hack and Bitstream Vera, Ubuntu Font Licence
1.0, SIL OFL 1.1, the MIT licence of emoji-icon-font) under
`epaint_default_fonts 0.36.2`. A stale checksum must not pass unnoticed.
Observed: as expected for step 1. For step 2, cargo-about only warns (`failed
to validate all files specified in clarification`), exits 0 and falls back to
the files it finds by itself, which miss `Hack-Regular.txt`. That is why
`check-notices` also needs each font's licence text: the fallback file fails
on `BITSTREAM VERA LICENSE`.

### Scenario 4: the archive check needs the notices
1. Stage the tar.gz as `release.yml` does, with empty binaries, without the
   notices; `pack`, then `check-archive` against `manifest-linux.txt`.
2. The same with `licenses/THIRD-PARTY-NOTICES.md`.
3. Collect the copyright files as the *Build the AppImage* step does
   (`dpkg -S` for the exact path, then `/usr/share/doc/<package>/copyright`).

Observed: (1) `…/licenses/THIRD-PARTY-NOTICES.md is missing`, exit 1;
(2) `… matches manifest-linux.txt`; (3) six files,
`libxkbcommon.so.0.copyright` … `libXi.so.6.copyright`, 1.7 to 4.4 KB, from
`libxkbcommon0`, `libxkbcommon-x11-0`, `libxcb-xkb1`, `libxcursor1`,
`libxfixes3` and `libxi6`. This machine runs Ubuntu 24.04, not the
`ubuntu-22.04` builder.

### CI on the branch
The first push (`c356492`) failed *Generate and check the third-party
notices*. The prebuilt cargo-about generated the file, but `cargo tree`
coloured its `(*)` markers because the workflows set
`CARGO_TERM_COLOR=always`, and `check-notices` refused the line. Fixed in
`97ac083`: the commands pass `--color never`, and `check-notices` strips
colour codes anyway. The run of the pull request's head uploads the
generated file as the artifact `third-party-notices`.

## Windows

No Windows step changes in `rust.yml`; the notices are generated on Linux
only. The ZIP gains the file in `release.yml`'s *Pack and check the ZIP*,
which runs only on a tag. Not run on Windows.

## Dry run

Not run: no tag was pushed by this branch, and `release.yml` has no other
trigger. The next tag (`v0.1.0-rc.2` or `v0.1.0`) exercises it as follows:
- `version`: the notices are generated and checked on `ubuntu-22.04`
  (Python 3.10, which is why `test_release.py` reads `about.toml` as text),
  then uploaded as `notices`;
- `build-linux`/`build-windows`: `check-archive` fails unless the tar.gz and
  the ZIP hold the file;
- `smoke-appimage`: *Check the AppImage's licence files* compares the
  AppImage's copy byte for byte with the artifact and requires the six
  `.copyright` files.

The REL-01 dry run (`v0.1.0-rc.1`) is now recorded in
[its report](../../2026-10-28/REL-01/report.md#dry-run), as #115 asked.

## Not verified

- The tag-triggered `release.yml` jobs: generation on `ubuntu-22.04`, the
  `notices` artifact, the packages carrying it, the AppImage's copyright
  files on the 22.04 builder, the smoke check. They run on the next tag.
- The prebuilt cargo-about on this machine, which refuses to run a
  downloaded binary. CI runs the prebuilt; locally the same version was
  built from crates.io. The two may differ in features, but no feature
  matters here: nothing is fetched.
- A legal review of the texts. cargo-about takes each crate's licence file
  as packaged. A crate that ships no file gets the SPDX reference text,
  without its copyright line.
- `Hack-Regular.txt` is filed under MIT in the clarification. It holds both
  Hack's MIT licence and the Bitstream Vera licence, and both texts appear
  in full. `Bitstream-Vera` is not in the accepted list, which criterion 1
  fixes exactly.

## Regression smoke list

Not run: no Rust code and no binary changes; the packages only gain files.
The workspace tests and the CI window probes cover the flows.

## Verification

- `python3 -I packaging/test_release.py`: 31 tests, OK.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings`, `cargo test --workspace --all-targets --locked`
  (63 test binaries, none failed) and `cargo run -p apiforge-convert-v1
  --locked -- --fixtures --check`: pass on this machine (Ubuntu 24.04,
  Rust 1.99.0) on `97ac083`.
- CI on the pull request: the Linux job's *Generate and check the
  third-party notices*, *Upload the third-party notices* and *Test the
  release checks (Linux)* pass with the prebuilt cargo-about.
