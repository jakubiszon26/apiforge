# ADR 0009: The run journal and the categories of a transport failure

- Status: accepted; Linux verified, native Windows verification pending
- Date: 2026-10-23
- Scope: `apiforge-core`, `apiforge-runner`, `apiforge-storage`, desktop
- Milestone: the run journal release gate of the [code and canvas plan](../roadmap.md)

## Context

A live run sends real requests, so it can change the tested API. The report says
what happened — but only for a run that reaches its end. A process that dies while
a live run is in progress leaves nothing: no report, no notice, and no way for the
user to learn that an order was created and never deleted, because cleanup never
ran. Cancellation is already reported; an interrupted process was not.

Two failures were also indistinguishable in reports. A request that fails before
anything is sent (an unresolvable host) and a request that is sent without an
answer (a timeout) both appeared as `Request failed: …`, although only the second
can leave a change behind. `reqwest` reports name resolution, TLS and TCP setup as
one `connect` error, so reports could not name the cause either.

## Decision

### The journal

`journal.json` lives beside the workspace, under the same directory lock as the
workspace and `drafts.json` ([ADR 0008](0008-storage-sessions.md)). It records live
runs that have not reported their own outcome:

- A live run writes its identity, scenario and start time before it may send
  anything. The identifier is the one the report carries.
- Every live request is recorded **before** it is sent, in state `inFlight`, and
  resolved once the outcome is known: `completed` with the response status,
  `notSent`, or `uncertain`. Entering the cleanup phase is recorded too.
- A run that reaches its end removes itself. The report is the record from then on,
  so a journal on disk always describes an interruption.

The journal is therefore not a run history, and it stays small: at most 16 runs of
at most 200 entries, with dropped entries counted. Each change is a full atomic
rewrite that flushes the file and its directory before returning, because a record
still sitting in a buffer records nothing.

A journal found at startup was left by a process that died: the lock rules out a
second live writer. The desktop lists its runs, their requests and whether cleanup
had started, and clears them only when the user marks them as handled. *Remind me
later* keeps them for the next start. A journal that cannot be parsed is preserved
as `journal.unreadable.json` and reported; a journal written by an unknown version
is never rewritten and disables journalling for the session.

**A live request that cannot be journalled is not sent.** A failure before the
request stops the run with nothing executed; an effect that cannot be recorded is
not performed. After the request there is nothing left to refuse, so a failed write
is reported in the run's error and the entry keeps its earlier state, which
overstates the uncertainty rather than hiding it.

Only live runs are journalled: a simulation sends nothing. A session with no
writable workspace — a read-only second window, `--project`, a temporary workspace
— has nowhere to keep a journal that the lock protects. Live runs are still
allowed there, and the window says plainly that they are not recorded.

The journal records what was sent, never what was answered beyond a status. URLs
and failure details pass through the report's redactor, so a secret interpolated
into an address does not reach the file.

### Failure categories

A failed live request now reports a category — the host name could not be resolved,
the TLS connection failed, the connection failed, no response in time, the body
could not be transferred, the response could not be decoded, the request could not
be sent — and whether the API may have performed it:

| Category | The API | Report adds |
| --- | --- | --- |
| DNS, TLS, connection, request | cannot have seen it | *Nothing was sent to the API.* |
| timeout, body, decoding, uncategorised | may have performed it | *The API may have performed this request.* |
| a status, a redirect, a response over a limit | performed it | — |

Cancellation while a request is in flight is in the second group: bytes were
already on their way.

The three connect stages are separated by the cause chain, which belongs to the
HTTP and TLS stacks rather than to this project. Name resolution and a connection
that was never established have stable wordings and are matched against a
documented marker list. What remains is a TLS failure when the address is `https`,
whatever the TLS stack called it: its wordings (`received corrupt message`, a
numbered alert, `peer misbehaved`) share no reliable keyword. On an `http` address
the same remainder stays a connection failure instead of becoming a guess. The URL
is removed from the error before any of this, so no word in an address can decide a
category. Tests assert the categories of real DNS, TCP and TLS failures produced
against loopback, not of strings written in a test.

## Consequences

An interrupted live run is now visible, with the requests it may have performed and
whether cleanup ran. A report says whether a failed request can have changed
anything. A live run costs one small flushed write per request, which is well
inside the cost of the request itself.

What the journal cannot do: it records what this process asked the API to do, not
what the API did. A request recorded as `uncertain` or `inFlight` may have been
performed in full, in part or not at all, and no local record can settle that. It
also cannot run compensating cleanup after a crash; that remains the user's
decision, which is why the dialog reports rather than acts.

## Evidence

Automated tests cover the states an entry passes through in real loopback runs, a
run abandoned mid-request (the crash case, by dropping the run future), cancellation
in flight, a connection that never carried the request, a journal that refuses every
write, a simulation that journals nothing, run identity shared with the report, the
file left behind and found by the next session, an unreadable and an unknown-version
file, read-only refusal, bounded growth, and the desktop's report-and-clear flow.

Native Windows execution remains a release gate, as in ADR 0008; the journal uses
the same atomic write and directory lock as the workspace.
