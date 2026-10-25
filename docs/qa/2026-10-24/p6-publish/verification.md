# Stage II P6b: component publication

Branch: `feat/stage-ii-p6-publish`. Base: unmerged P6a `aa3c356`
(`feat/stage-ii-p6-ui`); publication extends that window and session API.

Publication uses one existing roll-forward transaction for the exact working
source, new canonical immutable revision, registry and working/published
checkpoints. The published definition becomes visible only after commit. Only the
publishing document advances its base revision, including recovered drafts of the
same component. Existing scenarios retain their pins and published bytes remain
unchanged. A missing pin has an explicit source fix in Problems and text Undo.

Unattached comments require a decision before graphical semantic editing,
model-history projection or publication. Layout and exact source saving need no
decision. Acceptance is bound to the source version; the native dialog also
checks the analysis ticket so changed dependencies or a reopened session cannot
reuse an old decision. Cancelling leaves source and revision registry intact.

Ten publication/session regressions cover successive revisions, immutability,
old instances, exact working formatting, genuine recovered drafts with the same
entity ID, identity/layout after restart, new components, pin fixes, comment
consent invalidation, harmless layout and external edits of unchanged source.
Three window regressions use pointer/keyboard events for Publish/Cancel/Continue,
text Undo of the pin fix, and stale-dialog refusal.

Native Linux review used the actual WGPU window under Xvfb/Mesa Lavapipe:
[component at 960 × 540](component-960.png) and
[comment decision after an XTest pointer click](decision-960.png).
The dialog retains visible original source and reachable Continue/Cancel controls.
The header was also inspected at 1600 × 1000. This is not a screen-reader audit.

Verification uses Rust 1.99.0, formatting, all-target workspace clippy with warnings
denied, all-target workspace tests serially with isolated config/data directories,
and the converter fixture check. Language smoke fuzzers each run for 60 seconds:
syntax seed `1592639710`, 2,714,782 cases; semantics seed `2718281828`, 42,909 cases
(6,466 successful lowerings). Neither reported a failure.

The separate UI quality gate runs with `CARGO_TARGET_DIR=/tmp/apiforge-gate` to
avoid contaminating the repository's build cache. All ten probes passed ([log](ui-quality.log)). The standard workspace suite
passed 821 tests, with two parent-launched helpers ignored and no failures.

Publication I/O remains synchronous until P7's storage worker. Native transaction
failure dialogs, folder CLI, runs/reports and startup activation belong to later
packages. Windows QA and Windows directory durability remain open release gates.
