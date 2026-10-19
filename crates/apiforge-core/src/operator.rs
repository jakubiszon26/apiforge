//! Built-in expression operators: their catalogue and their evaluation.
//!
//! One catalogue drives the editor, graph ports, validation and the interpreter.
//! Evaluation is strict: operands are never coerced, a missing required operand is
//! an error, and every numeric result must be finite. Defaults declared in the
//! catalogue are applied by the caller that owns the operand slots (a formula node),
//! not here, so a nested expression with an empty slot fails visibly.

use crate::{
    js::{self, number_as_f64, number_value, strict_equals, type_name},
    model::ValueType,
    path,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::f64::consts::PI;
use thiserror::Error;

/// Longest text an operator may produce, in UTF-16 code units.
pub const MAX_TEXT_RESULT: usize = 100_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Random,
    Round,
    Equals,
    NotEquals,
    Greater,
    Less,
    AtLeast,
    AtMost,
    And,
    Or,
    Not,
    Concat,
    Letter,
    Length,
    Contains,
    Abs,
    Floor,
    Ceiling,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Ln,
    Log,
    Exp,
    Power10,
    TypeOf,
    Exists,
    Field,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorGroup {
    Math,
    Comparison,
    Logic,
    Text,
    Functions,
    Json,
}

/// Value an editor places in an operand slot when the operator is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OperandDefault {
    Null,
    Bool(bool),
    Integer(i64),
    Text(&'static str),
}

impl OperandDefault {
    pub fn to_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(value),
            Self::Integer(value) => Value::from(value),
            Self::Text(value) => Value::from(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OperandSpec {
    /// Slot name used in documents (`left` or `right`).
    pub name: &'static str,
    pub value_type: ValueType,
    /// A required operand must be present; an optional one may be missing.
    pub required: bool,
    pub default: Option<OperandDefault>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OperatorSpec {
    pub group: OperatorGroup,
    pub output: ValueType,
    pub operands: &'static [OperandSpec],
}

impl OperatorSpec {
    pub fn operand(&self, name: &str) -> Option<&'static OperandSpec> {
        self.operands.iter().find(|operand| operand.name == name)
    }
}

const fn operand(
    name: &'static str,
    value_type: ValueType,
    default: OperandDefault,
) -> OperandSpec {
    OperandSpec {
        name,
        value_type,
        required: true,
        default: Some(default),
    }
}

const LEFT_NUMBER: OperandSpec = operand("left", ValueType::Number, OperandDefault::Integer(0));
const RIGHT_NUMBER: OperandSpec = operand("right", ValueType::Number, OperandDefault::Integer(1));
const LEFT_ANY: OperandSpec = operand("left", ValueType::Any, OperandDefault::Null);
const RIGHT_ANY: OperandSpec = operand("right", ValueType::Any, OperandDefault::Null);
const LEFT_BOOLEAN: OperandSpec = operand("left", ValueType::Boolean, OperandDefault::Bool(false));
const RIGHT_BOOLEAN: OperandSpec =
    operand("right", ValueType::Boolean, OperandDefault::Bool(false));

const NUMBER_PAIR: &[OperandSpec] = &[LEFT_NUMBER, RIGHT_NUMBER];
const NUMBER_UNARY: &[OperandSpec] = &[LEFT_NUMBER];
const ANY_PAIR: &[OperandSpec] = &[LEFT_ANY, RIGHT_ANY];
const ROUND: &[OperandSpec] = &[
    LEFT_NUMBER,
    operand("right", ValueType::Number, OperandDefault::Integer(0)),
];
const LETTER: &[OperandSpec] = &[
    operand("left", ValueType::String, OperandDefault::Text("")),
    RIGHT_NUMBER,
];
const FIELD: &[OperandSpec] = &[
    LEFT_ANY,
    operand("right", ValueType::String, OperandDefault::Text("")),
];
const EXISTS: &[OperandSpec] = &[OperandSpec {
    name: "left",
    value_type: ValueType::Any,
    required: false,
    default: None,
}];

impl Operator {
    /// Every operator, in catalogue order.
    pub const ALL: [Self; 37] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Modulo,
        Self::Random,
        Self::Round,
        Self::Equals,
        Self::NotEquals,
        Self::Greater,
        Self::Less,
        Self::AtLeast,
        Self::AtMost,
        Self::And,
        Self::Or,
        Self::Not,
        Self::Concat,
        Self::Letter,
        Self::Length,
        Self::Contains,
        Self::Abs,
        Self::Floor,
        Self::Ceiling,
        Self::Sqrt,
        Self::Sin,
        Self::Cos,
        Self::Tan,
        Self::Asin,
        Self::Acos,
        Self::Atan,
        Self::Ln,
        Self::Log,
        Self::Exp,
        Self::Power10,
        Self::TypeOf,
        Self::Exists,
        Self::Field,
    ];

    pub fn spec(self) -> OperatorSpec {
        use OperatorGroup as G;
        use ValueType as T;
        let (group, output, operands) = match self {
            Self::Add
            | Self::Subtract
            | Self::Multiply
            | Self::Divide
            | Self::Modulo
            | Self::Random => (G::Math, T::Number, NUMBER_PAIR),
            Self::Round => (G::Math, T::Number, ROUND),
            Self::Equals | Self::NotEquals => (G::Comparison, T::Boolean, ANY_PAIR),
            Self::Greater | Self::Less | Self::AtLeast | Self::AtMost => {
                (G::Comparison, T::Boolean, NUMBER_PAIR)
            }
            Self::And | Self::Or => (G::Logic, T::Boolean, &[LEFT_BOOLEAN, RIGHT_BOOLEAN][..]),
            Self::Not => (G::Logic, T::Boolean, &[LEFT_BOOLEAN][..]),
            Self::Concat => (G::Text, T::String, ANY_PAIR),
            Self::Letter => (G::Text, T::String, LETTER),
            Self::Length => (G::Text, T::Number, &[LEFT_ANY][..]),
            Self::Contains => (G::Text, T::Boolean, ANY_PAIR),
            Self::Abs
            | Self::Floor
            | Self::Ceiling
            | Self::Sqrt
            | Self::Sin
            | Self::Cos
            | Self::Tan
            | Self::Asin
            | Self::Acos
            | Self::Atan
            | Self::Ln
            | Self::Log
            | Self::Exp
            | Self::Power10 => (G::Functions, T::Number, NUMBER_UNARY),
            Self::TypeOf => (G::Json, T::String, &[LEFT_ANY][..]),
            Self::Exists => (G::Json, T::Boolean, EXISTS),
            Self::Field => (G::Json, T::Any, FIELD),
        };
        OperatorSpec {
            group,
            output,
            operands,
        }
    }

    /// Applies the operator to resolved operands; `None` is a missing operand.
    ///
    /// The result is `None` only for `field` addressing a missing value.
    pub fn evaluate(
        self,
        left: Option<&Value>,
        right: Option<&Value>,
        random: &mut dyn RandomSource,
    ) -> Result<Option<Value>, OperatorError> {
        for spec in self.spec().operands {
            let value = match spec.name {
                "left" => left,
                _ => right,
            };
            check_operand(self, spec, value)?;
        }
        // Operand types are checked above, so these reads cannot fail for operators
        // that declare the corresponding type.
        let a = left
            .and_then(Value::as_number)
            .map(number_as_f64)
            .unwrap_or(f64::NAN);
        let b = right
            .and_then(Value::as_number)
            .map(number_as_f64)
            .unwrap_or(f64::NAN);
        let flag = |value: Option<&Value>| value.and_then(Value::as_bool).unwrap_or(false);

        let result = match self {
            Self::Add => number(a + b)?,
            Self::Subtract => number(a - b)?,
            Self::Multiply => number(a * b)?,
            Self::Divide if b == 0.0 => return Err(OperatorError::DivisionByZero),
            Self::Divide => number(a / b)?,
            Self::Modulo if b == 0.0 => return Err(OperatorError::ModuloByZero),
            // The sign follows the divisor, unlike Rust's `%`: -5 mod 3 = 1.
            Self::Modulo => number(((a % b) + b) % b)?,
            Self::Random => number(random_between(a, b, random))?,
            Self::Round => number(round(a, b)?)?,
            Self::Equals => Value::Bool(strict_equals(left, right)),
            Self::NotEquals => Value::Bool(!strict_equals(left, right)),
            Self::Greater => Value::Bool(a > b),
            Self::Less => Value::Bool(a < b),
            Self::AtLeast => Value::Bool(a >= b),
            Self::AtMost => Value::Bool(a <= b),
            // Both operands are always evaluated; there is no short-circuiting.
            Self::And => Value::Bool(flag(left) && flag(right)),
            Self::Or => Value::Bool(flag(left) || flag(right)),
            Self::Not => Value::Bool(!flag(left)),
            Self::Concat => {
                let mut text = js::to_display_string(left.unwrap_or(&Value::Null));
                text.push_str(&js::to_display_string(right.unwrap_or(&Value::Null)));
                Value::String(text)
            }
            Self::Letter => letter(left, b)?,
            Self::Length => match left {
                Some(Value::String(text)) => Value::from(text.chars().count()),
                Some(Value::Array(items)) => Value::from(items.len()),
                _ => return Err(OperatorError::LengthOperand),
            },
            Self::Contains => Value::Bool(match (left, right) {
                (Some(Value::String(text)), Some(Value::String(needle))) => {
                    text.contains(needle.as_str())
                }
                (Some(Value::Array(items)), needle) => {
                    items.iter().any(|item| strict_equals(Some(item), needle))
                }
                _ => return Err(OperatorError::ContainsOperands),
            }),
            Self::Abs => number(a.abs())?,
            Self::Floor => number(a.floor())?,
            Self::Ceiling => number(a.ceil())?,
            Self::Sqrt => number(libm::sqrt(a))?,
            // Angles are in degrees. Sine and cosine are rounded to ten decimals so
            // that sin(180) is 0 rather than 1.2e-16.
            Self::Sin => number(js::round_to_fixed(libm::sin(a * PI / 180.0), 10))?,
            Self::Cos => number(js::round_to_fixed(libm::cos(a * PI / 180.0), 10))?,
            Self::Tan => {
                let radians = a * PI / 180.0;
                if libm::cos(radians).abs() < 1e-12 {
                    return Err(OperatorError::UndefinedTangent);
                }
                number(libm::tan(radians))?
            }
            Self::Asin => number(libm::asin(a) * 180.0 / PI)?,
            Self::Acos => number(libm::acos(a) * 180.0 / PI)?,
            Self::Atan => number(libm::atan(a) * 180.0 / PI)?,
            Self::Ln => number(libm::log(a))?,
            Self::Log => number(libm::log10(a))?,
            Self::Exp => number(libm::exp(a))?,
            Self::Power10 => number(libm::pow(10.0, a))?,
            Self::TypeOf => Value::from(type_name(left)),
            Self::Exists => Value::Bool(left.is_some()),
            Self::Field => {
                let path = right.and_then(Value::as_str).unwrap_or_default();
                return Ok(left
                    .and_then(|value| path::resolve(value, path))
                    .map(|value| value.into_owned()));
            }
        };

        if let Value::String(text) = &result
            && js::utf16_len(text) > MAX_TEXT_RESULT
        {
            return Err(OperatorError::TextTooLong {
                limit: MAX_TEXT_RESULT,
            });
        }
        Ok(Some(result))
    }
}

/// Source of uniformly distributed numbers in `[0, 1)` for the `random` operator.
///
/// Injected so that runs can record a seed and tests stay deterministic.
pub trait RandomSource {
    fn next_unit(&mut self) -> f64;
}

impl<F: FnMut() -> f64> RandomSource for F {
    fn next_unit(&mut self) -> f64 {
        self()
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum OperatorError {
    #[error("operand `{operand}` of {operator:?} expects {expected}, got {actual}")]
    OperandType {
        operator: Operator,
        operand: &'static str,
        expected: ValueType,
        actual: &'static str,
    },
    #[error("division by zero")]
    DivisionByZero,
    #[error("modulo by zero")]
    ModuloByZero,
    #[error("rounding precision must be an integer from 0 to 12")]
    InvalidPrecision,
    #[error("letter position must be an integer of at least 1")]
    InvalidLetterPosition,
    #[error("length requires a string or an array")]
    LengthOperand,
    #[error("contains requires two strings, or an array and a value")]
    ContainsOperands,
    #[error("tangent is undefined for this angle")]
    UndefinedTangent,
    #[error("result is not a finite number")]
    NonFiniteResult,
    #[error("text result exceeds {limit} UTF-16 code units")]
    TextTooLong { limit: usize },
}

fn check_operand(
    operator: Operator,
    spec: &OperandSpec,
    value: Option<&Value>,
) -> Result<(), OperatorError> {
    if value.is_none() && !spec.required {
        return Ok(());
    }
    let actual = type_name(value);
    let accepted = value.is_some()
        && (spec.value_type == ValueType::Any || spec.value_type.as_str() == actual);
    if accepted {
        Ok(())
    } else {
        Err(OperatorError::OperandType {
            operator,
            operand: spec.name,
            expected: spec.value_type,
            actual,
        })
    }
}

fn number(value: f64) -> Result<Value, OperatorError> {
    number_value(value).ok_or(OperatorError::NonFiniteResult)
}

fn is_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

/// Integral bounds give an integer from the inclusive range, otherwise a real number.
fn random_between(a: f64, b: f64, random: &mut dyn RandomSource) -> f64 {
    let (low, high) = (a.min(b), a.max(b));
    let unit = random.next_unit();
    if is_integer(low) && is_integer(high) {
        low + (unit * (high - low + 1.0)).floor()
    } else {
        low + unit * (high - low)
    }
}

fn round(value: f64, places: f64) -> Result<f64, OperatorError> {
    if !is_integer(places) || !(0.0..=12.0).contains(&places) {
        return Err(OperatorError::InvalidPrecision);
    }
    // Every power of ten up to 1e12 is exact. The epsilon nudge mirrors the
    // reference implementation, so 1.005 rounds to 1.01.
    let scale = libm::pow(10.0, places);
    Ok(js_math_round((value + f64::EPSILON) * scale) / scale)
}

/// `Math.round`: halves round towards positive infinity, so -2.5 becomes -2.
fn js_math_round(value: f64) -> f64 {
    let floor = value.floor();
    if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// The `position`-th Unicode scalar value (1-based), or empty text past the end.
fn letter(text: Option<&Value>, position: f64) -> Result<Value, OperatorError> {
    if !is_integer(position) || position < 1.0 {
        return Err(OperatorError::InvalidLetterPosition);
    }
    let text = text.and_then(Value::as_str).unwrap_or_default();
    // Positions beyond usize::MAX saturate, which is past the end of any text.
    let index = (position - 1.0) as usize;
    Ok(Value::String(
        text.chars()
            .nth(index)
            .map(String::from)
            .unwrap_or_default(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn never() -> f64 {
        panic!("operator must not consume randomness")
    }

    fn eval(
        operator: Operator,
        left: Value,
        right: Option<Value>,
    ) -> Result<Option<Value>, OperatorError> {
        operator.evaluate(Some(&left), right.as_ref(), &mut never)
    }

    fn ok(operator: Operator, left: Value, right: Option<Value>) -> Value {
        eval(operator, left, right)
            .unwrap_or_else(|error| panic!("{operator:?}: {error}"))
            .unwrap_or_else(|| panic!("{operator:?}: missing result"))
    }

    #[test]
    fn every_operator_evaluates_its_reference_example() {
        use Operator::*;
        let examples = [
            (Add, json!(2), Some(json!(3)), json!(5)),
            (Subtract, json!(2), Some(json!(3)), json!(-1)),
            (Multiply, json!(2), Some(json!(3)), json!(6)),
            (Divide, json!(6), Some(json!(3)), json!(2)),
            (Modulo, json!(-5), Some(json!(3)), json!(1)),
            (Round, json!(1.234), Some(json!(2)), json!(1.23)),
            (
                Equals,
                json!({"a": 1, "b": 2}),
                Some(json!({"b": 2, "a": 1})),
                json!(true),
            ),
            (NotEquals, json!(1), Some(json!("1")), json!(true)),
            (Greater, json!(2), Some(json!(1)), json!(true)),
            (Less, json!(1), Some(json!(2)), json!(true)),
            (AtLeast, json!(2), Some(json!(2)), json!(true)),
            (AtMost, json!(2), Some(json!(2)), json!(true)),
            (And, json!(true), Some(json!(false)), json!(false)),
            (Or, json!(false), Some(json!(true)), json!(true)),
            (Not, json!(true), None, json!(false)),
            (
                Concat,
                json!("hello "),
                Some(json!(123)),
                json!("hello 123"),
            ),
            (Letter, json!("a😀b"), Some(json!(2)), json!("😀")),
            (Length, json!(["a", "b"]), None, json!(2)),
            (
                Contains,
                json!([{"a": 1}]),
                Some(json!({"a": 1})),
                json!(true),
            ),
            (Abs, json!(-5), None, json!(5)),
            (Floor, json!(1.9), None, json!(1)),
            (Ceiling, json!(1.1), None, json!(2)),
            (Sqrt, json!(4), None, json!(2)),
            (Sin, json!(90), None, json!(1)),
            (Cos, json!(180), None, json!(-1)),
            (Tan, json!(0), None, json!(0)),
            (Asin, json!(1), None, json!(90)),
            (Acos, json!(1), None, json!(0)),
            (Atan, json!(0), None, json!(0)),
            (Ln, json!(1), None, json!(0)),
            (Log, json!(100), None, json!(2)),
            (Exp, json!(0), None, json!(1)),
            (Power10, json!(2), None, json!(100)),
            (TypeOf, json!(null), None, json!("null")),
            (Exists, json!(false), None, json!(true)),
            (
                Field,
                json!({"a": [{"value": 42}]}),
                Some(json!("a[0].value")),
                json!(42),
            ),
        ];
        let mut covered: Vec<_> = examples.iter().map(|example| example.0).collect();
        covered.push(Random);
        covered.sort_by_key(|operator| Operator::ALL.iter().position(|o| o == operator));
        assert_eq!(covered, Operator::ALL);

        for (operator, left, right, expected) in examples {
            assert_eq!(ok(operator, left, right), expected, "{operator:?}");
        }
    }

    #[test]
    fn edge_cases_follow_the_reference_implementation() {
        use Operator::*;
        assert_eq!(ok(Length, json!("a😀b"), None), json!(3));
        assert_eq!(
            ok(Contains, json!("Hello"), Some(json!("hello"))),
            json!(false)
        );
        assert_eq!(
            ok(Contains, json!([1, null]), Some(json!(null))),
            json!(true)
        );
        assert_eq!(ok(TypeOf, json!([]), None), json!("array"));
        assert_eq!(ok(TypeOf, json!({}), None), json!("object"));
        assert_eq!(ok(Letter, json!("abc"), Some(json!(10))), json!(""));
        assert_eq!(ok(Round, json!(1.005), Some(json!(2))), json!(1.01));
        assert_eq!(ok(Round, json!(-2.5), Some(json!(0))), json!(-2));
        assert_eq!(ok(Modulo, json!(5.5), Some(json!(-2))), json!(-0.5));
        assert_eq!(ok(Equals, json!(1), Some(json!(1.0))), json!(true));
        assert_eq!(ok(Sin, json!(180), None), json!(0));
        assert_eq!(
            ok(Concat, json!([1, null, 2]), Some(json!({}))),
            json!("1,,2[object Object]")
        );
        assert_eq!(ok(Concat, json!(0.1), Some(json!(1e21))), json!("0.11e+21"));
        assert_eq!(eval(Field, json!({}), Some(json!("missing"))), Ok(None));
        assert_eq!(eval(Field, json!({"a": 1}), Some(json!("a..b"))), Ok(None));
        assert_eq!(
            Exists.evaluate(None, None, &mut never),
            Ok(Some(json!(false)))
        );
    }

    #[test]
    fn operands_are_never_coerced() {
        use Operator::*;
        let cases = [
            (Add, json!("2"), Some(json!(3))),
            (Multiply, json!(null), Some(json!(2))),
            (And, json!("true"), Some(json!(true))),
            (Not, json!(0), None),
            (Letter, json!(5), Some(json!(1))),
            (Field, json!({}), Some(json!(0))),
        ];
        for (operator, left, right) in cases {
            assert!(
                matches!(
                    eval(operator, left, right),
                    Err(OperatorError::OperandType { .. })
                ),
                "{operator:?}"
            );
        }
        assert_eq!(
            Add.evaluate(Some(&json!(1)), None, &mut never),
            Err(OperatorError::OperandType {
                operator: Add,
                operand: "right",
                expected: ValueType::Number,
                actual: "missing",
            })
        );
        assert!(TypeOf.evaluate(None, None, &mut never).is_err());
    }

    #[test]
    fn undefined_mathematics_is_an_error() {
        use Operator::*;
        let cases = [
            (
                Divide,
                json!(1),
                Some(json!(0)),
                OperatorError::DivisionByZero,
            ),
            (
                Modulo,
                json!(1),
                Some(json!(0)),
                OperatorError::ModuloByZero,
            ),
            (Sqrt, json!(-1), None, OperatorError::NonFiniteResult),
            (Tan, json!(90), None, OperatorError::UndefinedTangent),
            (Ln, json!(0), None, OperatorError::NonFiniteResult),
            (Asin, json!(2), None, OperatorError::NonFiniteResult),
            (
                Multiply,
                json!(1e308),
                Some(json!(10)),
                OperatorError::NonFiniteResult,
            ),
            (
                Round,
                json!(1),
                Some(json!(-1)),
                OperatorError::InvalidPrecision,
            ),
            (
                Round,
                json!(1),
                Some(json!(13)),
                OperatorError::InvalidPrecision,
            ),
            (
                Round,
                json!(1),
                Some(json!(0.5)),
                OperatorError::InvalidPrecision,
            ),
            (
                Letter,
                json!("text"),
                Some(json!(0)),
                OperatorError::InvalidLetterPosition,
            ),
            (
                Letter,
                json!("text"),
                Some(json!(1.5)),
                OperatorError::InvalidLetterPosition,
            ),
            (Length, json!({}), None, OperatorError::LengthOperand),
            (
                Contains,
                json!(12),
                Some(json!("2")),
                OperatorError::ContainsOperands,
            ),
            (
                Contains,
                json!("12"),
                Some(json!(2)),
                OperatorError::ContainsOperands,
            ),
        ];
        for (operator, left, right, error) in cases {
            assert_eq!(eval(operator, left, right), Err(error), "{operator:?}");
        }
    }

    #[test]
    fn text_results_are_limited_in_utf16_units() {
        let long = "a".repeat(MAX_TEXT_RESULT);
        assert_eq!(
            eval(Operator::Concat, json!(long), Some(json!("b"))),
            Err(OperatorError::TextTooLong {
                limit: MAX_TEXT_RESULT
            })
        );
        // Each emoji is two UTF-16 units although it is a single scalar value.
        let emoji = "😀".repeat(MAX_TEXT_RESULT / 2);
        assert!(eval(Operator::Concat, json!(emoji), Some(json!(""))).is_ok());
        let emoji = "😀".repeat(MAX_TEXT_RESULT / 2);
        assert!(eval(Operator::Concat, json!(emoji), Some(json!("x"))).is_err());
    }

    #[test]
    fn random_uses_the_injected_source() {
        let fixed = |unit: f64| move || unit;
        let random = |a: Value, b: Value, source: &mut dyn RandomSource| {
            Operator::Random
                .evaluate(Some(&a), Some(&b), source)
                .unwrap()
                .unwrap()
        };
        assert_eq!(random(json!(10), json!(1), &mut fixed(0.0)), json!(1));
        assert_eq!(
            random(json!(10), json!(1), &mut fixed(0.999_999)),
            json!(10)
        );
        assert_eq!(random(json!(7), json!(7), &mut fixed(0.5)), json!(7));
        assert_eq!(random(json!(0), json!(1.5), &mut fixed(0.5)), json!(0.75));
    }

    #[test]
    fn catalogue_defaults_match_operand_types() {
        for operator in Operator::ALL {
            let spec = operator.spec();
            assert!(
                !spec.operands.is_empty() && spec.operands.len() <= 2,
                "{operator:?}"
            );
            assert_eq!(spec.operands[0].name, "left");
            for operand in spec.operands {
                let Some(default) = operand.default else {
                    assert!(!operand.required, "{operator:?}");
                    continue;
                };
                let value = default.to_value();
                let actual = type_name(Some(&value));
                assert!(
                    operand.value_type == ValueType::Any || operand.value_type.as_str() == actual,
                    "{operator:?}.{}",
                    operand.name
                );
            }
        }
    }
}
