//! Differential tests against results recorded from the TypeScript reference.
//!
//! The corpus is produced by `scripts/export-compatibility-fixtures.ts`. A mismatch
//! means the port changed observable behaviour of existing projects; fix the port
//! or record a deliberate, versioned divergence instead of editing the corpus.

use apiforge_core::{
    js::{number_to_string, strict_equals},
    operator::{Operator, OperatorError},
    path,
};
use serde::Deserialize;
use serde_json::Value;
use std::{fmt::Debug, fs, path::PathBuf};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Outcome {
    Value {
        value: Value,
    },
    Missing {
        #[allow(dead_code, reason = "marker field selecting the variant")]
        missing: bool,
    },
    Error {
        #[allow(dead_code, reason = "marker field selecting the variant")]
        error: bool,
    },
}

/// Operators whose results may differ from V8 by one unit in the last place.
///
/// See docs/decisions/0001-numeric-semantics.md: the port uses a platform-independent
/// libm, and V8's `exp`, `pow` and `log10` are not correctly rounded in the same cases.
const ONE_ULP_OPERATORS: [Operator; 3] = [Operator::Exp, Operator::Log, Operator::Power10];

/// Distance in units in the last place between two finite doubles of equal sign.
fn ulp_distance(left: f64, right: f64) -> u64 {
    if left.is_sign_negative() != right.is_sign_negative() {
        return if left == right { 0 } else { u64::MAX };
    }
    left.to_bits().abs_diff(right.to_bits())
}

fn within_one_ulp(expected: &Value, actual: &Value) -> bool {
    match (expected.as_f64(), actual.as_f64()) {
        (Some(expected), Some(actual)) => ulp_distance(expected, actual) <= 1,
        _ => false,
    }
}

impl Outcome {
    fn matches(&self, actual: &Result<Option<Value>, impl Debug>) -> bool {
        self.matches_within(actual, false)
    }

    fn matches_within(&self, actual: &Result<Option<Value>, impl Debug>, one_ulp: bool) -> bool {
        match (self, actual) {
            (Self::Value { value }, Ok(Some(actual))) => {
                strict_equals(Some(value), Some(actual))
                    || (one_ulp && within_one_ulp(value, actual))
            }
            (Self::Missing { .. }, Ok(None)) | (Self::Error { .. }, Err(_)) => true,
            _ => false,
        }
    }
}

/// Reads a present field, including JSON null, as `Some`; an absent field stays `None`.
/// Plain `Option<Value>` would turn null into a missing operand.
fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorCase {
    operator: Operator,
    #[serde(default, deserialize_with = "present")]
    left: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    right: Option<Value>,
    random: Option<f64>,
    outcome: Outcome,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PathCase {
    path: String,
    outcome: Outcome,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberCase {
    value: f64,
    text: String,
}

fn corpus<T: for<'de> Deserialize<'de>>(name: &str) -> Vec<T> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/compatibility/semantics")
        .join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    text.lines()
        .enumerate()
        .map(|(line, json)| {
            serde_json::from_str(json)
                .unwrap_or_else(|error| panic!("{name}:{}: {error}", line + 1))
        })
        .collect()
}

fn assert_no_mismatches(name: &str, total: usize, mismatches: &[String]) {
    assert!(total > 0, "{name}: empty corpus");
    assert!(
        mismatches.is_empty(),
        "{name}: {} of {total} cases differ from the reference:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn operators_match_the_reference_implementation() {
    let cases: Vec<OperatorCase> = corpus("operators.jsonl");
    let mut mismatches = Vec::new();
    for case in &cases {
        let unit = case.random;
        let mut random = || unit.expect("only `random` cases consume randomness");
        let actual: Result<Option<Value>, OperatorError> =
            case.operator
                .evaluate(case.left.as_ref(), case.right.as_ref(), &mut random);
        let one_ulp = ONE_ULP_OPERATORS.contains(&case.operator);
        if !case.outcome.matches_within(&actual, one_ulp) {
            mismatches.push(format!(
                "{:?}({:?}, {:?}): expected {:?}, got {actual:?}",
                case.operator, case.left, case.right, case.outcome
            ));
        }
    }
    for operator in Operator::ALL {
        assert!(
            cases.iter().any(|case| case.operator == operator),
            "{operator:?} has no corpus cases"
        );
    }
    assert_no_mismatches("operators", cases.len(), &mismatches);
}

#[test]
fn paths_match_the_reference_implementation() {
    let mut lines = corpus::<Value>("paths.jsonl").into_iter();
    let document = lines
        .next()
        .and_then(|header| header.get("document").cloned())
        .expect("the first line holds the document");
    let cases: Vec<PathCase> = lines
        .map(|line| serde_json::from_value(line).expect("path case"))
        .collect();

    let mismatches: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let actual: Result<Option<Value>, ()> =
                Ok(path::resolve(&document, &case.path).map(|value| value.into_owned()));
            (!case.outcome.matches(&actual)).then(|| {
                format!(
                    "{:?}: expected {:?}, got {actual:?}",
                    case.path, case.outcome
                )
            })
        })
        .collect();
    assert_no_mismatches("paths", cases.len(), &mismatches);
}

#[test]
fn number_formatting_matches_javascript() {
    let cases: Vec<NumberCase> = corpus("numbers.jsonl");
    let mismatches: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let actual = number_to_string(case.value);
            (actual != case.text)
                .then(|| format!("{:e}: expected {}, got {actual}", case.value, case.text))
        })
        .collect();
    assert_no_mismatches("numbers", cases.len(), &mismatches);
}
