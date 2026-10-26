# KTLO-20: A sleeping dog keeps the empty board company in Diagram and Split

- Size: S · Owner review: 20 min · Risk: `visible UI`
- State: Analysis (issue #46) — mock-up posted on the issue, waits for the
  owner's decisions below
- Analysis model: Fable · Implementation model: — (Opus 5.5 when approved)
- Mock-up branch: `mock/ktlo-20-sleeping-dog`, commit 1506a9e (throwaway;
  deleted after the decision)
- Owner's wish of 2026-10-26; the last ticket added to the KTLO sprint.

## Goal

In the folder window, when no file is open, the Diagram view and the board
side of Split show the owner's sleeping dog above the text "Open a scenario
or component to see its diagram.", so the empty board is a resting place
rather than a blank.

## What the owner will see

- **New:** the dog from the owner's drawing (the grey loaf asleep, with the
  sleep bubble, three paws and "z z Z") above the hint, the pair centred in
  the board area; 220 points wide at 960 × 540 and 280 at 1600 × 1000, as it
  grows a little with the board's height. It appears in Diagram and on
  the board side of Split whenever no tab is open; closing the last tab shows
  it, opening a file hides it. It is drawn with the application's own font
  for the letters and without the drawing's soft drop shadow (the ground
  shadow stays); the mock-up shows exactly what will be drawn.
- **Changed:** the hint text, alone in the centre today, moves down by about
  80 points to sit under the dog. The code side of Split is unchanged: its own
  hint "Open a scenario or component from the document list." stays at its
  top left.
- **Removed or hidden:** nothing.
- **Migrated or converted:** nothing.
- **Default behaviour that changes:** none. Decision 1 decides whether the
  dog moves; with the recommendation it does not.

The dog is decoration: it has no accessible name and screen readers announce
only the hint, as today. The other hints of the board — a text file, a missing
or unreadable file, code with errors, "Checking the code…" — keep their text
alone (decision 2).

## Mock-up

`docs/qa/2026-10-26/KTLO-20/`: `mock-diagram-960x540.png`,
`mock-diagram-1600x1000.png`, `mock-split-960x540.png` and
`mock-split-1600x1000.png`. Posted on the issue.

Rendering notes: the mock-up draws the dog with the egui painter from the
paths of the supplied SVG, as the still frame the drawing itself shows when
the system asks for reduced motion. Whatever decision 1 brings, this is the
frame the owner approves; an animated dog breathes around it.

## Decisions for the owner

1. **Still or animated?**
   - **A (recommended): still.** The frame of the mock-up, always. Nothing
     is redrawn while the window rests; a laptop's battery and fan do not
     notice an empty board; tests settle as they do today; no new dependency.
     The drawing's own `prefers-reduced-motion` block shows this frame, so
     the author of the drawing already chose it for people who want calm.
   - **B: animated as the drawing animates** — breathing body, pulsing
     ground shadow, growing and shrinking bubble, ear flops, a tail twitch,
     floating letters, on a 4-second loop. Consequences: the window redraws
     about sixty times a second for as long as an empty board is on screen
     (today an idle window redraws nothing), which costs battery on a laptop;
     the operating system's reduced-motion setting cannot be honoured, egui
     does not expose it; every interaction test that reaches an empty board
     needs the animation switched off, or the harness never settles. The
     breathing is subtle and most of what moves is the bubble and the
     letters.
2. **Only the "no file open" hint, as asked?**
   - **A (recommended): yes.** The other hints of the board announce a
     problem (a missing file, code with errors, a file that is not UTF-8) or
     a wait; a sleeping dog beside "The code has errors" reads as
     indifference. The empty Code view keeps its own hint without a dog.
   - **B:** also under the neutral hint for a text file ("… is a text file,
     not a scenario or component, so it has no diagram.").
3. **The mock-up as a whole:** size (220 points wide at the minimum window,
   280 at the default), place (above the hint, the pair centred), the letters in the
   application's bold font, and no drop shadow.

## Acceptance criteria

1. In Diagram with no tab open, the board area holds the dog above the hint
   "Open a scenario or component to see its diagram.": the dog's box and the
   hint are both inside the board area and inside the window, do not overlap,
   share their horizontal centre, and the dog's box is 220 points wide at
   960 × 540 and 280 at 1600 × 1000 in the drawing's 50:34 proportion
   (interaction test at both sizes).
2. In Split with no tab open, criterion 1 holds on the board side, and the
   code side shows "Open a scenario or component from the document list." as
   today (both sizes).
3. With a tab open — a scenario, a component, a text file, a missing file or
   a file whose code has errors — no dog is drawn and the hints read as they
   do today.
4. Closing the last tab in Diagram shows the dog; opening a file from the
   document list hides it.
5. The hint is the only new or changed node in the accessibility tree: the
   dog adds none, and the existing accessibility audit passes.
6. With decision 1 = A: the empty board requests no repaint; `settle()` in
   the harness returns at once. With decision 1 = B: the animation runs only
   while the empty board is visible, stops when a tab opens, and is off in
   the test harness so that `settle()` still returns.
7. At 960 × 540 with double scale the dog is sharp and keeps its proportions
   (design review capture; manual QA step, no pixel assertion).

## Not in scope

The empty Code view and the start screen; the other hints of the board
(decision 2); the drop shadow and the drawing's font; any animation if
decision 1 = A; a setting to hide the dog.

## Technical appendix

- Today: `FolderApp::diagram_area` (`crates/apiforge-desktop/src/folder/diagram.rs`)
  calls `centred_hint` (`folder/mod.rs`), which is `ui.centered_and_justified`
  around `widgets::hint`. Split draws the same `diagram_area` in its central
  panel right of the `folder-split-code` panel; in the mock-up the board
  side is about 590 points wide at 960 × 540 and 1230 at 1600 × 1000, with no
  library or inspector panel while no file is open.
- The mock-up adds `folder/dog.rs` (≈ 180 lines): the SVG's cubic Béziers
  sampled into `Shape::convex_polygon` and `Shape::line`, ellipses as
  48-point polygons, the one non-convex shape (the curled tail) filled as
  three convex pieces under its outline, the letters painted with
  `theme::bold_family()` and an eight-offset outline. `hint_with_dog`
  replaces `centred_hint` for the no-file case only: dog box
  (`dog::size_for(area_height)`, 32 % of the board's height clamped to
  220–280 points wide), 12-point gap, then the hint in a `scope_builder`
  with a centred top-down layout.
  Strokes scale with the box, so the dog is sharp at double scale. The
  `feDropShadow` filter has no painter equivalent and is left out.
- Why the painter and not the SVG or a bitmap: the application has no SVG
  renderer (`resvg`/`usvg` would be a new dependency of several hundred
  kilobytes for one picture); a PNG rasterised from the SVG would need a
  tool none of the toolchain has, a second bitmap for double scale and
  ≈ 60 KB in the binary, and it would still substitute the letters' font. The
  painter version is about the size of this ticket and needs nothing new.
  Consequence for the owner: the dog looks exactly like the mock-up, at
  every scale, and differs from the SVG in the letters' font and the missing
  soft shadow.
- The owner's drawing is kept with the sources as
  `crates/apiforge-desktop/assets/reksio.svg` with a comment on its origin
  (drawn for API Forge, 2026-10-26), so the code can be re-derived; the
  owner supplied it, no third-party licence applies.
- Decision 1 = B would add a time-driven transform per animated group
  (`ui.input(|i| i.time)` into the SVG's keyframes), `request_repaint` on
  every frame while the empty board is drawn, and a switch the harness sets
  (`settle` fails after 15 s when repaints never stop, see
  `testing::settle`). egui does not read the OS reduced-motion setting.
  Estimate under B: M, 30 min review.
- Tests: `folder/acceptance_tests.rs`; a `pub(crate)` accessor for the dog's
  box of the last frame (`None` when not drawn) gives criteria 1–4 their
  measurements through `assert_within_window` and `assert_disjoint`-style
  checks against the hint's `rect()`. `testing.rs` keeps a `review_` view of
  the empty Diagram and Split.
