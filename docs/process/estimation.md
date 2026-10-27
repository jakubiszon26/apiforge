# Estimation

An agent's hours are cheap; the owner's review time and the risk of a wrong
product decision are not. An estimate therefore has three parts, and all three
are recorded in the ticket before the owner approves it and compared with what
happened when the ticket closes.

## Size

Size says how hard the ticket is to get right and to accept. It is judged on
four questions, not counted in lines:

| Question | Raises the size when |
| --- | --- |
| Complexity | The change crosses a crate boundary, touches a state machine or a protocol (sessions, transactions, the watcher, checkpoints), or needs a new concept |
| Uncertainty | The cause is not known, two designs are open, or the analysis says "probably" |
| Regression potential | Existing behaviour the owner relies on can break without a test noticing: storage, migration, the file format, run semantics |
| Verification cost | Proving it right needs platform runs, timing, real data, or several scenarios at the window |

| Size | Meaning |
| --- | --- |
| `S` | One answer to the four questions is "a little", the rest "no". One sitting to review. A one-line fix in checkpoint sealing is **not** `S`; a 1 000-line animation with no state is |
| `M` | Complexity or regression potential is real and bounded; the analysis names the risk and the test that covers it. The largest change the owner accepts in one session |
| `L` | Two or more questions answer "yes", or the owner would need more than one session. Not scheduled: split into `S`/`M` tickets that each deliver something the owner can see or a test can prove |

Changed lines are **recorded, not limited**. The pull request states them
(without generated fixtures, lock files and captures), the retrospective reads
them, and a count far above what the size suggests is a question to answer in
the ticket, not a reason to split. In the KTLO sprint only a quarter of the
changed lines of a code pull request was source code; the rest was the
acceptance tests, the QA report and the ticket file that the process itself
asks for. A rule that counts them discourages exactly that work.

## When to split

A ticket is split when **the owner could not accept it in one session**, or
when two parts carry different risk and the safer one should not wait for the
other. It is not split because a line count was crossed. Each part delivers
something the owner can see or a test can prove; a part that exists only to
make the other smaller is not a part.

## Owner review minutes

How long the owner needs to read the QA report and, if they choose, build and
use the change. Typical values:

| Review | Minutes |
| --- | --- |
| Documentation or internal change with no visible effect | 10 |
| Visible change covered by a report with captures | 20–30 |
| Visible change the owner will want to use locally | 45–60 |
| A decision ticket (format, migration, default) | 30 plus the discussion |

With three to five review sessions a week, the sum of review minutes of the
tickets in progress is the real capacity of a sprint. Plan against it, not
against agent time. The [work-in-progress limit](README.md#limits-that-keep-review-possible)
is expressed in these minutes.

## Risk flags

A flag names a reason the ticket may need an owner decision or may be
rejected at acceptance. A ticket with any flag has a ticket file and an
analysis the owner approves before code.

| Flag | Raised when |
| --- | --- |
| `format` | Anything Git tracks in a project changes: `.afs`, manifest, checkpoint files |
| `migration` | User data is converted, moved or read by a new loader |
| `default` | What happens without flags or settings changes |
| `visible UI` | A screen, panel, menu, dialog or shortcut changes |
| `removal` | A feature or screen disappears or is hidden |

## Recording

In the backlog row: `S · 20 min · visible UI`. In the ticket file, the same
line plus, at close, the actual values, all three of them:

```text
Actual: M (regression potential in the watcher) · 25 min · 1 round · 633 lines
```

The actual review minutes are the owner's, asked for at acceptance and written
by whoever closes the ticket; a ticket closed without them has no actual. The
[retrospective](retrospective.md) reads these columns.

## When an estimate is wrong

An estimate that proves wrong by a class is not a failure; it is the signal
the retrospective looks for. The ticket is re-approved or split at the point
it is noticed, with the reason in the ticket, not finished and explained
later.
