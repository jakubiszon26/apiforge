# Stage II P6d: folder runs, reports and close export

Branch: `feat/stage-ii-p6-runs`. Base: unmerged P6c `bea2441`
(`feat/stage-ii-p6-cli`), whose saved loader, closed snapshots and shared journal
owner are prerequisites.

Run settles pending source, freezes the selected scenario, its exact published
component closure and public configuration, and supplies session secrets
separately. Valid unsaved source can run. An invalid source or unresolved inspector
field needs consent to a verified earlier snapshot. The candidate retains the
configuration it originally used; changing source, configuration, fields or the
session invalidates consent. A missing candidate or conflict blocks Run.

Reports remain associated with their document and appear in Code, Diagram and
Split. Edits mark them stale and stop their outputs being used for completion,
inspector actions or board status. Graph/inspector edits still use the shared
history and comment-loss decision. Running snapshots remain immutable. Report
selection follows main/cleanup; comparison, variable assignment and export reuse
the existing report/graph services.

Project journals retain the writer lock and deliberate acknowledgement. Closing
cancels an active run and waits without blocking for cleanup before releasing the
session. A writable project with an unavailable journal cannot send live requests;
a read-only session requires the existing explicit unjournalled-live warning.
Unresolved pre-migration runs have a separate global review path under the legacy
workspace lock and are never assigned to a project by guessing scenario IDs.

Close failure offers Export and close. Its version-one JSON bundle contains the
public manifest and exact in-memory source/text recovery, selections, field drafts
and checkpoints. It includes dirty configuration, which source recovery alone
cannot retain, and excludes session secret values. The export uses atomic durable
writing on a file-dialog worker. Failure, cancellation or work changed during
export keeps the session open. A successful current export allows close without
rewriting a foreign or failed project recovery file.

Five library tests cover frozen inputs, settling, consent invalidation, earlier
configuration after restart, missing candidates, conflicts and exact/public
export. Six new window regressions cover pointer-driven Run and last-valid
consent, reports in all modes, staleness, export failure/cancellation/versioning,
failed recovery export, project/global acknowledgement and nonblocking live-run
cancellation with writer-lock lifetime through cleanup.

Native Linux review used Xvfb and Mesa Lavapipe with XTest input:

- [Report at 1600 × 1000](report-1600.png).
- [Report at 960 × 540](report-960.png).
- [Last-valid consent after an external edit](last-valid-960.png), with the
  earlier report visibly marked stale.
- [Read-only close after typing and a native close event](export-close-960.png),
  with reachable export/keep/retry/discard controls at the minimum window size.

The filesystem save dialog itself was not automated in this native probe; the
completion and durable-write paths are covered by tests. This does not constitute
Windows or native screen-reader QA. GUI project/journal loading and ordinary
project writes still await P7's storage worker. Startup activation and automatic
migration remain disabled until P7.

Verification on Linux (2026-10-24), Rust 1.99.0: formatting, all-target workspace
clippy with warnings denied, converter fixture checks and serial all-target
workspace tests passed: **843 passed, two parent-launched helpers ignored**.
Config/data directories were isolated. The UI quality gate passed **10/10**, with
`CARGO_TARGET_DIR=/tmp/apiforge-gate`. No grammar or layout algorithm changed.
