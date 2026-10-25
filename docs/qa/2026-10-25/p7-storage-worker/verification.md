# Stage II P7d: asynchronous folder storage

Branch: `feat/stage-ii-p7-storage-worker`. Base: unmerged P7c `fec1a43`
(`feat/stage-ii-p7-recovery`). The package depends on P7c's exact conflict and
transaction decisions, and the earlier protected project switch/export flows.

One persistent serial storage worker receives owned requests. The UI polls a
channel without waiting. Project snapshots are taken when a queued request
starts, after the preceding completion has updated its durable baselines.
Read results do not replace session state; save/recovery completions acknowledge
only their observed versions and preserve newer code, layout, history, fields,
text files and settings. Session identity/epoch rejects results for replaced
projects. Destructive operations temporarily disable editing and display the
shared working dialog; failures return the original form with its error.
Routine source and text saves allow continued typing. A slow-operation indicator
appears after 150 ms, avoiding flicker on ordinary fast operations.

Opening, initialisation, saved view restoration, recent metadata, configuration
validation, source/plain-text saves, automatic protection, publication, explorer
operations, trash, external reload, conflict previews/decisions, transaction
recovery/evidence, preferences and journal review all perform their I/O off the UI
thread. Scanning, source analysis, native choosers/export and the runner retain
their existing worker ownership. The store's shared owner serialises journal
writes with project mutations. Late chooser results are bound to their project
or opening generation; a late plain-text open cannot steal a newer tab selection.

Closing and project switching stop new scans and wait for outstanding storage
and Run cleanup. The writer is released only after protection succeeds, or after
an explicit discard or an exact successful export. Failed protection keeps the
session usable. Keeping the window invalidates pending export approval. Source
analysis is not installed while a session storage completion is pending, avoiding
inconsistent identity/history baselines. An export whose complete recovery bundle
changes before departure requires a new export; success never silently covers
newer work.

Moving dirty or missing plain-text files updates their tab key, widget history
and recovery path. File changes and recovery/backup post-images now share one
transaction, including case-only move stages. An unrelated open draft at an
absent destination blocks the move. A foreign recovery write is refused before
any file moves. Ordinary reads/saves still preserve exact CRLF bytes.

Verification includes controlled slow read/save requests with real egui text
input, queued-save dequeue semantics, writer ownership during close, stale
chooser/export rejection, recovered text membership and dirty text move/history/
restart. Existing folder interaction tests now await durable completion instead
of treating request acceptance as a completed save. Their raw frame helper stays
nonblocking, so the race tests exercise input while storage remains pending.
The UI acceptance click helper likewise waits for asynchronous file opening.

A native X11/Mesa run at 960 × 540 and 1600 × 1000 exercised a local edit, foreign
replacement, candidate review, a second foreign replacement, refused obsolete
confirmation, Escape, fresh review and explicit replacement. The compact regular
project header remains legible. Evidence:

- [Conflict at the minimum size](conflict-960.png)
- [Conflict at the larger size](conflict-1600.png)
- [Obsolete candidate refusal](conflict-stale-960.png)
- [Resolved file and refreshed status](conflict-resolved-960.png)

A `strace -ff -yy` audit of that native process covered file operations, reads,
writes, sync and flock. Across 40 traced threads, the main/UI thread made zero
calls whose pathname or annotated descriptor referenced this project or API
Forge's isolated configuration/data files. Managed file calls occurred on the
storage/scanning threads. This is evidence for the exercised path, supplemented
by the production folder I/O call-site review; it is not a universal operating
system trace assertion.

Final verification of implementation `db9c590`: formatting and workspace/all-target
clippy with warnings denied passed; 901 workspace tests passed across 47 test
binaries with two process helpers ignored, serially under isolated XDG paths.
The v1 fixture check and all 10 isolated UI acceptance probes passed. Release layout budgets passed: layered 0.32/20 ms, full
routing 25.28/30 ms, drag rerouting 0.20/8 ms, smoothing 0.26/8 ms and placing
0.06/8 ms. Random-board routing took 767 ms and has no strict budget. No language
implementation changed, so the language fuzzers were not rerun for this package.

Windows native UI and directory durability, actual Wayland parenting, complete
screen-reader operation, source/editing performance budgets, portability review
and normal startup activation remain their explicit later gates. This package
does not activate folder startup or automatically migrate a legacy workspace.
