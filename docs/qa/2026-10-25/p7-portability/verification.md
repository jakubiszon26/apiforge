# Stage II P7e: Git portability and external replacement

Branch: `feat/stage-ii-p7-portability`. Base: unmerged P7d `84e8177`
(`feat/stage-ii-p7-storage-worker`). Implementation: `a259bb8`, `a9f14cf`.

B7 retains one deterministic `.apiforge/diagram.json`. The real
`git merge-file -p ours base theirs` test begins with two saved scenarios and
full checkpoint records. Separate copies change each scenario's source and
semantic digest, main/cleanup positions and variable-proxy identity. Git merges
cleanly; the resulting file has exactly two records, and a saved-only reopen
produces both complete expected models, including all IDs, layout, proxy and
cleanup data. This supports the accepted granularity for independent document
edits. It does not promise clean merges of concurrent changes to the same
record, record deletion/insertion or public-context changes. No format, path or
hash version changes are introduced.

B8 appends these narrow rules during explicit initialisation and model-backed
project preparation, including migration:

```gitattributes
*.afs text eol=lf
/apiforge.project.json text eol=lf
/.apiforge/diagram.json text eol=lf
```

Existing `.gitattributes` bytes remain an exact prefix, with one separator newline
when needed. A complete existing suffix is not duplicated. Existing source bytes
and the Git index remain unchanged; no renormalisation runs. Ordinary Save still
preserves exact editor text, including CRLF. Merely opening an existing project
does not install or repair attributes. Migration fixture verification checks the
rules in every destination, while its existing original-preservation assertions
continue to cover the legacy workspace/drafts.

Actual local Git repositories/clones test `core.autocrlf=true` and `core.eol=crlf`:
root rules keep immutable sources, the manifest and diagram JSON at LF, and the
saved loader resolves every published revision. Without rules, revision bytes
become CRLF and integrity checks fail explicitly. A nested CRLF rule overrides
the root rule and also fails integrity verification. No newline-normalised hash
or cache fallback hides either failure. Users must review nested attributes and
explicitly coordinate already-tracked CRLF revisions; generated root rules do
not retrospectively rewrite or republish their immutable files.

A transaction replay test changes only the original source's LF to CRLF after
activation. Automatic replay refuses that foreign fingerprint. Explicit recovery
preserves every CRLF byte in its durable conflict copy before installing the
intended LF source. Expected/target and immutable source hashes remain exact.

B9 retains the documented best-effort external-writer boundary and defers a
local-head bump after reload. A channel-controlled uncoordinated replacement
pauses between equal-length edits to two scenario files. With a writer and with
a read-only session, both scanning and a stable saved reader can observe the
unchanging intermediate tree. The changed scenario invalidates an earlier run
ticket; the second replacement is detected on the next scan. Neither reader
advances local head. This demonstrates why an extra generation after detection
would not establish an atomic Git checkout. Cooperating API Forge transactions
still provide their full stable-generation guarantee.

Focused verification passed all four portability tests, all 13 migration tests
and the newline-only replay test. Final verification of implementation `a9f14cf`:
formatting and workspace/all-target clippy with warnings denied passed; 906
workspace tests passed across 48 test binaries with two process helpers ignored,
serially under isolated XDG paths. The v1 fixture check and all 10 isolated UI
acceptance probes passed. No language or layout implementation changes occur in
this package, so the fuzzers and layout benchmarks were not rerun. Native Windows checkout/UI and
directory durability remain open release gates; Linux Git evidence does not
close them. B10's storage/language dependency split remains deferred as recorded
in the consistency review.
