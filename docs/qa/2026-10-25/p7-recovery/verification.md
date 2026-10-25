# Stage II P7c: deliberate transaction and file-conflict recovery

Branch: `feat/stage-ii-p7-recovery`. Base: unmerged P7b `1311f42`
(`feat/stage-ii-p7-settings`). P7c uses its project navigation/settings and the
earlier protected close/export flows.

An interrupted project opens a recovery dialog before definitions are loaded.
The same review is available when opening a recent folder or when the watcher
detects an interrupted operation in the current session. The current project and
its work remain owned until recovery and the ordinary protected switch succeed.
While current-project recovery is unresolved, the window retains its documents
in memory and offers export, rather than ordinary execution or diagram changes.

The dialog shows original intent and available current/intended candidates.
Finish uses storage's immutable preview: changed head, descriptor, approval receipt
or target bytes require another preview and decision. Foreign states are preserved
under `.apiforge/local/conflicts/` before replacement. Read-only sessions inspect
and export but cannot finish another writer's operation. An inaccessible target
blocks completion. Damaged intent stays untouched and can be exported as a bounded
version-one evidence bundle: relative names, exact unsigned byte arrays and
fingerprints, plus names of unavailable entries. It is evidence for manual
recovery, not an executable replacement plan. Links are never followed; an
oversized export fails instead of truncating input.

Source and project-text conflicts have their own modal review and exact candidate
exports. Confirmation binds session/epoch, public context, source version, field
drafts/model/layout and the stable observed disk generation/bytes. A newer local
edit, foreign replacement, settings change or reopened session invalidates it.
Keeping a deleted file also rechecks absence; a reappearing foreign file cannot
be treated as missing. Graphical edits/history are blocked during a source
conflict. Accepting a text-file deletion closes its clean tab without recreating
an empty file. Export outcomes are visible inside the dialog and never approve a
conflict decision. Resolved conflicts replace the earlier saving-paused notice.

Reload after recovery preserves the latest in-memory source, fields, layout and
ordinary text, including edits made before automatic recovery was written. A
changed baseline leaves a draft for explicit acceptance. Unsaved public settings
require export or an explicit discard before reload. The temporary recovery
writer is released before opening a normal project owner.

All new dialogs use the shared settings dialog chrome: an 18 px semibold title,
regular body text, scrolling content, a close control and a stable footer.
Escape/Cancel preserves the decision; Ctrl+Enter invokes only an enabled primary
action. The compact 13 px regular project header from P7b remains in use.

Native Linux review used the real egui window, XTest input, Xvfb and Mesa:

- Transaction review at [960 × 540](transaction-960.png) and
  [1600 × 1000](transaction-1600.png).
- A real isolated D-Bus/GTK portal [save dialog](export-file-chooser.png) exported
  the foreign candidate. The probe checked every output byte and that head still
  named the active transaction. The [result stays visible](export-result-960.png).
- Source conflict review at [960 × 540](conflict-960.png) and
  [1600 × 1000](conflict-1600.png), followed by another foreign write.
  Ctrl+Enter [rejected the old preview](conflict-stale-960.png), with Refresh
  still fully visible in the fixed footer. Escape [kept the local source](conflict-cancelled-960.png);
  reopening review and confirming [loaded the new disk version](conflict-resolved-960.png)
  and cleared the earlier saving-paused notice.
- Explicit Finish [completed the operation](transaction-finished-960.png), with
  the exact foreign source verified in its conflict copy. Ctrl+Enter then
  [opened the recovered project](transaction-reopened-960.png); the probe verified
  the intended source and that the new project session retained its writer lock.

The isolated file portal was started from packages unpacked under `/tmp`, without
system installation. Document/permission-store and PipeWire services were absent;
the FileChooser interface was available and the export completed. This is not
native Wayland, window-manager parenting, a screen-reader audit or Windows QA.

Ordinary preview/finish/reload and project/journal I/O still move to the storage
worker in P7d. Binary/unreadable targets are exportable only where storage can
obtain bounded safe bytes; inaccessible or oversized originals remain on disk for
manual recovery. This package does not activate automatic startup migration.
Windows QA and Windows directory durability remain open release gates.

Verification at code commit `5977079` passed with Rust 1.99:

- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Serial all-target workspace tests with isolated config/data directories:
  **880 passed, 0 failed, 2 helper tests ignored** across 46 binaries. P7c adds
  nine session conflict/storage-boundary regressions, three opaque-evidence
  tests and ten egui recovery/export/geometry tests.
- `cargo run -p apiforge-convert-v1 --locked -- --fixtures --check`.
- `CARGO_TARGET_DIR=/tmp/apiforge-gate tools/qa/ui-quality/run.sh HEAD`: **10/10**.
- Strict release layout benchmark: layered layout 0.32/20 ms, full routing
  24.56/30 ms, dragging 0.20/8 ms, smoothing 0.26/8 ms, placing eight cards
  0.06/8 ms. Random routing took 776 ms with no defined budget.

No language implementation changed; sustained language fuzzers were not required.
