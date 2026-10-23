//! JSON value semantics compatible with JavaScript.
//!
//! Documents are JSON and expressions follow JavaScript value semantics: every number is an
//! IEEE-754 double, `1` and `1.0` are the same value, and text conversion follows
//! `String(value)`. Relying on Rust or serde defaults here would silently change the
//! outcome of existing formulas and assertions, so these rules are spelled out once.

use serde_json::{Number, Value};

/// Largest integer that a double represents exactly (`Number.MAX_SAFE_INTEGER`).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Name of the JSON type of a resolved value, using `"missing"` for an absent value.
///
/// Unlike `typeof`, `null` and arrays have their own names.
pub fn type_name(value: Option<&Value>) -> &'static str {
    match value {
        None => "missing",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

/// Numeric value of a JSON number as a double, exactly as `JSON.parse` would read it.
pub fn number_as_f64(number: &Number) -> f64 {
    // serde_json only produces finite numbers, and `as_f64` is total for them.
    number.as_f64().unwrap_or(f64::NAN)
}

/// Converts a computed double to JSON, or `None` when it is not finite.
///
/// Integral results within the safe integer range become JSON integers, so `6 / 3`
/// is exported as `2` rather than `2.0`, matching `JSON.stringify`. Negative zero
/// becomes `0`, which JavaScript also prints and compares as zero.
pub fn number_value(value: f64) -> Option<Value> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER {
        // Exact: the value is integral and within the range where i64 and f64 agree.
        return Some(Value::from(value as i64));
    }
    Number::from_f64(value).map(Value::Number)
}

/// Deep structural equality without type coercion.
///
/// Numbers compare by value (`1 == 1.0`), objects ignore field order and arrays
/// compare element-wise. A missing value only equals another missing value.
pub fn strict_equals(left: Option<&Value>, right: Option<&Value>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => values_equal(left, right),
        _ => false,
    }
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Number(left), Value::Number(right)) => number_as_f64(left) == number_as_f64(right),
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| values_equal(left, right))
                })
        }
        _ => false,
    }
}

/// Text conversion with the result of JavaScript `String(value)`.
///
/// Arrays join their elements with commas (null elements become empty) and objects
/// become `[object Object]`. This is the legacy behaviour of `concat` and of text
/// templates; it is not a serialisation format.
pub fn to_display_string(value: &Value) -> String {
    let mut output = String::new();
    write_display(value, &mut output);
    output
}

fn write_display(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&number_to_string(number_as_f64(value))),
        Value::String(value) => output.push_str(value),
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                if !value.is_null() {
                    write_display(value, output);
                }
            }
        }
        Value::Object(_) => output.push_str("[object Object]"),
    }
}

/// Compact JSON text with the result of JavaScript `JSON.stringify(value)`.
///
/// Differs from `serde_json::to_string` only in number formatting (`1e+21`).
pub fn json_stringify(value: &Value) -> String {
    let mut output = String::new();
    write_json(value, &mut output);
    output
}

fn write_json(value: &Value, output: &mut String) {
    match value {
        Value::Number(number) => output.push_str(&number_to_string(number_as_f64(number))),
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_json(item, output);
            }
            output.push(']');
        }
        Value::Object(fields) => {
            output.push('{');
            for (index, (key, item)) in fields.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_json(&Value::String(key.clone()), output);
                output.push(':');
                write_json(item, output);
            }
            output.push('}');
        }
        // Strings, booleans and null serialise identically in both implementations.
        scalar => output.push_str(&scalar.to_string()),
    }
}

/// Formats a double like ECMAScript `Number.prototype.toString()` in radix 10.
///
/// Uses the shortest digit string that round-trips, switching to exponential
/// notation below `1e-6` and from `1e21`, e.g. `1e+21`, `1.5e-7`, `0.000001`.
pub fn number_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if value < 0.0 {
        return format!("-{}", number_to_string(-value));
    }

    // `{:e}` yields a shortest round-tripping digit string, e.g. "1.2345e-7". When
    // several strings of that length round-trip, ECMAScript requires the one closest
    // to the exact value, which the shortest-digit search does not guarantee; the
    // correctly rounded string of the same length is preferred whenever it round-trips.
    let (mut digits, mut exponent) = split_scientific(&format!("{value:e}"));
    let closest = format!("{value:.precision$e}", precision = digits.len() - 1);
    if closest.parse::<f64>() == Ok(value) {
        (digits, exponent) = split_scientific(&closest);
    }

    // ECMAScript names the digit count k and the decimal point position n.
    let k = i32::try_from(digits.len()).expect("a double has at most 17 significant digits");
    let n = exponent + 1;

    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        let (integer, fraction) = digits.split_at(n as usize);
        format!("{integer}.{fraction}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let sign = if n - 1 < 0 { '-' } else { '+' };
        let magnitude = (n - 1).abs();
        let (first, rest) = digits.split_at(1);
        if rest.is_empty() {
            format!("{first}e{sign}{magnitude}")
        } else {
            format!("{first}.{rest}e{sign}{magnitude}")
        }
    }
}

/// Splits Rust exponential notation into its significant digits and exponent.
fn split_scientific(text: &str) -> (String, i32) {
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("exponential formatting always contains an exponent");
    let digits = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent = exponent
        .parse()
        .expect("exponential formatting produces an integer exponent");
    (digits, exponent)
}

/// Rounds like `Number(value.toFixed(digits))` for values below `1e21`.
///
/// `toFixed` works on the exact binary value and resolves exact ties away from zero,
/// whereas Rust's precision formatting resolves them to even. The exact decimal
/// expansion is therefore rounded by hand.
pub fn round_to_fixed(value: f64, digits: usize) -> f64 {
    if !value.is_finite() || value.abs() >= 1e21 {
        return value;
    }
    // 1100 fractional digits cover the exact expansion of every finite double.
    let exact = format!("{:.1100}", value.abs());
    let (integer, fraction) = exact
        .split_once('.')
        .expect("fixed formatting always contains a decimal point");
    let (kept, dropped) = fraction.split_at(digits);

    let mut magnitude: Vec<u8> = integer.bytes().chain(kept.bytes()).collect();
    if dropped.as_bytes()[0] >= b'5' {
        increment_decimal(&mut magnitude);
    }
    let magnitude = String::from_utf8(magnitude).expect("decimal digits are ASCII");
    let split = magnitude.len() - digits;
    let rounded: f64 = format!("{}.{}", &magnitude[..split], &magnitude[split..])
        .parse()
        .expect("rounded decimal digits parse as a double");
    rounded.copysign(value)
}

fn increment_decimal(digits: &mut Vec<u8>) {
    for digit in digits.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return;
        }
    }
    digits.insert(0, b'1');
}

/// Text length in UTF-16 code units, the unit of JavaScript `string.length`.
///
/// Limits inherited from version-one documents are expressed in this unit.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// JavaScript `String.prototype.trim`: Unicode white space and line terminators,
/// including U+FEFF but, unlike [`str::trim`], not U+0085.
pub fn trim(text: &str) -> &str {
    text.trim_matches(|character: char| {
        character == '\u{feff}' || (character != '\u{85}' && character.is_whitespace())
    })
}

/// The numeric value of a canonical array index key (`"0"`, `"17"`, never `"01"`),
/// which JavaScript objects order before every other key.
pub fn array_index_key(key: &str) -> Option<u32> {
    let canonical = key == "0" || (!key.starts_with('0') && !key.is_empty());
    if !canonical || !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    // 2^32 - 1 is not an array index.
    key.parse::<u32>().ok().filter(|index| *index != u32::MAX)
}

/// Reorders object entries the way a JavaScript object enumerates its own keys:
/// array index keys in ascending numeric order, then the others in insertion order.
///
/// Objects built from arbitrary names (node inputs, headers, parsed responses)
/// follow this order, which `Object.values` exposes.
pub fn js_key_order<K: AsRef<str>, V>(entries: impl IntoIterator<Item = (K, V)>) -> Vec<(K, V)> {
    let (mut indices, names): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(key, _)| array_index_key(key.as_ref()).is_some());
    indices.sort_by_key(|(key, _)| array_index_key(key.as_ref()));
    indices.extend(names);
    indices
}

/// [`js_key_order`] applied to every object inside a JSON value, as `JSON.parse` does.
pub fn with_js_key_order(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(with_js_key_order).collect()),
        Value::Object(fields) => Value::Object(
            js_key_order(fields)
                .into_iter()
                .map(|(key, value)| (key, with_js_key_order(value)))
                .collect(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn number_to_string_matches_ecmascript() {
        let cases = [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (123.456, "123.456"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (123_456_789_012_345_680_000.0, "123456789012345680000"),
            (0.000_001, "0.000001"),
            (0.000_000_15, "1.5e-7"),
            (1e-7, "1e-7"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (9_007_199_254_740_993.0, "9007199254740992"),
            // Two 17-digit strings round-trip; JavaScript prints the closer one.
            (-232_711_809_221_655.12, "-232711809221655.12"),
        ];
        for (value, expected) in cases {
            assert_eq!(number_to_string(value), expected, "{value:e}");
        }
    }

    #[test]
    fn object_keys_follow_javascript_enumeration_order() {
        assert_eq!(array_index_key("0"), Some(0));
        assert_eq!(array_index_key("4294967294"), Some(4_294_967_294));
        for key in ["", "01", "-1", "1.5", "4294967295", "1e3", " 1", "x"] {
            assert_eq!(array_index_key(key), None, "{key:?}");
        }
        let ordered: Vec<_> = js_key_order([("b", 1), ("10", 2), ("a", 3), ("2", 4), ("01", 5)])
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(ordered, ["2", "10", "b", "a", "01"]);
        let parsed: Value =
            serde_json::from_str(r#"{"z": {"y": 1, "3": 2}, "1": [{"b": 0, "0": 0}]}"#)
                .expect("valid JSON");
        assert_eq!(
            serde_json::to_string(&with_js_key_order(parsed)).expect("serializable"),
            r#"{"1":[{"0":0,"b":0}],"z":{"3":2,"y":1}}"#
        );
    }

    #[test]
    fn number_value_normalizes_integral_results() {
        assert_eq!(number_value(2.0), Some(json!(2)));
        assert_eq!(number_value(-0.0), Some(json!(0)));
        assert_eq!(number_value(0.5), Some(json!(0.5)));
        assert_eq!(number_value(1e300), Some(json!(1e300)));
        assert_eq!(number_value(f64::INFINITY), None);
        assert_eq!(number_value(f64::NAN), None);
    }

    #[test]
    fn strict_equality_compares_numbers_by_value_and_objects_without_order() {
        assert!(strict_equals(Some(&json!(1)), Some(&json!(1.0))));
        assert!(strict_equals(
            Some(&json!({"a": 1, "b": [1, {"c": null}]})),
            Some(&json!({"b": [1.0, {"c": null}], "a": 1}))
        ));
        assert!(!strict_equals(Some(&json!(1)), Some(&json!("1"))));
        assert!(!strict_equals(Some(&json!([])), Some(&json!({}))));
        assert!(!strict_equals(
            Some(&json!({"a": 1})),
            Some(&json!({"a": 1, "b": 2}))
        ));
        assert!(!strict_equals(Some(&json!(null)), None));
        assert!(strict_equals(None, None));
    }

    #[test]
    fn display_string_matches_javascript_string_conversion() {
        assert_eq!(to_display_string(&json!(null)), "null");
        assert_eq!(to_display_string(&json!(true)), "true");
        assert_eq!(to_display_string(&json!(1e21)), "1e+21");
        assert_eq!(
            to_display_string(&json!([1, null, [2, 3], "x"])),
            "1,,2,3,x"
        );
        assert_eq!(to_display_string(&json!([{"a": 1}])), "[object Object]");
        assert_eq!(to_display_string(&json!({})), "[object Object]");
    }

    #[test]
    fn json_stringify_formats_numbers_like_javascript() {
        assert_eq!(
            json_stringify(&json!({"a": [1, 1.5, 1e21, null], "b\"": "x\n"})),
            r#"{"a":[1,1.5,1e+21,null],"b\"":"x\n"}"#
        );
    }

    #[test]
    fn round_to_fixed_resolves_exact_ties_away_from_zero() {
        // 1/2048 = 0.00048828125 is an exact tie at ten decimal places.
        assert_eq!(round_to_fixed(1.0 / 2048.0, 10), 0.000_488_281_3);
        assert_eq!(round_to_fixed(-1.0 / 2048.0, 10), -0.000_488_281_3);
        assert_eq!(round_to_fixed(0.25, 1), 0.3);
        // 0.35 is stored slightly below the tie and therefore rounds down.
        assert_eq!(round_to_fixed(0.35, 1), 0.3);
        assert_eq!(round_to_fixed(0.999_999_999_99, 10), 1.0);
        assert_eq!(round_to_fixed(6.123_233_995_736_766e-17, 10), 0.0);
    }

    #[test]
    fn utf16_length_counts_surrogate_pairs_twice() {
        assert_eq!(utf16_len("a😀b"), 4);
        assert_eq!(utf16_len("zażółć"), 6);
    }

    #[test]
    fn trim_matches_javascript_white_space() {
        assert_eq!(trim("\u{feff}\t a b \u{a0}\n\u{2028}"), "a b");
        assert_eq!(trim("\u{85}x\u{85}"), "\u{85}x\u{85}");
    }
}
