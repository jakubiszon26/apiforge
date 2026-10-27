# KTLO-23a: A component runs from its tab — runner, session and the Run button

- Size: M · Owner review: 45 min · Risk: `visible UI`
- State: Awaiting acceptance (issue #53; branch `feat/ktlo-23a-component-run`)
  — approved 2026-10-27 (mock-up and decision 6)
- Actual: about 1570 changed lines without captures — over M (≤ 800), by
  the tests: code 358, tests 1022 (desktop 600, runner 350, project 73), QA
  report 182, this ticket 6; one round
- Analysis and acceptance criteria: `docs/tickets/KTLO-23.md`
- Analysis model: Fable · Implementation model: Opus 5.5

## Delivery

The runner gains a component target, the project session prepares a run for
a component document, and the folder header shows *Run* and *Run options*
on a component tab. Inputs come from the defaults written in code; an input
without a default fails its step as "Missing required input" until KTLO-23b
adds the dialog (decision 1). The header reserves 650 px for the toolbar on
every tab instead of 620 (decision 6), and the long-name header test
`long_project_names_do_not_overlap_header_actions_in_writable_or_read_only_windows`
gains a component-tab case.

The fixture for criterion 2 is a component whose inputs all have defaults:
the demo's `add-to-cart` has `product` without one, so under this ticket
alone its run fails with "Missing required input: product"; the demo's
`Product` component (`id` = 123) is a candidate.

Criteria covered: KTLO-23 criteria 1–8, 10 and 11, each as an interaction
test in `folder/acceptance_tests.rs` or a runner test in `apiforge-runner`.
Criterion 8 only with decision 3 = A.

## Not in scope

The *Run inputs* dialog (KTLO-23b). The command line.
