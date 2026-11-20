# LANG-20e: A commit of the project store refuses when something it only read has changed

- Size: S · Owner review: 10 min · Risk: none (no file's format changes;
  every commit of today passes an empty set)
- State: **Decided; labelled with the ruling of the independent review.**
  Split from LANG-20b on 2026-11-20. After LANG-13 (#386) only.
- Issue [#500](https://github.com/jakubiszon26/apiforge/issues/500) · LANG
  sprint, first half · branch `feat/lang-20e-store-read-set`
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

A caller that decided what to write from what it read can commit under
that reading: when any file it read has changed, nothing is written.

## What happens today

`apiforge-storage/src/project/transaction.rs`: a commit compares the
expected state of the files it **writes**, when it takes the lock and
again after staging, before activation; the manifest is compared with
its state when the commit began. A file the caller only read is not
compared at all.

## What the owner will see

Nothing by this ticket. LANG-20b's update is its first caller.

## Mock-up

Not applicable.

## Acceptance criteria

Tests in `crates/apiforge-storage/src/project/transaction_tests.rs` and
`crates/apiforge-storage/tests/acceptance.rs`.

1. A commit takes, beside its changes, a **read set**: paths with the
   state the caller saw (a fingerprint, or *missing*). A path may be in
   the read set and not among the changes.
   Test: `lang20e_criterion_1_*`.
2. The read set is compared **twice**, as the written files are: when
   the commit takes the lock, and after staging, immediately before
   activation. A mismatch at either point refuses the commit, writes
   nothing, leaves no journal to recover, and names the path. Tests:
   `lang20e_criterion_2_*`: a read-only path changed before the lock;
   changed during staging (the fault hook between staging and
   activation); the manifest read and not written, changed; a path
   expected missing that appeared.
3. A path in both the read set and the changes is compared once, by the
   change's expectation; the two must agree or the commit is refused as
   malformed. Test: `lang20e_criterion_3`.
4. Every commit of today passes an empty read set and behaves as before.
   Test: the storage suites unchanged (`transaction_tests`,
   `fault_tests`, `recovery_tests`, `conflict_tests`).
5. Recovery of an interrupted commit is unchanged: the read set is
   checked before activation only and is not part of the journal.
   Test: `lang20e_criterion_5`.
6. ADR 0011 says so. QA step: read in the pull request.

## Regression potential

Every write of a session goes through this function: criterion 4. The
commit's limits (64 MiB, 1 024 operations) do not count the read set;
its size is bounded by the number of files of a project.

## Not in scope

Who passes a read set and what it means (LANG-20b).

## Technical appendix

`apiforge-storage/src/project/transaction.rs`, `mod.rs` (the error),
ADR 0011.
