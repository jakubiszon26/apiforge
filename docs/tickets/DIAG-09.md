# DIAG-09: Add comparison and Assign to variable work before the first run

- Size: S · Owner review: 10 min · Risk: `visible UI`
- State: **Approved, ready for implementation** (issue #399) · taken into
  0.2.1 by the owner on 2026-11-21, with DIAG-08 (#521); **merges before
  DIAG-08**, which changes the same button · labelled `ready:opus`
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

Before a scenario has ever run, the Inspector's Output data shows the
step's sample with both buttons enabled, and a click does nothing and
says nothing (#399). A click then does what it does after a run.

## What the owner will see

- Changed: on a step that has not run, *Add comparison* on a field of
  the sample writes the check, and *Assign to variable* the variable,
  exactly as after a run. The value a comparison expects is the
  sample's.
- Unchanged: the buttons after a run; the hint above the tree, which
  already says where the data comes from. Removed or hidden: nothing.
- Mock-up: not applicable — two existing buttons start to work.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/diag09.rs`.

1. On the demo shop never run, *Add comparison* on a field of a
   request's sample adds one check of that field under the step; the
   file passes `check`. `diag09_criterion_1_comparison_before_a_run`.
2. *Assign to variable* on the same field adds the variable.
   `diag09_criterion_2_variable_before_a_run`.
3. The same on a step of the cleanup. `diag09_criterion_3_cleanup`.
4. On a read-only or observing window both buttons are disabled with a
   hover that says why; they are never enabled and silent.
   `diag09_criterion_4_never_silent`.
5. After a run the buttons do what they did. The existing tests of
   `AssertField` and `AssignField` pass unchanged.

## Not in scope

The menu of DIAG-08. A component call's output before a run when the
component has no sample to show (the tree is empty there today).

## Technical appendix

`folder/diagram.rs` near line 1188: `inspector::Action::AssertField` and
`AssignField` are sent on only `if let Some(run)`;
`runs.rs` `folder_report_action` takes the run but uses it for these two
actions only to tell the cleanup from the main flow, which the graph
that holds the node also tells. Regression: the count of checks after an
edit (`PendingCheck`), covered by criterion 5.
