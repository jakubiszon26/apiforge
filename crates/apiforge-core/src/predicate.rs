//! Predicates of Comparison, Data condition and IF nodes.

use crate::{
    js::{number_as_f64, strict_equals, type_name},
    model::Predicate,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PredicateError {
    #[error("a value required for the comparison is missing")]
    MissingValue,
    #[error("the comparison requires finite numbers")]
    NotANumber,
}

impl Predicate {
    pub const ALL: [Self; 9] = [
        Self::Equals,
        Self::NotEquals,
        Self::Greater,
        Self::Less,
        Self::Approximately,
        Self::Exists,
        Self::NotEmpty,
        Self::Contains,
        Self::Type,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Equals => "is equal to",
            Self::NotEquals => "is not equal to",
            Self::Greater => "is greater than",
            Self::Less => "is less than",
            Self::Approximately => "is approximately",
            Self::Exists => "exists",
            Self::NotEmpty => "is not empty",
            Self::Contains => "contains",
            Self::Type => "has type",
        }
    }

    /// Whether the predicate needs an expected value.
    pub fn takes_expected(self) -> bool {
        !matches!(self, Self::Exists | Self::NotEmpty)
    }

    /// Evaluates the predicate. `None` is a missing value, distinct from JSON null.
    ///
    /// `tolerance` applies to `approximately` only.
    pub fn check(
        self,
        actual: Option<&Value>,
        expected: Option<&Value>,
        tolerance: f64,
    ) -> Result<bool, PredicateError> {
        match self {
            Self::Exists => return Ok(actual.is_some()),
            Self::NotEmpty => {
                return Ok(match actual {
                    None | Some(Value::Null) => false,
                    Some(Value::String(text)) => !text.is_empty(),
                    Some(Value::Array(items)) => !items.is_empty(),
                    Some(Value::Object(fields)) => !fields.is_empty(),
                    Some(_) => true,
                });
            }
            _ => {}
        }
        let (Some(actual), Some(expected)) = (actual, expected) else {
            return Err(PredicateError::MissingValue);
        };
        Ok(match self {
            Self::Equals => strict_equals(Some(actual), Some(expected)),
            Self::NotEquals => !strict_equals(Some(actual), Some(expected)),
            Self::Type => expected.as_str() == Some(type_name(Some(actual))),
            Self::Contains => match (actual, expected) {
                (Value::String(text), Value::String(needle)) => text.contains(needle.as_str()),
                (Value::Array(items), needle) => items
                    .iter()
                    .any(|item| strict_equals(Some(item), Some(needle))),
                _ => false,
            },
            Self::Greater | Self::Less | Self::Approximately => {
                let (Value::Number(actual), Value::Number(expected)) = (actual, expected) else {
                    return Err(PredicateError::NotANumber);
                };
                let (actual, expected) = (number_as_f64(actual), number_as_f64(expected));
                match self {
                    Self::Greater => actual > expected,
                    Self::Less => actual < expected,
                    _ => (actual - expected).abs() <= tolerance,
                }
            }
            Self::Exists | Self::NotEmpty => unreachable!("handled above"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn check(
        predicate: Predicate,
        actual: Option<Value>,
        expected: Option<Value>,
    ) -> Result<bool, PredicateError> {
        predicate.check(actual.as_ref(), expected.as_ref(), 0.0)
    }

    #[test]
    fn existence_distinguishes_missing_from_falsy_values() {
        for value in [json!(null), json!(false), json!(0), json!("")] {
            assert_eq!(check(Predicate::Exists, Some(value), None), Ok(true));
        }
        assert_eq!(check(Predicate::Exists, None, None), Ok(false));
        for (value, expected) in [
            (json!(false), true),
            (json!(0), true),
            (json!(null), false),
            (json!(""), false),
            (json!([]), false),
            (json!({}), false),
            (json!([0]), true),
        ] {
            assert_eq!(
                check(Predicate::NotEmpty, Some(value.clone()), None),
                Ok(expected),
                "{value}"
            );
        }
        assert_eq!(check(Predicate::NotEmpty, None, None), Ok(false));
    }

    #[test]
    fn value_predicates_compare_json_without_coercion() {
        assert_eq!(
            check(
                Predicate::Equals,
                Some(json!({"a": 1})),
                Some(json!({"a": 1.0}))
            ),
            Ok(true)
        );
        assert_eq!(
            check(Predicate::Equals, Some(json!(1)), Some(json!("1"))),
            Ok(false)
        );
        assert_eq!(
            check(Predicate::Type, Some(json!([])), Some(json!("array"))),
            Ok(true)
        );
        assert_eq!(
            check(Predicate::Type, Some(json!(null)), Some(json!("null"))),
            Ok(true)
        );
        assert_eq!(
            check(
                Predicate::Contains,
                Some(json!("Hello")),
                Some(json!("ell"))
            ),
            Ok(true)
        );
        assert_eq!(
            check(
                Predicate::Contains,
                Some(json!([{"a": 1}])),
                Some(json!({"a": 1}))
            ),
            Ok(true)
        );
        assert_eq!(
            check(Predicate::Contains, Some(json!(12)), Some(json!(1))),
            Ok(false)
        );
        assert_eq!(
            check(Predicate::Equals, None, Some(json!(1))),
            Err(PredicateError::MissingValue)
        );
    }

    #[test]
    fn numeric_predicates_require_numbers() {
        assert_eq!(
            check(Predicate::Greater, Some(json!(2)), Some(json!(1))),
            Ok(true)
        );
        assert_eq!(
            check(Predicate::Less, Some(json!("1")), Some(json!(2))),
            Err(PredicateError::NotANumber)
        );
        assert_eq!(
            Predicate::Approximately.check(Some(&json!(1.05)), Some(&json!(1)), 0.1),
            Ok(true)
        );
        assert_eq!(
            Predicate::Approximately.check(Some(&json!(1.2)), Some(&json!(1)), 0.1),
            Ok(false)
        );
    }
}
