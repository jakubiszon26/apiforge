# REQ-03: Headers as a table; authorization chosen from a list

- Size: M · Owner review: 15 min · Risk: `visible UI`
- State: **Decided** (2026-11-19, 9 = A; [REQ-00](REQ-00.md)). Mock-up
  approved ([Headers, Auth](REQ-00.md#mock-up)). After REQ-02a.
  Issue [#460](https://github.com/jakubiszon26/apiforge/issues/460).
- Analysis model: Fable 5.2 · Implementation model: Opus

## Goal

Adding a header or an authorization to a request is one row.

## What the owner will see

- New: Headers as rows of name and value; *Auth* with None, Bearer
  token, Basic and Free header, showing the header it writes.
- Unchanged: the `.afs`, `headers { … }` in each request.

## Acceptance criteria

Tests in `folder/acceptance_tests/req03.rs`.

1. A row added, changed or removed writes only that header; the others
   keep their text and order. `req03_criterion_1_*`
2. *Bearer token* with a value writes `Authorization: "Bearer …"`,
   *Basic* the encoded pair; the tab shows the line it writes; *None*
   removes that header and no other. `req03_criterion_2_*`
3. An `Authorization` that fits no form is shown as *Free header*,
   unchanged, and choosing nothing writes nothing.
   `req03_criterion_3_*`
4. `Authorization` is edited in *Auth*; its row in Headers is read-only
   and leads there. `req03_criterion_4_*`
5. A literal token shows the note that a secret belongs in the
   environment; Run results still read `[REDACTED]`.
   `req03_criterion_5_*`
6. The tab's count (*Headers 2*) is the number of rows; an empty name
   is not written. `req03_criterion_6_*`
7. QA step: REQ-06 part 2 step 4 — an authorization added to *No token*
   and removed leaves an empty diff.

## Captures the owner judges on the pull request

*Basic*; a *Free header*; the read-only row; both tabs at 960 × 540.

## Not in scope

Headers shared by a scenario (LANG-18 #471); the button drawn in M3c.

## Technical appendix

- Files: new `steps/headers.rs`, `steps/auth.rs`; `steps/request.rs`
  (the two slots); the test file. The Inspector's headers field is not
  changed.
- Regression: redaction (`apiforge-runner/src/redact.rs`, criterion 5);
  the Inspector and the table write the same text (REQ-02a criterion 2
  extended to headers).
