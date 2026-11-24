# ADR 0016: Runs of a part of a scenario

- Status: accepted 2026-11-24 (REQ-00, REQ-04a; an internal design)
- Date: 2026-11-24
- Scope: `apiforge-runner` (`partial.rs`, `request.rs`, the engine's
  selection, gate and Cleanup condition)
- Related: [run after an error](0015-run-after-an-error.md),
  [run journal](0009-run-journal.md); tickets REQ-04a (#599), REQ-04b
  (#461), REQ-05 (#462)

## Context

*Run to here* and *Send alone* need runs the engine did not have. A run to
`target_node_id` selects what the target depends on, so it leaves out every
check; a run of `single_node_id` runs the node and then the whole Cleanup.
The recorded run corpus freezes both, and the workspace window uses them.

## Decision

`RunRequest::partial` names one of two runs. The older fields keep their
behaviour and exclude it (`ConflictingModes`); both runs are `diagnostic`.

**`Partial::Through { node_id }`** selects a prefix of
`graph::sequence`: up to the node and the data statements the engine
reaches before the next step. A prefix holds everything its nodes depend
on, so every rule of a full run (the order, the gates, the error policy,
branches) applies unchanged, and each result is the full run's. The whole
Cleanup runs after it; a Cleanup step that reads a step the part left out
is skipped and names it, as after a stopped run.

**`Partial::Alone { node_id, previous }`** selects the node and the checks
whose subjects include it (`graph::check_subjects`). There is no Cleanup.
Every other value is read from `previous`, an earlier report validated as
`previous_run` is; the results of the selected nodes themselves are not
taken from it. `previous_run` stays for `single_node_id` alone.

1. Nothing else runs, so nothing is ordered: the request and its checks
   wait for the values they read and for nothing more, under either error
   policy. A value exists when its statement made the output (ADR 0015 rule
   3). One that does not exist blocks its reader and names the statement;
   a false Data condition still guards the steps that read it.
2. The request is on the path by being asked for, whatever its branch, and
   so is a check that stands in the same braces. In other braces a check
   follows what their `if` decided in `previous`: outside the path in the
   branch not taken, blocked by the `if` when it did not decide. A missing
   value is named before the `if`. Otherwise checks of both arms of an `if`
   that reads the response would be judged, and one would always fail.
3. A failed check does not stop the others: the policy *Stop* protects the
   steps that follow, and none does.
4. The runner does not restrict the node's kind, as it does not for
   `single_node_id`; what the window offers is its own rule. The steps
   inside a component call follow the rules of every run.

## Consequences

- `previous` is redacted: a value it holds as `[REDACTED]` is read as that
  text, by the request and by a check's other operand. The caller refuses
  a live send that would carry one (REQ-05).
- A secret learned only from an earlier step's response is not known to a
  run of one request alone, which cannot mask it, as with `single_node_id`.
- A value made from the response (`total = add.body.unit * 2`) is not
  computed again: a check that reads the response and such a value reads
  the value of `previous`, and a check that reads only the value does not
  run.
- A journalled run of one request alone that is interrupted says that its
  Cleanup did not start, which is true.
- A report of either run holds only the steps it selected; the window words
  the ones left out (REQ-04b).
