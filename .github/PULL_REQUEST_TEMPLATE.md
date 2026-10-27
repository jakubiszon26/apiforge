## Ticket

Closes #<!-- issue number -->

<!-- ID and title, e.g. KTLO-05b: Block library beside the board; link docs/tickets/<ID>.md if it exists -->

- Estimate: <!-- S · 20 min · visible UI --> · Actual: <!-- lines · minutes · rounds -->
- Analysis model: <!-- Fable --> · Implementation model: <!-- Opus 5.5 -->
- Owner decisions this change depended on: <!-- ticket section, or "none" -->

## What the owner will see

<!-- New / changed / removed, in the owner's words. Link the approved mock-up if there was one. -->

## QA report

<!-- docs/qa/<date>/<ticket>/report.md — captures per platform and the "not verified" section -->

## Not verified

<!-- Copied from the report so it is visible here -->

## Checklist

- [ ] Branch starts at `main`; one ticket, one pull request from analysis to QA report; changed lines stated above and the size re-approved if it proved wrong by a class
- [ ] Acceptance tests were committed before the implementation
- [ ] `cargo fmt`, clippy with `-D warnings`, the workspace tests and the fixture check pass
- [ ] Interface change: interaction tests and design review per `docs/gui-testing.md`
- [ ] Regression smoke list run on the release binary; results in the report
- [ ] No `#[ignore]` without a ticket ID, other than a design-review view or a spawned helper
- [ ] WIP after this pull request: at most four awaiting acceptance, within 60 estimated review minutes
- [ ] The issue is labelled `awaiting-acceptance` and `docs/backlog.md` still lists it
