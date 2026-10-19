# ADR 0001: Numeric and text semantics of expressions

- Status: accepted
- Date: 2026-10-19
- Scope: `apiforge-core` (`js`, `path`, `operator` modules)

## Context

Version-one projects were evaluated by the TypeScript reference implementation, so
every expression result was produced by a JavaScript engine (V8 in Node). Existing
assertions compare those results, often by exact equality. The native port must keep
them passing, and it must give identical results on Linux and Windows.

Rust and serde defaults differ from JavaScript in ways that would silently change
results:

- `serde_json` distinguishes integers and floats (`1` vs `1.0`); JavaScript does not.
- Without the `float_roundtrip` feature, `serde_json` may parse a decimal number to a
  double one unit in the last place (ULP) away from the correctly rounded value.
- Rust's shortest float formatting may choose a different 17-digit string than
  ECMAScript `Number.prototype.toString`, which requires the closest one.
- Rust's precision formatting rounds exact ties to even; `toFixed` rounds them away
  from zero.
- `f64::sin` and friends call the platform C library, whose results differ between
  glibc and the Microsoft CRT.

## Decision

1. Numbers are IEEE-754 doubles during evaluation. Integral results within
   ±(2^53 − 1) are emitted as JSON integers, so documents and reports print `2`, not
   `2.0`. Equality compares numbers by value.
2. `serde_json` is built with `float_roundtrip`: imported documents and responses
   keep the exact double a JavaScript engine would read.
3. Text conversion (`concat`, text templates) follows `String(value)`, including
   ECMAScript number formatting, `[object Object]` and comma-joined arrays.
4. Lengths and positions of text count Unicode scalar values (`Array.from`), while
   size limits inherited from version one count UTF-16 code units.
5. Transcendental functions use the pure-Rust `libm` crate, never the platform C
   library. Results are bit-identical on every supported platform.
6. `random` takes an injected `RandomSource`, so runs can record a seed and tests are
   deterministic.

## Accepted divergence

The differential corpus (`fixtures/compatibility/semantics/`, generated from the
TypeScript sources) shows that `libm` and V8 disagree by exactly one ULP for a small
fraction of inputs to `exp`, `log` (base 10) and `power10`. Checked against 60-digit
decimal arithmetic:

- for `log`, the Rust result is the correctly rounded one and V8's is not;
- for `exp` and `power10`, V8's result is correctly rounded and `libm`'s is not
  (e.g. `exp(1)` is `2.7182818284590455` instead of `2.718281828459045`).

Emulating V8 would mean porting its fdlibm variants, including their inaccuracy in
`log10`. We accept a documented difference of at most one ULP for these three
operators. The corpus test enforces that bound and requires exact agreement for every
other operator, including `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `ln` and
`sqrt`.

Exact powers of ten, `exp(0)` and integer `power10` results are exact in both
implementations. Users who compare transcendental results should round them first,
which the `round` operator supports.

## Consequences

- A change to these rules is a change to the project format semantics and requires a
  versioned migration, not a silent fix.
- Replacing `libm` with a correctly rounded implementation would remove the `exp` and
  `power10` differences and keep `log` correct; the corpus would then only need the
  tolerance for V8's `log10`.
- The corpus must be regenerated with
  `node --import tsx scripts/export-compatibility-fixtures.ts` whenever the reference
  semantics change, and is checked in CI.
