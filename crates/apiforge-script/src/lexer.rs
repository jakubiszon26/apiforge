use crate::{Diagnostic, MAX_DIAGNOSTICS, MAX_SOURCE_BYTES};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    Comment,
    Identifier,
    Number,
    String,
    Symbol,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

/// Lossless and bounded: even rejected input is represented by tokens covering
/// every byte. No token slice can split a UTF-8 character.
pub fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let (tokens, diagnostics, _) = lex_counted(source);
    (tokens, diagnostics)
}

/// As `lex`, with the number of diagnostics dropped beyond `MAX_DIAGNOSTICS`.
pub(crate) fn lex_counted(source: &str) -> (Vec<Token>, Vec<Diagnostic>, usize) {
    if source.len() > MAX_SOURCE_BYTES {
        return (
            vec![Token {
                kind: TokenKind::Error,
                range: 0..source.len(),
            }],
            vec![Diagnostic::error(
                0..source.len(),
                "Source exceeds the 1 MiB limit",
            )],
            0,
        );
    }
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut omitted = 0;
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let kind = match bytes[at] {
            b if b.is_ascii_whitespace() => {
                at += 1;
                while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                    at += 1;
                }
                TokenKind::Whitespace
            }
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                at += 2;
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
                TokenKind::Comment
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                at += 1;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
                TokenKind::Identifier
            }
            b if b.is_ascii_digit() => {
                at += 1;
                while at < bytes.len() && bytes[at].is_ascii_digit() {
                    at += 1;
                }
                if bytes.get(at) == Some(&b'.') && bytes.get(at + 1).is_some_and(u8::is_ascii_digit)
                {
                    at += 1;
                    while at < bytes.len() && bytes[at].is_ascii_digit() {
                        at += 1;
                    }
                }
                if matches!(bytes.get(at), Some(b'e' | b'E')) {
                    at += 1;
                    if matches!(bytes.get(at), Some(b'+' | b'-')) {
                        at += 1;
                    }
                    while at < bytes.len() && bytes[at].is_ascii_digit() {
                        at += 1;
                    }
                }
                TokenKind::Number
            }
            b'"' => {
                let (end, valid) = scan_string(source, at, 0);
                at = end;
                if !valid && diagnostics.len() < MAX_DIAGNOSTICS {
                    diagnostics.push(Diagnostic::error(
                        start..at,
                        "Unterminated string or interpolation",
                    ));
                } else if !valid {
                    omitted += 1;
                }
                TokenKind::String
            }
            b if b.is_ascii() && b"{}[](),:.@=+-*/%!?<>|&~".contains(&b) => {
                at += 1;
                if matches!(
                    bytes.get(start..start + 2),
                    Some(b"==" | b"!=" | b"<=" | b">=" | b"&&" | b"||" | b"~=")
                ) {
                    at += 1;
                }
                TokenKind::Symbol
            }
            _ => {
                at += source[at..].chars().next().map_or(1, char::len_utf8);
                if diagnostics.len() < MAX_DIAGNOSTICS {
                    diagnostics.push(Diagnostic::error(start..at, "Unexpected character"));
                } else {
                    omitted += 1;
                }
                TokenKind::Error
            }
        };
        tokens.push(Token {
            kind,
            range: start..at,
        });
    }
    (tokens, diagnostics, omitted)
}

pub(crate) fn scan_string(source: &str, start: usize, depth: usize) -> (usize, bool) {
    if depth >= crate::MAX_DEPTH {
        return (source.len(), false);
    }
    let triple = source[start..].starts_with("\"\"\"");
    let delimiter = if triple { "\"\"\"" } else { "\"" };
    let mut at = start + delimiter.len();
    while at < source.len() {
        if source[at..].starts_with(delimiter) {
            return (at + delimiter.len(), true);
        }
        if source.as_bytes()[at] == b'\\' {
            at += 1;
            if at < source.len() {
                at += source[at..].chars().next().unwrap().len_utf8();
            }
        } else if source[at..].starts_with("{{") {
            let (end, valid) = interpolation_end(source, at + 2, depth + 1);
            at = end;
            if !valid {
                return (at, false);
            }
        } else {
            at += source[at..].chars().next().unwrap().len_utf8();
        }
    }
    (at, false)
}

/// Returns the position after `}}`, respecting object braces and nested strings.
pub(crate) fn interpolation_end(source: &str, start: usize, depth: usize) -> (usize, bool) {
    if depth >= crate::MAX_DEPTH {
        return (source.len(), false);
    }
    let mut at = start;
    let mut braces: usize = 0;
    while at < source.len() {
        if braces == 0 && source[at..].starts_with("}}") {
            return (at + 2, true);
        }
        match source.as_bytes()[at] {
            b'"' => {
                let (end, valid) = scan_string(source, at, depth + 1);
                at = end;
                if !valid {
                    return (at, false);
                }
            }
            b'{' => {
                braces += 1;
                at += 1;
            }
            b'}' => {
                braces = braces.saturating_sub(1);
                at += 1;
            }
            b'/' if source[at..].starts_with("//") => {
                at += source[at..].find('\n').unwrap_or(source.len() - at);
            }
            _ => at += source[at..].chars().next().unwrap().len_utf8(),
        }
    }
    (at, false)
}
