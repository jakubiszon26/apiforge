# KTLO-22: Publishing from the header is sometimes not recorded yet when the test asserts it

- Size: S · Owner review: 15 min · Risk: none
- State: Approved 2026-10-27 (issue #51); cause re-analysed the same day after
  the first fix was measured not to work — see Cause
- Analysis model: Fable · Implementation model: Opus 5.5

## Goal

`folder::tests::publishing_from_the_header_requires_a_comment_decision_and_cancel_keeps_source`
passes or fails on the same commit depending on the run, on Windows so far
(runs 40367559871 on `5cf3660`, a Markdown-only change, and 40410023496). A
red run that is not a regression costs every review, which is what KTLO-21
(#49) was raised for.

## What the owner will see

- New: nothing.
- Changed: a *Continue* in the "Continue without unattached comments?"
  prompt is no longer forgotten when it lands while the window is checking
  the folder for external changes. Today, about once a second for a few
  milliseconds, such a *Continue* silently fails: the prompt comes back and
  nothing is published or applied. After the fix the decision holds, as the
  prompt says it does.
- Removed, hidden, migrated or default behaviour: nothing.

## Mock-up

Not applicable: no interface change.

## Cause

Not what the first analysis said. The test already waits for the storage
worker after every click (`click` ends with `wait_storage`,
`folder/tests.rs:1287`, since `db9c590`); adding more waits changed nothing
(2 of 200 rounds still failed). What is lost is the *decision*, not time.

Once a second the watcher queues an exclusive storage job, "Checking
external changes…" (`folder/mod.rs:591`), even when nothing changed. The job
starts from a snapshot of the session and, when it completes,
`ProjectSession::complete_storage` (`apiforge-project/src/storage_work.rs:68`)
replaces the live session with the job's candidate. For a non-exclusive job
the live `comments_accepted` and `selection` are carried over document by
document (`storage_work.rs:133-134`); for an exclusive job they are not —
`same_storage_inputs` checks that no source changed and then the candidate
wins whole. `accept_comment_loss` (`comments.rs:19`) writes only
`comments_accepted`, so a *Continue* that lands while the watcher's job is
running is wiped out when the job completes; the publication queued behind
it fails with `CommentsWouldBeLost`, the prompt is shown again, no notice
appears, and the manifest keeps its 3 revisions — the CI symptom
(`left: 3, right: 4`, runs 40367559871 and 40410023496).

Measured on Linux (3 cores, other builds running): the unchanged test failed
4 of 200 rounds pinned to one core with two CPU burners, and 3 of 200 with
no added load — a once-a-second timer, not starvation. A deterministic
reproduction needs no timing at all: queue an exclusive no-op job as the
watcher does, run one frame so it starts, call `confirm_comment_loss` and
`wait_storage`; it fails every time. The same loss can hit `selection`.

## Decision

**The live session's per-document decisions survive an exclusive job that
did not change the document.** `complete_storage` carries `comments_accepted`
and `selection` from the live session into the candidate in the exclusive
branch exactly as the non-exclusive branch does, for every document whose
source the job left alone (which `same_storage_inputs` already guarantees
for the whole session). `comments_accepted` is bound to a `source_version`,
so a decision can only ever be carried onto the source it was taken for.
The decision does not travel with the job instead, because every exclusive
job — the watcher's check, a publication, an apply — would then need its
own copy of the rule. The agent's decision (internal architecture); it is
recorded here because it changes what the person sees.

## Acceptance criteria

1. A test in `apiforge-project` shows the loss without timing: take a storage
   snapshot in exclusive mode, accept comment loss on the live session, then
   complete the snapshot's job with an unchanged candidate; before the fix
   `require_comment_decision` fails afterwards, after it it passes. The same
   for `selection`.
2. A folder test shows it through the window: with an exclusive no-op
   storage job in flight, *Continue* in the comment-loss prompt publishes
   (the manifest gains a revision, the notice "Published revision …" shows,
   the prompt does not return). It fails every time before the fix.
3. `publishing_from_the_header_requires_a_comment_decision_and_cancel_keeps_source`
   is unchanged and passes 200 consecutive rounds under load after the fix
   (the loop commands and the counts before and after are in the QA report).
4. An exclusive job that *did* change a document still replaces it whole:
   the existing `complete_storage` tests pass unchanged.
5. The verification commands of the development guide pass.

## Not in scope

The watcher's once-a-second exclusive check itself (its cost and whether a
read-mode probe would do — a question for the backlog's *Later*); the
`settle()` finding there; the Windows runner's speed.

## Technical appendix

- `storage_work.rs:94-97`: the exclusive branch returns `StaleStorage` when
  inputs differ and otherwise falls through to the replacement; the carry-over
  loop of the non-exclusive branch (`:107-135`) is the pattern — copy its
  `else` arm for the exclusive case, for documents and for texts'
  `selection`.
- The first analysis of 2026-10-27 named the missing `wait_storage` as the
  cause; the implementation's measurement (4 of 200 and 2 of 200 with the
  waits) refuted it before any commit. Kept here as the record of the wrong
  turn.
