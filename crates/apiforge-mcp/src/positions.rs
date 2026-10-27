//! Byte offsets to line and column: 1-based, counted in Unicode scalar
//! values, a `\r` before `\n` part of the line break.
use std::ops::Range;

/// `(line, column, end_line, end_column)`, the end exclusive.
pub fn range(_text: &str, _range: Range<usize>) -> (usize, usize, usize, usize) {
    unimplemented!("MCP-01")
}
