# Stage II P6a — UI quality and project text files

Fixes for **BUG-035–043**, the user's four reported issues and the accompanying
QA findings assigned to P6 by the
[acceptance plan](../../../stage-ii-code-workspace.md#p6-ui-quality-and-text-file-acceptance).
Package P6a covers that row of P6; publication, the folder CLI and runs follow
in P6b–P6d.

## Fixes

| ID | Failure | Fixed behaviour |
| --- | --- | --- |
| BUG-035 | The folder session's fixed placer assumes one uniform 240 × 96 card, so two analysed blocks landed 120 points apart while the painted cards are 212 and 288 tall. | `Placer::place` now receives the project context the document was analysed in, and the window supplies a placer that measures each card and proxy chip exactly as the board paints them. Opening and initialising a folder both use it. |
| BUG-036 | The tab's pill enclosed only the filename; the close control was a separate button beside it. | A tab is one surface: name, state dot and close control share a rounded rectangle, the close control is inside it with a stable 14-point target, and a click on it does not activate the file it closes. |
| BUG-037 | One click only selected; opening needed a double-click or Enter. | A click selects and activates: a registered document opens as a document, any other file as text, a folder folds. Repeated activation keeps one tab. Enter still works. |
| BUG-038 | The tree listed the root's children only; the project had no row. | The project is the first row, named, selectable by mouse and keyboard, and the explicit destination for new files, folders, imports and drops; the New menu names that destination. Its chevron, Enter and the arrow keys fold it — a plain click only selects, because folding the project hides every row. Ordinary file actions cannot rename or delete it. |
| BUG-039 | Only registered `.afs` documents had tabs; `.gitignore` could only be revealed in the file manager. | Project text files open as text in their own tabs with exact-byte saving, dirty state, Save All, Save/Discard/Cancel on close, local recovery, held drafts, read-only handling and external-change conflicts. They have no diagram and cannot be run, and the diagram view says so. |
| BUG-040 | The active tab was not scrolled into view when the strip overflowed. | Tab widths are measured before anything is drawn, so activation, Ctrl+Tab and a narrower window scroll the active tab into view in the same frame. |
| BUG-041 | Two `alpha.afs` tabs were identical; long names consumed the strip. | A label carries the least parent path that tells it apart from another open file of the same name, is bounded to 160 points, and keeps its full path on hover. |
| BUG-042 | Explorer filenames were painted with no named accessibility node. | Every row reports its name, its role (collapsing header or selectable label), its selection and, for folders, its expansion. The selected row shows the list's keyboard focus. |
| BUG-043 | Reveal called `scroll_to_me`, which a virtualised row that was never drawn can never receive. | The row's place is computed from its index and the drawn range, and asked for through the scroll area. Two further causes were found and fixed: the rows were allocated 24 points apart while the list derived a 30-point pitch from the ambient item spacing, which left the last rows unreachable by scrolling at all; and a reveal was lost when the folder scan arrived afterwards and moved every row down. |

Two defects found during the visual review are fixed with them:

- Restoring a saved view opened its text files with `Workbench::open`, which
  took activation from the document that had it. Restoring now adds the tab
  without activating it.
- A disabled `ActionButton` was painted at full strength, so the disabled
  **Save** in the header read as live. Disabled buttons now use the muted
  foreground, a bordered neutral fill for the primary style, and no hover,
  pressed or focus state.

## Acceptance

`tools/qa/ui-quality/run.sh HEAD` — **10 passed, 0 failed**
([log](acceptance.log)). At the P5 baseline two controls passed and eight
acceptance probes failed. Give the gate its own `CARGO_TARGET_DIR`: it builds an
identical package from a temporary checkout, and sharing the repository's leaves
a stale binary that later repository runs silently reuse.

Seventeen new tests are part of the standard suite, not the gate:

- Eight session tests for project text files (`apiforge-project`): exact bytes
  including CRLF, refusal of managed data, the registry, `.git`, registered
  documents, directories, non-UTF-8 and the 1 MiB limit; recovery across a
  restart; a held draft of a changed file; external change, deletion and the
  keep/accept decisions; a read-only second session; an explorer move carrying
  unsaved work and a delete leaving it with a deletion conflict; and closing
  needing a decision.
- One storage test for the recovery format's text drafts and their boundaries.
- One placement test: the painted cards of a two-step scenario do not intersect.
- Seven window regressions driven by real pointer and keyboard events: one-click
  opening and the project row as a destination, the keyboard reaching the
  project row and opening a text file, a text file edited, saved exactly,
  recovered across a restart and closed by decision, a text file having no
  diagram, a tab's close control closing only its own file and still asking
  about unsaved work, equal names told apart with a bounded long label, and
  Ctrl+Tab bringing the active tab back into view.

## Native review

The real window under Xvfb with Mesa Lavapipe, through `--screenshot`:

| Capture | What it shows |
| --- | --- |
| [Code at 1600 × 1000](code-1600.png) | The project row, one tab with its close control inside it, text files listed beside documents. |
| [Code at 960 × 540](code-960.png) | The same at the minimum supported window; every control stays visible. |
| [Twenty tabs at 960 × 540](tabs-and-reveal-960.png) | The active `task-13.afs` revealed at the end of the strip and its explorer row revealed and highlighted at the end of a virtualised list; `README.md` and `.gitignore` as text tabs; `broken.afs` in the error colour; duplicate names carrying their folders. |
| [Split at 1600 × 1000](split-cards-1600.png) | Two freshly analysed cards placed without overlap, with the wire between them. |
| [A text file at 1600 × 1000](text-file-1600.png) | `README.md` in plain monospace with no scenario colouring, its own FILE panel, and the disabled Save controls painted as disabled. |

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked -- --test-threads=1`, with
  isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`

## Limits

This package does not include component publication, the folder CLI or runs
(P6b–P6d). File operations, saves and project reloads still run on the UI
thread. The accessibility evidence is egui's AccessKit output, not a native
screen-reader audit, and the native review used `--screenshot` captures only:
no pointer or keyboard input was replayed against the real window, because
`xdotool` is not available here. Windows QA and Windows directory durability
remain open P7 gates and are not addressed.

The `--screenshot` probe waits for the project to open and has no bound, so a
folder it cannot open leaves it waiting instead of failing. That cost a
diagnosis here and is worth a bounded wait in P7's native QA work.
