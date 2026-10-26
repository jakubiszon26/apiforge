# KTLO-23a: A component runs from its tab — runner, session and the Run button

- Size: M · Owner review: 45 min · Risk: `visible UI`
- State: Analysis — waits for the KTLO-23 mock-up; decisions 1–4 taken 2026-10-26
- Analysis and acceptance criteria: `docs/tickets/KTLO-23.md`
- Analysis model: Fable · Implementation model: —

## Delivery

The runner gains a component target, the project session prepares a run for
a component document, and the folder header shows *Run* and *Run options*
on a component tab. Inputs come from the defaults written in code; an input
without a default fails its step as "Missing required input" until KTLO-23b
adds the dialog (decision 1).

Criteria covered: KTLO-23 criteria 1–8 and 10, each as an interaction test
in `folder/acceptance_tests.rs` or a runner test in `apiforge-runner`.
Criterion 8 only with decision 3 = A.

## Not in scope

The *Run inputs* dialog (KTLO-23b). The command line.
