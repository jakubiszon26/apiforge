# ADR 0003: Shared arguments do not activate unrelated requests

- Status: accepted
- Date: 2026-10-19
- Scope: `apiforge-runner`

## Context

The React prototype traversed both successors and pure argument producers when
computing reachability from Start. Pulling in a shared variable consequently made
all its consumers reachable, including an HTTP request with no connection to the
active workflow. Preserving this bug would violate the user's requirement that
Start and request hierarchy determine which side effects run.

## Decision

The native runner distinguishes active successors from required arguments. It
follows data and ordering successors from Start as before. Pure prerequisites of
those nodes are included recursively, but their other consumers are not activated.
A value explicitly reached on the active path still activates its successors.
Legacy graphs without Start keep their existing behavior. IF branch selection
continues to happen in the execution gate; this change does not activate a branch
or change dependency ordering.

This is an intentional correction relative to the React prototype, rather than a
compatibility regression. A live loopback HTTP regression test verifies that only
the connected request is sent when it shares an argument with a disconnected one.
The existing simulation corpus, including IF cases, continues to pass unchanged.
