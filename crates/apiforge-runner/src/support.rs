//! Small runtime helpers: report timestamps and the default random source.

use apiforge_core::operator::RandomSource;
use std::{
    hash::{BuildHasher, Hasher},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// A UTC timestamp in the format of JavaScript `Date.prototype.toISOString`,
/// e.g. `2026-10-19T08:15:30.123Z`.
pub fn rfc3339_millis(time: SystemTime) -> String {
    let since_epoch = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let seconds = since_epoch.as_secs();
    let (days, of_day) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60,
        since_epoch.subsec_millis()
    )
}

/// Gregorian date of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// Whether `text` is an RFC 3339 date-time such as `toISOString` produces
/// (`YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`), with plausible field ranges.
pub fn is_rfc3339(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = text.get(range)?;
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let fields = (|| {
        let (month, day) = (digits(5..7)?, digits(8..10)?);
        let (hour, minute, second) = (digits(11..13)?, digits(14..16)?, digits(17..19)?);
        digits(0..4)?;
        let separators = bytes.get(4) == Some(&b'-')
            && bytes.get(7) == Some(&b'-')
            && matches!(bytes.get(10), Some(b'T' | b't'))
            && bytes.get(13) == Some(&b':')
            && bytes.get(16) == Some(&b':');
        Some(
            separators
                && (1..=12).contains(&month)
                && (1..=31).contains(&day)
                && hour < 24
                && minute < 60
                && second < 61,
        )
    })();
    if fields != Some(true) {
        return false;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let length = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if length == 0 {
            return false;
        }
        rest = &fraction[length..];
    }
    match rest.as_bytes() {
        [b'Z' | b'z'] => true,
        [b'+' | b'-', h1, h2, b':', m1, m2] => [h1, h2, m1, m2].iter().all(|b| b.is_ascii_digit()),
        _ => false,
    }
}

/// Default source for the `random` operator: SplitMix64, seeded per run.
///
/// Not cryptographic. The seed can be fixed for reproducible runs.
#[derive(Clone, Debug)]
pub struct SeededRandom {
    state: u64,
}

impl SeededRandom {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// A generator with an unpredictable seed from the standard library's
    /// per-process random hashing keys and the clock.
    pub fn from_entropy() -> Self {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        hasher.write_u128(nanos);
        Self::new(hasher.finish())
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl RandomSource for SeededRandom {
    fn next_unit(&mut self) -> f64 {
        // The top 53 bits give every double in [0, 1) with a 2^-53 step.
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Text form of a byte limit, e.g. `2 MiB`.
pub fn byte_size(bytes: usize) -> String {
    const MIB: usize = 1024 * 1024;
    if bytes >= MIB && bytes.is_multiple_of(MIB) {
        format!("{} MiB", bytes / MIB)
    } else {
        format!("{bytes} bytes")
    }
}

/// Text form of a duration limit, e.g. `120 s` or `250 ms`.
pub fn duration_label(duration: Duration) -> String {
    if duration.subsec_millis() == 0 && duration.as_secs() > 0 {
        format!("{} s", duration.as_secs())
    } else {
        format!("{} ms", duration.as_millis())
    }
}

/// Capitalises a diagnostic from a lower-level error and ends it with a period.
pub fn sentence(text: &str) -> String {
    let mut characters = text.chars();
    let mut output: String = match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => return String::new(),
    };
    if !output.ends_with('.') {
        output.push('.');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_match_to_iso_string() {
        assert_eq!(rfc3339_millis(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        let time = UNIX_EPOCH + Duration::from_millis(1_792_400_130_123);
        assert_eq!(rfc3339_millis(time), "2026-10-19T08:55:30.123Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(rfc3339_millis(leap), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn rfc3339_validation() {
        for valid in [
            "2026-10-19T08:15:30.123Z",
            "2026-10-19T08:15:30Z",
            "2026-10-19T08:15:30+02:00",
        ] {
            assert!(is_rfc3339(valid), "{valid}");
        }
        for invalid in [
            "",
            "yesterday",
            "2026-13-19T08:15:30Z",
            "2026-10-19 08:15:30Z",
            "2026-10-19T08:15:30.Z",
            "2026-10-19T08:15:30",
            "2026-10-19T08:15:30+0200",
            "２０２６-10-19T08:15:30Z",
        ] {
            assert!(!is_rfc3339(invalid), "{invalid}");
        }
    }

    #[test]
    fn seeded_random_is_deterministic_and_in_range() {
        let (mut a, mut b) = (SeededRandom::new(7), SeededRandom::new(7));
        for _ in 0..1000 {
            let value = a.next_unit();
            assert!((0.0..1.0).contains(&value));
            assert_eq!(value.to_bits(), b.next_unit().to_bits());
        }
    }

    #[test]
    fn labels() {
        assert_eq!(byte_size(2 * 1024 * 1024), "2 MiB");
        assert_eq!(byte_size(1000), "1000 bytes");
        assert_eq!(duration_label(Duration::from_secs(120)), "120 s");
        assert_eq!(duration_label(Duration::from_millis(250)), "250 ms");
        assert_eq!(sentence("missing value"), "Missing value.");
        assert_eq!(sentence("Done."), "Done.");
    }
}
