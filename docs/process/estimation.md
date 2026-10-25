# Estimation

An agent's hours are cheap; the owner's review time and the risk of a wrong
product decision are not. An estimate therefore has three parts, and all three
are recorded in the ticket before the owner approves it and compared with what
happened when the ticket closes.

## Size

Size is the changed lines of the pull request, excluding generated fixtures
and lock files, including tests and documentation.

| Size | Lines | Meaning |
| --- | --- | --- |
| `S` | ≤ 300 | One sitting to review; no design risk |
| `M` | ≤ 800 | The largest change that can be reviewed in one session |
| `L` | > 800 | Not scheduled. Split into `S`/`M` tickets that each deliver something the owner can see or a test can prove |

A pull request that outgrows its class is split before the QA report, not
explained in it. Three `M` tickets reviewed separately are faster to accept
than one `L` nobody reads; Stage II's 40 000 lines were accepted by nobody.

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
against agent time.

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
line plus, at close, the actual values: `Actual: 412 lines · 35 min · 1 round
of changes`. The [retrospective](retrospective.md) reads these columns.

## When an estimate is wrong

An estimate that proves wrong by a class (an `S` that became an `M`) is not a
failure; it is the signal the retrospective looks for. The ticket is split or
re-approved at the point it is noticed, not finished and explained later.
