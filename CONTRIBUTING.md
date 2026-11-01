# Contributing to API Forge

API Forge is licensed under the GNU General Public License, version 3 or any
later version ([`LICENSE`](LICENSE), `GPL-3.0-or-later`), copyright The API
Forge contributors. A contribution is accepted under the same licence; there
is no contributor licence agreement.

## Sign off your commits

Every commit of an external contribution carries a `Signed-off-by` line, by
which its author certifies the
[Developer Certificate of Origin 1.1](https://developercertificate.org/): that
they wrote the change, or otherwise have the right to submit it under the
project's licence, and that the contribution and the sign-off are public.
Git adds the line with `-s`:

```bash
git commit -s -m "fix(cli): …"
```

The line names a real person and an address they can be reached at:
`Signed-off-by: Jane Doe <jane@example.com>`. A pull request whose commits are
not signed off is not merged.

## How work is done here

The same rules apply to people and to coding agents:

- [AGENTS.md](AGENTS.md): the rules that are never bent.
- [Development process](docs/process/README.md): tickets, who decides what,
  acceptance tests before code, the QA report, and the limits on work in
  progress.
- [Development guide](docs/development.md): toolchain, the verification every
  change passes, branches and pull requests, conventions.

Start from an open issue of the current sprint ([backlog](docs/backlog.md)),
or open an issue first for anything that changes what a person sees, a
default, compatibility or the project file format: those are the owner's
decisions. Only the owner merges to `main`.
